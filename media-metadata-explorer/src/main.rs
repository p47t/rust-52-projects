use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::io::{stdout, Stdout, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode},
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use libavformat_ffi::safe::{FormatContext, MediaType, Packet, StreamInfo};
use serde::Serialize;
use thiserror::Error;
use walkdir::WalkDir;

type Result<T> = std::result::Result<T, ExplorerError>;

const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "mov", "avi", "webm", "mp3", "flac", "wav", "ogg", "m4a", "aac", "opus",
];

#[derive(Debug, Error)]
enum ExplorerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("libavformat error: {0}")]
    Av(#[from] libavformat_ffi::safe::AvError),
    #[error("failed to render JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("path does not exist: {0}")]
    MissingPath(String),
    #[error("path is not a file: {0}")]
    NotAFile(String),
    #[error("path is not a directory: {0}")]
    NotADirectory(String),
}

#[derive(Debug, Parser)]
#[command(
    name = "media-metadata-explorer",
    version,
    about = "Inspect media metadata and build directory-level summaries using libavformat"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Inspect a single media file
    Inspect {
        /// File to inspect
        input: PathBuf,
        /// Print structured JSON instead of text output
        #[arg(long)]
        json: bool,
    },
    /// Scan a directory and summarize media metadata
    Catalog {
        /// Directory to scan
        dir: PathBuf,
        /// Recurse into subdirectories
        #[arg(long)]
        recursive: bool,
        /// Print structured JSON instead of text output
        #[arg(long)]
        json: bool,
    },
    /// Interactive text UI tree for container, streams, and packets
    Tui {
        /// File to inspect interactively
        input: PathBuf,
        /// Maximum packets to read into the tree
        #[arg(long, default_value_t = 2000)]
        max_packets: usize,
    },
}

#[derive(Debug, Serialize, Clone)]
struct MediaReport {
    path: String,
    format_name: Option<String>,
    duration_seconds: Option<f64>,
    size_bytes: Option<u64>,
    bit_rate_bps: Option<u64>,
    tags: BTreeMap<String, String>,
    streams: Vec<StreamReport>,
}

#[derive(Debug, Serialize, Clone)]
struct StreamReport {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    frame_rate_fps: Option<f64>,
    sample_rate_hz: Option<u32>,
    channels: Option<u32>,
    bit_rate_bps: Option<u64>,
    language: Option<String>,
}

#[derive(Debug, Serialize)]
struct CatalogReport {
    root: String,
    files_scanned: usize,
    media_candidates: usize,
    successful: usize,
    failed: usize,
    total_duration_seconds: f64,
    containers: Vec<NameCount>,
    codecs: Vec<NameCount>,
    failures: Vec<ProbeFailure>,
}

#[derive(Debug, Serialize)]
struct NameCount {
    name: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct ProbeFailure {
    path: String,
    error: String,
}

#[derive(Debug, Clone)]
struct PacketReport {
    index: usize,
    stream_index: i32,
    pts: i64,
    dts: i64,
    duration: i64,
    size: i32,
    pos: i64,
    is_keyframe: bool,
}

#[derive(Debug, Clone)]
struct TreeNode {
    id: usize,
    label: String,
    children: Vec<TreeNode>,
}

#[derive(Debug, Clone)]
struct FlatLine {
    node_id: usize,
    label: String,
    depth: usize,
    has_children: bool,
    expanded: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Inspect { input, json } => {
            let report = probe_media_file(&input)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_media_report(&report);
            }
        }
        Commands::Catalog {
            dir,
            recursive,
            json,
        } => {
            let (files_scanned, media_candidates) = collect_candidates(&dir, recursive)?;
            let mut reports = Vec::with_capacity(media_candidates.len());
            let mut failures = Vec::new();

            for path in media_candidates {
                match probe_media_file(&path) {
                    Ok(report) => reports.push(report),
                    Err(error) => failures.push(ProbeFailure {
                        path: path.display().to_string(),
                        error: error.to_string(),
                    }),
                }
            }

            let report = build_catalog_report(&dir, files_scanned, reports, failures);

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_catalog_report(&report);
            }
        }
        Commands::Tui { input, max_packets } => {
            run_tui(&input, max_packets)?;
        }
    }

    Ok(())
}

#[derive(Debug, Default)]
struct TreeState {
    selected: usize,
    scroll: usize,
    expanded: BTreeSet<usize>,
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter(out: &mut Stdout) -> Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(out, EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let mut out = stdout();
        let _ = execute!(out, Show, LeaveAlternateScreen);
    }
}

fn run_tui(path: &Path, max_packets: usize) -> Result<()> {
    if !path.exists() {
        return Err(ExplorerError::MissingPath(path.display().to_string()));
    }
    if !path.is_file() {
        return Err(ExplorerError::NotAFile(path.display().to_string()));
    }

    let mut context = FormatContext::open(path)?;
    let stream_infos = context.streams();
    let report = media_report_from_context(path, &context, stream_infos.clone());
    let (packets, truncated) = capture_packets(&mut context, max_packets)?;
    let tree = build_tui_tree(&report, &stream_infos, &packets, truncated, max_packets);

    let mut state = TreeState::default();
    state.expanded.insert(tree.id);
    for child in &tree.children {
        state.expanded.insert(child.id);
    }

    let mut out = stdout();
    let _guard = TerminalGuard::enter(&mut out)?;

    let mut lines = Vec::new();
    let mut dirty = true;

    loop {
        if dirty {
            lines.clear();
            flatten_tree(&tree, &state.expanded, 0, &mut lines);
            if lines.is_empty() {
                break;
            }

            if state.selected >= lines.len() {
                state.selected = lines.len().saturating_sub(1);
            }

            render_tree(&mut out, &lines, &mut state)?;
            dirty = false;
        }

        match event::read()? {
            Event::Key(key) => {
                let mut changed = false;
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => {
                        if state.selected + 1 < lines.len() {
                            state.selected += 1;
                            changed = true;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let previous = state.selected;
                        state.selected = state.selected.saturating_sub(1);
                        changed = state.selected != previous;
                    }
                    KeyCode::PageDown => {
                        let page = 10usize;
                        let previous = state.selected;
                        state.selected = (state.selected + page).min(lines.len().saturating_sub(1));
                        changed = state.selected != previous;
                    }
                    KeyCode::PageUp => {
                        let page = 10usize;
                        let previous = state.selected;
                        state.selected = state.selected.saturating_sub(page);
                        changed = state.selected != previous;
                    }
                    KeyCode::Home => {
                        if state.selected != 0 {
                            state.selected = 0;
                            changed = true;
                        }
                    }
                    KeyCode::End => {
                        let last = lines.len().saturating_sub(1);
                        if state.selected != last {
                            state.selected = last;
                            changed = true;
                        }
                    }
                    KeyCode::Right | KeyCode::Enter | KeyCode::Char('l') => {
                        let line = &lines[state.selected];
                        if line.has_children && !state.expanded.contains(&line.node_id) {
                            state.expanded.insert(line.node_id);
                            changed = true;
                        }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        let line = &lines[state.selected];
                        if line.has_children
                            && state.expanded.contains(&line.node_id)
                            && line.depth > 0
                        {
                            state.expanded.remove(&line.node_id);
                            changed = true;
                        } else if let Some(parent_idx) = find_parent_index(&lines, state.selected) {
                            if parent_idx != state.selected {
                                state.selected = parent_idx;
                                changed = true;
                            }
                        }
                    }
                    KeyCode::Char(' ') => {
                        let line = &lines[state.selected];
                        if line.has_children {
                            if state.expanded.contains(&line.node_id) && line.depth > 0 {
                                state.expanded.remove(&line.node_id);
                                changed = true;
                            } else if !state.expanded.contains(&line.node_id) {
                                state.expanded.insert(line.node_id);
                                changed = true;
                            }
                        }
                    }
                    _ => {}
                }
                dirty = dirty || changed;
            }
            Event::Resize(_, _) => dirty = true,
            _ => {}
        }
    }

    Ok(())
}

fn render_tree(out: &mut Stdout, lines: &[FlatLine], state: &mut TreeState) -> Result<()> {
    let (width, height) = terminal::size()?;
    let width = width as usize;
    let height = height as usize;
    let body_height = height.saturating_sub(2).max(1);

    if state.selected < state.scroll {
        state.scroll = state.selected;
    }
    if state.selected >= state.scroll + body_height {
        state.scroll = state.selected + 1 - body_height;
    }

    queue!(out, MoveTo(0, 0), Clear(ClearType::All))?;

    let header = "TUI: q quit | Up/Down or j/k move | Enter/Right/Space expand | Left collapse";
    queue!(
        out,
        SetForegroundColor(Color::Cyan),
        Print(truncate_for_width(header, width)),
        ResetColor,
        Clear(ClearType::UntilNewLine)
    )?;

    for row in 0..body_height {
        let line_index = state.scroll + row;
        queue!(
            out,
            MoveTo(0, (row + 1) as u16),
            Clear(ClearType::CurrentLine)
        )?;
        if line_index >= lines.len() {
            continue;
        }

        let line = &lines[line_index];
        let selected = line_index == state.selected;
        if selected {
            queue!(
                out,
                SetBackgroundColor(Color::DarkBlue),
                SetForegroundColor(Color::White)
            )?;
        } else {
            queue!(
                out,
                SetBackgroundColor(Color::Reset),
                SetForegroundColor(line_color(&line.label))
            )?;
        }

        let marker = if line_index == state.selected {
            ">"
        } else {
            " "
        };
        let indent = "  ".repeat(line.depth);
        let branch = if line.has_children {
            if line.expanded {
                "[-]"
            } else {
                "[+]"
            }
        } else {
            "   "
        };
        let text = format!("{marker}{indent}{branch} {}", line.label);
        queue!(out, Print(truncate_for_width(&text, width)))?;
        queue!(out, ResetColor)?;
    }

    queue!(
        out,
        MoveTo(0, (height.saturating_sub(1)) as u16),
        Clear(ClearType::CurrentLine)
    )?;
    let footer = format!(
        "Node {} of {}",
        state.selected.saturating_add(1),
        lines.len()
    );
    queue!(
        out,
        SetForegroundColor(Color::DarkGrey),
        Print(truncate_for_width(&footer, width)),
        ResetColor
    )?;
    out.flush()?;

    Ok(())
}

fn line_color(label: &str) -> Color {
    if label.starts_with("file ") {
        Color::White
    } else if label == "container"
        || label.starts_with("format: ")
        || label.starts_with("duration: ")
        || label.starts_with("size: ")
        || label.starts_with("bitrate: ")
    {
        Color::Magenta
    } else if label.starts_with("streams") || label.starts_with("stream #") {
        Color::Blue
    } else if label.starts_with("packets captured")
        || label.starts_with("stream ") && label.contains(" packets")
    {
        Color::DarkGreen
    } else if label.starts_with("packet #") {
        Color::Green
    } else if label == "tags" || label == "metadata" {
        Color::Yellow
    } else if label.starts_with("keyframe: ") {
        if label.ends_with("true") {
            Color::Green
        } else {
            Color::DarkYellow
        }
    } else {
        Color::Grey
    }
}

fn truncate_for_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return text.to_string();
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let mut out = String::new();
    for c in chars.into_iter().take(width - 3) {
        out.push(c);
    }
    out.push_str("...");
    out
}

fn find_parent_index(lines: &[FlatLine], current_idx: usize) -> Option<usize> {
    let current_depth = lines.get(current_idx)?.depth;
    if current_depth == 0 {
        return None;
    }

    let mut idx = current_idx;
    while idx > 0 {
        idx -= 1;
        if lines[idx].depth == current_depth - 1 {
            return Some(idx);
        }
    }
    None
}

fn flatten_tree(
    node: &TreeNode,
    expanded_ids: &BTreeSet<usize>,
    depth: usize,
    out: &mut Vec<FlatLine>,
) {
    let has_children = !node.children.is_empty();
    let expanded = expanded_ids.contains(&node.id);
    out.push(FlatLine {
        node_id: node.id,
        label: node.label.clone(),
        depth,
        has_children,
        expanded,
    });

    if has_children && expanded {
        for child in &node.children {
            flatten_tree(child, expanded_ids, depth + 1, out);
        }
    }
}

fn new_node(next_id: &mut usize, label: impl Into<String>, children: Vec<TreeNode>) -> TreeNode {
    let node = TreeNode {
        id: *next_id,
        label: label.into(),
        children,
    };
    *next_id += 1;
    node
}

fn build_tui_tree(
    report: &MediaReport,
    stream_infos: &[StreamInfo],
    packets: &[PacketReport],
    truncated_packets: bool,
    max_packets: usize,
) -> TreeNode {
    let mut next_id = 0usize;

    let mut container_children = Vec::new();
    container_children.push(new_node(
        &mut next_id,
        format!(
            "format: {}",
            report.format_name.as_deref().unwrap_or("unknown")
        ),
        Vec::new(),
    ));
    if let Some(duration) = report.duration_seconds {
        container_children.push(new_node(
            &mut next_id,
            format!("duration: {} ({duration:.2}s)", format_duration(duration)),
            Vec::new(),
        ));
    }
    if let Some(size) = report.size_bytes {
        container_children.push(new_node(
            &mut next_id,
            format!("size: {}", format_bytes(size)),
            Vec::new(),
        ));
    }
    if let Some(bitrate) = report.bit_rate_bps {
        container_children.push(new_node(
            &mut next_id,
            format!("bitrate: {}", format_bit_rate(bitrate)),
            Vec::new(),
        ));
    }
    if !report.tags.is_empty() {
        let tag_nodes = report
            .tags
            .iter()
            .map(|(key, value)| new_node(&mut next_id, format!("{key}: {value}"), Vec::new()))
            .collect();
        container_children.push(new_node(&mut next_id, "tags", tag_nodes));
    }
    let container_node = new_node(&mut next_id, "container", container_children);

    let mut stream_nodes = Vec::new();
    for stream in stream_infos {
        let codec = stream.codec_name.as_deref().unwrap_or("unknown");
        let title = format!(
            "stream #{} [{}] codec={}",
            stream.index,
            media_type_name(stream.media_type),
            codec
        );

        let mut details = Vec::new();
        if stream.width > 0 && stream.height > 0 {
            details.push(new_node(
                &mut next_id,
                format!("resolution: {}x{}", stream.width, stream.height),
                Vec::new(),
            ));
        }
        if let Some(frame_rate_fps) = stream.avg_frame_rate_fps() {
            details.push(new_node(
                &mut next_id,
                format!("frame_rate: {frame_rate_fps:.3} fps"),
                Vec::new(),
            ));
        }
        if stream.sample_rate > 0 {
            details.push(new_node(
                &mut next_id,
                format!("sample_rate: {} Hz", stream.sample_rate),
                Vec::new(),
            ));
        }
        if stream.channels > 0 {
            details.push(new_node(
                &mut next_id,
                format!("channels: {}", stream.channels),
                Vec::new(),
            ));
        }
        if stream.bit_rate > 0 {
            details.push(new_node(
                &mut next_id,
                format!("bitrate: {}", format_bit_rate(stream.bit_rate as u64)),
                Vec::new(),
            ));
        }
        if let Some(duration) = stream.duration_secs() {
            details.push(new_node(
                &mut next_id,
                format!("duration: {} ({duration:.2}s)", format_duration(duration)),
                Vec::new(),
            ));
        }
        if !stream.metadata.is_empty() {
            let tag_nodes = stream
                .metadata
                .iter()
                .map(|(key, value)| new_node(&mut next_id, format!("{key}: {value}"), Vec::new()))
                .collect();
            details.push(new_node(&mut next_id, "metadata", tag_nodes));
        }

        stream_nodes.push(new_node(&mut next_id, title, details));
    }
    let streams_node = new_node(
        &mut next_id,
        format!("streams ({})", stream_infos.len()),
        stream_nodes,
    );

    let mut packets_by_stream: BTreeMap<i32, Vec<&PacketReport>> = BTreeMap::new();
    for packet in packets {
        packets_by_stream
            .entry(packet.stream_index)
            .or_default()
            .push(packet);
    }

    let mut packet_groups = Vec::new();
    for (stream_index, stream_packets) in packets_by_stream {
        let mut packet_nodes = Vec::new();
        for packet in stream_packets {
            let packet_label = format!(
                "packet #{:05} size={} keyframe={}",
                packet.index,
                packet.size,
                if packet.is_keyframe { "yes" } else { "no" }
            );
            let packet_fields = vec![
                new_node(
                    &mut next_id,
                    format!("stream_index: {}", packet.stream_index),
                    Vec::new(),
                ),
                new_node(&mut next_id, format!("pts: {}", packet.pts), Vec::new()),
                new_node(&mut next_id, format!("dts: {}", packet.dts), Vec::new()),
                new_node(
                    &mut next_id,
                    format!("duration: {}", packet.duration),
                    Vec::new(),
                ),
                new_node(&mut next_id, format!("size: {}", packet.size), Vec::new()),
                new_node(
                    &mut next_id,
                    format!("position: {}", packet.pos),
                    Vec::new(),
                ),
                new_node(
                    &mut next_id,
                    format!("keyframe: {}", packet.is_keyframe),
                    Vec::new(),
                ),
            ];
            packet_nodes.push(new_node(&mut next_id, packet_label, packet_fields));
        }
        packet_groups.push(new_node(
            &mut next_id,
            format!("stream {} packets ({})", stream_index, packet_nodes.len()),
            packet_nodes,
        ));
    }

    let packet_header = if truncated_packets {
        format!(
            "packets captured {} (truncated at {})",
            packets.len(),
            max_packets
        )
    } else {
        format!("packets captured {} (complete)", packets.len())
    };
    let packets_node = new_node(&mut next_id, packet_header, packet_groups);

    new_node(
        &mut next_id,
        format!("file {}", report.path),
        vec![container_node, streams_node, packets_node],
    )
}

fn capture_packets(
    context: &mut FormatContext,
    max_packets: usize,
) -> Result<(Vec<PacketReport>, bool)> {
    if max_packets == 0 {
        return Ok((Vec::new(), false));
    }

    let mut packet = Packet::new()?;
    let mut packets = Vec::with_capacity(max_packets);

    while packets.len() < max_packets {
        if !context.read_packet(&mut packet)? {
            return Ok((packets, false));
        }

        packets.push(PacketReport {
            index: packets.len(),
            stream_index: packet.stream_index(),
            pts: packet.pts(),
            dts: packet.dts(),
            duration: packet.duration(),
            size: packet.size(),
            pos: packet.pos(),
            is_keyframe: packet.is_keyframe(),
        });
    }

    let truncated = context.read_packet(&mut packet)?;
    Ok((packets, truncated))
}

fn collect_candidates(root: &Path, recursive: bool) -> Result<(usize, Vec<PathBuf>)> {
    if !root.exists() {
        return Err(ExplorerError::MissingPath(root.display().to_string()));
    }

    if !root.is_dir() {
        return Err(ExplorerError::NotADirectory(root.display().to_string()));
    }

    let mut files_scanned = 0;
    let mut media_candidates = Vec::new();

    if recursive {
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            if entry.file_type().is_file() {
                files_scanned += 1;
                let path = entry.path().to_path_buf();
                if is_media_file(&path) {
                    media_candidates.push(path);
                }
            }
        }
    } else {
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                files_scanned += 1;
                if is_media_file(&path) {
                    media_candidates.push(path);
                }
            }
        }
    }

    Ok((files_scanned, media_candidates))
}

fn is_media_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| MEDIA_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn probe_media_file(path: &Path) -> Result<MediaReport> {
    if !path.exists() {
        return Err(ExplorerError::MissingPath(path.display().to_string()));
    }

    if !path.is_file() {
        return Err(ExplorerError::NotAFile(path.display().to_string()));
    }

    let context = FormatContext::open(path)?;
    let streams = context.streams();
    Ok(media_report_from_context(path, &context, streams))
}

fn media_report_from_context(
    path: &Path,
    context: &FormatContext,
    streams: Vec<StreamInfo>,
) -> MediaReport {
    MediaReport {
        path: path.display().to_string(),
        format_name: context.format_name(),
        duration_seconds: context.duration_secs(),
        size_bytes: context.size_bytes().and_then(to_u64),
        bit_rate_bps: context.bit_rate().and_then(to_u64),
        tags: context.metadata(),
        streams: streams.into_iter().map(stream_report_from_info).collect(),
    }
}

fn stream_report_from_info(stream: StreamInfo) -> StreamReport {
    let frame_rate_fps = stream.avg_frame_rate_fps();
    StreamReport {
        index: stream.index as u32,
        codec_type: Some(media_type_name(stream.media_type).to_string()),
        codec_name: stream.codec_name,
        width: to_u32(stream.width),
        height: to_u32(stream.height),
        frame_rate_fps,
        sample_rate_hz: to_u32(stream.sample_rate),
        channels: to_u32(stream.channels),
        bit_rate_bps: to_u64(stream.bit_rate),
        language: stream.language,
    }
}

fn media_type_name(media_type: MediaType) -> &'static str {
    match media_type {
        MediaType::Unknown => "unknown",
        MediaType::Video => "video",
        MediaType::Audio => "audio",
        MediaType::Data => "data",
        MediaType::Subtitle => "subtitle",
        MediaType::Attachment => "attachment",
    }
}

fn build_catalog_report(
    root: &Path,
    files_scanned: usize,
    reports: Vec<MediaReport>,
    failures: Vec<ProbeFailure>,
) -> CatalogReport {
    let successful = reports.len();
    let failed = failures.len();
    let media_candidates = successful + failed;
    let total_duration_seconds = reports
        .iter()
        .filter_map(|report| report.duration_seconds)
        .sum();

    let mut containers = HashMap::new();
    let mut codecs = HashMap::new();

    for report in &reports {
        if let Some(format_name) = &report.format_name {
            let primary = format_name
                .split(',')
                .next()
                .map(str::trim)
                .unwrap_or(format_name.as_str())
                .to_string();
            *containers.entry(primary).or_insert(0) += 1;
        }

        for stream in &report.streams {
            if let Some(codec_name) = &stream.codec_name {
                *codecs.entry(codec_name.clone()).or_insert(0) += 1;
            }
        }
    }

    CatalogReport {
        root: root.display().to_string(),
        files_scanned,
        media_candidates,
        successful,
        failed,
        total_duration_seconds,
        containers: sort_counts(containers),
        codecs: sort_counts(codecs),
        failures,
    }
}

fn sort_counts(map: HashMap<String, usize>) -> Vec<NameCount> {
    let mut values: Vec<NameCount> = map
        .into_iter()
        .map(|(name, count)| NameCount { name, count })
        .collect();

    values.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.name.cmp(&right.name))
    });

    values
}

fn to_u64(value: i64) -> Option<u64> {
    if value <= 0 {
        None
    } else {
        u64::try_from(value).ok()
    }
}

fn to_u32(value: i32) -> Option<u32> {
    if value <= 0 {
        None
    } else {
        u32::try_from(value).ok()
    }
}

fn print_media_report(report: &MediaReport) {
    println!("File: {}", report.path);
    println!(
        "Format: {}",
        report.format_name.as_deref().unwrap_or("unknown")
    );

    if let Some(duration_seconds) = report.duration_seconds {
        println!(
            "Duration: {} ({duration_seconds:.2}s)",
            format_duration(duration_seconds)
        );
    }

    if let Some(size_bytes) = report.size_bytes {
        println!("Size: {}", format_bytes(size_bytes));
    }

    if let Some(bit_rate_bps) = report.bit_rate_bps {
        println!("Bit rate: {}", format_bit_rate(bit_rate_bps));
    }

    if !report.tags.is_empty() {
        println!("Tags:");
        for (key, value) in &report.tags {
            println!("  {key}: {value}");
        }
    }

    println!("Streams:");
    for stream in &report.streams {
        println!("  #{}", stream.index);
        if let Some(codec_type) = &stream.codec_type {
            println!("    Type: {codec_type}");
        }
        if let Some(codec_name) = &stream.codec_name {
            println!("    Codec: {codec_name}");
        }
        if let (Some(width), Some(height)) = (stream.width, stream.height) {
            println!("    Resolution: {width}x{height}");
        }
        if let Some(frame_rate_fps) = stream.frame_rate_fps {
            println!("    Frame rate: {frame_rate_fps:.3} fps");
        }
        if let Some(sample_rate_hz) = stream.sample_rate_hz {
            println!("    Sample rate: {sample_rate_hz} Hz");
        }
        if let Some(channels) = stream.channels {
            println!("    Channels: {channels}");
        }
        if let Some(bit_rate_bps) = stream.bit_rate_bps {
            println!("    Bit rate: {}", format_bit_rate(bit_rate_bps));
        }
        if let Some(language) = &stream.language {
            println!("    Language: {language}");
        }
    }
}

fn print_catalog_report(report: &CatalogReport) {
    println!("Root: {}", report.root);
    println!("Files scanned: {}", report.files_scanned);
    println!("Media candidates: {}", report.media_candidates);
    println!("Probed successfully: {}", report.successful);
    println!("Failures: {}", report.failed);
    println!(
        "Total duration: {} ({:.2}s)",
        format_duration(report.total_duration_seconds),
        report.total_duration_seconds
    );

    if !report.containers.is_empty() {
        println!("Top containers:");
        for entry in report.containers.iter().take(8) {
            println!("  {} ({})", entry.name, entry.count);
        }
    }

    if !report.codecs.is_empty() {
        println!("Top codecs:");
        for entry in report.codecs.iter().take(12) {
            println!("  {} ({})", entry.name, entry.count);
        }
    }

    if !report.failures.is_empty() {
        println!("Failed files:");
        for failure in &report.failures {
            println!("  {}", failure.path);
            println!("    {}", failure.error);
        }
    }
}

fn format_duration(total_seconds: f64) -> String {
    if !total_seconds.is_finite() || total_seconds < 0.0 {
        return "unknown".to_string();
    }

    let seconds = total_seconds.round() as u64;
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let remaining_seconds = seconds % 60;

    format!("{hours:02}:{minutes:02}:{remaining_seconds:02}")
}

fn format_bytes(size: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit_index = 0;

    while value >= 1024.0 && unit_index < units.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{size} {}", units[unit_index])
    } else {
        format!("{value:.2} {}", units[unit_index])
    }
}

fn format_bit_rate(bits_per_second: u64) -> String {
    if bits_per_second >= 1_000_000 {
        format!("{:.2} Mbps", bits_per_second as f64 / 1_000_000.0)
    } else if bits_per_second >= 1_000 {
        format!("{:.2} Kbps", bits_per_second as f64 / 1_000.0)
    } else {
        format!("{bits_per_second} bps")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_signed_numbers_to_options() {
        assert_eq!(to_u64(123), Some(123));
        assert_eq!(to_u64(0), None);
        assert_eq!(to_u64(-1), None);
        assert_eq!(to_u32(48_000), Some(48_000));
        assert_eq!(to_u32(0), None);
        assert_eq!(to_u32(-1), None);
    }

    #[test]
    fn aggregates_catalog_counts() {
        let reports = vec![
            MediaReport {
                path: "a.mp4".to_string(),
                format_name: Some("mov,mp4,m4a,3gp,3g2,mj2".to_string()),
                duration_seconds: Some(10.0),
                size_bytes: Some(1000),
                bit_rate_bps: Some(800_000),
                tags: BTreeMap::new(),
                streams: vec![
                    StreamReport {
                        index: 0,
                        codec_type: Some("video".to_string()),
                        codec_name: Some("h264".to_string()),
                        width: Some(1280),
                        height: Some(720),
                        frame_rate_fps: Some(30.0),
                        sample_rate_hz: None,
                        channels: None,
                        bit_rate_bps: Some(600_000),
                        language: None,
                    },
                    StreamReport {
                        index: 1,
                        codec_type: Some("audio".to_string()),
                        codec_name: Some("aac".to_string()),
                        width: None,
                        height: None,
                        frame_rate_fps: None,
                        sample_rate_hz: Some(48_000),
                        channels: Some(2),
                        bit_rate_bps: Some(192_000),
                        language: Some("eng".to_string()),
                    },
                ],
            },
            MediaReport {
                path: "b.webm".to_string(),
                format_name: Some("matroska,webm".to_string()),
                duration_seconds: Some(20.0),
                size_bytes: Some(2000),
                bit_rate_bps: Some(500_000),
                tags: BTreeMap::new(),
                streams: vec![StreamReport {
                    index: 0,
                    codec_type: Some("video".to_string()),
                    codec_name: Some("vp9".to_string()),
                    width: Some(1920),
                    height: Some(1080),
                    frame_rate_fps: Some(60.0),
                    sample_rate_hz: None,
                    channels: None,
                    bit_rate_bps: Some(400_000),
                    language: None,
                }],
            },
        ];

        let failures = vec![ProbeFailure {
            path: "broken.mkv".to_string(),
            error: "probe failed".to_string(),
        }];

        let summary = build_catalog_report(Path::new("media"), 10, reports, failures);

        assert_eq!(summary.files_scanned, 10);
        assert_eq!(summary.media_candidates, 3);
        assert_eq!(summary.successful, 2);
        assert_eq!(summary.failed, 1);
        assert!((summary.total_duration_seconds - 30.0).abs() < 0.001);
        assert_eq!(summary.containers[0].name, "matroska");
        assert_eq!(summary.containers[0].count, 1);
        assert_eq!(summary.codecs[0].name, "aac");
        assert_eq!(summary.codecs[0].count, 1);
    }
}

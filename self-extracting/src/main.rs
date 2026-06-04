use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use clap::{Parser, Subcommand};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use tar::Archive;
use walkdir::WalkDir;

/// Magic bytes at the start of the trailing footer. Picked to be unlikely
/// to appear by accident at exactly the right offset.
const MAGIC: &[u8; 8] = b"SFXR0001";
const FOOTER_LEN: u64 = 8 + 8 + 8; // magic + offset + length

#[derive(Parser)]
#[command(
    name = "sfxr",
    about = "Generic self-extracting archive tool — pack a folder into a stub binary, run the stub to extract.",
    long_about = "When run as a 'plain' (unpacked) stub, this binary exposes `pack` which produces \
                  a new self-extracting executable. When run as a stub with an appended payload, the \
                  same binary auto-extracts (or with `--info` / `--list`)."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Show payload metadata instead of extracting (only valid for a packed stub).
    #[arg(long, global = true, conflicts_with_all = ["list", "dest"])]
    info: bool,

    /// List payload entries instead of extracting (only valid for a packed stub).
    #[arg(long, global = true, conflicts_with = "info")]
    list: bool,

    /// Destination directory when auto-extracting a packed stub.
    #[arg(short, long, global = true, default_value = "extracted")]
    dest: PathBuf,

    /// Overwrite existing files when auto-extracting.
    #[arg(short, long, global = true)]
    force: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Pack a folder into a new self-extracting executable.
    Pack {
        /// Folder whose contents will be packed.
        #[arg(short, long)]
        source: PathBuf,
        /// Output path for the new self-extracting executable.
        #[arg(short, long)]
        output: PathBuf,
        /// Override the stub binary (defaults to the currently running executable).
        #[arg(long)]
        stub: Option<PathBuf>,
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let payload = locate_payload()?;

    match (cli.command, payload) {
        (
            Some(Command::Pack {
                source,
                output,
                stub,
            }),
            _,
        ) => {
            let stub_path = match stub {
                Some(p) => p,
                None => std::env::current_exe()?,
            };
            pack(&stub_path, &source, &output)
        }
        (None, Some(p)) if cli.info => info(&p),
        (None, Some(p)) if cli.list => list(&p),
        (None, Some(p)) => extract(&p, &cli.dest, cli.force),
        (None, None) => {
            eprintln!(
                "no payload is appended to this binary.\n\
                 Use `pack --source <DIR> --output <FILE>` to produce a self-extracting executable."
            );
            std::process::exit(2);
        }
    }
}

/// Location and size of the embedded payload inside the running executable.
struct PayloadLocation {
    exe_path: PathBuf,
    offset: u64,
    length: u64,
}

/// Inspect the running executable for an appended payload footer.
/// Returns `Ok(None)` if no footer is present (i.e. this is a plain stub).
fn locate_payload() -> io::Result<Option<PayloadLocation>> {
    let exe_path = std::env::current_exe()?;
    let mut file = File::open(&exe_path)?;
    let total = file.metadata()?.len();
    if total < FOOTER_LEN {
        return Ok(None);
    }

    file.seek(SeekFrom::End(-(FOOTER_LEN as i64)))?;
    let mut footer = [0u8; FOOTER_LEN as usize];
    file.read_exact(&mut footer)?;

    if &footer[0..8] != MAGIC {
        return Ok(None);
    }
    let offset = u64::from_le_bytes(footer[8..16].try_into().unwrap());
    let length = u64::from_le_bytes(footer[16..24].try_into().unwrap());

    let footer_start = total - FOOTER_LEN;
    if offset
        .checked_add(length)
        .map(|end| end > footer_start)
        .unwrap_or(true)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "payload footer points outside the executable",
        ));
    }

    Ok(Some(PayloadLocation {
        exe_path,
        offset,
        length,
    }))
}

fn pack(stub: &Path, source: &Path, output: &Path) -> io::Result<()> {
    if !source.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("source is not a directory: {}", source.display()),
        ));
    }

    let output = with_exe_suffix_if_windows(output);
    let output = output.as_path();

    // Step 1: copy the stub portion of `stub` into `output`. If `stub`
    // already has a payload appended (because we are running a packed
    // binary), copy only the bytes before the existing payload.
    let stub_size = stub_size(stub)?;
    let mut out_file = File::create(output)?;
    let mut stub_file = File::open(stub)?;
    let mut remaining = stub_size;
    let mut buf = [0u8; 64 * 1024];
    while remaining > 0 {
        let take = (buf.len() as u64).min(remaining) as usize;
        let n = stub_file.read(&mut buf[..take])?;
        if n == 0 {
            break;
        }
        out_file.write_all(&buf[..n])?;
        remaining -= n as u64;
    }
    let payload_offset = out_file.stream_position()?;

    // Step 2: stream a tar.gz of `source` directly into `output`.
    let encoder = GzEncoder::new(&mut out_file, Compression::best());
    let mut builder = tar::Builder::new(encoder);
    // Resolve symlinks to their targets when packing — symlink entries
    // can't be restored on Windows without admin/dev-mode privileges.
    builder.follow_symlinks(true);
    append_dir_sorted(&mut builder, source)?;
    let encoder = builder.into_inner()?;
    encoder.finish()?;

    let payload_end = out_file.stream_position()?;
    let payload_len = payload_end - payload_offset;

    // Step 3: footer.
    out_file.write_all(MAGIC)?;
    out_file.write_all(&payload_offset.to_le_bytes())?;
    out_file.write_all(&payload_len.to_le_bytes())?;
    out_file.flush()?;
    drop(out_file);

    // Step 4: make the result executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(output)?.permissions();
        perms.set_mode(perms.mode() | 0o111);
        fs::set_permissions(output, perms)?;
    }

    println!(
        "wrote self-extracting executable: {} ({} bytes payload from {})",
        output.display(),
        payload_len,
        source.display()
    );
    Ok(())
}

/// On Windows, append `.exe` to a plain output path so the produced file
/// is recognized as executable. On other platforms this is a no-op.
fn with_exe_suffix_if_windows(path: &Path) -> PathBuf {
    if cfg!(windows) && path.extension().is_none() {
        path.with_extension("exe")
    } else {
        path.to_path_buf()
    }
}

/// Size of the executable portion (excluding any appended payload + footer).
fn stub_size(path: &Path) -> io::Result<u64> {
    let mut file = File::open(path)?;
    let total = file.metadata()?.len();
    if total < FOOTER_LEN {
        return Ok(total);
    }
    file.seek(SeekFrom::End(-(FOOTER_LEN as i64)))?;
    let mut footer = [0u8; FOOTER_LEN as usize];
    file.read_exact(&mut footer)?;
    if &footer[0..8] != MAGIC {
        return Ok(total);
    }
    let offset = u64::from_le_bytes(footer[8..16].try_into().unwrap());
    Ok(offset)
}

fn append_dir_sorted<W: Write>(builder: &mut tar::Builder<W>, root: &Path) -> io::Result<()> {
    let mut entries: Vec<_> = WalkDir::new(root)
        .min_depth(1)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
        .collect();
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    for entry in entries {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap();
        // entry.file_type() reflects the resolved target because of follow_links(true).
        if entry.file_type().is_dir() {
            builder.append_dir(rel, path)?;
        } else if entry.file_type().is_file() {
            let mut file = File::open(path)?;
            builder.append_file(rel, &mut file)?;
        }
        // Anything else (broken symlink, fifo, socket, device) is skipped.
    }
    Ok(())
}

fn open_payload(loc: &PayloadLocation) -> io::Result<impl Read> {
    let mut file = File::open(&loc.exe_path)?;
    file.seek(SeekFrom::Start(loc.offset))?;
    Ok(file.take(loc.length))
}

fn info(loc: &PayloadLocation) -> io::Result<()> {
    let mut hasher = Sha256::new();
    let mut reader = open_payload(loc)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();

    let mut decoded_bytes = 0u64;
    let mut entry_count = 0u64;
    let mut archive = Archive::new(GzDecoder::new(open_payload(loc)?));
    for entry in archive.entries()? {
        let entry = entry?;
        decoded_bytes += entry.header().size().unwrap_or(0);
        entry_count += 1;
    }

    println!("executable      : {}", loc.exe_path.display());
    println!("payload offset  : {}", loc.offset);
    println!("compressed size : {} bytes", loc.length);
    println!("decoded size    : {decoded_bytes} bytes");
    println!("entry count     : {entry_count}");
    print!("payload sha256  : ");
    for byte in digest {
        print!("{byte:02x}");
    }
    println!();
    Ok(())
}

fn list(loc: &PayloadLocation) -> io::Result<()> {
    let mut archive = Archive::new(GzDecoder::new(open_payload(loc)?));
    for entry in archive.entries()? {
        let entry = entry?;
        let path = entry.path()?;
        let kind = if entry.header().entry_type().is_dir() {
            "DIR "
        } else {
            "FILE"
        };
        let size = entry.header().size().unwrap_or(0);
        println!("{kind} {size:>10}  {}", path.display());
    }
    Ok(())
}

fn extract(loc: &PayloadLocation, dest: &Path, force: bool) -> io::Result<()> {
    fs::create_dir_all(dest)?;
    let dest = dest.canonicalize()?;
    let mut archive = Archive::new(GzDecoder::new(open_payload(loc)?));

    let mut written = 0u64;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.into_owned();
        let safe_path = sanitize(&entry_path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsafe path in archive: {}", entry_path.display()),
            )
        })?;
        let target = dest.join(&safe_path);

        let etype = entry.header().entry_type();
        if etype.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if !etype.is_file() {
            // Symlinks, hard links, char/block devices, fifos, etc. We
            // skip them: packing already resolves symlinks to files, so
            // the only way to hit this branch is a hand-crafted archive.
            eprintln!(
                "skipping unsupported entry type ({:?}): {}",
                etype,
                safe_path.display()
            );
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if target.exists() && !force {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} already exists; pass --force to overwrite",
                    target.display()
                ),
            ));
        }

        let mut file = File::create(&target)?;
        io::copy(&mut entry, &mut file)?;
        file.flush()?;
        written += 1;
        println!("wrote {}", target.display());
    }
    println!("extracted {written} file(s) to {}", dest.display());
    Ok(())
}

/// Strip any prefix/parent components so an archive can never escape the
/// destination directory ("Zip Slip" defense).
fn sanitize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

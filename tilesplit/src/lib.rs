use std::fs;
use std::fs::File;
use std::panic;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use image::DynamicImage;
use image::codecs::jpeg::JpegEncoder;
use jpegli::decoder::{
    Decoder as JpegDecoder, PixelFormat as JpegPixelFormat, PreserveConfig, StandardProfile,
};
use jpegli::encoder::{ChromaSubsampling, EncoderConfig, PixelLayout, Unstoppable};
#[cfg(test)]
use ultrahdr::Encoder as UltraHdrEncoder;
use ultrahdr::{
    ColorGamut, ColorTransfer, Decoder as UltraHdrDecoder, GainMap, PixelFormat, RawImage,
};

pub const EXIT_USAGE: i32 = 2;
pub const EXIT_INVALID_INPUT: i32 = 3;
pub const EXIT_INVALID_CROP: i32 = 4;
pub const EXIT_UNSUPPORTED_ASPECT: i32 = 10;
pub const EXIT_IO_ERROR: i32 = 11;

const ASPECT_16_10: f64 = 16.0 / 10.0;
const ASPECT_3_2: f64 = 3.0 / 2.0;
const ASPECT_TOLERANCE: f64 = 0.01;
const SDR_TILE_JPEG_QUALITY: f32 = 100.0;
const GAINMAP_JPEG_QUALITY: f32 = 100.0;

#[derive(Clone, Copy)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub struct SplitParams {
    pub input: String,
    pub left_output: String,
    pub right_output: String,
    pub debug: bool,
}

enum UltraHdrSplitOutcome {
    Handled,
    NotUltraHdr,
}

fn catch_unwind_quiet<F, T>(f: F) -> std::thread::Result<T>
where
    F: FnOnce() -> T + panic::UnwindSafe,
{
    let old_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(f);
    panic::set_hook(old_hook);
    result
}

fn debug_log(enabled: bool, message: &str) {
    if enabled {
        eprintln!("[tilesplit] {message}");
    }
}

fn debug_log_metadata(enabled: bool, label: &str, metadata: &ultrahdr::GainMapMetadata) {
    if !enabled {
        return;
    }

    eprintln!(
        "[tilesplit] {label}: min=[{:.4},{:.4},{:.4}] max=[{:.4},{:.4},{:.4}] gamma=[{:.4},{:.4},{:.4}] hdr=[{:.4},{:.4}]",
        metadata.min_content_boost[0],
        metadata.min_content_boost[1],
        metadata.min_content_boost[2],
        metadata.max_content_boost[0],
        metadata.max_content_boost[1],
        metadata.max_content_boost[2],
        metadata.gamma[0],
        metadata.gamma[1],
        metadata.gamma[2],
        metadata.hdr_capacity_min,
        metadata.hdr_capacity_max
    );
}

fn extract_xmp_attribute_value(xmp: &str, attr_name: &str) -> Option<String> {
    let pattern = format!("{attr_name}=\"");
    if let Some(start) = xmp.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = xmp[value_start..].find('"') {
            return Some(xmp[value_start..value_start + end].to_string());
        }
    }

    let open_tag = format!("<{attr_name}>");
    let close_tag = format!("</{attr_name}>");
    if let Some(start) = xmp.find(&open_tag) {
        let value_start = start + open_tag.len();
        if let Some(end) = xmp[value_start..].find(&close_tag) {
            return Some(xmp[value_start..value_start + end].trim().to_string());
        }
    }

    None
}

fn parse_xmp_values_lenient(value: &str) -> [f32; 3] {
    let parsed: Vec<f32> = value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();

    match parsed.len() {
        0 => [0.0; 3],
        1 => [parsed[0]; 3],
        2 => [parsed[0], parsed[1], 0.0],
        _ => [parsed[0], parsed[1], parsed[2]],
    }
}

fn extract_xmp_seq_values(xmp: &str, tag_name: &str) -> Option<[f32; 3]> {
    let open_tag = format!("<{tag_name}>");
    let close_tag = format!("</{tag_name}>");
    let start = xmp.find(&open_tag)?;
    let content_start = start + open_tag.len();
    let end_rel = xmp[content_start..].find(&close_tag)?;
    let content = &xmp[content_start..content_start + end_rel];

    let mut values = Vec::new();
    let mut rest = content;
    while let Some(li_start_rel) = rest.find("<rdf:li>") {
        let li_content_start = li_start_rel + "<rdf:li>".len();
        let Some(li_end_rel) = rest[li_content_start..].find("</rdf:li>") else {
            break;
        };
        let value_str = rest[li_content_start..li_content_start + li_end_rel].trim();
        if let Ok(v) = value_str.parse::<f32>() {
            values.push(v);
        }

        let advance = li_content_start + li_end_rel + "</rdf:li>".len();
        if advance >= rest.len() {
            break;
        }
        rest = &rest[advance..];
    }

    if values.is_empty() {
        return None;
    }

    Some(match values.len() {
        1 => [values[0]; 3],
        2 => [values[0], values[1], 0.0],
        _ => [values[0], values[1], values[2]],
    })
}

fn apply_lenient_xmp_overrides(xmp: &str, metadata: &mut ultrahdr::GainMapMetadata) {
    if let Some(values) = extract_xmp_seq_values(xmp, "hdrgm:GainMapMin").or_else(|| {
        extract_xmp_attribute_value(xmp, "hdrgm:GainMapMin")
            .map(|val| parse_xmp_values_lenient(&val))
    }) {
        for (idx, v) in values.iter().enumerate() {
            metadata.min_content_boost[idx] = 2.0f32.powf(*v);
        }
    }

    if let Some(values) = extract_xmp_seq_values(xmp, "hdrgm:GainMapMax").or_else(|| {
        extract_xmp_attribute_value(xmp, "hdrgm:GainMapMax")
            .map(|val| parse_xmp_values_lenient(&val))
    }) {
        for (idx, v) in values.iter().enumerate() {
            metadata.max_content_boost[idx] = 2.0f32.powf(*v);
        }
    }

    if let Some(values) = extract_xmp_seq_values(xmp, "hdrgm:Gamma").or_else(|| {
        extract_xmp_attribute_value(xmp, "hdrgm:Gamma").map(|val| parse_xmp_values_lenient(&val))
    }) {
        metadata.gamma = values;
    }

    if let Some(val) = extract_xmp_attribute_value(xmp, "hdrgm:OffsetSDR") {
        metadata.offset_sdr = parse_xmp_values_lenient(&val);
    }

    if let Some(val) = extract_xmp_attribute_value(xmp, "hdrgm:OffsetHDR") {
        metadata.offset_hdr = parse_xmp_values_lenient(&val);
    }

    if let Some(val) = extract_xmp_attribute_value(xmp, "hdrgm:HDRCapacityMin")
        && let Ok(v) = val.trim().parse::<f32>()
    {
        metadata.hdr_capacity_min = 2.0f32.powf(v);
    }

    if let Some(val) = extract_xmp_attribute_value(xmp, "hdrgm:HDRCapacityMax")
        && let Ok(v) = val.trim().parse::<f32>()
    {
        metadata.hdr_capacity_max = 2.0f32.powf(v);
    }
}

pub fn default_output_paths(input: &str) -> (String, String) {
    let input_path = Path::new(input);
    let parent = input_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = input_path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("output");

    let left_file = format!("{stem}-left.jpg");
    let right_file = format!("{stem}-right.jpg");

    let left_path = if parent.as_os_str().is_empty() {
        PathBuf::from(left_file)
    } else {
        parent.join(left_file)
    };
    let right_path = if parent.as_os_str().is_empty() {
        PathBuf::from(right_file)
    } else {
        parent.join(right_file)
    };

    (
        left_path.to_string_lossy().into_owned(),
        right_path.to_string_lossy().into_owned(),
    )
}

fn is_close(a: f64, b: f64) -> bool {
    (a - b).abs() <= ASPECT_TOLERANCE
}

fn gamut_from_standard_profile(profile: Option<StandardProfile>) -> ColorGamut {
    match profile {
        Some(StandardProfile::DisplayP3) => ColorGamut::DisplayP3,
        _ => ColorGamut::Bt709,
    }
}

fn gamut_from_icc_bytes(icc: &[u8]) -> ColorGamut {
    // Display P3 profiles contain the "Display P3" description.
    // This is a lightweight heuristic for when we don't have jpegli's StandardProfile detection.
    if icc.windows(10).any(|w| w == b"Display P3") {
        ColorGamut::DisplayP3
    } else {
        ColorGamut::Bt709
    }
}

fn is_jpeg_path(path: &str) -> bool {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    extension == "jpg" || extension == "jpeg"
}

pub fn compute_split_rectangles(width: u32, height: u32) -> Result<(Rect, Rect), i32> {
    if height == 0 {
        return Err(EXIT_INVALID_INPUT);
    }

    let actual_aspect = width as f64 / height as f64;
    if !is_close(actual_aspect, ASPECT_16_10) && !is_close(actual_aspect, ASPECT_3_2) {
        return Err(EXIT_UNSUPPORTED_ASPECT);
    }

    let target_width = (height as f64 * ASPECT_16_10).round() as u32;
    let target_height = (width as f64 / ASPECT_16_10).round() as u32;

    let mut crop_x = 0u32;
    let mut crop_y = 0u32;
    let mut crop_width = width;
    let mut crop_height = height;

    if target_width <= width {
        crop_width = target_width;
        crop_x = (width - target_width) / 2;
    } else {
        crop_height = target_height;
        crop_y = (height - target_height) / 2;
    }

    if !crop_width.is_multiple_of(2) {
        crop_width -= 1;
    }

    if crop_width == 0 || crop_height == 0 {
        return Err(EXIT_INVALID_CROP);
    }

    let half_width = crop_width / 2;

    let left = Rect {
        x: crop_x,
        y: crop_y,
        width: half_width,
        height: crop_height,
    };
    let right = Rect {
        x: crop_x + half_width,
        y: crop_y,
        width: half_width,
        height: crop_height,
    };

    Ok((left, right))
}

fn save_image(img: &DynamicImage, path: &str) -> Result<(), i32> {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if extension == "jpg" || extension == "jpeg" {
        let file = File::create(path).map_err(|_| EXIT_IO_ERROR)?;
        let mut encoder = JpegEncoder::new_with_quality(file, 95);
        encoder.encode_image(img).map_err(|_| EXIT_IO_ERROR)?;
        return Ok(());
    }

    img.save(path).map_err(|_| EXIT_IO_ERROR)
}

fn encode_sdr_tile_jpegli(
    pixels: &[u8],
    width: u32,
    height: u32,
    bytes_per_pixel: usize,
    icc_profile: Option<&[u8]>,
) -> Result<Vec<u8>, i32> {
    let (layout, data): (PixelLayout, std::borrow::Cow<[u8]>) = match bytes_per_pixel {
        3 => (PixelLayout::Rgb8Srgb, std::borrow::Cow::Borrowed(pixels)),
        4 => {
            let rgb: Vec<u8> = pixels
                .chunks_exact(4)
                .flat_map(|rgba| [rgba[0], rgba[1], rgba[2]])
                .collect();
            (PixelLayout::Rgb8Srgb, std::borrow::Cow::Owned(rgb))
        }
        _ => return Err(EXIT_IO_ERROR),
    };

    let mut config = EncoderConfig::ycbcr(SDR_TILE_JPEG_QUALITY, ChromaSubsampling::None);
    if let Some(icc) = icc_profile
        && !icc.is_empty()
    {
        config = config.icc_profile(icc.to_vec());
    }

    let mut encoder = config
        .encode_from_bytes(width, height, layout)
        .map_err(|_| EXIT_IO_ERROR)?;
    encoder
        .push_packed(&data, Unstoppable)
        .map_err(|_| EXIT_IO_ERROR)?;
    encoder.finish().map_err(|_| EXIT_IO_ERROR)
}

fn crop_jpegli_pixels(
    pixels: &[u8],
    src_width: u32,
    src_height: u32,
    bytes_per_pixel: usize,
    rect: Rect,
) -> Result<Vec<u8>, i32> {
    let x_end = rect.x.saturating_add(rect.width);
    let y_end = rect.y.saturating_add(rect.height);
    if x_end > src_width || y_end > src_height || rect.width == 0 || rect.height == 0 {
        return Err(EXIT_INVALID_CROP);
    }

    let src_stride = (src_width as usize)
        .checked_mul(bytes_per_pixel)
        .ok_or(EXIT_IO_ERROR)?;
    let row_len = (rect.width as usize)
        .checked_mul(bytes_per_pixel)
        .ok_or(EXIT_IO_ERROR)?;
    let out_len = row_len
        .checked_mul(rect.height as usize)
        .ok_or(EXIT_IO_ERROR)?;
    let mut out = vec![0u8; out_len];

    for row in 0..rect.height as usize {
        let src_y = rect.y as usize + row;
        let src_off = src_y * src_stride + rect.x as usize * bytes_per_pixel;
        let dst_off = row * row_len;
        out[dst_off..dst_off + row_len].copy_from_slice(&pixels[src_off..src_off + row_len]);
    }

    Ok(out)
}

fn split_standard_jpeg(args: &SplitParams) -> Result<(), i32> {
    let source_bytes = fs::read(&args.input).map_err(|_| EXIT_IO_ERROR)?;

    let decoded = catch_unwind_quiet(AssertUnwindSafe(|| {
        JpegDecoder::new()
            .preserve(PreserveConfig::default())
            .output_format(JpegPixelFormat::Rgb)
            .decode(&source_bytes)
    }))
    .map_err(|_| EXIT_IO_ERROR)?
    .map_err(|_| EXIT_IO_ERROR)?;

    let bpp = decoded.bytes_per_pixel();
    let icc_profile = decoded
        .extras()
        .and_then(|e| e.icc_profile().map(|icc| icc.to_vec()));

    let (left_rect, right_rect) = compute_split_rectangles(decoded.width, decoded.height)?;

    let left_pixels =
        crop_jpegli_pixels(&decoded.data, decoded.width, decoded.height, bpp, left_rect)?;
    let right_pixels =
        crop_jpegli_pixels(&decoded.data, decoded.width, decoded.height, bpp, right_rect)?;

    let left_bytes = encode_sdr_tile_jpegli(
        &left_pixels,
        left_rect.width,
        left_rect.height,
        bpp,
        icc_profile.as_deref(),
    )?;
    let right_bytes = encode_sdr_tile_jpegli(
        &right_pixels,
        right_rect.width,
        right_rect.height,
        bpp,
        icc_profile.as_deref(),
    )?;

    fs::write(&args.left_output, left_bytes).map_err(|_| EXIT_IO_ERROR)?;
    fs::write(&args.right_output, right_bytes).map_err(|_| EXIT_IO_ERROR)?;

    Ok(())
}

fn split_standard_image(args: &SplitParams) -> Result<(), i32> {
    // For JPEG inputs, use jpegli to preserve ICC profiles and encode at high quality.
    if is_jpeg_path(&args.input) {
        if let Ok(()) = split_standard_jpeg(args) {
            return Ok(());
        }
        debug_log(
            args.debug,
            "Standard split: jpegli JPEG path failed, falling back to image crate",
        );
    }

    let image = image::open(&args.input).map_err(|_| EXIT_IO_ERROR)?;
    let (left_rect, right_rect) = compute_split_rectangles(image.width(), image.height())?;

    let left_image = image.crop_imm(left_rect.x, left_rect.y, left_rect.width, left_rect.height);
    let right_image = image.crop_imm(
        right_rect.x,
        right_rect.y,
        right_rect.width,
        right_rect.height,
    );

    save_image(&left_image, &args.left_output)?;
    save_image(&right_image, &args.right_output)?;

    Ok(())
}

fn div_ceil_u64(numerator: u64, denominator: u64) -> u64 {
    numerator.div_ceil(denominator)
}

fn map_rect_to_gainmap(
    rect: Rect,
    source_width: u32,
    source_height: u32,
    gainmap_width: u32,
    gainmap_height: u32,
) -> Rect {
    let x0 = (rect.x as u64 * gainmap_width as u64 / source_width as u64) as u32;
    let y0 = (rect.y as u64 * gainmap_height as u64 / source_height as u64) as u32;

    let right_edge = rect.x.saturating_add(rect.width);
    let bottom_edge = rect.y.saturating_add(rect.height);

    let mut x1 = div_ceil_u64(
        right_edge as u64 * gainmap_width as u64,
        source_width as u64,
    ) as u32;
    let mut y1 = div_ceil_u64(
        bottom_edge as u64 * gainmap_height as u64,
        source_height as u64,
    ) as u32;

    x1 = x1.clamp(0, gainmap_width);
    y1 = y1.clamp(0, gainmap_height);

    if x1 <= x0 {
        x1 = (x0 + 1).min(gainmap_width);
    }

    if y1 <= y0 {
        y1 = (y0 + 1).min(gainmap_height);
    }

    Rect {
        x: x0,
        y: y0,
        width: x1.saturating_sub(x0),
        height: y1.saturating_sub(y0),
    }
}

fn crop_raw_image(source: &RawImage, rect: Rect) -> Result<RawImage, i32> {
    let channels = match source.format {
        PixelFormat::Rgba8 => 4usize,
        PixelFormat::Rgb8 => 3usize,
        _ => return Err(EXIT_IO_ERROR),
    };

    if rect.width == 0 || rect.height == 0 {
        return Err(EXIT_INVALID_CROP);
    }

    let x_end = rect.x.saturating_add(rect.width);
    let y_end = rect.y.saturating_add(rect.height);
    if x_end > source.width || y_end > source.height {
        return Err(EXIT_INVALID_CROP);
    }

    let source_row_stride = source.stride as usize;
    let min_source_row_bytes = (source.width as usize)
        .checked_mul(channels)
        .ok_or(EXIT_IO_ERROR)?;
    if source_row_stride < min_source_row_bytes {
        return Err(EXIT_IO_ERROR);
    }

    let source_min_len = source_row_stride
        .checked_mul(source.height as usize)
        .ok_or(EXIT_IO_ERROR)?;
    if source.data.len() < source_min_len {
        return Err(EXIT_IO_ERROR);
    }

    let row_copy_len = (rect.width as usize)
        .checked_mul(channels)
        .ok_or(EXIT_IO_ERROR)?;
    let out_len = (rect.width as usize)
        .checked_mul(rect.height as usize)
        .and_then(|v| v.checked_mul(channels))
        .ok_or(EXIT_IO_ERROR)?;
    let mut out = vec![0u8; out_len];

    for row in 0..rect.height as usize {
        let src_y = rect.y as usize + row;
        let src_offset = src_y
            .checked_mul(source_row_stride)
            .and_then(|v| v.checked_add(rect.x as usize * channels))
            .ok_or(EXIT_IO_ERROR)?;
        let src_end = src_offset.checked_add(row_copy_len).ok_or(EXIT_IO_ERROR)?;
        let dst_offset = row.checked_mul(row_copy_len).ok_or(EXIT_IO_ERROR)?;
        let dst_end = dst_offset.checked_add(row_copy_len).ok_or(EXIT_IO_ERROR)?;

        if src_end > source.data.len() || dst_end > out.len() {
            return Err(EXIT_IO_ERROR);
        }

        out[dst_offset..dst_end].copy_from_slice(&source.data[src_offset..src_end]);
    }

    RawImage::from_data(
        rect.width,
        rect.height,
        source.format,
        source.gamut,
        source.transfer,
        out,
    )
    .map_err(|_| EXIT_IO_ERROR)
}

fn crop_gainmap(source: &GainMap, rect: Rect) -> Result<GainMap, i32> {
    let channels = source.channels as usize;
    if channels == 0 || rect.width == 0 || rect.height == 0 {
        return Err(EXIT_INVALID_CROP);
    }

    let x_end = rect.x.saturating_add(rect.width);
    let y_end = rect.y.saturating_add(rect.height);
    if x_end > source.width || y_end > source.height {
        return Err(EXIT_INVALID_CROP);
    }

    let source_row_stride = (source.width as usize)
        .checked_mul(channels)
        .ok_or(EXIT_IO_ERROR)?;
    let source_min_len = source_row_stride
        .checked_mul(source.height as usize)
        .ok_or(EXIT_IO_ERROR)?;
    if source.data.len() < source_min_len {
        return Err(EXIT_IO_ERROR);
    }

    let row_copy_len = (rect.width as usize)
        .checked_mul(channels)
        .ok_or(EXIT_IO_ERROR)?;
    let out_len = (rect.width as usize)
        .checked_mul(rect.height as usize)
        .and_then(|v| v.checked_mul(channels))
        .ok_or(EXIT_IO_ERROR)?;
    let mut out = vec![0u8; out_len];

    for row in 0..rect.height as usize {
        let src_y = rect.y as usize + row;
        let src_offset = src_y
            .checked_mul(source_row_stride)
            .and_then(|v| v.checked_add(rect.x as usize * channels))
            .ok_or(EXIT_IO_ERROR)?;
        let src_end = src_offset.checked_add(row_copy_len).ok_or(EXIT_IO_ERROR)?;
        let dst_offset = row.checked_mul(row_copy_len).ok_or(EXIT_IO_ERROR)?;
        let dst_end = dst_offset.checked_add(row_copy_len).ok_or(EXIT_IO_ERROR)?;

        if src_end > source.data.len() || dst_end > out.len() {
            return Err(EXIT_IO_ERROR);
        }

        out[dst_offset..dst_end].copy_from_slice(&source.data[src_offset..src_end]);
    }

    Ok(GainMap {
        width: rect.width,
        height: rect.height,
        channels: source.channels,
        data: out,
    })
}

fn encode_gainmap_jpeg(
    gainmap: &GainMap,
    metadata: &ultrahdr::GainMapMetadata,
) -> Result<Vec<u8>, i32> {
    let raw_jpeg = match gainmap.channels {
        1 => {
            let config = EncoderConfig::grayscale(GAINMAP_JPEG_QUALITY);
            let mut encoder = config
                .encode_from_bytes(gainmap.width, gainmap.height, PixelLayout::Gray8Srgb)
                .map_err(|_| EXIT_IO_ERROR)?;
            encoder
                .push_packed(&gainmap.data, Unstoppable)
                .map_err(|_| EXIT_IO_ERROR)?;
            encoder.finish().map_err(|_| EXIT_IO_ERROR)?
        }
        3 => {
            let config = EncoderConfig::ycbcr(GAINMAP_JPEG_QUALITY, ChromaSubsampling::None);
            let mut encoder = config
                .encode_from_bytes(gainmap.width, gainmap.height, PixelLayout::Rgb8Srgb)
                .map_err(|_| EXIT_IO_ERROR)?;
            encoder
                .push_packed(&gainmap.data, Unstoppable)
                .map_err(|_| EXIT_IO_ERROR)?;
            encoder.finish().map_err(|_| EXIT_IO_ERROR)?
        }
        _ => return Err(EXIT_IO_ERROR),
    };

    // Embed gain map metadata XMP into the gainmap JPEG (insert APP1 after SOI)
    let xmp = generate_gainmap_xmp(metadata);
    let xmp_marker = ultrahdr::metadata::xmp::create_xmp_app1_marker(&xmp);
    let mut output = Vec::with_capacity(raw_jpeg.len() + xmp_marker.len());
    output.extend_from_slice(&raw_jpeg[..2]); // SOI
    output.extend_from_slice(&xmp_marker);
    output.extend_from_slice(&raw_jpeg[2..]);
    Ok(output)
}

fn luminance_coefficients(gamut: ColorGamut) -> (f32, f32, f32) {
    match gamut {
        ColorGamut::Bt709 => (0.2126, 0.7152, 0.0722),
        ColorGamut::DisplayP3 => (0.2289, 0.6917, 0.0793),
        ColorGamut::Bt2100 => (0.2627, 0.6780, 0.0593),
    }
}

fn decode_gainmap_jpeg(gainmap_jpeg: &[u8], gamut: ColorGamut) -> Result<GainMap, i32> {
    let rgb_decoded = match catch_unwind_quiet(AssertUnwindSafe(|| {
        JpegDecoder::new()
            .preserve(PreserveConfig::none())
            .output_format(JpegPixelFormat::Rgb)
            .decode(gainmap_jpeg)
    })) {
        Ok(Ok(image)) => image,
        _ => {
            let gray_decoded = match catch_unwind_quiet(AssertUnwindSafe(|| {
                JpegDecoder::new()
                    .preserve(PreserveConfig::none())
                    .output_format(JpegPixelFormat::Gray)
                    .decode(gainmap_jpeg)
            })) {
                Ok(Ok(image)) => image,
                _ => return Err(EXIT_IO_ERROR),
            };

            if gray_decoded.width == 0 || gray_decoded.height == 0 {
                return Err(EXIT_IO_ERROR);
            }

            let pixel_count = (gray_decoded.width as usize)
                .checked_mul(gray_decoded.height as usize)
                .ok_or(EXIT_IO_ERROR)?;
            let bytes_per_pixel = gray_decoded.bytes_per_pixel();
            let min_len = pixel_count
                .checked_mul(bytes_per_pixel)
                .ok_or(EXIT_IO_ERROR)?;
            if gray_decoded.data.len() < min_len {
                return Err(EXIT_IO_ERROR);
            }

            let (lr, lg, lb) = luminance_coefficients(gamut);
            let data = match bytes_per_pixel {
                1 => gray_decoded.data,
                3 => gray_decoded
                    .data
                    .chunks_exact(3)
                    .map(|rgb| {
                        let r = rgb[0] as f32;
                        let g = rgb[1] as f32;
                        let b = rgb[2] as f32;
                        (lr * r + lg * g + lb * b).clamp(0.0, 255.0) as u8
                    })
                    .collect(),
                4 => gray_decoded
                    .data
                    .chunks_exact(4)
                    .map(|rgba| {
                        let r = rgba[0] as f32;
                        let g = rgba[1] as f32;
                        let b = rgba[2] as f32;
                        (lr * r + lg * g + lb * b).clamp(0.0, 255.0) as u8
                    })
                    .collect(),
                _ => return Err(EXIT_IO_ERROR),
            };

            return Ok(GainMap {
                width: gray_decoded.width,
                height: gray_decoded.height,
                channels: 1,
                data,
            });
        }
    };

    if rgb_decoded.width == 0 || rgb_decoded.height == 0 {
        return Err(EXIT_IO_ERROR);
    }

    let pixel_count = (rgb_decoded.width as usize)
        .checked_mul(rgb_decoded.height as usize)
        .ok_or(EXIT_IO_ERROR)?;
    let bytes_per_pixel = rgb_decoded.bytes_per_pixel();
    let min_len = pixel_count
        .checked_mul(bytes_per_pixel)
        .ok_or(EXIT_IO_ERROR)?;
    if rgb_decoded.data.len() < min_len {
        return Err(EXIT_IO_ERROR);
    }

    let (channels, data) = match bytes_per_pixel {
        1 => (1u8, rgb_decoded.data),
        3 => (3u8, rgb_decoded.data),
        4 => {
            let data: Vec<u8> = rgb_decoded
                .data
                .chunks_exact(4)
                .flat_map(|rgba| [rgba[0], rgba[1], rgba[2]])
                .collect();
            (3u8, data)
        }
        _ => return Err(EXIT_IO_ERROR),
    };

    Ok(GainMap {
        width: rgb_decoded.width,
        height: rgb_decoded.height,
        channels,
        data,
    })
}

fn metadata_looks_default_or_incomplete(metadata: &ultrahdr::GainMapMetadata) -> bool {
    let max_boost_is_neutral = metadata.max_content_boost.iter().all(|v| *v <= 1.001);
    let hdr_capacity_is_neutral = metadata.hdr_capacity_max <= 1.001;
    let invalid_gamma = metadata.gamma.iter().any(|v| !v.is_finite() || *v <= 0.0);
    let invalid_min = metadata
        .min_content_boost
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0);
    let invalid_max = metadata
        .max_content_boost
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0);
    (max_boost_is_neutral && hdr_capacity_is_neutral) || invalid_gamma || invalid_min || invalid_max
}

fn extract_metadata_from_gainmap_xmp(
    gainmap_jpeg: &[u8],
    debug: bool,
) -> Option<ultrahdr::GainMapMetadata> {
    debug_log(debug, "HDR probe: trying metadata from gainmap XMP");
    let decoded = match catch_unwind_quiet(AssertUnwindSafe(|| {
        JpegDecoder::new()
            .preserve(PreserveConfig::default())
            .output_format(JpegPixelFormat::Gray)
            .decode(gainmap_jpeg)
    })) {
        Ok(Ok(image)) => image,
        _ => match catch_unwind_quiet(AssertUnwindSafe(|| {
            JpegDecoder::new()
                .preserve(PreserveConfig::default())
                .output_format(JpegPixelFormat::Rgb)
                .decode(gainmap_jpeg)
        })) {
            Ok(Ok(image)) => image,
            Ok(Err(_)) => {
                debug_log(debug, "HDR probe: gainmap XMP decode returned error");
                return None;
            }
            Err(_) => {
                debug_log(debug, "HDR probe: gainmap XMP decode panicked");
                return None;
            }
        },
    };

    let extras = match decoded.extras() {
        Some(extras) => extras,
        None => {
            debug_log(debug, "HDR probe: gainmap XMP missing extras");
            return None;
        }
    };
    let xmp = match extras.xmp() {
        Some(xmp) => xmp,
        None => {
            debug_log(debug, "HDR probe: gainmap XMP missing");
            return None;
        }
    };
    let (mut metadata, _) = match ultrahdr::metadata::xmp::parse_xmp(xmp) {
        Ok(parsed) => parsed,
        Err(_) => {
            debug_log(debug, "HDR probe: gainmap XMP parse failed");
            return None;
        }
    };
    apply_lenient_xmp_overrides(xmp, &mut metadata);

    if metadata_looks_default_or_incomplete(&metadata) {
        debug_log(
            debug,
            "HDR probe: gainmap XMP metadata still looks default/incomplete",
        );
        return None;
    }

    debug_log(debug, "HDR probe: using metadata from gainmap XMP");
    debug_log_metadata(debug, "HDR probe: gainmap XMP metadata", &metadata);
    Some(metadata)
}

fn is_valid_jpeg_range(bytes: &[u8], start: usize, end: usize) -> bool {
    if start >= end || end > bytes.len() || end - start < 4 {
        return false;
    }

    bytes[start] == 0xFF
        && bytes[start + 1] == 0xD8
        && bytes[end - 2] == 0xFF
        && bytes[end - 1] == 0xD9
}

fn extract_gainmap_jpeg_from_mpf(source_bytes: &[u8], debug: bool) -> Option<Vec<u8>> {
    debug_log(debug, "HDR probe: MPF extraction start");
    let images = match ultrahdr::metadata::mpf::parse_mpf(source_bytes) {
        Ok(images) => images,
        Err(_) => {
            debug_log(debug, "HDR probe: MPF parse failed");
            return None;
        }
    };

    if images.len() < 2 {
        debug_log(debug, "HDR probe: MPF does not contain secondary image");
        return None;
    }

    let (offset, length) = images[1];
    let base_end = match offset.checked_add(length) {
        Some(end) => end,
        None => {
            debug_log(debug, "HDR probe: MPF gainmap offset+length overflow");
            return None;
        }
    };

    for delta in [0isize, 8isize, -8isize] {
        let start = if delta >= 0 {
            offset.checked_add(delta as usize)
        } else {
            offset.checked_sub((-delta) as usize)
        };
        let end = if delta >= 0 {
            base_end.checked_add(delta as usize)
        } else {
            base_end.checked_sub((-delta) as usize)
        };

        if let (Some(start), Some(end)) = (start, end)
            && is_valid_jpeg_range(source_bytes, start, end)
        {
            debug_log(
                debug,
                &format!(
                    "HDR probe: MPF gainmap slice validated start={} end={} (delta={})",
                    start, end, delta
                ),
            );
            return Some(source_bytes[start..end].to_vec());
        }
    }

    debug_log(
        debug,
        &format!(
            "HDR probe: MPF range invalid offset={} length={}, trying marker scan fallback",
            offset, length
        ),
    );

    let boundaries = ultrahdr::metadata::mpf::find_jpeg_boundaries(source_bytes);
    if boundaries.len() >= 2 {
        let (start, end) = boundaries[1];
        if is_valid_jpeg_range(source_bytes, start, end) {
            debug_log(
                debug,
                &format!(
                    "HDR probe: marker scan gainmap slice start={} end={}",
                    start, end
                ),
            );
            return Some(source_bytes[start..end].to_vec());
        }
    }

    debug_log(debug, "HDR probe: could not find valid gainmap JPEG slice");
    None
}

fn try_extract_ultrahdr_with_jpegli(
    source_bytes: &[u8],
    debug: bool,
) -> Option<(
    ultrahdr::GainMapMetadata,
    RawImage,
    GainMap,
    Option<Vec<u8>>,
)> {
    debug_log(debug, "HDR probe: jpegli path start");
    let decoded = match catch_unwind_quiet(AssertUnwindSafe(|| {
        JpegDecoder::new()
            .preserve(PreserveConfig::default())
            .output_format(JpegPixelFormat::Rgb)
            .decode(source_bytes)
    })) {
        Ok(Ok(image)) => {
            debug_log(debug, "HDR probe: jpegli decode success");
            image
        }
        Ok(Err(_)) => {
            debug_log(debug, "HDR probe: jpegli decode returned error");
            return None;
        }
        Err(_) => {
            debug_log(debug, "HDR probe: jpegli decode panicked");
            return None;
        }
    };

    let (metadata, gainmap_jpeg, icc_profile, detected_gamut): (
        ultrahdr::GainMapMetadata,
        Vec<u8>,
        Option<Vec<u8>>,
        ColorGamut,
    ) = {
        let extras = match decoded.extras() {
            Some(extras) => extras,
            None => {
                debug_log(debug, "HDR probe: no preserved extras");
                return None;
            }
        };
        let xmp = match extras.xmp() {
            Some(xmp) => xmp,
            None => {
                debug_log(debug, "HDR probe: no XMP in extras");
                return None;
            }
        };
        let (metadata, _) = match ultrahdr::metadata::xmp::parse_xmp(xmp) {
            Ok(parsed) => parsed,
            Err(_) => {
                debug_log(debug, "HDR probe: XMP parse failed");
                return None;
            }
        };
        let gainmap = match extras.gainmap() {
            Some(gainmap) => {
                debug_log(debug, "HDR probe: gainmap found in extras");
                gainmap.to_vec()
            }
            None => {
                debug_log(debug, "HDR probe: no gainmap in extras, trying MPF");
                match extract_gainmap_jpeg_from_mpf(source_bytes, debug) {
                    Some(gainmap_jpeg) => gainmap_jpeg,
                    None => {
                        debug_log(debug, "HDR probe: no gainmap in extras or MPF");
                        return None;
                    }
                }
            }
        };
        let metadata = if metadata_looks_default_or_incomplete(&metadata) {
            debug_log(
                debug,
                "HDR probe: primary XMP metadata looks incomplete, trying gainmap XMP",
            );
            extract_metadata_from_gainmap_xmp(&gainmap, debug).unwrap_or(metadata)
        } else {
            metadata
        };
        debug_log_metadata(debug, "HDR probe: selected metadata", &metadata);

        let detected_gamut = gamut_from_standard_profile(extras.icc_is_standard());
        let icc_profile = extras.icc_profile().map(|icc| icc.to_vec());
        (metadata, gainmap, icc_profile, detected_gamut)
    };
    debug_log(
        debug,
        &format!("HDR probe: jpegli found XMP + gainmap, gamut={detected_gamut:?}"),
    );

    let sdr = RawImage::from_data(
        decoded.width,
        decoded.height,
        PixelFormat::Rgb8,
        detected_gamut,
        ColorTransfer::Srgb,
        decoded.data,
    )
    .ok()?;
    let gainmap = match decode_gainmap_jpeg(&gainmap_jpeg, detected_gamut) {
        Ok(gainmap) => gainmap,
        Err(_) => {
            debug_log(debug, "HDR probe: gainmap JPEG decode failed");
            return None;
        }
    };
    debug_log(debug, "HDR probe: jpegli path success");

    Some((metadata, sdr, gainmap, icc_profile))
}

fn try_extract_ultrahdr_with_ultrahdr_decoder(
    source_bytes: &[u8],
    debug: bool,
) -> Option<(
    ultrahdr::GainMapMetadata,
    RawImage,
    GainMap,
    Option<Vec<u8>>,
)> {
    debug_log(debug, "HDR probe: ultrahdr-rs path start");
    let decoder = match catch_unwind_quiet(AssertUnwindSafe(|| UltraHdrDecoder::new(source_bytes)))
    {
        Ok(Ok(decoder)) => decoder,
        Ok(Err(_)) => {
            debug_log(debug, "HDR probe: ultrahdr-rs decoder init error");
            return None;
        }
        Err(_) => {
            debug_log(debug, "HDR probe: ultrahdr-rs decoder init panicked");
            return None;
        }
    };

    let is_ultrahdr = match catch_unwind_quiet(AssertUnwindSafe(|| decoder.is_ultrahdr())) {
        Ok(value) => value,
        Err(_) => {
            debug_log(debug, "HDR probe: ultrahdr-rs is_ultrahdr panicked");
            return None;
        }
    };
    if !is_ultrahdr {
        debug_log(debug, "HDR probe: ultrahdr-rs says not Ultra HDR");
        return None;
    }

    let metadata = match catch_unwind_quiet(AssertUnwindSafe(|| decoder.metadata().cloned())) {
        Ok(Some(metadata)) => metadata,
        _ => {
            debug_log(debug, "HDR probe: ultrahdr-rs missing metadata");
            return None;
        }
    };
    let sdr = match catch_unwind_quiet(AssertUnwindSafe(|| decoder.decode_sdr())) {
        Ok(Ok(sdr)) => sdr,
        _ => {
            debug_log(debug, "HDR probe: ultrahdr-rs decode_sdr failed/panicked");
            return None;
        }
    };
    let gainmap = match catch_unwind_quiet(AssertUnwindSafe(|| decoder.decode_gainmap())) {
        Ok(Ok(gainmap)) => gainmap,
        _ => {
            debug_log(
                debug,
                "HDR probe: ultrahdr-rs decode_gainmap failed/panicked",
            );
            return None;
        }
    };
    let icc_profile = catch_unwind_quiet(AssertUnwindSafe(|| decoder.icc_profile()))
        .ok()
        .flatten();

    // The ultrahdr-rs decoder defaults SDR to Bt709; detect actual gamut from ICC.
    let detected_gamut = match &icc_profile {
        Some(icc) => gamut_from_icc_bytes(icc),
        None => ColorGamut::Bt709,
    };
    let sdr = if sdr.gamut != detected_gamut {
        debug_log(
            debug,
            &format!("HDR probe: overriding SDR gamut to {detected_gamut:?}"),
        );
        RawImage::from_data(
            sdr.width,
            sdr.height,
            sdr.format,
            detected_gamut,
            sdr.transfer,
            sdr.data,
        )
        .ok()?
    } else {
        sdr
    };

    debug_log(debug, "HDR probe: ultrahdr-rs path success");

    Some((metadata, sdr, gainmap, icc_profile))
}

fn split_ultrahdr_tiles(
    args: &SplitParams,
    metadata: ultrahdr::GainMapMetadata,
    sdr: RawImage,
    gainmap: GainMap,
    source_icc_profile: Option<&[u8]>,
) -> Result<(), i32> {
    debug_log(
        args.debug,
        &format!(
            "HDR split: sdr={}x{}, gainmap={}x{}",
            sdr.width, sdr.height, gainmap.width, gainmap.height
        ),
    );
    if sdr.width == 0 || sdr.height == 0 || gainmap.width == 0 || gainmap.height == 0 {
        return Err(EXIT_INVALID_INPUT);
    }

    let (left_rect, right_rect) = compute_split_rectangles(sdr.width, sdr.height)?;
    debug_log(
        args.debug,
        &format!(
            "HDR split: left rect {}x{}+{},{} right rect {}x{}+{},{}",
            left_rect.width,
            left_rect.height,
            left_rect.x,
            left_rect.y,
            right_rect.width,
            right_rect.height,
            right_rect.x,
            right_rect.y
        ),
    );

    let left_sdr = crop_raw_image(&sdr, left_rect)?;
    let right_sdr = crop_raw_image(&sdr, right_rect)?;

    let left_gainmap_rect = map_rect_to_gainmap(
        left_rect,
        sdr.width,
        sdr.height,
        gainmap.width,
        gainmap.height,
    );
    let right_gainmap_rect = map_rect_to_gainmap(
        right_rect,
        sdr.width,
        sdr.height,
        gainmap.width,
        gainmap.height,
    );

    let left_gainmap = crop_gainmap(&gainmap, left_gainmap_rect)?;
    let right_gainmap = crop_gainmap(&gainmap, right_gainmap_rect)?;

    let left_bytes = encode_ultrahdr_tile(left_sdr, left_gainmap, &metadata, source_icc_profile)?;
    let right_bytes =
        encode_ultrahdr_tile(right_sdr, right_gainmap, &metadata, source_icc_profile)?;

    fs::write(&args.left_output, left_bytes).map_err(|_| EXIT_IO_ERROR)?;
    fs::write(&args.right_output, right_bytes).map_err(|_| EXIT_IO_ERROR)?;
    debug_log(
        args.debug,
        &format!(
            "HDR split: wrote '{}' and '{}'",
            args.left_output, args.right_output
        ),
    );

    Ok(())
}

/// Format a 3-element value for XMP: log2 if `use_log2`, then either scalar or `rdf:Seq`.
fn format_xmp_value(values: &[f32; 3], is_single: bool, use_log2: bool) -> String {
    let v: [f32; 3] = if use_log2 {
        [values[0].log2(), values[1].log2(), values[2].log2()]
    } else {
        *values
    };
    if is_single {
        format!("{:.6}", v[0])
    } else {
        format!("{:.6}, {:.6}, {:.6}", v[0], v[1], v[2])
    }
}

/// Format a 3-element value as an rdf:Seq block for XMP per-channel properties.
fn format_xmp_seq(tag: &str, values: &[f32; 3], is_single: bool, use_log2: bool) -> String {
    let v: [f32; 3] = if use_log2 {
        [values[0].log2(), values[1].log2(), values[2].log2()]
    } else {
        *values
    };
    if is_single {
        // Single channel: use attribute form
        format!("        hdrgm:{tag}=\"{:.6}\"", v[0])
    } else {
        // Per-channel: use rdf:Seq
        format!(
            "        <hdrgm:{tag}>\n          \
             <rdf:Seq>\n            \
             <rdf:li>{:.6}</rdf:li>\n            \
             <rdf:li>{:.6}</rdf:li>\n            \
             <rdf:li>{:.6}</rdf:li>\n          \
             </rdf:Seq>\n        \
             </hdrgm:{tag}>",
            v[0], v[1], v[2]
        )
    }
}

/// Generate XMP for the primary JPEG with Container directory and proper rdf:Seq format.
fn generate_primary_xmp(metadata: &ultrahdr::GainMapMetadata, gainmap_length: usize) -> String {
    let is_single = metadata.is_single_channel();
    let hdr_capacity_min = metadata.hdr_capacity_min.log2();
    let hdr_capacity_max = metadata.hdr_capacity_max.log2();

    let gain_map_min = format_xmp_seq("GainMapMin", &metadata.min_content_boost, is_single, true);
    let gain_map_max = format_xmp_seq("GainMapMax", &metadata.max_content_boost, is_single, true);
    let gamma = format_xmp_seq("Gamma", &metadata.gamma, is_single, false);
    let offset_sdr = format_xmp_value(&metadata.offset_sdr, is_single, false);
    let offset_hdr = format_xmp_value(&metadata.offset_hdr, is_single, false);

    format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/"
        xmlns:Container="http://ns.google.com/photos/1.0/container/"
        xmlns:Item="http://ns.google.com/photos/1.0/container/item/"
        hdrgm:Version="1.0"
        hdrgm:OffsetSDR="{offset_sdr}"
        hdrgm:OffsetHDR="{offset_hdr}"
        hdrgm:HDRCapacityMin="{hdr_capacity_min:.6}"
        hdrgm:HDRCapacityMax="{hdr_capacity_max:.6}"
        hdrgm:BaseRenditionIsHDR="False">
{gain_map_min}
{gain_map_max}
{gamma}
      <Container:Directory>
        <rdf:Seq>
          <rdf:li rdf:parseType="Resource">
            <Container:Item
                Item:Semantic="Primary"
                Item:Mime="image/jpeg"/>
          </rdf:li>
          <rdf:li rdf:parseType="Resource">
            <Container:Item
                Item:Semantic="GainMap"
                Item:Mime="image/jpeg"
                Item:Length="{gainmap_length}"/>
          </rdf:li>
        </rdf:Seq>
      </Container:Directory>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#
    )
}

/// Generate XMP for the gainmap JPEG (no Container directory).
fn generate_gainmap_xmp(metadata: &ultrahdr::GainMapMetadata) -> String {
    let is_single = metadata.is_single_channel();
    let hdr_capacity_min = metadata.hdr_capacity_min.log2();
    let hdr_capacity_max = metadata.hdr_capacity_max.log2();

    let gain_map_min = format_xmp_seq("GainMapMin", &metadata.min_content_boost, is_single, true);
    let gain_map_max = format_xmp_seq("GainMapMax", &metadata.max_content_boost, is_single, true);
    let gamma = format_xmp_seq("Gamma", &metadata.gamma, is_single, false);
    let offset_sdr = format_xmp_value(&metadata.offset_sdr, is_single, false);
    let offset_hdr = format_xmp_value(&metadata.offset_hdr, is_single, false);

    format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/"
        hdrgm:Version="1.0"
        hdrgm:BaseRenditionIsHDR="False"
        hdrgm:HDRCapacityMin="{hdr_capacity_min:.6}"
        hdrgm:HDRCapacityMax="{hdr_capacity_max:.6}"
        hdrgm:OffsetSDR="{offset_sdr}"
        hdrgm:OffsetHDR="{offset_hdr}">
{gain_map_min}
{gain_map_max}
{gamma}
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#
    )
}

/// Find the position after SOI and APP0/APP1/APP2 markers where MPF APP2 should be inserted.
fn find_mpf_insert_position(data: &[u8]) -> Result<usize, i32> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err(EXIT_IO_ERROR);
    }
    let mut pos = 2;
    while pos + 3 < data.len() {
        if data[pos] != 0xFF {
            break;
        }
        let marker = data[pos + 1];
        // Stop before non-APP markers; MPF goes after all existing APP markers
        if !(0xE0..=0xEF).contains(&marker) {
            break;
        }
        let length = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        pos += 2 + length;
    }
    Ok(pos)
}

/// Create an MPF APP2 marker for a two-image Ultra HDR JPEG.
///
/// `primary_size` is the total byte size of the final primary JPEG (including the MPF marker).
/// `gainmap_size` is the byte size of the gainmap JPEG.
/// `mpf_marker_offset` is the byte offset of the MPF marker within the final primary JPEG.
///
/// Per the MPF spec, secondary image offsets are relative to the start of the MPF's
/// TIFF header, which is 8 bytes after the MPF marker start (FF E2 + length(2) + "MPF\0"(4)).
fn create_mpf_app2(
    primary_size: u32,
    gainmap_size: u32,
    mpf_marker_offset: usize,
) -> Vec<u8> {
    let num_images: u32 = 2;
    let mut mpf_data = Vec::with_capacity(128);

    // TIFF header: big-endian
    mpf_data.extend_from_slice(b"MM");
    mpf_data.extend_from_slice(&0x002Au16.to_be_bytes());
    mpf_data.extend_from_slice(&8u32.to_be_bytes()); // IFD offset

    // IFD: 3 entries
    mpf_data.extend_from_slice(&3u16.to_be_bytes());

    // Entry 1: Version (0xB000), UNDEFINED(7), count=4, value="0100"
    mpf_data.extend_from_slice(&0xB000u16.to_be_bytes());
    mpf_data.extend_from_slice(&7u16.to_be_bytes());
    mpf_data.extend_from_slice(&4u32.to_be_bytes());
    mpf_data.extend_from_slice(b"0100");

    // Entry 2: NumberOfImages (0xB001), LONG(4), count=1
    mpf_data.extend_from_slice(&0xB001u16.to_be_bytes());
    mpf_data.extend_from_slice(&4u16.to_be_bytes());
    mpf_data.extend_from_slice(&1u32.to_be_bytes());
    mpf_data.extend_from_slice(&num_images.to_be_bytes());

    // Entry 3: MPEntry (0xB002), UNDEFINED(7), count=num_images*16
    let mp_entry_size = num_images * 16;
    // MP entry data offset: 8 (TIFF hdr) + 2 (num_entries) + 36 (3*12 IFD entries) + 4 (next IFD)
    let mp_entry_offset: u32 = 8 + 2 + 36 + 4;
    mpf_data.extend_from_slice(&0xB002u16.to_be_bytes());
    mpf_data.extend_from_slice(&7u16.to_be_bytes());
    mpf_data.extend_from_slice(&mp_entry_size.to_be_bytes());
    mpf_data.extend_from_slice(&mp_entry_offset.to_be_bytes());

    // Next IFD offset: 0 (none)
    mpf_data.extend_from_slice(&0u32.to_be_bytes());

    // MP Entry for primary image (attr=0x030000, size=primary_size, offset=0)
    mpf_data.extend_from_slice(&0x0003_0000u32.to_be_bytes());
    mpf_data.extend_from_slice(&primary_size.to_be_bytes());
    mpf_data.extend_from_slice(&0u32.to_be_bytes());
    mpf_data.extend_from_slice(&0u32.to_be_bytes());

    // MP Entry for gainmap image
    // Offset is relative to the TIFF header, which is at mpf_marker_offset + 8
    let tiff_header_offset = mpf_marker_offset as u32 + 8;
    let relative_offset = primary_size - tiff_header_offset;
    mpf_data.extend_from_slice(&0u32.to_be_bytes()); // attribute: dependent
    mpf_data.extend_from_slice(&gainmap_size.to_be_bytes());
    mpf_data.extend_from_slice(&relative_offset.to_be_bytes());
    mpf_data.extend_from_slice(&0u32.to_be_bytes());

    // Wrap in APP2 marker
    let payload_len = 2 + 4 + mpf_data.len(); // length field + "MPF\0" + data
    let mut marker = Vec::with_capacity(2 + payload_len);
    marker.push(0xFF);
    marker.push(0xE2);
    marker.push(((payload_len >> 8) & 0xFF) as u8);
    marker.push((payload_len & 0xFF) as u8);
    marker.extend_from_slice(b"MPF\0");
    marker.extend_from_slice(&mpf_data);
    marker
}

/// Assemble a complete Ultra HDR JPEG from SDR primary + gainmap + XMP metadata + MPF.
fn assemble_ultrahdr_jpeg(
    sdr_jpeg: &[u8],
    gainmap_jpeg: &[u8],
    metadata: &ultrahdr::GainMapMetadata,
) -> Result<Vec<u8>, i32> {
    let xmp = generate_primary_xmp(metadata, gainmap_jpeg.len());
    let xmp_marker = ultrahdr::metadata::xmp::create_xmp_app1_marker(&xmp);

    // Insert XMP APP1 after SOI
    let mut primary_with_xmp = Vec::with_capacity(sdr_jpeg.len() + xmp_marker.len());
    primary_with_xmp.extend_from_slice(&sdr_jpeg[..2]); // SOI
    primary_with_xmp.extend_from_slice(&xmp_marker);
    primary_with_xmp.extend_from_slice(&sdr_jpeg[2..]);

    // Find where to insert MPF APP2 (after all existing APP markers)
    let insert_pos = find_mpf_insert_position(&primary_with_xmp)?;

    // Create a placeholder MPF to determine its size (use u32::MAX to avoid underflow)
    let placeholder_mpf = create_mpf_app2(u32::MAX, gainmap_jpeg.len() as u32, insert_pos);
    let mpf_size = placeholder_mpf.len();

    // Final primary size includes the MPF marker
    let primary_final_size = (primary_with_xmp.len() + mpf_size) as u32;
    let mpf_marker = create_mpf_app2(
        primary_final_size,
        gainmap_jpeg.len() as u32,
        insert_pos,
    );

    // Assemble: primary[..insert] + MPF + primary[insert..] + gainmap
    let total = primary_with_xmp.len() + mpf_marker.len() + gainmap_jpeg.len();
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(&primary_with_xmp[..insert_pos]);
    output.extend_from_slice(&mpf_marker);
    output.extend_from_slice(&primary_with_xmp[insert_pos..]);
    output.extend_from_slice(gainmap_jpeg);

    Ok(output)
}

fn encode_ultrahdr_tile(
    sdr_tile: RawImage,
    gainmap_tile: GainMap,
    metadata: &ultrahdr::GainMapMetadata,
    source_icc_profile: Option<&[u8]>,
) -> Result<Vec<u8>, i32> {
    let gainmap_jpeg = encode_gainmap_jpeg(&gainmap_tile, metadata)?;

    // Encode SDR tile (without gainmap — we assemble the container ourselves for correct MPF offsets)
    let mut config = EncoderConfig::ycbcr(SDR_TILE_JPEG_QUALITY, ChromaSubsampling::None);
    if let Some(icc_profile) = source_icc_profile
        && !icc_profile.is_empty()
    {
        config = config.icc_profile(icc_profile.to_vec());
    }

    let (pixel_layout, pixel_data): (PixelLayout, std::borrow::Cow<[u8]>) = match sdr_tile.format {
        PixelFormat::Rgb8 => (
            PixelLayout::Rgb8Srgb,
            std::borrow::Cow::Borrowed(&sdr_tile.data),
        ),
        PixelFormat::Rgba8 => {
            let rgb: Vec<u8> = sdr_tile
                .data
                .chunks_exact(4)
                .flat_map(|rgba| [rgba[0], rgba[1], rgba[2]])
                .collect();
            (PixelLayout::Rgb8Srgb, std::borrow::Cow::Owned(rgb))
        }
        _ => return Err(EXIT_IO_ERROR),
    };

    let mut encoder = config
        .encode_from_bytes(sdr_tile.width, sdr_tile.height, pixel_layout)
        .map_err(|_| EXIT_IO_ERROR)?;
    encoder
        .push_packed(&pixel_data, Unstoppable)
        .map_err(|_| EXIT_IO_ERROR)?;
    let sdr_jpeg = encoder.finish().map_err(|_| EXIT_IO_ERROR)?;

    assemble_ultrahdr_jpeg(&sdr_jpeg, &gainmap_jpeg, metadata)
}

fn try_split_ultrahdr_jpeg(args: &SplitParams) -> Result<UltraHdrSplitOutcome, i32> {
    debug_log(args.debug, "HDR split: enter");
    if !is_jpeg_path(&args.input)
        || !is_jpeg_path(&args.left_output)
        || !is_jpeg_path(&args.right_output)
    {
        debug_log(args.debug, "HDR split: skipped (non-jpeg path)");
        return Ok(UltraHdrSplitOutcome::NotUltraHdr);
    }

    let source_bytes = fs::read(&args.input).map_err(|_| EXIT_IO_ERROR)?;
    debug_log(
        args.debug,
        &format!("HDR split: loaded source bytes ({})", source_bytes.len()),
    );

    if let Some((metadata, sdr, gainmap, source_icc_profile)) =
        try_extract_ultrahdr_with_jpegli(&source_bytes, args.debug)
    {
        debug_log(args.debug, "HDR split: using jpegli extraction path");
        split_ultrahdr_tiles(args, metadata, sdr, gainmap, source_icc_profile.as_deref())?;
        return Ok(UltraHdrSplitOutcome::Handled);
    }

    if let Some((metadata, sdr, gainmap, source_icc_profile)) =
        try_extract_ultrahdr_with_ultrahdr_decoder(&source_bytes, args.debug)
    {
        debug_log(args.debug, "HDR split: using ultrahdr-rs fallback path");
        split_ultrahdr_tiles(args, metadata, sdr, gainmap, source_icc_profile.as_deref())?;
        return Ok(UltraHdrSplitOutcome::Handled);
    }

    debug_log(args.debug, "HDR split: not detected as Ultra HDR");
    Ok(UltraHdrSplitOutcome::NotUltraHdr)
}

pub fn run(args: SplitParams) -> Result<(), i32> {
    debug_log(
        args.debug,
        &format!(
            "Run: input='{}' left='{}' right='{}'",
            args.input, args.left_output, args.right_output
        ),
    );
    match try_split_ultrahdr_jpeg(&args) {
        Ok(UltraHdrSplitOutcome::Handled) => {
            debug_log(args.debug, "Run: completed in HDR path");
            Ok(())
        }
        Ok(UltraHdrSplitOutcome::NotUltraHdr) => {
            debug_log(
                args.debug,
                "Run: fallback to standard split (not Ultra HDR)",
            );
            if is_jpeg_path(&args.input) {
                eprintln!(
                    "Warning: input is not Ultra HDR; output will be SDR (no HDR gain map)"
                );
            }
            split_standard_image(&args)
        }
        Err(EXIT_IO_ERROR) => {
            eprintln!(
                "Warning: HDR extraction failed; falling back to SDR output (brightness may be reduced)"
            );
            split_standard_image(&args)
        }
        Err(code) => {
            debug_log(args.debug, &format!("Run: failed with exit code {code}"));
            Err(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn default_output_paths_use_hyphenated_jpg_names() {
        let (left, right) = default_output_paths("photo.heic");
        assert_eq!(left, "photo-left.jpg");
        assert_eq!(right, "photo-right.jpg");
    }

    #[test]
    fn crop_gainmap_rejects_inconsistent_buffer_lengths() {
        let gainmap = GainMap {
            width: 100,
            height: 100,
            channels: 1,
            data: vec![0u8; 10],
        };
        let rect = Rect {
            x: 0,
            y: 0,
            width: 50,
            height: 50,
        };

        assert!(matches!(crop_gainmap(&gainmap, rect), Err(EXIT_IO_ERROR)));
    }

    fn unique_test_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("tilesplit-test-{}", nanos))
    }

    fn encode_rgb_jpeg(width: u32, height: u32, rgb: &[u8], quality: f32) -> Vec<u8> {
        let config = EncoderConfig::ycbcr(quality, jpegli::encoder::ChromaSubsampling::Quarter);
        let mut encoder = config
            .encode_from_bytes(width, height, PixelLayout::Rgb8Srgb)
            .expect("encoder");
        encoder.push_packed(rgb, Unstoppable).expect("push rgb");
        encoder.finish().expect("finish rgb")
    }

    fn build_test_ultrahdr(width: u32, height: u32) -> Vec<u8> {
        let mut rgb = vec![0u8; (width * height * 3) as usize];
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 3) as usize;
                rgb[idx] = ((x * 255) / width.max(1)) as u8;
                rgb[idx + 1] = ((y * 255) / height.max(1)) as u8;
                rgb[idx + 2] = 192;
            }
        }

        let sdr_jpeg = encode_rgb_jpeg(width, height, &rgb, 90.0);
        let metadata = ultrahdr::GainMapMetadata {
            max_content_boost: [4.0, 4.0, 4.0],
            min_content_boost: [1.0, 1.0, 1.0],
            gamma: [1.0, 1.0, 1.0],
            offset_sdr: [1.0 / 64.0, 1.0 / 64.0, 1.0 / 64.0],
            offset_hdr: [1.0 / 64.0, 1.0 / 64.0, 1.0 / 64.0],
            hdr_capacity_min: 1.0,
            hdr_capacity_max: 4.0,
            use_base_color_space: false,
        };

        let gainmap_width = (width / 4).max(1);
        let gainmap_height = (height / 4).max(1);
        let gainmap_data = vec![128u8; (gainmap_width * gainmap_height) as usize];
        let gainmap = GainMap {
            width: gainmap_width,
            height: gainmap_height,
            channels: 1,
            data: gainmap_data,
        };
        let gainmap_jpeg = encode_gainmap_jpeg(&gainmap, &metadata).expect("gainmap jpeg");

        let mut encoder = UltraHdrEncoder::new();
        encoder
            .set_compressed_sdr(sdr_jpeg)
            .set_existing_gainmap_jpeg(gainmap_jpeg, metadata);

        encoder.encode().expect("ultrahdr encode")
    }

    #[test]
    fn splits_ultrahdr_jpeg_and_keeps_ultrahdr_outputs() {
        let test_dir = unique_test_dir();
        fs::create_dir_all(&test_dir).expect("create test dir");

        let input_path = test_dir.join("input.jpg");
        let left_path = test_dir.join("left.jpg");
        let right_path = test_dir.join("right.jpg");

        let ultrahdr_bytes = build_test_ultrahdr(1500, 1000);
        fs::write(&input_path, ultrahdr_bytes).expect("write input");

        let args = SplitParams {
            input: input_path.to_string_lossy().into_owned(),
            left_output: left_path.to_string_lossy().into_owned(),
            right_output: right_path.to_string_lossy().into_owned(),
            debug: false,
        };
        run(args).expect("split run");

        let left_bytes = fs::read(&left_path).expect("read left");
        let right_bytes = fs::read(&right_path).expect("read right");

        let left_decoder = UltraHdrDecoder::new(&left_bytes).expect("left decoder");
        let right_decoder = UltraHdrDecoder::new(&right_bytes).expect("right decoder");
        assert!(left_decoder.is_ultrahdr());
        assert!(right_decoder.is_ultrahdr());

        let left_sdr = left_decoder.decode_sdr().expect("left sdr");
        let right_sdr = right_decoder.decode_sdr().expect("right sdr");
        assert_eq!(left_sdr.width, 750);
        assert_eq!(left_sdr.height, 938);
        assert_eq!(right_sdr.width, 750);
        assert_eq!(right_sdr.height, 938);

        let _ = fs::remove_file(left_path);
        let _ = fs::remove_file(right_path);
        let _ = fs::remove_file(input_path);
        let _ = fs::remove_dir(test_dir);
    }
}

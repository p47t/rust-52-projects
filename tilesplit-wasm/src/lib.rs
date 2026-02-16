use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageEncoder};
use serde::Serialize;
use ultrahdr_core::GainMapMetadata;
use wasm_bindgen::prelude::*;

// ---- Constants ----

const ASPECT_16_10: f64 = 16.0 / 10.0;
const ASPECT_3_2: f64 = 3.0 / 2.0;
const ASPECT_TOLERANCE: f64 = 0.01;

// ---- Geometry (ported from tilesplit) ----

#[derive(Clone, Copy)]
struct Rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn is_close(a: f64, b: f64) -> bool {
    (a - b).abs() <= ASPECT_TOLERANCE
}

fn compute_split_rectangles(width: u32, height: u32) -> Result<(Rect, Rect), String> {
    if height == 0 {
        return Err("Invalid image: zero height".into());
    }

    let actual_aspect = width as f64 / height as f64;
    if !is_close(actual_aspect, ASPECT_16_10) && !is_close(actual_aspect, ASPECT_3_2) {
        return Err(format!(
            "Unsupported aspect ratio {:.2}:1. Expected 16:10 ({:.2}:1) or 3:2 ({:.2}:1)",
            actual_aspect, ASPECT_16_10, ASPECT_3_2
        ));
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
        return Err("Invalid crop dimensions".into());
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

    let mut x1 = (right_edge as u64 * gainmap_width as u64).div_ceil(source_width as u64) as u32;
    let mut y1 = (bottom_edge as u64 * gainmap_height as u64).div_ceil(source_height as u64) as u32;

    x1 = x1.min(gainmap_width);
    y1 = y1.min(gainmap_height);

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

// ---- XMP Helpers (ported from tilesplit) ----

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
        let li_end_rel = match rest[li_content_start..].find("</rdf:li>") {
            Some(pos) => pos,
            None => break,
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

fn apply_lenient_xmp_overrides(xmp: &str, metadata: &mut GainMapMetadata) {
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

    if let Some(val) = extract_xmp_attribute_value(xmp, "hdrgm:HDRCapacityMin") {
        if let Ok(v) = val.trim().parse::<f32>() {
            metadata.hdr_capacity_min = 2.0f32.powf(v);
        }
    }

    if let Some(val) = extract_xmp_attribute_value(xmp, "hdrgm:HDRCapacityMax") {
        if let Ok(v) = val.trim().parse::<f32>() {
            metadata.hdr_capacity_max = 2.0f32.powf(v);
        }
    }
}

fn metadata_looks_default_or_incomplete(metadata: &GainMapMetadata) -> bool {
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

// ---- JPEG Marker Scanning ----

const XMP_NAMESPACE: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

fn extract_xmp_from_jpeg_bytes(data: &[u8]) -> Option<String> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return None;
    }

    let mut pos = 2;
    while pos + 4 <= data.len() {
        if data[pos] != 0xFF {
            break;
        }
        let marker = data[pos + 1];
        if marker == 0xDA || marker == 0xD9 {
            break;
        }
        if marker == 0x00 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            pos += 2;
            continue;
        }
        if pos + 4 > data.len() {
            break;
        }
        let length = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if length < 2 || pos + 2 + length > data.len() {
            break;
        }

        if marker == 0xE1 {
            let segment = &data[pos + 4..pos + 2 + length];
            if segment.len() > XMP_NAMESPACE.len() && segment.starts_with(XMP_NAMESPACE) {
                let xmp_bytes = &segment[XMP_NAMESPACE.len()..];
                return String::from_utf8(xmp_bytes.to_vec()).ok();
            }
        }
        pos += 2 + length;
    }
    None
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

fn extract_gainmap_from_mpf(data: &[u8]) -> Option<Vec<u8>> {
    let images = ultrahdr_core::metadata::mpf::parse_mpf(data).ok()?;
    if images.len() < 2 {
        return None;
    }

    let (offset, length) = images[1];
    let base_end = offset.checked_add(length)?;

    for delta in [0isize, 8, -8] {
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

        if let (Some(s), Some(e)) = (start, end) {
            if is_valid_jpeg_range(data, s, e) {
                return Some(data[s..e].to_vec());
            }
        }
    }

    // Fallback: scan for JPEG boundaries
    let boundaries = ultrahdr_core::metadata::mpf::find_jpeg_boundaries(data);
    if boundaries.len() >= 2 {
        let (start, end) = boundaries[1];
        if is_valid_jpeg_range(data, start, end) {
            return Some(data[start..end].to_vec());
        }
    }

    None
}

// ---- Ultra HDR Detection ----

struct UltraHdrData {
    metadata: GainMapMetadata,
    gainmap_jpeg: Vec<u8>,
}

fn detect_ultrahdr(data: &[u8]) -> Option<UltraHdrData> {
    let xmp = extract_xmp_from_jpeg_bytes(data)?;
    let (mut metadata, _) = ultrahdr_core::metadata::xmp::parse_xmp(&xmp).ok()?;
    let gainmap_jpeg = extract_gainmap_from_mpf(data)?;

    // If primary metadata looks incomplete, try the gainmap's own XMP
    if metadata_looks_default_or_incomplete(&metadata) {
        if let Some(gm_xmp) = extract_xmp_from_jpeg_bytes(&gainmap_jpeg) {
            if let Ok((mut gm_meta, _)) = ultrahdr_core::metadata::xmp::parse_xmp(&gm_xmp) {
                apply_lenient_xmp_overrides(&gm_xmp, &mut gm_meta);
                if !metadata_looks_default_or_incomplete(&gm_meta) {
                    metadata = gm_meta;
                }
            }
        }
    }

    apply_lenient_xmp_overrides(&xmp, &mut metadata);

    Some(UltraHdrData {
        metadata,
        gainmap_jpeg,
    })
}

// ---- Image Processing ----

fn crop_and_encode_jpeg(img: &DynamicImage, rect: Rect, quality: u8) -> Result<Vec<u8>, String> {
    let cropped = img.crop_imm(rect.x, rect.y, rect.width, rect.height);
    let mut buf = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .write_image(
            cropped.as_bytes(),
            cropped.width(),
            cropped.height(),
            cropped.color().into(),
        )
        .map_err(|e| format!("JPEG encode failed: {e}"))?;
    Ok(buf)
}

// ---- Ultra HDR Container Assembly ----
// Inlined from ultrahdr-rs container.rs (which can't compile without jpegli).

/// Find position after SOI and APP0/APP1 markers to insert MPF APP2.
fn find_mpf_insert_position(data: &[u8]) -> Result<usize, String> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err("Not a valid JPEG".into());
    }

    let mut pos = 2;
    while pos < data.len().saturating_sub(3) {
        if data[pos] != 0xFF {
            break;
        }
        let marker = data[pos + 1];
        // Stop before non-APP0/APP1 markers (MPF goes after APP1)
        if !(0xE0..=0xE1).contains(&marker) {
            break;
        }
        let length = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        pos += 2 + length;
    }

    Ok(pos)
}

/// Create MPF APP2 marker for a multi-image JPEG.
fn create_mpf_app2(
    primary_size: u32,
    secondary_sizes: &[u32],
    mpf_marker_offset: usize,
) -> Vec<u8> {
    let num_images = 1 + secondary_sizes.len();
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
    mpf_data.extend_from_slice(&(num_images as u32).to_be_bytes());

    // Entry 3: MPEntry (0xB002), UNDEFINED(7), count=entries*16, offset after IFD
    let mp_entry_size = (num_images * 16) as u32;
    let mp_entry_offset: u32 = 8 + 2 + 36 + 4; // TIFF hdr + num_entries + 3 IFD entries(12*3) + next IFD ptr
    mpf_data.extend_from_slice(&0xB002u16.to_be_bytes());
    mpf_data.extend_from_slice(&7u16.to_be_bytes());
    mpf_data.extend_from_slice(&mp_entry_size.to_be_bytes());
    mpf_data.extend_from_slice(&mp_entry_offset.to_be_bytes());

    // Next IFD offset: 0 (none)
    mpf_data.extend_from_slice(&0u32.to_be_bytes());

    // MP Entry: primary (attr=0x030000, offset=0)
    mpf_data.extend_from_slice(&0x03_0000u32.to_be_bytes()); // attribute: primary
    mpf_data.extend_from_slice(&primary_size.to_be_bytes());
    mpf_data.extend_from_slice(&0u32.to_be_bytes()); // offset 0 for primary
    mpf_data.extend_from_slice(&0u32.to_be_bytes()); // dependent entries

    // MP Entry: secondaries
    // Per MPF spec, offsets are relative to the TIFF header, which is 8 bytes
    // after the MPF marker start (FF E2(2) + length(2) + "MPF\0"(4)).
    let tiff_header_offset = mpf_marker_offset as u32 + 8;
    let mut offset = primary_size;
    for &size in secondary_sizes {
        let relative_offset = offset - tiff_header_offset;
        mpf_data.extend_from_slice(&0x00_0000u32.to_be_bytes()); // attribute: dependent child
        mpf_data.extend_from_slice(&size.to_be_bytes());
        mpf_data.extend_from_slice(&relative_offset.to_be_bytes());
        mpf_data.extend_from_slice(&0u32.to_be_bytes());
        offset += size;
    }

    // Wrap in APP2 marker
    let total_length = 2 + 4 + mpf_data.len(); // length field + "MPF\0" + data
    let mut marker = Vec::with_capacity(2 + total_length);
    marker.push(0xFF);
    marker.push(0xE2);
    marker.push(((total_length >> 8) & 0xFF) as u8);
    marker.push((total_length & 0xFF) as u8);
    marker.extend_from_slice(b"MPF\0");
    marker.extend_from_slice(&mpf_data);

    marker
}

fn format_xmp_seq(tag: &str, values: &[f32; 3], is_single: bool, use_log2: bool) -> String {
    let v: [f32; 3] = if use_log2 {
        [values[0].log2(), values[1].log2(), values[2].log2()]
    } else {
        *values
    };
    if is_single {
        format!("        hdrgm:{tag}=\"{:.6}\"", v[0])
    } else {
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

fn generate_primary_xmp(metadata: &GainMapMetadata, gainmap_length: usize) -> String {
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

fn generate_gainmap_xmp(metadata: &GainMapMetadata) -> String {
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

fn embed_xmp_in_jpeg(jpeg: &[u8], xmp: &str) -> Vec<u8> {
    let xmp_marker = ultrahdr_core::metadata::xmp::create_xmp_app1_marker(xmp);
    let mut output = Vec::with_capacity(jpeg.len() + xmp_marker.len());
    output.extend_from_slice(&jpeg[..2]); // SOI
    output.extend_from_slice(&xmp_marker);
    output.extend_from_slice(&jpeg[2..]);
    output
}

fn assemble_ultrahdr_tile(
    sdr_jpeg: &[u8],
    gainmap_jpeg: &[u8],
    metadata: &GainMapMetadata,
) -> Result<Vec<u8>, String> {
    // Embed gainmap XMP metadata into the gainmap JPEG
    let gainmap_xmp = generate_gainmap_xmp(metadata);
    let gainmap_jpeg_with_xmp = embed_xmp_in_jpeg(gainmap_jpeg, &gainmap_xmp);

    // Generate primary XMP and create APP1 marker (using gainmap size WITH its XMP)
    let xmp = generate_primary_xmp(metadata, gainmap_jpeg_with_xmp.len());
    let xmp_marker = ultrahdr_core::metadata::xmp::create_xmp_app1_marker(&xmp);

    // Insert XMP APP1 after SOI
    let mut primary_with_xmp = Vec::with_capacity(sdr_jpeg.len() + xmp_marker.len());
    primary_with_xmp.extend_from_slice(&sdr_jpeg[..2]); // SOI
    primary_with_xmp.extend_from_slice(&xmp_marker);
    primary_with_xmp.extend_from_slice(&sdr_jpeg[2..]);

    // Find where to insert MPF APP2
    let insert_pos = find_mpf_insert_position(&primary_with_xmp)?;

    // Calculate MPF header size (deterministic for 2 images)
    let gm_len = gainmap_jpeg_with_xmp.len() as u32;
    let placeholder_mpf = create_mpf_app2(u32::MAX, &[gm_len], insert_pos);
    let mpf_size = placeholder_mpf.len();

    // Final primary size includes the MPF header
    let primary_final_size = (primary_with_xmp.len() + mpf_size) as u32;
    let mpf_header = create_mpf_app2(primary_final_size, &[gm_len], insert_pos);

    // Assemble: primary[..insert] + MPF + primary[insert..] + gainmap
    let total = primary_with_xmp.len() + mpf_header.len() + gainmap_jpeg_with_xmp.len();
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(&primary_with_xmp[..insert_pos]);
    output.extend_from_slice(&mpf_header);
    output.extend_from_slice(&primary_with_xmp[insert_pos..]);
    output.extend_from_slice(&gainmap_jpeg_with_xmp);

    Ok(output)
}

// ---- WASM Exports ----

#[derive(Serialize)]
struct ImageInfo {
    width: u32,
    height: u32,
    aspect: String,
    #[serde(rename = "isUltraHdr")]
    is_ultra_hdr: bool,
    #[serde(rename = "tileWidth")]
    tile_width: u32,
    #[serde(rename = "tileHeight")]
    tile_height: u32,
}

enum Side {
    Left,
    Right,
}

fn split_tile(data: &[u8], quality: u8, side: Side) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(data).map_err(|e| format!("Failed to decode image: {e}"))?;

    let (left_rect, right_rect) = compute_split_rectangles(img.width(), img.height())?;
    let rect = match side {
        Side::Left => left_rect,
        Side::Right => right_rect,
    };

    // Try Ultra HDR path
    if let Some(uhdr) = detect_ultrahdr(data) {
        let gainmap_img = image::load_from_memory(&uhdr.gainmap_jpeg)
            .map_err(|e| format!("Failed to decode gainmap: {e}"))?;

        let gainmap_rect = map_rect_to_gainmap(
            rect,
            img.width(),
            img.height(),
            gainmap_img.width(),
            gainmap_img.height(),
        );

        let sdr_jpeg = crop_and_encode_jpeg(&img, rect, quality)?;
        // Always encode gain map at max quality — quantization errors get amplified
        // exponentially when the gain map is applied (boost = max_boost^(pixel/255)).
        let gm_jpeg = crop_and_encode_jpeg(&gainmap_img, gainmap_rect, 100)?;

        return assemble_ultrahdr_tile(&sdr_jpeg, &gm_jpeg, &uhdr.metadata);
    }

    // Standard path
    crop_and_encode_jpeg(&img, rect, quality)
}

#[wasm_bindgen]
pub fn validate_image(data: &[u8]) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let img = image::load_from_memory(data)
        .map_err(|e| JsValue::from_str(&format!("Failed to decode image: {e}")))?;

    let (width, height) = (img.width(), img.height());
    let is_ultra_hdr = detect_ultrahdr(data).is_some();

    let (left, _) = compute_split_rectangles(width, height).map_err(|e| JsValue::from_str(&e))?;

    let aspect = {
        let ratio = width as f64 / height as f64;
        if is_close(ratio, ASPECT_3_2) {
            "3:2"
        } else if is_close(ratio, ASPECT_16_10) {
            "16:10"
        } else {
            "unknown"
        }
    };

    let info = ImageInfo {
        width,
        height,
        aspect: aspect.to_string(),
        is_ultra_hdr,
        tile_width: left.width,
        tile_height: left.height,
    };

    serde_wasm_bindgen::to_value(&info)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {e}")))
}

#[wasm_bindgen]
pub fn split_left(data: &[u8], quality: u8) -> Result<Vec<u8>, JsValue> {
    console_error_panic_hook::set_once();
    split_tile(data, quality, Side::Left).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn split_right(data: &[u8], quality: u8) -> Result<Vec<u8>, JsValue> {
    console_error_panic_hook::set_once();
    split_tile(data, quality, Side::Right).map_err(|e| JsValue::from_str(&e))
}

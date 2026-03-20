/// Lightweight binary state serialization helpers for save states.
///
/// Each component pushes its fields to a `Vec<u8>` in a fixed order,
/// and reads them back in the same order. No framing, no versioning —
/// save states are tied to the exact code version.

pub fn write_u8(out: &mut Vec<u8>, val: u8) {
    out.push(val);
}

pub fn write_u16(out: &mut Vec<u8>, val: u16) {
    out.extend_from_slice(&val.to_le_bytes());
}

pub fn write_u32(out: &mut Vec<u8>, val: u32) {
    out.extend_from_slice(&val.to_le_bytes());
}

pub fn write_u64(out: &mut Vec<u8>, val: u64) {
    out.extend_from_slice(&val.to_le_bytes());
}

pub fn write_i16(out: &mut Vec<u8>, val: i16) {
    out.extend_from_slice(&val.to_le_bytes());
}

pub fn write_f32(out: &mut Vec<u8>, val: f32) {
    out.extend_from_slice(&val.to_le_bytes());
}

pub fn write_f64(out: &mut Vec<u8>, val: f64) {
    out.extend_from_slice(&val.to_le_bytes());
}

pub fn write_bool(out: &mut Vec<u8>, val: bool) {
    out.push(val as u8);
}

pub fn write_bytes(out: &mut Vec<u8>, data: &[u8]) {
    write_u32(out, data.len() as u32);
    out.extend_from_slice(data);
}

pub fn read_u8(data: &mut &[u8]) -> u8 {
    let v = data[0];
    *data = &data[1..];
    v
}

pub fn read_u16(data: &mut &[u8]) -> u16 {
    let v = u16::from_le_bytes([data[0], data[1]]);
    *data = &data[2..];
    v
}

pub fn read_u32(data: &mut &[u8]) -> u32 {
    let v = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    *data = &data[4..];
    v
}

pub fn read_u64(data: &mut &[u8]) -> u64 {
    let v = u64::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ]);
    *data = &data[8..];
    v
}

pub fn read_i16(data: &mut &[u8]) -> i16 {
    let v = i16::from_le_bytes([data[0], data[1]]);
    *data = &data[2..];
    v
}

pub fn read_f32(data: &mut &[u8]) -> f32 {
    let v = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    *data = &data[4..];
    v
}

pub fn read_f64(data: &mut &[u8]) -> f64 {
    let v = f64::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ]);
    *data = &data[8..];
    v
}

pub fn read_bool(data: &mut &[u8]) -> bool {
    read_u8(data) != 0
}

pub fn read_bytes(data: &mut &[u8]) -> Vec<u8> {
    let len = read_u32(data) as usize;
    let v = data[..len].to_vec();
    *data = &data[len..];
    v
}

pub fn write_mirroring(out: &mut Vec<u8>, m: crate::ines::Mirroring) {
    use crate::ines::Mirroring;
    write_u8(
        out,
        match m {
            Mirroring::Horizontal => 0,
            Mirroring::Vertical => 1,
            Mirroring::FourScreen => 2,
            Mirroring::SingleScreenLower => 3,
            Mirroring::SingleScreenUpper => 4,
        },
    );
}

pub fn read_mirroring(data: &mut &[u8]) -> crate::ines::Mirroring {
    use crate::ines::Mirroring;
    match read_u8(data) {
        0 => Mirroring::Horizontal,
        1 => Mirroring::Vertical,
        2 => Mirroring::FourScreen,
        3 => Mirroring::SingleScreenLower,
        4 => Mirroring::SingleScreenUpper,
        _ => Mirroring::Horizontal,
    }
}

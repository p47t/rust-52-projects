use std::fmt;

const VERSION_1_SIZE: usize = 21;
const TOTAL_CODEWORDS: usize = 26;
const MASK_PATTERN: u8 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum EcLevel {
    L,
    M,
    Q,
    H,
}

#[derive(Debug)]
pub enum QrError {
    DataTooLong,
}

impl fmt::Display for QrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataTooLong => write!(f, "Data too long for a minimal version 1 QR code"),
        }
    }
}

impl std::error::Error for QrError {}

#[derive(Debug)]
pub struct QrCode {
    pub size: usize,
    pub matrix: Vec<Vec<bool>>,
}

pub fn generate(data: &str, ec_level: EcLevel) -> Result<QrCode, QrError> {
    let data_codewords = data_codewords(ec_level);
    let payload = encode_byte_mode(data.as_bytes(), data_codewords)?;
    let ecc = reed_solomon_encode(&payload, ec_codewords(ec_level));
    let mut codewords = payload;
    codewords.extend(ecc);

    let (mut matrix, reserved) = build_base_matrix();
    place_data(&mut matrix, &reserved, &codewords);
    apply_mask(&mut matrix, &reserved, MASK_PATTERN);
    add_format_information(&mut matrix, ec_level, MASK_PATTERN);

    Ok(QrCode {
        size: VERSION_1_SIZE,
        matrix,
    })
}

fn data_codewords(ec_level: EcLevel) -> usize {
    match ec_level {
        EcLevel::L => 19,
        EcLevel::M => 16,
        EcLevel::Q => 13,
        EcLevel::H => 9,
    }
}

fn ec_codewords(ec_level: EcLevel) -> usize {
    TOTAL_CODEWORDS - data_codewords(ec_level)
}

fn encode_byte_mode(data: &[u8], data_codewords: usize) -> Result<Vec<u8>, QrError> {
    let capacity_bits = data_codewords * 8;
    let payload_bits = 4 + 8 + data.len() * 8;
    if payload_bits > capacity_bits {
        return Err(QrError::DataTooLong);
    }

    let mut bits = BitBuffer::new();
    bits.push(0b0100, 4);
    bits.push(data.len() as u32, 8);
    for &byte in data {
        bits.push(byte as u32, 8);
    }

    let remaining = capacity_bits - bits.len();
    bits.push(0, remaining.min(4));
    bits.pad_to_byte();

    let mut codewords = bits.into_bytes();
    let pad_bytes = [0xEC, 0x11];
    let mut pad_index = 0;
    while codewords.len() < data_codewords {
        codewords.push(pad_bytes[pad_index % 2]);
        pad_index += 1;
    }

    Ok(codewords)
}

fn reed_solomon_encode(data: &[u8], degree: usize) -> Vec<u8> {
    let generator = rs_generator_polynomial(degree);
    let mut remainder = vec![0u8; degree];

    for &byte in data {
        let factor = byte ^ remainder[0];
        remainder.rotate_left(1);
        remainder[degree - 1] = 0;
        for (i, &coefficient) in generator.iter().enumerate().skip(1) {
            remainder[i - 1] ^= gf_mul(coefficient, factor);
        }
    }

    remainder
}

fn rs_generator_polynomial(degree: usize) -> Vec<u8> {
    let mut generator = vec![1u8];
    for i in 0..degree {
        let root = gf_pow(2, i as u8);
        let mut next = vec![0u8; generator.len() + 1];
        for (j, &value) in generator.iter().enumerate() {
            next[j] ^= value;
            next[j + 1] ^= gf_mul(value, root);
        }
        generator = next;
    }
    generator
}

fn gf_pow(x: u8, power: u8) -> u8 {
    let mut result = 1u8;
    for _ in 0..power {
        result = gf_mul(result, x);
    }
    result
}

fn gf_mul(mut x: u8, mut y: u8) -> u8 {
    let mut result = 0u8;
    while y != 0 {
        if y & 1 != 0 {
            result ^= x;
        }
        let carry = x & 0x80 != 0;
        x <<= 1;
        if carry {
            x ^= 0x1D;
        }
        y >>= 1;
    }
    result
}

fn build_base_matrix() -> (Vec<Vec<bool>>, Vec<Vec<bool>>) {
    let mut matrix = vec![vec![false; VERSION_1_SIZE]; VERSION_1_SIZE];
    let mut reserved = vec![vec![false; VERSION_1_SIZE]; VERSION_1_SIZE];

    add_finder_pattern(&mut matrix, &mut reserved, 0, 0);
    add_finder_pattern(&mut matrix, &mut reserved, VERSION_1_SIZE - 7, 0);
    add_finder_pattern(&mut matrix, &mut reserved, 0, VERSION_1_SIZE - 7);
    add_timing_patterns(&mut matrix, &mut reserved);
    reserve_format_areas(&mut reserved);
    set_module(&mut matrix, &mut reserved, 8, VERSION_1_SIZE - 8, true);

    (matrix, reserved)
}

fn add_finder_pattern(
    matrix: &mut [Vec<bool>],
    reserved: &mut [Vec<bool>],
    origin_x: usize,
    origin_y: usize,
) {
    for dy in -1isize..=7 {
        for dx in -1isize..=7 {
            let x = origin_x as isize + dx;
            let y = origin_y as isize + dy;
            if x < 0 || y < 0 || x >= VERSION_1_SIZE as isize || y >= VERSION_1_SIZE as isize {
                continue;
            }

            let x = x as usize;
            let y = y as usize;
            let in_finder = (0..=6).contains(&dx) && (0..=6).contains(&dy);
            let dark = in_finder
                && (dx == 0
                    || dx == 6
                    || dy == 0
                    || dy == 6
                    || ((2..=4).contains(&dx) && (2..=4).contains(&dy)));
            set_module(matrix, reserved, x, y, dark);
        }
    }
}

fn add_timing_patterns(matrix: &mut [Vec<bool>], reserved: &mut [Vec<bool>]) {
    for i in 8..(VERSION_1_SIZE - 8) {
        let dark = i % 2 == 0;
        set_module(matrix, reserved, i, 6, dark);
        set_module(matrix, reserved, 6, i, dark);
    }
}

fn reserve_format_areas(reserved: &mut [Vec<bool>]) {
    let first_copy = [
        (8, 0),
        (8, 1),
        (8, 2),
        (8, 3),
        (8, 4),
        (8, 5),
        (8, 7),
        (8, 8),
        (7, 8),
        (5, 8),
        (4, 8),
        (3, 8),
        (2, 8),
        (1, 8),
        (0, 8),
    ];
    let second_copy = [
        (VERSION_1_SIZE - 1, 8),
        (VERSION_1_SIZE - 2, 8),
        (VERSION_1_SIZE - 3, 8),
        (VERSION_1_SIZE - 4, 8),
        (VERSION_1_SIZE - 5, 8),
        (VERSION_1_SIZE - 6, 8),
        (VERSION_1_SIZE - 7, 8),
        (VERSION_1_SIZE - 8, 8),
        (8, VERSION_1_SIZE - 7),
        (8, VERSION_1_SIZE - 6),
        (8, VERSION_1_SIZE - 5),
        (8, VERSION_1_SIZE - 4),
        (8, VERSION_1_SIZE - 3),
        (8, VERSION_1_SIZE - 2),
        (8, VERSION_1_SIZE - 1),
    ];

    for &(x, y) in first_copy.iter().chain(second_copy.iter()) {
        reserved[y][x] = true;
    }
}

fn set_module(
    matrix: &mut [Vec<bool>],
    reserved: &mut [Vec<bool>],
    x: usize,
    y: usize,
    dark: bool,
) {
    matrix[y][x] = dark;
    reserved[y][x] = true;
}

fn place_data(matrix: &mut [Vec<bool>], reserved: &[Vec<bool>], codewords: &[u8]) {
    let bits = codewords
        .iter()
        .flat_map(|byte| (0..8).rev().map(move |shift| ((byte >> shift) & 1) != 0))
        .collect::<Vec<_>>();

    let mut bit_index = 0usize;
    let mut x = VERSION_1_SIZE as isize - 1;
    let mut upward = true;

    while x > 0 {
        if x == 6 {
            x -= 1;
        }

        for offset in 0..VERSION_1_SIZE {
            let y = if upward {
                VERSION_1_SIZE - 1 - offset
            } else {
                offset
            };

            for dx in [0isize, -1] {
                let xx = (x + dx) as usize;
                if reserved[y][xx] {
                    continue;
                }
                let bit = bits.get(bit_index).copied().unwrap_or(false);
                matrix[y][xx] = bit;
                bit_index += 1;
            }
        }

        upward = !upward;
        x -= 2;
    }
}

fn apply_mask(matrix: &mut [Vec<bool>], reserved: &[Vec<bool>], pattern: u8) {
    for y in 0..VERSION_1_SIZE {
        for x in 0..VERSION_1_SIZE {
            if reserved[y][x] {
                continue;
            }
            if mask_applies(pattern, x, y) {
                matrix[y][x] = !matrix[y][x];
            }
        }
    }
}

fn mask_applies(pattern: u8, x: usize, y: usize) -> bool {
    match pattern {
        0 => (x + y).is_multiple_of(2),
        1 => y.is_multiple_of(2),
        2 => x.is_multiple_of(3),
        3 => (x + y).is_multiple_of(3),
        4 => (x / 3 + y / 2).is_multiple_of(2),
        5 => ((x * y) % 2 + (x * y) % 3) == 0,
        6 => ((x * y) % 2 + (x * y) % 3).is_multiple_of(2),
        7 => ((x + y) % 2 + (x * y) % 3).is_multiple_of(2),
        _ => false,
    }
}

fn add_format_information(matrix: &mut [Vec<bool>], ec_level: EcLevel, mask_pattern: u8) {
    let format = format_bits(ec_level, mask_pattern);
    let first_copy = [
        (8, 0),
        (8, 1),
        (8, 2),
        (8, 3),
        (8, 4),
        (8, 5),
        (8, 7),
        (8, 8),
        (7, 8),
        (5, 8),
        (4, 8),
        (3, 8),
        (2, 8),
        (1, 8),
        (0, 8),
    ];
    let second_copy = [
        (VERSION_1_SIZE - 1, 8),
        (VERSION_1_SIZE - 2, 8),
        (VERSION_1_SIZE - 3, 8),
        (VERSION_1_SIZE - 4, 8),
        (VERSION_1_SIZE - 5, 8),
        (VERSION_1_SIZE - 6, 8),
        (VERSION_1_SIZE - 7, 8),
        (VERSION_1_SIZE - 8, 8),
        (8, VERSION_1_SIZE - 7),
        (8, VERSION_1_SIZE - 6),
        (8, VERSION_1_SIZE - 5),
        (8, VERSION_1_SIZE - 4),
        (8, VERSION_1_SIZE - 3),
        (8, VERSION_1_SIZE - 2),
        (8, VERSION_1_SIZE - 1),
    ];

    for (index, &(x, y)) in first_copy.iter().enumerate() {
        matrix[y][x] = ((format >> index) & 1) != 0;
    }
    for (index, &(x, y)) in second_copy.iter().enumerate() {
        matrix[y][x] = ((format >> index) & 1) != 0;
    }
}

fn format_bits(ec_level: EcLevel, mask_pattern: u8) -> u16 {
    let ec_bits = match ec_level {
        EcLevel::L => 0b01,
        EcLevel::M => 0b00,
        EcLevel::Q => 0b11,
        EcLevel::H => 0b10,
    };
    let data = (ec_bits << 3) | mask_pattern as u16;
    let mut remainder = data << 10;
    let generator = 0x537u16;

    while bit_length(remainder) >= 11 {
        let shift = bit_length(remainder) - 11;
        remainder ^= generator << shift;
    }

    ((data << 10) | remainder) ^ 0x5412
}

fn bit_length(value: u16) -> u32 {
    if value == 0 {
        0
    } else {
        u16::BITS - value.leading_zeros()
    }
}

struct BitBuffer {
    bits: Vec<bool>,
}

impl BitBuffer {
    fn new() -> Self {
        Self { bits: Vec::new() }
    }

    fn len(&self) -> usize {
        self.bits.len()
    }

    fn push(&mut self, value: u32, width: usize) {
        for shift in (0..width).rev() {
            self.bits.push(((value >> shift) & 1) != 0);
        }
    }

    fn pad_to_byte(&mut self) {
        while !self.bits.len().is_multiple_of(8) {
            self.bits.push(false);
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bits
            .chunks(8)
            .map(|chunk| {
                chunk.iter().enumerate().fold(0u8, |byte, (index, bit)| {
                    byte | ((*bit as u8) << (7 - index))
                })
            })
            .collect()
    }
}

impl QrCode {
    pub fn render_as_string(&self) -> String {
        self.matrix
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&dark| if dark { "##" } else { "  " })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, ImageBuffer, Luma};

    fn render_to_image(qr: &QrCode, module_size: u32) -> GrayImage {
        let quiet_zone = 4 * module_size;
        let qr_size = qr.size as u32 * module_size;
        let size = qr_size + quiet_zone * 2;
        let mut img = ImageBuffer::from_pixel(size, size, Luma([255u8]));

        for (y, row) in qr.matrix.iter().enumerate() {
            for (x, dark) in row.iter().copied().enumerate() {
                let color = if dark { Luma([0u8]) } else { Luma([255u8]) };
                for dy in 0..module_size {
                    for dx in 0..module_size {
                        img.put_pixel(
                            quiet_zone + x as u32 * module_size + dx,
                            quiet_zone + y as u32 * module_size + dy,
                            color,
                        );
                    }
                }
            }
        }

        img
    }

    fn decode_image(img: GrayImage) -> String {
        let mut prepared = rqrr::PreparedImage::prepare(img);
        let grids = prepared.detect_grids();
        let grid = grids.into_iter().next().expect("expected a QR grid");
        let (_, content) = grid.decode().expect("expected the QR to decode");
        content
    }

    #[test]
    fn generate_small_payload() {
        let qr = generate("HELLO", EcLevel::M).unwrap();
        assert_eq!(qr.size, VERSION_1_SIZE);
    }

    #[test]
    fn generate_empty_payload() {
        let qr = generate("", EcLevel::M).unwrap();
        assert_eq!(qr.size, VERSION_1_SIZE);
    }

    #[test]
    fn rejects_payloads_that_exceed_version_1_capacity() {
        let err = generate("1234567890ABCDE", EcLevel::M).unwrap_err();
        assert!(matches!(err, QrError::DataTooLong));
    }

    #[test]
    fn matrix_is_square() {
        let qr = generate("TEST", EcLevel::M).unwrap();
        assert_eq!(qr.matrix.len(), qr.size);
        for row in &qr.matrix {
            assert_eq!(row.len(), qr.size);
        }
    }

    #[test]
    fn has_finder_patterns() {
        let qr = generate("X", EcLevel::M).unwrap();
        let size = qr.size;
        assert!(qr.matrix[0][0]);
        assert!(qr.matrix[0][size - 7]);
        assert!(qr.matrix[size - 7][0]);
    }

    #[test]
    fn render_output_contains_dark_modules() {
        let qr = generate("A", EcLevel::M).unwrap();
        let output = qr.render_as_string();
        assert!(!output.is_empty());
        assert!(output.contains("##"));
    }

    #[test]
    fn round_trips_supported_payloads() {
        let cases = [
            ("HELLO", EcLevel::L),
            ("HELLO", EcLevel::M),
            ("HELLO", EcLevel::Q),
            ("HELLO", EcLevel::H),
            ("Hello, world!", EcLevel::M),
        ];

        for (payload, ec_level) in cases {
            let qr = generate(payload, ec_level).unwrap();
            let img = render_to_image(&qr, 10);
            let decoded = decode_image(img);
            assert_eq!(decoded, payload);
        }
    }
}

//! Dequantization and quantized dot-product kernels.
//!
//! GGML quantized formats group elements into fixed-size blocks. Each block
//! carries its own scale (and sometimes zero-point), so dot products are
//! computed block-by-block: integer accumulator × block scale.

use half::f16;

pub const QK8_0: usize = 32;
pub const QK4_0: usize = 32;

/// K-quants superblock element count.
pub const QK_K: usize = 256;

/// Q8_0 block: 1 fp16 scale + 32 int8 quantized values.
pub const Q8_0_BLOCK_SIZE: usize = 2 + QK8_0;

/// Q4_0 block: 1 fp16 scale + 16 packed int4 (nibbles).
pub const Q4_0_BLOCK_SIZE: usize = 2 + QK4_0 / 2;

/// Q6_K block: ql(128) + qh(64) + scales(16 i8) + d(fp16) = 210 bytes per 256 elements.
pub const Q6_K_BLOCK_SIZE: usize = QK_K / 2 + QK_K / 4 + QK_K / 16 + 2;

#[inline]
fn read_f16(b: &[u8]) -> f32 {
    f16::from_le_bytes([b[0], b[1]]).to_f32()
}

/// Compute `sum_i dequant(q)[i] * x[i]` for a row encoded as Q8_0 blocks.
/// `q` is the full row payload, `x` is the input vector.
pub fn dot_q8_0(q: &[u8], x: &[f32]) -> f32 {
    debug_assert_eq!(x.len() % QK8_0, 0);
    debug_assert_eq!(q.len(), (x.len() / QK8_0) * Q8_0_BLOCK_SIZE);

    let mut acc = 0.0f32;
    let mut qp = 0usize;
    let mut xp = 0usize;
    let n_blocks = x.len() / QK8_0;
    for _ in 0..n_blocks {
        let d = read_f16(&q[qp..qp + 2]);
        qp += 2;
        let mut s = 0.0f32;
        for i in 0..QK8_0 {
            let qi = q[qp + i] as i8 as f32;
            s += qi * x[xp + i];
        }
        acc += d * s;
        qp += QK8_0;
        xp += QK8_0;
    }
    acc
}

pub fn dot_q4_0(q: &[u8], x: &[f32]) -> f32 {
    debug_assert_eq!(x.len() % QK4_0, 0);
    debug_assert_eq!(q.len(), (x.len() / QK4_0) * Q4_0_BLOCK_SIZE);

    let mut acc = 0.0f32;
    let mut qp = 0usize;
    let mut xp = 0usize;
    let n_blocks = x.len() / QK4_0;
    for _ in 0..n_blocks {
        let d = read_f16(&q[qp..qp + 2]);
        qp += 2;
        let mut s = 0.0f32;
        // First half (i in 0..16) is the low nibble; second half (16..32) is the high nibble.
        // Both nibbles map to signed values via `(nibble as i32) - 8`.
        for i in 0..QK4_0 / 2 {
            let byte = q[qp + i];
            let lo = (byte & 0x0F) as i32 - 8;
            let hi = (byte >> 4) as i32 - 8;
            s += lo as f32 * x[xp + i];
            s += hi as f32 * x[xp + i + QK4_0 / 2];
        }
        acc += d * s;
        qp += QK4_0 / 2;
        xp += QK4_0;
    }
    acc
}

/// Dequantize one Q6_K super-block (`block` is exactly `Q6_K_BLOCK_SIZE` bytes)
/// into 256 f32 outputs at `out`.
///
/// Layout (matches ggml `dequantize_row_q6_K` in ggml-quants.c):
///   ql:     128 bytes — low-4 bits.
///   qh:     64  bytes — high-2 bits.
///   scales: 16  i8    — one per 16-element sub-block.
///   d:      f16       — super-block scale.
///
/// Iterated as two halves of 128 elements. For each half, 32 base positions
/// `l in 0..32` produce four output positions `(l, l+32, l+64, l+96)` whose
/// six-bit quants are stitched from `ql` (low 4 bits) and `qh` (high 2 bits),
/// each scaled by one of four scales drawn from the 16-i8 scale table.
fn dequant_q6_k_block(block: &[u8], out: &mut [f32]) {
    debug_assert_eq!(block.len(), Q6_K_BLOCK_SIZE);
    debug_assert_eq!(out.len(), QK_K);

    let ql_base = 0;
    let qh_base = QK_K / 2;
    let sc_base = QK_K / 2 + QK_K / 4;
    let d = read_f16(&block[Q6_K_BLOCK_SIZE - 2..]);

    let ql = &block[ql_base..ql_base + QK_K / 2];
    let qh = &block[qh_base..qh_base + QK_K / 4];
    let scales = &block[sc_base..sc_base + QK_K / 16];

    // Two 128-element halves. Each half consumes 64 ql bytes, 32 qh bytes,
    // and 8 scale bytes. Within a half, l in 0..32 produces 4 outputs whose
    // sub-block (16-element) scales depend on l < 16 vs l >= 16.
    for n in 0..2 {
        let ql = &ql[n * 64..(n + 1) * 64];
        let qh = &qh[n * 32..(n + 1) * 32];
        let sc = &scales[n * 8..(n + 1) * 8];
        let out = &mut out[n * 128..(n + 1) * 128];
        for l in 0..32 {
            let is = l / 16; // 0 if l<16 else 1
            let q1 = ((ql[l] & 0xF) as i32 | ((qh[l] & 3) as i32) << 4) - 32;
            let q2 = ((ql[l + 32] & 0xF) as i32 | (((qh[l] >> 2) & 3) as i32) << 4) - 32;
            let q3 = ((ql[l] >> 4) as i32 | (((qh[l] >> 4) & 3) as i32) << 4) - 32;
            let q4 = ((ql[l + 32] >> 4) as i32 | (((qh[l] >> 6) & 3) as i32) << 4) - 32;
            out[l] = d * (sc[is] as i8 as i32) as f32 * q1 as f32;
            out[l + 32] = d * (sc[is + 2] as i8 as i32) as f32 * q2 as f32;
            out[l + 64] = d * (sc[is + 4] as i8 as i32) as f32 * q3 as f32;
            out[l + 96] = d * (sc[is + 6] as i8 as i32) as f32 * q4 as f32;
        }
    }
}

pub fn dequant_row_q6_k(q: &[u8], out: &mut [f32]) {
    debug_assert!(out.len().is_multiple_of(QK_K));
    let n_blocks = out.len() / QK_K;
    for b in 0..n_blocks {
        let qb = &q[b * Q6_K_BLOCK_SIZE..(b + 1) * Q6_K_BLOCK_SIZE];
        let ob = &mut out[b * QK_K..(b + 1) * QK_K];
        dequant_q6_k_block(qb, ob);
    }
}

pub fn dot_q6_k(q: &[u8], x: &[f32]) -> f32 {
    debug_assert!(x.len().is_multiple_of(QK_K));
    debug_assert_eq!(q.len(), (x.len() / QK_K) * Q6_K_BLOCK_SIZE);
    let mut acc = 0.0f32;
    let mut tmp = [0.0f32; QK_K];
    let n_blocks = x.len() / QK_K;
    for b in 0..n_blocks {
        let qb = &q[b * Q6_K_BLOCK_SIZE..(b + 1) * Q6_K_BLOCK_SIZE];
        dequant_q6_k_block(qb, &mut tmp);
        let xb = &x[b * QK_K..(b + 1) * QK_K];
        let mut s = 0.0f32;
        for i in 0..QK_K {
            s += tmp[i] * xb[i];
        }
        acc += s;
    }
    acc
}

pub fn dot_f32(q: &[u8], x: &[f32]) -> f32 {
    debug_assert_eq!(q.len(), x.len() * 4);
    let mut acc = 0.0f32;
    for (i, xi) in x.iter().enumerate() {
        let bytes = [q[i * 4], q[i * 4 + 1], q[i * 4 + 2], q[i * 4 + 3]];
        acc += f32::from_le_bytes(bytes) * xi;
    }
    acc
}

pub fn dot_f16(q: &[u8], x: &[f32]) -> f32 {
    debug_assert_eq!(q.len(), x.len() * 2);
    let mut acc = 0.0f32;
    for (i, xi) in x.iter().enumerate() {
        let v = f16::from_le_bytes([q[i * 2], q[i * 2 + 1]]).to_f32();
        acc += v * xi;
    }
    acc
}

/// Dequantize a full row into `out` (len must equal logical element count).
pub fn dequant_row_q8_0(q: &[u8], out: &mut [f32]) {
    debug_assert_eq!(out.len() % QK8_0, 0);
    let n_blocks = out.len() / QK8_0;
    let mut qp = 0usize;
    let mut op = 0usize;
    for _ in 0..n_blocks {
        let d = read_f16(&q[qp..qp + 2]);
        qp += 2;
        for i in 0..QK8_0 {
            out[op + i] = (q[qp + i] as i8) as f32 * d;
        }
        qp += QK8_0;
        op += QK8_0;
    }
}

pub fn dequant_row_q4_0(q: &[u8], out: &mut [f32]) {
    debug_assert_eq!(out.len() % QK4_0, 0);
    let n_blocks = out.len() / QK4_0;
    let mut qp = 0usize;
    let mut op = 0usize;
    for _ in 0..n_blocks {
        let d = read_f16(&q[qp..qp + 2]);
        qp += 2;
        for i in 0..QK4_0 / 2 {
            let byte = q[qp + i];
            let lo = (byte & 0x0F) as i32 - 8;
            let hi = (byte >> 4) as i32 - 8;
            out[op + i] = lo as f32 * d;
            out[op + i + QK4_0 / 2] = hi as f32 * d;
        }
        qp += QK4_0 / 2;
        op += QK4_0;
    }
}

pub fn dequant_row_f16(q: &[u8], out: &mut [f32]) {
    debug_assert_eq!(q.len(), out.len() * 2);
    for (i, o) in out.iter_mut().enumerate() {
        *o = f16::from_le_bytes([q[i * 2], q[i * 2 + 1]]).to_f32();
    }
}

pub fn dequant_row_f32(q: &[u8], out: &mut [f32]) {
    debug_assert_eq!(q.len(), out.len() * 4);
    for (i, o) in out.iter_mut().enumerate() {
        *o = f32::from_le_bytes([q[i * 4], q[i * 4 + 1], q[i * 4 + 2], q[i * 4 + 3]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q8_0_roundtrip_dot() {
        // Build one block: scale 0.5, qs all 1 → values all 0.5.
        let mut block = Vec::new();
        let d = f16::from_f32(0.5);
        block.extend_from_slice(&d.to_le_bytes());
        for _ in 0..QK8_0 {
            block.push(1i8 as u8);
        }
        let x = vec![2.0f32; QK8_0];
        let got = dot_q8_0(&block, &x);
        // 32 * 0.5 * 2 = 32
        assert!((got - 32.0).abs() < 1e-3, "got {got}");

        let mut out = vec![0.0f32; QK8_0];
        dequant_row_q8_0(&block, &mut out);
        for v in &out {
            assert!((v - 0.5).abs() < 1e-3);
        }
    }

    #[test]
    fn q4_0_dot_matches_dequant_then_dot() {
        // 2 blocks (64 elements), random-ish data. Compare dot_q4_0 against
        // dequant_row_q4_0 + naive f32 dot.
        let mut q = Vec::new();
        for blk in 0..2 {
            let d = f16::from_f32(0.25 * (blk as f32 + 1.0));
            q.extend_from_slice(&d.to_le_bytes());
            for j in 0..QK4_0 / 2 {
                q.push(((j as u8 & 0x0F) | (((j as u8 + 1) & 0x0F) << 4)).wrapping_mul(1));
            }
        }
        let x: Vec<f32> = (0..64).map(|i| ((i % 7) as f32) - 3.0).collect();
        let got = dot_q4_0(&q, &x);
        let mut deq = vec![0.0f32; 64];
        dequant_row_q4_0(&q, &mut deq);
        let expected: f32 = deq.iter().zip(x.iter()).map(|(a, b)| a * b).sum();
        assert!(
            (got - expected).abs() < 1e-3,
            "dot_q4_0 = {got} vs dequant+dot = {expected}",
        );
    }

    #[test]
    fn q6_k_dot_matches_dequant_then_dot() {
        // 1 block (256 elements). Build a deterministic block payload.
        let mut q = vec![0u8; Q6_K_BLOCK_SIZE];
        for (i, slot) in q.iter_mut().take(QK_K / 2).enumerate() {
            *slot = ((i & 0x0F) | (((i + 5) & 0x0F) << 4)) as u8;
        }
        for (i, slot) in q.iter_mut().skip(QK_K / 2).take(QK_K / 4).enumerate() {
            *slot = ((i * 11) & 0xFF) as u8;
        }
        for (i, slot) in q
            .iter_mut()
            .skip(QK_K / 2 + QK_K / 4)
            .take(QK_K / 16)
            .enumerate()
        {
            *slot = ((i as i8) - 4) as u8;
        }
        let d = f16::from_f32(0.0625);
        let off = Q6_K_BLOCK_SIZE - 2;
        q[off] = d.to_le_bytes()[0];
        q[off + 1] = d.to_le_bytes()[1];
        let x: Vec<f32> = (0..QK_K)
            .map(|i| (((i * 13) % 19) as f32 - 9.0) * 0.1)
            .collect();

        let got = dot_q6_k(&q, &x);
        let mut deq = vec![0.0f32; QK_K];
        dequant_row_q6_k(&q, &mut deq);
        let expected: f32 = deq.iter().zip(x.iter()).map(|(a, b)| a * b).sum();
        assert!(
            (got - expected).abs() < 1e-3,
            "dot_q6_k = {got} vs dequant+dot = {expected}",
        );
    }

    #[test]
    fn q4_0_roundtrip_dot() {
        // Block: scale 1.0, all nibbles = 0x0F → low and high both = 7.
        let mut block = Vec::new();
        let d = f16::from_f32(1.0);
        block.extend_from_slice(&d.to_le_bytes());
        block.extend(std::iter::repeat_n(0xFFu8, QK4_0 / 2));
        let x = vec![1.0f32; QK4_0];
        let got = dot_q4_0(&block, &x);
        // 32 * 7 = 224
        assert!((got - 224.0).abs() < 1e-3, "got {got}");
    }
}

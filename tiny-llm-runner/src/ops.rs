//! Transformer building blocks operating on f32 vectors.

use rayon::prelude::*;

use crate::tensor::TensorView;

/// Root-mean-square layer norm: `out = w ⊙ (x / rms(x))`.
pub fn rmsnorm(out: &mut [f32], x: &[f32], w: &[f32], eps: f32) {
    debug_assert_eq!(out.len(), x.len());
    debug_assert_eq!(out.len(), w.len());
    let mut ss = 0.0f64;
    for &v in x {
        ss += v as f64 * v as f64;
    }
    ss /= x.len() as f64;
    ss += eps as f64;
    let scale = (1.0 / ss.sqrt()) as f32;
    for i in 0..x.len() {
        out[i] = w[i] * (x[i] * scale);
    }
}

/// `out[i] = dot(W.row(i), x)` for `i in 0..rows`. Parallelized over rows.
pub fn matvec(out: &mut [f32], w: &TensorView<'_>, x: &[f32]) {
    debug_assert_eq!(out.len(), w.dim1());
    debug_assert_eq!(x.len(), w.dim0());
    out.par_iter_mut().enumerate().for_each(|(i, o)| {
        *o = w.dot_row(i, x);
    });
}

/// In-place softmax over `x`.
pub fn softmax(x: &mut [f32]) {
    let mut max = f32::NEG_INFINITY;
    for &v in x.iter() {
        if v > max {
            max = v;
        }
    }
    let mut sum = 0.0f32;
    for v in x.iter_mut() {
        *v = (*v - max).exp();
        sum += *v;
    }
    let inv = 1.0 / sum;
    for v in x.iter_mut() {
        *v *= inv;
    }
}

/// RoPE convention. `Llama` is the original adjacent-pair `(x[2i], x[2i+1])`
/// rotation used by `llama.cpp`'s `convert.py` (which permutes Q/K so this
/// matches HF's "rotate half"). `Neox` is the unpermuted `(x[i], x[i+d/2])`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RopeStyle {
    Llama,
    Neox,
}

/// Apply rotary position embedding to a stacked head-vector at position `pos`.
pub fn apply_rope(
    vec: &mut [f32],
    pos: usize,
    head_dim: usize,
    rot_dim: usize,
    base: f32,
    style: RopeStyle,
) {
    debug_assert!(rot_dim <= head_dim);
    debug_assert_eq!(vec.len() % head_dim, 0);
    debug_assert!(rot_dim.is_multiple_of(2));
    let half = rot_dim / 2;
    let n_heads = vec.len() / head_dim;
    for h in 0..n_heads {
        let off = h * head_dim;
        for i in 0..half {
            let freq = base.powf(-((2 * i) as f32) / rot_dim as f32);
            let theta = pos as f32 * freq;
            let (sin, cos) = theta.sin_cos();
            let (i0, i1) = match style {
                RopeStyle::Llama => (2 * i, 2 * i + 1),
                RopeStyle::Neox => (i, i + half),
            };
            let v0 = vec[off + i0];
            let v1 = vec[off + i1];
            vec[off + i0] = v0 * cos - v1 * sin;
            vec[off + i1] = v0 * sin + v1 * cos;
        }
    }
}

/// SiLU (Swish): `x * sigmoid(x)`.
#[inline]
pub fn silu(x: f32) -> f32 {
    x / (1.0 + (-x).exp())
}

/// Add `b` into `a` in-place.
pub fn add_inplace(a: &mut [f32], b: &[f32]) {
    debug_assert_eq!(a.len(), b.len());
    for (x, y) in a.iter_mut().zip(b.iter()) {
        *x += *y;
    }
}

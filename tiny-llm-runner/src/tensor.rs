//! A tensor view backed by an mmap'd byte slice plus a quantization type.
//!
//! Tensors are stored row-major in GGUF; for a 2-D weight matrix of shape
//! `[K, N]` (in GGUF dim order: `dimensions = [K, N]` means `K` columns and
//! `N` rows), each row has `K` logical elements. We expose row-by-row
//! access plus a fast `dot_row(i, x)` that computes the dot product of row
//! `i` with an f32 vector, avoiding any full-matrix dequant.

use anyhow::{bail, Result};
use llm_gguf_parser::{GgmlType, TensorInfo};

use crate::dequant;

#[derive(Clone, Copy)]
pub struct TensorView<'a> {
    pub data: &'a [u8],
    pub ggml_type: GgmlType,
    pub dims: [u64; 4],
    pub n_dims: usize,
}

impl<'a> TensorView<'a> {
    pub fn from_info(info: &TensorInfo, blob: &'a [u8]) -> Result<Self> {
        let mut dims = [1u64; 4];
        for (i, d) in info.dimensions.iter().enumerate().take(4) {
            dims[i] = *d;
        }
        let n_dims = info.dimensions.len();
        let elem_count: u64 = info.dimensions.iter().product();
        let nbytes = bytes_for(info.ggml_type_or_panic(), elem_count)?;
        let start = info.offset as usize;
        let end = start + nbytes;
        if end > blob.len() {
            bail!(
                "tensor {} out of range: {start}..{end} > blob {}",
                info.name,
                blob.len()
            );
        }
        Ok(Self {
            data: &blob[start..end],
            ggml_type: info.ggml_type_or_panic(),
            dims,
            n_dims,
        })
    }

    /// Logical element count along axis 0 (the row stride for 2-D matrices).
    pub fn dim0(&self) -> usize {
        self.dims[0] as usize
    }

    pub fn dim1(&self) -> usize {
        self.dims[1] as usize
    }

    /// Bytes per row for 2-D weight matrices.
    pub fn row_bytes(&self) -> usize {
        bytes_per_row(self.ggml_type, self.dim0())
    }

    /// Slice covering row `i` of a 2-D weight.
    pub fn row(&self, i: usize) -> &'a [u8] {
        let rb = self.row_bytes();
        &self.data[i * rb..(i + 1) * rb]
    }

    /// Dot product of row `i` with input vector `x` (length = dim0).
    pub fn dot_row(&self, i: usize, x: &[f32]) -> f32 {
        debug_assert_eq!(x.len(), self.dim0());
        let row = self.row(i);
        match self.ggml_type {
            GgmlType::F32 => dequant::dot_f32(row, x),
            GgmlType::F16 => dequant::dot_f16(row, x),
            GgmlType::Q8_0 => dequant::dot_q8_0(row, x),
            GgmlType::Q4_0 => dequant::dot_q4_0(row, x),
            GgmlType::Q6_K => dequant::dot_q6_k(row, x),
            t => panic!("unsupported tensor type for matmul: {t}"),
        }
    }

    /// Dequantize row `i` into `out` (length = dim0).
    pub fn dequant_row(&self, i: usize, out: &mut [f32]) {
        let row = self.row(i);
        match self.ggml_type {
            GgmlType::F32 => dequant::dequant_row_f32(row, out),
            GgmlType::F16 => dequant::dequant_row_f16(row, out),
            GgmlType::Q8_0 => dequant::dequant_row_q8_0(row, out),
            GgmlType::Q4_0 => dequant::dequant_row_q4_0(row, out),
            GgmlType::Q6_K => dequant::dequant_row_q6_k(row, out),
            t => panic!("unsupported tensor type for dequant: {t}"),
        }
    }
}

/// Helper trait to keep call sites tidy. The parser already validates the
/// type id, so unwrapping in the constructor is fine.
pub trait TensorInfoExt {
    fn ggml_type_or_panic(&self) -> GgmlType;
}

impl TensorInfoExt for TensorInfo {
    fn ggml_type_or_panic(&self) -> GgmlType {
        self.tensor_type
    }
}

fn bytes_for(t: GgmlType, elems: u64) -> Result<usize> {
    let elems = elems as usize;
    Ok(match t {
        GgmlType::F32 => elems * 4,
        GgmlType::F16 => elems * 2,
        GgmlType::Q8_0 => {
            if !elems.is_multiple_of(dequant::QK8_0) {
                bail!(
                    "Q8_0 element count {elems} not multiple of {}",
                    dequant::QK8_0
                );
            }
            (elems / dequant::QK8_0) * dequant::Q8_0_BLOCK_SIZE
        }
        GgmlType::Q4_0 => {
            if !elems.is_multiple_of(dequant::QK4_0) {
                bail!(
                    "Q4_0 element count {elems} not multiple of {}",
                    dequant::QK4_0
                );
            }
            (elems / dequant::QK4_0) * dequant::Q4_0_BLOCK_SIZE
        }
        GgmlType::Q6_K => {
            if !elems.is_multiple_of(dequant::QK_K) {
                bail!(
                    "Q6_K element count {elems} not multiple of {}",
                    dequant::QK_K
                );
            }
            (elems / dequant::QK_K) * dequant::Q6_K_BLOCK_SIZE
        }
        other => bail!("unsupported tensor type: {other}"),
    })
}

fn bytes_per_row(t: GgmlType, cols: usize) -> usize {
    match t {
        GgmlType::F32 => cols * 4,
        GgmlType::F16 => cols * 2,
        GgmlType::Q8_0 => (cols / dequant::QK8_0) * dequant::Q8_0_BLOCK_SIZE,
        GgmlType::Q4_0 => (cols / dequant::QK4_0) * dequant::Q4_0_BLOCK_SIZE,
        GgmlType::Q6_K => (cols / dequant::QK_K) * dequant::Q6_K_BLOCK_SIZE,
        other => panic!("unsupported tensor type: {other}"),
    }
}

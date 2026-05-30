//! Model weights: locate, validate, and expose every tensor needed for the
//! Llama forward pass. The byte data lives in the mmap; we only build
//! lightweight views.

use anyhow::{bail, Context, Result};
use llm_gguf_parser::{GgmlType, TensorInfo};
use std::collections::HashMap;

use crate::config::LlamaConfig;
use crate::dequant;
use crate::tensor::TensorView;

pub struct LayerWeights<'a> {
    pub attn_norm: Vec<f32>,
    pub wq: TensorView<'a>,
    pub wk: TensorView<'a>,
    pub wv: TensorView<'a>,
    pub wo: TensorView<'a>,
    pub ffn_norm: Vec<f32>,
    pub w_gate: TensorView<'a>,
    pub w_up: TensorView<'a>,
    pub w_down: TensorView<'a>,
}

pub struct LlamaModel<'a> {
    pub config: LlamaConfig,
    pub token_embd: TensorView<'a>,
    pub output_norm: Vec<f32>,
    pub output: TensorView<'a>,
    pub layers: Vec<LayerWeights<'a>>,
}

impl<'a> LlamaModel<'a> {
    pub fn load(config: LlamaConfig, tensors: &[TensorInfo], blob: &'a [u8]) -> Result<Self> {
        let by_name: HashMap<&str, &TensorInfo> =
            tensors.iter().map(|t| (t.name.as_str(), t)).collect();

        let token_embd = view(&by_name, "token_embd.weight", blob)?;
        let output_norm = load_f32_vec(&by_name, "output_norm.weight", blob)?;
        // Some models tie output to token_embd; in GGUF this is typically
        // explicit (`output.weight` is present). We support both.
        let output = match by_name.get("output.weight") {
            Some(info) => TensorView::from_info(info, blob)?,
            None => token_embd,
        };

        let mut layers = Vec::with_capacity(config.n_layer);
        for l in 0..config.n_layer {
            layers.push(LayerWeights {
                attn_norm: load_f32_vec(&by_name, &format!("blk.{l}.attn_norm.weight"), blob)?,
                wq: view(&by_name, &format!("blk.{l}.attn_q.weight"), blob)?,
                wk: view(&by_name, &format!("blk.{l}.attn_k.weight"), blob)?,
                wv: view(&by_name, &format!("blk.{l}.attn_v.weight"), blob)?,
                wo: view(&by_name, &format!("blk.{l}.attn_output.weight"), blob)?,
                ffn_norm: load_f32_vec(&by_name, &format!("blk.{l}.ffn_norm.weight"), blob)?,
                w_gate: view(&by_name, &format!("blk.{l}.ffn_gate.weight"), blob)?,
                w_up: view(&by_name, &format!("blk.{l}.ffn_up.weight"), blob)?,
                w_down: view(&by_name, &format!("blk.{l}.ffn_down.weight"), blob)?,
            });
        }

        Ok(Self {
            config,
            token_embd,
            output_norm,
            output,
            layers,
        })
    }

    /// Copy embedding row for `token` into `out` (length = n_embd).
    pub fn embed(&self, token: u32, out: &mut [f32]) {
        debug_assert_eq!(out.len(), self.config.n_embd);
        // token_embd has dim0 = n_embd (row width), dim1 = vocab_size (rows).
        self.token_embd.dequant_row(token as usize, out);
    }
}

fn view<'a>(
    by_name: &HashMap<&str, &TensorInfo>,
    name: &str,
    blob: &'a [u8],
) -> Result<TensorView<'a>> {
    let info = by_name
        .get(name)
        .with_context(|| format!("missing tensor {name}"))?;
    TensorView::from_info(info, blob)
}

fn load_f32_vec(by_name: &HashMap<&str, &TensorInfo>, name: &str, blob: &[u8]) -> Result<Vec<f32>> {
    let info = by_name
        .get(name)
        .with_context(|| format!("missing tensor {name}"))?;
    if info.tensor_type != GgmlType::F32 {
        bail!("expected {name} to be F32, got {:?}", info.tensor_type);
    }
    let elems: u64 = info.dimensions.iter().product();
    let nbytes = (elems * 4) as usize;
    let start = info.offset as usize;
    let end = start + nbytes;
    if end > blob.len() {
        bail!(
            "tensor {name} out of range: {start}..{end} > blob {}",
            blob.len()
        );
    }
    let mut out = vec![0.0f32; elems as usize];
    dequant::dequant_row_f32(&blob[start..end], &mut out);
    Ok(out)
}

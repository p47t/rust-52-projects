use anyhow::{anyhow, bail, Context, Result};
use llm_gguf_parser::{GgufFile, Value};

#[derive(Debug, Clone)]
pub struct LlamaConfig {
    pub n_ctx: usize,
    pub n_embd: usize,
    pub n_layer: usize,
    pub n_head: usize,
    pub n_head_kv: usize,
    pub n_ff: usize,
    pub vocab_size: usize,
    pub rms_eps: f32,
    pub rope_freq_base: f32,
    pub rope_dim_count: usize,
}

impl LlamaConfig {
    pub fn from_gguf(g: &GgufFile) -> Result<Self> {
        let arch = match g.metadata.get("general.architecture") {
            Some(Value::String(s)) => s.as_str(),
            _ => bail!("missing general.architecture"),
        };
        if arch != "llama" {
            bail!("unsupported architecture {arch:?}: only `llama` is supported");
        }

        let n_embd = get_usize(g, "llama.embedding_length")?;
        let n_layer = get_usize(g, "llama.block_count")?;
        let n_head = get_usize(g, "llama.attention.head_count")?;
        let n_head_kv = get_usize(g, "llama.attention.head_count_kv").unwrap_or(n_head);
        let n_ff = get_usize(g, "llama.feed_forward_length")?;
        let n_ctx = get_usize(g, "llama.context_length")?;
        let rms_eps = get_f32(g, "llama.attention.layer_norm_rms_epsilon").unwrap_or(1e-5);
        let rope_freq_base = get_f32(g, "llama.rope.freq_base").unwrap_or(10000.0);
        let head_dim = n_embd / n_head;
        let rope_dim_count = get_usize(g, "llama.rope.dimension_count").unwrap_or(head_dim);

        let vocab_size = match g.metadata.get("tokenizer.ggml.tokens") {
            Some(Value::Array(_, v)) => v.len(),
            _ => bail!("missing tokenizer.ggml.tokens"),
        };

        if n_embd % n_head != 0 {
            bail!("n_embd {n_embd} not divisible by n_head {n_head}");
        }
        if n_head % n_head_kv != 0 {
            bail!("n_head {n_head} not divisible by n_head_kv {n_head_kv}");
        }

        Ok(Self {
            n_ctx,
            n_embd,
            n_layer,
            n_head,
            n_head_kv,
            n_ff,
            vocab_size,
            rms_eps,
            rope_freq_base,
            rope_dim_count,
        })
    }

    pub fn head_dim(&self) -> usize {
        self.n_embd / self.n_head
    }

    pub fn kv_dim(&self) -> usize {
        self.head_dim() * self.n_head_kv
    }

    pub fn gqa_groups(&self) -> usize {
        self.n_head / self.n_head_kv
    }
}

fn get_usize(g: &GgufFile, key: &str) -> Result<usize> {
    let v = g
        .metadata
        .get(key)
        .with_context(|| format!("missing metadata {key}"))?;
    match v {
        Value::Uint8(x) => Ok(*x as usize),
        Value::Uint16(x) => Ok(*x as usize),
        Value::Uint32(x) => Ok(*x as usize),
        Value::Uint64(x) => Ok(*x as usize),
        Value::Int8(x) => Ok(*x as usize),
        Value::Int16(x) => Ok(*x as usize),
        Value::Int32(x) => Ok(*x as usize),
        Value::Int64(x) => Ok(*x as usize),
        _ => Err(anyhow!("metadata {key} is not an integer")),
    }
}

fn get_f32(g: &GgufFile, key: &str) -> Result<f32> {
    let v = g
        .metadata
        .get(key)
        .with_context(|| format!("missing metadata {key}"))?;
    match v {
        Value::Float32(x) => Ok(*x),
        Value::Float64(x) => Ok(*x as f32),
        _ => Err(anyhow!("metadata {key} is not a float")),
    }
}

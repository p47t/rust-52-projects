//! Forward pass + KV cache. Single-batch, single-sequence.

use crate::model::LlamaModel;
use crate::ops::{add_inplace, apply_rope, matvec, rmsnorm, silu, softmax, RopeStyle};

pub struct Runner<'a, 'm> {
    model: &'m LlamaModel<'a>,
    rope_style: RopeStyle,
    /// Per-layer K cache: `[n_layer][n_ctx * kv_dim]`.
    kcache: Vec<Vec<f32>>,
    /// Per-layer V cache: `[n_layer][n_ctx * kv_dim]`.
    vcache: Vec<Vec<f32>>,
    pub pos: usize,

    // Scratch buffers, reused every forward pass.
    x: Vec<f32>,
    xb: Vec<f32>,
    xb2: Vec<f32>,
    hb: Vec<f32>,
    hb2: Vec<f32>,
    q: Vec<f32>,
    att: Vec<f32>,
    logits: Vec<f32>,
}

impl<'a, 'm> Runner<'a, 'm> {
    pub fn new(model: &'m LlamaModel<'a>, rope_style: RopeStyle) -> Self {
        let cfg = &model.config;
        let n_embd = cfg.n_embd;
        let kv_dim = cfg.kv_dim();
        let n_layer = cfg.n_layer;
        let n_ctx = cfg.n_ctx;
        let n_ff = cfg.n_ff;
        let vocab = cfg.vocab_size;

        Self {
            model,
            rope_style,
            kcache: (0..n_layer).map(|_| vec![0.0; n_ctx * kv_dim]).collect(),
            vcache: (0..n_layer).map(|_| vec![0.0; n_ctx * kv_dim]).collect(),
            pos: 0,
            x: vec![0.0; n_embd],
            xb: vec![0.0; n_embd],
            xb2: vec![0.0; n_embd],
            hb: vec![0.0; n_ff],
            hb2: vec![0.0; n_ff],
            q: vec![0.0; n_embd],
            att: vec![0.0; cfg.n_head * n_ctx],
            logits: vec![0.0; vocab],
        }
    }

    pub fn reset(&mut self) {
        self.pos = 0;
    }

    /// Run one forward step for `token` at the current position; advance `pos`.
    /// Returns a slice of logits (length = vocab_size).
    pub fn forward(&mut self, token: u32) -> &[f32] {
        let cfg = &self.model.config;
        let head_dim = cfg.head_dim();
        let kv_dim = cfg.kv_dim();
        let gqa = cfg.gqa_groups();
        let pos = self.pos;
        assert!(pos < cfg.n_ctx, "context overflow");

        // 1. Embed.
        self.model.embed(token, &mut self.x);

        // 2. Layers.
        for l in 0..cfg.n_layer {
            let layer = &self.model.layers[l];

            // attention norm
            rmsnorm(&mut self.xb, &self.x, &layer.attn_norm, cfg.rms_eps);

            // qkv projections
            matvec(&mut self.q, &layer.wq, &self.xb);
            // K and V go directly into the cache row at `pos`.
            let kc = &mut self.kcache[l];
            let vc = &mut self.vcache[l];
            let krow = &mut kc[pos * kv_dim..(pos + 1) * kv_dim];
            let vrow = &mut vc[pos * kv_dim..(pos + 1) * kv_dim];
            matvec(krow, &layer.wk, &self.xb);
            matvec(vrow, &layer.wv, &self.xb);

            // RoPE on Q and current-step K.
            apply_rope(
                &mut self.q,
                pos,
                head_dim,
                cfg.rope_dim_count,
                cfg.rope_freq_base,
                self.rope_style,
            );
            apply_rope(
                krow,
                pos,
                head_dim,
                cfg.rope_dim_count,
                cfg.rope_freq_base,
                self.rope_style,
            );

            // Multi-head attention with GQA.
            let scale = 1.0 / (head_dim as f32).sqrt();
            // Zero out scratch attention output once per layer.
            for v in self.xb.iter_mut() {
                *v = 0.0;
            }
            // Attention is naturally parallel across heads, but each head writes
            // to a disjoint slice of `xb`. Keep it sequential — head_dim is
            // small and matvec is the actual hotspot.
            for h in 0..cfg.n_head {
                let kv_head = h / gqa;
                let q_off = h * head_dim;
                let q = &self.q[q_off..q_off + head_dim];

                // Compute attention scores against all cached K up to and including pos.
                let att = &mut self.att[h * cfg.n_ctx..h * cfg.n_ctx + (pos + 1)];
                for (t, score) in att.iter_mut().enumerate() {
                    let k_off = t * kv_dim + kv_head * head_dim;
                    let k = &self.kcache[l][k_off..k_off + head_dim];
                    let mut s = 0.0f32;
                    for i in 0..head_dim {
                        s += q[i] * k[i];
                    }
                    *score = s * scale;
                }
                softmax(att);

                // Weighted sum of V.
                let out_slice = &mut self.xb[q_off..q_off + head_dim];
                for v in out_slice.iter_mut() {
                    *v = 0.0;
                }
                for (t, &a) in att.iter().enumerate() {
                    let v_off = t * kv_dim + kv_head * head_dim;
                    let v = &self.vcache[l][v_off..v_off + head_dim];
                    for i in 0..head_dim {
                        out_slice[i] += a * v[i];
                    }
                }
            }

            // Output projection.
            matvec(&mut self.xb2, &layer.wo, &self.xb);
            add_inplace(&mut self.x, &self.xb2);

            // FFN: x = x + Wdown(silu(Wgate(norm(x))) * Wup(norm(x)))
            rmsnorm(&mut self.xb, &self.x, &layer.ffn_norm, cfg.rms_eps);
            matvec(&mut self.hb, &layer.w_gate, &self.xb);
            matvec(&mut self.hb2, &layer.w_up, &self.xb);
            for i in 0..self.hb.len() {
                self.hb[i] = silu(self.hb[i]) * self.hb2[i];
            }
            matvec(&mut self.xb2, &layer.w_down, &self.hb);
            add_inplace(&mut self.x, &self.xb2);
        }

        // 3. Final norm + lm_head.
        rmsnorm(&mut self.xb, &self.x, &self.model.output_norm, cfg.rms_eps);
        matvec(&mut self.logits, &self.model.output, &self.xb);

        self.pos += 1;
        &self.logits
    }
}

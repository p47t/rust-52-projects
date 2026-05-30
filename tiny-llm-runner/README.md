# tiny-llm-runner

A pure-Rust Llama-architecture inference engine that runs a GGUF model end-to-end with zero C dependencies. Built on top of `llm-gguf-parser` from this same repo.

## What it does

Loads a GGUF file via memory mapping, parses the metadata + tensor index, sets up an f32 forward pass for the Llama architecture (RMSNorm → Q/K/V → RoPE → grouped-query attention with KV cache → SwiGLU FFN → residuals → final RMSNorm + lm_head), and runs autoregressive generation with greedy or temperature/top-k sampling.

## Supported

- Architecture: `general.architecture == "llama"` (Llama 1/2/3-style: RMSNorm, RoPE, SwiGLU, GQA).
- Weight types: `F32`, `F16`, `Q8_0`, `Q4_0`, and `Q6_K`. The last is needed because llama.cpp routinely keeps `output.weight` in Q6_K even when the rest of the model is Q4_0.
- RoPE: configurable via `--rope llama` (adjacent-pair, default — matches files produced by the older `convert.py` permuted-Q/K layout, including TheBloke's TinyLlama GGUFs) or `--rope neox` (paired-half, modern `convert_hf_to_gguf.py`).
- Tokenizer: SentencePiece-BPE with the standard llama.cpp encoding loop (highest-score adjacent merge), including `<0xAB>` byte-fallback.
- Sampling: greedy (temperature=0), or temperature + top-k with a tiny xorshift PRNG.

## Verified

- `TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF` `tinyllama-1.1b-chat-v1.0.Q4_0.gguf` (Q4_0 weights, Q6_K `output.weight`). Output matches llama.cpp byte-for-byte at temperature=0.

## Out of scope

K-quants (Q4_K, Q5_K, Q6_K, …), GPU/SIMD kernels, batch>1, prompt-batching during prefill, beam search, sliding-window attention, MoE, and architectures other than Llama.

## Build & run

```bash
cargo build --release
./target/release/tiny-llm-runner --model path/to/llama.gguf --prompt "Hello"
```

A small Llama-architecture GGUF (e.g. TinyLlama-1.1B-Chat F16 or Q8_0) works well for verification. The bigger the model, the slower — this runner has no SIMD and uses a naive matmul (parallelized by `rayon` over output rows).

CLI options:

```text
-m, --model        Path to a Llama-architecture GGUF file
-p, --prompt       Prompt (default: "Once upon a time")
-n, --n-predict    Tokens to generate (default: 64)
-t, --temperature  0 = greedy (default: 0.8)
    --top-k        Top-K cutoff, 0 disables (default: 40)
    --seed         PRNG seed (default: 42)
    --no-bos       Don't prepend BOS to the prompt
```

## Layout

| File | Purpose |
| --- | --- |
| `src/config.rs` | Read Llama hyperparameters from GGUF metadata. |
| `src/dequant.rs` | Block-wise dequantization + dot kernels for F32/F16/Q8_0/Q4_0. |
| `src/tensor.rs` | `TensorView` over the mmap with `dot_row` / `dequant_row`. |
| `src/model.rs` | Locate every Llama tensor (`token_embd`, `blk.N.*`, `output*`). |
| `src/ops.rs` | RMSNorm, matvec, softmax, RoPE, SiLU, vector add. |
| `src/runner.rs` | Forward pass + KV cache, single-batch. |
| `src/tokenizer.rs` | SP-BPE encode/decode driven by GGUF vocab + scores. |
| `src/sampler.rs` | Greedy / temperature / top-k sampler. |
| `src/main.rs` | CLI: load → encode → prefill → decode → print tok/s. |

## Why this exists

The previous `llm-local-studio-*` projects used `llama-cpp-2` to run inference. This crate is the inverse experiment: take the same GGUF files those bindings consume and run them with no C code. The goal is pedagogical — every line of the forward pass is auditable Rust.

use anyhow::{Context, Result};
use clap::Parser;
use llm_gguf_parser::parse_gguf;
use memmap2::Mmap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use tiny_llm_runner::{LlamaConfig, LlamaModel, RopeStyle, Runner, Sampler, Tokenizer};

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Pure-Rust llama-architecture inference over a GGUF model"
)]
struct Args {
    /// Path to a Llama-architecture GGUF file (F32/F16/Q8_0/Q4_0 weights).
    #[arg(short, long)]
    model: PathBuf,

    /// Prompt to feed before generation.
    #[arg(short, long, default_value = "Once upon a time")]
    prompt: String,

    /// Number of tokens to generate.
    #[arg(short, long, default_value_t = 64)]
    n_predict: usize,

    /// Sampling temperature (0 = greedy).
    #[arg(short, long, default_value_t = 0.8)]
    temperature: f32,

    /// Top-K cutoff (0 = disabled).
    #[arg(long, default_value_t = 40)]
    top_k: usize,

    /// PRNG seed for sampling.
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Suppress BOS prefix on the prompt.
    #[arg(long)]
    no_bos: bool,

    /// RoPE convention: `llama` (adjacent-pair, old `convert.py`) or
    /// `neox` (interleaved-half, modern `convert_hf_to_gguf.py`).
    #[arg(long, default_value = "llama")]
    rope: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let file =
        File::open(&args.model).with_context(|| format!("opening {}", args.model.display()))?;
    let mmap = unsafe { Mmap::map(&file)? };
    let gguf = parse_gguf(&mmap).map_err(|e| anyhow::anyhow!("parsing GGUF: {e}"))?;

    let config = LlamaConfig::from_gguf(&gguf)?;
    eprintln!(
        "[loaded] n_layer={} n_embd={} n_head={} n_head_kv={} n_ff={} vocab={} n_ctx={}",
        config.n_layer,
        config.n_embd,
        config.n_head,
        config.n_head_kv,
        config.n_ff,
        config.vocab_size,
        config.n_ctx,
    );

    let blob = &mmap[gguf.data_offset as usize..];
    let model = LlamaModel::load(config.clone(), &gguf.tensors, blob)?;
    let tokenizer = Tokenizer::from_gguf(&gguf)?;

    let prompt_ids = tokenizer.encode(&args.prompt, !args.no_bos);
    eprintln!("[prompt] {} tokens", prompt_ids.len());

    let rope_style = match args.rope.as_str() {
        "llama" => RopeStyle::Llama,
        "neox" => RopeStyle::Neox,
        other => anyhow::bail!("unknown --rope value {other:?} (expected `llama` or `neox`)"),
    };
    let mut runner = Runner::new(&model, rope_style);
    let mut sampler = Sampler::new(args.temperature, args.top_k, args.seed);

    print!("{}", args.prompt);
    std::io::stdout().flush().ok();

    // Prefill.
    let prefill_start = Instant::now();
    let mut last_logits: Option<Vec<f32>> = None;
    for &tok in &prompt_ids {
        let logits = runner.forward(tok);
        last_logits = Some(logits.to_vec());
    }
    let prefill_elapsed = prefill_start.elapsed();
    eprintln!(
        "\n[prefill] {} tok in {:.2}s ({:.1} tok/s)",
        prompt_ids.len(),
        prefill_elapsed.as_secs_f64(),
        prompt_ids.len() as f64 / prefill_elapsed.as_secs_f64().max(1e-9),
    );

    // Decode.
    let decode_start = Instant::now();
    let mut generated: Vec<u32> = Vec::with_capacity(args.n_predict);
    let mut logits = last_logits.expect("empty prompt");
    for _ in 0..args.n_predict {
        let next = sampler.sample(&mut logits);
        if next == tokenizer.eos {
            break;
        }
        generated.push(next);
        let piece = tokenizer.decode(&[next]);
        print!("{piece}");
        std::io::stdout().flush().ok();
        logits = runner.forward(next).to_vec();
    }
    println!();

    let decode_elapsed = decode_start.elapsed();
    eprintln!(
        "[decode] {} tok in {:.2}s ({:.1} tok/s)",
        generated.len(),
        decode_elapsed.as_secs_f64(),
        generated.len() as f64 / decode_elapsed.as_secs_f64().max(1e-9),
    );

    Ok(())
}

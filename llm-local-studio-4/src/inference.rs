use crate::chat_template::{ChatMessage, ChatTemplate, auto_detect};
use llama_cpp_4::context::params::LlamaContextParams;
use llama_cpp_4::llama_backend::LlamaBackend;
use llama_cpp_4::llama_batch::LlamaBatch;
use llama_cpp_4::model::params::LlamaModelParams;
use llama_cpp_4::model::{AddBos, LlamaModel, Special};
use llama_cpp_4::mtmd::{MtmdBitmap, MtmdContext, MtmdContextParams, MtmdInputChunks, MtmdInputText};
use llama_cpp_4::sampling::LlamaSampler;
use std::io::Write;
use std::num::NonZeroU32;
use std::path::PathBuf;

use anyhow::{Context, Result};

// ---------------------------------------------------------------------------
// Public request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LoadModelRequest {
    pub model_id: String,
    pub path: PathBuf,
    pub context_size: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelHandle {
    pub model_id: String,
}

pub type StreamCallback = Box<dyn Fn(&str) + Send>;

pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub seed: Option<u32>,
    pub stream_callback: Option<StreamCallback>,
}

#[derive(Debug, Clone)]
pub struct LoadedModelInfo {
    pub model_id: String,
    /// Whether the engine has an mmproj loaded and can handle multimodal input.
    pub multimodal_ready: bool,
}

pub struct GenerateRequest {
    pub prompt: String,
    pub max_tokens: u32,
    pub seed: Option<u32>,
    pub stream_callback: Option<StreamCallback>,
}

pub struct GenerateOutput {
    pub text: String,
    pub prompt_tokens: u32,
    pub generated_tokens: u32,
}

/// Request for direct multimodal audio inference via `libmtmd`.
///
/// The `audio_path` must point to a WAV file (16 kHz mono recommended).
pub struct MultimodalRequest {
    /// Natural-language instruction, e.g. "Transcribe this audio."
    pub prompt: String,
    /// Path to the input WAV file.
    pub audio_path: PathBuf,
    /// Per-request override for the mmproj path (usually `None`).
    pub mmproj_path: Option<PathBuf>,
    pub max_tokens: u32,
    pub seed: Option<u32>,
    pub stream_callback: Option<StreamCallback>,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

pub trait InferenceEngine {
    fn load_model(&mut self, request: LoadModelRequest) -> Result<ModelHandle>;
    fn load_mmproj(&mut self, mmproj_path: PathBuf) -> Result<()>;
    fn run(&mut self, request: GenerateRequest) -> Result<GenerateOutput>;
    fn chat(&mut self, request: ChatRequest) -> Result<GenerateOutput>;
    fn run_multimodal(&mut self, request: MultimodalRequest) -> Result<GenerateOutput>;
    fn model_info(&self) -> Option<LoadedModelInfo>;
}

// ---------------------------------------------------------------------------
// Internal session state
// ---------------------------------------------------------------------------

struct LoadedSession {
    backend: LlamaBackend,
    model: Box<LlamaModel>,
    model_id: String,
    chat_template: Box<dyn ChatTemplate>,
}

pub struct LlamaCppEngine {
    session: Option<LoadedSession>,
    loaded_context_params: LlamaContextParams,
    mmproj_path: Option<PathBuf>,
    mtmd_ctx: Option<MtmdContext>,
}

impl LlamaCppEngine {
    pub fn new() -> Self {
        Self {
            session: None,
            loaded_context_params: LlamaContextParams::default(),
            mmproj_path: None,
            mtmd_ctx: None,
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for LlamaCppEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helper: token-sampling loop (shared by text and multimodal paths)
// ---------------------------------------------------------------------------

fn sample_loop(
    session: &LoadedSession,
    context: &mut llama_cpp_4::context::LlamaContext,
    mut batch: LlamaBatch,
    mut position: i32,
    max_tokens: u32,
    seed: Option<u32>,
    stream_callback: &Option<StreamCallback>,
) -> Result<(String, u32)> {
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::dist(seed.unwrap_or(42)),
        LlamaSampler::greedy(),
    ]);
    let mut generated_tokens = 0u32;
    let mut output_text = String::new();

    while generated_tokens < max_tokens {
        let token = sampler.sample(context, batch.n_tokens() - 1);
        sampler.accept(token);

        if session.model.is_eog_token(token) {
            break;
        }

        let piece = session
            .model
            .token_to_str(token, Special::Tokenize)
            .context("failed to decode token")?;

        if let Some(cb) = stream_callback {
            cb(&piece);
        } else {
            print!("{piece}");
            std::io::stdout().flush()?;
        }
        output_text.push_str(&piece);

        batch.clear();
        batch.add(token, position, &[0], true)?;
        context
            .decode(&mut batch)
            .context("failed to evaluate token")?;

        position += 1;
        generated_tokens += 1;
    }

    if stream_callback.is_none() {
        println!();
    }

    Ok((output_text, generated_tokens))
}

// ---------------------------------------------------------------------------
// InferenceEngine impl
// ---------------------------------------------------------------------------

impl InferenceEngine for LlamaCppEngine {
    fn load_model(&mut self, request: LoadModelRequest) -> Result<ModelHandle> {
        let backend = LlamaBackend::init().context("failed to initialize llama.cpp backend")?;
        let model =
            LlamaModel::load_from_file(&backend, &request.path, &LlamaModelParams::default().with_n_gpu_layers(99))
                .with_context(|| format!("failed to load model {}", request.path.display()))?;

        let n_ctx = request
            .context_size
            .and_then(NonZeroU32::new)
            .unwrap_or(NonZeroU32::new(4096).unwrap());
        self.loaded_context_params = LlamaContextParams::default().with_n_ctx(Some(n_ctx));

        let handle = ModelHandle {
            model_id: request.model_id.clone(),
        };
        let chat_template = auto_detect(&request.model_id);
        self.session = Some(LoadedSession {
            backend,
            model: Box::new(model),
            model_id: request.model_id.clone(),
            chat_template,
        });

        // If an mmproj path was pre-configured, initialise the mtmd context now.
        if let Some(mmproj) = self.mmproj_path.clone() {
            if let Err(e) = self.load_mmproj(mmproj) {
                eprintln!("[warn] failed to initialise mtmd context: {e}");
            }
        }

        Ok(handle)
    }

    fn load_mmproj(&mut self, mmproj_path: PathBuf) -> Result<()> {
        self.mmproj_path = Some(mmproj_path.clone());

        let session = self
            .session
            .as_ref()
            .context("load_model must be called before load_mmproj")?;

        let params = MtmdContextParams::default();
        let mtmd_ctx = MtmdContext::init_from_file(&mmproj_path, &session.model, params)
            .with_context(|| {
                format!("failed to load mmproj from {}", mmproj_path.display())
            })?;

        println!(
            "[mtmd] multimodal projector loaded from {}",
            mmproj_path.display()
        );
        self.mtmd_ctx = Some(mtmd_ctx);

        Ok(())
    }

    fn run(&mut self, request: GenerateRequest) -> Result<GenerateOutput> {
        let session = self
            .session
            .as_mut()
            .context("no model loaded — call `load_model` first")?;

        let mut context = session
            .model
            .new_context(&session.backend, self.loaded_context_params.clone())
            .context("failed to create llama.cpp context")?;

        if request.max_tokens == 0 {
            return Ok(GenerateOutput {
                text: String::new(),
                prompt_tokens: 0,
                generated_tokens: 0,
            });
        }

        let prompt_tokens = session
            .model
            .str_to_token(&request.prompt, AddBos::Always)
            .context("failed to tokenize prompt")?;
        if prompt_tokens.is_empty() {
            anyhow::bail!("prompt produced no tokens");
        }

        let requested_tokens = prompt_tokens.len() + request.max_tokens as usize;
        if requested_tokens > context.n_ctx() as usize {
            anyhow::bail!(
                "prompt + max_tokens ({requested_tokens}) exceeds context size ({})",
                context.n_ctx()
            );
        }

        let mut batch = LlamaBatch::new(512, 1);
        let n_prompt_tokens = prompt_tokens.len() as u32;
        let last = (prompt_tokens.len() - 1) as i32;
        for (pos, token) in (0_i32..).zip(prompt_tokens) {
            batch.add(token, pos, &[0], pos == last)?;
        }
        context
            .decode(&mut batch)
            .context("failed to evaluate prompt")?;

        let position = batch.n_tokens();
        let (output_text, generated_tokens) = sample_loop(
            session,
            &mut context,
            batch,
            position,
            request.max_tokens,
            request.seed,
            &request.stream_callback,
        )?;

        Ok(GenerateOutput {
            text: output_text,
            prompt_tokens: n_prompt_tokens,
            generated_tokens,
        })
    }

    fn chat(&mut self, request: ChatRequest) -> Result<GenerateOutput> {
        let prompt = {
            let session = self
                .session
                .as_ref()
                .context("no model loaded — call `load_model` first")?;
            session.chat_template.apply(&request.messages)
        };

        self.run(GenerateRequest {
            prompt,
            max_tokens: request.max_tokens,
            seed: request.seed,
            stream_callback: request.stream_callback,
        })
    }

    fn run_multimodal(&mut self, request: MultimodalRequest) -> Result<GenerateOutput> {
        // Resolve the mmproj to use — either the per-request override or the
        // one we loaded at start-up.
        let mmproj_path = request
            .mmproj_path
            .clone()
            .or_else(|| self.mmproj_path.clone())
            .context("no mmproj loaded — start the server with --mmproj <path>")?;

        // If mtmd_ctx is not yet initialised, load it now.
        if self.mtmd_ctx.is_none() {
            self.load_mmproj(mmproj_path)?;
        }

        let session = self
            .session
            .as_mut()
            .context("no model loaded — call `load_model` first")?;

        let mtmd_ctx = self
            .mtmd_ctx
            .as_ref()
            .context("mtmd context not available")?;

        // Build the text prompt with the default media marker placeholder.
        let marker = MtmdContext::default_marker();
        let full_prompt = format!("{} {}", request.prompt, marker);

        // Encode the audio file into an MtmdBitmap.
        let bitmap = MtmdBitmap::from_file(mtmd_ctx, &request.audio_path).with_context(|| {
            format!("failed to load audio from {}", request.audio_path.display())
        })?;

        // Tokenise: split the text+marker prompt into chunks, substituting
        // the media marker with audio token embeddings.
        let text = MtmdInputText::new(&full_prompt, true, true);
        let bitmaps = [&bitmap];
        let mut chunks = MtmdInputChunks::new();
        mtmd_ctx
            .tokenize(&text, &bitmaps, &mut chunks)
            .context("mtmd tokenize failed")?;

        // Create a fresh inference context.
        let mut lctx = session
            .model
            .new_context(&session.backend, self.loaded_context_params.clone())
            .context("failed to create llama.cpp context")?;

        // Evaluate all chunks (text + audio embeddings) in one call.
        let n_batch = lctx.n_batch() as i32;
        let mut n_past = 0i32;
        mtmd_ctx
            .eval_chunks(lctx.as_ptr(), &chunks, 0, 0, n_batch, true, &mut n_past)
            .context("mtmd eval_chunks failed")?;

        let n_prompt_tokens = n_past as u32;

        // Sample generation tokens in the standard loop.
        let batch = LlamaBatch::new(512, 1);
        let position = n_past;
        let (output_text, generated_tokens) = sample_loop(
            session,
            &mut lctx,
            batch,
            position,
            request.max_tokens,
            request.seed,
            &request.stream_callback,
        )?;

        Ok(GenerateOutput {
            text: output_text,
            prompt_tokens: n_prompt_tokens,
            generated_tokens,
        })
    }

    fn model_info(&self) -> Option<LoadedModelInfo> {
        self.session.as_ref().map(|s| LoadedModelInfo {
            model_id: s.model_id.clone(),
            multimodal_ready: self.mtmd_ctx.is_some(),
        })
    }
}

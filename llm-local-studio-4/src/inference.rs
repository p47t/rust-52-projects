use crate::chat_template::{ChatMessage, ChatTemplate, auto_detect};
use encoding_rs::UTF_8;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use std::io::Write;
use std::num::NonZeroU32;
use std::path::PathBuf;

use anyhow::{Context, Result};

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

pub trait InferenceEngine {
    fn load_model(&mut self, request: LoadModelRequest) -> Result<ModelHandle>;
    fn run(&mut self, request: GenerateRequest) -> Result<GenerateOutput>;
    fn chat(&mut self, request: ChatRequest) -> Result<GenerateOutput>;
    fn model_info(&self) -> Option<LoadedModelInfo>;
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

struct LoadedSession {
    backend: LlamaBackend,
    model: Box<LlamaModel>,
    model_id: String,
    chat_template: Box<dyn ChatTemplate>,
}

pub struct LlamaCppEngine {
    session: Option<LoadedSession>,
    /// Saved from load_model so run() can recreate the context each call.
    loaded_context_params: LlamaContextParams,
}

impl LlamaCppEngine {
    pub fn new() -> Self {
        Self {
            session: None,
            loaded_context_params: LlamaContextParams::default(),
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for LlamaCppEngine {
    fn default() -> Self {
        Self {
            session: None,
            loaded_context_params: LlamaContextParams::default(),
        }
    }
}

impl InferenceEngine for LlamaCppEngine {
    fn load_model(&mut self, request: LoadModelRequest) -> Result<ModelHandle> {
        let backend = LlamaBackend::init().context("failed to initialize llama.cpp backend")?;
        let model =
            LlamaModel::load_from_file(&backend, &request.path, &LlamaModelParams::default())
                .with_context(|| format!("failed to load model {}", request.path.display()))?;

        let n_ctx = request
            .context_size
            .and_then(NonZeroU32::new)
            .unwrap_or(NonZeroU32::new(2048).unwrap());
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
        Ok(handle)
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

        let mut decoder = UTF_8.new_decoder();
        let mut batch = LlamaBatch::new(512, 1);
        let n_prompt_tokens = prompt_tokens.len() as u32;
        let last = (prompt_tokens.len() - 1) as i32;
        for (pos, token) in (0_i32..).zip(prompt_tokens) {
            batch.add(token, pos, &[0], pos == last)?;
        }
        context
            .decode(&mut batch)
            .context("failed to evaluate prompt")?;

        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::dist(request.seed.unwrap_or(42)),
            LlamaSampler::greedy(),
        ]);
        let mut position = batch.n_tokens();
        let mut generated_tokens = 0u32;
        let mut output_text = String::new();

        while generated_tokens < request.max_tokens {
            let token = sampler.sample(&context, batch.n_tokens() - 1);
            sampler.accept(token);

            if session.model.is_eog_token(token) {
                break;
            }

            let piece = session
                .model
                .token_to_piece(token, &mut decoder, true, None)
                .context("failed to decode token")?;

            if let Some(ref cb) = request.stream_callback {
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

        if request.stream_callback.is_none() {
            println!();
        }

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

    fn model_info(&self) -> Option<LoadedModelInfo> {
        self.session.as_ref().map(|s| LoadedModelInfo {
            model_id: s.model_id.clone(),
        })
    }
}

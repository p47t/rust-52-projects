use anyhow::{Context, Result};
use llama_cpp_4::context::params::LlamaContextParams;
use llama_cpp_4::llama_backend::LlamaBackend;
use llama_cpp_4::llama_batch::LlamaBatch;
use llama_cpp_4::model::params::LlamaModelParams;
use llama_cpp_4::model::{LlamaModel, Special};
use llama_cpp_4::mtmd::{MtmdBitmap, MtmdContext, MtmdContextParams, MtmdInputChunks, MtmdInputText};
use llama_cpp_4::sampling::LlamaSampler;
use std::num::NonZeroU32;
use std::path::Path;


pub struct LoadedSession {
    pub backend: LlamaBackend,
    pub model: LlamaModel,
}

pub struct GemmaVoiceEngine {
    session: LoadedSession,
    loaded_context_params: LlamaContextParams,
    mtmd_ctx: MtmdContext,
}

impl GemmaVoiceEngine {
    pub fn load(model_path: &Path, mmproj_path: &Path, context_size: u32) -> Result<Self> {
        let backend = LlamaBackend::init().context("failed to initialize llama.cpp backend")?;
        
        let mut model_params = LlamaModelParams::default();
        // Request Vulkan or GPU offload using 99 layers (all of them)
        model_params = model_params.with_n_gpu_layers(99);

        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .with_context(|| format!("failed to load model from {}", model_path.display()))?;

        let n_ctx = NonZeroU32::new(context_size)
            .context("context size must be non-zero")?;
        let loaded_context_params = LlamaContextParams::default().with_n_ctx(Some(n_ctx));

        let mtmd_params = MtmdContextParams::default();
        let mtmd_ctx = MtmdContext::init_from_file(mmproj_path, &model, mtmd_params)
            .with_context(|| format!("failed to load mmproj from {}", mmproj_path.display()))?;

        Ok(Self {
            session: LoadedSession { backend, model },
            loaded_context_params,
            mtmd_ctx,
        })
    }

    pub fn run_multimodal(
        &mut self,
        prompt: &str,
        audio_path: &Path,
        max_tokens: u32,
        seed: Option<u32>,
        mut stream_callback: impl FnMut(&str) + Send,
    ) -> Result<String> {
        let marker = MtmdContext::default_marker();
        let full_prompt = format!("{} {}", prompt, marker);

        // Encode the audio file into an MtmdBitmap
        let bitmap = MtmdBitmap::from_file(&self.mtmd_ctx, audio_path)
            .with_context(|| format!("failed to load audio from {}", audio_path.display()))?;

        // Tokenise: split the text+marker prompt into chunks, substituting the media marker
        let text = MtmdInputText::new(&full_prompt, true, true);
        let bitmaps = [&bitmap];
        let mut chunks = MtmdInputChunks::new();
        self.mtmd_ctx
            .tokenize(&text, &bitmaps, &mut chunks)
            .context("mtmd tokenize failed")?;

        // Create context
        let mut lctx = self.session.model
            .new_context(&self.session.backend, self.loaded_context_params.clone())
            .context("failed to create llama.cpp context")?;

        // Evaluate all chunks in one call
        let n_batch = lctx.n_batch() as i32;
        let mut n_past = 0i32;
        self.mtmd_ctx
            .eval_chunks(lctx.as_ptr(), &chunks, 0, 0, n_batch, true, &mut n_past)
            .context("mtmd eval_chunks failed")?;

        let mut batch = LlamaBatch::new(512, 1);
        let mut position = n_past;

        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::dist(seed.unwrap_or(42)),
            LlamaSampler::greedy(),
        ]);
        let mut generated_tokens = 0u32;
        let mut output_text = String::new();

        while generated_tokens < max_tokens {
            let token = sampler.sample(&lctx, batch.n_tokens() - 1);
            sampler.accept(token);

            if self.session.model.is_eog_token(token) {
                break;
            }

            let piece = self.session.model
                .token_to_str(token, Special::Tokenize)
                .context("failed to decode token")?;

            stream_callback(&piece);
            output_text.push_str(&piece);

            batch.clear();
            batch.add(token, position, &[0], true)?;
            lctx.decode(&mut batch)
                .context("failed to evaluate token")?;

            position += 1;
            generated_tokens += 1;
        }

        Ok(output_text)
    }
}

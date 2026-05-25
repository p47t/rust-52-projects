use std::path::Path;
use anyhow::{Result, Context};
use candle::{Device, Tensor, IndexOp};
use candle_nn::VarBuilder;
use tokenizers::Tokenizer;
use candle_transformers::models::whisper::{self as m, audio, Config};

pub enum WhisperModel {
    Normal(m::model::Whisper),
    Quantized(m::quantized_model::Whisper),
}

impl WhisperModel {
    pub fn config(&self) -> &Config {
        match self {
            Self::Normal(m) => &m.config,
            Self::Quantized(m) => &m.config,
        }
    }

    pub fn encoder_forward(&mut self, x: &Tensor, flush: bool) -> candle::Result<Tensor> {
        match self {
            Self::Normal(m) => m.encoder.forward(x, flush),
            Self::Quantized(m) => m.encoder.forward(x, flush),
        }
    }

    pub fn decoder_forward(&mut self, x: &Tensor, xa: &Tensor, flush: bool) -> candle::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.forward(x, xa, flush),
            Self::Quantized(m) => m.decoder.forward(x, xa, flush),
        }
    }

    pub fn decoder_final_linear(&self, x: &Tensor) -> candle::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.final_linear(x),
            Self::Quantized(m) => m.decoder.final_linear(x),
        }
    }
}

pub struct WhisperEngine {
    model: WhisperModel,
    tokenizer: Tokenizer,
    mel_filters: Vec<f32>,
    device: Device,
}

impl WhisperEngine {
    pub fn load(model_path_or_id: &str) -> Result<Self> {
        let device = Device::Cpu;

        let (config_path, tokenizer_path, weights_path) = if Path::new(model_path_or_id).is_dir() {
            let dir = Path::new(model_path_or_id);
            (
                dir.join("config.json"),
                dir.join("tokenizer.json"),
                dir.join("model.safetensors"),
            )
        } else {
            println!("Resolving and downloading Whisper model '{}' from Hugging Face...", model_path_or_id);
            let api = hf_hub::api::sync::Api::new()?;
            let repo = api.repo(hf_hub::Repo::new(model_path_or_id.to_string(), hf_hub::RepoType::Model));
            (
                repo.get("config.json").context("Failed to download config.json")?,
                repo.get("tokenizer.json").context("Failed to download tokenizer.json")?,
                repo.get("model.safetensors").context("Failed to download model.safetensors")?,
            )
        };

        let config: Config = serde_json::from_str(&std::fs::read_to_string(&config_path)?)
            .context("Failed to parse config.json")?;
        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(anyhow::Error::msg)
            .context("Failed to load tokenizer.json")?;

        let mel_bytes = match config.num_mel_bins {
            80 => include_bytes!("melfilters.bytes").as_slice(),
            128 => include_bytes!("melfilters128.bytes").as_slice(),
            nmel => anyhow::bail!("unexpected num_mel_bins {nmel}"),
        };
        let mut mel_filters = vec![0f32; mel_bytes.len() / 4];
        for (i, chunk) in mel_bytes.chunks_exact(4).enumerate() {
            mel_filters[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }

        let is_quantized = weights_path.extension().and_then(|ext| ext.to_str()) == Some("gguf");
        let model = if is_quantized {
            let vb = candle_transformers::quantized_var_builder::VarBuilder::from_gguf(&weights_path, &device)
                .context("Failed to load quantized GGUF weights")?;
            WhisperModel::Quantized(m::quantized_model::Whisper::load(&vb, config)?)
        } else {
            let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], m::DTYPE, &device) }
                .context("Failed to map safetensors weights")?;
            WhisperModel::Normal(m::model::Whisper::load(&vb, config)?)
        };

        Ok(Self {
            model,
            tokenizer,
            mel_filters,
            device,
        })
    }

    pub fn transcribe(&mut self, pcm_data: &[f32], is_translation: bool, language: Option<&str>) -> Result<String> {
        let (num_mel_bins, max_target_positions, vocab_size) = {
            let config = self.model.config();
            (config.num_mel_bins, config.max_target_positions, config.vocab_size)
        };
        let mel = audio::pcm_to_mel(self.model.config(), pcm_data, &self.mel_filters);
        let mel_len = mel.len();
        let mel = Tensor::from_vec(
            mel,
            (1, num_mel_bins, mel_len / num_mel_bins),
            &self.device,
        )?;

        let sot_token = token_id(&self.tokenizer, m::SOT_TOKEN)?;
        let transcribe_token = token_id(&self.tokenizer, m::TRANSCRIBE_TOKEN)?;
        let translate_token = token_id(&self.tokenizer, m::TRANSLATE_TOKEN)?;
        let eot_token = token_id(&self.tokenizer, m::EOT_TOKEN)?;
        let no_timestamps_token = token_id(&self.tokenizer, m::NO_TIMESTAMPS_TOKEN)?;

        let is_multilingual = vocab_size > 51864;
        let mut language_token = None;
        if is_multilingual {
            if let Some(lang) = language {
                if lang == "auto" {
                    let seq_len = mel.dim(2)?;
                    let first_segment = mel.narrow(2, 0, usize::min(seq_len, m::N_FRAMES))?;
                    language_token = Some(self.detect_language(&first_segment)?);
                } else {
                    language_token = token_id(&self.tokenizer, &format!("<|{lang}|>")).ok();
                }
            }
        }

        let (_, _, content_frames) = mel.dims3()?;
        let mut seek = 0;
        let mut full_text = String::new();

        while seek < content_frames {
            let segment_size = usize::min(content_frames - seek, m::N_FRAMES);
            let mel_segment = mel.narrow(2, seek, segment_size)?;
            seek += segment_size;

            let mut tokens = vec![sot_token];
            if let Some(lang_tok) = language_token {
                tokens.push(lang_tok);
            }
            if is_translation {
                tokens.push(translate_token);
            } else {
                tokens.push(transcribe_token);
            }
            tokens.push(no_timestamps_token);

            let audio_features = self.model.encoder_forward(&mel_segment, true)?;
            let sample_len = max_target_positions / 2;

            for i in 0..sample_len {
                let tokens_t = Tensor::new(tokens.as_slice(), &self.device)?;
                let tokens_t = tokens_t.unsqueeze(0)?;
                let ys = self.model.decoder_forward(&tokens_t, &audio_features, i == 0)?;

                let (_, seq_len, _) = ys.dims3()?;
                let logits = self.model.decoder_final_linear(&ys.i((..1, seq_len - 1..))?)?.i(0)?.i(0)?;

                let logits_v: Vec<f32> = logits.to_vec1()?;
                let next_token = logits_v
                    .iter()
                    .enumerate()
                    .max_by(|(_, u), (_, v)| u.total_cmp(v))
                    .map(|(i, _)| i as u32)
                    .unwrap();

                tokens.push(next_token);
                if next_token == eot_token {
                    break;
                }
            }

            let text = self.tokenizer.decode(&tokens, true).map_err(anyhow::Error::msg)?;
            if !text.is_empty() {
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(text.trim());
            }
        }

        Ok(full_text)
    }

    pub fn detect_language(&mut self, mel: &Tensor) -> Result<u32> {
        let sot_token = token_id(&self.tokenizer, m::SOT_TOKEN)?;
        let audio_features = self.model.encoder_forward(mel, true)?;
        
        let language_tokens: Vec<u32> = crate::multilingual::LANGUAGES
            .iter()
            .filter_map(|(lang, _)| {
                token_id(&self.tokenizer, &format!("<|{lang}|>")).ok()
            })
            .collect();
        
        let tokens_t = Tensor::new(&[[sot_token]], &self.device)?;
        let ys = self.model.decoder_forward(&tokens_t, &audio_features, true)?;
        let logits = self.model.decoder_final_linear(&ys.i(..1)?)?.i(0)?.i(0)?;
        
        let mut best_lang_token = None;
        let mut max_logit = f32::NEG_INFINITY;
        
        let logits_v: Vec<f32> = logits.to_vec1()?;
        for tok_id in language_tokens {
            if let Some(&logit) = logits_v.get(tok_id as usize) {
                if logit > max_logit {
                    max_logit = logit;
                    best_lang_token = Some(tok_id);
                }
            }
        }
        
        best_lang_token.ok_or_else(|| anyhow::anyhow!("Failed to detect language"))
    }
}

pub fn token_id(tokenizer: &Tokenizer, token: &str) -> Result<u32> {
    match tokenizer.token_to_id(token) {
        None => anyhow::bail!("no token-id for {token}"),
        Some(id) => Ok(id),
    }
}

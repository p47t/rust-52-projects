use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::api::{ServerConfig, start_server};
use crate::engine_service::EngineService;
use crate::hf::{DownloadRequest, HuggingFaceClient, HuggingFaceModel};
use crate::inference::{GenerateRequest, InferenceEngine, LlamaCppEngine, LoadModelRequest};
use crate::registry::{ModelLoadStatus, ModelRecord, ModelRegistry, ModelSource};

#[derive(Debug, Parser)]
#[command(author, version, about = "A learning-focused local LLM app shell")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        self.command.run().await
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Scan a local folder recursively for GGUF files.
    Scan {
        /// Folder containing local models.
        #[arg(default_value = "models")]
        dir: PathBuf,
    },
    /// Search Hugging Face for GGUF models.
    HfSearch {
        /// Search query, for example "tinyllama gguf".
        query: String,
        /// Maximum number of models to return.
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    /// Download a GGUF model from Hugging Face by repository ID or search name.
    HfDownload {
        /// Exact repository ID like owner/repo, or a search name like tinyllama.
        name: String,
        /// Specific GGUF filename to download. If omitted, a common quant is selected.
        #[arg(short, long)]
        filename: Option<String>,
        /// Root folder for downloaded models.
        #[arg(short, long, default_value = "models")]
        dir: PathBuf,
        /// Print the resolved download URL without downloading the file.
        #[arg(long)]
        print_url: bool,
        /// Overwrite an existing file.
        #[arg(long)]
        force: bool,
    },
    /// Run a local GGUF model in-process with llama.cpp.
    Run {
        /// Local GGUF path, model ID, or a unique part of the downloaded filename.
        model: String,
        /// Prompt text to send to the model.
        #[arg(short, long)]
        prompt: String,
        /// Root folder used when resolving downloaded model names.
        #[arg(short, long, default_value = "models")]
        dir: PathBuf,
        /// Context size passed to llama-cli.
        #[arg(long, default_value_t = 2048)]
        ctx_size: u32,
        /// Number of tokens to predict.
        #[arg(short = 'n', long, default_value_t = 128)]
        max_tokens: u32,
    },
    /// Start an OpenAI-compatible HTTP API server.
    Serve {
        /// Local GGUF path, model ID, or Hugging Face repository ID.
        model: String,
        /// Root folder used when resolving downloaded model names.
        #[arg(short, long, default_value = "models")]
        dir: PathBuf,
        /// Context size for the model.
        #[arg(long, default_value_t = 2048)]
        ctx_size: u32,
        /// Host address to bind the server to.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port to bind the server to.
        #[arg(long, default_value_t = 8080)]
        port: u16,
        /// Path to Whisper model directory or HF Repo ID for ASR and AST support.
        #[arg(long)]
        whisper_model: Option<String>,
    },
}

impl Command {
    async fn run(self) -> Result<()> {
        match self {
            Self::Scan { dir } => {
                tokio::task::spawn_blocking(move || scan_models(dir)).await??;
                Ok(())
            }
            Self::HfSearch { query, limit } => {
                tokio::task::spawn_blocking(move || search_hugging_face(query, limit)).await??;
                Ok(())
            }
            Self::HfDownload {
                name,
                filename,
                dir,
                print_url,
                force,
            } => {
                tokio::task::spawn_blocking(move || {
                    download_hugging_face_model(DownloadRequest {
                        name,
                        filename,
                        models_dir: dir,
                        print_url,
                        force,
                    })
                })
                .await??;
                Ok(())
            }
            Self::Run {
                model,
                prompt,
                dir,
                ctx_size,
                max_tokens,
            } => {
                tokio::task::spawn_blocking(move || {
                    let model_record = resolve_model(&model, dir)?;
                    let mut engine = LlamaCppEngine::new();
                    engine.load_model(LoadModelRequest {
                        model_id: model_record.id.clone(),
                        path: model_record.path.clone(),
                        context_size: Some(ctx_size),
                    })?;
                    engine.run(GenerateRequest {
                        prompt,
                        max_tokens,
                        seed: None,
                        stream_callback: None,
                    })?;
                    Ok::<(), anyhow::Error>(())
                })
                .await??;
                Ok(())
            }
            Self::Serve {
                model,
                dir,
                ctx_size,
                host,
                port,
                whisper_model,
            } => serve_model(model, dir, ctx_size, host, port, whisper_model).await,
        }
    }
}

fn scan_models(dir: PathBuf) -> Result<()> {
    let registry = ModelRegistry::scan_dir(&dir)?;

    if registry.models().is_empty() {
        println!("No GGUF models found under {}", dir.display());
        return Ok(());
    }

    for model in registry.models() {
        println!("{model}");
    }

    Ok(())
}

fn search_hugging_face(query: String, limit: usize) -> Result<()> {
    let client = HuggingFaceClient::default();
    let models = client.search_models(&query, limit)?;

    if models.is_empty() {
        println!("No GGUF models found.");
        return Ok(());
    }

    for model in models {
        println!("{model}");
    }

    Ok(())
}

fn download_hugging_face_model(request: DownloadRequest) -> Result<()> {
    let client = HuggingFaceClient::default();
    let plan = client.plan_download(&request)?;

    println!("model: {}", plan.model.id());
    println!("file: {}", plan.file.rfilename);
    if let Some(size) = plan.file.size {
        println!("size: {size} bytes");
    }
    println!("url: {}", plan.url);

    if request.print_url {
        return Ok(());
    }

    let downloaded = client.download_gguf(&plan, request.force)?;
    println!(
        "downloaded: {} bytes\npath: {}",
        downloaded.bytes_written,
        downloaded.path.display()
    );

    Ok(())
}

fn resolve_model(model: &str, dir: PathBuf) -> Result<ModelRecord> {
    let direct_path = PathBuf::from(model);
    if direct_path.try_exists()? {
        let metadata = fs::metadata(&direct_path)?;
        return Ok(ModelRecord {
            id: direct_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_owned(),
            path: direct_path,
            size_bytes: metadata.len(),
            source: ModelSource::Local,
            status: ModelLoadStatus::Available,
        });
    }
    let registry = ModelRegistry::scan_dir(&dir)?;
    Ok(registry.find(model)?.clone())
}

async fn serve_model(
    model: String,
    dir: PathBuf,
    ctx_size: u32,
    host: String,
    port: u16,
    whisper_model: Option<String>,
) -> Result<()> {
    let engine = EngineService::new();

    let model_record = match resolve_model(&model, dir) {
        Ok(rec) => {
            println!("Loading model: {}", rec.id);
            println!("  path: {}", rec.path.display());
            println!("  size: {} bytes", rec.size_bytes);
            LoadModelRequest {
                model_id: rec.id,
                path: rec.path,
                context_size: Some(ctx_size),
            }
        }
        Err(_) => {
            // Treat model as a HF repository ID directly
            println!("Model path not resolved locally, treating as Hugging Face Repo ID: {}", model);
            LoadModelRequest {
                model_id: model.clone(),
                path: PathBuf::from(&model),
                context_size: Some(ctx_size),
            }
        }
    };

    if let Some(ref path) = whisper_model {
        println!("Using Whisper model: {}", path);
    }
    println!();

    engine.load_model(model_record).await?;

    let config = ServerConfig {
        host,
        port,
        whisper_model,
    };
    start_server(config, engine).await
}

impl std::fmt::Display for HuggingFaceModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let downloads = display_optional_count(self.downloads);
        let likes = display_optional_count(self.likes);

        write!(
            f,
            "{}\n  downloads: {}\n  likes: {}\n  url: https://huggingface.co/{}",
            self.id, downloads, likes, self.id
        )
    }
}

impl std::fmt::Display for ModelRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\n  path: {}\n  size: {} bytes\n  status: {:?}",
            self.id,
            self.path.display(),
            self.size_bytes,
            self.status
        )
    }
}

fn display_optional_count(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub mod assets;
pub mod routes;
pub mod types;

use std::sync::Arc;
use anyhow::{Context, Result};
use axum::{
    Router,
    routing::{get, post},
};
use tower_http::cors::{Any, CorsLayer};
use tokio::sync::Mutex as TokioMutex;

use crate::engine_service::EngineService;
use crate::whisper_engine::WhisperEngine;

/// Configuration for the HTTP API server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub whisper_model: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            whisper_model: None,
        }
    }
}

/// Shared application state for Axum route handlers.
#[derive(Clone)]
pub struct AppState {
    pub engine: EngineService,
    pub whisper_engine: Option<Arc<TokioMutex<WhisperEngine>>>,
}

/// Start the OpenAI-compatible HTTP server.
///
/// This function blocks until the server is shut down (e.g., via Ctrl+C).
pub async fn start_server(config: ServerConfig, engine: EngineService) -> Result<()> {
    let bind_addr = format!("{}:{}", config.host, config.port);

    // Initialize WhisperEngine if a model path or repo ID is provided
    let whisper_engine = if let Some(ref model_path_or_id) = config.whisper_model {
        println!("Loading Candle Whisper model from {}...", model_path_or_id);
        let engine = WhisperEngine::load(model_path_or_id)
            .context("Failed to load Candle Whisper model")?;
        Some(Arc::new(TokioMutex::new(engine)))
    } else {
        None
    };

    let state = AppState {
        engine,
        whisper_engine,
    };

    println!("Starting llm-local-studio server (axum)...");
    println!("  Base URL: http://{bind_addr}");
    println!("  Endpoints:");
    println!("    GET  /health");
    println!("    GET  /v1/models");
    println!("    POST /v1/chat/completions");
    if state.whisper_engine.is_some() {
        println!("    POST /v1/audio/transcriptions");
        println!("    POST /v1/audio/translations");
    }
    println!("    GET  / (Web UI)");
    println!();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(routes::health))
        .route("/v1/models", get(routes::list_models))
        .route("/v1/chat/completions", post(routes::chat_completions))
        .route("/v1/audio/transcriptions", post(routes::audio_transcriptions))
        .route("/v1/audio/translations", post(routes::audio_translations))
        .fallback(get(assets::static_handler))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("failed to bind HTTP server to {bind_addr}"))?;

    axum::serve(listener, app)
        .await
        .context("HTTP server error")
}

//! OpenAI-compatible HTTP API server.

pub mod routes;
pub mod types;

use anyhow::{Context, Result};
use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::engine_service::EngineService;

/// Configuration for the HTTP API server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

/// Start the OpenAI-compatible HTTP server.
///
/// This function blocks until the server is shut down (e.g., via Ctrl+C).
pub async fn start_server(config: ServerConfig, engine: EngineService) -> Result<()> {
    let bind_addr = format!("{}:{}", config.host, config.port);

    println!("Starting llm-local-studio server (axum)...");
    println!("  Base URL: http://{bind_addr}");
    println!("  Endpoints:");
    println!("    GET  /health");
    println!("    GET  /v1/models");
    println!("    POST /v1/chat/completions");
    println!();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(routes::health))
        .route("/v1/models", get(routes::list_models))
        .route("/v1/chat/completions", post(routes::chat_completions))
        .layer(cors)
        .with_state(engine);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("failed to bind HTTP server to {bind_addr}"))?;

    axum::serve(listener, app)
        .await
        .context("HTTP server error")
}

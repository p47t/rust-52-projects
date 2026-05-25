//! Async-safe wrapper around [`LlamaCppEngine`] for use with the HTTP server.
//!
//! All inference calls are dispatched onto a blocking thread via
//! [`tokio::task::spawn_blocking`] so that the async HTTP server is never
//! blocked. A standard [`std::sync::Mutex`] guards the engine because the lock
//! is only held inside blocking tasks — never across `.await` points.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use crate::chat_template::ChatMessage;
use crate::inference::{
    ChatRequest, GenerateOutput, InferenceEngine, LlamaCppEngine, LoadModelRequest,
    LoadedModelInfo, ModelHandle,
};

/// Thread-safe, async-friendly inference service.
///
/// Cloning is cheap — all clones share the same underlying engine.
#[derive(Clone)]
pub struct EngineService {
    inner: Arc<Mutex<LlamaCppEngine>>,
}

impl EngineService {
    /// Create a new service wrapping a fresh [`LlamaCppEngine`].
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LlamaCppEngine::new())),
        }
    }

    /// Load a model. Blocks the calling async task briefly (runs on a blocking
    /// thread).
    pub async fn load_model(&self, request: LoadModelRequest) -> Result<ModelHandle> {
        let engine = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut engine = engine.lock().expect("engine mutex poisoned");
            engine.load_model(request)
        })
        .await
        .context("load_model task panicked")?
    }

    /// Return metadata about the currently loaded model, if any.
    pub fn model_info(&self) -> Option<LoadedModelInfo> {
        let engine = self.inner.lock().expect("engine mutex poisoned");
        engine.model_info()
    }

    /// Run a non-streaming chat completion.
    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        seed: Option<u32>,
    ) -> Result<GenerateOutput> {
        let engine = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut engine = engine.lock().expect("engine mutex poisoned");
            engine.chat(ChatRequest {
                messages,
                max_tokens,
                seed,
                stream_callback: None,
            })
        })
        .await
        .context("chat task panicked")?
    }

    /// Run a streaming chat completion. Returns a receiver that yields token
    /// strings as they are generated.
    ///
    /// The final message on the channel will be `None`, indicating that
    /// generation has finished. After that, the returned `GenerateOutput`
    /// future resolves.
    pub fn chat_streaming(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        seed: Option<u32>,
    ) -> (
        mpsc::Receiver<String>,
        tokio::task::JoinHandle<Result<GenerateOutput>>,
    ) {
        let (tx, rx) = mpsc::channel::<String>(64);
        let engine = self.inner.clone();

        let handle = tokio::task::spawn_blocking(move || {
            let mut engine = engine.lock().expect("engine mutex poisoned");
            let tx_clone = tx.clone();
            let result = engine.chat(ChatRequest {
                messages,
                max_tokens,
                seed,
                stream_callback: Some(Box::new(move |piece: &str| {
                    // If the receiver is dropped (client disconnected), we
                    // silently ignore the send error. Inference will still run
                    // to completion, but tokens are discarded.
                    let _ = tx_clone.blocking_send(piece.to_owned());
                })),
            });
            drop(tx); // close the channel so the receiver sees completion
            result
        });

        (rx, handle)
    }
}

impl Default for EngineService {
    fn default() -> Self {
        Self::new()
    }
}

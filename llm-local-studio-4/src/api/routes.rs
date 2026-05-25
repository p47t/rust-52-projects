//! HTTP route handlers for the OpenAI-compatible API.

use std::io::Write;
use std::process::{Command, Stdio};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{
    IntoResponse, Response,
    sse::{Event, Sse},
};
use base64::Engine as _;
use futures::stream::{self, StreamExt};
use tokio_stream::wrappers::ReceiverStream;

use crate::api::AppState;
use crate::api::types::{
    ChatChoice, ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse,
    ChatResponseMessage, ChunkChoice, ChunkDelta, ErrorBody, ErrorResponse,
    ModelListResponse, ModelObject, Usage, generate_completion_id, unix_timestamp,
};
use crate::chat_template::Role;
use crate::engine_service::EngineService;
use crate::inference::MultimodalRequest;

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// GET /health
pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

/// GET /v1/models
pub async fn list_models(State(state): State<AppState>) -> Response {
    let models = match state.engine.model_info() {
        Some(info) => vec![ModelObject {
            id: info.model_id,
            object: "model".to_string(),
            created: unix_timestamp(),
            owned_by: "local".to_string(),
        }],
        None => vec![],
    };

    Json(ModelListResponse {
        object: "list".to_string(),
        data: models,
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// Chat Completions
// ---------------------------------------------------------------------------

/// POST /v1/chat/completions
pub async fn chat_completions(
    State(state): State<AppState>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    let max_tokens = request.max_tokens.unwrap_or(512);
    let seed = request.seed;
    let streaming = request.stream.unwrap_or(false);

    let model_id = state
        .engine
        .model_info()
        .map(|info| info.model_id)
        .unwrap_or_else(|| request.model.clone());

    // Check whether any message contains an audio part (scanning from latest to oldest).
    let audio_part = request.messages.iter().rev().find_map(|msg| {
        msg.content.first_audio().map(|input_audio| {
            // Collect the text prompt from all messages
            let prompt = request
                .messages
                .iter()
                .map(|m| m.content.text_only())
                .collect::<Vec<_>>()
                .join("\n");
            (input_audio.data.clone(), input_audio.format.clone(), prompt)
        })
    });

    if let Some((audio_b64, audio_format, prompt)) = audio_part {
        return handle_multimodal(state.engine, audio_b64, audio_format, prompt, max_tokens, seed, model_id, streaming).await;
    }

    // Text-only path
    let messages: Vec<crate::chat_template::ChatMessage> = request
        .messages
        .iter()
        .map(|m| m.to_chat_message())
        .collect();

    if streaming {
        handle_streaming(state.engine, messages, max_tokens, seed, model_id).await
    } else {
        handle_non_streaming(state.engine, messages, max_tokens, seed, model_id).await
    }
}

// ---------------------------------------------------------------------------
// Multimodal handler
// ---------------------------------------------------------------------------

async fn handle_multimodal(
    engine: EngineService,
    audio_b64: String,
    audio_format: String,
    prompt: String,
    max_tokens: u32,
    seed: Option<u32>,
    model_id: String,
    streaming: bool,
) -> Response {
    // Check whether the engine has a multimodal projector loaded.
    let multimodal_ready = engine
        .model_info()
        .map(|i| i.multimodal_ready)
        .unwrap_or(false);

    if !multimodal_ready {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Multimodal projector not loaded. \
             Start the server with --mmproj <path/to/mmproj.gguf> to enable direct audio inference.",
        );
    }

    // Decode base64 → raw bytes.
    let audio_bytes = match base64::engine::general_purpose::STANDARD.decode(&audio_b64) {
        Ok(b) => b,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to decode base64 audio: {e}"),
            );
        }
    };

    // Convert to WAV (16 kHz mono) via ffmpeg → temp file on disk.
    let wav_path = match audio_bytes_to_wav(&audio_bytes, &audio_format) {
        Ok(p) => p,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Audio conversion failed: {e}"),
            );
        }
    };

    if streaming {
        let (rx, result_handle) = engine.multimodal_streaming(MultimodalRequest {
            prompt,
            audio_path: wav_path,
            mmproj_path: None,
            max_tokens,
            seed,
            stream_callback: None,
        });
        stream_to_sse_response(rx, result_handle, model_id, max_tokens).await
    } else {
        let wav_path_clone = wav_path.clone();
        let result = engine
            .run_multimodal(MultimodalRequest {
                prompt,
                audio_path: wav_path,
                mmproj_path: None,
                max_tokens,
                seed,
                stream_callback: None,
            })
            .await;

        // Clean up temp WAV file.
        let _ = std::fs::remove_file(&wav_path_clone);

        match result {
            Ok(output) => {
                let finish_reason = if output.generated_tokens >= max_tokens {
                    "length"
                } else {
                    "stop"
                };

                let response = ChatCompletionResponse {
                    id: generate_completion_id(),
                    object: "chat.completion".to_string(),
                    created: unix_timestamp(),
                    model: model_id,
                    choices: vec![ChatChoice {
                        index: 0,
                        message: ChatResponseMessage {
                            role: Role::Assistant,
                            content: output.text,
                        },
                        finish_reason: finish_reason.to_string(),
                    }],
                    usage: Usage {
                        prompt_tokens: output.prompt_tokens,
                        completion_tokens: output.generated_tokens,
                        total_tokens: output.prompt_tokens + output.generated_tokens,
                    },
                };
                Json(response).into_response()
            }
            Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
        }
    }
}

/// Convert raw audio bytes (any format ffmpeg understands) to a 16 kHz mono WAV
/// temp file.  Returns the path to the WAV file; the caller is responsible for
/// deleting it.
fn audio_bytes_to_wav(
    audio_bytes: &[u8],
    _format_hint: &str,
) -> Result<std::path::PathBuf, String> {
    let tmp_dir = std::env::temp_dir();
    let wav_path = tmp_dir.join(format!(
        "llm_audio_{}.wav",
        uuid::Uuid::new_v4()
    ));

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",            // overwrite
            "-i", "pipe:0",  // read from stdin
            "-ar", "16000",  // 16 kHz sample rate
            "-ac", "1",      // mono
            "-f", "wav",
            wav_path.to_str().ok_or("temp path not UTF-8")?,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg: {e}"))?;

    {
        let mut stdin = child.stdin.take().ok_or("Failed to open ffmpeg stdin")?;
        stdin
            .write_all(audio_bytes)
            .map_err(|e| format!("Failed to write audio to ffmpeg: {e}"))?;
    }

    let status = child
        .wait()
        .map_err(|e| format!("ffmpeg wait error: {e}"))?;
    if !status.success() {
        return Err("ffmpeg failed to convert audio to WAV".to_string());
    }

    Ok(wav_path)
}

// ---------------------------------------------------------------------------
// Text-only helpers
// ---------------------------------------------------------------------------

async fn stream_to_sse_response(
    rx: tokio::sync::mpsc::Receiver<String>,
    result_handle: tokio::task::JoinHandle<anyhow::Result<crate::inference::GenerateOutput>>,
    model_id: String,
    max_tokens: u32,
) -> Response {
    let completion_id = generate_completion_id();
    let created = unix_timestamp();
    let token_stream = ReceiverStream::new(rx);

    // First chunk: role announcement.
    let role_chunk = ChatCompletionChunk {
        id: completion_id.clone(),
        object: "chat.completion.chunk".to_string(),
        created,
        model: model_id.clone(),
        choices: vec![ChunkChoice {
            index: 0,
            delta: ChunkDelta {
                role: Some(Role::Assistant),
                content: None,
            },
            finish_reason: None,
        }],
    };
    let role_event = Event::default().data(serde_json::to_string(&role_chunk).unwrap_or_default());

    let id_for_tokens = completion_id.clone();
    let model_for_tokens = model_id.clone();

    let token_events = token_stream.map(move |piece| {
        let chunk = ChatCompletionChunk {
            id: id_for_tokens.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model_for_tokens.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta {
                    role: None,
                    content: Some(piece),
                },
                finish_reason: None,
            }],
        };
        Ok::<Event, std::convert::Infallible>(
            Event::default().data(serde_json::to_string(&chunk).unwrap_or_default()),
        )
    });

    let id_for_final = completion_id;
    let model_for_final = model_id;

    let final_events = stream::once(async move {
        let finish_reason = match result_handle.await {
            Ok(Ok(output)) if output.generated_tokens >= max_tokens => "length",
            _ => "stop",
        };

        let done_chunk = ChatCompletionChunk {
            id: id_for_final,
            object: "chat.completion.chunk".to_string(),
            created,
            model: model_for_final,
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta {
                    role: None,
                    content: None,
                },
                finish_reason: Some(finish_reason.to_string()),
            }],
        };

        let done_json = serde_json::to_string(&done_chunk).unwrap_or_default();
        let events = vec![
            Ok::<Event, std::convert::Infallible>(Event::default().data(done_json)),
            Ok::<Event, std::convert::Infallible>(Event::default().data("[DONE]")),
        ];
        stream::iter(events)
    })
    .flatten();

    let role_stream =
        stream::once(async move { Ok::<Event, std::convert::Infallible>(role_event) });
    let full_stream = role_stream.chain(token_events).chain(final_events);

    let sse = Sse::new(full_stream).keep_alive(axum::response::sse::KeepAlive::default());

    (
        [
            (axum::http::header::CACHE_CONTROL, "no-cache"),
            (
                axum::http::header::HeaderName::from_static("x-accel-buffering"),
                "no",
            ),
        ],
        sse,
    )
        .into_response()
}

async fn handle_non_streaming(
    engine: EngineService,
    messages: Vec<crate::chat_template::ChatMessage>,
    max_tokens: u32,
    seed: Option<u32>,
    model_id: String,
) -> Response {
    let result = engine.chat(messages, max_tokens, seed).await;

    match result {
        Ok(output) => {
            let finish_reason = if output.generated_tokens >= max_tokens {
                "length"
            } else {
                "stop"
            };

            let response = ChatCompletionResponse {
                id: generate_completion_id(),
                object: "chat.completion".to_string(),
                created: unix_timestamp(),
                model: model_id,
                choices: vec![ChatChoice {
                    index: 0,
                    message: ChatResponseMessage {
                        role: Role::Assistant,
                        content: output.text,
                    },
                    finish_reason: finish_reason.to_string(),
                }],
                usage: Usage {
                    prompt_tokens: output.prompt_tokens,
                    completion_tokens: output.generated_tokens,
                    total_tokens: output.prompt_tokens + output.generated_tokens,
                },
            };

            Json(response).into_response()
        }
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
    }
}

async fn handle_streaming(
    engine: EngineService,
    messages: Vec<crate::chat_template::ChatMessage>,
    max_tokens: u32,
    seed: Option<u32>,
    model_id: String,
) -> Response {
    let (rx, result_handle) = engine.chat_streaming(messages, max_tokens, seed);
    stream_to_sse_response(rx, result_handle, model_id, max_tokens).await
}

fn error_response(status: StatusCode, message: &str) -> Response {
    let body = ErrorResponse {
        error: ErrorBody {
            message: message.to_string(),
            error_type: "server_error".to_string(),
            code: None,
        },
    };
    (status, Json(body)).into_response()
}



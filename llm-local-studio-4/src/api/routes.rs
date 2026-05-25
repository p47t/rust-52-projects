//! HTTP route handlers for the OpenAI-compatible API.

use axum::Json;
use axum::extract::{State, Multipart};
use axum::http::StatusCode;
use axum::response::{
    IntoResponse, Response,
    sse::{Event, Sse},
};
use futures::stream::{self, StreamExt};
use tokio_stream::wrappers::ReceiverStream;
use std::process::{Command, Stdio};
use std::io::Write;

use crate::api::AppState;
use crate::api::types::{
    ChatChoice, ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse,
    ChatResponseMessage, ChunkChoice, ChunkDelta, ErrorBody, ErrorResponse, ModelListResponse,
    ModelObject, Usage, generate_completion_id, unix_timestamp,
};
use crate::chat_template::Role;
use crate::engine_service::EngineService;

/// GET /health
pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

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
    }).into_response()
}

/// POST /v1/chat/completions
pub async fn chat_completions(
    State(state): State<AppState>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    let max_tokens = request.max_tokens.unwrap_or(128);
    let seed = request.seed;
    let streaming = request.stream.unwrap_or(false);

    let model_id = state.engine
        .model_info()
        .map(|info| info.model_id)
        .unwrap_or_else(|| request.model.clone());

    if streaming {
        handle_streaming(state.engine, request.messages, max_tokens, seed, model_id).await
    } else {
        handle_non_streaming(state.engine, request.messages, max_tokens, seed, model_id).await
    }
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
    let completion_id = generate_completion_id();
    let created = unix_timestamp();

    let (rx, result_handle) = engine.chat_streaming(messages, max_tokens, seed);
    let token_stream = ReceiverStream::new(rx);

    // First chunk: send the role
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

    // Token chunks
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

    // Final chunks: finish_reason + [DONE]
    let id_for_final = completion_id;
    let model_for_final = model_id;

    let final_events = stream::once(async move {
        // Wait for inference to complete to get the finish reason
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

    // Combine: role event -> token events -> final events
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

/// POST /v1/audio/transcriptions
pub async fn audio_transcriptions(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    handle_audio(State(state), multipart, false).await
}

/// POST /v1/audio/translations
pub async fn audio_translations(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    handle_audio(State(state), multipart, true).await
}

async fn handle_audio(
    State(state): State<AppState>,
    mut multipart: Multipart,
    is_translation: bool,
) -> Response {
    let whisper_engine = match &state.whisper_engine {
        Some(engine) => engine.clone(),
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "Whisper model not loaded on this server. Start the server with --whisper-model <path/id>",
            );
        }
    };

    let mut file_bytes = None;
    let mut language = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let res = field.bytes().await;
            if let Ok(bytes) = res {
                file_bytes = Some(bytes.to_vec());
            }
        } else if name == "language" {
            let res = field.text().await;
            if let Ok(text) = res {
                language = Some(text);
            }
        }
    }

    let file_bytes = match file_bytes {
        Some(bytes) => bytes,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "Missing 'file' parameter in multipart form",
            );
        }
    };

    let pcm = match decode_audio_to_pcm(&file_bytes) {
        Ok(pcm) => pcm,
        Err(err) => return error_response(StatusCode::BAD_REQUEST, &err),
    };

    let result = tokio::task::spawn_blocking(move || {
        let mut engine = whisper_engine.blocking_lock();
        let lang = language.as_deref().unwrap_or("auto");
        engine.transcribe(&pcm, is_translation, Some(lang))
    })
    .await
    .unwrap();

    match result {
        Ok(text) => Json(serde_json::json!({ "text": text.trim() })).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
    }
}

fn decode_audio_to_pcm(audio_bytes: &[u8]) -> Result<Vec<f32>, String> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-i", "pipe:0",
            "-ar", "16000",
            "-ac", "1",
            "-f", "s16le",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg: {e}"))?;

    {
        let mut stdin = child.stdin.take().ok_or("Failed to open stdin")?;
        stdin
            .write_all(audio_bytes)
            .map_err(|e| format!("Failed to write to stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait on ffmpeg: {e}"))?;
    if !output.status.success() {
        return Err("ffmpeg failed to process audio".to_string());
    }

    let pcm_bytes = output.stdout;
    let samples_count = pcm_bytes.len() / 2;
    let mut f32_samples = Vec::with_capacity(samples_count);

    for chunk in pcm_bytes.chunks_exact(2) {
        let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
        let sample_f32 = sample_i16 as f32 / 32768.0;
        f32_samples.push(sample_f32);
    }

    Ok(f32_samples)
}

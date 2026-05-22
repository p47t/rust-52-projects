#![allow(dead_code)]
//! OpenAI-compatible request/response types for the chat completions and models API.
//!
//! These types mirror the OpenAI REST API schema so that any client speaking the
//! OpenAI protocol can talk to our local inference server unchanged.

use crate::chat_template::{ChatMessage, Role};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Chat Completion Request
// ---------------------------------------------------------------------------

/// A chat completion request following the OpenAI `POST /v1/chat/completions` schema.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionRequest {
    /// Model identifier (e.g. `"llama-3-8b"`).
    pub model: String,
    /// The conversation history sent by the client.
    pub messages: Vec<ChatMessage>,
    /// Sampling temperature (0.0–2.0). `None` means the engine default.
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Upper bound on the number of tokens to generate.
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// If `true`, the response is delivered as SSE stream of [`ChatCompletionChunk`]s.
    #[serde(default)]
    pub stream: Option<bool>,
    /// Optional RNG seed for reproducible sampling.
    #[serde(default)]
    pub seed: Option<u32>,
}

// ---------------------------------------------------------------------------
// Chat Completion Response (non-streaming)
// ---------------------------------------------------------------------------

/// A complete chat completion response.
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionResponse {
    /// Unique completion identifier, e.g. `"chatcmpl-<uuid>"`.
    pub id: String,
    /// Always `"chat.completion"`.
    pub object: String,
    /// Unix timestamp (seconds) when the response was created.
    pub created: i64,
    /// The model that produced this completion.
    pub model: String,
    /// One or more generated choices.
    pub choices: Vec<ChatChoice>,
    /// Token usage statistics.
    pub usage: Usage,
}

/// A single choice inside a [`ChatCompletionResponse`].
#[derive(Debug, Clone, Serialize)]
pub struct ChatChoice {
    /// Zero-based index of this choice.
    pub index: u32,
    /// The assistant's reply.
    pub message: ChatResponseMessage,
    /// Why generation stopped: `"stop"` (natural end) or `"length"` (hit max_tokens).
    pub finish_reason: String,
}

/// The assistant message returned in a non-streaming response.
#[derive(Debug, Clone, Serialize)]
pub struct ChatResponseMessage {
    /// Always [`Role::Assistant`].
    pub role: Role,
    /// The generated text.
    pub content: String,
}

/// Token usage breakdown.
#[derive(Debug, Clone, Serialize)]
pub struct Usage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: u32,
    /// Number of tokens generated.
    pub completion_tokens: u32,
    /// `prompt_tokens + completion_tokens`.
    pub total_tokens: u32,
}

// ---------------------------------------------------------------------------
// Streaming (SSE) types
// ---------------------------------------------------------------------------

/// A single SSE chunk emitted during a streaming chat completion.
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionChunk {
    /// Same id shared by every chunk in a single completion.
    pub id: String,
    /// Always `"chat.completion.chunk"`.
    pub object: String,
    /// Unix timestamp (seconds).
    pub created: i64,
    /// The model producing the stream.
    pub model: String,
    /// Chunk-level choices (typically one).
    pub choices: Vec<ChunkChoice>,
}

/// A single choice delta inside a [`ChatCompletionChunk`].
#[derive(Debug, Clone, Serialize)]
pub struct ChunkChoice {
    /// Zero-based index.
    pub index: u32,
    /// Incremental content for this choice.
    pub delta: ChunkDelta,
    /// `None` while generating, `Some("stop")` or `Some("length")` on the final chunk.
    pub finish_reason: Option<String>,
}

/// The incremental payload carried inside a [`ChunkChoice`].
#[derive(Debug, Clone, Serialize)]
pub struct ChunkDelta {
    /// Present only in the very first chunk to announce the role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    /// The next piece of generated text (`None` in the final chunk).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// ---------------------------------------------------------------------------
// Models API
// ---------------------------------------------------------------------------

/// A single model entry, matching the OpenAI `GET /v1/models` schema.
#[derive(Debug, Clone, Serialize)]
pub struct ModelObject {
    /// Unique model identifier.
    pub id: String,
    /// Always `"model"`.
    pub object: String,
    /// Unix timestamp of when the model was registered.
    pub created: i64,
    /// Owner label; we use `"local"`.
    pub owned_by: String,
}

/// The list wrapper returned by `GET /v1/models`.
#[derive(Debug, Clone, Serialize)]
pub struct ModelListResponse {
    /// Always `"list"`.
    pub object: String,
    /// The available models.
    pub data: Vec<ModelObject>,
}

// ---------------------------------------------------------------------------
// Error response
// ---------------------------------------------------------------------------

/// The inner body of an error response.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorBody {
    /// Human-readable error description.
    pub message: String,
    /// Error category (e.g. `"invalid_request_error"`).
    #[serde(rename = "type")]
    pub error_type: String,
    /// Optional machine-readable error code.
    pub code: Option<String>,
}

/// An error response envelope matching the OpenAI error format.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    /// The error details.
    pub error: ErrorBody,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a unique completion id in the form `"chatcmpl-<uuid>"`.
pub fn generate_completion_id() -> String {
    format!("chatcmpl-{}", uuid::Uuid::new_v4())
}

/// Return the current UTC time as a Unix timestamp (seconds).
pub fn unix_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that a non-streaming response serializes to the expected OpenAI shape.
    #[test]
    fn test_chat_completion_response_serialization() {
        let response = ChatCompletionResponse {
            id: "chatcmpl-test-id".to_string(),
            object: "chat.completion".to_string(),
            created: 1_700_000_000,
            model: "test-model".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatResponseMessage {
                    role: Role::Assistant,
                    content: "Hello!".to_string(),
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
        };

        let json = serde_json::to_value(&response).expect("failed to serialize");

        assert_eq!(json["id"], "chatcmpl-test-id");
        assert_eq!(json["object"], "chat.completion");
        assert_eq!(json["created"], 1_700_000_000);
        assert_eq!(json["model"], "test-model");

        let choice = &json["choices"][0];
        assert_eq!(choice["index"], 0);
        assert_eq!(choice["message"]["content"], "Hello!");
        assert_eq!(choice["finish_reason"], "stop");

        let usage = &json["usage"];
        assert_eq!(usage["prompt_tokens"], 10);
        assert_eq!(usage["completion_tokens"], 5);
        assert_eq!(usage["total_tokens"], 15);
    }

    /// Verify that a streaming chunk serializes correctly, including
    /// `skip_serializing_if` behaviour for `ChunkDelta` fields.
    #[test]
    fn test_chat_completion_chunk_serialization() {
        // First chunk — has role, has content, no finish_reason.
        let first_chunk = ChatCompletionChunk {
            id: "chatcmpl-stream-id".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1_700_000_000,
            model: "test-model".to_string(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta {
                    role: Some(Role::Assistant),
                    content: Some("Hi".to_string()),
                },
                finish_reason: None,
            }],
        };

        let json = serde_json::to_value(&first_chunk).expect("failed to serialize");
        assert_eq!(json["object"], "chat.completion.chunk");
        let delta = &json["choices"][0]["delta"];
        assert!(delta.get("role").is_some(), "first chunk should include role");
        assert_eq!(delta["content"], "Hi");
        assert!(json["choices"][0]["finish_reason"].is_null());

        // Final chunk — no role, no content, has finish_reason.
        let final_chunk = ChatCompletionChunk {
            id: "chatcmpl-stream-id".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1_700_000_000,
            model: "test-model".to_string(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta {
                    role: None,
                    content: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
        };

        let json = serde_json::to_value(&final_chunk).expect("failed to serialize");
        let delta = &json["choices"][0]["delta"];
        assert!(delta.get("role").is_none(), "final chunk should omit role");
        assert!(
            delta.get("content").is_none(),
            "final chunk should omit content"
        );
        assert_eq!(json["choices"][0]["finish_reason"], "stop");
    }

    /// `generate_completion_id` should produce the expected prefix.
    #[test]
    fn test_generate_completion_id_prefix() {
        let id = generate_completion_id();
        assert!(
            id.starts_with("chatcmpl-"),
            "id should start with 'chatcmpl-', got: {id}"
        );
    }
}

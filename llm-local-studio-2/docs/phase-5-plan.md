# Phase 5 Plan: OpenAI-Compatible HTTP API with Streaming Chat Completions

Builds on the completed Phases 1–4 (model registry, HuggingFace integration,
llama.cpp FFI, model loading, tokenization, and single-prompt completion).

## Architecture

```text
+--------------------------------------------------------------+
| CLI: scan, hf-search, hf-download, run, serve               |
+-----------------------------+--------------------------------+
                              |
                              v
+--------------------------------------------------------------+
| EngineService (async wrapper)                                 |
| tokio::sync::Mutex + spawn_blocking + mpsc channels          |
+-----------------------------+--------------------------------+
                              |
            +-----------------+-----------------+
            v                                   v
+-------------------------+   +-------------------------------+
| LlamaCppEngine          |   | HTTP API (actix-web)          |
| load, run, chat         |   | /v1/chat/completions          |
| InferenceEngine trait   |   | /v1/models                    |
+-------------------------+   | /health                       |
                              | SSE streaming + JSON           |
                              +-------------------------------+
                              |
                              v
+--------------------------------------------------------------+
| Chat Templates                                                |
| ChatML, Llama3, Generic fallback                              |
| auto_detect(model_name) heuristic                             |
+--------------------------------------------------------------+
```

## What Was Built

### Chat Template System (`chat_template.rs`)
- `ChatTemplate` trait with `apply()` and `name()` methods
- `ChatMLTemplate` — widely supported `<|im_start|>/<|im_end|>` format
- `Llama3Template` — Meta Llama 3/3.1 special token format
- `GenericTemplate` — simple markdown-style `### User:` fallback
- `auto_detect()` — model name heuristic for template selection

### Async Engine Service (`engine_service.rs`)
- `EngineService` wraps `LlamaCppEngine` behind `Arc<Mutex<>>` for thread safety
- All inference dispatched to `tokio::task::spawn_blocking` threads
- `chat_streaming()` returns an `mpsc::Receiver<String>` for token-by-token streaming

### OpenAI-Compatible HTTP API (`api/`)
- **`POST /v1/chat/completions`** — full chat completion with messages array
  - Non-streaming: returns complete `ChatCompletionResponse` JSON
  - Streaming: returns SSE `text/event-stream` with `ChatCompletionChunk` deltas
  - Terminates with `data: [DONE]`
- **`GET /v1/models`** — lists the currently loaded model
- **`GET /health`** — simple health check
- Permissive CORS enabled by default for web UI compatibility
- Request logging middleware

### CLI Integration
- New `serve` subcommand:
  ```
  llm-local-studio serve <model> [--port 8080] [--host 127.0.0.1] [--ctx-size 2048]
  ```
- Reuses existing model resolution (direct path, registry scan, fuzzy match)

### Inference Engine Extensions
- `chat()` method applies the model's chat template then delegates to `run()`
- `model_info()` returns the currently loaded model's ID
- `GenerateOutput` now includes `prompt_tokens` count for usage tracking

## New Dependencies

| Crate | Purpose |
|---|---|
| actix-web 4 | HTTP server framework |
| actix-cors 0.7 | CORS middleware |
| tokio 1 (full) | Async runtime, channels, spawn_blocking |
| tokio-stream 0.1 | ReceiverStream for SSE bridging |
| serde_json 1.0 | JSON serialization |
| uuid 1 (v4) | Completion ID generation |
| chrono 0.4 | Unix timestamps |
| futures 0.3 | Stream combinators |

## Usage

```bash
# Start the server
cargo run -- serve path/to/model.gguf --port 8080

# Test non-streaming
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"test","messages":[{"role":"user","content":"Hello!"}],"max_tokens":50}'

# Test streaming
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"test","messages":[{"role":"user","content":"Hello!"}],"stream":true}'

# List models
curl http://localhost:8080/v1/models
```

## Later Phases

1. Phase 6: Desktop or web UI
2. Temperature / top-p / top-k sampling parameters
3. Multi-model hot-swap via API
4. Download progress bars
5. GPU layer offloading configuration

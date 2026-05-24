# Phase 1 Plan: Model Registry, Hugging Face, and llama.cpp Boundary

This project is a learning-oriented LM Studio/Ollama-style app that will use
`llama.cpp` directly as the inference engine through a native library adapter,
not by spawning `llama-server`.

## Architecture

```text
+--------------------------------------------------------------+
| App Shell                                                     |
| CLI first, then desktop/web UI                                |
+-----------------------------+--------------------------------+
                              |
                              v
+--------------------------------------------------------------+
| Application Services                                          |
| model registry, Hugging Face catalog, settings, logs          |
+-----------------------------+--------------------------------+
                              |
                              v
+--------------------------------------------------------------+
| Inference Engine Interface                                    |
| stable app-owned trait for load/unload/chat/complete/health   |
+-----------------------------+--------------------------------+
                              |
                              v
+--------------------------------------------------------------+
| llama.cpp Native Adapter                                      |
| Rust FFI wrapper around llama.cpp / ggml                      |
+-----------------------------+--------------------------------+
                              |
                              v
+--------------------------------------------------------------+
| llama.cpp                                                     |
| GGUF loading, tokenizer, KV cache, sampling, CPU/GPU backends |
+--------------------------------------------------------------+
```

## Phase 1 Goals

Phase 1 prepares the app layer. It does not run inference yet.

- Create a standalone app crate separate from the existing GGUF parser project.
- Define the model registry data model.
- Scan local folders for `.gguf` files.
- Add a Hugging Face integration layer for GGUF repository discovery and download planning.
- Define the app-owned inference interface that the direct `llama.cpp` adapter will implement.
- Save enough architecture documentation to keep later phases focused.

## Deliverables

- `ModelRegistry` scans a configured model directory recursively.
- `ModelRecord` stores model ID, path, file size, source, and load status.
- `HuggingFaceClient` builds search and download URLs for GGUF files.
- `InferenceEngine` trait documents the boundary for the future native `llama.cpp` adapter.
- CLI commands exist for:
  - listing local models
  - scanning a model folder
  - searching Hugging Face
  - planning a Hugging Face download URL

## Hugging Face Integration Scope

Start simple and deterministic:

- Search model repositories with `filter=gguf` later when HTTP is wired.
- Prefer repositories that publish `.gguf` files under the `resolve/main/...` download URL shape.
- Support optional token authentication later via `HF_TOKEN`.
- Store downloads under a local model directory:

```text
models/
  huggingface/
    <owner>/
      <repo>/
        <filename>.gguf
```

Phase 1 only creates the app abstraction and URL planning. Actual authenticated
HTTP downloads can be added once the dependency stack is chosen.

## llama.cpp Direct Integration Plan

Do not let the rest of the app depend on llama.cpp symbols directly. The app
depends on this interface:

```rust
trait InferenceEngine {
    fn load_model(&mut self, request: LoadModelRequest) -> Result<ModelHandle>;
    fn unload_model(&mut self, model_id: &str) -> Result<()>;
    fn health(&self) -> EngineHealth;
}
```

The first implementation will be:

```text
LlamaCppEngine
  -> llama.cpp C API through bindgen or handwritten FFI
  -> owns llama_model, llama_context, llama_sampler, and KV cache state
```

## Later Phases

1. Phase 2: Native `llama.cpp` build and FFI smoke test.
2. Phase 3: Model load/unload through `LlamaCppEngine`.
3. Phase 4: Tokenization and non-streaming completion.
4. Phase 5: Streaming chat completion API.
5. Phase 6: OpenAI-compatible HTTP API.
6. Phase 7: Desktop or web UI.

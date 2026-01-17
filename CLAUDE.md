# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

A learning-focused Rust repository containing 19 independent projects designed to build Rust skills through practical implementations. Each project is self-contained with its own Cargo.toml—there is no workspace.

## Build Commands

All commands must be run from within a project directory:

```bash
cd <project-name>
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Run tests
cargo test --verbose           # Run tests with output
cargo fmt -- --check           # Check formatting
cargo clippy --all-targets --all-features -- -D warnings  # Lint with warnings as errors
```

### Special Projects

- **small-os**: Requires nightly Rust, excluded from rust-analyzer, skip tests (bare-metal OS)
- **wasm-markdown-editor**: Build with `wasm-pack build --target web`
- **builder-derive**: Uses trybuild for compile-fail tests
- **libavformat-ffi**: Requires FFmpeg dev libraries (`libavformat-dev libavcodec-dev libavutil-dev`) and `libclang-dev`

## Project Categories

**System Programming**: small-os, shell
**Web/Networking**: web-server, hyper-app, socks5, game-server, file-transfer, tftp
**Parsers/Encoders**: ebml, clog
**Utilities**: calculator, count-words, rolldice, plum
**Advanced**: async-job-queue (Tokio async job queue), builder-derive (procedural macro)
**Cloud/WASM**: programming-wisdom (Lambda), wasm-markdown-editor

## Architecture Notes

### Independent Project Model
Each project is completely independent with no shared dependencies. This allows different Rust editions (2018/2021) and dependency versions per project. Any project can be modified or removed without affecting others.

### Notable Implementations

**async-job-queue**: Producer-consumer pattern with SQLite persistence, Tokio workers, priority scheduling, and retry logic with exponential backoff.

**builder-derive**: Procedural macro demonstrating syn/quote for parsing AST and generating builder pattern code. Has compile-fail tests in `tests/compile_fail/`.

**shell**: PEG parsing with pest, recursive data structures, process management.

### Dependency Patterns

Async frameworks: tokio, hyper, actix-web
Parsers: nom, pulldown-cmark, pest
Error handling: thiserror, anyhow (prefer over deprecated `failure`)
CLI: clap (prefer over deprecated `structopt`)
Serialization: serde, bincode, serde_json

## CI/CD

GitHub Actions workflow (`.github/workflows/ci.yml`):
- Smart change detection: only tests changed projects on PRs
- Matrix strategy runs each project in parallel
- Runs fmt check, clippy, build, test, and release build
- Uses stable Rust except small-os (nightly)

## Project Ideas

Future project ideas to expand Rust skills:

### Concurrency & Systems
- **thread-pool**: Custom thread pool implementation - teaches `Arc`, `Mutex`, channels, and worker patterns
- **memcache-clone**: In-memory key-value store with TTL - teaches concurrent data structures and cache eviction
- **custom-allocator**: A simple memory allocator - teaches unsafe Rust and low-level memory management

### Interpreters & Languages
- **brainfuck-vm**: Brainfuck interpreter with JIT compilation - teaches parsing, VMs, and optional unsafe optimization
- **lisp-repl**: Minimal Lisp interpreter - teaches recursive descent parsing, environments, and closures
- **regex-engine**: Build a regex matcher from scratch - teaches NFAs/DFAs and state machines

### Networking
- **dns-resolver**: DNS client/stub resolver - teaches binary protocols and UDP
- **chat-server**: WebSocket chat with rooms - teaches tokio + tungstenite, broadcasts
- **http-client**: Minimal HTTP/1.1 client - complements web-server project

### Graphics & Visualization
- **ray-tracer**: Weekend ray tracer in Rust - teaches math, traits, and parallelism with rayon
- **terminal-game**: Snake or Tetris in terminal - teaches game loops, crossterm/ratatui

### Data & Compression
- **lz77-compressor**: Implement LZ77/DEFLATE - teaches algorithms and bit manipulation
- **sqlite-parser**: Parse SQLite file format - teaches binary parsing and B-trees
- **json-db**: JSON document database with queries - teaches indexing and query planning

### CLI Tools
- **git-stats**: Analyze git history/contributions - teaches libgit2 bindings
- **fuzzer**: Simple mutation-based fuzzer - teaches property testing concepts

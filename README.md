Build Rust programming skills by creating 52 projects ~~in one year~~.

## Projects (in chronological order)

1. **rolldice** (2019-03-26) - Random dice rolling utility
2. **tftp** (2019-04-03) - TFTP server implementation
3. **shell** (2019-04-08) - Command-line shell with PEG parsing
4. **ebml** (2019-04-17) - EBML format parser
5. **game-server** (2019-05-24) - Multiplayer game server
6. **clog** (2019-05-28) - Colored log parser
7. **web-server** (2019-09-04) - HTTP web server
8. **programming-wisdom** (2020-04-28) - AWS Lambda function for programming quotes
9. **hyper-app** (2020-08-17) - Application using Hyper HTTP library
10. **plum** (2020-08-17) - Plum language interpreter
11. **file-transfer** (2021-01-03) - File transfer utility
12. **count-words** (2021-05-04) - Word counting tool
13. **calculator** (2021-09-13) - Command-line calculator
14. **small-os** (2022-02-26) - Bare-metal OS (requires nightly Rust)
15. **socks5** (2022-02-26) - SOCKS5 proxy server
16. **async-job-queue** (2026-01-03) - Tokio-based async job queue with SQLite persistence
17. **wasm-markdown-editor** (2026-01-04) - Markdown editor compiled to WebAssembly
18. **builder-derive** (2026-01-06) - Procedural macro for builder pattern
19. **adb-client** (2026-01-17) - Android Debug Bridge client implementation
20. **libavformat-ffi** (2026-01-17) - FFI bindings for FFmpeg's libavformat
21. **gpui-calculator** (2026-01-17) - Calculator built with GPUI framework
22. **iced-calculator** (2026-02-08) - Calculator with expression parser using Iced UI
23. **guitar-fretboard** (2026-02-08) - Guitar fretboard visualizer with audio playback
24. **flashcard-app** (2026-02-08) - Spaced repetition learning app with Slint UI (desktop + Android)
25. **wgpu-game-of-life** (2026-02-09) - Conway's Game of Life with WebGPU compute shaders and WASM

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
- **wgpu-game-of-life**: Build with `wasm-pack build --target web`, requires WebGPU-capable browser
- **builder-derive**: Uses trybuild for compile-fail tests
- **libavformat-ffi**: Requires FFmpeg dev libraries (`libavformat-dev libavcodec-dev libavutil-dev`) and `libclang-dev`
- **flashcard-app**: Android builds require NDK and cargo-apk (`cargo install cargo-apk`)

## Project Categories

**System Programming**: small-os, shell
**Web/Networking**: web-server, hyper-app, socks5, game-server, file-transfer, tftp
**Parsers/Encoders**: ebml, clog
**Utilities**: calculator, count-words, rolldice, plum
**GUI Applications**: iced-calculator, gpui-calculator, flashcard-app, guitar-fretboard
**Advanced**: async-job-queue (async job queue), builder-derive (procedural macro)
**Cloud/WASM**: programming-wisdom (Lambda), wasm-markdown-editor, wgpu-game-of-life
**Android**: flashcard-app, adb-client
**FFI**: libavformat-ffi

## Architecture

Each project is completely independent with no shared dependencies. This allows different Rust editions (2018/2021) and dependency versions per project. Any project can be modified or removed without affecting others.

See [CLAUDE.md](CLAUDE.md) for detailed project descriptions and development guidelines.

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
26. **mcp-adb** (2026-02-11) - MCP server for Android Debug Bridge - control Android devices from AI assistants
27. **media-metadata-explorer** (2026-02-13) - Multimedia metadata inspector with libavformat-powered stream analysis and catalog summaries
28. **tilesplit** (2026-02-13) - tile splitter with Ultra HDR gain map support
29. **tilesplit-wasm** (2026-02-14) - Browser-based image tile splitter with Ultra HDR gain map support (WASM)
30. **comic-viewer** (2026-02-19) - CBZ comic book viewer with keyboard navigation using Iced UI
31. **nes-cpu** (2026-02-27) - NES 2A03 CPU emulator (6502) that passes nestest.nes
32. **nes-ppu** (2026-03-01) - NES 2C02 PPU emulator with cycle-accurate VBL/NMI timing
33. **nes-joypad** (2026-03-07) - NES joypad emulation with keyboard and gamepad input
34. **nes-mapper** (2026-03-08) - NES cartridge mapper implementations (NROM, MMC1, UxROM, CNROM, AxROM)
35. **nes-apu** (2026-03-09) - NES 2A03 APU emulator with all 5 audio channels and full game demo
36. **nes-emu** (2026-03-17) - NES emulator frontend using Bevy engine with video, audio, and input
37. **qrcode** (2026-04-19) - QR code encoder/decoder with Version 1 support and CLI
38. **llm-gguf-parser** (2026-05-20) - GGUF model parser for metadata and tensor info
39. **llm-local-studio** (2026-05-24) - CLI client for Hugging Face model downloads and in-process llama.cpp text generation
40. **llm-local-studio-2** (2026-05-24) - Local LLM backend with OpenAI-compatible API server using Axum (supporting streaming chat completions)
41. **llm-local-studio-3** (2026-05-24) - Local LLM workspace bundling an embedded Vite Web UI inside a single compiled Rust binary
42. **llm-local-studio-4** (2026-05-24) - Local LLM workspace adding Audio Speech Recognition (ASR), Automatic Speech Translation (AST) via whisper-rs, and Gemma 4 Multimodal Audio proxy support

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
- **media-metadata-explorer**: Uses `libavformat-ffi`; requires FFmpeg dev libraries (`libavformat-dev libavcodec-dev libavutil-dev`) and `libclang-dev`
- **tilesplit-wasm**: Build with `wasm-pack build --target web`
- **llm-local-studio-3**: Requires `npm` on PATH to compile the embedded Vite Web UI during compilation
- **llm-local-studio-4**: Requires `npm` on PATH to compile the embedded Vite Web UI, and `ffmpeg` on PATH to decode and resample audio inputs for whisper.cpp and Gemma 4 audio completions

## Project Categories

**System Programming**: small-os, shell
**Emulation**: nes-cpu, nes-ppu, nes-joypad, nes-mapper, nes-apu, nes-emu
**Web/Networking**: web-server, hyper-app, socks5, game-server, file-transfer, tftp, llm-local-studio-2, llm-local-studio-3, llm-local-studio-4
**Parsers/Encoders**: ebml, clog, llm-gguf-parser, qrcode
**Utilities**: calculator, count-words, rolldice, plum, llm-local-studio
**GUI Applications**: iced-calculator, gpui-calculator, flashcard-app, guitar-fretboard, llm-local-studio-3, llm-local-studio-4
**Advanced**: async-job-queue (async job queue), builder-derive (procedural macro)
**Cloud/WASM**: programming-wisdom (Lambda), wasm-markdown-editor, wgpu-game-of-life, tilesplit-wasm
**Android**: flashcard-app, adb-client, mcp-adb
**FFI**: libavformat-ffi
**Multimedia**: media-metadata-explorer, libavformat-ffi, guitar-fretboard

## Architecture

Each project is completely independent with no shared dependencies. This allows different Rust editions (2018/2021) and dependency versions per project. Any project can be modified or removed without affecting others.

See [CLAUDE.md](CLAUDE.md) for detailed project descriptions and development guidelines.

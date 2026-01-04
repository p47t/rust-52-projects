# WASM Markdown Editor 📝

A blazing-fast markdown editor powered by Rust and WebAssembly. This project demonstrates how to build interactive web applications using Rust compiled to WASM, featuring real-time markdown parsing and live preview.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=flat&logo=webassembly&logoColor=white)

## Features

- ⚡ **Blazing Fast**: Markdown parsing powered by Rust's `pulldown-cmark` crate
- 🔄 **Live Preview**: See your markdown rendered in real-time as you type
- 📊 **Statistics**: Track word count, character count, and estimated reading time
- 💾 **Auto-save**: Your work is automatically saved to browser local storage
- 🎨 **Clean UI**: Modern, responsive interface with split-pane editing
- 📤 **Export**: Download your rendered markdown as a standalone HTML file
- ⌨️ **Keyboard Shortcuts**: Efficient editing with hotkeys

## What This Project Teaches

This project is part of a Rust learning journey and covers:

### WebAssembly Concepts
- Compiling Rust to WASM using `wasm-pack`
- JavaScript interop with `wasm-bindgen`
- Memory management between JS and Rust
- Optimizing WASM bundle size

### Rust Skills
- Working with the `pulldown-cmark` parser
- Using `serde` for serialization to JavaScript
- Error handling with panic hooks
- Module organization and API design
- Writing tests for WASM code

### Web Development
- Vanilla JavaScript ES6 modules
- DOM manipulation from WASM
- Browser APIs (LocalStorage)
- Responsive CSS Grid layout
- Debouncing for performance

## Prerequisites

You'll need to install:

1. **Rust** (latest stable version)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **wasm-pack** - Tool for building Rust-generated WebAssembly
   ```bash
   cargo install wasm-pack
   ```

3. **A local web server** (choose one):
   - Python: `python -m http.server` (built-in)
   - Node.js: `npm install -g http-server`
   - Rust: `cargo install basic-http-server`

## Building and Running

### Step 1: Build the WASM Module

From the project root directory:

```bash
wasm-pack build --target web
```

This will:
- Compile the Rust code to WebAssembly
- Generate JavaScript bindings
- Create a `pkg/` directory with the output

### Step 2: Serve the Web Application

From the **project root directory** (not the `www` directory), start a local server:

```bash
python -m http.server 8080
```

Or use any other local server:
```bash
# Using Python 3
python -m http.server 8080

# Using Node.js http-server
npx http-server -p 8080

# Using Rust basic-http-server
basic-http-server -p 8080
```

**Important**: Serve from the project root, not from the `www` directory!

### Step 3: Open in Browser

Open your browser and navigate to:
```
http://localhost:8080
```

You should see the markdown editor ready to use!

## Project Structure

```
wasm-markdown-editor/
├── Cargo.toml              # Rust dependencies and project config
├── src/
│   ├── lib.rs             # Main WASM entry point and public API
│   ├── parser.rs          # Markdown parsing logic
│   └── utils.rs           # Utility functions (panic hooks, logging)
├── www/                   # Web frontend
│   ├── index.html         # Main HTML page
│   ├── index.js           # JavaScript application logic
│   ├── styles.css         # Styling
│   └── package.json       # NPM scripts (optional)
├── tests/
│   └── web.rs            # WASM-specific tests
└── pkg/                   # Generated WASM output (after build)
    ├── wasm_markdown_editor.js
    ├── wasm_markdown_editor_bg.wasm
    └── ...
```

## Usage

### Basic Editing
1. Type markdown in the left pane
2. See live preview in the right pane
3. Your work is automatically saved

### Keyboard Shortcuts
- `Ctrl/Cmd + S`: Export to HTML
- `Ctrl/Cmd + K`: Clear editor

### Buttons
- **Clear**: Clear the editor (with confirmation)
- **Export HTML**: Download rendered HTML file
- **Load Sample**: Load example markdown

### Supported Markdown Features

- Headings (`# H1` through `###### H6`)
- **Bold** and *italic* text
- [Links](https://example.com)
- Inline `code`
- Code blocks with syntax
- Lists (ordered and unordered)
- Blockquotes
- Tables
- ~~Strikethrough~~
- Task lists
- Footnotes
- And more!

## Running Tests

### Unit Tests (Rust)
```bash
cargo test
```

### WASM Tests (in browser)
```bash
wasm-pack test --headless --firefox
# or
wasm-pack test --headless --chrome
```

## Optimization

The release build is optimized for size:
- LTO (Link Time Optimization) enabled
- `opt-level = "s"` for small binary size
- Optional `wee_alloc` for smaller allocator

Current bundle size: ~100-150 KB (WASM + JS)

### Further Optimization Ideas
- Enable `wee_alloc` feature for production
- Use `wasm-opt` for additional optimization
- Code splitting for larger applications

## Learning Resources

- [Rust and WebAssembly Book](https://rustwasm.github.io/docs/book/)
- [wasm-bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)
- [pulldown-cmark Documentation](https://docs.rs/pulldown-cmark/)
- [WebAssembly MDN](https://developer.mozilla.org/en-US/docs/WebAssembly)

## Next Steps / Ideas for Enhancement

Want to expand this project? Try adding:

- [ ] **Syntax highlighting** for code blocks (using highlight.js or Prism)
- [ ] **Multiple themes** (dark mode, high contrast)
- [ ] **File upload/download** (.md files)
- [ ] **Markdown extensions** (emojis, math equations with KaTeX)
- [ ] **Table of contents** generation
- [ ] **Split screen sync scroll**
- [ ] **Search and replace** functionality
- [ ] **Vim/Emacs keybindings**
- [ ] **Collaborative editing** (with WebRTC or WebSockets)
- [ ] **PWA support** (offline capability)

## Troubleshooting

### WASM module not loading
- Make sure you ran `wasm-pack build --target web`
- Check browser console for errors
- Verify you're serving from the `www` directory
- Some browsers require HTTPS for WASM; try Chrome/Firefox locally

### Build errors
- Update Rust: `rustup update`
- Clean build: `cargo clean && wasm-pack build --target web`
- Check you have the latest wasm-pack version

### Module not found errors
- The `index.js` imports from `../pkg/` - make sure the pkg directory exists
- The import path should be relative to the `www` directory

## License

This project is dual-licensed under MIT OR Apache-2.0, following the Rust convention.

## Contributing

This is a learning project, but suggestions and improvements are welcome!

## Acknowledgments

- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) - The markdown parser
- [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen) - JavaScript interop
- Rust and WebAssembly community for excellent documentation

---

**Happy Markdown Editing!** ✨

Part of the [rust-52-projects](https://github.com/yourusername/rust-52-projects) learning journey.

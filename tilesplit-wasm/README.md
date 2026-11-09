# TileSplit Web (WASM)

Browser-based Ultra HDR image tile splitter powered by Rust, WebAssembly, and Vite. Deployed to Cloudflare Workers using Workers Static Assets.

## Live Deployment

- **URL**: [https://tilesplit.simplypatrick.workers.dev](https://tilesplit.simplypatrick.workers.dev)

## Development & Building

### Prerequisites

- [mise](https://mise.jdx.dev/) (run `mise install` to automatically install `node` and `wasm-pack` defined in `mise.toml`)
- [Rust](https://rustup.rs/) with `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)

### Commands

```bash
# Install dependencies
npm install

# Compile Rust to WASM
npm run build:wasm

# Start Vite dev server
npm run dev

# Build for production (WASM + Vite)
npm run build

# Preview production build locally
npm run preview

# Deploy to Cloudflare Workers
npm run deploy
```

## Cloudflare Workers Configuration

Configured in `wrangler.toml` using Cloudflare Workers Static Assets:

```toml
name = "tilesplit"
compatibility_date = "2024-09-23"

[assets]
directory = "./dist"
```

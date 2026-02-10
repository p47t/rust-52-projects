# Plan: wgpu-game-of-life

## Context

Add a new project to rust-52-projects that implements Conway's Game of Life using WebGPU compute shaders, compiled to WASM. This teaches GPU programming fundamentals (compute pipelines, render pipelines, storage buffers, WGSL shaders) while building on the repo's existing WASM patterns from wasm-markdown-editor.

## File Structure

```
wgpu-game-of-life/
├── Cargo.toml
├── src/
│   ├── lib.rs              # WASM entry point, exported API
│   ├── gpu.rs              # wgpu init, pipeline creation, render loop
│   ├── compute.wgsl        # Compute shader (Game of Life rules)
│   └── render.wgsl         # Vertex + fragment shaders (grid visualization)
├── index.html              # Main page (matches wasm-markdown-editor pattern)
└── www/
    ├── index.js            # JS: controls, canvas interaction, animation loop
    └── styles.css          # Styling
```

## Cargo.toml

```toml
[package]
name = "wgpu-game-of-life"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wgpu = "24"
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = { version = "0.3", features = [
    "Document", "Window", "Element",
    "HtmlCanvasElement", "console",
] }
console_error_panic_hook = "0.1"
js-sys = "0.3"
```

Release profile: `opt-level = "s"`, `lto = true` (same as wasm-markdown-editor).

## Architecture

### GPU Pipeline Overview

```
[Storage Buffer A] ──read──> [Compute Shader] ──write──> [Storage Buffer B]
                                                              │
                              [Render Pipeline] <──read───────┘
                                    │
                              [Canvas Output]
```

Each simulation step: compute shader reads cell states from one buffer, writes next generation to the other, then swap. Render pipeline draws the current state.

### Rust Modules

**`lib.rs`** — WASM entry point
- `#[wasm_bindgen] pub async fn start(canvas_id: &str, width: u32, height: u32)` — initializes GPU, creates simulation, returns handle
- `#[wasm_bindgen] pub fn step(handle)` — advance one generation + render
- `#[wasm_bindgen] pub fn reset_random(handle)` — randomize grid
- `#[wasm_bindgen] pub fn clear(handle)` — clear grid
- `#[wasm_bindgen] pub fn toggle_cell(handle, x, y)` — toggle a single cell
- `#[wasm_bindgen] pub fn get_generation(handle) -> u32`
- `#[wasm_bindgen] pub fn get_population(handle) -> u32`
- Store state in a `thread_local! { RefCell<Option<Simulation>> }` or return an opaque handle

**`gpu.rs`** — GPU setup and pipelines
- `Simulation` struct holding:
  - `device`, `queue`, `surface`, `surface_config`
  - `cell_buffers: [Buffer; 2]` (ping-pong storage buffers)
  - `uniform_buffer` (grid dimensions)
  - `compute_pipeline`, `compute_bind_groups: [BindGroup; 2]`
  - `render_pipeline`, `render_bind_groups: [BindGroup; 2]`
  - `step: usize` (tracks which buffer is current)
  - `generation: u32`, `grid_width: u32`, `grid_height: u32`
- `Simulation::new(canvas, width, height)` — async init:
  1. Get wgpu instance (`wgpu::Backends::BROWSER_WEBGPU | GL`)
  2. Create surface from canvas element
  3. Request adapter + device
  4. Create two storage buffers sized `width * height * 4` bytes (u32 per cell)
  5. Initialize buffer A with random data via `queue.write_buffer`
  6. Create uniform buffer with `[width, height]`
  7. Build compute pipeline + 2 bind groups (A→B and B→A)
  8. Build render pipeline + 2 bind groups (read from A or B)
  9. Configure surface
- `Simulation::step()`:
  1. Create command encoder
  2. Compute pass: dispatch `ceil(width/8) x ceil(height/8)` workgroups
  3. Render pass: draw 6 vertices (full-screen quad), fragment shader reads cell buffer
  4. Swap step index
  5. Submit + present
- `Simulation::write_cells(data: &[u32])` — write cell data to current input buffer
- `Simulation::toggle_cell(x, y)` — read-back is expensive on GPU; keep a CPU-side mirror of cell state for toggle, then write to buffer

### WGSL Shaders

**`compute.wgsl`**
```wgsl
@group(0) @binding(0) var<uniform> grid: vec2u;          // grid dimensions
@group(0) @binding(1) var<storage, read> cellsIn: array<u32>;
@group(0) @binding(2) var<storage, read_write> cellsOut: array<u32>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= grid.x || id.y >= grid.y) { return; }
    let idx = id.y * grid.x + id.x;
    // Count 8 neighbors with wrapping (toroidal grid)
    var neighbors: u32 = 0;
    for (var dy: i32 = -1; dy <= 1; dy++) {
        for (var dx: i32 = -1; dx <= 1; dx++) {
            if (dx == 0 && dy == 0) { continue; }
            let nx = (i32(id.x) + dx + i32(grid.x)) % i32(grid.x);
            let ny = (i32(id.y) + dy + i32(grid.y)) % i32(grid.y);
            neighbors += cellsIn[u32(ny) * grid.x + u32(nx)];
        }
    }
    // Conway's rules: birth=3, survive=2|3
    cellsOut[idx] = select(0u, 1u, neighbors == 3u || (cellsIn[idx] == 1u && neighbors == 2u));
}
```

**`render.wgsl`**
```wgsl
@group(0) @binding(0) var<uniform> grid: vec2u;
@group(0) @binding(1) var<storage, read> cells: array<u32>;

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VertexOutput {
    // Full-screen quad from 6 vertices (2 triangles)
    let positions = array<vec2f, 6>(
        vec2f(-1, -1), vec2f(1, -1), vec2f(-1, 1),
        vec2f(-1,  1), vec2f(1, -1), vec2f( 1, 1),
    );
    var out: VertexOutput;
    out.pos = vec4f(positions[i], 0, 1);
    out.uv = (positions[i] + 1.0) * 0.5;  // [0,1] range
    return out;
}

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4f {
    let cell = vec2u(in.uv * vec2f(grid));
    let idx = cell.y * grid.x + cell.x;
    let alive = f32(cells[idx]);
    // Alive = bright green, dead = dark background
    return mix(vec4f(0.05, 0.05, 0.1, 1.0), vec4f(0.2, 0.9, 0.3, 1.0), alive);
}
```

### JavaScript (`www/index.js`)

- Import `init, start, step, reset_random, clear, toggle_cell, get_generation, get_population` from `../pkg/wgpu_game_of_life.js`
- `run()`: init WASM, call `await start("canvas", 128, 128)`
- Animation loop using `requestAnimationFrame`:
  - If playing: call `step()` at configured speed (steps per frame or frame skip)
  - Update generation/population display
- Button handlers: Play/Pause, Step, Randomize, Clear, Speed control
- Canvas click handler: map pixel coords to grid coords, call `toggle_cell(x, y)`
- Canvas drag: draw cells while mouse is held down

### HTML (`index.html`)

Following wasm-markdown-editor pattern:
- Header with title + subtitle
- Toolbar: Play/Pause button, Step button, Randomize, Clear, Speed slider
- Stats: Generation counter, Population counter
- `<canvas id="canvas" width="512" height="512">` — WebGPU render target
- Footer
- `<script type="module" src="www/index.js">`

### Grid Configuration

- Default: **128x128** cells rendered to a 512x512 (or responsive) canvas
- Toroidal wrapping (edges wrap around)
- CPU-side cell state mirror (`Vec<u32>`) for toggle_cell without GPU readback

## Implementation Order

1. **Cargo.toml + project skeleton** — set up dependencies, `lib.rs` stub
2. **`gpu.rs` — wgpu initialization** — surface, adapter, device, configure surface
3. **Shaders** — `compute.wgsl` and `render.wgsl`
4. **`gpu.rs` — pipeline creation** — compute pipeline, render pipeline, bind groups, buffers
5. **`gpu.rs` — simulation loop** — `step()` with compute dispatch + render pass
6. **`lib.rs` — WASM API** — exported functions wrapping Simulation methods
7. **`www/`** — HTML, CSS, JS with controls and animation loop
8. **Interaction** — cell toggling, randomize, clear
9. **Polish** — styling, speed control, responsive canvas

## Build & Run

```bash
cd wgpu-game-of-life
wasm-pack build --target web
# Serve with any HTTP server (needed for WASM MIME type)
python -m http.server 8080
# Open http://localhost:8080 in a WebGPU-capable browser (Chrome 113+, Edge 113+, Firefox 141+)
```

## Verification

1. `wasm-pack build --target web` compiles without errors
2. Open in browser — canvas shows random initial state
3. Click Play — cells animate following Game of Life rules
4. Click individual cells to toggle them
5. Randomize/Clear buttons work
6. Generation and population counters update
7. Speed slider adjusts simulation rate
8. No console errors

mod gpu;

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {
    static SIMULATION: RefCell<Option<gpu::Simulation>> = RefCell::new(None);
}

fn with_sim<F, R>(f: F) -> R
where
    F: FnOnce(&mut gpu::Simulation) -> R,
{
    SIMULATION.with(|sim| {
        let mut borrow = sim.borrow_mut();
        let sim = borrow.as_mut().expect("simulation not initialized — call start() first");
        f(sim)
    })
}

/// Initialize the simulation. Must be called before any other function.
/// `canvas_id` is the id of the HTML canvas element.
/// `grid_width` and `grid_height` set the cell grid dimensions.
#[wasm_bindgen]
pub async fn start(canvas_id: &str, grid_width: u32, grid_height: u32) {
    console_error_panic_hook::set_once();
    let sim = gpu::Simulation::new(canvas_id, grid_width, grid_height).await;
    SIMULATION.with(|s| {
        *s.borrow_mut() = Some(sim);
    });
    // Initial render
    with_sim(|sim| sim.render());
}

/// Advance one generation and render
#[wasm_bindgen]
pub fn step() {
    with_sim(|sim| sim.step());
}

/// Render current state without advancing
#[wasm_bindgen]
pub fn render() {
    with_sim(|sim| sim.render());
}

/// Randomize the grid and reset generation counter
#[wasm_bindgen]
pub fn reset_random() {
    with_sim(|sim| sim.reset_random());
}

/// Clear all cells and reset generation counter
#[wasm_bindgen]
pub fn clear() {
    with_sim(|sim| sim.clear());
}

/// Toggle a cell at grid coordinates (x, y)
#[wasm_bindgen]
pub fn toggle_cell(x: u32, y: u32) {
    with_sim(|sim| sim.toggle_cell(x, y));
}

/// Set a cell alive at grid coordinates (x, y)
#[wasm_bindgen]
pub fn set_cell(x: u32, y: u32, alive: bool) {
    with_sim(|sim| sim.set_cell(x, y, alive));
}

/// Get the current generation number
#[wasm_bindgen]
pub fn get_generation() -> u32 {
    with_sim(|sim| sim.generation)
}

/// Get the current population (number of alive cells)
#[wasm_bindgen]
pub fn get_population() -> u32 {
    with_sim(|sim| sim.population())
}

/// Get the grid width
#[wasm_bindgen]
pub fn get_grid_width() -> u32 {
    with_sim(|sim| sim.grid_width)
}

/// Get the grid height
#[wasm_bindgen]
pub fn get_grid_height() -> u32 {
    with_sim(|sim| sim.grid_height)
}

/// Notify the simulation of a canvas resize
#[wasm_bindgen]
pub fn resize(width: u32, height: u32) {
    with_sim(|sim| {
        sim.resize_surface(width, height);
        sim.render();
    });
}

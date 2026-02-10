@group(0) @binding(0) var<uniform> grid: vec2u;
@group(0) @binding(1) var<storage, read> cells_in: array<u32>;
@group(0) @binding(2) var<storage, read_write> cells_out: array<u32>;

fn cell_index(x: u32, y: u32) -> u32 {
    return y * grid.x + x;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= grid.x || id.y >= grid.y) {
        return;
    }

    // Count 8 neighbors with toroidal wrapping
    // Cell values are ages: 0 = dead, 1+ = alive for N generations
    var neighbors: u32 = 0u;
    for (var dy: i32 = -1; dy <= 1; dy++) {
        for (var dx: i32 = -1; dx <= 1; dx++) {
            if (dx == 0 && dy == 0) {
                continue;
            }
            let nx = u32((i32(id.x) + dx + i32(grid.x)) % i32(grid.x));
            let ny = u32((i32(id.y) + dy + i32(grid.y)) % i32(grid.y));
            neighbors += select(0u, 1u, cells_in[cell_index(nx, ny)] > 0u);
        }
    }

    let idx = cell_index(id.x, id.y);
    let age = cells_in[idx];
    let was_alive = age > 0u;

    // Conway's rules with age tracking
    if (neighbors == 3u && !was_alive) {
        // Birth: new cell starts at age 1
        cells_out[idx] = 1u;
    } else if (was_alive && (neighbors == 2u || neighbors == 3u)) {
        // Survive: increment age (cap at 255 to avoid overflow)
        cells_out[idx] = min(age + 1u, 255u);
    } else {
        // Death
        cells_out[idx] = 0u;
    }
}

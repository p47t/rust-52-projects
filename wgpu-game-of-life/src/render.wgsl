@group(0) @binding(0) var<uniform> grid: vec2u;
@group(0) @binding(1) var<storage, read> cells: array<u32>;

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

// Full-screen quad from 6 vertices (2 triangles)
@vertex
fn vs(@builtin(vertex_index) i: u32) -> VertexOutput {
    let positions = array<vec2f, 6>(
        vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
        vec2f(-1.0,  1.0), vec2f(1.0, -1.0), vec2f( 1.0, 1.0),
    );
    var out: VertexOutput;
    out.pos = vec4f(positions[i], 0.0, 1.0);
    // Map from clip space [-1,1] to UV [0,1], flip Y so top-left is (0,0)
    out.uv = vec2f(
        (positions[i].x + 1.0) * 0.5,
        (1.0 - positions[i].y) * 0.5,
    );
    return out;
}

// Map cell age to a color gradient
fn age_color(age: u32) -> vec4f {
    if (age == 0u) {
        return vec4f(0.06, 0.06, 0.12, 1.0); // dead
    }
    // Normalize age: 1 = newborn, saturates around 50+
    let t = clamp(f32(age - 1u) / 50.0, 0.0, 1.0);

    // Color ramp: bright green -> yellow -> orange -> warm white
    let c0 = vec3f(0.15, 0.90, 0.30); // newborn: vivid green
    let c1 = vec3f(0.80, 0.90, 0.15); // young: yellow-green
    let c2 = vec3f(0.95, 0.60, 0.10); // mature: orange
    let c3 = vec3f(1.00, 0.85, 0.70); // ancient: warm white

    var color: vec3f;
    if (t < 0.33) {
        color = mix(c0, c1, t / 0.33);
    } else if (t < 0.66) {
        color = mix(c1, c2, (t - 0.33) / 0.33);
    } else {
        color = mix(c2, c3, (t - 0.66) / 0.34);
    }
    return vec4f(color, 1.0);
}

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4f {
    let cell = vec2u(in.uv * vec2f(grid));
    // Clamp to grid bounds
    let cx = min(cell.x, grid.x - 1u);
    let cy = min(cell.y, grid.y - 1u);
    let idx = cy * grid.x + cx;
    let age = cells[idx];

    let base = age_color(age);

    // Subtle grid lines
    let cell_uv = fract(in.uv * vec2f(grid));
    let grid_line = step(cell_uv.x, 0.02) + step(cell_uv.y, 0.02);
    let grid_dim = min(grid_line, 1.0) * 0.03;

    return base - vec4f(grid_dim, grid_dim, grid_dim, 0.0);
}

use std::io::{Result, Write};

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub normal: Vertex,
    pub v1: Vertex,
    pub v2: Vertex,
    pub v3: Vertex,
}

pub struct ReliefConfig {
    pub width_mm: f32,
    pub height_mm: f32,
    pub relief_height_mm: f32,
    pub base_height_mm: f32,
}

/// Computes the normal vector for a triangle using the cross product of its edges.
fn calculate_normal(v1: Vertex, v2: Vertex, v3: Vertex) -> Vertex {
    let ax = v2.x - v1.x;
    let ay = v2.y - v1.y;
    let az = v2.z - v1.z;

    let bx = v3.x - v1.x;
    let by = v3.y - v1.y;
    let bz = v3.z - v1.z;

    // Cross product: A x B
    let nx = ay * bz - az * by;
    let ny = az * bx - ax * bz;
    let nz = ax * by - ay * bx;

    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len > 1e-6 {
        Vertex {
            x: nx / len,
            y: ny / len,
            z: nz / len,
        }
    } else {
        Vertex {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// Generates a list of triangles representing a watertight depth relief mesh.
pub fn generate_mesh(
    depth_data: &[f32],
    width: usize,
    height: usize,
    config: &ReliefConfig,
) -> Vec<Triangle> {
    let mut triangles = Vec::new();
    if width < 2 || height < 2 || depth_data.len() < width * height {
        return triangles;
    }

    let sx = config.width_mm / ((width - 1) as f32);
    let sy = config.height_mm / ((height - 1) as f32);

    // 1. Precalculate top (heightfield) and bottom (flat base) vertices
    let mut top_vertices = vec![
        vec![
            Vertex {
                x: 0.0,
                y: 0.0,
                z: 0.0
            };
            width
        ];
        height
    ];
    let mut bottom_vertices = vec![
        vec![
            Vertex {
                x: 0.0,
                y: 0.0,
                z: 0.0
            };
            width
        ];
        height
    ];

    for y in 0..height {
        for x in 0..width {
            let depth_idx = y * width + x;
            let depth_val = depth_data[depth_idx].clamp(0.0, 1.0);

            let px = (x as f32) * sx;
            let py = ((height - 1 - y) as f32) * sy; // Invert Y-axis so top of image maps to back of print bed
            let pz = config.base_height_mm + depth_val * config.relief_height_mm;

            top_vertices[y][x] = Vertex {
                x: px,
                y: py,
                z: pz,
            };
            bottom_vertices[y][x] = Vertex {
                x: px,
                y: py,
                z: 0.0,
            };
        }
    }

    // 2. Generate Top Surface Triangles (facing upwards)
    for y in 0..(height - 1) {
        for x in 0..(width - 1) {
            let a = top_vertices[y][x];
            let b = top_vertices[y][x + 1];
            let c = top_vertices[y + 1][x + 1];
            let d = top_vertices[y + 1][x];

            // Triangle 1: a -> c -> b (counter-clockwise)
            triangles.push(Triangle {
                normal: calculate_normal(a, c, b),
                v1: a,
                v2: c,
                v3: b,
            });

            // Triangle 2: a -> d -> c (counter-clockwise)
            triangles.push(Triangle {
                normal: calculate_normal(a, d, c),
                v1: a,
                v2: d,
                v3: c,
            });
        }
    }

    // 3. Generate Bottom Surface Triangles (facing downwards, z = 0.0)
    for y in 0..(height - 1) {
        for x in 0..(width - 1) {
            let a = bottom_vertices[y][x];
            let b = bottom_vertices[y][x + 1];
            let c = bottom_vertices[y + 1][x + 1];
            let d = bottom_vertices[y + 1][x];

            // Triangle 1: a -> b -> c (clockwise looking from top, facing down)
            triangles.push(Triangle {
                normal: calculate_normal(a, b, c),
                v1: a,
                v2: b,
                v3: c,
            });

            // Triangle 2: a -> c -> d (clockwise looking from top, facing down)
            triangles.push(Triangle {
                normal: calculate_normal(a, c, d),
                v1: a,
                v2: c,
                v3: d,
            });
        }
    }

    // 4. Generate Side Wall: Left (x = 0, facing -X)
    for y in 0..(height - 1) {
        let t0 = top_vertices[y][0];
        let t1 = top_vertices[y + 1][0];
        let b0 = bottom_vertices[y][0];
        let b1 = bottom_vertices[y + 1][0];

        triangles.push(Triangle {
            normal: calculate_normal(b0, t1, t0),
            v1: b0,
            v2: t1,
            v3: t0,
        });

        triangles.push(Triangle {
            normal: calculate_normal(b0, b1, t1),
            v1: b0,
            v2: b1,
            v3: t1,
        });
    }

    // 5. Generate Side Wall: Right (x = width - 1, facing +X)
    for y in 0..(height - 1) {
        let t0 = top_vertices[y][width - 1];
        let t1 = top_vertices[y + 1][width - 1];
        let b0 = bottom_vertices[y][width - 1];
        let b1 = bottom_vertices[y + 1][width - 1];

        triangles.push(Triangle {
            normal: calculate_normal(b0, t0, t1),
            v1: b0,
            v2: t0,
            v3: t1,
        });

        triangles.push(Triangle {
            normal: calculate_normal(b0, t1, b1),
            v1: b0,
            v2: t1,
            v3: b1,
        });
    }

    // 6. Generate Side Wall: Front (y = height - 1, physical Y = 0, facing -Y)
    for x in 0..(width - 1) {
        let t0 = top_vertices[height - 1][x];
        let t1 = top_vertices[height - 1][x + 1];
        let b0 = bottom_vertices[height - 1][x];
        let b1 = bottom_vertices[height - 1][x + 1];

        triangles.push(Triangle {
            normal: calculate_normal(b0, t1, t0),
            v1: b0,
            v2: t1,
            v3: t0,
        });

        triangles.push(Triangle {
            normal: calculate_normal(b0, b1, t1),
            v1: b0,
            v2: b1,
            v3: t1,
        });
    }

    // 7. Generate Side Wall: Back (y = 0, physical Y = Y_max, facing +Y)
    for x in 0..(width - 1) {
        let t0 = top_vertices[0][x];
        let t1 = top_vertices[0][x + 1];
        let b0 = bottom_vertices[0][x];
        let b1 = bottom_vertices[0][x + 1];

        triangles.push(Triangle {
            normal: calculate_normal(b0, t0, t1),
            v1: b0,
            v2: t0,
            v3: t1,
        });

        triangles.push(Triangle {
            normal: calculate_normal(b0, t1, b1),
            v1: b0,
            v2: t1,
            v3: b1,
        });
    }

    triangles
}

/// Exports the given triangles into a binary STL file format.
pub fn write_binary_stl<W: Write>(writer: &mut W, triangles: &[Triangle]) -> Result<()> {
    // Write 80-byte header
    let header = b"Antigravity 3D Depth Relief Generator - STL Output";
    let mut padded_header = [0u8; 80];
    let len = header.len().min(80);
    padded_header[..len].copy_from_slice(&header[..len]);
    writer.write_all(&padded_header)?;

    // Write number of triangles (4 bytes, u32)
    let num_triangles = triangles.len() as u32;
    writer.write_all(&num_triangles.to_le_bytes())?;

    // Write triangles
    for t in triangles {
        // Normal vector (12 bytes)
        writer.write_all(&t.normal.x.to_le_bytes())?;
        writer.write_all(&t.normal.y.to_le_bytes())?;
        writer.write_all(&t.normal.z.to_le_bytes())?;

        // Vertex 1 (12 bytes)
        writer.write_all(&t.v1.x.to_le_bytes())?;
        writer.write_all(&t.v1.y.to_le_bytes())?;
        writer.write_all(&t.v1.z.to_le_bytes())?;

        // Vertex 2 (12 bytes)
        writer.write_all(&t.v2.x.to_le_bytes())?;
        writer.write_all(&t.v2.y.to_le_bytes())?;
        writer.write_all(&t.v2.z.to_le_bytes())?;

        // Vertex 3 (12 bytes)
        writer.write_all(&t.v3.x.to_le_bytes())?;
        writer.write_all(&t.v3.y.to_le_bytes())?;
        writer.write_all(&t.v3.z.to_le_bytes())?;

        // Attribute byte count (2 bytes)
        writer.write_all(&0u16.to_le_bytes())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mesh_simple() {
        let depth_data = vec![0.0, 0.5, 0.5, 1.0];
        let config = ReliefConfig {
            width_mm: 10.0,
            height_mm: 10.0,
            relief_height_mm: 5.0,
            base_height_mm: 2.0,
        };

        let mesh = generate_mesh(&depth_data, 2, 2, &config);
        // Expected triangles:
        // Top: 2 triangles
        // Bottom: 2 triangles
        // 4 side walls, each has 1 segment, so 2 triangles per wall: 4 * 2 = 8 triangles
        // Total: 2 + 2 + 8 = 12 triangles
        assert_eq!(mesh.len(), 12);
    }
}

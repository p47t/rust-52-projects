#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Material uniforms: params.r = scanline_intensity, params.g = curvature,
// params.b = vignette_intensity, params.a = brightness_boost
@group(2) @binding(0) var<uniform> params: vec4<f32>;
@group(2) @binding(1) var source_texture: texture_2d<f32>;
@group(2) @binding(2) var source_sampler: sampler;

// NES resolution for scanline calculation
const NES_HEIGHT: f32 = 240.0;

// Apply barrel distortion to simulate CRT curvature
fn barrel_distort(uv: vec2<f32>, curvature: f32) -> vec2<f32> {
    let centered = uv - 0.5;
    let r2 = dot(centered, centered);
    let distorted = centered * (1.0 + curvature * r2);
    return distorted + 0.5;
}

// Horizontal scanline darkening
fn scanline(uv: vec2<f32>, intensity: f32) -> f32 {
    let line = sin(uv.y * NES_HEIGHT * 3.14159265) * 0.5 + 0.5;
    return mix(1.0, line, intensity);
}

// Shadow mask: simulate RGB phosphor triads
fn shadow_mask(uv: vec2<f32>) -> vec3<f32> {
    let pixel = vec2<i32>(vec2<f32>(uv.x * NES_HEIGHT * (256.0 / 240.0) * 3.0, uv.y * NES_HEIGHT * 3.0));
    let col = pixel.x % 3;
    if col == 0 {
        return vec3<f32>(1.0, 0.7, 0.7);
    } else if col == 1 {
        return vec3<f32>(0.7, 1.0, 0.7);
    } else {
        return vec3<f32>(0.7, 0.7, 1.0);
    }
}

// Vignette: darken edges of screen
fn vignette(uv: vec2<f32>, intensity: f32) -> f32 {
    let centered = uv - 0.5;
    let dist = dot(centered, centered);
    return 1.0 - dist * intensity * 2.0;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Detect Bevy's 1x1 white fallback texture (shown before bind group is ready)
    // and output black instead of white to prevent flash.
    let dims = textureDimensions(source_texture);
    if dims.x <= 1u && dims.y <= 1u {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    let curvature = params.g;
    let scanline_intensity = params.r;
    let vignette_intensity = params.b;
    let brightness = params.a;

    // Apply barrel distortion
    let uv = barrel_distort(mesh.uv, curvature);

    // Clip pixels outside the curved screen area
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // Sample the NES framebuffer
    var color = textureSample(source_texture, source_sampler, uv).rgb;

    // Apply scanlines
    color *= scanline(uv, scanline_intensity);

    // Apply shadow mask
    color *= shadow_mask(uv);

    // Apply vignette
    color *= vignette(uv, vignette_intensity);

    // Brightness boost to compensate for darkening from effects
    color *= brightness;

    return vec4<f32>(color, 1.0);
}

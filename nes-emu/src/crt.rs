use bevy::asset::load_internal_asset;
use bevy::color::LinearRgba;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Shader, ShaderRef};
use bevy::sprite::{Material2d, Material2dPlugin};

/// Shader handle for the embedded CRT shader.
const CRT_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(0x4E45535F4352545F53484144455201);

/// Bevy plugin that registers the CRT material and its embedded shader.
pub struct CrtPlugin;

impl Plugin for CrtPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            CRT_SHADER_HANDLE,
            "../assets/shaders/crt.wgsl",
            Shader::from_wgsl
        );
        app.add_plugins(Material2dPlugin::<CrtMaterial>::default());
    }
}

/// CRT post-processing material that applies retro TV effects to the NES framebuffer.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct CrtMaterial {
    /// Shader parameters (maps to uniform vec4):
    /// r: scanline_intensity (0.0-1.0)
    /// g: curvature (0.0-1.0)
    /// b: vignette_intensity (0.0-1.0)
    /// a: brightness_boost (compensates for darkening)
    #[uniform(0)]
    pub params: LinearRgba,

    /// The NES framebuffer texture to apply CRT effects to.
    #[texture(1)]
    #[sampler(2)]
    pub source_texture: Option<Handle<Image>>,
}

impl CrtMaterial {
    pub fn new(source_texture: Handle<Image>) -> Self {
        Self {
            params: LinearRgba::new(
                0.7, // scanline_intensity
                0.4, // curvature
                0.6, // vignette_intensity
                1.3, // brightness_boost
            ),
            source_texture: Some(source_texture),
        }
    }
}

impl Material2d for CrtMaterial {
    fn fragment_shader() -> ShaderRef {
        CRT_SHADER_HANDLE.into()
    }
}

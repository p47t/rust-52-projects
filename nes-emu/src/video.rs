use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::crt::CrtMaterial;

const NES_WIDTH: u32 = 256;
const NES_HEIGHT: u32 = 240;
const SCALE: f32 = 3.0;

/// Handle to the Bevy Image asset used as the NES framebuffer texture.
#[derive(Resource)]
pub struct FramebufferHandle(pub Handle<Image>);

/// Handle to the CRT material, used to trigger bind group recreation on image updates.
#[derive(Resource)]
pub struct CrtMaterialHandle(pub Handle<CrtMaterial>);

/// Startup system: creates the framebuffer texture, camera, and display quad.
pub fn setup_video(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CrtMaterial>>,
) {
    let mut image = Image::new_fill(
        Extent3d {
            width: NES_WIDTH,
            height: NES_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    // Linear filtering for CRT barrel distortion sub-texel sampling
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());

    let handle = images.add(image);
    commands.insert_resource(FramebufferHandle(handle.clone()));

    let mat_handle = materials.add(CrtMaterial::new(handle));
    commands.insert_resource(CrtMaterialHandle(mat_handle.clone()));

    // 2D camera, offset left so game is centered in the non-panel area
    commands.spawn((
        Camera2d,
        Transform::from_xyz(crate::debug_ui::PANEL_WIDTH / 2.0, 0.0, 0.0),
    ));

    // Quad with CRT material
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(mat_handle),
        Transform::default().with_scale(Vec3::new(
            NES_WIDTH as f32 * SCALE,
            NES_HEIGHT as f32 * SCALE,
            1.0,
        )),
    ));
}

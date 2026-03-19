use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

const NES_WIDTH: u32 = 256;
const NES_HEIGHT: u32 = 240;
const SCALE: f32 = 3.0;

/// Handle to the Bevy Image asset used as the NES framebuffer texture.
#[derive(Resource)]
pub struct FramebufferHandle(pub Handle<Image>);

/// Startup system: creates the framebuffer texture, camera, and sprite.
pub fn setup_video(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // Create a blank 256x240 RGBA8 image
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

    // Pixel-perfect scaling: nearest-neighbor filtering, no blur
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());

    let handle = images.add(image);
    commands.insert_resource(FramebufferHandle(handle.clone()));

    // 2D camera
    commands.spawn(Camera2d);

    // Sprite displaying the framebuffer, scaled up
    commands.spawn((
        Sprite::from_image(handle),
        Transform::from_scale(Vec3::splat(SCALE)),
    ));
}

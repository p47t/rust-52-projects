use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::{EguiContexts, EguiPlugin};

use bevy::image::{ImageSampler, ImageSamplerDescriptor};

use crate::crt::CrtMaterial;
use crate::emulation::NesSystem;
use crate::video::{CrtMaterialHandle, FramebufferHandle};

/// Width of the debug side panel in logical pixels.
pub const PANEL_WIDTH: f32 = 250.0;

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum VideoFilter {
    Nearest,
    Linear,
    #[default]
    Crt,
}

impl std::fmt::Display for VideoFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoFilter::Nearest => write!(f, "Nearest"),
            VideoFilter::Linear => write!(f, "Linear"),
            VideoFilter::Crt => write!(f, "CRT"),
        }
    }
}

#[derive(Resource)]
struct DebugUiVisible(bool);

pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .insert_resource(DebugUiVisible(true))
            .init_resource::<VideoFilter>()
            .add_systems(Update, toggle_debug_ui)
            .add_systems(
                Update,
                (draw_debug_ui, apply_video_filter)
                    .chain()
                    .after(toggle_debug_ui),
            );
    }
}

fn toggle_debug_ui(keys: Res<ButtonInput<KeyCode>>, mut visible: ResMut<DebugUiVisible>) {
    if keys.just_pressed(KeyCode::F3) {
        visible.0 = !visible.0;
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_debug_ui(
    visible: Res<DebugUiVisible>,
    nes: NonSend<NesSystem>,
    crt_handle: Res<CrtMaterialHandle>,
    mut materials: ResMut<Assets<CrtMaterial>>,
    mut filter: ResMut<VideoFilter>,
    mut contexts: EguiContexts,
    time: Res<Time>,
) {
    if !visible.0 {
        return;
    }

    let ctx = contexts.ctx_mut();

    egui::SidePanel::right("debug_panel")
        .exact_width(PANEL_WIDTH)
        .resizable(false)
        .show(ctx, |ui| {
            // FPS
            let fps = 1.0 / time.delta_secs();
            ui.label(format!("FPS: {fps:.1}"));
            ui.separator();

            // CPU registers
            ui.heading("CPU");
            let cpu = &nes.sys.cpu;
            ui.monospace(format!("A:{:02X}  X:{:02X}  Y:{:02X}", cpu.a, cpu.x, cpu.y));
            ui.monospace(format!("PC:{:04X}  SP:{:02X}", cpu.pc, cpu.sp));
            let p = cpu.p;
            ui.monospace(format!(
                "NV-BDIZC: {:1}{:1}{:1}{:1}{:1}{:1}{:1}{:1}",
                (p >> 7) & 1,
                (p >> 6) & 1,
                (p >> 5) & 1,
                (p >> 4) & 1,
                (p >> 3) & 1,
                (p >> 2) & 1,
                (p >> 1) & 1,
                p & 1,
            ));
            ui.monospace(format!("Cycles: {}", cpu.cycles));
            ui.separator();

            // PPU state
            ui.heading("PPU");
            let ppu = nes.sys.ppu.borrow();
            ui.monospace(format!(
                "Scanline: {:>3}  Dot: {:>3}",
                ppu.scanline, ppu.dot
            ));
            ui.monospace(format!(
                "CTRL:{:02X}  MASK:{:02X}  STATUS:{:02X}",
                ppu.ctrl, ppu.mask, ppu.status
            ));
            drop(ppu);
            ui.separator();

            // Video filter selection
            ui.heading("Video");
            let mut current = *filter;
            egui::ComboBox::from_label("Filter")
                .selected_text(current.to_string())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut current, VideoFilter::Nearest, "Nearest");
                    ui.selectable_value(&mut current, VideoFilter::Linear, "Linear");
                    ui.selectable_value(&mut current, VideoFilter::Crt, "CRT");
                });
            if current != *filter {
                *filter = current;
            }

            // CRT shader parameters (only when CRT filter is active)
            if *filter == VideoFilter::Crt {
                ui.separator();
                ui.heading("CRT");
                if let Some(mat) = materials.get_mut(&crt_handle.0) {
                    let mut scanlines = mat.params.red;
                    let mut curvature = mat.params.green;
                    let mut vignette = mat.params.blue;
                    let mut brightness = mat.params.alpha;

                    ui.add(egui::Slider::new(&mut scanlines, 0.0..=1.0).text("Scanlines"));
                    ui.add(egui::Slider::new(&mut curvature, 0.0..=1.0).text("Curvature"));
                    ui.add(egui::Slider::new(&mut vignette, 0.0..=1.0).text("Vignette"));
                    ui.add(egui::Slider::new(&mut brightness, 0.5..=2.0).text("Brightness"));

                    mat.params.red = scanlines;
                    mat.params.green = curvature;
                    mat.params.blue = vignette;
                    mat.params.alpha = brightness;
                }
            }
        });
}

/// Apply video filter changes: update image sampler and CRT material params.
fn apply_video_filter(
    filter: Res<VideoFilter>,
    fb_handle: Res<FramebufferHandle>,
    crt_handle: Res<CrtMaterialHandle>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<CrtMaterial>>,
) {
    if !filter.is_changed() {
        return;
    }

    // Set texture sampler based on filter
    if let Some(image) = images.get_mut(&fb_handle.0) {
        image.sampler = match *filter {
            VideoFilter::Nearest => ImageSampler::Descriptor(ImageSamplerDescriptor::nearest()),
            VideoFilter::Linear | VideoFilter::Crt => {
                ImageSampler::Descriptor(ImageSamplerDescriptor::linear())
            }
        };
    }

    // Set CRT shader params
    if let Some(mat) = materials.get_mut(&crt_handle.0) {
        match *filter {
            VideoFilter::Nearest | VideoFilter::Linear => {
                mat.params.red = 0.0;
                mat.params.green = 0.0;
                mat.params.blue = 0.0;
                mat.params.alpha = 1.0;
            }
            VideoFilter::Crt => {
                mat.params.red = 0.7;
                mat.params.green = 0.4;
                mat.params.blue = 0.6;
                mat.params.alpha = 1.3;
            }
        }
    }
}

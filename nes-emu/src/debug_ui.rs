use std::time::SystemTime;

use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::{EguiContexts, EguiPlugin};

use crate::crt::CrtMaterial;
use crate::emulation::{CartridgeInfo, NesInput, NesSystem};
use crate::video::{CrtMaterialHandle, FramebufferHandle};

/// Width of the debug side panel in logical pixels.
pub const PANEL_WIDTH: f32 = 250.0;

/// Holds the in-memory save state (single slot).
#[derive(Resource, Default)]
pub struct SaveSlot {
    pub data: Option<Vec<u8>>,
}

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

#[derive(Event)]
struct SaveStateEvent;

#[derive(Event)]
struct LoadStateEvent;

#[derive(Resource)]
struct DebugUiVisible(bool);

#[derive(Resource, Default)]
struct FpsDisplay {
    value: f32,
    elapsed: f32,
    frames: u32,
}

pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .insert_resource(DebugUiVisible(true))
            .init_resource::<VideoFilter>()
            .init_resource::<FpsDisplay>()
            .init_resource::<SaveSlot>()
            .add_event::<SaveStateEvent>()
            .add_event::<LoadStateEvent>()
            .add_systems(Update, toggle_debug_ui)
            .add_systems(
                Update,
                (draw_debug_ui, apply_video_filter)
                    .chain()
                    .after(toggle_debug_ui),
            )
            .add_systems(Update, handle_save_load.after(draw_debug_ui));
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
    cart: Res<CartridgeInfo>,
    fb_handle: Res<FramebufferHandle>,
    images: Res<Assets<Image>>,
    crt_handle: Res<CrtMaterialHandle>,
    mut materials: ResMut<Assets<CrtMaterial>>,
    nes_input: Res<NesInput>,
    mut filter: ResMut<VideoFilter>,
    mut contexts: EguiContexts,
    time: Res<Time>,
    mut fps_display: ResMut<FpsDisplay>,
    save_slot: Res<SaveSlot>,
    mut save_events: EventWriter<SaveStateEvent>,
    mut load_events: EventWriter<LoadStateEvent>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    // Keyboard shortcuts work even when panel is hidden
    let sav_path = save_state_path(&cart);
    let can_load = save_slot.data.is_some() || sav_path.exists();
    if keys.just_pressed(KeyCode::F5) {
        save_events.send(SaveStateEvent);
    }
    if keys.just_pressed(KeyCode::F7) && can_load {
        load_events.send(LoadStateEvent);
    }
    if keys.just_pressed(KeyCode::F12) {
        save_screenshot(&fb_handle, &images);
    }

    if !visible.0 {
        return;
    }

    fps_display.elapsed += time.delta_secs();
    fps_display.frames += 1;
    if fps_display.elapsed >= 1.0 {
        fps_display.value = fps_display.frames as f32 / fps_display.elapsed;
        fps_display.elapsed = 0.0;
        fps_display.frames = 0;
    }

    let ctx = contexts.ctx_mut();

    egui::SidePanel::right("debug_panel")
        .exact_width(PANEL_WIDTH)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Play");
            ui.label(format!("FPS: {:.1}", fps_display.value));
            if ui.button("Screenshot (F12)").clicked() {
                save_screenshot(&fb_handle, &images);
            }
            ui.horizontal(|ui| {
                if ui.button("Save (F5)").clicked() {
                    save_events.send(SaveStateEvent);
                }
                ui.add_enabled_ui(can_load, |ui| {
                    if ui.button("Load (F7)").clicked() {
                        load_events.send(LoadStateEvent);
                    }
                });
            });
            ui.separator();

            // Cartridge info
            ui.heading("Cartridge");
            ui.monospace(&cart.file_name);
            ui.monospace(format!("PRG: {}KB  CHR: {}KB", cart.prg_kb, cart.chr_kb));
            ui.monospace(format!("Mapper: {}  {}", cart.mapper, cart.mirroring));
            if cart.battery {
                ui.monospace("Battery: yes");
            }
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

            // APU state
            ui.heading("APU");
            let apu = nes.sys.apu.borrow();
            ui.monospace(format!(
                "P1:{:>4} L:{:>2}  P2:{:>4} L:{:>2}",
                apu.pulse1.timer_period,
                apu.pulse1.length_counter.counter,
                apu.pulse2.timer_period,
                apu.pulse2.length_counter.counter,
            ));
            ui.monospace(format!(
                "Tri:{:>4} L:{:>2}  Noi: L:{:>2}",
                apu.triangle.timer_period,
                apu.triangle.length_counter.counter,
                apu.noise.length_counter.counter,
            ));
            ui.monospace(format!(
                "DMC: lvl:{:>3} {}",
                apu.dmc.output_level,
                if apu.dmc.enabled { "ON" } else { "OFF" },
            ));
            drop(apu);
            ui.separator();

            // Joypad state
            ui.heading("Joypad");
            let btns = nes_input.0;
            let btn_str = |bit: u8, name: &str| -> String {
                if btns & (1 << bit) != 0 {
                    name.to_uppercase()
                } else {
                    ".".to_string()
                }
            };
            ui.monospace(format!(
                "{}  {}  {}  {}  {}  {}  {}  {}",
                btn_str(0, "A"),
                btn_str(1, "B"),
                btn_str(2, "Sel"),
                btn_str(3, "Sta"),
                btn_str(4, "U"),
                btn_str(5, "D"),
                btn_str(6, "L"),
                btn_str(7, "R"),
            ));
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

fn save_screenshot(fb_handle: &FramebufferHandle, images: &Assets<Image>) {
    let Some(image) = images.get(&fb_handle.0) else {
        eprintln!("Screenshot: framebuffer not available");
        return;
    };

    let width = image.width();
    let height = image.height();

    let Some(buf) = image::RgbaImage::from_raw(width, height, image.data.clone()) else {
        eprintln!("Screenshot: failed to create image buffer");
        return;
    };

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let path = format!("screenshot_{timestamp}.png");
    match buf.save(&path) {
        Ok(()) => println!("Screenshot saved: {path}"),
        Err(e) => eprintln!("Screenshot failed: {e}"),
    }
}

fn save_state_path(cart: &CartridgeInfo) -> std::path::PathBuf {
    let stem = std::path::Path::new(&cart.file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string());
    std::path::PathBuf::from(format!("{stem}.sav"))
}

fn handle_save_load(
    mut nes: NonSendMut<NesSystem>,
    cart: Res<CartridgeInfo>,
    mut save_slot: ResMut<SaveSlot>,
    mut save_events: EventReader<SaveStateEvent>,
    mut load_events: EventReader<LoadStateEvent>,
) {
    let path = save_state_path(&cart);

    for _ in save_events.read() {
        let data = crate::save_state::save(&nes);
        let size = data.len();
        match std::fs::write(&path, &data) {
            Ok(()) => println!("State saved to {} ({} bytes)", path.display(), size),
            Err(e) => eprintln!("Failed to save state: {e}"),
        }
        save_slot.data = Some(data);
    }

    for _ in load_events.read() {
        // Try loading from memory first, then from file
        let data = if let Some(data) = &save_slot.data {
            data.clone()
        } else {
            match std::fs::read(&path) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Failed to read state file: {e}");
                    return;
                }
            }
        };
        match crate::save_state::load(&mut nes, &data) {
            Ok(()) => println!("State loaded from {}", path.display()),
            Err(e) => eprintln!("Failed to load state: {e}"),
        }
    }
}

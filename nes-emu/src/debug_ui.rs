use std::time::SystemTime;

use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::{EguiContexts, EguiPlugin};
use egui_plot::{Line, PlotPoints};

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

            // Status flags with colored indicators
            draw_flags(ui, cpu.p);

            ui.monospace(format!("Cycles: {}", cpu.cycles));

            // Mini disassembly: current instruction + next few
            ui.add_space(2.0);
            draw_disassembly(ui, cpu);

            // Stack peek
            draw_stack(ui, cpu);
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

            // Decoded scroll position
            let scroll_x = ((ppu.v & 0x1F) << 3) | ppu.fine_x as u16;
            let scroll_y = ((ppu.v >> 5) & 0x1F) << 3 | ((ppu.v >> 12) & 0x07);
            let nt = (ppu.v >> 10) & 0x03;
            ui.monospace(format!("Scroll: X:{scroll_x:>3} Y:{scroll_y:>3} NT:{nt}"));

            // Rendering flags decoded from MASK
            let show_bg = ppu.mask & 0x08 != 0;
            let show_spr = ppu.mask & 0x10 != 0;
            let grey = ppu.mask & 0x01 != 0;
            ui.monospace(format!(
                "BG:{}  SPR:{}{}",
                if show_bg { "ON " } else { "off" },
                if show_spr { "ON " } else { "off" },
                if grey { "  GREY" } else { "" },
            ));

            // Sprite info
            let spr_h = if ppu.ctrl & 0x20 != 0 { 16 } else { 8 };
            let active_sprites = (0..64)
                .filter(|&i| {
                    let y = ppu.oam[i * 4] as u16;
                    y < 240
                })
                .count();
            ui.monospace(format!("Sprites: {active_sprites}/64  H:{spr_h}"));

            // Palette visualization
            ui.add_space(2.0);
            draw_palette(ui, &ppu.palette);

            drop(ppu);
            ui.separator();

            // APU waveforms
            ui.heading("APU");
            let apu = nes.sys.apu.borrow();
            let waveforms = &apu.channel_waveforms;
            let channels: [(&str, egui::Color32, &nes_apu::apu::WaveformRing); 5] = [
                (
                    "Pulse 1",
                    egui::Color32::from_rgb(0x4C, 0xAF, 0x50),
                    &waveforms.pulse1,
                ),
                (
                    "Pulse 2",
                    egui::Color32::from_rgb(0x29, 0x96, 0xF3),
                    &waveforms.pulse2,
                ),
                (
                    "Triangle",
                    egui::Color32::from_rgb(0xFF, 0x98, 0x00),
                    &waveforms.triangle,
                ),
                (
                    "Noise",
                    egui::Color32::from_rgb(0xAB, 0x47, 0xBC),
                    &waveforms.noise,
                ),
                (
                    "DMC",
                    egui::Color32::from_rgb(0xEF, 0x53, 0x50),
                    &waveforms.dmc,
                ),
            ];
            const DISPLAY_SAMPLES: usize = 256;
            for (label, color, ring) in &channels {
                ui.small(*label);
                let (buf, write_pos) = ring.as_slice_ordered();
                let start = (write_pos + buf.len() - DISPLAY_SAMPLES) % buf.len();
                let points: PlotPoints = (0..DISPLAY_SAMPLES)
                    .map(|i| {
                        let idx = (start + i) % buf.len();
                        [i as f64, buf[idx] as f64]
                    })
                    .collect();
                let line = Line::new(points).color(*color);
                egui_plot::Plot::new(*label)
                    .height(40.0)
                    .show_axes(false)
                    .show_grid(false)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .allow_scroll(false)
                    .allow_boxed_zoom(false)
                    .include_y(0.0)
                    .include_y(1.0)
                    .set_margin_fraction(egui::Vec2::ZERO)
                    .show(ui, |plot_ui| {
                        plot_ui.line(line);
                    });
            }
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
            draw_joypad(ui, btns);
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

/// Draw an NES controller layout showing button states.
///
/// Layout (approximate):
/// ```text
///        [U]              [Sel] [Sta]          [B] [A]
///     [L] + [R]
///        [D]
/// ```
/// Draw CPU status flags as colored labels.
fn draw_flags(ui: &mut egui::Ui, p: u8) {
    let on = egui::Color32::from_rgb(0x4C, 0xAF, 0x50);
    let off = egui::Color32::from_gray(80);
    let flags: [(&str, u8); 7] = [
        ("N", 7),
        ("V", 6),
        ("D", 3),
        ("I", 2),
        ("Z", 1),
        ("C", 0),
        ("B", 4),
    ];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for (name, bit) in &flags {
            let set = (p >> bit) & 1 != 0;
            let color = if set { on } else { off };
            ui.colored_label(color, egui::RichText::new(*name).monospace().size(12.0));
        }
    });
}

/// Draw a mini disassembly view around the current PC.
fn draw_disassembly(ui: &mut egui::Ui, cpu: &nes_cpu::cpu::Cpu) {
    use nes_cpu::opcodes::{get_opcodes, instr_byte_count};

    let opcodes = get_opcodes();
    let mut addr = cpu.pc;
    let highlight = egui::Color32::from_rgb(0xFF, 0xD5, 0x4F);

    for i in 0..6 {
        let opcode = cpu.bus.peek(addr);
        let instr = &opcodes[opcode as usize];
        let size = instr_byte_count(instr.mode);

        // Format: ADDR  BYTES  MNEMONIC OPERAND
        let bytes_str = match size {
            1 => format!("{:02X}      ", opcode),
            2 => format!(
                "{:02X} {:02X}   ",
                opcode,
                cpu.bus.peek(addr.wrapping_add(1))
            ),
            3 => format!(
                "{:02X} {:02X} {:02X}",
                opcode,
                cpu.bus.peek(addr.wrapping_add(1)),
                cpu.bus.peek(addr.wrapping_add(2))
            ),
            _ => String::new(),
        };

        let operand = format_operand(cpu, addr, instr.mode);
        let line = format!("{:04X} {} {} {}", addr, bytes_str, instr.mnemonic, operand);

        let text = egui::RichText::new(line).monospace().size(10.0);
        if i == 0 {
            ui.colored_label(highlight, text);
        } else {
            ui.label(text);
        }

        addr = addr.wrapping_add(size as u16);
    }
}

/// Format an instruction operand for display.
fn format_operand(cpu: &nes_cpu::cpu::Cpu, addr: u16, mode: nes_cpu::opcodes::AddrMode) -> String {
    use nes_cpu::opcodes::AddrMode;

    let lo = || cpu.bus.peek(addr.wrapping_add(1));
    let word = || {
        let l = cpu.bus.peek(addr.wrapping_add(1)) as u16;
        let h = cpu.bus.peek(addr.wrapping_add(2)) as u16;
        h << 8 | l
    };

    match mode {
        AddrMode::Implied | AddrMode::Accumulator => String::new(),
        AddrMode::Immediate => format!("#${:02X}", lo()),
        AddrMode::ZeroPage => format!("${:02X}", lo()),
        AddrMode::ZeroPageX => format!("${:02X},X", lo()),
        AddrMode::ZeroPageY => format!("${:02X},Y", lo()),
        AddrMode::Absolute => format!("${:04X}", word()),
        AddrMode::AbsoluteX => format!("${:04X},X", word()),
        AddrMode::AbsoluteY => format!("${:04X},Y", word()),
        AddrMode::Indirect => format!("(${:04X})", word()),
        AddrMode::IndirectX => format!("(${:02X},X)", lo()),
        AddrMode::IndirectY => format!("(${:02X}),Y", lo()),
        AddrMode::Relative => {
            let offset = lo() as i8;
            let target = addr.wrapping_add(2).wrapping_add(offset as u16);
            format!("${:04X}", target)
        }
    }
}

/// Draw top of the stack.
fn draw_stack(ui: &mut egui::Ui, cpu: &nes_cpu::cpu::Cpu) {
    let sp = cpu.sp;
    let depth = (0xFFu16).wrapping_sub(sp as u16).min(8) as u8;
    if depth == 0 {
        return;
    }
    ui.add_space(2.0);
    ui.small("Stack");
    let mut line = String::new();
    for i in 0..depth {
        let stack_addr = 0x0100u16 | (sp.wrapping_add(1 + i)) as u16;
        let val = cpu.bus.peek(stack_addr);
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&format!("{:02X}", val));
    }
    ui.monospace(&line);
}

fn draw_palette(ui: &mut egui::Ui, palette: &[u8; 32]) {
    use nes_ppu::ppu::NES_PALETTE;

    let swatch = 12.0;
    let gap = 1.0;
    let row_h = swatch + gap;
    let total_w = swatch * 4.0 + gap * 3.0;

    // BG palettes (0-3) on first row, Sprite palettes (4-7) on second row
    let labels = ["BG", "SPR"];
    for (row, label) in labels.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.monospace(*label);
            let start_idx = row * 16;
            // 4 sub-palettes, each 4 colors
            let desired = egui::vec2(total_w * 4.0 + gap * 6.0, row_h);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            let painter = ui.painter_at(rect);

            for pal in 0..4 {
                let base_x = rect.left() + pal as f32 * (total_w + gap * 2.0);
                for col in 0..4 {
                    let nes_color = palette[start_idx + pal * 4 + col] as usize & 0x3F;
                    let rgb = NES_PALETTE[nes_color];
                    let r = ((rgb >> 16) & 0xFF) as u8;
                    let g = ((rgb >> 8) & 0xFF) as u8;
                    let b = (rgb & 0xFF) as u8;
                    let color = egui::Color32::from_rgb(r, g, b);

                    let x = base_x + col as f32 * (swatch + gap);
                    let swatch_rect = egui::Rect::from_min_size(
                        egui::pos2(x, rect.top()),
                        egui::vec2(swatch, swatch),
                    );
                    painter.rect_filled(swatch_rect, 1.0, color);
                }
            }
        });
    }
}

fn draw_joypad(ui: &mut egui::Ui, btns: u8) {
    let pressed = |bit: u8| btns & (1 << bit) != 0;

    let on_color = egui::Color32::from_rgb(0xEF, 0x53, 0x50);
    let off_color = egui::Color32::from_gray(60);

    let btn_color = |bit: u8| {
        if pressed(bit) {
            on_color
        } else {
            off_color
        }
    };

    // Reserve space for the joypad graphic
    let desired = egui::vec2(ui.available_width(), 58.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Draw controller body
    let body = rect.shrink2(egui::vec2(4.0, 5.0));
    painter.rect_filled(body, 6.0, egui::Color32::from_gray(35));

    // D-pad dimensions
    let dpad_cx = body.left() + 40.0;
    let dpad_cy = body.center().y;
    let s = 7.0; // half-size of each dpad button
    let gap = 1.0;

    // D-pad cross background
    let cross_h = egui::Rect::from_center_size(
        egui::pos2(dpad_cx, dpad_cy),
        egui::vec2(s * 6.0 + gap * 2.0, s * 2.0),
    );
    let cross_v = egui::Rect::from_center_size(
        egui::pos2(dpad_cx, dpad_cy),
        egui::vec2(s * 2.0, s * 6.0 + gap * 2.0),
    );
    painter.rect_filled(cross_h, 2.0, egui::Color32::from_gray(25));
    painter.rect_filled(cross_v, 2.0, egui::Color32::from_gray(25));

    // D-pad buttons: U=4, D=5, L=6, R=7
    let dpad_btn = |cx: f32, cy: f32, bit: u8| {
        let r = egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(s * 2.0, s * 2.0));
        painter.rect_filled(r, 2.0, btn_color(bit));
    };
    dpad_btn(dpad_cx, dpad_cy - s * 2.0 - gap, 4); // Up
    dpad_btn(dpad_cx, dpad_cy + s * 2.0 + gap, 5); // Down
    dpad_btn(dpad_cx - s * 2.0 - gap, dpad_cy, 6); // Left
    dpad_btn(dpad_cx + s * 2.0 + gap, dpad_cy, 7); // Right

    // Select (2) and Start (3) — small rounded rects in the middle
    let mid_x = body.center().x;
    let mid_y = body.center().y + 4.0;
    let pill = egui::vec2(26.0, 10.0);

    let sel_rect = egui::Rect::from_center_size(egui::pos2(mid_x - 18.0, mid_y), pill);
    painter.rect_filled(sel_rect, 4.0, btn_color(2));
    painter.text(
        sel_rect.center(),
        egui::Align2::CENTER_CENTER,
        "S",
        egui::FontId::proportional(7.0),
        egui::Color32::WHITE,
    );

    let sta_rect = egui::Rect::from_center_size(egui::pos2(mid_x + 18.0, mid_y), pill);
    painter.rect_filled(sta_rect, 4.0, btn_color(3));
    painter.text(
        sta_rect.center(),
        egui::Align2::CENTER_CENTER,
        "S",
        egui::FontId::proportional(7.0),
        egui::Color32::WHITE,
    );

    // B (1) and A (0) — circles on the right
    let btn_r = 12.0;
    let ab_y = body.center().y;
    let a_cx = body.right() - 25.0;
    let b_cx = a_cx - 30.0;

    painter.circle_filled(egui::pos2(b_cx, ab_y), btn_r, btn_color(1));
    painter.text(
        egui::pos2(b_cx, ab_y),
        egui::Align2::CENTER_CENTER,
        "B",
        egui::FontId::proportional(11.0),
        egui::Color32::WHITE,
    );

    painter.circle_filled(egui::pos2(a_cx, ab_y), btn_r, btn_color(0));
    painter.text(
        egui::pos2(a_cx, ab_y),
        egui::Align2::CENTER_CENTER,
        "A",
        egui::FontId::proportional(11.0),
        egui::Color32::WHITE,
    );
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

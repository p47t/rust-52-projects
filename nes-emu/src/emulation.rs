use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait};
use nes_cpu::ines::INesRom;

use crate::crt::CrtMaterial;
use crate::video::{CrtMaterialHandle, FramebufferHandle};

/// Maximum audio buffer size (in samples) to prevent runaway growth.
const MAX_AUDIO_BUFFER: usize = 8192;

/// NTSC NES frame duration: ~16.6393ms (60.0988 FPS).
const NTSC_FRAME_SECS: f64 = 1.0 / 60.0988;

/// Wrapper for the NES system. This is !Send due to Rc<RefCell> internals,
/// so it must be stored as a non-send resource and accessed via NonSendMut.
pub struct NesSystem {
    pub sys: nes_apu::system::System,
}

/// ROM file path passed from CLI.
#[derive(Resource)]
pub struct RomPath(pub String);

/// Shared audio ring buffer between emulation (producer) and cpal (consumer).
#[derive(Resource, Clone)]
pub struct AudioBuffer {
    pub buffer: Arc<Mutex<VecDeque<f32>>>,
}

/// Current joypad button state from input system.
#[derive(Resource, Default)]
pub struct NesInput(pub u8);

/// Accumulates real time and runs NES frames to match, decoupling emulation
/// speed from display refresh rate.
#[derive(Resource)]
pub struct EmulationTimer {
    accumulator: f64,
}

impl Default for EmulationTimer {
    fn default() -> Self {
        Self {
            // Start with one frame's worth so the first update produces a frame immediately.
            accumulator: NTSC_FRAME_SECS,
        }
    }
}

/// Cartridge metadata extracted from the iNES ROM header.
#[derive(Resource)]
pub struct CartridgeInfo {
    pub file_name: String,
    pub prg_kb: usize,
    pub chr_kb: usize,
    pub mapper: u8,
    pub mirroring: String,
    pub battery: bool,
}

/// Exclusive startup system that creates the NES system as a non-send resource.
pub fn setup_emulation(world: &mut World) {
    let rom_path = world.resource::<RomPath>().0.clone();
    let rom = INesRom::load(&rom_path).unwrap_or_else(|e| {
        panic!("Failed to load ROM '{}': {}", rom_path, e);
    });

    let file_name = std::path::Path::new(&rom_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| rom_path.clone());

    println!("Loaded: {}", rom_path);
    println!(
        "PRG: {}KB, CHR: {}KB, Mapper: {}, Mirroring: {:?}",
        rom.prg_rom.len() / 1024,
        rom.chr_rom.len() / 1024,
        rom.mapper,
        rom.mirroring
    );

    world.insert_resource(CartridgeInfo {
        file_name,
        prg_kb: rom.prg_rom.len() / 1024,
        chr_kb: rom.chr_rom.len() / 1024,
        mapper: rom.mapper,
        mirroring: format!("{:?}", rom.mirroring),
        battery: rom.has_battery,
    });

    // Query the audio device sample rate for APU downsampling
    let sample_rate = cpal::default_host()
        .default_output_device()
        .and_then(|d| d.default_output_config().ok())
        .map(|c| c.sample_rate().0 as f64)
        .unwrap_or(44100.0);

    let sys =
        nes_apu::system::System::from_rom(rom, sample_rate).expect("Failed to create NES system");

    world.insert_non_send_resource(NesSystem { sys });
}

/// Per-frame system: accumulates real time and runs enough NES frames to keep
/// emulation in sync, regardless of display refresh rate.
#[allow(clippy::too_many_arguments)]
pub fn run_emulation_frame(
    mut nes: NonSendMut<NesSystem>,
    nes_input: Res<NesInput>,
    audio_buf: Res<AudioBuffer>,
    fb_handle: Res<FramebufferHandle>,
    crt_handle: Res<CrtMaterialHandle>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<CrtMaterial>>,
    mut timer: ResMut<EmulationTimer>,
    time: Res<Time>,
) {
    timer.accumulator += time.delta_secs_f64();

    // Cap accumulator to prevent spiral of death (e.g. after window drag stall).
    if timer.accumulator > NTSC_FRAME_SECS * 4.0 {
        timer.accumulator = NTSC_FRAME_SECS * 4.0;
    }

    let mut ran_frame = false;
    while timer.accumulator >= NTSC_FRAME_SECS {
        timer.accumulator -= NTSC_FRAME_SECS;

        // Feed input
        nes.sys.joypad1.borrow_mut().set_buttons(nes_input.0);

        // Run one frame of emulation
        nes.sys.run_until_frame();
        ran_frame = true;

        // Feed audio samples to the shared ring buffer
        let samples = nes.sys.apu.borrow_mut().drain_samples();
        if !samples.is_empty() {
            if let Ok(mut buf) = audio_buf.buffer.lock() {
                for &s in &samples {
                    if buf.len() < MAX_AUDIO_BUFFER {
                        buf.push_back(s);
                    }
                }
            }
        }
    }

    // Only update the GPU texture when at least one NES frame ran
    if ran_frame {
        if let Some(image) = images.get_mut(&fb_handle.0) {
            let ppu = nes.sys.ppu.borrow();
            let fb = &*ppu.framebuffer;
            let data = &mut image.data;
            for (i, &pixel) in fb.iter().enumerate() {
                let offset = i * 4;
                data[offset] = ((pixel >> 16) & 0xFF) as u8; // R
                data[offset + 1] = ((pixel >> 8) & 0xFF) as u8; // G
                data[offset + 2] = (pixel & 0xFF) as u8; // B
                data[offset + 3] = 255; // A
            }
            // Touch the material to force bind group recreation with the updated GpuImage.
            let _ = materials.get_mut(&crt_handle.0);
        }
    }
}

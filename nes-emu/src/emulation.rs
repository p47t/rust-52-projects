use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait};
use nes_cpu::ines::INesRom;

use crate::crt::CrtMaterial;
use crate::video::{CrtMaterialHandle, FramebufferHandle};

/// Maximum audio buffer size (in samples) to prevent runaway growth.
const MAX_AUDIO_BUFFER: usize = 8192;

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

/// Exclusive startup system that creates the NES system as a non-send resource.
pub fn setup_emulation(world: &mut World) {
    let rom_path = world.resource::<RomPath>().0.clone();
    let rom = INesRom::load(&rom_path).unwrap_or_else(|e| {
        panic!("Failed to load ROM '{}': {}", rom_path, e);
    });

    println!("Loaded: {}", rom_path);
    println!(
        "PRG: {}KB, CHR: {}KB, Mapper: {}, Mirroring: {:?}",
        rom.prg_rom.len() / 1024,
        rom.chr_rom.len() / 1024,
        rom.mapper,
        rom.mirroring
    );

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

/// Per-frame system: runs one NES frame, copies framebuffer, feeds audio.
pub fn run_emulation_frame(
    mut nes: NonSendMut<NesSystem>,
    nes_input: Res<NesInput>,
    audio_buf: Res<AudioBuffer>,
    fb_handle: Res<FramebufferHandle>,
    crt_handle: Res<CrtMaterialHandle>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<CrtMaterial>>,
) {
    // Feed input
    nes.sys.joypad1.borrow_mut().set_buttons(nes_input.0);

    // Run one frame of emulation
    nes.sys.run_until_frame();

    // Copy PPU framebuffer (u32 0x00RRGGBB) to Bevy Image (RGBA8 bytes)
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
        // Without this, Material2d caches a stale bind group that references a freed GPU texture.
        let _ = materials.get_mut(&crt_handle.0);
    }

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

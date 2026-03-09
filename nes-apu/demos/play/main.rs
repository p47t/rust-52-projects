#![cfg(feature = "demos")]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use minifb::{Key, Window, WindowOptions};
use nes_cpu::ines::INesRom;
use nes_joypad::input::keyboard_to_buttons;

use nes_apu::system::System;

const WIDTH: usize = 256;
const HEIGHT: usize = 240;
const SCALE: usize = 3;

/// Maximum audio buffer size (in samples) to prevent runaway growth.
const MAX_AUDIO_BUFFER: usize = 8192;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let rom_path = args.get(1).context("Usage: nes-game <rom-path>")?;

    let rom =
        INesRom::load(rom_path).with_context(|| format!("Failed to load ROM: {}", rom_path))?;

    println!("Loaded: {}", rom_path);
    println!(
        "PRG: {}KB, CHR: {}KB, Mapper: {}, Mirroring: {:?}",
        rom.prg_rom.len() / 1024,
        rom.chr_rom.len() / 1024,
        rom.mapper,
        rom.mirroring
    );

    // Set up audio output
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("No audio output device found")?;
    let supported_config = device
        .default_output_config()
        .context("Failed to get default audio config")?;

    let sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels() as usize;
    println!("Audio: {}Hz, {} channels", sample_rate, channels);

    // Create the NES system with the audio sample rate
    let mut sys = System::from_rom(rom, sample_rate as f64)?;

    // Shared audio ring buffer (Arc<Mutex> because cpal callback runs on a separate thread)
    let audio_buffer: Arc<Mutex<VecDeque<f32>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(MAX_AUDIO_BUFFER)));
    let audio_buffer_playback = Arc::clone(&audio_buffer);

    let config: cpal::StreamConfig = supported_config.into();
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buf = audio_buffer_playback.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    let sample = buf.pop_front().unwrap_or(0.0);
                    for s in frame.iter_mut() {
                        *s = sample;
                    }
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )
        .context("Failed to build audio stream")?;

    stream.play().context("Failed to start audio stream")?;

    // Set up gamepad input
    #[cfg(feature = "gamepad")]
    let mut gilrs_ctx = match gilrs::Gilrs::new() {
        Ok(g) => {
            println!("Gamepad subsystem initialized");
            Some(g)
        }
        Err(e) => {
            eprintln!("Failed to initialize gamepad subsystem: {}", e);
            None
        }
    };
    #[cfg(feature = "gamepad")]
    let gamepad_id = gilrs_ctx.as_ref().and_then(|g| {
        let mut found = None;
        for (id, gp) in g.gamepads() {
            println!("  Found gamepad [{}]: {}", id, gp.name());
            if found.is_none() {
                found = Some(id);
            }
        }
        if found.is_none() {
            println!("  No gamepads detected (keyboard-only mode)");
        }
        found
    });

    // Create window
    let mut window = Window::new(
        "NES Player",
        WIDTH * SCALE,
        HEIGHT * SCALE,
        WindowOptions {
            resize: true,
            scale_mode: minifb::ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .context("Failed to create window")?;

    window.set_target_fps(60);

    println!("Controls: Arrow keys = D-pad, Z = A, X = B, A = Select, S = Start");
    println!("Press Escape to quit");

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Poll input
        #[allow(unused_mut)]
        let mut p1_state = keyboard_to_buttons(&window);

        #[cfg(feature = "gamepad")]
        if let (Some(ref mut gilrs), Some(gp_id)) = (&mut gilrs_ctx, gamepad_id) {
            while gilrs.next_event().is_some() {}
            p1_state |= nes_joypad::input::gamepad_to_buttons(gilrs, gp_id);
        }

        sys.joypad1.borrow_mut().set_buttons(p1_state);

        // Run one frame of emulation
        sys.run_until_frame();

        // Feed audio samples to the ring buffer
        let samples = sys.apu.borrow_mut().drain_samples();
        {
            let mut buf = audio_buffer.lock().unwrap();
            for &s in &samples {
                if buf.len() < MAX_AUDIO_BUFFER {
                    buf.push_back(s);
                }
            }
        }

        // Display
        let ppu = sys.ppu.borrow();
        window
            .update_with_buffer(&*ppu.framebuffer, WIDTH, HEIGHT)
            .context("Failed to update window buffer")?;
    }

    Ok(())
}

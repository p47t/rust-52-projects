#![cfg(feature = "demos")]

use anyhow::Context;
use minifb::{Key, Window, WindowOptions};
use nes_cpu::ines::INesRom;
use nes_joypad::input::keyboard_to_buttons;
use nes_joypad::system::System;

const WIDTH: usize = 256;
const HEIGHT: usize = 240;
const SCALE: usize = 3;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let rom_path = args.get(1).context("Usage: nes-play <rom-path>")?;

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
    anyhow::ensure!(rom.mapper == 0, "Only mapper 0 (NROM) is supported");

    let mut sys = System::from_rom(rom);

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

        // Display
        let ppu = sys.ppu.borrow();
        window
            .update_with_buffer(&*ppu.framebuffer, WIDTH, HEIGHT)
            .context("Failed to update window buffer")?;
    }

    Ok(())
}

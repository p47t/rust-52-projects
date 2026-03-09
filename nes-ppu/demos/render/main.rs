#![cfg(feature = "demos")]

use anyhow::Context;
use minifb::{Key, Window, WindowOptions};
use nes_cpu::ines::INesRom;
use nes_ppu::system::System;

const WIDTH: usize = 256;
const HEIGHT: usize = 240;
const SCALE: usize = 3;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let rom_path = args.get(1).context("Usage: nes-render <rom-path>")?;

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

    let mut sys = System::from_rom(rom)?;

    let mut window = Window::new(
        "NES PPU Render",
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

    while window.is_open() && !window.is_key_down(Key::Escape) {
        sys.run_until_frame();

        let ppu = sys.ppu.borrow();
        window
            .update_with_buffer(&*ppu.framebuffer, WIDTH, HEIGHT)
            .context("Failed to update window buffer")?;
    }

    Ok(())
}

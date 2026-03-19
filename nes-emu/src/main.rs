mod audio;
mod emulation;
mod input;
mod video;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::window::PresentMode;

use emulation::{AudioBuffer, NesInput, RomPath};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rom_path = args.get(1).expect("Usage: nes-emu <rom-path>").to_string();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "NES Emulator".to_string(),
                resolution: (768.0, 720.0).into(), // 256*3, 240*3
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(RomPath(rom_path))
        .insert_resource(AudioBuffer {
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(8192))),
        })
        .init_resource::<NesInput>()
        .add_systems(
            Startup,
            (
                emulation::setup_emulation,
                video::setup_video,
                audio::setup_audio,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (input::read_input, emulation::run_emulation_frame).chain(),
        )
        .run();
}

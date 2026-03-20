mod audio;
mod crt;
mod debug_ui;
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
                resolution: (768.0 + debug_ui::PANEL_WIDTH, 720.0).into(),
                present_mode: PresentMode::AutoVsync,
                // Transparent window prevents OS from painting a white background
                // before Bevy's first render. ClearColor::BLACK fills it immediately.
                transparent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(crt::CrtPlugin)
        .add_plugins(debug_ui::DebugUiPlugin)
        .insert_resource(ClearColor(Color::BLACK))
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

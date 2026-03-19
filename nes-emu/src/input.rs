use bevy::prelude::*;
use nes_joypad::joypad::Button;

use crate::emulation::NesInput;

/// Update system: maps Bevy keyboard input to NES joypad button bitfield.
///
/// Default mapping (same as existing NES demos):
/// - Arrow keys: D-pad
/// - Z: A, X: B
/// - A: Select, S: Start
pub fn read_input(keys: Res<ButtonInput<KeyCode>>, mut nes_input: ResMut<NesInput>) {
    let mut state = 0u8;

    if keys.pressed(KeyCode::ArrowUp) {
        state |= 1 << Button::Up as u8;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        state |= 1 << Button::Down as u8;
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        state |= 1 << Button::Left as u8;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        state |= 1 << Button::Right as u8;
    }
    if keys.pressed(KeyCode::KeyZ) {
        state |= 1 << Button::A as u8;
    }
    if keys.pressed(KeyCode::KeyX) {
        state |= 1 << Button::B as u8;
    }
    if keys.pressed(KeyCode::KeyA) {
        state |= 1 << Button::Select as u8;
    }
    if keys.pressed(KeyCode::KeyS) {
        state |= 1 << Button::Start as u8;
    }

    nes_input.0 = state;
}

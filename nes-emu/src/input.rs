use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use nes_joypad::joypad::Button;

use crate::emulation::NesInput;

/// Update system: maps keyboard and gamepad input to NES joypad button bitfield.
///
/// Keyboard mapping:
/// - Arrow keys: D-pad
/// - Z: A, X: B
/// - A: Select, S: Start
///
/// Gamepad mapping:
/// - D-pad / Left stick: D-pad
/// - South (A/Cross): A, East (B/Circle): B
/// - Select/Back: Select, Start: Start
pub fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut nes_input: ResMut<NesInput>,
) {
    let mut state = 0u8;

    // Keyboard
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

    // Gamepad (use the first connected gamepad)
    if let Some(gamepad) = gamepads.iter().next() {
        if gamepad.pressed(GamepadButton::DPadUp) {
            state |= 1 << Button::Up as u8;
        }
        if gamepad.pressed(GamepadButton::DPadDown) {
            state |= 1 << Button::Down as u8;
        }
        if gamepad.pressed(GamepadButton::DPadLeft) {
            state |= 1 << Button::Left as u8;
        }
        if gamepad.pressed(GamepadButton::DPadRight) {
            state |= 1 << Button::Right as u8;
        }

        // Left stick as D-pad (with deadzone)
        let stick = gamepad.left_stick();
        if stick.y > 0.5 {
            state |= 1 << Button::Up as u8;
        }
        if stick.y < -0.5 {
            state |= 1 << Button::Down as u8;
        }
        if stick.x < -0.5 {
            state |= 1 << Button::Left as u8;
        }
        if stick.x > 0.5 {
            state |= 1 << Button::Right as u8;
        }

        // Face buttons
        if gamepad.pressed(GamepadButton::South) || gamepad.pressed(GamepadButton::North) {
            state |= 1 << Button::A as u8;
        }
        if gamepad.pressed(GamepadButton::East) || gamepad.pressed(GamepadButton::West) {
            state |= 1 << Button::B as u8;
        }
        if gamepad.pressed(GamepadButton::Select) {
            state |= 1 << Button::Select as u8;
        }
        if gamepad.pressed(GamepadButton::Start) {
            state |= 1 << Button::Start as u8;
        }
    }

    nes_input.0 = state;
}

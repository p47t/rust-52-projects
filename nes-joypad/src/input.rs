/// Map minifb keyboard state to NES button bitfield for Player 1.
///
/// Default mapping:
/// - Arrow keys → D-pad
/// - Z → A, X → B (common emulator convention)
/// - A → Select, S → Start
#[cfg(feature = "demos")]
pub fn keyboard_to_buttons(window: &minifb::Window) -> u8 {
    use crate::joypad::Button;
    use minifb::Key;

    let mut state = 0u8;
    if window.is_key_down(Key::Up) {
        state |= 1 << Button::Up as u8;
    }
    if window.is_key_down(Key::Down) {
        state |= 1 << Button::Down as u8;
    }
    if window.is_key_down(Key::Left) {
        state |= 1 << Button::Left as u8;
    }
    if window.is_key_down(Key::Right) {
        state |= 1 << Button::Right as u8;
    }
    if window.is_key_down(Key::Z) {
        state |= 1 << Button::A as u8;
    }
    if window.is_key_down(Key::X) {
        state |= 1 << Button::B as u8;
    }
    if window.is_key_down(Key::A) {
        state |= 1 << Button::Select as u8;
    }
    if window.is_key_down(Key::S) {
        state |= 1 << Button::Start as u8;
    }
    state
}

/// Map gilrs gamepad state to NES button bitfield.
///
/// Supports D-pad, left analog stick (0.5 deadzone), and face buttons.
#[cfg(feature = "gamepad")]
pub fn gamepad_to_buttons(gilrs: &gilrs::Gilrs, gamepad_id: gilrs::GamepadId) -> u8 {
    use crate::joypad::Button;
    use gilrs::{Axis, Button as GBtn};

    let gp = gilrs.gamepad(gamepad_id);
    let mut state = 0u8;

    // D-pad
    if gp.is_pressed(GBtn::DPadUp) {
        state |= 1 << Button::Up as u8;
    }
    if gp.is_pressed(GBtn::DPadDown) {
        state |= 1 << Button::Down as u8;
    }
    if gp.is_pressed(GBtn::DPadLeft) {
        state |= 1 << Button::Left as u8;
    }
    if gp.is_pressed(GBtn::DPadRight) {
        state |= 1 << Button::Right as u8;
    }

    // Left analog stick with deadzone
    if let Some(axis) = gp.axis_data(Axis::LeftStickX) {
        if axis.value() < -0.5 {
            state |= 1 << Button::Left as u8;
        }
        if axis.value() > 0.5 {
            state |= 1 << Button::Right as u8;
        }
    }
    if let Some(axis) = gp.axis_data(Axis::LeftStickY) {
        if axis.value() > 0.5 {
            state |= 1 << Button::Up as u8;
        }
        if axis.value() < -0.5 {
            state |= 1 << Button::Down as u8;
        }
    }

    // Face buttons (Xbox: A=South, B=East)
    if gp.is_pressed(GBtn::South) {
        state |= 1 << Button::A as u8;
    }
    if gp.is_pressed(GBtn::East) {
        state |= 1 << Button::B as u8;
    }
    if gp.is_pressed(GBtn::Select) {
        state |= 1 << Button::Select as u8;
    }
    if gp.is_pressed(GBtn::Start) {
        state |= 1 << Button::Start as u8;
    }

    state
}

use crate::vision::GestureState;
use anyhow::Result;
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use std::sync::mpsc::Receiver;

// Win32 FFI to get monitor resolution dynamically with zero dependencies
#[cfg(target_os = "windows")]
extern "system" {
    fn GetSystemMetrics(nIndex: i32) -> i32;
}

#[cfg(target_os = "windows")]
fn get_screen_size() -> (i32, i32) {
    unsafe {
        // SM_CXSCREEN = 0, SM_CYSCREEN = 1
        (GetSystemMetrics(0), GetSystemMetrics(1))
    }
}

#[cfg(not(target_os = "windows"))]
fn get_screen_size() -> (i32, i32) {
    // Default fallback for other platforms
    (1920, 1080)
}

pub fn run_controller_loop(rx: Receiver<GestureState>) -> Result<()> {
    // 1. Initialize Enigo input simulator
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow::anyhow!("Failed to initialize Enigo: {:?}", e))?;

    let (screen_w, screen_h) = get_screen_size();
    println!("Detected screen resolution: {}x{}", screen_w, screen_h);
    println!("Controller thread active. Ready to process gesture inputs.");

    let mut is_pressed = false;
    let mut last_x = 0.5f32;
    let mut last_y = 0.5f32;

    // EMA (Exponential Moving Average) smoothing factor. Smaller = smoother, larger = more responsive.
    let alpha = 0.20f32;

    // Scroll accumulation buffer
    let mut scroll_accumulator = 0.0f32;

    // 2. Event processing loop
    while let Ok(state) = rx.recv() {
        match state {
            GestureState::Hover { x, y } => {
                // If left click was active (drag state), release it
                if is_pressed {
                    enigo.button(Button::Left, Direction::Release)?;
                    is_pressed = false;
                    println!("Mouse Release");
                }

                // Smooth coordinates to prevent cursor jitter
                let target_x = alpha * x + (1.0 - alpha) * last_x;
                let target_y = alpha * y + (1.0 - alpha) * last_y;

                let px = (target_x * (screen_w as f32)) as i32;
                let py = (target_y * (screen_h as f32)) as i32;

                enigo.move_mouse(px, py, Coordinate::Abs)?;
                last_x = target_x;
                last_y = target_y;
            }
            GestureState::Click { x, y } => {
                // If left click is not active, press it (start drag/click)
                if !is_pressed {
                    enigo.button(Button::Left, Direction::Press)?;
                    is_pressed = true;
                    println!("Mouse Press");
                }

                // Drag the cursor smoothly
                let target_x = alpha * x + (1.0 - alpha) * last_x;
                let target_y = alpha * y + (1.0 - alpha) * last_y;

                let px = (target_x * (screen_w as f32)) as i32;
                let py = (target_y * (screen_h as f32)) as i32;

                enigo.move_mouse(px, py, Coordinate::Abs)?;
                last_x = target_x;
                last_y = target_y;
            }
            GestureState::Scroll { dy } => {
                // Release left click if active
                if is_pressed {
                    enigo.button(Button::Left, Direction::Release)?;
                    is_pressed = false;
                    println!("Mouse Release");
                }

                // Accumulate scroll amount
                scroll_accumulator += dy;
                let scroll_clicks = scroll_accumulator.trunc() as i32;
                if scroll_clicks != 0 {
                    // Scroll axis: positive = scroll up, negative = scroll down
                    enigo.scroll(-scroll_clicks, Axis::Vertical)?;
                    scroll_accumulator -= scroll_clicks as f32;
                }
            }
            GestureState::WaveLeft => {
                // Windows virtual desktop switch left: Ctrl + Win + LeftArrow
                println!("Triggering: Ctrl + Win + Left");
                enigo.key(Key::Control, Direction::Press)?;
                enigo.key(Key::Meta, Direction::Press)?;
                enigo.key(Key::LeftArrow, Direction::Click)?;
                enigo.key(Key::Meta, Direction::Release)?;
                enigo.key(Key::Control, Direction::Release)?;
            }
            GestureState::WaveRight => {
                // Windows virtual desktop switch right: Ctrl + Win + RightArrow
                println!("Triggering: Ctrl + Win + Right");
                enigo.key(Key::Control, Direction::Press)?;
                enigo.key(Key::Meta, Direction::Press)?;
                enigo.key(Key::RightArrow, Direction::Click)?;
                enigo.key(Key::Meta, Direction::Release)?;
                enigo.key(Key::Control, Direction::Release)?;
            }
            GestureState::Idle => {
                // Hand lost: release click if active
                if is_pressed {
                    enigo.button(Button::Left, Direction::Release)?;
                    is_pressed = false;
                    println!("Hand lost -> Mouse Release");
                }
            }
        }
    }

    Ok(())
}

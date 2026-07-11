use anyhow::Result;
use clap::Parser;
use std::thread;

mod controller;
mod vision;

/// Hand gesture OS controller using OpenCV 5 and Enigo
#[derive(Parser)]
#[command(
    name = "gesture-control",
    version,
    about = "Zero-overhead hand gesture OS controller in Rust"
)]
struct Cli {
    /// Webcam index to capture video from
    #[arg(short, long, default_value_t = 0)]
    camera: i32,

    /// Disable visual preview feedback and HSV sliders
    #[arg(short = 'n', long)]
    no_preview: bool,

    /// Default Low Hue for skin segmentation [0-180]
    #[arg(long, default_value_t = 0)]
    low_h: i32,

    /// Default High Hue for skin segmentation [0-180]
    #[arg(long, default_value_t = 20)]
    high_h: i32,

    /// Default Low Saturation for skin segmentation [0-255]
    #[arg(long, default_value_t = 30)]
    low_s: i32,

    /// Default High Saturation for skin segmentation [0-255]
    #[arg(long, default_value_t = 150)]
    high_s: i32,

    /// Default Low Value for skin segmentation [0-255]
    #[arg(long, default_value_t = 60)]
    low_v: i32,

    /// Default High Value for skin segmentation [0-255]
    #[arg(long, default_value_t = 255)]
    high_v: i32,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let hsv_low = [cli.low_h, cli.low_s, cli.low_v];
    let hsv_high = [cli.high_h, cli.high_s, cli.high_v];
    let show_preview = !cli.no_preview;

    // 1. Create a lightweight cross-thread channel
    let (tx, rx) = std::sync::mpsc::channel();

    println!("Starting Vision Processor thread...");
    // 2. Spawn the vision thread for low-latency video decoding and tracking
    let vision_handle = thread::spawn(move || {
        if let Err(e) = vision::run_vision_loop(cli.camera, tx, hsv_low, hsv_high, show_preview) {
            eprintln!("Error in vision thread: {:?}", e);
        }
    });

    println!("Starting OS Controller main loop...");
    // 3. Run the controller loop (enigo simulation) in the main thread
    controller::run_controller_loop(rx)?;

    // Wait for vision thread to clean up on exit
    if let Err(e) = vision_handle.join() {
        eprintln!("Failed to join vision thread: {:?}", e);
    }

    println!("Shutdown successful.");
    Ok(())
}

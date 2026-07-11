use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::watch;
use tracing::{info, warn};

use opencv::{
    core::{AlgorithmHint, Mat, Point, Rect, Scalar, Size, Vector},
    imgcodecs, imgproc,
    prelude::*,
    videoio,
};

/// The camera frame capture and CV processing loop running on a dedicated worker thread.
/// Automatically falls back to a simulated mock camera feed if a physical camera is not connected.
pub fn run_capture_loop(
    camera_index: i32,
    frame_state: Arc<Mutex<Vec<u8>>>,
    watch_tx: watch::Sender<u64>,
) -> anyhow::Result<()> {
    info!("Initializing video capture backend...");
    let mut cam = videoio::VideoCapture::new(camera_index, videoio::CAP_ANY)?;
    let mut is_simulated = false;

    if cam.is_opened()? {
        info!("Successfully opened camera at index {}", camera_index);
        // Set standard streaming resolution (640x360 for high performance / low bandwidth).
        let _ = cam.set(videoio::CAP_PROP_FRAME_WIDTH, 640.0);
        let _ = cam.set(videoio::CAP_PROP_FRAME_HEIGHT, 360.0);
    } else {
        warn!(
            "Failed to open physical camera at index {}. Falling back to Simulated Mock Feed.",
            camera_index
        );
        is_simulated = true;
    }

    let mut frame = Mat::default();
    let mut last_fps_time = Instant::now();
    let mut frame_count = 0u64;
    let mut fps = 30.0; // Starting guess for FPS

    loop {
        let loop_start = Instant::now();

        if is_simulated {
            // 1. Generate Simulated Frame
            frame = Mat::new_rows_cols_with_default(
                360,
                640,
                opencv::core::CV_8UC3,
                Scalar::new(15.0, 10.0, 10.0, 0.0), // deep space dark background
            )?;

            // Draw clean background grid lines.
            let size = frame.size()?;
            for x in (0..size.width).step_by(80) {
                imgproc::line(
                    &mut frame,
                    Point::new(x, 0),
                    Point::new(x, size.height),
                    Scalar::new(25.0, 20.0, 20.0, 0.0),
                    1,
                    imgproc::LINE_8,
                    0,
                )?;
            }
            for y in (0..size.height).step_by(80) {
                imgproc::line(
                    &mut frame,
                    Point::new(0, y),
                    Point::new(size.width, y),
                    Scalar::new(25.0, 20.0, 20.0, 0.0),
                    1,
                    imgproc::LINE_8,
                    0,
                )?;
            }

            // Draw a spinning radar scanner line.
            let center = Point::new(size.width / 2, size.height / 2);
            let angle = (frame_count as f64 * 0.05) % (2.0 * std::f64::consts::PI);
            let radius = 120.0;
            let radar_end = Point::new(
                (center.x as f64 + radius * angle.cos()) as i32,
                (center.y as f64 + radius * angle.sin()) as i32,
            );
            imgproc::line(
                &mut frame,
                center,
                radar_end,
                Scalar::new(0.0, 255.0, 0.0, 0.0), // green radar beam
                1,
                imgproc::LINE_AA,
                0,
            )?;

            // Draw target circles.
            imgproc::circle(
                &mut frame,
                center,
                40,
                Scalar::new(50.0, 50.0, 50.0, 0.0),
                1,
                imgproc::LINE_AA,
                0,
            )?;
            imgproc::circle(
                &mut frame,
                center,
                80,
                Scalar::new(50.0, 50.0, 50.0, 0.0),
                1,
                imgproc::LINE_AA,
                0,
            )?;
            imgproc::circle(
                &mut frame,
                center,
                120,
                Scalar::new(50.0, 50.0, 50.0, 0.0),
                1,
                imgproc::LINE_AA,
                0,
            )?;

            // Draw animated pulse target.
            let target_radius = ((frame_count * 2) % 40 + 5) as i32;
            imgproc::circle(
                &mut frame,
                center,
                target_radius,
                Scalar::new(99.0, 102.0, 241.0, 0.0), // indigo pulse
                2,
                imgproc::LINE_AA,
                0,
            )?;

            // Text tag for simulated mode.
            imgproc::put_text(
                &mut frame,
                "⚠️ CAMERA OFFLINE - RUNNING IN SIMULATED MODE",
                Point::new(20, size.height - 20),
                imgproc::FONT_HERSHEY_SIMPLEX,
                0.5,
                Scalar::new(99.0, 102.0, 241.0, 0.0),
                1,
                imgproc::LINE_AA,
                false,
            )?;

            // Sleep to mimic ~30 FPS capture rate.
            let elapsed = loop_start.elapsed();
            let target_duration = Duration::from_millis(33);
            if elapsed < target_duration {
                std::thread::sleep(target_duration - elapsed);
            }
        } else {
            // 2. Read physical hardware frame.
            cam.read(&mut frame)?;
            if frame.empty() {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
        }

        // Calculate dynamic live FPS.
        frame_count += 1;
        let now = Instant::now();
        let delta = now.duration_since(last_fps_time).as_secs_f64();
        last_fps_time = now;
        if delta > 0.0 {
            let instant_fps = 1.0 / delta;
            // Smooth FPS reading using Exponential Moving Average.
            fps = fps * 0.95 + instant_fps * 0.05;
        }

        // --- COMPUTER VISION PROCESSING PIPELINE (imgproc) ---

        // A. Define ROI and overlay target bounding box.
        let size = frame.size()?;
        let box_w = 220;
        let box_h = 220;
        let box_x = (size.width - box_w) / 2;
        let box_y = (size.height - box_h) / 2;
        let roi_rect = Rect::new(box_x, box_y, box_w, box_h);

        // Draw outer bounding box.
        imgproc::rectangle(
            &mut frame,
            roi_rect,
            Scalar::new(99.0, 102.0, 241.0, 0.0), // indigo bounding box
            2,
            imgproc::LINE_AA,
            0,
        )?;

        // Draw corners markers for professional HUD style.
        let corner_len = 15;
        // Top-left corner
        imgproc::line(
            &mut frame,
            Point::new(box_x, box_y),
            Point::new(box_x + corner_len, box_y),
            Scalar::new(16.0, 185.0, 129.0, 0.0),
            3,
            imgproc::LINE_AA,
            0,
        )?;
        imgproc::line(
            &mut frame,
            Point::new(box_x, box_y),
            Point::new(box_x, box_y + corner_len),
            Scalar::new(16.0, 185.0, 129.0, 0.0),
            3,
            imgproc::LINE_AA,
            0,
        )?;
        // Top-right corner
        imgproc::line(
            &mut frame,
            Point::new(box_x + box_w, box_y),
            Point::new(box_x + box_w - corner_len, box_y),
            Scalar::new(16.0, 185.0, 129.0, 0.0),
            3,
            imgproc::LINE_AA,
            0,
        )?;
        imgproc::line(
            &mut frame,
            Point::new(box_x + box_w, box_y),
            Point::new(box_x + box_w, box_y + corner_len),
            Scalar::new(16.0, 185.0, 129.0, 0.0),
            3,
            imgproc::LINE_AA,
            0,
        )?;
        // Bottom-left corner
        imgproc::line(
            &mut frame,
            Point::new(box_x, box_y + box_h),
            Point::new(box_x + corner_len, box_y + box_h),
            Scalar::new(16.0, 185.0, 129.0, 0.0),
            3,
            imgproc::LINE_AA,
            0,
        )?;
        imgproc::line(
            &mut frame,
            Point::new(box_x, box_y + box_h),
            Point::new(box_x, box_y + box_h - corner_len),
            Scalar::new(16.0, 185.0, 129.0, 0.0),
            3,
            imgproc::LINE_AA,
            0,
        )?;
        // Bottom-right corner
        imgproc::line(
            &mut frame,
            Point::new(box_x + box_w, box_y + box_h),
            Point::new(box_x + box_w - corner_len, box_y + box_h),
            Scalar::new(16.0, 185.0, 129.0, 0.0),
            3,
            imgproc::LINE_AA,
            0,
        )?;
        imgproc::line(
            &mut frame,
            Point::new(box_x + box_w, box_y + box_h),
            Point::new(box_x + box_w, box_y + box_h - corner_len),
            Scalar::new(16.0, 185.0, 129.0, 0.0),
            3,
            imgproc::LINE_AA,
            0,
        )?;

        // B. Apply OpenCV CV Filter inside the bounding box (Gaussian Blur ROI / Canny Edge).
        // Uses safe checks to degrade gracefully if sub-matrix initialization fails.
        if let Ok(mut roi) = Mat::roi_mut(&mut frame, roi_rect) {
            // Apply a nice processing effect: Gaussian Blur + Color space operations
            let mut roi_blurred = Mat::default();
            if imgproc::gaussian_blur(
                &roi,
                &mut roi_blurred,
                Size::new(15, 15),
                0.0,
                0.0,
                opencv::core::BORDER_DEFAULT,
                AlgorithmHint::ALGO_HINT_DEFAULT,
            )
            .is_ok()
            {
                let _ = roi_blurred.copy_to(&mut roi);
            }
        }

        // C. Render scanning red laser overlay line inside the bounding box.
        let scan_period = 100;
        let scan_pos = (frame_count % scan_period) as i32;
        let scan_y = box_y + (scan_pos * box_h / scan_period as i32);
        imgproc::line(
            &mut frame,
            Point::new(box_x + 2, scan_y),
            Point::new(box_x + box_w - 2, scan_y),
            Scalar::new(0.0, 0.0, 255.0, 0.0), // red scanning laser
            2,
            imgproc::LINE_AA,
            0,
        )?;

        // Label above the bounding box.
        imgproc::put_text(
            &mut frame,
            "CV PROCESS: BLUR FILTER",
            Point::new(box_x, box_y - 8),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            Scalar::new(16.0, 185.0, 129.0, 0.0), // green
            2,
            imgproc::LINE_AA,
            false,
        )?;

        // D. Overlay Live FPS Counter.
        let fps_label = format!("STREAM: ACTIVE | FPS: {:.1}", fps);
        imgproc::put_text(
            &mut frame,
            &fps_label,
            Point::new(20, 35),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.65,
            Scalar::new(0.0, 255.0, 0.0, 0.0), // neon green
            2,
            imgproc::LINE_AA,
            false,
        )?;

        // E. Overlay Timestamp.
        let time_str = chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S.%3f")
            .to_string();
        imgproc::put_text(
            &mut frame,
            &time_str,
            Point::new(20, 65),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.65,
            Scalar::new(255.0, 255.0, 255.0, 0.0), // white
            2,
            imgproc::LINE_AA,
            false,
        )?;

        // --- COMPRESSION PIPELINE (imgcodecs) ---
        // Compress matrix directly inside the capture task so that the web event loop threads
        // do not have to perform heavy CPU compression calculations.
        let mut jpeg_buf = Vector::<u8>::new();
        let mut encode_params = Vector::<i32>::new();
        encode_params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
        encode_params.push(85); // Compress at 85% quality to optimize network bandwidth and image quality.

        imgcodecs::imencode(".jpg", &frame, &mut jpeg_buf, &encode_params)?;
        let jpeg_bytes = jpeg_buf.to_vec();

        // Write the compressed JPEG frame to the synchronized state.
        {
            let mut lock = frame_state.lock().unwrap();
            *lock = jpeg_bytes;
        }

        // Notify all subscribers of the new frame counter update.
        let current_count = watch_tx.borrow().wrapping_add(1);
        let _ = watch_tx.send(current_count);
    }
}

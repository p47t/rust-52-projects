use anyhow::{bail, Context, Result};
use clap::Parser;
use opencv::{
    core::{Mat, Point, Rect, Scalar, Size, Vector},
    highgui,
    imgcodecs,
    imgproc,
    objdetect::FaceDetectorYN,
    prelude::*,
    videoio,
};
use std::path::PathBuf;

/// Face detection using OpenCV's YuNet model.
///
/// Detects faces in images or webcam feed, drawing bounding boxes and
/// facial landmarks (eyes, nose, mouth corners).
#[derive(Parser)]
#[command(name = "face-detect", version)]
struct Cli {
    /// Path to an image file. If omitted, opens the webcam.
    #[arg(short, long)]
    image: Option<PathBuf>,

    /// Path to the YuNet ONNX model file.
    #[arg(short, long, default_value = "face_detection_yunet_2023mar.onnx")]
    model: PathBuf,

    /// Minimum confidence score to keep a detection (0.0–1.0).
    #[arg(short, long, default_value_t = 0.9)]
    score_threshold: f32,

    /// NMS IoU threshold (0.0–1.0).
    #[arg(short, long, default_value_t = 0.3)]
    nms_threshold: f32,

    /// Path to save the annotated output image (image mode only).
    #[arg(short, long)]
    output: Option<PathBuf>,
}

// Landmark colours: right eye, left eye, nose, right mouth, left mouth.
const LANDMARK_COLORS: [(f64, f64, f64); 5] = [
    (255.0, 0.0, 0.0),     // right eye  – blue
    (0.0, 0.0, 255.0),     // left eye   – red
    (0.0, 255.0, 0.0),     // nose tip   – green
    (255.0, 0.0, 255.0),   // right mouth – pink
    (0.0, 255.0, 255.0),   // left mouth  – yellow
];

/// Bounding box colour (green).
const BOX_COLOR: Scalar = Scalar::new(0.0, 255.0, 0.0, 0.0);

/// Confidence text colour (white).
const TEXT_COLOR: Scalar = Scalar::new(255.0, 255.0, 255.0, 0.0);

fn main() -> Result<()> {
    let cli = Cli::parse();

    let model_path = cli
        .model
        .to_str()
        .context("model path is not valid UTF-8")?;

    if !cli.model.exists() {
        bail!(
            "Model file not found: {}\n\
             Download it from: https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet",
            cli.model.display()
        );
    }

    // Create the YuNet face detector.
    // Initial input_size is a placeholder; we resize per-frame.
    let mut detector = FaceDetectorYN::create(
        model_path,
        "",
        Size::new(320, 320),
        cli.score_threshold,
        cli.nms_threshold,
        5000, // top_k
        0,    // backend – default
        0,    // target  – CPU
    )?;

    match cli.image {
        Some(ref path) => {
            detect_in_image(&mut detector, path, cli.output.as_deref())?;
        }
        None => {
            detect_in_webcam(&mut detector)?;
        }
    }

    Ok(())
}

// ── Image mode ──────────────────────────────────────────────────────────────

fn detect_in_image(
    detector: &mut impl FaceDetectorYNTrait,
    path: &std::path::Path,
    output: Option<&std::path::Path>,
) -> Result<i32> {
    let mut img = imgcodecs::imread(
        path.to_str().context("image path is not valid UTF-8")?,
        imgcodecs::IMREAD_COLOR,
    )?;

    if img.empty() {
        bail!("Failed to read image: {}", path.display());
    }

    let size = img.size()?;
    let max_dim = 800.0;
    let mut scale = 1.0;
    
    let mut detect_img = img.clone();
    if size.width > 800 || size.height > 800 {
        scale = f64::max(size.width as f64, size.height as f64) / max_dim;
        let new_w = (size.width as f64 / scale).round() as i32;
        let new_h = (size.height as f64 / scale).round() as i32;
        let new_size = Size::new(new_w, new_h);
        imgproc::resize(&img, &mut detect_img, new_size, 0.0, 0.0, imgproc::INTER_LINEAR)?;
    }

    detector.set_input_size(detect_img.size()?)?;

    let mut faces = Mat::default();
    detector.detect(&detect_img, &mut faces)?;
    let count = faces.rows();
    println!("Detected {} face(s)", count);

    // Scale the detections back to the original image size.
    if scale != 1.0 && count > 0 {
        for i in 0..count {
            for j in 0..14 {
                let v = *faces.at_2d::<f32>(i, j)?;
                *faces.at_2d_mut::<f32>(i, j)? = (v as f64 * scale) as f32;
            }
        }
    }

    draw_detections(&mut img, &faces)?;

    // Save or display.
    if let Some(out) = output {
        let out_str = out.to_str().context("output path is not valid UTF-8")?;
        let params = Vector::<i32>::new();
        imgcodecs::imwrite(out_str, &img, &params)?;
        println!("Saved to {}", out.display());
    } else {
        let window = "face-detect";
        highgui::named_window(window, highgui::WINDOW_AUTOSIZE)?;
        highgui::imshow(window, &img)?;
        println!("Press any key to close...");
        highgui::wait_key(0)?;
        highgui::destroy_all_windows()?;
    }

    Ok(count)
}

// ── Webcam mode ─────────────────────────────────────────────────────────────

fn detect_in_webcam(detector: &mut impl FaceDetectorYNTrait) -> Result<()> {
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !cam.is_opened()? {
        bail!("Cannot open default webcam (index 0)");
    }

    let window = "face-detect – webcam (press Q to quit)";
    highgui::named_window(window, highgui::WINDOW_AUTOSIZE)?;

    let mut frame = Mat::default();
    loop {
        cam.read(&mut frame)?;
        if frame.empty() {
            continue;
        }

        let size = frame.size()?;
        detector.set_input_size(size)?;

        let mut faces = Mat::default();
        detector.detect(&frame, &mut faces)?;
        let count = faces.rows();

        draw_detections(&mut frame, &faces)?;

        // Show FPS-style face count overlay.
        let label = format!("Faces: {count}");
        imgproc::put_text(
            &mut frame,
            &label,
            Point::new(10, 30),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.8,
            TEXT_COLOR,
            2,
            imgproc::LINE_AA,
            false,
        )?;

        highgui::imshow(window, &frame)?;

        // 'q' or ESC to quit.
        let key = highgui::wait_key(1)?;
        if key == b'q' as i32 || key == 27 {
            break;
        }
    }

    highgui::destroy_all_windows()?;
    Ok(())
}

// ── Drawing helpers ─────────────────────────────────────────────────────────

/// Draw bounding boxes, landmarks, and confidence scores onto `img`.
///
/// The `faces` Mat is N×15 (f32) where each row is:
///   [x, y, w, h, re_x, re_y, le_x, le_y, nt_x, nt_y, rm_x, rm_y, lm_x, lm_y, score]
fn draw_detections(img: &mut Mat, faces: &Mat) -> Result<()> {
    for i in 0..faces.rows() {
        // Bounding box.
        let x = *faces.at_2d::<f32>(i, 0)? as i32;
        let y = *faces.at_2d::<f32>(i, 1)? as i32;
        let w = *faces.at_2d::<f32>(i, 2)? as i32;
        let h = *faces.at_2d::<f32>(i, 3)? as i32;
        let score = *faces.at_2d::<f32>(i, 14)?;

        imgproc::rectangle(
            img,
            Rect::new(x, y, w, h),
            BOX_COLOR,
            2,
            imgproc::LINE_AA,
            0,
        )?;

        // Confidence label above the box.
        let label = format!("{:.0}%", score * 100.0);
        imgproc::put_text(
            img,
            &label,
            Point::new(x, y - 6),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            TEXT_COLOR,
            1,
            imgproc::LINE_AA,
            false,
        )?;

        // 5 facial landmarks (columns 4..14, pairs of x,y).
        for (j, &(b, g, r)) in LANDMARK_COLORS.iter().enumerate() {
            let col = 4 + j as i32 * 2;
            let lx = *faces.at_2d::<f32>(i, col)? as i32;
            let ly = *faces.at_2d::<f32>(i, col + 1)? as i32;
            imgproc::circle(
                img,
                Point::new(lx, ly),
                3,
                Scalar::new(b, g, r, 0.0),
                imgproc::FILLED,
                imgproc::LINE_AA,
                0,
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn setup_detector(score_threshold: f32) -> Result<opencv::core::Ptr<FaceDetectorYN>> {
        let model_path = "face_detection_yunet_2023mar.onnx";
        FaceDetectorYN::create(
            model_path,
            "",
            Size::new(320, 320),
            score_threshold,
            0.3, // NMS
            5000,
            0,
            0,
        ).map_err(Into::into)
    }

    #[test]
    fn test_face_detection_lena() -> Result<()> {
        let mut detector = setup_detector(0.5)?;
        let in_path = PathBuf::from("tests/lena.jpg");
        let out_path = PathBuf::from("tests/lena_out.jpg");
        if out_path.exists() { std::fs::remove_file(&out_path).unwrap(); }
        let count = detect_in_image(&mut detector, &in_path, Some(&out_path))?;
        assert_eq!(count, 1, "Should detect exactly 1 face in lena.jpg");
        assert!(out_path.exists(), "Output image should have been created");
        Ok(())
    }

    #[test]
    fn test_face_detection_messi() -> Result<()> {
        let mut detector = setup_detector(0.5)?;
        let in_path = PathBuf::from("tests/messi5.jpg");
        let out_path = PathBuf::from("tests/messi5_out.jpg");
        if out_path.exists() { std::fs::remove_file(&out_path).unwrap(); }
        let count = detect_in_image(&mut detector, &in_path, Some(&out_path))?;
        // Messi image has 1 clear face (Messi) and maybe a partial one in background depending on threshold.
        // At 0.5 threshold, it usually detects 1.
        assert!(count >= 1, "Should detect at least 1 face in messi5.jpg");
        assert!(out_path.exists(), "Output image should have been created");
        Ok(())
    }

    #[test]
    fn test_face_detection_none() -> Result<()> {
        let mut detector = setup_detector(0.5)?;
        let in_path = PathBuf::from("tests/no_face.png");
        // Pass a dummy output path so it doesn't try to open a GUI window during tests
        let out_path = PathBuf::from("tests/no_face_out.png");
        let count = detect_in_image(&mut detector, &in_path, Some(&out_path))?;
        assert_eq!(count, 0, "Should detect 0 faces in the OpenCV logo");
        Ok(())
    }

    #[test]
    fn test_face_detection_lfw_benchmark() -> Result<()> {
        let mut detector = setup_detector(0.5)?;
        let lfw_dir = PathBuf::from("tests/lfw_funneled");
        
        if !lfw_dir.exists() {
            println!("LFW dataset not found, skipping benchmark.");
            return Ok(());
        }

        let mut images = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&lfw_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                        for sub in sub_entries.flatten() {
                            if sub.path().extension().and_then(|s| s.to_str()) == Some("jpg") {
                                images.push(sub.path());
                                if images.len() >= 1000 {
                                    break;
                                }
                            }
                        }
                    }
                }
                if images.len() >= 1000 {
                    break;
                }
            }
        }

        assert!(!images.is_empty(), "No images found in LFW directory");

        let mut hits = 0;
        let dummy_out = PathBuf::from("tests/lfw_benchmark_dummy.jpg");
        for path in &images {
            // Passing Some to avoid launching the GUI window and freezing the test
            let count = detect_in_image(&mut detector, path, Some(&dummy_out))?;
            if count >= 1 {
                hits += 1;
            }
        }

        let hit_rate = (hits as f32 / images.len() as f32) * 100.0;
        println!("LFW Benchmark: {} / {} faces detected ({:.1}%)", hits, images.len(), hit_rate);
        
        // Assert we get a decent hit rate on real-world data
        assert!(hit_rate >= 80.0, "Hit rate {:.1}% is below acceptable 80% threshold", hit_rate);

        Ok(())
    }

    #[test]
    fn test_face_detection_negative_benchmark() -> Result<()> {
        let mut detector = setup_detector(0.8)?;
        let bg_dir = PathBuf::from("tests/iccv09Data/images");
        
        if !bg_dir.exists() {
            println!("Negative dataset not found, skipping benchmark.");
            return Ok(());
        }

        let mut images = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&bg_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("jpg") {
                    images.push(entry.path());
                }
            }
        }

        assert!(!images.is_empty(), "No images found in negative directory");

        let mut false_positives = 0;
        let dummy_out = PathBuf::from("tests/negative_benchmark_dummy.jpg");
        for path in &images {
            let count = detect_in_image(&mut detector, path, Some(&dummy_out))?;
            if count > 0 {
                false_positives += 1;
            }
        }

        let total = images.len() as f32;
        let fp_rate = (false_positives as f32 / total) * 100.0;
        println!("Negative Benchmark: {} / {} false positives ({:.1}%)", false_positives, images.len(), fp_rate);
        
        // Assert we get < 5% false positives
        assert!(fp_rate <= 5.0, "False positive rate {:.1}% is above acceptable 5% threshold", fp_rate);

        Ok(())
    }
}

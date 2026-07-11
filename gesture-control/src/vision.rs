use anyhow::{bail, Result};
use opencv::{
    core::{Mat, Point, Rect, Scalar, Size, CV_32F, CV_8UC1},
    highgui, imgproc,
    prelude::*,
    videoio,
};
use std::sync::mpsc::Sender;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureState {
    Hover { x: f32, y: f32 },
    Click { x: f32, y: f32 },
    Scroll { dy: f32 },
    WaveLeft,
    WaveRight,
    Idle,
}

/// Helper to calculate Euclidean distance between two points
fn distance(p1: Point, p2: Point) -> f32 {
    (((p1.x - p2.x) as f32).powi(2) + ((p1.y - p2.y) as f32).powi(2)).sqrt()
}

/// Computes the area of a 2D contour using the Shoelace formula (Green's theorem).
fn calculate_contour_area(contour: &[Point]) -> f64 {
    let n = contour.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..n {
        let p1 = contour[i];
        let p2 = contour[(i + 1) % n];
        area += (p1.x as f64) * (p2.y as f64) - (p2.x as f64) * (p1.y as f64);
    }
    area.abs() / 2.0
}

/// Computes the indices of the convex hull vertices of a contour using Andrew's Monotone Chain algorithm.
fn calculate_convex_hull_indices(points: &[Point]) -> Vec<usize> {
    let n = points.len();
    if n < 3 {
        return (0..n).collect();
    }

    // Sort indices by points x, then y
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&i, &j| {
        let p_i = points[i];
        let p_j = points[j];
        p_i.x.cmp(&p_j.x).then_with(|| p_i.y.cmp(&p_j.y))
    });

    // 2D cross product to find winding direction
    fn cross(o: Point, a: Point, b: Point) -> i32 {
        (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
    }

    let mut lower = Vec::new();
    for &i in &indices {
        while lower.len() >= 2 {
            let pen = points[lower[lower.len() - 2]];
            let last = points[lower[lower.len() - 1]];
            if cross(pen, last, points[i]) <= 0 {
                lower.pop();
            } else {
                break;
            }
        }
        lower.push(i);
    }

    let mut upper = Vec::new();
    for &i in indices.iter().rev() {
        while upper.len() >= 2 {
            let pen = points[upper[upper.len() - 2]];
            let last = points[upper[upper.len() - 1]];
            if cross(pen, last, points[i]) <= 0 {
                upper.pop();
            } else {
                break;
            }
        }
        upper.push(i);
    }

    lower.pop();
    upper.pop();

    lower.extend(upper);
    lower
}

/// Computes convexity defects between a contour and its convex hull indices.
/// Returns a list of (start_idx, end_idx, far_idx, depth_pixels).
fn calculate_defects(contour: &[Point], hull_indices: &[usize]) -> Vec<(usize, usize, usize, f32)> {
    let mut defects = Vec::new();
    let n_hull = hull_indices.len();
    if n_hull < 3 {
        return defects;
    }

    let n_contour = contour.len();

    for i in 0..n_hull {
        let start_idx = hull_indices[i];
        let end_idx = hull_indices[(i + 1) % n_hull];

        // Find the index range wrapping around the closed contour loop
        let mut idxs = Vec::new();
        let mut curr = start_idx;
        while curr != end_idx {
            idxs.push(curr);
            curr = (curr + 1) % n_contour;
        }
        idxs.push(end_idx);

        if idxs.len() < 3 {
            continue;
        }

        let start_pt = contour[start_idx];
        let end_pt = contour[end_idx];

        let vx = (end_pt.x - start_pt.x) as f32;
        let vy = (end_pt.y - start_pt.y) as f32;
        let v_len = (vx * vx + vy * vy).sqrt();
        if v_len < 1e-6 {
            continue;
        }

        let mut max_depth = 0.0f32;
        let mut far_idx = start_idx;

        for &idx in &idxs[1..(idxs.len() - 1)] {
            let pt = contour[idx];
            let wx = (pt.x - start_pt.x) as f32;
            let wy = (pt.y - start_pt.y) as f32;

            // Perpendicular distance = |V x W| / |V|
            let depth = (vx * wy - vy * wx).abs() / v_len;

            if depth > max_depth {
                max_depth = depth;
                far_idx = idx;
            }
        }

        if max_depth > 0.0 {
            defects.push((start_idx, end_idx, far_idx, max_depth));
        }
    }

    defects
}

pub fn run_vision_loop(
    camera_id: i32,
    tx: Sender<GestureState>,
    mut hsv_low_default: [i32; 3],
    mut hsv_high_default: [i32; 3],
    show_preview: bool,
) -> Result<()> {
    // 1. Open webcam
    let mut cap = videoio::VideoCapture::new(camera_id, videoio::CAP_ANY)?;
    if !cap.is_opened()? {
        bail!("Failed to open camera with ID {}", camera_id);
    }

    // Set resolution to 640x480 for fast processing
    cap.set(videoio::CAP_PROP_FRAME_WIDTH, 640.0)?;
    cap.set(videoio::CAP_PROP_FRAME_HEIGHT, 480.0)?;

    let window_name = "Gesture Control - Feed (Press ESC to exit)";
    let settings_window = "HSV Calibration";

    if show_preview {
        highgui::named_window(window_name, highgui::WINDOW_AUTOSIZE)?;
        highgui::named_window(settings_window, highgui::WINDOW_AUTOSIZE)?;

        // Create HSV Calibration Trackbars
        highgui::create_trackbar(
            "Low H",
            settings_window,
            Some(&mut hsv_low_default[0]),
            180,
            None,
        )?;
        highgui::create_trackbar(
            "High H",
            settings_window,
            Some(&mut hsv_high_default[0]),
            180,
            None,
        )?;
        highgui::create_trackbar(
            "Low S",
            settings_window,
            Some(&mut hsv_low_default[1]),
            255,
            None,
        )?;
        highgui::create_trackbar(
            "High S",
            settings_window,
            Some(&mut hsv_high_default[1]),
            255,
            None,
        )?;
        highgui::create_trackbar(
            "Low V",
            settings_window,
            Some(&mut hsv_low_default[2]),
            255,
            None,
        )?;
        highgui::create_trackbar(
            "High V",
            settings_window,
            Some(&mut hsv_high_default[2]),
            255,
            None,
        )?;
    }

    let mut frame = Mat::default();
    let mut hsv = Mat::default();
    let mut mask = Mat::default();
    let mut dist_mat = Mat::default();

    // Wave gesture detection history
    let mut last_palm_x = 0.0f32;
    let mut last_wave_time = Instant::now();
    let mut velocity_x_smoothed = 0.0f32;

    // Last known coordinate for click position
    let mut last_coord = (0.5f32, 0.5f32);
    let mut last_scroll_y = 0.0f32;
    let mut scroll_initialized = false;

    println!("Starting vision processing loop...");

    loop {
        cap.read(&mut frame)?;
        if frame.empty() {
            continue;
        }

        // Mirror frame horizontally for intuitive interaction
        let mut flipped = Mat::default();
        opencv::core::flip(&frame, &mut flipped, 1)?;
        frame = flipped;

        let rows = frame.rows();
        let cols = frame.cols();

        // 2. Read current HSV calibration values
        let (low_h, high_h, low_s, high_s, low_v, high_v) = if show_preview {
            (
                highgui::get_trackbar_pos("Low H", settings_window)?,
                highgui::get_trackbar_pos("High H", settings_window)?,
                highgui::get_trackbar_pos("Low S", settings_window)?,
                highgui::get_trackbar_pos("High S", settings_window)?,
                highgui::get_trackbar_pos("Low V", settings_window)?,
                highgui::get_trackbar_pos("High V", settings_window)?,
            )
        } else {
            (
                hsv_low_default[0],
                hsv_high_default[0],
                hsv_low_default[1],
                hsv_high_default[1],
                hsv_low_default[2],
                hsv_high_default[2],
            )
        };

        // 3. Image Processing: Segmentation
        imgproc::gaussian_blur(
            &frame,
            &mut hsv,
            Size::new(5, 5),
            0.0,
            0.0,
            opencv::core::BORDER_DEFAULT,
            opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;
        imgproc::cvt_color(
            &hsv.clone(),
            &mut hsv,
            imgproc::COLOR_BGR2HSV,
            0,
            opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        // HSV Skin color thresholding
        let lower = Scalar::new(low_h as f64, low_s as f64, low_v as f64, 0.0);
        let upper = Scalar::new(high_h as f64, high_s as f64, high_v as f64, 0.0);
        opencv::core::in_range(&hsv, &lower, &upper, &mut mask)?;

        // Morphological clean up (opening and closing)
        let kernel = imgproc::get_structuring_element(
            imgproc::MORPH_ELLIPSE,
            Size::new(5, 5),
            Point::new(-1, -1),
        )?;
        imgproc::morphology_ex(
            &mask.clone(),
            &mut mask,
            imgproc::MORPH_OPEN,
            &kernel,
            Point::new(-1, -1),
            1,
            opencv::core::BORDER_CONSTANT,
            Scalar::default(),
        )?;
        imgproc::morphology_ex(
            &mask.clone(),
            &mut mask,
            imgproc::MORPH_CLOSE,
            &kernel,
            Point::new(-1, -1),
            1,
            opencv::core::BORDER_CONSTANT,
            Scalar::default(),
        )?;

        // 4. Contour Analysis (using generic Vector representation to avoid types module missing aliases)
        let mut contours = opencv::core::Vector::<opencv::core::Vector<Point>>::new();
        imgproc::find_contours(
            &mask,
            &mut contours,
            imgproc::RETR_EXTERNAL,
            imgproc::CHAIN_APPROX_SIMPLE,
            Point::new(0, 0),
        )?;

        // Find largest contour (our hand)
        let mut largest_contour_idx = -1;
        let mut max_area = 0.0;
        let mut hand_points = Vec::new();

        for i in 0..contours.len() {
            let contour = contours.get(i)?;

            // Map Vector<Point> to Vec<Point> for pure Rust math
            let mut pts = Vec::with_capacity(contour.len());
            for j in 0..contour.len() {
                pts.push(contour.get(j)?);
            }

            let area = calculate_contour_area(&pts);
            if area > max_area {
                max_area = area;
                largest_contour_idx = i as i32;
                hand_points = pts;
            }
        }

        let mut current_state = GestureState::Idle;

        if largest_contour_idx >= 0 && max_area > 5000.0 {
            // 5. Distance Transform to find Palm Center & Radius
            let mut hand_mask =
                Mat::new_rows_cols_with_default(rows, cols, CV_8UC1, Scalar::all(0.0))?;
            let mut draw_contours = opencv::core::Vector::<opencv::core::Vector<Point>>::new();
            draw_contours.push(contours.get(largest_contour_idx as usize)?);
            imgproc::draw_contours(
                &mut hand_mask,
                &draw_contours,
                0,
                Scalar::all(255.0),
                -1, // FILLED
                imgproc::LINE_8,
                &Mat::default(),
                2147483647,
                Point::new(0, 0),
            )?;

            // Compute distance transform (DIST_L2 = 2, DIST_MASK_3 = 3)
            imgproc::distance_transform(&hand_mask, &mut dist_mat, 2, 3, CV_32F)?;

            // Find point furthest from edge (palm center)
            let mut palm_radius = 0.0f64;
            let mut palm_center = Point::default();
            opencv::core::min_max_loc(
                &dist_mat,
                None,
                Some(&mut palm_radius),
                None,
                Some(&mut palm_center),
                &Mat::default(),
            )?;

            let palm_radius_f32 = palm_radius as f32;

            // 6. Convex Hull & Convexity Defects (computed in pure Rust)
            let hull_indices = calculate_convex_hull_indices(&hand_points);
            let defects = calculate_defects(&hand_points, &hull_indices);

            let mut fingertips = Vec::new();
            let mut candidates = Vec::new();

            for defect in defects {
                let start_idx = defect.0;
                let end_idx = defect.1;
                let far_idx = defect.2;
                let depth = defect.3;

                // Valleys must be deep (between fingers)
                if depth > 0.4 * palm_radius_f32 && depth > 10.0 {
                    let start_pt = hand_points[start_idx];
                    let end_pt = hand_points[end_idx];
                    let far_pt = hand_points[far_idx];

                    // Candidates must be far enough from palm center
                    if distance(start_pt, palm_center) > 1.3 * palm_radius_f32 {
                        candidates.push(start_pt);
                    }
                    if distance(end_pt, palm_center) > 1.3 * palm_radius_f32 {
                        candidates.push(end_pt);
                    }

                    if show_preview {
                        // Draw valley depth lines
                        imgproc::circle(
                            &mut frame,
                            far_pt,
                            4,
                            Scalar::new(0.0, 0.0, 255.0, 0.0),
                            -1,
                            8,
                            0,
                        )?;
                        imgproc::line(
                            &mut frame,
                            start_pt,
                            far_pt,
                            Scalar::new(0.0, 255.0, 0.0, 0.0),
                            1,
                            8,
                            0,
                        )?;
                        imgproc::line(
                            &mut frame,
                            end_pt,
                            far_pt,
                            Scalar::new(0.0, 255.0, 0.0, 0.0),
                            1,
                            8,
                            0,
                        )?;
                    }
                }
            }

            // Cluster candidates to merge duplicates
            for cand in candidates {
                let mut exists = false;
                for ft in &fingertips {
                    if distance(cand, *ft) < 0.6 * palm_radius_f32 {
                        exists = true;
                        break;
                    }
                }
                // Fingers should also point upwards generally (Y coordinate smaller than palm)
                if !exists && cand.y < palm_center.y + (0.5 * palm_radius_f32) as i32 {
                    fingertips.push(cand);
                }
            }

            // Draw palm and fingertips in debug mode
            if show_preview {
                imgproc::circle(
                    &mut frame,
                    palm_center,
                    palm_radius_f32 as i32,
                    Scalar::new(255.0, 0.0, 0.0, 0.0),
                    2,
                    8,
                    0,
                )?;
                imgproc::circle(
                    &mut frame,
                    palm_center,
                    6,
                    Scalar::new(255.0, 255.0, 0.0, 0.0),
                    -1,
                    8,
                    0,
                )?;

                for ft in &fingertips {
                    imgproc::circle(
                        &mut frame,
                        *ft,
                        8,
                        Scalar::new(0.0, 255.0, 255.0, 0.0),
                        -1,
                        8,
                        0,
                    )?;
                }
            }

            // Define "tracking bounding box" in center of webcam frame
            // 640x480 resolution -> Center area: X [160, 480], Y [120, 360]
            let track_x_min = 160.0f32;
            let track_x_max = 480.0f32;
            let track_y_min = 120.0f32;
            let track_y_max = 360.0f32;

            if show_preview {
                // Draw tracking region boundary
                imgproc::rectangle(
                    &mut frame,
                    Rect::new(
                        track_x_min as i32,
                        track_y_min as i32,
                        (track_x_max - track_x_min) as i32,
                        (track_y_max - track_y_min) as i32,
                    ),
                    Scalar::new(255.0, 255.0, 255.0, 0.0),
                    1,
                    8,
                    0,
                )?;
            }

            // Helper to map Point inside tracking window to [0.0, 1.0] range
            let map_coordinates = |pt: Point| -> (f32, f32) {
                let nx = ((pt.x as f32) - track_x_min) / (track_x_max - track_x_min);
                let ny = ((pt.y as f32) - track_y_min) / (track_y_max - track_y_min);
                (nx.clamp(0.0, 1.0), ny.clamp(0.0, 1.0))
            };

            // 7. Gesture Classification
            let num_fingers = fingertips.len();

            // Wave Gesture Detection
            let current_time = Instant::now();
            let elapsed_wave = current_time.duration_since(last_wave_time).as_secs_f32();
            let dx = (palm_center.x as f32) - last_palm_x;
            last_palm_x = palm_center.x as f32;

            // Compute smoothed velocity
            let alpha_vel = 0.3f32;
            velocity_x_smoothed = alpha_vel * dx + (1.0 - alpha_vel) * velocity_x_smoothed;

            let mut waved = false;
            if num_fingers >= 4 && elapsed_wave > 1.2 {
                // Fast movement to the left (negative velocity)
                if velocity_x_smoothed < -15.0 {
                    current_state = GestureState::WaveLeft;
                    last_wave_time = current_time;
                    waved = true;
                    println!("Wave Left Detected! (Velocity: {:.2})", velocity_x_smoothed);
                }
                // Fast movement to the right (positive velocity)
                else if velocity_x_smoothed > 15.0 {
                    current_state = GestureState::WaveRight;
                    last_wave_time = current_time;
                    waved = true;
                    println!(
                        "Wave Right Detected! (Velocity: {:.2})",
                        velocity_x_smoothed
                    );
                }
            }

            if !waved {
                match num_fingers {
                    0 => {
                        // Pinch / Fist -> Click/Drag state
                        let coord = map_coordinates(palm_center);
                        current_state = GestureState::Click {
                            x: coord.0,
                            y: coord.1,
                        };
                        last_coord = coord;
                        scroll_initialized = false;
                    }
                    1 => {
                        // 1 Finger extended -> Hover / Move state
                        let tip = fingertips[0];
                        let coord = map_coordinates(tip);
                        current_state = GestureState::Hover {
                            x: coord.0,
                            y: coord.1,
                        };
                        last_coord = coord;
                        scroll_initialized = false;
                    }
                    2 => {
                        // 2 Fingers extended -> Scroll state
                        let avg_y = fingertips.iter().map(|f| f.y as f32).sum::<f32>() / 2.0;
                        if !scroll_initialized {
                            last_scroll_y = avg_y;
                            scroll_initialized = true;
                            current_state = GestureState::Scroll { dy: 0.0 };
                        } else {
                            let dy = (avg_y - last_scroll_y) / 10.0; // Scale scroll sensitivity
                            current_state = GestureState::Scroll { dy };
                            last_scroll_y = avg_y;
                        }
                    }
                    _ => {
                        // Open hand (no wave) -> Hover using last coordinate
                        current_state = GestureState::Hover {
                            x: last_coord.0,
                            y: last_coord.1,
                        };
                        scroll_initialized = false;
                    }
                }
            }
        } else {
            // No hand detected
            current_state = GestureState::Idle;
            scroll_initialized = false;
        }

        // 8. Transmit State to Controller Thread
        tx.send(current_state)?;

        // Draw HUD overlay in preview window
        if show_preview {
            let label = match current_state {
                GestureState::Hover { x, y } => format!("Hover: {:.2}, {:.2}", x, y),
                GestureState::Click { x, y } => format!("Click / Drag: {:.2}, {:.2}", x, y),
                GestureState::Scroll { dy } => format!("Scroll: dy={:.2}", dy),
                GestureState::WaveLeft => "Wave Left (Switch Desktop)".to_string(),
                GestureState::WaveRight => "Wave Right (Switch Desktop)".to_string(),
                GestureState::Idle => "Searching for hand...".to_string(),
            };

            // Display HUD on frame
            imgproc::put_text(
                &mut frame,
                &label,
                Point::new(20, 40),
                imgproc::FONT_HERSHEY_SIMPLEX,
                0.8,
                Scalar::new(0.0, 255.0, 0.0, 0.0),
                2,
                8,
                false,
            )?;

            // Render camera feed and calibration mask
            highgui::imshow(window_name, &frame)?;
            highgui::imshow("Mask View", &mask)?;

            let key = highgui::wait_key(10)?;
            if key == 27 {
                // ESC key pressed
                break;
            }
        }
    }

    if show_preview {
        highgui::destroy_all_windows()?;
    }

    Ok(())
}

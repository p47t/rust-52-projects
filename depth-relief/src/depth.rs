use anyhow::{bail, Result};
use opencv::{
    core::{AlgorithmHint, Mat, Scalar, Size, CV_32F},
    dnn, imgproc,
    prelude::*,
    stereo,
};

/// Estimates the depth map from a single photo using a MiDaS ONNX model.
pub fn estimate_depth_single(img: &Mat, model_path: &str) -> Result<Mat> {
    if img.empty() {
        bail!("Input image is empty");
    }

    // 1. Load the ONNX model (in OpenCV 5, read_net_from_onnx takes an engine parameter)
    let mut net = dnn::read_net_from_onnx(model_path, 0)
        .map_err(|e| anyhow::anyhow!("Failed to load ONNX model from {}: {}", model_path, e))?;

    // 2. Preprocess the image
    // MiDaS Small model expects 256x256 input
    let target_size = Size::new(256, 256);
    let mut resized = Mat::default();
    imgproc::resize(
        img,
        &mut resized,
        target_size,
        0.0,
        0.0,
        imgproc::INTER_CUBIC,
    )?;

    // MiDaS expects normalized RGB input (mean=[0.485, 0.456, 0.406], std=[0.229, 0.224, 0.225])
    let mut preprocessed = Mat::new_rows_cols_with_default(
        target_size.height,
        target_size.width,
        opencv::core::CV_32FC3,
        Scalar::default(),
    )?;

    for y in 0..target_size.height {
        for x in 0..target_size.width {
            let bgr: opencv::core::Vec3b = *resized.at_2d::<opencv::core::Vec3b>(y, x)?;

            // BGR to RGB and scale to [0, 1]
            let r = (bgr[2] as f32) / 255.0;
            let g = (bgr[1] as f32) / 255.0;
            let b = (bgr[0] as f32) / 255.0;

            // Normalize using ImageNet statistics
            let r_norm = (r - 0.485) / 0.229;
            let g_norm = (g - 0.456) / 0.224;
            let b_norm = (b - 0.406) / 0.225;

            *preprocessed.at_2d_mut::<opencv::core::Vec3f>(y, x)? =
                opencv::core::Vec3f::from([r_norm, g_norm, b_norm]);
        }
    }

    // Create 4D blob from preprocessed image
    let blob = dnn::blob_from_image(
        &preprocessed,
        1.0,
        target_size,
        Scalar::default(),
        false, // Already swapped BGR to RGB and resized
        false,
        CV_32F,
    )?;

    // 3. Run inference
    net.set_input(&blob, "", 1.0, Scalar::default())?;
    let mut output = Mat::default();

    // In opencv-rust, net.forward expects a Vector of output blob names
    let out_blob_names = opencv::core::Vector::<String>::new();
    net.forward(&mut output, &out_blob_names)?;

    // 4. Post-process the output
    let dims = output.dims();
    let mat_size = output.mat_size();
    let (h, w) = if dims == 3 {
        (mat_size.get(1)?, mat_size.get(2)?)
    } else {
        (mat_size.get(0)?, mat_size.get(1)?)
    };

    let mut depth_map = Mat::new_rows_cols_with_default(h, w, CV_32F, Scalar::default())?;
    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;

    for y in 0..h {
        for x in 0..w {
            let val = if dims == 3 {
                *output.at_3d::<f32>(0, y, x)?
            } else {
                *output.at_2d::<f32>(y, x)?
            };
            if val < min_val {
                min_val = val;
            }
            if val > max_val {
                max_val = val;
            }
            *depth_map.at_2d_mut::<f32>(y, x)? = val;
        }
    }

    // Normalize output depth map to [0, 1] range
    let diff = max_val - min_val;
    let scale = if diff.abs() > 1e-5 { 1.0 / diff } else { 1.0 };

    for y in 0..h {
        for x in 0..w {
            let val = *depth_map.at_2d::<f32>(y, x)?;
            let norm_val = (val - min_val) * scale;
            *depth_map.at_2d_mut::<f32>(y, x)? = norm_val;
        }
    }

    Ok(depth_map)
}

/// Estimates depth from a stereo pair using Stereo Block Matching (StereoBM).
pub fn estimate_depth_stereo(
    left: &Mat,
    right: &Mat,
    num_disparities: i32,
    block_size: i32,
) -> Result<Mat> {
    if left.empty() || right.empty() {
        bail!("One or both stereo images are empty");
    }

    // 1. Convert inputs to grayscale (OpenCV 5 cvt_color takes AlgorithmHint)
    let mut left_gray = Mat::default();
    let mut right_gray = Mat::default();
    imgproc::cvt_color(
        left,
        &mut left_gray,
        imgproc::COLOR_BGR2GRAY,
        0,
        AlgorithmHint::ALGO_HINT_DEFAULT,
    )?;
    imgproc::cvt_color(
        right,
        &mut right_gray,
        imgproc::COLOR_BGR2GRAY,
        0,
        AlgorithmHint::ALGO_HINT_DEFAULT,
    )?;

    // 2. Initialize and configure StereoBM (in OpenCV 5, StereoBM is in opencv::stereo)
    let mut stereo = stereo::StereoBM::create(num_disparities, block_size)?;
    let mut disparity = Mat::default();
    stereo.compute(&left_gray, &right_gray, &mut disparity)?;

    // 3. Post-process and normalize disparity to [0, 1]
    let h = disparity.rows();
    let w = disparity.cols();
    let mut depth_map = Mat::new_rows_cols_with_default(h, w, CV_32F, Scalar::default())?;

    // Disparity values in StereoBM are represented as 16-bit signed integers (CV_16S)
    // with sub-pixel resolution: actual_disparity = val / 16.0
    // Invalid disparities are negative (often less than 0 or -16)
    let mut min_disp = f32::MAX;
    let mut max_disp = f32::MIN;

    for y in 0..h {
        for x in 0..w {
            let val = *disparity.at_2d::<i16>(y, x)? as f32 / 16.0;
            // Filter invalid disparites (represented by values <= 0)
            if val > 0.0 {
                if val < min_disp {
                    min_disp = val;
                }
                if val > max_disp {
                    max_disp = val;
                }
                *depth_map.at_2d_mut::<f32>(y, x)? = val;
            } else {
                *depth_map.at_2d_mut::<f32>(y, x)? = -1.0; // Mark as invalid for now
            }
        }
    }

    // Normalize valid disparities to [0, 1], invalid ones map to 0.0 (background)
    let diff = max_disp - min_disp;
    let scale = if diff.abs() > 1e-5 { 1.0 / diff } else { 1.0 };

    for y in 0..h {
        for x in 0..w {
            let val = *depth_map.at_2d::<f32>(y, x)?;
            if val >= 0.0 {
                let norm_val = (val - min_disp) * scale;
                *depth_map.at_2d_mut::<f32>(y, x)? = norm_val;
            } else {
                *depth_map.at_2d_mut::<f32>(y, x)? = 0.0; // Map invalid to flat background
            }
        }
    }

    Ok(depth_map)
}

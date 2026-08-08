use anyhow::{bail, Result};
use opencv::{
    core::{AlgorithmHint, Mat, Scalar, Size, CV_32F},
    dnn, imgproc,
    prelude::*,
    stereo,
};

#[derive(Debug, Clone, Copy)]
pub struct DepthPostProcessOptions {
    /// Quantile lower bound for clipping background outliers (0.0 to 0.49)
    pub clip_min_quantile: f32,
    /// Quantile upper bound for clipping foreground peak outliers (0.51 to 1.0)
    pub clip_max_quantile: f32,
    /// Gamma contrast exponent (< 1.0 boosts facial feature midtones)
    pub gamma: f32,
    /// High-pass photo detail weight (0.0 to 1.0) to engrave sharp eyes, nose, lips, hair
    pub detail_weight: f32,
    /// Enable CLAHE for adaptive local feature sharpening
    pub enable_clahe: bool,
    /// Spatial radius for bilateral edge-preserving smoothing (0 to disable)
    pub smooth_radius: i32,
}

impl Default for DepthPostProcessOptions {
    fn default() -> Self {
        Self {
            clip_min_quantile: 0.01,
            clip_max_quantile: 0.99,
            gamma: 0.7,
            detail_weight: 0.4,
            enable_clahe: false,
            smooth_radius: 3,
        }
    }
}

pub fn extract_high_pass_detail(img: &Mat, target_size: Size) -> Result<Mat> {
    let mut gray = Mat::default();
    if img.channels() > 1 {
        imgproc::cvt_color(
            img,
            &mut gray,
            imgproc::COLOR_BGR2GRAY,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;
    } else {
        gray = img.clone();
    }

    let mut resized_gray = Mat::default();
    imgproc::resize(
        &gray,
        &mut resized_gray,
        target_size,
        0.0,
        0.0,
        imgproc::INTER_CUBIC,
    )?;

    let mut f32_gray = Mat::default();
    resized_gray.convert_to(&mut f32_gray, CV_32F, 1.0 / 255.0, 0.0)?;

    let mut blurred = Mat::default();
    let ksize = Size::new(15, 15);
    imgproc::gaussian_blur(
        &f32_gray,
        &mut blurred,
        ksize,
        3.0,
        3.0,
        opencv::core::BORDER_DEFAULT,
        AlgorithmHint::ALGO_HINT_DEFAULT,
    )?;

    let mut detail_map = Mat::new_rows_cols_with_default(
        target_size.height,
        target_size.width,
        CV_32F,
        Scalar::default(),
    )?;

    for y in 0..target_size.height {
        for x in 0..target_size.width {
            let orig_val = *f32_gray.at_2d::<f32>(y, x)?;
            let blur_val = *blurred.at_2d::<f32>(y, x)?;
            let hp = orig_val - blur_val;
            *detail_map.at_2d_mut::<f32>(y, x)? = hp;
        }
    }

    Ok(detail_map)
}

pub fn post_process_depth_map(
    raw_map: &Mat,
    opt_source_img: Option<&Mat>,
    options: &DepthPostProcessOptions,
) -> Result<Mat> {
    let h = raw_map.rows();
    let w = raw_map.cols();
    let total_pixels = (w * h) as usize;
    if total_pixels == 0 {
        bail!("Empty depth map matrix");
    }

    // 1. Gather valid float depth values for quantile computation
    let mut valid_vals: Vec<f32> = Vec::with_capacity(total_pixels);
    for y in 0..h {
        for x in 0..w {
            let val = *raw_map.at_2d::<f32>(y, x)?;
            if val.is_finite() && val > -0.5 {
                valid_vals.push(val);
            }
        }
    }

    if valid_vals.is_empty() {
        bail!("No valid depth pixels found");
    }

    valid_vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let num_valid = valid_vals.len();

    let min_q = options.clip_min_quantile.clamp(0.0, 0.49);
    let max_q = options.clip_max_quantile.clamp(0.51, 1.0);

    let min_idx = ((num_valid - 1) as f32 * min_q) as usize;
    let max_idx = ((num_valid - 1) as f32 * max_q) as usize;

    let v_min = valid_vals[min_idx];
    let v_max = valid_vals[max_idx];
    let range = if (v_max - v_min).abs() > 1e-6 { v_max - v_min } else { 1.0 };

    println!(
        "Post-processing Depth: Clip range [{:.2}, {:.2}] (quantiles {:.1}% - {:.1}%), Gamma={:.2}",
        v_min, v_max, min_q * 100.0, max_q * 100.0, options.gamma
    );

    // 2. Clamp, Normalize to [0, 1], and apply Gamma Curve
    let mut norm_map = Mat::new_rows_cols_with_default(h, w, CV_32F, Scalar::default())?;
    let gamma_exp = if options.gamma > 0.0 { options.gamma } else { 1.0 };

    for y in 0..h {
        for x in 0..w {
            let val = *raw_map.at_2d::<f32>(y, x)?;
            if val <= -0.5 {
                *norm_map.at_2d_mut::<f32>(y, x)? = 0.0;
            } else {
                let clamped = val.clamp(v_min, v_max);
                let norm = (clamped - v_min) / range;
                let final_val = if (gamma_exp - 1.0).abs() > 1e-4 {
                    norm.powf(gamma_exp)
                } else {
                    norm
                };
                *norm_map.at_2d_mut::<f32>(y, x)? = final_val;
            }
        }
    }

    // 3. High-pass Photo Detail Blending (for crisp eyes, nose, lips, hair texture)
    if let Some(src_img) = opt_source_img {
        if options.detail_weight > 0.0 {
            println!(
                "Blending high-pass photo details (detail_weight={:.2})...",
                options.detail_weight
            );
            let target_sz = Size::new(w, h);
            let hp_detail = extract_high_pass_detail(src_img, target_sz)?;
            let weight = options.detail_weight;

            for y in 0..h {
                for x in 0..w {
                    let base_val = *norm_map.at_2d::<f32>(y, x)?;
                    let hp_val = *hp_detail.at_2d::<f32>(y, x)?;
                    let blended = (base_val + weight * hp_val).clamp(0.0, 1.0);
                    *norm_map.at_2d_mut::<f32>(y, x)? = blended;
                }
            }
        }
    }

    // 4. Bilateral / Edge-preserving smoothing
    let mut smooth_map = if options.smooth_radius > 0 {
        let mut dst = Mat::default();
        let d = options.smooth_radius * 2 + 1;
        let sigma_color = 0.1f64;
        let sigma_space = options.smooth_radius as f64;
        imgproc::bilateral_filter(
            &norm_map,
            &mut dst,
            d,
            sigma_color,
            sigma_space,
            opencv::core::BORDER_DEFAULT,
        )?;
        dst
    } else {
        norm_map
    };

    // 5. CLAHE enhancement (optional)
    if options.enable_clahe {
        println!("Applying CLAHE (Contrast Limited Adaptive Histogram Equalization)...");
        let mut u8_map = Mat::default();
        smooth_map.convert_to(&mut u8_map, opencv::core::CV_8U, 255.0, 0.0)?;

        let mut clahe_obj = imgproc::create_clahe_def()?;
        clahe_obj.set_clip_limit(3.0)?;
        clahe_obj.set_tiles_grid_size(Size::new(8, 8))?;

        let mut clahe_out = Mat::default();
        clahe_obj.apply(&u8_map, &mut clahe_out)?;

        let mut f32_clahe = Mat::default();
        clahe_out.convert_to(&mut f32_clahe, CV_32F, 1.0 / 255.0, 0.0)?;
        smooth_map = f32_clahe;
    }

    Ok(smooth_map)
}

/// Estimates the depth map from a single photo using a MiDaS ONNX model.
pub fn estimate_depth_single(
    img: &Mat,
    model_path: &str,
    options: &DepthPostProcessOptions,
) -> Result<Mat> {
    if img.empty() {
        bail!("Input image is empty");
    }

    // 1. Load the ONNX model using OpenCV DNN default engine
    let mut net = dnn::read_net_from_onnx_def(model_path)
        .map_err(|e| anyhow::anyhow!("Failed to load ONNX model from {}: {}", model_path, e))?;

    // 2. Preprocess the image
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

    // MiDaS expects normalized RGB input
    let mut preprocessed = Mat::new_rows_cols_with_default(
        target_size.height,
        target_size.width,
        opencv::core::CV_32FC3,
        Scalar::default(),
    )?;

    for y in 0..target_size.height {
        for x in 0..target_size.width {
            let bgr: opencv::core::Vec3b = *resized.at_2d::<opencv::core::Vec3b>(y, x)?;

            let r = (bgr[2] as f32) / 255.0;
            let g = (bgr[1] as f32) / 255.0;
            let b = (bgr[0] as f32) / 255.0;

            let r_norm = (r - 0.485) / 0.229;
            let g_norm = (g - 0.456) / 0.224;
            let b_norm = (b - 0.406) / 0.225;

            *preprocessed.at_2d_mut::<opencv::core::Vec3f>(y, x)? =
                opencv::core::Vec3f::from([r_norm, g_norm, b_norm]);
        }
    }

    let blob = dnn::blob_from_image(
        &preprocessed,
        1.0,
        target_size,
        Scalar::default(),
        false,
        false,
        CV_32F,
    )?;

    // 3. Run inference
    net.set_input(&blob, "", 1.0, Scalar::default())?;
    let mut output = Mat::default();

    let out_blob_names = opencv::core::Vector::<String>::new();
    net.forward(&mut output, &out_blob_names)?;

    // 4. Extract raw depth matrix
    let dims = output.dims();
    let mat_size = output.mat_size();
    let (h, w) = if dims == 3 {
        (mat_size.get(1)?, mat_size.get(2)?)
    } else {
        (mat_size.get(0)?, mat_size.get(1)?)
    };

    let mut raw_depth = Mat::new_rows_cols_with_default(h, w, CV_32F, Scalar::default())?;
    for y in 0..h {
        for x in 0..w {
            let val = if dims == 3 {
                *output.at_3d::<f32>(0, y, x)?
            } else {
                *output.at_2d::<f32>(y, x)?
            };
            *raw_depth.at_2d_mut::<f32>(y, x)? = val;
        }
    }

    // 5. Post-process depth map (quantile clipping, gamma correction, photo detail blending, smoothing, CLAHE)
    post_process_depth_map(&raw_depth, Some(img), options)
}

/// Estimates depth from a stereo pair using Stereo Block Matching (StereoBM).
pub fn estimate_depth_stereo(
    left: &Mat,
    right: &Mat,
    num_disparities: i32,
    block_size: i32,
    options: &DepthPostProcessOptions,
) -> Result<Mat> {
    if left.empty() || right.empty() {
        bail!("One or both stereo images are empty");
    }

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

    let mut stereo = stereo::StereoBM::create(num_disparities, block_size)?;
    let mut disparity = Mat::default();
    stereo.compute(&left_gray, &right_gray, &mut disparity)?;

    let h = disparity.rows();
    let w = disparity.cols();
    let mut raw_depth = Mat::new_rows_cols_with_default(h, w, CV_32F, Scalar::default())?;

    for y in 0..h {
        for x in 0..w {
            let val = *disparity.at_2d::<i16>(y, x)? as f32 / 16.0;
            if val > 0.0 {
                *raw_depth.at_2d_mut::<f32>(y, x)? = val;
            } else {
                *raw_depth.at_2d_mut::<f32>(y, x)? = -1.0;
            }
        }
    }

    post_process_depth_map(&raw_depth, Some(left), options)
}

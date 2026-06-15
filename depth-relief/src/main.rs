use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};
use opencv::{core::Mat, prelude::*};
use std::path::{Path, PathBuf};

mod depth;
mod stl;

#[derive(Parser)]
#[command(
    name = "depth-relief",
    version,
    about = "Generates 3D-printable depth reliefs (bas-reliefs) from photos"
)]
struct Cli {
    /// Mode of operation: single photo depth estimation or stereo pair matching
    #[arg(short, long, value_enum, default_value_t = Mode::Single)]
    mode: Mode,

    /// Path to input image (required for single image mode)
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Path to left image (required for stereo mode)
    #[arg(long)]
    left: Option<PathBuf>,

    /// Path to right image (required for stereo mode)
    #[arg(long)]
    right: Option<PathBuf>,

    /// Path to the MiDaS ONNX model file
    #[arg(short, long, default_value = "model-small.onnx")]
    model: PathBuf,

    /// Output STL file path
    #[arg(short = 'o', long, default_value = "relief.stl")]
    output_stl: PathBuf,

    /// Output depth map image path (saved as grayscale PNG)
    #[arg(short = 'd', long, default_value = "depth.png")]
    output_depth: PathBuf,

    /// Physical width of the 3D relief in mm
    #[arg(long, default_value_t = 100.0)]
    width: f32,

    /// Physical height of the 3D relief in mm (if 0.0, automatically calculated to match image aspect ratio)
    #[arg(long, default_value_t = 0.0)]
    height: f32,

    /// Relief amplitude (maximum depth displacement) in mm
    #[arg(short = 'z', long, default_value_t = 8.0)]
    relief_height: f32,

    /// Flat base height in mm
    #[arg(short = 'b', long, default_value_t = 2.0)]
    base_height: f32,

    /// Number of disparities for stereo matching (must be divisible by 16)
    #[arg(long, default_value_t = 64)]
    stereo_disparities: i32,

    /// Block size for stereo matching (must be an odd integer >= 5)
    #[arg(long, default_value_t = 15)]
    stereo_block_size: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Mode {
    /// Single photo depth estimation using MiDaS ONNX
    Single,
    /// Stereo photo depth estimation using Block Matching
    Stereo,
}

/// Helper function to check for the MiDaS model file and download it if missing.
fn check_and_download_model(model_path: &Path) -> Result<()> {
    if model_path.exists() {
        return Ok(());
    }

    // Only auto-download if the user is using the default "model-small.onnx" filename
    if model_path.file_name().and_then(|n| n.to_str()) != Some("model-small.onnx") {
        bail!(
            "Model file not found at: {}\n\
             Please download a MiDaS ONNX model (e.g. model-small.onnx) and place it at that path.",
            model_path.display()
        );
    }

    println!("MiDaS model file '{}' not found.", model_path.display());
    println!(
        "Downloading model-small.onnx (approx. 58MB) from official Intel ISL GitHub releases..."
    );

    let url = "https://github.com/intel-isl/MiDaS/releases/download/v2_1/model-small.onnx";

    // Attempt download using curl (available out of the box in Windows 10/11)
    let status = std::process::Command::new("curl")
        .arg("-L")
        .arg("-o")
        .arg(model_path)
        .arg(url)
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("Download complete!");
            Ok(())
        }
        _ => {
            // Fallback to PowerShell Invoke-WebRequest if curl fails
            println!("curl download failed or not found. Retrying with PowerShell...");
            let ps_status = std::process::Command::new("powershell")
                .arg("-Command")
                .arg(format!(
                    "Invoke-WebRequest -Uri '{}' -OutFile '{}'",
                    url,
                    model_path.to_string_lossy()
                ))
                .status();

            match ps_status {
                Ok(s) if s.success() => {
                    println!("Download complete!");
                    Ok(())
                }
                _ => {
                    bail!(
                        "Failed to download model file. Please download it manually from:\n  {}\nand place it at '{}'",
                        url,
                        model_path.display()
                    );
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // 1. Get depth map based on selected mode
    let depth_map = match cli.mode {
        Mode::Single => {
            let input_path = cli.input.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Input image path is required for 'single' mode. Use --input or -i")
            })?;

            if !input_path.exists() {
                bail!("Input image file not found: {}", input_path.display());
            }

            check_and_download_model(&cli.model)?;

            println!("Reading input image: {}", input_path.display());
            let img = opencv::imgcodecs::imread(
                &input_path.to_string_lossy(),
                opencv::imgcodecs::IMREAD_COLOR,
            )?;

            println!("Estimating depth using MiDaS ONNX model...");
            depth::estimate_depth_single(&img, &cli.model.to_string_lossy())?
        }
        Mode::Stereo => {
            let left_path = cli.left.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Left image path is required for 'stereo' mode. Use --left")
            })?;
            let right_path = cli.right.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Right image path is required for 'stereo' mode. Use --right")
            })?;

            if !left_path.exists() {
                bail!("Left image file not found: {}", left_path.display());
            }
            if !right_path.exists() {
                bail!("Right image file not found: {}", right_path.display());
            }

            if cli.stereo_disparities <= 0 || cli.stereo_disparities % 16 != 0 {
                bail!(
                    "Stereo disparities must be positive and divisible by 16 (got {}).",
                    cli.stereo_disparities
                );
            }
            if cli.stereo_block_size < 5 || cli.stereo_block_size % 2 == 0 {
                bail!(
                    "Stereo block size must be an odd integer >= 5 (got {}).",
                    cli.stereo_block_size
                );
            }

            println!(
                "Reading stereo pair:\n  Left: {}\n  Right: {}",
                left_path.display(),
                right_path.display()
            );
            let left_img = opencv::imgcodecs::imread(
                &left_path.to_string_lossy(),
                opencv::imgcodecs::IMREAD_COLOR,
            )?;
            let right_img = opencv::imgcodecs::imread(
                &right_path.to_string_lossy(),
                opencv::imgcodecs::IMREAD_COLOR,
            )?;

            println!("Estimating depth (disparity) using StereoBM block matching...");
            depth::estimate_depth_stereo(
                &left_img,
                &right_img,
                cli.stereo_disparities,
                cli.stereo_block_size,
            )?
        }
    };

    let w = depth_map.cols();
    let h = depth_map.rows();
    println!("Depth map size: {}x{}", w, h);

    // 2. Determine physical dimensions
    let aspect_ratio = (w as f32) / (h as f32);
    let (width_mm, height_mm) = if cli.height == 0.0 {
        let h_calc = cli.width / aspect_ratio;
        println!("Calculated height from aspect ratio: {:.2} mm", h_calc);
        (cli.width, h_calc)
    } else {
        (cli.width, cli.height)
    };

    // 3. Save depth map visualization as a grayscale PNG
    let mut vis_map = Mat::default();
    depth_map.convert_to(&mut vis_map, opencv::core::CV_8U, 255.0, 0.0)?;
    println!(
        "Saving depth map visualization to: {}",
        cli.output_depth.display()
    );
    opencv::imgcodecs::imwrite(
        &cli.output_depth.to_string_lossy(),
        &vis_map,
        &opencv::core::Vector::new(),
    )?;

    // 4. Extract raw depth data from Mat to a flat Vec<f32> (row-major order)
    let mut flat_depth_data = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            flat_depth_data[(y * w + x) as usize] = *depth_map.at_2d::<f32>(y, x)?;
        }
    }

    // 5. Generate STL Mesh
    println!("Triangulating heightfield into watertight STL mesh...");
    let config = stl::ReliefConfig {
        width_mm,
        height_mm,
        relief_height_mm: cli.relief_height,
        base_height_mm: cli.base_height,
    };
    let triangles = stl::generate_mesh(&flat_depth_data, w as usize, h as usize, &config);
    println!("Generated {} triangles.", triangles.len());

    // 6. Export to STL file
    println!("Exporting STL to: {}", cli.output_stl.display());
    let stl_file = std::fs::File::create(&cli.output_stl)?;
    let mut buf_writer = std::io::BufWriter::new(stl_file);
    stl::write_binary_stl(&mut buf_writer, &triangles)?;

    println!("Success! You can now import the STL file into a slicer or OpenSCAD.");
    Ok(())
}

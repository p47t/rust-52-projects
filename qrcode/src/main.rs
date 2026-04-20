mod qr;

use clap::{Parser, Subcommand};
use image::{ImageBuffer, ImageFormat, Luma};
use qr::EcLevel;
use std::io::Read;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "qrcode")]
#[command(about = "QR code encoder in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Encode {
        data: Option<String>,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = "10")]
        module_size: u32,
    },
    Decode {
        input: Option<PathBuf>,
    },
    Render {
        data: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Encode {
            data,
            output,
            module_size,
        } => {
            let data = data.unwrap_or_else(|| {
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s).unwrap();
                s.trim().to_string()
            });

            let qroc = qr::generate(&data, EcLevel::M)?;

            let quiet = 4 * module_size;
            let qr_size = qroc.size as u32 * module_size;
            let size = qr_size + quiet * 2;
            let mut img = ImageBuffer::from_pixel(size, size, Luma([255u8]));

            for y in 0..qroc.size as u32 {
                for x in 0..qroc.size as u32 {
                    let color = if qroc.matrix[y as usize][x as usize] {
                        Luma([0u8])
                    } else {
                        Luma([255u8])
                    };
                    for dy in 0..module_size {
                        for dx in 0..module_size {
                            img.put_pixel(
                                quiet + x * module_size + dx,
                                quiet + y * module_size + dy,
                                color,
                            );
                        }
                    }
                }
            }

            let output = output.unwrap_or_else(|| PathBuf::from("qrcode.png"));
            img.save_with_format(&output, ImageFormat::Png)?;
            eprintln!("QR width: {}, image: {}x{}", qroc.size, size, size);
            println!("Written to {} (minimal v1)", output.display());
        }
        Commands::Decode { input } => {
            let input = input.unwrap_or_else(|| PathBuf::from("qrcode.png"));
            let img = image::open(&input)?.to_luma8();
            let mut prepared = rqrr::PreparedImage::prepare(img);
            let grids = prepared.detect_grids();

            let result = grids
                .into_iter()
                .next()
                .ok_or("No QR code found")?
                .decode()
                .map(|(_, c)| c);

            match result {
                Ok(content) => println!("{}", content),
                Err(e) => eprintln!("Decode error: {}", e),
            }
        }
        Commands::Render { data } => {
            let data = data.unwrap_or_else(|| {
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s).unwrap();
                s.trim().to_string()
            });

            let qroc = qr::generate(&data, EcLevel::M)?;
            println!("{}", qroc.render_as_string());
        }
    }

    Ok(())
}

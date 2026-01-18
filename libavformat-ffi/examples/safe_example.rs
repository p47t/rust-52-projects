//! Example using the safe Rust wrapper.
//!
//! This demonstrates the idiomatic Rust API built on top of the FFI bindings.
//! No unsafe code is needed - resource management is handled via RAII.
//!
//! Run with: cargo run --example safe_example -- /path/to/video.mp4

use libavformat_ffi::safe::{FormatContext, MediaType, Packet};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <media_file>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];

    // Open the media file
    println!("Opening: {}", path);
    let mut ctx = FormatContext::open(path)?;

    // Print format information
    if let Some(name) = ctx.format_name() {
        println!("Format: {}", name);
    }

    if let Some(duration) = ctx.duration_secs() {
        println!("Duration: {:.2} seconds", duration);
    }

    // Print stream information
    println!(
        "\n--- Stream Information ({} streams) ---",
        ctx.nb_streams()
    );
    for stream in ctx.streams() {
        let type_str = match stream.media_type {
            MediaType::Video => "video",
            MediaType::Audio => "audio",
            MediaType::Subtitle => "subtitle",
            MediaType::Data => "data",
            MediaType::Attachment => "attachment",
            MediaType::Unknown => "unknown",
        };

        println!(
            "Stream {}: type={}, codec_id={}, bitrate={}",
            stream.index, type_str, stream.codec_id, stream.bit_rate
        );

        match stream.media_type {
            MediaType::Video => {
                println!(
                    "  Video: {}x{}, time_base={}/{}",
                    stream.width, stream.height, stream.time_base_num, stream.time_base_den
                );
                if let Some(dur) = stream.duration_secs() {
                    println!("  Duration: {:.2} seconds", dur);
                }
            }
            MediaType::Audio => {
                println!(
                    "  Audio: time_base={}/{}",
                    stream.time_base_num, stream.time_base_den
                );
            }
            _ => {}
        }
    }

    // Dump format info
    println!("\n--- Format Dump ---");
    ctx.dump_format();

    // Create a packet for reading
    let mut packet = Packet::new()?;

    // Read packets and gather statistics
    println!("\n--- Reading Packets ---");
    let mut packet_count = 0;
    let mut video_packets = 0;
    let mut audio_packets = 0;
    let mut total_bytes = 0u64;
    let mut keyframes = 0;

    // Read first 100 packets for statistics
    while ctx.read_packet(&mut packet)? {
        let stream_info = ctx.stream_info(packet.stream_index() as usize);
        if let Some(info) = stream_info {
            match info.media_type {
                MediaType::Video => video_packets += 1,
                MediaType::Audio => audio_packets += 1,
                _ => {}
            }
        }

        total_bytes += packet.size() as u64;

        if packet.is_keyframe() {
            keyframes += 1;
        }

        // Print first 10 packets in detail
        if packet_count < 10 {
            println!(
                "Packet {}: stream={}, pts={}, dts={}, size={}, keyframe={}",
                packet_count,
                packet.stream_index(),
                packet.pts(),
                packet.dts(),
                packet.size(),
                packet.is_keyframe()
            );
        }

        packet_count += 1;
        if packet_count >= 100 {
            break;
        }
    }

    // Print statistics
    println!("\n--- Statistics (first {} packets) ---", packet_count);
    println!("Video packets: {}", video_packets);
    println!("Audio packets: {}", audio_packets);
    println!("Keyframes: {}", keyframes);
    println!("Total bytes: {}", total_bytes);
    if packet_count > 0 {
        println!(
            "Average packet size: {:.1} bytes",
            total_bytes as f64 / packet_count as f64
        );
    }

    println!("\nSafe wrapper example completed successfully!");

    // Resources are automatically cleaned up when ctx and packet go out of scope
    Ok(())
}

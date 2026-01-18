//! Example using the manual FFI bindings.
//!
//! This demonstrates direct use of hand-written extern "C" declarations.
//! It requires careful handling of raw pointers and manual memory management.
//!
//! Run with: cargo run --example manual_example -- /path/to/video.mp4

use libavformat_ffi::manual::{self, AVFormatContext, AVPacket};
use std::env;
use std::ffi::CString;
use std::ptr;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <media_file>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    let c_path = CString::new(path.as_str()).expect("Path contains null byte");

    unsafe {
        // Allocate format context pointer
        let mut ctx: *mut AVFormatContext = ptr::null_mut();

        // Open the input file
        println!("Opening: {}", path);
        let ret =
            manual::avformat_open_input(&mut ctx, c_path.as_ptr(), ptr::null(), ptr::null_mut());

        if ret < 0 {
            eprintln!("Error opening input: {}", manual::get_error_string(ret));
            std::process::exit(1);
        }

        // Find stream information
        let ret = manual::avformat_find_stream_info(ctx, ptr::null_mut());
        if ret < 0 {
            eprintln!(
                "Error finding stream info: {}",
                manual::get_error_string(ret)
            );
            manual::avformat_close_input(&mut ctx);
            std::process::exit(1);
        }

        // Print version info
        let version = manual::avformat_version();
        let major = (version >> 16) & 0xff;
        let minor = (version >> 8) & 0xff;
        let micro = version & 0xff;
        println!("libavformat version: {}.{}.{}", major, minor, micro);

        // Dump format info to stderr
        println!("\n--- Format Information ---");
        manual::av_dump_format(ctx, 0, c_path.as_ptr(), 0);

        // Allocate packet for reading
        let pkt: *mut AVPacket = manual::av_packet_alloc();
        if pkt.is_null() {
            eprintln!("Failed to allocate packet");
            manual::avformat_close_input(&mut ctx);
            std::process::exit(1);
        }

        // Read first 10 packets
        println!("\n--- First 10 Packets ---");
        let mut count = 0;
        loop {
            let ret = manual::av_read_frame(ctx, pkt);
            if ret < 0 {
                if ret == manual::AVERROR_EOF {
                    println!("End of file reached");
                } else {
                    println!("Read error: {}", manual::get_error_string(ret));
                }
                break;
            }

            println!(
                "Packet {}: stream={}, pts={}, dts={}, size={}, keyframe={}",
                count,
                (*pkt).stream_index,
                (*pkt).pts,
                (*pkt).dts,
                (*pkt).size,
                ((*pkt).flags & 0x0001) != 0
            );

            // Unreference packet for next read
            manual::av_packet_unref(pkt);

            count += 1;
            if count >= 10 {
                break;
            }
        }

        // Clean up
        let mut pkt_ptr = pkt;
        manual::av_packet_free(&mut pkt_ptr);
        manual::avformat_close_input(&mut ctx);

        println!("\nManual FFI example completed successfully!");
    }
}

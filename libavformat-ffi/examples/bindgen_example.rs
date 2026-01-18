//! Example using the bindgen-generated FFI bindings.
//!
//! This demonstrates using automatically generated bindings from bindgen.
//! The types and functions are auto-generated from FFmpeg headers.
//!
//! Run with: cargo run --example bindgen_example -- /path/to/video.mp4

use libavformat_ffi::bindgen;
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
        let mut ctx: *mut bindgen::AVFormatContext = ptr::null_mut();

        // Open the input file
        println!("Opening: {}", path);
        let ret =
            bindgen::avformat_open_input(&mut ctx, c_path.as_ptr(), ptr::null(), ptr::null_mut());

        if ret < 0 {
            eprintln!("Error opening input: {}", bindgen::get_error_string(ret));
            std::process::exit(1);
        }

        // Find stream information
        let ret = bindgen::avformat_find_stream_info(ctx, ptr::null_mut());
        if ret < 0 {
            eprintln!(
                "Error finding stream info: {}",
                bindgen::get_error_string(ret)
            );
            bindgen::avformat_close_input(&mut ctx);
            std::process::exit(1);
        }

        // Print version info
        let version = bindgen::avformat_version();
        let major = (version >> 16) & 0xff;
        let minor = (version >> 8) & 0xff;
        let micro = version & 0xff;
        println!("libavformat version: {}.{}.{}", major, minor, micro);

        // Print stream information using bindgen types
        let nb_streams = (*ctx).nb_streams;
        println!("\n--- Stream Information ({} streams) ---", nb_streams);

        for i in 0..nb_streams {
            let stream = *(*ctx).streams.add(i as usize);
            let codecpar = (*stream).codecpar;

            let media_type = match (*codecpar).codec_type {
                bindgen::AVMediaType::AVMEDIA_TYPE_VIDEO => "video",
                bindgen::AVMediaType::AVMEDIA_TYPE_AUDIO => "audio",
                bindgen::AVMediaType::AVMEDIA_TYPE_SUBTITLE => "subtitle",
                bindgen::AVMediaType::AVMEDIA_TYPE_DATA => "data",
                _ => "unknown",
            };

            println!(
                "Stream {}: type={}, codec_id={}, bitrate={}",
                i,
                media_type,
                (*codecpar).codec_id,
                (*codecpar).bit_rate
            );

            if (*codecpar).codec_type == bindgen::AVMediaType::AVMEDIA_TYPE_VIDEO {
                println!(
                    "  Video: {}x{}, time_base={}/{}",
                    (*codecpar).width,
                    (*codecpar).height,
                    (*stream).time_base.num,
                    (*stream).time_base.den
                );
            } else if (*codecpar).codec_type == bindgen::AVMediaType::AVMEDIA_TYPE_AUDIO {
                println!(
                    "  Audio: sample_rate={}, channels={}",
                    (*codecpar).sample_rate,
                    (*codecpar).ch_layout.nb_channels
                );
            }
        }

        // Dump format info
        println!("\n--- Format Dump ---");
        bindgen::av_dump_format(ctx, 0, c_path.as_ptr(), 0);

        // Allocate packet for reading
        let pkt = bindgen::av_packet_alloc();
        if pkt.is_null() {
            eprintln!("Failed to allocate packet");
            bindgen::avformat_close_input(&mut ctx);
            std::process::exit(1);
        }

        // Read first 10 packets
        println!("\n--- First 10 Packets ---");
        let mut count = 0;
        loop {
            let ret = bindgen::av_read_frame(ctx, pkt);
            if ret < 0 {
                let eof_code =
                    -('E' as i32 | ('O' as i32) << 8 | ('F' as i32) << 16 | (' ' as i32) << 24);
                if ret == eof_code {
                    println!("End of file reached");
                } else {
                    println!("Read error: {}", bindgen::get_error_string(ret));
                }
                break;
            }

            let keyframe = ((*pkt).flags & 0x0001) != 0;
            println!(
                "Packet {}: stream={}, pts={}, dts={}, size={}, keyframe={}",
                count,
                (*pkt).stream_index,
                (*pkt).pts,
                (*pkt).dts,
                (*pkt).size,
                keyframe
            );

            // Unreference packet for next read
            bindgen::av_packet_unref(pkt);

            count += 1;
            if count >= 10 {
                break;
            }
        }

        // Clean up
        let mut pkt_ptr = pkt;
        bindgen::av_packet_free(&mut pkt_ptr);
        bindgen::avformat_close_input(&mut ctx);

        println!("\nBindgen FFI example completed successfully!");
    }
}

use std::env;
use std::path::PathBuf;

fn main() {
    // Use pkg-config to find FFmpeg libraries
    let avformat = pkg_config::probe_library("libavformat").expect(
        "libavformat not found. Install FFmpeg development libraries:\n\
         Ubuntu/Debian: sudo apt install libavformat-dev libavcodec-dev libavutil-dev\n\
         Fedora: sudo dnf install ffmpeg-devel\n\
         macOS: brew install ffmpeg",
    );

    let avcodec = pkg_config::probe_library("libavcodec").expect("libavcodec not found");
    let avutil = pkg_config::probe_library("libavutil").expect("libavutil not found");

    // Collect all include paths
    let mut include_paths = Vec::new();
    include_paths.extend(avformat.include_paths.iter().cloned());
    include_paths.extend(avcodec.include_paths.iter().cloned());
    include_paths.extend(avutil.include_paths.iter().cloned());

    // Configure bindgen
    let mut builder = bindgen::Builder::default()
        .header_contents(
            "wrapper.h",
            r#"
            #include <libavformat/avformat.h>
            #include <libavcodec/avcodec.h>
            #include <libavutil/avutil.h>
            #include <libavutil/error.h>
            "#,
        )
        // Allowlist functions we need
        .allowlist_function("avformat_open_input")
        .allowlist_function("avformat_find_stream_info")
        .allowlist_function("avformat_close_input")
        .allowlist_function("av_read_frame")
        .allowlist_function("av_packet_alloc")
        .allowlist_function("av_packet_free")
        .allowlist_function("av_packet_unref")
        .allowlist_function("av_strerror")
        .allowlist_function("avformat_version")
        .allowlist_function("av_dump_format")
        .allowlist_function("avcodec_get_name")
        .allowlist_function("av_dict_get")
        .allowlist_function("avio_size")
        // Allowlist types
        .allowlist_type("AVFormatContext")
        .allowlist_type("AVPacket")
        .allowlist_type("AVStream")
        .allowlist_type("AVCodecParameters")
        .allowlist_type("AVMediaType")
        .allowlist_type("AVDictionary")
        .allowlist_type("AVDictionaryEntry")
        .allowlist_type("AVIOContext")
        .allowlist_type("AVRational")
        // Allowlist constants
        .allowlist_var("AVMEDIA_TYPE_.*")
        .allowlist_var("AV_NOPTS_VALUE")
        .allowlist_var("AVERROR.*")
        // Generate constants as enums where possible
        .rustified_enum("AVMediaType")
        // Derive traits
        .derive_debug(true)
        .derive_default(true)
        // Layout tests can be noisy, disable them
        .layout_tests(false);

    // Add include paths to bindgen
    for path in &include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }

    // Generate bindings
    let bindings = builder.generate().expect("Failed to generate bindings");

    // Write bindings to output file
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Failed to write bindings");

    // Re-run build script if these change
    println!("cargo:rerun-if-changed=build.rs");
}

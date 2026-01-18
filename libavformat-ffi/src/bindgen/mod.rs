//! Auto-generated FFI bindings via bindgen.
//!
//! This module provides bindings automatically generated at build time from
//! the FFmpeg C headers using bindgen.
//!
//! Advantages:
//! - Accurate: generated directly from C headers
//! - Complete: includes all types, functions, and constants
//! - Maintainable: regenerates when headers change
//!
//! Disadvantages:
//! - Build-time dependency on libclang
//! - Generated code can be verbose
//! - May include more than needed

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::all)]

// Include the generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// Helper function to get a Rust String from an FFmpeg error code
pub fn get_error_string(errnum: std::ffi::c_int) -> String {
    let mut buf = [0i8; 256];
    unsafe {
        av_strerror(errnum, buf.as_mut_ptr(), buf.len());
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    cstr.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avformat_version() {
        let version = unsafe { avformat_version() };
        assert!(version > 0);
        let major = (version >> 16) & 0xff;
        let minor = (version >> 8) & 0xff;
        let micro = version & 0xff;
        println!(
            "libavformat version (bindgen): {}.{}.{}",
            major, minor, micro
        );
    }

    #[test]
    fn test_packet_alloc_free() {
        unsafe {
            let pkt = av_packet_alloc();
            assert!(!pkt.is_null());
            av_packet_free(&mut (pkt as *mut _));
        }
    }
}

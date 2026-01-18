//! Hand-written FFI declarations for libavformat.
//!
//! This module demonstrates the manual approach to FFI, where you write
//! the extern "C" declarations yourself based on C header files.
//!
//! Advantages:
//! - Full control over types and layouts
//! - No build-time dependencies (no bindgen)
//! - Can define only what you need
//!
//! Disadvantages:
//! - Error-prone: must match C declarations exactly
//! - Maintenance burden when library updates
//! - May miss subtle ABI details

pub mod types;

pub use types::*;

use std::ffi::c_int;
use std::os::raw::c_char;

// Allow clashing declarations - this module intentionally re-declares FFI functions
// that are also declared by bindgen, to demonstrate manual FFI bindings.
#[allow(clashing_extern_declarations)]
#[link(name = "avformat")]
#[link(name = "avcodec")]
#[link(name = "avutil")]
extern "C" {
    /// Open an input stream and read the header.
    ///
    /// # Safety
    /// - `ps` must be a valid pointer to a NULL AVFormatContext pointer
    /// - `url` must be a valid null-terminated C string
    /// - `fmt` can be NULL for auto-detection
    /// - `options` can be NULL
    ///
    /// Returns 0 on success, negative AVERROR on failure.
    pub fn avformat_open_input(
        ps: *mut *mut AVFormatContext,
        url: *const c_char,
        fmt: *const std::ffi::c_void,        // AVInputFormat*
        options: *mut *mut std::ffi::c_void, // AVDictionary**
    ) -> c_int;

    /// Read packets of a media file to get stream information.
    ///
    /// # Safety
    /// - `ic` must be a valid, opened AVFormatContext pointer
    /// - `options` can be NULL
    ///
    /// Returns >= 0 on success, negative AVERROR on failure.
    pub fn avformat_find_stream_info(
        ic: *mut AVFormatContext,
        options: *mut *mut std::ffi::c_void, // AVDictionary**
    ) -> c_int;

    /// Close an opened input AVFormatContext.
    ///
    /// # Safety
    /// - `s` must be a valid pointer to an AVFormatContext pointer
    /// - After this call, *s is set to NULL
    pub fn avformat_close_input(s: *mut *mut AVFormatContext);

    /// Return the next frame of a stream.
    ///
    /// # Safety
    /// - `s` must be a valid, opened AVFormatContext pointer
    /// - `pkt` must be a valid AVPacket pointer (allocated with av_packet_alloc)
    ///
    /// Returns 0 on success, negative AVERROR on error/EOF.
    pub fn av_read_frame(s: *mut AVFormatContext, pkt: *mut AVPacket) -> c_int;

    /// Allocate an AVPacket and set its fields to default values.
    ///
    /// Returns pointer to packet, or NULL on allocation failure.
    pub fn av_packet_alloc() -> *mut AVPacket;

    /// Free the packet, if the packet is reference counted, it will be unreferenced first.
    ///
    /// # Safety
    /// - `pkt` must be a valid pointer to an AVPacket pointer
    /// - After this call, *pkt is set to NULL
    pub fn av_packet_free(pkt: *mut *mut AVPacket);

    /// Wipe the packet. Unreference the buffer and reset fields to defaults.
    ///
    /// # Safety
    /// - `pkt` must be a valid AVPacket pointer
    pub fn av_packet_unref(pkt: *mut AVPacket);

    /// Put a description of the AVERROR code errnum in errbuf.
    ///
    /// # Safety
    /// - `errbuf` must be a valid buffer of at least `errbuf_size` bytes
    ///
    /// Returns 0 on success, negative value if errnum is not found.
    pub fn av_strerror(errnum: c_int, errbuf: *mut c_char, errbuf_size: usize) -> c_int;

    /// Return the LIBAVFORMAT_VERSION_INT constant.
    pub fn avformat_version() -> c_int;

    /// Print detailed information about the input or output format.
    ///
    /// # Safety
    /// - `ic` must be a valid AVFormatContext pointer
    /// - `url` must be a valid null-terminated C string
    /// - `is_output`: 0 for input, non-zero for output
    pub fn av_dump_format(
        ic: *mut AVFormatContext,
        index: c_int,
        url: *const c_char,
        is_output: c_int,
    );
}

/// Helper function to get a Rust String from an FFmpeg error code
pub fn get_error_string(errnum: c_int) -> String {
    let mut buf = [0i8; 256];
    unsafe {
        av_strerror(errnum, buf.as_mut_ptr(), buf.len());
    }
    // Convert to Rust string, stopping at null terminator
    let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    cstr.to_string_lossy().into_owned()
}

/// Get the number of streams in a format context.
///
/// # Safety
/// - `ctx` must be a valid AVFormatContext pointer
pub unsafe fn get_nb_streams(ctx: *mut AVFormatContext) -> u32 {
    // The nb_streams field is at a known offset in AVFormatContext
    // This is fragile but demonstrates manual FFI challenges
    let ptr = ctx as *const u8;
    // nb_streams is typically at offset after several pointer fields
    // This offset may vary by FFmpeg version - use bindgen for robustness!
    let nb_streams_ptr = ptr.add(44) as *const u32;
    *nb_streams_ptr
}

/// Get a pointer to the streams array.
///
/// # Safety
/// - `ctx` must be a valid AVFormatContext pointer
pub unsafe fn get_streams(ctx: *mut AVFormatContext) -> *mut *mut AVStream {
    let ptr = ctx as *const u8;
    // streams pointer is typically at offset 48 (after nb_streams)
    let streams_ptr = ptr.add(48) as *const *mut *mut AVStream;
    *streams_ptr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = unsafe { avformat_version() };
        // Version should be non-zero
        assert!(version > 0);
        // Print version for debugging
        let major = (version >> 16) & 0xff;
        let minor = (version >> 8) & 0xff;
        let micro = version & 0xff;
        println!("libavformat version: {}.{}.{}", major, minor, micro);
    }

    #[test]
    fn test_error_string() {
        // Test with EOF error
        let msg = get_error_string(AVERROR_EOF);
        assert!(!msg.is_empty());
        println!("AVERROR_EOF message: {}", msg);
    }
}

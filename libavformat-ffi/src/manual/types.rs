//! Hand-written C type definitions for libavformat FFI.
//!
//! These are manually defined to demonstrate understanding of C struct layouts
//! and how they map to Rust. Only fields we need are included.

use std::ffi::c_int;
use std::os::raw::{c_char, c_longlong, c_uint, c_void};

/// Opaque format context - we only use it as a pointer
#[repr(C)]
pub struct AVFormatContext {
    _opaque: [u8; 0],
}

/// AVRational represents a rational number (numerator/denominator)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AVRational {
    pub num: c_int,
    pub den: c_int,
}

/// Media type enumeration
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AVMediaType {
    Unknown = -1,
    Video = 0,
    Audio = 1,
    Data = 2,
    Subtitle = 3,
    Attachment = 4,
    Nb = 5,
}

impl Default for AVMediaType {
    fn default() -> Self {
        AVMediaType::Unknown
    }
}

/// Codec parameters - partial definition with commonly accessed fields
#[repr(C)]
pub struct AVCodecParameters {
    pub codec_type: AVMediaType,
    pub codec_id: c_uint,
    pub codec_tag: c_uint,
    pub extradata: *mut u8,
    pub extradata_size: c_int,
    pub format: c_int,
    pub bit_rate: c_longlong,
    pub bits_per_coded_sample: c_int,
    pub bits_per_raw_sample: c_int,
    pub profile: c_int,
    pub level: c_int,
    pub width: c_int,
    pub height: c_int,
    // ... more fields exist but we don't need them
    _padding: [u8; 256], // Padding to ensure we don't undersize the struct
}

/// AVStream - partial definition
/// Note: The actual struct is much larger; we define enough to read basic info
#[repr(C)]
pub struct AVStream {
    pub index: c_int,
    pub id: c_int,
    _priv_data: *mut c_void,
    pub time_base: AVRational,
    pub start_time: c_longlong,
    pub duration: c_longlong,
    pub nb_frames: c_longlong,
    _disposition: c_int,
    _discard: c_int,
    pub sample_aspect_ratio: AVRational,
    _metadata: *mut c_void,
    pub avg_frame_rate: AVRational,
    _attached_pic: [u8; 128], // AVPacket embedded struct
    _side_data: *mut c_void,
    _nb_side_data: c_int,
    _event_flags: c_int,
    pub r_frame_rate: AVRational,
    pub codecpar: *mut AVCodecParameters,
    // ... more fields exist
}

/// AVPacket - packet for compressed data
/// We define the commonly accessed fields
#[repr(C)]
pub struct AVPacket {
    pub buf: *mut c_void, // AVBufferRef*
    pub pts: c_longlong,
    pub dts: c_longlong,
    pub data: *mut u8,
    pub size: c_int,
    pub stream_index: c_int,
    pub flags: c_int,
    pub side_data: *mut c_void,
    pub side_data_elems: c_int,
    pub duration: c_longlong,
    pub pos: c_longlong,
    // Opaque fields for internal use
    _opaque: *mut c_void,
    _opaque_ref: *mut c_void,
    pub time_base: AVRational,
}

/// Special value indicating no timestamp
pub const AV_NOPTS_VALUE: c_longlong = 0x8000000000000000u64 as c_longlong;

/// Error codes (AVERROR values are typically negative)
pub const AVERROR_EOF: c_int =
    -(('E' as c_int) | (('O' as c_int) << 8) | (('F' as c_int) << 16) | ((' ' as c_int) << 24));

//! Safe wrapper for AVFormatContext.

use crate::bindgen;
use crate::safe::error::{check_error, AvError, Result};
use crate::safe::packet::Packet;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;

/// Media type enumeration (mirrors AVMediaType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Unknown,
    Video,
    Audio,
    Data,
    Subtitle,
    Attachment,
}

impl From<bindgen::AVMediaType> for MediaType {
    fn from(t: bindgen::AVMediaType) -> Self {
        match t {
            bindgen::AVMediaType::AVMEDIA_TYPE_VIDEO => MediaType::Video,
            bindgen::AVMediaType::AVMEDIA_TYPE_AUDIO => MediaType::Audio,
            bindgen::AVMediaType::AVMEDIA_TYPE_DATA => MediaType::Data,
            bindgen::AVMediaType::AVMEDIA_TYPE_SUBTITLE => MediaType::Subtitle,
            bindgen::AVMediaType::AVMEDIA_TYPE_ATTACHMENT => MediaType::Attachment,
            _ => MediaType::Unknown,
        }
    }
}

/// Information about a stream in the container.
#[derive(Debug, Clone)]
pub struct StreamInfo {
    /// Stream index within the container
    pub index: usize,
    /// Media type (video, audio, etc.)
    pub media_type: MediaType,
    /// Codec ID
    pub codec_id: u32,
    /// Bitrate in bits/second (may be 0 if unknown)
    pub bit_rate: i64,
    /// For video: width in pixels
    pub width: i32,
    /// For video: height in pixels
    pub height: i32,
    /// Duration in stream time base units
    pub duration: i64,
    /// Number of frames (may be 0 if unknown)
    pub nb_frames: i64,
    /// Time base numerator
    pub time_base_num: i32,
    /// Time base denominator
    pub time_base_den: i32,
}

impl StreamInfo {
    /// Get the duration in seconds (if known).
    pub fn duration_secs(&self) -> Option<f64> {
        if self.duration <= 0 || self.time_base_den == 0 {
            None
        } else {
            Some(self.duration as f64 * self.time_base_num as f64 / self.time_base_den as f64)
        }
    }
}

/// Safe wrapper around AVFormatContext for reading media containers.
///
/// This struct handles opening/closing the format context automatically.
/// Use `open()` to create an instance and read packets with `read_packet()`.
pub struct FormatContext {
    ptr: *mut bindgen::AVFormatContext,
}

impl FormatContext {
    /// Open a media file for reading.
    ///
    /// # Arguments
    /// * `path` - Path to the media file
    ///
    /// # Returns
    /// A `FormatContext` on success, or an error if the file couldn't be opened.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_str = path
            .as_ref()
            .to_str()
            .ok_or_else(|| AvError::InvalidArg("Path contains invalid UTF-8".into()))?;

        let c_path = CString::new(path_str)
            .map_err(|_| AvError::InvalidArg("Path contains null byte".into()))?;

        let mut ctx: *mut bindgen::AVFormatContext = ptr::null_mut();

        // Open the input file
        let ret = unsafe {
            bindgen::avformat_open_input(&mut ctx, c_path.as_ptr(), ptr::null(), ptr::null_mut())
        };

        if ret < 0 {
            return Err(AvError::OpenInput(bindgen::get_error_string(ret)));
        }

        // Find stream information
        let ret = unsafe { bindgen::avformat_find_stream_info(ctx, ptr::null_mut()) };
        if ret < 0 {
            // Clean up on error
            unsafe {
                bindgen::avformat_close_input(&mut ctx);
            }
            return Err(AvError::StreamInfo(bindgen::get_error_string(ret)));
        }

        Ok(FormatContext { ptr: ctx })
    }

    /// Get the number of streams in this container.
    pub fn nb_streams(&self) -> usize {
        unsafe { (*self.ptr).nb_streams as usize }
    }

    /// Get information about all streams.
    pub fn streams(&self) -> Vec<StreamInfo> {
        let nb = self.nb_streams();
        let mut result = Vec::with_capacity(nb);

        for i in 0..nb {
            if let Some(info) = self.stream_info(i) {
                result.push(info);
            }
        }

        result
    }

    /// Get information about a specific stream.
    pub fn stream_info(&self, index: usize) -> Option<StreamInfo> {
        if index >= self.nb_streams() {
            return None;
        }

        unsafe {
            let streams = (*self.ptr).streams;
            let stream = *streams.add(index);
            let codecpar = (*stream).codecpar;

            Some(StreamInfo {
                index,
                media_type: MediaType::from((*codecpar).codec_type),
                codec_id: (*codecpar).codec_id,
                bit_rate: (*codecpar).bit_rate,
                width: (*codecpar).width,
                height: (*codecpar).height,
                duration: (*stream).duration,
                nb_frames: (*stream).nb_frames,
                time_base_num: (*stream).time_base.num,
                time_base_den: (*stream).time_base.den,
            })
        }
    }

    /// Get the container duration in microseconds, or None if unknown.
    pub fn duration(&self) -> Option<i64> {
        let dur = unsafe { (*self.ptr).duration };
        if dur <= 0 {
            None
        } else {
            Some(dur)
        }
    }

    /// Get the container duration in seconds, or None if unknown.
    pub fn duration_secs(&self) -> Option<f64> {
        // AV_TIME_BASE is 1000000
        self.duration().map(|d| d as f64 / 1_000_000.0)
    }

    /// Get the container format name.
    pub fn format_name(&self) -> Option<String> {
        unsafe {
            let iformat = (*self.ptr).iformat;
            if iformat.is_null() {
                return None;
            }
            let name = (*iformat).name;
            if name.is_null() {
                return None;
            }
            Some(CStr::from_ptr(name).to_string_lossy().into_owned())
        }
    }

    /// Read the next packet from the container.
    ///
    /// # Arguments
    /// * `packet` - A packet to receive the data
    ///
    /// # Returns
    /// `Ok(true)` if a packet was read, `Ok(false)` if EOF, or an error.
    pub fn read_packet(&mut self, packet: &mut Packet) -> Result<bool> {
        // Clear previous packet data
        packet.unref();

        let ret = unsafe { bindgen::av_read_frame(self.ptr, packet.as_mut_ptr()) };

        if ret >= 0 {
            Ok(true)
        } else {
            // Check for EOF
            let eof_code =
                -('E' as i32 | ('O' as i32) << 8 | ('F' as i32) << 16 | (' ' as i32) << 24);
            if ret == eof_code {
                Ok(false)
            } else {
                Err(AvError::ReadFrame(bindgen::get_error_string(ret)))
            }
        }
    }

    /// Dump format information to stderr (for debugging).
    pub fn dump_format(&self) {
        unsafe {
            // Create a dummy filename for display
            let filename = CString::new("<input>").unwrap();
            bindgen::av_dump_format(self.ptr, 0, filename.as_ptr(), 0);
        }
    }

    /// Get the raw pointer (for advanced FFI usage).
    ///
    /// # Safety
    /// The returned pointer is valid only for the lifetime of this FormatContext.
    pub unsafe fn as_ptr(&self) -> *mut bindgen::AVFormatContext {
        self.ptr
    }
}

impl Drop for FormatContext {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                bindgen::avformat_close_input(&mut self.ptr);
            }
        }
    }
}

// FormatContext is not Send/Sync by default due to raw pointer
// This is intentional for safety

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_nonexistent() {
        let result = FormatContext::open("/nonexistent/file.mp4");
        assert!(result.is_err());
    }

    #[test]
    fn test_media_type_conversion() {
        assert_eq!(
            MediaType::from(bindgen::AVMediaType::AVMEDIA_TYPE_VIDEO),
            MediaType::Video
        );
        assert_eq!(
            MediaType::from(bindgen::AVMediaType::AVMEDIA_TYPE_AUDIO),
            MediaType::Audio
        );
    }
}

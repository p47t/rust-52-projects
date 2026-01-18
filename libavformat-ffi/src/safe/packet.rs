//! Safe wrapper for AVPacket.

use crate::bindgen;
use crate::safe::error::{AvError, Result};

/// Safe wrapper around AVPacket.
///
/// Handles allocation and deallocation automatically via RAII.
/// The packet can be reused by calling `unref()` between reads.
pub struct Packet {
    ptr: *mut bindgen::AVPacket,
}

impl Packet {
    /// Allocate a new packet.
    pub fn new() -> Result<Self> {
        let ptr = unsafe { bindgen::av_packet_alloc() };
        if ptr.is_null() {
            return Err(AvError::Alloc);
        }
        Ok(Packet { ptr })
    }

    /// Get the raw pointer (for FFI calls).
    pub fn as_mut_ptr(&mut self) -> *mut bindgen::AVPacket {
        self.ptr
    }

    /// Unreference the packet, resetting it for reuse.
    pub fn unref(&mut self) {
        unsafe {
            bindgen::av_packet_unref(self.ptr);
        }
    }

    /// Get the presentation timestamp.
    pub fn pts(&self) -> i64 {
        unsafe { (*self.ptr).pts }
    }

    /// Get the decompression timestamp.
    pub fn dts(&self) -> i64 {
        unsafe { (*self.ptr).dts }
    }

    /// Get the stream index this packet belongs to.
    pub fn stream_index(&self) -> i32 {
        unsafe { (*self.ptr).stream_index }
    }

    /// Get the packet data size in bytes.
    pub fn size(&self) -> i32 {
        unsafe { (*self.ptr).size }
    }

    /// Get the packet duration.
    pub fn duration(&self) -> i64 {
        unsafe { (*self.ptr).duration }
    }

    /// Get the byte position in the stream, or -1 if unknown.
    pub fn pos(&self) -> i64 {
        unsafe { (*self.ptr).pos }
    }

    /// Get the packet flags.
    pub fn flags(&self) -> i32 {
        unsafe { (*self.ptr).flags }
    }

    /// Check if this is a keyframe.
    pub fn is_keyframe(&self) -> bool {
        // AV_PKT_FLAG_KEY = 0x0001
        self.flags() & 0x0001 != 0
    }

    /// Get the packet data as a byte slice.
    ///
    /// Returns None if the packet has no data.
    pub fn data(&self) -> Option<&[u8]> {
        unsafe {
            let data = (*self.ptr).data;
            let size = (*self.ptr).size;
            if data.is_null() || size <= 0 {
                None
            } else {
                Some(std::slice::from_raw_parts(data, size as usize))
            }
        }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                bindgen::av_packet_free(&mut self.ptr);
            }
        }
    }
}

// Packet is not Send/Sync by default due to raw pointer
// This is intentional for safety

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_alloc() {
        let packet = Packet::new();
        assert!(packet.is_ok());
    }

    #[test]
    fn test_packet_initial_state() {
        let packet = Packet::new().unwrap();
        assert_eq!(packet.size(), 0);
        assert!(packet.data().is_none());
    }

    #[test]
    fn test_packet_unref() {
        let mut packet = Packet::new().unwrap();
        packet.unref(); // Should not panic
        assert_eq!(packet.size(), 0);
    }
}

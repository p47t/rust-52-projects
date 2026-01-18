//! FFI bindings for libavformat demonstrating three approaches.
//!
//! This crate provides bindings to FFmpeg's libavformat library using three
//! different FFI approaches:
//!
//! 1. **Manual FFI** (`manual` module) - Hand-written extern "C" declarations
//! 2. **Bindgen FFI** (`bindgen` module) - Auto-generated bindings via bindgen
//! 3. **Safe Wrapper** (`safe` module) - Idiomatic Rust API on top of bindgen
//!
//! # Prerequisites
//!
//! You need FFmpeg development libraries installed:
//!
//! ```sh
//! # Ubuntu/Debian
//! sudo apt install libavformat-dev libavcodec-dev libavutil-dev
//!
//! # Fedora
//! sudo dnf install ffmpeg-devel
//!
//! # macOS
//! brew install ffmpeg
//! ```
//!
//! # Choosing an Approach
//!
//! - Use **manual** when you need fine-grained control or minimal dependencies
//! - Use **bindgen** when you need complete, accurate bindings
//! - Use **safe** for most applications - it prevents common FFI errors
//!
//! # Example
//!
//! ```no_run
//! use libavformat_ffi::safe::{FormatContext, Packet};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Open a media file
//!     let mut ctx = FormatContext::open("video.mp4")?;
//!
//!     // Print stream information
//!     for stream in ctx.streams() {
//!         println!("Stream {}: {:?}", stream.index, stream.media_type);
//!     }
//!
//!     // Read packets
//!     let mut packet = Packet::new()?;
//!     let mut count = 0;
//!     while ctx.read_packet(&mut packet)? && count < 10 {
//!         println!("Packet: stream={}, size={}", packet.stream_index(), packet.size());
//!         count += 1;
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod bindgen;
pub mod manual;
pub mod safe;

// Re-export the safe API at the crate root for convenience
pub use safe::{AvError, FormatContext, MediaType, Packet, Result, StreamInfo};

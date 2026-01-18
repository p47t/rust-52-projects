//! Safe, idiomatic Rust wrapper for libavformat.
//!
//! This module provides a high-level, safe API built on top of the raw
//! bindgen bindings. It uses RAII for resource management and Rust's
//! type system to prevent common FFI errors.
//!
//! # Example
//!
//! ```no_run
//! use libavformat_ffi::safe::{FormatContext, Packet};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut ctx = FormatContext::open("video.mp4")?;
//!     let mut packet = Packet::new()?;
//!
//!     println!("Format: {:?}", ctx.format_name());
//!     println!("Streams: {}", ctx.nb_streams());
//!
//!     while ctx.read_packet(&mut packet)? {
//!         println!("Packet: stream={}, size={}", packet.stream_index(), packet.size());
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod format_context;
pub mod packet;

pub use error::{AvError, Result};
pub use format_context::{FormatContext, MediaType, StreamInfo};
pub use packet::Packet;

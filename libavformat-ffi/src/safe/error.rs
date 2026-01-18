//! Error types for the safe FFI wrapper.

use crate::bindgen;
use std::ffi::c_int;
use thiserror::Error;

/// FFmpeg-specific error type
#[derive(Error, Debug)]
pub enum AvError {
    /// End of file reached
    #[error("End of file")]
    Eof,

    /// Could not open the input file
    #[error("Failed to open input: {0}")]
    OpenInput(String),

    /// Could not find stream information
    #[error("Failed to find stream info: {0}")]
    StreamInfo(String),

    /// Error reading a frame/packet
    #[error("Failed to read frame: {0}")]
    ReadFrame(String),

    /// Memory allocation failure
    #[error("Memory allocation failed")]
    Alloc,

    /// Invalid argument provided
    #[error("Invalid argument: {0}")]
    InvalidArg(String),

    /// Generic FFmpeg error with code and message
    #[error("FFmpeg error ({code}): {message}")]
    Ffmpeg { code: c_int, message: String },
}

impl AvError {
    /// Create an AvError from an FFmpeg error code
    pub fn from_code(code: c_int) -> Self {
        // Check for EOF specifically
        let eof_code =
            -('E' as c_int | ('O' as c_int) << 8 | ('F' as c_int) << 16 | (' ' as c_int) << 24);
        if code == eof_code {
            return AvError::Eof;
        }

        let message = bindgen::get_error_string(code);
        AvError::Ffmpeg { code, message }
    }
}

/// Result type alias for operations that may fail with AvError
pub type Result<T> = std::result::Result<T, AvError>;

/// Convert an FFmpeg return code to a Result
pub fn check_error(code: c_int) -> Result<()> {
    if code >= 0 {
        Ok(())
    } else {
        Err(AvError::from_code(code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AvError::Eof;
        assert_eq!(format!("{}", err), "End of file");

        let err = AvError::OpenInput("file not found".into());
        assert!(format!("{}", err).contains("file not found"));
    }

    #[test]
    fn test_from_code() {
        // Test with a negative code
        let err = AvError::from_code(-1);
        if let AvError::Ffmpeg { code, .. } = err {
            assert_eq!(code, -1);
        }
    }
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdbError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ADB server returned FAIL: {0}")]
    ServerFail(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Invalid response: expected {expected}, got {actual}")]
    InvalidResponse { expected: String, actual: String },

    #[error("No device connected")]
    NoDevice,

    #[error("Multiple devices connected; specify a serial number")]
    MultipleDevices,

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Connection refused: is the ADB server running? (try 'adb start-server')")]
    ConnectionRefused,

    #[error("Sync protocol error: {0}")]
    SyncError(String),

    #[error("File not found: {0}")]
    FileNotFound(String),
}

pub type AdbResult<T> = Result<T, AdbError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AdbError::ServerFail("device not found".into());
        assert_eq!(
            err.to_string(),
            "ADB server returned FAIL: device not found"
        );

        let err = AdbError::ConnectionRefused;
        assert!(err.to_string().contains("ADB server running"));
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken");
        let adb_err: AdbError = io_err.into();
        assert!(matches!(adb_err, AdbError::Io(_)));
        assert!(adb_err.to_string().contains("broken"));
    }
}

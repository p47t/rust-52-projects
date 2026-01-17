use crate::error::{AdbError, AdbResult};

// ADB server protocol uses a simple length-prefixed format:
//
// Request:  {4-digit hex length}{payload}
// Response: OKAY{4-digit hex length}{data}
//       or: FAIL{4-digit hex length}{error_message}

/// Format a request for the ADB server.
///
/// Encodes as `"{:04X}{payload}"` where the hex length is the payload byte length.
pub fn encode_request(payload: &str) -> Vec<u8> {
    format!("{:04X}{}", payload.len(), payload).into_bytes()
}

/// The two possible response statuses from the ADB server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdbStatus {
    Okay,
    Fail,
}

/// Parse a 4-byte status prefix (`OKAY` or `FAIL`) from a byte slice.
pub fn parse_status(buf: &[u8]) -> AdbResult<AdbStatus> {
    if buf.len() < 4 {
        return Err(AdbError::Protocol(format!(
            "Status too short: {} bytes, need 4",
            buf.len()
        )));
    }
    match &buf[..4] {
        b"OKAY" => Ok(AdbStatus::Okay),
        b"FAIL" => Ok(AdbStatus::Fail),
        other => Err(AdbError::InvalidResponse {
            expected: "OKAY or FAIL".into(),
            actual: String::from_utf8_lossy(other).to_string(),
        }),
    }
}

/// Parse a 4-character hex length string into a `usize`.
pub fn parse_hex_length(buf: &[u8]) -> AdbResult<usize> {
    if buf.len() < 4 {
        return Err(AdbError::Protocol(format!(
            "Hex length too short: {} bytes, need 4",
            buf.len()
        )));
    }
    let hex_str = std::str::from_utf8(&buf[..4])
        .map_err(|_| AdbError::Protocol(format!("Invalid UTF-8 in hex length: {:?}", &buf[..4])))?;
    usize::from_str_radix(hex_str, 16)
        .map_err(|_| AdbError::Protocol(format!("Invalid hex length: {:?}", hex_str)))
}

/// Known ADB host service commands (handled by the ADB server itself).
#[derive(Debug, Clone)]
pub enum HostCommand {
    /// Get ADB server protocol version.
    Version,
    /// List connected devices in short format.
    Devices,
    /// List connected devices with extended info.
    DevicesLong,
    /// Stream device connect/disconnect events.
    TrackDevices,
    /// Switch to a specific device by serial number.
    Transport(String),
    /// Switch to any available device.
    TransportAny,
    /// Kill the ADB server.
    Kill,
}

impl HostCommand {
    /// Convert the command to its wire format string.
    pub fn to_wire(&self) -> String {
        match self {
            HostCommand::Version => "host:version".to_string(),
            HostCommand::Devices => "host:devices".to_string(),
            HostCommand::DevicesLong => "host:devices-l".to_string(),
            HostCommand::TrackDevices => "host:track-devices".to_string(),
            HostCommand::Transport(serial) => format!("host:transport:{}", serial),
            HostCommand::TransportAny => "host:transport-any".to_string(),
            HostCommand::Kill => "host:kill".to_string(),
        }
    }

    /// Encode the command as a full request (with length prefix).
    pub fn encode(&self) -> Vec<u8> {
        encode_request(&self.to_wire())
    }
}

/// Local service commands (forwarded to device after transport selection).
#[derive(Debug, Clone)]
pub enum LocalCommand {
    /// Execute a shell command on the device.
    Shell(String),
    /// Open an interactive shell session.
    ShellInteractive,
    /// Stream logcat output.
    Logcat,
    /// Enter file sync mode.
    Sync,
}

impl LocalCommand {
    /// Convert the command to its wire format string.
    pub fn to_wire(&self) -> String {
        match self {
            LocalCommand::Shell(cmd) => format!("shell:{}", cmd),
            LocalCommand::ShellInteractive => "shell:".to_string(),
            LocalCommand::Logcat => "shell:logcat".to_string(),
            LocalCommand::Sync => "sync:".to_string(),
        }
    }

    /// Encode the command as a full request (with length prefix).
    pub fn encode(&self) -> Vec<u8> {
        encode_request(&self.to_wire())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_request() {
        assert_eq!(encode_request("host:version"), b"000Chost:version");
        assert_eq!(encode_request("host:devices"), b"000Chost:devices");
        assert_eq!(encode_request("shell:ls"), b"0008shell:ls");
        assert_eq!(encode_request("sync:"), b"0005sync:");
    }

    #[test]
    fn test_encode_empty() {
        assert_eq!(encode_request(""), b"0000");
    }

    #[test]
    fn test_parse_status_okay() {
        assert_eq!(parse_status(b"OKAY").unwrap(), AdbStatus::Okay);
    }

    #[test]
    fn test_parse_status_fail() {
        assert_eq!(parse_status(b"FAIL").unwrap(), AdbStatus::Fail);
    }

    #[test]
    fn test_parse_status_unknown() {
        assert!(parse_status(b"WHAT").is_err());
    }

    #[test]
    fn test_parse_status_too_short() {
        assert!(parse_status(b"OK").is_err());
    }

    #[test]
    fn test_parse_hex_length() {
        assert_eq!(parse_hex_length(b"000C").unwrap(), 12);
        assert_eq!(parse_hex_length(b"0000").unwrap(), 0);
        assert_eq!(parse_hex_length(b"FFFF").unwrap(), 65535);
        assert_eq!(parse_hex_length(b"001A").unwrap(), 26);
    }

    #[test]
    fn test_parse_hex_length_invalid() {
        assert!(parse_hex_length(b"ZZZZ").is_err());
    }

    #[test]
    fn test_parse_hex_length_too_short() {
        assert!(parse_hex_length(b"00").is_err());
    }

    #[test]
    fn test_host_command_wire_format() {
        assert_eq!(HostCommand::Version.to_wire(), "host:version");
        assert_eq!(HostCommand::Devices.to_wire(), "host:devices");
        assert_eq!(HostCommand::DevicesLong.to_wire(), "host:devices-l");
        assert_eq!(HostCommand::TrackDevices.to_wire(), "host:track-devices");
        assert_eq!(
            HostCommand::Transport("emulator-5554".into()).to_wire(),
            "host:transport:emulator-5554"
        );
        assert_eq!(HostCommand::TransportAny.to_wire(), "host:transport-any");
        assert_eq!(HostCommand::Kill.to_wire(), "host:kill");
    }

    #[test]
    fn test_host_command_encode_round_trip() {
        let cmd = HostCommand::Version;
        let encoded = cmd.encode();
        // "000Chost:version" -> length=12, payload="host:version"
        let (len_bytes, payload) = encoded.split_at(4);
        let len = parse_hex_length(len_bytes).unwrap();
        assert_eq!(len, payload.len());
        assert_eq!(payload, b"host:version");
    }

    #[test]
    fn test_local_command_wire_format() {
        assert_eq!(
            LocalCommand::Shell("ls -la".into()).to_wire(),
            "shell:ls -la"
        );
        assert_eq!(LocalCommand::ShellInteractive.to_wire(), "shell:");
        assert_eq!(LocalCommand::Logcat.to_wire(), "shell:logcat");
        assert_eq!(LocalCommand::Sync.to_wire(), "sync:");
    }

    #[test]
    fn test_local_command_encode() {
        let cmd = LocalCommand::Shell("echo hello".into());
        let encoded = cmd.encode();
        let (len_bytes, payload) = encoded.split_at(4);
        let len = parse_hex_length(len_bytes).unwrap();
        assert_eq!(len, payload.len());
        assert_eq!(payload, b"shell:echo hello");
    }
}

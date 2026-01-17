use crate::device::DeviceInfo;
use crate::error::{AdbError, AdbResult};
use crate::protocol::{self, AdbStatus, HostCommand, LocalCommand};
use crate::sync::{self, DentEntry, StatResponse, SyncHeader, SyncId, SYNC_DATA_MAX};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::debug;

/// Client for communicating with the ADB server over TCP.
///
/// Each command opens a fresh TCP connection to the ADB server — this matches
/// the real ADB client behavior where connections are one-shot.
pub struct AdbClient {
    host: String,
    port: u16,
}

impl AdbClient {
    /// Create a client connecting to the default ADB server at `127.0.0.1:5037`.
    pub fn new() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 5037,
        }
    }

    /// Create a client connecting to a specific address (useful for testing).
    pub fn with_address(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
        }
    }

    /// Open a new TCP connection to the ADB server.
    async fn connect(&self) -> AdbResult<TcpStream> {
        let addr = format!("{}:{}", self.host, self.port);
        debug!("Connecting to ADB server at {}", addr);
        TcpStream::connect(&addr).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::ConnectionRefused {
                AdbError::ConnectionRefused
            } else {
                AdbError::Io(e)
            }
        })
    }

    /// Send a request and read the OKAY/FAIL status response.
    async fn send_command(stream: &mut TcpStream, command: &[u8]) -> AdbResult<()> {
        stream.write_all(command).await?;
        let mut status_buf = [0u8; 4];
        stream.read_exact(&mut status_buf).await?;
        match protocol::parse_status(&status_buf)? {
            AdbStatus::Okay => Ok(()),
            AdbStatus::Fail => {
                let error_msg = Self::read_length_prefixed(stream).await?;
                Err(AdbError::ServerFail(
                    String::from_utf8_lossy(&error_msg).to_string(),
                ))
            }
        }
    }

    /// Read a length-prefixed response body (4-hex-digit length + data).
    async fn read_length_prefixed(stream: &mut TcpStream) -> AdbResult<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let len = protocol::parse_hex_length(&len_buf)?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;
        Ok(data)
    }

    /// Read all remaining data from the stream until EOF.
    async fn read_to_end(stream: &mut TcpStream) -> AdbResult<Vec<u8>> {
        let mut data = Vec::new();
        stream.read_to_end(&mut data).await?;
        Ok(data)
    }

    /// Read an 8-byte sync header from the stream.
    async fn read_sync_header(stream: &mut TcpStream) -> AdbResult<SyncHeader> {
        let mut buf = [0u8; 8];
        stream.read_exact(&mut buf).await?;
        SyncHeader::from_bytes(&buf)
    }

    // --- Transport helpers ---

    /// Select a device transport, then execute a local service command.
    /// Returns the stream positioned after the OKAY response.
    async fn with_transport(
        &self,
        serial: Option<&str>,
        command: &LocalCommand,
    ) -> AdbResult<TcpStream> {
        let mut stream = self.connect().await?;

        // Step 1: Select device transport
        let transport_cmd = match serial {
            Some(s) => HostCommand::Transport(s.to_string()),
            None => HostCommand::TransportAny,
        };
        debug!("Selecting transport: {:?}", transport_cmd.to_wire());
        Self::send_command(&mut stream, &transport_cmd.encode()).await?;

        // Step 2: Send the local service command
        debug!("Sending local command: {:?}", command.to_wire());
        Self::send_command(&mut stream, &command.encode()).await?;

        Ok(stream)
    }

    /// Enter sync mode on a transport, returning the stream ready for sync commands.
    async fn enter_sync(&self, serial: Option<&str>) -> AdbResult<TcpStream> {
        self.with_transport(serial, &LocalCommand::Sync).await
    }

    // --- Public API ---

    /// Get ADB server protocol version.
    pub async fn server_version(&self) -> AdbResult<u32> {
        let mut stream = self.connect().await?;
        Self::send_command(&mut stream, &HostCommand::Version.encode()).await?;
        let data = Self::read_length_prefixed(&mut stream).await?;
        let hex_str = std::str::from_utf8(&data)
            .map_err(|_| AdbError::Protocol("Invalid UTF-8 in version response".into()))?;
        u32::from_str_radix(hex_str, 16)
            .map_err(|_| AdbError::Protocol(format!("Invalid version hex: {:?}", hex_str)))
    }

    /// List connected devices.
    pub async fn list_devices(&self) -> AdbResult<Vec<DeviceInfo>> {
        let mut stream = self.connect().await?;
        Self::send_command(&mut stream, &HostCommand::Devices.encode()).await?;
        let data = Self::read_length_prefixed(&mut stream).await?;
        let text = String::from_utf8_lossy(&data);
        Ok(DeviceInfo::parse_device_list(&text))
    }

    /// Execute a shell command on the device and return its output.
    pub async fn shell(&self, serial: Option<&str>, command: &str) -> AdbResult<String> {
        let mut stream = self
            .with_transport(serial, &LocalCommand::Shell(command.to_string()))
            .await?;
        let data = Self::read_to_end(&mut stream).await?;
        Ok(String::from_utf8_lossy(&data).to_string())
    }

    /// Stream logcat output. Returns the TCP stream for the caller to read from.
    pub async fn logcat(&self, serial: Option<&str>) -> AdbResult<TcpStream> {
        self.with_transport(serial, &LocalCommand::Logcat).await
    }

    /// Stat a remote file on the device.
    pub async fn stat(&self, serial: Option<&str>, remote_path: &str) -> AdbResult<StatResponse> {
        let mut stream = self.enter_sync(serial).await?;

        // Send STAT request
        let req = sync::encode_stat_request(remote_path);
        stream.write_all(&req).await?;

        // Read STAT response: "STAT" + mode(4) + size(4) + mtime(4)
        let mut buf = [0u8; 16];
        stream.read_exact(&mut buf).await?;

        let id = SyncId::from_bytes(&buf[0..4])?;
        if id == SyncId::Fail {
            let len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
            let mut msg = vec![0u8; len as usize];
            stream.read_exact(&mut msg).await?;
            return Err(AdbError::SyncError(
                String::from_utf8_lossy(&msg).to_string(),
            ));
        }
        if id != SyncId::Stat {
            return Err(AdbError::Protocol(format!(
                "Expected STAT response, got {:?}",
                id
            )));
        }

        let stat = StatResponse::from_bytes(&buf[4..16])?;

        // Send QUIT
        stream.write_all(&sync::encode_quit()).await?;

        Ok(stat)
    }

    /// List a remote directory on the device.
    pub async fn list_dir(
        &self,
        serial: Option<&str>,
        remote_path: &str,
    ) -> AdbResult<Vec<DentEntry>> {
        let mut stream = self.enter_sync(serial).await?;

        // Send LIST request
        let req = sync::encode_list_request(remote_path);
        stream.write_all(&req).await?;

        let mut entries = Vec::new();

        loop {
            let header = Self::read_sync_header(&mut stream).await?;

            match header.id {
                SyncId::Dent => {
                    // Read the DENT payload: mode(4) + size(4) + mtime(4) + namelen(4) + name
                    let mut payload = vec![0u8; header.length as usize];
                    stream.read_exact(&mut payload).await?;
                    // The DENT header.length covers: mode + size + mtime + namelen + name
                    // But the on-wire format has mode/size/mtime/namelen as the first 16 bytes
                    // of the payload following the DENT id+length header
                    let entry = DentEntry::from_bytes(&payload)?;
                    entries.push(entry);
                }
                SyncId::Done => {
                    break;
                }
                SyncId::Fail => {
                    let mut msg = vec![0u8; header.length as usize];
                    stream.read_exact(&mut msg).await?;
                    return Err(AdbError::SyncError(
                        String::from_utf8_lossy(&msg).to_string(),
                    ));
                }
                other => {
                    return Err(AdbError::Protocol(format!(
                        "Unexpected sync ID in LIST response: {:?}",
                        other
                    )));
                }
            }
        }

        // Send QUIT
        stream.write_all(&sync::encode_quit()).await?;

        Ok(entries)
    }

    /// Push a local file to the device.
    pub async fn push(
        &self,
        serial: Option<&str>,
        local_path: &Path,
        remote_path: &str,
    ) -> AdbResult<()> {
        let file_data = tokio::fs::read(local_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AdbError::FileNotFound(local_path.display().to_string())
            } else {
                AdbError::Io(e)
            }
        })?;

        let metadata = tokio::fs::metadata(local_path).await?;
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0);

        let mut stream = self.enter_sync(serial).await?;

        // Send SEND request with file mode 0644
        let req = sync::encode_send_request(remote_path, 0o644);
        stream.write_all(&req).await?;

        // Send file data in chunks
        for chunk in file_data.chunks(SYNC_DATA_MAX as usize) {
            let data_msg = sync::encode_data_chunk(chunk);
            stream.write_all(&data_msg).await?;
        }

        // Send DONE with mtime
        stream.write_all(&sync::encode_done(mtime)).await?;

        // Read response (OKAY or FAIL)
        let header = Self::read_sync_header(&mut stream).await?;
        match header.id {
            SyncId::Okay => {}
            SyncId::Fail => {
                let mut msg = vec![0u8; header.length as usize];
                stream.read_exact(&mut msg).await?;
                return Err(AdbError::SyncError(
                    String::from_utf8_lossy(&msg).to_string(),
                ));
            }
            other => {
                return Err(AdbError::Protocol(format!(
                    "Expected OKAY after push, got {:?}",
                    other
                )));
            }
        }

        // Send QUIT
        stream.write_all(&sync::encode_quit()).await?;

        Ok(())
    }

    /// Pull a remote file from the device to a local path.
    pub async fn pull(
        &self,
        serial: Option<&str>,
        remote_path: &str,
        local_path: &Path,
    ) -> AdbResult<()> {
        let mut stream = self.enter_sync(serial).await?;

        // Send RECV request
        let req = sync::encode_recv_request(remote_path);
        stream.write_all(&req).await?;

        // Read DATA chunks until DONE
        let mut file_data = Vec::new();

        loop {
            let header = Self::read_sync_header(&mut stream).await?;

            match header.id {
                SyncId::Data => {
                    let mut chunk = vec![0u8; header.length as usize];
                    stream.read_exact(&mut chunk).await?;
                    file_data.extend_from_slice(&chunk);
                }
                SyncId::Done => {
                    break;
                }
                SyncId::Fail => {
                    let mut msg = vec![0u8; header.length as usize];
                    stream.read_exact(&mut msg).await?;
                    return Err(AdbError::SyncError(
                        String::from_utf8_lossy(&msg).to_string(),
                    ));
                }
                other => {
                    return Err(AdbError::Protocol(format!(
                        "Expected DATA/DONE in pull, got {:?}",
                        other
                    )));
                }
            }
        }

        // Write to local file
        tokio::fs::write(local_path, &file_data).await?;

        // Send QUIT
        stream.write_all(&sync::encode_quit()).await?;

        Ok(())
    }
}

impl Default for AdbClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    /// Spawn a mock ADB server that sends canned responses.
    /// Returns the port it's listening on.
    async fn mock_adb_server(handler: impl FnOnce(TcpStream) + Send + 'static) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            handler(socket);
        });
        port
    }

    /// Spawn a mock that reads the request then sends a byte sequence.
    async fn mock_simple_response(response: Vec<u8>) -> u16 {
        mock_adb_server(move |mut socket| {
            tokio::spawn(async move {
                // Read and discard the request
                let mut buf = [0u8; 256];
                let _ = socket.read(&mut buf).await;
                // Send the response
                socket.write_all(&response).await.unwrap();
            });
        })
        .await
    }

    #[tokio::test]
    async fn test_server_version() {
        // Mock server responds: OKAY + "0004" + "001f" (version 31)
        let mut response = Vec::new();
        response.extend_from_slice(b"OKAY");
        response.extend_from_slice(b"0004");
        response.extend_from_slice(b"001f");
        let port = mock_simple_response(response).await;

        let client = AdbClient::with_address("127.0.0.1", port);
        let version = client.server_version().await.unwrap();
        assert_eq!(version, 31);
    }

    #[tokio::test]
    async fn test_list_devices() {
        let device_list = b"emulator-5554\tdevice\n";
        let len_str = format!("{:04X}", device_list.len());

        let mut response = Vec::new();
        response.extend_from_slice(b"OKAY");
        response.extend_from_slice(len_str.as_bytes());
        response.extend_from_slice(device_list);
        let port = mock_simple_response(response).await;

        let client = AdbClient::with_address("127.0.0.1", port);
        let devices = client.list_devices().await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].serial, "emulator-5554");
    }

    #[tokio::test]
    async fn test_list_devices_empty() {
        let mut response = Vec::new();
        response.extend_from_slice(b"OKAY");
        response.extend_from_slice(b"0000");
        let port = mock_simple_response(response).await;

        let client = AdbClient::with_address("127.0.0.1", port);
        let devices = client.list_devices().await.unwrap();
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn test_server_fail_response() {
        let error_msg = b"device not found";
        let len_str = format!("{:04X}", error_msg.len());

        let mut response = Vec::new();
        response.extend_from_slice(b"FAIL");
        response.extend_from_slice(len_str.as_bytes());
        response.extend_from_slice(error_msg);
        let port = mock_simple_response(response).await;

        let client = AdbClient::with_address("127.0.0.1", port);
        let result = client.server_version().await;
        match result {
            Err(AdbError::ServerFail(msg)) => assert_eq!(msg, "device not found"),
            other => panic!("Expected ServerFail, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_connection_refused() {
        // Use port 1 which should refuse connections
        let client = AdbClient::with_address("127.0.0.1", 1);
        let result = client.server_version().await;
        assert!(
            matches!(
                result,
                Err(AdbError::ConnectionRefused) | Err(AdbError::Io(_))
            ),
            "Expected ConnectionRefused or Io error, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_shell_command() {
        // Mock: transport OKAY, shell OKAY, then output
        let port = mock_adb_server(move |mut socket| {
            tokio::spawn(async move {
                let mut buf = [0u8; 256];

                // Read transport command
                let _ = socket.read(&mut buf).await;
                socket.write_all(b"OKAY").await.unwrap();

                // Read shell command
                let _ = socket.read(&mut buf).await;
                socket.write_all(b"OKAY").await.unwrap();

                // Send shell output
                socket.write_all(b"hello world\n").await.unwrap();

                // Close connection (EOF signals end of output)
                drop(socket);
            });
        })
        .await;

        let client = AdbClient::with_address("127.0.0.1", port);
        let output = client.shell(None, "echo hello world").await.unwrap();
        assert_eq!(output, "hello world\n");
    }

    #[tokio::test]
    async fn test_stat_file() {
        // Mock: transport OKAY, sync OKAY, then STAT response
        let port = mock_adb_server(move |mut socket| {
            tokio::spawn(async move {
                let mut buf = [0u8; 256];

                // Read transport command, respond OKAY
                let _ = socket.read(&mut buf).await;
                socket.write_all(b"OKAY").await.unwrap();

                // Read sync command, respond OKAY
                let _ = socket.read(&mut buf).await;
                socket.write_all(b"OKAY").await.unwrap();

                // Read STAT request
                let _ = socket.read(&mut buf).await;

                // Send STAT response: STAT + mode(4) + size(4) + mtime(4)
                let mut resp = Vec::new();
                resp.extend_from_slice(b"STAT");
                resp.extend_from_slice(&0x000081A4u32.to_le_bytes()); // mode: regular file, 0644
                resp.extend_from_slice(&1024u32.to_le_bytes()); // size
                resp.extend_from_slice(&1_700_000_000u32.to_le_bytes()); // mtime
                socket.write_all(&resp).await.unwrap();

                // Read QUIT
                let _ = socket.read(&mut buf).await;
            });
        })
        .await;

        let client = AdbClient::with_address("127.0.0.1", port);
        let stat = client.stat(None, "/sdcard/test.txt").await.unwrap();
        assert_eq!(stat.size, 1024);
        assert!(stat.is_file());
        assert_eq!(stat.permissions(), 0o644);
    }
}

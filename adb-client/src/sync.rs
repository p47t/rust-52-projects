use crate::error::{AdbError, AdbResult};

/// Maximum chunk size for DATA packets in sync protocol (64 KB).
pub const SYNC_DATA_MAX: u32 = 64 * 1024;

/// Sync protocol command IDs — 4 ASCII characters.
///
/// Every sync message has an 8-byte header: a 4-byte ASCII command ID
/// followed by a 4-byte little-endian u32 length/value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncId {
    /// Query file metadata.
    Stat,
    /// List directory contents.
    List,
    /// Send (push) a file to the device.
    Send,
    /// Receive (pull) a file from the device.
    Recv,
    /// Data chunk within a send/recv transfer.
    Data,
    /// Marks the end of a file transfer.
    Done,
    /// Success acknowledgment.
    Okay,
    /// Error response.
    Fail,
    /// Directory entry (response to LIST).
    Dent,
    /// Quit sync mode.
    Quit,
}

impl SyncId {
    /// The 4-byte ASCII representation of this command ID.
    pub fn as_bytes(&self) -> &[u8; 4] {
        match self {
            SyncId::Stat => b"STAT",
            SyncId::List => b"LIST",
            SyncId::Send => b"SEND",
            SyncId::Recv => b"RECV",
            SyncId::Data => b"DATA",
            SyncId::Done => b"DONE",
            SyncId::Okay => b"OKAY",
            SyncId::Fail => b"FAIL",
            SyncId::Dent => b"DENT",
            SyncId::Quit => b"QUIT",
        }
    }

    /// Parse a 4-byte ASCII slice into a `SyncId`.
    pub fn from_bytes(bytes: &[u8]) -> AdbResult<SyncId> {
        if bytes.len() < 4 {
            return Err(AdbError::Protocol(format!(
                "Sync ID too short: {} bytes, need 4",
                bytes.len()
            )));
        }
        match &bytes[..4] {
            b"STAT" => Ok(SyncId::Stat),
            b"LIST" => Ok(SyncId::List),
            b"SEND" => Ok(SyncId::Send),
            b"RECV" => Ok(SyncId::Recv),
            b"DATA" => Ok(SyncId::Data),
            b"DONE" => Ok(SyncId::Done),
            b"OKAY" => Ok(SyncId::Okay),
            b"FAIL" => Ok(SyncId::Fail),
            b"DENT" => Ok(SyncId::Dent),
            b"QUIT" => Ok(SyncId::Quit),
            other => Err(AdbError::Protocol(format!(
                "Unknown sync ID: {:?}",
                String::from_utf8_lossy(other)
            ))),
        }
    }
}

/// The 8-byte sync header: 4-byte command ID + 4-byte little-endian u32 length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncHeader {
    pub id: SyncId,
    pub length: u32,
}

impl SyncHeader {
    pub fn new(id: SyncId, length: u32) -> Self {
        Self { id, length }
    }

    /// Serialize to exactly 8 bytes.
    pub fn to_bytes(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0..4].copy_from_slice(self.id.as_bytes());
        buf[4..8].copy_from_slice(&self.length.to_le_bytes());
        buf
    }

    /// Parse from a byte slice (must be at least 8 bytes).
    pub fn from_bytes(buf: &[u8]) -> AdbResult<Self> {
        if buf.len() < 8 {
            return Err(AdbError::Protocol(format!(
                "Sync header too short: {} bytes, need 8",
                buf.len()
            )));
        }
        let id = SyncId::from_bytes(&buf[0..4])?;
        let length = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        Ok(Self { id, length })
    }
}

/// STAT response: file metadata returned by the device.
///
/// The on-wire format is 16 bytes total: `STAT` (4) + mode (4) + size (4) + mtime (4).
/// This struct holds the 12 bytes after the `STAT` id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatResponse {
    /// Unix file mode (type + permissions).
    pub mode: u32,
    /// File size in bytes.
    pub size: u32,
    /// Last modification time (Unix timestamp).
    pub mtime: u32,
}

impl StatResponse {
    /// Parse from the 12 bytes following the STAT id.
    pub fn from_bytes(buf: &[u8]) -> AdbResult<Self> {
        if buf.len() < 12 {
            return Err(AdbError::Protocol(format!(
                "STAT response too short: {} bytes, need 12",
                buf.len()
            )));
        }
        let mode = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let size = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let mtime = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        Ok(Self { mode, size, mtime })
    }

    /// Whether this is a regular file (S_IFREG = 0o100000).
    pub fn is_file(&self) -> bool {
        (self.mode & 0o170000) == 0o100000
    }

    /// Whether this is a directory (S_IFDIR = 0o040000).
    pub fn is_directory(&self) -> bool {
        (self.mode & 0o170000) == 0o040000
    }

    /// Extract the permission bits (lower 12 bits).
    pub fn permissions(&self) -> u32 {
        self.mode & 0o7777
    }
}

/// Directory entry from LIST command response (DENT).
///
/// On-wire format: `DENT` (4) + mode (4) + size (4) + mtime (4) + namelen (4) + name.
/// This struct holds everything after the `DENT` id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DentEntry {
    /// Unix file mode.
    pub mode: u32,
    /// File size in bytes.
    pub size: u32,
    /// Last modification time.
    pub mtime: u32,
    /// File/directory name.
    pub name: String,
}

impl DentEntry {
    /// Parse from raw bytes: mode (4) + size (4) + mtime (4) + namelen (4) + name.
    pub fn from_bytes(buf: &[u8]) -> AdbResult<Self> {
        if buf.len() < 16 {
            return Err(AdbError::Protocol(format!(
                "DENT entry too short: {} bytes, need at least 16",
                buf.len()
            )));
        }
        let mode = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let size = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let mtime = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let namelen = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]) as usize;

        if buf.len() < 16 + namelen {
            return Err(AdbError::Protocol(format!(
                "DENT entry name truncated: have {} bytes, need {}",
                buf.len() - 16,
                namelen
            )));
        }
        let name = String::from_utf8_lossy(&buf[16..16 + namelen]).to_string();
        Ok(Self {
            mode,
            size,
            mtime,
            name,
        })
    }

    /// Total byte size of this entry on the wire (excluding the DENT id).
    pub fn wire_size(&self) -> usize {
        16 + self.name.len()
    }
}

/// Encode a STAT request: `STAT` + LE path length + path bytes.
pub fn encode_stat_request(remote_path: &str) -> Vec<u8> {
    let path_bytes = remote_path.as_bytes();
    let mut buf = Vec::with_capacity(8 + path_bytes.len());
    buf.extend_from_slice(b"STAT");
    buf.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(path_bytes);
    buf
}

/// Encode a LIST request: `LIST` + LE path length + path bytes.
pub fn encode_list_request(remote_path: &str) -> Vec<u8> {
    let path_bytes = remote_path.as_bytes();
    let mut buf = Vec::with_capacity(8 + path_bytes.len());
    buf.extend_from_slice(b"LIST");
    buf.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(path_bytes);
    buf
}

/// Encode a RECV request: `RECV` + LE path length + path bytes.
pub fn encode_recv_request(remote_path: &str) -> Vec<u8> {
    let path_bytes = remote_path.as_bytes();
    let mut buf = Vec::with_capacity(8 + path_bytes.len());
    buf.extend_from_slice(b"RECV");
    buf.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(path_bytes);
    buf
}

/// Encode a SEND request: `SEND` + LE length + `{remote_path},{mode}`.
pub fn encode_send_request(remote_path: &str, mode: u32) -> Vec<u8> {
    let payload = format!("{},{}", remote_path, mode);
    let payload_bytes = payload.as_bytes();
    let mut buf = Vec::with_capacity(8 + payload_bytes.len());
    buf.extend_from_slice(b"SEND");
    buf.extend_from_slice(&(payload_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload_bytes);
    buf
}

/// Encode a DATA chunk: `DATA` + LE data length + data bytes.
pub fn encode_data_chunk(data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + data.len());
    buf.extend_from_slice(b"DATA");
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(data);
    buf
}

/// Encode a DONE message with modification time: `DONE` + LE mtime.
pub fn encode_done(mtime: u32) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(b"DONE");
    buf[4..8].copy_from_slice(&mtime.to_le_bytes());
    buf
}

/// Encode a QUIT message: `QUIT` + LE 0.
pub fn encode_quit() -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(b"QUIT");
    // length is already 0
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- SyncId tests ---

    #[test]
    fn test_sync_id_all_variants_round_trip() {
        let variants = [
            (SyncId::Stat, b"STAT"),
            (SyncId::List, b"LIST"),
            (SyncId::Send, b"SEND"),
            (SyncId::Recv, b"RECV"),
            (SyncId::Data, b"DATA"),
            (SyncId::Done, b"DONE"),
            (SyncId::Okay, b"OKAY"),
            (SyncId::Fail, b"FAIL"),
            (SyncId::Dent, b"DENT"),
            (SyncId::Quit, b"QUIT"),
        ];
        for (id, bytes) in &variants {
            assert_eq!(id.as_bytes(), *bytes);
            assert_eq!(SyncId::from_bytes(*bytes).unwrap(), *id);
        }
    }

    #[test]
    fn test_sync_id_unknown() {
        assert!(SyncId::from_bytes(b"XXXX").is_err());
    }

    #[test]
    fn test_sync_id_too_short() {
        assert!(SyncId::from_bytes(b"ST").is_err());
    }

    // --- SyncHeader tests ---

    #[test]
    fn test_sync_header_round_trip() {
        let header = SyncHeader::new(SyncId::Stat, 42);
        let bytes = header.to_bytes();
        let parsed = SyncHeader::from_bytes(&bytes).unwrap();
        assert_eq!(header, parsed);
    }

    #[test]
    fn test_sync_header_raw_bytes() {
        let header = SyncHeader::new(SyncId::Stat, 42);
        let bytes = header.to_bytes();
        assert_eq!(&bytes[0..4], b"STAT");
        assert_eq!(
            u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            42
        );
    }

    #[test]
    fn test_sync_header_zero_length() {
        let header = SyncHeader::new(SyncId::Quit, 0);
        let bytes = header.to_bytes();
        assert_eq!(&bytes, b"QUIT\x00\x00\x00\x00");
    }

    #[test]
    fn test_sync_header_max_length() {
        let header = SyncHeader::new(SyncId::Data, u32::MAX);
        let bytes = header.to_bytes();
        let parsed = SyncHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.length, u32::MAX);
    }

    #[test]
    fn test_sync_header_too_short() {
        assert!(SyncHeader::from_bytes(&[0, 1, 2]).is_err());
    }

    // --- StatResponse tests ---

    #[test]
    fn test_stat_response_regular_file() {
        // mode = 0o100644 = 0x81A4 (regular file, rw-r--r--)
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x000081A4u32.to_le_bytes()); // mode
        buf.extend_from_slice(&1024u32.to_le_bytes()); // size
        buf.extend_from_slice(&1_700_000_000u32.to_le_bytes()); // mtime

        let stat = StatResponse::from_bytes(&buf).unwrap();
        assert_eq!(stat.size, 1024);
        assert_eq!(stat.mtime, 1_700_000_000);
        assert!(stat.is_file());
        assert!(!stat.is_directory());
        assert_eq!(stat.permissions(), 0o644);
    }

    #[test]
    fn test_stat_response_directory() {
        // mode = 0o040755 = 0x41ED (directory, rwxr-xr-x)
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x000041EDu32.to_le_bytes());
        buf.extend_from_slice(&4096u32.to_le_bytes());
        buf.extend_from_slice(&1_700_000_000u32.to_le_bytes());

        let stat = StatResponse::from_bytes(&buf).unwrap();
        assert!(!stat.is_file());
        assert!(stat.is_directory());
        assert_eq!(stat.permissions(), 0o755);
    }

    #[test]
    fn test_stat_response_nonexistent() {
        // mode=0, size=0, mtime=0 means file doesn't exist
        let buf = [0u8; 12];
        let stat = StatResponse::from_bytes(&buf).unwrap();
        assert_eq!(stat.mode, 0);
        assert_eq!(stat.size, 0);
        assert!(!stat.is_file());
        assert!(!stat.is_directory());
    }

    #[test]
    fn test_stat_response_too_short() {
        assert!(StatResponse::from_bytes(&[0u8; 8]).is_err());
    }

    // --- DentEntry tests ---

    #[test]
    fn test_dent_entry_parse() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x000041EDu32.to_le_bytes()); // mode (dir, 0755)
        buf.extend_from_slice(&4096u32.to_le_bytes()); // size
        buf.extend_from_slice(&1_700_000_000u32.to_le_bytes()); // mtime
        buf.extend_from_slice(&5u32.to_le_bytes()); // namelen
        buf.extend_from_slice(b"hello"); // name

        let dent = DentEntry::from_bytes(&buf).unwrap();
        assert_eq!(dent.name, "hello");
        assert_eq!(dent.size, 4096);
        assert_eq!(dent.wire_size(), 21); // 16 + 5
    }

    #[test]
    fn test_dent_entry_empty_name() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // namelen = 0

        let dent = DentEntry::from_bytes(&buf).unwrap();
        assert_eq!(dent.name, "");
    }

    #[test]
    fn test_dent_entry_truncated_name() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&10u32.to_le_bytes()); // namelen = 10
        buf.extend_from_slice(b"short"); // only 5 bytes

        assert!(DentEntry::from_bytes(&buf).is_err());
    }

    #[test]
    fn test_dent_entry_too_short() {
        assert!(DentEntry::from_bytes(&[0u8; 8]).is_err());
    }

    // --- Encode function tests ---

    #[test]
    fn test_encode_stat_request() {
        let encoded = encode_stat_request("/sdcard/test.txt");
        let header = SyncHeader::from_bytes(&encoded[0..8]).unwrap();
        assert_eq!(header.id, SyncId::Stat);
        assert_eq!(header.length, 16); // "/sdcard/test.txt".len()
        assert_eq!(&encoded[8..], b"/sdcard/test.txt");
    }

    #[test]
    fn test_encode_list_request() {
        let encoded = encode_list_request("/sdcard/");
        let header = SyncHeader::from_bytes(&encoded[0..8]).unwrap();
        assert_eq!(header.id, SyncId::List);
        assert_eq!(header.length, 8);
        assert_eq!(&encoded[8..], b"/sdcard/");
    }

    #[test]
    fn test_encode_recv_request() {
        let encoded = encode_recv_request("/data/local/tmp/file");
        let header = SyncHeader::from_bytes(&encoded[0..8]).unwrap();
        assert_eq!(header.id, SyncId::Recv);
        assert_eq!(&encoded[8..], b"/data/local/tmp/file");
    }

    #[test]
    fn test_encode_send_request() {
        let encoded = encode_send_request("/sdcard/file.txt", 0o644);
        let header = SyncHeader::from_bytes(&encoded[0..8]).unwrap();
        assert_eq!(header.id, SyncId::Send);
        let payload = std::str::from_utf8(&encoded[8..]).unwrap();
        assert!(payload.starts_with("/sdcard/file.txt,"));
        assert!(payload.ends_with("420")); // 0o644 = 420 decimal
    }

    #[test]
    fn test_encode_data_chunk() {
        let data = b"hello world";
        let encoded = encode_data_chunk(data);
        assert_eq!(&encoded[0..4], b"DATA");
        let len = u32::from_le_bytes([encoded[4], encoded[5], encoded[6], encoded[7]]);
        assert_eq!(len, 11);
        assert_eq!(&encoded[8..], b"hello world");
    }

    #[test]
    fn test_encode_data_chunk_empty() {
        let encoded = encode_data_chunk(b"");
        assert_eq!(&encoded[0..4], b"DATA");
        let len = u32::from_le_bytes([encoded[4], encoded[5], encoded[6], encoded[7]]);
        assert_eq!(len, 0);
        assert_eq!(encoded.len(), 8);
    }

    #[test]
    fn test_encode_done() {
        let mtime: u32 = 1_700_000_000;
        let encoded = encode_done(mtime);
        assert_eq!(&encoded[0..4], b"DONE");
        let val = u32::from_le_bytes([encoded[4], encoded[5], encoded[6], encoded[7]]);
        assert_eq!(val, mtime);
    }

    #[test]
    fn test_encode_quit() {
        let encoded = encode_quit();
        assert_eq!(&encoded[0..4], b"QUIT");
        let val = u32::from_le_bytes([encoded[4], encoded[5], encoded[6], encoded[7]]);
        assert_eq!(val, 0);
    }

    // --- Round-trip tests ---

    #[test]
    fn test_encode_decode_stat_round_trip() {
        let path = "/mnt/sdcard/DCIM/photo.jpg";
        let encoded = encode_stat_request(path);
        let header = SyncHeader::from_bytes(&encoded[0..8]).unwrap();
        assert_eq!(header.id, SyncId::Stat);
        let decoded_path = std::str::from_utf8(&encoded[8..8 + header.length as usize]).unwrap();
        assert_eq!(decoded_path, path);
    }

    #[test]
    fn test_all_encode_functions_have_correct_header() {
        // Verify each encode function produces a valid SyncHeader
        let test_cases: Vec<(&str, Vec<u8>, SyncId)> = vec![
            ("stat", encode_stat_request("/test"), SyncId::Stat),
            ("list", encode_list_request("/test"), SyncId::List),
            ("recv", encode_recv_request("/test"), SyncId::Recv),
            ("send", encode_send_request("/test", 0o644), SyncId::Send),
            ("data", encode_data_chunk(b"payload"), SyncId::Data),
        ];

        for (name, encoded, expected_id) in test_cases {
            let header = SyncHeader::from_bytes(&encoded[0..8])
                .unwrap_or_else(|_| panic!("Failed to parse header for {}", name));
            assert_eq!(header.id, expected_id, "Wrong ID for {}", name);
            assert_eq!(
                header.length as usize,
                encoded.len() - 8,
                "Wrong length for {}",
                name
            );
        }
    }
}

use bytes::{Buf, BufMut, BytesMut};
use std::io::{Cursor, Read, BufRead};

#[derive(PartialEq, Debug)]
pub enum Packet {
    ReadRequest {
        filename: String,
        mode: String,
    },
    WriteRequest {
        filename: String,
        mode: String,
    },
    Data {
        block_num: u16,
        data: Vec<u8>,
    },
    Ack {
        block_num: u16,
    },
    Error {
        error_code: u16,
        error_msg: String,
    },
}

const OP_RRQ: u16 = 1;
const OP_WRQ: u16 = 2;
const OP_DATA: u16 = 3;
const OP_ACK: u16 = 4;
const OP_ERROR: u16 = 5;

impl Packet {
    pub fn from(payload: &[u8]) -> Option<Packet> {
        let mut cursor = Cursor::new(payload);
        match cursor.get_u16_be() {
            OP_RRQ => Some(Packet::ReadRequest {
                filename: Self::read_cstr(&mut cursor),
                mode: Self::read_cstr(&mut cursor),
            }),
            OP_WRQ => Some(Packet::WriteRequest {
                filename: Self::read_cstr(&mut cursor),
                mode: Self::read_cstr(&mut cursor),
            }),
            OP_DATA => Some(Packet::Data {
                block_num: cursor.get_u16_be(),
                data: {
                    let mut vec = Vec::<u8>::new();
                    cursor.read_to_end(&mut vec);
                    vec
                },
            }),
            OP_ACK => Some(Packet::Ack {
                block_num: cursor.get_u16_be(),
            }),
            OP_ERROR => Some(Packet::Error {
                error_code: cursor.get_u16_be(),
                error_msg: Self::read_cstr(&mut cursor),
            }),
            _ => None
        }
    }

    fn read_cstr(cursor: &mut Cursor<&[u8]>) -> String {
        let mut cstr = String::new();
        loop {
            let b = cursor.get_u8();
            if b == 0u8 {
                break;
            }
            cstr.push(b as char);
        }
        cstr
    }

    fn read_null_terminated_utf8(cursor: &mut Cursor<&[u8]>) -> String {
        let mut vec = Vec::<u8>::new();
        cursor.read_until(0u8, &mut vec);
        String::from_utf8(vec).unwrap()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(1024);
        match self {
            Packet::ReadRequest { filename, mode } => {
                buf.put_u16_be(OP_RRQ);

                filename.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);

                mode.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
            }
            Packet::WriteRequest { filename, mode } => {
                buf.put_u16_be(OP_WRQ);

                filename.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);

                mode.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
            }
            Packet::Data { block_num, data } => {
                buf.put_u16_be(OP_DATA);
                buf.put_u16_be(*block_num);
                buf.put_slice(data);
            }
            Packet::Ack { block_num } => {
                buf.put_u16_be(OP_ACK);
                buf.put_u16_be(*block_num);
            }
            Packet::Error { error_code, error_msg } => {
                buf.put_u16_be(OP_ERROR);
                buf.put_u16_be(*error_code);
                error_msg.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
            }
        }
        buf.to_vec()
    }
}

pub struct Receiver {}

impl Receiver {
    pub fn new(_file: &str) -> Receiver {
        Receiver {}
    }

    pub fn process(&self, _packet: &Packet) -> Option<Packet> {
        None
    }
}

pub struct Sender {}

impl Sender {
    pub fn new(_file: &str) -> Sender {
        Sender {}
    }

    pub fn process(&self, _packet: &Packet) -> Option<Packet> {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::tftp::Packet;

    #[test]
    fn test_packet_parse() {
        let bytes = [
            00u8, 0x01,
            0x72, 0x66, 0x63, 0x31, 0x33, 0x35, 0x30, 0x2e, 0x74, 0x78, 0x74, 0x00,
            0x6f, 0x63, 0x74, 0x65, 0x74, 0x00
        ];
        assert_eq!(Packet::from(&bytes),
                   Some(Packet::ReadRequest {
                       filename: "rfc1350.txt".to_owned(),
                       mode: "octet".to_owned(),
                   }));

        let bytes = [
            00u8, 0x02,
            0x72, 0x66, 0x63, 0x31, 0x33, 0x35, 0x30, 0x2e, 0x74, 0x78, 0x74, 0x00,
            0x6f, 0x63, 0x74, 0x65, 0x74, 0x00
        ];
        assert_eq!(Packet::from(&bytes),
                   Some(Packet::WriteRequest {
                       filename: "rfc1350.txt".to_owned(),
                       mode: "octet".to_owned(),
                   }));

        let bytes = [
            00u8, 0x03,
            0x00, 0x01,
            0x0a, 0x0a,
        ];
        assert_eq!(Packet::from(&bytes),
                   Some(Packet::Data {
                       block_num: 1,
                       data: vec![0x0a, 0x0a],
                   }));

        let bytes = [
            00u8, 0x04,
            0x00, 0x01,
        ];
        assert_eq!(Packet::from(&bytes),
                   Some(Packet::Ack {
                       block_num: 1
                   }));

        let bytes = [
            00u8, 0x05,
            0x00, 0x02,
            0x64, 0x65, 0x6e, 0x69, 0x65, 0x64, 0x00,
        ];
        assert_eq!(Packet::from(&bytes),
                   Some(Packet::Error {
                       error_code: 2,
                       error_msg: "denied".to_owned(),
                   }));
    }

    #[test]
    fn test_packet_serialization() {
        let bytes = [
            00u8, 0x01,
            0x72, 0x66, 0x63, 0x31, 0x33, 0x35, 0x30, 0x2e, 0x74, 0x78, 0x74, 0x00,
            0x6f, 0x63, 0x74, 0x65, 0x74, 0x00
        ];
        assert_eq!(Packet::ReadRequest {
                       filename: "rfc1350.txt".to_owned(),
                       mode: "octet".to_owned(),
                   }.to_bytes(), bytes);

        let bytes = [
            00u8, 0x02,
            0x72, 0x66, 0x63, 0x31, 0x33, 0x35, 0x30, 0x2e, 0x74, 0x78, 0x74, 0x00,
            0x6f, 0x63, 0x74, 0x65, 0x74, 0x00
        ];
        assert_eq!(Packet::WriteRequest {
                       filename: "rfc1350.txt".to_owned(),
                       mode: "octet".to_owned(),
                   }.to_bytes(), bytes);

        let bytes = [
            00u8, 0x03,
            0x00, 0x01,
            0x0a, 0x0a,
        ];
        assert_eq!(Packet::Data {
                       block_num: 1,
                       data: vec![0x0a, 0x0a],
                   }.to_bytes(), bytes);

        let bytes = [
            00u8, 0x04,
            0x00, 0x01,
        ];
        assert_eq!(Packet::Ack {
                       block_num: 1
                   }.to_bytes(), bytes);

        let bytes = [
            00u8, 0x05,
            0x00, 0x02,
            0x64, 0x65, 0x6e, 0x69, 0x65, 0x64, 0x00,
        ];
        assert_eq!(Packet::Error {
                       error_code: 2,
                       error_msg: "denied".to_owned(),
                   }.to_bytes(), bytes);
    }
}
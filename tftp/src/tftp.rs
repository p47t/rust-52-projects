use std::fs::File;
use std::io::{Cursor, Read, Write};

use bytes::{Buf, BufMut, BytesMut};

const BLOCK_SIZE: usize = 512;

enum OpCode {
    Rrq = 1,
    Wrq,
    Data,
    Ack,
    Error,
    Invalid,
}

impl From<u16> for OpCode {
    fn from(val: u16) -> OpCode {
        match val {
            1 => OpCode::Rrq,
            2 => OpCode::Wrq,
            3 => OpCode::Data,
            4 => OpCode::Ack,
            5 => OpCode::Error,
            _ => OpCode::Invalid,
        }
    }
}

#[allow(dead_code)]
enum ErrorCode {
    NotDefined = 0,
    FileNotFound = 1,
    AccessViolation = 2,
    DiskFull = 3,
    IllegalOp = 4,
    UnknownTid = 5,
    FileAlreadyExists = 6,
    NoSuchUser = 7,
}

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

impl Packet {
    pub fn from(payload: &[u8]) -> Option<Packet> {
        let mut cursor = Cursor::new(payload);
        match OpCode::from(cursor.get_u16_be()) {
            OpCode::Rrq => Some(Packet::ReadRequest {
                filename: read_cstr(&mut cursor),
                mode: read_cstr(&mut cursor),
            }),
            OpCode::Wrq => Some(Packet::WriteRequest {
                filename: read_cstr(&mut cursor),
                mode: read_cstr(&mut cursor),
            }),
            OpCode::Data => Some(Packet::Data {
                block_num: cursor.get_u16_be(),
                data: {
                    let mut vec = Vec::<u8>::new();
                    let _ = cursor.read_to_end(&mut vec);
                    vec
                },
            }),
            OpCode::Ack => Some(Packet::Ack {
                block_num: cursor.get_u16_be(),
            }),
            OpCode::Error => Some(Packet::Error {
                error_code: cursor.get_u16_be(),
                error_msg: read_cstr(&mut cursor),
            }),
            _ => None
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf;
        match self {
            Packet::ReadRequest { filename, mode } => {
                buf = BytesMut::with_capacity(128);
                buf.put_u16_be(OpCode::Rrq as u16);
                filename.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
                mode.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
            }
            Packet::WriteRequest { filename, mode } => {
                buf = BytesMut::with_capacity(128);
                buf.put_u16_be(OpCode::Wrq as u16);
                filename.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
                mode.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
            }
            Packet::Data { block_num, data } => {
                buf = BytesMut::with_capacity(4);
                buf.put_u16_be(OpCode::Data as u16);
                buf.put_u16_be(*block_num);
                buf.extend_from_slice(data);
            }
            Packet::Ack { block_num } => {
                buf = BytesMut::with_capacity(4);
                buf.put_u16_be(OpCode::Ack as u16);
                buf.put_u16_be(*block_num);
            }
            Packet::Error { error_code, error_msg } => {
                buf = BytesMut::with_capacity(128);
                buf.put_u16_be(OpCode::Error as u16);
                buf.put_u16_be(*error_code);
                error_msg.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
            }
        }
        buf.freeze().to_vec()
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

pub trait LockStep {
    fn process(&mut self, packet: &Packet) -> Option<Packet>;
    fn done(&self) -> bool;
}

pub struct Receiver {
    current_block: u16,
    done: bool,
    file: std::fs::File,
}

impl Receiver {
    pub fn new(path: &str) -> std::io::Result<Receiver> {
        Ok(Receiver {
            current_block: 0,
            done: false,
            file: File::create(path)?
        })
    }
}

impl LockStep for Receiver {
    fn process(&mut self, packet: &Packet) -> Option<Packet> {
        match packet {
            Packet::WriteRequest { .. } => {
                Some(Packet::Ack { block_num: 0 })
            }
            Packet::Data { block_num, data } => {
                if *block_num != self.current_block + 1 {
                    return None;
                }
                self.current_block = *block_num;
                let _ = self.file.write(data);
                if data.len() < BLOCK_SIZE {
                    self.done = true;
                }

                Some(Packet::Ack { block_num: *block_num })
            }
            _ => None
        }
    }

    fn done(&self) -> bool { self.done }
}

pub struct Sender {
    current_block: u16,
    done: bool,
    file: std::fs::File,
}

impl Sender {
    pub fn new(path: &str) -> std::io::Result<Sender> {
        Ok(Sender {
            current_block: 0,
            done: false,
            file: File::open(path)?,
        })
    }

    pub fn next_block(&mut self) -> std::io::Result<Packet> {
        let mut data = vec![0; BLOCK_SIZE];
        let size = self.file.read(data.as_mut_slice())?;
        self.current_block += 1;
        if size < BLOCK_SIZE {
            data.truncate(size);
            self.done = true;
        }
        Ok(Packet::Data {
            block_num: self.current_block,
            data,
        })
    }
}

impl LockStep for Sender {
    fn process(&mut self, packet: &Packet) -> Option<Packet> {
        match packet {
            Packet::ReadRequest { .. } => {
                if let Ok(packet) = self.next_block() {
                    return Some(packet)
                }
                None
            }
            Packet::Ack { block_num } => {
                if *block_num == self.current_block {
                    if let Ok(packet) = self.next_block() {
                        return Some(packet)
                    }
                }
                None
            }
            _ => None
        }
    }

    fn done(&self) -> bool { self.done }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: &str = "rfc1350.txt";
    const OUTPUT: &str = "rfc1350-received.txt";
    const MODE: &str = "octet";
    const CONTENT: &[u8] = b"TFTP";

    #[test]
    fn test_packet_parse() {
        let bytes = [
            00u8, 0x01,
            0x72, 0x66, 0x63, 0x31, 0x33, 0x35, 0x30, 0x2e, 0x74, 0x78, 0x74, 0x00,
            0x6f, 0x63, 0x74, 0x65, 0x74, 0x00
        ];
        assert_eq!(Packet::from(&bytes),
                   Some(Packet::ReadRequest {
                       filename: INPUT.to_owned(),
                       mode: MODE.to_owned(),
                   }));

        let bytes = [
            00u8, 0x02,
            0x72, 0x66, 0x63, 0x31, 0x33, 0x35, 0x30, 0x2e, 0x74, 0x78, 0x74, 0x00,
            0x6f, 0x63, 0x74, 0x65, 0x74, 0x00
        ];
        assert_eq!(Packet::from(&bytes),
                   Some(Packet::WriteRequest {
                       filename: INPUT.to_owned(),
                       mode: MODE.to_owned(),
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
            filename: INPUT.to_owned(),
            mode: MODE.to_owned(),
        }.to_bytes(), bytes);

        let bytes = [
            00u8, 0x02,
            0x72, 0x66, 0x63, 0x31, 0x33, 0x35, 0x30, 0x2e, 0x74, 0x78, 0x74, 0x00,
            0x6f, 0x63, 0x74, 0x65, 0x74, 0x00
        ];
        assert_eq!(Packet::WriteRequest {
            filename: INPUT.to_owned(),
            mode: MODE.to_owned(),
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

    #[test]
    fn test_receiver() {
        let mut receiver = Receiver::new(OUTPUT).unwrap();
        assert!(!receiver.done());

        let reply = receiver.process(&Packet::Ack { block_num: 0 });
        assert_eq!(reply, None);
        assert!(!receiver.done());

        let reply = receiver.process(&Packet::Data { block_num: 2, data: vec![] });
        assert_eq!(reply, None);
        assert!(!receiver.done());

        let reply = receiver.process(&Packet::Data { block_num: 1, data: vec![] });
        assert_eq!(reply, Some(Packet::Ack { block_num: 1 }));
        assert!(receiver.done());
    }

    #[test]
    fn test_sender() {
        let mut sender = Sender::new(INPUT).unwrap();
        assert!(!sender.done());

        let reply = sender.process(&Packet::Ack { block_num: 0 });
        assert_eq!(reply, Some(Packet::Data { block_num: 1, data: CONTENT.to_vec() }));
        assert!(sender.done());
    }

    #[test]
    fn test_sender_receiver() {
        let mut sender = Sender::new(INPUT).unwrap();
        let mut receiver = Receiver::new(OUTPUT).unwrap();

        let reply = receiver.process(&Packet::WriteRequest {
            filename: OUTPUT.to_string(),
            mode: MODE.to_string(),
        });
        assert_eq!(reply, Some(Packet::Ack { block_num: 0 }));
        let reply = sender.process(&reply.unwrap());
        assert_eq!(reply, Some(Packet::Data { block_num: 1, data: CONTENT.to_vec() }));
        assert!(sender.done());
        let reply = receiver.process(&reply.unwrap());
        assert_eq!(reply, Some(Packet::Ack { block_num: 1 }));
        assert!(receiver.done());
    }

    #[test]
    fn test_receiver_sender() {
        let mut sender = Sender::new(INPUT).unwrap();
        let mut receiver = Receiver::new(OUTPUT).unwrap();

        let reply = sender.process(&Packet::ReadRequest {
            filename: INPUT.to_string(),
            mode: MODE.to_string(),
        });
        assert_eq!(reply, Some(Packet::Data { block_num: 1, data: CONTENT.to_vec() }));
        assert!(sender.done());
        let reply = receiver.process(&reply.unwrap());
        assert_eq!(reply, Some(Packet::Ack { block_num: 1 }));
        assert!(receiver.done());
    }
}
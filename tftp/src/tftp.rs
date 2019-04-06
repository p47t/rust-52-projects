use bytes::{Buf, BufMut, BytesMut};
use std::io::{Cursor, Read};

enum OpCode {
    Rrq = 1, Wrq, Data, Ack, Error, Invalid
}

impl OpCode {
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
        let mut buf = BytesMut::with_capacity(1024);
        match self {
            Packet::ReadRequest { filename, mode } => {
                buf.put_u16_be(OpCode::Rrq as u16);

                filename.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);

                mode.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
            }
            Packet::WriteRequest { filename, mode } => {
                buf.put_u16_be(OpCode::Wrq as u16);

                filename.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);

                mode.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
            }
            Packet::Data { block_num, data } => {
                buf.put_u16_be(OpCode::Data as u16);
                buf.put_u16_be(*block_num);
                buf.put_slice(data);
            }
            Packet::Ack { block_num } => {
                buf.put_u16_be(OpCode::Ack as u16);
                buf.put_u16_be(*block_num);
            }
            Packet::Error { error_code, error_msg } => {
                buf.put_u16_be(OpCode::Error as u16);
                buf.put_u16_be(*error_code);
                error_msg.bytes().for_each(|b| buf.put(b));
                buf.put(0u8);
            }
        }
        buf.to_vec()
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

pub trait Processor {
    fn process(&mut self, packet: &Packet) -> Result<Option<Packet>, ()>;
    fn done(&self) -> bool;
}

pub struct Receiver {
    current_block: u16,
    done: bool,
}

impl Receiver {
    pub fn new(_file: &str) -> Receiver {
        Receiver {
            current_block: 0,
            done: false,
        }
    }
}

impl Processor for Receiver {
    fn process(&mut self, packet: &Packet) -> Result<Option<Packet>, ()> {
        match packet {
            Packet::WriteRequest { .. } => {
                Ok(Some(Packet::Ack { block_num: 0 }))
            }
            Packet::Data { block_num, data } => {
                if *block_num != self.current_block + 1 {
                    return Err(());
                }
                self.current_block = *block_num;

                // TODO: write data to file
                if data.len() < 512 {
                    self.done = true;
                }

                Ok(Some(Packet::Ack { block_num: *block_num }))
            }
            _ => {
                Err(())
            }
        }
    }

    fn done(&self) -> bool { self.done }
}

pub struct Sender {
    current_block: u16,
    done: bool,
}

impl Sender {
    pub fn new(_file: &str) -> Sender {
        Sender {
            current_block: 0,
            done: false,
        }
    }
}

impl Processor for Sender {
    fn process(&mut self, packet: &Packet) -> Result<Option<Packet>, ()> {
        match packet {
            Packet::ReadRequest { .. } => {
                Ok(Some(Packet::Ack { block_num: 0 }))
            }
            Packet::Ack { block_num } => {
                if *block_num == self.current_block {
                    // TODO: read data from file
                    let data = vec![0u8];
                    self.current_block += 1;
                    if data.len() < 512 {
                        self.done = true;
                    }

                    Ok(Some(Packet::Data { block_num: self.current_block, data }))
                } else {
                    Err(())
                }
            }
            _ => {
                Err(())
            }
        }
    }

    fn done(&self) -> bool { self.done }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_receiver() {
        let mut receiver = Receiver::new("rfc1350.txt");
        assert!(!receiver.done());

        let reply = receiver.process(&Packet::Ack { block_num: 0 });
        assert_eq!(reply, Err(()));
        assert!(!receiver.done());

        let reply = receiver.process(&Packet::Data { block_num: 2, data: vec![0u8] });
        assert_eq!(reply, Err(()));
        assert!(!receiver.done());

        let reply = receiver.process(&Packet::Data { block_num: 1, data: vec![0u8] });
        assert_eq!(reply, Ok(Some(Packet::Ack { block_num: 1 })));
        assert!(receiver.done());
    }

    #[test]
    fn test_sender() {
        let mut sender = Sender::new("rfc1350.txt");
        assert!(!sender.done());

        let reply = sender.process(&Packet::Ack { block_num: 0 });
        assert_eq!(reply, Ok(Some(Packet::Data { block_num: 1, data: vec![0u8] })));
        assert!(sender.done());
    }
}
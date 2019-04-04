use bytes::{Buf, BufMut, BytesMut};
use std::io::{Cursor, Read};

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
    pub fn parse(payload: &[u8]) -> Option<Packet> {
        let mut buf = Cursor::new(payload);
        match buf.get_u16_be() {
            OP_RRQ => Some(Packet::ReadRequest {
                filename: "".to_owned(),
                mode: "".to_owned(),
            }),
            OP_WRQ => Some(Packet::WriteRequest {
                filename: "".to_owned(),
                mode: "".to_owned(),
            }),
            OP_DATA => Some(Packet::Data {
                block_num: 0,
                data: Vec::new(),
            }),
            OP_ACK => Some(Packet::Ack {
                block_num: 0,
            }),
            OP_ERROR => Some(Packet::Error {
                error_code: 0,
                error_msg: "".to_owned(),
            }),
            _ => None
        }
    }

    pub fn generate(packet: &Packet) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(1024);
        match packet {
            Packet::ReadRequest { .. } => {
                buf.put_u16_be(OP_RRQ);
            }
            Packet::WriteRequest { .. } => {
                buf.put_u16_be(OP_WRQ);
            }
            Packet::Data { .. } => {
                buf.put_u16_be(OP_DATA);
            }
            Packet::Ack { .. } => {
                buf.put_u16_be(OP_ACK);
            }
            Packet::Error { .. } => {
                buf.put_u16_be(OP_ERROR);
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
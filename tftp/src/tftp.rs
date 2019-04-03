pub struct Receiver {}

enum Packet {
    ReadRequest,
    WriteRequest,
    Ack,
    Data,
}

impl Receiver {
    pub fn new(_file: &str) -> Receiver {
        Receiver {}
    }

    pub fn process(&self, packet: &Packet) -> Option<Packet> {
        None
    }
}

pub struct Sender {}

impl Sender {
    pub fn new(_file: &str) -> Sender {
        Sender {}
    }

    pub fn process(&self, packet: &Packet) -> Option<Packet> {
        None
    }
}
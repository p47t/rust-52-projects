use std::net::UdpSocket;
use crate::tftp::{Sender, Receiver, Packet};

pub struct Client {}

impl Client {
    pub fn new() -> Client {
        Client {}
    }

    pub fn send(&self, file: &str, to: &str) -> std::io::Result<()> {
        let _sender = Sender::new(file);
        self.connect(to)?;
        Ok(())
    }

    pub fn recv(&self, file: &str, from: &str) -> std::io::Result<()> {
        let _receiver = Receiver::new(file);
        self.connect(from)?;
        Ok(())
    }

    pub fn connect(&self, addr: &str) -> std::io::Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect(addr)?;

        let mut buf = [0u8; 10];
        loop {
            let packet = Packet::generate(&Packet::ReadRequest {
                filename: "".to_owned(),
                mode: "".to_owned(),
            });
            socket.send(packet.as_slice())?;
            socket.recv(&mut buf)?;
        }
    }
}
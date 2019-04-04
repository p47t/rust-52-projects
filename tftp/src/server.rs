use std::net::UdpSocket;
use crate::tftp::{Sender, Receiver, Packet};

pub struct Server {}

impl Server {
    pub fn new() -> Server {
        Server {}
    }

    pub fn send(&self, file: &str, at: &str) -> std::io::Result<()> {
        let _sender = Sender::new(file);
        self.serve(at)?;
        Ok(())
    }

    pub fn recv(&self, file: &str, at: &str) -> std::io::Result<()> {
        let _receiver = Receiver::new(file);
        self.serve(at)?;
        Ok(())
    }

    pub fn serve(&self, addr: &str) -> std::io::Result<()> {
        let socket = UdpSocket::bind(addr)?;
        let mut buf = [0u8; 10];
        loop {
            socket.recv(&mut buf)?;
            let _packet = Packet::parse(&buf);
            socket.send(&buf)?;
        }
    }
}
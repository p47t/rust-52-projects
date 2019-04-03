use std::net::UdpSocket;
use crate::tftp::{Sender, Receiver};

pub struct Server {}

impl Server {
    pub fn new() -> Server {
        Server {}
    }

    pub fn send(&self, file: &str, at: &str) -> std::io::Result<()> {
        let sender = Sender::new(file);
        self.serve(at)?;
        Ok(())
    }

    pub fn recv(&self, file: &str, at: &str) -> std::io::Result<()> {
        let receiver = Receiver::new(file);
        self.serve(at)?;
        Ok(())
    }

    pub fn serve(&self, addr: &str) -> std::io::Result<()> {
        let socket = UdpSocket::bind(addr)?;
        let mut buf = [0u8; 10];
        loop {
            socket.recv(&mut buf)?;
            socket.send(&buf)?;
        }
    }
}
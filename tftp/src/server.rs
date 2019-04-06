use std::net::UdpSocket;
use crate::tftp::{Sender, Receiver, Packet, Processor};
use std::io;

pub struct Server {}

impl Server {
    pub fn new() -> Server {
        Server {}
    }

    pub fn send(&self, file: &str, at: &str) -> std::io::Result<()> {
        let mut sender = Sender::new(file)?;
        self.serve(at, &mut sender)
    }

    pub fn recv(&self, file: &str, at: &str) -> std::io::Result<()> {
        let mut receiver = Receiver::new(file)?;
        self.serve(at, &mut receiver)
    }

    fn serve<T: Processor>(&self, addr: &str, processor: &mut T) -> io::Result<()> {
        let socket = UdpSocket::bind(addr)?;
        let mut buf = [0u8; 1024];
        while !processor.done() {
            let (size, org) = socket.recv_from(&mut buf)?;
            if let Some(packet) = Packet::from(&buf[..size]) {
                if let Some(reply) = processor.process(&packet) {
                    socket.send_to(reply.to_bytes().as_slice(), org)?;
                }
            }
        }
        Ok(())
    }
}
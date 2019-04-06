use std::net::UdpSocket;
use std::io;
use crate::tftp::{Sender, Receiver, Packet, Processor};

pub struct Client {}

impl Client {
    pub fn new() -> Client {
        Client {}
    }

    pub fn send(&self, file: &str, to: &str) -> io::Result<()> {
        let mut sender = Sender::new(file);
        self.start(to, &mut sender, &Packet::WriteRequest {
            filename: file.to_owned(),
            mode: "octet".to_owned(),
        })
    }

    pub fn recv(&self, file: &str, from: &str) -> io::Result<()> {
        let mut receiver = Receiver::new(file);
        self.start(from, &mut receiver, &Packet::ReadRequest {
            filename: file.to_owned(),
            mode: "octet".to_owned(),
        })
    }

    fn start<T: Processor>(&self, addr: &str, processor: &mut T, req: &Packet) -> io::Result<()> {
        // send initial request and get origin address of response
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.send_to(req.to_bytes().as_slice(), addr)?;

        let mut buf = [0u8; 1024];
        while !processor.done() {
            let (size, org) = socket.recv_from(&mut buf)?;
            if let Some(packet) = Packet::from(&buf[..size]) {
                if let Ok(Some(reply)) = processor.process(&packet) {
                    socket.send_to(reply.to_bytes().as_slice(), org);
                }
            }
        }
        Ok(())
    }
}
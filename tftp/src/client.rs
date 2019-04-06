use std::net::UdpSocket;
use std::io;
use crate::tftp::{Packet, Processor, Sender, Receiver};

pub struct Client {}

impl Client {
    pub fn new() -> Client {
        Client {}
    }

    pub fn upload(&self, file: &str, to: &str) -> io::Result<()> {
        let mut sender = Sender::new(file);
        self.execute(to, &mut sender, &Packet::WriteRequest {
            filename: file.to_owned(),
            mode: "octet".to_owned(),
        })
    }

    pub fn download(&self, file: &str, from: &str) -> io::Result<()> {
        let mut receiver = Receiver::new(file);
        self.execute(from, &mut receiver, &Packet::ReadRequest {
            filename: file.to_owned(),
            mode: "octet".to_owned(),
        })
    }

    // Execute request with specified packet processor
    fn execute<T: Processor>(&self, addr: &str, processor: &mut T, req: &Packet) -> io::Result<()> {
        // send initial request and get origin address of response
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.send_to(req.to_bytes().as_slice(), addr)?;

        let mut buf = [0u8; 1024];
        while !processor.done() {
            let (size, org) = socket.recv_from(&mut buf)?;
            if let Some(packet) = Packet::from(&buf[..size]) {
                if let Ok(Some(reply)) = processor.process(&packet) {
                    socket.send_to(reply.to_bytes().as_slice(), org)?;
                }
            }
        }
        Ok(())
    }
}
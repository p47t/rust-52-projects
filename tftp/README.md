My second rust app is to implement TFTP (RFC-1350) client/server.

Things I learned:

- Use trait to reuse code with static dispatching
- Use failure crate to handle various errors
- How to use `UdpSocket`
- More advance enum usage
- Byte array manipulation with bytes crate
- TFTP protocol, of course!
- Implement lockstep behavior with a packet processor
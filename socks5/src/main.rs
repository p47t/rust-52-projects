#[cfg(test)]
use strum::IntoEnumIterator;

#[cfg(test)]
#[derive(Debug, Clone, Copy, strum_macros::EnumIter)]
enum ResponseCode {
    Success = 0x00,
    Failure = 0x01,
    RuleFailure = 0x02,
    NetworkUnreachable = 0x03,
    HostUnreachable = 0x04,
    ConnectionRefused = 0x05,
    TtlExpired = 0x06,
    CommandNotSupported = 0x07,
    AddrTypeNotSupported = 0x08,
}

#[cfg(test)]
struct SocksReply {
    // The SOCKS request information is sent by the client as soon as it has
    // established a connection to the SOCKS server, and completed the
    // authentication negotiations.  The server evaluates the request, and
    // returns a reply formed as follows:
    //
    //         +----+-----+-------+------+----------+----------+
    //         |VER | REP |  RSV  | ATYP | BND.ADDR | BND.PORT |
    //         +----+-----+-------+------+----------+----------+
    //         | 1  |  1  | X'00' |  1   | Variable |    2     |
    //         +----+-----+-------+------+----------+----------+
    //
    //     Where:
    //
    //         o  VER    protocol version: X'05'
    //         o  REP    Reply field:
    //             o  X'00' succeeded
    //             o  X'01' general SOCKS server failure
    //             o  X'02' connection not allowed by ruleset
    //             o  X'03' Network unreachable
    //             o  X'04' Host unreachable
    //             o  X'05' Connection refused
    //             o  X'06' TTL expired
    //             o  X'07' Command not supported
    //             o  X'08' Address type not supported
    //             o  X'09' to X'FF' unassigned
    //         o  RSV    RESERVED
    //         o  ATYP   address type of following address
    //
    //            o  IP V4 address: X'01'
    //            o  DOMAINNAME: X'03'
    //            o  IP V6 address: X'04'
    //         o  BND.ADDR       server bound address
    //         o  BND.PORT       server bound port in network octet order
    buf: [u8; 10],
}

#[cfg(test)]
impl SocksReply {
    fn new(status: ResponseCode) -> Self {
        let buf = [5, status as u8, 0, 1, 0, 0, 0, 0, 0, 0];
        Self { buf }
    }
}

#[cfg(test)]
#[test]
fn test_socks_reply() {
    for rc in ResponseCode::iter() {
        let reply = SocksReply::new(rc);
        assert_eq!(reply.buf[0], 5);
        assert_eq!(reply.buf[1], rc as u8)
    }
}

fn main() {}

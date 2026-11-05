//! Opening the host's port by PCP (RFC 6887) or, on older routers, NAT-PMP (RFC 6886), its
//! predecessor, both asked of the default route's gateway on UDP 5351 (D8). A router that
//! speaks only NAT-PMP answers PCP's request with NAT-PMP's version (RFC 6887, section 9),
//! and is asked by NAT-PMP then; one that speaks neither doesn't answer.
//!
//! Every call waits on the network for up to a couple of seconds, so make them off the
//! frame loop.

use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

use crate::portmap::Protocol;

/// The port PCP and NAT-PMP servers listen on.
pub const SERVER_PORT: u16 = 5351;
/// How long each try waits for its answer: RFC 6886's 250 ms, doubling, cut short at
/// three tries so a router that answers neither doesn't hold up the join code long.
const WAITS: [Duration; 3] = [
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_millis(1000),
];

const PCP_VERSION: u8 = 2;
const NAT_PMP_VERSION: u8 = 0;
/// PCP's MAP, and NAT-PMP's mapping of a UDP port.
const OPCODE_MAP: u8 = 1;
const OPCODE_ADDRESS: u8 = 0;
/// Set in an answer's opcode.
const ANSWER: u8 = 0x80;
const UDP: u8 = 17;
/// A MAP request or answer: the common header and MAP's own fields.
const PCP_MAP_LENGTH: usize = 60;
const NAT_PMP_MAP_LENGTH: usize = 16;
const NAT_PMP_ADDRESS_LENGTH: usize = 12;

#[derive(Debug, thiserror::Error)]
pub enum PcpError {
    #[error("nothing answered at {0}")]
    Silent(SocketAddr),
    #[error("the router refused: {0}")]
    Refused(&'static str),
    #[error("the router's answer is malformed")]
    Malformed,
    #[error("network: {0}")]
    Io(#[from] io::Error),
}

/// A UDP port the router forwards by PCP or NAT-PMP.
#[derive(Debug)]
pub struct PcpMapping {
    /// Where the other peer reaches this machine through the router.
    pub external: SocketAddr,
    /// The router's server.
    pub server: SocketAddr,
    how: How,
}

#[derive(Debug)]
enum How {
    /// A PCP mapping is known by its nonce and the internal address.
    Pcp {
        nonce: [u8; 12],
        local: SocketAddr,
    },
    NatPmp {
        internal_port: u16,
    },
}

/// Asks the server at `server` to forward UDP to `local` (this machine's address on the
/// network) for `lease`, on the same port outside if it can: by PCP, or by NAT-PMP if the
/// server speaks only that.
pub fn map(server: SocketAddr, local: SocketAddr, lease: Duration) -> Result<PcpMapping, PcpError> {
    let socket = open(server)?;
    let lifetime = u32::try_from(lease.as_secs()).unwrap_or(u32::MAX);
    let nonce = crate::stun::new_transaction();
    let answer = ask(&socket, server, &pcp_map(&nonce, local, lifetime))?;
    if answer[0] == PCP_VERSION {
        let external = read_pcp_map(&answer, &nonce)?;
        return Ok(PcpMapping {
            external,
            server,
            how: How::Pcp { nonce, local },
        });
    }
    let answer = ask(
        &socket,
        server,
        &nat_pmp_map(local.port(), local.port(), lifetime),
    )?;
    let port = read_nat_pmp_map(&answer)?;
    let ip = read_nat_pmp_address(&ask(&socket, server, &NAT_PMP_ADDRESS)?)?;
    Ok(PcpMapping {
        external: SocketAddr::new(ip.into(), port),
        server,
        how: How::NatPmp {
            internal_port: local.port(),
        },
    })
}

impl PcpMapping {
    pub fn protocol(&self) -> Protocol {
        match self.how {
            How::Pcp { .. } => Protocol::Pcp,
            How::NatPmp { .. } => Protocol::NatPmp,
        }
    }

    /// Asks the router to stop forwarding the port: the same request with no lifetime.
    pub fn remove(self) -> Result<(), PcpError> {
        let socket = open(self.server)?;
        match self.how {
            How::Pcp { nonce, local } => {
                let answer = ask(&socket, self.server, &pcp_map(&nonce, local, 0))?;
                read_pcp_map(&answer, &nonce).map(|_| ())
            }
            How::NatPmp { internal_port } => {
                let answer = ask(&socket, self.server, &nat_pmp_map(internal_port, 0, 0))?;
                read_nat_pmp_map(&answer).map(|_| ())
            }
        }
    }
}

fn open(server: SocketAddr) -> io::Result<UdpSocket> {
    let any = if server.is_ipv6() {
        SocketAddr::from(([0u16; 8], 0))
    } else {
        SocketAddr::from(([0, 0, 0, 0], 0))
    };
    UdpSocket::bind(any)
}

/// Sends `request` until the server answers it, at most [`WAITS`] times. Any answer to the
/// request's opcode counts, NAT-PMP's to a PCP request included.
fn ask(socket: &UdpSocket, server: SocketAddr, request: &[u8]) -> Result<Vec<u8>, PcpError> {
    let opcode = ANSWER | request[1];
    let mut buffer = [0; 1100];
    for wait in WAITS {
        socket.send_to(request, server)?;
        let until = Instant::now() + wait;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            socket.set_read_timeout(Some(left))?;
            match socket.recv_from(&mut buffer) {
                Ok((length, from))
                    if from == server
                        && length >= 4
                        && [PCP_VERSION, NAT_PMP_VERSION].contains(&buffer[0])
                        && buffer[1] == opcode =>
                {
                    return Ok(buffer[..length].to_vec());
                }
                Ok(_) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                // Windows hears that nothing listens there.
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    return Err(PcpError::Silent(server));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    Err(PcpError::Silent(server))
}

/// PCP's MAP request for UDP from `local`, asking for the same port outside on any of the
/// router's IPv4 addresses; a `lifetime` of 0 removes the mapping.
pub fn pcp_map(nonce: &[u8; 12], local: SocketAddr, lifetime: u32) -> [u8; PCP_MAP_LENGTH] {
    let mut packet = [0; PCP_MAP_LENGTH];
    packet[0] = PCP_VERSION;
    packet[1] = OPCODE_MAP;
    packet[4..8].copy_from_slice(&lifetime.to_be_bytes());
    packet[8..24].copy_from_slice(&as_ipv6(local.ip()).octets());
    packet[24..36].copy_from_slice(nonce);
    packet[36] = UDP;
    packet[40..42].copy_from_slice(&local.port().to_be_bytes());
    packet[42..44].copy_from_slice(&local.port().to_be_bytes());
    // `::ffff:0.0.0.0`: any IPv4 address (RFC 6887, section 11.1).
    packet[44..60].copy_from_slice(&Ipv4Addr::UNSPECIFIED.to_ipv6_mapped().octets());
    packet
}

/// PCP carries IPv4 addresses mapped into IPv6.
fn as_ipv6(ip: IpAddr) -> Ipv6Addr {
    match ip {
        IpAddr::V4(ip) => ip.to_ipv6_mapped(),
        IpAddr::V6(ip) => ip,
    }
}

/// The external address a PCP MAP answer for `nonce` assigns.
pub fn read_pcp_map(packet: &[u8], nonce: &[u8; 12]) -> Result<SocketAddr, PcpError> {
    if packet.len() < 4 || packet[0] != PCP_VERSION || packet[1] != ANSWER | OPCODE_MAP {
        return Err(PcpError::Malformed);
    }
    if packet[3] != 0 {
        return Err(PcpError::Refused(pcp_result(packet[3])));
    }
    if packet.len() < PCP_MAP_LENGTH || packet[24..36] != nonce[..] {
        return Err(PcpError::Malformed);
    }
    let port = u16::from_be_bytes([packet[42], packet[43]]);
    let ip =
        Ipv6Addr::from(<[u8; 16]>::try_from(&packet[44..60]).map_err(|_| PcpError::Malformed)?);
    let ip = ip.to_ipv4_mapped().map_or(IpAddr::V6(ip), IpAddr::V4);
    Ok(SocketAddr::new(ip, port))
}

/// PCP's result codes (RFC 6887, section 7.4).
fn pcp_result(code: u8) -> &'static str {
    match code {
        1 => "unsupported version",
        2 => "not authorized",
        3 => "malformed request",
        4 => "unsupported opcode",
        5 => "unsupported option",
        6 => "malformed option",
        7 => "network failure",
        8 => "no resources",
        9 => "unsupported protocol",
        10 => "user over quota",
        11 => "cannot provide that external address",
        12 => "this machine's address isn't the one the router sees",
        13 => "too many remote peers",
        _ => "unknown result",
    }
}

/// NAT-PMP's request for the router's external address.
pub const NAT_PMP_ADDRESS: [u8; 2] = [NAT_PMP_VERSION, OPCODE_ADDRESS];

/// NAT-PMP's request to map UDP `internal_port`, on `external_port` outside if it can; a
/// `lifetime` of 0 with no external port removes the mapping.
pub fn nat_pmp_map(internal_port: u16, external_port: u16, lifetime: u32) -> [u8; 12] {
    let mut packet = [0; 12];
    packet[0] = NAT_PMP_VERSION;
    packet[1] = OPCODE_MAP;
    packet[4..6].copy_from_slice(&internal_port.to_be_bytes());
    packet[6..8].copy_from_slice(&external_port.to_be_bytes());
    packet[8..12].copy_from_slice(&lifetime.to_be_bytes());
    packet
}

/// NAT-PMP's answer's result, after its version and opcode.
fn nat_pmp_result(packet: &[u8], opcode: u8, length: usize) -> Result<(), PcpError> {
    if packet.len() < 4 || packet[0] != NAT_PMP_VERSION || packet[1] != ANSWER | opcode {
        return Err(PcpError::Malformed);
    }
    let refused = match u16::from_be_bytes([packet[2], packet[3]]) {
        0 if packet.len() >= length => return Ok(()),
        0 => return Err(PcpError::Malformed),
        1 => "unsupported version",
        2 => "not authorized",
        3 => "network failure",
        4 => "out of resources",
        5 => "unsupported opcode",
        _ => "unknown result",
    };
    Err(PcpError::Refused(refused))
}

/// The external port a NAT-PMP mapping answer gives.
pub fn read_nat_pmp_map(packet: &[u8]) -> Result<u16, PcpError> {
    nat_pmp_result(packet, OPCODE_MAP, NAT_PMP_MAP_LENGTH)?;
    Ok(u16::from_be_bytes([packet[10], packet[11]]))
}

/// The external address a NAT-PMP address answer gives.
pub fn read_nat_pmp_address(packet: &[u8]) -> Result<Ipv4Addr, PcpError> {
    nat_pmp_result(packet, OPCODE_ADDRESS, NAT_PMP_ADDRESS_LENGTH)?;
    Ok(Ipv4Addr::new(packet[8], packet[9], packet[10], packet[11]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A router's server on this machine, answering as `answer` says until it hears
    /// nothing for a while.
    fn serve(answer: fn(&[u8]) -> Vec<u8>) -> SocketAddr {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let at = socket.local_addr().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        std::thread::spawn(move || {
            let mut buffer = [0; 1100];
            while let Ok((length, from)) = socket.recv_from(&mut buffer) {
                let _ = socket.send_to(&answer(&buffer[..length]), from);
            }
        });
        at
    }

    const EXTERNAL: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 9);

    /// A PCP server that maps every request on the next port up.
    fn pcp_server(request: &[u8]) -> Vec<u8> {
        let mut reply = request.to_vec();
        reply[1] |= ANSWER;
        reply[2..4].fill(0);
        let port = u16::from_be_bytes([request[42], request[43]]) + 1;
        reply[42..44].copy_from_slice(&port.to_be_bytes());
        reply[44..60].copy_from_slice(&EXTERNAL.to_ipv6_mapped().octets());
        reply
    }

    /// A router that speaks only NAT-PMP: PCP is answered with its version, unsupported.
    fn nat_pmp_server(request: &[u8]) -> Vec<u8> {
        let mut reply = vec![NAT_PMP_VERSION, ANSWER | request[1], 0, 0, 0, 0, 1, 0];
        match (request[0], request[1]) {
            (PCP_VERSION, _) => reply[3] = 1,
            (_, OPCODE_ADDRESS) => reply.extend(EXTERNAL.octets()),
            // The ports asked for, and the lifetime.
            (_, _) => reply.extend(&request[4..12]),
        }
        reply
    }

    #[test]
    fn a_pcp_router_maps_the_port() {
        let server = serve(pcp_server);
        let local = SocketAddr::from(([127, 0, 0, 1], 26450));
        let mapping = map(server, local, Duration::from_secs(60)).unwrap();
        assert_eq!(mapping.protocol(), Protocol::Pcp);
        assert_eq!(mapping.external, SocketAddr::from((EXTERNAL, 26451)));
        mapping.remove().unwrap();
    }

    #[test]
    fn a_nat_pmp_router_answering_pcp_with_its_version_is_asked_by_nat_pmp() {
        let server = serve(nat_pmp_server);
        let local = SocketAddr::from(([127, 0, 0, 1], 26450));
        let mapping = map(server, local, Duration::from_secs(60)).unwrap();
        assert_eq!(mapping.protocol(), Protocol::NatPmp);
        assert_eq!(mapping.external, SocketAddr::from((EXTERNAL, 26450)));
        mapping.remove().unwrap();
    }

    #[test]
    fn a_router_that_answers_nothing_is_silent() {
        // Bound and never read, so nothing answers but nothing refuses either.
        let quiet = UdpSocket::bind("127.0.0.1:0").unwrap();
        let server = quiet.local_addr().unwrap();
        let local = SocketAddr::from(([127, 0, 0, 1], 26450));
        let error = map(server, local, Duration::from_secs(60)).unwrap_err();
        assert!(
            matches!(error, PcpError::Silent(at) if at == server),
            "{error}"
        );
    }

    #[test]
    fn requests_are_laid_out_as_the_rfcs_have_them() {
        let nonce = [7; 12];
        let request = pcp_map(&nonce, SocketAddr::from(([192, 168, 1, 20], 26450)), 7200);
        assert_eq!(&request[..8], &[2, 1, 0, 0, 0, 0, 0x1C, 0x20]);
        assert_eq!(
            &request[8..24],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF, 192, 168, 1, 20]
        );
        assert_eq!(&request[36..44], &[17, 0, 0, 0, 0x67, 0x52, 0x67, 0x52]);
        assert_eq!(
            nat_pmp_map(26450, 26450, 7200),
            [0, 1, 0, 0, 0x67, 0x52, 0x67, 0x52, 0, 0, 0x1C, 0x20]
        );
        // A refusal says why; a stray answer for another nonce is malformed.
        let mut refused = request;
        refused[1] |= ANSWER;
        refused[3] = 2;
        assert!(matches!(
            read_pcp_map(&refused, &nonce),
            Err(PcpError::Refused("not authorized"))
        ));
        refused[3] = 0;
        assert!(matches!(
            read_pcp_map(&refused, &[8; 12]),
            Err(PcpError::Malformed)
        ));
        assert!(matches!(
            read_nat_pmp_address(&[0, 0x80, 0, 2, 0, 0, 0, 0]),
            Err(PcpError::Refused("not authorized"))
        ));
    }
}

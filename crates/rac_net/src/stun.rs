//! Learning this machine's public address from public STUN servers (RFC 5389's Binding
//! request, D8), asked over the session's own socket: a NAT maps each socket to its own
//! public port, so only an answer to that socket says where the other peer must send.
//!
//! STUN shares the port with the tagged packets of [`crate::link`], which tells its
//! answers apart by the magic cookie every STUN message carries.

use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hasher},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs},
    time::{Duration, Instant},
};

use crate::link::Link;

/// Public STUN servers, asked together; the first answer wins.
pub const SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun.cloudflare.com:3478",
];
/// How often the servers are asked again while none has answered.
pub const RESEND: Duration = Duration::from_millis(400);
/// How many times they are asked before giving up.
pub const TRIES: u32 = 6;

/// Every STUN message carries it at bytes 4 to 8 (RFC 5389, section 6).
const MAGIC_COOKIE: u32 = 0x2112_A442;
const HEADER: usize = 20;
const BINDING_REQUEST: u16 = 0x0001;
const BINDING_SUCCESS: u16 = 0x0101;
const MAPPED_ADDRESS: u16 = 0x0001;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// A STUN transaction's identifier, which its answer repeats.
pub type Transaction = [u8; 12];

/// A new transaction identifier. It only has to be unlikely to repeat, so the standard
/// library's randomly keyed hasher is enough.
pub fn new_transaction() -> Transaction {
    let mut id = [0; 12];
    for chunk in id.chunks_mut(8) {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(0);
        let bytes = hasher.finish().to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    id
}

/// A Binding request with no attributes.
pub fn request(transaction: &Transaction) -> [u8; HEADER] {
    let mut packet = [0; HEADER];
    packet[0..2].copy_from_slice(&BINDING_REQUEST.to_be_bytes());
    // Length 0: no attributes.
    packet[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    packet[8..20].copy_from_slice(transaction);
    packet
}

/// Whether `packet` is a STUN message: the top two bits clear, the magic cookie in place,
/// and a length that matches.
pub fn is_stun(packet: &[u8]) -> bool {
    packet.len() >= HEADER
        && packet[0] & 0xC0 == 0
        && packet[4..8] == MAGIC_COOKIE.to_be_bytes()
        && usize::from(u16::from_be_bytes([packet[2], packet[3]])) == packet.len() - HEADER
}

/// The address a Binding success answer for `transaction` reports, from its
/// XOR-MAPPED-ADDRESS, or its MAPPED-ADDRESS from an old server.
pub fn parse_answer(packet: &[u8], transaction: &Transaction) -> Option<SocketAddr> {
    if !is_stun(packet)
        || u16::from_be_bytes([packet[0], packet[1]]) != BINDING_SUCCESS
        || packet[8..20] != transaction[..]
    {
        return None;
    }
    let mut attributes = &packet[HEADER..];
    let mut mapped = None;
    while attributes.len() >= 4 {
        let kind = u16::from_be_bytes([attributes[0], attributes[1]]);
        let length = usize::from(u16::from_be_bytes([attributes[2], attributes[3]]));
        let value = attributes.get(4..4 + length)?;
        match kind {
            XOR_MAPPED_ADDRESS => return address(value, Some(&packet[4..20])),
            MAPPED_ADDRESS => mapped = address(value, None),
            _ => {}
        }
        // Attributes are padded to four bytes.
        let padded = (4 + length).next_multiple_of(4);
        attributes = attributes.get(padded..).unwrap_or_default();
    }
    mapped
}

/// An address attribute's value, unmasked with the cookie and transaction for the XOR
/// form.
fn address(value: &[u8], xor: Option<&[u8]>) -> Option<SocketAddr> {
    let mask = |i: usize| xor.map_or(0, |key| key[i]);
    let port = u16::from_be_bytes([value.get(2)? ^ mask(0), value.get(3)? ^ mask(1)]);
    let ip = match value.get(1)? {
        0x01 => {
            let mut octets = [0; 4];
            for (i, octet) in octets.iter_mut().enumerate() {
                *octet = value.get(4 + i)? ^ mask(i);
            }
            IpAddr::V4(Ipv4Addr::from(octets))
        }
        0x02 => {
            let mut octets = [0; 16];
            for (i, octet) in octets.iter_mut().enumerate() {
                *octet = value.get(4 + i)? ^ mask(i);
            }
            IpAddr::V6(Ipv6Addr::from(octets))
        }
        _ => return None,
    };
    Some(SocketAddr::new(ip, port))
}

/// A Binding success answer telling the asker it was seen from `seen`, as a server writes
/// it: for tests.
pub fn answer(transaction: &Transaction, seen: SocketAddr) -> Vec<u8> {
    let mut key = [0; 16];
    key[0..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    key[4..16].copy_from_slice(transaction);
    let (family, ip) = match seen.ip() {
        IpAddr::V4(ip) => (0x01, ip.octets().to_vec()),
        IpAddr::V6(ip) => (0x02, ip.octets().to_vec()),
    };
    let mut value = vec![0, family];
    value.extend(
        seen.port()
            .to_be_bytes()
            .iter()
            .zip(&key)
            .map(|(byte, k)| byte ^ k),
    );
    value.extend(ip.iter().zip(&key).map(|(byte, k)| byte ^ k));
    let mut packet = Vec::with_capacity(HEADER + 4 + value.len());
    packet.extend_from_slice(&BINDING_SUCCESS.to_be_bytes());
    packet.extend_from_slice(&((4 + value.len()) as u16).to_be_bytes());
    packet.extend_from_slice(&key);
    packet.extend_from_slice(&XOR_MAPPED_ADDRESS.to_be_bytes());
    packet.extend_from_slice(&(value.len() as u16).to_be_bytes());
    packet.extend_from_slice(&value);
    packet
}

/// Looks the servers up, keeping those of `family`'s kind (IPv4 or IPv6, as the socket
/// is). It waits on the system's resolver, so call it off the frame loop.
pub fn resolve(servers: &[&str], ipv6: bool) -> Vec<SocketAddr> {
    servers
        .iter()
        .filter_map(|server| server.to_socket_addrs().ok())
        .flatten()
        .filter(|address| address.is_ipv6() == ipv6)
        .collect()
}

/// Asking the servers for this socket's public address, until one answers or the tries
/// run out.
pub struct StunQuery {
    servers: Vec<SocketAddr>,
    transaction: Transaction,
    last_sent: Option<Instant>,
    tries: u32,
    found: Option<SocketAddr>,
}

impl StunQuery {
    pub fn new(servers: Vec<SocketAddr>) -> Self {
        Self {
            servers,
            transaction: new_transaction(),
            last_sent: None,
            tries: 0,
            found: None,
        }
    }

    /// Takes the servers' answers from `link`, and asks again if it is time to. Returns
    /// the public address once a server has said it.
    pub fn poll(&mut self, link: &Link, now: Instant) -> Option<SocketAddr> {
        for (from, packet) in link.take_stun() {
            if self.found.is_none() && self.servers.contains(&from) {
                self.found = parse_answer(&packet, &self.transaction);
            }
        }
        if self.found.is_some() {
            return self.found;
        }
        let due = self
            .last_sent
            .is_none_or(|last| now.duration_since(last) >= RESEND);
        if due && self.tries < TRIES {
            self.tries += 1;
            self.last_sent = Some(now);
            let request = request(&self.transaction);
            for server in &self.servers {
                link.send_raw(*server, &request);
            }
        }
        None
    }

    /// No server answered in time, or there were none to ask.
    pub fn gave_up(&self, now: Instant) -> bool {
        self.found.is_none()
            && (self.servers.is_empty()
                || (self.tries >= TRIES
                    && self
                        .last_sent
                        .is_none_or(|last| now.duration_since(last) >= RESEND)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_gives_back_the_address_it_was_written_with() {
        let transaction = new_transaction();
        for seen in [
            SocketAddr::from(([203, 0, 113, 7], 40_123)),
            SocketAddr::from(([0x2001, 0xdb8, 0, 0, 0, 0, 0, 0x42], 51_000)),
        ] {
            let packet = answer(&transaction, seen);
            assert!(is_stun(&packet));
            assert_eq!(parse_answer(&packet, &transaction), Some(seen));
            assert_eq!(parse_answer(&packet, &new_transaction()), None);
        }
    }

    #[test]
    fn rfc_5769s_sample_ipv4_answer_is_read() {
        // RFC 5769, section 2.2: a Binding success answer with SOFTWARE,
        // XOR-MAPPED-ADDRESS (192.0.2.1:32853), MESSAGE-INTEGRITY and FINGERPRINT.
        let packet: [u8; 80] = [
            0x01, 0x01, 0x00, 0x3c, 0x21, 0x12, 0xa4, 0x42, 0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34,
            0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae, 0x80, 0x22, 0x00, 0x0b, 0x74, 0x65, 0x73, 0x74,
            0x20, 0x76, 0x65, 0x63, 0x74, 0x6f, 0x72, 0x20, 0x00, 0x20, 0x00, 0x08, 0x00, 0x01,
            0xa1, 0x47, 0xe1, 0x12, 0xa6, 0x43, 0x00, 0x08, 0x00, 0x14, 0x2b, 0x91, 0xf5, 0x99,
            0xfd, 0x9e, 0x90, 0xc3, 0x8c, 0x74, 0x89, 0xf9, 0x2a, 0xf9, 0xba, 0x53, 0xf0, 0x6b,
            0xe7, 0xd7, 0x80, 0x28, 0x00, 0x04, 0xc0, 0x7d, 0x4c, 0x96,
        ];
        let transaction: Transaction = packet[8..20].try_into().unwrap();
        assert_eq!(
            parse_answer(&packet, &transaction),
            Some(SocketAddr::from(([192, 0, 2, 1], 32853)))
        );
    }

    #[test]
    fn a_request_is_stun_and_the_sessions_packets_are_not() {
        let transaction = new_transaction();
        assert!(is_stun(&request(&transaction)));
        // A tagged packet of the handshake's or ggrs's.
        assert!(!is_stun(&[
            2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]));
        assert!(!is_stun(&[1, 0x34, 0x12, 0, 0, 0, 0]));
        assert_ne!(new_transaction(), transaction);
    }
}

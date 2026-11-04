//! Join codes (D8): the addresses a peer can be reached at, best first, as a short text
//! the players swap themselves, since no server of ours introduces them. A code holds a
//! public address (from STUN or a mapped port) and the machine's own on its network, so
//! peers on one network meet directly.
//!
//! The text is Crockford's base32 in groups of four, read in either case and with the
//! letters O, I and L taken as the digits they look like. A check of 16 bits catches
//! typing mistakes.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};

/// The code's own version: a code from another version is refused.
const VERSION: u8 = 1;
/// The most addresses a code carries.
pub const MOST_ADDRESSES: usize = 4;
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const GROUP: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CodeError {
    #[error("{0:?} isn't a letter or digit of a join code")]
    Character(char),
    #[error("the join code is mistyped: its check doesn't match")]
    Check,
    #[error("the join code is from another version of the game")]
    Version,
    #[error("the join code is cut short or has extra letters")]
    Length,
    #[error("the join code holds no address")]
    Empty,
}

/// A peer's addresses, best first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinCode(pub Vec<SocketAddr>);

impl JoinCode {
    /// The code as text, `XXXX-XXXX-…`. Addresses past [`MOST_ADDRESSES`] are left out.
    pub fn encode(&self) -> String {
        let mut bytes = vec![VERSION];
        for address in self.0.iter().take(MOST_ADDRESSES) {
            match address.ip() {
                IpAddr::V4(ip) => {
                    bytes.push(4);
                    bytes.extend_from_slice(&ip.octets());
                }
                IpAddr::V6(ip) => {
                    bytes.push(6);
                    bytes.extend_from_slice(&ip.octets());
                }
            }
            bytes.extend_from_slice(&address.port().to_be_bytes());
        }
        bytes.extend_from_slice(&check(&bytes).to_be_bytes());
        let text = to_base32(&bytes);
        text.as_bytes()
            .chunks(GROUP)
            .map(|group| std::str::from_utf8(group).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// Reads a code typed or pasted in: case, dashes and spaces don't matter.
    pub fn decode(text: &str) -> Result<Self, CodeError> {
        let bytes = from_base32(text)?;
        let (body, sum) = bytes.split_last_chunk::<2>().ok_or(CodeError::Length)?;
        if check(body) != u16::from_be_bytes(*sum) {
            return Err(CodeError::Check);
        }
        let (&version, mut rest) = body.split_first().ok_or(CodeError::Length)?;
        if version != VERSION {
            return Err(CodeError::Version);
        }
        let mut addresses = Vec::new();
        while let Some((&kind, after)) = rest.split_first() {
            let length = match kind {
                4 => 4,
                6 => 16,
                _ => return Err(CodeError::Length),
            };
            let (ip, after) = after.split_at_checked(length).ok_or(CodeError::Length)?;
            let (port, after) = after.split_first_chunk::<2>().ok_or(CodeError::Length)?;
            let ip = match kind {
                4 => IpAddr::V4(Ipv4Addr::from(<[u8; 4]>::try_from(ip).unwrap_or_default())),
                _ => IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(ip).unwrap_or_default())),
            };
            addresses.push(SocketAddr::new(ip, u16::from_be_bytes(*port)));
            rest = after;
        }
        if addresses.is_empty() {
            return Err(CodeError::Empty);
        }
        if addresses.len() > MOST_ADDRESSES {
            return Err(CodeError::Length);
        }
        Ok(Self(addresses))
    }
}

/// This machine's address on its own network for `port`: the one its default route
/// leaves from, which pointing a UDP socket at a public address shows without sending
/// anything.
pub fn local_address(port: u16, ipv6: bool) -> Option<SocketAddr> {
    let (bind, public) = if ipv6 {
        (
            SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0)),
            SocketAddr::from(([0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888], 53)),
        )
    } else {
        (
            SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
            SocketAddr::from(([8, 8, 8, 8], 53)),
        )
    };
    let socket = UdpSocket::bind(bind).ok()?;
    socket.connect(public).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then_some(SocketAddr::new(ip, port))
}

/// FNV-1a folded to 16 bits: enough to catch a mistyped letter.
fn check(bytes: &[u8]) -> u16 {
    let mut hash: u32 = 0x811C_9DC5;
    for &byte in bytes {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash ^ (hash >> 16)) as u16
}

fn to_base32(bytes: &[u8]) -> String {
    let mut text = String::new();
    let mut bits = 0u32;
    let mut held = 0;
    for &byte in bytes {
        bits = (bits << 8) | u32::from(byte);
        held += 8;
        while held >= 5 {
            held -= 5;
            text.push(ALPHABET[((bits >> held) & 31) as usize] as char);
        }
    }
    if held > 0 {
        text.push(ALPHABET[((bits << (5 - held)) & 31) as usize] as char);
    }
    text
}

fn from_base32(text: &str) -> Result<Vec<u8>, CodeError> {
    let mut bytes = Vec::new();
    let mut bits = 0u32;
    let mut held = 0;
    for c in text.chars() {
        if c == '-' || c.is_whitespace() {
            continue;
        }
        let value = match c.to_ascii_uppercase() {
            'O' => 0,
            'I' | 'L' => 1,
            upper => ALPHABET
                .iter()
                .position(|&letter| letter as char == upper)
                .ok_or(CodeError::Character(c))? as u32,
        };
        bits = (bits << 5) | value;
        held += 5;
        if held >= 8 {
            held -= 8;
            bytes.push((bits >> held) as u8);
        }
    }
    // What is left over is the padding of the last letter, under a byte.
    if bits & ((1 << held) - 1) != 0 {
        return Err(CodeError::Length);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code() -> JoinCode {
        JoinCode(vec![
            SocketAddr::from(([203, 0, 113, 7], 40_123)),
            SocketAddr::from(([192, 168, 1, 20], 26_450)),
            SocketAddr::from(([0x2001, 0xdb8, 0, 0, 0, 0, 0, 0x42], 26_450)),
        ])
    }

    #[test]
    fn a_code_reads_back_as_its_addresses() {
        let text = code().encode();
        assert_eq!(JoinCode::decode(&text), Ok(code()));
        // An IPv4 public and LAN address make a code of 28 letters.
        let short = JoinCode(code().0[..2].to_vec()).encode();
        assert_eq!(short.replace('-', "").len(), 28, "{short}");
    }

    #[test]
    fn it_reads_in_either_case_without_dashes_and_with_look_alike_letters() {
        let text = code().encode();
        let loose = text
            .replace('-', " ")
            .to_lowercase()
            .replace('0', "o")
            .replace('1', "l");
        assert_eq!(JoinCode::decode(&loose), Ok(code()));
    }

    #[test]
    fn a_mistyped_letter_is_caught() {
        let text = code().encode();
        for at in [0, 5, text.len() - 1] {
            let mut typo: Vec<char> = text.chars().collect();
            if typo[at] == '-' {
                continue;
            }
            typo[at] = if typo[at] == 'Z' { 'Y' } else { 'Z' };
            let typo: String = typo.into_iter().collect();
            assert!(JoinCode::decode(&typo).is_err(), "{typo}");
        }
        assert_eq!(
            JoinCode::decode("hello world!"),
            Err(CodeError::Character('!'))
        );
        assert!(JoinCode::decode("").is_err());
        assert!(JoinCode::decode(&text[..text.len() - 5]).is_err());
    }
}

//! How a session's packets travel: over UDP ([`UdpLink`]), IPv4 and IPv6 at once, or
//! through memory with chosen delays and losses for tests ([`MemoryNet`]).
//!
//! Each datagram starts with a tag byte, so `ggrs`'s own packets and the handshake's
//! ([`crate::handshake`]) share one port. STUN's ([`crate::stun`]) come untagged, and are
//! told apart by the magic cookie every STUN message carries.

use bevy::log::{debug, warn};
use ggrs::{Message, NonBlockingSocket};
use std::{
    collections::{BTreeSet, VecDeque},
    io,
    net::{SocketAddr, UdpSocket},
    sync::{Arc, Mutex, MutexGuard},
};

const GGRS_TAG: u8 = 1;
const CONTROL_TAG: u8 = 2;
/// Larger than any packet either side sends; `ggrs` keeps its own under about 500 bytes.
const MAX_PACKET: usize = 4096;

/// Something packets go out of and come in through: a socket, or a test's memory.
pub trait Transport: Send + Sync + 'static {
    fn send(&mut self, to: SocketAddr, bytes: &[u8]);
    /// Every datagram that has arrived since the last call.
    fn receive(&mut self) -> Vec<(SocketAddr, Vec<u8>)>;
}

/// A link to the other peers, shared between the handshake and the `ggrs` session, which
/// each take their own packets from it. Cloning it gives another handle to the same link.
#[derive(Clone)]
pub struct Link {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    transport: Box<dyn Transport>,
    ggrs: Vec<(SocketAddr, Message)>,
    control: VecDeque<(SocketAddr, Vec<u8>)>,
    stun: Vec<(SocketAddr, Vec<u8>)>,
}

impl Link {
    pub fn new(transport: impl Transport) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                transport: Box::new(transport),
                ggrs: Vec::new(),
                control: VecDeque::new(),
                stun: Vec::new(),
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic elsewhere can't leave the queues half-written, so carry on.
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Sends one of the handshake's packets.
    pub fn send_control(&self, to: SocketAddr, bytes: &[u8]) {
        let mut packet = Vec::with_capacity(bytes.len() + 1);
        packet.push(CONTROL_TAG);
        packet.extend_from_slice(bytes);
        self.lock().transport.send(to, &packet);
    }

    /// The handshake's packets that have arrived.
    pub fn take_control(&self) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut inner = self.lock();
        inner.pump();
        inner.control.drain(..).collect()
    }

    /// Sends `bytes` as they are, untagged: a STUN request.
    pub fn send_raw(&self, to: SocketAddr, bytes: &[u8]) {
        self.lock().transport.send(to, bytes);
    }

    /// The STUN messages that have arrived.
    pub fn take_stun(&self) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut inner = self.lock();
        inner.pump();
        std::mem::take(&mut inner.stun)
    }
}

impl Inner {
    /// Sorts what has arrived into the queues, dropping anything malformed.
    fn pump(&mut self) {
        for (from, packet) in self.transport.receive() {
            // A STUN answer's first byte is 1, `ggrs`'s tag, but no tagged packet carries
            // STUN's cookie where STUN does.
            if crate::stun::is_stun(&packet) {
                self.stun.push((from, packet));
                continue;
            }
            match packet.split_first() {
                Some((&GGRS_TAG, body)) => match bincode::deserialize::<Message>(body) {
                    Ok(message) => self.ggrs.push((from, message)),
                    Err(error) => warn!("dropped a malformed packet from {from}: {error}"),
                },
                Some((&CONTROL_TAG, body)) => self.control.push_back((from, body.to_vec())),
                _ => {}
            }
        }
    }
}

impl NonBlockingSocket<SocketAddr> for Link {
    fn send_to(&mut self, message: &Message, to: &SocketAddr) {
        let mut packet = vec![GGRS_TAG];
        // Serialising into a `Vec` can't fail.
        if bincode::serialize_into(&mut packet, message).is_ok() {
            self.lock().transport.send(*to, &packet);
        }
    }

    fn receive_all_messages(&mut self) -> Vec<(SocketAddr, Message)> {
        let mut inner = self.lock();
        inner.pump();
        std::mem::take(&mut inner.ggrs)
    }
}

/// Non-blocking UDP sockets, one for IPv4 and one for IPv6, either of which may be
/// missing: packets go out of the one for their address's family.
pub struct UdpLink {
    v4: Option<UdpSocket>,
    v6: Option<UdpSocket>,
    buffer: Box<[u8; MAX_PACKET]>,
}

/// A non-blocking socket bound to `address`. An IPv6 one takes IPv6 alone, everywhere
/// (systems differ in what they do unasked), so it leaves the IPv4 port to its own socket.
fn bind_socket(address: SocketAddr) -> io::Result<UdpSocket> {
    let socket = if address.is_ipv6() {
        let socket = socket2::Socket::new(
            socket2::Domain::IPV6,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )?;
        socket.set_only_v6(true)?;
        socket.bind(&address.into())?;
        socket.into()
    } else {
        UdpSocket::bind(address)?
    };
    socket.set_nonblocking(true)?;
    Ok(socket)
}

impl UdpLink {
    /// Binds `address` alone: `0.0.0.0:port` to host on a port, port 0 for any free one.
    pub fn bind(address: SocketAddr) -> io::Result<Self> {
        let socket = Some(bind_socket(address)?);
        let (v4, v6) = if address.is_ipv6() {
            (None, socket)
        } else {
            (socket, None)
        };
        Ok(Self {
            v4,
            v6,
            buffer: Box::new([0; MAX_PACKET]),
        })
    }

    /// Binds UDP `port` on IPv4 and, if `ipv6`, on IPv6 as well, so peers reach this
    /// machine by either: port 0 takes a free one, which IPv6 then takes too if it can. An
    /// IPv6 socket that can't be bound is left out; it is IPv4's that must be.
    pub fn bind_both(port: u16, ipv6: bool) -> io::Result<Self> {
        let mut link = Self::bind(SocketAddr::from(([0, 0, 0, 0], port)))?;
        if ipv6 {
            let port = link.local_addrs().first().map_or(port, SocketAddr::port);
            let any = |port| SocketAddr::from(([0u16; 8], port));
            link.v6 = bind_socket(any(port))
                .or_else(|_| bind_socket(any(0)))
                .inspect_err(|error| warn!("couldn't open IPv6 UDP: {error}; IPv4 only"))
                .ok();
        }
        Ok(link)
    }

    /// Where each socket is bound, IPv4's first.
    pub fn local_addrs(&self) -> Vec<SocketAddr> {
        [&self.v4, &self.v6]
            .into_iter()
            .flatten()
            .filter_map(|socket| socket.local_addr().ok())
            .collect()
    }
}

impl Transport for UdpLink {
    fn send(&mut self, to: SocketAddr, bytes: &[u8]) {
        let socket = if to.is_ipv6() { &self.v6 } else { &self.v4 };
        let Some(socket) = socket else {
            debug!("no socket of {to}'s family to send to it from");
            return;
        };
        // UDP may drop anything; the protocols above resend what matters.
        match socket.send_to(bytes, to) {
            Ok(_) => {}
            // Every address in a join code is tried, and some lead nowhere from here.
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NetworkUnreachable | io::ErrorKind::HostUnreachable
                ) =>
            {
                debug!("couldn't send to {to}: {error}");
            }
            Err(error) => warn!("couldn't send to {to}: {error}"),
        }
    }

    fn receive(&mut self) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut received = Vec::new();
        for socket in [&self.v4, &self.v6].into_iter().flatten() {
            loop {
                match socket.recv_from(&mut self.buffer[..]) {
                    Ok((length, from)) => received.push((from, self.buffer[..length].to_vec())),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    // Windows reports a peer's closed port as a reset on the next receive.
                    Err(error) if error.kind() == io::ErrorKind::ConnectionReset => continue,
                    Err(error) => {
                        warn!("couldn't receive: {error}");
                        break;
                    }
                }
            }
        }
        received
    }
}

/// A network in memory for tests: packets arrive `latency` steps after they are sent, and
/// each is lost with probability `loss`, decided by a seeded generator so a test runs the
/// same every time. Endpoints may sit behind NATs ([`MemoryNet::behind_nat`]).
#[derive(Clone)]
pub struct MemoryNet {
    inner: Arc<Mutex<Memory>>,
}

struct Memory {
    step: u64,
    latency: u64,
    loss: f32,
    random: u64,
    in_flight: Vec<(u64, SocketAddr, SocketAddr, Vec<u8>)>,
    nats: Vec<Nat>,
}

/// A port-restricted cone NAT in front of one endpoint, the commonest kind at home: what
/// the endpoint sends leaves from `public`, and only what comes back from an address it
/// has sent to gets in. Its `private` address can't be reached from outside.
struct Nat {
    private: SocketAddr,
    public: SocketAddr,
    sent_to: BTreeSet<SocketAddr>,
}

impl MemoryNet {
    pub fn new(latency: u64, loss: f32, seed: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Memory {
                step: 0,
                latency,
                loss,
                random: seed | 1,
                in_flight: Vec::new(),
                nats: Vec::new(),
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Memory> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// The end of the network at `address`.
    pub fn endpoint(&self, address: SocketAddr) -> MemoryLink {
        MemoryLink {
            net: self.clone(),
            address,
        }
    }

    /// Moves time on one step, delivering what is due.
    pub fn advance(&self) {
        self.lock().step += 1;
    }

    /// Puts the endpoint at `private` behind a NAT that shows it as `public`.
    pub fn behind_nat(&self, private: SocketAddr, public: SocketAddr) {
        self.lock().nats.push(Nat {
            private,
            public,
            sent_to: BTreeSet::new(),
        });
    }

    /// Changes how the network behaves from now on.
    pub fn set(&self, latency: u64, loss: f32) {
        let mut memory = self.lock();
        memory.latency = latency;
        memory.loss = loss;
    }
}

/// One end of a [`MemoryNet`].
pub struct MemoryLink {
    net: MemoryNet,
    address: SocketAddr,
}

impl Transport for MemoryLink {
    fn send(&mut self, to: SocketAddr, bytes: &[u8]) {
        let mut memory = self.net.lock();
        // xorshift64: enough for choosing losses.
        memory.random ^= memory.random << 13;
        memory.random ^= memory.random >> 7;
        memory.random ^= memory.random << 17;
        let roll = (memory.random >> 40) as f32 / (1u64 << 24) as f32;
        if roll < memory.loss {
            return;
        }
        let due = memory.step + memory.latency;
        let from = match memory
            .nats
            .iter_mut()
            .find(|nat| nat.private == self.address)
        {
            Some(nat) => {
                nat.sent_to.insert(to);
                nat.public
            }
            None => self.address,
        };
        memory.in_flight.push((due, from, to, bytes.to_vec()));
    }

    fn receive(&mut self) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut memory = self.net.lock();
        let Memory {
            step,
            in_flight,
            nats,
            ..
        } = &mut *memory;
        let nat = nats.iter().find(|nat| nat.private == self.address);
        let mut arrived = Vec::new();
        in_flight.retain(|(due, from, to, bytes)| {
            if *due > *step {
                return true;
            }
            let lets_in = match nat {
                Some(nat) => *to == nat.public && nat.sent_to.contains(from),
                None => *to == self.address,
            };
            if lets_in {
                arrived.push((*from, bytes.clone()));
            }
            // A NAT drops what it doesn't let in.
            !(lets_in || nat.is_some_and(|nat| *to == nat.public || *to == nat.private))
        });
        arrived
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What arrives at `socket` within a second.
    fn heard(socket: &UdpSocket) -> Vec<u8> {
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
            .unwrap();
        let mut buffer = [0; 64];
        let (length, _) = socket.recv_from(&mut buffer).unwrap();
        buffer[..length].to_vec()
    }

    #[test]
    fn a_link_on_both_families_talks_to_peers_of_each() {
        let mut link = UdpLink::bind_both(0, true).unwrap();
        let bound = link.local_addrs();
        let loopback = |at: &SocketAddr| match at {
            SocketAddr::V4(at) => SocketAddr::from(([127, 0, 0, 1], at.port())),
            SocketAddr::V6(at) => SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], at.port())),
        };
        // A machine without IPv6 binds IPv4 alone.
        assert!(bound[0].is_ipv4());
        assert!(bound.len() == 1 || bound[1].is_ipv6());
        // The IPv6 socket takes IPv6 alone, so the IPv4 port is IPv4's.
        let peers: Vec<UdpSocket> = bound
            .iter()
            .map(|at| UdpSocket::bind(SocketAddr::new(loopback(at).ip(), 0)).unwrap())
            .collect();
        for (peer, at) in peers.iter().zip(&bound) {
            peer.send_to(&[at.is_ipv6() as u8], loopback(at)).unwrap();
        }
        let mut received = Vec::new();
        let start = std::time::Instant::now();
        while received.len() < peers.len() && start.elapsed().as_secs() < 2 {
            received.extend(link.receive());
        }
        assert_eq!(received.len(), peers.len());
        for (from, bytes) in received {
            assert_eq!(bytes, [from.is_ipv6() as u8]);
            link.send(from, b"back");
        }
        for peer in &peers {
            assert_eq!(heard(peer), b"back");
        }
    }
}

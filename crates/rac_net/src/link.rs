//! How a session's packets travel: over UDP ([`UdpLink`]), or through memory with chosen
//! delays and losses for tests ([`MemoryNet`]).
//!
//! Each datagram starts with a tag byte, so `ggrs`'s own packets and the handshake's
//! ([`crate::handshake`]) share one port. STUN's ([`crate::stun`]) come untagged, and are
//! told apart by the magic cookie every STUN message carries.

use bevy::log::warn;
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

/// A non-blocking UDP socket.
pub struct UdpLink {
    socket: UdpSocket,
    buffer: Box<[u8; MAX_PACKET]>,
}

impl UdpLink {
    /// Binds `address`: `0.0.0.0:port` to host on a port, port 0 for any free one.
    pub fn bind(address: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(address)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            buffer: Box::new([0; MAX_PACKET]),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

impl Transport for UdpLink {
    fn send(&mut self, to: SocketAddr, bytes: &[u8]) {
        // UDP may drop anything; the protocols above resend what matters.
        if let Err(error) = self.socket.send_to(bytes, to) {
            warn!("couldn't send to {to}: {error}");
        }
    }

    fn receive(&mut self) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut received = Vec::new();
        loop {
            match self.socket.recv_from(&mut self.buffer[..]) {
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

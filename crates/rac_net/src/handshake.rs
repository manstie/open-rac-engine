//! Agreeing to play before the session starts: the joiner says hello with its build and
//! the rules it plays by, and the host welcomes it into a slot or says why not
//! (PLAN.md section 4.2, "Safety nets").
//!
//! Both sides keep answering: the joiner says hello every [`RESEND`] until it is welcomed,
//! and the host welcomes it again whenever it does, so lost packets only cost time.
//!
//! Through NATs (D8) the joiner says hello at every address in the host's join code at
//! once, and a host given the joiner's code sends it punches as often: each NAT lets the
//! other's packets in once its own side has sent to them.

use bincode::Options;
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::RandomState,
    hash::BuildHasher,
    net::SocketAddr,
    time::{Duration, Instant},
};

use crate::link::Link;

/// The handshake's own version; peers with another can't play together.
pub const PROTOCOL: u16 = 2;
/// How often the joiner says hello until it is answered.
pub const RESEND: Duration = Duration::from_millis(250);
/// The most a handshake packet may be, so a stray one can't make us allocate much.
const LIMIT: u64 = 1200;

/// What both peers must agree on to simulate alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terms {
    /// The app and its version: peers simulate alike only on the same build.
    pub build: String,
    /// A digest of every setting that changes the simulation, which the app computes (the
    /// movement profile, the guns, their level).
    pub rules: u64,
}

/// Why a host turned a joiner away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum Refusal {
    #[error("the host speaks handshake version {host}, this game {ours}")]
    Protocol { host: u16, ours: u16 },
    #[error("the host runs {host}, this game {ours}")]
    Build { host: String, ours: String },
    #[error("the host plays with other settings (movement, guns or gun level)")]
    Rules,
    #[error("the host is already playing with someone else")]
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Packet {
    Hello {
        protocol: u16,
        terms: Terms,
        /// The joiner's own choices for its player, which the app reads (its camera).
        setup: Vec<u8>,
        /// Chosen at random by the joiner, so its hellos reaching the host by two paths
        /// (its network's and its NAT's) are known as one joiner's.
        joiner: u64,
    },
    Welcome {
        slot: u8,
        players: u8,
        setup: Vec<u8>,
    },
    Refused(Refusal),
    /// Nothing to answer: it opens the sender's NAT to the receiver.
    Punch,
}

fn options() -> impl Options {
    bincode::DefaultOptions::new().with_limit(LIMIT)
}

fn send(link: &Link, to: SocketAddr, packet: &Packet) {
    if let Ok(bytes) = options().serialize(packet) {
        link.send_control(to, &bytes);
    }
}

fn received(link: &Link) -> Vec<(SocketAddr, Packet)> {
    link.take_control()
        .into_iter()
        .filter_map(|(from, bytes)| Some((from, options().deserialize(&bytes).ok()?)))
        .collect()
}

/// Who the session is with, once both sides agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agreed {
    pub peer: SocketAddr,
    /// This machine's player slot.
    pub slot: usize,
    pub players: usize,
    /// The other peer's choices for its player.
    pub peer_setup: Vec<u8>,
}

/// The host's side: waits for a joiner and welcomes the first whose terms match.
pub struct Host {
    link: Link,
    terms: Terms,
    setup: Vec<u8>,
    /// The guest's address, as its first hello came, and its id.
    guest: Option<(SocketAddr, u64)>,
    /// The addresses in the joiner's code, punched until it arrives.
    punch: Vec<SocketAddr>,
    last_punch: Option<Instant>,
}

impl Host {
    pub fn new(link: Link, terms: Terms, setup: Vec<u8>) -> Self {
        Self {
            link,
            terms,
            setup,
            guest: None,
            punch: Vec::new(),
            last_punch: None,
        }
    }

    /// Punches through this machine's NAT to the joiner at `addresses`, the ones in its
    /// join code, until it arrives.
    pub fn punch_to(&mut self, addresses: Vec<SocketAddr>) {
        self.punch = addresses;
        self.last_punch = None;
    }

    /// Answers whatever has arrived, and punches if it is time to; returns the guest once
    /// one is welcomed. Keep calling it after that, so a guest whose welcome was lost is
    /// welcomed again.
    pub fn poll(&mut self) -> Option<Agreed> {
        let now = Instant::now();
        if self.guest.is_none()
            && self
                .last_punch
                .is_none_or(|last| now.duration_since(last) >= RESEND)
        {
            self.last_punch = Some(now);
            for &address in &self.punch {
                send(&self.link, address, &Packet::Punch);
            }
        }
        let mut welcomed = None;
        for (from, packet) in received(&self.link) {
            let Packet::Hello {
                protocol,
                terms,
                setup,
                joiner,
            } = packet
            else {
                continue;
            };
            // The guest by another path: answered where it was first heard.
            let from = match self.guest {
                Some((guest, id)) if id == joiner => guest,
                _ => from,
            };
            let refusal = if protocol != PROTOCOL {
                Some(Refusal::Protocol {
                    host: PROTOCOL,
                    ours: protocol,
                })
            } else if terms.build != self.terms.build {
                Some(Refusal::Build {
                    host: self.terms.build.clone(),
                    ours: terms.build,
                })
            } else if terms.rules != self.terms.rules {
                Some(Refusal::Rules)
            } else if self.guest.as_ref().is_some_and(|(guest, _)| *guest != from) {
                Some(Refusal::Full)
            } else {
                None
            };
            if let Some(refusal) = refusal {
                send(&self.link, from, &Packet::Refused(refusal));
                continue;
            }
            let first = self.guest.is_none();
            self.guest = Some((from, joiner));
            send(
                &self.link,
                from,
                &Packet::Welcome {
                    slot: 1,
                    players: 2,
                    setup: self.setup.clone(),
                },
            );
            if first {
                welcomed = Some(Agreed {
                    peer: from,
                    slot: 0,
                    players: 2,
                    peer_setup: setup,
                });
            }
        }
        welcomed
    }
}

/// What a joiner hears back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Welcomed(Agreed),
    Refused(Refusal),
}

/// The joiner's side: says hello to the host until it answers.
pub struct Joiner {
    link: Link,
    /// Every address the host may be reached at.
    hosts: Vec<SocketAddr>,
    terms: Terms,
    setup: Vec<u8>,
    id: u64,
    last_hello: Option<Instant>,
}

impl Joiner {
    pub fn new(link: Link, host: SocketAddr, terms: Terms, setup: Vec<u8>) -> Self {
        Self::to_any(link, vec![host], terms, setup)
    }

    /// A joiner saying hello at each of `hosts`, the addresses in the host's join code,
    /// which takes the answer from whichever the host is reached at.
    pub fn to_any(link: Link, hosts: Vec<SocketAddr>, terms: Terms, setup: Vec<u8>) -> Self {
        Self {
            link,
            hosts,
            terms,
            setup,
            id: RandomState::new().hash_one(Instant::now()),
            last_hello: None,
        }
    }

    /// Says hello if it is time to, and returns the host's answer once there is one.
    pub fn poll(&mut self, now: Instant) -> Option<Answer> {
        for (from, packet) in received(&self.link) {
            // A NAT may show the host on another port than its code says, so its address
            // is known by its IP.
            if !self.hosts.iter().any(|host| host.ip() == from.ip()) {
                continue;
            }
            match packet {
                Packet::Welcome {
                    slot,
                    players,
                    setup,
                } => {
                    return Some(Answer::Welcomed(Agreed {
                        peer: from,
                        slot: usize::from(slot),
                        players: usize::from(players),
                        peer_setup: setup,
                    }));
                }
                Packet::Refused(refusal) => return Some(Answer::Refused(refusal)),
                Packet::Hello { .. } | Packet::Punch => {}
            }
        }
        if self
            .last_hello
            .is_none_or(|last| now.duration_since(last) >= RESEND)
        {
            self.last_hello = Some(now);
            let hello = Packet::Hello {
                protocol: PROTOCOL,
                terms: self.terms.clone(),
                setup: self.setup.clone(),
                joiner: self.id,
            };
            for &host in &self.hosts {
                send(&self.link, host, &hello);
            }
        }
        None
    }
}

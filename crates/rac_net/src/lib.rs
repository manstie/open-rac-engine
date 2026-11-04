//! Direct-IP co-op for the hero kit (PLAN.md P1.18, section 4.2, G4).
//!
//! Both peers simulate everything and send each other only their [`PlayerInput`]s, GGPO's
//! way, through [`ggrs`]. A peer that guessed the other's input wrong loads the snapshot of
//! that tick (`rac_sim::Snapshot`) and simulates forward again. How the kit's simulation
//! stays the same on both peers is in `docs/netcode.md`.
//!
//! - [`handshake`] agrees the terms before a session: same build, same rules, a slot each.
//! - [`link`] carries the packets: UDP, or memory with chosen delays and losses for tests.
//! - [`Session`] is a running session. [`start_session`] puts it in charge of the
//!   simulation's ticks; [`end_session`] hands them back.
//! - [`SessionStatus`] and [`SessionEvent`]s say how it is going, for the app to show.

pub mod handshake;
pub mod link;

use bevy::{
    app::FixedMainScheduleOrder,
    ecs::schedule::{InternedScheduleLabel, ScheduleLabel},
    prelude::*,
};
use ggrs::{
    DesyncDetection, GgrsError, GgrsEvent, GgrsRequest, InputPredictor, InputStatus, P2PSession,
    PlayerType, SessionBuilder, SessionState, SyncTestSession,
};
use rac_input::{Buttons, InputSource, LocalSampler, PlayerInput, PlayerInputs};
use rac_sim::{Resimulating, RollbackWindow, SimTick, Snapshot, TICK_HZ, rollback};
use std::{collections::BTreeMap, net::SocketAddr, time::Duration};

pub use ggrs;
pub use handshake::{Agreed, Answer, Host, Joiner, Refusal, Terms};
pub use link::{Link, MemoryLink, MemoryNet, Transport, UdpLink};

/// How many confirmed ticks' checksums a session keeps.
const KEPT_CHECKSUMS: u64 = 64;

/// How `ggrs` is set up for the kit: its inputs, snapshots and addresses.
pub struct NetConfig;

impl ggrs::Config for NetConfig {
    type Input = PlayerInput;
    type InputPredictor = PredictHeld;
    type State = Snapshot;
    type Address = SocketAddr;
}

/// Guesses that a remote player keeps holding what they held, without pressing anything
/// new: a press repeated every guessed tick would fire again and again until corrected.
pub struct PredictHeld;

impl InputPredictor<PlayerInput> for PredictHeld {
    fn predict(previous: PlayerInput) -> PlayerInput {
        PlayerInput {
            pressed: Buttons::NONE,
            ..previous
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("network: {0}")]
    Io(#[from] std::io::Error),
    #[error("rollback session: {0}")]
    Session(#[from] GgrsError),
}

/// How a direct-IP session behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSettings {
    /// Ticks each local input waits before it applies. More hides more latency without
    /// rollbacks, but makes the controls feel later.
    pub input_delay: usize,
    /// The most ticks a peer simulates ahead of the other's inputs before it waits.
    pub max_prediction: usize,
    /// How often, in ticks, peers compare checksums.
    pub desync_interval: u32,
    /// How long the other peer may be silent before the session drops it.
    pub disconnect_timeout: Duration,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            input_delay: 2,
            max_prediction: 8,
            desync_interval: 10,
            disconnect_timeout: Duration::from_secs(3),
        }
    }
}

/// How a session is going, for the app to show.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// Exchanging round trips with the other peer before the first tick.
    Synchronizing {
        done: u32,
        of: u32,
    },
    Running,
    /// Nothing heard for a while; the peer is dropped `in_ms` from now unless it returns.
    Interrupted {
        in_ms: u128,
    },
    /// The other peer left or timed out. Its hero stands still from then on.
    Disconnected,
    /// The peers' checksums differed at `tick`: they no longer simulate the same game.
    Desynced {
        tick: u64,
    },
}

/// Something that happened in the session.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    Synchronized,
    Interrupted,
    Resumed,
    Disconnected,
    Desynced { tick: u64 },
}

enum Kind {
    SyncTest(SyncTestSession<NetConfig>),
    P2P {
        // Boxed: it is several times the size of a sync test.
        session: Box<P2PSession<NetConfig>>,
        /// The host keeps answering the guest's hellos, in case its welcome was lost.
        host: Option<Host>,
    },
}

/// Where each local player's input comes from: the devices, or a test's script.
pub type LocalInput = fn(&mut World, usize) -> PlayerInput;

/// A running session: `ggrs` decides when ticks run, which run again, and with whose
/// input.
#[derive(Resource)]
pub struct Session {
    kind: Kind,
    local: Vec<usize>,
    local_input: LocalInput,
    /// The schedules one tick runs, taken from Bevy while the session runs them.
    tick_labels: Vec<InternedScheduleLabel>,
    start_tick: u64,
    /// One past the newest tick simulated so far: ticks before it run again only after a
    /// rollback.
    newest: u64,
    /// Ticks to sit out, as `ggrs` recommends when this peer runs ahead of the other.
    skip: u32,
    /// The latest checksum saved for each recent tick. Those up to the confirmed tick are
    /// final, and peers' should match.
    checksums: BTreeMap<u64, u128>,
}

/// The device input, for the first local player.
pub fn device_input(world: &mut World, _player: usize) -> PlayerInput {
    world
        .get_resource_mut::<LocalSampler>()
        .map_or_else(PlayerInput::default, |mut sampler| {
            sampler.take_tick_input()
        })
}

impl Session {
    /// A local session that runs every tick, then loads the snapshot `check_distance`
    /// ticks back and simulates forward again, checking the checksums match: a test that
    /// rollback restores everything the simulation depends on.
    pub fn sync_test(players: usize, check_distance: usize) -> Result<Self, NetError> {
        let session = SessionBuilder::<NetConfig>::new()
            .with_num_players(players)?
            .with_check_distance(check_distance)
            .with_max_prediction_window(check_distance + 1)
            .start_synctest_session()?;
        Ok(Self::new(Kind::SyncTest(session), (0..players).collect()))
    }

    /// A two-player session over `link` with the peer `agreed` on. `host` is the host's
    /// handshake, which keeps welcoming the guest.
    pub fn p2p(
        link: Link,
        agreed: &Agreed,
        host: Option<Host>,
        settings: SessionSettings,
    ) -> Result<Self, NetError> {
        let mut builder = SessionBuilder::<NetConfig>::new()
            .with_num_players(agreed.players)?
            .with_input_delay(settings.input_delay)
            .with_max_prediction_window(settings.max_prediction)
            .with_desync_detection_mode(DesyncDetection::On {
                interval: settings.desync_interval.max(1),
            })
            .with_disconnect_timeout(settings.disconnect_timeout)
            .with_fps(TICK_HZ as usize)?;
        for player in 0..agreed.players {
            let kind = if player == agreed.slot {
                PlayerType::Local
            } else {
                PlayerType::Remote(agreed.peer)
            };
            builder = builder.add_player(kind, player)?;
        }
        let session = builder.start_p2p_session(link)?;
        Ok(Self::new(
            Kind::P2P {
                session: Box::new(session),
                host,
            },
            vec![agreed.slot],
        ))
    }

    fn new(kind: Kind, local: Vec<usize>) -> Self {
        Self {
            kind,
            local,
            local_input: device_input,
            tick_labels: Vec::new(),
            start_tick: 0,
            newest: 0,
            skip: 0,
            checksums: BTreeMap::new(),
        }
    }

    /// Takes local players' input from `input` instead of the devices.
    pub fn with_local_input(mut self, input: LocalInput) -> Self {
        self.local_input = input;
        self
    }

    /// The local players' slots.
    pub fn local_players(&self) -> &[usize] {
        &self.local
    }

    /// The newest tick whose inputs are all confirmed: it can no longer be rolled back.
    pub fn confirmed_tick(&self) -> Option<u64> {
        let frame = match &self.kind {
            Kind::SyncTest(session) => session.current_frame() - session.check_distance() as i32,
            Kind::P2P { session, .. } => session.confirmed_frame(),
        };
        u64::try_from(frame)
            .ok()
            .map(|frame| self.start_tick + frame)
    }

    /// The checksum of the state at the start of `tick`, once that tick is confirmed.
    pub fn confirmed_checksum(&self, tick: u64) -> Option<u128> {
        if self.confirmed_tick()? < tick {
            return None;
        }
        self.checksums.get(&tick).copied()
    }

    /// Round-trip time and queue length to the other peer, for showing.
    pub fn network_stats(&self) -> Option<ggrs::NetworkStats> {
        let Kind::P2P { session, .. } = &self.kind else {
            return None;
        };
        let remote = *session.remote_player_handles().first()?;
        session.network_stats(remote).ok()
    }

    /// One fixed step of the app: polls the network, adds this machine's input, and does
    /// what `ggrs` asks.
    fn step(&mut self, world: &mut World) {
        if let Kind::P2P { session, host } = &mut self.kind {
            if let Some(host) = host {
                host.poll();
            }
            session.poll_remote_clients();
            let events: Vec<_> = session.events().collect();
            for event in events {
                self.note(world, event);
            }
            let Kind::P2P { session, .. } = &self.kind else {
                unreachable!()
            };
            if session.current_state() != SessionState::Running {
                return;
            }
            if self.skip > 0 {
                self.skip -= 1;
                return;
            }
        }
        let local = self.local.clone();
        for player in local {
            let input = (self.local_input)(world, player);
            let added = match &mut self.kind {
                Kind::SyncTest(session) => session.add_local_input(player, input),
                Kind::P2P { session, .. } => session.add_local_input(player, input),
            };
            if let Err(error) = added {
                warn!("couldn't add player {player}'s input: {error}");
            }
        }
        let requests = match &mut self.kind {
            Kind::SyncTest(session) => session.advance_frame(),
            Kind::P2P { session, .. } => session.advance_frame(),
        };
        match requests {
            Ok(requests) => {
                for request in requests {
                    self.fulfil(world, request);
                }
            }
            // Too far ahead of the other peer's inputs: wait for them.
            Err(GgrsError::PredictionThreshold | GgrsError::NotSynchronized) => {}
            Err(GgrsError::MismatchedChecksum {
                mismatched_frames, ..
            }) => {
                let tick = self.start_tick + mismatched_frames.first().copied().unwrap_or(0) as u64;
                error!("the sync test found tick {tick} different after a rollback");
                world.insert_resource(SessionStatus::Desynced { tick });
                world.write_message(SessionEvent::Desynced { tick });
            }
            Err(error) => warn!("the session couldn't advance: {error}"),
        }
        if let Some(confirmed) = self.confirmed_tick() {
            // Nothing before the confirmed tick can be loaded again.
            rollback::bury_retired(world, confirmed);
            // Kept a while, so peers that confirm at different times can compare.
            self.checksums = self
                .checksums
                .split_off(&confirmed.saturating_sub(KEPT_CHECKSUMS));
        }
    }

    fn fulfil(&mut self, world: &mut World, request: GgrsRequest<NetConfig>) {
        match request {
            GgrsRequest::SaveGameState { cell, frame } => {
                let snapshot = Snapshot::save(world);
                let checksum = snapshot.checksum();
                self.checksums.insert(snapshot.tick(), checksum);
                cell.save(frame, Some(snapshot), Some(checksum));
            }
            GgrsRequest::LoadGameState { cell, .. } => {
                if let Some(snapshot) = cell.load() {
                    snapshot.load(world);
                }
            }
            GgrsRequest::AdvanceFrame { inputs } => {
                if let Some(mut slots) = world.get_resource_mut::<PlayerInputs>() {
                    for (slot, (input, status)) in slots.0.iter_mut().zip(&inputs) {
                        *slot = match status {
                            InputStatus::Disconnected => PlayerInput::default(),
                            _ => *input,
                        };
                    }
                }
                let tick = world.get_resource::<SimTick>().map_or(0, |tick| tick.0);
                world.insert_resource(Resimulating(tick < self.newest));
                for label in &self.tick_labels {
                    let _ = world.try_run_schedule(*label);
                }
                world.insert_resource(Resimulating(false));
                self.newest = self.newest.max(tick + 1);
            }
        }
    }

    fn note(&mut self, world: &mut World, event: GgrsEvent<NetConfig>) {
        let (status, event) = match event {
            GgrsEvent::Synchronizing { total, count, .. } => (
                Some(SessionStatus::Synchronizing {
                    done: count,
                    of: total,
                }),
                None,
            ),
            GgrsEvent::Synchronized { .. } => (
                Some(SessionStatus::Running),
                Some(SessionEvent::Synchronized),
            ),
            GgrsEvent::Disconnected { addr } => {
                info!("{addr} left the session");
                (
                    Some(SessionStatus::Disconnected),
                    Some(SessionEvent::Disconnected),
                )
            }
            GgrsEvent::NetworkInterrupted {
                disconnect_timeout, ..
            } => (
                Some(SessionStatus::Interrupted {
                    in_ms: disconnect_timeout,
                }),
                Some(SessionEvent::Interrupted),
            ),
            GgrsEvent::NetworkResumed { .. } => {
                (Some(SessionStatus::Running), Some(SessionEvent::Resumed))
            }
            GgrsEvent::WaitRecommendation { skip_frames } => {
                self.skip = self.skip.max(skip_frames);
                (None, None)
            }
            GgrsEvent::DesyncDetected {
                frame,
                local_checksum,
                remote_checksum,
                addr,
            } => {
                let tick = self.start_tick + u64::try_from(frame).unwrap_or(0);
                error!(
                    "desync with {addr} at tick {tick}: {local_checksum:032x} here, \
                     {remote_checksum:032x} there"
                );
                (
                    Some(SessionStatus::Desynced { tick }),
                    Some(SessionEvent::Desynced { tick }),
                )
            }
        };
        if let Some(status) = status {
            // A desync stays reported: the game can't recover from it yet.
            if !matches!(
                world.get_resource::<SessionStatus>(),
                Some(SessionStatus::Desynced { .. })
            ) {
                world.insert_resource(status);
            }
        }
        if let Some(event) = event {
            world.write_message(event);
        }
    }
}

/// The schedule a session's ticks run from, in place of the five Bevy runs per tick.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionStep;

/// Adds the session machinery. Sessions start with [`start_session`].
pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SessionEvent>()
            .init_schedule(SessionStep)
            .add_systems(SessionStep, run_session);
    }
}

fn run_session(world: &mut World) {
    if world.contains_resource::<Session>() {
        world.resource_scope(|world, mut session: Mut<Session>| session.step(world));
    }
}

/// Puts `session` in charge of the simulation's ticks, from the current tick on. Start it
/// at the same tick, on the same state, on every peer: at a level's start.
pub fn start_session(world: &mut World, mut session: Session) {
    end_session(world);
    let tick = world.get_resource::<SimTick>().map_or(0, |tick| tick.0);
    session.start_tick = tick;
    session.newest = tick;
    let mut order = world.resource_mut::<FixedMainScheduleOrder>();
    session.tick_labels = std::mem::replace(&mut order.labels, vec![SessionStep.intern()]);
    world.insert_resource(RollbackWindow);
    world.insert_resource(InputSource::Session);
    world.insert_resource(match session.kind {
        Kind::SyncTest(_) => SessionStatus::Running,
        Kind::P2P { .. } => SessionStatus::Synchronizing { done: 0, of: 0 },
    });
    world.insert_resource(session);
}

/// Ends the session, if one runs, and hands the ticks back to Bevy's fixed loop.
pub fn end_session(world: &mut World) {
    let Some(session) = world.remove_resource::<Session>() else {
        return;
    };
    world.resource_mut::<FixedMainScheduleOrder>().labels = session.tick_labels;
    world.remove_resource::<RollbackWindow>();
    world.remove_resource::<SessionStatus>();
    world.insert_resource(InputSource::Local);
    // No rollback can bring them back now.
    rollback::bury_retired(world, u64::MAX);
}

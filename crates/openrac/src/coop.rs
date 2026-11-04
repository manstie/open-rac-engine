//! Direct-IP co-op in the demo (PLAN.md P1.18): hosting, joining, and a local sync test.
//!
//! The level spawns with both heroes and waits at its first tick while the peers shake
//! hands (`rac_net::handshake`); then a `rac_net::Session` runs the ticks. The host is
//! player 1, the guest player 2. The game clock keeps running behind the pause menu
//! during a session, since the other player plays on.

use bevy::{app::FixedMainScheduleOrder, ecs::schedule::InternedScheduleLabel, prelude::*};
use rac_demo::DemoSettings;
use rac_hero::{FollowCamera, HeroTuning};
use rac_input::{LocalPlayer, PlayerInput};
use rac_net::{
    Answer, Host, Joiner, Link, NetPlugin, Session, SessionEvent, SessionSettings, SessionStatus,
    Terms, UdpLink, device_input, end_session, start_session,
};
use rac_sim::StateHasher;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Instant,
};

use crate::{
    camera,
    flow::{AppState, Pause},
};

/// The port a host listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 26450;

/// How this run plays: alone, or with someone.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub enum CoopChoice {
    #[default]
    Alone,
    /// Wait for a guest on this port.
    Host(u16),
    /// Join the host at this address.
    Join(SocketAddr),
    /// Both heroes on this machine, with every tick rolled back and run again to check
    /// the simulation restores exactly (`rac_net::Session::sync_test`).
    SyncTest,
}

impl CoopChoice {
    /// The player slot this machine plays, and how many heroes the level has.
    pub fn slots(&self) -> (usize, usize) {
        match self {
            CoopChoice::Alone => (0, 1),
            CoopChoice::Host(_) | CoopChoice::SyncTest => (0, 2),
            CoopChoice::Join(_) => (1, 2),
        }
    }
}

/// Parses `host:port` or `host`, which takes [`DEFAULT_PORT`].
pub fn parse_address(text: &str) -> Result<SocketAddr, String> {
    let with_port = if text
        .rsplit_once(':')
        .is_some_and(|(_, port)| port.parse::<u16>().is_ok())
        && !text.ends_with(']')
    {
        text.to_owned()
    } else {
        format!("{text}:{DEFAULT_PORT}")
    };
    with_port
        .to_socket_addrs()
        .map_err(|error| format!("{text:?}: {error}"))?
        .next()
        .ok_or_else(|| format!("{text:?} has no address"))
}

/// How the session behaves, from the command line.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct CoopSettings(pub SessionSettings);

/// The handshake under way.
#[derive(Resource)]
enum Connecting {
    Host { host: Host, link: Link },
    Join { joiner: Joiner, link: Link },
}

/// The schedules a tick runs, taken from Bevy's fixed loop while the peers shake hands, so
/// the level waits at its first tick for the session to run it.
#[derive(Resource)]
struct HeldTicks(Vec<InternedScheduleLabel>);

fn hold_ticks(world: &mut World) {
    let labels = std::mem::take(&mut world.resource_mut::<FixedMainScheduleOrder>().labels);
    world.insert_resource(HeldTicks(labels));
}

fn release_ticks(world: &mut World) {
    if let Some(HeldTicks(labels)) = world.remove_resource::<HeldTicks>() {
        world.resource_mut::<FixedMainScheduleOrder>().labels = labels;
    }
}

/// The local player's input, neutral while the pause menu is open: the session plays on
/// behind it, and the menu's keys mustn't move the hero.
fn coop_input(world: &mut World, player: usize) -> PlayerInput {
    let input = device_input(world, player);
    let paused = world
        .get_resource::<State<Pause>>()
        .is_some_and(|pause| *pause.get() == Pause::Paused);
    if paused {
        PlayerInput::default()
    } else {
        input
    }
}

/// The line shown at the top of the screen while there is something to say.
#[derive(Component)]
struct StatusLine;

pub struct CoopPlugin;

impl Plugin for CoopPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(NetPlugin)
            .init_resource::<CoopChoice>()
            .init_resource::<CoopSettings>()
            .add_systems(
                OnEnter(AppState::InGame),
                (
                    choose_slots.before(rac_demo::spawn_demo),
                    begin.after(rac_demo::spawn_demo),
                ),
            )
            .add_systems(OnExit(AppState::InGame), leave)
            .add_systems(
                Update,
                (connect, follow_session, show_status).run_if(in_state(AppState::InGame)),
            );
    }
}

/// Which hero is this machine's, and how many there are, before the level spawns.
fn choose_slots(
    choice: Res<CoopChoice>,
    mut local: ResMut<LocalPlayer>,
    mut demo: ResMut<DemoSettings>,
) {
    let (slot, players) = choice.slots();
    local.0 = slot;
    demo.players = players;
}

/// What both peers must agree on: the build, and every setting that changes the
/// simulation.
fn terms(tuning: &HeroTuning, armory: &rac_combat::Armory, level: rac_demo::DemoGunLevel) -> Terms {
    let mut hasher = StateHasher::default();
    let profile = ron::to_string(&tuning.0).unwrap_or_default();
    let guns = ron::to_string(armory).unwrap_or_default();
    std::hash::Hasher::write(&mut hasher, profile.as_bytes());
    std::hash::Hasher::write(&mut hasher, guns.as_bytes());
    std::hash::Hasher::write(&mut hasher, &[level.0]);
    Terms {
        build: format!("openrac {}", env!("CARGO_PKG_VERSION")),
        rules: hasher.finish128() as u64,
    }
}

/// The local player's choices the other peer needs: their camera setting.
fn setup(demo: &DemoSettings) -> Vec<u8> {
    vec![camera::code(demo.camera)]
}

/// Starts the sync test, or the handshake, once the level has spawned.
fn begin(world: &mut World) {
    let choice = world.resource::<CoopChoice>().clone();
    if choice == CoopChoice::Alone {
        return;
    }
    if choice == CoopChoice::SyncTest {
        match Session::sync_test(2, 7) {
            Ok(session) => {
                info!("sync test: every tick is rolled back 7 ticks and run again");
                start_session(world, session.with_local_input(coop_input));
            }
            Err(error) => error!("couldn't start the sync test: {error}"),
        }
        return;
    }
    let terms = terms(
        world.resource::<HeroTuning>(),
        world.resource::<rac_combat::Armory>(),
        *world.resource::<rac_demo::DemoGunLevel>(),
    );
    let setup = setup(world.resource::<DemoSettings>());
    let bind = match choice {
        CoopChoice::Host(port) => SocketAddr::from(([0, 0, 0, 0], port)),
        CoopChoice::Join(host) if host.is_ipv6() => SocketAddr::from(([0u16; 8], 0)),
        _ => SocketAddr::from(([0, 0, 0, 0], 0)),
    };
    let udp = match UdpLink::bind(bind) {
        Ok(udp) => udp,
        Err(error) => {
            error!("couldn't open UDP {bind}: {error}; playing alone");
            return;
        }
    };
    let link = Link::new(udp);
    let connecting = match choice {
        CoopChoice::Host(port) => {
            info!("hosting on UDP port {port}: waiting for player 2");
            Connecting::Host {
                host: Host::new(link.clone(), terms, setup),
                link,
            }
        }
        CoopChoice::Join(host) => {
            info!("joining {host}");
            Connecting::Join {
                joiner: Joiner::new(link.clone(), host, terms, setup),
                link,
            }
        }
        _ => return,
    };
    world.insert_resource(connecting);
    hold_ticks(world);
}

/// Polls the handshake, and starts the session once both sides agree.
fn connect(world: &mut World) {
    let Some(mut connecting) = world.remove_resource::<Connecting>() else {
        return;
    };
    let agreed = match &mut connecting {
        Connecting::Host { host, .. } => host.poll(),
        Connecting::Join { joiner, .. } => match joiner.poll(Instant::now()) {
            Some(Answer::Welcomed(agreed)) => Some(agreed),
            Some(Answer::Refused(refusal)) => {
                error!("the host refused: {refusal}; playing alone");
                world.insert_resource(Refused(refusal.to_string()));
                release_ticks(world);
                return;
            }
            None => None,
        },
    };
    let Some(agreed) = agreed else {
        world.insert_resource(connecting);
        return;
    };
    let expected = world.resource::<LocalPlayer>().0;
    if agreed.slot != expected {
        error!(
            "the session gave this machine slot {}, not {expected}",
            agreed.slot
        );
        return;
    }
    // The other player's camera, as they chose it.
    let others = agreed
        .peer_setup
        .first()
        .and_then(|&code| camera::from_code(code))
        .unwrap_or(None);
    world.resource_mut::<DemoSettings>().others_camera = others;
    let mut cameras = world.query::<&mut FollowCamera>();
    for mut camera in cameras.iter_mut(world) {
        if camera.player != expected {
            camera.setting = others;
        }
    }
    let settings = world.resource::<CoopSettings>().0;
    let (link, host) = match connecting {
        Connecting::Host { host, link } => (link, Some(host)),
        Connecting::Join { link, .. } => (link, None),
    };
    release_ticks(world);
    match Session::p2p(link, &agreed, host, settings) {
        Ok(session) => {
            info!("playing with {} as player {}", agreed.peer, agreed.slot + 1);
            start_session(world, session.with_local_input(coop_input));
        }
        Err(error) => error!("couldn't start the session: {error}; playing alone"),
    }
}

/// The host turned this machine away; the level is played alone.
#[derive(Resource)]
struct Refused(String);

/// Ends the session when the other player leaves, so this one plays on alone.
fn follow_session(world: &mut World) {
    let left = world
        .resource_mut::<Messages<SessionEvent>>()
        .drain()
        .any(|event| event == SessionEvent::Disconnected);
    if left {
        info!(
            "player {} left; playing on alone",
            2 - world.resource::<LocalPlayer>().0
        );
        end_session(world);
        world.insert_resource(Left);
    }
}

/// The other player left this session.
#[derive(Resource)]
struct Left;

fn show_status(
    mut commands: Commands,
    choice: Res<CoopChoice>,
    connecting: Option<Res<Connecting>>,
    status: Option<Res<SessionStatus>>,
    refused: Option<Res<Refused>>,
    left: Option<Res<Left>>,
    session: Option<Res<Session>>,
    mut lines: Query<(Entity, &mut Text), With<StatusLine>>,
) {
    let text = if let Some(refused) = refused {
        Some(format!("The host refused: {}", refused.0))
    } else if left.is_some() {
        Some("The other player left".to_owned())
    } else if connecting.is_some() {
        Some(match &*choice {
            CoopChoice::Host(port) => format!("Waiting for player 2 on UDP port {port}…"),
            CoopChoice::Join(host) => format!("Joining {host}…"),
            _ => String::new(),
        })
    } else {
        match status.as_deref() {
            Some(SessionStatus::Synchronizing { done, of }) => {
                Some(format!("Connecting… {done}/{of}"))
            }
            Some(SessionStatus::Interrupted { in_ms }) => Some(format!(
                "Connection lost: waiting {:.1} s",
                *in_ms as f32 / 1000.0
            )),
            Some(SessionStatus::Desynced { tick }) => Some(format!(
                "Out of step with the other player since tick {tick}"
            )),
            Some(SessionStatus::Running) if *choice == CoopChoice::SyncTest => {
                Some("Sync test".to_owned())
            }
            Some(SessionStatus::Running) => session
                .and_then(|session| session.network_stats())
                .map(|stats| format!("{} ms", stats.ping)),
            _ => None,
        }
    };
    match (text, lines.single_mut()) {
        (Some(text), Ok((_, mut line))) => {
            if line.0 != text {
                line.0 = text;
            }
        }
        (Some(text), Err(_)) => {
            commands.spawn((
                StatusLine,
                rac_demo::DemoEntity,
                Text::new(text),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(8.0),
                    right: Val::Px(12.0),
                    ..default()
                },
            ));
        }
        (None, Ok((entity, _))) => commands.entity(entity).despawn(),
        (None, Err(_)) => {}
    }
}

/// Leaving the level ends any session and handshake.
fn leave(world: &mut World) {
    end_session(world);
    release_ticks(world);
    world.remove_resource::<Connecting>();
    world.remove_resource::<Refused>();
    world.remove_resource::<Left>();
    *world.resource_mut::<LocalPlayer>() = LocalPlayer(0);
}

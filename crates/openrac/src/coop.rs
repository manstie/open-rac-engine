//! Direct-IP co-op in the demo (PLAN.md P1.18): hosting, joining, and a local sync test.
//!
//! The level spawns with both heroes and waits at its first tick while the peers shake
//! hands (`rac_net::handshake`); then a `rac_net::Session` runs the ticks. The host is
//! player 1, the guest player 2. The game clock keeps running behind the pause menu
//! during a session, since the other player plays on.
//!
//! Meanwhile each side learns where it can be reached and shows it as a join code (D8),
//! copied to the clipboard: the guest joins by the host's code or address. The host asks
//! its router to forward its port (UPnP); where it won't, the host pastes the guest's code
//! (Ctrl+V) to punch through its NAT to it.

use bevy::{
    app::FixedMainScheduleOrder, clipboard::Clipboard, ecs::schedule::InternedScheduleLabel,
    prelude::*,
};
use rac_demo::DemoSettings;
use rac_hero::{FollowCamera, HeroTuning};
use rac_input::{LocalPlayer, PlayerInput};
use rac_net::{
    Answer, Host, Joiner, Link, NetPlugin, Session, SessionEvent, SessionSettings, SessionStatus,
    Terms, UdpLink,
    code::{JoinCode, local_address},
    device_input, end_session, start_session,
    stun::{self, StunQuery},
    upnp::{self, Mapping, MappingError},
};
use rac_sim::StateHasher;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    thread::JoinHandle,
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
    /// Join the host at any of these addresses: one given, or those in its join code.
    Join(Vec<SocketAddr>),
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

/// The addresses a host may be reached at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAddresses(pub Vec<SocketAddr>);

/// Parses a host's join code, or its address as [`parse_address`] does.
pub fn parse_join(text: &str) -> Result<HostAddresses, String> {
    match JoinCode::decode(text) {
        Ok(code) => Ok(HostAddresses(code.0)),
        Err(error) => parse_address(text)
            .map(|address| HostAddresses(vec![address]))
            .map_err(|address| format!("{address}, nor a join code ({error})")),
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

/// The host doesn't ask its router to forward its port.
#[derive(Resource, Debug, Clone, Copy)]
pub struct NoPortMapping;

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
            .add_systems(Last, unmap_on_exit.run_if(resource_exists::<PortMapping>))
            .add_systems(
                Update,
                (
                    connect,
                    follow_mapping.run_if(resource_exists::<PortMapping>),
                    (learn_code, swap_codes).run_if(resource_exists::<Reach>),
                    follow_session,
                    show_status,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
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
    // IPv6 only to a host reached by nothing else.
    let ipv6 = matches!(&choice, CoopChoice::Join(hosts) if hosts.iter().all(SocketAddr::is_ipv6));
    let bind = match choice {
        CoopChoice::Host(port) => SocketAddr::from(([0, 0, 0, 0], port)),
        _ if ipv6 => SocketAddr::from(([0u16; 8], 0)),
        _ => SocketAddr::from(([0, 0, 0, 0], 0)),
    };
    let udp = match UdpLink::bind(bind) {
        Ok(udp) => udp,
        Err(error) => {
            error!("couldn't open UDP {bind}: {error}; playing alone");
            return;
        }
    };
    let port = udp.local_addr().map_or(0, |address| address.port());
    let link = Link::new(udp);
    world.insert_resource(Reach::new(link.clone(), port, ipv6));
    let connecting = match choice {
        CoopChoice::Host(port) => {
            info!("hosting on UDP port {port}: waiting for player 2");
            let local =
                local_address(port, false).filter(|_| !world.contains_resource::<NoPortMapping>());
            world.insert_resource(match local {
                Some(local) => {
                    PortMapping::Asking(std::thread::spawn(move || upnp::map_port(local)))
                }
                None => PortMapping::Unmapped,
            });
            Connecting::Host {
                host: Host::new(link.clone(), terms, setup),
                link,
            }
        }
        CoopChoice::Join(hosts) => {
            let hosts: Vec<_> = hosts
                .into_iter()
                .filter(|host| host.is_ipv6() == ipv6)
                .collect();
            info!("joining the host at {hosts:?}");
            Connecting::Join {
                joiner: Joiner::to_any(link.clone(), hosts, terms, setup),
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
                world.remove_resource::<Reach>();
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
    // The codes are swapped; they aren't needed again, nor copied over the clipboard.
    world.remove_resource::<Reach>();
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

/// Where this machine can be reached, for its join code: its address on its own network,
/// and the public one STUN servers see the session's socket at (`rac_net::stun`).
#[derive(Resource)]
struct Reach {
    link: Link,
    local: Option<SocketAddr>,
    /// The STUN servers' addresses, looked up off the frame loop.
    servers: Option<JoinHandle<Vec<SocketAddr>>>,
    query: Option<StunQuery>,
    code: Option<String>,
    /// The router forwards the host's port, so the guest needs only the host's code.
    forwarded: bool,
    /// What became of the last code pasted, for the status line.
    pasted: Option<String>,
}

/// The host's port on its router (`rac_net::upnp`), asked for off the frame loop, and
/// given back on leaving the level. It outlives the handshake, so a mapping made after
/// the guest arrived by another way is still removed.
#[derive(Resource)]
enum PortMapping {
    Asking(JoinHandle<Result<Mapping, MappingError>>),
    Mapped(Mapping),
    Unmapped,
}

fn follow_mapping(mut mapping: ResMut<PortMapping>) {
    if !matches!(&*mapping, PortMapping::Asking(handle) if handle.is_finished()) {
        return;
    }
    let PortMapping::Asking(handle) = std::mem::replace(&mut *mapping, PortMapping::Unmapped)
    else {
        return;
    };
    match handle.join() {
        Ok(Ok(mapped)) => {
            info!(
                "the router at {} forwards UDP from {}",
                mapped.router(),
                mapped.external
            );
            *mapping = PortMapping::Mapped(mapped);
        }
        Ok(Err(error)) => info!("the port isn't forwarded: {error}"),
        Err(_) => warn!("asking the router to forward the port failed"),
    }
}

/// Gives the router's port back, waiting for the router.
fn unmap(mapping: PortMapping) {
    let mapped = match mapping {
        PortMapping::Asking(handle) => handle.join().ok().and_then(Result::ok),
        PortMapping::Mapped(mapped) => Some(mapped),
        PortMapping::Unmapped => None,
    };
    if let Some(mapped) = mapped
        && let Err(error) = mapped.remove()
    {
        warn!("the router kept forwarding the port: {error}");
    }
}

/// Quitting from the level gives the port back before the process ends, or the router
/// would forward it until its lease ran out.
fn unmap_on_exit(mut exits: MessageReader<AppExit>, mut commands: Commands) {
    if exits.read().next().is_some() {
        commands.queue(|world: &mut World| {
            if let Some(mapping) = world.remove_resource::<PortMapping>() {
                unmap(mapping);
            }
        });
    }
}

impl Reach {
    fn new(link: Link, port: u16, ipv6: bool) -> Self {
        Self {
            link,
            local: local_address(port, ipv6),
            servers: Some(std::thread::spawn(move || {
                stun::resolve(stun::SERVERS, ipv6)
            })),
            query: None,
            code: None,
            forwarded: false,
            pasted: None,
        }
    }
}

/// Asks the STUN servers for this machine's public address, and waits for the host's port
/// mapping, then makes its join code and copies it to the clipboard: the mapped address
/// first, then the public one, then its own on its network, which is enough on a LAN.
fn learn_code(
    mut reach: ResMut<Reach>,
    mapping: Option<Res<PortMapping>>,
    clipboard: Option<ResMut<Clipboard>>,
) {
    if reach.code.is_some() {
        return;
    }
    if reach.servers.as_ref().is_some_and(JoinHandle::is_finished)
        && let Some(servers) = reach.servers.take()
    {
        reach.query = Some(StunQuery::new(servers.join().unwrap_or_default()));
    }
    let Reach {
        link, query, local, ..
    } = &mut *reach;
    let Some(query) = query.as_mut() else {
        return;
    };
    let now = Instant::now();
    let public = query.poll(link, now);
    if public.is_none() && !query.gave_up(now) {
        return;
    }
    let mapped = match mapping.as_deref() {
        Some(PortMapping::Asking(_)) => return,
        Some(PortMapping::Mapped(mapped)) => Some(mapped.external),
        _ => None,
    };
    let mut addresses: Vec<SocketAddr> = Vec::new();
    for address in [mapped, public, *local].into_iter().flatten() {
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    reach.forwarded = mapped.is_some();
    if addresses.is_empty() {
        warn!("this machine's address isn't known, so it has no join code");
        reach.code = Some(String::new());
        return;
    }
    let code = JoinCode(addresses.clone()).encode();
    info!("your join code is {code}, for {addresses:?}");
    if let Some(mut clipboard) = clipboard
        && let Err(error) = clipboard.set_text(code.clone())
    {
        warn!("couldn't copy the join code: {error}");
    }
    reach.code = Some(code);
}

/// Ctrl+V takes the other player's join code from the clipboard: a host punches through
/// its NAT to the guest's addresses. Ctrl+C copies this machine's code again.
fn swap_codes(
    keys: Res<ButtonInput<KeyCode>>,
    clipboard: Option<ResMut<Clipboard>>,
    mut reach: ResMut<Reach>,
    mut connecting: Option<ResMut<Connecting>>,
) {
    // Let go in the same frame still counts: a quick press can come in one frame.
    let ctrls = [KeyCode::ControlLeft, KeyCode::ControlRight];
    let ctrl = keys.any_pressed(ctrls) || keys.any_just_released(ctrls);
    let (Some(mut clipboard), true) = (clipboard, ctrl) else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyC)
        && let Some(code) = reach.code.clone().filter(|code| !code.is_empty())
        && let Err(error) = clipboard.set_text(code)
    {
        warn!("couldn't copy the join code: {error}");
    }
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }
    let Some(Ok(text)) = clipboard.fetch_text().poll_result() else {
        reach.pasted = Some("The clipboard holds no text".to_owned());
        return;
    };
    let code = match JoinCode::decode(text.trim()) {
        Ok(code) => code,
        Err(error) => {
            reach.pasted = Some(format!("That isn't a join code: {error}"));
            return;
        }
    };
    if Some(text.trim()) == reach.code.as_deref() {
        reach.pasted = Some("That is your own code: paste the other player's".to_owned());
        return;
    }
    match connecting.as_deref_mut() {
        Some(Connecting::Host { host, .. }) => {
            info!("punching through to player 2 at {:?}", code.0);
            host.punch_to(code.0);
            reach.pasted = Some("Reaching player 2...".to_owned());
        }
        _ => {
            reach.pasted = Some("Only the host needs the other player's code".to_owned());
        }
    }
}

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
    reach: Option<Res<Reach>>,
    mut lines: Query<(Entity, &mut Text), With<StatusLine>>,
) {
    let text = if let Some(refused) = refused {
        Some(format!("The host refused: {}", refused.0))
    } else if left.is_some() {
        Some("The other player left".to_owned())
    } else if connecting.is_some() {
        // Three dots, as the default font has no ellipsis.
        let mut text = match &*choice {
            CoopChoice::Host(port) => format!("Waiting for player 2 on UDP port {port}..."),
            CoopChoice::Join(_) => "Joining the host...".to_owned(),
            _ => String::new(),
        };
        if let Some(reach) = &reach {
            match reach.code.as_deref() {
                None => text.push_str("\nFinding your join code..."),
                Some("") => {}
                Some(code) => {
                    text.push_str(&format!("\nYour join code: {code} (copied; Ctrl+C again)"));
                    text.push_str(match &*choice {
                        CoopChoice::Host(_) if reach.forwarded => {
                            "\nGive it to player 2: your router forwards the port"
                        }
                        CoopChoice::Host(_) => {
                            "\nGive it to player 2, and paste theirs here with Ctrl+V"
                        }
                        _ => "\nIf the host can't be reached, give it to them",
                    });
                }
            }
            if let Some(pasted) = &reach.pasted {
                text.push_str(&format!("\n{pasted}"));
            }
        }
        Some(text)
    } else {
        match status.as_deref() {
            Some(SessionStatus::Synchronizing { done, of }) => {
                Some(format!("Connecting... {done}/{of}"))
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
    world.remove_resource::<Reach>();
    if let Some(mapping) = world.remove_resource::<PortMapping>() {
        std::thread::spawn(move || unmap(mapping));
    }
    world.remove_resource::<Refused>();
    world.remove_resource::<Left>();
    *world.resource_mut::<LocalPlayer>() = LocalPlayer(0);
}

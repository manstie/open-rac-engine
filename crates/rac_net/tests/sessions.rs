//! Sessions run on a small simulation of their own that does what makes rollback hard:
//! spawning and retiring entities, drawing random numbers, and a message read on the tick
//! after it is written. Each session's result is checked against a plain run with the same
//! inputs.

use bevy::{app::FixedMain, prelude::*};
use rac_input::{Action, Buttons, PlayerInput, PlayerInputs};
use rac_net::{
    Answer, Host, Joiner, Link, MemoryLink, MemoryNet, NetPlugin, Refusal, Session,
    SessionSettings, SessionStatus, Terms, Transport,
    code::JoinCode,
    end_session, handshake, start_session,
    stun::{self, StunQuery},
};
use rac_sim::{
    RetireExt, RollbackApp, SimId, SimMessageApp, SimMessageReader, SimPlugin, SimRng, SimSet,
    SimTick, SimTransform, Snapshot,
};
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

#[derive(Component, Debug, Clone, PartialEq)]
#[require(SimId, SimTransform)]
struct Walker {
    player: usize,
    bumps: u32,
}

#[derive(Component, Debug, Clone, PartialEq)]
#[require(SimId, SimTransform)]
struct Shot {
    owner: usize,
    ticks_left: u32,
    speed: f32,
}

/// A shot ran out; its owner hears of it on the next tick.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
struct Bump {
    player: usize,
}

fn walk(
    inputs: Res<PlayerInputs>,
    mut bumps: SimMessageReader<Bump>,
    mut rng: ResMut<SimRng>,
    mut walkers: Query<(&mut Walker, &mut SimTransform)>,
    mut commands: Commands,
) {
    let bumped: Vec<usize> = bumps.read().map(|bump| bump.player).collect();
    let mut walkers: Vec<_> = walkers.iter_mut().collect();
    walkers.sort_by_key(|(walker, _)| walker.player);
    for (walker, transform) in &mut walkers {
        let input = inputs.get(walker.player);
        transform.translation += input.move_vector().extend(0.0) * 0.1;
        walker.bumps += bumped
            .iter()
            .filter(|&&player| player == walker.player)
            .count() as u32;
        if input.pressed(Action::Fire) {
            commands.spawn((
                Shot {
                    owner: walker.player,
                    ticks_left: 10 + rng.below(20),
                    speed: rng.range_f32(0.5, 1.5),
                },
                SimTransform::from_translation(transform.translation),
            ));
        }
    }
}

fn fly(
    mut shots: Query<(Entity, &SimId, &mut Shot, &mut SimTransform)>,
    mut bumps: MessageWriter<Bump>,
    mut commands: Commands,
) {
    let mut shots: Vec<_> = shots.iter_mut().collect();
    shots.sort_by_key(|(_, id, ..)| **id);
    for (entity, _, shot, transform) in &mut shots {
        transform.translation.z -= shot.speed * 0.1;
        shot.ticks_left -= 1;
        if shot.ticks_left == 0 {
            commands.entity(*entity).retire();
            bumps.write(Bump { player: shot.owner });
        }
    }
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, SimPlugin, NetPlugin))
        .init_resource::<PlayerInputs>()
        .add_sim_message::<Bump>()
        .roll_back_component::<Walker>()
        .roll_back_component::<Shot>()
        .add_systems(
            FixedUpdate,
            (walk.in_set(SimSet::World), fly.in_set(SimSet::Particles)),
        );
    for player in 0..2 {
        app.world_mut().spawn((
            Walker { player, bumps: 0 },
            SimTransform::from_xyz(player as f32, 0.0, 0.0),
        ));
    }
    app
}

/// What `player` does at `tick`: wanders, and fires every so often.
fn script(player: usize, tick: u64) -> PlayerInput {
    let phase = tick as i64 * (3 + player as i64);
    let mut pressed = Buttons::NONE;
    if (tick + 7 * player as u64).is_multiple_of(17) {
        pressed.insert(Action::Fire);
    }
    PlayerInput {
        move_stick: [
            ((phase % 255) - 127) as i8,
            (((phase / 3) % 255) - 127) as i8,
        ],
        held: pressed,
        pressed,
        ..default()
    }
}

/// The script, read at the tick a session takes the input.
fn scripted(world: &mut World, player: usize) -> PlayerInput {
    script(player, world.resource::<SimTick>().0)
}

/// The checksum after `ticks` ticks without a session, each player's input `delay` ticks
/// late, as a session's input delay makes it.
fn reference(ticks: u64, delay: u64) -> u128 {
    let mut app = app();
    for tick in 0..ticks {
        for player in 0..2 {
            app.world_mut().resource_mut::<PlayerInputs>().0[player] = match tick.checked_sub(delay)
            {
                Some(given) => script(player, given),
                None => PlayerInput::default(),
            };
        }
        app.world_mut().run_schedule(FixedMain);
    }
    Snapshot::save(app.world_mut()).checksum()
}

fn status(app: &App) -> Option<SessionStatus> {
    app.world().get_resource::<SessionStatus>().copied()
}

fn session(app: &App) -> &Session {
    app.world().resource::<Session>()
}

#[test]
fn a_sync_test_finds_every_tick_the_same_after_rollbacks() {
    let mut app = app();
    let sync_test = Session::sync_test(2, 7).unwrap().with_local_input(scripted);
    start_session(app.world_mut(), sync_test);
    for _ in 0..300 {
        app.world_mut().run_schedule(FixedMain);
        assert_eq!(status(&app), Some(SessionStatus::Running));
    }
    assert_eq!(app.world().resource::<SimTick>().0, 300);
    let confirmed = session(&app).confirmed_tick().unwrap();
    assert_eq!(
        session(&app).confirmed_checksum(confirmed),
        Some(reference(confirmed, 0))
    );
    // Shots were fired and ran out, so retiring and the late message were exercised.
    let bumps: u32 = app
        .world_mut()
        .query::<&Walker>()
        .iter(app.world())
        .map(|walker| walker.bumps)
        .sum();
    assert!(bumps > 10, "{bumps}");
    end_session(app.world_mut());
}

fn address(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn terms(rules: u64) -> Terms {
    Terms {
        build: "test".into(),
        rules,
    }
}

/// A host and a guest on `net`, agreed and with their sessions started.
fn connected(net: &MemoryNet, settings: SessionSettings) -> (App, App) {
    let host_link = Link::new(net.endpoint(address(7000)));
    let guest_link = Link::new(net.endpoint(address(7001)));
    let mut host = Host::new(host_link.clone(), terms(1), vec![1]);
    let mut joiner = Joiner::new(guest_link.clone(), address(7000), terms(1), vec![2]);
    let (mut hosted, mut joined) = (None, None);
    let start = Instant::now();
    while hosted.is_none() || joined.is_none() {
        assert!(start.elapsed() < Duration::from_secs(10), "no handshake");
        if let Some(Answer::Welcomed(agreed)) = joiner.poll(Instant::now()) {
            joined = Some(agreed);
        }
        if let Some(agreed) = host.poll() {
            hosted = Some(agreed);
        }
        net.advance();
        std::thread::sleep(Duration::from_millis(1));
    }
    let (hosted, joined) = (hosted.unwrap(), joined.unwrap());
    assert_eq!((hosted.slot, joined.slot), (0, 1));
    assert_eq!(
        (hosted.peer_setup.as_slice(), joined.peer_setup.as_slice()),
        (&[2][..], &[1][..])
    );

    let mut host_app = app();
    let mut guest_app = app();
    let host_session = Session::p2p(host_link, &hosted, Some(host), settings)
        .unwrap()
        .with_local_input(scripted);
    let guest_session = Session::p2p(guest_link, &joined, None, settings)
        .unwrap()
        .with_local_input(scripted);
    start_session(host_app.world_mut(), host_session);
    start_session(guest_app.world_mut(), guest_session);
    (host_app, guest_app)
}

#[test]
fn peers_on_a_slow_lossy_network_agree_with_a_plain_run() {
    let net = MemoryNet::new(3, 0.05, 11);
    let settings = SessionSettings::default();
    let (mut host, mut guest) = connected(&net, settings);
    let target = 300;
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(60), "too slow");
        host.world_mut().run_schedule(FixedMain);
        guest.world_mut().run_schedule(FixedMain);
        net.advance();
        for app in [&host, &guest] {
            assert!(
                !matches!(status(app), Some(SessionStatus::Desynced { .. })),
                "{:?}",
                status(app)
            );
        }
        let confirmed = |app: &App| session(app).confirmed_tick().unwrap_or(0);
        if confirmed(&host) >= target && confirmed(&guest) >= target {
            break;
        }
        // ggrs times its resends by the clock.
        std::thread::sleep(Duration::from_micros(200));
    }
    let expected = reference(target, settings.input_delay as u64);
    assert_eq!(session(&host).confirmed_checksum(target), Some(expected));
    assert_eq!(session(&guest).confirmed_checksum(target), Some(expected));
}

#[test]
fn a_host_refuses_other_rules() {
    let net = MemoryNet::new(1, 0.0, 1);
    let mut host = Host::new(Link::new(net.endpoint(address(7000))), terms(1), Vec::new());
    let mut joiner = Joiner::new(
        Link::new(net.endpoint(address(7001))),
        address(7000),
        terms(2),
        Vec::new(),
    );
    for _ in 0..10 {
        assert!(host.poll().is_none());
        if let Some(answer) = joiner.poll(Instant::now()) {
            assert_eq!(answer, Answer::Refused(Refusal::Rules));
            return;
        }
        net.advance();
    }
    panic!("no answer");
}

#[test]
fn a_silent_peer_is_dropped_and_the_game_goes_on() {
    let net = MemoryNet::new(1, 0.0, 3);
    let settings = SessionSettings {
        disconnect_timeout: Duration::from_millis(300),
        ..default()
    };
    let (mut host, mut guest) = connected(&net, settings);
    let start = Instant::now();
    while host.world().resource::<SimTick>().0 < 60 {
        assert!(start.elapsed() < Duration::from_secs(20), "never ran");
        host.world_mut().run_schedule(FixedMain);
        guest.world_mut().run_schedule(FixedMain);
        net.advance();
        std::thread::sleep(Duration::from_micros(200));
    }
    // The guest goes quiet.
    net.set(1, 1.0);
    let start = Instant::now();
    while status(&host) != Some(SessionStatus::Disconnected) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            status(&host)
        );
        host.world_mut().run_schedule(FixedMain);
        net.advance();
        std::thread::sleep(Duration::from_millis(1));
    }
    let tick = host.world().resource::<SimTick>().0;
    for _ in 0..30 {
        host.world_mut().run_schedule(FixedMain);
    }
    assert!(host.world().resource::<SimTick>().0 > tick);
}

/// A STUN server on `net`: answers each Binding request with where it came from.
fn serve_stun(server: &mut MemoryLink) {
    for (from, request) in server.receive() {
        if stun::is_stun(&request) && request[0..2] == [0, 1] {
            let transaction: stun::Transaction = request[8..20].try_into().unwrap();
            server.send(from, &stun::answer(&transaction, from));
        }
    }
}

/// The public address `link` is seen at, asked of the STUN server at `server`.
fn learn_public(
    net: &MemoryNet,
    link: &Link,
    server: &mut MemoryLink,
    at: SocketAddr,
) -> SocketAddr {
    let mut query = StunQuery::new(vec![at]);
    let mut now = Instant::now();
    for _ in 0..50 {
        now += stun::RESEND / 4;
        if let Some(&public) = query.poll(link, now).first() {
            return public;
        }
        serve_stun(server);
        net.advance();
    }
    panic!("no STUN answer");
}

const STUN_SERVER: SocketAddr = SocketAddr::new(
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(198, 51, 100, 1)),
    3478,
);

/// A host and a guest, each behind its own NAT, with their links and their join codes:
/// the public address STUN saw, then the address on their own network.
fn behind_nats(net: &MemoryNet) -> ((Link, JoinCode), (Link, JoinCode)) {
    let mut server = net.endpoint(STUN_SERVER);
    let mut peer = |private: SocketAddr, public: SocketAddr| {
        net.behind_nat(private, public);
        let link = Link::new(net.endpoint(private));
        let seen = learn_public(net, &link, &mut server, STUN_SERVER);
        assert_eq!(seen, public);
        let code = JoinCode::decode(&JoinCode(vec![seen, private]).encode()).unwrap();
        (link, code)
    };
    let host = peer(
        SocketAddr::from(([10, 0, 0, 2], 26450)),
        SocketAddr::from(([203, 0, 113, 10], 41_000)),
    );
    let guest = peer(
        SocketAddr::from(([192, 168, 1, 20], 51_234)),
        SocketAddr::from(([198, 51, 100, 77], 62_001)),
    );
    (host, guest)
}

#[test]
fn peers_behind_nats_meet_by_swapping_join_codes_and_play() {
    let net = MemoryNet::new(2, 0.02, 5);
    let ((host_link, host_code), (guest_link, guest_code)) = behind_nats(&net);
    let mut host = Host::new(host_link.clone(), terms(1), Vec::new());
    host.punch_to(guest_code.0.clone());
    let mut joiner = Joiner::to_any(
        guest_link.clone(),
        host_code.0.clone(),
        terms(1),
        Vec::new(),
    );
    let (mut hosted, mut joined) = (None, None);
    let start = Instant::now();
    while hosted.is_none() || joined.is_none() {
        assert!(start.elapsed() < Duration::from_secs(10), "no handshake");
        if joined.is_none()
            && let Some(Answer::Welcomed(agreed)) = joiner.poll(Instant::now())
        {
            joined = Some(agreed);
        }
        if let Some(agreed) = host.poll() {
            hosted = Some(agreed);
        }
        net.advance();
        std::thread::sleep(Duration::from_millis(1));
    }
    let (hosted, joined) = (hosted.unwrap(), joined.unwrap());
    // Each knows the other by its NAT's address.
    assert_eq!(hosted.peer, guest_code.0[0]);
    assert_eq!(joined.peer, host_code.0[0]);

    let settings = SessionSettings::default();
    let mut host_app = app();
    let mut guest_app = app();
    let host_session = Session::p2p(host_link, &hosted, Some(host), settings)
        .unwrap()
        .with_local_input(scripted);
    let guest_session = Session::p2p(guest_link, &joined, None, settings)
        .unwrap()
        .with_local_input(scripted);
    start_session(host_app.world_mut(), host_session);
    start_session(guest_app.world_mut(), guest_session);
    let target = 120;
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(60), "too slow");
        host_app.world_mut().run_schedule(FixedMain);
        guest_app.world_mut().run_schedule(FixedMain);
        net.advance();
        let confirmed = |app: &App| session(app).confirmed_tick().unwrap_or(0);
        if confirmed(&host_app) >= target && confirmed(&guest_app) >= target {
            break;
        }
        std::thread::sleep(Duration::from_micros(200));
    }
    let expected = reference(target, settings.input_delay as u64);
    assert_eq!(
        session(&host_app).confirmed_checksum(target),
        Some(expected)
    );
    assert_eq!(
        session(&guest_app).confirmed_checksum(target),
        Some(expected)
    );
}

#[test]
fn a_host_behind_a_nat_hears_no_one_until_it_punches_to_them() {
    let net = MemoryNet::new(1, 0.0, 9);
    let ((host_link, host_code), (guest_link, guest_code)) = behind_nats(&net);
    let mut host = Host::new(host_link, terms(1), Vec::new());
    let mut joiner = Joiner::to_any(guest_link, host_code.0, terms(1), Vec::new());
    let mut now = Instant::now();
    let mut poll = |host: &mut Host, joiner: &mut Joiner, steps: u32| {
        for _ in 0..steps {
            now += handshake::RESEND;
            if let Some(answer) = joiner.poll(now) {
                return Some(answer);
            }
            host.poll();
            net.advance();
        }
        None
    };
    assert_eq!(poll(&mut host, &mut joiner, 20), None);
    host.punch_to(guest_code.0);
    assert!(matches!(
        poll(&mut host, &mut joiner, 20),
        Some(Answer::Welcomed(_))
    ));
}

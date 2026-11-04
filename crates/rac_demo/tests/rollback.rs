//! The whole demo under rollback (PLAN.md P1.18, `docs/netcode.md`): two heroes, each
//! with his own camera setting, break crates, fire every gun at the dummies, shop and
//! fight the arena's enemies, under `rac_net`'s sync test, which rolls every tick back and
//! runs it again, and as two peers on a slow, lossy network. Under the sync test, effects
//! and sounds are given what a plain run gives them, once. The demo runs as the app has
//! it, HUD, rig and effects included, without a window or a renderer.

use bevy::{
    asset::AssetPlugin, ecs::system::RunSystemOnce, math::ops,
    mesh::skinning::SkinnedMeshInverseBindposes, prelude::*, time::TimeUpdateStrategy,
};
use rac_combat::pickups::{CrateBlast, PickupSound, PickupSoundKind, VendorSound};
use rac_combat::{Armory, Arsenal, Crate, Destroyed, GunEvent, GunEventKind};
use rac_demo::{Bolts, DemoGunLevel, DemoPlugin, DemoSettings, Enemy, spawn_demo};
use rac_hero::{CameraSetting, FollowCamera, Health, Hero, HeroProfile, HeroTuning};
use rac_input::{Action, Buttons, LocalPlayer, PlayerInput, PlayerInputs};
use rac_net::{
    Answer, Host, Joiner, Link, MemoryNet, NetPlugin, Session, SessionSettings, SessionStatus,
    Terms, end_session, start_session,
};
use rac_sim::{ShownReader, SimPlugin, SimSet, SimTick, SimTransform, TICK_HZ};
use std::{
    collections::BTreeSet,
    net::SocketAddr,
    time::{Duration, Instant},
};

/// The settings that change the simulation, which peers agree on.
struct Rules {
    profile: HeroProfile,
    armory: Armory,
    gun_level: u8,
    /// Each player's camera setting; `None` is the profile's.
    cameras: [Option<CameraSetting>; 2],
}

impl Rules {
    /// Player 2 looks out of his eyes.
    fn uya(gun_level: u8) -> Self {
        Self {
            profile: HeroProfile::uya(),
            armory: Armory::uya(),
            gun_level,
            cameras: [None, Some(CameraSetting::FirstPerson)],
        }
    }

    /// Player 1 has DL's lock-strafe camera, player 2 the follow camera.
    fn dl() -> Self {
        Self {
            profile: HeroProfile::dl(),
            armory: Armory::dl(),
            gun_level: 0,
            cameras: [None, Some(CameraSetting::ThirdPerson)],
        }
    }
}

/// What the run did, seen from outside the simulation: so a test knows its script
/// reached what it was meant to.
#[derive(Resource, Debug, Default)]
struct Seen {
    /// Each player's guns that fired, by index.
    fired: BTreeSet<(usize, usize)>,
    enemies_destroyed: BTreeSet<Entity>,
    hurt: BTreeSet<usize>,
    vendor: BTreeSet<VendorSound>,
    /// Players whose quick-select ring opened.
    ring: BTreeSet<usize>,
    /// Gun events written, as many times as their ticks ran.
    gun_events_written: usize,
    /// What effects and sounds were given, read as they read it.
    shown: Shown,
}

#[derive(Debug, Default, PartialEq)]
struct Shown {
    guns: Vec<GunEvent>,
    sounds: Vec<PickupSound>,
    blasts: Vec<CrateBlast>,
}

fn see(
    mut guns: MessageReader<GunEvent>,
    mut destroyed: MessageReader<Destroyed>,
    mut pickups: MessageReader<PickupSound>,
    enemies: Query<(), With<Enemy>>,
    heroes: Query<(&Hero, &Health, &Arsenal)>,
    mut seen: ResMut<Seen>,
) {
    for event in guns.read() {
        seen.gun_events_written += 1;
        if matches!(
            event.kind,
            GunEventKind::Fired { .. }
                | GunEventKind::Ray { .. }
                | GunEventKind::Sucking { .. }
                | GunEventKind::Arc { .. }
        ) {
            seen.fired.insert((event.player, event.weapon.0));
        }
    }
    for event in destroyed.read() {
        if enemies.contains(event.target) {
            seen.enemies_destroyed.insert(event.target);
        }
    }
    for event in pickups.read() {
        if let PickupSoundKind::Vendor(sound) = event.kind {
            seen.vendor.insert(sound);
        }
    }
    for (hero, health, arsenal) in &heroes {
        if health.current < health.max {
            seen.hurt.insert(hero.player);
        }
        if arsenal.quick.open.is_some() {
            seen.ring.insert(hero.player);
        }
    }
}

fn show(
    mut guns: ShownReader<GunEvent>,
    mut sounds: ShownReader<PickupSound>,
    mut blasts: ShownReader<CrateBlast>,
    mut seen: ResMut<Seen>,
) {
    seen.shown.guns.extend(guns.read());
    seen.shown.sounds.extend(sounds.read());
    seen.shown.blasts.extend(blasts.read());
}

/// The demo with two heroes, this machine playing `local`'s, without a window or a
/// renderer: one tick per update.
fn demo_app(rules: Rules, local: usize) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .init_asset::<SkinnedMeshInverseBindposes>()
        // `UiPlugin`'s, which the HUD's sights read.
        .init_resource::<UiScale>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / TICK_HZ,
        )))
        .add_plugins((SimPlugin, DemoPlugin, NetPlugin))
        .init_resource::<PlayerInputs>()
        .insert_resource(HeroTuning(rules.profile))
        .insert_resource(rules.armory)
        .insert_resource(DemoGunLevel(rules.gun_level))
        .insert_resource(DemoSettings {
            players: 2,
            camera: rules.cameras[local],
            others_camera: rules.cameras[1 - local],
            ..default()
        })
        .insert_resource(LocalPlayer(local))
        .init_resource::<Seen>()
        .add_systems(Update, (see, show));
    app.update(); // Startup, and starts the clock.
    app.world_mut().run_system_once(spawn_demo).unwrap();
    app
}

/// Where a hero heads, how close he stops, for how long, and what he does on the way.
#[derive(Clone, Copy)]
struct Leg {
    to: Vec2,
    within: f32,
    ticks: u64,
    doing: Doing,
}

#[derive(Clone, Copy, PartialEq)]
enum Doing {
    /// Swings the wrench, and jumps now and then.
    Wrench,
    /// Fires the gun in hand in bursts, and changes to the next every so often.
    Guns,
    /// The same, locked on.
    LockedGuns,
    /// Buys rounds twice in the vendor's menu, then closes it.
    Shop,
    /// Holds quick select for the ring, chooses the gun to its right, and fires it.
    Ring,
}

/// How long each gun is fired before the next.
const TICKS_A_GUN: u64 = 72;

/// Each hero's route: the hazard pad (before the Shield Charger's shield can take its
/// hits), the yard's bolt and ammo crates with the wrench, every gun at the dummies, the
/// explosive crates, the quick-select ring, the vendor, then the arena's enemies.
/// Positions are x and z.
fn route(player: usize, guns: usize) -> [Leg; 9] {
    let leg = |x: f32, z: f32, within: f32, ticks: u64, doing| Leg {
        to: Vec2::new(x, z),
        within,
        ticks,
        doing,
    };
    let side = if player == 0 { -1.0 } else { 1.0 };
    let all_guns = TICKS_A_GUN * guns as u64;
    [
        leg(6.0 + 0.8 * side, -24.0, 0.4, 330, Doing::Wrench),
        leg(2.0 * side, 8.0, 1.6, 330, Doing::Wrench),
        leg(4.0 * side, 8.0, 1.6, 120, Doing::Wrench),
        leg(-8.0, 10.0 + 4.0 * player as f32, 1.6, 150, Doing::Wrench),
        leg(1.5 * side, 14.0, 5.0, all_guns, Doing::Guns),
        leg(7.0 + 1.3 * player as f32, 8.0, 6.0, 120, Doing::Guns),
        leg(7.0 + 1.3 * player as f32, 8.0, 6.0, 90, Doing::Ring),
        leg(-9.0 + 0.5 * side, 3.0, 0.4, 240, Doing::Shop),
        leg(
            24.0,
            16.0 + 6.0 * player as f32,
            3.0,
            600,
            Doing::LockedGuns,
        ),
    ]
}

/// How many ticks the whole route takes.
fn route_ticks(guns: usize) -> u64 {
    route(0, guns).iter().map(|leg| leg.ticks).sum()
}

/// The move stick that heads `player`'s hero for `to` as his camera faces, at full tilt.
fn steer(world: &mut World, player: usize, to: Vec2, within: f32) -> [i8; 2] {
    let mut heroes = world.query::<(&Hero, &SimTransform)>();
    let Some(at) = heroes
        .iter(world)
        .find(|(hero, _)| hero.player == player)
        .map(|(_, transform)| transform.translation.xz())
    else {
        return [0, 0];
    };
    let mut cameras = world.query::<&FollowCamera>();
    let yaw = cameras
        .iter(world)
        .find(|camera| camera.player == player)
        .map_or(0.0, |camera| camera.yaw);
    let way = to - at;
    if way.length() < within {
        return [0, 0];
    }
    // The controller's basis (`stick_on_ground`): yaw 0 looks down −z.
    let forward = Vec2::new(-ops::sin(yaw), -ops::cos(yaw));
    let right = Vec2::new(ops::cos(yaw), -ops::sin(yaw));
    let stick = Vec2::new(way.dot(right), way.dot(forward));
    let stick = stick / stick.x.abs().max(stick.y.abs()) * 127.0;
    [stick.x.round() as i8, stick.y.round() as i8]
}

/// What `player` does this tick: the route's leg for the tick, then standing still.
fn scripted(world: &mut World, player: usize) -> PlayerInput {
    let tick = world.resource::<SimTick>().0;
    let guns = world.resource::<Armory>().weapons.len();
    let mut start = 0;
    let Some((leg, t)) = route(player, guns).into_iter().find_map(|leg| {
        let t = tick.checked_sub(start).filter(|&t| t < leg.ticks);
        start += leg.ticks;
        t.map(|t| (leg, t))
    }) else {
        return PlayerInput::default();
    };
    let mut move_stick = steer(world, player, leg.to, leg.within);
    let arrived = move_stick == [0, 0];
    let mut held = Buttons::NONE;
    let mut pressed = Buttons::NONE;
    let mut press = |action| {
        held.insert(action);
        pressed.insert(action);
    };
    match leg.doing {
        Doing::Wrench => {
            // Only once there: a swing stops him.
            if arrived && t % 24 == 0 {
                press(Action::Melee);
            }
            if t % 90 == 45 {
                press(Action::Jump);
            }
        }
        Doing::Guns | Doing::LockedGuns => {
            // Three bursts, the first while a changed gun is still coming out, and one long
            // enough to charge (the Spitting Hydra fizzles if let go before 20 ticks).
            let slot = t % TICKS_A_GUN;
            if [0, 18, 46].contains(&slot) {
                press(Action::Fire);
            } else if slot < 12 || (18..42).contains(&slot) || (46..64).contains(&slot) {
                held.insert(Action::Fire);
            } else if slot == TICKS_A_GUN - 6 {
                press(Action::NextWeapon);
            }
            if leg.doing == Doing::LockedGuns {
                held.insert(Action::LockOn);
                if t == 0 {
                    pressed.insert(Action::LockOn);
                }
            }
        }
        Doing::Shop => match t {
            // On the pad by now: a rounds row, how many, bought; the next row, again.
            130 | 145 | 175 | 190 => press(Action::Jump),
            160 => press(Action::NextWeapon),
            // Backs out of a count if one is left open, and closes the list.
            205 | 220 => press(Action::QuickSelect),
            _ => {}
        },
        Doing::Ring => {
            if t == 0 {
                press(Action::QuickSelect);
            }
            if t == 55 {
                press(Action::Fire);
            }
            // Held past the ring's delay it opens, the stick points at a slot, and letting
            // go takes that gun.
            if (1..45).contains(&t) {
                held.insert(Action::QuickSelect);
            }
            if (20..45).contains(&t) {
                move_stick = [127, 0];
            }
            if (56..80).contains(&t) {
                held.insert(Action::Fire);
            }
        }
    }
    PlayerInput {
        move_stick,
        held,
        pressed,
        ..default()
    }
}

fn status(app: &App) -> Option<SessionStatus> {
    app.world().get_resource::<SessionStatus>().copied()
}

fn session(app: &App) -> &Session {
    app.world().resource::<Session>()
}

fn crates(app: &mut App) -> usize {
    app.world_mut().query::<&Crate>().iter(app.world()).count()
}

fn bolts(app: &mut App) -> u32 {
    app.world_mut()
        .query::<&Bolts>()
        .iter(app.world())
        .map(|bolts| bolts.0)
        .sum()
}

/// Plays the route under the sync test, checking each tick comes out the same when it
/// is rolled back 7 ticks and run again, and that the route did what it is for.
fn sync_test(rules: Rules) -> Seen {
    let guns = rules.armory.weapons.len();
    let mut app = demo_app(rules, 0);
    let crates_before = crates(&mut app);
    let sync_test = Session::sync_test(2, 7).unwrap().with_local_input(scripted);
    start_session(app.world_mut(), sync_test);
    let ticks = route_ticks(guns);
    for _ in 0..ticks {
        app.update();
        assert_eq!(
            status(&app),
            Some(SessionStatus::Running),
            "at tick {}",
            app.world().resource::<SimTick>().0
        );
    }
    assert_eq!(app.world().resource::<SimTick>().0, ticks);

    let broken = crates_before - crates(&mut app);
    let bolts = bolts(&mut app);
    let seen = app.world_mut().remove_resource::<Seen>().unwrap();
    for player in 0..2 {
        let missed: Vec<_> = (0..guns)
            .filter(|&gun| !seen.fired.contains(&(player, gun)))
            .collect();
        assert!(missed.is_empty(), "player {player} never fired {missed:?}");
    }
    assert!(broken >= 6, "{broken} crates broken");
    assert!(bolts > 0);
    assert!(!seen.enemies_destroyed.is_empty(), "no enemy destroyed");
    assert_eq!(seen.hurt.len(), 2, "heroes hurt: {:?}", seen.hurt);
    for sound in [VendorSound::Opened, VendorSound::Closed] {
        assert!(seen.vendor.contains(&sound), "{:?}", seen.vendor);
    }
    assert_eq!(seen.ring.len(), 2, "rings opened: {:?}", seen.ring);
    end_session(app.world_mut());
    seen
}

/// Gives both heroes the script's input before each tick, as the sync test's session
/// does.
fn feed(world: &mut World) {
    let inputs = [scripted(world, 0), scripted(world, 1)];
    world.resource_mut::<PlayerInputs>().0[..2].copy_from_slice(&inputs);
}

/// Plays the route without a session, each tick run once.
fn plain_run(rules: Rules) -> Seen {
    let ticks = route_ticks(rules.armory.weapons.len());
    let mut app = demo_app(rules, 0);
    app.add_systems(FixedFirst, feed);
    while app.world().resource::<SimTick>().0 < ticks {
        app.update();
    }
    app.world_mut().remove_resource::<Seen>().unwrap()
}

/// Effects and sounds were given each shot, sound and blast once, as a plain run gives
/// them, though the sync test wrote them again each time it ran their ticks.
fn shown_once(rules: Rules, seen: &Seen) {
    let plain = plain_run(rules);
    assert!(!seen.shown.guns.is_empty() && !seen.shown.blasts.is_empty());
    assert!(seen.gun_events_written > 2 * seen.shown.guns.len());
    assert_eq!(seen.shown.guns.len(), plain.shown.guns.len());
    assert_eq!(seen.shown.sounds.len(), plain.shown.sounds.len());
    assert_eq!(seen.shown.blasts.len(), plain.shown.blasts.len());
    assert!(seen.shown == plain.shown, "shown differently");
}

#[test]
fn the_demo_with_uyas_moves_and_guns_at_v5_rolls_back_exactly() {
    let seen = sync_test(Rules::uya(4));
    shown_once(Rules::uya(4), &seen);
    assert!(
        seen.vendor.contains(&VendorSound::Bought),
        "{:?}",
        seen.vendor
    );
}

/// The kit's DL guns have no rounds for sale yet, so the vendor only refuses.
#[test]
fn the_demo_with_deadlockeds_moves_and_guns_rolls_back_exactly() {
    let seen = sync_test(Rules::dl());
    shown_once(Rules::dl(), &seen);
}

/// State a snapshot doesn't hold.
#[derive(Resource, Default)]
struct Unsaved(u32);

/// Moves the first hero by a count that a rollback doesn't take back.
fn drift(mut unsaved: ResMut<Unsaved>, mut heroes: Query<(&Hero, &mut SimTransform)>) {
    unsaved.0 += 1;
    for (hero, mut transform) in &mut heroes {
        if hero.player == 0 {
            transform.translation.x += unsaved.0 as f32 * 1e-4;
        }
    }
}

/// The checks above have teeth: a change the snapshot misses is found within a few ticks.
#[test]
fn the_sync_test_finds_state_left_out_of_the_snapshot() {
    let mut app = demo_app(Rules::uya(2), 0);
    app.init_resource::<Unsaved>()
        .add_systems(FixedUpdate, drift.in_set(SimSet::Particles));
    let sync_test = Session::sync_test(2, 7).unwrap().with_local_input(scripted);
    start_session(app.world_mut(), sync_test);
    for _ in 0..30 {
        app.update();
    }
    assert!(
        matches!(status(&app), Some(SessionStatus::Desynced { .. })),
        "{:?}",
        status(&app)
    );
}

fn address(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

/// Two peers' demos on a network 3 steps slow that loses 5% of packets: the confirmed
/// state is the same on both, though each predicted the other's input and rolled back
/// when it guessed wrong, and each spawned its own HUD among the level's entities, as
/// the app does, so their `Entity` values differ.
#[test]
fn two_peers_on_a_slow_lossy_network_play_the_demo_the_same() {
    let net = MemoryNet::new(3, 0.05, 23);
    let terms = Terms {
        build: "test".into(),
        rules: 1,
    };
    let host_link = Link::new(net.endpoint(address(7000)));
    let guest_link = Link::new(net.endpoint(address(7001)));
    let mut host = Host::new(host_link.clone(), terms.clone(), Vec::new());
    let mut joiner = Joiner::new(guest_link.clone(), address(7000), terms, Vec::new());
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

    let guns = Armory::uya().weapons.len();
    let settings = SessionSettings::default();
    let mut host_app = demo_app(Rules::uya(2), hosted.slot);
    let mut guest_app = demo_app(Rules::uya(2), joined.slot);
    let host_session = Session::p2p(host_link, &hosted, Some(host), settings)
        .unwrap()
        .with_local_input(scripted);
    let guest_session = Session::p2p(guest_link, &joined, None, settings)
        .unwrap()
        .with_local_input(scripted);
    start_session(host_app.world_mut(), host_session);
    start_session(guest_app.world_mut(), guest_session);

    // Into the arena, past the guns at the dummies.
    let target = route_ticks(guns) - 300;
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(300), "too slow");
        host_app.update();
        guest_app.update();
        net.advance();
        for app in [&host_app, &guest_app] {
            assert!(
                !matches!(status(app), Some(SessionStatus::Desynced { .. })),
                "{:?}",
                status(app)
            );
        }
        let confirmed = |app: &App| session(app).confirmed_tick().unwrap_or(0);
        if confirmed(&host_app) >= target && confirmed(&guest_app) >= target {
            break;
        }
        // ggrs times its resends by the clock.
        std::thread::sleep(Duration::from_micros(200));
    }
    let checksum = session(&host_app).confirmed_checksum(target);
    assert!(checksum.is_some());
    assert_eq!(checksum, session(&guest_app).confirmed_checksum(target));
}

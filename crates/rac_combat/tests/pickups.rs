//! Pickups, crates and the vendor through the whole plugin (PLAN.md P1.10): a crate the
//! wrench breaks scatters ammo for his gun that fills it as he takes it, a bolt crate's
//! bolts reach his purse, ammo that doesn't fit is lost, the vendor's pad sells rounds
//! for bolts, explosive crates go off, nanotech crates heal and doubler crates double. All
//! on synthetic floors.

use bevy::{prelude::*, time::TimeUpdateStrategy};
use rac_combat::pickups::{Blast, BoltMultiplier, Fuse, NanotechOrbs, PickupMotion, PickupTuning};
use rac_combat::weapons::Hand;
use rac_combat::{
    Armory, Arsenal, Bolts, CombatPlugin, Crate, CrateKind, Destroyed, Pickup, PickupKind, Vendor,
    WeaponId, target_bundle,
};
use rac_hero::{Collider, Health, HeroPlugin, HeroProfile, hero_bundle};
use rac_input::{Action, Buttons, PlayerInput, PlayerInputs};
use rac_sim::{SimPlugin, SimTransform, TICK_HZ};
use std::time::Duration;

const N60: WeaponId = WeaponId(1);

/// One tick per update and a floor.
fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, SimPlugin, HeroPlugin, CombatPlugin))
        .init_resource::<PlayerInputs>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / TICK_HZ,
        )));
    app.world_mut().spawn((
        Collider::Cuboid {
            half_extents: Vec3::new(50.0, 0.5, 50.0),
        },
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    app
}

/// The hero of `player` standing at `at` facing −z, owning the N60 at V1 with `ammo`
/// rounds, and `bolts`.
fn hero(app: &mut App, player: usize, at: Vec3, ammo: u32, bolts: u32) -> Entity {
    let armory = Armory::uya();
    let mut arsenal = Arsenal::default();
    arsenal.give(N60, &armory);
    arsenal.gun_mut(N60).unwrap().ammo = ammo;
    let profile = HeroProfile::uya();
    app.world_mut()
        .spawn((
            hero_bundle(player, at, 0.0, -20.0, &profile),
            arsenal,
            Bolts(bolts),
        ))
        .id()
}

/// A crate of `kind` whose middle is `centre`, as the demo builds them.
fn crate_at(app: &mut App, kind: CrateKind, centre: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            target_bundle(0.55, 0.0, 1.0),
            Crate(kind),
            Transform::from_translation(centre),
        ))
        .id()
}

fn ticks(app: &mut App, count: usize) {
    for _ in 0..count {
        app.update();
    }
}

fn ammo(app: &App, hero: Entity) -> u32 {
    app.world()
        .get::<Arsenal>(hero)
        .unwrap()
        .gun(N60)
        .unwrap()
        .ammo
}

fn pickups(app: &mut App) -> Vec<PickupMotion> {
    app.world_mut()
        .query::<&Pickup>()
        .iter(app.world())
        .map(|pickup| pickup.0)
        .collect()
}

#[test]
fn a_crate_the_wrench_breaks_scatters_ammo_for_his_gun_that_he_collects() {
    let mut app = app();
    let hero = hero(&mut app, 0, Vec3::ZERO, 10, 0);
    let target = crate_at(&mut app, CrateKind::Ammo, Vec3::new(0.0, 0.45, -1.6));
    app.update(); // Starts the clock.

    let mut buttons = Buttons::NONE;
    buttons.insert(Action::Melee);
    app.world_mut().resource_mut::<PlayerInputs>().0[0] = PlayerInput {
        held: buttons,
        pressed: buttons,
        ..default()
    };
    app.update();
    app.world_mut().resource_mut::<PlayerInputs>().0[0] = PlayerInput::default();
    let mut broke = None;
    for tick in 0..40 {
        app.update();
        if app.world().get_entity(target).is_err() {
            broke = Some(tick);
            break;
        }
    }
    assert!(broke.is_some(), "the swing never broke the crate");

    let armory = Armory::uya();
    let rounds = armory.get(N60).unwrap().pickup_rounds;
    let scattered = pickups(&mut app);
    let count = scattered.len() as u32;
    assert!((1..=2).contains(&count), "{count}");
    assert!(scattered.iter().all(|pickup| pickup.kind
        == PickupKind::Ammo {
            weapon: N60,
            rounds
        }));
    // He stands in reach, so each is his as it lands, and then flies in.
    let mut filled = None;
    for tick in 0..120 {
        app.update();
        if filled.is_none() && ammo(&app, hero) == 10 + count * rounds {
            filled = Some(tick);
        }
    }
    assert!(pickups(&mut app).is_empty());
    assert_eq!(ammo(&app, hero), 10 + count * rounds);
    assert!(filled.is_some_and(|tick| tick < 60), "{filled:?}");
}

#[test]
fn a_bolt_crate_s_bolts_reach_his_purse() {
    let mut app = app();
    let hero = hero(&mut app, 0, Vec3::ZERO, 150, 7);
    let target = crate_at(&mut app, CrateKind::Bolts, Vec3::new(2.0, 0.45, 0.0));
    app.update();
    // Its hit points run out as the combat code would end them.
    app.world_mut().write_message(Destroyed {
        target,
        player: 0,
        weapon: None,
    });
    app.update();
    assert!(app.world().get_entity(target).is_err());
    let scattered: u32 = pickups(&mut app)
        .iter()
        .map(|pickup| match pickup.kind {
            PickupKind::Bolts(value) => value,
            other => panic!("{other:?}"),
        })
        .sum();
    let table = PickupTuning::default().crates;
    let spread = table.bolts / 4;
    assert!((table.bolts - spread..=table.bolts + spread).contains(&scattered));
    ticks(&mut app, 120);
    assert!(pickups(&mut app).is_empty());
    let bolts = app.world().get::<Bolts>(hero).unwrap().0;
    assert_eq!(bolts, 7 + scattered);
}

#[test]
fn ammo_goes_to_the_nearest_hero_with_room_and_what_does_not_fit_is_lost() {
    let mut app = app();
    // Player 1 is nearer but his N60 is full; player 0 has room for 5 rounds.
    let room = hero(&mut app, 0, Vec3::new(3.0, 0.0, 0.0), 145, 0);
    let full = hero(&mut app, 1, Vec3::new(-1.0, 0.0, 0.0), 150, 0);
    let tuning = PickupTuning::default();
    let motion = PickupMotion {
        landed: true,
        ..PickupMotion::new(
            PickupKind::Ammo {
                weapon: N60,
                rounds: 30,
            },
            Vec3::new(0.0, tuning.radius, 0.0),
            Vec3::ZERO,
        )
    };
    app.world_mut().spawn((
        Pickup(motion),
        SimTransform::from_translation(motion.position),
    ));
    app.update();
    app.update();
    assert_eq!(ammo(&app, room), 150);
    assert_eq!(ammo(&app, full), 150);
    // It flies in for show, then goes; the 25 rounds that didn't fit are gone with it.
    assert!(pickups(&mut app).iter().all(|pickup| pickup.taken));
    ticks(&mut app, 120);
    assert!(pickups(&mut app).is_empty());
}

#[test]
fn the_vendor_s_pad_sells_rounds_for_the_gun_in_hand_while_he_stands_on_it() {
    let mut app = app();
    let hero = hero(&mut app, 0, Vec3::ZERO, 140, 100);
    app.world_mut().get_mut::<Arsenal>(hero).unwrap().hand = Hand::Gun {
        weapon: N60,
        ticks: 100,
    };
    let vendor = app
        .world_mut()
        .spawn((
            Vendor {
                half_extents: Vec3::new(1.5, 2.0, 1.5),
            },
            Transform::from_xyz(10.0, 0.0, 0.0),
        ))
        .id();
    app.update();
    ticks(&mut app, 30);
    // Off the pad, nothing.
    assert_eq!(ammo(&app, hero), 140);
    app.world_mut()
        .get_mut::<Transform>(vendor)
        .unwrap()
        .translation = Vec3::ZERO;
    ticks(
        &mut app,
        5 * PickupTuning::default().vendor.round_ticks as usize,
    );
    let bought = ammo(&app, hero) - 140;
    assert!((4..=6).contains(&bought), "{bought}");
    // Up to the maximum, at the gun's price a round.
    ticks(&mut app, 60);
    assert_eq!(ammo(&app, hero), 150);
    let price = Armory::uya().get(N60).unwrap().level(0).round_price;
    assert_eq!(app.world().get::<Bolts>(hero).unwrap().0, 100 - 10 * price);
}

/// An explosive crate of the demo's size whose middle is `centre`, solid to the hero.
fn explosive_at(app: &mut App, centre: Vec3) -> Entity {
    let crate_ = crate_at(app, CrateKind::Explosive, centre);
    app.world_mut().entity_mut(crate_).insert(Collider::Cuboid {
        half_extents: Vec3::splat(0.45),
    });
    crate_
}

fn health(app: &App, hero: Entity) -> u32 {
    app.world().get::<Health>(hero).unwrap().current
}

fn break_crate(app: &mut App, target: Entity) {
    app.world_mut().write_message(Destroyed {
        target,
        player: 0,
        weapon: None,
    });
}

#[test]
fn a_hit_explosive_crate_goes_off_in_a_few_ticks_setting_off_its_neighbour_and_hurting_him() {
    let mut app = app();
    let hero = hero(&mut app, 0, Vec3::ZERO, 10, 0);
    let first = explosive_at(&mut app, Vec3::new(0.0, 0.45, -2.5));
    let second = explosive_at(&mut app, Vec3::new(1.5, 0.45, -2.5));
    app.update(); // Starts the clock.
    let most = health(&app, hero);
    break_crate(&mut app, first);
    app.update();
    let lit = app
        .world()
        .get::<Fuse>(first)
        .expect("a hit lights its fuse")
        .left;
    assert!((5..=14).contains(&lit), "{lit}");
    assert!(app.world().get_entity(second).is_ok());
    let went = (0..20).find(|_| {
        app.update();
        app.world().get_entity(first).is_err()
    });
    assert!(went.is_some(), "it never went off");
    // Its blast reaches the second within 2 u by about half its growth, which then goes
    // off a tick later.
    let second_went = (0..30).find(|_| {
        app.update();
        app.world().get_entity(second).is_err()
    });
    assert!(second_went.is_some_and(|tick| tick < 15), "{second_went:?}");
    ticks(&mut app, 30);
    // Hurt once for 4, the second blast coming while he can't be hurt again.
    assert_eq!(health(&app, hero), most - 4);
}

#[test]
fn a_touched_explosive_crate_goes_off_three_seconds_on() {
    let mut app = app();
    hero(&mut app, 0, Vec3::ZERO, 10, 0);
    // Its near face just past his capsule's radius.
    let target = explosive_at(&mut app, Vec3::new(0.0, 0.45, -0.92));
    app.update(); // Starts the clock.
    ticks(&mut app, 2);
    let lit = app
        .world()
        .get::<Fuse>(target)
        .expect("touching lights it")
        .left;
    assert!((176..=179).contains(&lit), "{lit}");
    ticks(&mut app, lit as usize - 1);
    assert!(app.world().get_entity(target).is_ok());
    ticks(&mut app, 2);
    assert!(app.world().get_entity(target).is_err());
    let blasts = app.world_mut().query::<&Blast>().iter(app.world()).count();
    assert_eq!(blasts, 1);
}

#[test]
fn a_broken_nanotech_crate_heals_a_hurt_hero_near_it_a_fifth_and_waits_for_a_full_one() {
    let mut app = app();
    let hero = hero(&mut app, 0, Vec3::ZERO, 10, 0);
    let target = crate_at(&mut app, CrateKind::Nanotech, Vec3::new(0.0, 0.45, -4.0));
    app.update(); // Starts the clock.
    break_crate(&mut app, target);
    ticks(&mut app, 100);
    // He was full: they wait.
    let orbs = |app: &mut App| {
        app.world_mut()
            .query::<&NanotechOrbs>()
            .iter(app.world())
            .count()
    };
    assert_eq!(orbs(&mut app), 1);
    let most = health(&app, hero);
    app.world_mut().get_mut::<Health>(hero).unwrap().current = most - 5;
    ticks(&mut app, 90);
    assert_eq!(health(&app, hero), most - 5 + most / 5);
    assert_eq!(orbs(&mut app), 0);
}

#[test]
fn a_doubler_crate_doubles_the_bolts_of_the_crates_he_breaks() {
    let mut app = app();
    let hero = hero(&mut app, 0, Vec3::ZERO, 10, 0);
    let doubler = crate_at(&mut app, CrateKind::Doubler, Vec3::new(0.0, 0.45, -4.0));
    let bolts = crate_at(&mut app, CrateKind::Bolts, Vec3::new(2.0, 0.45, -4.0));
    app.update(); // Starts the clock.
    break_crate(&mut app, doubler);
    app.update();
    let doubled = *app.world().get::<BoltMultiplier>(hero).unwrap();
    assert_eq!(doubled.exp, 1);
    assert!(doubled.ticks > 1790, "{}", doubled.ticks);
    break_crate(&mut app, bolts);
    app.update();
    let total: u32 = pickups(&mut app)
        .iter()
        .map(|pickup| match pickup.kind {
            PickupKind::Bolts(value) => value,
            other => panic!("{other:?}"),
        })
        .sum();
    assert!(
        (66..=110).contains(&total) && total.is_multiple_of(2),
        "{total}"
    );
}

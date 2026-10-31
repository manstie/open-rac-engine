//! The demo's enemies (PLAN.md P1.11): their states driven through the pure step on
//! synthetic ground, then a charger in an app with the hero, the wrench and its drops.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use rac_combat::{Afflicted, Affliction, CombatPlugin, Destroyed, Hitpoints, Hurtbox, WeaponId};
use rac_demo::enemy::brain::{
    self, AttackPhase, EnemyMotion, EnemyState, EnemyView, HeroShape, HeroView, Outcome,
    Surroundings,
};
use rac_demo::enemy::def::{Attack, EnemyDef};
use rac_demo::{Bestiary, Bolts, EnemyBody, EnemyKind, EnemyPlugin, Pickup, enemy_bundle};
use rac_hero::{BodyPlacement, Collider, Health, HeroPlugin, HeroProfile, ParryWorld, hero_bundle};
use rac_input::{Action, Buttons, PlayerInput, PlayerInputs};
use rac_sim::{SimPlugin, SimRng, TICK_HZ};
use std::f32::consts::PI;
use std::time::Duration;

const SHAPE: HeroShape = HeroShape {
    radius: 0.45,
    bottom: 0.5,
    top: 1.05,
};

/// The enemy a test steps, and another, as they know each other.
const ME: Entity = Entity::from_raw_u32(1).unwrap();
const OTHER: Entity = Entity::from_raw_u32(2).unwrap();

fn floor() -> BodyPlacement {
    BodyPlacement::fixed(
        Collider::Cuboid {
            half_extents: Vec3::new(80.0, 0.5, 80.0),
        },
        Vec3::new(0.0, -0.5, 0.0),
        Quat::IDENTITY,
    )
}

/// The floor and a wall 4 u wide and `height` tall across −z, its near face at z = `z`.
fn walled(z: f32, height: f32) -> ParryWorld {
    ParryWorld::new(&[
        floor(),
        BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(2.0, height / 2.0, 0.5),
            },
            Vec3::new(0.0, height / 2.0, z - 0.5),
            Quat::IDENTITY,
        ),
    ])
}

fn open() -> ParryWorld {
    ParryWorld::new(&[floor()])
}

fn hero(player: usize, x: f32, z: f32) -> HeroView {
    HeroView {
        player,
        position: Vec3::new(x, 0.0, z),
        alive: true,
    }
}

/// An enemy at the origin facing −z, on no patrol.
fn standing() -> EnemyMotion {
    EnemyMotion::new(Vec3::ZERO, 0.0, Vec::new())
}

fn tick(
    enemy: &mut EnemyMotion,
    def: &EnemyDef,
    heroes: &[HeroView],
    world: &ParryWorld,
    rng: &mut SimRng,
) -> Outcome {
    let around = Surroundings {
        heroes,
        hero_shape: SHAPE,
        me: ME,
        enemies: &[],
        gravity: 30.0,
        world,
    };
    brain::step(enemy, def, &around, rng)
}

#[test]
fn an_enemy_walks_its_patrol_and_waits_at_each_point() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    let world = open();
    let mut rng = SimRng::new(3, 0);
    let mut enemy = EnemyMotion::new(Vec3::ZERO, 0.0, vec![Vec3::new(4.0, 0.0, 0.0), Vec3::ZERO]);
    let mut arrivals = Vec::new();
    for t in 0..900 {
        let before = enemy.state;
        tick(&mut enemy, def, &[], &world, &mut rng);
        assert!(enemy.position.y.abs() < 1e-3, "{}", enemy.position);
        if before == EnemyState::Patrol && enemy.state == EnemyState::Idle {
            arrivals.push((t, enemy.position, enemy.wait));
        }
    }
    assert!(arrivals.len() >= 3, "{arrivals:?}");
    let (low, high) = def.moves.wait_ticks;
    for (i, (_, at, wait)) in arrivals.iter().enumerate() {
        let point = if i % 2 == 0 {
            Vec3::new(4.0, 0.0, 0.0)
        } else {
            Vec3::ZERO
        };
        assert!(at.distance(point) <= 0.5, "{i}: {at}");
        assert!((low..=high).contains(wait));
    }
    // It waits at least the shortest wait, then walks 4 u at its walking speed.
    let gap = arrivals[1].0 - arrivals[0].0;
    let walk = (3.5 / def.moves.walk_speed * 60.0) as usize;
    assert!(gap >= low as usize + walk, "{gap}");
}

#[test]
fn it_sees_a_hero_ahead_hears_one_close_behind_and_not_through_a_wall() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    let mut rng = SimRng::default();
    let noticed = |heroes: &[HeroView], world: &ParryWorld, rng: &mut SimRng| {
        let mut enemy = standing();
        for _ in 0..30 {
            tick(&mut enemy, def, heroes, world, rng);
            if enemy.state == EnemyState::Aggro {
                return enemy.target;
            }
        }
        None
    };
    assert_eq!(noticed(&[hero(0, 0.0, -8.0)], &open(), &mut rng), Some(0));
    assert_eq!(noticed(&[hero(0, 0.0, 8.0)], &open(), &mut rng), None);
    assert_eq!(noticed(&[hero(0, 0.0, 3.0)], &open(), &mut rng), Some(0));
    assert_eq!(
        noticed(&[hero(0, 0.0, -8.0)], &walled(-4.0, 3.0), &mut rng),
        None
    );
    // The nearer of two.
    let both = [hero(0, 0.0, -10.0), hero(1, 1.0, -6.0)];
    assert_eq!(noticed(&both, &open(), &mut rng), Some(1));
    // A dead hero isn't there.
    let dead = HeroView {
        alive: false,
        ..hero(0, 0.0, -8.0)
    };
    assert_eq!(noticed(&[dead], &open(), &mut rng), None);
}

#[test]
fn a_charger_runs_in_winds_up_and_lunges_hitting_once_an_attack() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    let Attack::Lunge(lunge) = def.attack else {
        unreachable!()
    };
    let world = open();
    let mut rng = SimRng::default();
    let heroes = [hero(0, 0.0, -9.0)];
    let mut enemy = standing();
    let mut started = None;
    let mut hits = Vec::new();
    for t in 0..400 {
        let before = enemy.state;
        let outcome = tick(&mut enemy, def, &heroes, &world, &mut rng);
        if before == EnemyState::Aggro && enemy.state == EnemyState::Attack && started.is_none() {
            started = Some(t);
        }
        for hit in outcome.hits {
            hits.push((t, hit));
        }
    }
    let started = started.expect("it never attacked");
    // It ran 9 u less its range and stopped short.
    assert!(started > 60, "{started}");
    assert!(hits.len() >= 2, "{hits:?}");
    let (first, strike) = hits[0];
    assert!(first > started + lunge.wind_up_ticks, "{first} {started}");
    assert!(first <= started + lunge.wind_up_ticks + lunge.lunge_ticks);
    assert_eq!(strike.player, 0);
    assert_eq!(strike.damage, lunge.hit.damage);
    assert!(strike.knockback.z < -lunge.hit.knockback * 0.99);
    assert_eq!(strike.knockback.y, lunge.hit.lift);
    // The hero stands still, so it attacks again after its recovery and cooldown, hitting
    // once each time.
    let cycle = lunge.wind_up_ticks + lunge.lunge_ticks + lunge.recover_ticks;
    for pair in hits.windows(2) {
        assert!(
            pair[1].0 - pair[0].0 >= cycle + lunge.cooldown_ticks,
            "{hits:?}"
        );
    }
}

#[test]
fn stepping_aside_as_the_lunge_starts_dodges_it() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    let world = open();
    let mut rng = SimRng::default();
    let mut heroes = [hero(0, 0.0, -2.2)];
    let mut enemy = standing();
    let mut lunged = false;
    for _ in 0..120 {
        if enemy.attack_phase(def) == Some(AttackPhase::Strike) && !lunged {
            lunged = true;
            heroes[0].position.x = 2.0;
        }
        let outcome = tick(&mut enemy, def, &heroes, &world, &mut rng);
        assert!(outcome.hits.is_empty());
        if lunged && enemy.state != EnemyState::Attack {
            return;
        }
    }
    panic!("the attack didn't finish: {:?}", enemy.state);
}

#[test]
fn a_shooter_fires_its_burst_along_its_facing_then_keeps_its_distance() {
    let bestiary = Bestiary::default();
    let def = &bestiary.shooter;
    let Attack::Shoot(shoot) = def.attack else {
        unreachable!()
    };
    let world = open();
    let mut rng = SimRng::new(5, 0);

    // In range and ahead: it aims, then fires its shots at the hero.
    let heroes = [hero(0, 0.0, -10.0)];
    let mut enemy = standing();
    let mut shots = Vec::new();
    let mut started = None;
    for t in 0..120u32 {
        let outcome = tick(&mut enemy, def, &heroes, &world, &mut rng);
        if enemy.state == EnemyState::Attack && started.is_none() {
            started = Some(t);
        }
        shots.extend(outcome.shots.into_iter().map(|shot| (t, shot)));
    }
    // The tick it turned to the attack stands, then it aims.
    let started = started.unwrap() + 1;
    let times: Vec<u32> = shots.iter().map(|(t, _)| t - started).collect();
    let aim = shoot.aim_ticks;
    let every = shoot.shot_every;
    assert_eq!(times, [aim, aim + every, aim + 2 * every]);
    for (_, shot) in &shots {
        assert!((shot.velocity.length() - shoot.shot.speed).abs() < 1e-3);
        let towards = SHAPE.centre(heroes[0].position) - shot.position;
        assert!(shot.velocity.normalize().dot(towards.normalize()) > 0.999);
    }

    // Too near: between bursts it backs away.
    let heroes = [hero(0, 0.0, -3.0)];
    let mut enemy = standing();
    let distance = |enemy: &EnemyMotion| enemy.position.distance(heroes[0].position);
    while enemy.state != EnemyState::Attack {
        tick(&mut enemy, def, &heroes, &world, &mut rng);
    }
    while enemy.state == EnemyState::Attack {
        tick(&mut enemy, def, &heroes, &world, &mut rng);
    }
    let before = distance(&enemy);
    for _ in 0..40 {
        tick(&mut enemy, def, &heroes, &world, &mut rng);
    }
    assert!(
        distance(&enemy) > before + 0.5,
        "{before} -> {}",
        distance(&enemy)
    );

    // Too far: it closes in.
    let heroes = [hero(0, 0.0, -15.0)];
    let mut enemy = standing();
    let distance = |enemy: &EnemyMotion| enemy.position.distance(heroes[0].position);
    while enemy.state != EnemyState::Attack {
        tick(&mut enemy, def, &heroes, &world, &mut rng);
    }
    while enemy.state == EnemyState::Attack {
        tick(&mut enemy, def, &heroes, &world, &mut rng);
    }
    let before = distance(&enemy);
    for _ in 0..40 {
        tick(&mut enemy, def, &heroes, &world, &mut rng);
    }
    assert!(
        distance(&enemy) < before - 0.5,
        "{before} -> {}",
        distance(&enemy)
    );
}

#[test]
fn a_hit_cuts_off_its_attack_and_turns_it_on_the_attacker() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    let world = open();
    let mut rng = SimRng::default();
    let heroes = [hero(0, 0.0, -2.2), hero(1, 0.0, 5.0)];
    let mut enemy = standing();
    while enemy.attack_phase(def) != Some(AttackPhase::WindUp) {
        tick(&mut enemy, def, &heroes, &world, &mut rng);
    }
    assert_eq!(enemy.target, Some(0));
    brain::struck(&mut enemy, 1, Vec3::new(0.0, 0.0, -4.0));
    assert_eq!(enemy.state, EnemyState::Hurt);
    assert_eq!(enemy.target, Some(1));
    let start = enemy.position;
    for _ in 0..def.hurt_ticks {
        assert!(
            tick(&mut enemy, def, &heroes, &world, &mut rng)
                .hits
                .is_empty()
        );
        assert_eq!(enemy.state, EnemyState::Hurt);
    }
    // Pushed along the hit, slowing.
    assert!(enemy.position.z < start.z - 0.1);
    tick(&mut enemy, def, &heroes, &world, &mut rng);
    assert_eq!(enemy.state, EnemyState::Aggro);
}

#[test]
fn killed_it_lies_dead_then_vanishes_then_comes_back_at_home() {
    let bestiary = Bestiary::default();
    let def = &bestiary.shooter;
    let world = open();
    let mut rng = SimRng::default();
    let home = Vec3::new(2.0, 0.0, 1.0);
    let mut enemy = EnemyMotion::new(home, 1.0, Vec::new());
    enemy.position = Vec3::new(5.0, 0.0, 5.0);
    assert!(brain::killed(&mut enemy));
    assert!(!brain::killed(&mut enemy));
    brain::struck(&mut enemy, 0, Vec3::X);
    assert_eq!(enemy.state, EnemyState::Dead);
    let mut vanished = None;
    let mut respawned = None;
    for t in 0..def.death_ticks + def.respawn_ticks + 5 {
        let outcome = tick(&mut enemy, def, &[hero(0, 5.0, 3.0)], &world, &mut rng);
        if outcome.vanished {
            vanished = Some(t);
        }
        if outcome.respawned {
            respawned = Some(t);
            break;
        }
        assert!(outcome.hits.is_empty() && outcome.shots.is_empty());
    }
    assert_eq!(vanished, Some(def.death_ticks));
    assert_eq!(respawned, Some(def.death_ticks + def.respawn_ticks));
    assert_eq!(enemy, EnemyMotion::new(home, 1.0, Vec::new()));
}

#[test]
fn it_gives_up_on_a_hero_who_gets_away_and_ignores_him_until_calm() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    let world = open();
    let mut rng = SimRng::default();
    let mut enemy = standing();
    let mut heroes = [hero(0, 0.0, -10.0)];
    tick(&mut enemy, def, &heroes, &world, &mut rng);
    assert_eq!(enemy.state, EnemyState::Aggro);

    heroes[0].position.z = -40.0;
    tick(&mut enemy, def, &heroes, &world, &mut rng);
    assert_eq!(enemy.state, EnemyState::Patrol);
    assert_eq!(enemy.target, None);
    // Back in sight, but it is still calm.
    heroes[0].position = enemy.position + enemy.facing() * 8.0;
    for _ in 1..def.senses.calm_ticks {
        tick(&mut enemy, def, &heroes, &world, &mut rng);
        assert_ne!(enemy.state, EnemyState::Aggro);
    }
    for _ in 0..5 {
        tick(&mut enemy, def, &heroes, &world, &mut rng);
    }
    assert_eq!(enemy.state, EnemyState::Aggro);

    // Led too far from home, it gives up however near he is.
    enemy.position = Vec3::new(0.0, 0.0, -def.senses.leash - 1.0);
    heroes[0].position = enemy.position + Vec3::new(0.0, 0.0, -5.0);
    tick(&mut enemy, def, &heroes, &world, &mut rng);
    assert_eq!(enemy.state, EnemyState::Patrol);
}

#[test]
fn walls_stop_it_and_it_stays_on_the_ground() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    // Too high to step over, low enough to see over.
    let world = walled(-3.0, 0.8);
    let mut rng = SimRng::default();
    let heroes = [hero(0, 0.0, -6.5)];
    let mut chased = false;
    let mut enemy = standing();
    for _ in 0..300 {
        tick(&mut enemy, def, &heroes, &world, &mut rng);
        chased |= enemy.state == EnemyState::Aggro;
        assert!(
            enemy.position.z > -3.0 + def.radius - 0.01,
            "{}",
            enemy.position
        );
        assert!(enemy.position.y.abs() < 1e-3, "{}", enemy.position);
    }
    assert!(chased);
    assert!(enemy.position.z < -2.0, "{}", enemy.position);
}

#[test]
fn it_steps_off_a_ledge_and_falls_to_the_ground() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    let world = ParryWorld::new(&[
        floor(),
        BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(2.0, 1.0, 2.0),
            },
            Vec3::new(0.0, 1.0, 0.0),
            Quat::IDENTITY,
        ),
    ]);
    let mut rng = SimRng::default();
    let mut enemy = EnemyMotion::new(
        Vec3::new(0.0, 2.0, 0.0),
        0.0,
        vec![Vec3::new(0.0, 0.0, -6.0)],
    );
    for _ in 0..300 {
        tick(&mut enemy, def, &[], &world, &mut rng);
    }
    assert!(enemy.position.y.abs() < 1e-3, "{}", enemy.position);
    assert!(enemy.position.z < -5.0);
}

#[test]
fn crowded_enemies_part() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    let world = open();
    let mut rng = SimRng::default();
    let mut enemy = standing();
    let mut other = EnemyView::of(OTHER, &standing(), def);
    other.position = Vec3::new(0.3, 0.0, 0.0);
    for _ in 0..60 {
        let around = Surroundings {
            heroes: &[],
            hero_shape: SHAPE,
            me: ME,
            enemies: &[EnemyView::of(ME, &enemy, def), other],
            gravity: 30.0,
            world: &world,
        };
        brain::step(&mut enemy, def, &around, &mut rng);
    }
    // It wants its spot back, so it settles where its walk balances the push: most of the
    // overlap gone.
    let apart = enemy.position.distance(other.position);
    assert!(apart > 0.7, "{apart}");
}

/// Another enemy of `def`'s kind standing at `at`.
fn other_at(def: &EnemyDef, at: Vec3, infected: bool) -> EnemyView {
    let mut other = EnemyView::of(OTHER, &standing(), def);
    other.position = at;
    other.infected = infected;
    other
}

/// One tick of `enemy` among the heroes and `other`.
fn tick_among(
    enemy: &mut EnemyMotion,
    def: &EnemyDef,
    heroes: &[HeroView],
    other: EnemyView,
    world: &ParryWorld,
    rng: &mut SimRng,
) -> Outcome {
    let around = Surroundings {
        heroes,
        hero_shape: SHAPE,
        me: ME,
        enemies: &[EnemyView::of(ME, enemy, def), other],
        gravity: 30.0,
        world,
    };
    brain::step(enemy, def, &around, rng)
}

#[test]
fn an_infected_charger_leaves_the_hero_for_another_enemy_until_its_infection_ends() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    let world = open();
    let mut rng = SimRng::default();
    let mut enemy = standing();
    let heroes = [hero(0, 2.0, 0.0)];
    tick(&mut enemy, def, &heroes, &world, &mut rng);
    assert_eq!(enemy.target, Some(0));

    brain::afflict(&mut enemy, Affliction::Infected { ticks: 400 });
    let other = other_at(def, Vec3::new(0.0, 0.0, -6.0), false);
    let mut struck = Vec::new();
    for t in 0..399 {
        let outcome = tick_among(&mut enemy, def, &heroes, other, &world, &mut rng);
        assert!(outcome.hits.is_empty(), "{t}");
        struck.extend(outcome.foe_hits.into_iter().map(|hit| (t, hit)));
    }
    assert_eq!(enemy.foe, Some(OTHER));
    assert!(struck.len() >= 2, "{struck:?}");
    let Attack::Lunge(lunge) = def.attack else {
        panic!("the charger lunges")
    };
    for (_, hit) in &struck {
        assert_eq!((hit.target, hit.by), (OTHER, ME));
        assert_eq!(hit.damage, lunge.hit.damage as f32);
        assert!(
            (hit.push.length() - lunge.hit.knockback).abs() < 1e-3 && hit.push.y == 0.0,
            "{}",
            hit.push
        );
    }
    // Once an attack.
    for pair in struck.windows(2) {
        assert!(pair[1].0 - pair[0].0 > lunge.lunge_ticks, "{struck:?}");
    }

    // Its infection over, the other is no foe and the hero near it is left alone a while.
    tick_among(&mut enemy, def, &heroes, other, &world, &mut rng);
    assert_eq!(enemy.foe, None);
    assert_eq!(enemy.target, None);
}

#[test]
fn an_enemy_an_infected_one_hits_fights_back_until_the_infection_ends() {
    let bestiary = Bestiary::default();
    let def = &bestiary.charger;
    let world = open();
    let mut rng = SimRng::default();
    let mut enemy = standing();
    brain::struck_by(&mut enemy, OTHER, Vec3::new(0.0, 0.0, 3.0));
    assert_eq!(enemy.foe, Some(OTHER));
    assert_eq!(enemy.state, EnemyState::Hurt);

    let mut other = other_at(def, Vec3::new(0.0, 0.0, -4.0), true);
    let mut struck = false;
    for _ in 0..200 {
        let outcome = tick_among(&mut enemy, def, &[], other, &world, &mut rng);
        struck |= outcome.foe_hits.iter().any(|hit| hit.target == OTHER);
    }
    assert!(struck);
    other.infected = false;
    tick_among(&mut enemy, def, &[], other, &world, &mut rng);
    assert_eq!(enemy.foe, None);
    // It finishes any attack under way, then goes back.
    for _ in 0..120 {
        if enemy.state == EnemyState::Patrol {
            break;
        }
        tick_among(&mut enemy, def, &[], other, &world, &mut rng);
    }
    assert_eq!(enemy.state, EnemyState::Patrol);

    // A tame enemy only reels.
    let mut tame = standing();
    brain::afflict(&mut tame, Affliction::Infected { ticks: 60 });
    brain::struck_by(&mut tame, OTHER, Vec3::ZERO);
    assert_eq!((tame.foe, tame.state), (None, EnemyState::Hurt));
}

#[test]
fn an_infected_shooters_shots_are_fired_at_its_foe() {
    let bestiary = Bestiary::default();
    let def = &bestiary.shooter;
    let world = open();
    let mut rng = SimRng::default();
    let mut enemy = standing();
    brain::afflict(&mut enemy, Affliction::Infected { ticks: 600 });
    let heroes = [hero(0, 0.0, -3.0)];
    let other = other_at(&bestiary.charger, Vec3::new(0.0, 0.0, -9.0), false);
    let shots: Vec<_> = (0..300)
        .flat_map(|_| tick_among(&mut enemy, def, &heroes, other, &world, &mut rng).shots)
        .collect();
    assert!(!shots.is_empty());
    for shot in &shots {
        assert_eq!(shot.turned, Some(ME));
        assert!(shot.velocity.z < 0.0, "{}", shot.velocity);
    }
}

#[test]
fn the_same_inputs_give_the_same_enemy() {
    let bestiary = Bestiary::default();
    let world = walled(-12.0, 3.0);
    let run = || {
        let mut rng = SimRng::new(11, 2);
        let mut enemies = [
            EnemyMotion::new(Vec3::ZERO, 0.0, vec![Vec3::new(5.0, 0.0, 0.0)]),
            EnemyMotion::new(Vec3::new(3.0, 0.0, 3.0), PI, Vec::new()),
        ];
        let kinds = [&bestiary.charger, &bestiary.shooter];
        let mut heroes = [hero(0, 0.0, -9.0)];
        for t in 0..600 {
            heroes[0].position.x = (t as f32 * 0.05).sin() * 6.0;
            for (enemy, def) in enemies.iter_mut().zip(kinds) {
                tick(enemy, def, &heroes, &world, &mut rng);
            }
        }
        (enemies, rng)
    };
    assert_eq!(run(), run());
}

/// One tick per update: a floor, the hero at the origin facing −z, and a charger `ahead`
/// of him facing him.
fn app(ahead: f32) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        SimPlugin,
        HeroPlugin,
        CombatPlugin,
        EnemyPlugin,
    ))
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
    let profile = HeroProfile::uya();
    let hero = app
        .world_mut()
        .spawn((
            hero_bundle(0, Vec3::ZERO, 0.0, -20.0, &profile),
            Bolts::default(),
        ))
        .id();
    let def = Bestiary::default().charger;
    let enemy = app
        .world_mut()
        .spawn(enemy_bundle(
            EnemyKind::Charger,
            &def,
            Vec3::new(0.0, 0.0, -ahead),
            PI,
            Vec::new(),
        ))
        .id();
    app.update(); // Starts the clock.
    (app, hero, enemy)
}

fn state(app: &App, enemy: Entity) -> EnemyState {
    app.world().get::<EnemyBody>(enemy).unwrap().0.state
}

#[test]
fn in_an_app_the_wrench_makes_a_charger_reel() {
    let (mut app, _, enemy) = app(1.8);
    let mut buttons = Buttons::NONE;
    buttons.insert(Action::Melee);
    app.world_mut().resource_mut::<PlayerInputs>().0[0] = PlayerInput {
        held: buttons,
        pressed: buttons,
        ..default()
    };
    app.update();
    app.world_mut().resource_mut::<PlayerInputs>().0[0] = PlayerInput::default();
    let mut reeled = false;
    for _ in 0..30 {
        app.update();
        reeled |= state(&app, enemy) == EnemyState::Hurt;
    }
    assert!(reeled);
    let def = Bestiary::default().charger;
    assert_eq!(
        app.world().get::<Hitpoints>(enemy).unwrap().current,
        def.hitpoints - 1.0
    );
}

#[test]
fn in_an_app_a_charger_hurts_the_hero_and_dies_into_drops_he_collects() {
    let (mut app, hero, enemy) = app(6.0);
    let health = |app: &App| app.world().get::<Health>(hero).unwrap().current;
    let full = health(&app);
    let mut ticks = 0;
    while health(&app) == full {
        app.update();
        ticks += 1;
        assert!(ticks < 300, "the charger never hurt him");
    }
    assert_eq!(full - health(&app), 2);

    // Its hit points run out as the combat code would end them.
    app.world_mut().get_mut::<Hitpoints>(enemy).unwrap().current = 0.0;
    app.world_mut().write_message(Destroyed {
        target: enemy,
        player: 0,
        weapon: None,
    });
    app.update();
    assert_eq!(state(&app, enemy), EnemyState::Dead);
    assert!(app.world().get::<Hurtbox>(enemy).is_none());
    let drops = app.world_mut().query::<&Pickup>().iter(app.world()).count();
    let table = Bestiary::default().charger.drops;
    assert!(drops >= table.bolts.0 as usize, "{drops}");

    // He stands near enough for all of them to fly to him.
    for _ in 0..120 {
        app.update();
    }
    let left = app.world_mut().query::<&Pickup>().iter(app.world()).count();
    assert_eq!(left, 0);
    let bolts = app.world().get::<Bolts>(hero).unwrap().0;
    assert!(bolts >= table.bolts.0 * table.bolt_value, "{bolts}");

    // It comes back.
    let def = Bestiary::default().charger;
    for _ in 0..def.death_ticks + def.respawn_ticks {
        app.update();
    }
    assert_ne!(state(&app, enemy), EnemyState::Dead);
    assert!(app.world().get::<Hurtbox>(enemy).is_some());
    assert_eq!(
        app.world().get::<Hitpoints>(enemy).unwrap().current,
        def.hitpoints
    );
}

#[test]
fn in_an_app_a_frozen_charger_dies_with_no_drops() {
    let (mut app, _, enemy) = app(30.0);
    app.world_mut().get_mut::<Hitpoints>(enemy).unwrap().current = 0.0;
    app.world_mut().write_message(Afflicted {
        target: enemy,
        player: 0,
        weapon: WeaponId(3),
        affliction: Affliction::Frozen,
    });
    app.world_mut().write_message(Destroyed {
        target: enemy,
        player: 0,
        weapon: Some(WeaponId(3)),
    });
    app.update();
    assert_eq!(state(&app, enemy), EnemyState::Dead);
    let drops = app.world_mut().query::<&Pickup>().iter(app.world()).count();
    assert_eq!(drops, 0);
}

#[test]
fn in_an_app_an_infected_charger_kills_another_into_its_drops() {
    let (mut app, hero, infected) = app(30.0);
    let def = Bestiary::default().charger;
    let other = app
        .world_mut()
        .spawn(enemy_bundle(
            EnemyKind::Charger,
            &def,
            Vec3::new(4.0, 0.0, -30.0),
            0.0,
            Vec::new(),
        ))
        .id();
    app.world_mut().write_message(Afflicted {
        target: infected,
        player: 0,
        weapon: WeaponId(0),
        affliction: Affliction::Infected { ticks: 3000 },
    });
    let full = app.world().get::<Health>(hero).unwrap().current;
    let mut ticks = 0;
    while state(&app, other) != EnemyState::Dead {
        app.update();
        ticks += 1;
        assert!(ticks < 1500, "the infected charger never killed the other");
    }
    assert_ne!(state(&app, infected), EnemyState::Dead);
    assert!(app.world().get::<Hurtbox>(other).is_none());
    let drops = app.world_mut().query::<&Pickup>().iter(app.world()).count();
    assert!(drops >= def.drops.bolts.0 as usize, "{drops}");
    assert_eq!(app.world().get::<Health>(hero).unwrap().current, full);
}

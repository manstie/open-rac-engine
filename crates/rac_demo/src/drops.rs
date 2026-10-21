//! What enemies leave when they die (PLAN.md P1.11): bolts and nanotech that pop out,
//! bounce, and fly to a hero who comes near, as UYA's do. Bolts add to his [`Bolts`];
//! nanotech heals his `Health`. P1.10's pickups and crates can build on these.

use bevy::prelude::*;
use rac_hero::{CollisionScene, CollisionWorld, Health, Hero, HeroBody, HeroState, HeroTuning};
use rac_sim::{SimRng, SimTransform};
use serde::{Deserialize, Serialize};

use crate::DemoEntity;
use crate::enemy::brain::{HeroShape, HeroView, TICK, flat};
use crate::enemy::def::DropTable;

/// The bundled tuning.
pub const DROPS_RON: &str = include_str!("../assets/drops.ron");

/// How drops move.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DropTuning {
    /// u/s².
    pub gravity: f32,
    pub radius: f32,
    /// The share of its speed into the ground it keeps as it bounces; slower than
    /// `rest_speed`, it stops.
    pub bounce: f32,
    pub rest_speed: f32,
    /// u/s², slowing it along the ground.
    pub ground_brake: f32,
    /// How fast it pops out: a range across the ground and a range up, u/s.
    pub pop_speed: (f32, f32),
    pub pop_lift: (f32, f32),
    /// Ticks after it pops out before it flies to a hero within `magnet_radius` of it.
    pub magnet_delay_ticks: u32,
    pub magnet_radius: f32,
    /// It flies at `fly_speed`, gaining `fly_accel` u/s², and is his within
    /// `collect_radius` of his middle.
    pub fly_speed: f32,
    pub fly_accel: f32,
    pub collect_radius: f32,
    /// Ticks before one no one collects vanishes, blinking for the last `blink_ticks`.
    pub life_ticks: u32,
    pub blink_ticks: u32,
}

#[derive(Debug, thiserror::Error)]
#[error("drop tuning: {0}")]
pub struct DropTuningError(#[from] ron::error::SpannedError);

impl DropTuning {
    pub fn from_ron(text: &str) -> Result<Self, DropTuningError> {
        Ok(ron::from_str(text)?)
    }
}

impl Default for DropTuning {
    fn default() -> Self {
        Self::from_ron(DROPS_RON).expect("the bundled drop tuning parses")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupKind {
    /// This many bolts.
    Bolts(u32),
    /// Heals this much.
    Nanotech(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickupMotion {
    pub kind: PickupKind,
    /// Its middle.
    pub position: Vec3,
    /// u/s.
    pub velocity: Vec3,
    /// Ticks since it popped out.
    pub age: u32,
    /// The player of the hero it flies to.
    pub flying_to: Option<usize>,
}

/// A bolt or nanotech orb lying about or flying to a hero.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(SimTransform)]
pub struct Pickup(pub PickupMotion);

/// The bolts a hero has collected.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Bolts(pub u32);

/// How a pickup ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupEnd {
    /// The hero of this player has it.
    Collected(usize),
    Expired,
}

/// What a dead enemy leaves at `at`, popping out every way.
pub fn scatter(
    at: Vec3,
    table: &DropTable,
    tuning: &DropTuning,
    rng: &mut SimRng,
) -> Vec<PickupMotion> {
    let bolts = crate::enemy::brain::between(rng, table.bolts);
    let mut kinds = vec![PickupKind::Bolts(table.bolt_value); bolts as usize];
    if rng.unit_f32() < table.nanotech_chance {
        kinds.push(PickupKind::Nanotech(table.nanotech));
    }
    kinds
        .into_iter()
        .map(|kind| {
            let angle = rng.range_f32(0.0, std::f32::consts::TAU);
            let speed = rng.range_f32(tuning.pop_speed.0, tuning.pop_speed.1);
            let lift = rng.range_f32(tuning.pop_lift.0, tuning.pop_lift.1);
            let across = Vec3::new(
                bevy::math::ops::cos(angle),
                0.0,
                bevy::math::ops::sin(angle),
            );
            PickupMotion {
                kind,
                position: at,
                velocity: across * speed + Vec3::Y * lift,
                age: 0,
                flying_to: None,
            }
        })
        .collect()
}

/// One tick of a pickup: `None` while it stays about.
pub fn step(
    pickup: &mut PickupMotion,
    heroes: &[HeroView],
    hero_shape: &HeroShape,
    tuning: &DropTuning,
    world: &dyn CollisionWorld,
) -> Option<PickupEnd> {
    pickup.age += 1;
    let hero_centre = |player: usize| {
        heroes
            .iter()
            .find(|hero| hero.player == player && hero.alive)
            .map(|hero| hero_shape.centre(hero.position))
    };
    if pickup
        .flying_to
        .is_some_and(|player| hero_centre(player).is_none())
    {
        // Its hero died: it drops where it is, rather than flying on at its homing speed.
        pickup.flying_to = None;
        pickup.velocity = Vec3::ZERO;
    }
    if pickup.flying_to.is_none() {
        if pickup.age >= tuning.life_ticks {
            return Some(PickupEnd::Expired);
        }
        if pickup.age >= tuning.magnet_delay_ticks {
            let mut nearest: Option<(usize, f32)> = None;
            for hero in heroes.iter().filter(|hero| hero.alive) {
                let distance = hero_shape.centre(hero.position).distance(pickup.position);
                if distance <= tuning.magnet_radius && nearest.is_none_or(|(_, d)| distance < d) {
                    nearest = Some((hero.player, distance));
                }
            }
            if let Some((player, _)) = nearest {
                pickup.flying_to = Some(player);
                pickup.velocity = Vec3::ZERO;
            }
        }
    }

    if let Some(player) = pickup.flying_to
        && let Some(centre) = hero_centre(player)
    {
        // Straight to him, through anything in the way, as the games' bolts go.
        let speed = (pickup.velocity.length().max(tuning.fly_speed)) + tuning.fly_accel * TICK;
        let to = centre - pickup.position;
        let distance = to.length();
        if distance <= tuning.collect_radius + speed * TICK {
            return Some(PickupEnd::Collected(player));
        }
        pickup.velocity = to / distance * speed;
        pickup.position += pickup.velocity * TICK;
        return None;
    }

    pickup.velocity.y -= tuning.gravity * TICK;
    let motion = pickup.velocity * TICK;
    let Some(hit) = world.cast_sphere(tuning.radius, pickup.position, motion) else {
        pickup.position += motion;
        return None;
    };
    pickup.position += motion * hit.fraction;
    let into = pickup.velocity.dot(hit.normal);
    if into < 0.0 {
        pickup.velocity -= hit.normal * into * (1.0 + tuning.bounce);
    }
    if hit.normal.y > 0.5 {
        if pickup.velocity.y.abs() < tuning.rest_speed {
            pickup.velocity.y = 0.0;
        }
        let across = flat(pickup.velocity);
        let slower = (across.length() - tuning.ground_brake * TICK).max(0.0);
        let across = across.normalize_or_zero() * slower;
        pickup.velocity = Vec3::new(across.x, pickup.velocity.y, across.z);
    }
    None
}

/// Whether a pickup this old shows this tick: it blinks before it expires.
pub fn shows(pickup: &PickupMotion, tuning: &DropTuning) -> bool {
    let left = tuning.life_ticks.saturating_sub(pickup.age);
    pickup.flying_to.is_some() || left > tuning.blink_ticks || (left / 6).is_multiple_of(2)
}

/// Puts `pickups` in the world.
pub(crate) fn spawn(commands: &mut Commands, pickups: Vec<PickupMotion>) {
    for pickup in pickups {
        let name = match pickup.kind {
            PickupKind::Bolts(_) => "bolt",
            PickupKind::Nanotech(_) => "nanotech",
        };
        commands.spawn((
            Name::new(name),
            DemoEntity,
            Pickup(pickup),
            SimTransform::from_translation(pickup.position),
        ));
    }
}

/// Moves every pickup, and gives those collected to their heroes.
pub(crate) fn move_pickups(
    tuning: Res<DropTuning>,
    hero_tuning: Res<HeroTuning>,
    scene: Res<CollisionScene>,
    mut heroes: Query<(&Hero, &HeroBody, &mut Health, Option<&mut Bolts>)>,
    mut pickups: Query<(Entity, &mut Pickup, &mut SimTransform)>,
    mut commands: Commands,
) {
    let mut views: Vec<HeroView> = heroes
        .iter()
        .map(|(hero, body, ..)| HeroView {
            player: hero.player,
            position: body.0.position,
            alive: body.0.state != HeroState::Dead,
        })
        .collect();
    views.sort_by_key(|view| view.player);
    let shape = HeroShape::from_profile(&hero_tuning.0.body);
    let mut pickups: Vec<_> = pickups.iter_mut().collect();
    pickups.sort_by_key(|(entity, ..)| *entity);
    for (entity, pickup, transform) in &mut pickups {
        let end = step(&mut pickup.0, &views, &shape, &tuning, &scene.0);
        transform.translation = pickup.0.position;
        let Some(end) = end else {
            continue;
        };
        commands.entity(*entity).despawn();
        let PickupEnd::Collected(player) = end else {
            continue;
        };
        let Some((_, _, mut health, bolts)) =
            heroes.iter_mut().find(|(hero, ..)| hero.player == player)
        else {
            continue;
        };
        match pickup.0.kind {
            PickupKind::Bolts(value) => {
                if let Some(mut bolts) = bolts {
                    bolts.0 = bolts.0.saturating_add(value);
                }
            }
            PickupKind::Nanotech(amount) => {
                health.current = health.current.saturating_add(amount).min(health.max);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_hero::{BodyPlacement, Collider, ParryWorld};

    const SHAPE: HeroShape = HeroShape {
        radius: 0.45,
        bottom: 0.5,
        top: 1.05,
    };

    fn floor() -> ParryWorld {
        ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(50.0, 0.5, 50.0),
            },
            Vec3::new(0.0, -0.5, 0.0),
            Quat::IDENTITY,
        )])
    }

    const TABLE: DropTable = DropTable {
        bolts: (4, 7),
        bolt_value: 5,
        nanotech_chance: 0.5,
        nanotech: 2,
    };

    #[test]
    fn a_kill_scatters_its_tables_bolts_and_sometimes_nanotech() {
        let tuning = DropTuning::default();
        let mut rng = SimRng::new(7, 0);
        let mut counts = Vec::new();
        let mut nanotech = 0;
        for _ in 0..200 {
            let drops = scatter(Vec3::Y, &TABLE, &tuning, &mut rng);
            let bolts = drops
                .iter()
                .filter(|drop| drop.kind == PickupKind::Bolts(5))
                .count();
            counts.push(bolts);
            nanotech += drops.len() - bolts;
            for drop in &drops {
                assert!(drop.velocity.y >= tuning.pop_lift.0);
            }
        }
        assert_eq!(counts.iter().min(), Some(&4));
        assert_eq!(counts.iter().max(), Some(&7));
        assert!((60..140).contains(&nanotech), "{nanotech}");
    }

    #[test]
    fn a_drop_lands_and_settles_on_the_ground() {
        let tuning = DropTuning::default();
        let world = floor();
        let mut drop = scatter(Vec3::Y, &TABLE, &tuning, &mut SimRng::new(1, 0))[0];
        for _ in 0..240 {
            assert_eq!(step(&mut drop, &[], &SHAPE, &tuning, &world), None);
        }
        assert!(
            (drop.position.y - tuning.radius).abs() < 0.01,
            "{}",
            drop.position
        );
        assert_eq!(drop.velocity, Vec3::ZERO);
    }

    #[test]
    fn a_drop_waits_then_flies_to_the_nearest_hero_near_enough() {
        let tuning = DropTuning::default();
        let world = floor();
        let mut drop = PickupMotion {
            kind: PickupKind::Nanotech(2),
            position: Vec3::new(0.0, 0.15, 0.0),
            velocity: Vec3::ZERO,
            age: 0,
            flying_to: None,
        };
        let far = HeroView {
            player: 0,
            position: Vec3::new(0.0, 0.0, 5.0),
            alive: true,
        };
        let near = HeroView {
            player: 1,
            position: Vec3::new(3.0, 0.0, 0.0),
            alive: true,
        };
        let away = HeroView {
            player: 2,
            position: Vec3::new(-20.0, 0.0, 0.0),
            alive: true,
        };
        let heroes = [far, near, away];
        let mut ticks = 0;
        let end = loop {
            ticks += 1;
            if ticks < tuning.magnet_delay_ticks {
                assert_eq!(drop.flying_to, None);
            }
            if let Some(end) = step(&mut drop, &heroes, &SHAPE, &tuning, &world) {
                break end;
            }
            assert!(ticks < 120);
        };
        assert_eq!(end, PickupEnd::Collected(1));
        assert!(ticks > tuning.magnet_delay_ticks);
    }

    #[test]
    fn a_drop_whose_hero_dies_on_its_way_falls_where_it_is() {
        let tuning = DropTuning::default();
        let world = floor();
        let mut drop = PickupMotion {
            kind: PickupKind::Bolts(5),
            position: Vec3::new(0.0, 0.15, 0.0),
            velocity: Vec3::ZERO,
            age: tuning.magnet_delay_ticks,
            flying_to: None,
        };
        let mut hero = HeroView {
            player: 0,
            position: Vec3::new(5.0, 0.0, 0.0),
            alive: true,
        };
        for _ in 0..8 {
            assert_eq!(step(&mut drop, &[hero], &SHAPE, &tuning, &world), None);
        }
        assert_eq!(drop.flying_to, Some(0));
        let caught = drop.position;
        hero.alive = false;
        for _ in 0..120 {
            assert_eq!(step(&mut drop, &[hero], &SHAPE, &tuning, &world), None);
        }
        assert_eq!(drop.flying_to, None);
        assert!(
            flat(drop.position - caught).length() < 0.01,
            "{}",
            drop.position
        );
        assert!(
            (drop.position.y - tuning.radius).abs() < 0.01,
            "{}",
            drop.position
        );
    }

    #[test]
    fn a_drop_no_one_collects_blinks_and_expires() {
        let tuning = DropTuning::default();
        let world = floor();
        let mut drop = PickupMotion {
            kind: PickupKind::Bolts(5),
            position: Vec3::new(0.0, 0.15, 0.0),
            velocity: Vec3::ZERO,
            age: 0,
            flying_to: None,
        };
        let mut hidden = 0;
        for _ in 1..tuning.life_ticks {
            assert_eq!(step(&mut drop, &[], &SHAPE, &tuning, &world), None);
            hidden += u32::from(!shows(&drop, &tuning));
        }
        assert_eq!(
            step(&mut drop, &[], &SHAPE, &tuning, &world),
            Some(PickupEnd::Expired)
        );
        assert!(hidden > 0 && hidden < tuning.blink_ticks, "{hidden}");
    }
}

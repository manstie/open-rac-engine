//! Pickups and the bolt economy (PLAN.md P1.10): the bolts, nanotech and ammo that crates
//! ([`crates`]) and dead enemies leave, and a vendor stub that sells rounds ([`vendor`]).
//!
//! - Pickups pop out and bounce. [`step`] is a pickup's whole motion as one pure function
//!   over the heroes as it sees them ([`Collector`]); [`move_pickups`] runs it for every
//!   [`Pickup`] each tick, in entity order, and gives what a hero gets: bolts to his
//!   [`Bolts`], nanotech to his `Health`, and ammo to his [`Arsenal`] (`Arsenal::add_ammo`,
//!   UYA's `ammo_add`, which caps it at the gun's maximum).
//! - A hero reaches a pickup within 8.7 u across and 3 u up or down from his feet, as UYA's
//!   pickups and bolts are taken (`pickup_radius`, `pickup_height`).
//! - Ammo is for one gun, and follows UYA's ammo pickups (`weapons.md`, "Ammo pickups,
//!   crates and the vendor"): taken only while its gun isn't full, and only once it has
//!   landed or as it bounces, unless he came in reach while it was in the air; once it has
//!   fallen 2 u below where it was thrown, at once. Its rounds go in as it is taken and it
//!   then flies in for show, shrinking away. It never expires. In co-op the nearest hero
//!   who owns the gun with room takes it.
//! - Bolts and nanotech wait a little, fly to the nearest hero in reach and are his when
//!   they arrive, and vanish if no one takes them [placeholder: UYA's bolts aren't read
//!   beyond their reach].
//! - The numbers are in `assets/pickups.ron`.

pub mod crates;
pub mod vendor;

use bevy::prelude::*;
use rac_hero::{CollisionScene, CollisionWorld, Health, Hero, HeroBody, HeroState, HeroTuning};
use rac_sim::{SimRng, SimTransform};
use serde::{Deserialize, Serialize};

use crate::TICK;
use crate::weapons::{Armory, Arsenal, Hand, WeaponId};

pub use crates::{Crate, CrateKind, CrateTable};
pub use vendor::{Vendor, VendorTuning};

/// The bundled tuning.
pub const PICKUPS_RON: &str = include_str!("../../assets/pickups.ron");

/// How pickups move, and what crates and the vendor give.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PickupTuning {
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
    /// A hero reaches a pickup within `reach` across and `reach_height` up or down from
    /// his feet.
    pub reach: f32,
    pub reach_height: f32,
    /// Bolts and nanotech: ticks after they pop out before they fly to a hero in reach.
    /// They fly at `fly_speed`, gaining `fly_accel` u/s², and are his within
    /// `collect_radius` of his middle.
    pub magnet_delay_ticks: u32,
    pub fly_speed: f32,
    pub fly_accel: f32,
    pub collect_radius: f32,
    /// Ticks before bolts or nanotech no one collects vanish, blinking for the last
    /// `blink_ticks`.
    pub life_ticks: u32,
    pub blink_ticks: u32,
    pub ammo: AmmoRules,
    /// What each kind of crate holds.
    pub crates: CrateTable,
    pub vendor: VendorTuning,
}

/// How ammo pickups are taken and fly in (`ammo_pickup_update`, 0x30E7F8).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AmmoRules {
    /// Fallen this far below where it was thrown, it is taken at once.
    pub fall: f32,
    /// Taken, it flies in from rest, gaining `accel` u/s² up to `top_speed`, to the point
    /// `height` above his feet, carried along with him. It shrinks away over the last
    /// `shrink_ticks` of its way.
    pub accel: f32,
    pub top_speed: f32,
    pub height: f32,
    pub shrink_ticks: u32,
}

#[derive(Debug, thiserror::Error)]
#[error("pickup tuning: {0}")]
pub struct PickupTuningError(#[from] ron::error::SpannedError);

impl PickupTuning {
    pub fn from_ron(text: &str) -> Result<Self, PickupTuningError> {
        Ok(ron::from_str(text)?)
    }
}

impl Default for PickupTuning {
    fn default() -> Self {
        // A test parses the bundled file, so this cannot fail at run time.
        Self::from_ron(PICKUPS_RON).expect("the bundled pickup tuning parses")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupKind {
    /// A bolt worth this many.
    Bolts(u32),
    /// Heals this much.
    Nanotech(u32),
    /// This many rounds for one gun.
    Ammo { weapon: WeaponId, rounds: u32 },
}

impl PickupKind {
    /// An ammo pickup for `weapon`, holding its `pickup_rounds`; `None` for a gun with
    /// none.
    pub fn ammo(weapon: WeaponId, armory: &Armory) -> Option<Self> {
        let rounds = armory.get(weapon)?.pickup_rounds;
        (rounds > 0).then_some(PickupKind::Ammo { weapon, rounds })
    }
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
    /// The height it was thrown from.
    pub thrown_from: f32,
    /// It has touched the ground since it was thrown.
    pub landed: bool,
    /// Ammo that this player's hero came in reach of while it was in the air: his as it
    /// lands.
    pub armed: Option<usize>,
    /// Ammo already given, flying in for show.
    pub taken: bool,
    /// Its size as drawn, 1 until it shrinks away.
    pub scale: f32,
}

impl PickupMotion {
    /// A pickup at `position`, thrown from there with `velocity`.
    pub fn new(kind: PickupKind, position: Vec3, velocity: Vec3) -> Self {
        Self {
            kind,
            position,
            velocity,
            age: 0,
            flying_to: None,
            thrown_from: position.y,
            landed: false,
            armed: None,
            taken: false,
            scale: 1.0,
        }
    }
}

/// A bolt, nanotech orb or ammo lying about or flying to a hero.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(SimTransform)]
pub struct Pickup(pub PickupMotion);

/// The bolts a hero has collected.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Bolts(pub u32);

/// What became of a pickup this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupEvent {
    /// The hero of this player takes it now, and it goes.
    Collected(usize),
    /// The hero of this player takes it now, and it flies on in for show.
    Taken(usize),
    /// It flew in after being taken, or its hero is gone: it goes.
    Gone,
    Expired,
}

/// A hero as a pickup sees him.
#[derive(Debug, Clone, PartialEq)]
pub struct Collector {
    pub player: usize,
    pub feet: Vec3,
    /// His middle, which bolts and nanotech fly to.
    pub centre: Vec3,
    /// How far he moved this tick, which carries ammo flying in along with him.
    pub moved: Vec3,
    pub alive: bool,
    /// He keeps bolts (he has [`Bolts`]).
    pub keeps_bolts: bool,
    /// The guns he owns, and the rounds each has room for ([`ammo_room`]); a gun not
    /// listed in `ammo_room` takes none.
    pub guns: Vec<WeaponId>,
    pub ammo_room: Vec<(WeaponId, u32)>,
}

impl Collector {
    /// Whether he can take a pickup of `kind`.
    pub fn takes(&self, kind: PickupKind) -> bool {
        self.alive
            && match kind {
                PickupKind::Bolts(_) => self.keeps_bolts,
                PickupKind::Nanotech(_) => true,
                PickupKind::Ammo { weapon, .. } => self.room(weapon) > 0,
            }
    }

    pub fn room(&self, weapon: WeaponId) -> u32 {
        self.ammo_room
            .iter()
            .find(|(gun, _)| *gun == weapon)
            .map_or(0, |(_, room)| *room)
    }

    /// Whether `at` is within his reach.
    pub fn reaches(&self, at: Vec3, tuning: &PickupTuning) -> bool {
        let across = Vec2::new(at.x - self.feet.x, at.z - self.feet.z);
        across.length_squared() < tuning.reach * tuning.reach
            && (at.y - self.feet.y).abs() < tuning.reach_height
    }
}

/// The rounds each gun he owns has room for, in the order given; full guns, and guns that
/// use no ammo or have no ammo pickups, aren't listed.
pub fn ammo_room(arsenal: &Arsenal, armory: &Armory) -> Vec<(WeaponId, u32)> {
    arsenal
        .guns
        .iter()
        .filter_map(|gun| {
            let def = armory.get(gun.weapon)?;
            let room = def.level(gun.level).ammo.saturating_sub(gun.ammo);
            (room > 0 && def.pickup_rounds > 0).then_some((gun.weapon, room))
        })
        .collect()
}

/// The gun a hero is using: the one in his hand or coming out, else, with the wrench out,
/// the one his fire button brings out.
pub fn current_gun(arsenal: &Arsenal) -> Option<WeaponId> {
    match arsenal.hand {
        Hand::Gun { weapon, .. } | Hand::Drawing { weapon, .. } => Some(weapon),
        Hand::Wrench => arsenal.equipped,
    }
}

/// A uniform integer in the range, both ends included.
pub(crate) fn between(rng: &mut SimRng, (low, high): (u32, u32)) -> u32 {
    low + rng.below(high.saturating_sub(low).saturating_add(1))
}

/// Pops `kinds` out of `at`, each a random way.
pub fn scatter(
    at: Vec3,
    kinds: Vec<PickupKind>,
    tuning: &PickupTuning,
    rng: &mut SimRng,
) -> Vec<PickupMotion> {
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
            PickupMotion::new(kind, at, across * speed + Vec3::Y * lift)
        })
        .collect()
}

/// One tick of a pickup: `None` while nothing became of it.
pub fn step(
    pickup: &mut PickupMotion,
    heroes: &[Collector],
    tuning: &PickupTuning,
    world: &dyn CollisionWorld,
) -> Option<PickupEvent> {
    pickup.age += 1;
    if pickup.taken {
        return fly_in(pickup, heroes, &tuning.ammo);
    }
    if matches!(pickup.kind, PickupKind::Ammo { .. }) {
        let touched = fall(pickup, tuning, world);
        return take_ammo(pickup, heroes, tuning, touched);
    }
    home(pickup, heroes, tuning, world)
}

/// Ammo lying about or thrown, taken by UYA's rules (`ammo_pickup_collect_test`,
/// 0x30EDC0) by the nearest hero with room for it in reach.
fn take_ammo(
    pickup: &mut PickupMotion,
    heroes: &[Collector],
    tuning: &PickupTuning,
    touched: bool,
) -> Option<PickupEvent> {
    let PickupKind::Ammo { weapon, .. } = pickup.kind else {
        return None;
    };
    let nearest = |heroes: &mut dyn Iterator<Item = &Collector>| {
        heroes
            .map(|hero| {
                let across = Vec2::new(
                    hero.feet.x - pickup.position.x,
                    hero.feet.z - pickup.position.z,
                );
                (hero.player, across.length_squared())
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(player, _)| player)
    };
    // Fallen off where it was thrown, it goes to its gun at once, full or not.
    if pickup.position.y < pickup.thrown_from - tuning.ammo.fall {
        let owner = nearest(
            &mut heroes
                .iter()
                .filter(|hero| hero.alive && hero.guns.contains(&weapon)),
        );
        return Some(owner.map_or(PickupEvent::Gone, PickupEvent::Taken));
    }
    let in_reach = nearest(
        &mut heroes
            .iter()
            .filter(|hero| hero.takes(pickup.kind) && hero.reaches(pickup.position, tuning)),
    );
    if pickup.landed || touched {
        let armed = pickup.armed.take().filter(|&player| {
            heroes
                .iter()
                .any(|h| h.player == player && h.takes(pickup.kind))
        });
        return armed.or(in_reach).map(PickupEvent::Taken);
    }
    if in_reach.is_some() {
        pickup.armed = in_reach;
    }
    None
}

/// Taken ammo flying in to its hero.
fn fly_in(
    pickup: &mut PickupMotion,
    heroes: &[Collector],
    rules: &AmmoRules,
) -> Option<PickupEvent> {
    let Some(hero) = pickup
        .flying_to
        .and_then(|player| heroes.iter().find(|hero| hero.player == player))
    else {
        return Some(PickupEvent::Gone);
    };
    // Its velocity is along its way to him; he carries it along besides.
    let speed = (pickup.velocity.length() + rules.accel * TICK).min(rules.top_speed);
    pickup.position += hero.moved;
    let to = hero.feet + Vec3::Y * rules.height - pickup.position;
    let distance = to.length();
    let ticks_left = distance / (speed * TICK).max(f32::EPSILON);
    if ticks_left < rules.shrink_ticks as f32 {
        pickup.scale -= 1.0 / rules.shrink_ticks.max(1) as f32;
    }
    if pickup.scale <= 0.0 || ticks_left <= 1.0 {
        return Some(PickupEvent::Gone);
    }
    pickup.velocity = to / distance * speed;
    pickup.position += pickup.velocity * TICK;
    None
}

/// Bolts and nanotech: they wait, fly to the nearest hero in reach who can take them, and
/// are his on arrival.
fn home(
    pickup: &mut PickupMotion,
    heroes: &[Collector],
    tuning: &PickupTuning,
    world: &dyn CollisionWorld,
) -> Option<PickupEvent> {
    let kind = pickup.kind;
    let centre_of = |player: usize| {
        heroes
            .iter()
            .find(|hero| hero.player == player && hero.takes(kind))
            .map(|hero| hero.centre)
    };
    if pickup
        .flying_to
        .is_some_and(|player| centre_of(player).is_none())
    {
        // Its hero died: it drops where it is, rather than flying on at its homing speed.
        pickup.flying_to = None;
        pickup.velocity = Vec3::ZERO;
    }
    if pickup.flying_to.is_none() {
        if pickup.age >= tuning.life_ticks {
            return Some(PickupEvent::Expired);
        }
        if pickup.age >= tuning.magnet_delay_ticks {
            let mut nearest: Option<(usize, f32)> = None;
            for hero in heroes
                .iter()
                .filter(|hero| hero.takes(kind) && hero.reaches(pickup.position, tuning))
            {
                let distance = hero.centre.distance(pickup.position);
                if nearest.is_none_or(|(_, d)| distance < d) {
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
        && let Some(centre) = centre_of(player)
    {
        // Straight to him, through anything in the way, as the games' bolts go.
        let speed = (pickup.velocity.length().max(tuning.fly_speed)) + tuning.fly_accel * TICK;
        let to = centre - pickup.position;
        let distance = to.length();
        if distance <= tuning.collect_radius + speed * TICK {
            return Some(PickupEvent::Collected(player));
        }
        pickup.velocity = to / distance * speed;
        pickup.position += pickup.velocity * TICK;
        return None;
    }
    fall(pickup, tuning, world);
    None
}

/// Falls, bounces and slides along the ground for a tick. Returns whether it touched
/// anything; touching the ground marks it landed.
fn fall(pickup: &mut PickupMotion, tuning: &PickupTuning, world: &dyn CollisionWorld) -> bool {
    pickup.velocity.y -= tuning.gravity * TICK;
    let motion = pickup.velocity * TICK;
    let Some(hit) = world.cast_sphere(tuning.radius, pickup.position, motion) else {
        pickup.position += motion;
        return false;
    };
    pickup.position += motion * hit.fraction;
    let into = pickup.velocity.dot(hit.normal);
    if into < 0.0 {
        pickup.velocity -= hit.normal * into * (1.0 + tuning.bounce);
    }
    if hit.normal.y > 0.5 {
        pickup.landed = true;
        if pickup.velocity.y.abs() < tuning.rest_speed {
            pickup.velocity.y = 0.0;
        }
        let across = Vec3::new(pickup.velocity.x, 0.0, pickup.velocity.z);
        let slower = (across.length() - tuning.ground_brake * TICK).max(0.0);
        let across = across.normalize_or_zero() * slower;
        pickup.velocity = Vec3::new(across.x, pickup.velocity.y, across.z);
    }
    true
}

/// Whether a pickup shows this tick: bolts and nanotech blink before they expire.
pub fn shows(pickup: &PickupMotion, tuning: &PickupTuning) -> bool {
    if matches!(pickup.kind, PickupKind::Ammo { .. }) {
        return true;
    }
    let left = tuning.life_ticks.saturating_sub(pickup.age);
    pickup.flying_to.is_some() || left > tuning.blink_ticks || (left / 6).is_multiple_of(2)
}

/// Puts `pickups` in the world.
pub fn spawn(commands: &mut Commands, pickups: Vec<PickupMotion>) {
    for pickup in pickups {
        let name = match pickup.kind {
            PickupKind::Bolts(_) => "bolt",
            PickupKind::Nanotech(_) => "nanotech",
            PickupKind::Ammo { .. } => "ammo",
        };
        commands.spawn((
            Name::new(name),
            Pickup(pickup),
            SimTransform::from_translation(pickup.position),
        ));
    }
}

type HeroItem<'a> = (
    &'a Hero,
    &'a HeroBody,
    &'a mut Health,
    Option<&'a mut Bolts>,
    Option<&'a mut Arsenal>,
);

/// Moves every pickup, and gives those taken to their heroes.
pub fn move_pickups(
    tuning: Res<PickupTuning>,
    armory: Res<Armory>,
    hero_tuning: Res<HeroTuning>,
    scene: Res<CollisionScene>,
    mut heroes: Query<HeroItem>,
    mut pickups: Query<(Entity, &mut Pickup, &mut SimTransform)>,
    mut commands: Commands,
) {
    let body = &hero_tuning.0.body;
    let middle = Vec3::Y * ((body.bottom + body.top) / 2.0);
    let mut collectors: Vec<Collector> = heroes
        .iter()
        .map(|(hero, hero_body, _, bolts, arsenal)| Collector {
            player: hero.player,
            feet: hero_body.0.position,
            centre: hero_body.0.position + middle,
            moved: hero_body.0.velocity * TICK,
            alive: hero_body.0.state != HeroState::Dead,
            keeps_bolts: bolts.is_some(),
            guns: arsenal.as_ref().map_or_else(Vec::new, |arsenal| {
                arsenal.guns.iter().map(|gun| gun.weapon).collect()
            }),
            ammo_room: arsenal.map_or_else(Vec::new, |arsenal| ammo_room(arsenal, &armory)),
        })
        .collect();
    collectors.sort_by_key(|collector| collector.player);
    let mut pickups: Vec<_> = pickups.iter_mut().collect();
    pickups.sort_by_key(|(entity, ..)| *entity);
    for (entity, pickup, transform) in &mut pickups {
        let event = step(&mut pickup.0, &collectors, &tuning, &scene.0);
        transform.translation = pickup.0.position;
        let player = match event {
            None => continue,
            Some(PickupEvent::Gone | PickupEvent::Expired) => {
                commands.entity(*entity).despawn();
                continue;
            }
            Some(PickupEvent::Collected(player)) => {
                commands.entity(*entity).despawn();
                player
            }
            Some(PickupEvent::Taken(player)) => {
                pickup.0.taken = true;
                pickup.0.flying_to = Some(player);
                pickup.0.velocity = Vec3::ZERO;
                player
            }
        };
        let Some((hero, _, mut health, bolts, arsenal)) =
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
            PickupKind::Ammo { weapon, rounds } => {
                // What doesn't fit is lost, as `ammo_add` caps it.
                if let Some(mut arsenal) = arsenal {
                    arsenal.add_ammo(weapon, rounds, &armory);
                    // Later pickups this tick see the room he has left.
                    if let Some(collector) = collectors
                        .iter_mut()
                        .find(|collector| collector.player == hero.player)
                    {
                        collector.ammo_room = ammo_room(&arsenal, &armory);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_hero::{BodyPlacement, Collider, ParryWorld};

    const N60: WeaponId = WeaponId(1);
    const NITRO: WeaponId = WeaponId(2);
    const LAVA: WeaponId = WeaponId(3);

    fn floor() -> ParryWorld {
        ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(50.0, 0.5, 50.0),
            },
            Vec3::new(0.0, -0.5, 0.0),
            Quat::IDENTITY,
        )])
    }

    /// A hero standing at (x, 0, z), owning the guns he has room for.
    fn hero(player: usize, x: f32, z: f32, room: &[(WeaponId, u32)]) -> Collector {
        Collector {
            player,
            feet: Vec3::new(x, 0.0, z),
            centre: Vec3::new(x, 0.775, z),
            moved: Vec3::ZERO,
            alive: true,
            keeps_bolts: true,
            guns: room.iter().map(|(gun, _)| *gun).collect(),
            ammo_room: room.to_vec(),
        }
    }

    /// Lying still on the floor, old enough to fly.
    fn lying(kind: PickupKind, tuning: &PickupTuning) -> PickupMotion {
        PickupMotion {
            age: tuning.magnet_delay_ticks,
            landed: true,
            ..PickupMotion::new(kind, Vec3::new(0.0, tuning.radius, 0.0), Vec3::ZERO)
        }
    }

    /// Steps until something becomes of it, within `limit` ticks.
    fn run(
        pickup: &mut PickupMotion,
        heroes: &[Collector],
        tuning: &PickupTuning,
        world: &ParryWorld,
        limit: u32,
    ) -> Option<PickupEvent> {
        (0..limit).find_map(|_| step(pickup, heroes, tuning, world))
    }

    const AMMO: PickupKind = PickupKind::Ammo {
        weapon: N60,
        rounds: 15,
    };

    #[test]
    fn the_bundled_tuning_parses_and_a_broken_file_is_an_error() {
        let tuning = PickupTuning::default();
        assert!(tuning.reach > tuning.collect_radius);
        assert!(PickupTuning::from_ron("(gravity: 30.0)").is_err());
    }

    #[test]
    fn scatter_pops_each_kind_out_upwards() {
        let tuning = PickupTuning::default();
        let mut rng = SimRng::new(7, 0);
        let kinds = vec![PickupKind::Bolts(5); 40];
        let drops = scatter(Vec3::Y, kinds, &tuning, &mut rng);
        assert_eq!(drops.len(), 40);
        for drop in &drops {
            assert_eq!(drop.position, Vec3::Y);
            assert_eq!(drop.thrown_from, 1.0);
            assert!(drop.velocity.y >= tuning.pop_lift.0);
            let across = Vec3::new(drop.velocity.x, 0.0, drop.velocity.z).length();
            assert!(across >= tuning.pop_speed.0 - 1e-4 && across < tuning.pop_speed.1);
        }
    }

    #[test]
    fn a_drop_lands_and_settles_on_the_ground() {
        let tuning = PickupTuning::default();
        let world = floor();
        // The draws the demo's drop table made before this drop's, as when this test was
        // written there: its count and its nanotech roll.
        let mut rng = SimRng::new(1, 0);
        rng.below(4);
        rng.unit_f32();
        let mut drop = scatter(Vec3::Y, vec![PickupKind::Bolts(5)], &tuning, &mut rng)[0];
        for _ in 0..240 {
            assert_eq!(step(&mut drop, &[], &tuning, &world), None);
        }
        assert!(
            (drop.position.y - tuning.radius).abs() < 0.01,
            "{}",
            drop.position
        );
        assert_eq!(drop.velocity, Vec3::ZERO);
        assert!(drop.landed);
    }

    #[test]
    fn a_hero_reaches_8_7_across_and_3_up_or_down_from_his_feet() {
        let tuning = PickupTuning::default();
        let hero = hero(0, 0.0, 0.0, &[]);
        assert!(hero.reaches(Vec3::new(8.6, 2.9, 0.0), &tuning));
        assert!(hero.reaches(Vec3::new(6.0, -2.9, 6.0), &tuning));
        assert!(!hero.reaches(Vec3::new(6.2, 0.0, 6.2), &tuning));
        assert!(!hero.reaches(Vec3::new(0.0, 3.1, 0.0), &tuning));
    }

    #[test]
    fn a_drop_waits_then_flies_to_the_nearest_hero_in_reach() {
        let tuning = PickupTuning::default();
        let world = floor();
        let mut drop = PickupMotion {
            age: 0,
            ..lying(PickupKind::Nanotech(2), &tuning)
        };
        let heroes = [
            hero(0, 0.0, 5.0, &[]),
            hero(1, 3.0, 0.0, &[]),
            hero(2, -20.0, 0.0, &[]),
        ];
        let mut ticks = 0;
        let end = loop {
            ticks += 1;
            if ticks < tuning.magnet_delay_ticks {
                assert_eq!(drop.flying_to, None);
            }
            if let Some(end) = step(&mut drop, &heroes, &tuning, &world) {
                break end;
            }
            assert!(ticks < 120);
        };
        assert_eq!(end, PickupEvent::Collected(1));
        assert!(ticks > tuning.magnet_delay_ticks);
    }

    #[test]
    fn a_drop_whose_hero_dies_on_its_way_falls_where_it_is() {
        let tuning = PickupTuning::default();
        let world = floor();
        let mut drop = lying(PickupKind::Bolts(5), &tuning);
        let mut heroes = [hero(0, 5.0, 0.0, &[])];
        for _ in 0..8 {
            assert_eq!(step(&mut drop, &heroes, &tuning, &world), None);
        }
        assert_eq!(drop.flying_to, Some(0));
        let caught = drop.position;
        heroes[0].alive = false;
        for _ in 0..120 {
            assert_eq!(step(&mut drop, &heroes, &tuning, &world), None);
        }
        assert_eq!(drop.flying_to, None);
        let moved = drop.position - caught;
        assert!(Vec3::new(moved.x, 0.0, moved.z).length() < 0.01, "{moved}");
        assert!(
            (drop.position.y - tuning.radius).abs() < 0.01,
            "{}",
            drop.position
        );
    }

    #[test]
    fn bolts_no_one_collects_blink_and_expire_but_ammo_never_does() {
        let tuning = PickupTuning::default();
        let world = floor();
        let mut drop = PickupMotion {
            age: 0,
            ..lying(PickupKind::Bolts(5), &tuning)
        };
        let mut hidden = 0;
        for _ in 1..tuning.life_ticks {
            assert_eq!(step(&mut drop, &[], &tuning, &world), None);
            hidden += u32::from(!shows(&drop, &tuning));
        }
        assert_eq!(
            step(&mut drop, &[], &tuning, &world),
            Some(PickupEvent::Expired)
        );
        assert!(hidden > 0 && hidden < tuning.blink_ticks, "{hidden}");
        let mut ammo = lying(AMMO, &tuning);
        for _ in 0..tuning.life_ticks * 2 {
            assert_eq!(step(&mut ammo, &[], &tuning, &world), None);
            assert!(shows(&ammo, &tuning));
        }
    }

    #[test]
    fn ammo_is_taken_at_once_in_reach_then_flies_in_and_shrinks_away() {
        let tuning = PickupTuning::default();
        let world = floor();
        let mut drop = lying(AMMO, &tuning);
        // Out of reach, it lies there.
        let far = [hero(0, 9.0, 0.0, &[(N60, 50)])];
        for _ in 0..30 {
            assert_eq!(step(&mut drop, &far, &tuning, &world), None);
        }
        // In reach, it's his that tick, before it flies.
        let near = [hero(0, 8.0, 0.0, &[(N60, 50)])];
        assert_eq!(
            step(&mut drop, &near, &tuning, &world),
            Some(PickupEvent::Taken(0))
        );
        drop.taken = true;
        drop.flying_to = Some(0);
        let mut ticks = 0;
        let mut shrank = false;
        while step(&mut drop, &near, &tuning, &world).is_none() {
            ticks += 1;
            shrank |= drop.scale < 1.0;
            assert!(ticks < 120);
        }
        assert!(shrank);
        // From rest to 8 u away at 24 u/s² takes about 0.8 s.
        assert!((40..56).contains(&ticks), "{ticks}");
    }

    #[test]
    fn ammo_flying_in_is_carried_along_with_its_hero() {
        let tuning = PickupTuning::default();
        let world = floor();
        let mut still = PickupMotion {
            taken: true,
            flying_to: Some(0),
            ..lying(AMMO, &tuning)
        };
        let mut carried = still;
        let standing = [hero(0, 8.0, 0.0, &[])];
        let mut running = standing.clone();
        running[0].moved = Vec3::new(0.0, 0.0, 0.2);
        for _ in 0..10 {
            step(&mut still, &standing, &tuning, &world);
            step(&mut carried, &running, &tuning, &world);
        }
        // Carried 2 u along with him, less a little homing back to where he stands.
        let carried = carried.position.z - still.position.z;
        assert!((1.8..=2.0).contains(&carried), "{carried}");
    }

    #[test]
    fn ammo_in_the_air_is_taken_as_it_lands_if_he_came_in_reach() {
        let tuning = PickupTuning::default();
        let world = floor();
        let thrown = PickupMotion::new(AMMO, Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 5.0, 0.0));
        // In reach while it flies up, nothing yet; then he walks off.
        let mut drop = thrown;
        let near = [hero(0, 3.0, 0.0, &[(N60, 50)])];
        let gone = [hero(0, 20.0, 0.0, &[(N60, 50)])];
        assert_eq!(step(&mut drop, &near, &tuning, &world), None);
        assert_eq!(drop.armed, Some(0));
        let landed = (1..60)
            .find_map(|tick| step(&mut drop, &gone, &tuning, &world).map(|event| (tick, event)));
        let (tick, event) = landed.expect("it lands");
        assert_eq!(event, PickupEvent::Taken(0));
        assert!(drop.landed && tick > 10, "{tick}");
        // Never in reach in the air, it lies where it lands.
        let mut drop = thrown;
        assert_eq!(run(&mut drop, &gone, &tuning, &world, 120), None);
    }

    #[test]
    fn ammo_fallen_2_u_below_its_throw_goes_to_its_gun_full_or_not() {
        let tuning = PickupTuning::default();
        let world = floor();
        let mut drop = PickupMotion::new(AMMO, Vec3::new(0.0, 5.0, 0.0), Vec3::ZERO);
        let heroes = [
            hero(0, 30.0, 0.0, &[(LAVA, 10)]),
            Collector {
                ammo_room: Vec::new(),
                ..hero(1, 40.0, 0.0, &[(N60, 0)])
            },
        ];
        let event = run(&mut drop, &heroes, &tuning, &world, 60);
        assert_eq!(event, Some(PickupEvent::Taken(1)));
        assert!(drop.position.y < 3.0);
    }

    #[test]
    fn ammo_goes_past_a_nearer_hero_whose_gun_is_full_to_one_with_room() {
        let tuning = PickupTuning::default();
        let world = floor();
        // Player 0 is nearest but his N60 is full; player 1 lacks it; player 2 has room.
        let heroes = [
            hero(0, 1.0, 0.0, &[(LAVA, 20)]),
            hero(1, 0.0, 2.0, &[]),
            hero(2, -4.0, 0.0, &[(N60, 5)]),
        ];
        let mut drop = lying(AMMO, &tuning);
        assert_eq!(
            step(&mut drop, &heroes, &tuning, &world),
            Some(PickupEvent::Taken(2))
        );
        // Bolts go to the nearest whatever his guns.
        let mut bolts = lying(PickupKind::Bolts(5), &tuning);
        assert_eq!(
            run(&mut bolts, &heroes, &tuning, &world, 120),
            Some(PickupEvent::Collected(0))
        );
    }

    #[test]
    fn ammo_no_one_near_can_use_lies_where_it_is() {
        let tuning = PickupTuning::default();
        let world = floor();
        let ammo = PickupKind::Ammo {
            weapon: NITRO,
            rounds: 2,
        };
        let heroes = [hero(0, 1.0, 0.0, &[(N60, 5)])];
        let mut drop = lying(ammo, &tuning);
        for _ in 0..240 {
            assert_eq!(step(&mut drop, &heroes, &tuning, &world), None);
        }
        assert_eq!(drop.flying_to, None);
        // It settles a little into the floor, as resting drops do.
        let moved = drop.position - Vec3::new(0.0, tuning.radius, 0.0);
        assert!(Vec3::new(moved.x, 0.0, moved.z).length() < 0.01, "{moved}");
        assert!(moved.y.abs() < 0.02, "{moved}");
    }

    #[test]
    fn a_hero_takes_ammo_only_for_a_gun_he_owns_with_room() {
        let armory = Armory::uya();
        let mut arsenal = Arsenal::default();
        arsenal.give(N60, &armory);
        arsenal.give(LAVA, &armory);
        assert!(ammo_room(&arsenal, &armory).is_empty());
        arsenal.gun_mut(N60).unwrap().ammo = 140;
        assert_eq!(ammo_room(&arsenal, &armory), [(N60, 10)]);
        let collector = Collector {
            ammo_room: ammo_room(&arsenal, &armory),
            ..hero(0, 0.0, 0.0, &[])
        };
        let n60 = PickupKind::ammo(N60, &armory).unwrap();
        let lava = PickupKind::ammo(LAVA, &armory).unwrap();
        let nitro = PickupKind::ammo(NITRO, &armory).unwrap();
        assert!(collector.takes(n60));
        assert!(!collector.takes(lava));
        assert!(!collector.takes(nitro));
        assert!(
            !Collector {
                alive: false,
                ..collector
            }
            .takes(n60)
        );
    }
}

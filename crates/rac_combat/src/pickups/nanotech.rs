//! The nanotech crate's orbs (`nanotech_holder_update`, 0x301F58; `weapons.md`, "Ammo
//! pickups, crates and the vendor"): a broken nanotech crate leaves its orbs where it
//! stood until a hero near enough has room for what they carry. Then they fly to him on
//! curves fanned round his way, and the first to arrive heals him, once.
//!
//! [`step_cluster`] is a cluster's whole motion as one pure function over the heroes as it
//! sees them ([`Healee`]); [`move_orbs`] runs it for every [`NanotechOrbs`] each tick, in
//! entity order.

use bevy::math::ops;
use bevy::prelude::*;
use rac_hero::{Health, HelpMeasure, Hero, HeroBody, HeroState, HeroTuning};
use rac_sim::{SimRng, SimTransform};
use serde::{Deserialize, Serialize};

use super::{PickupSound, PickupSoundKind, PickupTuning, between};

/// How the orbs wait, are let out and fly.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NanotechTuning {
    /// They are let out to a hero within this far of the crate's middle.
    pub reach: f32,
    /// They heal his most health over the first, at least 1, or with UYA's help measure
    /// at 100 or more over the second, and at 200 or more the third (0x3FE4C0).
    pub divisors: [u32; 3],
    pub orbs: u32,
    /// Each flies for a random number of ticks in this range.
    pub flight: (u32, u32),
    /// How far out square to his way each curve is bowed, before the curve's own
    /// easing (a quarter of it at most), and the degrees between the orbs' bows.
    pub bow: f32,
    pub fan: f32,
    /// They fly to this high above his feet.
    pub height: f32,
    /// Ticks after they are let out before the cluster goes.
    pub life: u32,
    /// Waiting, they circle the crate's middle this far out, turning `spin` degrees a tick.
    pub ring: f32,
    pub spin: f32,
}

/// One orb.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orb {
    pub position: Vec3,
    /// Let out: where it set off, its curve's bow, and its ticks left of `total`.
    pub from: Vec3,
    pub bow: Vec3,
    pub left: u32,
    pub total: u32,
    pub arrived: bool,
}

/// To whom the orbs were let out, the ticks since, and whether he has been healed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Released {
    pub player: usize,
    pub ticks: u32,
    pub healed: bool,
}

/// A broken nanotech crate's orbs.
#[derive(Debug, Clone, PartialEq)]
pub struct OrbCluster {
    pub middle: Vec3,
    pub orbs: Vec<Orb>,
    /// Ticks since the crate broke.
    pub age: u32,
    pub released: Option<Released>,
}

impl OrbCluster {
    /// The orbs of a crate broken with its middle at `middle`.
    pub fn new(middle: Vec3, tuning: &NanotechTuning) -> Self {
        let mut cluster = Self {
            middle,
            orbs: vec![
                Orb {
                    position: middle,
                    from: middle,
                    bow: Vec3::ZERO,
                    left: 0,
                    total: 0,
                    arrived: false,
                };
                tuning.orbs as usize
            ],
            age: 0,
            released: None,
        };
        cluster.circle(tuning);
        cluster
    }

    /// Places the waiting orbs on their ring, every other one a little higher.
    fn circle(&mut self, tuning: &NanotechTuning) {
        let count = self.orbs.len().max(1) as f32;
        for (index, orb) in self.orbs.iter_mut().enumerate() {
            let angle = (self.age as f32 * tuning.spin + index as f32 * 360.0 / count).to_radians();
            let rise = if index % 2 == 0 { 0.08 } else { -0.08 };
            orb.position = self.middle
                + Vec3::new(
                    ops::cos(angle) * tuning.ring,
                    rise,
                    ops::sin(angle) * tuning.ring,
                );
        }
    }

    /// Lets the orbs out to `player`, standing at `feet`: each sets off for a random time
    /// on a curve bowed out square to the way from the middle to him.
    fn release(&mut self, player: usize, feet: Vec3, tuning: &NanotechTuning, rng: &mut SimRng) {
        let way = feet + Vec3::Y * tuning.height - self.middle;
        let axis = way.try_normalize().unwrap_or(Vec3::Z);
        // The up within the plane square to his way, turned a right angle to his side.
        let upward = way.cross(Vec3::Y).cross(way);
        let side = Quat::from_axis_angle(axis, -std::f32::consts::FRAC_PI_2)
            * upward.try_normalize().unwrap_or(Vec3::X)
            * tuning.bow;
        for (index, orb) in self.orbs.iter_mut().enumerate() {
            let turn = Quat::from_axis_angle(axis, (index as f32 * tuning.fan).to_radians());
            orb.from = orb.position;
            orb.bow = way * 0.5 + turn * side;
            orb.total = between(rng, tuning.flight).max(1);
            orb.left = orb.total;
        }
        self.released = Some(Released {
            player,
            ticks: 0,
            healed: false,
        });
    }
}

/// A broken nanotech crate's orbs in the world.
#[derive(Component, Debug, Clone, PartialEq)]
#[require(SimTransform)]
pub struct NanotechOrbs(pub OrbCluster);

/// A hero as the orbs see him.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Healee {
    pub player: usize,
    pub feet: Vec3,
    pub alive: bool,
    /// His health, with what orbs already let out to him carry, is under his most.
    pub room: bool,
}

/// What became of a cluster this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbEvent {
    /// Let out to this player's hero.
    Released(usize),
    /// The first orb reached this player's hero, which heals him.
    Healed(usize),
    /// Its time is up, or its hero is gone: it goes.
    Gone,
}

/// Where a curve from `from` to `to` with the tangents `start` and `end` is `t` of the
/// way along (`cubic`, 0x442098, as the orbs call it).
fn hermite(from: Vec3, to: Vec3, start: Vec3, end: Vec3, t: f32) -> Vec3 {
    let (t2, t3) = (t * t, t * t * t);
    from * (2.0 * t3 - 3.0 * t2 + 1.0)
        + start * (t3 - 2.0 * t2 + t)
        + to * (-2.0 * t3 + 3.0 * t2)
        + end * (t3 - t2)
}

/// One tick of a cluster: `None` while nothing became of it.
pub fn step_cluster(
    cluster: &mut OrbCluster,
    heroes: &[Healee],
    tuning: &NanotechTuning,
    rng: &mut SimRng,
) -> Option<OrbEvent> {
    cluster.age += 1;
    let Some(released) = cluster.released else {
        cluster.circle(tuning);
        let nearest = heroes
            .iter()
            .filter(|hero| hero.alive && hero.room)
            .map(|hero| (hero, hero.feet.distance(cluster.middle)))
            .filter(|(_, distance)| *distance < tuning.reach)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let (hero, _) = nearest?;
        cluster.release(hero.player, hero.feet, tuning, rng);
        return Some(OrbEvent::Released(hero.player));
    };
    let Some(hero) = heroes
        .iter()
        .find(|hero| hero.player == released.player && hero.alive)
    else {
        return Some(OrbEvent::Gone);
    };
    let ticks = released.ticks + 1;
    if ticks >= tuning.life {
        return Some(OrbEvent::Gone);
    }
    let to = hero.feet + Vec3::Y * tuning.height;
    let mut healed = released.healed;
    let mut event = None;
    for orb in cluster.orbs.iter_mut().filter(|orb| !orb.arrived) {
        orb.left = orb.left.saturating_sub(1);
        if orb.left == 0 {
            orb.arrived = true;
            orb.position = to;
            if !healed {
                healed = true;
                event = Some(OrbEvent::Healed(released.player));
            }
            continue;
        }
        let t = 1.0 - orb.left as f32 / orb.total as f32;
        let straight = to - orb.from;
        orb.position = hermite(orb.from, to, straight + orb.bow, straight - orb.bow, t);
    }
    cluster.released = Some(Released {
        ticks,
        healed,
        ..released
    });
    if event.is_none() && cluster.orbs.iter().all(|orb| orb.arrived) {
        return Some(OrbEvent::Gone);
    }
    event
}

/// What the orbs heal a hero whose most health is `most`, with the help measure at
/// `measure`.
pub fn amount(most: u32, measure: u32, tuning: &NanotechTuning) -> u32 {
    let divisor = tuning.divisors[(measure / 100).min(2) as usize];
    (most / divisor.max(1)).max(1)
}

/// Moves every cluster, lets them out to heroes with room and heals those they reach.
pub(crate) fn move_orbs(
    tuning: Res<PickupTuning>,
    hero_tuning: Res<HeroTuning>,
    help: Res<HelpMeasure>,
    mut rng: ResMut<SimRng>,
    mut heroes: Query<(&Hero, &HeroBody, &mut Health)>,
    mut clusters: Query<(Entity, &mut NanotechOrbs, &mut SimTransform)>,
    mut sounds: MessageWriter<PickupSound>,
    mut commands: Commands,
) {
    let tuning = &tuning.nanotech;
    let heal = |player, most| {
        let measure = help.measure(player, hero_tuning.0.hit.help.as_ref());
        amount(most, measure, tuning)
    };
    let mut clusters: Vec<_> = clusters.iter_mut().collect();
    clusters.sort_by_key(|(entity, ..)| *entity);
    // What orbs already let out to each player carry, which counts against his room.
    let mut carried: Vec<(usize, u32)> = Vec::new();
    let carrying = |carried: &[(usize, u32)], player| {
        carried
            .iter()
            .filter(|(to, _)| *to == player)
            .map(|(_, amount)| amount)
            .sum::<u32>()
    };
    for (_, orbs, _) in &clusters {
        if let Some(released) = orbs.0.released.filter(|released| !released.healed)
            && let Some((_, _, health)) = heroes
                .iter()
                .find(|(hero, ..)| hero.player == released.player)
        {
            carried.push((released.player, heal(released.player, health.max)));
        }
    }
    for (entity, orbs, transform) in &mut clusters {
        let mut healees: Vec<Healee> = heroes
            .iter()
            .map(|(hero, body, health)| Healee {
                player: hero.player,
                feet: body.0.position,
                alive: body.0.state != HeroState::Dead,
                room: health.current + carrying(&carried, hero.player) < health.max,
            })
            .collect();
        healees.sort_by_key(|healee| healee.player);
        let was: Vec<bool> = orbs.0.orbs.iter().map(|orb| orb.arrived).collect();
        let event = step_cluster(&mut orbs.0, &healees, tuning, &mut rng);
        transform.translation = orbs.0.middle;
        // Each orb sounds as it arrives (0x301F58).
        for (orb, _) in orbs
            .0
            .orbs
            .iter()
            .zip(was)
            .filter(|(orb, was)| orb.arrived && !was)
        {
            sounds.write(PickupSound::at(PickupSoundKind::Nanotech, orb.position));
        }
        match event {
            None => {}
            Some(OrbEvent::Released(player)) => {
                if let Some((_, _, health)) = heroes.iter().find(|(hero, ..)| hero.player == player)
                {
                    carried.push((player, heal(player, health.max)));
                }
            }
            Some(OrbEvent::Healed(player)) => {
                if let Some((_, _, mut health)) =
                    heroes.iter_mut().find(|(hero, ..)| hero.player == player)
                {
                    let gain = heal(player, health.max);
                    health.current = health.current.saturating_add(gain).min(health.max);
                }
            }
            Some(OrbEvent::Gone) => commands.entity(*entity).despawn(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> NanotechTuning {
        PickupTuning::default().nanotech
    }

    fn healee(x: f32, room: bool) -> Healee {
        Healee {
            player: 0,
            feet: Vec3::new(x, 0.0, 0.0),
            alive: true,
            room,
        }
    }

    #[test]
    fn the_orbs_wait_for_a_hero_near_enough_with_room() {
        let tuning = tuning();
        let mut rng = SimRng::new(1, 0);
        let mut cluster = OrbCluster::new(Vec3::Y * 0.5, &tuning);
        assert_eq!(cluster.orbs.len(), 8);
        for heroes in [vec![healee(12.0, true)], vec![healee(3.0, false)], vec![]] {
            for _ in 0..30 {
                assert_eq!(step_cluster(&mut cluster, &heroes, &tuning, &mut rng), None);
            }
        }
        for orb in &cluster.orbs {
            assert!((orb.position.xz().length() - tuning.ring).abs() < 1e-4);
        }
        assert_eq!(
            step_cluster(&mut cluster, &[healee(9.0, true)], &tuning, &mut rng),
            Some(OrbEvent::Released(0))
        );
    }

    #[test]
    fn let_out_they_bow_out_to_him_and_the_first_there_heals_him_once() {
        let tuning = tuning();
        let mut rng = SimRng::new(2, 0);
        let middle = Vec3::Y * 0.5;
        let mut cluster = OrbCluster::new(middle, &tuning);
        let heroes = [healee(6.0, true)];
        step_cluster(&mut cluster, &heroes, &tuning, &mut rng);
        let first = cluster.orbs.iter().map(|orb| orb.total).min().unwrap();
        assert!(
            cluster
                .orbs
                .iter()
                .all(|orb| (20..=80).contains(&orb.total))
        );
        let to = Vec3::new(6.0, tuning.height, 0.0);
        let mut heals = Vec::new();
        let mut widest: f32 = 0.0;
        for tick in 1..=80 {
            match step_cluster(&mut cluster, &heroes, &tuning, &mut rng) {
                Some(OrbEvent::Healed(player)) => heals.push((tick, player)),
                Some(OrbEvent::Gone) => break,
                _ => {}
            }
            for orb in cluster.orbs.iter().filter(|orb| !orb.arrived) {
                // How far off the straight line from the middle to him.
                let along = (to - middle).normalize();
                let off = orb.position - middle;
                widest = widest.max((off - along * off.dot(along)).length());
            }
        }
        assert_eq!(heals, [(first, 0)]);
        assert!(cluster.orbs.iter().all(|orb| orb.arrived));
        // A quarter of the 5 u bow at most, but well off the line.
        assert!(widest > 0.8 && widest < 1.4, "{widest}");
    }

    #[test]
    fn they_heal_a_fifth_of_the_most_at_least_one() {
        let tuning = tuning();
        assert_eq!(amount(10, 0, &tuning), 2);
        assert_eq!(amount(80, 0, &tuning), 16);
        assert_eq!(amount(3, 0, &tuning), 1);
        // The help measure's 100 and 200 (2 and 4 deaths at 50).
        assert_eq!(amount(80, 100, &tuning), 20);
        assert_eq!(amount(80, 299, &tuning), 26);
        assert_eq!(amount(80, 5000, &tuning), 26);
    }

    #[test]
    fn they_go_with_their_hero() {
        let tuning = tuning();
        let mut rng = SimRng::new(3, 0);
        let mut cluster = OrbCluster::new(Vec3::ZERO, &tuning);
        step_cluster(&mut cluster, &[healee(2.0, true)], &tuning, &mut rng);
        assert_eq!(
            step_cluster(&mut cluster, &[], &tuning, &mut rng),
            Some(OrbEvent::Gone)
        );
    }
}

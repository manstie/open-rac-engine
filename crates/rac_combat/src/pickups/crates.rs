//! Crates (PLAN.md P1.10): targets that break when their hit points run out, as UYA's do
//! on any hit, and scatter what they hold from their middle (`weapons.md`, "Ammo pickups,
//! crates and the vendor"):
//! - a bolt crate its bolts, a quarter more or less at random, in coins of 30000 down to 1
//!   (`moby_killed`, `bolts_drop`). UYA drops at most a few coins and banks the rest for
//!   later kills; the kit drops them all [simplified];
//! - an ammo crate 1 ammo pickup, or 2 once in 5, for one gun of the hero who broke it, at
//!   random among those not full, else among all, never one crates don't serve
//!   (`ammo_crate_pick`, 0x440E98).
//!
//! A crate is any target with a [`Crate`] beside its hurtbox and hit points
//! (`crate::target_bundle`); [`break_crates`] despawns it when it is [`Destroyed`].

use bevy::prelude::*;
use rac_hero::Hero;
use rac_sim::{SimRng, SimTransform};
use serde::{Deserialize, Serialize};

use super::{PickupKind, PickupTuning, between, scatter, spawn};
use crate::weapons::{Armory, Arsenal, WeaponId};
use crate::{Destroyed, Hurtbox, position};

/// A crate, and what kind it is.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crate(pub CrateKind);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CrateKind {
    /// Bolts: the games' commonest crate.
    #[default]
    Bolts,
    /// Ammo for the guns of whoever breaks it.
    Ammo,
}

/// What each kind of crate holds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CrateTable {
    /// A bolt crate's bolts.
    pub bolts: u32,
    /// An ammo crate throws out 2 pickups once in this many, else 1.
    pub two_pickups_in: u32,
}

/// The coins bolts are dropped in, largest first (`bolts_drop`, 0x44A0E8) [code].
pub const COINS: [u32; 10] = [30000, 10000, 3000, 1000, 500, 100, 50, 20, 5, 1];

/// `bolts` a quarter more or less at random, or 1 under 8, as UYA's kills and crates pay
/// out (`moby_killed`, 0x4412D0) [code].
pub fn bolts_paid(bolts: u32, rng: &mut SimRng) -> u32 {
    let spread = match bolts {
        0 => return 0,
        8.. => bolts / 4,
        2..8 => 1,
        _ => 0,
    };
    between(rng, ((bolts - spread).max(1), bolts + spread))
}

/// `bolts` as coins, taking the largest that fit first.
pub fn coins(mut bolts: u32) -> Vec<u32> {
    let mut coins = Vec::new();
    for coin in COINS {
        while bolts >= coin {
            coins.push(coin);
            bolts -= coin;
        }
    }
    coins
}

/// The gun an ammo crate broken by a hero with `arsenal` throws ammo out for: one of his
/// guns that uses ammo and that crates serve, at random among those not full, else among
/// all; `None` if he has none.
pub fn ammo_crate_gun(arsenal: &Arsenal, armory: &Armory, rng: &mut SimRng) -> Option<WeaponId> {
    let served: Vec<(WeaponId, bool)> = arsenal
        .guns
        .iter()
        .filter_map(|gun| {
            let def = armory.get(gun.weapon)?;
            let most = def.level(gun.level).ammo;
            (most > 0 && def.crate_ammo && def.pickup_rounds > 0)
                .then_some((gun.weapon, gun.ammo < most))
        })
        .collect();
    let not_full: Vec<WeaponId> = served
        .iter()
        .filter(|(_, room)| *room)
        .map(|(gun, _)| *gun)
        .collect();
    let from: Vec<WeaponId> = if not_full.is_empty() {
        served.iter().map(|(gun, _)| *gun).collect()
    } else {
        not_full
    };
    (!from.is_empty()).then(|| from[rng.below(from.len() as u32) as usize])
}

/// What a crate of `kind` holds, broken by a hero with the guns `breaker`. An ammo crate
/// broken by a hero without guns crates serve holds nothing.
pub fn contents(
    kind: CrateKind,
    table: &CrateTable,
    breaker: Option<&Arsenal>,
    armory: &Armory,
    rng: &mut SimRng,
) -> Vec<PickupKind> {
    match kind {
        CrateKind::Bolts => coins(bolts_paid(table.bolts, rng))
            .into_iter()
            .map(PickupKind::Bolts)
            .collect(),
        CrateKind::Ammo => {
            let Some(weapon) = breaker.and_then(|arsenal| ammo_crate_gun(arsenal, armory, rng))
            else {
                return Vec::new();
            };
            let count = if rng.below(table.two_pickups_in.max(1)) == 0 {
                2
            } else {
                1
            };
            PickupKind::ammo(weapon, armory)
                .map(|ammo| vec![ammo; count])
                .unwrap_or_default()
        }
    }
}

/// Breaks the crates knocked out this tick, in the order they were, scattering what they
/// hold.
pub(crate) fn break_crates(
    tuning: Res<PickupTuning>,
    armory: Res<Armory>,
    mut rng: ResMut<SimRng>,
    mut destroyed: MessageReader<Destroyed>,
    crates: Query<(
        &Crate,
        Option<&Hurtbox>,
        Option<&Transform>,
        Option<&SimTransform>,
    )>,
    heroes: Query<(&Hero, &Arsenal)>,
    mut commands: Commands,
) {
    for event in destroyed.read() {
        let Ok((kind, hurtbox, transform, simulated)) = crates.get(event.target) else {
            continue;
        };
        let Some(at) = position(transform, simulated) else {
            continue;
        };
        let middle = at + Vec3::Y * hurtbox.map_or(0.0, |hurtbox| hurtbox.height);
        let breaker = heroes
            .iter()
            .find(|(hero, _)| hero.player == event.player)
            .map(|(_, arsenal)| arsenal);
        let kinds = contents(kind.0, &tuning.crates, breaker, &armory, &mut rng);
        spawn(&mut commands, scatter(middle, kinds, &tuning, &mut rng));
        commands.entity(event.target).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N60: WeaponId = WeaponId(1);
    const NITRO: WeaponId = WeaponId(2);
    const LAVA: WeaponId = WeaponId(3);
    const FLUX: WeaponId = WeaponId(5);

    const TABLE: CrateTable = CrateTable {
        bolts: 44,
        two_pickups_in: 5,
    };

    #[test]
    fn bolts_come_a_quarter_more_or_less_in_the_largest_coins() {
        assert_eq!(coins(44), [20, 20, 1, 1, 1, 1]);
        assert_eq!(coins(31_234), [30000, 1000, 100, 100, 20, 5, 5, 1, 1, 1, 1]);
        assert_eq!(coins(0), [] as [u32; 0]);
        let mut rng = SimRng::new(3, 0);
        let paid: Vec<u32> = (0..400).map(|_| bolts_paid(44, &mut rng)).collect();
        assert_eq!(paid.iter().min(), Some(&33));
        assert_eq!(paid.iter().max(), Some(&55));
        let paid: Vec<u32> = (0..100).map(|_| bolts_paid(5, &mut rng)).collect();
        assert_eq!((paid.iter().min(), paid.iter().max()), (Some(&4), Some(&6)));
        assert!((0..20).all(|_| bolts_paid(1, &mut rng) == 1));
        assert_eq!(bolts_paid(0, &mut rng), 0);
    }

    #[test]
    fn a_bolt_crate_holds_its_bolts_in_coins() {
        let armory = Armory::uya();
        let mut rng = SimRng::new(3, 0);
        for _ in 0..50 {
            let held = contents(CrateKind::Bolts, &TABLE, None, &armory, &mut rng);
            let total: u32 = held
                .iter()
                .map(|kind| match kind {
                    PickupKind::Bolts(value) => *value,
                    other => panic!("{other:?}"),
                })
                .sum();
            assert!((33..=55).contains(&total), "{total}");
        }
    }

    #[test]
    fn an_ammo_crate_serves_a_gun_not_full_one_pickup_or_two() {
        let armory = Armory::uya();
        let mut rng = SimRng::new(3, 0);
        let mut arsenal = Arsenal::default();
        for weapon in [N60, NITRO, LAVA, FLUX] {
            arsenal.give(weapon, &armory);
        }
        assert_eq!(armory.get(FLUX).unwrap().name, "Flux Rifle");
        arsenal.gun_mut(LAVA).unwrap().ammo = 0;
        arsenal.gun_mut(FLUX).unwrap().ammo = 0;
        // Only the Lava Gun isn't full, the Flux Rifle not being served.
        let lava = PickupKind::ammo(LAVA, &armory).unwrap();
        let mut counts = [0; 3];
        for _ in 0..500 {
            let held = contents(CrateKind::Ammo, &TABLE, Some(&arsenal), &armory, &mut rng);
            assert!(held.iter().all(|kind| *kind == lava), "{held:?}");
            counts[held.len()] += 1;
        }
        assert_eq!(counts[0], 0);
        assert!((60..140).contains(&counts[2]), "{counts:?}");
        // All full: any served gun, never the Flux Rifle.
        arsenal.gun_mut(LAVA).unwrap().ammo = 200;
        arsenal.gun_mut(FLUX).unwrap().ammo = 0;
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..200 {
            seen.insert(ammo_crate_gun(&arsenal, &armory, &mut rng).unwrap());
        }
        assert!(!seen.contains(&FLUX));
        assert!(arsenal.gun(FLUX).unwrap().ammo == 0);
        // Without guns, nothing.
        let held = contents(CrateKind::Ammo, &TABLE, None, &armory, &mut rng);
        assert!(held.is_empty());
    }
}

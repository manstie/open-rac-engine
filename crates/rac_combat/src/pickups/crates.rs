//! Crates (PLAN.md P1.10): targets that break when their hit points run out, as UYA's do
//! on one hit, and scatter what they hold from their middle: bolts, or ammo for the guns of
//! the hero who broke them.
//!
//! A crate is any target with a [`Crate`] beside its hurtbox and hit points
//! (`crate::target_bundle`); [`break_crates`] despawns it when it is [`Destroyed`].

use bevy::prelude::*;
use rac_hero::Hero;
use rac_sim::{SimRng, SimTransform};
use serde::{Deserialize, Serialize};

use super::{PickupKind, PickupTuning, ammo_for, between, scatter, spawn};
use crate::weapons::{Armory, Arsenal};
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
    /// A bolt crate's bolts: how many, a range, each worth `bolt_value`.
    pub bolts: (u32, u32),
    pub bolt_value: u32,
    /// An ammo crate's ammo pickups, each for the gun [`ammo_for`] picks.
    pub ammo_pickups: u32,
}

/// What a crate of `kind` holds, broken by a hero with the guns `breaker`. An ammo crate
/// broken by a hero without guns holds nothing.
pub fn contents(
    kind: CrateKind,
    table: &CrateTable,
    breaker: Option<&Arsenal>,
    armory: &Armory,
    rng: &mut SimRng,
) -> Vec<PickupKind> {
    match kind {
        CrateKind::Bolts => {
            let count = between(rng, table.bolts);
            vec![PickupKind::Bolts(table.bolt_value); count as usize]
        }
        CrateKind::Ammo => {
            let Some(arsenal) = breaker else {
                return Vec::new();
            };
            (0..table.ammo_pickups)
                .filter_map(|_| PickupKind::ammo(ammo_for(arsenal, armory, rng)?, armory))
                .collect()
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
    use crate::weapons::{Hand, WeaponId};

    const N60: WeaponId = WeaponId(1);
    const LAVA: WeaponId = WeaponId(3);

    const TABLE: CrateTable = CrateTable {
        bolts: (4, 7),
        bolt_value: 5,
        ammo_pickups: 2,
    };

    #[test]
    fn a_bolt_crate_holds_its_tables_bolts() {
        let armory = Armory::uya();
        let mut rng = SimRng::new(3, 0);
        let counts: Vec<usize> = (0..200)
            .map(|_| {
                let held = contents(CrateKind::Bolts, &TABLE, None, &armory, &mut rng);
                assert!(held.iter().all(|kind| *kind == PickupKind::Bolts(5)));
                held.len()
            })
            .collect();
        assert_eq!(counts.iter().min(), Some(&4));
        assert_eq!(counts.iter().max(), Some(&7));
    }

    #[test]
    fn an_ammo_crate_holds_ammo_for_the_breakers_gun_in_hand() {
        let armory = Armory::uya();
        let mut rng = SimRng::new(3, 0);
        let mut arsenal = Arsenal::default();
        arsenal.give(N60, &armory);
        arsenal.give(LAVA, &armory);
        arsenal.gun_mut(LAVA).unwrap().ammo = 0;
        arsenal.gun_mut(N60).unwrap().ammo = 0;
        arsenal.hand = Hand::Gun {
            weapon: LAVA,
            ticks: 10,
        };
        let held = contents(CrateKind::Ammo, &TABLE, Some(&arsenal), &armory, &mut rng);
        let lava = PickupKind::ammo(LAVA, &armory).unwrap();
        assert_eq!(held, [lava, lava]);
        // Without guns, nothing.
        let held = contents(CrateKind::Ammo, &TABLE, None, &armory, &mut rng);
        assert!(held.is_empty());
    }
}

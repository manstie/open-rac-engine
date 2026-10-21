//! A vendor stub (PLAN.md P1.10): a pad that, while a hero stands on it, tops up the gun he
//! is using a round at a time, at the gun's price in bolts, up to its maximum, as UYA's
//! vendor sells rounds (`weapons.md`, "Owning, ammo, levels and XP": row +0x84 bolts a
//! round, up to the maximum [code]). The pad, its pace and buying with no button are the
//! kit's own [placeholder], until the vendor's menu (P1.12).

use bevy::prelude::*;
use rac_hero::{Hero, HeroBody, HeroState};
use rac_sim::SimTick;
use serde::{Deserialize, Serialize};

use super::{Bolts, PickupTuning, current_gun};
use crate::weapons::{Armory, Arsenal};

/// A hero whose feet are in this box sells rounds; it is centred on the entity's
/// `Transform`, as a `rac_hero::Checkpoint` is.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Vendor {
    pub half_extents: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VendorTuning {
    /// Ticks between rounds bought.
    pub round_ticks: u32,
}

/// Buys a round for the gun he is using if it has room and he has its price, returning
/// whether he did. His bolts never go below 0.
pub fn buy_round(arsenal: &mut Arsenal, bolts: &mut Bolts, armory: &Armory) -> bool {
    let Some(weapon) = current_gun(arsenal) else {
        return false;
    };
    let (Some(def), Some(gun)) = (armory.get(weapon), arsenal.gun(weapon)) else {
        return false;
    };
    let level = def.level(gun.level);
    if level.ammo == 0 || gun.ammo >= level.ammo || bolts.0 < level.round_price {
        return false;
    }
    arsenal.add_ammo(weapon, 1, armory);
    bolts.0 -= level.round_price;
    true
}

/// Sells a round every `round_ticks` to each living hero on a vendor's pad. The pace comes
/// from the tick, so every peer sells on the same ticks.
pub(crate) fn run_vendors(
    tick: Res<SimTick>,
    tuning: Res<PickupTuning>,
    armory: Res<Armory>,
    vendors: Query<(&Vendor, &Transform)>,
    mut heroes: Query<(&Hero, &HeroBody, &mut Arsenal, &mut Bolts)>,
) {
    if !tick
        .0
        .is_multiple_of(u64::from(tuning.vendor.round_ticks.max(1)))
    {
        return;
    }
    // Each hero buys only for himself, but in player order all the same.
    let mut heroes: Vec<_> = heroes.iter_mut().collect();
    heroes.sort_by_key(|(hero, ..)| hero.player);
    for (_, body, arsenal, bolts) in &mut heroes {
        if body.0.state == HeroState::Dead {
            continue;
        }
        let feet = body.0.position;
        let on = vendors.iter().any(|(vendor, transform)| {
            (feet - transform.translation)
                .abs()
                .cmple(vendor.half_extents)
                .all()
        });
        if on {
            buy_round(arsenal, bolts, &armory);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::{Hand, WeaponId};

    const BLASTER: WeaponId = WeaponId(0);
    const N60: WeaponId = WeaponId(1);

    #[test]
    fn the_vendor_sells_rounds_for_the_gun_in_hand_while_he_has_the_bolts() {
        let armory = Armory::uya();
        let price = armory.get(N60).unwrap().level(0).round_price;
        assert!(price > 0);
        let mut arsenal = Arsenal::default();
        arsenal.give(BLASTER, &armory);
        arsenal.give(N60, &armory);
        arsenal.gun_mut(N60).unwrap().ammo = 147;
        arsenal.gun_mut(BLASTER).unwrap().ammo = 0;
        arsenal.hand = Hand::Gun {
            weapon: N60,
            ticks: 10,
        };
        let mut bolts = Bolts(price * 2 + price / 2);
        assert!(buy_round(&mut arsenal, &mut bolts, &armory));
        assert!(buy_round(&mut arsenal, &mut bolts, &armory));
        // Not enough left for a third.
        assert!(!buy_round(&mut arsenal, &mut bolts, &armory));
        assert_eq!(arsenal.gun(N60).unwrap().ammo, 149);
        assert_eq!(bolts.0, price / 2);
        // Only up to the maximum, and only the gun he uses.
        bolts.0 = 10_000;
        assert!(buy_round(&mut arsenal, &mut bolts, &armory));
        assert!(!buy_round(&mut arsenal, &mut bolts, &armory));
        assert_eq!(arsenal.gun(N60).unwrap().ammo, 150);
        assert_eq!(arsenal.gun(BLASTER).unwrap().ammo, 0);
        assert_eq!(bolts.0, 10_000 - price);
    }
}

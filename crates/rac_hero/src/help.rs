//! UYA's help measure (`docs/research/weapons.md`, "The help measure"): the deaths in each
//! area of a level, weighted, which lighten the hero's hits where he keeps dying, and
//! raise what the nanotech crate heals (`rac_combat`).
//!
//! - A hero's area is the last [`Checkpoint`](crate::Checkpoint) he reached, as UYA's
//!   checkpoints set it, or the level's start before one. UYA's areas can hold several
//!   checkpoints and weigh their neighbours' deaths; the kit's each weigh their own.
//! - One measure for the world, as UYA's is its one player's; each hero is measured by
//!   his own area.

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::profile::HelpProfile;

/// An area of the level: the checkpoint that begins it, or `None` for the level's start.
pub type Area = Option<Entity>;

#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct HelpMeasure {
    /// The deaths counted in each area.
    pub deaths: BTreeMap<Area, u32>,
    /// The checkpoint each player's hero last reached.
    pub reached: BTreeMap<usize, Entity>,
    /// Ticks since the level began, which the deaths count from.
    pub ticks: u64,
}

impl HelpMeasure {
    pub fn area(&self, player: usize) -> Area {
        self.reached.get(&player).copied()
    }

    /// The deaths counted in `player`'s hero's area.
    pub fn deaths(&self, player: usize) -> u32 {
        self.deaths.get(&self.area(player)).copied().unwrap_or(0)
    }

    /// The measure of `player`'s hero's area (`help_measure`, 0x3FE420); 0 for a game
    /// without one.
    pub fn measure(&self, player: usize, help: Option<&HelpProfile>) -> u32 {
        help.map_or(0, |help| help.weight * self.deaths(player))
    }

    /// Counts a death of `player`'s hero, as UYA's `hero_death_done` (0x4EA040) does once
    /// he is brought back: in his area, from the level's tick `help.count_from.0`, or
    /// `.1` after a fall out of the level.
    pub fn died(&mut self, player: usize, fell_out: bool, help: &HelpProfile) {
        let from = if fell_out {
            help.count_from.1
        } else {
            help.count_from.0
        };
        if self.ticks >= from {
            *self.deaths.entry(self.area(player)).or_default() += 1;
        }
    }

    /// `player`'s hero stands in `checkpoint`. One other than his last becomes his area
    /// and takes `help.refund / help.weight`, rounded up, off its deaths
    /// (`checkpoint_reached`, 0x325A60).
    pub fn reach(&mut self, player: usize, checkpoint: Entity, help: Option<&HelpProfile>) {
        if self.reached.insert(player, checkpoint) == Some(checkpoint) {
            return;
        }
        if let Some(help) = help
            && let Some(deaths) = self.deaths.get_mut(&Some(checkpoint))
        {
            *deaths = deaths.saturating_sub(help.refund.div_ceil(help.weight.max(1)));
        }
    }

    /// What a hit on `player`'s hero is multiplied by (`hero_damage`, 0x4DC1C0): less over
    /// `help.ease_above` with more than `help.ease_after` deaths, more under
    /// `help.harden_below` with none or one.
    pub fn damage_factor(&self, player: usize, help: Option<&HelpProfile>) -> f32 {
        let Some(help) = help else {
            return 1.0;
        };
        let deaths = self.deaths(player);
        let measure = help.weight * deaths;
        let mut factor = 1.0;
        if measure > help.ease_above && deaths > help.ease_after {
            factor *= 1.0 - (help.ease * (deaths - help.ease_after) as f32).min(help.ease_most);
        }
        if measure < help.harden_below {
            match deaths {
                0 => factor *= help.harden.0,
                1 => factor *= help.harden.1,
                _ => {}
            }
        }
        factor
    }
}

/// `damage` times `factor`, rounded, and at least 1 if there was any.
pub fn scale_damage(damage: u32, factor: f32) -> u32 {
    if damage == 0 {
        return 0;
    }
    ((damage as f32 * factor + 0.5) as u32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uya() -> HelpProfile {
        crate::profile::HeroProfile::uya().hit.help.unwrap()
    }

    #[test]
    fn deaths_count_in_the_heros_area_once_the_level_is_under_way() {
        let help = uya();
        let mut measure = HelpMeasure::default();
        measure.died(0, false, &help);
        assert_eq!(measure.deaths(0), 0);
        measure.ticks = 900;
        measure.died(0, true, &help);
        assert_eq!(measure.deaths(0), 0, "a fall counts from 1800");
        measure.died(0, false, &help);
        assert_eq!(measure.deaths(0), 1);
        // Player 1 moved on to a checkpoint of his own, which has none yet.
        measure.reach(1, Entity::from_raw_u32(7).unwrap(), Some(&help));
        assert_eq!(measure.deaths(1), 0);
        measure.died(1, false, &help);
        assert_eq!(measure.measure(1, Some(&help)), 50);
        assert_eq!(measure.measure(1, None), 0);
    }

    #[test]
    fn a_new_checkpoint_takes_two_deaths_off_its_area() {
        let help = uya();
        let (first, second) = (
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
        );
        let mut measure = HelpMeasure {
            ticks: 900,
            ..default()
        };
        measure.reach(0, first, Some(&help));
        for _ in 0..5 {
            measure.died(0, false, &help);
        }
        // Standing in it again takes nothing.
        measure.reach(0, first, Some(&help));
        assert_eq!(measure.deaths(0), 5);
        // Back from another, it takes 100 / 50.
        measure.reach(0, second, Some(&help));
        measure.reach(0, first, Some(&help));
        assert_eq!(measure.deaths(0), 3);
    }

    #[test]
    fn hits_harden_with_few_deaths_and_ease_past_six() {
        let help = uya();
        let mut measure = HelpMeasure::default();
        let factor_at = |measure: &mut HelpMeasure, deaths| {
            measure.deaths.insert(None, deaths);
            measure.damage_factor(0, Some(&help))
        };
        assert_eq!(factor_at(&mut measure, 0), 1.2);
        // One death is already 50, which isn't under 50.
        assert_eq!(factor_at(&mut measure, 1), 1.0);
        assert_eq!(factor_at(&mut measure, 6), 1.0);
        assert!((factor_at(&mut measure, 7) - 0.93).abs() < 1e-6);
        assert!((factor_at(&mut measure, 9) - 0.79).abs() < 1e-6);
        assert_eq!(factor_at(&mut measure, 20), 0.5);
        assert_eq!(measure.damage_factor(0, None), 1.0);
        assert_eq!(scale_damage(2, 1.2), 2);
        assert_eq!(scale_damage(3, 1.2), 4);
        assert_eq!(scale_damage(1, 0.5), 1);
        assert_eq!(scale_damage(0, 1.2), 0);
    }
}

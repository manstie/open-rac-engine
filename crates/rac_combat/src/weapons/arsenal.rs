//! A hero's guns (`weapons.md`, "Owning, ammo, levels and XP" and "Switching and firing"):
//! the ones he owns with their ammo, XP and levels, the one he brings out, the recently
//! chosen ones the quick switch goes round, the quick-select rings, and what is in his
//! hand.

use bevy::prelude::*;

use super::{
    def::{Armory, Mods, WeaponId},
    fire::Trigger,
    ring::QuickSelect,
    shots::Shot,
};

/// One gun a hero owns (UYA's per-item arrays at 0x1425C0 on).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnedGun {
    pub weapon: WeaponId,
    /// 0 is V1 (`item_weapon_rows`).
    pub level: u8,
    /// Total XP, in whole points (the game counts 1/32 points).
    pub xp: u32,
    pub ammo: u32,
    /// Mods given on top of its level's, as Deadlocked's are.
    pub extra_mods: Mods,
}

/// What is in the hero's hand.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    #[default]
    Wrench,
    /// Bringing `weapon` out: it is in his hand when `ticks_left` runs out.
    Drawing { weapon: WeaponId, ticks_left: u32 },
    /// `weapon` in his hand for `ticks`.
    Gun { weapon: WeaponId, ticks: u32 },
}

impl Hand {
    /// The gun in his hand, not one still coming out.
    pub fn gun(self) -> Option<WeaponId> {
        match self {
            Hand::Gun { weapon, .. } => Some(weapon),
            _ => None,
        }
    }
}

/// A gun's level went up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelUp {
    pub weapon: WeaponId,
    pub level: u8,
}

/// A hero's guns. Put it beside the [`rac_hero::Hero`]; the weapons code fires them and
/// tells his moves what he holds.
#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct Arsenal {
    /// In the order they were given.
    pub guns: Vec<OwnedGun>,
    /// The gun the fire button brings out (`equipped_gun`, 0x1D4C40).
    pub equipped: Option<WeaponId>,
    /// The guns chosen most recently, newest first (`recent_guns`, 0x142670).
    pub recent: Vec<WeaponId>,
    /// The quick-select rings, and the one on screen while it is open.
    pub quick: QuickSelect,
    pub hand: Hand,
    /// The shots in flight.
    pub shots: Vec<Shot>,
    /// Shots so far, which number them.
    pub(crate) shot_serial: u32,
    /// The gun in hand's own state.
    pub(crate) trigger: Trigger,
    /// Ticks left of the firing animation.
    pub(crate) fire_ticks: u32,
    /// Shots so far, counting round, which restart the firing animation.
    pub(crate) shots_fired: u8,
    /// Ticks since the quick-select button was pressed, while it is held.
    pub(crate) quick_held: Option<u32>,
    /// His moves were last told a gun is in his hand.
    pub(crate) shown_out: bool,
}

impl Arsenal {
    pub fn gun(&self, weapon: WeaponId) -> Option<&OwnedGun> {
        self.guns.iter().find(|gun| gun.weapon == weapon)
    }

    pub fn gun_mut(&mut self, weapon: WeaponId) -> Option<&mut OwnedGun> {
        self.guns.iter_mut().find(|gun| gun.weapon == weapon)
    }

    /// Gives him a gun (`item_give`, 0x416B38): owned at V1 with its ammo full, put in the
    /// quick-select rings, and brought out by the fire button if nothing else is. Giving one
    /// he owns only tops up an empty gun.
    pub fn give(&mut self, weapon: WeaponId, armory: &Armory) {
        let Some(def) = armory.get(weapon) else {
            return;
        };
        let ammo = def.level(0).ammo;
        match self.gun_mut(weapon) {
            Some(gun) => {
                if gun.ammo == 0 {
                    gun.ammo = def.level(gun.level).ammo;
                }
            }
            None => {
                self.guns.push(OwnedGun {
                    weapon,
                    level: 0,
                    xp: 0,
                    // The game's first ammo is row +0x8E, not tabulated yet [placeholder:
                    // full].
                    ammo,
                    extra_mods: Mods::default(),
                });
                self.quick.add(weapon);
            }
        }
        if self.equipped.is_none() {
            self.equipped = Some(weapon);
            self.push_recent(weapon, armory);
        }
    }

    /// Gives him a gun at `level`, as `weapon_set_level` (0x416790) does.
    pub fn give_at(&mut self, weapon: WeaponId, level: u8, armory: &Armory) {
        self.give(weapon, armory);
        let Some(def) = armory.get(weapon) else {
            return;
        };
        let level = level.min(def.levels.len().saturating_sub(1) as u8);
        if let Some(gun) = self.gun_mut(weapon) {
            gun.level = level;
            // The XP the level starts from: the last threshold passed in its chain.
            gun.xp = if level >= armory.rules.bought_level {
                (armory.rules.bought_level..level)
                    .map(|l| def.level(l).xp)
                    .next_back()
                    .unwrap_or(0)
            } else {
                level.checked_sub(1).map_or(0, |l| def.level(l).xp)
            };
            gun.ammo = def.level(level).ammo;
        }
    }

    /// The mods on for `weapon` now.
    pub fn mods(&self, weapon: WeaponId, armory: &Armory) -> Mods {
        match (self.gun(weapon), armory.get(weapon)) {
            (Some(gun), Some(def)) => def.mods_at(gun.level).or(gun.extra_mods),
            _ => Mods::default(),
        }
    }

    /// Takes `rounds` of `weapon`'s ammo if it has them (`ammo_use`, 0x50D8F8). A gun
    /// with no maximum never runs out.
    pub fn use_ammo(&mut self, weapon: WeaponId, rounds: u32, armory: &Armory) -> bool {
        let Some(def) = armory.get(weapon) else {
            return false;
        };
        let Some(gun) = self.gun_mut(weapon) else {
            return false;
        };
        if def.level(gun.level).ammo == 0 {
            return true;
        }
        if gun.ammo < rounds {
            return false;
        }
        gun.ammo -= rounds;
        true
    }

    /// Whether `weapon` has a round to fire.
    pub fn has_ammo(&self, weapon: WeaponId, armory: &Armory) -> bool {
        match (self.gun(weapon), armory.get(weapon)) {
            (Some(gun), Some(def)) => def.level(gun.level).ammo == 0 || gun.ammo > 0,
            _ => false,
        }
    }

    /// Adds ammo up to the maximum (`ammo_add`, 0x50D980), returning what didn't fit.
    pub fn add_ammo(&mut self, weapon: WeaponId, rounds: u32, armory: &Armory) -> u32 {
        let (Some(def), Some(gun)) = (armory.get(weapon), self.gun_mut(weapon)) else {
            return rounds;
        };
        let most = def.level(gun.level).ammo;
        let room = most.saturating_sub(gun.ammo);
        let taken = rounds.min(room);
        gun.ammo += taken;
        rounds - taken
    }

    /// Adds XP to `weapon` and levels it up past each threshold it reaches
    /// (`weapon_level_up`, 0x416640). A gun at the end of its chain, such as V5 before
    /// Mega is bought, gets none.
    pub fn add_xp(&mut self, weapon: WeaponId, xp: u32, armory: &Armory) -> Vec<LevelUp> {
        let mut ups = Vec::new();
        let (Some(def), Some(gun)) = (armory.get(weapon), self.gun_mut(weapon)) else {
            return ups;
        };
        if def.level(gun.level).xp == 0 {
            return ups;
        }
        gun.xp = gun.xp.saturating_add(xp);
        loop {
            let threshold = def.level(gun.level).xp;
            let next = gun.level + 1;
            if threshold == 0 || gun.xp < threshold || usize::from(next) >= def.levels.len() {
                break;
            }
            gun.level = next;
            gun.ammo = def.level(next).ammo;
            ups.push(LevelUp {
                weapon,
                level: next,
            });
        }
        ups
    }

    /// The vendor's upgrade to the bought level, Mega (0x4176F8): XP back to 0, ammo full.
    /// Only from the level before it.
    pub fn buy_upgrade(&mut self, weapon: WeaponId, armory: &Armory) -> Option<LevelUp> {
        let bought = armory.rules.bought_level;
        let def = armory.get(weapon)?;
        let gun = self.gun_mut(weapon)?;
        if gun.level + 1 != bought || usize::from(bought) >= def.levels.len() {
            return None;
        }
        gun.level = bought;
        gun.xp = 0;
        gun.ammo = def.level(bought).ammo;
        Some(LevelUp {
            weapon,
            level: bought,
        })
    }

    /// Makes `weapon` the one the fire button brings out, and the most recent
    /// (`recent_guns_push`, 0x503930). With a gun in hand, it comes out instead.
    pub fn choose(&mut self, weapon: WeaponId, armory: &Armory) {
        if self.gun(weapon).is_none() {
            return;
        }
        self.equipped = Some(weapon);
        self.push_recent(weapon, armory);
        if self.hand != Hand::Wrench && self.hand.gun() != Some(weapon) {
            self.draw(weapon, armory);
        }
    }

    /// Chooses `weapon` in the quick-select ring: it becomes the one the fire button brings
    /// out and the most recent, and comes out at once, even from the wrench, unless he is
    /// `busy` attacking. Choosing a gun out of ammo is allowed [placeholder].
    pub(crate) fn pick(&mut self, weapon: WeaponId, busy: bool, armory: &Armory) {
        self.choose(weapon, armory);
        if !busy && self.hand == Hand::Wrench && self.equipped == Some(weapon) {
            self.draw(weapon, armory);
        }
    }

    /// Starts bringing `weapon` out.
    pub(crate) fn draw(&mut self, weapon: WeaponId, armory: &Armory) {
        self.hand = Hand::Drawing {
            weapon,
            ticks_left: armory.rules.draw_ticks,
        };
        self.trigger = Trigger::default();
        self.fire_ticks = 0;
    }

    /// The wrench back in his hand, remembering the gun.
    pub(crate) fn put_away(&mut self) {
        self.hand = Hand::Wrench;
        self.trigger = Trigger::default();
        self.fire_ticks = 0;
    }

    fn push_recent(&mut self, weapon: WeaponId, armory: &Armory) {
        let rules = &armory.rules;
        let from = self.recent.iter().position(|&recent| recent == weapon);
        if let Some(from) = from {
            self.recent.remove(from);
        }
        // A gun coming in from further back pushes an empty one out of the first few.
        if from.is_none_or(|from| from >= rules.quick_switch_guns)
            && let Some(empty) = self
                .recent
                .iter()
                .take(rules.quick_switch_guns)
                .position(|&recent| !self.has_ammo(recent, armory))
        {
            self.recent.remove(empty);
        }
        self.recent.insert(0, weapon);
        self.recent.truncate(rules.recent_guns);
    }

    /// The quick switch (`hero_quick_switch`, 0x5037C0): with the wrench out, the gun the
    /// fire button brings out; otherwise the next of the most recent guns, turning them
    /// round by one, past any out of ammo.
    pub(crate) fn quick_switch(&mut self, armory: &Armory) {
        let Some(equipped) = self.equipped else {
            return;
        };
        if self.hand == Hand::Wrench {
            self.draw(equipped, armory);
            return;
        }
        let few = self.recent.len().min(armory.rules.quick_switch_guns);
        for _ in 1..few {
            self.recent[..few].rotate_left(1);
            let next = self.recent[0];
            if self.has_ammo(next, armory) {
                self.equipped = Some(next);
                self.draw(next, armory);
                return;
            }
        }
    }

    /// The next (or previous) owned gun after the equipped one, in the order they were
    /// given: the kit's own weapon buttons.
    pub(crate) fn cycle(&mut self, forward: bool, armory: &Armory) {
        if self.guns.is_empty() {
            return;
        }
        let count = self.guns.len();
        let at = self
            .equipped
            .and_then(|equipped| self.guns.iter().position(|gun| gun.weapon == equipped))
            .unwrap_or(0);
        let next = if forward {
            (at + 1) % count
        } else {
            (at + count - 1) % count
        };
        let weapon = self.guns[next].weapon;
        self.choose(weapon, armory);
        if self.hand == Hand::Wrench {
            self.draw(weapon, armory);
        }
    }

    pub(crate) fn next_serial(&mut self) -> u32 {
        self.shot_serial += 1;
        self.shot_serial
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLASTER: WeaponId = WeaponId(0);
    const N60: WeaponId = WeaponId(1);
    const NITRO: WeaponId = WeaponId(2);
    const LAVA: WeaponId = WeaponId(3);

    #[test]
    fn a_gun_given_is_owned_full_and_brought_out_first() {
        let armory = Armory::uya();
        let mut arsenal = Arsenal::default();
        arsenal.give(N60, &armory);
        arsenal.give(BLASTER, &armory);
        assert_eq!(arsenal.gun(N60).unwrap().ammo, 150);
        assert_eq!(arsenal.equipped, Some(N60));
        assert_eq!(arsenal.recent, [N60]);
        assert!(arsenal.use_ammo(N60, 1, &armory));
        assert_eq!(arsenal.gun(N60).unwrap().ammo, 149);
        assert_eq!(arsenal.add_ammo(N60, 5, &armory), 4);
        assert_eq!(arsenal.gun(N60).unwrap().ammo, 150);
    }

    #[test]
    fn xp_levels_a_gun_up_its_chain_and_stops_at_v5_until_mega_is_bought() {
        let armory = Armory::uya();
        let mut arsenal = Arsenal::default();
        arsenal.give(BLASTER, &armory);
        // 150, 400, 700 and 1000 are totals.
        assert!(arsenal.add_xp(BLASTER, 149, &armory).is_empty());
        let ups = arsenal.add_xp(BLASTER, 300, &armory);
        assert_eq!(
            ups,
            [
                LevelUp {
                    weapon: BLASTER,
                    level: 1
                },
                LevelUp {
                    weapon: BLASTER,
                    level: 2
                }
            ]
        );
        // Each level up fills the ammo to the new maximum.
        assert_eq!(arsenal.gun(BLASTER).unwrap().ammo, 40);
        arsenal.add_xp(BLASTER, 10_000, &armory);
        let gun = *arsenal.gun(BLASTER).unwrap();
        assert_eq!(gun.level, 4);
        // At V5 nothing more is counted.
        assert!(arsenal.add_xp(BLASTER, 50_000, &armory).is_empty());
        assert_eq!(arsenal.gun(BLASTER).unwrap().xp, gun.xp);
        // Mega is bought, and counts from 0 to Giga's 10,000.
        assert!(arsenal.buy_upgrade(BLASTER, &armory).is_some());
        assert_eq!(arsenal.gun(BLASTER).unwrap().xp, 0);
        assert!(arsenal.add_xp(BLASTER, 9_999, &armory).is_empty());
        assert_eq!(arsenal.add_xp(BLASTER, 1, &armory).len(), 1);
        assert_eq!(arsenal.gun(BLASTER).unwrap().level, 6);
        assert!(arsenal.buy_upgrade(BLASTER, &armory).is_none());
    }

    #[test]
    fn a_gun_given_at_a_level_starts_from_that_levels_xp() {
        let armory = Armory::uya();
        let mut arsenal = Arsenal::default();
        arsenal.give_at(BLASTER, 2, &armory);
        assert_eq!(arsenal.gun(BLASTER).unwrap().xp, 400);
        assert!(arsenal.mods(BLASTER, &armory).lock_on);
        arsenal.give_at(N60, 6, &armory);
        assert_eq!(arsenal.gun(N60).unwrap().xp, 15_000);
    }

    #[test]
    fn the_quick_switch_goes_round_the_three_most_recent_guns_past_empty_ones() {
        let armory = Armory::uya();
        let mut arsenal = Arsenal::default();
        for weapon in [BLASTER, N60, NITRO, LAVA] {
            arsenal.give(weapon, &armory);
            arsenal.choose(weapon, &armory);
        }
        assert_eq!(arsenal.recent, [LAVA, NITRO, N60, BLASTER]);
        // With the wrench out, the gun the fire button brings.
        arsenal.quick_switch(&armory);
        assert_eq!(
            arsenal.hand,
            Hand::Drawing {
                weapon: LAVA,
                ticks_left: 17
            }
        );
        arsenal.hand = Hand::Gun {
            weapon: LAVA,
            ticks: 30,
        };
        arsenal.quick_switch(&armory);
        assert_eq!(arsenal.equipped, Some(NITRO));
        assert_eq!(arsenal.recent, [NITRO, N60, LAVA, BLASTER]);
        // An empty gun is passed over.
        arsenal.gun_mut(N60).unwrap().ammo = 0;
        arsenal.quick_switch(&armory);
        assert_eq!(arsenal.equipped, Some(LAVA));
        assert_eq!(arsenal.recent, [LAVA, NITRO, N60, BLASTER]);
    }

    #[test]
    fn a_gun_chosen_from_further_back_pushes_an_empty_one_out_of_the_first_three() {
        let armory = Armory::uya();
        let mut arsenal = Arsenal::default();
        for weapon in [BLASTER, N60, NITRO, LAVA] {
            arsenal.give(weapon, &armory);
            arsenal.choose(weapon, &armory);
        }
        arsenal.gun_mut(NITRO).unwrap().ammo = 0;
        arsenal.choose(BLASTER, &armory);
        assert_eq!(arsenal.recent, [BLASTER, LAVA, N60]);
    }
}

//! The vendor (PLAN.md P1.10): a pad that opens a hero's menu as he steps onto it, where he
//! buys guns, rounds for the guns he owns, all their rounds at once and Mega upgrades, by
//! UYA's weapons menu (`weapons.md`, "The vendor's menu": `vendor_weapon_list`,
//! `vendor_list_buttons` and `vendor_buy`) [code].
//!
//! UYA stops the world while its vendor is open. The kit runs the menu over the world, so
//! that co-op peers stay in step, as it does the quick-select ring (PLAN.md §5,
//! 2026-10-22): the menu is part of the simulation, worked only by each tick's
//! `PlayerInput`, and the hero takes no other input while it is open
//! ([`rac_hero::GunHand::selecting`]).
//! - The rows, rebuilt each tick from what he owns, in the armory's order: a rounds row
//!   for each gun he owns that has a price a round and isn't full; buy-all; then each gun
//!   with a price he doesn't own and, with [`VendorTuning::upgrades`] on (UYA's challenge
//!   mode), the upgrade of each gun he owns at the level before the bought one. A rebuilt
//!   list keeps the selection on its gun.
//! - Left and Right (the d-pad's, which are the kit's weapon buttons, or the move stick
//!   pushed) step between rows without wrapping, and again on the 21st, 31st… tick held.
//! - Jump (Cross) on a row he can't buy is refused: a gun or upgrade costing more than
//!   his bolts, rounds for a full gun or when he hasn't one's price, or buy-all with
//!   nothing to fill or too few bolts. Rounds then ask how many, starting from as many as
//!   fit and his bolts buy, which Left and Right change (again every 8 ticks held from 16,
//!   and every tick from 48); the other rows ask once more. Jump buys and quick select
//!   (Triangle) backs out; quick select in the list closes the menu.
//!
//! The pad, opening as a living hero's feet come onto it and not again until he steps off
//! and on, and the menu closing as he dies, are the kit's own [placeholder]: what UYA's
//! vendor moby needs to open isn't read. Not done: UYA's stock, which grows with the story
//! (the kit sells every gun with a price), the 9/10 prices of 0x244EBC, and the last
//! special row [not read].

use bevy::prelude::*;
use rac_hero::{Hero, HeroBody, HeroState};
use rac_input::{Action, PlayerInput, PlayerInputs};
use serde::{Deserialize, Serialize};

use super::{Bolts, PickupSound, PickupSoundKind};
use crate::weapons::{Armory, Arsenal, WeaponId};

/// A hero whose feet come into this box opens his vendor menu; it is centred on the
/// entity's `Transform`, as a `rac_hero::Checkpoint` is.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Vendor {
    pub half_extents: Vec3,
}

/// How the menu is worked, UYA's by default (`weapons.md`, "The vendor's menu").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VendorTuning {
    /// Held Left or Right steps between rows again on each tick held past `row_after`
    /// that is 1 modulo `row_every` (`vendor_list_buttons`: 15 and 10) [code].
    pub row_after: u32,
    pub row_every: u32,
    /// Held, they change the count of rounds again on each tick held past `count_after`
    /// that is a multiple of `count_every`, and on every tick past `count_fast_after`
    /// (`vendor_buy`: 15, 8 and 47) [code].
    pub count_after: u32,
    pub count_every: u32,
    pub count_fast_after: u32,
    /// The move stick pushed this far, in stick counts, is Left or Right [placeholder].
    pub stick: i8,
    /// Sells upgrades to the bought level, as UYA does only in challenge mode [code].
    pub upgrades: bool,
}

impl Default for VendorTuning {
    fn default() -> Self {
        Self {
            row_after: 15,
            row_every: 10,
            count_after: 15,
            count_every: 8,
            count_fast_after: 47,
            stick: 64,
            upgrades: false,
        }
    }
}

/// A row of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// Rounds for a gun he owns.
    Rounds(WeaponId),
    /// Every gun he owns filled.
    BuyAll,
    /// A gun he doesn't own.
    Gun(WeaponId),
    /// A gun's upgrade to the bought level.
    Upgrade(WeaponId),
}

impl Row {
    pub fn weapon(self) -> Option<WeaponId> {
        match self {
            Row::Rounds(weapon) | Row::Gun(weapon) | Row::Upgrade(weapon) => Some(weapon),
            Row::BuyAll => None,
        }
    }
}

/// Where the open menu is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Choosing a row.
    List,
    /// Choosing how many rounds to buy.
    Count(u32),
    /// Asked to buy the row once more.
    Confirm,
}

/// What a press last did, for a HUD's sounds and words (UYA's sounds 7, 2 and 0, and
/// messages 0x17A and 0x17B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    Bought,
    /// Jump on a row he can't buy.
    Refused,
    /// Too few bolts by the time he confirmed.
    Short,
    /// Backed out of a count or a confirm.
    Back,
}

/// What the menu sounds for (`weapons.md`, "Their sounds": the vendor class's 3, 1, 2,
/// 1, 0, 7 and 5).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, Deserialize,
)]
pub enum VendorSound {
    Opened,
    /// The selection moved to another row.
    Moved,
    /// Jump on a row he can't buy.
    Refused,
    /// The count of rounds changed.
    Counted,
    /// Backed out of a count or a confirm.
    Back,
    /// Too few bolts by the time he confirmed.
    Short,
    Bought,
    Closed,
}

/// The menu on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenMenu {
    pub rows: Vec<Row>,
    pub selected: usize,
    pub stage: Stage,
    /// What a press last did, and the ticks since.
    pub said: Option<(Said, u32)>,
}

impl OpenMenu {
    pub fn row(&self) -> Row {
        self.rows[self.selected]
    }
}

/// A hero's vendor menu. Every hero with [`Bolts`] has one.
#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct VendorMenu {
    pub open: Option<OpenMenu>,
    /// His feet were on a vendor's pad last tick.
    on_pad: bool,
    /// Ticks Left and Right have been held (UYA's 0x1E1008 and 0x1E100C).
    left: u32,
    right: u32,
}

/// The menu's rows for what he owns (`vendor_weapon_list`, 0x4B6E88).
pub fn rows(arsenal: &Arsenal, armory: &Armory, tuning: &VendorTuning) -> Vec<Row> {
    let weapons = || (0..armory.weapons.len()).map(WeaponId);
    let mut rows: Vec<Row> = weapons()
        .filter(|&weapon| round_room(arsenal, armory, weapon).is_some_and(|(room, _)| room > 0))
        .map(Row::Rounds)
        .collect();
    rows.push(Row::BuyAll);
    let bought = armory.rules.bought_level;
    for weapon in weapons() {
        let Some(def) = armory.get(weapon) else {
            continue;
        };
        match arsenal.gun(weapon) {
            None if def.price > 0 => rows.push(Row::Gun(weapon)),
            Some(gun)
                if tuning.upgrades
                    && def.upgrade_price > 0
                    && gun.level + 1 == bought
                    && usize::from(bought) < def.levels.len() =>
            {
                rows.push(Row::Upgrade(weapon));
            }
            _ => {}
        }
    }
    rows
}

/// The room left in a gun he owns and its price a round, if the vendor sells its rounds.
fn round_room(arsenal: &Arsenal, armory: &Armory, weapon: WeaponId) -> Option<(u32, u32)> {
    let (def, gun) = (armory.get(weapon)?, arsenal.gun(weapon)?);
    let level = def.level(gun.level);
    (level.round_price > 0 && level.ammo > 0)
        .then_some((level.ammo.saturating_sub(gun.ammo), level.round_price))
}

/// What filling every gun he owns costs: each one's room × its price a round
/// (`vendor_draw_weapons`, 0x4BEED8).
pub fn buy_all_price(arsenal: &Arsenal, armory: &Armory) -> u32 {
    arsenal
        .guns
        .iter()
        .filter_map(|gun| round_room(arsenal, armory, gun.weapon))
        .fold(0u32, |total, (room, price)| {
            total.saturating_add(room.saturating_mul(price))
        })
}

/// What a row costs: a gun's or upgrade's price, one round, or buy-all's total.
pub fn price(row: Row, arsenal: &Arsenal, armory: &Armory) -> u32 {
    match row {
        Row::Rounds(weapon) => round_room(arsenal, armory, weapon).map_or(0, |(_, price)| price),
        Row::BuyAll => buy_all_price(arsenal, armory),
        Row::Gun(weapon) => armory.get(weapon).map_or(0, |def| def.price),
        Row::Upgrade(weapon) => armory.get(weapon).map_or(0, |def| def.upgrade_price),
    }
}

/// The most rounds of `weapon` he can buy now: as many as fit, or as his bolts buy.
pub fn most_rounds(arsenal: &Arsenal, armory: &Armory, weapon: WeaponId, bolts: u32) -> u32 {
    round_room(arsenal, armory, weapon).map_or(0, |(room, price)| room.min(bolts / price))
}

/// Whether Jump on the row goes on to buy it, rather than being refused
/// (`vendor_list_buttons`, 0x4B9A80).
pub fn can_buy(row: Row, arsenal: &Arsenal, armory: &Armory, bolts: u32) -> bool {
    match row {
        Row::Rounds(weapon) => most_rounds(arsenal, armory, weapon, bolts) > 0,
        Row::BuyAll => (1..=bolts).contains(&buy_all_price(arsenal, armory)),
        Row::Gun(weapon) => arsenal.gun(weapon).is_none() && price(row, arsenal, armory) <= bolts,
        Row::Upgrade(_) => price(row, arsenal, armory) <= bolts,
    }
}

/// Buys the row, `rounds` of it for a rounds row, returning whether he could
/// (`vendor_buy`, 0x4B8560). His bolts never go below 0.
pub fn buy(
    row: Row,
    rounds: u32,
    arsenal: &mut Arsenal,
    bolts: &mut Bolts,
    armory: &Armory,
) -> bool {
    let unit = price(row, arsenal, armory);
    let cost = match row {
        Row::Rounds(_) => unit.saturating_mul(rounds),
        _ => unit,
    };
    if rounds == 0 || cost > bolts.0 || !can_buy(row, arsenal, armory, bolts.0) {
        return false;
    }
    match row {
        Row::Rounds(weapon) => {
            // Only the rounds that fit are paid for.
            let left = arsenal.add_ammo(weapon, rounds, armory);
            bolts.0 -= unit * (rounds - left);
            return true;
        }
        Row::BuyAll => {
            for weapon in arsenal
                .guns
                .iter()
                .map(|gun| gun.weapon)
                .collect::<Vec<_>>()
            {
                if round_room(arsenal, armory, weapon).is_some() {
                    arsenal.add_ammo(weapon, u32::MAX, armory);
                }
            }
        }
        Row::Gun(weapon) => arsenal.give(weapon, armory),
        Row::Upgrade(weapon) => {
            if arsenal.buy_upgrade(weapon, armory).is_none() {
                return false;
            }
        }
    }
    bolts.0 -= cost;
    true
}

/// Left and Right this tick: pressed, or stepping again while held.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Steps {
    left: bool,
    right: bool,
}

impl VendorMenu {
    /// Counts how long Left and Right have been held (`vendor_frame`, 0x4BA3E0).
    fn hold(&mut self, input: &PlayerInput, tuning: &VendorTuning) {
        let stick = input.move_stick[0];
        let left = input.held(Action::PrevWeapon) || stick <= -tuning.stick;
        let right = input.held(Action::NextWeapon) || stick >= tuning.stick;
        self.left = if left { self.left.saturating_add(1) } else { 0 };
        self.right = if right {
            self.right.saturating_add(1)
        } else {
            0
        };
    }

    /// The steps Left and Right take, held this many ticks, by `again`.
    fn steps(&self, input: &PlayerInput, again: impl Fn(u32) -> bool) -> Steps {
        Steps {
            left: input.pressed(Action::PrevWeapon) || self.left == 1 || again(self.left),
            right: input.pressed(Action::NextWeapon) || self.right == 1 || again(self.right),
        }
    }

    /// One tick of the menu, worked by `input`; returns what it sounds for.
    pub fn update(
        &mut self,
        input: &PlayerInput,
        arsenal: &mut Arsenal,
        bolts: &mut Bolts,
        armory: &Armory,
        tuning: &VendorTuning,
    ) -> Vec<VendorSound> {
        let mut sounds = Vec::new();
        self.hold(input, tuning);
        let row_steps = self.steps(input, |held| {
            held > tuning.row_after && held % tuning.row_every.max(1) == 1
        });
        let count_steps = self.steps(input, |held| {
            (held > tuning.count_after && held % tuning.count_every.max(1) == 0)
                || held > tuning.count_fast_after
        });
        // The count ticks only on a press, or on held ticks that are multiples of 4 from
        // 16 (`vendor_buy`).
        let ticks = |held: u32| held < 16 || held.is_multiple_of(4);
        let (left_ticks, right_ticks) = (ticks(self.left), ticks(self.right));
        let Some(open) = &mut self.open else {
            return sounds;
        };
        if let Some((_, ticks)) = &mut open.said {
            *ticks = ticks.saturating_add(1);
        }
        refresh(open, arsenal, armory, tuning, bolts.0);
        let back = input.pressed(Action::QuickSelect);
        let buy_pressed = input.pressed(Action::Jump);
        let row = open.row();
        match open.stage {
            Stage::List => {
                if back {
                    self.open = None;
                    sounds.push(VendorSound::Closed);
                    return sounds;
                }
                if row_steps.right && open.selected + 1 < open.rows.len() {
                    open.selected += 1;
                    sounds.push(VendorSound::Moved);
                }
                if row_steps.left && open.selected > 0 {
                    open.selected -= 1;
                    sounds.push(VendorSound::Moved);
                }
                if buy_pressed {
                    let row = open.row();
                    if !can_buy(row, arsenal, armory, bolts.0) {
                        open.said = Some((Said::Refused, 0));
                        sounds.push(VendorSound::Refused);
                    } else if let Row::Rounds(weapon) = row {
                        open.stage = Stage::Count(most_rounds(arsenal, armory, weapon, bolts.0));
                    } else {
                        open.stage = Stage::Confirm;
                    }
                }
            }
            Stage::Count(count) => {
                let most = row
                    .weapon()
                    .map_or(0, |weapon| most_rounds(arsenal, armory, weapon, bolts.0));
                let mut count = count;
                if count_steps.left && count > 1 {
                    count -= 1;
                    if left_ticks {
                        sounds.push(VendorSound::Counted);
                    }
                }
                if count_steps.right && count < most {
                    count += 1;
                    if right_ticks {
                        sounds.push(VendorSound::Counted);
                    }
                }
                open.stage = Stage::Count(count);
                if back {
                    open.stage = Stage::List;
                    open.said = Some((Said::Back, 0));
                    sounds.push(VendorSound::Back);
                } else if buy_pressed {
                    let bought = buy(row, count, arsenal, bolts, armory);
                    sounds.push(said(open, bought));
                }
            }
            Stage::Confirm => {
                if back {
                    open.stage = Stage::List;
                    open.said = Some((Said::Back, 0));
                    sounds.push(VendorSound::Back);
                } else if buy_pressed {
                    let bought = buy(row, 1, arsenal, bolts, armory);
                    sounds.push(said(open, bought));
                }
            }
        }
        refresh(open, arsenal, armory, tuning, bolts.0);
        sounds
    }
}

/// Back to the list after a purchase, saying how it went.
fn said(open: &mut OpenMenu, bought: bool) -> VendorSound {
    open.stage = Stage::List;
    if bought {
        open.said = Some((Said::Bought, 0));
        VendorSound::Bought
    } else {
        open.said = Some((Said::Short, 0));
        VendorSound::Short
    }
}

/// Rebuilds the rows for what he owns now, keeping the selection on its gun, and leaves a
/// count or confirm whose row has gone; a count stays within what he can buy.
fn refresh(
    open: &mut OpenMenu,
    arsenal: &Arsenal,
    armory: &Armory,
    tuning: &VendorTuning,
    bolts: u32,
) {
    let rows = rows(arsenal, armory, tuning);
    let was = open.rows.get(open.selected).copied();
    if rows != open.rows {
        // UYA keeps the selection on the row with the same item; buy-all has none.
        open.selected = was
            .and_then(|was| rows.iter().position(|row| row.weapon() == was.weapon()))
            .unwrap_or(open.selected)
            .min(rows.len() - 1);
        open.rows = rows;
    }
    if open.stage != Stage::List && Some(open.row()) != was {
        open.stage = Stage::List;
    }
    if let (Stage::Count(count), Row::Rounds(weapon)) = (open.stage, open.row()) {
        let most = most_rounds(arsenal, armory, weapon, bolts);
        open.stage = if most == 0 {
            Stage::List
        } else {
            Stage::Count(count.clamp(1, most))
        };
    }
}

/// Opens each living hero's menu as his feet come onto a vendor's pad, closes it as he
/// dies, and works each open menu by his input, in player order.
pub(crate) fn run_vendors(
    tuning: Res<super::PickupTuning>,
    armory: Res<Armory>,
    inputs: Res<PlayerInputs>,
    vendors: Query<(&Vendor, &Transform)>,
    mut heroes: Query<(&Hero, &HeroBody, &mut Arsenal, &mut Bolts, &mut VendorMenu)>,
    mut sounds: MessageWriter<PickupSound>,
) {
    let mut sound = |kind| {
        sounds.write(PickupSound {
            kind: PickupSoundKind::Vendor(kind),
            at: None,
            bolts: 0,
        });
    };
    let tuning = &tuning.vendor;
    let mut heroes: Vec<_> = heroes.iter_mut().collect();
    heroes.sort_by_key(|(hero, ..)| hero.player);
    for (hero, body, arsenal, bolts, menu) in &mut heroes {
        let alive = body.0.state != HeroState::Dead;
        let feet = body.0.position;
        let on = alive
            && vendors.iter().any(|(vendor, transform)| {
                (feet - transform.translation)
                    .abs()
                    .cmple(vendor.half_extents)
                    .all()
            });
        let stepped_on = on && !menu.on_pad;
        menu.on_pad = on;
        if !alive {
            menu.open = None;
        }
        let input = inputs.get(hero.player);
        if stepped_on && menu.open.is_none() {
            // Nothing pressed as it opens counts.
            menu.hold(&input, tuning);
            menu.open = Some(OpenMenu {
                rows: rows(arsenal, &armory, tuning),
                selected: 0,
                stage: Stage::List,
                said: None,
            });
            sound(VendorSound::Opened);
            continue;
        }
        for kind in menu.update(&input, arsenal, bolts, &armory, tuning) {
            sound(kind);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_input::Buttons;

    const BLASTER: WeaponId = WeaponId(0);
    const N60: WeaponId = WeaponId(1);

    fn n60_round() -> u32 {
        Armory::uya().get(N60).unwrap().level(0).round_price
    }

    /// The Shock Blaster and the N60 owned at V1 with these rounds.
    fn owning(blaster: u32, n60: u32) -> Arsenal {
        let armory = Armory::uya();
        let mut arsenal = Arsenal::default();
        arsenal.give(BLASTER, &armory);
        arsenal.give(N60, &armory);
        arsenal.gun_mut(BLASTER).unwrap().ammo = blaster;
        arsenal.gun_mut(N60).unwrap().ammo = n60;
        arsenal
    }

    fn press(actions: &[Action]) -> PlayerInput {
        let mut buttons = Buttons::NONE;
        for &action in actions {
            buttons.insert(action);
        }
        PlayerInput {
            held: buttons,
            pressed: buttons,
            ..default()
        }
    }

    fn hold(actions: &[Action]) -> PlayerInput {
        PlayerInput {
            pressed: Buttons::NONE,
            ..press(actions)
        }
    }

    /// A menu open on its first row.
    fn open(arsenal: &Arsenal, armory: &Armory, tuning: &VendorTuning) -> VendorMenu {
        VendorMenu {
            open: Some(OpenMenu {
                rows: rows(arsenal, armory, tuning),
                selected: 0,
                stage: Stage::List,
                said: None,
            }),
            ..default()
        }
    }

    #[test]
    fn the_rows_are_rounds_for_guns_not_full_then_buy_all_then_guns_for_sale() {
        let armory = Armory::uya();
        let tuning = VendorTuning::default();
        let arsenal = owning(30, 100);
        let list = rows(&arsenal, &armory, &tuning);
        // The Blaster is full; the N60 isn't.
        assert_eq!(&list[..2], &[Row::Rounds(N60), Row::BuyAll]);
        // Every gun with a price he doesn't own, in the armory's order: not the free
        // Nitro Launcher, nor the two he owns.
        let for_sale: Vec<_> = armory
            .weapons
            .iter()
            .enumerate()
            .filter(|(id, def)| def.price > 0 && *id != N60.0)
            .map(|(id, _)| Row::Gun(WeaponId(id)))
            .collect();
        assert_eq!(&list[2..], &for_sale[..]);
        assert!(!list.iter().any(|row| matches!(row, Row::Upgrade(_))));

        // Upgrades only when sold, and only from the level before the bought one.
        let upgrades = VendorTuning {
            upgrades: true,
            ..tuning
        };
        let mut arsenal = arsenal;
        arsenal.gun_mut(N60).unwrap().level = armory.rules.bought_level - 1;
        let list = rows(&arsenal, &armory, &upgrades);
        assert_eq!(
            list.iter()
                .filter(|row| matches!(row, Row::Upgrade(_)))
                .collect::<Vec<_>>(),
            [&Row::Upgrade(N60)]
        );
    }

    #[test]
    fn buy_all_costs_every_gun_s_room_at_its_price_a_round() {
        let armory = Armory::uya();
        let blaster_round = armory.get(BLASTER).unwrap().level(0).round_price;
        let arsenal = owning(20, 100);
        let total = 10 * blaster_round + 50 * n60_round();
        assert_eq!(buy_all_price(&arsenal, &armory), total);
        assert!(!can_buy(Row::BuyAll, &arsenal, &armory, total - 1));
        let mut arsenal = arsenal;
        let mut bolts = Bolts(total + 7);
        assert!(buy(Row::BuyAll, 1, &mut arsenal, &mut bolts, &armory));
        assert_eq!(bolts.0, 7);
        assert_eq!(arsenal.gun(BLASTER).unwrap().ammo, 30);
        assert_eq!(arsenal.gun(N60).unwrap().ammo, 150);
        // Nothing left to fill: refused.
        assert!(!can_buy(Row::BuyAll, &arsenal, &armory, 1_000_000));
    }

    #[test]
    fn rounds_start_at_what_fits_and_the_bolts_buy_and_left_and_right_change_them() {
        let armory = Armory::uya();
        let tuning = VendorTuning::default();
        let price = n60_round();
        let mut arsenal = owning(30, 140);
        let mut bolts = Bolts(price * 6);
        let mut menu = open(&arsenal, &armory, &tuning);
        let mut tick = |menu: &mut VendorMenu, input: PlayerInput, arsenal: &mut Arsenal| {
            menu.update(&input, arsenal, &mut bolts, &armory, &tuning);
        };
        tick(&mut menu, press(&[Action::Jump]), &mut arsenal);
        // The bolts buy 6, the gun has room for 10.
        assert_eq!(menu.open.as_ref().unwrap().stage, Stage::Count(6));
        tick(&mut menu, press(&[Action::NextWeapon]), &mut arsenal);
        assert_eq!(menu.open.as_ref().unwrap().stage, Stage::Count(6));
        tick(&mut menu, PlayerInput::default(), &mut arsenal);
        tick(&mut menu, press(&[Action::PrevWeapon]), &mut arsenal);
        tick(&mut menu, PlayerInput::default(), &mut arsenal);
        tick(&mut menu, press(&[Action::PrevWeapon]), &mut arsenal);
        assert_eq!(menu.open.as_ref().unwrap().stage, Stage::Count(4));
        tick(&mut menu, PlayerInput::default(), &mut arsenal);
        tick(&mut menu, press(&[Action::Jump]), &mut arsenal);
        let open = menu.open.as_ref().unwrap();
        assert_eq!(open.stage, Stage::List);
        assert_eq!(open.said, Some((Said::Bought, 0)));
        assert_eq!(arsenal.gun(N60).unwrap().ammo, 144);
        assert_eq!(bolts.0, price * 2);
    }

    #[test]
    fn held_left_counts_down_on_uya_s_ticks() {
        let armory = Armory::uya();
        let tuning = VendorTuning::default();
        let mut arsenal = owning(30, 0);
        let mut bolts = Bolts(1_000_000);
        let mut menu = open(&arsenal, &armory, &tuning);
        menu.update(
            &press(&[Action::Jump]),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        menu.update(
            &PlayerInput::default(),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        assert_eq!(menu.open.as_ref().unwrap().stage, Stage::Count(150));
        // Held 60 ticks: the press, then 16, 24, 32 and 40, then every tick from 48.
        let mut counts = Vec::new();
        for held in 1..=60 {
            let input = if held == 1 {
                press(&[Action::PrevWeapon])
            } else {
                hold(&[Action::PrevWeapon])
            };
            menu.update(&input, &mut arsenal, &mut bolts, &armory, &tuning);
            let Stage::Count(count) = menu.open.as_ref().unwrap().stage else {
                panic!();
            };
            counts.push(count);
        }
        assert_eq!(counts[0], 149);
        assert_eq!(counts[46], 145);
        assert_eq!(counts[59], 145 - 13);
    }

    #[test]
    fn rows_step_without_wrapping_and_again_on_the_21st_tick_held() {
        let armory = Armory::uya();
        let tuning = VendorTuning::default();
        let mut arsenal = owning(30, 150);
        let mut bolts = Bolts(0);
        let mut menu = open(&arsenal, &armory, &tuning);
        let selected = |menu: &VendorMenu| menu.open.as_ref().unwrap().selected;
        menu.update(
            &press(&[Action::PrevWeapon]),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        assert_eq!(selected(&menu), 0);
        let mut at = Vec::new();
        for _ in 1..=31 {
            let input = PlayerInput {
                move_stick: [127, 0],
                ..default()
            };
            menu.update(&input, &mut arsenal, &mut bolts, &armory, &tuning);
            at.push(selected(&menu));
        }
        assert_eq!(at[0], 1);
        assert_eq!(at[19], 1);
        assert_eq!(at[20], 2);
        assert_eq!(at[30], 3);
        // The last row and no further.
        let last = menu.open.as_ref().unwrap().rows.len() - 1;
        for _ in 0..last + 2 {
            menu.update(
                &PlayerInput::default(),
                &mut arsenal,
                &mut bolts,
                &armory,
                &tuning,
            );
            menu.update(
                &press(&[Action::NextWeapon]),
                &mut arsenal,
                &mut bolts,
                &armory,
                &tuning,
            );
        }
        assert_eq!(selected(&menu), last);
    }

    #[test]
    fn a_gun_he_cannot_pay_for_is_refused_and_one_he_can_is_his_after_a_confirm() {
        let armory = Armory::uya();
        let tuning = VendorTuning::default();
        let mut arsenal = owning(30, 150);
        let menu_rows = rows(&arsenal, &armory, &tuning);
        let at = menu_rows
            .iter()
            .position(|row| matches!(row, Row::Gun(_)))
            .unwrap();
        let Row::Gun(gun) = menu_rows[at] else {
            unreachable!()
        };
        let cost = armory.get(gun).unwrap().price;
        let mut bolts = Bolts(cost - 1);
        let mut menu = open(&arsenal, &armory, &tuning);
        menu.open.as_mut().unwrap().selected = at;
        menu.update(
            &press(&[Action::Jump]),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        let open = menu.open.as_ref().unwrap();
        assert_eq!(
            (open.stage, open.said),
            (Stage::List, Some((Said::Refused, 0)))
        );

        bolts.0 = cost + 5;
        menu.update(
            &PlayerInput::default(),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        menu.update(
            &press(&[Action::Jump]),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        assert_eq!(menu.open.as_ref().unwrap().stage, Stage::Confirm);
        // Backing out buys nothing.
        menu.update(
            &press(&[Action::QuickSelect]),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        assert_eq!(menu.open.as_ref().unwrap().stage, Stage::List);
        assert!(arsenal.gun(gun).is_none());
        menu.update(
            &press(&[Action::Jump]),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        menu.update(
            &PlayerInput::default(),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        menu.update(
            &press(&[Action::Jump]),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        assert!(arsenal.gun(gun).is_some());
        assert_eq!(bolts.0, 5);
        // Its row has gone; the selection stays in the list.
        let open = menu.open.as_ref().unwrap();
        assert!(!open.rows.contains(&Row::Gun(gun)));
        assert!(open.selected < open.rows.len());
        // Quick select in the list closes the menu.
        menu.update(
            &press(&[Action::QuickSelect]),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        assert!(menu.open.is_none());
    }

    #[test]
    fn the_menu_sounds_as_uya_s_does() {
        let armory = Armory::uya();
        let tuning = VendorTuning::default();
        let mut arsenal = owning(30, 0);
        let mut bolts = Bolts(1_000_000);
        let mut menu = open(&arsenal, &armory, &tuning);
        let mut tick = |menu: &mut VendorMenu, input: PlayerInput| {
            menu.update(&input, &mut arsenal, &mut bolts, &armory, &tuning)
        };
        use VendorSound::*;
        assert_eq!(tick(&mut menu, press(&[Action::NextWeapon])), [Moved]);
        assert_eq!(tick(&mut menu, PlayerInput::default()), []);
        assert_eq!(tick(&mut menu, press(&[Action::PrevWeapon])), [Moved]);
        assert_eq!(tick(&mut menu, PlayerInput::default()), []);
        // Nowhere to go, nothing to hear.
        assert_eq!(tick(&mut menu, press(&[Action::PrevWeapon])), []);
        assert_eq!(tick(&mut menu, PlayerInput::default()), []);
        // The N60's 150 rounds: Left held ticks on its press, at 16, 24, 32 and 40, then
        // every fourth tick from 48.
        assert_eq!(tick(&mut menu, press(&[Action::Jump])), []);
        let mut counted = Vec::new();
        for held in 1..=60 {
            let input = if held == 1 {
                press(&[Action::PrevWeapon])
            } else {
                hold(&[Action::PrevWeapon])
            };
            if tick(&mut menu, input) == [Counted] {
                counted.push(held);
            }
        }
        assert_eq!(counted, [1, 16, 24, 32, 40, 48, 52, 56, 60]);
        assert_eq!(tick(&mut menu, press(&[Action::QuickSelect])), [Back]);
        assert_eq!(tick(&mut menu, press(&[Action::Jump])), []);
        assert_eq!(tick(&mut menu, PlayerInput::default()), []);
        assert_eq!(tick(&mut menu, press(&[Action::Jump])), [Bought]);
        assert_eq!(tick(&mut menu, PlayerInput::default()), []);
        assert_eq!(tick(&mut menu, press(&[Action::QuickSelect])), [Closed]);
    }

    #[test]
    fn an_upgrade_takes_the_gun_to_the_bought_level() {
        let armory = Armory::uya();
        let tuning = VendorTuning {
            upgrades: true,
            ..default()
        };
        let mut arsenal = owning(30, 150);
        arsenal.give_at(N60, armory.rules.bought_level - 1, &armory);
        let cost = armory.get(N60).unwrap().upgrade_price;
        let mut bolts = Bolts(cost);
        assert!(buy(Row::Upgrade(N60), 1, &mut arsenal, &mut bolts, &armory));
        assert_eq!(arsenal.gun(N60).unwrap().level, armory.rules.bought_level);
        assert_eq!(bolts.0, 0);
        assert!(!rows(&arsenal, &armory, &tuning).contains(&Row::Upgrade(N60)));
    }

    #[test]
    fn rounds_filled_while_counting_leave_the_count() {
        let armory = Armory::uya();
        let tuning = VendorTuning::default();
        let mut arsenal = owning(30, 140);
        let mut bolts = Bolts(10_000);
        let mut menu = open(&arsenal, &armory, &tuning);
        menu.update(
            &press(&[Action::Jump]),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        assert_eq!(menu.open.as_ref().unwrap().stage, Stage::Count(10));
        // A pickup tops it up part way, then fills it.
        arsenal.gun_mut(N60).unwrap().ammo = 147;
        menu.update(
            &PlayerInput::default(),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        assert_eq!(menu.open.as_ref().unwrap().stage, Stage::Count(3));
        arsenal.gun_mut(N60).unwrap().ammo = 150;
        menu.update(
            &PlayerInput::default(),
            &mut arsenal,
            &mut bolts,
            &armory,
            &tuning,
        );
        let open = menu.open.as_ref().unwrap();
        assert_eq!(open.stage, Stage::List);
        assert_eq!(open.row(), Row::BuyAll);
        assert_eq!(bolts.0, 10_000);
    }
}

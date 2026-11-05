//! The guns' sounds from the user's UYA disc (PLAN.md P1.13, P3.13), played in place of
//! the kit's placeholders (`rac_demo::sfx`) by `rac_audio`'s port of the game's sound
//! library. Nothing is stored: each gun's class sound table and its bank are read from the
//! gadget WAD at start-up, and its shots' from Veldin's core and sound bank
//! (`docs/research/effects.md`, "Sounds"). The crates', pickups' and vendor's sounds
//! ([`PickupSound`], P1.10) are from Veldin's core too ([`pickup_entry`]), and those it
//! lacks from level 4's.
//!
//! - Which entry of a class's table each cue plays is each gun's update's
//!   ([`UYA_GUN_SOUNDS`]). The indices are the same at every level, but from V5 most guns
//!   are another class with a table of its own ([`gun_models::UYA_GUN_MODELS`]).
//! - An entry's volume falls from the camera as the game's `sound_volume` has it
//!   ([`ClassSound::volume_at`]), capped at full, and one quieter than 0x20 doesn't start;
//!   its pan is its angle round the camera, and its pitch is picked between its two
//!   limits. A looped entry only plays as a held loop, which follows the hero.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use bevy::prelude::*;
use rac_audio::{BankId, Play, SoundHandle, Synth};
use rac_combat::pickups::{CrateKind, PickupSound, PickupSoundKind, VendorSound};
use rac_combat::weapons::{Shot, ShotKind};
use rac_combat::{Afflicted, Affliction, Armory, Arsenal, GunEvent};
use rac_demo::sfx::{Cue, Listeners, Render, SfxSystems, SoundClaims, SoundStream, held_cues};
use rac_formats::gadget::{Gadget, GadgetTable};
use rac_formats::level::{Core, Toc, read_uya_sound_bank};
use rac_formats::sound::ClassSound;
use rac_formats::{DiscReader, FormatError};
use rac_hero::{Hero, HeroBody};

use crate::gun_models::UYA_GUN_MODELS;

/// Where a cue's sound is in the game's tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// An entry of the table of the gun's class at its level.
    Gun(usize),
    /// An entry of a shot's class in the level's core.
    Shot { class: u32, entry: usize },
    /// Nothing: the game plays no sound there, so the placeholder mustn't either.
    Silent,
}

/// One of a gun's cues: from which level (0 for V1) it plays which entry, and at least how
/// many 60 Hz frames apart (0 for any). With a `shot`, only for the events of that kind of
/// shot ([`GunEvent::shot`]), before any without; without, for the gun's own and any
/// other shot's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CueSound {
    pub cue: Cue,
    pub level: u8,
    pub entry: Entry,
    pub gap: u8,
    pub shot: Option<&'static str>,
}

const fn gun(cue: Cue, level: u8, entry: usize) -> CueSound {
    CueSound {
        cue,
        level,
        entry: Entry::Gun(entry),
        gap: 0,
        shot: None,
    }
}

const fn shot(cue: Cue, level: u8, class: u32, entry: usize) -> CueSound {
    CueSound {
        cue,
        level,
        entry: Entry::Shot { class, entry },
        gap: 0,
        shot: None,
    }
}

const fn silent(cue: Cue, level: u8) -> CueSound {
    CueSound {
        cue,
        level,
        entry: Entry::Silent,
        gap: 0,
        shot: None,
    }
}

/// `sound`, for the events of `kind`'s shots alone.
const fn of(kind: &'static str, sound: CueSound) -> CueSound {
    CueSound {
        shot: Some(kind),
        ..sound
    }
}

/// The guns' cues by item, from each one's update [code]. An empty gun plays its "no ammo"
/// entry (most are pitched up three semitones, 381) when the trigger is pressed, and once
/// when held on past the last round. A gun's V5 class plays from its own table, by the
/// same update unless one is named.
pub const UYA_GUN_SOUNDS: &[(u16, &[CueSound])] = &[
    // Shock Blaster (0x36A790; V5's 0x385948, whose charged shot plays entry 0 instead).
    // Entry 2 plays when 0x1DD88C is set, for something not yet known.
    (
        0x27,
        &[
            gun(Cue::Fire, 0, 0),
            gun(Cue::Empty, 0, 3),
            gun(Cue::Charge, 0, 4),
            gun(Cue::Fire, 4, 4),
            gun(Cue::Empty, 4, 1),
            gun(Cue::Charge, 4, 3),
        ],
    ),
    // N60 Storm (0x3A5928; V5's 0x322E68 plays the same). Its bullets have no class table.
    (0x2F, &[gun(Cue::Fire, 0, 3), gun(Cue::Empty, 0, 4)]),
    // Nitro Launcher (0x36E360; V5's 0x3893E8 the same); its charge's blast (0x361218).
    (
        0x77,
        &[
            gun(Cue::Fire, 0, 0),
            gun(Cue::Empty, 0, 1),
            shot(Cue::Impact, 0, 0x185A, 0),
        ],
    ),
    // The shots' own sounds are as `effects.md` has them ("The shots' entries"); those
    // that no event marks are [`shot_loop`]'s and [`shot_once`]'s.
    // Lava Gun (0x303F88, both classes): the stream's loop, and a sizzle where it lands
    // (0x304A18), 8 frames apart.
    (
        0x11,
        &[
            gun(Cue::Flow, 0, 0),
            gun(Cue::Empty, 0, 2),
            CueSound {
                gap: 8,
                ..gun(Cue::Impact, 0, 1)
            },
        ],
    ),
    // Annihilator (0x36D5A0; V5's class 0x1887 is taken to play the same, its table
    // matching); its missile's blast (0x396A20), which plays entry 1 from V5.
    (
        0x3F,
        &[
            gun(Cue::Fire, 0, 0),
            gun(Cue::Empty, 0, 1),
            shot(Cue::Impact, 0, 0x1894, 0),
            shot(Cue::Impact, 4, 0x1894, 1),
        ],
    ),
    // Flux Rifle (0x3444B0): the scope's loops zooming in and out.
    (
        0x6F,
        &[
            gun(Cue::Fire, 0, 0),
            gun(Cue::Empty, 0, 1),
            gun(Cue::ZoomIn, 0, 2),
            gun(Cue::ZoomOut, 0, 3),
        ],
    ),
    // Infector (0x3688E8; V5's 0x389F58); its blob's burst (0x3DE4F0), and V5's
    // Infecto-Bomb's (0x38E3A0).
    (
        0x37,
        &[
            gun(Cue::Fire, 0, 0),
            gun(Cue::Empty, 0, 2),
            gun(Cue::Empty, 4, 1),
            of("blob", shot(Cue::Impact, 0, 0x1E05, 1)),
            of("bomb", shot(Cue::Impact, 4, 0x188A, 0)),
        ],
    ),
    // Spitting Hydra (0x31FB90, both classes): entry 1 as it fires, its loop 2 while held,
    // 0 as it marks each target and 5 as it fizzles.
    (
        0x47,
        &[
            gun(Cue::Fire, 0, 1),
            gun(Cue::Empty, 0, 3),
            gun(Cue::Charge, 0, 2),
            gun(Cue::Mark, 0, 0),
            gun(Cue::Fizzle, 0, 5),
        ],
    ),
    // Disc Blade Gun (0x36EFC0, both classes); its disc (0x36FA78) plays 1 as it glances
    // off a wall and 4 as it bursts. Off an enemy, from V3, it plays 2 or 3 by what the
    // enemy is made of, which the kit doesn't know, so 1 there too.
    (
        0x4F,
        &[
            gun(Cue::Fire, 0, 1),
            gun(Cue::Empty, 0, 2),
            shot(Cue::Bounce, 0, 0x1867, 1),
            shot(Cue::Impact, 0, 0x1867, 4),
        ],
    ),
    // Agents of Doom (0x35C9F0): the glove only clicks, empty or with eight agents out;
    // its bank has no other sound, so the throw is silent [data]. An agent's end bursts
    // at V1 (0x3C2140); from V2 what it ends in has no table, nor has its bullet; V5's
    // missiles burst as the small missile does (0x343330).
    (
        0x57,
        &[
            gun(Cue::Empty, 0, 0),
            silent(Cue::Fire, 0),
            of("agent", shot(Cue::Impact, 0, 0x1ADA, 0)),
            of("agent", silent(Cue::Impact, 1)),
            of("comet", silent(Cue::Impact, 0)),
            of("slug", silent(Cue::Impact, 0)),
            of("guided", shot(Cue::Impact, 0, 0xA82, 0)),
        ],
    ),
    // Rift Inducer (0x372990, both classes); its rift ends with its loop, silent, and
    // pulls what it takes in silently until it swallows it ([`play_shots`]).
    (
        0x5F,
        &[
            gun(Cue::Fire, 0, 1),
            gun(Cue::Empty, 0, 5),
            of("rift", silent(Cue::Impact, 0)),
            of("rift", silent(Cue::Arc, 0)),
        ],
    ),
    // Holo Shield Glove (0x3717A8, both classes; 3 when it has none to throw); its shield
    // landing and unfolding (0x376B88).
    (
        0x67,
        &[
            gun(Cue::Fire, 0, 2),
            gun(Cue::Empty, 0, 1),
            of("barrier", shot(Cue::Impact, 0, 0x186D, 2)),
        ],
    ),
    // Plasma Whip (0x356110): a lash plays entry 1 at V1 and V2 and 4 from V3; each rope
    // piece meeting something 6 (0x357F10), where the kit sends one hit a strike. Its
    // idle loop is [`HERO_LOOPS`]'; its "no ammo" plays on the tip link (0x183C's 0); its
    // V4 wave has no table.
    (
        0x7F,
        &[
            gun(Cue::Fire, 0, 1),
            gun(Cue::Fire, 2, 4),
            gun(Cue::Impact, 0, 6),
            shot(Cue::Empty, 0, 0x183C, 0),
            of("wave", silent(Cue::Impact, 0)),
            of("cut", silent(Cue::Impact, 0)),
        ],
    ),
    // Suck Cannon (0x3619C8, both classes): the vortex's loop, and what it fires.
    (0x87, &[gun(Cue::Suck, 0, 0), gun(Cue::Fire, 0, 2)]),
    // RY3NO (0x3805B0, both classes): entry 1 at V1, 3 from V2 (by the row's level); V5
    // also plays class 0x1F4's entry 1 on the hero, which isn't read. Its missile bursts
    // at V1 (0x383370); V2's (0x1DCE) has no table; V5's big blast plays the bolt crate's
    // entry 2 (0x3DD558).
    (
        0x97,
        &[
            gun(Cue::Fire, 0, 1),
            gun(Cue::Fire, 1, 3),
            gun(Cue::Empty, 0, 2),
            of("rocket", shot(Cue::Impact, 0, 0x1873, 0)),
            of("rocket", silent(Cue::Impact, 1)),
            of("rocket", shot(Cue::Impact, 4, 0x1F4, 2)),
        ],
    ),
    // Plasma Coil (0x315638; V5's 0x306F68); its ball going (0x30BF18), and each arc
    // reaching (V5's storm, 0x30A3D0).
    (
        0x10,
        &[
            gun(Cue::Fire, 0, 2),
            gun(Cue::Empty, 0, 7),
            gun(Cue::Empty, 4, 6),
            of("ball", shot(Cue::Impact, 0, 0xA9B, 2)),
            of("storm", shot(Cue::Arc, 4, 0x9A9, 1)),
        ],
    ),
    // Bouncer (0x31C890, both classes); the canister's burst (0x31D6C0) and each
    // bomblet's (0x31E488).
    (
        0x13,
        &[
            gun(Cue::Fire, 0, 0),
            gun(Cue::Empty, 0, 1),
            of("canister", shot(Cue::Impact, 0, 0xE64, 0)),
            of("bomblet", shot(Cue::Impact, 0, 0xECE, 1)),
        ],
    ),
    // Miniturret Glove (0x346078; V5's 0x343088 the same): 0 as a turret is thrown. The
    // turret's shots (0x32BF28) and its end's burst (0x32AFB0), V5's of its own class;
    // the Megaturret's missiles burst as the small missile does.
    (
        0x15,
        &[
            gun(Cue::Fire, 0, 0),
            gun(Cue::Empty, 0, 2),
            of("turret", shot(Cue::Fire, 0, 0x1366, 0)),
            of("turret", shot(Cue::Fire, 4, 0x1367, 0)),
            of("turret", shot(Cue::Impact, 0, 0x1366, 3)),
            of("turret", shot(Cue::Impact, 4, 0x1367, 3)),
            of("guided", shot(Cue::Impact, 4, 0xA82, 0)),
        ],
    ),
    // Qwack-O-Ray (0x37DEB0, both classes): its beam's loop while on; the ducks ending in
    // feathers (0x3D2870; V4's 0x39E8A0, V5's 0x3D9008) and their eggs popping
    // (0x3D48B8).
    (
        0x8F,
        &[
            gun(Cue::Beam, 0, 0),
            of("duck", shot(Cue::Impact, 0, 0x1BC3, 1)),
            of("duck", shot(Cue::Impact, 3, 0x189B, 1)),
            of("duck", shot(Cue::Impact, 4, 0x1D3E, 3)),
            of("egg", shot(Cue::Impact, 0, 0x1C59, 1)),
        ],
    ),
    // Shield Charger (0x340DD0): raising the shield (class 0xA5C, V5's 0x1192), and
    // clicking empty on 0xA5C's; its loop and end are [`HERO_LOOPS`]' and
    // [`shield_end`]'s.
    (
        0x16,
        &[
            shot(Cue::Fire, 0, 0xA5C, 1),
            shot(Cue::Fire, 4, 0x1192, 1),
            shot(Cue::Empty, 0, 0xA5C, 0),
        ],
    ),
];

/// The entry `item`'s gun plays for `cue` at `level`, for an event of `shot`'s kind.
pub fn cue_sound(item: u16, cue: Cue, level: u8, shot: Option<&str>) -> Option<CueSound> {
    let (_, cues) = UYA_GUN_SOUNDS.iter().find(|(gun, _)| *gun == item)?;
    let pick = |kind: Option<&str>| {
        cues.iter()
            .filter(|sound| sound.cue == cue && sound.level <= level && sound.shot == kind)
            .max_by_key(|sound| sound.level)
            .copied()
    };
    shot.and_then(|kind| pick(Some(kind)))
        .or_else(|| pick(None))
}

/// The class `item`'s gun is at `level`.
fn gun_class(item: u16, level: u8) -> Option<u32> {
    UYA_GUN_MODELS
        .iter()
        .filter(|model| model.item == item && model.level <= level)
        .max_by_key(|model| model.level)
        .map(|model| model.class)
}

/// Where UYA plays a pickup sound from (`weapons.md`, "Their sounds") [code]: an entry of a
/// class's table, at least `gap` 60 Hz frames after the last of its kind (0 for any).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickupEntry {
    pub class: u32,
    pub entry: usize,
    pub gap: u8,
}

/// The coins' classes, each from the least value it is for (`bolt_coin_spawn`, 0x2F8F60).
/// Veldin has none of the coins of 500 and up (0x12 and on), so they sound as 0x11's
/// [placeholder].
const COINS: [(u32, u32); 4] = [(0, 0xD), (20, 0xF), (50, 0x10), (100, 0x11)];

/// Where `kind`'s sound is, for a coin of `bolts` ([`PickupSound::bolts`]).
pub fn pickup_entry(kind: PickupSoundKind, bolts: u32) -> PickupEntry {
    let crate_class = |kind| match kind {
        CrateKind::Bolts => 0x1F4,
        CrateKind::Nanotech => 0x1F5,
        CrateKind::Doubler => 0x1F7,
        CrateKind::Explosive => 0x1F9,
        CrateKind::Ammo => 0x1FF,
    };
    let (class, entry, gap) = match kind {
        // `crate_break` 0x33BE30: none within a frame of the last crate's (0x1E1914).
        PickupSoundKind::CrateBroke(CrateKind::Ammo) => (0x1FF, 1, 2),
        PickupSoundKind::CrateBroke(kind) => (crate_class(kind), 0, 2),
        PickupSoundKind::FuseBeep => (crate_class(CrateKind::Explosive), 1, 0),
        PickupSoundKind::DoublerBeep => (crate_class(CrateKind::Doubler), 1, 0),
        PickupSoundKind::Bolt => {
            let coin = COINS.iter().rev().find(|(least, _)| bolts >= *least);
            (coin.map_or(COINS[0].1, |coin| coin.1), 0, 0)
        }
        // `ammo_pickup_update` 0x30E7F8: none within 11 frames of the last (0x1E17A8).
        PickupSoundKind::Ammo => (0xCDB, 0, 11),
        PickupSoundKind::Nanotech => (crate_class(CrateKind::Nanotech), 1, 0),
        // The vendor's class (0x4B7948 to 0x4BA3E0).
        PickupSoundKind::Vendor(said) => {
            let entry = match said {
                VendorSound::Back | VendorSound::Short => 0,
                VendorSound::Moved | VendorSound::Counted => 1,
                VendorSound::Refused => 2,
                VendorSound::Opened => 3,
                VendorSound::Closed => 5,
                VendorSound::Bought => 7,
            };
            (0xB, entry, 0)
        }
    };
    PickupEntry { class, entry, gap }
}

/// Every class a pickup sound plays from.
fn pickup_classes() -> impl Iterator<Item = u32> {
    PickupSoundKind::ALL
        .into_iter()
        .map(|kind| pickup_entry(kind, 0).class)
        .chain(COINS.map(|(_, class)| class))
}

/// What a pickup sound's gap is timed from: UYA times every crate's from the last crate's.
fn gap_key(kind: PickupSoundKind) -> PickupSoundKind {
    match kind {
        PickupSoundKind::CrateBroke(_) => PickupSoundKind::CrateBroke(CrateKind::Bolts),
        kind => kind,
    }
}

/// A class's sound table, and the bank its entries' sounds are in.
#[derive(Debug, Clone, PartialEq)]
struct Table {
    bank: BankId,
    sounds: Vec<ClassSound>,
}

/// The guns' sounds read from the disc: the synthesizer with their banks, and the tables.
pub struct DiscGunSounds {
    synth: Synth,
    tables: BTreeMap<u32, Table>,
}

/// Reads the sounds of every gun in [`UYA_GUN_SOUNDS`] at every level, and of their shots
/// and the pickups in `core`, level `level`.
pub fn import(
    disc: &mut impl DiscReader,
    core: &Core,
    level: usize,
) -> Result<DiscGunSounds, FormatError> {
    let gadgets = GadgetTable::read_uya(&Toc::read_uya_disc(disc)?)?;
    let mut synth = Synth::new();
    let mut tables = BTreeMap::new();
    let mut ours = None;
    for (item, cues) in UYA_GUN_SOUNDS {
        for sound in cues.iter() {
            match sound.entry {
                Entry::Gun(_) => {
                    let classes = UYA_GUN_MODELS
                        .iter()
                        .filter(|model| model.item == *item)
                        .map(|model| model.class);
                    for class in classes {
                        if tables.contains_key(&class) {
                            continue;
                        }
                        // The few the WAD lacks are the level's, as their models are.
                        let Some(index) = gadgets.find(class) else {
                            let bank = level_bank(&mut synth, &mut ours, disc, level)?;
                            let sounds = core.moby_sounds(class)?;
                            tables.insert(class, Table { bank, sounds });
                            continue;
                        };
                        let Some(bank) = gadgets.read_sounds(disc, index)? else {
                            continue;
                        };
                        let sounds = Gadget::read_from(disc, &gadgets, index)?.sounds;
                        let bank = synth.add_bank(bank);
                        tables.insert(class, Table { bank, sounds });
                    }
                }
                Entry::Shot { class, .. } => {
                    shot_class(&mut synth, &mut tables, disc, core, level, &mut ours, class)?;
                }
                Entry::Silent => {}
            }
        }
    }
    for class in UNCUED_CLASSES {
        shot_class(&mut synth, &mut tables, disc, core, level, &mut ours, class)?;
    }
    level_classes(&mut synth, &mut tables, disc, core, level, &mut ours)?;
    // Not every level has every crate: the rest from one that has them, if needed.
    if pickup_classes().any(|class| !tables.contains_key(&class)) {
        let other = Core::read_uya(disc, CRATES_LEVEL)?;
        let mut theirs = None;
        level_classes(
            &mut synth,
            &mut tables,
            disc,
            &other,
            CRATES_LEVEL,
            &mut theirs,
        )?;
    }
    Ok(DiscGunSounds { synth, tables })
}

/// Adds the table of shot class `class` if `core`, level `level`, has it and `tables`
/// lacks it, with the level's bank (`bank`, added the first time it's needed). A level
/// lacking it, as Veldin lacks some V5 shots, leaves them silent.
fn shot_class(
    synth: &mut Synth,
    tables: &mut BTreeMap<u32, Table>,
    disc: &mut impl DiscReader,
    core: &Core,
    level: usize,
    bank: &mut Option<BankId>,
    class: u32,
) -> Result<(), FormatError> {
    let here = core.header.moby_class(class).is_some_and(|c| c.offset != 0);
    if !here || tables.contains_key(&class) {
        return Ok(());
    }
    let bank = level_bank(synth, bank, disc, level)?;
    let sounds = core.moby_sounds(class)?;
    tables.insert(class, Table { bank, sounds });
    Ok(())
}

/// A level whose core has every crate's class: Veldin has no explosive or doubler crate.
const CRATES_LEVEL: usize = 4;

/// Adds the tables of the pickups' classes that `core`, level `level`, has and `tables`
/// lacks, with the level's bank (`bank`, added the first time it's needed).
fn level_classes(
    synth: &mut Synth,
    tables: &mut BTreeMap<u32, Table>,
    disc: &mut impl DiscReader,
    core: &Core,
    level: usize,
    bank: &mut Option<BankId>,
) -> Result<(), FormatError> {
    for class in pickup_classes() {
        let here = core.header.moby_class(class).is_some_and(|c| c.offset != 0);
        if !here || tables.contains_key(&class) {
            continue;
        }
        let bank = level_bank(synth, bank, disc, level)?;
        let sounds = core.moby_sounds(class)?;
        tables.insert(class, Table { bank, sounds });
    }
    Ok(())
}

/// The level's bank, added to `synth` the first time it is asked for.
fn level_bank(
    synth: &mut Synth,
    added: &mut Option<BankId>,
    disc: &mut impl DiscReader,
    level: usize,
) -> Result<BankId, FormatError> {
    if let Some(bank) = *added {
        return Ok(bank);
    }
    let bank = synth.add_bank(read_uya_sound_bank(disc, level)?);
    *added = Some(bank);
    Ok(bank)
}

/// The table entry `item`'s gun plays for `cue` at `level` for an event of `shot`'s kind,
/// and the bank it's in; `None` for a silent one.
fn lookup(
    tables: &BTreeMap<u32, Table>,
    item: u16,
    cue: Cue,
    level: u8,
    shot: Option<&str>,
) -> Option<(CueSound, BankId, ClassSound)> {
    let sound = cue_sound(item, cue, level, shot)?;
    let (bank, entry) = resolve(tables, item, level, sound.entry)?;
    Some((sound, bank, entry))
}

/// What `entry` is for `item`'s gun at `level`, and the bank it's in; `None` for a silent
/// one or one the disc lacks.
fn resolve(
    tables: &BTreeMap<u32, Table>,
    item: u16,
    level: u8,
    entry: Entry,
) -> Option<(BankId, ClassSound)> {
    let (class, entry) = match entry {
        Entry::Gun(entry) => (gun_class(item, level)?, entry),
        Entry::Shot { class, entry } => (class, entry),
        Entry::Silent => return None,
    };
    let table = tables.get(&class)?;
    Some((table.bank, *table.sounds.get(entry)?))
}

/// The table entry `kind` plays for a coin of `bolts`, and the bank it's in, if it has a
/// sound that doesn't loop.
fn lookup_pickup(
    tables: &BTreeMap<u32, Table>,
    kind: PickupSoundKind,
    bolts: u32,
) -> Option<(PickupEntry, BankId, ClassSound)> {
    let place = pickup_entry(kind, bolts);
    let table = tables.get(&place.class)?;
    let entry = *table.sounds.get(place.entry)?;
    (entry.sound.is_some() && !entry.looped).then_some((place, table.bank, entry))
}

/// The synthesizer, which the stream renders and the systems play on.
struct DiscSynth(Synth);

impl Render for DiscSynth {
    fn render(&mut self, out: &mut [f32]) {
        self.0.render(out);
    }
}

/// The disc's guns' sounds, playing.
#[derive(Resource)]
struct GunSounds {
    synth: Arc<Mutex<DiscSynth>>,
    tables: BTreeMap<u32, Table>,
    /// Each hero's held loop: the gun's item, its level, the cue, and the sound.
    held: BTreeMap<usize, (u16, u8, Cue, SoundHandle, ClassSound)>,
    /// When each hero's gun may next play a cue with a gap, in seconds.
    next: BTreeMap<(usize, u16, Cue), f64>,
    /// When each pickup sound with a gap may next play, by [`gap_key`].
    next_pickup: BTreeMap<PickupSoundKind, f64>,
    /// The loops no cue holds ([`play_shots`]), by hero: what plays, and its sound.
    kept: BTreeMap<(usize, LoopKey), (Entry, SoundHandle, ClassSound)>,
    /// Each hero's shots as last seen, by serial.
    seen: BTreeMap<(usize, u32), Seen>,
    /// Each hero's Shield Charger shield was up last frame.
    shields: BTreeMap<usize, bool>,
    /// Sounds to play later: when, whose gun at which level, what, and where from.
    later: Vec<(f64, u16, u8, Entry, Vec3)>,
    /// For the pitches picked between an entry's limits.
    rng: u32,
}

impl GunSounds {
    /// Plays `entry` once, heard from `at`, or at full volume if `None`.
    fn start(&mut self, listeners: &Listeners, bank: BankId, entry: ClassSound, at: Option<Vec3>) {
        let Some(number) = entry.sound else {
            return;
        };
        let heard = match at {
            Some(at) => volume_pan(listeners, &entry, at),
            None => Some((entry.max_volume.min(0x400), 0)),
        };
        let Some((volume, pan)) = heard else {
            return;
        };
        let pitch_mod = self.pitch(&entry);
        let play = Play {
            volume,
            pan: Some(pan),
            pitch_mod,
            pitch_bend: 0,
        };
        self.synth
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .0
            .play(bank, usize::from(number), play);
    }

    fn pitch(&mut self, entry: &ClassSound) -> i32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        let span = (entry.pitch_high - entry.pitch_low).max(0) as u32 + 1;
        entry.pitch_low + (self.rng % span) as i32
    }
}

/// The volume (out of 0x400) and pan (degrees) the game gives `entry` heard from `at`;
/// `None` if too quiet to start.
fn volume_pan(listeners: &Listeners, entry: &ClassSound, at: Vec3) -> Option<(i32, i32)> {
    let heard = listeners.hear(at)?;
    let volume = entry.volume_at(heard.distance).min(0x400);
    (volume >= 0x20).then_some((volume, heard.pan as i32))
}

/// Plays the disc's guns' sounds in place of the placeholders. After the kit's plugins,
/// whose `SfxPlugin` registers the stream with Bevy's audio.
pub fn add(app: &mut App, read: DiscGunSounds) {
    app.insert_resource(GunSounds {
        synth: Arc::new(Mutex::new(DiscSynth(read.synth))),
        tables: read.tables,
        held: BTreeMap::new(),
        next: BTreeMap::new(),
        next_pickup: BTreeMap::new(),
        kept: BTreeMap::new(),
        seen: BTreeMap::new(),
        shields: BTreeMap::new(),
        later: Vec::new(),
        rng: 0x9E37_79B9,
    })
    .add_systems(Startup, (claim_cues, claim_pickups, start_stream))
    .add_systems(
        Update,
        (play_cues, hold_cues, play_shots, play_pickups).in_set(SfxSystems::Play),
    );
}

/// Tells the placeholders which guns' cues the disc plays.
fn claim_cues(armory: Res<Armory>, sounds: Res<GunSounds>, mut claims: ResMut<SoundClaims>) {
    let mut claimed = std::collections::BTreeSet::new();
    for def in &armory.weapons {
        let Some((_, cues)) = UYA_GUN_SOUNDS.iter().find(|(item, _)| *item == def.item) else {
            continue;
        };
        for sound in cues.iter() {
            // Only if the disc has it, or it's silent.
            let found = lookup(&sounds.tables, def.item, sound.cue, sound.level, sound.shot);
            if sound.entry == Entry::Silent || found.is_some() {
                claims.claim(&def.effects, sound.cue);
                claimed.insert((def.item, sound.cue));
            }
        }
    }
    info!("the guns' sounds from the disc: {} cues", claimed.len());
}

fn start_stream(
    sounds: Res<GunSounds>,
    mut streams: ResMut<Assets<SoundStream>>,
    mut commands: Commands,
) {
    let stream = streams.add(SoundStream(sounds.synth.clone()));
    commands.spawn((
        Name::new("the disc's gun sounds"),
        AudioPlayer(stream),
        PlaybackSettings::ONCE,
    ));
}

fn play_cues(
    armory: Res<Armory>,
    listeners: Res<Listeners>,
    time: Res<Time<Real>>,
    heroes: Query<(&Hero, &Arsenal)>,
    mut sounds: ResMut<GunSounds>,
    mut events: rac_sim::ShownReader<GunEvent>,
) {
    let now = time.elapsed_secs_f64();
    for event in events.read() {
        let Some((cue, at)) = Cue::of_event(event.kind) else {
            continue;
        };
        if cue.loops() {
            continue;
        }
        let Some(def) = armory.get(event.weapon) else {
            continue;
        };
        let level = heroes
            .iter()
            .find(|(hero, _)| hero.player == event.player)
            .and_then(|(_, arsenal)| arsenal.gun(event.weapon))
            .map_or(0, |owned| owned.level);
        let Some((sound, bank, entry)) = lookup(&sounds.tables, def.item, cue, level, event.shot)
        else {
            continue;
        };
        if entry.sound.is_none() || entry.looped {
            continue;
        }
        let key = (event.player, def.item, cue);
        if sound.gap > 0 {
            if sounds.next.get(&key).is_some_and(|&next| now < next) {
                continue;
            }
            sounds.next.insert(key, now + f64::from(sound.gap) / 60.0);
        }
        sounds.start(&listeners, bank, entry, at);
    }
}

/// Tells the placeholders which pickup sounds the disc plays.
fn claim_pickups(sounds: Res<GunSounds>, mut claims: ResMut<SoundClaims>) {
    let mut claimed = 0;
    for kind in PickupSoundKind::ALL {
        if lookup_pickup(&sounds.tables, kind, 0).is_some() {
            claims.claim_pickup(kind);
            claimed += 1;
        }
    }
    info!(
        "the pickups' sounds from the disc: {claimed} of {}",
        PickupSoundKind::ALL.len()
    );
}

fn play_pickups(
    listeners: Res<Listeners>,
    time: Res<Time<Real>>,
    mut sounds: ResMut<GunSounds>,
    mut events: rac_sim::ShownReader<PickupSound>,
) {
    let now = time.elapsed_secs_f64();
    for event in events.read() {
        let Some((place, bank, entry)) = lookup_pickup(&sounds.tables, event.kind, event.bolts)
        else {
            continue;
        };
        if place.gap > 0 {
            let key = gap_key(event.kind);
            if sounds.next_pickup.get(&key).is_some_and(|&next| now < next) {
                continue;
            }
            sounds
                .next_pickup
                .insert(key, now + f64::from(place.gap) / 60.0);
        }
        sounds.start(&listeners, bank, entry, event.at);
    }
}

/// Starts, follows and stops each hero's held loop.
fn hold_cues(
    armory: Res<Armory>,
    listeners: Res<Listeners>,
    heroes: Query<(&Hero, &Arsenal, &GlobalTransform)>,
    mut sounds: ResMut<GunSounds>,
) {
    let mut wanted = BTreeMap::new();
    for held in held_cues(&heroes) {
        if let Some(def) = armory.get(held.weapon) {
            wanted.insert(held.player, (def.item, held.level, held.cue, held.at));
        }
    }
    let sounds = &mut *sounds;
    let mut synth = sounds.synth.lock().unwrap_or_else(PoisonError::into_inner);
    sounds.held.retain(|player, (item, level, cue, handle, _)| {
        let keep = wanted
            .get(player)
            .is_some_and(|want| (want.0, want.1, want.2) == (*item, *level, *cue))
            && synth.0.is_playing(*handle);
        if !keep {
            synth.0.stop(*handle);
        }
        keep
    });
    for (player, (item, level, cue, at)) in wanted {
        if let Some((.., handle, entry)) = sounds.held.get(&player) {
            // Out of earshot it keeps going, silent, as the game's looping sounds do.
            let (volume, pan) = volume_pan(&listeners, entry, at).unwrap_or((0, 0));
            synth.0.set_volume_pan(*handle, volume, Some(pan));
            continue;
        }
        let Some((_, bank, entry)) = lookup(&sounds.tables, item, cue, level, None) else {
            continue;
        };
        let Some(number) = entry.sound else {
            continue;
        };
        let Some((volume, pan)) = volume_pan(&listeners, &entry, at) else {
            continue;
        };
        let play = Play {
            volume,
            pan: Some(pan),
            pitch_mod: entry.pitch_low,
            pitch_bend: 0,
        };
        if let Some(handle) = synth.0.play(bank, usize::from(number), play) {
            sounds
                .held
                .insert(player, (item, level, cue, handle, entry));
        }
    }
}

/// Shot classes that [`shot_loop`], [`shot_once`] and [`play_shots`] play from, beyond the
/// cues'.
const UNCUED_CLASSES: [u32; 12] = [
    0x1ADA, 0x186B, 0x186D, 0x1CB8, 0x1867, 0xA9B, 0x1366, 0x1367, 0x1BC3, 0x189B, 0xA5C, 0x1192,
];

/// A holo shield stands unfolded this many ticks after it lands (its wall's unfold,
/// `effects.md`, "Holo Shield Glove"), and its hum starts then (0x376B88).
const HOLO_UNFOLD_TICKS: u32 = 34;
/// A holo shield's reaction to a hit lasts this many frames, ending with its entry 2.
const HOLO_REACT_TICKS: u32 = 30;

/// A loop no cue holds: one of a hero's shots', by its serial, or his own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LoopKey {
    Shot(u32),
    Whip,
    Shield,
}

/// What a shot was when last seen, for what it plays as it changes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Seen {
    landed: bool,
    hits_left: u32,
}

impl Seen {
    fn of(shot: &Shot) -> Seen {
        match &shot.kind {
            ShotKind::Turret(state) => Seen {
                landed: state.landed.is_some(),
                ..default()
            },
            ShotKind::Barrier(state) => Seen {
                landed: state.landed.is_some(),
                hits_left: state.hits_left,
            },
            _ => Seen::default(),
        }
    }
}

/// What one of `item`'s shots keeps playing as it is (`effects.md`, "The shots' entries")
/// [code]: an agent jetting in or hovering from V4, an open rift, a holo shield standing,
/// V5's cutting wave, a disc in flight and a coil ball.
fn shot_loop(item: u16, shot: &Shot) -> Option<Entry> {
    let class = |class, entry| Some(Entry::Shot { class, entry });
    match (item, &shot.kind) {
        (0x57, ShotKind::Agent(state)) if shot.level >= 3 && state.hover.is_some() => {
            class(0x1ADA, 9)
        }
        (0x5F, ShotKind::Rift(state)) if state.open.is_some() => class(0x186B, 0),
        (0x67, ShotKind::Barrier(state))
            if state.landed.is_some_and(|ticks| ticks >= HOLO_UNFOLD_TICKS) =>
        {
            class(0x186D, 0)
        }
        (0x7F, ShotKind::Cut { .. }) => class(0x1CB8, 0),
        (0x4F, ShotKind::Disc { .. }) => class(0x1867, 0),
        (0x10, ShotKind::Ball { .. }) => class(0xA9B, 0),
        _ => None,
    }
}

/// What one of `item`'s shots plays once as it comes (`seen` none) or changes from `seen`
/// (`effects.md`, "The shots' entries") [code]: an agent appearing, its bullets at V2–V4
/// and its missiles at V5; a target turning into a duck (the gun's entry 1) and a duck's
/// egg laid; a turret landing open; a hit on a holo shield.
fn shot_once(item: u16, shot: &Shot, seen: Option<Seen>) -> Option<Entry> {
    let class = |class, entry| Some(Entry::Shot { class, entry });
    let Some(seen) = seen else {
        return match (item, &shot.kind) {
            (0x57, ShotKind::Agent(_)) => class(0x1ADA, 3),
            (0x57, ShotKind::Slug) => class(0x1ADA, 7),
            (0x57, ShotKind::Guided { .. }) => class(0x1ADA, 8),
            (0x8F, ShotKind::Duck(_)) => Some(Entry::Gun(1)),
            (0x8F, ShotKind::Egg { .. }) if shot.level < 3 => class(0x1BC3, 0),
            (0x8F, ShotKind::Egg { .. }) if shot.level == 3 => class(0x189B, 0),
            _ => None,
        };
    };
    match (item, &shot.kind) {
        (0x15, ShotKind::Turret(state)) if state.landed.is_some() && !seen.landed => {
            class(if shot.level >= 4 { 0x1367 } else { 0x1366 }, 1)
        }
        (0x67, ShotKind::Barrier(state)) if state.hits_left < seen.hits_left => class(0x186D, 1),
        _ => None,
    }
}

/// The Shield Charger's class at `level`: 0xA5C, V5's 0x1192.
fn shield_class(level: u8) -> u32 {
    if level >= 4 { 0x1192 } else { 0xA5C }
}

/// Plays what the heroes' shots keep playing and play as they change, and the heroes' own
/// loops: the Plasma Whip out in his hand and shown (0x356110: entry 0, from V3 3) and
/// the Shield Charger's shield up (0x3410E8: its entry 3, then 2 as it goes). No gun
/// event marks them. Also what takes a target: a rift swallowing it (0x373990: its entry
/// 2) and the Suck Cannon taking it in (0x362908: the gun's entry 1, which the game plays
/// as it reaches the muzzle).
fn play_shots(
    armory: Res<Armory>,
    listeners: Res<Listeners>,
    time: Res<Time<Real>>,
    heroes: Query<(&Hero, &HeroBody, &Arsenal, &GlobalTransform)>,
    mut afflicted: rac_sim::ShownReader<Afflicted>,
    mut sounds: ResMut<GunSounds>,
) {
    let now = time.elapsed_secs_f64();
    let mut wanted = BTreeMap::new();
    let mut once = Vec::new();
    let mut seen = BTreeMap::new();
    let mut shields = BTreeMap::new();
    for (hero, body, arsenal, place) in &heroes {
        let player = hero.player;
        for shot in &arsenal.shots {
            let Some(def) = armory.get(shot.weapon) else {
                continue;
            };
            let id = (player, shot.serial);
            let before = sounds.seen.get(&id).copied();
            if let Some(entry) = shot_once(def.item, shot, before) {
                once.push((def.item, shot.level, entry, shot.position));
                if def.item == 0x67 && before.is_some() {
                    let ends = Entry::Shot {
                        class: 0x186D,
                        entry: 2,
                    };
                    let when = now + f64::from(HOLO_REACT_TICKS) / 60.0;
                    sounds
                        .later
                        .push((when, def.item, shot.level, ends, shot.position));
                }
            }
            seen.insert(id, Seen::of(shot));
            if let Some(entry) = shot_loop(def.item, shot) {
                let key = (player, LoopKey::Shot(shot.serial));
                wanted.insert(key, (def.item, shot.level, entry, shot.position));
            }
        }
        let at = place.translation();
        if let Some(weapon) = arsenal.hand.gun()
            && armory.get(weapon).is_some_and(|def| def.item == 0x7F)
            && body.0.gun.whip.is_some()
        {
            let level = arsenal.gun(weapon).map_or(0, |owned| owned.level);
            let entry = Entry::Gun(if level >= 2 { 3 } else { 0 });
            wanted.insert((player, LoopKey::Whip), (0x7F, level, entry, at));
        }
        let shield = arsenal
            .guns
            .iter()
            .find(|owned| armory.get(owned.weapon).is_some_and(|def| def.item == 0x16));
        if let Some(owned) = shield {
            let class = shield_class(owned.level);
            if arsenal.shield_up() {
                let entry = Entry::Shot { class, entry: 3 };
                wanted.insert((player, LoopKey::Shield), (0x16, owned.level, entry, at));
            } else if sounds.shields.get(&player) == Some(&true) {
                once.push((0x16, owned.level, Entry::Shot { class, entry: 2 }, at));
            }
        }
        shields.insert(player, arsenal.shield_up());
    }
    for event in afflicted.read() {
        let Some(def) = armory.get(event.weapon) else {
            continue;
        };
        let Some((_, _, arsenal, place)) =
            heroes.iter().find(|(hero, ..)| hero.player == event.player)
        else {
            continue;
        };
        let level = arsenal.gun(event.weapon).map_or(0, |owned| owned.level);
        match (def.item, event.affliction) {
            (0x5F, Affliction::Swallowed { into }) => {
                once.push((
                    0x5F,
                    level,
                    Entry::Shot {
                        class: 0x186B,
                        entry: 2,
                    },
                    into,
                ));
            }
            (0x87, Affliction::Sucked) => {
                once.push((0x87, level, Entry::Gun(1), place.translation()));
            }
            _ => {}
        }
    }
    let sounds = &mut *sounds;
    sounds.seen = seen;
    sounds.shields = shields;
    sounds.later.retain(|&(when, item, level, entry, at)| {
        let due = when <= now;
        if due {
            once.push((item, level, entry, at));
        }
        !due
    });
    for (item, level, entry, at) in once {
        if let Some((bank, entry)) = resolve(&sounds.tables, item, level, entry)
            && !entry.looped
        {
            sounds.start(&listeners, bank, entry, Some(at));
        }
    }
    let mut synth = sounds.synth.lock().unwrap_or_else(PoisonError::into_inner);
    sounds.kept.retain(|key, (entry, handle, _)| {
        let keep =
            wanted.get(key).is_some_and(|want| want.2 == *entry) && synth.0.is_playing(*handle);
        if !keep {
            synth.0.stop(*handle);
        }
        keep
    });
    for (key, (item, level, entry, at)) in wanted {
        if let Some((_, handle, sound)) = sounds.kept.get(&key) {
            // Out of earshot it keeps going, silent, as the game's looping sounds do.
            let (volume, pan) = volume_pan(&listeners, sound, at).unwrap_or((0, 0));
            synth.0.set_volume_pan(*handle, volume, Some(pan));
            continue;
        }
        let Some((bank, sound)) = resolve(&sounds.tables, item, level, entry) else {
            continue;
        };
        // Only a looped entry starts as a loop (`sound_start`).
        let (true, Some(number)) = (sound.looped, sound.sound) else {
            continue;
        };
        let Some((volume, pan)) = volume_pan(&listeners, &sound, at) else {
            continue;
        };
        let play = Play {
            volume,
            pan: Some(pan),
            pitch_mod: sound.pitch_low,
            pitch_bend: 0,
        };
        if let Some(handle) = synth.0.play(bank, usize::from(number), play) {
            sounds.kept.insert(key, (entry, handle, sound));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gun_plays_its_levels_cue_or_the_one_before() {
        assert_eq!(
            cue_sound(0x27, Cue::Empty, 2, None).unwrap().entry,
            Entry::Gun(3)
        );
        assert_eq!(
            cue_sound(0x27, Cue::Empty, 4, None).unwrap().entry,
            Entry::Gun(1)
        );
        assert_eq!(
            cue_sound(0x2F, Cue::Fire, 4, None).unwrap().entry,
            Entry::Gun(3)
        );
        assert_eq!(cue_sound(0x2F, Cue::Charge, 0, None), None);
        assert_eq!(
            cue_sound(0x3F, Cue::Impact, 4, Some("missile"))
                .unwrap()
                .entry,
            Entry::Shot {
                class: 0x1894,
                entry: 1
            }
        );
        assert_eq!(gun_class(0x27, 3), Some(0x1863));
        assert_eq!(gun_class(0x27, 4), Some(0x1882));
        assert_eq!(gun_class(0x6F, 4), Some(0xC04));
    }

    #[test]
    fn a_shots_own_cue_comes_before_its_guns_and_may_be_silent() {
        let entry = |item, cue, level, shot| cue_sound(item, cue, level, shot).map(|s| s.entry);
        // The whip's rope plays its entry 6 where it hits; its wave nothing.
        assert_eq!(entry(0x7F, Cue::Impact, 3, None), Some(Entry::Gun(6)));
        assert_eq!(
            entry(0x7F, Cue::Impact, 3, Some("wave")),
            Some(Entry::Silent)
        );
        assert_eq!(
            entry(0x7F, Cue::Impact, 4, Some("cut")),
            Some(Entry::Silent)
        );
        // A kind with no row of its own takes the gun's.
        assert_eq!(
            entry(0x7F, Cue::Impact, 0, Some("other")),
            Some(Entry::Gun(6))
        );
        // An agent's end bursts at V1 alone.
        let burst = Entry::Shot {
            class: 0x1ADA,
            entry: 0,
        };
        assert_eq!(entry(0x57, Cue::Impact, 0, Some("agent")), Some(burst));
        assert_eq!(
            entry(0x57, Cue::Impact, 1, Some("agent")),
            Some(Entry::Silent)
        );
        assert_eq!(entry(0x57, Cue::Impact, 1, None), None);
    }

    fn shot(level: u8, kind: ShotKind) -> Shot {
        Shot {
            serial: 1,
            weapon: rac_combat::WeaponId(0),
            level,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            ticks: 0,
            damage: 0.0,
            splash: 0.0,
            mods: default(),
            kind,
        }
    }

    #[test]
    fn shots_loop_and_play_as_they_change() {
        use rac_combat::weapons::{BarrierState, RiftState};
        let class = |class, entry| Some(Entry::Shot { class, entry });
        // A rift hums once open.
        let rift = |open| shot(0, ShotKind::Rift(RiftState { open, ..default() }));
        assert_eq!(shot_loop(0x5F, &rift(None)), None);
        assert_eq!(shot_loop(0x5F, &rift(Some(3))), class(0x186B, 0));
        // A holo shield hums once unfolded, and sounds each hit.
        let holo = |landed, hits_left| {
            let state = BarrierState {
                landed,
                hits_left,
                ..default()
            };
            shot(0, ShotKind::Barrier(state))
        };
        assert_eq!(shot_loop(0x67, &holo(Some(10), 5)), None);
        assert_eq!(shot_loop(0x67, &holo(Some(40), 5)), class(0x186D, 0));
        let before = Seen::of(&holo(Some(40), 5));
        assert_eq!(shot_once(0x67, &holo(Some(41), 5), Some(before)), None);
        assert_eq!(
            shot_once(0x67, &holo(Some(41), 4), Some(before)),
            class(0x186D, 1)
        );
        // An agent sounds as it appears, and only then.
        let agent = shot(0, ShotKind::Agent(default()));
        assert_eq!(shot_once(0x57, &agent, None), class(0x1ADA, 3));
        assert_eq!(shot_once(0x57, &agent, Some(Seen::of(&agent))), None);
    }

    #[test]
    fn every_gun_with_sounds_is_one_of_the_armorys() {
        let armory = Armory::uya();
        for (item, cues) in UYA_GUN_SOUNDS {
            assert!(
                armory.weapons.iter().any(|def| def.item == *item),
                "{item:#x}"
            );
            // The Shield Charger plays only its shield's class's.
            let own = cues
                .iter()
                .any(|sound| matches!(sound.entry, Entry::Gun(_)));
            assert!(!own || gun_class(*item, 0).is_some(), "{item:#x}");
        }
    }

    #[test]
    fn pickups_play_uyas_classes_entries() {
        let entry = |kind, bolts| {
            let place = pickup_entry(kind, bolts);
            (place.class, place.entry, place.gap)
        };
        let crate_broke = PickupSoundKind::CrateBroke;
        assert_eq!(entry(crate_broke(CrateKind::Bolts), 0), (0x1F4, 0, 2));
        assert_eq!(entry(crate_broke(CrateKind::Ammo), 0), (0x1FF, 1, 2));
        assert_eq!(entry(PickupSoundKind::FuseBeep, 0), (0x1F9, 1, 0));
        assert_eq!(entry(PickupSoundKind::Ammo, 0), (0xCDB, 0, 11));
        let vendor = PickupSoundKind::Vendor;
        assert_eq!(entry(vendor(VendorSound::Bought), 0), (0xB, 7, 0));
        assert_eq!(entry(vendor(VendorSound::Short), 0), (0xB, 0, 0));
        // Coins sound by their value.
        let coins = [1, 5, 19, 20, 49, 50, 100, 499, 500, 30000];
        let classes = coins.map(|bolts| entry(PickupSoundKind::Bolt, bolts).0);
        assert_eq!(
            classes,
            [0xD, 0xD, 0xD, 0xF, 0xF, 0x10, 0x11, 0x11, 0x11, 0x11]
        );
        // Every crate's gap is timed from the last crate's.
        assert_eq!(
            gap_key(crate_broke(CrateKind::Nanotech)),
            gap_key(crate_broke(CrateKind::Ammo))
        );
        assert_ne!(
            gap_key(PickupSoundKind::Ammo),
            gap_key(PickupSoundKind::Bolt)
        );
    }

    /// Reads the real disc: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
    #[test]
    #[ignore = "needs game discs"]
    fn every_pickup_sound_is_on_the_disc() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) = crate::discs::find(
            std::path::Path::new(&folder),
            crate::discs::Release::UyaNtsc,
        )
        .expect("a UYA NTSC-U disc");
        let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        let mut read = import(&mut disc, &core, crate::hero_anims::LEVEL).unwrap();
        let mut silent = Vec::new();
        for kind in PickupSoundKind::ALL {
            for (bolts, _) in COINS {
                let Some((_, bank, entry)) = lookup_pickup(&read.tables, kind, bolts) else {
                    silent.push(kind);
                    break;
                };
                let number = usize::from(entry.sound.unwrap());
                assert!(number < read.synth.sound_count(bank), "{kind:?}");
            }
        }
        assert_eq!(silent, []);
        // A bolt crate breaking (Veldin's bank) and an explosive one (level 4's), each
        // rendered for a fifth of a second, are heard.
        for crate_kind in [CrateKind::Bolts, CrateKind::Explosive] {
            let kind = PickupSoundKind::CrateBroke(crate_kind);
            let (_, bank, entry) = lookup_pickup(&read.tables, kind, 0).unwrap();
            read.synth
                .play(bank, usize::from(entry.sound.unwrap()), Play::default())
                .unwrap();
            let mut out = vec![0.0; 2 * 9600];
            read.synth.render(&mut out);
            assert!(
                out.iter().any(|sample| sample.abs() > 0.1),
                "{crate_kind:?}"
            );
        }
    }

    /// Reads the real disc: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
    #[test]
    #[ignore = "needs game discs"]
    fn every_cue_of_uyas_guns_plays_a_sound_from_the_disc() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) = crate::discs::find(
            std::path::Path::new(&folder),
            crate::discs::Release::UyaNtsc,
        )
        .expect("a UYA NTSC-U disc");
        let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        let mut read = import(&mut disc, &core, crate::hero_anims::LEVEL).unwrap();
        let mut silent = Vec::new();
        for (item, cues) in UYA_GUN_SOUNDS {
            for sound in cues.iter().filter(|sound| sound.entry != Entry::Silent) {
                for level in sound.level..5 {
                    // Silenced from a later level.
                    let picked = cue_sound(*item, sound.cue, level, sound.shot);
                    if picked.is_some_and(|picked| picked.entry == Entry::Silent) {
                        continue;
                    }
                    let found = lookup(&read.tables, *item, sound.cue, level, sound.shot);
                    let Some((_, bank, entry)) = found else {
                        silent.push((*item, level, sound.shot));
                        continue;
                    };
                    assert_eq!(entry.looped, sound.cue.loops(), "{item:#x} {:?}", sound.cue);
                    let number = usize::from(entry.sound.unwrap());
                    assert!(number < read.synth.sound_count(bank), "{item:#x}");
                }
            }
        }
        silent.dedup();
        // The Holo Shield's V5 glove, which is Veldin's class, has no sounds there.
        assert_eq!(silent, [(0x67, 4, None)]);
        // What no cue plays: the loops loop, and the rest don't.
        let looped = |class, entry| {
            let (_, sound) = resolve(&read.tables, 0, 0, Entry::Shot { class, entry })
                .unwrap_or_else(|| panic!("{class:#x} {entry}"));
            sound.looped
        };
        for (class, entry) in [
            (0x1ADA, 9),
            (0x186B, 0),
            (0x186D, 0),
            (0x1CB8, 0),
            (0x1867, 0),
            (0xA9B, 0),
            (0xA5C, 3),
            (0x1192, 3),
        ] {
            assert!(looped(class, entry), "{class:#x} {entry}");
        }
        for (class, entry) in [
            (0x1ADA, 3),
            (0x1ADA, 7),
            (0x1ADA, 8),
            (0x1BC3, 0),
            (0x189B, 0),
            (0x1366, 1),
            (0x1367, 1),
            (0x186D, 1),
            (0x186D, 2),
            (0xA5C, 2),
            (0x1192, 2),
        ] {
            assert!(!looped(class, entry), "{class:#x} {entry}");
        }
        for (level, entry) in [(0, 0), (2, 3)] {
            let (_, idle) =
                resolve(&read.tables, 0x7F, level, Entry::Gun(entry)).expect("the whip's idle");
            assert!(idle.looped);
        }
        // The N60's shot, rendered for a fifth of a second, is heard.
        let (_, bank, entry) = lookup(&read.tables, 0x2F, Cue::Fire, 0, None).unwrap();
        read.synth
            .play(bank, usize::from(entry.sound.unwrap()), Play::default())
            .unwrap();
        let mut out = vec![0.0; 2 * 9600];
        read.synth.render(&mut out);
        assert!(out.iter().any(|sample| sample.abs() > 0.1));
    }

    /// The guns' sounds from R&C3 PAL are UYA NTSC-U's: the same tables, and the same banks
    /// in the gadget WAD and the levels they're read from.
    #[test]
    #[ignore = "needs game discs"]
    fn rac3_pals_gun_sounds_are_uya_ntscs() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let read = |release| {
            let (_, mut disc) =
                crate::discs::find(std::path::Path::new(&folder), release).expect("the disc");
            let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
            let tables = import(&mut disc, &core, crate::hero_anims::LEVEL)
                .unwrap()
                .tables;
            let gadgets = GadgetTable::read_uya(&Toc::read_uya_disc(&mut disc).unwrap()).unwrap();
            let banks: Vec<_> = (0..gadgets.entries.len())
                .map(|index| gadgets.read_sounds(&mut disc, index).unwrap())
                .collect();
            let levels: Vec<_> = [crate::hero_anims::LEVEL, CRATES_LEVEL]
                .map(|level| read_uya_sound_bank(&mut disc, level).unwrap())
                .into();
            (tables, banks, levels)
        };
        let ntsc = read(crate::discs::Release::UyaNtsc);
        let pal = read(crate::discs::Release::Rac3Pal);
        assert!(!ntsc.0.is_empty() && ntsc.1.iter().any(Option::is_some));
        assert!(ntsc.0 == pal.0, "the tables differ");
        for (index, (ntsc, pal)) in ntsc.1.iter().zip(&pal.1).enumerate() {
            assert!(ntsc == pal, "gadget {index}'s bank differs");
        }
        assert!(ntsc.2 == pal.2, "the levels' banks differ");
    }
}

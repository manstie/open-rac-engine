//! The guns' sounds from the user's UYA disc (PLAN.md P1.13, P3.13), played in place of
//! the kit's placeholders (`rac_demo::sfx`) by `rac_audio`'s port of the game's sound
//! library. Nothing is stored: each gun's class sound table and its bank are read from the
//! gadget WAD at start-up, and its shots' from Veldin's core and sound bank
//! (`docs/research/effects.md`, "Sounds"). The crates', pickups' and vendor's sounds
//! ([`PickupSound`], P1.10) are from Veldin's core too, those it has ([`pickup_entry`]).
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
use rac_combat::{Armory, Arsenal, GunEvent};
use rac_demo::sfx::{Cue, Listeners, Render, SfxSystems, SoundClaims, SoundStream, held_cues};
use rac_formats::gadget::{Gadget, GadgetTable};
use rac_formats::level::{Core, Toc, read_uya_sound_bank};
use rac_formats::sound::ClassSound;
use rac_formats::{DiscReader, FormatError};
use rac_hero::Hero;

use crate::gun_models::UYA_GUN_MODELS;

/// Where a cue's sound is in the game's tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// An entry of the table of the gun's class at its level.
    Gun(usize),
    /// An entry of a shot's class in the level's core.
    Shot { class: u32, entry: usize },
}

/// One of a gun's cues: from which level (0 for V1) it plays which entry, and at least how
/// many 60 Hz frames apart (0 for any).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CueSound {
    pub cue: Cue,
    pub level: u8,
    pub entry: Entry,
    pub gap: u8,
}

const fn gun(cue: Cue, level: u8, entry: usize) -> CueSound {
    CueSound {
        cue,
        level,
        entry: Entry::Gun(entry),
        gap: 0,
    }
}

const fn shot(cue: Cue, level: u8, class: u32, entry: usize) -> CueSound {
    CueSound {
        cue,
        level,
        entry: Entry::Shot { class, entry },
        gap: 0,
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
    // Infector (0x3688E8; V5's 0x389F58).
    (
        0x37,
        &[
            gun(Cue::Fire, 0, 0),
            gun(Cue::Empty, 0, 2),
            gun(Cue::Empty, 4, 1),
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
    // Agents of Doom (0x35C9F0): the glove only clicks, empty or with eight agents out.
    (0x57, &[gun(Cue::Empty, 0, 0)]),
    // Rift Inducer (0x372990, both classes).
    (0x5F, &[gun(Cue::Fire, 0, 1), gun(Cue::Empty, 0, 5)]),
    // Holo Shield Glove (0x3717A8, both classes; 3 when it has none to throw).
    (0x67, &[gun(Cue::Fire, 0, 2), gun(Cue::Empty, 0, 1)]),
    // Plasma Whip (0x356110): a lash plays entry 1 at V1 and V2 and 4 from V3. Its idle
    // loop (0, or 3 from V3) isn't cued; its "no ammo" plays on another moby.
    (0x7F, &[gun(Cue::Fire, 0, 1), gun(Cue::Fire, 2, 4)]),
    // Suck Cannon (0x3619C8, both classes): the vortex's loop, and what it fires.
    (0x87, &[gun(Cue::Suck, 0, 0), gun(Cue::Fire, 0, 2)]),
    // RY3NO (0x3805B0, both classes): entry 1 at V1, 3 from V2 (by the row's level); V5
    // also plays class 0x1F4's entry 1 on the hero, which isn't read.
    (
        0x97,
        &[
            gun(Cue::Fire, 0, 1),
            gun(Cue::Fire, 1, 3),
            gun(Cue::Empty, 0, 2),
        ],
    ),
    // Plasma Coil (0x315638; V5's 0x306F68).
    (
        0x10,
        &[
            gun(Cue::Fire, 0, 2),
            gun(Cue::Empty, 0, 7),
            gun(Cue::Empty, 4, 6),
        ],
    ),
    // Bouncer (0x31C890, both classes).
    (0x13, &[gun(Cue::Fire, 0, 0), gun(Cue::Empty, 0, 1)]),
    // Miniturret Glove (0x346078; V5's 0x343088 the same): 0 as a turret is thrown.
    (0x15, &[gun(Cue::Fire, 0, 0), gun(Cue::Empty, 0, 2)]),
];

/// The entry `item`'s gun plays for `cue` at `level`.
pub fn cue_sound(item: u16, cue: Cue, level: u8) -> Option<CueSound> {
    let (_, cues) = UYA_GUN_SOUNDS.iter().find(|(gun, _)| *gun == item)?;
    cues.iter()
        .filter(|sound| sound.cue == cue && sound.level <= level)
        .max_by_key(|sound| sound.level)
        .copied()
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
#[derive(Debug, Clone)]
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
                    if tables.contains_key(&class) {
                        continue;
                    }
                    let bank = level_bank(&mut synth, &mut ours, disc, level)?;
                    let sounds = core.moby_sounds(class)?;
                    tables.insert(class, Table { bank, sounds });
                }
            }
        }
    }
    for class in pickup_classes() {
        // Not every level has every crate.
        let here = core.header.moby_class(class).is_some_and(|c| c.offset != 0);
        if !here || tables.contains_key(&class) {
            continue;
        }
        let bank = level_bank(&mut synth, &mut ours, disc, level)?;
        let sounds = core.moby_sounds(class)?;
        tables.insert(class, Table { bank, sounds });
    }
    Ok(DiscGunSounds { synth, tables })
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

/// The table entry `item`'s gun plays for `cue` at `level`, and the bank it's in.
fn lookup(
    tables: &BTreeMap<u32, Table>,
    item: u16,
    cue: Cue,
    level: u8,
) -> Option<(CueSound, BankId, ClassSound)> {
    let sound = cue_sound(item, cue, level)?;
    let (class, entry) = match sound.entry {
        Entry::Gun(entry) => (gun_class(item, level)?, entry),
        Entry::Shot { class, entry } => (class, entry),
    };
    let table = tables.get(&class)?;
    Some((sound, table.bank, *table.sounds.get(entry)?))
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
    /// For the pitches picked between an entry's limits.
    rng: u32,
}

impl GunSounds {
    fn find(&self, item: u16, cue: Cue, level: u8) -> Option<(CueSound, BankId, ClassSound)> {
        lookup(&self.tables, item, cue, level)
    }

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
        rng: 0x9E37_79B9,
    })
    .add_systems(Startup, (claim_cues, claim_pickups, start_stream))
    .add_systems(
        Update,
        (play_cues, hold_cues, play_pickups).in_set(SfxSystems::Play),
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
            // Only if the disc has it.
            if sounds.find(def.item, sound.cue, sound.level).is_some() {
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
    mut events: MessageReader<GunEvent>,
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
        let Some((sound, bank, entry)) = sounds.find(def.item, cue, level) else {
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
    mut events: MessageReader<PickupSound>,
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
        let Some((_, bank, entry)) = lookup(&sounds.tables, item, cue, level) else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gun_plays_its_levels_cue_or_the_one_before() {
        assert_eq!(cue_sound(0x27, Cue::Empty, 2).unwrap().entry, Entry::Gun(3));
        assert_eq!(cue_sound(0x27, Cue::Empty, 4).unwrap().entry, Entry::Gun(1));
        assert_eq!(cue_sound(0x2F, Cue::Fire, 4).unwrap().entry, Entry::Gun(3));
        assert_eq!(cue_sound(0x2F, Cue::Charge, 0), None);
        assert_eq!(
            cue_sound(0x3F, Cue::Impact, 4).unwrap().entry,
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
    fn every_gun_with_sounds_is_one_of_the_armorys() {
        let armory = Armory::uya();
        for (item, _) in UYA_GUN_SOUNDS {
            assert!(
                armory.weapons.iter().any(|def| def.item == *item),
                "{item:#x}"
            );
            assert!(gun_class(*item, 0).is_some(), "{item:#x}");
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
    fn veldin_has_the_pickups_sounds_but_the_explosive_and_doubler_crates() {
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
        assert_eq!(
            silent,
            [
                PickupSoundKind::CrateBroke(CrateKind::Explosive),
                PickupSoundKind::CrateBroke(CrateKind::Doubler),
                PickupSoundKind::FuseBeep,
                PickupSoundKind::DoublerBeep,
            ]
        );
        // A bolt crate breaking, rendered for a fifth of a second, is heard.
        let kind = PickupSoundKind::CrateBroke(CrateKind::Bolts);
        let (_, bank, entry) = lookup_pickup(&read.tables, kind, 0).unwrap();
        read.synth
            .play(bank, usize::from(entry.sound.unwrap()), Play::default())
            .unwrap();
        let mut out = vec![0.0; 2 * 9600];
        read.synth.render(&mut out);
        assert!(out.iter().any(|sample| sample.abs() > 0.1));
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
            for sound in cues.iter() {
                for level in 0..5 {
                    let Some((_, bank, entry)) = lookup(&read.tables, *item, sound.cue, level)
                    else {
                        silent.push((*item, level));
                        continue;
                    };
                    assert_eq!(entry.looped, sound.cue.loops(), "{item:#x} {:?}", sound.cue);
                    let number = usize::from(entry.sound.unwrap());
                    assert!(number < read.synth.sound_count(bank), "{item:#x}");
                }
            }
        }
        // The Holo Shield's V5 glove, which is Veldin's class, has no sounds there.
        silent.dedup();
        assert_eq!(silent, [(0x67, 4)]);
        // The N60's shot, rendered for a fifth of a second, is heard.
        let (_, bank, entry) = lookup(&read.tables, 0x2F, Cue::Fire, 0).unwrap();
        read.synth
            .play(bank, usize::from(entry.sound.unwrap()), Play::default())
            .unwrap();
        let mut out = vec![0.0; 2 * 9600];
        read.synth.render(&mut out);
        assert!(out.iter().any(|sample| sample.abs() > 0.1));
    }
}

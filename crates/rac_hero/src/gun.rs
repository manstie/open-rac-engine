//! The hero's side of his guns (`docs/research/weapons.md`, "The hero's side of firing"):
//! whether one is in his hand, and the firing flags his moves read.
//!
//! The guns themselves belong to the game's weapons code (the kit's is `rac_combat`),
//! which fills in [`HeroMotion::gun`](crate::HeroMotion::gun) each tick before the hero
//! moves, as UYA's gun mobys update before the hero. A hero without guns keeps the
//! default: the wrench in hand, never firing. A wrench attack puts the gun away.

use serde::{Deserialize, Serialize};

/// What the hero's gun tells his moves.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GunHand {
    /// A gun is in his hand rather than the wrench (UYA's held item, 0x1A5E08, isn't 9).
    /// In the lock-strafe and first-person cameras it makes him strafe.
    pub out: bool,
    /// The gun fired and its firing animation still plays (`firing`, 0x1A71C9): no double
    /// jump meanwhile.
    pub firing: bool,
    /// The gun is one fired by holding the button (its row's +0x30), and is firing
    /// (`firing_held`, 0x1A71CB): the walk keeps its setting-off turn and doesn't speed up
    /// while the stick is well round from his facing.
    pub firing_held: bool,
    /// While it fires the walk's setting-off turn is the profile's steady one
    /// (`gun.steady_part`, `gun.steady_full`): the Shock Blaster, N60 Storm, Plasma Coil
    /// and Lava Gun.
    pub steady: bool,
    /// The gun in his hand, by the game's item number, for the rig to show it. `None`
    /// while the wrench is in his hand, and while a gun is still coming out.
    pub item: Option<u16>,
    /// Its level, 0 for V1: UYA's guns change their model at V5.
    pub level: u8,
    /// How he holds it and what he plays as it fires.
    pub pose: GunPose,
    /// Shots so far, counting round: each restarts the firing animation, as each of the
    /// game's guns calls `hero_fire_anim` (0x4E6A08) as it fires.
    pub shots: u8,
    /// The quick-select ring is open and holds his input: he takes none, as UYA blocks
    /// the pad's gameplay input some ticks after its ring opens over the running world
    /// (the kit plays the ring as UYA does with QUICK SELECT PAUSE off, so co-op peers
    /// stay in step).
    pub selecting: bool,
    /// The gun in hand is the Plasma Whip.
    pub whip: Option<WhipHand>,
}

/// The Plasma Whip in his hand (`weapons.md`, "Plasma Whip"): the fire button starts the
/// wrench's moves in their windows, each taking a round, and the whip's rope hits along
/// its length instead of the wrench.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct WhipHand {
    /// It has a round for an attack: without one the fire button starts none.
    pub loaded: bool,
    pub damage: f32,
    /// What a hit is pushed: along the attack, and up.
    pub push: f32,
    pub lift: f32,
    /// How far the rope reaches from the hand: in swings 1 and 2, swing 3, the jump attack.
    pub reach: [f32; 3],
    /// It hits from these ticks into a combo swing and the jump attack.
    pub combo_from: u32,
    pub jump_from: u32,
}

/// A shield round the hero (the Shield Charger's, `weapons.md`): a hit's damage comes off
/// its points first, and for `GRACE_TICKS` after one it takes every hit for nothing.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Shield {
    pub points: f32,
    pub grace: u32,
}

impl Shield {
    /// Ticks after a hit while it takes every hit for nothing (0x4C57B0's 15 frames).
    pub const GRACE_TICKS: u32 = 15;

    pub fn is_up(&self) -> bool {
        self.points > 0.0
    }

    /// Takes a hit of `damage`, returning what gets through to the hero.
    pub fn absorb(&mut self, damage: u32) -> u32 {
        if !self.is_up() {
            return damage;
        }
        if self.grace > 0 {
            return 0;
        }
        self.points -= damage as f32;
        self.grace = Self::GRACE_TICKS;
        if self.points >= 0.0 {
            return 0;
        }
        let through = (-self.points).ceil() as u32;
        self.points = 0.0;
        through
    }
}

/// How he holds a gun (row +0x18).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Hold {
    #[default]
    None,
    OneHand,
    TwoHands,
}

/// What the hero plays as a gun fires (`weapons.md`, "The hero's side of firing"): the
/// game's own animation numbers, which a game with the hero's animations maps to clips.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GunPose {
    /// Nothing for the gloves, which play their throw with an empty hand.
    #[serde(default)]
    pub hold: Hold,
    /// The whole-body firing animation, standing (row +0x24).
    #[serde(default)]
    pub standing: Option<u8>,
    /// The arms-only one on the move, and crouched (+0x28, +0x2C): on the right arm, and
    /// the left too for a gun held in both hands.
    #[serde(default)]
    pub arms: Option<u8>,
    #[serde(default)]
    pub crouched: Option<u8>,
}

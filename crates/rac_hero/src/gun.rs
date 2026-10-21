//! The hero's side of his guns (`docs/research/weapons.md`, "The hero's side of firing"):
//! whether one is in his hand, and the firing flags his moves read.
//!
//! The guns themselves belong to the game's weapons code (the kit's is `rac_combat`),
//! which fills in [`HeroMotion::gun`](crate::HeroMotion::gun) each tick before the hero
//! moves, as UYA's gun mobys update before the hero. A hero without guns keeps the
//! default: the wrench in hand, never firing. A wrench attack puts the gun away.

use serde::{Deserialize, Serialize};

/// What the hero's gun tells his moves.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// How he holds it and what he plays as it fires.
    pub pose: GunPose,
    /// Shots so far, counting round: each restarts the firing animation, as each of the
    /// game's guns calls `hero_fire_anim` (0x4E6A08) as it fires.
    pub shots: u8,
    /// The quick-select ring is open, and the move stick picks a gun in it: his moves read
    /// the stick as centred. UYA pauses the game while its ring is open; the kit runs on,
    /// so co-op peers stay in step, and he stands still instead.
    pub selecting: bool,
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

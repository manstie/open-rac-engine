//! The hero's side of his guns (`docs/research/weapons.md`, "The hero's side of firing"):
//! whether one is in his hand, and the firing flags his moves read.
//!
//! The guns themselves belong to the game's weapons code (the kit's is `rac_combat`),
//! which fills in [`HeroMotion::gun`](crate::HeroMotion::gun) each tick before the hero
//! moves, as UYA's gun mobys update before the hero. A hero without guns keeps the
//! default: the wrench in hand, never firing. A wrench attack puts the gun away.

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
}

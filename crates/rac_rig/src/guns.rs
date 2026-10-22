//! The gun in the hero's hand (P1.9): a model for each of a game's guns, by the game's item
//! number, shown in his right fist while the gun is out ([`rac_hero::GunHand::item`]), as
//! UYA spawns the held item's moby on his hand (`hero_spawn_item_mobys`, 0x4E7148). The
//! wrench hides meanwhile.
//!
//! - A gun is modelled in its own space: the grip at the origin, the barrel along −Z and
//!   its top along +Y. [`crate::model::gun_grip`] turns it into the fist, the same way
//!   for every model, so clips from a disc aim any gun as the game aims its own.
//! - [`open_guns`] are the open placeholders: a blocky gun for each of UYA's first six
//!   guns, nothing for the Shield Charger, and one for any other gun, made in code. A game may add its own
//!   ([`RigGuns::add`]), such as the guns' models from the user's disc.
//! - A gun may change its model as it levels up, as UYA's do from V5 (row +0x10): a model
//!   shows from its level ([`GunData::level`]) until the next model's.
//! - A gun with a skeleton of its own ([`GunSkeleton`]) plays its own sequences as the game
//!   does: it grows in, settles into its rest, and recoils as it fires
//!   ([`crate::gun_anim`]).

use std::collections::BTreeMap;

use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;

use crate::clips::RigClip;
use crate::hull::convex_hull;
use crate::shapes::{beam, block};
use crate::wear::{Paint, RigSurface, Surface, rest_from_binds};

/// A gun's model before it's registered.
#[derive(Debug, Clone, PartialEq)]
pub struct GunData {
    /// The game's item number it shows; `None` stands in for any gun without its own.
    pub item: Option<u16>,
    /// The gun's level it shows from, 0 for V1 ([`rac_hero::GunHand::level`]).
    pub level: u8,
    /// In the gun's own space, each in its own material ([`Paint::Own`]); surfaces in
    /// other paints aren't drawn. With a skeleton they are skinned to its joints, in its
    /// origin's space; without one they are rigid.
    pub surfaces: Vec<Surface>,
    pub skeleton: Option<GunSkeleton>,
}

/// A gun's own skeleton and sequences, as a moby's.
#[derive(Debug, Clone, PartialEq)]
pub struct GunSkeleton {
    /// Where its origin sits in the gun's space.
    pub origin: Transform,
    /// Each joint's parent, which comes before it; the first joint has none.
    pub parents: Vec<Option<usize>>,
    /// Each joint's place, turn and scale at rest from the origin: what its inverse bind
    /// undoes.
    pub binds: Vec<Mat4>,
    /// Its sequences by number. UYA's guns have three: 0 grows the gun from nothing as it
    /// comes out, 1 holds it, 2 recoils.
    pub clips: BTreeMap<u8, RigClip>,
    pub motion: GunMotion,
}

/// How a gun's own sequences follow its firing (`weapons.md`, "The gun's own animation").
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GunMotion {
    pub recoil: Recoil,
    /// Fired while it still grows in, it goes to its rest over these ticks instead.
    pub early: Option<f32>,
    /// Ticks it takes to go to its rest once a sequence has come round: 2 for UYA's guns,
    /// 0 (a cut) for the Plasma Coil's.
    pub settle_ticks: f32,
}

/// What a shot does to the gun's own animation.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Recoil {
    /// Nothing.
    #[default]
    None,
    /// Cuts to the recoil.
    Cut,
    /// Cuts to the recoil unless it plays it already, then plays at `speed`.
    Restart { speed: f32 },
    /// Blends to the recoil over `ticks`.
    Blend { ticks: f32 },
}

/// A registered gun model.
#[derive(Debug, Clone)]
pub(crate) struct RigGun {
    pub surfaces: Vec<RigSurface>,
    pub skeleton: Option<RigGunSkeleton>,
}

#[derive(Debug, Clone)]
pub(crate) struct RigGunSkeleton {
    pub origin: Transform,
    pub parents: Vec<Option<usize>>,
    /// Each joint's transform from its parent at rest.
    pub rest: Vec<Transform>,
    pub inverse_bindposes: Handle<SkinnedMeshInverseBindposes>,
    pub clips: BTreeMap<u8, RigClip>,
    pub motion: GunMotion,
}

/// Every gun model the rig can put in a hero's hand.
#[derive(Resource, Debug, Clone, Default)]
pub struct RigGuns {
    guns: BTreeMap<(Option<u16>, u8), RigGun>,
}

impl RigGuns {
    /// Adds a gun's model, replacing any for the same item from the same level.
    pub fn add(
        &mut self,
        data: GunData,
        meshes: &mut Assets<Mesh>,
        bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
    ) {
        let skinned = data.skeleton.is_some();
        let surfaces = data
            .surfaces
            .iter()
            .map(|surface| {
                let mut mesh = surface.mesh(skinned, Vec3::ZERO);
                if skinned && let Ok(bounded) = mesh.clone().with_generated_skinned_mesh_bounds() {
                    mesh = bounded;
                }
                RigSurface {
                    mesh: meshes.add(mesh),
                    paint: surface.paint.clone(),
                    wrench: false,
                }
            })
            .collect();
        let skeleton = data.skeleton.map(|skeleton| RigGunSkeleton {
            origin: skeleton.origin,
            rest: rest_from_binds(&skeleton.binds, &skeleton.parents),
            parents: skeleton.parents,
            inverse_bindposes: bindposes
                .add(skeleton.binds.iter().map(Mat4::inverse).collect::<Vec<_>>()),
            clips: skeleton.clips,
            motion: skeleton.motion,
        });
        self.guns
            .insert((data.item, data.level), RigGun { surfaces, skeleton });
    }

    /// The model for `item` at `level`, and the level it shows from: its own latest model
    /// by then, or the stand-in for guns without one.
    pub(crate) fn get(&self, item: u16, level: u8) -> Option<(u8, &RigGun)> {
        self.guns
            .range((Some(item), 0)..=(Some(item), level))
            .next_back()
            .or_else(|| self.guns.range((None, 0)..=(None, u8::MAX)).next())
            .map(|((_, from), gun)| (*from, gun))
    }

    /// Whether `item` has a model of its own.
    pub fn has(&self, item: u16) -> bool {
        self.guns
            .range((Some(item), 0)..=(Some(item), u8::MAX))
            .next()
            .is_some()
    }
}

/// UYA's items for the six guns the kit has (`weapons.md`, "Items").
mod item {
    pub const SHOCK_BLASTER: u16 = 0x27;
    pub const N60_STORM: u16 = 0x2F;
    pub const ANNIHILATOR: u16 = 0x3F;
    pub const FLUX_RIFLE: u16 = 0x6F;
    pub const NITRO_LAUNCHER: u16 = 0x77;
    pub const LAVA_GUN: u16 = 0x11;
    /// The Shield Charger, whose row's class is the shield round him: he holds nothing.
    pub const SHIELD_CHARGER: u16 = 0x16;
}

/// What a block of an open gun is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    /// Dark gunmetal.
    Body,
    /// The gun's own colour.
    Trim,
    /// The gun's colour, glowing: muzzles, emitters and tanks.
    Glow,
}

/// One open gun: its colour and its blocks.
struct OpenGun {
    item: Option<u16>,
    colour: Color,
    blocks: Vec<(Tone, Vec<Vec3>)>,
}

/// A block from its centre and half sizes (right, up, back), slightly bevelled.
fn boxed(centre: [f32; 3], half: [f32; 3]) -> Vec<Vec3> {
    block(Vec3::from_array(centre), Vec3::from_array(half), 0.006)
}

/// A barrel or tube running from `from` to `to`, square in section.
fn tube(from: [f32; 3], to: [f32; 3], half: f32, end: f32) -> Vec<Vec3> {
    beam(
        Vec3::from_array(from),
        Vec3::from_array(to),
        Vec3::X,
        Vec2::splat(half),
        Vec2::splat(end),
        0.004,
    )
}

/// The grip under the origin, raked back a little as a pistol's.
fn grip() -> Vec<Vec3> {
    tube([0.0, 0.03, -0.01], [0.0, -0.08, 0.025], 0.02, 0.018)
}

/// The open guns' designs, in metres. Each only has to read at a glance: its size, what
/// it fires from, and its colour.
fn open_designs() -> Vec<OpenGun> {
    use Tone::*;
    vec![
        // A compact blaster with a forked emitter.
        OpenGun {
            item: Some(item::SHOCK_BLASTER),
            colour: Color::srgb(0.3, 0.8, 1.0),
            blocks: vec![
                (Body, boxed([0.0, 0.05, -0.07], [0.035, 0.04, 0.12])),
                (Body, grip()),
                (Trim, boxed([0.0, 0.095, -0.05], [0.02, 0.012, 0.08])),
                (
                    Trim,
                    tube([0.025, 0.05, -0.18], [0.025, 0.05, -0.26], 0.012, 0.006),
                ),
                (
                    Trim,
                    tube([-0.025, 0.05, -0.18], [-0.025, 0.05, -0.26], 0.012, 0.006),
                ),
                (Glow, boxed([0.0, 0.05, -0.2], [0.014, 0.014, 0.012])),
            ],
        },
        // Two barrels side by side over a drum.
        OpenGun {
            item: Some(item::N60_STORM),
            colour: Color::srgb(1.0, 0.75, 0.2),
            blocks: vec![
                (Body, boxed([0.0, 0.05, -0.02], [0.045, 0.045, 0.08])),
                (Body, grip()),
                (
                    Body,
                    tube([0.022, 0.06, -0.08], [0.022, 0.06, -0.3], 0.016, 0.014),
                ),
                (
                    Body,
                    tube([-0.022, 0.06, -0.08], [-0.022, 0.06, -0.3], 0.016, 0.014),
                ),
                (Trim, boxed([0.0, -0.005, -0.06], [0.035, 0.03, 0.035])),
                (Glow, boxed([0.022, 0.06, -0.305], [0.01, 0.01, 0.006])),
                (Glow, boxed([-0.022, 0.06, -0.305], [0.01, 0.01, 0.006])),
            ],
        },
        // A fat tube with a canister on top.
        OpenGun {
            item: Some(item::NITRO_LAUNCHER),
            colour: Color::srgb(1.0, 0.45, 0.1),
            blocks: vec![
                (
                    Body,
                    tube([0.0, 0.07, 0.1], [0.0, 0.07, -0.28], 0.055, 0.06),
                ),
                (Body, grip()),
                (
                    Trim,
                    tube([0.0, 0.07, -0.26], [0.0, 0.07, -0.31], 0.068, 0.068),
                ),
                (Trim, boxed([0.0, 0.14, 0.0], [0.025, 0.02, 0.06])),
                (Glow, boxed([0.0, 0.07, -0.312], [0.035, 0.035, 0.006])),
            ],
        },
        // A nozzle fed from a tank on its back.
        OpenGun {
            item: Some(item::LAVA_GUN),
            colour: Color::srgb(1.0, 0.25, 0.05),
            blocks: vec![
                (Body, boxed([0.0, 0.05, -0.04], [0.04, 0.04, 0.11])),
                (Body, grip()),
                (
                    Body,
                    tube([0.0, 0.05, -0.15], [0.0, 0.04, -0.32], 0.025, 0.012),
                ),
                (Glow, boxed([0.0, 0.13, 0.04], [0.045, 0.045, 0.07])),
                (Trim, boxed([0.0, 0.13, 0.04], [0.05, 0.012, 0.075])),
                (Glow, boxed([0.0, 0.04, -0.325], [0.008, 0.008, 0.006])),
            ],
        },
        // A launcher of four tubes in a block, on the shoulder side.
        OpenGun {
            item: Some(item::ANNIHILATOR),
            colour: Color::srgb(0.4, 1.0, 0.3),
            blocks: vec![
                (Body, boxed([0.0, 0.1, -0.06], [0.075, 0.075, 0.2])),
                (Body, grip()),
                (Trim, boxed([0.0, 0.1, -0.265], [0.08, 0.08, 0.012])),
                (Glow, boxed([0.035, 0.135, -0.28], [0.022, 0.022, 0.004])),
                (Glow, boxed([-0.035, 0.135, -0.28], [0.022, 0.022, 0.004])),
                (Glow, boxed([0.035, 0.065, -0.28], [0.022, 0.022, 0.004])),
                (Glow, boxed([-0.035, 0.065, -0.28], [0.022, 0.022, 0.004])),
            ],
        },
        // A long thin rifle with a stock and a scope.
        OpenGun {
            item: Some(item::FLUX_RIFLE),
            colour: Color::srgb(0.6, 0.4, 1.0),
            blocks: vec![
                (Body, boxed([0.0, 0.045, -0.02], [0.03, 0.035, 0.09])),
                (Body, grip()),
                (
                    Body,
                    tube([0.0, 0.05, -0.1], [0.0, 0.05, -0.52], 0.016, 0.012),
                ),
                (Body, tube([0.0, 0.03, 0.07], [0.0, 0.0, 0.2], 0.022, 0.03)),
                (
                    Trim,
                    tube([0.0, 0.1, 0.02], [0.0, 0.1, -0.12], 0.018, 0.018),
                ),
                (Glow, boxed([0.0, 0.05, -0.525], [0.01, 0.01, 0.006])),
            ],
        },
        // Nothing in his hand.
        OpenGun {
            item: Some(item::SHIELD_CHARGER),
            colour: Color::WHITE,
            blocks: Vec::new(),
        },
        // Any other gun.
        OpenGun {
            item: None,
            colour: Color::srgb(0.9, 0.9, 0.95),
            blocks: vec![
                (Body, boxed([0.0, 0.05, -0.06], [0.035, 0.04, 0.12])),
                (Body, grip()),
                (
                    Body,
                    tube([0.0, 0.05, -0.17], [0.0, 0.05, -0.26], 0.015, 0.012),
                ),
                (Glow, boxed([0.0, 0.05, -0.265], [0.01, 0.01, 0.005])),
            ],
        },
    ]
}

/// The open placeholder guns, flat-shaded in their own materials.
pub fn open_guns(materials: &mut Assets<StandardMaterial>) -> Vec<GunData> {
    let body = materials.add(StandardMaterial {
        base_color: Color::srgb(0.17, 0.18, 0.21),
        metallic: 0.8,
        perceptual_roughness: 0.4,
        ..default()
    });
    open_designs()
        .into_iter()
        .map(|design| {
            let colour = design.colour.to_linear();
            let trim = materials.add(StandardMaterial {
                base_color: design.colour,
                metallic: 0.6,
                perceptual_roughness: 0.35,
                ..default()
            });
            let glow = materials.add(StandardMaterial {
                base_color: design.colour,
                emissive: colour * 3.0,
                ..default()
            });
            let surfaces = [
                (Tone::Body, &body),
                (Tone::Trim, &trim),
                (Tone::Glow, &glow),
            ]
            .into_iter()
            .filter_map(|(tone, material)| {
                let mut surface = Surface::new(Paint::Own(material.clone()), false);
                for (_, points) in design.blocks.iter().filter(|(t, _)| *t == tone) {
                    surface.push_rigid(&convex_hull(points), 0);
                }
                (!surface.indices.is_empty()).then_some(surface)
            })
            .collect();
            GunData {
                item: design.item,
                level: 0,
                surfaces,
                skeleton: None,
            }
        })
        .collect()
}

/// Registers the open guns at start-up, before any game adds its own.
pub(crate) fn register_open_guns(
    mut guns: ResMut<RigGuns>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    bindposes: Option<ResMut<Assets<SkinnedMeshInverseBindposes>>>,
) {
    let (Some(mut meshes), Some(mut materials), Some(mut bindposes)) =
        (meshes, materials, bindposes)
    else {
        return;
    };
    for gun in open_guns(&mut materials) {
        if gun.item.is_none_or(|item| !guns.has(item)) {
            guns.add(gun, &mut meshes, &mut bindposes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_open_gun_points_its_barrel_down_minus_z_from_a_grip_at_the_origin() {
        for design in open_designs().into_iter().filter(|d| !d.blocks.is_empty()) {
            let points: Vec<Vec3> = design.blocks.iter().flat_map(|(_, p)| p.clone()).collect();
            let front = points.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
            let back = points.iter().map(|p| p.z).fold(f32::NEG_INFINITY, f32::max);
            assert!(front < -0.2, "{:?} reaches {front}", design.item);
            assert!(
                -front > back,
                "{:?}: more ahead of the grip than behind",
                design.item
            );
            let low = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
            assert!(
                low < -0.05,
                "{:?}: the grip hangs below the fist",
                design.item
            );
        }
    }

    #[test]
    fn a_gun_without_its_own_model_gets_the_stand_in() {
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let mut bindposes = Assets::<SkinnedMeshInverseBindposes>::default();
        let mut guns = RigGuns::default();
        for gun in open_guns(&mut materials) {
            guns.add(gun, &mut meshes, &mut bindposes);
        }
        assert!(guns.has(item::N60_STORM));
        assert!(!guns.has(0x37), "the Infector has no open model");
        let (_, stand_in) = guns.get(0x37, 0).unwrap();
        assert_eq!(stand_in.surfaces.len(), 2, "body and glow");
        assert_ne!(
            guns.get(item::N60_STORM, 0).unwrap().1.surfaces,
            stand_in.surfaces
        );
    }

    #[test]
    fn a_gun_shows_its_latest_model_by_its_level() {
        let mut meshes = Assets::<Mesh>::default();
        let mut bindposes = Assets::<SkinnedMeshInverseBindposes>::default();
        let mut guns = RigGuns::default();
        let mut add = |item, level, triangles: usize| {
            let mut surface = Surface::new(Paint::Own(Handle::default()), false);
            for _ in 0..triangles {
                surface.push_rigid(&[[Vec3::ZERO, Vec3::X, Vec3::Y]], 0);
            }
            guns.add(
                GunData {
                    item,
                    level,
                    surfaces: vec![surface],
                    skeleton: None,
                },
                &mut meshes,
                &mut bindposes,
            );
        };
        add(None, 0, 1);
        add(Some(item::N60_STORM), 0, 2);
        add(Some(item::N60_STORM), 4, 3);
        let from = |guns: &RigGuns, item, level| guns.get(item, level).map(|(from, _)| from);
        assert_eq!(from(&guns, item::N60_STORM, 0), Some(0));
        assert_eq!(from(&guns, item::N60_STORM, 3), Some(0));
        assert_eq!(from(&guns, item::N60_STORM, 4), Some(4));
        assert_eq!(from(&guns, item::N60_STORM, 7), Some(4));
        assert!(guns.has(item::N60_STORM));
        // Another gun gets the stand-in at any level.
        assert!(!guns.has(item::LAVA_GUN));
        assert_eq!(from(&guns, item::LAVA_GUN, 5), Some(0));
        assert_eq!(
            guns.get(item::LAVA_GUN, 5).unwrap().1.surfaces,
            guns.get(0x37, 0).unwrap().1.surfaces
        );
    }
}

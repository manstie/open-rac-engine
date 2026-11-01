//! The Quantum Whip's cutting wave as the game draws it (0x3D6C00 and 0x3D6740; `effects.md`,
//! "The Quantum Whip's cutting wave"): strands from the middle of its blade to each tip
//! each frame, and particles laid between its joints each tick. The blade itself is a
//! class a game gives ([`ShotModels`](crate::ShotModels)), and its ribbon a
//! [`Sweep`](super::sweep::Sweep).

use std::array;

use bevy::prelude::*;
use rac_combat::{Armory, Arsenal};
use rac_hero::Hero;
use rac_sim::SimTick;
use serde::Deserialize;

use super::rope::{
    AlongRope, STRAND_ORDER, Slots, Strand, drop_undrawn, lay_strands, put_mesh, strand_material,
};
use super::{FxImages, FxRecipes, FxRng, Making, eye};

/// What shows on a shot's blade: `joints` from tip to tip, each how far across its way and
/// how far behind the shot it lies. Each of `strands` runs from the middle joint towards
/// each tip, and `along` lays particles on each piece between the joints from `along_from`
/// ticks into its flight.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Blade {
    pub joints: Vec<(f32, f32)>,
    pub strands: Vec<Strand>,
    pub along: Vec<AlongRope>,
    pub along_from: u32,
}

impl Blade {
    /// Its joints in the world, the shot at `at` going `velocity`.
    fn placed(&self, at: Vec3, velocity: Vec3) -> Vec<Vec3> {
        let way = velocity.normalize_or(Vec3::NEG_Z);
        let across = Vec3::Y.cross(way).normalize_or(Vec3::X);
        self.joints
            .iter()
            .map(|&(side, back)| at + across * side - way * back)
            .collect()
    }

    /// The four points the game lays a strand through, from the middle joint to `tip`, its
    /// last a quarter short of it (0x3D6C00).
    fn spoke(middle: Vec3, tip: Vec3) -> [Vec3; 4] {
        array::from_fn(|k| middle.lerp(tip, k as f32 * 0.25))
    }
}

/// Where a blade's strands come in the order see-through things are drawn: after its model,
/// which the game draws before its effects, and on top of its smoke (take 14).
const BLADE_STRANDS: f32 = 100.0;

/// The strands drawn now: by player, the shot, which tip and the strand's place in its
/// recipe.
#[derive(Resource, Debug, Default)]
pub(crate) struct BladeStrands(Slots<(usize, u32, usize, usize)>);

/// Lays each blade's strands this frame, where its model is drawn: between its last two
/// places, as a shot's transform is interpolated.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_blades(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    tick: Res<SimTick>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut strands: ResMut<BladeStrands>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut places: Query<&mut Transform>,
    mut commands: Commands,
) {
    let eye = eye(&cameras);
    let ticks = time.elapsed_secs_wrapped() * 60.0;
    let behind = fixed.timestep().as_secs_f32() * (1.0 - fixed.overstep_fraction());
    let mut drawn = Vec::new();
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let Some(blade) = recipes.blade(key, shot.level, Some(shot.kind.name())) else {
                continue;
            };
            let at = shot.position - shot.velocity * behind;
            let joints = blade.placed(at, shot.velocity);
            let (Some(&first), Some(&last)) = (joints.first(), joints.last()) else {
                continue;
            };
            let middle = joints[joints.len() / 2];
            let which = (hero.player as u64) << 32 | shot.serial as u64;
            let mut rng = FxRng::for_tick(tick.0, which);
            for (end, tip) in [first, last].into_iter().enumerate() {
                let base = Blade::spoke(middle, tip);
                let laid = lay_strands(&blade.strands, &base, false, eye, ticks, &mut rng);
                for (index, mesh, place) in laid {
                    let id = (hero.player, shot.serial, end, index);
                    let strand = &blade.strands[index];
                    put_mesh(
                        &mut strands.0,
                        id,
                        mesh,
                        place,
                        &mut meshes,
                        &mut places,
                        |shown| {
                            let order = BLADE_STRANDS + STRAND_ORDER * index as f32;
                            let material = strand_material(&mut materials, &images, strand, order);
                            commands
                                .spawn((Name::new("blade strand"), shown, MeshMaterial3d(material)))
                                .id()
                        },
                    );
                    drawn.push(id);
                }
            }
        }
    }
    drop_undrawn(&mut strands.0, &drawn, &mut commands);
}

/// Lays each blade's particles between its joints.
pub(crate) fn emit_along_blades(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut rng: ResMut<FxRng>,
    mut making: Making,
) {
    let mut spawner = making.spawner();
    let eye = eye(&cameras);
    for (_, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let Some(blade) = recipes.blade(key, shot.level, Some(shot.kind.name())) else {
                continue;
            };
            if shot.ticks < blade.along_from {
                continue;
            }
            let joints = blade.placed(shot.position, shot.velocity);
            for along in &blade.along {
                for piece in joints.windows(2) {
                    let count =
                        along.per_piece as u32 + u32::from(rng.unit() < along.per_piece.fract());
                    for _ in 0..count {
                        let at = piece[0].lerp(piece[1], rng.unit());
                        for emit in recipes.expand(std::slice::from_ref(&along.emit)) {
                            spawner.emit(emit, at, None, eye, &mut rng);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blade(recipes: &FxRecipes) -> &Blade {
        recipes.blade("whip", 4, Some("cut")).expect("the blade")
    }

    #[test]
    fn its_tips_trail_either_side_of_its_way() {
        let recipes = FxRecipes::default();
        let joints = blade(&recipes).placed(Vec3::Y, Vec3::NEG_Z * 30.0);
        assert_eq!(joints.len(), 7);
        // The middle leads, and the tips are about 4.8 u apart, 2.25 u behind it.
        assert!((joints[3] - Vec3::new(0.0, 1.0, 0.03)).length() < 1e-3);
        let (a, b) = (joints[0], joints[6]);
        assert!((a.distance(b) - 4.8).abs() < 1e-3);
        assert!(
            (a.z - 3.25 + 1.0).abs() < 1e-3 && (b.z - a.z).abs() < 1e-3,
            "{a} {b}"
        );
        assert!((a.y - 1.0).abs() < 1e-3);
    }

    #[test]
    fn a_spoke_stops_a_quarter_short_of_the_tip() {
        let spoke = Blade::spoke(Vec3::ZERO, Vec3::X * 4.0);
        assert_eq!(spoke, [Vec3::ZERO, Vec3::X, Vec3::X * 2.0, Vec3::X * 3.0]);
    }

    #[test]
    fn only_the_quantum_whips_cuts_have_a_blade_and_they_end_without_the_ropes_sparks() {
        let recipes = FxRecipes::default();
        assert!(recipes.blade("whip", 4, None).is_none());
        assert!(recipes.blade("whip", 4, Some("wave")).is_none());
        assert!(recipes.impact("whip", 4, Some("cut")).is_empty());
        assert_eq!(recipes.sweep("whip", 4, Some("cut")).len(), 1);
    }
}

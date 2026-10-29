//! Arcs fanned out from a shot to each of its ends, drawn afresh each frame along the
//! game's curve: the Spitting Hydra's spit (0x321210, drawn by 0x321E98; `effects.md`,
//! "Spitting Hydra").

use bevy::prelude::*;
use rac_combat::weapons::ShotKind;
use rac_combat::{Armory, Arsenal};
use rac_hero::Hero;
use serde::Deserialize;

use super::rope::{Slots, Strand, drop_undrawn, lay_strands, put_mesh};
use super::{FxImages, FxRecipes, FxRng, eye, fx_material};

/// Strands from a shot to each of its ends, through `points` points of the game's curve
/// (0x4420D0): it leaves the shot heading away from a point `bow` units off it, below it
/// turned round the way by `roll.0` degrees for the first end and `roll.1` more for each
/// after, and comes into its end heading for a point `past` units beyond it. Each end
/// first shows `stagger.0` ticks after the one before, plus up to `stagger.1`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Fan {
    pub points: usize,
    pub bow: f32,
    pub roll: (f32, f32),
    pub past: f32,
    #[serde(default)]
    pub stagger: (u32, u32),
    pub strands: Vec<Strand>,
}

impl Fan {
    /// The points of end `index`'s arc from `from` to `to`, both included.
    pub(crate) fn curve(&self, index: usize, from: Vec3, to: Vec3) -> Vec<Vec3> {
        let way = (to - from).normalize_or(Vec3::NEG_Z);
        let roll = (self.roll.0 + self.roll.1 * index as f32).to_radians();
        let control = from + Quat::from_axis_angle(way, roll) * Vec3::NEG_Y * self.bow;
        let beyond = to + way * self.past;
        // p(t) = b t³ + (a − b) t² + (to − control) t + from, with a = control − from and
        // b = beyond − to − a: it heads along to − control at the start and along
        // beyond − from at the end.
        let a = control - from;
        let b = beyond - to - a;
        let last = self.points.max(2) - 1;
        (0..=last)
            .map(|point| {
                // The game steps 1/16 to its fifteenth point and puts its last at the end.
                let t = if point == last {
                    1.0
                } else {
                    point as f32 / self.points as f32
                };
                b * t * t * t + (a - b) * t * t + (to - control) * t + from
            })
            .collect()
    }

    /// When end `index` of shot `serial` first shows, in ticks of the shot's age.
    fn shows_at(&self, serial: u32, index: usize) -> u32 {
        // The game rolls each end's wait once; a hash of the shot and end stands for it.
        let roll = (u64::from(serial) << 8 | index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.stagger.0 * index as u32 + (roll >> 40) as u32 % (self.stagger.1 + 1)
    }
}

/// The fans' strands drawn now: by player, shot, end and the strand's place in its
/// recipe.
#[derive(Resource, Debug, Default)]
pub(crate) struct Fans(Slots<(usize, u32, usize, usize)>);

/// Lays each fanned shot's strands to its ends this frame, making those that start and
/// dropping those whose shots are gone.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_fans(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    time: Res<Time>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut fans: ResMut<Fans>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut places: Query<&mut Transform>,
    mut commands: Commands,
) {
    let eye = eye(&cameras);
    let ticks = time.elapsed_secs_wrapped() * 60.0;
    let mut drawn = Vec::new();
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let ShotKind::Spit { ends, .. } = &shot.kind else {
                continue;
            };
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let Some(fan) = recipes.fan(key, shot.level, Some(shot.kind.name())) else {
                continue;
            };
            for (end, &to) in ends.iter().enumerate() {
                if shot.ticks < fan.shows_at(shot.serial, end) {
                    continue;
                }
                let curve = fan.curve(end, shot.position, to);
                let laid = lay_strands(&fan.strands, &curve, false, eye, ticks, &mut rng);
                for (index, mesh, place) in laid {
                    let id = (hero.player, shot.serial, end, index);
                    let strand = &fan.strands[index];
                    put_mesh(
                        &mut fans.0,
                        id,
                        mesh,
                        place,
                        &mut meshes,
                        &mut places,
                        |shown| {
                            let material = fx_material(
                                &mut materials,
                                &images,
                                strand.texture,
                                strand.colour,
                                strand.blend,
                            );
                            commands
                                .spawn((Name::new("fan strand"), shown, MeshMaterial3d(material)))
                                .id()
                        },
                    );
                    drawn.push(id);
                }
            }
        }
    }
    drop_undrawn(&mut fans.0, &drawn, &mut commands);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fan(roll: (f32, f32)) -> Fan {
        Fan {
            points: 16,
            bow: 15.0,
            roll,
            past: 30.0,
            stagger: (2, 7),
            strands: Vec::new(),
        }
    }

    #[test]
    fn an_arc_runs_from_the_shot_to_its_end_bowing_away_from_its_turn() {
        let (from, to) = (Vec3::ZERO, Vec3::new(0.0, 0.0, -32.0));
        // Turned 0°, the point it heads away from is below: it bows up.
        let curve = fan((0.0, 107.0)).curve(0, from, to);
        assert_eq!(curve.len(), 16);
        assert_eq!(curve[0], from);
        assert!(curve[15].distance(to) < 1e-4);
        assert!(curve[1..15].iter().all(|point| point.y > 0.0));
        // The next end is turned 107° round the way: it bows to the side, as far.
        let side = fan((0.0, 107.0)).curve(1, from, to);
        assert!(side[7].x.abs() > side[7].y.abs());
        assert!((side[7].length() - curve[7].length()).abs() < 1e-3);
    }

    #[test]
    fn each_end_shows_after_the_one_before_within_its_stagger() {
        let fan = fan((0.0, 107.0));
        for serial in 0..20 {
            for end in 0..7 {
                let at = fan.shows_at(serial, end);
                assert!((2 * end as u32..=2 * end as u32 + 7).contains(&at), "{at}");
            }
        }
    }
}

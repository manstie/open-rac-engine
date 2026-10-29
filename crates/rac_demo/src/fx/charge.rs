//! What a gun shows while it charges ([`Arsenal::charge_ticks`]): the Spitting Hydra's
//! strands crackling round its muzzle (0x31FB90, drawn by 0x321CB8), and the glow and
//! light at its muzzle (0x322A80), which grow as it is held, and its glare once charged
//! far enough (0x322B80; `effects.md`, "Spitting Hydra").

use std::collections::BTreeMap;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rac_combat::{Armory, Arsenal};
use rac_hero::{CollisionScene, CollisionWorld, Hero};
use rac_sim::SimTick;
use serde::Deserialize;

use super::fan::cubic;
use super::light::{self, ShotLight, point_light};
use super::rope::{Slots, Strand, drop_undrawn, lay_strands, put_mesh};
use super::{FxImages, FxRecipes, FxRng, Glare, Making, Trail, eye, fx_material};
use crate::DemoEntity;

/// A gun's look while it charges. Its charge is `growth` for each tick held, at most `most`
/// (the Hydra's 0.007 and 0.7).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Charge {
    pub growth: f32,
    pub most: f32,
    /// The strands' alpha is their colour's times this for each tick held: the game blends
    /// them at half the ticks held, out of 0x80.
    pub alpha_per_tick: f32,
    #[serde(default)]
    pub crackles: Vec<Crackle>,
    /// Particles laid at the muzzle each tick, sized by the charge over `most`.
    #[serde(default)]
    pub glow: Vec<Trail>,
    /// A light at the muzzle, its radius times the charge over `most`.
    #[serde(default)]
    pub light: Option<ShotLight>,
    /// Rays round the muzzle once the charge is at least `glare_from`, one a tick.
    #[serde(default)]
    pub glare: Option<Glare>,
    #[serde(default)]
    pub glare_from: f32,
}

impl Charge {
    fn amount(&self, held: u32) -> f32 {
        (held as f32 * self.growth).min(self.most)
    }

    /// The charge over its most: what the glow and light are sized by.
    fn share(&self, held: u32) -> f32 {
        if self.most > 0.0 {
            self.amount(held) / self.most
        } else {
            0.0
        }
    }
}

/// `count` strands from the muzzle while it has been held more than `after` ticks, each
/// laid anew every `every` ticks through `points` points of the game's curve to its end
/// ([`Reach`]). Each leaves the muzzle heading away from a point at a random turn round his
/// facing, a random `bow` (plus the charge) off the muzzle.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Crackle {
    pub count: usize,
    pub points: usize,
    #[serde(default)]
    pub after: u32,
    pub every: u32,
    pub bow: (f32, f32),
    pub reach: Reach,
    pub strands: Vec<Strand>,
}

/// Where a crackle's strands end.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum Reach {
    /// Back into the gun (its point 1), `back` behind the muzzle along his facing and `up`
    /// above it, coming in heading on from a random `past` (less the charge) behind the
    /// muzzle.
    Gun {
        back: f32,
        up: f32,
        past: (f32, f32),
    },
    /// Out to a wall within `reach` of the muzzle a random way, else to the floor `reach`
    /// from his feet within `spread` degrees of his facing, kept `keep` ticks; coming in
    /// heading on from a random `past` (plus the charge) times the way from the muzzle.
    World {
        reach: f32,
        spread: f32,
        keep: (u32, u32),
        past: (f32, f32),
    },
}

/// Where the gun is: the muzzle, his facing level and his feet.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Gun {
    muzzle: Vec3,
    facing: Vec3,
    feet: Vec3,
}

impl Gun {
    /// As the weapons code puts the muzzle (`GunRules::muzzle`), from his place without
    /// its pitch.
    fn of(place: &GlobalTransform, armory: &Armory) -> Self {
        let facing = (place.rotation() * Vec3::NEG_Z)
            .with_y(0.0)
            .normalize_or(Vec3::NEG_Z);
        let right = facing.cross(Vec3::Y);
        let [r, u, f] = armory.rules.muzzle;
        let feet = place.translation();
        Self {
            muzzle: feet + right * r + Vec3::Y * u + facing * f,
            facing,
            feet,
        }
    }
}

/// One strand's way as last laid, and a world reach's end with its ticks left.
#[derive(Debug, Default, Clone, PartialEq)]
struct Laid {
    points: Vec<Vec3>,
    end: Option<(Vec3, u32)>,
}

impl Crackle {
    /// Lays `laid` anew, or for a world reach whose end has had its time, picks a new end
    /// and keeps the way it had (0x31FB90 draws the old points that frame).
    fn lay(
        &self,
        laid: &mut Laid,
        gun: &Gun,
        charge: f32,
        world: Option<&dyn CollisionWorld>,
        rng: &mut FxRng,
    ) {
        // Straight up turned round his facing: a way at random across it.
        let turn = Quat::from_axis_angle(gun.facing, rng.between((-180.0, 180.0)).to_radians());
        let control = gun.muzzle + turn * Vec3::Y * (rng.between(self.bow) + charge);
        let (to, beyond) = match self.reach {
            Reach::Gun { back, up, past } => (
                gun.muzzle - gun.facing * back + Vec3::Y * up,
                gun.muzzle + gun.facing * (charge - rng.between(past)),
            ),
            Reach::World {
                reach,
                spread,
                keep,
                past,
            } => {
                let end = match laid.end {
                    Some((end, left)) if left > 1 => {
                        laid.end = Some((end, left - 1));
                        end
                    }
                    _ => {
                        let end = world_end(gun, reach, spread, world, rng);
                        laid.end = Some((end, rng.ticks(keep.0, keep.1)));
                        return;
                    }
                };
                let beyond = gun.muzzle + (end - gun.muzzle) * (rng.between(past) + charge);
                (end, beyond)
            }
        };
        laid.points = cubic(self.points, gun.muzzle, to, control, beyond);
    }
}

/// A new end for a world reach: where a line `reach` long from the muzzle a random way
/// meets the world, else on the floor `reach` from his feet within `spread` degrees of
/// his facing (0x31FB90's `coll_line`, then the hero's position).
fn world_end(
    gun: &Gun,
    reach: f32,
    spread: f32,
    world: Option<&dyn CollisionWorld>,
    rng: &mut FxRng,
) -> Vec3 {
    let way = rng.direction(None) * reach;
    if let Some(hit) = world.and_then(|world| world.cast_ray(gun.muzzle, way)) {
        return gun.muzzle + way * hit.fraction;
    }
    let turn = Quat::from_rotation_y(rng.between((-spread, spread)).to_radians());
    gun.feet + turn * gun.facing * reach
}

/// The charges' strands: each way as last laid, by player, crackle and strand, the tick
/// they were laid on, and their meshes by those and the strand's place in its recipe.
#[derive(Resource, Debug, Default)]
pub(crate) struct Charges {
    laid: BTreeMap<(usize, usize, usize), Laid>,
    tick: u64,
    shown: Slots<(usize, usize, usize, usize)>,
}

/// The recipe of the charge a hero's gun in hand is under, the ticks it has been held, and
/// where the gun is.
fn charging<'a>(
    recipes: &'a FxRecipes,
    armory: &Armory,
    arsenal: &Arsenal,
    place: &GlobalTransform,
) -> Option<(&'a Charge, u32, Gun)> {
    let held = arsenal.charge_ticks()?;
    let weapon = arsenal.hand.gun()?;
    let def = armory.get(weapon)?;
    let level = arsenal.gun(weapon).map_or(0, |gun| gun.level);
    let charge = recipes.charge(&def.effects, level)?;
    Some((charge, held, Gun::of(place, armory)))
}

/// Lays each charging gun's strands, anew on the ticks each crackle is laid on, and draws
/// them at its charge's alpha; drops those of guns no longer charging.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_charges(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    time: Res<Time>,
    tick: Res<SimTick>,
    scene: Option<Res<CollisionScene>>,
    heroes: Query<(&Hero, &Arsenal, &GlobalTransform)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut charges: ResMut<Charges>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut places: Query<&mut Transform>,
    looks: Query<&MeshMaterial3d<StandardMaterial>>,
    mut commands: Commands,
) {
    let eye = eye(&cameras);
    let ticks = time.elapsed_secs_wrapped() * 60.0;
    let world = scene
        .as_deref()
        .map(|scene| &scene.0 as &dyn CollisionWorld);
    let new_tick = (tick.0 != charges.tick).then_some(tick.0);
    charges.tick = tick.0;
    let charges = &mut *charges;
    let mut kept = Vec::new();
    let mut drawn = Vec::new();
    for (hero, arsenal, place) in &heroes {
        let Some((charge, held, gun)) = charging(&recipes, &armory, arsenal, place) else {
            continue;
        };
        let amount = charge.amount(held);
        let fade = held as f32 * charge.alpha_per_tick;
        for (which, crackle) in charge.crackles.iter().enumerate() {
            if held <= crackle.after {
                continue;
            }
            let lay = new_tick.is_some_and(|tick| tick % u64::from(crackle.every.max(1)) == 0);
            for strand_of in 0..crackle.count {
                let id = (hero.player, which, strand_of);
                kept.push(id);
                let laid = charges.laid.entry(id).or_default();
                // A new one is laid at once (a world reach's only picks its end).
                if lay || (laid.points.is_empty() && laid.end.is_none()) {
                    crackle.lay(laid, &gun, amount, world, &mut rng);
                }
                let laid = lay_strands(&crackle.strands, &laid.points, false, eye, ticks, &mut rng);
                for (index, mesh, at) in laid {
                    let strand = &crackle.strands[index];
                    let mut colour = strand.colour;
                    colour[3] *= fade;
                    let slot = (hero.player, which, strand_of, index);
                    put_mesh(
                        &mut charges.shown,
                        slot,
                        mesh,
                        at,
                        &mut meshes,
                        &mut places,
                        |shown| {
                            let material = fx_material(
                                &mut materials,
                                &images,
                                strand.texture,
                                colour,
                                strand.blend,
                            );
                            commands
                                .spawn((
                                    Name::new("charge strand"),
                                    shown,
                                    MeshMaterial3d(material),
                                ))
                                .id()
                        },
                    );
                    // Its alpha follows the charge.
                    if let Some(&(entity, _)) = charges.shown.get(&slot)
                        && let Ok(look) = looks.get(entity)
                        && let Some(mut material) = materials.get_mut(&look.0)
                    {
                        material.base_color.set_alpha(colour[3]);
                    }
                    drawn.push(slot);
                }
            }
        }
    }
    charges.laid.retain(|id, _| kept.contains(id));
    drop_undrawn(&mut charges.shown, &drawn, &mut commands);
}

/// The lights at charging guns' muzzles, by player.
#[derive(Resource, Debug, Default)]
pub(crate) struct ChargeLights(BTreeMap<usize, Entity>);

/// A light at a charging gun's muzzle.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct ChargeLit;

/// Lays each charging gun's glow at its muzzle a tick, and keeps its light there, sized by
/// its charge, within the game's light slots.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_charges(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal, &GlobalTransform)>,
    others: Query<
        (),
        (
            Or<(With<light::Lit>, With<light::Riding>)>,
            Without<ChargeLit>,
        ),
    >,
    mut lit: ResMut<ChargeLights>,
    mut lights: Query<(&mut Transform, &mut PointLight), With<ChargeLit>>,
    mut rng: ResMut<FxRng>,
    mut making: Making,
) {
    let mut free = light::SLOTS.saturating_sub(others.iter().count() + lit.0.len());
    let mut shown = BTreeMap::new();
    let mut spawner = making.spawner();
    for (hero, arsenal, place) in &heroes {
        let Some((charge, held, gun)) = charging(&recipes, &armory, arsenal, place) else {
            continue;
        };
        let share = charge.share(held);
        for glow in &charge.glow {
            for _ in 0..glow.count(0.0, rng.unit()) {
                spawner.trail(glow, gun.muzzle, Vec3::ZERO, share, &mut rng);
            }
        }
        if let Some(glare) = &charge.glare
            && charge.amount(held) >= charge.glare_from
        {
            spawner.glare(glare, gun.muzzle, &mut rng);
        }
        let Some(recipe) = &charge.light else {
            continue;
        };
        let at = gun.muzzle + Vec3::Y * recipe.above;
        let looks = point_light(recipe.radius * share, recipe.colour);
        let entity = match lit.0.remove(&hero.player) {
            Some(entity) => {
                if let Ok((mut transform, mut light)) = lights.get_mut(entity) {
                    transform.translation = at;
                    *light = looks;
                }
                entity
            }
            None if free > 0 => {
                free -= 1;
                spawner
                    .commands
                    .spawn((
                        Name::new("charge light"),
                        DemoEntity,
                        ChargeLit,
                        NotShadowCaster,
                        looks,
                        Transform::from_translation(at),
                    ))
                    .id()
            }
            None => continue,
        };
        shown.insert(hero.player, entity);
    }
    for (_, entity) in std::mem::replace(&mut lit.0, shown) {
        spawner.commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crackle(reach: Reach) -> Crackle {
        Crackle {
            count: 2,
            points: 20,
            after: 4,
            every: 2,
            bow: (1.0, 4.0),
            reach,
            strands: Vec::new(),
        }
    }

    fn gun() -> Gun {
        Gun {
            muzzle: Vec3::new(0.2, 1.0, -0.6),
            facing: Vec3::NEG_Z,
            feet: Vec3::ZERO,
        }
    }

    #[test]
    fn a_crackle_into_the_gun_loops_out_from_the_muzzle_and_back_into_it() {
        // The Hydra's (0x31FB90): its point 1 is 0.72 behind the muzzle and 0.16 off it.
        let reach = Reach::Gun {
            back: 0.72,
            up: 0.16,
            past: (1.0, 4.0),
        };
        let gun = gun();
        let mut rng = FxRng(7);
        for _ in 0..50 {
            let mut laid = Laid::default();
            crackle(reach).lay(&mut laid, &gun, 0.35, None, &mut rng);
            let points = &laid.points;
            assert_eq!(points.len(), 20);
            assert_eq!(points[0], gun.muzzle);
            let into = gun.muzzle + Vec3::new(0.0, 0.16, 0.72);
            assert!(points[19].distance(into) < 1e-4);
            // It bows out across his facing, at most about 4/27 of its bow (0x4420D0's
            // t (1 − t)² at a third of the way).
            let most = points
                .iter()
                .map(|point| (*point - gun.muzzle).with_z(0.0).length())
                .fold(0.0, f32::max);
            assert!(most > 0.1 && most < 0.85, "{most}");
        }
    }

    #[test]
    fn a_crackle_out_to_the_world_holds_its_end_for_its_time_then_picks_another() {
        let reach = Reach::World {
            reach: 3.0,
            spread: 70.0,
            keep: (15, 25),
            past: (1.0, 4.0),
        };
        let gun = gun();
        let mut rng = FxRng(11);
        let mut laid = Laid::default();
        let crackle = crackle(reach);
        // The first lay only picks its end, on the floor 3 from his feet within 70°.
        crackle.lay(&mut laid, &gun, 0.2, None, &mut rng);
        assert!(laid.points.is_empty());
        let (end, keep) = laid.end.unwrap();
        assert!((15..=25).contains(&keep));
        assert!((end.length() - 3.0).abs() < 1e-4 && end.y == 0.0);
        assert!(end.normalize().dot(Vec3::NEG_Z) >= 70f32.to_radians().cos() - 1e-4);
        for _ in 1..keep {
            crackle.lay(&mut laid, &gun, 0.2, None, &mut rng);
            assert_eq!(laid.end.unwrap().0, end);
            assert!(laid.points[19].distance(end) < 1e-4);
        }
        crackle.lay(&mut laid, &gun, 0.2, None, &mut rng);
        assert_ne!(laid.end.unwrap().0, end);
    }

    #[test]
    fn the_hydras_charge_grows_to_its_most() {
        let charge = Charge {
            growth: 0.007,
            most: 0.7,
            alpha_per_tick: 1.0 / 256.0,
            crackles: Vec::new(),
            glow: Vec::new(),
            light: None,
            glare: None,
            glare_from: 0.45,
        };
        assert!((charge.amount(50) - 0.35).abs() < 1e-6);
        assert_eq!(charge.amount(150), 0.7);
        assert!((charge.share(50) - 0.5).abs() < 1e-6);
        // Its glare from 0.45: 65 ticks held (0.007 × 64 falls just short).
        assert!(charge.amount(64) < charge.glare_from);
        assert!(charge.amount(65) >= charge.glare_from);
    }

    #[test]
    fn the_hydra_glares_from_a_charge_of_0_45_at_every_level_and_while_its_spit_lasts() {
        let recipes = FxRecipes::default();
        for level in 0..8 {
            let charge = recipes.charge("hydra", level).unwrap();
            let glare = charge.glare.as_ref().unwrap();
            assert_eq!(charge.glare_from, 0.45);
            assert_eq!((glare.length, glare.ticks), (1.35, 15));
            let spit = recipes.glare("hydra", level, Some("spit")).unwrap();
            assert_eq!(spit.until, Some(30));
            // Not on its bolts, the shared lightning.
            assert!(recipes.glare("hydra", level, Some("bolt")).is_none());
        }
    }
}

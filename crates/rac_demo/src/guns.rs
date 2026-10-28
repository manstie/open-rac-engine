//! The hero's guns in the demo (P1.9): UYA's twenty at V3, and how their shots look in
//! flight: a sprite where the gun's effects name one ([`crate::fx`]), else a coloured
//! shape. Their flames, rays, arcs and impacts are [`crate::fx`]'s; the HUD shows what is in
//! his hand ([`crate::hud`]).

use std::collections::BTreeMap;

use bevy::ecs::system::SystemParam;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rac_combat::{
    Armory, Arsenal,
    weapons::{FireMode, ShotKind},
};
use rac_hero::Hero;
use rac_sim::SimTransform;

use crate::DemoEntity;
use crate::fx::{Billboard, FxImages, FxQuad, FxRecipes, FxRng, ShotLayer, ShotStreak};

/// The level the demo gives every gun: V3, where most have lock-on.
const DEMO_LEVEL: u8 = 2;

/// Every gun of the armory at V3, with the first one brought out by the fire button.
pub fn demo_arsenal(armory: &Armory) -> Arsenal {
    let mut arsenal = Arsenal::default();
    for index in 0..armory.weapons.len() {
        arsenal.give_at(rac_combat::WeaponId(index), DEMO_LEVEL, armory);
    }
    arsenal
}

/// The look of one shot in flight: which hero's, its number, and whether it was made for
/// the shot [`standing`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShotLook {
    player: usize,
    serial: u32,
    standing: bool,
}

/// Whether a shot stands where it landed: a holo shield flies as its recipe's sprites and
/// stands as the kit's wall.
fn standing(kind: &ShotKind) -> bool {
    matches!(kind, ShotKind::Barrier(state) if state.landed.is_some())
}

/// Meshes and materials for the guns' effects, made once.
#[derive(Resource)]
pub(crate) struct GunLooks {
    ball: Handle<Mesh>,
    beam: Handle<Mesh>,
    bullet: Handle<StandardMaterial>,
    sniper: Handle<StandardMaterial>,
    charge: Handle<StandardMaterial>,
    blast: Handle<StandardMaterial>,
    lava: Handle<StandardMaterial>,
    missile: Handle<StandardMaterial>,
    arc: Handle<StandardMaterial>,
    plasma: Handle<StandardMaterial>,
    bomb: Handle<StandardMaterial>,
    blob: Handle<StandardMaterial>,
    disc: Handle<StandardMaterial>,
    rift: Handle<StandardMaterial>,
    thing: Handle<StandardMaterial>,
    turret: Handle<StandardMaterial>,
    holo: Handle<StandardMaterial>,
    agent: Handle<StandardMaterial>,
    duck: Handle<StandardMaterial>,
    egg: Handle<StandardMaterial>,
}

impl GunLooks {
    pub(crate) fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let ball = meshes.add(Sphere::new(1.0));
        // A unit cube, stretched along its −z into a beam.
        let beam = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
        let mut glow = |r: f32, g: f32, b: f32, alpha: f32| {
            materials.add(StandardMaterial {
                base_color: Color::srgba(r, g, b, alpha),
                emissive: LinearRgba::rgb(r * 4.0, g * 4.0, b * 4.0),
                alpha_mode: if alpha < 1.0 {
                    AlphaMode::Blend
                } else {
                    AlphaMode::Opaque
                },
                unlit: true,
                ..default()
            })
        };
        Self {
            ball,
            beam,
            bullet: glow(1.0, 0.85, 0.3, 1.0),
            sniper: glow(0.6, 0.4, 1.0, 1.0),
            charge: glow(1.0, 0.45, 0.1, 1.0),
            blast: glow(1.0, 0.55, 0.15, 0.35),
            lava: glow(1.0, 0.3, 0.05, 1.0),
            missile: glow(0.95, 0.95, 0.9, 1.0),
            arc: glow(0.6, 0.9, 1.0, 0.9),
            plasma: glow(0.5, 0.55, 1.0, 1.0),
            bomb: glow(0.95, 0.65, 0.2, 1.0),
            blob: glow(0.45, 0.9, 0.3, 1.0),
            disc: glow(0.85, 0.9, 0.95, 1.0),
            rift: glow(0.35, 0.1, 0.55, 0.85),
            thing: glow(0.75, 0.6, 0.4, 1.0),
            turret: glow(0.55, 0.6, 0.65, 1.0),
            holo: glow(0.35, 0.9, 0.95, 0.35),
            agent: glow(0.9, 0.3, 0.3, 1.0),
            duck: glow(1.0, 0.85, 0.2, 1.0),
            egg: glow(0.95, 0.95, 0.85, 1.0),
        }
    }
}

/// How a shot looks: a ball of a size in a material, or for a holo shield a wall.
struct Look {
    mesh: Handle<Mesh>,
    scale: Vec3,
    material: Handle<StandardMaterial>,
    rotation: Quat,
}

fn shot_look(kind: &ShotKind, velocity: Vec3, fire: Option<&FireMode>, looks: &GunLooks) -> Look {
    let speed = velocity.length();
    let ball = |size: f32, material: &Handle<StandardMaterial>| Look {
        mesh: looks.ball.clone(),
        scale: Vec3::splat(size),
        material: material.clone(),
        rotation: Quat::IDENTITY,
    };
    match kind {
        // The Flux Rifle's shot is far faster than the N60's bullets.
        ShotKind::Bullet { .. } if speed > 200.0 => ball(0.1, &looks.sniper),
        ShotKind::Bullet { .. } => ball(0.07, &looks.bullet),
        ShotKind::Charge { .. } => ball(0.2, &looks.charge),
        ShotKind::Blast { radius, .. } => ball(*radius, &looks.blast),
        ShotKind::Stream => ball(0.12, &looks.lava),
        ShotKind::Missile { .. } | ShotKind::Rocket(_) => ball(0.14, &looks.missile),
        ShotKind::Ball { .. } => ball(0.3, &looks.plasma),
        ShotKind::Storm { .. } => ball(0.5, &looks.plasma),
        ShotKind::Arc { .. } => ball(0.05, &looks.arc),
        ShotKind::Canister { .. } => ball(0.25, &looks.bomb),
        ShotKind::Bomblet { .. } => ball(0.1, &looks.bomb),
        ShotKind::Blob { .. } => ball(0.15, &looks.blob),
        ShotKind::Bomb => ball(0.3, &looks.blob),
        ShotKind::Disc { .. } => Look {
            scale: Vec3::new(0.35, 0.05, 0.35),
            ..ball(0.0, &looks.disc)
        },
        // The spit and the bolts are drawn as their arcs, the Flux Rifle's beam as its
        // ray ([`crate::fx`]).
        ShotKind::Spit { .. } | ShotKind::Bolt { .. } | ShotKind::Beam { .. } => Look {
            scale: Vec3::ZERO,
            ..ball(0.0, &looks.arc)
        },
        ShotKind::Rift(_) => ball(0.3, &looks.rift),
        ShotKind::Thing { .. } => ball(0.3, &looks.thing),
        ShotKind::Turret(_) => ball(0.35, &looks.turret),
        ShotKind::Pellet => ball(0.06, &looks.bullet),
        ShotKind::Guided { .. } => ball(0.1, &looks.missile),
        ShotKind::Seeker { .. } => ball(0.08, &looks.missile),
        // A wall standing up from where it landed, as wide and high as the kit's.
        ShotKind::Barrier(state) => {
            let (half_width, height) = match fire {
                Some(FireMode::Barrier(fire)) => (fire.half_width, fire.height),
                _ => (1.0, 1.0),
            };
            Look {
                mesh: looks.beam.clone(),
                scale: Vec3::new(half_width * 2.0, height * 2.0, 0.1),
                material: looks.holo.clone(),
                rotation: Quat::from_rotation_y(state.yaw),
            }
        }
        ShotKind::Pod => ball(0.3, &looks.agent),
        ShotKind::Agent(_) => ball(0.3, &looks.agent),
        ShotKind::Slug => ball(0.05, &looks.bullet),
        ShotKind::Comet { .. } => ball(0.2, &looks.charge),
        ShotKind::Wave => ball(0.5, &looks.arc),
        ShotKind::Pulse { .. } => ball(0.2, &looks.holo),
        ShotKind::Duck(_) => ball(0.35, &looks.duck),
        ShotKind::Egg { .. } => ball(0.12, &looks.egg),
        // A blade of light across its way, as wide as what it cuts.
        ShotKind::Cut { .. } => {
            let width = match fire {
                Some(FireMode::Whip(fire)) => fire.cut.map_or(3.0, |cut| cut.width),
                _ => 3.0,
            };
            Look {
                mesh: looks.beam.clone(),
                scale: Vec3::new(width, 0.3, 0.1),
                material: looks.arc.clone(),
                rotation: Quat::from_rotation_y(ops::atan2(-velocity.x, -velocity.z)),
            }
        }
    }
}

/// A game's model for a gun's shots in flight, in place of the kit's shape: its parts,
/// facing the way the shot flies along their −z.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotModel {
    pub parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

/// The shot models a game gives, by gun key. They are drawn for a gun's own shots (a
/// charge, missile, rocket, disc, canister, turret, rift, blob or ball), not for the
/// blasts and bomblets they become.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct ShotModels(pub BTreeMap<String, ShotModel>);

/// Whether a shot of this kind is drawn as its gun's model, when there is one.
fn modelled(kind: &ShotKind) -> bool {
    matches!(
        kind,
        ShotKind::Charge { .. }
            | ShotKind::Missile { .. }
            | ShotKind::Rocket(_)
            | ShotKind::Disc { .. }
            | ShotKind::Canister { .. }
            | ShotKind::Turret(_)
            | ShotKind::Rift(_)
            | ShotKind::Blob { .. }
            | ShotKind::Ball { .. }
    )
}

/// A shot drawn as a model, turned to its way each tick.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Facing;

/// What a shot can be drawn as.
#[derive(SystemParam)]
pub(crate) struct ShotDraw<'w> {
    looks: Res<'w, GunLooks>,
    armory: Res<'w, Armory>,
    recipes: Res<'w, FxRecipes>,
    images: Res<'w, FxImages>,
    quad: Option<Res<'w, FxQuad>>,
    models: Res<'w, ShotModels>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    rng: ResMut<'w, FxRng>,
}

/// The way a shot flies, as a turn of a model's −z, or none while it stands still.
fn way(velocity: Vec3) -> Option<Quat> {
    let way = velocity.try_normalize()?;
    let up = if way.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
    Some(Transform::IDENTITY.looking_to(way, up).rotation)
}

/// Keeps a look for every shot in flight, following it in the simulation so it is drawn
/// between ticks, and drops the looks of shots that ended.
pub(crate) fn show_shots(
    mut draw: ShotDraw,
    heroes: Query<(&Hero, &Arsenal)>,
    mut shown: Query<(
        Entity,
        &ShotLook,
        &mut SimTransform,
        &mut Transform,
        Has<Facing>,
    )>,
    mut commands: Commands,
) {
    let mut seen = Vec::new();
    for (entity, look, mut simulated, mut transform, facing) in &mut shown {
        let shot = heroes
            .iter()
            .filter(|(hero, _)| hero.player == look.player)
            .find_map(|(_, arsenal)| arsenal.shots.iter().find(|shot| shot.serial == look.serial));
        let Some(shot) = shot.filter(|shot| standing(&shot.kind) == look.standing) else {
            commands.entity(entity).despawn();
            continue;
        };
        simulated.translation = shot.position;
        seen.push(*look);
        if facing {
            if let Some(turn) = way(shot.velocity) {
                simulated.rotation = turn;
            }
            continue;
        }
        // Interpolation leaves the scale alone, so a blast grows through it, and a rift
        // opens.
        match shot.kind {
            ShotKind::Blast { radius, .. } => transform.scale = Vec3::splat(radius),
            ShotKind::Rift(state) if state.open.is_some() => {
                transform.scale = Vec3::splat(0.9);
            }
            _ => {}
        }
    }
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let look = ShotLook {
                player: hero.player,
                serial: shot.serial,
                standing: standing(&shot.kind),
            };
            if seen.contains(&look) {
                continue;
            }
            let def = draw.armory.get(shot.weapon);
            let key = def.map_or("", |def| def.effects.as_str());
            if let Some(model) = draw.models.0.get(key).filter(|_| modelled(&shot.kind)) {
                let turn = way(shot.velocity).unwrap_or_default();
                commands
                    .spawn((
                        Name::new("shot"),
                        DemoEntity,
                        look,
                        Facing,
                        Visibility::default(),
                        SimTransform::from_translation(shot.position).with_rotation(turn),
                        Transform::from_translation(shot.position).with_rotation(turn),
                    ))
                    .with_children(|parts| {
                        for (mesh, material) in &model.parts {
                            parts.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
                        }
                    });
                continue;
            }
            let name = Some(shot.kind.name());
            let (sprites, streak) = if look.standing {
                (&[][..], None)
            } else {
                (
                    draw.recipes.shot(key, shot.level, name),
                    draw.recipes.streak(key, shot.level, name),
                )
            };
            if let Some(quad) = draw
                .quad
                .as_ref()
                .filter(|_| !sprites.is_empty() || streak.is_some())
            {
                let quad = quad.0.clone();
                let material =
                    |materials: &mut Assets<StandardMaterial>, texture, colour, blend| {
                        let [r, g, b, a] = colour;
                        materials.add(StandardMaterial {
                            base_color: Color::srgba(r, g, b, a),
                            base_color_texture: draw.images.0.get(&texture).cloned(),
                            alpha_mode: crate::fx::Blend::alpha_mode(blend),
                            unlit: true,
                            double_sided: true,
                            cull_mode: None,
                            ..default()
                        })
                    };
                let layers: Vec<_> = sprites
                    .iter()
                    .map(|sprite| {
                        let handle = material(
                            &mut draw.materials,
                            sprite.texture,
                            sprite.colour,
                            sprite.blend,
                        );
                        let layer = ShotLayer {
                            spin: sprite.spin.to_radians(),
                            flickers: sprite.flickers,
                        };
                        let roll = draw.rng.roll(Some((0.0, 360.0)));
                        (handle, layer, roll, sprite.size)
                    })
                    .collect();
                let streak = streak.map(|streak| {
                    let handle = material(
                        &mut draw.materials,
                        streak.texture,
                        streak.colour,
                        streak.blend,
                    );
                    (handle, streak.clone())
                });
                commands
                    .spawn((
                        Name::new("shot"),
                        DemoEntity,
                        look,
                        Visibility::default(),
                        SimTransform::from_translation(shot.position),
                        Transform::from_translation(shot.position),
                    ))
                    .with_children(|parts| {
                        for (handle, layer, roll, size) in layers {
                            parts.spawn((
                                Mesh3d(quad.clone()),
                                MeshMaterial3d(handle),
                                NotShadowCaster,
                                Billboard { axis: None, roll },
                                layer,
                                Transform::from_scale(Vec3::splat(size)),
                            ));
                        }
                        if let Some((handle, streak)) = streak {
                            parts.spawn((
                                Mesh3d(quad.clone()),
                                MeshMaterial3d(handle),
                                NotShadowCaster,
                                Billboard::default(),
                                ShotStreak(streak),
                                Transform::default(),
                                Visibility::Hidden,
                            ));
                        }
                    });
                continue;
            }
            let fire = def.map(|def| &def.fire);
            let shown = shot_look(&shot.kind, shot.velocity, fire, &draw.looks);
            // A blast its recipe shows only through its impact, or a shot drawn as a
            // ribbon through it and its fellows, keeps a hidden look, so it is still
            // followed and dropped as the others are.
            let hidden = match shot.kind {
                ShotKind::Blast { .. } => !draw.recipes.blast_sphere(key, shot.level, name),
                // A standing shot drawn as its recipe's panels.
                _ if look.standing => draw.recipes.panels(key, shot.level, name).is_some(),
                _ => !draw.recipes.ribbon(key, shot.level, name).is_empty(),
            };
            let visibility = if hidden {
                Visibility::Hidden
            } else {
                Visibility::default()
            };
            commands.spawn((
                Name::new("shot"),
                DemoEntity,
                look,
                Mesh3d(shown.mesh),
                MeshMaterial3d(shown.material),
                NotShadowCaster,
                visibility,
                SimTransform::from_translation(shot.position).with_rotation(shown.rotation),
                Transform::from_translation(shot.position)
                    .with_rotation(shown.rotation)
                    .with_scale(shown.scale),
            ));
        }
    }
}

/// Turns each shot's sprites a tick, and lays its streak along its way once it has flown
/// far enough.
pub(crate) fn step_shot_layers(
    heroes: Query<(&Hero, &Arsenal)>,
    looks: Query<&ShotLook>,
    mut layers: Query<(&ShotLayer, &mut Billboard), Without<ShotStreak>>,
    mut streaks: Query<
        (
            &ChildOf,
            &ShotStreak,
            &mut Billboard,
            &mut Transform,
            &mut Visibility,
        ),
        Without<ShotLayer>,
    >,
    mut rng: ResMut<FxRng>,
) {
    for (layer, mut billboard) in &mut layers {
        billboard.roll = if layer.flickers {
            rng.roll(Some((0.0, 360.0)))
        } else {
            billboard.roll + layer.spin
        };
    }
    for (parent, ShotStreak(streak), mut billboard, mut transform, mut visibility) in &mut streaks {
        let shot = looks.get(parent.parent()).ok().and_then(|look| {
            heroes
                .iter()
                .filter(|(hero, _)| hero.player == look.player)
                .find_map(|(_, arsenal)| {
                    arsenal.shots.iter().find(|shot| shot.serial == look.serial)
                })
        });
        let Some((shot, way)) = shot.and_then(|shot| Some((shot, shot.velocity.try_normalize()?)))
        else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let flown = shot.velocity.length() * shot.ticks as f32 / 60.0;
        if flown < streak.after {
            *visibility = Visibility::Hidden;
            continue;
        }
        let length = rng.between(streak.length);
        let width = rng.between(streak.width);
        *visibility = Visibility::Inherited;
        billboard.axis = Some(way);
        transform.translation = way * (length / 2.0 - streak.behind);
        transform.scale = Vec3::new(length, width, 1.0);
    }
}

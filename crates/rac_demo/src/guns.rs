//! The hero's guns in the demo (P1.9): UYA's twenty at V3, and how their shots, rays, arcs
//! and impacts look. The HUD shows what is in his hand ([`crate::hud`]).

use bevy::prelude::*;
use rac_combat::{
    Armory, Arsenal, GunEvent, GunEventKind,
    weapons::{FireMode, ShotKind},
};
use rac_hero::Hero;
use rac_sim::SimTransform;

use crate::DemoEntity;

/// The level the demo gives every gun: V3, where most have lock-on.
const DEMO_LEVEL: u8 = 2;

/// How long a ray or arc stays on screen.
const BEAM_TICKS: u32 = 10;

/// Every gun of the armory at V3, with the first one brought out by the fire button.
pub fn demo_arsenal(armory: &Armory) -> Arsenal {
    let mut arsenal = Arsenal::default();
    for index in 0..armory.weapons.len() {
        arsenal.give_at(rac_combat::WeaponId(index), DEMO_LEVEL, armory);
    }
    arsenal
}

/// The look of one shot in flight: which hero's, and its number.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShotLook {
    player: usize,
    serial: u32,
}

/// A ray or arc, or the flash of an impact, fading out.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Fading {
    ticks_left: u32,
    total: u32,
    /// Its scale when new.
    scale: Vec3,
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
    ray: Handle<StandardMaterial>,
    arc: Handle<StandardMaterial>,
    flash: Handle<StandardMaterial>,
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
            ray: glow(0.45, 0.8, 1.0, 0.8),
            arc: glow(0.6, 0.9, 1.0, 0.9),
            flash: glow(1.0, 0.9, 0.6, 0.9),
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
        // ray ([`show_gun_events`]).
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

/// Keeps a look for every shot in flight, following it in the simulation so it is drawn
/// between ticks, and drops the looks of shots that ended.
pub(crate) fn show_shots(
    looks: Res<GunLooks>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal)>,
    mut shown: Query<(Entity, &ShotLook, &mut SimTransform, &mut Transform)>,
    mut commands: Commands,
) {
    let mut seen = Vec::new();
    for (entity, look, mut simulated, mut transform) in &mut shown {
        let shot = heroes
            .iter()
            .filter(|(hero, _)| hero.player == look.player)
            .find_map(|(_, arsenal)| arsenal.shots.iter().find(|shot| shot.serial == look.serial));
        match shot {
            Some(shot) => {
                simulated.translation = shot.position;
                // Interpolation leaves the scale alone, so a blast grows through it, and a
                // rift opens.
                match shot.kind {
                    ShotKind::Blast { radius, .. } => transform.scale = Vec3::splat(radius),
                    ShotKind::Rift(state) if state.open.is_some() => {
                        transform.scale = Vec3::splat(0.9);
                    }
                    _ => {}
                }
                seen.push(*look);
            }
            None => commands.entity(entity).despawn(),
        }
    }
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let look = ShotLook {
                player: hero.player,
                serial: shot.serial,
            };
            if seen.contains(&look) {
                continue;
            }
            let fire = armory.get(shot.weapon).map(|def| &def.fire);
            let shown = shot_look(&shot.kind, shot.velocity, fire, &looks);
            commands.spawn((
                Name::new("shot"),
                DemoEntity,
                look,
                Mesh3d(shown.mesh),
                MeshMaterial3d(shown.material),
                SimTransform::from_translation(shot.position).with_rotation(shown.rotation),
                Transform::from_translation(shot.position)
                    .with_rotation(shown.rotation)
                    .with_scale(shown.scale),
            ));
        }
    }
}

/// Rays, arcs and impact flashes from the guns' events.
pub(crate) fn show_gun_events(
    looks: Res<GunLooks>,
    mut events: MessageReader<GunEvent>,
    mut commands: Commands,
) {
    for event in events.read() {
        let (from, to, material, width) = match event.kind {
            GunEventKind::Ray { from, to } => (from, to, looks.ray.clone(), 0.025),
            GunEventKind::Arc { from, to } => (from, to, looks.arc.clone(), 0.06),
            GunEventKind::Impact { at } => {
                let scale = Vec3::splat(0.25);
                commands.spawn((
                    Name::new("impact"),
                    DemoEntity,
                    Fading {
                        ticks_left: BEAM_TICKS,
                        total: BEAM_TICKS,
                        scale,
                    },
                    Mesh3d(looks.ball.clone()),
                    MeshMaterial3d(looks.flash.clone()),
                    Transform::from_translation(at).with_scale(scale),
                ));
                continue;
            }
            GunEventKind::Fired { .. } | GunEventKind::LevelUp { .. } => continue,
        };
        let length = from.distance(to);
        if length < 1e-3 {
            continue;
        }
        let scale = Vec3::new(width, width, length);
        commands.spawn((
            Name::new("beam"),
            DemoEntity,
            Fading {
                ticks_left: BEAM_TICKS,
                total: BEAM_TICKS,
                scale,
            },
            Mesh3d(looks.beam.clone()),
            MeshMaterial3d(material),
            Transform::from_translation((from + to) / 2.0)
                .looking_at(to, Vec3::Y)
                .with_scale(scale),
        ));
    }
}

/// Thins the beams and shrinks the flashes until they go.
pub(crate) fn fade(
    mut fading: Query<(Entity, &mut Fading, &mut Transform)>,
    mut commands: Commands,
) {
    for (entity, mut fade, mut transform) in &mut fading {
        fade.ticks_left = fade.ticks_left.saturating_sub(1);
        if fade.ticks_left == 0 {
            commands.entity(entity).despawn();
            continue;
        }
        let left = fade.ticks_left as f32 / fade.total as f32;
        transform.scale = Vec3::new(fade.scale.x * left, fade.scale.y * left, fade.scale.z);
    }
}

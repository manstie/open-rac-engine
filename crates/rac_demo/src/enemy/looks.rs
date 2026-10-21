//! How the enemies and their shots look: placeholder shapes made in code (their drops are
//! shown with every pickup, in `crate::drops`).
//! Each enemy's parts lean and sink with what it is doing, and the parts that give its
//! attack away light up through the wind-up.

use bevy::prelude::*;
use rac_combat::HitReaction;
use std::f32::consts::FRAC_PI_2;

use super::brain::{AttackPhase, EnemyMotion, EnemyState, phases};
use super::def::{Attack, Bestiary, EnemyDef, EnemyKind};
use super::{BURST_TICKS, Enemy, EnemyBody, EnemyShot, ShotBurst};

/// Meshes and materials for everything here, made when the demo spawns.
#[derive(Resource, Debug, Clone)]
pub struct EnemyLooks {
    /// What every part shows while its enemy flashes from a hit.
    lit: Handle<StandardMaterial>,
    charger: Vec<PartLook>,
    shooter: Vec<PartLook>,
    shot: Handle<Mesh>,
    burst: Handle<Mesh>,
    shot_look: Handle<StandardMaterial>,
}

#[derive(Debug, Clone)]
struct PartLook {
    mesh: Handle<Mesh>,
    normal: Handle<StandardMaterial>,
    /// What it shows through the wind-up, for the parts that give the attack away.
    tell: Option<Handle<StandardMaterial>>,
    transform: Transform,
}

/// One piece of an enemy's model.
#[derive(Component, Debug, Clone)]
pub(crate) struct EnemyPart {
    owner: Entity,
    normal: Handle<StandardMaterial>,
    tell: Option<Handle<StandardMaterial>>,
}

/// The pivot at an enemy's feet that its parts lean and sink with.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct EnemyPose {
    owner: Entity,
}

impl EnemyLooks {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let mut plain = |r: f32, g: f32, b: f32| materials.add(Color::srgb(r, g, b));
        let rust = plain(0.55, 0.2, 0.12);
        let bone = plain(0.9, 0.85, 0.7);
        let visor = plain(0.95, 0.8, 0.2);
        let violet = plain(0.32, 0.25, 0.55);
        let eye = plain(0.3, 0.9, 0.95);
        let gunmetal = plain(0.35, 0.35, 0.4);
        let mut glowing = |base: Color, emissive: LinearRgba| {
            materials.add(StandardMaterial {
                base_color: base,
                emissive,
                ..default()
            })
        };
        let lit = glowing(Color::srgb(1.0, 0.95, 0.8), LinearRgba::rgb(2.0, 1.6, 0.8));
        let tell = glowing(Color::srgb(1.0, 0.2, 0.05), LinearRgba::rgb(8.0, 0.8, 0.1));
        let shot_look = glowing(Color::srgb(1.0, 0.5, 0.1), LinearRgba::rgb(8.0, 2.5, 0.4));

        // Forward is −z; a cone or cylinder along +y turned this way points forward.
        let forward = Quat::from_rotation_x(-FRAC_PI_2);
        let part = |mesh: Handle<Mesh>, normal: &Handle<StandardMaterial>, at: Vec3| PartLook {
            mesh,
            normal: normal.clone(),
            tell: None,
            transform: Transform::from_translation(at),
        };
        let horn = meshes.add(Cone::new(0.09, 0.45));
        let charger = vec![
            part(
                meshes.add(Capsule3d::new(0.5, 0.3)),
                &rust,
                Vec3::new(0.0, 0.65, 0.0),
            ),
            PartLook {
                transform: Transform::from_xyz(-0.28, 0.95, -0.3).with_rotation(forward),
                ..part(horn.clone(), &bone, Vec3::ZERO)
            },
            PartLook {
                transform: Transform::from_xyz(0.28, 0.95, -0.3).with_rotation(forward),
                ..part(horn, &bone, Vec3::ZERO)
            },
            PartLook {
                tell: Some(tell.clone()),
                ..part(
                    meshes.add(Cuboid::new(0.55, 0.12, 0.1)),
                    &visor,
                    Vec3::new(0.0, 0.85, -0.46),
                )
            },
        ];
        let shooter = vec![
            part(
                meshes.add(Capsule3d::new(0.4, 0.9)),
                &violet,
                Vec3::new(0.0, 0.85, 0.0),
            ),
            PartLook {
                tell: Some(tell.clone()),
                ..part(
                    meshes.add(Sphere::new(0.13)),
                    &eye,
                    Vec3::new(0.0, 1.45, -0.32),
                )
            },
            PartLook {
                transform: Transform::from_xyz(0.0, 1.15, -0.4).with_rotation(forward),
                ..part(meshes.add(Cylinder::new(0.09, 0.7)), &gunmetal, Vec3::ZERO)
            },
            PartLook {
                tell: Some(tell),
                ..part(
                    meshes.add(Sphere::new(0.1)),
                    &gunmetal,
                    Vec3::new(0.0, 1.15, -0.76),
                )
            },
        ];
        Self {
            lit,
            charger,
            shooter,
            shot: meshes.add(Sphere::new(0.2)),
            burst: meshes.add(Sphere::new(0.45)),
            shot_look,
        }
    }
}

/// Gives new enemies, shots and bursts their looks.
pub(crate) fn dress(
    looks: Res<EnemyLooks>,
    enemies: Query<(Entity, &Enemy), Added<Enemy>>,
    shots: Query<Entity, Added<EnemyShot>>,
    bursts: Query<Entity, Added<ShotBurst>>,
    mut commands: Commands,
) {
    for (owner, enemy) in &enemies {
        commands.entity(owner).insert(Visibility::default());
        let pose = commands
            .spawn((
                EnemyPose { owner },
                Transform::default(),
                Visibility::default(),
                ChildOf(owner),
            ))
            .id();
        let parts = match enemy.kind {
            EnemyKind::Charger => &looks.charger,
            EnemyKind::Shooter => &looks.shooter,
        };
        for part in parts {
            commands.spawn((
                EnemyPart {
                    owner,
                    normal: part.normal.clone(),
                    tell: part.tell.clone(),
                },
                Mesh3d(part.mesh.clone()),
                MeshMaterial3d(part.normal.clone()),
                part.transform,
                ChildOf(pose),
            ));
        }
    }
    for shot in &shots {
        commands.entity(shot).insert((
            Mesh3d(looks.shot.clone()),
            MeshMaterial3d(looks.shot_look.clone()),
            Visibility::default(),
        ));
    }
    for burst in &bursts {
        commands.entity(burst).insert((
            Mesh3d(looks.burst.clone()),
            MeshMaterial3d(looks.shot_look.clone()),
            Visibility::default(),
        ));
    }
}

/// Leans, sinks, lights and hides each enemy's parts by what it is doing.
pub(crate) fn pose_enemies(
    looks: Res<EnemyLooks>,
    bestiary: Res<Bestiary>,
    mut enemies: Query<(&Enemy, &EnemyBody, Option<&HitReaction>, &mut Visibility)>,
    mut poses: Query<(&EnemyPose, &mut Transform)>,
    mut parts: Query<(&EnemyPart, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (enemy, body, _, mut visibility) in &mut enemies {
        let def = bestiary.get(enemy.kind);
        visibility.set_if_neq(if body.0.is_gone(def) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    for (pose, mut transform) in &mut poses {
        let Ok((enemy, body, ..)) = enemies.get(pose.owner) else {
            continue;
        };
        let (lean, sink) = lean(bestiary.get(enemy.kind), &body.0);
        // About x at the feet: a positive lean tips the top back, away from −z.
        transform.rotation = Quat::from_rotation_x(lean);
        transform.translation = Vec3::NEG_Y * sink;
    }
    for (part, mut material) in &mut parts {
        let Ok((enemy, body, reaction, _)) = enemies.get(part.owner) else {
            continue;
        };
        let def = bestiary.get(enemy.kind);
        let wanted = if reaction.is_some_and(|reaction| reaction.flash_ticks > 0) {
            &looks.lit
        } else if body.0.attack_phase(def) == Some(AttackPhase::WindUp)
            && let Some(tell) = &part.tell
        {
            tell
        } else {
            &part.normal
        };
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
    }
}

/// How far an enemy leans back (radians; forward is negative) and sinks into the ground.
fn lean(def: &EnemyDef, enemy: &EnemyMotion) -> (f32, f32) {
    let ticks = enemy.ticks as f32;
    let share = |of: u32| (ticks / of.max(1) as f32).min(1.0);
    match enemy.state {
        EnemyState::Hurt => (0.3 * (1.0 - share(def.hurt_ticks)), 0.0),
        // Falls back over a quarter of a second, then sinks.
        EnemyState::Dead => (1.5 * share(15), 0.4 * share(def.death_ticks)),
        EnemyState::Attack => {
            let (wind_up, strike, recover) = phases(def);
            let phase = enemy.attack_phase(def);
            match (def.attack, phase) {
                (Attack::Lunge(_), Some(AttackPhase::WindUp)) => (0.35 * share(wind_up), 0.0),
                (Attack::Lunge(_), Some(AttackPhase::Strike)) => (-0.4, 0.0),
                (Attack::Lunge(_), _) => {
                    let into = (ticks - (wind_up + strike) as f32) / recover.max(1) as f32;
                    (-0.4 * (1.0 - into.clamp(0.0, 1.0)), 0.0)
                }
                (Attack::Shoot(_), Some(AttackPhase::WindUp)) => (0.1 * share(wind_up), 0.0),
                (Attack::Shoot(shoot), Some(AttackPhase::Strike)) => {
                    // Each shot kicks it back.
                    let every = shoot.shot_every.max(1);
                    let since = (enemy.ticks - wind_up) % every;
                    (0.2 * (1.0 - since as f32 / every as f32), 0.0)
                }
                (Attack::Shoot(_), _) => (0.0, 0.0),
            }
        }
        _ => (0.0, 0.0),
    }
}

/// Shrinks shot bursts.
pub(crate) fn shrink_bursts(mut bursts: Query<(&ShotBurst, &mut Transform)>) {
    for (burst, mut transform) in &mut bursts {
        transform.scale = Vec3::splat(burst.ticks_left as f32 / BURST_TICKS as f32);
    }
}

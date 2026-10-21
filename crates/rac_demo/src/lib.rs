//! The hero kit's standalone demo (PLAN.md Phase 1): the greybox test level (P1.4) with
//! the hero (P1.5) and his camera, or a spectator camera to look round it with, and a
//! melee yard of crates and dummies to hit with the wrench (P1.8) and his guns (P1.9), and
//! to lock on to; and an arena of enemies (P1.11, [`enemy`]) that drop bolts and nanotech
//! ([`drops`]).
//!
//! The app decides when the demo exists: it runs [`spawn_demo`] when gameplay starts and
//! [`despawn_demo`] when it ends. [`DemoPlugin`] adds the demo's simulation systems, and
//! the hero's if the app hasn't.

mod guns;
mod level;
mod spectator;

use bevy::prelude::*;
use rac_combat::{
    Armory, CombatPlugin, Destroyed, HitReaction, Hitpoints, KillXp, Pushable, target_bundle,
};
use rac_hero::{
    CameraSetting, Checkpoint, Collider, FollowCamera, Hero, HeroBody, HeroPlugin, HeroTuning,
    LockOn, hero_bundle,
};
use rac_rig::{RigLook, RigPlugin};
use rac_sim::{SimSet, SimTick, SimTransform};

pub use guns::demo_arsenal;
pub use level::{
    CRATE_HALF, DUMMY_HEIGHT, DUMMY_RADIUS, GAP_WIDTHS, Mover, Piece, Shape, Surface, Target,
    TargetKind, greybox_level, melee_targets,
};
pub use spectator::SpectatorCamera;

/// Marks everything [`spawn_demo`] creates.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct DemoEntity;

/// A target's look, and how it looks when hit.
#[derive(Component, Debug, Clone)]
struct Flash {
    normal: Handle<StandardMaterial>,
    lit: Handle<StandardMaterial>,
}

/// A spark where the wrench clanked off something, shrinking away.
#[derive(Component, Debug, Clone, Copy)]
struct Spark {
    ticks_left: u32,
}

/// How long a clank's spark lasts.
const SPARK_TICKS: u32 = 10;

/// The ring shown round the target the hero is locked on to.
#[derive(Component, Debug, Default, Clone, Copy)]
struct LockMarker;

/// Moves the level's platforms and flies the spectator camera, both in the simulation.
pub struct DemoPlugin;

/// How the demo is set up when it spawns.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DemoSettings {
    /// Fly a free camera instead of playing the hero.
    pub spectator: bool,
    /// The hero camera's setting; `None` takes the movement profile's default (third
    /// person for UYA, lock-strafe for Deadlocked). Changing it changes the camera.
    pub camera: Option<CameraSetting>,
}

/// Where the hero starts, on the floor facing down the level.
const HERO_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 0.0);
/// Falling this far below the floor puts the hero back at the start.
const RESPAWN_BELOW: f32 = -20.0;
/// The XP a dummy knocked out gives the gun that did it [placeholder: UYA's per-class XP
/// isn't read].
const DUMMY_XP: u32 = 40;

impl Plugin for DemoPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<HeroPlugin>() {
            app.add_plugins(HeroPlugin);
        }
        if !app.is_plugin_added::<CombatPlugin>() {
            app.add_plugins(CombatPlugin);
        }
        if !app.is_plugin_added::<RigPlugin>() {
            app.add_plugins(RigPlugin);
        }
        app.init_resource::<DemoSettings>()
            .add_systems(FixedUpdate, move_platforms.in_set(SimSet::World))
            .add_systems(
                FixedUpdate,
                (break_targets, show_clanks).in_set(SimSet::Particles),
            )
            .add_systems(
                FixedUpdate,
                (guns::show_shots, guns::show_gun_events, guns::fade)
                    .in_set(SimSet::Particles)
                    .run_if(resource_exists::<guns::GunLooks>),
            )
            .add_systems(FixedUpdate, spectator::fly.in_set(SimSet::Camera))
            .add_systems(
                Update,
                (
                    flash_targets,
                    show_lock_on,
                    choose_camera.run_if(resource_changed::<DemoSettings>),
                    hide_hero_in_first_person,
                    guns::update_hud,
                ),
            );
    }
}

/// Spawns the greybox level, its lights and the spectator camera.
#[allow(clippy::too_many_arguments)]
pub fn spawn_demo(
    settings: Res<DemoSettings>,
    tuning: Res<HeroTuning>,
    armory: Res<Armory>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // A sky above, a warm horizon and dark ground for metal (the hero's gold) to reflect.
    let surroundings = EnvironmentMapLight::hemispherical_gradient(
        &mut images,
        Color::srgb(0.55, 0.7, 0.95),
        Color::srgb(0.9, 0.85, 0.75),
        Color::srgb(0.25, 0.22, 0.2),
    )
    .with_intensity(1500.0);
    let mut material_for = |surface: Surface, index: usize| {
        // Neighbouring pieces alternate shades so edges show without textures.
        let shade = if index.is_multiple_of(2) { 1.0 } else { 0.85 };
        let (r, g, b) = match surface {
            Surface::Ground => (0.55, 0.57, 0.6),
            Surface::Ledge => (0.85, 0.55, 0.25),
            Surface::WallJump => (0.3, 0.45, 0.8),
            Surface::Moving => (0.9, 0.8, 0.2),
            Surface::GrindRail => (0.75, 0.75, 0.8),
            Surface::ZipLine => (0.95, 0.55, 0.15),
            Surface::SwingTarget => (0.2, 0.85, 0.35),
            Surface::Hazard => (0.85, 0.15, 0.1),
            Surface::Checkpoint => (0.3, 0.9, 0.9),
            Surface::Water => (0.15, 0.4, 0.85),
        };
        // Water is see-through, so the hero shows under it.
        let (alpha, alpha_mode) = match surface {
            Surface::Water => (0.45, AlphaMode::Blend),
            _ => (1.0, AlphaMode::Opaque),
        };
        materials.add(StandardMaterial {
            base_color: Color::srgba(r * shade, g * shade, b * shade, alpha),
            alpha_mode,
            perceptual_roughness: 0.9,
            ..default()
        })
    };

    for (index, piece) in greybox_level().into_iter().enumerate() {
        let material = material_for(piece.surface, index);
        let rail = piece.rail();
        let water = piece.water();
        let (mesh, rotation) = match piece.shape {
            // A checkpoint shows as a pad on the ground at its spawn point.
            Shape::Box { half_extents } if piece.surface == Surface::Checkpoint => (
                meshes.add(Cuboid::new(
                    half_extents.x * 1.2,
                    0.04,
                    half_extents.z * 1.2,
                )),
                piece.rotation,
            ),
            Shape::Box { half_extents } => (
                meshes.add(Cuboid::from_size(half_extents * 2.0)),
                piece.rotation,
            ),
            Shape::Rail { start, end, radius } => (
                meshes.add(Cylinder::new(radius, start.distance(end))),
                // A cylinder's axis is +y; turn it to run from start to end.
                Quat::from_rotation_arc(Vec3::Y, (end - start).normalize()),
            ),
            Shape::Sphere { radius } => (meshes.add(Sphere::new(radius)), piece.rotation),
        };
        let mut entity = commands.spawn((
            Name::new(piece.name),
            DemoEntity,
            piece.surface,
            Mesh3d(mesh),
            MeshMaterial3d(material),
        ));
        if let Some(rail) = rail {
            entity.insert(rail);
        }
        if let Some(water) = water {
            entity.insert(water);
        }
        if let Shape::Box { half_extents } = piece.shape {
            match piece.surface {
                Surface::Checkpoint => {
                    entity.insert(Checkpoint {
                        half_extents,
                        spawn_yaw: 0.0,
                    });
                }
                surface if surface.is_solid() => {
                    entity.insert((Collider::Cuboid { half_extents }, surface.material()));
                }
                _ => {}
            }
        }
        match piece.mover {
            Some(mover) => {
                entity.insert((
                    mover,
                    SimTransform::from_translation(mover.position_at(0)).with_rotation(rotation),
                ));
            }
            None => {
                entity
                    .insert(Transform::from_translation(piece.translation).with_rotation(rotation));
            }
        }
    }

    let crate_mesh = meshes.add(Cuboid::from_length(2.0 * CRATE_HALF));
    let dummy_mesh = meshes.add(Capsule3d::new(
        DUMMY_RADIUS,
        DUMMY_HEIGHT - 2.0 * DUMMY_RADIUS,
    ));
    let lit = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.95, 0.8),
        emissive: LinearRgba::rgb(2.0, 1.6, 0.8),
        ..default()
    });
    let crate_look = materials.add(Color::srgb(0.75, 0.5, 0.2));
    let dummy_look = materials.add(Color::srgb(0.6, 0.25, 0.3));
    for target in melee_targets() {
        let (mesh, look) = match target.kind {
            TargetKind::Crate => (crate_mesh.clone(), crate_look.clone()),
            TargetKind::Dummy => (dummy_mesh.clone(), dummy_look.clone()),
        };
        let flash = Flash {
            normal: look.clone(),
            lit: lit.clone(),
        };
        let mut entity = commands.spawn((
            Name::new(target.name),
            DemoEntity,
            target.kind,
            flash,
            Visibility::default(),
        ));
        match target.kind {
            TargetKind::Crate => {
                // A collider is centred on its entity.
                entity.insert((
                    target_bundle(CRATE_HALF + 0.1, 0.0, 1.0),
                    Collider::Cuboid {
                        half_extents: Vec3::splat(CRATE_HALF),
                    },
                    Transform::from_translation(target.position + Vec3::Y * CRATE_HALF),
                    children![(Mesh3d(mesh), MeshMaterial3d(look), Transform::default())],
                ));
            }
            TargetKind::Dummy => {
                entity.insert((
                    target_bundle(DUMMY_RADIUS + 0.1, DUMMY_HEIGHT / 2.0, 5.0),
                    KillXp(DUMMY_XP),
                    Pushable,
                    SimTransform::from_translation(target.position),
                    children![(
                        Mesh3d(mesh),
                        MeshMaterial3d(look),
                        Transform::from_xyz(0.0, DUMMY_HEIGHT / 2.0, 0.0),
                    )],
                ));
            }
        }
    }

    commands.spawn((
        Name::new("sun"),
        DemoEntity,
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(30.0, 60.0, 20.0).looking_at(Vec3::new(0.0, 0.0, -40.0), Vec3::Y),
    ));
    if settings.spectator {
        commands.spawn((
            Name::new("spectator camera"),
            DemoEntity,
            Camera3d::default(),
            surroundings,
            SpectatorCamera::default(),
            SimTransform::from_xyz(0.0, 6.0, 12.0),
        ));
        return;
    }

    commands.insert_resource(guns::GunLooks::new(&mut meshes, &mut materials));
    commands.spawn((
        Name::new("hero"),
        DemoEntity,
        hero_bundle(0, HERO_SPAWN, 0.0, RESPAWN_BELOW, &tuning.0),
        demo_arsenal(&armory),
        // The rig dresses him in the chosen model.
        RigLook::for_player(0),
        Visibility::default(),
    ));
    commands.spawn(guns::hud());
    commands.spawn((
        Name::new("lock-on marker"),
        DemoEntity,
        LockMarker,
        // Wide enough to clear a crate's corners and a dummy's height.
        Mesh3d(meshes.add(Torus::new(0.78, 0.92))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.35, 0.2),
            unlit: true,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));
    let mut camera = FollowCamera::for_player(0, 0.0);
    camera.setting = settings.camera;
    let (translation, rotation) = camera.placement(HERO_SPAWN, &tuning.0.camera);
    commands.spawn((
        Name::new("hero camera"),
        DemoEntity,
        Camera3d::default(),
        surroundings,
        camera,
        SimTransform::from_translation(translation).with_rotation(rotation),
    ));
}

/// Removes everything [`spawn_demo`] created.
pub fn despawn_demo(mut commands: Commands, entities: Query<Entity, With<DemoEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// A crate knocked out breaks; a dummy gets up again.
fn break_targets(
    mut destroyed: MessageReader<Destroyed>,
    mut targets: Query<(&TargetKind, &mut Hitpoints)>,
    mut commands: Commands,
) {
    for event in destroyed.read() {
        let Ok((kind, mut hitpoints)) = targets.get_mut(event.target) else {
            continue;
        };
        match kind {
            TargetKind::Crate => commands.entity(event.target).despawn(),
            TargetKind::Dummy => hitpoints.current = hitpoints.max,
        }
    }
}

/// Sparks where the wrench clanks off a wall or the ground, each shrinking away.
fn show_clanks(
    heroes: Query<&HeroBody>,
    mut sparks: Query<(Entity, &mut Spark, &mut Transform)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut spark, mut transform) in &mut sparks {
        spark.ticks_left = spark.ticks_left.saturating_sub(1);
        if spark.ticks_left == 0 {
            commands.entity(entity).despawn();
        } else {
            transform.scale = Vec3::splat(spark.ticks_left as f32 / SPARK_TICKS as f32);
        }
    }
    for body in &heroes {
        let Some(clank) = body.0.clank else {
            continue;
        };
        commands.spawn((
            Name::new("clank"),
            DemoEntity,
            Spark {
                ticks_left: SPARK_TICKS,
            },
            Mesh3d(meshes.add(Sphere::new(0.12))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.9, 0.5),
                unlit: true,
                ..default()
            })),
            Transform::from_translation(clank.point),
        ));
    }
}

/// Lights a target up while it flashes from a hit.
fn flash_targets(
    targets: Query<(&HitReaction, &Flash, &Children)>,
    mut looks: Query<&mut MeshMaterial3d<StandardMaterial>>,
) {
    for (reaction, flash, children) in &targets {
        let wanted = if reaction.flash_ticks > 0 {
            &flash.lit
        } else {
            &flash.normal
        };
        for &child in children {
            if let Ok(mut look) = looks.get_mut(child)
                && look.0 != *wanted
            {
                look.0 = wanted.clone();
            }
        }
    }
}

/// Rings the locked-on target, turned to the camera and spinning.
fn show_lock_on(
    time: Res<Time>,
    locks: Query<&LockOn>,
    cameras: Query<&GlobalTransform, With<FollowCamera>>,
    mut markers: Query<(&mut Transform, &mut Visibility), With<LockMarker>>,
) {
    let target = locks.iter().find_map(LockOn::target_point);
    for (mut transform, mut visibility) in &mut markers {
        let (Some(at), Ok(camera)) = (target, cameras.single()) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Visible);
        let towards = (camera.translation() - at).normalize_or(Vec3::Z);
        // A torus lies flat about +y; stand it up facing the camera.
        let facing = Quat::from_rotation_arc(Vec3::Y, towards);
        *transform = Transform::from_translation(at)
            .with_rotation(facing * Quat::from_rotation_y(time.elapsed_secs() * 3.0));
    }
}

/// Gives the hero's camera the chosen setting.
fn choose_camera(settings: Res<DemoSettings>, mut cameras: Query<&mut FollowCamera>) {
    for mut camera in &mut cameras {
        if camera.setting != settings.camera {
            camera.setting = settings.camera;
        }
    }
}

/// Hides the hero while his camera looks out of his eyes.
fn hide_hero_in_first_person(
    cameras: Query<&FollowCamera>,
    mut heroes: Query<(&Hero, &mut Visibility)>,
) {
    for (hero, mut visibility) in &mut heroes {
        let first_person = cameras
            .iter()
            .any(|camera| camera.player == hero.player && camera.is_first_person());
        visibility.set_if_neq(if first_person {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
}

fn move_platforms(tick: Res<SimTick>, mut platforms: Query<(&Mover, &mut SimTransform)>) {
    for (mover, mut transform) in &mut platforms {
        transform.translation = mover.position_at(tick.0);
    }
}

//! The hero's guns in the demo (P1.9): UYA's six read guns at V3, and how their shots,
//! rays, arcs and impacts look, with a line saying what is in his hand.

use bevy::prelude::*;
use rac_combat::{
    Armory, Arsenal, GunEvent, GunEventKind,
    weapons::{Hand, ShotKind},
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

/// The line at the bottom of the screen naming the gun in his hand.
#[derive(Component, Debug, Default, Clone, Copy)]
pub(crate) struct GunHud;

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
        }
    }
}

/// How big a shot looks, and in what.
fn shot_look(kind: &ShotKind, speed: f32, looks: &GunLooks) -> (f32, Handle<StandardMaterial>) {
    match kind {
        // The Flux Rifle's shot is far faster than the N60's bullets.
        ShotKind::Bullet { .. } if speed > 200.0 => (0.1, looks.sniper.clone()),
        ShotKind::Bullet { .. } => (0.07, looks.bullet.clone()),
        ShotKind::Charge { .. } => (0.2, looks.charge.clone()),
        ShotKind::Blast { radius, .. } => (*radius, looks.blast.clone()),
        ShotKind::Stream => (0.12, looks.lava.clone()),
        ShotKind::Missile { .. } => (0.14, looks.missile.clone()),
    }
}

/// Keeps a look for every shot in flight, following it in the simulation so it is drawn
/// between ticks, and drops the looks of shots that ended.
pub(crate) fn show_shots(
    looks: Res<GunLooks>,
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
                // Interpolation leaves the scale alone, so a blast grows through it.
                if let ShotKind::Blast { radius, .. } = shot.kind {
                    transform.scale = Vec3::splat(radius);
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
            let (size, material) = shot_look(&shot.kind, shot.velocity.length(), &looks);
            commands.spawn((
                Name::new("shot"),
                DemoEntity,
                look,
                Mesh3d(looks.ball.clone()),
                MeshMaterial3d(material),
                SimTransform::from_translation(shot.position),
                Transform::from_translation(shot.position).with_scale(Vec3::splat(size)),
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

/// The HUD's line.
pub(crate) fn hud() -> impl Bundle {
    (
        Name::new("gun hud"),
        DemoEntity,
        GunHud,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(22.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.95, 0.8)),
        // Top right, clear of the app's controls at the bottom left.
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(24.0),
            top: Val::Px(20.0),
            ..default()
        },
    )
}

/// Says what is in his hand: the gun, its level, ammo and XP.
pub(crate) fn update_hud(
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal)>,
    mut lines: Query<&mut Text, With<GunHud>>,
) {
    let Some((_, arsenal)) = heroes.iter().find(|(hero, _)| hero.player == 0) else {
        return;
    };
    let line = match arsenal.hand {
        Hand::Wrench => {
            let next = arsenal
                .equipped
                .and_then(|weapon| armory.get(weapon))
                .map_or(String::new(), |def| {
                    format!("  (fire for the {})", def.name)
                });
            format!("OmniWrench{next}")
        }
        Hand::Drawing { weapon, .. } | Hand::Gun { weapon, .. } => {
            let (Some(def), Some(gun)) = (armory.get(weapon), arsenal.gun(weapon)) else {
                return;
            };
            const TIERS: [&str; 8] = ["V1", "V2", "V3", "V4", "V5", "Mega", "Giga", "Omega"];
            let tier = TIERS.get(usize::from(gun.level)).unwrap_or(&"");
            let stats = def.level(gun.level);
            let ammo = if stats.ammo == 0 {
                String::new()
            } else {
                format!("  {}/{}", gun.ammo, stats.ammo)
            };
            let xp = if stats.xp == 0 {
                String::new()
            } else {
                format!("  XP {}/{}", gun.xp, stats.xp)
            };
            format!("{} {tier}{ammo}{xp}", def.name_at(gun.level))
        }
    };
    for mut text in &mut lines {
        if text.0 != line {
            text.0 = line.clone();
        }
    }
}

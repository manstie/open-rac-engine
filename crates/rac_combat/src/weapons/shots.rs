//! Shots in flight, each moved and tested once a tick (`weapons.md`, "Each gun"): the
//! N60's and Flux Rifle's shots, the Nitro Launcher's charges and blasts, the Lava Gun's
//! stream and the Annihilator's missiles.

use bevy::prelude::*;
use rac_hero::CollisionWorld;

use super::{
    GunEventKind,
    aim::{GunTarget, first_target},
    def::{Armory, FireMode, Homing, Lobbed, Mods, Projectile, Stream, WeaponId},
    fire::{Hit, Outcome},
};

/// Seconds per tick.
const TICK: f32 = 1.0 / 60.0;

/// How far off a surface a bouncing shot is put back, so it doesn't hit it again at once.
const BOUNCE_GAP: f32 = 0.01;

/// A shot in flight.
#[derive(Debug, Clone, PartialEq)]
pub struct Shot {
    /// Numbered from 1 for each hero, so a renderer can follow it.
    pub serial: u32,
    pub weapon: WeaponId,
    pub level: u8,
    pub position: Vec3,
    /// u/s.
    pub velocity: Vec3,
    /// Ticks since it was fired.
    pub ticks: u32,
    pub damage: f32,
    pub splash: f32,
    pub mods: Mods,
    pub kind: ShotKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShotKind {
    /// Flies straight for `lifetime` ticks, bouncing off the world if `bounces`.
    Bullet { lifetime: u32, bounces: bool },
    /// Thrown in an arc; `fuse` counts down once it has bounced.
    Charge { fuse: Option<u32> },
    /// Grows to hit what it touches, each target once.
    Blast { radius: f32, struck: Vec<Entity> },
    /// One point of a stream, falling.
    Stream,
    /// Turns after `target` while it lives.
    Missile { target: Option<Entity> },
}

impl Shot {
    fn hit(&self, target: Entity, damage: f32, push: Vec3) -> Hit {
        Hit {
            target,
            weapon: self.weapon,
            damage,
            push,
            shock: self.mods.shock,
            acid: self.mods.acid,
        }
    }
}

/// Moves every shot a tick, hitting what they meet; shots that end are dropped, and a
/// charge that bursts leaves a blast behind.
pub(crate) fn step(
    shots: &mut Vec<Shot>,
    serial: &mut u32,
    armory: &Armory,
    world: &dyn CollisionWorld,
    targets: &[GunTarget],
    out: &mut Outcome,
) {
    let mut born = Vec::new();
    shots.retain_mut(|shot| {
        let Some(def) = armory.get(shot.weapon) else {
            return false;
        };
        shot.ticks += 1;
        match &def.fire {
            FireMode::Projectile(fire) => bullet(shot, fire, world, targets, out),
            FireMode::Lobbed(fire) if matches!(shot.kind, ShotKind::Blast { .. }) => {
                blast(shot, fire, targets, out)
            }
            FireMode::Lobbed(fire) => {
                let keep = charge(shot, fire, world, targets);
                if !keep {
                    out.event(shot.weapon, GunEventKind::Impact { at: shot.position });
                    born.push(Shot {
                        velocity: Vec3::ZERO,
                        ticks: 0,
                        kind: ShotKind::Blast {
                            radius: fire.blast.radius_from,
                            struck: Vec::new(),
                        },
                        ..shot.clone()
                    });
                }
                keep
            }
            FireMode::Stream(fire) => stream_point(shot, fire, world, targets, out),
            FireMode::Homing(fire) => missile(shot, fire, world, targets, out),
            FireMode::Hitscan(_) => false,
        }
    });
    for mut shot in born {
        *serial += 1;
        shot.serial = *serial;
        shots.push(shot);
    }
}

/// What a step from `from` by `motion` meets first: a target (by index) or the world,
/// with how far along.
enum Meets {
    Target(usize, f32),
    World(f32, Vec3),
    Nothing,
}

fn meets(world: &dyn CollisionWorld, targets: &[GunTarget], from: Vec3, motion: Vec3) -> Meets {
    let target = first_target(targets, from, from + motion, |_| true);
    let solid = world.cast_ray(from, motion);
    match (target, solid) {
        (Some((index, t)), Some(hit)) if t <= hit.fraction => Meets::Target(index, t),
        (Some((index, t)), None) => Meets::Target(index, t),
        (_, Some(hit)) => Meets::World(hit.fraction, hit.normal),
        (None, None) => Meets::Nothing,
    }
}

/// Every target within `radius` of `at` but `except`.
fn in_ball(
    targets: &[GunTarget],
    at: Vec3,
    radius: f32,
    except: Option<Entity>,
) -> impl Iterator<Item = &GunTarget> {
    targets.iter().filter(move |target| {
        Some(target.entity) != except && target.centre.distance(at) <= radius + target.radius
    })
}

/// The N60's bullet (0x35B390) and the Flux Rifle's shot (0x3463A0): straight on, stopping
/// at the first target, and at the world unless it bounces.
fn bullet(
    shot: &mut Shot,
    fire: &Projectile,
    world: &dyn CollisionWorld,
    targets: &[GunTarget],
    out: &mut Outcome,
) -> bool {
    let ShotKind::Bullet { lifetime, bounces } = shot.kind else {
        return false;
    };
    let motion = shot.velocity * TICK;
    match meets(world, targets, shot.position, motion) {
        Meets::Target(index, t) => {
            let target = &targets[index];
            let at = shot.position + motion * t;
            let push = shot.velocity.normalize_or_zero() * fire.push;
            out.hits.push(shot.hit(target.entity, shot.damage, push));
            if fire.target_radius > 0.0 {
                for other in in_ball(targets, at, fire.target_radius, Some(target.entity)) {
                    out.hits.push(shot.hit(other.entity, shot.damage, push));
                }
            }
            out.event(shot.weapon, GunEventKind::Impact { at });
            false
        }
        Meets::World(t, normal) if bounces => {
            shot.position += motion * t + normal * BOUNCE_GAP;
            shot.velocity = shot.velocity.reflect(normal);
            shot.ticks < lifetime
        }
        Meets::World(t, _) => {
            let at = shot.position + motion * t;
            if fire.world_radius > 0.0 {
                let push = shot.velocity.normalize_or_zero() * fire.push;
                for target in in_ball(targets, at, fire.world_radius, None) {
                    out.hits.push(shot.hit(target.entity, shot.damage, push));
                }
            }
            out.event(shot.weapon, GunEventKind::Impact { at });
            false
        }
        Meets::Nothing => {
            shot.position += motion;
            shot.ticks < lifetime
        }
    }
}

/// The Nitro Launcher's charge (0x361218): it falls, bounces off the world, and bursts on
/// a target near it, a while after a bounce, or on the ground once it has flown a while.
/// Returns false when it bursts.
fn charge(
    shot: &mut Shot,
    fire: &Lobbed,
    world: &dyn CollisionWorld,
    targets: &[GunTarget],
) -> bool {
    let ShotKind::Charge { fuse } = &mut shot.kind else {
        return false;
    };
    if let Some(left) = fuse {
        *left = left.saturating_sub(1);
        if *left == 0 {
            return false;
        }
    }
    shot.velocity.y -= fire.gravity * TICK;
    let motion = shot.velocity * TICK;
    match world.cast_ray(shot.position, motion) {
        Some(hit) => {
            shot.position += motion * hit.fraction + hit.normal * BOUNCE_GAP;
            shot.velocity = shot.velocity.reflect(hit.normal);
            fuse.get_or_insert(fire.fuse_ticks);
        }
        None => shot.position += motion,
    }
    let near_target =
        in_ball(targets, shot.position, fire.target_radius, None).any(|target| target.alive);
    let on_ground = shot.ticks > fire.ground_after_ticks
        && world
            .cast_ray(shot.position, Vec3::NEG_Y * fire.ground_probe)
            .is_some();
    !(near_target || on_ground || shot.ticks >= fire.lifetime)
}

/// The Nitro Launcher's blast (0x35E718): a flash, then a ball growing from `radius_from`
/// to `radius_to`, hitting each target it touches once, pushing it away.
fn blast(shot: &mut Shot, fire: &Lobbed, targets: &[GunTarget], out: &mut Outcome) -> bool {
    let blast = &fire.blast;
    let ShotKind::Blast { radius, struck } = &mut shot.kind else {
        return false;
    };
    let ticks = shot.ticks;
    if ticks <= blast.flash_ticks {
        return true;
    }
    let grown = (ticks - blast.flash_ticks) as f32 / blast.grow_ticks.max(1) as f32;
    let to = blast.radius_to.at(shot.level).unwrap_or(blast.radius_from);
    *radius = blast.radius_from + (to - blast.radius_from) * grown.min(1.0);
    for target in in_ball(targets, shot.position, *radius, None) {
        if struck.contains(&target.entity) {
            continue;
        }
        struck.push(target.entity);
        let away = (target.centre - shot.position).normalize_or_zero() * blast.push;
        out.hits.push(Hit {
            target: target.entity,
            weapon: shot.weapon,
            damage: shot.damage,
            push: away,
            shock: shot.mods.shock,
            acid: shot.mods.acid,
        });
    }
    ticks < blast.flash_ticks + blast.grow_ticks
}

/// One point of the Lava Gun's stream (0x304A18): it falls, stops on the world, and on a
/// target does its damage, and the splash to everything near.
fn stream_point(
    shot: &mut Shot,
    fire: &Stream,
    world: &dyn CollisionWorld,
    targets: &[GunTarget],
    out: &mut Outcome,
) -> bool {
    shot.velocity.y -= fire.gravity * TICK;
    let motion = shot.velocity * TICK;
    match meets(world, targets, shot.position, motion) {
        Meets::Target(index, t) => {
            let target = &targets[index];
            let at = shot.position + motion * t;
            out.hits
                .push(shot.hit(target.entity, shot.damage, Vec3::ZERO));
            if shot.splash > 0.0 {
                for other in in_ball(targets, at, fire.splash_radius, Some(target.entity)) {
                    out.hits
                        .push(shot.hit(other.entity, shot.splash, Vec3::ZERO));
                }
            }
            out.event(shot.weapon, GunEventKind::Impact { at });
            false
        }
        Meets::World(t, _) => {
            out.event(
                shot.weapon,
                GunEventKind::Impact {
                    at: shot.position + motion * t,
                },
            );
            false
        }
        Meets::Nothing => {
            shot.position += motion;
            shot.ticks < fire.lifetime
        }
    }
}

/// The Annihilator's missile (0x396A20): it turns after its target while that lives, and
/// bursts on the first thing it meets, its blast doing the splash damage round it.
fn missile(
    shot: &mut Shot,
    fire: &Homing,
    world: &dyn CollisionWorld,
    targets: &[GunTarget],
    out: &mut Outcome,
) -> bool {
    let ShotKind::Missile { target } = &mut shot.kind else {
        return false;
    };
    let chased = target.and_then(|entity| {
        targets
            .iter()
            .find(|candidate| candidate.entity == entity && candidate.alive)
    });
    match chased {
        Some(chased) if shot.ticks <= fire.homing_ticks => {
            let wanted = (chased.centre - shot.position).normalize_or_zero();
            let direction = shot.velocity.normalize_or_zero();
            if wanted != Vec3::ZERO && direction != Vec3::ZERO {
                let turned = super::aim::bend(direction, wanted, fire.turn_degrees.to_radians());
                shot.velocity = turned * fire.speed;
            }
        }
        Some(_) => {}
        None => *target = None,
    }
    let motion = shot.velocity * TICK;
    let at = match meets(world, targets, shot.position, motion) {
        Meets::Target(index, t) => {
            let struck = &targets[index];
            let push = shot.velocity.normalize_or_zero() * fire.push;
            out.hits.push(shot.hit(struck.entity, shot.damage, push));
            shot.position + motion * t
        }
        Meets::World(t, _) => shot.position + motion * t,
        Meets::Nothing => {
            shot.position += motion;
            return shot.ticks < fire.lifetime;
        }
    };
    if shot.splash > 0.0 {
        for near in in_ball(targets, at, fire.blast_radius, None) {
            out.hits
                .push(shot.hit(near.entity, shot.splash, Vec3::ZERO));
        }
    }
    out.event(shot.weapon, GunEventKind::Impact { at });
    false
}

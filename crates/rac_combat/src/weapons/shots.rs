//! Shots in flight, each moved and tested once a tick (`weapons.md`, "Each gun"): the
//! N60's and Flux Rifle's shots, the Nitro Launcher's charges and blasts, the Lava Gun's
//! stream and the Annihilator's missiles here; what the other guns fire, throw or put down
//! in [`super::seekers`], [`super::thrown`], [`super::grab`] and [`super::hand`].

use bevy::prelude::*;
use rac_hero::CollisionWorld;
use rac_sim::SimRng;

use super::{
    GunEventKind,
    aim::{GunTarget, first_target},
    def::{Armory, FireMode, Homing, Lobbed, Mods, Projectile, Stream, WeaponId},
    ducks,
    fire::{GunInput, Hit, Outcome},
    grab, hand, seekers, thrown,
};

/// Seconds per tick.
pub(super) const TICK: f32 = 1.0 / 60.0;

/// How far off a surface a bouncing shot is put back, so it doesn't hit it again at once.
pub(super) const BOUNCE_GAP: f32 = 0.01;

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
    /// Flies straight for `lifetime` ticks, bouncing off the world `bounces` more times.
    Bullet { lifetime: u32, bounces: u32 },
    /// Thrown in an arc; `fuse` counts down once it has bounced.
    Charge { fuse: Option<u32> },
    /// Grows to hit what it touches, each target once.
    Blast { radius: f32, struck: Vec<Entity> },
    /// One point of a stream, falling.
    Stream,
    /// Turns after `target` while it lives: the Annihilator's.
    Missile { target: Option<Entity> },
    /// The RY3NO's missile.
    Rocket(RocketState),
    /// The Plasma Coil's ball; once it sticks, to a target or the world, the ticks since.
    Ball {
        stuck: Option<(Option<Entity>, u32)>,
    },
    /// An arc from where a coil ball stuck, reaching `target` as it ends.
    Arc { target: Entity },
    /// The Plasma Storm's ball: the targets it has arced to, and when.
    Storm { arced: Vec<(Entity, u32)> },
    /// The Bouncer's canister: its bounces, and the ticks it has rolled since the last.
    Canister { bounces: u32, rolled: Option<u32> },
    /// One of the canister's bomblets: its number, and the ticks it lives.
    Bomblet { index: u32, life: u32 },
    /// The Infector's blob: the ticks it lives, and the enemies it has infected.
    Blob { life: u32, infected: Vec<Entity> },
    /// The Infecto-Bomb, falling.
    Bomb,
    /// The Disc Blade Gun's disc: what it flies at, which way it curves (radians a tick),
    /// and the target it last bounced off.
    Disc {
        target: Option<Entity>,
        curve: f32,
        last: Option<Entity>,
        /// The Multi-Disc Gun's splits it came of.
        splits: u32,
    },
    /// The Spitting Hydra's spit: arcs from the gun reaching for `ends`, for `life`
    /// ticks.
    Spit { life: u32, ends: Vec<Vec3> },
    /// A lightning bolt (0x34DAA0) on `target`, striking it as it goes in `left` ticks,
    /// pushed `push` along it.
    Bolt {
        target: Entity,
        left: u32,
        push: f32,
    },
    /// The Rift Inducer's rift, flying until it opens.
    Rift(RiftState),
    /// Something the Suck Cannon fires back out, pulled at `target`.
    Thing { target: Option<Entity> },
    /// A Miniturret.
    Turret(TurretState),
    /// A turret's shot.
    Pellet,
    /// A Megaturret's missile at `target`.
    Guided { target: Option<Entity> },
    /// A holo shield.
    Barrier(BarrierState),
    /// The Agents of Doom's pod, falling until it lands and lets them out.
    Pod,
    /// One of the agents, running at `target` for `chase` ticks so far.
    Agent { target: Option<Entity>, chase: u32 },
    /// The comet an agent ends in from V2, flying `life` ticks.
    Comet { life: u32 },
    /// The Plasma Whip's shock wave along the ground.
    Wave,
    /// The Quantum Whip's cutting wave, and what it has struck.
    Cut { struck: Vec<Entity> },
    /// An Ultrashield's pulse at `target`.
    Pulse { target: Entity },
    /// A duck the Qwack-O-Ray made.
    Duck(DuckState),
    /// A duck's egg, living `life` ticks.
    Egg { life: u32 },
}

/// A duck: the way it faces, the ticks before it turns and lays, the enemy it is after,
/// its pecks so far and the ticks it waits before it starts.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DuckState {
    pub yaw: f32,
    pub turn: u32,
    pub egg: u32,
    pub target: Option<Entity>,
    pub pecks: u32,
    pub wait: u32,
}

/// A rift: open for the ticks since, and the target it is pulling in for how long.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct RiftState {
    pub open: Option<u32>,
    pub taking: Option<(Entity, u32)>,
}

/// A RY3NO missile: it spirals round the line it set off along, `tilt` off it, its roll
/// turning `spin` a tick (radians).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RocketState {
    pub target: Option<Entity>,
    pub along: Vec3,
    pub tilt: f32,
    pub roll: f32,
    pub spin: f32,
    /// Ticks it waits before it flies, and ticks it has left once it does.
    pub wait: u32,
    pub life: u32,
    /// u/s.
    pub speed: f32,
}

/// A Miniturret: falling until it lands, then unfolding and shooting what it finds.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct TurretState {
    /// Ticks since it landed.
    pub landed: Option<u32>,
    pub target: Option<Entity>,
    /// How far its barrel has turned to the target, 0 to 1.
    pub turn: f32,
    pub cooldown: u32,
    pub fired: u32,
    /// Ticks since it landed or last fired.
    pub idle: u32,
    /// The Megaturret: ticks to its next look at walking back, and where it walks to.
    pub walk_wait: u32,
    pub walk_to: Option<Vec3>,
}

/// A holo shield: falling until it lands, then standing as a wall that takes hits.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct BarrierState {
    /// Ticks since it landed.
    pub landed: Option<u32>,
    /// The way it faces, as his yaw when he threw it.
    pub yaw: f32,
    pub hits_left: u32,
    /// Folding away: ticks left.
    pub folding: Option<u32>,
    /// Ticks until the bolt a hit sends (V3–V4), or into the reaction that sends the
    /// Ultrashield's pulses.
    pub bolt: Option<u32>,
    pub react: Option<u32>,
    /// Ticks to the next point of health it gives him, and to its next drain of an enemy
    /// near it (from V2).
    pub heal: u32,
    pub drain: u32,
}

impl Shot {
    pub(super) fn hit(&self, target: Entity, damage: f32, push: Vec3) -> Hit {
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

/// What the shots in flight see.
pub(super) struct Flight<'a> {
    pub world: &'a dyn CollisionWorld,
    pub targets: &'a [GunTarget],
    /// Where the hero is and the way he faces: some shots keep near him.
    pub hero: Vec3,
    pub facing: Vec3,
    /// He has less health than he can have.
    pub hurt: bool,
}

/// Moves every shot a tick, hitting what they meet; shots that end are dropped, and those
/// that leave others behind (a charge's blast, a canister's bomblets, a pod's agents) add
/// them.
pub(crate) fn step(
    shots: &mut Vec<Shot>,
    serial: &mut u32,
    armory: &Armory,
    gun_input: &GunInput,
    rng: &mut SimRng,
    out: &mut Outcome,
) {
    let flight = Flight {
        world: gun_input.world,
        targets: gun_input.targets,
        hero: gun_input.hero.position,
        facing: Quat::from_rotation_y(gun_input.hero.yaw) * Vec3::NEG_Z,
        hurt: gun_input.hurt,
    };
    let (world, targets) = (flight.world, flight.targets);
    let mut agents = shots
        .iter()
        .filter(|shot| matches!(shot.kind, ShotKind::Agent { .. }))
        .count();
    let mut discs = shots
        .iter()
        .filter(|shot| matches!(shot.kind, ShotKind::Disc { .. }))
        .count();
    let mut born = Vec::new();
    shots.retain_mut(|shot| {
        let Some(def) = armory.get(shot.weapon) else {
            return false;
        };
        shot.ticks += 1;
        if matches!(shot.kind, ShotKind::Bolt { .. }) {
            return bolt(shot, targets, out);
        }
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
            FireMode::Homing(fire) => missile(shot, &Seek::homing(fire), world, targets, out),
            FireMode::Volley(fire) => seekers::rocket(shot, fire, &flight, out),
            FireMode::Coil(fire) => seekers::coil_shot(shot, fire, &flight, out, &mut born),
            FireMode::Disc(fire) => {
                seekers::disc_shot(shot, fire, &flight, rng, out, &mut born, &mut discs)
            }
            FireMode::Hydra(fire) => seekers::spit(shot, fire, &flight, out),
            FireMode::Cluster(fire) => {
                thrown::cluster_shot(shot, fire, &flight, rng, out, &mut born)
            }
            FireMode::Infect(fire) => thrown::blob(shot, fire, &flight, out),
            FireMode::Turret(fire) => thrown::turret_shot(shot, fire, &flight, rng, out, &mut born),
            FireMode::Barrier(fire) => thrown::barrier_shot(shot, fire, &flight, out, &mut born),
            FireMode::Agents(fire) => {
                thrown::agents_shot(shot, fire, &flight, out, &mut born, &mut agents)
            }
            FireMode::Rift(fire) => grab::rift_shot(shot, fire, &flight, rng, out, &mut born),
            FireMode::Suck(fire) => grab::thing(shot, fire, &flight, out),
            FireMode::Whip(fire) if matches!(shot.kind, ShotKind::Cut { .. }) => {
                hand::cut(shot, fire, &flight, out)
            }
            FireMode::Whip(fire) => hand::wave(shot, fire, &flight, out),
            FireMode::Beam(fire) => ducks::duck_shot(shot, fire, &flight, rng, out, &mut born),
            FireMode::Hitscan(_) | FireMode::Shield(_) => false,
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
pub(super) enum Meets {
    Target(usize, f32),
    World(f32, Vec3),
    Nothing,
}

pub(super) fn meets(
    world: &dyn CollisionWorld,
    targets: &[GunTarget],
    from: Vec3,
    motion: Vec3,
) -> Meets {
    meets_where(world, targets, from, motion, |_| true)
}

/// [`meets`], among the targets that are `wanted`.
pub(super) fn meets_where(
    world: &dyn CollisionWorld,
    targets: &[GunTarget],
    from: Vec3,
    motion: Vec3,
    wanted: impl Fn(&GunTarget) -> bool,
) -> Meets {
    let target = first_target(targets, from, from + motion, wanted);
    let solid = world.cast_ray(from, motion);
    match (target, solid) {
        (Some((index, t)), Some(hit)) if t <= hit.fraction => Meets::Target(index, t),
        (Some((index, t)), None) => Meets::Target(index, t),
        (_, Some(hit)) => Meets::World(hit.fraction, hit.normal),
        (None, None) => Meets::Nothing,
    }
}

/// Every target within `radius` of `at` but `except`.
pub(super) fn in_ball(
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
    let ShotKind::Bullet { lifetime, bounces } = &mut shot.kind else {
        return false;
    };
    let lifetime = *lifetime;
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
        Meets::World(t, normal) if *bounces > 0 => {
            *bounces -= 1;
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
    if let Some(eruption) = blast.eruption.filter(|e| shot.level >= e.from_level) {
        if ticks <= eruption.wait_ticks {
            return true;
        }
        let grown = (ticks - eruption.wait_ticks) as f32 / eruption.grow_ticks.max(1) as f32;
        let (from, to) = eruption.radius;
        *radius = from + (to - from) * grown.min(1.0);
        for target in in_ball(targets, shot.position, *radius, None) {
            let away = (target.centre - shot.position).normalize_or_zero() * eruption.push;
            out.hits.push(Hit {
                target: target.entity,
                weapon: shot.weapon,
                damage: shot.damage,
                push: away,
                shock: shot.mods.shock,
                acid: shot.mods.acid,
            });
        }
        return ticks < eruption.wait_ticks + eruption.grow_ticks;
    }
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

/// How a missile turns after its target and bursts.
pub(super) struct Seek {
    speed: f32,
    lifetime: u32,
    homing_ticks: u32,
    turn_degrees: f32,
    push: f32,
    blast_radius: f32,
}

impl Seek {
    fn homing(fire: &Homing) -> Self {
        Self {
            speed: fire.speed,
            lifetime: fire.lifetime,
            homing_ticks: fire.homing_ticks,
            turn_degrees: fire.turn_degrees,
            push: fire.push,
            blast_radius: fire.blast_radius,
        }
    }
}

/// The Annihilator's missile (0x396A20): it turns after its target while that lives, and
/// bursts on the first thing it meets, its blast doing the splash damage round it.
fn missile(
    shot: &mut Shot,
    fire: &Seek,
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

/// A lightning bolt (0x34DD38, its strike 0x34DFD0): it holds on its target, and as it goes
/// the target, if it still lives, takes the bolt's damage, pushed along it.
fn bolt(shot: &mut Shot, targets: &[GunTarget], out: &mut Outcome) -> bool {
    let ShotKind::Bolt { target, left, push } = &mut shot.kind else {
        return false;
    };
    let Some(struck) = targets
        .iter()
        .find(|candidate| candidate.entity == *target && candidate.alive)
    else {
        return false;
    };
    out.event(
        shot.weapon,
        GunEventKind::Arc {
            from: shot.position,
            to: struck.centre,
        },
    );
    *left = left.saturating_sub(1);
    if *left > 0 {
        return true;
    }
    let push = (struck.centre - shot.position).normalize_or_zero() * *push;
    out.hits.push(shot.hit(struck.entity, shot.damage, push));
    false
}

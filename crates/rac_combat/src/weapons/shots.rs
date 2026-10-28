//! Shots in flight, each moved and tested once a tick (`weapons.md`, "Each gun"): the
//! N60's and Flux Rifle's shots, the Nitro Launcher's charges and blasts, the Lava Gun's
//! stream and the Annihilator's missiles here; what the other guns fire, throw or put down
//! in [`super::seekers`], [`super::thrown`], [`super::grab`] and [`super::hand`].

use bevy::{math::ops, prelude::*};
use rac_hero::CollisionWorld;
use rac_sim::SimRng;
use std::f32::consts::TAU;

use super::{
    GunEventKind,
    aim::{GunTarget, first_target},
    def::{
        Armory, FireMode, Homing, Lobbed, Mods, Projectile, ScopeBeam, SlugRules, SmallMissile,
        Spray, Stream, WeaponId,
    },
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
    /// The Flux Rifle's scoped beam, until it `hit` something.
    Beam { hit: bool },
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
    /// A lightning bolt (0x34DAA0) on `target` for `left` ticks more, striking it
    /// [`BOLT_STRIKE_TICKS`] in, pushed `push` along it.
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
    /// One of the Decimator's seekers at `target`, gaining `accel` u/s a tick to `top`.
    Seeker {
        target: Option<Entity>,
        accel: f32,
        top: f32,
    },
    /// A holo shield.
    Barrier(BarrierState),
    /// The Agents of Doom's pod, falling until it lands and lets them out.
    Pod,
    /// One of the agents.
    Agent(AgentState),
    /// UYA's small bullet, which the agents' guns and the Nitro Eruptor fire.
    Slug,
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

impl ShotKind {
    /// The kind's name, by which a game tells apart what a gun's shots of each kind do
    /// ([`super::GunEvent::shot`]).
    pub fn name(&self) -> &'static str {
        match self {
            ShotKind::Bullet { .. } => "bullet",
            ShotKind::Beam { .. } => "beam",
            ShotKind::Charge { .. } => "charge",
            ShotKind::Blast { .. } => "blast",
            ShotKind::Stream => "stream",
            ShotKind::Missile { .. } => "missile",
            ShotKind::Rocket(_) => "rocket",
            ShotKind::Ball { .. } => "ball",
            ShotKind::Arc { .. } => "arc",
            ShotKind::Storm { .. } => "storm",
            ShotKind::Canister { .. } => "canister",
            ShotKind::Bomblet { .. } => "bomblet",
            ShotKind::Blob { .. } => "blob",
            ShotKind::Bomb => "bomb",
            ShotKind::Disc { .. } => "disc",
            ShotKind::Spit { .. } => "spit",
            ShotKind::Bolt { .. } => "bolt",
            ShotKind::Rift(_) => "rift",
            ShotKind::Thing { .. } => "thing",
            ShotKind::Turret(_) => "turret",
            ShotKind::Pellet => "pellet",
            ShotKind::Guided { .. } => "guided",
            ShotKind::Seeker { .. } => "seeker",
            ShotKind::Barrier(_) => "barrier",
            ShotKind::Pod => "pod",
            ShotKind::Agent(_) => "agent",
            ShotKind::Slug => "slug",
            ShotKind::Comet { .. } => "comet",
            ShotKind::Wave => "wave",
            ShotKind::Cut { .. } => "cut",
            ShotKind::Pulse { .. } => "pulse",
            ShotKind::Duck(_) => "duck",
            ShotKind::Egg { .. } => "egg",
        }
    }
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

/// An agent of the Agents of Doom (`weapons.md`, "Agents of Doom").
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct AgentState {
    /// It has made its first search, as it came out.
    pub out: bool,
    /// The target it chases, and the ticks into the chase.
    pub target: Option<Entity>,
    pub chase: u32,
    pub mode: AgentMode,
    /// The way it faces (as the hero's yaw) and how fast that turns, radians a tick.
    pub yaw: f32,
    pub turn: f32,
    /// u/s along the ground.
    pub speed: f32,
    /// Its own top speeds following and chasing, drawn as it comes out.
    pub follow_top: f32,
    pub chase_top: f32,
    /// Following, it idles within this of him, drawn as it sets off.
    pub near: f32,
    /// Ticks idle.
    pub idle: u32,
    /// Its own rounds, and ticks to its next shot.
    pub rounds: u32,
    pub cooldown: u32,
}

/// What an agent does while it has no target.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum AgentMode {
    #[default]
    Idle,
    /// Walking to a point round the hero.
    Wander(Vec3),
    Follow,
}

/// What a hero's agents share: the rounds of their guns at the levels that share them,
/// and the targets they have claimed, with the ticks each claim has left.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct AgentBook {
    pub(crate) rounds: u32,
    pub(crate) claims: Vec<(Entity, u32)>,
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
    book: &mut AgentBook,
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
        .filter(|shot| matches!(shot.kind, ShotKind::Agent(_)))
        .count();
    book.claims.retain_mut(|(_, left)| {
        *left = left.saturating_sub(1);
        *left > 0
    });
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
        if shot.kind == ShotKind::Slug {
            return slug(shot, &armory.rules.slug, &flight, out);
        }
        match &def.fire {
            FireMode::Projectile(fire) => match &fire.scope_beam {
                Some(beam_def) if matches!(shot.kind, ShotKind::Beam { .. }) => {
                    beam(shot, beam_def, world, targets, out)
                }
                _ => bullet(shot, fire, world, targets, out),
            },
            FireMode::Lobbed(fire) if matches!(shot.kind, ShotKind::Blast { .. }) => {
                blast(shot, fire, targets, out)
            }
            FireMode::Lobbed(fire) => {
                let keep = charge(shot, fire, world, targets);
                if !keep {
                    out.shot_event(shot, GunEventKind::Impact { at: shot.position });
                    if let Some(spray) = fire
                        .blast
                        .eruption
                        .filter(|eruption| shot.level >= eruption.from_level)
                        .and_then(|eruption| eruption.spray)
                    {
                        spray_slugs(shot, &spray, targets, rng, &mut born);
                    }
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
            FireMode::Homing(fire) => match shot.kind {
                ShotKind::Bomblet { index, life } => match fire.mines.at(shot.level) {
                    Some(mines) => thrown::bomblet(
                        shot,
                        mines.gravity,
                        &mines.bomblet,
                        index,
                        life,
                        &flight,
                        out,
                    ),
                    None => false,
                },
                ShotKind::Seeker { .. } => match &fire.seekers {
                    Some(seekers) => seeker(shot, &seekers.missile, &flight, out),
                    None => false,
                },
                _ => match missile(shot, &Seek::homing(fire), world, targets, out) {
                    Course::On => true,
                    Course::Out => false,
                    Course::Burst(at) => {
                        burst_into(shot, fire, at, &flight, rng, &mut born);
                        false
                    }
                },
            },
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
                thrown::agents_shot(shot, fire, &flight, rng, out, &mut born, &mut agents, book)
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
            out.shot_event(shot, GunEventKind::Impact { at });
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
            out.shot_event(shot, GunEventKind::Impact { at });
            false
        }
        Meets::Nothing => {
            shot.position += motion;
            shot.ticks < lifetime
        }
    }
}

/// The Flux Rifle's scoped beam (0x37CB68): its head tests the line of its step, up to
/// `tests` times from the level it pierces, each passing over what the one before hit.
/// Every hit takes a ball at the point hit; the first stops the head, and the beam ends
/// after its ticks.
fn beam(
    shot: &mut Shot,
    def: &ScopeBeam,
    world: &dyn CollisionWorld,
    targets: &[GunTarget],
    out: &mut Outcome,
) -> bool {
    let ShotKind::Beam { hit: false } = shot.kind else {
        return shot.ticks < def.ticks;
    };
    let motion = shot.velocity * TICK;
    let tests = if shot.level >= def.pierce_from {
        def.tests
    } else {
        1
    };
    let radius = def.radius.at(shot.level).unwrap_or(0.0);
    let push = shot.velocity.normalize_or_zero() * def.push.at(shot.level).unwrap_or(0.0);
    let mut skip = None;
    let mut first = None;
    for _ in 0..tests {
        let (t, struck) = match meets_where(world, targets, shot.position, motion, |target| {
            Some(target.entity) != skip
        }) {
            Meets::Target(index, t) => (t, Some(targets[index].entity)),
            Meets::World(t, _) => (t, None),
            Meets::Nothing => break,
        };
        let at = shot.position + motion * t;
        for target in in_ball(targets, at, radius, None) {
            out.hits.push(shot.hit(target.entity, shot.damage, push));
        }
        first.get_or_insert(at);
        skip = struck;
    }
    match first {
        Some(at) => {
            shot.kind = ShotKind::Beam { hit: true };
            shot.position = at;
            shot.velocity = Vec3::ZERO;
            out.shot_event(shot, GunEventKind::Impact { at });
        }
        None => shot.position += motion,
    }
    shot.ticks < def.ticks
}

/// The Nitro Eruptor's burst of small bullets (0x38A668): one at each target far enough
/// off of the first near it (in the targets' order, as the game's list), the rest any way
/// round.
fn spray_slugs(
    shot: &Shot,
    spray: &Spray,
    targets: &[GunTarget],
    rng: &mut SimRng,
    born: &mut Vec<Shot>,
) {
    let at = shot.position;
    let aimed: Vec<Vec3> = targets
        .iter()
        .filter(|target| target.centre.distance(at) <= spray.within)
        .take(spray.aimed)
        .filter(|target| target.centre.distance(at) > spray.near)
        .map(|target| (target.centre - at).normalize_or(Vec3::Y))
        .collect();
    for index in 0..spray.count as usize {
        let way = aimed.get(index).copied().unwrap_or_else(|| {
            let yaw = rng.unit_f32() * TAU;
            let pitch = rng
                .range_f32(-spray.down_degrees, spray.up_degrees)
                .to_radians();
            Vec3::new(
                -ops::sin(yaw) * ops::cos(pitch),
                ops::sin(pitch),
                -ops::cos(yaw) * ops::cos(pitch),
            )
        });
        let (low, high) = spray.speed;
        let speed = if high > low {
            rng.range_f32(low, high)
        } else {
            low
        };
        born.push(Shot {
            position: at,
            velocity: way * speed,
            ticks: 0,
            damage: spray.damage,
            kind: ShotKind::Slug,
            ..shot.clone()
        });
    }
}

/// UYA's small bullet (0x35D9C0): straight on while it lives and keeps near the hero;
/// what it meets takes a ball of its damage there.
fn slug(shot: &mut Shot, rules: &SlugRules, flight: &Flight, out: &mut Outcome) -> bool {
    let motion = shot.velocity * TICK;
    let (t, radius) = match meets(flight.world, flight.targets, shot.position, motion) {
        Meets::Target(_, t) => (t, rules.target_radius),
        Meets::World(t, _) => (t, rules.world_radius),
        Meets::Nothing => {
            shot.position += motion;
            return shot.ticks < rules.life_ticks
                && shot.position.distance(flight.hero) <= rules.leash;
        }
    };
    let at = shot.position + motion * t;
    for target in in_ball(flight.targets, at, radius, None) {
        out.hits
            .push(shot.hit(target.entity, shot.damage, motion * rules.push));
    }
    out.shot_event(shot, GunEventKind::Impact { at });
    false
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
            out.shot_event(shot, GunEventKind::Impact { at });
            false
        }
        Meets::World(t, _) => {
            out.shot_event(
                shot,
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

/// How a missile's tick ended.
enum Course {
    /// It flies on.
    On,
    /// It ran out of time, or isn't a missile.
    Out,
    /// It burst here.
    Burst(Vec3),
}

/// The Annihilator's missile (0x396A20): it turns after its target while that lives, and
/// bursts on the first thing it meets, its blast doing the splash damage round it.
fn missile(
    shot: &mut Shot,
    fire: &Seek,
    world: &dyn CollisionWorld,
    targets: &[GunTarget],
    out: &mut Outcome,
) -> Course {
    let ShotKind::Missile { target } = &mut shot.kind else {
        return Course::Out;
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
            return if shot.ticks < fire.lifetime {
                Course::On
            } else {
                Course::Out
            };
        }
    };
    if shot.splash > 0.0 {
        for near in in_ball(targets, at, fire.blast_radius, None) {
            out.hits
                .push(shot.hit(near.entity, shot.splash, Vec3::ZERO));
        }
    }
    out.shot_event(shot, GunEventKind::Impact { at });
    Course::Burst(at)
}

/// What the Annihilator's missile leaves as it bursts at `at` (0x396A20's switch on the
/// level): at V2–V4 its mines, from V5 the Decimator's seekers.
fn burst_into(
    shot: &Shot,
    fire: &Homing,
    at: Vec3,
    flight: &Flight,
    rng: &mut SimRng,
    born: &mut Vec<Shot>,
) {
    if let Some(seekers) = fire.seekers.filter(|s| shot.level >= s.from_level) {
        // Fanned either side of the way back from the blast towards the hero.
        let back = super::aim::flat(flight.hero - at).normalize_or(-flight.facing);
        let base = ops::atan2(-back.x, -back.z);
        for _ in 0..seekers.count {
            let bearing = base + rng.range_f32(-1.0, 1.0) * seekers.spread_degrees.to_radians();
            let way = Vec3::new(-ops::sin(bearing), 0.0, -ops::cos(bearing));
            let target = nearest_enemy(flight.targets, at, seekers.range);
            born.push(small_missile(shot, &seekers.missile, at, way, target, rng));
        }
        return;
    }
    if let Some(mines) = fire.mines.at(shot.level).filter(|m| m.count > 0) {
        thrown::scatter(
            shot,
            at,
            mines.count,
            &mines.bomblet,
            shot.damage * mines.share,
            rng,
            born,
        );
    }
}

/// A small missile from `at` along `way` at `target`, with its speeds drawn.
pub(super) fn small_missile(
    shot: &Shot,
    missile: &SmallMissile,
    at: Vec3,
    way: Vec3,
    target: Option<Entity>,
    rng: &mut SimRng,
) -> Shot {
    let speed = rng.range_f32(missile.speed.0, missile.speed.1);
    let accel = rng.range_f32(missile.accel.0, missile.accel.1);
    let top = rng.range_f32(missile.top.0, missile.top.1);
    Shot {
        position: at,
        velocity: way * speed,
        ticks: 0,
        damage: shot.damage * missile.share,
        kind: ShotKind::Seeker { target, accel, top },
        ..shot.clone()
    }
}

/// The nearest living enemy within `range` of `at`.
fn nearest_enemy(targets: &[GunTarget], at: Vec3, range: f32) -> Option<Entity> {
    targets
        .iter()
        .filter(|t| t.alive && t.enemy && t.centre.distance(at) <= range)
        .min_by(|a, b| a.centre.distance(at).total_cmp(&b.centre.distance(at)))
        .map(|t| t.entity)
}

/// A small missile: faster each tick to its top speed, turning after its enemy, bursting
/// on what it meets.
pub(super) fn seeker(
    shot: &mut Shot,
    missile: &SmallMissile,
    flight: &Flight,
    out: &mut Outcome,
) -> bool {
    let ShotKind::Seeker { target, accel, top } = &mut shot.kind else {
        return false;
    };
    let (accel, top) = (*accel, *top);
    if shot.ticks >= missile.lifetime {
        return false;
    }
    let chased = target.and_then(|entity| {
        flight
            .targets
            .iter()
            .find(|candidate| candidate.entity == entity && candidate.alive)
    });
    if chased.is_none() {
        *target = None;
    }
    let speed = (shot.velocity.length() + accel).min(top);
    let mut way = shot.velocity.normalize_or(flight.facing);
    if let Some(chased) = chased {
        let wanted = (chased.centre - shot.position).normalize_or(way);
        way = super::aim::bend(way, wanted, missile.turn_degrees.to_radians());
    }
    shot.velocity = way * speed;
    let motion = shot.velocity * TICK;
    let at = match meets(flight.world, flight.targets, shot.position, motion) {
        Meets::Target(_, t) | Meets::World(t, _) => shot.position + motion * t,
        Meets::Nothing => {
            shot.position += motion;
            return true;
        }
    };
    for near in in_ball(flight.targets, at, missile.blast_radius, None) {
        out.hits.push(shot.hit(near.entity, shot.damage, way * 0.5));
    }
    out.shot_event(shot, GunEventKind::Impact { at });
    false
}

/// When a lightning bolt strikes, in ticks from when it's made: its strike timer starts at
/// 6 (0x34DF10) and it strikes as it runs out (0x34DFD0) [code].
pub const BOLT_STRIKE_TICKS: u32 = 6;

/// A lightning bolt (0x34DD38, its strike 0x34DFD0): it holds on its target, which, if it
/// still lives, takes the bolt's damage [`BOLT_STRIKE_TICKS`] in, pushed along it; the
/// bolt goes on to the end of its life.
fn bolt(shot: &mut Shot, targets: &[GunTarget], out: &mut Outcome) -> bool {
    let ShotKind::Bolt { target, .. } = shot.kind else {
        return false;
    };
    let Some(struck) = targets
        .iter()
        .find(|candidate| candidate.entity == target && candidate.alive)
    else {
        return false;
    };
    out.shot_event(
        shot,
        GunEventKind::Arc {
            from: shot.position,
            to: struck.centre,
        },
    );
    let ShotKind::Bolt { left, push, .. } = &mut shot.kind else {
        return false;
    };
    *left = left.saturating_sub(1);
    let (more, push) = (*left > 0, *push);
    // A bolt too short to reach its strike strikes as it goes.
    if shot.ticks == BOLT_STRIKE_TICKS || (!more && shot.ticks < BOLT_STRIKE_TICKS) {
        let push = (struck.centre - shot.position).normalize_or_zero() * push;
        out.hits.push(shot.hit(struck.entity, shot.damage, push));
        out.shot_event(shot, GunEventKind::Impact { at: struck.centre });
    }
    more
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_hero::ParryWorld;

    fn flux() -> (WeaponId, ScopeBeam) {
        let armory = Armory::uya();
        let (index, beam) = armory
            .weapons
            .iter()
            .enumerate()
            .find_map(|(index, def)| match &def.fire {
                FireMode::Projectile(fire) => fire.scope_beam.clone().map(|beam| (index, beam)),
                _ => None,
            })
            .expect("the Flux Rifle has a scoped beam");
        (WeaponId(index), beam)
    }

    fn target(raw: u32, z: f32) -> GunTarget {
        GunTarget {
            entity: Entity::from_raw_u32(raw).unwrap(),
            centre: Vec3::new(0.0, 0.0, z),
            radius: 0.5,
            height: 0.0,
            alive: true,
            health: None,
            enemy: true,
        }
    }

    /// The targets the beam hits in its first `ticks`, at `level`.
    fn struck(level: u8, targets: &[GunTarget], ticks: u32) -> Vec<Entity> {
        let (weapon, def) = flux();
        let mut shot = Shot {
            serial: 1,
            weapon,
            level,
            position: Vec3::ZERO,
            velocity: Vec3::NEG_Z * (def.step / TICK),
            ticks: 0,
            damage: 10.0,
            splash: 0.0,
            mods: Mods::default(),
            kind: ShotKind::Beam { hit: false },
        };
        let world = ParryWorld::default();
        let mut out = Outcome::default();
        for _ in 0..ticks {
            shot.ticks += 1;
            beam(&mut shot, &def, &world, targets, &mut out);
        }
        let mut hit: Vec<_> = out.hits.iter().map(|hit| hit.target).collect();
        hit.dedup();
        hit
    }

    #[test]
    fn at_v1_the_scoped_beam_stops_at_the_first_target() {
        // Three in a line within one 30 u step, 3 u apart: the 0.6 u ball reaches only
        // the first.
        let targets = [target(1, -40.0), target(2, -43.0), target(3, -46.0)];
        assert_eq!(struck(0, &targets, 4), [targets[0].entity]);
    }

    #[test]
    fn from_v2_it_passes_the_first_target_to_reach_the_second() {
        // Each test passes over only what the one before hit, so of three in a line it
        // goes back and forth between the first two, and stops.
        let targets = [target(1, -40.0), target(2, -43.0), target(3, -46.0)];
        let hit = struck(1, &targets, 26);
        assert!(hit.contains(&targets[0].entity) && hit.contains(&targets[1].entity));
        assert!(!hit.contains(&targets[2].entity));
    }

    #[test]
    fn the_beam_ends_after_its_ticks() {
        let (weapon, def) = flux();
        let mut shot = Shot {
            serial: 1,
            weapon,
            level: 0,
            position: Vec3::ZERO,
            velocity: Vec3::NEG_Z * (def.step / TICK),
            ticks: 0,
            damage: 10.0,
            splash: 0.0,
            mods: Mods::default(),
            kind: ShotKind::Beam { hit: false },
        };
        let world = ParryWorld::default();
        let mut out = Outcome::default();
        let mut alive = 0;
        loop {
            shot.ticks += 1;
            if !beam(&mut shot, &def, &world, &[], &mut out) {
                break;
            }
            alive += 1;
        }
        assert_eq!(alive + 1, def.ticks);
        assert!((shot.position.z + def.step * def.ticks as f32).abs() < 1e-2);
    }
}

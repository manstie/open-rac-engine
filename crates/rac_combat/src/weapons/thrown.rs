//! The guns that throw what they fire (`weapons.md`): the Bouncer's canister and its
//! bomblets, the Infector's blob, and the gloves' Miniturrets, holo shields and the Agents
//! of Doom's pods.

use bevy::{math::ops, prelude::*};
use rac_hero::{CollisionWorld, HeroState};
use rac_input::Action;
use rac_sim::SimRng;
use std::f32::consts::TAU;

use super::{
    Affliction, GunEventKind,
    aim::{ALL_ROUND, GunTarget, assist_targets, flat, in_sight, level_search, wrap, yaw_of},
    arsenal::{Arsenal, Landing},
    def::{
        AgentEnd, AgentGun, AgentHover, AgentMoves, Agents, AimAssist, Armory, Barrier, Bomblet,
        Bounce, Cluster, Deploy, FireMode, Gait, Infect, InfectBomb, Mega, Throw, Timing, Turret,
    },
    fire::{Afflict, Gun, GunInput, Outcome, fire_anim, run_dry, take_shot},
    grab,
    shots::{
        AgentBook, AgentMode, AgentState, BOUNCE_GAP, BarrierState, Drift, Flight, Hovering, Meets,
        Shot, ShotKind, TICK, TurretState, in_ball, meets, meets_where, seeker, small_missile,
        spring_turn,
    },
};

/// Ticks the kit lets a thrown thing fall before it gives it up, where the game's rule
/// needs what the kit doesn't know: the holo shield goes once it leaves the level's box
/// (`weapons.md`, "Thrown things off the world") [kit's own].
const FALL_TICKS: u32 = 600;

/// The agents' pod lets them out where it is once it has fallen this long unlanded
/// (0x35D178 sets its countdown).
const POD_TICKS: u32 = 300;

/// The launch that carries a throw from `from` to `to` across the ground at `ground_speed`,
/// its upward speed solved for `gravity` (`lob_velocity`, 0x4C3888).
fn arc(from: Vec3, to: Vec3, way: Vec3, ground_speed: f32, gravity: f32) -> Vec3 {
    let across = flat(to - from);
    let distance = across.length();
    let way = if distance > 1e-4 {
        across / distance
    } else {
        way
    };
    let seconds = (distance / ground_speed).max(TICK);
    let up = (to.y - from.y + 0.5 * gravity * seconds * seconds) / seconds;
    way * ground_speed + Vec3::Y * up
}

/// A throw from `from` (`lob_velocity`): at the lock-on target, else where the view centres
/// while guns aim there, else ahead at the height of his feet; kept between the throw's
/// least and most away across the ground.
fn throw(gun: &Gun, throw: &Throw, gun_input: &GunInput, from: Vec3) -> Vec3 {
    let hero = gun_input.hero;
    let most = if hero.state == HeroState::Look {
        throw.most_aimed
    } else {
        throw.most
    };
    let facing = flat(gun.facing).normalize_or(Vec3::NEG_Z);
    let point = match (gun.lock, gun.view_point) {
        (Some((_, at)), _) => at,
        (None, Some(view)) => Vec3::new(view.x, hero.position.y, view.z),
        (None, None) => hero.position + facing * most,
    };
    let across = flat(point - hero.position);
    let way = across.try_normalize().unwrap_or(facing);
    let kept = across.length().clamp(throw.least, most);
    let point = Vec3::new(hero.position.x, point.y, hero.position.z) + way * kept;
    arc(from, point, way, throw.ground_speed, throw.gravity)
}

/// The ticks a thrown thing's flight is followed for, to find where it lands (0x4C3E68).
const LANDING_TICKS: u32 = 300;

/// Where what the gun in his hand throws would land, if it throws one: its flight followed
/// from the muzzle until it meets the world (0x4C3E68, for 0x4C3500's marker).
pub(super) fn landing(gun: &Gun, gun_input: &GunInput) -> Option<Landing> {
    let throwing = match &gun.def.fire {
        FireMode::Cluster(fire) => fire.throw,
        FireMode::Infect(fire) => fire.bomb.filter(|bomb| gun.level >= bomb.from_level)?.throw,
        FireMode::Turret(fire) => fire.deploy.throw,
        FireMode::Barrier(fire) => fire.deploy.throw,
        FireMode::Agents(fire) => fire.deploy.throw,
        _ => return None,
    };
    let mut velocity = throw(gun, &throwing, gun_input, gun.muzzle);
    let mut at = gun.muzzle;
    for _ in 0..LANDING_TICKS {
        let motion = velocity * TICK;
        if let Some(hit) = gun_input.world.cast_ray(at, motion) {
            return Some(Landing {
                at: at + motion * hit.fraction,
                normal: hit.normal,
            });
        }
        at += motion;
        velocity.y -= throwing.gravity * TICK;
    }
    None
}

/// `velocity` reflected off a surface facing `normal`, keeping `keep` of its part along the
/// normal and all of it along the surface (UYA's resolver, 0x4C42C0).
pub(super) fn rebound(velocity: Vec3, normal: Vec3, keep: f32) -> Vec3 {
    let reflected = velocity.reflect(normal);
    let along = normal * reflected.dot(normal);
    reflected - along + along * keep
}

/// A bounce off the world ([`rebound`]), and if the bounce homes, its ground velocity turns
/// at the nearest living enemy within reach (the Bouncer's bomblets, 0x31E488, and the
/// Infector's blob).
fn bounce(shot: &mut Shot, normal: Vec3, bounce: &Bounce, targets: &[GunTarget]) {
    shot.velocity = rebound(shot.velocity, normal, bounce.keep);
    let Some(range) = bounce.home_range else {
        return;
    };
    let nearest = targets
        .iter()
        .filter(|target| target.alive && target.enemy)
        .map(|target| (flat(target.centre - shot.position).length(), target))
        .filter(|&(distance, _)| distance <= range)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, target)) = nearest {
        let way = flat(target.centre - shot.position).normalize_or_zero();
        let speed = flat(shot.velocity).length().max(bounce.home_speed);
        shot.velocity.x = way.x * speed;
        shot.velocity.z = way.z * speed;
    }
}

/// Falls a tick under `gravity`; where its step meets the world or a target, it lands
/// there and the point is returned.
fn fall(shot: &mut Shot, gravity: f32, flight: &Flight) -> Option<Vec3> {
    shot.velocity.y -= gravity * TICK;
    let motion = shot.velocity * TICK;
    match meets(flight.world, flight.targets, shot.position, motion) {
        Meets::Target(_, t) | Meets::World(t, _) => {
            shot.position += motion * t;
            Some(shot.position)
        }
        Meets::Nothing => {
            shot.position += motion;
            None
        }
    }
}

/// The Bouncer (0x31C890): held, a round throws a canister on the cooldown.
pub(super) fn cluster(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Cluster,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    let trigger = &mut arsenal.trigger;
    trigger.cooldown = trigger.cooldown.saturating_sub(1);
    if !gun_input.input.held(Action::Fire) || trigger.cooldown > 0 || !gun.ready {
        return;
    }
    if !take_shot(arsenal, armory, gun, out) {
        return;
    }
    arsenal.trigger.cooldown = Timing::Rate(fire.rate).ticks();
    let velocity = throw(gun, &fire.throw, gun_input, gun.muzzle);
    let serial = arsenal.next_serial();
    arsenal.shots.push(gun.shot(
        serial,
        gun.muzzle,
        velocity,
        ShotKind::Canister {
            bounces: 0,
            rolled: None,
        },
    ));
}

/// The Bouncer's canister (0x31D0B0) and bomblets: the canister bounces, then rolls, and
/// bursts near a target or when it has rolled a while, into bomblets that bounce towards
/// enemies and, once armed, burst on the first with hit points near them.
pub(super) fn cluster_shot(
    shot: &mut Shot,
    fire: &Cluster,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
) -> bool {
    if let ShotKind::Bomblet { index, life } = shot.kind {
        let (_, b) = fire.at(shot.level);
        return bomblet(shot, fire.gravity, b, index, life, flight, out);
    }
    let ShotKind::Canister { bounces, rolled } = &mut shot.kind else {
        return false;
    };
    if let Some(ticks) = rolled {
        *ticks += 1;
        shot.velocity.x *= fire.roll_keep;
        shot.velocity.z *= fire.roll_keep;
    }
    shot.velocity.y -= fire.gravity * TICK;
    let motion = shot.velocity * TICK;
    match flight.world.cast_ray(shot.position, motion) {
        Some(hit) => {
            shot.position += motion * hit.fraction + hit.normal * BOUNCE_GAP;
            if rolled.is_some() {
                // Rolling, it keeps to the surface.
                shot.velocity -= hit.normal * shot.velocity.dot(hit.normal);
            } else {
                *bounces += 1;
                if *bounces < fire.bounces {
                    shot.velocity = rebound(shot.velocity, hit.normal, fire.bounce.keep);
                } else {
                    // The last touch bounces only off a ceiling, then sets it rolling on
                    // along the surface at its rolling speed.
                    if hit.normal.y < 0.0 {
                        shot.velocity = rebound(shot.velocity, hit.normal, fire.bounce.keep);
                    }
                    let along = shot.velocity - hit.normal * shot.velocity.dot(hit.normal);
                    shot.velocity = along.normalize_or_zero() * fire.roll_speed(shot.level);
                    *rolled = Some(0);
                }
            }
        }
        None => shot.position += motion,
    }
    let (radius, b) = fire.at(shot.level);
    let near = in_ball(flight.targets, shot.position, radius + fire.reach, None)
        .any(|target| target.alive);
    let rolled_out = rolled.is_some_and(|ticks| ticks >= fire.roll_ticks);
    if !near && !rolled_out && shot.ticks < fire.lifetime {
        return true;
    }
    out.shot_event(
        shot,
        GunEventKind::Impact {
            at: shot.position,
            surface: None,
        },
    );
    scatter(
        shot,
        shot.position,
        fire.bomblets,
        b,
        shot.damage,
        rng,
        born,
    );
    false
}

/// Bursts into `count` bomblets at `at`, each doing `damage`: at random bearings, up from
/// the ground and at speeds within the bomblet's, living a random while (0x31D6C0).
pub(super) fn scatter(
    shot: &Shot,
    at: Vec3,
    count: u32,
    b: &Bomblet,
    damage: f32,
    rng: &mut SimRng,
    born: &mut Vec<Shot>,
) {
    for index in 0..count {
        let bearing = rng.unit_f32() * TAU;
        let up = rng.range_f32(b.up_degrees.0, b.up_degrees.1).to_radians();
        let speed = rng.range_f32(b.speed.0, b.speed.1);
        let (low, high) = b.life;
        let life = low + rng.below(high.saturating_sub(low) + 1);
        let direction = Vec3::new(
            -ops::sin(bearing) * ops::cos(up),
            ops::sin(up),
            -ops::cos(bearing) * ops::cos(up),
        );
        born.push(Shot {
            position: at,
            velocity: direction * speed,
            ticks: 0,
            damage,
            kind: ShotKind::Bomblet { index, life },
            ..shot.clone()
        });
    }
}

/// A bomblet (0x31E488), falling under `gravity`: it bounces towards enemies and, once
/// armed, bursts on the first target with hit points near it.
pub(super) fn bomblet(
    shot: &mut Shot,
    gravity: f32,
    b: &Bomblet,
    index: u32,
    life: u32,
    flight: &Flight,
    out: &mut Outcome,
) -> bool {
    shot.velocity.y -= gravity * TICK;
    let motion = shot.velocity * TICK;
    match flight.world.cast_ray(shot.position, motion) {
        Some(hit) => {
            shot.position += motion * hit.fraction + hit.normal * BOUNCE_GAP;
            bounce(shot, hit.normal, &b.bounce, flight.targets);
        }
        None => shot.position += motion,
    }
    if shot.ticks >= b.armed_ticks + index
        && let Some(target) = in_ball(flight.targets, shot.position, b.hit_radius, None)
            .find(|target| target.alive && target.health.is_some())
    {
        let push = shot.velocity * TICK;
        out.hits.push(shot.hit(target.entity, shot.damage, push));
        out.shot_event(
            shot,
            GunEventKind::Impact {
                at: shot.position,
                surface: None,
            },
        );
        return false;
    }
    // At the end of its life it bursts with no damage.
    if shot.ticks < life {
        return true;
    }
    out.shot_event(
        shot,
        GunEventKind::Impact {
            at: shot.position,
            surface: None,
        },
    );
    false
}

/// The Infector (0x3688E8): a press, a while since the last, takes a round and throws a
/// blob along his aim.
pub(super) fn infect(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Infect,
    gun_input: &GunInput,
    rng: &mut SimRng,
    out: &mut Outcome,
) {
    let trigger = &mut arsenal.trigger;
    trigger.cooldown = trigger.cooldown.saturating_sub(1);
    if !gun_input.input.pressed(Action::Fire) || trigger.cooldown > 0 || !gun.ready {
        return;
    }
    if !take_shot(arsenal, armory, gun, out) {
        return;
    }
    arsenal.trigger.cooldown = fire.cooldown_ticks;
    let from = gun.muzzle;
    if let Some(bomb) = fire.bomb.filter(|bomb| gun.level >= bomb.from_level) {
        let velocity = throw(gun, &bomb.throw, gun_input, from);
        let serial = arsenal.next_serial();
        arsenal
            .shots
            .push(gun.shot(serial, from, velocity, ShotKind::Bomb));
        return;
    }
    let direction = gun.view_aim(from, armory);
    let (low, high) = fire.life;
    let life = low + rng.below(high.saturating_sub(low) + 1);
    let serial = arsenal.next_serial();
    arsenal.shots.push(gun.shot(
        serial,
        from,
        direction * fire.speed,
        ShotKind::Blob {
            life,
            infected: Vec::new(),
        },
    ));
}

/// The Infector's blob (0x3DE4F0): it falls, slows and bounces towards enemies, infecting
/// the weak ones it touches as it flies on; touching anything else with hit points, at the
/// end of its life, or come to rest, it bursts, infecting the enemies round it and hitting
/// what is near with a small blast that infects too.
pub(super) fn blob(shot: &mut Shot, fire: &Infect, flight: &Flight, out: &mut Outcome) -> bool {
    if let (ShotKind::Bomb, Some(bomb)) = (&shot.kind, fire.bomb) {
        return infect_bomb(shot, fire, &bomb, flight, out);
    }
    shot.velocity.y -= fire.gravity * TICK;
    shot.velocity *= fire.drag;
    let motion = shot.velocity * TICK;
    match flight.world.cast_ray(shot.position, motion) {
        Some(hit) => {
            shot.position += motion * hit.fraction + hit.normal * BOUNCE_GAP;
            bounce(shot, hit.normal, &fire.bounce, flight.targets);
        }
        None => shot.position += motion,
    }
    let ShotKind::Blob { life, infected } = &mut shot.kind else {
        return false;
    };
    let mut burst = false;
    for target in in_ball(flight.targets, shot.position, fire.touch, None) {
        let Some((health, _)) = target.health.filter(|_| target.alive) else {
            continue;
        };
        if target.enemy && health <= fire.weak_health {
            if !infected.contains(&target.entity) {
                infected.push(target.entity);
                out.afflictions
                    .push(infection(target, shot.weapon, shot.damage, fire));
            }
        } else {
            burst = true;
        }
    }
    if !burst && shot.ticks < *life && shot.velocity.length() >= fire.rest_speed {
        return true;
    }
    let at = shot.position;
    for target in flight.targets {
        if !target.alive || !target.enemy || infected.contains(&target.entity) {
            continue;
        }
        let distance = target.centre.distance(at) - target.radius;
        // The burst's ball, then the blast's, which infects what it hits.
        if distance <= fire.burst_radius.max(fire.blast_radius) {
            infected.push(target.entity);
            out.afflictions
                .push(infection(target, shot.weapon, shot.damage, fire));
        }
    }
    for target in in_ball(flight.targets, at, fire.blast_radius, None) {
        let push = (target.centre - at).normalize_or_zero() * fire.blast_push;
        out.hits
            .push(shot.hit(target.entity, fire.blast_damage, push));
    }
    out.shot_event(shot, GunEventKind::Impact { at, surface: None });
    false
}

/// An infection lasts the gun's damage over the target's hit points of the infection's
/// ticks.
fn infection(target: &GunTarget, weapon: super::WeaponId, damage: f32, fire: &Infect) -> Afflict {
    let health = target.health.map_or(1.0, |(now, _)| now).max(1.0);
    Afflict {
        target: target.entity,
        weapon,
        affliction: Affliction::Infected {
            ticks: (damage / health * fire.infect_ticks).round() as u32,
        },
    }
}

/// A glove's throw (the Miniturret, Holo Shield and Agents of Doom): a press with room for
/// another plays the throw, and `throw_ticks` later it leaves his hand; the next can't
/// start for a while. A round goes with the press, or as it leaves for `round_on_leave`.
/// Returns true on the tick it leaves.
#[allow(clippy::too_many_arguments)]
fn deploy(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    deploy: &Deploy,
    gun_input: &GunInput,
    out_now: usize,
    round_on_leave: bool,
    out: &mut Outcome,
) -> bool {
    let trigger = &mut arsenal.trigger;
    trigger.cooldown = trigger.cooldown.saturating_sub(1);
    if let Some(left) = trigger.launch {
        if left > 1 {
            trigger.launch = Some(left - 1);
            return false;
        }
        trigger.launch = None;
        return !round_on_leave || arsenal.use_ammo(gun.weapon, 1, armory);
    }
    let press = gun_input.input.pressed(Action::Fire)
        || (gun_input.hero.state == HeroState::Look && gun_input.input.pressed(Action::LockOn));
    if !press || trigger.cooldown > 0 || !gun.ready || out_now >= deploy.most {
        return false;
    }
    if round_on_leave {
        if !arsenal.has_ammo(gun.weapon, armory) {
            return false;
        }
        fire_anim(arsenal, gun, out);
    } else if !take_shot(arsenal, armory, gun, out) {
        run_dry(arsenal);
        return false;
    }
    let trigger = &mut arsenal.trigger;
    trigger.launch = Some(deploy.throw_ticks.max(1));
    trigger.cooldown = deploy
        .gap_ticks
        .max(deploy.throw_ticks + deploy.ready_ticks);
    false
}

fn count(arsenal: &Arsenal, gun: &Gun, is: impl Fn(&ShotKind) -> bool) -> usize {
    arsenal
        .shots
        .iter()
        .filter(|shot| shot.weapon == gun.weapon && is(&shot.kind))
        .count()
}

/// The Miniturret Glove (0x346078): a press takes a round and throws a turret.
pub(super) fn turret(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Turret,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    // It refuses a throw by those landed (0x3462F0), six of the Megaturret's.
    let landed = count(
        arsenal,
        gun,
        |kind| matches!(kind, ShotKind::Turret(state) if state.landed.is_some()),
    );
    let most = fire
        .mega
        .filter(|mega| gun.level >= mega.from_level)
        .map_or(fire.deploy.most, |mega| mega.most);
    if !deploy(
        arsenal,
        armory,
        gun,
        &Deploy {
            most,
            ..fire.deploy
        },
        gun_input,
        landed,
        false,
        out,
    ) {
        return;
    }
    let velocity = throw(gun, &fire.deploy.throw, gun_input, gun.muzzle);
    let serial = arsenal.next_serial();
    arsenal.shots.push(gun.shot(
        serial,
        gun.muzzle,
        velocity,
        ShotKind::Turret(TurretState {
            // The Megaturret's first look at moving to him is 3 s in (0x32DE00's state 0).
            move_wait: fire.mega.map_or(0, |mega| mega.move_every.0),
            ..default()
        }),
    ));
}

/// A Miniturret (0x32AFB0) and its shots: it falls and lands, unfolds, then turns to the
/// enemy it finds all round and shoots it; it bursts after its shots, idle a long while, or
/// left far behind.
pub(super) fn turret_shot(
    shot: &mut Shot,
    fire: &Turret,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
) -> bool {
    if shot.kind == ShotKind::Pellet {
        return pellet(shot, fire.push, flight, out);
    }
    if let (ShotKind::Guided { .. }, Some(mega)) = (&shot.kind, fire.mega) {
        return guided(shot, &mega, flight, rng, out);
    }
    let ShotKind::Turret(mut state) = shot.kind else {
        return false;
    };
    let keep = turret_tick(shot, &mut state, fire, flight, rng, out, born);
    shot.kind = ShotKind::Turret(state);
    if !keep {
        out.shot_event(
            shot,
            GunEventKind::Impact {
                at: shot.position,
                surface: None,
            },
        );
    }
    keep
}

fn turret_tick(
    shot: &mut Shot,
    state: &mut TurretState,
    fire: &Turret,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
) -> bool {
    let mega = fire.mega.filter(|mega| shot.level >= mega.from_level);
    // Left far behind a Miniturret bursts; a Megaturret goes to him.
    if mega.is_none() && shot.position.distance(flight.hero) > fire.leash {
        return false;
    }
    // Its countdown starts as it leaves the hand (0x32C320) and whatever it is doing runs
    // on; each shot sets it again.
    state.idle += 1;
    if state.idle >= mega.map_or(fire.life_ticks, |mega| mega.life_ticks) {
        return false;
    }
    state.folding = state.folding.saturating_sub(1);
    let Some(landed) = &mut state.landed else {
        let way = flat(shot.velocity).normalize_or(Vec3::NEG_Z);
        if fall(shot, fire.gravity, flight).is_some() {
            shot.velocity = Vec3::ZERO;
            state.landed = Some(0);
            state.barrel = way;
            state.barrel_from = way;
        }
        return true;
    };
    // Folded up until it is among the nearest him, then it unfolds again.
    if state.move_to.is_none() && !flight.turrets.contains(&shot.serial) {
        if *landed > fire.unfold_ticks {
            state.folding = fire.unfold_ticks;
        }
        *landed = 0;
        state.target = None;
        state.turn = 0.0;
        state.barrel_from = state.barrel;
        return true;
    }
    *landed += 1;
    if *landed <= fire.unfold_ticks {
        return true;
    }
    if let Some(mega) = mega
        && move_to_him(shot, state, &mega, flight, rng)
    {
        return true;
    }
    let barrel =
        shot.position + Vec3::Y * mega.map_or(fire.barrel_height, |mega| mega.barrel_height);
    let targets = flight.targets;
    let kept = state.target.and_then(|entity| {
        targets.iter().find(|target| {
            target.entity == entity && target.alive && target.centre.distance(barrel) <= fire.range
        })
    });
    let target = match kept {
        Some(target) => target,
        None => {
            let round = AimAssist {
                range: fire.range,
                near: ALL_ROUND,
                far: ALL_ROUND,
            };
            let found = assist_targets(
                targets,
                barrel,
                Vec3::NEG_Z,
                &round,
                flight.world,
                |target| target.enemy,
                1,
            );
            state.turn = 0.0;
            state.barrel_from = state.barrel;
            state.target = found.first().map(|&index| targets[index].entity);
            let Some(&index) = found.first() else {
                return true;
            };
            &targets[index]
        }
    };
    state.turn = (state.turn + fire.turn_step).min(1.0);
    let at_target = (target.centre - barrel).normalize_or(state.barrel);
    state.barrel = state
        .barrel_from
        .lerp(at_target, ease_turn(state.turn))
        .normalize_or(at_target);
    state.cooldown = state.cooldown.saturating_sub(1);
    if state.turn <= fire.fire_share || state.cooldown > 0 {
        return true;
    }
    let rate = fire.rate.at(shot.level).unwrap_or(1.0);
    state.cooldown = mega.map_or(Timing::Rate(rate).ticks(), |mega| mega.every_ticks);
    state.fired += 1;
    state.idle = 0;
    let direction = (target.centre - barrel).normalize_or(Vec3::NEG_Z);
    out.shot_event(
        shot,
        GunEventKind::Fired {
            at: barrel,
            direction,
        },
    );
    born.push(match mega {
        Some(mega) => Shot {
            // From its two barrels in turn.
            position: barrel
                + direction * mega.barrels.0
                + direction.cross(Vec3::Y).normalize_or_zero()
                    * if state.fired.is_multiple_of(2) {
                        mega.barrels.1
                    } else {
                        -mega.barrels.1
                    },
            velocity: direction * mega.speed.0,
            ticks: 0,
            kind: ShotKind::Guided {
                target: Some(target.entity),
                turn: 0.0,
                drift: Drift::new(rng),
            },
            ..shot.clone()
        },
        None => Shot {
            position: barrel + direction * fire.barrel_along,
            velocity: direction * fire.speed,
            ticks: 0,
            kind: ShotKind::Pellet,
            ..shot.clone()
        },
    });
    state.fired < fire.shots.at(shot.level).unwrap_or(0)
}

/// The share of the barrel's turn its count has reached (0x442070: quick, then easing).
fn ease_turn(share: f32) -> f32 {
    1.0 - (1.0 - share) * (1.0 - share)
}

/// The Megaturret's move to him (0x32DE00's states 3, 10 and 11, 0x407010): with the hero
/// far off and enemies near him, now and then it fades out, goes to a point on the ground
/// near him, and fades back in. Returns whether it is moving.
fn move_to_him(
    shot: &mut Shot,
    state: &mut TurretState,
    mega: &Mega,
    flight: &Flight,
    rng: &mut SimRng,
) -> bool {
    if let Some(to) = state.move_to {
        state.faded += 1;
        if state.faded == mega.fade_ticks {
            shot.position = to;
        }
        if state.faded >= mega.fade_ticks * 2 {
            state.move_to = None;
            state.faded = 0;
            return false;
        }
        return true;
    }
    state.move_wait = state.move_wait.saturating_sub(1);
    let far = flight.hero.distance(shot.position) > mega.beyond;
    let threat = flight.targets.iter().any(|target| {
        target.alive && target.enemy && target.centre.distance(flight.hero) <= mega.beyond
    });
    if state.move_wait > 0 || !far || !threat {
        return false;
    }
    let (low, high) = mega.move_every;
    state.move_wait = low + rng.below(high.saturating_sub(low) + 1);
    let bearing = rng.unit_f32() * TAU;
    let reach = rng.range_f32(mega.round.0, mega.round.1);
    let above =
        flight.hero + Vec3::new(-ops::sin(bearing) * reach, 1.0, -ops::cos(bearing) * reach);
    // It stays if there's no ground under the point within 10 u (0x32E950).
    let down = Vec3::NEG_Y * 10.0;
    let Some(hit) = flight.world.cast_ray(above, down) else {
        return false;
    };
    state.move_to = Some(above + down * hit.fraction);
    state.faded = 0;
    true
}

/// The Megaturret's missile (0x343330): faster each tick to its most, after its target,
/// going off where it meets something.
fn guided(
    shot: &mut Shot,
    mega: &Mega,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
) -> bool {
    let ShotKind::Guided {
        target,
        turn,
        drift,
    } = &mut shot.kind
    else {
        return false;
    };
    if shot.ticks >= mega.missile_ticks {
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
    let speed = (shot.velocity.length() + mega.accel).min(mega.speed.1);
    let mut way = shot.velocity.normalize_or(flight.facing);
    if let Some(chased) = chased {
        let off = drift.aim(chased.centre, shot.position, rng) - shot.position;
        let close = off.length() < 0.25;
        way = spring_turn(way, off.normalize_or(way), turn, mega.turn, close);
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
    // Pushed by its velocity a frame.
    for near in in_ball(flight.targets, at, mega.blast_radius, None) {
        out.hits.push(shot.hit(near.entity, shot.damage, motion));
    }
    out.shot_event(shot, GunEventKind::Impact { at, surface: None });
    false
}

/// A turret's shot (0x277110's): straight on until its step meets something, a target
/// taking its damage.
fn pellet(shot: &mut Shot, push: f32, flight: &Flight, out: &mut Outcome) -> bool {
    let motion = shot.velocity * TICK;
    match meets(flight.world, flight.targets, shot.position, motion) {
        Meets::Target(index, t) => {
            let along = shot.velocity.normalize_or_zero() * push;
            out.hits
                .push(shot.hit(flight.targets[index].entity, shot.damage, along));
            out.shot_event(
                shot,
                GunEventKind::Impact {
                    at: shot.position + motion * t,
                    surface: None,
                },
            );
            false
        }
        Meets::World(t, _) => {
            out.shot_event(
                shot,
                GunEventKind::Impact {
                    at: shot.position + motion * t,
                    surface: None,
                },
            );
            false
        }
        Meets::Nothing => {
            shot.position += motion;
            shot.ticks < FALL_TICKS
        }
    }
}

/// The Holo Shield Glove (0x3717A8): a press with fewer than four out takes a round and
/// throws a shield.
pub(super) fn barrier(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Barrier,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    let out_now = count(arsenal, gun, |kind| matches!(kind, ShotKind::Barrier(_)));
    if !deploy(
        arsenal,
        armory,
        gun,
        &fire.deploy,
        gun_input,
        out_now,
        false,
        out,
    ) {
        return;
    }
    let velocity = throw(gun, &fire.deploy.throw, gun_input, gun.muzzle);
    let serial = arsenal.next_serial();
    arsenal.shots.push(gun.shot(
        serial,
        gun.muzzle,
        velocity,
        ShotKind::Barrier(BarrierState {
            yaw: gun_input.hero.yaw,
            hits_left: fire.hits,
            heal: fire.heal_every,
            drain: fire.drain_every,
            ..default()
        }),
    ));
}

/// The Infecto-Bomb (0x38E3A0): it falls until it meets something, and there its burst
/// (0x38EF90) infects every enemy near, a blast round it doing a little damage.
fn infect_bomb(
    shot: &mut Shot,
    fire: &Infect,
    bomb: &InfectBomb,
    flight: &Flight,
    out: &mut Outcome,
) -> bool {
    let Some(at) = fall(shot, bomb.throw.gravity, flight) else {
        return shot.ticks < FALL_TICKS;
    };
    for target in flight.targets {
        if target.alive && target.enemy && target.centre.distance(at) <= bomb.burst_radius {
            out.afflictions
                .push(infection(target, shot.weapon, shot.damage, fire));
        }
    }
    for target in in_ball(flight.targets, at, bomb.blast_radius, None) {
        let push = (target.centre - at).normalize_or_zero() * fire.blast_push;
        out.hits
            .push(shot.hit(target.entity, fire.blast_damage, push));
    }
    out.shot_event(shot, GunEventKind::Impact { at, surface: None });
    false
}

/// A holo shield (0x376B88): it falls until it meets something, hurting what is where it
/// lands, then stands as a wall that takes hits ([`block_shot`]) until its time or hits run
/// out or he leaves it behind, and folds away. From V3 each hit it takes sends a bolt at an
/// enemy near it; from V2 it gives him health while it stands.
pub(super) fn barrier_shot(
    shot: &mut Shot,
    fire: &Barrier,
    flight: &Flight,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
    book: &mut AgentBook,
) -> bool {
    if matches!(shot.kind, ShotKind::Pulse { .. }) {
        return pulse(shot, fire, flight, out);
    }
    let ShotKind::Barrier(mut state) = shot.kind else {
        return false;
    };
    let keep = barrier_tick(shot, &mut state, fire, flight, out, born, book);
    shot.kind = ShotKind::Barrier(state);
    keep
}

fn barrier_tick(
    shot: &mut Shot,
    state: &mut BarrierState,
    fire: &Barrier,
    flight: &Flight,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
    book: &mut AgentBook,
) -> bool {
    let Some(landed) = &mut state.landed else {
        let velocity = shot.velocity;
        let Some(at) = fall(shot, fire.gravity, flight) else {
            return shot.ticks < FALL_TICKS;
        };
        let push = velocity * TICK * fire.land_push;
        for target in in_ball(flight.targets, at, fire.land_radius, None) {
            out.hits
                .push(shot.hit(target.entity, fire.land_damage, push));
        }
        shot.velocity = Vec3::ZERO;
        state.landed = Some(0);
        // It unfolds where it lands (0x376B88's state 2).
        out.shot_event(shot, GunEventKind::Impact { at, surface: None });
        return true;
    };
    *landed += 1;
    if let Some(left) = &mut state.folding {
        *left = left.saturating_sub(1);
        return *left > 0;
    }
    if *landed >= fire.stand_ticks
        || state.hits_left == 0
        || shot.position.distance(flight.hero) > fire.leash
    {
        state.folding = Some(fire.fold_ticks.max(1));
        return true;
    }
    if let Some(left) = &mut state.bolt {
        *left = left.saturating_sub(1);
        if *left == 0 {
            state.bolt = None;
            let from = shot.position + Vec3::Y * fire.height / 2.0;
            let struck = flight.targets.iter().find(|target| {
                target.alive && target.enemy && target.centre.distance(from) <= fire.bolt_range
            });
            if let Some(struck) = struck {
                born.push(Shot {
                    position: from,
                    velocity: Vec3::ZERO,
                    ticks: 0,
                    kind: ShotKind::Bolt {
                        target: struck.entity,
                        left: fire.bolt_ticks,
                        push: fire.bolt_push,
                    },
                    ..shot.clone()
                });
            }
        }
    }
    if let Some(pulse) = fire.pulse
        && let Some(ticks) = &mut state.react
    {
        *ticks += 1;
        if (*ticks).is_multiple_of(pulse.every.max(1)) {
            let from = shot.position + Vec3::Y * fire.height / 2.0;
            let seen = flight
                .targets
                .iter()
                .filter(|target| target.alive && target.enemy)
                .find(|target| {
                    target.centre.distance(from) <= pulse.range
                        && !book.picked(shot.serial, target.entity)
                });
            if let Some(seen) = seen {
                book.pick(shot.serial, seen.entity, pulse.picked_ticks);
                born.push(Shot {
                    position: from,
                    velocity: Vec3::ZERO,
                    ticks: 0,
                    damage: shot.damage * pulse.share,
                    kind: ShotKind::Pulse {
                        target: seen.entity,
                    },
                    ..shot.clone()
                });
            }
        }
        if *ticks >= pulse.react_ticks {
            state.react = None;
        }
    }
    if fire.heal_from.is_some_and(|from| shot.level >= from) && flight.hurt {
        drain(shot, state, fire, flight, out);
    }
    true
}

/// An Ultrashield's pulse (0x371F48): at its target, faster each tick, striking the
/// first thing it meets; it fades out over its last ticks.
fn pulse(shot: &mut Shot, fire: &Barrier, flight: &Flight, out: &mut Outcome) -> bool {
    let (Some(pulse), ShotKind::Pulse { target }) = (fire.pulse, &shot.kind) else {
        return false;
    };
    if shot.ticks >= pulse.ticks {
        return false;
    }
    let way = flight
        .targets
        .iter()
        .find(|candidate| candidate.entity == *target)
        .map_or(shot.velocity, |target| target.centre - shot.position)
        .normalize_or(Vec3::NEG_Z);
    shot.velocity = way * (shot.velocity.length() + pulse.accel);
    let damage = if shot.ticks > pulse.fade_from {
        let left = pulse.ticks.saturating_sub(shot.ticks) as f32;
        shot.damage * left / (pulse.ticks - pulse.fade_from).max(1) as f32
    } else {
        shot.damage
    };
    let motion = shot.velocity * TICK;
    match meets(flight.world, flight.targets, shot.position, motion) {
        Meets::Target(index, t) => {
            shot.position += motion * t;
            out.hits
                .push(shot.hit(flight.targets[index].entity, damage, way * pulse.push));
            out.shot_event(
                shot,
                GunEventKind::Impact {
                    at: shot.position,
                    surface: None,
                },
            );
            false
        }
        Meets::World(t, _) => {
            shot.position += motion * t;
            out.shot_event(
                shot,
                GunEventKind::Impact {
                    at: shot.position,
                    surface: None,
                },
            );
            false
        }
        Meets::Nothing => {
            shot.position += motion;
            true
        }
    }
}

/// A holo shield's drain (0x37BB28, 0x37BDC0): while an enemy is within its reach, one
/// at a time they take a little damage on a shared timer, and he, if near, gains health a
/// point at a time, each point costing the shield a hit.
fn drain(
    shot: &Shot,
    state: &mut BarrierState,
    fire: &Barrier,
    flight: &Flight,
    out: &mut Outcome,
) {
    let from = shot.position + Vec3::Y * fire.height / 2.0;
    let mut linked = false;
    for enemy in in_ball(flight.targets, shot.position, fire.drain_reach, None) {
        if !enemy.alive || !enemy.enemy {
            continue;
        }
        linked = true;
        out.shot_event(
            shot,
            GunEventKind::Ray {
                from,
                to: enemy.centre,
            },
        );
        state.drain = state.drain.saturating_sub(1);
        if state.drain == 0 {
            state.drain = fire.drain_every;
            out.hits
                .push(shot.hit(enemy.entity, fire.drain_damage, Vec3::ZERO));
        }
    }
    if !linked || flight.hero.distance(shot.position) >= fire.heal_reach {
        return;
    }
    out.shot_event(
        shot,
        GunEventKind::Ray {
            from,
            to: flight.hero + Vec3::Y * 0.8,
        },
    );
    state.heal = state.heal.saturating_sub(1);
    if state.heal == 0 {
        state.heal = fire.heal_every;
        out.heal += 1;
        state.hits_left = state.hits_left.saturating_sub(1);
    }
}

/// Whether one of `arsenal`'s standing holo shields stops a shot moving from `from` by
/// `motion` this tick, and how far along: the shields are things the game's shots hit
/// ("Holo Shield Glove"). The shield takes the hit, and from V3 sends a bolt back.
pub fn block_shot(arsenal: &mut Arsenal, armory: &Armory, from: Vec3, motion: Vec3) -> Option<f32> {
    let mut nearest: Option<(f32, usize)> = None;
    for (index, shot) in arsenal.shots.iter().enumerate() {
        let ShotKind::Barrier(state) = &shot.kind else {
            continue;
        };
        let Some(FireMode::Barrier(fire)) = armory.get(shot.weapon).map(|def| &def.fire) else {
            continue;
        };
        if state.landed.is_none() || state.folding.is_some() || state.hits_left == 0 {
            continue;
        }
        let normal = Vec3::new(-ops::sin(state.yaw), 0.0, -ops::cos(state.yaw));
        let across = motion.dot(normal);
        if across.abs() < 1e-6 {
            continue;
        }
        let t = (shot.position - from).dot(normal) / across;
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let on = from + motion * t - shot.position;
        let side = Vec3::new(-normal.z, 0.0, normal.x);
        if on.dot(side).abs() > fire.half_width || on.y < 0.0 || on.y > fire.height {
            continue;
        }
        if nearest.is_none_or(|(least, _)| t < least) {
            nearest = Some((t, index));
        }
    }
    let (t, index) = nearest?;
    let shot = &mut arsenal.shots[index];
    let (bolt_from, pulses) = match armory.get(shot.weapon).map(|def| &def.fire) {
        Some(FireMode::Barrier(fire)) => {
            let pulses = fire
                .pulse
                .is_some_and(|pulse| shot.level >= pulse.from_level);
            let bolt = fire
                .bolt_from
                .filter(|&from| shot.level >= from && !pulses)
                .map(|_| fire.bolt_after);
            (bolt, pulses)
        }
        _ => (None, false),
    };
    if let ShotKind::Barrier(state) = &mut shot.kind {
        state.hits_left = state.hits_left.saturating_sub(1);
        if state.bolt.is_none() {
            state.bolt = bolt_from;
        }
        if pulses && state.react.is_none() {
            state.react = Some(0);
        }
    }
    let weapon = shot.weapon;
    let def = armory.get(weapon);
    let xp = def.map_or(0, |def| {
        let (times, over) = match &def.fire {
            FireMode::Barrier(fire) => fire.hit_xp,
            _ => (0, 1),
        };
        def.level(shot.level).xp * times / over.max(1)
    });
    arsenal.pending_xp.push((weapon, xp));
    Some(t)
}

/// The Agents of Doom (0x35C9F0): a press with fewer than the most about plays the throw,
/// and as the pod leaves his hand it takes a round; from V4 the agents come to him at once.
pub(super) fn agents(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Agents,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    let about = count(arsenal, gun, |kind| matches!(kind, ShotKind::Agent(_)));
    if !deploy(
        arsenal,
        armory,
        gun,
        &fire.deploy,
        gun_input,
        about,
        true,
        out,
    ) {
        return;
    }
    let hero = gun_input.hero;
    if fire.direct_from.is_some_and(|from| gun.level >= from) {
        let facing = flat(gun.facing).normalize_or(Vec3::NEG_Z);
        for at in round(hero.position, facing, fire.spread)
            .into_iter()
            .take(fire.per_pod as usize)
            .take(fire.deploy.most.saturating_sub(about))
        {
            // To a slot round him while one is free, else on foot beside him.
            let free = fire.hover.as_ref().and_then(|hover| {
                (0..hover.slots.len()).find(|slot| {
                    !arsenal.shots.iter().any(|shot| {
                        matches!(shot.kind, ShotKind::Agent(AgentState {
                            hover: Some(Hovering { slot: taken, landing: None }),
                            ..
                        }) if taken == *slot)
                    })
                })
            });
            let (at, hovering) = match (free, &fire.hover) {
                (Some(slot), Some(hover)) => (
                    slot_place(hover.slots[slot], hero.position, facing),
                    Some(Hovering {
                        slot,
                        landing: None,
                    }),
                ),
                _ => (at, None),
            };
            let serial = arsenal.next_serial();
            arsenal.shots.push(gun.shot(
                serial,
                at,
                Vec3::ZERO,
                ShotKind::Agent(AgentState {
                    hover: hovering,
                    ..Default::default()
                }),
            ));
        }
        return;
    }
    let velocity = throw(gun, &fire.deploy.throw, gun_input, gun.muzzle);
    let serial = arsenal.next_serial();
    arsenal
        .shots
        .push(gun.shot(serial, gun.muzzle, velocity, ShotKind::Pod));
}

/// A slot round the hero (0x4C5220): turned over his head from his left, and out from a
/// point behind him.
fn slot_place((degrees, out, behind): (f32, f32, f32), hero: Vec3, facing: Vec3) -> Vec3 {
    let left = Vec3::Y.cross(facing).normalize_or_zero();
    let angle = degrees.to_radians();
    hero - facing * behind + (left * ops::cos(angle) + Vec3::Y * ops::sin(angle)) * out
}

/// An agent's tick by its slot: it springs to the slot, or once the rounds are gone, to the
/// ground, where it goes on foot.
fn hover_tick(
    shot: &mut Shot,
    state: &mut AgentState,
    hover: &AgentHover,
    flight: &Flight,
    rng: &mut SimRng,
    book: &AgentBook,
) {
    let Some(mut now) = state.hover else {
        return;
    };
    if now.landing.is_none() && book.rounds == 0 {
        let way = flight.facing * hover.ahead - Vec3::Y * hover.below;
        now.landing = Some(
            flight
                .world
                .cast_ray(shot.position, way)
                .map_or(flight.hero, |hit| shot.position + way * hit.fraction),
        );
    }
    let goal = now
        .landing
        .unwrap_or_else(|| slot_place(hover.slots[now.slot], flight.hero, flight.facing));
    let from_him = shot.position.distance(flight.hero);
    let far = now.landing.is_none() && from_him > hover.far;
    let (gain, top) = if now.landing.is_some() {
        (draw(rng, hover.land_gain), draw(rng, hover.land_top))
    } else if far {
        (hover.far_gain, hover.far_top)
    } else {
        (draw(rng, hover.near_gain), draw(rng, hover.near_top))
    };
    let way = goal - shot.position;
    let mut left = way.length();
    let along = way.normalize_or_zero();
    let before = left;
    let mut speed = state.speed;
    grab::spring_towards(&mut left, &mut speed, gain / 3600.0, top / 60.0);
    let step = along * (before - left);
    let look = step + along * hover.look;
    let free = far || now.landing.is_some();
    if free || flight.world.cast_ray(shot.position, look).is_none() {
        shot.position += step;
        state.speed = speed;
    }
    if let Some(landing) = now.landing
        && shot.position.distance(landing) < hover.landed
    {
        state.hover = None;
        state.mode = AgentMode::Follow;
        state.speed = 0.0;
        return;
    }
    state.hover = Some(now);
}

/// To the front, back, left and right of `at`.
fn round(at: Vec3, facing: Vec3, spread: f32) -> [Vec3; 4] {
    let side = Vec3::new(-facing.z, 0.0, facing.x);
    [
        at + facing * spread,
        at - facing * spread,
        at - side * spread,
        at + side * spread,
    ]
}

/// The pod (0x35CEC8) falls and lets its agents out where it lands; an agent (0x3BD298)
/// keeps by the hero until it finds a target, runs at it, and blows up when it gets there.
#[allow(clippy::too_many_arguments)]
pub(super) fn agents_shot(
    shot: &mut Shot,
    fire: &Agents,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
    about: &mut usize,
    book: &mut AgentBook,
) -> bool {
    match shot.kind.clone() {
        ShotKind::Pod => {
            let way = flat(shot.velocity).normalize_or(Vec3::NEG_Z);
            let at = match fall(shot, fire.gravity, flight) {
                Some(at) => at,
                None if shot.ticks >= POD_TICKS => shot.position,
                None => return true,
            };
            for spot in round(at, way, fire.spread)
                .into_iter()
                .take(fire.per_pod as usize)
            {
                if *about >= fire.deploy.most {
                    break;
                }
                *about += 1;
                born.push(Shot {
                    position: spot,
                    velocity: Vec3::ZERO,
                    ticks: 0,
                    kind: ShotKind::Agent(AgentState::default()),
                    ..shot.clone()
                });
            }
            false
        }
        ShotKind::Agent(mut state) => {
            let keep = agent(shot, &mut state, fire, flight, rng, out, born, book);
            shot.kind = ShotKind::Agent(state);
            if !keep {
                *about = about.saturating_sub(1);
            }
            keep
        }
        ShotKind::Comet { life, .. } => comet(shot, life, fire, flight, out),
        ShotKind::Blast { .. } => grow(shot, fire, flight, out),
        ShotKind::Seeker { .. } => match fire.gun.at(shot.level) {
            Some(AgentGun::Missiles { missile, .. }) => seeker(shot, &missile, flight, rng, out),
            _ => false,
        },
        _ => false,
    }
}

fn draw(rng: &mut SimRng, (low, high): (f32, f32)) -> f32 {
    if high > low {
        rng.range_f32(low, high)
    } else {
        low
    }
}

fn draw_ticks(rng: &mut SimRng, (low, high): (u32, u32)) -> u32 {
    low + rng.below(high.saturating_sub(low) + 1)
}

/// An agent's tick (0x3BD298): its search, its end, its gun and its walk.
#[allow(clippy::too_many_arguments)]
fn agent(
    shot: &mut Shot,
    state: &mut AgentState,
    fire: &Agents,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
    book: &mut AgentBook,
) -> bool {
    let sight = &fire.sight;
    let level = Some(sight.level_degrees.to_radians());
    let bearing = sight.bearing_degrees.to_radians();
    let targets = flight.targets;
    let claimed = |book: &AgentBook, target: &GunTarget| {
        book.claims
            .iter()
            .any(|(entity, _)| *entity == target.entity)
    };
    if !state.out {
        // As it comes out: its speeds, its gun's rounds, and the best of anything with hit
        // points near.
        state.out = true;
        state.yaw = yaw_of(flight.facing);
        state.follow_top = draw(rng, fire.moves.follow.top);
        state.chase_top = draw(rng, fire.moves.chase.top);
        match fire.gun.at(shot.level) {
            Some(AgentGun::Bullets {
                first_ticks,
                rounds,
                shared,
                ..
            }) => {
                state.cooldown = first_ticks;
                if shared {
                    book.rounds = rounds;
                } else {
                    state.rounds = rounds;
                }
            }
            Some(AgentGun::Missiles { every, rounds, .. }) => {
                state.cooldown = draw_ticks(rng, every);
                book.rounds = rounds;
            }
            None => {}
        }
        state.sequence = moves_first_idle(&fire.moves);
        state.slow = true;
        if state.hover.is_none()
            && let Some(index) =
                level_search(targets, shot.position, bearing, sight.first, level, |t| {
                    t.alive && t.health.is_some()
                })
        {
            state.target = Some(targets[index].entity);
            state.fast = true;
            book.claims.push((targets[index].entity, sight.claim_ticks));
        }
    }
    // Keeping to its slot it only fires.
    if let (Some(_), Some(hover)) = (state.hover, &fire.hover) {
        if shot.ticks >= fire.life_ticks || shot.position.distance(flight.hero) > fire.leash {
            agent_end(shot, None, fire, flight, out, born);
            return false;
        }
        hover_tick(shot, state, hover, flight, rng, book);
        agent_gun(shot, state, fire, flight, rng, born, book);
        return true;
    }
    // Its target while it lives; without one, the best enemy no agent has claimed.
    let kept = state.target.and_then(|entity| {
        targets
            .iter()
            .find(|candidate| candidate.entity == entity && candidate.alive)
    });
    let chased = kept.or_else(|| {
        let index = level_search(targets, shot.position, bearing, sight.range, level, |t| {
            t.alive && t.enemy && !claimed(book, t)
        })?;
        book.claims.push((targets[index].entity, sight.claim_ticks));
        Some(&targets[index])
    });
    state.chase = match (chased, kept) {
        (Some(_), Some(_)) => state.chase + 1,
        (Some(_), None) => 1,
        (None, _) => 0,
    };
    state.target = chased.map(|chased| chased.entity);
    let eye = shot.position + Vec3::Y * fire.touch_height;
    let ended = shot.ticks >= fire.life_ticks
        || shot.position.distance(flight.hero) > fire.leash
        || state.chase > fire.chase_ticks
        || chased.is_some_and(|chased| {
            chased.centre.distance(eye) <= fire.touch + chased.radius
                || shot.position.distance(chased.feet()) < fire.reach
        });
    if ended {
        agent_end(shot, chased, fire, flight, out, born);
        return false;
    }
    agent_gun(shot, state, fire, flight, rng, born, book);

    let moves = &fire.moves;
    let away = flat(flight.hero - shot.position).length();
    if chased.is_none() {
        // Idle and wandering, it follows once he is far enough off.
        let leave = match state.mode {
            AgentMode::Idle => Some(moves.follow_beyond),
            AgentMode::Wander(_) => Some(moves.wander_leave),
            AgentMode::Follow => None,
        };
        state.mode = match state.mode {
            _ if leave.is_some_and(|leave| away > leave) => {
                state.near = draw(rng, moves.idle_within);
                AgentMode::Follow
            }
            AgentMode::Idle => {
                state.idle += 1;
                let (mut wander, sequence) = (false, state.sequence);
                let idle = moves.idles.iter().find(|idle| idle.sequence == sequence);
                let speed = if state.slow { 2 } else { 1 };
                if let Some(idle) = idle.filter(|idle| state.idle >= idle.ticks * speed) {
                    state.idle = 0;
                    state.slow = false;
                    state.sequence = idle.next[rng.below(2) as usize];
                    wander = rng.below(moves.wander_odds.max(1)) == 0;
                }
                if wander {
                    let around = rng.unit_f32() * TAU;
                    let off = Vec3::new(-ops::sin(around), 0.0, -ops::cos(around));
                    AgentMode::Wander(flight.hero + off * draw(rng, moves.wander_round))
                } else {
                    AgentMode::Idle
                }
            }
            AgentMode::Wander(to) if flat(to - shot.position).length() <= moves.wander_stop => {
                AgentMode::Idle
            }
            AgentMode::Follow if away <= state.near => AgentMode::Idle,
            mode => mode,
        };
        if state.mode != AgentMode::Idle {
            state.idle = 0;
            state.sequence = moves_first_idle(moves);
            state.slow = true;
        }
    }
    let goal = match (chased, state.mode) {
        (Some(chased), _) => Some((chased.feet(), &moves.chase, state.chase_top, moves.stop)),
        (None, AgentMode::Idle) => None,
        (None, AgentMode::Wander(to)) => {
            let top = (moves.wander.top.0 + moves.wander.top.1) / 2.0;
            Some((to, &moves.wander, top, moves.wander_stop))
        }
        (None, AgentMode::Follow) => {
            Some((flight.hero, &moves.follow, state.follow_top, moves.stop))
        }
    };
    match goal {
        Some((to, gait, top, stop)) => {
            let way = steer(
                state,
                shot.position,
                to,
                gait,
                top,
                stop,
                moves.facing_degrees,
            );
            walk(shot, way, fire.gravity, flight.world);
        }
        None => {
            state.speed = 0.0;
            state.turn = 0.0;
            walk(shot, Vec3::ZERO, fire.gravity, flight.world);
        }
    }
    true
}

/// The idle sequence an agent starts on.
fn moves_first_idle(moves: &AgentMoves) -> u8 {
    moves.idles.first().map_or(0, |idle| idle.sequence)
}

/// A walker's step towards `to` (0x4577D0): it turns, faster each tick and easing to a
/// stop as it comes round (`angle_approach`, 0x445758), and moves only while facing
/// within `facing_degrees` of it, gaining speed up to its top but no faster than lets it
/// stop `stop` short. Its velocity across the ground.
fn steer(
    state: &mut AgentState,
    at: Vec3,
    to: Vec3,
    gait: &Gait,
    top: f32,
    stop: f32,
    facing_degrees: f32,
) -> Vec3 {
    let across = flat(to - at);
    let off = wrap(yaw_of(across) - state.yaw);
    let accel = gait.turn_accel.to_radians();
    let ease = (2.0 * accel * off.abs()).sqrt();
    state.turn = (state.turn + accel)
        .min(gait.turn_top.to_radians())
        .min(ease);
    state.yaw = wrap(state.yaw + off.signum() * state.turn.min(off.abs()));
    let room = (across.length() - stop).max(0.0);
    let most = top.min((2.0 * gait.accel * room).sqrt());
    state.speed = if wrap(yaw_of(across) - state.yaw).abs() <= facing_degrees.to_radians() {
        (state.speed + gait.accel * TICK).min(most)
    } else {
        0.0
    };
    Vec3::new(-ops::sin(state.yaw), 0.0, -ops::cos(state.yaw)) * state.speed
}

/// An agent's gun (`agent_tick`, 0x3C0128): small bullets at V2–V4, the small missile
/// from V5 while it keeps by the hero.
fn agent_gun(
    shot: &Shot,
    state: &mut AgentState,
    fire: &Agents,
    flight: &Flight,
    rng: &mut SimRng,
    born: &mut Vec<Shot>,
    book: &mut AgentBook,
) {
    let Some(gun) = fire.gun.at(shot.level) else {
        return;
    };
    state.cooldown = state.cooldown.saturating_sub(1);
    if state.cooldown > 0 {
        return;
    }
    let targets = flight.targets;
    match gun {
        AgentGun::Bullets {
            range,
            bearing_degrees,
            height,
            jitter,
            speed,
            share,
            every_ticks,
            shared,
            ..
        } => {
            let rounds = if shared {
                &mut book.rounds
            } else {
                &mut state.rounds
            };
            let from = shot.position + Vec3::Y * height;
            let Some(index) = level_search(
                targets,
                shot.position,
                bearing_degrees.to_radians(),
                range,
                None,
                |t| t.alive && t.enemy,
            )
            .filter(|&index| *rounds > 0 && in_sight(flight.world, from, targets[index].centre)) else {
                return;
            };
            let off = Vec3::new(
                rng.range_f32(-jitter, jitter),
                rng.range_f32(-jitter, jitter),
                rng.range_f32(-jitter, jitter),
            );
            let way = (targets[index].centre + off - from).normalize_or(Vec3::NEG_Z);
            *rounds -= 1;
            state.cooldown = every_ticks;
            born.push(Shot {
                position: from,
                velocity: way * speed,
                ticks: 0,
                damage: shot.damage * share,
                kind: ShotKind::Slug,
                ..shot.clone()
            });
        }
        AgentGun::Missiles {
            range,
            height,
            every,
            missile,
            ..
        } => {
            if state.target.is_some() || book.rounds == 0 {
                return;
            }
            let from = shot.position + Vec3::Y * height;
            let Some(target) = targets
                .iter()
                .filter(|t| t.alive && t.enemy && t.centre.distance(from) <= range)
                .filter(|t| in_sight(flight.world, from, t.centre))
                .min_by(|a, b| a.centre.distance(from).total_cmp(&b.centre.distance(from)))
            else {
                return;
            };
            book.rounds -= 1;
            state.cooldown = draw_ticks(rng, every);
            let way = (target.centre - from).normalize_or(Vec3::NEG_Z);
            born.push(small_missile(
                shot,
                &missile,
                from,
                way,
                Some(target.entity),
                rng,
            ));
        }
    }
}

/// How an agent ends (0x3C2140): a blast at V1, a comet at V2–V4 (0x395480), the
/// Agents of Dread's growing blast from V5 (0x3914C0).
fn agent_end(
    shot: &Shot,
    chased: Option<&GunTarget>,
    fire: &Agents,
    flight: &Flight,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
) {
    let at = shot.position + Vec3::Y * fire.touch_height;
    match fire.end.at(shot.level) {
        Some(AgentEnd::Blast { radius, push }) => {
            for near in in_ball(flight.targets, at, radius, None) {
                let away = (near.centre - at).normalize_or_zero() * push;
                out.hits.push(shot.hit(near.entity, shot.damage, away));
            }
            out.shot_event(shot, GunEventKind::Impact { at, surface: None });
        }
        Some(AgentEnd::Comet { speed, life, .. }) => {
            let way = chased.map_or(flight.facing, |chased| {
                (chased.centre - at).normalize_or(flight.facing)
            });
            // Its speed: halfway between the two, as the random draw's middle.
            born.push(Shot {
                position: at,
                velocity: way * (speed.0 + speed.1) / 2.0,
                ticks: 0,
                kind: ShotKind::Comet { life, target: None },
                ..shot.clone()
            });
        }
        Some(AgentEnd::Grow { .. }) => born.push(Shot {
            position: at,
            velocity: Vec3::ZERO,
            ticks: 0,
            kind: ShotKind::Blast {
                radius: 0.0,
                struck: Vec::new(),
            },
            ..shot.clone()
        }),
        None => {}
    }
}

/// An agent's comet (0x394068): on until it meets something, which a blast round it
/// strikes, or its time runs out.
fn comet(shot: &mut Shot, life: u32, fire: &Agents, flight: &Flight, out: &mut Outcome) -> bool {
    let Some(AgentEnd::Comet { radius, push, .. }) = fire.end.at(shot.level) else {
        return false;
    };
    let motion = shot.velocity * TICK;
    let at = match meets_where(flight.world, flight.targets, shot.position, motion, |t| {
        t.alive
    }) {
        Meets::Target(_, t) | Meets::World(t, _) => shot.position + motion * t,
        Meets::Nothing => {
            shot.position += motion;
            return shot.ticks < life;
        }
    };
    for near in in_ball(flight.targets, at, radius, None) {
        let away = (near.centre - at).normalize_or_zero() * push;
        out.hits.push(shot.hit(near.entity, shot.damage, away));
    }
    out.shot_event(shot, GunEventKind::Impact { at, surface: None });
    false
}

/// The Agents of Dread's blast (0x3914C0): it grows to its radius, striking each target
/// it reaches once [the game strikes them each tick it touches them].
fn grow(shot: &mut Shot, fire: &Agents, flight: &Flight, out: &mut Outcome) -> bool {
    let Some(AgentEnd::Grow {
        radius: most,
        ticks,
        push,
    }) = fire.end.at(shot.level)
    else {
        return false;
    };
    let ShotKind::Blast { radius, struck } = &mut shot.kind else {
        return false;
    };
    *radius = most * (shot.ticks as f32 / ticks.max(1) as f32).min(1.0);
    let reached: Vec<&GunTarget> = in_ball(flight.targets, shot.position, *radius, None)
        .filter(|near| !struck.contains(&near.entity))
        .collect();
    struck.extend(reached.iter().map(|near| near.entity));
    for near in reached {
        let away = (near.centre - shot.position).normalize_or_zero() * push;
        out.hits.push(shot.hit(near.entity, shot.damage, away));
    }
    shot.ticks < ticks
}

/// A step along the ground: across it unless a wall is in the way, then down onto it or
/// falling.
pub(super) fn walk(shot: &mut Shot, across: Vec3, gravity: f32, world: &dyn CollisionWorld) {
    const BODY: f32 = 0.3;
    let step = across * TICK;
    let body = shot.position + Vec3::Y * BODY;
    if step != Vec3::ZERO && world.cast_ray(body, step).is_none() {
        shot.position += step;
    }
    shot.velocity.y -= gravity * TICK;
    let drop = Vec3::Y * (shot.velocity.y * TICK - BODY);
    match world.cast_ray(shot.position + Vec3::Y * BODY, drop) {
        Some(ground) if shot.velocity.y <= 0.0 => {
            shot.position.y += BODY + drop.y * ground.fraction;
            shot.velocity.y = 0.0;
        }
        _ => shot.position.y += shot.velocity.y * TICK,
    }
}

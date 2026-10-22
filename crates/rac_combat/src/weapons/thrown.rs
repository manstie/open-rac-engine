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
    aim::{ALL_ROUND, GunTarget, assist_targets, flat, in_sight},
    arsenal::Arsenal,
    def::{
        AgentEnd, Agents, AimAssist, Armory, Barrier, Bounce, Cluster, Deploy, FireMode, Infect,
        InfectBomb, Mega, Throw, Timing, Turret,
    },
    fire::{Afflict, Gun, GunInput, Outcome, fire_anim, run_dry, take_shot},
    shots::{
        BOUNCE_GAP, BarrierState, Flight, Meets, Shot, ShotKind, TICK, TurretState, in_ball, meets,
        meets_where,
    },
};

/// Ticks a thrown thing may fall before it is given up [placeholder: none read].
const FALL_TICKS: u32 = 600;

/// A walking turret falls as the agents do [placeholder].
const GRAVITY_WALK: f32 = 11.0;

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

/// A bounce off the world: `keep` of the speed comes back, and if the bounce homes, its
/// ground velocity turns at the nearest living enemy within reach (the Bouncer's bomblets,
/// 0x31E488, and the Infector's blob).
fn bounce(shot: &mut Shot, normal: Vec3, bounce: &Bounce, targets: &[GunTarget]) {
    shot.velocity = shot.velocity.reflect(normal) * bounce.keep;
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
        return bomblet(shot, fire, index, life, flight, out);
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
                shot.velocity = shot.velocity.reflect(hit.normal) * fire.bounce.keep;
                *bounces += 1;
                if *bounces >= fire.bounces {
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
    out.event(shot.weapon, GunEventKind::Impact { at: shot.position });
    for index in 0..fire.bomblets {
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
            velocity: direction * speed,
            ticks: 0,
            kind: ShotKind::Bomblet { index, life },
            ..shot.clone()
        });
    }
    false
}

fn bomblet(
    shot: &mut Shot,
    fire: &Cluster,
    index: u32,
    life: u32,
    flight: &Flight,
    out: &mut Outcome,
) -> bool {
    let (_, b) = fire.at(shot.level);
    shot.velocity.y -= fire.gravity * TICK;
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
        out.event(shot.weapon, GunEventKind::Impact { at: shot.position });
        return false;
    }
    // At the end of its life it bursts with no damage.
    shot.ticks < life
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
    let from = gun.muzzle - Vec3::Y * fire.below;
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
    out.event(shot.weapon, GunEventKind::Impact { at });
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
    let out_now = count(arsenal, gun, |kind| matches!(kind, ShotKind::Turret(_)));
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
        ShotKind::Turret(TurretState {
            // The Megaturret's first look at walking back is 3 s in (0x32DE00's state 0).
            walk_wait: fire.mega.map_or(0, |mega| mega.walk_every.0),
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
        return guided(shot, &mega, flight, out);
    }
    let ShotKind::Turret(mut state) = shot.kind else {
        return false;
    };
    let keep = turret_tick(shot, &mut state, fire, flight, rng, out, born);
    shot.kind = ShotKind::Turret(state);
    if !keep {
        out.event(shot.weapon, GunEventKind::Impact { at: shot.position });
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
    if shot.position.distance(flight.hero) > fire.leash {
        return false;
    }
    let Some(landed) = &mut state.landed else {
        if fall(shot, fire.gravity, flight).is_some() {
            shot.velocity = Vec3::ZERO;
            state.landed = Some(0);
        }
        return shot.ticks < FALL_TICKS;
    };
    *landed += 1;
    if *landed <= fire.unfold_ticks {
        return true;
    }
    let mega = fire.mega.filter(|mega| shot.level >= mega.from_level);
    if let Some(mega) = mega
        && walk_back(shot, state, &mega, flight, rng)
    {
        return true;
    }
    state.idle += 1;
    if state.idle >= fire.life_ticks {
        return false;
    }
    let barrel = shot.position + Vec3::Y * fire.barrel_height;
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
            state.target = found.first().map(|&index| targets[index].entity);
            let Some(&index) = found.first() else {
                return true;
            };
            &targets[index]
        }
    };
    state.turn = (state.turn + fire.turn_step).min(1.0);
    state.cooldown = state.cooldown.saturating_sub(1);
    if state.turn <= fire.fire_share || state.cooldown > 0 {
        return true;
    }
    let rate = fire.rate.at(shot.level).unwrap_or(1.0);
    state.cooldown = mega.map_or(Timing::Rate(rate).ticks(), |mega| mega.every_ticks);
    state.fired += 1;
    state.idle = 0;
    let direction = (target.centre - barrel).normalize_or(Vec3::NEG_Z);
    out.event(
        shot.weapon,
        GunEventKind::Fired {
            at: barrel,
            direction,
        },
    );
    born.push(match mega {
        Some(mega) => Shot {
            position: barrel,
            velocity: direction * mega.speed.0,
            ticks: 0,
            kind: ShotKind::Guided {
                target: Some(target.entity),
            },
            ..shot.clone()
        },
        None => Shot {
            position: barrel,
            velocity: direction * fire.speed,
            ticks: 0,
            kind: ShotKind::Pellet,
            ..shot.clone()
        },
    });
    state.fired < fire.shots.at(shot.level).unwrap_or(0)
}

/// The Megaturret's walk back (0x32DE00's states 3, 10 and 11): with the hero far off and
/// enemies near him, now and then it walks to a point near him. Returns whether it is
/// walking.
fn walk_back(
    shot: &mut Shot,
    state: &mut TurretState,
    mega: &Mega,
    flight: &Flight,
    rng: &mut SimRng,
) -> bool {
    if let Some(to) = state.walk_to {
        let across = flat(to - shot.position);
        if across.length() <= mega.walk_speed * TICK {
            state.walk_to = None;
            return false;
        }
        walk(
            shot,
            across.normalize_or_zero() * mega.walk_speed,
            GRAVITY_WALK,
            flight.world,
        );
        return true;
    }
    state.walk_wait = state.walk_wait.saturating_sub(1);
    let far = flat(flight.hero - shot.position).length() > mega.beyond;
    let threat = flight.targets.iter().any(|target| {
        target.alive && target.enemy && target.centre.distance(flight.hero) <= mega.beyond
    });
    if state.walk_wait > 0 || !far || !threat {
        return false;
    }
    let (low, high) = mega.walk_every;
    state.walk_wait = low + rng.below(high.saturating_sub(low) + 1);
    let bearing = rng.unit_f32() * TAU;
    let reach = rng.range_f32(mega.round.0, mega.round.1);
    state.walk_to =
        Some(flight.hero + Vec3::new(-ops::sin(bearing), 0.0, -ops::cos(bearing)) * reach);
    true
}

/// The Megaturret's missile (0x343330): faster each tick to its most, after its target,
/// going off where it meets something.
fn guided(shot: &mut Shot, mega: &Mega, flight: &Flight, out: &mut Outcome) -> bool {
    let ShotKind::Guided { target } = &mut shot.kind else {
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
        let wanted = (chased.centre - shot.position).normalize_or(way);
        way = super::aim::bend(way, wanted, mega.turn_degrees.to_radians());
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
    for near in in_ball(flight.targets, at, mega.blast_radius, None) {
        out.hits.push(shot.hit(near.entity, shot.damage, way * 0.5));
    }
    out.event(shot.weapon, GunEventKind::Impact { at });
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
            out.event(
                shot.weapon,
                GunEventKind::Impact {
                    at: shot.position + motion * t,
                },
            );
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
    out.event(shot.weapon, GunEventKind::Impact { at });
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
) -> bool {
    if matches!(shot.kind, ShotKind::Pulse { .. }) {
        return pulse(shot, fire, flight, out);
    }
    let ShotKind::Barrier(mut state) = shot.kind else {
        return false;
    };
    let keep = barrier_tick(shot, &mut state, fire, flight, out, born);
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
                        && in_sight(flight.world, from, target.centre)
                });
            if let Some(seen) = seen {
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
            out.event(shot.weapon, GunEventKind::Impact { at: shot.position });
            false
        }
        Meets::World(t, _) => {
            shot.position += motion * t;
            out.event(shot.weapon, GunEventKind::Impact { at: shot.position });
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
        out.event(
            shot.weapon,
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
    out.event(
        shot.weapon,
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
    let about = count(arsenal, gun, |kind| matches!(kind, ShotKind::Agent { .. }));
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
            let serial = arsenal.next_serial();
            arsenal.shots.push(gun.shot(
                serial,
                at,
                Vec3::ZERO,
                ShotKind::Agent {
                    target: None,
                    chase: 0,
                },
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
/// runs at the nearest enemy it can see, else keeps with the hero, and blows up when it
/// touches its target.
pub(super) fn agents_shot(
    shot: &mut Shot,
    fire: &Agents,
    flight: &Flight,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
    about: &mut usize,
) -> bool {
    match shot.kind.clone() {
        ShotKind::Pod => {
            let way = flat(shot.velocity).normalize_or(Vec3::NEG_Z);
            let Some(at) = fall(shot, fire.gravity, flight) else {
                return shot.ticks < FALL_TICKS;
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
                    kind: ShotKind::Agent {
                        target: None,
                        chase: 0,
                    },
                    ..shot.clone()
                });
            }
            false
        }
        ShotKind::Agent { target, chase } => {
            let keep = agent(shot, target, chase, fire, flight, out, born);
            if !keep {
                *about = about.saturating_sub(1);
            }
            keep
        }
        ShotKind::Comet { life } => comet(shot, life, fire, flight, out),
        ShotKind::Blast { .. } => grow(shot, fire, flight, out),
        _ => false,
    }
}

fn agent(
    shot: &mut Shot,
    target: Option<Entity>,
    chase: u32,
    fire: &Agents,
    flight: &Flight,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
) -> bool {
    let eye = shot.position + Vec3::Y * fire.touch_height;
    let targets = flight.targets;
    let kept = target.and_then(|entity| {
        targets
            .iter()
            .find(|candidate| candidate.entity == entity && candidate.alive)
    });
    let chased = kept.or_else(|| {
        targets
            .iter()
            .filter(|candidate| {
                candidate.alive && candidate.enemy && candidate.centre.distance(eye) <= fire.sight
            })
            .map(|candidate| (candidate.centre.distance(eye), candidate))
            .filter(|(_, candidate)| in_sight(flight.world, eye, candidate.centre))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, candidate)| candidate)
    });
    let chase = match (chased, kept) {
        (Some(_), Some(_)) => chase + 1,
        (Some(_), None) => 1,
        (None, _) => 0,
    };
    shot.kind = ShotKind::Agent {
        target: chased.map(|chased| chased.entity),
        chase,
    };
    let ended = shot.ticks >= fire.life_ticks
        || shot.position.distance(flight.hero) > fire.leash
        || chase > fire.chase_ticks
        || chased.is_some_and(|chased| {
            chased.centre.distance(eye) <= fire.touch + chased.radius
                || flat(chased.centre - shot.position).length() < fire.reach
        });
    if ended {
        agent_end(shot, chased, fire, flight, out, born);
        return false;
    }
    // At its enemy, or back to within a few steps of the hero.
    let goal = chased.map(|chased| chased.centre).or_else(|| {
        (flat(flight.hero - shot.position).length() > fire.follow).then_some(flight.hero)
    });
    let way = goal.map_or(Vec3::ZERO, |goal| {
        flat(goal - shot.position).normalize_or_zero()
    });
    walk(shot, way * fire.speed, fire.gravity, flight.world);
    true
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
            out.event(shot.weapon, GunEventKind::Impact { at });
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
                kind: ShotKind::Comet { life },
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
    out.event(shot.weapon, GunEventKind::Impact { at });
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

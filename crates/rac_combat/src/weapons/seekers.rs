//! The guns whose shots seek out enemies (`weapons.md`): the Plasma Coil's balls and their
//! arcs, the Disc Blade Gun's discs, the RY3NO's volleys and the Spitting Hydra's marks.

use bevy::math::ops;
use bevy::prelude::*;
use rac_input::Action;
use rac_sim::SimRng;
use std::f32::consts::TAU;

use super::{
    GunEventKind,
    aim::{ALL_ROUND, GunTarget, assist_targets, flat, in_sight},
    arsenal::Arsenal,
    def::{AimAssist, Armory, Coil, Disc, Hydra, RocketBlast, Storm, Timing, Volley},
    fire::{Gun, GunInput, Outcome, ball, fire_anim, run_dry, take_shot},
    shots::{BOUNCE_GAP, Flight, Meets, RocketState, Shot, ShotKind, TICK, in_ball, meets_where},
};

/// How far off the world a coil ball sticks.
const STICK_GAP: f32 = 0.1;

fn alive(targets: &[GunTarget], entity: Entity) -> Option<&GunTarget> {
    targets
        .iter()
        .find(|target| target.entity == entity && target.alive)
}

/// The Plasma Coil (0x315638): held, a press of the cooldown takes a round and spins the
/// coil up, and the ball leaves when it has spun; at most a few balls at once.
pub(super) fn coil(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Coil,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    let trigger = &mut arsenal.trigger;
    trigger.cooldown = trigger.cooldown.saturating_sub(1);
    let spun = match trigger.launch {
        Some(left) if left > 1 => {
            trigger.launch = Some(left - 1);
            false
        }
        Some(_) => {
            trigger.launch = None;
            true
        }
        None => false,
    };
    if spun {
        // At the lock-on target or the assist's, else anything at the muzzle, else ahead.
        let (target, mut aim) = gun.seek(gun.muzzle, &fire.assist, gun_input, armory);
        if target.is_none()
            && let Some(near) =
                ball(gun_input.targets, gun.muzzle, fire.muzzle_reach).find(|target| target.alive)
        {
            aim = (near.centre - gun.muzzle).normalize_or(aim);
        }
        let serial = arsenal.next_serial();
        let shot = match fire.storm.filter(|storm| gun.level >= storm.from_level) {
            Some(storm) => gun.shot(
                serial,
                gun.muzzle,
                aim * storm.speed,
                ShotKind::Storm { arced: Vec::new() },
            ),
            None => gun.shot(
                serial,
                gun.muzzle,
                aim * fire.speed,
                ShotKind::Ball { stuck: None },
            ),
        };
        arsenal.shots.push(shot);
    }
    let balls = arsenal
        .shots
        .iter()
        .filter(|shot| {
            shot.weapon == gun.weapon
                && matches!(shot.kind, ShotKind::Ball { .. } | ShotKind::Storm { .. })
        })
        .count();
    let trigger = &arsenal.trigger;
    if !gun_input.input.held(Action::Fire)
        || !gun.ready
        || trigger.cooldown > 0
        || trigger.launch.is_some()
        || balls >= fire.most
    {
        return;
    }
    if !take_shot(arsenal, armory, gun, out) {
        run_dry(arsenal);
        return;
    }
    arsenal.trigger.cooldown = fire.cooldown_ticks;
    arsenal.trigger.launch = Some(fire.spin_ticks);
}

/// A coil ball (0x30BF18) and its arcs: it flies straight and sticks to the first thing it
/// touches, a target taking its damage; a while later arcs leave it for a few other enemies
/// round it, each striking its target as it arrives.
pub(super) fn coil_shot(
    shot: &mut Shot,
    fire: &Coil,
    flight: &Flight,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
) -> bool {
    let targets = flight.targets;
    if let ShotKind::Arc { target } = shot.kind {
        if shot.ticks < fire.arc_ticks {
            return true;
        }
        if let Some(struck) = alive(targets, target) {
            let push = (struck.centre - shot.position) * fire.push;
            out.hits.push(shot.hit(struck.entity, shot.splash, push));
        }
        return false;
    }
    if let Some(storm) = fire.storm
        && matches!(shot.kind, ShotKind::Storm { .. })
    {
        return storm_ball(shot, &storm, fire, flight, out, born);
    }
    let ShotKind::Ball { stuck } = &mut shot.kind else {
        return false;
    };
    if let Some((on, ticks)) = stuck {
        *ticks += 1;
        // Stuck to a target, it goes where it goes.
        if let Some(carrier) = on.and_then(|entity| targets.iter().find(|t| t.entity == entity)) {
            shot.position = carrier.centre;
        }
        if *ticks < fire.stick_ticks {
            return true;
        }
        let on = *on;
        let round = AimAssist {
            range: fire.arc_range,
            near: ALL_ROUND,
            far: ALL_ROUND,
        };
        let struck = assist_targets(
            targets,
            shot.position,
            Vec3::NEG_Z,
            &round,
            flight.world,
            |target| target.enemy && Some(target.entity) != on,
            fire.arcs,
        );
        for index in struck {
            let target = &targets[index];
            out.shot_event(
                shot,
                GunEventKind::Arc {
                    from: shot.position,
                    to: target.centre,
                },
            );
            born.push(Shot {
                velocity: Vec3::ZERO,
                ticks: 0,
                kind: ShotKind::Arc {
                    target: target.entity,
                },
                ..shot.clone()
            });
        }
        return false;
    }
    // A target within its reach takes it, pushed by its velocity a frame.
    if let Some(target) = in_ball(targets, shot.position, fire.radius, None).find(|t| t.alive) {
        let push = shot.velocity * TICK * fire.push;
        out.hits.push(shot.hit(target.entity, shot.damage, push));
        out.shot_event(
            shot,
            GunEventKind::Impact {
                at: shot.position,
                surface: None,
            },
        );
        shot.kind = ShotKind::Ball {
            stuck: Some((Some(target.entity), 0)),
        };
        return true;
    }
    let motion = shot.velocity * TICK;
    match flight.world.cast_ray(shot.position, motion) {
        Some(hit) => {
            shot.position += motion * hit.fraction + hit.normal * STICK_GAP;
            out.shot_event(
                shot,
                GunEventKind::Impact {
                    at: shot.position,
                    surface: None,
                },
            );
            shot.kind = ShotKind::Ball {
                stuck: Some((None, 0)),
            };
            true
        }
        None => {
            shot.position += motion;
            // Out of time in the air, it fizzles.
            shot.ticks < fire.lifetime
        }
    }
}

/// The Plasma Storm's ball (0x309480): it flies, bouncing, striking what it touches and
/// arcing to what it sees round it; then it fades and goes.
fn storm_ball(
    shot: &mut Shot,
    storm: &Storm,
    fire: &Coil,
    flight: &Flight,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
) -> bool {
    if shot.ticks >= storm.fly_ticks + storm.fade_ticks {
        out.shot_event(
            shot,
            GunEventKind::Impact {
                at: shot.position,
                surface: None,
            },
        );
        return false;
    }
    if shot.ticks >= storm.fly_ticks {
        return true;
    }
    let targets = flight.targets;
    // What it touches takes its damage, and it bounces off.
    if let Some(touched) = in_ball(targets, shot.position, fire.radius, None).find(|t| t.alive) {
        let push = shot.velocity * TICK * fire.push;
        out.hits.push(shot.hit(touched.entity, shot.damage, push));
        let away =
            (shot.position - touched.centre).normalize_or(-shot.velocity.normalize_or_zero());
        if shot.velocity.dot(away) < 0.0 {
            shot.velocity = shot.velocity.reflect(away) * storm.bounce_keep;
        }
    }
    let motion = shot.velocity * TICK;
    match flight.world.cast_ray(shot.position, motion) {
        Some(hit) => {
            shot.position += motion * hit.fraction + hit.normal * BOUNCE_GAP;
            shot.velocity = shot.velocity.reflect(hit.normal) * storm.bounce_keep;
        }
        None => shot.position += motion,
    }
    let now = shot.ticks;
    let ShotKind::Storm { arced } = &mut shot.kind else {
        return false;
    };
    let mut arced = std::mem::take(arced);
    arced.retain(|&(_, at)| now < at + storm.again_ticks);
    let out_now = arced
        .iter()
        .filter(|&&(_, at)| now < at + fire.arc_ticks)
        .count();
    let mut room = storm.arcs.saturating_sub(out_now);
    let at = shot.position;
    for target in targets {
        if room == 0 {
            break;
        }
        if !target.alive
            || !target.enemy
            || target.centre.distance(at) > storm.reach
            || arced.iter().any(|&(entity, _)| entity == target.entity)
            || !in_sight(flight.world, at, target.centre)
        {
            continue;
        }
        room -= 1;
        arced.push((target.entity, now));
        out.shot_event(
            shot,
            GunEventKind::Arc {
                from: at,
                to: target.centre,
            },
        );
        born.push(Shot {
            velocity: Vec3::ZERO,
            ticks: 0,
            kind: ShotKind::Arc {
                target: target.entity,
            },
            ..shot.clone()
        });
    }
    shot.kind = ShotKind::Storm { arced };
    true
}

/// The Disc Blade Gun (0x36EFC0): held, a disc every so often, leaving turned to his side,
/// each the other side from the last.
pub(super) fn disc(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Disc,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    let trigger = &mut arsenal.trigger;
    trigger.cooldown = trigger.cooldown.saturating_sub(1);
    if !gun_input.input.held(Action::Fire) || trigger.cooldown > 0 || !gun.ready {
        return;
    }
    if !take_shot(arsenal, armory, gun, out) {
        run_dry(arsenal);
        return;
    }
    let trigger = &mut arsenal.trigger;
    trigger.cooldown = fire.cooldown_ticks;
    let side = if trigger.barrel.is_multiple_of(2) {
        1.0
    } else {
        -1.0
    };
    trigger.barrel = trigger.barrel.wrapping_add(1);
    let Some(assist) = fire.assist.at(gun.level) else {
        return;
    };
    let (target, aim) = gun.seek(gun.muzzle, &assist, gun_input, armory);
    let turn = if target.is_some() {
        fire.side_aimed_degrees
    } else {
        fire.side_degrees
    };
    let direction = Quat::from_rotation_y(side * turn.to_radians()) * aim;
    let serial = arsenal.next_serial();
    arsenal.shots.push(gun.shot(
        serial,
        gun.muzzle,
        direction * fire.speed,
        ShotKind::Disc {
            target,
            // It curves back across his aim.
            curve: -side * fire.curve_degrees.to_radians(),
            last: None,
            splits: 0,
        },
    ));
}

/// A disc (0x36FA78): it flies at its target, or curves, and bounces off the world; a
/// target it touches takes its damage. From V2 a bounce turns it at a new enemy; from V3
/// it bounces off enemies too, hitting each; from V4 its end is a blast. Each bounce is a
/// [`GunEventKind::Bounce`] and its end, wherever it comes, an impact, as the game sparks
/// at each (0x370CD8) and bursts at its end.
pub(super) fn disc_shot(
    shot: &mut Shot,
    fire: &Disc,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
    discs: &mut usize,
) -> bool {
    let keep = disc_tick(shot, fire, flight, rng, out, born, discs);
    if !keep {
        *discs = discs.saturating_sub(1);
    }
    keep
}

fn disc_tick(
    shot: &mut Shot,
    fire: &Disc,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
    discs: &mut usize,
) -> bool {
    let targets = flight.targets;
    let level = shot.level;
    let retargets = fire.retarget_from.is_some_and(|from| level >= from);
    let through = fire.through_from.is_some_and(|from| level >= from);
    let lifetime = fire.lifetime.at(level).unwrap_or(0);
    let ShotKind::Disc {
        mut target,
        curve,
        mut last,
        mut splits,
    } = shot.kind
    else {
        return false;
    };
    let speed = shot.velocity.length();
    match target.and_then(|entity| alive(targets, entity)) {
        Some(chased) => {
            shot.velocity = (chased.centre - shot.position).normalize_or(shot.velocity) * speed;
        }
        None => {
            target = None;
            shot.velocity = Quat::from_rotation_y(curve) * shot.velocity;
        }
    }
    let ended = shot.ticks >= lifetime || shot.position.distance(flight.hero) > fire.leash;
    let touching = in_ball(targets, shot.position, fire.radius, last)
        .find(|candidate| candidate.alive)
        .map(|candidate| (candidate.entity, candidate.centre));
    let mut over = ended;
    if !ended && let Some((entity, centre)) = touching {
        let push = shot.velocity * TICK * fire.push;
        out.hits.push(shot.hit(entity, shot.damage, push));
        if through {
            out.shot_event(shot, GunEventKind::Bounce { at: shot.position });
            // Off the enemy, and at another it can see.
            let away = (shot.position - centre).normalize_or(-shot.velocity.normalize_or_zero());
            if shot.velocity.dot(away) < 0.0 {
                shot.velocity = shot.velocity.reflect(away);
            }
            last = Some(entity);
            target = retarget(targets, flight, shot.position, fire.retarget_range, last);
            if let Some(split) = fire.split.filter(|split| level >= split.from_level)
                && *discs < split.most
            {
                splits += 1;
                let odds = 1 << ((splits - 1) / 2).min(31);
                if rng.below(odds) == 0 {
                    // The game adds to each axis of its velocity a frame, always up,
                    // then gives it back its speed.
                    let (low, high) = split.jitter;
                    let mut jitter = || rng.range_f32(low, high);
                    let way = shot.velocity * TICK + Vec3::new(jitter(), jitter(), jitter());
                    born.push(Shot {
                        velocity: way.normalize_or_zero() * speed,
                        ticks: 0,
                        kind: ShotKind::Disc {
                            target: None,
                            curve,
                            last,
                            splits,
                        },
                        ..shot.clone()
                    });
                    *discs += 1;
                }
                over = rng.below(odds) != 0;
            }
        } else {
            over = true;
        }
    }
    if over {
        if let Some(blast) = fire.end_blast.filter(|blast| level >= blast.from_level) {
            for near in in_ball(targets, shot.position, blast.radius, None) {
                out.hits
                    .push(shot.hit(near.entity, shot.damage * blast.share, Vec3::ZERO));
            }
        }
        out.shot_event(
            shot,
            GunEventKind::Impact {
                at: shot.position,
                surface: None,
            },
        );
        return false;
    }
    let motion = shot.velocity * TICK;
    match flight.world.cast_ray(shot.position, motion) {
        Some(hit) => {
            shot.position += motion * hit.fraction + hit.normal * BOUNCE_GAP;
            shot.velocity = shot.velocity.reflect(hit.normal);
            out.shot_event(shot, GunEventKind::Bounce { at: shot.position });
            if retargets {
                target = retarget(targets, flight, shot.position, fire.retarget_range, last);
            }
        }
        None => shot.position += motion,
    }
    shot.kind = ShotKind::Disc {
        target,
        curve,
        last,
        splits,
    };
    true
}

/// The nearest living enemy within `range` of `at` that can be seen from it, but `except`.
fn retarget(
    targets: &[GunTarget],
    flight: &Flight,
    at: Vec3,
    range: f32,
    except: Option<Entity>,
) -> Option<Entity> {
    targets
        .iter()
        .filter(|target| target.alive && target.enemy && Some(target.entity) != except)
        .map(|target| (target.centre.distance(at), target))
        .filter(|&(distance, _)| distance <= range)
        .filter(|(_, target)| in_sight(flight.world, at, target.centre))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, target)| target.entity)
}

/// The RY3NO (0x3805B0): it marks the enemies its assist finds each tick, and held, a round
/// fires a volley of missiles from the muzzle, each at the next marked target in turn,
/// else the lock-on target, else along his aim; from `ahead_from` all at a point ahead.
pub(super) fn volley(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Volley,
    gun_input: &GunInput,
    rng: &mut SimRng,
    out: &mut Outcome,
) {
    let count = fire.missiles.at(gun.level).unwrap_or(0) as usize;
    let aim = gun.view_aim(gun.muzzle, armory);
    let marked: Vec<Entity> = assist_targets(
        gun_input.targets,
        gun.muzzle,
        aim,
        &fire.assist,
        gun_input.world,
        |target| target.enemy,
        count,
    )
    .into_iter()
    .map(|index| gun_input.targets[index].entity)
    .collect();
    let trigger = &mut arsenal.trigger;
    trigger.marked = marked.clone();
    trigger.cooldown = trigger.cooldown.saturating_sub(1);
    if !gun_input.input.held(Action::Fire) || trigger.cooldown > 0 || !gun.ready {
        return;
    }
    if !take_shot(arsenal, armory, gun, out) {
        run_dry(arsenal);
        return;
    }
    let slow = if fire.slow_from.is_some_and(|from| gun.level >= from) {
        fire.slow
    } else {
        1.0
    };
    arsenal.trigger.cooldown = (Timing::Rate(fire.rate).ticks() as f32 * slow).ceil() as u32;
    let ahead = fire
        .ahead_from
        .is_some_and(|from| gun.level >= from)
        .then(|| {
            let hero = gun_input.hero.position;
            hero + flat(gun.facing).normalize_or(Vec3::NEG_Z) * fire.ahead + Vec3::Y * fire.up
        });
    let lock = gun.lock.map(|(entity, _)| entity);
    let speed = fire.speed * fire.speed_share.at(gun.level).unwrap_or(1.0);
    let (low, high) = fire.tilt.at(gun.level).unwrap_or_default();
    let wait = fire.wait.at(gun.level).unwrap_or(0);
    for i in 0..count {
        let target = match ahead {
            Some(_) => None,
            None if marked.is_empty() => lock,
            None => Some(marked[i % marked.len()]),
        };
        let at = target
            .and_then(|entity| alive(gun_input.targets, entity))
            .map(|target| target.centre)
            .or(ahead);
        let along = at.map_or(aim, |at| (at - gun.muzzle).normalize_or(aim));
        let spin = rng.range_f32(fire.spin.0, fire.spin.1).to_radians();
        let state = RocketState {
            target,
            along,
            tilt: rng.range_f32(low, high).to_radians(),
            roll: rng.range_f32(0.0, TAU),
            spin: if rng.below(2) == 0 { spin } else { -spin },
            wait: if wait > 0 { rng.below(wait + 1) } else { 0 },
            life: fire.lifetime.at(gun.level).unwrap_or(0),
            speed,
        };
        let serial = arsenal.next_serial();
        arsenal
            .shots
            .push(gun.shot(serial, gun.muzzle, along * speed, ShotKind::Rocket(state)));
    }
}

/// A RY3NO missile (0x383370; 0x3DB4D0 from V2): after its wait it spirals round the line
/// it set off along, and near its target also steps straight at it. What it flies into
/// takes the damage, and it bursts ([`RocketBlast`]); out of time, or too far from the
/// hero, it goes with no blast.
pub(super) fn rocket(shot: &mut Shot, fire: &Volley, flight: &Flight, out: &mut Outcome) -> bool {
    let ShotKind::Rocket(mut state) = shot.kind else {
        return false;
    };
    let keep = rocket_tick(shot, &mut state, fire, flight, out);
    shot.kind = ShotKind::Rocket(state);
    keep
}

fn rocket_tick(
    shot: &mut Shot,
    state: &mut RocketState,
    fire: &Volley,
    flight: &Flight,
    out: &mut Outcome,
) -> bool {
    if state.wait > 0 {
        state.wait -= 1;
        return true;
    }
    let chased = state
        .target
        .and_then(|entity| alive(flight.targets, entity));
    if chased.is_none() {
        state.target = None;
    }
    // Its frame: the line it set off along, and two ways across it.
    let across = state.along.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = across.cross(state.along);
    let (sin_roll, cos_roll) = ops::sin_cos(state.roll);
    let (sin_tilt, cos_tilt) = ops::sin_cos(state.tilt);
    let forward = state.along * cos_tilt + (across * cos_roll + up * sin_roll) * sin_tilt;
    state.roll += state.spin;
    let step = state.speed * TICK;
    let from = shot.position;
    let mut to = from + forward * step;
    if let Some(chased) = chased
        && chased.centre.distance(to) < fire.home_within
    {
        to += (chased.centre - to).normalize_or_zero() * step;
    }
    shot.velocity = (to - from) / TICK;
    let blast = fire.blast.at(shot.level).unwrap_or(RocketBlast {
        radius: 0.0,
        splash: false,
        push: (0.0, 0.0),
    });
    let (at, struck) = match meets_where(flight.world, flight.targets, from, to - from, |t| t.alive)
    {
        Meets::Target(index, t) => {
            let struck = &flight.targets[index];
            let push =
                (flat(forward).normalize_or_zero() + Vec3::Y * fire.push_up).normalize_or_zero();
            out.hits.push(shot.hit(struck.entity, shot.damage, push));
            (from + (to - from) * t, Some(struck.entity))
        }
        Meets::World(t, _) => (from + (to - from) * t, None),
        Meets::Nothing => {
            shot.position = to;
            state.life = state.life.saturating_sub(1);
            return state.life > 0 && to.distance(flight.hero) <= fire.leash;
        }
    };
    shot.position = at;
    for near in in_ball(flight.targets, at, blast.radius, None) {
        if blast.splash {
            if Some(near.entity) == struck {
                continue;
            }
            let out_of = flat(near.centre - at).normalize_or_zero();
            let push = out_of * blast.push.0 + Vec3::Y * blast.push.1;
            out.hits.push(shot.hit(near.entity, shot.splash, push));
        } else {
            let push = forward * blast.push.0;
            out.hits.push(shot.hit(near.entity, shot.damage, push));
        }
    }
    out.shot_event(shot, GunEventKind::Impact { at, surface: None });
    false
}

/// The Spitting Hydra (0x31FB90): held, it marks an enemy every so often, up to the level's
/// most; let go, it plays the firing animation and a bolt strikes each marked one, which
/// takes the damage round it at once and the bolt's share as the bolt goes (no round
/// taken). With none marked it takes a round and spits ([`spit`]). Let go too soon, it
/// fizzles.
pub(super) fn hydra(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Hydra,
    gun_input: &GunInput,
    rng: &mut SimRng,
    out: &mut Outcome,
) {
    let most = fire.most_targets.at(gun.level).unwrap_or(0);
    let aim = gun.view_aim(gun.muzzle, armory);
    if gun_input.input.held(Action::Fire) && gun.ready && arsenal.has_ammo(gun.weapon, armory) {
        arsenal.fire_ticks = gun.def.fire_ticks;
        let trigger = &mut arsenal.trigger;
        if trigger.held >= fire.max_hold {
            return;
        }
        trigger.held += 1;
        let every = fire.mark_every.at(gun.level).unwrap_or(1).max(1);
        if (trigger.held.is_multiple_of(every) || trigger.held == fire.max_hold)
            && trigger.marked.len() < most
        {
            let marked = &trigger.marked;
            let next = assist_targets(
                gun_input.targets,
                gun.muzzle,
                aim,
                &fire.assist,
                gun_input.world,
                |target| target.enemy && !marked.contains(&target.entity),
                1,
            );
            if let Some(&index) = next.first() {
                trigger.marked.push(gun_input.targets[index].entity);
            }
        }
        return;
    }
    let trigger = &mut arsenal.trigger;
    if trigger.held == 0 {
        return;
    }
    let held = std::mem::take(&mut trigger.held);
    let marked = std::mem::take(&mut trigger.marked);
    if held < fire.tap_ticks {
        return;
    }
    if !marked.is_empty() {
        fire_anim(arsenal, gun, out);
        for entity in marked {
            let Some(struck) = alive(gun_input.targets, entity) else {
                continue;
            };
            out.event(
                gun.weapon,
                GunEventKind::Arc {
                    from: gun.muzzle,
                    to: struck.centre,
                },
            );
            let push = (struck.centre - gun.muzzle).normalize_or_zero() * fire.push;
            for target in ball(gun_input.targets, struck.centre, fire.strike_radius) {
                out.hits.push(gun.hit(target.entity, gun.damage, push));
            }
            let serial = arsenal.next_serial();
            arsenal.shots.push(Shot {
                damage: gun.damage * fire.bolt_share,
                ..gun.shot(
                    serial,
                    gun.muzzle,
                    Vec3::ZERO,
                    ShotKind::Bolt {
                        target: struck.entity,
                        left: fire.bolt_ticks,
                        push: fire.push,
                    },
                )
            });
        }
        return;
    }
    if !take_shot(arsenal, armory, gun, out) {
        return;
    }
    let reach = gun.muzzle + aim * fire.spit_reach;
    let jitter = fire.spit_jitter;
    let ends = (0..most)
        .map(|_| {
            reach
                + Vec3::new(
                    rng.range_f32(-jitter, jitter),
                    rng.range_f32(-jitter, jitter),
                    rng.range_f32(-jitter, jitter),
                )
        })
        .collect();
    let serial = arsenal.next_serial();
    arsenal.shots.push(gun.shot(
        serial,
        gun.muzzle,
        Vec3::ZERO,
        ShotKind::Spit {
            life: fire.spit_ticks.at(gun.level).unwrap_or(0),
            ends,
        },
    ));
}

/// The Hydra's spit (0x321210, 0x321580): its arcs reach from the gun for their ends while
/// it lasts. At [`Hydra::spit_strike`] ticks before its end each arc within its range that
/// meets something on the way strikes: what it meets first takes the damage, pushed along
/// it, and a blast round its end does the damage too.
pub(super) fn spit(shot: &mut Shot, fire: &Hydra, flight: &Flight, out: &mut Outcome) -> bool {
    let ShotKind::Spit { life, ends } = &shot.kind else {
        return false;
    };
    let from = shot.position;
    for &to in ends {
        out.shot_event(shot, GunEventKind::Arc { from, to });
    }
    if *life >= fire.spit_strike && shot.ticks == life - fire.spit_strike {
        let blast = fire.spit_blast.at(shot.level).unwrap_or(0.0);
        for &to in ends {
            if from.distance(to) > fire.spit_range {
                continue;
            }
            let push = (to - from).normalize_or_zero() * fire.spit_push;
            let met = match meets_where(flight.world, flight.targets, from, to - from, |t| t.alive)
            {
                Meets::Target(index, t) => {
                    out.hits
                        .push(shot.hit(flight.targets[index].entity, shot.damage, push));
                    Some(from + (to - from) * t)
                }
                Meets::World(t, _) => Some(from + (to - from) * t),
                Meets::Nothing => None,
            };
            let Some(at) = met else {
                continue;
            };
            for near in in_ball(flight.targets, to, blast, None) {
                out.hits
                    .push(shot.hit(near.entity, shot.damage, Vec3::ZERO));
            }
            out.shot_event(shot, GunEventKind::Impact { at, surface: None });
        }
    }
    shot.ticks < *life
}

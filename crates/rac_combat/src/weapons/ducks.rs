//! The Qwack-O-Ray's ducks and their eggs (`weapons.md`, "Qwack-O-Ray"): the enemy the
//! beam brings down is gone, and a duck the gun owns stands where it was.

use bevy::{math::ops, prelude::*};
use rac_sim::SimRng;

use super::{
    GunEventKind,
    aim::{GunTarget, flat, in_sight},
    def::{Beam, Duck, DuckWaddle, Egg},
    fire::{Gun, Outcome},
    shots::{BOUNCE_GAP, DuckMood, DuckState, Flight, Shot, ShotKind, TICK, in_ball},
    thrown::{rebound, walk},
};

/// Ducks fall as the agents do [placeholder: they keep to the ground in the game].
const GRAVITY: f32 = 11.0;

/// A duck where `target` stood, facing away from the hero (0x3D2480, 0x39E5D8, 0x3D8840).
pub(super) fn hatch(gun: &Gun, fire: &Beam, target: &GunTarget, hero: Vec3, serial: u32) -> Shot {
    let away = flat(target.centre - hero).normalize_or(gun.facing);
    let blitzer = matches!(fire.duck.at(gun.level), Some(Duck::Blitzer { .. }));
    let at = if blitzer {
        target.centre
    } else {
        target.centre - Vec3::Y * target.radius
    };
    let wait = match fire.duck.at(gun.level) {
        Some(Duck::Blitzer { wait, .. }) => wait,
        _ => 0,
    };
    gun.shot(
        serial,
        at,
        Vec3::ZERO,
        ShotKind::Duck(DuckState {
            yaw: ops::atan2(-away.x, -away.z),
            turn: 0,
            egg: fire.egg.every.0,
            target: None,
            pecks: 0,
            wait,
            flies: blitzer,
            ..default()
        }),
    )
}

/// A duck's tick, or an egg's.
pub(super) fn duck_shot(
    shot: &mut Shot,
    fire: &Beam,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
) -> bool {
    match shot.kind {
        ShotKind::Egg { life } => egg(shot, &fire.egg, life, flight, out),
        ShotKind::Duck(state) => {
            let Some(duck) = fire.duck.at(shot.level) else {
                return false;
            };
            let (keep, state) = match duck {
                Duck::Waddle {
                    speed,
                    turn_every,
                    ticks,
                    eggs,
                } => {
                    let mut state = state;
                    waddle(
                        shot,
                        &mut state,
                        &fire.waddle,
                        speed,
                        turn_every,
                        flight,
                        rng,
                    );
                    // At frame 15 of its third idle sequence (0x3D22B0).
                    if eggs
                        && state.mood == DuckMood::Idle
                        && state.sequence == fire.egg.idle_sequence
                        && state.sequence_ticks == fire.egg.idle_ticks
                    {
                        lay_egg(shot, state.yaw, &fire.egg, rng, born);
                    }
                    let keep = shot.ticks < ticks;
                    if !keep {
                        out.shot_event(
                            shot,
                            GunEventKind::Impact {
                                at: shot.position,
                                surface: None,
                            },
                        );
                    }
                    (keep, state)
                }
                Duck::Soldier { .. } => soldier(shot, state, duck, fire, flight, rng, out, born),
                Duck::Blitzer { .. } => blitzer(shot, state, duck, flight, out),
            };
            shot.kind = ShotKind::Duck(state);
            keep
        }
        _ => false,
    }
}

/// How it passes its time on the ground (0x3D1728, 0x39DB60; `DuckWaddle`): idling, or
/// walking along its facing, a new way every so often [simplified: the game's aim wobbles
/// every tick], or standing. Returns whether an idle sequence just ended.
fn waddle(
    shot: &mut Shot,
    state: &mut DuckState,
    time: &DuckWaddle,
    speed: f32,
    turn_every: (u32, u32),
    flight: &Flight,
    rng: &mut SimRng,
) -> bool {
    if state.left == 0 && state.sequence_ticks == 0 && state.mood == DuckMood::Idle {
        state.left = between(rng, time.first_ticks);
    }
    state.sequence_ticks += 1;
    state.left = state.left.saturating_sub(1);
    let near = flat(flight.hero - shot.position).length() < time.near;
    let mut ended = false;
    let mut way = Vec3::ZERO;
    match state.mood {
        DuckMood::Idle => {
            let length = time
                .idle
                .get(usize::from(state.sequence))
                .map_or(0, |&(_, length)| length);
            if state.sequence_ticks >= length {
                ended = true;
                state.sequence_ticks = 0;
                state.sequence = pick_idle(time, rng);
            }
            if near {
                state.mood = DuckMood::Walk;
                state.left = state.left.max(1);
            } else if state.left == 0 {
                if rng.below(2) == 0 {
                    state.mood = DuckMood::Stand;
                    state.left = between(rng, time.stand_ticks);
                } else {
                    state.mood = DuckMood::Walk;
                    state.left = between(rng, time.walk_ticks);
                }
            }
        }
        DuckMood::Walk => {
            if state.turn == 0 {
                state.yaw += rng.range_f32(-45.0, 45.0).to_radians();
                state.turn = between(rng, turn_every);
            }
            state.turn -= 1;
            way = Vec3::new(-ops::sin(state.yaw), 0.0, -ops::cos(state.yaw)) * speed;
            if state.left == 0 && !near {
                state.mood = DuckMood::Idle;
                state.left = between(rng, time.idle_ticks);
                state.sequence_ticks = 0;
                state.sequence = pick_idle(time, rng);
            }
        }
        DuckMood::Stand => {
            if state.left == 0 {
                state.mood = DuckMood::Idle;
                state.left = between(rng, time.after_stand);
                state.sequence_ticks = 0;
                state.sequence = rng.below(2) as u8;
            }
        }
    }
    walk(shot, way, GRAVITY, flight.world);
    ended
}

/// One of its idle sequences, by their chances.
fn pick_idle(time: &DuckWaddle, rng: &mut SimRng) -> u8 {
    let mut roll = rng.unit_f32();
    for (number, &(chance, _)) in time.idle.iter().enumerate() {
        if roll < chance {
            return number as u8;
        }
        roll -= chance;
    }
    time.idle.len().saturating_sub(1) as u8
}

/// An egg behind it when its time comes (0x3D22B0, 0x39EF18).
fn lay(
    shot: &Shot,
    mut state: DuckState,
    egg: &Egg,
    rng: &mut SimRng,
    born: &mut Vec<Shot>,
) -> DuckState {
    state.egg = state.egg.saturating_sub(1);
    if state.egg > 0 {
        return state;
    }
    state.egg = between(rng, egg.every);
    lay_egg(shot, state.yaw, egg, rng, born);
    state
}

/// An egg behind a duck facing `facing` (0x3D22B0).
fn lay_egg(shot: &Shot, facing: f32, egg: &Egg, rng: &mut SimRng, born: &mut Vec<Shot>) {
    let yaw = facing + (180.0 + rng.range_f32(-egg.back_degrees, egg.back_degrees)).to_radians();
    let up = rng
        .range_f32(egg.up_degrees.0, egg.up_degrees.1)
        .to_radians();
    let speed = rng.range_f32(egg.speed.0, egg.speed.1);
    let way = Vec3::new(
        -ops::sin(yaw) * ops::cos(up),
        ops::sin(up),
        -ops::cos(yaw) * ops::cos(up),
    );
    born.push(Shot {
        position: shot.position + Vec3::Y * egg.height,
        velocity: way * speed,
        ticks: 0,
        kind: ShotKind::Egg {
            life: between(rng, egg.life),
        },
        ..shot.clone()
    });
}

/// An egg (0x3D48B8): it falls and bounces, from `seek_from` turning at an enemy as it
/// bounces; an enemy within reach sets it off, striking that one for the gun's damage.
/// Its time out, it goes off harmlessly.
fn egg(shot: &mut Shot, egg: &Egg, life: u32, flight: &Flight, out: &mut Outcome) -> bool {
    shot.velocity.y -= egg.gravity * TICK;
    let motion = shot.velocity * TICK;
    match flight.world.cast_ray(shot.position, motion) {
        Some(hit) => {
            shot.position += motion * hit.fraction + hit.normal * BOUNCE_GAP;
            shot.velocity = rebound(shot.velocity, hit.normal, egg.bounce_keep);
            if egg.seek_from.is_some_and(|from| shot.level >= from) {
                seek(shot, egg, flight.targets);
            }
        }
        None => shot.position += motion,
    }
    let struck = in_ball(flight.targets, shot.position, egg.reach, None)
        .find(|target| target.alive && target.enemy);
    if let Some(target) = struck {
        let push = shot.velocity.normalize_or_zero() * egg.push;
        out.hits.push(shot.hit(target.entity, shot.damage, push));
    }
    if struck.is_some() || shot.ticks >= life {
        out.shot_event(
            shot,
            GunEventKind::Impact {
                at: shot.position,
                surface: None,
            },
        );
        return false;
    }
    true
}

/// A bounce turned at the nearest enemy within its range.
fn seek(shot: &mut Shot, egg: &Egg, targets: &[GunTarget]) {
    let nearest = targets
        .iter()
        .filter(|target| target.alive && target.enemy)
        .map(|target| (target.centre.distance(shot.position), target))
        .filter(|&(distance, _)| distance <= egg.seek_range)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, target)) = nearest {
        let way = flat(target.centre - shot.position).normalize_or_zero();
        let speed = flat(shot.velocity).length().max(egg.seek_speed);
        shot.velocity.x = way.x * speed;
        shot.velocity.z = way.z * speed;
    }
}

/// The V4 duck (0x39DB60): it waddles until a look sends it at the nearest enemy, laying
/// eggs as it runs; there it blows up (0x39E8A0). Its enemy gone, it waddles again.
#[allow(clippy::too_many_arguments)]
fn soldier(
    shot: &mut Shot,
    mut state: DuckState,
    duck: Duck,
    fire: &Beam,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
) -> (bool, DuckState) {
    let Duck::Soldier {
        waddle: waddle_speed,
        speed,
        turn_every,
        look_chance,
        reach,
        radius,
        share,
    } = duck
    else {
        return (false, state);
    };
    let enemies = || {
        flight
            .targets
            .iter()
            .filter(|target| target.alive && target.enemy)
    };
    let mut chased = state
        .target
        .and_then(|entity| enemies().find(|target| target.entity == entity));
    if chased.is_none() {
        // Each time an idle sequence ends, a roll looks for the nearest enemy.
        let ended = waddle(
            shot,
            &mut state,
            &fire.waddle,
            waddle_speed,
            turn_every,
            flight,
            rng,
        );
        if ended && rng.unit_f32() < look_chance {
            chased = enemies()
                .map(|target| (target.centre.distance(shot.position), target))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, target)| target);
        }
        state.target = chased.map(|target| target.entity);
        if chased.is_none() {
            return (true, state);
        }
    }
    state.target = chased.map(|target| target.entity);
    let Some(chased) = chased else {
        return (true, state);
    };
    if flat(chased.centre - shot.position).length() <= reach + chased.radius {
        blow_up(shot, radius, share, flight, out);
        return (false, state);
    }
    let way = flat(chased.centre - shot.position).normalize_or_zero();
    state.yaw = ops::atan2(-way.x, -way.z);
    walk(shot, way * speed, GRAVITY, flight.world);
    (true, lay(shot, state, &fire.egg, rng, born))
}

/// The Qwack-O-Blitzer's duck (0x3D7998, 0x3D8420): it flies by the hero, and after an
/// enemy within sight that it can see; at it, it pecks a tick at a time, and its pecks
/// done it blows up (0x3D9008).
fn blitzer(
    shot: &mut Shot,
    mut state: DuckState,
    duck: Duck,
    flight: &Flight,
    out: &mut Outcome,
) -> (bool, DuckState) {
    let Duck::Blitzer {
        sight,
        speed,
        back_speed,
        near_speed,
        over,
        behind,
        left,
        reach,
        peck_radius,
        pecks,
        radius,
        share,
        ..
    } = duck
    else {
        return (false, state);
    };
    if state.wait > 0 {
        state.wait -= 1;
        return (true, state);
    }
    let enemies = || {
        flight
            .targets
            .iter()
            .filter(|target| target.alive && target.enemy)
    };
    let kept = state
        .target
        .and_then(|entity| enemies().find(|target| target.entity == entity));
    let chased = kept.or_else(|| {
        enemies()
            .map(|target| (target.centre.distance(shot.position), target))
            .filter(|&(distance, target)| {
                distance <= sight && in_sight(flight.world, shot.position, target.centre)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, target)| target)
    });
    state.target = chased.map(|target| target.entity);
    let (goal, pace) = match chased {
        Some(chased) => (chased.centre, speed),
        None => {
            let facing = flat(flight.facing);
            let home =
                flight.hero + Vec3::Y * over - facing * behind + Vec3::Y.cross(facing) * left;
            let pace = if home.distance(shot.position) > 2.0 {
                back_speed
            } else {
                near_speed
            };
            (home, pace)
        }
    };
    let to = goal - shot.position;
    let step = (pace * TICK).min(to.length());
    shot.position += to.normalize_or_zero() * step;
    if to.length() > 1e-3 {
        state.yaw = ops::atan2(-to.x, -to.z);
    }
    state.pecking = false;
    if let Some(chased) = chased
        && chased.centre.distance(shot.position) <= reach + chased.radius
    {
        state.pecking = true;
        state.pecks += 1;
        for near in in_ball(flight.targets, shot.position, peck_radius, None)
            .filter(|near| near.alive && near.enemy)
        {
            let away = (near.centre - shot.position).normalize_or_zero();
            out.hits
                .push(shot.hit(near.entity, shot.damage * share, away));
        }
        if state.pecks >= pecks {
            blow_up(shot, radius, share, flight, out);
            return (false, state);
        }
    }
    (true, state)
}

/// A duck going up: the enemies within `radius` take `share` of the damage.
fn blow_up(shot: &Shot, radius: f32, share: f32, flight: &Flight, out: &mut Outcome) {
    for near in
        in_ball(flight.targets, shot.position, radius, None).filter(|near| near.alive && near.enemy)
    {
        let away = (near.centre - shot.position).normalize_or_zero();
        out.hits
            .push(shot.hit(near.entity, shot.damage * share, away));
    }
    out.shot_event(
        shot,
        GunEventKind::Impact {
            at: shot.position,
            surface: None,
        },
    );
}

/// A random count of ticks from `low` to `high`.
fn between(rng: &mut SimRng, (low, high): (u32, u32)) -> u32 {
    low + rng.below(high.saturating_sub(low) + 1)
}

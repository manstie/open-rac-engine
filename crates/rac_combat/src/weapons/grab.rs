//! The guns that take enemies (`weapons.md`): the Rift Inducer's rifts swallow weak ones,
//! the Suck Cannon sucks them in and fires them back out, and the Qwack-O-Ray turns them
//! into ducks.

use bevy::prelude::*;
use rac_input::Action;
use rac_sim::SimRng;

use super::{
    Affliction, GunEventKind, Sustain,
    aim::{flat, in_sight},
    arsenal::Arsenal,
    def::{Armory, Beam, Rift, Ripper, Suck, VortexComet},
    ducks,
    fire::{Afflict, Gun, GunInput, Outcome, ball, take_shot},
    shots::{
        BOUNCE_GAP, Flight, Meets, RiftState, Shot, ShotKind, TICK, in_ball, meets, meets_where,
    },
};

/// The Rift Inducer (0x372990): a press, a while since the last, takes a round and
/// launches a rift at the lock-on target or the assist's, at a random speed.
pub(super) fn rift(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Rift,
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
    let (_, aim) = gun.seek(gun.muzzle, &fire.assist, gun_input, armory);
    let speed = rng.range_f32(fire.speed.0, fire.speed.1);
    let spring = rng.range_f32(fire.pull.0, fire.pull.1) * fire.size * PULL_SCALE;
    let serial = arsenal.next_serial();
    arsenal.shots.push(gun.shot(
        serial,
        gun.muzzle,
        aim * speed,
        ShotKind::Rift(RiftState {
            spring,
            ..RiftState::default()
        }),
    ));
}

/// A rift (0x373990): it slows as it flies and opens after a while or where it meets
/// something, knocking what it met. Open, it takes one enemy (or other thing with hit
/// points) at a time within its reach whose hit points are at most twice the gun's damage
/// times its size, pulls it in and swallows it, until its time runs out or he leaves it
/// behind.
pub(super) fn rift_shot(
    shot: &mut Shot,
    fire: &Rift,
    flight: &Flight,
    rng: &mut SimRng,
    out: &mut Outcome,
    born: &mut Vec<Shot>,
) -> bool {
    let ShotKind::Rift(mut state) = shot.kind else {
        return false;
    };
    if state.open.is_some()
        && let Some(ripper) = fire.ripper
        && shot.level >= ripper.from_level
    {
        rip(shot, fire, ripper, flight, rng, born);
    }
    let keep = rift_tick(shot, &mut state, fire, flight, out);
    shot.kind = ShotKind::Rift(state);
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

/// The Rift Ripper's bolts on the enemies round it.
fn rip(
    shot: &Shot,
    fire: &Rift,
    ripper: Ripper,
    flight: &Flight,
    rng: &mut SimRng,
    born: &mut Vec<Shot>,
) {
    for near in in_ball(
        flight.targets,
        shot.position,
        fire.size + ripper.reach,
        None,
    ) {
        if !near.enemy || !near.alive || rng.below(ripper.odds) != 0 {
            continue;
        }
        born.push(Shot {
            velocity: Vec3::ZERO,
            ticks: 0,
            damage: shot.damage * ripper.share,
            kind: ShotKind::Bolt {
                target: near.entity,
                left: ripper.bolt_ticks,
                push: ripper.push,
            },
            ..shot.clone()
        });
    }
}

fn rift_tick(
    shot: &mut Shot,
    state: &mut RiftState,
    fire: &Rift,
    flight: &Flight,
    out: &mut Outcome,
) -> bool {
    let targets = flight.targets;
    let Some(open) = &mut state.open else {
        shot.velocity *= fire.keep;
        let motion = shot.velocity * TICK;
        match meets(flight.world, targets, shot.position, motion) {
            Meets::Target(index, t) => {
                shot.position += motion * t;
                let knock = shot.velocity * TICK * fire.knock;
                out.hits.push(shot.hit(targets[index].entity, 0.0, knock));
                state.open = Some(0);
            }
            Meets::World(t, _) => {
                shot.position += motion * t;
                state.open = Some(0);
            }
            Meets::Nothing => {
                shot.position += motion;
                if shot.ticks >= fire.fly_ticks {
                    state.open = Some(0);
                }
            }
        }
        if state.open.is_some() {
            shot.velocity = Vec3::ZERO;
        }
        return true;
    };
    *open += 1;
    if *open >= fire.open_ticks || shot.position.distance(flight.hero) > fire.leash {
        return false;
    }
    if let Some((entity, left, speed)) = &mut state.taking {
        let Some(taken) = targets
            .iter()
            .find(|target| target.entity == *entity && target.alive)
        else {
            state.taking = None;
            return true;
        };
        pull(left, speed, state.spring);
        out.shot_event(
            shot,
            GunEventKind::Arc {
                from: shot.position,
                to: taken.centre,
            },
        );
        if *left <= SWALLOW_WITHIN {
            out.afflictions.push(Afflict {
                target: taken.entity,
                weapon: shot.weapon,
                affliction: Affliction::Swallowed {
                    into: shot.position,
                },
            });
            state.taking = None;
        }
        return true;
    }
    let most = 2.0 * shot.damage * fire.size;
    let taken = in_ball(targets, shot.position, fire.size + fire.reach, None).find(|target| {
        target.alive
            && target
                .health
                .is_some_and(|(health, _)| health > 0.0 && health <= most)
    });
    if let Some(taken) = taken {
        state.taking = Some((taken.entity, taken.centre.distance(shot.position), 0.0));
    }
    true
}

/// The rift's spring is the drawn strength times its size and this (0x373738).
const PULL_SCALE: f32 = 1.5;

/// A target this near the rift's middle is swallowed.
const SWALLOW_WITHIN: f32 = 0.1;

/// A tick of the rift's pull (0x445208): `speed` springs towards the `left` still to come,
/// at `spring` / 3600 a tick, at most `spring` / 60 u a tick and never past the middle, and
/// carries it that much closer; under a hundredth of the most it snaps there.
fn pull(left: &mut f32, speed: &mut f32, spring: f32) {
    spring_towards(left, speed, spring / 3600.0, spring / 60.0);
}

/// `speed` springs towards the `left` still to come by `gain`, at most `most` a tick and
/// never past it, and carries it that much closer (0x445208).
pub(super) fn spring_towards(left: &mut f32, speed: &mut f32, gain: f32, most: f32) {
    *speed = speed.clamp(-*left, *left);
    *speed += gain * (*left - *speed);
    *speed = speed.clamp(-most, most).clamp(-*left, *left);
    *left -= *speed;
    if *left < 0.01 * most {
        *left = 0.0;
    }
}

/// The Suck Cannon (0x3619C8): held, it sucks in every few ticks what its vortex touches,
/// while it has room; let go with something held, each press fires one back out.
pub(super) fn suck(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Suck,
    gun_input: &GunInput,
    rng: &mut SimRng,
    out: &mut Outcome,
) {
    let held = gun_input.input.held(Action::Fire);
    let capacity = fire.capacity.at(gun.level).unwrap_or(0);
    arsenal.trigger.cooldown = arsenal.trigger.cooldown.saturating_sub(1);
    if arsenal.trigger.expelling {
        if arsenal.sucked == 0 {
            arsenal.trigger.expelling = false;
            return;
        }
        if !gun_input.input.pressed(Action::Fire) || arsenal.trigger.cooldown > 0 || !gun.ready {
            return;
        }
        // It uses no ammo: what it holds is its ammo.
        take_shot(arsenal, armory, gun, out);
        arsenal.sucked -= 1;
        arsenal.trigger.cooldown = fire.cooldown_ticks;
        if arsenal.sucked == 0 {
            arsenal.trigger.expelling = false;
        }
        let (target, aim) = gun.seek(gun.muzzle, &fire.assist, gun_input, armory);
        let serial = arsenal.next_serial();
        let (velocity, kind) = match fire.comet.filter(|comet| gun.level >= comet.from_level) {
            Some(comet) => {
                // The game's way a frame has its two ground axes thrown, then a new speed.
                let way = aim * fire.speed * TICK
                    + Vec3::new(
                        rng.range_f32(-comet.jitter, comet.jitter),
                        0.0,
                        rng.range_f32(-comet.jitter, comet.jitter),
                    );
                let speed = rng.range_f32(comet.speed.0, comet.speed.1);
                let life = comet.life;
                (
                    way.normalize_or(aim) * speed,
                    ShotKind::Comet { life, target },
                )
            }
            None => (aim * fire.speed, ShotKind::Thing { target }),
        };
        arsenal
            .shots
            .push(gun.shot(serial, gun.muzzle, velocity, kind));
        return;
    }
    if !held || !gun.ready {
        if std::mem::take(&mut arsenal.trigger.held) > 0 && arsenal.sucked > 0 {
            arsenal.trigger.expelling = true;
        }
        return;
    }
    arsenal.fire_ticks = gun.def.fire_ticks;
    arsenal.sustain = Some(Sustain::Sucking);
    let trigger = &mut arsenal.trigger;
    trigger.held += 1;
    if !(trigger.held - 1).is_multiple_of(fire.grab_every.max(1)) || arsenal.sucked >= capacity {
        return;
    }
    // The vortex runs along his aim, cut short where it meets the world.
    let aim = gun.view_aim(gun.muzzle, armory);
    let length = fire.vortex;
    let reach = gun_input
        .world
        .cast_ray(gun.muzzle, aim * length)
        .map_or(length, |hit| hit.fraction * length);
    let mut taken: Vec<Entity> = Vec::new();
    for &(along, radius) in &fire.grabs {
        if along > reach {
            break;
        }
        let at = gun.muzzle + aim * along;
        for target in ball(gun_input.targets, at, radius) {
            if taken.contains(&target.entity) || !target.alive {
                continue;
            }
            let Some((health, _)) = target.health else {
                continue;
            };
            taken.push(target.entity);
            if target.enemy && health > fire.most_hitpoints {
                // Too big to take: the suck's hit.
                let toward = (gun.muzzle - target.centre).normalize_or_zero();
                out.hits
                    .push(gun.hit(target.entity, fire.grab_damage, toward));
            } else if arsenal.sucked < capacity {
                arsenal.sucked += 1;
                out.afflictions.push(Afflict {
                    target: target.entity,
                    weapon: gun.weapon,
                    affliction: Affliction::Sucked,
                });
            }
        }
    }
}

/// What the Suck Cannon fires (0x382830, its flight 0x381B98): it flies on, pulled at its
/// target while that lives. The line of its step, or a ball round it, meeting a target (not
/// the hero) bursts it there; meeting the world, it bursts, or from
/// [`Suck::ricochet_from`] bounces off. Out of time it bursts where it is. Each burst is a
/// blast of the gun's damage.
pub(super) fn thing(shot: &mut Shot, fire: &Suck, flight: &Flight, out: &mut Outcome) -> bool {
    let ShotKind::Thing { target } = &mut shot.kind else {
        return false;
    };
    if shot.ticks > fire.lifetime {
        let at = shot.position;
        burst(shot, flight, at, fire.end_radius, Vec3::ZERO, out);
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
    let from = shot.position;
    let mut to = from + shot.velocity * TICK;
    if let Some(chased) = chased {
        to += (chased.centre - to).normalize_or_zero() * fire.pull * TICK;
    }
    let frame = shot.velocity * TICK;
    let mut met = meets_where(flight.world, flight.targets, from, to - from, |t| t.alive);
    if matches!(met, Meets::Nothing)
        && in_ball(flight.targets, from, fire.sweep, None).any(|target| target.alive)
    {
        met = Meets::Target(0, 0.0);
    }
    match met {
        Meets::World(t, normal) => {
            let at = from + (to - from) * t;
            if fire.ricochet_from.is_some_and(|from| shot.level >= from) {
                shot.position = at + normal * BOUNCE_GAP;
                shot.velocity = shot.velocity.reflect(normal);
                out.shot_event(shot, GunEventKind::Impact { at, surface: None });
                return true;
            }
            burst(shot, flight, at, fire.radius, frame * fire.world_push, out);
            false
        }
        Meets::Target(_, t) => {
            let at = from + (to - from) * t;
            burst(shot, flight, at, fire.radius, frame * fire.target_push, out);
            false
        }
        Meets::Nothing => {
            shot.position = to;
            true
        }
    }
}

/// The Vortex Cannon's comet (0x394068): it drifts on, falling, pulled at its target; what
/// it meets takes its damage and a blast does the same round it.
pub(super) fn comet(
    shot: &mut Shot,
    comet: &VortexComet,
    flight: &Flight,
    out: &mut Outcome,
) -> bool {
    let ShotKind::Comet { life, target } = &mut shot.kind else {
        return false;
    };
    let life = *life;
    let chased = target.and_then(|entity| {
        flight
            .targets
            .iter()
            .find(|candidate| candidate.entity == entity && candidate.alive)
    });
    if chased.is_none() {
        *target = None;
    }
    let from = shot.position;
    let mut to = from + shot.velocity * TICK;
    if let Some(chased) = chased {
        to += (chased.centre - to).normalize_or_zero() * comet.pull * TICK;
    }
    shot.velocity.y -= comet.gravity * TICK;
    let (at, direct) = match meets_where(flight.world, flight.targets, from, to - from, |t| t.alive)
    {
        Meets::Target(index, t) => (from + (to - from) * t, Some(index)),
        Meets::World(t, _) => (from + (to - from) * t, None),
        Meets::Nothing => {
            shot.position = to;
            return shot.ticks < life;
        }
    };
    let struck = direct.map(|index| &flight.targets[index]);
    if let Some(struck) = struck {
        let away = (struck.centre - flight.hero).normalize_or_zero() * comet.push;
        out.hits.push(shot.hit(struck.entity, shot.damage, away));
    }
    for near in in_ball(
        flight.targets,
        at,
        comet.radius,
        struck.map(|struck| struck.entity),
    ) {
        out.hits
            .push(shot.hit(near.entity, shot.damage, Vec3::ZERO));
    }
    out.shot_event(shot, GunEventKind::Impact { at, surface: None });
    false
}

/// The blast a sucked-in thing bursts in: every target within `radius` takes its damage.
fn burst(shot: &Shot, flight: &Flight, at: Vec3, radius: f32, push: Vec3, out: &mut Outcome) {
    for near in in_ball(flight.targets, at, radius, None) {
        out.hits.push(shot.hit(near.entity, shot.damage, push));
    }
    out.shot_event(shot, GunEventKind::Impact { at, surface: None });
}

/// The Qwack-O-Ray (0x37DEB0): held, its beam picks an enemy (0x37D7D8) and keeps a new one
/// a while; the beam brings the target's duck meter, its hit points to start with, down by
/// the gun's damage a second, and at nothing it is a duck. Letting go, or losing the
/// target, starts again.
pub(super) fn beam(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Beam,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    if !gun_input.input.held(Action::Fire) || !gun.ready {
        arsenal.trigger.beam = None;
        return;
    }
    arsenal.fire_ticks = gun.def.fire_ticks;
    let targets = gun_input.targets;
    let kept = arsenal.trigger.beam.filter(|&(entity, ticks, _)| {
        ticks < fire.hold_ticks
            && targets
                .iter()
                .any(|target| target.entity == entity && target.alive && target.enemy)
    });
    let aim = gun.view_aim(gun.muzzle, armory);
    let picked = kept
        .map(|(entity, ..)| entity)
        .or_else(|| beam_target(gun, fire, gun_input, aim));
    let Some(target) = picked.and_then(|entity| targets.iter().find(|t| t.entity == entity)) else {
        arsenal.trigger.beam = None;
        // With nothing to take it still shines along his aim, as far as it reaches (0x37E830).
        let line = aim * fire.range;
        let reach = gun_input
            .world
            .cast_ray(gun.muzzle, line)
            .map_or(1.0, |hit| hit.fraction);
        out.event(
            gun.weapon,
            GunEventKind::Ray {
                from: gun.muzzle,
                to: gun.muzzle + line * reach,
            },
        );
        return;
    };
    let (_, ticks, meter) = match arsenal.trigger.beam {
        Some(beam) if beam.0 == target.entity => beam,
        _ => (
            target.entity,
            0,
            target.health.map_or(0.0, |(health, _)| health),
        ),
    };
    let meter = meter - gun.damage * TICK;
    out.event(
        gun.weapon,
        GunEventKind::Ray {
            from: gun.muzzle,
            to: target.centre,
        },
    );
    if meter <= 0.0 {
        out.afflictions.push(Afflict {
            target: target.entity,
            weapon: gun.weapon,
            affliction: Affliction::Duck,
        });
        arsenal.trigger.beam = None;
        let serial = arsenal.next_serial();
        let duck = ducks::hatch(gun, fire, target, gun_input.hero.position, serial);
        arsenal.shots.push(duck);
    } else {
        arsenal.trigger.beam = Some((target.entity, ticks + 1, meter));
    }
}

/// The beam's pick: an enemy close in front of him, else the nearest within its range
/// whose bearing is near the beam's, with nothing in the way.
fn beam_target(gun: &Gun, fire: &Beam, gun_input: &GunInput, aim: Vec3) -> Option<Entity> {
    let hero = gun_input.hero;
    let enemies = || {
        gun_input
            .targets
            .iter()
            .filter(|target| target.alive && target.enemy)
    };
    let facing = flat(gun.facing).normalize_or(Vec3::NEG_Z);
    let close = enemies().find(|target| {
        let across = flat(target.centre - hero.position);
        across.length() <= fire.near
            && (target.centre.y - hero.position.y).abs() <= fire.near_height
            && across.angle_between(facing) <= fire.near_degrees.to_radians()
    });
    if let Some(close) = close {
        return Some(close.entity);
    }
    let limit = fire.cone_degrees.to_radians();
    enemies()
        .map(|target| (target.centre.distance(gun.muzzle), target))
        .filter(|&(distance, target)| {
            distance <= fire.range && (target.centre - gun.muzzle).angle_between(aim) <= limit
        })
        .filter(|(_, target)| in_sight(gun_input.world, gun.muzzle, target.centre))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, target)| target.entity)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticks_to_swallow(from: f32, spring: f32) -> u32 {
        let (mut left, mut speed) = (from, 0.0);
        let mut ticks = 0;
        while left > SWALLOW_WITHIN {
            pull(&mut left, &mut speed, spring);
            ticks += 1;
        }
        ticks
    }

    #[test]
    fn the_rifts_pull_brings_a_target_in_from_its_reach_in_36_to_111_ticks() {
        // The size's 0.5 makes the springs 5.25 to 17.25.
        let (weakest, strongest) = (7.0 * 0.5 * PULL_SCALE, 23.0 * 0.5 * PULL_SCALE);
        let slow = ticks_to_swallow(9.5, weakest);
        let fast = ticks_to_swallow(9.5, strongest);
        assert!((108..=114).contains(&slow), "{slow}");
        assert!((33..=39).contains(&fast), "{fast}");
    }
}

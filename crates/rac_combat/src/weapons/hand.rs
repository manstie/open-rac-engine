//! The guns that work from his hand rather than by firing (`weapons.md`, "Plasma Whip" and
//! "Shield Charger"): the whip, whose rope his own wrench moves swing, and the shield round
//! him.

use bevy::prelude::*;
use rac_hero::{HeroState, WhipHand};
use rac_input::Action;
use rac_sim::SimRng;

use super::{
    GunEventKind,
    arsenal::Arsenal,
    def::{Armory, FireMode, Mods, ShieldCharger, Tesla, WeaponDef, WeaponId, Whip},
    fire::{Gun, GunInput, Hit, Outcome, ball, take_shot},
    shots::{Flight, Shot, ShotKind, TICK, in_ball},
};

/// The Shield Charger's shield up round him.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ShieldUp {
    weapon: WeaponId,
    level: u8,
    /// Ticks to its next zap.
    zap: u32,
    /// The Tesla Barrier's bolts so far.
    bolts: u32,
    /// Its points, as last seen.
    points: f32,
}

/// What his moves need of the Plasma Whip in his hand: `None` for any other gun.
pub(super) fn whip_hand(
    arsenal: &Arsenal,
    armory: &Armory,
    def: &WeaponDef,
    level: u8,
) -> Option<WhipHand> {
    let FireMode::Whip(fire) = &def.fire else {
        return None;
    };
    let weapon = arsenal.hand.gun()?;
    let ready = matches!(arsenal.hand, super::Hand::Gun { ticks, .. } if ticks >= def.ready_ticks);
    // Laid straight, its last end is a stretch a piece and a lead a link from the hand.
    let rope = |stretch: f32| {
        fire.links as f32 * stretch + fire.links.saturating_sub(1) as f32 * fire.lead + fire.tip
    };
    let (swing, last_swing, jump, _) = fire.stretch;
    Some(WhipHand {
        loaded: ready && arsenal.has_ammo(weapon, armory),
        damage: def.level(level).damage,
        push: fire.push,
        lift: fire.lift,
        reach: [rope(swing), rope(last_swing), rope(jump)],
        combo_from: fire.combo_from,
        jump_from: fire.jump_from,
    })
}

/// The Plasma Whip (0x356110): each attack his moves started with it takes a round, and
/// from V4 the jump attack sends a shock wave along the ground (class 0x1890).
pub(super) fn whip(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Whip,
    gun_input: &GunInput,
    attacks: u32,
    out: &mut Outcome,
) {
    for _ in 0..attacks {
        if arsenal.use_ammo(gun.weapon, 1, armory) {
            out.event(
                gun.weapon,
                GunEventKind::Fired {
                    at: gun.muzzle,
                    direction: gun.facing,
                },
            );
        }
    }
    let hero = gun_input.hero;
    if let Some(wave) = fire.wave
        && gun.level >= wave.from_level
        && hero.whip_attack()
        && hero.state == HeroState::JumpAttack
        && hero.ticks == wave.at_tick
    {
        let along = Vec3::new(gun.facing.x, 0.0, gun.facing.z).normalize_or(Vec3::NEG_Z);
        let serial = arsenal.next_serial();
        arsenal.shots.push(Shot {
            damage: gun.damage * wave.share,
            ..gun.shot(serial, gun.muzzle, along * wave.speed, ShotKind::Wave)
        });
    }
    if let Some(cut) = fire.cut
        && gun.level >= cut.from_level
        && hero.whip_attack()
        && let HeroState::Combo(swing) = hero.state
        && let Some(&at) = cut.at_ticks.get(swing as usize)
        && hero.ticks == at
    {
        let facing = Vec3::new(gun.facing.x, 0.0, gun.facing.z).normalize_or(Vec3::NEG_Z);
        let turn = cut.turns.get(swing as usize).copied().unwrap_or(0.0);
        let along = Quat::from_rotation_y(turn.to_radians()) * facing;
        let serial = arsenal.next_serial();
        arsenal.shots.push(gun.shot(
            serial,
            gun.muzzle,
            along * cut.speed,
            ShotKind::Cut { struck: Vec::new() },
        ));
    }
}

/// The Quantum Whip's cutting wave (0x3D6740): it runs on, and each tick the line across
/// it strikes what it crosses [each target once: the game strikes each tick].
pub(super) fn cut(shot: &mut Shot, fire: &Whip, flight: &Flight, out: &mut Outcome) -> bool {
    let Some(cut) = fire.cut else {
        return false;
    };
    shot.position += shot.velocity * TICK;
    let across = Vec3::Y.cross(shot.velocity).normalize_or(Vec3::X) * cut.width / 2.0;
    let (a, b) = (shot.position - across, shot.position + across);
    let push = shot.velocity * TICK * cut.push;
    let crossed: Vec<Entity> = flight
        .targets
        .iter()
        .filter(|target| target.alive && on_line(a, b, target.centre, target.radius))
        .map(|target| target.entity)
        .collect();
    let ShotKind::Cut { struck } = &mut shot.kind else {
        return false;
    };
    let new: Vec<Entity> = crossed
        .into_iter()
        .filter(|entity| !struck.contains(entity))
        .collect();
    struck.extend(&new);
    for entity in new {
        out.hits.push(shot.hit(entity, shot.damage, push));
    }
    shot.ticks < cut.ticks
}

/// Whether a ball of `radius` round `centre` touches the line from `a` to `b`.
fn on_line(a: Vec3, b: Vec3, centre: Vec3, radius: f32) -> bool {
    let line = b - a;
    let t = ((centre - a).dot(line) / line.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (a + line * t).distance(centre) <= radius
}

/// The whip's shock wave (0x393690): it runs along his facing, and from a few ticks on,
/// every few, finds the ground under where it was a little before and strikes a ball there.
pub(super) fn wave(shot: &mut Shot, fire: &Whip, flight: &Flight, out: &mut Outcome) -> bool {
    let Some(wave) = fire.wave else {
        return false;
    };
    shot.position += shot.velocity * TICK;
    if shot.ticks >= wave.first && (shot.ticks - wave.first).is_multiple_of(wave.every.max(1)) {
        let behind = shot.position - shot.velocity * (wave.behind as f32 * TICK);
        let probe = Vec3::NEG_Y * wave.probe;
        if let Some(ground) = flight.world.cast_ray(behind, probe) {
            let at = behind + probe * ground.fraction;
            let push = shot.velocity.normalize_or_zero() * wave.push;
            for target in in_ball(flight.targets, at, wave.radius, None) {
                out.hits.push(shot.hit(target.entity, shot.damage, push));
            }
            out.shot_event(shot, GunEventKind::Impact { at, surface: None });
        }
    }
    shot.ticks < wave.ticks
}

/// The Shield Charger (0x3410E8, `shield_charger_fire` 0x340DD0): a press with no shield up
/// takes a round and raises one, of the level's points; with one up it does nothing.
pub(super) fn shield(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &ShieldCharger,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    if !gun_input.input.pressed(Action::Fire) || !gun.ready {
        return;
    }
    if gun_input.hero.shield.is_up() || arsenal.shield.is_some() {
        return;
    }
    if !take_shot(arsenal, armory, gun, out) {
        return;
    }
    let points = fire.points.at(gun.level).unwrap_or(0.0);
    out.shield = Some(points);
    arsenal.shield = Some(ShieldUp {
        weapon: gun.weapon,
        level: gun.level,
        zap: 0,
        bolts: 0,
        points,
    });
}

/// While the shield is up, at once and then every half second, a ball round it strikes
/// the living enemies touching it, pushing them straight out. It goes with its points, as
/// the hero's damage takes them ([`rac_hero::Shield`]).
pub(super) fn shield_tick(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun_input: &GunInput,
    rng: &mut SimRng,
    out: &mut Outcome,
) {
    let Some(mut up) = arsenal.shield else {
        return;
    };
    let def = armory.get(up.weapon);
    let fire = def.and_then(|def| match &def.fire {
        FireMode::Shield(fire) => Some(fire),
        _ => None,
    });
    let (Some(def), Some(fire)) = (def, fire) else {
        arsenal.shield = None;
        return;
    };
    if !gun_input.hero.shield.is_up() && out.shield.is_none() {
        arsenal.shield = None;
        return;
    }
    // Dead, or falling out of the level, it goes (0x342FB0, whose other tests are of things
    // the kit hasn't got).
    if matches!(gun_input.hero.state, HeroState::Dead | HeroState::FallOut) {
        out.shield = Some(0.0);
        arsenal.shield = None;
        return;
    }
    // Each point the hero's damage took from it gives the gun XP.
    let points = gun_input.hero.shield.points;
    if out.shield.is_none() && points != up.points {
        let taken = (up.points - points).abs() as u32;
        out.xp.push((up.weapon, taken * fire.xp_per_point));
        up.points = points;
    }
    if let Some(tesla) = fire.tesla
        && up.level >= tesla.from_level
    {
        let damage = def.level(up.level).damage;
        tesla_bolts(arsenal, up, tesla, fire.height, damage, gun_input, rng, out);
        return;
    }
    if up.zap > 0 {
        up.zap -= 1;
        arsenal.shield = Some(up);
        return;
    }
    up.zap = fire.zap_ticks.saturating_sub(1);
    arsenal.shield = Some(up);
    let centre = gun_input.hero.position + Vec3::Y * fire.height;
    let damage = def.level(up.level).damage;
    for target in ball(gun_input.targets, centre, fire.zap_radius) {
        if !target.enemy || !target.alive {
            continue;
        }
        out.hits.push(Hit {
            target: target.entity,
            weapon: up.weapon,
            damage,
            push: (target.centre - centre).normalize_or_zero() * fire.push,
            shock: false,
            acid: false,
        });
        out.event(
            up.weapon,
            GunEventKind::Arc {
                from: centre,
                to: target.centre,
            },
        );
    }
}

/// The Tesla Barrier's tick: a new bolt on an enemy near that has none while fewer than
/// the most are out, and the shield gone after its last.
#[allow(clippy::too_many_arguments)]
fn tesla_bolts(
    arsenal: &mut Arsenal,
    mut up: ShieldUp,
    tesla: Tesla,
    height: f32,
    damage: f32,
    gun_input: &GunInput,
    rng: &mut SimRng,
    out: &mut Outcome,
) {
    let centre = gun_input.hero.position + Vec3::Y * height;
    let struck: Vec<Entity> = arsenal
        .shots
        .iter()
        .filter(|shot| shot.weapon == up.weapon)
        .filter_map(|shot| match shot.kind {
            ShotKind::Bolt { target, .. } => Some(target),
            _ => None,
        })
        .collect();
    let new = ball(gun_input.targets, centre, tesla.reach)
        .find(|target| target.enemy && target.alive && !struck.contains(&target.entity));
    if struck.len() < tesla.most
        && let Some(target) = new
    {
        let (low, high) = tesla.bolt_ticks;
        let serial = arsenal.next_serial();
        arsenal.shots.push(Shot {
            serial,
            weapon: up.weapon,
            level: up.level,
            position: centre,
            velocity: Vec3::ZERO,
            ticks: 0,
            damage,
            splash: 0.0,
            mods: Mods::default(),
            kind: ShotKind::Bolt {
                target: target.entity,
                left: low + rng.below(high.saturating_sub(low) + 1),
                push: tesla.push,
            },
        });
        up.bolts += 1;
    }
    if up.bolts >= tesla.bolts {
        out.shield = Some(0.0);
        arsenal.shield = None;
    } else {
        arsenal.shield = Some(up);
    }
}

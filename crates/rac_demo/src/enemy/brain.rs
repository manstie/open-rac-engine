//! An enemy's behaviour as one pure step over (its state, what it can see, its definition,
//! collision), like the hero's controller, so tests drive it without an app.
//!
//! - **Idle** at a patrol point for a while, then **Patrol** to the next one.
//! - A hero it sees or hears makes it **Aggro**: the charger runs at him, the shooter keeps
//!   its distance and strafes.
//! - Near enough and facing him, it **Attack**s: a wind-up that tells the player what is
//!   coming, the strike (a lunge, or its shots) and a recovery that leaves it open.
//! - A hit makes it **Hurt** for a moment, cutting off whatever it was doing, and turns it
//!   on whoever hit it. The last hit makes it **Dead**; a while later it comes back at home.
//! - It gives up on a hero who gets far enough away, or once it strays too far from home,
//!   and walks back, ignoring heroes for a moment.

use bevy::math::ops;
use bevy::prelude::*;
use rac_hero::CollisionWorld;
use rac_hero::profile::BodyProfile;
use rac_sim::SimRng;
use std::f32::consts::{PI, TAU};

use super::def::{Attack, EnemyDef, HeroHit, Shoot};
use super::shot::ShotMotion;

/// Seconds per tick.
pub const TICK: f32 = 1.0 / 60.0;

/// The highest step it walks up without noticing.
const STEP: f32 = 0.4;
/// How far below its feet it still finds the ground while walking, so it follows slopes
/// down.
const SNAP: f32 = 0.3;
/// Kept between it and a wall, so the next sweep doesn't start touching.
const SKIN: f32 = 0.005;
/// Closer than this to a patrol point, it has arrived.
const ARRIVED: f32 = 0.5;
/// u/s of push apart for each u it overlaps another body.
const CROWD_PUSH: f32 = 8.0;
/// Falling this far below home puts it back there.
const LOST_BELOW: f32 = 20.0;
/// The charger stops running at this share of its lunge's range, so it doesn't run into
/// the hero while turning to him.
const CLOSE_ENOUGH: f32 = 0.75;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyState {
    Idle,
    Patrol,
    Aggro,
    Attack,
    Hurt,
    Dead,
}

/// Where an attack is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttackPhase {
    WindUp,
    Strike,
    Recover,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnemyMotion {
    /// Where its feet are.
    pub position: Vec3,
    pub yaw: f32,
    /// u/s: across the ground its own movement and a hit's push, and up and down its fall.
    pub velocity: Vec3,
    pub state: EnemyState,
    /// Ticks since the state began; 0 on its first tick.
    pub ticks: u32,
    /// The player of the hero it is after.
    pub target: Option<usize>,
    /// Where it starts and comes back to, and the points it walks between.
    pub home: Vec3,
    pub home_yaw: f32,
    pub patrol: Vec<Vec3>,
    /// The patrol point it walks to next.
    pub next_point: usize,
    /// How long it waits at the point it reached.
    pub wait: u32,
    /// Ticks before it can attack again.
    pub cooldown: u32,
    /// Ticks left of ignoring heroes after giving up on one.
    pub calm: u32,
    /// The players the current attack has hit, a bit each, so it hits each once.
    pub hit_players: u32,
    /// The shooter's strafing side, +1 to its right and −1 to its left, and the ticks
    /// before it picks again.
    pub strafe_side: f32,
    pub strafe_ticks: u32,
    pub on_ground: bool,
}

impl EnemyMotion {
    /// Standing at `home` facing `yaw`, about to walk to the first of `patrol`'s points.
    pub fn new(home: Vec3, yaw: f32, patrol: Vec<Vec3>) -> Self {
        Self {
            position: home,
            yaw,
            velocity: Vec3::ZERO,
            state: EnemyState::Idle,
            ticks: 0,
            target: None,
            home,
            home_yaw: yaw,
            patrol,
            next_point: 0,
            wait: 0,
            cooldown: 0,
            calm: 0,
            hit_players: 0,
            strafe_side: 1.0,
            strafe_ticks: 0,
            on_ground: true,
        }
    }

    pub fn facing(&self) -> Vec3 {
        facing(self.yaw)
    }

    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw)
    }

    /// Where the attack under way is, if one is.
    pub fn attack_phase(&self, def: &EnemyDef) -> Option<AttackPhase> {
        if self.state != EnemyState::Attack {
            return None;
        }
        let (wind_up, strike, _) = phases(def);
        Some(if self.ticks < wind_up {
            AttackPhase::WindUp
        } else if self.ticks < wind_up + strike {
            AttackPhase::Strike
        } else {
            AttackPhase::Recover
        })
    }

    pub fn is_alive(&self) -> bool {
        self.state != EnemyState::Dead
    }

    /// Dead long enough to have vanished.
    pub fn is_gone(&self, def: &EnemyDef) -> bool {
        self.state == EnemyState::Dead && self.ticks >= def.death_ticks
    }

    fn enter(&mut self, state: EnemyState) {
        self.state = state;
        self.ticks = 0;
    }

    fn eye(&self, def: &EnemyDef) -> Vec3 {
        self.position + Vec3::Y * (def.height * 0.8)
    }
}

/// What an enemy knows of a hero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeroView {
    pub player: usize,
    /// His feet.
    pub position: Vec3,
    pub alive: bool,
}

/// The heroes' bodies, for hits: a capsule round the segment from `bottom` to `top` above
/// the feet (the movement profile's [`BodyProfile`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeroShape {
    pub radius: f32,
    pub bottom: f32,
    pub top: f32,
}

impl HeroShape {
    pub fn from_profile(body: &BodyProfile) -> Self {
        Self {
            radius: body.radius,
            bottom: body.bottom,
            top: body.top,
        }
    }

    /// His middle, which enemies look and aim at.
    pub fn centre(&self, feet: Vec3) -> Vec3 {
        feet + Vec3::Y * ((self.bottom + self.top) / 2.0)
    }

    /// How far `point` is outside the body of a hero standing at `feet`; negative inside.
    pub fn distance(&self, feet: Vec3, point: Vec3) -> f32 {
        let low = feet + Vec3::Y * self.bottom;
        let high = feet + Vec3::Y * self.top;
        distance_to_segment(point, low, high) - self.radius
    }
}

/// Everything round an enemy that its step looks at.
pub struct Surroundings<'a> {
    pub heroes: &'a [HeroView],
    pub hero_shape: HeroShape,
    /// The other living enemies' feet and radii, which it keeps clear of.
    pub crowd: &'a [(Vec3, f32)],
    /// u/s².
    pub gravity: f32,
    pub world: &'a dyn CollisionWorld,
}

/// An attack that reached a hero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeroStrike {
    pub player: usize,
    pub damage: u32,
    /// u/s.
    pub knockback: Vec3,
}

/// What a step did that the game acts on.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Outcome {
    pub hits: Vec<HeroStrike>,
    pub shots: Vec<ShotMotion>,
    /// It vanished from view, dead.
    pub vanished: bool,
    /// It came back at home: its hit points refill and it can be hit again.
    pub respawned: bool,
}

/// A hit from `player`, pushing it at `push` (u/s): it reels and turns on him.
pub fn struck(enemy: &mut EnemyMotion, player: usize, push: Vec3) {
    if !enemy.is_alive() {
        return;
    }
    enemy.velocity = Vec3::new(push.x, enemy.velocity.y, push.z);
    enemy.target = Some(player);
    enemy.calm = 0;
    enemy.enter(EnemyState::Hurt);
}

/// Its hit points ran out: it dies where it stands, keeping the last hit's push. Returns
/// whether it was alive.
pub fn killed(enemy: &mut EnemyMotion) -> bool {
    if !enemy.is_alive() {
        return false;
    }
    enemy.target = None;
    enemy.enter(EnemyState::Dead);
    true
}

/// Ticks of an attack's wind-up, strike and recovery.
pub fn phases(def: &EnemyDef) -> (u32, u32, u32) {
    match def.attack {
        Attack::Lunge(lunge) => (lunge.wind_up_ticks, lunge.lunge_ticks, lunge.recover_ticks),
        Attack::Shoot(shoot) => (
            shoot.aim_ticks,
            shoot.shot_every.max(1) * shoot.shots.saturating_sub(1) + 1,
            shoot.recover_ticks,
        ),
    }
}

/// One tick of an enemy.
pub fn step(
    enemy: &mut EnemyMotion,
    def: &EnemyDef,
    around: &Surroundings,
    rng: &mut SimRng,
) -> Outcome {
    let mut outcome = Outcome::default();
    enemy.cooldown = enemy.cooldown.saturating_sub(1);
    enemy.calm = enemy.calm.saturating_sub(1);
    let state = enemy.state;
    // The velocity across the ground it wants, or `None` when the state set it outright.
    let wish = match state {
        EnemyState::Idle => idle(enemy, def, around),
        EnemyState::Patrol => patrol(enemy, def, around, rng),
        EnemyState::Aggro => aggro(enemy, def, around, rng),
        EnemyState::Attack => attack(enemy, def, around, rng, &mut outcome),
        EnemyState::Hurt => {
            if enemy.ticks >= def.hurt_ticks {
                enemy.enter(EnemyState::Aggro);
            }
            Some(Vec3::ZERO)
        }
        EnemyState::Dead => {
            if enemy.ticks == def.death_ticks {
                outcome.vanished = true;
            }
            if enemy.ticks >= def.death_ticks + def.respawn_ticks {
                respawn(enemy);
                outcome.respawned = true;
                return outcome;
            }
            Some(Vec3::ZERO)
        }
    };
    if !enemy.is_gone(def) {
        travel(enemy, def, wish, around);
    }
    if enemy.position.y < enemy.home.y - LOST_BELOW {
        respawn(enemy);
        outcome.respawned = true;
        return outcome;
    }
    if enemy.state == state {
        enemy.ticks += 1;
    }
    outcome
}

fn idle(enemy: &mut EnemyMotion, def: &EnemyDef, around: &Surroundings) -> Option<Vec3> {
    if let Some(player) = notice(enemy, def, around) {
        enemy.target = Some(player);
        enemy.enter(EnemyState::Aggro);
    } else if enemy.patrol.is_empty() {
        // A guard with no round stands at home, facing the way it was put.
        if flat(enemy.home - enemy.position).length() > ARRIVED {
            enemy.enter(EnemyState::Patrol);
        } else {
            turn_toward(enemy, enemy.home_yaw, def);
        }
    } else if enemy.ticks >= enemy.wait {
        enemy.enter(EnemyState::Patrol);
    }
    Some(Vec3::ZERO)
}

fn patrol(
    enemy: &mut EnemyMotion,
    def: &EnemyDef,
    around: &Surroundings,
    rng: &mut SimRng,
) -> Option<Vec3> {
    if let Some(player) = notice(enemy, def, around) {
        enemy.target = Some(player);
        enemy.enter(EnemyState::Aggro);
        return Some(Vec3::ZERO);
    }
    let goal = enemy
        .patrol
        .get(enemy.next_point)
        .copied()
        .unwrap_or(enemy.home);
    let to = flat(goal - enemy.position);
    if to.length() <= ARRIVED {
        if !enemy.patrol.is_empty() {
            enemy.next_point = (enemy.next_point + 1) % enemy.patrol.len();
        }
        enemy.wait = between(rng, def.moves.wait_ticks);
        enemy.enter(EnemyState::Idle);
        return Some(Vec3::ZERO);
    }
    Some(walk_toward(enemy, def, to, def.moves.walk_speed))
}

fn aggro(
    enemy: &mut EnemyMotion,
    def: &EnemyDef,
    around: &Surroundings,
    rng: &mut SimRng,
) -> Option<Vec3> {
    let Some(hero) = target(enemy, around) else {
        give_up(enemy, def);
        return Some(Vec3::ZERO);
    };
    let to = flat(hero.position - enemy.position);
    let distance = to.length();
    if distance > def.senses.lose || flat(enemy.position - enemy.home).length() > def.senses.leash {
        give_up(enemy, def);
        return Some(Vec3::ZERO);
    }
    match def.attack {
        Attack::Lunge(lunge) => {
            turn_toward(enemy, yaw_of(to), def);
            let off = angle_off(enemy.yaw, to);
            if distance <= lunge.range
                && enemy.cooldown == 0
                && off <= lunge.face_degrees.to_radians()
            {
                start_attack(enemy);
                return Some(Vec3::ZERO);
            }
            if distance <= lunge.range * CLOSE_ENOUGH {
                return Some(Vec3::ZERO);
            }
            // It runs the way it faces, slowing to turn, so a hero who circles it gains.
            Some(enemy.facing() * def.moves.run_speed * ops::cos(off).max(0.0))
        }
        Attack::Shoot(shoot) => {
            turn_toward(enemy, yaw_of(to), def);
            let off = angle_off(enemy.yaw, to);
            let centre = around.hero_shape.centre(hero.position);
            if enemy.cooldown == 0
                && distance <= shoot.range
                && off <= shoot.face_degrees.to_radians()
                && clear_line(around.world, muzzle(enemy, &shoot), centre)
            {
                start_attack(enemy);
                return Some(Vec3::ZERO);
            }
            let ahead = to.normalize_or_zero();
            Some(if distance < shoot.near {
                -ahead * def.moves.walk_speed
            } else if distance > shoot.far {
                ahead * def.moves.run_speed
            } else {
                enemy.strafe_ticks = enemy.strafe_ticks.saturating_sub(1);
                if enemy.strafe_ticks == 0 {
                    enemy.strafe_side = if rng.below(2) == 0 { 1.0 } else { -1.0 };
                    enemy.strafe_ticks = between(rng, shoot.strafe_ticks).max(1);
                }
                right_of(ahead) * enemy.strafe_side * shoot.strafe_speed
            })
        }
    }
}

fn start_attack(enemy: &mut EnemyMotion) {
    enemy.hit_players = 0;
    enemy.enter(EnemyState::Attack);
}

fn attack(
    enemy: &mut EnemyMotion,
    def: &EnemyDef,
    around: &Surroundings,
    rng: &mut SimRng,
    outcome: &mut Outcome,
) -> Option<Vec3> {
    let (wind_up, strike, recover) = phases(def);
    let hero = target(enemy, around).copied();
    let ticks = enemy.ticks;
    if ticks < wind_up {
        if let Some(hero) = hero {
            turn_toward(enemy, yaw_of(flat(hero.position - enemy.position)), def);
        }
        return Some(Vec3::ZERO);
    }
    if ticks < wind_up + strike {
        match def.attack {
            Attack::Lunge(lunge) => {
                let facing = enemy.facing();
                enemy.velocity = facing * lunge.lunge_speed + Vec3::Y * enemy.velocity.y;
                let front = enemy.position + facing * def.radius + Vec3::Y * (def.height / 2.0);
                for hero in around.heroes.iter().filter(|hero| hero.alive) {
                    let bit = 1u32.checked_shl(hero.player as u32).unwrap_or(0);
                    if enemy.hit_players & bit != 0
                        || around.hero_shape.distance(hero.position, front) > lunge.reach
                    {
                        continue;
                    }
                    enemy.hit_players |= bit;
                    outcome.hits.push(HeroStrike {
                        player: hero.player,
                        damage: lunge.hit.damage,
                        knockback: knockback(facing, &lunge.hit),
                    });
                }
                return None;
            }
            Attack::Shoot(shoot) => {
                let fired = ticks - wind_up;
                let every = shoot.shot_every.max(1);
                if fired.is_multiple_of(every) && fired / every < shoot.shots {
                    outcome.shots.push(fire(enemy, &shoot, hero, around));
                }
                return Some(Vec3::ZERO);
            }
        }
    }
    if ticks < wind_up + strike + recover {
        return Some(Vec3::ZERO);
    }
    enemy.cooldown = match def.attack {
        Attack::Lunge(lunge) => lunge.cooldown_ticks,
        Attack::Shoot(shoot) => between(rng, shoot.cooldown_ticks),
    };
    enemy.enter(EnemyState::Aggro);
    Some(Vec3::ZERO)
}

/// A shot along its facing, tilted up or down to the hero's middle: a hero who outruns
/// its turn is missed.
fn fire(
    enemy: &EnemyMotion,
    shoot: &Shoot,
    hero: Option<HeroView>,
    around: &Surroundings,
) -> ShotMotion {
    let facing = enemy.facing();
    let from = muzzle(enemy, shoot);
    let direction = match hero {
        Some(hero) => {
            let centre = around.hero_shape.centre(hero.position);
            let along = flat(centre - from).length().max(0.1);
            (facing * along + Vec3::Y * (centre.y - from.y)).normalize()
        }
        None => facing,
    };
    ShotMotion::new(from, direction * shoot.shot.speed, &shoot.shot)
}

fn muzzle(enemy: &EnemyMotion, shoot: &Shoot) -> Vec3 {
    enemy.position + Vec3::Y * shoot.muzzle_height + enemy.facing() * shoot.muzzle_ahead
}

/// The knockback of `hit` along `direction`.
pub(crate) fn knockback(direction: Vec3, hit: &HeroHit) -> Vec3 {
    flat(direction).normalize_or_zero() * hit.knockback + Vec3::Y * hit.lift
}

fn give_up(enemy: &mut EnemyMotion, def: &EnemyDef) {
    enemy.target = None;
    enemy.calm = def.senses.calm_ticks;
    enemy.enter(EnemyState::Patrol);
}

fn respawn(enemy: &mut EnemyMotion) {
    let patrol = std::mem::take(&mut enemy.patrol);
    *enemy = EnemyMotion::new(enemy.home, enemy.home_yaw, patrol);
}

/// The hero it is after, while he lives.
fn target<'a>(enemy: &EnemyMotion, around: &'a Surroundings) -> Option<&'a HeroView> {
    let player = enemy.target?;
    around
        .heroes
        .iter()
        .find(|hero| hero.player == player && hero.alive)
}

/// The nearest living hero it sees or hears, unless it is calm.
fn notice(enemy: &EnemyMotion, def: &EnemyDef, around: &Surroundings) -> Option<usize> {
    if enemy.calm > 0 {
        return None;
    }
    let eye = enemy.eye(def);
    let mut nearest: Option<(usize, f32)> = None;
    for hero in around.heroes.iter().filter(|hero| hero.alive) {
        let centre = around.hero_shape.centre(hero.position);
        let distance = centre.distance(eye);
        let heard = distance <= def.senses.hearing;
        let seen = distance <= def.senses.sight
            && angle_off(enemy.yaw, flat(centre - eye)) <= def.senses.sight_degrees.to_radians()
            && clear_line(around.world, eye, centre);
        if (heard || seen) && nearest.is_none_or(|(_, best)| distance < best) {
            nearest = Some((hero.player, distance));
        }
    }
    nearest.map(|(player, _)| player)
}

fn clear_line(world: &dyn CollisionWorld, from: Vec3, to: Vec3) -> bool {
    world.cast_ray(from, to - from).is_none()
}

/// Turns towards `to` and walks the way it faces, slower the further it has to turn, so it
/// turns on the spot rather than circling.
fn walk_toward(enemy: &mut EnemyMotion, def: &EnemyDef, to: Vec3, speed: f32) -> Vec3 {
    turn_toward(enemy, yaw_of(to), def);
    enemy.facing() * speed * ops::cos(angle_off(enemy.yaw, to)).max(0.0)
}

fn turn_toward(enemy: &mut EnemyMotion, yaw: f32, def: &EnemyDef) {
    let most = def.moves.turn_degrees_per_second.to_radians() * TICK;
    enemy.yaw = wrap(enemy.yaw + wrap(yaw - enemy.yaw).clamp(-most, most));
}

/// Moves it by its velocity, eased towards `wish`: across the ground with a sphere swept
/// above its steps, sliding along walls, then down onto the ground or falling.
fn travel(enemy: &mut EnemyMotion, def: &EnemyDef, wish: Option<Vec3>, around: &Surroundings) {
    let mut across = flat(enemy.velocity);
    if let Some(wish) = wish {
        across += (wish - across).clamp_length_max(def.moves.accel * TICK);
    }
    let crowding = if enemy.is_alive() {
        crowd_push(enemy, def, around)
    } else {
        Vec3::ZERO
    };
    let mut motion = (across + crowding) * TICK;
    let mut centre = enemy.position + Vec3::Y * (STEP + def.radius);
    for _ in 0..3 {
        let length = motion.length();
        if length < 1e-6 {
            break;
        }
        let Some(hit) = around.world.cast_sphere(def.radius, centre, motion) else {
            centre += motion;
            break;
        };
        centre += motion * ((hit.fraction * length - SKIN).max(0.0) / length);
        let normal = flat(hit.normal).normalize_or_zero();
        let left = motion * (1.0 - hit.fraction);
        motion = left - normal * left.dot(normal);
        across -= normal * across.dot(normal).min(0.0);
    }
    enemy.position.x = centre.x;
    enemy.position.z = centre.z;

    let fall = (-enemy.velocity.y * TICK).max(0.0);
    let reach = STEP + if enemy.on_ground { SNAP } else { 0.0 } + fall;
    let origin = Vec3::new(centre.x, enemy.position.y + STEP, centre.z);
    let ground = (enemy.velocity.y <= 0.0)
        .then(|| around.world.cast_ray(origin, Vec3::NEG_Y * reach))
        .flatten()
        .filter(|hit| hit.normal.y > 0.5);
    match ground {
        Some(hit) => {
            enemy.position.y = origin.y - hit.fraction * reach;
            enemy.velocity.y = 0.0;
            enemy.on_ground = true;
        }
        None => {
            enemy.velocity.y -= around.gravity * TICK;
            enemy.position.y += enemy.velocity.y * TICK;
            enemy.on_ground = false;
        }
    }
    enemy.velocity = Vec3::new(across.x, enemy.velocity.y, across.z);
}

/// The push apart from the living heroes and enemies it overlaps.
fn crowd_push(enemy: &EnemyMotion, def: &EnemyDef, around: &Surroundings) -> Vec3 {
    let heroes = around
        .heroes
        .iter()
        .filter(|hero| hero.alive)
        .map(|hero| (hero.position, around.hero_shape.radius));
    let mut push = Vec3::ZERO;
    for (at, radius) in around.crowd.iter().copied().chain(heroes) {
        let away = flat(enemy.position - at);
        let distance = away.length();
        let overlap = def.radius + radius - distance;
        if overlap <= 0.0 {
            continue;
        }
        // Two on the same spot part along x, the same way on every peer.
        let direction = if distance > 1e-4 {
            away / distance
        } else {
            Vec3::X
        };
        push += direction * overlap * CROWD_PUSH;
    }
    push
}

/// A whole number of ticks in `low..=high`.
pub(crate) fn between(rng: &mut SimRng, (low, high): (u32, u32)) -> u32 {
    low + rng.below(high.saturating_sub(low).saturating_add(1))
}

pub(crate) fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

fn facing(yaw: f32) -> Vec3 {
    Vec3::new(-ops::sin(yaw), 0.0, -ops::cos(yaw))
}

fn yaw_of(direction: Vec3) -> f32 {
    ops::atan2(-direction.x, -direction.z)
}

/// Its right, for a facing along the ground: −z's is +x.
fn right_of(facing: Vec3) -> Vec3 {
    Vec3::new(-facing.z, 0.0, facing.x)
}

/// How far `direction` is from the facing `yaw`, 0 to π; 0 for no direction.
fn angle_off(yaw: f32, direction: Vec3) -> f32 {
    if direction.length_squared() < 1e-8 {
        return 0.0;
    }
    wrap(yaw_of(direction) - yaw).abs()
}

/// `angle` brought into −π..π.
fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn distance_to_segment(point: Vec3, a: Vec3, b: Vec3) -> f32 {
    let along = b - a;
    let t = if along.length_squared() > 0.0 {
        ((point - a).dot(along) / along.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    point.distance(a + along * t)
}

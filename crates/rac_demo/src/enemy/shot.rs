//! The shooter's shots: balls that fly straight, hurt the first hero they touch and stop at
//! the first wall.

use bevy::prelude::*;
use rac_hero::CollisionWorld;

use super::brain::{HeroShape, HeroStrike, HeroView, TICK, knockback};
use super::def::{HeroHit, ShotDef};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotMotion {
    pub position: Vec3,
    /// u/s.
    pub velocity: Vec3,
    pub radius: f32,
    pub ticks_left: u32,
    pub hit: HeroHit,
}

impl ShotMotion {
    pub fn new(position: Vec3, velocity: Vec3, def: &ShotDef) -> Self {
        Self {
            position,
            velocity,
            radius: def.radius,
            ticks_left: def.life_ticks,
            hit: def.hit,
        }
    }
}

/// How a shot ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShotEnd {
    Hero(HeroStrike),
    /// It hit something solid, where it now is.
    Wall,
    /// It flew its time out.
    Fizzled,
}

/// One tick of a shot: `None` while it flies on.
pub fn step(
    shot: &mut ShotMotion,
    heroes: &[HeroView],
    hero_shape: &HeroShape,
    world: &dyn CollisionWorld,
) -> Option<ShotEnd> {
    let motion = shot.velocity * TICK;
    let wall = world.cast_sphere(shot.radius, shot.position, motion);
    shot.position += motion * wall.map_or(1.0, |hit| hit.fraction);
    // A tick's flight is shorter than a hero is wide, so testing where it got to misses no
    // one.
    for hero in heroes.iter().filter(|hero| hero.alive) {
        if hero_shape.distance(hero.position, shot.position) <= shot.radius {
            return Some(ShotEnd::Hero(HeroStrike {
                player: hero.player,
                damage: shot.hit.damage,
                knockback: knockback(shot.velocity, &shot.hit),
            }));
        }
    }
    if wall.is_some() {
        return Some(ShotEnd::Wall);
    }
    shot.ticks_left = shot.ticks_left.saturating_sub(1);
    (shot.ticks_left == 0).then_some(ShotEnd::Fizzled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_hero::{BodyPlacement, Collider, ParryWorld};

    const SHAPE: HeroShape = HeroShape {
        radius: 0.45,
        bottom: 0.5,
        top: 1.05,
    };

    fn shot(velocity: Vec3) -> ShotMotion {
        ShotMotion::new(
            Vec3::new(0.0, 1.0, 0.0),
            velocity,
            &ShotDef {
                speed: velocity.length(),
                radius: 0.2,
                life_ticks: 30,
                hit: HeroHit {
                    damage: 1,
                    knockback: 5.0,
                    lift: 3.0,
                },
            },
        )
    }

    fn fly(shot: &mut ShotMotion, heroes: &[HeroView], world: &ParryWorld) -> (u32, ShotEnd) {
        for tick in 1..1000 {
            if let Some(end) = step(shot, heroes, &SHAPE, world) {
                return (tick, end);
            }
        }
        panic!("the shot never ended");
    }

    #[test]
    fn a_shot_hits_the_hero_in_its_path_and_knocks_him_along_it() {
        let world = ParryWorld::new(&[]);
        let hero = HeroView {
            player: 1,
            position: Vec3::new(0.0, 0.0, -5.0),
            alive: true,
        };
        let mut shot = shot(Vec3::new(0.0, 0.0, -12.0));
        let (ticks, end) = fly(&mut shot, &[hero], &world);
        // 5 u less his and its radii, at 0.2 u a tick.
        assert!((21..=23).contains(&ticks), "{ticks}");
        let ShotEnd::Hero(strike) = end else {
            panic!("{end:?}")
        };
        assert_eq!(strike.player, 1);
        assert_eq!(strike.knockback, Vec3::new(0.0, 3.0, -5.0));
    }

    #[test]
    fn a_shot_passes_a_dead_hero_and_stops_at_a_wall() {
        let world = ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(2.0, 2.0, 0.5),
            },
            Vec3::new(0.0, 1.0, -5.5),
            Quat::IDENTITY,
        )]);
        let hero = HeroView {
            player: 0,
            position: Vec3::new(0.0, 0.0, -3.0),
            alive: false,
        };
        let mut shot = shot(Vec3::new(0.0, 0.0, -12.0));
        let (_, end) = fly(&mut shot, &[hero], &world);
        assert_eq!(end, ShotEnd::Wall);
        assert!((shot.position.z - -4.8).abs() < 1e-3, "{}", shot.position);
    }

    #[test]
    fn a_shot_that_hits_nothing_fizzles_out() {
        let world = ParryWorld::new(&[]);
        let mut shot = shot(Vec3::new(12.0, 0.0, 0.0));
        assert_eq!(fly(&mut shot, &[], &world), (30, ShotEnd::Fizzled));
    }
}

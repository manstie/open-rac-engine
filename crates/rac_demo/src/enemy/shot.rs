//! The shooter's shots: balls that fly straight, hurt the first hero they touch and stop at
//! the first wall. One fired at another enemy (`brain`'s foes) hurts the first enemy but its
//! shooter instead, and passes the heroes.

use bevy::prelude::*;
use rac_hero::CollisionWorld;

use super::brain::{
    EnemyStrike, EnemyView, HeroShape, HeroStrike, HeroView, TICK, flat, knockback,
};
use super::def::{HeroHit, ShotDef};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotMotion {
    pub position: Vec3,
    /// u/s.
    pub velocity: Vec3,
    pub radius: f32,
    pub ticks_left: u32,
    pub hit: HeroHit,
    /// Fired by this enemy at another.
    pub turned: Option<Entity>,
}

impl ShotMotion {
    pub fn new(position: Vec3, velocity: Vec3, def: &ShotDef) -> Self {
        Self {
            position,
            velocity,
            radius: def.radius,
            ticks_left: def.life_ticks,
            hit: def.hit,
            turned: None,
        }
    }
}

/// How a shot ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShotEnd {
    Hero(HeroStrike),
    Enemy(EnemyStrike),
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
    enemies: &[EnemyView],
    world: &dyn CollisionWorld,
) -> Option<ShotEnd> {
    let motion = shot.velocity * TICK;
    let wall = world.cast_sphere(shot.radius, shot.position, motion);
    shot.position += motion * wall.map_or(1.0, |hit| hit.fraction);
    // A tick's flight is shorter than a body is wide, so testing where it got to misses no
    // one.
    if let Some(shooter) = shot.turned {
        let struck = enemies.iter().find(|enemy| {
            enemy.entity != shooter && enemy.alive && enemy.distance(shot.position) <= shot.radius
        });
        if let Some(enemy) = struck {
            return Some(ShotEnd::Enemy(EnemyStrike {
                target: enemy.entity,
                by: shooter,
                damage: shot.hit.damage as f32,
                push: flat(knockback(shot.velocity, &shot.hit)),
            }));
        }
    }
    for hero in heroes
        .iter()
        .filter(|hero| hero.alive && shot.turned.is_none())
    {
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
            if let Some(end) = step(shot, heroes, &SHAPE, &[], world) {
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
    fn a_shot_at_another_enemy_passes_the_hero_and_its_shooter() {
        let world = ParryWorld::new(&[]);
        let shooter = Entity::from_raw_u32(1).unwrap();
        let foe = Entity::from_raw_u32(2).unwrap();
        let body = |entity, z| EnemyView {
            entity,
            position: Vec3::new(0.0, 0.0, z),
            radius: 0.5,
            height: 1.3,
            alive: true,
            infected: entity == shooter,
        };
        let enemies = [body(shooter, 0.0), body(foe, -6.0)];
        let hero = HeroView {
            player: 0,
            position: Vec3::new(0.0, 0.0, -3.0),
            alive: true,
        };
        let mut shot = shot(Vec3::new(0.0, 0.0, -12.0));
        shot.turned = Some(shooter);
        let end = (0..60)
            .find_map(|_| step(&mut shot, &[hero], &SHAPE, &enemies, &world))
            .expect("the shot never ended");
        assert_eq!(
            end,
            ShotEnd::Enemy(EnemyStrike {
                target: foe,
                by: shooter,
                damage: 1.0,
                push: Vec3::new(0.0, 0.0, -5.0),
            })
        );
        assert!((shot.position.z - -5.3).abs() < 0.25, "{}", shot.position);
    }

    #[test]
    fn a_shot_that_hits_nothing_fizzles_out() {
        let world = ParryWorld::new(&[]);
        let mut shot = shot(Vec3::new(12.0, 0.0, 0.0));
        assert_eq!(fly(&mut shot, &[], &world), (30, ShotEnd::Fizzled));
    }
}

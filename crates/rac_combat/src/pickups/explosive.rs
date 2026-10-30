//! The explosive crate (`crate_update`, 0x33A5C0; `weapons.md`, "Ammo pickups, crates and
//! the vendor"): touched, its [`Fuse`] is lit for about three seconds; hit, for a few
//! ticks; caught in another's blast, for one. When it runs out the crate goes off: a
//! [`Blast`] grows round its middle, hurting the targets and heroes it reaches.

use bevy::prelude::*;
use rac_hero::{Collider, Hero, HeroBody, HeroState, HeroTuning, HurtHero};
use rac_sim::SimTransform;
use serde::{Deserialize, Serialize};

use super::{PickupSound, PickupSoundKind, PickupTuning, crates::Crate, crates::CrateKind};
use crate::{Destroyed, HitReaction, Hitpoints, Hurtbox, Struck, position};

/// The fuse and the blast.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExplosiveTuning {
    /// Ticks of fuse when a hero touches it, and when it is hit (a range).
    pub touched_ticks: u32,
    pub hit_ticks: (u32, u32),
    /// Ticks the blast takes to grow to `reach` round the crate's middle, hurting targets
    /// it reaches for `damage`, and to `hero_reach` for heroes, who take `hero_damage`,
    /// pushed `push` u/s away and `lift` up (which their hit caps).
    pub blast_ticks: u32,
    pub reach: f32,
    pub damage: f32,
    pub hero_reach: f32,
    pub hero_damage: u32,
    pub push: f32,
    pub lift: f32,
    /// A hero this close to its sides, or standing on it, touches it.
    pub touch: f32,
}

/// A lit explosive crate's fuse: the ticks left before it goes off, and who lit it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fuse {
    pub left: u32,
    pub player: usize,
}

impl Fuse {
    /// Hit while lit: cut to `ticks` if more are left than a hit's fuse could be.
    pub fn cut(&mut self, ticks: u32) {
        if self.left >= 16 {
            self.left = ticks;
        }
    }

    /// Whether it shows its warning tint this tick: the last 19 ticks of each second left,
    /// and all of its last 14.
    pub fn flashing(&self) -> bool {
        self.left % 60 >= 41 || self.left <= 14
    }

    /// Whether it beeps this tick.
    pub fn beeps(&self) -> bool {
        self.left % 60 == 57 || self.left == 13
    }
}

/// An explosive crate's blast growing round `at`, set off by `player`; the targets and
/// heroes it has hurt, each once.
#[derive(Component, Debug, Clone, PartialEq)]
#[require(SimTransform)]
pub struct Blast {
    pub at: Vec3,
    pub player: usize,
    pub tick: u32,
    pub hit: Vec<Entity>,
    pub heroes_hit: Vec<usize>,
}

/// An explosive crate went off.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct CrateBlast {
    pub at: Vec3,
    pub player: usize,
}

/// How far a hero, a capsule standing at `feet`, is from touching a box of `half` round
/// `middle`, and whether he stands on it, to within `touch`.
pub fn touches(
    feet: Vec3,
    (radius, bottom, top): (f32, f32, f32),
    middle: Vec3,
    half: Vec3,
    touch: f32,
) -> bool {
    let across =
        (Vec2::new(feet.x - middle.x, feet.z - middle.z).abs() - half.xz()).max(Vec2::ZERO);
    let on_top = across == Vec2::ZERO && (feet.y - (middle.y + half.y)).abs() <= touch;
    // The capsule's line runs straight up from his lower sphere to his upper.
    let (low, high) = (feet.y + bottom, feet.y + top);
    let below = (middle.y - half.y) - high;
    let above = low - (middle.y + half.y);
    let gap = Vec3::new(across.x, below.max(above).max(0.0), across.y);
    on_top || gap.length() <= radius + touch
}

/// Lights the fuse of each explosive crate a hero touches: at once while he grinds.
pub(crate) fn touch_crates(
    tuning: Res<PickupTuning>,
    hero_tuning: Res<HeroTuning>,
    heroes: Query<(&Hero, &HeroBody)>,
    crates: Query<
        (
            Entity,
            &Crate,
            Option<&Collider>,
            Option<&Hurtbox>,
            Option<&Transform>,
            Option<&SimTransform>,
        ),
        Without<Fuse>,
    >,
    mut commands: Commands,
) {
    let body = &hero_tuning.0.body;
    let shape = (body.radius, body.bottom, body.top);
    let mut heroes: Vec<_> = heroes.iter().collect();
    heroes.sort_by_key(|(hero, _)| hero.player);
    for (entity, kind, collider, hurtbox, transform, simulated) in &crates {
        if kind.0 != CrateKind::Explosive {
            continue;
        }
        let Some(middle) = position(transform, simulated) else {
            continue;
        };
        let half = match collider {
            Some(Collider::Cuboid { half_extents }) => *half_extents,
            _ => Vec3::splat(hurtbox.map_or(0.5, |hurtbox| hurtbox.radius)),
        };
        let toucher = heroes.iter().find(|(_, body)| {
            body.0.state != HeroState::Dead
                && touches(body.0.position, shape, middle, half, tuning.explosive.touch)
        });
        if let Some((hero, body)) = toucher {
            let grinding = matches!(body.0.state, HeroState::Grind | HeroState::GrindAttack(_));
            commands.entity(entity).insert(Fuse {
                left: if grinding {
                    1
                } else {
                    tuning.explosive.touched_ticks
                },
                player: hero.player,
            });
        }
    }
}

/// Burns each lit fuse down a tick, and sets off the crates whose fuses run out.
pub(crate) fn burn_fuses(
    mut fuses: Query<(
        Entity,
        &mut Fuse,
        Option<&Hurtbox>,
        Option<&Transform>,
        Option<&SimTransform>,
    )>,
    mut blasts: MessageWriter<CrateBlast>,
    mut sounds: MessageWriter<PickupSound>,
    mut commands: Commands,
) {
    let mut fuses: Vec<_> = fuses.iter_mut().collect();
    fuses.sort_by_key(|(entity, ..)| *entity);
    for (entity, fuse, hurtbox, transform, simulated) in &mut fuses {
        let Some(at) = position(*transform, *simulated) else {
            continue;
        };
        let at = at + Vec3::Y * hurtbox.map_or(0.0, |hurtbox| hurtbox.height);
        if fuse.left > 1 {
            fuse.left -= 1;
            if fuse.beeps() {
                sounds.write(PickupSound::at(PickupSoundKind::FuseBeep, at));
            }
            continue;
        }
        sounds.write(PickupSound::at(
            PickupSoundKind::CrateBroke(CrateKind::Explosive),
            at,
        ));
        commands.entity(*entity).despawn();
        commands.spawn((
            Name::new("crate blast"),
            Blast {
                at,
                player: fuse.player,
                tick: 0,
                hit: Vec::new(),
                heroes_hit: Vec::new(),
            },
            SimTransform::from_translation(at),
        ));
        blasts.write(CrateBlast {
            at,
            player: fuse.player,
        });
    }
}

type BlastTarget<'a> = (
    Entity,
    &'a Hurtbox,
    Option<&'a Transform>,
    Option<&'a SimTransform>,
    Option<&'a mut Hitpoints>,
    &'a mut HitReaction,
    Option<&'a Crate>,
    Option<&'a mut Fuse>,
);

/// Grows each blast a tick, hurting what it newly reaches: explosive crates' fuses go to
/// one tick, other targets take its damage, and heroes are hurt and pushed away.
#[allow(clippy::too_many_arguments)]
pub(crate) fn grow_blasts(
    tuning: Res<PickupTuning>,
    hero_tuning: Res<HeroTuning>,
    heroes: Query<(&Hero, &HeroBody)>,
    mut blasts: Query<(Entity, &mut Blast)>,
    mut targets: Query<BlastTarget, Without<Blast>>,
    mut struck: MessageWriter<Struck>,
    mut destroyed: MessageWriter<Destroyed>,
    mut hurt: MessageWriter<HurtHero>,
    mut commands: Commands,
) {
    let tuning = &tuning.explosive;
    let body = &hero_tuning.0.body;
    let middle = Vec3::Y * ((body.bottom + body.top) / 2.0);
    let mut heroes: Vec<_> = heroes.iter().collect();
    heroes.sort_by_key(|(hero, _)| hero.player);
    let mut blasts: Vec<_> = blasts.iter_mut().collect();
    blasts.sort_by_key(|(entity, _)| *entity);
    let mut targets: Vec<_> = targets.iter_mut().collect();
    targets.sort_by_key(|(entity, ..)| *entity);
    for (entity, blast) in &mut blasts {
        let grown = blast.tick as f32 / tuning.blast_ticks.max(1) as f32;
        for (target, hurtbox, transform, simulated, hitpoints, reaction, kind, fuse) in &mut targets
        {
            if blast.hit.contains(target) {
                continue;
            }
            let Some(at) = position(*transform, *simulated) else {
                continue;
            };
            let centre = at + Vec3::Y * hurtbox.height;
            if centre.distance(blast.at) > tuning.reach * grown + hurtbox.radius {
                continue;
            }
            blast.hit.push(*target);
            if kind.is_some_and(|kind| kind.0 == CrateKind::Explosive) {
                match fuse {
                    Some(fuse) => fuse.left = 1,
                    None => {
                        commands.entity(*target).insert(Fuse {
                            left: 1,
                            player: blast.player,
                        });
                    }
                }
                continue;
            }
            reaction.flash_ticks = reaction.flash_ticks.max(1);
            struck.write(Struck {
                target: *target,
                player: blast.player,
                damage: tuning.damage,
                push: Vec3::ZERO,
                weapon: None,
            });
            if let Some(hitpoints) = hitpoints {
                let was_alive = hitpoints.current > 0.0;
                hitpoints.current = (hitpoints.current - tuning.damage).max(0.0);
                if was_alive && hitpoints.current <= 0.0 {
                    destroyed.write(Destroyed {
                        target: *target,
                        player: blast.player,
                        weapon: None,
                    });
                }
            }
        }
        for (hero, body) in &heroes {
            if blast.heroes_hit.contains(&hero.player) || body.0.state == HeroState::Dead {
                continue;
            }
            let centre = body.0.position + middle;
            if centre.distance(blast.at) > tuning.hero_reach * grown {
                continue;
            }
            blast.heroes_hit.push(hero.player);
            let away = (centre - blast.at).with_y(0.0).normalize_or_zero();
            hurt.write(HurtHero {
                player: hero.player,
                damage: tuning.hero_damage,
                knockback: away * tuning.push + Vec3::Y * tuning.lift,
            });
        }
        blast.tick += 1;
        if blast.tick > tuning.blast_ticks {
            commands.entity(*entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHAPE: (f32, f32, f32) = (0.4, 0.5, 1.1);
    const HALF: Vec3 = Vec3::splat(0.45);
    const MIDDLE: Vec3 = Vec3::new(0.0, 0.45, 0.0);

    #[test]
    fn a_hero_touches_a_crate_against_its_side_or_standing_on_it() {
        // Against its side, his capsule's radius from it.
        assert!(touches(
            Vec3::new(0.85, 0.0, 0.0),
            SHAPE,
            MIDDLE,
            HALF,
            0.05
        ));
        assert!(!touches(
            Vec3::new(1.0, 0.0, 0.0),
            SHAPE,
            MIDDLE,
            HALF,
            0.05
        ));
        // On its top, and over its corner.
        assert!(touches(Vec3::new(0.2, 0.9, 0.1), SHAPE, MIDDLE, HALF, 0.05));
        assert!(!touches(
            Vec3::new(0.2, 1.4, 0.1),
            SHAPE,
            MIDDLE,
            HALF,
            0.05
        ));
    }

    #[test]
    fn its_fuse_warns_each_second_and_through_its_last_ticks() {
        let fuse = |left| Fuse { left, player: 0 };
        let flashes: Vec<u32> = (1..=179).filter(|&left| fuse(left).flashing()).collect();
        assert!(flashes.contains(&179) && flashes.contains(&161) && !flashes.contains(&160));
        assert!(flashes.contains(&14) && !flashes.contains(&15));
        let beeps: Vec<u32> = (1..=179).filter(|&left| fuse(left).beeps()).collect();
        assert_eq!(beeps, [13, 57, 117, 177]);
        // A hit cuts a long fuse short, not a short one.
        let mut long = fuse(150);
        long.cut(9);
        assert_eq!(long.left, 9);
        let mut short = fuse(12);
        short.cut(9);
        assert_eq!(short.left, 12);
    }
}

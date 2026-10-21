//! The hero's guns (PLAN.md P1.9), after UYA (`docs/research/weapons.md`).
//!
//! - [`Armory`] holds a game's guns as RON definitions: each one's levels, [`FireMode`]
//!   and mods. [`Armory::uya`] is UYA NTSC-U's six guns whose code is read.
//! - An [`Arsenal`] beside a hero holds what he owns: ammo, XP and levels, the gun the fire
//!   button brings out, the quick-select rings ([`ring`]), and what is in his hand.
//! - Each tick, before the heroes move, [`fire::step`] runs every hero's guns and their
//!   shots, and tells his moves what he holds ([`rac_hero::GunHand`]) and whether lock-on
//!   is allowed. Then the hits go through the damage queue: a target takes only its
//!   strongest hit a tick, and none while it recovers from the last. Kills give their
//!   [`KillXp`] to the gun.
//! - The shock and acid mods follow the hits; [`GunEvent`]s tell the game what to show and
//!   play.

pub mod aim;
pub mod arsenal;
pub mod def;
pub mod fire;
pub mod ring;
pub mod shots;

use bevy::prelude::*;
use rac_hero::{CollisionScene, FollowCamera, Hero, HeroBody, HeroTuning, LockOn};
use rac_input::PlayerInputs;
use rac_sim::{SimRng, SimTransform};

use crate::{CombatTuning, Destroyed, HitReaction, Hitpoints, Hurtbox, Struck, position};

pub use aim::GunTarget;
pub use arsenal::{Arsenal, Hand, LevelUp, OwnedGun};
pub use def::{Armory, FireMode, Mods, WeaponDef, WeaponId};
pub use fire::{GunInput, Hit, Outcome, View};
pub use ring::{OpenRing, QuickSelect, RingRules};
pub use shots::{Shot, ShotKind};

/// The XP a target gives the gun that kills it (UYA's per-class table, 0x4412D0).
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct KillXp(pub u32);

/// Something a gun did, for the game to show and play (with the gun's [`def::Effects`]).
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct GunEvent {
    pub player: usize,
    pub weapon: WeaponId,
    pub kind: GunEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GunEventKind {
    /// A shot left the muzzle, or a stream took a round.
    Fired { at: Vec3, direction: Vec3 },
    /// One of a hitscan's rays, from the muzzle to where it stopped.
    Ray { from: Vec3, to: Vec3 },
    /// A shot hit something, burst or splashed.
    Impact { at: Vec3 },
    /// The shock mod's arc from the target hit to another.
    Arc { from: Vec3, to: Vec3 },
    /// The gun reached a new level.
    LevelUp { level: u8 },
}

/// A target burning with acid (`weapon_acid_mod`, 0x31AC38).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burn {
    pub target: Entity,
    pub player: usize,
    pub weapon: WeaponId,
    /// Health off each tick.
    pub per_tick: f32,
    pub ticks_left: u32,
}

/// The targets burning with acid, at most `acid_slots` of them, each once.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct Burns(pub Vec<Burn>);

/// A hit waiting in the damage queue.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Queued {
    player: usize,
    hit: Hit,
}

/// Adds a hit to the queue as `damage_queue_add` (0x443938) does: one pending hit per
/// target, replaced by one doing at least as much damage; a weaker one only adds its mods.
fn queue(pending: &mut Vec<(Entity, Queued)>, player: usize, hit: Hit) {
    match pending.iter_mut().find(|(target, _)| *target == hit.target) {
        Some((_, queued)) => {
            let shock = queued.hit.shock || hit.shock;
            let acid = queued.hit.acid || hit.acid;
            if hit.damage >= queued.hit.damage {
                *queued = Queued { player, hit };
            }
            queued.hit.shock = shock;
            queued.hit.acid = acid;
        }
        None => pending.push((hit.target, Queued { player, hit })),
    }
}

type TargetItem<'a> = (
    Entity,
    &'a Hurtbox,
    Option<&'a Transform>,
    Option<&'a SimTransform>,
    Option<&'a mut Hitpoints>,
    &'a mut HitReaction,
    Option<&'a KillXp>,
);

/// Runs every hero's guns, then puts their hits through the damage queue, the shock and
/// acid mods, and gives kills' XP to the guns.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fire_guns(
    armory: Res<Armory>,
    tuning: Res<CombatTuning>,
    hero_tuning: Res<HeroTuning>,
    inputs: Res<PlayerInputs>,
    scene: Res<CollisionScene>,
    mut rng: ResMut<SimRng>,
    mut burns: ResMut<Burns>,
    cameras: Query<(&FollowCamera, &SimTransform)>,
    mut heroes: Query<(&Hero, &mut HeroBody, &mut Arsenal, &mut LockOn)>,
    mut targets: Query<TargetItem>,
    mut events: MessageWriter<GunEvent>,
    mut struck: MessageWriter<Struck>,
    mut destroyed: MessageWriter<Destroyed>,
) {
    let mut list: Vec<GunTarget> = targets
        .iter()
        .filter_map(|(entity, hurtbox, transform, simulated, hitpoints, ..)| {
            let at = position(transform, simulated)?;
            Some(GunTarget {
                entity,
                centre: at + Vec3::Y * hurtbox.height,
                radius: hurtbox.radius,
                alive: hitpoints.is_none_or(|hitpoints| hitpoints.current > 0.0),
            })
        })
        .collect();
    list.sort_by_key(|target| target.entity);

    let mut order: Vec<_> = heroes.iter_mut().collect();
    order.sort_by_key(|(hero, ..)| hero.player);
    let mut pending: Vec<(Entity, Queued)> = Vec::new();
    // Each gun's damage now, which its shock arcs take a share of.
    let mut gun_damage: Vec<((usize, WeaponId), f32)> = Vec::new();
    for (hero, body, arsenal, lock) in &mut order {
        let view = cameras
            .iter()
            .find(|(camera, _)| camera.player == hero.player)
            .map(|(camera, transform)| View {
                setting: camera.setting_with(&hero_tuning.0.camera),
                eye: transform.translation,
                direction: transform.rotation * Vec3::NEG_Z,
            });
        let input = inputs.get(hero.player);
        let outcome = fire::step(
            arsenal,
            &armory,
            &GunInput {
                input: &input,
                hero: &body.0,
                lock: lock.target.map(|target| (target, lock.at)),
                view,
                world: &scene.0,
                targets: &list,
            },
            &mut rng,
        );
        body.0.gun = outcome.hand;
        lock.allowed = outcome.lock_on;
        for (weapon, kind) in outcome.events {
            events.write(GunEvent {
                player: hero.player,
                weapon,
                kind,
            });
        }
        for gun in &arsenal.guns {
            if let Some(def) = armory.get(gun.weapon) {
                gun_damage.push(((hero.player, gun.weapon), def.level(gun.level).damage));
            }
        }
        for hit in outcome.hits {
            queue(&mut pending, hero.player, hit);
        }
    }

    // Each shocked hit arcs to the first other living target near it.
    let shock = armory.rules.shock;
    let arcs: Vec<(usize, Hit, Vec3, Vec3)> = pending
        .iter()
        .filter(|(_, queued)| queued.hit.shock)
        .filter_map(|(entity, queued)| {
            let from = list.iter().find(|target| target.entity == *entity)?;
            let to = list.iter().find(|target| {
                target.entity != *entity
                    && target.alive
                    && target.centre.distance(from.centre) <= shock.range
            })?;
            let damage = gun_damage
                .iter()
                .find(|(key, _)| *key == (queued.player, queued.hit.weapon))
                .map_or(0.0, |(_, damage)| *damage);
            let arc = Hit {
                target: to.entity,
                weapon: queued.hit.weapon,
                damage: damage * shock.share,
                push: Vec3::ZERO,
                shock: false,
                acid: false,
            };
            Some((queued.player, arc, from.centre, to.centre))
        })
        .collect();
    for (player, arc, from, to) in arcs {
        events.write(GunEvent {
            player,
            weapon: arc.weapon,
            kind: GunEventKind::Arc { from, to },
        });
        queue(&mut pending, player, arc);
    }
    pending.sort_by_key(|(entity, _)| *entity);

    let mut kills: Vec<(usize, WeaponId, u32)> = Vec::new();
    for (entity, Queued { player, hit }) in pending {
        let Ok((_, _, _, _, hitpoints, mut reaction, xp)) = targets.get_mut(entity) else {
            continue;
        };
        if reaction.gun_recovery > 0 {
            continue;
        }
        reaction.gun_recovery = tuning.gun_recovery_ticks;
        reaction.flash_ticks = tuning.flash_ticks;
        let push = hit.push * tuning.push_speed;
        if push != Vec3::ZERO {
            reaction.push = push;
        }
        struck.write(Struck {
            target: entity,
            player,
            damage: hit.damage,
            push,
            weapon: Some(hit.weapon),
        });
        let xp = xp.map_or(0, |xp| xp.0);
        if let Some(mut hitpoints) = hitpoints {
            let was_alive = hitpoints.current > 0.0;
            hitpoints.current = (hitpoints.current - hit.damage).max(0.0);
            if was_alive && hitpoints.current <= 0.0 {
                destroyed.write(Destroyed {
                    target: entity,
                    player,
                    weapon: Some(hit.weapon),
                });
                kills.push((player, hit.weapon, xp));
                continue;
            }
        }
        // Acid takes hold of a target that lives on, once at a time.
        let level = order
            .iter()
            .find(|(hero, ..)| hero.player == player)
            .and_then(|(_, _, arsenal, _)| arsenal.gun(hit.weapon))
            .map_or(0, |gun| gun.level);
        let acid = armory
            .get(hit.weapon)
            .and_then(|def| def.acid.at(level))
            .filter(|_| hit.acid);
        if let Some(acid) = acid
            && burns.0.len() < armory.rules.acid_slots
            && !burns.0.iter().any(|burn| burn.target == entity)
        {
            burns.0.push(Burn {
                target: entity,
                player,
                weapon: hit.weapon,
                per_tick: acid.per_second / 60.0,
                ticks_left: (acid.seconds * 60.0).round() as u32,
            });
        }
    }

    // Acid burns past the damage queue; the burn that kills credits the gun.
    burns.0.retain_mut(|burn| {
        let Ok((_, _, _, _, Some(mut hitpoints), _, xp)) = targets.get_mut(burn.target) else {
            return false;
        };
        if hitpoints.current <= 0.0 || burn.ticks_left == 0 {
            return false;
        }
        burn.ticks_left -= 1;
        hitpoints.current = (hitpoints.current - burn.per_tick).max(0.0);
        if hitpoints.current <= 0.0 {
            destroyed.write(Destroyed {
                target: burn.target,
                player: burn.player,
                weapon: Some(burn.weapon),
            });
            kills.push((burn.player, burn.weapon, xp.map_or(0, |xp| xp.0)));
            return false;
        }
        true
    });

    for (player, weapon, xp) in kills {
        let Some((_, _, arsenal, _)) = order.iter_mut().find(|(hero, ..)| hero.player == player)
        else {
            continue;
        };
        for up in arsenal.add_xp(weapon, xp, &armory) {
            events.write(GunEvent {
                player,
                weapon,
                kind: GunEventKind::LevelUp { level: up.level },
            });
        }
    }
}

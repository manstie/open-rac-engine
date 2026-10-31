//! The demo's enemies (PLAN.md P1.11): a melee charger and a ranged shooter, with numbers
//! of their own (`assets/enemies.ron`), not read from any game.
//!
//! - [`brain::step`] is an enemy's whole behaviour as one pure function. [`EnemyPlugin`]
//!   runs it for every [`Enemy`] each tick after the world moves and before the heroes do,
//!   so its hits ([`rac_hero::HurtHero`]) land that tick, and so it sees the guns' hits of
//!   this tick and the wrench's of the last in a fixed order.
//! - Enemies are targets like the melee yard's (`rac_combat`): the wrench and guns hit
//!   their hurtbox, each hit makes them reel, and the last kills them. Dead, they drop
//!   bolts, nanotech and ammo ([`crate::drops`]), and a while later come back at home.
//! - The shooter's shots are [`EnemyShot`]s.
//! - Infected by the Infector, an enemy fights the others, and they fight back: their hits
//!   on each other ([`FoeHits`]) land at the start of the next tick, and an enemy killed so
//!   drops what it would for a hero, without his XP.

pub mod brain;
pub mod def;
mod looks;
pub mod shot;

use bevy::prelude::*;
use rac_combat::pickups::{self, BoltBank, BoltMultiplier, PickupTuning};
use rac_combat::{
    Afflicted, Affliction, Armory, Arsenal, Destroyed, Hitpoints, Hostile, Hurtbox, KillXp, Struck,
    target_bundle, weapons::block_shot,
};
use rac_hero::{AimTarget, CollisionScene, Hero, HeroBody, HeroState, HeroTuning, HurtHero};
use rac_sim::{PreviousSimTransform, SimRng, SimSet, SimTransform};

use crate::DemoEntity;
use crate::drops;
use brain::{EnemyMotion, EnemyStrike, EnemyView, HeroShape, HeroView, Surroundings};
use def::{Bestiary, EnemyDef, EnemyKind};
use shot::{ShotEnd, ShotMotion};

pub use looks::EnemyLooks;

/// An enemy of a kind; what it is doing is in its [`EnemyBody`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[require(SimTransform)]
pub struct Enemy {
    pub kind: EnemyKind,
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct EnemyBody(pub EnemyMotion);

/// A shot an enemy fired.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(SimTransform)]
pub struct EnemyShot(pub ShotMotion);

/// Where a shot ended, for the few ticks it shows.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(SimTransform)]
pub struct ShotBurst {
    pub ticks_left: u32,
}

/// How long a shot's burst shows.
pub const BURST_TICKS: u32 = 10;

/// The enemies' hits on each other, landing at the start of the next tick.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct FoeHits(pub Vec<EnemyStrike>);

/// Runs the enemies and their shots in the simulation, and, once [`EnemyLooks`] exist,
/// shows them. Their drops are `rac_combat`'s pickups, which `rac_combat::CombatPlugin`
/// moves.
pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bestiary>()
            .init_resource::<FoeHits>()
            .add_systems(
                FixedUpdate,
                (run_enemies, fly_shots)
                    .chain()
                    .after(SimSet::World)
                    .before(SimSet::Hero),
            )
            .add_systems(FixedUpdate, fade_bursts.in_set(SimSet::Particles))
            .add_systems(
                Update,
                (looks::dress, looks::pose_enemies, looks::shrink_bursts)
                    .run_if(resource_exists::<EnemyLooks>),
            );
    }
}

/// What a new enemy needs, standing at `home` facing `yaw`, to walk round `patrol`.
pub fn enemy_bundle(
    kind: EnemyKind,
    def: &EnemyDef,
    home: Vec3,
    yaw: f32,
    patrol: Vec<Vec3>,
) -> impl Bundle {
    let motion = EnemyMotion::new(home, yaw, patrol);
    (
        Enemy { kind },
        hurtbox(def),
        Hostile,
        KillXp(def.kill_xp),
        SimTransform::from_translation(home).with_rotation(motion.rotation()),
        EnemyBody(motion),
    )
}

/// What the wrench and guns hit and aim at, and its hit points.
fn hurtbox(def: &EnemyDef) -> impl Bundle {
    target_bundle(def.radius + 0.1, def.height / 2.0, def.hitpoints)
}

fn hero_views(heroes: &Query<(&Hero, &HeroBody)>) -> Vec<HeroView> {
    let mut views: Vec<HeroView> = heroes
        .iter()
        .map(|(hero, body)| HeroView {
            player: hero.player,
            position: body.0.position,
            alive: body.0.state != HeroState::Dead,
        })
        .collect();
    views.sort_by_key(|view| view.player);
    views
}

/// Every enemy, in entity order.
fn enemy_views(
    bestiary: &Bestiary,
    enemies: &Query<(Entity, &Enemy, &EnemyBody)>,
) -> Vec<EnemyView> {
    let mut views: Vec<EnemyView> = enemies
        .iter()
        .map(|(entity, enemy, body)| EnemyView::of(entity, &body.0, bestiary.get(enemy.kind)))
        .collect();
    views.sort_by_key(|view| view.entity);
    views
}

type EnemyItem<'a> = (
    Entity,
    &'a Enemy,
    &'a mut EnemyBody,
    &'a mut Hitpoints,
    &'a mut SimTransform,
    &'a mut PreviousSimTransform,
);

/// Takes this tick's hits and kills, the enemies' on each other from the last tick among
/// them, then steps every enemy in entity order, so every peer draws the same random
/// numbers.
#[allow(clippy::too_many_arguments)]
fn run_enemies(
    bestiary: Res<Bestiary>,
    pickup_tuning: Res<PickupTuning>,
    hero_tuning: Res<HeroTuning>,
    scene: Res<CollisionScene>,
    mut rng: ResMut<SimRng>,
    mut bank: ResMut<BoltBank>,
    mut foe_hits: ResMut<FoeHits>,
    mut struck: MessageReader<Struck>,
    mut destroyed: MessageReader<Destroyed>,
    mut afflicted: MessageReader<Afflicted>,
    heroes: Query<(&Hero, &HeroBody)>,
    doublers: Query<(&Hero, &BoltMultiplier)>,
    mut enemies: Query<EnemyItem>,
    mut hurt: MessageWriter<HurtHero>,
    mut commands: Commands,
) {
    let views = hero_views(&heroes);
    let shape = HeroShape::from_profile(&hero_tuning.0.body);
    let mut enemies: Vec<_> = enemies.iter_mut().collect();
    enemies.sort_by_key(|(entity, ..)| *entity);
    let order: Vec<Entity> = enemies.iter().map(|(entity, ..)| *entity).collect();

    for hit in struck.read() {
        if let Ok(i) = order.binary_search(&hit.target) {
            brain::struck(&mut enemies[i].2.0, hit.player, hit.push);
        }
    }
    // Killed by another enemy, or by a hero below.
    let mut fallen = Vec::new();
    for hit in std::mem::take(&mut foe_hits.0) {
        let Ok(i) = order.binary_search(&hit.target) else {
            continue;
        };
        let (_, _, body, hitpoints, ..) = &mut enemies[i];
        if !body.0.is_alive() || hitpoints.current <= 0.0 {
            continue;
        }
        brain::struck_by(&mut body.0, hit.by, hit.push);
        hitpoints.current = (hitpoints.current - hit.damage).max(0.0);
        if hitpoints.current <= 0.0 {
            fallen.push((i, None));
        }
    }
    // Frozen, it pays its XP to the gun and no bolts (0x304A18).
    let mut frozen = Vec::new();
    for afflict in afflicted.read() {
        let Ok(i) = order.binary_search(&afflict.target) else {
            continue;
        };
        brain::afflict(&mut enemies[i].2.0, afflict.affliction);
        if afflict.affliction == Affliction::Frozen {
            frozen.push(i);
        }
        // A duck is no longer an enemy for the guns that take only enemies.
        if afflict.affliction == Affliction::Duck {
            commands.entity(enemies[i].0).remove::<Hostile>();
        }
    }
    fallen.extend(destroyed.read().filter_map(|kill| {
        let i = order.binary_search(&kill.target).ok()?;
        Some((i, Some(kill.player)))
    }));
    for (i, killer) in fallen {
        let (entity, enemy, body, ..) = &mut enemies[i];
        if !brain::killed(&mut body.0) {
            continue;
        }
        // Out of reach of the wrench, guns and lock-on until it comes back.
        commands.entity(*entity).remove::<(Hurtbox, AimTarget)>();
        if frozen.contains(&i) {
            continue;
        }
        let def = bestiary.get(enemy.kind);
        let at = body.0.position + Vec3::Y * (def.height / 2.0);
        // A hero's kill pays his doubling (`moby_killed`).
        let doubled = doublers
            .iter()
            .find(|(hero, _)| Some(hero.player) == killer)
            .map(|(_, doubled)| *doubled)
            .unwrap_or_default();
        let kinds = drops::drops(&def.drops, &doubled, &mut bank, &mut rng);
        pickups::spawn(
            &mut commands,
            pickups::scatter(at, kinds, &pickup_tuning, &mut rng),
        );
    }

    let others: Vec<EnemyView> = enemies
        .iter()
        .map(|(entity, enemy, body, ..)| EnemyView::of(*entity, &body.0, bestiary.get(enemy.kind)))
        .collect();
    for (entity, enemy, body, hitpoints, transform, previous) in &mut enemies {
        let def = bestiary.get(enemy.kind);
        let around = Surroundings {
            heroes: &views,
            hero_shape: shape,
            me: *entity,
            enemies: &others,
            gravity: bestiary.gravity,
            world: &scene.0,
        };
        let outcome = brain::step(&mut body.0, def, &around, &mut rng);
        foe_hits.0.extend(outcome.foe_hits);
        for strike in outcome.hits {
            hurt.write(HurtHero {
                player: strike.player,
                damage: strike.damage,
                knockback: strike.knockback,
            });
        }
        for shot in outcome.shots {
            commands.spawn((
                Name::new("enemy shot"),
                DemoEntity,
                EnemyShot(shot),
                SimTransform::from_translation(shot.position),
            ));
        }
        if outcome.respawned {
            **hitpoints = Hitpoints::full(def.hitpoints);
            commands.entity(*entity).insert((hurtbox(def), Hostile));
        }
        transform.translation = body.0.position;
        transform.rotation = body.0.rotation();
        if outcome.respawned {
            previous.0 = **transform;
        }
    }
}

/// Flies the shots, hurting the heroes they hit, or the enemies for one an enemy fired at
/// another. The heroes' holo shields stop them.
#[allow(clippy::too_many_arguments)]
fn fly_shots(
    bestiary: Res<Bestiary>,
    hero_tuning: Res<HeroTuning>,
    armory: Res<Armory>,
    scene: Res<CollisionScene>,
    heroes: Query<(&Hero, &HeroBody)>,
    enemies: Query<(Entity, &Enemy, &EnemyBody)>,
    mut arsenals: Query<(&Hero, &mut Arsenal)>,
    mut shots: Query<(Entity, &mut EnemyShot, &mut SimTransform)>,
    mut hurt: MessageWriter<HurtHero>,
    mut foe_hits: ResMut<FoeHits>,
    mut commands: Commands,
) {
    let views = hero_views(&heroes);
    let others = enemy_views(&bestiary, &enemies);
    let shape = HeroShape::from_profile(&hero_tuning.0.body);
    let mut shots: Vec<_> = shots.iter_mut().collect();
    shots.sort_by_key(|(entity, ..)| *entity);
    let mut arsenals: Vec<_> = arsenals.iter_mut().collect();
    arsenals.sort_by_key(|(hero, _)| hero.player);
    for (entity, shot, transform) in &mut shots {
        let motion = shot.0.velocity * brain::TICK;
        let blocked = arsenals
            .iter_mut()
            .find_map(|(_, arsenal)| block_shot(arsenal, &armory, shot.0.position, motion));
        let end = match blocked {
            Some(t) => {
                shot.0.position += motion * t;
                Some(ShotEnd::Wall)
            }
            None => shot::step(&mut shot.0, &views, &shape, &others, &scene.0),
        };
        transform.translation = shot.0.position;
        let Some(end) = end else {
            continue;
        };
        commands.entity(*entity).despawn();
        match end {
            ShotEnd::Hero(strike) => {
                hurt.write(HurtHero {
                    player: strike.player,
                    damage: strike.damage,
                    knockback: strike.knockback,
                });
            }
            ShotEnd::Enemy(strike) => foe_hits.0.push(strike),
            ShotEnd::Wall | ShotEnd::Fizzled => {}
        }
        if end != ShotEnd::Fizzled {
            commands.spawn((
                Name::new("shot burst"),
                DemoEntity,
                ShotBurst {
                    ticks_left: BURST_TICKS,
                },
                SimTransform::from_translation(shot.0.position),
            ));
        }
    }
}

fn fade_bursts(mut bursts: Query<(Entity, &mut ShotBurst)>, mut commands: Commands) {
    for (entity, mut burst) in &mut bursts {
        burst.ticks_left = burst.ticks_left.saturating_sub(1);
        if burst.ticks_left == 0 {
            commands.entity(entity).despawn();
        }
    }
}

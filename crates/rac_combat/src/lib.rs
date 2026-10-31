//! What the hero's attacks hit, and what that does to it (PLAN.md P1.8), and his guns
//! (P1.9).
//!
//! The hero kit's controller works out, each tick, where the wrench can hit
//! ([`rac_hero::Strike`]: a swing's swept line, the jump attack's slam, the thrown wrench).
//! This crate tests those against every [`Hurtbox`]:
//!
//! - a hit takes [`Hitpoints`], if the target has them, and [`Destroyed`] says when they
//!   run out;
//! - a [`HitReaction`] remembers the hit, so each attack hits a target once, and flashes
//!   and pushes it for the game to show;
//! - a [`Pushable`] target moves with the push.
//!
//! A hero with an [`Arsenal`] also has guns ([`weapons`]), whose shots hit the same
//! hurtboxes. Each hit is also sent as a [`Struck`] message. Enemies (P1.11) build on
//! these.
//!
//! [`pickups`] are what the hero collects (P1.10): bolts, nanotech and ammo that crates
//! (a [`Crate`] target breaks when [`Destroyed`]) and enemies leave, and a [`Vendor`]'s pad
//! whose menu sells guns and rounds for bolts.

pub mod pickups;
pub mod weapons;

use bevy::prelude::*;
use rac_hero::{AimTarget, Hero, HeroBody};
use rac_sim::{SimSet, SimTransform};

pub use pickups::{Bolts, Crate, CrateKind, Pickup, PickupKind, PickupTuning, Vendor, VendorMenu};
pub use weapons::{
    Afflicted, Affliction, Armory, Arsenal, Burns, GunEvent, GunEventKind, KillXp, Sustain,
    WeaponId,
};

/// Seconds per tick.
pub(crate) const TICK: f32 = 1.0 / 60.0;

/// How many recent attacks a target remembers being hit by.
const REMEMBERED: usize = 8;

/// A ball the hero's attacks can hit, `height` above the entity's position. The hero's
/// attacks also turn to its centre, as an [`AimTarget`].
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(HitReaction)]
pub struct Hurtbox {
    pub radius: f32,
    pub height: f32,
}

/// What a target can take before it is [`Destroyed`]. The wrench takes 1 a swing.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Hitpoints {
    pub current: f32,
    pub max: f32,
}

impl Hitpoints {
    pub fn full(max: f32) -> Self {
        Self { current: max, max }
    }
}

/// How a target shows being hit: a flash, and a push that dies away.
#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct HitReaction {
    /// Ticks left of the flash.
    pub flash_ticks: u32,
    /// u/s, slowing to a stop.
    pub push: Vec3,
    /// The attacks that hit it lately: the player and the attack's serial.
    struck_by: Vec<(usize, u32)>,
    /// Ticks left before a gun's hit can hurt it again.
    pub(crate) gun_recovery: u32,
}

/// A target that is an enemy (UYA's class category 5), not a crate or a dummy: the guns
/// that seek, infect, swallow or turn things into ducks only take enemies.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Hostile;

/// A target that the push moves (through its `SimTransform`).
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct Pushable;

/// A target was hit.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct Struck {
    pub target: Entity,
    /// Who hit it.
    pub player: usize,
    pub damage: f32,
    /// u/s.
    pub push: Vec3,
    /// The gun that hit it; `None` for the wrench.
    pub weapon: Option<WeaponId>,
}

/// A target's [`Hitpoints`] ran out. The game decides what that means: a crate breaks, an
/// enemy dies.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct Destroyed {
    pub target: Entity,
    pub player: usize,
    /// The gun that did it; `None` for the wrench.
    pub weapon: Option<WeaponId>,
}

/// How hits look. The games hand each target a push strength that the target's own code
/// interprets; these turn it into motion [placeholder].
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CombatTuning {
    /// u/s of push per unit of the attack's push strength.
    pub push_speed: f32,
    /// How fast a push dies away, u/s².
    pub push_brake: f32,
    pub flash_ticks: u32,
    /// Ticks after a gun's hit before a target takes another. The games leave it to each
    /// enemy's own code, which isn't read [placeholder].
    pub gun_recovery_ticks: u32,
}

impl Default for CombatTuning {
    fn default() -> Self {
        Self {
            push_speed: 4.0,
            push_brake: 20.0,
            flash_ticks: 8,
            gun_recovery_ticks: 8,
        }
    }
}

/// Tests the heroes' strikes against hurtboxes after the heroes move, fires their guns
/// before they move, as the games' gun mobys update before the hero, and moves pushed
/// targets with the world. After the hits, crates break, pickups move and are collected,
/// and vendors sell, in that order, since all three spend the same random numbers, ammo and
/// bolts.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombatTuning>()
            .init_resource::<Armory>()
            .init_resource::<Burns>()
            .init_resource::<weapons::Arcs>()
            .init_resource::<PickupTuning>()
            .init_resource::<pickups::BoltBank>()
            .init_resource::<pickups::CrateDebris>()
            .init_resource::<pickups::HelpGiven>()
            .add_message::<Struck>()
            .add_message::<Destroyed>()
            .add_message::<GunEvent>()
            .add_message::<Afflicted>()
            .add_message::<pickups::CrateBlast>()
            .add_message::<pickups::PickupSound>()
            .add_message::<pickups::HelpAsked>()
            .add_systems(
                FixedUpdate,
                (react, weapons::fire_guns).chain().in_set(SimSet::World),
            )
            .add_systems(
                FixedUpdate,
                strike_targets.after(SimSet::Hero).before(SimSet::Particles),
            )
            .add_systems(
                FixedUpdate,
                (
                    pickups::step_multipliers,
                    pickups::explosive::touch_crates,
                    pickups::explosive::burn_fuses,
                    pickups::explosive::grow_blasts,
                    pickups::crates::break_crates,
                    pickups::move_pickups,
                    pickups::nanotech::move_orbs,
                    pickups::vendor::run_vendors,
                )
                    .chain()
                    .in_set(SimSet::Particles),
            );
    }
}

/// What a new target needs: a hurtbox the hero can hit and aim at, and its hit points.
pub fn target_bundle(radius: f32, height: f32, hitpoints: f32) -> impl Bundle {
    (
        Hurtbox { radius, height },
        AimTarget { height },
        Hitpoints::full(hitpoints),
    )
}

/// Where a target is in the simulation: its `SimTransform` if it has one.
pub(crate) fn position(
    transform: Option<&Transform>,
    simulated: Option<&SimTransform>,
) -> Option<Vec3> {
    match simulated {
        Some(simulated) => Some(simulated.translation),
        None => transform.map(|transform| transform.translation),
    }
}

#[allow(clippy::too_many_arguments)]
fn strike_targets(
    tuning: Res<CombatTuning>,
    armory: Res<Armory>,
    mut heroes: Query<(
        &Hero,
        &HeroBody,
        Option<&mut Arsenal>,
        Option<&pickups::BoltMultiplier>,
    )>,
    mut targets: Query<(
        Entity,
        &Hurtbox,
        Option<&Transform>,
        Option<&SimTransform>,
        Option<&mut Hitpoints>,
        &mut HitReaction,
        Option<&KillXp>,
    )>,
    mut struck: MessageWriter<Struck>,
    mut destroyed: MessageWriter<Destroyed>,
    mut events: MessageWriter<GunEvent>,
) {
    let mut heroes: Vec<_> = heroes.iter_mut().collect();
    heroes.sort_by_key(|(hero, ..)| hero.player);
    let mut targets: Vec<_> = targets.iter_mut().collect();
    targets.sort_by_key(|(entity, ..)| *entity);

    for (hero, body, arsenal, doubled) in &mut heroes {
        // The Plasma Whip's rope is the gun in his hand: its kills give that gun XP.
        let gun = arsenal.as_ref().and_then(|arsenal| arsenal.hand.gun());
        let mut kills = Vec::new();
        for strike in &body.0.strikes {
            let weapon = if strike.gun { gun } else { None };
            for (entity, hurtbox, transform, simulated, hitpoints, reaction, xp) in &mut targets {
                let Some(at) = position(*transform, *simulated) else {
                    continue;
                };
                let mark = (hero.player, strike.serial);
                if reaction.struck_by.contains(&mark) {
                    continue;
                }
                let centre = at + Vec3::Y * hurtbox.height;
                if !strike.shape.touches(centre, hurtbox.radius) {
                    continue;
                }
                if reaction.struck_by.len() == REMEMBERED {
                    reaction.struck_by.remove(0);
                }
                reaction.struck_by.push(mark);
                reaction.flash_ticks = tuning.flash_ticks;
                let push = strike.push * tuning.push_speed;
                reaction.push = push;
                struck.write(Struck {
                    target: *entity,
                    player: hero.player,
                    damage: strike.damage,
                    push,
                    weapon,
                });
                if let Some(hitpoints) = hitpoints {
                    let was_alive = hitpoints.current > 0.0;
                    hitpoints.current = (hitpoints.current - strike.damage).max(0.0);
                    if was_alive && hitpoints.current <= 0.0 {
                        destroyed.write(Destroyed {
                            target: *entity,
                            player: hero.player,
                            weapon,
                        });
                        if let Some(weapon) = weapon {
                            kills.push((weapon, xp.map_or(0, |xp| xp.0)));
                        }
                    }
                }
            }
        }
        if let Some(arsenal) = arsenal {
            for (weapon, xp) in kills {
                let xp = doubled.map_or(xp, |doubled| doubled.apply(xp));
                for up in arsenal.add_xp(weapon, xp, &armory) {
                    events.write(GunEvent {
                        player: hero.player,
                        weapon,
                        kind: GunEventKind::LevelUp { level: up.level },
                        shot: None,
                    });
                }
            }
        }
    }
}

/// Counts flashes down and moves pushed targets, slowing their push.
fn react(
    tuning: Res<CombatTuning>,
    mut targets: Query<(&mut HitReaction, Option<&mut SimTransform>, Has<Pushable>)>,
) {
    for (mut reaction, simulated, pushable) in &mut targets {
        reaction.flash_ticks = reaction.flash_ticks.saturating_sub(1);
        reaction.gun_recovery = reaction.gun_recovery.saturating_sub(1);
        if reaction.push == Vec3::ZERO {
            continue;
        }
        if pushable && let Some(mut simulated) = simulated {
            let horizontal = Vec3::new(reaction.push.x, 0.0, reaction.push.z);
            simulated.translation += horizontal * TICK;
        }
        let speed = reaction.push.length();
        let slower = (speed - tuning.push_brake * TICK).max(0.0);
        reaction.push = if slower > 0.0 {
            reaction.push * (slower / speed)
        } else {
            Vec3::ZERO
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use rac_hero::{Collider, HeroPlugin, HeroProfile, hero_bundle};
    use rac_input::{Action, Buttons, PlayerInput, PlayerInputs};
    use rac_sim::{SimPlugin, TICK_HZ};
    use std::time::Duration;

    /// One tick per update: a floor, a hero at the origin facing −z, and a target `ahead`
    /// in front of him.
    fn app(ahead: f32, hitpoints: f32) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimPlugin, HeroPlugin, CombatPlugin))
            .init_resource::<PlayerInputs>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / TICK_HZ,
            )));
        app.world_mut().spawn((
            Collider::Cuboid {
                half_extents: Vec3::new(50.0, 0.5, 50.0),
            },
            Transform::from_xyz(0.0, -0.5, 0.0),
        ));
        let profile = HeroProfile::uya();
        app.world_mut()
            .spawn(hero_bundle(0, Vec3::ZERO, 0.0, -20.0, &profile));
        let target = app
            .world_mut()
            .spawn((
                target_bundle(0.4, 0.5, hitpoints),
                Pushable,
                SimTransform::from_xyz(0.0, 0.0, -ahead),
            ))
            .id();
        app.update(); // Starts the clock.
        (app, target)
    }

    fn press(app: &mut App, actions: &[Action]) {
        let mut buttons = Buttons::NONE;
        for &action in actions {
            buttons.insert(action);
        }
        app.world_mut().resource_mut::<PlayerInputs>().0[0] = PlayerInput {
            held: buttons,
            pressed: buttons,
            ..default()
        };
        app.update();
        app.world_mut().resource_mut::<PlayerInputs>().0[0] = PlayerInput::default();
    }

    fn ticks(app: &mut App, count: usize) {
        for _ in 0..count {
            app.update();
        }
    }

    fn hitpoints(app: &App, target: Entity) -> f32 {
        app.world().get::<Hitpoints>(target).unwrap().current
    }

    #[test]
    fn a_swing_hits_a_target_in_front_once() {
        let (mut app, target) = app(1.6, 5.0);
        press(&mut app, &[Action::Melee]);
        ticks(&mut app, 40);
        assert_eq!(hitpoints(&app, target), 4.0);
        let reaction = app.world().get::<HitReaction>(target).unwrap();
        assert_eq!(reaction.struck_by.len(), 1);
    }

    #[test]
    fn a_target_out_of_reach_is_not_hit() {
        let (mut app, target) = app(4.0, 5.0);
        press(&mut app, &[Action::Melee]);
        ticks(&mut app, 40);
        assert_eq!(hitpoints(&app, target), 5.0);
    }

    #[test]
    fn the_thrown_wrench_hits_far_away_and_destroys_the_last_hitpoint() {
        let (mut app, target) = app(6.0, 1.0);
        let mut cursor = app.world().resource::<Messages<Destroyed>>().get_cursor();
        press(&mut app, &[Action::Crouch, Action::Melee]);
        let mut events = Vec::new();
        for _ in 0..40 {
            app.update();
            let messages = app.world().resource::<Messages<Destroyed>>();
            events.extend(cursor.read(messages).copied());
        }
        assert_eq!(hitpoints(&app, target), 0.0);
        assert_eq!(
            events,
            [Destroyed {
                target,
                player: 0,
                weapon: None
            }]
        );
    }

    #[test]
    fn the_first_doubler_crate_asks_for_its_help_and_the_next_only_doubles_again() {
        let (mut app, first) = app(1.6, 1.0);
        app.world_mut()
            .entity_mut(first)
            .insert(Crate(CrateKind::Doubler));
        let hero = app
            .world_mut()
            .query_filtered::<Entity, With<Hero>>()
            .single(app.world())
            .unwrap();
        app.world_mut().entity_mut(hero).insert(Bolts::default());
        let mut cursor = app
            .world()
            .resource::<Messages<pickups::HelpAsked>>()
            .get_cursor();
        let mut asked = Vec::new();
        for _ in 0..2 {
            press(&mut app, &[Action::Melee]);
            for _ in 0..40 {
                app.update();
                let messages = app.world().resource::<Messages<pickups::HelpAsked>>();
                asked.extend(cursor.read(messages).copied());
            }
            // The next crate where the first stood.
            app.world_mut().spawn((
                target_bundle(0.4, 0.5, 1.0),
                Crate(CrateKind::Doubler),
                SimTransform::from_xyz(0.0, 0.0, -1.6),
            ));
        }
        assert!(app.world().get_entity(first).is_err());
        assert_eq!(
            asked,
            [pickups::HelpAsked {
                player: 0,
                help: pickups::Help::Doubler
            }]
        );
        let doubled = app
            .world_mut()
            .query::<&pickups::BoltMultiplier>()
            .single(app.world())
            .unwrap();
        assert_eq!(doubled.exp, 2);
    }

    #[test]
    fn a_hit_pushes_a_pushable_target_and_flashes_it() {
        let (mut app, target) = app(1.6, 5.0);
        let start = app.world().get::<SimTransform>(target).unwrap().translation;
        press(&mut app, &[Action::Melee]);
        let mut flashed = false;
        for _ in 0..40 {
            app.update();
            flashed |= app.world().get::<HitReaction>(target).unwrap().flash_ticks > 0;
        }
        let end = app.world().get::<SimTransform>(target).unwrap().translation;
        assert!(flashed);
        assert!(end.distance(start) > 0.1, "{start} -> {end}");
        assert_eq!(
            app.world().get::<HitReaction>(target).unwrap().push,
            Vec3::ZERO
        );
    }
}

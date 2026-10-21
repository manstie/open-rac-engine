//! The guns through the whole plugin against `docs/research/weapons.md`: drawing, each fire
//! mode's timing and what its shots hit, XP from kills, lock-on and the mods. The Shock
//! Blaster's and N60's timings are checked against UYA NTSC traces there; the rest against
//! the code's numbers, on synthetic yards.

use bevy::{ecs::message::MessageCursor, prelude::*, time::TimeUpdateStrategy};
use rac_combat::{
    Armory, Arsenal, Burns, CombatPlugin, Destroyed, GunEvent, GunEventKind, Hitpoints, KillXp,
    WeaponId, target_bundle,
    weapons::{Hand, Mods},
};
use rac_hero::{Collider, GunPose, HeroBody, HeroPlugin, HeroProfile, Hold, LockOn, hero_bundle};
use rac_input::{Action, Buttons, PlayerInput, PlayerInputs};
use rac_sim::{SimPlugin, SimTransform, TICK_HZ};
use std::time::Duration;

const BLASTER: WeaponId = WeaponId(0);
const N60: WeaponId = WeaponId(1);
const NITRO: WeaponId = WeaponId(2);
const LAVA: WeaponId = WeaponId(3);
const ANNIHILATOR: WeaponId = WeaponId(4);
const FLUX: WeaponId = WeaponId(5);

struct Yard {
    app: App,
    hero: Entity,
    events: Vec<GunEvent>,
    destroyed: Vec<Destroyed>,
    event_cursor: MessageCursor<GunEvent>,
    destroyed_cursor: MessageCursor<Destroyed>,
}

impl Yard {
    /// One tick per update: a floor, and the hero at the origin facing −z with `guns` at
    /// their levels, the first in his hand and ready to fire.
    fn new(guns: &[(WeaponId, u8)]) -> Self {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimPlugin, HeroPlugin, CombatPlugin))
            .init_resource::<PlayerInputs>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / TICK_HZ,
            )));
        app.world_mut().spawn((
            Collider::Cuboid {
                half_extents: Vec3::new(200.0, 0.5, 200.0),
            },
            Transform::from_xyz(0.0, -0.5, 0.0),
        ));
        let armory = Armory::uya();
        let mut arsenal = Arsenal::default();
        for &(weapon, level) in guns {
            arsenal.give_at(weapon, level, &armory);
        }
        if let Some(&(weapon, _)) = guns.first() {
            arsenal.hand = Hand::Gun { weapon, ticks: 100 };
        }
        let profile = HeroProfile::uya();
        let hero = app
            .world_mut()
            .spawn((hero_bundle(0, Vec3::ZERO, 0.0, -20.0, &profile), arsenal))
            .id();
        app.update(); // Starts the clock.
        let event_cursor = app.world().resource::<Messages<GunEvent>>().get_cursor();
        let destroyed_cursor = app.world().resource::<Messages<Destroyed>>().get_cursor();
        let mut yard = Self {
            app,
            hero,
            events: Vec::new(),
            destroyed: Vec::new(),
            event_cursor,
            destroyed_cursor,
        };
        // A tick for the hand to reach his moves.
        yard.tick(&[], &[]);
        yard
    }

    /// A target whose middle is `centre`, `height` above where it stands.
    fn target(&mut self, centre: Vec3, hitpoints: f32) -> Entity {
        let height = 1.0;
        self.app
            .world_mut()
            .spawn((
                target_bundle(0.5, height, hitpoints),
                SimTransform::from_translation(centre - Vec3::Y * height),
            ))
            .id()
    }

    fn tick(&mut self, held: &[Action], pressed: &[Action]) {
        self.tick_with_stick([0, 0], held, pressed);
    }

    /// A tick with the move stick at `stick`, in PS2 counts.
    fn tick_with_stick(&mut self, stick: [i8; 2], held: &[Action], pressed: &[Action]) {
        let mut buttons = Buttons::NONE;
        for &action in held.iter().chain(pressed) {
            buttons.insert(action);
        }
        let mut press = Buttons::NONE;
        for &action in pressed {
            press.insert(action);
        }
        self.app.world_mut().resource_mut::<PlayerInputs>().0[0] = PlayerInput {
            move_stick: stick,
            held: buttons,
            pressed: press,
            ..default()
        };
        self.app.update();
        let world = self.app.world();
        self.events.extend(
            self.event_cursor
                .read(world.resource::<Messages<GunEvent>>())
                .copied(),
        );
        self.destroyed.extend(
            self.destroyed_cursor
                .read(world.resource::<Messages<Destroyed>>())
                .copied(),
        );
    }

    fn hold(&mut self, actions: &[Action], ticks: usize) {
        for _ in 0..ticks {
            self.tick(actions, &[]);
        }
    }

    fn idle(&mut self, ticks: usize) {
        self.hold(&[], ticks);
    }

    fn hitpoints(&self, target: Entity) -> f32 {
        self.app.world().get::<Hitpoints>(target).unwrap().current
    }

    fn arsenal(&self) -> &Arsenal {
        self.app.world().get::<Arsenal>(self.hero).unwrap()
    }

    fn body(&self) -> &HeroBody {
        self.app.world().get::<HeroBody>(self.hero).unwrap()
    }

    /// The ticks, counted from the first tick of the `log`, on which a shot was fired.
    fn fired_ticks(log: &[(usize, Vec<GunEvent>)]) -> Vec<usize> {
        log.iter()
            .filter(|(_, events)| {
                events
                    .iter()
                    .any(|event| matches!(event.kind, GunEventKind::Fired { .. }))
            })
            .map(|(tick, _)| *tick)
            .collect()
    }

    /// Holds `actions` for `ticks`, keeping each tick's events apart.
    fn hold_logged(&mut self, actions: &[Action], ticks: usize) -> Vec<(usize, Vec<GunEvent>)> {
        (0..ticks)
            .map(|tick| {
                let before = self.events.len();
                self.tick(actions, &[]);
                (tick, self.events[before..].to_vec())
            })
            .collect()
    }
}

#[test]
fn the_fire_button_brings_the_gun_out_after_17_ticks() {
    let mut yard = Yard::new(&[]);
    let armory = Armory::uya();
    yard.app
        .world_mut()
        .get_mut::<Arsenal>(yard.hero)
        .unwrap()
        .give(N60, &armory);
    yard.tick(&[], &[Action::Fire]);
    for _ in 0..16 {
        assert!(!yard.body().0.gun.out);
        yard.idle(1);
    }
    assert!(!yard.body().0.gun.out);
    yard.idle(1);
    // "Checked in traces": in his hand 17 frames after Circle.
    assert!(yard.body().0.gun.out);
    assert_eq!(
        yard.arsenal().hand,
        Hand::Gun {
            weapon: N60,
            ticks: 0
        }
    );
}

#[test]
fn the_n60_fires_every_14_ticks_while_held_one_round_each() {
    let mut yard = Yard::new(&[(N60, 0)]);
    let log = yard.hold_logged(&[Action::Fire], 43);
    assert_eq!(Yard::fired_ticks(&log), [0, 14, 28, 42]);
    assert_eq!(yard.arsenal().gun(N60).unwrap().ammo, 146);
    // Both flags while it fires, as the trace shows; both clear once it is let go.
    let hand = yard.body().0.gun;
    assert!(hand.out && hand.firing && hand.firing_held && hand.steady);
    yard.idle(1);
    let hand = yard.body().0.gun;
    assert!(hand.out && !hand.firing && !hand.firing_held);
    // Letting go starts the interval again: the next press fires at once.
    let log = yard.hold_logged(&[Action::Fire], 2);
    assert_eq!(Yard::fired_ticks(&log), [0]);
}

#[test]
fn the_hand_names_the_gun_its_hold_and_firing_animations_and_counts_its_shots() {
    // "The hero's side of firing": rows +0x18, +0x24 and +0x2C on Veldin [data].
    let mut yard = Yard::new(&[(N60, 0), (BLASTER, 0)]);
    let hand = yard.body().0.gun;
    assert_eq!(hand.item, Some(0x2F));
    assert_eq!(hand.pose.hold, Hold::OneHand);
    assert_eq!(
        (hand.pose.standing, hand.pose.crouched),
        (Some(51), Some(57))
    );
    let first = hand.shots;
    yard.hold(&[Action::Fire], 29);
    assert_eq!(
        yard.body().0.gun.shots,
        first.wrapping_add(3),
        "three shots, each starting the animation again"
    );

    // Putting it away empties his hand for the rig.
    yard.tick(&[], &[Action::Melee]);
    yard.idle(1);
    let hand = yard.body().0.gun;
    assert_eq!(hand.item, None);
    assert_eq!(hand.pose, GunPose::default());
}

#[test]
fn an_n60_bullet_hits_a_target_ahead() {
    let mut yard = Yard::new(&[(N60, 0)]);
    let target = yard.target(Vec3::new(0.25, 1.0, -10.0), 10.0);
    yard.hold(&[Action::Fire], 1);
    yard.idle(20);
    assert_eq!(yard.hitpoints(target), 8.5);
    assert!(yard.arsenal().shots.is_empty());
}

#[test]
fn from_v2_the_n60s_bullets_bounce_off_walls() {
    for (level, bounced) in [(0, false), (1, true)] {
        let mut yard = Yard::new(&[(N60, level)]);
        yard.app.world_mut().spawn((
            Collider::Cuboid {
                half_extents: Vec3::new(5.0, 3.0, 0.5),
            },
            Transform::from_xyz(0.0, 3.0, -5.5),
        ));
        // Behind him, where only a bullet coming back off the wall goes.
        let target = yard.target(Vec3::new(0.25, 1.0, 4.0), 10.0);
        yard.idle(1);
        yard.hold(&[Action::Fire], 1);
        yard.idle(45);
        assert_eq!(yard.hitpoints(target) < 10.0, bounced, "level {level}");
    }
}

#[test]
fn the_shock_blaster_fires_every_35_ticks_while_held_standing() {
    // "Checked in traces": held, the next shot 35 frames on, the 5-frame hit delay and
    // the 30-frame cooldown.
    let mut yard = Yard::new(&[(BLASTER, 0)]);
    let target = yard.target(Vec3::new(0.25, 1.0, -5.0), 10.0);
    let log = yard.hold_logged(&[Action::Fire], 80);
    assert_eq!(Yard::fired_ticks(&log), [0, 35, 70]);
    // Each landed shot is one hit of 2, whatever number of its rays found the target; the
    // third lands on tick 75.
    assert_eq!(yard.hitpoints(target), 4.0);
    let rays = log[5]
        .1
        .iter()
        .filter(|event| matches!(event.kind, GunEventKind::Ray { .. }))
        .count();
    assert_eq!(rays, 23);
}

#[test]
fn the_shock_blasters_charged_shot_does_three_times_the_damage() {
    let mut yard = Yard::new(&[(BLASTER, 1)]);
    let target = yard.target(Vec3::new(0.25, 1.0, -5.0), 100.0);
    // A shot on the press, then charging; let go after 79 ticks held.
    yard.hold(&[Action::Fire], 79);
    assert_eq!(yard.hitpoints(target), 97.5);
    yard.idle(10);
    assert_eq!(yard.hitpoints(target), 97.5 - 7.5);
    // Let go sooner, nothing more.
    yard.hold(&[Action::Fire], 40);
    yard.idle(10);
    assert_eq!(yard.hitpoints(target), 90.0 - 2.5);
}

#[test]
fn the_nitro_launchers_charge_bursts_and_its_blast_hits_once() {
    let mut yard = Yard::new(&[(NITRO, 0)]);
    let target = yard.target(Vec3::new(0.0, 1.0, -8.0), 50.0);
    yard.hold(&[Action::Fire], 1);
    yard.idle(150);
    assert_eq!(yard.hitpoints(target), 40.0);
    let impacts = yard
        .events
        .iter()
        .filter(|event| matches!(event.kind, GunEventKind::Impact { .. }))
        .count();
    assert_eq!(impacts, 1);
    assert!(yard.arsenal().shots.is_empty());
    // 2 s before the next.
    let log = yard.hold_logged(&[Action::Fire], 121);
    assert_eq!(Yard::fired_ticks(&log), [0, 120]);
}

#[test]
fn the_lava_guns_stream_takes_a_round_every_10_ticks_and_burns_what_it_reaches() {
    let mut yard = Yard::new(&[(LAVA, 0)]);
    let target = yard.target(Vec3::new(0.25, 1.0, -3.0), 200.0);
    yard.hold(&[Action::Fire], 120);
    assert_eq!(yard.arsenal().gun(LAVA).unwrap().ammo, 150 - 12);
    let hand = yard.body().0.gun;
    assert!(hand.firing && hand.firing_held);
    let hurt = 200.0 - yard.hitpoints(target);
    assert!(hurt > 0.0 && hurt % 8.0 == 0.0, "{hurt}");
    // The stream runs on as its strength falls, then stops.
    yard.idle(120);
    assert!(yard.arsenal().shots.is_empty());
}

#[test]
fn acid_from_the_lava_gun_burns_a_target_past_the_queue() {
    // V2's acid [text], with the table's bank 0: 2 a second for 4 s.
    let mut yard = Yard::new(&[(LAVA, 1)]);
    let target = yard.target(Vec3::new(0.25, 1.0, -3.0), 500.0);
    for _ in 0..60 {
        yard.hold(&[Action::Fire], 1);
        if !yard.app.world().resource::<Burns>().0.is_empty() {
            break;
        }
    }
    let burns = &yard.app.world().resource::<Burns>().0;
    assert_eq!(burns.len(), 1);
    assert_eq!(burns[0].target, target);
    assert_eq!(burns[0].per_tick, 2.0 / 60.0);
}

#[test]
fn the_annihilators_missile_turns_after_the_target_it_picked() {
    let mut yard = Yard::new(&[(ANNIHILATOR, 0)]);
    let target = yard.target(Vec3::new(3.0, 1.0, -12.0), 100.0);
    // The pick takes 5 ticks of the target being best.
    yard.idle(6);
    yard.hold(&[Action::Fire], 1);
    // It moves off; the missile follows.
    yard.app
        .world_mut()
        .get_mut::<SimTransform>(target)
        .unwrap()
        .translation
        .x += 2.0;
    yard.idle(40);
    // The missile's 25, which the splash's 6 doesn't add to.
    assert_eq!(yard.hitpoints(target), 75.0);
}

#[test]
fn the_flux_rifles_shot_crosses_its_range_in_a_few_ticks() {
    let mut yard = Yard::new(&[(FLUX, 0)]);
    let near = yard.target(Vec3::new(0.25, 1.0, -60.0), 100.0);
    yard.hold(&[Action::Fire], 1);
    yard.idle(9);
    assert_eq!(yard.hitpoints(near), 85.0);
    // Beyond its 128 u, nothing; and a second a shot.
    let mut yard = Yard::new(&[(FLUX, 0)]);
    let far = yard.target(Vec3::new(0.25, 1.0, -150.0), 100.0);
    let log = yard.hold_logged(&[Action::Fire], 61);
    yard.idle(30);
    assert_eq!(yard.hitpoints(far), 100.0);
    assert_eq!(Yard::fired_ticks(&log), [0, 60]);
}

#[test]
fn a_kill_gives_the_gun_its_xp_and_levels_it_up() {
    let mut yard = Yard::new(&[(N60, 0)]);
    let target = yard.target(Vec3::new(0.25, 1.0, -10.0), 1.5);
    yard.app.world_mut().entity_mut(target).insert(KillXp(250));
    yard.hold(&[Action::Fire], 1);
    yard.idle(20);
    assert_eq!(yard.hitpoints(target), 0.0);
    assert_eq!(
        yard.destroyed,
        [Destroyed {
            target,
            player: 0,
            weapon: Some(N60)
        }]
    );
    let gun = yard.arsenal().gun(N60).unwrap();
    assert_eq!((gun.level, gun.xp, gun.ammo), (1, 250, 175));
    assert!(
        yard.events
            .iter()
            .any(|event| event.kind == GunEventKind::LevelUp { level: 1 })
    );
}

#[test]
fn shock_arcs_from_a_hit_to_another_target_for_a_quarter_of_the_damage() {
    // The N60 at V4: 2.5 a bullet, and an arc of a quarter to the first other target
    // within 8 u.
    let mut yard = Yard::new(&[(N60, 3)]);
    let struck = yard.target(Vec3::new(0.25, 1.0, -8.0), 10.0);
    let near = yard.target(Vec3::new(3.5, 1.0, -9.0), 10.0);
    yard.hold(&[Action::Fire], 1);
    yard.idle(20);
    assert_eq!(yard.hitpoints(struck), 7.5);
    assert_eq!(yard.hitpoints(near), 10.0 - 0.625);
    assert!(
        yard.events
            .iter()
            .any(|event| matches!(event.kind, GunEventKind::Arc { .. }))
    );
}

#[test]
fn lock_on_is_allowed_only_by_the_guns_lock_on_mod() {
    for (level, allowed) in [(0, false), (2, true)] {
        let yard = Yard::new(&[(N60, level)]);
        let lock = yard.app.world().get::<LockOn>(yard.hero).unwrap();
        assert_eq!(lock.allowed, allowed, "level {level}");
    }
    // A mod given on top of the level's counts too, as Deadlocked's do.
    let mut yard = Yard::new(&[(N60, 0)]);
    yard.app
        .world_mut()
        .get_mut::<Arsenal>(yard.hero)
        .unwrap()
        .gun_mut(N60)
        .unwrap()
        .extra_mods = Mods {
        lock_on: true,
        ..default()
    };
    yard.idle(1);
    assert!(yard.app.world().get::<LockOn>(yard.hero).unwrap().allowed);
}

#[test]
fn a_wrench_swing_puts_the_gun_away() {
    let mut yard = Yard::new(&[(N60, 0)]);
    assert!(yard.body().0.gun.out);
    yard.tick(&[], &[Action::Melee]);
    assert!(yard.body().0.state.is_attacking());
    yard.idle(1);
    assert_eq!(yard.arsenal().hand, Hand::Wrench);
    assert!(!yard.body().0.gun.out);
}

#[test]
fn the_quick_select_tapped_switches_guns_and_held_it_doesnt() {
    let mut yard = Yard::new(&[(N60, 0), (BLASTER, 0)]);
    let armory = Armory::uya();
    yard.app
        .world_mut()
        .get_mut::<Arsenal>(yard.hero)
        .unwrap()
        .choose(BLASTER, &armory);
    yard.idle(20);
    assert_eq!(yard.arsenal().hand.gun(), Some(BLASTER));
    yard.tick(&[], &[Action::QuickSelect]);
    yard.hold(&[Action::QuickSelect], 25);
    yard.idle(1);
    assert_eq!(yard.arsenal().equipped, Some(BLASTER));
    yard.tick(&[], &[Action::QuickSelect]);
    yard.hold(&[Action::QuickSelect], 5);
    yard.idle(1);
    assert_eq!(yard.arsenal().equipped, Some(N60));
    assert!(matches!(
        yard.arsenal().hand,
        Hand::Drawing { weapon: N60, .. }
    ));
}

/// The move stick pushed right and up-right, in PS2 counts.
const RIGHT: [i8; 2] = [127, 0];
const UP_RIGHT: [i8; 2] = [127, 127];

impl Yard {
    fn arsenal_mut(&mut self) -> Mut<'_, Arsenal> {
        self.app.world_mut().get_mut::<Arsenal>(self.hero).unwrap()
    }

    /// Presses quick select and holds it until the ring opens.
    fn open_ring(&mut self) {
        self.tick(&[], &[Action::QuickSelect]);
        self.hold(&[Action::QuickSelect], 20);
        assert!(self.arsenal().quick.open.is_some());
    }
}

#[test]
fn guns_given_fill_the_first_quick_select_ring_in_order() {
    let yard = Yard::new(&[(N60, 0), (BLASTER, 0), (NITRO, 0)]);
    let quick = &yard.arsenal().quick;
    assert_eq!(
        quick.rings[0][..4],
        [Some(N60), Some(BLASTER), Some(NITRO), None]
    );
    assert_eq!(quick.rings[1], [None; 8]);
}

#[test]
fn quick_select_held_opens_the_ring_and_the_stick_chooses_a_gun_without_moving_him() {
    let mut yard = Yard::new(&[(N60, 0), (BLASTER, 0), (NITRO, 0), (LAVA, 0)]);
    let start = yard.body().0.position;
    // Not before the quick switch's 20 ticks are up.
    yard.tick(&[], &[Action::QuickSelect]);
    yard.hold(&[Action::QuickSelect], 19);
    assert_eq!(yard.arsenal().quick.open, None);
    assert!(!yard.body().0.gun.selecting);
    yard.hold(&[Action::QuickSelect], 1);
    assert!(yard.arsenal().quick.open.is_some());
    assert!(yard.body().0.gun.selecting);
    // The stick points at slot 2, on the right, and stays chosen as it comes back.
    for _ in 0..20 {
        yard.tick_with_stick(RIGHT, &[Action::QuickSelect], &[]);
    }
    yard.hold(&[Action::QuickSelect], 2);
    assert_eq!(yard.arsenal().quick.highlighted(), Some(NITRO));
    assert_eq!(yard.body().0.position, start);
    assert_eq!(yard.arsenal().equipped, Some(N60));
    // Letting go chooses it: equipped, the most recent, and coming out.
    yard.idle(1);
    let arsenal = yard.arsenal();
    assert_eq!(arsenal.quick.open, None);
    assert_eq!(arsenal.equipped, Some(NITRO));
    assert_eq!(arsenal.recent.first(), Some(&NITRO));
    assert!(matches!(arsenal.hand, Hand::Drawing { weapon: NITRO, .. }));
    assert!(!yard.body().0.gun.selecting);
}

#[test]
fn letting_go_of_the_ring_with_nothing_highlighted_closes_it() {
    let mut yard = Yard::new(&[(N60, 0), (BLASTER, 0)]);
    yard.open_ring();
    yard.hold(&[Action::QuickSelect], 10);
    yard.idle(1);
    let arsenal = yard.arsenal();
    assert_eq!(arsenal.quick.open, None);
    assert_eq!(arsenal.equipped, Some(N60));
    assert_eq!(arsenal.hand.gun(), Some(N60));
}

#[test]
fn the_weapon_buttons_turn_the_open_ring_instead_of_switching_guns() {
    let mut yard = Yard::new(&[(N60, 0), (BLASTER, 0)]);
    yard.open_ring();
    yard.tick(&[Action::QuickSelect], &[Action::NextWeapon]);
    assert_eq!(yard.arsenal().quick.ring, 1);
    assert_eq!(yard.arsenal().equipped, Some(N60));
    // The second ring's slot 1 is empty, so letting go there chooses nothing.
    yard.tick_with_stick(UP_RIGHT, &[Action::QuickSelect], &[]);
    yard.idle(1);
    assert_eq!(yard.arsenal().equipped, Some(N60));
    assert_eq!(yard.arsenal().hand.gun(), Some(N60));
    // It opens on the ring last shown, and turns back.
    yard.open_ring();
    assert_eq!(yard.arsenal().quick.ring, 1);
    yard.tick(&[Action::QuickSelect], &[Action::PrevWeapon]);
    yard.tick_with_stick(UP_RIGHT, &[Action::QuickSelect], &[]);
    yard.idle(1);
    assert_eq!(yard.arsenal().equipped, Some(BLASTER));
}

#[test]
fn a_gun_chosen_in_the_ring_comes_out_from_the_wrench_even_out_of_ammo() {
    let mut yard = Yard::new(&[]);
    let armory = Armory::uya();
    for weapon in [N60, LAVA] {
        yard.arsenal_mut().give(weapon, &armory);
    }
    yard.arsenal_mut().gun_mut(LAVA).unwrap().ammo = 0;
    assert_eq!(yard.arsenal().hand, Hand::Wrench);
    yard.open_ring();
    yard.tick_with_stick(UP_RIGHT, &[Action::QuickSelect], &[]);
    yard.idle(1);
    assert_eq!(yard.arsenal().equipped, Some(LAVA));
    assert!(matches!(
        yard.arsenal().hand,
        Hand::Drawing { weapon: LAVA, .. }
    ));
}

#[test]
fn a_hero_without_guns_opens_no_ring() {
    let mut yard = Yard::new(&[]);
    yard.tick(&[], &[Action::QuickSelect]);
    yard.hold(&[Action::QuickSelect], 30);
    assert_eq!(yard.arsenal().quick.open, None);
    assert!(!yard.body().0.gun.selecting);
}

#[test]
fn the_shock_blaster_held_as_it_comes_out_fires_once_it_can() {
    // At V2 and up its one shot a press comes in the first ticks it can fire, however
    // long the button was held before.
    let mut yard = Yard::new(&[]);
    let armory = Armory::uya();
    yard.app
        .world_mut()
        .get_mut::<Arsenal>(yard.hero)
        .unwrap()
        .give_at(BLASTER, 2, &armory);
    yard.tick(&[], &[Action::Fire]);
    let log = yard.hold_logged(&[Action::Fire], 60);
    // 17 ticks to his hand and 23 more before it may fire.
    assert_eq!(Yard::fired_ticks(&log), [39]);
}

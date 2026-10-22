//! The guns through the whole plugin against `docs/research/weapons.md`: drawing, each fire
//! mode's timing and what its shots hit, XP from kills, lock-on and the mods. The Shock
//! Blaster's and N60's timings are checked against UYA NTSC traces there; the rest against
//! the code's numbers, on synthetic yards.

use bevy::{ecs::message::MessageCursor, prelude::*, time::TimeUpdateStrategy};
use rac_combat::{
    Afflicted, Affliction, Armory, Arsenal, Burns, CombatPlugin, Destroyed, GunEvent, GunEventKind,
    Hitpoints, Hostile, KillXp, WeaponId, target_bundle,
    weapons::{Hand, Mods, ShotKind, block_shot},
};
use rac_hero::{
    CameraSetting, Collider, FollowCamera, GunPose, Health, HeroBody, HeroPlugin, HeroProfile,
    Hold, LockOn, Zoom, hero_bundle,
};
use rac_input::{Action, Buttons, PlayerInput, PlayerInputs};
use rac_sim::{SimPlugin, SimTransform, TICK_HZ};
use std::time::Duration;

const BLASTER: WeaponId = WeaponId(0);
const N60: WeaponId = WeaponId(1);
const NITRO: WeaponId = WeaponId(2);
const LAVA: WeaponId = WeaponId(3);
const ANNIHILATOR: WeaponId = WeaponId(4);
const FLUX: WeaponId = WeaponId(5);
const INFECTOR: WeaponId = WeaponId(6);
const HYDRA: WeaponId = WeaponId(7);
const DISC: WeaponId = WeaponId(8);
const AGENTS: WeaponId = WeaponId(9);
const RIFT: WeaponId = WeaponId(10);
const HOLO: WeaponId = WeaponId(11);
const WHIP: WeaponId = WeaponId(12);
const SUCK: WeaponId = WeaponId(13);
const QWACK: WeaponId = WeaponId(14);
const RYNO: WeaponId = WeaponId(15);
const COIL: WeaponId = WeaponId(16);
const BOUNCER: WeaponId = WeaponId(17);
const TURRET: WeaponId = WeaponId(18);
const SHIELD: WeaponId = WeaponId(19);

struct Yard {
    app: App,
    hero: Entity,
    events: Vec<GunEvent>,
    destroyed: Vec<Destroyed>,
    afflicted: Vec<Afflicted>,
    event_cursor: MessageCursor<GunEvent>,
    destroyed_cursor: MessageCursor<Destroyed>,
    afflicted_cursor: MessageCursor<Afflicted>,
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
        let afflicted_cursor = app.world().resource::<Messages<Afflicted>>().get_cursor();
        let mut yard = Self {
            app,
            hero,
            events: Vec::new(),
            destroyed: Vec::new(),
            afflicted: Vec::new(),
            event_cursor,
            destroyed_cursor,
            afflicted_cursor,
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

    /// An enemy ([`Hostile`]): the guns that seek, infect or take things take only these.
    fn enemy(&mut self, centre: Vec3, hitpoints: f32) -> Entity {
        let target = self.target(centre, hitpoints);
        self.app.world_mut().entity_mut(target).insert(Hostile);
        target
    }

    fn count_shots(&self, is: impl Fn(&ShotKind) -> bool) -> usize {
        self.arsenal()
            .shots
            .iter()
            .filter(|shot| is(&shot.kind))
            .count()
    }

    fn ammo(&self, weapon: WeaponId) -> u32 {
        self.arsenal().gun(weapon).unwrap().ammo
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
        self.afflicted.extend(
            self.afflicted_cursor
                .read(world.resource::<Messages<Afflicted>>())
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

    /// Gives him a camera in `setting`, which his gun's scope zooms, and lets it run a
    /// tick: until it has, his look counts as ready.
    fn camera(&mut self, setting: CameraSetting) -> Entity {
        let camera = self
            .app
            .world_mut()
            .spawn((
                FollowCamera::for_player(0, 0.0).with_setting(setting),
                SimTransform::default(),
            ))
            .id();
        self.idle(2);
        camera
    }

    /// What his gun asked of the camera this tick.
    fn zoom(&self) -> Option<Zoom> {
        self.body().0.gun.zoom
    }

    /// Holds the first-person button until his gun's scope asks for a zoom.
    fn look_through_scope(&mut self) -> Zoom {
        for _ in 0..120 {
            self.tick(&[Action::FirstPerson], &[]);
            if let Some(zoom) = self.zoom() {
                return zoom;
            }
        }
        panic!("the scope never zoomed");
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
fn the_annihilators_blast_leaves_mines_at_v2_to_v4_and_seekers_from_v5() {
    // "Annihilator": 4 mines at V2, 6 at V3 and V4 (heavy), none from V5, which sends the
    // Decimator's seven seekers instead.
    for (level, mines, seekers) in [(0, 0, 0), (1, 4, 0), (2, 6, 0), (3, 6, 0), (4, 0, 7)] {
        let mut yard = Yard::new(&[(ANNIHILATOR, level)]);
        // A wall of a target far enough off that the mines don't reach it at once.
        yard.target(Vec3::new(0.0, 1.0, -12.0), 10_000.0);
        yard.idle(6);
        yard.hold(&[Action::Fire], 1);
        let mut most = (0, 0);
        for _ in 0..40 {
            yard.idle(1);
            let bomblets = yard.count_shots(|kind| matches!(kind, ShotKind::Bomblet { .. }));
            let seeking = yard.count_shots(|kind| matches!(kind, ShotKind::Seeker { .. }));
            most = (most.0.max(bomblets), most.1.max(seeking));
        }
        assert_eq!(most, (mines, seekers), "V{}", level + 1);
    }
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
fn the_flux_scope_starts_a_quarter_in_zooms_by_r1_and_r2_and_springs_back_out() {
    // "Aim indicators", the zoom: from the view's 63.6° to 20° at V1, 0.01 rad a frame.
    let mut yard = Yard::new(&[(FLUX, 0)]);
    let camera = yard.camera(CameraSetting::ThirdPerson);
    let (wide, narrow) = (1.1100293, 20f32.to_radians());
    let start = wide + (narrow - wide) * 0.25;
    let zoom = yard.look_through_scope();
    assert!((zoom.fov - start).abs() < 1e-5, "{}", zoom.fov);
    assert_eq!(zoom.spring.gain, 0.02);
    // R1 (crouch) zooms in, R2 (strafe) out.
    yard.hold(&[Action::FirstPerson, Action::Crouch], 10);
    assert!((yard.zoom().unwrap().fov - (start - 0.1)).abs() < 1e-4);
    yard.hold(&[Action::FirstPerson, Action::Crouch], 100);
    assert_eq!(yard.zoom().unwrap().fov, narrow);
    yard.hold(&[Action::FirstPerson, Action::Strafe], 5);
    assert!((yard.zoom().unwrap().fov - (narrow + 0.05)).abs() < 1e-4);
    let follow = yard.app.world().get::<FollowCamera>(camera).unwrap();
    assert!(follow.zoom() < 0.5, "{}", follow.zoom());
    // Out of the look (which his moves leave after the guns' tick), the camera is asked
    // back out once, slowly.
    yard.idle(2);
    let back = yard.zoom().unwrap();
    assert_eq!((back.fov, back.spring.gain), (wide, 0.005));
    yard.idle(1);
    assert_eq!(yard.zoom(), None);
    // The next look starts a quarter in again.
    let zoom = yard.look_through_scope();
    assert!((zoom.fov - start).abs() < 1e-5);
}

#[test]
fn the_flux_scopes_ring_needs_a_live_enemy_first_on_the_eyes_ray() {
    // In first person the left stick zooms, here at V3 down to 10°.
    let mut yard = Yard::new(&[(FLUX, 2)]);
    let camera = yard.camera(CameraSetting::FirstPerson);
    yard.look_through_scope();
    for _ in 0..200 {
        yard.tick_with_stick([0, 127], &[Action::FirstPerson], &[]);
    }
    assert_eq!(yard.zoom().unwrap().fov, 10f32.to_radians());
    let place = *yard.app.world().get::<SimTransform>(camera).unwrap();
    let ahead = |distance: f32| place.translation + place.rotation * Vec3::NEG_Z * distance;
    assert!(!yard.arsenal().sights.scope_on);
    let enemy = yard.enemy(ahead(60.0), 100.0);
    yard.hold(&[Action::FirstPerson], 2);
    assert!(yard.arsenal().sights.scope_on);
    // A crate in front of it takes the ray first.
    let crate_ahead = yard.target(ahead(30.0), 100.0);
    yard.hold(&[Action::FirstPerson], 2);
    assert!(!yard.arsenal().sights.scope_on);
    yard.app.world_mut().despawn(crate_ahead);
    yard.hold(&[Action::FirstPerson], 2);
    assert!(yard.arsenal().sights.scope_on);
    // So does a wall.
    yard.app.world_mut().spawn((
        Collider::Cuboid {
            half_extents: Vec3::new(5.0, 5.0, 0.5),
        },
        Transform::from_translation(ahead(20.0)).with_rotation(place.rotation),
    ));
    yard.hold(&[Action::FirstPerson], 2);
    assert!(!yard.arsenal().sights.scope_on);
    // Past its 128 u, nothing.
    yard.app.world_mut().despawn(enemy);
    yard.enemy(ahead(130.0), 100.0);
    yard.hold(&[Action::FirstPerson], 2);
    assert!(!yard.arsenal().sights.scope_on);
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
fn shock_arcs_from_a_hit_to_another_enemy_for_a_quarter_of_the_damage_10_ticks_on() {
    // The N60 at V4: 2.5 a bullet, and an arc of a quarter to the first other enemy in
    // sight within 8 u, which reaches it 10 ticks after the hit (`shock_arc_add`).
    let mut yard = Yard::new(&[(N60, 3)]);
    let struck = yard.enemy(Vec3::new(0.25, 1.0, -8.0), 10.0);
    let near = yard.enemy(Vec3::new(3.5, 1.0, -9.0), 10.0);
    // Not a dummy that isn't an enemy, though it is nearer.
    let dummy = yard.target(Vec3::new(1.5, 1.0, -8.5), 10.0);
    yard.hold(&[Action::Fire], 1);
    let mut arc_tick = None;
    let mut struck_tick = None;
    for tick in 0..30 {
        yard.idle(1);
        if arc_tick.is_none()
            && yard
                .events
                .iter()
                .any(|event| matches!(event.kind, GunEventKind::Arc { .. }))
        {
            arc_tick = Some(tick);
        }
        if struck_tick.is_none() && yard.hitpoints(near) < 10.0 {
            struck_tick = Some(tick);
        }
    }
    assert_eq!(yard.hitpoints(struck), 7.5);
    assert_eq!(yard.hitpoints(near), 10.0 - 0.625);
    assert_eq!(yard.hitpoints(dummy), 10.0);
    let (arc, hit) = (arc_tick.unwrap(), struck_tick.unwrap());
    assert_eq!(hit - arc, 10, "the arc reaches its target 10 ticks on");
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

    /// Presses quick select and holds it, the stick centred, past the hold check and the
    /// stick's lock-out, so the ring is ready to pick.
    fn open_ring(&mut self) {
        self.tick(&[], &[Action::QuickSelect]);
        self.hold(&[Action::QuickSelect], 6);
        assert!(self.arsenal().quick.open.is_some());
    }

    /// Lets go of quick select and waits out the ring's closing.
    fn close_ring(&mut self) {
        self.idle(8);
        assert_eq!(self.arsenal().quick.open, None);
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
fn the_ring_opens_on_the_press_and_holds_his_input_after_15_ticks() {
    // "The quick-select ring", with QUICK SELECT PAUSE off: the world runs on, and the
    // hero takes no input once 15 ticks have passed.
    let mut yard = Yard::new(&[(N60, 0), (BLASTER, 0), (NITRO, 0), (LAVA, 0)]);
    let start = yard.body().0.position;
    yard.tick_with_stick(RIGHT, &[], &[Action::QuickSelect]);
    assert!(yard.arsenal().quick.open.is_some());
    for _ in 0..13 {
        yard.tick_with_stick(RIGHT, &[Action::QuickSelect], &[]);
        assert!(!yard.body().0.gun.selecting);
    }
    assert_ne!(yard.body().0.position, start, "the stick still walks him");
    yard.tick_with_stick(RIGHT, &[Action::QuickSelect], &[]);
    assert!(yard.body().0.gun.selecting);
    // Pushed since it opened, the stick picks nothing until its 30-tick lock-out ends.
    assert_eq!(yard.arsenal().quick.highlighted(), None);
    for _ in 0..20 {
        yard.tick_with_stick(RIGHT, &[Action::QuickSelect], &[]);
    }
    assert_eq!(yard.arsenal().quick.highlighted(), Some(NITRO));
    let stopped = yard.body().0.position;
    yard.hold(&[Action::QuickSelect], 2);
    assert_eq!(yard.body().0.position, stopped);
    // Letting go closes the ring over 8 ticks, and then chooses: equipped, the most
    // recent, and coming out.
    yard.idle(7);
    assert_eq!(yard.arsenal().equipped, Some(N60));
    assert!(yard.body().0.gun.selecting);
    yard.idle(1);
    let arsenal = yard.arsenal();
    assert_eq!(arsenal.quick.open, None);
    assert_eq!(arsenal.equipped, Some(NITRO));
    assert_eq!(arsenal.recent.first(), Some(&NITRO));
    assert!(matches!(arsenal.hand, Hand::Drawing { weapon: NITRO, .. }));
    assert!(!yard.body().0.gun.selecting);
}

#[test]
fn the_ring_holds_his_input_later_just_after_a_tap_that_switched_nothing() {
    // One gun: the tap has nothing to switch to, so the ring opened soon after waits 27
    // ticks (`quick_switch_recent`).
    let mut yard = Yard::new(&[(N60, 0)]);
    yard.tick(&[], &[Action::QuickSelect]);
    yard.idle(1);
    assert_eq!(yard.arsenal().hand.gun(), Some(N60));
    yard.idle(8);
    yard.tick(&[], &[Action::QuickSelect]);
    yard.hold(&[Action::QuickSelect], 25);
    assert!(!yard.body().0.gun.selecting);
    yard.hold(&[Action::QuickSelect], 1);
    assert!(yard.body().0.gun.selecting);
}

#[test]
fn letting_go_of_the_ring_with_nothing_highlighted_closes_it() {
    let mut yard = Yard::new(&[(N60, 0), (BLASTER, 0)]);
    yard.open_ring();
    yard.hold(&[Action::QuickSelect], 10);
    yard.close_ring();
    let arsenal = yard.arsenal();
    assert_eq!(arsenal.equipped, Some(N60));
    assert_eq!(arsenal.hand.gun(), Some(N60));
}

#[test]
fn the_weapon_buttons_step_round_the_open_ring_and_crouch_shows_the_second() {
    let mut yard = Yard::new(&[(N60, 0), (BLASTER, 0), (NITRO, 0)]);
    yard.open_ring();
    // With nothing picked, right steps to slot 2, and left from there to slot 6.
    yard.tick(&[Action::QuickSelect], &[Action::NextWeapon]);
    assert_eq!(yard.arsenal().quick.highlighted(), Some(NITRO));
    assert_eq!(yard.arsenal().equipped, Some(N60), "the ring, not a switch");
    yard.tick(&[Action::QuickSelect], &[Action::PrevWeapon]);
    assert_eq!(yard.arsenal().quick.open.unwrap().highlighted, Some(6));
    // Crouch held shows the second ring, whose slot 6 is empty too.
    yard.hold(&[Action::QuickSelect, Action::Crouch], 1);
    assert_eq!(yard.arsenal().quick.ring, 1);
    yard.close_ring();
    assert_eq!(yard.arsenal().equipped, Some(N60));
    // It opens on the first ring.
    yard.open_ring();
    assert_eq!(yard.arsenal().quick.ring, 0);
    yard.tick_with_stick(UP_RIGHT, &[Action::QuickSelect], &[]);
    // Slot 1 holds the Shock Blaster.
    assert_eq!(yard.arsenal().quick.highlighted(), Some(BLASTER));
    yard.close_ring();
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
    yard.close_ring();
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

#[test]
fn every_gun_fires_and_its_shots_run_out_at_v1_and_v5() {
    // The Suck Cannon fires only what it holds and the Qwack-O-Ray a beam: the rest say
    // they fired, the Plasma Whip as his moves swing it.
    for index in 6..20 {
        let weapon = WeaponId(index);
        for level in [0, 4] {
            let mut yard = Yard::new(&[(weapon, level)]);
            yard.enemy(Vec3::new(0.0, 1.0, -8.0), 1000.0);
            yard.hold(&[Action::Fire], 1);
            yard.tick(&[Action::Fire], &[Action::Fire]);
            yard.hold(&[Action::Fire], 200);
            yard.idle(2400);
            let fired = yard
                .events
                .iter()
                .any(|event| matches!(event.kind, GunEventKind::Fired { .. }));
            let quiet = [SUCK, QWACK].contains(&weapon);
            assert_eq!(fired, !quiet, "{weapon:?} at level {level}");
            assert!(
                yard.arsenal().shots.is_empty(),
                "{weapon:?} {:?}",
                yard.arsenal().shots
            );
        }
    }
}

#[test]
fn a_coil_ball_sticks_to_the_enemy_it_meets_and_arcs_to_three_others_near_it() {
    let mut yard = Yard::new(&[(COIL, 0)]);
    let stuck = yard.enemy(Vec3::new(0.0, 1.0, -6.0), 1000.0);
    let others: Vec<Entity> = [(3.0, -6.0), (-3.0, -6.0), (0.0, -10.0), (0.0, -17.0)]
        .into_iter()
        .map(|(x, z)| yard.enemy(Vec3::new(x, 1.0, z), 1000.0))
        .collect();
    yard.hold(&[Action::Fire], 1);
    // The coil spins up for 10 ticks before the ball leaves.
    assert_eq!(
        yard.count_shots(|kind| matches!(kind, ShotKind::Ball { .. })),
        0
    );
    yard.idle(10);
    assert_eq!(
        yard.count_shots(|kind| matches!(kind, ShotKind::Ball { .. })),
        1
    );
    yard.idle(100);
    assert_eq!(yard.hitpoints(stuck), 880.0);
    let arced: Vec<f32> = others.iter().map(|&other| yard.hitpoints(other)).collect();
    // The three within 10 u take the second damage (120); the one 11 u off doesn't.
    assert_eq!(arced, [880.0, 880.0, 880.0, 1000.0]);
}

#[test]
fn the_bouncers_canister_bursts_into_17_bomblets_that_find_the_enemy() {
    let mut yard = Yard::new(&[(BOUNCER, 0)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -10.0), 1000.0);
    yard.hold(&[Action::Fire], 1);
    assert_eq!(yard.ammo(BOUNCER), 9);
    let mut most = 0;
    for _ in 0..400 {
        yard.idle(1);
        most = most.max(yard.count_shots(|kind| matches!(kind, ShotKind::Bomblet { .. })));
    }
    assert_eq!(most, 17);
    assert!(yard.hitpoints(enemy) <= 970.0, "{}", yard.hitpoints(enemy));
}

#[test]
fn the_infectors_blob_infects_a_weak_enemy_it_touches_and_bursts_on_a_strong_one() {
    let mut yard = Yard::new(&[(INFECTOR, 0)]);
    let weak = yard.enemy(Vec3::new(0.0, 1.0, -3.0), 2.0);
    let strong = yard.enemy(Vec3::new(0.0, 1.0, -6.0), 50.0);
    for enemy in [weak, strong] {
        yard.app.world_mut().entity_mut(enemy).insert(KillXp(30));
    }
    yard.tick(&[Action::Fire], &[Action::Fire]);
    yard.idle(120);
    let infected: Vec<(Entity, Affliction)> = yard
        .afflicted
        .iter()
        .map(|afflicted| (afflicted.target, afflicted.affliction))
        .collect();
    // The gun's 9 damage over each one's hit points, of 600 ticks.
    assert_eq!(
        infected,
        [
            (weak, Affliction::Infected { ticks: 2700 }),
            (strong, Affliction::Infected { ticks: 108 }),
        ]
    );
    // The burst's blast does 1.
    assert_eq!(yard.hitpoints(strong), 49.0);
    // Infections give the gun XP as kills do.
    assert_eq!(yard.arsenal().gun(INFECTOR).unwrap().xp, 60);
}

#[test]
fn the_disc_blade_gun_fires_every_30_ticks_and_its_disc_finds_the_enemy() {
    let mut yard = Yard::new(&[(DISC, 0)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -10.0), 1000.0);
    let log = yard.hold_logged(&[Action::Fire], 61);
    assert_eq!(Yard::fired_ticks(&log), [0, 30, 60]);
    yard.idle(120);
    assert_eq!(yard.hitpoints(enemy), 925.0);
}

#[test]
fn the_ry3no_fires_eight_missiles_a_round_each_at_a_marked_enemy() {
    let mut yard = Yard::new(&[(RYNO, 0)]);
    let enemies: Vec<Entity> = [-4.0, 0.0, 4.0]
        .into_iter()
        .map(|x| yard.enemy(Vec3::new(x, 1.0, -20.0), 10_000.0))
        .collect();
    yard.hold(&[Action::Fire], 1);
    assert_eq!(yard.ammo(RYNO), 24);
    assert_eq!(
        yard.count_shots(|kind| matches!(kind, ShotKind::Rocket(_))),
        8
    );
    yard.idle(120);
    for enemy in enemies {
        assert!(yard.hitpoints(enemy) < 10_000.0);
    }
}

#[test]
fn the_sights_name_the_guns_target_and_marks_for_a_hud() {
    let mut yard = Yard::new(&[(N60, 0)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -8.0), 100.0);
    yard.idle(2);
    let sights = &yard.arsenal().sights;
    assert_eq!(sights.target, Some(enemy));
    assert!(sights.ready);
    // The Annihilator's mark is its held pick, which takes 5 ticks of being best.
    let mut yard = Yard::new(&[(ANNIHILATOR, 0)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -8.0), 100.0);
    yard.idle(2);
    assert_eq!(yard.arsenal().sights.target, None);
    yard.idle(5);
    assert_eq!(yard.arsenal().sights.target, Some(enemy));
    // The Hydra's marks, newest last.
    let mut yard = Yard::new(&[(HYDRA, 0)]);
    let enemies: Vec<Entity> = [-2.0, 2.0]
        .into_iter()
        .map(|x| yard.enemy(Vec3::new(x, 1.0, -15.0), 1000.0))
        .collect();
    yard.hold(&[Action::Fire], 30);
    let marks = &yard.arsenal().sights.marks;
    assert_eq!(marks.len(), 2);
    assert!(marks.iter().all(|mark| enemies.contains(mark)));
}

#[test]
fn the_hydra_marks_while_held_and_strikes_each_mark_without_a_round() {
    let mut yard = Yard::new(&[(HYDRA, 0)]);
    let enemies: Vec<Entity> = [-2.0, 0.0, 2.0, 4.0]
        .into_iter()
        .map(|x| yard.enemy(Vec3::new(x, 1.0, -15.0), 1000.0))
        .collect();
    // Marks at 12, 24 and 36 ticks held: V1's three.
    yard.hold(&[Action::Fire], 40);
    yard.idle(1);
    let struck = enemies
        .iter()
        .filter(|&&enemy| yard.hitpoints(enemy) == 990.0)
        .count();
    assert_eq!(struck, 3);
    assert_eq!(yard.ammo(HYDRA), 15);
    // Each one's bolt strikes it again for half as it goes.
    yard.idle(15);
    let struck = enemies
        .iter()
        .filter(|&&enemy| yard.hitpoints(enemy) == 985.0)
        .count();
    assert_eq!(struck, 3);
    // A tap fizzles; held long enough with nothing marked, it spits, taking a round.
    yard.hold(&[Action::Fire], 10);
    yard.idle(1);
    assert_eq!(yard.ammo(HYDRA), 15);
    let mut empty = Yard::new(&[(HYDRA, 0)]);
    empty.hold(&[Action::Fire], 25);
    empty.idle(1);
    assert_eq!(empty.ammo(HYDRA), 14);
    assert_eq!(
        empty.count_shots(|kind| matches!(kind, ShotKind::Spit { ends, .. } if ends.len() == 3)),
        1
    );
}

#[test]
fn the_hydras_spit_strikes_what_its_arcs_meet_three_ticks_in() {
    let mut yard = Yard::new(&[(HYDRA, 0)]);
    // Not an enemy, so never marked, but in the arcs' way.
    let enemy = yard.target(Vec3::new(0.0, 1.0, -15.0), 1000.0);
    yard.hold(&[Action::Fire], 25);
    // V1's 30-tick spit strikes 27 ticks before its end.
    yard.idle(2);
    assert_eq!(yard.hitpoints(enemy), 1000.0);
    yard.idle(2);
    assert_eq!(yard.hitpoints(enemy), 990.0);
}

#[test]
fn a_rift_swallows_an_enemy_no_stronger_than_the_guns_damage_and_leaves_a_strong_one() {
    let mut yard = Yard::new(&[(RIFT, 0)]);
    let strong = yard.enemy(Vec3::new(1.5, 1.0, -10.0), 200.0);
    let weak = yard.enemy(Vec3::new(-1.5, 1.0, -10.0), 50.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    yard.idle(120);
    assert_eq!(yard.hitpoints(weak), 0.0);
    assert_eq!(yard.hitpoints(strong), 200.0);
    assert!(
        yard.afflicted
            .iter()
            .any(|afflicted| afflicted.target == weak
                && matches!(afflicted.affliction, Affliction::Swallowed { .. }))
    );
    assert_eq!(yard.destroyed.len(), 1);
}

#[test]
fn the_suck_cannon_takes_in_a_small_enemy_and_fires_it_back_out() {
    let mut yard = Yard::new(&[(SUCK, 0)]);
    let small = yard.enemy(Vec3::new(0.0, 1.0, -2.5), 4.0);
    let big = yard.enemy(Vec3::new(0.0, 1.0, -12.0), 100.0);
    yard.hold(&[Action::Fire], 5);
    assert_eq!(yard.arsenal().sucked, 1);
    assert_eq!(yard.hitpoints(small), 0.0);
    // Let go, a press fires it.
    yard.idle(2);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    assert_eq!(yard.arsenal().sucked, 0);
    yard.idle(30);
    assert_eq!(yard.hitpoints(big), 90.0);
}

#[test]
fn the_qwack_o_ray_turns_an_enemy_into_a_duck_once_its_hit_points_are_spent() {
    let mut yard = Yard::new(&[(QWACK, 0)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -6.0), 10.0);
    // 50 a second: 10 hit points in 12 ticks.
    yard.hold(&[Action::Fire], 11);
    assert!(yard.afflicted.is_empty());
    yard.hold(&[Action::Fire], 1);
    assert_eq!(yard.afflicted.len(), 1);
    assert_eq!(yard.afflicted[0].target, enemy);
    assert_eq!(yard.afflicted[0].affliction, Affliction::Duck);
    // The enemy is gone, and the gun's duck stands there until it goes in a puff.
    assert_eq!(yard.hitpoints(enemy), 0.0);
    assert!(yard.destroyed.iter().any(|kill| kill.target == enemy));
    let ducks = |yard: &Yard| yard.count_shots(|kind| matches!(kind, ShotKind::Duck(_)));
    assert_eq!(ducks(&yard), 1);
    yard.idle(1390);
    assert_eq!(ducks(&yard), 1);
    // A V1 duck lays no eggs.
    assert_eq!(
        yard.count_shots(|kind| matches!(kind, ShotKind::Egg { .. })),
        0
    );
    yard.idle(20);
    assert_eq!(ducks(&yard), 0);
}

#[test]
fn a_v2_duck_lays_eggs_that_go_off_on_an_enemy_for_the_guns_damage() {
    let mut yard = Yard::new(&[(QWACK, 1)]);
    yard.enemy(Vec3::new(0.0, 1.0, -6.0), 10.0);
    yard.hold(&[Action::Fire], 12);
    yard.idle(125);
    let egg = yard
        .arsenal()
        .shots
        .iter()
        .find(|shot| matches!(shot.kind, ShotKind::Egg { .. }))
        .map(|shot| shot.position)
        .expect("an egg within its first 120 ticks");
    // An enemy by the egg sets it off: 75, the V2 damage.
    let near = yard.enemy(egg + Vec3::X, 1000.0);
    yard.idle(1);
    assert_eq!(yard.hitpoints(near), 925.0);
}

#[test]
fn a_v4_duck_runs_at_an_enemy_and_blows_up_there() {
    let mut yard = Yard::new(&[(QWACK, 3)]);
    yard.enemy(Vec3::new(0.0, 1.0, -6.0), 10.0);
    yard.hold(&[Action::Fire], 12);
    let far = yard.enemy(Vec3::new(0.0, 1.0, -16.0), 1000.0);
    yard.idle(1200);
    assert_eq!(
        yard.count_shots(|kind| matches!(kind, ShotKind::Duck(_))),
        0
    );
    assert!(yard.hitpoints(far) < 1000.0);
}

#[test]
fn the_qwack_o_blitzers_duck_flies_at_an_enemy_pecks_it_and_blows_up() {
    let mut yard = Yard::new(&[(QWACK, 4)]);
    yard.enemy(Vec3::new(0.0, 1.0, -6.0), 10.0);
    yard.hold(&[Action::Fire], 12);
    let far = yard.enemy(Vec3::new(0.0, 1.0, -12.0), 1000.0);
    // It waits a second before it starts.
    yard.idle(55);
    assert_eq!(yard.hitpoints(far), 1000.0);
    yard.idle(300);
    assert_eq!(
        yard.count_shots(|kind| matches!(kind, ShotKind::Duck(_))),
        0
    );
    // Its pecks and its end each take a fifth of the V5's 150.
    let taken = 1000.0 - yard.hitpoints(far);
    assert!(taken >= 30.0 && taken % 30.0 == 0.0, "took {taken}");
}

#[test]
fn a_miniturret_lands_and_shoots_the_enemy_near_it() {
    let mut yard = Yard::new(&[(TURRET, 0)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -16.0), 1000.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    assert_eq!(yard.ammo(TURRET), 9);
    // It leaves his hand 20 ticks after the press.
    yard.idle(19);
    assert_eq!(
        yard.count_shots(|kind| matches!(kind, ShotKind::Turret(_))),
        0
    );
    yard.idle(1);
    assert_eq!(
        yard.count_shots(|kind| matches!(kind, ShotKind::Turret(_))),
        1
    );
    yard.idle(240);
    assert!(yard.hitpoints(enemy) < 1000.0);
}

#[test]
fn a_holo_shield_stands_where_it_lands_and_takes_shots_crossing_it() {
    let mut yard = Yard::new(&[(HOLO, 0)]);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    yard.idle(60);
    let armory = Armory::uya();
    let at = yard.arsenal().shots[0].position;
    assert!(at.y.abs() < 0.05 && at.z < -1.0, "{at}");
    let mut arsenal = yard.arsenal_mut();
    // A shot coming at him through it is stopped halfway along its step.
    let from = at + Vec3::new(0.0, 1.0, -0.5);
    let stopped = block_shot(&mut arsenal, &armory, from, Vec3::new(0.0, 0.0, 1.0));
    assert!(
        stopped.is_some_and(|t| (t - 0.5).abs() < 1e-4),
        "{stopped:?}"
    );
    let ShotKind::Barrier(state) = arsenal.shots[0].kind else {
        unreachable!("the shield");
    };
    assert_eq!(state.hits_left, 6);
    // One passing beside it isn't.
    let beside = from + Vec3::X * 3.0;
    assert_eq!(block_shot(&mut arsenal, &armory, beside, Vec3::Z), None);
}

#[test]
fn a_v2_holo_shield_drains_a_near_enemy_to_heal_him_a_point_for_a_hit() {
    let mut yard = Yard::new(&[(HOLO, 1)]);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    yard.idle(40);
    let at = yard.arsenal().shots[0].position;
    assert!(at.length() < 5.0, "{at}");
    let enemy = yard.enemy(at + Vec3::new(0.0, 1.0, -3.0), 100.0);
    let hits = |yard: &Yard| match yard.arsenal().shots[0].kind {
        ShotKind::Barrier(state) => state.hits_left,
        _ => unreachable!("the shield"),
    };
    let before = hits(&yard);
    // Not hurt, it doesn't drain.
    yard.idle(50);
    assert_eq!(hits(&yard), before);
    let health = |yard: &Yard| *yard.app.world().get::<Health>(yard.hero).unwrap();
    let max = health(&yard).max;
    let hero = yard.hero;
    yard.app
        .world_mut()
        .get_mut::<Health>(hero)
        .unwrap()
        .current = max - 6;
    yard.idle(45);
    assert_eq!(health(&yard).current, max - 5);
    assert_eq!(hits(&yard), before - 1);
    // The enemy near it takes a point every 180 ticks he is hurt.
    yard.idle(134);
    assert_eq!(yard.hitpoints(enemy), 100.0);
    yard.idle(1);
    assert_eq!(yard.hitpoints(enemy), 99.0);
}

#[test]
fn from_v2_agents_end_in_comets_and_from_v5_in_growing_blasts() {
    for (level, comet) in [(1, true), (4, false)] {
        let mut yard = Yard::new(&[(AGENTS, level)]);
        let enemy = yard.enemy(Vec3::new(0.0, 1.0, -9.0), 10_000.0);
        yard.tick(&[Action::Fire], &[Action::Fire]);
        let mut seen = false;
        for _ in 0..400 {
            yard.idle(1);
            seen |= yard.count_shots(|kind| match kind {
                ShotKind::Comet { .. } => comet,
                ShotKind::Blast { .. } => !comet,
                _ => false,
            }) > 0;
        }
        assert!(seen, "V{}", level + 1);
        assert!(yard.hitpoints(enemy) < 10_000.0, "V{}", level + 1);
    }
}

#[test]
fn the_agents_pod_lets_out_four_agents_that_run_at_the_enemy_and_blow_up() {
    let mut yard = Yard::new(&[(AGENTS, 0)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -9.0), 1000.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    // The round goes as the pod leaves his hand.
    assert_eq!(yard.ammo(AGENTS), 6);
    yard.idle(20);
    assert_eq!(yard.ammo(AGENTS), 5);
    yard.idle(30);
    assert_eq!(
        yard.count_shots(|kind| matches!(kind, ShotKind::Agent(_))),
        4
    );
    yard.idle(300);
    assert!(yard.hitpoints(enemy) < 1000.0);
}

#[test]
fn from_v2_agents_shoot_small_bullets_at_an_enemy_before_they_reach_it() {
    // "Agents of Doom": 0.03 of the damage a bullet, at an enemy within 25 u, first 27
    // ticks after an agent comes out.
    let mut yard = Yard::new(&[(AGENTS, 1)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -22.0), 1000.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    let damage = Armory::uya().weapons[AGENTS.0].level(1).damage;
    let mut slugs = 0;
    let mut first_hit = None;
    for _ in 0..150 {
        let before = yard.hitpoints(enemy);
        yard.idle(1);
        slugs = slugs.max(yard.count_shots(|kind| *kind == ShotKind::Slug));
        let lost = before - yard.hitpoints(enemy);
        if lost > 0.0 && first_hit.is_none() {
            first_hit = Some(lost);
        }
    }
    assert!(slugs > 0);
    let first_hit = first_hit.expect("a bullet reached the enemy");
    assert!((first_hit - damage * 0.03).abs() < 1e-4, "{first_hit}");
}

#[test]
fn agents_leave_an_enemy_well_above_level_alone() {
    // Their search takes only targets within 5° of level, feet to feet.
    let mut yard = Yard::new(&[(AGENTS, 0)]);
    let enemy = yard.enemy(Vec3::new(0.0, 5.0, -10.0), 1000.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    yard.idle(400);
    assert_eq!(yard.hitpoints(enemy), 1000.0);
    assert_eq!(
        yard.count_shots(|kind| matches!(kind, ShotKind::Agent(_))),
        4
    );
}

#[test]
fn the_nitro_eruptors_burst_fires_15_small_bullets_one_at_a_target_farther_off() {
    let mut yard = Yard::new(&[(NITRO, 4)]);
    yard.target(Vec3::new(0.0, 1.0, -8.0), 10_000.0);
    // Within 13 u of the burst but farther than 8 u: one bullet goes straight at it.
    let far = Vec3::new(0.0, 1.0, -18.0);
    yard.target(far, 10_000.0);
    yard.idle(6);
    yard.hold(&[Action::Fire], 1);
    for _ in 0..120 {
        yard.idle(1);
        let slugs: Vec<_> = yard
            .arsenal()
            .shots
            .iter()
            .filter(|shot| shot.kind == ShotKind::Slug)
            .cloned()
            .collect();
        if slugs.is_empty() {
            continue;
        }
        assert_eq!(slugs.len(), 15);
        assert!(slugs.iter().all(|slug| slug.damage == 10.0));
        let at_far = slugs
            .iter()
            .filter(|slug| {
                slug.velocity
                    .normalize()
                    .angle_between((far - slug.position).normalize())
                    < 0.02
            })
            .count();
        assert_eq!(at_far, 1);
        return;
    }
    panic!("the charge never burst");
}

#[test]
fn the_shield_charger_raises_a_shield_that_takes_hits_and_zaps_what_touches_it() {
    let mut yard = Yard::new(&[(SHIELD, 0)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -1.0), 100.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    assert_eq!(yard.body().0.shield.points, 225.0);
    assert_eq!(yard.ammo(SHIELD), 2);
    // A second press with it up does nothing.
    yard.tick(&[Action::Fire], &[Action::Fire]);
    assert_eq!(yard.ammo(SHIELD), 2);
    // At once and every half second, 3 damage to the enemy touching it.
    yard.idle(60);
    assert_eq!(yard.hitpoints(enemy), 91.0);
}

#[test]
fn the_whip_swings_on_the_fire_button_for_a_round_and_reaches_past_the_wrench() {
    let mut yard = Yard::new(&[(WHIP, 0)]);
    let far = yard.target(Vec3::new(0.0, 1.0, -5.0), 100.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    yard.idle(40);
    assert_eq!(yard.ammo(WHIP), 24);
    assert_eq!(yard.hitpoints(far), 98.0);
    // It stays in his hand through the swing.
    assert_eq!(yard.arsenal().hand.gun(), Some(WHIP));
}

#[test]
fn the_quantum_whips_swings_send_cutting_waves_that_strike_once_far_past_the_rope() {
    for (level, cuts) in [(3, false), (4, true)] {
        let mut yard = Yard::new(&[(WHIP, level)]);
        let far = yard.target(Vec3::new(0.0, 1.0, -12.0), 100.0);
        yard.tick(&[Action::Fire], &[Action::Fire]);
        yard.idle(60);
        assert_eq!(yard.hitpoints(far) < 100.0, cuts, "V{}", level + 1);
        if cuts {
            // Struck once as it passed, though it crossed him over several ticks.
            let struck = 100.0 - yard.hitpoints(far);
            yard.idle(60);
            assert_eq!(100.0 - yard.hitpoints(far), struck);
        }
    }
}

#[test]
fn the_tesla_barrier_bolts_up_to_four_enemies_near_him_and_goes_after_forty_bolts() {
    let mut yard = Yard::new(&[(SHIELD, 4)]);
    let near: Vec<Entity> = [-2.0, 2.0]
        .into_iter()
        .map(|x| yard.enemy(Vec3::new(x, 1.0, -1.0), 10_000.0))
        .collect();
    let far = yard.enemy(Vec3::new(0.0, 1.0, -8.0), 10_000.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    assert_eq!(yard.body().0.shield.points, 525.0);
    yard.idle(61);
    for &enemy in &near {
        assert!(yard.hitpoints(enemy) < 10_000.0);
    }
    assert_eq!(yard.hitpoints(far), 10_000.0);
    assert!(yard.count_shots(|kind| matches!(kind, ShotKind::Bolt { .. })) <= 4);
    // Two at a time, 20 to 60 ticks each: its forty are done well within 2400 ticks.
    yard.idle(2400);
    assert_eq!(yard.body().0.shield.points, 0.0);
}

#[test]
fn the_rift_rippers_bolts_strike_the_enemies_round_it() {
    let damage = Armory::uya().get(RIFT).unwrap().level(4).damage;
    for (level, rips) in [(3, false), (4, true)] {
        let mut yard = Yard::new(&[(RIFT, level)]);
        let strong = yard.enemy(Vec3::new(0.0, 1.0, -10.0), 10_000.0);
        yard.tick(&[Action::Fire], &[Action::Fire]);
        yard.idle(300);
        let taken = 10_000.0 - yard.hitpoints(strong);
        assert_eq!(taken > 0.0, rips, "V{} took {taken}", level + 1);
        if rips {
            let bolts = taken / (damage * 0.1);
            assert!((bolts - bolts.round()).abs() < 1e-3, "took {taken}");
        }
    }
}

#[test]
fn the_multi_disc_guns_disc_splits_bouncing_off_an_enemy() {
    let mut yard = Yard::new(&[(DISC, 4)]);
    for x in [-1.0, 1.0] {
        yard.enemy(Vec3::new(x, 1.0, -8.0), 100_000.0);
    }
    yard.tick(&[Action::Fire], &[Action::Fire]);
    let mut most = 0;
    for _ in 0..200 {
        yard.idle(1);
        most = most.max(yard.count_shots(|kind| matches!(kind, ShotKind::Disc { .. })));
    }
    assert!(most >= 2, "at most {most} discs");
}

#[test]
fn the_ultrashield_sends_pulses_at_an_enemy_as_a_hit_shakes_it() {
    let damage = Armory::uya().get(HOLO).unwrap().level(4).damage;
    let mut yard = Yard::new(&[(HOLO, 4)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -12.0), 1000.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    yard.idle(60);
    // The landing struck nothing so far off.
    assert_eq!(yard.hitpoints(enemy), 1000.0);
    let armory = Armory::uya();
    let at = yard
        .arsenal()
        .shots
        .iter()
        .find(|shot| matches!(shot.kind, ShotKind::Barrier(_)))
        .map(|shot| shot.position)
        .expect("the shield");
    {
        let mut arsenal = yard.arsenal_mut();
        let from = at + Vec3::new(0.0, 1.0, -0.5);
        assert!(block_shot(&mut arsenal, &armory, from, Vec3::Z).is_some());
    }
    yard.idle(60);
    let taken = 1000.0 - yard.hitpoints(enemy);
    let pulses = taken / (damage * 0.25);
    assert!(
        taken > 0.0 && (pulses - pulses.round()).abs() < 1e-3,
        "took {taken}"
    );
}

#[test]
fn the_plasma_storms_ball_arcs_to_every_enemy_it_sees_round_it_then_goes() {
    let mut yard = Yard::new(&[(COIL, 4)]);
    let enemies: Vec<Entity> = [-4.0, 4.0]
        .into_iter()
        .map(|x| yard.enemy(Vec3::new(x, 1.0, -8.0), 10_000.0))
        .collect();
    yard.hold(&[Action::Fire], 1);
    yard.idle(60);
    for &enemy in &enemies {
        assert!(yard.hitpoints(enemy) < 10_000.0);
    }
    let storms = |yard: &Yard| yard.count_shots(|kind| matches!(kind, ShotKind::Storm { .. }));
    assert_eq!(storms(&yard), 1);
    // It flies 120 ticks and fades 40.
    yard.idle(120);
    assert_eq!(storms(&yard), 0);
}

#[test]
fn the_infecto_bomb_infects_every_enemy_near_where_it_bursts() {
    let mut yard = Yard::new(&[(INFECTOR, 4)]);
    let near: Vec<Entity> = [0.0, 3.0]
        .into_iter()
        .map(|x| yard.enemy(Vec3::new(x, 1.0, -12.0), 1000.0))
        .collect();
    let far = yard.enemy(Vec3::new(0.0, 1.0, -24.0), 1000.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    yard.idle(120);
    let infected = |entity: Entity| {
        yard.afflicted.iter().any(|afflicted| {
            afflicted.target == entity
                && matches!(afflicted.affliction, Affliction::Infected { .. })
        })
    };
    assert!(near.iter().all(|&enemy| infected(enemy)));
    assert!(!infected(far));
}

#[test]
fn the_megaturret_fires_guided_missiles_and_walks_back_to_a_far_hero() {
    let mut yard = Yard::new(&[(TURRET, 4)]);
    let enemy = yard.enemy(Vec3::new(0.0, 1.0, -16.0), 100_000.0);
    yard.tick(&[Action::Fire], &[Action::Fire]);
    let (mut guided, mut pellets) = (0, 0);
    for _ in 0..200 {
        yard.idle(1);
        guided += yard.count_shots(|kind| matches!(kind, ShotKind::Guided { .. }));
        pellets += yard.count_shots(|kind| *kind == ShotKind::Pellet);
    }
    assert!(guided > 0 && pellets == 0);
    assert!(yard.hitpoints(enemy) < 100_000.0);
    // Carried 24 u off, with the enemy near him, it walks back to him.
    let turret = |yard: &Yard| {
        yard.arsenal()
            .shots
            .iter()
            .find(|shot| matches!(shot.kind, ShotKind::Turret(_)))
            .map(|shot| shot.position)
            .expect("the turret")
    };
    let at = turret(&yard);
    let hero = yard.hero;
    let world = yard.app.world_mut();
    if let Some(mut body) = world.get_mut::<HeroBody>(hero) {
        body.0.position = Vec3::new(at.x + 24.0, 0.0, at.z);
    }
    if let Some(mut transform) = world.get_mut::<SimTransform>(enemy) {
        transform.translation = Vec3::new(at.x + 26.0, 0.0, at.z);
    }
    yard.idle(400);
    assert!(turret(&yard).distance(at) > 10.0, "{}", turret(&yard));
}

#[test]
fn the_n90_hurricane_fires_every_10_ticks_and_its_bullets_bounce_seven_times() {
    let mut yard = Yard::new(&[(N60, 4)]);
    let log = yard.hold_logged(&[Action::Fire], 31);
    assert_eq!(Yard::fired_ticks(&log), [0, 10, 20, 30]);
    let bullets: Vec<(u32, u32)> = yard
        .arsenal()
        .shots
        .iter()
        .filter_map(|shot| match shot.kind {
            ShotKind::Bullet { lifetime, bounces } => Some((lifetime, bounces)),
            _ => None,
        })
        .collect();
    // 30 u at 60 to 80 u/s, then 30 ticks more.
    assert!(
        bullets
            .iter()
            .all(|&(lifetime, bounces)| (52..=60).contains(&lifetime) && bounces <= 7),
        "{bullets:?}"
    );
}

#[test]
fn the_nitro_eruptors_blast_grows_to_eleven_units() {
    for (level, reaches) in [(3, false), (4, true)] {
        let mut yard = Yard::new(&[(NITRO, level)]);
        let near = yard.target(Vec3::new(0.0, 1.0, -8.0), 10_000.0);
        let far = yard.target(Vec3::new(9.0, 1.0, -8.0), 10_000.0);
        yard.hold(&[Action::Fire], 1);
        yard.idle(150);
        assert!(yard.hitpoints(near) < 10_000.0);
        assert_eq!(yard.hitpoints(far) < 10_000.0, reaches, "V{}", level + 1);
    }
}

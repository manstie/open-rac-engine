//! A hero's guns, one tick at a time (`weapons.md`, "Switching and firing", "The hero's
//! side of firing" and "Each gun").
//!
//! [`step`] is a plain function of the hero's [`Arsenal`], the [`Armory`], this tick's
//! input and what the guns can see. It brings guns out and puts them away, works the
//! quick-select ring ([`super::ring`]), fires the one in hand by its [`FireMode`], moves
//! the shots in flight, and says what they hit and what the hero's moves should know
//! ([`GunHand`]).

use bevy::prelude::*;
use rac_hero::{CameraSetting, CollisionWorld, GunHand, HeroMotion, HeroState, controller};
use rac_input::{Action, PlayerInput};
use rac_sim::SimRng;
use std::f32::consts::TAU;

use super::{
    Affliction, GunEventKind,
    aim::{GunTarget, assist_target, assist_targets, bend, first_target, view_point},
    arsenal::{Arsenal, Hand},
    def::{
        AimAssist, Armory, FireMode, Hitscan, Homing, Lobbed, Mods, Projectile, Stream, Timing,
        WeaponDef, WeaponId,
    },
    grab, hand,
    ring::RingInput,
    seekers,
    shots::{self, Shot, ShotKind, TICK},
    thrown,
};

/// One hit a gun did this tick, before the damage queue keeps each target's strongest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub target: Entity,
    pub weapon: WeaponId,
    pub damage: f32,
    /// The push the target is handed: a direction scaled by the gun's push strength.
    pub push: Vec3,
    /// The shock and acid mods' checks follow the hit.
    pub shock: bool,
    pub acid: bool,
}

/// What a tick of a hero's guns did.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Outcome {
    pub hits: Vec<Hit>,
    pub events: Vec<(WeaponId, GunEventKind)>,
    /// What his moves should know this tick.
    pub hand: GunHand,
    /// The gun he would lock on with has its lock-on mod.
    pub lock_on: bool,
    /// What the guns did to targets beyond their damage.
    pub afflictions: Vec<Afflict>,
    /// The Shield Charger raised a shield of these points round him.
    pub shield: Option<f32>,
    /// Health the guns gave him back (the Holo Shield's, from V2).
    pub heal: u32,
}

/// A gun afflicted a target ([`Affliction`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Afflict {
    pub target: Entity,
    pub weapon: WeaponId,
    pub affliction: Affliction,
}

impl Outcome {
    pub(crate) fn event(&mut self, weapon: WeaponId, kind: GunEventKind) {
        self.events.push((weapon, kind));
    }
}

/// What the hero's camera shows, for the guns that shoot where it looks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub setting: CameraSetting,
    pub eye: Vec3,
    /// A unit vector.
    pub direction: Vec3,
}

/// What a hero's guns see this tick.
pub struct GunInput<'a> {
    pub input: &'a PlayerInput,
    pub hero: &'a HeroMotion,
    /// The lock-on target and its middle.
    pub lock: Option<(Entity, Vec3)>,
    pub view: Option<View>,
    pub world: &'a dyn CollisionWorld,
    /// In a fixed order, as every peer builds it.
    pub targets: &'a [GunTarget],
    /// He has less health than he can have.
    pub hurt: bool,
}

/// The gun in hand's own state, started afresh each time a gun comes out.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Trigger {
    pub(super) cooldown: u32,
    /// Ticks to the next shot while the button is held ([`Timing::Repeat`]).
    pub(super) repeat: u32,
    /// Ticks the fire button has been held.
    pub(super) held: u32,
    /// This hold has fired its shot (the Shock Blaster's charging levels).
    pub(super) fired: bool,
    /// The Shock Blaster's shot on its way: ticks to its hit, and its damage scale.
    pub(super) pending: Option<(u32, f32)>,
    pub(super) after_hit: u32,
    /// Ticks in the look, which some guns wait out.
    pub(super) look: u32,
    /// The Lava Gun's stream strength, its spring's rate, and ticks to its next round.
    pub(super) strength: f32,
    pub(super) strength_rate: f32,
    pub(super) ammo_ticks: u32,
    pub(super) barrel: usize,
    /// The Annihilator's target, the best candidate, how long that has been best, and how
    /// long another than the target has been.
    pub(super) picked: Option<Entity>,
    pub(super) candidate: Option<Entity>,
    pub(super) candidate_ticks: u32,
    pub(super) other_ticks: u32,
    /// Ticks until what a press started leaves the hand: the Plasma Coil's ball once its
    /// coil has spun up, a glove's throw.
    pub(super) launch: Option<u32>,
    /// The targets the Spitting Hydra has marked, and those the RY3NO marks this tick.
    pub(super) marked: Vec<Entity>,
    /// The Qwack-O-Ray's target, the ticks it has been on it, and its duck meter.
    pub(super) beam: Option<(Entity, u32, f32)>,
    /// The Suck Cannon fires what it holds rather than sucking.
    pub(super) expelling: bool,
}

/// The states a gun fires in. The game's list isn't read: these are the ones where he has
/// his hands free [placeholder].
fn can_fire_in(state: HeroState) -> bool {
    matches!(
        state,
        HeroState::Idle
            | HeroState::Look
            | HeroState::Walk
            | HeroState::Skid
            | HeroState::Crouch
            | HeroState::Fall
            | HeroState::Jump(_)
            | HeroState::DoubleJump
            | HeroState::Glide
            | HeroState::Grind
    )
}

/// One tick of a hero's guns.
pub fn step(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun_input: &GunInput,
    rng: &mut SimRng,
) -> Outcome {
    let mut out = Outcome::default();
    let hero = gun_input.hero;
    // The Plasma Whip's attacks keep it in his hand.
    let busy = (hero.state.is_attacking() && !hero.whip_attack()) || hero.state == HeroState::Dead;
    // The whip's attacks his moves started since last tick, each taking a round.
    let whip_attacks = hero.whip_attacks.wrapping_sub(arsenal.whip_seen);
    arsenal.whip_seen = hero.whip_attacks;
    // The ring reads the pad as it is. Once it holds the hero's input, his guns see none.
    quick_select(arsenal, armory, gun_input.input, busy);
    let blocked = arsenal.quick.blocks_input();
    let still = PlayerInput::default();
    let gun_input = &GunInput {
        input: if blocked { &still } else { gun_input.input },
        ..*gun_input
    };
    let input = gun_input.input;

    // A wrench attack puts the gun away: the controller clears his hand as it starts one.
    if (busy || (arsenal.shown_out && !hero.gun.out)) && arsenal.hand != Hand::Wrench {
        arsenal.put_away();
    }
    match &mut arsenal.hand {
        Hand::Drawing { weapon, ticks_left } => {
            *ticks_left = ticks_left.saturating_sub(1);
            if *ticks_left == 0 {
                arsenal.hand = Hand::Gun {
                    weapon: *weapon,
                    ticks: 0,
                };
            }
        }
        Hand::Gun { ticks, .. } => *ticks = ticks.saturating_add(1),
        Hand::Wrench => {}
    }

    buttons(arsenal, armory, input, busy);

    arsenal.trigger.look = if hero.state == HeroState::Look {
        arsenal.trigger.look.saturating_add(1)
    } else {
        0
    };
    arsenal.fire_ticks = arsenal.fire_ticks.saturating_sub(1);
    if let Hand::Gun { weapon, ticks } = arsenal.hand
        && let Some(def) = armory.get(weapon)
    {
        // Setting another animation ends the firing one, unless the gun is held.
        if hero.ticks == 0 && !def.held {
            arsenal.fire_ticks = 0;
        }
        let ready = ticks >= def.ready_ticks
            && can_fire_in(hero.state)
            && (hero.state != HeroState::Look || arsenal.trigger.look >= def.look_wait_ticks);
        let gun = Gun::new(arsenal, armory, weapon, def, ready, gun_input);
        let before = out.events.len();
        match &def.fire {
            FireMode::Hitscan(fire) => {
                hitscan(arsenal, armory, &gun, fire, gun_input, rng, &mut out)
            }
            FireMode::Projectile(fire) => {
                projectile(arsenal, armory, &gun, fire, gun_input, rng, &mut out);
            }
            FireMode::Lobbed(fire) => lobbed(arsenal, armory, &gun, fire, gun_input, &mut out),
            FireMode::Stream(fire) => stream(arsenal, armory, &gun, fire, gun_input, &mut out),
            FireMode::Homing(fire) => homing(arsenal, armory, &gun, fire, gun_input, &mut out),
            FireMode::Coil(fire) => seekers::coil(arsenal, armory, &gun, fire, gun_input, &mut out),
            FireMode::Cluster(fire) => {
                thrown::cluster(arsenal, armory, &gun, fire, gun_input, &mut out);
            }
            FireMode::Infect(fire) => {
                thrown::infect(arsenal, armory, &gun, fire, gun_input, rng, &mut out);
            }
            FireMode::Disc(fire) => seekers::disc(arsenal, armory, &gun, fire, gun_input, &mut out),
            FireMode::Volley(fire) => {
                seekers::volley(arsenal, armory, &gun, fire, gun_input, rng, &mut out);
            }
            FireMode::Hydra(fire) => {
                seekers::hydra(arsenal, armory, &gun, fire, gun_input, rng, &mut out);
            }
            FireMode::Rift(fire) => {
                grab::rift(arsenal, armory, &gun, fire, gun_input, rng, &mut out)
            }
            FireMode::Suck(fire) => grab::suck(arsenal, armory, &gun, fire, gun_input, &mut out),
            FireMode::Beam(fire) => grab::beam(arsenal, armory, &gun, fire, gun_input, &mut out),
            FireMode::Turret(fire) => {
                thrown::turret(arsenal, armory, &gun, fire, gun_input, &mut out);
            }
            FireMode::Barrier(fire) => {
                thrown::barrier(arsenal, armory, &gun, fire, gun_input, &mut out);
            }
            FireMode::Agents(fire) => {
                thrown::agents(arsenal, armory, &gun, fire, gun_input, &mut out);
            }
            FireMode::Shield(fire) => {
                hand::shield(arsenal, armory, &gun, fire, gun_input, &mut out)
            }
            FireMode::Whip(fire) => {
                hand::whip(
                    arsenal,
                    armory,
                    &gun,
                    fire,
                    gun_input,
                    whip_attacks,
                    &mut out,
                );
            }
        }
        // A held gun stops firing when the button is let go.
        if def.held && !input.held(Action::Fire) {
            arsenal.fire_ticks = 0;
        }
        // Each shot, and each of a stream's rounds, starts the firing animation again.
        let fired = out.events[before..]
            .iter()
            .any(|(_, kind)| matches!(kind, GunEventKind::Fired { .. }));
        if fired {
            arsenal.shots_fired = arsenal.shots_fired.wrapping_add(1);
        }
    }

    shots::step(
        &mut arsenal.shots,
        &mut arsenal.shot_serial,
        armory,
        gun_input,
        rng,
        &mut out,
    );
    hand::shield_tick(arsenal, armory, gun_input, rng, &mut out);

    let in_hand = arsenal.hand.gun().and_then(|weapon| armory.get(weapon));
    let firing = in_hand.is_some() && arsenal.fire_ticks > 0;
    let level = arsenal
        .hand
        .gun()
        .and_then(|weapon| arsenal.gun(weapon))
        .map_or(0, |gun| gun.level);
    let target = in_hand.and_then(|def| head_target(def, level, gun_input, armory));
    out.hand = GunHand {
        out: in_hand.is_some(),
        firing,
        firing_held: firing && in_hand.is_some_and(|def| def.held),
        steady: in_hand.is_some_and(|def| def.steady),
        item: in_hand.map(|def| def.item),
        level,
        pose: in_hand.map(|def| def.pose).unwrap_or_default(),
        shots: arsenal.shots_fired,
        selecting: blocked,
        whip: in_hand.and_then(|def| hand::whip_hand(arsenal, armory, def, level)),
        target,
    };
    arsenal.shown_out = out.hand.out;
    out.lock_on = arsenal
        .hand
        .gun()
        .or(arsenal.equipped)
        .is_some_and(|weapon| arsenal.mods(weapon, armory).lock_on);
    out
}

/// The point the gun in hand aims at this tick, for his head and upper body to turn to
/// (`gun_target_now`; `weapons.md`, "The head and upper body turn to the gun's
/// target"): none while he is locked on, whom he faces anyway, or in the lock-strafe
/// camera, where guns aim with the view; else the enemy its own assist picks from the
/// muzzle along his facing. UYA's guns set it as they aim, each in its own update
/// (the Shock Blaster and Lava Gun too, which the kit's give no assist): here every gun
/// with an assist names its pick each tick [simplified].
fn head_target(def: &WeaponDef, level: u8, gun_input: &GunInput, armory: &Armory) -> Option<Vec3> {
    let assist = def.fire.assist(level)?;
    let hero = gun_input.hero;
    let strafing = gun_input
        .view
        .is_some_and(|view| view.setting == CameraSetting::LockStrafe);
    if gun_input.lock.is_some() || strafing {
        return None;
    }
    let [right, up, forward] = armory.rules.muzzle;
    let muzzle = hero.position + Quat::from_rotation_y(hero.yaw) * Vec3::new(right, up, -forward);
    let picked = assist_targets(
        gun_input.targets,
        muzzle,
        hero.facing(),
        &assist,
        gun_input.world,
        |target| target.enemy,
        1,
    );
    picked.first().map(|&index| gun_input.targets[index].centre)
}

/// The quick-select button (`weapons.md`, "The quick-select ring"): pressed, it opens the
/// ring; let go soon after, it switches guns, but not while he attacks; held, the ring
/// picks a gun, and letting go chooses it there, which comes out unless he is `busy`.
fn quick_select(arsenal: &mut Arsenal, armory: &Armory, input: &PlayerInput, busy: bool) {
    let rules = &armory.rules;
    arsenal.quick_tapped = arsenal.quick_tapped.saturating_sub(1);
    if input.pressed(Action::QuickSelect) {
        arsenal.quick_held = Some(0);
        if !busy && arsenal.quick.open.is_none() && !arsenal.quick.is_empty() {
            let delay = if arsenal.quick_tapped > 0 {
                rules.ring.input_delay_after_tap_ticks
            } else {
                rules.ring.input_delay_ticks
            };
            arsenal.quick.open(delay, &rules.ring);
        }
    } else if input.held(Action::QuickSelect) {
        if let Some(ticks) = &mut arsenal.quick_held {
            *ticks = ticks.saturating_add(1);
        }
    } else if let Some(ticks) = arsenal.quick_held.take()
        && ticks < rules.quick_switch_ticks
        && !busy
    {
        // A tap that switches no gun makes the ring opened soon after hold his input
        // later (`quick_switch_recent`).
        arsenal.quick_tapped = rules.quick_switch_ticks;
        if arsenal.quick_switch(armory) {
            arsenal.quick_tapped = 0;
        }
    }
    let ring = RingInput {
        held: arsenal.quick_held,
        move_stick: input.move_stick,
        ring_two: input.held(Action::Crouch),
        left: input.pressed(Action::PrevWeapon),
        right: input.pressed(Action::NextWeapon),
    };
    if let Some(weapon) = arsenal.quick.update(&ring, &rules.ring) {
        arsenal.pick(weapon, busy, armory);
    }
}

/// The buttons that bring a gun out and change it. While he is `busy` (attacking or dead)
/// they do nothing, and while the ring is open the weapon buttons work it instead.
fn buttons(arsenal: &mut Arsenal, armory: &Armory, input: &PlayerInput, busy: bool) {
    // The fire button, and lock-on with the gun's lock-on mod, bring the gun out.
    if !busy
        && arsenal.hand == Hand::Wrench
        && let Some(equipped) = arsenal.equipped
    {
        let lock = input.pressed(Action::LockOn) && arsenal.mods(equipped, armory).lock_on;
        if input.pressed(Action::Fire) || lock {
            arsenal.draw(equipped, armory);
        }
    }
    if arsenal.quick.open.is_none() && !busy {
        if input.pressed(Action::NextWeapon) {
            arsenal.cycle(true, armory);
        } else if input.pressed(Action::PrevWeapon) {
            arsenal.cycle(false, armory);
        }
    }
}

/// The gun in hand this tick, and where it points.
pub(super) struct Gun<'a> {
    pub(super) weapon: WeaponId,
    pub(super) def: &'a WeaponDef,
    pub(super) level: u8,
    pub(super) damage: f32,
    pub(super) splash: f32,
    pub(super) mods: Mods,
    pub(super) muzzle: Vec3,
    /// His facing, which is the fire direction (`hero_fire_dir`).
    pub(super) facing: Vec3,
    /// It may fire this tick: out long enough, in a state that fires, past any wait.
    pub(super) ready: bool,
    /// The lock-on target's middle, while it lives.
    pub(super) lock: Option<(Entity, Vec3)>,
    /// Where the view centres, while guns shoot there.
    pub(super) view_point: Option<Vec3>,
}

impl<'a> Gun<'a> {
    fn new(
        arsenal: &Arsenal,
        armory: &Armory,
        weapon: WeaponId,
        def: &'a WeaponDef,
        ready: bool,
        gun_input: &GunInput,
    ) -> Self {
        let hero = gun_input.hero;
        let level = arsenal.gun(weapon).map_or(0, |gun| gun.level);
        let stats = def.level(level);
        let rotation = Quat::from_rotation_y(hero.yaw);
        let [right, up, forward] = armory.rules.muzzle;
        let muzzle = hero.position + rotation * Vec3::new(right, up, -forward);
        let lock = gun_input.lock.filter(|(entity, _)| {
            gun_input
                .targets
                .iter()
                .any(|target| target.entity == *entity && target.alive)
        });
        // The lock-strafe and first-person cameras, and the look, aim where the view
        // centres (`weapons.md`, "How guns aim with it").
        let view_point = gun_input
            .view
            .filter(|view| {
                view.setting != CameraSetting::ThirdPerson || hero.state == HeroState::Look
            })
            .map(|view| {
                view_point(
                    gun_input.world,
                    gun_input.targets,
                    view.eye,
                    view.direction,
                    armory.rules.view_range,
                )
            });
        Self {
            weapon,
            def,
            level,
            damage: stats.damage,
            splash: stats.splash,
            mods: arsenal.mods(weapon, armory),
            muzzle,
            facing: hero.facing(),
            ready,
            lock,
            view_point,
        }
    }

    /// The fire direction bent towards where the view centres (`aim_bend_to_point`), or
    /// the facing.
    pub(super) fn view_aim(&self, from: Vec3, armory: &Armory) -> Vec3 {
        let Some(point) = self.view_point else {
            return self.facing;
        };
        let rules = &armory.rules;
        let wanted = (point - from).normalize_or(self.facing);
        let mut most = rules.bend_degrees.to_radians();
        if point.distance(from) < rules.bend_near {
            most *= 0.1;
        }
        bend(self.facing, wanted, most)
    }

    /// Straight at the lock-on target (`aim_at_target`), else where the view centres.
    fn aim(&self, from: Vec3, armory: &Armory) -> Vec3 {
        match self.lock {
            Some((_, at)) => (at - from).normalize_or(self.facing),
            None => self.view_aim(from, armory),
        }
    }

    /// What a gun that aims for itself shoots at from `from` (`weapons.md`, "Gun aim
    /// assist"): the lock-on target while it lives; with the lock-strafe camera, where the
    /// view centres; otherwise the enemy its assist picks, else where the view centres.
    pub(super) fn seek(
        &self,
        from: Vec3,
        assist: &AimAssist,
        gun_input: &GunInput,
        armory: &Armory,
    ) -> (Option<Entity>, Vec3) {
        if let Some((entity, at)) = self.lock {
            return (Some(entity), (at - from).normalize_or(self.facing));
        }
        let strafing = gun_input
            .view
            .is_some_and(|view| view.setting == CameraSetting::LockStrafe);
        let picked = (!strafing)
            .then(|| {
                assist_targets(
                    gun_input.targets,
                    from,
                    self.facing,
                    assist,
                    gun_input.world,
                    |target| target.enemy,
                    1,
                )
            })
            .and_then(|picked| picked.first().copied());
        match picked {
            Some(index) => {
                let target = &gun_input.targets[index];
                (
                    Some(target.entity),
                    (target.centre - from).normalize_or(self.facing),
                )
            }
            None => (None, self.view_aim(from, armory)),
        }
    }

    pub(super) fn shot(&self, serial: u32, position: Vec3, velocity: Vec3, kind: ShotKind) -> Shot {
        Shot {
            serial,
            weapon: self.weapon,
            level: self.level,
            position,
            velocity,
            ticks: 0,
            damage: self.damage,
            splash: self.splash,
            mods: self.mods,
            kind,
        }
    }

    pub(super) fn hit(&self, target: Entity, damage: f32, push: Vec3) -> Hit {
        Hit {
            target,
            weapon: self.weapon,
            damage,
            push,
            shock: self.mods.shock,
            acid: self.mods.acid,
        }
    }
}

/// Takes a round and starts the firing animation; false if it is out of ammo.
pub(super) fn take_shot(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    out: &mut Outcome,
) -> bool {
    if !arsenal.use_ammo(gun.weapon, 1, armory) {
        return false;
    }
    fire_anim(arsenal, gun, out);
    true
}

/// Starts the firing animation (`hero_fire_anim`, 0x4E6A08) without a round.
pub(super) fn fire_anim(arsenal: &mut Arsenal, gun: &Gun, out: &mut Outcome) {
    arsenal.fire_ticks = gun.def.fire_ticks;
    out.event(
        gun.weapon,
        GunEventKind::Fired {
            at: gun.muzzle,
            direction: gun.facing,
        },
    );
}

/// A held gun out of ammo stops firing at once (`hero_fire_stop`).
pub(super) fn run_dry(arsenal: &mut Arsenal) {
    arsenal.fire_ticks = 0;
}

/// Every target touching a ball of `radius` round `at`.
pub(super) fn ball(
    targets: &[GunTarget],
    at: Vec3,
    radius: f32,
) -> impl Iterator<Item = &GunTarget> {
    targets
        .iter()
        .filter(move |target| target.centre.distance(at) <= radius + target.radius)
}

/// The Shock Blaster (0x36A790): a shot while the button is held, or from its charging
/// level one shot a press and a charged one on letting go; each hits a few ticks later
/// with a ball at the gun and a spray of rays.
fn hitscan(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Hitscan,
    gun_input: &GunInput,
    rng: &mut SimRng,
    out: &mut Outcome,
) {
    let hero = gun_input.hero;
    let held = gun_input.input.held(Action::Fire);
    let trigger = &mut arsenal.trigger;
    trigger.cooldown = trigger.cooldown.saturating_sub(1);
    trigger.after_hit = trigger.after_hit.saturating_sub(1);
    let charge = fire.charge.filter(|charge| gun.level >= charge.from_level);

    if let Some((left, scale)) = trigger.pending {
        if left <= 1 {
            trigger.pending = None;
            trigger.after_hit = fire.after_hit_ticks;
            if charge.is_none() {
                trigger.cooldown = Timing::Rate(fire.rate).ticks();
            }
            hitscan_hit(gun, fire, scale, armory, gun_input, rng, out);
        } else {
            trigger.pending = Some((left - 1, scale));
        }
    }

    let free = gun.ready && trigger.pending.is_none() && trigger.after_hit == 0;
    let mut shoot = None;
    match charge {
        None => {
            if held && free && trigger.cooldown == 0 {
                shoot = Some(1.0);
            }
        }
        Some(charge) => {
            // The hold counts from when the gun can fire, so one held as it comes out
            // still fires.
            if held && gun.ready {
                trigger.held = trigger.held.saturating_add(1);
                if !trigger.fired && trigger.held <= charge.try_ticks && free {
                    trigger.fired = true;
                    shoot = Some(1.0);
                }
            } else if !held {
                if trigger.held > charge.ticks && free {
                    shoot = Some(charge.damage_scale);
                }
                trigger.held = 0;
                trigger.fired = false;
            }
        }
    }
    let Some(scale) = shoot else {
        return;
    };
    if !take_shot(arsenal, armory, gun, out) {
        return;
    }
    // Standing, the whole-body firing animation this shot starts already plays, and that
    // delays the hit too, as does moving.
    let standing = matches!(hero.state, HeroState::Idle | HeroState::Look);
    let moving = hero.velocity.length_squared() > 1e-6;
    let delay = if standing || moving {
        fire.busy_hit_ticks
    } else {
        fire.hit_ticks
    };
    arsenal.trigger.pending = Some((delay, scale));
}

/// The Shock Blaster's hit: the ball at the gun, then the rays.
fn hitscan_hit(
    gun: &Gun,
    fire: &Hitscan,
    scale: f32,
    armory: &Armory,
    gun_input: &GunInput,
    rng: &mut SimRng,
    out: &mut Outcome,
) {
    let damage = gun.damage * scale;
    let aim = gun.aim(gun.muzzle, armory);
    for target in ball(gun_input.targets, gun.muzzle, fire.ball_radius) {
        out.hits
            .push(gun.hit(target.entity, damage, aim * fire.push));
    }
    let mut rays = Vec::new();
    // Rays within the spread of the aim, turned any way round it.
    let side = aim.any_orthonormal_vector();
    for _ in 0..fire.random_rays.at(gun.level).unwrap_or(0) {
        let off = rng.unit_f32() * fire.spread_degrees.to_radians();
        let roll = rng.unit_f32() * TAU;
        let axis = Quat::from_axis_angle(aim, roll) * side;
        rays.push(Quat::from_axis_angle(axis, off) * aim);
    }
    // And a fan below it.
    let flat_right = Vec3::new(-aim.z, 0.0, aim.x).normalize_or(Vec3::X);
    let down = Quat::from_axis_angle(flat_right, -fire.down_degrees.to_radians()) * aim;
    for i in 0..fire.down_rays {
        let centre = (fire.down_rays as f32 - 1.0) / 2.0;
        let yaw = (i as f32 - centre) * fire.down_spread_degrees.to_radians();
        rays.push(Quat::from_rotation_y(yaw) * down);
    }
    for direction in rays {
        let line = direction * fire.ray_length;
        let solid = gun_input
            .world
            .cast_ray(gun.muzzle, line)
            .map_or(1.0, |hit| hit.fraction);
        let target = first_target(gun_input.targets, gun.muzzle, gun.muzzle + line, |_| true)
            .filter(|&(_, t)| t <= solid);
        let reach = match target {
            Some((index, t)) => {
                let struck = &gun_input.targets[index];
                // As read, only the ball's hits get the shock mod's check.
                out.hits.push(Hit {
                    shock: false,
                    ..gun.hit(struck.entity, damage, direction * fire.push)
                });
                t
            }
            None => solid,
        };
        out.event(
            gun.weapon,
            GunEventKind::Ray {
                from: gun.muzzle,
                to: gun.muzzle + line * reach,
            },
        );
    }
}

/// The N60 Storm (0x3A5928) and Flux Rifle (0x345350): a shot that flies, on the gun's
/// timing, from each barrel in turn.
fn projectile(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Projectile,
    gun_input: &GunInput,
    rng: &mut SimRng,
    out: &mut Outcome,
) {
    let held = gun_input.input.held(Action::Fire);
    let trigger = &mut arsenal.trigger;
    trigger.cooldown = trigger.cooldown.saturating_sub(1);
    let due = match fire.timing {
        Timing::Repeat(_) if !held => {
            trigger.repeat = 0;
            false
        }
        Timing::Repeat(_) => {
            trigger.repeat = trigger.repeat.saturating_sub(1);
            trigger.repeat == 0
        }
        Timing::Rate(_) => held && trigger.cooldown == 0,
    };
    if !due || !gun.ready {
        return;
    }
    if !take_shot(arsenal, armory, gun, out) {
        run_dry(arsenal);
        return;
    }
    let hurricane = fire
        .hurricane
        .filter(|hurricane| gun.level >= hurricane.from_level);
    let trigger = &mut arsenal.trigger;
    match fire.timing {
        Timing::Repeat(ticks) => {
            trigger.repeat = hurricane.map_or(ticks, |hurricane| hurricane.repeat_ticks);
        }
        timing => trigger.cooldown = timing.ticks(),
    }
    let barrel = fire
        .barrels
        .get(trigger.barrel % fire.barrels.len().max(1))
        .copied()
        .unwrap_or_default();
    trigger.barrel = trigger.barrel.wrapping_add(1);
    let rotation = Quat::from_rotation_y(gun_input.hero.yaw);
    let from = gun.muzzle + rotation * Vec3::new(barrel[0], barrel[1], -barrel[2]);

    // The lock-on target wins; in the views that aim, the view's aim; otherwise the gun's
    // own assist, which the lock-strafe camera turns off.
    let assisted = || {
        let assist = fire.assist?;
        if gun_input
            .view
            .is_some_and(|view| view.setting == CameraSetting::LockStrafe)
        {
            return None;
        }
        let index = assist_target(
            gun_input.targets,
            from,
            gun.facing,
            &assist,
            gun_input.world,
        )?;
        Some(gun_input.targets[index].centre)
    };
    let mut direction = match (gun.lock, gun.view_point) {
        (Some((_, at)), _) => (at - from).normalize_or(gun.facing),
        (None, Some(_)) => gun.view_aim(from, armory),
        (None, None) => assisted().map_or(gun.facing, |at| (at - from).normalize_or(gun.facing)),
    };
    // Scoped in the look, the aim bends to the nearest target near it.
    let scoped = gun_input.hero.state == HeroState::Look;
    let mut scoped_at = None;
    if let Some((range, degrees)) = fire.scope
        && scoped
    {
        let limit = degrees.to_radians();
        let nearest = gun_input
            .targets
            .iter()
            .filter(|target| target.alive && target.centre.distance(from) <= range)
            .map(|target| {
                let to = (target.centre - from).normalize_or_zero();
                (direction.angle_between(to), to, target.centre)
            })
            .filter(|&(angle, ..)| angle <= limit)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, to, at)) = nearest {
            direction = to;
            scoped_at = Some(at);
        }
    }
    // And a beam goes from the eye along the aim with the shot.
    if let (Some(beam), Some(view), true) = (&fire.scope_beam, gun_input.view, scoped) {
        let aim = scoped_at.map_or(view.direction, |at| {
            (at - view.eye).normalize_or(view.direction)
        });
        out.event(
            gun.weapon,
            GunEventKind::Ray {
                from: view.eye,
                to: view.eye + aim * beam.shown,
            },
        );
        let serial = arsenal.next_serial();
        arsenal.shots.push(gun.shot(
            serial,
            view.eye,
            aim * (beam.step / TICK),
            ShotKind::Beam { hit: false },
        ));
    }

    if fire.muzzle_ball > 0.0 {
        for target in ball(gun_input.targets, gun.muzzle, fire.muzzle_ball) {
            out.hits
                .push(gun.hit(target.entity, gun.damage, direction * fire.push));
        }
    }
    let (low, high) = fire.speed;
    let speed = if high > low {
        rng.range_f32(low, high)
    } else {
        low
    };
    let bounces = fire.bounce_from.is_some_and(|from| gun.level >= from);
    let kind = match hurricane {
        Some(hurricane) => ShotKind::Bullet {
            lifetime: (hurricane.reach / (speed * TICK)) as u32 + hurricane.extra_ticks,
            bounces: hurricane.bounces,
        },
        None => ShotKind::Bullet {
            lifetime: fire.lifetime.at(gun.level).unwrap_or(0),
            bounces: if bounces { u32::MAX } else { 0 },
        },
    };
    let serial = arsenal.next_serial();
    arsenal
        .shots
        .push(gun.shot(serial, from, direction * speed, kind));
}

/// The Nitro Launcher (0x36E360): a charge lobbed at the target, or ahead, on an arc
/// solved for the aim's gravity.
fn lobbed(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Lobbed,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    let trigger = &mut arsenal.trigger;
    trigger.cooldown = trigger.cooldown.saturating_sub(1);
    if !gun_input.input.held(Action::Fire) || trigger.cooldown > 0 || !gun.ready {
        return;
    }
    if !take_shot(arsenal, armory, gun, out) {
        return;
    }
    arsenal.trigger.cooldown = Timing::Rate(fire.rate).ticks();
    let hero = gun_input.hero;
    let target = gun
        .lock
        .filter(|(_, at)| at.distance(hero.position) <= fire.lock_range)
        .map(|(_, at)| at)
        .or_else(|| {
            let index = assist_target(
                gun_input.targets,
                gun.muzzle,
                gun.facing,
                &fire.assist,
                gun_input.world,
            )?;
            Some(gun_input.targets[index].centre)
        });
    let aim_point = target.unwrap_or(gun.muzzle + gun.facing * fire.reach);
    let velocity = lob(gun.muzzle, aim_point, gun.facing, fire);
    let serial = arsenal.next_serial();
    arsenal.shots.push(gun.shot(
        serial,
        gun.muzzle,
        velocity,
        ShotKind::Charge { fuse: None },
    ));
}

/// The launch that carries a charge from `from` across the ground at the arc's speed and
/// up or down to `to`'s height under the aim's gravity (0x36EA80).
fn lob(from: Vec3, to: Vec3, facing: Vec3, fire: &Lobbed) -> Vec3 {
    let across = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let distance = across.length();
    let way = if distance > 1e-4 {
        across / distance
    } else {
        facing
    };
    // At least a tick's flight, so a point overhead doesn't divide by nothing.
    let seconds = (distance / fire.ground_speed).max(1.0 / 60.0);
    let rise = to.y - from.y;
    let up = (rise + 0.5 * fire.aim_gravity * seconds * seconds) / seconds;
    let mut launch = way * fire.ground_speed + Vec3::Y * up;
    if launch.length() > fire.max_launch {
        launch = launch.normalize() * fire.cut_launch;
    }
    launch
}

/// The Lava Gun (0x303F88): while the button is held the stream's strength springs up,
/// a round going every few ticks; while it is above 0, a point leaves the nozzle each
/// tick.
fn stream(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Stream,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    let hero = gun_input.hero;
    let mut flowing = gun_input.input.held(Action::Fire) && gun.ready;
    if flowing {
        if arsenal.trigger.ammo_ticks == 0 {
            if arsenal.use_ammo(gun.weapon, 1, armory) {
                arsenal.trigger.ammo_ticks = fire.ammo_ticks;
                out.event(
                    gun.weapon,
                    GunEventKind::Fired {
                        at: gun.muzzle,
                        direction: gun.facing,
                    },
                );
            } else {
                flowing = false;
                run_dry(arsenal);
            }
        }
        arsenal.trigger.ammo_ticks = arsenal.trigger.ammo_ticks.saturating_sub(1);
    } else {
        arsenal.trigger.ammo_ticks = 0;
    }
    if flowing {
        arsenal.fire_ticks = gun.def.fire_ticks;
    }
    let trigger = &mut arsenal.trigger;
    let (target, spring) = if flowing {
        (fire.full, fire.rise)
    } else {
        (0.0, fire.fall)
    };
    trigger.strength = controller::game_spring(
        trigger.strength,
        target,
        spring,
        0.0,
        &mut trigger.strength_rate,
    );
    if trigger.strength <= 0.0 {
        return;
    }

    let mut direction = gun.aim(gun.muzzle, armory);
    direction.y += fire.raise;
    direction = direction.normalize_or(gun.facing);
    if hero.velocity.length_squared() > 1e-6 {
        let right = Vec3::new(-direction.z, 0.0, direction.x).normalize_or(Vec3::X);
        direction =
            Quat::from_axis_angle(right, -fire.moving_tilt_degrees.to_radians()) * direction;
    }
    let speed = trigger.strength * fire.speed_per_strength;
    let points = arsenal
        .shots
        .iter()
        .filter(|shot| shot.weapon == gun.weapon && shot.kind == ShotKind::Stream)
        .count();
    if points >= fire.points
        && let Some(oldest) = arsenal
            .shots
            .iter()
            .position(|shot| shot.weapon == gun.weapon && shot.kind == ShotKind::Stream)
    {
        arsenal.shots.remove(oldest);
    }
    let serial = arsenal.next_serial();
    arsenal
        .shots
        .push(gun.shot(serial, gun.muzzle, direction * speed, ShotKind::Stream));
    for target in ball(gun_input.targets, gun.muzzle, fire.nozzle_radius) {
        out.hits
            .push(gun.hit(target.entity, gun.damage, Vec3::ZERO));
    }
}

/// The Annihilator (0x36D5A0): it picks a target as the assist keeps finding it, and fires
/// a missile that turns after it.
fn homing(
    arsenal: &mut Arsenal,
    armory: &Armory,
    gun: &Gun,
    fire: &Homing,
    gun_input: &GunInput,
    out: &mut Outcome,
) {
    let hero = gun_input.hero;
    let aimed = hero.state == HeroState::Look
        || gun_input
            .view
            .is_some_and(|view| view.setting == CameraSetting::LockStrafe);
    let assist = if aimed {
        &fire.aimed_assist
    } else {
        &fire.assist
    };
    let from = hero.position + Vec3::Y * fire.assist_height;
    let best = assist_target(gun_input.targets, from, gun.facing, assist, gun_input.world)
        .map(|index| gun_input.targets[index].entity);
    pick(&mut arsenal.trigger, best, fire, gun_input.targets);

    let trigger = &mut arsenal.trigger;
    trigger.cooldown = trigger.cooldown.saturating_sub(1);
    if !gun_input.input.held(Action::Fire) || trigger.cooldown > 0 || !gun.ready {
        return;
    }
    if !take_shot(arsenal, armory, gun, out) {
        return;
    }
    arsenal.trigger.cooldown = Timing::Rate(fire.rate).ticks();
    let picked = arsenal.trigger.picked.and_then(|entity| {
        gun_input
            .targets
            .iter()
            .find(|target| target.entity == entity)
            .map(|target| (entity, target.centre))
    });
    let near = || {
        let ahead = gun.muzzle + gun.facing * fire.near_ahead;
        ball(gun_input.targets, ahead, fire.near_radius)
            .find(|target| target.alive)
            .map(|target| (target.entity, target.centre))
    };
    let target = gun.lock.or(picked).or_else(near);
    let direction = match target {
        Some((_, at)) => (at - gun.muzzle).normalize_or(gun.facing),
        None => gun.view_aim(gun.muzzle, armory),
    };
    let serial = arsenal.next_serial();
    arsenal.shots.push(gun.shot(
        serial,
        gun.muzzle,
        direction * fire.speed,
        ShotKind::Missile {
            target: target.map(|(entity, _)| entity),
        },
    ));
}

/// The Annihilator's pick: the best candidate becomes the target once it has been best a
/// while, and another being best a while, or the target dying, drops it.
fn pick(trigger: &mut Trigger, best: Option<Entity>, fire: &Homing, targets: &[GunTarget]) {
    if let Some(picked) = trigger.picked {
        let alive = targets
            .iter()
            .any(|target| target.entity == picked && target.alive);
        if !alive {
            trigger.picked = None;
        } else if best == Some(picked) {
            trigger.other_ticks = 0;
        } else {
            trigger.other_ticks += 1;
            if trigger.other_ticks >= fire.drop_ticks {
                trigger.picked = None;
            }
        }
    }
    if trigger.picked.is_some() {
        return;
    }
    if best.is_some() && best == trigger.candidate {
        trigger.candidate_ticks += 1;
    } else {
        trigger.candidate = best;
        trigger.candidate_ticks = u32::from(best.is_some());
    }
    if trigger.candidate_ticks >= fire.pick_ticks {
        trigger.picked = trigger.candidate;
        trigger.other_ticks = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nitro() -> Lobbed {
        match &Armory::uya().weapons[2].fire {
            FireMode::Lobbed(fire) => fire.clone(),
            _ => unreachable!("the third gun is the Nitro Launcher"),
        }
    }

    #[test]
    fn a_lob_reaches_its_point_under_the_aims_gravity() {
        let fire = nitro();
        let from = Vec3::new(0.0, 1.0, 0.0);
        let to = Vec3::new(0.0, 0.0, -8.0);
        let launch = lob(from, to, Vec3::NEG_Z, &fire);
        // 12.5 u/s across the ground.
        assert!((Vec2::new(launch.x, launch.z).length() - 12.5).abs() < 1e-4);
        let seconds = 8.0 / 12.5;
        let height = from.y + launch.y * seconds - 0.5 * fire.aim_gravity * seconds * seconds;
        assert!((height - to.y).abs() < 1e-4, "{height}");
    }

    #[test]
    fn a_launch_too_fast_is_cut() {
        let fire = nitro();
        // Far above: the upward speed needed is well past 30 u/s.
        let launch = lob(Vec3::ZERO, Vec3::new(0.0, 40.0, -1.0), Vec3::NEG_Z, &fire);
        assert!((launch.length() - fire.cut_launch).abs() < 1e-4);
    }

    #[test]
    fn the_pick_waits_for_a_candidate_to_stay_best_and_drops_it_for_another() {
        let fire = match &Armory::uya().weapons[4].fire {
            FireMode::Homing(fire) => fire.clone(),
            _ => unreachable!("the fifth gun is the Annihilator"),
        };
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(2).unwrap();
        let targets = [a, b].map(|entity| GunTarget {
            entity,
            centre: Vec3::ZERO,
            radius: 0.5,
            alive: true,
            health: None,
            enemy: true,
        });
        let mut trigger = Trigger::default();
        for _ in 0..4 {
            pick(&mut trigger, Some(a), &fire, &targets);
        }
        assert_eq!(trigger.picked, None);
        pick(&mut trigger, Some(a), &fire, &targets);
        assert_eq!(trigger.picked, Some(a));
        for _ in 0..4 {
            pick(&mut trigger, Some(b), &fire, &targets);
        }
        assert_eq!(trigger.picked, Some(a));
        pick(&mut trigger, Some(b), &fire, &targets);
        assert_eq!(trigger.picked, None);
    }
}

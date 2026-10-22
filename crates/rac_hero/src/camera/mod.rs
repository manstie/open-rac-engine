//! The hero's camera (PLAN.md P1.7), after UYA's and Deadlocked's (`docs/research/camera.md`).
//!
//! It runs in the simulation, because the hero reads the stick relative to the camera's
//! yaw, as in the games: what the camera did last tick decides where "forward" is this
//! tick.
//!
//! Each player's [`FollowCamera`] shows one of three cameras, chosen by the player's
//! [`CameraSetting`] and by what the hero does:
//! - **the follow camera** ([`follow`]) in third person: a drag camera that swings round
//!   behind the hero as he runs;
//! - **the look**: in third person, while the hero stands in [`HeroState::Look`], the free
//!   look at his eye;
//! - **the hero camera** for the lock-strafe and first-person settings: the free look
//!   ([`free_look`]), 6 u behind his eye or at it, which only the player turns. The
//!   first-person button gives first person in lock-strafe.
//!
//! Switching between them blends the shown camera from where it was ("Blending between
//! cameras"). [`CameraVolume`]s retune the follow camera while the hero is inside them.

use bevy::{math::ops, prelude::*};
use rac_input::{PlayerInput, PlayerInputs};
use rac_sim::SimTransform;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

mod follow;
mod free_look;
mod obstacles;
mod vertical;
mod volume;

pub use crate::profile::CameraSetting;
pub use volume::CameraVolume;

use crate::{
    CollisionScene, Hero, HeroBody, HeroTuning, LockOn,
    collision::CollisionWorld,
    controller::{HeroMotion, HeroState},
    profile::{Blend as BlendProfile, CameraProfile, HeroProfile, Spring},
    stick::read_axes,
};
use follow::{FollowInput, Rig};
use free_look::{Aim, EyeMode, FreeLook};

/// The camera-speed option both games start with (0 to 100).
pub const DEFAULT_SPEED: f32 = 50.0;

/// Follows the hero of the same player slot.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct FollowCamera {
    pub player: usize,
    /// The direction the camera faces, radians about +y; 0 looks down −z. The hero reads the
    /// stick relative to this.
    pub yaw: f32,
    /// Radians; negative looks down.
    pub pitch: f32,
    /// The player's camera setting; `None` takes the movement profile's default.
    pub setting: Option<CameraSetting>,
    /// The camera-speed option, 0 to 100.
    pub speed: f32,
    state: Option<CameraState>,
}

/// Which camera shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Follow,
    /// The third-person look. It shows the follow camera's view from the eye until its
    /// position blend is half done; then it is ready, and turns and turns the hero.
    Look {
        ready: bool,
    },
    /// The lock-strafe and first-person settings' free look.
    Hero,
}

#[derive(Debug, Clone, PartialEq)]
struct CameraState {
    rig: Rig,
    free: FreeLook,
    view: View,
    /// Ticks the hero has been in [`HeroState::Look`].
    look_ticks: u32,
    blend: Option<Blend>,
    /// What was shown last tick.
    shown: (Vec3, Quat),
    /// The volumes the hero is in, in the order he entered them.
    volumes: Vec<Entity>,
}

impl FollowCamera {
    /// A camera for `player` that will start behind the hero, facing `yaw`.
    pub fn for_player(player: usize, yaw: f32) -> Self {
        Self {
            player,
            yaw,
            pitch: 0.0,
            setting: None,
            speed: DEFAULT_SPEED,
            state: None,
        }
    }

    pub fn with_setting(mut self, setting: CameraSetting) -> Self {
        self.setting = Some(setting);
        self
    }

    /// The setting in force with `profile`.
    pub fn setting_with(&self, profile: &CameraProfile) -> CameraSetting {
        self.setting.unwrap_or(profile.default_setting)
    }

    /// Whether the camera is at the hero's eye, so his model is in the way.
    pub fn is_first_person(&self) -> bool {
        self.state.as_ref().is_some_and(|state| match state.view {
            View::Follow => false,
            View::Look { ready } => ready,
            View::Hero => state.free.mode == EyeMode::FirstPerson,
        })
    }

    /// Whether the camera looks where the hero does in [`HeroState::Look`], so the look
    /// turns him with it: in the lock-strafe and first-person settings always, in third
    /// person once the look has blended half way in (UYA's 0x1A71C5).
    pub fn look_ready(&self) -> bool {
        self.state.as_ref().is_none_or(|state| match state.view {
            View::Follow => false,
            View::Look { ready } => ready,
            View::Hero => true,
        })
    }

    /// Where the camera rests, and how it is turned, for a hero standing at `feet` with
    /// the camera facing `self.yaw`.
    pub fn placement(&self, feet: Vec3, profile: &CameraProfile) -> (Vec3, Quat) {
        let free = &profile.free_look;
        let forward = view_direction(self.yaw, 0.0);
        match self.setting_with(profile) {
            CameraSetting::ThirdPerson => {
                let position =
                    feet + Vec3::Y * profile.height + arm(self.yaw, 0.0, profile.distance);
                let aim = feet + Vec3::Y * profile.look_height;
                (position, look_rotation(aim - position))
            }
            CameraSetting::LockStrafe => (
                feet + Vec3::Y * free.eye_lock_strafe - forward * free.offset,
                look_rotation(forward),
            ),
            CameraSetting::FirstPerson => (
                feet + Vec3::Y * free.eye_first_person,
                look_rotation(forward),
            ),
        }
    }

    /// One tick: picks the camera to show, runs it and blends to it. Returns where the
    /// camera goes and how it turns.
    #[allow(clippy::too_many_arguments)]
    fn tick(
        &mut self,
        hero: &HeroMotion,
        input: &PlayerInput,
        lock: Option<Vec3>,
        targets: &[(Entity, Vec3)],
        volumes: &[(Entity, &CameraVolume, &Transform)],
        profile: &HeroProfile,
        world: &dyn CollisionWorld,
    ) -> (Vec3, Quat) {
        let camera = &profile.camera;
        let setting = self.setting_with(camera);
        let speed = self.speed;
        let start_yaw = self.yaw;
        let state = self
            .state
            .get_or_insert_with(|| CameraState::new(hero, start_yaw, setting, profile));
        let stick = read_axes(input.look_stick, &profile.stick);
        let mouse = input.look_radians();
        let aim = Aim {
            targets,
            velocity: hero.velocity,
            tick: crate::controller::TICK,
            pushed: input.look_stick != [0, 0] || input.move_stick != [0, 0],
            world,
        };
        let looking = hero.state == HeroState::Look;
        state.look_ticks = if looking { state.look_ticks + 1 } else { 0 };
        let blends = &camera.blends;

        // Which camera shows, and which free-look view.
        let view = match setting {
            CameraSetting::ThirdPerson
                if looking
                    && (matches!(state.view, View::Look { .. })
                        || state.look_ticks >= blends.look_lead_ticks) =>
            {
                match state.view {
                    View::Look { ready } => View::Look { ready },
                    _ => View::Look { ready: false },
                }
            }
            CameraSetting::ThirdPerson => View::Follow,
            _ => View::Hero,
        };
        let eye_mode = match setting {
            CameraSetting::ThirdPerson => EyeMode::Off,
            CameraSetting::LockStrafe
                if looking && state.look_ticks >= camera.free_look.first_person_lead_ticks =>
            {
                EyeMode::FirstPerson
            }
            CameraSetting::LockStrafe => EyeMode::LockStrafe,
            CameraSetting::FirstPerson => EyeMode::FirstPerson,
        };
        state.switch(view, eye_mode, hero, profile);
        if state.view == View::Hero
            && eye_mode == EyeMode::FirstPerson
            && setting == CameraSetting::LockStrafe
            && camera.free_look.hold_resets_limits
        {
            // UYA sets first person afresh every tick the button is held.
            state.free.set_limits(&camera.free_look);
        }

        // Run the camera that shows.
        let target = match state.view {
            View::Follow => {
                let steer = looking.then_some((hero.yaw, blends.look_steer));
                let volume = volume::settings(&mut state.volumes, hero.position, volumes);
                let input = FollowInput {
                    stick,
                    mouse,
                    speed,
                    lock,
                    steer,
                    volume,
                };
                let (position, _) = follow::step(&mut state.rig, hero, &input, profile, world);
                (position, turn(state.rig.view))
            }
            View::Look { ready } => {
                if ready {
                    // Once ready, the position blend ends and the camera is at the eye.
                    if let Some(blend) = &mut state.blend {
                        blend.position = 1.0;
                    }
                    let free = &camera.free_look;
                    let stick =
                        look_stick(stick, read_axes(input.move_stick, &profile.stick), free);
                    state.free.turn(stick, mouse, speed, free, Some(&aim));
                }
                state.free.place(&camera.free_look, world)
            }
            View::Hero => {
                state
                    .free
                    .turn(stick, mouse, speed, &camera.free_look, Some(&aim));
                state.free.place(&camera.free_look, world)
            }
        };
        state.free.update_eye(hero, &camera.free_look, world);

        let shown = match state.blend.as_mut().and_then(|blend| blend.apply(target)) {
            Some(shown) => shown,
            None => {
                state.blend = None;
                target
            }
        };
        if let View::Look { ready: false } = state.view
            && state
                .blend
                .as_ref()
                .is_none_or(|blend| blend.position >= 0.5)
        {
            state.view = View::Look { ready: true };
            state.free.set_mode(EyeMode::FirstPerson, &camera.free_look);
        }
        state.shown = shown;
        let (yaw, pitch) = view_angles(target.1 * Vec3::NEG_Z);
        self.yaw = yaw;
        self.pitch = pitch;
        shown
    }
}

impl CameraState {
    fn new(hero: &HeroMotion, yaw: f32, setting: CameraSetting, profile: &HeroProfile) -> Self {
        let rig = Rig::behind(hero, yaw, &profile.camera);
        let mut free = FreeLook::new(hero, yaw, &profile.camera.free_look);
        let view = match setting {
            CameraSetting::ThirdPerson => View::Follow,
            CameraSetting::LockStrafe => {
                free.set_mode(EyeMode::LockStrafe, &profile.camera.free_look);
                View::Hero
            }
            CameraSetting::FirstPerson => {
                free.set_mode(EyeMode::FirstPerson, &profile.camera.free_look);
                View::Hero
            }
        };
        Self {
            shown: (rig.camera, look_rotation(view_direction(yaw, 0.0))),
            rig,
            free,
            view,
            look_ticks: 0,
            blend: None,
            volumes: Vec::new(),
        }
    }

    /// Changes the camera shown to `view`, and the free look to `eye_mode`, starting the
    /// blends the games start (`camera.md`, "Which camera mode shows it").
    fn switch(&mut self, view: View, eye_mode: EyeMode, hero: &HeroMotion, profile: &HeroProfile) {
        let camera = &profile.camera;
        let blends = &camera.blends;
        let free = &camera.free_look;
        let from = self.view;
        let entering_free = from == View::Follow && view != View::Follow;
        if entering_free {
            // The free look starts out looking the way the shown camera does.
            let (yaw, pitch) = view_angles(self.shown.1 * Vec3::NEG_Z);
            self.free.look_along(yaw, pitch, free);
        }
        match (from, view) {
            (View::Follow, View::Look { .. }) => {
                self.free.set_mode(EyeMode::Off, free);
                self.blend = Some(Blend::new(self.shown, blends.into_look));
            }
            (View::Look { .. }, View::Follow) => {
                // Leaving before its own blend is done cuts back.
                self.blend = match self.blend {
                    Some(_) => None,
                    None => Some(Blend::new(self.shown, blends.out_of_look)),
                };
                self.free.set_mode(EyeMode::Off, free);
                let off = wrap(self.rig.arm_yaw - hero.yaw).abs();
                if off >= blends.look_reset_degrees.to_radians() {
                    self.rig = Rig::behind(hero, hero.yaw, camera);
                }
            }
            (View::Follow | View::Look { .. }, View::Hero) => {
                self.free.set_mode(eye_mode, free);
                self.blend = Some(Blend::new(self.shown, blends.into_lock_strafe));
            }
            (View::Hero, View::Follow | View::Look { .. }) => {
                self.free.set_mode(EyeMode::Off, free);
                self.rig = Rig::behind(hero, self.free.yaw, camera);
                self.blend = Some(Blend::new(self.shown, blends.out_of_lock_strafe));
            }
            _ => {}
        }
        if from == View::Hero && view == View::Hero && self.free.mode != eye_mode {
            // First person cuts in; going back to lock-strafe blends.
            self.blend = match eye_mode {
                EyeMode::FirstPerson => None,
                _ => Some(Blend::new(self.shown, blends.out_of_first_person)),
            };
            self.free.set_mode(eye_mode, free);
        }
        self.view = view;
    }
}

/// The free look's stick in the third-person look: each axis the further pushed of the
/// two sticks', and nothing under the threshold (UYA 0x410B08, with free-look flag 0).
fn look_stick(look: Vec2, movement: Vec2, profile: &crate::profile::FreeLookProfile) -> Vec2 {
    let pick = |a: f32, b: f32| if b.abs() > a.abs() { b } else { a };
    let stick = Vec2::new(pick(look.x, movement.x), pick(look.y, movement.y));
    if stick.length() < profile.look_stick_threshold {
        Vec2::ZERO
    } else {
        stick
    }
}

/// A blend of the shown camera from where it was to the camera now running (`camera.md`,
/// "Blending between cameras"): position by `lerp` and rotation by `slerp`, each eased by
/// `(1 − cos πt) / 2`, t moving `rate` a tick.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Blend {
    from: (Vec3, Quat),
    position: f32,
    rotation: f32,
    rate: f32,
}

impl Blend {
    fn new(from: (Vec3, Quat), profile: BlendProfile) -> Self {
        Self {
            from,
            position: profile.start,
            rotation: profile.start,
            rate: profile.rate,
        }
    }

    /// What shows this tick, or `None` once the blend is over: the tick after both
    /// progresses reach 1.
    fn apply(&mut self, to: (Vec3, Quat)) -> Option<(Vec3, Quat)> {
        if self.position >= 1.0 && self.rotation >= 1.0 {
            return None;
        }
        let shown = (
            self.from.0.lerp(to.0, cosine_ease(self.position)),
            self.from.1.slerp(to.1, cosine_ease(self.rotation)),
        );
        self.position = (self.position + self.rate).min(1.0);
        self.rotation = (self.rotation + self.rate).min(1.0);
        Some(shown)
    }
}

fn cosine_ease(t: f32) -> f32 {
    (1.0 - ops::cos(PI * t.clamp(0.0, 1.0))) / 2.0
}

/// The arm with the given yaw, tilt and length.
fn arm(yaw: f32, tilt: f32, length: f32) -> Vec3 {
    let horizontal = ops::cos(tilt) * length;
    Vec3::new(
        ops::sin(yaw) * horizontal,
        ops::sin(tilt) * length,
        ops::cos(yaw) * horizontal,
    )
}

/// The direction a camera with this yaw and pitch looks along.
fn view_direction(yaw: f32, pitch: f32) -> Vec3 {
    -arm(yaw, -pitch, 1.0)
}

fn look_rotation(direction: Vec3) -> Quat {
    turn(view_angles(direction))
}

/// A camera's turn from its yaw and pitch.
fn turn((yaw, pitch): (f32, f32)) -> Quat {
    Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)
}

/// The yaw and pitch of a camera looking along `direction`.
fn view_angles(direction: Vec3) -> (f32, f32) {
    let horizontal = Vec2::new(direction.x, direction.z).length();
    (
        ops::atan2(-direction.x, -direction.z),
        ops::atan2(direction.y, horizontal),
    )
}

fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn approach(value: f32, target: f32, step: f32) -> f32 {
    if value < target {
        (value + step).min(target)
    } else {
        (value - step).max(target)
    }
}

/// One tick of the games' spring: the rate gains `gain × error`, loses `damping × rate`,
/// and never carries the value past the target.
fn spring(value: f32, target: f32, spring: Spring, rate: &mut f32) -> f32 {
    let error = target - value;
    *rate += spring.gain * error - spring.damping * *rate;
    *rate = rate.clamp(-error.abs(), error.abs());
    value + *rate
}

/// [`spring`] for an angle, the short way round.
fn angle_spring(value: f32, target: f32, spring: Spring, rate: &mut f32) -> f32 {
    let error = wrap(target - value);
    *rate += spring.gain * error - spring.damping * *rate;
    *rate = rate.clamp(-error.abs(), error.abs());
    wrap(value + *rate)
}

fn lerp_spring(a: Spring, b: Spring, t: f32) -> Spring {
    Spring {
        gain: a.gain + (b.gain - a.gain) * t,
        damping: a.damping + (b.damping - a.damping) * t,
    }
}

/// The games' speed-up-and-brake mover (UYA 0x445300): `speed` heads for the most that
/// still stops in time, speeding up by `accel` a tick up to `max`, and brakes by `brake`
/// to arrive; a speed the wrong way brakes first.
fn ease_to(value: &mut f32, speed: &mut f32, target: f32, accel: f32, brake: f32, max: f32) {
    let error = target - *value;
    if *speed == 0.0 && error == 0.0 {
        return;
    }
    if *speed * error < 0.0 {
        *speed -= brake.copysign(*speed);
        *value += *speed;
        return;
    }
    let distance = error.abs();
    let stopping = *speed * *speed / brake * 0.5;
    if distance < stopping {
        let rate = if stopping < distance + speed.abs() {
            brake
        } else {
            brake * 1.1
        };
        *speed = approach(*speed, 0.0, rate);
    } else {
        let most = ops::sqrt(2.0 * brake * distance).min(max).copysign(error);
        *speed = approach(*speed, most, accel);
    }
    if distance <= speed.abs() {
        *value = target;
    } else {
        *value += *speed;
    }
}

/// How far a locked-on target turns the arm this tick (UYA 0x3E42C0, 0x3E7168): as a
/// right stick pushed by `2x − x²` of the way, where x is the arm's angle off the line
/// behind the hero from the target in right angles, at a speed that ramps up from nothing.
fn lock_turn(
    arm_yaw: f32,
    from: Vec3,
    target: Vec3,
    lock_ticks: u32,
    profile: &CameraProfile,
) -> f32 {
    let to = target - from;
    if Vec2::new(to.x, to.z).length() < 1e-4 || profile.lock_ramp_ticks == 0 {
        return 0.0;
    }
    let speed = profile.lock_turn * lock_ticks as f32 / profile.lock_ramp_ticks as f32;
    // The camera looks along −arm, so behind the hero means the arm points away from it.
    steer_turn(arm_yaw, ops::atan2(-to.x, -to.z), speed)
}

/// The games' steer (`camera_steer_to`, 0x3E7168): turns a view yaw towards `towards` as a
/// stick pushed `2x − x²` of the way would at `speed`, x being the angle left in right
/// angles.
fn steer_turn(yaw: f32, towards: f32, speed: f32) -> f32 {
    let off = wrap(towards - yaw);
    let x = off.abs() / FRAC_PI_2;
    let push = (2.0 * x - x * x).min(1.0);
    off.signum() * push * speed
}

pub(crate) fn follow(
    inputs: Res<PlayerInputs>,
    tuning: Res<HeroTuning>,
    scene: Res<CollisionScene>,
    heroes: Query<(&Hero, &HeroBody, Option<&LockOn>)>,
    // Disjoint from the cameras, whose `SimTransform`s this system writes.
    targets: Query<crate::TargetItem, Without<FollowCamera>>,
    volumes: Query<(Entity, &CameraVolume, &Transform)>,
    mut cameras: Query<(&mut FollowCamera, &mut SimTransform)>,
) {
    let profile = &tuning.0;
    let targets = crate::aim_targets(&targets);
    let mut volumes: Vec<_> = volumes.iter().collect();
    volumes.sort_by_key(|(entity, ..)| *entity);
    for (mut camera, mut transform) in &mut cameras {
        let Some((_, body, lock)) = heroes
            .iter()
            .find(|(hero, ..)| hero.player == camera.player)
        else {
            continue;
        };
        let lock = lock.and_then(LockOn::target_point);
        let input = inputs.get(camera.player);
        let (position, rotation) =
            camera.tick(&body.0, &input, lock, &targets, &volumes, profile, &scene.0);
        transform.translation = position;
        transform.rotation = rotation;
    }
}

#[cfg(test)]
mod tests;

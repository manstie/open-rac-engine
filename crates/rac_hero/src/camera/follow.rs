//! The third-person follow camera, after UYA's camera mode 0 (`camera.md`, "How the follow
//! camera works" and the sections after it). Deadlocked's is the same with its own
//! distance.
//!
//! It is a drag camera. Each tick:
//! 1. The follow point is the hero's position horizontally, at the height his state's
//!    vertical mode gives ([`super::vertical`]): held through jumps, raised over heli,
//!    double and flip jumps.
//! 2. A look point trails the follow point on springs, more slowly in the air.
//! 3. The arm towards the camera takes up most of the hero's movement, so the camera stays
//!    roughly where it was and swings round to follow, unless he strafes. The player turns
//!    and tilts it; a locked-on target turns it as the stick would.
//! 4. The clearance point keeps the arm's pivot between floor and ceiling, and obstacles
//!    swing the arm round things beside it and pull it in from walls
//!    ([`super::obstacles`]). Its length is the follow distance less that pull-in, the
//!    distance growing while the hero runs at the camera.
//! 5. The camera's actual arm chases that arm on springs.
//! 6. The camera sits at the look point, raised by the follow height, plus the actual arm,
//!    and aims a little above the follow point; while he strafes or is locked on, the aim
//!    moves over to the look point, and back once he stops. A view blocked too long, or a
//!    camera far from where it should be, starts again behind the hero.
//! 7. The view faces the aim across, but its tilt comes from the arm and the heights, so
//!    the follow point's height never tilts it.

use super::{
    angle_spring, approach, arm, lerp_spring, lock_turn, obstacles, spring, steer_turn,
    vertical::{self, Vertical},
    volume::VolumeSettings,
    wrap,
};
use crate::{
    collision::{CapsuleShape, CollisionWorld},
    controller::HeroMotion,
    profile::{CameraProfile, HeroProfile, Spring},
};
use bevy::{math::ops, prelude::*};

/// A camera whose follow point ends up this far from its hero, sideways or vertically,
/// lost him, as after a respawn, and starts again behind him.
const LOST_SIDEWAYS: f32 = 8.0;
const LOST_VERTICALLY: f32 = 20.0;
/// How far short of geometry the kit's last sweep stops the camera.
const SKIN: f32 = 0.01;

/// What the follow camera gets each tick besides the hero.
pub(super) struct FollowInput {
    /// The look stick after the pad's dead zone: x right, y up.
    pub stick: Vec2,
    /// The mouse's turn this tick, radians: x right, y up.
    pub mouse: Vec2,
    /// The camera-speed option.
    pub speed: f32,
    /// Where a locked-on target is.
    pub lock: Option<Vec3>,
    /// Turn to look along this yaw, at up to this many radians a tick: the look's lead-in.
    pub steer: Option<(f32, f32)>,
    pub volume: VolumeSettings,
}

/// The follow camera's state between ticks.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Rig {
    /// The point followed: the hero horizontally, at the height `vertical` gives.
    pub follow: Vec3,
    pub vertical: Vertical,
    pub look: Vec3,
    look_rate: Vec3,
    /// How far the look point's height was from the follow point's last tick.
    look_gap: f32,
    /// Ticks left of the look point's air gain after landing.
    air_ticks: u32,
    /// The dragged arm, from the pivot towards the camera: its yaw (the camera's view yaw,
    /// so 0 puts the camera at +z), its tilt (positive raises the camera), the tilt the
    /// mouse set, and the stick's tilt as a share of the limit.
    pub arm_yaw: f32,
    pub arm_tilt: f32,
    tilt_target: f32,
    stick_tilt: f32,
    pub distance: f32,
    distance_rate: f32,
    /// The distance target, raised while the hero runs at the camera, and the gain it
    /// springs with, which a camera volume lowers.
    distance_target: f32,
    distance_gain: f32,
    height: f32,
    height_rate: f32,
    height_gain: f32,
    look_height: f32,
    look_height_rate: f32,
    /// The arm the camera actually uses, chasing the dragged one.
    actual_yaw: f32,
    actual_yaw_rate: f32,
    actual_tilt: f32,
    actual_tilt_rate: f32,
    actual_length: f32,
    actual_length_rate: f32,
    /// The share of the hero's movement the arm takes up. A camera volume can change it,
    /// and it stays changed until the camera starts again, as in the games.
    drag: f32,
    /// How far obstacles pull the arm in from the distance, and the timer that eases it
    /// back out.
    pub pull_in: f32,
    pub pull_in_timer: u32,
    /// Ticks running that the line from the pivot to the camera has been blocked.
    pub blocked_ticks: u32,
    /// Ticks locked on to a target, up to the profile's ramp.
    lock_ticks: u32,
    /// How far the aim has moved from the follow point to the look point, in ticks of the
    /// profile's `aim_blend_ticks`.
    aim_blend: u32,
    /// Where the camera was and aimed last tick.
    pub camera: Vec3,
    pub aim: Vec3,
    /// The yaw and pitch of the view shown last tick.
    pub view: (f32, f32),
    /// The height and look height as the view's tilt sees them, chasing the rig's.
    view_height: f32,
    view_height_rate: f32,
    view_look_height: f32,
    view_look_height_rate: f32,
    /// How far the view tilts up beyond that with the arm below level.
    look_up: f32,
    look_up_rate: f32,
    /// A floor stopped the camera last tick, so the stick can't tilt it lower.
    pub on_floor: bool,
}

impl Rig {
    /// At rest behind the hero, looking along `yaw`.
    pub fn behind(hero: &HeroMotion, yaw: f32, profile: &CameraProfile) -> Self {
        let follow = hero.position;
        Self {
            follow,
            vertical: Vertical::Follow,
            look: follow,
            look_rate: Vec3::ZERO,
            look_gap: 0.0,
            air_ticks: 0,
            arm_yaw: yaw,
            arm_tilt: 0.0,
            tilt_target: 0.0,
            stick_tilt: 0.0,
            distance: profile.distance,
            distance_rate: 0.0,
            distance_target: profile.distance,
            distance_gain: profile.distance_spring.gain,
            height: profile.height,
            height_rate: 0.0,
            height_gain: profile.height_spring.gain,
            look_height: profile.look_height,
            look_height_rate: 0.0,
            actual_yaw: yaw,
            actual_yaw_rate: 0.0,
            actual_tilt: 0.0,
            actual_tilt_rate: 0.0,
            actual_length: profile.distance,
            actual_length_rate: 0.0,
            drag: profile.drag,
            pull_in: 0.0,
            pull_in_timer: profile.obstacles.pull_in_rest,
            blocked_ticks: 0,
            lock_ticks: 0,
            aim_blend: 0,
            camera: follow + Vec3::Y * profile.height + arm(yaw, 0.0, profile.distance),
            aim: follow + Vec3::Y * profile.look_height,
            view: (
                yaw,
                view_pitch(0.0, profile.height, profile.look_height, profile.distance),
            ),
            view_height: profile.height,
            view_height_rate: 0.0,
            view_look_height: profile.look_height,
            view_look_height_rate: 0.0,
            look_up: 0.0,
            look_up_rate: 0.0,
            on_floor: false,
        }
    }

    /// The games' reset (`camera_reset_follow`, 0x3F3918): behind the hero along his
    /// facing, stopping short of anything in the way.
    pub fn reset(hero: &HeroMotion, profile: &CameraProfile, world: &dyn CollisionWorld) -> Self {
        let mut rig = Self::behind(hero, hero.yaw, profile);
        let pivot = rig.follow + Vec3::Y * profile.height;
        let reach = rig.camera - pivot;
        if let Some(hit) = world.cast_ray(pivot, reach) {
            let length = (hit.fraction * profile.distance - profile.obstacles.reset_skin)
                .max(profile.obstacles.min_arm_wall);
            rig.pull_in = profile.distance - length;
            rig.actual_length = length;
            rig.camera = pivot + reach * (length / profile.distance);
            rig.view.1 = view_pitch(0.0, profile.height, profile.look_height, length);
        }
        rig
    }

    /// The dragged arm's length.
    pub fn arm_length(&self) -> f32 {
        self.distance - self.pull_in
    }
}

/// Advances the rig one tick and returns where the camera goes and where it aims.
pub(super) fn step(
    rig: &mut Rig,
    hero: &HeroMotion,
    input: &FollowInput,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) -> (Vec3, Vec3) {
    let camera = &profile.camera;
    let lost = {
        let away = hero.position - rig.follow;
        Vec2::new(away.x, away.z).length() > LOST_SIDEWAYS || away.y.abs() > LOST_VERTICALLY
    };
    if lost {
        *rig = Rig::behind(hero, rig.arm_yaw, camera);
    }

    // This tick's settings (`camera_follow_params`): strafing with the buttons held stops
    // the drag; strafing and lock-on stiffen the look point and move the aim to it; a
    // camera volume retunes the rest.
    let volume = &input.volume;
    if let Some(drag) = volume.drag {
        rig.drag = drag;
    }
    let strafing = hero.strafing;
    let locked = input.lock.is_some();
    let drag = if strafing { 0.0 } else { rig.drag };
    let look_spring = if strafing {
        camera.strafe_look_spring
    } else if locked {
        camera.lock_look_spring
    } else {
        camera.look_spring
    };
    // The aim moves a tick at a time, so toggling strafe doesn't jerk the view between
    // the two points.
    rig.aim_blend = if strafing || locked {
        (rig.aim_blend + 1).min(camera.aim_blend_ticks)
    } else {
        rig.aim_blend.saturating_sub(1)
    };
    let mut distance = camera.distance;
    let mut push_back = true;
    if let Some(wanted) = volume.distance {
        distance = wanted;
        rig.distance_gain = camera.volume_gain;
        push_back = (camera.distance..=camera.approach_distance).contains(&wanted);
    }
    let mut height_target = camera.height;
    if let Some(wanted) = volume.height {
        height_target = wanted;
        rig.height_gain = camera.volume_gain;
    }
    let mut height_gain = rig.height_gain;
    let look_height_target = volume.look_height.unwrap_or(camera.look_height);

    // 1. The follow point.
    let previous_follow = rig.follow;
    let height = vertical::step(rig, hero, input.stick, profile, world);
    rig.follow = Vec3::new(hero.position.x, height, hero.position.z);

    // 2. The look point trails it, its height barely moving for small gaps in the air.
    let air = if hero.ground.is_none() {
        rig.air_ticks = camera.vertical.air_after_land_ticks;
        true
    } else if rig.air_ticks > 0 {
        rig.air_ticks -= 1;
        true
    } else {
        false
    };
    let vertical_spring = if air {
        let v = &camera.vertical;
        Spring {
            gain: v.air_gain + v.air_gain_gap * rig.look_gap.min(v.air_gap) / v.air_gap,
            damping: camera.look_spring_vertical.damping,
        }
    } else {
        camera.look_spring_vertical
    };
    rig.look.x = spring(rig.look.x, rig.follow.x, look_spring, &mut rig.look_rate.x);
    rig.look.z = spring(rig.look.z, rig.follow.z, look_spring, &mut rig.look_rate.z);
    rig.look.y = spring(
        rig.look.y,
        rig.follow.y,
        vertical_spring,
        &mut rig.look_rate.y,
    );
    rig.look_gap = (rig.look.y - rig.follow.y).abs();

    // 3. The arm takes up most of the movement, then the player turns and tilts it.
    let length = rig.arm_length();
    let dragged = arm(rig.arm_yaw, rig.arm_tilt, length) - drag * (rig.follow - previous_follow);
    let horizontal = Vec2::new(dragged.x, dragged.z).length();
    if horizontal > 1e-4 {
        rig.arm_yaw = ops::atan2(dragged.x, dragged.z);
    }
    let tilt = ops::atan2(dragged.y, horizontal);
    // A dragged arm that comes out longer takes back some of the pull-in.
    let dragged_length = dragged.length();
    if dragged_length > length + 1e-4 {
        rig.pull_in = (rig.distance - dragged_length).max(0.0);
    }
    let turn_rate = camera.stick_turn + camera.stick_turn_per_speed * input.speed / 100.0;
    let turn = turn_rate * input.stick.x + input.mouse.x;
    // Turning right lowers the yaw.
    rig.arm_yaw = wrap(rig.arm_yaw - turn);
    rig.lock_ticks = match input.lock {
        Some(_) => (rig.lock_ticks + 1).min(camera.lock_ramp_ticks),
        None => 0,
    };
    // A target or the look's lead-in steer like the stick, while the stick rests.
    if turn == 0.0 {
        if let Some(target) = input.lock {
            rig.arm_yaw = wrap(
                rig.arm_yaw + lock_turn(rig.arm_yaw, rig.follow, target, rig.lock_ticks, camera),
            );
        } else if let Some((yaw, speed)) = input.steer {
            rig.arm_yaw = wrap(rig.arm_yaw + steer_turn(rig.arm_yaw, yaw, speed));
        }
    }
    rig.arm_tilt = tilt_arm(rig, tilt, input, camera);

    // 4. The clearance point, which takes the height under a low ceiling.
    let clearance = obstacles::clearance(hero, rig.follow, rig.height, camera, world);
    if let Some(wanted) = clearance.height {
        height_target = wanted;
        height_gain = camera.clearance.height_gain;
    }
    let pivot = clearance.point;

    // 5. Running at the camera pushes its distance out.
    let motion = Vec3::new(hero.velocity.x, 0.0, hero.velocity.z);
    let speed = motion.length();
    let view = -arm(rig.arm_yaw, 0.0, 1.0);
    let approaching =
        if push_back && speed > 1e-3 && motion.dot(view) / speed < camera.approach_facing {
            (speed / camera.approach_full_speed).min(1.0)
        } else {
            0.0
        };
    if approaching > 0.0 {
        let target = distance + (camera.approach_distance - distance) * approaching;
        rig.distance_target = rig.distance_target.max(target);
        rig.distance_gain = camera.distance_spring.gain;
    } else {
        rig.distance_target = distance;
    }

    // 6. Obstacles turn the arm and pull it in.
    obstacles::avoid(rig, pivot, input.stick, camera, world);

    // 7. The actual arm chases the dragged one.
    rig.actual_yaw = angle_spring(
        rig.actual_yaw,
        rig.arm_yaw,
        camera.arm_spring,
        &mut rig.actual_yaw_rate,
    );
    rig.actual_tilt = angle_spring(
        rig.actual_tilt,
        rig.arm_tilt,
        camera.arm_spring,
        &mut rig.actual_tilt_rate,
    );
    let length_spring = lerp_spring(
        camera.arm_length_spring,
        camera.arm_length_spring_approach,
        approaching,
    );
    rig.actual_length = spring(
        rig.actual_length,
        rig.arm_length(),
        length_spring,
        &mut rig.actual_length_rate,
    );

    // The distance and heights settle at the end of the tick, as in the game.
    let distance_spring = Spring {
        gain: rig.distance_gain,
        ..camera.distance_spring
    };
    rig.distance = spring(
        rig.distance,
        rig.distance_target,
        distance_spring,
        &mut rig.distance_rate,
    );
    let height_spring = Spring {
        gain: height_gain,
        ..camera.height_spring
    };
    rig.height = spring(
        rig.height,
        height_target,
        height_spring,
        &mut rig.height_rate,
    );
    rig.look_height = spring(
        rig.look_height,
        look_height_target,
        camera.look_height_spring,
        &mut rig.look_height_rate,
    );

    // 8. Place the camera: pushed out of geometry, the look point moving with it.
    let wanted =
        rig.look + Vec3::Y * rig.height + arm(rig.actual_yaw, rig.actual_tilt, rig.actual_length);
    let ball = CapsuleShape {
        half_height: 0.0,
        radius: camera.collision_radius,
    };
    let mut position = wanted;
    for _ in 0..3 {
        let deepest = world
            .capsule_penetrations(ball, position)
            .into_iter()
            .max_by(|a, b| a.depth.total_cmp(&b.depth));
        let Some(push) = deepest else {
            break;
        };
        position += push.normal * push.depth;
    }
    rig.look += position - wanted;
    // The kit also keeps the camera on the hero's side of any wall [simplified].
    let reach = position - pivot;
    if let Some(hit) = world.cast_sphere(camera.collision_radius, pivot, reach) {
        let length = reach.length();
        position = pivot + reach / length * (hit.fraction * length - SKIN).max(0.0);
    }
    let blend = match camera.aim_blend_ticks {
        0 if strafing || locked => 1.0,
        0 => 0.0,
        ticks => rig.aim_blend as f32 / ticks as f32,
    };
    let aim = rig.follow.lerp(rig.look, blend) + Vec3::Y * rig.look_height;

    // 9. A view blocked for long, or a camera far from its spot, starts again.
    let spot = pivot + arm(rig.arm_yaw, rig.arm_tilt, rig.arm_length());
    rig.blocked_ticks = match world.cast_ray(pivot, spot - pivot) {
        Some(_) => rig.blocked_ticks + 1,
        None => 0,
    };
    let o = &camera.obstacles;
    if rig.blocked_ticks >= o.reset_blocked_ticks || position.distance(spot) > o.reset_distance {
        *rig = Rig::reset(hero, camera, world);
        return (rig.camera, rig.aim);
    }
    rig.camera = position;
    rig.aim = aim;

    // 10. The view.
    turn_view(rig, hero, position, aim, camera);
    (position, aim)
}

/// The view this tick (`camera_orient`, 0x3E7B50): it turns to face the aim across, but
/// tilts as if the aim were the look height above the arm's pivot, from the arm's rise
/// and the heights on a slower spring of their own. So the follow point's height, which
/// snaps when a jump's hold ends in a glide, never tilts the view. With the dragged arm
/// below level on the ground it tilts further up, the most half way to the pitch limit.
fn turn_view(rig: &mut Rig, hero: &HeroMotion, position: Vec3, aim: Vec3, camera: &CameraProfile) {
    let view = &camera.view;
    let across = Vec2::new(aim.x - position.x, aim.z - position.z);
    let distance = across.length();
    if distance >= view.min_across {
        rig.view.0 = ops::atan2(-across.x, -across.y);
    }
    rig.view_height = spring(
        rig.view_height,
        rig.height,
        view.height_spring,
        &mut rig.view_height_rate,
    );
    rig.view_look_height = spring(
        rig.view_look_height,
        rig.look_height,
        view.height_spring,
        &mut rig.view_look_height_rate,
    );
    let rise = ops::sin(rig.actual_tilt) * rig.actual_length;
    let pitch = view_pitch(rise, rig.view_height, rig.view_look_height, distance);

    // The games take the dragged arm's angle as atan2(rise, length).
    let below = -ops::atan(ops::sin(rig.arm_tilt)) / camera.pitch_limit_degrees.to_radians();
    let look_up = if hero.ground.is_some() && below > view.look_up_from {
        2.0 * below.min(1.0 - below) * view.look_up_peak_degrees.to_radians()
    } else {
        0.0
    };
    rig.look_up = angle_spring(
        rig.look_up,
        look_up,
        view.look_up_spring,
        &mut rig.look_up_rate,
    );
    let cap = view.pitch_cap_degrees.to_radians();
    rig.view.1 = (pitch + rig.look_up).clamp(-cap, cap);
}

/// The view's tilt with the camera `across` from the aim and the arm rising `rise` from
/// its pivot.
fn view_pitch(rise: f32, height: f32, look_height: f32, across: f32) -> f32 {
    ops::atan2(-(rise + height - look_height), across)
}

/// The arm's tilt this tick (0x3E7668): towards the tilt the mouse set (or a camera
/// volume's while the stick rests) plus the stick's, by at most `pitch_step`.
fn tilt_arm(rig: &mut Rig, tilt: f32, input: &FollowInput, camera: &CameraProfile) -> f32 {
    let limit = camera.pitch_limit_degrees.to_radians();
    // Looking up lowers the camera.
    rig.tilt_target = (rig.tilt_target - input.mouse.y).clamp(-limit, limit);
    let pushed = ((input.stick.y.abs() - camera.stick_tilt_dead_zone)
        / (1.0 - camera.stick_tilt_dead_zone))
        .clamp(0.0, 1.0);
    let share = if pushed > 0.0 {
        -pushed.copysign(input.stick.y)
    } else {
        0.0
    };
    let outward = share.abs() > rig.stick_tilt.abs() && share * rig.stick_tilt >= 0.0;
    let cap = if outward {
        camera.stick_tilt_out
    } else {
        camera.stick_tilt_back
    };
    let before = rig.stick_tilt;
    rig.stick_tilt = approach(before, share, cap);
    // A floor under the camera stops the stick lowering it further.
    if rig.on_floor && rig.stick_tilt < before {
        rig.stick_tilt = before;
    }
    let base = match input.volume.tilt {
        Some(volume) if pushed == 0.0 => volume,
        _ => rig.tilt_target,
    };
    let wanted = (base + rig.stick_tilt * limit).clamp(-limit, limit);
    approach(tilt, wanted, camera.pitch_step)
}

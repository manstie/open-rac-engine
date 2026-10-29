//! The camera's shake (`effects.md`, "Lights, shake, rumble and the screen"): the game's
//! three channels at 0x1E2AE0, each an amplitude and frames left, moving the camera along
//! its up, along its view or rolling it (`camera_shake_apply` 0x3F2A98). A gun's code sets
//! a channel outright, the latest setting winning.

use bevy::prelude::*;
use rac_hero::FollowCamera;
use serde::Deserialize;

/// Which way a channel moves the camera.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Shaking {
    /// Channel 0, which the guns set: up and down along the camera's up.
    #[default]
    Up,
    /// Channel 1: in and out along its view.
    Forward,
    /// Channel 2: a roll about its view.
    Roll,
}

impl Shaking {
    fn channel(self) -> usize {
        match self {
            Shaking::Up => 0,
            Shaking::Forward => 1,
            Shaking::Roll => 2,
        }
    }
}

/// One channel: its amplitude and its ticks left of the most it had.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct Channel {
    amplitude: f32,
    left: u32,
    total: u32,
    /// What it moves the camera by this tick: units, or radians for the roll.
    now: f32,
}

/// The camera's shake now, for every follow camera.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct CameraShake([Channel; 3]);

impl CameraShake {
    /// Sets the `along` channel to `amplitude` for `ticks`, in place of what it had.
    pub fn set(&mut self, along: Shaking, amplitude: f32, ticks: u32) {
        self.0[along.channel()] = Channel {
            amplitude,
            left: ticks,
            total: 0,
            now: 0.0,
        };
    }

    /// What `along` moves the camera by this tick.
    pub fn now(&self, along: Shaking) -> f32 {
        self.0[along.channel()].now
    }

    /// A tick on, as 0x3F2A98 takes each channel a frame: the ticks left `l` of the most
    /// `t` it had, up and in move by the amplitude × cos(2l) × (l/t)², and the roll by the
    /// amplitude × sin(2001° × s − 210°) × (l/t)³, `s` the seconds since it was set, eased
    /// in over its first sixth of a second.
    pub(crate) fn step(&mut self) {
        for (index, channel) in self.0.iter_mut().enumerate() {
            if channel.left == 0 {
                channel.total = 0;
                channel.now = 0.0;
                continue;
            }
            channel.total = channel.total.max(channel.left);
            channel.left -= 1;
            let left = channel.left as f32;
            let share = left / channel.total as f32;
            channel.now = if index < 2 {
                channel.amplitude * ops::cos(2.0 * left) * share * share
            } else {
                let seconds = (channel.total - channel.left) as f32 / 60.0;
                // 10 × 2001° is a whole number of turns and 210° more.
                let angle = (seconds * 2001.0 - 20010.0).to_radians();
                let ease = (seconds * 6.0).min(1.0);
                channel.amplitude * ops::sin(angle) * share * share * share * ease
            };
        }
    }
}

/// What a shake of `amplitude` set `far` from the camera sets: less `lose` a unit of the
/// way, and from `within` of it `beyond` (none: the shake is left as it was), as the
/// Nitro's blast (0x35E718) and the shared explosion (0x32FE90) measure from the camera.
pub(crate) fn amplitude_at(
    amplitude: f32,
    lose: f32,
    within: Option<f32>,
    beyond: Option<f32>,
    far: f32,
) -> Option<f32> {
    match within {
        Some(within) if far >= within => beyond,
        _ => Some((amplitude - lose * far).max(0.0)),
    }
}

/// Steps the shake a tick.
pub(crate) fn step_shake(mut shake: ResMut<CameraShake>) {
    shake.step();
}

/// Moves each follow camera by the shake, after the simulation's place for this frame
/// has been put in its transform.
pub(crate) fn shake_cameras(
    shake: Res<CameraShake>,
    mut cameras: Query<&mut Transform, With<FollowCamera>>,
) {
    let (up, forward, roll) = (
        shake.now(Shaking::Up),
        shake.now(Shaking::Forward),
        shake.now(Shaking::Roll),
    );
    if up == 0.0 && forward == 0.0 && roll == 0.0 {
        return;
    }
    for mut transform in &mut cameras {
        let turn = transform.rotation;
        transform.translation += turn * Vec3::Y * up + turn * Vec3::NEG_Z * forward;
        transform.rotation = turn * Quat::from_rotation_z(roll);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shake_rattles_and_dies_away_over_its_ticks() {
        let mut shake = CameraShake::default();
        shake.set(Shaking::Up, 0.4, 25);
        let mut moves = Vec::new();
        for _ in 0..30 {
            shake.step();
            moves.push(shake.now(Shaking::Up));
        }
        // cos(48) × (24/25)² × 0.4 first, shrinking with the square of the time left.
        let first = 0.4 * ops::cos(48.0) * (24.0f32 / 25.0).powi(2);
        assert!((moves[0] - first).abs() < 1e-6, "{}", moves[0]);
        assert!(moves.iter().all(|now| now.abs() <= 0.4));
        assert!(moves[..12].iter().any(|now| *now > 0.1));
        assert!(moves[..12].iter().any(|now| *now < -0.1));
        assert!(moves[24..].iter().all(|now| *now == 0.0));
        assert_eq!(shake.now(Shaking::Forward), 0.0);
    }

    #[test]
    fn a_new_shake_takes_the_channels_place() {
        let mut shake = CameraShake::default();
        shake.set(Shaking::Up, 0.4, 25);
        (0..20).for_each(|_| shake.step());
        shake.set(Shaking::Up, 0.04, 30);
        shake.step();
        assert!(shake.now(Shaking::Up).abs() <= 0.04);
    }

    #[test]
    fn a_blast_shakes_the_camera_less_the_further_it_is() {
        // The Nitro's: 0.4 − 0.02 a unit within 20, and nothing from there.
        let nitro = |far| amplitude_at(0.4, 0.02, Some(20.0), Some(0.0), far);
        assert_eq!(nitro(0.0), Some(0.4));
        assert!((nitro(10.0).unwrap() - 0.2).abs() < 1e-6);
        assert_eq!(nitro(25.0), Some(0.0));
        // The shared explosion's: the same within 48, and leaves the shake past it.
        assert_eq!(
            amplitude_at(0.075, 0.0, Some(48.0), None, 30.0),
            Some(0.075)
        );
        assert_eq!(amplitude_at(0.075, 0.0, Some(48.0), None, 60.0), None);
    }

    #[test]
    fn the_shake_moves_the_camera_from_where_the_simulation_put_it() {
        let mut shake = CameraShake::default();
        shake.set(Shaking::Up, 0.4, 25);
        shake.step();
        let up = shake.now(Shaking::Up);
        assert!(up != 0.0);
        let mut app = App::new();
        app.insert_resource(shake)
            .add_systems(Update, shake_cameras);
        let looking = Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);
        let camera = app
            .world_mut()
            .spawn((FollowCamera::for_player(0, 0.0), looking))
            .id();
        app.update();
        let moved = app.world().get::<Transform>(camera).unwrap();
        let along = looking.rotation * Vec3::Y * up;
        assert!((moved.translation - (looking.translation + along)).length() < 1e-5);
        assert_eq!(moved.rotation, looking.rotation);
    }

    #[test]
    fn a_roll_eases_in() {
        let mut shake = CameraShake::default();
        shake.set(Shaking::Roll, 0.15, 25);
        shake.step();
        let first = shake.now(Shaking::Roll).abs();
        // A sixth of the way in at most, after one tick of ten.
        assert!(first <= 0.15 / 10.0 + 1e-6, "{first}");
    }
}

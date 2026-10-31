//! A moby's own sequences as the games play them (`moby_anim_advance`, 0x4389D8): one
//! sequence at a time, looping and saying so as it wraps, changed by a cut or a blend from
//! where it is to the new sequence's first pose (0x441BA0, `moby_anim_blend` 0x441C78),
//! each resetting its speed to 1. The gun in his hand plays them ([`crate::gun_anim`]), and
//! so do the things guns send out that have a skeleton of their own.

use std::collections::BTreeMap;

use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;

use crate::clips::{RigClip, Sample, mix};
use crate::wear::rest_from_binds;

/// A moby's skeleton and its sequences, for something that plays them on its own.
#[derive(Debug, Clone, PartialEq)]
pub struct MobySkeleton {
    /// Each joint's parent, a parent before its children.
    pub parents: Vec<Option<usize>>,
    /// Each joint's transform from its parent at rest.
    pub rest: Vec<Transform>,
    pub inverse_bindposes: Handle<SkinnedMeshInverseBindposes>,
    pub clips: BTreeMap<u8, RigClip>,
}

impl MobySkeleton {
    /// From where each joint sits at rest from the origin.
    pub fn new(
        parents: Vec<Option<usize>>,
        binds: &[Mat4],
        clips: BTreeMap<u8, RigClip>,
        bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
    ) -> Self {
        Self {
            rest: rest_from_binds(binds, &parents),
            parents,
            inverse_bindposes: bindposes.add(binds.iter().map(Mat4::inverse).collect::<Vec<_>>()),
            clips,
        }
    }

    /// Spawns its joints under `origin`, at rest.
    pub fn spawn_joints(&self, commands: &mut Commands, origin: Entity) -> Vec<Entity> {
        let mut joints: Vec<Entity> = Vec::with_capacity(self.rest.len());
        for (rest, parent) in self.rest.iter().zip(&self.parents) {
            let joint = commands.spawn((*rest, Visibility::default())).id();
            let parent = parent.map_or(origin, |parent| joints[parent]);
            commands.entity(parent).add_child(joint);
            joints.push(joint);
        }
        joints
    }

    /// `joint`'s transform in `pose`, or at rest without one.
    pub fn local(&self, pose: Option<&Sample>, joint: usize) -> Transform {
        match pose {
            Some((rotations, translations, scales)) if joint < rotations.len() => Transform {
                translation: translations[joint],
                rotation: rotations[joint],
                scale: scales[joint],
            },
            _ => self.rest[joint],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Blend {
    from: Sample,
    length: f32,
    done: f32,
}

/// Where a moby's animation is.
#[derive(Debug, Clone, PartialEq)]
pub struct SequencePlayer {
    sequence: u8,
    /// Ticks into the sequence's loop.
    ticks: f32,
    speed: f32,
    blend: Option<Blend>,
    /// The sequence came round on the last tick (bit 0x02 of the moby's +0x60).
    wrapped: bool,
    /// Its animation frame on the last tick, for [`Self::reached`].
    last_frame: f32,
    /// A fraction of a tick not yet played.
    pending: f32,
}

impl SequencePlayer {
    /// A moby just made, playing `sequence` from its start at speed 1.
    pub fn new(sequence: u8) -> Self {
        Self {
            sequence,
            ticks: 0.0,
            speed: 1.0,
            blend: None,
            wrapped: false,
            last_frame: 0.0,
            pending: 0.0,
        }
    }

    pub fn sequence(&self) -> u8 {
        self.sequence
    }

    /// It came round on the last tick.
    pub fn wrapped(&self) -> bool {
        self.wrapped
    }

    pub fn blending(&self) -> bool {
        self.blend.is_some()
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// How fast it plays from here on: negative plays it backwards (+0x48).
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    #[cfg(test)]
    pub(crate) fn ticks(&self) -> f32 {
        self.ticks
    }

    /// Its animation frame now: what the games time things by (0x436500).
    pub fn frame(&self, clips: &BTreeMap<u8, RigClip>) -> f32 {
        clips
            .get(&self.sequence)
            .map_or(0.0, |clip| clip.frame_at(self.ticks))
    }

    /// Whether it passed `frame` on the last tick (`anim_frame_reached`, 0x44AF40); never
    /// while it blends.
    pub fn reached(&self, clips: &BTreeMap<u8, RigClip>, frame: f32) -> bool {
        if self.blend.is_some() {
            return false;
        }
        let now = self.frame(clips);
        if self.speed < 0.0 {
            now <= frame && frame < self.last_frame
        } else {
            self.last_frame < frame && frame <= now
        }
    }

    /// Plays `sequence` from its start, blending from where it is over `blend` ticks (a
    /// cut for none), at speed 1.
    pub fn play(&mut self, clips: &BTreeMap<u8, RigClip>, sequence: u8, blend: f32) {
        self.play_from(clips, sequence, 0, blend);
    }

    /// The same from its keyframe `keyframe`.
    pub fn play_from(
        &mut self,
        clips: &BTreeMap<u8, RigClip>,
        sequence: u8,
        keyframe: usize,
        blend: f32,
    ) {
        let from = self.pose(clips);
        self.sequence = sequence;
        self.ticks = clips
            .get(&sequence)
            .map_or(0.0, |clip| clip.ticks_to(keyframe));
        self.speed = 1.0;
        self.last_frame = self.frame(clips);
        self.blend = from.filter(|_| blend > 0.0).map(|from| Blend {
            from,
            length: blend,
            done: 0.0,
        });
    }

    /// Moves on by a tick. A blend holds the sequence at its start until it is done.
    pub fn tick(&mut self, clips: &BTreeMap<u8, RigClip>) {
        self.wrapped = false;
        self.last_frame = self.frame(clips);
        match &mut self.blend {
            Some(blend) => {
                blend.done += 1.0;
                if blend.done >= blend.length {
                    self.ticks += blend.done - blend.length;
                    self.blend = None;
                }
            }
            None => {
                self.ticks += self.speed;
                let cycle = clips.get(&self.sequence).map_or(0.0, RigClip::cycle);
                if cycle > 0.0 && !(0.0..cycle).contains(&self.ticks) {
                    self.ticks = self.ticks.rem_euclid(cycle);
                    self.wrapped = true;
                }
            }
        }
    }

    /// Moves on by `seconds` of the 60 Hz ticks whole, with `each` called before each one,
    /// and returns the ticks played.
    pub fn step(
        &mut self,
        clips: &BTreeMap<u8, RigClip>,
        seconds: f32,
        mut each: impl FnMut(&mut Self),
    ) -> u32 {
        self.pending += seconds * 60.0;
        let whole = self.pending.floor();
        self.pending -= whole;
        for _ in 0..whole as u32 {
            each(self);
            self.tick(clips);
        }
        whole as u32
    }

    /// Its pose now, if it has a clip for where it is.
    pub fn pose(&self, clips: &BTreeMap<u8, RigClip>) -> Option<Sample> {
        let pose = clips.get(&self.sequence)?.at_time(self.ticks)?;
        Some(match &self.blend {
            Some(blend) => mix(&blend.from, &pose, (blend.done / blend.length).min(1.0)),
            None => pose,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clips::RigKeyframe;

    /// A one-joint looping clip of `keyframes` keyframes 2 ticks and 2 frames apart.
    fn clip(keyframes: usize) -> RigClip {
        RigClip {
            keyframes: (0..keyframes)
                .map(|i| RigKeyframe {
                    ticks: 2.0,
                    frame: 2.0 * i as f32,
                    rotations: vec![Quat::from_rotation_y(i as f32 * 0.1)],
                    offsets: vec![Vec3::ZERO],
                    scales: vec![Vec3::ONE],
                })
                .collect(),
            looping: true,
            start: 0,
        }
    }

    #[test]
    fn it_loops_saying_so_and_backwards_too() {
        let clips = BTreeMap::from([(0, clip(4))]);
        let mut player = SequencePlayer::new(0);
        for tick in 1..=8 {
            player.tick(&clips);
            assert_eq!(player.wrapped(), tick == 8, "{tick}");
        }
        player.set_speed(-1.0);
        player.tick(&clips);
        assert!(player.wrapped(), "going back past its start");
    }

    #[test]
    fn a_blend_holds_the_new_sequence_at_its_start_and_resets_the_speed() {
        let clips = BTreeMap::from([(0, clip(4)), (1, clip(2))]);
        let mut player = SequencePlayer::new(0);
        player.set_speed(0.5);
        player.play(&clips, 1, 3.0);
        assert_eq!(player.speed(), 1.0);
        assert!(player.blending());
        player.tick(&clips);
        player.tick(&clips);
        assert!(player.blending());
        assert_eq!(player.frame(&clips), 0.0);
        player.tick(&clips);
        assert!(!player.blending());
    }

    #[test]
    fn it_says_when_it_passes_a_frame() {
        let clips = BTreeMap::from([(0, clip(8))]);
        let mut player = SequencePlayer::new(0);
        let mut seen = Vec::new();
        for tick in 1..=8 {
            player.tick(&clips);
            if player.reached(&clips, 3.0) {
                seen.push(tick);
            }
        }
        assert_eq!(seen, [3]);
    }

    #[test]
    fn it_can_start_part_way_in_and_step_whole_ticks() {
        let clips = BTreeMap::from([(0, clip(8))]);
        let mut player = SequencePlayer::new(0);
        player.play_from(&clips, 0, 3, 0.0);
        assert_eq!(player.frame(&clips), 6.0);
        let mut calls = 0;
        let played = player.step(&clips, 2.5 / 60.0, |_| calls += 1);
        assert_eq!((played, calls), (2, 2));
        assert_eq!(player.frame(&clips), 8.0);
    }
}

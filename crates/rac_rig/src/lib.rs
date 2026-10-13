//! The hero kit's placeholder model (PLAN.md P1.17): a blocky, flat-shaded gold Ratchet
//! with Clank on his back and the wrench in his hand, built and animated in code.
//!
//! - [`model`] describes it: a skeleton laid out like UYA's hero, and blocks that are each
//!   the convex hull of a few points, modelled by hand with UYA's hero as the reference
//!   (`docs/research/moby-format.md`). Nothing in it comes from a disc.
//! - [`spawn_rig`] builds it under a hero entity; [`RigPlugin`] poses it every frame from
//!   the hero's state, and flies a copy of the wrench while it is thrown.
//! - A game with real animation inserts [`RigAnimations`]: clips for some [`RigMove`]s,
//!   already on the rig's joints. Those moves play their clip; the rest stay procedural.
//!
//! The rig only reads the controller ([`rac_hero::HeroBody`]), so it can be swapped for
//! another model, or the disc's own (P3.15), without touching the simulation.

mod clips;
mod hull;
pub mod model;
mod pose;
mod shapes;

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use rac_hero::HeroBody;

pub use clips::{FlipSide, RigAnimations, RigClip, RigKeyframe, RigMove};
pub use hull::convex_hull;
pub use model::{JointDef, Look, ModelDef, PartDef, joint};
pub use pose::Pose;

/// Poses every [`HeroRig`] from its hero each frame.
pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (animate, fly_thrown_wrenches));
    }
}

/// The colours of one hero's model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigLook {
    pub main: Color,
    pub accent: Color,
}

impl RigLook {
    /// Gold for player 1, silver for player 2 (P1.17's player colours).
    pub fn for_player(player: usize) -> Self {
        if player.is_multiple_of(2) {
            Self {
                main: Color::srgb(1.0, 0.78, 0.34),
                accent: Color::srgb(0.45, 0.30, 0.10),
            }
        } else {
            Self {
                main: Color::srgb(0.86, 0.88, 0.92),
                accent: Color::srgb(0.25, 0.28, 0.34),
            }
        }
    }

    fn material(&self, look: Look) -> StandardMaterial {
        let (color, roughness) = match look {
            Look::Main => (self.main, 0.32),
            Look::Accent => (self.accent, 0.45),
        };
        StandardMaterial {
            base_color: color,
            metallic: 1.0,
            perceptual_roughness: roughness,
            ..default()
        }
    }
}

/// A hero's model, under the hero entity. Its joints are child entities in the order of
/// [`model::joint`].
#[derive(Component, Debug)]
pub struct HeroRig {
    pub hero: Entity,
    pub joints: Vec<Entity>,
    skeleton: Vec<JointDef>,
    animator: pose::Animator,
}

/// One joint of a [`HeroRig`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RigJoint(pub usize);

/// The copy of the wrench that flies while it is thrown.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrownWrenchModel {
    pub hero: Entity,
}

/// A flat-shaded mesh of triangles: every corner carries its face's normal.
pub fn flat_mesh(triangles: &[[Vec3; 3]]) -> Mesh {
    let mut positions = Vec::with_capacity(triangles.len() * 3);
    let mut normals = Vec::with_capacity(triangles.len() * 3);
    for [a, b, c] in triangles {
        let normal = (*b - *a).cross(*c - *a).normalize_or_zero();
        for corner in [a, b, c] {
            positions.push(corner.to_array());
            normals.push(normal.to_array());
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
}

/// The model's blocks for each joint and look, in the joint's space.
fn joint_meshes(model: &ModelDef, offset: Vec3) -> BTreeMap<(usize, Look), Vec<[Vec3; 3]>> {
    let mut meshes: BTreeMap<(usize, Look), Vec<[Vec3; 3]>> = BTreeMap::new();
    for part in &model.parts {
        let points: Vec<Vec3> = model
            .local_points(part)
            .into_iter()
            .map(|p| p - offset)
            .collect();
        meshes
            .entry((part.joint, part.look))
            .or_default()
            .extend(convex_hull(&points));
    }
    meshes
}

/// Builds the placeholder model under `hero`, and the wrench it throws, and returns the
/// model's root.
pub fn spawn_rig(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    hero: Entity,
    look: RigLook,
) -> Entity {
    let model = model::hero();
    let main = materials.add(look.material(Look::Main));
    let accent = materials.add(look.material(Look::Accent));
    let material = |look: Look| match look {
        Look::Main => main.clone(),
        Look::Accent => accent.clone(),
    };

    let root = commands
        .spawn((
            Name::new("hero model"),
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    commands.entity(hero).add_child(root);

    let mut joints: Vec<Entity> = Vec::with_capacity(model.joints.len());
    for (index, joint) in model.joints.iter().enumerate() {
        let parent_rest = joint.parent.map_or(Vec3::ZERO, |p| model.joints[p].rest);
        let entity = commands
            .spawn((
                Name::new(joint.name),
                RigJoint(index),
                Transform::from_translation(joint.rest - parent_rest),
                Visibility::default(),
            ))
            .id();
        let parent = joint.parent.map_or(root, |p| joints[p]);
        commands.entity(parent).add_child(entity);
        joints.push(entity);
    }

    for ((joint, look), triangles) in joint_meshes(&model, Vec3::ZERO) {
        let part = commands
            .spawn((
                Mesh3d(meshes.add(flat_mesh(&triangles))),
                MeshMaterial3d(material(look)),
                Transform::default(),
            ))
            .id();
        commands.entity(joints[joint]).add_child(part);
    }

    // The thrown wrench spins about its middle, 0.2 m up the shaft from the grip.
    let wrench = model::wrench_parts_only();
    let middle = shapes::at(0.2, -0.06, 0.0);
    let thrown = commands
        .spawn((
            Name::new("thrown wrench"),
            ThrownWrenchModel { hero },
            Transform::default(),
            Visibility::Hidden,
        ))
        .id();
    for ((_, look), triangles) in joint_meshes(&wrench, middle) {
        let part = commands
            .spawn((
                Mesh3d(meshes.add(flat_mesh(&triangles))),
                MeshMaterial3d(material(look)),
                Transform::default(),
            ))
            .id();
        commands.entity(thrown).add_child(part);
    }

    commands.entity(root).insert(HeroRig {
        hero,
        joints,
        skeleton: model.joints,
        animator: pose::Animator::default(),
    });
    root
}

fn animate(
    time: Res<Time>,
    animations: Option<Res<RigAnimations>>,
    heroes: Query<&HeroBody>,
    mut rigs: Query<&mut HeroRig>,
    mut transforms: Query<(&mut Transform, &mut Visibility), With<RigJoint>>,
) {
    let seconds = time.delta_secs();
    for mut rig in &mut rigs {
        let Ok(body) = heroes.get(rig.hero) else {
            continue;
        };
        let hero = &body.0;
        let rig = &mut *rig;
        let animator = &mut rig.animator;
        animator.time += seconds;
        animator.phase = pose::advance_phase(animator.phase, hero, seconds);
        let mut target = pose::target(&rig.skeleton, hero, animator.phase, animator.time);
        let playing = play_clip(animator, animations.as_deref(), hero, seconds, &mut target);
        // Attacks and clips track closely; procedural poses ease in.
        let speed = if hero.state.is_attacking() {
            40.0
        } else if playing {
            25.0
        } else {
            14.0
        };
        animator.pose.ease(&target, 1.0 - (-speed * seconds).exp());

        for (index, &entity) in rig.joints.iter().enumerate() {
            let Ok((mut transform, mut visibility)) = transforms.get_mut(entity) else {
                continue;
            };
            let joint = &rig.skeleton[index];
            let parent_rest = joint.parent.map_or(Vec3::ZERO, |p| rig.skeleton[p].rest);
            transform.translation = joint.rest - parent_rest + animator.pose.offsets[index];
            transform.rotation = animator.pose.rotations[index];
            if index == joint::WRENCH {
                let wanted = if hero.thrown.is_some() {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
                visibility.set_if_neq(wanted);
            }
        }
    }
}

/// Puts the clip for the hero's move, if the game gave one, over the procedural `target`.
/// Returns whether one is playing.
fn play_clip(
    animator: &mut pose::Animator,
    animations: Option<&RigAnimations>,
    hero: &rac_hero::HeroMotion,
    seconds: f32,
    target: &mut Pose,
) -> bool {
    let current = RigMove::of(hero);
    if current != animator.clip_move {
        animator.clip_move = current;
        animator.clip_ticks = 0.0;
    } else {
        let rate = match current {
            // The run cycle plays faster the faster he runs: 1.25 times at UYA's 6.2 u/s.
            Some(RigMove::Run) => {
                (Vec2::new(hero.velocity.x, hero.velocity.z).length() / 5.0).clamp(0.5, 1.5)
            }
            Some(RigMove::DoubleJump) => 0.8,
            _ => 1.0,
        };
        animator.clip_ticks += seconds * 60.0 * rate;
    }
    let (Some(animations), Some(current)) = (animations, current) else {
        return false;
    };
    let Some(clip) = animations.clips.get(&current) else {
        return false;
    };
    let sample = if current.follows_attack_frame() {
        clip.at_frame(hero.attack_frame())
    } else {
        clip.at_time(animator.clip_ticks)
    };
    let Some((rotations, offsets)) = sample else {
        return false;
    };
    for (index, driven) in animations.driven.iter().enumerate() {
        if *driven && index < target.rotations.len() && index < rotations.len() {
            target.rotations[index] = rotations[index];
            target.offsets[index] = offsets[index];
        }
    }
    true
}

/// Spins the thrown wrench along its flight, and removes it with its hero.
fn fly_thrown_wrenches(
    time: Res<Time>,
    heroes: Query<&HeroBody>,
    mut wrenches: Query<(Entity, &ThrownWrenchModel, &mut Transform, &mut Visibility)>,
    mut commands: Commands,
) {
    for (entity, wrench, mut transform, mut visibility) in &mut wrenches {
        let Ok(body) = heroes.get(wrench.hero) else {
            commands.entity(entity).despawn();
            continue;
        };
        let Some(thrown) = body.0.thrown else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Visible);
        *transform = Transform::from_translation(thrown.position)
            .with_rotation(Quat::from_rotation_y(time.elapsed_secs() * -25.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_meshes_give_every_corner_its_faces_normal() {
        let mesh = flat_mesh(&[[Vec3::ZERO, Vec3::X, Vec3::Y]]);
        assert_eq!(mesh.count_vertices(), 3);
        let normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap();
        assert_eq!(normals.len(), 3);
    }

    #[test]
    fn every_joint_with_blocks_gets_a_mesh() {
        let model = model::hero();
        let meshes = joint_meshes(&model, Vec3::ZERO);
        for joint in [joint::HEAD, joint::HAND_R, joint::CLANK_HEAD, joint::WRENCH] {
            assert!(meshes.contains_key(&(joint, Look::Main)), "{joint}");
        }
        assert!(meshes.contains_key(&(joint::HEAD, Look::Accent)), "eyes");
    }
}

//! The hero kit's model (PLAN.md P1.17): a skeleton laid out like UYA's hero, posed from
//! the hero's state, wearing any model skinned to it.
//!
//! - [`model`] describes the skeleton and the open model: a blocky, flat-shaded gold
//!   Ratchet with Clank on his back and the wrench in his hand, whose blocks are each the
//!   convex hull of a few points, modelled by hand with UYA's hero as the reference
//!   (`docs/research/moby-format.md`). Nothing in it comes from a disc.
//! - [`RigModels`] holds the models heroes can wear: the open one, and any a game adds,
//!   such as its hero imported from the user's own disc. Every one wears the same skeleton,
//!   so the same poses drive it; [`SkeletonMap`] lines a game's skeleton up with it.
//! - A hero entity with a [`RigLook`] is dressed in the chosen model. [`RigPlugin`] poses
//!   it every frame from the hero's state, and flies a copy of the wrench while it is
//!   thrown.
//! - A game with real animation inserts [`RigAnimations`]: clips for some [`RigMove`]s,
//!   already on the rig's joints, and its rules for fidgets. Those moves play their clip,
//!   chosen and timed as the game does (fidgets, landings, the walk's speed bands); the
//!   rest stay procedural.
//!
//! The rig only reads the controller ([`rac_hero::HeroBody`]), so models and animations
//! change without touching the simulation.

mod clips;
mod hull;
pub mod model;
mod play;
mod pose;
mod retarget;
mod shapes;
mod wear;

use bevy::camera::visibility::DynamicSkinnedMeshBounds;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use rac_hero::{Health, HeroBody};

pub use clips::{
    FidgetRule, FlipSide, Landing, RUN_BELOW, RigAnimations, RigClip, RigKeyframe, RigMove,
    SET_OFF_BELOW, Sample,
};
pub use hull::convex_hull;
pub use model::{JointDef, Look, ModelDef, PartDef, joint};
pub use pose::Pose;
pub use retarget::{LineUpError, SkeletonMap, SourceJoint};
pub use wear::{
    ModelData, ModelError, OPEN_MODEL, Paint, RigModel, RigModels, Surface, open_model,
};

/// Dresses heroes in the chosen model and poses every [`HeroRig`] from its hero each
/// frame.
pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RigModels>()
            .add_systems(Startup, wear::register_open_model)
            .add_systems(Update, (dress, animate, fly_thrown_wrenches).chain());
    }
}

/// The colours of one hero's model. A hero entity with one is dressed in the chosen
/// model.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct RigLook {
    pub main: Color,
    pub accent: Color,
    /// Whether a model's own materials are tinted with `main`, to tell players apart.
    pub tint_own: bool,
}

impl RigLook {
    /// Gold for player 1, silver for player 2 (P1.17's player colours).
    pub fn for_player(player: usize) -> Self {
        if player.is_multiple_of(2) {
            Self {
                main: Color::srgb(1.0, 0.78, 0.34),
                accent: Color::srgb(0.45, 0.30, 0.10),
                tint_own: player > 0,
            }
        } else {
            Self {
                main: Color::srgb(0.86, 0.88, 0.92),
                accent: Color::srgb(0.25, 0.28, 0.34),
                tint_own: true,
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
    /// The copy of the wrench that flies while it is thrown.
    pub thrown: Entity,
    skeleton: Vec<JointDef>,
    /// Each joint's orientation at rest in the model ([`ModelData::turns`]).
    turns: Vec<Quat>,
    /// The surfaces of the wrench in his hand, hidden while it is thrown.
    wrench: Vec<Entity>,
    animator: pose::Animator,
}

/// On a hero entity: the model it wears.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Worn {
    pub model: String,
    pub rig: Entity,
}

/// One joint of a [`HeroRig`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RigJoint(pub usize);

/// The copy of the wrench that flies while it is thrown.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrownWrenchModel {
    pub hero: Entity,
}

/// Dresses `hero` in `model` and returns the model's root and the thrown wrench's copy.
/// Materials come from `look`, or the model's own, tinted for players after the first.
pub fn spawn_rig(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    hero: Entity,
    look: RigLook,
    model: &RigModel,
) -> (Entity, Entity) {
    let main = materials.add(look.material(Look::Main));
    let accent = materials.add(look.material(Look::Accent));
    let mut material = |paint: &Paint| match paint {
        Paint::Look(Look::Main) => main.clone(),
        Paint::Look(Look::Accent) => accent.clone(),
        Paint::Own(own) if !look.tint_own => own.clone(),
        Paint::Own(own) => {
            let mut tinted = materials.get(own).cloned().unwrap_or_default();
            tinted.base_color = look.main;
            materials.add(tinted)
        }
    };

    let root = commands
        .spawn((
            Name::new(format!("hero model ({})", model.name)),
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    commands.entity(hero).add_child(root);

    let skeleton = &model.skeleton;
    let mut joints: Vec<Entity> = Vec::with_capacity(skeleton.len());
    for (index, joint) in skeleton.iter().enumerate() {
        let entity = commands
            .spawn((
                Name::new(joint.name),
                RigJoint(index),
                joint_transform(
                    skeleton,
                    &model.turns,
                    index,
                    Quat::IDENTITY,
                    Vec3::ZERO,
                    Vec3::ONE,
                ),
                Visibility::default(),
            ))
            .id();
        let parent = joint.parent.map_or(root, |p| joints[p]);
        commands.entity(parent).add_child(entity);
        joints.push(entity);
    }

    // Skinned meshes are placed by their joints, whatever their own transform.
    let mut wrench = Vec::new();
    for surface in &model.surfaces {
        let entity = commands
            .spawn((
                Mesh3d(surface.mesh.clone()),
                MeshMaterial3d(material(&surface.paint)),
                SkinnedMesh {
                    inverse_bindposes: model.inverse_bindposes.clone(),
                    joints: joints.clone(),
                },
                DynamicSkinnedMeshBounds,
                Transform::default(),
            ))
            .id();
        commands.entity(root).add_child(entity);
        if surface.wrench {
            wrench.push(entity);
        }
    }

    let thrown = commands
        .spawn((
            Name::new("thrown wrench"),
            ThrownWrenchModel { hero },
            Transform::default(),
            Visibility::Hidden,
        ))
        .id();
    for surface in &model.thrown {
        let part = commands
            .spawn((
                Mesh3d(surface.mesh.clone()),
                MeshMaterial3d(material(&surface.paint)),
                Transform::default(),
            ))
            .id();
        commands.entity(thrown).add_child(part);
    }

    commands.entity(root).insert(HeroRig {
        hero,
        joints,
        thrown,
        skeleton: model.skeleton.clone(),
        turns: model.turns.clone(),
        wrench,
        animator: pose::Animator::new(hero.to_bits() as u32),
    });
    (root, thrown)
}

/// Dresses each hero with a [`RigLook`] in the chosen model, again whenever the choice
/// changes.
fn dress(
    mut commands: Commands,
    models: Res<RigModels>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    heroes: Query<(Entity, &RigLook, Option<&Worn>), With<HeroBody>>,
    rigs: Query<&HeroRig>,
) {
    let (Some(model), Some(mut materials)) = (models.chosen(), materials) else {
        return;
    };
    for (hero, look, worn) in &heroes {
        if let Some(worn) = worn {
            if worn.model == model.name {
                continue;
            }
            if let Ok(rig) = rigs.get(worn.rig) {
                commands.entity(rig.thrown).despawn();
            }
            commands.entity(worn.rig).despawn();
        }
        let (rig, _) = spawn_rig(&mut commands, &mut materials, hero, *look, model);
        commands.entity(hero).insert(Worn {
            model: model.name.clone(),
            rig,
        });
    }
}

/// A joint's transform relative to its parent, posed by a rotation (its turn from rest,
/// relative to its parent's), an offset from rest and a scale, in a model whose joints may
/// be turned at rest.
fn joint_transform(
    skeleton: &[JointDef],
    turns: &[Quat],
    index: usize,
    rotation: Quat,
    offset: Vec3,
    scale: Vec3,
) -> Transform {
    let joint = &skeleton[index];
    let turn = turns.get(index).copied().unwrap_or_default();
    let (parent_rest, parent_turn) = joint.parent.map_or((Vec3::ZERO, Quat::IDENTITY), |p| {
        (skeleton[p].rest, turns.get(p).copied().unwrap_or_default())
    });
    // Poses are in the parent's unturned axes; the joint's own axes are turned at rest.
    let into_parent = parent_turn.inverse();
    Transform {
        translation: into_parent * (joint.rest - parent_rest + offset),
        rotation: into_parent * rotation * turn,
        scale,
    }
}

fn animate(
    time: Res<Time>,
    animations: Option<Res<RigAnimations>>,
    heroes: Query<(&HeroBody, Option<&Health>)>,
    mut rigs: Query<&mut HeroRig>,
    mut transforms: Query<&mut Transform, With<RigJoint>>,
    mut visibilities: Query<&mut Visibility, Without<RigJoint>>,
) {
    let seconds = time.delta_secs();
    for mut rig in &mut rigs {
        let Ok((body, health)) = heroes.get(rig.hero) else {
            continue;
        };
        let hero = &body.0;
        let health = health.map(|h| h.current as f32 / h.max.max(1) as f32);
        let rig = &mut *rig;
        let animator = &mut rig.animator;
        animator.time += seconds;
        animator.phase = pose::advance_phase(animator.phase, hero, seconds);
        let mut target = pose::target(&rig.skeleton, hero, animator.phase, animator.time);
        let playing = play_clip(
            animator,
            animations.as_deref(),
            hero,
            health,
            seconds,
            &mut target,
        );
        // Attacks and clips track closely; procedural poses ease in.
        let speed = if hero.state.is_attacking() {
            40.0
        } else if playing {
            25.0
        } else {
            14.0
        };
        animator.ease(
            &target,
            pose::strafe_twist(hero),
            1.0 - (-speed * seconds).exp(),
        );

        for (index, &entity) in rig.joints.iter().enumerate() {
            let Ok(mut transform) = transforms.get_mut(entity) else {
                continue;
            };
            let pose = &animator.pose;
            *transform = joint_transform(
                &rig.skeleton,
                &rig.turns,
                index,
                animator.rotation(index),
                pose.offsets[index],
                pose.scales[index],
            );
        }
        let wanted = if hero.thrown.is_some() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        for &surface in &rig.wrench {
            if let Ok(mut visibility) = visibilities.get_mut(surface) {
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
    health: Option<f32>,
    seconds: f32,
    target: &mut Pose,
) -> bool {
    let director = &mut animator.director;
    let current = director.step(hero, health, animations, seconds);
    let (Some(animations), Some(current)) = (animations, current) else {
        return false;
    };
    let Some(clip) = animations.clips.get(&current) else {
        return false;
    };
    let sample = if current.follows_attack_frame() {
        clip.at_frame(hero.attack_frame())
    } else {
        clip.at_time(director.ticks)
    };
    let Some((rotations, offsets, scales)) = sample else {
        return false;
    };
    for (index, driven) in animations.driven.iter().enumerate() {
        if *driven && index < target.rotations.len() && index < rotations.len() {
            target.rotations[index] = rotations[index];
            target.offsets[index] = offsets[index];
            if let Some(scale) = scales.get(index) {
                target.scales[index] = *scale;
            }
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
    use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
    use rac_hero::HeroMotion;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<SkinnedMeshInverseBindposes>()
            .add_plugins(RigPlugin);
        app
    }

    fn rigs(app: &mut App) -> Vec<String> {
        let mut query = app.world_mut().query::<&Name>();
        let world = app.world();
        let mut names: Vec<String> = query
            .iter(world)
            .filter(|n| n.as_str().starts_with("hero model"))
            .map(|n| n.to_string())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn heroes_are_dressed_in_the_chosen_model_and_redressed_when_it_changes() {
        let mut app = app();
        app.world_mut().spawn((
            HeroBody(HeroMotion::new(Vec3::ZERO, 0.0)),
            RigLook::for_player(0),
        ));
        app.update();
        app.update();
        assert_eq!(rigs(&mut app), vec!["hero model (open)"]);

        let mut other = open_model();
        other.name = "other".into();
        app.world_mut()
            .resource_scope(|world, mut models: Mut<RigModels>| {
                world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
                    let mut bindposes = world.resource_mut::<Assets<SkinnedMeshInverseBindposes>>();
                    models.add(other, &mut meshes, &mut bindposes).unwrap();
                });
                models.choose("other");
            });
        app.update();
        app.update();
        assert_eq!(
            rigs(&mut app),
            vec!["hero model (other)"],
            "the old model is gone"
        );

        let mut skinned = app.world_mut().query::<&SkinnedMesh>();
        let count = skinned.iter(app.world()).count();
        assert_eq!(count, open_model().surfaces.len());
        assert!(
            skinned
                .iter(app.world())
                .all(|s| s.joints.len() == joint::COUNT)
        );
    }
}

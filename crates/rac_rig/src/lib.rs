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
//! - A model's attachments ([`AttachmentData`]) ride on their joints without the joints'
//!   scale, and play the clips [`BackAnimations`] gives them by the game's rules, as UYA's
//!   Clank and heli-pack do on the hero's back.
//! - While a gun is in his hand ([`rac_hero::GunHand`]) he holds its model ([`RigGuns`])
//!   instead of the wrench, and plays its firing animations if the game gave them.
//!
//! The rig only reads the controller ([`rac_hero::HeroBody`]), so models and animations
//! change without touching the simulation.

mod back;
mod clips;
mod gun_anim;
mod guns;
mod hull;
pub mod model;
mod play;
mod pose;
mod retarget;
mod rope;
mod shapes;
mod springs;
mod wear;

use bevy::camera::visibility::DynamicSkinnedMeshBounds;
use bevy::math::Affine3A;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use rac_hero::{Health, HeroBody};

pub use clips::{
    BackAnimations, BackHold, BackRow, FidgetRule, FlipSide, HoldAnimations, JumpTiming, Landing,
    RUN_BELOW, RigAnimations, RigClip, RigKeyframe, RigMove, SET_OFF_BELOW, Sample,
};
pub use guns::{GunData, GunMotion, GunSkeleton, Recoil, RigGuns, open_guns};
pub use hull::convex_hull;
pub use model::{JointDef, Look, ModelDef, PartDef, joint};
pub use pose::Pose;
pub use retarget::{LineUpError, SkeletonMap, SourceJoint};
pub use rope::{LINK_LEAD, ROPE_POINTS, RopeModel, RopeModels, RopeShape};
pub use wear::{
    AttachmentData, ModelData, ModelError, OPEN_MODEL, Paint, RigModel, RigModels, Surface,
    open_model,
};

/// Dresses heroes in the chosen model and poses every [`HeroRig`] from its hero each
/// frame.
pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RigModels>()
            .init_resource::<RigGuns>()
            .init_resource::<RopeModels>()
            .add_systems(
                Startup,
                (
                    wear::register_open_model,
                    guns::register_open_guns,
                    rope::register_rope_looks,
                ),
            )
            .add_systems(
                Update,
                (
                    dress,
                    animate,
                    hold_guns,
                    rope::draw_ropes,
                    fly_thrown_wrenches,
                )
                    .chain()
                    .in_set(RigSystems),
            );
    }
}

/// The rig's posing each frame, for what is drawn on it to come after.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RigSystems;

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
    attachments: Vec<Attached>,
    animator: pose::Animator,
    back: back::BackDirector,
    /// The game's springs on a few joints over the animation: the raised gun arm and the
    /// turn to the gun's target.
    springs: springs::JointSprings,
    /// Where a gun's grip sits in his fist, in the model's space, as last posed.
    grip: Transform,
    /// The gun model in his hand.
    gun: Option<InHand>,
    /// The Plasma Whip's rope, where it was last drawn, and its links drawn in the world.
    rope: rope::Rope,
    rope_shape: Option<RopeShape>,
    links: Vec<Entity>,
    /// What the links were made as: from a game's models or the kit's, and their glow.
    link_look: Option<rope::LinkLook>,
}

impl HeroRig {
    /// Where the Plasma Whip's rope lies this frame, while its links show.
    pub fn rope(&self) -> Option<&RopeShape> {
        self.rope_shape.as_ref()
    }
}

/// The gun model in a hero's hand.
#[derive(Debug)]
struct InHand {
    item: u16,
    /// The level its model shows from.
    from: u8,
    entity: Entity,
    /// A gun with a skeleton: its joints, and where its own animation is.
    joints: Vec<Entity>,
    player: Option<gun_anim::GunPlayer>,
}

/// The model of the gun in a hero's hand.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldGun {
    pub item: u16,
}

/// One joint of the gun in a hero's hand, or its origin.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GunJoint;

/// An attachment a rig carries: the entity placed on its joint, and its own joints.
#[derive(Debug)]
struct Attached {
    name: String,
    on: usize,
    root: Entity,
    joints: Vec<Entity>,
    rest: Vec<Transform>,
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

/// An attachment's place on its joint, or one of its own joints: posed each frame with the
/// rig's joints.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentJoint;

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
                // The wrench's surfaces hide while it flies or a gun is out.
                Visibility::default(),
            ))
            .id();
        commands.entity(root).add_child(entity);
        if surface.wrench {
            wrench.push(entity);
        }
    }

    let mut attachments = Vec::new();
    for attachment in &model.attachments {
        let place = commands
            .spawn((
                Name::new(format!("attachment ({})", attachment.name)),
                AttachmentJoint,
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        commands.entity(root).add_child(place);
        let mut own: Vec<Entity> = Vec::with_capacity(attachment.rest.len());
        for (index, rest) in attachment.rest.iter().enumerate() {
            let entity = commands
                .spawn((
                    Name::new(format!("{} joint {index}", attachment.name)),
                    AttachmentJoint,
                    *rest,
                    Visibility::default(),
                ))
                .id();
            let parent = attachment.parents[index].map_or(place, |p| own[p]);
            commands.entity(parent).add_child(entity);
            own.push(entity);
        }
        for surface in &attachment.surfaces {
            let entity = commands
                .spawn((
                    Mesh3d(surface.mesh.clone()),
                    MeshMaterial3d(material(&surface.paint)),
                    SkinnedMesh {
                        inverse_bindposes: attachment.inverse_bindposes.clone(),
                        joints: own.clone(),
                    },
                    DynamicSkinnedMeshBounds,
                    Transform::default(),
                ))
                .id();
            commands.entity(place).add_child(entity);
        }
        attachments.push(Attached {
            name: attachment.name.clone(),
            on: attachment.on,
            root: place,
            joints: own,
            rest: attachment.rest.clone(),
        });
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
        attachments,
        animator: pose::Animator::new(hero.to_bits() as u32),
        back: back::BackDirector::new(hero.to_bits() as u32 ^ 0x9E37_79B9),
        springs: springs::JointSprings::default(),
        grip: Transform::default(),
        gun: None,
        rope: rope::Rope::default(),
        rope_shape: None,
        links: Vec::new(),
        link_look: None,
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
                for &link in &rig.links {
                    commands.entity(link).despawn();
                }
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
    mut transforms: Query<&mut Transform, Or<(With<RigJoint>, With<AttachmentJoint>)>>,
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

        let lifts = animations.as_deref().is_some_and(|animations| {
            let director = &animator.director;
            animator
                .hold
                .lifts(director.current, director.ticks, animations)
        });
        rig.springs.step(hero, lifts, seconds);
        let springs = &rig.springs;
        let locals: Vec<Transform> = (0..rig.joints.len())
            .map(|index| {
                let rotation = animator.rotation(index);
                joint_transform(
                    &rig.skeleton,
                    &rig.turns,
                    index,
                    springs.turn(index).map_or(rotation, |turn| rotation * turn),
                    animator.pose.offsets[index],
                    animator.pose.scales[index],
                )
            })
            .collect();
        for (&entity, local) in rig.joints.iter().zip(&locals) {
            if let Ok(mut transform) = transforms.get_mut(entity) {
                *transform = *local;
            }
        }
        rig.grip = gripped(&rig.skeleton, &rig.turns, &locals);

        if !rig.attachments.is_empty() {
            let back = animations.as_deref().and_then(|a| a.back.as_ref());
            let director = &animator.director;
            let poses = back.map_or_else(Vec::new, |back| {
                rig.back
                    .step(back, director.current, director.rate, seconds)
            });
            for attached in &rig.attachments {
                if let Ok(mut transform) = transforms.get_mut(attached.root) {
                    *transform = placed_on(&rig.skeleton, &locals, attached.on);
                }
                let pose = back
                    .and_then(|b| b.parts.iter().position(|p| *p == attached.name))
                    .and_then(|part| poses.get(part))
                    .and_then(Option::as_ref);
                for (index, &entity) in attached.joints.iter().enumerate() {
                    let Ok(mut transform) = transforms.get_mut(entity) else {
                        continue;
                    };
                    *transform = match pose {
                        Some((rotations, translations, scales)) if index < rotations.len() => {
                            Transform {
                                translation: translations[index],
                                rotation: rotations[index],
                                scale: scales[index],
                            }
                        }
                        _ => attached.rest[index],
                    };
                }
            }
        }
        let wanted = if hero.thrown.is_some() || hero.gun.item.is_some() {
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

/// Where an attachment sits on rig joint `on`, in the model's space: the joint's place
/// and turn, from each joint's transform from its parent, but not its scale, as the games
/// normalise the joint's axes before putting a moby on it.
fn placed_on(skeleton: &[JointDef], locals: &[Transform], on: usize) -> Transform {
    let mut placed = Affine3A::IDENTITY;
    let mut joint = Some(on);
    while let Some(index) = joint {
        placed = locals[index].compute_affine() * placed;
        joint = skeleton[index].parent;
    }
    let (_, rotation, translation) = placed.to_scale_rotation_translation();
    Transform::from_translation(translation).with_rotation(rotation)
}

/// Where a gun's grip sits in the right fist, in the model's space: the hand's place and
/// its turn from rest without the model's own turn at rest, so every model holds a gun
/// the same way ([`model::gun_grip`]).
fn gripped(skeleton: &[JointDef], turns: &[Quat], locals: &[Transform]) -> Transform {
    let hand = placed_on(skeleton, locals, joint::HAND_R);
    let unturned = hand.rotation
        * turns
            .get(joint::HAND_R)
            .copied()
            .unwrap_or_default()
            .inverse();
    Transform::from_translation(hand.translation + unturned * model::GRIP_FROM_HAND)
        .with_rotation(unturned * model::gun_grip())
}

/// Puts the model of the gun in each hero's hand in his fist, changing it when he changes
/// guns, and takes it away when he puts it away. A gun with a skeleton plays its own
/// sequences ([`gun_anim`]).
fn hold_guns(
    time: Res<Time>,
    guns: Res<RigGuns>,
    heroes: Query<&HeroBody>,
    mut rigs: Query<(Entity, &mut HeroRig)>,
    mut transforms: Query<&mut Transform, Or<(With<HeldGun>, With<GunJoint>)>>,
    mut commands: Commands,
) {
    let seconds = time.delta_secs();
    for (root, mut rig) in &mut rigs {
        let hand = heroes.get(rig.hero).ok().map(|body| body.0.gun);
        let wanted = hand.and_then(|hand| {
            let item = hand.item?;
            let (from, gun) = guns.get(item, hand.level)?;
            Some((item, from, gun))
        });
        let showing = rig.gun.as_ref().map(|held| (held.item, held.from));
        if showing != wanted.map(|(item, from, _)| (item, from)) {
            if let Some(held) = rig.gun.take() {
                commands.entity(held.entity).despawn();
            }
            let (Some((item, from, gun)), Some(hand)) = (wanted, hand) else {
                continue;
            };
            let entity = commands
                .spawn((
                    Name::new(format!("gun {item:#x}")),
                    HeldGun { item },
                    rig.grip,
                    Visibility::default(),
                ))
                .id();
            commands.entity(root).add_child(entity);
            let mut joints = Vec::new();
            // Skinned meshes are placed by their joints; rigid ones sit on the gun.
            let parent = match &gun.skeleton {
                Some(skeleton) => {
                    let origin = commands
                        .spawn((
                            Name::new("gun origin"),
                            GunJoint,
                            skeleton.origin,
                            Visibility::default(),
                        ))
                        .id();
                    commands.entity(entity).add_child(origin);
                    for (index, rest) in skeleton.rest.iter().enumerate() {
                        let joint = commands
                            .spawn((
                                Name::new(format!("gun joint {index}")),
                                GunJoint,
                                *rest,
                                Visibility::default(),
                            ))
                            .id();
                        let parent = skeleton.parents[index].map_or(origin, |p| joints[p]);
                        commands.entity(parent).add_child(joint);
                        joints.push(joint);
                    }
                    origin
                }
                None => entity,
            };
            for surface in &gun.surfaces {
                let Paint::Own(material) = &surface.paint else {
                    continue;
                };
                let mut part = commands.spawn((
                    Mesh3d(surface.mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                ));
                if let Some(skeleton) = &gun.skeleton {
                    part.insert((
                        SkinnedMesh {
                            inverse_bindposes: skeleton.inverse_bindposes.clone(),
                            joints: joints.clone(),
                        },
                        DynamicSkinnedMeshBounds,
                    ));
                }
                let part = part.id();
                commands.entity(parent).add_child(part);
            }
            rig.gun = Some(InHand {
                item,
                from,
                entity,
                joints,
                player: gun
                    .skeleton
                    .as_ref()
                    .map(|_| gun_anim::GunPlayer::new(hand.shots)),
            });
            continue;
        }
        let grip = rig.grip;
        let (Some(held), Some((_, _, gun)), Some(hand)) = (rig.gun.as_mut(), wanted, hand) else {
            continue;
        };
        if let Ok(mut transform) = transforms.get_mut(held.entity) {
            *transform = grip;
        }
        let (Some(player), Some(skeleton)) = (held.player.as_mut(), &gun.skeleton) else {
            continue;
        };
        let pose = player.step(&skeleton.clips, &skeleton.motion, hand.shots, seconds);
        for (index, &joint) in held.joints.iter().enumerate() {
            let Ok(mut transform) = transforms.get_mut(joint) else {
                continue;
            };
            *transform = match &pose {
                Some((rotations, translations, scales)) if index < rotations.len() => Transform {
                    translation: translations[index],
                    rotation: rotations[index],
                    scale: scales[index],
                },
                _ => skeleton.rest[index],
            };
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
    animator.arms.step(hero, animations, seconds);
    let firing_arms = animator.arms.firing;
    animator
        .hold
        .step(hero, current, firing_arms, animations, seconds);
    let Some(animations) = animations else {
        return false;
    };
    let ticks = animator.director.ticks;
    let clip = current.and_then(|current| Some((current, animations.clips.get(&current)?)));
    let sample = clip.and_then(|(current, clip)| {
        if current.follows_attack_frame() {
            clip.at_frame(hero.attack_frame())
        } else {
            clip.at_time(ticks)
        }
    });
    let playing = sample.is_some();
    if let Some((rotations, offsets, scales)) = sample {
        for (index, driven) in animations.driven.iter().enumerate() {
            if *driven && index < target.rotations.len() && index < rotations.len() {
                target.rotations[index] = rotations[index];
                target.offsets[index] = offsets[index];
                if let Some(scale) = scales.get(index) {
                    target.scales[index] = *scale;
                }
            }
        }
    }

    // The arms holding the gun: each arm's clip for the move, or the move's own arms where
    // it has none, gone over to their rest as far as they have.
    let hold = &animator.hold;
    if let Some(holds) = animations.hold.as_ref().filter(|_| hold.on()) {
        for (side, joints) in [joint::ARM_R, joint::ARM_L].into_iter().enumerate() {
            let weight = hold.weights[side];
            if weight <= 0.0 {
                continue;
            }
            let follow = current.and_then(|m| holds.follow[side].get(&m)?.at_time(ticks));
            let rest = holds.rest[side]
                .as_ref()
                .filter(|_| hold.resting > 0.0)
                .and_then(|rest| rest.at_time(hold.rest_ticks));
            for index in joints {
                let Some(mut held) = joint_of(target, follow.as_ref(), index) else {
                    continue;
                };
                if let Some(rest) = rest.as_ref().and_then(|r| joint_of(target, Some(r), index)) {
                    held = mix_joint(held, rest, hold.resting);
                }
                lay_over(target, &animations.driven, index, held, weight);
            }
        }
    }

    // The arms-only firing animation, laid over whatever the rest plays.
    let arms = &animator.arms;
    let layer = arms
        .playing
        .filter(|_| arms.weight > 0.0)
        .and_then(|playing| animations.clips.get(&playing)?.at_time(arms.ticks));
    if let Some(layer) = layer {
        let sides = if arms.both { 2 } else { 1 };
        for index in [joint::ARM_R, joint::ARM_L]
            .into_iter()
            .take(sides)
            .flatten()
        {
            if let Some(fired) = joint_of(target, Some(&layer), index) {
                lay_over(target, &animations.driven, index, fired, arms.weight);
            }
        }
    }
    playing
}

/// Joint `index` of `sample`, or of `target` where `sample` is `None` or lacks a scale.
/// `None` for a joint neither has.
fn joint_of(target: &Pose, sample: Option<&Sample>, index: usize) -> Option<(Quat, Vec3, Vec3)> {
    let scale = |scales: &[Vec3]| scales.get(index).copied();
    match sample {
        Some((rotations, offsets, scales)) => Some((
            *rotations.get(index)?,
            *offsets.get(index)?,
            scale(scales).or_else(|| scale(&target.scales))?,
        )),
        None => Some((
            *target.rotations.get(index)?,
            *target.offsets.get(index)?,
            scale(&target.scales)?,
        )),
    }
}

/// `t` of the way from one joint's pose to another's.
fn mix_joint(
    (rotation, offset, scale): (Quat, Vec3, Vec3),
    (to_rotation, to_offset, to_scale): (Quat, Vec3, Vec3),
    t: f32,
) -> (Quat, Vec3, Vec3) {
    (
        rotation.slerp(to_rotation, t),
        offset.lerp(to_offset, t),
        scale.lerp(to_scale, t),
    )
}

/// Lays a layer's pose of joint `index` over `target`'s at `weight`, if the clips drive it.
fn lay_over(
    target: &mut Pose,
    driven: &[bool],
    index: usize,
    layer: (Quat, Vec3, Vec3),
    weight: f32,
) {
    if !driven.get(index).copied().unwrap_or(false) || index >= target.rotations.len() {
        return;
    }
    let own = (
        target.rotations[index],
        target.offsets[index],
        target.scales[index],
    );
    (
        target.rotations[index],
        target.offsets[index],
        target.scales[index],
    ) = mix_joint(own, layer, weight);
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
    fn a_gun_in_one_hand_lays_the_right_arms_hold_over_the_bodys_clip() {
        let posed = |rotation: Quat| RigClip {
            keyframes: vec![RigKeyframe {
                ticks: 2.0,
                frame: 0.0,
                rotations: vec![rotation; joint::COUNT],
                offsets: vec![Vec3::ZERO; joint::COUNT],
                scales: vec![Vec3::ONE; joint::COUNT],
            }],
            looping: true,
            start: 0,
        };
        let held = Quat::from_rotation_x(1.0);
        let animations = RigAnimations {
            clips: [(RigMove::Idle, posed(Quat::IDENTITY))].into(),
            driven: vec![true; joint::COUNT],
            hold: Some(HoldAnimations {
                follow: [
                    [(RigMove::Idle, posed(held))].into(),
                    [(RigMove::Idle, posed(held))].into(),
                ],
                ..default()
            }),
            ..default()
        };
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.gun = rac_hero::GunHand {
            out: true,
            item: Some(0x2F),
            pose: rac_hero::GunPose {
                hold: rac_hero::Hold::OneHand,
                ..default()
            },
            ..default()
        };
        let mut animator = pose::Animator::new(1);
        let mut target = Pose::rest();
        // Half in after two ticks, at 0.25 a tick.
        for _ in 0..2 {
            target = Pose::rest();
            play_clip(
                &mut animator,
                Some(&animations),
                &hero,
                None,
                1.0 / 60.0,
                &mut target,
            );
        }
        let half = Quat::IDENTITY.slerp(held, 0.5);
        assert!(target.rotations[joint::HAND_R].angle_between(half) < 1e-4);
        for _ in 0..2 {
            target = Pose::rest();
            play_clip(
                &mut animator,
                Some(&animations),
                &hero,
                None,
                1.0 / 60.0,
                &mut target,
            );
        }
        for index in joint::ARM_R {
            assert!(
                target.rotations[index].angle_between(held) < 1e-4,
                "{index}"
            );
        }
        for index in joint::ARM_L.chain([joint::CHEST, joint::HEAD]) {
            assert!(
                target.rotations[index].angle_between(Quat::IDENTITY) < 1e-4,
                "{index}"
            );
        }
    }

    #[test]
    fn an_attachment_takes_its_joints_place_and_turn_but_not_its_scale() {
        let skeleton = model::hero().joints;
        let mut locals = vec![Transform::IDENTITY; skeleton.len()];
        locals[joint::ROOT] = Transform::from_scale(Vec3::splat(2.0));
        use joint::*;
        for joint in [HIPS, WAIST, SPINE, SPINE_UPPER, CHEST] {
            locals[joint] = Transform::from_translation(Vec3::Y * 0.1);
        }
        locals[CHEST].rotation = Quat::from_rotation_y(1.0);
        let placed = placed_on(&skeleton, &locals, CHEST);
        assert_eq!(placed.scale, Vec3::ONE);
        assert!(
            placed.translation.distance(Vec3::Y) < 1e-5,
            "5 × 0.1 m, doubled"
        );
        assert!(placed.rotation.angle_between(Quat::from_rotation_y(1.0)) < 1e-5);
    }

    #[test]
    fn attachments_are_dressed_and_play_the_backs_clips() {
        let mut app = app();
        let mut model = open_model();
        model.name = "packed".into();
        model.attachments.push(wear::tests::attachment());
        app.world_mut()
            .resource_scope(|world, mut models: Mut<RigModels>| {
                world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
                    let mut bindposes = world.resource_mut::<Assets<SkinnedMeshInverseBindposes>>();
                    models.add(model, &mut meshes, &mut bindposes).unwrap();
                });
                models.choose("packed");
            });
        // The pack's rest sequence lifts its root 2 m and triples it.
        let keyframe = RigKeyframe {
            ticks: 10.0,
            frame: 0.0,
            rotations: vec![Quat::IDENTITY; 2],
            offsets: vec![Vec3::Y * 2.0, Vec3::X * 0.5],
            scales: vec![Vec3::splat(3.0), Vec3::ONE],
        };
        let clip = RigClip {
            keyframes: vec![keyframe.clone(), keyframe],
            looping: true,
            start: 0,
        };
        app.insert_resource(RigAnimations {
            back: Some(BackAnimations {
                parts: vec!["pack".into()],
                clips: vec![[(1, clip)].into()],
                rest: vec![1],
                ..default()
            }),
            ..default()
        });
        app.world_mut().spawn((
            HeroBody(HeroMotion::new(Vec3::ZERO, 0.0)),
            RigLook::for_player(0),
        ));
        for _ in 0..3 {
            app.update();
        }
        let mut skinned = app.world_mut().query::<&SkinnedMesh>();
        let own: Vec<usize> = skinned
            .iter(app.world())
            .map(|s| s.joints.len())
            .filter(|&n| n != joint::COUNT)
            .collect();
        assert_eq!(own, [2], "the pack's surface on its own two joints");
        let mut named = app.world_mut().query::<(&Name, &Transform)>();
        let mut transform = |name: &str| {
            named
                .iter(app.world())
                .find(|(n, _)| n.as_str() == name)
                .map(|(_, t)| *t)
                .unwrap()
        };
        let root = transform("pack joint 0");
        assert!(root.translation.distance(Vec3::Y * 2.0) < 1e-5);
        assert!(root.scale.distance(Vec3::splat(3.0)) < 1e-5);
        assert_eq!(transform("attachment (pack)").scale, Vec3::ONE);
    }

    #[test]
    fn a_gun_out_is_held_in_the_fist_in_place_of_the_wrench() {
        let mut app = app();
        let hero = app
            .world_mut()
            .spawn((
                HeroBody(HeroMotion::new(Vec3::ZERO, 0.0)),
                RigLook::for_player(0),
            ))
            .id();
        app.update();
        app.update();
        let held = |app: &mut App| {
            let mut query = app.world_mut().query::<&HeldGun>();
            query
                .iter(app.world())
                .map(|gun| gun.item)
                .collect::<Vec<_>>()
        };
        let wrench_shown = |app: &mut App| {
            let mut rigs = app.world_mut().query::<&HeroRig>();
            let surfaces = rigs.iter(app.world()).next().unwrap().wrench.clone();
            surfaces
                .iter()
                .all(|&s| app.world().get::<Visibility>(s) != Some(&Visibility::Hidden))
        };
        assert!(held(&mut app).is_empty());
        assert!(wrench_shown(&mut app));

        let give = |app: &mut App, item: Option<u16>| {
            let mut body = app.world_mut().get_mut::<HeroBody>(hero).unwrap();
            body.0.gun.out = item.is_some();
            body.0.gun.item = item;
            app.update();
        };
        give(&mut app, Some(0x2F));
        assert_eq!(held(&mut app), [0x2F]);
        assert!(!wrench_shown(&mut app));
        give(&mut app, Some(0x37));
        assert_eq!(held(&mut app), [0x37], "a gun without a model of its own");
        give(&mut app, None);
        assert!(held(&mut app).is_empty());
        assert!(wrench_shown(&mut app));
    }

    #[test]
    fn a_gun_with_a_skeleton_grows_in_settles_and_recoils_on_its_own_joints() {
        let mut app = app();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 60.0),
        ));
        // Each sequence holds the gun's first joint at its own height.
        let clip = |height: f32| {
            let keyframe = RigKeyframe {
                ticks: 2.0,
                frame: 0.0,
                rotations: vec![Quat::IDENTITY; 2],
                offsets: vec![Vec3::Y * height, Vec3::X],
                scales: vec![Vec3::ONE; 2],
            };
            RigClip {
                keyframes: vec![keyframe.clone(), keyframe],
                looping: true,
                start: 0,
            }
        };
        let mut surface = Surface::new(Paint::Own(Handle::default()), false);
        surface.push_rigid(&[[Vec3::ZERO, Vec3::X, Vec3::Y]], 1);
        let gun = GunData {
            item: Some(0x99),
            level: 0,
            surfaces: vec![surface],
            skeleton: Some(GunSkeleton {
                origin: Transform::default(),
                parents: vec![None, Some(0)],
                binds: vec![Mat4::IDENTITY, Mat4::from_translation(Vec3::X)],
                clips: [(0, clip(1.0)), (1, clip(2.0)), (2, clip(3.0))].into(),
                motion: GunMotion {
                    recoil: Recoil::Cut,
                    early: None,
                    settle_ticks: 2.0,
                },
            }),
        };
        let world = app.world_mut();
        world.resource_scope(|world, mut guns: Mut<RigGuns>| {
            world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
                let mut bindposes = world.resource_mut::<Assets<SkinnedMeshInverseBindposes>>();
                guns.add(gun, &mut meshes, &mut bindposes);
            });
        });
        let hero = app
            .world_mut()
            .spawn((
                HeroBody(HeroMotion::new(Vec3::ZERO, 0.0)),
                RigLook::for_player(0),
            ))
            .id();
        app.update();
        app.update();
        let fire = |app: &mut App, shots: u8, ticks: usize| {
            let mut body = app.world_mut().get_mut::<HeroBody>(hero).unwrap();
            body.0.gun.out = true;
            body.0.gun.item = Some(0x99);
            body.0.gun.shots = shots;
            for _ in 0..ticks {
                app.update();
            }
        };
        let height = |app: &mut App| {
            let mut named = app.world_mut().query::<(&Name, &Transform)>();
            named
                .iter(app.world())
                .find(|(n, _)| n.as_str() == "gun joint 0")
                .map(|(_, t)| t.translation.y)
                .unwrap()
        };
        fire(&mut app, 0, 2);
        let mut skinned = app.world_mut().query::<&SkinnedMesh>();
        let own: Vec<usize> = skinned
            .iter(app.world())
            .map(|s| s.joints.len())
            .filter(|&n| n != joint::COUNT)
            .collect();
        assert_eq!(own, [2], "the gun's surface on its own two joints");
        assert!((height(&mut app) - 1.0).abs() < 1e-5, "growing in");
        fire(&mut app, 0, 20);
        assert!((height(&mut app) - 2.0).abs() < 1e-5, "at rest");
        fire(&mut app, 1, 1);
        assert!((height(&mut app) - 3.0).abs() < 1e-5, "recoiling");
        fire(&mut app, 1, 20);
        assert!((height(&mut app) - 2.0).abs() < 1e-5, "settled again");
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

//! Models the rig can wear (PLAN.md §5, 2026-10-13): the open blocky model, or any other
//! mesh skinned to the rig's skeleton, such as a game's hero imported from the user's own
//! disc.
//!
//! A model is a [`ModelData`]: where each rig joint sits at rest in its proportions, and
//! surfaces whose skin weights name rig joints ([`crate::model::joint`]). Because every
//! model shares the skeleton, the same procedural poses and clips drive any of them.
//! [`RigModels`] holds the registered models and the one heroes wear; changing it
//! re-dresses them.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::hull::convex_hull;
use crate::model::{self, JointDef, Look, joint};
use crate::shapes::at;

/// The open model's name.
pub const OPEN_MODEL: &str = "open";

/// What a surface is drawn in.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// One of the player's colours ([`crate::RigLook`]).
    Look(Look),
    /// The model's own material. Players after the first get a copy tinted with their
    /// main colour.
    Own(Handle<StandardMaterial>),
}

/// One mesh of a model in the hero's space at rest, skinned to the rig's joints.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    pub paint: Paint,
    /// Whether it's the wrench, which hides while thrown and flies as a copy.
    pub wrench: bool,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Option<Vec<Vec2>>,
    /// Up to four rig joints per vertex, with their weights.
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    /// Triangles: three vertex indices each.
    pub indices: Vec<u32>,
}

impl Surface {
    pub fn new(paint: Paint, wrench: bool) -> Self {
        Self {
            paint,
            wrench,
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: None,
            joints: Vec::new(),
            weights: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Adds flat-shaded triangles that move with one joint.
    pub fn push_rigid(&mut self, triangles: &[[Vec3; 3]], joint: usize) {
        for [a, b, c] in triangles {
            let normal = (*b - *a).cross(*c - *a).normalize_or_zero();
            for corner in [a, b, c] {
                self.indices.push(self.positions.len() as u32);
                self.positions.push(*corner);
                self.normals.push(normal);
                self.joints.push([joint as u16, 0, 0, 0]);
                self.weights.push([1.0, 0.0, 0.0, 0.0]);
            }
        }
    }

    fn mesh(&self, skinned: bool, shift: Vec3) -> Mesh {
        let positions: Vec<[f32; 3]> = self.positions.iter().map(|p| (*p - shift).into()).collect();
        let normals: Vec<[f32; 3]> = self.normals.iter().map(|n| n.to_array()).collect();
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(self.indices.clone()));
        if let Some(uvs) = &self.uvs {
            let uvs: Vec<[f32; 2]> = uvs.iter().map(|uv| uv.to_array()).collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        }
        if skinned {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_JOINT_INDEX,
                VertexAttributeValues::Uint16x4(self.joints.clone()),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, self.weights.clone());
        }
        mesh
    }
}

/// A model before it's registered.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelData {
    /// Short, for the command line and logs: `open`, `uya`.
    pub name: String,
    /// What to call it on screen.
    pub title: String,
    /// Where each rig joint sits at rest, in the hero's space ([`joint`] order).
    pub rest: Vec<Vec3>,
    pub surfaces: Vec<Surface>,
    /// Where the thrown wrench spins, in the hero's space at rest.
    pub wrench_spin: Vec3,
}

/// Why a model can't be worn.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ModelError {
    #[error("{model}: {found} joint rest positions, but the rig has {expected} joints")]
    Rest {
        model: String,
        found: usize,
        expected: usize,
    },
    #[error("{model}: surface {surface} has {found} {what} for {vertices} vertices")]
    Attribute {
        model: String,
        surface: usize,
        what: &'static str,
        found: usize,
        vertices: usize,
    },
    #[error("{model}: surface {surface} names joint {joint}, but the rig has {count}")]
    Joint {
        model: String,
        surface: usize,
        joint: u16,
        count: usize,
    },
    #[error("{model}: a vertex of surface {surface} has weights summing to {sum}")]
    Weights {
        model: String,
        surface: usize,
        sum: f32,
    },
    #[error("{model}: surface {surface}'s triangles are malformed")]
    Triangles { model: String, surface: usize },
    #[error("{model}: {error}")]
    Bounds { model: String, error: String },
}

impl ModelData {
    /// Checks that the model fits the rig's skeleton: a rest position for every joint, and
    /// weights on rig joints that add up to one.
    pub fn check(&self) -> Result<(), ModelError> {
        let model = || self.name.clone();
        if self.rest.len() != joint::COUNT {
            return Err(ModelError::Rest {
                model: model(),
                found: self.rest.len(),
                expected: joint::COUNT,
            });
        }
        for (index, surface) in self.surfaces.iter().enumerate() {
            let vertices = surface.positions.len();
            let lengths = [
                ("normals", surface.normals.len()),
                ("joint sets", surface.joints.len()),
                ("weight sets", surface.weights.len()),
                ("UVs", surface.uvs.as_ref().map_or(vertices, Vec::len)),
            ];
            for (what, found) in lengths {
                if found != vertices {
                    return Err(ModelError::Attribute {
                        model: model(),
                        surface: index,
                        what,
                        found,
                        vertices,
                    });
                }
            }
            for (joints, weights) in surface.joints.iter().zip(&surface.weights) {
                for (&joint, &weight) in joints.iter().zip(weights) {
                    if weight != 0.0 && joint as usize >= model::joint::COUNT {
                        return Err(ModelError::Joint {
                            model: model(),
                            surface: index,
                            joint,
                            count: joint::COUNT,
                        });
                    }
                }
                let sum: f32 = weights.iter().sum();
                if (sum - 1.0).abs() > 1e-3 || weights.iter().any(|w| *w < 0.0) {
                    return Err(ModelError::Weights {
                        model: model(),
                        surface: index,
                        sum,
                    });
                }
            }
            if !surface.indices.len().is_multiple_of(3)
                || surface.indices.iter().any(|&i| i as usize >= vertices)
            {
                return Err(ModelError::Triangles {
                    model: model(),
                    surface: index,
                });
            }
        }
        Ok(())
    }

    /// The rig's skeleton in this model's proportions.
    pub fn skeleton(&self) -> Vec<JointDef> {
        model::hero()
            .joints
            .into_iter()
            .zip(&self.rest)
            .map(|(joint, rest)| JointDef {
                rest: *rest,
                ..joint
            })
            .collect()
    }
}

/// The open model: the blocky gold Ratchet, Clank and wrench of [`crate::model`], every
/// block moving rigidly with its joint.
pub fn open_model() -> ModelData {
    let def = model::hero();
    let mut surfaces = Vec::new();
    for wrench in [false, true] {
        for look in [Look::Main, Look::Accent] {
            let mut surface = Surface::new(Paint::Look(look), wrench);
            for part in &def.parts {
                if part.look == look && (part.joint == joint::WRENCH) == wrench {
                    surface.push_rigid(&convex_hull(&part.points), part.joint);
                }
            }
            if !surface.indices.is_empty() {
                surfaces.push(surface);
            }
        }
    }
    ModelData {
        name: OPEN_MODEL.into(),
        title: "Blocky gold".into(),
        rest: def.joints.iter().map(|joint| joint.rest).collect(),
        surfaces,
        // 0.2 m up the shaft from the grip.
        wrench_spin: def.joints[joint::WRENCH].rest + model::wrench_turn() * at(0.2, -0.06, 0.0),
    }
}

/// A surface ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RigSurface {
    pub mesh: Handle<Mesh>,
    pub paint: Paint,
    pub wrench: bool,
}

/// A registered model: its meshes are assets, ready to put on any hero.
#[derive(Debug, Clone)]
pub struct RigModel {
    pub name: String,
    pub title: String,
    pub(crate) skeleton: Vec<JointDef>,
    pub(crate) surfaces: Vec<RigSurface>,
    /// The wrench's surfaces without skinning, centred where it spins.
    pub(crate) thrown: Vec<RigSurface>,
    pub(crate) inverse_bindposes: Handle<SkinnedMeshInverseBindposes>,
}

/// Every model the rig can wear, and which one heroes wear.
#[derive(Resource, Debug, Clone, Default)]
pub struct RigModels {
    models: Vec<RigModel>,
    chosen: Option<String>,
}

impl RigModels {
    /// Checks a model and adds it, replacing one of the same name.
    pub fn add(
        &mut self,
        data: ModelData,
        meshes: &mut Assets<Mesh>,
        bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
    ) -> Result<(), ModelError> {
        data.check()?;
        let bounds = |error: &dyn std::fmt::Display| ModelError::Bounds {
            model: data.name.clone(),
            error: error.to_string(),
        };
        let mut surfaces = Vec::new();
        let mut thrown = Vec::new();
        for surface in &data.surfaces {
            let mesh = surface
                .mesh(true, Vec3::ZERO)
                .with_generated_skinned_mesh_bounds()
                .map_err(|error| bounds(&error))?;
            surfaces.push(RigSurface {
                mesh: meshes.add(mesh),
                paint: surface.paint.clone(),
                wrench: surface.wrench,
            });
            if surface.wrench {
                thrown.push(RigSurface {
                    mesh: meshes.add(surface.mesh(false, data.wrench_spin)),
                    paint: surface.paint.clone(),
                    wrench: true,
                });
            }
        }
        // No joint is rotated at rest, so each inverse bind pose only undoes the position.
        let inverse: Vec<Mat4> = data
            .rest
            .iter()
            .map(|rest| Mat4::from_translation(-*rest))
            .collect();
        let model = RigModel {
            skeleton: data.skeleton(),
            name: data.name,
            title: data.title,
            surfaces,
            thrown,
            inverse_bindposes: bindposes.add(inverse),
        };
        match self.models.iter_mut().find(|m| m.name == model.name) {
            Some(existing) => *existing = model,
            None => self.models.push(model),
        }
        Ok(())
    }

    pub fn models(&self) -> &[RigModel] {
        &self.models
    }

    /// The model heroes wear: the chosen one, else the open one, else the first.
    pub fn chosen(&self) -> Option<&RigModel> {
        let named = |name: &str| self.models.iter().find(|m| m.name == name);
        self.chosen
            .as_deref()
            .and_then(named)
            .or_else(|| named(OPEN_MODEL))
            .or_else(|| self.models.first())
    }

    /// Asks for a model by name. One that isn't registered (yet) leaves the current one on.
    pub fn choose(&mut self, name: &str) {
        self.chosen = Some(name.to_string());
    }

    /// Moves on to the next model, and returns it.
    pub fn choose_next(&mut self) -> Option<&RigModel> {
        let current = self.chosen().map(|m| m.name.clone());
        let at = self
            .models
            .iter()
            .position(|m| Some(&m.name) == current.as_ref())?;
        let next = self.models[(at + 1) % self.models.len()].name.clone();
        self.chosen = Some(next);
        self.chosen()
    }
}

/// Registers the open model at start-up.
pub(crate) fn register_open_model(
    mut models: ResMut<RigModels>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    bindposes: Option<ResMut<Assets<SkinnedMeshInverseBindposes>>>,
) {
    let (Some(mut meshes), Some(mut bindposes)) = (meshes, bindposes) else {
        return;
    };
    // Its tests check that it fits.
    let fitted = models.add(open_model(), &mut meshes, &mut bindposes);
    debug_assert!(fitted.is_ok(), "{fitted:?}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_open_model_fits_the_rig() {
        let model = open_model();
        model.check().unwrap();
        assert_eq!(
            model.surfaces.len(),
            4,
            "body and wrench, in two colours each"
        );
        let wrench: usize = model
            .surfaces
            .iter()
            .filter(|s| s.wrench)
            .map(|s| s.positions.len())
            .sum();
        assert!(wrench > 0);
        for surface in model.surfaces.iter().filter(|s| s.wrench) {
            assert!(
                surface
                    .joints
                    .iter()
                    .all(|j| j[0] as usize == joint::WRENCH)
            );
        }
    }

    #[test]
    fn models_that_dont_fit_are_refused() {
        let mut model = open_model();
        model.rest.pop();
        assert!(matches!(model.check(), Err(ModelError::Rest { .. })));

        let mut model = open_model();
        model.surfaces[0].weights[0] = [0.5, 0.0, 0.0, 0.0];
        assert!(matches!(model.check(), Err(ModelError::Weights { .. })));

        let mut model = open_model();
        model.surfaces[0].joints[0] = [joint::COUNT as u16, 0, 0, 0];
        assert!(matches!(model.check(), Err(ModelError::Joint { .. })));

        let mut model = open_model();
        model.surfaces[0].normals.pop();
        assert!(matches!(model.check(), Err(ModelError::Attribute { .. })));

        let mut model = open_model();
        model.surfaces[0].indices.push(u32::MAX);
        assert!(matches!(model.check(), Err(ModelError::Triangles { .. })));
    }

    #[test]
    fn heroes_wear_the_chosen_model_or_the_open_one() {
        let mut meshes = Assets::<Mesh>::default();
        let mut bindposes = Assets::<SkinnedMeshInverseBindposes>::default();
        let mut models = RigModels::default();
        assert!(models.chosen().is_none());
        let mut other = open_model();
        other.name = "other".into();
        models.add(other, &mut meshes, &mut bindposes).unwrap();
        models
            .add(open_model(), &mut meshes, &mut bindposes)
            .unwrap();
        assert_eq!(models.chosen().unwrap().name, OPEN_MODEL);
        models.choose("missing");
        assert_eq!(models.chosen().unwrap().name, OPEN_MODEL);
        models.choose("other");
        assert_eq!(models.chosen().unwrap().name, "other");
        assert_eq!(models.choose_next().unwrap().name, OPEN_MODEL);
        assert_eq!(models.choose_next().unwrap().name, "other");
        // Adding a model of a name already there replaces it.
        models
            .add(open_model(), &mut meshes, &mut bindposes)
            .unwrap();
        assert_eq!(models.models().len(), 2);
    }
}

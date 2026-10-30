//! What enemies leave when they die (PLAN.md P1.11): bolts and sometimes nanotech, from
//! their [`DropTable`]; never ammo, as UYA's enemies drop only bolts (`weapons.md`, "Ammo
//! pickups, crates and the vendor"). They are `rac_combat`'s pickups (P1.10,
//! [`rac_combat::pickups`]), which move and are collected there; this module also shows
//! every pickup, the crates' included, with placeholder shapes made in code.

use bevy::prelude::*;
use rac_combat::pickups::{
    self, BoltBank, BoltMultiplier, NanotechOrbs, Pickup, PickupKind, PickupTuning, kill_coins,
};
use rac_sim::SimRng;

use crate::enemy::brain::between;
use crate::enemy::def::DropTable;

/// What a dead enemy with `table` leaves, killed by a hero with the doubling `doubled`:
/// its bolts, the bank's share as UYA's kills pay it (`moby_killed`; all of it in coins,
/// the demo's enemies having no most), and nanotech by chance.
pub fn drops(
    table: &DropTable,
    doubled: &BoltMultiplier,
    bank: &mut BoltBank,
    rng: &mut SimRng,
) -> Vec<PickupKind> {
    let bolts = between(rng, table.bolts);
    let mut kinds = vec![PickupKind::Bolts(doubled.apply(table.bolt_value)); bolts as usize];
    kinds.extend(
        kill_coins(0, None, bank, rng)
            .into_iter()
            .map(PickupKind::Bolts),
    );
    if rng.unit_f32() < table.nanotech_chance {
        kinds.push(PickupKind::Nanotech(table.nanotech));
    }
    kinds
}

/// Meshes and materials for the pickups, made when the demo spawns.
#[derive(Resource, Debug, Clone)]
pub struct PickupLooks {
    bolt: Handle<Mesh>,
    bolt_look: Handle<StandardMaterial>,
    nanotech: Handle<Mesh>,
    nanotech_look: Handle<StandardMaterial>,
    ammo: Handle<Mesh>,
    ammo_look: Handle<StandardMaterial>,
}

impl PickupLooks {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        Self {
            // A hex nut.
            bolt: meshes.add(Cylinder::new(0.12, 0.07).mesh().resolution(6)),
            bolt_look: materials.add(StandardMaterial {
                base_color: Color::srgb(0.85, 0.8, 0.6),
                metallic: 1.0,
                perceptual_roughness: 0.3,
                ..default()
            }),
            nanotech: meshes.add(Sphere::new(0.16)),
            nanotech_look: materials.add(StandardMaterial {
                base_color: Color::srgb(0.3, 1.0, 0.6),
                emissive: LinearRgba::rgb(0.4, 2.0, 1.0),
                ..default()
            }),
            // A glowing blue cartridge, apart from the gold bolts and green nanotech.
            ammo: meshes.add(Cuboid::new(0.16, 0.28, 0.16)),
            ammo_look: ammo_look(materials),
        }
    }
}

impl PickupLooks {
    /// A nanotech orb's mesh and look.
    pub(crate) fn nanotech(&self) -> (Mesh3d, MeshMaterial3d<StandardMaterial>) {
        (
            Mesh3d(self.nanotech.clone()),
            MeshMaterial3d(self.nanotech_look.clone()),
        )
    }
}

/// One of a nanotech crate's orbs, by its place in its cluster.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct OrbLook(usize);

/// The orbs' size beside a nanotech pickup's.
const ORB_SCALE: f32 = 0.6;

/// Gives a broken nanotech crate's orbs their looks.
pub(crate) fn dress_orbs(
    looks: Res<PickupLooks>,
    clusters: Query<(Entity, &NanotechOrbs), Added<NanotechOrbs>>,
    mut commands: Commands,
) {
    for (entity, cluster) in &clusters {
        commands.entity(entity).insert(Visibility::default());
        for index in 0..cluster.0.orbs.len() {
            commands.spawn((
                OrbLook(index),
                looks.nanotech(),
                Transform::from_scale(Vec3::splat(ORB_SCALE)),
                ChildOf(entity),
            ));
        }
    }
}

/// Puts each orb where its cluster has it, and hides those that have arrived.
pub(crate) fn place_orbs(
    clusters: Query<&NanotechOrbs>,
    mut orbs: Query<(&OrbLook, &ChildOf, &mut Transform, &mut Visibility)>,
) {
    for (look, parent, mut transform, mut visibility) in &mut orbs {
        let Some(orb) = clusters
            .get(parent.parent())
            .ok()
            .and_then(|cluster| Some((cluster.0.middle, *cluster.0.orbs.get(look.0)?)))
        else {
            continue;
        };
        let (middle, orb) = orb;
        transform.translation = orb.position - middle;
        visibility.set_if_neq(if orb.arrived {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
}

/// The ammo pickups' look, which the ammo crates' band shares.
pub(crate) fn ammo_look(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.6, 1.0),
        emissive: LinearRgba::rgb(0.9, 1.8, 4.0),
        unlit: true,
        ..default()
    })
}

/// Turns a pickup about its axis.
#[derive(Component, Debug, Default, Clone, Copy)]
pub(crate) struct Spin;

/// Gives new pickups their looks.
pub(crate) fn dress(
    looks: Res<PickupLooks>,
    pickups: Query<(Entity, &Pickup), Added<Pickup>>,
    mut commands: Commands,
) {
    for (entity, pickup) in &pickups {
        let (mesh, look) = match pickup.0.kind {
            PickupKind::Bolts(_) => (&looks.bolt, &looks.bolt_look),
            PickupKind::Nanotech(_) => (&looks.nanotech, &looks.nanotech_look),
            PickupKind::Ammo { .. } => (&looks.ammo, &looks.ammo_look),
        };
        commands.entity(entity).insert(Visibility::default());
        commands.spawn((
            Spin,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(look.clone()),
            Transform::default(),
            ChildOf(entity),
        ));
    }
}

/// Blinks pickups about to vanish.
pub(crate) fn blink(tuning: Res<PickupTuning>, mut pickups: Query<(&Pickup, &mut Visibility)>) {
    for (pickup, mut visibility) in &mut pickups {
        visibility.set_if_neq(if pickups::shows(&pickup.0, &tuning) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// Turns pickups, tipped so the bolts' faces show, at the size they shrink to as they
/// fly in.
pub(crate) fn spin(
    time: Res<Time>,
    pickups: Query<&Pickup>,
    mut spinning: Query<(&mut Transform, &ChildOf), With<Spin>>,
) {
    let rotation = Quat::from_rotation_y(time.elapsed_secs() * 4.0) * Quat::from_rotation_x(1.1);
    for (mut transform, parent) in &mut spinning {
        transform.rotation = rotation;
        let scale = pickups
            .get(parent.parent())
            .map_or(1.0, |p| p.0.scale.max(0.0));
        transform.scale = Vec3::splat(scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: DropTable = DropTable {
        bolts: (4, 7),
        bolt_value: 5,
        nanotech_chance: 0.5,
        nanotech: 2,
    };

    #[test]
    fn a_kill_leaves_its_tables_bolts_and_sometimes_nanotech() {
        let mut rng = SimRng::new(7, 0);
        let (mut counts, mut nanotech) = (Vec::new(), 0);
        for _ in 0..400 {
            let kinds = drops(
                &TABLE,
                &BoltMultiplier::default(),
                &mut BoltBank::default(),
                &mut rng,
            );
            let bolts = kinds
                .iter()
                .filter(|kind| **kind == PickupKind::Bolts(5))
                .count();
            counts.push(bolts);
            let orbs = kinds
                .iter()
                .filter(|kind| **kind == PickupKind::Nanotech(2))
                .count();
            nanotech += orbs;
            assert_eq!(kinds.len(), bolts + orbs);
        }
        assert_eq!(counts.iter().min(), Some(&4));
        assert_eq!(counts.iter().max(), Some(&7));
        assert!((140..260).contains(&nanotech), "{nanotech}");
    }
}

//! What enemies leave when they die (PLAN.md P1.11): bolts, sometimes nanotech, and
//! sometimes ammo for a gun of the hero who killed them, from their [`DropTable`]. They are
//! `rac_combat`'s pickups (P1.10, [`rac_combat::pickups`]), which move and are collected
//! there; this module also shows every pickup, the crates' included, with placeholder
//! shapes made in code.

use bevy::prelude::*;
use rac_combat::pickups::{self, Pickup, PickupKind, PickupTuning};
use rac_combat::{Armory, Arsenal};
use rac_sim::SimRng;

use crate::enemy::brain::between;
use crate::enemy::def::DropTable;

/// What a dead enemy with `table` leaves, killed by a hero with the guns `killer`: its
/// bolts, nanotech by chance, and by chance ammo for a gun of his
/// ([`pickups::ammo_for`]).
pub fn drops(
    table: &DropTable,
    killer: Option<&Arsenal>,
    armory: &Armory,
    rng: &mut SimRng,
) -> Vec<PickupKind> {
    let bolts = between(rng, table.bolts);
    let mut kinds = vec![PickupKind::Bolts(table.bolt_value); bolts as usize];
    if rng.unit_f32() < table.nanotech_chance {
        kinds.push(PickupKind::Nanotech(table.nanotech));
    }
    if rng.unit_f32() < table.ammo_chance
        && let Some(arsenal) = killer
        && let Some(ammo) = pickups::ammo_for(arsenal, armory, rng)
            .and_then(|weapon| PickupKind::ammo(weapon, armory))
    {
        kinds.push(ammo);
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

/// Turns pickups, tipped so the bolts' faces show.
pub(crate) fn spin(time: Res<Time>, mut spinning: Query<&mut Transform, With<Spin>>) {
    let rotation = Quat::from_rotation_y(time.elapsed_secs() * 4.0) * Quat::from_rotation_x(1.1);
    for mut transform in &mut spinning {
        transform.rotation = rotation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_combat::WeaponId;
    use rac_combat::weapons::Hand;

    const N60: WeaponId = WeaponId(1);

    const TABLE: DropTable = DropTable {
        bolts: (4, 7),
        bolt_value: 5,
        nanotech_chance: 0.5,
        nanotech: 2,
        ammo_chance: 0.25,
    };

    #[test]
    fn a_kill_leaves_its_tables_bolts_and_sometimes_nanotech_and_ammo() {
        let armory = Armory::uya();
        let mut killer = Arsenal::default();
        killer.give(N60, &armory);
        killer.gun_mut(N60).unwrap().ammo = 10;
        killer.hand = Hand::Gun {
            weapon: N60,
            ticks: 30,
        };
        let n60 = PickupKind::ammo(N60, &armory).unwrap();
        let mut rng = SimRng::new(7, 0);
        let (mut counts, mut nanotech, mut ammo) = (Vec::new(), 0, 0);
        for _ in 0..400 {
            let kinds = drops(&TABLE, Some(&killer), &armory, &mut rng);
            let bolts = kinds
                .iter()
                .filter(|kind| **kind == PickupKind::Bolts(5))
                .count();
            counts.push(bolts);
            nanotech += kinds
                .iter()
                .filter(|kind| **kind == PickupKind::Nanotech(2))
                .count();
            ammo += kinds.iter().filter(|kind| **kind == n60).count();
            assert_eq!(kinds.len(), bolts + nanotech_and_ammo(&kinds, n60));
        }
        assert_eq!(counts.iter().min(), Some(&4));
        assert_eq!(counts.iter().max(), Some(&7));
        assert!((140..260).contains(&nanotech), "{nanotech}");
        assert!((60..140).contains(&ammo), "{ammo}");
        // A killer without guns gets no ammo.
        for _ in 0..100 {
            let kinds = drops(&TABLE, None, &armory, &mut rng);
            assert!(!kinds.contains(&n60));
        }
    }

    fn nanotech_and_ammo(kinds: &[PickupKind], ammo: PickupKind) -> usize {
        kinds
            .iter()
            .filter(|kind| **kind == PickupKind::Nanotech(2) || **kind == ammo)
            .count()
    }
}

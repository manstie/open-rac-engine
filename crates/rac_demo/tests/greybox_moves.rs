//! The greybox level played with the hero controller: each test area can be done with
//! the moves it is built for (PLAN.md P1.4, P1.5).

use bevy::prelude::*;
use rac_demo::{CRATE_HALF, DUMMY_HEIGHT, Shape, TargetKind, greybox_level, melee_targets};
use rac_hero::{
    BodyPlacement, Collider, HeroMotion, HeroProfile, HeroState, JumpKind, LockOn, ParryWorld,
    controller::step, lock_on, water::WaterVolume,
};
use rac_input::{Action, Buttons, PlayerInput};
use std::f32::consts::{FRAC_PI_2, PI};

/// Every solid box of the level, with moving ones where they start.
fn level_world() -> ParryWorld {
    let rails = greybox_level()
        .iter()
        .filter_map(|piece| piece.rail())
        .collect();
    let water = greybox_level()
        .iter()
        .filter_map(|piece| {
            let water = piece.water()?;
            Some(WaterVolume::new(piece.translation, water.half_extents))
        })
        .collect();
    ParryWorld::new(&level_bodies())
        .with_rails(rails)
        .with_water(water)
}

fn level_bodies() -> Vec<BodyPlacement> {
    greybox_level()
        .into_iter()
        .filter(|piece| piece.surface.is_solid())
        .filter_map(|piece| match piece.shape {
            Shape::Box { half_extents } => Some(BodyPlacement {
                material: piece.surface.material(),
                ..BodyPlacement::fixed(
                    Collider::Cuboid { half_extents },
                    piece
                        .mover
                        .map_or(piece.translation, |mover| mover.position_at(0)),
                    piece.rotation,
                )
            }),
            _ => None,
        })
        .collect()
}

fn input(stick: Option<[i8; 2]>, held: &[Action], pressed: &[Action]) -> PlayerInput {
    let mut held_buttons = Buttons::NONE;
    let mut pressed_buttons = Buttons::NONE;
    for &action in held.iter().chain(pressed) {
        held_buttons.insert(action);
    }
    for &action in pressed {
        pressed_buttons.insert(action);
    }
    PlayerInput {
        move_stick: stick.unwrap_or([0, 0]),
        look: [0, 0],
        look_stick: [0, 0],
        held: held_buttons,
        pressed: pressed_buttons,
    }
}

#[test]
fn the_wall_jump_shaft_leads_onto_the_high_platform() {
    let world = level_world();
    let profile = HeroProfile::uya();
    // At the foot of the taller left wall (its face is at x = 20.75), facing it.
    let mut hero = HeroMotion::new(Vec3::new(21.25, 0.0, -36.0), FRAC_PI_2);
    let press = input(None, &[], &[Action::Jump]);
    let mut wall_jumps = 0;
    for _ in 0..1500 {
        step(&mut hero, &press, 0.0, &profile, &world);
        if hero.state == HeroState::Jump(JumpKind::Wall) && hero.ticks == 1 {
            wall_jumps += 1;
        }
        if wall_jumps > 0 && hero.ground.is_some() {
            break;
        }
    }
    assert!(
        hero.position.y > 11.9 && hero.position.x > 23.5,
        "ended at {} after {wall_jumps} wall jumps",
        hero.position
    );
}

#[test]
fn the_3_u_block_can_be_grabbed_and_climbed() {
    let world = level_world();
    let profile = HeroProfile::uya();
    // The "3 u ledge" block's front face is at z = −34; stand 0.5 u in front, facing it.
    let mut hero = HeroMotion::new(Vec3::new(-24.0, 0.0, -33.5), 0.0);
    step(
        &mut hero,
        &input(None, &[], &[Action::Jump]),
        0.0,
        &profile,
        &world,
    );
    for _ in 0..60 {
        step(
            &mut hero,
            &input(None, &[Action::Jump], &[]),
            0.0,
            &profile,
            &world,
        );
        if hero.state == HeroState::LedgeGrab {
            break;
        }
    }
    assert_eq!(hero.state, HeroState::LedgeGrab);
    for _ in 0..30 {
        step(&mut hero, &input(None, &[], &[]), 0.0, &profile, &world);
    }
    assert_eq!(hero.state, HeroState::LedgeHang);
    step(
        &mut hero,
        &input(None, &[], &[Action::Jump]),
        0.0,
        &profile,
        &world,
    );
    for _ in 0..120 {
        step(&mut hero, &input(None, &[], &[]), 0.0, &profile, &world);
        if hero.ground.is_some() {
            break;
        }
    }
    assert!(hero.position.y > 2.9, "ended at {}", hero.position);
}

#[test]
fn the_grind_rail_carries_the_hero_up_to_its_end() {
    let world = level_world();
    let profile = HeroProfile::uya();
    // Under the rail near its low end, which runs along -z at x = 36 from 1.2 u up to 4 u.
    let mut hero = HeroMotion::new(Vec3::new(36.0, 0.0, -6.0), 0.0);
    step(&mut hero, &input(None, &[], &[]), 0.0, &profile, &world);
    step(
        &mut hero,
        &input(None, &[], &[Action::Jump]),
        0.0,
        &profile,
        &world,
    );
    let held = input(None, &[Action::Jump], &[]);
    let mut ticks = 0;
    while hero.state != HeroState::Grind {
        step(&mut hero, &held, 0.0, &profile, &world);
        ticks += 1;
        assert!(
            ticks < 120,
            "never got on the rail: {:?} at {}",
            hero.state,
            hero.position
        );
    }
    let mut highest_grinding = 0.0f32;
    while hero.state == HeroState::Grind {
        step(&mut hero, &input(None, &[], &[]), 0.0, &profile, &world);
        if hero.state == HeroState::Grind {
            highest_grinding = highest_grinding.max(hero.position.y);
        }
        ticks += 1;
        assert!(ticks < 600, "still grinding at {}", hero.position);
    }
    // Off the top end, near its 4.08 u.
    assert!(highest_grinding > 3.9, "{highest_grinding}");
    assert!(hero.position.z < -43.0, "{}", hero.position);
    assert_eq!(hero.state, HeroState::Fall);
}

#[test]
fn the_zip_line_carries_the_hero_down_from_the_high_platform() {
    let world = level_world();
    let profile = HeroProfile::uya();
    // On the high platform (top at 12 u), under the cable's start 2.2 u up, facing along
    // it (-z).
    let mut hero = HeroMotion::new(Vec3::new(28.25, 12.0, -35.0), 0.0);
    step(&mut hero, &input(None, &[], &[]), 0.0, &profile, &world);
    step(
        &mut hero,
        &input(None, &[], &[Action::Jump]),
        0.0,
        &profile,
        &world,
    );
    let held = input(None, &[Action::Jump], &[]);
    let mut ticks = 0;
    while hero.state != HeroState::ZipLine {
        step(&mut hero, &held, 0.0, &profile, &world);
        ticks += 1;
        assert!(
            ticks < 60,
            "never caught the cable: {:?} at {}",
            hero.state,
            hero.position
        );
    }
    while hero.state == HeroState::ZipLine {
        step(&mut hero, &input(None, &[], &[]), 0.0, &profile, &world);
        ticks += 1;
        assert!(ticks < 600, "still on the cable at {}", hero.position);
    }
    // Let go past the cable's end at z = -84, 3 u up, then down to the floor.
    assert!(hero.position.z < -84.0, "{}", hero.position);
    while hero.ground.is_none() {
        step(&mut hero, &input(None, &[], &[]), 0.0, &profile, &world);
        ticks += 1;
        assert!(ticks < 900, "never landed: {}", hero.position);
    }
    assert!(hero.position.y.abs() < 0.01, "{}", hero.position);
}

#[test]
fn the_pool_is_climbed_into_swum_across_and_jumped_out_of() {
    let world = level_world();
    let profile = HeroProfile::uya();
    let forward = input(Some([0, 127]), &[], &[]);
    // At the foot of the pool's steps, which climb along -z to its near wall at x = -50.
    let mut hero = HeroMotion::new(Vec3::new(-50.0, 0.0, -17.0), 0.0);
    let mut ticks = 0;
    while hero.state != HeroState::SurfaceIdle {
        step(&mut hero, &forward, 0.0, &profile, &world);
        ticks += 1;
        assert!(
            ticks < 600,
            "never got in: {:?} at {}",
            hero.state,
            hero.position
        );
    }
    // Swim across to the far wall, whose inside face is at z = -36.
    for _ in 0..600 {
        step(&mut hero, &forward, 0.0, &profile, &world);
    }
    assert_eq!(hero.state, HeroState::SurfaceSwim);
    assert!(hero.position.z < -35.0, "{}", hero.position);
    // The water jump clears the 4 u wall: the feet float at 3.38 and it rises about 1.5.
    step(
        &mut hero,
        &input(Some([0, 127]), &[], &[Action::Jump]),
        0.0,
        &profile,
        &world,
    );
    assert_eq!(hero.state, HeroState::Jump(JumpKind::Water));
    let held = input(Some([0, 127]), &[Action::Jump], &[]);
    let mut ticks = 0;
    while hero.ground.is_none() {
        step(&mut hero, &held, 0.0, &profile, &world);
        ticks += 1;
        assert!(
            ticks < 240,
            "never landed: {:?} at {}",
            hero.state,
            hero.position
        );
    }
    assert!(!hero.state.is_swimming());
    assert!(hero.position.z < -36.0, "{}", hero.position);
}

#[test]
fn the_melee_yard_has_a_dummy_to_lock_on_to() {
    // The yard's crates are solid, as in the demo, and each target is aimed at its middle.
    let mut bodies = level_bodies();
    let mut targets = Vec::new();
    for (index, target) in melee_targets().into_iter().enumerate() {
        let middle = match target.kind {
            TargetKind::Crate(_) => {
                let centre = target.position + Vec3::Y * CRATE_HALF;
                bodies.push(BodyPlacement::fixed(
                    Collider::Cuboid {
                        half_extents: Vec3::splat(CRATE_HALF),
                    },
                    centre,
                    Quat::IDENTITY,
                ));
                centre
            }
            TargetKind::Dummy => target.position + Vec3::Y * DUMMY_HEIGHT / 2.0,
        };
        targets.push((Entity::from_raw_u32(index as u32 + 1).unwrap(), middle));
    }
    let world = ParryWorld::new(&bodies);
    let profile = HeroProfile::uya();
    // Past the crates on the right, facing the dummies with the camera behind him.
    let hero = HeroMotion::new(Vec3::new(6.0, 0.0, 9.0), PI);
    let mut lock = LockOn::default();
    lock_on::update(
        &mut lock,
        &hero,
        &input(None, &[Action::LockOn], &[Action::LockOn]),
        PI,
        &targets,
        &profile.lock_on,
        &world,
    );
    let dummy = melee_targets()
        .iter()
        .position(|target| target.name == "dummy 2")
        .unwrap();
    assert_eq!(lock.target, Some(targets[dummy].0));
}

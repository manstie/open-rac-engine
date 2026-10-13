//! The greybox level played with the hero controller: each test area can be done with
//! the moves it is built for (PLAN.md P1.4, P1.5).

use bevy::prelude::*;
use rac_demo::{Shape, greybox_level};
use rac_hero::{
    BodyPlacement, Collider, HeroMotion, HeroProfile, HeroState, JumpKind, ParryWorld,
    controller::step, water::WaterVolume,
};
use rac_input::{Action, Buttons, PlayerInput};
use std::f32::consts::FRAC_PI_2;

/// Every solid box of the level, with moving ones where they start.
fn level_world() -> ParryWorld {
    let bodies: Vec<BodyPlacement> = greybox_level()
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
        .collect();
    let rails = greybox_level()
        .iter()
        .filter_map(|piece| piece.grind_rail())
        .collect();
    let water = greybox_level()
        .iter()
        .filter_map(|piece| {
            let water = piece.water()?;
            Some(WaterVolume::new(piece.translation, water.half_extents))
        })
        .collect();
    ParryWorld::new(&bodies).with_rails(rails).with_water(water)
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

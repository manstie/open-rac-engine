//! The simulation core shared by the hero kit (PLAN.md P1.2).
//!
//! Gameplay runs in `FixedUpdate` at [`TICK_HZ`], in the order given by [`SimSet`], and
//! must be deterministic: the same inputs (`rac_input::PlayerInput`) have to give
//! bit-identical state on every platform, because co-op exchanges only inputs (PLAN.md
//! section 4.2). This crate supplies the pieces every kit crate needs for that:
//!
//! - [`SimPlugin`]: the 60 Hz clock, the [`SimSet`] order and the [`SimTick`] counter;
//! - [`SimTransform`]: where a simulated entity really is, which the plugin interpolates
//!   into its `Transform` for rendering;
//! - [`SimRng`]: the per-level random number generator.
//!
//! Simulation code uses `bevy::math::ops` for trigonometry (this crate turns on Bevy's
//! `libm` feature), never reads the wall clock, and never depends on hash-map order.

mod rng;
mod transform;

use bevy::prelude::*;

pub use rng::SimRng;
pub use transform::{PreviousSimTransform, SimTransform};

/// Simulation ticks per second. UYA and Deadlocked NTSC-U are tuned for 60 Hz, and the kit
/// uses their values (PLAN.md section 5, 2026-10-12).
pub const TICK_HZ: f64 = 60.0;

/// The order gameplay runs in within each tick, matching the games' own frame: pad, mobys,
/// hero, particles, camera (`docs/research/engine-notes.md`).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimSet {
    /// Latches this tick's `PlayerInput` for every player.
    Input,
    /// Enemies, moving platforms, pickups and other level objects (the games' "mobys").
    /// They move before the hero so the hero can stand on them.
    World,
    /// The heroes.
    Hero,
    /// Effects that follow the hero and the world.
    Particles,
    /// Cameras, which look at where everything ended up.
    Camera,
}

/// Ticks since the current level started. During a tick's `FixedUpdate` it holds that
/// tick's number, starting from 0.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SimTick(pub u64);

/// Sets up the fixed clock, the [`SimSet`] order, the tick counter and interpolation.
pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
            .init_resource::<SimTick>()
            .init_resource::<SimRng>()
            .configure_sets(
                FixedUpdate,
                (
                    SimSet::Input,
                    SimSet::World,
                    SimSet::Hero,
                    SimSet::Particles,
                    SimSet::Camera,
                )
                    .chain(),
            )
            .add_systems(FixedFirst, transform::remember_previous)
            .add_systems(FixedLast, advance_tick)
            .add_systems(
                RunFixedMainLoop,
                transform::interpolate.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            );
    }
}

fn advance_tick(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct Order(Vec<SimSet>);

    fn record(set: SimSet) -> impl FnMut(ResMut<Order>, Res<SimTick>) {
        move |mut order, tick| {
            // Only the first tick, so the expected order stays short.
            if tick.0 == 0 {
                order.0.push(set);
            }
        }
    }

    #[test]
    fn sets_run_in_game_order_once_per_tick() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimPlugin))
            .init_resource::<Order>()
            // Registered in reverse so only the set ordering can make them come out right.
            .add_systems(
                FixedUpdate,
                (
                    record(SimSet::Camera).in_set(SimSet::Camera),
                    record(SimSet::Particles).in_set(SimSet::Particles),
                    record(SimSet::Hero).in_set(SimSet::Hero),
                    record(SimSet::World).in_set(SimSet::World),
                    record(SimSet::Input).in_set(SimSet::Input),
                ),
            );

        // Drive the fixed loop by hand: three and a half ticks' worth of time in one frame.
        // (A tick is 16,666,667 ns after rounding, so exactly 3/60 s holds only two.)
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f64(3.5 / TICK_HZ),
        ));
        app.update(); // The first update only starts the clock.
        app.update();

        assert_eq!(
            app.world().resource::<Order>().0,
            [
                SimSet::Input,
                SimSet::World,
                SimSet::Hero,
                SimSet::Particles,
                SimSet::Camera,
            ]
        );
        assert_eq!(*app.world().resource::<SimTick>(), SimTick(3));
    }
}

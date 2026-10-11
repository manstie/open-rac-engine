//! Open Ratchet & Clank Engine: application entry point.
//!
//! For now this opens a window with a greybox placeholder scene. The roadmap lives
//! in `PLAN.md` at the repository root.

use bevy::{diagnostic::FrameCount, prelude::*, render::RenderPlugin};

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Open Ratchet & Clank Engine".into(),
                    ..default()
                }),
                ..default()
            })
            // Compile pipelines one at a time. With NVIDIA's 595.91.07 Linux driver,
            // Bevy's default parallel compilation makes the driver's shader compiler
            // fail ("NVVM compilation failed: 3"), losing the device and segfaulting
            // within a second of startup. Revisit when the driver or wgpu changes.
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            }),
    )
    .add_systems(Startup, spawn_placeholder_scene);

    if let Some(frames) = exit_after_frames_arg() {
        app.insert_resource(ExitAfterFrames(frames))
            .add_systems(Update, exit_after_frames);
    }

    app.run()
}

/// Quit once this many frames have run. Set with `--exit-after-frames N` so CI and
/// smoke tests can check that the app boots and renders without a human present.
#[derive(Resource)]
struct ExitAfterFrames(u32);

fn exit_after_frames_arg() -> Option<u32> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--exit-after-frames" {
            let count = args.next().and_then(|n| n.parse().ok());
            return Some(count.expect("--exit-after-frames needs a frame count, e.g. 120"));
        }
    }
    None
}

fn exit_after_frames(
    limit: Res<ExitAfterFrames>,
    frames: Res<FrameCount>,
    mut exit: MessageWriter<AppExit>,
) {
    if frames.0 >= limit.0 {
        exit.write(AppExit::Success);
    }
}

/// Stand-in until the Phase 1 demo level exists: ground, a capsule "player", a sun
/// and a camera.
fn spawn_placeholder_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(40.0, 40.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.35, 0.45, 0.35))),
    ));
    // Radius 0.4 + segment 1.0 = 1.8 units tall, so its centre sits at y = 0.9.
    commands.spawn((
        Mesh3d(meshes.add(Capsule3d::new(0.4, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.95, 0.6, 0.2))),
        Transform::from_xyz(0.0, 0.9, 0.0),
    ));
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.0, 8.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
}

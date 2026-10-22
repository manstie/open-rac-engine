//! The main menu, the pause menu and the in-game controls hint (PLAN.md P1.1).
//!
//! A menu is a column of buttons with one focused. The mouse focuses by hovering and
//! clicks to choose; keys and the pad move the focus with the menu actions and choose with
//! Accept. On the Model, Moves and Camera buttons, left and right step back and forth
//! through the hero's models, the games he can move like and the camera settings.
//! Settings screens and full UI navigation come with P1.12.

use bevy::{
    picking::hover::Hovered,
    prelude::*,
    ui_widgets::{Activate, Button, observe},
};
use rac_demo::DemoSettings;
use rac_hero::HeroTuning;
use rac_input::{Action, ActionState};
use rac_rig::RigModels;

use crate::camera;
use crate::flow::{AppState, Pause};
use crate::moves::{self, ChosenMoves};
use crate::settings::{self, Paths};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuCommand {
    PlayDemo,
    /// Puts the hero in the next model, and remembers it.
    NextModel,
    /// Puts the hero back in the previous model, and remembers it.
    PreviousModel,
    /// Makes the hero move like the next game, and remembers it.
    NextMoves,
    /// Makes the hero move like the previous game, and remembers it.
    PreviousMoves,
    /// Gives the hero's camera the next setting, and remembers it.
    NextCamera,
    /// Gives the hero's camera the previous setting, and remembers it.
    PreviousCamera,
    Resume,
    MainMenu,
    Quit,
}

/// The text of the button that changes the hero's model.
#[derive(Component, Debug)]
struct ModelLabel;

/// The text of the button that changes the game the hero moves like.
#[derive(Component, Debug)]
struct MovesLabel;

/// The text of the button that changes the camera's setting.
#[derive(Component, Debug)]
struct CameraLabel;

#[derive(Component, Debug, Clone, Copy)]
struct MenuItem {
    index: usize,
    command: MenuCommand,
}

/// The focused item of whichever menu is open.
#[derive(Resource, Debug, Default)]
struct Focus(usize);

/// A command chosen this frame, by click or by Accept.
#[derive(Resource, Debug, Default)]
struct Chosen(Option<MenuCommand>);

const TEXT: Color = Color::srgb(0.92, 0.92, 0.92);
const BUTTON: Color = Color::srgb(0.16, 0.17, 0.2);
const BUTTON_FOCUSED: Color = Color::srgb(0.85, 0.45, 0.1);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Focus>()
            .init_resource::<Chosen>()
            .init_resource::<ChosenMoves>()
            .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(OnEnter(Pause::Paused), spawn_pause_menu)
            .add_systems(OnEnter(AppState::InGame), spawn_controls_hint)
            .add_systems(
                Update,
                (
                    navigate,
                    focus_on_hover,
                    accept,
                    run_chosen,
                    show_focus,
                    label_model.run_if(resource_changed::<RigModels>),
                    label_new_model_buttons,
                    label_moves.run_if(resource_changed::<HeroTuning>),
                    label_new_moves_buttons,
                    label_camera.run_if(
                        resource_changed::<HeroTuning>.or_else(resource_changed::<DemoSettings>),
                    ),
                    label_new_camera_buttons,
                )
                    .chain(),
            );
    }
}

fn spawn_main_menu(
    mut commands: Commands,
    mut focus: ResMut<Focus>,
    disc: Option<Res<rac_rig::RigAnimations>>,
) {
    focus.0 = 0;
    let found = if disc.is_some() {
        "Your UYA disc gives the hero Ratchet's animations, and his model under Model."
    } else {
        "No supported disc found. The hero kit's demo needs none."
    };
    commands.spawn((Camera2d, DespawnOnExit(AppState::MainMenu)));
    commands.spawn((
        DespawnOnExit(AppState::MainMenu),
        menu_root(Color::srgb(0.05, 0.06, 0.08)),
        children![
            title("Open Ratchet & Clank Engine"),
            subtitle(found),
            button(0, "Play the demo", MenuCommand::PlayDemo),
            model_button(1),
            moves_button(2),
            camera_button(3),
            button(4, "Quit", MenuCommand::Quit),
        ],
    ));
}

fn spawn_pause_menu(mut commands: Commands, mut focus: ResMut<Focus>) {
    focus.0 = 0;
    commands.spawn((
        DespawnOnExit(Pause::Paused),
        menu_root(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        children![
            title("Paused"),
            button(0, "Resume", MenuCommand::Resume),
            model_button(1),
            moves_button(2),
            camera_button(3),
            button(4, "Main menu", MenuCommand::MainMenu),
            button(5, "Quit", MenuCommand::Quit),
        ],
    ));
}

const HERO_CONTROLS: &str = "Move: WASD / left stick. Look: mouse / right stick. Pause: Esc / Start.\n\
    Jump: Space / Cross. Again in the air: double jump; keep holding: glide.\n\
    Crouch: Ctrl / R1. Then jump: heli jump, or a long jump while running.\n\
    Crouch, then push sideways or back as you jump: flip.\n\
    Strafe: right mouse / L2 or R2. Then jump sideways or back: flip.\n\
    Look round: V / L1 (first person in lock-strafe). Camera setting: in the menus.\n\
    Wrench: F / Square, again in time for the combo. Crouch first: throw it.\n\
    Gun: left mouse / Circle brings it out and fires. Switch: Q, E or wheel / d-pad;\n\
    tap Tab / Triangle for the last guns; hold it and point the stick for the ring (Q, E:\n\
    step round it; Ctrl / R1: the other ring). Lock on (gamepad, not in lock-strafe, with\n\
    the gun's lock-on mod): L2 and R2 together.";

const SPECTATOR_CONTROLS: &str = "Spectator camera. Move: WASD / left stick. Look: mouse / right stick.\n\
    Up: Space / Cross. Down: Ctrl / R1. Faster: right mouse / L2 or R2.\n\
    Pause: Esc / Start.";

fn spawn_controls_hint(mut commands: Commands, demo: Res<DemoSettings>) {
    let text = if demo.spectator {
        SPECTATOR_CONTROLS
    } else {
        HERO_CONTROLS
    };
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Node {
            position_type: PositionType::Absolute,
            left: px(12),
            bottom: px(12),
            ..default()
        },
        children![(
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(TEXT),
        )],
    ));
}

fn menu_root(background: Color) -> impl Bundle {
    (
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(14),
            ..default()
        },
        BackgroundColor(background),
    )
}

fn title(text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(40.0),
            ..default()
        },
        TextColor(TEXT),
    )
}

fn subtitle(text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(TEXT.with_alpha(0.7)),
        Node {
            margin: UiRect::bottom(px(18)),
            ..default()
        },
    )
}

fn button(index: usize, label: &str, command: MenuCommand) -> impl Bundle {
    button_with(index, label, command, ())
}

/// Its label is filled in by [`label_model`].
fn model_button(index: usize) -> impl Bundle {
    button_with(index, "Model", MenuCommand::NextModel, ModelLabel)
}

/// Its label is filled in by [`label_moves`].
fn moves_button(index: usize) -> impl Bundle {
    button_with(index, "Moves", MenuCommand::NextMoves, MovesLabel)
}

/// Its label is filled in by [`label_camera`].
fn camera_button(index: usize) -> impl Bundle {
    button_with(index, "Camera", MenuCommand::NextCamera, CameraLabel)
}

fn button_with(
    index: usize,
    label: &str,
    command: MenuCommand,
    marker: impl Bundle,
) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        MenuItem { index, command },
        Node {
            width: px(420),
            height: px(56),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(BUTTON),
        observe(move |_: On<Activate>, mut chosen: ResMut<Chosen>| {
            chosen.0 = Some(command);
        }),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(24.0),
                ..default()
            },
            TextColor(TEXT),
            marker,
        )],
    )
}

fn label_model(models: Res<RigModels>, mut labels: Query<&mut Text, With<ModelLabel>>) {
    let title = models.chosen().map_or("none", |model| model.title.as_str());
    for mut text in &mut labels {
        text.0 = format!("Model: {title}");
    }
}

/// Labels a menu's model button as soon as it appears.
fn label_new_model_buttons(
    models: Res<RigModels>,
    labels: Query<(), Added<ModelLabel>>,
    texts: Query<&mut Text, With<ModelLabel>>,
) {
    if !labels.is_empty() {
        label_model(models, texts);
    }
}

fn label_moves(tuning: Res<HeroTuning>, mut labels: Query<&mut Text, With<MovesLabel>>) {
    for mut text in &mut labels {
        text.0 = format!("Moves: {}", tuning.0.name);
    }
}

/// Labels a menu's moves button as soon as it appears.
fn label_new_moves_buttons(
    tuning: Res<HeroTuning>,
    labels: Query<(), Added<MovesLabel>>,
    texts: Query<&mut Text, With<MovesLabel>>,
) {
    if !labels.is_empty() {
        label_moves(tuning, texts);
    }
}

fn label_camera(
    tuning: Res<HeroTuning>,
    demo: Res<DemoSettings>,
    mut labels: Query<&mut Text, With<CameraLabel>>,
) {
    for mut text in &mut labels {
        text.0 = camera::label(demo.camera, &tuning.0);
    }
}

/// Labels a menu's camera button as soon as it appears.
fn label_new_camera_buttons(
    tuning: Res<HeroTuning>,
    demo: Res<DemoSettings>,
    labels: Query<(), Added<CameraLabel>>,
    texts: Query<&mut Text, With<CameraLabel>>,
) {
    if !labels.is_empty() {
        label_camera(tuning, demo, texts);
    }
}

fn navigate(actions: Res<ActionState>, items: Query<&MenuItem>, mut focus: ResMut<Focus>) {
    let count = items.iter().count();
    if count == 0 {
        return;
    }
    if actions.just_pressed(Action::UiDown) {
        focus.0 = (focus.0 + 1) % count;
    }
    if actions.just_pressed(Action::UiUp) {
        focus.0 = (focus.0 + count - 1) % count;
    }
}

fn focus_on_hover(items: Query<(&MenuItem, &Hovered), Changed<Hovered>>, mut focus: ResMut<Focus>) {
    for (item, hovered) in &items {
        if hovered.get() {
            focus.0 = item.index;
        }
    }
}

fn accept(
    actions: Res<ActionState>,
    items: Query<&MenuItem>,
    focus: Res<Focus>,
    mut chosen: ResMut<Chosen>,
) {
    let Some(item) = items.iter().find(|item| item.index == focus.0) else {
        return;
    };
    // The buttons that step through a list go back with left.
    let previous = match item.command {
        MenuCommand::NextModel => Some(MenuCommand::PreviousModel),
        MenuCommand::NextMoves => Some(MenuCommand::PreviousMoves),
        MenuCommand::NextCamera => Some(MenuCommand::PreviousCamera),
        _ => None,
    };
    if actions.just_pressed(Action::UiAccept) {
        chosen.0 = Some(item.command);
    } else if let Some(previous) = previous {
        if actions.just_pressed(Action::UiRight) {
            chosen.0 = Some(item.command);
        } else if actions.just_pressed(Action::UiLeft) {
            chosen.0 = Some(previous);
        }
    }
}

fn run_chosen(
    mut chosen: ResMut<Chosen>,
    mut app_state: ResMut<NextState<AppState>>,
    mut pause: ResMut<NextState<Pause>>,
    mut models: ResMut<RigModels>,
    mut tuning: ResMut<HeroTuning>,
    mut chosen_moves: ResMut<ChosenMoves>,
    mut demo: ResMut<DemoSettings>,
    paths: Option<Res<Paths>>,
    mut exit: MessageWriter<AppExit>,
) {
    match chosen.0.take() {
        Some(MenuCommand::PlayDemo) => app_state.set(AppState::InGame),
        Some(command @ (MenuCommand::NextModel | MenuCommand::PreviousModel)) => {
            let model = if command == MenuCommand::NextModel {
                models.choose_next()
            } else {
                models.choose_previous()
            };
            let Some(model) = model else {
                return;
            };
            info!("the hero's model: {} ({})", model.title, model.name);
            let name = model.name.clone();
            if let Some(paths) = paths {
                let mut saved = settings::load_or_create(&paths.settings_file);
                saved.hero_model = Some(name);
                if let Err(error) = settings::save(&paths.settings_file, &saved) {
                    warn!("could not save the hero's model: {error:#}");
                }
            }
        }
        Some(command @ (MenuCommand::NextMoves | MenuCommand::PreviousMoves)) => {
            let name = moves::step(&chosen_moves.0, command == MenuCommand::NextMoves);
            let profile = match moves::load(name) {
                Ok(profile) => profile,
                Err(error) => {
                    warn!("{error:#}");
                    return;
                }
            };
            info!("the hero moves like {} ({name})", profile.name);
            tuning.0 = profile;
            chosen_moves.0 = name.to_owned();
            if let Some(paths) = paths {
                let mut saved = settings::load_or_create(&paths.settings_file);
                saved.hero_moves = Some(name.to_owned());
                if let Err(error) = settings::save(&paths.settings_file, &saved) {
                    warn!("could not save the game the hero moves like: {error:#}");
                }
            }
        }
        Some(command @ (MenuCommand::NextCamera | MenuCommand::PreviousCamera)) => {
            let choice = camera::step(demo.camera, command == MenuCommand::NextCamera);
            info!("the hero's camera: {}", camera::label(choice, &tuning.0));
            demo.camera = choice;
            if let Some(paths) = paths {
                let mut saved = settings::load_or_create(&paths.settings_file);
                saved.hero_camera = choice.map(|setting| camera::name(setting).to_owned());
                if let Err(error) = settings::save(&paths.settings_file, &saved) {
                    warn!("could not save the camera setting: {error:#}");
                }
            }
        }
        Some(MenuCommand::Resume) => pause.set(Pause::Running),
        Some(MenuCommand::MainMenu) => app_state.set(AppState::MainMenu),
        Some(MenuCommand::Quit) => {
            exit.write(AppExit::Success);
        }
        None => {}
    }
}

fn show_focus(focus: Res<Focus>, mut items: Query<(&MenuItem, &mut BackgroundColor)>) {
    for (item, mut background) in &mut items {
        let color = if item.index == focus.0 {
            BUTTON_FOCUSED
        } else {
            BUTTON
        };
        if background.0 != color {
            background.0 = color;
        }
    }
}

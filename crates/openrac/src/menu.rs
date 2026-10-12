//! The main menu, the pause menu and the in-game controls hint (PLAN.md P1.1).
//!
//! A menu is a column of buttons with one focused. The mouse focuses by hovering and
//! clicks to choose; keys and the pad move the focus with the menu actions and choose with
//! Accept. Settings screens and full UI navigation come with P1.12.

use bevy::{
    picking::hover::Hovered,
    prelude::*,
    ui_widgets::{Activate, Button, observe},
};
use rac_input::{Action, ActionState};

use crate::flow::{AppState, Pause};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuCommand {
    PlayDemo,
    Resume,
    MainMenu,
    Quit,
}

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
            .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(OnEnter(Pause::Paused), spawn_pause_menu)
            .add_systems(OnEnter(AppState::InGame), spawn_controls_hint)
            .add_systems(
                Update,
                (navigate, focus_on_hover, accept, run_chosen, show_focus).chain(),
            );
    }
}

fn spawn_main_menu(mut commands: Commands, mut focus: ResMut<Focus>) {
    focus.0 = 0;
    commands.spawn((Camera2d, DespawnOnExit(AppState::MainMenu)));
    commands.spawn((
        DespawnOnExit(AppState::MainMenu),
        menu_root(Color::srgb(0.05, 0.06, 0.08)),
        children![
            title("Open Ratchet & Clank Engine"),
            subtitle("No supported disc found. The hero kit's demo needs none."),
            button(0, "Play the demo", MenuCommand::PlayDemo),
            button(1, "Quit", MenuCommand::Quit),
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
            button(1, "Main menu", MenuCommand::MainMenu),
            button(2, "Quit", MenuCommand::Quit),
        ],
    ));
}

fn spawn_controls_hint(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Node {
            position_type: PositionType::Absolute,
            left: px(12),
            bottom: px(12),
            ..default()
        },
        children![(
            Text::new(
                "Spectator camera. Move: WASD / left stick. Look: mouse / right stick.\n\
                 Up: Space / Cross. Down: Ctrl / R1. Faster: right mouse / L2 or R2.\n\
                 Pause: Esc / Start."
            ),
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
    (
        Button,
        Hovered::default(),
        MenuItem { index, command },
        Node {
            width: px(280),
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
        )],
    )
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
    if actions.just_pressed(Action::UiAccept)
        && let Some(item) = items.iter().find(|item| item.index == focus.0)
    {
        chosen.0 = Some(item.command);
    }
}

fn run_chosen(
    mut chosen: ResMut<Chosen>,
    mut app_state: ResMut<NextState<AppState>>,
    mut pause: ResMut<NextState<Pause>>,
    mut exit: MessageWriter<AppExit>,
) {
    match chosen.0.take() {
        Some(MenuCommand::PlayDemo) => app_state.set(AppState::InGame),
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

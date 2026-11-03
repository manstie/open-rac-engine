//! The menus (PLAN.md P1.1, P1.12): the main menu, the pause menu, the options and the
//! controls.
//!
//! A page is a grid of items with one focused: buttons, choices that left and right step
//! through (the hero's model, moves and camera, and the options), and on the controls page
//! the bindings. The mouse focuses by hovering and clicks to choose; a right click steps a
//! choice back or clears a binding. Keys and the pad move the focus with the menu actions,
//! repeating while held, and choose with Accept. Back goes up a page, and from the pause
//! menu back to the game, as Pause does from any of its pages. A page taller than the
//! window scrolls with the wheel and follows the focus.

mod choices;
mod controls;
mod names;

use bevy::{
    input::{
        InputSystems,
        mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    },
    picking::{events::PointerClick, hover::Hovered, pointer::PointerButton},
    prelude::*,
    ui::UiGlobalTransform,
    ui_widgets::{Button, observe},
};
use rac_input::{Action, ActionState, BindingCapture, Slot};
use std::collections::BTreeSet;

use crate::flow::{AppState, Pause};
use crate::moves::ChosenMoves;
use choices::{Choice, Choices};
pub use names::KeyNames;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Main,
    Pause,
    Options,
    Controls,
}

/// The pages open, the last on show, each with the focus it had. Empty in play.
#[derive(Resource, Debug, Default)]
struct Pages(Vec<(Page, Focus)>);

/// The root of the page on show.
#[derive(Component, Debug)]
struct PageRoot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuCommand {
    PlayDemo,
    Resume,
    MainMenu,
    Quit,
    Open(Page),
    /// Up a page, or from the pause menu back to the game.
    Back,
    /// Steps a choice forwards (true) or back.
    Step(Choice, bool),
    /// Listens for a binding to put in the slot.
    Bind(Slot),
    Clear(Slot),
    ResetControls,
}

impl MenuCommand {
    /// What a right click does: steps a choice back, or clears a binding.
    fn reverse(self) -> Option<MenuCommand> {
        match self {
            MenuCommand::Step(choice, forward) => Some(MenuCommand::Step(choice, !forward)),
            MenuCommand::Bind(slot) => Some(MenuCommand::Clear(slot)),
            _ => None,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
struct MenuItem {
    row: usize,
    column: usize,
    command: MenuCommand,
}

impl MenuItem {
    fn at(&self) -> Focus {
        Focus {
            row: self.row,
            column: self.column,
        }
    }
}

/// The focused item of the page on show, by its row and column.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Focus {
    row: usize,
    column: usize,
}

/// A command chosen this frame, by click or by Accept.
#[derive(Resource, Debug, Default)]
struct Chosen(Option<MenuCommand>);

/// Set while a mouse button that was caught for a binding is still down, so it clicks
/// nothing as it comes up.
#[derive(Resource, Debug, Default)]
struct SwallowClick(bool);

/// A page's scrolling list.
#[derive(Component, Debug)]
struct MenuScroll;

/// An item in a page's scrolling list, which the list scrolls to when it is focused.
#[derive(Component, Debug)]
struct Scrolled;

/// A text showing a choice: with its name ("Model: Ratchet"), or its value alone.
#[derive(Component, Debug, Clone, Copy)]
struct ChoiceText {
    choice: Choice,
    named: bool,
}

const TEXT: Color = Color::srgb(0.92, 0.92, 0.92);
const BUTTON: Color = Color::srgb(0.16, 0.17, 0.2);
const BUTTON_FOCUSED: Color = Color::srgb(0.85, 0.45, 0.1);

/// How long a menu direction is held before it repeats, and then how often, in seconds.
const REPEAT_DELAY: f32 = 0.4;
const REPEAT_EVERY: f32 = 0.08;

/// How far a notch of the wheel scrolls a page, in logical pixels.
const WHEEL_LINE: f32 = 40.0;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pages>()
            .init_resource::<Focus>()
            .init_resource::<Chosen>()
            .init_resource::<SwallowClick>()
            .init_resource::<ChosenMoves>()
            .init_resource::<controls::Rebinding>()
            .init_resource::<controls::Notice>()
            .init_resource::<KeyNames>()
            .add_systems(OnEnter(AppState::MainMenu), open_main_menu)
            .add_systems(OnEnter(Pause::Paused), open_pause_menu)
            .add_systems(OnExit(AppState::MainMenu), close_pages)
            .add_systems(OnExit(Pause::Paused), close_pages)
            .add_systems(PreUpdate, names::learn_key_names.after(InputSystems))
            .add_systems(
                Update,
                (
                    show_page.run_if(resource_changed::<Pages>),
                    controls::catch,
                    unswallow_click,
                    fix_focus,
                    navigate,
                    focus_on_hover,
                    accept,
                    run_chosen,
                    fix_focus,
                    show_focus,
                    label_choices,
                    controls::label_slots,
                    controls::show_notice,
                    scroll,
                )
                    .chain(),
            );
    }
}

fn open_main_menu(mut commands: Commands, mut pages: ResMut<Pages>) {
    commands.spawn((Camera2d, DespawnOnExit(AppState::MainMenu)));
    pages.0 = vec![(Page::Main, Focus::default())];
}

fn open_pause_menu(mut pages: ResMut<Pages>) {
    pages.0 = vec![(Page::Pause, Focus::default())];
}

fn close_pages(
    mut commands: Commands,
    mut pages: ResMut<Pages>,
    mut capture: ResMut<BindingCapture>,
    mut rebinding: ResMut<controls::Rebinding>,
    roots: Query<Entity, With<PageRoot>>,
) {
    pages.0.clear();
    capture.stop();
    rebinding.slot = None;
    for root in &roots {
        commands.entity(root).despawn();
    }
}

fn show_page(
    mut commands: Commands,
    pages: Res<Pages>,
    roots: Query<Entity, With<PageRoot>>,
    mut focus: ResMut<Focus>,
    mut notice: ResMut<controls::Notice>,
    disc: Option<Res<rac_rig::RigAnimations>>,
) {
    for root in &roots {
        commands.entity(root).despawn();
    }
    let Some(&(page, page_focus)) = pages.0.last() else {
        return;
    };
    *focus = page_focus;
    match page {
        Page::Main => spawn_main_menu(&mut commands, disc.is_some()),
        Page::Pause => spawn_pause_menu(&mut commands),
        Page::Options => spawn_options(&mut commands),
        Page::Controls => controls::spawn(&mut commands, &mut notice),
    }
}

fn spawn_main_menu(commands: &mut Commands, disc: bool) {
    let found = if disc {
        "Your UYA disc gives the hero Ratchet's animations, and his model under Model."
    } else {
        "No supported disc found. The hero kit's demo needs none."
    };
    commands.spawn((
        PageRoot,
        menu_root(Color::srgb(0.05, 0.06, 0.08)),
        children![
            title("Open Ratchet & Clank Engine"),
            subtitle(found),
            button(0, "Play the demo", MenuCommand::PlayDemo),
            named_choice(1, Choice::Model),
            named_choice(2, Choice::Moves),
            named_choice(3, Choice::Camera),
            button(4, "Options", MenuCommand::Open(Page::Options)),
            button(5, "Quit", MenuCommand::Quit),
        ],
    ));
}

fn spawn_pause_menu(commands: &mut Commands) {
    commands.spawn((
        PageRoot,
        menu_root(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        children![
            title("Paused"),
            button(0, "Resume", MenuCommand::Resume),
            named_choice(1, Choice::Model),
            named_choice(2, Choice::Moves),
            named_choice(3, Choice::Camera),
            button(4, "Options", MenuCommand::Open(Page::Options)),
            button(5, "Main menu", MenuCommand::MainMenu),
            button(6, "Quit", MenuCommand::Quit),
        ],
    ));
}

/// The options, under headings, in the order the page lists them.
enum OptionLine {
    Heading(&'static str),
    Choice(Choice),
    Controls,
}

const OPTIONS: [OptionLine; 19] = [
    OptionLine::Heading("Controls"),
    OptionLine::Controls,
    OptionLine::Choice(Choice::MouseSpeed),
    OptionLine::Choice(Choice::InvertLookX),
    OptionLine::Choice(Choice::InvertLookY),
    OptionLine::Choice(Choice::SwapSticks),
    OptionLine::Choice(Choice::MoveDeadZone),
    OptionLine::Choice(Choice::LookDeadZone),
    OptionLine::Heading("Display"),
    OptionLine::Choice(Choice::Window),
    OptionLine::Choice(Choice::Resolution),
    OptionLine::Choice(Choice::Vsync),
    OptionLine::Choice(Choice::FrameCap),
    OptionLine::Choice(Choice::FieldOfView),
    OptionLine::Choice(Choice::InterfaceSize),
    OptionLine::Choice(Choice::Backend),
    OptionLine::Heading("Sound"),
    OptionLine::Choice(Choice::Volume),
    OptionLine::Heading(""),
];

fn spawn_options(commands: &mut Commands) {
    commands
        .spawn((
            PageRoot,
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(px(20)),
                row_gap: px(10),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.035, 0.05, 0.92)),
        ))
        .with_children(|page| {
            page.spawn(title("Options"));
            let mut row = 0;
            page.spawn((
                MenuScroll,
                ScrollPosition::default(),
                Node {
                    width: percent(100),
                    max_width: px(800),
                    // Whatever room the title and footer leave, however long the list.
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_height: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
            ))
            .with_children(|list| {
                for line in OPTIONS {
                    match line {
                        OptionLine::Heading(text) => {
                            list.spawn(heading(text));
                        }
                        OptionLine::Controls => {
                            list.spawn((
                                Scrolled,
                                item(
                                    row,
                                    0,
                                    MenuCommand::Open(Page::Controls),
                                    option_node(),
                                    (),
                                    "Bindings...",
                                ),
                            ));
                            row += 1;
                        }
                        OptionLine::Choice(choice) => {
                            list.spawn((Scrolled, option_row(row, choice)));
                            row += 1;
                        }
                    }
                }
            });
            page.spawn(item(
                row,
                0,
                MenuCommand::Back,
                Node {
                    width: px(260),
                    height: px(48),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                (),
                "Back",
            ));
        });
}

fn heading(text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(TEXT.with_alpha(0.6)),
        Node {
            margin: UiRect::top(px(10)),
            flex_shrink: 0.0,
            ..default()
        },
    )
}

fn option_node() -> Node {
    Node {
        width: percent(100),
        height: px(42),
        flex_shrink: 0.0,
        padding: UiRect::horizontal(px(18)),
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        border_radius: BorderRadius::all(px(6)),
        ..default()
    }
}

/// A choice on the options page: its name on the left, its value on the right.
fn option_row(row: usize, choice: Choice) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        MenuItem {
            row,
            column: 0,
            command: MenuCommand::Step(choice, true),
        },
        option_node(),
        BackgroundColor(BUTTON),
        on_click(MenuCommand::Step(choice, true)),
        children![
            label_text(choice.name(), 22.0, ()),
            (
                // Arrows at fixed places, whatever the value's length.
                Node {
                    width: px(360),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
                children![
                    label_text("<", 22.0, ()),
                    label_text(
                        "",
                        22.0,
                        ChoiceText {
                            choice,
                            named: false,
                        }
                    ),
                    label_text(">", 22.0, ()),
                ],
            ),
        ],
    )
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
        Node {
            flex_shrink: 0.0,
            ..default()
        },
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

fn label_text(text: &str, size: f32, marker: impl Bundle) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(TEXT),
        marker,
    )
}

fn menu_button_node() -> Node {
    Node {
        width: px(520),
        height: px(56),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border_radius: BorderRadius::all(px(6)),
        ..default()
    }
}

/// A main or pause menu button.
fn button(row: usize, label: &str, command: MenuCommand) -> impl Bundle {
    item(row, 0, command, menu_button_node(), (), label)
}

/// A main or pause menu button stepping through a choice, labelled by [`label_choices`].
fn named_choice(row: usize, choice: Choice) -> impl Bundle {
    item(
        row,
        0,
        MenuCommand::Step(choice, true),
        menu_button_node(),
        ChoiceText {
            choice,
            named: true,
        },
        "",
    )
}

/// A focusable item with one line of text, `marker` on the text.
fn item(
    row: usize,
    column: usize,
    command: MenuCommand,
    node: Node,
    marker: impl Bundle,
    label: &str,
) -> impl Bundle {
    let size = if node.height == px(56) { 24.0 } else { 20.0 };
    (
        Button,
        Hovered::default(),
        MenuItem {
            row,
            column,
            command,
        },
        node,
        BackgroundColor(BUTTON),
        on_click(command),
        children![label_text(label, size, marker)],
    )
}

/// Chooses `command` on a left click, and its reverse on a right click.
fn on_click(command: MenuCommand) -> impl Bundle {
    observe(
        move |click: On<PointerClick>,
              mut chosen: ResMut<Chosen>,
              swallow: Res<SwallowClick>,
              rebinding: Res<controls::Rebinding>| {
            if swallow.0 || rebinding.is_active() {
                return;
            }
            chosen.0 = match click.button {
                PointerButton::Primary => Some(command),
                PointerButton::Secondary => command.reverse(),
                PointerButton::Middle => None,
            };
        },
    )
}

fn unswallow_click(mouse: Res<ButtonInput<MouseButton>>, mut swallow: ResMut<SwallowClick>) {
    if swallow.0 && mouse.get_pressed().next().is_none() {
        swallow.0 = false;
    }
}

/// Moves the focus to the nearest item when none is at it, as when a page opens.
fn fix_focus(items: Query<&MenuItem>, mut focus: ResMut<Focus>) {
    let cells: Vec<_> = items.iter().map(MenuItem::at).collect();
    if let Some(nearest) = nearest(&cells, *focus) {
        *focus = nearest;
    }
}

fn nearest(cells: &[Focus], to: Focus) -> Option<Focus> {
    if cells.contains(&to) {
        return Some(to);
    }
    cells.iter().copied().min_by_key(|cell| {
        (
            cell.row.abs_diff(to.row),
            cell.column.abs_diff(to.column),
            *cell,
        )
    })
}

/// The item a row up or down from `from`, wrapping round, at the column nearest its own.
fn moved_row(cells: &[Focus], from: Focus, down: bool) -> Focus {
    let rows: BTreeSet<_> = cells.iter().map(|cell| cell.row).collect();
    let row = if down {
        rows.range(from.row + 1..).next().or(rows.first())
    } else {
        rows.range(..from.row).next_back().or(rows.last())
    };
    let Some(&row) = row else {
        return from;
    };
    cells
        .iter()
        .copied()
        .filter(|cell| cell.row == row)
        .min_by_key(|cell| (cell.column.abs_diff(from.column), cell.column))
        .unwrap_or(from)
}

/// The item beside `from` in its row, if there is one that way.
fn moved_column(cells: &[Focus], from: Focus, right: bool) -> Focus {
    let beside = cells.iter().copied().filter(|cell| cell.row == from.row);
    let next = if right {
        beside.filter(|cell| cell.column > from.column).min()
    } else {
        beside.filter(|cell| cell.column < from.column).max()
    };
    next.unwrap_or(from)
}

/// The held menu direction, and how long until it repeats.
#[derive(Debug, Default)]
struct Repeat {
    held: Option<Action>,
    wait: f32,
}

impl Repeat {
    /// The direction that moves this frame, one just pressed or one held long enough, and
    /// whether it is a repeat.
    fn fire(
        &mut self,
        just_pressed: impl Fn(Action) -> bool,
        held: impl Fn(Action) -> bool,
        seconds: f32,
    ) -> Option<(Action, bool)> {
        const DIRECTIONS: [Action; 4] = [
            Action::UiUp,
            Action::UiDown,
            Action::UiLeft,
            Action::UiRight,
        ];
        if let Some(pressed) = DIRECTIONS.into_iter().find(|a| just_pressed(*a)) {
            self.held = Some(pressed);
            self.wait = REPEAT_DELAY;
            return Some((pressed, false));
        }
        let held = self.held.filter(|action| held(*action))?;
        self.wait -= seconds;
        if self.wait > 0.0 {
            return None;
        }
        self.wait += REPEAT_EVERY;
        Some((held, true))
    }
}

/// The column up and down aim for: the one the player last chose, so passing a row
/// without it (lock-on has no keys) doesn't move them across.
#[derive(Debug, Default)]
struct Aim {
    /// Where the focus was left by moving up or down, and the column aimed for then.
    left_at: Option<(Focus, usize)>,
}

impl Aim {
    fn column(&self, focus: Focus) -> usize {
        match self.left_at {
            Some((at, column)) if at == focus => column,
            _ => focus.column,
        }
    }
}

fn navigate(
    actions: Res<ActionState>,
    time: Res<Time<Real>>,
    mut repeat: Local<Repeat>,
    mut aim: Local<Aim>,
    items: Query<&MenuItem>,
    mut focus: ResMut<Focus>,
    mut chosen: ResMut<Chosen>,
) {
    let fired = repeat.fire(
        |action| actions.just_pressed(action),
        |action| actions.held(action),
        time.delta_secs(),
    );
    let Some((direction, repeated)) = fired else {
        return;
    };
    let cells: Vec<_> = items.iter().map(MenuItem::at).collect();
    let focused = items.iter().find(|item| item.at() == *focus);
    match direction {
        Action::UiUp | Action::UiDown => {
            let column = aim.column(*focus);
            let from = Focus {
                row: focus.row,
                column,
            };
            *focus = moved_row(&cells, from, direction == Action::UiDown);
            aim.left_at = Some((*focus, column));
        }
        _ => {
            let right = direction == Action::UiRight;
            match focused.map(|item| item.command) {
                // Only numbers run on while held: a window mode or a model flicking
                // past would be no use.
                Some(MenuCommand::Step(choice, _)) if repeated && !choice.repeats() => {}
                Some(MenuCommand::Step(choice, _)) => {
                    chosen.0 = Some(MenuCommand::Step(choice, right));
                }
                _ => *focus = moved_column(&cells, *focus, right),
            }
        }
    }
}

fn focus_on_hover(
    items: Query<(&MenuItem, &Hovered), Changed<Hovered>>,
    rebinding: Res<controls::Rebinding>,
    mut focus: ResMut<Focus>,
) {
    if rebinding.is_active() {
        return;
    }
    for (item, hovered) in &items {
        if hovered.get() {
            *focus = item.at();
        }
    }
}

fn accept(
    actions: Res<ActionState>,
    items: Query<&MenuItem>,
    focus: Res<Focus>,
    pages: Res<Pages>,
    mut chosen: ResMut<Chosen>,
) {
    let focused = items.iter().find(|item| item.at() == *focus);
    if actions.just_pressed(Action::UiAccept) {
        if let Some(item) = focused {
            chosen.0 = Some(item.command);
        }
    } else if actions.just_pressed(Action::UiClear) {
        if let Some(MenuCommand::Bind(slot)) = focused.map(|item| item.command) {
            chosen.0 = Some(MenuCommand::Clear(slot));
        }
    } else if actions.just_pressed(Action::UiBack) {
        chosen.0 = Some(MenuCommand::Back);
    } else if actions.just_pressed(Action::Pause)
        && pages
            .0
            .first()
            .is_some_and(|(page, _)| *page == Page::Pause)
    {
        chosen.0 = Some(MenuCommand::Resume);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_chosen(
    mut chosen: ResMut<Chosen>,
    mut pages: ResMut<Pages>,
    focus: Res<Focus>,
    mut app_state: ResMut<NextState<AppState>>,
    mut pause: ResMut<NextState<Pause>>,
    mut choices: Choices,
    mut capture: ResMut<BindingCapture>,
    mut rebinding: ResMut<controls::Rebinding>,
    mut notice: ResMut<controls::Notice>,
    time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(command) = chosen.0.take() else {
        return;
    };
    match command {
        MenuCommand::PlayDemo => app_state.set(AppState::InGame),
        MenuCommand::Resume => pause.set(Pause::Running),
        MenuCommand::MainMenu => app_state.set(AppState::MainMenu),
        MenuCommand::Quit => {
            exit.write(AppExit::Success);
        }
        MenuCommand::Open(page) => {
            if let Some(top) = pages.0.last_mut() {
                top.1 = *focus;
            }
            pages.0.push((page, Focus::default()));
        }
        MenuCommand::Back => {
            if pages.0.len() > 1 {
                pages.0.pop();
            } else if pages
                .0
                .first()
                .is_some_and(|(page, _)| *page == Page::Pause)
            {
                pause.set(Pause::Running);
            }
        }
        MenuCommand::Step(choice, forward) => choices.step(choice, forward),
        MenuCommand::Bind(slot) => {
            controls::listen(slot, &mut capture, &mut rebinding, time.elapsed_secs());
        }
        MenuCommand::Clear(slot) => controls::clear(slot, &mut choices, &mut notice),
        MenuCommand::ResetControls => controls::reset(&mut choices, &mut notice),
    }
}

fn show_focus(
    focus: Res<Focus>,
    rebinding: Res<controls::Rebinding>,
    mut items: Query<(&MenuItem, &mut BackgroundColor)>,
) {
    for (item, mut background) in &mut items {
        let color = controls::listening_colour(&rebinding, item.command).unwrap_or(
            if item.at() == *focus {
                BUTTON_FOCUSED
            } else {
                BUTTON
            },
        );
        if background.0 != color {
            background.0 = color;
        }
    }
}

fn label_choices(choices: Choices, mut texts: Query<(&ChoiceText, &mut Text)>) {
    for (label, mut text) in &mut texts {
        let value = choices.value(label.choice);
        let wanted = if label.named {
            format!("{}: {value}", label.choice.name())
        } else {
            value
        };
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
}

/// Scrolls a page's list with the wheel, and to the focused item when the focus moves.
fn scroll(
    focus: Res<Focus>,
    wheel: Res<AccumulatedMouseScroll>,
    rebinding: Res<controls::Rebinding>,
    // The focus followed, and at what scale: a new size for the menus moves it.
    mut followed: Local<Option<(Focus, f32)>>,
    items: Query<(&MenuItem, &ComputedNode, &UiGlobalTransform), With<Scrolled>>,
    mut lists: Query<(&mut ScrollPosition, &ComputedNode, &UiGlobalTransform), With<MenuScroll>>,
) {
    for (mut position, list, place) in &mut lists {
        let scale = list.inverse_scale_factor();
        let mut y = position.y;
        if !rebinding.is_active() && wheel.delta.y != 0.0 {
            let lines = match wheel.unit {
                MouseScrollUnit::Line => WHEEL_LINE,
                MouseScrollUnit::Pixel => 1.0,
            };
            y -= wheel.delta.y * lines;
        }
        if *followed != Some((*focus, scale))
            && let Some((_, node, item)) = items.iter().find(|(item, ..)| item.at() == *focus)
            && node.size().y > 0.0
        {
            // Physical pixels, from the top of the window.
            let top = place.translation.y - list.size().y / 2.0;
            let bottom = top + list.size().y;
            let item_top = item.translation.y - node.size().y / 2.0;
            let item_bottom = item_top + node.size().y;
            if item_top < top {
                y -= (top - item_top) * scale;
            } else if item_bottom > bottom {
                y += (item_bottom - bottom) * scale;
            }
            *followed = Some((*focus, scale));
        }
        let most = ((list.content_size().y - list.size().y) * scale).max(0.0);
        let y = y.clamp(0.0, most);
        if position.y != y {
            position.y = y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(row: usize, column: usize) -> Focus {
        Focus { row, column }
    }

    /// Two rows of four slots with a gap, then a footer of two.
    fn grid() -> Vec<Focus> {
        vec![
            at(0, 1),
            at(0, 2),
            at(0, 3),
            at(0, 4),
            at(1, 2),
            at(1, 3),
            at(2, 1),
            at(2, 3),
        ]
    }

    #[test]
    fn up_and_down_keep_to_the_nearest_column_and_wrap() {
        let cells = grid();
        assert_eq!(moved_row(&cells, at(0, 4), true), at(1, 3));
        assert_eq!(moved_row(&cells, at(1, 2), true), at(2, 1));
        assert_eq!(moved_row(&cells, at(2, 1), true), at(0, 1));
        assert_eq!(moved_row(&cells, at(0, 1), false), at(2, 1));
        assert_eq!(moved_row(&cells, at(0, 1), true), at(1, 2));
    }

    #[test]
    fn left_and_right_stop_at_the_ends_of_a_row() {
        let cells = grid();
        assert_eq!(moved_column(&cells, at(0, 1), true), at(0, 2));
        assert_eq!(moved_column(&cells, at(0, 4), true), at(0, 4));
        assert_eq!(moved_column(&cells, at(2, 3), false), at(2, 1));
        assert_eq!(moved_column(&cells, at(0, 1), false), at(0, 1));
    }

    #[test]
    fn up_and_down_keep_aiming_for_the_column_chosen() {
        let mut aim = Aim::default();
        // Row 1 has no column 1, so passing it lands on 2, but row 2 is back on 1.
        let cells = grid();
        let mut focus = at(0, 1);
        for expected in [at(1, 2), at(2, 1)] {
            let column = aim.column(focus);
            focus = moved_row(&cells, Focus { column, ..focus }, true);
            aim.left_at = Some((focus, column));
            assert_eq!(focus, expected);
        }
        // Moved some other way, the focus's own column counts.
        assert_eq!(aim.column(at(1, 3)), 3);
    }

    #[test]
    fn a_lost_focus_goes_to_the_nearest_item() {
        let cells = grid();
        assert_eq!(nearest(&cells, at(0, 0)), Some(at(0, 1)));
        assert_eq!(nearest(&cells, at(1, 4)), Some(at(1, 3)));
        assert_eq!(nearest(&[], at(0, 0)), None);
    }

    #[test]
    fn a_held_direction_repeats_after_a_pause() {
        let mut repeat = Repeat::default();
        let down = |action| action == Action::UiDown;
        let none = |_| false;
        assert_eq!(
            repeat.fire(down, down, 0.016),
            Some((Action::UiDown, false))
        );
        assert_eq!(repeat.fire(none, down, REPEAT_DELAY / 2.0), None);
        assert_eq!(
            repeat.fire(none, down, REPEAT_DELAY / 2.0),
            Some((Action::UiDown, true))
        );
        assert_eq!(
            repeat.fire(none, down, REPEAT_EVERY),
            Some((Action::UiDown, true))
        );
        assert_eq!(repeat.fire(none, none, 1.0), None);
    }
}

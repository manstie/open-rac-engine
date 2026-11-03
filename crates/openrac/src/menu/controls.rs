//! The controls page: each control's bindings, two on the keyboard and mouse and two on
//! the gamepad. Choosing a slot listens for what to put in it (`rac_input::BindingCapture`),
//! for five seconds; a binding another control had swaps or moves as
//! `InputSettings::assign` settles it, and the page says what changed.

use bevy::prelude::*;
use rac_input::{
    Action, BindingCapture, Caught, Change, Control, Device, InputSettings, MoveKey, RebindError,
    SLOTS, Slot,
};

use super::{
    BUTTON, MenuCommand, MenuScroll, PageRoot, Scrolled, TEXT, choices::Choices, item, names, title,
};

/// The controls in the order the page lists them: moving, then play, then the menus.
const CONTROLS: [Control; 23] = [
    Control::Move(MoveKey::Forward),
    Control::Move(MoveKey::Back),
    Control::Move(MoveKey::Left),
    Control::Move(MoveKey::Right),
    Control::Action(Action::Jump),
    Control::Action(Action::Crouch),
    Control::Action(Action::Melee),
    Control::Action(Action::Fire),
    Control::Action(Action::QuickSelect),
    Control::Action(Action::Strafe),
    Control::Action(Action::LockOn),
    Control::Action(Action::FirstPerson),
    Control::Action(Action::NextWeapon),
    Control::Action(Action::PrevWeapon),
    Control::Action(Action::Map),
    Control::Action(Action::Pause),
    Control::Action(Action::UiUp),
    Control::Action(Action::UiDown),
    Control::Action(Action::UiLeft),
    Control::Action(Action::UiRight),
    Control::Action(Action::UiAccept),
    Control::Action(Action::UiBack),
    Control::Action(Action::UiClear),
];

/// How long a slot listens for a press before giving up, in seconds.
const LISTEN_SECONDS: f32 = 5.0;

const ROW_HEIGHT: f32 = 34.0;
const SLOT_LISTENING: Color = Color::srgb(0.75, 0.2, 0.15);
const DIM: Color = Color::srgba(0.92, 0.92, 0.92, 0.35);

/// The slot a press is being caught for, and when the page started listening.
#[derive(Resource, Debug, Default)]
pub(super) struct Rebinding {
    pub slot: Option<Slot>,
    since: f32,
    /// Whether the listening ended this frame, so the press that ended it does nothing
    /// else, such as scrolling the page.
    pub ended: bool,
}

impl Rebinding {
    /// Whether a press may still be on its way to the slot. A click comes up in the frame
    /// after the press was caught, or in the same frame when press and release are that
    /// quick, before [`catch`] has seen it; it must not choose another slot.
    pub fn is_active(&self) -> bool {
        self.slot.is_some() || self.ended
    }
}

/// What the page last said about a change, under the list.
#[derive(Resource, Debug, Default)]
pub(super) struct Notice(String);

/// The text of a slot's button.
#[derive(Component, Debug)]
pub(super) struct SlotText(Slot);

/// The line under the list.
#[derive(Component, Debug)]
pub(super) struct NoticeText;

/// The grid column of a slot: the keyboard and mouse's in 1 and 2, the gamepad's in 3
/// and 4.
fn column(device: Device, index: usize) -> usize {
    1 + index
        + match device {
            Device::KeyboardMouse => 0,
            Device::Gamepad => SLOTS,
        }
}

pub(super) fn spawn(commands: &mut Commands, notice: &mut Notice) {
    notice.0.clear();
    let footer_row = CONTROLS.len();
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
            page.spawn(title("Controls"));
            page.spawn(grid_row()).with_children(|header| {
                header.spawn(name_cell(""));
                header.spawn(heading_cell("Keyboard and mouse"));
                header.spawn(heading_cell("Gamepad"));
            });
            page.spawn((
                MenuScroll,
                ScrollPosition::default(),
                Node {
                    width: percent(100),
                    max_width: px(1040),
                    // Whatever room the title and footer leave, however long the list.
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_height: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
            ))
            .with_children(|list| {
                for (row, control) in CONTROLS.into_iter().enumerate() {
                    list.spawn(grid_row()).with_children(|cells| {
                        cells.spawn(name_cell(names::control(control)));
                        for device in [Device::KeyboardMouse, Device::Gamepad] {
                            for index in 0..SLOTS {
                                let slot = Slot {
                                    control,
                                    device,
                                    index,
                                };
                                if index < control.slots(device) {
                                    cells.spawn((
                                        Scrolled,
                                        item(
                                            row,
                                            column(device, index),
                                            MenuCommand::Bind(slot),
                                            slot_node(),
                                            SlotText(slot),
                                            "",
                                        ),
                                    ));
                                } else {
                                    cells.spawn(no_slot(control, device, index));
                                }
                            }
                        }
                    });
                }
            });
            page.spawn((
                NoticeText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(TEXT.with_alpha(0.85)),
                Node {
                    min_height: px(48),
                    max_width: px(1040),
                    ..default()
                },
            ));
            page.spawn(Node {
                column_gap: px(16),
                ..default()
            })
            .with_children(|footer| {
                footer.spawn(item(
                    footer_row,
                    1,
                    MenuCommand::ResetControls,
                    footer_node(),
                    (),
                    "Reset to defaults",
                ));
                footer.spawn(item(
                    footer_row,
                    column(Device::Gamepad, 0),
                    MenuCommand::Back,
                    footer_node(),
                    (),
                    "Back",
                ));
            });
        });
}

fn grid_row() -> Node {
    Node {
        width: percent(100),
        max_width: px(1040),
        height: px(ROW_HEIGHT),
        flex_shrink: 0.0,
        column_gap: px(6),
        align_items: AlignItems::Center,
        ..default()
    }
}

fn name_cell(text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(19.0),
            ..default()
        },
        TextColor(TEXT),
        Node {
            width: percent(24),
            flex_shrink: 0.0,
            ..default()
        },
    )
}

/// A heading over one device's slots.
fn heading_cell(text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(19.0),
            ..default()
        },
        TextColor(TEXT.with_alpha(0.7)),
        TextLayout::justify(Justify::Center),
        Node {
            flex_grow: 2.0,
            flex_basis: px(0),
            ..default()
        },
    )
}

fn slot_node() -> Node {
    Node {
        flex_grow: 1.0,
        flex_basis: px(0),
        height: percent(100),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border_radius: BorderRadius::all(px(4)),
        ..default()
    }
}

fn footer_node() -> Node {
    Node {
        width: px(260),
        height: px(48),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border_radius: BorderRadius::all(px(6)),
        ..default()
    }
}

/// A cell where a control takes no binding: the movement keys' gamepad side says the
/// stick moves, lock-on's keyboard side that it is gamepad only.
fn no_slot(control: Control, device: Device, index: usize) -> impl Bundle {
    let text = match (control, device, index) {
        (Control::Move(_), Device::Gamepad, 0) => "Stick",
        (Control::Action(_), Device::KeyboardMouse, 0) => "Gamepad only",
        _ => "",
    };
    (
        slot_node(),
        BackgroundColor(BUTTON.with_alpha(0.35)),
        children![(
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(17.0),
                ..default()
            },
            TextColor(DIM),
        )],
    )
}

/// Starts listening for what to put in `slot`.
pub(super) fn listen(
    slot: Slot,
    capture: &mut BindingCapture,
    rebinding: &mut Rebinding,
    now: f32,
) {
    capture.listen(slot.device);
    rebinding.slot = Some(slot);
    rebinding.since = now;
}

pub(super) fn clear(slot: Slot, choices: &mut Choices, notice: &mut Notice) {
    let before = choices.input().binding_in(slot);
    match choices.input_mut().clear(slot) {
        Ok(()) => {
            if let Some(binding) = before {
                notice.0 = format!(
                    "{} no longer has {}.",
                    names::control(slot.control),
                    names::binding(binding, choices.keys())
                );
                choices.save_input();
            }
        }
        Err(error) => notice.0 = refusal(slot, error),
    }
}

pub(super) fn reset(choices: &mut Choices, notice: &mut Notice) {
    choices.input_mut().reset_bindings();
    choices.save_input();
    notice.0 = "Every binding is back to its default.".into();
}

/// Puts what was caught in the slot being listened for, gives up after
/// [`LISTEN_SECONDS`], and makes the mouse button that was caught, or that called it off,
/// click nothing as it comes up.
pub(super) fn catch(
    time: Res<Time<Real>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut capture: ResMut<BindingCapture>,
    mut rebinding: ResMut<Rebinding>,
    mut notice: ResMut<Notice>,
    mut swallow: ResMut<super::SwallowClick>,
    mut choices: Choices,
) {
    rebinding.ended = false;
    let Some(slot) = rebinding.slot else {
        return;
    };
    let caught = capture.take();
    if caught.is_none() && capture.is_listening() {
        if time.elapsed_secs() - rebinding.since > LISTEN_SECONDS {
            capture.stop();
            rebinding.slot = None;
            rebinding.ended = true;
            notice.0 = "Nothing was pressed, so it is as it was.".into();
        }
        return;
    }
    rebinding.slot = None;
    rebinding.ended = true;
    if mouse.get_pressed().next().is_some() {
        swallow.0 = true;
    }
    match caught {
        Some(Caught::Binding(binding)) => match choices.input_mut().assign(slot, binding) {
            Ok(changes) => {
                notice.0 = assigned(slot, binding, &changes, choices.keys());
                choices.save_input();
            }
            Err(error) => notice.0 = refusal(slot, error),
        },
        Some(Caught::Cancelled) | None => notice.0 = "Left as it was.".into(),
    }
}

/// What the page says after putting `binding` in `slot`.
fn assigned(
    slot: Slot,
    binding: rac_input::Binding,
    changes: &[Change],
    keys: &names::KeyNames,
) -> String {
    let mut text = format!(
        "{}: {}.",
        names::control(slot.control),
        names::binding(binding, keys)
    );
    for change in changes {
        text += &match *change {
            Change::Swapped { control, took } => format!(
                " {} takes {} instead.",
                names::control(control),
                names::binding(took, keys)
            ),
            Change::Removed { control } => {
                format!(" {} no longer has it.", names::control(control))
            }
        };
    }
    text
}

fn refusal(slot: Slot, error: RebindError) -> String {
    match error {
        RebindError::LastBinding(control @ Control::Move(_)) => format!(
            "{} must keep its key; give it another first.",
            names::control(control)
        ),
        RebindError::LastBinding(control) => format!(
            "{} must keep a binding on the {}, to reach the menus; give it another first.",
            names::control(control),
            device_name(slot.device)
        ),
        RebindError::Unsuitable => "Movement takes keys only.".into(),
        RebindError::NoSlot => "That can't be bound there.".into(),
    }
}

fn device_name(device: Device) -> &'static str {
    match device {
        Device::KeyboardMouse => "keyboard and mouse",
        Device::Gamepad => "gamepad",
    }
}

pub(super) fn label_slots(
    input: Res<InputSettings>,
    keys: Res<names::KeyNames>,
    rebinding: Res<Rebinding>,
    mut texts: Query<(&SlotText, &mut Text)>,
) {
    for (slot, mut text) in &mut texts {
        let wanted = if rebinding.slot == Some(slot.0) {
            "Press...".to_owned()
        } else {
            input.binding_in(slot.0).map_or_else(
                || "-".to_owned(),
                |b| names::capitalised(&names::binding(b, &keys)),
            )
        };
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
}

pub(super) fn show_notice(
    time: Res<Time<Real>>,
    input: Res<InputSettings>,
    keys: Res<names::KeyNames>,
    rebinding: Res<Rebinding>,
    notice: Res<Notice>,
    mut texts: Query<&mut Text, With<NoticeText>>,
) {
    let wanted = match rebinding.slot {
        Some(slot) => {
            let left = (LISTEN_SECONDS - (time.elapsed_secs() - rebinding.since))
                .ceil()
                .max(0.0);
            let what = match slot.device {
                Device::KeyboardMouse => "a key, a mouse button or the wheel",
                Device::Gamepad => "a button, or two together",
            };
            format!(
                "Press {what} for {}, or Esc to leave it ({left:.0} s).",
                names::control(slot.control)
            )
        }
        None if !notice.0.is_empty() => notice.0.clone(),
        None => format!(
            "{}: change a binding. {} or right-click: clear one.",
            names::bindings_of(&input, &keys, Control::Action(Action::UiAccept)),
            names::bindings_of(&input, &keys, Control::Action(Action::UiClear)),
        ),
    };
    for mut text in &mut texts {
        if text.0 != wanted {
            text.0 = wanted.clone();
        }
    }
}

/// The colour of a slot's button while it listens.
pub(super) fn listening_colour(rebinding: &Rebinding, command: MenuCommand) -> Option<Color> {
    match command {
        MenuCommand::Bind(slot) if rebinding.slot == Some(slot) => Some(SLOT_LISTENING),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_lists_every_action_once() {
        for action in Action::ALL {
            let count = CONTROLS
                .iter()
                .filter(|control| **control == Control::Action(action))
                .count();
            assert_eq!(count, 1, "{action:?}");
        }
    }

    #[test]
    fn the_defaults_fit_the_slots() {
        let settings = InputSettings::default();
        for control in CONTROLS {
            for device in [Device::KeyboardMouse, Device::Gamepad] {
                let shown = (0..SLOTS)
                    .filter(|&index| {
                        settings
                            .binding_in(Slot {
                                control,
                                device,
                                index,
                            })
                            .is_some()
                    })
                    .count();
                let Control::Action(action) = control else {
                    continue;
                };
                let all = settings
                    .bindings_for(action)
                    .iter()
                    .filter(|binding| binding.device() == device)
                    .count();
                assert_eq!(shown, all, "{action:?} has bindings the page can't show");
            }
        }
    }

    #[test]
    fn a_change_says_what_it_did_to_the_others() {
        let slot = Slot {
            control: Control::Action(Action::Jump),
            device: Device::KeyboardMouse,
            index: 0,
        };
        let text = assigned(
            slot,
            rac_input::Binding::Key(KeyCode::KeyF),
            &[Change::Swapped {
                control: Control::Action(Action::Melee),
                took: rac_input::Binding::Key(KeyCode::Space),
            }],
            &names::KeyNames::default(),
        );
        assert_eq!(text, "Jump: F. Wrench takes Space instead.");
    }
}

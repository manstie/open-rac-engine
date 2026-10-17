//! The hero camera's setting, by the names settings and command lines use: `third-person`,
//! `lock-strafe` or `first-person`, or none for the movement profile's own (third person
//! for UYA, lock-strafe for Deadlocked). The menus' Camera button steps through them.

use rac_hero::{CameraSetting, HeroProfile};

/// The choices in the order the Camera button steps through them; `None` is the game's.
const CHOICES: [Option<CameraSetting>; 4] = [
    None,
    Some(CameraSetting::ThirdPerson),
    Some(CameraSetting::LockStrafe),
    Some(CameraSetting::FirstPerson),
];

pub fn name(setting: CameraSetting) -> &'static str {
    match setting {
        CameraSetting::ThirdPerson => "third-person",
        CameraSetting::LockStrafe => "lock-strafe",
        CameraSetting::FirstPerson => "first-person",
    }
}

pub fn parse(name: &str) -> Option<CameraSetting> {
    CameraSetting::ALL
        .into_iter()
        .find(|setting| self::name(*setting) == name)
}

fn title(setting: CameraSetting) -> &'static str {
    match setting {
        CameraSetting::ThirdPerson => "Third person",
        CameraSetting::LockStrafe => "Lock-strafe",
        CameraSetting::FirstPerson => "First person",
    }
}

/// The Camera button's label for `choice`, with `profile` giving the game's own.
pub fn label(choice: Option<CameraSetting>, profile: &HeroProfile) -> String {
    match choice {
        Some(setting) => format!("Camera: {}", title(setting)),
        None => format!(
            "Camera: {} (the game's)",
            title(profile.camera.default_setting)
        ),
    }
}

/// The choice after `current`, or before it, wrapping round.
pub fn step(current: Option<CameraSetting>, forward: bool) -> Option<CameraSetting> {
    let count = CHOICES.len();
    let index = CHOICES
        .iter()
        .position(|choice| *choice == current)
        .unwrap_or(0);
    let next = if forward {
        (index + 1) % count
    } else {
        (index + count - 1) % count
    };
    CHOICES[next]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for setting in CameraSetting::ALL {
            assert_eq!(parse(name(setting)), Some(setting));
        }
        assert_eq!(parse("isometric"), None);
    }

    #[test]
    fn the_button_steps_through_the_games_and_the_three_settings() {
        let mut choice = None;
        let mut seen = Vec::new();
        for _ in 0..4 {
            choice = step(choice, true);
            seen.push(choice);
        }
        assert_eq!(
            seen,
            CHOICES[1..]
                .iter()
                .copied()
                .chain([None])
                .collect::<Vec<_>>()
        );
        assert_eq!(step(None, false), Some(CameraSetting::FirstPerson));
        assert_eq!(
            label(None, &HeroProfile::dl()),
            "Camera: Lock-strafe (the game's)"
        );
    }
}

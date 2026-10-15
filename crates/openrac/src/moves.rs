//! Which game the hero moves like: one of the movement profiles that ship with the hero
//! kit, by its short name (`rac_hero::profile::BUNDLED`: `uya`, `dl`), or a RON file of the
//! same shape. The menus' Moves button steps through the bundled ones.

use anyhow::{Context, Result};
use bevy::prelude::Resource;
use rac_hero::{HeroProfile, profile::BUNDLED};

/// The profile used when nothing is chosen: UYA NTSC-U's, the kit's default.
pub const DEFAULT: &str = "uya";

/// The name the hero's movement profile was chosen by.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct ChosenMoves(pub String);

impl Default for ChosenMoves {
    fn default() -> Self {
        Self(DEFAULT.to_owned())
    }
}

/// A bundled profile by its short name, or else the profile in the RON file at `choice`.
pub fn load(choice: &str) -> Result<HeroProfile> {
    if let Some(profile) = HeroProfile::bundled(choice) {
        return Ok(profile);
    }
    let text = std::fs::read_to_string(choice).with_context(|| {
        let names: Vec<_> = BUNDLED.iter().map(|(name, _)| *name).collect();
        format!(
            "{choice:?} is neither a bundled movement profile ({}) nor a readable file",
            names.join(", ")
        )
    })?;
    HeroProfile::from_ron(&text).with_context(|| format!("reading the profile {choice}"))
}

/// The bundled profile after `current`, or before it, wrapping round. From a profile
/// loaded from a file, the first bundled one.
pub fn step(current: &str, forward: bool) -> &'static str {
    let count = BUNDLED.len();
    let next = match BUNDLED.iter().position(|(name, _)| *name == current) {
        Some(index) if forward => (index + 1) % count,
        Some(index) => (index + count - 1) % count,
        None => 0,
    };
    BUNDLED[next].0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_names_and_files_load() {
        assert_eq!(load("dl").unwrap(), HeroProfile::dl());
        let path = std::env::temp_dir().join(format!("openrac-moves-{}.ron", std::process::id()));
        std::fs::write(&path, rac_hero::profile::DL_RON).unwrap();
        assert_eq!(load(path.to_str().unwrap()).unwrap(), HeroProfile::dl());
        std::fs::remove_file(&path).unwrap();
        assert!(load("no-such-game").is_err());
    }

    #[test]
    fn the_moves_button_steps_round_the_bundled_profiles() {
        assert_eq!(step("uya", true), "dl");
        assert_eq!(step("dl", true), "uya");
        assert_eq!(step("uya", false), "dl");
        assert_eq!(step("my-moves.ron", true), "uya");
    }
}

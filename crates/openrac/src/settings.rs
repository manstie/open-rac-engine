//! The player's settings file and the app's directories (PLAN.md P1.1).

use anyhow::{Context, Result};
// Named imports: the Bevy prelude's `ContextExt` would clash with anyhow's `Context`.
use bevy::prelude::{Resource, warn};
use directories::ProjectDirs;
use rac_input::InputSettings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Everything saved in `settings.toml`. Missing fields take their defaults, so files
/// written by older builds keep loading.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub input: InputSettings,
    /// The hero's model, by name (`rac_rig::RigModels`); the open model if unset.
    pub hero_model: Option<String>,
    /// Which game the hero moves like (`crate::moves`); UYA if unset.
    pub hero_moves: Option<String>,
    /// The hero camera's setting (`crate::camera`); the game's own if unset.
    pub hero_camera: Option<String>,
}

/// Where the app keeps its files.
#[derive(Resource, Debug, Clone)]
pub struct Paths {
    pub settings_file: PathBuf,
    /// Extracted and cached data. In development builds this is `./cache`, the
    /// repository's ignored cache directory, so nothing lands in the user's profile.
    pub data_dir: PathBuf,
}

impl Paths {
    /// The platform's standard locations, with `settings_override` taking the place of the
    /// settings file when given.
    pub fn resolve(settings_override: Option<PathBuf>) -> Result<Self> {
        let dirs = ProjectDirs::from("", "", "openrac");
        let settings_file = match settings_override {
            Some(path) => path,
            None => dirs
                .as_ref()
                .context("no home directory to keep settings in; pass --settings <file>")?
                .config_dir()
                .join("settings.toml"),
        };
        let data_dir = if cfg!(debug_assertions) {
            PathBuf::from("cache")
        } else {
            dirs.as_ref()
                .context("no home directory to keep data in")?
                .data_dir()
                .to_path_buf()
        };
        Ok(Self {
            settings_file,
            data_dir,
        })
    }
}

/// Reads the settings file. A missing file gives the defaults and is written out, so
/// players have a file to edit. A file that doesn't parse also gives the defaults, but is
/// left alone so a typo doesn't cost the player their settings.
pub fn load_or_create(path: &Path) -> Settings {
    match std::fs::read_to_string(path) {
        Ok(text) => match toml::from_str(&text) {
            Ok(settings) => settings,
            Err(error) => {
                warn!(
                    "{} is not valid, so the default settings apply until it is fixed: {error}",
                    path.display()
                );
                Settings::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let settings = Settings::default();
            if let Err(error) = save(path, &settings) {
                warn!("could not write default settings: {error:#}");
            }
            settings
        }
        Err(error) => {
            warn!(
                "could not read {}, so the default settings apply: {error}",
                path.display()
            );
            Settings::default()
        }
    }
}

pub fn save(path: &Path, settings: &Settings) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let text = toml::to_string_pretty(settings).context("serialising settings")?;
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openrac-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn a_missing_file_is_created_with_defaults() {
        let path = temp_file("missing.toml");
        let _ = std::fs::remove_file(&path);
        assert_eq!(load_or_create(&path), Settings::default());
        assert!(path.exists());
        assert_eq!(load_or_create(&path), Settings::default());
    }

    #[test]
    fn a_broken_file_is_kept() {
        let path = temp_file("broken.toml");
        std::fs::write(&path, "input = 3").unwrap();
        assert_eq!(load_or_create(&path), Settings::default());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "input = 3");
    }

    #[test]
    fn saved_settings_load_back() {
        let path = temp_file("saved.toml");
        let mut settings = Settings::default();
        settings.input.invert_look_y = true;
        settings.hero_model = Some("uya".into());
        settings.hero_moves = Some("dl".into());
        settings.hero_camera = Some("lock-strafe".into());
        save(&path, &settings).unwrap();
        assert_eq!(load_or_create(&path), settings);
    }
}

//! Builds the `PCSX2.ini` for the private, headless PCSX2 that replays input recordings.
//!
//! It starts from the owner's own settings, because emulation settings (CPU, game fixes,
//! patches, BIOS) must match the ones the recording was made with or the replay drifts.
//! Only settings that matter for running unattended are changed: no prompts, no sound, no
//! controllers, PINE on a separate slot, an OpenGL renderer (which works on Xvfb through
//! Mesa's software rasteriser), and an optional speed limit. Optionally, controller 1 is
//! bound to the keyboard so a script can play the game with xdotool (see [`KEYBOARD_PAD`]).

/// The settings the headless instance changes.
#[derive(Debug, Clone)]
pub struct HeadlessSettings {
    /// Absolute path of the folder holding the BIOS files.
    pub bios_dir: String,
    /// PINE slot, so the headless instance never collides with one the owner is using.
    pub pine_slot: u16,
    /// Emulation speed as a fraction of full speed. Below 1.0 the frame limiter gives the
    /// capture more host time per frame.
    pub speed: f32,
    /// Bind controller 1 to the keyboard ([`KEYBOARD_PAD`]) instead of the owner's pads.
    pub keyboard_pad: bool,
}

/// Controller 1's keyboard bindings with `keyboard_pad`, as (pad button, key). Return is
/// Start; none of the keys is one of PCSX2's default hotkeys. The right stick is R, F, D
/// and G (up, down, left, right): DL's opening tutorial waits for it.
pub const KEYBOARD_PAD: [(&str, &str); 22] = [
    ("Up", "Keyboard/Up"),
    ("Down", "Keyboard/Down"),
    ("Left", "Keyboard/Left"),
    ("Right", "Keyboard/Right"),
    ("Cross", "Keyboard/X"),
    ("Circle", "Keyboard/C"),
    ("Square", "Keyboard/S"),
    ("Triangle", "Keyboard/T"),
    ("Start", "Keyboard/Return"),
    ("Select", "Keyboard/Backspace"),
    ("L1", "Keyboard/Q"),
    ("R1", "Keyboard/E"),
    ("L2", "Keyboard/Z"),
    ("R2", "Keyboard/V"),
    ("LUp", "Keyboard/I"),
    ("LDown", "Keyboard/K"),
    ("LLeft", "Keyboard/J"),
    ("LRight", "Keyboard/L"),
    ("RUp", "Keyboard/R"),
    ("RDown", "Keyboard/F"),
    ("RLeft", "Keyboard/D"),
    ("RRight", "Keyboard/G"),
];

fn overrides(s: &HeadlessSettings) -> Vec<(&'static str, &'static str, String)> {
    let t = |v: &str| v.to_string();
    let mut list = vec![
        ("UI", "ConfirmShutdown", t("false")),
        ("UI", "SetupWizardIncomplete", t("false")),
        ("UI", "StartFullscreen", t("false")),
        ("UI", "PauseOnFocusLoss", t("false")),
        ("UI", "InhibitScreensaver", t("false")),
        ("Folders", "Bios", s.bios_dir.clone()),
        ("EmuCore", "EnablePINE", t("true")),
        ("EmuCore", "PINESlot", s.pine_slot.to_string()),
        ("EmuCore", "EnableRecordingTools", t("true")),
        ("EmuCore", "SaveStateOnShutdown", t("false")),
        ("EmuCore", "InhibitScreensaver", t("false")),
        ("EmuCore", "EnableDiscordPresence", t("false")),
        // 12 is OpenGL in PCSX2's renderer enum.
        ("EmuCore/GS", "Renderer", t("12")),
        ("SPU2/Output", "Backend", t("Null")),
        ("SPU2/Output", "OutputMuted", t("true")),
        ("InputSources", "SDL", t("false")),
        ("InputSources", "Mouse", t("false")),
        ("GameList", "RecursivePaths", String::new()),
        ("Achievements", "Enabled", t("false")),
        ("Framerate", "NominalScalar", s.speed.to_string()),
    ];
    if s.keyboard_pad {
        list.push(("InputSources", "Keyboard", t("true")));
        list.push(("Pad1", "Type", t("DualShock2")));
        list.extend(KEYBOARD_PAD.iter().map(|&(b, k)| ("Pad1", b, t(k))));
    }
    list
}

/// Window geometry from the owner's desktop is meaningless on a virtual display.
const DROPPED_KEYS: [(&str, &str); 2] = [("UI", "MainWindowGeometry"), ("UI", "MainWindowState")];

/// Rewrites `source` (the text of an existing `PCSX2.ini`) with the headless overrides.
pub fn headless_ini(source: &str, settings: &HeadlessSettings) -> String {
    let overrides = overrides(settings);
    let mut done = vec![false; overrides.len()];
    let mut out: Vec<String> = Vec::new();
    let mut section = String::new();

    // Appends overrides for `section` that the source didn't contain, ahead of the blank
    // lines that close the section.
    let flush = |section: &str, out: &mut Vec<String>, done: &mut [bool]| {
        let trailing = out.iter().rev().take_while(|l| l.is_empty()).count();
        let at = out.len() - trailing;
        let missing: Vec<String> = overrides
            .iter()
            .zip(done.iter_mut())
            .filter(|((s, _, _), d)| *s == section && !**d)
            .map(|((_, k, v), d)| {
                *d = true;
                format!("{k} = {v}")
            })
            .collect();
        out.splice(at..at, missing);
    };

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            flush(&section, &mut out, &mut done);
            section = trimmed[1..trimmed.len() - 1].to_string();
            out.push(line.to_string());
            continue;
        }
        if let Some((key, _)) = trimmed.split_once('=') {
            let key = key.trim();
            if DROPPED_KEYS.contains(&(section.as_str(), key)) {
                continue;
            }
            if let Some(i) = overrides
                .iter()
                .position(|(s, k, _)| *s == section && *k == key)
            {
                out.push(format!("{key} = {}", overrides[i].2));
                done[i] = true;
                continue;
            }
        }
        out.push(line.to_string());
    }
    flush(&section, &mut out, &mut done);

    // Sections the source didn't have at all.
    let mut missing_sections: Vec<&str> = Vec::new();
    for ((s, _, _), d) in overrides.iter().zip(&done) {
        if !*d && !missing_sections.contains(s) {
            missing_sections.push(s);
        }
    }
    for s in missing_sections {
        out.push(String::new());
        out.push(format!("[{s}]"));
        flush(s, &mut out, &mut done);
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_adds_and_keeps_settings() {
        let source = "[UI]\nConfirmShutdown = true\nMainWindowGeometry = abc\nTheme = dark\n\n[EmuCore]\nEnablePINE = false\nEnableCheats = false\n\n[Framerate]\nNominalScalar = 1\n";
        let settings = HeadlessSettings {
            bios_dir: "/bios".into(),
            pine_slot: 28012,
            speed: 0.5,
            keyboard_pad: false,
        };
        let ini = headless_ini(source, &settings);
        assert!(ini.contains("ConfirmShutdown = false\n"));
        assert!(!ini.contains("MainWindowGeometry"));
        assert!(
            ini.contains("Theme = dark\n"),
            "unrelated settings are kept"
        );
        assert!(ini.contains("EnableCheats = false\n"));
        assert!(ini.contains("EnablePINE = true\nEnableCheats = false\nPINESlot = 28012\n"));
        assert!(ini.contains("NominalScalar = 0.5\n"));
        assert!(
            ini.contains("[Folders]\nBios = /bios\n"),
            "missing sections are added"
        );
        assert!(ini.contains("[SPU2/Output]\nBackend = Null\nOutputMuted = true\n"));
        assert_eq!(ini.matches("PINESlot").count(), 1);
        assert!(
            !ini.contains("Keyboard/"),
            "the owner's pads are left alone"
        );
    }

    #[test]
    fn binds_pad_one_to_the_keyboard() {
        let source = "[InputSources]\nKeyboard = false\nSDL = true\n\n[Pad1]\nType = DualShock2\nCross = SDL-0/FaceSouth\nR3 = SDL-0/RightStick\n";
        let settings = HeadlessSettings {
            bios_dir: "/bios".into(),
            pine_slot: 28013,
            speed: 1.0,
            keyboard_pad: true,
        };
        let ini = headless_ini(source, &settings);
        assert!(ini.contains("[InputSources]\nKeyboard = true\nSDL = false\n"));
        assert!(ini.contains("Cross = Keyboard/X\n"));
        assert!(ini.contains("Start = Keyboard/Return\n"));
        assert!(ini.contains("RLeft = Keyboard/D\n"));
        assert_eq!(ini.matches("Cross =").count(), 1);
        assert!(
            ini.contains("R3 = SDL-0/RightStick\n"),
            "unbound buttons keep their (disabled) SDL binding"
        );
    }
}

//! Builds the `PCSX2.ini` for the private, headless PCSX2 that replays input recordings.
//!
//! It starts from the owner's own settings, because emulation settings (CPU, game fixes,
//! patches, BIOS) must match the ones the recording was made with or the replay drifts.
//! Only settings that matter for running unattended are changed: no prompts, no sound, no
//! controllers, PINE on a separate slot, an OpenGL renderer (which works on Xvfb through
//! Mesa's software rasteriser), and an optional speed limit. Optionally, controller 1 is
//! bound to the keyboard so a script can play the game with xdotool (see [`KEYBOARD_PAD`]),
//! and controller 2 to other keys for Deadlocked's two-player campaign ([`KEYBOARD_PAD2`]).

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
    /// Bind controller 2 to the keyboard ([`KEYBOARD_PAD2`]) as well; otherwise port 2 is
    /// left as the owner has it.
    pub keyboard_pad2: bool,
    /// With `keyboard_pad`, PCSX2's analog sensitivity for both sticks (`AxisScale`), so a
    /// held key tilts a stick only part-way: keys alone give full tilt, and stick-response
    /// tests need less.
    pub axis_scale: Option<f32>,
}

/// Controller 1's keyboard bindings with `keyboard_pad`, as (pad button, key). Return is
/// Start; none of the keys is one of PCSX2's default hotkeys. The right stick is R, F, D
/// and G (up, down, left, right): DL's opening tutorial waits for it. L3 and R3 are for
/// DL's co-op, whose team moves need both players to click a stick.
pub const KEYBOARD_PAD: [(&str, &str); 24] = [
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
    ("L3", "Keyboard/Comma"),
    ("R3", "Keyboard/Slash"),
];

/// Controller 2's keyboard bindings with `keyboard_pad2`, none shared with controller 1 or
/// PCSX2's default hotkeys. P is Start; the sticks are U, H, A, W and 7, 8, 9, 0 (up,
/// down, left, right).
pub const KEYBOARD_PAD2: [(&str, &str); 24] = [
    ("Up", "Keyboard/Home"),
    ("Down", "Keyboard/End"),
    ("Left", "Keyboard/Delete"),
    ("Right", "Keyboard/PageDown"),
    ("Cross", "Keyboard/N"),
    ("Circle", "Keyboard/M"),
    ("Square", "Keyboard/B"),
    ("Triangle", "Keyboard/Y"),
    ("Start", "Keyboard/P"),
    ("Select", "Keyboard/O"),
    ("L1", "Keyboard/1"),
    ("R1", "Keyboard/2"),
    ("L2", "Keyboard/3"),
    ("R2", "Keyboard/4"),
    ("L3", "Keyboard/5"),
    ("R3", "Keyboard/6"),
    ("LUp", "Keyboard/U"),
    ("LDown", "Keyboard/H"),
    ("LLeft", "Keyboard/A"),
    ("LRight", "Keyboard/W"),
    ("RUp", "Keyboard/7"),
    ("RDown", "Keyboard/8"),
    ("RLeft", "Keyboard/9"),
    ("RRight", "Keyboard/0"),
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
        if let Some(scale) = s.axis_scale {
            list.push(("Pad1", "AxisScale", scale.to_string()));
        }
    }
    if s.keyboard_pad2 {
        list.push(("InputSources", "Keyboard", t("true")));
        list.push(("Pad2", "Type", t("DualShock2")));
        list.extend(KEYBOARD_PAD2.iter().map(|&(b, k)| ("Pad2", b, t(k))));
        if let Some(scale) = s.axis_scale {
            list.push(("Pad2", "AxisScale", scale.to_string()));
        }
    }
    // Both pads push the same InputSources key; keep the first.
    let mut seen = Vec::new();
    list.retain(|&(section, key, _)| {
        let new = !seen.contains(&(section, key));
        seen.push((section, key));
        new
    });
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
            keyboard_pad2: false,
            axis_scale: Some(0.5),
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
            !ini.contains("Keyboard/") && !ini.contains("AxisScale"),
            "the owner's pads are left alone"
        );
    }

    #[test]
    fn binds_pad_one_to_the_keyboard() {
        let source = "[InputSources]\nKeyboard = false\nSDL = true\n\n[Pad1]\nType = DualShock2\nCross = SDL-0/FaceSouth\nAnalog = SDL-0/Guide\n";
        let settings = HeadlessSettings {
            bios_dir: "/bios".into(),
            pine_slot: 28013,
            speed: 1.0,
            keyboard_pad: true,
            keyboard_pad2: false,
            axis_scale: None,
        };
        let ini = headless_ini(source, &settings);
        assert!(ini.contains("[InputSources]\nKeyboard = true\nSDL = false\n"));
        assert!(ini.contains("Cross = Keyboard/X\n"));
        assert!(ini.contains("Start = Keyboard/Return\n"));
        assert!(ini.contains("RLeft = Keyboard/D\n"));
        assert_eq!(ini.matches("Cross =").count(), 1);
        assert!(
            ini.contains("Analog = SDL-0/Guide\n"),
            "unbound buttons keep their (disabled) SDL binding"
        );
        assert!(!ini.contains("AxisScale"));
    }

    #[test]
    fn scales_the_keyboard_sticks() {
        let source = "[Pad1]\nType = DualShock2\nAxisScale = 1.33\n";
        let settings = HeadlessSettings {
            bios_dir: "/bios".into(),
            pine_slot: 28013,
            speed: 1.0,
            keyboard_pad: true,
            keyboard_pad2: false,
            axis_scale: Some(0.5),
        };
        let ini = headless_ini(source, &settings);
        assert!(ini.contains("AxisScale = 0.5\n"));
        assert_eq!(ini.matches("AxisScale").count(), 1);
    }

    #[test]
    fn binds_pad_two_for_co_op() {
        let source = "[InputSources]\nKeyboard = false\n\n[Pad1]\nType = DualShock2\n\n[Pad2]\nType = None\n";
        let settings = HeadlessSettings {
            bios_dir: "/bios".into(),
            pine_slot: 28019,
            speed: 1.0,
            keyboard_pad: true,
            keyboard_pad2: true,
            axis_scale: Some(0.5),
        };
        let ini = headless_ini(source, &settings);
        assert_eq!(ini.matches("Keyboard = true").count(), 1);
        assert!(ini.contains("[Pad2]\nType = DualShock2\n"));
        assert!(ini.contains("Start = Keyboard/P\n"));
        assert_eq!(ini.matches("AxisScale = 0.5").count(), 2);
        assert!(
            KEYBOARD_PAD2
                .iter()
                .all(|(_, k)| KEYBOARD_PAD.iter().all(|(_, k1)| k1 != k)),
            "the two pads share no key"
        );
    }
}

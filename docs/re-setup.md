# Reverse-engineering setup

How the reverse-engineering tools are installed and configured (PLAN.md task RE.1). The
layout below is on the owner's Linux machine. Follow the same steps on another machine.

Nothing here needs sudo. Everything lives under `~/.local`.

## What is installed

| Tool | Version | Location | Launcher |
|---|---|---|---|
| Eclipse Temurin JDK | 21.0.12.1 | `~/.local/opt/jdk-21.0.12.1+1/` | Used only by the Ghidra launchers (not on `PATH`) |
| Ghidra | 12.1.4 | `~/.local/opt/ghidra_12.1.4_PUBLIC/` | `ghidra` (GUI), `ghidra-headless` (`analyzeHeadless`) |
| ghidra-emotionengine-reloaded | v2.1.38 (build for Ghidra 12.1.4) | `…/ghidra_12.1.4_PUBLIC/Ghidra/Extensions/ghidra-emotionengine-reloaded/` | Loaded automatically |
| PCSX2 | 2.8.2 (official AppImage, extracted) | `~/.local/opt/pcsx2-v2.8.2/squashfs-root/` | `pcsx2` |
| xdotool | system package | `/usr/bin/xdotool` | Drives PCSX2's menus during replays |
| Xvfb | 21.1.12 (Ubuntu `xvfb` package, installed by the owner on 2026-10-12) | `/usr/bin/Xvfb` | Virtual display for replays and headless boots |

- **Launchers:** they are small shell scripts in `~/.local/bin`. On Ubuntu, `~/.profile` adds
  that folder to `PATH` at login. Until your next login, call them by their full path.
- **App menu:** `~/.local/share/applications/` also has entries for PCSX2 and Ghidra.
- **Sources:**

  | Tool | Where it came from |
  |---|---|
  | Ghidra | `NationalSecurityAgency/ghidra` GitHub releases |
  | PS2 extension | `chaoticgd/ghidra-emotionengine-reloaded` GitHub releases |
  | PCSX2 | `PCSX2/pcsx2` GitHub releases |
  | JDK | The Adoptium API |

- **Verification:** every download was checked against its published SHA-256 before it was
  unpacked.

### Two install quirks

1. **Ghidra needs a `java` on `PATH` or a `JAVA_HOME` to bootstrap.** That is true even with
   `JAVA_HOME_OVERRIDE` set in `support/launch.properties`, because the launcher needs Java
   just to read that setting. The wrapper scripts export `JAVA_HOME` to the private JDK,
   which keeps Java off the global `PATH`.
2. **The PCSX2 AppImage needs `libfuse.so.2`**, which Ubuntu 24.04 doesn't ship by default.
   Instead of installing FUSE with sudo, the AppImage was unpacked with `--appimage-extract`,
   and the launcher runs `squashfs-root/AppRun`.

## PCSX2 configuration

- **Config and data folder:** `~/.config/PCSX2/`.
  - The settings file is `inis/PCSX2.ini`.
  - It was created without opening a window by running `pcsx2 -testconfig`, which writes
    the full default config and exits.
- **Changes from the defaults:**

  | Section | Key | Value | Why |
  |---|---|---|---|
  | `[EmuCore]` | `EnablePINE` | `true` | Lets our tools read and write emulated memory (slot 28011) |
  | `[GameList]` | `RecursivePaths` | `~/GitHub/open-rac-engine/source` (absolute path) | The game discs show up in the game list |

- **The first-run setup wizard is left on.** It asks you to choose the BIOS and map a
  controller. Everything else uses PCSX2's defaults.
- **BIOS:** dumps live in **`~/.config/PCSX2/bios/`**. Since 2026-10-11 the owner's
  EUR (SCPH-77004, the selected one), USA (SCPH-90001) and JAP (SCPH-77000) dumps are
  installed.
  - A dump usually includes a 4 MB `.bin` plus optional `.rom1`, `.rom2`, `.erom`, `.nvm`
    and `.mec` files. Keep them together with the same base name.
  - The BIOS must come from your own console; see PCSX2's
    [BIOS guide](https://pcsx2.net/docs/setup/bios).
  - A BIOS from any region runs all of our discs.
  - **Never copy a BIOS into this repository.**
- **Savestates** go in `~/.config/PCSX2/sstates/`, named `<serial> (<crc>).<slot>.p2s`. They
  are zip archives whose members are Zstandard-compressed (zip method 93), which Python's
  `zipfile` and Ghidra cannot read. `pine-probe savestate-list`, `savestate-extract` and
  `savestate-peek` read them directly.
- **Default hotkeys:**

  | Key | Action |
  |---|---|
  | F1 | Save state to the current slot |
  | F2 | Next slot (Shift+F2 for the previous slot) |
  | F3 | Load state |
  | F8 | Screenshot |
  | Space | Pause |
  | Esc | Pause menu |
  | Shift+R | Toggle input-recording mode |

- **PINE** listens on the Unix socket `$XDG_RUNTIME_DIR/pcsx2.sock` while a game is running.
  `tools/pine-probe` (RE.5) will connect to it with the `pine-ipc` crate.
- The debugger is documented at <https://pcsx2.net/docs/advanced/debugger>.

## Ghidra usage

**Headless analysis** is what Claude sessions use. For example:

```sh
ghidra-headless /path/to/open-rac-engine/cache/ghidra uya_ntsc \
  -import /path/to/unpacked/game.elf
```

- **Processor:** the PS2 extension detects R5900 ELFs automatically as language
  `r5900:LE:32:default:default`.
- **Harmless warning:** "Failed to add language defined memory block … rom0" just means the
  extension's BIOS ROM region overlapped a block the importer had already created.
- **Where projects go:** keep Ghidra projects and anything exported from them (function
  lists, decompiled code) in `/cache/ghidra/` or outside the repo. They are derived from the
  game, so they are never committed.
- **Savestates:** the extension advertises a savestate importer, but headless import of a
  `.p2s` fails with "No load spec found", even after re-packing it with Deflate. What works
  (verified 2026-10-11, 7 s) is importing the raw EE memory, which holds the unpacked game
  plus the current level's overlay:

  ```sh
  pine-probe savestate-extract "<state>.p2s" eeMemory.bin -o cache/ghidra/<name>.ee.bin
  ghidra-headless cache/ghidra <project> -import cache/ghidra/<name>.ee.bin \
    -processor r5900:LE:32:default -loader BinaryLoader -loader-baseAddr 0 -noanalysis
  ```

  No functions are defined after a raw import, so on its own, analysis has nowhere to
  start. `tools/ghidra/import-savestate.sh` does the whole job; see the next section.
- **The GUI** (`ghidra`) is optional. It shows a user agreement on first launch.

## Static analysis of a savestate (RE.3)

How the hero code was found and read on 2026-10-12. The scripts are in `tools/ghidra/`.

```sh
tools/ghidra/import-savestate.sh "<state>.p2s" <name> [re/<serial>/symbols.toml]
ghidra-headless cache/ghidra/proj <name> -process <name>.ee.bin -noanalysis -readOnly \
  -scriptPath tools/ghidra -postScript DecompileAt.java cache/ghidra/<out>.c <address hex> ...
```

- **`import-savestate.sh`** (about 3 minutes) does these steps:
  1. Extracts `eeMemory.bin` to `cache/ghidra/ee/<name>.ee.bin`.
  2. Reads `$gp` with `pine-probe savestate-regs`.
  3. Imports the memory as a raw R5900 binary into `cache/ghidra/proj/<name>`.
  4. Runs `SeedFunctions.java` before auto-analysis, which:
     - sets `$gp` everywhere, so gp-relative globals resolve;
     - finds the code regions: 16 KiB blocks with at least three `jr ra` and three
       stack-frame prologues;
     - creates functions at every prologue that follows a return, and at every target
       called twice or more.
  5. If a `symbols.toml` is given, `ApplySymbols.java` names its functions and data.
- **`DecompileAt.java`** decompiles the functions containing the given addresses into one
  file, with each function's callers. **That output is decompiled game code: keep it in
  `cache/`, never in the repository.** What we learn goes into `docs/research/` in our own
  words and into `re/<serial>/symbols.toml`.
- **`RefsTo.java`** (same arguments: an output file, then addresses) lists every
  instruction that reads or writes each address, with the function it is in. Hero fields
  are gp-relative or based on the hero's address, so the import's `$gp` must be set.
  - That is how the wall-contact window and the ledge block were found on 2026-10-12.
  - The listing names game addresses only, but still keep it in `cache/`.
- **Finding things:**
  - The fastest way in is a value from a trace. For example, find the code that writes a
    measured constant (search the image for the `lui`/`ori` pair that loads it), or the
    code that reads or writes a field of a known struct.
  - Compare the two R&C3 PAL images (or a PAL and an NTSC image) to separate code from
    data.
- **Gotchas:**
  - Code addresses are per level. Each level overlay holds its own build of the game
    program, so an address found on Veldin is wrong anywhere else. The title screen's
    image has no hero code at all.
  - R&C3/UYA address the hero as a static global at 0x1A4BE0, so the decompiler shows
    its fields as `DAT_001a....` globals rather than struct offsets.
  - Gladiator/DL pass the hero as a pointer argument.
  - Gladiator's `$gp` is odd (0x2259FF) on purpose; its gp-relative offsets are odd too.
  - Ghidra misses switch cases on some jump tables (Gladiator's `hero_change_state`).
    Read the table from memory: one word per case.
  - Float constants usually load as `lui` plus `ori`. A bare `lui` match on the upper
    half also hits unrelated values, so check the lower half.
  - Two headless runs can't open the same Ghidra project at once. Give each image its own
    project, as the script does.
  - `pgrep -f` or `pkill -f` with a pattern from the PCSX2 command line also matches the
    shell running it. Kill PCSX2 by PID.

## NTSC savestates without a capture session (RE.8)

The NTSC discs can be booted headlessly and played with xdotool, so savestates of a level
don't need the owner. Recordings of moves still do.

```sh
tools/pine-probe/boot.sh "<disc>.iso" <name>     # leave it running
export DISPLAY=:98
W=$(xdotool search --onlyvisible --classname . | tail -n 1)   # the render window
xdotool windowfocus --sync "$W"
xdotool keydown Return; sleep 0.15; xdotool keyup Return      # press Start
xdotool key F8    # screenshot -> cache/pcsx2/boot/<name>/xdg/PCSX2/snaps/
xdotool key F1    # savestate  -> cache/pcsx2/boot/<name>/xdg/PCSX2/sstates/
```

- **What `boot.sh` does:** it starts a private PCSX2 on Xvfb `:98` (PINE slot 28013), with
  the owner's emulation settings, copies of their memory cards, and controller 1 bound to
  the keyboard (`pine-probe headless-config --keyboard-pad`):

  | Pad | Key | Pad | Key |
  |---|---|---|---|
  | D-pad | arrow keys | Start, Select | Return, Backspace |
  | Cross, Circle | X, C | Square, Triangle | S, T |
  | L1, R1 | Q, E | L2, R2 | Z, V |
| Left stick | I, J, K, L | | |

- **Hold each key for about 150 ms** (`keydown`, sleep, `keyup`). A bare `xdotool key`
  can be too short for the game to see.
- **Look before every step:** take a screenshot (F8) and look at it.
- **Paths used on 2026-10-12.** Both took about two minutes, and both games accept a new
  game without creating a save:
  - **UYA (SCUS-97353):**
    1. Wait about 60 s, then press Start at "Press START".
    2. Down to New Game, Cross.
    3. Triangle (don't create a save), then Cross (continue anyway).
    4. Start skips the opening video.
    5. Veldin loads in about 15 s.
  - **Deadlocked (SCUS-97465):**
    1. Start at the title.
    2. Single Player, then New Game.
    3. Cross on No (create a save?), then Right and Cross (continue anyway).
    4. Cross for the default difficulty.
    5. Start skips the intro.
    6. DreadZone Station loads, and an in-engine cutscene plays.
- **Saving:** a savestate taken during a level's opening cutscene already holds that
  level's overlay. Copy it out of the boot folder straight away, before the next boot
  overwrites it.
  - Copy it to `source/<game>/<release>-<level>-<what>.p2s`, which is where files worth
    keeping live.
  - Make a working copy in `cache/pcsx2/ntsc-states/` if you need one.
- **Coming back:** `boot.sh` keeps `cache/pcsx2/boot/<name>/` between runs. After
  booting, F3 loads savestate slot 1, so a saved gameplay state skips the menus.

### Making traces with scripted play

With a gameplay savestate loaded, a trace of scripted moves takes a minute:

```sh
target/release/pine-probe capture --layout re/SCUS-97353/hero.toml --slot 28013 \
  --seconds 60 --out cache/pcsx2/traces/<name>.trace &
# then drive the keys with xdotool: keydown, sleep, keyup (see the table above)
```

- **Speed:** with the software renderer on Xvfb the game ran at about 28 frames per real
  second. The capture still gets every frame, and xdotool timings simply stretch in game
  time.
- **The layout records the pad:** UYA's `hero.toml` has a `pad` region, so the trace
  shows what the game saw on each frame.
- **Moves on UYA:**
  - X jumps; R1 crouches; Square swings the wrench.
  - Run, then R1 and X: long jump. R1, then X: heli jump.
  - Hold X after a double jump: glide.
  - L2 or R2 with left, right or back, then X: flips.
  - L1 is first-person look.
- **Keyboard limits:** the keys give full stick tilt only, so stick-response tests still
  need a real controller or a recording.
- **Capture in gameplay only:** UYA's frame counter runs faster on the title screen.
- **Keep the script and the trace together.** Write the key sequence as a script, not
  ad-hoc commands, and save it as `source/<game>/<trace name>.sh`.
  - Give it a header saying which savestate and boot it ran from.
  - Copy the trace beside it. xdotool timing isn't reproducible, so the trace is the only
    exact record of what happened.

## Captures from the owner

### What exists (2026-10-12)

The owner captured the **PAL** releases first. Everything below is game-derived, so it
stays out of the repository.

| Release | Where | What |
|---|---|---|
| R&C3 PAL (SCES-52456), Veldin | `~/.config/PCSX2/sstates/SCES-52456 (17125698).0{1..5}.p2s` | 1: open ground with enemies ahead. 2: next to crates. 3: hanging from a crate's ledge (state 24). 4: at a cliff edge. 5: enemy in view, weapon out. |
| R&C3 PAL | `source/Ratchet & Clank 3/save-state-{5,6}-inputs` plus `_SaveState.p2s` | Input recordings of free play from the cliff edge: 1,823 and 1,383 frames of running, jumps, double jumps, crouch jumps, wrench combos and firing. |
| Gladiator PAL (SCES-53285), DreadZone Station | `~/.config/PCSX2/sstates/SCES-53285 (D697D204).0{1..5}.p2s` | 1: open floor. 2: next to a wall. 3: hanging from a ledge (state 24). 4 and 5: another open area. |
| Gladiator PAL | `source/Ratchet Gladiator/save-state-{4,5}-inputs` plus `_SaveState.p2s` | Input recordings of free play: 2,512 and 2,329 frames. |
| UYA NTSC-U (SCUS-97353), Veldin | `source/Ratchet & Clank 3/uyantsc-veldin-intro.p2s` | Not from the owner: a headless new game (`tools/pine-probe/boot.sh`), saved during Veldin's opening cutscene. Used for static analysis; no recording. |
| Deadlocked NTSC-U (SCUS-97465), DreadZone Station | `source/Ratchet Gladiator/dlntsc-dreadzone-intro.p2s` | The same, saved during DreadZone Station's opening cutscene. |
| UYA NTSC-U, Veldin gameplay | `source/Ratchet & Clank 3/uyantsc-veldin-play.p2s` and `uyantsc-veldin-play-2.p2s` | Headless, at the start of Veldin's gameplay (enemies ahead). The second was taken 76 frames later, standing still, to find the frame counter. The first is the starting point for the traces below. |
| UYA NTSC-U traces | `source/Ratchet & Clank 3/uyantsc-veldin-moves-{1,2,3}.trace`, each with the `.sh` input script that drove it | Self-made with scripted keyboard play: 1, jumps (tapped and held), double jump, glide, heli jump, walk and stop, run jump and long jump; 2, wrench combo, jump attack and L1 look; 3, strafe walk, side and back flips. 2,069, 1,195 and 1,075 frames. |
| UYA NTSC-U camera trace | `source/Ratchet & Clank 3/uyantsc-veldin-cam-1.trace`, with its script `uyantsc-veldin-cam-1.sh` and layout `uyantsc-veldin-cam.toml` | Self-made, 2026-10-12, for the camera (RE.4): left stick only (runs in each direction and a circle, a held jump, a double jump, a run jump), with the camera, its update object and its follow block captured. The hero ran off the cliff ahead of the start twice and respawned. 2,673 frames. |
| R&C3 PAL camera trace | `cache/pcsx2/traces/rc3pal-veldin-5-cam.trace`, layout `source/Ratchet & Clank 3/rc3pal-veldin-5-cam.toml` | A replay of the owner's recording 5 with the camera block added (RE.4, `docs/research/camera.md`). It can be regenerated from the layout. |

Working copies of the self-made savestates and traces are also in `cache/pcsx2/ntsc-states/`
and `cache/pcsx2/traces/`.

For the replay harness, copies live in `cache/pcsx2/recordings/` as
`<release>-<level>-<n>.p2m2` with `<name>.p2m2_SaveState.p2s` beside them (PCSX2 looks for
`<recording file name>_SaveState.p2s`). Their traces are in `cache/pcsx2/traces/`.

### Still wanted

Since 2026-10-12 the movement constants come from the game code
(`docs/research/hero-controller.md`), so recordings are now for **checking** the code
reading, not for measuring. On UYA NTSC we can also make our own traces with scripted
keyboard play ("Making traces with scripted play"). From the owner, these still help:

1. **Moves that need a particular place:** wall jump, ledge grab and ledge jump,
   grinding, swimming and charge boots, on any release. A savestate next to each is
   enough, and we can script the move from there. A recording is better.
2. **Half-tilted stick:** keyboard play only gives full tilt. A recording that holds the
   stick part-way for a few seconds, on UYA or DL NTSC, would check the dead zone and
   magnitude rules on NTSC.
3. **DL NTSC gameplay:** a savestate on DreadZone Station in play rather than in a
   cutscene. Booting a new game gets there too, but the owner's would save time.

### How to make a recording

1. Load a savestate, then **Tools → Input Recording → New**, choose "from savestate", and
   name the file `<something>.p2m2`.
2. Play the move, then **Tools → Input Recording → Stop**.
3. Leave the `.p2m2` and its `_SaveState.p2s` together in `source/<game>/` or
   `cache/pcsx2/recordings/`.

## Replaying input recordings headlessly (RE.5)

`tools/pine-probe/replay.sh` replays a recording in a private PCSX2 and records the hero
every frame:

```sh
tools/pine-probe/replay.sh "<disc>.iso" cache/pcsx2/recordings/<name>.p2m2 \
  re/<serial>/hero.toml cache/pcsx2/traces/<name>.trace
target/release/pine-probe export cache/pcsx2/traces/<name>.trace --layout re/<serial>/hero.toml > <name>.csv
target/release/pine-probe recording cache/pcsx2/recordings/<name>.p2m2 > <name>.rec.csv
```

- **Requirements:** `xdotool` and Xvfb (`sudo apt install xvfb`; installed on the owner's
  machine since 2026-10-12). Without sudo, `apt download xvfb` and `dpkg -x xvfb_*.deb
  <dir>` give a working `<dir>/usr/bin/Xvfb` (the other libraries it needs are already on a
  desktop install); pass it as `XVFB=`.
- **What it does:**
  1. Starts Xvfb on `:99`, so nothing appears on your desktop and your keyboard focus is
     never touched.
  2. Writes a private `PCSX2.ini` (`pine-probe headless-config`) into
     `cache/pcsx2/headless/xdg/PCSX2/` from yours: same BIOS, CPU settings, patches and
     game fixes (the replay must match the recording), but no sound, no controllers, no
     prompts, OpenGL through Mesa, PINE on slot 28012 and half speed.
  3. Copies your memory cards and patches the copies' checksums (`pine-probe memcard-sync`).
  4. Boots the disc, opens **Tools → Input Recording → Play** with xdotool, starts
     `pine-probe capture`, then unpauses with Space.
  5. The capture stops 5 s after PCSX2 pauses at the end of the recording.
- **Takes** about 80 s for a 1,400-frame recording at half speed on the owner's machine.
- **Verified 2026-10-11:** all four PAL recordings replayed with no missing frames, and
  replaying the same recording three times gave identical hero data on every frame.

Things that broke the first attempts, and how the harness avoids them:

- **Memory-card eject.** A savestate stores each card's checksum. If the card was written
  since (autosave does this), PCSX2 ejects the cards on load and R&C3 stops on a "memory
  card removed" message. `memcard-sync` writes the savestate's checksum into the copied
  card (PCSX2 keeps it at byte 0x210 of the card file). See `tools/pine-probe/src/memcard.rs`.
- **Paused start.** Playback loads the savestate and leaves the emulator paused. The
  harness unpauses only after the capture is running, so frame 0 is not missed.
- **Missed frames at full speed.** With the software renderer using every core, the
  capture thread sometimes went 20 ms or more without running. Half speed, plus `nice` on
  PCSX2, removed every gap.
- **Torn reads.** PINE answers while the game runs. The capture rereads the frame counter
  after each sample and keeps the settled version of each frame (see
  `tools/pine-probe/src/capture.rs`).
- **Alignment.** The first trace record is the paused frame 0. A button pressed in
  recording frame *n* changes the hero state in trace record *n* + 1 to *n* + 3 (one or
  two frames of game-side input latency, depending on the move), so analyses should
  calibrate against a known press.
- **Settings drift.** PCSX2 rewrites its ini on exit, so the harness regenerates the
  private ini before every run.

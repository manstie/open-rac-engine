# CLAUDE.md

How to work in this repository, for Claude sessions and for people.

- **What** the project is for: [README.md](README.md).
- **Where it stands and what's next:** [PLAN.md](PLAN.md), the source of truth for scope,
  decisions, the task list and status.
- **What we know** about the discs and the engine: [docs/research/](docs/research/).

## Every session

**At the start:**
1. Read PLAN.md §0 (status and next actions) and §5 (decisions). Don't reopen a decision
   unless the owner asks.
2. Run `git status` and `git log --oneline -15`.
3. Do what the owner asks. If they name nothing, take the next action in §0.
4. Before you begin, mark the work in PLAN.md ("Mark your work in PLAN.md", below).

**Mark your work in PLAN.md.** Several sessions can run at once and any of them can end
without warning, so PLAN.md has to show what is under way and what went wrong on the way:
- **In progress.** Before starting work you've been asked for, find the PLAN task it
  belongs to; if none fits, use the closest one or add a task. Set its marker to `[~]`,
  then add a sub-bullet saying what you're doing, for example `- In progress (2026-10-15):
  fixing Clank's eyes.` Chores such as committing or tidying docs don't need one. Leave
  other sessions' `In progress` notes alone unless the owner asks you to take one over.
- **Problems met.** When something important comes up that has to be dealt with (a bug, a
  wrong assumption, a format quirk, a workaround), add a sub-bullet to the same task. Say
  what it was and how it was fixed, or that it is still open, for example `- glam's
  scalar-math broke Bevy's GPU mesh buffers; turned off, determinism comes from building
  without FMA (§5).` Keep it to one or two lines and point to `docs/research/` or a code
  comment for the details. Write it as soon as the problem is understood, not at the end.
- **When you stop**, replace your `In progress` note with what's done, what isn't, and the
  exact next step.

**As you go.** A session can end without warning, and anything only in the conversation,
`cache/` or the scratchpad is then lost. So:
- Put findings into `docs/research/` as soon as they are checked, labelled with how they
  were verified (for example `[code]`, `[verified on UYA NTSC]`).
- Keep the state of unfinished work in PLAN.md, under its task: what is done, what isn't,
  and the exact next step.
- Save every savestate, input recording, input script, capture layout and
  non-regenerable trace you make into `source/<game>/` straight away, named
  `<release>-<level>-<what>` (for example `uyantsc-veldin-play.p2s`), and list it in the
  catalogue in `docs/re-setup.md`. `source/` is gitignored; `cache/` and the scratchpad
  may be wiped.

**Delegating.** Hand easy, repetitive, well-specified work to Haiku agents at high effort
(`model: "haiku"`, `effort: "high"`) rather than Opus ones. Examples: PCSX2 captures ("Reverse-engineering
workflow"), pulling named numbers out of traces or CSVs, sweeping files for a pattern,
running commands and summarising their output, and mechanical edits that follow a set
pattern.
- **Brief them fully.** Say which rules here apply (legal, private emulator instances,
  stopping by PID, build limits), give the inputs and exact steps, say what to return and
  which files they may create or edit, and say what they must leave alone.
- **Keep the judgement in the main session.** That means design, reading results against
  the code, and anything written to `docs/research/`, the kit or PLAN.md.
- **Check before trusting.** Spot-check a sample of the work before building on it.
- **One build.** Agents in their own worktrees don't build: they stage their changes and
  the main session merges them and tests the result in one build. Worktrees can't share
  `target/` (cargo names a workspace's crates by their path from its root, so trees
  overwrite each other's builds), and a target per worktree costs a full build and tens
  of gigabytes each.

**At the end:** tick or update the tasks you touched in PLAN.md and §0's next actions,
replace your `In progress` notes, record new decisions in §5 with the date, and leave the
tree green ("Definition of done").

**How to write PLAN.md.** It records the current state, not the history: git log is the
history. Keep each task to what is done, what isn't and the next step, pointing to
`docs/research/` and the code rather than repeating them. Two kinds of sub-bullet are
allowed beyond that: a dated `In progress` note, which goes when the work stops, and notes
on problems met, which stay with the task while they help whoever works on it next. No
session log, no dated progress diaries, no test counts, and no notes on who asked for
something or whether it is committed.

## Legal rules (non-negotiable)

The repository is public, so everything committed here is published.

**Where the discs and derived data live:**
- The owner's game discs are in `source/` and are ignored by `source/.gitignore`. Keep it
  that way.
- Put anything derived from a disc in one of these places, nowhere else. All of them are
  outside git.
  - `source/<game>/`: savestates, recordings and traces worth keeping.
  - `/cache/`: working copies, and anything that can be regenerated.
  - A temp directory outside the repo.
- Before every commit, check `git status` for stray data files and run
  `tools/check-data-hygiene.sh`.

**Never commit:**
- ISOs.
- Anything extracted, decompressed or converted from a disc: WADs, textures, models, audio,
  video, ELFs, overlays, VU microcode, memory dumps, savestates, game screenshots.
- Disassembly listings or decompiled game code, even when it comes from a public
  decompilation repo.
- Sony SDK or NDA material.

**Fine to commit:**
- Our own code.
- Facts about the discs written in our own words: offsets, sizes, hashes, struct layouts,
  symbol names and addresses.
- Test fixtures our tests generate themselves.
- Checksums of extracted data.

**Licences.** This repo is GPL-3.0-or-later (PLAN §5, 2026-10-15).
- Code under a licence GPLv3 can absorb may be reused: keep its copyright and licence
  notice in the file header, and list it in `docs/research/prior-art.md`.
  - Permissive: ReRAC (ISC), OpenGOAL (ISC), Play! (BSD), ico-recomp (MIT), libdl and
    libuya (MIT).
  - Copyleft: GPL-3.0, GPL-2.0-or-later and LGPL projects such as Wrench, PCSX2,
    Replanetizer, PS2Recomp, DobieStation, m2c and the decompilation projects' own
    tooling.
- **Reference only** (read them for facts, then implement independently; never translate
  their code line by line): GPL-2.0-only code, and code with no licence (ps2tek,
  RCMemory).
- Decompiled game code stays forbidden whatever licence its project puts on it
  (rac1-decomp, Lombyte, OpenRAC's decompilations): the game's copyright is Sony's.
- The full licence table is in [docs/research/prior-art.md](docs/research/prior-art.md).

## Environment

Development is native, with no container.

- **Rust:** pinned in `rust-toolchain.toml` (1.99.0); rustup installs it on first use.
- **Linux packages** (Ubuntu/Debian): `sudo apt install build-essential pkg-config
  libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`. If they are missing and you
  can't use sudo, ask the owner. (A verification-only build can instead use `apt
  download`ed `-dev` packages unpacked into a temp sysroot, with `PKG_CONFIG_SYSROOT_DIR`
  and `PKG_CONFIG_LIBDIR` pointing at it.)
  - For reverse engineering: `binutils-mips-linux-gnu` (`mips-linux-gnu-objdump -m
    mips:5900 -D <elf>`).
  - The owner's machine links with `mold` and `clang`, set in `~/.cargo/config.toml`, not
    the repo.
- **Windows:** rustup plus the MSVC Build Tools.
- **Graphics backend:** wgpu registers Vulkan before DX12 and sorts adapters stably, so
  **Vulkan wins whenever a Vulkan driver exists**, even on Windows. Force one with
  `WGPU_BACKEND=vulkan|dx12|gl`. The shipped Windows default is open (PLAN P1.14).

## Commands

| Command | What it does |
|---|---|
| `cargo run` | Runs the game: a main menu, then the hero kit's demo level. The default workspace member is `crates/openrac`. |
| `cargo run --features dev` | Bevy's dynamic linking for faster rebuilds. Never for release builds. |
| `cargo run -- --exit-after-frames 120` | Smoke test: exits with code 0 after 120 frames. Add `--demo` to start in the level, `--spectator` for a free camera. |
| `cargo run -- --demo` with the UYA NTSC-U disc image in `source/` | The hero plays Ratchet's real animations, read from the disc at start-up (`--discs DIR` to look elsewhere). Without it his animation is procedural. |
| `cargo run -- --demo --model uya` (or `open`, `uya-snowman`, `uya-mp-robo`, `dl-jak`, …) | Chooses the hero's model: a UYA suit or multiplayer skin, a DL or Gladiator skin (names in `crates/openrac/src/hero_model.rs`, `SUITS`, `MP_SKINS` and `DL_SUITS`), or the open blocky one. The menus' Model button does the same and saves the choice. |
| `cargo run -- --demo --moves dl` | Chooses the game the hero moves like: `uya` (the default), `dl`, or a profile's RON file. The menus' Moves button does the same and saves the choice. |
| `cargo run -- --demo --camera lock-strafe` | Chooses the camera: `third-person`, `lock-strafe` or `first-person`; without it, the game's own (UYA third person, DL lock-strafe). The menus' Camera button does the same and saves the choice. |
| `cargo run -- --demo --record-input x.orin`, then `--replay-input x.orin` | Records every tick's input, then replays it. `--settings <file>` keeps test runs out of the real settings file. |
| `cargo fmt --all` | Formats the code. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lints, with warnings as errors. |
| `cargo test --workspace` | Runs the tests. |
| `cargo build --release` | Optimised build (thin LTO, one codegen unit). |
| `tools/check-data-hygiene.sh` | Fails on disc images, dumps, captures or big binaries in the tree (CI runs it; `git config core.hooksPath .githooks` runs it before each commit). |
| `cargo run -p pine-probe --release -- --help` | The reverse-engineering probe ("Reverse-engineering workflow"). |

The first full build takes about 7 minutes; incremental rebuilds of the app take seconds.

**Keep builds from starving the desktop.** The workspace links about 20 test binaries that
each contain Bevy, and an unrestricted clippy-plus-tests run once froze the owner's machine
(8 cores, 31 GB). So:
- Run heavy commands as `nice -n 10 ionice -c 2 -n 7 cargo <cmd> -j 6`; drop to `-j 4`
  if the desktop lags.
- Run clippy and the tests as separate commands, not chained.
- The dev profile keeps line tables only. Set `debug = true` locally for a debugger.

**Running the app off-screen:** start `Xvfb :97 -screen 0 1280x800x24 -nolisten tcp`, run
the app with `DISPLAY=:97 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json` (Mesa's
software Vulkan), drive it with `xdotool`, and take screenshots with `xwd -root -display
:97` and `ffmpeg`. Stop Xvfb and the app afterwards. A plain `cargo run` opens a window on
the owner's desktop.

## Workspace conventions

- **Layout:** a virtual workspace of `crates/*` and `tools/*`; `tools/*` holds
  reverse-engineering tools that never ship. The binary is `openrac`; libraries are
  `rac_*` (crate map in PLAN.md §4.3); the CLI is `openrac-tool` (`rac_tool`).
- **New crates** inherit `version`, `edition`, `license` and `repository` from the
  workspace, set `publish = false` and `[lints] workspace = true`, and take shared
  dependencies from `[workspace.dependencies]`.
- **Keep Bevy out of the core crates:** `rac_iso`, `rac_formats`, `rac_recomp` and the
  runtime core must not depend on it.
- **The hero kit is ISO-free:** `rac_sim`, `rac_input`, `rac_hero`, `rac_rig`,
  `rac_combat`, `rac_net` and `rac_demo` never depend on the disc, format, asset or
  runtime crates, so other Bevy projects can reuse them as plain plugins (PLAN P1.16).
  They ask Bevy only for the features they use (the workspace turns Bevy's defaults off;
  `openrac` turns them on), keep collision behind the kit's own interface, and keep their
  configuration in RON assets.
- **Simulation code must be deterministic**, for lockstep and rollback co-op (PLAN §4.2):
  - Use `bevy::math::ops` (Bevy's `libm` feature) instead of `f32` trig, exp and pow.
  - Never depend on `HashMap`/`HashSet` iteration order; keep entity ordering stable.
  - Draw randomness only from the level's seeded RNG.
  - Never read the wall clock or frame delta time inside `FixedUpdate`.
  - All gameplay input arrives as per-tick `PlayerInput` values.
  - Never turn on glam's `scalar-math` (for example through parry3d's
    `enhanced-determinism`): it breaks Bevy's GPU mesh buffers (PLAN §5, 2026-10-13).
    Determinism relies on building x86-64 without FMA instead.
- **Gameplay** runs in `FixedUpdate` at 60 Hz; rendering interpolates between ticks.
- **Errors:** libraries return typed errors (`thiserror`) and never panic on bad input,
  since user discs can be corrupt or unsupported. Binaries and tools may use `anyhow`.
- **Comments** explain why, not what, in the style of the surrounding code.
- **Placeholder assets** must be CC0, each recorded in `crates/openrac/assets/CREDITS.md`.

## Bevy 0.20 notes

- **Read the real API rather than recalling it.** Release notes and migration guides ship
  in `~/.cargo/registry/src/*/bevy-0.20.0/_release-content/`, examples in
  `.../bevy-0.20.0/examples/`.
- Changed recently: `AppExit` is a message (`MessageWriter<AppExit>`, `.write(...)`);
  lights use `shadow_maps_enabled`; shapes moved to `bevy_shape`; shaders are WESL; the
  BSN macros changed syntax, so prefer plain `Commands` spawning.
- **Ecosystem lag:** check a crate's Bevy dependency before adding it. Prefer crates that
  don't depend on Bevy where one fits (`ggrs`, `parry3d`, `igd-next`).
- **Upgrades:** one dedicated change per Bevy upgrade, following the migration guides.
  Raise Rust too if Bevy's MSRV needs it, and update `rust-toolchain.toml` and PLAN §5
  together.

## GPU and driver workaround

`crates/openrac/src/main.rs` sets `RenderPlugin { synchronous_pipeline_compilation: true }`.
On NVIDIA driver 595.91.07 under Linux, Bevy's parallel pipeline compilation crashes within
a second of startup (`NVVM compilation failed: 3`, then `DeviceLost` and a segfault). Don't
remove it without re-testing (PLAN P1.15).

## Game discs

The eight discs and their hashes are in
[docs/research/disc-layout.md](docs/research/disc-layout.md).

| Folder in `source/` | Release(s) |
|---|---|
| `Ratchet & Clank` | R&C1 NTSC-U SCUS-97199, and R&C1 PAL v2.00 SCES-50916 |
| `Ratchet & Clank 2` | GC NTSC-U v2.00 SCUS-97268, and GC PAL SCES-51607 |
| `Ratchet & Clank 3` | R&C3 PAL SCES-52456, and UYA NTSC-U SCUS-97353 |
| `Ratchet Gladiator` | Deadlocked NTSC-U SCUS-97465, and Gladiator PAL SCES-53285 |

- UYA (SCUS-97353) and DL (SCUS-97465) are the primary references; R&C3 and Gladiator PAL
  come next; then R&C1 and GC, with their NTSC-U discs as the references.
- **Identify a release by its boot ELF name and SHA-1**, never by the ISO's file name.
- **Tests that need discs** read the disc directory from `OPENRAC_SOURCE_DIR`, are marked
  `#[ignore = "needs game discs"]`, and run with `cargo test -- --ignored`. CI never runs
  them.

## Reverse-engineering workflow

Setup, launchers, menu paths, gotchas and the catalogue of savestates and traces are in
[docs/re-setup.md](docs/re-setup.md).

- **Tools:** PCSX2 2.8+ with PINE IPC (`$XDG_RUNTIME_DIR/pcsx2.sock`); Ghidra 12.1 with
  ghidra-emotionengine-reloaded; launchers `ghidra`, `ghidra-headless` and `pcsx2` in
  `~/.local/bin`; throwaway scripts kept outside the repo, with Python run as `python3 -I`
  on disc data.
- **Only use private research PCSX2 instances** (`tools/pine-probe/boot.sh`,
  `replay.sh`, `warp.sh`), never the owner's. Stop processes by PID, not `pkill -f`.
- **Prefer static analysis:** derive constants from the game code in Ghidra; traces verify
  them.
- **Unpacked UYA/DL code:** `openrac-tool unpack-elf --iso <disc> --out
  cache/elf/<boot>.elf` writes the boot program (the engine core and the title screen) as
  an ELF that Ghidra imports directly (about a minute). Level code, the hero's included,
  comes from savestates until PLAN P2.6: `tools/ghidra/import-savestate.sh <state.p2s>
  <name> [symbols.toml]` imports a savestate's EE memory into Ghidra and analyses it
  (about 3 minutes); use a savestate in a level, not the title screen. `DecompileAt.java`
  dumps chosen functions into `cache/`, never the repo. `wrenchbuild unpack` also works,
  and Wrench's code may be reused with its notices ("Licences").
- **Measuring the hero:** `tools/pine-probe/replay.sh` replays a recording, or `boot.sh`
  plus xdotool plays a scripted take, and `pine-probe capture` logs the hero every frame
  from a layout; `pine-probe export` writes CSV. Where the hero lives per release is in
  `re/<serial>/hero.toml`. Raw traces stay in `cache/pcsx2/traces/` (copied to
  `source/<game>/` if they can't be regenerated); only derived constants are committed.
- **Captures suit Haiku agents** ("Delegating", under "Every session"): finding a spot in
  a level, scripting and capturing takes, and saving and cataloguing the files.
  - Give each agent its own `DISPLAY_NUM` and `PINE_SLOT`, plus the savestate, keys and
    layout, what each take does and what to measure, and the file names.
  - It edits only its own files in `source/<game>/` and its catalogue rows in
    `docs/re-setup.md`.
  - Spot-check a trace or two (timings, positions, states) before writing findings from
    them.
- **Recording results:** findings in `docs/research/*.md`, labelled verified or reported;
  addresses and names in `re/<serial>/symbols.toml` (PLAN RE.2), seeded from libuya and
  libdl (MIT) with attribution.

## Definition of done

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace` all pass.
- New behaviour has tests on synthetic data, or a stated reason why it has none.
- PLAN.md and the docs are updated, and the diff contains no game data.
- Commit only when the owner asks, with descriptive messages that mention task IDs. Never
  push: pushing needs the owner's credentials.

# CLAUDE.md

How to work in this repository, for Claude sessions and for people.

- **What** the project is for: [README.md](README.md).
- **Where it stands and what's next**: [PLAN.md](PLAN.md). That is the source of truth for
  scope, decisions, the task list and the session log.
- **What we know** about the discs and the engine: [docs/research/](docs/research/).

## Every session

**At the start:**
1. Read PLAN.md §0 (status and next actions) and §5 (decisions). Don't reopen a decision
   that is already made unless the owner asks.
2. Run `git status` and `git log --oneline -15` to see recent work.
3. Do what the owner asks. If they don't name anything, take the next unchecked task in
   PLAN.md order.

**As you go.** A session can end without warning: a usage limit, a dropped remote-control
connection, a closed terminal. Anything that exists only in the conversation, `cache/` or
the scratchpad is then lost, so:
1. Write ongoing work down while you do it, not only at the end.
   - Put findings into `docs/research/` as soon as they are checked, with their status
     label.
   - Put the state of unfinished work into PLAN.md, under its task and in §0: what is
     done, what is half done, and the exact next step.
2. Save every savestate, input recording, input script and capture layout you make into
   `source/<game>/` as soon as it exists.
   - Name each file `<release>-<level>-<what>`, for example `uyantsc-veldin-play.p2s`.
   - Copy there any trace that can't be regenerated, such as one from scripted play, and
     keep it beside the script that drove it.
   - List each file in the catalogue in `docs/re-setup.md`.
   - `source/` is gitignored and is where files worth keeping live. `cache/` and the
     scratchpad hold working copies and may be wiped.

**At the end:**
1. In PLAN.md, tick or annotate the tasks you touched (refer to them by ID, e.g. `P2.4`),
   update §0 if the status changed, and add one line to the session log in §9.
2. Write new facts into `docs/research/`, saying how each one was verified. Record new
   decisions, with the date, in PLAN.md §5.
3. Leave the tree green: fmt, clippy and tests all pass (see "Definition of done").

## Legal rules (non-negotiable)

The repository is public, so everything committed here is published.

**Where the discs and derived data live:**
- The owner's game discs are in `source/` and are ignored by `source/.gitignore`. Keep it
  that way.
- Put anything derived from a disc in one of these places, nowhere else. All of them are
  outside git.
  - `source/<game>/`: savestates, recordings and traces worth keeping (see "As you go").
  - `/cache/`: working copies, and anything that can be regenerated.
  - A temp directory outside the repo.
- Before every commit, check `git status` for stray data files.

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

**Licences.** This repo is MIT.
- GPL and LGPL projects are **reference only**: read them for facts, then implement
  independently, and never translate their code line by line. That covers Wrench, PCSX2,
  OpenRAC, rac1-decomp, Lombyte, Replanetizer, PS2Recomp and m2c.
- Permissively licensed code may be reused with attribution, given both in the file header
  and in `docs/research/prior-art.md`. Examples: ReRAC (ISC), OpenGOAL (ISC), Play! (BSD),
  ico-recomp (MIT), libdl and libuya (MIT).
- The full licence table is in [docs/research/prior-art.md](docs/research/prior-art.md).

## Environment

Development is native, with no container.

- **Rust:** pinned in `rust-toolchain.toml`, currently 1.99.0. rustup installs it on first
  use, or ahead of time with `rustup toolchain install`.
- **Linux system packages** (Ubuntu/Debian):
  `sudo apt install build-essential pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`.
  - Optional, for reverse engineering: `binutils-mips-linux-gnu`, then
    `mips-linux-gnu-objdump -m mips:5900 -D <elf>`.
  - Optional, for much faster links: `mold` and `clang`, enabled per machine in
    `~/.cargo/config.toml` (not in the repo, so CI and contributors don't need them):
    ```toml
    [target.x86_64-unknown-linux-gnu]
    linker = "clang"
    rustflags = ["-C", "link-arg=-fuse-ld=mold"]
    ```
    The owner's machine has this since 2026-10-12.
  - **If the packages are missing and you can't use sudo, ask the owner to install them.**
  - For a verification-only build, you can `apt download` the `-dev` packages plus
    `libffi-dev`, unpack them with `dpkg -x` into a temp sysroot, repoint the unversioned
    `.so` symlinks at `/usr/lib/x86_64-linux-gnu`, and build with `PKG_CONFIG_SYSROOT_DIR` and
    `PKG_CONFIG_LIBDIR` pointing at it. That is how the 2026-10-11 session built.
- **Windows:** rustup plus the MSVC Build Tools.
- **Graphics backend:** wgpu (in Bevy 0.20) registers Vulkan before DX12, and its adapter
  sort is stable. So **Vulkan wins whenever a Vulkan driver exists**, even on Windows, and
  DX12 is only the fallback.
  - Force a backend with `WGPU_BACKEND=vulkan` or `WGPU_BACKEND=dx12` (or `gl`).
  - Whether DX12 should be the shipped default on Windows is still open (PLAN P1.14).

## Commands

| Command | What it does |
|---|---|
| `cargo run` | Runs the game: a main menu, then the hero kit's demo level. The default workspace member is `crates/openrac`. |
| `cargo run --features dev` | Uses Bevy's dynamic linking for faster rebuilds. Never use it for release builds. |
| `cargo run -- --exit-after-frames 120` | Smoke test. Exits with code 0 after 120 frames. Use it to check rendering works. Add `--demo` to start in the level instead of the menu, and `--spectator` to fly a free camera instead of playing the hero. |
| `cargo run -- --demo` with the UYA NTSC-U disc image in `source/` | The hero plays Ratchet's real animations, read from the disc at start-up (`--discs DIR` to look elsewhere). Without it his animation is procedural. |
| `cargo run -- --demo --record-input x.orin`, then `--replay-input x.orin` | Records every tick's input, then replays it into the same simulation. `--settings <file>` keeps test runs out of the real settings file. |
| `cargo fmt --all` | Formats the code. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lints, with warnings treated as errors. |
| `cargo test --workspace` | Runs the tests. |
| `cargo build --release` | Optimised build (thin LTO, one codegen unit). |
| `tools/check-data-hygiene.sh` | Fails on disc images, dumps, captures or big binaries in the tree (CI runs it). `git config core.hooksPath .githooks` runs it before each commit. |
| `cargo run -p pine-probe --release -- --help` | The reverse-engineering probe (see "Reverse-engineering workflow"). |

The first full build takes about 7 minutes on the owner's machine. Incremental rebuilds
of the app take seconds.

**Keep builds from starving the desktop.** The workspace links about 20 test binaries
that each contain Bevy. On 2026-10-12 an unrestricted clippy-plus-tests run froze the
owner's machine (8 cores, 31 GB) and crashed VS Code. So:
- Run heavy commands at low priority with fewer jobs, for example
  `nice -n 19 ionice -c 3 cargo test --workspace -j 4`.
- Run clippy and the tests as separate commands, not chained.
- The dev profile keeps line tables only for the same reason (`Cargo.toml`). Set
  `debug = true` locally when you need a debugger.

**Running the app without the owner's screen:** start `Xvfb :97 -screen 0 1280x800x24
-nolisten tcp`. Then run the app with `DISPLAY=:97` and
`VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json` (Mesa's software Vulkan, which is
slow but works). Drive it with `xdotool`, and take screenshots with `xwd -root -display :97`
converted by `ffmpeg`. A plain `cargo run` opens a window on the owner's desktop.

## Workspace conventions

- **Layout:** a virtual workspace whose members are `crates/*` and `tools/*`, so new crate
  directories are picked up automatically. `tools/*` holds reverse-engineering tools that
  never ship.
  - The binary is `openrac`. Libraries are named `rac_*`; the planned crate map is in
    PLAN.md §4.2.
  - A command-line tool will be `openrac-tool`.
- **New crates** inherit the shared settings: `version.workspace = true`,
  `edition.workspace = true`, `license.workspace = true`, `repository.workspace = true`,
  `publish = false` and `[lints] workspace = true`. Shared dependencies go in
  `[workspace.dependencies]`.
- **Keep Bevy out of the core crates:** `rac_iso`, `rac_formats`, `rac_recomp` and the
  runtime core must not depend on it. They should stay usable from tools and tests, and
  survive Bevy upgrades.
- **The hero kit is ISO-free:** `rac_sim`, `rac_input`, `rac_hero`, `rac_rig`,
  `rac_combat`, `rac_net` and `rac_demo` must never depend on the disc, format, asset or
  runtime crates.
  - Other Bevy projects and game mash-ups reuse them as plain plugins (PLAN P1.16).
  - They ask Bevy only for the features they use: the workspace turns Bevy's default
    features off, and `openrac` turns them back on.
  - Keep their collision behind the kit's own interface and their configuration in RON
    assets.
- **Simulation code must be deterministic.** Lockstep and rollback co-op (PLAN §4.2) need
  the same inputs to produce bit-identical state on Windows and Linux.
  - Use `bevy::math::ops` (Bevy's `libm` feature) instead of `f32` trig, exp and pow
    methods.
  - Never depend on `HashMap`/`HashSet` iteration order.
  - Keep entity ordering stable.
  - Draw randomness only from the level's seeded RNG.
  - Never read the wall clock or frame delta time inside `FixedUpdate`.
  - All gameplay input arrives as per-tick `PlayerInput` values.
- **Errors:** libraries return typed errors (`thiserror`) and never panic on bad input,
  because user discs can be corrupt or unsupported. Binaries and tools may use `anyhow`.
- **Gameplay** systems run in `FixedUpdate` at 60 Hz, and rendering interpolates between
  ticks.
- **Comments** explain why, not what. Match the style of the surrounding code.
- **Placeholder assets** must be CC0, with each file's source and licence recorded in
  `crates/openrac/assets/CREDITS.md`.

## Bevy 0.20 notes

Bevy 0.20 was released on 2026-10-08.

- **Read the real API rather than recalling it.** Bevy changes a lot between releases.
  Release notes and migration guides ship inside the crate at
  `~/.cargo/registry/src/*/bevy-0.20.0/_release-content/`, and the examples are in
  `.../bevy-0.20.0/examples/`.
- **Things that changed in 0.20 or recently:**
  - `AppExit` is a *message*: use `MessageWriter<AppExit>` and `.write(...)`.
  - Lights use `shadow_maps_enabled`.
  - Shapes moved to `bevy_shape`.
  - Shaders are WESL.
  - The BSN scene macros (`bsn!`, `bsn_list!`) changed syntax in 0.20, so prefer plain
    `Commands` spawning for now.
- **Ecosystem lag.** On 2026-10-11 these crates still targeted Bevy **0.19**:
  leafwing-input-manager 0.21, bevy_enhanced_input 0.26, avian3d 0.7, bevy_tnua 0.32,
  bevy-inspector-egui 0.37, bevy_seedling 0.8, bevy_ggrs 0.22, lightyear 0.30 and
  bevy_replicon 0.44. bevy_egui 0.43 and bevy_kira_audio 0.27 support 0.20.
  - Check a crate's Bevy dependency before adding it (crates.io, or `cargo info`).
  - Prefer libraries that don't depend on Bevy where one fits: `ggrs` (netcode), `parry3d`
    (collision queries), `igd-next` (UPnP).
- **Upgrades:** do each Bevy upgrade as one dedicated change. Follow the migration guides,
  and raise the Rust version too if Bevy's MSRV requires it. Update `rust-toolchain.toml` and
  PLAN.md §5 together.

## GPU and driver workaround

`crates/openrac/src/main.rs` turns on `RenderPlugin { synchronous_pipeline_compilation: true }`.

On NVIDIA driver 595.91.07 under Linux, Bevy's default parallel pipeline compilation
reliably crashes within about a second of startup. The driver logs
`NVVM compilation failed: 3`, wgpu reports `DeviceLost`, and the process segfaults. Don't
remove the workaround without re-testing it (PLAN P1.15).

## Game discs

The six discs and their identifying hashes are listed in
[docs/research/disc-layout.md](docs/research/disc-layout.md). In summary:

| Folder in `source/` | Release(s) |
|---|---|
| `Ratchet & Clank` | R&C1 PAL v2.00, SCES-50916 |
| `Ratchet & Clank 2` | GC PAL, SCES-51607 |
| `Ratchet & Clank 3` | R&C3 PAL SCES-52456, and UYA NTSC-U SCUS-97353 |
| `Ratchet Gladiator` | Deadlocked NTSC-U SCUS-97465, and Gladiator PAL SCES-53285 |

- **Current focus (D2, D3):**
  - UYA (SCUS-97353) and DL (SCUS-97465) are the primary references.
  - Their PAL variants, R&C3 and Gladiator, come next.
  - R&C1 and GC wait for the owner's NTSC-U discs.
- **Identify a release by its boot ELF name and SHA-1**, never by the ISO's file name.
- **Tests that need discs:** read the disc directory from `OPENRAC_SOURCE_DIR`, mark the
  test `#[ignore = "needs game discs"]`, and run them with `cargo test -- --ignored`. CI
  never runs them.

## Reverse-engineering workflow

- **Setup:** tool locations, launchers and settings are in
  [docs/re-setup.md](docs/re-setup.md).
  - The launchers in `~/.local/bin` are `ghidra`, `ghidra-headless` and `pcsx2`. Call them by
    full path if `~/.local/bin` isn't on `PATH` yet.
  - The PCSX2 config is `~/.config/PCSX2/inis/PCSX2.ini`.
  - The owner's savestates are in `~/.config/PCSX2/sstates/`. Their input recordings (each
    with a `_SaveState.p2s`) arrive in `source/<game>/` and are copied to
    `cache/pcsx2/recordings/` as `<release>-<level>-<n>.p2m2`. The catalogue of what exists
    is in `docs/re-setup.md`.
  - Savestates, input scripts and scripted-play traces that sessions make also go in
    `source/<game>/` (see "As you go").
- **Tools:**
  - PCSX2 2.8+ with its debugger, and PINE IPC at `$XDG_RUNTIME_DIR/pcsx2.sock`, slot
    28011, for live memory.
  - Ghidra 12.1 with ghidra-emotionengine-reloaded.
  - Wrench's `wrenchbuild unpack` to cross-check our extractor.
  - Throwaway scripts, kept outside the repo. Run Python on disc data with `python3 -I`.
- **Knowledge we already have:** R&C1 and GC ship plain ELFs with section headers
  (`core.*`, `lvl.*vtbl`, VU overlays). R&C3, UYA, DL and Gladiator ship a stub plus a
  WAD-compressed game. Per-level code lives in overlays.
- **Getting unpacked UYA/DL code before PLAN P2.5 exists:**
  - `tools/ghidra/import-savestate.sh <state.p2s> <name> [symbols.toml]` imports a
    savestate's EE memory into Ghidra, sets `$gp`, seeds functions and analyses it (about
    3 minutes).
    - `DecompileAt.java` dumps chosen functions. The output goes in `cache/`, never the
      repo.
    - The memory image includes the current level's overlay. Hero code is per level, and
      the title screen has none.
    - Ghidra can't open the `.p2s` itself (Zstandard members).
  - NTSC savestates: `tools/pine-probe/boot.sh` boots a disc headlessly with the keyboard
    as controller 1. Drive it with xdotool. `docs/re-setup.md` has the menu paths for UYA
    and DL.
  - Or run `wrenchbuild unpack`. Using a GPL tool is fine; copying its code is not.
- **Measuring the hero for feel-matching (PLAN RE.5, RE.6):**
  `tools/pine-probe/replay.sh` replays an input recording in a headless PCSX2 on Xvfb and
  logs the hero's memory every frame. `pine-probe export` turns the trace into CSV.
  - Where the hero lives per release: `re/<serial>/hero.toml`.
  - Results so far: `docs/research/hero-controller.md`.
  - Raw traces stay in `cache/pcsx2/traces/`. A trace that can't be regenerated is also
    copied to `source/<game>/`.
  - Commit only the derived constants and tolerances.
- **Recording results:**
  - Findings go in `docs/research/*.md`, marked as verified or reported.
  - Machine-readable facts go in the knowledge base under `re/<release>/` (PLAN RE.2).
    Seed it from libuya and libdl (MIT) with attribution.

## Definition of done

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace` all pass.
- New behaviour has tests that run on synthetic data, or a stated reason why it has none.
- PLAN.md and the docs are updated, and the diff contains no game data.
- Commit only when the owner asks. Write descriptive messages that mention task IDs.

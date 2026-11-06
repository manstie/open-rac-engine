# Open Ratchet & Clank Engine: project plan

The source of truth for **scope, decisions, status and next steps**.

- [README.md](README.md) summarises the goals (section 1) for visitors; keep it in step.
- Working rules and commands: [CLAUDE.md](CLAUDE.md).
- Findings, each labelled with how it was verified, are in [docs/research/](docs/research/):
  - [hero-controller.md](docs/research/hero-controller.md): how Ratchet moves (RE.3).
  - [camera.md](docs/research/camera.md): how the camera follows him (RE.4).
  - [weapons.md](docs/research/weapons.md): his weapons, their table, levels and mods (RE.7).
  - [effects.md](docs/research/effects.md): particles, effect textures, beams, lights
    and the guns' effects (P3.18).
  - [coop.md](docs/research/coop.md): Deadlocked's two-player campaign (RE.10).
  - [moby-format.md](docs/research/moby-format.md): moby skeletons, animation and meshes
    (P3.7).
  - [gameplay-format.md](docs/research/gameplay-format.md): levels' gameplay files (P3.10).
  - [disc-layout.md](docs/research/disc-layout.md): the discs, their TOCs and level data.
  - [engine-notes.md](docs/research/engine-notes.md): the engine.
  - [prior-art.md](docs/research/prior-art.md): other projects, tools and their licences.
- How the kit's co-op works, and the rules simulation code follows for it:
  [docs/netcode.md](docs/netcode.md).
- The reverse-engineering setup, and the catalogue of savestates, recordings and traces:
  [docs/re-setup.md](docs/re-setup.md).

Task IDs (`P1.5`, `RE.3`) are stable; use them in commits. Markers: `[x]` done, `[~]` started,
`[ ]` not started. An `In progress (<date>)` sub-bullet means a session is working on the
task now; notes on problems met stay with their task (CLAUDE.md, "Mark your work in
PLAN.md").

---

## 0. Status

The project is a **character controller for UYA and Deadlocked**, in Rust, as Bevy
plugins for game mash-ups, with a demo app around it.

- **The hero** (P1.5) has UYA's whole move set and DL's, each game a movement profile
  (`uya.ron`, `dl.ron`; `--moves` or the menus). He has the follow, lock-strafe and
  first-person cameras, lock-on, the free look's aim assist and the blends between them
  (P1.7), and melee for both games (P1.8).
- **The guns** (P1.9): all twenty of UYA's at every level, with mods, aim assist and the
  quick-select ring, and four of DL's ten (`--guns dl`); UYA's pickups, crates, vendor
  and bolt bank (P1.10); a HUD on UYA's timings with the crosshairs, lock-on reticle and
  Flux scope (P1.12). Their effects and sounds are placeholders made in code, or the
  game's own with a UYA disc (P1.13, P3.13, P3.18).
- **The rig** (P1.17) has UYA's hero's skeleton. He wears the open blocky model or, from
  the user's discs, 15 UYA suits, 21 UYA multiplayer skins or 17 DL skins, and plays
  Ratchet's real animations with a UYA disc.
- **Co-op** (P1.18): two players by IP or join code, with rollback through `ggrs`,
  checked headless on the whole demo.
- **The demo app:** menus with every control rebindable (P1.3, P1.12), a greybox level
  (P1.4) with an arena of enemies (P1.11), and input recording and replay. CI passes on
  Windows and Linux; `release.yml` builds packages, none published yet (P7.1, P7.2).
- **Reverse engineering:** the hero's movement (RE.3) and camera (RE.4) are read in UYA's
  and DL's code and checked against traces; UYA's weapons are read (RE.7) and DL's begun;
  DL's two-player campaign is read (RE.10).

**Next actions, in order:**
1. P1.18's next step, the owner's: co-op on two machines, over a LAN and then the
   internet by join code, with the router's port mapping (D8). Nothing else is left in
   it.
2. P1.9's review lists how far UYA's guns are from the game and the next to fix (the
   agents' jets, then their arms' pitch while walking). P3.18 and P1.10 have what is left in their tasks.
3. P1.16, the kit in an unrelated Bevy app: the sandbox example, the dependency check in
   CI and the public API's documentation. Then DL's other six guns (P1.9).
4. At lower priority: RE.3's moves read in code but untraced (lock-on first), the rest of
   RE.2, and the disc layer's P2.3 and P2.4.

---

## 1. Goals

| ID | Goal | Done when |
|---|---|---|
| **G1** | **A character controller for UYA and Deadlocked** in Rust: the hero's moves, camera, melee and guns, each game with its own values read from its code (D7), as Bevy plugins that other projects and game mash-ups can reuse. It runs on a placeholder rig built from primitive shapes, with a standalone demo level. With the user's own UYA or DL disc, the demo wears Ratchet's real models and animations and plays the guns' own sounds and effects, read at start-up. | The demo is playable for 5–10 minutes on Windows and Linux with no game data. The kit crates work in an unrelated Bevy app (P1.16) and depend on no disc crate. Measured movement matches the games within agreed tolerances (RE.6). |
| **G3** | Controller and keyboard+mouse support, with **key remapping**. | Every action can be remapped and the bindings persist. Gamepad and KB+M both work in menus and gameplay. |
| **G4** | **Direct-IP two-player online co-op** in the kit, with rollback. | Two PCs connect by IP or join code and play the demo together (P1.18). |

**Non-goals:**
- Running or remastering the original games; R&C1 and Going Commando.
- Competitive online multiplayer, matchmaking or hosted servers. Co-op connects the two
  players directly, with no servers of our own (D8).
- Shipping any game data.
- Contributing to other projects' repositories (D5).
- macOS, mobile or web. They aren't ruled out, just not targeted.

## 2. Ground rules

The full rules are in CLAUDE.md ("Legal rules", "Workspace conventions"). Those that shape
the plan:
- **No game data in the repo.** What the demo takes from a disc is read from the user's
  own disc at start-up and never stored (§5, 2026-10-13). CI blocks data files (P0.12).
- **GPL-3.0-or-later** (§5, 2026-10-15). Code under compatible licences may be reused with
  its notices; decompiled game code never.
- **A fixed, deterministic 60 Hz simulation** with interpolated rendering: rollback co-op
  needs bit-identical state on Windows and Linux.
- **The hero kit is ISO-free** (`rac_sim`, `rac_input`, `rac_hero`, `rac_rig`,
  `rac_combat`, `rac_net`, `rac_demo`), and the disc crates don't depend on Bevy.
- Unit tests use synthetic data; tests that need a disc are gated (section 7).

## 3. What we know (summary)

The details and evidence are in `docs/research/`.

- **Discs:** UYA NTSC-U (SCUS-97353) and DL NTSC-U (SCUS-97465) are the references. R&C3
  PAL (SCES-52456) and Gladiator PAL (SCES-53285) are their PAL builds, which the app reads
  wherever their data is the same (P2.10). The data is reached through each game's table
  of contents at LBA 1001 and compressed with Insomniac's "WAD" LZ codec
  (`disc-layout.md`).
- **Engine:** C++, built with SN ProDG GCC 2.95.3. A resident core is joined by a code
  overlay per level that holds nearly the whole game program, the hero's included, so his
  code is read from savestates taken in a level (`docs/re-setup.md`).
- **Hero movement:** UYA and DL tune the hero in per-second units at 60 Hz, and each PAL
  release is a conversion that keeps real-time behaviour (RE.8). The two games share
  their movement code and differ in a handful of values and rules (`hero-controller.md`,
  "Differences between the games").
- **Prior art** (`prior-art.md`): Horizon's `libuya` and `libdl` (MIT) for the hero's
  structs, constants and names; Wrench (GPL) for the formats; OpenGOAL (ISC), whose
  989snd sound effects `rac_audio` ports; `ggrs` (MIT/Apache) for netcode; ReRAC (ISC)
  for the moby mesh.

## 4. Architecture

### 4.1 Crates

All crates live in `crates/`. Libraries are prefixed `rac_*`; the binary is `openrac`.

```
The hero kit: ISO-free, reusable in any Bevy app; never depends on the disc crates
  rac_sim      fixed 60 Hz clock, system order, render interpolation, seeded RNG,
               rollback snapshots
  rac_input    actions, bindings, remapping, per-tick input snapshots (G3)
  rac_hero     kinematic controller, per-game movement profiles, camera, melee
  rac_rig      the hero's skeleton, the models it wears, procedural poses and clips
  rac_combat   health and damage, weapons, projectiles, pickups, quick-select, vendor
  rac_net      direct-IP sessions over ggrs with rollback, desync checks (G4)
  rac_demo     greybox demo level, demo enemies, HUD, the guns' effects and sounds

Disc imports: read the user's disc at start-up                                (no Bevy)
  rac_iso      ISO9660, disc identification, sector reads
  rac_formats  WAD codec, packed ELFs, level cores, mobys, textures, sound banks,
               gameplay files
  rac_audio    989snd sound effects, ported from OpenGOAL
  rac_tool     the CLI `openrac-tool`, for reverse engineering and checks

App
  openrac      the demo: menus, settings, co-op sessions, and what it reads from a disc,
               handed to the kit as kit types (§5, 2026-10-13)
```

Reverse-engineering tools that never ship live in `tools/` (`pine-probe`, `ghidra/`).

### 4.2 Co-op and networking

- **Topology:** two peers and no servers of our own (D8). One hosts on a UDP port and the
  other joins by `IP:port` or join code. The host's router is asked to forward the port
  (PCP, NAT-PMP or UPnP IGD); where it won't, the players swap join codes holding each
  peer's public addresses, learned from public STUN servers, and punch through their NATs.
- **Model:** like GGPO, both peers simulate everything deterministically and only inputs
  cross the network, with rollback through `ggrs` (§5, 2026-11-04). How it works, and the
  rules simulation code follows for it: `docs/netcode.md`.
- **Safety nets:** a handshake checks the build and the rules (movement profile, guns, gun
  level); per-tick state checksums detect desyncs.
- **Mechanics:** the demo's co-op is two heroes in one world. DL's own co-op rules (shared
  bolts and XP, respawn by the partner, the tether, the team moves) are read (RE.10) but
  not in the kit.

## 5. Decisions

### Open, needing the owner

- **Effects' colours** (P1.9): the kit adds them in linear light, the PS2 in gamma, so a
  layer at alpha a is about 1/a^1.2 too bright over black (6.7× for the Hydra's glows) and
  too pale over a bright floor. Matching it means adding effects in gamma space
  (`effects.md`, "Importing them", "Colours").
- **The Tempest's bolts** (P1.9): at V5 the game's bolt code misses the Tempest's class
  (0x1B4F), so its bolts strike with the Shield Charger's damage (`weapons.md`, "At V5 the
  class test misses"). The kit keeps half the gun's; whether to copy the quirk is open.

### Made

| Date | Decision |
|---|---|
| 2026-10-11 | **D5:** The repository is public. No contributions to other repositories are planned. |
| 2026-10-11 | Rust pinned to **1.99.0** in `rust-toolchain.toml`; Bevy **0.20**; a virtual workspace in `crates/*` and `tools/*`. |
| 2026-10-11 | Pipelines compile **synchronously** (`RenderPlugin::synchronous_pipeline_compilation`). Bevy's parallel compilation crashed 8 runs of 8 on NVIDIA driver 595.91.07 / RTX 4070 SUPER; synchronous gave 18 clean runs of 18. Revisit under P1.15. |
| 2026-10-11 | Savestates and traces come from **private headless PCSX2 instances** (`boot.sh`, `replay.sh`, `warp.sh`) with copies of the owner's memory cards and settings, never from the owner's live instance. Their memory may be written (`pine-probe poke`, the level-change request) to grant an item or reach a level; the catalogue in `docs/re-setup.md` records it. |
| 2026-10-12 | The kit's movement is in **per-second units with the NTSC-U values, simulated at 60 Hz** (RE.8: NTSC-U is the native tuning, PAL a conversion). |
| 2026-10-12 | The kit crate **`rac_sim`** holds the 60 Hz clock, the `SimSet` order, `SimTick`, interpolated `SimTransform` and `SimRng` (PCG32). The workspace turns Bevy's default features off so kit crates ask only for what they use; `openrac` turns them back on. |
| 2026-10-12 | Settings are **TOML** (`settings.toml` in the platform config directory). `PlayerInput` is a fixed encoding (the move stick in PS2 counts, the look in binary-angle units, held and pressed bits), 12 bytes since the look stick joined it (2026-10-17); input recordings are `.orin` files of them. |
| 2026-10-12 | Hero collision uses **`parry3d` 0.31** behind the kit's own `CollisionWorld`. The hero's origin is at his feet; his capsule is libdl's (radius 0.45, sphere centres 0.7/0.5 and 1.05), floating over a ground probe. |
| 2026-10-12 | `rac_input` maps pad sticks from **circle to square**, and keys send full counts on both axes: the games expect a PS2 stick's square range. |
| 2026-10-12 | Hero surfaces carry a **`Material`** (solid, wall-jump, no-grab, hazard, deadly) after the games' collision materials. Health is a `Health` component beside the controller, which only reports hazard contact and whether a hit is fatal. |
| 2026-10-12 | Rules the games time by **animation frame** assume one animation frame per tick until a trace says otherwise. Presses are **buffered** as in the games: each counts once, within its move's window. |
| 2026-10-12 | **Moves the games grant as items** (charge boots, heli-pack and thruster-pack moves, glide, grind boots, gadgets) are per-hero ability flags, which each game sets from its own items and saves. The demo turns every flag on. |
| 2026-10-13 | **The open model** is a blocky, flat-shaded gold Ratchet with Clank and the wrench, made in code with UYA's hero as the visual reference: an acceptable placeholder for the public repository. It lives in the kit crate **`rac_rig`**, so `rac_hero` stays free of rendering, and follows UYA's joint layout so animations from the user's disc can drive it. |
| 2026-10-13 | **Game data reaches the kit only at run time, through kit types.** `openrac` reads the user's disc at start-up and hands `rac_rig` clips and models on the rig's joints. The kit never reads a disc, and nothing read is stored. The disc folder is `source/` by default (`--discs`). |
| 2026-10-13 | **The hero's model is selectable:** the open model or one imported from the user's discs. Every model wears the rig's skeleton. An import is offered only when its bones line up: each rig joint has a source joint descending from the rig parent's, and the other source joints fold into their nearest such ancestor. Clips carry offsets from the source's rest pose, so each model keeps its proportions. Joints whose frame matters follow UYA's convention (`model::wrench_points`). Chosen in the menus (saved in `settings.toml`) or with `--model`. |
| 2026-10-13 | **parry3d's `enhanced-determinism` stays off.** It turns on glam's `scalar-math`, which drops `Vec4`'s 16-byte alignment that Bevy's GPU mesh buffers rely on (the app quit on a wgpu validation error). `rac_hero` depends on `glamx` for its `libm` feature, so parry's scalar maths still avoid each platform's C library; `openrac` asserts the alignment at compile time. Determinism relies on building x86-64 without FMA. |
| 2026-10-15 | **Each game is a movement profile; the controller is shared.** Where the games differ in numbers, the profile holds them; where a rule differs, the profile gets a switch (`strafe.flips`), not a second controller. A value not yet read in DL keeps UYA's, marked so, and a test lists the fields that differ from UYA's. `uya` is the default; the app chooses with `--moves` or the menus. |
| 2026-10-15 | **D7:** Each game's hero moves with that game's own values (jump heights, speeds, timings), read from its code into its own profile. Moves granted by items stay with the games that have them (2026-10-12). |
| 2026-10-15 | **D8:** Connectivity goes as far as it can without servers of our own: direct IP; automatic port mapping (UPnP IGD via `igd-next`, NAT-PMP, PCP); IPv6 where both peers have it; and UDP hole punching, with each peer's public address (learned from public STUN servers) carried in join codes the two players swap themselves. No master, rendezvous or relay servers. Two peers both behind symmetric NATs may still need a forwarded port. |
| 2026-10-15 | **The rig's skeleton is UYA's hero's whole skeleton** (111 joints in his order, then Clank's and the wrench's), so clips drive the face and fingers joint for joint. Poses carry a scale per joint, which its children take on as in the games; a model bound to joints turned at rest says so (`ModelData::turns`). A rig joint a source skeleton lacks may sit between two it has. Which clip plays, and how fast, follows the game's own rules (`rac_rig/src/play.rs`); a game hands the rig its rules for fidgets with its clips. |
| 2026-10-15 | **The repository is GPL-3.0-or-later, replacing MIT**, the hero kit included, so projects reusing the kit must be GPL-compatible. Code under licences GPLv3 can absorb (permissive, GPL-3.0, GPL-2.0-or-later, LGPL) may be reused with its notices kept and listed in `prior-art.md`; GPL-2.0-only and unlicensed code stay reference only, and decompiled game code is never committed. GPLv2 was ruled out because several dependencies (parry3d, winit, cpal) are Apache-2.0 only. Copies of the MIT commits that were already pushed stay MIT. |
| 2026-10-15 | **Lock-on is gamepad only:** with a mouse the player aims. `rac_input` ignores keyboard and mouse bindings for `Action::LockOn`; its default is UYA's, both triggers together. |
| 2026-10-17 | **The look stick reaches the simulation as stick counts** (`PlayerInput::look_stick`; `.orin` version 2, version 1 files still load), so each camera applies its game's own response: the follow camera's linear turn and springing tilt, the free look's squared (UYA) or linear (DL) speeds and quick turn. The mouse stays a direct turn in every camera, and the follow camera keeps the tilt the mouse sets. The camera setting (third person, lock-strafe, first person) is a player option on `FollowCamera` whose default is the movement profile's. |
| 2026-10-20 | **What the games carry on the hero as mobys of their own are attachments** (UYA's Clank and heli-pack): a model part on a skeleton of its own, riding a rig joint with its place and turn but not its scale, as the games normalise the joint before placing a moby. Their clips hold each joint's whole transform, and the game hands the rig its rules for choosing them (`BackAnimations`), as for fidgets. The open model keeps its own Clank on the rig's Clank joints. |
| 2026-10-22 | **The free look's aim assist is the stick's**, as lock-on is the gamepad's (2026-10-15): once the mouse turns the view the assist stops until the stick turns it again. **The quick-select ring plays as UYA's with QUICK SELECT PAUSE off**: it opens over the running world and holds the hero's input after 15 ticks, so co-op stays in step. |
| 2026-10-23 | **The guns' sounds and effects go by the game's numbers, with placeholders made in code.** The kit cues sounds from `GunEvent`s and `Arsenal::sustain` and draws effects from recipes naming the game's effect textures and particle types; its placeholders are synthesized and painted at start-up, so no audio or art ships. With the user's disc the app claims the cues it plays (`SoundClaims`) and puts the disc's textures and shot models in place of the placeholders. **Sound is mixed by the game, not by Bevy's spatial audio** (which fades with the square of each ear's distance): the kit and the app each render a stereo stream (`rac_demo::sfx::SoundStream`) with UYA's falloff from the camera and pan round it; the disc's sounds play through `rac_audio`. |
| 2026-10-31 | **Help messages are named by the kit and worded by the app**: the kit asks for one by a key (`rac_combat::pickups::Help`) when the game would, once a world, and the HUD shows it on the game's timing. The demo words them itself; the game's own texts may take their place from the user's disc later, as its sounds and textures do. |
| 2026-10-31 | **A gun's muzzle comes from numbers, not from the rig's pose.** The game follows the posed hand, but the kit's rig runs per frame (`Update`), differs with or without a disc, and isn't part of the deterministic simulation rollback needs, so the simulation doesn't read it. The weapons file carries where the hand is by what he is doing (`Hands`, measured from the game) and each gun's point, within about 0.1 u of the game's standing and 0.3 u walking (`weapons.md`, "The muzzle"). |
| 2026-10-31 | **DL's guns are chosen on their own, by `--guns dl`, while only some are in.** `--moves dl` keeps UYA's twenty, so a movement test doesn't lose them; once all ten are in and have looks, the guns should follow the game the hero moves like, in the menus too. |
| 2026-11-01 | **The demo keeps Bevy's default tonemapper.** The PS2 has no tone curve and clamps each channel, while Bevy's default pales saturated colours (V5's violet ring shows periwinkle). Comparisons with the game use `--plain-colours`: `ReinhardLuminance`, which keeps the violet and the floor, and a sandy floor. `None` is the game's colours but blows out the demo's floor, as the kit's lights aren't calibrated for it (`effects.md`, "The Quantum Whip's cutting wave"). |
| 2026-11-03 | **The effects' lights are drawn as the game's, not with Bevy's lights.** Each is a sphere that multiplies what the depth prepass shows within its radius by 1 + colour × (1 − distance / radius), as the game's code (0x4A3D80) and footage show, so the demo's 3D cameras get a depth prepass. Bevy's point lights fall off as 1/d² and lit only a spot by the gun (`effects.md`, "How the lights fall off"). |
| 2026-11-03 | **Rebinding swaps.** A binding put in a slot is taken from any other control read in the same place (play or menus; Space can jump and choose), which gets the slot's old binding instead, or loses it if the slot was empty. Pause and the menu actions keep a binding on each kind of device, and each movement key its key, so the player can't lock themselves out; such a change is refused. Esc always calls off listening, so it can't be bound once taken off Pause and Back (Reset gives it back). |
| 2026-11-03 | **The display settings:** one size for the HUD and the menus (Bevy's `UiScale`; the sights keep to the view, as the game sizes them by the screen); a graphics API chosen in the menus is used from the next start only if the machine has a GPU for it (a software adapter doesn't count), or else wgpu chooses, so a wrong choice can't stop the game starting; key names are learned from what the keys type and saved. |
| 2026-11-03 | **Release packages are built on GitHub from a `v*` tag into a draft release**, which the owner checks and publishes (`release.yml`, P7.1). Windows links the C runtime in and, in release builds, opens no console window; Linux builds on the oldest supported Ubuntu runner, for the oldest glibc. **CI caches with `Swatinem/rust-cache`, not `cargo-chef`**, which only helps Docker builds. |
| 2026-11-04 | **Rollback is the kit's own, over `ggrs` without `bevy_ggrs`** (which lags Bevy). A session swaps Bevy's `FixedMainScheduleOrder` for a driver that runs the usual tick schedules when `ggrs` asks, so no system moves. Snapshots clone the components and resources each kit plugin registers, for entities with a `SimId`, numbered in spawn order so it is the same on every peer; simulation code sorts by `SimId`, never `Entity`. The simulation removes entities with `retire()`, which in a session disables them so a rollback brings them back under the same `Entity`. Simulation messages are read with `SimMessageReader`, whose unread messages are part of the snapshot. The tick runs on the single-threaded executor, so systems with no order between them run in the same order on every peer. Presentation on the tick (effects, shot models) skips resimulated ticks and stays out of snapshots (`docs/netcode.md`). |
| 2026-11-04 | **Co-op in the demo:** the host is player 1, the guest player 2; they must run the same build with the same movement profile, guns and gun level (a digest in the handshake), and each keeps their own camera setting, sent in the handshake. During a session the pause menu doesn't stop the game and sends neutral input, and Moves and Camera can't change. A peer that leaves is dropped and the other plays on alone. UDP port 26450 by default. |

## 6. Roadmap

The reverse-engineering track (RE) runs alongside the kit's work. Every task follows
CLAUDE.md's definition of done.

### Phase 0: Foundations

- [x] **P0.1–P0.13** The toolchain, workspace and app (`--exit-after-frames`, the `dev`
  feature); the ignores for `source/` and `cache/`; the first look at the discs; prior
  art; the NVIDIA workaround (§5); CI on Linux and Windows (P0.7); the discs'
  hashes against redump.org; `CONTRIBUTING.md`; the data-hygiene guard (P0.12:
  `tools/check-data-hygiene.sh`, its CI job and `.githooks/pre-commit`); and the relicence
  to GPL-3.0-or-later (P0.13).
  - Uncached, a CI run takes 21 minutes on Linux and 36 on Windows, nearly all of it
    compiling the test binaries. Optional, not done: a Linux smoke run under `xvfb-run`
    with lavapipe (`openrac --exit-after-frames 60`).

### Track RE: Reverse engineering

**Order:** UYA NTSC-U and DL NTSC-U, then their PAL builds where the app reads them. Raw
dumps, savestates and traces stay out of git; only facts in our own words are committed
(names, addresses, layouts, derived constants).

- [x] **RE.1** Tools and setup (`docs/re-setup.md`): PCSX2 2.8 with PINE, Ghidra 12.1 with
  ghidra-emotionengine-reloaded, and `tools/ghidra/` to import and analyse a savestate,
  decompile chosen functions into `cache/`, and apply `symbols.toml`.
- [~] **RE.2** Machine-readable knowledge base (`re/README.md`):
  - Done: `re/<serial>/hero.toml` (the hero struct, verified fields and a frame counter,
    read by `pine-probe`) and `symbols.toml` (each symbol with a confidence and source,
    checked by `pine-probe`'s tests) for UYA NTSC-U, R&C3 PAL, DL NTSC-U and Gladiator
    PAL. Seeded from libdl (MIT, attributed): DL NTSC-U's resident library functions and
    globals (100 symbols), and the library functions found by matching code in the other
    three (37, 37 and 53; method in `engine-notes.md`, "Code layout and overlays").
  - libuya's addresses are for UYA's multiplayer program, which is linked differently
    from single player, so none could be copied.
  - To do: libuya's addresses, from a savestate in a local multiplayer map; a
    code-matching tool in `tools/` in place of the seeds' throwaway script, to map the PAL
    builds' level overlays and carry libuya's names into single player.
- [~] **RE.3** **Hero controller** (`hero-controller.md`; addresses in
  `re/<serial>/symbols.toml`). Feeds P1.5.
  - Read in code and checked against traces, in both games where both have it: the stick
    at part tilt, walking and the sharp-turn skid, every jump type with its entry rules,
    air control, the double jump, glide, the heli and long jumps, flips, the crouch and its
    turn, falling and the 0.43 u step limit, the wall jump, ledges and DL's shimmy, the
    slope limit, getting hit, dying, falling out of the level and the respawn, melee,
    swimming, zip lines, strafing, DL's charge boots, the animation each state plays, the
    moves that need items, and DL's differences from UYA ("Deadlocked's moves",
    "Deadlocked's ledges"). DL's smaller items are read too, so `dl.ron` has nothing left
    "not read in DL".
  - Read in code only, not traced: grinding and the grind's jumps (no UYA level can be
    grinded, and a rail written into Veldin crashes PCSX2), lock-on (it needs a gun none of
    our savestates holds), wading, moving platforms, the walk's start and stop
    thresholds, changing strafe sides, and DL's hits, swimming, strafing and grinding
    (`hero-controller.md` has each).
  - DL's trace rows are read part way through the hero's update: the animation frame is a
    tick ahead of the state and motion, and the timer one behind what the transitions
    see; UYA's rows come after the whole update (`re-setup.md`, "Rows read mid-update";
    `hero-controller.md`, "Reading the traces").
  - Open: where DL's chained rails put him on the new rail (they keep the segment index;
    the kit starts at the new rail's start; "Deadlocked's grinding"); the grind's own step
    cap, which only DL's 28 u/s after a descent reaches.
  - Next step, at low priority: trace lock-on on UYA NTSC with a gun granted as
    `uyantsc-veldin-fire.sh` grants one, then the others as savestates with a rail,
    shallow water or a moving platform turn up.
- [x] **RE.4** **Camera** (`camera.md`). Feeds P1.7.
  - UYA NTSC: the follow camera with its height modes, obstacles and collision; lock-on's
    steering; the camera manager, level cameras, blends, camera volumes, the fixed-point,
    path and 0x11 cameras; the look, lock-strafe, first person and the strafing a gun
    forces; the right stick at part tilt. All read in code, and all but lock-on, the orbit
    blend and 0x11 checked against traces (`camera.md`, "Traces").
  - DL NTSC: the follow camera (UYA's, 6 u back, no push-back), lock-strafe and first
    person, read in code and traced, the stick at part tilt included.
  - Open, at low priority (`camera.md`, "Next steps" 6): a 1° unexplained in the follow
    camera's tilt, the vertical invert's default, the first-person gun placement, the
    camera-only collision groups, DL's aim assist and special follow branches, and the
    untraced vertical modes.
  - For captures: the camera blocks move with each level's build, so find them per level;
    captures slow the game and some samples come out torn, which simulations must skip;
    a take that writes the camera setting must also clear the free look's flag bit 0, as
    `hero_camera_set` does.
- [~] **RE.5** **`tools/pine-probe`**: per-frame capture driven by a layout, savestate
  and recording readers, CSV export, `replay.sh` (a recording on Xvfb), `boot.sh` (a
  disc with the keyboard as controller 1, driven by xdotool, its sticks part-tilted with
  `AXIS_SCALE`), `warp.sh` (any level), `peek`, `poke`, `load-state` and `save-state`.
  Extend it as moves need.
- [~] **RE.6** **Feel-matching harness:** `rac_hero/tests/feel.rs`, `moves.rs`,
  `strafe.rs` and `deadlocked.rs` check the controller against figures from our traces.
  - To do: replay whole traces through the kit and compare them frame by frame (apex
    height, time to apex, run speed, acceleration, turn times, air control), with
    tolerances as tests.
- [~] **RE.7** **Weapons and combat** (`weapons.md`; addresses in
  `re/SCUS-97353/symbols.toml`). Feeds P1.8–P1.10.
  - Done for UYA NTSC-U, in code: the weapon table (0xF0-byte rows per item and level)
    with every gun's numbers, owning, ammo, levelling, XP and the Mega levels; the mods;
    switching and firing; aim assist; the damage queue; all twenty guns' firing and
    shots, their V5 variants included; the hero's side of firing and how he holds each
    gun; ammo pickups, crates, the vendor, the bolt bank, the quick-select ring and the
    HUD; the guns' aim indicators; and the effects engine the guns draw with
    (`effects.md`). The hero's side of firing is traced, its turn spring exact to 10⁻⁸
    ("Checked in traces").
  - The weapon table is static data in each level's overlay and moves per level, so its
    numbers come from savestate images.
  - Earlier readers mistook 0x4C46A8 for a sight test (it asks the table of who was sent
    at what) and read 0x40CCA0's −π/8 about his left axis as 90° to a side. Check other
    readings of those two functions when a gun looks wrong.
  - Open in UYA: what the damage-record flags mean, the rest of the agent's update, the
    Plasma Storm's arc, the Decimator's and Lava Gun's V5 (`weapons.md`, "Next steps" 2
    and 4); placed pickups' rounds, vendor kind 4, multiplayer's ring; projectiles in
    traces (spawned mobys a fixed layout can't follow); whether another build fills the
    acid entry the Nitro Launcher's blast (item 0x77) lacks.
  - DL, begun 2026-10-31 (`weapons.md`, "Deadlocked's guns"): the gadget table (0xB0-byte
    `GadgetDef`), its levels and the mods' numbers, and four guns' code. Not read: the
    other six guns.
  - Next step: DL's guns, in the order of `weapons.md`'s next steps 5; at lower priority,
    the rest of the hero's side of firing in traces (next steps 1).
- [x] **RE.8** **Timing across regions** (`hero-controller.md`, "Frames and regions"):
  answered for UYA and DL from both regions' code and checked with NTSC traces. Constants
  are per second, tuned at 60 Hz; R&C3 PAL folds them with 1/50, Gladiator scales them at
  run time, and frame counts are cut to about 5/6.
- [x] **RE.10** **Gladiator's co-op mechanics** (`coop.md`; names in
  `re/SCUS-97465/symbols.toml` and `re/SCES-53285/symbols.toml`): read in DL NTSC-U's code
  and checked in play on DreadZone Station's and Catacrom Four's co-op builds; Gladiator
  PAL is the same code with 50 Hz frame counts. Who player 2 is; what the players share
  (bolts, XP, maximum health, one gadget box); death and the 6 s respawn by the partner,
  with no revive and a restart when both are dead; ammo pickups at ×2.4; kills' XP; the
  split screen; the team moves (L3: ×4 damage; R3: a shield only for show in co-op); and
  the tether (class 0x247B: over 40 u apart for 10 s kills both). Only minor values are
  unread (`coop.md`, "Not read yet / next steps"), none needed unless the kit takes up
  DL's co-op rules (section 4.2).
  - A co-op savestate won't leave its pause menu while port 2 is empty. Each DL level's
    build moves globals as well as code; `tools/ghidra/match-function.py` carries
    addresses across builds.

### Phase 1: The hero kit and its demo (G1, G3, G4)

Crates: `rac_sim`, `rac_input`, `rac_hero`, `rac_rig`, `rac_combat`, `rac_net` and
`rac_demo`, wired up in `openrac`. Any non-code asset must be CC0, recorded in
`crates/openrac/assets/CREDITS.md`. Check that an ecosystem crate supports Bevy 0.20
before adding it (CLAUDE.md).

- [x] **P1.1** App flow (`openrac`: `flow.rs`, `menu.rs`, `settings.rs`): `AppState`
  Boot → MainMenu → InGame with a `Pause` sub-state, the game clock running only in game;
  a focus-list menu; `settings.toml` (missing files are created, broken ones kept and
  ignored); the cache directory is `./cache` in dev builds.
- [x] **P1.2** Simulation clock and determinism (`rac_sim`): `FixedUpdate` at 60 Hz with
  `SimSet` Input → World → Hero → Particles → Camera, interpolated `SimTransform`,
  `SimRng`, Bevy's `libm` feature, and input record and replay. Cross-OS hash checks are
  P1.19.
- [~] **P1.3** Input layer (`rac_input`, G3):
  - Done: actions with UYA's pad layout and KB+M defaults (WASD, mouse look and aim, right
    mouse to look round, Left Shift to strafe), dead zones, sensitivity, invert and
    swapped sticks saved in the settings file, the 12-byte `PlayerInput` (§5), `ActionState`
    for menus, the `.orin` codec, two-button chords and gamepad-only lock-on (§5,
    2026-10-15). Rebinding (`rac_input/src/rebind.rs`; §5, 2026-11-03): two slots per
    device, swaps, clearing, reset, and `BindingCapture` for the next key, button, wheel
    notch or chord. Keys named as the layout has them (`openrac/src/menu/names.rs`). Saved
    settings that predate an action get its defaults.
  - Not done: checking the full stick curve, since Bevy's own axis processing runs before
    ours; catching a real gamepad's press (only tested on synthetic pads); a key's name
    before its first press on a new layout (the US name).
- [x] **P1.4** Greybox level (`rac_demo`), as data tagged by purpose (`Surface`): ramps,
  stairs, ledge blocks, a wall-jump shaft to a high platform, a gap course, a moving
  platform, a grind rail, a rail circuit of two lanes 3 u apart to switch between, a zip
  line, a pool, a melee yard, a hazard pad, checkpoints and the enemies' arena (P1.11).
  - The circuit's lanes are closed loops. At a lap's seam UYA's hero drops off the rail's
    end and gets back on 8 ticks later, as UYA has no rule for a rail running on into
    another; DL's rides on (`grind.chain`).
- [~] **P1.5** Kinematic hero controller (`rac_hero`), the UYA and DL move sets from RE.3.
  - **Design:** a pure step function over (state, input, camera yaw, profile, collision)
    (`controller::step`), so tests drive it without an app. Collision goes through
    `CollisionWorld` (sweeps, rays, a `Material` per body, rails, water) with a
    brute-force `parry3d` backend. The capsule floats 0.25 u over a ground probe, so steps
    up to 0.25 u need no code.
  - **Profiles:** each game's numbers are a RON profile, each value's source in a comment:
    `assets/uya.ron` (the default) and `assets/dl.ron` (§5, 2026-10-15).
  - **Done:** idle, walk (turn springs, the stop, the sharp-turn skid), wading, crouch and
    its turn; standing, run, long and heli jumps, flips (strafe, landing and crouch), the
    double jump, glide, falls, air control and landings; moving platforms and the carry
    off them; the wall jump; ledges; hits, hazards, death, falling out of the level,
    checkpoints and the respawn's fade; DL's charge boots; the ability flags; grinding,
    with the grind jump and the switch jump between rails; swimming; zip lines; strafing
    and lock-on; melee (P1.8). DL's own flips, ledges, hits, stick, strafe sides and
    grinding (facing along the rail, its hop round, chained rails), and the moves it
    lacks, are switches in `dl.ron`. Tests: `rac_hero/tests/` and
    `rac_demo/tests/greybox_moves.rs`.
  - **To do:** auto-jumping rails.
  - **Placeholders:** the end of the charge's bounce; on the hazard surface the rig plays
    the get-hit animation [simplified]. Not modelled: a platform turning the hero, and the
    moby flags.
  - **Known mismatch:** a long jump from a 7-tick crouch leaves at 8.2 u/s in the kit and
    7.8 u/s in the trace: the game's first crouch frame brakes less, and its jump frame
    brakes once more. A crouch flip from a run carries one crouch tick's braking more
    speed than the game's for the same reason.
- [x] **P1.7** The hero's camera (`rac_hero/src/camera/`, the profile's `camera` section),
  from RE.4 (`camera.md`, "In the kit"):
  - **The follow camera** (third person): UYA's drag camera with its height modes,
    springs, push-back, the stick's turn and tilt, lock-on, strafing, the aim's 90-tick
    blend, the clearance point, the arm sweep, the camera-end sphere, the pull-in and the
    resets. DL's is the same 6 u back, so it has no push-back.
  - **The free-look camera** for the lock-strafe and first-person settings and the
    third-person look: UYA's squared and DL's linear stick, the quick turn, the limits, the
    eye's spring and jump hold, and the free look's aim assist (§5, 2026-10-22). The
    setting is `FollowCamera::setting` (UYA third person, DL lock-strafe by default),
    chosen in the menus or with `--camera`.
  - **Blends** between the cameras, and **`CameraVolume`**.
  - **Simplified or left out:** the kit also sweeps the camera from the pivot so it never
    ends behind a wall; the platform carry, the swim and critter rules, volume steering
    and fields of view, and the modes for states the kit lacks (jets, rocket jumps, jump
    pads, the bounce's mode 11) aren't modelled; the camera setting isn't in input
    recordings.
  - Not done: a replay of RE.4's traces through the kit camera, to check it end to end
    (`camera.md`, "Next steps").
- [x] **P1.8** Melee (`rac_hero/src/controller/melee.rs`, the profile's `melee` section,
  and `rac_combat`): the wrench combo, jump attack, throw, grind attack and the clank off
  walls, for UYA and DL, with hurtboxes, hit points, one hit per attack, knockback and
  flash; with a gun in hand he waits for the wrench (`wrench_draw_ticks`). The demo has a
  melee yard.
  - The disc's attack animations played the frame's creep through each blend as clip
    frames, then stepped back a frame, so chained swings didn't line up. The rig now
    cross-fades from the pose it showed to the start frame, as `hero_anim_set` does
    (`hero-controller.md`, "Animation frames"); not yet compared with footage. DL's
    attacks don't creep, and whether DL cross-fades the pose isn't read, so they still
    change pose by the rig's own ease.
  - Not modelled: DL's Scorpion Flail (state 145, read: `hero-controller.md`, "The
    Scorpion Flail"; with DL's guns), UYA's states 77 and 114, and the giant-Clank mode.
  - The grind attack's wrench line is computed from the disc's animations, as UYA's grind
    can't be traced (RE.3).
  - Open in DL: Triangle and R2 keeping an attack from starting; hero +0x4AC, which keeps
    the combo from starting; the throw button's camera rule (the kit throws with crouch);
    one trace of the double-jump attack starting later than the code's frame-16 rule
    gives (`hero-controller.md`, "Deadlocked's melee"). Next: read the double jump's own
    transitions for what holds a press back, then +0x4AC's writers.
- [~] **P1.9** Weapons (`rac_combat`), following RE.7. UYA's rules are in `weapons.md`;
  what the kit has of them, and its placeholders, in its "In the kit".
  - **Done:** all twenty UYA guns in `assets/uya_weapons.ron` with every level's numbers
    (matching the game at all eight levels, checked by script against EE memory), the
    mods and a fire mode each; the `Arsenal` (owning, levels, switching), the guns' aim
    assist and the view's aim point, the damage queue, shock, acid and XP from kills;
    `GunEvent`s naming each gun's effects for sounds and visuals; in `rac_hero`,
    `GunHand`, the raised arm and what a gun changes in his moves; the quick-select ring
    (§5, 2026-10-22). With a UYA disc he holds each gun's own model by level, in one hand
    or both as UYA's hold layers have it, and the guns play their own sequences. Their
    effects are P3.18's.
  - **Against the game at V5** (`effects.md`, "The twenty guns against the game at V5 and
    V3"): fourteen guns close and six partial (the Liquid Nitrogen Gun, Infecto-Bomb,
    Agents of Dread, Ultrashield, Rynocirator and Megaturret Glove); the table says what
    is left for each. Also open:
    - The agents' jets are dimmer than the game's; from V5 their arms' pitch while
      walking or running isn't modelled, nor the gun's at V2–V4 (`effects.md`, "Its
      barrels' arms at V5").
    - Not drawn: the Eruptor's dome of 23, masked discs of 71 and 72 and quads of 54; the
      Shock Cannon's charging sprites and glare; the N90's bullet model (0x1881), left out
      until its blend is read (`shot_models.rs`). Not seen on screen: the Ultrashield's
      hit rings. Not compared with the game's footage: the rift's close. Not followed: the
      Lava Gun's hold sway and the waist's pitch.
    - In first person the other guns bend from his facing, where the game's fire
      direction is the camera's (0x50F410); only points within 4 u and pitches past 80°
      differ.
    - The Miniturret's tracers lack the game's bright heads: they need a tapering,
      fading `Streak`.
    - The whip: the swing's turn of links 1, 2 and 5 (read, but it tangles the kit's
      rope; trace the links' yaw, +0xF8, through a combo in PCSX2), its scorches and its
      light.
    - The V4 duck on screen and its bangles; the rift's spirals of 71, orbiting sparks
      and 0x13 spray; the blob shadow (0x444070); the Nitro guns' and Infecto-Bomb's
      landing marker and the marker's conditions; a duck's first random speed; an
      appearing agent's opacity.
    - Not checked: V1, V2, V4 and V6–V8 on screen, and the sounds by ear. The Shield
      Charger hangs the game on Veldin at V3 and V5, so its V5 is judged from one frozen
      frame.
  - The game's footage shows its 16:9 view at 4:3, everything 0.75 as wide as it is
    (UYA's savestates have the 16:9 option on): widths measured in it are × 4/3
    (`re-setup.md`, "Gotchas").
  - **DL's guns** (`weapons.md`, "Deadlocked's guns"; §5, 2026-10-31): the Dual Vipers,
    Magma Cannon (the `Cone` fire mode), The Arbiter and Fusion Rifle in `Armory::dl`.
    Not done: the other six, the mods in the kit, DL's sounds, looks and models, and its
    rules and hands (still UYA's).
  - The agents showed twice the game's size: from V5 their code scales the barrels'
    arms (joints 3, 8) to 0.45 through joint layers, which the kit now applies
    (`ShotSkeleton::scaled`). Found in the hover savestate's layers and checked on its own
    screenshot (`effects.md`, "Its barrels' arms at V5").
  - In progress (2026-11-06): the agents' jets, placed, sized and carried as 0x46AB08 has them.
  - **Next step:** the agents' jets, then their arms' pitch while walking. For DL: the B6's blast at the end of 0x40F3E0's state 4, then
    `weapons.md`'s next steps 5.
- [~] **P1.10** Pickups and economy (`rac_combat::pickups`, `assets/pickups.ron`), on
  UYA's rules (`weapons.md`, "Ammo pickups, crates and the vendor" and "In the kit"):
  - Done: ammo pickups; bolt, ammo, nanotech, explosive and doubler crates, with the
    doubler's x2 on the HUD and its help message (§5, 2026-10-31); the vendor's menu over
    the running world (`VendorMenu`), with gun and Mega prices; the bolt bank and its
    debris cap; UYA's help measure (`rac_hero::HelpMeasure`); their sounds, placeholders
    or the disc's (`openrac/src/gun_sounds.rs`). The demo has each in the yard. Rounds and
    prices are the game's.
  - Not done: the bank's drops scaled by bolts per active minute (its gate 0x1D6714 by
    the killed moby's +0xB0 isn't understood), the challenge multiplier and the XP bank
    (0x142698); every moby slot taken as free in the debris cap [simplified]; expired
    bolts banked [placeholder]; the hero's armor (UYA scales his damage by it, 1.0 to
    0.2); the help measure's areas; Triangle closing a help message, and voiced help. The
    vendor: what UYA's vendor moby needs to open its menu and its last special row (not
    read); the stock growing with the story, the challenge-only five and the 9/10
    discount (read, not built). The pad opening it on stepping in is the kit's own.
  - Next step: move the enemies' bolt and nanotech drops and the hero's `Bolts`
    (`rac_demo/src/drops.rs`) here, or read what UYA's vendor moby needs to open its menu.
- [x] **P1.11** Demo enemies (`rac_demo/src/enemy/`, `assets/enemies.ron`), the demo's
  own, every number a placeholder sized against the hero's: a melee charger and a ranged
  shooter over one pure step (`enemy::brain::step`), with idle, patrol, aggro, attack,
  hurt and death, sight and hearing, leashes and respawns. Health and damage go through
  `rac_combat`; they drop bolts and nanotech (`drops.rs`); infected, they fight each other
  (`FoeHits`). Tests: `rac_demo/tests/enemies.rs`.
  - Enemies run after `SimSet::World` and before `SimSet::Hero`: inside `World`, their
    reads of the combat messages would be unordered against the systems that write them,
    and the order could differ between peers.
  - To do: their sounds (P1.13), health and bolts on the HUD (P1.12), and tuning by play.
- [~] **P1.12** UI and HUD, PC first:
  - Done: the HUD (`rac_demo::hud`, `assets/hud.ron`): health, bolts, the gun panel, the
    level-up notice and the ring per player, on UYA's show-and-fade timings; the guns' aim
    indicators (`hud/sights.rs`): the lock-on reticle, each gun's crosshair, the target
    marks and the Flux scope with its zoom, with the disc's textures or placeholders. The
    menus (`openrac/src/menu/`): Options from the main and pause menus, with the mouse and
    sticks, display (window mode, resolution, vsync, frame-rate cap, field of view, HUD
    and menu size, graphics API; §5, 2026-11-03) and volume, and the controls page, every
    control's bindings on the keyboard and mouse and on the gamepad. Seen running on Xvfb.
  - Not done: L3's show-all (no binding); the scope drawn through the frame's alpha as
    the game does; a music volume (no music yet). What the Flux's zoom drives through the
    four floats it lerps at 0x1DF720 is open (`weapons.md`, "Aim indicators").
  - Not seen: borderless, fullscreen and its video modes (Xvfb has no window manager and
    one mode); the volume heard; the lock-on reticle (gamepad only).
  - `warn!` before `App::new` is lost, as Bevy's log isn't up yet: `settings::
    load_or_create`'s warning (a broken settings file) and the movement profile's
    fallback in `main` still are.
  - Next step: try the window modes, fullscreen resolutions and a gamepad's rebinding on
    the owner's desktop.
- [~] **P1.13** Sound effects and music, cued by events: placeholders made in code, or the
  game's own sounds with a UYA disc.
  - Done: the guns' sounds (`rac_demo::sfx`, cued from `GunEvent`s and
    `Arsenal::sustain`), synthesized from `rac_demo/assets/sounds.ron` on the kit's own
    stereo stream with UYA's falloff and pan (§5, 2026-10-23); with the disc, 18 guns' and
    the shots' own sounds (`openrac/src/gun_sounds.rs`, P3.13); the Spitting Hydra's hold,
    marks and fizzle; the pickups' (P1.10).
  - Not done: nobody has listened yet (off-screen runs use ALSA's null device); the
    wrench's, the hero's and the enemies' sounds; music; shots' sounds cued from their own
    classes, as `rac_demo::sfx` cues by gun (so the agents' guns send none, and a bolt's
    strike and the shield's landing play their gun's impact cue); the sounds not played
    (`effects.md`, "In the kit and the app").
  - Next step: listen in the app with the disc (the arena replays `-agents`, `-rift`,
    `-whip`, `-qwack`), then the bounces' events.
- [ ] **P1.14** Platform checks:
  - Windows: pick the default backend. Today wgpu prefers Vulkan over DX12 whenever both
    exist. Test both, then set the default accordingly. The Options menu's Graphics API
    already chooses (P1.12): the chosen backend if it has a GPU, or else wgpu's.
  - Linux: Vulkan.
  - Gamepad hotplug on both.
  - Performance: at least 60 fps at 1080p on a mid-range GPU.
- [ ] **P1.15** Pipeline compilation:
  - Prewarm the pipelines behind a loading screen, so that synchronous compilation never
    causes stutter during gameplay.
  - After each NVIDIA driver or wgpu update, re-test Bevy's default parallel compilation:
    run `openrac --exit-after-frames 120` ten times. If it passes every time, drop the
    workaround.
- [~] **P1.16** Make the kit ready for mash-ups (G1's test):
  - Done: the controller sees collision only through `rac_hero::CollisionWorld`; kit
    crates enable only the Bevy features they need; configuration comes from RON.
  - To do: make the system that gathers `Collider`s independent of the parry backend; a
    CI check on `cargo tree` that the kit depends on no disc crate; document the public
    API; `examples/hero_sandbox.rs`, the kit in a plain Bevy app.
- [~] **P1.17** The hero's rig and models (`rac_rig`, `openrac/src/hero_model.rs`; §5,
  2026-10-13, 2026-10-15 and 2026-10-20):
  - Done: the rig's skeleton is UYA's hero's 111 joints, face and fingers included, plus
    Clank's and the wrench's, with a scale per joint; models may be bound to joints
    turned at rest (`ModelData::turns`). The open model (gold for player 1, silver for
    player 2) has procedural poses for every hero state, with two-bone IK putting the
    wrench on the controller's swing line and the fist held as UYA's idle holds it
    (`moby-format.md`, "How the idle holds it"). The rig wears any model skinned to its
    skeleton (`RigModels`, `ModelData`, `SkeletonMap`), with attachments on skeletons of
    their own (`AttachmentData`, `BackAnimations`). From the user's discs `openrac`
    imports 15 UYA suits with Clank and the heli-pack on the back, played by UYA's back
    rules (`moby-format.md`, "Clank and the heli-pack on the hero's back"); UYA's 21
    multiplayer skins (`MP_SKINS`); and 17 DL skins with their armors' bangles. With a UYA
    disc the hero plays Ratchet's real animations, chosen and timed as UYA's code does
    (`rac_rig/src/play.rs`; `hero-controller.md`, "Idle fidgets", "Landings", "The walk's
    animation"), and holds the guns by their hold layers (P1.9).
  - Read, not yet worn: DL's wrench (class 0x23FA, `Core::read_dl`,
    `Core::moby_textures`).
  - To do: DL's wrench on the skins in its sequence 1 (DL classes' sequences don't read as
    UYA's do yet: read them first, P3.7); DL's firing poses (its gadget table names each
    weapon's firing sequence: `moby-format.md`, "DL's skins"); Clank's blink and glow
    (UYA 0x4DCC10) and the head look's hold on it, which needs a head look in the rig; the
    Suck Cannon's own sequence 2 while it pulls and 0x441FB8 as it lets go (`weapons.md`,
    "The gun's own animation"): read 0x441FB8, then give it `GunMotion::sustain`.
  - The strafe's twist is one angle eased beside the pose (`pose::Animator::ease`), as
    slerping joint by joint spun the body a full turn; UYA's own two twist springs
    (`hero-controller.md`, "Strafing") aren't used yet.
- [~] **P1.18** Direct-IP co-op in the demo (`rac_net`, section 4.2; §5, 2026-11-04):
  - Done: rollback of the kit's simulation (`rac_sim::rollback`: snapshots, `SimId`,
    `retire`, `SimMessageReader`), every kit crate's state registered; `rac_net`'s `ggrs`
    driver, UDP link and handshake; STUN, join codes and hole punching on IPv4 and IPv6
    (D8); the host's port mapped by PCP, NAT-PMP or UPnP IGD (`--no-port-mapping`
    doesn't); `--host`, `--join`, `--input-delay`, `--sync-test` and a status line
    (`openrac/src/coop.rs`), and the main menu's Co-op page (`menu/coop.rs`); effects,
    sounds and the HUD showing what resimulated ticks write once (`rac_sim::ShownReader`);
    both heroes, player 2 in silver; a peer that leaves is dropped and the game plays on.
  - Verified (`docs/netcode.md`, "How it was checked"): `rac_net`'s tests (the sync test,
    two peers on a slow, lossy memory network, NAT traversal behind cone NATs, PCP and
    NAT-PMP against fake routers); the whole demo headless with two scripted heroes under
    the sync test and as two peers (`rac_demo/tests/rollback.rs`); the app's `--sync-test`
    for 1500 ticks; a host and a guest on one machine, the guest joining by the host's
    code from the command line and from the Co-op pages.
  - Not done (`docs/netcode.md`, "Known gaps"): runs on two machines, over a LAN and the
    internet; a mapping asked of a real router (it changes the router's forwarding, so it
    waits for the owner's run; the owner's router answers neither PCP nor NAT-PMP, so
    there it is UPnP's); an IPv6 pinhole by PCP; renewing mappings past their two hours.
  - Next step: the owner's runs on two machines, over a LAN and then the internet (Co-op,
    Host a game on one; the code it shows sent over, copied, and Join with the copied code
    on the other; if the guest can't reach the host, the guest's code pasted into the host
    with Ctrl+V), checking the host's log says its router forwards the port ("the router
    at … forwards UDP from … (UPnP)") and that leaving gives it back with no warning. Then
    the task is done.
- [ ] **P1.19** Determinism tests:
  - Replaying recorded inputs twice must give identical state hashes.
  - CI runs the same replays on Linux and on Windows and compares the hashes across the two
    operating systems.
- **Done when:**
  - The demo plays for 5–10 minutes with gamepad and KB+M, on Windows and Linux.
  - Remapped bindings persist.
  - Two players can co-op by IP.
  - The kit runs inside the sandbox example.
  - The RE.6 tolerances pass.
  - No game data is used.

### Phase 2: Disc layer

What the demo's imports and the reverse engineering need. Crates: `rac_iso`,
`rac_formats` and `rac_tool`.

- [x] **P2.1** `rac_iso` reads ISO9660 over `Read + Seek` (PVD, directories, file extents)
  with a raw sector API, never loading a whole image.
- [x] **P2.2** Disc identification (`rac_iso::Release`; `disc-layout.md`,
  "Identification"): every one of the owner's eight releases by its boot executable's
  SHA-1; unknown discs get an error saying why, with the SHA-1 of an unknown build of a
  known game. `openrac` finds its discs with it, and `openrac-tool identify` lists them.
- [~] **P2.3** TOC parsers. Done: UYA's (`rac_formats::level::Toc`; `disc-layout.md`,
  "Level data"), and DL's and Gladiator's eight global headers (`Toc::read_dl_disc`).
  - To do, when DL's guns' imports need it: DL's level table (what follows its global
    headers isn't UYA's), with its missions.
- [~] **P2.4** WAD codec. Done: the decompressor, written from UYA's own
  (`rac_formats::wad`), run on every WAD of Veldin's level data.
  - To do: fuzzing (`cargo-fuzz`) and property tests, as user discs can be corrupt.
- [x] **P2.5** Unpacker for the packed executables (`rac_formats::elf`; `openrac-tool
  unpack-elf`): the boot ELF with `$gp` and section names, for Ghidra, checked byte for
  byte against savestates (`disc-layout.md`, "Packed executables"). The boot program's
  `.text` is the title screen's; level code comes from savestates.
- [~] **P2.7** `openrac-tool` CLI. Done: `moby` (a class from an EE memory image, or as a
  `.glb` into `cache/`), `hero`, `rails`, `cameras`, `mobys`, `sounds`, `unpack-elf` and
  `identify`. To do: `toc`.
- [x] **P2.10** The R&C3 PAL disc (SCES-52456) wherever the app reads UYA NTSC-U's: the
  hero's animations, suits and skins, the guns' models and sounds, and the effects.
  `openrac` takes either (`discs::UYA`, UYA NTSC-U first) and DL NTSC-U or Gladiator PAL
  (`discs::DL`). Everything it reads is the same on both R&C3 discs, value for value, so
  nothing converts (`disc-layout.md`, "R&C3 PAL beside UYA"; disc tests in `main.rs` and
  `gun_sounds.rs`).
- **Done when:** the four UYA and DL discs identify, and everything the demo imports reads
  from them.

### Phase 3: What the demo imports from the disc

Crates: `rac_formats` and `rac_audio`, read by `openrac`.

- [~] **P3.2** Textures. Done: the armor WAD's PIF2 images (8-bit, CSM1 palette, alpha
  0x80) → `rac_formats::texture` → Bevy `Image`; the level core's effect, particle and
  class textures, UYA's kept in EE memory in rows and DL's in the GS's order
  (`Core::gs_ordered`; `disc-layout.md`, "Level textures").
  - To do, as imports need them: PSMT4 and RGBA16 CLUTs, GS swizzles and mips.
- [~] **P3.3** The level core header and the class tables. Done for UYA: the level header,
  data part, core header, moby class table and hero sequence table
  (`rac_formats::level`).
  - To do, as imports need them: the core header's other tables, and DL's.
- [~] **P3.7** Mobys (`rac_formats::moby`, `moby-format.md`). Done for UYA, from memory or
  the disc: the class header, skeleton, every keyframe, the complete mesh (vertex cache,
  one to three joints per vertex, normals, texture coordinates and switches) and metal
  packets; the armor and gadget WADs. For DL: classes and the skin WAD
  (`MobyClass::read_dl`, `armor::{SkinTable, Skin}`).
  - To do: bangles, UYA's armor entries 35–55 (whole classes), and DL's sequences.
  - Open: where the game multiplies by the inverse bind (not found in code); whether
    children inherit a joint's scale; classes whose offsets don't fit their binds
    (0x189B, 0x18AD, …); DL armors' joint 44.
- [x] **P3.10** Gameplay data (`rac_formats::gameplay`, `gameplay-format.md`), for
  reverse engineering: UYA's rails, grind and zip line, checked against the game's own
  list on Florana; UYA's cameras and camera volumes, checked against five savestates
  (`openrac-tool cameras`); DL's mobys and pvars (`Gameplay::read_dl`), checked against
  Catacrom Four's co-op savestate (`openrac-tool mobys`).
- [x] **P3.13** Audio: UYA's 989snd sound-effect banks (`SBlk`, the gadget WAD's per gun
  and the level's) read in `rac_formats::sound`, with class sound tables and the core's
  numbering; `rac_audio` ports OpenGOAL's 989snd sound effects (ISC) and plays them;
  `openrac-tool sounds` lists a bank and renders its sounds to WAV under `cache/`. Which
  are played is P1.13's.
- [~] **P3.18** The guns' **effects from the disc** (`effects.md`, "Importing them"):
  - Done: the level core's effect and particle textures and particle lists
    (`Core::fx_textures`, `Core::particle_textures`); the kit's ISO-free effects layer
    `rac_demo::fx` (glows, bursts, beams and bent beams, strips, ribbons, trails, rings,
    domes, shells, decals, models, lights drawn as the game's (§5, 2026-11-03), camera
    shake, screen flashes and warps, refraction), in the game's blends, driven by recipes
    in `rac_demo/assets/effects.ron` keyed by the game's texture numbers, with painted
    placeholders when there is no disc; every gun's calls read with their arguments and
    in recipes as far as the layer goes; the shots' models, skinned and animated by level
    (`openrac/src/shot_models.rs`). How each gun compares with the game is P1.9's review.
  - Open in the layer (`effects.md`, "Importing them" 3 and 4, has the rest): particle
    type 0x5C changes evenly (each caller's delay and first-phase length, its 9th and
    11th arguments, aren't read) and lacks the 0.9 u its spawner moves it toward the
    camera; the Tesla Barrier's `rand(20, 60)` is each slot's wait before its next bolt,
    but the kit's bolts last that long; the bolt's strike, 6 ticks after the Hydra's hit,
    falls in the kit's placeholder 8-tick recovery from a gun's hit
    (`CombatTuning::gun_recovery_ticks`), so in play half the bolt is lost (the game
    leaves it to each enemy's code); the Hydra's arcs aren't cut at walls, and its
    strips' ends sit on an offset from the kit's muzzle, not the model's posed joints;
    effects driven by a gun's sustain (the Suck Cannon's vortex) or by events the kit
    doesn't send (taking something in, ricochets); children particles and meshes other
    than the ring; the V5 shots' classes; the strips' masks (a custom material); the
    Lava Gun's ribbon's scrolling texture; the yaw `whip_link_place` adds in a combo,
    which the kit reads as the hand's travel added to links 1 and 2.
  - Next step: the guns' looks, in the order P1.9's review gives.

### Phase 7: Release

- [~] **P7.1** Windows package: MSVC build, zip.
  - Done: `.github/workflows/release.yml` builds the game in release and zips
    `openrac.exe` with `LICENSE` and `README.md`, the C runtime linked in (`+crt-static`;
    blake3 is the only C code, built through `cc`, which follows it). A `v*` tag makes a
    draft GitHub release of both platforms' packages; a run by hand keeps them as
    artifacts (§5, 2026-11-03). Release builds open no console window and attach to the
    terminal they were started from (`attach_parent_console`), so a crash when started
    from Explorer shows nothing until P7.4's crash logs.
  - Not done: no tag has published packages yet. The packages lack the dependencies'
    licence notices, which MIT and Apache-2.0 require with binaries (for example `cargo
    about`); add them before publishing a release. The backend default waits on P1.14.
  - Tag `v0.1.0`'s publish job failed: `download-artifact` unpacks any zip, so the Windows
    package arrived as loose files. Fixed with `skip-decompress: true`; the tag still
    points at the old workflow.
  - Next step: try both packages, and attach them to the next tag's release.
- [~] **P7.2** Linux package: a tarball, built on `ubuntu-22.04` so the binary needs only
  glibc 2.35 (`release.yml`). Not done: AppImage or Flatpak.
- [ ] **P7.4** Polish:
  - Controller glyphs, accessibility options and crash logs.
  - Networking help: firewall, and a forwarded port for when mapping and hole punching
    fail (D8).
- [ ] **P7.5** Legal review of disclaimers and branding (trademark use in the project
  name).

## 7. Cross-cutting

- **Testing:** unit tests on synthetic data; disc tests read `OPENRAC_SOURCE_DIR`, are
  `#[ignore]`d with a reason and run with `cargo test -- --ignored`; feel-matching tests
  (RE.6); the sync test and rollback tests (P1.18); determinism replays (P1.19); visual
  comparisons with the game, run locally only.
- **CI** (P0.7, P0.12): fmt, clippy with `-D warnings`, tests, Windows and Linux builds,
  the data-hygiene guard, and later the kit's dependency check (P1.16) and cross-OS
  determinism hashes (P1.19). Disc tests never run in CI. Release packages come from
  `release.yml` (P7.1, P7.2).
- **Determinism** is a requirement because of co-op: a fixed 60 Hz simulation, seeded
  RNG, `libm` maths, ordering by `SimId`, x86-64 without FMA, input record and replay,
  and per-tick checksums (`docs/netcode.md`).
- **Performance budgets:** the demo at 60 fps or better at 1080p on a mid-range GPU; co-op
  playable at a 100 ms round trip.
- **Diagnostics:** `tracing` through Bevy's logging (`RUST_LOG`), the `bevy/trace_tracy`
  feature for profiling, and co-op's status line.
- **Documentation:** findings in `docs/research/`; the kit's public API documented for
  mash-ups (P1.16).

## 8. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| The controller doesn't feel like the original | G1 fails at its core | Reading the games' code (RE.3, RE.4); measured traces (RE.5); tolerance tests (RE.6). |
| Determinism breaks between machines or operating systems | Co-op desyncs | `libm` maths, ordering by `SimId`, x86-64 without FMA, the sync test, replays across operating systems in CI (P1.19), per-tick checksums. |
| Legal problems in a public repo (incompatible or unlicensed code, derived data, trademarks) | Takedown or relicensing | The CLAUDE.md rules (GPL-3.0-or-later; reused code keeps its notices; GPL-2.0-only and unlicensed code reference only), the CI data guard (P0.12), reading discs only at run time, and P7.5. |
| Bevy churn (a breaking release roughly every 4 months; ecosystem lag) | Upgrade cost | Pin versions; upgrade in one dedicated change; keep the disc crates free of Bevy; prefer crates that don't depend on Bevy (`ggrs` without `bevy_ggrs`). |
| The kit's API churns and breaks mash-ups | Reuse suffers | A small documented API, the sandbox example (P1.16), and semantic versioning once it stabilises. |
| NVIDIA driver crash when compiling pipelines in parallel | Crashes at startup | Synchronous compilation (done); prewarm pipelines; re-test regularly (P1.15). |
| Continuity across many AI sessions | Lost context | Keep §0 and each task's state current, and write findings into `docs/research/` as they are checked. |

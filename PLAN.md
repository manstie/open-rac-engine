# Open Ratchet & Clank Engine: project plan

The source of truth for **scope, strategy, decisions, status and next steps**.

- [README.md](README.md) summarises the goals (section 1) for visitors; keep it in step.
- Working rules and commands: [CLAUDE.md](CLAUDE.md).
- Findings, each labelled with how it was verified, are in [docs/research/](docs/research/):
  - [disc-layout.md](docs/research/disc-layout.md): the eight discs, their TOCs and level data.
  - [engine-notes.md](docs/research/engine-notes.md): the engine.
  - [prior-art.md](docs/research/prior-art.md): other projects, tools and their licences.
  - [hero-controller.md](docs/research/hero-controller.md): how Ratchet moves (RE.3).
  - [camera.md](docs/research/camera.md): how the camera follows him (RE.4).
  - [weapons.md](docs/research/weapons.md): his weapons, their table, levels and mods (RE.7).
  - [moby-format.md](docs/research/moby-format.md): moby skeletons, animation and meshes
    (P3.7).
  - [gameplay-format.md](docs/research/gameplay-format.md): levels' gameplay files (P3.10).
  - [effects.md](docs/research/effects.md): particles, effect textures, beams, lights
    and the guns' effects (P3.18).
- The reverse-engineering setup, and the catalogue of savestates, recordings and traces:
  [docs/re-setup.md](docs/re-setup.md).

Task IDs (`P2.4`, `RE.3`) are stable; use them in commits. Markers: `[x]` done, `[~]` started,
`[ ]` not started, `[-]` dropped or moved. An `In progress (<date>)` sub-bullet means a
session is working on the task now; notes on problems met stay with their task (CLAUDE.md,
"Mark your work in PLAN.md").

---

## 0. Status

- **Phase 0** is done apart from CI's first run (P0.7).
- **Phase 1** has the app flow, simulation core, input layer and greybox level (P1.1–P1.4).
  The hero (P1.5) has the whole UYA move set, DL's charge boots, and a movement profile
  per game (`uya.ron`, `dl.ron`; `--moves` or the menus), whose switches drop the moves
  DL's code lacks (crouch, wall jump, zip lines, dive, campaign lock-on) and give it its
  own flips, stick, strafing and grinding. He has the follow camera with
  lock-on, the lock-strafe and first-person cameras with the free look's aim assist and
  their blends (P1.7), melee for both games (P1.8), and all twenty of UYA's guns with their
  levels, mods and aim assist (P1.9), the whip swinging his wrench moves, held in his hand (with a UYA disc, the guns' own
  models, each level's, held in one hand or both as UYA's hold layers have them) with
  their firing animations, UYA's quick-select ring, UYA's ammo
  pickups and crates, a vendor's pad (P1.10), and a HUD on UYA's timings with the guns'
  crosshairs, lock-on reticle, target marks and the Flux scope, with its zoom (P1.12;
  the disc's textures for them, P3.18). He
  wears the open blocky model or, from the user's
  discs, any of 15 UYA suits (Clank and the heli-pack on his back, played by UYA's
  rules), 21 UYA multiplayer skins or 17 DL skins, and plays Ratchet's real animations
  with a UYA disc, face, fingers, fidgets and landings included (P1.17). Input can be recorded
  and replayed. The demo has an arena of enemies, a melee charger and a ranged shooter,
  that drop bolts and nanotech and, infected, fight each other (P1.11).
- **Reverse engineering:** the hero's movement is read in UYA's and DL's code and checked
  against traces (RE.3); the camera is finished (RE.4): every mode, blend, volume and
  collision rule the kit needs is read and traced in both games; RE.8 is answered
  (NTSC-U is the native tuning).
  UYA's weapons are read in code (RE.7): the weapon table with every gun's numbers per
  level, ammo, XP, mods and what they do, aim assist, all twenty guns' firing and shots,
  their V5 variants included (`weapons.md`, "Next steps" 4 has the few left), how
  firing affects the hero, the guns' aim indicators, and the effects engine the guns
  draw with (`effects.md`).
- **Disc layer:** UYA's TOC, levels, core, moby classes, armor WAD and rails are read
  from the disc, and DL's skin WAD, levels and cores (P2.1–P2.4, P3.3, P3.7, P3.10). The packed boot
  executables unpack into ELFs for Ghidra (P2.5). All eight discs, including the NTSC-U
  R&C1 and GC, are identified by their boot executables' hashes (P2.2).
- The work is on branch `re/hero-movement`, which isn't pushed yet.

**Next actions, in order:**
1. **Owner:** push `re/hero-movement` so CI runs (it needs the owner's credentials). Then
   fix anything Windows-specific it finds (P0.7).
2. RE.3 and P1.5: a trace of a death in UYA or DL for `death.ticks`, since the respawn is
   the level's code. DL's movement sections are read; left in `dl.ron` as UYA's are
   smaller items (the flick, the landing, the strafe flip's stick, the wrench's line, the
   gun arm, some camera numbers, and grinding's reach, switch search and speed).
3. The guns' effects from the disc (P3.18), with the whip's links and shield, then
   P1.10's nanotech, explosive and doubler crates ("Ammo
   pickups, crates and the vendor").
4. At lower priority: the disc layer, P2.3, P2.4 and P2.6, and the rest of RE.2.

**Wanted from the owner, when convenient:** savestates by moves that need a particular
place (`docs/re-setup.md`, "Still wanted").

---

## 1. Goals

| ID | Goal | Done when |
|---|---|---|
| **G1** | Rewrite the engine of Ratchet & Clank (2002) through Ratchet: Gladiator (2005) in Rust with Bevy. The first deliverable is an **ISO-free hero kit**: the R&C3/Gladiator player controller, camera and weapons on a **placeholder rig built from primitive shapes**, packaged as Bevy plugins that other projects and game mash-ups can reuse. It comes with a standalone demo level. | The demo is playable for 5–10 minutes on Windows and Linux with no game data. The kit crates work in an unrelated Bevy app (P1.16) and depend on no disc crate. Measured movement matches the originals within agreed tolerances (RE.6). |
| **G2** | Run each game **natively** on Windows and Linux (DX12 and Vulkan), with users supplying their own ISOs in `./source`. Every game runs on one shared hero controller with the R&C3/Gladiator controls, using that game's own movement values and abilities (D7). | Each supported release can be completed start to finish (Phase 5), with no emulator in the shipped path. |
| **G3** | Controller and keyboard+mouse support, with **key remapping**. | Every action can be remapped and the bindings persist. Gamepad and KB+M both work in menus and gameplay. |
| **G4** | **Direct-IP two-player online co-op in every game**, with mechanics derived from Ratchet: Gladiator. | Two PCs connect by IP address and play a campaign together in every supported game (Phase 6). The demo gets co-op first (P1.18). |
| **G5** | **A full PC remaster by default** (D4): widescreen and ultrawide, any resolution, high frame rates over a 60 Hz simulation, modern anti-aliasing and post-processing, mouse-first controls and complete settings menus. | Every enhancement is on by default and can be switched off individually. A "classic" mode reproduces the original presentation for comparison. |

**Non-goals for now:**
- Competitive online multiplayer, matchmaking or hosted servers (Horizon already revives UYA
  and DL online). Co-op connects the two players directly, with no servers of our own
  (D8).
- The PS3, PS4 or PS5 versions.
- A pixel-exact emulator.
- Shipping any game data.
- Contributing to other projects' repositories (D5).
- macOS, mobile or web. They aren't ruled out, just not targeted.

## 2. Ground rules

The full rules are in CLAUDE.md ("Legal rules", "Workspace conventions"). Those that shape
the plan:
- **No game data in the repo.** Anything derived from a disc is generated on the user's
  machine, from their own disc, into a local cache. CI blocks data files (P0.12).
- **GPL-3.0-or-later.** Code under compatible licences (Wrench, PCSX2, permissive
  projects) may be reused with its notices; decompiled game code never.
- **A fixed, deterministic 60 Hz simulation** with interpolated rendering: lockstep and
  rollback co-op need bit-identical state on Windows and Linux.
- **The hero kit is ISO-free** (`rac_sim`, `rac_input`, `rac_hero`, `rac_rig`,
  `rac_combat`, `rac_net`, `rac_demo`), and parsing and format crates don't depend on Bevy.
- Unit tests use synthetic data; tests that need a disc are gated (section 7).

## 3. What we know (summary)

The details and evidence are in `docs/research/`.

- **Discs** (all eight owned by the user):

  | Disc | Region | Role now |
  |---|---|---|
  | UYA SCUS-97353 | NTSC-U | **Primary reference for R&C3.** Packed ELF, 60 Hz, libuya, decompilation about 7% done. |
  | R&C3 SCES-52456 | PAL | The R&C3 PAL variant. libuya has a PAL build. |
  | DL SCUS-97465 | NTSC-U | **Primary reference for Gladiator.** Packed ELF; libdl has the hero structs and constants. Reference for co-op. |
  | Gladiator SCES-53285 | PAL | The Gladiator PAL variant. |
  | R&C1 SCUS-97199 | NTSC-U | Later; the reference for R&C1. Plain ELF. Lombyte targets this exact build. |
  | R&C1 SCES-50916 v2.00 | PAL | The R&C1 PAL variant. rac1-decomp targets this exact build. |
  | GC SCUS-97268 v2.00 | NTSC-U | Later; the reference for GC. Plain ELF; its files are visible on the disc. The Greatest Hits build; rac2-gc-decomp targets v1.01. |
  | GC SCES-51607 | PAL | The GC PAL variant. |

  Except on GC PAL, the data is hidden from the filesystem and reached through each game's
  own table of contents: R&C1's at LBA 1500, the others' at LBA 1001. The data inside is
  compressed with Insomniac's "WAD" LZ codec.
- **Engine:**
  - C++, built with SN ProDG GCC 2.95.3.
  - A resident "core" is joined by **per-level code overlays**, each holding nearly the
    whole game program, dispatched through `lvl.vtbl`, `lvl.camvtbl` and `lvl.sndvtbl`.
  - Rendering is VIF → VU1 microcode → GS; the terrain, tie and shrub renderers come from
    Naughty Dog's.
  - Audio is Sony's **989snd**, the same library family as Jak & Daxter's.
- **How much is shared:**
  - **Formats are highly shared** across all four engine generations: one asset pipeline
    with per-game adapters will do.
  - **Gameplay code is not shared**: less than 1% is identical across games, and each game
    has 3.7–7 MB of distinct code. Only the engine core carries over within each family
    (R&C1 with GC, UYA with DL).
  - So game logic has to come from **automatic translation of each game's own code**
    (section 4).
- **Hero movement:** UYA and DL tune the hero in per-second units at 60 Hz, and each PAL
  release is a conversion that keeps real-time behaviour (RE.8). The two games share
  their movement code and differ in a handful of values and rules (`hero-controller.md`,
  "Differences between the games").
- **Prior art** (October 2026, all moving fast):
  - **ReRAC**: Rust and Bevy, ISC. A native R&C1 port; a reference, narrower in scope (D1).
  - **OpenRAC**: GPL. Decompilations of all four games plus a planned C++ native port.
  - **Wrench**: GPL. The format reference for all four games.
  - **Horizon**: MIT. Its UYA and DL headers (`libuya`, `libdl`) are our starting point for
    the hero and weapons.
  - **OpenGOAL**: ISC. The closest architectural precedent; its 989snd reimplementation is
    directly reusable.
  - **ico-recomp**: MIT, Rust. A PS2 static recompiler whose crates we can reuse.
  - **ggrs**: MIT/Apache. GGPO-style peer-to-peer netcode, independent of Bevy.

## 4. Strategy

### 4.1 Approach: the hybrid "native engine plus translated game code"

The original README asked whether to use the ISOs only as data or to translate the game
code when an ISO is first selected. **We do both, by layer:**

| Layer | Source | Where it runs |
|---|---|---|
| Platform: window, input, audio out, GPU (DX12/Vulkan), sockets | Bevy, wgpu and ggrs | Native |
| Engine services: rendering (classic and remaster), audio engine, file and disc I/O, saves, input mapping | **Our Rust code** | Native |
| Assets: levels, models, textures, animation, collision, sound, video | The user's ISO, **extracted and converted on first run** into a local cache | Native data |
| Game logic: thousands of per-class update functions, missions, bosses, menus | The user's ISO, **statically recompiled on first run** (R5900 → Cranelift → native code, cached) | Native code, on top of our runtime, which emulates the hardware and IOP services at a high level (HLE) |
| Rewritten systems: the hero and camera (one controller for every game, with the R&C3/Gladiator controls and each game's own movement values and abilities), weapons, HUD, co-op and netcode | **Our Rust code (the hero kit)**, replacing recompiled functions through hooks | Native |

- **Why it fits:** formats are shared but code isn't. Recompiling scales to 4 engines × ~50
  overlays without hand-porting, and keeps the game logic faithful. Our own code provides
  the remastered rendering, the input, the hero, and co-op. No game code or data is ever
  distributed, and users don't need a C toolchain.
- **Progressive rewrite:** any recompiled function can be swapped for hand-written Rust,
  using the decompilations as the specification and the recompiled version as a
  differential-testing oracle.
- **Precedents:** OpenGOAL (native renderers fed by the game's own logic); N64Recomp and
  XenonRecomp (the user supplies the game, code is generated from it, function hooks allow
  changes); Dark Cloud's PC port (fixed 60 Hz logic, interpolated rendering).

**Rejected:** hand-rewriting all gameplay (too large, given how little code is shared);
embedding an emulator core (not native); compiling decompiled C
(ships game-derived code, and only R&C1's decompilation is mature).

**Phase 4 is the decision gate.** If the recompilation spike fails, we fall back to a
native rewrite of each game's core, using the decompilations as the specification.

### 4.2 Co-op and networking

- **Topology:** two peers, and no servers of our own (D8). One hosts on a UDP port and
  the other joins by `IP:port`. The host's port is opened automatically where the router
  allows (UPnP IGD via `igd-next`, NAT-PMP or PCP). Otherwise the players swap join
  codes, each holding that peer's public address (learned from public STUN servers), and
  both peers punch through their NATs at once. IPv6 connects directly when both peers
  have it.
- **Model:** like GGPO, both peers simulate the whole game deterministically, and only
  player inputs cross the network, via `ggrs`.
  - **The demo** (Phase 1) uses **rollback**: its state is small and easy to snapshot, and
    this proves the netcode for the reusable kit.
  - **The full games** start with **delay-based lockstep** (`ggrs` with prediction off,
    plus a few frames of input delay). Rolling back recompiled game state would mean
    snapshotting about 32 MB of emulated memory every frame. Rollback comes later if
    snapshots prove cheap enough (P6.2).
- **Safety nets:** a handshake checks the app build, the release (by boot-ELF hash) and
  the simulation-affecting settings; per-tick state checksums detect desyncs; the host can
  resend a full snapshot to resync.
- **Mechanics:** taken from Gladiator's co-op (RE.10), implemented once in the native hero
  and co-op layer, and applied to every game through per-game adapters (Phase 6).
- **Rendering:** each peer renders its own view. Local split-screen comes after online
  co-op, on the same foundation (D6).

### 4.3 Architecture

All crates live in `crates/`. Libraries are prefixed `rac_*`; the binary is `openrac`.

```
ISO-free hero kit: reusable in any Bevy app; never depends on the game-layer crates
  rac_sim      fixed 60 Hz clock, system order, render interpolation, seeded RNG
  rac_input    actions, bindings, remapping, per-tick input snapshots (G3)
  rac_hero     kinematic controller (R&C3/Gladiator controls), per-game profiles, camera
  rac_rig      the hero's skeleton, the models it wears, procedural poses and clips
  rac_combat   health and damage, melee, weapons, projectiles, pickups, quick-select
  rac_net      direct-IP sessions over ggrs: rollback or lockstep, desync checks (G4)
  rac_demo     greybox demo level, demo enemies and HUD (the standalone demo)

Game layer: needs the user's discs
  rac_iso      ISO9660, disc identification, TOC, sector streams                   (no Bevy)
  rac_formats  WAD codec, ELF/overlays, level/core/gameplay, mobys, ties, tfrags,   (no Bevy)
               shrubs, sky, collision, textures, VAG, 989snd banks, PSS
  rac_assets   extraction cache + Bevy AssetLoaders → Mesh/Image/AnimationClip/Audio
  rac_render   classic (PS2-faithful) and remaster render paths
  rac_audio    989snd port (from OpenGOAL), VAG/PSS streaming
  rac_recomp   R5900 (+MMI, FPU, VU0 macro) → Cranelift; compiled at install       (no Bevy)
               and cached
  rac_runtime  EE memory, HLE kernel and IOP RPC, hooks, render bridge (G2)
  rac_tool     CLI binary `openrac-tool`

App
  openrac      launcher and game: the demo when no disc is present, full games when one is
```

### 4.4 Order of targets

| # | Release(s) | Why it is in this position |
|---|---|---|
| 1 | **UYA NTSC-U (SCUS-97353)**, then R&C3 PAL (SCES-52456) | The controller feel to match (D2). Both regions are owned. libuya (MIT) covers NTSC and PAL, and the decompilation supplies names for the boot path. |
| 2 | **DL NTSC-U (SCUS-97465)**, then Gladiator PAL (SCES-53285) | Same code family as UYA, with about 69% of the core shared. libdl (MIT) has the hero structs and constants. It holds the co-op implementation behind G4. |
| 3 | R&C1: NTSC-U SCUS-97199, then PAL v2.00 (SCES-50916) | References: rac1-decomp (PAL, about 78% done) and Lombyte (NTSC, our build exactly), and ReRAC. ReRAC's ISC format crates may be reused (P3.16). |
| 4 | GC: NTSC-U SCUS-97268 v2.00, then PAL (SCES-51607) | Same code family as R&C1. |

- NTSC-U builds are the primary references: they have 60 Hz data and the community's
  symbols. PAL releases are supported variants with their own address maps, languages and
  50 Hz handling (RE.8).
- The hero kit implements the UYA/DL move set, with each game's values in its own profile
  (RE.3–RE.6). R&C1 and GC get its controls and the moves that need no item, with their
  own movement values and abilities (D7).

## 5. Decisions

### Open, needing the owner

None at the moment.

### Made

| Date | Decision |
|---|---|
| 2026-10-11 | **D1:** An independent project, broader than ReRAC: an ISO-free, reusable hero kit (usable in mash-ups) and direct-IP two-player co-op in every game, with mechanics from Ratchet: Gladiator. ReRAC and OpenRAC stay references; permissively licensed code may be reused with attribution. |
| 2026-10-11 | **D2:** Match R&C3's and Gladiator's player controller, and backport its controls and the moves that need no item to the older games (D7). UYA/R&C3 and DL/Gladiator come first (section 4.4). |
| 2026-10-11 | **D3:** R&C1 and GC use their NTSC-U discs as references, as UYA and DL do; the owner supplied both on 2026-10-15. They come after UYA/R&C3 and DL/Gladiator (section 4.4). |
| 2026-10-11 | **D4:** A full PC remaster by default (G5), with a classic mode for comparison. |
| 2026-10-11 | **D5:** The repository is public. No contributions to other repositories are planned. |
| 2026-10-11 | Reverse engineering is a continuous track from Phase 0, led by the hero controller. |
| 2026-10-11 | Netcode: deterministic GGPO-style peer-to-peer via `ggrs`. Rollback in the demo; the full games start with lockstep (section 4.2). Provisional until P6.2's ADR. |
| 2026-10-11 | Rust pinned to **1.99.0** in `rust-toolchain.toml`; Bevy **0.20**; a virtual workspace in `crates/*` and `tools/*`. |
| 2026-10-11 | Pipelines compile **synchronously** (`RenderPlugin::synchronous_pipeline_compilation`). Bevy's parallel compilation crashed 8 runs of 8 on NVIDIA driver 595.91.07 / RTX 4070 SUPER; synchronous gave 18 clean runs of 18. Revisit under P1.15. |
| 2026-10-11 | Strategy: the hybrid in section 4.1, provisional until the Phase 4 gate. |
| 2026-10-11 | Reverse-engineering tools live in `tools/*` and never ship. `pine-probe` has its own PINE client: the protocol is tiny, and we need raw batched reads with our own tear handling. |
| 2026-10-11 | Savestates and traces come from **private headless PCSX2 instances** (`boot.sh`, `replay.sh`, `warp.sh`) with copies of the owner's memory cards and settings, never from the owner's live instance. Their memory may be written (`pine-probe poke`, the level-change request) to grant an item or reach a level; the catalogue in `docs/re-setup.md` records it. |
| 2026-10-12 | Hero constants come from **static analysis of the game code**; traces verify them. |
| 2026-10-12 | The kit's movement is in **per-second units with the NTSC-U values, simulated at 60 Hz** (RE.8: NTSC-U is the native tuning, PAL a conversion). |
| 2026-10-12 | Ghidra scripts live in `tools/ghidra/`, outside the Cargo workspace. Decompiled output stays in `cache/`; only facts in our own words and `re/<serial>/*.toml` are committed. |
| 2026-10-12 | The kit crate **`rac_sim`** holds the 60 Hz clock, the `SimSet` order, `SimTick`, interpolated `SimTransform` and `SimRng` (PCG32). The workspace turns Bevy's default features off so kit crates ask only for what they use; `openrac` turns them back on. |
| 2026-10-12 | Settings are **TOML** (`settings.toml` in the platform config directory). `PlayerInput` is a fixed 10-byte encoding (move stick in PS2 counts, look in binary-angle units, held and pressed bits); input recordings are `.orin` files of them. |
| 2026-10-12 | Hero collision uses **`parry3d` 0.31** behind the kit's own `CollisionWorld`. The hero's origin is at his feet; his capsule is libdl's (radius 0.45, sphere centres 0.7/0.5 and 1.05), floating over a ground probe. |
| 2026-10-12 | `rac_input` maps pad sticks from **circle to square**, and keys send full counts on both axes: the games expect a PS2 stick's square range. |
| 2026-10-12 | Hero surfaces carry a **`Material`** (solid, wall-jump, no-grab, hazard, deadly) after the games' collision materials. Health is a `Health` component beside the controller, which only reports hazard contact and whether a hit is fatal. |
| 2026-10-12 | Rules the games time by **animation frame** assume one animation frame per tick until a trace says otherwise. Presses are **buffered** as in the games: each counts once, within its move's window. |
| 2026-10-12 | The dev profile keeps **line tables only**, and heavy cargo commands run at reduced priority with fewer jobs (CLAUDE.md, "Commands"): full debug info and unrestricted parallel links froze a 31 GB machine. |
| 2026-10-12 | **"Backport" means one controller for every game, carrying the controls and the moves that need no item; each game keeps its own abilities** and its own movement values (D7). Moves the games grant as items (charge boots, heli-pack and thruster-pack moves, glide, grind boots, gadgets) are per-hero ability flags that each game sets from its own items and saves, so R&C1 never gets charge boots. The demo turns every flag on. |
| 2026-10-12 | **Nothing is left only in a session's context:** work is written into PLAN.md and `docs/` as it happens, and every savestate, recording, script and layout goes into `source/<game>/` (CLAUDE.md, "Every session"). |
| 2026-10-13 | **The open model** is a blocky, flat-shaded gold Ratchet with Clank and the wrench, made in code with UYA's hero as the visual reference. It is an acceptable placeholder for the public repository. It lives in the kit crate **`rac_rig`**, so `rac_hero` stays free of rendering, and follows UYA's joint layout so animations from the user's disc can drive it. |
| 2026-10-13 | **Game data reaches the kit only at run time, through kit types.** `openrac` reads the user's disc at start-up and hands `rac_rig` clips and models on the rig's joints. The kit never reads a disc, and nothing read is stored. The disc folder is `source/` by default (`--discs`). |
| 2026-10-13 | **The hero's model is selectable:** the open model or one imported from the user's discs. Every model wears the rig's skeleton. An import is offered only when its bones line up: each rig joint has a source joint descending from the rig parent's, and the other source joints fold into their nearest such ancestor. Clips carry offsets from the source's rest pose, so each model keeps its proportions. Joints whose frame matters follow UYA's convention (`model::wrench_points`). Chosen in the menus (saved in `settings.toml`) or with `--model`. |
| 2026-10-13 | **parry3d's `enhanced-determinism` stays off.** It turns on glam's `scalar-math`, which drops `Vec4`'s 16-byte alignment that Bevy's GPU mesh buffers rely on (the app quit on a wgpu validation error). `rac_hero` depends on `glamx` for its `libm` feature, so parry's scalar maths still avoid each platform's C library; `openrac` asserts the alignment at compile time. Determinism relies on building x86-64 without FMA. |
| 2026-10-15 | **Each game is a movement profile; the controller is shared.** Where the games differ in numbers, the profile holds them; where a rule differs, the profile gets a switch (`strafe.flips`), not a second controller. A game's profile keeps UYA's value, marked "not read in DL" (or that game's equivalent), until its code is read, and a test lists the fields that differ from UYA's. `uya` is the default; the app chooses with `--moves` or the menus. |
| 2026-10-15 | **D6:** Local split-screen co-op, as Gladiator had, comes after direct-IP co-op works, on the same multi-hero foundation (P6.6). |
| 2026-10-15 | **D7:** Each game's hero moves with that game's own values (jump heights, speeds, timings), read from its code into its own profile. What carries across the games ("backported") is the controls and the moves that need no item; moves granted by items stay with the games that have them (2026-10-12). The older games also offer their original hero ("classic movement") as an option in single-player; co-op always uses the native hero. |
| 2026-10-15 | **D8:** Connectivity goes as far as it can without servers of our own: direct IP; automatic port mapping (UPnP IGD via `igd-next`, NAT-PMP, PCP); IPv6 where both peers have it; and UDP hole punching, with each peer's public address (learned from public STUN servers) carried in join codes the two players swap themselves. No master, rendezvous or relay servers. Two peers both behind symmetric NATs may still need a forwarded port. |
| 2026-10-15 | **The rig's skeleton is UYA's hero's whole skeleton** (111 joints in his order, then Clank's and the wrench's), replacing the 44-joint one, so clips drive the face and fingers joint for joint. Poses carry a scale per joint, which its children take on as in the games; a model bound to joints turned at rest says so (`ModelData::turns`). A rig joint a source skeleton lacks may sit between two it has. Which clip plays, and how fast, follows the game's own rules (`rac_rig/src/play.rs`); a game hands the rig its rules for fidgets with its clips. |
| 2026-10-15 | **The repository is GPL-3.0-or-later, replacing MIT**, the hero kit included, so projects reusing the kit must be GPL-compatible. Code under licences GPLv3 can absorb (permissive, GPL-3.0, GPL-2.0-or-later, LGPL; so Wrench and PCSX2) may be reused with its notices kept and listed in `prior-art.md`; GPL-2.0-only and unlicensed code stay reference only, and decompiled game code is never committed. GPLv2 was ruled out because several dependencies (parry3d, winit, cpal) are Apache-2.0 only. Copies of the MIT commits that were already pushed stay MIT. |
| 2026-10-15 | **Lock-on is gamepad only:** with a mouse the player aims. `rac_input` ignores keyboard and mouse bindings for `Action::LockOn`; its default is UYA's, both triggers together. |
| 2026-10-17 | **The look stick reaches the simulation as stick counts** (`PlayerInput::look_stick`, making the encoding 12 bytes and `.orin` version 2; version 1 files still load), so each camera applies its game's own response: the follow camera's linear turn and springing tilt, the free look's squared (UYA) or linear (DL) speeds and quick turn. The mouse stays a direct turn in every camera (G5), and the follow camera keeps the tilt the mouse sets. The camera setting (third person, lock-strafe, first person) is a player option on `FollowCamera` whose default is the movement profile's. |
| 2026-10-20 | **What the games carry on the hero as mobys of their own are attachments** (UYA's Clank and heli-pack): a model part on a skeleton of its own, riding a rig joint with its place and turn but not its scale, as the games normalise the joint before placing a moby. Their clips hold each joint's whole transform, and the game hands the rig its rules for choosing them (`BackAnimations`), as for fidgets. The rig's skeleton stays the hero's; the open model keeps its own Clank on the rig's Clank joints. |
| 2026-10-22 | **The free look's aim assist is the stick's**, as lock-on is the gamepad's (2026-10-15): once the mouse turns the view the assist stops until the stick turns it again. **The quick-select ring plays as UYA's with QUICK SELECT PAUSE off**: it opens over the running world and holds the hero's input after 15 ticks, so co-op stays in step. |

## 6. Roadmap

The phases run roughly in order. The **reverse-engineering track (RE)** runs alongside
every phase. Every task follows CLAUDE.md's definition of done.

### Phase 0: Foundations

- [x] **P0.1** Native toolchain, Rust pinned in `rust-toolchain.toml`.
- [x] **P0.2** The Cargo workspace and the Bevy 0.20 app `crates/openrac`, with
  `--exit-after-frames N` and a `dev` feature for dynamic linking.
- [x] **P0.3** `source/.gitignore` ignores everything but itself; the root `.gitignore`
  ignores `*.iso` and `/cache/`.
- [x] **P0.4** First inspection of the discs → `docs/research/disc-layout.md`.
- [x] **P0.5** Prior art and emulators → `prior-art.md`, `engine-notes.md`.
- [x] **P0.6** Work around the NVIDIA parallel pipeline compilation crash (section 5).
- [~] **P0.7** CI with GitHub Actions on `ubuntu-latest` and `windows-latest`
  (`.github/workflows/ci.yml`): fmt, clippy, tests and a build, cached with
  `Swatinem/rust-cache`. Every step passes locally on Linux.
  - To do: its first run on GitHub, especially Windows; optionally a Linux smoke run under
    `xvfb-run` with lavapipe (`openrac --exit-after-frames 60`).
- [-] **P0.8** Evaluate ReRAC. Moved to P3.16: it only matters for R&C1.
- [x] **P0.9** Decisions D1–D5 (section 5).
- [x] **P0.10** All eight whole-image SHA-1s match redump.org (`disc-layout.md`).
- [x] **P0.11** `CONTRIBUTING.md`.
- [x] **P0.12** Data-hygiene guard: `tools/check-data-hygiene.sh`, the `data-hygiene` CI
  job and `.githooks/pre-commit`. It rejects disc-derived file types, savestates, memory
  cards, recordings, traces and dumps, and any binary over 1 MB not on its allowlist.
  - It failed on a tracked file deleted or moved in the working tree (it measured every
    tracked path); it now skips paths that are gone.
- [x] **P0.13** Relicense the repository from MIT to GPL-3.0-or-later (section 5):
  `LICENSE`, the workspace's `license`, the README's notice, and the licence rules in
  CLAUDE.md, CONTRIBUTING.md and `prior-art.md`.

### Track RE: Reverse engineering and the knowledge base

**Order:** UYA NTSC-U and DL NTSC-U, then their PAL variants (R&C3, Gladiator), then R&C1
and GC. Raw dumps, savestates and traces stay out of git; only facts in our own words are
committed (names, addresses, layouts, derived constants).

- [x] **RE.1** Tools and setup (`docs/re-setup.md`): PCSX2 2.8 with PINE, Ghidra 12.1 with
  ghidra-emotionengine-reloaded, and `tools/ghidra/` to import and analyse a savestate,
  decompile chosen functions into `cache/`, and apply `symbols.toml`.
- [~] **RE.2** Machine-readable knowledge base (`re/README.md`):
  - Done: `re/<serial>/hero.toml` (the hero struct, verified fields and a frame counter,
    read by `pine-probe`) for R&C3 PAL, Gladiator PAL, UYA NTSC-U and DL NTSC-U; and
    `symbols.toml` for those four, each symbol with a confidence and source, checked by
    `pine-probe`'s tests. Seeded from libdl (MIT, attributed): DL NTSC-U's resident
    library functions and globals (100 symbols), and the library functions found by
    matching code in UYA NTSC-U, R&C3 PAL and Gladiator PAL (37, 37 and 53; method in
    `engine-notes.md`, "Code layout and overlays").
  - To do: libuya's addresses, which need a memory image of UYA's multiplayer program (a
    savestate in a local multiplayer map); a code-matching tool in `tools/` in place of
    the throwaway script used for the seeds, to map the PAL builds' level overlays and to
    carry libuya's names into single player; `structs.toml` and `memory-map.md`; a Ghidra
    import script and the Rust hook tables (P4.7).
  - libuya's addresses are for UYA's multiplayer program, which is linked differently from
    single player, so none could be copied. UYA and R&C3 got libdl's library functions by
    matching code instead.
- [~] **RE.3** **Hero controller**, the first priority, for UYA and DL
  (`hero-controller.md`; addresses in `re/<serial>/symbols.toml`). Feeds P1.5.
  - Read in code and checked against traces: the stick pipeline, walking and the
    sharp-turn skid, every jump type with its entry rules and parameters, air control, the
    double jump, glide, heli jump, long jump, flips (strafe, landing and crouch), the
    crouch turn, falling, the wall jump, ledges, the slope limit, getting hit and dying,
    melee (UYA's and DL's), swimming, zip lines, strafing, DL's charge boots, the
    animation each state plays, the moves that need items, and DL's differences from UYA:
    among them, DL flips only as a strafing jump lands, and has no crouch (nor the long
    jump, heli jump or crouch flips), no glide, no wall jump and, in the campaign, no
    lock-on (only multiplayer levels set a target); its ledge's lip stays
    where the top's cast hit, so he hangs against the wall, and its shimmy runs 32-frame
    cycles of 0.60 u (`hero-controller.md`, "Deadlocked's moves", "Deadlocked's
    ledges").
  - Read in code only: grinding and the grind attack (no UYA level can be grinded, and a
    rail written into Veldin crashes PCSX2, so a trace waits for GC or DL), the wrench's
    clank, lock-on (needs a weapon none of our savestates holds), changing strafe sides,
    the walk's start at m 0.22 and stop under 0.17; and in DL, the missing zip lines and
    dive, hits (a 47- or 70-frame safe time by camera), swimming (UYA's otherwise), the
    stick with no cut-off and its length as m, strafing's 60° back, 0.05 back stick
    and side change that stops both ways, grinding facing along the rail with its back
    to the camera (a hop round past 120°, state 63) and chained rails, and death, which
    reports 10 frames in: "Deadlocked's moves", "Deadlocked's hits", "Deadlocked's
    swimming", "Input", "Deadlocked's strafing", "Deadlocked's grinding", "Getting hit
    and dying".
  - Both games' death ends by reporting to the level's code, which respawns the hero (not
    read), so `death.ticks` needs a trace of a death.
  - DL's lock-strafe camera is read and traced under RE.4 (`camera.md`, "Deadlocked's
    camera").
  - `hero-controller.md` had DL's skid crouch on R1 "as UYA"; DL's code works out R1's
    pad bit there and ignores it, as everywhere else it would crouch. Corrected.
  - DL's shimmy steps are 1.5 × UYA's table, read as three times UYA's pace; the trace
    shows about UYA's pace, because DL's lip search has a zero-length probe, leaving the
    hang point further ahead than the wall lets him come, so the wall takes two-thirds of
    each step. The kit models the lip (`ledge.lip_walk`).
  - The UYA shimmy's side look was read as 0.1 u; the helper sets the vector's length,
    so it is 0.2 u in both games. Corrected in both profiles.
  - Both games' climbs hang still through the push-off and a frame more (traces), where
    the kit pressed the hero down and left a frame early. Fixed in the kit.
  - To do (`hero-controller.md`, "Next steps"): a trace of a death in either game; the
    smaller items `dl.ron` still marks "not read in DL"; which callers of
    the move-angle function snap the stick's axes ("Input", 5); wading (state 117); how the games carry the hero on moving platforms; and a diagonal part-tilted
    stick on NTSC (the dead zone, the 0.25 cut-off and the speed's scaling are verified
    with `AXIS_SCALE` keys).
  - DL's chained rails swap only the spline pointer and keep the segment index, so where
    on the new rail he lands wasn't worked out; the kit starts at the new rail's start
    ("Deadlocked's grinding").
  - Next step: a capture of a death on UYA NTSC, from any savestate where the hero can
    be killed, logging the frames from state 57 to the respawn.
- [x] **RE.4** **Camera** (`camera.md`). Feeds P1.7.
  - Done, UYA NTSC: the follow camera with its height modes, obstacles and collision
    (clearance point, sweep, pull-in, the 30-frame and 30 u resets, the collision masks);
    lock-on's steering; the camera manager, level cameras, blends between cameras,
    camera volumes, the fixed-point, path and 0x11 cameras; the look, lock-strafe, first
    person and the strafing a gun forces; the right stick at part tilt. Each is read in
    code, and all but lock-on, the orbit blend and 0x11 are checked against traces
    (`camera.md`, "Traces").
  - Done, DL NTSC: the follow camera (UYA's, 6 u back, no push-back), lock-strafe and
    first person, read in code and traced, the stick at part tilt included (DL doesn't
    square it).
  - Open, at lower priority (`camera.md`, "Next steps" item 6): a 1° unexplained in the
    follow camera's tilt, the vertical invert's default, the first-person gun placement,
    the camera-only collision groups, the camera classes only other builds hold, DL's aim
    assist and special follow branches, and the untraced vertical modes.
  - Keys tilt the sticks part-way on a `boot.sh` started with `AXIS_SCALE` (PCSX2's
    `AxisScale`; `docs/re-setup.md`), so stick-response checks no longer need the owner.
  - The camera blocks move with each level's build (0x580 lower on the Phoenix, 0x1740 on
    Florana, whose core globals sit 0x1680 lower), like the rest of the level overlay's
    data: find them per level before capturing.
  - Captures slow the game to about 19 to 55 frames a real second, and some samples come
    out torn (a value repeats, then skips a frame); simulations must skip those frames.
  - A take that writes the camera setting must also clear the free look's flag bit 0, as
    `hero_camera_set` does: left set, the left stick turns the view too (the first take
    2 had to be redone).
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
  `re/SCUS-97353/symbols.toml`). Feeds P1.8 and P1.9.
  - Done 2026-10-23: how the hero holds a gun: the hold layers, their classes, weights
    and rests, and the whip's own scale (`weapons.md`, "Holding a gun").
  - Done 2026-10-22: the last of UYA's guns' open items: the Flux Rifle's piercing (its
    scoped beam, class 0x186F), the Annihilator's V2–V4 mines and V5 seekers, the agents'
    sight, walking and gun (from V2), the Nitro Eruptor's bullets, the shock arc
    (enemies in sight, 10 frames to reach), the view's aim point (`fps_cam_find_target`,
    30 u), the joint springs behind the raised arm and the upper body's turn, and when
    the gun's own sequences play (`weapons.md`, "The gun's own animation"). Also the aim
    indicators (`weapons.md`, "Aim indicators": the lock-on reticle, markers, crosshair
    colours, each gun's crosshair and marks) and the effects engine with each gun's
    effects (`effects.md`, for P3.18).
  - Open: what the damage-record flags mean, the rest of the agent's update, the Plasma
    Storm's arc, the Decimator's and Lava Gun's V5 (`weapons.md`, "Next steps" 2 and 4).
  - Done 2026-10-22: UYA's other fourteen guns, read in code at the level of their
    firing and their shots (`weapons.md`, "Where each gun's code is" and the sections
    after the Flux Rifle). Not read: the V5 variants' own code, the agents' AI, the ducks'
    and infected enemies' behaviour (each enemy's own code), and the Hydra's spit, the
    Suck Cannon's and the RY3NO's shots in flight (`weapons.md`, "Next steps" 4).
  - Item 0x7F, taken for a multiplayer wrench in `hero-controller.md` and
    `symbols.toml`, is the Plasma Whip; corrected.
  - Read in code 2026-10-21: ammo pickups, crates and the vendor's rounds; the
    quick-select ring; the weapon, health and bolt HUDs (`weapons.md`, those sections).
    Open: placed pickups' rounds, vendor kind 4, multiplayer's ring.
  - Done for UYA NTSC-U, in code: the weapon table (0xF0-byte rows
    per item and level) with every gun's damage, ammo, prices and XP thresholds; owning,
    ammo, levelling, XP from kills and the bought Mega levels; the lock-on, shock and acid
    mods and the levels that set them; switching and firing animations; the guns' aim
    assist; the damage queue's one pending hit per target; and the Shock Blaster
    (hitscan), N60 Storm (rapid fire), Nitro Launcher (lobbed), Lava Gun (stream),
    Annihilator (homing) and Flux Rifle (sniper). Also, in code: the hero's side of
    firing (the firing flags and animations, the walk's turn while firing, no double
    jump while firing, the raised gun arm, the upper body's turn to the gun's target, the
    fire direction and how guns aim with it, state 29, Triangle's quick switch), and
    what the shock and acid mods do, with their tables (`weapons.md`, "The hero's side of
    firing", "What shock and acid do").
  - Traced on UYA NTSC (`uyantsc-veldin-fire-{1,2}`, the Shock Blaster and N60 given by
    writing their bytes): the hero's side of firing, its turn spring exact to 10⁻⁸
    (`weapons.md`, "Checked in traces"). Projectiles aren't traced.
  - The Shock Blaster at V1 was read as firing on a press; the trace and the code show it
    fires while Circle is held, as often as its cooldown allows. Corrected.
  - The weapon table is static data in each level's overlay and moves per level, so the
    disc path needs P2.6; until then the numbers come from savestate images.
  - Row +0x50 is the rate of fire only for the guns whose code reads it; the N60 Storm
    has its own 14-frame interval (`weapons.md`, "Numbers").
  - The Nitro Launcher's acid does nothing as read: its blast passes item 0x77, which has
    no entry in the acid table. Still open whether another build fills it.
  - `hero-controller.md` had row +0x24 as a held weapon's idle and the release skid's
    hand-slot condition as unknown; +0x24 is the firing sequence, and the skid needs the
    wrench in hand. Corrected.
  - Open: projectiles in traces (they are spawned mobys a fixed layout can't follow),
    and all of DL.
  - Next step: the rest of the hero's side of firing in traces (a glove's arm layers, the
    raised arm, a target), with `uyantsc-veldin-fire.sh` extended (`weapons.md`, "Next
    steps", item 1). P1.9 is built from what is read (`weapons.md`, "In the kit").
  - Weapon definitions (DL's `GadgetDef` is 0xB0 bytes): fire rates, damage and projectile
    behaviour.
  - Upgrade and XP tables: UYA V1–V5, and DL V1–V10 with mods.
  - Quick-select, aim assist, and how weapons use lock-on.
- [~] **RE.8** **Timing across regions:** answered for UYA and DL from both regions' code
  and checked with NTSC traces (`hero-controller.md`, "Frames and regions"). Constants
  are per second, tuned at 60 Hz; R&C3 PAL folds them with 1/50, Gladiator scales them at
  run time, and frame counts are cut to about 5/6. To do: R&C1 and GC, including R&C1's
  50/60 Hz menu choice.
- [ ] **RE.9** **Engine structure:** the main loop, tick order, VSync and threads, overlay
  loading and `lvl.vtbl` dispatch. UYA/DL first, then R&C1/GC.
- [ ] **RE.10** **Gladiator's co-op mechanics**, from DL NTSC-U and Gladiator PAL:
  - Who player 2 is.
  - Respawn and revive rules.
  - What is shared versus per player: bolts, ammo, weapons, upgrades, health.
  - Camera.
  - How missions and cutscenes handle two players.
  - The split-screen level builds (21–35).
  - Difficulty scaling.
  - Output: `docs/research/coop.md`. Feeds Phase 6.
- [ ] **RE.11** **Renderer:**
  - For each VU program: its input packet format and meaning.
  - How the DMA chain is built, and the hook points (`VU1_sendChain`, `VU1_syncChain`,
    `sceGsSyncV`).
  - Which GS tricks are used: bloom, render-to-texture, alpha tricks.
  - Feeds Phase 3 and P4.6.
- [ ] **RE.12** **IOP RPC services:**
  - 989snd, cdvd, mc, and pad/dbcman (with its vibration wavetables).
  - The unknown "stash", and DL's streamer and astrm.
  - The network modules can be stubbed.
  - Feeds P4.5.

### Phase 1: ISO-free hero kit and standalone demo (G1, G3, first G4 milestone)

Crates: `rac_sim`, `rac_input`, `rac_hero`, `rac_rig`, `rac_combat`, `rac_net` and
`rac_demo`, wired up in `openrac`, which runs the demo whenever no supported disc is found.
Any non-code asset must be CC0, recorded in `crates/openrac/assets/CREDITS.md`. Check that
an ecosystem crate supports Bevy 0.20 before adding it (CLAUDE.md).

- [x] **P1.1** App flow (`openrac`: `flow.rs`, `menu.rs`, `settings.rs`): `AppState`
  Boot → MainMenu → InGame with a `Pause` sub-state, the game clock running only in game;
  a focus-list menu; `settings.toml` (missing files are created, broken ones kept and
  ignored); the cache directory is `./cache` in dev builds.
- [x] **P1.2** Simulation clock and determinism (`rac_sim`): `FixedUpdate` at 60 Hz with
  `SimSet` Input → World → Hero → Particles → Camera, interpolated `SimTransform`,
  `SimRng`, Bevy's `libm` feature, and input record and replay. Cross-OS hash checks are
  P1.19.
- [~] **P1.3** Input layer (`rac_input`, G3): actions with UYA's pad layout and KB+M
  defaults (WASD plus mouse look and aim), dead zones, sensitivity, invert and swapped
  sticks, `InputSettings` saved in the settings file, the 12-byte `PlayerInput` (the
  look stick as stick counts, so each camera applies its game's response; section 5),
  `ActionState` for menus, the `.orin` codec, `Binding::PadBoth` for two-button chords,
  and gamepad-only lock-on (section 5). Saved settings that predate an action get its
  defaults.
  - To do: the rebinding UI (with P1.12), and checking the full stick curve, since Bevy's
    own axis processing runs before ours.
- [x] **P1.4** Greybox level (`rac_demo`), as data tagged by purpose (`Surface`): ramps,
  stairs, ledge blocks, a wall-jump shaft to a high platform, a gap course, a moving
  platform, a grind rail, a zip line, a pool, a melee yard, a hazard pad and checkpoints.
- [~] **P1.5** Kinematic hero controller (`rac_hero`), implementing the UYA/DL move set
  from RE.3.
  - **Design:** a pure step function over (state, input, camera yaw, profile, collision)
    (`controller::step`), so tests drive it without an app. Collision goes through
    `CollisionWorld` (sweeps, rays, a `Material` per body, rails, water) with a
    brute-force `parry3d` backend. The capsule floats 0.25 u over a ground probe, so steps
    up to 0.25 u need no code.
  - **Profiles:** each game's numbers are a RON profile, each value's source in a comment:
    `assets/uya.ron` (the default) and `assets/dl.ron` (section 5, 2026-10-15). R&C1 and
    GC will get profiles of their own, read from their code (D7, P5.7).
  - **Done:** idle, walk (turn springs, the stop, the sharp-turn skid), crouch and its
    turn; standing, run, long and heli jumps, flips (strafe, landing and crouch), the
    double jump, glide, fall and air control, the fall's landings and lockout; moving
    platforms; the wall jump; ledges
    (grab, hang, shimmy, climb, let go); hits, hazards, death and checkpoints; DL's charge
    boots; the ability flags; grinding, side on; swimming; zip lines; strafing and lock-on
    (`lock_on.rs`); melee (P1.8); DL's own flips, ledges (traced) and hits, and its
    missing crouch, wall jump, zip lines, dive and lock-on (`strafe.landing_window`,
    `walk.crouches`, `wall_jump.enabled`, `zip_line.enabled`, `swim.dives`,
    `lock_on.enabled`, `dl.ron`'s `ledge` and `hit`); DL's stick and strafe sides
    (`stick.magnitude`, `walk.start_above`, `strafe.back_degrees`,
    `strafe.reverse_eases`); DL's grinding facing along the rail, its hop round
    (`JumpKind::GrindTurn`) and chained rails (`grind.turn_round`, `grind.chain`). Tests: `rac_hero/tests/` and
    `rac_demo/tests/greybox_moves.rs`.
  - **To do:** auto-jumping rails, wading, and DL's open items under RE.3.
  - The motor dropped a skin's width of every slide along a wall, cutting each of DL's
    shimmy steps (which point into the wall) by 0.0016 u; it now keeps all the motion it
    didn't travel (`motor.rs`).
  - The grind faced along the rail; UYA's code turns him 90° to its right (a stance
    offset missed in the first reading). It is now `grind.stance_degrees`, which DL's
    profile sets to 0: DL's hero does face along the rail.
  - The crouch turned on the walk's setting-off spring, about four times faster than UYA's
    own, which closed the crouch flip's window; it now has the game's (`walk.crouch_turn`),
    verified on UYA NTSC.
  - The rig took a flip's side from its velocity, which a crouch flip from a run points
    forward; the controller now records the side (`HeroMotion::flip_side`).
  - **Placeholders:** the death's length, DL's starting health, and the end of the
    charge's bounce.
  - The hazard surface threw the hero up and back with made-up numbers; both games stop
    him where he stands for 41 ticks and take the most health / 8 (`hit.hazard_ticks`,
    `hazard_divisor`; `hero-controller.md`, "Getting hit and dying"). The rig still plays
    the get-hit's animation for it **[simplified]**.
  - **Known mismatch:** a long jump from a 7-tick crouch leaves at 8.2 u/s in the kit and
    7.8 u/s in the trace: the game's first crouch frame brakes less, and its jump frame
    brakes once more. A crouch flip from a run carries one crouch tick's braking more
    speed than the game's for the same reason.
- [-] **P1.6** Movement feel reference. Moved to RE.3, RE.5 and RE.6.
- [x] **P1.7** The hero's camera (`rac_hero/src/camera/`, the profile's `camera` section),
  from RE.4 (`camera.md`):
  - Done 2026-10-22: the free look's aim assist (`FreeLookAssist`; `camera.md`, "In the
    kit"): the target, the cuts, the quick turn's wait, the snap and keeping a target in
    view while he moves; the stick's only (§5, 2026-10-22).
  - **The follow camera** (third person): UYA's drag camera with the follow point's
    height modes (the jump hold, the heli, double-jump and flip rises, the jump attack's
    halfway height), the look point's air gain, the distance and height springs, the
    push-back, the stick's linear turn and springing tilt, lock-on's turn and look
    spring, strafing's drag-free arm, the aim's 90-tick blend to the look point while
    strafing or locked on, the clearance point, the arm sweep, the camera-end sphere, the
    cosine pull-in with its stick bump and 1.5 u minimum, and the 30-tick and 30 u
    resets. DL's is the same 6 u back, so it has no push-back.
  - **The free-look camera** for the lock-strafe and first-person settings and the
    third-person look (`Action::FirstPerson`, `HeroState::Look`): UYA's squared and DL's
    linear stick, the quick turn, the limits and their ease, the eye's spring and jump
    hold, the 6 u offset with its ray cut and ease-out, from last tick's eye. The
    setting is `FollowCamera::setting` (UYA third person, DL lock-strafe by default),
    chosen in the menus or with `--camera`; lock-strafe doesn't lock on.
  - **Blends** between the cameras (the cosine lerp at the games' rates), and
    **`CameraVolume`**, a box with a priority and optional overrides.
  - Standing while strafing, he steps round to the camera once it is 10° off.
  - **Simplified or left out:** the kit also sweeps the camera from the pivot so it never
    ends behind a wall; the platform carry, the swim and critter rules, volume steering and fields of
    view, and the modes for states the kit lacks (jets, rocket jumps, jump pads, the
    bounce's mode 11) aren't modelled; the camera setting isn't in input recordings.
  - **Not traced against the kit:** a replay of RE.4's traces through the kit camera
    would check it end to end (camera.md, "Next steps").
  - Tapping strafe while moving sideways jerked the view: the kit snapped the aim between
    the follow and look points. The games blend it over 90 frames (`camera.md`, step 8;
    read in code, not traced); fixed.
  - A double or heli jump turning into a glide jolted the view up 35° in a tick: the kit
    pointed the view at the aim, so the follow point's snap from the jump's hold to his
    height reached it. The games take only the yaw from the aim and tilt the view from
    the arm and sprung heights, plus a look-up with the arm below level (0x3E7B50,
    0x51E680; traced, `camera.md`, step 8); fixed in `follow::turn_view`.
- [x] **P1.8** Melee (`rac_hero/src/controller/melee.rs`, the profile's `melee` section,
  and the crate `rac_combat`): the wrench combo, jump attack, throw, grind attack and the
  clank off walls, for UYA and DL (`dl.ron`'s own numbers and wrench line), with
  hurtboxes, hit points, one hit per attack, knockback and flash. The demo has a melee
  yard and shows clanks as sparks.
  - Not modelled: DL's flail, shuriken, glove and magnetic attacks (with P1.9), UYA's
    states 77 and 114, and the giant-Clank mode (`hero-controller.md`, "Not read yet").
  - UYA's grind can't be traced: a rail written into Veldin's empty rail list crashes
    PCSX2 two frames into the grind. The grind attack's wrench line is computed from the
    disc's animations instead. The method reproduces the traced combo
    (`hero-controller.md`, "The grind attack").
  - DL's attack frames don't creep through a blend, and its lunges test the frame before
    its advance. Both were found in the traces (`melee.blend_creeps`, and DL's lunge
    frames half a frame on).
  - One trace of DL's double-jump attack started later than the code's frame-16 rule
    gives; still open (`hero-controller.md`, "Deadlocked's melee").
- [~] **P1.9** Weapons framework (`rac_combat`), following RE.7:
  - Done 2026-10-23: with a UYA disc the arms hold each gun by its hold (one hand, both,
    or none for the whip and gloves), from UYA's hold classes 1 and 2 laid over the
    body's moves with the game's weights, rests and the flips' lift
    (`hero_anims::hold_animations`, `rac_rig`'s `HoldLayer`; `weapons.md`, "Holding a
    gun"); the whip's handle at 0.3 of its class's size, as its update draws it. Seen
    on Xvfb: the two-handed guns held up in both hands, the N60 in one, the whip's
    handle in the fist.
  - UYA's hold classes have no skeleton (+0x14 and +0x18 are 0), which `MobyClass` took
    for a bad bind; it now reads such a class with no joints (`moby-format.md`).
  - Done 2026-10-22: the agents' search (`aim::level_search`, claims), states, walker and
    gun (`ShotKind::Slug`, UYA's small bullet, and the shared `AgentBook`); the Nitro
    Eruptor's 15 small bullets; `SmallMissile`, shared by the Decimator and the Agents of
    Dread; the guns' own sequences on the held gun (`rac_rig::GunSkeleton`,
    `gun_anim.rs`, recoils by `openrac`'s `gun_models::uya_motion`); the whip's rope and
    the shield on the rig (`rope.rs`, placeholder looks). Seen running on Xvfb: the disc
    guns skinned in his hand, the rope trailing and pulling out, the shield round him.
  - Done 2026-10-22: the Flux Rifle's scoped beam, piercing from V2 (`ShotKind::Beam`);
    the Annihilator's mines (V2–V4) and seekers (V5); shock arcs only to enemies in
    sight, striking 10 ticks on (`Arcs`); the view's aim point within 30 u; the raised
    gun arm (`rac_hero::GunArm`, which also picks the arms-only firing animation) and the
    gun's aim point (`GunHand::target`), which turns his head and upper body through
    `rac_rig`'s joint springs (`springs.rs`).
  - Next step: the whip's links and the shield from their disc classes with P3.18.
  - Done 2026-10-22: all twenty of UYA's guns (`weapons.md`, "In the kit", "The other
    fourteen"): their RON entries, a fire mode each in `weapons/{seekers,thrown,grab,
    hand}.rs`, enemies-only searches (`Hostile`), `Affliction`s (infected, duck,
    swallowed, sucked) as `Afflicted` messages, the Shield Charger's `rac_hero::Shield`,
    the Plasma Whip's `WhipHand` swinging the wrench's moves on the fire button, and the
    holo shields' `block_shot`. The demo's enemies are `Hostile`, fight each other while
    infected (the demo's own rules: `rac_demo::FoeHits`), are gone while a duck, and have
    their shots stopped by holo shields; the HUD shows every gun and the Suck Cannon's
    count.
  - Done 2026-10-22: the guns follow the game's code where it is read: the fourteen
    guns' disc models; the Hydra's spit and bolts, the RY3NO's missiles, the Suck
    Cannon's shot, the Holo Shield's drain, the agents' ends, the Quantum Whip's cut, the
    Qwack-O-Ray's ducks and eggs, and the V5 variants (`weapons.md`, "In the kit", which
    lists what is still [placeholder]).
  - Done 2026-10-21: the gun on the rig (`weapons.md`, "In the kit", "On the rig"):
    `GunHand` names the gun, its `GunPose` and shots; the standing firing sequences and
    the crouched arms layer play from the disc; guns sit in the fist (`RigGuns`,
    `model::gun_grip`, checked against UYA's firing poses); the open model aims
    procedurally. The quick-select ring (`weapons/ring.rs`), on placeholder rules.
  - Done 2026-10-22: with a UYA disc he holds the guns' own models from the gadget WAD
    (`openrac/src/gun_models.rs`), each level's: V2–V4 show more of the class's bangles,
    V5 on another class (`moby-format.md`, "The gadget WAD"). Baked at rest; the guns'
    own draw, idle and recoil sequences aren't played yet.
  - Done 2026-10-22: the ring follows UYA's rules with QUICK SELECT PAUSE off (so co-op
    stays in step): it opens on the press, holds his input after 15 ticks, picks after
    the hold check and the stick's lock-out, steps by the d-pad's neighbour table, shows
    the second ring with R1 and chooses as it closes (`weapons.md`, "In the kit").
  - UYA's rules and numbers are in `docs/research/weapons.md`; what the kit has of them,
    and its placeholders, in its "In the kit".
  - Done: `rac_combat::weapons`, with UYA's six read guns in RON
    (`assets/uya_weapons.ron`): every level's damage, ammo and XP, the mods' levels, and
    the five fire modes (hitscan, projectile, lobbed, stream, homing) with their shots, so
    the five archetypes are the N60, Nitro Launcher, Lava Gun, Annihilator and Flux Rifle.
    The `Arsenal` owns, levels and switches them (the quick switch, next and previous);
    the gun aim assist and the view's aim point; the damage queue, shock, acid and XP from
    kills; `GunEvent`s, which carry each gun's effect names for sounds and visuals. In
    `rac_hero`, `GunHand` and what it changes in his moves, lock-on by the gun's mod and
    the strafing a gun forces. The demo gives every gun at V3 and draws its shots.
  - A Shock Blaster at V2 or up drawn by holding fire used its 20 tries before it could
    fire; the hold counts from when it can, as the kit's own reading.
  - Not done: DL's guns and mods (RE.7), beyond `OwnedGun::extra_mods`.
- [~] **P1.10** Pickups and economy (`rac_combat`): magnetised bolts; crates (bolt, ammo,
  nanotech, explosive); health; a vendor stub that sells ammo.
  - Done 2026-10-21 (`rac_combat::pickups`, `assets/pickups.ron`): the drops moved here;
    ammo pickups, bolt and ammo crates and a vendor's pad (`weapons.md`, "In the kit",
    "Pickups"). Rounds and prices are the game's.
  - Done 2026-10-22: UYA's rules ("Ammo pickups, crates and the vendor"): the 8.7 by 3 u
    reach; ammo taken before it flies, on landing, capped, never expiring; ammo crates'
    1 or 2 pickups for one gun not full, never the Flux Rifle; bolt crates' bolts in
    coins; enemies drop no ammo.
  - Not done: nanotech, explosive and doubler crates; the vendor's menu; UYA's cap of a
    few coins a crate with the rest banked (`bolts_drop`'s bank isn't read).
  - Next step: the nanotech and explosive crates ("Ammo pickups, crates and the vendor").
  - Started by P1.11: enemies' bolt and nanotech drops, which pop out, bounce and fly to
    the hero, and the hero's `Bolts`, are in `rac_demo/src/drops.rs`. Move them here with
    the crates and the vendor.
- [x] **P1.11** Demo enemies (`rac_demo/src/enemy/`, `assets/enemies.ron`). They are the
  demo's own, not read from a game: every number is a placeholder sized against the
  hero's.
  - Two archetypes as RON definitions over one pure step (`enemy::brain::step`): a melee
    charger (runs in, winds up for 30 ticks, lunges and hits once, then recovers) and a
    ranged shooter (keeps 6–11 u away and strafes, aims, then fires three slow shots
    along its facing, so outrunning its turn dodges them).
  - States: idle, patrol, aggro, attack (wind-up, strike, recovery), hurt and death. They
    see in a cone with a line-of-sight ray and hear close by; they give up past a distance
    or a leash from home, and come back at home a while after dying.
  - Health and damage go through `rac_combat` (hurtbox, hit points, kill XP), so the
    wrench and guns hit them; they hurt the hero through `HurtHero`. Drops are bolts and
    nanotech (`drops.rs`, `assets/drops.ron`).
  - Infected by the Infector, one turns on the nearest other enemy it sees, which fights
    back until the infection ends; their hits on each other go through `FoeHits`.
  - An arena in the greybox level beside the melee yard: three chargers, two shooters, low
    cover and a pillar. Tests: `rac_demo/tests/enemies.rs`.
  - Enemies run after `SimSet::World` and before `SimSet::Hero`. Inside `World`, their
    reads of the combat messages would be unordered against the combat systems that
    write them, and the order could differ between peers.
  - To do: their sounds (P1.13), health and bolts on the HUD (P1.12), and tuning by play
    (in a scripted run, a hero who stood his ground and swung died in about 10 s).
- [~] **P1.12** UI and HUD, PC first (G5):
  - Started 2026-10-21 (`rac_demo::hud`, `assets/hud.ron`): health, bolts, the gun
    panel, the level-up notice and the ring, per player; seen running.
  - Done 2026-10-22: UYA's show-and-fade timings (`hud/clock.rs`; `weapons.md`, "In the
    kit", "HUD"): the gun, health and bolts shown for the game's frames after what shows
    them, the bolt tally and its roll, the low pulses, the next two recent guns, the
    ring's fade; tests on the clock and the view. Seen running on Xvfb.
  - Done 2026-10-22: the guns' aim indicators (`hud/sights.rs`; `weapons.md`, "In the
    kit", "Aim indicators"): the lock-on reticle with its health arc, each gun's
    crosshair, the target marks, the Hydra's triangles and the Flux scope, from each
    gun's `sight` and `Arsenal::sights`, on placeholder sprites keyed by effect-texture
    number. They replace the demo's lock-on torus. Seen running on Xvfb: the crosshairs
    and the scope.
  - Done 2026-10-22: the Flux scope's zoom (the weapons code asks the camera through
    `GunHand::zoom`; the camera springs its field of view, slows the free look by it and
    narrows the projection) and its ray, now in the weapons code, which walls and other
    targets block (`Sights::scope_on`). Seen running on Xvfb. State 0x74 and the world
    stopping have nothing in the kit to follow.
  - The Flux's zoom also lerps four floats at 0x1DF720 that nothing else in Veldin's
    code reads; what they drive is still open (`weapons.md`, "Aim indicators").
  - Done 2026-10-22: the sights' textures from the disc's effect table (`effects.md`,
    "Effect textures"; `crates/openrac/src/fx_textures.rs`), the rings and the scope
    drawn from their quarters as the game draws them, the scope over a 4:3 box. Seen
    running on Xvfb with the disc's textures and without.
  - Lock-on is gamepad-only (`Action::is_gamepad_only`), so under Xvfb the reticle can
    only be tested, not seen.
  - Not done: L3's show-all (no binding), the pause menu and settings; the scope drawn
    through the frame's alpha as the game does (with P3.18's blends).
  - Next step: the pause menu and settings.
  - xdotool's keys didn't reach the app under Xvfb (no window manager, so no focus);
    `xdotool windowfocus --sync` on the window first fixes it (CLAUDE.md, "Running the
    app off-screen").
  - HUD: health, bolts, ammo and weapon icon; plus the quick-select ring.
  - Pause menu.
  - Settings:
    - display: resolution, window mode, vsync, frame-rate cap, FOV, HUD scale, and
      preferred backend (DX12/Vulkan);
    - volumes;
    - controls, including rebinding and mouse sensitivity.
- [ ] **P1.13** Placeholder sound effects and music (CC0) through `bevy_audio`, triggered by
  events.
- [ ] **P1.14** Platform checks:
  - Windows: pick the default backend. Today wgpu prefers Vulkan over DX12 whenever both
    exist. Test both, then set `WgpuSettings::backends` accordingly and offer the other in
    settings.
  - Linux: Vulkan.
  - Gamepad hotplug on both.
  - Performance: at least 60 fps at 1080p on a mid-range GPU.
- [ ] **P1.15** Pipeline compilation:
  - Prewarm the pipelines behind a loading screen, so that synchronous compilation never
    causes stutter during gameplay.
  - After each NVIDIA driver or wgpu update, re-test Bevy's default parallel compilation:
    run `openrac --exit-after-frames 120` ten times. If it passes every time, drop the
    workaround.
- [~] **P1.16** Make the kit ready for mash-ups:
  - Done: the controller sees collision only through `rac_hero::CollisionWorld`; kit
    crates enable only the Bevy features they need; configuration comes from RON.
  - To do: make the system that gathers `Collider`s independent of the parry backend; a CI
    check on `cargo tree` that the kit depends on no disc, format or runtime crate;
    document the public API; `examples/hero_sandbox.rs`, the kit in a plain Bevy app.
- [~] **P1.17** The hero's rig and models (`rac_rig`, `openrac/src/hero_model.rs`;
  section 5, 2026-10-13 and 2026-10-15):
  - Done: the rig's skeleton is UYA's hero's 111 joints, face and fingers included, plus
    Clank's and the wrench's; poses and clips carry each joint's scale, which its children
    take on; models may be bound to joints turned at rest (`ModelData::turns`). The open
    model (gold for player 1, silver for player 2) has procedural poses for every hero
    state, with two-bone IK putting the wrench on the controller's swing line. The rig
    wears any model skinned to its skeleton (`RigModels`, `ModelData`, `SkeletonMap`), and
    a model may carry attachments: parts on skeletons of their own riding a rig joint
    without its scale, posed by clips of their own (`AttachmentData`, `BackAnimations`,
    `rac_rig/src/back.rs`). From the user's discs `openrac` imports 15 UYA suits, titled
    with the game's names, with Clank and the heli-pack on the back as attachments played
    by UYA's back rules (`moby-format.md`, "Clank and the heli-pack on the hero's back"):
    Clank's arms, the rotors in the heli jump, glide and long jump, and his fidgets. Also
    UYA's 21 multiplayer skins with the wrench and without Clank (`MP_SKINS`), and 17 DL
    skins from a DL NTSC-U or Gladiator PAL disc, the five armors with their collar and
    helmet bangles. With a UYA disc the hero plays Ratchet's real animations, chosen and
    timed as UYA's code does (`rac_rig/src/play.rs`; `hero-controller.md`, "Idle
    fidgets", "Landings", "The walk's animation"): the walk's bands, rates and stride,
    fidgets, the hurt idle, each jump's air animation timed to its arc, landings and the
    run's skid. Chosen with `--model` or the menus' Model button.
  - Read, not yet worn: DL's wrench (class 0x23FA, `Core::read_dl`,
    `Core::moby_textures`).
  - Found not needed: the third GS texture slot is the ship's paint job.
  - To do: DL's wrench on the skins in its sequence 1 (DL classes' sequences don't read
    as UYA's do yet: read them first); firing poses (P1.9; DL's gadget table names each
    weapon's firing sequence: `moby-format.md`, "DL's skins"); for the back, Clank's
    blink and glow (UYA 0x4DCC10) and the head look's hold on it, which the rig has no
    head look for yet.
  - UYA's Clank had no arms and no propellers: the heli-pack (class 0x25F), long taken
    for Clank's "pack form", is his arms and the rotors, and wasn't worn. Fixed by wearing
    it and Clank as attachments; UYA's level textures now read as DL's
    (`Core::read_uya` reads the GS RAM image) for the pack's.
  - `wrench_points` had been measured on a bake that ignored the wrench's rest turn (P3.7).
    It is now (0.47, −0.12, −0.87), forward from the fist, which matches the game's
    screenshot. Every model's wrench and the procedural poses follow it.
  - Changing sides in a sideways strafe sometimes spun the body above the hips a full turn:
    the strafe's twist was slerped joint by joint towards a target half a turn away, and
    hips and chest could take the same way round. It is now one angle eased beside the pose
    (`pose::Animator::ease`); UYA's own two twist springs (`hero-controller.md`,
    "Strafing") aren't used yet.
- [ ] **P1.18** Direct-IP co-op in the demo (`rac_net`), the first G4 milestone:
  - Host and join by `IP:port`, or by swapping join codes, over UDP.
  - Port mapping (UPnP IGD, NAT-PMP, PCP) and hole punching (D8).
  - A handshake checking the build and settings.
  - A `ggrs` session with rollback and configurable input delay.
  - Snapshot and restore of the demo's simulation.
  - Desync detection with checksums every tick.
  - A second hero in player-2 colours.
  - Disconnect handling.
  - Tests with simulated latency and packet loss, then real runs over LAN and over the
    internet, including behind NATs.
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

### Phase 2: Disc and data layer

Crates: `rac_iso`, `rac_formats` and `rac_tool`. The boot programs unpack (P2.5), so only
level code still needs savestates or Wrench, until P2.6 extracts the overlays.

- [x] **P2.1** `rac_iso` reads ISO9660 over `Read + Seek` (PVD, directories, file extents)
  with a raw sector API, never loading a whole image.
- [x] **P2.2** Disc identification (`rac_iso::Release`; `disc-layout.md`,
  "Identification"): a table of the eight releases (game, region, title, serial, version,
  boot executable and its SHA-1, TOC sector, packed or not), and `Release::identify`,
  which reads `SYSTEM.CNF` and hashes the boot executable. Unknown discs get an error
  saying why, with the SHA-1 of an unknown build of a known game. `openrac` finds its discs
  with it, and `openrac-tool identify` lists them. Every disc identifies.
  - `SYSTEM.CNF`'s own sector was left out of the table: nothing reads it, and rebuilt
    images move it.
- [~] **P2.3** TOC parsers. Done: UYA's (`rac_formats::level::Toc`; `disc-layout.md`,
  "Level data"), and DL's and Gladiator's eight global headers (`Toc::read_dl_disc`).
  - To do: DL's level table (what follows its global headers isn't UYA's), with its
    missions; GC's at LBA 1001; R&C1's at LBA 1500 (the header, global ranges and
    `levels[19]`).
- [~] **P2.4** WAD codec. Done: the decompressor, written from UYA's own
  (`rac_formats::wad`), run on every WAD of Veldin's level data.
  - To do: run it on every WAD of every disc; fuzzing (`cargo-fuzz`) and property tests; a
    compressor later, for tests and mods.
- [x] **P2.5** Unpacker for the packed executables (R&C3, UYA, DL, Gladiator, and the
  network start-up `I5BOOTN.ELF`): `rac_formats::elf` reads the stub's WAD as the boot
  ELF's sections, in the overlays' record format, and writes them out as a plain ELF that
  Ghidra imports with `$gp` and section names; `openrac-tool unpack-elf`. Read in the
  stub's code and checked byte for byte against savestates (`disc-layout.md`, "Packed
  executables"). The boot program's `.text` is the title screen's, so level code waits
  for P2.6.
- [ ] **P2.6** Level overlay extraction: read the 16-byte `{dest, size, type, entry}`
  records and produce per-level code images (for reverse engineering and the recompiler).
  `rac_formats::elf::Program` already reads the records and writes ELFs (P2.5); what's
  left is finding each level's overlay on the disc and the loader that applies it.
- [~] **P2.7** `openrac-tool` CLI. Done: `moby` (a class from an EE memory image, or as a
  `.glb` into `cache/`), `hero`, `rails`, `cameras`, `unpack-elf` and `identify`.
  - To do: `toc`, `extract --out cache/<release>/` and `dump-overlay`.
- [ ] **P2.8** Cache layout and versioning:
  - Path: `cache/<serial>-<elfsha1[0..8]>/…`.
  - A manifest records the tool version and source hashes, so extraction can be incremental
    and repeatable.
- [ ] **P2.9** Validation:
  - Commit golden **checksums** (not data) of the decompressed outputs.
  - Tests that read real discs are gated on `OPENRAC_SOURCE_DIR`.
  - Cross-check against `wrenchbuild unpack` output on the local machine.
- **Done when:** all eight discs identify, every TOC entry is listed, every WAD decompresses,
  the packed ELFs unpack, and our output matches Wrench's wherever they can be compared.

### Phase 3: Asset formats, native level viewer and remaster renderer

Work order: UYA NTSC-U → R&C3 PAL → DL NTSC-U → Gladiator PAL → R&C1 → GC. Crates:
`rac_formats`, `rac_assets`, `rac_render`, `rac_audio`.

- [ ] **P3.1** ELF and overlay model, and a **catalogue of each game's VU microcode**
  (DVP overlays). The renderer work depends on it.
- [~] **P3.2** Textures. Done: the armor WAD's PIF2 images (8-bit, CSM1 palette, alpha
  0x80) → `rac_formats::texture` → Bevy `Image`.
  - To do: PSMT4 and RGBA16 CLUTs, GS swizzles and mips; the core header's texture tables
    and the GS RAM table (located, not read; `disc-layout.md`); a texture browser.
- [~] **P3.3** The level core header and the class tables. Done for UYA: the level header,
  data part, core header, moby class table and hero sequence table (`rac_formats::level`).
  - To do: the core header's other tables, and DL's.
- [ ] **P3.4** Tfrags (terrain, with LOD 0 first), **P3.5** ties (instanced, with
  per-instance vertex colours), **P3.6** shrubs.
- [~] **P3.7** Mobys (`rac_formats::moby`, `moby-format.md`). Done for UYA, from memory or
  the disc: the class header, skeleton, every keyframe, the complete mesh (vertex cache,
  one to three joints per vertex, normals, texture coordinates and switches) and metal
  packets; the armor WAD (`armor`). For DL: classes and the skin WAD
  (`MobyClass::read_dl`, `armor::{SkinTable, Skin}`).
  - To do: bangles, UYA's armor entries 35–55 (whole classes), DL's sequences, and
    mobys other than the hero as Bevy meshes and `AnimationClip`s.
  - UYA joints can be turned at rest (Clank's face, the whole wrench, the hero's eyes and
    legs). The skeleton record is the inverse bind as four VU0 columns, but we read −t
    as the rest position and left the turn out, which threw Clank's eyes off his face.
    Fixed: `Joint::inverse_bind` and `rest_rotation`, poses as turns from rest, and the
    bake applies keyframe scales (`moby-format.md`, "Skeleton").
  - Open: where the game multiplies by the inverse bind (not found in code); whether
    children inherit a joint's scale; classes whose offsets don't fit their binds (0x189B,
    0x18AD, …); DL armors' joint 44.
- [ ] **P3.8** Sky: shells, and rotation converted from per-frame to per-second using the
  region's refresh rate.
- [ ] **P3.9** Collision: the octant grid and triangle/quad faces with their type byte; a
  debug overlay; **colliders for the hero kit**.
- [~] **P3.10** Gameplay data. Done: UYA's rails, grind and zip line
  (`rac_formats::gameplay`, `gameplay-format.md`), checked against the game's own list on
  Florana; UYA's cameras, camera volumes (moby instances of class 2329 with their pvars),
  paths, cuboids, spheres, cylinders and the camera collision list, checked against
  five savestates (`openrac-tool cameras`).
  - To do: level settings; tie and shrub instances, and the per-game moby record sizes;
    lights, sound instances and areas; DL's gameplay files, its missions and art
    instances.
- [ ] **P3.11** The **classic** render path, PS2-faithful, used for validation and for
  classic mode, with shaders in WESL:
  - Vertex colour modulation (0x80 = 1.0, so 2× modulate).
  - Fog, alpha test, the GS blend modes and filtering.
  - Then the GS quirks listed in engine-notes: bloom, partial texture invalidation, and
    accurate alpha.
- [ ] **P3.12** Level viewer mode:
  - Pick a release and level.
  - Free camera.
  - Layer toggles: tfrag, tie, shrub, moby, collision, volumes, paths.
  - Instance inspector, with `bevy_egui` 0.43 (which supports 0.20).
  - A switch between the classic and remaster paths.
- [ ] **P3.13** Audio:
  - VAG ADPCM decoding and streaming (music, speech).
  - **Port OpenGOAL's 989snd (ISC) to Rust** for the sound banks.
  - An approximation of the SPU reverb.
- [ ] **P3.14** Video:
  - Demultiplex PSS (MPEG-2 plus SPU ADPCM per language).
  - Choose a decoder: an `ffmpeg` feature flag or a pure-Rust MPEG-2 decoder.
  - Play it back as a Bevy texture.
- [ ] **P3.15** **Milestone "walkabout":** the hero kit runs through a real level using its
  real collision, wearing the disc's hero model. No game logic yet.
- [ ] **P3.16** R&C1 and GC formats, from their NTSC-U and PAL discs:
  - Evaluate ReRAC's ISC format crates for reuse with attribution (this was P0.8).
  - Add the R&C1 and GC variants of the formats.
- [ ] **P3.17** The **remaster** render path (G5), which is the default:
  - HDR with tonemapping and bloom, dynamic sun shadows and ambient occlusion.
  - Longer LOD and draw distances.
  - Anisotropic filtering and MSAA/TAA.
  - Field of view and culling that are correct in widescreen.
  - Uncapped frame rates through interpolation.
  - Support for user-supplied high-resolution texture packs.
- [ ] **P3.18** The guns' **effects from the disc** (`effects.md`, "Importing them"):
  - Read the level's effect and particle textures and particle definitions from its core
    (`rac_formats`), and the shot classes' meshes (`Core::moby_class`).
  - An ISO-free effects layer in the kit: billboards, beams, strips, trails, rings, point
    lights, camera shake, screen flashes and particles with the game's motion, in the
    four blends that matter; placeholders without a disc.
  - Each gun's recipes, read gun by gun from its code, keyed by the effect names
    `GunEvent`s carry; the sounds with P3.13.
  - Done 2026-10-22: the effect textures (`Core::fx_textures`), and the aim indicators'
    into `rac_demo::hud::SightImages` by their numbers.
  - Not done: the particle textures and definitions, the effects layer, the recipes, and
    the whip's links and the shield from their classes. Next step: the whip and shield
    from their classes (`weapons.md`, "Not read yet").
- **Done when:** every level of each finished release renders correctly in both paths:
  textures and vertex colours match in classic mode, and the collision overlay lines up.
  Check this against PCSX2 screenshots, kept locally and never committed.

### Phase 4: Runtime spike, static recompilation plus HLE (the gate for G2)

**Goal:** UYA NTSC-U's unpacked game ELF plus one level overlay runs natively far enough to
render that level through our renderer.

- [ ] **P4.1** R5900 decoder in `rac_recomp`, using rabbitizer (Rust) or ico-recomp's
  `r5900-decode` (MIT). It must cover:
  - the integer instructions and 128-bit MMI;
  - the COP1 FPU with PS2 semantics (no NaN, infinity or denormals; clamping);
  - COP2 VU0 macro mode and the needed subset of COP0;
  - branch delay slots and branch-likely instructions.

  The generated code must be deterministic: the PS2's floating-point semantics are
  implemented explicitly rather than inherited from the host.
- [ ] **P4.2** Function discovery and control flow:
  - Heuristics plus the RE.2 symbols, and jump tables.
  - Indirect calls go through an address → function table.
  - Each overlay is its own unit, keyed by (release, overlay). Functions that are identical
    across overlays are deduplicated by hash, since DL's overlays share 85–97% of their
    code.
- [ ] **P4.3** Backend: Cranelift JIT for development, and ahead-of-time compilation
  (`cranelift-object`) at **install time**, with the output cached in the data directory.
- [ ] **P4.4** `rac_runtime`:
  - Memory: 32 MB RAM, the scratchpad, and stubs for memory-mapped I/O.
  - The EE kernel syscalls, emulated at a high level: threads, semaphores, interrupt
    handlers, alarms.
  - Interrupts are delivered at safe points.
  - Fast snapshot and restore of the whole runtime state. Co-op resync, debugging and maybe
    rollback (P6.2) all need it.
- [ ] **P4.5** IOP services emulated at the SIF RPC level, using Play! (BSD) and PS2SDK as
  references (RE.12). CD reads go to `rac_iso`; mc to save files; pad to `rac_input`;
  989snd to `rac_audio`.
- [ ] **P4.6** Render bridge:
  - Intercept the submission of VU1 chains and translate each known VU1 program into native
    draw calls, as OpenGOAL does with its buckets (RE.11).
  - Reimplement the VU0 moby-skinning helper natively.
- [ ] **P4.7** Hook mechanism: a table mapping (release, address) → a Rust function,
  generated from RE.2. It serves four purposes: HLE of library functions (memcpy, printf,
  …), engine services, the native hero, and the progressive rewrites.
- [ ] **P4.8** Differential harness: compare RAM and hero state against PCSX2 at
  checkpoints, using PINE (RE.5).
- **Gate:** if the UYA level boots and renders within the spike's budget, adopt section 4.1
  for G2. If not, write an ADR and fall back to rewriting each game's core natively, with
  the decompilations as the specification.

### Phase 5: Full games, one release at a time

Order: section 4.4.

- [ ] **P5.1** Boot → title screen → menus, with options and language selection (PAL discs
  carry 5 languages).
- [ ] **P5.2** Saves: memory card → local save files. Import PCSX2 memory-card images.
  Support the cross-game save bonuses.
- [ ] **P5.3** Every level, mission, boss, vendor and minigame (races, ship sections, Clank
  and Qwark sections, Giant Clank, arenas, vehicles).
- [ ] **P5.4** Cutscenes (in-engine scenes and FMVs), music and speech.
- [ ] **P5.5** Remaster features (G5), **all on by default**, with classic mode available:
  - widescreen and ultrawide, with HUD layouts to match;
  - resolution scaling;
  - high frame rates (a 60 Hz simulation plus interpolation);
  - MSAA/TAA;
  - mouse camera and aiming, and FOV;
  - complete PC settings menus;
  - rebinding everywhere.
- [ ] **P5.6** Compatibility tracker at `docs/compat.md`, listing each release and level
  with its status.
- [ ] **P5.7** Bring the shared hero controller to R&C1 and GC (D2, D7):
  - The native hero replaces the recompiled one, with a movement profile read from that
    game's own code (RE.3 for R&C1 and GC).
  - What carries across is the controls and the moves that need no item. Each game keeps
    its own movement values and its own abilities, gated by its own items (section 5,
    2026-10-12): no charge boots in R&C1.
  - Fix individual levels where the new moves break assumptions: sequence breaks, triggers,
    camera volumes.
  - Offer the classic hero as a single-player option (D7).
- **Done when (for each release):** it can be finished start to finish, including 100%
  completion, with no softlocks, and its behaviour matches PCSX2 in the spot checks.

### Phase 6: Direct-IP co-op in every game (G4)

Mechanics come from Gladiator (RE.10). Netcode follows section 4.2.

- [ ] **P6.1** Co-op rules for every game, built on RE.10 and written in `docs/coop-design.md`:
  - What is shared and what is per player.
  - Respawn and revive.
  - What player 2 does in single-player-only sections (Clank, Giant Clank, ships,
    hoverboards, races, turrets).
  - Difficulty scaling.
- [ ] **P6.2** ADR on netcode for the full games:
  - Lockstep or rollback, given what a runtime snapshot costs.
  - Input delay.
  - Desync detection, and resync by transferring a snapshot.
  - Joining only at level load, or at any time.
  - Pause and menus during co-op.
  - Whose save the progress goes to.
- [ ] **P6.3** Two heroes in the runtime:
  - The native hero subsystem, with two heroes, replaces the recompiled hero.
  - Enemy targeting and every other "the hero" reference are redirected through hooks.
  - Each player has their own camera and HUD, and each peer renders its own view.
- [ ] **P6.4** Sessions inside the full game:
  - Host and join from the main menu.
  - A handshake checking the release (by boot-ELF hash), the build and the settings.
  - Synchronised level loads.
  - Disconnect and reconnect.
- [ ] **P6.5** Per-game adapters, applied in the order UYA → DL → R&C1 → GC:
  - A co-op compatibility list for each release and level.
  - Difficulty options.
- [ ] **P6.6** Local split-screen (D6), built on the same foundation: Bevy viewports and two
  local input sources.
- [ ] **P6.7** Network quality:
  - Tests with simulated latency, packet loss and jitter.
  - Long soak tests that look for desyncs.
  - Target: playable at 100 ms round trip with at most 3 frames of input delay.

### Phase 7: Release

- [ ] **P7.1** Windows package: MSVC build, zip or installer. It uses the default backend
  chosen in P1.14 and offers the other as an option.
- [ ] **P7.2** Linux package: tarball and AppImage or Flatpak, using Vulkan.
- [ ] **P7.3** First-run flow:
  - Choose the ISOs (either `./source` or a file picker).
  - Identify them.
  - Extract and recompile, with a progress UI.
- [ ] **P7.4** Polish:
  - Controller glyphs, accessibility options and crash logs.
  - Networking help: firewall, and a forwarded port for when mapping and hole punching
    fail (D8).
  - Possibly mod support.
- [ ] **P7.5** Legal review of disclaimers and branding (trademark use in the project
  name).

## 7. Cross-cutting

- **Testing:** unit tests on synthetic data; disc tests read `OPENRAC_SOURCE_DIR`, are
  `#[ignore]`d with a reason and run with `cargo test -- --ignored`; golden checksums;
  feel-matching tests (RE.6); determinism replays (P1.19); differential tests against
  PCSX2 (Phase 4 onward); visual comparisons, run locally only.
- **CI** (P0.7, P0.12): fmt, clippy with `-D warnings`, tests, Windows and Linux builds,
  the data-hygiene guard, and later the ISO-free kit check (P1.16) and cross-OS
  determinism hashes. Disc tests never run in CI.
- **Determinism** is a requirement because of co-op: a fixed 60 Hz simulation, a seeded
  RNG per level, `libm` maths, stable ordering, input record and replay, and state
  checksums.
- **Performance budgets:** the demo at 60 fps or better at 1080p on a mid-range GPU;
  recompiled game logic under 4 ms a frame; level loads under 2 s from a warm cache; co-op
  playable at a 100 ms round trip.
- **Diagnostics:** `tracing` through Bevy's logging (`RUST_LOG`), the `bevy/trace_tracy`
  feature for profiling, an egui debug overlay, and network stats (round trip, rollbacks,
  desyncs).
- **Documentation:** findings in `docs/research/`; our own format specs, in our words and
  citing sources, in `docs/formats/`; architecture decision records for big choices (the
  Phase 4 gate, the netcode model) in `docs/adr/`.

## 8. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| The controller doesn't feel like the original | G1 fails at its core | Reverse engineering from day one (RE.3, RE.4); measured traces (RE.5); tolerance tests (RE.6). |
| Determinism breaks between machines or operating systems | Co-op desyncs | `libm` math, no reliance on iteration order, replay tests across operating systems in CI, per-tick checksums, resync by snapshot. |
| Lockstep feels laggy in a fast shooter | Poor co-op | Low input delay; rollback for the demo; reconsider rollback for the full games once runtime snapshots are fast (P6.2). |
| Backported moves break older levels (sequence breaks, triggers, cameras) | Bugs and softlocks | Per-level fixes, the classic hero option (D7), and the compatibility list (P5.6). |
| Legal problems in a public repo (incompatible or unlicensed code, derived data, trademarks) | Takedown or relicensing | The CLAUDE.md rules (GPL-3.0-or-later; reused code keeps its notices; GPL-2.0-only and unlicensed code reference only), the CI data guard (P0.12), generating everything at install, and P7.5. |
| VU1 and GS semantics (bloom, render-to-texture, texture paging) | Visual bugs | Native renderers per renderer type, OpenGOAL as precedent, the PCSX2 GameDB fixes as a map of hard spots, PCSX2 comparisons. |
| Recompiler complexity (overlays, indirect jumps, asynchronous VU0, non-IEEE floats) | G2 slips | The Phase 4 spike and gate; reuse rabbitizer and ico-recomp; differential testing. |
| IOP and sound gaps (989snd variants, "stash", DL's streamer) | Missing audio | Port OpenGOAL's 989snd; emulate at the RPC level (RE.12). |
| PAL data tuned for 50 Hz (R&C3 PAL, Gladiator PAL) | Wrong speeds | NTSC-U as the primary reference; RE.8; per-release adapters. |
| Bevy churn (a breaking release roughly every 4 months; ecosystem lag) | Upgrade cost | Pin versions; upgrade in one dedicated change; keep the format, recompiler, runtime and netcode cores free of Bevy. |
| The kit's API churns and breaks mash-ups | Reuse suffers | A small documented API, the sandbox example (P1.16), and semantic versioning once it stabilises. |
| NVIDIA driver crash when compiling pipelines in parallel | Crashes at startup | Synchronous compilation (done); prewarm pipelines; re-test regularly (P1.15). |
| Scope: 6 discs, 4 engines, online co-op and a remaster | Never finishing | Strict phase gates, UYA and DL first, and the demo first for visible progress. |
| Continuity across many AI sessions | Lost context | Keep §0 and each task's state current, write findings into `docs/research/` as they are checked. |

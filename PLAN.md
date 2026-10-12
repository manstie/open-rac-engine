# Open Ratchet & Clank Engine: project plan

This is the source of truth for **scope, strategy, status and next steps**, written for the
project owner and for every Claude session that picks this work up.

- Goals are in section 1. They began as the original README's preliminary plan and were
  refined by the owner's decisions (section 5). [README.md](README.md) now summarises them
  for visitors on GitHub, so keep it in step when the goals change.
- Working rules and commands are in [CLAUDE.md](CLAUDE.md).
- Research findings are in [docs/research/](docs/research/):
  - [disc-layout.md](docs/research/disc-layout.md): our own inspection of the six discs.
  - [engine-notes.md](docs/research/engine-notes.md): what is known about the engine.
  - [prior-art.md](docs/research/prior-art.md): other projects, tools and their licences.

Task IDs (for example `P2.4` or `RE.3`) are stable, so refer to them in commits and session
logs. Status markers: `[x]` done, `[~]` in progress, `[ ]` not started, `[-]` dropped or
moved (say why).

---

## 0. Status (updated 2026-10-12)

**Phase 0 (foundations) is done apart from CI's first run (P0.7). The reverse-engineering
track has the hero's movement code: constants and models are read from the game itself.**
- The Linux build dependencies are installed. The workspace builds and runs natively: a
  Bevy 0.20 window showing a greybox scene.
- The owner has decided D1–D5 (section 5):
  - an independent project with an ISO-free, reusable hero kit;
  - R&C3 and Gladiator first;
  - direct-IP two-player co-op in every game;
  - a full PC remaster;
  - a public repository.
- CI and the data-hygiene guard are written (P0.7, P0.12). CI runs for the first time on
  the next push.
- **Reverse engineering:**
  - The owner captured savestates and input recordings of the **PAL** releases (R&C3,
    Gladiator).
  - `tools/pine-probe` replays them headlessly and logs the hero every frame (RE.5).
  - Where the hero lives in memory is in `re/<serial>/hero.toml` (RE.2).
  - The first measured movement constants are in `docs/research/hero-controller.md`
    (RE.3): run speed and acceleration, the variable-height jump, double jump, jump attack
    and falling.
  - **2026-10-12: static analysis (RE.3).** The hero code was found in Ghidra in all
    four builds:
    - the PAL builds from the savestates of the owner's recordings;
    - UYA and DL NTSC-U from savestates taken by booting a new game headlessly
      (`tools/pine-probe/boot.sh`).

    Results:
    - The input, ground and jump models, and every jump-type move's parameters, are in
      `hero-controller.md`. They reproduce the PAL traces frame for frame.
    - The addresses are in `re/<serial>/symbols.toml`.
    - **RE.8 is answered:** both engines are tuned in per-second units at 60 Hz, and each
      PAL release is a conversion that keeps the real-time behaviour. So the kit uses the
      NTSC values at 60 Hz.

**Next actions, in order:**
1. **Owner, when convenient:** savestates or recordings of the moves that need a
   particular place (wall jump, ledges, grinding, swimming, charge boots), and a half-tilt
   stick recording on an NTSC disc (`docs/re-setup.md`, "Still wanted").
   - Most jump moves are now verified on UYA NTSC with our own scripted traces.
2. Push the branch so CI runs, and fix anything Windows-specific it finds (P0.7).
3. Start the kit's foundations, P1.1–P1.4: app flow, simulation clock, input and a greybox
   level.
   - The hero controller (P1.5) can then start from `hero-controller.md`, using the NTSC
     values at 60 Hz.
   - It is tuned against the traces through RE.6.
4. RE.3, continued:
   - wall detection, ledge grab, swimming and grinding, in code;
   - DL NTSC traces, by scripted play as for UYA.

   Then the camera (RE.4).
5. RE.2: seed the knowledge base from libuya and libdl (MIT).
6. In parallel, at lower priority: the disc layer, P2.1–P2.5.

---

## 1. Goals

| ID | Goal | Done when |
|---|---|---|
| **G1** | Rewrite the engine of Ratchet & Clank (2002) through Ratchet: Gladiator (2005) in Rust with Bevy. The first deliverable is an **ISO-free hero kit**: the R&C3/Gladiator player controller, camera and weapons on a **placeholder rig built from primitive shapes**, packaged as Bevy plugins that other projects and game mash-ups can reuse. It comes with a standalone demo level. | The demo is playable for 5–10 minutes on Windows and Linux with no game data. The kit crates work in an unrelated Bevy app (P1.16) and depend on no disc crate. Measured movement matches the originals within agreed tolerances (RE.6). |
| **G2** | Run each game **natively** on Windows and Linux (DX12 and Vulkan), with users supplying their own ISOs in `./source`. The newer R&C3/Gladiator mechanics are backported to the older games. | Each supported release can be completed start to finish (Phase 5), with no emulator in the shipped path. |
| **G3** | Controller and keyboard+mouse support, with **key remapping**. | Every action can be remapped and the bindings persist. Gamepad and KB+M both work in menus and gameplay. |
| **G4** | **Direct-IP two-player online co-op in every game**, with mechanics derived from Ratchet: Gladiator. | Two PCs connect by IP address and play a campaign together in every supported game (Phase 6). The demo gets co-op first (P1.18). |
| **G5** | **A full PC remaster by default** (D4): widescreen and ultrawide, any resolution, high frame rates over a 60 Hz simulation, modern anti-aliasing and post-processing, mouse-first controls and complete settings menus. | Every enhancement is on by default and can be switched off individually. A "classic" mode reproduces the original presentation for comparison. |

**Non-goals for now:**
- Competitive online multiplayer, matchmaking or hosted servers (Horizon already revives UYA
  and DL online). Co-op connects by direct IP only.
- The PS3, PS4 or PS5 versions.
- A pixel-exact emulator.
- Shipping any game data.
- Contributing to other projects' repositories (D5).
- macOS, mobile or web. They aren't ruled out, just not targeted.

## 2. Ground rules

1. **Legal.** Full rules are in CLAUDE.md. In short:
   - Never commit game data, anything extracted from a disc, decompiled game code, or Sony
     SDK material.
   - The repo is MIT, so GPL code is reference only.
   - Anything derived from a disc is generated on the user's machine, from their own disc,
     into a local cache.
   - The repository is public, so CI also blocks data files (P0.12).
2. **Engineering:**
   - Gameplay runs on a **fixed 60 Hz simulation** and rendering is interpolated.
   - **The simulation is deterministic.** The same inputs must produce bit-identical state
     on Windows and Linux, because lockstep co-op depends on it. In simulation code:
     - use `bevy::math::ops` with Bevy's `libm` feature instead of `f32::sin` and friends;
     - never depend on `HashMap`/`HashSet` iteration order;
     - keep entity ordering stable;
     - use one seeded RNG per level;
     - never read the wall clock.
   - **The hero kit is ISO-free.** `rac_input`, `rac_hero`, `rac_combat`, `rac_net` and
     `rac_demo` never depend on the disc, format or runtime crates. CI enforces this
     (P1.16).
   - Parsing and format crates **don't depend on Bevy**, so tools and tests stay fast and
     survive Bevy upgrades.
   - Unit tests use **synthetic data**. Tests that need a disc are gated (see section 7).
   - Libraries must not panic on bad input. Return typed errors.
   - Facts learned from the discs go in `docs/research/`, and decisions get a dated entry in
     section 5 or 9.

## 3. What we know (summary)

The details and evidence are in `docs/research/`.

- **Discs** (all six owned by the user). NTSC-U discs of R&C1 and GC will follow (D3).

  | Disc | Region | Role now |
  |---|---|---|
  | UYA SCUS-97353 | NTSC-U | **Primary reference for R&C3.** Packed ELF, 60 Hz, libuya, decompilation about 7% done. |
  | R&C3 SCES-52456 | PAL | The R&C3 PAL variant. libuya has a PAL build. |
  | DL SCUS-97465 | NTSC-U | **Primary reference for Gladiator.** Packed ELF; libdl has the hero structs and constants. Reference for co-op. |
  | Gladiator SCES-53285 | PAL | The Gladiator PAL variant. |
  | R&C1 SCES-50916 v2.00 | PAL | Later. rac1-decomp targets this exact build. |
  | GC SCES-51607 | PAL | Later. Plain ELF; its files are visible on the disc. |

  Except on GC PAL, the data is hidden from the filesystem and reached through each game's
  own table of contents:
  - R&C1's TOC is at LBA 1500.
  - The others' TOCs are at LBA 1001.
  - The data inside is compressed with Insomniac's "WAD" LZ codec.
- **Engine:**
  - It is C++, built with SN ProDG GCC 2.95.3.
  - A resident "core" is joined by **per-level code overlays**, each holding nearly the
    whole game program. They are dispatched through `lvl.vtbl`, `lvl.camvtbl` and
    `lvl.sndvtbl`.
  - Rendering is VIF → VU1 microcode → GS, and the terrain, tie and shrub renderers come
    from Naughty Dog's.
  - Audio is Sony's **989snd**, the same library family as Jak & Daxter's.
- **How much is shared** (this answers the open question in the original README):
  - **Formats are highly shared** across all four engine generations: one asset pipeline
    with per-game adapters will do.
  - **Gameplay code is not shared**: less than 1% of it is identical across games, and each
    game has 3.7–7 MB of distinct code. Only the engine core carries over within each family
    (R&C1 with GC, UYA with DL).
  - Hand-porting all gameplay code for four games is therefore not realistic. Game logic
    has to come from **automatic translation of each game's own code** (see section 4).
- **Prior art** (October 2026, all moving fast):
  - **ReRAC**: Rust and Bevy, ISC. A native R&C1 port. It is a reference for us, and its
    scope is narrower than ours (D1).
  - **OpenRAC**: GPL. Decompilations of all four games plus a planned C++ native port.
  - **Wrench**: GPL. The format reference, and it supports all six of the user's discs.
  - **Horizon**: MIT. Its headers for UYA and DL (`libuya`, `libdl`) are our starting
    point for the hero and weapons.
  - **OpenGOAL**: ISC. The closest architectural precedent, and its 989snd reimplementation
    is directly reusable.
  - **ico-recomp**: MIT, Rust. A PS2 static recompiler whose crates we can reuse.
  - **ggrs**: MIT/Apache. GGPO-style peer-to-peer netcode, independent of Bevy.

## 4. Strategy

### 4.1 Approach: the hybrid "native engine plus translated game code"

The original README asked whether to use the ISOs only as data or to translate the game
code when an ISO is first selected, depending on how much code is shared. **We do both, by
layer:**

| Layer | Source | Where it runs |
|---|---|---|
| Platform: window, input, audio out, GPU (DX12/Vulkan), sockets | Bevy, wgpu and ggrs | Native |
| Engine services: rendering (classic and remaster), audio engine, file and disc I/O, saves, input mapping | **Our Rust code** | Native |
| Assets: levels, models, textures, animation, collision, sound, video | The user's ISO, **extracted and converted on first run** into a local cache | Native data |
| Game logic: thousands of per-class update functions, missions, bosses, menus | The user's ISO, **statically recompiled on first run** (R5900 → Cranelift → native code, cached) | Native code, on top of our runtime, which emulates the hardware and IOP services at a high level (HLE) |
| Rewritten systems: the hero (R&C3/Gladiator mechanics, backported to the older games), camera, weapons, HUD, co-op and netcode | **Our Rust code (the hero kit)**, replacing recompiled functions through hooks | Native |

- **Why it fits:** formats are shared but code isn't. Recompiling scales to 4 engines × ~50
  overlays without hand-porting, and keeps the game logic faithful. Our own code provides
  the remastered rendering, the input, the hero, and co-op. No game code or data is ever
  distributed, and users don't need a C toolchain.
- **Progressive rewrite:** any recompiled function can be swapped for hand-written Rust,
  using the decompilations as the specification and the recompiled version as a
  differential-testing oracle.
- **Precedents:**
  - OpenGOAL: native renderers fed by the game's own logic.
  - N64Recomp and XenonRecomp: the user supplies the game, code is generated from it, and
    function hooks allow changes.
  - Dark Cloud's PC port: fixed 60 Hz logic with interpolated rendering.

**Alternatives we rejected:**
- Hand-rewriting all gameplay is too large, given how little code is shared.
- Embedding an emulator core isn't native, and the available cores are GPL.
- Compiling decompiled C would mean shipping game-derived code, and only R&C1's
  decompilation is mature.

**Phase 4 is the decision gate.** If the recompilation spike fails, we fall back to a
native rewrite of each game's core, using the decompilations as the specification.

### 4.2 Co-op and networking

- **Topology:** two peers. One hosts on a UDP port and the other joins by `IP:port`. There
  are no servers. UPnP port mapping (`igd-next`) is optional (D8).
- **Model:** like GGPO, both peers simulate the whole game deterministically, and only
  player inputs cross the network. The library is `ggrs`, which doesn't depend on Bevy.
  - **The demo** (Phase 1) uses **rollback** from the start. Its state is small and easy to
    snapshot, and this proves the netcode for the reusable kit.
  - **The full games** start with **delay-based lockstep**: `ggrs` with prediction turned
    off, plus a few frames of input delay. Rolling back recompiled game state would mean
    snapshotting about 32 MB of emulated memory plus our native state every frame. Rollback
    is adopted later if snapshots prove cheap enough (ADR in P6.2).
- **Safety nets:**
  - A handshake checks that both peers have the same app build, the same release (by
    boot-ELF hash) and the same simulation-affecting settings.
  - Checksums of the state every tick detect desyncs.
  - The host can resend a full snapshot to resync.
- **Mechanics:** taken from Gladiator's co-op (RE.10) and implemented once in the native
  hero and co-op layer, then applied to every game through per-game adapters (Phase 6).
- **Rendering:** each peer renders its own view, so online co-op needs no split-screen.
  Local split-screen is an optional extra on the same foundation (D6).

### 4.3 Architecture

All crates live in `crates/`. Libraries are prefixed `rac_*`; the binary is `openrac`.

```
ISO-free hero kit: reusable in any Bevy app; never depends on the game-layer crates
  rac_input    actions, bindings, remapping, per-tick input snapshots (G3)
  rac_hero     kinematic controller (R&C3/Gladiator feel), abilities, camera, placeholder rig
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
  rac_tool     CLI binary `openrac-tool`: identify, toc, extract, unpack-elf, dump-*

App
  openrac      launcher and game: the demo when no disc is present, full games when one is
```

### 4.4 Order of targets

| # | Release(s) | Why it is in this position |
|---|---|---|
| 1 | **UYA NTSC-U (SCUS-97353)**, then R&C3 PAL (SCES-52456) | The controller feel to match (D2). Both regions are owned. libuya (MIT) covers NTSC and PAL, and the decompilation is about 7% done, which supplies names for the boot path. |
| 2 | **DL NTSC-U (SCUS-97465)**, then Gladiator PAL (SCES-53285) | Same code family as UYA, with about 69% of the core shared. libdl (MIT) has the hero structs and constants. It holds the co-op implementation behind G4. |
| 3 | R&C1: NTSC-U SCUS-97199 (when provided) and PAL v2.00 (owned) | References: rac1-decomp (PAL, about 78% done) and Lombyte/ReRAC (NTSC). ReRAC's ISC format crates may be reused (P3.16). |
| 4 | GC: NTSC-U SCUS-97268 (when provided) and PAL (owned) | Same code family as R&C1. |

- NTSC-U builds are the primary development references, because they have 60 Hz data and
  the community's symbols. PAL releases are supported variants: they need their own
  address maps, languages and handling of data tuned for 50 Hz (RE.8).
- The hero kit (Phase 1) implements the UYA/DL move set. It is tuned against measurements
  of both games (RE.3–RE.6), and it is the hero that gets backported to R&C1 and GC.

## 5. Decisions

### Open, needing the owner

| ID | Question | Options | Recommendation |
|---|---|---|---|
| **D6** | Also offer local split-screen co-op, as Gladiator originally did? | Yes / no | **Yes, after direct-IP co-op works.** It reuses the same multi-hero foundation. |
| **D7** | In the older games, offer the original hero ("classic movement") as an option alongside the backported R&C3/Gladiator hero? | Yes / no | **Yes, for single-player only.** It comes almost free, because the recompiled original hero code is there anyway. Co-op always uses the native hero. |
| **D8** | How far beyond direct IP should connectivity go? | Direct IP only / plus UPnP / plus NAT hole punching or a relay | **Direct IP plus optional UPnP** (`igd-next`). No hosted services. |

### Made

| Date | Decision |
|---|---|
| 2026-10-11 | **D1:** An independent project, broader than ReRAC. It has an ISO-free, reusable hero kit (placeholder rigged shapes, usable in mash-ups) and direct-IP two-player co-op in every game, with mechanics derived from Ratchet: Gladiator. ReRAC and OpenRAC stay references; permissively licensed code may still be reused with attribution. |
| 2026-10-11 | **D2:** Match the player-controller feel of R&C3 and Gladiator, and backport their mechanics to the older games. UYA/R&C3 and DL/Gladiator come first (section 4.4). |
| 2026-10-11 | **D3:** NTSC-U R&C1 and GC discs will be provided later. Until then, work with R&C3/UYA and Gladiator/DL, which we have in both regions. |
| 2026-10-11 | **D4:** A full PC remaster by default (G5). A classic mode stays available for comparison. |
| 2026-10-11 | **D5:** The repository is public. No contributions to other repositories are planned. |
| 2026-10-11 | Reverse engineering starts in Phase 0, as a continuous track led by the hero controller, so that the kit can match the originals as closely as possible. |
| 2026-10-11 | Netcode: deterministic peer-to-peer in the GGPO style, via `ggrs`. The demo uses rollback; full games start with lockstep (section 4.2). Provisional until the ADR in P6.2. |
| 2026-10-11 | Rust pinned to **1.99.0** in `rust-toolchain.toml`. Bevy is **0.20** (released 2026-10-08). Virtual workspace in `crates/*`. |
| 2026-10-11 | Pipelines compile **synchronously** (`RenderPlugin::synchronous_pipeline_compilation`). Bevy's default parallel compilation crashed 8 out of 8 runs on NVIDIA driver 595.91.07 / RTX 4070 SUPER ("NVVM compilation failed: 3" → `DeviceLost` → segfault). Synchronous compilation gave 18 clean runs out of 18. Revisit under P1.15. |
| 2026-10-11 | Strategy: the hybrid in section 4.1, provisional until the Phase 4 gate. |
| 2026-10-11 | Reverse-engineering tools live in `tools/*` as workspace members and never ship. `pine-probe` has its own PINE client rather than the `pine-ipc` crate: the protocol is tiny, and we need raw batched reads with our own tear handling. |
| 2026-10-11 | Hero measurements come from replaying the owner's PCSX2 input recordings in a private, headless PCSX2 (Xvfb, the owner's emulation settings, copied memory cards), never from the owner's live instance. Measured PAL figures are recorded as per-frame values, because the NTSC relationship is unknown (RE.8). |
| 2026-10-12 | Hero constants come from **static analysis of the game code**, checked against the traces. Recordings of single moves are no longer needed to measure them, only to verify them. |
| 2026-10-12 | The hero kit's movement is specified **in per-second units with the NTSC-U values, simulated at 60 Hz**. RE.8 showed that NTSC-U is the native tuning and PAL a conversion. |
| 2026-10-12 | Ghidra scripts live in `tools/ghidra/` (Java), which is excluded from the Cargo workspace. Decompiled output stays in `cache/`. Only facts in our own words and `re/<serial>/symbols.toml` are committed. |
| 2026-10-12 | NTSC savestates for static analysis are made headlessly (`tools/pine-probe/boot.sh`: a new game driven with xdotool), using copies of the owner's memory cards and never their live instance. |

## 6. Roadmap

The phases run roughly in order. The **reverse-engineering track (RE)** starts in Phase 0
and runs alongside every phase; its first target is the hero controller.

Every task follows the definition of done in CLAUDE.md: tests pass, clippy and fmt are
clean, docs are updated, and the task's checkbox is ticked here.

### Phase 0: Foundations

- [x] **P0.1** Native toolchain: rustup with Rust pinned to 1.99.0 (`rust-toolchain.toml`).
- [x] **P0.2** Cargo workspace and the Bevy 0.20 app `crates/openrac`:
  - Bevy's recommended dev profiles and workspace lints.
  - A `--exit-after-frames N` smoke-test flag.
  - A `dev` feature that turns on dynamic linking.
- [x] **P0.3** Fix `source/.gitignore`. It said `.iso` instead of `*.iso`, so the 25 GB of
  ISOs were not ignored. It now ignores everything except itself, and the root
  `.gitignore` ignores `*.iso` and `/cache/` too.
- [x] **P0.4** First inspection of the six discs → `docs/research/disc-layout.md`.
- [x] **P0.5** Research into prior art and emulators → `docs/research/prior-art.md` and
  `docs/research/engine-notes.md`.
- [x] **P0.6** Work around the NVIDIA parallel pipeline compilation crash (decision in
  section 5).
- [~] **P0.7** CI with GitHub Actions on `ubuntu-latest` and `windows-latest`:
  - `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
    `cargo test --workspace`, and a build.
  - Cache with `Swatinem/rust-cache`, and install the apt dependencies on Linux.
  - Optionally, a Linux smoke run under `xvfb-run` with Mesa lavapipe:
    `openrac --exit-after-frames 60`.
  - 2026-10-11: `.github/workflows/ci.yml` written with everything above except the smoke
    run. Every step passes locally on Linux. Still to do: its first run on GitHub,
    especially Windows.
- [-] **P0.8** Evaluate ReRAC. Moved to P3.16: it only matters for R&C1, which is now later
  in the order (D1, D3).
- [x] **P0.9** Owner decisions D1–D5, recorded in section 5.
- [x] **P0.10** Check the six disc hashes against redump.org and note the result in
  `disc-layout.md`. Done 2026-10-11: all six whole-image SHA-1s match their redump entries.
- [x] **P0.11** `CONTRIBUTING.md`: the legal rules from CLAUDE.md, a short "how to build",
  and the definition of done. Done 2026-10-11.
- [x] **P0.12** Data-hygiene guard, because the repo is public:
  - A CI job, plus an optional pre-commit hook, rejects disc-derived file types: `*.iso`,
    `*.wad`, `*.elf`, `*.irx`, `*.vag`, `*.pss`, savestates and memory cards.
  - It also rejects any binary over 1 MB that isn't on an allowlist.
  - Done 2026-10-11: `tools/check-data-hygiene.sh`, the `data-hygiene` CI job, and
    `.githooks/pre-commit` (enable with `git config core.hooksPath .githooks`). It also
    bans `.bin`, input recordings, GS dumps and pine-probe traces. Checked on a staged
    `.p2s` file and on a 1.1 MB file.

### Track RE: Reverse engineering and the knowledge base (starts now, runs throughout)

**Order:** UYA NTSC-U and DL NTSC-U first, then their PAL variants (R&C3, Gladiator), then
R&C1 and GC.

**Getting unpacked code before P2.5 exists:** Ghidra can import PCSX2 savestates, which
contain the full memory image including the current level's overlay. `wrenchbuild unpack`
also works: using a GPL tool is fine, copying its code is not.

**Rules:**
- Raw dumps, savestates and telemetry traces stay in `/cache` or outside the repo.
- Only facts written by us are committed: names, addresses, layouts and derived constants.

**Tasks:**
- [x] **RE.1** Tools and setup, documented in `docs/re-setup.md`:
  - PCSX2 2.8+ with the debugger, PINE enabled, input recording and savestates.
  - Ghidra 12.1 with ghidra-emotionengine-reloaded.
  - A workflow for importing a savestate of each level.
  - Installed on 2026-10-11:
    - JDK 21, Ghidra 12.1.4 and extension v2.1.38. A headless ELF import was verified.
    - PCSX2 2.8.2 with PINE turned on.
  - The owner installed their BIOS and captured the PAL releases.
  - Savestate import: Ghidra can't open PCSX2's Zstandard `.p2s` files. Extracting
    `eeMemory.bin` with `pine-probe` and importing it as a raw binary works (7 s).
  - 2026-10-12: `tools/ghidra/` imports and analyses a savestate in one step: it sets
    `$gp` from the savestate's registers, finds the code regions and seeds functions. It
    also decompiles chosen functions into `cache/` and applies `symbols.toml` names.
    `docs/re-setup.md` has the workflow and its gotchas.
- [~] **RE.2** Machine-readable knowledge base, written by us:
  - Started 2026-10-11: `re/README.md` and `re/<serial>/hero.toml` for R&C3 PAL and
    Gladiator PAL. They hold the hero struct, its verified fields and a per-frame counter,
    and pine-probe reads them.
  - 2026-10-12:
    - `re/<serial>/symbols.toml` for all four UYA/DL-era builds, R&C3 and Gladiator PAL
      plus UYA and DL NTSC-U. Each symbol has a confidence and how it was found.
    - The format is in `re/README.md`. `pine-probe`'s tests check the files, and
      `tools/ghidra/ApplySymbols.java` applies them.
    - Both PAL `hero.toml` files gained the jump block, the stick, the state timer and
      the intended velocity, each checked against the traces.
    - libuya/libdl seeding, `structs.toml` and the NTSC `hero.toml` files (which need a
      trace for the frame counter) are still to do.
  - Files: `re/<release>/symbols.toml` (address, name, source, confidence),
    `re/<release>/structs.toml` and `re/<release>/memory-map.md`.
  - Seed UYA and DL NTSC-U from libuya and libdl (MIT, attributed).
  - Map the PAL builds by function hashing.
  - Generators produce a Ghidra import script and the Rust hook tables (P4.7).
- [~] **RE.3** **Hero controller, the first priority**, for UYA and DL:
  - Started 2026-10-11 with the PAL builds: `docs/research/hero-controller.md`, from
    four replayed recordings.
    - Measured: run speed and acceleration, stopping, the variable-height jump (take-off,
      thrust while held, ascent and descent gravity), double jump, crouch jump, jump
      attack, falling, input latency, and the state numbering.
    - Still to do: NTSC-U, static analysis, and the moves free play didn't cover.
  - 2026-10-12: static analysis in Ghidra of R&C3/Gladiator PAL and UYA/DL NTSC-U
    (`docs/research/hero-controller.md`, `re/<serial>/symbols.toml`).
    - **Found:** the update entry, the state update, the state-change routine (which
      fills each jump type's parameters) and the vertical jump function.
    - **Verified frame for frame against the PAL traces:** the stick pipeline (dead
      zone, 0.25 d-pad cut-off, magnitude), walking speed and acceleration, the
      target-height jump model, apex gravity, and the double-jump boost and its cap.
    - **Read from code but not yet seen in a trace:**
      - the turn spring and the idle braking;
      - the parameters of every jump-type state: long jump, flips, rocket, heli, charge,
        wall, water, ledge, grind and lava;
      - the heli thrust table;
      - the double jump's height-dependent cut.
    - **Still to do:** air control, the long-jump and flip horizontal speeds, glide,
      wall and ledge detection, swimming and grinding, the jump attack and the fall
      state.
  - 2026-10-12 (second pass, with no new recordings from the owner):
    - **Read in code:** air control, the flip and long-jump horizontal speeds, glide,
      the fall state, the jump attack, the wrench combo, get hit and the ledge states.
    - **Verified against the PAL traces:**
      - the walk turn spring, which has a setting-off and a moving set of values (724
        of 724 frames);
      - air control (246 of 256 frames; the rest hit a wall);
      - the strafe flip (Gladiator) and the jump attack.
    - **Our own UYA NTSC traces** (`boot.sh` plus scripted xdotool keys, with the pad
      recorded in the layout) verified the NTSC values of:
      - walking and stopping;
      - tapped and held jumps, the run jump and the double jump;
      - glide, the heli jump and its thrust, and the long jump;
      - strafe side and back flips, and the jump attack.
    - **New for the kit:** the long jump keeps its take-off speed with no air control,
      the stick release goes through the skid path, and the stop residual is 1.5 u/s.
    - **Still to do:** moves that need a particular place (wall jump, ledges, grinding,
      swimming, charge boots), half-tilt stick on NTSC, and DL NTSC traces.
  - The hero's update entry points and its state machine: states, transitions, and the
    input conditions for each.
  - Physics for each state:
    - gravity;
    - acceleration curves and top speeds;
    - jump impulses: jump, double jump, high jump and long jump;
    - side, back and strafe flips;
    - wall jump, ledge grab, glide and charge boots;
    - swimming and grinding.
  - Ground and collision handling: probes, slopes, steps, moving platforms.
  - Analog input processing: dead zones and response curves.
  - Strafing and lock-on, and hit reactions.
  - The animation each state plays.
  - Output:
    - `docs/research/hero-controller.md`: state diagram, constants converted to metres and
      seconds at 60 Hz, and the differences between UYA and DL.
    - `re/<release>/hero.toml`.
  - Feeds P1.5.
- [ ] **RE.4** **Camera:**
  - Following and recentring.
  - Strafe/lock and first-person modes.
  - Camera collision.
  - The camera volumes and splines that come from level data.
  - Their constants.
  - Feeds P1.7.
- [~] **RE.5** **`tools/pine-probe`**, using the `pine-ipc` crate (MIT):
  - Built 2026-10-11, with our own PINE client instead of `pine-ipc` (section 5). It
    provides:
    - per-frame capture driven by `re/<serial>/hero.toml`, which keeps the settled sample
      of each frame;
    - readers for savestates and input recordings, CSV export, and the memory-card and
      config helpers;
    - `replay.sh`, which runs a recording headlessly on Xvfb.
  - 2026-10-12, additions:
    - `savestate-regs` (the EE registers, for Ghidra's `$gp`);
    - `headless-config --keyboard-pad`;
    - `boot.sh`, which boots a disc headlessly with the keyboard as controller 1, so
      xdotool can play it and take savestates. That is how the NTSC savestates were
      made.
  - All four PAL recordings replayed with no missing frames. Still to do: camera fields
    (with RE.4), and pad input read live rather than from the recording.
  - Reads the hero's position, velocity and state, the camera and the pad input every
    frame from PCSX2.
  - Combined with PCSX2 input recordings, it gives repeatable move tests: running, each
    jump type, flips, gliding, and so on.
- [ ] **RE.6** **Feel-matching harness:**
  - Replay the same input sequences in the hero kit and compare the trajectories with the
    reference measurements: apex height, time to apex, run speed, acceleration and turn
    times, and air control.
  - Tolerances become automated tests.
  - This is the measurable form of "as close as possible" (G1).
- [ ] **RE.7** **Weapons and combat:**
  - Weapon definitions (DL's `GadgetDef` is 0xB0 bytes): fire rates, damage and projectile
    behaviour.
  - Upgrade and XP tables: UYA V1–V5, and DL V1–V10 with mods.
  - How quick-select behaves, the melee combos, and aim assist and lock-on.
  - Feeds P1.8 and P1.9.
- [~] **RE.8** **Timing across regions:**
  - 2026-10-11: on R&C3 PAL and Gladiator PAL the hero updates once per 50 Hz frame at a
    fixed step (`docs/research/hero-controller.md`). The NTSC side is still needed.
  - **2026-10-12, answered for UYA and DL from the code of both regions.**
    - Constants are per second, tuned at 60 Hz.
    - R&C3 PAL folds them with 1/50 where UYA uses 1/60, and Gladiator scales them at
      run time with `frame_dt` = 1/50.
    - Frame counts are cut to about 5/6 on PAL.
    - So PAL plays at NTSC speed, and NTSC-U is the native tuning.
    - Small PAL-only differences: the walk-turn damping and the heli thrust.
    - Details: `hero-controller.md`, "Frames and regions".
    - Confirmed on UYA by our own NTSC traces: the same jumps reach the same heights with
      the 60 Hz values. DL NTSC traces and R&C1/GC are still to do.
  - How R&C3 PAL and Gladiator PAL run at 50 Hz: do they scale per-frame values, or does
    the game simply run slower?
  - Compare RE.5 traces between regions.
  - R&C1's 50/60 Hz menu choice comes later.
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

Crates: `rac_input`, `rac_hero`, `rac_combat`, `rac_net` and `rac_demo`, wired up in
`openrac`. `openrac` runs the demo whenever no supported disc is found.

- **Assets:**
  - The character is the code-built placeholder rig (P1.17).
  - Any other asset must be CC0, with its source and licence recorded in
    `crates/openrac/assets/CREDITS.md`. Bevy resolves `assets/` relative to the binary
    crate's directory.
- **Ecosystem crates:** check that each one supports Bevy 0.20 before adding it (see
  CLAUDE.md). On 2026-10-11, `leafwing-input-manager`, `bevy_enhanced_input`, `avian3d`,
  `bevy_tnua` and `bevy_ggrs` were still on 0.19.
  - Prefer crates that don't depend on Bevy, such as `ggrs`, `parry3d` and `igd-next`.

**Tasks:**
- [ ] **P1.1** App flow:
  - Bevy `States` (Boot → MainMenu → InGame ⇄ Paused).
  - Settings stored as RON/TOML in the platform config directory (the `directories` crate).
  - No disc → the demo.
  - In dev builds, the cache and data directory is `./cache`.
- [ ] **P1.2** Simulation clock and determinism:
  - `FixedUpdate` at 60 Hz, with transform interpolation for rendering.
  - System order: input → enemies → hero → particles → camera.
  - Bevy's `libm` feature plus `bevy::math::ops`.
  - One seeded RNG per level, and no reliance on iteration order (section 2).
  - Inputs can be recorded and replayed.
- [ ] **P1.3** Input layer, `rac_input` (G3):
  - Actions: Move, Look, Jump, Crouch, Melee, Fire, QuickSelect (hold for the ring),
    Strafe/Lock, FirstPerson, Next/PrevWeapon, Map, Pause, and UI navigation.
  - Defaults for a PS-layout gamepad and for KB+M (WASD plus mouse look and aim).
  - Dead zones, sensitivity and invert options.
  - A rebinding UI, with bindings saved to settings.
  - **Each tick produces a compact, serialisable `PlayerInput` per player.** It is what
    replays record and what the netcode exchanges.
  - Use leafwing or enhanced_input once they support 0.20. Otherwise write a thin layer of
    our own over `ButtonInput` and `Gamepad`.
- [ ] **P1.4** Greybox test level built from primitives, containing:
  - flat ground, slopes and steps;
  - ledges, walls for wall-jumping and gaps for long jumps;
  - a high platform and a moving platform;
  - a grind rail and a swingshot target.
- [ ] **P1.5** Kinematic hero controller (`rac_hero`), a capsule against static colliders,
  implementing the **UYA/DL move set** from RE.3:
  - Ground snapping, slope limits and step-up.
  - Running with acceleration and deceleration.
  - Jump, double jump, high jump and long jump.
  - Side, back and strafe flips; wall jump; ledge grab and climb.
  - Glide and charge boots.
  - Strafe and lock-on movement.
  - Knockback, death, and respawn at a checkpoint.
  - Swimming and grinding as stretch goals.
  - Movement is defined by **profiles** (UYA, DL, and the backport profile for the older
    games) stored in RON, each value recording its source. The RE.6 tests must pass.
- [-] **P1.6** Movement feel reference. Moved to RE.3, RE.5 and RE.6.
- [ ] **P1.7** Third-person camera (`rac_hero`) following RE.4:
  - Follows with lag and recentres automatically behind the hero.
  - Right stick or mouse orbit, and collision via sphere casts.
  - Strafe/lock mode and first-person mode.
  - Hooks for camera volumes and splines from level data (Phase 3).
- [ ] **P1.8** Melee (`rac_combat`), following RE.7: the wrench combo, a thrown wrench and a
  downward strike, each with hitboxes and hit reactions.
- [ ] **P1.9** Weapons framework (`rac_combat`), following RE.7:
  - Data-driven definitions (RON):
    - fire mode: hitscan, projectile, lobbed, stream/beam or homing;
    - rate, ammo, damage and projectile physics;
    - hooks for visual and sound effects;
    - XP levels and DL-style mods.
  - Five sample archetypes: rapid-fire, lobbed explosive, stream, homing and sniper.
  - A quick-select ring, weapon cycling, aim assist and strafe aiming.
- [ ] **P1.10** Pickups and economy (`rac_combat`): magnetised bolts; crates (bolt, ammo,
  nanotech, explosive); health; a vendor stub that sells ammo.
- [ ] **P1.11** Demo enemies (`rac_demo`):
  - Two archetypes: a melee charger and a ranged shooter.
  - States: idle, patrol, aggro, attack, hurt and death.
  - Health, damage and drops.
- [ ] **P1.12** UI and HUD, PC first (G5):
  - HUD: health, bolts, ammo and weapon icon; plus the quick-select ring.
  - Pause menu.
  - Settings:
    - display: resolution, window mode, vsync, frame-rate cap, FOV, HUD scale, and
      preferred backend (DX12/Vulkan);
    - volumes;
    - controls, including mouse sensitivity.
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
- [ ] **P1.16** Make the kit ready for mash-ups:
  - The kit crates depend on no disc, format or runtime crate. A CI check on `cargo tree`
    enforces this.
  - Collision goes through a narrow backend interface, so the kit works with any level
    geometry.
  - The public API is documented, and configuration comes from RON assets.
  - `examples/hero_sandbox.rs` drops the kit into a plain Bevy app.
- [ ] **P1.17** Placeholder rigged character:
  - A skeleton built in code, following a documented bone-naming convention.
  - Body parts made from primitive shapes (capsules, boxes, spheres) attached to the bones.
  - Animations authored in code (Bevy `AnimationClip`s) or generated procedurally. They are
    driven by the hero's animation state: idle, run, jump, double jump, glide, crouch,
    flips, ledge hang, melee, firing, hurt and death.
  - A swap point for real skinned models later: the disc's hero model in P3.15, or other
    games' models in mash-ups.
  - Different colours for player 1 and player 2.
- [ ] **P1.18** Direct-IP co-op in the demo (`rac_net`), the first G4 milestone:
  - Host and join by `IP:port` over UDP.
  - A handshake checking the build and settings.
  - A `ggrs` session with rollback and configurable input delay.
  - Snapshot and restore of the demo's simulation.
  - Desync detection with checksums every tick.
  - A second hero in player-2 colours.
  - Disconnect handling.
  - Tests with simulated latency and packet loss, then real runs over LAN and over the
    internet (port forwarding or UPnP, per D8).
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

Crates: `rac_iso`, `rac_formats::wad`/`elf`, and `rac_tool`. P2.4 and P2.5 come first: once
the UYA and DL executables can be unpacked, the RE track no longer needs savestates or
Wrench.

- [ ] **P2.1** `rac_iso` reads ISO9660 over `Read + Seek`: the PVD, directories and file
  extents. It exposes a raw sector API. It never loads a whole 4 GB image.
- [ ] **P2.2** Disc identification:
  - Read `SYSTEM.CNF` (`BOOT2`, `VER`, `VMODE`) and the SHA-1 of the boot ELF.
  - Map them to a `Release { game: Rac1|Gc|Uya|Dl, region, version, serial }`.
  - Build a table of known releases: the six discs in `disc-layout.md`, plus SCUS-97199 and
    SCUS-97268. Each entry records the TOC LBA, the `SYSTEM.CNF` LBA and whether the ELF is
    packed.
  - Unknown discs get a clear error.
- [ ] **P2.3** TOC parsers:
  - GC/UYA/DL at LBA 1001: global WAD headers identified by size, and a level table whose
    order differs by game. Include DL's missions.
  - R&C1 at LBA 1500: the header, global ranges and `levels[19]`, which point to level
    headers.
- [ ] **P2.4** WAD codec:
  - Decompressor for the packet stream, the 0x2000-byte blocks, and the padding and dummy
    packets.
  - Fuzzing (`cargo-fuzz`) and property tests.
  - A compressor later, for tests and mods.
- [ ] **P2.5** Unpacker for the packed executables (R&C3, UYA, DL, Gladiator: the stub plus
  a WAD in `.rodata`, at offset 0x7080 on our discs) → the real game ELF.
- [ ] **P2.6** Level overlay extraction: read the 16-byte `{dest, size, type, entry}`
  records and produce per-level code images (for reverse engineering and the recompiler).
- [ ] **P2.7** `openrac-tool` CLI: `identify`, `toc`, `extract --out cache/<release>/`,
  `unpack-elf` and `dump-overlay`.
- [ ] **P2.8** Cache layout and versioning:
  - Path: `cache/<serial>-<elfsha1[0..8]>/…`.
  - A manifest records the tool version and source hashes, so extraction can be incremental
    and repeatable.
- [ ] **P2.9** Validation:
  - Commit golden **checksums** (not data) of the decompressed outputs.
  - Tests that read real discs are gated on `OPENRAC_SOURCE_DIR`.
  - Cross-check against `wrenchbuild unpack` output on the local machine.
- **Done when:** all six discs identify, every TOC entry is listed, every WAD decompresses,
  the packed ELFs unpack, and our output matches Wrench's wherever they can be compared.

### Phase 3: Asset formats, native level viewer and remaster renderer

Work order: UYA NTSC-U → R&C3 PAL → DL NTSC-U → Gladiator PAL → R&C1 → GC. Crates:
`rac_formats`, `rac_assets`, `rac_render`, `rac_audio`.

- [ ] **P3.1** ELF and overlay model, and a **catalogue of each game's VU microcode**
  (DVP overlays). The renderer work depends on it.
- [ ] **P3.2** Textures:
  - Formats: PSMT8/PSMT4 with RGBA32/16 CLUTs, CSM1 palette order, GS swizzles, alpha 0x80
    → 1.0, and mips.
  - Texture tables and the GS RAM table → Bevy `Image`.
  - A texture browser for debugging.
- [ ] **P3.3** The level core header (0xBC) and the class tables.
- [ ] **P3.4** Tfrags (terrain, with LOD 0 first), **P3.5** ties (instanced, with
  per-instance vertex colours), **P3.6** shrubs.
- [ ] **P3.7** Mobys:
  - Class header, packets and LODs.
  - Skeleton and joints, and skinning with up to 3 joints per vertex.
  - Animation sequences → Bevy `AnimationClip`.
  - Metal (environment map) materials and bangles.
- [ ] **P3.8** Sky: shells, and rotation converted from per-frame to per-second using the
  region's refresh rate.
- [ ] **P3.9** Collision: the octant grid and triangle/quad faces with their type byte.
  - Debug overlay.
  - **Colliders for the hero kit.**
- [ ] **P3.10** Gameplay data:
  - Level settings.
  - Moby/tie/shrub instances with their pvars and per-game record sizes.
  - Paths, grind paths, volumes, cameras, lights, sound instances and areas.
  - DL missions and art instances.
- [ ] **P3.11** The **classic** render path, PS2-faithful, used for validation and for
  classic mode. Its shaders are written in WESL (as of Bevy 0.20):
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
  real collision, with the placeholder rig swapped for the disc's hero model through the
  P1.17 swap point. No game logic yet.
- [ ] **P3.16** R&C1 and GC formats, once the NTSC-U discs arrive:
  - Evaluate ReRAC's ISC format crates for reuse with attribution (this was P0.8).
  - Add the R&C1 and GC variants of the formats.
- [ ] **P3.17** The **remaster** render path (G5), which is the default:
  - HDR with tonemapping and bloom, dynamic sun shadows and ambient occlusion.
  - Longer LOD and draw distances.
  - Anisotropic filtering and MSAA/TAA.
  - Field of view and culling that are correct in widescreen.
  - Uncapped frame rates through interpolation.
  - Support for user-supplied high-resolution texture packs.
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
- [ ] **P5.7** Backport the R&C3/Gladiator mechanics to R&C1 and GC (D2):
  - The native hero replaces the recompiled one.
  - Fix individual levels where the new moves break assumptions: sequence breaks, triggers,
    camera volumes.
  - Offer the classic hero as an option (D7).
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
  - Networking help: firewall, port forwarding, UPnP.
  - Possibly mod support.
- [ ] **P7.5** Legal review of disclaimers and branding (trademark use in the project name).

## 7. Cross-cutting

- **Testing:**
  - Unit tests on synthetic data.
  - Tests that need a disc read the `OPENRAC_SOURCE_DIR` environment variable. Mark them
    `#[ignore]` with a reason and run them with `cargo test -- --ignored`.
  - Golden checksums.
  - Feel-matching tests (RE.6).
  - Determinism replays (P1.19).
  - Differential tests against PCSX2 (Phase 4 onward).
  - Visual comparisons, run locally only.
- **CI** (P0.7, P0.12):
  - fmt, clippy with `-D warnings`, tests, and Windows and Linux builds.
  - A check that the kit stays ISO-free (P1.16), the data-hygiene guard, and determinism
    hashes compared across operating systems.
  - Tests that need a disc never run in CI.
- **Determinism** is a requirement, not a nicety, because of co-op (section 2):
  - fixed 60 Hz simulation and a seeded RNG for each level;
  - `libm` math and stable ordering;
  - input recording and replay;
  - state checksums.
- **Performance budgets:**
  - The demo runs at 60 fps or better at 1080p on a mid-range GPU.
  - Recompiled game logic uses less than 4 ms of each frame.
  - Level loads take less than 2 s from a warm cache.
  - Co-op is playable at a 100 ms round trip.
- **Diagnostics:** `tracing` through Bevy's logging (`RUST_LOG`), the `bevy/trace_tracy`
  feature for profiling, an egui debug overlay, and network stats (round trip, rollbacks,
  desyncs).
- **Documentation:**
  - `docs/research/` holds findings.
  - `docs/formats/` holds our own format specs, written in our words and citing sources.
  - `docs/adr/` holds architecture decision records for big choices, such as the Phase 4
    gate and the netcode model.

## 8. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| The controller doesn't feel like the original | G1 fails at its core | Reverse engineering from day one (RE.3, RE.4); measured traces (RE.5); tolerance tests (RE.6). |
| Determinism breaks between machines or operating systems | Co-op desyncs | `libm` math, no reliance on iteration order, replay tests across operating systems in CI, per-tick checksums, resync by snapshot. |
| Lockstep feels laggy in a fast shooter | Poor co-op | Low input delay; rollback for the demo; reconsider rollback for the full games once runtime snapshots are fast (P6.2). |
| Backported moves break older levels (sequence breaks, triggers, cameras) | Bugs and softlocks | Per-level fixes, the classic hero option (D7), and the compatibility list (P5.6). |
| Legal problems in a public repo (GPL contamination, derived data, trademarks) | Takedown or relicensing | The CLAUDE.md rules, the CI data guard (P0.12), generating everything at install, reference-only use of GPL, and P7.5. |
| VU1 and GS semantics (bloom, render-to-texture, texture paging) | Visual bugs | Native renderers per renderer type, OpenGOAL as precedent, the PCSX2 GameDB fixes as a map of hard spots, PCSX2 comparisons. |
| Recompiler complexity (overlays, indirect jumps, asynchronous VU0, non-IEEE floats) | G2 slips | The Phase 4 spike and gate; reuse rabbitizer and ico-recomp; differential testing. |
| IOP and sound gaps (989snd variants, "stash", DL's streamer) | Missing audio | Port OpenGOAL's 989snd; emulate at the RPC level (RE.12). |
| PAL data tuned for 50 Hz (R&C3 PAL, Gladiator PAL) | Wrong speeds | NTSC-U as the primary reference; RE.8; per-release adapters. |
| Bevy churn (a breaking release roughly every 4 months; ecosystem lag) | Upgrade cost | Pin versions; upgrade in one dedicated change; keep the format, recompiler, runtime and netcode cores free of Bevy. |
| The kit's API churns and breaks mash-ups | Reuse suffers | A small documented API, the sandbox example (P1.16), and semantic versioning once it stabilises. |
| NVIDIA driver crash when compiling pipelines in parallel | Crashes at startup | Synchronous compilation (done); prewarm pipelines; re-test regularly (P1.15). |
| Scope: 6 discs, 4 engines, online co-op and a remaster | Never finishing | Strict phase gates, UYA and DL first, and the demo first for visible progress. |
| Continuity across many AI sessions | Lost context | Keep PLAN.md and CLAUDE.md up to date, keep changes small, and keep the session log. |

## 9. Session log

Newest first. One or two lines per session: what was done, and what's next.

- **2026-10-12 (second update):** RE.3 continued without new recordings from the owner.
  - Read air control, flips, the long jump, glide, falling, attacks and ledges in code.
    Verified the walk turn spring and air control against the PAL traces.
  - Made our own UYA NTSC traces by scripted keyboard play in the headless boot. They
    verified the NTSC jump, walk, glide, heli, long-jump, flip and jump-attack values.
    Added UYA's `hero.toml` (frame counter confirmed, pad region) and L2/R2 to the
    keyboard pad.
  - The changes are on branch `re/hero-movement`, uncommitted. The owner will commit and
    sign them.
  - Next: moves that need particular places, DL NTSC traces, then the camera (RE.4).

- **2026-10-12:** Static analysis of the hero (RE.3) with Ghidra, now that Xvfb is
  installed.
  - Found the hero's movement code in R&C3 and Gladiator PAL, and in UYA and DL NTSC-U.
    The NTSC savestates were booted headlessly with the new `boot.sh`.
  - Verified the input, walk and jump models frame for frame against the PAL traces, and
    read every jump type's parameters from code. Rewrote `hero-controller.md`.
  - Answered RE.8: per-second constants at 60 Hz, which PAL converts.
  - Added:
    - `symbols.toml` for four releases, and new `hero.toml` fields (RE.2);
    - the `tools/ghidra/` scripts;
    - `pine-probe savestate-regs` and `headless-config --keyboard-pad`.
  - Next: NTSC and single-move recordings from the owner, CI, then the kit foundations
    and P1.5 from the code-derived constants.

- **2026-10-11 (fifth update):** The owner supplied PAL savestates and input recordings.
  - Located the hero and a per-frame counter in R&C3 PAL and Gladiator PAL (RE.2, RE.3).
  - Built `tools/pine-probe` and the headless replay harness (RE.5). All four recordings
    were replayed with no missing frames.
  - Measured the first movement constants (`docs/research/hero-controller.md`).
  - Finished RE.1 (raw EE-memory import into Ghidra).
  - Added CI (P0.7, not yet run on GitHub), the data-hygiene guard (P0.12) and
    CONTRIBUTING.md (P0.11). Checked the disc hashes against redump.org (P0.10).
  - Next: NTSC-U and single-move captures from the owner, a first CI run, and static
    analysis of the measured constants.

- **2026-10-11 (fourth update):** Installed the reverse-engineering tools (RE.1), all under
  `~/.local` without sudo:
  - JDK 21, Ghidra 12.1.4 with the PS2 extension, and PCSX2 2.8.2 with PINE turned on.
  - Wrote `docs/re-setup.md`.
  - Next: the owner's BIOS and capture session, then RE.3 and RE.5.
- **2026-10-11 (third update):**
  - Rewrote README.md for visitors on GitHub; the original preliminary plan now lives in
    sections 1 and 5.
  - Corrected the graphics-backend notes. wgpu prefers Vulkan over DX12 whenever both
    exist, so a Windows default is now a decision to make in P1.14.
  - Made the first commit of the scaffolding.
  - Next: section 0.
- **2026-10-11 (second update):** Recorded the owner's decisions D1–D5 and reshaped the plan.
  - The hero kit is ISO-free and reusable.
  - R&C3/UYA and Gladiator/DL come first.
  - Direct-IP co-op in every game, using deterministic `ggrs` netcode.
  - A remaster by default.
  - Reverse engineering moved to a track starting in Phase 0. The runtime, full-game,
    co-op and release phases are renumbered 4–7.
  - The Linux build dependencies are installed and the native build is verified.
  - Next: section 0.
- **2026-10-11:** Project bootstrap.
  - Inspected all six discs (LBA layouts, TOCs, ELF generations, toolchain strings,
    hashes).
  - Researched the PS2 and R&C landscape.
  - Fixed the ISO ignore rule.
  - Pinned Rust 1.99.0. Created the workspace and a Bevy 0.20 app that renders a greybox
    scene; fmt and clippy are clean.
  - Found and worked around the NVIDIA parallel pipeline compilation crash.
  - Dropped the dev container at the owner's request.

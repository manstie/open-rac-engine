# Prior art, references and licences

This is a snapshot from 2026-10-11. Most of the R&C projects listed here are only weeks
old and change daily, so re-check them before you rely on any detail. Entries marked ✔
were confirmed this session through the GitHub API or the project's README. Everything
else comes from web research and has not been re-checked.

**Licence rule for this repo (GPL-3.0-or-later since 2026-10-15; MIT before):**
- Code under a licence GPLv3 can absorb may be reused, keeping its copyright and licence
  notice in the file header and listed below: permissive licences (MIT, ISC, BSD,
  Apache-2.0, zlib, AFL-2.0), GPL-3.0, GPL-2.0-or-later, and LGPL.
- GPL-2.0-only code and code with no licence are reference only. Read them to learn facts,
  then write our own implementation. Never translate their code line by line.
- Never commit decompiled game code, game data or Sony SDK code, whatever licence label a
  project puts on it.

## Projects doing what we want to do

| Project | What it is | Target | Licence | Status |
|---|---|---|---|---|
| **ReRAC** ✔ [re-rac/rerac](https://github.com/re-rac/rerac) | A native PC port of R&C1 written in **Rust on Bevy** (0.19). Crates for formats, game logic, engine and extractor, plus `cargo xtask`. Format specs in `docs/formats/` were written from Wrench's behaviour without copying its code. Its `moby_rac1.md` (vertex IDs, the vertex cache, the VU0 skinning slots, normals and texture switches) was the reference for our moby mesh decoder on 2026-10-13, and its `moby_skinning_lighting.md` (the metal packets' vertex layout and what R&C1's code does with them) for our metal packets on 2026-10-15; our code is our own, and every rule was checked on UYA's data (`moby-format.md`, "Mesh", "Metal packets"). | One disc: R&C1 NTSC-U SCUS-97199 v1.00 | **ISC** ✔ | Created 2026-09-26 ✔. "All levels load and render", not playable start to finish ✔. Only macOS on Apple Silicon has been tested ✔. Has a launcher repo and a Discord. |
| **OpenRAC** ✔ [OpenRAC/OpenRAC](https://github.com/OpenRAC/OpenRAC) | A hub for matching decomps of all four games, plus a planned OpenGOAL-style native port (C++20, OpenGL 4.1, SDL3, no emulation) | All four games | NOASSERTION ✔ (GPL-3.0-or-later) | Created 2026-10-03 ✔. Port design in `docs/port`: 989snd replaced at its API, no IOP model, rendering hooks at `VU1_sendChain`, `VU1_syncChain` and `sceGsSyncV`. Cross-game docs in `docs/engine/`. |
| **rac1-decomp** ✔ [OpenRAC/rac1-decomp](https://github.com/OpenRAC/rac1-decomp) | Matching decompilation of R&C1 | **SCES_509.16 PAL v2.00 with SHA-1 `79956931…`, which is exactly the user's disc** ✔ | GPL-3.0 ✔ (MIT before 2026-10-07) | About 78% of code matched (decomp.dev). Covers the boot ELF and all 19 overlays. Has `config/names.tsv` and `symbol_addrs.txt`. |
| **Lombyte** ✔ [lombyte-project/Lombyte](https://github.com/lombyte-project/Lombyte) | Matching decompilation of R&C1. Its long-term goal is a native Rust port. | SCUS-97199: its `config/us/checksum.sha1` gives `SCUS_971.99` SHA-1 `72fd1de3…`, **which is exactly the user's disc** ✔ (2026-10-15) | GPL-3.0 ✔ | About 70% |
| OpenRAC/rac2-gc-decomp | GC decompilation | SCUS-97268 v1.01 ✔ (`docs/REGIONS.md`). The user's GC USA disc is v2.00 (Greatest Hits), a different build | MIT | 1–3% |
| platypet2217-star/RAC2Decomp | Functional, non-matching GC decompilation done in Ghidra | GC PAL SCES-51607 | MIT | Unknown |
| OpenRAC/rac3-uya-decomp | UYA decompilation | SCUS-97353 | Tooling GPL-3.0 | The boot pieces rebuild byte-identically; overlays barely started |
| OpenRAC/rac-deadlocked-decomp | DL decompilation | SCUS-97465 | MIT | About 0.5% |
| enzonami/Pipe | UYA extractor, viewer and repacker, written in Rust | SCUS-97353 | MIT | Unknown |

Decompilation progress on decomp.dev, as reported: R&C1 PAL 78.5% (16% fully linked),
R&C1 US 72%, UYA 6.9%, GC 2.8%, DL 0.5%. No decompilation targets R&C3 PAL or Gladiator PAL.

No retail or prototype EE executable is known to keep its symbol tables, so names come
only from the decompilation projects.

## Modding tools and format references

- **Wrench** ✔ [chaoticgd/wrench](https://github.com/chaoticgd/wrench)
  - Licence: GPL-3.0 ✔. `src/memmap.c` is BSD-2.
  - Version: v0.6, released 2026-10-04.
  - **Supports all six of the user's serials** checked on 2026-10-11 (the NTSC-U R&C1 and GC
    discs came later and are unchecked).
  - Main tool: `wrenchbuild unpack|pack -g rac|gc|uya|dl -r us|eu|...`.
  - Docs worth reading for facts: `file_loading.md`, `level_core.md`, `gameplay.md`,
    `renderers.md`, `moby_renderer.md`, `tfrag_renderer.md`, `shrub_renderer.md`,
    `collision.md`, `textures.md` and `asset_reference.md`.
  - Source worth reading for facts: `src/iso/table_of_contents.*`,
    `src/iso/wad_identifier.cpp`, `src/engine/compression.cpp`, `src/core/elf.cpp`,
    `src/core/release.cpp` and `src/wrenchbuild/level/*`.
  - Known bugs:
    - Repacked DL and Gladiator ISOs hang when a level loads (#313, #329).
    - Repacked R&C1 ISOs have texture and collision glitches (#288).
    - When unpacking R&C1, Wrench reads only `gameplay_ntsc`.
  - It is the best tool for cross-checking our own extractor's output locally.
- **Replanetizer** ([RatchetModding/Replanetizer](https://github.com/RatchetModding/Replanetizer))
  - Licence: GPL-3.0.
  - It edits levels from the PS3 HD versions.
  - Its gameplay layouts are the PS2 ones converted to big-endian, which makes it useful
    for field names and meanings.
  - Its render formats are re-encoded and differ from the PS2.
- **Horizon** ([Horizon-Private-Server](https://github.com/Horizon-Private-Server))
  - Licence: MIT. It revives online play for UYA and DL.
  - **libdl** ✔ ([Dnawrkshp/libdl](https://github.com/Dnawrkshp/libdl), MIT ✔) targets DL
    NTSC-U SCUS-97465 only.
    - Headers: `Moby` (about 400 moby IDs), `Player`, `HeroPlayerConstants`, `GadgetDef`.
    - It also lists function addresses. Its fixed addresses for the resident program
      (library code, the network library high in memory, globals) hold in single player
      too; its `_inGame` and `_inLobby` ones are multiplayer overlays. They seeded
      `re/SCUS-97465/symbols.toml` on 2026-10-15, and through matching code, the other
      three releases' (`engine-notes.md`, "Code layout and overlays").
    - Its `Player.SkinMoby` and skin and wrench class IDs led to DL's skins on 2026-10-15
      (`moby-format.md`, "DL's skins"); everything used was checked in the game's code
      and data.
  - **libuya** (`horizon-uya-patch/libuya`, MIT) builds for both UYA NTSC and PAL.
    - It defines `MobyClass` and `Moby`.
      - Its `MobyClass` and `MobySeq` layouts were the starting point for
        `moby-format.md` (2026-10-13). Everything used was re-checked against UYA NTSC's
        data. Its `Moby` animation fields don't match UYA's single-player hero.
    - It lists per-map overlay addresses.
    - **Its addresses are the multiplayer program's**, which is linked differently from
      single player: none of its data addresses is used by single-player code, and its
      library functions sit 0x32D0 to 0x57D38 higher than in single player
      (`engine-notes.md`, "Code layout and overlays"). Using them needs a multiplayer
      memory image, or matching each function's code into single player.
- **Lists of memory addresses:** Programie/RCMemory covers PAL R&C1–3 in PCSX2 format. It
  has no licence, so we take single addresses from it as facts and re-verify them (its R&C3
  hero position, 0x1A4C60, checked out on 2026-10-11).
  There is also the "Ratchet & Clank Series Addresses" spreadsheet and CreepNT/MobyViewer.
  RaCMAN covers only the PS3 and RPCS3 versions.
- **Index of community links:** [rac-modding-resources](https://github.com/RatchetModding) (CC0).

## Emulators and runtimes (for HLE and correctness reference)

- **PCSX2** ([PCSX2/pcsx2](https://github.com/PCSX2/pcsx2))
  - Licence: GPL-3.0+. Version: v2.8.2, released 2026-09-04.
  - Its debugger can import symbols (ELF, `.mdebug`/STABS, SNDLL, `.sym`) through CCC. It
    supports conditional breakpoints, memory search and function hashing, which helps tell
    overlays apart.
  - **PINE IPC:**
    - Enable it under Settings → Advanced. It listens on a Unix socket at
      `$XDG_RUNTIME_DIR/pcsx2.sock`, slot 28011.
    - It can read and write 8, 16, 32 and 64-bit values in EE memory, batched, and can
      save and load states.
    - A Rust client exists: `pine-ipc` (MIT). We wrote our own small client in
      `tools/pine-probe` instead, from the protocol facts in PCSX2's `PINE.cpp`, because we
      need raw batched reads and the protocol is tiny.
    - PINE reads run on PCSX2's own thread while the game keeps running, so a batch is not
      a consistent snapshot. `pine-probe capture` works around this.
  - **Input recordings** (`.p2m2`) and **savestates** (`.p2s`, zip with Zstandard members)
    are read by `tools/pine-probe` from format facts in PCSX2's `Recording/` and
    `SIO/Memcard/` sources (GPL, facts only).
  - **GameDB:** all eight R&C serials need `EETimingHack`. There are also GS fixes, listed
    in [engine-notes.md](engine-notes.md#rendering).
  - **Patches:** widescreen patches exist for every serial except the R&C1 NTSC-U one, which
    is broken. 60 fps split-screen patches exist for UYA and DL.
- **Play!** ([jpd002/Play-](https://github.com/jpd002/Play-))
  - Licence: BSD-2-Clause, permissive.
  - It emulates the BIOS at a high level and reimplements the IOP kernel and modules in C++
    (`Source/iop/`).
  - It is the best permissively licensed reference for high-level emulation of the IOP
    kernel and SIF RPC.
- **DobieStation**
  - Licence: GPL-3.0. Inactive since 2021.
  - A compact C++ codebase covering the EE, VU, IOP and a software GS.
- **PS2SDK**
  - Licence: AFL-2.0.
  - Open reimplementations of the IOP modules (sifman, sifcmd, padman, mcserv, libsd, …).
    Use it for RPC protocols and import tables.
- **ps2tek**
  - <https://psi-rockin.github.io/ps2tek/>. It has no licence file: read it, but don't
    copy from it.
  - A hardware reference covering the EE, GS, DMAC, VIF/VU, IPU, IOP, SIF and BIOS.
- **Rust PS2 emulators:** nothing mature exists, only toy projects.

## Static recompilation and native-port precedents

- **PS2Recomp** ✔ ([ran-j/PS2Recomp](https://github.com/ran-j/PS2Recomp))
  - Licence: GPL-3.0 ✔. About 3.4k stars ✔.
  - It translates R5900 code into literal C++.
  - The runtime includes VU1 and VIF interpreters, a software GS and an R3000 IOP
    interpreter.
  - Its README says "Performance is very bad for VU and GS". No game is publicly confirmed
    playable.
- **ico-recomp** (nathanialf/ico-recomp)
  - Licence: **MIT**. Archived on 2026-10-07.
  - **Rust** crates: `r5900-decode`, `vu-decode`, `ee-emit`, `vu-emit`, `ee-interp`,
    `vu-interp`, `ingest` and `recomp-cli`. The generated C is gitignored and tied to the
    input disc by SHA-1.
  - Its 5 VU1 programs are statically recompiled.
  - The ICO PAL disc boots, renders and plays with sound.
  - **We can reuse these crates.**
- **ps2-web-recomp** (snuri00)
  - Licence: MIT.
  - It turns EE code into WebAssembly and compiles VU1 programs ahead of time to SIMD C++.
  - It emulates the GS at a low level in WebGPU compute.
- **N64Recomp** (MIT) and Zelda64Recomp; **XenonRecomp** (MIT) and Unleashed Recompiled
  - Both use the same legal model: building needs the user's own game files to generate
    code, and the repo contains no game code.
  - Releases don't contain game assets, and the game data is supplied at runtime.
- **OpenGOAL** ([open-goal/jak-project](https://github.com/open-goal/jak-project)): licence
  **ISC**. The closest precedent overall.
  - **Extraction:** an extractor and decompiler run on the user's ISO and convert the data
    to PC formats. Hand-written assembly goes through Mips2C.
  - **Rendering:**
    - The DMA chain is intercepted at sync points and split into buckets, each handled by a
      native C++ renderer (tfrag, tie, shrub, merc, generic, sky, …).
    - It does not emulate the GS.
  - **Sound:** **989snd is reimplemented in C++ (`game/sound/989snd`)**. This is our port
    source for audio.
  - **Frame rate:** it generalises frame-rate scaling (`time-adjust-ratio`).
  - Jak 1 was released in 2022. Jak 3 entered beta on 2026-04-01.
- **Dark Cloud (Chronicle)**: a complete decompilation plus a native SDL3/Vulkan port. It
  runs a fixed 60 Hz logic tick and interpolates rendering.

## Tooling

| Tool | Licence | Notes |
|---|---|---|
| Ghidra 12.1 + [ghidra-emotionengine-reloaded](https://github.com/chaoticgd/ghidra-emotionengine-reloaded) v2.1.38 | Apache-2.0 | Supports MMI, VU0 macro mode and STABS, and can import PCSX2 savestates. No VU micro-mode support. |
| CCC (chaoticgd/ccc) | MIT | Parses STABS/`.mdebug` symbol tables. PCSX2 includes it. |
| rabbitizer | MIT | MIPS disassembler with a pure-Rust 2.0 alpha. Decodes the R5900, **including MMI and VU0 macro** instructions, but not VU micro mode. |
| splat, spimdisasm, objdiff, decomp.me | MIT / Apache | Standard decompilation tooling. decomp.me has SN EE GCC 2.95.3 v1.14 and v1.36. |
| m2c | GPL-3.0 | MIPS-to-C decompiler with a `mipsee` target. |
| Cranelift 0.136 | Apache-2.0 with LLVM exception | `cranelift-jit` and `cranelift-object` (for compiling ahead of time). The gecko GameCube/Wii emulator (GPL) is a precedent for using it this way. |
| `ps2-memcard` / `ps2mc` crates | various | Read PS2 memory-card images, for importing PCSX2 saves. |
| `objdiff-core` | MIT/Apache | Function diffing, for differential testing. |
| `ggrs` 0.13 | MIT/Apache | GGPO-style peer-to-peer netcode over UDP. Supports rollback, and lockstep with prediction turned off. It doesn't depend on Bevy. Our planned co-op transport (PLAN §4.2). |
| `bevy_ggrs` 0.22, `lightyear` 0.30, `bevy_replicon` 0.44 | MIT/Apache | Bevy networking crates, all still on Bevy 0.19 as of 2026-10-11. |
| `igd-next` 0.18 | MIT | UPnP port mapping, so direct-IP hosts can open a port automatically (PLAN D8). |

## Reuse summary for this repo

| Reuse, keeping its notices | Reference only (read, never copy) | Never commit |
|---|---|---|
| ReRAC (ISC), OpenGOAL (ISC), Play! (BSD-2), ico-recomp crates (MIT), libdl and libuya (MIT), Pipe (MIT), rabbitizer (MIT), CCC (MIT), pine-ipc (MIT), Cranelift (Apache), ggrs (MIT/Apache), igd-next (MIT), PS2SDK (AFL-2.0), rac-modding-resources (CC0); Wrench, Replanetizer, PCSX2, DobieStation, PS2Recomp, m2c, and the tooling of OpenRAC, rac1-decomp and Lombyte (all GPL-3.0), paraLLEl-GS (LGPL) | ps2tek and RCMemory (no licence), any GPL-2.0-only code, Sony's manuals | Decompiled game code (whatever its label, so the decompiled sources of rac1-decomp, Lombyte and OpenRAC's decompilations), game data or anything extracted from it, Sony SDK code or NDA material |

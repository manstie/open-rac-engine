# Engine notes: what is known about the Insomniac PS2 engine

This is a working summary of technical facts that are needed for the asset pipeline, the
renderer, the runtime and gameplay. Last updated 2026-10-11.

Most entries come from community sources. Those sources are listed in
[prior-art.md](prior-art.md), and the main ones are Wrench's docs, OpenRAC's docs, ReRAC's
docs and Horizon's headers. Entries that our own disc inspection confirmed are marked
**[verified]** (see [disc-layout.md](disc-layout.md)). Entries a source itself flags as
uncertain are marked **[unverified]**. Everything else is "reported" and should be
re-checked against the disc before you depend on it.

All of this is written in our own words. Code reused from other projects belongs in the
code, with its notices, not here; what may be reused is in `CLAUDE.md`'s legal rules.

## Engine generations and families

- **One engine, two code families.**
  - R&C1 and GC share about 70% of their engine core. UYA and DL share about 69%. R&C1
    and UYA/DL share only about 27%.
  - Game and level code is mostly unique to each game. Less than 1% is identical across
    games, except UYA and DL at 6–10%.
  - Distinct code per game is roughly 3.7, 5.1, 7.0 and 5.4 MB for R&C1, GC, UYA and DL.
  - Source: OpenRAC's cross-game function map, 2026-10-04.
- **Formats are shared much more than code is:**
  - Common to all four games: the WAD codec, the overlay format, the level core header, the
    moby/tie/shrub/tfrag encodings, textures and palettes, VAG, 989snd banks and PSS video.
  - Different per game: the TOC (R&C1 versus the rest), WAD headers, gameplay layouts and
    instance sizes, chunks (GC onward), packed executables and team palettes (UYA/DL), and
    missions and art instances (DL only).
- **Toolchain:** SN Systems ProDG.
  - R&C1 matches SN GCC 2.95.3 "v1.14" with `-O2 -G2`.
  - UYA and DL match SN GCC 2.95.3 v1.36 with `-O2 -G8`.
  - The compiler for GC is disputed.
  - Our strings show a newlib-style assert format and DVP overlay sections **[verified]**.
- **Language and codename:** the game is C++, and its internal codename is "i5". Source
  paths look like `C:\code\i5\...` and `z:/i5/...` **[verified for the z:/i5 paths]**.

## Disc, TOC and WAD

- **Sectors and SYSTEM.CNF:**
  - A sector is 0x800 bytes.
  - `SYSTEM.CNF` sits at LBA 289 on R&C1 and LBA 1000 on the other games. The memory-card
    code depends on that position **[verified positions]**.
- **R&C1 TOC (LBA 1500)**:
  - Header `{version = 1, header_size = 0x2960}` **[verified]**.
  - Then `(lba, sectors)` ranges for the global assets: debug font, save template, hero and
    HUD animation sets, vendor, help, `frontbin`, IRX modules, the global 989snd bank, HUD
    banks, text, NTSC and PAL credits, 88 MPEG entries, and help audio.
  - Then 19 level entries. Each points to a 0x2434-byte level header:
    `id, header_size, data, gameplay_ntsc, gameplay_pal, occlusion, bindata[36], music[15]`
    plus the scene records.
- **GC, UYA and DL TOC (LBA 1001):**
  - It starts with the global WAD headers stored back to back. Each begins
    `{header_size, lba}` and is identified by its size.
    - The global WADs are armor, audio, bonus, gadget, hud, misc, mpeg, scene, space and
      (DL only) online.
  - A level table follows, with three `(lba, size)` pairs per level.
    - GC orders them LEVEL, AUDIO, SCENE. UYA and DL order them AUDIO, LEVEL, SCENE.
  - On R&C2 PAL this region is also the visible file `RC2.HDR` **[verified]**.
- **Level WAD headers:**
  - GC and UYA use 0x60 bytes: size, lba, id, reverb, data, sound bank, gameplay,
    occlusion, `chunk[3]`, `chunk_sound_bank[3]`.
  - GC may also have a 0x68 variant with separate NTSC and PAL gameplay **[unverified]**.
  - DL uses 0xC68 bytes and adds `missions[128]` (instances, classes, sound bank) and art
    instances.
- **WAD compression** is an LZ77/LZO-like packet stream:
  - The header is 16 bytes: the magic `"WAD"`, then a `u32` compressed size at +3
    (unaligned), then padding. The magic is **[verified]** in the packed executables.
  - The type of each packet is decided by its flag byte:

    | Flag byte | Packet type |
    |---|---|
    | 0x00–0x0F | literal run |
    | 0x10–0x1F | far match |
    | 0x20–0x3F | medium match |
    | 0x40–0xFF | short match |

  - The low 2 bits of a match carry up to 3 trailing literals.
  - Packets never cross a 0x2000-byte block.
  - Padding packets are `12 00 00` followed by 0xEE fill.
  - A dummy packet, `11 00 00`, separates two literal runs.
  - Wrench realigns to 0x1000. OpenRAC reports that R&C1 PAL uses 0x2000. **Resolved for
    UYA (2026-10-13):** its decompressor reads 0x2000-byte blocks; the full codec is in
    [disc-layout.md](disc-layout.md#wad-compression-verified-2026-10-13).
- **Packed executables (R&C3, UYA, DL, Gladiator):** a loader stub plus a WAD-compressed
  game image in `.rodata`, which on our discs begins at file offset 0x7080 **[verified]**.
  The image is the boot ELF's sections as overlay records (below); `openrac-tool
  unpack-elf` writes it as a plain ELF **[verified 2026-10-15,
  [disc-layout.md](disc-layout.md#2-rc3-uya-deadlocked-and-gladiator-packed-executables-verified-2026-10-15)]**.
  Wrench 0.4 and later can unpack them too.

## Code layout and overlays

- **What stays resident:** the boot ELF keeps `.vutext` and `core.text/.data/.rdata/.bss/.lit`
  resident **[verified section names on R&C1/R&C2]**. On R&C3, UYA, DL and Gladiator,
  `.vutext`, `core.text` and `core.rdata` in a level match the unpacked boot program byte
  for byte, while `.lit`, `.data`, the `lvl.*` tables and `.text` don't: the boot program's
  `.text` is the title screen's **[verified 2026-10-15 against savestates]**.
- **What each level replaces:** `.lit .bss .data lvl.vtbl lvl.camvtbl lvl.sndvtbl .text`,
  with DL adding `net.text`.
  - Each level carries its own build of almost the whole game program.
  - R&C1 has about 3.5 MB of distinct overlay code against a 348 KB boot `.text`. DL has
    47 overlays of 2.6–3.1 MB each, and 85–97% of that code is shared between levels, at
    different addresses.
- **Overlay data format:** a series of 16-byte records `{dest_addr, size, section_type, entry}`,
  each followed by its bytes. The loader copies records until `entry` changes, then calls
  the level-init entry point. The packed executables' stubs load the boot program from the
  same records **[verified in UYA NTSC-U's stub, 2026-10-15]**; `rac_formats::elf::Program`
  reads them.
- **Load addresses:**

  | Build | Addresses |
  |---|---|
  | R&C1 PAL | `.lit` 0x15F000 and `$gp` 0x166D00 **[verified]**; boot `.text` 0x1E9080 **[verified]** |
  | R&C1 NTSC-U | `.lit` 0x15EF00 and `$gp` 0x166C00 **[verified on our SCUS-97199 disc]**; boot `.text` 0x1E8D00 |
  | Our GC NTSC-U v2.00 disc | `.lit` 0x1A7C80, `.text` 0x26EA00, `$gp` 0x1AEFF0 **[verified]**. The figures reported for GC, region unclear, are this build's. |
  | Our GC PAL disc | `.lit` 0x1A7D00, `.text` 0x26E780, `$gp` 0x1AF070 **[verified]** |
  | DL, level `.text` | 0x3A1580–0x4EEF00, varying per level |

- **Level dispatch tables:**
  - `lvl.vtbl` maps an oClass to its update function. Each R&C1 entry is 12 bytes,
    `{oClass, update, ptr}`.
  - `lvl.camvtbl` entries are `{id, init, activate, update, exit}`, 20 bytes.
  - `lvl.sndvtbl` entries are `{id, fn}`, 8 bytes.
  - The boot ELF has exactly one entry of each size **[verified]**, so the boot ELF probably
    holds placeholders that the overlays replace.
- **The hero's code is part of each level's overlay** in R&C3, UYA, Gladiator and DL
  **[verified 2026-10-12]**.
  - It sits at different addresses per level.
  - UYA's title-screen level carries none of it.
  - R&C3/UYA keep the hero itself as a static global at the same address on both regions
    (0x1A4BE0 in code).
  - Overlay code regions seen: R&C3/UYA Veldin about 0x2F8000–0x524000; Gladiator/DL
    DreadZone Station about 0x3C8000–0x6CC000.
  - Addresses: `re/<serial>/symbols.toml`.
- **The resident program** (checked 2026-10-15 on our EE images, PLAN RE.2):
  - UYA's memory below about 0x1D4000 is nearly all the same in the title screen, Veldin,
    Florana and Sewers **[verified]**: resident code and constants.
  - UYA NTSC and R&C3 PAL have the same library code at the same addresses **[verified on
    37 functions]**. libuya says the same of the multiplayer program.
  - **UYA's multiplayer runs a separately linked program** **[verified by code]**. libuya's
    addresses are for it, and none hold in single player: single-player code never touches
    its data addresses, and its library functions are higher than single player's by
    shifts that vary by stretch of code: 0x32D0 (memcpy, memmove, memset), 0x74A8 and
    0x7608 (the Sif RPC calls), 0x8438 (`SifIopReset`), 0x87B0 (the 64-bit and soft-float
    helpers), 0xD680 (`matrix_unit`) and 0x57D38 (`wad_get_sectors`).
  - **DL's single player and multiplayer share one resident program** **[verified]**:
    libdl's fixed addresses for library code, its network library (about 0x1E9E000) and
    36 globals are called or used by DreadZone Station's code.
  - Gladiator PAL has most library functions at DL NTSC's addresses **[verified by code,
    53 functions]**; two higher up (`voice_enable_global_chat`, `wad_get_sectors`) are 8
    and 0x30 bytes later.
  - Matching was done on code with relocations masked (`j`/`jal` targets and the
    immediates of `lui`, `addiu`, `ori`, loads and stores), against call targets in the
    other image. The libc in UYA and R&C3 is a slightly different build from DL's, so
    most string and `printf` functions don't match. memcpy, memmove, memset and strncpy
    match 70–97%.

## Gameplay objects ("mobys")

- **The moby record:**
  - A runtime moby is a 0x100-byte record in a global array.
  - `pClass` points at class data in the level core, and `oClass` is the class id.
  - Each class has a per-level update function, which receives the moby in `$a0`.
  - Pvars are per-instance structs specific to each class. They live in the gameplay file,
    with link fix-ups and shared sub-structs.
- **The hero ("Ratchet") and gadgets:**
  - Measured movement constants, state numbering and where the hero lives in memory on the
    PAL builds: [hero-controller.md](hero-controller.md) **[verified]**.
  - Horizon's MIT `libdl` documents DL's `Player` struct (matrices, velocity, collision and
    ground state, timers, camera) and `HeroPlayerConstants` (jump and gravity tuning).
    R&C3's single-player hero follows a similar layout with some blocks shifted
    **[verified for the fields in `re/SCES-52456/hero.toml`]**.
  - It also documents `GadgetDef` (0xB0 bytes) and weapon and mod IDs.
  - These are a good reference for the Phase 1 controller's parameters.

## Level contents

- **Core** has a 0xBC-byte header with game-specific unions. It points to:
  - the GS RAM table, tfrags, occlusion, sky and collision;
  - the moby/tie/shrub class tables (`{offset, o_class, 16 texture ids}`);
  - the texture tables, particle definitions and sound remap;
  - the hero animation sets (R&C1–UYA) or light cuboids (DL);
  - the gadget table (R&C1) and the moby GS stash (GC onward).
- **Gameplay:**
  - Contents: level settings, lights, cameras, sound instances, localized help, classes and
    instances (moby/tie/shrub) with groups, the pvar table with fix-ups, volumes (cuboid,
    sphere, cylinder, pill), paths and grind paths, point lights, environment samples, the
    camera-collision grid, occlusion mappings and areas.
  - Moby instance records are 0x78 bytes in R&C1, 0x88 in GC and UYA, and 0x70 in DL.
    Tie instances are 0xE0 in R&C1 and 0x60 later.
  - DL splits gameplay into a core file, art instances and per-mission files.
- **Chunks** (GC onward): up to 3 per level, each with its own tfrags, collision and sound
  bank, which can be swapped without a full level load.
- **R&C1 gameplay data** exists in both NTSC and PAL variants, and so do its scene chunks,
  FMVs and credits. Use the NTSC/60 Hz variants.

## Rendering

- **Data path:** everything is sent to VU1 as VIF packets.
  - tfrag (terrain, 3 LODs), tie and shrub derive from Naughty Dog's Jak & Daxter
    renderers, which makes OpenGOAL's renderer work directly relevant.
- **Mobys:**
  - They are skinned with up to three joints per vertex. Blending happens on VU0 using
    matrices in scratchpad.
  - Materials include "metal" (an environment map) and bangles (optional sub-parts).
  - UYA and DL add team palettes.
- **Sky** has up to 8 shells, rotating at a per-frame rate that depends on the region.
- **R&C1 microcode** is 9 VU1 programs plus 3 VU0 images.
  - This matches the 12 DVP overlay programs in the R&C1 ELF **[verified count]**.
  - Ties, mobys and the first shrub program do about 90% of VU1 work. One VU0 moby helper
    does 99.7% of VU0 work.
  - GC edits several of these programs and adds one. We saw 13 programs in R&C2
    **[verified]**.
- **Textures:**
  - Mostly 8-bit indexed with RGBA32 or RGBA16 palettes.
  - Palette entries are CSM1-ordered (index bits 3 and 4 swapped), and pixel data may be
    GS-swizzled.
  - Alpha 0x80 means 1.0. In UYA and DL, alpha above 0x80 marks bloom.
  - The engine pages textures through GS RAM, so a VRAM address is not a stable texture ID.
- **GS conventions:**
  - R&C1 PAL renders at 512×448 and uses 12.4 fixed-point coordinates.
  - Still unknown: projection, clipping, lighting, fog rules and the terrain morph.
- **Hard spots flagged by PCSX2's GameDB fixes:**
  - half-pixel offset for post-processing bloom and native-scaling light blooms;
  - autoflush (GC);
  - accurate blending and alpha test for shadows and glass (UYA/DL);
  - partial texture invalidation, without which geometry vanishes (Aquatos, vendor and
    pause screens).
- **Hook points for a native renderer** suggested by OpenRAC: `VU1_sendChain`,
  `VU1_syncChain` and `sceGsSyncV`.

## Collision

- A 4×4×4 octant grid with a lookup tree.
- Faces are triangles or quads, each with a type byte that names the surface material.
- Separate collision groups affect only the player.

## Audio and video

- **Sound uses Sony's 989snd**, the same library family as Jak & Daxter's with MIDI removed
  **[verified library present]**.
  - It runs a 240 Hz tick on the IOP.
  - OpenGOAL reimplemented it in C++ under the ISC license, which makes it our port source.
- **Music and speech** are VAG files (`VAGp` header, big-endian, SPU ADPCM).
- **IOP modules loaded:**
  - GC: sio2man, mcman, mcserv, dbcman, sio2d, ds2u, stash, libsd and 989snd.
  - UYA and DL add padman, mtapman, the network stack, usbd, lgaud and lgkbm.
  - DL also adds streamer and astrm.
  - The purpose of "stash" is unknown.
- **FMVs** are PSS files: an MPEG-2 program stream with SPU ADPCM audio, one channel per
  language. The MPEG WAD holds 88 entries in R&C1, 50 in GC and 100 in UYA/DL, with
  separate NTSC and PAL copies.
- **In-engine cutscenes ("scenes")** are streamed chunk WADs plus speech VAGs.
- **DualShock vibration** uses wavetables (`shock/wavetable*.bin`) **[verified path strings]**.

## Timing

- **NTSC tick:** NTSC runs at 59.94 Hz. R&C1 runs one 60 Hz gameplay tick per frame, plus
  at most one catch-up tick when the RCNT1 counter shows more than one field has passed.
- **R&C1 tick order:** pad, mobys, hero, particles, camera. There is a single `rand` stream,
  seeded `srand(1234)` per level.
- **R&C1 PAL has a 50/60 Hz choice** on its main menu, and gameplay speed is the same in
  both.
- **The other PAL releases (GC, UYA, Gladiator) reportedly have no 60 Hz mode**
  **[unverified, single source]**.
- **R&C3 PAL and Gladiator PAL update the hero once per 50 Hz frame, at a fixed step**
  **[verified 2026-10-11 from four replays]**.
  - Each keeps a `u32` that counts game frames (0x15BC80 and 0x1AF180). It stopped for about
    18 vsyncs during a scripted event in R&C3, so game frames can be fewer than vsyncs.
  - Some data is stored per frame (for example sky rotation) and must be converted using the
    region's refresh rate.
- **How the PAL releases keep NTSC speed (RE.8, answered 2026-10-12) [verified on PAL,
  read in code on NTSC].**
  - Gameplay constants are written in per-second units for 60 Hz.
  - UYA/R&C3 fold them into per-frame literals at compile time: 1/60 per frame on NTSC,
    1/50 on PAL.
  - DL/Gladiator multiply them at run time by frame-time globals: `frame_dt`, its square,
    and a frame-rate ratio (50/60 on PAL, 1 on NTSC). Gladiator keeps them at
    0x21DE78–0x21DE84, DL at 0x21DDF0–0x21DDFC.
  - Frame counts are reduced to about 5/6 on PAL.
  - So PAL gameplay runs at NTSC speed in real time. NTSC-U holds the native tuning.
  - Details: `hero-controller.md`, "Frames and regions".
- **Concurrency on the original hardware:** VU0 programs started with `VCALLMS` run
  asynchronously from the EE. PCSX2 needs `EETimingHack` for all eight R&C serials.
- **Our plan:** a fixed 60 Hz simulation with interpolated rendering, which is also what the
  Dark Cloud PC port does.

## Co-op and split-screen (for Goal 4)

- **Deadlocked's story co-op** is built as separate split-screen builds of every campaign
  level: DL levels 21–35 mirror levels 1–15, each with its own overlay, running the same
  game code, which tests a co-op flag set from the level number. Player 2 replaces the
  combat bots **[code, 2026-10-28]**. The rules (what is shared, death and respawn, split
  screen, menus) are in [coop.md](coop.md).
- **Split-screen multiplayer maps** also have their own builds: UYA levels 50–55 and DL
  levels 61–74.
- **The single-player code in R&C1–UYA assumes one hero.** A co-op backport therefore needs
  native support for several heroes, cameras and HUDs (see PLAN.md, Phase 6 and RE.10).

## Misc

- **Insomniac Museum:** a hidden level in GC (level 30) and UYA (level 24). It is reached
  through a teleporter that activates at about 3 AM on the console clock
  **[unverified details]**, so the engine needs a real-time clock source.
- **R&C2 PAL reads R&C1 PAL saves** (`BESCES-50916RATCHET/save%d.bin`) for its cross-game
  bonus **[verified string]**. PCSX2's memcard filters show that R&C3 also reads R&C1 and
  R&C2 saves.

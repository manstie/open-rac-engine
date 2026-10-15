# Disc layout: preliminary inspection

First look at the six user-supplied discs in `source/`, done on 2026-10-11. Every
read was read-only, using a throwaway ISO9660 walker plus `readelf` and `strings` on
boot executables copied to a temp directory outside the repo. This file records
facts *about* the discs (serials, addresses, sizes, hashes). It contains no game
data. Re-verify anything marked *unverified* before you build on it.

## The six discs

| Folder in `source/` | Title (region) | Serial | `VMODE` | `VER` | Volume ID | Mastered |
|---|---|---|---|---|---|---|
| `Ratchet & Clank` | Ratchet & Clank (Europe, Australia) v2.00 | SCES-50916 | PAL | 2.00 | RATCHETANDCLANK | 2003-01-29 |
| `Ratchet & Clank 2` | Ratchet & Clank 2 (Europe, Australia) | SCES-51607 | PAL | 1.00 | RATCHETANDCLANK2 | 2003-10-16 |
| `Ratchet & Clank 3` | Ratchet & Clank 3 (Europe, Australia) | SCES-52456 | PAL | 1.00 | RATCHETANDCLANK3 | 2004-09-30 |
| `Ratchet & Clank 3` | Ratchet & Clank: Up Your Arsenal (USA) | SCUS-97353 | NTSC | 1.00 | RATCHETANDCLANK3 | 2004-09-23 |
| `Ratchet Gladiator` | Ratchet: Deadlocked (USA) | SCUS-97465 | NTSC | 1.00 | RATCHETDEADLOCKED | 2005-09-22 |
| `Ratchet Gladiator` | Ratchet: Gladiator (Europe, Australia) | SCES-53285 | PAL | 1.00 | RATCHETGLADIATOR | 2005-10-10 |

"Mastered" is the primary volume descriptor's creation date. The preparer ID is
`INSOMNIAC GAMES` on R&C1 and `INSOMNIAC` on the rest. The publisher ID is SCEE for the
PAL R&C1–3 discs, SCEA for UYA and SONY for Deadlocked and Gladiator. The file names
follow Redump naming.

**All six images are verified good dumps.** On 2026-10-11 each whole-image SHA-1 below was
looked up with redump.org's quick search, and the matching disc page lists the same SHA-1
(PLAN.md P0.10):

| Serial | Redump entry | Version |
|---|---|---|
| SCES-50916 | [disc 4254](http://redump.org/disc/4254/) | 2.00 |
| SCES-51607 | [disc 234](http://redump.org/disc/234/) | 1.00 |
| SCES-52456 | [disc 225](http://redump.org/disc/225/) | 1.00 |
| SCUS-97353 | [disc 11307](http://redump.org/disc/11307/) | 1.00 |
| SCUS-97465 | [disc 15053](http://redump.org/disc/15053/) | 1.00 |
| SCES-53285 | [disc 921](http://redump.org/disc/921/) | 1.00 |

### Identification hashes (SHA-1)

| Serial | Whole image | Sectors (2048 B) | Boot ELF | Boot ELF bytes | Boot ELF SHA-1 |
|---|---|---|---|---|---|
| SCES-50916 | `ef304c641c7cd426ba492707dda5974679befbc0` | 2,058,000 | `SCES_509.16` | 1,388,100 | `79956931bd62fafd8d20fa2eae796dbaf2e15e83` |
| SCES-51607 | `5a64211823e62faf33c0e86f1ad4df962a028ed0` | 1,732,688 | `SCES_516.07` | 2,878,084 | `cb59ce5396ddebb8aa539b0847f8bf66d6a47fe0` |
| SCES-52456 | `5593eab45eac8814ce4b95fa1c23510fc0a21ebf` | 2,179,744 | `SCES_524.56` | 771,648 | `e3a8f37d6033b007a80f63dfbfca9afe24f27a47` |
| SCUS-97353 | `b080139c5a94688ef9472b78d5f5585a756d1ee4` | 2,138,368 | `SCUS_973.53` | 771,008 | `601012bb03ef1213ad1253baebae8a071b8a7df7` |
| SCUS-97465 | `a85482348f741d848dd2ff4e4c3ba7c2c40f431d` | 2,127,392 | `SCUS_974.65` | 1,692,216 | `aa91b1c3b9b1a244320c47580b77342ef9856e95` |
| SCES-53285 | `092b5cf23f309b8426cdec2b8a871da939c2bbf7` | 2,158,240 | `SCES_532.85` | 1,696,356 | `2ca8d367f625001a4da12fcf466b7d218431bfd0` |

Game identification should use `SYSTEM.CNF` (`BOOT2` serial, `VER`, `VMODE`) plus the
boot-ELF hash. That is cheap, and hashing a whole 4 GB image is not.

## What the ISO9660 filesystem exposes

Most game data is **not** in the ISO9660 directory tree. The games address raw
sectors through their own table of contents (TOC). R&C2 PAL is the exception.

- **R&C1 (SCES-50916):** the tree holds only `SYSTEM.CNF` (LBA 289), `SCES_509.16` (LBA 290) and
  `IOPRP243.IMG` (LBA 968). The other ~2,057,000 sectors aren't referenced by the
  filesystem.
- **R&C2 (SCES-51607):** everything is visible.
  - `RC2.HDR` at LBA 1001 (408,444 B) is the TOC.
  - `/G/` holds the global WADs: `MPEG.WAD` (1.06 GB) and `MPEG1.WAD` (771 MB) are the
    FMVs, plus `MISC`, `HUD`, `BONUS`, `AUDIO`, `SPACE`, `SCENE`, `GADGET` and `ARMOR`.
  - `/G/` also holds `LEVELn.WAD`, `AUDIOn.WAD` and `SCENEn.WAD` for **n = 0..26**,
    which is 27 level slots.
  - `/G/DUMMY.` is the same size as `MISC.WAD`, so it's probably padding.
  - Only ~1,000 sectors are unreferenced.
- **R&C3 (SCES-52456) and UYA (SCUS-97353):** the game data is hidden. The tree contains:
  - `SYSTEM.CNF` (LBA 1000), the boot ELF and `I5BOOTN.ELF` (a second packed ELF whose
    purpose is TBD);
  - `IOPRP300.IMG`, `DNAS300.IMG` (online authentication) and `DEV9.IRX` (network
    adapter);
  - `/CNF/` (icons) and `/NETGUI/`, which is Sony's network-configuration GUI with its own
    IOP modules (`LIBSD.IRX`, `PADMAN.IRX`, `MCMAN.IRX`, `USBKB.IRX`, …);
  - `/SLY2/`, the **Sly 2 demo** (`SLY2.ELF` and `SLY2.WAL`);
  - on UYA only, `Z6TAIL.DUP`, a 32 MB padding file.
- **Deadlocked (SCUS-97465) and Gladiator (SCES-53285):** the same pattern as R&C3/UYA, but
  without `I5BOOTN.ELF` or the Sly 2 demo. Deadlocked keeps an empty `/SLY2` directory.

## Tables of contents

- **R&C1: LBA 1500 (0x5DC).** The sector starts `01 00 00 00 60 29 00 00`, which is
  `version = 1, header_size = 0x2960`. That matches the community documentation. After it
  come `(lba: u32, sector_count: u32)` pairs that chain contiguously: (1506, 9), then
  (1515, 46), then (1561, 50), and so on.
- **R&C2 through Gladiator: LBA 1001 (0x3E9),** immediately after `SYSTEM.CNF` at LBA 1000.
  - The first `u32` is `0x328` on R&C2 and `0x648` on R&C3, UYA, Deadlocked and
    Gladiator.
  - On R&C2 the second `u32` is `0xAB1` (2737), which is the LBA of `/G/MPEG.WAD`.
  - Together these two values are the first global WAD header, `{header_size, lba}`.
  - The rest of the layout is summarised in
    [engine-notes.md](engine-notes.md#disc-toc-and-wad): the remaining global WAD headers,
    then a level table of LEVEL/AUDIO/SCENE entries. Confirm it in Phase 2.

## Boot executables

All of them are ELF32 little-endian. The flags are `0x20924001` (noreorder, **R5900**,
eabi64, mips3). readelf reports the machine as "MIPS R3000", which is normal for PS2
executables.

There are two generations.

### 1. R&C1 and R&C2: plain executables with section headers

These load at `0x00100080`. R&C1 has 61 sections, entry `0x0012d868` and GP
`0x00166d00`. R&C2 has 71 sections, entry `0x00131b48` and GP `0x001af070`. The section
names describe the engine's structure:

| Section(s) | Meaning (interpretation) |
|---|---|
| `.vutext` | VU microcode staged in EE RAM (R&C1 `0x12290` B, R&C2 `0x15160` B). |
| `core.text`, `core.data`, `core.rdata`, `core.bss`, `core.lit` | The engine "core", linked separately from game code. |
| `.text`, `.data`, `.bss`, `.lit` | Game code and data. `.lit` is GP-relative small data. |
| `lvl.vtbl` (12 B), `lvl.camvtbl` (20 B), `lvl.sndvtbl` (8 B) | The interface for **per-level code overlays**. Each section is exactly one entry long: `{oClass, update, ptr}`, `{id, init, activate, update, exit}` and `{id, fn}` respectively (see engine-notes.md). |
| `.DVP.overlay..0x<vuaddr>.<id>.<line>.<n>` plus `.DVP.ovlytab` and `.DVP.ovlystrtab` | **VU1 microcode overlays** from Sony's DVP assembler, split into 2 KB chunks. |
| `legal.data`, `mc1.data` ×2 (R&C2 only) | A second load segment at `0x01800000` (legal screens and memory-card data). |

The VU overlays come in **12 programs in R&C1 and 13 in R&C2**. OpenRAC counts R&C1's
programs as 9 for VU1 and 3 for VU0. The program IDs match across the two games: 104691, 436083, 221571, 56467, 912339, 55907, 903379, 13507, 224979, 28259, 57843
and 13859. R&C2 adds 56883. Several R&C2 programs have more or larger chunks. So the
rendering microcode is shared and evolved from game to game, which is strong evidence
for one engine lineage.

### 2. R&C3, UYA, Deadlocked and Gladiator: packed executables

These load at `0x00800000` with entry `0x00800008`.

- `.text` is a `0x4fd8`-byte unpacker stub. The size is identical in all four games and
  in `I5BOOTN.ELF`.
- `.data` is `0x9dc` bytes.
- `.rodata` contains the compressed game. At **file offset `0x7080`** it starts with
  `"WAD"` and a `u32` compressed size: `0x000b53fc` on R&C3 PAL and `0x00195fbf` on
  Deadlocked.
- Entropy is about 7.0 bits/byte, against 3.3–4.1 for the R&C1/R&C2 executables.
- `.mdebug.eabi64` is present but **empty**, so there is no STABS debug info.

So the **WAD decompressor must come before any reverse engineering** of these four games.
The stub's printf strings (`# TLB spad=0 kernel=1:%d …`) show that it also sets up the TLB.

## Toolchain and libraries (from strings)

- **Language: C++.** Source names include `hud.cpp`, `loaders.cpp`, `map.cpp`,
  `gui/behaviors.cpp`, `gui/mathUtil.cpp` and `gui/memoryManagement.cpp`.
- **Compiler: a GNU-based EE toolchain.** The evidence is a newlib-style assert format,
  `.lit` sections and DVP overlay sections. The exact compiler and version are TBD.
- **Sony SDK libraries** are statically linked: libgraph (`sceGs*`), libcdvd (`sceCd*`),
  SIF RPC (`sceSif*`), libmc, and libdbc/`dbcman.irx` (DualShock). The IOP replacement
  images are `IOPRP243`, `IOPRP255` and `IOPRP300`.
- **Sound uses Sony's 989snd** (`/usr/local/989snd/ee/989snd.c`, `snd_BankLoad*`). Jak &
  Daxter used the same library family, and OpenGOAL reimplemented it natively. See
  [prior-art.md](prior-art.md).
- **Development paths:**
  - **The internal codename was "i5".** R&C1 has `host0:z:/i5/levels/level%d/npcs/scene_%d/scene.dat`
    and R&C2 has `host0:z:/i5/shock/wavetable1.bin`. The codename explains the name
    `I5BOOTN.ELF`.
  - R&C2 also has `host0:built/scenes/level%d/scene%d/scene.dat` and
    `host0:r:/rc2/shock/wavetable.bin`. The `shock` paths hold DualShock vibration
    wavetables.
  - R&C1 references `cdrom0:\CODE\I5\PARAM.TXT` and `cdrom0:\DATA\LEVELS\LEVEL`, which
    are development-time paths that don't exist on the retail disc.
- R&C2 PAL contains `BESCES-50916RATCHET/save%d.bin`, so it reads an R&C1 PAL save. This is the
  cross-game save bonus.

## Level data (UYA NTSC-U) [verified 2026-10-13]

Read with our own code (`rac_iso`, `rac_formats::{wad, level}`) and checked against the
game's loader in Ghidra (`level_load_core`, 0x428750, and `wad_decompress`, 0x433EA0;
`re/SCUS-97353/symbols.toml`). The final check: Veldin's core, unpacked from the disc by
`openrac-tool hero`, holds the hero's class and every one of his keyframes byte for byte
as the Veldin memory image has them.

- **The table of contents** (sector 1001): UYA has eight global WAD headers back to back,
  each starting `{header size, first sector}`, 0x7400 bytes in all (sizes 0x648, 0x48,
  0xBF0, 0xC30, 0x398, 0x2340, 0x3C8, 0x2AB0). Then the level table: 24 bytes per level,
  three `{sector, count}` ranges in the order audio, level, scene, each pointing at the
  file's header sector.
  - Level 0 is empty. Levels 1–36 have all three files (some are empty), and 38–55 (the
    multiplayer maps) only level and scene. The table is zero-padded after 55.
- **A level WAD's header** (its first sector, 0x60 bytes): its size (0x60), its first
  sector on the disc, the level number, the reverb, then `{sector, count}` ranges counted
  from the WAD's first sector: data, sound bank, gameplay, occlusion, three chunks and
  three chunk sound banks. Veldin: data at +0x314 for 0x1F28 sectors.
- **The data part** starts with a 0x80-byte table of `{offset, size}` pairs in bytes from
  its start. `level_load_core` uses:
  - +0x08/+0x0C: the **core header**, stored as is (0x8990 bytes on Veldin);
  - +0x48: the **core**, a WAD that unpacks whole (11.6 MB to 20 MB on Veldin) to a fixed
    address (0x91E000 on Veldin).
  - The other pairs are 3.4 MB stored as is at +0x80, three more blocks, and three WADs
    of 16 bytes (empty). Not identified yet.
- **The core header:**
  - +0x18 and +0x1C: the moby class count (227 on Veldin) and the table's offset. Each
    entry is 0x20 bytes: the class's offset in the core, its class number, two words, and
    16 texture indices. Class 0 is the hero.
  - +0x78: the hero's sequence table, one core offset per sequence (0 where a level
    leaves one out). The core keeps his sequences outside his class.
  - +0x88 and +0x8C: the core's packed and unpacked sizes. (+0x84 is a count the loader
    passes with the GS RAM table at +0x00/+0x04 when it sends the textures to the GS.)
  - Between them, `{count, offset}` pairs of tables in the core header [data, from the
    values; not read in code yet]: +0x20 tie classes, +0x28 shrub classes, then textures
    for tfrags (+0x30), mobys (+0x38, 143 on Veldin, 16 bytes each), ties, shrubs,
    particles and effects (+0x58). The texture data itself is probably the data part's
    first block (3.4 MB at +0x80), which the loader sends to the GS.
  - At load the game also fills a memory map of the level's regions (0x1EA9C0): the core,
    the moby classes, the hero's sequences, the mobys and the armor mesh.
- **DL's and Gladiator's table of contents** [verified 2026-10-15] starts the same way:
  eight global WAD headers back to back (sizes 0x648, 0x50, 0x2A8, 0x68, 0x228, 0xA870,
  0xF88, 0x68; 0xC430 bytes in all), the fifth the skins' (`moby-format.md`, "DL's
  skins"). What follows isn't UYA's 24-byte level table and isn't read yet.
- **The hero on other levels:** every level with a hero carries the same 111-joint
  skeleton and only the sequences it uses. Veldin and level 2 have all 127, level 6 has
  88, level 30 six, and the multiplayer maps have no class 0. His mesh isn't in the core
  at all; it comes from the armor WAD (`moby-format.md`, "The armor WAD"), the fifth of
  the eight global headers.

### WAD compression [verified 2026-10-13]

Read in UYA's `wad_decompress` (0x433EA0) and confirmed by unpacking every WAD of
Veldin's level data:

- The header is 16 bytes: `"WAD"`, the whole WAD's size (header included) as an unaligned
  `u32`, then padding. A WAD of 16 bytes is empty.
- The packets run in **0x2000-byte blocks** counted from the end of the header. The
  game DMAs one block at a time into scratchpad, so a packet never crosses a block.
- A first byte above 0x11 is a literal run of (byte − 0x11). After that, by flag byte:

  | Flag | Packet |
  |---|---|
  | 0x00–0x0F | Literal run of flag + 3 bytes, or of the next byte + 18 if the flag is 0. Not allowed straight after literals (the game traps). |
  | 0x10–0x1F | Far match: length flag & 7 (or the next byte + 7) + 2, distance ((flag & 8) << 11) + (u16 >> 2) + 0x4000. |
  | 0x20–0x3F | Medium match: length flag & 0x1F (or the next byte + 0x1F) + 2, distance (u16 >> 2) + 1. |
  | 0x40–0xFF | Short match: length (flag >> 5) + 1, distance ((flag >> 2) & 7) + (next byte << 3) + 1. |

- A match's last header byte (the flag for short matches, the low distance byte
  otherwise) carries 0–3 trailing literals in its low two bits.
- A far match whose distance field is 0 copies nothing. With length 1 (`11 xx 00`) it's an
  empty packet whose trailing literals still count; it lets a literal run follow
  literals and starts blocks. With any other length (`12 00 00`) it ends the block: the
  rest is padding (0xEE) and the next packet starts the next block.

## Implications for the plan

1. The disc layer needs two access paths: ISO9660 (to find `SYSTEM.CNF`, the
   executables and R&C2's `/G/` files) and raw sector reads driven by each game's TOC.
2. The TOC formats come in **two generations** (R&C1, then R&C2 and later), and so do the
   executable formats (plain, then WAD-packed). That fits a single engine with per-game
   profiles.
3. A **WAD decompressor** is the first codec to write. Phase 2 depends on it, and so does
   any automated reverse engineering of the packed executables. Until it exists, PCSX2
   savestates (imported into Ghidra) supply the unpacked code.
4. R&C1 and R&C2 keep section headers. That makes them the easiest first targets for
   mapping engine versus game code and the level-overlay interface (`lvl.*vtbl`).
5. The VU1 microcode is cleanly separated and catalogued, so the renderer's VU programs can
   be enumerated per game. That matters for a high-level native renderer (see PLAN.md).
6. 989snd means existing open-source work on that sound library can guide the audio
   pipeline.

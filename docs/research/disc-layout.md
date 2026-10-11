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
follow Redump naming. Checking the hashes below against redump.org is task P0.10 in PLAN.md.

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

# Disc layout: preliminary inspection

First look at the user-supplied discs in `source/`: six on 2026-10-11, and the NTSC-U
R&C1 and GC discs on 2026-10-15. Every read was read-only, using a throwaway ISO9660 walker plus `readelf` and `strings` on
boot executables copied to a temp directory outside the repo. This file records
facts *about* the discs (serials, addresses, sizes, hashes). It contains no game
data. Re-verify anything marked *unverified* before you build on it.

## The eight discs

| Folder in `source/` | Title (region) | Serial | `VMODE` | `VER` | Volume ID | Mastered |
|---|---|---|---|---|---|---|
| `Ratchet & Clank` | Ratchet & Clank (USA) | SCUS-97199 | NTSC | 1.00 | RATCHETANDCLANK | 2002-10-02 |
| `Ratchet & Clank` | Ratchet & Clank (Europe, Australia) v2.00 | SCES-50916 | PAL | 2.00 | RATCHETANDCLANK | 2003-01-29 |
| `Ratchet & Clank 2` | Ratchet & Clank: Going Commando (USA) v2.00, Greatest Hits | SCUS-97268 | NTSC | 2.00 | RATCHETANDCLANK2 | 2004-02-03 |
| `Ratchet & Clank 2` | Ratchet & Clank 2 (Europe, Australia) | SCES-51607 | PAL | 1.00 | RATCHETANDCLANK2 | 2003-10-16 |
| `Ratchet & Clank 3` | Ratchet & Clank 3 (Europe, Australia) | SCES-52456 | PAL | 1.00 | RATCHETANDCLANK3 | 2004-09-30 |
| `Ratchet & Clank 3` | Ratchet & Clank: Up Your Arsenal (USA) | SCUS-97353 | NTSC | 1.00 | RATCHETANDCLANK3 | 2004-09-23 |
| `Ratchet Gladiator` | Ratchet: Deadlocked (USA) | SCUS-97465 | NTSC | 1.00 | RATCHETDEADLOCKED | 2005-09-22 |
| `Ratchet Gladiator` | Ratchet: Gladiator (Europe, Australia) | SCES-53285 | PAL | 1.00 | RATCHETGLADIATOR | 2005-10-10 |

"Mastered" is the primary volume descriptor's creation date. The preparer ID is
`INSOMNIAC GAMES` on R&C1 PAL, `INSOMNIACGAMES` on R&C1 USA and `INSOMNIAC` on the rest.
The publisher ID is SCEE for the PAL R&C1–3 discs, SCEA for R&C1 USA and UYA, and SONY for
GC USA, Deadlocked and Gladiator. The file names follow Redump naming.

The GC USA disc is the v2.00 Greatest Hits release (redump serial `SCUS-97268GH`, boot ELF
dated 2004-01-20), not the original v1.01. The R&C1 USA image is the one redump lists for
both the original and the Greatest Hits editions.

**All eight images are verified good dumps.** Each whole-image SHA-1 below was looked up
with redump.org's quick search (2026-10-11; the NTSC-U R&C1 and GC on 2026-10-15), and the
matching disc page lists the same SHA-1 (PLAN.md P0.10):

| Serial | Redump entry | Version |
|---|---|---|
| SCUS-97199 | [disc 5362](http://redump.org/disc/5362/) | 1.00 |
| SCES-50916 | [disc 4254](http://redump.org/disc/4254/) | 2.00 |
| SCUS-97268 | [disc 36578](http://redump.org/disc/36578/) | 2.00 |
| SCES-51607 | [disc 234](http://redump.org/disc/234/) | 1.00 |
| SCES-52456 | [disc 225](http://redump.org/disc/225/) | 1.00 |
| SCUS-97353 | [disc 11307](http://redump.org/disc/11307/) | 1.00 |
| SCUS-97465 | [disc 15053](http://redump.org/disc/15053/) | 1.00 |
| SCES-53285 | [disc 921](http://redump.org/disc/921/) | 1.00 |

### Identification hashes (SHA-1)

| Serial | Whole image | Sectors (2048 B) | Boot ELF | Boot ELF bytes | Boot ELF SHA-1 |
|---|---|---|---|---|---|
| SCUS-97199 | `aee9275384f739c9b24cd5de6f7e200292db7b01` | 2,057,664 | `SCUS_971.99` | 1,383,028 | `72fd1de379dafd8f243dceeb3be50980f3fd3cd5` |
| SCES-50916 | `ef304c641c7cd426ba492707dda5974679befbc0` | 2,058,000 | `SCES_509.16` | 1,388,100 | `79956931bd62fafd8d20fa2eae796dbaf2e15e83` |
| SCUS-97268 | `eeba0fff2118b14cbf838e6638ef7af3e72c0c31` | 1,867,488 | `SCUS_972.68` | 2,621,256 | `4124a79c1e5a1ceb56f514df0dbbc11f072af56f` |
| SCES-51607 | `5a64211823e62faf33c0e86f1ad4df962a028ed0` | 1,732,688 | `SCES_516.07` | 2,878,084 | `cb59ce5396ddebb8aa539b0847f8bf66d6a47fe0` |
| SCES-52456 | `5593eab45eac8814ce4b95fa1c23510fc0a21ebf` | 2,179,744 | `SCES_524.56` | 771,648 | `e3a8f37d6033b007a80f63dfbfca9afe24f27a47` |
| SCUS-97353 | `b080139c5a94688ef9472b78d5f5585a756d1ee4` | 2,138,368 | `SCUS_973.53` | 771,008 | `601012bb03ef1213ad1253baebae8a071b8a7df7` |
| SCUS-97465 | `a85482348f741d848dd2ff4e4c3ba7c2c40f431d` | 2,127,392 | `SCUS_974.65` | 1,692,216 | `aa91b1c3b9b1a244320c47580b77342ef9856e95` |
| SCES-53285 | `092b5cf23f309b8426cdec2b8a871da939c2bbf7` | 2,158,240 | `SCES_532.85` | 1,696,356 | `2ca8d367f625001a4da12fcf466b7d218431bfd0` |

**Identification** [code, verified on all eight discs 2026-10-15]:
`rac_iso::Release::identify` takes the boot executable's name from `SYSTEM.CNF`'s `BOOT2`
and pins the build by that file's SHA-1 (the table above). That is cheap, and hashing a
whole 4 GB image is not. A known serial with another hash (GC USA v1.01, say) is refused
with its SHA-1, because every address we know belongs to one build. `openrac-tool identify
[images or folders]` says what each image is.

- `SYSTEM.CNF` is at LBA 289 on both R&C1 discs and LBA 1000 on the other six. Its lines
  end in CRLF, and on all but R&C1's discs each value is followed by a space
  (`VER = 1.00 `).
- The disc tests check each release's `VER` and `VMODE`, the first words at its TOC
  sector, and that exactly the R&C3, UYA, DL and Gladiator boot executables are stubs
  entered at `0x800008` (`rac_iso`, `every_disc_identifies`).

## What the ISO9660 filesystem exposes

Most game data is **not** in the ISO9660 directory tree. The games address raw
sectors through their own table of contents (TOC). R&C2 is the exception.

- **R&C1 (SCES-50916):** the tree holds only `SYSTEM.CNF` (LBA 289), `SCES_509.16` (LBA 290) and
  `IOPRP243.IMG` (LBA 968). The other ~2,057,000 sectors aren't referenced by the
  filesystem.
- **R&C1 USA (SCUS-97199):** the same three files, with `SCUS_971.99` at LBA 290 and
  `IOPRP243.IMG` at LBA 966.
- **R&C2 (SCES-51607):** everything is visible.
  - `RC2.HDR` at LBA 1001 (408,444 B) is the TOC.
  - `/G/` holds the global WADs: `MPEG.WAD` (1.03 GiB) and `MPEG1.WAD` (771 MiB) are the
    FMVs, plus `MISC`, `HUD`, `BONUS`, `AUDIO`, `SPACE`, `SCENE`, `GADGET` and `ARMOR`.
  - `/G/` also holds `LEVELn.WAD`, `AUDIOn.WAD` and `SCENEn.WAD` for **n = 0..26**,
    which is 27 level slots.
  - `/G/DUMMY.` is the same size as `MISC.WAD`, so it's probably padding.
  - Only ~1,000 sectors are unreferenced.
- **R&C2 USA v2.00 (SCUS-97268):** the same layout: `SYSTEM.CNF` at LBA 1000, `RC2.HDR` at
  LBA 1001 (408,444 B, the same size as PAL's), `SCUS_972.68` at LBA 1201 and
  `IOPRP255.IMG` at 2481. `/G/` has the same WADs and 27 level slots, but **three** FMV
  WADs, `MPEG.WAD` (1.00 GiB, LBA 2611), `MPEG1.WAD` (1.21 GiB) and `MPEG2.WAD`
  (109 MiB), against PAL's two. `AUDIO.WAD` is 34 MiB against PAL's 104 MiB, which carries
  more languages.
- **R&C3 (SCES-52456) and UYA (SCUS-97353):** the game data is hidden. The tree contains:
  - `SYSTEM.CNF` (LBA 1000), the boot ELF and `I5BOOTN.ELF` (a second packed ELF, the
    network start-up; see "Packed executables");
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
  (1515, 46), then (1561, 50), and so on. R&C1 USA's TOC starts with the same words.
- **R&C2 through Gladiator: LBA 1001 (0x3E9),** immediately after `SYSTEM.CNF` at LBA 1000.
  - The first `u32` is `0x328` on R&C2 and `0x648` on R&C3, UYA, Deadlocked and
    Gladiator.
  - On R&C2 the second `u32` is the LBA of `/G/MPEG.WAD`: `0xAB1` (2737) on PAL and
    `0xA33` (2611) on USA.
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

These load at `0x00100080`. GP is the `.reginfo` section's value.

| Boot ELF | Sections | Entry | GP | `.lit` | `.text` |
|---|---|---|---|---|---|
| R&C1 USA `SCUS_971.99` | 61 | `0x0012d728` | `0x00166c00` | `0x0015ef00` | `0x001e8d00` |
| R&C1 PAL `SCES_509.16` | 61 | `0x0012d868` | `0x00166d00` | `0x0015f000` | `0x001e9080` |
| R&C2 USA `SCUS_972.68` | 71 | `0x00131ae8` | `0x001aeff0` | `0x001a7c80` | `0x0026ea00` |
| R&C2 PAL `SCES_516.07` | 71 | `0x00131b48` | `0x001af070` | `0x001a7d00` | `0x0026e780` |

Each USA executable has the same sections as its PAL counterpart, a few hundred bytes
apart. The section names describe the engine's structure:

| Section(s) | Meaning (interpretation) |
|---|---|
| `.vutext` | VU microcode staged in EE RAM (R&C1 `0x12290` B, R&C2 `0x15160` B). |
| `core.text`, `core.data`, `core.rdata`, `core.bss`, `core.lit` | The engine "core", linked separately from game code. |
| `.text`, `.data`, `.bss`, `.lit` | Game code and data. `.lit` is GP-relative small data. |
| `lvl.vtbl` (12 B), `lvl.camvtbl` (20 B), `lvl.sndvtbl` (8 B) | The interface for **per-level code overlays**. Each section is exactly one entry long: `{oClass, update, ptr}`, `{id, init, activate, update, exit}` and `{id, fn}` respectively (see engine-notes.md). |
| `.DVP.overlay..0x<vuaddr>.<id>.<line>.<n>` plus `.DVP.ovlytab` and `.DVP.ovlystrtab` | **VU1 microcode overlays** from Sony's DVP assembler, split into 2 KB chunks. |
| `legal.data`, `mc1.data` ×2 (R&C2 only) | A second load segment at `0x01800000` (legal screens and memory-card data). |

The VU overlays come in **12 programs in R&C1 and 13 in R&C2**, the same IDs on both
regions' discs. OpenRAC counts R&C1's
programs as 9 for VU1 and 3 for VU0. The program IDs match across the two games: 104691, 436083, 221571, 56467, 912339, 55907, 903379, 13507, 224979, 28259, 57843
and 13859. R&C2 adds 56883. Several R&C2 programs have more or larger chunks. So the
rendering microcode is shared and evolved from game to game, which is strong evidence
for one engine lineage.

### 2. R&C3, UYA, Deadlocked and Gladiator: packed executables [verified 2026-10-15]

These load at `0x00800000` with entry `0x00800008`: a small loader stub that carries the
real program. `openrac-tool unpack-elf` unpacks it into a plain ELF (`rac_formats::elf`,
PLAN.md P2.5).

- **The stub:** `.text` is `0x4fd8` bytes, the same size in all four games and in
  `I5BOOTN.ELF`; `.data` is `0x9dc` bytes; `.mdebug.eabi64` is present but **empty**, so
  there is no STABS debug info. The file's entropy is about 7.0 bits/byte, against 3.3–4.1
  for the R&C1/R&C2 executables. Its TLB set-up (0x8026F0, with printf strings like `# TLB
  spad=0 kernel=1:%d …`) is called only from a `main` at 0x800698 that nothing on the
  entry's path calls, so it is probably linked in unused.
- **How it loads the program** (read in UYA NTSC-U's stub, whose loader is at `0x800488`;
  the four games' stubs are the same size):
  1. The entry clears the registers, puts the stack at `0x1FFF000`, calls the loader and
     jumps to the address it returns.
  2. The loader unpacks the WAD at `0x806080` (`0x680` bytes into `.rodata`, **file offset
     `0x7080`**) to `0x8BC700`, just past the stub's own `.bss`. Its decompressor is the
     levels' WAD codec, DMAing each 0x2000-byte block to scratchpad. The WAD header's size
     word counts the whole WAD: `0xB53FC` on R&C3 PAL, `0x195FBF` on Deadlocked.
  3. The unpacked program is a list of records: four `u32`s `{address, size, type,
     entry}`, then `size` bytes. The loader copies each record's bytes to its address and
     stops at the first record whose entry differs from the first record's, then returns
     that first entry. There is no count: on every disc the records fill the unpacked data
     exactly, and the loader stops on the zeroed memory after it. (When a size or address
     isn't a multiple of 8 it copies by words and copies one word too many, into the
     padding after the section.)
- **The records are the original ELF's allocated sections.** `type` is the ELF section
  type: 1 (`PROGBITS`), 8 (`NOBITS`, whose bytes are carried anyway, all zero) or
  `0x70000006` (`MIPS_REGINFO`, 0x18 bytes, the last word being `$gp`). The first fourteen
  have the types and order of GC's plain boot ELF's allocated sections, and the three level
  tables hold one entry each (12, 20 and 8 bytes), as in R&C1 and GC. So we give them GC's
  names; the stub itself carries none [interpretation].

  | Section | UYA NTSC-U | R&C3 PAL | DL NTSC-U | Gladiator PAL |
  |---|---|---|---|---|
  | `.reginfo` | 0x100000 +0x18 | 0x100000 +0x18 | 0x100000 +0x18 | 0x100000 +0x18 |
  | `.vutext` | 0x100080 +0x16EB0 | 0x100080 +0x16EB0 | 0x100080 +0x1A0E0 | 0x100080 +0x1A0E0 |
  | `core.text` | 0x116F80 +0x264A0 | 0x116F80 +0x264A0 | 0x11A180 +0x49990 | 0x11A180 +0x49A38 |
  | `core.data` | 0x13D480 +0x12B90 | 0x13D480 +0x12B90 | 0x163B80 +0x2E350 | 0x163C00 +0x2E310 |
  | `core.rdata` | 0x150080 +0x2310 | 0x150080 +0x2310 | 0x191F00 +0x13248 | 0x191F80 +0x13248 |
  | `core.bss` | 0x152400 +0x82980 | 0x152400 +0x82980 | 0x1A5180 +0x78AB8 | 0x1A5200 +0x78AB8 |
  | `core.lit` | 0x1D4D80 +0x8C0 | 0x1D4D80 +0x8C0 | 0x21DC80 +0x4B8 | 0x21DD00 +0x4C0 |
  | `.lit` | 0x1D5680 +0x4230 | 0x1D5680 +0x41F0 | 0x21E180 +0x2EF0 | 0x21E200 +0x2F10 |
  | `.bss` | 0x1D9900 +0x142578 | 0x1D9880 +0x142578 | 0x221080 +0x1A4908 | 0x221180 +0x1A4808 |
  | `.data` | 0x31BE80 +0x65110 | 0x31BE00 +0x65100 | 0x3C5A00 +0x2B404 | 0x3C5A00 +0x2B44C |
  | `lvl.vtbl` | 0x381000 +0xC | 0x380F00 +0xC | 0x3F0E80 +0xC | 0x3F0E80 +0xC |
  | `lvl.camvtbl` | 0x381080 +0x14 | 0x380F80 +0x14 | 0x3F0F00 +0x14 | 0x3F0F00 +0x14 |
  | `lvl.sndvtbl` | 0x381100 +0x8 | 0x381000 +0x8 | 0x3F0F80 +0x8 | 0x3F0F80 +0x8 |
  | `.text` | 0x381180 +0x71438 | 0x381080 +0x716F8 | 0x3F1000 +0x15C6D8 | 0x3F1000 +0x15DC40 |
  | a second reginfo, all zero | 0x3F2600 | 0x3F2780 | 0x54D700 | 0x54EC80 |
  | unnamed | 0x1800000 +0x6374 | 0x1800000 +0x6374 | 0x1E9A000 +0x97330 | 0x1E9A000 +0x97330 |
  | unnamed | 0x1806380 +0x2F3C0 | 0x1806380 +0x2F3C0 | 0x1F31380 +0x6C20 | 0x1F31380 +0x6C20 |
  | unnamed | 0x1835780 +0x34318 | 0x1835780 +0x34318 | | |

  On R&C3 and UYA the section at 0x1800000 starts like GC's `legal.data`, so the three
  are probably `legal.data` and two `mc1.data`. On DL and Gladiator the one at 0x1E9A000
  is code.
- **Entry and `$gp`:** the entry is crt0, in `core.text`, which loads `$gp` itself
  (DL: `lui 0x22; addiu 0x597F`), matching `.reginfo`.

  | Executable | Entry | `$gp` | `.text` |
  |---|---|---|---|
  | R&C3 PAL `SCES_524.56` | 0x13ADA8 | 0x1DC8B0 | 0x381080 +0x716F8 |
  | UYA NTSC-U `SCUS_973.53` | 0x13ADA8 | 0x1DC8B0 | 0x381180 +0x71438 |
  | DL NTSC-U `SCUS_974.65` | 0x1574E8 | 0x22597F | 0x3F1000 +0x15C6D8 |
  | Gladiator PAL `SCES_532.85` | 0x1574E8 | 0x2259FF | 0x3F1000 +0x15DC40 |
  | R&C3 PAL `I5BOOTN.ELF` | 0x192D18 | 0x247FC0 | 0x27B100 +0x25938 |
  | UYA NTSC-U `I5BOOTN.ELF` | 0x192E08 | 0x248140 | 0x27B200 +0x257C8 |

- **Checked against memory:** on UYA NTSC-U's title screen (`cache/ghidra/ee/
  uyantsc-title.ee.bin`) every code and read-only section, and the sections past `.text`,
  match EE memory byte for byte, and so does the ELF `unpack-elf` writes; only writable
  data differs. In levels (UYA and R&C3 on Veldin, DL and Gladiator on DreadZone Station)
  `.vutext`, `core.text` and `core.rdata` still match on all four, while `.lit`, `.data`,
  the `lvl.*` tables and `.text` are mostly replaced and the sections past them are
  overwritten. So **the boot program's `.text` is the title screen's**, and each level's
  overlay replaces it ([engine-notes.md](engine-notes.md#code-layout-and-overlays)).
- **`I5BOOTN.ELF`** (R&C3 and UYA) is the **network start-up**, packed the same way. Its
  strings are network configuration (`ezNetCnf*`, `ezNetCtl*`), `cdrom0:\DNAS300.IMG`,
  `cdrom0:\DEV9.IRX` and ISP connection messages [strings].

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
- Both R&C2 executables contain `BASCUS-97199RATCHET/save%d.bin`,
  `BESCES-50916RATCHET/save%d.bin` and `BISCPS-15037RATCHET/save%d.bin`, so each reads an
  R&C1 save from the USA, PAL or Japanese release. This is the cross-game save bonus.
- R&C1 USA has the same development paths as R&C1 PAL.

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
  - +0x10/+0x14: the **GS RAM image**, stored as is (0x79800 bytes on Veldin), as DL's
    at +0x18: DL's table has one more pair before it [verified 2026-10-20: the
    heli-pack's level textures decode from it with DL's rules, "Level textures" below].
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
- **The guns** [code, data, 2026-10-21]: the seventh global header (0x3C8 bytes) is the
  gadget WAD, 48 entries of {class, file, sound bank} that the game streams as guns are
  given and drawn (`moby-format.md`, "The gadget WAD").

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

## Level data (DL NTSC-U and Gladiator PAL) [verified 2026-10-15]

Read in DL's loader in our DreadZone Station image (`dl_level_read_headers` 0x159AB0,
`dl_level_load_steps` 0x578530, `dl_level_load_core` 0x578940; `re/SCUS-97465/symbols.toml`)
and with `rac_formats::level` (`Toc::read_dl`, `LevelHeader::read_dl`, `Core::read_dl`)
on both discs, whose disc tests read every level.
- **The table of contents** is 0x1A sectors from UYA's place: eight global WAD headers,
  then UYA's level table, 24 bytes a level (audio, level, scene). Levels 0 (the menu),
  1–15 (the story's planets), 21–35 (the same for two players) and 41–74 (the
  multiplayer maps) have level files, 47 in all on both discs. Levels 20 to 39 have no
  audio or scene of their own: the game uses level − 20's.
- **A level WAD's header** is 0xC68 bytes: its size, its first sector, the level number,
  then from +0x18 the data and the sound bank as `{sector, count}`. When it reloads a
  level's gameplay alone the game unpacks +0x58, which is what the data's own gameplay
  WAD gives on a full load. The chunks and 128 missions after aren't read. DL has no
  occlusion range where UYA has one.
- **The data's table** is UYA's with one more pair at +0x08 (the level's code): the core
  header at +0x10, the GS RAM image at +0x18 (stored as is) and the core WAD at +0x50.
- **The core's class table** has 0x20 bytes a class, as UYA's: the offset, the class
  number, at +8 where its textures come from (0 the level's own; 1–3 the skin WAD's
  first, second or third texture group), and from +0x10 sixteen indices into the level's
  moby texture table (0xFF after the last).
- **Level textures:** the core header lists the moby textures at +0x38 (count) and +0x3C
  (offset), 16 bytes each: where the pixels are, width, height, a kind (0 kept in the GS
  for the whole level, its pixels in the GS RAM image; 1 or 3 kept in EE memory, from the
  core's texture base at header + 0x60, stored in the GS's 32-bit order), the palette
  and a small mipmap in the GS RAM image, in 0x100-byte blocks. The GS RAM table (header
  +0x00 count, +0x04 offset, 16 bytes an entry) lists the palettes and mipmaps sent first;
  the entry after them starts the textures the GS keeps.

## Implications for the plan

1. The disc layer needs two access paths: ISO9660 (to find `SYSTEM.CNF`, the
   executables and R&C2's `/G/` files) and raw sector reads driven by each game's TOC.
2. The TOC formats come in **two generations** (R&C1, then R&C2 and later), and so do the
   executable formats (plain, then WAD-packed). That fits a single engine with per-game
   profiles.
3. A **WAD decompressor** is the first codec to write. Phase 2 depends on it, and so does
   any automated reverse engineering of the packed executables. Done (P2.4, P2.5):
   `openrac-tool unpack-elf` gives Ghidra the boot program (the engine core and the title
   screen) as an ELF. Level code still comes from savestates until P2.6 extracts the
   overlays.
4. R&C1 and R&C2 keep section headers. That makes them the easiest first targets for
   mapping engine versus game code and the level-overlay interface (`lvl.*vtbl`).
5. The VU1 microcode is cleanly separated and catalogued, so the renderer's VU programs can
   be enumerated per game. That matters for a high-level native renderer (see PLAN.md).
6. 989snd means existing open-source work on that sound library can guide the audio
   pipeline.

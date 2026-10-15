# Gameplay files

What a level's gameplay file holds, for PLAN.md P3.10. Started 2026-10-15 on UYA NTSC-U,
with the grind paths: the rails, which in UYA's single player are zip lines
(`hero-controller.md`, "Zip lines").

## Status and method

- **The disc:** each level WAD's header points at the gameplay file (`disc-layout.md`,
  "Level data"). It is a WAD that unpacks whole, 1.9 to 4.0 MB on UYA.
  `rac_formats::gameplay::Gameplay::read_uya` reads it.
- **Code read in Ghidra** (addresses in `re/SCUS-97353/symbols.toml`, Veldin's build):
  `level_load_gameplay` (0x4252F0, called from `level_load_core`) loads the gameplay
  file and unpacks it, then copies its blocks into the level's lists.
- **Checks:** the rails read from the disc match the ones the game loaded on Florana,
  found by searching a savestate's memory for the first rail's points.
- **Labels** as in `hero-controller.md`: **[verified]** (read in code and checked against
  the data), **[code]**, **[data]** (seen consistently in the data, not read in code).

## Loading [code, UYA NTSC]

- The level header in memory is at 0x1685E0 in Veldin's build: +0x04 is the WAD's first
  sector and +0x20/+0x24 the gameplay file's `{sector, count}`, as on the disc.
- `level_load_gameplay` reads that range from the disc, unpacks it to the address held
  at 0x1EA9CC, and keeps that address as the file's base.
- The file starts with a header of block offsets from its base, 0 for a block the level
  doesn't have. The loader walks them in turn, copying each block's records into a list
  on the level heap, 64-byte aligned.

## Grind paths [verified, UYA NTSC]

- **The header's word at +0x7C** is the block's offset. 0 means no rails.
- **The block:**

  | Offset | What |
  |---|---|
  | +0x00 | The rail count |
  | +0x04 | Where the spline data starts, from the block's start |
  | +0x08 | The spline data's size |
  | +0x0C | 0 |
  | +0x10 | One 0x20-byte record per rail |
  | then | One word per rail: its spline's offset in the spline data |

- **A record:** the bounding sphere's centre and radius (four floats), then a word the
  loader copies to the list entry's +0x14, then three words it skips. The word is 0 on
  every UYA rail; the game passes it to its spline search with the spline.
  - The list entry the game builds is the sphere, the spline's address at +0x10, and
    that word at +0x14.
- **A spline:** its point count, three zero words, then the points, 16 bytes each:
  x, y, z and a fourth float.
  - **The fourth float [data]:** the length of the segment the point starts. The last
    point's holds the spline's whole length instead (Qwark's Hideout: nine segments of
    5.633 u, then 50.697).
- **The game moves along a rail in the order of its points.**

## UYA's rails [data, 2026-10-15]

From `openrac-tool rails` on the UYA NTSC-U disc. All flag words are 0.

| Level | Rails | What they are |
|---|---|---|
| 2 Florana | 3: 83, 27 and 16 u, all downhill | Zip lines (the first seen on screen and traced; the build has no grind code) |
| 11 Obani Gemini | 1: 131 u in 120 points | Probably a zip line: the level has no grind animations |
| 17 Crash Site | 1: 213 u in 17 points, downhill over a chasm | A zip line (seen on screen) |
| 19 Qwark's Hideout | 1: 51 u in 10 points | Probably a zip line: the level has no grind animations |
| 30, 32–35 Qwark vid-comics | 1 to 8, mostly vertical (x and y fixed) | Not checked |

- The other levels on the disc have none, the multiplayer maps (39–55) included.

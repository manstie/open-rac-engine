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

## Cameras [verified, UYA NTSC, 2026-10-16]

The level's cameras: one record per camera the level offers, follow camera included. What
the camera does with them is in `camera.md`, "Camera volumes and splines".
- **The header's word at +0x08** is the block's offset: a count, three zero words, then
  one 0x20-byte record per camera:

  | Offset | What |
  |---|---|
  | +0x00 | The class: a camera mode number, as in the camera mode table (0x2F7700) |
  | +0x04 | The position, three floats |
  | +0x10 | The rotation, three floats (radians) |
  | +0x1C | The pvar index, -1 for none |

- **The loader** (`level_load_gameplay`) copies the records to a list (pointer 0x1DF884,
  count 0x1DF888 in Veldin's build), reordered as position, class (+0x0C), rotation, and a
  pointer to the camera's pvars (+0x1C). `camera_system_init` (0x3F3A38) then gives each
  camera an update object of 0xB0 bytes, in a table of 48 per player at 0x1E3050 (+0x2100
  per player), and writes that object's address into pvar +0x04. Nothing checks the 48.
- **Every level** has cameras of classes 0, 4, 5, 7, 0x17 and 0x20; most add others
  ("UYA's cameras" below). Classes missing from a level build's mode table get no
  functions (`camera_mode_index`, 0x3F38D0, returns the table's end).
- **Checked** in five savestates (Veldin, Florana, Starship Phoenix, Aquatos sewers,
  Crash Site; `openrac-tool cameras --check-ee`): every camera was found in memory by
  its contents, with its class, and its pvars as on the disc except the words the
  camera's own code changes while it runs (listed below).

### The trigger fields [code; data]

Every camera's pvars start with these 0x20 bytes. Wrench's DL `cameraShared` has the same
layout and names.

| Offset | What |
|---|---|
| +0x00 | Float. 8.0 on the 0x17 and 0x11 cameras, 0 elsewhere; not read yet ("lead strength" in Wrench) |
| +0x04 | Filled in at load: the camera's update object |
| +0x08 | Sphere index, -1 for none |
| +0x0C | Cuboid index, -1 for none |
| +0x10 | Cylinder index, -1 for none |
| +0x14 | Path index, -1 for none |
| +0x18 | Float, 1.5 on every camera seen; not read yet ("focus height") |
| +0x1C | Byte: priority. 5 on the follow camera (class 0), 4 on 0x17, 6 on the rest |
| +0x1D | Byte: how the switch to this camera blends (0 to 6; `camera.md`) |
| +0x1E | Byte: not read yet ("blend type out") |
| +0x1F | Byte: when the camera may take over (activation type, 0 to 7; `camera.md`) |

- The classes that test the hero against a shape (0x13, 0x17, activation 4) try the
  cuboid, then the cylinder, then the sphere, then (0x13 only) the path.

### The path camera, class 0x13 [code; data]

A camera that flies along one of the level's paths ("Paths" below) while looking at
another path or a moby, then hands back. 0xB0 bytes of pvars:

| Offset | What |
|---|---|
| +0x20 | The path it moves along |
| +0x24 | The path it looks at, at the same fraction of its length, -1 for none |
| +0x28 | A moby link to look at instead (the moby's UID in the file, its index once loaded) |
| +0x2C | Three floats added to that moby's position |
| +0x38, +0x39 | Bytes set while running: started and finished |
| +0x3A | Halfword: the flight's length in frames when positive; the speed is then the path's length over it |
| +0x3C | Float: the speed along the path in u/s (written by the camera when +0x3A sets it) |
| +0x40 | Byte: tested at the end of the flight; not read yet |
| +0x41 | Byte: 0 holds the hero in state 116 (0x74) for the flight (0x4FC518) |
| +0x42, +0x43 | Signed bytes: -1, or the hero state (0x1A71A4) and state type (0x1A71AC) it needs to start |
| +0x44 | Byte set once initialised |
| +0x46, +0x48 | Halfword and word: hold times at the end; not read in full |
| +0x4C | Probably a camera (index in the list) to start when this one ends, -1 for none |
| +0x5C | A moby (by index) whose class function is called at the end, -1 for none |
| +0x60 | Eight floats, written at start: fields of view in degrees, from the path |
| +0x80 | Eight words, written at start: the path points those apply at |
| +0xA0 | A progress flag (byte 0x1BBC40 + flag + level × 16, bit 0x80) that stops it for good once set, -1 for none |

- **Data:** 31 path cameras on the disc (levels 1, 2, 3, 6, 7, 8, 9, 27, 28). Only
  Florana's has a shape (cuboid 77, a thin wall across the path: 25 × 2 × 11 u) and a
  progress flag (2): it plays once, when the hero walks through. The others have no shape
  or path, so only a script can start them (moby code at 0x32A998 and 0x3135C0 calls
  0x3EAB30, which sets +0x38). Tyhrranosis (9) chains them through +0x4C (1 → 10, 2 → 9,
  9 → 11). Those in levels 27 and 28 have no path at all.

### The fixed-point camera, class 0x17 [code; data]

Every level has one, usually round the hero's arrival point (Veldin: cuboid 43, 8 × 10 ×
6 u, round the landing pad; Florana: cuboid 30, a 4.8 u cube where the hero stands after
landing). It holds the follow camera at the camera's position (`camera.md`). Its pvars
are 0x60 bytes:

| Offset | What |
|---|---|
| +0x20 | Word (runtime): frames it has applied, up to 200 |
| +0x24 | Halfword: 0 makes it clear a follow-block flag (+0x10) each frame; 1 on Veldin and Florana |
| +0x26 | Halfword (runtime): set when it lets go; it never applies again in that visit |
| +0x28 | Float: a tilt in degrees to hold, 0 for none |
| +0x2C, +0x30 | Floats: the view's pitch and yaw (radians); when both are 0, the camera's rotation y and z |
| +0x34 | Word: nonzero makes it let go as soon as the right stick turns the camera |
| +0x38 | Float (runtime): the horizontal distance from the hero to the camera's position |
| +0x3C | Float (runtime): the camera's position's height above the hero |
| +0x40 | Float (runtime): the look height that gives the pitch at +0x2C |
| +0x44 | Float (runtime): a yaw to hold, 0 for none |
| +0x48, +0x4A | Halfwords: nonzero lets it apply only in, or only out of, a follow-camera state (0x1E2D3B of 1 or 2); not read further |

## Pvars [verified, UYA NTSC]

- **The pvar table** (header +0x5C) has an 8-byte entry per pvar index: the offset of its
  pvars in the pvar data (header +0x60) and their size. The table has no count; indices
  come from the cameras, mobys and sound instances.
- **Moby links** (header +0x58): pairs of words, a pvar index and a byte offset, ended
  by a negative index. The word at that offset holds a moby's UID in the file; the
  loader replaces it with the moby's index.
- **Relative pointers** (header +0x64): pairs of the same kind; the loader adds the pvar
  block's address to the word.

## Mobys [verified for the fields read]

- **The header's word at +0x4C** is the block's offset: the count of placed mobys, the
  count of mobys spawned later, two zero words, then one record per placed moby. Each
  record starts with its size (0x88 on every UYA moby), and the loader steps by it.
- Fields read: +0x10 the UID, +0x28 the class, +0x40 the position, +0x4C the rotation,
  +0x58 the moby group, +0x68 the pvar index (Wrench's layout; the class, group and pvar
  index checked against the camera volumes in memory).

## Shapes [verified, UYA NTSC]

- **Header +0x68 cuboids, +0x6C spheres, +0x70 cylinders, +0x74 pills**: each a count,
  three zero words, then 0x80-byte records:

  | Offset | What |
  |---|---|
  | +0x00 | Three rows: the shape's axes, each scaled to its half size |
  | +0x30 | The centre (w 1) |
  | +0x40 | Three rows: the inverse of the axes |
  | +0x70 | The rotation, three floats |
  | +0x7C | Unused, 0 |

- **Lists** in Veldin's build: cuboids at the pointer 0x1E0180 (count 0x1E0184), spheres
  0x1E01A4 (0x1E01A8), cylinders 0x1E01AC (0x1E01B0), pills 0x1E019C (0x1E01A0; 0x90
  bytes an entry in memory).
- **Tests [code]**, with `local` = (point − centre) × the inverse rows (x × row 0 + y ×
  row 1 + z × row 2, as the VU0 helper 0x40C2D0 does):
  - cuboid (0x440760): every local coordinate from -1 to 1, ends included;
  - sphere (0x448E00): nearer the centre than the length of row 0;
  - cylinder (0x448D50): local x, y within 1 of the axis, local z from -1 to 1;
  - pill: the cylinder, or within a radius of either end.
- **Checked:** the cuboids found in the five savestates' memory match the disc (125 of
  125, 95 of 95, 76 of 77 on the Phoenix, where one moves with the level, 299 of 299,
  109 of 109).
- Cuboids are used by much else besides cameras (Veldin has 125).

## Paths [code; data, UYA NTSC]

- **The header's word at +0x78**: a count, the offset of the spline data from the block's
  start, its size, a zero word, then one word per path: its offset in the spline data.
  The splines are as the grind paths' (a point count, three words, then x, y, z and a
  fourth float per point).
- **The loader** copies the spline data and keeps a pointer per path at 0x21F740 (count
  0x1E01B4) in Veldin's build, zeroing each spline's second word.
- **The fourth float:** the path camera reads a positive one as a field of view (degrees)
  to reach at that point, then overwrites every point's with the distance to the next
  point (the last point's to the first). It is -1 (no key) on every camera path printed
  (levels 1, 2, 3 and 9), so no UYA path camera seen changes the field of view.
- Moby code uses the paths too (patrols, flights); 0 to 230 per level.

## Camera collision grid [code; data]

- **The header's word at +0x88**: the size of the cell lists, three zero words, then
  64 × 64 words, one per 16 u cell from the origin (row by row in y), each 0 or a list's
  offset from the grid's start. A list is a count, three zero words, then 0x30-byte
  entries: a bounding sphere, the shape kind (3 cuboid, 5 sphere, 6 cylinder, 7 pill),
  its index, flags, an int and a float.
- **The game** keeps the camera out of these shapes (0x3E2260, 0x3E2378, from the
  obstacle code), and the mode 4 camera won't take over while the hero is in one whose
  flags have bit 0.
- **Data:** every UYA level's grid is empty (Veldin's loaded grid is all zeros too).

## Camera volumes [verified, UYA NTSC, 2026-10-16]

Mobys of class 2329 that tune the follow camera while the hero is in their cuboid
(`camera.md`, "Camera volumes and splines"). Their pvars:

| Offset | What | Copied to the volume slot |
|---|---|---|
| +0x00 | Cuboid index the hero must be in | — |
| +0x04 | Float: follow distance, -1 for none | +0x00 |
| +0x08 | Float: height, -1 for none | +0x04 |
| +0x0C | Float: look height, -1 for none | +0x08 |
| +0x10 | Float: tilt (radians), -1 for none | +0x0C |
| +0x14 | Float: field of view (radians), -1 for none | +0x10 |
| +0x18 | Float: arm drag, -1 for none | +0x14 |
| +0x1C | Byte "stiff look", copied unless the halfword is -1 | +0x18 |
| +0x1E | Halfword: stick lock, -1 for none | +0x1A |
| +0x20 | Byte: priority, 0 keeps the slot's 5 | +0x1C |
| +0x21 | Byte, copied if nonzero | +0x1D (the slot's in-use flag) |
| +0x22 | Byte: steer always | +0x1E |
| +0x23 | Byte: steer once the stick rests | +0x1F |
| +0x24 | Halfword: the rest timer, -1 for none | +0x30 |
| +0x26 | Halfword: frames of rest before steering, -1 for none | +0x32 |
| +0x28 | Word: the hero is inside (runtime) | — |
| +0x2C | Word: the slot it holds (runtime) | — |
| +0x30 | Four floats: the steer direction, unless the first is -1 | +0x20 |
| +0x40 | Word: the moby group that must have a living member | — |

- **Data:** only Veldin (1) and Florana (2) have any. Veldin's: cuboid 63 (16 × 15 × 18 u,
  turned -45°), distance 4, height 5, group 51. Florana's two: cuboids 58 (40 × 60 ×
  20 u) and 21 (21 × 50 × 9 u), each distance 8 and height 5, groups 9 and 8. Nothing
  else is set on any of them.
- **Checked:** all three found in the savestates' memory with these pvars, idle.
- Class 2329's update is in the level's own code (0x3093A0 on Veldin, 0x3071B8 on
  Florana); builds without the class have none. The Phoenix build also fills a volume
  slot, from a function at 0x32C790 in its own code that isn't identified.

## UYA's cameras [data, 2026-10-16]

From `openrac-tool cameras` on the UYA NTSC-U disc. Every level also has classes 0, 4, 5,
7, 0x17 and 0x20; "Other classes" are the rest. Shapes: cuboids / spheres / cylinders.

| Level | Cameras | Other classes | Camera volumes | Paths | Shapes |
|---|---|---|---|---|---|
| 1 Veldin | 7 | 0x13 | 1 | 159 | 125 / 0 / 1 |
| 2 Florana | 8 | 0x13, 0x11 | 2 | 115 | 95 / 0 / 0 |
| 3 Starship Phoenix | 10 | 0x13 × 4 | 0 | 15 | 77 / 1 / 0 |
| 4 Marcadia | 6 | — | 0 | 150 | 311 / 0 / 0 |
| 5 Daxx | 6 | — | 0 | 143 | 220 / 1 / 0 |
| 6 Phoenix under attack | 7 | 0x13 | 0 | 43 | 91 / 0 / 0 |
| 7 Annihilation Nation | 7 | 0x13 | 0 | 60 | 141 / 1 / 0 |
| 8 Aquatos | 12 | 0x13 × 6 | 0 | 95 | 132 / 0 / 0 |
| 9 Tyhrranosis | 19 | 0x13 × 12, 0x1D | 0 | 230 | 290 / 0 / 2 |
| 10 Zeldrin Starport | 6 | — | 0 | 165 | 195 / 1 / 12 |
| 11 Obani Gemini | 8 | 0x18 × 2 | 0 | 22 | 97 / 2 / 0 |
| 12 Blackwater City | 6 | — | 0 | 58 | 126 / 0 / 0 |
| 13 Holostar Studios | 6 | — | 0 | 85 | 156 / 0 / 0 |
| 14 Koros | 6 | — | 0 | 201 | 90 / 0 / 0 |
| 16 Metropolis | 7 | 0x11 | 0 | 77 | 171 / 0 / 0 |
| 17 Crash Site | 7 | 0x3 | 0 | 73 | 109 / 0 / 0 |
| 18 Aridia | 7 | 0x1D | 0 | 206 | 212 / 0 / 0 |
| 19 Qwark's Hideout | 6 | — | 0 | 58 | 249 / 1 / 0 |
| 20 Launch Site | 7 | 0x1D | 0 | 36 | 46 / 0 / 0 |
| 21 Obani Draco | 6 | — | 0 | 19 | 26 / 0 / 0 |
| 22 Command Center | 6 | — | 0 | 46 | 127 / 0 / 0 |
| 23 Holostar, as Clank | 6 | — | 0 | 7 | 85 / 0 / 0 |
| 24 Insomniac Museum | 7 | 0x1D | 0 | 1 | 31 / 0 / 0 |
| 26 Metropolis, Rangers | 7 | 0x1D | 0 | 74 | 125 / 0 / 0 |
| 27 Aquatos, as Clank | 7 | 0x13 (no path) | 0 | 9 | 109 / 0 / 0 |
| 28 Aquatos sewers | 7 | 0x13 (no path) | 0 | 97 | 299 / 0 / 0 |
| 29 Tyhrranosis, Rangers | 7 | 0x1D | 0 | 135 | 162 / 0 / 0 |
| 30–36 Qwark vid-comics | 7 each | 0x1E | 0 | 0 to 19 | 48 to 268 / 0 / 0 |
| 39–55 multiplayer | 8 each | 0x21, 0x1D | 0 | 0 to 40 | 0 to 201 / 0 / 0 |

- No level has camera colliders (the grid at +0x88 is empty everywhere).
- Classes 0x3, 0x11, 0x18, 0x1D, 0x1E and 0x21 aren't in Veldin's mode table, so their
  code is in other levels' builds and not read yet. The 0x11 cameras look like 0x17's
  (lead 8, priority 4, a cuboid, a distance 3 and a height 4 in their pvars).

## Deadlocked [verified for the mobys and pvars, DL NTSC-U and Gladiator PAL, 2026-10-30]

- **The file:** a DL level WAD's header keeps it at +0x58 (`disc-layout.md`, "Level
  data"), the same file the level data's own gameplay WAD gives;
  `Gameplay::read_dl` reads it, and `openrac-tool mobys` lists its mobys and pvars on
  either game's disc.
- **The header moves the blocks** (Wrench's `DL_GAMEPLAY_CORE_BLOCKS`; `Layout::DL`):
  +0x04 cameras, +0x2C moby classes, +0x30 mobys, +0x34 moby groups, +0x3C moby links,
  +0x40 the pvar table, +0x44 the pvar data, +0x48 relative pointers, +0x4C cuboids,
  +0x50 spheres, +0x54 cylinders, +0x58 pills, +0x5C paths, +0x60 grind paths, +0x6C the
  camera collision grid. The art (ties, shrubs, lights, occlusion) is in another file,
  and each mission has its own moby and pvar blocks; neither is read.
- **Moby records are 0x70 bytes** (Wrench's `DlMobyInstance`): +0x08 the UID, +0x10 the
  class, +0x28 the position, +0x34 the rotation, +0x40 the group, +0x50 the pvar index.
  The block and the pvar table and data are laid out as UYA's.
- **Checked:** Catacrom Four's co-op tether (`coop.md`, "The tether") reads from the disc
  as the game loaded it in `dlntsc-catacrom-coop-play.p2s`: class 0x247B, UID 21, at
  (591.92, 345.50, 0.00), pvars 40.0, 10, −1, 1 in 0x120 bytes. Only the moby and pvar
  blocks are checked; the other offsets are Wrench's.

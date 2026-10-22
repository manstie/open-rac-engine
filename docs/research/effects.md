# Effects: particles, quads, lights and the rest

How UYA draws what isn't a model: the guns' shots, flashes, beams, trails, sparks and
explosions, and the reticles and HUD sprites that share their textures. The reference for
importing the weapons' effects from the user's disc (PLAN.md P3.18) and for the kit's
effects layer (P1.9, P1.12).

## Status and method

- Read 2026-10-22 on UYA NTSC-U (SCUS-97353) in Veldin's memory image (the import
  `uyantsc-veldin`, `docs/re-setup.md`), and in every level's core header on the disc.
  Addresses are Veldin's.
- **Labels:** **[code]** read in the game's code; **[data]** read from the image or the
  disc. Nothing here is traced or drawn by us yet, apart from contact sheets of the
  textures made to check their format (kept in `cache/`).
- The aim indicators that use these textures are in `weapons.md`, "Aim indicators"; each
  gun's shots in `weapons.md`, "Each gun".

## What the engine has

Shots and effects are mostly drawn by code: a shot's update spawns particles and lights
and queues draw functions of its own each frame. Only some shots are mobys with a mesh.

### Effect textures [code, data]

- **The table:** core header +0x58 is the count (105 on Veldin), +0x5C its offset in the
  header; 16-byte entries {palette offset, pixel offset, width, height} (sizes as log2;
  negative for 4-bit images, none seen), offsets counted from the unpacked core plus the
  header's +0x68. `fx_textures_init` (0x4225E8, from `level_load_core`) copies it to
  0x1E7D00 (count 0x1DFA00, base 0x1DFA04).
- **Format:** 8-bit images, 16 to 256 pixels a side, stored linear (not GS-swizzled), each
  with a 256-colour RGBA palette in CSM1 order (`rac_formats::texture::csm1`), alpha 0x80
  opaque: the same as the armor's images [data: decoded cleanly; the swizzled reading
  doesn't].
- **Use:** `fx_tex0` (0x401CD0) gives a texture's GS registers by its number, uploading it
  on its first use in a frame (reset by 0x401978). Negative numbers are built-ins: −1 to
  −4 two small images in the boot program (0x141AD0, 0x141DC0), −5 to −7 the frame
  buffers (so −7 is a copy of the frame), −8 another GS buffer.
- **Global and per level** [data]: textures 0–99 are byte-identical on Veldin, Florana,
  the Phoenix and level 28, and every level has at least 100 (levels 11, 39, 44 and 54
  exactly 100; up to 117), so 0–99 are the engine's own set and 100 on are the level's.
  (Checked byte for byte on those four; inferred for the rest.)
- **What's in the set:** reticle pieces (13, 17, 18, 31–39, 51–53, 68, 86–91), lightning
  (47, 48, 60), fireballs (55, 56), muzzle flames (62, 63, 65), glows (11, 23, 44),
  feathers (78, 79), leaves (80–84), portraits (92–99), a 256² gradient (66) for screen
  warps.

### Particles [code, data]

- **The pool:** 0x800 particles of 0x40 bytes (pointer 0x1E02F8, bitmap 0x2208C0).
  `particle_alloc` (0x45E438, by type) and 0x45E580 free; `particle_update_all`
  (0x45E788) runs each through its type's update (`particle_update_table`, 0x220EC0,
  types 0–0x80, filled by 0x45F4D0); `particle_draw_all` (0x45E810, scratchpad and VU0
  code) sorts them and draws camera-facing sprites.
- **A particle:** +0 type; +1 flags (negative skips it); +2 its particle texture; +3 and
  +9 render bytes; +4 RGBA (R low, 0x80 opaque); +8 spin; +0xA life in frames; +0xC
  size; +0x10 position; +0x20 velocity; +0x2C gravity (along the level's up where it has
  a gravity centre); +0x30 size a frame; +0x34 alpha a frame; +0x35 spin a frame; +0x3C an
  optional update of its own. Sizes are world units × 210000.
- **Spawners**, one per type, taking position, velocity, colour, start and end size,
  life, start and end alpha and gravity. The guns use 0x61 (0x46DCB8, the general
  sprite: size and alpha lerped over its life, its last argument picking an entry of the
  type's texture list), 0x17 (0x4633E8), 0x2C (0x466F98), 0x55 (0x46C468), 0x5A (0x46CF70,
  a glare along a direction), 0x35 (0x467DA8), 0x5C (0x46D570, a burst of parts), 0x0F
  (0x462190), 0x19 (0x463938), and 0x13, 0x22, 0x5B, 0x6F, 0x04, 0x3E, 0x74, 0x7E, 0x80,
  0x6A–0x6E, 0x72, 0x76 and 0x77.
- **Particle textures:** core header +0x50 count (65 on Veldin; 63–87 in single player,
  51 in multiplayer), +0x54 offset; 16-byte entries {palette offset, a 4-bit field
  (always 0), pixel offset, size}, square 32 or 64, data from the core plus the header's
  +0x64, the same 8-bit linear CSM1 format. `particle_textures_init` (0x4224C8) packs
  them into 0x220AC0 (count 0x1E030C).
- **Which textures each type draws:** core header +0x6C is the offset of a block in the
  header: {type count 129, texture count, the byte table's offset (0x220) and size}, then
  129 offsets (0 for the default list), then each type's list of particle texture
  numbers. Copied to 0x221118 (a pointer per type) and 0x221370.
- **Per level** [data]: the particle textures differ in count and order between levels,
  though 63 of Veldin's 65 are in each of three others, and each type's list differs too;
  resolved through each level's own lists, 112 of the 129 types give the same images. So
  an importer resolves type → texture through the level's own tables.
- **Veldin's lists for the guns' types:**

  | Type | Particle textures | Looks like |
  |---|---|---|
  | 0x0F | 7, 11 | fire and smoke puffs |
  | 0x13, 0x55 | 12 | fire |
  | 0x17 | 0 | dark smoke |
  | 0x19 | 14 | a small white dot |
  | 0x22 | 16 | a bubble |
  | 0x2C | 18 | an ember |
  | 0x35 | 21 | a four-point sparkle |
  | 0x5A, 0x6F | 25, 26; 25 | red glares |
  | 0x5B | 12, 27–32 | coloured glows |
  | 0x5C | 33–38 | the explosion's set |
  | 0x61 | 12, 40–47, 28, 48–51 | fire, smokes, a streak, a flare, swirls, a star, rocks, a sparkle |
  | 0x7E, 0x80 | 52, 48–50; 52, … | |

### Quads, beams, strips and glows [code]

- `fx_quad_setup` (0x4075A8: quad, texture, blend, fixed alpha) fills a quad of four
  positions, colours and texture coordinates with a texture and a blend from the table at
  0x2C5450 (five ints an entry, the GS's A, B, C, D and FIX): 0 normal alpha, 1 additive,
  4 brightens what's under it (Cd × (1 + As)), 6 and 7 by the frame's alpha.
  `fx_quad_draw` (0x405A58) draws it with a matrix.
- Built on them: a camera-facing billboard (`fx_billboard` 0x4082B0: width, height, roll,
  position, texture, colour, blend); a camera-facing beam between two points (`fx_beam`
  0x4084B0: width, texture, blend, four colours); a quad on a moby's axes (0x408090); a
  two-texture beam (0x407240); a glow at a random angle in texture 44 (0x4076F8) and an arc
  glow in 23 (0x407A60); fans and rings of 22-vertex strips (`fx_ring_fan` 0x409A50) and
  rings from a table (0x409448); a raw strip of n vertices (`fx_strip` 0x4066DC) and a
  trail of segments (0x4068C8); a flat-coloured rectangle on the screen (0x403708, the
  full-screen flashes) and a textured one (0x403CA8).
- **Draw callbacks:** `draw_callback_add` (0x4026A0) queues up to 64 functions with an
  argument for the frame's draw (run by 0x4026F0 from 0x4010D0); 0x402498 is a second
  list of 64. Shots queue their draws there every frame they want drawing.

### Lights, shake, rumble and the screen [code]

- **Point lights:** `light_add` (0x421DE0: radius, ?, position, colour) takes one of eight
  slots (0x1F1BC0) or fails; 0x421F08 takes the colour as floats; 0x421FE0 frees one.
- **Camera shake:** three channels (0x1E2AE0, 16 bytes apart: amplitude, current, frames,
  total). Guns write channel 0: an amplitude of 0.04–0.45 for 25, 30 or 40 frames. Each
  frame (0x3F2A98) moves the camera by amplitude × cos(2 × frames left) × (t / T)² along
  one of its axes (kind 2 a roll).
- **Rumble:** `rumble_start` (0x3F0118: pattern, player) plays a pattern of up to eight
  16-byte steps ended by −1 (per-port state at 0x1E2500 + port × 0x80); 0x3F0338 stops
  it. The patterns are in each level's overlay (the Shock Blaster's at 0x2B3940).
- **Flashes:** 0x403708 over the whole screen with a fading alpha (the charged Shock
  Blaster, the shared explosion).
- **Screen warps:** 0x36CBC0 (and copies in the Nitro, Flux scope and RY3NO code)
  redraws the frame (texture −7) in four mirrored quarters through texture 66.

### The shared explosion and the lightning bolt [code]

- `explosion_spawn` (0x32FE90) spawns class 0x13A1, which has no model, from about 30
  arguments (size, position, counts, colours and timings; most not read), each caller
  passing its own. Its update (0x330C80, 0x3302F8) spawns particles of types 0x55, 0x61,
  0x5A and 0x17, shakes the camera 0.075 and queues a flash and a glow in texture 11
  (0x3317C8). Crates and enemies use it too.
- `lightning_bolt_spawn` (0x34DAA0) spawns class 0x1195 (no model; update 0x34DD38,
  drawn by 0x34E5E0): the Spitting Hydra's, Shield Charger's, Holo Shield's and Rift
  Inducer's bolts.

### Sounds [code, data]

- `moby_play_sound` (0x4A7200) plays entry i of the class's sound table (class +0x28,
  count at +0xD, 0x20 bytes each); 0x4A6E88 stops one, 0x4A72A8 plays by class number.
- An entry [data, on five shot classes]: +4 range (40–80), +0xC volume (about
  0x400–0x6CC), +0x10 and +0x14 an optional pitch range, +0x18 a loop byte, +0x19 flags,
  +0x1A the sound's number in its bank, +0x1C which bank.
- Shot classes from the level's core use bank 0, the level's own (Veldin's shots are
  sounds 0x5E–0xC5); guns from the gadget WAD are set to bank 4 by `gadget_install`, the
  gun's own 989snd bank in its gadget entry (`moby-format.md`, "The gadget WAD").

## Shot classes on the disc [data, every single-player level]

- **Every shot class is in each single-player level's core** class table (levels 1–36);
  the multiplayer maps (39–55) have none. The guns themselves (0x1843, 0x5CA, 0xA65,
  0x1886, …) are in no core: they come from the gadget WAD.
- **Listed with no model** (offset 0), drawn by code: 0x1851 (the N60's bullet), 0x1862
  and 0x1883 (the Shock Blaster's and Cannon's), 0x13A1, 0x1855, 0x1858, 0x1888, 0x1898,
  0xC41, 0xCF7, 0x186F, 0x1890, 0x1195, 0x1869, 0x188F, 0x188B, 0x1DF6.
- **With meshes in the core:** 0x1881, 0x185A, 0x1A50, 0x1E52, 0xA82, 0xECE, 0x1894,
  0x1CB8, 0xA9B, 0x9A9, 0xE64, 0x1366, 0x1367, 0x186D, 0x1853, 0x1893, 0x1ADA, 0x1E05,
  0x188A, 0x1899, 0x1867, 0x186B, 0x1872, 0x189B, 0x1BC3, 0x1C59, 0x1D3E, 0x1873, 0x1DCE,
  and the whip's links 0x183A and 0x183C and the Shield Charger's 0xA5C and 0x1192.
- Muzzle flames and beams use the effect textures (62, 63, 65, 23, 43, …), not the
  gadget WAD's 128² image, which is the gun's own skin.

## Each gun's effects [code]

What each gun's code, its shots' updates and its draw callbacks call: an upper bound, as
shared helpers (the explosion) bring their own. Texture numbers are the global effect set;
classes in italics have no model.

| Gun (V5) | Shots | Particle types | Effect textures | Other |
|---|---|---|---|---|
| Shock Blaster (Shock Cannon) | *0x1862* (*0x1883*) | 0F 17 2C 55 61 (+6F) | 11 23 26 44 61, 66 with −7 (V5 +10) | Rays drawn by a callback (0x36C078) in quads of 26 and 61 and a glow of 11 (blend 4); rings (0x36C910, 0xFF8080 and 0xFFC480); a screen warp and a flash; shake 0.04 and 0.09 (V5 0.06, 0.12); rumble 0x2B3940 (0x2B3C10). |
| N60 Storm (N90 Hurricane) | *0x1851* (0x1881) | 0F 17 2C 55 5A 5C 61 (+5B) | 11 47 49 57 63 65 (V5 62) | A muzzle flame quad of 63 (62 at V5); an explosion on a hit; a flash; shake 0.075; rumble 0x2BAF00 (0x2AEA10). |
| Nitro Launcher (Eruptor) | 0x185A; *0x1858* (*0x1888*) | 04 17 2C 35 55 5A 5B 5C 61 6F 76 77 | 11 14 16 23 26 43 47 57 61 63 66 73–77 | The blast's shake (0.4 − 0.02 × distance, within 20 u); a screen warp, a flash, a landing marker, a strip; rumbles 0x2B3A40, 0x2B3DE0 (V5 0x2B3CE0). |
| Lava Gun | *0xC41*, 0x1E52 | 19 61 6D 72 7E 80 | 11 14 23 43 44 | The stream drawn by a callback (0x305858). |
| Annihilator (Decimator) | 0x1894, 0xA82, 0xECE | 04 13 17 2C 35 55 5A 5C 61 6E | 11 23 44 | A light on the missile; the explosion; rumble 0x2B39C0. |
| Flux Rifle | *0xCF7*, *0x186F* | 35 5A 5C 61 74 | 10 14 21 23 70; 66 with −7 scoped | A light (radius 10, 0xFF0000FF); rings (0x37D1E0); rumble 0x2B2970. |
| Plasma Whip | *0x1890*, 0x1CB8 | 17 19 2C 55 5A 61 72 | 11 14 23 43 71 | A light (radius 5.5); a trail; shake 0.4 and 0.45; rumble 0x2B2B80. |
| Plasma Coil (Storm) | 0xA9B (0x9A9) | 35 | 14 43 44 47 48 (V5 41 42 45 49) | The ball's draws (0x30D680, 0x315FA8; V5 0x3075F8, 0x309EF8). |
| Bouncer | 0xE64, 0xECE | 17 55 5A 61 | 11 | A landing marker; the explosion; rumble 0x2AE8C0. |
| Miniturret (Megaturret) | 0x1366 (0x1367, 0xA82) | 22 35 (+17 55 5A 61 6E) | 14 23 44 | A landing marker. |
| Shield Charger | *0x1195* | 35 3E | 11 14 21 23 | The bubble's strips and glows (`weapons.md`, "Shield Charger"). |
| Holo Shield | 0x186D, *0x1869*, *0x1195* | 0F 17 2C 35 3C 5A 61 | 11 14 23 42 43 44 47 49 57 62 63 | A light, bolts, a landing marker, the explosion. |
| Agents of Doom | 0x1853, 0x1ADA, 0x1893, *0x188F* | 04 17 19 23 2C 35 4A 55 5A 61 6A–6E | 11 23 43 44 47 50 57 63 | Shake 0.45; a landing marker. |
| Infector (Infecto-Bomb) | 0x1E05 (0x188A, *0x188B*, 0x1899) | 19 22 34 61 | none | V5 shake 0.4 − 0.0175 × distance; rumble 0x2B3D60. |
| Spitting Hydra | *0x1195* | 17 2C 35 6F | 11 14 23 43 57 61 71 | Bolts; a light; rumble 0x2AE990. |
| Disc Blade Gun | 0x1867 | 0F 17 2C 35 55 5A 5C 61 74 | 11 47 49 57 | A trail; the explosion. |
| Rift Inducer | 0x186B, *0x1195* | 0F 13 2C 35 55 61 | 11 14 18 23 49 57 62 63 65 75–77 | Bolts; a light; rumble 0x2B3AC0. |
| Suck Cannon | 0x1872, 0x1893 | 02 0F 17 19 2C 35 55 5A 5C 61 6A–6C | 11 21 43 | A strip (the vortex). |
| Qwack-O-Ray | 0x1BC3, 0x189B, 0x1C59, 0x1D3E | 16 17 19 35 4E 55 5A 61 66 79 | 8 11 13 14 16 57 | The beam's draw (0x37F738); lights of radius 5.5 and 7.5. |
| RY3NO | 0x1873, 0x1DCE, *0xC41*, *0x1DF6* | 13 17 2C 55 5A 5C 61 7E 80 | 11 14 43 44 66 with −7 | A screen warp; rumble 0x2B3B90. |

The calls' own arguments (colours, sizes, lives) are listed per gun in the decompiled
output kept in `cache/`, not read one by one yet.

## Importing them (P3.18)

What it takes to show the guns' effects as the game does, from the user's disc:
1. **Read the textures** (`rac_formats`): the core header's effect table (+0x58, +0x5C,
   data from +0x68), particle textures (+0x50, +0x54, data from +0x64) and particle
   definitions (+0x6C) of the level played, or Veldin's for the demo; images as the
   armor's (8-bit, CSM1 palette, alpha 0x80). The effect textures 0–99 can come from any
   level; particle textures must be looked up through the same level's tables.
2. **Read the shot classes** with a mesh from the same core (`Core::moby_class`, as the
   Holo Shield's V5 glove already is), and the whip's links.
3. **An effects layer in the kit**, ISO-free as the rest: billboards, beams, strips,
   trails, rings, point lights, camera shake, screen flashes and particles with the
   game's motion (size and alpha lerped over a life, velocity and gravity, spin), drawn
   with the four blends that matter (normal, additive, brighten, by the frame's alpha).
   A game hands it textures and recipes at run time, as it hands the rig its models;
   without a disc it draws its own placeholders.
4. **Recipes per gun:** each gun's calls with their arguments (texture or particle type,
   colour, sizes, life, blend), read gun by gun, starting with the six first guns; the
   kit's `GunEvent`s already carry an effect name for each gun's fire and impact
   (`rac_combat::weapons::Effects`).
5. **Sounds** with P3.13: the gadget WAD's bank per gun and the level's bank for shots,
   by the class's sound table.

## Not read yet

- The particle renderer's byte fields (+3, +9) and its table at 0x2BFDA0; each particle
  type's own update.
- What the built-in textures −1 to −4 hold.
- 0x4C4A18 and 0x4C4AD8, which every projectile runs (gated by the camera mode and
  0x1A4F48, 0x1D6740, 0x1D673C; perhaps shots placed for the first-person view).
- Whether 0x4C3500 is the landing marker the lobbed and thrown guns show (every glove,
  the Bouncer, the Nitro guns and the Infecto-Bomb call it).
- What the rumble patterns' bytes mean, and most of the explosion's arguments.
- The Lava Gun's stream and the N60's bullet's own draws (0x305858; 0x35BBC8, 0x3A63B8).

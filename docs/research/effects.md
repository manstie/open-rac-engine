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
  disc. Nothing here is traced yet. Contact sheets of the textures checked their format,
  and since 2026-10-23 the kit draws the guns' effects with them, checked in screenshots
  of an off-screen run (both kept out of the repo).
- The aim indicators that use these textures are in `weapons.md`, "Aim indicators"; each
  gun's shots in `weapons.md`, "Each gun".

## What the engine has

Shots and effects are mostly drawn by code: a shot's update spawns particles and lights
and queues draw functions of its own each frame. Only some shots are mobys with a mesh.

### Effect textures [code, data]

- **The table:** core header +0x58 is the count (105 on Veldin), +0x5C its offset in the
  header; 16-byte entries {palette offset, pixel offset, width, height}, sizes in pixels
  [data: 32 to 128 for the aim indicators], offsets counted from the unpacked core plus
  the header's +0x68. `fx_textures_init` (0x4225E8, from `level_load_core`) copies it to
  0x1E7D00 (count 0x1DFA00, base 0x1DFA04) in the form `fx_tex0` reads: the GS
  registers once uploaded (+0), the pixel and palette offsets in 64-byte units (+8,
  +0xA), the width and height as log2 (+0xC, +0xD) and the GS pixel format (+0xE).
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
  header: four words {type count 129, texture count, the byte table's offset (0x220) and
  size (0x70 on Veldin)}, then a word per type, the offset of its list in the block (0
  for none: read as the table's start), then the byte table, one particle texture number
  a byte. The lists share the bytes and have no lengths; `CoreHeader::particle_lists`
  ends each where the next one starts, which gives the lists below [data, checked on
  Veldin 2026-10-23]. Copied to 0x221118 (a pointer per type) and 0x221370.
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
  | 0x5C | 33–35, 24, 36–38 | the explosion's set |
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

### Sounds [code, data, 2026-10-23]

Read in Ghidra on Veldin's image and from the disc's banks; `rac_formats::sound` reads
the banks and tables, and its disc tests check them against the image.
- **Playing:** `moby_play_sound` (0x4A7200: index, flags, moby) plays entry *index* of the
  moby's class's sound table (class +0x28, count +0xD, 0x20 bytes an entry);
  `class_sound_play` (0x4A72A8) the same by class number; `level_sound_play` (0x4A7368)
  an entry of the level's own table (below). All go through `sound_start` (0x4A6F58),
  which takes one of 52 slots of 0x80 bytes at 0x1A3170 (42 unless the moby is the
  hero's), and returns −1 when the sound would be quieter than 0x20 out of 0x400 or the
  entry loops (+0x18) without flag 4 in the call (a loop only starts when asked for one,
  and the reverse). `sound_stop` (0x4A6E88) stops a slot; 0x4A6D70 and 0x4A6DF8 stop a
  class's or a moby's.
- **An entry:** +0 near and +4 far (f32), +8 the volume at far and beyond and +0xC within
  near (out of 0x400; some are louder and are capped at 0x400 less the fade, 0x1D52E4),
  +0x10 and +0x14 a pitch modulation picked at random between them in 128ths of a
  semitone (±127 is a semitone, −1524 an octave down; [inferred from the values]), +0x18
  loops, +0x19 flags (bit 0: the volume falls with the square of the distance left), +0x1A
  the sound's number in its bank, +0x1C the bank (0 the level's; `gadget_install` sets a
  gun's to 4, its own).
- **Volume** (0x4A53D0): within near the near volume; between, the far volume plus
  (far − d)/(far − near) of the difference (squared with bit 0); past far, the far
  volume. *d* is the distance from the camera (0x1E2A40; every camera's in split screen,
  summed). Multiplied by the slot's own scale (+0x5C, 0x400 unless set) and redone every
  frame as the moby moves (`sound_update`, 0x4A6468).
- **Pan** (0x4A5508): the sound's angle round the camera, in degrees (0 ahead, 90 to the
  right), in the camera's frame; within 2 u of the camera it keeps its last pan while the
  sound is roughly ahead or behind, so it doesn't flip.
- **To the library:** `snd_PlaySoundVolPanPMPB`'s stub (0x13C0B8: bank, sound, volume,
  pan, then the slot's +0x18 and the random pitch) and an update (0x13C2C0) each frame.
- **The core's sound block** (core header +0x70): halves {where the level's entries are,
  how many, where the table numbering them is, its length}, then two halves per class in
  the class table's order, the first where its list is: the bank less one (−1, so 0),
  then each of the class's entries' number in the level's bank as a word.
  `level_load_core` (0x428750, from 0x429230) writes them into each class's table, and
  copies the level's own entries (42 on Veldin) to 0x1DFBEC, numbering them through the
  table (0xFFFF past its end). A class stores its own numbering (0, 1, …): the Nitro
  Launcher's charge (0x185A) stores 0 and 1 and plays Veldin's 181 and 182.
- **Classes with no model** (offset 0 in the core's class table) have no class at all:
  `moby_class_register` (0x424368) only gives them their update (0x45B030), so they have
  no sound table. The N60's bullet and the Shock Blaster's shot are such; their sounds,
  if any, are the gun's.
- **The banks** (989snd `SBlk`, as OpenGOAL's 989snd reads them, `prior-art.md`): a file
  of chunks {type 1 or 3, count, then {offset, size}}: the bank, then its samples. Every
  gun's bank in the gadget WAD is named `mn00`, version 3 but for entries 0, 3 and 10
  (version 1, 40-byte grains), with 1 to 9 sounds (the five entries of class 0, 57);
  entry 44 has none. Veldin's (level header +0x18) is `DAW`, version 3, 321 sounds and
  1.5 MB of samples. A sound's grains include random pitch bends (27), random registers
  (31) that tones take for their volume, loops (21, 22) and random picks (25).
- **Tones** give their sample's note as a signed byte: negative for a sample at 48 kHz
  (the note is its magnitude), positive for one at 44.1 kHz, as OpenGOAL's
  `PS1Note2Pitch` reads it; UYA's gun tones are mostly negative.
- **Each gun's entries** (of the gun moby's class, from its gadget bank), read in each
  gun's update. The indices are the same at every level, but from V5 most guns are
  another class with its own table; where the V5 class has an update of its own it is
  named.

  | Gun (update; V5's) | Fire | No ammo | Loops | Other |
  |---|---|---|---|---|
  | Shock Blaster (0x36A790; 0x385948) | 0; V5 4 (0 when charged) | 3; V5 1 | 4 charging, while held; V5 3 | 2 when 0x1DD88C is set and its timer runs out. V5's update also plays 3 without flag 4 where V1's plays "no ammo", which the loop refuses: silent. |
  | N60 Storm (0x3A5928; 0x322E68) | 3 | 4 | | |
  | Nitro Launcher (0x36E360; 0x3893E8) | 0 | 1 | | the charge's blast: its class 0x185A's 0 (0x361218) |
  | Lava Gun (0x303F88, both) | | 2 | 0 the stream, while it flows | 1 where it hits, 8 frames apart (`class_sound_play` of 0x5C9 or 0xC59, 0x304A18) |
  | Annihilator (0x36D5A0) | 0 | 1 | | the missile's hit: 0x1894's 0 (1 at V5, 0x396A20). V5's class 0x1887 has no update of its own in the table; its two entries match. |
  | Flux Rifle (0x3444B0) | 0 | 1 | 2 zooming in, 3 zooming out | |
  | Infector (0x3688E8; 0x389F58) | 0 | 2; V5 1 | | |
  | Spitting Hydra (0x31FB90, both) | 1 | 3 | 2, as it aims | 0 for each target found, 5 while counting |
  | Disc Blade Gun (0x36EFC0, both) | 1 | 2 | | |
  | Agents of Doom (0x35C9F0) | | 0 (also with 8 agents out) | | |
  | Rift Inducer (0x372990, both) | 1 | 5 | | |
  | Holo Shield Glove (0x3717A8, both) | 2 | 1 | | 3 with none left to throw. Its V5 class (0x1C65, in the core) has no sound table on Veldin: silent. |
  | Plasma Whip (0x356110) | 1 to V2, 4 from V3 (by the row's level, 0x1425C0 + 0x7F) | on another moby (0x1E1AC4) | 0, from V3 3 | |
  | Suck Cannon (0x3619C8, both) | 2 | | 0 while sucking | |
  | Qwack-O-Ray (0x37DEB0, both) | | | 0 | 1 when a timer ends; neither mapped to a cue yet |
  | RY3NO (0x3805B0, both) | 1 at V1, 3 from V2 | 2 | | V5 also plays class 0x1F4's 1 on the hero |
  | Plasma Coil (0x315638; 0x306F68) | 2 | 7; V5 6 | | |
  | Bouncer (0x31C890, both) | 0 | 1 | | 3 when its timer ends with ammo |
  | Miniturret Glove (0x346078; 0x343088) | 0 | 2 | | |
  | Shield Charger (0x3410E8) | | | 3, the shield | 5, 2 |

  "No ammo" plays when the fire button is pressed with none (`0x50D8F8` or `0x50DA08`
  failing with the button's press bit, 0x1D52FC + 0x1A4), or once on the next try after
  the last round went (a per-gun flag such as 0x1E1D90). Most are entries pitched up
  three semitones (381). The calls of every caller of 0x4A7200 and 0x4A72A8, with the
  class each update is for, are listed in `cache/snd/soundcalls.txt` (not committed).
- **In the kit and the app** (2026-10-23): `rac_demo::sfx` turns the guns' events into
  cues (fire, no ammo, impact, arc, level up, and the loops of charging, the stream, the
  vortex and the scope) and plays placeholders made in code with this volume and pan;
  with the disc, `openrac/src/gun_sounds.rs` claims the cues of the table above and plays
  the disc's sounds through `rac_audio`. Bevy's spatial audio isn't used: it fades with
  the square of each ear's distance.

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
   - Done for the effect textures (2026-10-22): `rac_formats::level::Core::fx_textures`
     reads the table (`CoreHeader::fx_textures`), and `openrac` hands the aim
     indicators' fourteen to the HUD (`crates/openrac/src/fx_textures.rs`, `weapons.md`,
     "In the kit"). Rings and the scope are each stored as a top-left quarter with the
     centre at its bottom-right corner, and drawn as four quarters turned or mirrored.
   - Done for the particle textures and lists (2026-10-23): `Core::particle_textures`
     and `CoreHeader::particle_lists`; `openrac` hands them, with the effect textures, to
     the kit's effects (`fx_textures.rs`, `register_disc_fx_images`).
2. **Read the shot classes** with a mesh from the same core (`Core::moby_class`, as the
   Holo Shield's V5 glove already is), and the whip's links.
   - Done for six (2026-10-23, `openrac/src/shot_models.rs`): the Nitro Launcher's
     charge 0x185A, the Annihilator's missile 0x1894, the Disc Blade 0x1867, the
     Bouncer's canister 0xE64, the Miniturret 0x1366 and the RY3NO's rocket 0x1873, at
     rest, facing their way of flight (the class's +x, Bevy's −z), each checked in flight
     in the app. Not drawn from their classes: the Plasma Coil's ball 0xA9B (untextured),
     the rift 0x186B (0.05 units across, scaled by its code) and the Infector's 0x1E05 (a
     mound of goo 1.4 units high, not the shot). The whip's links are still to do.
3. **An effects layer in the kit**, ISO-free as the rest: billboards, beams, strips,
   trails, rings, point lights, camera shake, screen flashes and particles with the
   game's motion (size and alpha lerped over a life, velocity and gravity, spin), drawn
   with the four blends that matter (normal, additive, brighten, by the frame's alpha).
   A game hands it textures and recipes at run time, as it hands the rig its models;
   without a disc it draws its own placeholders.
   - Started (2026-10-23): `rac_demo::fx` draws flames, glows, bursts of particles
     (velocity, gravity, size and alpha over a life), beams and sprites on shots in
     flight, additive or alpha-blended, from recipes in `rac_demo/assets/effects.ron` by
     the game's texture numbers, with painted placeholders; and `rac_demo::ShotModels`
     takes a game's shot models. Strips, trails, rings, lights, shake and flashes are
     still to do.
4. **Recipes per gun:** each gun's calls with their arguments (texture or particle type,
   colour, sizes, life, blend), read gun by gun, starting with the six first guns. Each
   gun's `effects` key in `uya_weapons.ron` names its recipe and sounds. The recipes so
   far take their textures from the table above and the explosion's and N60 hit's
   particle types from their code; their sizes, colours and lives are the kit's own.
5. **Sounds** with P3.13: the gadget WAD's bank per gun and the level's bank for shots,
   by the class's sound table. Done (2026-10-23) for every gun's fire, no ammo, impact
   and loops that the kit cues ("Sounds", above).

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

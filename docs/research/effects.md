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
  on its first use in a frame (reset by 0x401978). Negative numbers are built-ins: −5 to
  −7 the frame buffers (so −7 is a copy of the frame), −8 another GS buffer; −1 and −2 a
  copy of the frame as drawn so far, made on their first use in a frame by the boot
  program's packet at 0x141AD0, which draws the frame in 16 strips of sprites, bilinear,
  into a new 256 × 256 16-bit buffer (the 512 × 416 frame shrunk to half its width and
  0.62 of its height); −3 and −4 a smaller copy the same way (0x141DC0, a buffer 64
  wide) [code, 2026-11-04: 0x401CD0, and the packet's GIF tags read from EE memory].
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
- **When it first draws one** [code, 2026-11-01; footage agrees]: `gameplay_update`
  (0x4B5368) runs the mobys' updates (0x45B2B8) and `hero_tick`, then
  `particle_update_all`, so a particle a moby makes steps once before its first draw. Its
  life test (0x40BAC0) frees it on the step that takes its life to 0, so one of life L
  shows for L − 1 frames, the first a step from its start, and one of life 1 never shows.
  The whip's life-2 sparkles show once, at half their start size. Effect textures aren't
  drawn by this pool and show for their whole count.
- **How big it draws one** [verified on UYA NTSC, 2026-10-31]: its corners are the size
  from the middle (each a unit vector from the spin's table at scratchpad 0x3400 × size /
  depth [code]), so the size is half the diagonal and the half width size / √2.
  Measured by pinning three of the Qwack-O-Ray's live 0x4E sparks in a savestate
  (still, white, texture 45): two small ones 2 u apart as a ruler (181 px at 7.4 u from
  the camera) and one of size 1.0, then 0.5, between them. Fitting texture 45's own
  picture to the sprite on screen gives a half width of 0.65–0.75 u for size 1.0 (half
  width = size fits worst, size / 2 in between), and size 0.5 is half as wide
  (`docs/re-setup.md`, "UYA NTSC-U particle size"). The streaks' widths aren't read.
- **Its fades by depth** [code, 2026-10-31, read through the VU0 code in part]: byte +9's
  low nibble × 0.25 u is taken off the depth, and alpha is scaled by that depth (in
  units) up to 1, so a particle fades out within 1 u of the camera (0x35's +9 is 0x20)
  or, with 4 (0x4E's 0x24), is gone within 1 u and whole from 2 u; its high nibble × 32
  u is a far limit (also capped by a value not traced), fading out over the last 16 u
  before it (64 u for 0x20 and 0x24).
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
  (0x462190), 0x19 (0x463938), 0x5B (0x46D1D0, a glow going from one colour to
  another), and 0x13, 0x22, 0x6F, 0x04, 0x3E, 0x74, 0x7E, 0x80, 0x6A–0x6E, 0x72, 0x76 and
  0x77.
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

### The landing marker (0x4C3500) [code, data, 2026-10-31]

Each glove and lobbing gun has one drawn each frame from where its throw would land. 0x4C3E68
follows the throw (0x4C3888) step by step for up to 300 frames until it meets the world, or
gives up if it falls 20 u past the hand; it keeps the point and normal. 0x4C3500 then draws
two quads flat on that surface, half width 1.0, turning opposite ways, a turn in 180 frames.
- **Textures:** 88 and 87 for the Miniturret (0x3462F0), Megaturret (0x3432F8), Bouncer
  (0x31CF38), Holo Shield (0x371CB8) and Nitro (0x36EA28); 17 and 18 for the Agents
  (0x35CDA0).
- **Colours:** (0xB0, 0xF0, 0xF0) at 0x70 and 0x50, alpha-blended (0x1DB038). The Agents use
  (0x7F, 0x10, 0x10) at 0x4F and (0xAF, 0, 0) at 0x70 on ground whose normal's height is
  0.9 or less (0x1DD720) or where they can't go. The Holo Shield has a pair of its own for
  that case too, not read.
- **When:** not in hero states 0x17–0x1B or 0x74. The Miniturret shows it only while a throw
  is allowed (under 9 landed). The kit shows it whenever the gun is in his hand
  (`Sights::landing`, `fx/marker.rs`), for the Agents with their red on a slope, and ignores
  those conditions. The Nitro guns' and the Infecto-Bomb's aren't in the kit; the
  Infecto-Bomb's textures aren't read.

### Quads, beams, strips and glows [code]

- `fx_quad_setup` (0x4075A8: quad, texture, blend, fixed alpha) fills a quad of four
  positions, colours and texture coordinates with a texture and a blend from the table at
  0x2C5450 (five ints an entry, the GS's A, B, C, D and FIX): 0 normal alpha, 1 additive,
  4 brightens what's under it (Cd × (1 + As)), 6 and 7 by the frame's alpha. Modes 0
  and 1 weight by the source's alpha (C = As), so the fixed alpha passed in doesn't
  count for them [data, Veldin 2026-10-29]: 0 is (Cs − Cd)·As + Cd, 1 is Cs·As + Cd.
  `fx_quad_draw` (0x405A58) draws it with a matrix.
- Built on them: a camera-facing billboard (`fx_billboard` 0x4082B0: half its width, a
  push toward the camera, roll, position, texture, colour, blend [code, 2026-10-24]); a camera-facing beam between two points (`fx_beam`
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

- **Point lights:** `light_add` (0x421DE0: radius, ?, position, colour) takes the first
  free of eight 0x20-byte slots (0x1F1BC0: the colour as floats, the second argument at
  +0xC, the position at +0x10, the radius at +0x1C) or returns −1. The colour is a word
  with red in its low byte, each byte over 128 (0xFF0000FF is red at 1.99); 0x421F08
  takes the colour as floats; 0x421FE0 frees one. A gun that keeps its light writes the
  slot each frame: the Flux's (0x3444B0) is made red but each frame set to (t/30,
  0.7 t/30, 0) as its timer t runs down from 30, 1.5 u ahead of the gun, so it shows
  orange. 0x4221E0 lists per light the world's pieces in its reach (0x4B4578, 0x4A4330,
  0x4AF350) and 0x4A3D80 adds the lights to their vertices on VU0.
- **How the lights fall off** [code for mobys; footage for the floor, 2026-11-03]: 0x4A3D80
  gives each moby, from each light in its mask, the light's colour × (1 − d / r), d its
  distance from the light and r the light's radius (the slot's +0x1C), summed into a
  colour and a direction. The floor falls off the same way, with no turn for its angle to
  the light: in the game's take of the Qwack-O-Blitzer (review take 0x93; its light at
  radius 7.5 in (1.0, 0.5, 0.25)), Veldin's sand is 1.54, 1.32 and 1.18 times as bright
  beside his feet while he fires as before, about 1.33 times 4 u away and 1.06 times 7 u
  away, which fits brightening by 1 + 0.65 × colour × (1 − d / r) in the GS's values.
  Not read: how the world's pieces are lit (0x4221E0's lists, VU1), which the footage
  stands in for, nor whether a vertex colour clamps. The kit draws each light so
  (`rac_demo`'s `fx/light.rs` and `light.wesl`): a sphere that multiplies what the depth
  prepass shows inside it by that factor, raised to the power 2.2 for its linear light,
  at most 2, before the effects.
- **The light moby** (class 0x27F, spawned by 0x300650 from a template, updated by
  0x300820): each frame a light at the template's radius × 0.8, at an offset from where
  it was spawned, whose channels each climb from a start by a rise to a top and then
  fall by a fall (bytes over 100), for a life in frames. Templates: the small blast's
  (0x2E9D20: 1.3 up, 45 frames, radius 13; red and green climbing 0.19 to 2.55 and
  falling 0.15 and 0.17, blue to 1.0 and back by 0.19); the Bouncer's muzzle (0x2AE940:
  10 frames, radius 10; red and green climbing 0.25, blue to 1.0 and back by 0.25);
  `explosion_spawn`'s (0x2AF440: 0.3 up, 60 frames, its radius the call's light
  argument when positive and the explosion in view; red +0.17 −0.11, green +0.17
  −0.13, blue to 1.0 +0.16 −0.13); and two for pieces (0x2E9C80, 0x2E9CD0).
- **Guns' lights:** the Annihilator's and the RY3NO's muzzles (radius 8, (1.0, 0.8,
  0.5) climbing over 3 frames and falling over 12, a little ahead), the Flux's (above),
  the Bouncer's (its template), the Holo Shield and the Rift Inducer in flight (radius 5;
  (0.125, 0.25, 0.5) and (0.45, 0.2, 1.0)), the Spitting Hydra's spit (0x322A80: radius
  4 × 0.7, (rand(0.1, 0.35), 0.35, rand(0.75, 1.0))), the whip's and the Qwack-O-Ray's
  (in their sections); one in 0x3098F8 of (0.25, 0.14, 1.0) isn't placed.
- **Camera shake:** three channels (0x1E2AE0, 16 bytes apart: amplitude, current, frames
  left, the most frames). Each frame 0x3F2A98 (camera, channel, kind) takes channel n
  with kind n: it keeps the most frames it has seen (forgotten once they run out),
  counts one off and moves the camera (`game_camera0` 0x1E2AC0, its forward at
  0x1E2E40, left at 0x1E2E50, up at 0x1E2E60) by amplitude × cos(2 × left) × (left /
  most)², along its up for kind 0 and its forward for kind 1. Kind 2 rolls it by
  amplitude × sin(2001° × s − 20010°) × (left / most)³ × min(6 s, 1), s the seconds since
  it was set. The trig helpers (0x40C3C8, 0x40C3E0) are VU0 microprograms (0xDA0, 0xDB0)
  and which is the sine wasn't checked. A gun sets a channel outright, so the last to
  set it wins:
  - the shared explosion with its shake flag, in view (0x4562A0 with 128) and within
    48 u of the camera: 0.075 for 40 frames;
  - the Nitro's blast (0x35E718): 0.4 − 0.02 × its distance from the camera within
    20 u, else 0 (which stops a shake under way), for 25 frames;
  - the Shock Blaster (0x36A790): 0.04, or 0.09 when its +0x48 is set (the charged
    shot), for 30 frames on channels 0 and 1; the Shock Cannon (0x385948) 0.06, or 0.12
    when its hit waits the 5 frames (its +0x48; whether V1's +0x48 is that too isn't read);
  - the Eruptor (0x38A7B8): 0.4 on channel 0 and a roll of 0.15 on channel 2, 25 frames;
  - the Infecto-Bomb (0x38F6F0) and the debris pieces (0x447D00): 0.4 − 0.0175 × the
    distance within 20 u, else 0.05, 25 frames;
  - the whip's jump attack (0x356110) 0.4 and its V4 wave (0x393690) 0.45, 25 frames;
    0x390EE0 0.45 for 40.
- **Rumble:** `rumble_start` (0x3F0118: pattern, player) plays a pattern of up to eight
  16-byte steps ended by −1 (per-port state at 0x1E2500 + port × 0x80); 0x3F0338 stops
  it. The patterns are in each level's overlay (the Shock Blaster's at 0x2B3940).
- **Flashes:** 0x403708 over the whole screen with a fading alpha (the charged Shock
  Blaster, the shared explosion).
- **Screen warps:** 0x36CBC0 (and copies in the Nitro, Flux scope and RY3NO code)
  redraws the frame (texture −7) in four mirrored quarters through texture 66. The kit's
  `Warp` emit draws one ("The Shock Cannon at V5", "The screen warp").
- **Refractions:** the Rift Inducer's lens and rings (0x3753F8, 0x3750E0) write a mask's
  alpha into the frame's, then draw the copy of the frame (texture −1) over themselves
  mixed in by it, each point taking the copy from elsewhere. The kit's `refract.rs` draws
  them from Bevy's view transmission texture ("Rift Inducer", "The lens").

### The shared explosion and the lightning bolt [code]

- `explosion_spawn` (0x32FE90) spawns class 0x13A1, which has no model, from about 30
  arguments (size, position, counts, colours and timings; most not read), each caller
  passing its own. Its update (0x330C80, 0x3302F8) spawns particles of types 0x55, 0x61,
  0x5A and 0x17, shakes the camera 0.075 and queues a flash and a glow in texture 11
  (0x3317C8). Crates and enemies use it too.
- `lightning_bolt_spawn` (0x34DAA0) spawns class 0x1195 (no model; update 0x34DD38,
  drawn by 0x34E5E0): the Spitting Hydra's, Shield Charger's, Holo Shield's and Rift
  Inducer's bolts ("The lightning bolt", under "The calls' arguments").

### Rings (class 0x307) [code, data, 2026-10-29]

- **Spawn:** `fx_ring_spawn` (0x3013B0: scale, grow, position, rotation, fade) spawns
  class 0x307 at its class's scale (0.0155) times the scale, at opacity 0x80 (+0x23, as
  `moby_init` 0x435F98 sets every moby), tinted by its caller (0x438908: r, g, b). A
  rotation with a zero x, y or z stands it on the level's up (or toward the gravity
  centre, where the level has one) at a random yaw; 0x301508 passes such a one
  (0x1D66B0), and every gun's call goes through it.
- **Update** (0x301530): each frame its scale times the grow and its opacity less the
  fade; it goes once the opacity is no more than the fade (25 frames from 0x80 at 5).
- **The model** [data, Veldin, read by the class tables at 0x1FDC50 and 0x1FD200 from the
  memory image and by `Core::moby_class` from the disc]: 64 vertices in 16 quads, four
  groups of four walls round the up axis, each group turned from the others: two 0.23 u
  high leaning out from about 0.15 u from the axis to 0.22, one 0.32 high from 0.11 to
  0.13, and one 0.37 high narrowing from 0.08 to 0.06 (at the class's scale). One 64×64
  texture, white with vertical streaks whose alpha is
  densest across the middle and none at the top and bottom edges, so it reads as a crown
  of spray. Its v runs down each wall.
- **Callers:** the Infector's burst (scale 2–2.5, grow 1.04, fade 5, (0x52, 0xD1, 5)),
  the Plasma Whip's V4 wave (3.5–4.5, 1.04, 5, (0x10, 0x10, 0xFF) to V4 and (0x40, 0x10,
  0xFF) at V5), a Miniturret landing in water (2, 1.025, 3, opacity 0x70); also
  0x32AFB0, 0x35D2C0, 0x38EF90 (the Infecto-Bomb's burst), 0x39B500, 0x32DE00, 0x2F9B30,
  0x4E20F0 and 0x451430, not read.

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
  | Spitting Hydra (0x31FB90, both) | 1 | 3 | 2, while held (charging) | 0 as each target is marked, 5 let go before 20 frames (a fizzle); corrected 2026-10-29 |
  | Disc Blade Gun (0x36EFC0, both) | 1 | 2 | | |
  | Agents of Doom (0x35C9F0) | | 0 (also with 8 agents out) | | |
  | Rift Inducer (0x372990, both) | 1 | 5 | | |
  | Holo Shield Glove (0x3717A8, both) | 2 | 1 | | 3 with none left to throw. Its V5 class (0x1C65, in the core) has no sound table on Veldin: silent. |
  | Plasma Whip (0x356110) | 1 to V2, 4 from V3 (by the row's level, 0x1425C0 + 0x7F) | on another moby (0x1E1AC4) | 0, from V3 3 | |
  | Suck Cannon (0x3619C8, both) | 2 | | 0 while sucking | |
  | Qwack-O-Ray (0x37DEB0, both) | | | 0, the beam while on | 1 as a target turns into a duck (below) |
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
- **The shots' entries** [code, 2026-11-01; spot-checked in the Holo Shield's, the
  rift's and the whip's updates]: what the guns' shots and things they leave play, of
  their own classes (Veldin's core) unless a gun's ("gun") or another class is named.
  Besides direct calls, a sequence can play entries as its animation passes set frames
  (+0x12 a count, then {entry, frame × 16} after the frame table; 0x4389D8, forwards and
  not while blending), and +0x11 names a looped entry for the sequence (no shot uses
  one). A call past a class's count is silent, and `sound_start` never drops repeats.

  | Gun | What plays it | Entry |
  |---|---|---|
  | Agents of Doom | each agent appearing (0x3C0F78) | 0x1ADA's 3 |
  | | an agent's end: its blast at V1 (0x3C2140); from V2 the comet or blast it ends in has no table | 0; silent |
  | | each bullet, V2–V4 (0x3C0128); each missile while hovering, V5 | 7; 8 |
  | | jetting in and hovering, V4–V5, stopped as it lands | 9, looped |
  | | walking (sequence 2, frames 0.5, 5.25, 9.75); running (sequence 3: 6 at frame 2, 5 at 7.3) | 10; 6, 5 |
  | Rift Inducer | open (0x373990), stopped as it goes | 0x186B's 0, looped |
  | | something pulled in reaching within 0.1 u of its middle | 2 |
  | | two rifts merging: the one taken in, at the end of its 21-frame close | 1 |
  | Holo Shield Glove | landing, as it unfolds (0x376B88) | 0x186D's 2 |
  | | standing, from unfolded through its hits and its 20-frame fold | 0, looped |
  | | each hit, and the end of its 30-frame reaction | 1, then 2 |
  | Plasma Whip | out and shown (0x356110) | gun 0, from V3 3, looped |
  | | each rope piece that meets something, each frame of a lash (0x357F10) | gun 6 |
  | | no ammo, on the tip link | 0x183C's 0 |
  | | V5's cutting wave, its 40 frames (0x3D6740) | 0x1CB8's 0, looped |
  | Miniturret Glove | landing open, or opening again (0x32AFB0; its sequence 1 plays it again at frame 0.5) | 0x1366's 1; V5 0x1367's |
  | | each shot (0x32BF28); its end's burst | 0; 3 (V5 the same of 0x1367) |
  | | V5 teleporting: as it fades out, and 5 frames into fading in (0x32DE00) | 0x1367's 5 |
  | Bouncer | the canister's burst (0x31D6C0); its bounce asks for an entry past its one | 0xE64's 0; silent |
  | | each bomblet's bounce, and its burst (0x31E488; the Annihilator's mines the same) | 0xECE's 0, 1 |
  | Qwack-O-Ray | the beam, while it is on (0x37DEB0) | gun 0, looped |
  | | a target turning into a duck | gun 1 |
  | | a duck laying, V2–V3 (0x3D1728); its end in feathers (0x3D2870) | 0x1BC3's 0; 1 |
  | | V4's duck: laying as it runs at an enemy, and blowing up | 0x189B's 0, 1 |
  | | V5's: each peck (at most every 10 frames), and blowing up | 0x1D3E's 3, 3 |
  | | an egg bouncing, and popping (V2–V5) | 0x1C59's 0, 1 |
  | | the ducks' sequences also quack (frames listed in `cache/snd/shot-sounds.md`) | |
  | RY3NO | the missile's burst at V1 (0x383370); from V2 the missile (0x1DCE) has no table; V5's big blast plays the bolt crate's | 0x1873's 0; silent; 0x1F4's 2 |
  | Disc Blade Gun | in flight, stopped at its burst (0x36FA78) | 0x1867's 0, looped |
  | Infector | the blob bouncing (at most every 7 frames), and bursting (0x3DE4F0) | 0x1E05's 0, 1 |
  | | V5's bursts (0x38E3A0, 0x39B500) | 0x188A's 0, 0x1899's 0 |
  | Plasma Coil | the ball, flying or stuck, stopped as it starts to go (0x30BF18) | 0xA9B's 0, looped |
  | | each arc, 15 frames after it sticks; the ball going | 1; 2 |
  | | V5: each arc sent (0x309480), each rebound and strike | 0x9A9's 0; 1 |
  | Lava Gun | V5: the ice on a frozen enemy shattering (0x3E1328; its time inferred) | 0x1E52's 0 |
  | Annihilator | V5's small missile bursting (0x343330; the Megaturret's and V5 agents' too) | 0xA82's 0 |
  | Suck Cannon | something taken in at the muzzle (0x362908) | gun 1 |
  | Shield Charger | the shield raised (0x340DD0), up (0x3410E8, looped), each bolt, its end | 0xA5C's (V5 0x1192's) 1, 3, 5, 2 |
  | | no ammo, at the hero | 0xA5C's 0 |

  Silent, with no table: the pod 0x1853, the comet 0x1893, 0x188F, the whip's wave
  0x1890, 0x1872, 0x1DCE, 0x1881, 0x1195, 0x1869 and every shot with no model. The
  Shock Blaster's entry 2 plays 50 frames after its last shot (0x1DD88C counts them
  down), for what isn't known.
- **In the kit and the app** (2026-10-23): `rac_demo::sfx` turns the guns' events into
  cues (fire, no ammo, impact, arc, level up, and the loops of charging, the stream, the
  vortex and the scope) and plays placeholders made in code with this volume and pan;
  with the disc, `openrac/src/gun_sounds.rs` claims the cues of the table above and plays
  the disc's sounds through `rac_audio`. Bevy's spatial audio isn't used: it fades with
  the square of each ear's distance. Since 2026-11-01 the shots' entries above too: a
  cue's row can name a kind of shot (an event's `shot`), taken before the gun's own, or
  be silent where the game plays nothing, so the placeholder stays quiet; what no event
  marks, the app follows each hero's shots by serial for (`play_shots`): the shots'
  loops (an agent hovering, an open rift, a holo shield standing, V5's cutting wave, a
  disc, a coil ball), an agent appearing and its bullets and missiles, a duck made and
  its eggs laid, a turret landing open, a holo shield's hits and their reactions' end;
  and the hero's own loops, the whip out and the Shield Charger's shield up, with its
  end. The Qwack-O-Ray's beam is a held loop (`Sustain::Beaming`). The disc test checks
  each loop is a looped entry and each of the rest isn't [data]. Not played: the
  sequences' sounds (the agents' steps, the ducks' quacks), the bounces of the
  canister, bomblets, blobs and eggs (the kit sends no event), two rifts merging, the
  coil's arcs at V1–V4, the Megaturret's teleport, the V5 ice
  shattering and the Shock Blaster's entry 2. The whip hits once a strike where the game
  sounds each rope piece each frame. Since the owner's second look (2026-11-01): the
  rift's pull, an arc event each tick it holds something, played the kit's "spark" each
  tick; it is now silent, as the game's, which plays entry 2 as it swallows (the kit's
  `Afflicted` swallow); the Suck Cannon plays its entry 1 as it takes something; the
  Agents of Doom's throw is silent, its glove's bank holding only the empty click
  [data, `openrac-tool sounds --gadget 1852`].

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
| Shock Blaster (Shock Cannon) | Close, from its code (2026-11-01): the flash on the lens, the violet-white ring sphere, the spokes, dust and the rays' white and blue plume, as the game's. The charged lance drawn (2026-11-04, compared on `review-shock-charged`): the pale violet oval round him round a blue column along the aim, swept as he turns; its domes and the uncharged lance's now open from the gun, as the game's | The charging sprites and glare at the gun aren't drawn. The screen warp (0x388508) is drawn (2026-11-04), though its ghosts show crisper on the greybox than on the game's scenery. The game's column is white at its core where the kit's is pale blue, adding in linear light |
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
| Infector (Infecto-Bomb) | Partial. V5's blob, its smoke, globs and bubbles, gas and rings are from code (2026-11-01); the game's yellow-green spatter lands on enemies, the kit's blob bursts in the air off to the side | V3's wash is its gas clouds (0x34) on a grey floor |
| Spitting Hydra | *0x1195* | 17 2C 35 6F | 11 14 23 43 57 61 71 | Bolts; a light; rumble 0x2AE990. |
| Disc Blade Gun | 0x1867 | 0F 17 2C 35 55 5A 5C 61 74 | 11 47 49 57 | A trail; the explosion. |
| Rift Inducer | 0x186B, *0x1195* | 0F 13 2C 35 55 61 | 11 14 18 23 49 57 62 63 65 75–77 | Bolts; a light; rumble 0x2B3AC0. |
| Suck Cannon | 0x1872, 0x1893 | 02 0F 17 19 2C 35 55 5A 5C 61 6A–6C | 11 21 43 | A strip (the vortex). |
| Qwack-O-Ray | 0x1BC3, 0x189B, 0x1C59, 0x1D3E | 16 17 19 35 4E 55 5A 61 66 79 | 8 11 13 14 16 57 | The beam's draw (0x37F738); lights of radius 5.5 and 7.5. |
| RY3NO | 0x1873, 0x1DCE, *0xC41*, *0x1DF6* | 13 17 2C 55 5A 5C 61 7E 80 | 11 14 43 44 66 with −7 | A screen warp; rumble 0x2B3B90. |

### The calls' arguments [code, 2026-10-24]

Read for the six first guns (PLAN.md P3.18) and written into the kit's recipes
(`rac_demo/assets/effects.ron`); the Plasma Whip, Plasma Coil and Bouncer are read too
(2026-10-28) and in recipes where the layer can draw them ("Importing them" 4), and so is
the Miniturret Glove and the Agents of Doom (2026-10-28), and the Shield Charger, Rift
Inducer, Suck Cannon, Qwack-O-Ray, RY3NO and Infector (2026-10-29). Their conventions:
- **Sizes are halves.** `fx_billboard` draws a unit quad with corners at ±1 scaled by its
  size, so a size of 0.6 is a billboard 1.2 units across; the recipes give full widths.
  Particles' sizes are taken the same way. The particle renderer scales the size by a
  sine and a cosine for each corner, so it could instead be half the diagonal, which
  would make the particles √2 smaller than drawn (not settled).
- **Colours** are RGBA bytes, R in the low byte, 0x80 showing the texture as it is, so
  0xFF is about twice as bright; the recipes divide by 0x80 and keep values over 1.
- **Levels:** guns pick colours, counts and parts by their level (V1 is level 0); the
  recipes' `from_level` entries replace the parts they give from that level on.

- **Particle types' updates** [code], as the guns use them (0x6F's, a ray from its head
  back along its step, under "Spitting Hydra", "The glare"): 0x2C fades its alpha to 0
  over its life, grows its size by a set step a frame (500 or 1000 / 210000 here), keeps
  a share of its velocity (its drag, 1.0 for none) and adds gravity; 0x0F shrinks to
  nothing, lerps its colour to an end colour, falls under gravity and either keeps 0.96
  of its velocity or, if spawned with its flag, sheds a child on odd frames; 0x61 steps
  its size and alpha by set amounts a frame; 0x55 lerps size and alpha; 0x35 and 0x5C
  reach halfway from start to final size (and half their alpha) by frame 5 (0x5C: a
  phase its spawner sets), then go on to the final size and alpha 0 (0x35's 0x467F40;
  0x4E's 0x46B050 the same, and it homes on a target within 2 u; checked on live
  particles, 2026-10-31). 0x5C's (0x46D740; spawner 0x46D570, 2026-10-31) first waits
  out a timer at +0x38 (the spawner's ninth argument), then keeps the first phase while
  its age is under +0x3E (the eleventh argument), still stepping by age / 5, so a phase
  longer than 5 overshoots halfway and one of 0 starts at half alpha; the spawner puts it
  0.9 u toward the camera. Which arguments each caller passes isn't read; 0x5A is a streak
  from its head back 0.9 of its velocity, white, fading after its first 15 frames; 0x17
  grows by a random factor a frame and fades in before it fades out. Spins are in 1/256
  turns a frame (assumed). A particle drawn with another type's texture has its texture
  byte overwritten after the spawn.
- **More of the particle types** [code, 2026-10-28]: 0x61 (spawner 0x46DCB8) builds its
  colour word as start alpha << 24 | rgb, so an rgb argument with a top byte ORs into
  the start alpha, and steps alpha by (end − start) / life as an integer; 0x35 starts at
  its f13 size and ends at f12; 0x17 by default fades in over 10 frames (alpha 0x20 plus
  9.6 a frame, to about 0x76) and out over 20 from 0x7F, or, in its "phase 2", fades
  linearly from 0x7F × frames left / life.
- **Shared helpers:** 0x448CD0 adds up to ±its argument to each axis of a vector;
  0x40D020 lerps two colours, the first at t = 0; 0x4C6EC8 copies a position unless
  0x1DB02C is set.

#### Shock Blaster (0x36A790, V1–V4)

- **At the muzzle,** once a shot (0x36BB68): 20 0x17 smokes (size 0.2–0.75, life 60,
  alpha 0x30 after a 5-frame fade-in, at the muzzle ±0.3 plus 0.4 along the aim, moving
  with the hero); 20 0x2C sparks (size 0.2–1.75, life 6, grey 0x40, alpha 0x7F, 12–16
  units a second on a 20° cone round the aim, drag 0.95, ±0.6); two 0x0F embers (size
  0.095, life 60–120, (0xFF, 0x7F, 0) at 0x4F fading to (0x7F, 0, 0) at 0x1F, 8–12
  units a second along the aim plus 3.5–5.25 at 15–80° up, gravity 14.6, with
  children); camera shake 0.04 (0.09) for 30 frames.
- **Its draw for 8 frames** (0x36C078): eight spokes of texture 26, 4 units long, for
  the first 2 frames; two squares of texture 61, (0xFF, 0x80, 0x80), alpha 20 falling to
  2, flying 14 units out along the shot and growing from 3 to 35 units across, turning
  +2° and −2.5° a frame; a texture-11 glow over the view (blend 4) and a full-screen
  flash for 3 frames, on every shot. The flash adds grey 0x80 (ALPHA 0x48, Cs·As + Cd)
  at alpha 32 × n/3 as a countdown n goes 3, 2, 1 (pvar +0x40 set from 0x1DD884 = 3,
  +0x44 = 1/3; 32.0 at 0x1DD888 [data]), so it brightens the screen by 32, 21 and 11 of
  255; the glow's alpha is 255 × n/3 [code].
- **The rays:** 20 random within 17° of the aim and 3 at 15° down (`shock_blaster_ray`
  0x36B888), each a class 0x1862 moby (0x1883 when charged) for 15 frames (20). Each
  frame (0x369CC0): two 0x55 sparkles of size 2.0 in 0x35's texture at its end, life 2,
  alpha 0x20 to 0x10, (0x80, 0x20, 0x20) and (0x80, 0x40, 0x20); two blue 0x55 glows with
  white cores at random points along it; and ribbons (0x36A198) of texture 23 (half
  width 1.4 at the end narrowing to 0.6, (0xFF, 0x80, 0x40) to grey 0x90, alpha 64 × the
  time left, normal blend) and 44 (half width 0.6, (0xFF, 0x40, 0), added). Their tail
  creeps out from the muzzle at 8 units a second. Where each ends, once (0x36A560), 15
  0x2C embers as the N60's.
- **Charging** (V2–V4): two 0x61 particles a held frame, coloured by the frames held;
  once charged, rings (0x36C910) and a screen warp (0x36CB68). Rumble 0x2B3940; no
  lights.

#### N60 Storm (0x3A5928, V1–V4)

Veldin's 0x1DE380 is 1 and nothing writes it (no store to it through the global
pointer, `gp` + 0x1AD0, anywhere in the image), so the bullets always take the second
of the two paths below: the larger muzzle flames, the tracer and the second trail.
- **Firing** (each shot: the gun's update queues the draw 0x3A63B8 for one frame and
  calls 0x3A6168). The barrels take turns (0x1DE3A0), the muzzle at the gun's +0x20 or
  +0x10. The bullet flies at 60–80 units a second.
  - Billboards for that frame, additive: texture 49 twice at random rolls, and 11 pushed
    0.08 toward the camera, all of size 0.6 and colour (0x90, 0x40, 0x20) with alpha
    0x3F; then 65 of size 0.75 at a roll of 30–60°, grey 0x90 with alpha 0x3F.
  - Nine points from the muzzle, stepping back 1/30 unit along the gun's axis; at each a
    0x55 flame of size 0.2 (0.175 without the flag), life 4, white at alpha 0x7F,
    jittered ±0.15 (±0.1), and a 0x55 of size 1.25, life 4, colour (0x80, 0x30, 0x20) at
    alpha 0x10 drawn with type 0x35's texture.
- **In flight** (the bullet's update 0x35B390, each frame while it is within 96 units of
  the camera):
  - Its draw 0x35BBC8, additive: texture 57 three times, size 0.75, grey 0x7F at alpha
    0x20, rolling at −1.5, 0.5 and 2.5 times a counter that grows by 1° a frame; 49
    twice, size 0.5, yellow (0xFF, 0xFF, 0) at alpha 0x10, rolling at ±0.5 times it; 47
    of size 0.2, grey at alpha 0x50, at a new random roll each frame.
  - With the flag, once it has flown 2 units, a tracer (0x35B9E8): a quad of texture 63
    along its way, facing the camera, alpha-blended, white at alpha 0x7F, starting 3.7
    units behind it, 4.0–4.5 long and of size 0.45–0.7 across, both picked again each
    frame.
  - With the flag, the trail 0x35C620: a 0x2C particle every 0.1 unit of the frame's
    path in 0x61's entry 4 or 5, size 0.2 growing 1000 / 210000 a frame, life 3–6,
    rising 0.1–0.3 units a second, spinning ±1–3, alpha 0x60 or 0x40 fading out; V1
    (0x7F, 0x7F, 0x10), V2 (0xFF, 0x40, 0x40) or (0xFF, 0x7F, 0x40), V3 and V4 (0xAF,
    0x10, 0x10). From V2 also a 0x0F puff a frame in 0x35's texture, size 0.45, life
    10–20, (0xFF, 0xDF, 0x80) or (0xFF, 0x7F, 0x40) at 0x7F (V3 and V4 (0xAF, 0x10,
    0x10)) fading to red at 0, moving back at 1/30 of the bullet's speed, ±6 units a
    second on each axis and 3 up, gravity 6.
  - Without it, the trail 0x35C2A8 instead: ten 0x61 particles a frame, size 0.3
    shrinking to 0.015, lives of 6–12, and a similar puff.
- **Landing.** At V1 anything it hits ends it (state 1) in the shared explosion at size
  0.14 (below), 0.1 unit back. From V2 a target hit ends it with nothing drawn, and a
  wall (or the end of its life, 30 frames at V1, 46 after) runs 0x35BDC8 and it bounces
  on: eight 0x2C embers of size 0.05–0.2 within ±0.1, thrown toward the camera (±80° of
  its yaw, 0–80° up) at 0.5–1.0 units a second, growing 500 / 210000 a frame, life 30,
  grey 0x7F at alpha 0x7F, spinning ±1–2; from V3, eight 0x5A glares toward the camera,
  lives 25–40; from V4, five 0x5C bursts.

#### N90 Hurricane (0x322E68, V5) [code, 2026-11-03]

V5's bullets are their own class, 0x1881 (0x384068: the class's scale times 18, from
0x1DDDD8 [data]; update 0x384338), with none of the N60's sprites or tracer.
- **In flight, each frame:** at where it was, two 0x55 flames (0x46C468) in 0x35's
  texture of half 1.0, (0xFF, 0x20, 0x10) at 0x40, and one of half 0.6, (0xFF, 0x4F,
  0x4F) at 0x30, for 2 frames; along its step, two in 0x5B's texture, half 0.2–0.6
  shrinking to a fifth, (0xFF, 0x80, 0x40) at 0x20 fading, for 15.
- **Its tail** (0x384CF0 keeps where it was its last 20 frames; 0x384F58 draws them, in
  texture 14, added): a strip 0.4 across through them, its colour lerped (0x40D020) from
  grey 0x70 at the bullet through grey 0x20 to red at its end, its alpha 0x40 falling to 0
  along it; its texture scrolls. It fades over 6 frames once the bullet ends. This is the
  one long flame the game's take shows.
- **Landing** (its own collision; the count at +0x3C counts its glances): off the world,
  and off a moby that is neither class 0x1F6 nor of category 5, it glances on (+0x3C
  plus 1, the velocity reflected); class 0x1F6 or category 5 ends it (state 1, 6 frames),
  with the shock mod's check (0x34BF98). Either way a 1 u damage ball (0x3FB1D0) and
  0x385220. While +0x3C is under 2 (its first glance, or a hit after at most one) also
  the shared explosion (below, "At each call") and ten 0x5A glares (0x46CF70, as a
  disc's glance: toward the camera at 0.1–0.2 u a frame, each turned ±0.05 a frame more,
  life 25–40); after, two glares. None when it meets the hero.
- **0x385220,** each landing: five 0x5C bursts (0x46D570) 0.9 u toward the camera: half
  0.1 to 1.05, white at 0x64, life 40, texture 1; 0.4 to 2.05, (0xF7, 0x82, 0x38) at
  0x3C, life 30; 0.15 to 1.65, the same at 0x32, life 40, texture 4; 0.5 to 2.35, (0xF7,
  0x45, 0) at 0x3C, life 27, after 3 frames; 0.3 to 1.75, the same at 0x1E, life 27,
  after 3, texture 4. While +0x3C is under 2 also a 0x5B sparkle (0x46D1D0) 0.15 u toward
  the camera ±0.175, half 0.24–0.38, drifting at up to 0.5 u/s, gravity 0.5 u/s², from
  (0xFF, 0x50, 0x50) at 0x4F to (0xEE, 0x34, 0x24) at 0x10 over 25–60 frames (10–20 on a
  target), and three dark bursts of texture 2, (0x14, 0x14, 0x14) at 0x80, 0x80 and 0x64,
  half 0.9 to 0.7, 1.7 to 0.7 and 2.0 to 1.5, life 10, drawn blended (render byte 0x44
  for the spawner's 0x48).
- **The 0x5B sparkle's update** (0x46D330): its colour lerps from start to end over its
  life (0x444338), it moves and falls; spawned with its flag (+0x3A), on odd frames it
  sheds a 0x0F (0x462190) of its size and colour going on at 0.75–0.95 of its velocity
  plus 15–25% of it any way, life 15–30, instead of keeping 0.96 of its speed.

#### Nitro Launcher (0x36E360, V1–V4)

No muzzle effect; the charge (0x185A) is its model only. Its blast (class 0x1858, update
0x35E718, 100 frames) never calls `explosion_spawn`:
- **Frames 1–9, the implosion** (0x35FCB8): eight billboards of 73 and 74, normal blend,
  4.8, 4.4, 3.6 and 4.0 units across times a scale falling from 1.37 to 0.33, dark blue
  ((0x05, 0x05, 0x40), (0x05, 0x05, 0x20) at 0x7F; V3 and V4 red), turning at −1 to
  −5° a frame; three masked discs of 71 and 72; a refraction ring of 77.
- **Frames 11–12:** a full-screen flash (0x35FC70: grey 0x80 added at alpha 0x80, half
  of full white over the whole screen [code]), and 50 0x61 smokes a frame in 0x17's texture
  going out level at 10 units a second, size 0.6 to 1.1, life 20–40, grey 0x60 at 0x40
  (V3 on (0x80, 0x60, 0x40)).
- **Frame 13, the burst:** shake 0.4 − 0.02 × the camera's distance for 25 frames; four
  0x35 sparkles (final sizes 17.5, 12.5, 8.75, 6.25 from 0.1, life 20; blue (0x20, 0x40,
  0x80) and (0x20, 0x40, 0x60) at 0xBF, then white at 0xC0 and 0xB0; V3 and V4 orange);
  three 0x5C (2.1 to 2.7, 2.1 to 5.1, 4.5 to 6.0; grey 0x78 at 0x46, 0x46, 0x2D; life 24);
  from V2, 50 0x76 acid streaks 2 units long at 20–25 units a second and an acid cloud
  (class 0x1898).
- **Frames 14–32:** three 0x5B glows a frame (size 0.65–0.85, (0x32, 0x32, 0x32) at 0x28
  fading to (0x10, 0x10, 0x10) at 0x08, life 15–25) and a 0x6F streak; two torus rings
  of texture 43 growing out to about 14 units; from V2 acid spray bands. Its fire-ring
  strip (0x35F3E0) is drawn at alpha 0 (its pvar is never written).
- The landing marker while aiming: two 2-unit quads of 88 and 87 (0x4C3500).

#### Nitro Eruptor's burst (class 0x1888, update 0x38A7B8, V5) [code, 2026-11-03; all of it 2026-11-05]

Spawned by 0x38A668 (position, the ground point a line test finds up to 20 u below the
charge, the charge's pvars) with its timer +0x24 at 105, its size +0x28 at 2.0, +0x2C at
30, +0x30 at 0x50 and +0x3C at 20. Frame by frame (each its update, 0x40BA90 counting a
timer down):
- **Gathering** (state 0 while the timer is above 90, 14 frames): +0x28 falls 0.1 a frame
  (1.9 to 0.6), and 0x38C238 draws round it, by +0x28:
  - eight billboards of 73 and 74 (`fx_billboard`), half +0x28 × 1.2, 1.1, 0.9 and 1.0,
    dark blue ((0x05, 0x05, 0x40) and (0x05, 0x05, 0x20) at 0x7F), normal blend, turning
    3–5°, 1–4° and 2–4° a frame either way: V1's implosion, shrinking;
  - three masked discs (0x4086B8: a 16-point disc facing the camera, radius +0x28 × 1.4,
    texture 71's alpha into the frame's, then 72 turned and coloured, mixed in by it) in
    (0xFF, 0, 0x32), (0x80, 0x40, 0x20) and (0x7F, 0x20, 0x20) at 0x35;
  - two of 0x38BDF8 (a quad of 54 masking a magnified copy of the frame, the rift's
    rings' way), half +0x28 × 2.1;
  - twelve bands of a ball (0x38C110 → 0x38B628): each a strip of the unit ball between
    two latitudes, 21 pairs of points over its span of longitude, u along it, v across,
    in a matrix turned about its z by its own angle (random at the spawn, 0x1E1FA0–
    0x1E1FCC) and leaning from the hero (0x40C658 of (0, the burst's elevation from him −
    20°, its heading)), so its axis stands 20° back toward him, scaled by +0x28 × k;
    added. Latitudes (25°, 35°), (10°, 20°), (−2°, 7°), (−5°, −10°), (−12°, −18°) and
    (5°, 15°) with k 1.5, 1.7, 2.0, 1.8, 1.7 and 1.5: six over 180° in texture 42,
    (0x80, 0x40, 0x20) at +0x2C × 6 (0xB4), turning 17°, 22°, 27°, 29°, 26° and 22° a
    frame, and six over 90° in texture 63, (0x80, 0x80, 0x10) at +0x2C × 7 (0xD2),
    turning 7°, 12°, 17°, 19°, 16° and 12°. Textures from 0x1DDFCC and 0x1DDFD0 [data].
- **The burst** (state 0 at 90): what is listed below, with 0x38CB78 drawn once (+0x28 at
  0.5), and state 1.
- **State 1** (3 frames): 0x38BDB0, the whole screen grey 0x80 added at 0x80 (V1's flash),
  and each frame 100 0x61s (0x46DCB8) in 0x17's texture from 0.3 u above the ground
  point, out level any way at 15 u/s, size 0.8 to 1.2, life 20–40, (0xFF, 0x80, 0x40)
  from 0x70 to nothing, spin 2–4 steps: an orange fire ring along the ground.
- **State 2** (30 frames, while in view within 100 u): +0x28 grows 0.5 a frame to 1.6,
  0x1DDF78 0.7 a frame from 0 and 0x1DDF70 0.8 from 1; +0x2C counts down from 30 and
  +0x30 falls 4 a frame. Drawn:
  - 0x38CB78: billboards of 76 at half 0x1DDF78 × 1.2 in (5, 5, 0x20) (to 25 u, a dark
    ring), of 76 at +0x28 × 0.5 in (0x32, 0, 0xFF) and × 0.2 in (5, 5, 0x30), all at
    +0x2C × 4, normal blend; of 75 at +0x28 × 0.6 in (0x80, 0x40, 0x20) at 0x2F; and
    two stars of 57 at +0x28 × 3 in (0xFF, 0x40, 0x20) at +0x2C × 7, added; and the six
    orange bands at 0x1DDF78 × k × 0.4, at +0x2C × 6, so growing to about 17 u as they
    fade;
  - 0x38D148 and 0x38D398: the ring at 0x2B3E60 [data] (64 quads, 16 round, the Dread
    blast's shape: radius 0.407–0.469, height ±0.062, v 0.75 to 0.25) × 0x1DDF70, at
    (2, 2, 0.25) and (1, 1, 0.125), 0.25 u above the ground point, texture 43 (0x1DE000)
    in (0x80, 0x40, 0x20) (0x1DE004) at +0x30, normal blend (0x1DDFEC–0x1DDFFC): two
    flat rings going out along the ground, gone in 20 frames;
  - while 0x1E1F9C runs (set at the spawn from the last burst's 30 less its distance from
    the camera, kept within 15 to 30; 15 at first), the screen warp 0x38BA90, the Shock
    Cannon's (texture 66, four quarters, the frame from (10, 8) to (492, 432)) at 5 ×
    that timer.
- **Throughout its first 19 frames** (0x38B8B0, while +0x3C runs): `fx_ring_fan` of 4
  rings in texture 23 (0x1DDF7C), (0xFF, 0x80, 0x80) at 3 × +0x3C, added, its pole back
  along the charge's velocity (the charge's pvar 0, in u a frame) and its middle five
  frames of it ahead, scaled across by 0x1DDF84 and 0x1DDF88 (from 1, growing 0.8 and
  0.4 a frame for 4 frames, then falling 0.5 and 0.3).
- **State 3:** it goes.

Its first frame bursting:
- 49 sparks (0x470F00) out at 18–27 u/s level within a random turn of up to 130° of the
  way to the camera (0x440918(0, 2.27)) and 0–14 u/s up, life 25–35, in (0xFF, 0x80,
  0x80); four growing sparkles (0x467DA8) to 21, 15, 10.5 and 7.5 in (0x80, 0x40, 0x20)
  and (0xA0, 0x40, 0x20) at 0xBF and white at 0xC0 and 0xB0; three 0x5C (0x46D570) in
  grey 0x78 at 0x46, 0x46 and 0x2D; 0x38D5E0 (not read); the shake (above).
- **The rocks:** five pieces of debris (0x329470(3.0, 14.6, …, life 100–130, kind 3,
  bouncing)), each thrown at 8–15 u/s at a random turn of up to 80° from the way to the
  camera (0x440918(0, 1.396)) and 5°–30° up; and five 0x61 particles in its rock textures
  (entries 10–12, blended 0x44) from within 0.2 u of the burst at 7–10 u/s level the same
  way and 0–12 u/s up, gravity 14.6 u/s² (0.00405 a frame²), life 90–150, size 0.15–0.2
  throughout, ±2–6 spin steps a frame, in (0x38, 0x35, 0x27) at 0x7F throughout. The
  way to the camera is level (0x40C490 of the camera's x and y less the burst's), and
  0x440918(a, b) is a random a to b of either sign.
- **The rest of its first frame:** the light moby from the template at 0x2BEE10
  (0x38D5E0) [data]: 1 u up, radius 20 (so 16), 70 frames, red and green climbing 1.28 a
  frame to 2.55 and falling 0.09 and 0.12, blue to 1.0 falling 0.13; the shake (above);
  rumble 0x2B3DE0; fifteen bullets (0x35D8E8, `weapons.md`). The 49 sparks are type
  0x77 streaks (0x470F00): width 0.04, from 2.7 u behind along their way; their widths'
  use isn't read.
- **In the kit** (2026-11-05, `effects.ron`'s "nitro" at V5): its own burst, no longer
  V4's. The bands are `Swell`s of shape `Band` spinning about an axis `Leaning(20.0)`
  toward the camera, which stands in for the hero; the frames after the burst are timed
  by `After` (emits made a number of ticks after the event). Not drawn: the dome of 23
  (the impact gives no velocity), the masked discs of 71 and 72 (V1's aren't either),
  the quads of 54 and the rumble. Its fifteen bullets are the agents' small bullets,
  drawn from the default recipe's "slug": until now each one's hit showed the gun's
  whole burst again.
- **Compared with the game** (2026-11-05, `review-nitro-0x7B` against the kit's
  `-nitro-near` at V5, `--every-tick`): close. Both show the orange and yellow arcs
  spinning round a tilted axis and shrinking for a quarter of a second, the small
  red-orange pop, the white screen, then a wide burst of fire with orange rings and
  rocks going out along the ground and the bullets flying off. The game's fireball is
  yellow-white where the kit's is redder and dimmer (it adds in linear light, P1.9).
- **A piece of debris** (0x329470, a moby of class 0x1A50, the chunk; update 0x329678):
  - Its model: Veldin's core has the chunk, one surface of 53 points about 0.17 × 0.17 ×
    0.14 u in a 128² texture of dark grey (mean texel 47) [data, the disc test
    `the_chunk_is_a_small_textured_rock`]. Drawn at its class's scale × the first
    argument (3 for the Eruptor, so about half a unit), its colour set to 0x7F grey
    (0x438908, libuya's `primaryColor`) and +0x78 to the caller's word (libuya's
    `glow_rgba`; the Eruptor's 0x80204080, brown). The game's chunks show maroon, as the
    texture × (1, 0.5, 0.25) would be [footage, reported]; how +0x78 is drawn isn't read.
  - Its flight: it turns 0.105–0.21 rad (6°–12°) a frame about its x and y each; moves
    by its speed; when told to bounce, a line test (0x3F8448) from where it was finds the
    world and it leaves turned off the surface (0x40C260) at 0.8 of its speed (0x40C0F0
    sets a vector's length); then falls at the second argument (14.6 u/s²).
  - Its end: it shrinks over the last quarter of its life (rounded down) to nothing, and
    goes then or once it leaves the level's bounds.
  - Its smoke: its kind, one of three at random when 3 is asked: none, a puff in half the
    frames (a coin each frame), or three puffs a frame, each where it now is. A puff
    (0x460318) is particle type 4 in its type's first texture, life 45, drifting at 1 u/s
    any way (a cube's direction set to that length) and keeping 0.98 of it a frame,
    turning a spin step a frame; its size grows evenly from 50 × 1000 to (50 + 120) ×
    1000 (0.24 to 0.81 u) through its life, 50 and 120 being the spawner's stack
    arguments (the Eruptor's, sp+0 and sp+8 at 0x38ADB8), and it fades from its colour
    to nothing (type 4's update, 0x460418). Its colour is one of four greys at alpha 0x2F, (0xFF, 0x80,
    0xC0, 0x40), unless the call gives others; added (0x48) when the last argument is
    set, as the Eruptor's is, else blended (0x44).
  - Its other users: the explosion's debris count (`explosion_spawn`'s t3) and the
    RY3NO's sun (0x391648: seven at 1.6, life 60–90).
  - In the kit: `Emit::Debris` (`fx/debris.rs`), the chunk imported with the effects'
    models; tinted by the colour above.

#### Lava Gun (0x303F88, V1–V4)

- **The stream** is a ribbon drawn by 0x305858 through up to 35 points from the nozzle:
  a core of texture 10 ((0x9F, 0x9F, 0x9F) at 0x60, half mixed), a band of 14 (grey
  0x7F at 0x50, added) and a glow of 43 ((0x80, 0x30, 0x10) at 0x55, added) six times as
  wide; 0.05–0.1 units across at the nozzle (the glow 0.3–0.6), narrowing by 0.94 a
  point; textures scrolling. Two glows of 44 at the nozzle, 0.5 and 1.0 across.
  [code, 2026-10-28, 0x305CA0] Its blends take the GS's fixed alpha, not the vertices':
  the core 0x64 ((Cs − Cd) × FIX + Cd) and the band and glow 0x68 (Cs × FIX + Cd), FIX
  0x40, 0x40 and 0x50, so the alphas above don't count. The half width is rand(0.5, 1) ×
  0.05 each frame (0.1 at V5, its core in texture 69 and its glow three times as wide,
  in (0x10, 0x30, 0x80)), one quad between each two points.
- **What it sheds:** from each point each frame, one time in 7 a 0x6D smoke (size
  0.125–0.25, (0x80, 0x30, 0x10) darkened by up to half, alpha 0x40–0x80 falling 1 a
  frame, 3 units a second along the stream, drag 0.99, sinking then rising once dark,
  growing 0.0075 a frame, darkening 0.975 a frame) and one time in 22 a 0x19 spark (size
  0.024–0.143, white going yellow then red, life 80, gravity 22.7, stopping where it
  lands). The smokes take turns at being added and blended: 0x305710 passes the
  particle's blend byte (+3) from the pair at 0x1DC610 by a counter's parity [code],
  and the pair holds 0x48 and 0x44 [data, Veldin savestate]. Their size is rand(0.05,
  0.1) × the stream's strength × 0.1, times 2.5 in the spawner (0x46F808) [code], so
  0.125–0.25 at full strength.
- **Pools** [code, 2026-10-29]: a point that meets something that isn't an enemy leaves a
  burning pool one time in 11 (0x3063C8 into the first free of 10 slots of 0x530 bytes at
  0x1DC608; none when all are taken), at the point it struck, with that surface's normal.
  Its strength (0x306658) springs to 0.5 through `game_spring` (gain 0.002, damping
  0.0002, at most 0.02 a frame): 36 frames, easing in; it holds 60 frames, then springs
  back to 0 with the rate it had, so it swells about 7% before it falls (46 frames).
  Drawn by 0x306838, to V4:
  - two glows of 11 lying on the surface (0x306DF8: a quad turned onto the normal, half
    0.35 in (0x80, 0x30, 0x10) and half 0.075 in white, added), not scaled by the
    strength;
  - two flames of 23, one blended (0x44) and one added (0x48), standing on the level's
    up from the point moved up to 0.1 either way level, facing the camera: half width
    rand(0.3, 0.5) at the foot and 0.4 of that at the top, height 2 × rand(0.9, 1), all
    times the strength while it grows and 0.5 after; (0x80, 0x30, 0x10) at the foot
    fading to alpha 0 at the top; the texture's u runs up the flame and scrolls up by
    rand(0.01, 0.02) a frame, mirrored across in half of them;
  - alpha 0x40 for all while it grows and holds, then strength × 0x80 as it falls.
  From V5, only the glows, the big one in (0x20, 0x20, 0x80). No lights or shake.

#### Annihilator (0x36D168, V1–V4)

- **At the muzzle:** 20 0x13 streaks (0.75 long, 6–18 units a second, 20° off the aim,
  grey 0x7F at 0x30 losing 2 a frame); 20 0x2C sparks (size 0.4–0.75, 12–16 units a
  second on a 12° cone, drag 0.95, life 10, grey 0x40, in 0x5B's texture); 6 frames of
  0x17 smoke (four a frame, (0x80, 0x40, 0x20), life 8) and white sparks; a light
  (radius 8, (1.0, 0.8, 0.5) rising over 3 frames and falling over 12). Rumble 0x2B39C0.
  The missile itself carries no light.
- **In flight:** a 0x2C smoke every 0.15 unit (size 0.15, growing 200–1000 / 210000 a
  frame, rising 0.2–1 units a second, life 30–55; half grey 0x7F at 0x50, added (V2 on
  (0x80, 0x40, 0x10)), half black at 0x40, normal); at its tail three glows of 44 (1.2–
  1.6, 0.6–0.8 and 0.3–0.4 across) and a ribbon of 23 through its last four places.
- **Landing:** the shared explosion at size 0.6 (V1) or 1.0 (V2–V4), and from V2 four
  0x35 sparkles (10.5, 7.5, 5.25, 3.75; blue) and three 0x5C, 0.5 unit toward the camera.

#### Flux Rifle (0x3444B0, V1–V4)

- **Unscoped, at the muzzle:** 10 0x74 streaks (12 at V4) 0.5 long at 5–10 units a
  second in any direction, life 10–25, green at V1 (blue V2, orange V3, red V4); four
  0x35 sparkles (final 5.25, 3.75, 2.625, 1.875 from 0.1, life 14, two in the level's
  colours, two white); three 0x5C (0.84 to 1.08, 0.84 to 2.04, 1.8 to 2.4, life 20)
  0.9 unit toward the camera; a light of radius 10 fading over 29 frames, orange.
- **In flight** (class 0xCF7, 400 units a second, 19 frames): a beam from the muzzle to
  the shot for 40 frames, of a 20-sided tube of 21 (0.4 across, normal blend), a glow
  ribbon of 14 (2.0) and a thin ribbon of 23 (0.2), both added, in the level's colours
  (teal and green at V1, blue V2, violet V3, red V4; each level's are in `effects.ron`
  [code]); a helix of two ribbons of 14 round
  the path; from V3 two sparkles of 54 a frame, growing from 0.6–1.0 to 1.5–2.5 units,
  pulsing between two blues (V4 reds), life 45–60.
- **Landing** (0x346EF0): at V1 ten 0x5A glares toward the camera, four 0x35 and three
  0x5C; from V2 the shared explosion at size 0.2, 0.3 and 0.4 (V2, V3, V4) with the
  counts 40, 20, 30, 40 and two glares; from V4 also ten 0x0F sparks and shells
  (0x447D00) and shake.
- **Scoped:** a beam of 14 from the eye 100 units along the aim (class 0x186F), 40 0x61
  motes a frame along it, from V4 rings (0x37D1E0); the scope overlay (0x345D48).
- **V5, the Splitter Rifle** [code, 2026-11-03]: the gun keeps class 0xC04 and its shot
  0xCF7 at every level. The shot's spawn (0x3463A0) sets V4's colours, sizes and counts
  from V4 up, but the texture of the trail's sprites (0x1E1A84, read by their draw 0x348C48;
  laid by 0x348A38 twice a frame from V3): 54 at V3–V4, a white ring, and 9 from V5, four soft
  spots in near-black (4, 4, 4), in blend 0 (normal), so V5's trail is dark puffs among
  its red ribbons. The sprites' colour swings between (0x7F, 0x30, 0x30) and (0x7F, 0x7F,
  0x30) by the sine of their age.
- **Its landing from V4** (0x346EF0): the shared explosion at S 0.4 (V4) or 0.5 (from V5),
  and pieces (0x447D00) with the shot given; read from the call's registers (f14 4.0, f15
  2.0, f16 9.0, f17 1.0, f18 15.0; a3 10, t0 3, t1 0x10, t2 −1, t3 1; stack 1, −1, 0,
  0x1010001). With a moby, the pieces' domes (class 0x70, `0x2F95A0`): factor 4 for 16
  frames in (0x96, 0x96, 0x96) at 0x20 and for 22 in (0x7F, 0x7F, 0x50) at 0x20, both only
  with the camera more than 9 u away (and a load figure, 0x1DFBCC, under 0.95); factor 4
  for 30 frames in (0x7F, 0x7F, 0) at 0x30; factor 2 for 27 in white at 0x20. With them
  ten 0x0F sparks (0x462190) and the shake within 20 u. These domes are the white-rimmed
  yellow ring the game's V5 take shows where its shot lands.

#### Plasma Whip (0x356110, V1–V4)

Read 2026-10-28 [code unless marked], as are the Plasma Coil and Bouncer below; spot-
checked against the Veldin image: the textures at 0x1DD6D0 (43) and 0x1DD6E8 (14) [data].
The colours change at V3, from red-orange to blue, and V4 adds a second strand and the
shock wave; the level colours are immediates in the code, not tables.
- **Light:** `light_add` of radius 5.5 and (0xFF, 0x80, 0x40) once, then each frame at
  the whip with radius 4.0 + 0.5 sin(0.1 × phase) and its colour lerped between two by
  0.5 + 0.5 sin(phase), the phase going rand(160, 800)° a second: V1–V2 (0xFF, 0x55,
  0x20) and (0xFF, 0x85, 0x55), V3–V4 (0x20, 0x55, 0xFF) and (0x55, 0x85, 0xFF). It goes
  when the links hide. The links are tinted (0xBF, 0x40, 0x20) at V1–V2, (0x20, 0x40,
  0xBF) at V3–V4 (0x438908).
- **The rope, drawn each frame the links show** (0x359110 on the second draw list):
  - Nine camera-facing quads of texture 14, one per piece from each link to the next,
    the whip's joint 0 to the tip (0x3599B0), in the quad helper's blend 1 (Cs·As + Cd,
    so their own alpha counts; below): half width 0.12 in (0x80, 0x40, 0x20) at alpha
    0x5F, and 0.06 in (0x80, 0x20, 0x20) at 0x5F; V3–V4 both (0x20, 0x40, 0x80). From V5
    both black at 0x7F in blend 0 (blended), half width 0.06 and 0.03 (0.09 and 0.045
    while attacking). Their u runs 0 to 1 along each piece, less a phase going up 0.1 a
    frame (wrapping at 4), so the texture moves out along the rope (corrected
    2026-10-29: earlier read as added at FIX 0x80).
  - A strip of texture 43 through joint 0 and rope points 1–8, fading to nothing over
    its last segment (0x359518): a core of half width rand(0.03, 0.05) (rand(0.01,
    0.03) while attacking or thrown), grey 0x7F added at FIX 0x4F, and a glow p2 =
    rand(3, 5) times as wide at FIX 0x1A (a third of the core's) in (0xFF, 0x20, 0x20),
    V3–V4 (0x40, 0x80, 0xFF), both widths picked again each frame; from V5 the core
    (0, 0, 0xFF) and the glow (0x10, 0x10, 0xFF). Each piece's far end takes the side
    the piece before ends with, so the strip is joined; the glow's u runs 0.33 a piece.
  - From V4 a second strand: the same strip with all ten points jittered ±0.12 along
    each axis each frame (±0.15 attacking), core half width 0.032 in grey, glow 1.2
    times as wide in (0xFF, 0x20, 0x20), from V5 (0x20, 0x20, 0xFF).
- **Along the rope each frame** (0x357F10), at random points of its eight segments:
  - attacking or thrown, eight 0x61 a segment, still, size 0.2 to 0.3, life 2–3, (0x40–
    0x64, 0x40, 0x40) at 0x5F, in 0x5B's texture 12, added; V3–V4 0.25 to 0.35, grey
    0xC8, texture 27, three in four added and one blended; and two rolls a segment, one
    in 100 for a 0x19 spark (as the Lava Gun's) at V1–V2, one in 200 at V3–V4 for a 0x72
    (size 0.1–0.2 shrinking 0.0286 u/s, (0x40, 0x40, 0x40–0x64) at 0x64 going to (0x0A,
    0x0A, 0xFF) at 0, life 120–240, gravity 22.7, stopping where it lands), rising 0.15–
    0.6 u/s;
  - at rest, two 0x61 a segment (size 0.1 to 0.22, life 6–12, alpha 0x7F losing 0x3F
    over its life, texture 12 or 27, (0x40–0x64, 0x40, 0x40) at V1–V4) and a 0x17
    smoke rising 0.15–0.6 u/s (size 0.1–0.2 shrinking by 0.99–0.995 a frame, spin ±1–5;
    (0x46–0x96, 0x40, 0x40) in texture 0, V3–V4 grey 0x60 in 27; two in three added),
    with the same spark rolls; the V3–V4 spark goes to (0x46, 0x46, 0xFF) instead.
  - The segments are rope points 1–2 to 8–9 (0x2843A0, the hand's piece left out).
    "Attacking" is states 0x13 and 0x14 (the combo and the jump attack) or the end
    thrown. Texture 12 is 0x5B's entry 0 and 27 its entry 1 (0x221284, 0x5B's list).
  - **V5** (0x357F10 [code, 2026-11-01]): attacking, eight a segment each
    of black (size rand(0.4, 0.7) to 0.03, life 2–3, alpha 0x7F held, blended, 0x35's
    entry 0, spin ±2–3) and violet (the same × 1.3 to 0.06, (0x30, 0, 0x7C), alpha 0x30
    held). At rest, two a segment of each of (size 0.11 to 0.2, life 6–12, (0x20, 0x40,
    0x80), alpha 0x3F to 0, added, spin 1), one in 0x68's entries 0–3 at random and one
    in 0x5B's entry 1, and on each of those black (rand(0.25, 0.35) to 0.01, life 8,
    0x7F held, blended, its texture read through the words at 0x1DD6C8 and 0x1DD6CC, which
    hold 0x35 and 0 in memory, so 0x35's entry 0)
    and violet (× 1.1 to 0.02, alpha 0x30 to 0). No smoke below V6.
- **Hits:** each link whose line meets something, each frame (0x357B20 × 10): 30 0x2C
  (size 0.01–0.1 growing 0.143 u/s, ±0.3, 0.1–3.5 u/s toward the camera's bearing ±80°
  and 0–80° up, life 7–15, grey 0x7F fading from 0x7F, spin ±1–2, texture 12 or 27);
  rumble 0x2B2B80 unless thrown. On what isn't an enemy, a scorch mark (0x359F28, 25
  slots at 0x284580): the Lava Gun's burning pool with its own numbers, growing on a
  spring to 0.5, holding 60 frames and shrinking; two upright flames of 23 (half width
  0.3–0.5, height 2 × rand(0.2, 1.3), the top 0.4 as wide, one blended, one added) and
  a decal of 11 on the hit's normal (half width 0.35, and 0.075 in white), (0x80, 0x30,
  0x10) at V1–V2, (0x10, 0x30, 0x80) at V3–V4, alpha 0x40.
- **Shake:** 0.4 for 25 frames at the jump attack's frame 30, V1–V3.
- **The V4 shock wave** (class 0x1890, update 0x393690): 45 u/s for 40 frames; every
  third frame from its 11th it casts a line from 6 u behind it 10 u down, and on the
  ground calls `explosion_spawn` (S 0.5, no shake, counts 2, 2, 2, 5, 2 glares, no
  debris, the distortion where the level has no gravity centre, no dome, flash, glow or
  rings; blues: (0x10, 0x2E, 0xFF), (0x20, 0x80, 0xFF), (0x40, 0x7F, 0xFF), (0x20, 0x30,
  0xFF), (0x5F, 0x8F, 0xFF), (0x20, 0x80, 0xFF), smoke (0x30, 0x30, 0xFF), ring (0x40,
  0x80, 0xFF)) and spawns a ring moby of class 0x307 ("Rings": rand(3.5, 4.5) times its
  class's scale, random yaw, tinted (0x10, 0x10, 0xFF), at V5 (0x40, 0x10, 0xFF),
  growing 1.04 a frame and fading 5 a frame);
  shake 0.45 for 25 frames on a cast at age 12–13. On some ground materials (above 1,
  not 0xB) two 0x61 rocks (0x393E28: size 0.06–0.22, 1–4.5 u/s any way plus 5–8 up,
  gravity 15.6, life 90–150, browns, textures 48–50, blended). From V5 each cast adds
  four each (0x393BC0 [code, 2026-11-01]) of black (size rand(0.8, 1.25) to 0.01, life 60,
  gravity 15.6, 1.5–4 u/s any way and 1–3 u/s more up, 0x7F held, blended, 0x35's entry
  0) and violet (× 1.25 to 0.02, (0x30, 0, 0x7C), 0x30 held). Its class has no model.

#### The Quantum Whip's cutting wave (class 0x1CB8, V5 and up) [code, disc, 2026-11-01]

- **The spawn** (0x3D6578): from 0.7 u above his feet (0x1A4CC0) and 1 u along his facing,
  turned 0, −20° or +10° for swings 1, 2 and 3 (frames 10, 10 and 30); 0.5 u a frame for
  40 frames. Scale +0x2C 0.1 (2.151 times the class's 0.046492), yaw its heading + 90° (its
  +x to the left of its flight, its +y back along it), matrix row +0xD0 stretched to
  length 3, opacity 0x40 (half), mode bits clear 0x200 and set 0x100,
  `moby_set_colour(0, 0, 0)`.
- **The class** [disc]: 46 vertices, 30 triangles, all on joint 0 of 7, one sequence of 11
  equal frames, so rigid. One 64×64 grey metal texture. As drawn, a shallow arrowhead 4.8 u
  across, its tips 2.25 u behind its middle: joints across and back (2.40, 2.25), (1.85,
  1.23), (0.80, 0.33), (0, 0.03), then the same on the other side.
- **Each frame it flies** (0x3D6740): with fewer than 30 frames left, for each of its 6
  joint pieces at a random point black (size rand(0.4, 0.8) to 0.1, life 3–6, spin ±2–3,
  0x7F held, blended, 0x35's entry 0) and violet (× 1.35, (0x30, 0, 0x7C), 0x30 held, the
  same texture), moving −1 u/s along its flight, ±2 u/s each way and −1 u/s up. A looped
  sound (class 0, entry 0, flag 4). No sparks or hit sound in its update.
- **Its draw** (0x3D6C00): a ribbon (0x4068C8) through 8 places kept every 8 frames, ±2.25
  along its x, from (0x30, 0x10, 0xFF) at 0x7F to (0x20, 0, 0xFF) at 0, effect texture 6,
  added; and two strands (0x359518) from the middle joint to each tip through 4 points (t
  = 0, 0.25, 0.5, 0.75), jittered ±0.175, half width rand(0.03, 0.05), grey 0x7F at FIX
  0x4F, with a glow rand(3, 5) times as wide in (0x40, 0x80, 0xFF) at FIX 0x1A.
- **Against the game** [footage, UYA NTSC, takes 14 and 15 against `kitdemo-arena-whip-combo`
  and `-whip-jump-open`, `docs/re-setup.md`, 2026-11-01]: the blade's shape, its size, where
  it leaves and how it flies match, as do the strands; the rope reads as the game's at rest
  and close to it swinging: blue with black dots in a violet haze and a jagged white line
  off it. What differs:
  - The game's black balls in the shock wave are soft and wide and the kit's are small
    dark stars: the same sprite and size, but the kit blends in linear light, where a soft
    edge darkens far less ("Importing them" 3).
  - Its blade's steel grey is lit by the kit's sun and looks lighter.
  - The ribbon and other added colours wash out on the kit's bright floor.
  - **Tonemapping:** Bevy's default `Tonemapping::TonyMcMapface` turns saturated colours
    towards white, so the ring's violet tint (0x40, 0x10, 0xFF) shows periwinkle and the
    rope's pure blue pales. Seen on screen with only the ring drawn; with `None` the ring
    is the game's violet but the kit's floor blows out, and `ReinhardLuminance` gives the
    violet with the floor as it was. It applies to every effect. The demo kit stays on
    Bevy's default; the remaster takes the closest match to the game (PLAN §5, 2026-11-01).

#### Plasma Coil (0x315638, ball class 0xA9B, V1–V4)

No level changes anything drawn.
- **On the gun, each frame it is out** (0x315FA8, no Z write), with a spin s from 0.1 at
  rest rising 0.1 a frame to fire past 1.0, then falling 0.01 a frame [data]: from the
  points of the table at 0x2AC270 [data], a point A at joint 0 + (0, −0.1, 0), a point
  wobbling 0.4 u out that a "ball point" follows on a spring (0x445208), and C, D, E at
  joints 1–3 + (0, −0.28, −0.11), (−0.11, −0.28, 0.03), (0.11, −0.28, 0.03):
  - glows of 44 (0x316F80, half width 2 × rand(0.75 p, p), 0.08 toward the camera, any
    roll, added, alpha 0x7F): at A p 0.11 in (0x20, 0x20, 0x7F) and 0.05 in (0x40, 0x40,
    0xFF); at the ball point p 0.165 s in (0x7F, 0x39, 0) and 0.075 s in (0xFF, 0x72, 0),
    orange; at C, D, E, joints 1–3 and the arcs' three ends p 0.055 and 0.025 in the
    blues;
  - seven arcs a frame (0x317010): A to the ball point, C, D and E to it offset s × 0.45
    sideways, and joints 1–3 to C, D and E; each ten points on a curve bowed by two
    wavering offsets (0.25–0.75 u, ±0.05–0.3 rad a frame), each point jittered 0.04 a
    frame; a ribbon of 43 of half width 0.0125, grey 0x7F added at FIX 0x80, and one of
    14 of half width 0.053, (0, 0, 0xFF) at FIX 0x2A;
  - one frame in 11 at each of C, D and E and at a random point (s above 0.1), a pair of
    0x35 (0x3175C0): s0 = 0.5–0.75 [data], 0.005–0.3 u/s toward the camera ±60°, (0x20,
    0x20, 0x7F) from s0 to 0.1 s0 and (0x40, 0x40, 0xFF) from 0.7 s0 to 0.07 s0, alpha
    0x7F, life 60.
- **The ball in flight** (0x30D680, besides its mesh): four camera-facing billboards at
  any roll, added: 49 of half width 1.25 in (0x80, 0x30, 0x10) and 0.75 in white, alpha
  0x50–0x60; 11 of 1.75 in (0x10, 0x30, 0x80) at 8–0x16 and 1.125 at 0x20–0x40; and 20
  lightning rays from its centre to 0.7–0.9 u along random axes, each picked again after
  20–50 frames (0x30D990): quads of 47 from (0, 0, 0x80) and of 48 from white, fading to
  0 along them, half width 0.625 narrowing to 0.3125, alpha 0.3–0.5 rising and falling
  over each ray's life, u scrolling rand(0.01, 0.075) a frame.
- **Landing:** stuck to an enemy, a burst of 10 (mode 1); to the world, 15 (mode 0);
  after 15 frames five arcs to each of up to three targets (0x30C5D8), reaching them
  over 10 frames and staying 20, bowed by 3.25–6.75 and 1.25–4.75 u: 24 points jittered
  0.075 a frame, ribbons of 43 of half width 0.02–0.1 in grey 0xAF and 0.15–0.3 in (0,
  0, 0xFF) at a third of the alpha, which is 0x40–0x80 and falls in the last 10 frames
  (0x30DEF0); arrived, two billboards of 49 at the end (half width 0.5 in (0x80, 0x30,
  0x10) and 0.3 in white) and a burst of 10 (mode 1) at the target. A ball that wasn't
  on an enemy, or fizzles in the air, ends in a burst of 10 (mode 1).
- **The burst** (0x30E388): n pairs of 0x35 at ±0.15, toward the camera ±60° at 0.05–3
  u/s plus 1–5 up, gravity 7.2, s0 = 0.5–2 [data]; blue (0, 0, 0x80) from s0 and grey
  0x7F from 0.7 s0, alpha 0x7F. Half of them go 1.5 times as fast and live 60 frames,
  ending at 0.35 s0 (blue) and 0.245 s0 (grey) in mode 0 or 0.3 and 0.21 in mode 1; the
  other half are pushed toward the camera and live 40, ending at 2.05 and 1.435 s0 in
  mode 0 or 0.25 and 0.175 in mode 1.

#### Bouncer (0x31C890, canister class 0xE64, bomblets 0xECE, V1–V4)

No level changes anything drawn (V5 is the heavy one, class 0xE9E).
- **At the muzzle** (0x31CDB8, the gun + (0.28, −0.85, 0) [data]): 20 0x17 smokes at
  ±0.3, size 0.333–0.714 growing 1.0–1.01 a frame, drifting 0.001–0.4 u/s any way, grey
  0x0A–0x7F, life 120 fading linearly from 0x7F, added, texture 0; a light (class 0x27F,
  0x300820) of radius 8 for 10 frames whose red and green climb 0.25 a frame from 0 (1.0
  being 0x80) and blue to 1.0 by frame 4 and back to 0 by 8; rumble 0x2AE8C0. The
  landing marker (0x4C3500) each frame of the lob, as the Nitro's.
- **In flight** (the canister's mesh at three times its class's scale [data]): for its
  first 14 frames three 0x17 puffs a frame along the frame's path, still, size 0.134 to
  0.054, jitter 0.025, spin up to ±5, grey 0x80, life 50 fading from 0x7F, added.
  Nothing on bounces or rolling.
- **The burst** (0x31D6C0, disassembled): `explosion_spawn` twice, at S 0.7 and then 1.0
  (both loads seen in the disassembly), with shake (0.075 for 40 frames within 48 u),
  counts 14, 10, 15, 10, 20 glares, no debris, the distortion and the flash, no dome,
  glow or rings, and colour set A ((0xF7, 0x8E, 0x38), (0xF7, 0x63, 0), (0xFF, 0x7F,
  0x40), (0xFF, 0x20, 0), (0xFF, 0x8F, 0), (0xFF, 0x64, 0x30), smoke (0xFF, 0xA0, 0x60));
  then the 17 bomblets, each bursting in `explosion_spawn` at S 0.6 (1.2 heavy), no
  shake, counts 6, 5, 6, 8, 10 glares, no flags, set A (0x31ECC8). Both canister calls
  pass 1 for the refraction and the flash (the 13th and 15th arguments, on the stack
  [disassembly]).
- **Compared with the game** (2026-10-31, `uyantsc-veldin-fxref.sh 11`, PCSX2 at 48%
  speed so the capture holds about every frame, against the kit's `-bouncer-near` drawn
  every tick):
  - The throw yellows the ground more each frame for 9 frames, then cuts off, as the
    light's colour climbs.
  - The canister bursts on the ground among the crates at 7.5 s. The bursts at 9.5 and
    10.7 s follow hits on the robots; the second has a dome, which the Bouncer's lacks.
  - The burst is a low fireball, saturated yellow-white, then orange streaks thrown up
    and smoke. The kit's has the same shape, half hidden by the floor, but is paler on
    its bright floor.
  - In the game the two flashes white out the whole frame for one frame. The kit's add
    in linear light ("Importing them" 3), so a dark sky rises only to about 0.65.
  - The flash shows only when the burst is wholly in view ("The shared explosion"). The
    kit does the same now (`Screen`'s `seen`).
- **Not read:** the Bouncer light's offset (zero in the image, set to length 1.5, so probably
  at the muzzle); the coil's spring 0x445208 beyond its arguments; the render byte +9
  (0x74 on the coil's sparkles); which moby the Bouncer's second burst follows (gp −
  0x1884).

#### Miniturret Glove (0x346078, turret class 0x1366, V1–V4)

Read 2026-10-28 [code unless marked]. The level colours are immediates, switched at V3 by
the row's level byte (under 2: orange and red; from V3 blue, as its ice shots are).
- **The glove:** the landing marker while it aims (0x3462F0, as the Nitro's); nothing as
  it throws.
- **The turret's muzzle** for 6 frames after each shot (+0x44, set by its fire 0x32BF28;
  drawn by 0x32CEB0 from its draw 0x32CAB0), at its barrel (moby point 1):
  - three camera-facing glows of 44 (0x32CAF8, added, a random roll each frame, 0.25 u
    toward the camera) of half width s = rand(0.3, 0.4), s/2 and s/4: (0x3F, 0x18, 8) at
    alpha 0x30–0x40, (0x80, 0x30, 0x10) at 0x40–0x7F and grey 0x7F at 0x40–0x7F; from V3
    (8, 0x18, 0x4F) and (0x10, 0x30, 0x80);
  - two flames of 23 along its aim, rand(2, 4) u long, added: half width w = rand(0.1,
    0.3) at the barrel narrowing to 0.15 w, in (0x80, 0x30, 0x10) at 0x7F, and 2 w to
    0.3 w at 0x3F; from V3 (0x10, 0x30, 0x80).
- **Its shots** (slots of 0x30 at 0x277110), each frame (0x32C980 → 0x32D670): two quads
  of 14 from the shot back 2.1 u along its way, added: a core of half width 0.015 at the
  shot to 0.00375, grey 0x7F fading to nothing, and a glow of 0.0825 to 0.0206 in (0x80,
  0, 0) at 0x7F fading to nothing; from V3 (0x60, 0x60, 0x80).
- **A shot's hit** (0x32C6A0 → 0x32DA38): pairs of 0x35 as the Plasma Coil's burst in its
  mode 0 (the same constants at 0x1DCED0–0x1DCEE8 [data]: 0.05–3 u/s toward the camera
  ±60°, 1–5 up, gravity 7.2, s0 0.5–2), 6 pairs on what takes damage and 3 on anything
  else, each pair a half at random: in (0x80, 0, 0) from V1, (0x60, 0x60, 0x80) from V3,
  and grey 0x7F.
- **Its end** (state 8): sound 3 and the small blast (0x4488B8, below) at S 0.5 with a
  light of radius 13 (weapons.md's "0.5 and 13").
- **Compared with the game** (2026-10-31, `uyantsc-veldin-fxref.sh 12` against the kit's
  arena `-turret` take drawn every tick; `-turret-near` has nothing in reach, so it
  never fires):
  - Both show each hit as a white glow with sparkles for about 20 ticks.
  - The game's tracers are blue streaks with bright heads. The kit's are thin pale
    lines: its `Streak` is one even sprite, so the recipe gives the two quads' average
    width and alpha (0.1 u, 0.5), not their taper and fade from the head.
  - The muzzle's glows and flames can't be made out at either take's 20–25 u.
- **In water** (state 7, landing where 0x4C73B0 finds water): it sinks at a tenth of
  gravity with a 0x22 bubble one frame in five (size 0.05–0.1, colour 0x88A0A0A0), and
  lands with a splash and a ring moby (class 0x307, 0x301508: scale 2, growing 1.025 a
  frame, opacity 0x70).

#### Agents of Doom (0x35C9F0, agent 0x3BD298, V1–V4)

Read 2026-10-28 [code unless marked]; its row's level byte is 0x142617. The agents'
comet (class 0x1893) and small bullet (class 0x1855) are shared with the Suck Cannon and
the Nitro Eruptor, branching on whose they are. The comet's scale is set to 0.05 outright
(0x395480) and its code draws it, so its class's model doesn't show.
- **The glove:** the landing marker while it aims (0x35CDA0); nothing as it throws.
- **An agent's end** (0x3C2140): at V1 the shared explosion at S 0.5 (shake 0, counts 20,
  20, 20, 20, no glares, 2 debris; the distortion where 0x3BD258 says, a dome, the
  ground and camera rings; colour set A [disassembly]); at V2–V4 the comet instead
  (spawned by 0x3BD298), from V5 the growing blast (0x3914C0, not read). When 0x4C7670
  holds (not read) a second explosion follows: S 0.5, shake, counts 3, 3, 3, 6, 3
  glares, a dome.
- **The comet in flight** (0x394068, state 1), each frame at its place, all added and
  fading linearly: four 0x17 puffs (jitter 0.2, size 1.0 × rand(0.9, 1.0) a frame, (0x80,
  0x40, 0x20) at 0x30, 5 frames), three (jitter 0.05, 0.75, (0xFF, 0x80, 0x40) at 0x40, 2
  frames) and two (0.5, white at 0x60, 2 frames); one frame in ten a 0x19 spark (±0.15,
  drifting 3 u/s). It turns after its target at 0.3 u a frame.
- **Where it lands** (state 2): the agents' damage sphere (2 u) and the shared explosion
  at S 0.9 (shake 0, counts 10, 10, 10, 10, 10 glares, 1 debris; the distortion and a
  dome; colour set A [disassembly]); six 0x19 sparks off the hit's normal at 2–6 u/s; then
  for 15 frames three quads of 11 facing its way (0x395E00, added): half widths rand(0.75,
  1.2), 0.75 and 0.5 of that, in (0x80, 0x30, 0x10) at 0x3F–0x5F, 0x4F–0x6F and 0x5F–0x7F,
  and a 0x6C particle each frame (0x46F288, not read).
- **The small bullet** (0x35D9C0, its draw 0x35E1D0), the agents' gun at V2–V4:
  - two camera-facing glows of 57 (texture at 0x1DD72C [data]) of half width 0.18
    (0x1DD73C), grey 0x7F at 0x20 (0x1DD730), turning opposite ways by a degree a frame
    (0x1DD734), and one of 47 of 0.05 in grey at 0x50 at a random roll, all added;
  - once it has flown 0.5 u (0x1DD740), a quad of 63 along its way (0x35DFF0): from 1.25
    u behind it, rand(1.333, 1.5) long, half width rand(0.14, 0.22), white at 0x7F, added;
  - every 0.1 u of its path a 0x2C ember in 0x17's texture (0x35E2F8): size 0.3 growing
    10000 / 210000 a frame, 3–6 frames, alpha 0x60 blended or 0x40 added, by the
    Agents' level: (0xAF, 0xAF, 0x40) or (0x7F, 0x7F, 0x40) at V2, (0xFF, 0x80, 0x40) at
    V3, (0xFF, 0x40, 0x40) or (0xFF, 0x7F, 0x40) from V4 (V1's (0xFF, 0x80, 0x40));
  - where it hits: under V3 the shared explosion at S 0.2 (counts 5, 2, 2, 2, 2 glares,
    no flags, colour set A [disassembly]), from V3 twenty 0x2C embers (0x35DDD0: ±0.1,
    size 0.05–0.2 growing 500 / 210000 a frame, 0.5–1 u/s within 80° round a bearing and
    0–80° up, 30 frames, grey 0x7F fading from 0x7F, spin ±1–2).
- **Its gun:** no flash where its bullet leaves (0x3C0128), only a sound [code, 2026-10-31].
- **Its jets** (V4 and up; 0x3C1D38, while it hovers and while it lands): each frame three
  type 0x4A particles (0x46A9C8, 0x4A's texture, added) at its point 0 (joint 54, 0.11 u up)
  ±0.025, falling rand(30, 40) / 600 u a frame and keeping 0.98 of it, carried with it. From
  the templates at 0x1DEC08, 0x1DEC18 and 0x1DEC28 [data]: blue at 0xCF fading to (0, 0,
  0xCF) over 5 frames, half 30000 → 40000 (/ 210000); yellow (0xFF, 0xFF, 0) at 0x60 to
  (0x80, 0, 0) over 6, the same size; (0x4F, 0x7F, 0xFF) at 0xEF to blue over 4, half 10000
  → 30000.
- **Hovering, it faces** [code, verified on UYA NTSC]: with no target, its slot while it is
  0.1 u or more off it (0x445758, 0.0087 rad a frame², at most 0.524), else where its spring
  takes it; with one, its target. In `uyantsc-veldin-agents-v5-hover.p2s` the four stood on
  their slots at the class's own scale (0.0983) and faced 8–14° off the hero's way.
- **An agent appearing** (0x3BD298, state 0x10): 0x35 sparkles of rand(0.35, 0.6) to
  0.001, rising, 20–40 frames, in blue (R rand & 0x2F, G 0x10–0x4F, B 0x7F) and one of
  type 0x68's four textures, added, as its opacity climbs 4 a frame.
  - **The Agents of Dread's lightning** [code, 2026-11-05; seen at V5]: the state is
    only from V4, for an agent spawned onto a free slot round him (the spawn, 0x3C0F78,
    sets it alpha 0 and a 30-frame timer at +0x33C); agents out of the pod (V1–V3) or
    on foot start following. Each of its 30 frames it sheds one sparkle at its place ±0.3
    with alpha byte 0xFF (nearly 2), and type 0x68's four textures (the list at
    0x2212B8: particle textures 53–56) are lightning crackles; its alpha rises 4 and mode
    bit 0x200 (libuya's `DRAW_TRANSPARENT_WEIRD`, drawn added) is set, its yaw turning
    rand(100, 1440)°/s; then alpha 0x80, the bit cleared. So each appears as a glowing
    white-gold silhouette of itself wrapped in violet crackles, as the game's V5 take
    shows (`uyantsc-veldin-fxreview.sh agents 0x5B`, 3.5–5 s). Nothing else in the agent,
    the glove (0x35C9F0) or the spawn draws at V5.
  - **Its end** (the same update, past the switch) [code, 2026-11-05]: through the last 30
    frames of its 2000-frame life (+0x144, counted down after the switch), a sparkle a
    frame of the same kind at rand(0.5, 0.75), one in four normal-blended (render byte
    0x44, else 0x48), as its alpha falls 4 a frame from 0x80, mode bit 0x200 set, and its
    yaw turns rand(100, 1440)°/s as it does appearing; on the last, `agent_end`.
- **The pod's burst** (`agent_pod_open` 0x35D738) [code, 2026-11-05; seen at V5]: the pod
  (0x35CEC8) spawns its four agents in state 2, then opens in state 1: unless it splashed
  (+0x24, set on materials 0, 1 and 0xB), `explosion_spawn` at its place at S 0.75 with
  every count 0 but 30 smoke, no glares, debris, dome, flash, glow or rings, no shake, and
  damage 0 within 2 u (f15 2.0 is the damage ball's radius, +0x68; f14, its damage at
  +0x6C, is 0). Its colours are set A with red and blue swapped: (0x38, 0x8E, 0xF7), (0,
  0x63, 0xF7), (0x40, 0x7F, 0xFF), (0, 0x20, 0xFF), (0, 0x8F, 0xFF), (0x30, 0x64, 0xFF), smoke
  (0x60, 0xA0, 0xFF), dome (0, 0, 0xFF) at 0x28, ring (0x30, 0x64, 0xFF). So: six 0x5C
  glows of 3S growing to 4.5S over 30 frames, then on the 3rd frame 30 blue puffs of 0x17
  (1.5S, life 40-70, alpha 0x40, blended). It is the white-blue cloud in the game's V5 take
  (`uyantsc-veldin-fxreview.sh agents 0x5B`, 12.1-13.1 s), from which the on-foot agents
  walk: from V4 the glove (0x35C9F0) sends agents straight to the hero only while a slot
  is free (`agent_slot_free` 0x3C15A8), and with none free throws the pod as below V4.
- **In the kit** (2026-11-05):
  - The pod opening reports an impact, and its recipe is the burst ("pod blast").
  - From V4 the glove sends agents to his free slots, else throws the pod, as the game
    does.
  - An agent on a slot stays where it came out for its first 30 ticks, spinning by a turn
    drawn each tick, and spins the same through its last 30. Both are in the simulation
    (`thrown.rs`, `agent`).
  - Its model is added as its alpha climbs, then again as it falls (`guns.rs`,
    `painted`).
  - The sparkles: from V4 at alpha 0xFF as it appears on a slot, and at every level
    through its last 30 ticks.
  - Seen at V5 on `kitdemo-arena-agents-near`: the second press's pod bursts blue and
    its agents walk out of it, as in the game.
  - Not done: an agent the pod lets out from V4 while a slot has come free walks
    instead of taking the slot (`agent_spawn` checks again).
- **Its barrels' arms at V5** [code, verified on UYA NTSC, 2026-11-05]: from V5,
  `agent_tick` (0x3C0128), which the agent's update runs first every frame, scales
  joints 3 and 8 to 0.45. Those are the roots of the arms that carry V5's barrels
  (bangle 2; the class's points 4 and 5). It does so through two joint layers in its
  pvars (+0x200, +0x280), handed each frame to `joint_layer_drive` (0x44B5C8).
  - So the barrels show at 0.45 of their modelled size, beside its head rather than
    above it. Hovering, the agent stands about 0.7 u tall and 0.58 u across, not 1.1 by
    1.14.
  - Walking (sequence 2) or running (3), the same layers also pitch both arms, to −80°
    or −55° less the agent's own pitch (springs 0.01, 0.02).
  - At V2–V4 one layer, on point 9, pitches its gun the same way, unscaled.
  - **How it was found:** the kit's agents showed about twice the game's size against
    the hero. In `uyantsc-veldin-agents-v5-hover.p2s` the four agents matched the kit
    in every other way: sequence 9, the class's scale (0.09826), the kit's bangles
    (+0x7C = 5), unscaled matrices, and the class's bounding sphere (moby +0x80, class
    units). Each also had the two layers at moby +0x54, each scaling by 0.45.
  - **How it was checked:** the savestate's own screenshot (PCSX2 keeps one in the
    `.p2s`) shows the game's agents 0.62–0.67 u tall and about 0.63 u across. That was
    measured through a projection fitted to their known places (fx 522 px, fy 696 px,
    centre (320, 251) on its 640 × 480; the 16:9 view at 4:3). Drawn into that
    screenshot with the arms at 0.45, the kit's agents lie on the game's; unscaled,
    their barrels stand twice as large and high.
  - **In the kit:** the V5 row in `openrac/src/shot_models.rs` names the joints and the
    factor, and `rac_demo` applies them over the pose (`ShotSkeleton::scaled`). The
    arms' pitch while walking isn't modelled.
- **Compared with the game** (2026-10-31, `uyantsc-veldin-fxref.sh 8` against the kit's
  `-agents-near` drawn every tick, V3): both show the cyan landing ring, the throw and
  agents appearing at his hand in blue sparkles, which in the game add up to a brighter
  white-blue burst; the game's agents run out ahead to the enemies and blow up in the
  shared explosion, the kit's run off to the demo's enemies out of view. The sparkles now
  flash as 0x35s; how many the game sheds a frame isn't read (the kit sheds one a tick).

#### The small blast (0x4488B8)

Shared by the turrets' ends, the small missiles (0x343330), and the callers 0x311140 and
0x390EE0 [code]: for a size S,
- three fireballs of particle type 0x0B (spawner 0x461128) of size 1.905 S (400000 S /
  210000), thrown any way at rand(8, 10) S u/s, living 15–20 frames, in a start colour
  and an end colour picked from the tables at 0x1D7BF8 and 0x1D7C10 [data]: (0xFF, 0x8F,
  0) twice, (0xFF, 0x7F, 0), (0xFF, 0x6F, 0) at alpha 0x4F and white at 0x2F twice;
  ending (0x7F, 0x5F, 0), (0x7F, 0x4F, 0), (0x7F, 0x3F, 0), (0x4F, 0, 0) at 0x2F and black
  at 0x2F and 0x3F; one in five blended, the rest added; drawn in type 0x0B's texture
  (Veldin's list starts at particle texture 0, the dark smoke);
- with a moby given, two domes (0x2F95A0, class 0x70) of 4 S and 3 S, and its sound;
- with a radius given (13 if not positive), a light (0x300650, the template at
  0x2E9D20).
- **Type 0x0B's update** (0x4613A8): it keeps 0.8 of its velocity a frame (0.6 with flag
  2), its size following a curve over its life that shrinks within 10 u of the camera
  (× 0.25 + 0.075 × distance), its colour lerped from start to end. At 85% of its life,
  in its first two generations, it spawns four children of its colour and size from
  where it is, at 0.75 of its first speed × rand(0.85, 1.0) and rand(0.9, 1.1) of its life,
  so three become 63. Its life over, it turns to smoke for two to three times as long:
  rising (0.00083 u a frame², about 3 u/s²), keeping 0.98 of its velocity, fading to
  (0x3F, 0x3F, 0x2F) at 0x0F if blended or to nothing if added.

#### Holo Shield Glove (0x3717A8, shield 0x376B88, V1–V4) [code, 2026-10-28]

- **In flight** (state 0): a light (0x421F08: radius 5, colour (0.125, 0.25, 0.5))
  follows it, and its draw (0x37B108) lays six added billboards on it, pushed 0.08
  toward the camera, their rolls from a phase that turns 1° a frame: 57 of half width
  0.5 in grey 0x50 at alpha 0x40, turning 2.5 times the phase; 42 and 44 of 0.5 in
  (0x40, 0x20, 0x7F) at 0x20, turning −1.5 and 1 times it; 49 of 0.36 in (0, 0x7F, 0xFF)
  at 0x20, half the phase; 49 again with colour word 0x0020FF7F, whose alpha is 0, so it
  doesn't show; and 47 of 0.11 in grey 0x7F at 0x50 at a new roll each frame.
- **Unfolding** (state 2, from where it lands): its width grows 0.06 a frame to 2 and its
  height 0.04 to 1.2 (about 34 frames) as it rises 0.032 u a frame along the level's up;
  each frame four 0x61 within ±0.15 across of its place, of half size 1 shrinking to 0.4,
  3 u/s level at any bearing, falling 0.00167 u a frame², life 10–30, (0x20, 0x40, 0x80),
  alpha 0x7F to 0, spin −2 to −4, in type 0x17's texture (the dark smoke), half
  normal-blended (0x44) and half added (0x48); the light again, and its draw (0x378A20 at
  V1–V2, 0x378298 from V3) with 0x37B2C8's 0x2C and 0x0F.
- **Later states:** 3 standing, 4 hit (from V3 a bolt, below, in (0x20, 0x40, 0x7F); the
  Ultrashield's pulses 0x372338), 5 folding (draws 0x37A5C0 and 0x379CE0, not read).
  State 4 lasts 30 frames at every level and is drawn by 0x379780 at V1–V2 (not read) and
  0x378FF8 from V3 (below, "As a hit shakes it").
- **The wall** (0x378A20 at V1–V2, 0x378298 from V3, queued each frame from state 2 on)
  [code, 2026-10-28] is quads in the shield's own plane: the unit square's corners (0,
  ±1, ±1) (0x1D7C50 [data]) times a size, turned about its facing axis and then taken
  through the shield's matrix, whose axes are its width (2 once unfolded) and height
  (1.2), so each sprite turns inside a level oval or rectangle 4 by 2.4 (0x40C960
  multiplies the turn first); some pushed 0.01–0.1 along the facing. Two kinds:
  `0x37ABF8` one textured quad, added or normal-blended; `0x37AD80` a masked strip, which
  first writes a texture's alpha into the frame's alpha alone (blend 0x6A, alpha test
  NEVER with AFAIL FB_ONLY through register 0x47) and then adds a second texture, its
  UVs turned and scaled about their middle, weighted by that alpha (0x58; 0x54 blended).
  Six phases turn by random steps each frame (V1–V2: −0.5 to −1°, −1 to −2°, −1 to
  −1.5°, 1.5 to 2.5°, −0.5 to −1.5°, 0.5 to 1.5°; from V3 the fourth and sixth turn the
  other way), each shared by several layers. Layers fade out as the camera comes within
  a squared distance across of 13 u², in fully by 25 or 38 (0x1DDB54–0x1DDB60 [data]);
  each has a flag (0x1DDB68–0x1DDB94, all 1 on Veldin [data]).
  - V1–V2: a strip of 72 through 71's mask in (0x32, 0, 0xFF) at 0x8F, its UVs scaled
    1/1.7; rings of 27 at 1.15 in (0x20, 0x40, 0x80) at 0x2F; glows of 76 at 0.5 in
    (0x32, 0, 0xFF) and 0.2 in (5, 5, 0x30); strips of 72 through 71 0.05 out in (0x20,
    0x40, 0x80) and 0.1 out in (0x7F, 0x20, 0x20) at 0x8F; the swirl 75 at 0.6 in (0x80,
    0x40, 0x20), normal-blended.
  - From V3: two camera-facing stars of 57 at its middle (`fx_billboard`, half 0.2,
    (0x20, 0x40, 0xFF) at 0x4F, added), rolled by two of the phases, the second the other
    way, faded by the second near pair and skipped below a quarter of it [code,
    2026-11-05]; glows
    of 11 at 0.225 in (0x40, 0x20, 0xFF) at 0x3F and at 1.0 in (0x10, 0x10, 0x7F) at
    0x15; rings of 27 at 1.15 in (0x80, 0x40, 0x20) at 0x4F, and of 8 at 1.1 in (0x10,
    0x10, 0xFF) at 0x1F; at V5 a quad of texture 5 (0x1DDB64 [data]: a soft disc with
    banded shading, like a lit sphere) at 1.15 in (0x80, 0x40, 0x20) at 0x5F, added,
    turning with the rings of 8, unfaded; and five strips of
    the cloud 67 through itself: grey 0x40, then (0x40, 0x40, 0x60) 0.02 either side and
    (0x7F, 0x40, 0x80) 0.04 either side, all at 0x2F.
  - Textures (`openrac-tool fx-textures`, Veldin): 8 a ring, 11 a soft glow, 27 a thin
    blue ring, 57 a fiery wisp, 67 a smoky cloud, 71 a starburst, 72 a fibrous disc, 75 a
    swirl, 76 a soft ring.
- **As a hit shakes it** (0x378FF8, from V3) [code, 2026-11-04]: the standing draw's
  layers, but the ring of 8 at 1.1 is (0x80, 0x40, 0x20) at 0x4F, unfaded, in place of the
  blue one at 0x1F; and another ring of 8 in (0x80, 0x40, 0x20) at 0x25 grows from 0.05 by
  0.05 a frame to 1.1 (+0x40, set to 0 as the reaction ends) and then isn't drawn.
- **The Ultrashield's "bubble"** [code, 2026-11-04]: none of its draws (0x378298, 0x378FF8,
  0x379CE0, and the quads 0x37ABF8 and 0x37AD80 they call) takes the copy of the frame
  (texture −1): nothing of it warps what is behind it. What reads as a pink-white bubble
  in the game's take (`review-holo-0x6B`) is V5's disc of 5 over V3's layers, the orange
  rings of a hit, several shields standing together, all added, and the shield's own
  model (below). Classes are found by a byte index at 0x1FDC50 + class into pointers at
  0x1FD200 (`moby_class_register`, 0x424368).
- **Its own model** (class 0x186D) [code, 2026-11-05]: drawn at every level, a lens-shaped
  disc 0.7 across and 0.12 thick at its class's scale, upright in the class's y and z,
  its 25 joints still through its one sequence (`openrac-tool moby` at 0x15D9AC0 in the
  Veldin image). The spawn (0x376960) sets its scale to 3.25 times its class's (+0x2C),
  its alpha to 0x14 (+0x23), and its matrix to be drawn as set (+0x34 bit 0x100): the
  hero's, with its across and up rows 0.05 long. Landing and each frame of unfolding set
  those rows to the wall's width and height (0x376B88), so it stands as an oval 4.55
  across, 2.73 high and 0.39 thick, 1.14 times the wall's size-1 layers; in flight it is
  a sliver 0.11 across. The width only enters the scale for the bounding sphere
  (`moby_update_bounds`, 0x439610, takes the matrix as it is), not the draw. Folding, its
  alpha is 0x15 less the frame, gone by the 20th (0x379CE0). Through its last 4 s
  standing (from frame 661) its colour (+0x3C, `moby_set_colour` 0x438908) swings
  between grey 0x80 and (0x80, 0x30, 0x10) on a sine turning 4° a frame. The game's
  `holo-fold` take shows it as a tint, 0x80 for 1: measured on the shell's rim over the
  sand, its green and blue fall to about 0.75 and 0.6 of its red and back, every 4.0 s
  of the take (90 frames at its 38% speed), red holding [verified on UYA NTSC,
  2026-11-05]. The kit tints the shell so (`guns.rs`, `painted`), measured the same way
  on `kitdemo-arena-holo-fold`: green and blue to 0.69 and 0.51 of red, every 90 ticks,
  through the last 240 before its fold. So the V4 duck's
  (0x40, 0x10, 0x10) (0x39C2F8), drawn as a red glow, may be a tint darkening it. In the game's take (`uyantsc-veldin-fxref.sh`
  10 at 5.6 s) it is a faint glassy shell round the wall, wider than its layers, with a
  dark grey rim: the rim the comparison below couldn't place.
- **Folding** (0x379CE0 from V3, 0x37A5C0 at V1–V2) [code, 2026-11-05; seen at V3 and
  V5]: the standing layers at their full size, each alpha the fold's frame t (1 to 20)
  takes 2 a frame from, mostly its own standing alpha: from V3 the blue ring of 8 and
  the last pair of strips of 67 from 0x3F (standing 0x1F and 0x2F), V5's disc from 0x4F
  by 3 a frame; the camera's nearness only decides whether a layer is drawn, not its
  alpha; the stars at half 0.1875. It fades rather than shrinking. The colour is a word
  `(alpha − 2t) << 24 | rgb` passed unclamped to the quad's corners (0x37ABF8), so the
  faint glow of 11 at 0x15 falls past 0 at its 11th frame and wraps to 0xFF, nearly 2:
  a blue glow the wall's size, added at twice its colour, through the last 10 frames.
  The game's takes (`uyantsc-veldin-fxreview.sh holo-fold` at rows 0x69 and 0x6B) show
  just that: the shield fading at its full size, turning bright blue for about its last
  10 frames (blue-violet over V5's pink), then gone at once. The model's alpha is 0x15
  less the frame.
  - In the kit (2026-11-05): panels keep their size and fade by 2 of 0x80 a tick from
    their own alpha, or from a `fold` of their own, as a byte that wraps
    (`fx/panel.rs`, `Panel::folding_alpha`); the model fades evenly to nothing
    (`guns.rs`, `FoldFade`). Seen at V3 on `kitdemo-arena-holo-fold`: the same fade, blue
    flash and end. The stars take their smaller size as it folds (`fold_size`).
- **Compared with the game** (2026-10-31, `uyantsc-veldin-fxref.sh 10` against the kit's
  `-holo-near` drawn every tick, V3, three shields stacked): both an oval of turning
  purple-blue layers. The game's is rounder on screen (about 1.3 wide for 1 high, from
  behind him), deeper in colour with a white middle, and has a dark grey rim; the kit's
  is flatter (about 1.8 to 1), paler on the demo's floor, with a soft edge. Not worked
  out: why the game's is rounder than the 4 by 2.4 its matrix gives (since mostly
  explained, below), and which layers make its dark rim (since found: the shield's own
  model, above).
- **Its shape, checked** (2026-10-31):
  - **The corners** at 0x1D7C50 are (0, ±1, ±1) [read from the Veldin memory image].
  - **The order:** both kinds of quad (0x37ABF8, 0x37AD80) copy the moby's matrix and
    multiply the turn in first (0x40C960(out, A, B) gives B·A, rows as axes, and
    0x40C2F8 takes the rows as axes), so each layer turns and is then stretched [code].
  - **The axes:** row 1 is set to the width's length (pvar +0x18, 2) and row 2 to the
    height's (+0x1C, 1.2) [code]. The rotation is the hero's, copied at the spawn
    (0x376960, from the moby at 0x1A71A0 + 0xF0) [code].
  - **Facing the camera:** a branch turns the wall about the level's up to face the
    camera, but its flag at 0x1D673C is 0 on Veldin [memory image].
  - So every layer lies inside a 4 by 2.4 oval in the hero's facing plane, as the kit
    draws it (4 by 2.3).
- **On screen** (2026-10-31): the first wall alone (take 10 at 5.6 and 6.2 s), measured
  across its dark rim, is about 305 by 220 px (1.39), and the stacked walls at 11 s are
  the same. Stacking doesn't explain it, and seen from above a 1.67 oval looks flatter,
  not rounder. Mostly explained (2026-11-03): the takes show the game's 16:9 view at
  4:3, squeezed across to 0.75 (`re-setup.md`, "Gotchas"), so the wall is really about
  1.85 wide to 1 high on screen, near the code's 1.67; perspective may make up the rest.

#### The lightning bolt (0x34DAA0, class 0x1195) [code, 2026-10-28]

- **Made** (0x34DAA0, 0x34DF10): life 15 frames (0x1DD48C [data]), a strike timer of 6;
  colour (0x20, 0x40, 0x7F) (0x1DD43C [data]), or (0x30, 0x05, 0x80) when the Rift Inducer
  makes it (0x800530); from the hero's point 0.75 up, from the owner's moby point for the
  shield (0x186D) and the Hydra's 0x184E, or from the rift; a target position, or the
  target moby's.
- **Each frame** (0x34DD38 → 0x34DFD0) two lines of points in a frame along the bolt
  (across it level, then up square to both), d/10 apart, where d grows from 0.01 to the
  whole distance as the strike timer runs down: ten points, the ends on the line and the
  others lifted 0.1 × sin(a + 36° each) + rand(±0.2), a turning 10° a frame; and 19 more
  (and a 20th left at the start), lifted 0.75 × sin(b + 12° each) × sin(c + 57° each) +
  rand(±0.2) and 0.5 × cos(c + 57° each) across, b turning 3° and c −46° a frame. So the
  first reaches 0.9 of the way and the second 1.8 of it, then back to the start.
- **Its draw** (0x34E5E0 → 0x34E688): each line as two added ribbons (blend 0x48): a core
  of 23 of half width 0.025 in grey 0x7F at alpha 0x80, and a glow of 14 of half width
  0.25 in the bolt's colour at 0x40, the first and last segments fading in from and out
  to alpha 0 at their ends, u scrolling 0.2 a frame.
- **The strike** (timer at 0, the 6th frame): `damage_queue` on the target, the gun's
  damage table entry (half for the Hydra, a tenth for the rift); from then to its end each
  frame at the target five pairs of 0x35 (start half size s = 0.4–0.8 ending at 0.3 s in
  the bolt's colour at 0x7F, and 0.5 s ending at 0.15 s in grey 0x7F at 0x30; 1–6 u/s
  any way, spins ±1–4 opposed, life 15) and one still 0x35 of 0.15–3 ending at a tenth,
  life 12, spin ±1–8.
- **The Tesla Barrier's** `rand(20, 60)` (0x3410E8) is each of its four slots' wait
  before its next bolt, not a bolt's life.

#### Spitting Hydra (0x31FB90) [code, 2026-10-29]

Read in Ghidra on the Veldin image; data values from it [data]. Its marked targets take
the lightning bolt (above); the spit is drawn by the gun itself.
- **The spit's arcs** (`weapons.md`, "Spitting Hydra"): one per most targets (3–7, at
  0x1DC980). Each has an end (0x276AB0, 32 u along the aim jittered ±0.2, 0x321108) and a
  point it bows from (0x276A40): 75 u straight down (0x1DC984) turned round the aim, set
  to 0.2 of its length (0.35 from V5; a random 0.05–0.1 if targets are marked) and added
  to the gun. The turn is applied twice, so the arcs lie at −107°, 0°, 107°… at V1–V2,
  −107.5° by 52.5° at V3–V4 and −90° by 30° from V5, 0° being below the gun. Each arc
  first shows 2 × its index + rand(0, 7) frames after the spit (timers at 0x1E1858).
- **Their points** (0x321210, on two frames in three): 16 per arc on the cubic of
  0x4420D0, p(t) = b t³ + (a − b) t² + (end − bow point) t + gun, with a = bow point −
  gun and b = beyond − end − a, beyond being 30 u past the end along the way (0x1DC988):
  so it leaves the gun heading away from its bow point and comes into its end along the
  aim. t steps 1/16 to the 15th point; the 16th is the end. Each point is then moved up
  to ±0.2 on each axis (0x1DC9B8).
- **Their draw** (callback 0x321E98 → 0x3220F8, a strip as the whip's): through the 16
  points, a core of 43 of half width rand(0.05, 0.06) in grey 0x7F added at FIX 0x4F
  and a glow rand(3, 5) times as wide in (0x20, 0x20, 0xFF) at a third of it, joined,
  narrowing by 0x1DC99C (1.0, so not at all) a point. Where a piece's line meets a wall
  (`coll_line`) the arc stops there, with a 0x17 smoke and a 0x2C ember as at a strike
  (below; from V5 also a blast of 2 u).
- **At the muzzle** as it spits: two billboards of 61 at the gun, half size s and 2s, s
  from 0.01 growing 0.35 a frame, in (0x32, 0x20, 0xC8) at alpha 0x5A losing 6 a frame
  (15 frames), added, pushed 0.08 toward the camera; and a lightning bolt in (0x20, 0x40,
  0x80) at `gun_assist`'s pick (0x44D930: the lock-on target, else the best within 10 u,
  far 20° by 55°, near 45° by 35°), else to a point 10 u along his facing (the camera's in
  the views that aim) and 1.5 up from his feet [code, 2026-11-03]. The rings are
  `fx_billboard` (0x4082B0, facing the camera) and only the spit's callback (0x321E98)
  draws them, so a release at marks shows none. While its timer is 5 or more
  (0x322A80): a light (radius 4 × 0.7), each frame with even odds a 0x2C of half 4.2 in
  (0x20, 0x40, 0x90) at alpha 6 in 0x5C's entry 1 and another of half 1.68 in (0x20,
  0x40, 0x80) at 0x7F in 0x5C's entry 0 jittered ±0.04, half added and half blended
  (both growing 500 / 210000 a frame, life 15), and a white 0x6F ray ("The glare").
- **The strike** (0x321580, when the timer reads 27): for each arc within 60 u whose
  line from the gun to its end meets something, the damage and blast (`weapons.md`),
  and where the line met it 20 times: a 0x17 smoke (half 0.3–0.75 growing ×1–1.01 a
  frame, ±0.2 about the point, rising 0.2–3 u/s, (0x20, 0x20, 0x20), blended, life 90,
  its alpha from 0x30 (+0x2A, read as its fade's start)) and a 0x2C ember (half
  0.25–0.7 growing 500 / 210000 a frame, ±0.1, 1–2 u/s within 80° either side of the
  camera's bearing and 0–80° up, life 30, (0x40, 0x80, 0xFF) at 0x7F, in 0x61's entry 6).
- **While held** (callback 0x321CB8, points made in 0x31FB90; 2026-10-29): the charge c
  is 0.007 a frame held, at most 0.7 (the count, 0x1E1830, stops at 150).
  - Past 4 frames, on odd frames, two strips of 20 points (0x276B20) on the curve from
    the muzzle (point 0) to the gun's point 1 (0x437698), t stepping 0.05 with the last
    at 1: the bow point is the hero's up turned a random −180°…180° round the aim, times
    rand(1, 4) + c, from the muzzle; the point beyond is the aim times c − rand(1, 4)
    from the muzzle (behind it). Each point moved ±0.05 on each axis. So each loops out
    from the muzzle by about 4/27 of its bow (0.15–0.7 u) and back into the gun. Drawn
    with core half 0.025–0.03 and glow 1.5–2.5 times, FIX half the frames held (the
    glow a third), texture 43 as the spit's.
  - From V3, past 19 frames, every frame, two of 10 (0x276DA0): each keeps an end for a
    timer of rand(15, 25) frames (0x1E1878); when it runs out (not in hero states 7,
    0xB, 0xE or 0x14, which keep it) a line 3 u from the muzzle a random way
    (0x440B30) picks a new end where it meets the world, else the floor 3 u from his
    feet along his facing turned ±70° (0x1A4C60 + facing × 3), and that frame's points
    aren't remade. The bow point as above; the point beyond is the way to the end times
    rand(1, 4) + c from the muzzle; points t 0.1 apart, ±0.04. Core half 0.015–0.02,
    glow 1.2–1.8; where a piece meets a wall two sparkles of 57 and a glow of 11.
  - Each frame 0x322A80 at c: the light (radius 4c), the glows of 0x322C20 sized by c,
    and from c 0.45 (65 frames: 64 × 0.007 falls just short) the glare (0x322B80).
- **The glare** (0x322B80, each frame 0x322A80 runs at 0.45 or more: through a hold from
  65 frames, and at 0.7 while the spit's timer is 5 or more) [code, 2026-10-29]: a point
  a random 0.025–0.15 u from the muzzle any way round (0x440B30), and a particle of type
  0x6F there (spawner 0x46FE70 at scale 1, life 15) moving 0.0125 u a frame back toward
  the muzzle. 0x46FE70: colour 0x64FFFFFF (white at 0x64; 0x64FF2020, blue, at any scale
  but 1), copied to +0xC as a second colour, added (+3 0x48), its type's first texture
  (25 on Veldin), half width scale × 0.013 in the position's w (as 0x5A keeps 0.05
  there), the scale at +0x3C. Its update (0x46FF70, the table's entry 0x6F): the head
  steps by its velocity and the tail is laid scale × 1.35 behind it along that step, so
  it is a ray from near the muzzle 1.35 u straight out; once fewer frames are left than
  +8 (its life less 15, so never here) its alpha falls in step.
  - The Hydra's points [data, gadget WAD on UYA NTSC-U]: class 0x184E (V1–V4) has point
    0 at joint 8 and point 1 at joint 11, at rest (0.099, −0.822, 0) and (0.264, −0.101,
    0) from its origin (scale 0.0405); class 0x1B4F (V5) joints 4 and 6, (0.015, −0.854,
    0) and (0.145, −0.274, 0) (scale 0.0576). So point 1 is near the grip, 0.72 u (V5
    0.58) back along the barrel from the muzzle and 0.165 (0.13) off its line.

#### Disc Blade Gun (0x36EFC0, disc class 0x1867, update 0x36FA78) [code, 2026-10-28]

The disc's level field (+0x2D4) is the gun's level byte, V1 being 0; "V3–V4" below are
its 2 and 3.
- **The disc itself:** its moby's scale starts at 0.019 and grows 0.001 a frame to
  0.0225 at V1 or 0.035 from V2; its x and y axes turn 60° a frame about its z, its
  axle (0x40CCA0), and all three 2° a frame more, either way as picked when it is made
  (0x4475E8, whose axis isn't read). Whether 0.035 is the class's own scale isn't
  checked; the kit draws the class at its own.
- **On it, to V4** (0x371240, queued while it is within 96 units of the view): added
  billboards pushed 0.08 toward the camera, their rolls from a phase that starts at
  random and turns 1° a frame (0x1DD9FC [data]): 57 (0x1DD9E8) of half width 0.5
  (0x1DDA04) three times, turning −1.5, 0.5 and 2.5 times the phase, in grey 0x7F at
  0x20 (V3–V4 (0x20, 0x20, 0xFF)); 49 of 0.36 twice, ±0.5 times it, in (0xFF, 0x7F, 0)
  at 0x10 (V3–V4 (0, 0x7F, 0xFF)); 47 of 0.11 (0x1DDA00) in grey 0x7F at 0x50 at a new
  roll each frame. Nothing from V5.
- **Behind it, to V4** (0x371430, the same frames): a 0x2C particle every 0.1 unit of
  the frame's way in type 0x17's texture (0x221174: the dark smoke), half 0.3 growing
  10000 / 210000 a frame, life 3–6, rising 0.1–0.2 u/s, spinning ±1–3, at random either
  (0xFF, 0x7F, 0x40) at 0x40 added or (0xFF, 0x40, 0x40) at 0x60 normal-blended (V3–V4
  (0x40, 0x7F, 0xFF) and (0x40, 0x40, 0xFF)); and every other frame on average a 0x0F
  puff in type 0x35's texture (0x2211EC), half 0.45, life 10–20, from (0xFF, 0x7F, 0x40)
  or (0xFF, 0xDF, 0x80) at 0x7F to (0xFF, 0, 0) at 0 (V3–V4 (0x40, 0x7F, 0xFF) or (0x80,
  0xDF, 0xFF) to blue), going back at 1/30 of the disc's speed, ±0.1 u a frame on each
  axis and 0.05 up, gravity 0.00167 u a frame² — the Agents of Doom's bullets' trail
  again ("Agents of Doom").
- **Its trail, from V2** (0x371128, drawn by 0x4068C8): a ring of the disc's places,
  six kept every 6 frames to V4 and four every 4 from V5, the newest the disc's own
  place each frame; at each place the two points ±0.5 along the disc's x axis as it was
  at its first draw (its moby's rotation at +0xF0 copied once to +0x290), which lies
  level about 60° from its first heading; quads between them added (blend 0x48 from
  0x1D63D4 and 0x1D63D8 [data]) with u fixed at 0.5 and v 0 to 1 across (0x1D63E0
  [data]), in texture 29 to V3 and 60 from V4 (the call's stack argument). Colours lerp
  (0x40D020) from the head's to the tail's, the kth kept place at t = (k − 1 + f) /
  (n − 1) for f the share of the frames to the next kept one and n the places, so the
  oldest fades out as it goes, and the disc's own at 1 / (n − 1): V2 (0xFF, 0x80, 0x40)
  at 0x6F to (0xFF, 0x20, 0) at 0; V3 (0x40, 0x80, 0xFF) at 0x6F to (0, 0x20, 0xFF); V4
  (0x20, 0x10, 0xFF) at 0x4F to (0, 0, 0xFF); V5 (0xFF, 0x10, 0x20) at 0x4F to (0xFF,
  0, 0).
- **Glancing off something** (0x370CD8, at the line's hit point, each world bounce and,
  from V3, each enemy): to V2, 15 0x5A glares at the disc toward the camera at 0.1–0.2 u
  a frame, each turned more aside than the last (±0.05 a frame on each axis added to
  the running velocity), life 25–40; from V3, the Flux Rifle's muzzle again ("Flux
  Rifle"): ten 0x74 streaks (0.1, 0.25, 0.9 long, 0.5) at 5–10 u/s any way, life 15–25,
  from (0x20, 0x40, 0x80) at 0x80 to blue at 0; then 0.4 unit toward the camera four
  0x35 sparkles (from half 0.1 to 3.5 and 2.5 in (0x20, 0x40, 0x80) and (0x20, 0x40,
  0x60) at 0xBF, 1.75 and 1.25 in white at 0xC0 and 0xB0; life 14) and three 0x5C (0.7
  to 0.9 and to 1.7 at 0x46, 1.5 to 2.0 at 0x2D, life 20); from V5 orange-red.
- **Its end** (state 1: at V1–V2 an enemy it meets, at any level the end of its life,
  200 frames or 400 from V4, more than 81 units from the hero or out of the world):
  `explosion_spawn` at its place, then sound 4 and it goes. To V2 S 0.3, counts 1, 1, 1
  and 20 smoke, debris 1, dome, colour set A, damage 1.0 in 0.75; V3 the same in blue
  (set A with red and blue swapped) and no damage; V4 S 0.5 with a shake, counts 25, 20,
  20 and 20, 45 glares, distortion, dome and ground ring, in blue, half the gun's damage
  in 4 units; V5 S 0.5, counts 1, 1, 1 and 10, nothing else, red (0x384EFF, 0x33FF,
  0x404FFF, 0x20FF, 0x8FFF, 0x30FF; smoke as set A).
- **Sounds** (its class's table): 0 loops as it flies, 1 as it glances off a wall (or
  an enemy that has no sound kind), 2 or 3 off an enemy by the enemy's kind, 4 at its
  end.

#### Shield Charger (0x3410E8, V1–V4) [code, 2026-10-29]

Its shield's look (`shield_draw` 0x341C10: arcs, shell, rim, orbiting trails, glow) is in
`weapons.md`, "Shield Charger"; the kit draws its shell, arcs, trails and, from V3, its
ball (`rac_demo::fx::shell`).
- **The shell** (0x342098) [code, data from the Veldin image, 2026-10-29]: its centre
  (the shield's +0x30), a frame turned so +x points at the camera (0x1E2A40), z up, each
  axis 3.0 long (0x1DD288). In it:
  - the dome: 290 points (0x2A5AE0) on the +x half of a 0.3 u sphere, every 11.25° of
    latitude from pole to pole and 22.5° of longitude, so 0.9 u round, drawn as three
    strips (148, 109 and 36 points from points 0, 146 and 254) in effect texture 21
    (0x1DD284) with coordinates that project it straight on (u = 0.5 − y / 0.6, v = 0.5
    − z / 0.6 in the sphere's units: from the camera, a disc of the whole texture), in
    the shield's colour at alpha 0x18 (0x1DD294), blended (0x44);
  - the rim: 32 quads (corners by the shorts at 0x2A7C20) between a ring of unit vectors
    in its y–z plane every 11.25° (0x2A7620) × 0.3 and a ring of 1.3 × 0.3 × 1.125
    (0x1DD28C, 0x1DD290): from 0.9 to 1.316 u facing the camera, in texture 11 with the
    inner corners at its middle (0.5, 0.5) and the outer at its edge (0.5, 1.0), the
    shield's colour at alpha 0x40, added (0x48).
- **The arcs** (laid in the update 0x3410E8, drawn by 0x341DA0) [code, data from the
  Veldin image, 2026-10-29]: five, of 10 points each at 0x279500 + i × 0xA0.
  - Each has a frame timer (shorts at 0x1E1A10); when it runs out the arc is laid anew:
    a life of rand_int(3, 45) frames (0x1E1A30), point 0 at the centre and a step of
    length 0.15 (0x1DD2CC) any way (0x440B30); each of the 9 further points is the last
    plus the step, turned first about the way from the camera (0x1E2A40) to the last
    point by rand(5°, 75°) (0x3DB2B8C2–0x3FA78D36 rad), +, −, + and so on in turn. Each
    point's w is 0.04.
  - Each frame the draw keeps the shape but moves point 0 to the centre (the arc follows
    him, with no jitter) and takes the alpha as 32 (0x1DD2D0) × frames left / life ×
    0x1E1A54 (1.0, set as it rises): 0x20 as it is laid, falling to 0x20 / life.
  - 0x407240(0, 4.0, 0, points, core texture, 14, 10, 0x7F7F7F at that alpha, edge colour
    at that alpha, 1) draws it as quads a piece, each point's side across the piece
    coming into it (the first's across the piece leaving it) as seen from the camera, ±w
    for the core in the core texture (0x17 at V1–V2, 0x0F at V3–V4, 0x30 at V5) and ±4w
    for the glow in texture 14, u along each piece and v across, both blend 1 at 0x80
    (0x4075A8). The last argument closes it: the ninth piece runs from point 8 back to
    point 0's sides, and point 9 is never drawn, so each arc is a loop out from the
    centre and back.
  - Edge colours (R in the low byte): V1 0x205080, V2 0x2040DF, V3 0x606020, V4
    0x7F4040, V5 0x7F7F7F.
- **The orbiting trails** (set up as it rises in 0x340DD0, drawn by 0x342478) [code,
  2026-10-29]: four, of up to 20 places each at 0x279000 + i × 0x140, kept from the
  centre, the count of each (0x1E19C0) growing a frame at a time to 20.
  - As it rises, with the moby's turn left as none: head 0 along x turning about z, head
    1 along y about x, head 2 along z about y, head 3 along (x + z) / √2 about that × y
    (axes at 0x1E1980 + i × 16), each 1.0 (0x1DD2A0) out.
  - Each frame the places shift one on and the head turns 6° (10° at V5) about its axis
    (0x4475E8, 0x40CBC0), has rand(−0.025, 0.025) added to each of x, y and z (0x448CD0)
    and is kept 0.9–1.1 × 1.0 out.
  - 0x342848(0.02, 0.07, 1.3, places from the tail, 20, texture, texture, 0x207F7F7F,
    0x007F7F7F, the shield's colour at 0x20, 0x20000000, from 0, to count − 1, count)
    (its last seven arguments read from the call's registers and stack) draws, from
    index 0 until the head's index, so the head itself left out:
    - the core: quads a piece, each point's side across the piece coming into it, ±0.02,
      its colour from 0x007F7F7F at the tail toward 0x207F7F7F, t = index / (count − 1)
      (`colour_lerp`), so a grey fading in to 0x20;
    - the glow: a quad a piece, its two ends taken out along the view to the farther
      one's distance from the camera, then each moved 1.3 × the piece on past its end,
      ±0.07 across, the whole texture along it, its colour from 0x20000000 toward the
      shield's colour at 0x20 by the same t: so nothing at the tail and the shield's
      colour at the head, added.
  - Texture 0x2C at V1–V2, 0x0F at V3–V4, 0x30 at V5, blend 1 at 0x80. The shield's
    colour is the one the shell takes (+8 of its variables), flash and all.
- **A zap** (V1–V4, every 0.5 s on each enemy touching the 0.9 u ball): no line to the
  enemy, but at the point on the shell toward it (the shield's centre plus 0.9 u, from
  0x1DD288 × 0.3) eight pairs of 0x35 sparkles (0x342DB8), each pair from one f =
  rand(1, 6): half f shrinking to 0.1 f in the shield's colour at 0x7F (its rest colour
  but for 30 frames after a hit: V1 (0x90, 0x20, 0x20), V2 (0x90, 0x40, 0x20), V3 (0x20,
  0x50, 0x20), V4 (0x20, 0x50, 0x90)), and 0.4 f to 0.04 f in grey 0x7F at 0x7F, spinning
  opposite ways; both jittered ±0.1, 0.5–6 u/s any way, life 20; render byte +9 0x74.
- **The 0x3E sparkles** (spawner 0x469378: half 0.143, (0x40, 0x40, 0x7F) at 0x7F,
  fading 127 / life a frame, following a moby when its mode byte is 2): eight on the 1 u
  sphere from the update, and two at each orbiting trail's head from the draw (0x342478),
  of half 0x1DD2B0 × 1 and × 0.15, the first in the shield's colour, the second (0x7F,
  0x7F, 0x7F) at 0x40.

#### Rift Inducer (0x372990, rift class 0x186B 0x373990, V1–V4) [code, 2026-10-29]

"s" is the rift's size, 0.5 (0x373738's first argument). V5 (the Rift Ripper) is the
row's flag 0x1DDA84.
- **As it fires,** for one frame (0x3735E0, queued once): two flames of 62 (63 at V5)
  from a point 1 u (V5 2 u) along the gun's −y axis, lying along the aim reversed
  (0x373378; which end is the muzzle isn't settled), rand(1, 1.3) long (V5
  rand(2, 2.5)) and half rand(0.275, 0.475) wide (V5 rand(0.75, 0.95)), white at 0x7F,
  added; two sparkles of 49 at random rolls and a glow of 11 pushed 0.08 toward the
  camera, half 0.9, (0x90, 0x40, 0x20) at 0x3F; a star of 65 of half 1.0 rolled 30–60°,
  grey 0x90 at 0x3F; all added. And (0x372F48) twelve pairs of still 0x55 down a column
  from 0.3 u above the gun and 0.15 u to its side, 0.033 u apart, jittered ±0.1 (V5
  ±0.3), life 7–12, at a random roll: a blue glow (0x5B's entry 1) of half 0.175 (V5
  0.4), grey 0x80 at 0x7F, and a sparkle (0x35's texture) of half 2.25 in (0x80, 0x30,
  0x20) at 0x10; then 25 0x13 particles (spawner 0x462B40) in a fan (not read).
- **In flight** (state 0, at most 36 frames): it wobbles 0.05 u up and aside by a sine
  turning 0.58 rad a frame; a light (0x421F08: radius 5, (0.45, 0.2, 1.0)) rides it.
  Within 96 u of the view (0x376010): three billboards of 76, half 1.2 s, 0.7 s and 0.2 s,
  in (0x40, 0x40, 0xFF) at 0x6F, (0x32, 0, 0xFF) at 0x7F and (0x10, 0x10, 0x70) at 0x7F,
  turning at rand(−1.5, −2.5)°, +rand(1, 2)° and −rand(1, 2)° a frame (shared by every
  rift, 0x1DDB2C–0x1DDB34, the drawn size 0x1DDB38 = 0.4 [data]); one of 75 of 0.7 s in
  (0x80, 0x40, 0x20) at 0x7F turning with the first; three spirals of 71 (0x4086B8, not
  read: size s, 1.0 and 1.4, blend 0x48) in (0x32, 0, 0xFF), (0x20, 0x40, 0x80) and
  (0x7F, 0x20, 0x20) at 0x4F; and a glow of 11 of 1.4 s in (0x40, 0x20, 0xFF) at 0x3F at
  a random roll; all pushed 0.01 toward the camera, added. Its trail (0x376328): every
  0.1 u of its way the Disc Blade's dark 0x2C smoke (half 0.3 growing 10000 / 210000 a
  frame, life 3–6, rising 0.1–0.2 u/s, spin ±1–3) in (0x40, 0x40, 0xFF) at 0x7F or
  (0x40, 0x7F, 0xFF) at 0x60 by the frame counter, all blended; in its first 60 frames,
  every other frame on average, the Disc's orange 0x0F puff; and three 0x61 sparks in
  0x61's entry 6, (0x20, 0x10, 0x80) at 0x6F, half 0.3 to 0.12–0.18, life 15, at points
  of a turning table (0x1DDAB0) whose placing isn't clear.
- **Opening** (state 1, where its step meets something or after 36 frames): its light
  goes; 24 rays' ways are set at random in the view's plane.
- **Open** (state 2, up to 600 frames), within 96 u of the view (0x3753F8): a lens of 32
  quads round it that warps the frame (texture −1 through 77), swelling with how full it
  is (+0x80); four refracting rings of 54 (0x3750E0) at radii from 2.25 s shrinking 0.04
  and 0.025 a frame and starting over, and two at 2.27 s turning opposite ways; 24 rays of
  18 (`fx_beam`, half width 0.08 s, growing 0.03 a frame to 3 s long, red (0xFF, 0x20,
  0x20) at its middle to blue (0x20, 0x20, 0xFF) at 0x2F at its end, the texture
  scrolling), turning; and stars of 57 (half 0.325 s and 0.351 s, white at 0x4F) and
  glows of 11 (0.39 s and 0.455 s, (0x40, 0x20, 0xFF) at 0x3F, blends 3 and 1), at new
  rolls each frame. Each frame four dark-smoke 0x61s in (0x40, 0x40, 0x80) fade in from a
  shell s–2.2 s out, drawn in at 2 s–2.7 s u/s (half 0.8 s to 0.3–0.5 s, life 20–25,
  spin ±2–5), and where the ground is within 5 u below, five more on a ring 2 s–3 s round
  that point drawn in at 4 s u/s (one blended) with brown rocks (0x61's entries 10–12) half
  of the time, and five on a ring in the view's plane.
- **The lens** (0x3753F8) [code, 2026-11-03; re-read in the disassembly 2026-11-04]: how
  full the rift is (+0x80, kept within 0 to 1) grows 0.005 a frame from opening to 0.75
  (0x373FE8). With f that, 33 points on an ellipse across the line from the camera to
  the rift: its axes the side (the game's down, 0x1A4EC0 = (0, 0, −1), crossed with that
  line) times f + 1 and the view's up (the line crossed with the side) times f + 0.5, so
  wider than tall; its middle and rim both f + 0.5 beyond the rift along the line (the
  code works out a push of 0.05 toward the camera, then adds the line's vector instead)
  and 0.05 to a side; its phase turning 0.0116 rad a frame. For each two points and the
  middle, a wedge drawn twice:
  - first texture 77 (white, its alpha 0 at its top row rising to full by its fifth),
    v 0 at the rim and 1 at the middle, with blend 0x4A ((0 − 0)·As + Cd: the colour
    kept) and TEST 0x31001 (alpha test NEVER failing to FB_ONLY, Z test ALWAYS), so its
    alpha alone goes into the frame's;
  - then the copy of the frame (texture −1, above: the frame as drawn so far, 256 × 256),
    its texture coordinates the points' places on the screen (0x401BB0), but the
    middle's pulled toward the middle of the wedge's rim by k = (1 − cos(f × π)) × 0.3,
    with blend 0x54 ((Cs − Cd)·Ad + Cd, mixed in by that alpha) and TEST 0x53001 (alpha
    test NEVER failing to RGB_ONLY, Z test GEQUAL, so behind what is nearer and leaving
    the alpha); then 0x5360B, the usual test, back.
  So each point takes the frame from k of the way out to the rim along its line from the
  middle: what lies round the rift is drawn in toward its middle, and what is behind its
  middle within k of the rim's distance doesn't show. k is 0 empty, 0.51 as full as it
  gets. The mask fades it out over the outer eighth.
- **The rings** (0x3750E0) [code, 2026-11-04]: a quad facing the line from the camera
  (the unit square's corners (0, ±1, ±1), 0x1D7C50, times the radius, rolled), 0.5 nearer
  the camera; texture 54 (a ragged ring) into the frame's alpha as the lens's mask, then
  the copy of the frame with each corner's coordinates those of its place 0.8 of the way
  out from the middle, so magnified 1.25 within the ring, by 0x54 and 0x53001 as above.
  Two of radius +0x70 and +0x74, shrinking 0.04 and 0.025 a frame and starting over at
  2.25 s at 0.1, rolled +0x78 (turning 0.0436 rad a frame) and +0x78 + 0.0727; two of
  2.27 s rolled −(+0x78) and +0x78.
- **In the kit** (`rac_demo`'s `fx/open.rs`, `fx/refract.rs`; the `open` of its recipe)
  [seen, 2026-11-04]: s is 0.5, from its spawn's first argument (0x373738, called from
  0x372990). The rays' alpha is 0 at their middle (0x2020FF has no alpha byte), so they
  show only toward their blue ends. The lens and rings are drawn in Bevy's transmissive
  pass by a material reading its view transmission texture (the frame as the opaque
  passes left it, so without the effects drawn before them in the game's copy), blurred
  by about a texel of the game's 256 × 256 copy, depth-tested, each point taking the
  copy from where the game's does (`refract.wesl`; the lens's mask by its distance from
  the middle, so without its 32 wedges or their phase). On `--gun-level 5` takes
  (`rift-lens2`) it draws the sky in toward the rift's middle and an enemy being
  swallowed out into a streak toward it, and the rings bow the horizon up round it; the
  game's take (`review-rift-0x63`) shows the cliff behind smeared in the same way.
  Closing, it stays a shot for its 20 ticks (`RiftState::closing`, 2026-11-04): the lens
  and smoke go, and the rays and rings go as "Closing" below has them (the recipe's
  `closing` for each; `open.rs`, `refract.rs`); its stars and glows are its impact's,
  from the tick it begins to close. Seen at V5 (`kitdemo-arena-rift-close.orin`,
  `--every-tick`): on its first closing tick the lens's warp of what is behind goes, the
  two rings as wide as the rays bow it a few ticks more, the rays draw in as short wide
  dashes and faint rings narrow round the star, then only the closing stars are left
  until it goes. Not compared with the game's footage of a close.
- **Its smoke** [code, 2026-10-31] (0x373990): every 0x61 it draws in has its texture byte
  set to 0x17's first (0x221174), and keeps 0x61's own blend, added (0x48; the spawner
  0x46DCB8 sets +3 = 0x48, +9 = 0x44), fading in from alpha 0 to 0x7F: the four from the
  shell in (0x40, 0x40, 0x80), and each of the two rings of five (on the ground, in the
  view's plane) in grey 0x40, the fourth of each blended (0x44), the ground's with a
  brown rock half the time. So most of it brightens; the dark disc seen round the open
  rift in the game is its lens.
- **Compared with the game** (2026-10-31, `uyantsc-veldin-fxref.sh 9` against the kit's
  `-rift-near` drawn every tick, `--every-tick`): firing and flight close (a tick of
  white flames and star, the pink 0x55 sparkles fading over 7–12 ticks, the shot leaving
  inside them). Open, the game shows a faint dark disc round a bright purple star with
  long rays, dust stirring; the kit drew grey lumps, as it blended all its smoke at half
  alpha. With the smoke added as the game's, the kit shows a bright centre in faint
  rings with dark bits and rocks on the ground. Not drawn still: the lens, which
  darkens the game's (drawn since, 2026-11-04: "In the kit" above), and the view's-plane
  ring.
- **Closing** (states 3 and 4, 20 frames, 0x375C70) [code, the ring calls checked in the
  disassembly, 2026-11-04]: no lens and no smoke (`weapons.md`, "Closing", for when).
  - The rings of +0x70 and +0x74 go on narrowing 0.04 and 0.025 a frame but stop at 0
    rather than starting over, and take the copy from 0.5 of each point's way out
    (0x3750E0's fourth argument; 0.8 open), so they magnify 2.
  - The other two, rolled −(+0x78) and +0x78, are as wide as the rays are long: their
    radius is +0x44, 3 s as it begins to close (2.27 s open), taking it from 0.8 still.
  - The rays shorten 0.125 a frame (gone in 12), still turning and scrolling, drawn at a
    half width of 0.08 (open 0.08 s) with their end at 0x1F (open 0x2F). `fx_beam`'s
    second argument, 3 s open and 3 closing, isn't read.
  - The stars of 57 (half 0.25 and 0.27) and glows of 11 (0.3 and 0.35; open, 1.3 s
    times those) stay, at new rolls each frame.
  - Sound 1 only at the end of state 4.

#### Suck Cannon (0x3619C8, shot class 0x1872 0x381B98, V1–V4) [code, 2026-10-29]

- **Sucking:** the vortex is a strip of 42 vertices (0x3650A0) in an effect texture from
  0x1DD7B0, and 0x17 smokes along it (0x363E98); not read further.
- **Taking something in** (0x3626E0, at the muzzle, with sound 1): the Flux's muzzle
  sparkles in orange: four 0x35 from half 0.1 growing to 5.25, 3.75, 2.625 and 1.875, in
  (0x80, 0x40, 0x20) and (0xA0, 0x40, 0x20) at 0xBF and white at 0xC0 and 0xB0, life 14,
  spin ±4; and three 0x5C (0.84 to 1.08, 0.84 to 2.04, 1.8 to 2.4, grey 0x78 at 0x46,
  0x46 and 0x2D, life 20).
- **The shot** (0x382830) is class 0x1872 given the model of what it took in (0x442E08)
  at 0.7 of its class's scale. Each frame (0x382A08) every 0.1 u of its way (five fewer a
  frame from V3) a 0x2C smoke jittered by a running ±0.35, half rand(0.75, 1.0) (from
  V3 rand(0.2, 0.4)) growing 10000 / 210000 a frame, life 12–15, rising 0.1–0.2 u/s,
  spin ±1–3, in (0x50, 0x40, 0x40) at 0x4F or grey 0x40 at 0x5F by the frame counter,
  blended; in the dark smoke's texture to V2 and from V3 in 0x5B's first (particle
  texture 12, fire): the V3 "sets them alight". In its first 60 frames, every other
  frame on average, the Disc Blade's orange 0x0F puff.
- **Ricocheting** (from V2, 0x382620): twenty 0x2C sparks as the Agents' small bullet's
  (half 0.05–0.2 growing 500 / 210000, 0.5–1 u/s within ±80° of a bearing and 0–80° up,
  life 30, grey 0x7F at 0x7F, spin ±1–2).
- **Its end** (state 1, by the class it held): for classes 500 and 0x1872 something
  picked one time in three (not read); for 0x1FF nothing; otherwise to V2 half the time
  the shared explosion at S 0.7 (counts 10, 10, 10, 10, 10 glares, flag 1) and half the
  time pieces (0x447D00) and twenty 0x02 particles (0x45FEC8) in pale colours; from V3
  always the explosion. Sound 0.

#### Qwack-O-Ray (0x37DEB0, beam 0x37E830, V1–V4) [code, 2026-10-29]

Numbers from Veldin's data at 0x1DDBC4 and 0x1DDC00–0x1DDCB0 [data]; V5 (the
Qwack-O-Blitzer) is the row's level byte above 3.
- **The beam** (0x37E830 each frame it's on, drawn by 0x37F738): 12 u long (0x1DDBC4),
  or to its target, bending onto it; 12 rings at 1/12 of its length, each of 20 points
  round it (18° apart) at a radius from 0.05 at the gun to 1.234 at the far end
  (0x1DDC18, 0x1DDC1C): a funnel. Quads between the rings in texture 13 (0x1DDCA4),
  blue (0, 0, 0xFF) at 0x40 (V5 red (0x7F, 0, 0) at 0x60), added, the texture scrolling;
  two strips spiralling round it (0x37FB70, textures 14 and 16 from 0x1DDCAC and
  0x1DDCB0; widths 0.05 to 0.4, at radii 0.01 to 0.5) in (0x10, 0x3F, 0xFF) and (0x10,
  0x10, 0x7F) (V5 (0xFF, 0x3F, 0x10) and (0xFF, 0x80, 0x40)); rings of 8 (0x380140) and
  a glow of 11 (0x380390), not read further. Every 45 frames a pulse runs along it at 8
  u/s, growing from 0.1 to 1.5 (up to three at once).
- **Its sparkles,** each frame: one time in two each, a pair of 0x35 at the last point of
  each spiral (0x28F790, 0x28F850), shrinking from size f = rand(0.1, 1.7) to 0.1 f in
  (0x10, 0x10, 0x7F)
  or (0x10, 0x4F, 0xFF) at 0x7F (V5 (0x7F, 0x10, 0x10) or (0xFF, 0x4F, 0x10)), and from
  0.7 f to 0.07 f in grey 0x7F, life 30, in 0x61's entry 7 (Veldin's particle texture
  46, a swirl); one time in two a pair of 0x4E (0x46AED8) flowing along it, in one of
  five blues (0x30, 0x7F, 0xFF), (0, 0x64, 0xFF), (0x20, 0x20, 0xFF), (0, 0x7F, 0xFF)
  and (0x20, 0x20, 0xFF) (V5 oranges; corrected 2026-10-31), in 0x61's entry 6 (texture
  45, a soft star). The 0x4E pair starts at the rope's first point less 0.25 × the
  beam moby's vector at +0xD0 (not identified), at 0.2 u a frame along the rope's first
  step turned up to 15° (0x1DDC44, 0x1DDC4C [data]), living 1.2 × the beam's length /
  0.2 frames (72 with nothing to take), three times that with a target, which the white
  one is given to home on (the blue one's call isn't clear). Both kinds flash (the types'
  two-phase curve, under "Particle types' updates"): at full size for a frame, halved and
  half faded by frame 5 [checked on live particles in a savestate, 2026-10-31].
- **How the beam is laid and drawn** [code, data, 2026-10-31] (`rac_demo`'s `fx/funnel.rs`):
  - Its rope lags: each of 12 points is a twelfth of the way from the one before, along a
    way between where that point was a frame ago and the aim, the aim's share 0.9 − 0.5 t
    for t along it. So its far end swings behind him. It reaches 11/12 of the way.
  - The quads run from ring 1 to ring 11, drawn only where they face away from the camera,
    so the far wall shows and what it hits stays in sight (`0 ≤` the dot skips them). Each
    is drawn twice: one coordinate goes 1/15 a ring and scrolls 0.04 a frame, the other
    goes once round the ring, the two layers scrolling 0.01 a frame opposite ways. Ring
    11's alpha is 0.
  - Each spiral's points are at a radius going 0.01 to 0.5 along the rope, at an angle
    growing 0.3997 rad a point and 0.2007 a frame, the second spiral the other way round.
    Each draws a core of 14, half width 0.05, grey at 0x80, and a glow of 16, half width
    growing to 0.4, in its colour at 0x40, both scrolling 0.3 a frame.
  - At the gun, 0.5 u along the barrel: glows of 11 of half 0.4 in (0x20, 0x40, 0x80) at
    0x7F and half 0.1 white (0x380390).
  - A pulse (0x380140, up to three): every 45 frames from the gun at 8 u/s for 60 frames,
    a ring of 8 growing from half 0.1 to 1.5, turned round the beam, (0, 0, 0x7F) at 0x40
    (V5 (0x80, 0x40, 0x20) at 0x60). The kit turns it to the camera.
  - With nothing to take it still shines, 12 u along his aim. The kit used to draw nothing
    then.
  - **Compared with the game** (2026-10-31, the game's `uyantsc-veldin-fxref.sh 7`
    against the kit's `-qwack-near` take, both at V3 from behind with nothing to take):
    the game shows a thin white-blue core past his head about a head across, the spirals
    curling into a blue loop at the far end, blue and purple sparkles along it and almost
    no funnel wall; its light tints the sand by (4, 8, 17) % of its colour beside him. The
    kit drew a white haze two or three heads across round the gun: its sparks stayed big
    and bright for their whole life instead of flashing, and every particle was drawn √2
    too wide ("Particles", "How big it draws one"). With both fixed it is close; it still
    looks whiter on the demo's pale floor, where added layers saturate sooner. The kit
    gives the flowing sparks one blue of the five.
- **Lights:** radius 7.5 at the gun while it fires (0x37DEB0; (0x20, 0x40, 0x80), V5 (0x80,
  0x40, 0x20)), 0.5 u down the gun's barrel from its origin, kept 20 frames after it stops
  (0x1E1E48); radius 5.5 (0x4080FF) on the V5 duck.
- **Which way the beam goes** [code, savestate, 2026-11-03]: the rope's steps follow the
  gun's barrel, the held gun moby's −y (its +0xD0 row negated; the muzzle is (0.21, −0.6,
  0) in the gun's frame, 0x1DDBE0), not the aim; the aim (0x1A52A0's yaw and pitch, or a
  target's) only casts the ray that sets its length. In `uyantsc-veldin-qwack-beam.p2s`
  (V3) the rope runs 11 u along his facing, rising 2.5° and 2.6° to his right. The takes'
  beam looks to rise past his head only from the camera's height behind him.
- **The ducks** (classes 0x1BC3 to V3, 0x189B at V4 tinted (0x40, 0x10, 0x10), 0x1D3E
  from V5): a duck's end to V3 (0x3D2870) is a 0x16 puff (half 0.714, grey 0x7F at 0x4F
  to 0x20) and a 0x79 (0x7E's textures 52 and 61); the V4 and V5 ducks going up (0x39E8A0,
  0x3D9008) add the shared explosion at S 0.8 (shake, counts 25, 20, 20, 20, 45 glares, 2
  debris), as the Annihilator's; the V5 duck's flight is drawn by 0x3D8F20 (two glows of
  11, (0x80, 0x20, 0x20) at 0x40) and 0x3D9858 (two stars of 57). An egg (0x3D48B8) sets
  off the explosion at S 0.3 (counts 5, 5, 5, 10, no glares, 3 debris) and sheds a 0x61
  and a 0x17 as it flies.

#### RY3NO (0x3805B0; missiles 0x383370 at V1, 0x3DB4D0 from V2) [code, 2026-10-29]

- **The gun:** a light of radius 8 at the muzzle as it fires; smoke there (0x381730: a
  0x13 and a 0x2C in (0x70, 0x50, 0x50) at 0x6F growing 500 / 210000 a frame, drag 0.95,
  life 12); a screen warp through texture 120 (0x3813A0 → 0x381418), not read further.
- **A missile in flight:** a glow of 44 at a random roll (0x4076F8, as the Annihilator's
  missile) in (0x20, 0x40, 0x80) at V1 (0x1DDDCC [data]) and (0xFF, 0x40, 0x20) from V2
  (0x3DC848), which adds two quads of 43 (0x3DC8D0) in (0xFF, 0x30, 0x10). Every 0.4 u
  of its way (0x383DA0, 0x3DC5B0) a 0x2C puff in 0x5C's entry 3 (0x1DDDC0, 0x1DDDC4 and
  0x1DF580, 0x1DF584 [data]: Veldin's particle texture 24, a soft glow) of half 0.1
  growing rand(200, 1000) / 210000 a frame, rising 0.2–1 u/s, alpha 0x50, spin ±1–3,
  blended: grey 0x7F or 0x40 by the frame counter, life 30–50, at V1; (0xFF, 0x80, 0x40)
  or (0x80, 0x40, 0x20), life 20–40, from V2.
- **Its end:** at V1 the shared explosion at S 0.2 (counts 7, 0, 0, 3 smoke, 3 glares, no
  flags); from V2 (0x3DBDA0) four 0x2C sparks (half 0.05–0.2 growing 500 / 210000, 0.5–1
  u/s within ±80° of a bearing and 0–80° up, life 30, (0x7F, 0x20, 0x20) at 0x7F) and
  five 0x5C fireballs shrinking from half 1.05, 2.05, 1.65, 2.35 and 1.75 to 0.05, 0.4,
  0.15, 0.5 and 0.3 over 80, 60, 80, 54 and 54 frames, in (0xFF, 0xA0, 0xA0) at 0x64,
  (0xF7, 0x32, 0x0A) at 0x3C and 0x32, and (0xF7, 0x14, 0) at 0x3C and 0x1E (0x5C's
  entries 1, 0, 4, 0 and 4); and a class 0x1DF6 (0x3DD390, drawn by a beam, 0x3DD9B0),
  not read.

#### Infector (0x3688E8, blob class 0x1E05 0x3DE4F0, V1–V4) [code, 2026-10-29]

Its green is 0x5A182, (0x82, 0xA1, 5); "glob" below is a 0x61 drawn in type 0x69's
texture (Veldin's particle texture 57, a white blob of goo), "smoke" one in 0x17's (the
dark smoke), "bubble" one in 0x22's. The 0x61s here are half added, half blended
(0x440848(2) picks the blend byte, 0x48 or 0x44). Speeds are per frame in the code; given
here in u/s.
- **At the muzzle** each frame while it has ammo: a glob (half 0.3 to 0.15, life 10–20,
  alpha 0x30, gravity 2 u/s², spin 1–4, added), and one frame in 12 a drop (0x19) jittered
  ±0.075, rising 0.3–0.9 u/s.
- **As it throws** (0x368FE8, from the same point the blob leaves), along the aim turned
  a little more for each particle (a random walk: rand(−2.5, 1)° about the hero's side
  axis and rand(−4, 4)° about his up, per particle, so the spray fans about 10° and drifts
  up to some 20°):
  - to V2: 16 globs (half 0.3 to 0.2, 6–30 u/s, life 30–50, alpha 0x30, gravity 1 u/s²,
    spin 1–4); 12 smokes (half 0.4 to 0.8, 6–24 u/s, turned only rand(−0.5, 1)° and
    rand(−1, 1)°, life 20–40, alpha 0x20, spin −1 to −3); 8 bubbles (half 0.3 to 0.1,
    6–30 u/s, life 20–40, alpha 0x30, spin −1); 8 drops (6–24 u/s, gravity 20 u/s²);
  - from V3: 20 globs (12–36 u/s, gravity 5 u/s²), 16 smokes (12–30 u/s, gravity 5,
    life 30–50), 12 bubbles (12–36 u/s, gravity 4, life 30–50) and 12 drops (12–30
    u/s), the turns rand(−2, 1)° and rand(−3, 3)°; and eight lines from a random point
    along 12 u of the aim 5 u down, each that meets the ground leaving a gas cloud there
    (0x368E20: a 0x34 within 0.6 u, half rand(0, 1.2) to rand(1, 4), (0x82, 0xB1, 5) at
    0x50 to 0, life 70–130, blended): the V3 splash.
- **The blob** is class 0x1E05's goo mound at a random 0.25–0.3 of its class's scale
  (0x3DE3A8), tumbling about a random axis at 2.5–5° a frame. Each frame (0x3DE4F0):
  a glob within ±0.1 rising 0.4–1.0 u/s (half 0.28 to 0.13, life 10–15, alpha 0x4F, spin
  1); within 96 u of the view (0x3DF888) each a third of the frames: a smoke (half 0.6 to
  1.1), a glob (0.5 to 0.4) and a bubble (0.4 to 0.2), each thrown any way at 2.1–3.0,
  0.6–2.4 and 0.6–1.8 u/s from ten frames' way out, life 10–30, alpha 0x40, gravity 24
  u/s², and a drop sinking 0.6–3 u/s (gravity 15); and a third of the frames, if the
  ground is within 5 u below it, a gas cloud there (0x3DFC48: within 0.6 u, half
  rand(0, 1.75) to rand(2, 5), (0x62, 0xC1, 5) at 0x50 to 0, life 100–160, two in three
  added).
- **Its burst** (sound 1): two gas clouds within 0.6 u (half rand(0, 1.5) to rand(1.5,
  5.5), (0x20, 0x50, 0x30) at 0x30 to (0x10, 0x28, 0x18) at 0, life 100–160, blended);
  a ring (class 0x307, 0x301508: scale rand(2, 2.5) growing 1.04 a frame) tinted (0x52,
  0xD1, 5); a bubble 1 u below it in (0x62, 0xD1, 0x10) at 0x88; one time in three a
  drop; and a gas cloud within 2.5 u (half rand(0, 1.5) to rand(3, 5), (0x52, 0xD1, 5) at
  0x50, two in three added).
- **The Infecto-Bomb** (V5, 0x389F58; bomb 0x38E3A0, burst 0x38EF90): the muzzle's glob,
  a landing marker, and a bomb and burst with the same kinds of parts (smokes, globs,
  bubbles, drops, gas clouds, the ring); its numbers aren't read.
- **The particle types** [code, 2026-10-29]:
  - 0x19 (update 0x463AB0), a drop or spark: it falls by its gravity argument (0 gives
    −0.0063 u a frame², 22.7 u/s²), shrinks 100 / 210000 a frame, and where its path
    meets the world it stops and doubles in size; its colour starts white at alpha 0.6
    (0x445B88) and loses red 0.01, green 0.03 and blue 0.05 a frame (white to yellow to
    red: the Lava Gun's sparks), or with its flag argument set red 0.03 and green 0.01
    (white to yellow-green to green: the Infector's drops); life 80; a negative size
    argument is a random half 0.024–0.143.
  - 0x34 (spawner 0x467BD8, update 0x467CF8), the gas cloud: still, 0.05 u above where
    it is put, its half size lerped from the first argument to the second and its colour
    from the fourth to the fifth over its life; type 0x34 has no list of its own, so it
    takes the table's first byte, particle texture 0 (the dark smoke) on Veldin. Whether
    the renderer draws it flat isn't read: its +9 byte is 0x22, where the spawners set
    0x44, 0x20, 0x42, 0x70, 0xA4 and some by their arguments in nibbles, so it holds two
    fields, not a flag. The kit draws it facing the camera, laid on the ground where the
    game finds the ground (2026-10-29).
  - 0x22 (spawner 0x465020, update 0x4651D0), the bubble: it grows 840 / 210000 a frame
    from 0.025 to its size × rand(0.75, 1.25), keeps 0.97 of its velocity, rises toward
    1.3 u/s and wobbles side to side; above its height argument (the water's) it stops
    rising and lives 12 frames more; it fades over its last 14 frames.

#### The twenty guns against the game at V5 and V3 [footage, 2026-11-01]

Each gun fired on Veldin at its V5 row, and seven at V3, against the kit's arena on the
same firing pattern (`uyantsc-veldin-fxreview-all.sh`, `kitdemo-arena-review-all.sh`;
`docs/re-setup.md`). The kit was filmed as built and again on a darker floor with no tone
mapping, a temporary build, so that colours could be judged. The place, camera and enemies
differ, so sizes and distances are rough, and lavapipe holds frames, so timing isn't judged
here. The game's takes show its 16:9 view at 4:3, everything 0.75 as wide as it is
(found 2026-11-03; `re-setup.md`, "Gotchas"). **Not filmed:** V1, V2, V4 and V6–V8. The Shield Charger hangs the game on Veldin at
V3 (row 0xC0) and V5 (0xC2), so its V5 is judged from one frozen frame and its V3 not at all.

| Gun (V5) | V5 against the game | What's behind it |
|---|---|---|
| Shock Blaster (Shock Cannon) | Close, from its code (2026-11-01): the flash on the lens, the violet-white ring sphere, the spokes, dust and the rays' white and blue plume, as the game's. The charged lance drawn (2026-11-04, compared on `review-shock-charged`): the pale violet oval round him round a blue column along the aim, swept as he turns; its domes and the uncharged lance's now open from the gun, as the game's | The charging sprites and glare at the gun aren't drawn. The screen warp (0x388508) is drawn (2026-11-04), though its ghosts show crisper on the greybox than on the game's scenery. The game's column is white at its core where the kit's is pale blue, adding in linear light |
| N60 Storm (N90 Hurricane) | Close (2026-11-03). The tracer is one long flame now, V5's own tail and flames ("N90 Hurricane"), redder than the game's | Its landing not seen in the kit's take; the bullet's model (0x1881) not drawn, its blend not read; the tail's scroll and fade; the colours add in linear light |
| Nitro Launcher (Eruptor) | Close (2026-11-05). V5's own burst, read whole: the orange and yellow bands spinning round it and shrinking before it bursts, the pop, the white screen, the fire ring, sparkles and streaks, the bands and rings growing along the ground, the warp, and the chunks and rocks (2026-11-03), compared on `review-nitro-0x7B` | Its fire comes out redder and dimmer than the game's yellow-white (linear light); its dome of 23, masked discs and quads of 54 aren't drawn; the chunks' smoke barely shows on the kit's bright floor |
| Lava Gun (Liquid Nitrogen Gun) | Partial. Its stream, nozzle and pools are V5's blue from code (2026-11-01). The game's arcs high because it leaves along the barrel, 12° up, not by a lock-on; the kit's does too since 2026-11-04 (`weapons.md`, "The barrel") | The hold's sway and the waist's pitch aren't followed |
| Annihilator (Decimator) | Close as far as the takes show; they land at different distances | |
| Flux Rifle (Splitter Rifle) | Close (2026-11-03). The game's red trail has dark puffs, V5's texture 9, and its orange ring is the pieces' domes where it lands (above, "V5, the Splitter Rifle"), now both in the kit; its red ribbons come out pinker | The domes not seen on screen (the kit's arena shot lands out of view); the colours add in linear light |
| Infector (Infecto-Bomb) | Partial. V5's blob, its smoke, globs and bubbles, gas and rings are from code (2026-11-01); the game's yellow-green spatter lands on enemies, the kit's blob bursts in the air off to the side | V3's wash is its gas clouds (0x34) on a grey floor |
| Spitting Hydra (Tempest) | Close, the same at V3 (2026-11-03). The game's "violet domes" are the spit's two rings of 61, camera-facing up to 10.5 and 21 u across with the floor hiding their lower halves, and the kit draws them so; its "gold ground bolt" is the charge's crackles to the floor, grey and blue added on sand. Each spit's bolt 10 u ahead is now drawn too. The kit's violet and glows come out bluer and stronger | The kit adds colours in linear light ("Importing them", "Colours"); arcs aren't cut short at walls |
| Disc Blade Gun (Multi-Disc) | Close, at V3 too. The fireball in the game's takes is an enemy dying | |
| Agents of Doom (Agents of Dread) | Partial. The Dread blast (below) and the jets are from code (2026-11-01), the jets dimmer than the game's; the agents stand on the game's slots and face his way, drawn there (logged 2026-11-01); the kit's take saw them 40° from behind, as its camera lagged his turn. Their violet and gold lightning as they appear on his slots is the game's (2026-11-05: the appearing sparkles, lightning crackles at alpha 0xFF, round their models drawn added as they fade in), as are their spin, the pod's blue burst that the on-foot agents walk out of, and their end's sparkles and fade (2026-11-05). From V5 their barrels' arms are scaled to 0.45 as its code scales them, which puts them at the game's size ("Its barrels' arms at V5", 2026-11-05) | The arms' pitch while walking or running from V5, and the gun's at V2–V4; the jets dimmer than the game's |
| Rift Inducer (Rift Ripper) | Close (2026-11-05, `review-rift-0x63` against `-rift-near`). In flight the game's small violet ball is its rings of 76 and glow, as the kit's; open, a bright violet star with faint rays in a dark smear, its lens, over stirring dust. Its lens and rings are drawn (2026-11-04): what is round it is drawn in toward its middle, as the game's; and as it closes, its rays shorten and its rings narrow away from its code. The kit's pale sphere round it, a placeholder, is gone: the rift's class is 0.05 units across at the scale its code leaves it (0x373738) | Not drawn: the smoke on a ring in the view's plane, the spray as it fires, its spirals of 71 and orbiting sparks; the lens copies only what is opaque |
| Holo Shield Glove (Ultrashield) | Partial. The game's a bright pink-white oval; nothing of it warps the frame, by its code (2026-11-04). The kit's now has V5's disc of 5, the orange rings of a hit, the stars of 57 and the shield class's own faint shell with its grey rim, as the game's takes show it (2026-11-05, `-holo-near` at V3 and V5) | Paler than the game's (the colours' linear light, P1.9); its colour's swing through its last 4 s. Its fold, a fade and a blue flash, is the game's (2026-11-05) |
| Plasma Whip (Quantum Whip) | Close (below, "The Quantum Whip's cutting wave") | V5's hit sparks not read |
| Suck Cannon (Vortex Cannon) | Close, from its code (2026-11-01): the brown tube, arcs, smoke and dust flowing to the gun, as the game's | Its refractions aren't drawn |
| Qwack-O-Ray (Qwack-O-Blitzer) | Close (2026-11-03). The floor round him is lit warm, as the game's: every gun's light is now drawn with the game's falloff (above, "How the lights fall off"), weaker on the kit's pale floor (1.25 times as bright beside him, the game's sand 1.54 times) under its tonemapper. The beam's colours are its code's; the game's looks yellower and its curls redder, and it looks to rise past his head only from the game's camera, higher behind him ("Which way the beam goes") | The colours add in linear light (below, "Importing them"); the light isn't kept 20 frames after the beam stops; the kit's beam starts along his aim, not the gun's barrel |
| RY3NO (Rynocirator) | Partial. The tail and the sun (rays and white screen) are from code (2026-11-01); the game's curling ribbons are thicker | The sun's strike on everything in view within 60 u isn't in the simulation |
| Plasma Coil (Plasma Storm) | Close, at V3 too; the kit's glow is bigger | V5's arcs (0x30A3D0) not read |
| Bouncer (Heavy Bouncer) | Close | |
| Miniturret Glove (Megaturret Glove) | Partial. The olive dome is the explosions' dome (class 0x70), now drawn with every small blast; the kit's take shows no missile bursting in view | Not filmed against an enemy |
| Shield Charger (Tesla Barrier) | Close (2026-11-03). The game's "tall oval" is the shell's circle facing the camera, shown 0.75 as wide (the takes' 16:9 at 4:3, above); the kit's squeezed so is the same shape. From V3 a ball round it fading down from its top, missing until now, is drawn (0x409A50; `weapons.md`, "Its look"). The game's rim and fill come out white where the kit's are lavender: the game adds the lavender onto bright sand in its own values, which saturates | The colours add in linear light ("Importing them"); the sparkles at the trails' heads |

**The numbers** match at all eight levels of all twenty: damage, ammo, XP, round price, price,
upgrade price, pickup, hold and animations, by script against the table at 0x2C5640 in EE
memory [code, data]. **The sounds** were checked against the code, not by ear; those not
played are listed under "In the kit and the app".

#### Which code draws each class [data, EE memory on Veldin]

The moby update table holds triples of class, update and 0x2F73B4, around 0x2F6xxx:

| Class | Update | Class | Update | Class | Update |
|---|---|---|---|---|---|
| 0x70 | 0x2F9760 | 0x1885 | 0x3893E8 | 0x1893 | 0x394068 |
| 0x1192 | 0x3410E8 | 0x1886 | 0x389F58 (Infecto gun) | 0x1894 | 0x396A20 |
| 0x1195 | 0x34DD38 | 0x1887 | 0x38A648 | 0x1896 | 0x398E58 |
| 0x1855 | 0x35D9C0 | 0x1888 | 0x38A7B8 | 0x1898 | 0x39A1D8 |
| 0x186B | 0x373990 | 0x1889 | 0x38D640 | 0x1899 | 0x39B500 |
| 0x186D | 0x376B88 | 0x188A | 0x38E3A0 | 0x189A | 0x39CC78 |
| 0x1872 | 0x381B98 | 0x188B | 0x38EF90 | 0x189B | 0x39DB60 |
| 0x1881 | 0x384338 | 0x188D | 0x38FEF0 | 0x1ADA | 0x3BD298 |
| 0x1882 | 0x385948 (Shock Cannon gun) | 0x188F | 0x390EE0 | 0x1C65 | 0x3717A8 |
| 0x1883 | 0x388920 | 0x1890 | 0x393690 | 0x1CB8 | 0x3D6740 |
| 0x1892 | 0x3619C8 | | | 0x1DCE | 0x3DB4D0 |
| | | | | 0x1DF6 | 0x3DD558 |

A particle type's texture is at 0x221118 + 4 × type (0x17 → 0x221174, 0x22 → 0x2211A0, 0x35
→ 0x2211EC, 0x5B → 0x221284, 0x5C → 0x221288, 0x61 → 0x22129C, 0x68 → 0x2212B8, 0x69 →
0x2212BC).

#### The dome drawn by code (`fx_dome` 0x409A50) [code]

Its arguments are a scroll, a matrix, a texture, a colour, a ring count and a mode. It is a unit
ball from its pole (+z): rings every 18° (radius sin, height cos), 10 quads round, u
alternating by column and v falling 1 a ring plus the scroll. The Shock Blaster's and Shock
Cannon's lances build its matrix with 0x40C658 from (0, −elevation − 90°, heading) of their
way, which puts its +z against that way, its x up and its y level for a level way [code,
2026-11-04]: so their poles are at the gun. The colour fades to the last ring
(mode 0 blended with GS alpha 0x44, its alpha going to 0; otherwise added with 0x48, its RGB
going to 0).

#### The explosions' dome (class 0x70) [code]

0x2F95A0 (factor, life, place, turn, life, r, g, b, opacity) spawns the class, a unit ball at
its class's scale (0.0416, read from the disc), at the class's scale × factor, turned at random
−π to π on all three axes. Its update (0x2F9760): scale × (life − timer) / life, the timer
starting at life − 4 and ending the moby at 0, so it starts at 4/life of its size; each axis
turns 0.0524 rad a frame; its opacity holds, then falls linearly over the last half of its life.

Callers: the small blast (0x4488B8: factor 4 S, life 20, tint (0x7F, 0x40, 0), opacity 0x30;
3 S, life 29, tint (0x60, 0x20, 0), opacity 0x20), `explosion_spawn` (0x330C80: 3.5 S, life 16,
colour and opacity from its +0x5C, raised 8 S / 60, when flag +0x1C is set) and pieces
(0x447D00: with a moby, factor 4 three times and 2 once, "Flux Rifle", "Its landing from
V4"). The Megaturret take's olive dome is very likely its small missiles' burst [not yet
compared].

#### The Dread blast (0x390EE0, spawned by 0x3914C0, class 0x188F) [code]

The Agents of Dread's end at V5.

- **Its radius r** (+0x10) grows 2/3 u a frame (state 1 for 20 frames, state 2 for 40); in state
  6 the speed falls 1/60 a frame to 1/12 while flames last. Opacity +0x24 starts at 0x6F and
  falls 5 a frame; opacity +0x1C holds 0x7F for 20 frames, then falls to 0 over 40.
- **At the start:** the small blast at S 2 (radius 20), a shake of 0.45 for 40 frames within
  100 u, the flames, and 0x391648: seven pieces of debris (0x329470(1.6, 14.3, ..., life
  60–90)), not read further, again at timer 0x14.
- **Drawn by 0x3919E0:**
  - 0x391AC0: a ring from data (positions 0x2B6BF0, faces 0x2B70F0, UV 0x2B74F0), 64 quads, r
    0.407–0.469, z ±0.062, at scale (2r, 2r, r/4), 0.5 up, texture 43 added, (0x80, 0x40, 0x20),
    alpha +0x24.
  - 0x391D08: the same at (r, r, r/8).
  - 0x391938: the whole screen white at +0x24.
  - 0x391F48: a column from data (0x2B77A0, 0x2B8480, UV 0x2B9200), 215 quads, radius 0.198 ×
    (2 + cos height), height 0.155 × 0.865 r, texture 50 white added, its foot unseen, v scrolling
    −0.04.
  - 0x3923A0: a ball from data (0x2B9900, 0x2BA160, UV 0x2BAA60), scale (r/16 + 1, r/16 + 1, 1),
    raised 0.15 r plus a bump, texture 50, v scrolling +0.04.
  - 0x392A48 → 0x393258 → 0x3933C8: flames, texture 46, added, (0x80, 0x30, 0x10) going to grey
    0–13, spinning ±18–20°, alpha 0x5F–0x7F. A ring of 48 at 0.15–0.75 u (half 0.25–0.35 →
    0.75–1.0, life 100–120) and flames on the ball's visible quads (half 0.25–0.5 → 1.0–1.5, 4–5
    u/s); all fade over 30–45 frames once state 2 has passed 2 frames.

#### The Shock Cannon at V5 (0x385948; rays class 0x1883) [code]

- **Numbers** (data 0x1DDDF0–0x1DDE58): 2 and 8-frame counters, spoke length 4, widths 0.1 and
  0.5, ring sizes 1.5→20, turns 2° and −2.5°, a 3-frame flash, 32, texture 23.
- **Each shot** (draws 0x387BF0, 0x3871E8, 0x3873F8, 0x388490): 40 dust of 0x17 for two frames
  (half 0.3→0.8, 10 u/s level, grey 0x70 at 0x40, life 10–30); 8 spokes of texture 26 for 2
  frames, (0x20, 0x40, 0xFF), alpha 255 × n/2; two sprites of 61, (0x80, 0x80, 0xFF), alpha
  20 t, flying 16 u out and growing from half 1.5 to 20.
- **The uncharged lance:** 20 frames, a dome of 4 rings of texture 23 (0x80, 0x80, 0xFF) at alpha
  5 t, and one of 2 rings of texture 11 (0x30, 5, 0xBF) at alpha 3 t, 1.25 times as wide; its
  centre 5 ahead, 5 long, its widths growing 0.5 and 0.3 a frame for 4, then falling 0.3 and
  0.2. Corrected 2026-11-04 [code; seen on UYA NTSC, `review-shock-charged`]: its pole is at
  the gun and it opens out along the aim, as `fx_dome`'s callers turn its axis against their
  way (below). The 0.5 is level (0x1DDE50) and the 0.3 up (0x1DDE54); they pass through
  nothing and grow again the other way, which looks the same, to 1.8 and 1.0 as it fades.
  The outer dome's rows are scaled by those widths × 1.25 after the inner's scaling, so its
  widths are the inner's squared × 1.25 (to 11.25 level), its length still 5.
- **Charging** (each frame held, 0x385948) [code, 2026-11-04]: under 79 frames, two 0x61
  sprites at the gun's point 0x1DDE60 ((0.2, −0.83, 0) in its frame), size 0.1 to 0.55, in
  (0x20, 0x80 + n, 0xFF − n) for n frames held at alpha n/3, life 10–20, spin −1 to −3, in
  one of 0x68's first four textures (0x2212B8). From 79, two at 0.4 to 1.05 in (0x50, 0x50,
  0x96) at 0x40 and two up to 0.2 off it in (0x96, 0x50, 0x50) in 0x61's entries 5–7, a
  0x6F glare as the Hydra's (0x46FE70 from a point 0.05–0.15 off), and half the frames a
  0x2C spark (0x466F98, not read further). Not drawn in the kit.
- **Let go charged** (after more than 78 frames) [code, 2026-11-04]: no ball at the gun and
  no rays (`weapons.md`, "Shock Blaster"); 10 rounds, sound 0 (4 uncharged), the rumble
  0x2B3C10 either way. Its timers: the lance (0x1E1F24) 30 and the warp (0x1E1F20) 27
  (0x1DDE2C), where an uncharged shot sets the uncharged lance's (0x1E1F3C) 20 and the
  warp's 6. The spokes, flash and dust are as an uncharged shot's.
- **The charged lance** for those 30 frames, from the frame let go, all following the aim
  each frame, so swept as he turns:
  - **Its beam** (0x3884E0 → `fx_beam_rings` 0x409448, its numbers at 0x2B3C90): from the
    gun's point 0 to 40 u along the aim (0x1A52D0; 0x1E1F50 to 0x1E1F60), queued both before
    and after the timer steps from its second frame, and the queue (0x4026A0) keeps both, so
    drawn twice. A camera-facing strip of texture 69, the texture once every 1.2 u along it
    (+0x30) and once across it. Its half width is 1 (+0x14) times a fade: 0 to 1.25 over 10
    frames (+0x00, +0x08), then 1, and from its 20th frame (its mode, +0x10, set 2 at the
    timer's 10) 1 falling to 0 over 10 (+0x04); half that at the gun, widening to it over
    0.55 u (+0x18, +0x1C); in pieces of at most 8.4 u (+0x30 × +0x34). Within 80 u of the
    camera (+0x38, +0x3C) each piece is three strips, each drawn twice: first the texture's
    alpha written into the frame's alpha (ALPHA 0x6A: Cd, colour unchanged), then the
    texture scrolled another way added through it (0x58: Cs·Ad + Cd), so its light is the
    texture times itself drifting. The middle 60% across in (0x20, 0x40, 0x80) at 0x7F,
    its two copies scrolling 0.07 and 0.08 a frame along (+0x24, +0x20); each side, from
    its edge to 0.2 of the half width past the middle, in (0x20, 0x40, 0xFF) at 0x7F,
    scrolling 0.042 and 0.016 (× +0x2C, +0x28). Farther off, one plain added strip.
  - **Its domes** (0x3873F8): at the gun's point 0 along his facing (the hero's +0xC0; the
    camera's in first person), centred 14.5 u out and 15 long (0x1DDE40), so the pole half a
    unit behind the gun and opening out along the lance; the game's take shows them as a
    tall pale violet oval round him round a white-blue column. `fx_dome` added:
    texture 23 in 8 rings, its widths turned 45° about its axis, (0x20, 0x40, 0x80),
    scrolling 0.15 a frame; turned −45°, the same in (0x40, 0x10, 0x80) scrolling 0.1 and
    (0x10, 0x40, 0x80) scrolling 0.05; then 1.1 times as wide, texture 10 in 6 rings, (0x40,
    0x0A, 0xD2), scrolling with the first. Alpha 2n for the first three and n for the
    last, n the timer after the frame's step (29 down to 0). Its widths start at 1, grow 0.1
    a frame while n is 16 or more, then fall 0.075 one way (0x1DDE48, up before the turn)
    and 0.125 the other (0x1DDE44, level): 1.1 to 2.4 over its first 14 frames, then to 1.2
    and 0.4. Which way the turns go isn't checked.
  - **Its hits** (`weapons.md`, "Shock Blaster").
  - **Sprites** each frame: five 0x61 at random points along the beam, 0.8 to 1.5, (0x20,
    0x40, 0x80) at 0x50, life 10–18, and one at the gun, 0.3 to 1.0; in 0x68's first four
    textures.
- **The flash on the lens** [code, seen on UYA NTSC]: for 3 frames, a sprite of texture 11 half
  0.04 wide, 0.1 u in front of the camera on its way to the muzzle (+0x40 × +0x44 × 255 of alpha,
  grey 0x80), so a soft white ball about 0.4 of the view's height across that hides the hero;
  and the whole screen added at FIX 32 × that share.
- **Its rays** (0x388920, drawn 0x388DD8): texture 23 (0x20, 0x40, 0x80), alpha 96 t, blended;
  texture 44 (0, 0x40, 0xFF) added; end sparkles (0x20, 0x20, 0x80). Shake 0.06. Each frame
  two 0x5A glares at its end (0x46C468, 0x35's texture, (0x20, 0x20, 0x80)) and two pairs of
  0x5B sparkles (entry 1) anywhere along it, ±0.04, white at 0x30, 1.8 − 0.25 × its pitch in
  degrees across, a fifth of that at the end, life 2–9 by where along it; so the kit's orange
  embers (0x2C) at its end were wrong [code, seen on UYA NTSC: the game's plume is white and
  blue]. The ray goes
  from the gun 2-ish u out toward the target along the first of its tests (0x386FB0: a line test
  (0x3F8448), then the moby's distance sets damage: under 10 u all of it, under 14 ¾, under 20
  ½, else ⅛).
- **The screen warp** (0x388490 → 0x388508, on 0x402768's list of four, each frame its timer
  runs: 6 frames for an uncharged shot, 27 charged) [code, 2026-11-04]: texture 66, a quarter
  of a ring (alpha high at its (0, 0) corner, none at (1, 1)), drawn in four mirrored
  quarters meeting at the screen's centre into the frame's alpha only (TEST 0x513F1: alpha
  test NEVER, FB_ONLY; ALPHA (0 − 0)·FIX + Cd), at grey 0x7F and alpha 5 × the timer, the
  texture's (0, 0) at the screen's corners and (0xFF, 0xFF) at its centre. Then the frame's
  copy (texture −7, 0x1DDED4) over the whole screen (0x1D0010: 512 × 416) from texel (10, 8)
  to (492, 432) (0x1DDEA4–0x1DDEB0; 236 and 240 for the last two when 0x1D5520 is set), so
  magnified 512 / 482 ≈ 1.06 across, mixed in by that alpha (ALPHA 0x54: (Cs − Cd)·Ad + Cd).
  How the copy's v maps to the screen wasn't read, so whether it is magnified as much
  upright is open. The four floats 0x388490 passes (1, 0, 0, 0.9) aren't read. So for a few
  frames all but the middle of the screen shows the frame pushed out about 6%, most at the
  corners [code; seen on UYA NTSC, `review-shock-charged`: the scenery doubled toward the
  edges, the lance's streaks too, the HUD not]. Texture 66 is full but for a hollow round
  its (1, 1) corner, falling to nothing over the last 0.7 of a side or so [verified on the
  disc's texture]. In the kit (2026-11-04) the `Warp` emit: `fx/warp.rs` draws the frame
  again after the 3D passes, before tone mapping, mixing in its copy magnified 1.062 both
  ways about the centre by the ring's alpha times 5 × the timer / 0x80, the timer falling
  from 6 (27 charged) a tick; the frame then holds the effects, as the game's copy does.

#### The Lava Gun at V5 (0x305858, 0x305CA0) [code]

Width parameter 0.1 where V1's is 0.05; its core texture 69 (0x45); its glow three times the
width, so as wide as V1's, (0x10, 0x30, 0x80) at FIX 0x50. Its nozzle's glows are (0x10, 0x30,
0x7F) at 0x7F half 0.25 and (8, 0x18, 0x3F) at 0x3F half 0.5.

#### The RY3NO's tail and its sun [code]

- From V2 the missile is class 0x1DCE (0x3DAEA8) at 2.5 of its scale. **Its tail:** 4 points,
  each going 0.2 of the way to the one before it a frame; a strip facing the camera, in texture
  43 (0x3DC8D0(0, 0.85, 0.25, ..., 0x1030FF, 4)), half 0.2125 and again doubled, ×0.7 a segment,
  u 0.25 a segment, alpha 0x7F and 0x3F, shown after 4 frames. Its glow is of 44, (0xFF, 0x40,
  0x20).
- **V5** (0x3805B0: `3 < level && first`): after the missiles' 30-frame wait, the first missile
  spawns class 0x1DF6 (0x3DD390) at the rally point, 10 u ahead and 3 up. State 1 lasts 30 frames
  with nothing drawn. State 5 lasts 64 frames: 24 rays (0x3DDB38, drawn 0x3DD9B0) of texture 14
  (0x1DF590), length 4 + 0.06 a frame, each turning 2° about its own random axis, half 0.3, its
  tip ×1.1, (0xFF, 0x80, 0x40) at 0x20 going to (0xFF, 0, 0) at 0, v scrolling −0.018; and four
  added full-screen layers of grey (0x80 − 2 × timer) (0x3DD8E8). State 3: everything in view
  within 60 u (0x44CF68, 4.71 and 1.396 rad) takes the row's damage, and it ends.

#### The Infecto-Bomb's looks at V5 [code]

- **Its blob** (class 0x188A, 0x38EE78): scale set to 0.0165 where its class's is 0.0588, opacity
  0x28.
- **Each frame** (0x38E930): a smoke of 0x17 (half 0.45→0.95, 2.1–3 u/s, alpha 0x20, half added),
  a glob of 0x69 (0.35→0.25, 0.6–2.4 u/s), a bubble of 0x22 (0.35→0.15, 0.6–1.8 u/s), life
  10–30, gravity 24; one in three a drop. All (0x82, 0xA1, 5).
- **Where a ray of it hits ground** (0x38ED30): gas, (0x62, 0xC1, 5) at 0x50, half 0–1.5 → 1–5,
  life 100–160. **On landing:** one dark cloud (0x20, 0x50, 0x30) at 0x30, then class 0x188B.
- **Class 0x188B** (0x38EF90): a strike of 5 u; a ring class 0x307 (scale 3.5–4.5, growing 1.05,
  tint (0x52, 0xD1, 5)) three times, at timers 69, 65 and 60; each frame two bubbles (tint
  0x10D162…) and one gas cloud ((0x52, 0xD1, 5) at 0x50, half 0–1.5 → 3–5, within 2.5 u, life
  100–160).
- **Class 0x1899** (0x39B500): the smaller blobs it splits into (ring 2–2.5, growing 1.04).

#### The Suck Cannon's vortex (0x363E98, drawn by 0x3633F8 and 0x3650A0) [code]

The suck's update (0x3619C8) calls 0x363E98 each frame the gun sucks; 0x363C60 starts it
straight along the aim the first time.
- **Its rope:** ten points a step of 1 u (0x1E1B64) apart from the muzzle. What it reaches
  for wanders: every 60 frames a new point 0.1 u off the aim at a heading turned a random
  45–315°, reached linearly over the 60. Each piece turns towards the way of the piece
  before it on a spring (0x3F1438: gain 0.06, damping 0.2, never past the angle), and any
  lag over 0.314 rad is taken up at once; pieces past the fifth keep a tenth of their old way
  per piece.
- **Its rings:** twenty points round each (the unit circle at 0x2B37B0, starting 18° round),
  radius 0.2 × ring + 0.1 + 0.1 sin(pulse + 0.7 × ring), the pulse going 0.1 a frame.
- **How far it shows:** a counter per ring (0x1E1B90): the first grows 4 a frame and each
  ring is 32 behind the one before. Where a line test along the aim meets the world, the
  rings past it fall 4 a frame to nothing and those before stay 32 apart.
- **The tube** (0x3633F8): quads between the rings, only those turned from the camera, in
  texture 21 (0x1DD7B0), added; grey 0x40 to V2, (0x80, 0x60, 0x40) from V3; each ring's
  alpha its counter held to 0–0x20, the last ring's 0. u is 0.3 a ring, scrolling 0.001 a
  frame; v 0.05 a point, scrolling 0.006 a frame for each ring.
- **The arcs** (0x365328 → 0x3650A0): round each ring whose counter is past 0x20, a strip of
  21 pairs on a ball of radius 0.17 × ring + 0.35, from 15° to 35° up towards the rope's
  way, spanning a random 40–150° and turned on a random 17–25° a frame; texture 29, (0x40,
  0x40, 0x60) at 0x20, added.
- **What flows to the gun:** up to 200 sprites, five new a frame at the furthest ring fully
  shown, each following a point of the rings back to the gun (between points rand(0.1,
  0.3) a frame, along rand(0.1, 0.4) u a frame), turning ±0.147 rad a frame; the last piece
  straight at the gun, shrinking 0.95 a frame. Smoke of 0x17, half rand(90000, 270000) /
  210000, one in four blended, grey 0x7F to V2, (0x80, 0x60, 0x40) from V3. One in ten past
  the second ring is dust, twice as far out (rand(0.5, 0.9) along, rand(0.2, 0.5) between):
  0x61's entry 3 grey at 0x2F, half 0.09–0.21, to V2; from V3 entries 10–12 in (0x38, 0x35,
  0x27), blended, half 0.13–0.25.
- **Not drawn in the kit:** six refractions (0x3630E0, texture 0x36) once it has run 30
  frames.

#### The shared explosion (`explosion_spawn` 0x32FE90)

- **Arguments** [code, from the disassembly of every call above]: a size S; the
  position; a shake flag; counts of four kinds of 0x61 and of 0x17 smoke, 0x5A glares
  and debris; flags for a refraction, a dome (class 0x70), a full-screen flash, a glow, a
  ground ring and a camera ring; nine colours (set A for the Annihilator and the Flux
  at V4, its own set for the N60 and the Flux at V2 and V3).
- **Where each goes** [code, 0x32FE90's stores and 0x330C80's reads, 2026-11-03]: S in
  f12; the position a0, the shake a1; the counts a2 (the 0x61 and 0x35 pairs), a3 and
  t0 (the two other 0x61 kinds), t1 smoke, t2 glares, t3 debris; then on the stack
  sp+0 the refraction (+0x18), sp+8 the dome (+0x1C: 3.5 S for 16 frames in the eighth
  colour), sp+0x10 the flash (+0x1A), sp+0x18 the glow (+0x1E), sp+0x20 the ground ring
  (+0x16), sp+0x28 the camera ring (+0x3C), sp+0x30 and sp+0x38 not read, the nine colours
  sp+0x40 to sp+0x80, and sp+0x88 the sparse ring (+0x36: 34 of 100).
- **At the spawn** (0x330C80): six 0x55 of 3S growing to 4.5S over 30 frames in 0x5C's
  textures; pairs of 0x61 (0.6–1.0 S and six times that in 0x35's texture) and two more
  kinds, thrown toward the camera at 9–12 S units a second within 80°, up to 12 S up,
  gravity 14.6, shrinking to 0.01 S over 30–50 frames.
- **Then** (by its 60-frame timer): glares on the 2nd frame (15–20 S units a second, life
  25–40); smoke on the 3rd (1.5 S, life 40–70); debris on the 4th; the camera ring on
  the 5th and the ground ring on the 6th (100 0x61, or 34 when sparse, sliding out
  level at 15 S units a second, 1.1 S, life 20–27). The flash shows on the first frame
  only (0x3317C8: grey 0x80 added at alpha 0x80, as the Nitro's [code]); the glow is off
  at every call above.
- **Only in view** [code, 0x32FE90 and 0x3317C8; seen on UYA NTSC]:
  - **The test:** at the spawn, 0x4562A0 tests a ball of 3S round the burst against the
    view frustum (0x40D0C0, VU0: −1 outside, 0 across an edge, 1 wholly inside).
  - **On 1:** pvar +0x14 is set to 10. The flash (+0x1A, the 15th argument) and the glow
    (+0x1E) are drawn only while it is set.
  - **Otherwise:** the refraction (+0x18, the 13th argument; texture 0x3B, 2S to 4S over
    10 frames) is turned off as well. On 1 it is kept only while 90° less the angle
    between the camera's vector at 0x1E2E40 (not read) and the way from the camera to
    the burst is under 26° (0.4538).
  - **On screen:** the Bouncer's flash whites out the frame, two adds of 0x80.
- **At each call:** the N60 at V1 (S 0.14; counts 5, 2, 2, 2; dome; a sparse ring); the
  N90's bullets (S 0.17; counts 1, 1, 1, 1, one glare; dome; red: (0xFF, 0x40, 0x20),
  (0xFF, 0x63, 0), (0xFF, 0x7F, 0x30), (0xFF, 0x20, 0), (0xFF, 0x8F, 0), (0xFF, 0x64,
  0x20), smoke (0xFF, 0x30, 0x30), dome (0xFF, 0, 0) at 0x28, ring (0xFF, 0x10, 0x10)); the
  Annihilator (S 0.6, then 1.0; 25, 20, 20, 20; 45 glares, 35 at V2; 2 debris; dome,
  flash, ring; shake); its mines (S 0.6, 1.0 heavy); the Decimator's seekers (S 0.4); the
  Eruptor's bullets (S 0.2, only while the Agents of Doom's level byte is under 2); the
  whip's V4 wave (S 0.5); the Bouncer (S 0.7 and 1.0, flash) and its bomblets (S 0.6);
  the Disc Blade's disc (S 0.3 to V3, 0.5 from V4; "Disc Blade Gun").

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
     mound of goo 1.4 units high, not the shot). The
     canister is drawn at three times its class's scale, as the Bouncer's code sets it
     (0x1DC8BC [data]; checked in the app 2026-10-28).
   - Done (2026-10-29) for the whip's links, 0x183A and the tip 0x183C, at 0.2 of their
     classes' scale (`UYA_ROPE_CLASSES`, given to `rac_rig::RopeModels`), their texture's
     sampler repeating since their coordinates run below 0 (`weapons.md`, "Plasma
     Whip"). The rope now starts each piece at the link before's joint 0, 0.194 u on,
     which it had left out. Seen on screen at V3 (`-whip.orin`): small translucent blue
     lozenges along the strands, lying flat in the swings and standing on edge at rest.
     Their texture (Veldin's 49) looked like four repeated strips until UYA's textures
     kept in EE memory were read in rows (`disc-layout.md`, "Level textures"); read so,
     it is an atlas, and the links take its silver chevron strip. In the game the links,
     being mobys, are drawn before the effects, so the rope's bright strands lie over
     them and they hardly show; sorted by distance alone, the kit drew those nearer the
     camera than the hand after the strands and darkened them into chunks. Their
     materials now sort 12 u farther off (`rac_rig`'s `LINK_SORT_BIAS`). That wasn't
     all: the owner still saw dark chunks (2026-11-01, in a throw's curl, where links
     lie off the strands). Below V5 the links' spawn sets mode bit 0x200, libuya's
     `MOBY_MODE_BIT_DRAW_TRANSPARENT_WEIRD`, which V5 clears with their colour; read as
     an added draw, the kit now adds them below V5, so they only brighten.
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
     takes a game's shot models.
   - Extended (2026-10-24) for the six first guns' calls: parts by the gun's level
     (`from_level`), random rolls and spins, end alphas and end colours, random start
     sizes, drag, delays and spreads over ticks, bursts toward the camera or level along
     the ground, particles drawn as streaks along their way, sprites that fly along the
     aim, layered spinning sprites on shots, tracers behind them, trails laid by count
     or by distance with drift, and beams along each tick of a shot's way and at each
     hitscan ray (`ray`, `ray_end`, `wake`).
   - Still to do: strips and ribbons with moving ends (the Shock Blaster's ray tails,
     the Flux's helix; the Lava Gun's since done, below), domes and rings drawn by
     strips (`fx_ring_fan`), discs and refraction, rumble, glows over the view, decals other than the Lava Gun's pools (the whip's scorches, below), meshes
     the effects spawn other than the ring (the explosion's dome and debris), and children
     particles.
   - Extended (2026-10-31) for the particle renderer: a glow's, burst's or trail's
     sprites in a particle texture (which only that renderer draws) are drawn with `size`
     corner to corner (`PARTICLE_ACROSS`), as the game draws its particles ("Particles"),
     which makes every such sprite 1/√2 as wide as before; and a glow or burst can
     `flash`, following the two-phase curve of types 0x35, 0x4E and 0x5C (`Fx::look`).
     They flash where the game's type is 0x35 or 0x4E: the Qwack-O-Ray's sparks, the
     growing sparkles of the Nitro, Annihilator, Flux Rifle and Disc Blade Gun, the
     Plasma Coil's and the Miniturret's hit sparkles, the lightning bolt's, the Shield
     Charger's and an appearing agent's (a trail can `flash` too). Not the ones only in
     0x35's or 0x5C's texture: the shared explosion's (types 0x55, 0x61, 0x5A, 0x17),
     the Shock Blaster's, N60's and Rift Inducer's firing sparkles (0x55), and the
     orange 0x0F puffs behind the Disc Blade, rift and Suck Cannon shots. The 0x5C glows stay even: whether each spawner
     sets 0x5C's phase isn't read. The six first guns' check on screen (2026-10-28) was
     made before this.
   - Extended (2026-10-29) for decals and rings. Gun events' `Impact` carries the
     normal of the world's surface where a shot ended on it (`surface`; given so far by
     bullets and streams), and recipes leave marks there (`mark`: glows lying on the
     surface and flames standing on it, one time in so many while fewer than so many of
     the gun's are shown, their strength on the game's spring: the Lava Gun's pools).
     Models the game spawns, by class number (`Emit::Model`, scale, grow and fade a
     tick), which a game gives the kit (`FxModels`; the app reads class 0x307 from
     Veldin's core) and the kit draws as its own crown without one: the ring of the
     Infector's burst and the whip's wave. Particles and trails can lie on the floor
     within so far below (`ground`), or not show where there's none, as the game puts
     its gas clouds.
   - Extended (2026-10-28) with the full-screen flashes (`Screen`): an untextured
     additive quad kept just in front of the camera, for the Shock Blaster's shots, the
     Nitro's blast and the Annihilator's explosions. Seen on screen for the Annihilator;
     the others last 2 and 3 ticks, too short to catch at the software renderer's 8 or so
     frames a second. They brighten in linear light, where the GS adds in the
     framebuffer's, so they are dimmer than the game's over all but black, most where
     two stack (the Bouncer's white out the game's frame; the kit's lift a dark sky to
     about 0.65). Extended (2026-10-31): the shared explosion's show only when a ball of
     3S round it is wholly in view (`seen`), as in the game.
   - Extended (2026-10-28) for the Bouncer, Plasma Coil and Plasma Whip: each gun event
     names the kind of shot that sent it (`GunEvent::shot`, by `ShotKind::name`), and a
     recipe can give parts for each kind (`shots`), so the Bouncer's bomblets and the
     Annihilator's mines show their own explosion and lay no trail, and a turret's shots
     can differ from its glove's throw; a bomblet bursts at the end of its life too, as
     0x31E488 does; each gun can give its own arcs and what shows where they end (`arc`,
     `arc_end`; the default's lightning for the rest); bursts can rise on top of their
     speed (`rise`); trails can stop at an age (`until`). Checked on screen with the
     disc: the Bouncer's canister, burst and bomblets, and the Plasma Coil's ball in
     flight; the coil's arcs and the whip's V4 wave weren't caught (no enemy near the
     ball; the demo's guns are V3).
   - Extended (2026-10-28) with ribbons through a gun's shots of a kind (`ribbon`): a
     mesh rebuilt each frame through the shots' points, newest first, facing the camera,
     narrowing a share a point, one quad between each two points; from the nozzle while
     the newest point is fresh. Drawn for the Lava Gun's stream, and seen on screen with
     the disc: one thin bright ribbon with an orange glow, as in the video. Its texture
     doesn't scroll, and it starts at the kit's muzzle, which sits above and behind the
     disc model's barrel tip. Strips through points that aren't shots: the whip's rope
     since 2026-10-29 (below); the Shock Blaster's ray tails and the Flux's helix are
     still to do.
   - Extended (2026-10-29) for the Plasma Whip's rope (`rac_demo::fx::rope`): the rig
     gives where the rope lies each frame (`HeroRig::rope`, the hand and each piece's
     end, and whether he attacks), and recipes draw strands through it (`rope`: widths
     picked each frame or as a share of the strand before, jitter, a fade over the last
     piece, a texture scrolling along each piece, each piece split where its u wraps)
     and lay particles at random points of its pieces each tick (`along_rope`, while
     attacking or at rest).
   - Extended (2026-10-29) for the Spitting Hydra's spit (`rac_demo::fx::fan`): strands
     drawn afresh each frame from a shot to each of its ends along the game's curve
     (`fan`: the bow point's distance and roll by end, the point past the end, the ends'
     stagger), sharing the rope's strands.
   - Extended (2026-10-28) for the lightning bolt: beams can bend through points pushed
     aside by a wave, a turn round the line and jitter (`jag`), or along the coil's cubic
     with two random bows (`bows`), drawn afresh each tick; a beam can follow the bends of
     the one before it (`same_bends`), as a glow round its core;
     the defaults can give parts for a kind of shot (`shots`), which come before the
     gun's own, so the four guns' bolts share one recipe; a bolt strikes 6 ticks into
     its life and sends an Impact there, as 0x34DFD0 does, where the kit had it strike
     at the end; a holo shield sends an Impact as it lands, and is drawn by its sprites
     while it flies and as the kit's wall once it stands.
   - Extended (2026-10-28) for the Disc Blade: a shot that glances off something and
     flies on sends a `Bounce` (its own recipe part, `bounce`, and sound cue), the disc
     at each wall and from V3 each enemy, and an Impact at its end wherever that comes;
     flat ribbons through where a shot has been (`sweep`: a point kept every so many
     ticks, its sides along one level line, fading back as 0x4068C8 does); glows pushed
     toward the camera (`nearer`), as the game keeps sparks off the wall they are on.
   - Checked on screen (2026-10-28): the six first guns at V3 in an off-screen run with
     the disc, against PCSX2 video of the same guns at V3 on Veldin (`docs/re-setup.md`,
     "UYA NTSC-U gun effects video"). Fixed: every sprite cast a shadow, as Bevy's
     shadow pass takes blended meshes too (the effects and the kit's shot shapes are now
     `NotShadowCaster`); the kit's sphere of the Nitro's reach hid the game's blast (a
     recipe can now say `blast_sphere: Some(false)`); the Lava Gun's smokes were all
     blended. Close to the game's: the Shock Blaster's fan of rays and its muzzle glow,
     the Nitro's implosion rings and orange burst, the N60's flames, the Annihilator's
     smoke and explosion and the Flux's beam. Still short of it: the full-screen flashes
     (the game's Nitro whitens the whole screen for a frame; since drawn, above), the
     Lava Gun's stream,
     which the game draws as one thin bright ribbon and the kit as sprites on its points,
     and the smoke's rising once dark. In the kit the stream fires level and its points
     reach about 10 u before they meet the floor (measured in `rac_combat`'s test yard),
     so seen from behind the hero its smoke stacks into one cloud; in the video the
     game's arcs high. Not a lock-on, as first thought: the stream leaves along the gun's
     barrel, 12° above his facing (`weapons.md`, "Lava Gun", "The barrel"), which the
     kit follows since 2026-11-04. The video runs at about a fifth of real speed, so timings and how much
     smoke builds up can't be compared from it.
   - **Colours** (open, 2026-11-03, worked out from the Tempest's footage against the
     kit's). The GS adds a layer's colour × alpha into the frame buffer's gamma-encoded
     bytes. The kit treats a recipe's colour as sRGB, turns it linear (`Color::srgba`),
     and Bevy adds linear colour × alpha in linear light. Over black, the two agree only
     at alpha 1: the kit's layer is lin(c) × a where the game's would be lin(c × a), about
     lin(c) × a^2.2, so at a < 1 it is 1/a^1.2 too bright. That is 1.8× for the Hydra's arc
     cores (0x4F) and 6.7× for their blue glow (0x1A); channels over 1 (0xFF is 1.99) also
     grow on sRGB's curve, 1.99 to 4.9. Over a bright floor the error runs the other way: a
     mid grey added to a mid grey gives white in the game and 0.69 in the kit. So the kit's
     glows look bluer and stronger over the dark sky and paler over the sand. Scaling the
     alpha alone only fixes the case over black. Matching both needs the effects added in
     gamma space, for example drawn into a non-sRGB target after the scene's tone mapping,
     a change to the renderer for the owner to decide (`--plain-colours` only changes the
     tone mapping).
4. **Recipes per gun:** each gun's calls with their arguments (texture or particle type,
   colour, sizes, life, blend), read gun by gun, starting with the six first guns. Each
   gun's `effects` key in `uya_weapons.ron` names its recipe and sounds.
   - Done (2026-10-24) for the six first guns and the shared explosion at each of their
     calls ("The calls' arguments"): the parts the layer can draw take the game's
     numbers, full widths being twice the game's sizes. Where the layer falls short the
     recipe says so, or leaves the part out (see the list above). The Annihilator's
     mines show the bomblets' explosion (2026-10-28), and its missile's glow shows only
     without the disc's model.
   - Done (2026-10-28) for the Bouncer, Plasma Coil and Plasma Whip, as far as the layer
     goes. The Bouncer: its muzzle smoke, the canister's puffs, the explosion twice
     where it bursts (S 0.7 and 1.0, each with its flash) and once per bomblet (S 0.6,
     1.2 heavy); not its light. The coil: its ball's four glows, the sparkles where it
     sticks (its mode 1, an enemy's, for any stick) and arcs of 43 with glows and
     sparkles where they arrive; not the lightning round the ball, the glows and arcs on
     the gun, nor the burst where the ball ends. Since (2026-10-28) five arcs to each
     target, bowed on the game's curve (0x30D500) through 24 points jittered 0.075, each a
     grey core with its blue glow on the same points; still and whole from the start,
     where the game's reach over 10 frames and waver each frame, and with no per-frame
     widths. Seen on screen with the disc (the arena replay in `docs/re-setup.md`): a
     ball stuck to a charger, burst, and its five arcs reached a shooter with their glows
     and sparkles. The whip: its V4 wave's blasts (S 0.5, blue), and since 2026-10-29
     its rope's bands and strips by level and the sparkles, smoke and sparks along it
     (V5's sparkles and smoke drawn as V4's), seen on screen at V3 with the disc (the
     arena replay `-whip.orin`): a blue band and glow hanging from his hand at rest, and
     strands with a sheath of sparkles following the rope out in each swing. Since
     2026-11-01 the sparks where it hits: a whip strike sends an impact where the rope's
     sweep came nearest what it struck (`StrikeShape::nearest`), and the recipe throws
     its 30 sparks there, once a strike where the game sprays them each frame a link
     meets something. Not drawn: the scorches where it hits and its light. The wave's
     ring since drawn (below), its blast now under the wave's own shots; the links from
     classes 0x183A and 0x183C (below).
   - Done (2026-10-28) for the Miniturret Glove, its parts by the kind of shot (`shots`):
     nothing for the glove's throw; for the turret's shots its muzzle's glows and flames
     (each flame drawn untapered, at its mean width), for its pellets the tracer's glow
     (not its thin core) and their sparkles (6 pairs, as on an enemy), and where it ends
     the small blast, its fireballs drawn as three waves (the splits at 85% of each life)
     fading over three times their life; orange to V2, blue from V3. Not drawn: the
     blast's domes and light, the water landing. Seen on screen with the disc (2026-10-28,
     the arena replay): the disc's turret on the ground firing blue V3 tracers at a
     charger, with sparkle bursts where they hit.
   - Done (2026-10-28) for the Agents of Doom, by the kind of shot: nothing for the
     glove's throw; the V1 agent's explosion with its ground ring; the comet's head of
     puffs and sparks in flight and its explosion, sparks and glows where it lands (the
     glows camera-facing, the game's face its way); the small bullet's glows, streak and
     embers by level, and its explosion under V3 and embers from V3. Not drawn: the
     agents' sparkles as they appear and their guns' flashes (the kit sends no event for
     either; a firing event from a shot would play the glove's throw sound, as the
     turret's shots already do), the explosions' domes, debris and distortion, the
     second explosion. Seen on screen with the disc (2026-10-28, the arena replay): the
     agents' bullets and comets with their trails of puffs, and explosions throwing the
     crates' pieces. Since 2026-11-01 the comet is its puffs and sparks alone; the kit had
     also drawn its own ball there, which the game doesn't (it scales the class to 0.05).
   - Done (2026-10-28) for the lightning bolt the Spitting Hydra, Holo Shield, Shield
     Charger and Rift Inducer share: each tick its two bent lines as cores of 23 and
     glows of 14, blue (purple for the rift), and from its strike its sparkles, spread
     over the 9 ticks the game makes them in. Its second line is drawn only to the
     target, its ends pinned to the line and with no slow swell (the sin(b) factor), the
     bolt at its whole length from the first tick, no u scroll and no fade at its ends.
     Checked by the recipe test, not on screen: in the arena replays the Hydra spat but
     never marked a target, though the enemies were within its 32 u (why not is open).
   - Done (2026-10-28) for the Holo Shield in flight and unfolding: five of its six
     billboards (the sixth has alpha 0) and the blue puffs where it lands, all at once
     spread over its 34 frames of unfolding, at the landing point rather than rising
     with it. Its standing wall since (2026-10-28) as the layer's panels (`panels`:
     sprites in a standing shot's plane, each turning in a frame stretched to its size,
     unfolding over 34 ticks and folding with it, faded near the camera), in place of
     the kit's flat wall, and the kit's wall grown to the size drawn (half width 2,
     height 2.3); each masked strip drawn as its second texture alone (no mask), each
     layer turning on its own phase, and without V3's two camera-facing stars and V5's
     layer. Seen on screen at V3: a level oval of turning purple and blue cloud with
     orange rings. Not drawn: its lights and the game's own folding draws. Seen on screen with the disc
     (2026-10-28): the puffs show as a dark-blue cloud at the wall's foot, as half of
     them are the dark smoke tinted blue; the shield in flight shows as a faint blue
     glow.
   - Done (2026-10-28) for the Disc Blade Gun ("Disc Blade Gun"): the disc's sprites to
     V4, its puffs and sparkles behind it, its trail from V2 (the disc's own point
     faded a step, as the game's), the glances by level and its end's explosion by level
     (S 0.3 to V3, the big blue one at V4, a small red one from V5). Not drawn: the
     sprites' push toward the camera, the explosions' domes, distortion, debris and the
     V4 shake; the trail's side is the first heading turned 60°, not the game's ±2°
     either way. Seen on screen with the disc at V3 (2026-10-28, the arena replay): the
     discs flying with blue glows and sparkles, the trail as the texture's thin streaks
     along their way, glances off the enemies and blue bursts.
   - Done (2026-10-29) for the six guns left, as far as the layer goes (their sections
     above say what isn't drawn): the Infector's spray, blob trail and gas clouds (since
     laid on the ground, below), and its blob as class 0x1E05's
     model at 0.275 of its scale; the Shield Charger's zap sparkles (at the enemy, not on
     the shell); the Rift Inducer's flash, sparkles, flight sprites, trail and closing
     glows (its open look, lens, rings and rays, not drawn: the rift keeps its flight
     look); the Suck Cannon's shot's trail and explosion; the Qwack-O-Ray's beam as two
     flat beams widening to the target (the game's is a funnel of rings), its sparkles,
     and the ducks' and eggs' explosions; the RY3NO's missiles' glows, trails and ends.
     Seen on screen at V3 (2026-10-29, the arena replays): the Infector's green spray at
     the muzzle and its gas clouds building a bright green haze round the blobs, heavier
     than the game's would look since the game's clouds sit on the ground, half hidden;
     the Rift Inducer's orange flash at firing and its blue rift in flight and open,
     pulling a crate apart (the kit's pull draws an `Arc` event, which drew the defaults'
     lightning from rift to crate until the recipe set `arc` and `arc_end` to none); the
     RY3NO's volleys with orange puff trails and fireballs; the Qwack-O-Ray's beam as a
     thin pale blue line for a tick or two with its sparkles at the enemy, which turns
     into a duck in a puff of feathers, and the ducks' and eggs' small blasts.
   - Done (2026-10-29) for the Spitting Hydra's spit ("Spitting Hydra"): its arcs as
     cores and glows on the game's curve by level, the muzzle's rings and glows, and the
     strike's smoke and embers; the Hydra's own arcs and the defaults' lightning turned
     off, its marked targets keeping the shared bolt. Seen on screen at V3 with the disc
     (the arena replay `-hydra.orin`): the rings growing out round him, the arcs bowing
     up and to the sides into the aim, and dark smoke rising where they struck. Not
     drawn: arcs cut short at walls, the bolt it throws ahead as it spits; the rings show
     on a release at marks too. The arcs' jitter and widths, which the game picks again
     each frame at 60 Hz, were picked each frame drawn, so at a higher refresh each shape
     showed for one frame (seen by the owner, 2026-11-01); they now hold for a tick
     (`FxRng::for_tick`), as do the whip's strands, the charge's strips, the streams'
     ribbons' widths and the holo walls' panels' turns. Everything else random is picked
     in the fixed step already, or (the Shield Charger's shell) in whole ticks.
   - Done (2026-10-29) for the Hydra's charge while held ("Spitting Hydra", "While
     held"; `rac_demo::fx::charge`, from `Arsenal::charge_ticks`): the strips into the
     gun and, from V3, out to the world (the kit's own ray from the muzzle, then the
     floor ahead), alpha by the frames held, and the muzzle's glows and light sized by
     the charge. The strips end at point 1 as an offset from the kit's muzzle (0.72 u
     back, 0.165 up; the class's x is taken as up), not the model's posed joint. Seen
     on screen at V3 with the disc (`-hydra-side.orin`, side on): the strips crackling
     up and round the gun, brighter as it is held, the glow swelling at the muzzle and
     thin strips reaching to the floor. Its glare ("The glare"; the recipes' `Glare`) from
     65 ticks held and while the spit lasts (its first 30 ticks, as its glows), seen on
     screen at V3 the same way: thin rays fanned round the muzzle, pinkish in the disc's
     texture 25. Not drawn: the sparkles where a strip meets a wall, and the end kept in
     hero states 7, 0xB, 0xE and 0x14. Over a long hold the blended half of the glows darkens the view round the
     muzzle; whether the game's does is open (a game capture of a hold would say).
   - Done (2026-10-29) for decals and rings: the Lava Gun's burning pools ("Lava Gun") to
     V4 and their blue glows at V5; the Infector's gas clouds on the ground within 5 u
     below the blob and the V3 splash (none where there's no ground in reach), and its
     burst's green ring; the ring of each of the whip's V4 wave blasts. Seen on screen
     at V3 with the disc (`-lava.orin` and `-infector-open.orin`, fired at open floor and
     then run past): pools as orange glows on the floor with thin streaked flames
     standing on them, burning and fading; the gas clouds as low green mounds on the
     floor (not compared with the game's on screen); the ring as a green crown of spray rising and
     widening where a blob bursts, plain only with the clouds turned off (it is faint
     and inside the clouds). The whip's ring wasn't seen: the demo's guns are V3.
   - Done (2026-10-29) for lights and shake. `Emit::Light` is a point light with the
     light moby's envelope (climbing by a rise to its colour, falling by a fall) and a
     shot's recipe can carry one (`light`, only while it flies if `flying`), within the
     game's eight slots; each is drawn with the game's falloff ("How the lights fall
     off", 2026-11-03), where Bevy's point lights, at 8,000 lm per unit of colour per
     square unit of radius, had lit a small spot at the gun. `Emit::Shake` sets the three channels with the game's motion, weakened
     by the distance from the camera and cut off beyond a reach as the game's calls are.
     Recipes: the Annihilator's, RY3NO's, Flux's and Bouncer's muzzles, the Holo Shield
     and Rift in flight and the Hydra's spit (its colour the middle of the game's
     random range); shakes for the Shock Blaster (0.04 on both channels; the kit fires
     no charged shot), the Nitro's blast at every level, the whip's wave and the shared
     explosion where its calls set the flag (the Annihilator's, the Bouncer's canister,
     the Disc Blade's V4 and the ducks'). Seen on screen at V3 with the disc
     (`-flux.orin`, `-annihilator.orin`): the Flux's light warms the floor beside him
     for about a quarter of a second (red +16, green +10 of 255 where nothing else is
     drawn; nothing there with the lights turned off), the Annihilator's as a pale warm
     patch by the gun. The shake wasn't seen: the takes' Nitro blasts land more than
     20 u from the camera and the Annihilator's is 0.075; tests cover its motion. Not
     drawn: the explosion's own light (each call's light argument isn't read), the
     small blast's (whether its callers pass a radius isn't read), the whip's (it
     follows the rope with its own wobble), the Qwack-O-Ray's and the V5 guns' shakes.
   - Done (2026-10-29) for the Shield Charger's shell ("Shield Charger", "The shell"):
     `shell` in a gun's recipe, a dome turned to the camera seen whole straight on and a
     ring round it from the middle of a glow to its edge, in the shield's colour by level,
     flashing to the hit colour whenever the shield's points change and fading back over
     30 frames; it replaces the rig's bubble. Its middle is taken as the shield moby's
     (0.75 u above his feet), not his joint 2. Seen on screen at V3 with the disc
     (`-shield.orin`): a translucent green disc round him with a soft green rim. Its
     arcs and trails (`arcs` and `trails` in `shell`) follow: five loops crackling out
     from the middle and back, each laid anew as its 3–45 frames run out, and four
     trails orbiting it, with their textures, widths and colours by level. The kit lays
     its first step any way at random (the game's from two random angles), takes the
     game's (x, y, z) as its (x, −z, y) for the trails' axes, and steps both in whole
     ticks of the frame's time. Seen on screen at V3 with the disc (`-shield.orin`):
     faint teal zigzags inside the shell and pale curved strokes round its rim. Not
     drawn: the trails' head sparkles, V3's scrolling glow and the 0x3E sparkles on the
     1 u sphere.
5. **Sounds** with P3.13: the gadget WAD's bank per gun and the level's bank for shots,
   by the class's sound table. Done (2026-10-23) for every gun's fire, no ammo, impact
   and loops that the kit cues ("Sounds", above).

## Not read yet

- The particle renderer's byte fields (+3, +9). As values of the GS's ALPHA register,
  0x48 is Cs·As + Cd (added) and 0x44 is (Cs − Cd)·As + Cd (blended), and the Lava Gun's
  smokes take the two in turn as +3 ("Lava Gun"), but the renderer's use of +3 isn't
  read (+9's fades and the size as half the diagonal are, under "Particles"). Also its
  table at 0x2BFDA0, its streak kinds (+1 = 3) and the streaks' widths (+0x1C, +0x2C).
- The updates of the types the guns use beyond those above (0x13, 0x19, 0x5B, 0x6D,
  0x6F, 0x72, 0x74, 0x76, 0x80 read only in passing).
- What the built-in textures −1 to −4 hold.
- 0x4C4A18 and 0x4C4AD8, which every projectile runs (gated by the camera mode and
  0x1A4F48, 0x1D6740, 0x1D673C; perhaps shots placed for the first-person view).
- What the rumble patterns' bytes mean; the light argument at each `explosion_spawn`
  call (class 0x27F, read above); how 0x4A3D80 fades a light over its radius; what the dome (0x70) and debris (0x7A)
  classes look like (the chunk 0x1A50's flight is read, "Nitro Eruptor's burst"); 0x447D00's types 0x0B and 0x08.
- Which way the Shock Blaster's charged rings face (built by VU0 microcode at 0xC80),
  and the screen warp's 0x4A04B0.
- The Nitro's acid arcs (0x39AA18, 0x39ACB0, 0x39B080), the V5 guns' draws (0x3443F0)
  and the Lava Gun's P72 gravity.
- Whether a moby updates in the frame it spawns, which sets whether the Shock Blaster's
  rays show for 14 or 15 frames.

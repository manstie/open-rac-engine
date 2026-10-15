# Mobys: classes, skeletons, animation and meshes

How a moby class (the games' animated objects: the hero, enemies, crates) stores its
skeleton, animations and mesh, for PLAN.md P3.7, and as the reference for the hero kit's
rig (P1.17). Started 2026-10-13 on UYA NTSC-U.

## Status and method

- **Memory image:** UYA NTSC-U (SCUS-97353) on Veldin, `cache/ghidra/ee/uyantsc-veldin.ee.bin`
  (a headless boot, `docs/re-setup.md`). A class in memory has absolute pointers.
- **The disc:** the same classes read from the UYA disc image ("On the disc" below), which
  is how the engine reads them, and the hero's mesh from the armor WAD ("The armor WAD").
- **Code read in Ghidra** (addresses and names in `re/SCUS-97353/symbols.toml`):
  - the hero's animation driver (`hero_anim_set`, `hero_anim_update`);
  - `moby_anim_frame`, which fetches one keyframe, and `anim_frame_unpack`, which
    decompresses it;
  - the generic layered animation update (`moby_anim_layer_update`) and the start of the
    pose builder (`moby_pose_build`).
- **Labels** as in `hero-controller.md`: **[verified]** (read in code and checked against
  the data), **[code]**, **[data]** (seen consistently in the data, not read in code) and
  **[reported]** (from libuya, MIT, unchecked).
- Units: a class's skeleton and vertices are in its own units. One unit is
  `class scale / 1024` world units **[verified]**; the hero's class scale is 0.145833, so
  one unit is 1/7022 u.
- Axes: z up, x forward, y to the hero's **left** **[verified]**: the wrench sits on the
  joint at y = −0.52, and Ratchet holds it in his right hand.

## Finding the hero

- The hero's moby is at hero struct + 0x161C (also at + 0xC18 and + 0x25E0), 0x1D36D00 in
  the Veldin image **[data]**. libuya's `HeroMobys` (hero + 0x424) is empty in this image.
- Things he carries are mobys of their own, placed on his joints each frame **[verified]**:

  | Object | Class (oclass) | Joints | Sequences | On hero joint |
  |---|---|---|---|---|
  | Hero | 0x000 | 111 | 127 | |
  | Wrench | 0x047 | 16 | 20 | 56 (right hand) |
  | Clank (back) | 0x259 | 33 | 17 | 5 (upper back) |
  | Clank (pack form) | 0x25F | 36 | 9 | 5 |
  | Unknown, 2 joints, 7 bangles | 0x50A | 2 | 1 | 7 (neck) |

  Placing the attached mobys' positions on those joints by our own forward kinematics
  (below) matched to 0.0005 u or better.

## On the disc [verified 2026-10-13]

- Classes live in each level's core (`disc-layout.md`, "Level data"), listed by the core
  header's class table. Stored, a class's pointers count from its own start, and the
  game adds the class's address at load (`moby_class_relocate`, 0x424048): the packet
  table and each packet's VIF and vertex pointers, the skeleton, the joint offsets, the
  animated-joint lists and their entries, the GIF usage, the sound definitions, the
  sequence pointers, and each sequence's keyframe pointers. A sequence's trigger pointer
  counts from the sequence instead.
- **The hero is different:** his class has no mesh (its packet table is 0; the armor WAD
  supplies it) and an empty sequence table. His sequences sit elsewhere in the core, listed
  by the core header (+0x78), and each one's keyframe and trigger pointers count from the
  sequence's own start (`hero_sequence_relocate`, 0x423DB8).
- The compressed keyframes' 0xF flag survives relocation: the game adds the address to
  the stored word.

## The class header [reported, checked]

libuya's `MobyClass` layout matches UYA's data:

| Offset | Field |
|---|---|
| 0x00 | Packet table (high LOD first, then low LOD) |
| 0x04, 0x05 | High-LOD and low-LOD packet counts (hero: 60 and 22) |
| 0x06, 0x07 | Metal packet count and start |
| 0x08 | Joint count |
| 0x0C | Sequence count |
| 0x14 | Skeleton: one 0x40-byte record per joint |
| 0x18 | Joint offsets: one 0x10-byte record per joint |
| 0x1C | Animated-joint lists (not needed to play the hero's animations) |
| 0x24 | Scale |
| 0x48 | Sequence pointers, one per sequence (null if unused) |

## Skeleton [verified]

- **Joint offsets** (class + 0x18), 16 bytes per joint: the joint's position relative to
  its parent as three floats, then the parent's address in scratchpad memory,
  0x70000000 + parent × 0x40. Joint 0 is the root and has no parent.
- **Skeleton records** (class + 0x14), 0x40 bytes per joint: a 3×4 matrix whose fourth
  column repeats the offset, then the negated rest position in model space at +0x30
  (an inverse bind with no rotation) and the parent's address again at +0x3C.
- **The rest pose has no rotations:** every joint's rest orientation is the identity, so
  a joint's rest position is the sum of its offsets up the chain.
- **UYA's hero skeleton** (111 joints, positions in world units at the hero's scale):

  | Joints | Part |
  |---|---|
  | 0 | Root, at the feet |
  | 1 | Hips (0.49 up) |
  | 2–5 | Spine (0.52 to 0.80) |
  | 6, 7 | Chest (0.90), neck (0.95) |
  | 8 | Head (1.08) |
  | 9–51 | Face: jaw 10–15, eyes, brows and lids 16–20 and 25–40, cheeks 47 and 48 |
  | 21–24 | Two pairs far in front of the face, probably eye-aim targets |
  | 41–43, 44–46 | Left and right ear (tips at 1.35, 0.36 out, 0.27 back) |
  | 52–56 | Right clavicle, shoulder, elbow, forearm twist, hand (0.52 out at 0.88 in the T-pose) |
  | 57–68, 69–70 | Right fingers (four of three joints) and thumb; 71 an elbow helper |
  | 72–91 | Left arm, the mirror of 52–71 |
  | 92–97 | Right hip, knee, ankle, heel, ball, toe |
  | 98–103 | Left leg |
  | 104–110 | Tail, straight back at hip height, 0.69 long |

## Sequences and keyframes [verified]

- **Sequence header** (0x1C bytes): a bounding sphere, the frame count (+0x10), a sound
  (+0x11), a trigger count (+0x12), and at +0x13 either 0xFF for an uncompressed sequence
  or the number of extra words after the frame pointers.
- **Frame pointers** follow at +0x1C, one per keyframe. A compressed sequence's pointers
  have 0xF in the top four bits (mask them off). Every hero sequence is compressed; the
  wrench's and Clank's are not.
- **A keyframe** (uncompressed, or after unpacking) is:

  | Offset | Field |
  |---|---|
  | 0x0 | Compressed: u16 ticks the keyframe lasts. Uncompressed: f32 1/ticks |
  | 0x2 or 0x4 | u16 time stamp in sixteenths of an animation frame (offset +0x4 once unpacked) |
  | 0x6 | Size in quadwords, minus one |
  | 0x8 | Joint count × 8 |
  | 0xA, 0xE | Scale count, translation count |
  | 0x10 | Per joint: rotation quaternion x, y, z, w as s16, 1.0 = 0x8000 |
  | then | Scale entries: x, y, z as u16 with 1.0 = 4096, then 0x8000 + joint |
  | then | Translation entries: x, y, z as s16 in class units, then the joint |

- **Playback:** the game interpolates from each keyframe to the next over the first
  keyframe's ticks (1 to 10 for the hero, mostly 2), and wraps to the first keyframe at the
  end. The hero's "animation frame" (0x1A57F8), which the melee windows are timed by, is
  the time stamp / 16 interpolated the same way: keyframes 2 ticks long advance it 0.5 a
  tick.
- **Compressed keyframes:** each sequence has a base block after its frame pointers:
  - a header word: bits 0–9 the offset of the frame's 16-bit values, bits 10–20 the offset
    of its byte deltas, bits 21–31 the offset of its sign bits, all from the frame's
    data at frame + 4;
  - per joint, the base x, y and z of its rotation as u16, whose low two bits say how a
    frame changes it: 0 keep it, 1 add a signed byte × 4, 2 add a signed 10-bit value × 4
    from a bit stream (three values to each 32-bit word, low bits first), 3 replace it
    with a 16-bit value;
  - the scale count and translation count (one byte each), then 8-byte base entries for
    each: x, y, z, and the joint.

  A frame holds two u16 (ticks and time stamp), then the bit stream, byte deltas, 16-bit
  values, one sign bit per joint for w (w = √(1 − x² − y² − z²)), and then per scale or
  translation entry a byte of three 2-bit modes (0 keep, 1 add a signed byte × 4, 2 a
  16-bit value that follows) with its data. All 127 hero sequences unpack without
  overrunning their frames.

## Posing a skeleton [verified]

- A joint's world transform is its parent's, times its translation (the keyframe's if it
  has one, else the rest offset), times its rotation, times its scale.
- **The stored quaternion is the inverse of the rotation**: conjugate it (negate x, y and z)
  before using it as a standard rotation. Checked by placing the Veldin image's attached
  mobys on the hero's joints from his current keyframe (sequence 2, keyframe 67): Clank,
  the wrench and the 0x50A object land within 0.0005 u.
- The moby's own rotation matrix (moby + 0xC0) maps class axes to world axes row by row,
  and its position (moby + 0x10) is the root's world position.

## Mesh [verified 2026-10-13]

Read with `rac_formats::moby::{Packet, Mesh}` on the UYA disc: Ratchet in every armor
mesh (entries 0–34, below), the wrench, and Clank's class from Veldin's core. Every
packet decodes with no load from an unset VU0 slot, no duplicate of an empty cache entry,
every blend weight summing to 256 and every strip index in range; Ratchet's armor 0 gives
5,655 vertices and 6,740 triangles. The layout follows ReRAC's moby notes (ISC, written
from Wrench's behaviour on R&C1; `prior-art.md`), checked here against UYA's data:
GC, UYA and DL halve the vertex table header's fields to u16 but otherwise match.

- **Packet table entries** (16 bytes): the VIF list's address, its size in quadwords
  (u16), the quadword offset of its texture data (u16), the vertex table's address, and
  four bytes: the vertex table's size in quadwords, two counts derived from the next, and
  the number of vertices sent to VU1. The table lists the high-detail packets, then the
  low-detail ones, then the metal ones, which have their own vertex format (next
  section).
- **Vertex table header** (u16 each): matrix transfers, two-joint vertices, three-joint
  vertices, one-matrix vertices, duplicates, vertices sent (the four counts before it
  added), the byte offset of the vertex records, and one we don't use. Then the matrix
  transfers (a joint and a VU0 address, a byte each), and, 8-byte aligned, one u16 per
  duplicate: a vertex-cache entry shifted left by 7.
- **Vertex records** (16 bytes): bytes 0–7 skin the vertex (below); 8 and 9 the normal as
  azimuth about z from x, then elevation towards z, 256 steps a turn
  (x = cos a cos e, y = sin a cos e, z = sin e; the decoded normals face the way 99.3% of
  Ratchet's triangles do); then x, y, z as s16 at the rest pose.
- **Vertex IDs:** the low 9 bits of a record's first halfword are the ID of the vertex 7
  records before it, since VU1 has seven vertices in flight. At least one record follows
  the real vertices; the IDs no record holds are up to six more halfwords at bytes 4–15
  of the last one. A vertex fills the vertex cache (512 entries) at its ID.
- **Skinning** replays VU0: 64 matrix slots of four quadwords, addressed 0–0xF0 (0xF4
  means "don't store"). Slots keep their matrices from packet to packet within a level of
  detail.
  - First the packet's matrix transfers put a joint's matrix in a slot.
  - A two-joint vertex: byte 1 >> 1 is a joint it moves into slot address byte 6 first;
    it blends slots byte 2 and byte 3 with weights byte 4 and byte 5 (out of 256), and
    stores the blend at byte 7.
  - A three-joint vertex blends slots byte 2, byte 3 and byte 1 & 0xFE with bytes 4–6,
    and stores at byte 7. It can't move a joint in.
  - A one-matrix vertex moves joint byte 1 >> 1 into slot byte 3, then uses slot byte 2,
    which may hold a blend an earlier vertex or packet stored.
  - Blend inputs are always single joints. Every vertex of every armor follows one to
    three joints this way.
- **Duplicates** follow a packet's own vertices: each copies the cached vertex its entry
  names (position, normal and skin), possibly from an earlier packet, with a texture
  coordinate of its own. That's how a seam shares a position between two UV islands.
- **Texture coordinates:** the VIF list's first UNPACK (V2-16) holds an s, t pair per
  vertex sent, own then duplicate, with 1.0 = 4096. Metal packets have none.
- **Strips:** the next UNPACK (V4-8) is a 4-byte header and then 1-based indices into the
  vertices sent. A set high bit means the vertex doesn't complete a triangle (the GS's ADC
  bit), which is how strips restart. An index of 0 switches texture: it takes the next
  64-byte block of GS registers from the third UNPACK (TEX1, CLAMP, TEX0, MIPTBP1; TEX0's
  low word is the class's texture number on the disc, −1 for none) and the next switch
  index, which is the header's third byte for the first switch and byte 0xC of quadword
  k − 1 of the third UNPACK for switch k. A switch index of 0 ends the packet, and the
  three indices before it (always 1, 1, 1) are never drawn. Texture state carries over
  from packet to packet.
- **Winding** isn't kept: the strips' triangles face either way about equally often
  (3,358 of 6,740 agree with their normals), and the GS draws both sides. Turn each
  triangle to agree with its vertex normals.

## Metal packets [verified 2026-10-15]

Read with `rac_formats::moby` (`PacketKind::Metal`) on every UYA armor mesh, the wrench
and Clank. The layout is the one ReRAC's notes give for R&C1 (ISC, read there in R&C1's
code; `prior-art.md`), and UYA's data follows it.

- **Where:** the class header's bytes 6 and 7 (an armor mesh file's bytes 2 and 3) are
  the metal packet count and the first one's entry in the packet table, straight after
  the high- and low-detail packets. Of UYA's armor WAD entries, 1, 2, 4, 14–28 and the
  wrench-sized 29–34 have them (armor 0 has none, the wrench 3); so does Clank's class
  (396 triangles).
- **Vertex table:** four words (the vertex count, then where the game's EE code puts the
  texture coordinates, positions and colours it computes, and their total size), then
  16 bytes per vertex: x, y, z as s16, the normal's azimuth and elevation, up to three
  joints, how many it uses, their weights out of 256 (none when it uses one), and a pad.
  The packet table entry gives the table's size as the count plus one quadword, and the
  count again in its last byte. Metal vertices use neither the VU0 slots nor the vertex
  cache.
- **VIF list:** strips and texture switches as in a regular packet, with no texture
  coordinates. The switch's TEX0 low word is −2, which R&C1's code replaces with the
  level's chrome map (and −3 with its glass map); it maps each vertex from its turned
  normal, so the map works like a reflection.
- **They redraw regular triangles:** every metal vertex sits on a vertex of the same
  mesh's high-detail packets, and every metal triangle has exactly one high-detail
  triangle with the same three corners (all 35 mesh entries, the wrench and Clank). In
  R&C1 the game draws them as a second, blended pass, pulled towards the camera (ReRAC).
  Our import paints those triangles as polished metal instead, which the scene's own
  surroundings reflect.

## The armor WAD [verified 2026-10-13]

The hero's class has no mesh on the disc; his armor brings it. Read in UYA's code
(`armor_load` 0x422838, `armor_relocate` 0x423F28, `armor_attach` 0x424008,
`wrench_load` 0x422D70 and `clank_texture_load` 0x423238 in
`re/SCUS-97353/symbols.toml`) and with our own reader, whose armor 0 matches the Veldin
memory image's hero mesh (0x1F28000) except for the relocated pointers.

- **The table** is the fifth global header in the table of contents (header size 0x398,
  first sector 0xAEA89 on our disc). After the size and sector: 56 entries of four words,
  each a `{sector, count}` from the WAD's first sector for an armor's mesh and then its
  textures; then two `{sector, count}` pairs for Clank's textures.
- **Loading an armor:** the game reads the textures into the armor buffer and sends each
  straight to the GS, then reads the mesh over them, relocates it and gives the hero's
  class its packet table, packet counts and texture-usage table. The mesh and textures
  are stored as they are, not WAD-compressed.
- **Mesh file:** a 16-byte header (high, low and metal packet counts and the first metal
  packet as bytes; the packet table's offset; the texture-usage table's offset; 0), then
  packets as in a class, with pointers counted from the file's start.
- **Texture file:** a count, an offset per texture, then each texture as a PIF2 image:
  the magic "2FIP", a word we don't use, width, height, the pixel format at +0x10 (0x13,
  8-bit indexed, for all of them), and the mipmap count at +0x1C; a 256-colour palette at
  +0x20 (RGBA, alpha 0x80 opaque) in the GS's CSM1 order, which swaps entries 8–15 and
  16–23 of every 32; the pixels at +0x420; then the smaller mipmaps. The mesh's texture
  numbers index this list.
- **Which entry is what:** 0 is the armor a new game starts in (Veldin). Entries 0–34
  are meshes of this kind and all decode. Seen on the rig with UYA's skeleton
  (2026-10-15, Xvfb): 0–4 and 14 are armors, 5 Ratchet in casual clothes, 6 a snowman,
  7 a tuxedo, and 8–13 aliens and robots; all 15 are skinned to the hero's own joints.
  15–28 are byte-for-byte copies of 14, and 32–34 of 29–31. Entry 31 is the wrench:
  `wrench_load` reads it whatever the armor, and gives its mesh to the wrench's class
  (0x47), whose class in the core has no packets; 29 and 30 are wrench-sized too. Which
  of the game's armor names goes with which entry isn't read yet. Entries 35–55 start
  with a whole moby class (a skeleton of their own), probably the multiplayer
  characters, and aren't read yet.
- **Clank's texture:** Clank's class (0x259, in the core) has its own mesh, but the core's
  class table names no textures for it (all 0xFF). `clank_texture_load` reads Clank
  texture 0 from the pairs after the armor entries: a bare PIF2 image.
- **The GS's copy:** the textures go to fixed GS addresses kept at 0x1A1EE0 (palettes:
  armor, wrench, a gadget, Clank) and 0x1A1EF4 (images, the same order), and each
  packet's TEX0 is rewritten to point there at load.

## The wrench and Clank on the hero [verified 2026-10-13]

- **Orientation:** in the Veldin image the wrench (on joint 56) and Clank (on joint 5)
  have their joint's world orientation to within 0.004, and their own class's scale
  (0.2083), not the hero's. So at rest the wrench sits at joint 56's rest position with
  its class axes unturned, and Clank at joint 5's.
- **Their own poses:** each plays its own sequence: the wrench sequence 1 and Clank
  sequence 13 in the Veldin image, where Ratchet stands. Posed that way, the wrench points
  out along Ratchet's T-pose arm and up, about (0.87, 0.47, 0.12) in the hero's
  Bevy-axis space from the hand; that is the direction the rig's wrench joint uses for
  every model (`rac_rig::model::wrench_points`).
- **Clank's back form** (class 0x259, 33 joints): 0 the root, 1 the hips, 4 the head's
  pivot, 5 the top of the head with the antenna chain 6–10, the face (eyes, lids, mouth)
  13–25 under 4, the feet 27 (right) and 28 (left) under 26, and a back plate 29–32. It
  has no arm joints.

## DL's skins [verified 2026-10-15]

Read with `rac_formats::armor::{SkinTable, Skin}` and `MobyClass::read_dl` on DL NTSC-U
(SCUS-97465), and in Ghidra on our DreadZone Station memory image (`skin_load` 0x5756D0,
`vendor_list` 0x631CE8 and the data they use, in `re/SCUS-97465/symbols.toml`).
Gladiator PAL (SCES-53285) has the same table of contents sizes and the same skins: every
skin's skeleton and textures come out identical.

- **The table** is the fifth of DL's eight global headers, as UYA's armor table is
  (size 0x228; first sector 0x9B465 on our NTSC disc). After the size and sector: 17
  entries of `{class sector, count, textures sector, count}` from the WAD's first
  sector, three empty ones, then texture files with no class, in three groups the game
  loads separately (from pairs 40, 52 and 60; one by a number kept in the save, two by
  level). Those aren't read yet. In memory the table of contents sits at 0x1C2040.
- **Loading a skin** (`skin_load`): the game reads entry *n*'s textures and sends them to
  the GS, then reads its class over them and relocates it, for player 1 or player 2.
  The skin worn is a saved number (0x171B52 for player 1), and a bit per entry records
  which have been bought. The player's moby draws with the skin's class (it reports
  class 0x23F7, which libdl calls Ratchet's blue armor with no head).
- **Which entry is what**, from the game's skin menu (`vendor_list`): entries 0–4 are
  Ratchet's armors (Marauder, Avenger, Crusader, Vindicator, Liberator), and the
  menu's skins are, in its order, Alpha Clank (5), "The Muscle" (8), Vernon (12), King
  Claude (11), LandShark (7), Squidzor (6), Kid Nova (13), Jak (15), W3RM (9),
  Starshield (10) and Venus (14), priced 0 to 75. Entry 16, Ratchet in DL's helmet and
  suit, isn't in the menu. Each matched what we saw when we wore it (Xvfb). The
  multiplayer skins are other classes (libdl: 0x251C–0x2531).
- **The armors have no head:** their meshes stop at the neck (1.03 m) at both levels of
  detail, and no moby near the player carries one in our image. Where the game draws
  Ratchet's head from isn't found yet.
- **Each skin is a whole class**, pointers counted from its start, with UYA's header
  for the packet table, packet counts, joint count and scale (0.2083, as UYA's hero),
  but:
  - the skeleton (+0x14) is a 3×4 inverse bind matrix per joint (0x30 bytes: three
    rows of rotation then translation), and some face joints are turned at rest, so a
    joint's rest position is where its matrix takes to the origin;
  - each joint offset record (+0x18) ends with a byte naming the parent in its low
    seven bits (0x7F for the root); the skins set the high bit, another class (a
    winged pack, 0x246B) doesn't;
  - one sequence (+0x48), not read.
- **Meshes and textures** decode with UYA's rules unchanged: 1,692 to 5,640 triangles,
  no metal packets, a low level of detail on most skins; two PIF2 textures each
  (256² and 128² for the armors, two 128² for the rest).
- **DL's player skeleton** (107 joints, the same in every skin; the heads differ a
  little): 0 the root, 1 and 2 at the root, 3–5 the hips (0.53 m), 6–11 the spine to the
  neck, 12 the head, 13–50 the face (19 and 20 the jaw, 29–32 the ears), 51–72 the left
  arm (52 shoulder, 54 elbow, 56 hand, 57 palm with four fingers, 70 thumb), 73–94 the
  right, 95 and 96 on the chest, 97–101 the left leg (thigh, knee, ankle, foot, toe)
  and 102–106 the right. It has no tail. The left side comes first, unlike UYA's.
- **The wrench** is a class of its own in the level cores (libdl: 0x23FA), which aren't
  read yet.

## Where this is used

- `crates/rac_formats/src/moby/`: the reader (class, skeleton, keyframes, packets and the
  assembled mesh), tested on synthetic data; `rac_formats::armor` and
  `rac_formats::texture` read the armor WAD and its images.
- `openrac-tool moby`: describes a class in an EE image and writes it as a `.glb` into
  `cache/models/`, now with the mesh's own skin weights (`docs/re-setup.md`, "Extracting a
  model").
- `openrac-tool hero`: reads the hero's skeleton and animations from the UYA disc image
  itself, and with `--check-ee` compares them with a memory image of the same level.
- `crates/rac_rig`: the hero kit's skeleton and its open model. Its joints sit where UYA's
  hero's do, rounded to the centimetre, and each names the UYA joint it stands for, so
  these animations can drive it. Its blocks were sized against the hero's mesh, measured
  per body part (head 0.40 × 0.39 m, ears to 1.42 m, feet 0.38 m long, tail 0.63 m). No
  geometry or animation from the disc is in the repository.
- `crates/openrac/src/hero_model.rs`: at start-up, Ratchet in each of the 15 distinct
  suits with its textures, the wrench and Clank, skinned to the rig's skeleton as the
  models "uya" (armor 0), "uya-blue", "uya-snowman" and so on (`SUITS`; PLAN.md §5,
  2026-10-13). Triangles under metal packets are polished metal. From a DL or Gladiator
  disc, the twelve complete skins as "dl-ratchet", "dl-jak" and so on (`DL_SUITS`),
  each lent the open model's wrench. Nothing read is stored.

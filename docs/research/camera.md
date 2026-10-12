# Camera: how the third-person camera follows Ratchet

How the camera follows the hero in R&C3/UYA (PLAN.md RE.4). It will be the reference for
the hero kit's camera (P1.7). Started 2026-10-12, at the end of a session that was cut
short, so these are early notes. Nothing here has been checked against a trace yet.

## Status and method

- **Memory images** (the same ones as `hero-controller.md`):
  - R&C3 PAL (SCES-52456) on Veldin, from the savestates of the owner's recordings 5 and 6;
  - UYA NTSC-U (SCUS-97353) on Veldin, from a headless boot, saved during the level's
    opening cutscene.
- **Labels** as in `hero-controller.md`: **[verified]**, **[code]**, **[measured]** (here,
  read from savestates) and **[reported]** (from libuya, unchecked).
- Units: world units (u), z up.

## Where the camera lives

- **The `GameCamera` struct** **[reported, consistent with the code]**. libuya's
  `camera.h` (MIT) describes a 0x460-byte `GameCamera`:
  - the position at +0x00 and the rotation at +0x10;
  - the current and previous update-camera pointers at +0x50 and +0x54;
  - the camera's copy of the hero (`CameraHeroData`) at +0x150;
  - the field-of-view block at +0x420.
- **Address:** camera 0 is at **0x1E2A40** in both R&C3 PAL and UYA NTSC. The code indexes
  further cameras at a stride of 0x460.
  - **[measured on R&C3 PAL]:** in both savestates, the hero copy at +0x190 equals the
    hero's position, and the camera position sits behind and above the hero.
- **The libuya layout holds on R&C3 PAL for the fields the follow code uses**
  **[code]**. The decompiled follow code reads `CameraHeroData` at exactly libuya's
  offsets:
  - the hero state (+0x250);
  - the hero's height (+0x198) and yaw (+0x1A8);
  - the swing target (+0x288) and the ideal swing radius (+0x290).

## Default follow position

- **4.64 u behind the hero and 2.0 u above** **[code; measured in three savestates]**.
  - **Savestates:**
    - R&C3 PAL, savestates 5 and 6: the camera is 4.640 u from the hero horizontally and
      1.996–2.004 u above (5.05 u in a straight line).
    - UYA NTSC, opening cutscene: exactly 4.641 u behind and 2.0 u above, with yaw π/2.
  - **Code:** the R&C3 PAL function at 0x3EC1A0 (called from 0x3EC848).
    - Outside the swing case below, it targets a distance of 4.64 and a height of 2.0.
    - Each time it runs (presumably once a frame), it moves three follow values towards
      their targets with `approach` (`re/SCES-52456/symbols.toml`), by at most a fixed
      step:

      | Value | Target | Step per call (PAL) |
      |---|---|---|
      | distance | 4.64 | 0.16 |
      | height | 2.0 | 0.08 |
      | a third value | height − 0.5 | 0.16 |

    - These are PAL steps. The NTSC steps aren't read yet. If UYA folds
      per-second constants at compile time as the hero code does, the NTSC steps would be
      5/6 of these.
- **4.64 appears as a code literal in four camera functions, on both builds:**

  | Build | Literal at | In the function at |
  |---|---|---|
  | R&C3 PAL | 0x3E71CC | 0x3E7138 |
  | R&C3 PAL | 0x3EBFD4 | 0x3EBF40 |
  | R&C3 PAL | 0x3EC290 | 0x3EC1A0 (the follow code above) |
  | R&C3 PAL | 0x3F2CEC | 0x3F2B58 |
  | UYA NTSC | 0x3E5360, 0x3EA124, 0x3EA3E0, 0x3F0B74 | not mapped yet |

  - Only 0x3EC1A0 has been read.
  - One of the other functions checks whether a value lies outside 4.64–6.0.
- **While swinging** **[code; that this is the Swingshot is inferred from libuya's field
  names]**. This case applies when the camera's copy of the hero state is 43 and a swing
  target is set. With t = clamp(target z − hero z, 0, ideal radius) / ideal radius:
  - the distance target is 3.65 + t;
  - the height target is 2t.
  - On levels 7, 9 and 14 the code also picks a swing target itself. It chooses among
    mobys of class 0x323, 0x2F6 and 0x19C4, scoring each by its distance minus 10 × its
    alignment with the hero's facing, so it prefers targets ahead of the hero.

## Partly seen

- **Field of view [measured, meaning unclear]:** on R&C3 PAL the FOV block holds 1.11
  (63.6° if radians) in the two fields libuya calls `speed` and `ideal`. The next field,
  `actual`, reads 0.005, so libuya's names may be off here.
- **Collision [code, only skimmed]:** the function at 0x3E77B0 looks like the camera's
  clearance test. It checks a sphere of radius 0.35 plus a speed-dependent term against
  several collision queries, and moves the camera in on a hit.

## Traces

- `cache/pcsx2/traces/rc3pal-veldin-5-cam.trace` is a replay of the owner's R&C3 PAL
  recording 5, with the camera's position and rotation added: 1,805 frames, none missing.
  - Its layout is `source/Ratchet & Clank 3/rc3pal-veldin-5-cam.toml`: `hero.toml` plus
    the camera block.
  - Not analysed yet.

## Next steps

1. Fit the follow lag from that trace. Do the distance and height settle as the `approach`
   steps predict? How does the yaw recentre behind the hero?
2. Read the other three 4.64 functions and the yaw and recentring code. Then read the same
   functions on UYA NTSC, for the 60 Hz steps.
3. Once checked, move the camera fields into `re/SCES-52456/hero.toml` (RE.5's "camera
   fields").
4. Then strafe and first-person modes, collision, and the camera volumes in level data.

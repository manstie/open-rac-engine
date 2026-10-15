# Camera: how the third-person camera follows Ratchet

How the camera follows the hero in R&C3/UYA (PLAN.md RE.4), the reference for the hero
kit's camera (P1.7). Started 2026-10-12. The same day the follow camera's code was read on
UYA NTSC-U and checked against a trace we made for it ("How the follow camera works").

## Status and method

- **Memory images** (the same ones as `hero-controller.md`):
  - R&C3 PAL (SCES-52456) on Veldin, from the savestates of the owner's recordings 5 and 6;
  - UYA NTSC-U (SCUS-97353) on Veldin, from a headless boot, saved during the level's
    opening cutscene.
- **Labels** as in `hero-controller.md`: **[verified]**, **[code]**, **[measured]** (here,
  read from savestates or traces) and **[reported]** (from libuya, unchecked).
- **Our UYA NTSC camera trace** `uyantsc-veldin-cam-1` (2026-10-12, 2,673 frames): scripted
  keyboard play from `uyantsc-veldin-play.p2s`, capturing the hero, the camera and its
  follow block (layout `source/Ratchet & Clank 3/uyantsc-veldin-cam.toml`).
  - The keyboard pad has no right stick, so every turn of the camera in it is automatic.
  - The hero ran off the cliff ahead of the start twice and respawned. The stretches
    used below avoid those moments.
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

## How the follow camera works [UYA NTSC, 2026-10-12]

Camera mode 0, the normal third-person camera. Each camera mode has an entry in a table at
0x2F7700: a mode number, then four functions. Mode 0's per-frame update is 0x3E8850.
- **The camera's state:**
  - an update-camera object at 0x1E31B0 (camera 0), whose +0x30 is the camera position;
  - a follow block at 0x1E5590;
  - a base block at 0x1E5A90 holding the defaults the follow block springs back to.
- **Units:** per frame at 60 Hz. The springs are the hero's turn springs again: each
  frame the rate gains `gain × error` and loses `damping × rate`, may be capped, and
  never carries the value past its target. Angle springs wrap the error.

Each frame, in this order:

1. **The follow point** (follow +0x60) is the hero's position. Vertically it holds
   still during jumps **[measured]**:
   - In a standing jump and a double jump the camera didn't rise at all. The follow
     point stayed at the take-off height while the hero rose 3.2 u.
   - Falling (state 6) it followed him down exactly.
   - A state machine in 0x3E2AE8 and 0x3E3130 handles the vertical modes per hero state
     type (jumps, ledges, flips and others), with blend rates of 0.0333–0.1 u per frame
     over 10–30 frames. It is not read in detail.
2. **The look point lags** (follow +0xC0) **[verified]**:
   - Horizontally it springs towards the follow point with gain 0.015 and damping 0.2.
   - Vertically it springs with gain 0.0125 and damping 0.2.
   - Simulated from the trace's follow points, it matches the trace's look point to
     0.0005 u over 200 frames.
   - Running steadily at 6.17 u/s (v = 0.1028 u/frame), it trails by
     `(0.2 / 0.015 − 1) × v` after each frame's update, which is 1.27 u.
3. **The arm is dragged** (follow +0x180, the desired arm from the camera's pivot)
   **[verified]**:
   - Each frame it becomes `lerp(arm, last frame's camera position − pivot, 0.85)`. So
     it takes up 85% of the hero's movement, and the camera stays mostly where it was and
     swings round to follow.
   - The arm's length is then set to the follow distance, less any pull-in.
   - Running sideways at 0.1028 u/frame, the arm turned 0.01883 rad a frame in the
     trace, which is `0.85 × 0.1028 / 4.64`.
   - Simulated over obstacle-free stretches, the arm matches the trace to 0.004 rad over
     82–100 frames.
   - **There is no other automatic recentring.** A rule that stops the drag for 32 frames
     after a sharp turn, and eases it back over 45, exists in 0x3E42C0, but a global flag
     (0x1DF6C4, zero) switches it off. The trace's timers for it stayed at zero.
4. **Manual control** (0x3E7478, 0x3E7668) **[code]**:
   - **Yaw:** the right stick's x turns the arm at `0.0218 + 0.0567 × speed / 100` rad
     per frame, where speed is the camera-speed option (default 50): 0.0502 rad/frame,
     3.01 rad/s.
   - **Pitch:** the stick's y, past a 0.3 dead zone and rescaled, isn't a rate. It sets a
     target tilt of the arm of up to ±40° (0.6981 rad). The target eases at 0.02 a frame
     when pushed further and 0.01 when returning, and comes back when the stick is
     released.
   - The tilt then moves towards its target by at most 0.0305 rad a frame.
   - Both axes have invert options.
5. **The distance and height** **[verified for the distance]**:
   - At the end of each frame (0x3E4008) the follow distance (+0x1AC) springs towards its
     target with gain 0.003 and damping 0.2. The target is 4.64 unless something set
     another for this frame.
   - The height (+0x1B0) springs towards 2.0, and the look height (+0x130) towards 1.5
     (gain 0.005), both with damping 0.2.
   - Simulated from the trace, the distance matches to 0.00001 u over four idle stretches
     of 120–300 frames.
   - The respawn camera starts at distance 6.34, height 1.21 and look height 2.76, and
     eases back from there.
6. **Running towards the camera pushes it back** (0x3E80A8) **[code; the trace's rise
   to 5.6 u fits]**:
   - It applies when the hero moves, and his motion direction is more than 0.3 against
     the camera's facing (a dot product below −0.3).
   - The distance target becomes `4.64 + (6.0 − 4.64) × min(speed / 0.07 u/frame, 1)`,
     kept at its highest during the approach. In some camera modes the far end is 4.0
     rather than 6.0.
   - At the same time the arm-length spring stiffens from gain 0.02 / damping 0.2 to
     0.04 / 0.3 at full speed.
7. **The camera's actual arm** (follow +0x190) chases the desired one (0x3E82F8)
   **[verified for yaw]**:
   - Its yaw and its elevation each spring with gain 0.015 and damping 0.2. Its length
     springs with the gains of step 6.
   - Steady sideways running leaves it about 0.25 rad behind the desired arm.
   - Simulated, its yaw matches the trace to 0.0009 rad over 100 frames.
8. **Placing and aiming the camera** **[code; positions consistent in the trace]**:
   - The camera goes at the look point + 2.0 u up (the height) + the actual arm.
   - It is pushed out of geometry with a 0.25 u sphere, up to three times.
   - It aims at the follow point + 1.5 u up, not at the lagging look point. A blend
     towards the look point exists, but only special modes turn it on.
9. **Obstacles** (0x3E64E0) **[code, partly read]**:
   - A pull-in (+0x240) lets the arm be shorter than the follow distance. It eases back
     to zero by a cosine over a timer, and never leaves the arm under 1.5 u.
   - Three helpers that set it (0x3E5930, 0x3E5C20, 0x3E6128) aren't read yet.
   - Some object classes near the arm turn it away by 1° a frame.
   - A view from the pivot to the camera that stays blocked for 30 frames resets the
     camera.
10. **Camera volumes from level data** (0x3F09E0) **[code]**:
    - Each camera has 32 volume slots at 0x1E5D10, 0x40 bytes each. The active one is at
      0x1DF980 (−1 for none, as in the whole trace).
    - A volume can override the distance (gain 0.001), the height and the look height,
      steer the arm towards a set direction within an angle, and switch on the look-point
      blend.

## Lock-on [code, UYA NTSC, 2026-10-15]

While the hero is locked on to a target (`hero-controller.md`, "Strafing and lock-on"),
the follow camera turns to look past him at it. Not traced: no savestate of ours holds a
weapon that allows lock-on.
- **A virtual right stick** (`camera_steer_to`, 0x3E7168): each frame
  `camera_follow_params` (0x3E42C0) sets the camera's virtual stick (+0x1FC) from the
  target. With x the arm's angle off the line from the target through the follow point,
  in right angles (0 to 2), it is `2x − x²`, signed to turn towards that line. It is
  fullest at a right angle and fades to nothing both at the line and straight across
  from it.
- **Its speed ramps up** (`camera_lock_on_turn`, 0x3E7430): the turn speed (+0x1F4) is
  `(t / 400) × π/2` rad a frame, where t counts the frames locked on, up to 400, and
  drops to 0 without a target. So the camera barely moves at first. From about 200
  frames on, a small error is overcorrected each frame, which the actual arm's spring
  (step 7) smooths out.
- **The player wins:** `camera_stick_input` (0x3E7478) uses the virtual stick only when
  the right stick's x is 0, and a real push resets the turn speed to the camera-speed
  option. The stick spring between them (+0x1E0: gain 2, damping 4, never past its
  target) settles in one frame, so the kit leaves it out.
- The camera ends the frame by clearing the virtual stick (`camera_end_of_frame`).
- **In the kit:** `rac_hero::camera`, `lock_turn`; its profile's `lock_turn` and
  `lock_ramp_ticks`.

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
  - The owner used the right stick between every pair of moments where the camera
    settled, so it couldn't separate automatic from manual turning. That led to the
    scripted UYA trace below.
- `source/Ratchet & Clank 3/uyantsc-veldin-cam-1.trace`, with its script `.sh` and layout
  `uyantsc-veldin-cam.toml`: see "Status and method".

## Next steps

1. **Done 2026-10-12:**
   - The follow lag, drag and distance are read on UYA NTSC and checked against
     `uyantsc-veldin-cam-1`.
   - The 0x3EA2F0 function with the `approach` steps is the swing and grapple camera
     (states 43 and 36), not the normal follow. Its NTSC steps are 0.1333 (distance) and
     0.0667 (height) u per frame: PAL's × 5/6.
2. Read the vertical state machine (0x3E2AE8, 0x3E3130) for landings at a new height and
   ledge grabs, and check them with a trace that jumps onto something.
3. Read the obstacle helpers (0x3E5930, 0x3E5C20, 0x3E6128), with a trace along walls.
4. The lock-strafe and first-person modes (camera modes 1 and 2), and the camera volumes
   in level data (Phase 3). Lock-on in the follow camera is read (section "Lock-on").

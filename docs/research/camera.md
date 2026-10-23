# Camera: how the third-person camera follows Ratchet

How the camera follows the hero in R&C3/UYA and Deadlocked (PLAN.md RE.4), the reference
for the hero kit's camera (P1.7). Started 2026-10-12. The same day the follow camera's code
was read on UYA NTSC-U and checked against a trace we made for it ("How the follow camera
works"). On 2026-10-16 the follow point's height, obstacles and collision, the camera
volumes and level cameras, the look, lock-strafe and first-person views, and Deadlocked's
camera followed, each in its own section below. On 2026-10-17 traces checked UYA's
lock-strafe, first person and a gun, a camera volume and a flythrough, the clearance
under a low ceiling, the arm sweep, the blocked-view reset, and both games' sticks at part
tilt; the blends between cameras and the collision masks were read. The same day the kit's
camera was built from all of it ("In the kit").

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
- **Address, corrected 2026-10-16 [code]:** libuya's layout holds from **0x1E2AC0** for
  camera 0 on UYA NTSC (Veldin's build), so its `CameraHeroData` starts at 0x1E2C10. The
  block at 0x1E2A40 that our layouts call "camera" is 0x80 earlier: a copy of the
  position and rotation that 0x497828 writes each frame. Offsets in our layouts' "camera"
  region are therefore libuya's + 0x80: the layouts' hero state copy at +0x2D0 is
  libuya's +0x250 (0x1E2D10), and the old `+0x2FA` and `+0x2FB` are 0x1E2D3A and 0x1E2D3B
  ("Where it reads the hero" under "The follow point's height"). The code indexes further
  cameras at a stride of 0x460.
  - The camera blocks move with the level's build: on the Starship Phoenix they sit 0x580
    lower (`GameCamera` 0x1E2540, update object 0x1E2E40, follow block 0x1E5010).
  - Deadlocked's camera 0 is at 0x230280, 0x470 bytes ("Deadlocked's camera").
  - **[measured on R&C3 PAL, 2026-10-12]:** in both savestates the 0x1E2A40 block's +0x190
    equals the hero's position, and the camera position sits behind and above the hero.
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

1. **The follow point** (follow +0x60) is the hero's position horizontally. Its height
   comes from a vertical mode per hero state: held during jumps, raised on a timer in
   heli, rocket and flip jumps, halfway in a jump attack. See "The follow point's height"
   below **[verified]**.
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
4. **Manual control** (0x3E7478, 0x3E7668) **[code; the stick response verified on
   `uyantsc-veldin-camhalf-0.5-1`]**:
   - **Yaw:** the right stick's x turns the arm at `0.0218 + 0.0567 × speed / 100` rad
     per frame, where speed is the camera-speed option (default 50): 0.0502 rad/frame,
     3.01 rad/s. **The rate is linear in the stick:** at x = 0.2105 the arm turned
     0.01056 rad a frame, and at −0.1974, −0.00991 (0.05015 × x).
   - **Pitch:** the stick's y, past a 0.3 dead zone and rescaled, isn't a rate. It sets a
     target tilt of the arm of up to ±40° (0.6981 rad). The target eases at 0.02 a frame
     when pushed further and 0.01 when returning, and comes back when the stick is
     released. A y of ±0.2 left the tilt alone, as the dead zone says.
   - **The tilt in detail** (0x3E7668): the rescaled y, `(|y| − 0.3) / 0.7`, is the target
     of a spring (gain 2, damping 4, never past its target) capped at 0.02 of the range a
     frame going out and 0.01 coming back; times 0.6981 that is the arm's target
     elevation, and the arm turns towards it by at most a per-frame cap. Coming back, the
     target follows the arm if the arm is nearer level.
   - **Seen at 0.632** (`uyantsc-veldin-camhalf-0.75-1`): the desired arm rose 0.81° a
     frame (0.02 × 40° is 0.80°) and settled at 19.2° for y = −0.618, 1° more than the
     formula's 18.2°, which isn't explained; pushed down at 0.632 it stopped at −11.8°,
     where the camera's sphere met the floor (a floor contact stops the downward tilt).
     Let go, it came back at about 0.45° a frame.
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
   - It aims at the follow point + 1.5 u up, not at the lagging look point, unless
     something turns on the blend towards the look point.
   - **The blend** (in `camera_follow_motion`, 0x3E39E0) **[code; the 90 read from
     memory in UYA NTSC and DL NTSC]**: the aim (+0xA0) is `lerp(follow point, look point,
     n / 90)` + the look height. Each frame, n (+0x158) rises by 1 up to 90 if the flag
     (+0x159) is set, and otherwise falls by 1 down to 0. `camera_end_of_frame` clears
     the flag and sets the 90 (+0x16C) from 0x1DF6B0 (DL: 0x222810), and the camera's
     init (0x3E52D0) clears n. So the aim takes 1.5 s to move over and as long to come
     back, and tapping strafe barely moves it.
   - Strafing, lock-on and a boss turn the blend on (`camera_follow_params`,
     "Per-state parameters" below), as do the jump pad and a camera volume's flags +0x18
     and +0x19. Deadlocked's code (0x51A660) is the same.
   - **The view doesn't point at the aim** (`camera_orient`, 0x3E7B50, last in mode 0's
     motion after the arm is placed) **[code; verified on UYA NTSC, 2026-10-21]**. Only
     its yaw comes from the aim; its tilt never sees the aim's height:
     - **Yaw:** the view faces the aim across: along the horizontal line from the
       camera to the aim, turned by an extra yaw at follow +0x48 (zero in every trace).
       Closer than 0.05 u across, it keeps last frame's.
     - **Pitch:** as if the aim were at the camera's height less the actual arm's rise
       (+0x190, along up), the height and the look height: `atan2(−(rise + h − l),
       distance across to the aim)`, where h (+0x28) and l (+0x24) chase the height
       (+0x1B0) and the look height (+0x130) on springs of gain 0.004 and damping 0.2,
       rates at +0x30 and +0x2C. At rest that is the aim's own −0.107 rad; but a follow
       point above or below the look point, as in every jump, a fall or a glide, doesn't
       tilt the view.
     - **Look-up:** on good ground (0x1E2D2C), with the dragged arm (+0x180) tilted
       below level, x = −atan2(rise, length) / 40°: past x = 0.1 the view tilts up
       towards 2 × min(x, 1 − x) × 15°, through +0x34 on an angle spring of gain 0.005 and
       damping 0.2 (+0x3C, +0x40, set at init and every frame's end in 0x3E4008; rate
       +0x38). Off good ground it springs back to 0.
     - The sum is capped at ±70° (1.2217 rad).
     - **Checked:** from each frame's arm, heights and positions, the pitch matches the
       update camera's forward row (or its rotation's y) to 0.00001 rad on all frames of
       `uyantsc-veldin-camvert-1` to `-5` and `uyantsc-phoenix-camwall-1`, and to 0.0016
       rad on `-camceil-1` and `-2`; the look-up to 0.00002 rad on `-camhalf-0.75-1`
       before its look (frame 358). Pointing at the aim instead is off by up to 0.42 rad
       in the jumps. `-camblock-1` (the hero written under the floor) fits neither, the
       camera being reset.
     - **Deadlocked** (0x51E680) does the same with the same constants **[code]**; on
       `dlntsc-dreadzone-camhalf-0.5-2`, a take without jumps, the pitch matches to
       0.00001 rad.
9. **Obstacles** (0x3E64E0): a sweep of growing spheres along the arm swings it round
   things beside it, a sphere at the camera end pulls it in, and the pull-in eases back
   by a cosine. See "Obstacles and collision" below **[code; the pull-in verified]**.
10. **Camera volumes from level data** (`camera_volumes_update`, 0x3F09E0): see "Camera
    volumes and splines". None is active in the whole trace (0x1DF980 stays −1).

## The follow point's height [UYA NTSC, 2026-10-16]

Read in `camera_vertical_modes` (0x3E2AE8, the transitions) and `camera_follow_vertical`
(0x3E3130, the height), both called from `camera_follow_motion` (0x3E39E0). Verified with
`uyantsc-veldin-camvert-{1..5}` ("Traces"): simulated from each frame's hero data, the mode
matched on all 3,648 frames and the follow point's height to 0.00016 u.
- **Where it reads the hero:** libuya's `GameCamera` layout holds on UYA NTSC from
  **0x1E2AC0** (0x80 after the copy of the position at 0x1E2A40 that the layouts call
  "camera"), so its `CameraHeroData` starts at 0x1E2C10. `camera_hero_copy` (0x4DE160)
  fills it each frame from the hero: state 0x1E2D10, state type 0x1E2D14, height above
  the ground 0x1E2D24 (0x1A4F1C), frames off any ground 0x1E2D30 (0x1A4F4C), frames to
  land 0x1E2D38 (0x1A505E), **past the apex 0x1E2D3A** (the byte the old layouts call
  +0x2FA; 0x1A504E) and **critter mode 0x1E2D3B** (+0x2FB; 0x1A71C4).
- **The split:** the follow point is kept as a horizontal part (+0x60) and a height
  (+0x70, a vector along up). Each frame the mode sets +0x70; +0x60 takes the hero's
  horizontal position; the look point then springs towards their sum (step 2).
- **"Ground below the hero"** below means: his position if he is on the ground this
  frame, else his position less his height above the ground (a 30 u ray down if that is
  30 or more).

**Transitions, each frame** (mode at follow +0x144; "blend" sets +0x145 to the old mode
and a timer +0x146 / rate +0x148 that only change the spring of a third point, +0x110,
which nothing in the follow camera reads, so the kit can drop them):

| From | When | To |
|---|---|---|
| 0 | state type fall (2) or jet (30) | 1 |
| 0 | heli jump (15) | 3: rise to ground below + 3.0 over 85 frames |
| 0 | crouch flip (12) | 8 |
| 0 | flip (11), unless strafing with the right stick's y above 0.3 | 6: rise to his position + 1.0 over 35 frames |
| 0 | rocket jump (13) or double jump (14) | 5: rise to ground below + 3.5 over 45 frames |
| 0 | any other jump-type state (type 4) | 2 |
| 2 | state 134 (jump pad) or 95 | 1 |
| any but 1, 9 | jump attack (20) | 9 |
| 1, 2 | state type not jump, fall or jet, and state not 134 | 0, at once |
| 3 | flip (11) | 6 |
| 3, 5 | state type not jump | 0 |
| 5 | crouch flip (12) | 8 |
| 6 | not flipping | 0 |
| 7 | past the apex and falling (type 2) | 1 |
| 7 | not past the apex | 6 again (+1.0 from the ground below, 35 frames) |
| 4 | type not jump | 1 if falling, else 0 |
| 8 | not crouch flip | 0 |
| 9, 10 | falling, or more than 54 frames off the ground | 1 (and the view direction eases back over 90 frames) |
| 9 | jump attack over | 10 |
| 10 | after 30 more frames | 0 |
| any | state 121 (falling out of the level in UYA's single player; libdl's "bounce") | 11 |

**The height, each frame:**
- **Modes 0, 1, 4 and 11:** the hero's height, exactly.
- **Mode 2, the jump hold:** keeps the height it had, with three ways out:
  - while on the ground, or past the apex with fewer than 16 frames to land, if the
    ground below is at or above the held height, the height jumps to it (landing higher
    snaps up at once; the look point's spring smooths it);
  - a wall jump (17) switches to mode 1;
  - past the apex and more than 0.2 u above the ground, when the hero's head (position
    + look height) drops below the camera's view axis (an angle, in the camera's
    vertical plane, under −0.01 rad, or over π/2), it switches to mode 1 and follows him
    down. In take 3 it switched on the frame the angle went from 0.0099 to −0.0236.
    The axis is last frame's view, the update camera's forward row (0x3E2638), not the
    line to the aim ("Placing and aiming the camera").
  - Landing lower ends the jump's state type, so mode 0 takes the hero's height at once.
- **Modes 3, 5, 6 and 7, timed rises:** `lerp(start, target, k / N)` on the k-th frame,
  linear. A 0.2 u sphere at the camera (mask 0x12) that touches anything stops the rise
  where it is. When the timer runs out, mode 6 becomes 7, which comes back down to the
  start over 35 frames; 3, 5 and 7 become 4.
  - Verified: the heli jump rose 3.0 over 85 frames, the flip 1.0 over 35 and back.
- **Mode 8:** held.
- **Modes 9 and 10:** halfway between the hero's height and the height at the start of
  the jump attack.
- **Mode 11:** the follow point freezes where it is at the look point's height, and the
  camera aims straight at the hero (0x3E7B50). Only a jump attack or a camera reset
  leaves mode 11 **[code]**.

**The look point's vertical spring in the air** (0x3E42C0) **[verified]**: while the hero
isn't on good ground, and for 15 frames after he lands (follow +0x174 counts down), the
look point's vertical gain is `0.002 + 0.0105 × min(gap, 2) / 2`, where gap is last
frame's distance between the look point's height and the follow point's (+0x170). So it
barely moves for small gaps and reaches the normal 0.0125 at 2 u. Simulated, the look
height matches the five takes to 0.0007 u; without this rule it is off by 0.42 u.

**Platforms** (`camera_platform_ride`, 0x3E2790) **[verified]**: the move that carried
the hero this frame (`CameraHeroData` +0x70, 0x1E2C80) is added to the camera, the look
point, the follow point, its held height and the rise target. Its z goes through a slow
copy (update object +0x98) that moves at most 0.01/60 u a frame.
- **Quirk:** standing on Veldin's crates reports a carried z of 0.015 u a frame, so the
  slow copy builds up to about 0.0125 and then leaks into the held height after he jumps
  off: in take 3 the held follow point crept up 0.35 u during a jump. On the crates the
  look point also sits about 0.15 u above the follow point.
- **In the kit:** carry the points by the platform's real movement; leave the leak out.

## Obstacles and collision [UYA NTSC, 2026-10-16]

Read in `camera_obstacles` (0x3E64E0) and its helpers, and in `camera_follow_motion` and
`camera_place`. The pull-in's ease and growth are verified on `uyantsc-phoenix-camwall-1`
(Starship Phoenix, against a corridor wall); the rest is **[code]**.
- **The pivot (clearance point, follow +0xB0)** (0x3E39E0) **[verified on the floor
  side]**: two 20 u rays, up and down, from the hero + 0.5 u (2.25 for critter mode 2,
  0.4 for 1, none while swimming), mask 0x94. With r = the clearance radius (below):
  - the gap between the hits under 2r: the pivot is their midpoint;
  - else, the ceiling more than height + r above the follow point: the pivot is the
    higher of the floor hit and the follow point, plus the height (on Veldin it was the
    floor on the 12 frames the floor was above the held follow point, else follow +
    height);
  - else the pivot is r under the ceiling;
  - in the first and third cases the height target becomes the pivot's height above the
    follow point, spring gain 0.006, for that frame.
  - **Verified under a low ceiling** (`uyantsc-veldin-camceil-{1,2}`, a crate written over
    the hero's head, since Veldin has no low ceiling): with its underside 2.6 u above his
    feet the pivot sat 1.05 under it (1.546 above his feet), and with it 1.9 u up, exactly
    halfway between floor and ceiling (0.948); each time the height sprang there at gain
    0.006, damping 0.2 (to 0.0004 u over 297 frames). So the clearance rays see mobys.
- **The clearance radius** r = 0.35 + growth × scale × (count − 1) = 0.35 + 0.1 × 1 × 7
  = **1.05 u** at rest (follow +0x254, +0x24C, +0x250, reset each frame by 0x3E4008;
  critter mode 1 makes the scale 0.6, critter 2 makes it 1.5 with growth 0.105).
  - Note: 0x3E77B0, quoted before as the clearance test, is inside `camera_manual_input`;
    the radius term is in the functions below.
- **Order each frame** (0x3E64E0, after the drag and the stick):
  1. **Objects near the arm:** mobys within `(distance − pull-in + 1.5) / 2` of the
     midpoint between the pivot and the camera. Ten classes (0x477, 0xB, 0x1AD6, 0x1ADA,
     0x1944, 0x18DE, 0x1F89, 0x1CAA, 0x109E, 0x109F) within 25° of the arm's direction
     turn it 1° a frame away from them. Other collidable mobys whose collision header's
     word +8 is 0 are hidden from the camera's collision for the frame. Classes 0x191F
     and 0x1920 set the growth to 0.04.
  2. **The sweep along the arm** (`camera_arm_sweep`, 0x3E6128; skipped while +0x25E is
     set, as in states 80, 125–127 and 129): spheres from the pivot outwards, radius
     0.45, 0.55, … 0.95 at 0.45, 1.0, 1.65, 2.4, 3.25 and 4.2 u (scaled by arm length /
     distance), mask 0x94, the hero ignored. Where one hits, `camera_arm_push` (0x3E5D00)
     moves the arm sideways until the arm's line clears the contact by
     `0.75 × (0.35 + 0.7 t)`, t the fraction along the arm, up to three tries. The arm
     keeps its length. The first sphere ignores contacts behind the pivot.
     - **Seen** (`uyantsc-veldin-camceil-3`, a crate written 0.4 u beside the arm's
       midpoint at its height): the desired arm swung 0.36 rad away from it within two
       frames and kept its 4.64 u, and the actual arm followed on its spring. So crates
       aren't among the mobys step 1 hides.
  3. **The camera end** (`camera_end_sphere`, 0x3E5930): if last frame's camera is more
     than r from the new desired spot (pivot + arm), a line between them (mask 0x14)
     stops it at the first wall, 0.005 u off it. Then up to three push-outs of a sphere
     of radius r (the world, mask 0x14, then a second query, below). The first contact sorts
     it: ceiling (normal · up over 0.75), floor (under −0.5) or wall.
     - **A wall** (`camera_set_pull_in`, 0x3E5728): the arm becomes pushed spot − pivot.
       If that is shorter (or falling, or inside a camera-blocker sphere), the pull-in
       becomes distance − its length, never leaving under 0.2 u of arm. If longer, the
       arm keeps its old length in the new direction.
     - **A ceiling or floor:** the arm takes the new direction at its old length. A floor
       contact also stops the stick tilting the camera further down (0x3E7668).
  4. **Turning into a wall:** in steps 2 and 3, a wall contact while the right stick's x
     is past 0.05 adds **0.075 u a frame** to the pull-in and sets its timer to 2180
     (critter mode 2 excepted) **[verified: steps of 0.075, or 0.15 when both fire]**.
  5. **The arm is never shorter than 1.5 u:** pull-in ≤ distance − 1.5 (timer 2000).
  6. **The pull-in's timer** (+0x244, 2000 at rest): with any contact this frame
     ("blocked"), a timer at 2000 or less is held at 2000 and the pull-in stays; above
     2000 it counts down without easing. Not blocked and either the timer is 1–1999 or
     the right stick is idle (both axes under 0.3): the timer counts down and
     **`pull-in ×= (1 − cos(π × timer / 2000)) / 2`** **[verified: the ratio matched to
     10⁻⁶ on every easing frame]**. So it hardly moves at first and is gone after about
     250 frames: from 2.66 it fell to 0.79 by timer 2000 (180 frames), then 0.39 by 1849.
     A flag (+0x24A, set when the camera starts further out than the distance) counts
     twice as fast.
  7. **Arm length** = distance − pull-in. The drag (step 3 of the follow camera) also
     shrinks the pull-in when the dragged arm comes out longer.
  8. **Reset when blocked** **[code; verified]**: a line from the pivot to the
     camera spot (mask 0x94 | 0x20, the hero ignored) that hits something not inside a
     camera-blocker sphere counts frames (+0x25C); at 30 the camera is re-made behind
     the hero (`camera_reset_follow`, 0x3F3918: 4.64 behind along his facing and 2.0 up, or 0.5
     short of whatever is in the way). It also resets at once when the current camera is
     more than 30 u from its spot, or, with the hero on good ground and the camera below
     the pivot, when a wall lies between them (under 20 u).
     - **Seen on Florana** (`uyantsc-florana-camvol-{1,3,4}`): each time the hero was
       written 45 to 180 u away, the next frame had the follow values at 4.64, 2.0 and
       1.5 and the camera behind him.
     - **The 30 frames, verified** (`uyantsc-veldin-camblock-1`, the hero written 1.5 u
       under the floor, where he fell): the camera stayed above the floor, the floor's
       underside blocked the line (and was the clearance rays' ceiling: mask bit 4 takes
       back faces), and the count rose by one a frame from 1 to 29. On the 30th frame the
       camera was re-made 4.64 u behind him along his facing and 2.0 u up, with the
       distance and height back at 4.64 and 2.0 and the count at 0.
- **The last push-out** (`camera_place`, 0x3E82F8) **[code]**: the camera (look point +
  height + actual arm) is pushed out of a 0.25 u sphere (mask 0x36) up to three times,
  and the same shift is added to the follow point, the look point and the rise target.
- **The second sphere query** (0x3FC030) **[code; data]** tests the third section of the
  level's collision file. Its header holds three offsets (`collision_load`, 0x4250B0):
  the terrain that `coll_line` uses (0x1EAA80), a list at 0x1DFC80 and a list at
  0x1DFC84. Wrench calls the first two `mesh` and `hero_groups` (groups of 16 bytes: a
  bounding sphere in 1/64 u, triangle and vertex counts, and an offset); the third is a
  list of the same groups that Wrench doesn't read, and only the camera's end sphere (and
  0x443D00) queries it, so it is probably geometry only the camera collides with.
  Veldin has one group (16 triangles in a 5.7 u sphere round (389.3, 155.9, 78.3));
  Florana has none.
- **The camera-blocker spheres** of steps 3 and 8 are the level's camera colliders
  (`camera_collider_at`, 0x3F51A8, from gameplay +0x88), which no UYA level has.
- **The collision masks** (2026-10-17). libdl's `CollisionFlags` (MIT) name the bits
  **[reported]**; the tests marked [code] were found in UYA's `coll_line` (0x3F8448):
  - bit 0 (1): skip the level's terrain and mobys that can't be damaged **[code: the
    terrain pass runs only with it clear, and mobys are skipped unless their mode word
    has 0x4000]**;
  - bit 1 (2): skip the hero's colliders and mobys' primitive colliders **[code: a moby
    primitive's flag is compared with it]**;
  - bit 2 (4): use a moby's other primitive type **[code: picks which of two primitive
    counts is read]**;
  - bit 3 (8): skip mobys that can't be damaged **[code, as bit 0]**;
  - bit 4 (0x10): libdl says "include surfaces with bit 0x10"; in UYA's triangle test a
    set bit 4 also accepts triangles facing away from the line **[code]**;
  - bit 5 (0x20): skip the one collision material whose number is in bits 8–12 **[code]**;
  - bit 7 (0x80): skip triangles whose material byte has 0x80 set **[code]**.
  - So the camera's masks are: 0x94 (the clearance rays, the sweep and the blocked line)
    = bits 2, 4 and 7, and 0xB4 under water adds bit 5 with material 0, which libdl calls
    swimmable water, so under water the rays pass through the surface; 0x14 (the camera
    end) = bits 2 and 4; 0x12 (the rise-stopping sphere) = bits 1 and 4; 0x36 (the last
    push-out) = bits 1, 2, 4 and 5 (water skipped again); the blocked line adds 0x20. None sets bit 0 or 3, so the camera
    collides with the terrain and with every moby that has collision, except the ones
    `camera_obstacles` hides each frame.

**Per-state parameters** (`camera_follow_params`, 0x3E42C0) **[code]**, applied each frame
before the motion and cleared at its end; lock-on and camera volumes are elsewhere:

| When | Look height | Distance | Height | Other |
|---|---|---|---|---|
| Critter mode 2 | 3.75 (gain 0.005) | 9.0 (0.003) | 6.75 (0.003) | snapped in the first 2 frames; clearance scale 1.5, growth 0.105; no run-at-camera push |
| Critter mode 1, no volume | 1.0 | 3.0 (0.003) | 2.0 | clearance scale 0.6; run-at-camera far end 4.0 |
| Jet (type 30) | 0.75 (set) | | | look vertical spring 0.04 / 0.2 |
| Swimming under water (type 17) | 0.25 (set) | | 0.5 (0.003) | ray mask 0xB4 |
| Gliding (type 5) | 0.25 (set) | | 2.5 (1.5 for critter 1) (0.003) | |
| Sliding (type 16) | 0.25 (set) | | | |
| Jump pad (134) | | | | look point = follow point (spring 1.0 / 1.0), vertical 0.06; aim blended to the look point; no push |
| Rocket hover (128) | | 3.0 (0.003) | | look spring 0.04; steered to his facing; growth 0.18; no push |
| Strafing, 131 or 135 with L2/R2 held | | | | no drag; aim blended to the look point; look spring 0.04 (after lock-on's, so it wins) |
| Skydive (type 37) | | | | no drag; look spring 0.005 / 0.1 |
| Desired camera 13 with a heading | | | | steered to it at 0.2094 rad a frame |
| States 80, 125–127, 129 | | | | no sweep and no blocked count |
| A boss set | | | | steered to it as lock-on, with lock-on's look spring and aim; the right stick suspends it for 240 frames |

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
- **The look point and the aim:** with a target, each frame also sets the look point's
  horizontal spring to 0.03 / 0.2 and turns on the aim's blend towards the look point
  (step 8). Deadlocked's (0x51A660) does the same, with the target at hero + 0x5F0,
  which DL's campaign never sets (`hero-controller.md`, "Deadlocked's lock-on").
- **In the kit:** `rac_hero::camera`, `lock_turn`; its profile's `lock_turn`,
  `lock_ramp_ticks` and `lock_look_spring`.

## Camera volumes and splines [UYA NTSC, 2026-10-16]

How a level's gameplay file steers the camera. The data's layout is in
`gameplay-format.md` ("Cameras", "Camera volumes", "Shapes", "Paths"); names and
addresses (Veldin's build) are in `re/SCUS-97353/symbols.toml`. Two separate mechanisms:
- **The level's cameras.** Each camera record is one camera mode (its class), the
  follow camera included. Each frame the game picks which one runs.
- **Camera volumes.** Mobys of class 2329 that, while the hero is in their cuboid, fill
  one of 32 override slots that retune the follow camera.

**Checked:** the camera records, their pvars, the cuboids and the volumes read from the
disc match what the game loaded in five savestates (`gameplay-format.md`). A volume and
Florana's flythrough were traced on 2026-10-17 (`uyantsc-florana-camvol-{1..4}`, "Traces");
each subsection says what that checked.
- **Florana's build** keeps the camera data 0x1740 lower than Veldin's (`GameCamera`
  0x1E1380, follow block 0x1E3E50, volume slots 0x1E45D0) and the core's globals 0x1680
  lower (the active volume at 0x1DE300, the camera list pointer at 0x1DE204). Its follow
  camera is the fourth level camera, so its update object is 0x1E1B20.

### Which of the level's cameras runs [code]

- **Each frame** `camera_manager_update` (0x3F3660) calls `camera_select` (0x3F1BE0) for
  player 1. That runs the current camera's pre-choose function (mode table column 4),
  then walks the up to 48 cameras in list order. Each one other than the winner so far is
  tested by `camera_mode_eligible` (0x3F1988) against that winner (at first the current
  camera); one that passes becomes the winner. If the winner changed, `camera_switch`
  (0x3F1670) switches to it. Then the winner's per-frame function (column 3) runs.
- **`camera_mode_eligible(candidate, current)`**:
  1. A candidate with priority 0 never takes over.
  2. Its class's own test (column 1) can say no (−1) or yes (1); 0 falls through. The
     follow camera's says no while the camera's requested class (0x1E2D0C, camera
     +0x2CC) is 4 or 0x51; the 0x17 camera's always falls through to step 3 after doing
     its work (below); the path camera's is below.
  3. Then its activation type (pvar +0x1F):
     - **0:** yes if there's no current camera, if the current one is ending (its update
       object's +0x7E is nonzero), or if the candidate's priority is higher.
     - **1, 2:** the same, but only while something has asked for it (object +0x7D).
     - **4:** the same priority rule, and the hero (the camera's copy of his position,
       0x1E2C50) must be in the camera's cuboid.
     - **7:** the requested class (0x1E2D0C) must be the candidate's, with the same
       priority rule; class 3 also needs the hero to be on the rail its pvar +0x24 names.
     - **3, 5, 6:** never by this rule.
- **There is no hysteresis.** A camera keeps running until another passes the test, and
  an equal priority never passes. A camera ends by setting its own +0x7E (the path
  camera sets 4); then any eligible camera takes over whatever the priorities, which
  hands back to the follow camera (activation 0, priority 5).
- **The switch** reads the new camera's blend type (pvar +0x1D):
  - after a camera that ended with +0x7E = 4: a cut;
  - **1:** a spring blend from the old view, at a rate of object +0x78 if positive,
    otherwise 0.018;
  - **5:** a timed blend over object +0x78 frames if positive, otherwise 40;
  - **3, 6:** no blend, but the new camera starts where the old one was;
  - **0, 2, 4:** a cut.
  - It keeps a copy of the follow block for the old camera to blend from. The blend is
    below.
- **Object +0x78** is −1 from the loader; something else (scripts, probably) sets it.
- **Blend types in the data:** the follow camera, 0x13, mode 5 and 0x21 have 0 (a cut);
  0x20 and 0x1E have 1; 0x3, 0x4, 0x7, 0x11, 0x17, 0x18 and 0x1D have 3. No UYA level
  has a 5, so the orbit blend below is only reachable from code.

### Blending between cameras [code; the lerp verified on UYA NTSC, 2026-10-17]

Read in `camera_blend_start` (0x3F2090), `camera_blend_step` (0x3F29F0),
`camera_blend_lerp` (0x3F22B0) and `camera_blend_orbit` (0x3F2480), which
`camera_manager_update` runs after the winning camera's update. The state is in
`GameCamera` from +0x2A0 (0x1E2D60 in Veldin's build).
- **State** (+0x2A0): 0 none, 1 started by a switch, 2 restarted while one ran, 3
  running. **Kind** (+0x2A3): 0 the lerp, 2 the orbit (record type 5); the running kind
  is kept at +0x2A2. A forced cut (`camera_switch`'s second argument, or the reset timer
  +0x458) clears the state.
- **Who starts which:** `camera_switch` starts the lerp for record type 1 (rate object
  +0x78, else 0.018 a frame) and the orbit for type 5 (+0x78 frames, else 40). The look
  and the lock-strafe camera start their own lerp at 0.075 a frame; leaving the look
  starts it at t = 0.25 with 0.018. Those rates and starting t are staged at +0x2B8 and
  +0x2BC (rotation), +0x2C8 and +0x2CC (position), and copied in when the blend begins.
- **The lerp** (kind 0), each frame: the camera's position is `lerp(start, new camera's
  position, ease(tp))` and its rotation `slerp(start, new camera's rotation, ease(tr))`,
  with ease the cosine ease `(1 − cos πt) / 2` (`cosine_ease`). Then tp and tr each add
  their rate, capped at 1, and on the frame after both reach 1 the blend ends and the new
  camera shows as it is. The start is the old camera's position and rotation at the
  switch; the start position also moves with the hero's platform (CameraHeroData +0x70).
  - **Verified** (`uyantsc-veldin-camlock-2`): into lock-strafe, t went from 0 to 1 at
    0.075 a frame and the camera reached the lock-strafe spot on the 14th frame; out of
    L2's first person, t began at 0.25 (0.325 on the first frame read) and took 10
    frames.
- **The orbit** (kind 2) works round the hero, in his frame at the start
  (`camera_blend_orbit_setup`, 0x3F1D30: the yaw of the camera's offset in his horizontal
  plane from his forward, its pitch, and its length). Each frame the yaw, pitch and
  length each move by a fraction f of what is left towards the new camera's (angles
  wrapped), with `f = 1 / ease(1, N, k / N)` for a blend of N frames with k left: 1/N on
  the first frame, rising to nearly 1 on the last. The camera goes at the hero plus that
  offset, turned to keep looking the way the new camera does. A restart lengthens N by
  one.

### Camera volumes: the follow camera's override slots [code; verified on Florana]

- **The slots:** 32 per player, 0x40 bytes each, at 0x1E5D10 in Veldin's build (0x1E45D0
  on Florana, 0x1E5790 on the Phoenix: they move with the level's build). The active
  index is at 0x1DF980, −1 for none. `camera_system_init` (0x3F3A38) clears them.
- **Who fills them:** a class 2329 moby (`camera_volume_update`, 0x3093A0), each frame:
  - It claims a slot when the hero's position (0x1A4C60) is in its cuboid (pvar +0x00,
    the cuboid test of `gameplay-format.md`) **and** its moby group (pvar +0x40) has a
    living member (`moby_group_count`, 0x442998: members whose state byte is not
    negative; group −1 counts none, so such a volume never acts).
  - Claiming (`camera_volume_add`, 0x3F08D8) takes the first free slot and sets its
    defaults: every float −1, priority 5, steer speed 6°, rest frames 120, lifetime −1,
    the rest 0. Then `camera_volume_fill` (0x309218) copies the pvars that are set.
  - It frees the slot (`camera_volume_remove`, 0x3F0980) on the first frame either test
    fails. There is no margin or delay either way.
  - With all 32 slots taken the claim returns 0 and the fill writes through it; no level
    comes close.
- **Choosing the active slot** (`camera_volumes_update`, called each frame from
  `camera_follow_params`): the in-use slot with the highest priority (+0x1C); a tie goes
  to the lowest slot. A slot with a positive lifetime (+0x38) counts it down and frees
  itself at 0; −1 lasts for ever. When the active slot changes from one that sets the
  field of view to one that doesn't, the field of view springs back to 1.11.
- **Applying it**, every frame, only while the current camera is the follow camera
  (each setter checks for class 0). Each field is skipped while unset:

  | Slot field | Effect |
  |---|---|
  | +0x00 distance | Distance target, spring gain 0.001 (default 0.003). Outside 4.64–6.0 it also turns off the push-back of step 6 |
  | +0x04 height | Height target, gain 0.001 |
  | +0x08 look height | Look-height target, gain 0.005 |
  | +0x0C tilt | The arm's tilt target in radians while the right stick's y rests (follow +0x204) |
  | +0x10 field of view | FOV target (default 1.11), gain 0.005, damping 0.2; springs back to 1.11 when the slot is freed |
  | +0x14 drag | Replaces the 0.85 of step 3 (0x1D5FE8) |
  | +0x18 | Look-point blend on (step 8: over 90 frames), the look point's springs 0.03 horizontally and 0.01 vertically (damping 0.2), push-back off, follow +0x10 cleared |
  | +0x19 | Look-point blend on, springs 0.03 and 0.0075 (never set from data) |
  | +0x1A stick lock | 1 ignores both right-stick axes, 2 the yaw, 3 the pitch |
  | +0x1E steer | Turns the arm towards the direction at +0x20 (horizontally), as lock-on does (`camera_steer_to`), at up to +0x34 degrees a frame (6) |
  | +0x1F steer when idle | The same, once the right stick has rested +0x32 frames (120); any push of more than 0.01 restarts the count |
  | +0x3C | Keeps the mode 4 camera from taking over (never set from data) |

- **The overrides last one frame:** `camera_end_of_frame` clears them, so the follow
  camera springs back to its defaults once the slot is gone. But it doesn't reset the
  distance and height gains, so they spring back at 0.001 too until something sets them
  again (the push-back sets 0.003). Nor does it reset the drag: that stays until
  `camera_follow_init` sets 0.85 again.
- **In the data** only distance and height are ever set: Veldin's one volume (distance 4,
  height 5, round the area at x 375, y 203) and Florana's two (distance 8, height 5).
  The kit can treat a volume as a cuboid with optional overrides and a priority.
- **Verified** (`uyantsc-florana-camvol-4`, the hero dropped into Florana's cuboid 21 and
  then walked out of it): slot 0 filled on the first frame inside (distance 8, height 5,
  priority 5) and the active index went to 0; the distance and height then sprang at gain
  0.001, damping 0.2, matching a simulation to 3 × 10⁻⁵ u over 213 frames. The slot freed
  on the first frame outside (y 453.06, the cuboid ending at 452.88). Afterwards both
  gains stayed 0.001 and the distance and height sprang back to 4.64 and 2.0 with them
  (to 4 × 10⁻⁶ u over 477 frames). During the flythrough the slot was claimed but not
  applied, since only the follow camera runs `camera_volumes_update`.

### The fixed-point camera, class 0x17 [code; data]

Every level has one, round the arrival point. It isn't a camera mode of its own: its
condition (`camera_fixed_update`, 0x3EC628) never takes over but, while it holds, works
the follow camera (`camera_fixed_track`, 0x3EC0B0) so that it sits at the camera
record's position.
- **It holds while** the game is in normal play, the current camera is the follow camera,
  the hero (the camera's copy) is in its cuboid, cylinder or sphere (in hero states 0x66
  and 0x67, the arrival, within 3 u of the cuboid's centre is enough), he is almost still
  (0x1E2D20 at most 1/60), and, if pvar +0x34 is set, the right stick isn't turning the
  camera. Holding L1 for more than 10 frames ends it too ("The other camera modes").
- **On its first two frames** it measures the camera's position from the hero: the
  horizontal distance (+0x38), the height (+0x3C), and the look height (+0x40) that
  gives the set pitch (+0x2C), and snaps the camera there.
- **Every frame after:** distance target +0x38 (gain 0.003, push-back off), height
  +0x3C (0.003), look height +0x40 (0.005), a held tilt (+0x28 degrees) and yaw (+0x44).
- **When it lets go** it sets +0x26 and never holds again in that visit.
- **The respawn camera of step 5 is this one:** in both Veldin savestates its +0x38,
  +0x3C and +0x40 hold 6.339, 1.206 and 2.758 **[verified]**.

### Path cameras: the spline camera, class 0x13 [code; data]

The only camera that follows a spline: a fly-by along one of the level's paths
(`gameplay-format.md`, "The path camera"). 31 on the disc, in nine levels.
- **Starting** (`camera_flythrough_cond`, 0x3EBD50): never once finished (+0x39). A
  script start (+0x38, set by `camera_flythrough_start`, 0x3EAB30, from moby code) takes
  over at once from any camera but another path camera. Otherwise it must outrank the
  current camera (6 > 5), the hero must be in its cuboid, cylinder or sphere, or near
  its path (0x4432C0), and his state and state type must match +0x42 and +0x43 when
  those are set. A set progress flag (+0xA0) stops it for good. Only Florana's starts
  by shape; the rest by script.
- **The flight** (`camera_flythrough_update`, 0x3EB2F0), after the start ramp below:
  - The position moves along the path's points at the speed (+0x3C u/s, or the path's
    length over +0x3A frames), s/60 u a frame, straight from point to point
    (`spline_advance`, 0x472C40, keeps the segment and the distance into it).
  - It looks at the point at the same fraction of the look path's length, or at the
    look moby's position plus the offset, with the hero's up.
  - Fields of view keyed at path points (a positive fourth float) are blended by distance
    between keys. No UYA camera path has one.
- **The ramps:** a global (0x1DF9D0; 0x1DE350 on Florana) rises by 0.05 a frame to 1
  before the flight moves, and falls the same way at the end, unless pvar +0x54 (start)
  or +0x55 (end) is set. It is a fade to black: the view cuts while it is at 1.
- **The hero** is held in state 116 for the flight unless pvar +0x41 is set.
- **The end:** when the path runs out, the frames run out, or +0x39 is set: it marks
  itself finished, sets +0x7E = 4 so the follow camera cuts back in, releases the hero,
  and starts the camera at +0x4C if there is one.
- **Verified on Florana** (`uyantsc-florana-camvol-2`, the hero dropped into cuboid 77):
  1. On the first frame in the cuboid the hero went to state 116 and the path camera
     took over with a cut; for 20 frames the view stayed where the follow camera had
     been while the fade rose by 0.05 a frame.
  2. At full fade the camera jumped to the path's first point, and the fade cleared by
     0.05 a frame while the camera moved along path 6 at exactly 5.00 u/s (the record's
     speed), straight between points, reaching its end (6.47 u) after 78 frames.
  3. Then the fade rose again over 20 frames; on the frame it reached 1 the hero was
     released (state 0) and +0x7E read 4. The next frame the follow camera cut back in,
     re-made behind the hero (4.64 back, 2.0 up), and the fade fell by 1/15 a frame. (A
     respawn's fade falls by 1/8.)

### Class 0x11, a follow-camera zone [code, Florana's build]

Only Florana and Metropolis have one. Its mode table entry (in Florana's build, at
0x2F4E80 + 4 × 20) has empty init, update and check functions; like the 0x17 camera, its
condition (0x3C38C8) never takes over but works the follow camera from 0x3C2F90 while
the hero is in its shape.
- **The shape test** (0x3C2DD0) tries, in turn, the cuboids at pvars +0x0C and +0x4C, the
  cylinder at +0x10, the sphere at +0x08 and the camera collider at +0x14.
- **It claims the follow camera** as the 0x17 camera does (follow block +0x260, the higher
  priority, +0x1C, wins) and gives it back when the hero leaves.
- **What it sets, each frame:** a heading steer towards the record's yaw (as lock-on
  steers, `camera_steer_to`) at up to pvar +0x00 degrees a frame, ramped in over 200
  frames (its frame count is +0x2E); the distance target (+0x24), height (+0x28) and look
  height (+0x30), at gains 0.003, 0.003 and 0.005 (0.002 each for subtypes 6 and 8); the
  tilt (+0x50, degrees); and the look point's horizontal and vertical spring gains
  (+0x38, +0x3C; follow +0x15C, +0x160), eased in from their old values over 120 frames.
  A zero leaves that field alone.
- **Subtypes** (+0x2C) change the details: in 1, 7, 9 and 10 a push of the right stick
  jumps the count to 200, which stops the steer until the count passes 400 and starts
  again from 1; 2 and 4 hold the count at 200 and turn the push-back off; 10 and 11 set
  bits of the follow block's +0x1F0 and a value at +0x270, which 6 and 8 test before
  acting.
- **Florana's** (camera 1, cuboid 81) is subtype 0 with distance 3 and height 4.
- Not traced, and the subtypes aren't read in full.

### Not read yet / next steps

- Done 2026-10-17: a volume and the flythrough traced; the blend code; the fade global;
  class 0x11's code in Florana's build.
- The trigger fields +0x00, +0x18 and +0x1E, and what sets object +0x78 and +0x7D.
- The classes 0x3, 0x18, 0x1D, 0x1E and 0x21, whose code is only in other levels'
  builds (Crash Site, Obani Gemini, Aridia and others, the vid-comics, multiplayer), and
  0x11's subtypes in full.
- The Phoenix build's other volume-slot caller (0x32C790 there).

## Lock-strafe and first person [UYA NTSC, 2026-10-16]

Read in Veldin's build (addresses in `re/SCUS-97353/symbols.toml`). **The first-person
and lock-strafe views are not worked out by the camera modes.** The hero keeps his own
free-look camera, a block at 0x1A6100 (libuya's `HeroFps` and `FpsCamVars`, whose field
order holds here), and two camera modes only copy it into the camera each frame.
Per-frame values are at 60 Hz.

### The setting and the hero's camera mode [code]

- **The setting** (`camera_setting`, the byte at 0x1439F4; it was called `aim_mode`):
  0 third person, 1 lock-strafe, 2 first person. A new game starts at 0.
  - The options menu (0x49CB48) and three quick-select items (0x24 third person, 0x25
    first person, 0x26 lock-strafe, handled in 0x41AB20) set it, each through
    `hero_camera_set` (0x510530). So **lock-strafe can be the default camera:** it is a
    standing setting, not a button mode.
  - **Each setting has its own invert and speed options:** three tables of 3 × 8 words
    (setting × player) at 0x143A10 (vertical invert), 0x143A70 (horizontal invert) and
    0x143AD0 (speed, 0–100?, default 50). `hero_level_start` fills player 0's entries
    from the stored options (four words per setting at 0x143964, 0x143994 and
    0x1439C4). The follow camera reads them too, at the
    setting's row. Default rows: vertical 1 for all three; horizontal 1 for third person
    and 0 for the other two. Both read as "not inverted" in their own code, whose sign
    conventions differ.
  - **With setting 1 or 2 the buttons move** (`pad_button_for`, 0x45E3B8, and 0x50EE20,
    Ratchet only): what R1 does goes to R2, what L1 does (the look) goes to L2, and L1
    jumps as X does.
- **`hero_camera_mode`** (0x1A6290; libuya's `fps.active`): 0 follow, 1 lock-strafe,
  2 first person. `hero_camera_update` (0x50EE90), near the end of `hero_tick`, sets it:
  - **Setting 0:** 2 from the frame the look camera is half blended in (the look-ready
    flag 0x1A71C5, below) while the hero is in look (state 1) or targeting (29), until he
    leaves those states; else 0.
  - **Settings 1 and 2:** the setting, whenever `hero_camera_allowed` (0x50FF18) says so,
    else 0. It refuses while the no-look timer (0x1A4E18) runs, in game modes 2 and 6,
    while a scripted camera (mode 5) is on, in state type 0x14 and a few scripted states,
    and when the hero is dead.
  - **In lock-strafe, L2 (the moved L1) gives first person:** in look or targeting with it
    held the mode becomes 2; let go, it goes back to 1.
- **`hero_camera_set`** (0x510530) sets the mode and its limits:

  | Mode | Eye height (Ratchet) | Camera offset | Look down / up limits |
  |---|---|---|---|
  | 0 | 1.75 | none | 89° / 89° |
  | 1, lock-strafe | 1.75, springing to 2.0 | 6 u back along the view | 70° / 60° |
  | 2, first person | 1.75 | none | 89° / 89° (70° / 70° while the flag 0x1D673C is set) |

  - While the mode isn't 0, the up limit then eases each frame towards 60° (45° in state
    types 3, the ledges, and 0x14) at 2° a frame, so the first-person setting ends up at
    89° down and 60° up too **[verified: 14 frames, take 3]**. But in lock-strafe, while
    L2 is held in look or targeting, `hero_camera_update` calls `hero_camera_set(2)`
    every frame, so the ease only ever takes one step and that first person keeps 89°
    down and 87° up **[verified, take 2]**. Letting go calls `hero_camera_set(1)` and
    keeps the offset at 2 u or more.
  - `hero_camera_set(1)` and `(2)` also clear the free look's flag bit 0, copy the camera's
    forward into 0x1A6110, and set the eye's height target (0x1A62AC) to 1.75 (Clank 1.1).
  - Clank's eye is 1.1 with an offset of 3 u; a third character type 2 (probably Giant
    Clank) has 5.0 and 9 u.

### Which camera mode shows it [code; the look verified]

- **The hero asks for a camera mode** through 0x1A73EC, which the camera copies to its
  +0x2CC each frame: `hero_change_state` asks 4 for look (state 1), 0x51 for targeting
  (29), 7 for the swing and grapple states and 0 or 3 elsewhere. With setting 1 or 2,
  `hero_tick` replaces the request with 0x20 every frame the hero camera is allowed,
  unless it is 4 or 0x51.
- **The mode table's four functions are, in order:** a condition, the init, the per-frame
  update, and a check the current mode runs each frame before any switch.
  - The camera manager (0x3F1BE0) runs the current mode's check, then asks every other
    update object's condition: 1 switches to it, −1 refuses, and 0 falls back on its
    level record's rule (always / priority / a volume / the request).
  - A switch (0x3F1670) sets up a blend and calls the new mode's init.
- **Mode 4, the look (setting 0):**
  - **Entering:** while the request is 4 or 0x51 and the follow camera is on, it counts
    frames and switches on the 7th; from mode 0x20 it switches at once (a cut). Until
    then the follow camera is steered towards the hero's facing, as lock-on steers it
    (`camera_steer_to`), at a turn speed of 0.0698 rad (4°) a frame. A level volume at
    the hero's position (0x3F51A8, flag bit 0) can forbid the look.
  - **Its init** blends rotation and position in at 0.075 a frame each, eased (0x4421E8):
    14 frames.
  - **Its update** copies the hero's free-look camera: its position (0x1A6150) and its
    matrix (0x1A6120). Until the look is ready, that is the follow camera's rotation at
    the eye point. Once the position blend is half done it sets the look-ready flag
    0x1A71C5, which puts the hero camera in mode 2 and lets the look state turn the hero;
    on the next frame it ends the position blend (the camera jumps to the eye) while the
    rotation blend runs on.
  - **Leaving:** when the request drops, it blends back to the follow camera from 25% at
    0.018 a frame (42 frames, 0.7 s), or cuts if its own blend hadn't finished.
  - **Afterwards:** if the follow camera then faces 80° or more away from the hero's
    facing, it is re-initialised (0x3F3918). Otherwise, only when the camera before it
    was mode 0x20, the follow camera is turned towards his facing at 0.1396 rad (8°) a
    frame for 30 frames.
- **Mode 0x20, the lock-strafe and first-person settings:**
  - Entered only from the follow camera, when the request is 0x20 and the no-look timer is
    0.
  - It blends in at 0.075 a frame and copies the hero's camera as mode 4 does.
  - It leaves when the request changes. Its check sets a rate of 0.045 (0x1DF768), but
    the follow camera's init overrides it with 0.075 after mode 0x20, so the blend back
    takes 14 frames. A request for 4 or 0x51 cuts instead.

### The free-look camera (0x410B08) [code; rates verified in the look]

Called from `hero_camera_update` each frame. Its block (0x1A6120 on) holds the matrix,
then yaw (+0x40) and pitch (+0x60) with their speeds, the constants below, the target
(+0x8C), the limits, the offset (+0xC0), flags (+0xE0) and the offset's current length
(+0x124). `fps_cam_init` (0x410840, at level start) sets the constants and the yaw to the
hero's yaw, pitch 0. **Positive pitch looks down.**

1. **Rate scale k** = (current field of view, 0x1E2EE4 / 1.1100293) ×
   `lerp(0.25, 1.25, speed / 64)` for the setting's speed option: **1.03125** at the
   default 50. Zooming in slows the turn.
2. **A target** (0x411918): the moby of category 5 that a 30 u ray along the view hits;
   otherwise, among the live ones within 30 u and within 15° (× the zoom) of the view,
   the lowest of distance × (1 + b) that is in sight from the camera. Its weight b runs
   from 1 dead centre to 0 at 15° (`(cos θ − cos 15°) / (1 − cos 15°)`); a moby hit
   by the ray gets 1. The ray's end is the aim point (+0x110).
3. **The stick:** the pad's right-stick floats (pad +0x100 x, +0x104 y; each axis is
   positive towards the raw byte's high end, so y is negative pushed up, with the dead
   zone of the left stick, `hero-controller.md`, "Input"). The yaw input is −x, the pitch
   input y, each negated when the setting's invert option is set; in look states the
   horizontal option is ignored. So with the horizontal option at 0, pushing right turns
   right; with the vertical option at 1, as all three are in our image, pushing up
   tilts the view down. Each input is then squared, keeping its sign (x·|x|), so half
   tilt gives a quarter.
   - **With flag bit 0 set (the look in the third-person setting),** each axis first
     takes the larger of the right and left sticks, and if that vector is shorter than
     0.5 the d-pad replaces it (pad +0x1B0, the real d-pad only). So a right stick under
     0.5 turns nothing in the look **[verified: 0.2105 for 1.5 s, no turn]**, while the
     lock-strafe and first-person settings, which clear the flag, use any stick past the
     pad's dead zone **[verified: at 0.2105 the yaw speed grew 0.0000886 a frame to
     0.001197, `0.001939 × 0.2105² × k` and `0.02618 × 0.2105² × k`; the pitch at 0.1974
     likewise]**. At 0.632 the look turns too, with the same squared response (0.000798 a
     frame to 0.010769) **[verified, `uyantsc-veldin-camhalf-0.75-1`]**.
4. **Yaw speed target:**
   - Normally `0.02618 × x` rad a frame (1.5°, **90°/s** at full tilt), reached at
     `0.001939 × |x|` rad a frame per frame (7.0 rad/s² at full tilt).
   - **A quick turn** while the squared x is past 0.99, at once without a target and
     after 8 frames with one: **0.10472** rad a frame (6°, **360°/s**), at 0.005818 a
     frame per frame (21 rad/s², 18 frames to full speed); a third of that with a target.
     It stops when the stick drops back.
   - With a target, the normal target speed is cut by `0.5 b`.
   - Pushing past 0.85 (squared) towards a target holds that input for 7 frames (the
     snap).
5. **Pitch speed target:** `0.0349 × y` rad a frame (2°, **120°/s**), reached at
   `0.001454 × |y|` (5.2 rad/s²); cut by `0.6 b` with a target.
6. **Keeping a target in view while strafing:** with a target and either stick pushed, the
   hero's velocity relative to the target's (capped at 0.1333 u a frame) over the
   distance d adds `0.666 b` of its sideways part to the yaw speed target (at most
   0.02618 a frame), and `0.85 b` of its vertical part to the pitch, faded out below
   d = 12 u to nothing at 4 u.
7. The targets and rates of items 4 and 5 are × k; item 6's aren't. **A speed drops at
   once** to a smaller target (the camera stops when the stick is let go) and rises
   towards a larger one by at most its rate.
8. Yaw and pitch add their speeds; pitch is clamped to the limits. The view direction is
   `(cos yaw cos pitch, sin yaw cos pitch, −sin pitch)`, and the matrix's rows are
   forward, left and up.
9. **Placing it** (0x411538):
   - The eye point E (below) is pulled back towards the hero's middle (0.7 u up) if a
     0.4 u sphere swept from there hits something.
   - **First person:** the camera is at E.
   - **Lock-strafe:** the camera is at E − forward × L. L springs out to the offset's 6 u
     (at most 0.00556 u a frame per frame, up to 0.333 u a frame). A ray from E to 6.5 u
     back that hits something cuts L at once to the hit distance − 0.5. A 0.2 u sphere
     over the last unit stops it at walls. So **the lock-strafe camera sits 6 u behind
     the eye along the view, and looks at the eye:** level with it when the view is
     level, and higher as the view tilts down.

### The eye point (0x510000) [code]

- **E = hero position + his up vector × h**, where h springs (0x445300: speeding up and
  braking at 0.00444 u a frame per frame, at most 0.533 u a frame) to:
  - 2.0 in lock-strafe;
  - 1.75 in first person, 1.0 in state 4 (crouching).
- **Jumps in lock-strafe:** while jumping (state type 4) less than 3 u above the ground,
  h loses the hero's vertical speed each frame, so the eye holds its height. Once he is
  3 u up it latches and follows him again.
- **Looking down** moves E forward horizontally by `0.45 × pitch / 90°` u.
- **Swimming** keeps E at least 0.75 u above the water.

### The hero in these modes [code]

- **He strafes when he holds anything but the wrench:** in modes 1 and 2, with a hand
  item other than 9, `hero_camera_update` sets `strafing` to 2 and his move angle
  (0x1A4D80) to the camera's yaw every frame. So the left stick moves him in the four
  strafing sectors, and he turns to the camera's yaw with the strafe turn
  (`hero-controller.md`, "Strafing"). In lock-strafe, firing (R1 or Circle in the last
  2 frames) triples that turn's gain and cap and keeps his jumps standing jumps.
- **With the wrench in hand nothing forces strafing**, so he runs and turns as in the
  follow camera, relative to the view, and the view doesn't follow him.
- **Standing, he turns to the camera only once it is more than 10° off** his facing, and
  steps round (state 2) until within 2°, as in DL (`hero_transitions` tests 10°
  (0.17453) and 2°).
- **In look and targeting** (setting 0, or L2 in lock-strafe), once 0x1A71C5 is set, his
  facing is set to the camera's yaw every frame (`hero_turn_to_move_angle` with gain 1:
  it snaps). The look state shares the idle's movement, so the left stick doesn't move
  him; letting go of L1 goes back to idle, or to a crouch with R1 held.

### Checked in traces [verified on UYA NTSC, 2026-10-16 and 17]

`uyantsc-veldin-camlock-1` (setting 0, the look; 494 frames, `docs/re-setup.md`):
- **Entering the look:** L1 pressed, state 1 and request 4 the next frame; mode 4 took over
  on the request's 7th frame; blend progress 0.075 a frame; look-ready when the position
  progress passed 0.5, then the position blend jumped to 1 and the hero camera mode
  became 2. A 3-frame tap of L1 never switched the camera.
- **Yaw:** full right stick gave a quick turn at once, its speed growing 0.0060 a frame
  (0.005818 × k, k = 1.03125); when a target appeared the growth fell to 0.0020 (a
  third). Let go, the speed went to 0 at once. Turning towards the target in the normal
  state, the speed grew 0.0020 a frame and was capped at 0.0173, which is
  `0.02618 × k × (1 − 0.5 b)` for the trace's b = 0.7165. After the stick was let go the
  snap carried the turn on and stopped it on the target.
- **Pitch:** grew 0.0015 a frame (0.001454 × k); with a target near the centre it was
  capped at 0.0144, `0.0349 × k × 0.4`. Pushing the stick up raised the pitch (the view
  tilted down), as the vertical option of 1 says. The up limit eased from 89° to 60° at
  0.0349 rad a frame.
- **Leaving:** the frame after L1 was let go, idle, request 0, the follow camera back,
  blending from 0.268 at 0.018 a frame.
- **Flags:** in this look the free look's flags read 11: flag 1 (off in the lock-strafe
  and first-person modes) stays set, so in code the look also takes the left stick when
  it is pushed further than the right, and the d-pad below a 0.5 stick.

`uyantsc-veldin-camlock-{2,3,4}` (2026-10-17; settings 1, 2, and 1 with a gun; written
as `hero_camera_set` would, flag bit 0 included):
- **Lock-strafe:** on the frame the setting changed, the hero camera mode went to 1 and
  the 0x20 camera blended in over 14 frames. The eye height sprang from 1.75 to 2.0 in
  13 frames, speeding up by 0.00444 u a frame per frame and braking to arrive. **The
  camera sits at last frame's eye − forward × 6** (its residual is under 0.001 u on 1,267
  of 1,535 frames; the others are collision). Pitch reached exactly 70° down and 60° up.
- **Turns:** the quick turn waited while a target was in the cone and began on the
  frame it left, growing 0.0060 a frame to 0.10799 (`0.10472 × k`), and grew only 0.0020
  while another target crossed the view; pitch grew 0.0015 a frame to 0.0360.
- **Collision:** looking up, the ray from the eye hit the ground and cut the offset at
  once (6 → 4.39), which then grew back; walking back towards a crate, the camera stopped
  5.67 u from the eye with the offset still 6, held by the sphere over the last unit.
- **Jumps:** through a standing jump (1.48 u high) the eye stayed within 0.0002 u. In a
  double jump it held until the hero was 3.07 u above the ground, then latched and
  sprang back to 2.0 above him, dipping to 1.67 at the landing and recovering in about
  12 frames.
- **The wrench** forced nothing: the view stayed still while he walked forward, right
  and back, turning to each direction relative to it.
- **L2:** first person cut in on the next frame (state 1, request 4, the eye at 1.75 at
  once, the camera exactly at the eye); his facing snapped to the view's yaw. Let go,
  the lerp blend took 10 frames from 0.25.
- **The first-person setting:** the eye stayed 1.75 above his feet through jumps (no
  hold), and the camera was exactly at last frame's eye on every frame.
- **A gun** (the Shock Blaster, take 4): from the frame it was in his hand `strafing` read
  2 and his move angle followed the camera's yaw. Standing, he began to turn once the
  camera was 12–15° off (10° in code, a frame or two late) and stopped 1.5° short;
  his turn matched the strafe turn (gain 0.042, damping 0.25) towards last frame's
  camera yaw.

### The other camera modes [code]

| Mode | Functions (condition, init, update, check) | What it is |
|---|---|---|
| 4 | 0x3E8DD8, 0x3E92A8, 0x3E8D18, 0x3E9350 | The look (above). |
| 5 | 0x3EA020, 0x3E9B18, 0x3EA028, 0x3EA088 | A scripted fixed camera: game code (0x3E9588; the level's arrival among its callers) sets its position and look-at point. Its condition is always 0. It stops the look while it runs. |
| 7 | 0x3EA9E8, 0x3EA090, 0x3EA9F0, 0x3EAAE0 | The swing and grapple camera ("Default follow position"). |
| 0x13 | 0x3EBD50, 0x3EAF20, 0x3EB2F0, 0x3EBE40 | A flythrough on two level paths (camera and look-at) over a set time, played once, with the hero held in state 116. |
| 0x17 | 0x3EC628, 0x3EC6F8, 0x3EC700, 0x3EC708 | Not a mode of its own: its condition never takes over, but while the hero is in its volume (0x3EBE48) it works the follow camera to sit at the record's point ("Camera volumes and splines", "The fixed-point camera"). Holding L1 for more than 10 frames ends it, and it keeps the no-look timer at 10. |
| 0x20 | 0x3EC7C8, 0x3EC730, 0x3EC828, 0x3EC870 | The lock-strafe and first-person settings (above). |

- **Every mode is a level record.** Veldin's camera list (pointer 0x1DF884, 7 records of
  0x20 bytes) holds a 0x17 at (366.2, 97.2, 79.8) near the start, a 0x13 at
  (230.6, 2.9, 70.5), and modes 0, 4, 5, 0x20 and 7 at the origin. Each record gets an
  update object of 0xB0 bytes at 0x1E3050.

### Not read yet / next steps

1. Done 2026-10-17: lock-strafe, the first-person setting and a gun traced (above); the
   blends' ease is `cosine_ease` ("Blending between cameras"); a part-tilted right stick.
2. Whether the vertical invert option's 1 is the game's default or our memory card's.
3. The first-person gun placement (0x50F410) isn't read.
4. **In the kit (2026-10-17):** done, but for the strafing a gun forces, which waits for
   the kit's guns ("In the kit").

## Deadlocked's camera [DL NTSC, 2026-10-16]

Read on DL NTSC-U's DreadZone Station image (addresses in `re/SCUS-97465/symbols.toml`) and
checked against three scripted traces, `dlntsc-dreadzone-cam-{1,2,3}` (`docs/re-setup.md`):
1, lock-strafe right-stick turns, pitch, walks and jumps; 2, L2 first person and back,
then the wrench; 3, third person. The keyboard gives full stick tilt only. Per-frame
values at 60 Hz, per second in brackets. The camera code is resident (0x51xxxx–0x53xxxx);
the hero's look code is in the level overlay (0x68xxxx) plus resident helpers (0x56xxxx).

### Where it lives

- **Camera 0** (libdl's `GameCamera`) is at **0x230280**, 0x470 bytes a camera (UYA's are
  0x460). libdl's 0x22CDC0 is empty in single player.
- **The mode table** is at 0x3C7B00: UYA's modes in UYA's order (0, 4, 5, 7, 0x13, 0x17,
  0x20) **[code]**.
- **Which mode runs [measured]:** third person is mode 0, the follow camera; lock-strafe
  is mode 0x20; first person is mode 4 (desiredCam 81, UYA's 0x51 request). Modes 0x20
  and 4 copy the position and matrix the hero works out (`CameraHeroData` +0xE0 and
  +0xB0, 0x2304B0 and 0x230480) **[code]**. So, as in UYA ("Lock-strafe and first
  person"), these views are the hero's free-look camera, here libdl's hero block
  `CameraPos` .. `CameraElevation` (hero + 0x1950 to + 0x1B04; fields in the layout
  `dlntsc-dreadzone-cam.toml`). DL's free-look update is 0x5620A0, UYA's 0x410B08.

### Third person: UYA's follow camera, 6 u back

- **The mode 0 functions pair up one for one with UYA's** (symbols file), and their
  constants are UYA's **[code]**, except:
  - **The distance is 6.0, not 4.64:** the init (0x51B550) and the base block both hold
    6.0 **[code; 6.000 throughout trace 3]**. The height (2.0) and look height (1.5) are
    UYA's.
  - **So running towards the camera no longer pushes it back:** `camera_approach_distance`
    (0x51EF38) still heads for its far end of 6.0, which is now the base distance. Only
    the stiffer length spring remains **[code]**.
  - **The camera-speed option is kept per control scheme** (0x171EF0 + player × 4 +
    scheme × 0x28; 50 in all). The orbit rate is UYA's formula **[verified: 0.05018 rad
    a frame (3.01 rad/s) at speed 50, trace 3]**.
  - The springs (0x535610, 0x5356C8) double gain and cap with two or more local players
    at 30 Hz; single player is UYA's.
- **Verified in trace 3:** the actual arm's yaw spring (0.015 / 0.2) matches 442 of 493
  one-step checks; the misses are torn capture samples.
- **At half tilt** (`dlntsc-dreadzone-camhalf-0.5-2`): the orbit is linear in the stick
  (0.01056 rad a frame at 0.2105, −0.00991 at −0.1974), and ±0.2 doesn't tilt it (the 0.3
  dead zone), both as UYA.
- **Not read:** some special-mode branches of `camera_follow_params` (flags at
  0x2304FB–0x230503) use other numbers (distance 6.75, height 2.75).

### Lock-strafe, DL's default [code; verified on trace 1]

Each frame, `hero_camera_type_update` (0x68BBA0) runs, in this order:

1. **Look angles** (`hero_look_update`, 0x5620A0):
   - **Input:** yaw from minus the right stick's x, pitch from its y (`PAD.analog`, pad +
     0x100 and + 0x104). **Pushing right turns right, pushing down looks down; positive
     pitch looks down** **[verified]**. Invert options per scheme at 0x171E78 (x) and
     0x171E00 (y), + scheme × 0x28 + player × 4; lock-strafe's are 0.
   - **Speed scale s** = (field of view / 1.1100293) × `lerp(0.25, 1.25, speed / 64)` =
     **1.03125** at the default 50 **[verified]**; × 0.4 while aiming the Fusion Rifle
     (hand item 5) in states 1 and 29.
   - **Yaw, normal:** speed target 0.05236 × x × s (3°, **180°/s**), reached at
     0.0019393 × |x| × s a frame per frame (7.0 rad/s²). UYA's is half: 0.02618.
   - **Yaw, fast:** once |x| has been over 0.99 for two frames, the target is 0.10472 × s
     (6°, **360°/s**) at 0.0034907 × s a frame per frame (12.6 rad/s²; UYA 0.005818). It
     drops back when |x| does **[verified: first frame 0.0020, then +0.0036 a frame to
     0.10799]**.
   - **Pitch:** target 0.034907 × y × s (2°, **120°/s**) at 0.0014544 × |y| × s (5.2
     rad/s²), as UYA **[verified: 0.0015 a frame per frame to 0.0360]**.
   - **A speed drops at once** to a smaller target, so letting go stops the turn dead
     **[verified]**; it rises towards a larger one by the rate above.
   - **Limits:** pitch within ±70° (1.2217) in lock-strafe (0x68E3B0); the up limit eases
     to −45° in state type 3 and +10° in type 0x14, at 120°/s **[code]**.
   - **View:** forward = (cos yaw cos pitch, sin yaw cos pitch, −sin pitch) **[verified
     exact]**.
   - **The stick isn't squared** (UYA's is) **[verified at half tilt,
     `dlntsc-dreadzone-camhalf-0.5-1`: at x = 0.2105 the yaw speed grew 0.000421 a frame
     to 0.011368, `0.0019393 × x × s` and `0.05236 × x × s`; the pitch at −0.1974 grew
     0.000296 to 0.007105]**. Anything past the pad's dead zone turns it; there is no
     further dead zone.
   - **Aim assist [code only, no targets in our traces]:** as UYA's, with a 35 u ray and a
     20° cone (0x19EC, × zoom) instead of 30 u and 15°; yaw cut by 0.5 b, pitch by
     0.6 b, the strafe pull 0.666 / 0.85, capped at 1.5° a frame; the 7-frame snap past
     0.85.
     - **Read further (2026-10-23):** with a target the quick turn starts on the 2nd frame
       past 0.99, as without one (UYA waits to the 10th), at a third of its acceleration
       as UYA. The strafe pull is UYA's in single player (motion capped at 0.1333 u a
       frame, the pitch faded by distance); on multiplayer levels it is halved (cap
       0.0667, × 0.5, at most 0.75° a frame), and with the hacker ray (item 13) active
       it is 1.53 × with a 0.2 cap, no limit or fade, the pitch a third.
     - **Its cuts** are in the hero's look block, as UYA's: quick turn 0 (hero+0x19C8),
       yaw 0.5, pitch 0.6, with the strafe pull's 0.666 and 0.85 and the cone after them
       **[read in the DreadZone savestate's hero]**. The ray hits a target's collision, as
       UYA's.
   - **In the look** (free-look flag bit 0) each axis takes the further pushed of the two
     sticks, as UYA's, but there is **no 0.5 floor** and no d-pad under it **[code]**.
     The look itself (camera mode 4, condition 0x520010) is UYA's: it switches on the 7th
     frame of the request, steering the follow camera to his facing at 0.0698 rad a
     frame meanwhile, and at once from mode 0x20 **[code]**.
2. **Camera position** (`hero_look_place`, 0x562E40):
   - **The eye guard only works with the eye held low:** UYA's 0.4 u sphere (0x21E754)
     swept from 0.7 u above the feet to the pivot, pulling the pivot back on a hit, runs
     only while the eye height (hero+0x1AFC) is under 1.15. So it acts in a jump's hold
     and the landing after, not at the lock-strafe's 2.0 or first person's 1.15 **[code]**.
   - **camera = pivot − forward × d**, d = 6.0 (`CameraOffset` (−6, 0, 0)). It orbits a
     6 u sphere round the pivot: at 70° down it is 5.6 u above it.
   - **The pivot used is last frame's** (the pivot is updated after the camera), so the
     camera trails the hero by one frame of movement (0.12 u at a walk) and is otherwise
     rigid: no lag spring, no drag, no recentring **[verified: residual under 0.0001 u
     on every walking frame; camera 0 equals it every frame]**.
   - **Collision:** a ray from the pivot to d + 0.5; a hit cuts d at once to the hit
     distance − 0.5; clear, d eases back out speeding up at 1/180 u a frame per frame
     (20 u/s²) up to 1/3 u a frame (20 u/s), braking to arrive (0x594FF0) **[verified:
     looking up into the floor cut d to 3.51 in two frames; it eased back to 6 over 38
     frames]**.
3. **Pivot** (`hero_eye_height`, 0x68DF20):
   - **pivot = feet + up × eye height**; target 2.0 in lock-strafe.
   - **Looking down moves it forward** horizontally by 0.45 × pitch / 90° (0.35 u at 70°)
     **[verified exact]**.
   - **The eye height moves** by the same speed-up-and-brake mover at 32 u/s² (0.00889 u
     a frame per frame) up to 64 u/s (1.067 u a frame); UYA's is half both.
   - **Jumps hold it** (state type 4, not states 0x1B and 0x83): each frame the eye height
     first loses the hero's vertical move, then its target is 2.0 − his height above the
     ground below. So **the pivot stays 2.0 above the ground under him.** At 3.5 u up
     (UYA 3 u) it lets go for the rest of the jump **[verified: through a 1.89 u
     standing jump the pivot stayed within 0.01 u; simulation within 0.009 u (standing),
     0.005 u (run jump), 0.11 u (double jump, the ground height being read every other
     frame)]**.
   - **Falls and landings** (state 6 is type 2) aim back at 2.0, so a double jump that
     ends in a fall lands with the eye at 0.86 and the camera dipped; it recovers in
     about 15 frames **[measured]**.
4. **The hero's facing:**
   - **The strafe turn is UYA's** (0x67BDE0): gain 0.042, damping 0.25, cap 19.2 rad/s
     (0.32 a frame) **[verified on 264 clean one-step checks]**. Its target (hero +
     0x2CE8) is **last frame's camera yaw rounded to 1/255 of a turn** (the network's
     camera angle) **[verified]**.
   - **Standing, he only turns once the camera is more than 10° off his facing**
     (`hero_transitions` calls 0x67BC90 with 10°); he then steps round (state 2) until
     within 2° **[verified: started between 9.9° and 11.7°, stopped at 1°]**.
   - Firing a gun (R1 or Circle within 2 frames) raises the turn to gain ≥ 0.2, cap ≥
     52.4 rad/s (0.87 a frame) **[code]**.
   - **With the wrench** (hand item 1, or item 0xF) the strafe flag is 0: the camera stays
     as it is, and he turns to where he walks **[verified, trace 2]**.

### First person: L2 in lock-strafe (state 29)

- **L2 held 10 frames** starts it; a 7-frame tap didn't **[measured]**.
- **Camera type 2** (0x68E3B0): offset 0, so the camera is at the pivot; eye height 1.15
  at once (UYA 1.75); pitch 89° down, the up limit easing from 85° to 70° at 120°/s.
  Same look rates **[verified for the eye, the limits and the cut]**.
- **In, it cuts:** the first frame of state 29 is already the eye view.
- **Out (L2 let go),** the hero's data is lock-strafe again at once (eye 2.0, d 6.0), and
  the camera blends from the old eye to the new place over 10 frames: t starts at 0.25
  and adds 0.075 a frame; position = lerp(old, new, (1 − cos πt) / 2) (0x536AC8)
  **[verified: t steps of 0.075]**.
- **The field of view** stayed 1.1100 rad (63.6°) throughout, first person included
  **[measured, Dual Vipers]**.

### Not read yet / next steps

1. Done 2026-10-17: a half-tilted right stick (no squaring, no extra dead zone).
2. The aim assist with an enemy in view; the blends between third person and the other
   modes; the special follow branches.
3. Where the blend's 0.25 start is set, and the L2 hold count's code.
4. In the kit (2026-10-17): done, as `dl.ron`'s default camera, but for the aim assist
   ("In the kit").

## In the kit (P1.7) [2026-10-17]

`rac_hero::camera`, with every number in the profiles' `camera` section (`uya.ron`,
`dl.ron`, each value labelled with its source here):
- **`FollowCamera`** shows one of three cameras: the follow camera (`follow.rs`, with
  `vertical.rs` and `obstacles.rs`), the third-person look, or the hero camera for the
  lock-strafe and first-person settings (`free_look.rs`), and blends between them with
  the cosine lerp at the games' starts and rates. `FollowCamera::setting` is the
  player's camera setting; `None` takes the profile's (UYA third person, DL lock-strafe).
  The demo hides the hero while the camera is at his eye.
- **The follow camera** has steps 1 to 9 of "How the follow camera works", the vertical
  modes the kit's states reach, the look point's air gain, strafing's drag-free arm,
  the aim's 90-tick blend to the look point while strafing or locked on, the view's yaw
  to the aim and its tilt from the arm and the heights with the look-up (2026-10-21:
  pointing the view at the aim had it jolt 35° up in a tick when a double or heli jump
  turned into a glide), the clearance point, the sweep, the camera-end sphere, the pull-in with
  its stick bump, timer and cosine, the 1.5 u minimum arm, and the 30-tick and 30 u
  resets. The stick turns it linearly and its tilt springs back, as the games do; the
  mouse turns it directly and its tilt stays.
- **The free look** has items 1 to 9 of "The free-look camera", with UYA's squared stick and quick turn at once, DL's
  linear stick and quick turn from the second tick, the limits and the up limit's ease
  (UYA's held look re-setting it each tick), the eye's speed-up-and-brake spring
  (0x445300; 13 ticks from 1.75 to 2.0, as traced), the jump hold, the look-down shift,
  the eye guard (DL's only with the eye under 1.15), and the lock-strafe offset with its
  ray cut, ease-out and (UYA) end sphere. The camera uses last tick's eye.
- **Its aim assist** (2026-10-22, `FreeLookAssist`; items 2, 4 to 6 and 0x410B08 read
  further in code): the target the view's ray hits (weight 1), else the lowest distance ×
  (1 + weight) in sight within 30 u and 15°, among the targets each better than all
  before it in the list, as the game builds it; the stick's yaw cut by 0.5 × weight and
  its pitch by 0.6 (the quick turn's speed uncut, constant +0x58 is 0); with a target the
  quick turn from the 10th tick past 0.99, at a third of its acceleration; a push past
  0.85 towards the target held 7 ticks at twice the rate, ending as the turn would pass
  the target, turning exactly onto it; and, with either stick pushed, his velocity
  against the target's (capped at 0.1333 u a tick) over its distance turning the view
  across by 0.666 × weight (at most 1.5° a tick) and up or down by 0.85 × weight, the
  pitch alone faded from 12 u to 4 [code]. The ray's hit is a target's middle within
  1 u of it [placeholder: the game tests the moby's collision]; the targets are the
  kit's `AimTarget`s. DL's has its 35 u, 20° and quick turn ("Lock-strafe, DL's
  default"). It is the stick's: once the
  mouse turns the view there is none until the stick does (§5, 2026-10-22).
- **The third-person look** waits 7 ticks while the follow camera steers to his facing,
  blends in at 0.075 and is ready at half way, takes the further pushed of the two sticks
  with nothing under 0.5, and blends out from 0.25 at 0.018 (a cut if its own blend
  hasn't finished), re-making the follow camera behind him if it is 80° off.
- **The field of view** (2026-10-22): the profiles' `fov` (1.1100293). A gun's zoom
  (`GunHand::zoom`, the Flux Rifle's scope; `weapons.md`, "Aim indicators") is a target
  and a spring, as `camera_fov_set` takes them (0x3F0FA0, mode 3), and the camera keeps
  heading for the last one asked. The free look's rates and, our own, the mouse's turn
  are × the field of view over the profile's (item 1). `FollowCamera::zoom` gives the
  ratio of the half-angles' tangents, by which `zoom_projections` narrows the render
  camera's own field of view (kept in `UnzoomedFov`). The demo's 45° vertical view isn't
  the game's.
- **`CameraVolume`**: a box with a priority and optional distance, height, look height,
  tilt and drag; the distance and height gains stay at 0.001 after it, as on Florana.
- **The hero:** `HeroState::Look` for the look button; the standing strafe turn past 10°;
  lock-strafe doesn't lock on. With a gun in hand (`HeroMotion::gun`, P1.9) the
  lock-strafe and first-person cameras make him strafe, and firing in lock-strafe triples
  the strafe turn (`weapons.md`, "In the kit").
- **Left out:** the platform carry (step "Platforms"), the swim and critter rules, the
  modes for states the kit lacks (jets, rocket jumps, jump pads, state 12, the bounce's
  mode 11), volume steering and fields of view, the view's extra yaw (+0x48), the orbit blend, the 8° turn-back after
  a look that followed the lock-strafe camera, and the zoom's narrowing of the assist's cone. The
  kit also sweeps the camera from the pivot at the end, so it never ends behind a wall.
- **Seen in the running demo** (Xvfb, 2026-10-17): DL's lock-strafe camera stays pointed
  down the level while the hero runs sideways; UYA's third person doesn't turn while he
  strafes, and swings round behind him when he runs.

## Partly seen

- **Field of view [measured, meaning unclear]:** on R&C3 PAL the FOV block holds 1.11
  (63.6° if radians) in the two fields libuya calls `speed` and `ideal`. The next field,
  `actual`, reads 0.005, so libuya's names may be off here.

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
- `source/Ratchet & Clank 3/uyantsc-veldin-camvert-{1..5}.trace` (2026-10-16, script
  `uyantsc-veldin-camvert.sh <take>`, layout `uyantsc-camera-obstacles.toml`, which adds the
  vertical-mode, clearance and pull-in fields): from `uyantsc-veldin-ledge.p2s`. 1: held,
  double and heli jumps, a back flip, a jump attack, run jumps. 2, 3, 5: grabs and climbs
  onto the crate stack, a walk off it, a jump down. 4: a run jump past the stack. 1,926,
  582, 556, 303 and 281 frames.
- `source/Ratchet & Clank 3/uyantsc-phoenix-camwall-1.trace` (script
  `uyantsc-phoenix-camwall.sh`, layout `uyantsc-phoenix-camera-obstacles.toml`, whose camera
  blocks sit 0x580 lower): from `uyantsc-phoenix-wall.p2s`, the right stick turning the
  camera into the corridor's walls and back, runs along and away from the wall. 1,278
  frames.
- `source/Ratchet & Clank 3/uyantsc-veldin-camlock-{2,3,4}.trace` (2026-10-17, script
  `uyantsc-veldin-camlock.sh <take>`, layout `uyantsc-veldin-camlock.toml`): the
  lock-strafe setting (turns, tilts, walks with the wrench, jumps, L2), the first-person
  setting, and lock-strafe with the Shock Blaster. 1,661, 1,569 and 605 frames. A first
  run of take 2 left the free look's flag bit 0 set, so the left stick also turned the
  view; it is kept as `-2-flag1` and not used.
- `source/Ratchet & Clank 3/uyantsc-florana-camvol-{1..4}.trace` (2026-10-17, script
  `uyantsc-florana-camvol.sh a|b`, layout `uyantsc-florana-camera.toml`, Florana's
  bases): 1, the hero dropped into the volume (he landed in water and drowned); 2, into
  the flythrough's cuboid; 3, out of the volume (water again); 4, into the volume and then
  walked out of it. 339, 357, 356 and 799 frames.
- `source/Ratchet & Clank 3/uyantsc-veldin-camceil-{1,2,3}.trace` and
  `uyantsc-veldin-camblock-1.trace` (2026-10-17, scripts `uyantsc-veldin-camceil.sh [3]`
  and `uyantsc-veldin-camblock.sh`, layout `uyantsc-camera-obstacles.toml`), from
  `uyantsc-veldin-ledge.p2s`: a crate written 2.6 u and then 1.9 u over the hero's head,
  then 0.4 u beside the arm; and the hero written 1.5 u under the floor. 400, 407, 269
  and 244 frames.
- `source/Ratchet & Clank 3/uyantsc-veldin-camhalf-0.5-{1,2}.trace` (2026-10-17, script
  `uyantsc-veldin-camhalf.sh <take> 0.5`, layout `uyantsc-veldin-camlock.toml`), on a boot
  with `AXIS_SCALE=0.5`, so every stick key gave half tilt (the byte 191 or 64, 0.2105 or
  0.1974 after the pad's dead zone): 1, the follow camera's turns and tilts, the look, a
  walk; 2, the same in lock-strafe. 769 and 699 frames. `-0.75-{1,2}` are the same takes
  on a boot with `AXIS_SCALE=0.75` (the bytes 223 and 32, 0.6316 and 0.6184).
- `source/Ratchet Gladiator/dlntsc-dreadzone-camhalf-0.5-{1,2}.trace` (2026-10-17, script
  `dlntsc-dreadzone-camhalf.sh <take> 0.5`, layout `dlntsc-dreadzone-cam.toml`), on the
  `dl-camB` boot with `AXIS_SCALE=0.5`: the right stick each way and a walk, in
  lock-strafe (1) and third person (2).

## Next steps

1. **Done 2026-10-12:**
   - The follow lag, drag and distance are read on UYA NTSC and checked against
     `uyantsc-veldin-cam-1`.
   - The 0x3EA2F0 function with the `approach` steps is the swing and grapple camera
     (states 43 and 36), not the normal follow. Its NTSC steps are 0.1333 (distance) and
     0.0667 (height) u per frame: PAL's × 5/6.
2. **Done 2026-10-16:** the vertical modes, the look point's air gain and the platform
   carry ("The follow point's height"), verified on `uyantsc-veldin-camvert-{1..5}`.
   Not traced: rocket jumps, crouch flips (mode 8), gliding, swimming, jets, the bounce
   (mode 11) and the rise-stopping sphere.
3. **Done 2026-10-16 in code, the pull-in verified:** obstacles and collision ("Obstacles
   and collision"). **Done 2026-10-17:** the low ceiling's two cases, the sweep's push and
   the 30-frame reset traced; the masks' bits, the second sphere query and the
   camera-blocker spheres named. Still open: in the Phoenix take the right stick held the
   arm still against something beside it (frames 74–157) with no contact reported.
4. **Done 2026-10-16:** the look, lock-strafe and first person on UYA, read in code and
   the look traced ("Lock-strafe and first person"); Deadlocked's follow camera,
   lock-strafe and first person, read and traced ("Deadlocked's camera"); the level's
   cameras, camera volumes, the fixed-point and path cameras, read in code with their data
   checked against five savestates ("Camera volumes and splines").
5. **Done 2026-10-17:** UYA's lock-strafe, first-person setting and gun traced; a volume
   and Florana's flythrough traced; the blends between cameras read and the lerp
   verified; class 0x11 read in Florana's build; part-tilted right sticks on both games
   (UYA's free look squares the stick and its look ignores a stick under 0.5; DL's
   doesn't square it; both follow cameras are linear).
6. **Open, at lower priority** (details in each section's own list): the follow
   camera's 1° extra tilt at 0.632; whether the vertical invert option's default is the
   game's (the options menu, 0x49C368, only copies its widgets); the first-person gun
   placement (0x50F410); the camera-only collision groups (a trace through Veldin's one);
   the classes only other builds hold (0x3, 0x18, 0x1D, 0x1E, 0x21) and 0x11's subtypes;
   what sets object +0x78 and +0x7D; DL's aim assist with a target, its special follow
   branches, where its blend's 0.25 is set and its L2 hold count; untraced vertical modes
   (rocket jumps, crouch flips, gliding, swimming, jets, the bounce).
7. **Done 2026-10-17 in the kit (P1.7):** everything this list asked for ("In the
   kit"). Next for the kit: replay RE.4's traces' hero data through `FollowCamera` and
   compare the cameras frame by frame, and the parts "In the kit" leaves out.

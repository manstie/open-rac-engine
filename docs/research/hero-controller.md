# Hero controller: how Ratchet moves

How Ratchet moves in R&C3/UYA and Gladiator/Deadlocked (PLAN.md RE.3, fed by RE.5 and
RE.8). It is the reference for the hero kit's controller (P1.5) and its feel-matching tests
(RE.6). Last updated 2026-10-12 (wall jump, ledges, hits and slope limit; then DL's
charge boots, the skid state's braking and the moves that need items).

## Status and method

- **The numbers come from the game code.** We read the hero code in Ghidra in four memory
  images:
  - R&C3 PAL (SCES-52456) on Veldin and Gladiator PAL (SCES-53285) on DreadZone Station,
    from the savestates of the owner's input recordings;
  - UYA NTSC-U (SCUS-97353) on Veldin and Deadlocked NTSC-U (SCUS-97465) on DreadZone
    Station, from savestates taken by booting a new game headlessly
    (`docs/re-setup.md`, "NTSC savestates without a capture session").
- **Checking against the game:**
  - **PAL:** four replayed recordings of the owner's free play (7,500 frames).
  - **UYA NTSC:** three traces we made ourselves (4,300 frames, keyboard input driven by
    xdotool; `docs/re-setup.md`). They cover moves the free play didn't: held and tapped
    jumps, long jump, heli jump, glide, strafe flips and the jump attack.
  - Every trace holds the whole hero struct each frame. That includes the jump block and
    the stick values the code works with, so each model below was compared with them
    frame by frame.
- **Labels:**
  - **[verified]**: read from the code and reproduced exactly by the traces. The label
    says which region was checked.
  - **[code]**: read from the code, not yet seen in a trace.
  - **[measured]**: from the traces only, with no code read yet.
  - **[reported]**: from libuya or libdl, unchecked.
- **Where things are:** function and data addresses for all four builds are in
  `re/<serial>/symbols.toml`. Hero fields are in `re/<serial>/hero.toml`. The hero code is
  part of each level's overlay (see `docs/research/engine-notes.md`), so code addresses hold
  only for the level they were found in.
- **Units:** distances are world units (u), which the community treats as metres. z is up.
  Values are given **per second at the source** and, where the code holds per-frame
  numbers, as the native NTSC per-frame value with PAL's in brackets. t is the frame count
  in the current state; the game counts it after the update, so a rule written for
  `t > 12` first acts on the 14th frame.

## Frames and regions (RE.8)

**Answer: both engines are written in per-second units for 60 Hz. NTSC-U is the native
tuning, and each PAL release is a conversion that keeps the real-time behaviour.**
- **Speeds and accelerations [verified on R&C3 PAL and UYA NTSC]:**
  - The source values are the same in every region: for example jump gravity 29.7 u/s²,
    walk speed 6.17 u/s (R&C3/UYA) and 7.25 u/s (Gladiator/DL).
  - R&C3/UYA fold them into per-frame literals when compiled: gravity is 0.00825 per
    frame on UYA NTSC (29.7/3600) and 0.01188 on R&C3 PAL (29.7/2500). Some values are
    scaled at run time instead (walk speed × 0.02 on PAL).
  - Gladiator/DL multiply them at run time by frame-time globals:
    - `frame_dt` is 1/60 on DL and 1/50 on Gladiator;
    - `frame_dt2` is its square;
    - a ratio global holds 1.0 on DL and 50/60 on Gladiator.
- **Frame counts [verified on UYA NTSC for jumps, code elsewhere]:**
  - Counts are written for 60 Hz: the hold time of a jump, the push-off frames, and the
    double jump's boost window.
  - PAL uses about 5/6 of each, rounded to the nearest frame with halves rounded down.
    Examples, NTSC → PAL: 17 → 14, 14 → 12, 9 → 7, 8 → 7, 5 → 4, 36 → 30.
  - A few counts are scaled at run time on PAL as `(n × 5 + 2) / 6`, in integer
    arithmetic.
- **Exceptions [code]:**
  - R&C3 PAL's turn damping values are UYA's × 1.44: for example 0.27 per frame becomes
    0.3888. That rescales a per-frame factor as if it were an acceleration, so PAL turns
    are slightly more damped than NTSC's in real time.
  - UYA's heli-jump thrust is 49 u/s², and R&C3 PAL's is 48.
- **For the hero kit:** run at 60 Hz with the NTSC values. They are the originals, not a
  conversion.
- **Earlier PAL measurements stay valid as PAL facts.** Every per-frame number measured
  on 2026-10-11 is one of the PAL per-frame values below.

## Input

The hero reads the left stick as two floats that the pad layer prepares, and it falls back
to the d-pad when the stick is near the centre.

1. **Per axis, in the pad layer [verified on R&C3 PAL]:**
   - The raw byte v (0–255) gives `clamp((|v − 127| − 48) / 76, 0, 1)`, with the sign of
     `v − 127`. So there is a 48-count dead zone, and full output 124 counts from centre.
   - Stick y is negated, so forward is positive.
   - This fits 3,150 of 3,183 frames across both R&C3 traces exactly, two frames after
     the recording frame. The code that does it is in the pad library and not located yet.
2. **Cut-off, in the hero code [verified on R&C3 PAL, code on UYA NTSC]:**
   - If the vector's length is under 0.25, the stick is ignored and the d-pad is used
     instead, each direction giving ±1.
   - That rule is in `hero_update`, and it explains the other 33 frames: 24 small
     vectors zeroed, plus 9 frames in a scripted state.
   - Together with the dead zone, an axis has to be pushed about 67 counts from centre,
     half its travel, before Ratchet moves.
3. **Magnitude, in the hero code [verified on R&C3 PAL]:**
   - `m = clamp((max(|x|, |y|) + length) / 2, 0, 1)`.
   - A diagonal therefore counts as fuller than its Euclidean length.
4. **Direction:** the stick vector rotated by the camera yaw.
5. **Buttons on UYA [verified on UYA NTSC]:**
   - X jumps.
   - R1 crouches.
   - Square swings the wrench.
   - L1 enters first-person look (state 1).
   - L2 or R2 strafes.

## Ground movement

R&C3/UYA, with UYA NTSC per-frame values and R&C3 PAL values in brackets.

- **Walk target speed [verified on both]:**
  - `6.17 u/s × m`.
  - Gladiator/DL use 7.25 u/s, which is `maxWalkSpeed` in libdl's `HeroPlayerConstants`.
    Gladiator's running speed was measured at 7.25 u/s too.
- **Speeding up [verified on both]:**
  - The speed steps towards the target by 37 u/s², 0.010278 per frame (0.0148).
  - So top speed takes 10 frames (9).
- **Letting go of the stick [verified on UYA NTSC]:**
  1. Releasing the stick sets the walk state's skid flag (below), so for the next two
     frames it brakes at the skid rate: 17 u/s², 0.0047222 per frame (0.0068).
  2. Then the hero switches to idle with a residual speed of at most 1.5 u/s: 0.025 per
     frame (0.030).
  3. That speed shrinks by 17.7 u/s², 0.0049167 per frame (0.00708), so he stops about 6
     frames later.
  - The R&C3 PAL traces agree: every walk-to-idle switch drops to 0.030.
- **Other braking [code]:**
  - Walking with the stick pulled back to a lower target speed: 30 u/s², 0.008333 per
    frame (0.012).
  - Crouching: 27 u/s², 0.0075 per frame (0.0108) **[verified on UYA NTSC]**.
- **Skid [code]:** there are two different skids.
  - **The walk's skid flag (state 2) [code, UYA NTSC `hero_update_state`]:** with
    `err` the angle between the stick's direction and the facing, and a full stick:
    - **Starts** when the hero is slower than 1.7 u/s and `err` is over 120°, or when the
      stick's direction swung more than 110° in the last 6 frames and `err` is over 70°.
      - The pad keeps the stick's angle for each of the last 30 frames (`atan2`, with 0
        for a released stick, which is "right").
      - So a stick pushed left from neutral counts as a 180° swing.
    - **While it is set:**
      - the target speed is at most `6.17 − err × 5.5/π` u/s;
      - it is then scaled by `clamp(150° − err in radians, 0.2, 1)`;
      - it brakes at 17 u/s² (0.0047222 per frame) instead of 30;
      - it turns with gain 0.035, damping 0.27 and cap 0.1949 rad.
    - **Ends** when `err` is under 4°.
    - **Letting go of the stick** sets it too, only after the walk has seen a full stick
      under a condition on the hand slot not yet understood. That is the release braking
      above.
  - **Moving the stick's way [code]:** under 1.7 u/s the walk moves in the stick's
    direction rather than along the facing, until the facing is within 5° of it. A skid's
    start sets this too.
  - **Part-pushed stick [code]:**
    - under 0.75 of the stick and more than 35° round, the target speed is cut by
      `1 − 0.4 × err` (floored at 0);
    - under 0.9 and more than 45° round, the speed doesn't rise, so the hero turns first.
  - **The skid state (3):** after a long jump's landing, and after DL's charge. It brakes
    at **27 u/s²** (0.0075 per frame on UYA, `27 × frame_dt²` on DL), scaled down on
    slippery ground, and doesn't turn. Read in UYA's and DL's idle-type update, which
    states 0, 1, 3 and 4 share.
  - **Ways out of state 3, the same in UYA and DL:**
    - X pressed this frame: a standing jump (7). R1 held: a crouch (4). Both only while
      input isn't locked.
    - Off the ground for 9 frames and more than 0.3 u up: a fall.
    - The animation ending: idle. Otherwise it brakes until the speed is 0, then idles.
- **Staying on the ground [verified on both]:**
  - Every frame on the ground the intended velocity gets a −54 u/s² downward push,
    −0.015 per frame (−0.0216). Collision cancels it.
  - Gladiator's `move_vel` shows the same −0.0216.
- **Turning:** the yaw follows the move direction as a damped spring. Each frame:
  1. The yaw rate is clamped so it cannot pass the remaining angle.
  2. It gains `gain × angle error` and loses `damping × rate`.
  3. It is capped.
  4. The yaw advances by the rate, and snaps to the target when within 1% of the cap.

  The walk state uses two sets of values. While setting off from a standstill (substate 0,
  until the speed passes about 2.5 u/s, which takes 4–6 frames), they scale with the stick
  magnitude m. Once moving (substate 1, and straight away when landing from a jump) they
  are fixed. UYA's walk animation code switches the substate by speed **[code]**:
  - to moving above 2.35 u/s, and back to setting off below 1.9 u/s;
  - only after the walk's 4th frame.

  The kit keeps a single 2.5 u/s threshold, which gives the same frame from a standstill.
  The two sets:

  | | UYA NTSC, per frame | R&C3 PAL, per frame |
  |---|---|---|
  | setting off: gain | `clamp(0.02 m, 0.01, 0.022)` | `clamp(0.0288 m, 0.0144, 0.03168)` |
  | setting off: damping | 0.27 | 0.3888 |
  | setting off: rate cap | `clamp(0.064 m, 0.032, 0.0931)` rad | `clamp(0.0768 m, 0.0384, 0.1117)` rad |
  | moving: gain, damping, rate cap | 0.037, 0.13, 0.2036 rad (12.2 rad/s) | 0.0533, 0.1872, 0.2443 rad |
  | skidding: gain, damping, rate cap | 0.035, 0.27, 0.1949 rad [code] | 0.0504, 0.3888, 0.2339 rad |

  Skidding keeps the setting-off damping; that is read in UYA's code, and PAL's is
  assumed to follow its own setting-off value.

  This reproduces all 724 non-strafe walking frames of the R&C3 traces exactly
  **[verified on R&C3 PAL]**. While strafing the hero faces the camera direction instead
  **[code]**.

## Jumping

### Vertical model [verified on both]

Every jump-type state uses one model. Entering the state fills a jump block with that
move's parameters (table below). Then, each frame:
1. **Push-off:** while `t < push-off frames`, the hero is pressed to the ground with
   48 u/s², −0.01333 per frame (−0.0192), and the jump has no lift yet.
2. **Gravity:** afterwards, gravity is subtracted each frame, and the fall speed is
   capped at 50 u/s.
3. **Hold to go higher:** while X is held, for at most the move's hold frames, the target
   height h grows by `(max height − min height) / hold frames` each frame.
   - h starts at the minimum height.
   - The first frame counts as held, so a tap still gets one step.
4. **Lift:**
   - After push-off, the game adds the missing upward speed, `√(2·g·h) − (lift already
     given)`, where g is the per-frame gravity.
   - So the jump always reaches the height a projectile launched at `√(2·g·h)` would
     reach.
   - Holding X keeps topping it up, which shows in the traces as a weaker apparent
     gravity.
5. **After the apex:** once the vertical speed drops below 0.001 per frame, a standing
   jump switches to 1.17 × its gravity.

In the traces, R&C3, Gladiator and UYA all reproduce this exactly.
- **A standing jump on UYA NTSC, X held throughout:**
  - h climbs from 1.42 by 0.075294 per frame for 17 frames to 2.7;
  - the take-off speed is `−0.01333 − 0.00825 + 0.16098 = 0.13940` u/frame;
  - gravity switches to 0.0096525 at the apex.
- **The same jump on R&C3 PAL:** h climbs by 0.0914 per frame, the take-off is
  `−0.0192 − 0.01188 + 0.19515 = 0.16407`, and gravity switches to 0.0139 at the apex.
- The "take-off speed 0.1640625" measured earlier is this derived value: it depends on
  how long X is held.

### Jump heights [verified on UYA NTSC, 2026-10-12]

The hero kit's controller (`crates/rac_hero`) implements the model above. Its apex
heights match our own UYA NTSC trace `uyantsc-veldin-moves-1` to 0.001 u. Hold length is
the number of steps the jump block's target height took in the trace (the press counts
as one):

| Jump (trace frame) | Hold | Target height reached | Apex above take-off |
|---|---|---|---|
| Standing, held fully (165) | 17 | 2.7 | 1.8867 |
| Standing, tapped (82) | 3 | 1.6459 | 1.3115 |
| Heli jump (619) | 3 | 1.68 | 3.4327, with the thrust |
| Long jump (1158) | 5 | 1.9179 | 1.42717, from the game's intended vertical velocity |

- **The target height is not the apex.** The lift arrives over the hold time while gravity
  acts, so a fully held standing jump peaks at 1.89 u, not 2.7.
- **The long jump's trace is bumpy.** The trace's actual height reached 1.70 u because
  the capsule rode up uneven ground mid-flight: on frames 7–12 the actual vertical speed
  exceeds the intended one. The intended velocity follows the model exactly.
- **On push-off frames the intended vertical velocity is −0.01333.** The actual
  displacement is about zero, because the hero stays on the ground.

### Jump parameters by state

These are UYA NTSC values. In the frame columns PAL is in brackets.
- **Verified rows:** the standing, run, long, flip, double and heli jumps are verified on
  UYA NTSC, and the standing, run and double jumps on R&C3 and Gladiator PAL.
- **Other rows:** [code].

| State (libuya name) | Height min–max (u) | Hold frames | Push-off frames | Gravity (u/s²) | Air speed cap (u/s) | Notes |
|---|---|---|---|---|---|---|
| 7 jump | 1.42–2.7 | 17 (14) | 1 (1) | 29.7, then 34.75 after the apex | 6.17 | |
| 9 run jump | 1.42–2.7 | 17 (14) | 1 (1) | 29.7 | 6.17 | Extra forward push during push-off |
| 10 long jump | 1.9–1.95 | 14 (12) | 5 (4) | 11 | none: keeps its take-off speed | See "Long jump" |
| 11 flip jump | 3.2–3.25 | 14 (12) | 1 (1) | 29.7 | own rule | See "Flips" |
| 13 rocket jump | 4.0–4.07 | 14 (12) | 9 (7) | 29.7 | 1.9 | |
| 14 double jump | (own model, below) | 14 (12) | none | 29.7 | 4.75 | |
| 15 heli jump (UYA's jump from a crouch) | 1.65–1.7 | 5 (4) | 9 (7) | 29.7 | 3.3 | Thrust, below |
| 16 charge jump | 0.4–0.9 | 1 | 5 (4) | 8.5 | | Air acceleration 44 u/s², braking 50 u/s²; also sets a speed of 11.5 u/s whose use is not traced (probably the boots' forward speed) |
| 17 wall jump | 1.8–1.85, or 3.3–3.35 off a wall-jump surface | 12 (10) | 9 (7) | 29 | 4.9, away from the wall | See "Wall jump". The higher pair is for walls of collision material 15 |
| 18 water jump | 1.95–1.98 | 10 (8) | 5 (4) | 29.7 | | |
| 27 ledge jump | 2.0–2.05 | 1 | 4 (3) | 25 | 2, forward | Climbing up from a hang. See "Ledges" |
| 40 grind jump | 2.4–2.45 | 17 (14) | 5 (4) | 27 | | |
| 41 grind switch jump | 2.5–2.55 | 10 (8) | 2 (2) | 24 | | |
| 56 lava jump | 5.5–5.55 | 2 | 5 (4) | 15 | | |
| 105 quicksand jump | as state 7 | | | | | |
| 134 | 37–37.5 | 17 (14) | 1 (1) | 13 | 7 | Entered when jumping from a moby of class 0x9B9 or 0xF05, so probably a jump pad |

- **Defaults for every jump-type state:**
  - fall speed cap 50 u/s;
  - during push-off, the ground speed moves towards the air cap at 20 u/s² and brakes at
    11 u/s²;
  - in the air, air control (below) turns the hero at up to 15 rad/s. This is the jump
    block's field at +0x464: 0.25016 per frame (0.3002).
- **States 69, 72, 84, 107 and 115** also use the model; their names are not in libuya.
  For example, 107 jumps to a ledge's height plus 1.7–1.75.
- **A level-specific override [code]:** when the global at 0x1D545C is 0 (probably the
  level number; it is 1 on Veldin), states 7 and 9 instead use heights 7–10 and gravity
  19 u/s² from a data table. Not seen in play.

### Air control [verified on R&C3 PAL]

Most jump-type states steer the same way after push-off:
- **Target:** the horizontal intended velocity heads for the stick direction (camera
  relative) times `air speed cap × stick length`.
- **Step:**
  - It changes by at most 20 u/s², 0.005556 per frame (0.008), in one step.
  - That step is multiplied by `1.5 × (1 − cos θ)`, clamped to 1–3, where θ is the angle
    between the current and wanted velocity. So steering backwards brakes up to three
    times as hard.
- **Stick released:** with the stick under 20% of full tilt, the velocity bleeds off at
  8 u/s² instead, 0.0032 per frame on PAL.
- **Blocked jumps:** if a probe during push-off found something in the way, the target
  is a fixed 5 u/s (0.1 u/frame on PAL) instead.
- **Facing:** the hero turns towards the move direction with a spring of gain 0.0576,
  damping 0.288 and the 15 rad/s cap. These are PAL per-frame values.
- **Check:** this reproduces 246 of 256 airborne frames in the R&C3 traces exactly. The
  other 10 are one run jump that hit a wall.

### Long jump (state 10) [verified on UYA NTSC]

- **Input:** crouch (R1) while running, then jump. Crouching first brakes at 27 u/s², so
  the timing of the two buttons matters.
- **Which jump a crouch gives [code, UYA NTSC `hero_crouch_jump`]:**
  - X counts if it was pressed in the last 9 frames, but only once the crouch is 7
    frames old. A press early in the crouch is buffered, not lost.
  - With the stick past 0.7 and the hero still moving faster than 0.9 u/s
    (0.015 u/frame), it is a long jump. Otherwise it is the heli jump (state 15).
  - These states need Clank's heli-pack in the back slot (item 2 in slot 3). With item
    3 (R&C1's thruster-pack) the same inputs give states 16 and 13 instead. So state 16,
    libuya's "charge jump", is the thruster-pack's dash, not the charge boots.
  - A second way in: crouch pressed in the 2 frames before a standing or run jump's 3rd
    frame, while moving faster than 0.9 u/s, turns that jump into a long jump.
- **Run jump or standing jump [code]:** X from walking gives a run jump (state 9) when
  the stick is past 0.85, and a standing jump (7) otherwise. The speed doesn't matter.
- **Push-off (5 frames, PAL 4):**
  - the speed steps towards the 6.17 u/s air cap at 20 u/s²;
  - on top, `t × 0.014722` u/frame (PAL `t × 0.0212`) is added along the facing
    direction each frame, so the forward speed climbs faster on each push-off frame.
- **In the air there is no air control.** The horizontal velocity reached on the last
  push-off frame is kept for the whole flight.
  - In our trace that was 0.13044 u/frame (7.8 u/s).
  - From a full run it would be 0.1617 u/frame (9.7 u/s).
- **Vertically** it is the standard model with 11 u/s² gravity. With a hold of 3 frames
  the take-off was 0.0919 u/frame, and the flight lasted about 70 frames.

### Flips (state 11) [verified on UYA NTSC and Gladiator PAL]

- **Input:** while strafing (L2 or R2), push left, right or back and press X, for a side
  or back flip. Strafing forwards with X is an ordinary jump.
- **Horizontal:**
  - The flip moves in the stick direction at `max(2, 5.5 × stick)` u/s: 0.091667 u/frame
    at full tilt, 0.11 on PAL.
  - It starts at 5 u/s and closes the gap at 47 u/s², so at full tilt it is at speed on
    the first frame.
- **Vertical:** the standard model with 3.2–3.25 u.
- **Without strafing [code]:** a flip eases from its take-off speed towards 4.2 u/s at
  8 u/s². When it is entered this way is not yet clear.
- Gladiator's traced flip was a strafe side flip at the same 5.5 u/s, so Gladiator's
  default controls strafe there.

### Double jump (state 14) [verified on UYA NTSC, R&C3 and Gladiator PAL]

- The double jump doesn't use the target height. For its first 8 frames (7):
  1. gravity is applied;
  2. the vertical speed then steps up by 150 u/s² towards a cap.
- **The cap:**
  - It is 7 u/s × 1.25 = 8.75 u/s in R&C3/UYA.
  - UYA NTSC climbs by +0.033417 per frame (0.041667 − 0.00825) to 0.145833. PAL
    climbs by +0.0481 per frame to 0.175.
  - It is 7 u/s with no 1.25 factor in Gladiator/DL, hence the measured 0.140.
  - After the boost window the speed is held for one frame, then gravity takes over
    **[verified on UYA NTSC]**. In the moves-1 trace (frame 406) the cap is reached on
    frame 4 and kept by the boost until frame 7. Frame 8 holds it. From frame 9 it drops
    by 0.00825 per frame: 0.10458 on frame 13. That is the "two or three frames" seen
    earlier.
- **When it can start [code, UYA NTSC `hero_transitions`; consistent with the traces]:**
  - From a standing or run jump (7, 9), not before the jump's 15th frame (t > 14).
  - X counts if it was pressed within the last `t − 4` frames (at least 2, at most 30).
    So the take-off press doesn't count, and an early second press is buffered until
    frame 15.
  - Past the apex, the hero must also be more than 0.7 u above the ground and moving
    vertically slower than 5.8 u/s.
  - Both double jumps in the moves-1 trace came on frames 18 and 19, on the frame X was
    pressed, which fits.
- **Less boost when already high [code]:**
  - The cap is multiplied by 0.7 when Ratchet is more than 2.2 u above where the first
    jump took off, and by 0.47 above 2.8 u.
  - If something is in the way between 1 and 2.7 u above him, the thresholds are 1.7 and
    2.1 u and the factors 0.7 and 0.45.

### Heli jump thrust (state 15) [verified on UYA NTSC and R&C3 PAL]

- On frames 35–70 (29–58), the jump adds an upward 49 u/s² (48) from a data table.
  - The start frame is one earlier under some flag.
  - The table can hold more segments.
- **UYA:** the vertical speed rises by 0.005361 per frame, which is 49/3600 − 29.7/3600.
- **R&C3 PAL:** it rises by 0.0073 per frame for 30 frames, which is 0.0192 of thrust
  minus 0.01188 of gravity.

### Glide (state 8) [verified on UYA NTSC for the sink rate]

- **Entry [code, UYA NTSC `hero_transitions`; matches the moves-1 glide]:**
  - Holding X in a double jump after its 30th frame (t > 30), when more than 1.5 u above
    the ground. Entering the double jump sets that 30-frame minimum in its jump block.
  - Holding X in a heli jump also glides, after its 85th frame and above 0.6 u.
  - A standing or run jump can't glide: without a fresh X press their transition code
    skips the glide check. The moves-1 trace's fully held jump (41 frames of X) indeed
    never glided.
  - In the moves-1 trace the glide began on the double jump's 31st frame, at −0.0439 u
    per frame. An earlier reading took that for a fall-speed threshold (2.4 u/s); the
    code shows it is the frame count.
  - After entry, the speed eases to the −0.036 sink within about 14 frames.
- **Sink rate:** the vertical speed eases on a spring towards a sink of `3 u/s × 0.72` =
  2.16 u/s, 0.036 per frame, and holds it. The 0.72 can be overridden by a global.
- **Spring [code]:** gain 0.036, damping 0.432, cap 0.34 (PAL per frame).
- **Forward [code]:** it moves at 3 u/s in the stick direction, 5 u/s in one gadget
  mode, accelerating at 15 u/s² and braking at 7 u/s².

### Falling and other airborne moves

- **Falling (state 6):**
  - Gravity is 22 u/s² in both games **[measured on Gladiator PAL, code on R&C3 PAL]**:
    R&C3 PAL's literal is 0.0088.
  - The fall is capped at 50 u/s.
  - With the stick released, the horizontal speed brakes at 7 u/s² **[code]**.
- **Jump attack (state 20) [verified on UYA NTSC, R&C3 PAL and Gladiator PAL]:**
  1. The vertical speed steps up by 150 u/s² towards 7.7 u/s. The target is 5.4 u/s
     above 1.8 u of ground height, and 3.85 u/s above 2.25 u.
  2. It holds until about frame 15 (13).
  3. Then it falls at 130 u/s² (UYA 0.036111 per frame, PAL 0.052), capped at 37 u/s.
  - Horizontally the speed brakes at 15 u/s². It holds 4.5 u/s instead while some hero
    flags are set or Ratchet is more than 4 u above the ground. The R&C3 trace shows the
    4 u case **[code]**.
  - Close to the ground on PAL the fall slows again in a way not yet explained.

## Ground attacks, hits and ledges [code]

- **Wrench combo (state 19):**
  - Each swing lunges forward at a speed picked by the combo step and the animation
    frame: about 4.4, 5.7 or 3.7 u/s.
  - It is scaled when an enemy is targeted, and the hero turns towards that enemy.
  - The lunge accelerates at 37 u/s² and brakes at 28 u/s².
- **Get hit (state 22):** see "Getting hit and dying".
- **Ledges (23–27):** see "Ledges".
- The Gladiator traces contain ledge grabs and ledge jumps that we have not yet compared
  with this.

## Wall jump (state 17) [code, UYA NTSC, 2026-10-12]

Read in `hero_detect_wall` (0x4E39B8), the transitions (`hero_transitions`, 0x503B68),
`hero_change_state` and the air phase (`hero_air_horizontal`, 0x4EDE80). Not yet seen in a
trace: Veldin's start has no wall to jump from.

- **Wall contact is a 6-frame window.** Every frame the detector may set a countdown to 6
  (7 during a wall jump), which the per-frame timers then count down. A wall jump can
  start while it is non-zero.
- **When the detector looks:**
  - the hero is at least 0.4 u above the ground and not falling faster than 20 u/s;
  - he is in a jump-type state, or was one or two states ago;
  - no ledge is held, and a ledge-grab check at the point where his current trajectory
    will be 20 frames later finds nothing. If it would find a ledge, the wall is ignored,
    so ledges win over walls.
- **What counts as a wall.** All probes start on the hero's axis and point along his
  facing:
  1. A horizontal segment at 3.0 u above his feet (1.7 u during a wall jump), from the
     axis to 1.85 u out (the capsule radius plus 1.4), must hit something. That makes
     sure the wall is tall.
  2. The surface must not be of collision materials 10 or 12. It must be steeper than
     75° from the horizontal, and its normal must point back at the hero within 50°, so
     he is facing into it.
  3. The same segment at 0.2 u above his feet must hit within 0.72 u of the axis (0.95
     during a wall jump), so the wall is at most 0.27 u from the capsule's surface.
  4. A 10 u probe from his chest along the wall normal measures the open space behind
     him. It defaults to 5 u when nothing is hit.
- **Repeating it:**
  - A chained wall jump must leave from a wall facing at least 170° away from the last
    one's, so the hero alternates between two facing walls.
  - On ordinary walls, at most two wall jumps in a row are allowed.
    - Entering a jump-type state from a wall jump off an ordinary wall bumps a counter.
    - While the counter is non-zero, ordinary walls are ignored.
    - So the first wall jump (from a jump) and a second (from the first) work, and a
      third is refused.
  - Walls of **material 15** (wall-jump surfaces) reset that counter, so chains on them
    are unlimited, and they give the higher jump.
- **Starting it:** X pressed in the last 7 frames, after the current state began, while
  the window is open, in these cases:
  - in a standing or run jump (7, 9);
  - in a wall jump after its 25th frame;
  - in the first 8 frames of a glide, if the glide came from one of those states or a
    double jump.
- **The jump itself:**
  - Its 9 push-off frames are spent clinging to the wall, pressed down at the usual
    push-off rate. The standard vertical model then launches it (table above):
    1.8–1.85 u, or 3.3–3.35 u off material 15, with 29 u/s² gravity and a 12-frame hold.
  - From frame 9 the horizontal velocity is set every frame to 4.9 u/s (0.081667 u/frame)
    along the wall's normal. There is no air control.
  - From frame 10 the hero turns towards the normal's direction with gain 0.042, damping
    0.2 and cap 0.198 rad/frame.
  - Entering the state also writes 1.7 u/s, or 4.9 u/s scaled by the open space behind,
    to a jump-block field that no code in the Veldin overlay reads.

## Ledges (states 23–27) [code, UYA NTSC, 2026-10-12]

Read in `hero_detect_ledge` (0x4E4550), the state update and the transitions. The game
runs the detector every frame, before the transitions, at the hero's position, and keeps
its result in a ledge block at 0x1A50E0: the hang point, the lip point, the wall's yaw and
a found flag.

- **When it looks:**
  - in a jump-type state once past the apex, or rising slower than 3.5 u/s, and not
    before the frame after the push-off ends. The jump states' transition code returns
    early until `t > push-off frames + 1`, which also holds back their wall jumps and
    double jumps. So a climb doesn't grab the ledge it leaves. Only the crouch that turns
    a jump into a long jump is checked earlier;
  - while falling, gliding or shimmying;
  - not in a double jump before its animation's frame 37;
  - not while a 10-frame (40 after some drops) no-grab timer runs. Letting go starts it.
- **What counts as a ledge** (sizes for Ratchet; a second set is for Clank):
  1. **The top:** a vertical segment from 1.65 u above to 1.15 u below the point at the
     front of the capsule (the hero's position plus 0.45 along his facing) is cast down.
     It is tried four times, stepping 0.075 u further out each time. The hit must be
     flatter than 20°, and of a material other than 0, 9 or 12.
  2. If the top belongs to a moving object, that object must allow grabbing.
  3. The top must be between 0.5 and 1.5 u above the hero's feet. It must also be at
     least 1.8 u above the ground under him, so low ledges are landed on, not grabbed.
  4. **Clear lip:** a segment 0.005 u above the top, from 1 u back towards the hero to
     the hit point, must not be blocked, except by the same object.
  5. **The wall face:** a horizontal segment from the hero's axis to 1 u past the hit,
     0.7 u below the top, must hit. It is tried 5 times in all, 0.14 u lower each
     time. The wall's normal, flattened, gives the hanging direction.
  6. **The lip:** starting from the top's hit point, short vertical probes 0.03 u out
     from the wall climb it in 0.03 u steps, up to 2.4 u, until one finds nothing. Where
     it is clear is the lip.
- **The hang point** is the lip plus 0.5 u along the wall normal and 1.43 u down: the
  feet hang 1.43 u below the edge, 0.5 u out from the wall.
  - The detector also notes whether a 0.45 u sphere fits 0.6 u above the lip.
- **Grab (23):**
  - The hero moves towards the hang point at up to 4 u/s (0.0667 u/frame).
  - He turns to face the wall with gain 0.04, damping 0.2 and cap 0.105 rad/frame.
  - At animation frame 12 he hangs (24).
- **Hanging (24):**
  - Losing the ledge drops him into a fall, with the 10-frame no-grab timer.
  - X (pressed in the last 17 frames) climbs: a ledge jump (27).
  - R1, or X while pulling the stick more than 135° away from his facing, lets go.
  - The stick past 0.5 and within 45° of either side shimmies that way (25 or 26), if a
    ledge is found 0.3 u to that side.
- **Shimmy (25, 26):**
  - Each frame the detector runs at a point a short way to the side, and the hero moves
    towards the hang point it finds.
  - The step is that offset times a per-animation-frame factor times 0.5. The factors
    for its 15 frames are 0, 0, 0.025, 0.05, 0.075, 0.162, 0.162, 0.107, 0.037, 0.037,
    0.037, −0.017, −0.017, −0.035 and −0.035.
  - The resulting speed hasn't been traced yet.
  - It returns to hanging when the stick is released, or no ledge continues.
  - X climbs; X while pulling away lets go (40-frame no-grab timer).
- **Ledge jump (27):**
  - Vertically it is the standard model: 2.0–2.05 u, push-off 4 frames, hold 1 frame,
    gravity 25 u/s².
  - Its forward speed is tied to the animation. It is 0 until frame 13, eases to 2 u/s at
    8 u/s² until frame 27, then back towards 0, and is 0 after frame 30.
  - After 40 frames he turns and steers like a normal jump.

## Getting hit and dying [code, UYA NTSC, 2026-10-12]

- **Get hit (state 22):**
  - Entering takes the velocity the attacker set, capped at 7 u/s (0.11667 u/frame).
  - It also starts a 77-frame timer that stops a floor's damage from hitting again.
  - The speed then shrinks at 4 u/s² for 11 frames on the ground, then 10 u/s². In the
    air it shrinks at 4 u/s² throughout. PAL has 0.0016 and 0.004 per frame for 9 frames.
  - After 28 frames on the ground he returns to idle, or dies (57) if health (0x1A7430)
    is below 1. If he is more than 1 u above the ground after the animation's frame
    12.5, he falls instead.
- **Health:** idle or walking with health below 1 also starts the death state.
- **Floor materials** set by the ground probe trigger states:
  - Material 17 damages: its amount is a global divided by 8, with a 90-frame repeat
    timer, and it starts state 177, which shares the get-hit update.
  - Material 1 (lava) gives the lava jump (56), or death (57) at no health.
  - Material 3 (water) gives state 106, or the 125 sink when health is 0. In UYA's
    single-player code, 125 (libuya's "charge") sinks the hero at 1 u/s with the
    quicksand type.
  - Material 4 gives the slide (48).
- **Death (57):** the hero brakes like a landing while the death animation plays. The
  respawn timing is in the death code, which has not been read yet.

## Slope limit [code, UYA NTSC, 2026-10-12]

- The ground probe (`hero_ground_probe`, 0x4EBA68) casts down from above the hero. Its
  start is 2.2 × the capsule radius up, or 2.9 × while falling.
- It counts the hero as standing only when he is within 0.02 u of the hit, and the
  surface is at most **50°** (0.8727 rad) from the horizontal.
- Steeper ground leaves him airborne, so he falls and slides down it.
- The probe also records the height above the ground (0x1A4F1C, 32 when nothing is
  found) and the slope angle (0x1A4F20), which many rules above read.

## Charge boots (state 125) [code, then verified on DL NTSC, 2026-10-12]

UYA's hero code on Veldin has no charge-boots state, so they were read in Deadlocked's
(DreadZone Station; addresses in `re/SCUS-97465/symbols.toml`). DL multiplies its literals
by `frame_dt` at run time, so the values below are per second as written; frame counts
are at 60 Hz.

**Checked against our own DL NTSC traces** (`dlntsc-dreadzone-charge-*`, scripted
keyboard play; `docs/re-setup.md`). The boots aren't owned at DreadZone Station's start,
so gadget 0x11 was granted by writing its gadget-box entry. These match the code:
- the 15-frame window: presses 15 frames apart charge, 16 apart don't;
- 100 u/s² up to the 30 u/s target, reached on the 18th frame;
- 61 frames (0–60), then the skid with `noInput` 30 and the cooldown counting down from
  30;
- the last frame heads for 11.5 u/s: the skid's first speed is `0.5 − 15/3600 − 27/3600`
  u per frame exactly;
- the 0.17 u rise on the spring;
- the steering's 0.17 easing and its `t/50` ramp to 0.020362 rad per frame;
- the bounce off a wall at 9 u/s, slowing at 24 u/s².

**Found in the traces:**
- **The hero never moves faster than 0.43 u per frame (25.8 u/s).**
  - The intended velocity (hero+0x130) is capped there, while the speed variable reaches
    0.5 (30 u/s).
  - 0.43 is the collision radius less 0.02, the same expression as the wall probe's, so
    it is probably the collision's step limit. Its code isn't found yet.
  - Slower moves never reach it.
- **The bounce lasts 54 frames**, its animation, then the hero idles. Its speed is spent
  after 23 of them.
- **The skid after a charge** slides on its own vector:
  - the motion when the skid began (so 0.43 u per frame), shortened by 27 u/s² each
    frame (`hero_skid_slide`);
  - when the 30-frame input lock ends, the motion drops to about 0.02 u per frame and
    creeps to a stop;
  - the state lasts 61 frames, its animation, while the speed variable keeps braking
    unseen.
- **Controls:** DL's default control scheme (the scheme byte at 0x171DE0 is 1) maps the
  charge's action to **L2**, and R1 fires. The other scheme uses R1.
  - DL's default controls keep the hero facing the camera, so the left stick strafes.
  - The opening tutorial ignores the movement buttons until each step is done: look left
    and right with the right stick, fire, then jump.

- **Starting a charge** (`hero_common_transitions`):
  - **Input:** R1 pressed again within 15 frames of the last R1 press. Every R1 press
    reopens the 15-frame window. In DL's other control scheme the button is L2. R1 is also
    crouch, so the first press crouches.
  - **Needs:**
    - gadget 0x11 in the gadget box: the charge boots;
    - input not locked, and the 30-frame cooldown over;
    - not in first person or targeting;
    - a ground state: idle, walk or skid, crouch, or a jump or fall while still on the
      ground (its push-off).
  - **On entry:** the speed carries over from the state before.
- **Speed:**
  - It heads for **30 u/s** for the first 60 frames, rising at **100 u/s²** and falling at
    **15 u/s²**.
  - From frame 60 the target is 11.5 u/s. In single player that is only the last frame,
    because the charge ends at frame 61.
  - A global that is 0 in single player (0x21E878) makes 11.5 u/s the target throughout,
    and swaps the ground spring below for a downward push. It is probably multiplayer.
  - The hero moves along his facing, following the ground's slope.
- **Steering:**
  - Each frame, a stored value eases towards minus the stick's x by 0.17 and is clamped to
    ±1. DL never resets it between charges.
  - When the stick magnitude is over 0.2, the yaw turns by
    `1.2217 rad/s (70°/s) × frame_dt × magnitude × eased value`.
  - That turn is scaled by `t / 50` for the first 50 frames, so the start is nearly
    straight.
  - So the stick steers like a car (right turns right), rather than pointing the way to
    go.
- **Height:** in single player the hero's height follows the ground height plus 0.17 u
  on a spring: gain 0.04, damping 0.3, at most 7.5 u/s.
  - So he rides slightly above the ground.
  - Running off an edge, he keeps charging and sinks towards the ground below at up to
    7.5 u/s.
- **Each charge frame** sets the 30-frame cooldown, which the timers update counts down.
- **Ending (state 125's transitions):**
  - **At frame 61, on magnetic ground, or on a flag not yet identified:** a skid
    (state 3) with input locked for 30 frames (`noInput`). It brakes from about 30 u/s at
    27 u/s², which takes about 65 frames.
  - **Into a wall:** when the hero is touching a wall (`HeroColl.contact`) and either
    - he moved less than 75% of the intended velocity, or
    - a ray from his front at 1.0 u up finds something within 1.25 u,

    he bounces: state 121.
  - There is no jump out of a charge.
- **The bounce (state 121, libdl's "jump bounce"):**
  - On entry he keeps his facing and moves backwards at **9 u/s**, slowing at
    **24 u/s²**.
  - Off the ground he falls at 25 u/s².
  - It ends with its animation (length not read), into another state not read.
- **Also in the update, but not movement:**
  - A sphere 0.57 u wide, 3 u ahead, damages mobys it touches. That is how a charge
    breaks crates and hits enemies.
  - It sets the lean of the hero's joints and counts charge frames for the player's
    stats.
- **In the hero kit (P1.5):**
  - All of this is implemented, with the hero's feet kept on the ground: the 0.17 u lift
    is left to the animation [simplified].
  - Also from the traces: the 0.43 u step limit, the bounce's 54 ticks, and the skid's
    slide that stops when the lock ends. The skid then idles at once, rather than
    standing out its animation [simplified].

## Moves that need items [code, UYA NTSC]

The kit gates these with per-hero ability flags (PLAN.md section 5, 2026-10-12).
- **The crouch jumps** (`hero_crouch_jump`) check the back slot, which is item slot 3:
  - with Clank's heli-pack (item 2, and a global flag set): the long jump (10) or heli
    jump (15);
  - with R&C1's thruster-pack (item 3): the thruster dash (16) or rocket jump (13);
  - **with neither: a standing jump (7).**
  - The long jump from a jump's first frames (crouch pressed within 2 frames) needs the
    same items.
- **The glide** needs the same global flag, and no item 4 in the back slot.
- **The charge boots** are DL's gadget 0x11, above. The jumps, double jump, wall jump,
  ledges and flips need no item.

## Differences between the games

| | R&C3 / UYA | Gladiator / Deadlocked |
|---|---|---|
| How per-second values become per-frame | Folded into literals when compiled | Multiplied by `frame_dt` globals at run time |
| Hero in code | Static global, addressed directly (code base 0x1A4BE0) | Pointer argument (libdl's `Player`) |
| Walk speed and run-jump air cap | 6.17 u/s | 7.25 u/s |
| Double-jump cap | 8.75 u/s | 7 u/s |
| Ledge jump push-off on PAL | 3 frames | 2 frames |
| Standing jump animation frames (peak, land, game land) | 18, 30, 12 | 18, 27, 19 |
| Charge boots | no charge state in Veldin's code | state 125, read in DL's code |
| Everything else read so far | identical | identical |

- **`HeroPlayerConstants`** is a block of per-second constants, pointed to by
  hero+0x2EEC, that Gladiator and DL carry. Its values are identical in both:
  - walk 7.25 u/s;
  - jump heights 2.3 and 1.28, max up time 15;
  - jump gravity 29.7, fall gravity 22, max fall speed −50;
  - jump air speed 5.95;
  - collision capsule radius 0.45, top 1.05, bottom 0.7 (0.5 when falling).
- **What single-player uses from it:** only the capsule. It is read in four places, all
  collision code, and the movement code uses its own literals. Its other values are
  probably for multiplayer, where libdl reports them **[code]**.

## State numbers

The state numbers match libuya's `PlayerState`, and the types match `PlayerType`
**[measured]**. Pairs seen in the traces:

| State | Name (libuya) | Type | Seen in |
|---|---|---|---|
| 0 | idle | 0 idle | all |
| 1 | look (first person, L1) | 0 idle | all |
| 2 | walk | 1 walk | all |
| 3 | skid | | UYA |
| 4 | crouch | 12 crouch | R&C3, UYA |
| 6 | fall | 2 fall | Gladiator, UYA |
| 7 | jump | 4 jump | all |
| 8 | glide | | UYA |
| 9 | run jump | 4 jump | all |
| 10 | long jump | 4 jump | UYA |
| 11 | flip jump | 4 jump | Gladiator, UYA |
| 14 | double jump | 4 jump | all |
| 15 | heli jump (the jump from a crouch) | 4 jump | R&C3, UYA |
| 19 | combo attack (wrench) | 6 attack | all |
| 20 | jump attack | 6 attack | all |
| 22 | get hit | 7 get hit | R&C3 |
| 23, 24 | ledge grab, ledge idle | 3 ledge | Gladiator, plus 24 in an R&C3 savestate |
| 27 | ledge jump | 4 jump | Gladiator |
| 29 | targeting (weapon out) | 0 idle | R&C3, Gladiator |
| 101 | skid to position (scripted move) | 24 NPC | R&C3 |

Also read in DL's code but not seen in a trace: 121, the bounce off a wall after a charge
(type 10), and 125, the charge (type 28) [code].

## Timing [measured]

- **One update per frame, at a fixed step.** Frames that took longer on the host never
  show larger steps. During a scripted event in R&C3 (state 101), the game stopped
  counting frames for about 18 vsyncs, and the hero did not move during them.
- **Input latency:**
  - A button pressed in input-recording frame *n* first changes the hero state two frames
    later. 54 of 66 jump presses in the four recordings show exactly that.
  - The stick floats the hero reads also lag the recording by two frames.

## Next steps

1. **More traces on Deadlocked NTSC.** They work since 2026-10-12
   (`re/SCUS-97465/hero.toml`, `docs/re-setup.md`), and the charge is verified. Next: DL's
   jumps, to compare with Gladiator PAL and UYA, and the code behind the 0.43 u step limit
   and the skid's stop when the input lock ends.
2. **Moves that need a particular place:** wall jump, ledge grab and jump, grinding and
   swimming.
   - The wall jump, ledges, slope limit and getting hit are read in UYA's code, and the
     charge boots in DL's (sections above, 2026-10-12). Grinding and swimming aren't yet.
   - Veldin's starting area has none of them, so they are untraced. A savestate near a
     wall and a ledge (from the owner or a scripted play-through) would let us check the
     probe distances and the shimmy speed.
   - The Gladiator traces already hold ledge grabs to compare.
3. **Half-tilt stick on NTSC:** keyboard input only gives full tilt, so the pad layer's
   dead zone is verified on PAL only.
4. **Camera (RE.4):** started in [camera.md](camera.md).
5. **Constants for the kit:** write the tables above into the hero kit's RON profiles
   (P1.5) and use them as the RE.6 tolerance tests' expected values.

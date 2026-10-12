# Hero controller: how Ratchet moves

How Ratchet moves in R&C3/UYA and Gladiator/Deadlocked (PLAN.md RE.3, fed by RE.5 and
RE.8). It is the reference for the hero kit's controller (P1.5) and its feel-matching tests
(RE.6). Last updated 2026-10-12.

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
- **Skid [code]:**
  - When the stick swings far round from the direction of travel, or is released, a skid
    flag cuts the speed and slows it by 17 u/s² (0.0068).
  - The details depend on the angle and the speed; not characterised yet.
  - After a long jump the hero lands in state 3, labelled "skid" by libuya.
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
  are fixed:

  | | UYA NTSC, per frame | R&C3 PAL, per frame |
  |---|---|---|
  | setting off: gain | `clamp(0.02 m, 0.01, 0.022)` | `clamp(0.0288 m, 0.0144, 0.03168)` |
  | setting off: damping | 0.27 | 0.3888 |
  | setting off: rate cap | `clamp(0.064 m, 0.032, 0.0931)` rad | `clamp(0.0768 m, 0.0384, 0.1117)` rad |
  | moving: gain, damping, rate cap | 0.037, 0.13, 0.2036 rad (12.2 rad/s) | 0.0533, 0.1872, 0.2443 rad |
  | skidding: gain, rate cap | 0.035, 0.1949 rad | 0.0504, 0.2339 rad |

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
| 17 wall jump | 1.8–1.85, or 3.3–3.35 in a second mode | 12 (10) | 9 (7) | 29 | | Pushes off the wall at 1.7 u/s; in the second mode 4.9 u/s, scaled up to ×1.7 by another value. The flag that picks the mode is not identified |
| 18 water jump | 1.95–1.98 | 10 (8) | 5 (4) | 29.7 | | |
| 27 ledge jump | 2.0–2.05 | 1 | 4 (3) | 25 | | |
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

- **Entry:** holding X while falling after a double jump.
  - In the moves-1 trace the glide began on the double jump's 31st frame. The vertical
    speed was −0.0439 u per frame, having passed zero about 5 frames earlier.
  - So the entry looks like a fall-speed threshold of about −0.04 u per frame (2.4 u/s)
    **[measured, one sample; the rule isn't read in code]**.
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
- **Get hit (state 22):** the hero slides and loses speed at 4 u/s² for 9 frames, then
  10 u/s² (PAL 0.0016 and 0.004 per frame).
- **Ledge grab and idle (23, 24):** the hero is drawn to the ledge point and turned to
  face it.
- **Ledge shimmy (25, 26):** speed comes from a table indexed by animation frame.
- The Gladiator traces contain ledge grabs and ledge jumps that we have not yet compared
  with this.

## Differences between the games

| | R&C3 / UYA | Gladiator / Deadlocked |
|---|---|---|
| How per-second values become per-frame | Folded into literals when compiled | Multiplied by `frame_dt` globals at run time |
| Hero in code | Static global, addressed directly (code base 0x1A4BE0) | Pointer argument (libdl's `Player`) |
| Walk speed and run-jump air cap | 6.17 u/s | 7.25 u/s |
| Double-jump cap | 8.75 u/s | 7 u/s |
| Ledge jump push-off on PAL | 3 frames | 2 frames |
| Standing jump animation frames (peak, land, game land) | 18, 30, 12 | 18, 27, 19 |
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

## Timing [measured]

- **One update per frame, at a fixed step.** Frames that took longer on the host never
  show larger steps. During a scripted event in R&C3 (state 101), the game stopped
  counting frames for about 18 vsyncs, and the hero did not move during them.
- **Input latency:**
  - A button pressed in input-recording frame *n* first changes the hero state two frames
    later. 54 of 66 jump presses in the four recordings show exactly that.
  - The stick floats the hero reads also lag the recording by two frames.

## Next steps

1. **Traces on Deadlocked NTSC.** The UYA approach (`tools/pine-probe/boot.sh` with
   xdotool) works for DL too. It needs a DL NTSC `hero.toml`, which needs a frame counter.
2. **Moves that need a particular place:** wall jump, ledge grab and jump, grinding,
   swimming and charge boots.
   - Their code is read, but Veldin's starting area doesn't have them.
   - A savestate near each (from the owner or from a scripted play-through) would let
     us trace them.
   - The Gladiator traces already hold ledge grabs to compare.
3. **Half-tilt stick on NTSC:** keyboard input only gives full tilt, so the pad layer's
   dead zone is verified on PAL only.
4. **Camera (RE.4):** started in [camera.md](camera.md).
5. **Constants for the kit:** write the tables above into the hero kit's RON profiles
   (P1.5) and use them as the RE.6 tolerance tests' expected values.

# Hero controller: how Ratchet moves

How Ratchet moves in R&C3/UYA and Gladiator/Deadlocked (PLAN.md RE.3, fed by RE.5 and
RE.8). It is the reference for the hero kit's controller (P1.5) and its feel-matching tests
(RE.6). Last updated 2026-10-20 (Deadlocked's ledges, shimmy and climb, read in DL NTSC's
code and traced, and its missing wall jump, checked at a tall wall; the shimmy's 0.2 u side
look and the climb's still push-off in both games; Deadlocked's flips, its missing crouch
and its barred glide; the landing flip both games share). Before that,
2026-10-15: the grind attack, the wrench's clank and the grind's side-on stance, read in UYA
NTSC's code; Deadlocked's melee, read in code and traced on DL NTSC; the crouch flip and the
crouch turn, traced on UYA NTSC; Deadlocked's movement against UYA's; strafing and lock-on;
zip lines.

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
     the recording frame. On UYA NTSC it is the pad update, 0x45D5C8 [code].
   - **Verified on UYA NTSC** (2026-10-17, `uyantsc-veldin-camhalf-0.5-1`, keys at half
     tilt through PCSX2's `AxisScale`): the bytes 191 and 64 gave 0.2105 and 0.1974, on
     both sticks.
2. **Cut-off, in the hero code [verified on R&C3 PAL, code on UYA NTSC]:**
   - If the vector's length is under 0.25, the stick is ignored and the d-pad is used
     instead, each direction giving ±1.
   - That rule is in `hero_update`, and it explains the other 33 frames: 24 small
     vectors zeroed, plus 9 frames in a scripted state.
   - Together with the dead zone, an axis has to be pushed about 67 counts from centre,
     half its travel, before Ratchet moves.
   - **Verified on UYA NTSC** (the same trace): the left stick held at 0.1974 for 2 s left
     the hero's stick at 0 and him standing. The fallback reads the real d-pad (pad
     +0x1B0), not the direction bits the stick sets in +0x1A0.
3. **Magnitude, in the hero code [verified on R&C3 PAL]:**
   - `m = clamp((max(|x|, |y|) + length) / 2, 0, 1)`.
   - A diagonal therefore counts as fuller than its Euclidean length.
   - **On UYA NTSC** (2026-10-17, `uyantsc-veldin-camhalf-0.75-{1,2}`, keys at 0.6184
     straight ahead): the walk's speed target was 0.06359 u a frame, `6.17 / 60 × 0.6184`.
     A diagonal isn't checked on NTSC.
4. **Direction:** the stick vector rotated by the camera yaw.
5. **The stick as presses [code, UYA NTSC 0x45D5C8; seen in
   `uyantsc-veldin-crouchflip-1`]:** the pad update also turns the stick into button
   presses, kept with the others in its 30-frame history (`pad_pressed_within`).
   - Each axis sets a d-pad bit while it is past its dead zone (x below 0 left, 0x8000;
     above 0 right, 0x2000; y up 0x1000, down 0x4000). So an axis leaving its dead zone,
     or crossing to the other side, presses that direction.
   - **A flick** sets 0x10000 on a frame the stick is past 0.9, either up from under 0.25
     within 3 frames (passing nothing past 0.9), or pointing more than 55 degrees
     (0.9599311 rad) from where it pointed in any of the last 4 frames. A centred stick's
     angle is 0, so a push from rest flicks for 4 frames, as long as it points more than
     55 degrees from "right".
   - The crouch flip looks for any of these (the mask 0x1F000; "Flips").
6. **Buttons on UYA [verified on UYA NTSC]:**
   - X jumps.
   - R1 crouches.
   - Square swings the wrench.
   - L1 enters first-person look (state 1).
   - L2 or R2 strafes, and both together lock on (section "Strafing and lock-on").

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
- **Turning in a crouch [code, UYA NTSC state 4 in `hero_update_state`; verified on UYA
  NTSC]:** with the stick past 0.2 he turns to it on a slow spring of his own: gain 0.002,
  damping 0.07, cap 0.1658 rad a frame. That reproduces all 34 frames of the crouch turn
  in `uyantsc-veldin-crouchflip-1` (take 5) to 3×10⁻⁶ rad: about 0.17 rad in the first 12
  frames, so a stick pushed to a side stays there long enough to flip.
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
  - **Ways out of state 3, the same in UYA and DL but for the crouch:**
    - X pressed this frame: a standing jump (7). R1 held: a crouch (4) in UYA; DL works
      out R1's pad bit and ignores it (0x6A1770; "Deadlocked's moves"). Both only while
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
  **[verified on R&C3 PAL]**. While strafing the hero turns to the camera instead, on a
  spring of his own (section "Strafing and lock-on") **[code]**.

## Strafing and lock-on [code, strafing verified on UYA NTSC, 2026-10-15]

Read in Veldin's build (addresses in `re/SCUS-97353/symbols.toml`), from the readers and
writers of the hero's lock-on block at 0x1A6000, libuya's `HeroLockOn`.
- **Strafing was checked** against our trace `uyantsc-veldin-moves-3` (strafing left, back
  and forward, with flips), which holds the lock-on block each frame. It reproduces the
  turn spring on all 224 strafe-walking frames and 106 flip frames (to 6×10⁻⁷ rad), and
  the speeds on all 224 walking frames.
- **Not traced:** changing sides (the trace never goes from left to right), and lock-on,
  which needs a weapon that no savestate of ours holds.

### Strafing

- **The flag:** `hero_update` sets `strafing` (0x1A6014) to 1 while L2 or R2 is held (pad
  bits 1 and 2), and counts it down to 0 when they are let go. The first-person and
  lock-strafe cameras (L1's views) set it to 2, so they strafe too.
- **Where the stick points** (`hero_stick_move_angle`): while strafing, the stick, relative
  to the camera, falls in one of four sectors: left, right, forward or back, each 45° either
  side. An exact diagonal counts as forward. Back with the stick past 0.15 sets
  `strafe_back`.
- **The walk** (state 2) calls `hero_strafe_walk` instead of its turn:
  - **The speed is the walk's [verified]:** towards `6.17 × m` u/s at 37 u/s², braking
    at 30. Strafing has no speeds of its own. The walk's other rules, the skid and the
    part-pushed stick, see an angle error of 0, so they never apply.
  - **The velocity runs along the stick's direction** (`strafe_heading`) **[verified]**,
    kept when the stick is let go, so a strafe slides to a stop the way it went.
  - **He turns to the camera's yaw [verified]** with gain 0.042, damping 0.25 and cap
    0.32 rad a frame, on a rate kept apart from the walk's (`hero_strafe_turn`). The
    walk's own rate is zeroed every strafing frame.
  - **Letting go of the stick [verified]:** he keeps walking, braking at the skid's
    17 u/s² and turning to the camera, until he faces within 2° of the camera's yaw
    (`hero_transitions`, through 0x4EA1F8 while L2 or R2 is held); then he idles with at
    most 1.5 u/s, as from a walk. In the trace that took 33 frames, because the camera
    was still swinging round. The walk without strafing idles after 2 frames.
  - **Changing sides is damped**, by two 3-frame timers that aren't symmetric. Strafing
    right sets one; strafing left while it runs zeroes the speed, so right to left stops
    him for 2 frames. Strafing left sets the other; strafing right while it runs scales
    the speed by (3 − timer) / 3, so left to right takes it to 1/3, then 2/3 of itself.
    The velocity changes direction at once.
- **Jumping:** a jump's push-off moves along `strafe_heading` rather than the facing. In
  the air, jumps and falls turn him to the camera on their usual air spring (gain 0.04,
  damping 0.2, the jump's cap), even with the stick let go; air control still steers by
  the stick. The flip turns with `hero_strafe_turn`.
- **The flip:** the walk's X press (`hero_walk_jump`) flips while strafing when the stick
  is past 0.9 and its sector isn't forward. A diagonal is a jump.
  - **Otherwise a standing jump (7), never a run jump [verified]:** the run jump needs
    the stick past 0.85 and no strafing. The trace's forward strafe jump was a 7.
  - **A second way to flip [verified on UYA NTSC]:** with R1 held, the stick pushed to a
    side or back and X close together. That is the crouch flip in "Flips".
- **Animation:** strafing backwards plays sequence 20 (walking backwards) at 25 × the
  speed in u per frame, 0.7 to 2.2 (`hero_walk_anim`). Sideways, the run plays with a
  procedural twist (`hero_body_twist`): one joint slot turns by the angle from the facing
  to the movement, a second by minus that, on springs of 0.037 and 0.025 with damping
  0.27. Which joints the slots drive isn't read.

### Lock-on

- **Picking a target** (`hero_lock_on`, from `hero_transitions`):
  - It needs L2 and R2 both held, one of them pressed this frame. A failed search waits
    for the next press.
  - The held item must allow it: bit 5 of its byte in `item_flags` (0x142A60), the
    lock-on mod, which each gun's code sets from its level, mostly at V3 (`weapons.md`,
    "Mods"). The bytes are all zero in our savestates, which hold no gun; the wrench
    never sets it.
  - Not with the lock-strafe camera (camera mode 1).
  - The search (`target_search`, scored by `target_score` as the wrench's aim is: the
    least distance × (1 + angle off)) takes targets of class category 5 that are alive,
    within 25 u, 35° round and 70° up or down from the camera's view, and in sight along
    a ray from 0.7 u above the feet. Failing that, it searches round the hero's facing.
- **Keeping it:** the target is let go when it is more than 30 u from the hero, when L2 or
  R2 is let go, or when it dies.
- **What it does:**
  - Both strafe buttons are held, so he strafes and faces the camera.
  - **The camera turns to look past him at the target** (`camera.md`, "Lock-on"), so he
    ends up facing it.
  - Weapons read the target, and `lock_reticle` draws rings on it.

### In the kit [code numbers; simplified where marked]

- `HeroState::Walk` with `HeroMotion::strafing`: everything above, in `rac_hero`'s
  `controller.rs` (`strafe_walk`, `turn_to_camera`) and the profile's `strafe` section.
  The trace's corrections went in the same day: the stop that waits for him to face the
  camera, and the standing jump.
- `rac_hero::lock_on`: a `LockOn` component on each hero, `lock_on::update` each tick, and
  the camera turn in `FollowCamera`. The profile's `lock_on` section holds the search.
  - **[simplified]** The kit's sight line stops 0.75 u short of the target's middle,
    because the kit's targets can be solid, and a line into one would always be blocked.
    The game's ray probably ignores objects.
  - There is no weapon yet (P1.9), so lock-on works with anything.
  - **Gamepad only (PLAN §5, 2026-10-15):** `Action::LockOn` is
    bound to both triggers, and `rac_input` ignores keyboard and mouse bindings for it,
    because with a mouse the player aims themselves.
- Standing while strafing, he steps round to the camera only once it is more than
  `strafe.turn_after_degrees` (10°) off his facing, then idles within 2° (`camera.md`,
  "The hero in these modes"). `HeroState::Look` (state 1) is the first-person button's
  look: he stands, and turns to the camera's yaw once its view is ready
  (`HeroMotion::look_ready`, which the camera sets).
- `rac_rig`: `RigMove::StrafeBack` plays sequence 20. Sideways, the rig turns the hips by
  the angle to the movement and the chest back by the same **[assumed: the slots' joints
  aren't read]**.
- Tests: `rac_hero/tests/strafe.rs` (14), the camera's lock tests, `rac_input`'s pad-only
  tests, and the greybox's `the_melee_yard_has_a_dummy_to_lock_on_to`.
- Seen in the running demo (Xvfb, a replayed recording with lock-on held): the lock picks
  a crate, the demo's ring marks it, and the camera comes round behind the hero to it.

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
- **Stick released:** when the speed the stick asks for is under 20% of the walk speed
  (for a jump at the walk's air speed, a stick under 20% of full tilt), the velocity
  bleeds off at 8 u/s² instead, 0.0032 per frame on PAL. Read in UYA's code (0x4EDE80)
  and DL's ("Differences between the games").
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

There are three ways in: the strafe flip, from a walk or as a jump lands, and the crouch
flip, which the controls call the "backflip / sideflip" ("R1 + stick + X: flip in the air
from a crouching position in the intended direction"). All enter state 11 through the jump
entry (0x4FC518). Every flip records its side (left 0, right 1, forward 2, back 3;
`hero_strafe_side`, 0x4FB758), and the animation is sequence 28 plus that side ("Animations
per state"). Deadlocked keeps only the landing flip ("Deadlocked's flips", below).

- **The side:** while strafing, where the stick points; otherwise the stick (the move
  angle, or the facing with the stick released) against the hero's yaw, in the same
  90-degree sectors with an exact diagonal as forward. A forward side never flips.
- **Which motion:** the strafe flip's while strafing with the stick past 0.3 when the flip
  starts (a flag at 0x1A50DD), otherwise the crouch flip's.
- **Vertical:** the standard model with 3.2–3.25 u, for both.

#### The strafe flip

- **Input:** while strafing (L2 or R2), push left, right or back past 0.9 and press X. A
  diagonal or forward strafe is an ordinary jump ("Strafing and lock-on").
- **Horizontal:**
  - The flip moves in the stick direction at `max(2, 5.5 × m)` u/s: 0.091667 u/frame at
    full tilt, 0.11 on PAL. m is the stick magnitude each frame, so the speed follows the
    stick through the flip [code].
  - It starts at 5 u/s and closes the gap at 47 u/s², so at full tilt it is at speed on
    the first frame.
- Gladiator's traced flip was a strafe side flip at the same 5.5 u/s, so Gladiator's
  default controls strafe there.

#### The landing flip [code in UYA NTSC and DL NTSC; verified on DL NTSC, 2026-10-20]

A jump pressed as a jump-type move lands is a jump again, and while strafing it can be the
strafe flip. Read in the jump states' case of `hero_transitions` (UYA 0x503B68, DL 0x6A45F0;
the states 7, 9–18, 27 and a few scripted ones, the double jump among them).
- **When:** on the frame the move lands (the jump block's landed byte, UYA 0x1A504C,
  DL hero+0x1D0C, becomes 1) and the next (it becomes 2, and the move then walks or
  idles). The long jump goes to the skid instead.
- **What:** X pressed within the last 8 frames, input not locked, and off the ground fewer
  than 8 frames (UYA 0x1A4F4E) or 4 (DL hero+0x2FE). Then:
  - **a flip (11)** when strafing (UYA 0x1A6014, DL hero+0x5F4) or in the lock-strafe
    camera with R1 or Circle pressed in the last 2 frames (raw pad bits 0x28), with the
    stick past 0.9 and `hero_strafe_side` not forward;
  - otherwise a run jump (9) when moving faster than a threshold (UYA 0x1DBE4C) within
    60° of the facing and faster than 0.85 of the walk speed, else a standing jump (7).
- **Deadlocked's extra conditions:** the lock-strafe path also needs the gadget in hand to
  be neither the wrench (1) nor the flail (15, libdl's `WEAPON_ID_FLAIL`), and not in state
  16; and in first person (the camera byte 2) a back strafe doesn't flip.
- **[verified on DL NTSC]** (`dlntsc-dreadzone-flip-1`, third person, L2 held, X tapped
  every 0.15 s): double jumps strafing left, right and back landed straight into flips on
  presses 2, 6 and 7 frames old. A landing whose press was 9 frames old walked, and the
  next press jumped (7). Strafing forward, or sideways without L2 (take 2), gave only
  standing and run jumps.
- UYA's walk flips anyway while strafing, so there the landing flip only adds a second
  frame to press in; in Deadlocked it is the only flip.
- **In the kit:** `strafe.landing_window` and `landing_ticks` (8 and 2 in both profiles):
  the first two ground ticks after a jump-type move lands (`HeroMotion`'s `landed_ticks`)
  stand for the game's landed jump state. Tested in `rac_hero/tests/deadlocked.rs`.
  **[simplified]** The re-jump that doesn't flip is the walk's (run jump by the stick, not
  by the speed and its angle to the facing), and Deadlocked's lock-strafe and first-person
  conditions are left out: they need a gun in hand, which the kit doesn't have yet.

#### The crouch flip [code, verified on UYA NTSC, 2026-10-15]

Read in Veldin's build (`hero_crouch_jump` 0x502ED8, `hero_walk_jump` 0x502368,
`hero_transitions` 0x503B68, the air update 0x4EDE80, the flip's block at 0x1A5070–0x1A50DD),
and traced in `uyantsc-veldin-crouchflip-1` (seven takes: left, right and back from a
crouch, X before the stick, a long crouch turn, from a run, and all three buttons at once
from standing). Over the six flips' 316 frames in the air, the rules below reproduce the
flip's own speed and the horizontal velocity to 10⁻⁸ u a frame, and the facing to 6×10⁻⁶
rad.
- **The stick press:** the pad layer turns the stick into presses ("Input"): a direction
  press when an axis leaves its dead zone or crosses to the other side, and a flick. The
  flip looks for any of them (the mask 0x1F000), so the stick has to be pushed at about
  the time of X. Held to the side, it only turns him (take 5).
- **Three ways in, all with R1 held and the side not forward:**
  1. **From a crouch (state 4):** X and a stick press, each within the last 17 frames
     and fewer than 12 frames apart (`pad_pressed_together`, 0x45D3E8). Unlike the
     crouch's other jumps, it doesn't wait for the crouch to be 7 frames old: take 6
     flipped on its crouch's 5th frame.
  2. **From idle or a walk:** X within the walk's jump window (7 frames idle, 9 walking),
     with a stick press within that plus 12 frames and fewer than 12 apart. Pressing R1,
     the stick and X on one frame flips straight from idle (take 7).
  3. **From a jump still on the ground:** a standing, run, long or heli jump before its
     12th frame, on the ground (the ground counter at 0x1A4F40 non-zero), with a stick
     press in the last 5 frames, becomes the flip. In take 4, X from an old crouch gave
     the heli jump, and the stick pushed 6 frames later, during its push-off, flipped.
     The flip itself can only flip again this way once it has landed.
- **Horizontal:**
  - **Its own speed** starts at 0 and grows by 8 u/s² (0.0022222 u a frame) to 4.2 u/s
    (0.07) along the stick's direction at take-off, reached on its 32nd frame.
  - **Near the landing it slows:** once past the apex (0x1A504E), with 11 or fewer
    frames to the landing, it eases back to 0 at the same 8 u/s². The frames to the
    landing are worked out each frame (`hero_ticks_to_land`, 0x50C4A0) as the whole
    number of frames t for which h + v·t − g·t(t + 1)/2 reaches 0 (per-frame units, h the
    height over the ground the path meets), and the next frame acts on them. Take 1 held
    4.2 u/s for 11 frames and slowed for 10.
  - **The run carries on:** the speed he had when it started (0x1A4D54), if over 1.5 u/s
    (0.025 u a frame), carries on along the way he faced 10 frames before the flip, scaled
    by the sine of the angle from there to the flip's way. A side flip from a run keeps its
    whole speed forward while it moves sideways; a back flip keeps almost none. Take 6
    carried 3.82 u/s.
  - **Short of a drop [code]:** from frame 13, under 3 u over the ground, the flip looks
    0.57 u and 5.5 u ahead along its motion for ground between 2 u above and 3 u below its
    take-off height. With neither, its own speed is zeroed, for both kinds of flip.
- **Facing:** he turns back to the way he faced 10 frames before the flip (the hero keeps
  his yaw at the start of each of the last 32 frames, 0x1A724C; `hero_yaw_back`,
  0x4E5630), on the spring 0.04 gain, 0.2 damping, 0.11636 rad a frame cap. So the turn a
  crouch made towards the stick is undone. While strafing, it turns to the camera instead.
- **Landing:** the ground speed it lands with (0x1A4D94) is the carried speed, or none from
  a back flip. As for every jump-type state, landing with the stick under 0.2 caps the
  ground speed at 2.5 u/s (0.041666668 u a frame): take 6 carried 3.82 u/s and landed at
  2.5 with the stick let go.
- **In the kit:** `rac_hero`'s `controller.rs` (`flip_side`, `crouch_flip_due`,
  `flip_tick`, `ticks_to_land`, `flip_over_drop`) and `stick.rs` (`StickPresses`), with the
  profile's `flip.crouch`, `stick.flick`, `walk.crouch_turn` and `air.land_release_*`.
  Simplified:
  - the frames to the landing use the ground straight below, not where the path meets
    the ground (the same on flat ground);
  - the carried speed is the last tick's, where the game first brakes the crouch once
    more, as with the long jump (PLAN.md P1.5, "Known mismatch").

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
  - From a standing or run jump (7, 9), not before the jump's 15th frame (t > 14). DL
    allows it from the 11th (t > 10) **[verified on DL NTSC]**.
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
  - Its timing, aim and hits: "Melee".

## Melee (states 19–21 and 42) [code, verified on UYA NTSC, 2026-10-13]

Read in UYA NTSC's code on Veldin:
- the attack check that runs before the transitions' switch (`hero_attack_start`,
  0x501678);
- `hero_change_state`, the state handlers in `hero_update_state`, and `hero_transitions`;
- the wrench's own object (0x338088);
- the auto-aim (0x4EEA78, 0x4E5BB8, 0x4E56A8).

Two of our own traces check it:
- `uyantsc-veldin-moves-2`: a standing combo and jump attack;
- `uyantsc-veldin-melee-1`: two standing throws, a throw from a walking crouch, a combo
  with the stick held, and a run-jump attack.

Their layout, `uyantsc-veldin-melee.toml`, adds the attack's fields and the wrench
object to the hero's (`docs/re-setup.md`). Since the traces keep the whole hero struct,
the layout also reads the attack fields out of older traces.

The ground swings are UYA's wrench combo, the jump attack is its downward strike, and
the throw sends the wrench out and back (P1.8).

### Animation frames

- **Every attack is timed by the hero's animation frame (0x1A57F8) [verified].**
  1. An attack's animation blends in over a number of frames. During the blend the frame
     creeps from the start frame by 1/blend a frame.
  2. When the blend ends, the frame goes back to the start frame.
  3. From then on it advances 0.5 a frame at rate 1: the global 0x1A57E4 holds 0.5.

  Read in the animation code since (`moby-format.md`, "Sequences and keyframes"): the
  animation frame is the keyframes' time stamps over 16, interpolated between keyframes.
  0x1A57E4 is 1 / the current keyframe's length in ticks, so 0.5 means keyframes 2 ticks
  long, which most hero keyframes are.

  | Attack | Start frame | Blend (frames) |
  |---|---|---|
  | Swing 1 | 16 | 5 |
  | Swing 2 | 7 | 7 |
  | Swing 3 | 2 | 7 |
  | Jump attack | 19 | 11; 17 within 42 frames of a flip |
  | Throw | 30 | 5; 3 straight after another throw; 17 from first-person look |

- **The transitions see the frame the animation reaches in that frame.**
- **An attack's own motion starts on the frame after the one that entered its state.**
  In the traces, the row where the state first shows still has the old state's motion.

### The attack table (0x2F6310)

- **Each row is eleven words, 0x2C bytes.**
- **Rows:**
  - 0–2: the combo's swings;
  - 3: the throw;
  - 4: the jump attack;
  - 5–6: state 114's two swings;
  - 7–8: state 77's.
- The grind attack uses rows 0 and 1, the combo's first two swings' frames, on animations
  of its own ("The grind attack").
- **Words:**
  - the kind: 0 a swing, 1 the jump attack, 2 the throw;
  - the swing's number;
  - then seven animation frames, used as the columns below;
  - two more words, not read.

| Row | Chain before | Presses from | Chain from | Jump from | End | Hits |
|---|---|---|---|---|---|---|
| 0, swing 1 | 33 | 19 | 23 | 24 | 26 | 17–23 |
| 1, swing 2 | 25 | 7 | 11 | 13 | 15 | 8–13 |
| 2, swing 3 | 0 | 16 | 19 | 19 | 21 | 12–16 |
| 3, throw | 99 | 83 | 88 | 35 | 91 | none |
| 4, jump attack | 99 | 32 | 28 | 28 | 30 | 1–28 |

### Starting an attack

- **The wrench must be in the hand, not thrown.** In single player the wrench is hand
  item 9. The multiplayer wrench is item 0x7F, and its object (0x356110) runs the same
  checks.
- **Standing, walking or skidding (state types 0 and 1):**
  - Square pressed in the last 10 frames starts the combo.
  - With R1 (crouch) held as well, it throws.
- **Crouching (state 4):** Square in the last 15 frames throws.
- **In the air:**
  - From these states:
    - the jumps (7, 9, 10, 13, 15), the wall jump (17), the glide (8) and the fall (6);
    - the double jump (14) once its animation is past frame 32, which is 20 frames in;
    - the flips (11) once theirs is past frame 18, which is 31 frames in. One flip kind
      waits for frame 20 instead; which kind isn't read.
  - The state must also be at least 11 frames old.
  - Square counts if pressed in the last 14 frames, or 18 after a flip or double jump.
  - Within 0.1 u of the ground (0.57 with a flag set), this starts a combo swing.
    Otherwise it starts the jump attack.
- **During an attack (type 6):**
  - The attack can't be blending, and the wrench must be in hand.
  - From the row's chain-from frame, Square pressed within (frame − presses-from) × 2
    frames starts a combo swing. That window is at least 1 frame and at most 15.
  - With R1 held, the same press throws instead.
  - A throw more than 30 frames old, with R1 held, can throw again from its chain-from
    frame.
- **Which swing it is (`hero_change_state`):**
  - Entered straight from a swing whose frame is still under its chain-before frame: the
    next swing. After swing 3 that is swing 1.
  - Entered from the state after a swing, less than 27 frames after that swing ended:
    also the next swing.
  - Otherwise, swing 1.
  - So a pause of up to about half a second keeps the combo going, and too early a press
    is lost: one before the presses-from frame never counts.

### Ending an attack

- **Swings and the throw [verified]:**
  - **Jumping out:** past the jump-from frame and not blending. An X pressed within
    (frame − presses-from) × 2 frames jumps (8 frames for the throw). It is a standing
    jump (state 7), or a jump from a crouch with R1 held.
  - **Walking out:** past frame 34 with the stick over 0.9.
  - **Past the end frame:**
    - with the stick over 0.8, he walks;
    - otherwise he idles 5 frames later, or when the animation finishes.
  - In the traces, swing 3 idles at frame 26.5 and walks at 21.5 with the stick held.
- **Jump attack:** past its end frame (30) he idles. The trace has it at 30.5.

### Motion

- **Swings lunge at a speed set by the swing and the frame [verified tick by tick in both
  traces]:**
  - **Speeds:**
    - swing 1: 4.4 u/s until frame 18 (0.073333);
    - swing 2: 5.7 u/s up to frame 7, 4.5 to frame 8, and 3.0 to frame 9 (0.095, 0.075,
      0.05);
    - swing 3: 3.7 u/s until frame 12 (0.061667).
  - The lunge also runs during the blend.
  - **Speeding up and slowing down:** it reaches its speed at 37 u/s² (0.010278) and slows
    at 28 u/s² (0.0077778). A swing begun at a run drops at once to the lunge's speed.
- **Direction:**
  - A swing goes the way he faces.
  - With the stick past 0.5, it goes the stick's way at once, and he turns to face it.
- **Reaching a target:** a target found in the first three frames scales the lunge's
  speed.
  - The scale is ((horizontal distance − 0.5) − offset) / 0.9, at most 4 (1.9 for
    swing 3).
  - It is used only when over 1, and when the distance less 0.5 is within range + 3 of the
    offset.

    | Swing | Offset | Range |
    |---|---|---|
    | 1 | 0.75 | 0.8 |
    | 2 | 0.6 | 1.7 |
    | 3 | 1.3 | 1.7 |

  - The acceleration stays the same, so the swing only goes a little further.
- **Turning:** towards the target or the stick's direction, with the angle spring (gain
  0.05, damping 0.2, cap 0.2822 rad a frame).
  - Swing 2 stops looking for a target past frame 7.
  - The throw turns with a gain of 0.057.
- **Gravity:** a swing off the ground falls at 25 u/s² (0.0069444). On the ground, 54 u/s²
  holds it down.
- **Edges:** a moving swing on the ground stops dead at an edge.
  - It looks 3.7 frames of its motion ahead, from 0.3 u above the feet to 0.35 u below.
  - It stops if it finds no ground there, or ground steeper than 50°.
- **The jump attack:** its vertical motion is under "Falling and other airborne moves".
  - **Animation rate [verified]:**
    1. 0.33 on the way up.
    2. Then the rate that brings frame 26 to the landing: frames left / (frames to land ×
       0.5). The frames to land assume 130 u/s² from the height above the ground, up to
       60. Below 0.7 u the rate is at least 0.75.
    3. 2.0 once on the ground, until frame 26.
  - **With a target 2–5 u away,** it keeps 4.5 u/s towards it, changing speed at 25 u/s²
    (0.0069444).
- **The throw** slows to a stop at 28 u/s², turns to its aim, and lets go at frame 33.

### Aim (0x4EEA78)

- **When:** at the start of each attack, then every frame while the attack has neither a
  target nor a set direction.
  - Swing 2 stops looking past frame 7, the jump attack past frame 25, and the throw once
    it has blended in.
- **The stick:** past 0.5 it sets the direction to the stick's, and the target search
  looks that way.
- **Targets:**
  - The game scores the objects on its target list (0x21E8E0, flag 0x1000).
  - It rejects any further than the range, further round from the direction than the
    cone, or further up or down than the rise.
  - The least distance × (1 + angle off) wins. One class of object counts a further
    0.8 × the range.
  - A target, once found, stays the attack's target.

| Attack | Range | Cone | Rise |
|---|---|---|---|
| Swings | 5 u | 45° | 45° |
| Jump attack | 8 u | 60° | none at the start, 60° after; measured from the ground under him |
| Throw | 14 u | 30° | none at the start, 45° after |

### What the wrench hits

- **The line [verified for its shape, traced]:**
  - Each frame the wrench's object saves two points (at 0x1A5720 and 0x1A5730), and
    last frame's pair (at 0x1A5740 and 0x1A5750):
    - the hero's joint 0, the wrench hand;
    - the wrench's joint 1, its tip.
  - The tip is pushed 0.17 u further out.
  - Then the line is shortened by how far round from his facing it points (0x337FD0):
    full length straight ahead, falling evenly to 70% at 105° and staying there beyond.
    The grind attack skips this.
  - The kit's swing keys are these points, relative to his feet, from the traces'
    standing swings. Each reaches about 2.4 u ahead at 0.4–0.75 u up; swing 3 reaches
    2.0 u. Swing 1 sweeps right to left, swings 2 and 3 left to right. The jump attack
    holds the wrench 1.8 u behind and low as it rises, then brings it over the top to
    2.2 u ahead at the landing.
- **The hit:** only during the row's hit frames, not blending, and with the tip within 90°
  of his facing (the jump attack at any angle).
  - The line is swept from last frame's to this frame's as five lines in between
    (0x443AB8). Anything they cross is hit.
  - A ball round the wrench's head, 0.085 u in from the tip, hits too. Its radius is
    0.35, or 0.47 in the jump attack and 0.7 in the grind attack.
- **Damage:**
  - It comes from the wrench's weapon row. Rows are 0xF0 bytes from 0x2C5640; an item's row
    number is at 0x1425C0 + item.
  - The starting wrench does 1.0 for swings and the throw (+0x6C) and 2.0 for the jump
    attack (+0x70).
- **Push:**
  - A swing pushes across the wrench's line: to the hero's left for swings 1 and 3, to
    his right for swing 2. Its strength is 1.0.
  - The jump attack pushes forward at 1.55.
- **Once per attack:** each hit carries the attack's number, so a target takes one hit
  per swing. To recheck: the field the hit record fills there (+0x1A) is copied from the
  wrench object's +0xAA, which DL's code fills with the wrench's class instead (DL's
  reading below), and the only latch per attack found in either game is the clank's.
- **The jump attack's slam:** past frame 23.5, while falling and in the air, a ball of
  0.4 u at 0.5 u under his feet hits for 2, pushing forward at 2.
- **The clank off a wall [code, 0x338088]:** a wall in the wrench's way sparks and sounds,
  but doesn't stop the swing, slow it or knock him back.
  - **When:** in the combo and the jump attack (the type-6 states but for 114, and not the
    grind attack), once the state is more than 10 frames old, with the tip within 70° of
    his facing; the jump attack's tip may be anywhere.
  - **The test:** a ray from 0.5 u above his feet, 1.4 u horizontally towards the tip,
    shortened as the wrench's line is. It must hit a surface whose material isn't 0
    (0x3FC730 gives the hit's material, the low five bits of the collision result).
  - **Once per attack:** the clank sets a flag (0x1A57BE) that a new attack clears, and
    the wrench's first hit on a target sets it too. With it set there is no more clank,
    and no hit sound or sparks either.
  - **What it does:** the wrench object plays its sound 2 (0x4A7200 plays a class's sound
    by number), sprays sparks at the hit point (0x337BD8, about 30 of them, purely for
    show), and if the ray hit an object, hands that object 1 damage, pushed the ray's way
    (0x443938, the damage queue).
- **The jump attack's ground clank:** when its frame passes 27 it plays the same sound,
  and sparks where a ray from 0.7 u above the tip to 0.9 u below it meets the ground.

### The thrown wrench (0x4F1A08, 0x338088)

- **Release:** when the throw animation passes frame 33, the throw's 12th frame
  **[verified]**.
- **Out [verified]:**
  - It leaves from the hand, 0.67 u ahead, 0.5 u right and 0.39 u up **[traced]**, along
    his facing at 23 u/s (0.38333 a frame).
  - It doesn't carry the hero's motion: walking after it doesn't change its path.
  - Each frame it moves, then slows. The slowing grows by 0.000787 u/frame² each frame,
    170 u/s³. The trace's steps are 0.3833, 0.3826, 0.3810, 0.3786 and so on.
  - After 31 moves, 7.98 u, it stops and turns back: 8.65 u from the hero.
- **Height:** within 0.8 u of the ground, it keeps 0.55 u above it, rising at 4 u/s and
  sinking at 1 u/s.
- **Walls:** a wall in its way turns it back at once, at half speed.
- **Back [verified]:**
  - Its slowing carries over as acceleration, growing by 0.00025 a frame (54 u/s³).
  - It flies straight to the hand, through walls.
  - It is caught when closer than a frame's travel, after 25 frames in the trace. If it
    is over 32 u away it is back in the hand at once.
- **The catch:** when it is under 4 frames from the hand, the hero's animation jumps to
  frame 82, the catch. A throw held for 101 frames plays it anyway.
  - The trace has the jump a frame later than this reading gives.
- **Hits:**
  - Damage 1.
  - A ball of 0.4 u round each end of the wrench.
  - It pushes at 1.2 along the throw on the way out, and at 0.37 back towards the hero
    on the way back.
- **Duration:** standing still, the throw lasts 95 frames. He can walk off once the
  wrench is gone, at frame 34.

### The grind attack (state 42) [code, UYA NTSC, 2026-10-15]

Read in `hero_change_state`, `hero_update_state`, `hero_transitions` and the wrench object
(0x338088). UYA has nowhere to grind ("Grinding"), so it is untraced.

- **Starting:** on a rail (state 39), Square in the last 10 frames, while the rail hasn't
  run out (both rail-end counters 0).
- **Which swing:** the first, unless it comes straight from a grind swing, when it is the
  other one. Its state type is the grind's (15), not an attack's.
  - **Swing 1:** the hero's sequence 70 and the wrench's 12;
  - **swing 2:** sequences 71 and 13.
  - Each starts from its first keyframe, blending over 4 frames. That is frame 14 of
    sequence 70 and frame 6 of 71, both at 0.5 frames a frame.
- **Its frames are the table's rows 0 and 1** (the combo's first two swings):
  - swing 1 hits on frames 17–23, can jump from 24 and ends after 26;
  - swing 2 hits on 8–13, can jump from 13 and ends after 15.
- **The transitions, in order:**
  1. X in the last 5 frames, from the jump frame on: the grind jump (40).
  2. Square in the last 10 frames: past frame 23 (swing 1) or 14 (swing 2), the other
     swing. Before that the press waits, and the swing doesn't end meanwhile.
  3. Past the end frame: back to the grind (39), whose speed carries on.
  4. The rail running out (6 frames) before then: the fall (6), as from the grind.
- **Motion:** the grind's own. The shared update for states 39–42 carries him along the
  rail at the grind's speed and turns him side on to it ("Grinding").
- **Hits:**
  - **The wrench's line** hits on the row's frames, at any angle from his facing (the grind
    attack skips the in-front test and the shortening). Its head ball is 0.7 u.
  - **Damage and push** are a swing's: 1, pushing across the line at 1.0, to the left for
    swing 1 and to the right for swing 2.
  - **A ball round him:** while the state is under 20 frames old, a ball of 1 u round a
    point 0.6 u above his feet hits for 1. It pushes along the rail turned 20°–35° to
    the left, drawn at random each frame. It carries no attack number.
  - **No clank:** the wall ray isn't cast.
- **The wrench's line, from the animations [computed, 2026-10-15]:**
  - **Method:** posing the hero's skeleton from the disc. The hand is point 0 of the
    hero's class, joint 56; the tip is point 1 of the wrench's class (0x47), its joint 5.
    A class's points are chains of joints listed at class + 0x1C, the last being the
    point; `moby_point_position` (0x437698) takes the chain's last joint's world origin.
  - **The wrench's pose:** the wrench sits on joint 56 and plays its own animation, which
    stretches it from about 0.8 u to over 3 u along a swing.
  - **Checked against the combo's traced keys:** the wrench's animation runs a tick (half
    a frame) ahead of the hero's. With that, the 0.17 u push and the shortening above, the
    hand lands within 0.006–0.1 u RMS of the traced keys. The tip lands within 0.04 u
    (swing 3), 0.12 (swing 2) and 0.2 (swing 1, the fastest sweep, which moves up to
    1.4 u a half frame).
  - **Swing 1's tip** sweeps from his right across his front and out to his left. That is
    ahead along the rail, reaching 3.3 u at frame 19.5.
  - **Swing 2's** comes from behind, out ahead along the rail and round to his front.

### Not read yet

- State 114 (rows 5–6), state 77 (rows 7–8) and state 32 (the attack bounce).
- The wrench upgrades' rows.
- The giant-Clank mode (flag 0x1A4E14): damage × 50, a stronger lunge and a wider aim.
- What material 0 is, which the clank skips.

### In the kit

- `rac_hero/src/controller/melee.rs` and the profile's `melee` section implement the
  above, the grind attack and the clank included. Hits are tested in `rac_combat`. A
  clank is `HeroMotion::clank` for the tick, with the hit point and normal; the demo shows
  a spark there.
- Simplifications:
  - the swing's line comes from the traced keys, at every half frame; the grind attack's
    from the keys computed above;
  - the grind attack's ball pushes at 27.5°, the middle of the game's random 20°–35°;
  - a clank happens on any surface, and only a wall stops later clanks, not a hit (the
    controller doesn't know what its strikes hit); what it damages is a point strike at
    the hit;
  - the jump attack's ground clank is only reported where the ray finds ground, though
    the game plays its sound either way;
  - the thrown wrench hits as one ball at its middle;
  - targets are points;
  - the animation holds at frame 81 while the wrench is out;
  - the kit's catch starts a tick before the trace's, so its throw lasts 94 ticks to the
    game's 95.

### Deadlocked's melee [code, verified on DL NTSC, 2026-10-15]

**Sources:**
- **Code:** read in DL NTSC's code on DreadZone Station (`re/SCUS-97465/symbols.toml`):
  - the attack states' handlers, found through the state tables;
  - the attack start, which is part of `hero_common_transitions` (0x6A5248) and runs where
    UYA's `hero_attack_start` does;
  - DL's attack table (0x3B0358) and the wrench object (0x460530).
- **Traces:** five of our own, `dlntsc-dreadzone-melee-1` to `-5` (`docs/re-setup.md`),
  from the third-person savestate:
  - single swings and the combo at two paces;
  - swings from a run;
  - jump attacks from a standing jump, a run jump and double jumps;
  - throws;
  - Square, X and R1 pressed into a swing.
- **Getting the wrench:** the savestate's hero holds a gun, and the first Square swaps it
  for the wrench and swings.

**The same as UYA [code]:**
- the states (19 combo, 20 jump attack, 21 throw);
- the press windows: 10 frames on the ground, 15 crouching, 14 or 18 in the air, and the
  chain window from the presses-from frame;
- how the next swing is chosen;
- the walk-out and the end;
- the jump attack's rise, hold, fall, braking and carry;
- the aim's ranges and cones, and the reach scale;
- the edge stop;
- the throw's motion and the thrown wrench's flight;
- the wrench line's 0.17 u push and its shortening;
- the pushes' directions.

The table's rows 0–4 are UYA's but for the jump attack's last hit frame. The lunges'
acceleration (37 u/s²) and braking (28 u/s²) match **[traced]**.

**Where DL differs:**
- **The animation frame [traced]:** an attack's frame doesn't creep through a blend and
  start over. It runs from the start frame at 0.5 a frame, already half a frame on in
  the frame the attack begins, and the blending flag (hero + 0x64C) stays 0. Swing 1
  shows 15.5, 16, 16.5 … in its first frames.
- **Swing 1's start frame:** 15, keyframe 1 of DL's sequence 40 (UYA 16) **[code,
  traced]**.
- **Lunge speeds [code, traced]:**
  - swing 1: 4.75 u/s while the frame is under 20;
  - swing 2: 5.95, then 4.75 past frame 7 and 3.25 past frame 10, while under 12;
  - swing 3: 3.95 while under 12.
  - DL's update tests the frame before the frame's own advance. So a lunge holds through
    the frame whose frame number reaches the limit, as all three swings' traces show.
- **The head ball:** 0.75 u for the swings and 1.75 for the jump attack (UYA 0.35 and
  0.47) **[code]**.
- **Damage** comes from the wrench's level (the gadget box and DL's gadget tables at
  0x3BE158 and 0x3BE5B8), not a fixed weapon row **[code]**:

  | Level | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
  |---|---|---|---|---|---|---|---|---|
  | Swings, thrown wrench | 1 | 3 | 5 | 7 | 10 | 15 | 20 | 25 |
  | Jump attack and its slam | 4 | 12 | 20 | 28 | 40 | 60 | 80 | 100 |

  - The push grows with it: (1 + level / 5), times 1.55 in the jump attack.
  - Multiplayer reads other columns.
- **The jump attack** hits to frame 29 (UYA 28). While in the air 16 or more frames from
  landing, its animation runs at 0.01, all but stopped **[code]**. The trace's standing
  jump attack holds at frame 21.3 for several frames that way **[traced]**.
- **No throw from a chain [code, traced]:** a press with R1 held during an attack swings,
  where UYA throws. R1 and Square at swing 1's frame 28 chained swing 2. The throw-again
  branch is dead too.
- **Jumping out is always a standing jump [code, traced]:** R1 and X at swing 1's frame
  26 jumped as from standing (state 7).
- **From a double jump**, an attack can start once its animation is past frame 16 (UYA
  32) **[code]**. That is its 12th frame.
  - In the traces, a press on its 13th frame attacked on its 14th.
  - One held from its 9th waited until its 15th, which this rule doesn't explain.
- **The throw's button** follows the camera option: L2 in lock-strafe and first person,
  L1 or R1 in third person. In the kit it is crouch, as in UYA.
- **The thrown wrench** hits as a ball of 0.75 u at either end (UYA 0.4), and its catch
  starts within 5 frames of the hand (UYA 4) **[code]**.
- **The clank** does no damage to what it hits, and the jump attack's ground clank comes
  at frame 28 (UYA 27) **[code]**.
- **The grind attack's code is there, but nothing starts it:** the grind reads Square
  and drops the result **[code, medium-high confidence]**.
- **Blocking:** Triangle (and R2 in lock-strafe) blocks a ground attack; R2 blocks one in
  the air. Neither is modelled.

**Not read yet:**
- DL's other melee states:
  - the flail (145, its own combo over table rows 7–10);
  - the shuriken throw (133);
  - the glove (34);
  - the magnetic attack (111, rows 5–6).
- DL's throw: with the wrench back in the hand, the animation jumps to frame 62. As read,
  that would cut a throw short before the release, so something in DL's blend or
  release isn't understood yet.
- Hero fields +0x4AC (it blocks the combo) and +0x435/+0x436/+0x439.

**In the kit:** `dl.ron`'s `melee` section has all of the above at the wrench's level 0.
- The wrench's line is DL's own, from the traces: one standing swing each, and a
  standing jump attack.
- Switches for the differences:
  - `blend_creeps`, `chain_throw`, `crouch_jump_out`;
  - the jump attack's `hover_ticks`;
  - a grind attack press window of 0.
- **Simplified:** DL's lunge frames are given half a frame later than the code's, as the
  kit tests the frame after its advance.

## Ledges and getting hit

- **Get hit (state 22):** see "Getting hit and dying".
- **Ledges (23–27):** see "Ledges".
- The Gladiator traces contain ledge grabs and ledge jumps that we have not yet compared
  with this.

## Wall jump (state 17) [code, then verified on UYA NTSC, 2026-10-13]

Read in `hero_detect_wall` (0x4E39B8), the transitions (`hero_transitions`, 0x503B68),
`hero_change_state` and the air phase (`hero_air_horizontal`, 0x4EDE80).

**Checked against our own UYA NTSC traces** (`uyantsc-phoenix-wall-{1,2}`, scripted keyboard
play against a side wall of the Starship Phoenix's bridge corridor, level 3;
`docs/re-setup.md`). A standing jump at an ordinary wall, then X again 0.13 s later. These
match the code:
- **The wall window** opened on the jump's 6th frame, the first with the feet 0.4 u or more
  up (0.53 u), and counted down from there.
  - It wasn't set again while he rose past the low blocks beside him, so it lasted 5
    frames.
  - X pressed later became a double jump. So against a low-based wall the timing is
    tight.
- **The wall jump's parameters:** 1.8–1.85 u (tapped and held), push-off 9 frames, hold
  12, gravity 29 u/s².
- **The cling:** the 9 push-off frames hold him against the wall, sinking 0.0133 u a
  frame.
- **The launch:** sqrt(2 × height × gravity) is added to that sinking speed, less one
  frame's gravity, as in every jump.
- **Moving away:** from the 10th frame (`t > 8` in the code) he moves at exactly 4.9 u/s
  along the wall's normal, with no air control.
- **The turn** towards the normal starts on the 11th frame: gain 0.042, damping 0.2. Its
  cap is 0.197804 rad a frame (680°/s), which the code reading rounded to 0.198.
  - It uses the game's spring: the rate is first clamped to the error, and the yaw snaps
    to the target once the step leaves it within 1% of the cap.
- Landing on the edge of a low block afterwards, he went through state 123 for 9 frames
  before falling. That state isn't read.

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
    0.2 and cap 0.197804 rad/frame [verified].
  - Entering the state also writes 1.7 u/s, or 4.9 u/s scaled by the open space behind,
    to a jump-block field that no code in the Veldin overlay reads.

## Ledges (states 23–27) [code, UYA NTSC, 2026-10-12; traced 2026-10-13]

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
- **Shimmy (25, 26)** [verified]:
  - Each frame the detector runs at a point 0.2 u to the side, and the hero moves towards
    the hang point it finds. The code sets the side vector's length to 0.2 with the
    helper (0x40C0F0) that sets each step's length below; an earlier reading took it as
    0.2 × the hang point's 0.5 u offset from the lip, so 0.1 u [code, 2026-10-20].
  - **Each step's length** is 0.5 × a per-animation-frame factor, whatever the distance:
    the code sets the vector's length. The factors for the 15 frames are 0, 0, 0.025,
    0.05, 0.075, 0.162, 0.162, 0.107, 0.037, 0.037, 0.037, −0.017, −0.017, −0.035 and
    −0.035.
    - The animation advances 0.5 frames a tick, after 6 ticks of blending in at 1/6.
    - A cycle ends when the animation reaches frame 14, so the last entry is never used.
      One cycle covers 0.623 u in 28 ticks, about 1.3 u/s if the animation loops on a
      long ledge. The stack was too narrow to see a second cycle.
  - Where the side probe finds nothing, he doesn't move that tick.
  - **It goes on** while the stick stays past 0.5 within 45° of that side and a ledge is
    found 0.3 u that way. Otherwise it returns to hanging when the animation ends. With
    the stick still held, the next shimmy starts from hanging.
  - X climbs; X while pulling away lets go (40-frame no-grab timer).
- **Ledge jump (27)** [verified]:
  - Vertically it is the standard model: 2.0–2.05 u, push-off 4 frames, hold 1 frame,
    gravity 25 u/s². It peaks 1.97 u above the hanging feet, 0.54 u above the lip.
  - **But he isn't pressed down during the push-off:** he hangs still for 5 frames, the
    push-off and one more, and leaves on the 6th at 0.1618 u a frame, √(2 × 25 × 2.05)
    less a frame of gravity (`uyantsc-veldin-ledge-3`, read again 2026-10-20). DL's
    climb does the same after its 2 (below).
  - **Its forward speed follows the animation frame:**
    - below frame 13 it isn't changed, so it stays 0;
    - from 13 to 27 it approaches 2 u/s at 8 u/s²;
    - from 27 to 30 it approaches 0;
    - past 30 it isn't changed again. (The code reading said 0, which was wrong.)
  - **Timing:** the climb's animation starts at frame 12 and runs 0.304 frames a tick.
    So the speed starts on the climb's 5th frame and reaches 2 u/s on its 19th.
    - In the trace the animation then switched to another clip at frame 31 on the 36th
      frame. The speed therefore held at 2 u/s until he landed on top, 41 frames in.
  - After 40 frames he turns and steers like a normal jump.

**Checked against our own UYA NTSC traces** (`uyantsc-veldin-ledge-{1,2,3}`, scripted
keyboard play on Veldin at the two stacked crates west of the start; `docs/re-setup.md`).
These match the code:
- **The grab:**
  - It starts on the first frame the top is 0.5 u or more above the feet (0.550; the
    frame before had 0.463).
  - The hang point is the lip less 1.43 u, 0.5 u out from the wall.
  - He moves towards it at exactly 4 u/s.
  - He hangs on the 14th frame, once the grab animation (which starts at frame 11) has
    passed frame 12.
- **Letting go:** R1, and X with the stick pulled away, each drop him into a fall.
- **The shimmy's steps and the climb,** as above.

**Found in the traces:**
- **Hanging (24) doesn't run the detector into the ledge block; shimmying does.**
  - When a shimmy on the 1 u-wide stack ended at its edge, the hang moved the hero back to
    the hang point of the original grab, at 4 u/s. Meanwhile he turned towards the lip
    the shimmy had last found.
  - About 20° off the wall the hang's own check (the detector at his position, run by the
    transitions) failed, and he fell.
  - Whether a long ledge does the same after each shimmy cycle is untested.

**In the hero kit (P1.5, 2026-10-13):** the shimmy steps by the table as above, and the
climb's forward speed starts on tick 5 and holds from tick 35 (`uya.ron`). **[simplified]:**
- after a shimmy the kit hangs where it ended;
- the kit's capsule keeps a skin's width from the lip, so the climb rubs it for a few
  ticks where the game's hero passes clear.

### Deadlocked's ledges [code and traced, DL NTSC, 2026-10-20]

Read in DL's detector (0x672960, run every frame by the per-frame driver 0x6A65E8 into the
ledge block at hero+0x1DA0: hang point, lip, then the wall's yaw at +0x24 and the found
flag at +0x28), the states' entries in `hero_change_state` (0x699C70), their motion in
`hero_update_state` (0x690F18) and their transitions in `hero_transitions`. Everything not
listed here reads as UYA's: the top's slope, materials and moving objects, the 0.5–1.5 u
rise and 1.8 u drop, the wall face's five probes 0.14 u apart, the hang point (0.5 out,
1.43 down), the same 65° angle test (0x3F91361E) before the lip search, the grab's 4 u/s
and turn, the hang's and shimmy's buttons, windows and angles, the shimmy's 0.2 u side look
and its turn, and the 40-frame timer after letting go mid-shimmy.
- **When it looks:** a jump grabs once past its apex or rising slower than 0.5 u/s (UYA
  3.5), and a double jump from its animation's frame 24 (UYA 37).
- **The top's probe:** one vertical cast 0.3 + 0.3 u past the capsule's front, 1.05 u
  ahead of the feet, from 1.65 u above them to 1.15 below. UYA casts four times, 0.525 to
  0.75 u ahead. That is for the local player's hero (`IsLocal`, hero+0x266D, 1 in the
  savestates); any other hero, while not jumping, casts four times 0.3 u apart, from
  2.4 u above to 1.9 below, and has its found flag set only if a vtable check (+0x54)
  agrees. Online co-op's remote heroes would be such heroes (RE.10).
- **No clear lip:** DL doesn't check for anything just above the lip, back towards the
  hero.
- **The lip stays where the cast hit the top.** The search that should walk it out to the
  wall's edge builds both ends of its short vertical probe as 0.03 u out and 0.05 u below
  (both `vadd`, 0x6730D0, where UYA's second end is 0.05 u above), so the probe has no
  length, never hits, and the first step ends the search.
  - So the lip is 1.05 u ahead of the feet and the hang point 0.55 u ahead of his axis:
    further than a wall he touches lets him come (his radius is 0.45). The grab and the
    hang pull him towards it at 4 u/s, the wall holds him at its face, and he eases down
    to the hang's height, the drop shrinking by about 13% a frame as the blocked part
    comes to dominate the 4 u/s.
- **The grab's animation starts at keyframe 1** (sequence 70, blended over 11 frames),
  where UYA's starts at 11, so it passes frame 12 and hangs on the grab's 24th frame (UYA
  14th). When a speed at hero+0x1B4 is over 3.5 u/s it starts at keyframe 0 over 9 frames
  instead [code].
- **Losing the ledge** from under his hands drops him with a 30-frame no-grab timer (UYA
  10). Letting go on purpose gives 10, and sets libdl's `HideWeapon` byte (hero+0x2663):
  UYA's R1, which in DL's lock-strafe camera is L2 (`pad_action_mask`, "Deadlocked's
  moves"; R1 there does nothing), or X with the stick pulled away.
- **The shimmy steps 1.5 × the same table** (0x3B0BA0) where UYA's is 0.5 ×, towards the
  hang point found 0.2 u to the side. That point is 0.55 u ahead of his axis like the
  hang's, so the step points 70° into the wall, which takes all but 0.342 of it: along the
  wall each step is 0.513 × the table's entry, about UYA's 0.5 ×.
  - The table's negative entries step him away from that point, so 0.29 u off the wall in
    a cycle, and the next cycle's first steps put him back against it.
  - Each tick the detector must also find the ledge at his own position, or he doesn't
    move (UYA the same).
  - Its animation (sequence 73 left, 74 right) starts at frame 0 and runs 0.5 frames a
    tick from the first tick, with no hold at frame 0 as in UYA, and loops over 16
    frames. So a cycle is 32 ticks and uses all 16 entries, the last two a step back and
    nothing.
  - **A cycle covers 0.60 u along the wall**, 2 × 0.513 × the table's sum (0.588), about
    1.1 u/s against UYA's 1.3.
- **The climb (27)** is UYA's vertically: 2.05 u with X held, 25 u/s², leaving at 0.1618 u
  a frame and peaking 1.967 u above the hanging feet.
  - He hangs still for 3 frames, the push-off's 2 and one more, as UYA's does after its 4.
  - Its animation starts at keyframe 14, past the 13 where the forward speed starts, so
    the speed rises at 8 u/s² from the first frame and reaches 2 u/s on the 15th. The
    animation passed 27 on the 38th frame, where the speed eased off as he landed on the
    top on the 39th.

**Checked against our own DL NTSC traces** (`dlntsc-dreadzone-ledge-{1,2,3}`, scripted
keyboard play on DreadZone Station at the tutorial's ledge block, whose top is 2.22 u up;
`docs/re-setup.md`). These match the code:
- **The grab** started on the first frame the top was 0.5 u above the feet (0.504; 0.486
  the frame before), just past the apex. The lip was exactly the cast's hit, 1.05 u
  along his facing (yaw 1.343, not square to the wall), the hang point 0.5 u out from it,
  and he hung at the wall's face 0.49 u short of the hang point in y, where the owner's
  Gladiator savestate hangs too. He turned square to the wall in 12 frames.
- **Hanging** on the grab's 24th frame, at animation frame 13.
- **The shimmy:** steps of 0.0128, 0.0257, 0.0386, 0.0833, 0.0551 and 0.0190 u along the
  wall for the entries 0.025 to 0.037 (0.513 ×), 0.0255 and 0.0525 u back and out for the
  negative ones (1.5 ×); every cycle 32 frames; 0.6054 u a cycle left and 0.5998 right;
  0.29 u off the wall and back each cycle. With the stick released it finished the cycle
  and hung, then moved back to the wall at 4 u/s.
  - At the block's west end the wall no longer took its share, so the last cycle carried
    him 1.03 u and round the corner, where he hung with the stick still held.
- **Letting go:** L2 and X with the stick pulled back each dropped him into a fall with the
  10-frame timer; R1 did nothing.
- **The climb,** as above; the wall kept him from moving forward until its 8th frame.

**In the kit:** `dl.ron`'s `ledge` section (`probe_ahead`, `probe_step`, `probe_tries`,
`clear_back: 0` for no clear-lip check, `lip_walk: false`, `grab_rising_below`,
`double_jump_after_tick`, `grab_ticks: 23`, `no_grab_ticks_lost`, `shimmy_step_scale`,
`shimmy_blend_ticks: 0`, `shimmy_cycle_frames: 16`, the climb's forward ticks). Tested in
`rac_hero/tests/moves.rs` (`deadlocked_finds_ledges_further_out`,
`deadlocked_hangs_against_the_wall_short_of_its_hang_point`,
`deadlocked_shimmies_along_the_wall_at_about_uyas_pace`,
`deadlocked_climbs_forward_from_its_first_tick`). Both games' climbs now hang still through
the push-off and a frame more, and the kit's motor no longer drops a skin's width of every
slide along a wall, which had cut each of DL's shimmy steps by 0.0016 u.

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
  - Material 3 (quicksand, corrected 2026-10-13: see "Swimming") gives state 106, or
    the 125 sink when health is 0. In UYA's single-player code, 125 (libuya's "charge")
    sinks the hero at 1 u/s with the quicksand type.
  - Material 4 gives the slide (48).
- **Death (57):** the hero brakes like a landing while the death animation plays. The
  respawn timing is in the death code, which has not been read yet.

### Deadlocked's hits [code, DL NTSC, 2026-10-20]

Read in DL's entry for state 22 (0x69DAAC, through the enter table at 0x3B0BE0), its
motion (case 22 of `hero_update_state`) and its transitions (`hero_transitions`). As UYA's:
the knockback capped at 0.11667 u a frame (7 u/s), braking at 4 u/s², or 10 on the ground
from frame 11, and the return after 28 frames, or death with no health.
- **The fall:** more than 1 u over the ground past the animation's frame 16 (UYA 12.5).
- **Can't be hurt again** (hero+0x39C, which the damage code at 0x67916C tests): 47
  frames with DL's camera byte non-zero (lock-strafe or first person, the default) and
  70 in third person, while the global at 0x21E824 is 0 as in single player (0 there in
  the savestates); 47 otherwise. UYA's is 77.
- **Also on entry:** the pad rumbles (0x533730), and a 45-frame timer at hero+0x3D2 runs
  that `hero_back_moby_update` fades by.
- **Not read:** the hazard floor's damage, and death and the respawn.
- **In the kit:** `dl.ron`'s `hit` (`fall_after_tick` 16, `safe_ticks` 47 for the
  default camera). **[simplified]** The kit doesn't vary the timer with the camera.

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
- **Controls:** DL's default control scheme, its lock-strafe camera (the byte at 0x171DE0
  is 1; "Deadlocked's moves"), maps the charge's action to **L2**, and R1 fires. The
  third-person camera uses R1.
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

## Grinding (states 39–42, 62) [code, UYA NTSC, 2026-10-12]

Read in `hero_update_state`, `hero_transitions`, `hero_change_state` and the rail search
(`hero_find_rail`, 0x4E5118, and `rail_nearest`, 0x4E4E00).

- **UYA has nowhere to grind, so this stays code only [checked 2026-10-15].**
  - Every level's rails are in its gameplay file (`gameplay-format.md`, "Grind paths";
    `openrac-tool rails` lists them). Levels 2, 11, 17, 19 and the vid-comics 30 and
    32–35 have some; the multiplayer maps have none.
  - **The ones on Florana (2) and Crash Site (17) are zip lines** (next section), seen on
    screen. Florana's build of the hero code has no `hero_find_rail` at all.
  - **Levels 11, 17 and 19 have the grind code, but their hero class lacks the grind
    animations** (sequences 44, 45 and 72). With the grind boots set, a fall onto Crash
    Site's cable starts state 39. Then the game writes through a bad pointer, and PCSX2
    aborts on the next frame (twice with OpenGL, once with the software renderer).
  - Veldin's hero class has all 127 sequences, but Veldin has no rails. Sequence 45, the
    grind's second stance, is a stub there: one keyframe, no length.
  - **A rail written into Veldin's memory isn't enough either [tried 2026-10-15].**
    - **The setup:** Veldin's rail list is empty (count 0 at 0x1DFD14). We pointed it at
      a list of our own in spare memory: one 25 u rail 0.5 u over flat ground north of
      the start, in the gameplay file's layout, with the grind boots set.
    - **What happened:** a hero dropped onto it starts state 39, and PCSX2 aborts two
      frames later on a read of the VIF1 FIFO with nothing in it. Blanking the grind's
      spark and sound calls in the update (0x4F6CD8, 0x4F6E0C, 0x4F7CBC, 0x4F7CCC,
      0x4F7DC8, 0x4F6D24) didn't help.
    - **Not the cause:** the rail data, which is sane in the hero's grind block, and the
      first stance's animation (sequence 44, five keyframes).
    - **Still unknown:** what grinding uses that UYA's levels lack. 208 instructions read
      the state type, so we left it.
  - So a grind trace needs GC or DL. GC's rails are many but its disc is PAL and later
    (PLAN D3); DL's arenas can't be played from a new game yet (`docs/re-setup.md`).
- **Rails** are splines in a level list (0x1DFD10 in Veldin's build, count at 0x1DFD14),
  filled from the gameplay file when the level loads. Each entry is 0x20 bytes: a
  bounding sphere, the spline, and a flag word.
  - **The list's address changes with the level's build:** on Florana it is 0x1DE690
    (found 2026-10-15 by searching memory for the rail's points). The hero struct stays at
    0x1A4BC0 in every build seen so far, but other globals move.
  - **A rail runs one way, along its spline.** The grind always moves in the spline's
    direction.
- **Getting on [code, `hero_find_rail`, every frame]:** needs the grind boots (a global
  flag) and the 8-frame no-rail timer run out. Then:
  - **The hero must be** standing, walking or falling, or in a jump past its apex.
  - **The nearest rail point must be:**
    - within 0.9 u horizontally (0.3 u when standing or walking);
    - within 1.5 u vertically;
    - above something solid (a ray from 0.25 u above to 1 u below it).
  - **The feet must be** at most 0.5 u below that point and less than 0.57 u above it.
  - Any jump past its apex lands on a rail this way, the grind jump included.
- **Speed:** each frame the speed is a base plus a slope term, never below 0. The hero is
  moved along the rail by that distance.
  - **The base** eases towards **12 u/s at 15 u/s²**.
  - **Getting on, the base is** the hero's velocity along the rail, which can be
    negative. Coming from a grind jump, it keeps its value.
  - **The slope term** depends on the rail's pitch θ over the next 0.3 u:
    - its target is `−θ × 15/π` u/s uphill, so 2.5 u/s less at 30°;
    - downhill it is `|θ| × 72/π` u/s, so 12 u/s more at 30°;
    - it rises at 8 u/s² and falls at 2 u/s², so speed from a descent lingers;
    - it is clamped to −2.5..12 u/s.
- **Facing: he rides side on.** The shared update for states 39–42 turns him to the
  rail's direction plus a stance offset (0x1A51D8), with gain 0.015, damping 0.15 and a
  cap of 0.1047 rad per frame.
  - Getting on sets the offset to −90°, so he faces to the rail's right with his left
    side leading, playing sequence 44.
  - It is +90° with sequence 45 instead when an object the hero struct points to
    (0x1A5228) lies more than 90° the other way. What that object is wasn't read.
  - The stick only leans him (up to 45°, for the animation). Until 2026-10-15 this note
    said he faced the rail's direction; the offset had been missed.
- **Ways off:**
  - **X pressed within 7 frames:** the grind jump (40). It is 2.4–2.45 u, hold 17,
    push-off 5, gravity 27.
  - **Square within 10 frames:** the grind attack (42), a wrench swing on the rail ("The
    grind attack", under "Melee").
  - **The rail's end:** 6 frames after the spline runs out, the hero falls (state 6). He
    keeps his velocity, and rails are ignored for 8 frames.
  - The hero also falls when he is more than 1.07 u above the ground under the rail for
    6 frames, once 30 frames into the grind. That is a rail with nothing under it.
- **Jumps from a rail (40, 41) follow it:**
  - The hero's horizontal position keeps tracking the rail, moving along at the speed
    he left with. Only the height jumps.
  - The rail's own rise is added to the height.
- **Switching rails:** in the grind jump's first 10 frames, a stick past 0.5 that points
  20°–160° away from the rail's direction looks for another rail to that side.
  - **The search:** probe points are 48 frames of the current velocity ahead, then 2.0,
    2.75, 3.5 or 4.25 u to the side, each checked with the getting-on search.
  - **The first rail found** starts the switch jump (41): 2.5–2.55 u, hold 10, push-off
    2, gravity 24.
  - **Moving across:** the horizontal position blends from the old rail to the nearest
    point on the new one over 48 frames, with both moving along.
- **Not modelled yet:** rails that jump the hero over a gap automatically (flags at
  0x1A520C/0x1A5210), and the grind hit (62).
- **In the hero kit (P1.5):** rails are one-way polylines (`GrindRail`), and the
  getting-on rule, speed model, side-on stance, grind jump, switch jump, rail end and grind
  attack are as above. The pull onto the rail uses the game's spring (gain 0.03, damping
  0.3, up to 7 u/s).
  **[simplified]:**
  - nothing collides while grinding;
  - the hero falls as soon as the rail ends, not 6 frames later;
  - no solid ground is needed under a rail;
  - the fall from a rail with nothing under it isn't there;
  - the stance is always −90°; the object that turns it the other way isn't modelled.

## Zip lines (state 118) [code, then verified on UYA NTSC, 2026-10-15]

Read in `hero_find_zipline` (0x4E5300 in Veldin's build, 0x4BA800 in Florana's),
`hero_update_state`, `hero_change_state` and `hero_transitions`. Verified on Florana's
first cable with two traces (`uyantsc-florana-zip-1`, `-2`; 376 and 374 frames of the ride)
and one at Crash Site (`uyantsc-crash-zip-1`; `docs/re-setup.md`).

- **The cables are the level's rails.** The zip line searches the same list as the grind
  (`rail_nearest`), so a cable is a rail the hero hangs from rather than stands on.
  libuya calls state 118 `DEATH_FALL`; in UYA's single player it is the zip line.
- **Catching a cable [verified, Florana]:** every frame, with the 30-frame no-catch timer
  (0x1A4DE4) run out, while jumping (state type 4, unless a flag at 0x1A504E is set),
  falling (type 2) or in type 15:
  - **the hands** are a point 1.34 u above the feet (and 0.3 u forward, from the
    arguments; not checked);
  - the nearest rail point to the hands must pass `rail_nearest`'s test (0.9 u
    horizontally, 1.5 u vertically, solid ground under it);
  - and **the hands must be from 0.7 u below to 0.4 u above it**, or 1.2 u below with a
    flag whose meaning isn't known (bit 7 of the word at +0x1A4 of what 0x1D52FC points
    to).
  - **In a build with both searches, the grind wins:** `hero_transitions` takes the grind
    (39) when `hero_find_rail` found a rail, and only otherwise the zip line.
  - Florana trace: falling at 0.27 u per frame, he caught the cable with his hands 0.31 u
    above it.
  - **From above it doesn't work [verified, Crash Site]:** the cable has collision on
    top. Dropped 1 u above it, he landed on it with his feet 0.14 u over the spline
    (state 123), slid off sideways and fell.
- **Speed [verified]:**
  - **On catching:** half his velocity along the cable (measured 1 u ahead), never below
    0. Falling straight down onto it gave 0.
  - **Then it rises by 0.0025 u per frame each frame (9 u/s²) to 0.23333 u per frame
    (14 u/s).** The trace reached the cap 94 frames after the catch and held it to the
    end.
  - Slope doesn't change it: Florana's cable falls 15 u over its 83 u.
- **Hanging [verified]:** the hands follow the cable's spline at that speed. A spring
  pulls them onto it (their distance, 0x1A5298, settled at about 0.04 u). The hero faces
  the cable's direction.
- **Input does nothing [verified, trace 2]:** X, Square and the stick left and right
  changed neither the speed nor the path. The stick turned him by 0.004 rad at most.
- **The cable's end [verified]:** at its last point he keeps his last step along it, and
  gravity of 0.0061 u per frame² (22 u/s²) starts to act. A counter (0x1A5280) runs from
  1, and at 6 he falls (state 6). Then the no-catch timer starts at 30 frames.
- **Fields:** the found point at 0x1A5250, the last step at 0x1A5260, the spline, segment
  and position along it at 0x1A5270–0x1A5278, the found flag at 0x1A527C, the end counter
  at 0x1A5280, the speed at 0x1A5284, the rail's flag word at 0x1A528C, heading and slope
  at 0x1A5290 and 0x1A5294 (layout `uyantsc-florana-zip.toml`).
- **Animation [code]:** `hero_change_state` starts sequence 90 for state 118, blending
  over 8 frames. Levels with zip lines keep it (11, 17 and 19 among them).
- **In the hero kit (P1.5, 2026-10-15):** a rail of kind `RailKind::ZipLine`
  (`GrindRail::zip_line`). The catch, the speed, the turn, the end and the no-catch timer
  are as above, and the catch works from any jump, a fall or a grind. **[simplified]:**
  - the hands are 1.34 u straight above the feet, and their 0.3 u forward offset is left
    out (it didn't fit the trace; the hero's rotation may tilt it);
  - riding, the feet hang 1.63 u under the cable, as measured, and get there from the
    catch on the game's hang spring. In the game the spring pulls the hands towards the
    cable and they settle about 0.25 u under it; the kit's settles a little faster than
    the trace (1.45 u under after 8 ticks against 1.35);
  - the kit's cables have no collision, so falling onto one from above, the hands catch
    it on the way past. In the game the cable's own collision stopped him on top;
  - no ground is needed under the cable, and nothing collides during the ride or the six
    ticks past its end.

## Swimming (states 50–54, 18) [code, then verified on UYA NTSC, 2026-10-13]

Read in `hero_update_state`, `hero_transitions`, `hero_change_state`, the water check
(`hero_enter_water`, 0x5010B8) and the surface float (`hero_surface_float`, 0x501458).

**Checked against our own UYA NTSC traces** (`uyantsc-sewers-swim-{1..4}`, scripted
keyboard play in the Aquatos sewers, level 28; `docs/re-setup.md`). The sewers' water
flows, which moves the hero (below), so the swim states were checked through what they
ask for, the intended velocity (hero+0x110), and through the float's own variables. These
match the code:
- **The float:** the feet rest 0.12 u under the surface, and the bob on getting in and
  its damped oscillation hold on all 390 surface-idle frames, to the PS2's rounding.
- **Getting in** from a water jump: the test uses the feet after that frame's move, and
  so does the bob's starting offset. Its rate starts at 0.37 × that frame's vertical
  velocity.
- **The surface swim (53),** on all 167 frames to 3e-8: the turn spring, the 3 u/s
  target that is 0 while the stick points more than 45° away, the 4 u/s² rise, and the
  velocity, speed × 5.5 × the stroke, along the facing.
- **The water jump:** 1.95–1.98 u, hold 10, push-off 5, gravity 29.7 u/s². The push-off
  presses him down 0.0134 u a frame, 0.067 u in all.
- **Under water (50, 52):** 87 frames exact and 240 more after the wall slide (below).
  - the dive's 40 frames and its 6 u/s target (in 52 too);
  - 3 u/s in 50 and 7 u/s in 52 with R1 held;
  - rises of 7 and 13 u/s² and the 4 u/s² fall;
  - the pitch's cap, 0.01164 rad per frame with no button and 0.02036 at full press;
  - the stroke's rate, 15 × the speed clamped to 0.25–0.75;
  - the velocity, speed × 5 × the stroke in 50 and the speed itself in 52, along the yaw
    and the pitch.
- **Underwater idle (51)** after a fast swim keeps the velocity's direction and shortens
  it by 8 u/s² exactly.
- **Transitions seen:** 54 → 50 on Square, 54 → 52 on Square with R1 held, 50 → 52 on R1,
  52 → 51 on letting go, surfacing into 54, and 53 → wading (117) in shallow water.
- A state change takes effect after that frame's update, so the first frame of a new
  state still moves as the old one did.

**Found in the traces:**
- **Under water the velocity slides along walls.** After the swim sets its velocity,
  `hero_wall_slide` (0x4EB5D0) probes 0.7 u along it. If it finds a surface steeper than
  50° (or of material 8 or 12), it removes the velocity's part into that surface. The
  surface swim doesn't call it.
- **Water can flow.** In the sewers' channels the water carried the hero at up to 8 u/s
  (0.1334 u per frame), speeding up at about 7 u/s² and turning with the channel. Where
  the channel turned it also lifted him, up to 0.07 u a frame, and it lifted him back to
  the surface within about two seconds of a dive.
  - None of it is in the swim states' intended velocity, only in the motion, so something
    outside the hero's state code applies it. Not read yet.
- **Diving is switched off by a timer,** `hero_no_dive_timer` (0x1A4DDE, an i16), which
  must be 0. The hero's timers update counts it down.
  - In Veldin's build the timers update holds it at 10 on Florana (2) and Marcadia (4).
  - In the sewers a level object (its update is at 0x323B68 in that overlay) sets it to
    2 every frame while the hero is in its volume and outside two exception volumes. In
    a new game's sewers that covers all the water.
  - For the dive traces the object's store was patched in the research instance to write
    0 (`uyantsc-sewers-dive.p2s`).
- **Diving lifts the origin.** On the dive's first frame the hero's position rose
  0.156 u at once, every time, and stayed there. Diving also calls a function with 3 that
  isn't read, probably a collision-shape change.

- **The states** (libuya's names):
  - on the surface, type 18: 53 swim (`SWIMSURF`) and 54 idle (`IDLESURF`);
  - under water, type 17: 50 swim (`SWIMUNDER`), 51 idle (`IDLEUNDER`) and 52 fast swim
    (`CHARGEUNDER`).
  - Their working values are in libuya's `HeroSwim` block at 0x1A5570, whose field names
    match the code: the dive timer at +0x10, the pitch rate at +0x18, the button hold
    counts at +0x30. The surface float's values are at 0x1A5300.
- **Water** comes from the ground probe (`hero_ground_probe`). Its first cast, from 2.2 u
  above the feet to 48 u below, can hit a water polygon. The code then:
  - takes the water's height from that polygon's plane under the hero (0x1A4F30, libuya's
    `waterHeight`);
  - casts again past the water for the floor;
  - sets the depth, surface minus floor, at 0x1A7428.
  - Collision material 14 is a puddle: the surface is 0.2 u above the floor.
  - **Shallow water** (0.25–0.85 u deep) sets a wading flag, and walking in it is state
    117. That state isn't read yet.
  - Correction to "Getting hit and dying" below: material 3 is quicksand. The probe keeps
    its height apart from water's, at 0x1A4F34.
- **Getting in [code, `hero_enter_water`, every frame]:** only in water, and not from a
  ledge or the surface states. The hero goes to surface idle (54), or to state 108
  (probably drowning) with no health, when either:
  - he is falling, the water is more than 0.8 u deep, and his feet are 0.45 u below the
    surface, give or take max(0.2, 0.07 + his intended vertical speed per frame) u;
  - or his feet are more than 0.8 u under.
  - A jump only counts once past its apex. The water jump (18) also comes back in with
    its feet 0.7 u under.
  - **Every 16 frames,** a probe also looks for water above a hero who isn't swimming. If
    it finds water with no surface over him, he starts under water (51). That probe isn't
    fully read.
- **Floating on the surface (53, 54):**
  - **Rest height:** the feet float 0.12 u below the surface (Clank 0.05). A tracked
    surface height follows the water's with the game's spring (gain 0.027, damping 0.3,
    no cap), and an offset bobs around it.
  - **Bob on getting in:**
    - the offset starts at the feet's height above the rest height;
    - its rate starts at 0.37 × the vertical velocity, at most 7 u/s downwards.
  - **When idle (54):** the offset is a damped oscillator. Each frame its rate loses
    0.005 × offset + 0.045 × rate, so it bobs with a period of about 1.5 s.
    - While the state's timer is 10 or less it moves freely, unless it is below the rest
      height or the hero came up from under water. The timer is counted after the
      update, so that is the first 11 updates [verified].
  - **When swimming (53):** the offset springs to 0 (gain 0.03, damping 0.3, cap
    0.025 u per frame). Above the rest height it also sinks by 0.7 u/s.
  - **Drift:** getting in carries the horizontal velocity on, capped at 5.5 u/s. It slows
    at 4.2 u/s² and adds to both surface states' motion.
- **Surface swim (53):**
  - **Direction:** the stick sets it, relative to the camera. The facing turns with the
    game's spring: gain 0.007, damping 0.08, cap 0.0873 rad per frame.
  - **Target speed** is 3 u/s × the stick, but:
    - at least 1.5 u/s while the stick is held;
    - 3 u/s whatever the stick for the state's first 45 frames, so a stroke finishes;
    - 0 while the stick points more than 45° from the facing.
  - **Speed** rises at 4 u/s² and falls at 5 u/s².
  - **Velocity:** speed × 5.5 × the stroke (below), along the facing and flat.
- **Surface transitions:**
  - **X pressed within 8 frames:** the water jump (18): 1.95–1.98 u, hold 10, push-off 5,
    gravity 29.7. From idle, only after 10 frames.
  - **Square pressed within 7 frames:** a dive. From idle, R1 or R2 also dives. It goes
    to 52 if R1 or R2 is held, else to 50.
    - Diving needs `hero_no_dive_timer` at 0, which levels hold above 0 to forbid it
      ("Found in the traces" above). The hero's class must also have a pointer at
      +0x120 set, probably the dive animation.
  - **53 → 54:** stick under 0.1, speed 0, and the 45-frame stroke over.
  - **54 → 53:** stick past 0.2, once 10 frames in.
  - **Standing up:** 53 in water under 0.7 u deep goes to wading (117). 54 in water under
    0.4 u deep goes to idle (0).
- **Under water: steering.**
  - **Yaw:** the stick sets the direction, relative to the camera, with the same spring as
    the surface swim.
  - **Pitch** is positive nose down. Its target:
    - Square held without Cross: +1.3614 rad (78°) × clamp(1.5 × Square's pressure, 0.2, 1);
    - Cross held: −1.3614 rad × clamp(1.5 × Cross's pressure, 0.2, 1);
    - neither, or in idle (51): 0.
    - **Pads without pressure buttons:** the factor ramps from 0.5 to 1 over 20 frames
      held.
    - **While diving:** at least 0.35.
  - **The pitch spring:** gain 0.015, damping 0.2. Its cap is 0.01164 + 0.00873 × the
    larger pressure, in rad per frame: 0.70 rad/s with no button and 1.22 rad/s at full
    press.
  - **The pressures** are in the pad's float array (pad+0x100): Cross at +0x128, Square
    at +0x12C. The pad's terminal id (+0x1DC) is 0x79 for a DualShock 2 in pressure mode.
  - **Bank (for the animation):** roll targets −7 × yaw rate × cos(pitch), with gain
    0.007, damping 0.17 and cap 0.0204.
- **Under water: speed.**
  - **Target speed:**
    - 3 u/s in 50 and 51, or 6 u/s in the dive;
    - in 52: 7 u/s with R1 or R2 held, otherwise 2.8 u/s.
  - **Scaled by the stick magnitude,** except in 52 with R1 or R2 held:
    - with Cross or Square held, or in the dive, the scale is raised to 1;
    - with the stick past 0.15, it is at least 0.35 (0.4 in 52).
  - **Speed** rises at 7 u/s² and falls at 4 u/s². In 52 it rises at 13 and falls at 8.
- **Under water: velocity,** along the yaw and pitch:
  - **50:** speed × 5 × the stroke. For 15 frames after a fast swim it is at least
    4.5 u/s.
  - **52:** the speed itself.
  - **51:** the velocity it was entered with, slowing at 4 u/s² (8 after a fast swim) in
    the same direction.
  - Nothing pulls him up or down: no buoyancy, no gravity and no air limit in these
    states.
- **The dive:** entering 50 or 52 from the surface starts a 40-frame dive (substate 1).
  During it:
  - the target speed is 6 u/s;
  - the pitch is at least 0.35 × 78° down;
  - he can't surface.
- **Surfacing:** an underwater state goes to surface idle (54) when he is rising with his
  feet less than 0.4 u under the surface, unless he is in the dive.
- **Under water: transitions.**
  - **50 → 52:** R1 or R2 held, once 20 frames in.
  - **52 → 50:** R1 and R2 released, with the stick past 0.5.
  - **50 → 51:** all of:
    - no Cross or Square, and not in the dive;
    - stick under 0.1 and speed under 2 u/s;
    - the stroke animation between its frames 9 and 19.
  - **52 → 51:** no Cross, Square, R1 or R2, and the stick under 0.7.
  - **51 → 50 or 52:** stick past 0.2, or Cross or Square held, or R1 or R2 held (which
    goes to 52).
- **The stroke** makes swimming speed pulse with the arms.
  - The animation is a 30-frame cycle:
    - its rate is 0.6 on the surface;
    - under water it is 15 × speed (u per frame), clamped to 0.25–0.75.
  - It advances 0.5 × rate frames a tick. The 0.5 is a global read as 0.5 in the Veldin
    image, so it may differ per animation.
  - **The stroke factor** comes from a table (0x2F6630 on Veldin, 0x2F37B0 in the
    sewers) [verified]. With the animation at frame f after a step s, and k the whole
    frame of f, it is `frac(f) × table[k] + (s − frac(f)) × table[k − 1]`.
    - The formula assumes each tick reaches a new frame. With steps under a frame the
      second term turns negative, so the push saws up and down within each frame.
    - Over a cycle it still averages s × the table's mean.
  - **The table:**
    - 0.4 on frames 0–1;
    - rising to 1.0 on frames 8–11;
    - falling back to 0.4 by frame 23.
  - **So on the surface** the velocity averages about 1.06 × the speed, between about
    0.55 and 1.9 × through the stroke. Under water at full rate it averages about 1.2 ×
    the speed.
- **Not read yet:** wading (117), drowning (108), getting hit in the water, the
  every-16-frames underwater probe, flowing water, and the lift on diving.
- **In the hero kit (P1.5, 2026-10-13):** water is a box whose top is the surface
  (`Water`, `WaterVolume`), found under or up to 2.2 u below the feet. The five states,
  the water jump, the dive, the surface float and bob, the stroke and the numbers are as
  above, in `uya.ron`'s `swim` section.
  - **Buttons:** Cross is jump, Square is melee, and R1 or R2 is crouch or strafe (so L2
    swims fast too, as the kit has one strafe action).
  - **[simplified]:**
    - Buttons always count as fully pressed. So the pitch heads straight for ±78° and
      turns at up to 1.22 rad/s, as on a DualShock 2 pressed hard.
    - Swimming into shallow water lands and walks instead of wading.
    - The level-out of the pitch on the surface is the kit's own. The game's surface
      code doesn't touch the pitch.
    - The water jump keeps the velocity it swam with, without the game's flattening in
      water deeper than 1.25 u, which is a no-op for a flat swim anyway.
    - The drift is taken from the velocity. The game copies a vector at hero+0x140,
      taken here to be the velocity.
    - Diving is allowed everywhere. A level would forbid it with the game's no-dive
      timer.
    - The kit's capsule slides along walls under water, which has the effect of the
      game's `hero_wall_slide`.
    - Water doesn't flow.
  - **The water jump's apex** is about 1.55 u above the floating feet in the kit: the
    target height isn't the apex (see "Jump heights"), and the push-off presses him
    0.07 u into the water first.

### Deadlocked's swimming [code, DL NTSC, 2026-10-20; untraced in DL]

Read in DL's water entry (0x6A60A8 and 0x6A64D0, which `hero_transitions` runs first in
single player), the surface motion (states 53 and 54 in `hero_update_state`), the
underwater motion (0x699060, which DL's update calls for 50–52) and the surface
transitions, side by side with UYA's.
- **The same, with DL's per-second literals:** getting in (feet 0.45 u under, give or take
  max(0.2, 0.07 + the fall), in water over 0.8 u deep; or 0.8 u under; a water jump back
  in at 0.7 u), surfacing (rising, feet within 0.4 u, not in the dive), popping out above
  0.4 u into a fall, the every-16-frames probe for water above (4 u, or 16 every 63rd
  frame), the surface swim (3 u/s with a 1.5 u/s floor for digital sticks, 4 and
  5 u/s², 5.5 × the stroke, the 45° stop, the 4.2 u/s² drift brake, the 0.007, 0.08,
  300°/s turn), the water jump on X within 8 frames (from idle after 10), 53 ↔ 54, and
  standing up in shallow water. Under water: the pitch spring (0.015, 0.2, 0.698 +
  0.524 × the pressure rad/s, ±78°), the speeds (3, 6 in the dive, 7 or 7 × 0.4), the 7/4
  and 13/8 u/s² rates, the stroke with its 4.5 u/s floor after a fast swim, the bank
  spring and the joint numbers.
- **No dive:** DL's surface states lack UYA's Square (or idle R1/R2) dive, and no code
  checks Square within 7 frames. `hero_transitions` has no cases for 50–52 either, so
  under water, reached only through the every-16-frames probe, the hero has no way to
  swim on or surface but rising.
- **Entering with no health** goes to state 106 (UYA 108), with a 70-frame timer.
- **Not read in DL:** the surface float and bob (UYA 0x501458), wading (117) and
  drowning.
- **In the kit:** `dl.ron`'s `swim` keeps UYA's numbers, which DL's code matches, with
  `dives: false`. Tested in `rac_hero/tests/moves.rs`
  (`deadlocked_swims_on_the_surface_without_diving`).

## Animations per state [code and trace, UYA NTSC, 2026-10-13]

Which of the hero's sequences (`moby-format.md`) each state plays, read in
`hero_change_state` (0x4FC518), which starts each state's animation with `hero_anim_set`
(blend ticks, sequence, start keyframe), and checked against our trace
`uyantsc-veldin-anim-1` (scripted play on Veldin with the wrench in hand, logging the hero's
moby's sequence every frame). **[verified]** marks sequences the trace showed.

| State | Sequence | Notes |
|---|---|---|
| Idle (0) | 0 **[verified]** | The idle fidgets 1 and 2 play now and then **[verified]**, and 73 ("Idle fidgets"). At 20% health or less, 74, with 78 now and then; holding a weapon, that weapon's idle from `weapon_table` (row + 0x24). |
| Walk (2) | 3 + substate: 3 setting off, 4 moving **[verified]** | Strafing backwards, 20 [code]. Bands and rates under "The walk's animation". |
| Skid (3) | 6, or 5 straight out of a run between frames 3 and 16 | |
| Crouch (4) | 13 **[verified]**; 14 moving **[verified]** | Starts at keyframe 2. |
| Fall (6) | 10, then 11 **[verified]**, then 12 on landing **[verified]** | "Landings". The game also plays 11 in state 121, falling out of the level **[verified]**. |
| Jump (7) | 7 **[verified]** | |
| Glide (8) | 19 **[verified]** | |
| Run jump (9) | 8 or 9 **[verified 9]** | Out of a run, a per-frame table picks by the run cycle's frame (which foot). |
| Long jump (10) | 18 | |
| Flip (11) | 28 + direction: 28 left **[verified]**, 29 right **[verified]**, 30, 31 back **[verified]** | |
| Double jump (14) | 22 **[verified]** | At rate 0.8, from keyframe 3. |
| Heli jump (15) | 21 **[verified]** | |
| Wall jump (17) | 35 | |
| Water jump (18) | 89 | |
| Combo (19) | 23, 24, 25 for swings 1–3 **[verified]** | Their keyframes' time stamps are the animation frames the attack windows use: swing 1's run 16–35, starting at its start frame 16. |
| Jump attack (20) | 40 **[verified]** | |
| Throw (21) | 26 **[verified]** | Frames 30–118, 95 ticks as traced. |
| Hit (22) | 16 | |
| Ledge grab (23) | 32 | |
| Ledge hang (24) | 33 | |
| Shimmy (25, 26) | 36 left, 37 right | |
| Ledge jump (27) | 34 | |
| Grind (39) | 44 + a stance value | |
| Grind jump (40) | 72 | |
| Grind switch (41) | 30 or 31 by side | |
| Zip line (118) | 90 | From `hero_change_state` [code], 2026-10-15. |
| Underwater swim, idle, fast (50–52) | 54, 55, 56 | |
| Surface swim (53), idle (54) | 82, 53 | |
| Dead (57) | 64 | |

- The hero's skeleton and every sequence are on the disc in each level's core
  (`disc-layout.md`, "Level data"). Every level has the skeleton but only the sequences
  it uses.

### Idle fidgets [code, UYA NTSC, 2026-10-15]

Read in the idle state's transitions (0x503B68) and the fidget picker 0x502930 on Veldin.
- **The table** at 0x2F4370 has four fidgets of 0x70 bytes: +0x44 0 to play a sequence
  (else enter state 60), +0x48 the sequence, +0x50 the hand item it needs (−1 any), +0x58
  the mean seconds between plays, +0x5C the cooldown in seconds, +0x60 the cooldown
  counter in frames:

  | Fidget | Sequence | Every | Cooldown | Only |
  |---|---|---|---|---|
  | 0 | 1 | 10 s | 5.5 s | |
  | 1 | 2 | 10 s | 5.5 s | |
  | 2 | 73 | 8 s | 5 s | nothing within 8 u above him (a ray up), and a global at 0x1D545C of −1 |
  | 3 | 78 | 4 s | 2.5 s | at low health (below) |

- **Each frame** standing in the plain idle animation, when no fidget has played for 120
  frames (0x1A4DE8): each fidget whose cooldown is over has a chance of 1 in (its mean ×
  60) of starting, chosen by weight. Starting one sets its cooldown and the 120-frame gap,
  and plays it with a 12-tick blend; the cooldowns and the gap count down every frame
  (0x4FAA68). When it ends the idle comes back (blend −2: 16 ticks from the table at
  0x1DBE90).
- **At 20% health or less** (0x4DC080) the idle is 74, and at each end of its loop fidget
  3 plays one time in three if its cooldown is over.
- In the kit: `rac_rig/src/play.rs`. The rig has no collision, so it plays fidget 2 as if
  the sky were clear.

### Landings [code, UYA NTSC, 2026-10-15]

- **Jumps** time their animation to land on a frame of their own: `hero_change_state`
  sets the peak frame (0x1A5078), the landing frame (0x1A507C) and a landing keyframe
  (0x1A5080) per state, and the air rate is warped to meet them. On landing, short of the
  landing frame, the jump's own sequence restarts from the landing keyframe + 2 (blend 7),
  and the jump state plays it out until the stick moves him off or it ends:

  | State | Sequence | Peak, landing frame | Landing keyframe |
  |---|---|---|---|
  | Jump (7) | 7 | 18, 30 | 12 |
  | Run jump (9) | 8, 9 | 18, 29 | 11 |
  | Long jump (10) | 18 | 18, 29 | 10; lands into a skid playing 5 |
  | Flip (11) | 28–31 | 18, 31 | 24 |
  | Double jump (14) | 22 | 32, 43 | 19 |
  | Heli jump (15) | 21 | 50, 64 | 30 |
  | Wall jump (17) | 35 | 12, 20 | 10 |
  | Water jump (18) | 89 | 18, 28 | 14 |
  | Ledge jump (27) | 34 | 20, 29 | 19 |

- **Falls** (state 6) start with 10, a one-keyframe step-off pose, and change to 11 after
  17 frames or at once from more than 1.75 u up. Landing from 11 goes to idle playing 12:
  from keyframe 9 (blend −1) after under 90 frames of falling, from keyframe 4 (blend 6)
  after more. A fall still in 10 lands without one. The landing also sets a lockout
  (0x1A4DC0: 7, 10 after state 44, 18 after a fall over 50 frames, 22 after a long one),
  which the kit's controller doesn't have yet.
- In the kit the controller lands into the walk, so the rig plays a landing while he stays
  at the speed he landed with, at most 2.5 u/s (a landing with the stick let go), and
  stops it when he speeds up. It doesn't warp the air animation yet.

### The walk's animation [code, UYA NTSC, 2026-10-15]

`hero_walk_anim` (0x5024B8), with the bands at 0x1DBE40 (16 bytes a substate):
- The walk always starts setting off (substate 0, sequence 3) at keyframe 15, blended
  over 8 ticks (12 at low health, 18 after a fall). After 4 frames it runs (substate 1,
  sequence 4, from keyframe 16 when entered directly) above 2.35 u/s, and sets off again
  below 1.9 u/s.
- Changing keeps the stride: the new keyframe is the old one × the new sequence's
  keyframe count / the old one's, plus 1 into the run or 5 into setting off (0x1DBE60).
- Rates, from his speed in u per frame: setting off × 157 (0.6 to 4), running × 13 (0.6
  to 2.2), walking backwards × 25 (0.7 to 2.2). The trace's "1.25 at 6.2 u/s" was the
  run; the code gives 1.34.
- A skid out of a run whose cycle is between frames 3 and 16 plays 5, otherwise 6.

## Moves that need items [code, UYA NTSC]

The kit gates these with per-hero ability flags (PLAN.md section 5, 2026-10-12).
- **The crouch jumps** (`hero_crouch_jump`) check the back slot, which is item slot 3:
  - with Clank's heli-pack (item 2, and a global flag set): the long jump (10) or heli
    jump (15);
  - with R&C1's thruster-pack (item 3): the thruster dash (16) or rocket jump (13);
  - **with neither: a standing jump (7).**
  - The long jump from a jump's first frames (crouch pressed within 2 frames) needs the
    same items.
- **The glide** needs the same global flag, and no item 4 in the back slot. Deadlocked
  never glides: its own flag is set for good ("Deadlocked's moves").
- **Grinding** needs the grind boots: `hero_find_rail` checks a global flag
  (0x142CAE) before looking for a rail. **Zip lines need no item.**
- **The charge boots** are DL's gadget 0x11, above. The jumps, double jump, wall jump,
  ledges and flips need no item.

## Differences between the games

Read side by side in UYA's code and DL's (DreadZone Station) on 2026-10-15, and checked
against our own DL NTSC traces (below). DL's hero code is UYA's engine: the same
functions in the same order, with the literals scaled by `frame_dt` at run time.

| | R&C3 / UYA | Gladiator / Deadlocked |
|---|---|---|
| How per-second values become per-frame | Folded into literals when compiled | Multiplied by `frame_dt` globals at run time |
| Hero in code | Static global, addressed directly (code base 0x1A4BE0) | Pointer argument (libdl's `Player`) |
| Walk speed (a global: UYA 0x1DBE58, DL 0x3B01A0) | 6.17 u/s | 7.25 u/s **[verified on DL NTSC]** |
| The jump block's default air speed: standing, run, ledge and water jumps | 6.17 u/s | 7.25 u/s **[verified on DL NTSC for the run jump]** |
| The fall's air speed: the walk's speed target | 6.17 u/s | 7.25 u/s [code] |
| Double-jump cap | 8.75 u/s | 7 u/s **[verified on DL NTSC]** |
| Double jump from a standing or run jump | from t > 14 | from t > 10 **[verified on DL NTSC]** |
| Glide (state 8): forward speed, sink | 3 u/s (5 in one gadget mode), 2.16 u/s | 5 u/s, 3.6 u/s (5 × 0.72) [code] |
| Ledge jump push-off | 4 frames NTSC (he leaves on the 6th), 3 on R&C3 PAL | 2 frames on DL NTSC (he leaves on the 4th) **[verified on DL NTSC]** and on Gladiator PAL |
| Ledge lip | walked out over the top to the wall's edge | where the top's cast hit, 1.05 u ahead of the feet: the walk's probe has no length **[code; verified on DL NTSC]** |
| Ledge grab, then hanging | from the grab's 14th frame | from its 24th: its animation starts at keyframe 1, not 11 **[verified on DL NTSC]** |
| Shimmy | 0.5 × the table sideways; 28 frames and 0.623 u a cycle | 1.5 × the table towards a point the wall half blocks; 32 frames and 0.60 u a cycle **[verified on DL NTSC]** |
| A jump from a strafing walk | flips to the side or back | always a standing jump **[verified on DL NTSC]** |
| Flips | from a walk, as a jump lands, and from a crouch | only as a jump lands **[verified on DL NTSC]** |
| Crouch (state 4), and with it the crouch flip, the long jump and the heli jump | R1 | none: nothing enters states 4, 10 or 15 **[code; R1 traced]** |
| What bars the glide | no heli-pack in the back slot | hero+0x2E2C, set when the hero is created and never cleared **[code; traced]** |
| `hero_strafe_side` for back; flip animation | 3; sequence 28 + side | −1; sequence 17 + side [code; −1 traced] |
| The landing flip: off the ground fewer than | 8 frames | 4 frames [code] |
| Wall jump (state 17) | from standing and run jumps, glides and wall jumps | none: nothing enters it, and no wall detector runs **[code; checked on DL NTSC at a tall wall]** |
| Zip lines (UYA's state 118) | yes, on the level's rails | none; 118 is the death fall [code] |
| Diving | Square from the surface | none: DL's surface states don't dive [code] |
| Default camera | follow (third person) | lock-strafe |
| Standing jump animation frames (peak, land, game land) | 18, 30, 12 | 18, 27, 19 |
| Charge boots | no charge state in Veldin's code | state 125, read in DL's code |
| Everything else read so far | identical | identical |

- **Read the same in DL's code** (2026-10-15; addresses in `re/SCUS-97465/symbols.toml`):
  - the walk (`hero_update_walk`): 37 u/s² up and 30 down, the skid's limits and its
    17 u/s² braking, the part-pushed stick's rules, the ground push of 54 u/s²;
  - the walk's turning springs (`hero_walk_turn`), setting off, moving and skidding;
  - every jump-type state's heights, hold and push-off frames and gravity
    (`hero_change_state`), apart from the ledge jump's push-off;
  - the vertical model and the double jump's boost, its height factors and its press
    window (`hero_jump_vertical`, `hero_transitions`);
  - the heli jump's thrust: 49 u/s² from frame 35 to 70 (the table at 0x2207D0);
  - air control (`hero_air_horizontal`): 20 u/s², the release braking at 8, the push-off
    rates, the wall jump's turn and its 4.9 u/s, the ledge jump's forward speed;
  - the fall: 22 u/s², braking at 7 with the stick released, capped at 50 u/s;
  - the glide's springs, and the heights and frames past which a double jump (1.5 u,
    t > 30) or a heli jump (0.6 u) turns into it. DL's check also has 1.0 u for standing
    and run jumps. All of it is unreachable: DL never glides ("Deadlocked's moves").
- **The stick in DL [verified on DL NTSC, 2026-10-17, `dlntsc-dreadzone-camhalf-0.5-{1,2}`]:**
  the pad layer's dead zone is UYA's (the bytes 191 and 64 gave 0.2105 and 0.1974), and
  a left stick held at 0.1974 left the hero standing in both camera settings, as UYA's
  0.25 cut-off would; the cut-off itself isn't read in DL's code.
- **Not read in DL yet:** lock-on and grinding (DL has no wall jump and no zip lines:
  "Deadlocked's moves"; its ledges, hits and swimming: "Deadlocked's ledges",
  "Deadlocked's hits", "Deadlocked's swimming"). The kit's DL profile keeps UYA's numbers
  for them, marked as such. Melee is read ("Deadlocked's melee"), and so are the strafe
  turn and the camera (2026-10-16, `camera.md`, "Deadlocked's camera"): UYA's strafe turn,
  and a follow camera 6 u back. The flips, the crouch and the glide are read (2026-10-20,
  "Deadlocked's moves").
- **Releasing the stick in the air, both games:** the game brakes instead of steering
  when the speed the stick asks for (the move's air speed × the stick) is under 20% of
  the walk speed, not when the stick is under 20% (UYA 0x4EDE80, DL 0x680A88) [code].
  For the double jump's 4.75 u/s that means a stick under 0.26 in UYA and 0.31 in DL.

### Deadlocked's moves [verified on DL NTSC, 2026-10-15]

Seven traces of scripted keyboard play on DreadZone Station, `dlntsc-dreadzone-moves-{1..7}`
(`docs/re-setup.md`), 7,100 frames. Takes 1, 2, 4, 5 and 7 play with DL's camera option
set to third person (the word at 0x1711D0 written to 0), where the stick turns the hero as
in UYA; takes 3 and 6 with DL's default lock-strafe camera.
- **The walk:** up at 0.6167 u/s a frame (37 u/s²) to 7.25 u/s.
- **Jumps:** standing and run jumps have UYA's heights, hold and push-off. Held fully, a
  jump peaks 1.8865 u above the take-off (UYA's trace: 1.8867). A run jump flies at
  7.25 u/s.
- **The double jump:** its vertical speed tops out at 0.116667 u a frame (7 u/s). A
  second press on the jump's 9th frame was kept and double jumped on the 11th; a press on
  the 13th double jumped at once. Past the apex, a press while falling at 4.8 u/s double
  jumped, and at 5.2 u/s didn't: the game compares the 5.8 u/s limit with the speed after
  that frame's gravity, which had reached 5.8 u/s.
- **No glide:** X held through a double jump that rose 3 u gave no glide. Explained in
  code (2026-10-20, below): the glide is barred for good.
- **Strafing:** in third person L2 strafes (the strafe flag at hero+0x5F4 is 1), at the
  walk's speeds. Sideways or back with X it jumps (state 7) and keeps moving the stick's
  way at 7.25 u/s: DL's walk-jump function (0x687430) works out a flip and then always
  starts a standing jump.
- **Flips:** none came from L2 or R1 held with a direction and X, in either camera. They
  come only as a strafing jump lands (below).
- **No crouch:** R1 (third person) and L2 (lock-strafe) don't crouch on DreadZone
  Station. In lock-strafe, L2 is first-person aiming (state 29, camera type 2), and L1
  jumps as X does.

Read in DL's code on 2026-10-20 (`re/SCUS-97465/symbols.toml`), and traced in
`dlntsc-dreadzone-flip-{1,2}`:
- **Deadlocked's flips [code, verified on DL NTSC]:** the only code that enters state 11 is
  the landing flip ("Flips"): a jump pressed as a jump lands, strafing, the stick past 0.9
  and not forward. The take's double jumps strafing left, right and back landed into flips
  with sides 0, 1 and −1.
- **Deadlocked has no crouch [code]:** no code in the level's build enters the crouch (4),
  the long jump (10) or the heli jump (15). DL's hero code is UYA's with the crouch cut out:
  where UYA crouches or flips on R1, DL still works out R1's pad bit and ignores the
  result: in the skid (0x6A1770), in a jump's first frames, where UYA turns it into a long
  jump (0x6A3D88), and in a jump still on the ground, where UYA turns it into the crouch
  flip (0x6A3D28). The crouch's own case in `hero_transitions` is still there, unreached.
  So the long jump, the heli jump and the crouch flips are unreachable too, and the
  stick's flick (UYA's pad bits 0x1F000) has no use left.
- **The glide is barred [code, traced]:** DL's glide checks only a halfword at hero+0x2E2C
  where UYA checks the heli-pack. The hero is created with it set to 1 (0x669008), and no
  other store anywhere in EE memory reaches it by offset (a scan of every store
  instruction; a write through a computed pointer would escape it). It also skips the
  update of the moby
  at hero+0x23E4 (0x669D30) and, in `hero_set_animation`, a call to 0x688A60: probably
  Clank's place on his back, which DL doesn't use. The flip takes held it at 1
  throughout.
- **Deadlocked has no wall jump [code; checked on DL NTSC, 2026-10-20]:** nothing in the
  level's code enters state 17 (every `li a1, 0x11` near a state change was checked; two
  pass gadget 0x11, the charge boots, to the gadget code at 0x688858). The jump states'
  motion (`hero_update_jump`, 0x696620) ends with `hero_jump_vertical` where UYA's goes on
  to `hero_detect_wall`, and no other code probes for walls with UYA's 75°, 50° and 170°
  limits. The jump states' transitions lack UYA's wall-jump check between the thruster
  dash's fall and the long jump's attack window. State 17's entry and its motion in the
  air control are still there, unreached, like the crouch's. `dlntsc-dreadzone-wall-2`
  jumped against the steps beside the start and a wall above them with X tapped: double
  jumps and no state 17, but the wall there wasn't a clean, tall one.
  `dlntsc-dreadzone-tallwall-{1..10}` jumped at DreadZone Station's tall back wall
  (x ≈ 658.5, facing −x), the hero against it from his jump's second frame, with X again
  7, 9, 13 and 19 frames in (and later), standing and from a walk. Each gave a double jump,
  never state 17. He edged 0.3 u further into the wall as he rose, so its foot may slope a
  little.
- **Deadlocked has no zip lines [code]:** its state 118 is the death fall (libdl's
  `DEATH_FALL`), not UYA's zip line: `hero_transitions` enters it when the hero is below
  the level's kill height (0x222BFC) and more than 2 u over the ground, its motion keeps
  0.9 of the horizontal speed each frame and falls at up to 50 u/s, and after 120 frames
  it empties his health (hero+0x2E20). No code in the level's build holds the zip line's
  1.34 u hand height (0x3FAB851F), which UYA's catch tests.
- **Buttons per camera (`pad_action_mask`, 0x5BE808):** the hero asks for a button by
  UYA's pad bit, and with
  DL's camera byte (0x171DE0 + player) not 0, R1 and L1 become L2, Triangle becomes
  Triangle or R2, and X becomes X or L1. While the global at 0x21E824 isn't 1 (not
  identified), L2 becomes L2 or R2.
- **Letting go of the walk with a gun in hand:** one frame braking at 30 u/s², then idle
  at 1.5 u/s less one frame of 17.7 u/s². UYA's trace (the wrench in hand) skids two frames
  at 17 first. The release skid depends on the held item (`hero_update_walk` reads the
  hand slot), not on the game.
- **The lock-strafe camera:** the strafe flag is 2 while idle and 1 while walking, so the
  hero faces the camera, with UYA's strafe turn; standing, he only turns once the camera
  is more than 10° off. With the wrench in hand the flag is 0 and he turns to where he
  walks (`camera.md`, "Deadlocked's camera"). The kit doesn't have this camera yet.
- **DL's camera option:** libdl's `CameraType` (hero+0x1AE0: 0 third person,
  1 lock-strafe, 2 first person) follows a byte per player at 0x171DE0, which follows the
  option word per player at 0x1711D0 (`hero_camera_type_update`, 0x68BBA0).

### In the kit: a profile per game

The controller is shared; each game is a RON profile of numbers
(`crates/rac_hero/assets/uya.ron` and `dl.ron`, `HeroProfile::dl()`), plus switches where
the games' rules differ (`strafe.flips`, `walk.crouches`, `wall_jump.enabled`,
`zip_line.enabled`, `swim.dives`). With
`walk.crouches` off, the
crouch button still throws the wrench and taps the charge boots, as DL's R1 does, but never
crouches, so the crouch flips and the long and heli jumps can't start. DL's barred glide is
`Abilities::heli_pack` off, which a DL game sets; the demo turns every ability on (PLAN.md
section 5, 2026-10-12). `dl.ron` holds DL's values from the table
above, and UYA's where DL's code isn't read yet, marked in its comments. A test
(`deadlocked_differs_from_uya_only_where_the_games_do`) lists the fields that differ, and
`crates/rac_hero/tests/deadlocked.rs` checks the DL traces' walk, jumps, double jump,
strafe jump, landing flip and missing crouch. The app chooses with `--moves uya|dl|FILE` or the menus' Moves button.

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
| 21 | throw attack (wrench) | 6 attack | UYA |
| 22 | get hit | 7 get hit | R&C3 |
| 23, 24 | ledge grab, ledge idle | 3 ledge | Gladiator, plus 24 in an R&C3 savestate |
| 27 | ledge jump | 4 jump | Gladiator |
| 29 | targeting (weapon out) | 0 idle | R&C3, Gladiator |
| 101 | skid to position (scripted move) | 24 NPC | R&C3 |

Also read in code but not seen in a trace:
- in DL's: 121, the bounce off a wall after a charge (type 10), and 125, the charge
  (type 28);
- in UYA's: 18, the water jump; 50–52, swimming under water (type 17); and 53–54,
  swimming on the surface (type 18).
- UYA's code uses 117 for wading, where libuya's names say "get hit under water", so
  libuya's numbering above about 100 may not hold for UYA's single-player code.
- UYA's single player uses 118 for the zip line; DL's uses it for the death fall, as
  libuya's and libdl's `DEATH_FALL` name it ("Deadlocked's moves").

## Timing [measured]

- **One update per frame, at a fixed step.** Frames that took longer on the host never
  show larger steps. During a scripted event in R&C3 (state 101), the game stopped
  counting frames for about 18 vsyncs, and the hero did not move during them.
- **Input latency:**
  - A button pressed in input-recording frame *n* first changes the hero state two frames
    later. 54 of 66 jump presses in the four recordings show exactly that.
  - The stick floats the hero reads also lag the recording by two frames.

## Next steps

1. **More on Deadlocked NTSC.** The charge (2026-10-12), the walk, jumps and double jump
   (2026-10-15), and the flips, the missing crouch, the barred glide, the missing wall
   jump and the ledges, shimmy and climb (2026-10-20) are verified ("Deadlocked's moves",
   "Deadlocked's ledges"). Next: the parts of the kit's DL profile still marked "not read
   in DL" (lock-on, grinding, the hazard floor, death, the surface float, the stick's
   cut-off), and the code behind the 0.43 u step limit. The lock-strafe camera is read
   and traced (2026-10-16, `camera.md`, "Deadlocked's camera"), and the kit has it, as
   `dl.ron`'s default camera (P1.7).
2. **Moves that need a particular place:** wall jump, ledge grab and jump, grinding and
   swimming. Swimming, ledges and the wall jump were traced on 2026-10-13, and zip lines
   on 2026-10-15. UYA has no rail to grind on (section "Grinding"), so grinding waits for
   GC or DL.
   - The wall jump, ledges, slope limit and getting hit are read in UYA's code, and the
     charge boots in DL's (sections above, 2026-10-12). Grinding and swimming followed on
     2026-10-13.
   - Veldin's starting area has none of them, so they are untraced. A savestate near a
     wall and a ledge (from the owner or a scripted play-through) would let us check the
     probe distances and the shimmy speed.
   - The Gladiator traces already hold ledge grabs to compare.
3. **Part-tilted stick on NTSC:** the dead zone, the 0.25 cut-off and the speed's scaling
   with the magnitude are verified with keys at part tilt (`AXIS_SCALE`,
   `docs/re-setup.md`). Still to check: a diagonal (two keys at once), for the
   `(max + length) / 2` rule.
4. **Camera (RE.4):** started in [camera.md](camera.md).
5. **Constants for the kit:** write the tables above into the hero kit's RON profiles
   (P1.5) and use them as the RE.6 tolerance tests' expected values.

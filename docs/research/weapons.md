# Weapons: how Ratchet's guns work

How the games define, level up and fire weapons (PLAN.md RE.7), the reference for the
hero kit's weapons framework (P1.9). Started 2026-10-16 on UYA NTSC-U; DL follows.

## Status and method

- **Memory images:** UYA NTSC-U (SCUS-97353) on Veldin, the import `uyantsc-veldin` of
  `docs/re-setup.md` (a new game, so the hero owns no gun yet). Florana, the Phoenix and
  the Aquatos sewers were compared for the table's placement.
- **Labels:** **[code]** read in the game's code; **[data]** read from the memory image;
  **[text]** said by the game's own vendor descriptions (row +0x3E), which tell what each
  level adds, not checked in code unless also marked [code]. Nothing here is traced yet:
  no savestate of ours holds a gun.
- Addresses below are in Veldin's image. Code and the weapon table move with each level's
  overlay; the item arrays at 0x142xxx and the hero at 0x1A4BC0 don't.
- Names come from `re/SCUS-97353/symbols.toml`, where the functions are listed.

## Where the data lives [code, data, UYA NTSC, 2026-10-16]

- **The weapon table** (`weapon_table`): rows of 0xF0 bytes, one per item and level, at
  0x2C5640 on Veldin. It is static data in each level's copy of the game program: the
  same bytes sit at 0x2C3200 on Florana, 0x2C4A00 on the Phoenix and 0x2C2940 in the
  sewers, and neither the boot ELF nor the title screen has it. So it is on the disc,
  inside the level overlays, and can be read from the user's disc once P2.6 unpacks them.
- **Per-item arrays**, indexed by item number, kept with the save:

  | Address | Size | What |
  |---|---|---|
  | 0x1425C0 | byte | `item_weapon_rows`: the item's current row, which is its level |
  | 0x1427F0 | word | `item_ammo` |
  | 0x142A60 | byte | `item_flags`: the mods (below) |
  | 0x142CA0 | byte | `item_owned` |
  | 0x142D40 | byte | `item_new`: set when given; the menus' "new" mark |
  | 0x142DE0 | word | `item_xp`, in 1/32 points |

- **The hero's hand:** `held_item` (0x1A5E08) is what he holds; `equipped_gun` (0x1D4C40)
  the gun Circle brings out; 0x1D4C44 the boots slot, 0x1D4C48 the suit slot, 0x1D4C4C
  the back. `hero_spawn_item_mobys` (0x4E7148) spawns each slot's moby from its row.
- **Quick select:** two rings of eight items, 0x1D4C60 and 0x1D4C80. Giving an item adds it
  to the first free slot of the first ring, then the second (0x416A10).

## Items [data]

The item number is the row number of its first level. Names are the game's, from row +0x04.

| Items | What |
|---|---|
| 0x02–0x04 | Heli-Pack, Thruster-Pack, Hydro-Pack |
| 0x05–0x08 | Map-O-Matic, Commando Suit, Bolt Grabber V2, Levitator |
| 0x09 | OmniWrench (its V2–V8 rows: 0xA4, 0xA5, 0xC6–0xCA) |
| 0x0B | Hypershot |
| 0x0D–0x0F | Gravity Boots, Grindboots, Glider |
| 0x10, 0x11, 0x13, 0x15, 0x16 | Plasma Coil, Lava Gun, Bouncer, Miniturret Glove, Shield Charger (their later levels are rows 0xA0–0xC5) |
| 0x12, 0x14 | Refractor, The Hacker |
| 0x1D–0x23 | Charge Boots, Tyhrra-Guise, Warp Pad, Nano-pak, Star Map, Master Plan, PDA |
| 0x24–0x26 | The camera options: third person, first person, lock-strafe |
| 0x27, 0x2F, … 0x97 | The guns, eight rows each (V1–V4, the renamed V5, Mega, Giga, Omega): Shock Blaster 0x27, N60 Storm 0x2F, Infector 0x37, Annihilator 0x3F, Spitting Hydra 0x47, Disc Blade Gun 0x4F, Agents of Doom 0x57, Rift Inducer 0x5F, Holo Shield Glove 0x67, Flux Rifle 0x6F, Nitro Launcher 0x77, Plasma Whip 0x7F, Suck Cannon 0x87, Qwack-O-Ray 0x8F, RY3NO 0x97 (five rows only) |

A new game on Veldin owns 0x02, 0x03, 0x06, 0x17 and the three camera options; the
Shock Blaster and Nitro Launcher come later and cost nothing (price 0).

## The weapon row (0xF0 bytes) [code unless marked]

| Offset | Type | What | Read by |
|---|---|---|---|
| +0x00 | u8 | 1 for a weapon | many |
| +0x01 | u8 | level, 0–7 (V1 … Omega) | many |
| +0x02 | u8 | not read; shared by some weapons [data] | |
| +0x04 | u32 | name (text number) | menus |
| +0x08 | u32 | slot: 0 hand, 1 feet, 3 back, 4 body, 6 camera option [data, partly code] | `hero_item_in_slot` |
| +0x10, +0x14 | i32 | moby classes spawned for the item in hand (−1 none) | 0x4E7148 |
| +0x18 | u8 | hold type: 0 none, 1 one hand, 2 both hands | 0x4E7148, 0x4E6A08 |
| +0x24, +0x28, +0x2C | i32 | firing animations: whole body, arms only, arms only in hero state type 0xC | 0x4E6A08 |
| +0x30 | u8 | copied to 0x1A71CB when firing | 0x4E6A08 |
| +0x34, +0x36 | u16 | ammo-pickup message for several rounds, one round | ammo pickups |
| +0x3A | u16 | moby class of this weapon's ammo pickup | 0x30E7F8 |
| +0x3E | u16 | vendor description (text) | vendor |
| +0x44 | u16 | name of the tier (V1–V4 share one, V5 and up another) | 0x50F920 |
| +0x46, +0x48 | i16 | next and previous level's row, 0 at the ends | level-up |
| +0x4C | f32 | projectile speed in u/s (the Annihilator's 60, the Flux Rifle's 400) | those guns |
| +0x50 | f32 | shots per second, for the guns whose code reads it (the Shock Blaster, Nitro Launcher, Annihilator, Flux Rifle) | weapon code |
| +0x68 | i32 | total XP at which the weapon leaves this level; 0 for none | 0x416640, menus |
| +0x6C | f32 | damage | every weapon's code |
| +0x70–+0x78 | f32 | further damages; meaning per weapon (the wrench's jump attack is +0x70) | |
| +0x7C | i32 | price in bolts: the gun's on its V1 row, the Mega upgrade's on its Mega row | vendor, help text |
| +0x84, +0x86 | u16 | price of one round at the vendor; the second in another vendor mode | 0x4B8560 |
| +0x88 | u16 | not read yet | |
| +0x8A | u16 | maximum ammo; 0 means no ammo used | 0x50D8F8 and others |
| +0x8E | u16 | ammo when first given | 0x416B38 |
| +0xD0 | i32 | message on reaching this level (text) | 0x416CC8 |

## Owning, ammo, levels and XP [code, UYA NTSC, 2026-10-16]

- **Giving an item** (`item_give`, 0x416B38): marks it owned and new, fills its ammo from
  +0x8E if it uses ammo and has none, lists it for the vendor (0x416BF8), and adds it to
  quick select.
- **Ammo:**
  - `ammo_use(item, n)` (0x50D8F8), called by every gun as it fires: with no maximum
    (+0x8A = 0) it always succeeds; otherwise it takes *n* rounds if there are that many,
    else fails and the gun doesn't fire. Item −1 means the held item.
  - `ammo_get` (0x50DA08) returns 1 for guns without ammo.
  - `ammo_add` (0x50D980) adds and caps at the maximum, returning what didn't fit.
  - **Pickups** (0x30E7F8): an ammo moby finds its gun by matching its class to row
    +0x3A, then adds the amount its own data holds.
  - **The vendor** sells rounds at +0x84 bolts each, up to the maximum.
- **Levels:** a row's +0x46 is the next level's row, so the level-ups of one gun form a
  chain. `item_weapon_rows` holds the current row, so levelling up just moves the item
  along its chain:
  - `weapon_level_up(item)` (0x416640): raises the XP to the row's threshold if it is
    below, moves to the next row, and fills the ammo to the new maximum. 0x416CC8 then
    shows the row's +0xD0 message.
  - `weapon_set_level(item, level)` (0x416790) walks the chain from V1.
- **XP:** `item_xp` counts in 1/32 points. Row +0x68 is the total XP at which the weapon
  leaves that level, so V1 → V2 → … → V5 at the four thresholds of V1–V4 (the Shock
  Blaster's 150, 400, 700 and 1000). V5 has none: in the first game a gun stops at V5.
  - **Mega is bought:** the vendor's upgrade (0x4176F8) moves a gun to level 5 (Mega),
    resets its XP to 0 and refills its ammo; the price is the Mega row's +0x7C. Mega and
    Giga then level up by XP from 0 (the Shock Blaster's 10,000 and 25,000); Omega is the
    end.
- **XP from kills** (`moby_killed`, 0x4412D0): the enemy's XP (a per-class table) goes
  to the gun whose hit killed it (the target's damage record +0x17), times
  (1 << 0x1A74A8) and, in challenge mode, a per-level factor (0x2F1640 + level × 4).
  - A V5 gun gets none outside challenge mode.
  - For a tracked big enemy (the list at 0x1E0284), the XP is shared among all guns in
    proportion to the damage each did (per-gun tallies at 0x21FFB8, 0x270 bytes each).
  - The same kill adds to the hero's own XP (0x142694), which raises his health, and
    drops the enemy's bolts.

## Mods [code for the flags; text for what they do]

`item_flags` holds three mods per gun, set by the gun's own code from its level each frame:

| Bit | Mod | What it does |
|---|---|---|
| 0x20 | lock-on | L2 and R2 together lock on (`hero-controller.md`, "Lock-on") |
| 0x02 | shock | each hit also arcs to enemies near the target, as damage over time (0x34BF98) |
| 0x08 | acid | hits leave acid that does damage over time [text]; checked by 0x31AC38 on each hit [code] |

Seen in code [code]:

| Gun | Lock-on from | Shock from | Acid from |
|---|---|---|---|
| Shock Blaster | V3 | V4 | |
| N60 Storm | V3 | V4 | |
| Annihilator | V4 | V3 | |
| Rift Inducer, Lava Gun, Plasma Coil, Bouncer | V3 | | |
| Nitro Launcher | V3 | | V2 |

The vendor's texts add acid at V2 for the Lava Gun, Plasma Coil and Bouncer, and lock-on
for several V5 guns [text]; their code isn't read yet.

## Switching and firing [code, UYA NTSC, 2026-10-16]

- **Buttons** (0x4E8830, from `hero_transitions`):
  - Square brings out the wrench (item 9) and remembers the gun (0x1A7210).
  - Circle brings out the equipped gun. R1 does too in the camera modes where R1 fires
    (0x50FF18).
  - With the wrench out, L2 and R2 together bring out the gun when its lock-on mod is on.
  - The gun fires on the button in 0x1A5DF0: 0x20 (Circle) for guns, 0x80 (Square) for
    the wrench.
- **Animation** (0x4E6A08, called as a gun fires): standing still, the hero plays the
  row's whole-body firing animation (+0x24); moving, or in the aim mode with the right
  stick past 10°, the arms-only one (+0x28, or +0x2C in hero state type 0xC) on one arm
  layer, or both for a two-handed gun (+0x18 = 2).
- **Damage** reaches targets through the same damage records as the wrench's
  (`damage_info_fill`, 0x443628), with the weapon's item number in the record, which is how
  a kill credits its XP.
- **One pending hit per target** (`damage_queue`, 0x443840; `damage_queue_add`,
  0x443938): hits go into a ring of 64 records at 0x1EED00, and a moby's +0xA8 is the
  index of its pending one. A new hit on a moby that already has one pending replaces it
  if it does at least as much damage (record +0x1C); a weaker one only adds its flags. So
  a target takes at most its strongest hit per update, however many shots, stream points
  or blast frames touch it. How often each enemy then takes its pending hit (any
  invulnerability after a hit) is in each enemy's own code, not read.

## Each gun

### Numbers [data]

Per level, V1–V5 then Mega, Giga, Omega (RY3NO has V1–V5 only). "Price" is the gun's,
then the Mega upgrade's.

| Gun | Damage (+0x6C) | +0x50 | Max ammo (+0x8A) | Round price | Price; Mega | XP thresholds (+0x68) |
|---|---|---|---|---|---|---|
| Shock Blaster | 2, 2.5, 3, 4, 5; 55, 70, 105 | 2 | 30, 35, 40, 40, 50; 50, 55, 60 | 5 | free; 420,000 | 150, 400, 700, 1000; 10,000, 25,000 |
| N60 Storm | 1.5, 1.75, 2, 2.5, 3.5; 35, 50, 60 | 7 | 150, 175, 200, 225, 300; 300, 350, 400 | 2 | 15,000; 460,000 | 200, 500, 1500, 3300; 15,000, 37,500 |
| Infector | 9, 12, 16, 20, 30; 200, 250, 300 | 5 (0.7 from V5) | 15, 15, 18, 18, 20; 20, 25, 30 | 20 | 18,000; 220,000 | 400, 800, 2000, 3800; 10,000, 25,000 |
| Annihilator | 25, 30, 40, 55, 70; 150, 200, 250 | 1.7 | 20, 20, 22, 22, 25; 25, 28, 30 | 20 | 150,000; 580,000 | 800, 2400, 6400, 12,400; 10,000, 25,000 |
| Spitting Hydra | 10, 12, 14, 16, 20; 160, 270, 300 | 1.7 | 15 (18, 20 at Giga, Omega) | 5 | 40,000; 540,000 | 300, 900, 1800, 3000; 15,000, 37,500 |
| Disc Blade Gun | 25, 30, 35, 40, 45; 55, 70, 105 | 2 | 25 (28, 30) | 10 | 175,000; 500,000 | 700, 2100, 6100, 12,100; 10,000, 25,000 |
| Agents of Doom | 12, 16, 22, 25, 30; 40, 50, 60 | – | 6, 6, 8, 8, 8; 8, 10, 12 | 75 | 60,000; 660,000 | 400, 1000, 3000, 6000; 7500, 20,000 |
| Rift Inducer | 50, 65, 75, 85, 100; 200, 250, 300 | 1.7 | 8, 8, 10, 10, 12; 12, 14, 16 | 100 | 180,000; 780,000 | 800, 2400, 6400, 12,400; 15,000, 37,500 |
| Holo Shield Glove | 10, 15, 20, 25, 30; 50, 75, 100 | 1.7 | 8, 8, 10, 10, 12; 12, 12, 14 | 20 | 30,000; 140,000 | 150, 450, 1350, 2700; 5000, 12,500 |
| Flux Rifle | 15, 20, 25, 80, 140; 260, 350, 420 | 1 | 10, 12, 12, 12, 15; 15, 18, 20 | 50 | 75,000; 340,000 | 200, 600, 1500, 2900; 7500, 20,000 |
| Nitro Launcher | 10, 12, 15, 20, 40; 210, 250, 300 | 0.5 | 8, 8, 10, 10, 12; 12, 14, 16 | 50 | free; 380,000 | 200, 500, 1100, 2600; 7500, 20,000 |
| Plasma Whip | 2, 2.5, 3.5, 5, 7; 70, 90, 120 | – | 25, 30, 35, 40, 50; 50, 55, 60 | 3 | 7500; 180,000 | 200, 800, 1800, 3300; 10,000, 25,000 |
| Suck Cannon | 10, 13, 16, 22, 30; 210, 250, 300 | – | none | – | 8000; 100,000 | 200, 600, 1200, 2000; 5000, 12,500 |
| Qwack-O-Ray | 50, 75, 100, 125, 150; 200, 250, 300 | – | none | – | 200,000; 300,000 | 1000, 3000, 8000, 16,000; 10,000, 25,000 |
| RY3NO | 300, 350, 400, 450, 500 | 1 | 25, 30, 35, 40, 50 | 1000 | 3,000,000 | 20,000, 50,000, 90,000, 140,000 |
| Plasma Coil | 120, 150, 180, 210, 150; 170, 190, 210 | 1 | 15, 15, 18, 18, 20; 20, 22, 25 | 50 | 250,000; 820,000 | 8000, 18,000, 30,000, 44,000; 15,000, 37,500 |
| Lava Gun | 8, 12, 18, 25, 35; 130, 150, 180 | 0 at V1, then 4.5, 7.5 | 150, 150, 175, 175, 200; 200, 250, 300 | 1, then 3 | 40,000; 260,000 | 600, 1500, 2700, 4200; 10,000, 25,000 |
| Bouncer | 30, 35, 40, 45, 50; 75, 90, 110 | 0.3 | 10, 10, 12, 12, 12; 12, 14, 16 | 50 | 150,000; 740,000 | 2500, 8500, 18,500, 30,500; 10,000, 25,000 |
| Miniturret Glove | 1.5, 2, 2.5, 3, 8; 40, 50, 60 | 6 (1.5 from V5) | 10, 10, 12, 12, 12; 12, 14, 16 | 15, then 30 | 25,000; 620,000 | 400, 1000, 2000, 3500; 10,000, 25,000 |
| Shield Charger | 3, 5, 7, 9, 12; 15, 20, 25 | – | 3, 3, 3, 4, 4; 4, 5, 5 | 500 | 250,000; 700,000 | 2200, 5000, 9600, 16,800; 12,000, 30,000 |

- **+0x50 is not every gun's rate [code].** The Shock Blaster, Nitro Launcher,
  Annihilator and Flux Rifle read it as shots per second. The N60 Storm ignores its 7 and
  fires every 14 frames (below); the other guns' rates aren't read yet.
- Guns with a second damage (+0x70): the Annihilator 6, Spitting Hydra, Rift Inducer and
  Holo Shield Glove 5, RY3NO a tenth of its damage, and the Plasma Whip, Suck Cannon and
  Plasma Coil the same as +0x6C. What each uses it for isn't read.
- Prices on other rows than V1 and Mega are copies and unused as far as read.

### What each level adds [text]

From the vendor descriptions, in short; [code] where the code was read:
- **Shock Blaster:** a spread of shots that knocks enemies back. V2 adds a charged shot
  [code]; V3 lock-on [code]; V4 shock [code]. The Shock Cannon charges into a beam that
  can be swept.
- **N60 Storm:** a rapid-fire gun. V2 longer range, and shots ricochet off walls; V3
  lock-on [code]; V4 shock [code]. The N90 Hurricane's shots bounce off anything.
- **Infector:** turns enemies against each other. V2 spreads to badly hurt enemies nearby;
  V3 infects by splash; V4 infects faster. The Infecto-Bomb infects everything in its blast.
- **Annihilator:** guided mini-missiles. V2 missiles scatter mines; V3 mines roll and home;
  V4 better tracking and lock-on [code: lock-on at V4, shock at V3]. The Decimator
  fires a dozen heat-seekers.
- **Spitting Hydra:** hold to mark targets, release for lightning at each; up to 4, 5 and
  6 targets at V2–V4, 7 for the Tempest.
- **Disc Blade Gun:** a homing disc that ricochets off walls. V2 picks a new target at
  each ricochet; V3 ricochets off enemies too; V4 longer range and harder blast. The
  Multi-Disc Gun's discs split, up to sixteen.
- **Agents of Doom:** robot bodyguards. V2 four agents with a cannon, then kamikaze; V3
  rapid fire; V4 jet packs. The Agents of Dread carry seeking rockets and explode at the end.
- **Rift Inducer:** black holes that swallow enemies. V2 holes merge; V3 lock-on [code];
  V4 stronger pull. The Rift Ripper adds shock.
- **Holo Shield Glove:** shields to hide behind. V2 drains health from enemies to Ratchet;
  V3 absorbs shots and fires back; V4 bigger and tougher. The Ultrashield fires three
  pulses when hit.
- **Flux Rifle:** a sniper rifle with a zoom. V2 and V3 pierce several enemies; V4 shots
  explode. The Splitter Rifle hits harder.
- **Nitro Launcher:** lobbed charges with a short range and a wide blast. V2 acid [code];
  V3 lock-on [code]; V4 bigger blast. The Nitro Eruptor adds a spread of missiles.
- **Plasma Whip:** a long melee whip. V2 and V3 more damage; V4 a jump-attack shock
  wave. The Quantum Whip sends cutting waves in combos.
- **Suck Cannon:** sucks up enemies and crates and fires them back. V2 harder shots, double
  range, ricochets; V3 holds more and sets them alight; V4 harder still. The Vortex
  Cannon fires burning comets.
- **Qwack-O-Ray:** turns enemies into ducks. V2 ducks lay exploding eggs; V3 the eggs
  seek; V4 ducks attack and draw fire. The Qwack-O-Blitzer's ducks fly and explode.
- **RY3NO:** eight mini-missiles, each locking on to its own enemy; 12, 14 and 16 seeking
  blasts at V2–V4. The RYNOCIRATOR fires a huge blast.
- **Plasma Coil:** plasma balls with some tracking that arc lightning on impact. V2 acid,
  V3 lock-on [code], V4 more power; the Plasma Storm.
- **Lava Gun:** a stream of lava. V2 acid, V3 lock-on [code], V4 more power; the Liquid
  Nitrogen Gun freezes.
- **Bouncer:** a canister that bursts into bouncing bomblets. V2 acid, V3 lock-on [code],
  V4 more power; the Heavy Bouncer.
- **Miniturret Glove:** turrets that shoot nearby enemies. V2 bigger blasts; V3 ice
  shots; V4 faster aim. Megaturrets fire guided missiles and move.
- **Shield Charger:** a shield round Ratchet that shocks what touches it; stronger at
  each level. The Tesla Barrier zaps enemies nearby.

### Shock Blaster (0x36A790) [code, UYA NTSC, 2026-10-16]

The gun moby's update reads its row through item 0x27.
- **When it may fire:** the gun has been out for 23 frames or more (0x1A5E00), its last
  shot is over, the cooldown has run out, there is ammo, the game is in normal play (game
  mode 0) and the hero isn't in state 116.
- **Firing at V1:** Circle pressed fires. Each shot takes one round, plays the firing
  animation (0x4E6A08) and sets a cooldown of 1 / (+0x50) = 0.5 s.
- **Firing from V2:** no cooldown. Holding Circle fires at once (the gun tries on each of
  the first 20 frames held) and then charges, with sparks that grow. Released after more
  than 78 frames held, it fires a charged shot of three times the damage; released
  between, nothing more.
- **A shot's timing:** the hit lands 2 frames after the shot starts, or 5 while the
  whole-body firing animation is already playing, the hero is moving, or 0x1A7454 is set.
  Then 20 frames pass before the next shot.
- **The shot is hitscan:**
  - First, balls along the barrel: the *n*th at the gun's position less 1.35*n* u along
    its matrix's second row and 0.175*n* u along its first, of radius 1.5, 1.75, 2.0 and
    2.25. Only the first is used unless 0x1D7BE0 is above 8 (it is −1 in normal play) and
    the level isn't 8, 22 or 28. They hit anything but the hero.
  - Then 23 rays of 32 u: 20 at random, up to 17° off the aim with any roll, and three at
    15° down, spread 20° apart (0x36B888 tests each).
  - At V2 and up, each enemy the balls hit also gets the shock mod's check (0x34BF98).
- **Aim:** with a target in 0x1A57B8 (the lock-on target is copied there), the aim is
  turned towards it (0x454298) into 0x1A52A0; the shot uses 0x1A52A0, or the camera's aim
  (0x1E2A60) with the lock-strafe camera (camera mode 1).
- **First person:** in the look state (1) the gun puts the hero in state 29, its aiming
  state, where R1 fires as well as Circle.
- The moby's +0x7C (2 at V2, 3 from V3) is set from the level; its use isn't read.

### N60 Storm (0x3A5928, bullets 0x35B390) [code, UYA NTSC, 2026-10-16]

- **Firing:** while Circle is held (or R1 in state 29), with the same conditions as the
  Shock Blaster's (game mode 0, the gun out more than 22 frames), it fires at once and
  then every 14 frames (about 4.3 shots a second), one round each. Letting go resets the
  timer, so the next press fires at once.
- **Two barrels:** the shots alternate between two muzzle points (0x1DE340 and 0x1DE350
  in the gun's frame).
- **Aim:** the gun aim assist below (35 u; 3° by 6° at range, 45° close), overridden by
  the lock-on target. With a target the bullet heads for it, else along the hero's facing.
  In the aiming state (29), and in the lock-strafe or first-person camera without a
  lock-on target, it goes along the camera's aim (0x1E2A60) instead.
- **The bullet** is a moby of class 0x1851 (0x35B240 spawns it):
  - speed: a random 60–80 u/s, fixed at the shot;
  - each frame it moves, then tests the line from its last position (0x3F8448);
  - lifetime: 30 frames at V1 (30–40 u), 46 from V2 (the "longer range");
  - it carries the row's damage (+0x6C) and the gun as attacker;
  - **at V1** it stops at the first thing it hits, damaging a ball of 0.6 u there (1.0 u
    on a target);
  - **from V2** it bounces off anything that isn't a target (the velocity reflected about
    the surface's normal) and keeps flying; a target takes the hit, the shock mod's check
    and ends the bullet. It passes through the N60 itself and the shooter;
  - the impact throws sparks, more from V2 and more again from V3 (0x35BDC8).
- **Mods:** lock-on from V3, shock from V4 [code]. The moby's +0x7C is 1 at V2, 3 at V3.

### Nitro Launcher (0x36E360, charge 0x361218, blast 0x35E718) [code, UYA NTSC, 2026-10-16]

- **Firing:** Circle held, with the usual conditions, fires one charge and sets a cooldown
  of 1 / (+0x50) = 2 s. In the look state (1) or state 29 it waits 40 frames first.
- **Target:** the lock-on target if it is alive and within 12 u (6 u in the levels with
  their own gravity), else the aim assist with range 11.1 u, a cone of nothing near and
  22.5° yaw by 90° pitch at range, skipping the classes in 0x1DD950 (the Nitro guns
  among them). With 0x1DB030 set on level 5: 30 u, and other cones.
- **The arc** (0x36EA80): the aim point is the target, or 12 u ahead along the hero's aim.
  The charge flies at 12.5 u/s across the ground, and its upward speed is solved so that it
  reaches the aim point's height under 16 u/s² of gravity. A launch faster than 0.5 u a
  frame is cut to 22 u/s. With the button of action 4 held (0x45E3B8), it aims instead
  where the camera's aim meets the ground under the hero, up to the 12 u.
- **The charge** (class 0x185A, spawned by 0x361880) spins as it flies:
  - gravity 12 u/s² along the level's up (0x4C4190), not the 16 it was aimed with, so it
    flies higher and lands further than the aim point;
  - it explodes on an enemy within 1.5 u of it (class category 5);
  - it bounces off the world (velocity reflected) and explodes 10 frames later;
  - it passes through the Nitro guns (classes 0x1865, 0x1885) and objects of class 0;
  - after its first 40 frames, it explodes on reaching the ground (a 1 u probe down).
- **The blast** (class 0x1858, spawned by 0x35E5D8): 12 frames of flash, then for 20
  frames a ball whose radius grows from 2 u to 6 u (7 u from V4), damaging all it touches
  but the hero for the row's damage (+0x6C), pushing them away at 1.5. Each target hit
  also gets the acid mod's check (0x31AC38). The camera shakes by 0.4 − 0.02 × the
  distance to it, within 20 u.
- **Mods:** acid from V2 (set as each blast spawns), lock-on from V3 [code].

### Lava Gun (0x303F88, stream 0x304A18) [code, UYA NTSC, 2026-10-16]

- **The stream's strength** (gun pvar +0x50) follows Circle through `game_spring`
  (0x445168): held, it rises towards 10 (gain 0.0075, damping 0.002); released, it falls
  to 0 (0.06, 0.01). One round goes every 10 frames held (6 a second).
- **The stream** is a ring of 35 points (0x305540): while the strength is above 0, each
  frame adds one at the nozzle, moving at strength × 0.05 u a frame (up to 30 u/s) along
  the gun's aim raised by 0.02, and living 40 frames. A lock-on target turns the aim onto
  it (0x454298); while the hero moves, the aim tilts 5.6° down.
- **Each point** (0x304A18) falls at 21.6 u/s² and tests the line of its step:
  - on the world it stops and splashes, and one time in 11 leaves burning lava there
    (0x3063C8);
  - on an enemy it does the row's damage (+0x6C) to it and +0x70 (8 at V1, 12 at V2, 0
    after) to everything within 1.5 u, as fire, or as cold from V5;
  - from V5 (the Liquid Nitrogen Gun), an enemy with no more health than one hit's
    damage is frozen to death instead, and its XP goes straight to the gun.
  - Each point hits once; with the damage queue's one pending hit per target, an enemy in
    the stream takes at most one hit per update.
- **At the nozzle** a ball of 0.5 u also damages what it touches (+0x6C).
- **Mods:** lock-on from V3 [code].

### Annihilator (0x36D5A0, missile 0x396A20) [code, UYA NTSC, 2026-10-16]

- **Target:** the gun aim assist from 0.75 u above the hero's feet along his aim
  (0x1A52D0): 30 u, cones of 18° yaw and 17° pitch (4° in the aiming state or with the
  lock-strafe camera). A candidate becomes the target after 5 frames as the best one, and
  is dropped after 5 frames of another being best, or when it dies. A reticle marks it.
- **Firing:** Circle held (R1 in state 29), the gun out 28 frames or more, and the cooldown
  over: one round, one missile, and a cooldown of 1 / (+0x50) = 0.59 s. With no target, an
  enemy within 1 u of a point just ahead of the barrel becomes the missile's target.
- **The missile** (class 0x1894, 0x3965B0):
  - speed: row +0x4C u/s (60), so it flies 75 frames, 75 u;
  - it starts towards the target, else along the hero's aim (the camera's aim in the
    aiming state or the lock-strafe camera);
  - it homes on its target while the target lives, for up to 294 frames (0x396A20);
  - each frame it tests the line of its step, carrying the row's damage (push 1.5) to the
    first thing it hits, and explodes;
  - from V5 the explosion also scatters seven bomblets of a fifth of the damage each
    (0x3441D0). The mines the vendor gives V2–V4 aren't found yet;
  - its model is drawn 2.5 times larger, 4 times from V5.
- **Mods:** shock from V3, lock-on from V4 [code].

### Flux Rifle (0x345350, shot 0x3463A0) [code, UYA NTSC, 2026-10-16]

- **The shot** moves at row +0x4C u/s (400), along the hero's aim, or the camera's aim with
  the lock-strafe or first-person camera; towards the target in 0x1A57B8 when there is
  one. It lives 128 u / 400 u/s = 19 frames, so its range is 128 u.
- **At the muzzle** a ball of 1.5 u takes the row's damage too.
- **Scoped** (the look state 1 or the aiming state 29 with 0x1A71C5 set), the aim is
  corrected towards targets within 128 u and 20° (0x4C7010), and from the first-person
  camera a hitscan from the camera goes along the aim as well (0x37C818).
- **Each frame** the shot tests the line of its step (0x346D90); what it hits takes the
  row's damage, and the shot stops there if that object can take damage (0x4435E8: its
  +0x98 record has +8 set), else flies on. Where V2 and up pierce isn't found: the shot's
  update (0x346A00) and its per-step calls (0x3478E8, and 0x348A38 from V3) only lay its
  trail.
- **Rate** (0x3444B0): a shot every 1 / (+0x50) = 1 s. Scoped, the zoom follows the
  buttons' pressure (pad +0x134 and +0x13C), and a ray from the camera along the scope
  (0x1E2E40, 128 u) marks a target with a reticle every fourth frame.

### Gun aim assist (0x44D930, 0x44CF68) [code, UYA NTSC, 2026-10-16]

Guns that aim for themselves (the N60 Storm, Disc Blade Gun, Annihilator, Spitting Hydra,
Rift Inducer and others; callers of 0x44D930) pick a target this way:
- **The lock-on target wins** while it is alive (health above 0, +0x20 not −2 or −3).
- **None in the lock-strafe camera** (camera mode 1 at 0x1A6290), where guns shoot
  along the camera's aim.
- Otherwise `gun_aim_target` (0x44CF68) searches the targetable list (0x21E8E0) for
  enemies (class category 5) with health left, aiming at each one's position raised by
  its damage record's height (+0x10):
  - within the range and at least 0.1 u away;
  - inside a cone round the aim whose half-angles go from a near pair (yaw, pitch) to a
    far pair as near + (far − near) × (1 − (1 − distance / range)³) (`cubic`, 0x442098,
    with −2, 0, 1, 0);
  - scored by yaw off × 3 + pitch off × 0.2 × distance + distance × 0.1 (scaled by
    range × 0.8 for some classes); the best-scored one in sight from the muzzle wins;
  - classes 0xC87, 0xECC and 0x1D86 are never picked (list at 0x1D7C30).
- The N60 asks for 35 u, far 3° yaw and 6° pitch, near 45° and 45°. With the byte at
  0x1DB030 set (0 in our image; several guns read it), it asks 50 u, 20° and 10°.

## Next steps

1. **The hero's side of firing:** 0x4E6D40, 0x4E6CB0 and 0x503930 / 0x5037C0 in
   `hero_transitions` (decompiled into `cache/ghidra/out/weap-d.c`, not read yet): whether
   firing turns or slows the hero, and the aiming state 29.
2. **Open in the archetypes:** the Flux Rifle's piercing from V2, the Annihilator's
   V2–V4 mines, and how the lock-on target bends each gun's shot (0x454298, 0x4C7010).
3. **The shock and acid mods' damage over time:** 0x34BF98 with its table at 0x2822A0
   (mostly zero in this image), and 0x31AC38.
4. **A savestate with guns, to trace firing:** on the private boot instance
   (`docs/re-setup.md`), set `item_owned` (0x142CA0 + item) and `item_ammo`, call or
   imitate `item_give`'s quick-select entry, then capture shots and compare with the
   numbers here (fire intervals, bullet speeds, the Nitro's arc).
5. **The other guns**, as above; their entry points are in `symbols.toml` and their row
   readers were found by listing the users of `weapon_table` and the item byte each reads
   (0x1425C0 + item).
6. **Deadlocked:** its `GadgetDef` table (0x3BF460, `moby-format.md`) and its mods.

How this was read, to carry on: import or reuse the Veldin project (copy
`cache/ghidra/proj/uya.{gpr,rep}` to a name of your own so sessions don't collide), list a
global's users with `RefsTo.java`, and decompile them with `DecompileAt.java`; row fields
show as `DAT_002c56xx` (0x2C5640 + offset) in the output. Names come from the text table
(16-byte entries of string pointer and text number, 0x1C31440–0x1C3ED80 in this image).

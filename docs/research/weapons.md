# Weapons: how Ratchet's guns work

How the games define, level up and fire weapons (PLAN.md RE.7), the reference for the
hero kit's weapons framework (P1.9). Started 2026-10-16 on UYA NTSC-U; DL follows.

## Status and method

- **Memory images:** UYA NTSC-U (SCUS-97353) on Veldin, the import `uyantsc-veldin` of
  `docs/re-setup.md` (a new game, so the hero owns no gun yet). Florana, the Phoenix and
  the Aquatos sewers were compared for the table's placement.
- **Labels:** **[code]** read in the game's code; **[data]** read from the memory image;
  **[text]** said by the game's own vendor descriptions (row +0x3E), which tell what each
  level adds, not checked in code unless also marked [code]; **[verified]** checked in
  our traces, which so far cover the hero's side of firing with the Shock Blaster and N60
  ("Checked in traces"). Projectiles aren't traced yet.
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
| +0x0C | i32 | the hero's matrix slot the item's moby is put on (0x1A5810 + slot × 0x40): 6 for the Miniturret Glove, 0 for every other gun and the wrench [code, data, 2026-10-23] | `hero_place_item_mobys` |
| +0x10, +0x14 | i32 | moby classes spawned for the item in hand (−1 none) | 0x4E7148 |
| +0x18 | u8 | hold type: 0 none, 1 one hand, 2 both hands ("Holding a gun") | 0x4E7148, 0x4E6A08, 0x4E61E8 |
| +0x24, +0x28, +0x2C | i32 | firing animations: whole body, arms only, arms only in hero state type 0xC | 0x4E6A08 |
| +0x30 | u8 | copied to 0x1A71CB when firing | 0x4E6A08 |
| +0x34, +0x36 | u16 | ammo-pickup message ("Got %d … ammo") for several rounds and for one; the same text for the six guns read [data] | `ammo_pickup_collect_test` |
| +0x38 | | the item's icon on the HUD and the quick-select ring (0 none) | 0x41AA38, 0x475078 |
| +0x3A | u16 | a level-placed ammo pickup whose class this is fills this gun; but guns share values (0xCC5 is the Annihilator's, Hydra's, Disc Blade's and others') and no row has the two pickup classes, so it reads as the ammo's model [code, data] | 0x30E7F8 |
| +0x3E | u16 | vendor description (text) | vendor |
| +0x44 | u16 | name of the tier (V1–V4 share one, V5 and up another) | 0x50F920 |
| +0x46, +0x48 | i16 | next and previous level's row, 0 at the ends | level-up |
| +0x4C | f32 | projectile speed in u/s (the Annihilator's 60, the Flux Rifle's 400) | those guns |
| +0x50 | f32 | shots per second, for the guns whose code reads it (the Shock Blaster, Nitro Launcher, Annihilator, Flux Rifle) | weapon code |
| +0x68 | i32 | total XP at which the weapon leaves this level; 0 for none | 0x416640, menus |
| +0x6C | f32 | damage | every weapon's code |
| +0x70–+0x78 | f32 | further damages; meaning per weapon (the wrench's jump attack is +0x70) | |
| +0x7C | i32 | price in bolts: the gun's on its V1 row, the Mega upgrade's on its Mega row | vendor, help text |
| +0x84, +0x86 | u16 | price of one round at the vendor; the second (10 × the first for the six guns read, but the Lava Gun's V1) at vendor kind 4 (`vendor_round_mode`) | 0x4B8560 |
| +0x88 | u16 | rounds in each pickup an ammo crate throws out; the same at every level [code, data] | `ammo_pickup_spawn` |
| +0x8A | u16 | maximum ammo; 0 means no ammo used | 0x50D8F8 and others |
| +0x8E | u16 | ammo when first given; equal to +0x8A in all 117 rows of the 15 eight-row guns and the other five guns' V1 rows (2026-10-31) [data] | 0x416B38 |
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
  - **Pickups** (0x30E7F8): see "Ammo pickups, crates and the vendor".
  - **The vendor** sells rounds at +0x84 bolts each, up to the maximum ("The vendor's
    menu").
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

## Ammo pickups, crates and the vendor [code, data, UYA NTSC, 2026-10-21]

Read in Ghidra on the Veldin image; names in `symbols.toml`. Nothing here is traced yet.
- **Two pickup classes** share `ammo_pickup_update` (0x30E7F8; the class table at
  0x2F6984 lists its users): 0xCDB, which crates throw out (`ammo_pickup_spawn`,
  0x30EF98), and 0xAE0, seemingly placed in levels (none on Veldin). The gun is a pvar
  (+0x68) and the rounds another (+0x60), not the class; a crate's pickup carries row
  +0x88's rounds of the gun's current level. A placed pickup's rounds come from the level's
  data, not read.
- **When one is taken** (`ammo_pickup_collect_test`, 0x30EDC0), with the hero alive:
  - only while its gun isn't full (row +0x8A); a pickup for a full gun stays where it lies,
    for good (pickups have no lifetime). Ownership isn't checked, but crates only throw out
    owned guns' ammo;
  - within `pickup_radius` across (8.7 u; 17.0 with the Bolt Grabber V2) and
    `pickup_height` up from his feet (3.0 u; 4.5), the pair bolts use too; the hero's
    update sets them every 4 frames (0x4FAA68);
  - not while still in the air from its throw: coming in range then arms a 3-frame timer,
    and it is taken on landing or bouncing, or at once if that timer still runs;
  - from anywhere, full or not, once it has fallen 2 u below where it was thrown.
- **Taken first, then flown in:** the rounds go in at once (`ammo_add`, capped), with row
  +0x36's message for one round or +0x34's for more, and a sound at most every 10 frames.
  Only then does it fly to him (state 2): from rest, gaining 0.00667 u a frame each frame
  up to 1.0667 (64 u/s), at his middle 0.7 u above his feet, carried along with his own
  movement, shrinking away over its last 30 frames.
- **Crates** (`crate_update` 0x33A5C0, `crate_drop` 0x33B940; classes as libuya names
  them): 0x1F4 bolts, 0x1F5 and 0x200 nanotech, 0x1F6 only broken by explosions, 0x1F7 a
  bolt and XP doubler, 0x1F8 the wrench only, 0x1F9 explosive, 0x1FA another object,
  0x1FF ammo. Any hit with damage breaks one (the queue's pending record), the wrench's or
  a gun's alike; so does one landing on him. Veldin has 131 bolt crates (11, 44 or 61
  bolts in moby +0xB4), 14 nanotech and 41 ammo [data].
- **An ammo crate** (`ammo_crate_pick`, 0x440E98) throws out 1 pickup (4 times in 5) or 2,
  for a random owned gun that uses ammo, chosen among those not full if any are, else
  among all. Never the Shield Charger, Agents of Doom, **Flux Rifle** or RY3NO
  (`ammo_crate_gun_allowed`, 0x440E38). With no such gun, nothing.
- **A bolt crate** pays out through `moby_killed`: its bolts times a share by how often
  that crate was broken before (100, 40, 20, then 10%; 0x1D60C8), doubled per
  `bolt_multiplier_exp`, randomised by a quarter either way (±1 under 8), split into coins
  of 30000 down to 1 by `bolts_drop`, at most 2 or 3 coins in a quiet scene, the rest
  banked for later kills ("The bolt bank", below).
- **The explosive crate** (0x1F9, `crate_update` 0x33A5C0) [code, 2026-10-29]:
  - touched (the moby the hero stands on, 0x1A4E70, or runs into, 0x1A4E6C, 0x1A4F3C,
    0x1A5118, or a record flagged 0x2000000), its fuse is lit at 179 frames (0xB3); while
    he grinds (hero type 15) it blows at once;
  - hit with damage, its fuse is lit at rand(5, 15) frames, or 1 for a hit flagged
    0x20000 (an explosion's); a hit while 16 or more are left cuts it to rand(5, 15);
  - it goes off through `crate_break` with its blast flag, which also sets off the shared
    explosion at its middle (`explosion_spawn(1.0, 0, 0, 2.0, …)`, `effects.md`) and calls
    0x348FF0(5.0, …) (not read);
  - lit, it beeps (its class's sound 1) when the frames left % 60 is 57, and at 13; it is
    tinted (200, 64, 64) (0x438908) while the frames left % 60 are 41 or more, or 14 or
    fewer, and its own colour otherwise; at 1 left it breaks (`crate_break` with its
    blast);
  - the blast (state 3, 21 frames, f = frame / 20 from 0 to 1): every moby within 2f of
    its middle takes 2.0 flagged 0x30001 (so other explosive crates go off a frame later),
    and the hero within 3f takes 4.0, pushed 1 out and 1 up (`0x4445E0`, flags 0x30000;
    the hero's damage, "hero-controller.md", "Getting hit and dying", scales it by his
    armor).
- **The nanotech crate** (0x1F5; 0x200 the purple kind) [code, 2026-10-29]: `crate_init`
  puts a holder (class 0x326, `nanotech_holder_update` 0x301F58) inside it with three
  amounts (the crate's pvars +0x134–0x13C, or the most health / 3, / 4 and / 5: 3, 2 and
  2 on a new game) and eight orbs (sprites, blue (0x40, 0x40, 0x7F) or purple (0x7F, 0x40,
  0x7F) for 0x200, size 60000) round its middle.
  - Only once the crate is broken (state 2) are they let out: when the hero within 10 u
    (`vec_distance`) has room, his health plus what orbs in flight carry (+0xB of the
    first holder) under the most (0x142668), and 0x301F00 agrees (not read).
  - Let out (state 3), each orb flies for rand_int(20, 80) frames (0x1DC4F8, 0x1DC4FC)
    on `cubic` (0x442098, a Hermite curve) from where it was to his middle, 0.75 u above
    his feet as he moves, with tangents (end − start) ± C: C is half the way from the
    holder to him plus 5 u (0x1DC504) square to that way, turned i × 0.4485 rad (25.7°)
    about it for orb i, so they fan out over half a turn and bow up to 1.25 u off the
    line; each throws sparkles (type 0x14) on the way.
  - The first to arrive heals him, once: the amount by the help measure (0x3FE4C0, 3 −
    measure / 100, at least 1: the most / 5 while the measure is under 100, / 4 under 200,
    / 3 from there), at least 1 (4 for 0x200), times 2 or a level's factor only in
    challenge mode; a sound (class 0x1F5's or 0x200's). The others just arrive. The
    holder goes 300 frames after letting them out.
  - The help measure (0x3FE420) is UYA's count of recent deaths, weighted: see "The help
    measure", below.
- **The doubler crate** (0x1F7, `crate_drop`) [code, 2026-10-29]: adds 1 to
  `bolt_multiplier_exp` (no cap) and 1800 frames to `bolt_multiplier_timer`, so a second
  stacks both, and shows a help message the first time (0x1406). The timer counts down
  each frame (0x1D6694 ≠ 4), beeping (class 0x1F7's sound 1) at 300, 240, 180, 120, 60
  and 1 frames left, and the exponent goes to 0 with it. `moby_killed` shifts both a
  kill's bolts and its XP left by the exponent, crates' bolts included.
- **The vendor** (`vendor_buy`, 0x4B8560) sells as many rounds as fit and the bolts buy at
  row +0x84 each; buy-all fills every owned gun with a round price.
- **Enemies never drop ammo:** `moby_killed` drops only bolts.
- **Each gun's** [data]: rounds per crate pickup (+0x88) and price a round (+0x84):
  Shock Blaster 4 and 5, N60 Storm 15 and 2, Annihilator 2 and 20, Flux Rifle 1 and 50
  (never from crates), Nitro Launcher 1 and 50, Lava Gun 15 and 3 (1 at V1). The kit's
  maximum ammo matches all 48 rows.
- Also seen [code, low confidence]: the wrench moby in its state 10 sweeps up bolts and
  ammo within 2.7 u (`wrench_pickup_sweep`, 0x337E58).

### Their sounds [code, data, UYA NTSC, 2026-10-30]

Which entry of which class's sound table each plays (`moby_play_sound` 0x4A7200 on the
moby's own class, `class_sound_play` 0x4A72A8 by class; tables as `effects.md`, "Sounds",
reads them). Veldin's core has tables for every class here but 0x1F7, 0x1F9, 0x1FA,
0x200 and the coins of 500 and up [data, `openrac-tool sounds --level 1`]; level 4's has
0x1F7 and 0x1F9 (both placed there, `openrac-tool mobys --class`) [data].

| What | Where | Sound |
|---|---|---|
| A crate breaks | `crate_break` 0x33BE30 | its class's 0; the ammo crate (0x1FF) its 1, 0x1FA the bolt crate's (0x1F4) 0. Not within a frame of the last crate's (0x1E1914). |
| An explosive crate's fuse beeps | `crate_update` 0x33A5C0 | its class's 1, with frames left % 60 = 57, and at 13 |
| The doubler runs down | ("The doubler crate", above) | class 0x1F7's 1, at 300, 240, 180, 120, 60 and 1 frames left |
| A bolt is collected | `bolt_coin_update` 0x2F7930 | its class's 0: 0xD under 20 bolts, 0xF under 50, 0x10 under 100, 0x11 under 500, then 0x12, 0x13, 0x1D5C, 0x1D5D, 0x1BA5 (`bolt_coin_spawn` 0x2F8F60); each its own sound |
| Ammo is taken | `ammo_pickup_update` 0x30E7F8 | its class's 0 (0xCDB from crates), not within 11 frames of the last (0x1E17A8) |
| A nanotech orb arrives | 0x301F58 | class 0x1F5's 1, or 0x200's for the purple kind |
| The vendor's menu | 0x4B7948 to 0x4BA3E0 | the vendor's class, 0xB: 3 opening; 1 moving between rows; 2 refused; 1 as the count of rounds steps (when it changes, and only on held frames that are multiples of 4 from 16); 0 backing out, or too few bolts on buying; 7 bought; 5 closing |

### The bolt bank [code, UYA NTSC, 2026-10-30]

Read for the kit (PLAN P1.10) in Veldin's build; decompiles in `cache/ghidra/out/bank/`.
Bolts that don't drop as coins go into `bolt_bank` (0x14269C) and come back with later
kills, so few are lost.

- **Every kill pays from it:** `moby_killed` (0x4412D0), unless flagged 0x1000, takes
  (bank + 49) / 50 out of the bank, a fiftieth rounded up, and adds it to the kill's
  bolts after the multiplier: bolts × (challenge multiplier `<<` `bolt_multiplier_exp`)
  + the share. That total is randomised by a quarter either way (±1 from 2 to 7, none
  under 2) and handed to `bolts_drop` with flag 8 and the kill's coin cap.
- **`bolts_drop`** (0x44A0E8; args z, moby, min, max, flags, −1, coins) picks the
  amount at random in min..max (min at least 1). With flag 8 it drops at most the cap's
  coins, the largest that fit first (30000, 10000, 3000, 1000, 500, 100, 50, 20, 5, then
  1), and adds what is left to the bank, divided by the challenge multiplier
  (0x1426BA + 1) so the bank holds unmultiplied bolts. Once the cap is spent the rest is
  banked. The coins of 1 are the exception: as many as fit are dropped, but the
  remainder isn't reduced by them, so they are banked as well (up to 4 bolts more than
  were paid; checked in the listing, 0x44A54C–0x44A584). Without flag 8 every bolt drops.
- **The crate's cap** (`crate_drop`, 0x33B940) is 0 or 1 at random plus 2 while
  `crate_debris_recent` (0x1DCFF0) is 10 or under, 1 from 11 to 20, else 0, so it limits
  coins while the scene is busy. Enemies pass their own caps (one passes 3). Every crate
  is counted, explosive ones as they go off, in `crate_break` (0x33BE30), which
  `crate_update` calls just before `crate_drop`, so a crate's own count is in its cap
  [code, 2026-10-31; decompiles in `cache/ghidra/out/bank/crates.c` and `ammo/a4.c`]:
  - first the count falls: by (frames since the last crate broke) / 4 toward 0 (0x445110
    steps an int toward a target by at most a step), or to 0 when more than 240 frames
    have passed; the frame is taken anew whenever it differs, so the part step is lost
    and crates broken 3 frames apart never let it fall;
  - then it rises by 4 while `moby_free_slots` (0x1DFFC0) is 251 or more and the count
    is 4 or under, else 3 with 201 free and 9 or under, else 2 with 151 and 14 or under,
    else 1 with 101 and under 20, else 0; so breaking crates alone never takes it past
    20, and the cap's 0 is never reached that way. Veldin had 362 slots free in both
    savestates read [data].
  - The rise sizes the crate's debris: bolt and ammo crates throw 1, 5 or 37 pieces for
    a rise of 1, 2 or 3 and up; nanotech crates (0x1F5, 0x200) 2^(rise − 1) − 1 (0, 1,
    3 or 7); the others none (the spawner 0x44ED60 not read).
- **Coins out of the way are banked too:** a coin that would spawn 64 u or more from the
  camera (0x1E2A40) is banked instead (0x2F8F60), and a coin in its state 3 more than
  32 u from the hero is banked and removed (`bolt_coin_update` 0x2F7930; it also tests
  the coin's place in the moby list against 0x1E00A0, not read).
- **Bolts per active minute:** for drops under 500 bolts, while 0x1D6714 is set and the
  drop isn't flagged 0x20, `bolts_drop` scales the amount by the level's bolts per
  active minute, r = 0x1A9280[level] × 3600 / 0x1A91E8[level] (halved if any of
  0x142692–0x142693 is set): × 5 under 181; × (380 − r) / 40 from 181 to 299 (capped at
  1000 bolts, 2000 if 0x142693 is set); unchanged from 300 to 600; × (720 − r) / 160
  from 601 to 700 and ÷ 10 above (at least 1). 0x1A9280 counts bolts collected
  (`bolt_pickup_collect` 0x2F8918) and 0x1A91E8 frames played (0x4B5258, not while the pad
  has been untouched for 900 frames), both only while 0x1D6714 is set; `bolts_drop` sets
  0x1D6714 from bit 7 of 0x1DFF00[the killed moby's +0xB0] for classes outside 500–540.
  What +0xB0 indexes isn't read; in the savestates read it is 0 and both counts are 0.
- **The challenge multiplier** (0x1426BA, 0 to 19, the bolt multiplier less one):
  0x4410E8, in challenge mode, counts kills in 0x1426BB and raises it once they reach
  half the multiplier.
- **An XP bank alongside:** 0x142698 pays out the same way, a fiftieth rounded up a
  kill, into the gun's XP; 0x3177F8 (a trigger whose class isn't read) adds rewards from
  the tables 0x1EFD00 (XP) and 0x1EFDC0 (bolts) to both banks, once per entry.

### The vendor's menu [code, UYA NTSC, 2026-10-30]

Read for the kit's vendor (PLAN P1.10) in Veldin's build; decompiles in
`cache/ghidra/out/vendor/`.

- **Opening** (`vendor_open`, 0x4B7948, called by the vendor moby's code at 0x456C70):
  the vendor has four modes in 0x244790, 0 weapons, 2 ship upgrades, 3 armor and 5 skins,
  chosen by its kind or the moby's pvars; kind 4 is weapons with rounds priced at row
  +0x86 (`vendor_round_mode`). It sets game mode 5, so the world stops while it is open,
  and puts its own camera in front of the vendor.
- **The weapons list** (0x4B6E88, rebuilt after each purchase) fills 0x244880, 0x14 bytes
  a row (0x4B6D60: +0x00 the item, +0x04 the kind, 1 rounds and 0 an item, +0x0C text
  0xCDB, +0x11 the Mega flag; 0x4B6DA8 writes the two special rows, setting +0x12 on the
  first and +0x13 on the last), from the vendor's
  stock: 32 halfwords at 0x1D54D8, each an item (low 14 bits) with bit 15 set if owned and
  bit 14 set if not yet for sale. In order:
  - a **rounds row** for each owned gun that has a price a round (+0x84) and isn't full
    (ammo 0x1427F0 under the maximum, +0x8A); kind 4 leaves out items 0x10, 0x11, 0x13,
    0x15 and 0x16;
  - the first special row (item 0, kind 1, +0x12 set; what it does isn't read);
  - an **item row** for each gun for sale, not owned and without bit 14; items 0x10,
    0x11, 0x13, 0x15 and 0x16 only when 0x417EA0 holds (taken to be challenge mode), and
    then also a **Mega row** (+0x11 set) for each owned gun at level 4 except item
    0x97;
  - unless a condition on 0x244EBC and 0x1D545C holds, the last special row (item 1,
    kind 0, +0x13 set; not read).
  The selection stays on the item it was on when the list is rebuilt.
- **The per-frame menu** (0x4BA3E0, while 0x244740 is 1) counts how long Left and Right
  are held (0x1E1008, 0x1E100C), runs the list's buttons (0x4B9A80 for weapons, armor and
  ship upgrades), the confirm screen (`vendor_confirm`, 0x4B9910) and the vendor's lines
  (0x4BE2F8), then draws. Buttons are the game's mask (libuya's pad bits byte-swapped:
  Left 0x8000, Right 0x2000, Cross 0x40, Triangle 0x10, Square 0x80), new presses from the
  pad's +0x1C4 and held ones from +0x1C0.
- **Moving between rows:** Right selects the next row and Left the one before (0x2447C0),
  with no wrap at either end. Held, they step again once the held count is past 15 and is
  1 modulo 10, that is on the 21st, 31st, 41st… frame. Each step also slides the row strip
  by 0x38 (0x2447AC) and won't step while it is still more than 0x38 off.
- **Cross** (newly pressed, not held from the last frame) is refused with sound 2 unless
  the row can be bought:
  - an item or Mega row when its price is at most the bolts (0x142660);
  - a rounds row when the gun isn't full and the bolts cover one round (+0x84);
  - the buy-all row when its total is above 0 and at most the bolts.
  Otherwise it sets 0x2447C4, the confirm stage, which stops the rows from moving.
- **Triangle** closes the menu (0x4B95D8). Square, when 0x1D545C is 3, closes it too and
  keeps the row's item at 0x244E9C (not read further).
- **Buying rounds** (`vendor_buy` states 2 and 4): a count from 1 that Left lowers and
  Right raises, up to the room left or what the bolts buy at the price a round, whichever
  is less; held, it steps again every 8 frames after 16 and every frame after 48, ticking
  a sound every 4 frames. Cross buys: price × count off the bolts (only for the rounds
  that fit), the list is rebuilt and message 0x17A shows; with too few bolts, message
  0x17B instead. Triangle backs out.
- **Buy all** (the first special row): its total, worked out each frame it is shown
  (0x4BEED8), is the sum over every owned gun with a price a round of (maximum − ammo) ×
  that price (+0x86 instead in `vendor_round_mode`, which leaves out the five items above).
  Cross fills them all for that total.
- **Buying an item or its Mega upgrade:** priced by 0x4B6C98: the gun's row +0x7C, or for a
  Mega row its level-5 row's +0x7C; 9/10 of that when 0x244EBC is set (from bit 0 of
  0x1426A8 by 0x4BBD58; what sets that isn't read). Given with `item_give` or
  `weapon_buy_mega` (0x4176F8), then the list is rebuilt.
- **The screen** (0x4BEED8) shows the row's name, description and price, the bolts, and
  greys the price when the bolts fall short; a rounds row shows ammo / maximum.
- Not read: the last special row and the vendor's lines (0x4BE2F8, on 0x244E84), what the
  vendor moby (0x456C70) needs to open, and the screens' layout.

### The help measure [code, UYA NTSC, 2026-10-30]

UYA eases a level where the hero keeps dying: the nanotech crate's amount and the hero's
damage (`hero-controller.md`, "Damage") depend on `help_measure` (0x3FE420). Decompiles
in `cache/ghidra/out/help/`.

- **Deaths by area:** `area_deaths` (0x1C5290) holds an s32 for each of 16 areas of each
  level (0x40 bytes a level). `hero_death_done` (0x4EA040) adds 1 to the level's deaths
  (0x143D70 + level × 4) at every death, and 1 to the area the hero is in (0x1A71D7, 0xFF
  for none) only from the level's 900th frame (1800th after falling out of the level,
  state 121, or in 126) and where the area's flags (0x1BBC40 + level × 16 + area) lack
  0x80.
- **The measure** of a level and area is 0 in challenge mode (0x142692), from level 30 or
  for area 16 and up; otherwise the sum over 10 records (0x2C01A0 + level × 0x280 + area ×
  0x28; each a u8 level, a u8 area and an s16 weight) of weight × that area's deaths. So
  an area can weigh deaths in its neighbours too.
- **Checkpoints** (0x325A60, its moby's +0xB0 the area): reaching one other than the last
  (0x1DCA54) takes 100 / its area's first weight, rounded up, from that area's deaths
  (0x3FE520), at least 0, and makes it the hero's area.
- In Veldin's savestate [data] area 0 has one record, its own deaths × 50, and the
  others none, so every death there adds 50; area 1's flags have 0x80, so deaths there
  aren't counted. The nanotech crate then gives his most / 4 after 2 deaths and / 3
  after 4, and from the 7th death his damage falls by 7% for each past 6, at most by
  half.

## Mods [code for the flags; text for what they do]

`item_flags` holds three mods per gun, set by the gun's own code from its level each frame:

| Bit | Mod | What it does |
|---|---|---|
| 0x20 | lock-on | L2 and R2 together lock on (`hero-controller.md`, "Lock-on") |
| 0x02 | shock | each hit also arcs to an enemy near the target (0x34BF98; below) |
| 0x08 | acid | hits leave acid that does damage over time (0x31AC38; below) |

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

### What shock and acid do [code, data, UYA NTSC, 2026-10-20]

Both take (target, item, attacker) and run only when the item's mod bit is set (the item,
or the item its row +0x48 names, so 0xA0 and 0xA1 stand for the Plasma Coil and Lava
Gun); both affect only living enemies (class category 5, targetable), each target at
most once at a time, in 30 slots. Their numbers come from per-item tables in two banks:
**bank 1 while the gun is at V3 or V4, bank 0 otherwise**. The level start fills them
(0x34C438, 0x31AF48), with every other item zero.
- **Acid** (`weapon_acid_mod`, 0x31AC38; slots at 0x274600, ticked by 0x31A920):
  - each frame the target loses `damage per second / 60` straight off its health, past
    the damage queue; the hit that takes it to 0 goes through the queue as a kill credited
    to the attacker; it ends after the duration, when the target dies, or when it loses
    its damage record. It tints the target green, pulsing, and drips (0x31B0A0);
  - the table (damage per second, seconds):

    | Gun | Bank 0 | Bank 1 (V3, V4) |
    |---|---|---|
    | Plasma Coil (0x10, 0xA0) | 2, 4 | 5, 4 |
    | Lava Gun (0x11, 0xA1) | 2, 4 | 3, 6 |
    | Bouncer (0x13, 0xA6) | 2, 4 | 3, 6 |

  - **The Nitro Launcher has no entry:** its blast passes item 0x77, whose row is zero,
    so its acid lasts no time and does nothing, as read in Veldin's build.
- **Shock** (`weapon_shock_mod`, 0x34BF98; slots at 0x27F600, ticked by 0x34BA90):
  - on the hit the target loses the table's first damage at once (a kill goes through the
    queue); then, from that frame and every interval (1 s) for the duration, an arc jumps
    from it to the first other living enemy found within 8 u (not class 0x1AE6) for the
    arc's damage (0x34C5F8). With no duration the slot ends after its first frame: one
    arc per hit;
  - **the arc** (`shock_arc_add` 0x34C5F8, `shock_arcs_update` 0x34C870; 48 slots of 0xD0
    at 0x27FBA0; 2026-10-22): its target is the first entry of `ball_damage`'s 8 u list,
    in the list's order rather than the nearest, that is targetable, alive, in sight, an
    enemy (category 5, not class 0x1AE6) with health left, skipping the source. Two arcs
    go, one carrying the damage and one for show, each reaching the target over 10
    frames; when the reach ends the target, if it still has collision, takes the damage
    (flags 0x410000, pushed 1/30 u along the arc) and sparks (0x34D600), and the arcs
    stay 30 frames more. One frame in 11 a target crackles with a short arc to itself
    (reach 5–15, life 15–30). The Plasma Coil's arcs have the same form;
  - the arc's damage is the table's, but while the Shock Blaster, N60 Storm, Nitro
    Launcher, Miniturret Glove or Spitting Hydra is in his hand it is a quarter of that
    gun's current damage (+0x6C);
  - only the Miniturret Glove (0x15, 0xA2) has an entry: 1 at once, arcs of 2, for 3 s
    (bank 1: 2, 4, 3 s). So the Shock Blaster's and N60's shock is one arc per hit for a
    quarter of their damage.

## Switching and firing [code, UYA NTSC, 2026-10-16]

- **Buttons** (0x4E8830, from `hero_transitions`):
  - Square brings out the wrench (item 9) and remembers the gun (0x1A7210).
  - Circle brings out the equipped gun. R1 does too in the camera modes where R1 fires
    (0x50FF18).
  - With the wrench out, L2 and R2 together bring out the gun when its lock-on mod is on.
  - The gun fires on the button in 0x1A5DF0: 0x20 (Circle) for guns, 0x80 (Square) for
    the wrench.
- **Changing hands [traced, UYA NTSC and DL NTSC, 2026-10-24]:** the hand's state (UYA
  0x1A5E04, DL hero+0x2314) is 2 with the item in hand, 3 while changing and 0 empty,
  beside the item (0x1A5E08, +0x2318) and the frames it has been held (0x1A5E00,
  +0x2310, which restarts at 3). Square with a gun out ran the state 3, 0, 3, 3, 2 and
  the item gun, 0, wrench, so the wrench was in hand 4 frames after the press and the
  swing started on the 5th (`uyantsc-veldin-fire-2` with the N60; four first swings and
  a jump attack in `dlntsc-dreadzone-melee-{1..5}` with the Dual Vipers). The wrench
  comes out faster than a gun's 17 frames ("Checked in traces").
- **Triangle tapped** (`hero_quick_switch_button`, 0x503668): released within 20 frames
  of its press, it asks for a switch (0x1A74A0), which `hero_quick_switch` (0x5037C0)
  makes: with the wrench out, to the equipped gun; otherwise to the next of the three
  most recently chosen guns (`recent_guns`, 0x142670, eight items, most recent first,
  turned round by one each tap), skipping guns out of ammo. Not during the wrench's
  combo, jump attack or throw (states 19, 20, 21), nor states 55 and 136. Held longer,
  Triangle is the quick-select ring's. `recent_guns_push` (0x503930) moves an
  item to the front (never the wrench or items 0x16, 0x1E, 0x23), dropping an empty gun
  from the first three when it comes in from further back.
- **Animation** (0x4E6A08, called as a gun fires): see "The hero's side of firing".
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
- **The push** [code, 2026-10-31]: `damage_info_fill` copies a vector into the record's
  +0x00–+0x0C (libuya's `COLL_DAM_IN.momentum`) and sets +0x20; the queue keeps it
  (ring +0x10) and marks it (ring +0x30) when it is longer than 0.0001. Each gun's is a
  direction times a strength, in u (the N60's from its velocity a frame):
  - the Shock Blaster's rays 1.5 along each ray, its barrel ball 1.75 along the barrel;
    the N60's bullet its velocity a frame × 0.5 (0.5–0.67); the Flux shot 1 along its
    flight; the Annihilator's missile 1.5 along its flight; the Miniturret's shot 1 along
    it; the Shield Charger's zap 1 from the shield to the enemy's middle; the lightning
    bolt (the Tesla Barrier's, 0x34DFD0's default) 1 from its start to its target; the
    Hydra's strike and bolt 0.5 from the gun; the small missile its velocity;
  - `ball_damage_pushed` (0x443648, the Flux Rifle's muzzle ball among others) pushes by
    (cos yaw, sin yaw, 1) of the attacker × its strength: forward and 1 up.
  - **Enemies** that use the shared hit handler (`enemy_take_hit`, 0x44FCD0, and
    0x4503F8; which do isn't read) pick a reaction 0–6 by the hit's damage against
    shares of their most health. A flinch (1) ignores the push; for 2–6 the push's
    ground length is raised to at least 1 and its height to at least 1, the knock-back
    is (x H, y H, z V) with H and V the enemy's own speeds for the reaction, and the enemy
    turns to face back along it. No push is taken as its own facing reversed, height 1.
    So any push of 1 or less knocks the same, and only more knocks further.

## The quick-select ring [code, data, UYA NTSC, 2026-10-21]

Single player only: multiplayer is another program, not read.
- **Tap or hold:** a Triangle press sets a 20-frame timer (`quick_switch_tap_timer`);
  letting go while it runs is the quick switch. `hero_quick_switch` clears it and the
  "tapped lately" timer when it changes guns.
- **Pausing is an option**, QUICK SELECT PAUSE (`quick_select_pause`, 0x143A06, on in our
  image):
  - **On:** Triangle held 2 frames starts a hold check (`quick_select_hold_update`) that
    counts 4 more; still held, the game switches to mode 4 (sub-state 7), where the world
    doesn't update: the ring is open about 7 frames after the press. Not within 8 frames
    of a mode change, nor in hero states 0x1C, 0x31, 0x39, 0x58 and 0x74. The ring stays at
    least 6 frames; the tap check keeps running, so letting go early still quick switches.
  - **Off:** the ring opens over the world on the press, the world runs on, and after 15
    frames (27 just after a tap) the hero gets no pad input while it stays up. Picking
    starts with the same hold check.
- **Slots:** eight, from ring 1 (0x1D4C60) or ring 2 (0x1D4C80); slot 0 at the top, then
  clockwise every 45°. Holding R1 shows ring 2, and for 14 frames after letting go.
- **Picking:** nothing is selected when it opens, and a stick already pushed is ignored
  until it drops under 0.5, a d-pad button is pressed or 30 frames pass, which also
  clears the selection (`quick_select_ring_update`, 2026-10-22). Then the left stick past 0.9 of full picks
  the slot it points to, and keeps the slot picked within 33.75° of its centre; the
  cardinal slots are 27° wide and the diagonals 63°; pointing at an empty slot takes the
  filled neighbour the stick leans to, if any. Between the dead zone and 0.9 nothing
  changes. With the stick short of 0.9, the d-pad steps between slots by a neighbour
  table (first press: left 6, right 2, up 0, down 4; then by slot 0–7, left/right/up/down:
  7/1/0/4, 0/2/0/2, 6/2/1/3, 4/2/2/4, 5/3/0/4, 6/4/6/4, 6/2/7/5, 6/0/0/6, `quick_select_slots`
  +0x08–0x14 in Veldin's image [data, 2026-10-22]). Picking needs the hold check passed
  and Triangle held, so the selection stays put while the ring closes.
- **Choosing:** only letting go of Triangle chooses, up to 8 frames later as the ring
  closes: its opening counter (+0x70) climbs to 8 while Triangle is held, falls a frame at
  a time after, and the choice comes when it reaches 0 (at once if it is 0); X and Circle
  do nothing. Off, it opens on the press with the input delay of 15, or 27 while
  `quick_switch_recent` runs (20 frames from a tap, cleared by a switch made). A gun is asked for in his hand (`hand_item_request`),
  so `hero_item_switch` brings it out and makes it the equipped gun, and it goes to the
  front of the recent guns; the camera options (items 0x24–0x26) set the camera instead.
  Nothing selected chooses nothing. A gun with no ammo can be chosen; the ring shows its
  count in red.
- **It shows**, for the selected slot: the tier's name (row +0x44), V1–V8 (the game labels
  Mega, Giga and Omega V6–V8 here), the ammo (red at 0) and an XP bar. While open, it
  keeps the health and bolt counters up.

## The HUD [code, UYA NTSC, 2026-10-21]

- **The gun** (`hud_weapon_fill`, 0x475078): shown while a hand item other than the
  wrench is out, and for 10 frames after; no pop-up on firing, switching or pickups. The
  gun's icon (row +0x38) at the top left, the next two of the recent guns smaller and
  fainter beneath, its ammo as current/maximum, its V label and an XP bar from the last
  threshold to the next (full at the end). No name. Under 15% of the maximum the ammo
  pulses dark red, every 40 frames.
- **Health** (`hud_health_draw`, top centre): digits with "/" and the maximum, a bar, and
  the hero's EXP bar. Shown for 720 frames after a kill, 360 after damage, nanotech, more
  maximum health, L3 or the ring; under 15% it pulses red and stays up.
- **Bolts** (top right): shown for 180 frames after a pickup or tally, L3 or the ring.
  The count trails the real one: changes gather in a +N or −N beside it (fading in over 15
  frames), and 61 frames after the last change roll in over 15 frames, 1/14 a frame,
  then snap to the total. The doubler shows as "x2" and up.
- **The doubler's x2** (`hud_bolts_draw` 0x475B38, its pulse 0x476258, the layout 0x4753A8)
  [code, 2026-10-31; decompiles in `cache/ghidra/out/dbl/`]:
  - "x%d" of (challenge multiplier + 1) << `bolt_multiplier_exp`, shown while that is 2
    or more, as element 0x10007 inside the bolts' group (0x10000), so only while the
    bolts show; at (0.925, 0.15) of the screen, just under the count (0.885, 0.06),
    scale 1.5.
  - While `bolt_multiplier_timer` runs, the hero's update (0x4FAA68) shows the bolts for
    180 frames every frame, so the count and x2 stay up for the whole doubling and 3 s
    after.
  - At rest it is (255, 213, 127) with alpha 0x70 at scale 1.5. Round each beep but the
    last (300, 240, 180, 120 and 60 frames left) it pulses: from 2 frames before the
    beep, its green and blue go up 30 and its scale 0.1 each frame for 2 frames, down for
    6, up for 3, then snap back (scale 1.6, 1.7, 1.6, 1.5 … 1.1, then 1.2, 1.3, 1.4).
    Over its last 4 frames its scale drops 0.3 a frame (1.2 to 0.3).
  - The colour is packed (B << 16 | G << 8 | R) from the values before each frame's step,
    so it is a frame behind the scale, and the green's 273 at a pulse's top spills into
    the blue: on each beep's frame it shows (255, 17, 187) for one frame.
  - The beep (class 0x1F7's sound 1) is skipped while 0x417EA0 is set (the challenge
    check `moby_killed` uses too).
- **Help messages** (`help_show` 0x417F60, `help_update` 0x418478, `help_open` 0x4182C8,
  `help_draw` 0x418F00 not read) [code, 2026-10-31]:
  - `help_show(text, slot)` asks for a text from the help table (16-byte entries: text,
    id, voice line or −1, a follow-up id) unless one is already up or asked for, three
    other screen states are set (0x1CD03C, 0x1CCFF4, 0x1CD064; not read), the slot's
    count is −1, the game is in state 6 (0x1D6694), or within the level's first 6
    frames; it returns whether it was taken. Each slot counts how often it was shown
    (`0x1A7E18 + slot × 24`, a halfword) and when.
  - Shown for (the text's length × 7) frames after opening over 8 (and a frame and 4
    more), then closed over 8; Triangle (pad bit 0x10) skips to its closing. Voiced
    ones (sound 7000 + their voice id) wait for the line too. Its box is placed 60 lines
    above the screen's bottom, and drawn with (255, 204, 102) (0x4182C8).
  - The doubler crate asks for 0x1406 in slot 0x108 once a game: its flag (0x1B6E60) is
    set whether or not the message was taken. The wrench-only crate (0x1F8) asks for
    0x1405 in 0x107 while 0x1A8A6C is clear, and the 0x1FA crate 0x1409 in 0x10B while
    0x1A8A9C is.
- **Levelling up** (`xp_level_check`, 0x50F920): a gun not in hand levels up at once; one
  in hand, with him on the ground, plays the upgrade sequence with the world stopped
  (about 200 frames; `weapon_upgrade_start`), and his input stays locked 25 frames more.
  At most one level-up every 60 frames. The message shows 180 frames.

## Aim indicators [code, data, UYA NTSC, 2026-10-22]

Every reticle, crosshair and target mark the guns draw. All are flat sprites drawn over
the frame (never hidden by the world), in the level's effect textures (`effects.md`). Sizes
are PS2 pixels on the 512 × 416 frame; colours are PS2 RGBA with R in the low byte and
0x80 meaning full.
- **The lock-on reticle** (`lock_reticle`, 0x3F4700, drawn by `reticle_markers_draw`
  0x3F4FE0 each frame except in hero state 0x74):
  - It shows while there is a lock-on target or its fade (0x1A6044) is above 0. The fade
    moves 0.1 a frame (6/s), so it fades in over 10 frames; on a change of target the old
    one (0x1A6040) fades out fully before the new one is taken and fades in.
  - Its spin (0x1A6048) turns 45°/s (0.75° a frame), twice that while fading in.
  - It sits on the target's position raised by r along the target's up axis, where r is
    its damage record's height (+0x10) at most 2.0, or 0.75 without a record, projected
    to the screen (`world_to_screen`, 0x3FE7B0). Its size is
    s = clamp(350 × atan(r / d), 16, 128) pixels, d from the camera (0x1D6008–0x1D6014).
  - Three rings, each one quarter texture drawn four times round the centre (each quarter
    s × s with its corner on the centre, so a ring is 2s across): texture 0x33 turning
    with the spin, shrinking from 2× to 1× as it fades in; 0x34 turning against it,
    growing from nothing; 0x35 fixed at 45°. The first two are the texture's colour at
    alpha up to 0.5 (0x40808080), the third tinted blue (0x30E02020: × 0.25, 0.25, 1.75)
    at alpha up to 0.375, all lerped in from transparent by the fade.
  - A health arc when the target's record has a maximum (s16 at +4): `hud_arc`
    (0x404F68), 16 quads of texture 5 (a soft dot), between radii 12s/16 and 14s/16,
    centred below the target and spanning 2 × (2 atan(max / 6)) (a lower half circle at 6
    hit points, nearly a full ring for many): red 0x60000080 behind, green 0x60008000
    over the share of health left, fading with the reticle.
- **Markers** (`reticle_marker_add`, 0x3F4D78): the guns queue up to 14 sprites a frame
  (0x1E5200, cleared each frame by 0x3F4D68), each a world point (projected each frame) or
  a screen point, a size, an angle, a colour with alpha at most 0x40, and a texture; drawn
  after the lock-on reticle, skipping dead owners. While the world is stopped
  (0x1D6698) they stay, shrinking 0.75 a frame.
- **The crosshair's colour** (`crosshair_colour`, 0x4C7588, with a threshold and 150°/s):
  it fades in over about 13 frames (alpha 5 a frame to 20 counts, under the 0x40 cap),
  and pulses between green (0x0F, 0xFF, 0x00) and white by 0.5 + 0.5 sin, the phase
  moving 150°/s (a 2.4 s pulse). It is solid red (0x600000FF) while the free-look
  camera's target weight (+0xB0, 0x1A61D0; 1 for a target its ray hits, else
  (cos θ − cos 15°) / (1 − cos 15°) for one θ off the view) is above the gun's threshold:
  0.95 is about 3.3° off, 0.97 2.6°, 0.98 2.1°, 0.99 1.5°. A gun resets the fade and
  pulse on each frame it draws no crosshair.
- **The shared target mark:** texture 0x26 (a ring with four ticks), 40 × 40 px ×
  |0.9 − d / 150 × 0.65| (36 px close, 10 px at 150 u), green 0xFF0FFF0F at the 0x40
  cap, turning against its own spin of 2° a frame, on the target raised by its record's
  height (0x4C74C0).
- **Each gun** ("aiming" is state 29 or a camera mode other than follow):

  | Gun | Crosshair at the screen's centre (texture, size) | Red above | Target mark |
  |---|---|---|---|
  | Shock Blaster (and V5) | 0x5B, 30 × 55 turned 90° | 0.95 | none |
  | N60 Storm (and V5) | 0x56, 30 px; aiming without a lock-on | 0.99 | on its target or the lock-on target, in the follow camera outside state 29 |
  | Annihilator (and V5) | 0x44, 40 px; only in state 29 with the hero hidden (first person, guessed) | 0.97 | on its held target, follow camera, not with the arm raised |
  | Disc Blade Gun | 0x5A, 80 × 40 × 0.8 turned 180° | 0.98 | on its target, follow camera |
  | Suck Cannon | 0x44, 40 px | 0.97 | in 0x600FFF0F, follow camera, outside state 29 |
  | Rift Inducer | 0x21, 32 px; state 29 only | never | none |
  | Plasma Coil, Plasma Storm | 0x21, 32 px | never | the Coil's, on its target when ready to fire, any camera |
  | Qwack-O-Ray | none | – | on the beam's target, follow camera, outside state 29 |
  | RY3NO | none | – | V1–V4 without a lock-on: on each enemy its assist finds, one shared spin |
  | Spitting Hydra | none | – | its marks (`hydra_marks_draw`, 0x3213A8): texture 0x1F (a downward triangle) in green 0x7F40FF40, the newest in red-orange 0x7F2040FF, sized and turned as the shared mark, only those within 60° of the view (cos > 0.5) and not dying or hidden |
  | Flux Rifle | the scope, below | – | – |

  The Nitro Launcher, Lava Gun, Bouncer, Infector, Agents of Doom, Holo Shield,
  Miniturret, Shield Charger and Plasma Whip draw none: only the lock-on reticle. The
  wrench in the look draws 0x25 (40 px, blue-grey 0xFF917267, no pulse). The aim assist
  marks nothing itself; each gun's update queues its mark.
- **The Flux Rifle's scope** (0x3444B0): in the look or state 29 once the view is ready
  it hides the gun's moby and registers a draw callback (`flux_scope_draw`, 0x345D48):
  - the overlay: texture 0x46 (128², a dark surround, a pink-orange bezel and dotted
    centre lines) as four mirrored quarter-screen quads into the frame's alpha only, a
    shading quarter (0x42), then a copy of the frame drawn over the whole screen where
    that alpha allows (taken to be the lens);
  - the zoom (its target is `flux_scope_fov`, 0x1DD2F8): between the view's field of
    view (the gun's pvar +8, 1.11) and 20° at V1–V2 or 10° from V3. Each frame of the
    ready look it moves 0.01 rad, whatever the pressure:
    - in the third-person setting, in on R1's pressure (pad analog float +0x134) and
      out on R2's (+0x13C), the larger winning and R1 a tie (the floats follow the
      raw pad's order in libuya's `PAD`: sticks, then pressures from right to R2)
      [code; the buttons from libuya, MIT];
    - in the other settings, in on the left stick pushed up (minus +0x10C) and out
      pulled down.

    It is kept between the view's and the narrowest, and asks the camera to spring to
    it (`camera_fov_set`, gain 0.02, damping 0.2). Leaving the look sets the target a
    quarter of the way in for the next look and springs the camera back to the view's
    at gain 0.005 [code].
  - The free look's turn rates are × the camera's field of view over 1.1100293
    (`fps_cam_update`, 0x410B08), so a zoom slows the turn with it.
  - The zoom also lerps four floats at 0x1DF720–0x1DF738 by its share. At rest they hold
    0.02618 (1.5°), 0.03, 0.4 and 0, and at full zoom 0.010472 (0.6°), 0.03, 0.4 and the
    fourth's own value, so only the first changes. Nothing else in Veldin's loaded code
    reads or addresses them (a search of every load, store and address formed), so what
    they drive isn't known; the earlier "turn cap" reading of them is unconfirmed [code].
  - The scope is dropped, and the camera sent back out, in hero state 0x74 (held for a
    path camera's flight, `camera.md`), while 0x1D6694 is set, or when 0x4B6C68 says so
    [code; the last two not read].
  - the on-target ring: on 4 frames of every 8 (bit 2 of the frame count) a line from the
    camera 128 u along its view (0x3F8448) takes the first thing it meets. It is on
    target if that is a live enemy (category 5, not class 0x19C0 or 0x1AE6), or one
    0x447CA0 marks. While it is, the look's 40-frame aim wait (0x1A749C) is over, the shot
    is ready and there is ammo, texture 0x20 (a segmented ring), 32 px, red 0x200000FF,
    blinks on for 15 of every 31 frames: a count from 0 to 30 that moves only while on
    target, drawn from 16 [code].
- **How they are drawn:** `fx_texture` (0x401CD0) gives a texture's GS registers by its
  number in the effect table, uploading it on its first use in a frame (negative numbers
  are copies of the frame); `hud_sprite` (0x404740) draws a screen-space textured,
  alpha-blended strip with a pivot and flips. The reticles' images are 64² 8-bit, but 5
  (32²), 0x46 (128²) and 0x42 (256²).
- **Clank's ring:** 0x41BE10 (init, with update 0x41BE58 and draw 0x41C340) is a
  four-slot ring the game opens in place of the quick-select ring while he plays Clank
  (0x1A71C4 = 1), picked by the right stick or the d-pad's neighbour table; not an aim
  indicator, and what its choices do isn't read.

## The hero's side of firing [code, UYA NTSC, 2026-10-20]

Read in `hero_fire_anim` (0x4E6A08), the per-frame gun update `hero_gun_update`
(0x4E6D40, from 0x4E7118), `hero_transitions`, the walk's turn and the head-look
(0x4E2F50). Nothing here is traced yet.
- **Two flags:** each gun calls `hero_fire_anim` as it fires, which sets `firing`
  (0x1A71C9) and copies row +0x30 into `firing_held` (0x1A71CB). +0x30 is set for the
  guns fired by holding the button: the N60 Storm, Infector, Suck Cannon, Qwack-O-Ray,
  Plasma Coil, Lava Gun and Shield Charger [data]. `firing` clears when the firing
  animation ends, at once when a held gun runs dry (`hero_fire_stop`, 0x4E6CB0), and when
  any other animation is set while `firing_held` is clear (`hero_anim_set`). With
  `firing_held` set, neither the animation's end nor another animation clears it:
  `hero_gun_update` calls `hero_fire_stop` once the fire button (0x50DA08) is let go, so
  a held gun fires until then however far apart its shots (2026-10-31).
- **When a gun may fire** [code, 2026-10-31]: the guns don't test his state; they refuse
  while the no-firing flag (0x1A71CD) is set (the Shock Blaster, N60, Shield Charger and
  others test it beside the held frames, "When a drawn gun may fire"). Its writers
  (`RefsTo`):
  - `hero_change_state` clears it on every change, then sets it for states 23–26 (the
    ledge grab, hang and both shimmies), 28, 36, 37, 43, 49, 55, 101, 102, 116, 122,
    144, 145, 172 and 177 (the damaging floor's stagger);
  - the hand's update (0x4E9558, every frame) sets it, with 0x1A71C8, in state types 17
    and 18 (swimming under water and on the surface) and states 108 (drowning), 119,
    120, 132 and 176 (0x4E5540 is that test);
  - the state update sets it in 121 (falling out of the level); `hero_transitions` sets
    and clears it within a few states, and 0x4B7948 (a scripted scene) sets it.

  So a gun fires in the walk, jumps (the wall, water and ledge jumps too), glide, grind,
  zip line (118) and while knocked back (22), and the attacks fire none because the
  wrench takes the hand.
- **The firing animation:**
  - **Standing** (state type 0, the gun arm not raised, and not in the lock-strafe setting
    with the camera pitched more than 10°): the row's whole-body animation (+0x24) on the
    body. It blends in over 2 frames for the Shock Blaster, the Refractor and sequence 49,
    5 for the N60, else 11. In the idle, the look and state 29 a held gun keeps it
    playing while it fires (`hero_idle_sequence` returns it).
  - **How long it plays** [code, data, 2026-10-31]: `hero_anim_set` (0x50BB98) gives the
    first keyframe the blend's frames (its rate, 0x1A57E4, is 1 / the blend), and the
    rest play at their own keyframes' rates. Standing, a gun not held stays `firing`
    until its sequence reaches its last keyframe and the idle sets its own: 66 frames for
    sequence 49 (17 keyframes of 4 before the last), 92 for the Shock Blaster's 103 and
    101 for the Bouncer's (46 of 2), 111 for the gloves' 41 (26 of 4). A gun not held
    that repeats sooner (the Annihilator every 36 frames while Circle is held) restarts
    the sequence before it ends, so he never goes back to his idle between shots.
  - **Otherwise** (walking, in the air, crouched, the arm raised, or pitched in
    lock-strafe): the arms-only animation (+0x28; +0x2C in a crouch, state type 12) on
    the right arm's layer, and the left's too for a two-handed gun (+0x18 = 2), blending
    in at 0.2 of full weight a frame and out at 0.07 when it ends (0.25 when cut). Most
    guns have no arms-only animation (+0x28 = −1) and play nothing then; the gloves
    (Miniturret, Agents of Doom, Holo Shield) have one [data].
  - Whole-body sequences [data]: 49 for most two-handed guns, 51 for the one-handed
    (N60, Plasma Coil, Qwack-O-Ray, Shield Charger), 103 the Shock Blaster and Bouncer,
    46 the Suck Cannon, 41 the gloves; crouched 65 (two hands) or 57 (one).
- **Firing doesn't stop the hero, but it changes his walk:**
  - while `firing_held` is set, the walk keeps its setting-off turn (`hero_walk_turn`)
    instead of the moving one, and its speed doesn't rise while he is more than 45° from
    the stick, as for a part-pushed stick (`hero-controller.md`, "Skid");
  - while the Shock Blaster, N60, Plasma Coil or Lava Gun is firing (and he isn't
    skidding), that turn is gain 0.009 m, damping 0.15 and cap 0.0785 m rad a frame for a
    stick m ≤ 0.95, and 0.017, 0.18 and 0.1454 above;
  - in the lock-strafe camera, firing (or Circle or R1 in the last 2 frames) triples the
    turn's gain and cap and makes him strafe (`camera.md`, "The hero in these modes");
  - **no double jump while `firing` is set:** the jump's double-jump entry checks it.
- **The release skid is the wrench's:** the walk only arms its skid on letting go
  (`hero-controller.md`, "Skid", the hand-slot condition) with the wrench in hand
  (item 9), the stick fully pushed, and `firing_held` clear. With a gun he stops at
  once, as the DL trace with a gun in hand shows (`hero-controller.md`, "Letting go of the
  walk with a gun in hand").
- **The raised gun arm** (`gun_arm_raised`, 0x1A7454; libuya's `raisedGunArm`), each
  frame with a gun in hand (0x4E26F8): a ray 0.57 u up (0.7 while firing) from just inside
  his radius to 1.3 u ahead; if it hits a wall, the arm points up by
  `−70° × 1.25 × (1.3 − d) / 1.3` (d the hit's distance from his feet less his radius,
  0x1A4E64), clamped to
  −30° … −70°, and the flag is set. Guns then aim along the gun (the Annihilator) or
  delay the shot's hit to 5 frames (the Shock Blaster). In the lock-strafe camera the arm
  follows the camera's pitch instead (0x1A6180, clamped to −80° … 45°) and the flag
  stays clear.
- **The head and upper body turn to the gun's target** (`hero_head_look`, 0x4E2F50;
  "The hero's joint springs" has the joints): each frame the gun's update puts its
  target in `gun_target_now` (0x1A57B8; the lock-on target, or its own pick: the Shock
  Blaster, N60, Annihilator, Flux Rifle, Lava Gun, RY3NO, Rift Inducer, Plasma Coil and
  others set it), and without a lock-on target the upper chest turns to its yaw and the
  waist to its pitch on springs (gain 0.037, damping 0.3), yaw up to ±59°, pitch from
  25° up to 35° down. With a lock-on target he faces it by strafing anyway. With
  neither, the head looks at the nearest interesting thing within reach every 7th frame
  (the look-at list, 0x1A73DC).
- **The fire direction** (`hero_fire_dir`, 0x1A52A0; libuya's `HeroFireDir`: a matrix,
  the direction at 0x1A52D0 and angles at 0x1A52E0), set each frame by
  `hero_gun_update`: the hero's matrix and facing, and his angles with the yaw plus the
  upper-body twist (none while strafing or firing in lock-strafe). The matrix and
  direction don't take the twist (the code builds a twisted matrix and then overwrites it).
- **How guns aim with it:**
  - **At a target** (`aim_at_target`, 0x454298): straight from the muzzle to the
    target's position raised by its damage record's height (+0x10) along its up, times
    the speed. No limit on the turn.
  - **At the camera's aim point** (`aim_bend_to_point`, 0x4C7010): in the lock-strafe
    and first-person cameras, the N60's bullets, the Annihilator's missiles and others
    bend from the fire direction towards the free-look camera's aim point (0x1A6230,
    libuya's `aim_pos`) by at most 80° (a tenth of that when the point is within 4 u); the
    Flux Rifle does it scoped, within 20°. So shots converge on what the view centres on.
  - **The aim point** (`fps_cam_find_target`, 0x411918, first thing in each frame of
    the free-look camera, so in the lock-strafe and first-person cameras and the look;
    2026-10-22): a line from the camera's position along its forward, 30 u long (fps_cam
    +0x120), skipping one object (+0xBC); the point is where it hits, else its end. An
    enemy it hits (category 5, not classes 0x19C0 or 0x1AE6) becomes the camera's target
    (+0x8C) at weight 1; otherwise its target is the best living enemy in sight within
    30 u inside a cone of 15° × the zoom (+0x7C), scored distance × (1 + weight), +10
    for those 0x447CA0 accepts. The weight is what the crosshairs turn red by ("Aim
    indicators").
  - Otherwise along the fire direction.
- **The muzzle** [code, data, computed from the disc, checked in traces, 2026-10-31]: the
  gun moby rides hero joint 56 (attachment matrix 0, 0x1A5810; its columns are in every
  frame of our traces, the hero's region at +0xC50); in its frame −y runs along the barrel
  and +x is the gun's top. A shot leaves from a point of the gun's class
  (`moby_point_position`, 0x437698: the point's posed joint × the class's scale / 1024,
  turned by the gun's matrix, plus its place), so the muzzle follows the hand through the
  firing pose.
  - **Which point:** 0 for most (the arguments at the Shock Blaster's, Annihilator's,
    Nitro's, Lava Gun's, Hydra's, Suck Cannon's, Coil's, gloves' `moby_point_position`
    calls), 1 for the Flux Rifle. Each class's point, read from its class in the gadget
    WAD, is in `uya_weapons.ron`.
  - **Vectors of the guns' own,** taken through the gun's matrix (0x40C2D0), not points
    (up, along the barrel, to his right; 2026-10-31): the N60's two muzzles (below); the
    Infector's 0, 0.75, 0 (0.82 from V2); the Disc Blade Gun's 0.35, 1.1, 0 (from V5 two,
    0.4, 1.2, ±0.2, traced); the Rift's 0, 0.7, 0 (V5 0, 1.0, 0); the Qwack-O-Ray's 0.21,
    0.6, 0; the RY3NO's 0.513, 1.0, 0.183; the Bouncer's 0.28, 0.85, 0. The Shield
    Charger's class is the shield round him.
  - **Where the hand is**, right, up and forward of his feet (the disc's sequences, posed:
    `hand` at key k of a sequence matches the traces' matrix within 0.01 u; a test checks
    the weapons file against the disc):

    | When | Place | Barrel |
    |---|---|---|
    | Standing, about to fire (traced) | 0.30, 0.56, 0.05 | 12° up, the same one- or two-handed |
    | Sequence 51 held, firing (N60, Coil, Qwack-O-Ray) | 0.26, 0.60, 0.53 | level |
    | Sequence 49 at its middle keys, firing | 0.26, 0.43, −0.23 | level; key 1–2, the kick, are 0.1–0.18, 0.46, −0.35 |
    | Sequence 46, firing (Suck Cannon) | 0.20, 0.30, 0.38 | level |
    | Sequence 103 as the Shock Blaster's rays leave, 5 frames in | 0.21, 0.40, −0.17 | 12° up, whatever it was doing: its 2-frame blend undoes any earlier pose |
    | Walking (traced) | 0.29, 0.59, 0.09 | level; swings ±0.1 u |
    | In the air (traced) | 0.32, 0.77, 0.10 | level |
    | Crouched, sequence 65 / 57 | 0.33, 0.26, 0.02 / 0.33, 0.30, 0.40 | level |
    | A glove, 20 frames into 41 / 61 | 0.39, 1.17, −0.12 / 0.05, 0.78, 0.43 | where its throw takes it |

  - **Traced muzzles:** the N60 held standing 0.28 ± 0.2, 0.82, 1.12 (the table gives
    0.26, 0.80, 1.13); its first shot 0.3 ± 0.2, 0.89, 0.59 (0.30, 0.88, 0.61); walking
    0.11–0.58, 0.70–0.81, 0.59–0.77 (0.29, 0.79, 0.69). The Shock Blaster's rays 0.26–0.36,
    0.91, 0.72–0.77 (0.30, 0.91, 0.75). In the look the barrel follows the view's pitch.
  - The sequence 103's pump swings the gun across him 30–90 frames after a shot, up to
    1 u left and 1.4 u up at the muzzle, but no ray leaves during it.
  - **In the kit** [seen, Annihilator held standing, disc model, Xvfb, 2026-10-31]: its flash
    and smoke start at the barrel's end, at its height.
- **State 29, aiming:** a gun's update puts the hero in state 29 when it finds him in the
  look (state 1). State 29 is the look with a gun out: it shares the idle's movement (the
  left stick doesn't move him), his facing snaps to the camera's yaw once the view is
  ready, R1 fires as well as Circle, and letting go of the look's button goes back to idle.
  The Shock Blaster waits 40 frames from entering it before it fires (0x1A749C, reset to
  40 whenever he isn't in it), as the Nitro Launcher does (below).

### Deadlocked's side of firing [code, DL NTSC, 2026-10-23]

Read in DL's code on DreadZone Station against UYA's functions above. The item in hand
is hero+0x2318 (libdl's weapon ids: 1 the wrench, 2 the Dual Vipers, 3 the Magma Cannon,
4 the Arbiter, 5 the Fusion Rifle); firing is hero+0x265A.
- **The walk's turn while firing** (0x67D100, DL's `hero_walk_turn`): UYA's numbers
  (gain 0.009 m, damping 0.15, cap 4.5° m a frame up to a 0.95 stick; 0.017, 0.18 and
  8.33° above; the caps written per second, as DL's literals are), but only while the
  Dual Vipers fire and he isn't skidding (hero+0x1C8F), where UYA's are its four
  automatic guns.
- **Firing in the gun cameras** (0x67CFD0, the turn every move's turning goes through):
  not UYA's tripling. In the lock-strafe **and** first-person cameras (hero+0x1AE0 not
  0), while firing or with R1 or Circle in the last 2 frames, and with anything but the
  wrench in hand, the turn's gain is raised to at least 0.2 a frame and its cap to at
  least 50° a frame (52.36 rad/s), which all but snaps him to the camera. Not in state 63
  (the grind turn).
- **The raised gun arm** (0x670A50, ray 0x6708B0): UYA's ray (0.57 u up, 0.7 firing,
  from 0.02 inside his radius) and `(1.3 − d) / 1.3 × 1.25` × the most, but:
  - the ray runs to 2.3 u, and a hit counts within 1.3 u **of its start**, so walls up
    to about 1.73 u ahead of his centre raise the arm, where UYA's ray ends 1.3 u ahead;
  - the angles depend on the item: the Fusion Rifle 0°–30°, the Dual Vipers 0°–25°,
    item 9 0°–45°, anything else 30°–40° (UYA 30°–70° for every gun);
  - in lock-strafe the camera's pitch, kept from 80° up to 20° down (UYA 45° down), with
    small offsets: 8.5° down standing with the Arbiter or Fusion Rifle, 6.5° up with
    hero+0x2658 set (not read), and for the Vipers or Magma Cannon with gadget 7 or 0x56
    23° or 28° more;
  - in first person (hero+0x1AE0 = 2) the camera's pitch 10° lower, with no wall ray,
    kept to the same limits (9° up with hero+0x2658 set). UYA's arm doesn't follow the
    first-person camera.
  - The ray's result is cached (hero+0x2F90 to +0x2F98) while the height stays within
    0.133 u.

### The hero's joint springs [code, data, UYA NTSC, 2026-10-22]

After his animation poses him, the hero turns a few joints by angles of their own, each
eased on a spring towards a target the game sets afresh every frame. Read in the update
(0x4DE348), the springs' step (`hero_joint_spring_step`, 0x4DC480) and their users.
- **The springs** (`hero_joint_springs`, 0x2F4580): 43 records of 0xB0 bytes, each naming
  a moby (+0xA2: 0, 2, 3, 4 and 6 the hero, 1 Clank on his back, 7 another) and one of
  its points (+0xA0; the class's point list, `moby-format.md`, "Points"). Each holds
  three target angles (+0x60, about the joint's own x, y and z; UYA's axes are x
  forward, y left, z up), the angles and their rates (+0x40, +0x50) and a gain and
  damping (+0xA4, +0xA8). Each frame (`hero_joint_springs_update`, 0x4DC790) every
  record with anything to do eases its angles on `spring_angle` (0x4455F0: the rate,
  kept within the gap, gains `gain × gap` and loses `damping × rate`, is kept within the
  gap again and moves the angle), hands the moby a turn built from them (x, then y, then
  z; 0x443FD8) for its joint, and sets its targets back to 0, so a spring nothing drives
  goes home. Gains stay as last set.
- **The hero's** [data; points read from his class on Veldin]:

  | Record | Point | Joint | Gain, damping at load | What drives it |
  |---|---|---|---|---|
  | 0 | 9 | 0, the root | 0, 0 | the strafe's twist (`hero_body_twist`) |
  | 1 | 10 | 2, the waist | 0.008, 0.3 | the gun target's pitch; the raised arm past 35°; slopes |
  | 2 | 11 | 6, the upper chest | 0.014, 0.3 | the gun target's yaw; a quarter of the look-at's |
  | 3 | 4 | 7, the neck | 0.007, 0.3 | the look-at and idle glances |
  | 8, 9 | 13, 12 | 72 and 52, the clavicles | 0, 0 | the raised gun arm |
  | 12 | 21 | 18, the eyes' parent | 0.12, 0.3 | the neck's targets: pitch / 2.8 within −10° … 17°, yaw × 0.25 within ±8.5° (0x4DC908) |
  | 4–7 | 22, 23, 7, 8 | 100, 94, 98, 92: the ankles and thighs | 0.1, 0.3 | slopes (0x4EEC00, not read further) |
  | 10 | 6 | 54, the right elbow | 0, 0 | not read |
  | 13–16 | 25–28 | 104, 106, 108, 110: the tail | 0.013–0.017, 0.21–0.3 | slopes |
  | 17 | 24 | 8, the head | 0.03, 0.2 | not read |

  Records 18 on are Clank's and the other mobys'.
- **The raised gun arm** (`hero_gun_arm_raise`, the bullet above): an arm angle a (UYA's
  sign, negative up) of −35° or less is split: the waist's y target takes a + 35° on a
  spring of 0.05 and 0.3, and the arm keeps −35°. The right clavicle's y target is a
  (0.05, 0.3); for the Annihilator and RY3NO its x target is a × 0.00997 (at most 40°),
  a slight roll; for a gun held in both hands (0x1A71CC = 2) the left clavicle takes the
  same. While firing at an enemy close ahead (the ray's hit a moby that 0x447CA0 accepts)
  the flag is set but the arm doesn't turn.
- **The turn to the gun's target** (`hero_head_look`): only in state types 0, 1, 2, 4 and
  7 (standing, walking, falling, jumping and one not read), and not in state 0x2E. From
  0.5 u ahead of his feet and 0.55 u up, the target's position raised by its damage
  record's height: yaw within ±59° on the upper chest's z, pitch (down positive) from 25°
  up to 35° down on the waist's y, both springs 0.037 and 0.3. Once a hero timer at
  +0x1FC (0x1A4DBC, not read) passes 27 the yaw keeps within ±17° and the pitch is 0.
  The look-at (no gun target) instead turns the neck: from 0.3 ahead and 0.85 up, yaw ×
  0.75 and the pitch on 0.021 and 0.27, the upper chest a quarter of the yaw on 0.014.
- **Order each frame:** the raised arm, the idle glances (0x4E2A30), the turn to the
  target (which overrides the raise's waist pitch), the eyes, the slope lean (0x4EEC00,
  which on a slope sets the waist's x and, not raised, the clavicles' y against it), then
  the springs step.

### The gun's own animation [code, data, UYA NTSC, 2026-10-22]

Each gun's class has three sequences (`moby-format.md`, "The gadget WAD"): 0 grows it
from nothing, 1 holds it, 2 recoils. How a moby plays them: a new moby starts on sequence
0 at speed 1 (`moby_init`, 0x435F98); its animation advances by its speed (+0x48) times
the sequence's rate (+0x18, or each keyframe's own 1/ticks), and every sequence loops,
setting bit 0x02 of +0x60 on the frame it wraps (`moby_anim_advance`, 0x4389D8). It
changes sequence by a cut (0x441BA0) or a blend over some ticks (`moby_anim_blend`,
0x441C78; 0x441E00 with flags), each resetting the speed to 1.
- **Drawn:** the gun's moby is spawned on sequence 0 and left to play it
  (`hero_spawn_item_mobys`; only the wrench is put straight on its sequence 1). Each
  frame in his hand (`hand_slots_update`, 0x4E93B8), once sequence 0 has wrapped and no
  blend runs, it blends to sequence 1 over 2 ticks. The Shock Blaster's draw is 11
  keyframes of 2 ticks, so about 22 ticks, with the draw sound on its trigger [data].
  Nothing scales the moby: any growing is in the keyframes.
- **At rest:** most guns' own updates check the same each frame: a sequence that has
  wrapped, with no blend running, blends to 1 over 2 ticks (the Flux Rifle, Annihilator,
  N60 and N90, Suck Cannon, Infector, Nitro Launcher, Disc Blade Gun, RY3NO, Spitting
  Hydra and Bouncer, V5 included). So a recoil plays once and settles back.
- **Firing** plays sequence 2, right after `hero_fire_anim`:

  | Gun | Each shot |
  |---|---|
  | Flux Rifle, Annihilator, Infector, RY3NO | cuts to 2 |
  | Disc Blade Gun | cuts to 2 (still drawing, it first blends to 1 over 4 ticks) |
  | Nitro Launcher, Bouncer | blends to 2 over 2 ticks |
  | Spitting Hydra | nothing; but each frame it is held charging it cuts to 2 unless on it, after the rest's blend above, so 2 loops for the whole hold and settles once let go (corrected 2026-10-29: the call is in the hold's branch) |
  | Suck Cannon | cuts to 2 unless already on it; the call (0x441C78) is in its pull branch with its looped sound, so it is while it pulls rather than per shot, and as it lets go it calls 0x441FB8 with 2 or 1 (not read; 2026-10-29) |
  | N90 Hurricane (the N60's V5) | cuts to 2 unless already on it, then plays it at speed 3 |
  | Shock Blaster, N60 V1–V4, Lava Gun, Qwack-O-Ray, Rift Inducer, the gloves, the whip | nothing |

  The Plasma Coil and Plasma Storm cut to 1 once their draw has played and never play 2.
  The Annihilator fired before its draw ends blends to 1 over 4 ticks first.
- **Put away:** `hero_item_switch` (0x4E92E0) blends the held gun to sequence 2 over 2
  ticks and marks its slot as leaving; the next frame `hand_slot_remove` (0x4E8698)
  removes it at once (the hand's slots; the other slots wait for their sequence to wrap)
  and holds off the next spawn about 3 frames (and until the new gun's class has streamed
  in, `gadget_is_ready` 0x4E9228). So the old gun vanishes and the new one grows in.
- **In first person** (`hero_fp_gun_place`, 0x50F410): the gun is placed before the
  camera by its row's offsets (+0xA0 forward, 0.35–0.7; +0xA4 to the side, −0.2 to
  −0.65, mirrored with 0x1D5477; +0xA8 down, −0.5) and angles (+0xB0: 0, −90°, 90°),
  bobbing 0.02 × sin(phase), the phase easing to 600°/s while he moves and 90°/s at a
  speed of 0.02 u a frame or less; every other frame a ray pulls it back up to 1.5 u when
  something is in front; the fire direction is the camera's.

### Holding a gun [code, data, UYA NTSC, 2026-10-23]

A gun has no idle of its own for the hero: `hero_idle_sequence` (0x4DC0D0) gives the same
sequence 0 whatever is in his hand, and the walk and jumps are the wrench's too. What
holds the gun is a pair of **hold layers** over his arms (`hero_hold_layers_update`,
0x4E61E8, each frame from the hand's update 0x4E7118; the layers at 0x1A5A50), chosen by
the hand item's hold type (row +0x18, kept in `hero_hold_type`, 0x1A71CC, as its moby
spawns).
- **The classes:** each layer plays a class of its own from the level's core: class 1
  for the right arm, class 2 for the left [code, data]. Neither has a mesh, a skeleton or
  joint offsets (+0x14 and +0x18 are 0): only 20 joints' worth of keyframes, in 24
  uncompressed sequences, and at +0x1C a list mapping its joints to the hero's, 52–71
  (right clavicle to elbow helper, the fingers included) and 72–91 [data, Veldin's
  image]. Their translations name the hero's joints (52, 54, 56; 72, 74, 76, 86).
  `moby_anim_layer_keyframe_of` (0x437200) turns a keyframe of the class into one of the
  hero's: each joint's rotation into the hero joint the list names, its translations as
  they are. The layers drive the joints listed after the chain of the hero's points 12
  and 13 (`moby-format.md`, "The class header"): 52–71 and 72–91.
- **Which hold:** the right layer for hold type 1 or 2, the left for 2. Of UYA's guns
  [data]: one hand, the N60 Storm, Qwack-O-Ray, Plasma Coil and Shield Charger; none, the
  wrench, the Plasma Whip and the three gloves (Agents of Doom, Holo Shield, Miniturret);
  both hands, the other twelve.
- **Following the body:** each frame a layer takes the hero's own sequence number, its
  keyframe and its blend between sequences, and plays that sequence of its class. Classes
  1 and 2 have the hero's sequences 0–22 keyframe for keyframe (the same keyframe counts,
  but 17's 27 to the hero's 25; their 23 is never played) [data]; for the hero's sequences from 23 on (the
  firing animations, attacks, ledges, swimming…) the layer plays the hero's own sequence,
  which changes nothing. So the hold is the idle, walk, skid, crouch, jumps, falls,
  landings, glide, hit and fidgets 1 and 2 with the gun held.
- **Resting:** through the sequences `hero_arms_rest` (0x4E55A8) lists, the left, right
  and back flips (28, 29, 31; not the forward flip, 30), the grind's stances (44, 45), the
  grind jump (72) and 50, 66, 67, 80, 84 and 85, the layer blends to its own sequence 0
  over 15 frames and plays it on its own; after them it blends back over 10 frames. In the
  three flips past animation frame 25, both clavicles' springs (records 8 and 9, "The
  hero's joint springs") take a target of −75° on y at 0.05 and 0.3, lifting the gun; in
  50, −55°.
- **Weights:** a layer starts at 0 and rises 0.25 a frame; while the firing arms layer
  (`hero_fire_layers`, 0x1A5A58) plays it falls 0.15 a frame, and rises again as that
  one fades (0x1A5A64 set). Switching to an item of another hold type
  (`hero_item_switch`) marks both leaving: they fall 0.1 a frame and are dropped at 0,
  before the new hold's start. On a ledge (states 23–27 set `hero_no_hold`, 0x1A71D0;
  every other state change clears it) both leave and none starts.
- **Fidgets:** the third idle fidget (sequence 73) doesn't play with a gun held in both
  hands (`hero_fidget_pick`, 0x502930; `hero-controller.md`, "Idle fidgets").
- **The gun's own size:** a moby starts at its class's scale (`moby_init`, 0x435F98: +0x2C
  from class +0x24). Of the guns' updates, only the whip's changes it: on its first frame
  it sets its handle to 0.3 of its class's scale ("Plasma Whip").

### Checked in traces [verified on UYA NTSC, 2026-10-20]

`uyantsc-veldin-fire-1` (the Shock Blaster) and `-2` (the N60 Storm), each gun given by
writing its bytes and drawn with Circle (`docs/re-setup.md`), third-person camera:
- **Drawing:** the gun was in his hand 17 frames after Circle and first fired 33 frames
  after that, the frame after the press. The held frames (0x1A5E00) were 3 as the old
  item left and 4 on the gun's first frame in hand, in both traces.
- **When a drawn gun may fire** [code, 2026-10-31]: each gun's update compares the held
  frames itself (`slti` on 0x1A5E00): 23 for the Shock Blaster, N60, Nitro Launcher,
  Infector, Disc Blade Gun, RY3NO, Plasma Coil, Bouncer and Shield Charger (and their
  V5 code), 28 for the Annihilator, 16 for the Suck Cannon. The Lava Gun, Flux Rifle,
  Spitting Hydra, Agents of Doom, Rift Inducer, Holo Shield, Plasma Whip, Qwack-O-Ray
  and Miniturret never read it (`RefsTo`), so they fire from their first update in hand.
  With the count at 4 on that frame, a 23-frame gun is ready 19 frames after it is in
  his hand.
- **The flags:** the Shock Blaster set `firing` alone; the N60 set both flags together,
  and both cleared the frame after Circle was let go. Starting a walk or a jump cleared
  the Shock Blaster's `firing` at once, but not the N60's while Circle was held.
- **Standing:** whole-body sequence 103 (the Shock Blaster) and 51 (the N60), at their
  keyframes' own rates (2 frames each for 103, 4 for 51, whose first took the N60's
  5-frame blend), restarted by each shot, so between shots he holds the firing pose. The
  Shock Blaster's `firing` stayed set 59 frames between two shots (2026-10-31).
  While firing, the fire direction's yaw eased back to his facing within about 15 frames
  (the head-look drops its look-at target while firing).
- **Walking:** no arm layer ever started (neither gun has an arms-only animation).
- **The N60:** a shot every 14 frames while held, one round each.
- **The walk's turn while the N60 fires** (full stick, turned 90°): each frame's rate is
  `rate + 0.017 × (move angle − last yaw) − 0.18 × rate`, on all 31 frames of the turn
  (to 10⁻⁸); the same turn without firing fits the moving walk's 0.037 and 0.13 on its 9
  frames before it arrives.
- **No double jump while firing:** with the N60 firing, X again on the jump's 10th frame
  gave no double jump (the press is still inside the double jump's window of
  `state timer − 4` frames when it opens on the 15th); from the Shock Blaster's shot, whose
  `firing` the jump cleared, the same presses double jumped on the 15th frame.
- **No release skid with a gun:** letting go of a full-speed walk with the N60 out, firing
  or not, idled the next frame; the release flag (0x1A4FD0) never set with the gun, and
  set at once in a walk with the wrench.
- **State 29:** L1 with the Shock Blaster out went straight from idle to state 29; the
  40-frame wait counted down one a frame; R1 at 11 frames did nothing (and played
  sequence 13 while held), R1 at 62 frames fired; letting go of L1 went back to idle and
  reset the wait to 40.

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
  Plasma Coil the same as +0x6C. The Annihilator's is its blast's (below); the others'
  uses aren't read. The Flux Rifle has 0 there and half its damage at +0x74.
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
  each ricochet; V3 goes on through enemies too; V4 longer range and harder blast. The
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
- **Firing at V1:** Circle held fires (the pad's held mask, +0x1A0), as often as the
  cooldown lets it; a press during the cooldown isn't kept. Each shot takes one round,
  plays the firing animation (0x4E6A08) and sets a cooldown of 1 / (+0x50) = 0.5 s.
  **[verified on UYA NTSC, `uyantsc-veldin-fire-1`]:** the shot came the frame after the
  press; held, the next came 35 frames later, which fits the 5-frame hit delay (the
  whole-body animation was playing) plus the 30-frame cooldown; a tap 12 frames after a
  shot did nothing. (Read as "Circle pressed" until 2026-10-20.)
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
    15° down, spread 20° apart (0x36B888 tests each). They start at the gun model's
    point 0 (joint 13; 0.33 u above and 0.93 u along the barrel from its grip). A ray's
    hit does less the further it reaches: full under 10 u, × 0.75 to 14, × 0.5 to 20,
    × 0.125 beyond (× 3 charged) (read 2026-10-31).
  - At V2 and up, each enemy the balls hit also gets the shock mod's check (0x34BF98).
- **Aim:** with a target in 0x1A57B8 (the lock-on target is copied there), the aim points
  straight at it (`aim_at_target`) into 0x1A52A0; the shot uses 0x1A52A0, or the camera's
  aim (0x1E2A60) with the lock-strafe camera (camera mode 1).
- **First person:** in the look state (1) the gun puts the hero in state 29, its aiming
  state, where R1 fires as well as Circle.
- The moby's +0x7C (2 at V2, 3 from V3) is set from the level; its use isn't read.
- **V5 and up** (the Shock Cannon, 0x385948, 2026-10-22): the same firing, charge and
  balls; 23 random rays (0x386FB0 tests each) and the three down, 26 in all.

### N60 Storm (0x3A5928, bullets 0x35B390) [code, UYA NTSC, 2026-10-16]

- **Firing:** while Circle is held (or R1 in state 29), with the same conditions as the
  Shock Blaster's (game mode 0, the gun out more than 22 frames), it fires at once and
  then every 14 frames (about 4.3 shots a second), one round each. Letting go resets the
  timer, so the next press fires at once.
- **Two barrels:** the shots alternate between two muzzle points (0x1DE340 and 0x1DE350
  in the gun's frame): (0.2, −0.6, ±0.2), turned by the gun's matrix but not scaled, so
  0.6 u along the barrel, 0.2 u above it and 0.2 u either side; the flag 0x1DE3A0 swaps
  them, 0x1DE340 first. With a target the bullet aims from 0x1DE340's point whichever
  fires. The bullet flies along his facing at a random 60–80 u/s (read 2026-10-31).
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
- **V5 and up** (the N90 Hurricane, 0x322E68, 2026-10-22): a shot every 10 frames held;
  its bullets (0x384068, class 0x1881, flight 0x384338) leave at a random 60–80 u/s and
  live the frames 30 u takes plus 30 (52–60). The line of each step: the world bounces it
  (velocity reflected), the 8th bounce ending it; an enemy (category 5, or class 0x1F6)
  ends it, taking the row's damage, the shock check and a 1 u ball.

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
  frame is cut to 22 u/s.
  - With L1 held (pad bit 4; L2 in the swapped layout, 0x45E3B8) and no target (2026-10-31),
    the aim is along the camera's bearing, as far as the horizontal distance d from the
    camera to where its aim meets the hero's ground, up to 12 u (all 12 u if it looks up).
    The result is eased: 12 × (1 − (1 − d/12)²) (0x442070), and is measured from him.
- **The charge** (class 0x185A, spawned by 0x361880) spins as it flies:
  - gravity 12 u/s² along the level's up (0x4C4190), not the 16 it was aimed with, so it
    flies higher and lands further than the aim point;
  - it explodes on an enemy within 1.5 u of it (class category 5);
  - it bounces off the world (velocity reflected) and explodes 10 frames later;
  - it passes through the Nitro guns (classes 0x1865, 0x1885) and objects of class 0;
  - after its first 40 frames, it explodes on reaching the ground (a 1 u probe down);
  - it has no life limit, and one that falls off the world is never removed (read again
    2026-10-31: no other timer in 0x361218; on one level, with 0x1DB030 set and level 5,
    it also goes off below z 32.5 or 20 u from the point at 0x1DD790).
- **The blast** (class 0x1858, spawned by 0x35E5D8): 12 frames of flash, then for 20
  frames a ball whose radius grows from 2 u to 6 u (7 u from V4), damaging all it touches
  but the hero for the row's damage (+0x6C), pushing them away at 1.5. Each target hit
  also gets the acid mod's check (0x31AC38). The camera shakes by 0.4 − 0.02 × the
  distance to it, within 20 u.
- **Mods:** acid from V2 (set as each blast spawns), lock-on from V3 [code].
- **V5 and up** (the Nitro Eruptor, 0x3893E8, 2026-10-22; launch read again 2026-10-31): the
  same charge (0x361880), lobbed by its own arc (0x389A18, the same numbers and L1 aim, with
  no 12 u test on a lock-on target), its launch turned about his left axis (0xD0) before its
  speed is cut: 5° down with no target, 10° up with one (0x40CCA0, whose sign the Disc Blade
  Gun's trace fixes). "About his up" was a misreading. The charge marked as the Eruptor's
  ends in another blast (0x38A668, class 0x1888, update
  0x38A7B8): 18 frames, then for 30 frames a ball growing from 3 to 11 u in which
  everything (not the hero) takes the row's damage every frame (flags 0x1030000, pushed
  1.5 away) and the acid check. As it bursts (2026-10-22) it fires 15 bullets (class
  0x1855, `enemy_bullet_spawn` 0x35D8E8) at 20–30 u/s, each a flat 10 damage (flags
  0x10000): one at each target but the hero farther than 8 u among up to 10 within
  13 u, the rest in random directions (any bearing, 20° down to 15° up): the vendor's
  "spread of missiles". (Class 0x1888 is the Eruptor's blast, not the Decimator's.)

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
    damage (the row's byte +1 above 3, and 0x447CA0's few classes spared) is frozen to
    death instead (read again 2026-10-31):
    - no damage record, no splash, and no `moby_killed`, so no bolts in this branch; its
      XP goes straight to the gun's word (0x142E24 is the Lava Gun's in the table
      0x142DE0), a fiftieth of the XP bank with it;
    - it stops, flag 2 set, and joins the shared list 0x271BC0 as kind 5 (0x3154D8, run by
      class 0xC41's 0x313E28) for 1 s, a value falling from 1 at 1/s (0x1DC7B8), with
      cold puffs (0x315180) while it is above 0.9; then 0x3DC570 sets its record's state
      to 2 (what the enemy does with that isn't read);
    - a block of class 0x1E52 is set on its middle at its matrix, 1.5 tall, scaled by the
      class's size × the record's +0x10 × 0.75 (0x3E1418);
    - an enemy whose record is in state 3 only takes a plain cold hit (flags 0x10010000);
    - **in the kit:** the kill, XP and no drops (`Affliction::Frozen`); not the block, the
      puffs, its stopping for 1 s, or classes that are spared.
- **V5** (class 0xC59, 2026-10-22) runs the same update (0x303F88); besides the freezing,
  only its nozzle ball's flags (0x11010000 for 0x1110000), its sounds' and effects'
  class and its splashes (cold, 0x470258, for 0x463938) differ.
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
  enemy within 1 u of a point 1 u along the barrel and 0.275 u along the gun's x from the
  gun model's origin (not its muzzle; about 0x36DC40, read 2026-10-31) becomes the
  missile's target; once any candidate passes the filter, the target kept is the sphere
  query's first result (0x1EAA98), not necessarily that one.
- **The missile** (class 0x1894, 0x3965B0):
  - speed: row +0x4C u/s (60), so it flies 75 frames, 75 u;
  - it starts towards the target, else along the hero's aim (the camera's aim in the
    aiming state or the lock-strafe camera);
  - **it doesn't home** (read again 2026-10-31): the update homes only while byte +0x26
    of its variables is set (0x396B7C), and the spawn clears it (0x396834, and again at
    0x3968C8); no code sets it (every byte and halfword store at +0x26 in the image writes
    other structures). The RY3NO's V1 missile (0x382E18), with the same layout, does set
    it (0x383258). So it flies straight, at its target's middle as it fires. Its 294 is a
    test that it has at most 294 frames of life left before homing, which never fails;
  - it goes, without exploding, once more than 75 u from the hero;
  - each frame it tests the line of its step, carrying the row's damage (push 1.5) to the
    first thing it hits, and explodes;
  - **the blast** (0x3985C0, 2026-10-20) also does the row's second damage (+0x70: 6 at
    every level) to everything within 0.75 u of it but the missile;
  - **the mines** (found 2026-10-22, in the missile's explosion, a switch on the level):
    V2 scatters 4, V3 6 and V4 6 heavy ones, each a Bouncer bomblet
    (`bouncer_bomblet_spawn` 0x31E2D8, its update 0x31E488) marked the Annihilator's:
    model class 0x1897 at 0.275 of its scale (0.55 heavy), a tenth of the row's damage,
    bursting in a 2 u ball (7 u heavy); they leave 50–90° up from the surface's plane at
    4–9 u/s (V4: 40–75° at 4–12 u/s) on a random bearing, bounce towards enemies within
    7 u and arm after 70 frames plus their index, as the Bouncer's do;
  - from V5 (the Decimator) the explosion instead sends seven seekers (the small missile,
    class 0xA82, `small_missile_spawn` 0x3441D0) of a fifth of the damage each, fanned
    ±80° round the bearing towards the camera, starting at 1–3 u/s and speeding up by
    0.7–0.9 to 15–20 u/s; the call (0x39753C) gives them 300 frames of life, the missile
    as their first target and the weave on ("The small missile");
  - its model is drawn 2.5 times larger, 4 times from V5; its trail is tinted blue from
    V2 (0x398688).
- **Mods:** shock from V3, lock-on from V4 [code].
- **V5 and up** (the Decimator): its update (0x38A648) only calls the V1–V4 one
  (0x36D5A0); its level tests are in the shared code: the gun's animation plays at speed
  2 as it fires, the missile is scaled ×4 and its blast sends the seekers. It has no
  mines.

### The small missile (spawn 0x3441D0, flight 0x343330) [code, UYA NTSC, 2026-10-31]

Fired by the Decimator's blast (300 frames' life, weave on), the Megaturret (270 frames,
weave off) and the Agents of Dread (250 frames, weave off; the spawn's stack arguments at
its three calls, read 2026-10-31).
- **Search:** its first target here is invalid (the Decimator passes the parent missile),
  so on its first frame it takes the first enemy within 15 u of itself that the hero
  hasn't claimed, and claims it (0x24FA60, 32 entries, 200 frames), so the seven spread
  over different enemies; one that finds none never looks again. A target lost after its
  61st frame is looked for within 30 u.
- **Turn** (0x449148, into 0x3F1438): with θ the angle between its velocity and its aim
  point and ω its turn rate (radians a frame, from 0, kept frame to frame), each frame
  ω += 0.03 θ − 0.2 ω, |ω| is kept within θ, and the velocity turns ω towards the point.
  Within 0.25 u and under 90° the factors are 0.5 and 1, so ω = θ / 2. Past 165° it
  turns about a horizontal axis.
- **Aim point:** the target's middle plus a wander: x −cos a × min(0.751 d, 0.751),
  y sin a × min(0.749 d, 0.749), z (sin b + 1) × min(0.5 d, 0.5), with d its distance (0
  under 0.25 u), a moving a random 0.01–0.1 rad a frame and b 0.025–0.25.
- **Weave** (when on): a phase moving 3–5° a frame, wrapped to ±π, turns the velocity
  about its side axis and then its up axis by phase × π / 180 (at most ±3.14°) a frame.
- **Hits:** with the weave on, it meets nothing for its first 15 frames, then a 0.25 u
  sphere and the step's line; its blast is 1 u, pushed by its velocity.

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
  +0x98 record has +8 set), else flies on. This shot never pierces, at any level: its
  update (0x346A00) and per-step calls (0x3478E8, 0x348A38 from V3) only lay its trail.
- **The scoped beam pierces** (found 2026-10-22): class 0x186F (`flux_scope_beam_spawn`
  0x37C818, its update 0x37CB68), from the camera's position along the aim, fired only
  scoped (the look or state 29 with the view ready) and in first person or state 29. At
  V1 one line test: the first hit takes a 0.6 u ball of the row's damage and the beam
  stops hitting. From V2 up to 6 line tests a frame, each skipping only what the one
  before hit, so it passes the first target to reach the second (three in a line, it
  alternates between the first two); each hit is a ball of 0.8 u at V2–V3 and 2.5 u from
  V4 at the hit point, pushed along his aim × 1.75, flags 0x10000 (0x110000 from V4).
  Class 0x1CF7 stops it. The beam shows for a 20-frame timer in a colour by level.
  Its head (2026-10-22) flies 30 u a frame along the aim (from the camera with offsets
  along its forward and side in first person) and the tests are of each frame's step;
  any hit sets its speed to 0.2 u a frame backwards, so the first frame with a hit is
  the last that hits, and it goes after 26 frames. It is drawn from its start to 100 u
  ahead whatever it hits.
- **The impact** (0x346EF0, 2026-10-20): at V1 sparks; from V2 a bigger burst and a 2 u
  ball (`ball_damage`) carrying row +0x70, which is 0 on every Flux row, so it hurts
  nothing; from V4 a further effect (0x447D00). The scoped hitscan from the camera carries
  damage flag 0x100000 from V4 (0x10000 below), meaning not read. Row +0x74 holds half the
  damage on every Flux row and no reader is found.
- **Rate** (0x3444B0): a shot every 1 / (+0x50) = 1 s. Scoped, the zoom follows the
  buttons' pressure (pad +0x134 and +0x13C), and a ray from the camera along the scope
  (0x1E2E40, 128 u) marks a target with a reticle every fourth frame.

### Where each gun's code is [data, UYA NTSC, 2026-10-22]

A gun in hand is a moby of its row's +0x10 class, and `class_update_table` (0x2F6984,
216 entries of 12 bytes: a pointer, the class, its update) gives that class's update,
which is the gun's code. Read from Veldin's image:

| Gun | Classes (V1–V4; V5 up) | Update |
|---|---|---|
| Infector | 0x1861; 0x1886 | 0x3688E8; 0x389F58 |
| Spitting Hydra | 0x184E; 0x1B4F | 0x31FB90 |
| Disc Blade Gun | 0x1866; 0x1C44 | 0x36EFC0 |
| Agents of Doom | 0x1852 | 0x35C9F0 |
| Rift Inducer | 0x1C67; 0x186A | 0x372990 |
| Holo Shield Glove | 0x1868; 0x1C65 | 0x3717A8 |
| Plasma Whip | 0x1843 (+0x14 too) | 0x356110 |
| Suck Cannon | 0x185B; 0x1892 | 0x3619C8 |
| Qwack-O-Ray | 0x1870; 0x1C63 | 0x37DEB0 |
| RY3NO | 0x1871; 0x1DE1 | 0x3805B0 |
| Plasma Coil | 0xC4A; 0x5CA | 0x315638; 0x306F68 |
| Bouncer | 0xE14; 0xE9E | 0x31C890 |
| Miniturret Glove | 0xC5A; 0xA65 | 0x346078; 0x343088 |
| Shield Charger | 0xA5C | 0x3410E8 |

The six guns already read: Shock Blaster 0x36A790 (V5 0x385948), N60 0x3A5928 (V5
0x322E68), Annihilator 0x36D5A0 (V5 0x38A648), Flux Rifle 0x3444B0, Nitro Launcher
0x36E360 (V5 0x3893E8), Lava Gun 0x303F88. The rows also give each gun's hold and
firing animations (+0x18, +0x24–+0x2C): the gloves (Agents of Doom, Holo Shield,
Miniturret) hold nothing and play 41, arms 61; the Suck Cannon 46; the Qwack-O-Ray,
Plasma Coil and Shield Charger one-handed 51 (arms 57; the Shield Charger's arms 52); the
Plasma Whip none; the rest two-handed 49 (crouched 65), the Bouncer 103 as the Shock
Blaster does.

### Plasma Whip (0x356110) [code, UYA NTSC, 2026-10-22]

The whip is a melee weapon: with it in hand (item 0x7F) the hero's own attack states play
as with the wrench, and the whip's object hits along its rope. (Earlier notes called item
0x7F the multiplayer wrench; it is the whip, by its row's name.)
- **Attacks** (`hero_attack_start`, 0x501678): Circle starts the wrench's moves, in the
  same windows: on the ground the combo (state 19, its three swings and their chain), in
  the air the jump attack (20), and with R1 held, or crouched, the throw (21). Square
  still swings the wrench.
- **Ammo:** entering any of the three states takes a round (`hero_change_state`: each
  swing and the jump attack call `ammo_use(held, 1)`; the throw's release does). Without
  ammo the state change is refused and he stays as he was.
- **The rope** (`whip_rope_spawn` 0x3572B0, `whip_rope_ends` 0x357678, `whip_rope_lag`
  0x3574D0, `whip_link_place` 0x357778): the whip in his hand (class 0x1843, from the
  gadget WAD) leads nine links, eight of class 0x183A and the tip of 0x183C (corrected
  2026-10-22: the listing at 0x357318 picks 0x183A for links 1–8). Both link classes are
  in the core of every single-player level, 1–36, not the multiplayer maps [data]; each
  has one still sequence (0x183A: 3 joints, scale 0.04077; 0x183C: 2 joints, 0.05679).
  Both are rigid, every vertex on joint 0, and posed as at rest [data, 2026-10-29]: at
  the rope's 0.2 of its class's scale, 0x183A runs from 0.098 u behind where it sits to
  0.196 u ahead along its +x, 0.14 u wide and 0.06 thick, its joint 0 0.194 u ahead and
  joint 1 0.398; 0x183C to 0.273 u ahead, its joint 1 0.105. Each takes one strip of
  texture 49 (128 × 128 in Veldin; four strips side by side, the links' coordinates below
  0, 0.78–0.97 once wrapped).
  - Spawned once: the handle at its class scale × 0.3, tinted (0x40, 0x10, 0x80); each
    link at its class scale × 0.2, alpha 0x3C (about 47%), following the whip for lights
    and draw distance (`moby_follow_parent`, 0x452E70).
  - Each frame, each piece (the whip, then links 1–8) wants its end at its joint 0 plus
    the rope's stretch (the whip's +0x14) along joint 0's x axis, raised to at least
    0.15 u above the ground found by a line from 0.5 u above it. The stored end of each
    piece then turns from where it was towards that by 0.1 + the whip's second value
    (+0x1C: 0 at rest, up to 0.9 in a swing), so the rope trails more at rest and pulls
    taut as he swings (the direction of the turn is read with medium confidence).
  - Link k sits at the lagged end of piece k − 1, turned away from that piece's joint 0
    (yaw by atan2 + 180°, pitch from the height difference, roll 90°), so its joint 0,
    where piece k starts, is 0.194 u on along that piece's way. In the wrench
    combo (state 0x13) the links lie flat (no pitch or roll), and links 1, 2 and 5 add
    the hand's travel since last frame × 4, 2 and 1 (2 × (2 div k) in integer division;
    the others none), so the first links trail the swing. (Read again 2026-10-29: in
    `whip_link_place` that term is an angle added to the link's yaw, atan2 of the whip's
    position less a vector the whip keeps at +0x68, times 4, 2 or 1; what the vector
    holds after the spawn isn't read, so whether it is the travel is open.)
  - So the rope is 9 × the stretch and 8 × 0.194 u long [code, data; corrected
    2026-10-29, the links' joint 0 having been left out]. The links hide with the whip, in
    cutscenes, and 300 frames after the ammo runs out. The stretch moves towards a goal
    by at most a step a frame (`approach`, 0x4450C0):

  | When | Stretch (step) | Second value, +0x1C (step) |
  |---|---|---|
  | At rest | 0.2 | 0 |
  | Combo, first 7 frames | 0.25 | 0.5 |
  | Combo after (swings 1, 2) | 0.9 (0.2) | 0.9 (0.3) |
  | Combo after (swing 3) | 0.6 (0.2) | 0.8 (0.3) |
  | Jump attack, frames 7–20 | 0.7 (0.1) | 0.7 (0.1) |
  | Jump attack after | 0.8 | 0.8 |
  | Thrown | 0.5 (1/30) | 0.9 (1/30) |

  So swings 1 and 2 reach about 9.7 u, swing 3 7.0 u and the jump attack 8.8 u.
- **The hit** (0x357F10): in the combo once the state is over 6 frames old, in the jump
  attack over 20, and while the thrown end is out, every frame each link's line, from
  joint 0 of the piece before to its own joint 1, is tested (`coll_line`, 0x357DC0), and
  link 8's carried on to 1.5 times its length. Laid straight, the tip's line ends 9 ×
  the stretch + 1.657 u from the hand (8 × 0.194 + 0.105) and link 8's longer one 8.5 ×
  the stretch + 1.955, which reaches further only below a stretch of 0.6 [code, data,
  2026-10-29]. Whatever it crosses takes the row's damage
  (+0x6C), pushed 0.75 along the whip's facing and 1 up, with flags 0x110000 at V1–V2 and
  0x210000 from V3. A hit on something that isn't an enemy sparks (0x359F28); each hit
  plays sound 6. Each frame's hits go through the damage queue, so a target takes at most
  one a frame; nothing marks a target as hit for the rest of the swing.
- **The throw** (0x4F1CE0, at the wrench's release frame): instead of the wrench, a moby of
  the thrown wrench's class (0x47, run by `wrench_update`) flies out as the thrown wrench
  does ("The thrown wrench" in `hero-controller.md`), and the whip's end follows it; the
  rope hits along its length while it is out.
- **V4** (level 3 and up): at the jump attack's 20th frame a shock wave (class 0x1890,
  0x393690) sets off from the whip along his facing at 0.75 u a frame (45 u/s). It lives
  40 frames. From its 11th frame, every third frame, a ray 10 u down from where it was 6
  frames before finds the ground, and a 2 u ball there takes half the row's damage, pushed
  0.5 (flags 0x210000).
- **V5 and up** (the Quantum Whip): in swings 1 and 2 at frame 10 and swing 3 at frame 30,
  a cutting wave (class 0x1CB8, 0x3D6578) leaves along his facing, turned −20° for swing 2
  and +10° for swing 3. Its flight (0x3D6740) runs on at 0.5 u a frame (30 u/s) for 40
  frames; each frame the line between its joints 0 and 6 (about 3 u across it) strikes
  what it crosses for the row's damage, pushed by its velocity × 1.75 (flags 0x210000).
  The game strikes every frame it crosses; the kit strikes each target once.
- **Looks** (2026-10-22): each frame the links take a glow colour (`moby_set_colour`,
  0x438908; R, G, B) of (0xBF, 0x40, 0x20) at V1–V2 and (0x20, 0x40, 0xBF) at V3–V4;
  from V5 none. A light starts at radius 5.5 at the whip and then pulses between 3.5 and
  4.5 (3.5 + 0.5 + 0.5 sin(phase × 0.1)), its colour swinging between two (written R
  low: 0x2055FF and 0x5585FF at V1–V2, 0xFF5520 and 0xFF8555 from V3) by 0.5 + 0.5 sin,
  the phase moving a random 160–800°/s. Its sparks and trails are effects (`effects.md`).

### Plasma Coil (0x315638, ball 0x30BF18) [code, UYA NTSC, 2026-10-22]

- **Firing:** while Circle is held, with the gun out more than 22 frames, at most five
  balls alive (0x271B70) and the cooldown run out, it takes a round and starts the coil
  spinning: the spin rises 0.1 a frame from 0.1 and the ball leaves when it passes 1, the
  10th frame after it starts. Starting sets a cooldown of 0.5 s, so held it fires every
  0.5 s. After the shot the spin falls 0.01 a frame; the next start spins up from where
  it is. In the look (state 1)
  it puts him in the aiming state (29), as the other guns do. From V3 the update sets the
  item's lock-on bit itself.
- **Aim:** the lock-on target; else the gun's assist with 20 u, far 10° by 10° and near
  45° by 45°. The ball goes along the hero's aim (the camera's with the lock-strafe
  camera), at the target (its centre raised by its height) when there is one, or at
  anything targetable within 1 u of the muzzle; with the lock-strafe or first-person
  camera it bends to a target within 35 u and 80° (0x4C7010), as the Flux Rifle's scope
  does.
- **The ball** (class 0xA9B, spawned by 0x30D168) flies straight at row +0x4C = 15 u/s,
  for at most 160 frames (40 u):
  - each frame a 0.45 u ball round it looks for anything targetable (not another coil
    ball, not the hero): the ball sticks to it, and it takes the row's damage (+0x6C),
    pushed by the ball's velocity / 30, flags 0x410000, and the acid mod's check;
  - else the line of its step, or a 0.1 u ball, against the world: it sticks there, 0.1 u
    off the surface;
  - 15 frames after sticking, arcs leave it for up to 3 other targets within 10 u all
    round (`gun_aim_target` with a full cone), not the one it is stuck to; each arc
    reaches its target over 10 frames, and then that target takes row +0x70 (the same as
    the damage on every coil row), pushed by (target − ball) / 30, and the acid check.
    Up to 12 arcs are alive at once (0x1E17A4). Then the ball goes.
  - Out of time in the air, it fizzles with no blast.
- **Mods:** lock-on from V3 [code]; acid from V2 [text]: the hits run the acid check
  (bank 0: 2 a second for 4 s; V3–V4 5 a second), but no code read sets the coil's acid
  bit. V4's "more power" is the row's damage.
- **V5 and up** (the Plasma Storm, class 0x5CA, 0x306F68): it spins up as the coil does
  (its spin constants, 0x1DC62C on, aren't read) on the row's rate, and throws a ball
  (0x3098F8, class 0x9A9, update 0x309480) along the aim at 1/3 u a frame (20 u/s):
  - it flies 120 frames (+0x34), moving by its velocity (0x309AA0); what its sphere meets
    (the world, or a moby that isn't another storm ball) puts it at the contact and
    reflects it, keeping 0.85 of the speed into the surface, and a moby met takes the
    row's damage (flags 0x410000, pushed by its velocity a frame / 30) and the acid check;
  - each frame it finds up to 15 targets within 15 u all round (0x44CF68) that it can see
    and hasn't arced to in the last 120 frames (0x4C46A8, 0x4C4640), and sends each an arc
    (0x30A148: one of 30 shared slots, reaching its target over 10 frames; at most 15 from
    one ball at once). The arcs (`coil_arcs_update` 0x30A3D0, 30 slots of 0xA0 at
    0x26FDB0; read 2026-10-22) strike as the coil's: the start follows the ball, the end
    grows to the target's centre over the reach; then the target, if it still has
    collision and wasn't struck, takes the Plasma Coil's row +0x70, pushed 1/30 u along
    the arc (flags 0x410000), the acid check (item 0xA0), a sound and sparks (0x30BC80),
    and the arc stays 20 frames. Arcs waver by six random phases and rates (±0.001–0.005
    rad a frame, ×100 while reaching) of amplitude 3.25–4.75;
  - then for 40 frames it shrinks, jittering, sending no arcs, and goes once its arcs
    are done.

### Bouncer (0x31C890, canister 0x31D0B0, bomblets 0x31E488) [code, UYA NTSC, 2026-10-22]

- **Firing:** while Circle is held, with the gun out more than 22 frames and the cooldown
  run out, it takes a round and fires a canister, and sets a cooldown of 1 / (+0x50) =
  3.33 s. In the look it puts him in the aiming state, which waits 40 frames before it
  fires. From V3 the update sets the item's lock-on bit itself.
- **Aim** (`lob_velocity`, 0x4C3888, shared with the gloves, the Holo Shield and the
  Infecto-Bomb): the canister leaves from 0.28 u right and 0.85 u below the gun's origin
  in its frame, aimed at the lock-on target (its centre raised by its height), else at a
  point 10 u along his aim at the height of his feet. It crosses the ground at 0.25 u a
  frame (15 u/s), its upward speed solved to land on the point under 0.00405 u a frame²
  (14.6 u/s²). With the camera aiming (a button held in the lock-strafe camera) the point
  is where the camera's aim crosses his ground plane, kept between 1.6 and 10 u.
- **The canister** (class 0xE64) falls under the same gravity, its sweep 0.3 u round. It
  bursts at once if anything targetable comes within 0.8 u (the sweep plus 0.5, not the
  hero). Off the world it bounces, a random 3–6° kick to its spin (corrected
  2026-10-31). Its fourth touch of anything but a ceiling sets it rolling: its velocity
  loses its part along the ground's normal and takes 0.4 × 3° = 0.0209 u a frame
  (1.26 u/s; 1.6 times that for the Heavy Bouncer), its speed and spin then × 0.9 a frame,
  and it bursts 30 frames later. It has no life limit (its countdown only drives its
  trail) and passes through classes 0 and 0xE14 [code, 2026-10-31].
- **The burst** (0x31D6C0): 17 bomblets (class 0xECE), each at a random bearing, 50–90°
  up from the ground's plane, at a random 4–9 u/s.
- **A bomblet** falls under the same gravity, its sweep 0.125 u round, and lives a random
  240–360 frames:
  - each bounce turns its ground velocity towards the nearest living enemy (class
    category 5) within 7 u, keeping its speed (at least 4 u/s) and its bounce;
  - it is armed after 70 frames, plus its index (so 70–86): then a 2 u ball round it
    looks for the first target with hit points left, which takes the row's damage
    (+0x6C), halved for some big enemies' classes (0x19DF, 0x19F1, 0x1BC5 and eight
    more), pushed by the bomblet's velocity, and the acid check; it bursts on that hit;
  - at the end of its life, or on some surfaces, it bursts with no damage.
- **Bounces** [code, 2026-10-31]: the canister, its bomblets (and so the Annihilator's
  mines), the Infector's blob, the Miniturret and a few other shots go through one
  resolver (0x4C42C0): a 0.1 u-ish sphere at the step's end, else the step's line; on a
  surface steeper than its threshold (or always, threshold 1.5) the velocity is reflected
  about the normal and only its part along the normal is scaled by the shot's keep, the
  part along the surface kept whole. The canister keeps 0.45 (0x1DC8B0; its threshold
  drops to 0 for the fourth touch), the bomblets 0.95 (0x1DC8E0) and always bounce. Both
  burst at once on surfaces whose material class (0x4C73B0) is 1 (materials 1, 3, 0xB,
  0xC) or 2 (0, 0xD, 0xE, the liquids).
- **Mods:** lock-on from V3 [code]; acid from V2 [text], as the Plasma Coil's (2 a second
  for 4 s; V3–V4 3 for 6 s).
- **V5 and up** (the Heavy Bouncer, class 0xE9E, the same update; in the kit): the canister's sweep
  0.5 u; the bomblets leave 40–75° up at 4–12 u/s, their sweep 0.25 u, and their ball
  7 u. The Annihilator's missile scatters the same bomblets at V2–V4, not V5 (corrected
  2026-10-22; "Annihilator").

### Miniturret Glove (0x346078, turret 0x32AFB0) [code, data, UYA NTSC, 2026-10-22]

- **Throwing:** a Circle press takes a round and plays the throw (`hero_fire_anim`); 20
  frames later the turret leaves the hand, and 10 frames after that the glove can throw
  again. In the look it waits 40 frames in the aiming state, as the Bouncer does. The
  throw is lobbed as the Bouncer's canister is (`lob_velocity`): at the lock-on target
  or 11 u ahead, kept between 1.5 and 11 u, 15 u/s across, under 0.0030556 u a frame²
  (11 u/s²).
- **The turret** (class 0x1366) falls under 11 u/s² and lands where its sweep meets the
  ground (off another turret it bounces back faster, × 1.1). It takes a slot among the
  turrets (0x276EE0; when none is free it goes), unfolds over 6 frames of its animation,
  then:
  - looks all round within 15 u (35 u with 0x1DB030 set) for a target
    (`gun_aim_target`);
  - turns its barrel to it, the turn's share growing 0.0262 a frame (about 38 frames,
    eased), and fires once past 0.9, every 1 / (+0x50) s (6 a second, 10 frames; 1.5 at
    V5), at the target's centre;
  - each shot (a slot in 0x277110, up to 100) flies straight at row +0x4C = 30 u/s until
    its step's line hits something; a target (not the hero, not a turret) takes the row's
    damage (+0x6C), pushed along the shot, flags 0x10000, or 0x10010000 from V3 (the ice
    shots), and the shock mod's check for item 0x15 (whose bit no code read sets);
  - a target that dies or goes sends it looking again.
- **Its end:** after row +0x54's shots (20 at V1, 30 from V2), 1200 frames (20 s) after
  landing or its last shot, or more than 30 u from the hero, it bursts (0x4488B8 at size 0.5
  with a light of radius 13: `effects.md`, "The small blast") and goes. The 1200 is set at
  its spawn (0x32C320) and so runs in flight too (2026-10-31).
- **Its barrel** [data, class 0x1366 on the disc, 2026-10-31]: its shots leave point 1
  (joint 9), unfolded 1.08 u above where it stands and 0.72 u along its aim.
- **How many** [code, 2026-10-31]: the glove refuses a throw while 9 or more turrets have
  landed (the count at 0x277100, tested in 0x3462F0); and only the 5 nearest the hero
  (0x32C558 sets the 5, 0x4C7228 picks them) turn and fire, the others folding up until
  they are among them; their countdown runs on meanwhile. Its shot pushes along itself at 1 (the unit direction).
- **V5 and up** (the Megaturret Glove, class 0xA65 (0x343088) and turret 0x1367,
  0x32DE00): the glove throws as the Miniturret's does (1.5–11 u, 15 u/s across,
  0.00306 u a frame²), the throw leaving 20 frames after the press, refused with 6 or
  more landed, again only the 5 nearest active (0x32F5F0).
  The turret:
  - finds a target all round (0x32ECC0, 0x44CF68) and turns to it (0x32EEE8);
  - fires a guided missile (0x32EFE8) when its count (+0x5C, set to 0.4167 s after each)
    has run out, from its two barrels in turn (points 2 and 1: joints 10 and 14, 1.08 u up,
    0.57 u along and 0.2 u either side, class 0x1367 on the disc): the shared small missile (0x3441D0, class
    0xA82, flight 0x343330) of the row's damage, starting at 2 u/s and gaining 0.5 u/s a
    frame to 15 u/s, after its target, living 270 frames; its sphere (0.25 u) or step
    meeting something sets it off, everything within 1 u taking the damage (flags
    0x110000). After its shots (+0x60) it bursts (state 8, 0x4488B8) as the Miniturret;
    its life is 1600 frames (0x1DCF08), set again after each missile; it turns as the
    Miniturret does (0x32F208: 0.0262 a frame, eased, firing past 0.9), and its missiles
    turn as "The small missile" reads;
  - every 180 frames (then 180–240), with the hero more than 20 u away and an enemy
    within 20 u of him, it picks a point 3–6.5 u round him and goes there (states 10,
    11; 0x32E950, 0x407010): it doesn't walk but fades out (alpha −2 a frame for 45
    frames), moves to the point, and fades back in (+3 a frame up to 128 over 45 frames),
    with sparkles (read 2026-10-31). The point is 1 u over the ground at his height or
    above, and it stays unless a ray 10 u down from there meets ground that isn't a
    liquid. No leash ends a Megaturret: only the Miniturret tests its 30 u.
  The same small missile serves the agents' cannon (0x3C0128) and the Annihilator's V5
  bomblets (0x396A20).

### Shield Charger (0x3410E8, firing 0x340DD0) [code, UYA NTSC, 2026-10-22]

- **Firing** (`shield_charger_fire`, 0x340DD0, from the hero's item code at 0x4E8830):
  with no shield up, it takes a round of item 0x16 and raises the shield, a moby of the
  gun's class (0xA5C; 0x1192 from V5) kept 0.75 u above his feet; with one up it only
  plays a refusal.
- **Its points** (0x4C5830, `shield_points` 0x1E11C0): 225 at V1, then 75 more a level
  (300, 375, 450, 525, 600, 675, 750 at Omega).
- **Taking hits** (0x4C57B0, from the hero's damage, 0x4E9AD8): a hit's damage comes off
  the points, and only what's left over once they run out reaches the hero; then for 15
  frames every hit is taken by the shield for nothing.
- **XP** (2026-10-31): each frame its points differ from what it saw, the difference as a
  whole number × 32 goes to the gun's word (0x142E38). So 225 points are 7200 XP.
- **It goes** when its points run out, or (0x342FB0, read 2026-10-31) the hero is in state
  0x31 (49, the vehicle), or state type 0x14, or critter mode (0x1A71C4), or has no health
  (0x1A7430), or his moby's class isn't 0.
- **V1–V4:** every 0.5 s (the first at once) a 0.9 u ball round its centre finds the
  living enemies (class category 5) touching it; each takes the row's damage (+0x6C),
  pushed straight out, flags 0x10000.
- **V5 and up** (the Tesla Barrier): in place of the zaps, a frame at a time, while
  fewer than four bolts are out, one living enemy within 4 u that has none gets a
  lightning bolt (0x34DAA0, the row's damage when it strikes), and that bolt's slot waits
  a random 20–60 frames before its next (corrected 2026-10-28: not the bolt's life). After
  its 40th bolt (0x1DD29C) the shield goes. A shield of the V1–V4 class still up once the
  gun is V5 is replaced by a V5 one, for nothing, when its points run out.
- **Its colour** flashes on a hit and fades back (corrected 2026-10-22: it doesn't run
  with its points): a timer set to 30 frames whenever its points change (0x1DD2D4)
  counts down, and the colour is rest × (1 − t) + hit × t with t = timer / 30
  (`colour_lerp`, 0x40D020). Pairs (rest, hit; written R in the low byte): V1 0x202090,
  0xA02020; V2 0x204090, 0xA04020; V3 0x205020, 0x3030A0; V4 0x905020, 0x2040A0; V5 and
  up 0xA06565, 0x3030A0. Each point it absorbs is a point of item 0x16's XP (damage × 32
  at 0x142E38).
- **Its look** (`shield_draw`, 0x341C10, a draw callback the update registers each
  frame; only in game mode 0): the class's own mesh is never drawn (its moby keeps mode
  bit 0x01, medium confidence), and nothing grows it in or fades it out. It draws, round
  hero point 10 (joint 2), all from the level's effect textures (`effects.md`):
  - five crackling arcs, loops out from the centre and back, each laid at random and
    living 3–45 frames, keeping its shape (corrected 2026-10-29: not jittered each
    frame), as strips of texture 0x17 (V1–V2), 0x0F (V3–V4) or 0x30 (V5) in grey
    0x7F7F7F inside a glow four times as wide in an edge colour by level, fading with
    their life (`effects.md`, "Shield Charger");
  - a shell: a camera-facing dome of 290 vertices (0x2A5AE0, 0.3 u, × 3: 0.9 u) in
    texture 21, the shield's colour at alpha 0x18, then a rim of 32 additive quads from
    0.9 to about 1.32 u in texture 11 at alpha 0x40;
  - four trails of 20 points orbiting 0.9–1.1 u out, turning 6° a frame (10° at V5),
    each head throwing two sparkles, a core 0.04 wide in a glow 0.14 wide of texture 0x2C
    (V1–V2), 0x0F (V3–V4) or 0x30 (V5), fading in from the tail;
  - from V3 a camera-facing glow in texture 0x17 at alpha 0x1F whose texture scrolls 0.1
    a frame;
  - and from the update, eight sparkles on the 1 u sphere (texture 0x3E) and 8 pairs of
    zap sparks at each enemy it zaps (0x342DB8). Sound 1 plays as it rises, 3 loops while
    it is up and 2 as it goes; it is removed at once.

### Holo Shield Glove (0x3717A8, shield 0x376B88) [code, UYA NTSC, 2026-10-22]

- **Throwing:** a Circle press (or R1 in the aiming state) with fewer than four shields
  out (0x1E1150) takes a round and plays the throw; 20 frames later the shield leaves
  the hand (0x376960), and 60 frames after that the glove can throw again. Lobbed as the
  Miniturret is (11 u/s², 15 u/s across), at a point 1.5–5 u ahead (8 u in the aiming
  state). The glove's bangles: 1 at V2, 3 at V3, 7 at V4.
- **The shield** (class 0x186D) falls until the line of its step meets something. Where it
  lands, a 1 u ball takes 5 damage, pushed by its velocity × 3 (flags 0x10000); landing on
  a moby that isn't an enemy, it rides along with it. It takes a slot, unfolds (its
  scale growing 0.06 and 0.04 a frame to 2 and 1.2) and stands:
  - it stands 900 frames (15 s), until its hits run out, or until he is more than 35 u
    away; then it folds over 20 frames and goes;
  - its hits: 10 × 0.75 = 7 (+0x26), less each hit by the hit's own count (+0x54 of its
    record); each hit it takes gives the glove XP at its 30-frame reaction's start: its
    level's XP to the next level (row +0x68) × 32 / 150, to the word 0x142F7C (2026-10-31);
  - each hit plays a 30-frame reaction; from V3 (V3 and V4), at its 15th frame, a
    lightning bolt (0x34DAA0) strikes the first living enemy within 7.5 u; from V5 (the
    Ultrashield), every 10th frame of it, a pulse (0x372338, class 0x1869) of a quarter
    of the row's damage goes at the first enemy within 16 u that this shield hasn't sent
    one at in 35 frames (0x4C46A8 asks the table 0x24FA60, 0x4C4640 adds; no test of sight:
    read again 2026-10-31, "that it can see" was wrong).
    Its flight (0x371F48) gains 0.4167 u a frame each frame (25 u/s a frame); the line
    of its step strikes the first thing it meets (flags 0x10000, pushed 0.0667 along its
    way); its damage fades to nothing over frames 32–38, when it goes;
  - from V2, while it has hits left and the hero's health is under its maximum
    (0x142668), it drains: 0x37BB28 links it to every moby within 6 u of it (up to 12
    links, drawn as wavering beams), and 0x37BDC0, while a living enemy (not of classes
    0x1AE6, 0x1A17, 0x1B27) is linked, gives a linked enemy 1 damage (flags 0x10000) each
    time a shared timer of 180 frames runs out (it counts down once a frame for each
    linked enemy), and, with the hero linked and within 5 u, every 45 frames gives him a
    point of health (its +0x54, 1), capped at his maximum, and when he took it the shield
    loses that many hits.
- How it stops enemy shots is in the shots' own code (it is a moby that blocks them).

### Agents of Doom (0x35C9F0, pod 0x35CEC8, agent 0x3BD298) [code, UYA NTSC, 2026-10-22]

- **Throwing:** a Circle press (or R1 in the aiming state) with fewer than eight agents
  about (`agents_alive`, 0x1DD728) plays the throw; 20 frames later it takes a round and
  the pod (class 0x1853) leaves the hand, lobbed as the Miniturret is, 1.5–4.5 u ahead
  (11 u with the first-person camera). 10 frames on the glove is ready, but a throw
  waits 45 frames from the last.
- **The pod** lands and lets out four agents (class 0x1ADA, 0x3C0F78), 0.25–0.75 u to
  his front, back, left and right of where it is, each only while fewer than eight are
  about. From V4 four agents come at once to the hero instead of a pod.
- **Jetting in** (V4 and up; read 2026-10-31): an agent spawned while one of the four slots
  is free (`hero_slots_update` 0x4C5220 places them at 0x1E2240 each frame; 0x1E2280 says
  whose) starts on that slot, invisible, in state 0x10, turning and sparking, its alpha
  growing, for 30 frames, then state 7 hovers there. One spawned with none free walks.
  - **The slots:** a vector from his left, turned 36° at each step about his facing, so
    over his head: 36° out 1.475 u from 0.5 u behind him; 72° out 2 u (0.8 u with the
    lock-strafe camera), 108° out 2.2 u, both from 1 u behind him; 144° out 1.475 u from
    0.5 u behind him.
  - **Hovering** (state 7), it springs to its slot (0x445208): farther than 10 u from
    him, gain 4/3600 and top 10 u/s, through the world; nearer, gain and top drawn each
    frame (0.5–1 /3600, 3–6 u/s), and not if a line 0.2 u past its step meets the world.
    It faces its target or slot. It neither searches, chases nor ends at a target; its
    life and far ends still run. Its gun fires.
  - **Landing:** with the shared rounds gone (120 at V4, 40 from V5) it gives up its slot;
    state 8 springs (gain 1–2 /3600, top 3–4 u/s) to where a line from it, 5 u along his
    facing and 10 u down, meets something, else to him. Within 0.1 u it goes on foot,
    state 3 (following).
  - **In the kit:** all of that but the spin and alpha, the 0.8 u with the camera and its
    facing while it hovers; and it isn't drawn jetting.
- **An agent** (class 0x1ADA; spawned by 0x3C0F78, its update 0x3BD298, about 1500
  lines, read in part):
  - **sight** (corrected 2026-10-22): as it comes out it takes anything with health
    within 25 u and 5° of level (`target_search_level`, 0x44CD90; not class 0x18AD);
    then each frame without a target, an enemy within 30 u and 5° of level that no one
    has claimed. There is no cone round its facing: the search scores distance × (1 +
    the angle off a fixed bearing, 20°), so in practice it takes the nearest. A target
    found is claimed for 60 frames in the hero's claim list (`target_claim` 0x4C4640,
    32 entries at 0x24FA60), which other agents and the Plasma Storm skip
    (0x4C46A8). Some classes are never its target (0x1A17, 0x1B27, 0x1AE6, 0x107E,
    0x1D87). With a target it chases (state 5) for 420 frames.
  - **moving** (`walker_set_params` 0x458A70, run by 0x4577D0): it turns towards its
    goal, speeding the turn up and easing it to a stop (`angle_approach`, 0x445758), and
    only moves while facing within 120° of it, at min(top, √(2 × acceleration ×
    (distance − 0.5))), stopping 0.5 u short (0.4 wandering):

    | When | Acceleration | Top speed | Turn: acceleration, top |
    |---|---|---|---|
    | As spawned (0x3C0F78); the state's own set this the same frame, so it has no
    "coming out" | 7 u/s² | 3 u/s | 0.2°, 12° a frame |
    | Following (state 3) | 13 u/s² | a random 6.5–8 u/s (+0x310) | the same |
    | Chasing (state 5) | 13 u/s² | a random 5.5–6.5 u/s (+0x314) | the same |
    | Wandering (state 1) | 1 u/s² | 0.25 u/s | 0.1°, 1° a frame |

    It also turns itself towards its waypoint or target at 0.5° a frame², up to 30° a
    frame.
  - **by the hero:** idle (state 0) it follows once he is more than 4 u away; on frames
    30, 35 and 59 of its idle animations (`anim_frame_reached`, 0x44AF40; once read as
    distances) it starts wandering one time in three if it has no target. Wandering, it
    walks to random points 2–6 u round him and follows once farther than 5.5 u.
    Following, it finds a way by casting rays fanned 10° apart up to 160° either side
    (0x3C1620, up to 12 u), plans again when he has moved 1 u, when it is within 0.75 u
    of its waypoint or 4 u from where it planned, and idles within a random 1–4 u of
    him. Where the ground 1 u ahead falls away it looks 9 u ahead (0x3C1BA8): more than
    2 u lower or a slope of 10° or more ends it (at V2–V4 firing its comet first),
    otherwise it jumps down (state 9 or 10).
  - it ends (0x3C2140) when it comes within 2 u of its target's feet, when a 1 u ball
    0.3 u above its feet touches the target, 420 frames into a chase (+0x344), or 2000
    frames after it came out (+0x144): at V1 a 2 u blast of the row's damage, no push
    (flags 0x1010000); at V2–V4 it fires a comet (0x395480, class 0x1893, its update
    0x394068: a random 10–15 u/s halved, along his facing or at the lock-on target,
    living 680 frames), and whatever the comet meets is struck by a 2 u blast of the
    row's damage, pushed 1 (flags 0x10000); from V5 the Agents of Dread's blast (class
    0x188F, 0x3914C0): a ball growing to 8 u over 20 frames, which each frame strikes
    what it holds for the row's damage, pushed 2 out, with the acid check by item 0x57,
    which has no acid row, so nothing (0x31AC38). The agent then goes.
  - **its gun** (+0x308, 0x453FC0; `agent_tick` 0x3C0128, corrected 2026-10-22: it
    fires from V2, not only V5). At V2–V4, at an enemy within 25 u all round in a clear
    line from its gun's joint 8, its aim jittered ±0.05 u each way, it fires a bullet
    (class 0x1855, `enemy_bullet_spawn` 0x35D8E8) at 25 u/s for 0.03 of the row's damage
    (flags 0x10000): first 27 frames after it came out, then every 30 frames at V2 and
    15 at V3–V4. V2 and V3 give each agent 50 rounds (+0x15E); V4 a shared 120
    (0x1E22B0), refilled by each agent spawned. A bullet (0x35D9C0) flies straight for
    at most 180 frames while within 80 u of the hero, testing each step's line; a hit is
    a 0.6 u ball of its damage (1.0 u on a moby), pushed by its velocity × 3.
  - From V5, in state 7 (hovering by the hero) only, while a shared 40 rounds last:
    every 80–120 frames, at an enemy within 25 u seen from its gun's joint 6 or 7, the
    shared small missile (0x3441D0) of a quarter of the row's damage, starting at 1–3
    u/s and gaining 0.5–0.9 a frame to 12–17 u/s, living 250 frames (0xFA; the 100 once
    noted here is another argument). The Vortex Cannon's shots are the same comet as
    V2–V4's agents, with its own damage ("Suck Cannon").
  - **Where its gun is** [data, class 0x1ADA on the disc, 2026-10-31]: bullets leave point
    8 (joint 57, 0.45–0.5 u above its feet), missiles points 6 and 7 (joints 55 and 56, its
    hands, 0.68–0.73 u up, 0.3 u either side).
  - **Far from him** [code, 2026-10-31]: every 60 frames (the level's frame count, 0x1DFBC0
    % 60) one further than 25 u from the hero fires its comet (V2–V4) and ends. With no
    target, one further than 20 u, or 4 u above or below him, that a sphere test of its
    place (0x40D0C0, 64 u) passes, is set down where the camera's line 10 u ahead meets
    something (0x1E2A40; the hit's material may end it instead).
  - **Its idle** [data, 2026-10-31]: its sequence 0, 240 ticks and 120 of the game's frames
    (`stamp`), so frames 30, 35 and 59 are ticks 60, 70 and 118.
  - Its bangles (+0x7C): 1 at V1, 9 at V2–V4, 5 from V5.

### Infector (0x3688E8, blob 0x3DE4F0) [code, UYA NTSC, 2026-10-22]

- **Firing:** a Circle press (or R1 held in the aiming state), with the gun out more than
  22 frames and 45 frames since the last, takes a round and throws a blob (class 0x1E05,
  0x3DE3A8) from 0.75 u along the barrel from the gun's origin (0.82 from V2; its vector
  0x1DD830 is (0, −0.75, 0) in the gun's frame, where −y is the barrel; corrected
  2026-10-31, it was taken for 0.75 u below), at 12 u/s along his facing (the camera's
  aim in the aiming state and with the lock-strafe or first-person camera).
  The gun's bangles: 1 at V2, 3 at V3, 7 from V4.
- **The blob** falls under 0.00405 u a frame² (14.6 u/s²), loses 0.5% of its speed a
  frame, and bounces (sweep 0.075 u, the bounce keeping 0.89); each bounce turns its ground
  velocity towards the nearest enemy within 5 u, as the Bouncer's bomblets do. It lives a
  random 300–500 frames, and stops once slower than 0.5 u/s.
- **Infecting:** each frame a 1 u ball round it:
  - infects any enemy (class category 5) it touches with 2 health or less, and flies on;
  - touching any other target with health, it bursts.
  It also bursts when its life ends, when it stops, or on some surfaces. The burst
  infects every enemy within 1 u not yet infected, then a 1.5 u blast does 1 damage,
  pushed 1, with damage flag 0x8000000, which infects what it hits.
- **Infected** (the target's damage record +0x2F set): the infection lasts the row's
  damage / the target's health × 600 frames (+0x38 of the record); the infector gains XP
  for each infection, as for a kill.
- **What an infected enemy does** [code, 2026-10-22]: an enemy's damage record is at its
  pvars +0x30, so the flag is its pvars +0x5F, which the enemies' shared code reads.
  Each frame enemies sign up in a roster by team (`enemy_roster_add` 0x4538E0, up to 64,
  swapped in by 0x453930 from the hero's update): a healthy one as team 3, looking for
  the hero and team 1 (signed up by 0x3320A0, probably the Rangers); an infected one as
  team 5, looking for team 3, the other enemies. The shared target finder
  (`enemy_find_target` 0x4539A8, every 8 frames, staggered) scores distance + angle off
  its facing × distance within ranges in each enemy's pvars (+0x2C0–0x2DC); the hero
  scores −20, but 999999 for an infected enemy, which so never targets him. Its shots
  carry flag 0x10000 instead of 1, so they hit enemies as the hero's guns do. Healthy
  enemies never look for team 5: in the shared code they don't fight back, though an
  enemy's own reaction to a hit (not read) might. Never infected: classes 0x188A and 0x1886 (the
  Infecto-Bomb), the hero, and some big enemies (0x19DF, 0x19F1, and nine more, which give
  no XP).
- **V5 and up** (the Infecto-Bomb, class 0x1886, 0x389F58): on the row's rate a press
  takes a round and lobs a bomb (0x38EE78, class 0x188A) with `lob_velocity` at the
  target (the lock-on, else the assist's within 11.1 u), at most 12 u away, crossing the
  ground at 15 u/s under 0.00333 u a frame² (12 u/s²). The bomb (0x38E3A0) falls until
  its collision step meets something, and there bursts (0x38F678, class 0x188B, update
  0x38EF90): on its first frame every enemy within 5 u (not the hero, not the bombs, not
  one already infected) is infected for the row's damage / its health × 600 frames, its
  XP paid at once, and a 2.5 u ball does 1 damage (flags 0x8000000).

### Thrown things off the world [code, UYA NTSC, 2026-10-31]

There is no shared limit on how long a thrown thing may fall; each has its own rule:
- **The holo shield** in flight (0x376B88) and **the rift** in flight (0x373990) go once
  x, y or z leaves the level's box, 2 to 1021.
- **The Miniturret** (0x32AFB0) bursts more than 30 u from the hero (straight-line), and
  its 1200-frame countdown, set at its spawn (0x32C320), runs in flight too. It falls at
  11 u/s², bounces off steep surfaces keeping 0.45 ("Bouncer", the resolver), and lands
  where the surface's up is at least 0.556; it sinks in liquid materials and bursts on
  material class 1.
- **The agents' pod** (0x35D2C0) lets its agents out when its 300-frame countdown (set at
  its spawn, 0x35D178) runs out or its speed drops under 0.5 u/s; landing on material 0,
  1 or 0xB it splashes and goes with no agents.
- **An agent** (0x3BD298) falls at 9.8 u/s² (0.0027222 u a frame², set in 0x3C0F78), a
  spring (0.0111, 0.0111, at most 1 u a frame) lifting it back out of the ground. Every
  60th frame one more than 25 u from the hero ends (with its comet at V2–V4); one
  touching material 0, 1, 0xB, 0xD or 0xE ends; and one not chasing, off screen, and more
  than 20 u away or 4 u off his height is put on the ground under him.

### Spitting Hydra (0x31FB90) [code, UYA NTSC, 2026-10-22]

- **Its numbers by level:** the most targets 3, 4, 5, 6 (V1–V4) and 7 (V5 up); the
  spit's flight 30, 30, 35, 35 and 40 frames; the bangles 1, 3, 7 from V2.
- **Holding Circle** (with ammo, not mid-spit) counts up to 150 frames and plays the
  charge. Every 12 frames held (15 at V3–V4, 10 from V5, and at 150) it looks along his
  facing (the camera's aim in the aiming state and the lock-strafe camera) for targets
  within 32 u and 20° (`gun_aim_target`, at most 8), and marks them, one at a time, up
  to the level's most. The count stays at 150, so from then it looks every frame
  [code, 2026-10-29].
- **What the hold does to him and the gun** [code, 2026-10-29]: nothing calls
  `hero_fire_anim` until it is let go, so he keeps his own animation (with the gun held)
  and isn't `firing` (no double jump is lost); in the look (state 1) it puts him in state
  29, as other guns do. The gun's moby plays its sequence 2 throughout ("The gun's own
  animation"). It loops sound entry 2 from the first frame held, plays 0 as each target
  is marked, and stops the loop as it is let go; a release before 20 frames plays 5 and
  drops the marks (`effects.md`, "Sounds"). Its charge's strips, glow and light are in
  `effects.md`, "Spitting Hydra".
- **Letting go:**
  - before 20 frames: it fizzles (a sound; seven such taps bring up a hint), marks or
    not;
  - with targets marked: it plays the firing animation (`hero_fire_anim`) and a lightning
    bolt (0x34DAA0) strikes each, and a 1 u ball there takes the row's damage (+0x6C),
    pushed 0.5 along the line from the hero, flags 0x410000; this takes no round (only the
    spit calls `ammo_use`);
  - with none: once its spit timer reaches the flight time it takes a round and spits.
  - Either way the spit's timer (set to the flight time while held) then counts down,
    and until it runs out 0x1DC9B0 stays set and holding Circle doesn't charge again
    [code, 2026-10-29].
- **The bolt on a marked target** (class 0x1195, 0x34DAA0; its update 0x34DD38 and strike
  0x34DFD0): it holds between the gun and the target for its life (0x1DD48C: 15 frames
  here), and when its strike timer of 6 runs out (its 6th frame; corrected 2026-10-28,
  `effects.md`, "The lightning bolt") the target, if still there, takes half the Hydra's
  damage again
  (flags 0x410000, pushed 0.5 along it). The same bolt serves the Holo Shield (the shield's
  row damage), the Rift Ripper (a tenth of the rift's) and the Tesla Barrier (the Shield
  Charger's), by its source's class.
- **The spit** isn't bolts that fly: it is the level's number of lightning arcs
  (0x321108 sets their ends, 0x321210 redraws them). Each reaches from the gun to a point
  32 u along the aim, jittered up to 0.2 u on each axis, bowing out round the aim on the
  way (rolled −107°, 0°, 107°… at V1–V2; from −107.5° by 52.5° at V3–V4; from −90° by
  30° from V5; corrected 2026-10-29, the code turns by its first angle twice;
  `effects.md`, "Spitting Hydra"). Its timer counts down from the flight time; when it reads 27 (frame 3, 3, 8,
  8, 13 after the spit at V1–V5), 0x321580 strikes once for each arc within 60 u: the line
  from the gun to its end is tested with a damage record (the row's damage, pushed 0.7
  along it, flags 0x410000), so the first thing it meets takes it, and if it met anything a
  blast of 1 u (2 u from V5) round the arc's end does the row's damage too. An arc that
  meets nothing does nothing.

### Disc Blade Gun (0x36EFC0, disc 0x36FA78) [code, UYA NTSC, 2026-10-31; flight traced at V1 and V5]

- **Firing:** while Circle is held (or R1 in the aiming state), with the gun out more
  than 22 frames, every 30 frames, a round a disc (class 0x1867, 0x36F720). Its target is
  the gun's assist: 20 u (35 from V4), far 30.5° by 30.5°, near 45° by 45° (50 u, 20° by
  10° with 0x1DB030 set). Its way is along his facing, or at the target; in the aiming
  state and the lock-strafe camera, along the camera's aim (0x1E2E40), bent to a target
  within 30 u and 80° (0x4C7010).
  - **V1–V4:** from 0x1DD980 (0.35 up, 1.1 along the barrel), lifted about his left axis
    (0x40CCA0) by 22.5°, or 60.5° with a target, at 0.35 u a frame (21 u/s). In the views
    that aim it isn't lifted. A traced V1 leaves at (0, 0.3234, 0.1339) a frame with him
    facing +y: exactly 0.35 at 22.5°.
  - **V5 and up:** not lifted, 0.7 u a frame (42 u/s), from two muzzles in turn, the flag
    0x1DD9B0: 0x1DD9A0 first (0.4 up, 1.2 along, 0.2 to his right), then 0x1DD990 (the
    same, 0.2 to his left). A traced V5 leaves at (0, 0.700, 0) a frame.
- **Flight:**
  - **With no target,** from V1 to V4 and outside the views that aim (tested each frame),
    it falls 0.00444 u a frame² (16 u/s², 0x4C4190) for its first 38 frames. So it rises
    about 2 u over 30 frames, and goes on 6° below level (−0.0349 u a frame, traced) until
    it meets something. From V5 it flies straight.
  - **With a target,** each frame 0.1 u towards its centre is added to its way a frame,
    which then gets 0.35 u a frame's length, at any level: a V5 disc slows to it. So from
    its 60.5° it curves down on to the target. A target that goes (dead, or no longer an
    enemy) ends the chase, and the fall goes on if its 38 frames aren't up.
  - **The 2° a frame** (0x4475E8, either way, chosen at random, 0x36F720) and the 60° a
    frame about its up (0x40CCA0) only turn its matrix: its look, not its way. The velocity
    is untouched. This reading replaces "turned 90° to one side" and "curves 2° a frame",
    both wrong.
- **Ends** after 200 frames (400 from V4); or more than 81 u from the hero, or outside 2
  to 1021 on any axis. Tested once in 16 frames (by its address), it also ends beyond 50 u
  once 15 discs are out, or at V5 alone with 4 or more splits behind it.
- **Hits:** each frame the line of its step (0x3F8448). If that meets anything:
  - **V1 and V2** on an enemy: it ends, at that point, after a 1 u ball (0x3FB1D0) deals
    the row's damage (+0x6C), pushed by its velocity × 3, flags 0x10000.
  - **V1** on anything else: reflected (0x40C260).
  - **V2** on anything else: reflected, and its way is set at the first enemy listed
    within 10 u (0x3FB1D0's list is in the collision grid's order, not by distance); this
    doesn't make it a target, and any target it had goes on being chased.
  - **V3 and up** on an enemy: **not reflected**. It remembers the enemy so that its line
    ignores it, and goes on through. A wall reflects it as before.
  - **V3 and up,** in both cases: its way is set at the first enemy listed within 10 u
    that the gun's discs haven't been sent at in 500 frames (the table 0x24FA60 by shooter:
    0x4C46A8 asks, 0x4C4640 adds), and the target is dropped. The one just hit may be that
    enemy. No test of sight here, so the old "that it can see" was wrong; so was "bounces
    off enemies". In either case the 1 u ball deals its damage and doesn't end it.
  - **V4:** its end is a 4 u blast of half the damage (0x32FE90).
  - **V5 and up** (the Multi-Disc Gun): going through an enemy while fewer than 15 discs
    are out (0x1DD9E0), its nth split has odds of 1 in 2^((n − 1) / 2) of a new disc of the
    row's damage carrying the count, its velocity a frame given a random 0.1–0.2 on each
    axis (always up) and its speed back; a second roll at the same odds keeps it going,
    else it ends.
- **In the kit:** all of it, but that the fall is decided when it leaves rather than each
  frame; the 50 u end is tested every tick; the ball is tested every tick rather than when
  the line meets something; and "the first listed" is the nearest other than the one just
  hit, since the grid's order can't be had without the game's grid, and a hit always drops
  the target.

### Rift Inducer (0x372990, rift 0x373990) [code, UYA NTSC, 2026-10-22]

- **Firing:** a Circle press (or R1 held in the aiming state), 30 frames since the last,
  takes a round and launches a rift (class 0x186B, 0x373738, size 0.5) from the gun's
  muzzle at a random 60–80 u/s along his facing (the camera's aim in the aiming state and
  the lock-strafe camera), at the lock-on target or the gun's assist (35 u, far 2°, near
  3°). From V3 the update sets the item's lock-on bit itself.
- **In flight** it loses 8% of its speed a frame, so it goes about 14 u, and opens after
  36 frames or where the line of its step meets something (a 1 u ball there is knocked,
  for 0 damage, by its velocity × 3).
- **Open** (state 2), for 600 frames (10 s), or until he is more than 30 u away:
  - a value grows 0.005 a frame to 0.75 (+0x80); the pull doesn't use it (read again
    2026-10-31; what it drives isn't read);
  - it looks within its size + 9 u (9.5 u) for an enemy, or a damageable moby, not of a
    few classes (0x1F8, 0x1699, 0x165C, 0x1612, 0x16FA, 0x16CB), whose health is at
    most twice the row's damage × its size, so at most the row's damage: it takes that
    one (0x3738B0), springs it in to its middle by the pull (0x445208), its parts
    spinning, and swallows it. One at a time; one dragged more than 50 u away is let go;
  - **the pull** [code, 2026-10-31]: at its spawn the rift draws k0, a random 7–23, and
    k = k0 × size × 1.5 (5.25–17.25). Each frame, with d the target's distance from its
    middle, the speed v is kept within ±d, moves by (k / 3600)(d − v), is kept within
    ±k / 60 and ±d again, and carries the target v closer; it snaps to the middle under
    0.01 k / 60 left and is swallowed within 0.1 u (about 36–111 frames from 9.5 u). Held,
    it turns +0.3, +0.1 and −0.2 rad a frame about its axes and shrinks × 0.99 a frame for
    its first 30 frames, its parts pulled by the same spring;
  - enemies with more health are left alone [code, as read].
- **V5 and up** (the Rift Ripper): each frame, each enemy within its size + 4.5 u has a 1
  in 18 chance of a lightning bolt (0x34DAA0) of a tenth of the rift's damage.
- The row's second damage (5) isn't read here.

### Suck Cannon (0x3619C8) [code, UYA NTSC, 2026-10-22]

The gun uses no ammo: what it sucks in is its ammo.
- **It holds** 10 things at V1, 12 at V2, 15 at V3 and 20 from V4 (`suck_capacity`,
  0x1DD7D0). Its bangles: 1, 3, then 0x13.
- **Sucking:** while Circle is held (the gun out more than 15 frames), a vortex of ten
  1 u links runs from the muzzle along his aim (the camera's in the look), shortened where
  a ray meets the world (0x363E98). Every fourth frame, while it has room, balls of 1.2,
  1.4, 1.7 and 1.9 u at its points 2, 4, 6 and 8 (point 0 the muzzle; 0x28E420, 16 bytes
  each; so about 2, 4, 6 and 8 u out when it is straight; read 2026-10-31) look for an
  enemy or a damageable moby (not classes 0, 0x1F7, 0x1F8, nor with the hero in states
  0x35 and 0x36): one whose record allows it (+0xF ≠ 0, the only test; no health limit)
  is held (0x363088), and takes a 1-damage hit with flag 0x8000000 only if its record's
  +4 is above 1; crates are taken too (0x362440).
- **Firing:** let go with something held, the gun turns to firing; then each Circle press,
  30 frames apart, fires one held thing (0x3623E8) at 40 u/s along his aim, at the gun's
  assist target (35 u, far 7° by 8°, near 45° by 35°; with the lock-strafe camera the
  camera's aim and its target) as a shot (0x382830) carrying the row's damage; from V5
  another shot (0x395480). Empty, it goes back to sucking.
- **The shot** (class 0x1872, spawned by 0x382830, its update 0x381B98) carries the row's
  damage and flags 0x10000 (0x110000 from V2). It flies straight at 40 u/s, and with a
  target is also pulled 0.3 u a frame at the target's middle. Each frame the line of its
  step (or failing that a 0.2 u ball round it) is tested with a damage record (pushed by
  its velocity):
  - meeting the world: at V1, or if what it holds is of class 500 or 0x1FF, a 1 u blast
    there of its damage, pushed by its velocity a frame × 3, and it goes; from V2 it
    bounces off (its velocity reflected), which is the ricochet;
  - meeting a moby other than the hero: a 1 u blast there, pushed × 2, and it goes;
  - after 150 frames it bursts where it is in a 0.6 u blast.
  Its end plays debris or an explosion by what it held (0x32FE90 is the explosion's look).
- **The Vortex Cannon's comet** (V5 and up; 0x395480, class 0x1893, its update 0x394068;
  read 2026-10-31): each Circle press sends it from 0.5 u along the aim instead of the
  shot:
  - its way a frame (0.667 u along his aim) has a random ±0.15 u on each ground axis, then
    a speed of a random 10–15 u/s, halved, so 5–7.5 u/s; at the lock-on target if any;
  - it is pulled 0.3 u a frame at its target's middle (its assist's or the camera's) and
    falls at 0.5 u/s² (0x1DE104), for 680 frames;
  - meeting an enemy (not the hero, not class 0x5C9): the row's damage, pushed 1 away from
    the hero (flags 0x10000 with the row's), the acid check by item 0x87 (0x31AC38; the
    Suck Cannon's rows have no acid, so nothing), and a 3 u blast (0x32FE90) of its damage;
  - meeting the world, or anything else: the 3 u blast;
  - then it stays 15 frames as a look (0x395700), striking nothing.
- **"V3 burns"** (the vendor's text): from V2 its damage record's flags gain 0x100000, the
  same bit that the Lava Gun's and Nitro's hits carry (0x110000). The shot's update has no
  acid call; what the flag does is each enemy's code's. The kit has no such flag.
- **In the kit:** all of it, but that the comet's end shows only the impact, and its
  flags and class exception aren't there. The ricochet from V2 is in.

### Qwack-O-Ray (0x37DEB0, beam target 0x37D7D8) [code, UYA NTSC, 2026-10-22]

The gun uses no ammo.
- **The beam:** while Circle is held (in the aiming state after its 40-frame wait), a
  beam from the muzzle along his aim (the camera's in the look) picks an enemy each frame
  (0x37D7D8): one within 2.5 u, within 60° of his facing and 2 u of his height, or else
  the nearest within 12 u whose bearing is within 30° of the beam's, with nothing in the
  way (classes 0x1BC3, 0x189B, 0x1D3E, 0x1A90 are never picked).
- **Turning a target into a duck:** a new target is held at least 30 frames. Its duck
  meter starts at its health (its record's +0 over +4), and falls by the row's damage a
  second (50 at V1, `qwack_damage`, 0x1DDBC8) while the beam is on it; at 0 the enemy
  becomes a duck (0x3D2480 at V1–V3, 0x39E5D8 at V4, the Qwack-O-Blitzer's 0x3D8840 from
  V5). Letting go or losing the target starts again.
- **The enemy is gone:** the duck's spawn removes the enemy (0x436220) and a duck moby
  stands where it was, facing away from the hero. Before that, for an enemy whose class
  has flag 0x1000 (+0x34), it calls `moby_killed` (read in 0x39E5D8; 0x3D2480 and
  0x3D8840 call it too), so the duck counts as a kill and its XP goes to the gun
  (2026-10-31).
- **V1–V3 ducks** (class 0x1BC3, spawn 0x3D2480, update 0x3D1728): it waddles about,
  idling with three animations picked at random (20%, 20%, 60%) and walking off, turned
  up to 45° either way, for 400–600 or 180–360 frames at a time. With the hero within 2 u
  it slows and turns. It lives 1400 frames, then goes in a puff of feathers (0x3D2870);
  damage of the player's (mask 0x8010000) ends it sooner. Its walker (`walker_set_params`
  at its spawn): 2 u/s², top 0.75 u/s, turning 0.2° a frame² to 12° a frame
  (2026-10-31).
- **Eggs from V2** (0x3D22B0): at frame 15 of its third idle animation a V2 or V3 duck
  lays an egg 0.75 u up, 160–200° off its facing, 10–50° up, at 2–5 u/s. The function
  also sets a 60–120 frame count on the duck that its update doesn't read.
- **The egg** (class 0x1C59, spawn 0x3D4768, flight 0x3D48B8): it falls under 0.00405 u
  a frame² (14.6 u/s²), its speed kept 0.995 a frame, bouncing through the resolver
  ("Bounces"; it keeps 0.89 of its velocity along the normal, 0x1DF440, read 2026-10-31). It lives 360–500 frames
  and then pops harmlessly. Once armed, an enemy (not a duck) within 2 u sets it off:
  that one enemy takes the row's full damage (flags 0x1030000), pushed 1 along the egg's
  way. **From V3** each bounce turns its ground velocity at the nearest enemy within 7 u,
  keeping its speed along the ground, at least 4 u/s (2026-10-31).
- **V4 ducks** (class 0x189B, spawn 0x39E5D8, update 0x39DB60): it waddles the same, but
  each time an idle animation ends a 60% roll looks for the nearest enemy anywhere
  (0x44CD90, range 999) and runs at it, laying an egg every 60–120 frames (0x39EF18).
  It waddles at 0.0042 u a frame (0.25 u/s) and runs at 0.0417 (2.5 u/s), each step tested
  with a 0.5 u sweep (0x4425F0; read 2026-10-31); its idle animations are 62, 122 and 92
  ticks.
  It has no time limit. Where its run ends (that step's sweep meets something), when it
  is hurt, or when it falls off, it blows up (0x39E8A0): enemies within 3 u take a fifth of the row's damage (flags
  0x1010000).
- **The Qwack-O-Blitzer's duck** (class 0x1D3E, spawn 0x3D8840, update 0x3D7998, each
  frame 0x3D8420): it flies. It waits until a shared 60-frame count runs out, then keeps
  by the hero, at slot 1 of the points round him (0x1E24A0, set by 0x4C5220 as he stands:
  1 u behind him, 2.8 u above and 0.9 u to his left; 1.1 up and 0.4 left while the flag
  0x1A6290 is 1; 2026-10-31): at 10 u/s when more than 50 u off, else 3–6 u/s, and 6–8 u/s on its way
  back. Each frame it looks for an enemy within 30 u, in view (0x44CD90, 1.05 rad), and
  flies at it at 13 u/s (0.2167 u a frame). Within 0.75 u it pecks every frame, a 2 u
  ball taking a fifth of the row's damage (flags 0x110000), and looks again. After its
  11th peck it blows up (0x3D9008) as the V4 duck does. A count of 3000 set at its spawn
  isn't read in its update.

### RY3NO (0x3805B0) [code, UYA NTSC, 2026-10-22]

- **Targets** each frame: `gun_aim_target` along his aim (the camera's with the
  lock-strafe camera) within 48 u (60 u at V5), far 25° by 25°, near 45° by 45°, at most
  8 (16 from V2), each marked on screen.
- **Firing:** while Circle is held, the gun out more than 22 frames and the cooldown
  run out, one round fires a volley of 8 missiles (16 from V2) from the muzzle; each
  takes the next marked target in turn (0x4C4820), else the lock-on target, or anything
  within 1 u of the muzzle; at V1 the missile is 0x382E18, from V2 0x3DAEA8 (with 2, 5 and
  0.6 among its arguments). The cooldown is 1 / (+0x50) = 1 s, 1.3 s from V2. In the look
  it waits 40 frames in the aiming state.
- **Each missile** leaves from the muzzle along the line to its target's middle (or his
  aim). V1's (class 0x1873, 0x382E18, update 0x383370) flies at row +0x4C = 50 u/s;
  later levels' (class 0x1DCE, 0x3DAEA8, update 0x3DB4D0) at 0.8 of that, after waiting
  a random 0–30 frames at the gun. Each frame it spirals round that line: its heading is
  tilted off it a random 4–10° (50–70° from V5), and rolled round it by an angle that
  starts random and turns a random 6–10° a frame either way. Within 5 u of its target it
  also steps its speed straight at the target's middle. Its step's line is tested with a
  damage record (the row's damage, pushed along its heading tipped 1.5 up, flags
  0x1030000), ignoring the hero:
  - meeting a target or the world it bursts. V1's blast (0x383CC0): every moby within 4 u
    but the one it hit takes the splash (row +0x70), pushed 4 out and 6 up (0x4445E0,
    flags 0x1010000). Later levels' (0x3DC180): every moby within 4 u (1 u from V5) takes
    the row's damage, flags 0x110000; an enemy it would kill instead gives its bolts there
    and then and dies by 0x3154D8; with nothing in it, a 1.25 u blast of the damage
    (0x3DBDA0);
  - after 300 frames (120 from V5), or more than 60 u from the hero, it goes with no blast.
- **V5** (the RYNOCIRATOR): the volley has no targets and all go at a point 10 u ahead
  and 3 u up; the first also sets off a big blast of its own (class 0x1DF6, 0x3DD390, not
  read further).

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

## Deadlocked's guns [code, data, DL NTSC, 2026-10-31]

Read from the memory image `dlntsc-dreadzone.ee.bin` (a savestate on DreadZone Station).
Where the code is: `class_update_table` at 0x3C5DFC, 12-byte entries of pointer, class and
update, as in UYA. `GadgetDef`s at 0x3BF460, 0xB0 a gun, by libdl's item ids.

| Id | Gun | Class | Update | +0x48 | +0x4C shots/s | Row ids |
|---|---|---|---|---|---|---|
| 1 | Wrench | | | | | 1–8 |
| 2 | Dual Vipers | 0x1094 | 0x3EB940 | 60 | 10 | 9–18 |
| 3 | Magma Cannon | 0x1087 | 0x408260 | 80 | 2 | 19–28 |
| 4 | The Arbiter | 0x1864 | 0x41D370 | 60 | 0.66 | 29–38 |
| 5 | Fusion Rifle | 0x1096 | 0x413EA0 | 1600 | 0.7 | 39–48 |
| 6 | Hunter Mine Launcher | 0x2094 | 0x43EF98 | 1600 | 0.5 | 49–58 |
| 7 | B6-Obliterator | 0x108A | 0x40B498 | 0 | 0.75 | 59–68 |
| 8 | Holoshield Launcher | 0x10A5 | 0x3F50A0 | 0 | 1 | 69–78 |
| 9 | Mini Turret Launcher | 0x24F2 | 0x46E820 | 35 | 7 | 79–88 |
| 10 | The Harbinger | 0x2613 | 0x475618 | 60 | 0.38 | 89–98 |
| 15 | Scorpion Flail | 0x2106 | 0x44ECC0 | 1 | 2 | 103–112 |

Items 11–14 (classes 0x20F7, the EMP 0x20F9, the Hacker Ray 0x20FA, the Swingshot 0x18DF)
have a row each and aren't guns. The gadget box is hero+0x26E4; a gun's entry in it is
+0xA30 + 0x44 × item: its level at +0, its omega mod at +0xC (`FUN_006c0cb8`), its ten
alpha mods at +0x14 (`FUN_006c0c60` counts one kind).

**Levels.** A gun has ten rows (V1–V10), found through `gadget_level_index_table`
(0x3BE158, a word for 10 × item + level) in `gadget_level_defs_table` (0x3BE5B8, 0x20 a
row: +0x04 XP, +0x10 and +0x14 the damages out of multiplayer, +0x18 and +0x1C in it; libdl's
`GadgetDefsEntry`). `FUN_006be7d8(item, level, which, scale)` is `damage[which] × scale`,
`which` moved up by 2 in multiplayer. Multiplayer's rows have values for V1 and V2 only.

| Gun | Damage V1→V10 (the two damages are the same) | XP to leave V1…V9 |
|---|---|---|
| Dual Vipers | 1, 1.75, 2.5, 3.25, 4, 4.75, 5.5, 6.25, 7, 8 | 200, 500, 900, 1500, 2300, 3300, 4600, 6200, 8200 |
| Magma Cannon | 6, 8, 10, 12, 14, 16, 18, 20, 25, 30 | as the Vipers |
| The Arbiter | 60, 80, 100, 120, 140, 160, 180, 200, 220, 250 | 300, 900, 1900, 3400, 5400, 7900, 10900, 14400, 18400 |
| Fusion Rifle | 50, 65, 80, 95, 110, 125, 140, 155, 170, 200 | 100, 300, 700, 1300, 2200, 3400, 5000, 7000, 9500 |
| Hunter Mine Launcher | 12, 16, 20, 24, 28, 32, 36, 40, 44, 50 | 100, 250, 500, 900, 1450, 2150, 3150, 4650, 6650 |
| B6-Obliterator | 20, 25, 30, 35, 40, 45, 50, 55, 60, 75 | 150, 400, 800, 1350, 2050, 2950, 4050, 5350, 6850 |
| Holoshield Launcher | 12, 16, 20, 24, 28, 32, 36, 40, 44, 50 | 50, 110, 190, 290, 410, 550, 720, 920, 1170 |
| Mini Turret Launcher | 5, 6, 7, 8, 9, 10, 11, 12, 13, 15 | 150, 450, 950, 1650, 2550, 3750, 5250, 7050, 9150 |
| The Harbinger | 500, 550, 600, 650, 700, 750, 800, 850, 900, 1000 | 200, 800, 1800, 3200, 5000, 7200, 10000, 13000, 16800 |
| Scorpion Flail | 40, 50, 60, 70, 80, 90, 100, 110, 120, 150 | 300, 800, 1500, 2400, 3600, 5100, 6900, 9000, 11500 |

The last row's XP is 100000 (10000 for the Holoshield Launcher), which never comes. Past V10 (level
above 9 with `FUN_005688a0`, challenge mode, true) a gun stays on V10's row and adds
(level − 9) × the word at 0x3BE47C + 0x10 × item: Vipers 0.1, Magma 0.3, Arbiter 2.8, Fusion
2.3, Mines 0.6, B6 0.9, Holoshield 0.6, Turret 0.1, Harbinger 11.2, Flail 1.7. Levels run to 0x62,
V99, where the Vipers and Magma Cannon take other looks.

**Definitions** (`GadgetDef`, read from the table):

| Id | Gun, V10 name (then "Mega" and that at V99) | Most ammo (mp) | A mod of Ammo adds | Knockback | Full-body fire sequence | Price |
|---|---|---|---|---|---|---|
| 2 | Dual Vipers, Dual Raptors | 200 (300) | 20 | 1 | 65 | 2,250,000 |
| 3 | Magma Cannon, Vulcan Cannon | 20 (30) | 5 | 2 | 61 | 1,750,000 |
| 4 | The Arbiter, The Silencer | 8 | 2 | 2 | 63 | 2,750,000 |
| 5 | Fusion Rifle, Anti-Matter Rifle | 8 | 2 | 1 | 67 | 1,500,000 |
| 6 | Hunter Mine Launcher, Stalker Mine Launcher | 12 | 2 | 2 | 60 | 1,250,000 |
| 7 | B6-Obliterator, B11-Vaporizer | 6 (8) | 2 | 3 | 60 | 2,500,000 |
| 8 | Holoshield Launcher, Omni-shield Launcher | 8 | 2 | 2 | 60 | 750,000 |
| 9 | Mini Turret Launcher, Quasar Turret Launcher | 8 | 2 | 1 | 60 | 2,000,000 |
| 10 | The Harbinger, SuperNova | 3 | 1 | 3 | 69 | 3,000,000 |
| 15 | Scorpion Flail, Leviathan Flail | 20 | 5 | 3 | none | 1,000,000 |

A gun's next shot waits 1 / its shots a second (`FUN_006BEA20`, written to hero+0x3EC as the
firing sequence starts, 0x6A7648). The +0x48 is its shot's speed in u/s for the ones that
read it (`FUN_006BE9F0`, ÷ 60): the Arbiter's rocket and the Fusion Rifle's shot.

**Mods.** Alpha mods at 0x3C5710, omega mods at 0x3C5908, 0x38 an entry: +0 the id, +0x0C
the ten items it may go on, +0x18 a value, +0x30 its price. Ids as libdl has them: alpha 1
Speed, 2 Ammo, 3 Aiming, 4 Impact, 5 Area, 6 XP, 7 Jackpot, 8 Nanoleech; omega 1 Napalm, 2
Time Bomb, 3 Freeze, 4 Mini Bomb, 5 Morph, 6 Brainwash, 7 Acid, 8 Shock. Read so far:
- **Speed:** the wait between shots less 5% of itself for each (0x6A7648, snapped to whole
  frames with a random remainder for the Vipers).
- **Ammo:** the most ammo plus the table's number for each (`FUN_006BE270`, 0x3BDDE8 + 4 ×
  item).
- **Aiming:** the auto-aim's reach: the Vipers' 0.92 − 0.046 × each (0.9 − 0.045 in
  multiplayer), the Arbiter's 0.85 − 0.0765 × each, on the aim's lock (hero+0x1A20).
- **Impact:** the hit's push × (1 + 0.4 × each, the word at 0x220990) and knockback + each,
  with flag 0x20 on the damage record; the damage stays as it was.
- **Area:** a shot's ball grows by the entry's 1 u for each.
- **Omega mods on the damage record:** Freeze 0x800000, Acid 0x80, Shock 0x40, Brainwash
  0x100, Morph 0x400. The other three act at a shot's end (the B6's shell, 0x40F3E0's state
  4, and the Arbiter's rocket, 0x40E5B8):
  - Napalm: `FUN_0047E738` with (level + 1) × 3 damage, or 30 + a third for each level past
    V10, 10 in multiplayer.
  - Time Bomb: `FUN_0046D028`, a class 0x24F0 moby (update 0x46D138): a ball of 12 u, plus 1
    for each Area mod, takes up to 16 targets, which each take half the gun's damage later.
  - Mini Bomb: `FUN_00423A08`.
- XP, Jackpot and Nanoleech aren't read yet.

### Dual Vipers (0x3EB940, fire 0x3EA770, bullet 0x3F41E8) [code, DL NTSC, 2026-10-31]

Fires on the sequence's event 8; each shot hands over to the other gun of the pair (+0x67
cleared on one and set on the other). Each bullet is a moby of class 0x1095 (`FUN_003F1AB0`):
- 1.1667 u a tick (70 u/s; the table's +0x48 says 60, and this is what the code uses), along
  the aim. A line from the gun to the muzzle that hits something first puts the bullet out
  0.05 u short of the hit, aimed along the gun's own line.
- Life 35 ticks, 105 from V10.
- From V10 it goes on after a hit on the world (`FUN_003F3928`): generation + 1, reflected
  off the normal and put 0.01 u out, and at generation 5 it ends; each bounce gets 10 ticks
  of life and turns to the enemy `FUN_003F3368` finds ahead of it (within 60 u and 0.75 of
  the way round, or 30 u and 0.35, in that function's test).
- A hit on a target: `FUN_00593100` with the row × hero+0x2FB0 (a damage scale, 1 here).
  Otherwise a ball of 0.6 u (`FUN_005415A8`) on the world, plus 1 u for each Area mod
  (`FUN_003EEA20`).
- Recoil `FUN_00441588(0.3, 0.7, 0.75)`.
- Multiplayer: 8.5 shots/s.

### Magma Cannon (0x408260, sweep 0x406F30, cone 0x406450) [code, DL NTSC, 2026-10-31]

A shotgun of lines in a cone, fired at sequence event 8, not of shots in flight:
- Cone: 25° either side, row spread 13.5°, range 30 u (20 in multiplayer), 3 lines a side,
  set by `FUN_00406450`. The gun's entry +0x10 (not named by libdl, `FUN_006C0CE8`) changes
  them: 3: 14°, 34 u; 2: 41°, 23.5° spread, 27 u, 4 a side.
- Five rows of lines at 10° (the middle, first tick) with 7 lines; 16.75° and 3.25° (next
  tick) with 6, half a step in; 8.31° with 6 and −3.5° with 7 (last tick). What the 10° is
  measured from, and which way is up, isn't sure: the kit takes it as the aim and up.
- Each line that reaches a target hits it with the row's damage (`FUN_00593100`), so several
  lines on a big target add up, through the damage queue. The push is half the record's, ×
  (1 + 0.4 × Impact). A target locked at the moment of firing takes one hit besides.
- Each line draws through a class 0x262B moby (`FUN_00475BB8`), with looks of its own from
  V10.

### The Arbiter (0x41D370, message 0x41E890, rocket spawn 0x40BA00, flight 0x40C8E0) [code, DL NTSC, 2026-10-31]

- Each gun's `GadgetDef`+0xA0 is its fire message's handler, event 8 (a multiplayer path the
  single player goes through too), not a target function.
- The rocket is class 0x108C: `FUN_006BE9F0` × its +0x48, so 1 u a tick (60 u/s), and 95 ÷
  that in ticks of life. Aimed through hero+0x2EFC's point 100 u out in the views that aim,
  it starts 0.5 u along the aim from the muzzle.
- With a target it turns after it (`FUN_0040C5E8`, at most 40° on the first read). Which
  turn applies where is not read yet: the kit flies it straight.
- Its end (`FUN_0040E5B8`): a ball of 5 u, plus 1 u for each Area mod, at the second damage.
- V10 (hero's item level above 8) takes +0x4C = 1, V99 2: other looks.

### Fusion Rifle (0x413EA0, message 0x412028, fire 0x412C40, shot 0x414928) [code, DL NTSC, 2026-10-31]

A scoped shot: the shot's speed is the table's 1600 u/s (26.7 u a tick, class 0x1097). In
the look its update holds a zoom (hero+0x1A90: 196.65, 75 out of it), and the aim jitters
by ±0.5 × hero+0x1B0. Its hit (`FUN_00412848`, 0x416110) takes the row's damage with the
same omega flags. Its reach is not read: the kit's 160 u is a placeholder.

### B6-Obliterator (0x40B498, fire 0x40B028, shell 0x40F3E0) [code, DL NTSC, 2026-10-31]

Throws a shell (class 0x108E, `FUN_0040F0C8`), through `FUN_006353E0`, the same throw the
Hunter Mine Launcher and Mini Turret Launcher use: `least` the 1e-05 at 0x220F6C; `most` 6.5
u, or 16 with a target (0x220F68); 16 u/s across the ground (24 in multiplayer, 0x220F74,
0x220F78); gravity 16 u/s² (0x220F70). A collision (`FUN_006369E8`) takes the shell to state
4, its end. Not read: its blast's radius (the class 0x1093 it leaves looks only drawn).

### The other DL guns [code, DL NTSC, 2026-10-31; first read only]

- **Mini Turret Launcher (0x46E820, fire 0x46E348, turret class 0x24F8, 0x46EEF8):** thrown by
  `FUN_006353E0`: `least` 1e-05, `most` 12 u (0x2218E4, 0x2218E8), 15 u/s across the ground, 35
  u/s² (0x2218EC, 0x2218F0). Falls at the same gravity. Its shots' speed and rate are the
  table's 35 u/s and 7 a second; its range and life aren't read. Its own update's calls: state
  1 `FUN_0046F8D0`, `FUN_0046FB70`, its firing `FUN_004700E0`, `FUN_00470028`.
- **Holoshield Launcher (0x3F50A0, spawn 0x4182F0, shield class 0x10A9 0x419400):** a ball of 16
  u round the standing shield (0x419400) finds what it acts on, with the gun's damage at 0x4196xx
  (`FUN_006BE8B8` with item 8). Not read further.
- **The Harbinger (0x475618, 0x4752F8):** not read beyond its table row and the spawn.
- **Scorpion Flail (0x44ECC0, 0x44E978, head 0x450E28):** the two damages are its head's
  (0x450E28 chooses one by its state). Not read further.
- **Hunter Mine Launcher (0x43EF98, 0x43EC10, 0x43DE00):** thrown by `FUN_006353E0` (0x43EC10).
  Not read further.

### DL's guns in the kit

`Armory::dl` and `crates/rac_combat/assets/dl_weapons.ron`; `cargo run -- --demo --guns dl`
gives them. The file's rules, holds, sequences, muzzles and `ready_ticks` are still UYA's.
- **Dual Vipers:** a `Projectile` at 70 u/s with its lives and, from V10, 4 bounces. Not like
  the game: a bounce doesn't add 10 ticks or turn to an enemy, and the line from gun to muzzle
  doesn't cut a shot short.
- **Magma Cannon:** the new `Cone` fire mode, with its rows, lines and reach. The rows' tilt is
  taken from the aim and as up, not checked (above). Its entry +0x10 modes aren't in.
- **The Arbiter:** a `Projectile` at 60 u/s for 95 ticks with a 5 u ball, flying straight.
- **Fusion Rifle:** a `Projectile` at 1600 u/s; no zoom or jitter; its 160 u reach is a guess.
- **Not in yet:** the B6, Hunter Mine Launcher, Holoshield Launcher, Mini Turret Launcher, The
  Harbinger and Scorpion Flail; the alpha and omega mods; DL's sounds, looks, models, HUD
  icons and hand tables. Every push in the file is a placeholder.

## In the kit (P1.9) [2026-10-21]

`rac_combat::weapons`, with the guns' numbers in `crates/rac_combat/assets/uya_weapons.ron`
(each labelled with its section here), and the hero's side in `rac_hero` (`gun.rs` and the
profiles' `gun` section):
- **Definitions** (`Armory`): all twenty guns, with every level's damage, second damage,
  maximum ammo, XP thresholds and round price (the RY3NO's five), the levels their mods
  come on, the acid tables, the V5 names (from the image's text [data]) and a fire mode
  each: hitscan (the Shock Blaster), projectile (the N60's bullets and the Flux Rifle's
  shot), lobbed (the Nitro Launcher), stream (the Lava Gun) and homing (the Annihilator),
  and one per gun for the other fourteen (below). Shared rules: the 17-tick draw, the quick switch's 20
  ticks round three of eight recent guns, Mega as the bought level, the view's bend, the
  shock arc's 8 u and quarter, and 30 acid slots.
- **Owning** (`Arsenal`): giving, ammo used and added up to the maximum, XP up each level's
  chain to V5, the bought Mega counting from 0, the recent guns with the empty one pushed
  out, and the quick switch. The kit's next and previous weapon buttons go round the guns
  owned, in the order given.
- **The hand:** the fire button, the quick switch and, with the gun's lock-on mod,
  lock-on bring the gun out after 17 ticks; a wrench attack puts it away at once. A gun
  fires once it has been out its ready ticks (its code's count less the 4 it starts
  at, "When a drawn gun may fire"), and in the look after its wait.
- **Each gun** as "Each gun" reads it: the Shock Blaster's rate counted from each hit, its
  2- or 5-tick hit delay, its ball and 20 random and 3 low rays, and from V2 one shot a
  press and the charged shot of three times the damage; the N60's 14-tick repeat from two
  barrels, 60–80 u/s bullets living 30 or 46 ticks, bouncing from V2, with their 0.6 and
  1.0 u balls; the Nitro Launcher's target (lock-on within 12 u, else its assist), solved
  arc and cut launch, the charge's bounce, fuse, ground and enemy bursts, and the blast's
  flash and growth, hitting each target once; the Lava Gun's strength spring, a round
  every 10 ticks, 35 points falling at 21.6 u/s², their splash and the nozzle's ball; the
  Annihilator's 5-tick pick and drop, the enemy near the barrel, the 60 u/s missile
  turning after its target and its blast's second damage, from V2 to V4 its mines and
  from V5 its seven seekers; the Flux Rifle's 400 u/s shot for 19 ticks, its muzzle ball
  and the scoped bend, and with each scoped shot the beam from the view's eye (30 u a
  tick for 26 ticks, stopped by its first hit; from V2 up to six tests a tick, each
  passing over what the one before hit; balls of 0.6, 0.8 and 2.5 u: `ShotKind::Beam`).
- **Aim:** the lock-on target; else, in the lock-strafe and first-person cameras and the
  look, the point the view centres on, bent from his facing by at most 80° (8° within
  4 u); else the gun's own assist (`gun_aim_target`'s cubic cone and score; none in
  lock-strafe); else his facing.
- **Damage:** one pending hit per target a tick, the strongest, a weaker one adding its
  mods; shock arcs to the first other living enemy in sight within 8 u, striking it 10
  ticks later for a quarter of the gun's damage (`Arcs`); acid burns past the queue, and the burn that kills credits the gun; a kill gives
  the target's `KillXp` to the gun that did it.
- **The hero's side** (`GunHand`, set each tick before he moves): no double jump while
  firing; the held gun's setting-off turn and the steady turn (the N60 trace's law, to
  10⁻⁵); no speed-up past 45° while a held gun fires; with a gun, letting go brakes a tick
  at 30 u/s² and idles; in the lock-strafe and first-person cameras a gun makes him
  strafe, and firing in lock-strafe triples the strafe turn; lock-on only with the gun's
  lock-on mod (`LockOn::allowed`). The gun arm rises against a wall close ahead, or
  follows the lock-strafe camera's pitch (`GunArm`, "The raised gun arm"), and then he
  fires the arms-only animation; the gun's aim point (`GunHand::target`, from its assist
  while he isn't locked on) turns his head and upper body (`rac_rig`'s joint springs,
  "The head and upper body turn to the gun's target").
- **[placeholder] or [simplified]:**
  - The muzzle is each gun's point on a table of where his hand is by what he is doing
    (`Hands`; "The muzzle"), where the game's follows the posed hand: the simulation
    can't read the rig's pose (PLAN §5), so it doesn't swing with a walk or lean with the
    look, and its barrel is level but for the 12° at rest. The Miniturret's glove rides
    his forearm (slot 6) and is taken from his hand. `firing` lasts the
    gun's `fire_ticks` after a shot: for a gun not held, its standing sequence as played
    ("How long it plays"), even crouched, where the game ends it with the arms layer;
    a held gun fires from its first shot until it is let go or runs dry. A state change
    ends it unless the gun is held.
  - A target takes no gun hit for 8 ticks after one: the games leave that to each
    enemy's code, and without it the stream would hit every tick.
  - A gun fires in every state but those setting the no-firing flag ("When a gun may
    fire"), as far as the kit has them; DL's charge and bounce aren't read and don't
    fire. There is no state 29: the look with a gun out is the aiming state. No twist in
    the fire direction.
  - The view's aim point is where the camera's 30 u centre line meets something
    (`fps_cam_find_target`); the camera's own target and its weight aren't kept.
  - The Nitro
    charge's 300 ticks, the Bouncer's canister's 600 and the 600 a holo shield may fall
    are the kit's own culls: the game has none, or (the shield) goes at the level's box.
  - A Megaturret's fade shows as it shrinking and growing back, where the game fades its
    alpha, and its move doesn't test the ground's material or the view (0x32E950, 0x4C7670).
  - The Suck Cannon's balls sit 2, 4, 6 and 8 u along a straight vortex (the game's links
    stretch and blend), and it takes an enemy of at most 10 hit points, where the game
    asks the enemy's record (+0xF).
  - The Lava Gun's acid from V2 is the vendor's [text]; the Nitro Launcher's acid is on
    and does nothing, as read.
  - The first six guns' assist and lock-on take any hurtbox with hit points left, not
    only enemies (class category 5); the shock arc and the other fourteen's searches take
    only enemies (`rac_combat::Hostile`), as the game's do. XP counts whole points.
  - Not in yet: DL's guns; the Flux beam's class 0x1CF7 that stops it, a level's object.
- **The other fourteen** (2026-10-22; `weapons/seekers.rs`, `thrown.rs`, `grab.rs`,
  `hand.rs`), each as its section above reads it, from V5 on firing as V4 with V5's
  numbers:
  - the Plasma Coil's 10-tick spin, 0.5 s cooldown and five balls, the ball sticking to a
    target (its damage) or the world, and 15 ticks later arcs to the three best-scoring
    enemies within 10 u all round, each striking after 10 ticks (the second damage);
  - the Disc Blade Gun's disc every 30 ticks, below V5 lifted 22.5° (60.5° with a target)
    and falling for 38 ticks, steering at its target, bouncing off walls, from V2 sent at
    an enemy on each bounce, from V3 through enemies, from V4 a 4 u blast of half its
    damage at its end, from V5 straight at twice the speed from two muzzles in turn;
  - the RY3NO marking the enemies its assist finds and firing a volley of 8 (16 from V2)
    missiles a round, each at the next mark, its cooldown 1.3 times longer from V2;
  - the Spitting Hydra marking an enemy every 12, 15 or 10 ticks held up to its level's
    most (every tick once held 150), charging without firing (`Sustain::Charging`, its
    `Marked` and `Fizzled` events), striking each mark on letting go without a round,
    fizzling under 20 ticks, or spitting a fan of bolts for a round, then not charging
    again until the spit's time is up;
  - the Bouncer's canister on `lob_velocity`'s throw, bursting near a target or 30 ticks
    into its roll after four bounces into 17 bomblets, which home on a bounce and burst on
    the first target with hit points once armed;
  - the Infector's blob, slowing, bouncing towards enemies, infecting weak ones (2 hit
    points or less) as it flies and bursting on anything else, its burst infecting
    enemies within 1.5 u and its blast doing 1; an infection lasts damage / health × 600
    ticks and gives the gun the target's XP;
  - the gloves' throws (`deploy`: the round on the press, the agents' as the pod leaves;
    20 ticks to leave the hand; the wait before the next); the Miniturret landing,
    unfolding 6 ticks, turning to the enemy it finds within 15 u and shooting it at 6 a
    second, 20 or 30 shots, bursting idle after 1200 ticks or 30 u from him; the holo
    shield landing (5 damage in 1 u), standing 900 ticks as a wall that takes 7 hits
    (`block_shot`, which the demo's enemy shots call), from V3 sending a bolt at an enemy
    within 7.5 u 15 ticks after a hit, from V2 healing him; the agents' pod letting out
    four (from V4 four come to him), each finding its target by the level search
    (`aim::level_search`: anything with hit points within 25 u as it comes out, then
    enemies within 30 u no agent has claimed, both within 5° of level, scored by
    distance and a bearing of 20°; claims last 60 ticks), idling, wandering and
    following him and chasing for up to 420 ticks on the walker's speeds and turns,
    blowing up in 2 u; from V2 its gun's small bullets (`ShotKind::Slug`: 0.03 of the
    damage, every 30 or 15 ticks, 50 rounds each or 120 shared at V4), from V5 the small
    missile while it keeps by him;
  - the Rift Inducer's rift at 60–80 u/s keeping 0.92 a tick, opening after 36 ticks or
    where it meets something, then for 600 ticks taking one target at a time within 9.5 u
    whose hit points are at most its damage and swallowing it (`Affliction::Swallowed`,
    a kill);
  - the Suck Cannon sucking in, every 4 ticks held, what its four balls along the vortex
    touch up to its capacity (`Arsenal::sucked`, the HUD's count), and firing one back
    out a press, 30 ticks apart, at 40 u/s;
  - the Qwack-O-Ray's beam picking an enemy close in front or the nearest within 12 u and
    30°, keeping it 30 ticks, and making it a duck once the damage a second has used up
    its hit points (`Affliction::Duck`);
  - the Shield Charger's shield of 225 + 75 a level points round him, taking hits first
    with its 15-tick grace (`rac_hero::Shield`, in `hurt_hero`), zapping touching enemies
    every half second;
  - the Plasma Whip: with it in hand the fire button starts the wrench's combo, jump
    attack and throw in their windows (`rac_hero::WhipHand`), each for a round, keeping
    the whip in hand; the rope hits along its reach (9 stretches, 8 links' 0.194 and
    the tip's 0.105: 9.76, 7.06 and 8.86 u) from the 7th tick of a swing and the 21st of the jump attack, and the
    thrown end hits along the line from his hand; from V4 the jump attack's shock wave,
    and from V4 (the Quantum Whip) the cutting waves its swings send, striking each
    target once;
  - the shots in flight, as read: the Hydra's spit and bolts, the RY3NO's missiles, the
    Suck Cannon's shot, the Holo Shield's drain and heal, and the agents' ends (V1's
    blast, V2–V4's comet, V5's growing blast);
  - the Qwack-O-Ray's ducks: the enemy is gone and a duck stands where it was, waddling
    and turning up to 45° now and then, laying eggs from V2 that bounce, seek an enemy
    from V3 and burst on one; the V4 duck runs at the nearest enemy when a look finds
    one and blows up there; the Qwack-O-Blitzer's duck flies by the hero and after an
    enemy it sees, pecking it, then blows up;
  - from V5 each variant's own code: the Tesla Barrier's bolts at up to four enemies
    within 4 u, 40 in all; the Rift Ripper's bolts; the Multi-Disc's splits, rarer the
    more discs are about; the Ultrashield's pulses after a hit; the Plasma Storm's
    bouncing ball arcing to enemies within 15 u; the Infecto-Bomb infecting all in its
    burst; the Megaturret's guided missiles and its walk back to the hero; the Heavy
    Bouncer's bigger bomblets; the Shock Cannon's 23 rays; the N90 Hurricane's 10-tick
    repeat, 30 u reach and 7 bounces; the Nitro Eruptor's blast growing to 11 u and its
    15 small bullets.
  - **[placeholder] or [simplified]:** the agents' path finding (straight at him), their
    ledges and being set down near the camera; the search's +8 for a target 0x447CA0
    picks (a list of classes the kit hasn't got); the holo shield's size, which enemies
    the Suck Cannon can take (UYA asks each enemy's record), the ducks' falling (the game
    keeps them to the ground), and the Plasma Storm's arcs, which strike as the coil's
    (0x30A3D0 not read). The whip hits along a straight line along the wrench's, not its
    nine trailing links (which the rig draws), and a target takes one hit an attack (the
    game's queue allows one a frame). In the demo an infected enemy turns on the nearest
    other enemy it sees, which fights back, and a ducked one is gone until it comes back:
    the demo's own, since in UYA each enemy's own code decides. Not in: ducks drawing
    enemy fire (each enemy's own code), the Liquid Nitrogen Gun's block and puffs, the
    vehicle and critter ends of the Shield Charger, and what the Suck Cannon's fire flag
    does.
- **On the rig** (`rac_rig`, 2026-10-21): `GunHand` names the gun in hand, its hold and
  firing animations (`GunPose`: rows +0x18, +0x24, +0x2C [data]) and counts its shots.
  Standing, the gun's whole-body sequence plays (`RigMove::Fire`), from the start each
  shot at its own pace (the N60 trace); crouched, its arms-only one is laid over the arms
  (`RigMove::FireArms`), blending in at 0.2 a tick and out at 0.07. The wrench hides while
  a gun is out, and the gun's model sits in the right fist (`RigGuns`; `model::gun_grip`:
  barrel along the forearm, top on the thumb's side, which points it within 4° of ahead in
  UYA's sequences 49, 51 and 103, a disc test). With a UYA disc the hand holds the
  gun's own class from the gadget WAD (`openrac/src/gun_models.rs`; `moby-format.md`,
  "The gadget WAD"), moved once from where the game puts it on joint 56 into the grip,
  so UYA's hero holds it as the game does and every other model the same way. Each
  level's model is its row's class with the bangles its update shows (`GunHand::level`,
  `RigGuns`). It plays its own sequences as "The gun's own animation" reads
  (`rac_rig/src/gun_anim.rs`, on its own skeleton, `GunSkeleton`): it grows in on 0,
  settles into 1 over 2 ticks once a sequence has come round (the Plasma Coil cuts), and
  each shot does the gun's recoil (`openrac`'s `gun_models::uya_motion`); the Annihilator
  and Disc Blade Gun fired while growing in go to their rest over 4 ticks instead of
  recoiling (the kit's reading of "first"). The Agents' and Holo Shield's gloves and the
  whip's handle have one sequence and stay baked in it, the handle at 0.3 of its class's
  size as its update draws it (2026-10-23; it was drawn at its class's own, over three
  times too big). The arms hold each gun by its hold (2026-10-23, "Holding a gun"):
  `openrac` builds each arm's clip for every move whose sequence the hold classes have
  (`hero_anims::hold_animations`), and the rig lays them over the arms
  (`rac_rig::HoldAnimations`, `play.rs`'s `HoldLayer`) with the game's weights, the rest
  through the flips and the grind with the clavicles' lift late in the flips, nothing on
  a ledge, and the third fidget held back with a gun in both hands. The open model keeps
  its procedural hold. [simplified] The way back from the rest blends towards the body's
  current keyframe rather than from its first. The joint springs
  (`rac_rig/src/springs.rs`) turn the waist, chest and clavicles for the raised arm and
  towards the gun's aim point, as `hero_gun_arm_raise` and `hero_head_look` set them, at
  60 Hz. Without a disc the guns are blocky placeholders [placeholder]. The open model
  aims procedurally. The Plasma Whip's rope (`rac_rig/src/rope.rs`): nine links trailing from the handle by
  `whip_rope_ends`' rules (the stretch and pull by state, the links lying flat and links
  1 and 2 adding the hand's travel in a swing, kept 0.15 u off a floor taken at his feet,
  the end following the thrown wrench; each piece after the first starting 0.194 u along
  the link before), a visual only, since the kit's whip hits along a straight line. With
  the disc its links are classes 0x183A and 0x183C at 0.2 of their scale, placed and
  stood on edge as `whip_link_place` does, at the game's opacity and glow by level
  (`rac_rig::RopeModels`, 2026-10-29); without it, glowing translucent bars from link to
  link. The Shield Charger's shield shows as its shell, an effect (`rac_demo::fx::shell`,
  2026-10-29), 0.75 u above his feet while up; the game never draws its class's mesh.
- **Pickups** (`rac_combat::pickups`, P1.10, 2026-10-22): a hero reaches pickups within
  8.7 u across and 3 u up or down from his feet [code]. Ammo follows "Ammo pickups, crates
  and the vendor": taken only for a gun with room, on landing or bouncing, or as it lands
  if he came in reach while it flew; at once 2 u below where it was thrown; its rounds go
  in capped (the rest lost) and it flies in for show from rest at 24 u/s² up to 64 u/s to
  0.7 u above his feet, carried along with him, shrinking over its last 30 ticks; never
  expires [code]. In co-op the nearest hero with room takes it. Ammo crates throw 1
  pickup, or 2 once in 5, for one gun of the breaker's, not full if any, never the Flux
  Rifle (`crate_ammo`) [code]; bolt crates pay their bolts a quarter more or less in
  coins of 30000 down to 1 [code].
- **The bolt bank** (`rac_combat::pickups::BoltBank`, P1.10, 2026-10-30), by "The bolt
  bank" [code]: one bank for the world. Each broken crate takes a fiftieth of it, rounded
  up; a bolt crate adds that to its bolts (after the doubler), drops at most 2 or 3
  coins at random (`most_coins`) and banks the rest, the coins of 1 twice as the game
  does; the other crates bank their share again, a quarter more or less. The demo's
  enemy kills drop the share as coins. Bolts that expire uncollected are banked, in
  place of UYA's banking of coins far from the hero or the camera [placeholder]. Fewer
  coins while crates have broken lately (`rac_combat::pickups::CrateDebris`, 2026-10-31)
  by "The crate's cap" [code], with every moby slot taken as free [simplified]; the kit
  draws no debris. Not built: the bolts-per-minute scaling, the challenge multiplier,
  the XP bank. The demo's enemies drop no ammo, as UYA's don't.
  The crates' bolts (44, one of Veldin's sizes) [data]; how pickups pop out and bounce,
  and how bolts and nanotech fly in and expire, are the kit's [placeholder]. Rounds per
  pickup (+0x88) and the vendor's price a round (+0x84) are [data].
- **The vendor's menu** (`rac_combat::pickups::vendor`, P1.10, 2026-10-30), by "The
  vendor's menu" [code], over the running world as the ring is (PLAN §5, 2026-10-22): it
  holds the hero's input while open (`GunHand::selecting`).
  - Its rows are rebuilt each tick, in the armory's order: rounds for owned guns not
    full, buy-all, guns with a price he doesn't own, and Mega upgrades with
    `upgrades` on. Each gun's price and Mega price (+0x7C) are [data].
  - Left and Right are the weapon buttons or the move stick past 64 [placeholder]; the
    rest follows UYA's rules: no wrap, repeats on the 21st, 31st… tick held, Cross
    refused unless the row can be bought, a count of rounds from what fits and the bolts
    buy (repeating at 16, 24, 32, 40 and every tick from 48), a confirm for the other
    rows. Jump is Cross; quick select is Triangle, backing out or closing.
  - The pad opens it as a living hero's feet come in, not again until he steps off and
    on, and it closes as he dies [placeholder].
  - Not done: UYA's stock growing with the story (the kit sells every gun with a
    price), the challenge-only five, the 9/10 discount, the last special row.
- **Their sounds** (`rac_combat::pickups::PickupSound`, P1.10, 2026-10-30), by "Their
  sounds" [code]: the kit sends one as a crate breaks or blows, a fuse or the doubler
  beeps (on UYA's frames), a bolt (with its coin's value), ammo or a nanotech orb
  reaches a hero, and for each of the vendor's menu's events, the count's ticks on
  UYA's frames. The demo plays a placeholder for each (`sounds.ron`). With the UYA
  disc, `openrac`'s `gun_sounds.rs` plays the game's entries from Veldin's core and
  bank in their place, and the explosive and doubler crates', which Veldin lacks, from
  level 4's; with the crates' 2- and ammo's 11-frame gaps, coins by value (those of 500
  and up as 0x11's, which Veldin lacks [placeholder]) and the vendor's at full volume.
  Checked on the disc [verified on UYA NTSC: all 18 found, a bolt and an explosive
  crate's rendered audibly]. The purple nanotech isn't in the kit.
- **The help measure** (`rac_hero::HelpMeasure`, P1.10, 2026-10-30), by "The help
  measure" [code], in UYA's profile (`hit.help`; DL's has none): one for the world,
  counting each hero's deaths in his area as he is brought back, from the level's tick
  900 (1800 for a fall out of it). An area is the last checkpoint a hero reached, the
  level's start before one; reaching a new one takes 2 deaths off its area. What gets
  past his shield is scaled as `hero_damage` does: 1.2× with no deaths and 1.1× with
  one while the measure is under 50, 7% lighter a death past 6 over 250 (half at
  most), rounded and at least 1, the hazard floor's included. Each death weighs 50, as
  in Veldin's first area [data]; UYA's areas can span several checkpoints, weigh their
  neighbours' deaths and turn the measure off (flag 0x80), and the kit's don't
  [placeholder]. Its level is the time since a hero was first in it, and it starts
  again once none is.
- **The other crates** (`rac_combat::pickups`, P1.10, 2026-10-29), as "Ammo pickups,
  crates and the vendor" reads them [code]:
  - a nanotech crate leaves its eight orbs where it stood (`nanotech.rs`); they are let
    out to the nearest living hero within 10 u whose health, with what orbs already let
    out to him carry, is under his most, fly 20–80 ticks on the game's curves to 0.75 u
    above his feet, and the first there heals him his most / 5, / 4 or / 3 by the help
    measure of his area (below), at least 1; they go 300 ticks after. The kit works the
    amount out from his most as they arrive rather than as the crate is made, and circles
    the waiting orbs round the crate's middle [placeholder]; the sparkles aren't drawn.
  - an explosive crate (`explosive.rs`) lights a 179-tick fuse when a hero's capsule comes
    within 0.05 u of it or he stands on it [placeholder for the game's ground and wall
    mobys], at once while he grinds; a hit lights it for 5–14 (or cuts a long one to
    that), another's blast for 1. It tints as the game's warns; then its blast grows over
    20 ticks, each target it reaches taking 2 (an explosive crate's fuse going to 1
    instead) and each hero 4, pushed 60 u/s out and up, which his hit caps at 7. The kit
    tests boxes as unturned, hurts each target once a blast, and leaves out the armor in
    his damage (`hero_damage`; the kit has no armor) and 0x348FF0; the demo draws the
    shared explosion's set at its own size.
  - a doubler crate gives its breaker one more doubling and 1800 ticks (`BoltMultiplier`,
    on every hero with `Bolts`): his crates' bolts, his kills' XP and the demo's enemies'
    bolts he kills are doubled per doubling while it runs, and it all ends together. Its
    beeps are cued as `PickupSound`s ("Their sounds").
  - The doubler on the HUD (2026-10-31; `rac_demo::hud`, `doubler_look`), by "The
    doubler's x2" [code]: "x2" and up beside the bolt count while it runs, which keeps
    the bolts up and 180 frames after; its pulse round each beep and its shrink over the
    last 4 ticks follow the game's steps, by the ticks left, its colour a tick behind
    its scale and packed as the game packs it. The demo's layout puts it in the bolts'
    panel, not at the game's place. Seen on screen (`kitdemo-doubler.orin`, at lavapipe's
    slow frame rate): the x2 at rest, and round the first beep swelling, magenta for a
    frame, then smaller and orange.
  - Its help message (2026-10-31; `rac_combat::pickups::{Help, HelpAsked, HelpGiven}`, the
    demo's `PlayerHud`), by "Help messages" [code]: the first doubler crate broken asks
    for it, once a world, and lost if the HUD shows one already. The demo shows it in
    its own words with the game's timing (`HudTimings::help_phase`: the box opens over 9
    frames, the text shows 4 and 7 a character, the box closes over 8), in the game's
    colour, high in the middle rather than low (the app's controls are there). Not
    built: Triangle closing it, voiced help, the help table's counts and follow-ups.
    Seen on screen (`kitdemo-doubler.orin`).
  - Seen on screen (`kitdemo-yard-crates.orin`): the crates' placeholder looks, the
    explosive pair tinting as he touches them and going off in turn, his health 10 to 6.
- **Quick select** (`weapons/ring.rs`, 2026-10-22): two rings of eight, filled as
  `quick_select_add` does [code], worked as "The quick-select ring" reads with QUICK
  SELECT PAUSE off, so co-op stays in step: the ring opens on the press over the running
  world; 15 ticks on (27 after a tap that switched nothing) the hero takes no input until
  it has closed (`GunHand::selecting`); the stick picks after the hold check and its
  lock-out, past 0.9, kept within 33.75°, straight slots 27° wide, an empty slot leaning
  to a filled neighbour; the kit's next and previous weapon buttons (the pad's d-pad left
  and right) step by the game's neighbour table (`quick_select_slots`, read from Veldin's
  image [data]), d-pad up and down having no button in the kit's input [simplified];
  crouch (R1) shows the second ring and 14 ticks after; letting go closes it over up to
  8 ticks, then chooses. Not followed: the paused ring (the option on), the camera items
  0x24–0x26, the game's other conditions on opening (game mode, states 0x58 and others,
  `0x1A71C8`/`0x1A71CD`). (0x41BE10, once taken for the lock-strafe camera's own
  widget, is Clank's four-slot ring, opened in its place while he plays Clank.)
- **HUD** (`rac_demo::hud`, `assets/hud.ron`, P1.12, 2026-10-22): when its parts show
  follows "The HUD" (`hud/clock.rs`): the gun while one is out and 10 frames after;
  health 720 frames after a kill (any `Destroyed`, crates included [simplified]) and 360
  after damage, nanotech, more maximum health or the ring opening, staying up and
  pulsing dark red under 15%; bolts 180 frames after a change or the ring, the count
  trailing with a +N or −N tally that fades in over 15 frames and rolls in 61 frames
  after the last change; ammo pulsing dark red under 15%, every 40 frames; the next two
  recent guns smaller and fainter beneath the gun; the ring fading in and out with its
  opening, its pick's level as V1–V8 and its ammo red at 0; the level-up message for 180
  frames [code]. The layout, the pips, the gun's name and the icons are the demo's own
  [placeholder]. Not followed: L3's show-all, the upgrade sequence with the world stopped,
  at most one level-up every 60 frames.
- **Aim indicators** (`rac_demo::hud::sights`, P1.12, 2026-10-22), as "Aim indicators"
  reads them [code]. Each gun's `sight` in the RON says what it draws, and the wrench's is
  `wrench_sight`. The weapons code names what to mark each tick (`Arsenal::sights`: the
  gun's own target, its marks, whether it can fire), and the camera gives its target
  weight (`FollowCamera::target_weight`, kept while the mouse aims too, though the assist
  is then off). The HUD draws at the game's rates per frame:
  - the lock-on reticle's three rings, with their fade, spin, sizes and health arc;
  - each crosshair at the view's centre, with its fade-in, pulse, red over a target and
    when it shows;
  - the shared target mark, the RY3NO's marks on each target, and the Hydra's triangles
    within 60° (their size and spin are the shared mark's, checked in `hydra_marks_draw`);
  - the Flux Rifle's scope in the ready look, with its ring blinking 15 of every 31
    frames while it can fire and the eye's 128 u ray meets a live enemy before any wall
    or other target (`Sights::scope_on`, worked out in the weapons code every tick
    [simplified from 4 of 8]).

  The scope's zoom (2026-10-22) is the weapons code's (`Scope` in the RON,
  `look_through_scope`): it starts each look a quarter in and moves 0.01 rad a tick on
  R1/R2 (Crouch and Strafe; Ctrl and the right mouse button) in third person, or on the
  left stick in the other settings. Strafe also has L2 on the pad, so L2 zooms out too
  [simplified]. It asks the camera through `GunHand::zoom`, and the camera springs its
  field of view and slows the free look (`camera.md`, "In the kit"). Seen running on
  Xvfb: the look starting a quarter in, Ctrl zooming in, and the view springing back out.

  The sprites are flat UI images in game pixels scaled by the viewport's height, keyed by
  the effect-texture numbers (`SightImages`). With a disc, `openrac` reads the level's
  effect textures (`Core::fx_textures`, `effects.md`) and hands the HUD the fourteen it
  uses (`crates/openrac/src/fx_textures.rs`); without one the demo paints its own
  [placeholder]. The rings and the scope are drawn from a top-left quarter, as the game
  draws them: each ring as four quarters turned 90° apart round its centre, the scope
  mirrored into the four quarters of a 4:3 box as high as the view (the game's screen,
  for which its lens is drawn narrower across), with the sides beyond in the texture's
  surround. They replace the demo's torus round the lock-on target. Not followed:
  - what the zoom's four floats at 0x1DF720 drive (not found);
  - state 0x74 hiding the reticle and dropping the scope, and markers shrinking while
    the world is stopped: the kit has no camera-path flight and nothing stops the world;
  - the scope's overlay drawn as colour: the game draws it into the frame's alpha only
    and redraws the frame through it with a shading quarter (0x42);
  - the PS2's wider pixels (the sprites keep square pixels; only the scope fills its
    4:3 box).

  Seen running on Xvfb: the Shock Blaster's crosshair in the lock-strafe camera, the
  N60's in the look, and the Flux scope, with the disc's textures and without, its lens
  round in both. Lock-on takes a gamepad, so the reticle was only tested.
- **Tests:** `rac_combat`'s unit tests (the definitions, the arsenal, the assist, the lob,
  the pick and the Flux beam), `rac_combat/tests/weapons.rs` (a test for each of the fourteen and most
  V5 variants, and one that fires each at V1 and V5 until its shots run out),
  `rac_hero/tests/gun.rs` (the raised arm too), `rac_rig`'s springs, gun player and
  rope, and its app test of a skinned gun, the Flux scope's zoom and ray in
  `rac_combat/tests/weapons.rs` and the camera's spring in `rac_hero`, `rac_demo`'s
  `hud::sights` (each indicator's
  sprites, sizes, fades and colours),
  `rac_demo/tests/enemies.rs` (infected enemies fighting) and
  `openrac`'s `gun_models` (a disc test: every gun's model and bangles).
- **Seen in the running demo** (Xvfb, 2026-10-21; the demo gives every gun at V3): the
  Shock Blaster's spray of rays, one shot a press; the N60 knocking out a dummy, its ammo
  and XP counting; the Lava Gun's stream arcing onto a dummy.

## Next steps

1. **The rest of the hero's side of firing in traces** (done 2026-10-20 for the Shock
   Blaster and N60: "Checked in traces"): the arms-only layers (a glove: the Miniturret,
   Agents of Doom or Holo Shield), the raised gun arm against a wall, the speed held past
   45° while a held gun fires (from a standstill), the lock-strafe pitch's arms-only
   firing, and a target for the upper body's turn and the aim (Veldin's two attackers,
   left in place). `uyantsc-veldin-fire.sh` gives a gun by writing three bytes.
2. **Still open in the archetypes:** what the damage-record flags mean (0x10000,
   0x100000 on the Flux beam from V4, 0x1010000 and others), and the Flux beam's draw
   (its colour table and the four sprites, `effects.md`).
3. **Projectiles in traces:** bullets, charges and missiles are mobys spawned per shot,
   so a fixed layout region can't follow them; capturing their speeds and the Nitro's
   arc needs `pine-probe` to read the moby list (or the class's newest moby) each frame.
4. **The other guns' remaining code:** the agent's ledges and path (0x3BD298's states 9,
   10 and 0x11, 0x3C1620), the Plasma Storm's arc (0x30A3D0), and the target 0x447CA0
   marks for the level search's +8. In the kit still to follow what is read: the shield's
   V3 glow, with the effects (P3.18), and the agent's being set down in front of the
   camera (0x3BD298); the whip's links and the shield's shell, arcs and trails are drawn.
5. **Deadlocked:** "Deadlocked's guns" has the tables, the mods and four guns. Next, in this
   order: the B6's blast (the end of 0x40F3E0's state 4), the Mini Turret's range and life
   (0x46F8D0, 0x4700E0), then the Hunter Mine Launcher, Holoshield Launcher, The Harbinger and
   Scorpion Flail, then the mods in the kit.

How this was read, to carry on: import or reuse the Veldin project (copy
`cache/ghidra/proj/uya.{gpr,rep}` or `uya-weap` to a name of your own so sessions don't
collide), list a
global's users with `RefsTo.java`, and decompile them with `DecompileAt.java`; row fields
show as `DAT_002c56xx` (0x2C5640 + offset) in the output. Names come from the text table
(16-byte entries of string pointer and text number, 0x1C31440–0x1C3ED80 in this image).

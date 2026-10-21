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
| +0x10, +0x14 | i32 | moby classes spawned for the item in hand (−1 none) | 0x4E7148 |
| +0x18 | u8 | hold type: 0 none, 1 one hand, 2 both hands | 0x4E7148, 0x4E6A08 |
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
| +0x8E | u16 | ammo when first given; equal to +0x8A in all 48 rows of the six guns read [data] | 0x416B38 |
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
  of 30000 down to 1 by `bolts_drop`, at most 1–4 coins, the rest banked for later kills.
- **The explosive crate:** touched, it blows after 179 frames, beeping; hit, after 5–15
  (1 for an explosion); its blast grows over 20 frames, damaging mobys within 2 u at full
  size for 2.0, and the hero within 3 u (4.0 passed; whether as damage isn't read).
- **The nanotech crate** holds three orbs (its pvars, or max health /3, /4, /5: 3, 2 and 2
  on a new game), let out when the hero comes within 10 u and isn't full. Not read further.
- **The doubler crate** (0x1F7) adds 1 to `bolt_multiplier_exp` for 1800 frames.
- **The vendor** (`vendor_buy`, 0x4B8560) sells as many rounds as fit and the bolts buy at
  row +0x84 each; buy-all fills every owned gun with a round price.
- **Enemies never drop ammo:** `moby_killed` drops only bolts.
- **Each gun's** [data]: rounds per crate pickup (+0x88) and price a round (+0x84):
  Shock Blaster 4 and 5, N60 Storm 15 and 2, Annihilator 2 and 20, Flux Rifle 1 and 50
  (never from crates), Nitro Launcher 1 and 50, Lava Gun 15 and 3 (1 at V1). The kit's
  maximum ammo matches all 48 rows.
- Also seen [code, low confidence]: the wrench moby in its state 10 sweeps up bolts and
  ammo within 2.7 u (`wrench_pickup_sweep`, 0x337E58).

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
  until it drops under 0.5 or 30 frames pass. Then the left stick past 0.9 of full picks
  the slot it points to, and keeps the slot picked within 33.75° of its centre; the
  cardinal slots are 27° wide and the diagonals 63°; pointing at an empty slot takes the
  filled neighbour the stick leans to, if any. Between the dead zone and 0.9 nothing
  changes. With the stick centred, the d-pad steps between slots by a neighbour table
  (first press: left 6, right 2, up 0, down 4).
- **Choosing:** only letting go of Triangle chooses, up to 8 frames later as the ring
  closes; X and Circle do nothing. A gun is asked for in his hand (`hand_item_request`),
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
- **Levelling up** (`xp_level_check`, 0x50F920): a gun not in hand levels up at once; one
  in hand, with him on the ground, plays the upgrade sequence with the world stopped
  (about 200 frames; `weapon_upgrade_start`), and his input stays locked 25 frames more.
  At most one level-up every 60 frames. The message shows 180 frames.

## The hero's side of firing [code, UYA NTSC, 2026-10-20]

Read in `hero_fire_anim` (0x4E6A08), the per-frame gun update `hero_gun_update`
(0x4E6D40, from 0x4E7118), `hero_transitions`, the walk's turn and the head-look
(0x4E2F50). Nothing here is traced yet.
- **Two flags:** each gun calls `hero_fire_anim` as it fires, which sets `firing`
  (0x1A71C9) and copies row +0x30 into `firing_held` (0x1A71CB). +0x30 is set for the
  guns fired by holding the button: the N60 Storm, Infector, Suck Cannon, Qwack-O-Ray,
  Plasma Coil, Lava Gun and Shield Charger [data]. `firing` clears when the firing
  animation ends, at once when a held gun runs dry (`hero_fire_stop`, 0x4E6CB0), and when
  any other animation is set while `firing_held` is clear (`hero_anim_set`).
- **The firing animation:**
  - **Standing** (state type 0, the gun arm not raised, and not in the lock-strafe setting
    with the camera pitched more than 10°): the row's whole-body animation (+0x24) on the
    body. It blends in over 2 frames for the Shock Blaster, the Refractor and sequence 49,
    5 for the N60, else 11. In the idle, the look and state 29 a held gun keeps it
    playing while it fires (`hero_idle_sequence` returns it).
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
  `−70° × 1.25 × (1.3 − d) / 1.3` (d from the ray's start to the hit), clamped to
  −30° … −70°, and the flag is set. Guns then aim along the gun (the Annihilator) or
  delay the shot's hit to 5 frames (the Shock Blaster). In the lock-strafe camera the arm
  follows the camera's pitch instead (0x1A6180, clamped to −80° … 45°) and the flag
  stays clear.
- **The head and upper body turn to the gun's target** (0x4E2F50): the gun puts its
  target for the frame in `gun_target_now` (0x1A57B8; the lock-on target, or its own aim
  assist's), and without a lock-on target the head-look turns towards it on springs
  (gain 0.037, damping 0.3), yaw up to ±59°, pitch −25° to +35°. With a lock-on target he
  faces it by strafing anyway. With neither, the head looks at the nearest interesting
  thing within reach every 7th frame (the look-at list, 0x1A73DC).
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
  - Otherwise along the fire direction.
- **State 29, aiming:** a gun's update puts the hero in state 29 when it finds him in the
  look (state 1). State 29 is the look with a gun out: it shares the idle's movement (the
  left stick doesn't move him), his facing snaps to the camera's yaw once the view is
  ready, R1 fires as well as Circle, and letting go of the look's button goes back to idle.
  The Shock Blaster waits 40 frames from entering it before it fires (0x1A749C, reset to
  40 whenever he isn't in it), as the Nitro Launcher does (below).

### Checked in traces [verified on UYA NTSC, 2026-10-20]

`uyantsc-veldin-fire-1` (the Shock Blaster) and `-2` (the N60 Storm), each gun given by
writing its bytes and drawn with Circle (`docs/re-setup.md`), third-person camera:
- **Drawing:** the gun was in his hand 17 frames after Circle and first fired 33 frames
  after that, the frame after the press.
- **The flags:** the Shock Blaster set `firing` alone; the N60 set both flags together,
  and both cleared the frame after Circle was let go. Starting a walk or a jump cleared
  the Shock Blaster's `firing` at once, but not the N60's while Circle was held.
- **Standing:** whole-body sequence 103 (the Shock Blaster) and 51 (the N60), at half a
  keyframe a frame, restarted by each shot, so between shots he holds the firing pose.
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
    15° down, spread 20° apart (0x36B888 tests each).
  - At V2 and up, each enemy the balls hit also gets the shock mod's check (0x34BF98).
- **Aim:** with a target in 0x1A57B8 (the lock-on target is copied there), the aim points
  straight at it (`aim_at_target`) into 0x1A52A0; the shot uses 0x1A52A0, or the camera's
  aim (0x1E2A60) with the lock-strafe camera (camera mode 1).
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
  - **the blast** (0x3985C0, 2026-10-20) also does the row's second damage (+0x70: 6 at
    every level) to everything within 0.75 u of it but the missile;
  - from V5 the explosion also scatters seven bomblets of a fifth of the damage each
    (0x3441D0). The mines the vendor gives V2–V4 aren't in the missile's code (its
    update, blast and trail have no other level test) nor the gun's, which only sets the
    mods and the moby's +0x7C (1 at V2, 3 at V3, 7 from V4; probably which extra parts
    of the model show, as on the Shock Blaster and N60) [guess];
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
  trail, and the test (0x346D90) ends the shot on the first thing that can take damage at
  every level (2026-10-20).
- **The impact** (0x346EF0, 2026-10-20): at V1 sparks; from V2 a bigger burst and a 2 u
  ball (`ball_damage`) carrying row +0x70, which is 0 on every Flux row, so it hurts
  nothing; from V4 a further effect (0x447D00). The scoped hitscan from the camera carries
  damage flag 0x100000 from V4 (0x10000 below), meaning not read. Row +0x74 holds half the
  damage on every Flux row and no reader is found.
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

## In the kit (P1.9) [2026-10-21]

`rac_combat::weapons`, with the guns' numbers in `crates/rac_combat/assets/uya_weapons.ron`
(each labelled with its section here), and the hero's side in `rac_hero` (`gun.rs` and the
profiles' `gun` section):
- **Definitions** (`Armory`): the six guns read in code, with all eight levels' damage,
  second damage, maximum ammo and XP thresholds, the levels their mods come on, the Lava
  Gun's acid table, and a fire mode each: hitscan (the Shock Blaster), projectile (the
  N60's bullets and the Flux Rifle's shot), lobbed (the Nitro Launcher), stream (the Lava
  Gun) and homing (the Annihilator). Shared rules: the 17-tick draw, the quick switch's 20
  ticks round three of eight recent guns, Mega as the bought level, the view's bend, the
  shock arc's 8 u and quarter, and 30 acid slots.
- **Owning** (`Arsenal`): giving, ammo used and added up to the maximum, XP up each level's
  chain to V5, the bought Mega counting from 0, the recent guns with the empty one pushed
  out, and the quick switch. The kit's next and previous weapon buttons go round the guns
  owned, in the order given.
- **The hand:** the fire button, the quick switch and, with the gun's lock-on mod,
  lock-on bring the gun out after 17 ticks; a wrench attack puts it away at once. A gun
  fires once it has been out its ready ticks, and in the look after its wait.
- **Each gun** as "Each gun" reads it: the Shock Blaster's rate counted from each hit, its
  2- or 5-tick hit delay, its ball and 20 random and 3 low rays, and from V2 one shot a
  press and the charged shot of three times the damage; the N60's 14-tick repeat from two
  barrels, 60–80 u/s bullets living 30 or 46 ticks, bouncing from V2, with their 0.6 and
  1.0 u balls; the Nitro Launcher's target (lock-on within 12 u, else its assist), solved
  arc and cut launch, the charge's bounce, fuse, ground and enemy bursts, and the blast's
  flash and growth, hitting each target once; the Lava Gun's strength spring, a round
  every 10 ticks, 35 points falling at 21.6 u/s², their splash and the nozzle's ball; the
  Annihilator's 5-tick pick and drop, the enemy near the barrel, the 60 u/s missile
  turning after its target and its blast's second damage; the Flux Rifle's 400 u/s shot
  for 19 ticks, its muzzle ball and the scoped bend.
- **Aim:** the lock-on target; else, in the lock-strafe and first-person cameras and the
  look, the point the view centres on, bent from his facing by at most 80° (8° within
  4 u); else the gun's own assist (`gun_aim_target`'s cubic cone and score; none in
  lock-strafe); else his facing.
- **Damage:** one pending hit per target a tick, the strongest, a weaker one adding its
  mods; shock arcs to the first other living target within 8 u for a quarter of the gun's
  damage; acid burns past the queue, and the burn that kills credits the gun; a kill gives
  the target's `KillXp` to the gun that did it.
- **The hero's side** (`GunHand`, set each tick before he moves): no double jump while
  firing; the held gun's setting-off turn and the steady turn (the N60 trace's law, to
  10⁻⁵); no speed-up past 45° while a held gun fires; with a gun, letting go brakes a tick
  at 30 u/s² and idles; in the lock-strafe and first-person cameras a gun makes him
  strafe, and firing in lock-strafe triples the strafe turn; lock-on only with the gun's
  lock-on mod (`LockOn::allowed`).
- **[placeholder] or [simplified]:**
  - The muzzle is a fixed point from his feet and the N60's barrels are guessed. `firing` lasts the
    gun's `fire_ticks` after a shot (40 for the Shock Blaster, at least the trace's 35;
    30 for the others), and a state change ends it unless the gun is held.
  - A target takes no gun hit for 8 ticks after one: the games leave that to each
    enemy's code, and without it the stream would hit every tick.
  - The states a gun fires in are guessed, and there is no state 29: the look with a gun
    out is the aiming state. No raised gun arm, no head and upper-body turn to the target,
    no twist in the fire direction.
  - The view's aim point is the first thing on the camera's centre line within 128 u; the
    writer of 0x1A6230 isn't read.
  - The Annihilator's turn (6° a tick), its near-enemy point, and the Nitro charge's
    300-tick limit; the first ammo is the maximum (row +0x8E isn't tabulated).
  - The Lava Gun's acid from V2 is the vendor's [text]; the Nitro Launcher's acid is on
    and does nothing, as read.
  - The assist, the shock arc and lock-on take any hurtbox with hit points left, not only
    enemies (class category 5); XP counts whole points.
  - Not in yet: the V5-and-up extras (the Annihilator's bomblets, the Liquid Nitrogen
    Gun's freezing), the Flux Rifle's piercing (not found in code), the Nitro's aim at the
    ground with action 4, camera shake, the quick-select ring, the other guns and DL's.
- **On the rig** (`rac_rig`, 2026-10-21): `GunHand` names the gun in hand, its hold and
  firing animations (`GunPose`: rows +0x18, +0x24, +0x2C [data]) and counts its shots.
  Standing, the gun's whole-body sequence plays (`RigMove::Fire`), from the start each
  shot at its own pace (the N60 trace); crouched, its arms-only one is laid over the arms
  (`RigMove::FireArms`), blending in at 0.2 a tick and out at 0.07. The wrench hides while
  a gun is out, and the gun's model sits in the right fist (`RigGuns`; `model::gun_grip`:
  barrel along the forearm, top on the thumb's side, which points it within 4° of ahead in
  UYA's sequences 49, 51 and 103, a disc test). The models are blocky placeholders
  [placeholder]; the real ones are in the gadget WAD (`moby-format.md`), read by
  `rac_formats::gadget` but not yet worn. The open model aims procedurally.
- **Pickups** (`rac_combat::pickups`, P1.10): an ammo pickup carries rounds for one gun
  and flies only to a hero who owns it with room, the nearest; rounds that don't fit fall
  for a partner. Ammo crates and the demo's enemies scatter ammo for the breaker's gun in
  hand if it has room, else a random owned gun with room [placeholder: UYA's crates pick a
  random owned gun not full, 1 pickup or 2, never the Flux Rifle; its enemies drop none].
  Rounds per pickup (+0x88) and the vendor's price a round (+0x84) are [data]; the
  vendor's pad, selling a round every 3 ticks, is the kit's own.
- **Quick select** (`weapons/ring.rs`): two rings of eight, filled as `quick_select_add`
  does [code]; holding the button past the quick switch opens the ring, the move stick
  past 64 counts picks a slot (slot 0 ahead, clockwise), letting go chooses and brings the
  gun out, next and previous weapon turn the ring. It doesn't pause, so co-op stays in
  step: `GunHand::selecting` stands him still. The game's own rules ("The quick-select
  ring") are read but not yet followed [placeholder].
- **HUD** (`rac_demo::hud`, `assets/hud.ron`, P1.12): health, bolts, the gun panel
  (procedural icon, name, tier, ammo, XP bar), low and empty ammo warnings, a level-up
  notice and the ring; always shown [placeholder: the game's timings are in "The HUD"].
- **Tests:** `rac_combat`'s unit tests (the definitions, the arsenal, the assist, the lob
  and the pick), `rac_combat/tests/weapons.rs` (17) and `rac_hero/tests/gun.rs` (8).
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
2. **Still open in the archetypes:** the Flux Rifle's piercing from V2 and the
   Annihilator's V2–V4 mines are in neither the shots' nor the guns' code read
   (2026-10-20); look for them in the code that reads the moby's +0x7C, or in the
   damage-record flags the shots carry (0x100000 on the Flux's scoped hitscan from V4).
   Also the writer of the camera's aim point (`fps_aim_pos`, 0x1A6230), and the shock
   arc's own function (0x34C5F8).
3. **Projectiles in traces:** bullets, charges and missiles are mobys spawned per shot,
   so a fixed layout region can't follow them; capturing their speeds and the Nitro's
   arc needs `pine-probe` to read the moby list (or the class's newest moby) each frame.
4. **The other guns**, as above; their entry points are in `symbols.toml` and their row
   readers were found by listing the users of `weapon_table` and the item byte each reads
   (0x1425C0 + item).
5. **Deadlocked:** its `GadgetDef` table (0x3BF460, `moby-format.md`) and its mods.

How this was read, to carry on: import or reuse the Veldin project (copy
`cache/ghidra/proj/uya.{gpr,rep}` or `uya-weap` to a name of your own so sessions don't
collide), list a
global's users with `RefsTo.java`, and decompile them with `DecompileAt.java`; row fields
show as `DAT_002c56xx` (0x2C5640 + offset) in the output. Names come from the text table
(16-byte entries of string pointer and text number, 0x1C31440–0x1C3ED80 in this image).

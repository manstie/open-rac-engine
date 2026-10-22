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
- **V5 and up** (the Shock Cannon, 0x385948, 2026-10-22): the same firing, charge and
  balls; 23 random rays (0x386FB0 tests each) and the three down, 26 in all.

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
- **V5 and up** (the Nitro Eruptor, 0x3893E8, 2026-10-22): the same charge (0x361880),
  lobbed by its own arc (0x389A18) and turned 5° about his up one way or the other; the
  charge marked as the Eruptor's ends in another blast (0x38A668, class 0x1888, update
  0x38A7B8): 18 frames, then for 30 frames a ball growing from 3 to 11 u in which
  everything (not the hero) takes the row's damage every frame (flags 0x1030000, pushed
  1.5 away) and the acid check.

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
- **V5 and up** (the Decimator): its update (0x38A648) only calls the V1–V4 one
  (0x36D5A0).

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
- **The rope:** the whip in his hand (class 0x1843) leads nine links (one of class
  0x183A, then eight of 0x183C), each placed at its own end pushed out along its axis by
  the rope's stretch (the whip's +0x14) and turned to point back at the one before, kept
  0.15 u off the ground. So the rope is about 9 × the stretch long. In a combo swing the
  first links lag behind the hand's sweep (by 4/i of the hand's travel for link i under
  5), so the rope trails as he swings. The stretch moves towards a goal by at most a step
  a frame (`approach`, 0x4450C0):

  | When | Stretch (step) | Second value, +0x1C (step) |
  |---|---|---|
  | At rest | 0.2 | 0 |
  | Combo, first 7 frames | 0.25 | 0.5 |
  | Combo after (swings 1, 2) | 0.9 (0.2) | 0.9 (0.3) |
  | Combo after (swing 3) | 0.6 (0.2) | 0.8 (0.3) |
  | Jump attack, frames 7–20 | 0.7 (0.1) | 0.7 (0.1) |
  | Jump attack after | 0.8 | 0.8 |
  | Thrown | 0.5 (1/30) | 0.9 (1/30) |

  So swings 1 and 2 reach about 8 u, swing 3 5.4 u and the jump attack 7 u.
- **The hit** (0x357F10): in the combo once the state is over 6 frames old, in the jump
  attack over 20, and while the thrown end is out, every frame each link's line, from the
  joint of the link before to its own far joint, is tested (`coll_line`, 0x357DC0), the
  last one carried on to 1.5 times its length. Whatever it crosses takes the row's damage
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
- **Looks:** the rope glows orange at V1–V2 and blue at V3–V4 (0xBF4020, 0x2040BF), with a
  light of radius 5.5 at the whip; its sparks and trails are many small effects.

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
    one ball at once). The arcs' update (0x30A3D0) isn't read: the kit has them strike as
    the coil's do;
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
  hero). Off the world it bounces, a random 3–7° kick to its spin; after the fourth
  bounce it rolls, its speed × 0.9 a frame, and bursts 30 frames later.
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
- **Mods:** lock-on from V3 [code]; acid from V2 [text], as the Plasma Coil's (2 a second
  for 4 s; V3–V4 3 for 6 s).
- **V5 and up** (the Heavy Bouncer, class 0xE9E, the same update; in the kit): the canister's sweep
  0.5 u; the bomblets leave 40–75° up at 4–12 u/s, their sweep 0.25 u, and their ball
  7 u. The Annihilator's V5 missile scatters the same bomblets (0x31E2D8 from 0x396A20),
  for a tenth of its damage.

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
  landing or its last shot, or more than 30 u from the hero, it bursts (0x4488B8, with 0.5
  and 13, not read) and goes.
- **V5 and up** (the Megaturret Glove, class 0xA65 (0x343088) and turret 0x1367,
  0x32DE00): the glove throws as the Miniturret's does (1.5–11 u, 15 u/s across,
  0.00306 u a frame²), the throw leaving 20 frames after the press, with at most 6 out.
  The turret:
  - finds a target all round (0x32ECC0, 0x44CF68) and turns to it (0x32EEE8);
  - fires a guided missile (0x32EFE8) when its count (+0x5C, set to 0.4167 s after each)
    has run out, from its two barrels in turn: the shared small missile (0x3441D0, class
    0xA82, flight 0x343330) of the row's damage, starting at 2 u/s and gaining 0.5 u/s a
    frame to 15 u/s, after its target, living 270 frames; its sphere (0.25 u) or step
    meeting something sets it off, everything within 1 u taking the damage (flags
    0x110000). After its shots (+0x60) it bursts (state 8, 0x4488B8) as the Miniturret;
  - every 180 frames (then 180–240), with the hero more than 20 u away and an enemy
    within 20 u of him, it picks a point 3–6.5 u round him and walks there (states 10,
    11; its path 0x32E950, 0x407010).
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
- **It goes** when its points run out, or he is in state 0x31, or other hero conditions
  (0x342FB0: the hero's +0xAA class, 0x1A71C4, state type 0x14, 0x1A7430).
- **V1–V4:** every 0.5 s (the first at once) a 0.9 u ball round its centre finds the
  living enemies (class category 5) touching it; each takes the row's damage (+0x6C),
  pushed straight out, flags 0x10000.
- **V5 and up** (the Tesla Barrier): in place of the zaps, a frame at a time, while
  fewer than four bolts are out, one living enemy within 4 u that has none gets a
  lightning bolt (0x34DAA0, the row's damage as it goes) for a random 20–60 frames. After
  its 40th bolt (0x1DD29C) the shield goes. A shield of the V1–V4 class still up once the
  gun is V5 is replaced by a V5 one, for nothing, when its points run out.
- Its colour runs from blue to red as its points go (0x202090 to 0xA02020 at V1, other
  pairs at V2–V5).

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
    record); each hit it takes gives the glove XP (row +0x68 × 32 / 150);
  - each hit plays a 30-frame reaction; from V3 (V3 and V4), at its 15th frame, a
    lightning bolt (0x34DAA0) strikes the first living enemy within 7.5 u; from V5 (the
    Ultrashield), every 10th frame of it, a pulse (0x372338, class 0x1869) of a quarter
    of the row's damage goes at the first enemy within 16 u that it can see (0x4C46A8).
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
  about. From V4 four agents come at once to the hero instead of a pod (they fly in;
  four slots at 0x1E2280).
- **An agent** (class 0x1ADA; spawned by 0x3C0F78, its update 0x3BD298, about 1500
  lines, read in part):
  - as it comes out it looks for an enemy within 25 u and 20° (0x44CD90) and, with one,
    chases it (state 5); otherwise it idles by the hero (state 0) while he is within 4 u,
    wanders to random points 2–6 u round him (state 1), and follows him (state 3) once
    farther. From V4 they jet in to slots round him (states 6–8, 0x10). Its run speed is
    a random 5.5–6.5 (+0x314). It finds a way by casting rays fanned 10° apart up to
    160° either side (0x3C1620, 12 u at least). Some classes are never its target
    (0x1A17, 0x1B27, 0x1AE6, 0x107E, 0x1D87). It keeps with the hero (checks at 30, 35
    and 59 u).
  - it ends (0x3C2140) when it comes within 2 u of its target's feet, when a 1 u ball
    0.3 u above its feet touches the target, 420 frames into a chase (+0x344), or 2000
    frames after it came out (+0x144): at V1 a 2 u blast of the row's damage, no push
    (flags 0x1010000); at V2–V4 it fires a comet (0x395480, class 0x1893, its update
    0x394068: a random 10–15 u/s halved, along his facing or at the lock-on target,
    living 680 frames), and whatever the comet meets is struck by a 2 u blast of the
    row's damage, pushed 1 (flags 0x10000); from V5 the Agents of Dread's blast (class
    0x188F, 0x3914C0): a ball growing to 8 u over 20 frames, which each frame strikes
    what it holds for the row's damage, pushed 2 out, and sets it alight (0x31AC38 by
    item 0x57). The agent then goes.
  - V2–V4 also give it a gun moby (+0x308, 0x453FC0). Its firing (0x3C0128, 2026-10-22)
    only happens from V5, in state 7 (by the hero), while a shared count (0x1E22B0) of
    shots lasts: every 80–120 frames, at an enemy within 25 u (0x44CD90) seen from its
    gun's joint, the shared small missile (0x3441D0) of a quarter of the row's damage,
    starting at 1–3 u/s and gaining 0.5–0.9 a frame to 12–17 u/s, living 100 frames. The
    Suck Cannon's V5 shots are the same comet as V2–V4's agents, with its own damage,
    which set what they hit alight (item 0x87).

### Infector (0x3688E8, blob 0x3DE4F0) [code, UYA NTSC, 2026-10-22]

- **Firing:** a Circle press (or R1 held in the aiming state), with the gun out more than
  22 frames and 45 frames since the last, takes a round and throws a blob (class 0x1E05,
  0x3DE3A8) from 0.75 u below the gun's origin in its frame, at 12 u/s along his facing
  (the camera's aim in the aiming state and with the lock-strafe or first-person camera).
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
  for each infection, as for a kill. What an infected enemy does (turns on the others) is
  in each enemy's own code, not read. Never infected: classes 0x188A and 0x1886 (the
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

### Spitting Hydra (0x31FB90) [code, UYA NTSC, 2026-10-22]

- **Its numbers by level:** the most targets 3, 4, 5, 6 (V1–V4) and 7 (V5 up); the
  spit's flight 30, 30, 35, 35 and 40 frames; the bangles 1, 3, 7 from V2.
- **Holding Circle** (with ammo, not mid-spit) counts up to 150 frames and plays the
  charge. Every 12 frames held (15 at V3–V4, 10 from V5, and at 150) it looks along his
  facing (the camera's aim in the aiming state and the lock-strafe camera) for targets
  within 32 u and 20° (`gun_aim_target`, at most 8), and marks them, one at a time, up
  to the level's most.
- **Letting go:**
  - before 20 frames: it fizzles (a sound; seven such taps bring up a hint);
  - with targets marked: it plays the firing animation (`hero_fire_anim`) and a lightning
    bolt (0x34DAA0) strikes each, and a 1 u ball there takes the row's damage (+0x6C),
    pushed 0.5 along the line from the hero, flags 0x410000; this takes no round (only the
    spit calls `ammo_use`);
  - with none: once its spit timer reaches the flight time it takes a round and spits.
- **The bolt on a marked target** (class 0x1195, 0x34DAA0; its update 0x34DD38 and strike
  0x34DFD0): it holds between the gun and the target for its life (0x1DD48C: 15 frames
  here), and as it goes the target, if still there, takes half the Hydra's damage again
  (flags 0x410000, pushed 0.5 along it). The same bolt serves the Holo Shield (the shield's
  row damage), the Rift Ripper (a tenth of the rift's) and the Tesla Barrier (the Shield
  Charger's), by its source's class.
- **The spit** isn't bolts that fly: it is the level's number of lightning arcs
  (0x321108 sets their ends, 0x321210 redraws them). Each reaches from the gun to a point
  32 u along the aim, jittered up to 0.2 u on each axis, bowing out round the aim on the
  way (rolled −107°, 0°, 107°… at V1–V2; from −80° by 52.5° at V3–V4; from −60° by 30°
  from V5). Its timer counts down from the flight time; when it reads 27 (frame 3, 3, 8,
  8, 13 after the spit at V1–V5), 0x321580 strikes once for each arc within 60 u: the line
  from the gun to its end is tested with a damage record (the row's damage, pushed 0.7
  along it, flags 0x410000), so the first thing it meets takes it, and if it met anything a
  blast of 1 u (2 u from V5) round the arc's end does the row's damage too. An arc that
  meets nothing does nothing.

### Disc Blade Gun (0x36EFC0, disc 0x36FA78) [code, UYA NTSC, 2026-10-22]

- **Firing:** while Circle is held (or R1 in the aiming state), with the gun out more
  than 22 frames, every 30 frames, a round a disc (class 0x1867, 0x36F720). Its target is
  the gun's assist: 20 u (35 from V4), far 30.5° by 30.5°, near 45° by 45° (50 u, 20° by
  10° with 0x1DB030 set). The disc leaves along his facing turned 90° to one side (60.5°
  with a target), at row +0x4C = 0.35 u a frame (21 u/s); from V5 at 42 u/s, from two
  muzzles in turn. With the lock-strafe camera it goes along the camera's aim, bent to a
  target within 30 u and 80° (0x4C7010).
- **Flight:** it spins and curves 2° a frame; with a target it flies straight at it each
  frame. It goes after 200 frames (400 from V4), or 81 u from the hero.
- **Hits:** each frame the line of its step against the world and a 1 u ball round it
  against targets; a target takes the row's damage (+0x6C), pushed by its velocity × 3,
  flags 0x10000:
  - V1: the first target it hits ends it; walls bounce it (velocity reflected);
  - V2: as V1, and each bounce turns it at an enemy within 10 u;
  - V3 and up: it bounces off enemies too, hitting each, and each bounce turns it at a
    new enemy within 10 u that it can see;
  - V4: its end is a 4 u blast of half the damage (0x32FE90);
  - V5 and up (the Multi-Disc Gun): bouncing off an enemy while fewer than 15 discs are
    out (0x1DD9E0), its nth split has odds of 1 in 2^((n − 1) / 2) of a new disc of the
    row's damage carrying the count, its velocity a frame given a random 0.1–0.2 on each
    axis (always up) and its speed back; a second roll at the same odds keeps it going,
    else it ends.

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
  - its pull grows 0.005 a frame to 0.75;
  - it looks within its size + 9 u (9.5 u) for an enemy, or a damageable moby, not of a
    few classes (0x1F8, 0x1699, 0x165C, 0x1612, 0x16FA, 0x16CB), whose health is at
    most twice the row's damage × its size, so at most the row's damage: it takes that
    one (0x3738B0), springs it in to its middle by the pull (0x445208), its parts
    spinning, and swallows it. One at a time; one dragged more than 50 u away is let go;
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
  1.4, 1.7 and 1.9 u at its first four links look for an enemy or a damageable moby (not
  classes 0x1F7, 0x1F8): it takes a 1-damage hit with flag 0x8000000, and an enemy whose
  record allows it (+0xF) and has more than 1 health left takes it, then it is held
  (0x363088); crates are taken too (0x362440).
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
- The V3 burning and the Vortex Cannon's comets aren't read.

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
  stands where it was, facing away from the hero.
- **V1–V3 ducks** (class 0x1BC3, spawn 0x3D2480, update 0x3D1728): it waddles about,
  idling with three animations picked at random (20%, 20%, 60%) and walking off, turned
  up to 45° either way, for 400–600 or 180–360 frames at a time. With the hero within 2 u
  it slows and turns. It lives 1400 frames, then goes in a puff of feathers (0x3D2870);
  damage of the player's (mask 0x8010000) ends it sooner.
- **Eggs from V2** (0x3D22B0): at frame 15 of its third idle animation a V2 or V3 duck
  lays an egg 0.75 u up, 160–200° off its facing, 10–50° up, at 2–5 u/s. The function
  also sets a 60–120 frame count on the duck that its update doesn't read.
- **The egg** (class 0x1C59, spawn 0x3D4768, flight 0x3D48B8): it falls under 0.00405 u
  a frame² (14.6 u/s²), its speed kept 0.995 a frame, bouncing. It lives 360–500 frames
  and then pops harmlessly. Once armed, an enemy (not a duck) within 2 u sets it off:
  that one enemy takes the row's full damage (flags 0x1030000), pushed 1 along the egg's
  way. **From V3** each bounce turns its ground velocity at the nearest enemy within 7 u.
- **V4 ducks** (class 0x189B, spawn 0x39E5D8, update 0x39DB60): it waddles the same, but
  each time an idle animation ends a 60% roll looks for the nearest enemy anywhere
  (0x44CD90, range 999) and runs at it, laying an egg every 60–120 frames (0x39EF18).
  It has no time limit. Where its run ends, when it is hurt, or when it falls off, it
  blows up (0x39E8A0): enemies within 3 u take a fifth of the row's damage (flags
  0x1010000).
- **The Qwack-O-Blitzer's duck** (class 0x1D3E, spawn 0x3D8840, update 0x3D7998, each
  frame 0x3D8420): it flies. It waits until a shared 60-frame count runs out, then keeps
  by the hero: at 10 u/s when more than 50 u off, else 3–6 u/s, and 6–8 u/s on its way
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
  - The first six guns' assist, the shock arc and lock-on take any hurtbox with hit points
    left, not only enemies (class category 5); the other fourteen's searches take only
    enemies (`rac_combat::Hostile`), as the game's do. XP counts whole points.
  - Not in yet: the V5-and-up extras (the Annihilator's bomblets, the Liquid Nitrogen
    Gun's freezing), the Flux Rifle's piercing (not found in code), the Nitro's aim at the
    ground with action 4, camera shake, the other guns and DL's.
- **The other fourteen** (2026-10-22; `weapons/seekers.rs`, `thrown.rs`, `grab.rs`,
  `hand.rs`), each as its section above reads it, from V5 on firing as V4 with V5's
  numbers:
  - the Plasma Coil's 10-tick spin, 0.5 s cooldown and five balls, the ball sticking to a
    target (its damage) or the world, and 15 ticks later arcs to the three best-scoring
    enemies within 10 u all round, each striking after 10 ticks (the second damage);
  - the Disc Blade Gun's disc every 30 ticks, leaving 90° (60.5° with a target) to
    alternate sides, flying at its target or curving 2° a tick back, bouncing off walls,
    from V2 retargeting on each bounce, from V3 through enemies, from V4 a 4 u blast of
    half its damage at its end;
  - the RY3NO marking the enemies its assist finds and firing a volley of 8 (16 from V2)
    missiles a round, each at the next mark, its cooldown 1.3 times longer from V2;
  - the Spitting Hydra marking an enemy every 12, 15 or 10 ticks held up to its level's
    most, striking each mark on letting go without a round, fizzling under 20 ticks, or
    spitting a fan of bolts for a round;
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
    four (from V4 four come to him), each running at the nearest enemy it can see and
    blowing up in 2 u when it touches it;
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
    the whip in hand; the rope hits along its reach (9.5 links of the stretch: 8.6, 5.7
    and 7.6 u) from the 7th tick of a swing and the 21st of the jump attack, and the
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
    repeat, 30 u reach and 7 bounces; the Nitro Eruptor's blast growing to 11 u.
  - **[placeholder] or [simplified]:** the agents' running, sight and life, the holo
    shield's size, the bounces' keep for the Bouncer, the turrets' slots, the rift's pull
    time, which enemies the Suck Cannon can take (UYA asks each enemy's record), the
    duck's XP, the ducks' falling (the game keeps them to the ground), the Megaturret
    missiles' turn and its walk speed, and the Plasma Storm's arcs, which strike as the
    coil's (0x30A3D0 not read). The whip's rope is a straight line along the wrench's,
    not nine trailing links, and a target takes one hit an attack (the game's queue
    allows one a frame). In the demo an infected enemy turns on the nearest other enemy
    it sees, which fights back, and a ducked one is gone until it comes back: the demo's
    own, since in UYA each enemy's own code decides. Not in: the agents' cannon (V5 and
    up, 0x3C0128), the Vortex Cannon's comets, ducks drawing enemy fire, the Lava Gun's
    and the Decimator's V5 (0x38A648 only calls 0x36D5A0), the Nitro Eruptor's 5° launch
    turn, the Suck Cannon's ricochets and burning, the turrets' and holo shields' XP for
    hits, and the shield's state 0x31 and other ends.
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
  `RigGuns`). Baked at rest: its own draw, idle and recoil sequences aren't played
  [simplified]. Without a disc the guns are blocky placeholders [placeholder]. The open
  model aims procedurally.
- **Pickups** (`rac_combat::pickups`, P1.10, 2026-10-22): a hero reaches pickups within
  8.7 u across and 3 u up or down from his feet [code]. Ammo follows "Ammo pickups, crates
  and the vendor": taken only for a gun with room, on landing or bouncing, or as it lands
  if he came in reach while it flew; at once 2 u below where it was thrown; its rounds go
  in capped (the rest lost) and it flies in for show from rest at 24 u/s² up to 64 u/s to
  0.7 u above his feet, carried along with him, shrinking over its last 30 ticks; never
  expires [code]. In co-op the nearest hero with room takes it. Ammo crates throw 1
  pickup, or 2 once in 5, for one gun of the breaker's, not full if any, never the Flux
  Rifle (`crate_ammo`) [code]; bolt crates pay their bolts a quarter more or less in
  coins of 30000 down to 1 [code], all of them, where UYA drops a few and banks the rest
  [simplified]. The demo's enemies drop no ammo, as UYA's don't. The crates' bolts (44, one
  of Veldin's sizes) [data]; how pickups pop out and bounce, and how bolts and nanotech
  fly in and expire, are the kit's [placeholder]. Rounds per pickup (+0x88) and the
  vendor's price a round (+0x84) are [data]; the vendor's pad, selling a round every 3
  ticks, is the kit's own.
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
  `0x1A71C8`/`0x1A71CD`), and the lock-strafe camera's own widget (0x41BE10, not read).
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
- **Tests:** `rac_combat`'s unit tests (the definitions, the arsenal, the assist, the lob
  and the pick), `rac_combat/tests/weapons.rs` (a test for each of the fourteen and most
  V5 variants, and one that fires each at V1 and V5 until its shots run out),
  `rac_hero/tests/gun.rs`, `rac_demo/tests/enemies.rs` (infected enemies fighting) and
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
2. **Still open in the archetypes:** the Flux Rifle's piercing from V2 and the
   Annihilator's V2–V4 mines are in neither the shots' nor the guns' code read
   (2026-10-20); look for them in the code that reads the moby's +0x7C, or in the
   damage-record flags the shots carry (0x100000 on the Flux's scoped hitscan from V4).
   Also the writer of the camera's aim point (`fps_aim_pos`, 0x1A6230), and the shock
   arc's own function (0x34C5F8).
3. **Projectiles in traces:** bullets, charges and missiles are mobys spawned per shot,
   so a fixed layout region can't follow them; capturing their speeds and the Nitro's
   arc needs `pine-probe` to read the moby list (or the class's newest moby) each frame.
4. **The other guns' remaining code:** the agents' running and cannon (0x3BD298,
   0x3C0128), the Plasma Storm's arc (0x30A3D0), the Decimator and Lava Gun's V5, the
   whip's nine rope links (classes 0x183A and 0x183C, in Veldin's core) and the Shield
   Charger's bubble (0xA5C; 0x1192 at V5) for the rig, and how an infected enemy fights
   (each enemy's own code; Veldin's attackers would show it).
5. **Deadlocked:** its `GadgetDef` table (0x3BF460, `moby-format.md`) and its mods.

How this was read, to carry on: import or reuse the Veldin project (copy
`cache/ghidra/proj/uya.{gpr,rep}` or `uya-weap` to a name of your own so sessions don't
collide), list a
global's users with `RefsTo.java`, and decompile them with `DecompileAt.java`; row fields
show as `DAT_002c56xx` (0x2C5640 + offset) in the output. Names come from the text table
(16-byte entries of string pointer and text number, 0x1C31440–0x1C3ED80 in this image).

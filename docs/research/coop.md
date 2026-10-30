# Co-op: how Deadlocked plays its campaign with two players

How Ratchet: Deadlocked runs its campaign for two players on one console (PLAN.md RE.10),
the reference for the co-op rules every game gets (P6.1) and for the kit's co-op (P1.18).
Started 2026-10-28 from DL NTSC-U's code.

## Status and method

- **Memory image:** DL NTSC-U (SCUS-97465) on DreadZone Station (level 1), the Ghidra
  import `dlntsc-dreadzone` (`docs/re-setup.md`). Level 1 is a single-player build, but
  the code shared by every level tests `coop_flag` at run time: about 290 instructions in
  206 functions read it. So most of the co-op rules can be read in a single-player image.
- **A co-op build's image:** `dlntsc-dreadzone-coop.p2s` (level 21, paused), imported as
  `dlntsc-coop`. Its code reads `coop_flag` 288 times in 197 functions, so the co-op
  builds carry the same game code as the single-player ones and the flag does the work.
  Use this image to check a reading against two live heroes.
- **In play** (2026-10-28): `dlntsc-dreadzone-coop-play.p2s`, both heroes in normal play
  on a `boot.sh` with controller 2 on the keyboard (`PAD2=1`), and the traces
  `dlntsc-dreadzone-coop-death-{1,2,3}` (`docs/re-setup.md`) check the start, death,
  respawn, restart and the team moves.
- **A co-op level with enemies** (2026-10-29): `dlntsc-catacrom-coop-play.p2s`, Catacrom
  Four's co-op build (level 22) at its first challenge's start, reached by unlocking the
  challenge in memory ("Planets and challenges"), imported as `dlntsc-catacrom`; the
  traces `dlntsc-catacrom-coop-fight-{1,3,4}` check kills, ammo and a respawn there, and
  `-fight-{5,6}` (2026-10-30) weapon XP's first holder and the tether, whose countdown
  `dlntsc-catacrom-coop-tether-{1,2}.p2s` catch.
- **Every level's placed mobys** come from the discs' gameplay files (`openrac-tool
  mobys`, `gameplay-format.md`, "Deadlocked").
- **Labels** as in `hero-controller.md`: **[code]** read in the game's code,
  **[verified]** checked in a savestate or trace, **[inferred]** from how a value is used.
- **Addresses are DreadZone Station's build** (levels 1 and 21 share it). Each level has
  its own build of the game with its own `.data` and `.bss` (`engine-notes.md`, "Code
  layout and overlays"), so globals move as well as code [verified]. Catacrom Four's
  co-op build has `coop_flag` at 0x21E6F8, `multiplayer_flag` 0x21E6F4 and `split_mode`
  0x21E6E8 (each 0x130 lower), the hero pointers at 0x362C78 (heroes 0x365AE0 and
  0x368CD0), the safe-spot rings at 0x266A40, `coop_hp_scale` at 0x220380, `main_loop`'s
  co-op set-up at 0x4DF634 and the ammo pickup at 0x3C4800. `level_current`,
  `game_mode`, `level_next`, the request flag, and the save's data (`level_save`, bolts,
  XP, the gadget box) stay put. `tools/ghidra/match-function.py` finds a function of one
  build in another's memory and prints the addresses that changed.

## In short

- **Co-op is the campaign's levels 21–35,** two-player builds of levels 1–15 running the
  same code, which tests a flag. Progress is the single-player level's.
- **Player 2 is a second Ratchet** with the same moves and constants, his own skin,
  camera, camera setting, pad, health, gun in hand and quick-select.
- **Everything else is one pool:** bolts, XP, maximum health, and a single gadget box, so
  the guns, their ammo, levels and mods. Power-ups go to both players. Ammo pickups give
  2.4 times as much in co-op.
- **Death:** a dead player waits 6 s, then comes back near where the partner stood a
  moment ago, at full health. Only both players dead at once restarts the level.
  There's no revive.
- **A tether keeps the players together:** where a level places one, players further
  apart than its distance (40 u on Catacrom Four) start its countdown (10 s), shown with
  a beam between them going from blue to red; at zero both die and the level restarts.
- **Difficulty** isn't raised for two players; enemies' hit points can be scaled by a
  co-op factor, 1.0 so far.
- **Split screen** stacks the two views (staggered halves in widescreen), draws at 30
  frames a second over the 60 Hz simulation, and goes full screen for cutscenes,
  transports and menus. Either pad drives a menu, and the pause menu stops both.
- **Player 2 replaces the battle bots,** which co-op removes with their vendor. With a
  team meter filled from XP, both players clicking L3 together give both 30 s of ×4
  damage, and R3 together a 30 s shield that, in co-op, is only for show.

## Starting co-op [code]

- **Co-op is chosen by the level number alone.** Levels 21–35 are the campaign's
  two-player builds of levels 1–15 (`engine-notes.md`, "Co-op and split-screen"). Before
  its loop, `main_loop` (0x528098) tests `level − 21 < 15` (0x666600) on a campaign level
  (1–39):
  - co-op: the local player count (`local_player_count`, 0x172174, reached through the
    pointer at 0x21DDA4) is 2, `coop_flag` (0x21E828) is 1 and `split_mode` (0x21E818,
    the views beyond the first; "Split screen") is 1;
  - otherwise the count is 1 and both flags are 0.
  - 0x172180 takes a copy of the count. `multiplayer_flag` (0x21E824) is 0 either way.
- **Player 2's skin** is `skin_p2` (0x171B74, a skin-table entry kept in the save), which
  `main_loop` loads into player 2's slot (0x5756D0 with slot 1) as it loads `skin_p1`
  into slot 0. Out of co-op, slot 1 is emptied.
- **The two heroes** [code; verified in `dlntsc-dreadzone-coop.p2s`]: the pointers at
  0x3660F8 and 0x3660FC are player 1's and player 2's heroes. In single player 0x3660FC
  is 0 and 0x3660F8 holds 0x368F60, the hero of `re/SCUS-97465/hero.toml`. The hero
  set-up (0x6A8420, per player index) lays the heroes 0x31F0 bytes apart, so player 2's
  is at 0x36C150.
  - hero+0x2F10 is the player index (0 and 1 in the savestate), with a byte copy at
    +0x2FEA; every "the other player" test reads +0x2F10. Both heroes have `IsLocal`
    (hero+0x266D) set.
  - Each hero has its own camera (hero+0x2EFC: 0x230280 + i × 0x470, so `game_camera0`
    and the next) and its own pad block (hero+0x2F00): 0x1EE600 for player 1, and for
    player 2 port 2's pad or, when port 2 is empty, 0x1EEBC0 ("Split screen"). In the
    savestate, whose port 2 is empty, it is 0x1EEBC0.
  - Both heroes use the same movement constants (hero+0x2EEC, 0x3BD930).
  - **Created at level start** (0x57CA50 → 0x6A83B0 → 0x6A8420): hero 0 always, hero 1
    only on a campaign level with `coop_flag` set. The set-up is the same for both,
    parameterised by the index; for player 2 it also marks its moby (+0x74 = 1, flag
    0x8000 at +0x36).
  - **Settings per player:** the camera setting is one word per player from 0x1711D0
    (`camera_option` is player 1's), copied into a camera-type byte per player at
    0x171DE0. Three more words per player are copied the same way (from 0x171D50,
    0x171D80, 0x171DB0); what they hold isn't read.
  - **Where they start** [verified, `dlntsc-dreadzone-coop-death-2`]: after the level
    (re)loads, player 1 stands at the single-player start, (558.48, 330.26, 184.17) on
    DreadZone Station, and player 2 0.9 u beside him at (557.93, 329.54, 184.17). After
    our warp from a single-player state player 1 began at (652.0, 329.85) instead.
- **Code that tests co-op usually tests both flags:** `multiplayer_flag == 0 &&
  coop_flag == 1`. A few places test `coop_flag < 2`, which is true in single player too.

## Levels and progress [code]

- **A co-op level stands for its single-player level.** Wherever a table is indexed by
  level, co-op subtracts 20 (or 21 for a 0-based index): the mission tables
  (0x6C12A8 called from 0x48AE20 and 0x4FB2A8), the level's per-mission data (0x515F50,
  0x516CE8), the planet list (0x600D08, 0x600E50, 0x601138) and others (0x63FD68,
  0x63FDE0, 0x630A58). Code that names a level adds 20: DreadZone Station is 21 rather
  than 1 (0x608C40, 0x608D48, 0x608E98, 0x601E58, 0x5FF758), and a change to the station's
  interior requests 35 rather than 15 (0x510848).
- **Level progress is shared with single player:** each level has a 0x304-byte record
  (`level_save`, 0x1F87F0), indexed by `(level − 1) % 20`, so level 21 writes level 1's
  record ("Planets and challenges" below has its layout).
- **The death counters** (0x21EBE4, 0x21DA74 and one per level at 0x171F70 + 4 ×
  `((level − 1) % 20)`) count only in single player (`hero_death_done`, 0x67A4F8); co-op
  counts each player's deaths in their statistics instead.
- **On the disc** levels 21–35 have level files of their own but use level − 20's audio
  and scenes (`disc-layout.md`, "Level data (DL NTSC-U and Gladiator PAL)").

### Planets and challenges [code]

How a planet and its challenges unlock, read to reach a co-op level with enemies (a new
game's planets arrive on an empty challenge menu). Names are in
`re/SCUS-97465/symbols.toml`.

- **Tables in the game:** `planet_of_level` (0x3C0220) maps `level % 20` to one of 13
  planet records (0x3C0278, 0x38 bytes: level, text ids, first and last mission id at
  +0x10 and +0x14, an unlock rule at +0x20). Missions are records of 0x60 bytes by id
  (0x3C0550): planet, text ids, kind (+0x14: 0 a challenge, 1 a bonus), unlisted
  (+0x18), points and bolts won (+0x1C, +0x20, halved for each earlier win), a medal
  (+0x24), items given on starting and winning, and an unlock rule at +0x38.
- **An unlock rule** (`unlock_rule_met`, 0x6C1748) is six ints: points (0x171B4C) at
  least the first; at least *n* medals of one of five kinds (counters 0x171B61–0x171B65);
  a rank (0x171B76); a mission won; a story bit (0x171C08). −1 (or 0 for the mission)
  means no requirement.
- **The save's record per level** (`level_save` + 0x304 × ((level − 1) % 20)):
  - +0x8 + 12 × *i*, challenge *i*: a flag byte (1 listed, 2 unlocked, 4 played: its
    starting items given), then the times won, then a best.
  - +0x300, the planet: 1 visited, 2 unlocked, 8 every challenge won, 0x10 every bonus
    won.
- **The update** (`planet_progress_update`, 0x584828, for levels 1–15 from 0x584678)
  only ever sets bits: the planet's from its rule, then, once it is unlocked, each
  challenge's listed bit (unless unlisted) and its unlocked bit from its rule. A
  challenge whose rule names no mission (−1) also needs the every-challenge-won bit, so
  a planet's bonuses come after its challenges.
- **The menu** (`challenge_menu_build`, 0x5FDF00) lists the challenges whose listed bit
  is set (`challenge_listed`, 0x584DD8; a won challenge drops out until all are won); a
  line whose unlocked bit is clear won't start.
- **Catacrom Four (level 2)** has missions 0x2E–0x34: four challenges, each needing the
  one before won, then three bonuses. Its rule asks for one medal of kind 3; the next
  planets ask for points too (Sarathos 5,000 and two medals).
- **Checked in play** [verified, 2026-10-29]: in `dlntsc-dreadzone-coop-play.p2s`
  (level 21), one kind-3 medal (0x171B63 = 1), Catacrom Four's flags (0x1F8DF4 = 3) and
  its first challenge's (0x1F8AFC = 3) were written, then level 22 requested. The game
  played the planet's intro scene and started the first challenge without showing the
  menu, in co-op. Both flag bytes then read 7: the challenge's played bit, and a planet
  bit 4 not read yet.

## Death, respawn and failure [code]

- **Single player:** a death (`hero_death_done`, 0x67A4F8) counts itself, sets the
  report 0x368BE8 to 1, and `main_loop` restarts the level (0x5299F8). That is DL's
  reload after a death (`hero-controller.md`, "Getting hit and dying").
- **In co-op a death doesn't end the level.** The same function sets health to 0, counts
  the death in that player's statistics (0x20B490 + 0x2A0 × player) instead of the
  single-player counters, puts **360 frames (6 s)** in hero+0x3B2 and enters **state 153
  (0x99)**, dead and waiting. The team moves' power-ups run on through a death: their
  clearing (hero+0x2FB0 back to 1, +0x2FB4 to 0, timers +0x40A and +0x408 to 0) is in
  the multiplayer branch, with the respawn time taken from the game's settings
  [corrected 2026-10-30; verified, `dlntsc-catacrom-coop-fight-7`: player 2's armour 3
  and its timer outlasted his death and respawn].
- **State 153** (`hero_transitions` 0x69EA40, case 0x99) counts the 6 s down, showing that
  player a countdown on their HUD (text 0x25FB with the frames left).
  - **If the other hero is in state 153 too, the level restarts** as after a
    single-player death: the death counter 0x21EBE4, the report 0x368BE8 and 0x1711B9
    are set. So co-op fails only when both players are dead at once, that is, the
    second dies within the first's 6 s.
  - One exception: on level 30 (co-op Torval) in challenge 4 (0x21EBD8), a death sets
    the timer from a float instead and writes 2 to 0x21EBDC, which 0x510848 sets to 1 on
    a success. So a death fails that challenge [inferred].
  - **When the 6 s are up he respawns** (`hero_respawn`, 0x67ACB0) with full health
    (hero+0x2F24) and then 47 frames of safe time (hero+0x39C, as after a hit).
- **No revive by the partner:** nothing but the countdown writes hero+0x3B2 in state
  153, and no other way out of it was found.
- **Where he comes back:** each hero keeps a ring of its last 8 safe spots (position and
  rotation, 0x20 bytes each) at 0x269EC0 + 0x110 × player, newest index at +0x100 and
  count at +0x104.
  - The hero's update adds a spot (0x585DE0) when he stands on something, at least 4 u
    from the newest spot (0x586068), outside a few excluded situations (hero+0x430 = 12,
    one moby class on DreadZone Station).
  - **In co-op the search (0x585E78) tries the partner's ring first**, newest first, for
    a spot at least 2 u from the partner that passes a check (0x5861E0: up to 8
    registered objects may veto or move it, then a ground probe). Then his own ring the
    same way, then his own newest spot, then his fallback point (hero+0x2F70 position,
    +0x2F80 rotation, taken where he stood at set-up). So he comes back where the
    partner stood a moment ago, not on top of him.
  - **Vehicles:** if he was riding a hoverbike (class 0x2054) when he died and the
    partner is on one, or either left one recently (frame limits 0x220878 and
    0x22087C), the respawn puts him on the free hoverbike nearest the partner
    (0x6B33D8).
- **No menus while a player is down:** `game_mode_request` (0x5A9608), through which the
  pause menu and other screens change the game mode, refuses while any hero has 0
  health, and 0x5A9808, a check of the same kind that menus call, does too. In co-op
  both let it through if 0x21E904 is set; nothing in level 1's image writes 0x21E904.
- **Checked in traces** [verified, `dlntsc-dreadzone-coop-death-{1,2,3}`, level 21]:
  - one death: state 57, then 153 with 360 in hero+0x3B2; 361 frames later he stands
    again (state 0) with full health and 47 frames of safe time;
  - both dead: the tick after the second hero enters 153, the report 0x368BE8 and
    `coop_both_dead` are set, and the level restarts (the frame counter goes back to 0);
  - the spot: the partner's newest spot, 0.15 u from him, was skipped (under 2 u), and
    the next one, 4 u away, was taken; then 0x5861E0 moved it, rewriting the ring's entry,
    to (603.73, 455.29, 200.50), 131 u from the partner. So on DreadZone Station's start
    floor a level object sends every co-op respawn to one point; elsewhere the spot is
    the partner's.
  - **Away from the station** [verified, `dlntsc-catacrom-coop-fight-4`, Catacrom Four]:
    player 1, killed 16 u up the path, stood again about 400 frames later on player 2's
    newest safe spot, (127.20, 301.76, 65.15), 3.45 u behind him; the rings' spots
    (0x266A40 in that build) are 4.0–4.1 u apart along the path walked.
  - A take where both players died within 6 s of each other restarted the level with
    the bolts, XP, gun XP and ammo they had (`-fight-1` and `-3`).
- **Enemies that test both heroes' health** (for example a boss at 0x4E2FD8) keep
  attacking while either player is alive; they don't fail the mission themselves.

### The tether [code, Catacrom Four's build; verified in play]

A level object, moby class 0x247B (update 0x3FEDB0 in Catacrom Four's co-op build),
keeps the two players within a distance of each other. Out of co-op (`coop_flag` 0, or
`multiplayer_flag` set) it removes itself on its first update (0x539580).

- **Its pvars** (0x120 bytes): +0x00 the distance (float), +0x04 the time in seconds
  (int), +0x08 a volume (−1 for none) and +0x0C whether the heroes must be inside it (0)
  or outside it (nonzero) for the tether to apply (0x552368 tests a position against it).
  Then its state: +0x10 a bit per player already dead when it ran out, +0x14 the whole
  seconds last shown (−1 at rest), +0x18 the frames left, and from +0x20 two beams of two
  segments, 0x40 bytes each, with their colours and end points.
  - Catacrom Four's co-op build has one (uid 21) with 40.0 u, 10 s and no volume
    [verified, savestate in the countdown].
- **Each frame**, from the hero list (0x362C78 there), it measures the distance between
  the first two heroes' positions (hero+0xA0). The countdown runs only while all of these
  hold; otherwise it is reset (frames back to seconds × 60, the loop sound stopped):
  - the level is more than 4 frames old, the game mode isn't 1 or 2, no camera or
    cutscene check holds (a global's +0x86 isn't 0x13; 0x4ED390 for no player), and
    0x21EA9C is clear;
  - each hero is alive and in none of the death states (57, 104 quicksand, 106 drown,
    118, 122, 123 lava, 126, 127, 148, 153) nor grinding (39–42, 62, 63), hero+0x25D4
    isn't 15, and any vehicle he rides isn't being destroyed (its state 7, 8 or 10);
  - the volume test passes, when there is one;
  - **the heroes are further apart than the distance**, doubled while either rides a
    vehicle (hero+0x2EF4) other than class 0x263E.
- **While it runs** (frames counted down from seconds × 60, `timer_count_down`'s kind):
  - each hero gets a beam from one of his joints (4) towards the other, its colour mixed
    from red (0x803030F0) to blue (0x80F03030) by the fraction of the time left, so blue
    at the start and red at the end;
  - the seconds left, with two decimals, in the middle of the screen in the same colour,
    at a scale between 2.0 and 1.5 by the time left (0x546E50, not read); each new
    whole second plays sound 1, rumbles
    both pads (effect 6) and flashes the text;
  - a loop sound (4) on the object, a screen effect of strength 0.07 × the fraction gone
    (0x39D51C), and a help message, text 0x2F53, unless 0x1F5E70 is set.
- **When it runs out,** each hero still alive takes 1000 damage (`hero_damage`, 0x619398;
  hero+0x41E is set to 1), so both die together and the level restarts ("Death, respawn
  and failure" above). A hero riding a vehicle loses the vehicle instead (its moby's
  state to 9).
- [verified, `dlntsc-catacrom-coop-fight-5`] Player 1 stood at the start while player 2
  walked up the path: 605 frames after the heroes first stood more than 40 u apart, both
  heroes' health went to 0 on the same frame (player 1's from full), and 85 frames later
  the level restarted; the same again after the restart. A savestate in the countdown
  had 232 frames left at +0x18, and 166 66 frames later. With both walking together
  (takes 1, 3 and 6, never more than 30 u apart) it never ran.
- **Where** [verified on both discs, `openrac-tool mobys --class 0x247B`]: one tether in
  each of levels 2 (Catacrom Four), 4 (Sarathos), 5 (the Dark Cathedral), 6 (Shaar), 7
  (Valix), 8 (the Mining Facility), 14 (Ghost Station) and 15, and in their co-op builds
  22, 24–28, 34 and 35, the same instance (uid and place) in both. **All sixteen have
  40 u, 10 s and no volume.** DreadZone Station (1, 21), Torval (10, 30), Tempus (11, 31)
  and Maraxus (13, 33) have none, so there the players may part. (Names from libdl's
  multiplayer maps, whose ids are the campaign levels' plus 40.)
- **Gladiator PAL** [code, `match-function.py`]: the same placements and values, and the
  same code (0x494538 in its DreadZone Station build, 0x492D40 in DL's) with each 60.0
  (the start frames, the colour's fraction, the reset and the seconds shown) changed to
  50.0, so the countdown is 10 real seconds in both regions.
- Not read: what text 0x2F53 says, the beam's texture (0x39D4E0), the camera check at
  +0x86, and 0x4ED390.

## What the players share [code; checked in `dlntsc-dreadzone-coop.p2s` where marked]

| Shared by both | Per player |
|---|---|
| Bolts: one purse, 0x171B40. A bolt pickup (0x46AE78) adds its value with no player involved; the vendor and mission rewards use the same purse. | Health (hero+0x2E20). |
| XP: one total, 0x171B48, added by every kill. | The item in hand (hero+0x2318), saved each frame to 0x220810 + 4 × player and given back at the next level. |
| Maximum health: one level, 0x171B50 (10 in a new game), worked out from the XP (table 0x3B6458); when it rises both heroes are healed and both get the effect (0x68F398, 0x3E0AB0) [verified: XP written to 250 made both HUDs 11/11]. | Quick-select: 3 slots each at 0x20C690 + 0xC × player (libdl's `WEAPON_EQUIPSLOT`) [verified: player 1's were 3, 2, 0 and player 2's 2, 3, 0]. |
| **One gadget box** (0x1D3F90): in the campaign every hero's box pointer (hero+0x26E4) is set to it (0x6909C8) [verified: both heroes], so guns, ammo, weapon XP, levels and mods are shared. Multiplayer levels give each player a box of their own, 0x12B0 bytes apart. | The skin (`skin_p1`, `skin_p2`). |
| The vendor: one shop and one purse; a weapon bought goes into the shared box and is equipped for the player who bought it (`vendor_buy`, 0x632CA0). | Statistics: 0x2A0 bytes per player from about 0x20B480 (ammo picked up, kills by gadget, mod and enemy class, deaths), and a kill count at 0x35944C + 4 × player. The statistics screen shows player 1's plus player 2's (0x617FE8). |
| Power-ups: the damage power-up (×4 damage, hero+0x2FB0, for 1800 frames, 30 s; 0x400100) and a 180-frame one (+0x41C; 0x3FFC88) are given to both heroes in co-op. | The camera setting and three other settings ("Starting co-op"). |

- **A new co-op box** (0x6909C8) gets the same starting kit as single player (the wrench,
  Dual Vipers, Magma Cannon, items 0xC and 0xE, an alpha mod) plus item 7, libdl's B6
  Obliterator, which only co-op gives.
- **Ammo pickups go to the nearest living player** within his pickup radius (hero+0x2E24,
  height +0x2E28) whose box isn't full for that gun (0x3D7900, 0x3D81E8). It goes into
  the shared box; the collector's statistics and HUD show it.
  - [verified, Catacrom Four, 2026-10-29] An ammo crate moved in front of each player in
    turn and broken with his wrench: player 1 took "12 Magma Cannon ammo" (5 to 17 of
    25) and player 2 "48 Dual Viper ammo" (100 to 148 of 200), each message on the
    collector's view and the count on both HUDs. That is the maximum × 0.2 (difficulty
    1) × 0.5 for the Vipers × 2.4.
- **Weapon XP from a kill** (0x590990) [code; corrected 2026-10-29]: the kill's XP goes
  to the gun that made it (the damage's item, +0x46), once for each player from the
  first one holding that gun in hand (hero+0x2318) to the last; if nobody holds it,
  from player 1. The killer's index isn't used. With one shared box, a kill with a gun
  player 1 holds adds its XP twice; one with a gun only player 2 holds adds it once.
  - [verified, `dlntsc-catacrom-coop-fight-{1,3}`] With player 1 on the Dual Vipers and
    player 2 on the Magma Cannon: every Magma Cannon kill added its XP once (`xp_total`
    +1, +3, +4 with the gun's XP the same), and Viper kills twice (+1 → +2, +3 → +6).
    Some kills added `xp_total` with no gun XP (none of the two guns made them).
  - [verified, `dlntsc-catacrom-coop-fight-{5,6}`, 2026-10-30] **The first holder, not
    the killer:** with player 1's item in hand written to 3 (the Magma Cannon) and player
    1 never firing (the write holds until he fires), every Magma Cannon kill by player 2
    added its XP twice: `xp_total` +1 with the gun's XP +2, three times in take 5, and +2
    with +4 in take 6. In takes 1 and 3, with player 1 on the Vipers, the same kills
    added it once.
- **Skins in the vendor:** one menu with two preview models; 0x34AF78 says whose skin is
  being chosen, and each player's choice is worn on leaving (0x61B9A8–0x61D238). Skins
  are bought from the shared purse into the shared `skins_owned`.
- **Items 11, 12, 13 and 16** can't be equipped in single player but can in co-op
  (0x688578 asks 0x6BDDC0 only out of co-op). What those items are isn't checked.

## Difficulty and co-op scaling [code]

- **Co-op keeps the chosen difficulty** (0x21DF64, 1 by default); nothing scales it for
  two players. 0x5868E0 can lower it by one after a death report.
- **The difficulty table** (0x3BD3D0) holds nine floats per difficulty 0–4. Two columns
  are read so far: +0x0C, the share of a gun's maximum ammo a pickup gives (0.2; 0.25 on
  the hardest), and +0x14 (0.75, 1, 1.1, 1.4, 1.6), a multiplier enemy shots ask for
  through 0x64EEA8, which also applies 0.75 while the player is in third person or riding
  vehicle class 0x20AE (in co-op: both players in third person, or either riding).
- **Co-op's own scaling:**
  - **Ammo pickups give 2.4 times as much** (0x3D7900), as two players draw on one box
    [verified on Catacrom Four: "What the players share"].
  - **Enemy hit points** are multiplied by the float 0x2204C0 at the level's moby set-up
    (0x64ED20, from 0x578940), only in co-op. **It is 1.0, so co-op enemies keep their
    hit points** [verified]: 1.0 in levels 1, 21 and 22 (Catacrom Four's co-op build,
    where it is 0x220380), and in level 22's build too only that function reads it and
    nothing writes it. It is an initialised value in each build's data, not set at run
    time.
  - Nothing else read so far scales with the second player: no change was found to
    enemy damage, enemy counts or the difficulty.

## Co-op's own moves [code; verified in play on level 21]

- **A team meter:** each hero's byte hero+0x2FB8 is set once the XP earned since the last
  reset (0x171B54 − 0x171B70) reaches one health level's span (0x171B78), with the
  progress in +0x2FBC (0x68F238). In co-op only (`hero_update_timers`, 0x684EF0), with
  the meter full:
  - **both players clicking L3 within 30 frames of each other** (`pad_pressed_within`
    on both pads) gives the hero the damage power-up, ×4 (libdl's `DamageMultiplier`,
    +0x2FB0) for 1800 frames (30 s);
  - **both clicking R3** sets +0x2FB4 (libdl's `ArmorLevel`) to 3 for 1800 frames (timer
    +0x408), and a bubble shield shows round the hero.
  - **The R3 shield takes nothing off the damage in co-op** [code; verified]:
    `hero_damage` (0x6693A8) multiplies a hit by the armour table (0x3BD9A0: 1, 0.75,
    0.5, 0.25, 0 by hero+0x2FB4) only when `multiplayer_flag` is 1; in the campaign a
    running armour timer multiplies it by 1.0. Nothing else reads the armour but the
    bubble's drawing (0x66ACA0, which fades it over its last 30 frames and blinks it
    through its last 4 s) and the hit state (0x6790C8), which doubles one kind of hit's
    push (a damage record of type 4 carrying its own direction) only while the armour
    is 0. In `dlntsc-catacrom-coop-fight-7`, with both heroes on armour 3, hits whose
    raw damage (hero+0x2E80) was 3 and 4 took 3 and 4 health (armour 3 would have left
    1). So in co-op the R3 move shows a bubble and spares that one push.
  - Then that hero's meter resets (0x68F2D8 clears his +0x2FB8 and +0x2FBC and moves the
    shared XP baseline 0x171B70). Each hero makes the check in his own update.
  - **Checked in play:** with the meter's XP (0x171B54) written past the span, both
    players' L3 together gave both heroes ×4 with 1771 and 1769 frames left when read,
    and R3 together gave both armour 3 and the shield; both meters emptied. A rise in
    the health level also resets the meter.

## Split screen [code; the layout verified in `dlntsc-dreadzone-coop.p2s`]

- **The number of views:** 0x21E818 (`split_mode`) counts the views beyond the first: 0
  in single player, 1 in co-op, the player count − 1 in multiplayer (up to 3). `main_loop`
  sets it.
- **Two views stacked top and bottom.** A table at 0x3B65B0 holds one 0x28-byte layout
  per view: the rectangle (x0, x1, y0, y1 as fractions of the screen), a scale, an
  offset, a flag and an aspect factor; 0x548C48 picks player 1's and player 2's layouts
  for `split_mode` 1 [table verified in memory]:
  - **4:3:** player 1 at x 0–1, y 0–0.5 (scale 1 × 0.5, offset (0, −0.25)) and player 2 at
    x 0–1, y 0.5–1 (offset (0, +0.25)), both with aspect factor 1.333.
  - **Widescreen** (the byte 0x171DEB, set in both our images; it also divides the
    vertical field by 4/3 in 0x546B38, so it is taken to be the 16:9 option): the halves
    are staggered rather than full width. Player 1 is at x 0–0.6875 on top, player 2 at
    x 0.3125–1 below (scale 0.6875 × 0.5, offsets ∓0.15625 and ∓0.25, aspect factor
    1.375; table at 0x3B6740 through the pointers at 0x3B6790). **The savestate's
    screenshot shows exactly this:** player 1 in red armour with the Magma Cannon top
    left, player 2 in blue with the Dual Vipers bottom right, each with its own HUD.
  - **Between and beside the views** (0x527CE0): a one-pixel black line across the
    middle, and in widescreen the empty strip beside each half is filled with a texture
    tinted in that player's HUD colour (red beside player 1, blue beside player 2 in the
    screenshot). 0x527B08 loads the textures at the level's start when there are two or
    three players.
  - The projection (0x546B38) can divide the vertical field further by 1.66666 for two
    views when the word 0x2400A8 is set; it is 0 in both images, so it isn't known
    whether co-op ever sets it.
- **Split screen draws at 30 frames a second and simulates at 60.** While `split_mode` is
  not 0, `main_loop` runs its own loop (0x529F38): draw every view once, run a game tick
  (0x529EA8), wait for the vertical blank if early, run a second tick, then update the
  cameras (0x538610). If the frame overran (the counter past 0x4B00 on NTSC, 0x5A00
  otherwise) it runs a third tick. [code]
  - The cameras therefore update once per drawn frame, not inside each tick, and
    `camera_spring` doubles its rate and cap (60/30) with two or more local players.
  - With two players the pads are read in both ticks; with three or four the second
    tick only clears their new presses.
- **Each player's own camera:** a camera per hero (0x230280 + 0x470 × player), with that
  player's own camera setting (third person, lock-strafe or first person), speed and
  inversion ("Starting co-op").
- **Player 2's controller** is the pad in port 2. Every frame in co-op, 0x529310 moves
  player 2 to the pad block at 0x1EEBC0 (index 1 in steps of 0x5C0 from player 1's;
  taken to be a multitap slot) if port 2 reports no pad and that one does, and back
  again. In our savestate, whose port 2 is empty, player 2 is on 0x1EEBC0; with a pad in
  port 2 he reads 0x1EFD00, and each pad moves only its own hero [verified].
- **The opening tutorial** on DreadZone Station runs in co-op too, its banner on the
  split line, and both pads did each step together [seen].

## Full screen for cutscenes and transitions [code]

- **The game drops to one view and back** through a pending request, 0x21E81C (−1 for
  none), which `main_loop` applies at the end of the frame (keeping the old value in
  0x21E820):
  - **Path cameras** (camera mode 0x13, the scripted flythroughs; 0x522F20) ask for one
    view as they start in co-op and for two again when they end. So cutscenes play full
    screen.
  - **Their dialogue lines** each carry a tag: 0 always, 1 single player only, 2 co-op
    only. So co-op has lines of its own.
  - **DreadZone's transport ship** (class 0x2126, 0x510848 and 0x511B50) asks for one
    view while it runs and two again after; it takes the players to level 35 rather
    than 15 in co-op.
  - A new weapon's screen (around 0x6BF5D8) and a fade to another level (0x5A9030) ask
    for one view.
  - **Menus** (0x5DB500, 0x5DB590) save `split_mode` in 0x222EE4, show one view and give
    it back after. [verified: paused in the savestate, `split_mode` is 0 and 0x222EE4
    holds 1]
- At a level's start (game mode 15) the hero and cameras are updated only in single
  player (0x566540).

## HUD and menus [code]

- **The HUD is drawn per player:** its entry points take a player index, 0 or 1, with 4
  for the shared full-screen layer (0x5D7598, 0x5D7318, 0x5D74F0, 0x5D7518, 0x5D76A8);
  0x65A778 returns the player whose HUD is being built.
  - Each player has a mask of HUD elements at 0x32ED64 + 0x10 × player (0x5D76A8). The
    defaults (0x5DAA90) are 0x37 in single player and 0x3F in co-op, adding bit 0x08;
    which element that is isn't known.
  - Per-player elements take their positions from tables by the player's layout (for
    example 0.5, 0.8 on a full screen and 0.5, 0.35 in a top half).
  - **Shared messages move to the split line:** the banner sits at y 0.4 instead of 0.9
    (0x5E3938), and a pop-up is centred at y 0.5 (0x5E5980). With two local heroes in
    widescreen the banner takes a two-player layout table (0x3ADE08).
- **Either controller drives a menu:** 0x5D8CD0 reads both players' new menu presses and
  acts on whichever pressed, noting who in 0x21EFE8.
- **The pause menu and other full menus are one shared screen.** Each player has a screen
  state (0x31B2F8 + 0x54 × player), but `ui_screen_set` (0x5D87A0) works on player 1's
  outside multiplayer. In co-op the HUD screen (1) is copied to player 2, and any other
  screen sends player 2 back to his HUD. The pause screen asks for game mode 3, which
  stops the game for both.
  - While a player is down the game mode can't change ("Death, respawn and failure").
- **A per-player choice of four** (0x5EFD20–0x5F0C10, entries at 0x21F178) dims to 20%
  and refuses an entry the other player holds (0x5D8A40, which reads the other hero's
  equipped items +0x2674, else +0x268C). In single player the same screen lists other
  entries (0x3B8950), taken to be the bots' commands. What it offers in co-op isn't
  known.
- **Start in co-op** (0x630438): from a level's eighth frame, a player's Start press
  latches hero+0x2EDC and cuts the dialogue playing (0x570470). Whether that is how
  co-op pauses isn't read.

## Missions and mobys [code]

- **The current challenge** is 0x21EBD8, its index on the planet. A mission's id is the
  planet record's first id plus that index (0x6C12A8), and the planet is looked up by
  `level % 20` (0x6C1278, 0x3C0220), so a co-op level plays its planet's missions.
- **Removed in co-op**, each on its first update (`moby_destroy`, 0x586E80; class names
  from libdl's moby list):
  - **the battle bots** (class 0x268B, 0x4A1F90) unless their flag +0x67B is set: player
    2 takes the bots' place, as `engine-notes.md` says;
  - the bots' upgrade vendor (0x2643, 0x5148C0);
  - on DreadZone Station before challenge 8, two lights (0x245B, 0x245E; 0x3D1318,
    0x3D1EC8);
  - a bonus bolt crate (0x1F7) whose +0x228 is set (0x3FE648).
- DreadZone's per-mission script (0x3D8858) plays many of its sounds and objectives only
  in single player; the jump pad (0x2072, 0x4B04F8) also tests co-op. Neither is read.

## Gladiator PAL [code, 2026-10-30]

Gladiator PAL (SCES-53285) plays co-op as DL does, in the same real time.

- **Method:** every code address this file cites in DreadZone Station's build (105, in
  100 functions), the tether's and `hero_damage`, matched into the Gladiator PAL image
  `gladpal-dreadzone-4` (DreadZone Station, `$gp` 0x2259FF) by `match-function.py`'s
  masking over each whole function (a sweep script kept outside the repository). Where a
  whole function didn't match, its first 12 instructions placed it, and Ghidra's
  decompiles of the two were compared. PAL names are in `re/SCES-53285/symbols.toml`.
- **The same apart from 50 Hz:** each function differs from DL's only in moved calls and
  globals, and in frame counts and per-frame constants converted from 60 to 50 frames a
  second (counts cut to about 5/6, per-frame rates scaled up by 6/5), as RE.8 found for
  the hero (`hero-controller.md`, "Frames and regions"). For co-op:
  - the wait before a respawn is 300 frames (DL's 360) in `hero_death_done`; a hit's
    safe time 300 (360) in `hero_damage`; hero+0x422 at a respawn 100 (120);
  - the team moves last 1500 frames (1800), and the two clicks must come within 25
    frames (30), in `hero_update_timers`;
  - the tether counts 50 frames a second ("The tether");
  - multiplayer's respawn time multiplies the settings' seconds by 50 (60).
  - `hero_death_done` and `hero_respawn` also gain a check that runs only in
    multiplayer.
- **Unchanged:** the co-op tests and their order, what is shared, the respawn search,
  split screen and its layouts, menus and pads, the ammo pickups' co-op factor, the
  enemies' scaling, and the data the rules read: the health-level table, the view
  layouts, the difficulty table, the planet and mission records and the bots' commands
  are byte for byte DL's (at addresses 0x40 to 0x1F8 higher).
- **Not matched:** 0x61D238 (30 instructions, the vendor's skin choice) has no unique
  match. Nothing was checked in play on PAL.

## Not read yet / next steps

1. **Unread values:** HUD bit 0x08; the
   four-entry screen of exclusive choices; the three per-player settings beyond the
   camera; items 11, 12, 13 and 16; 0x21E904; player 2's moby marks (+0x74, flag 0x8000);
   whether co-op ever sets 0x2400A8; the difficulty table's other columns; the planet
   flag 4 set on arrival; what made the kills that gave no gun XP.

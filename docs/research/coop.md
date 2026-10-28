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
- **Labels** as in `hero-controller.md`: **[code]** read in the game's code,
  **[verified]** checked in a savestate or trace, **[inferred]** from how a value is used.
- Addresses are DreadZone Station's build. Level code moves between levels (`engine-notes.md`,
  "Code layout and overlays"); globals below 0x300000 don't.

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
- **Difficulty** isn't raised for two players; enemies' hit points can be scaled by a
  co-op factor, 1.0 so far.
- **Split screen** stacks the two views (staggered halves in widescreen), draws at 30
  frames a second over the 60 Hz simulation, and goes full screen for cutscenes,
  transports and menus. Either pad drives a menu, and the pause menu stops both.
- **Player 2 replaces the battle bots,** which co-op removes with their vendor, and both
  players clicking L3 (or R3) together with a full team meter gives a 30 s power-up.

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
  - Where the level places player 2 at load isn't found yet.
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
- **Level progress is shared with single player:** `main_loop` marks the level visited in
  a 0x304-byte record per level at 0x1F8AF0, indexed by `(level − 1) % 20`, so level 21
  writes level 1's record.
- **The death counters** (0x21EBE4, 0x21DA74 and one per level at 0x171F70 + 4 ×
  `((level − 1) % 20)`) count only in single player (`hero_death_done`, 0x67A4F8); co-op
  counts each player's deaths in their statistics instead.
- **On the disc** levels 21–35 have level files of their own but use level − 20's audio
  and scenes (`disc-layout.md`, "Level data (DL NTSC-U and Gladiator PAL)").

## Death, respawn and failure [code]

- **Single player:** a death (`hero_death_done`, 0x67A4F8) counts itself, sets the
  report 0x368BE8 to 1, and `main_loop` restarts the level (0x5299F8). That is DL's
  reload after a death (`hero-controller.md`, "Getting hit and dying").
- **In co-op a death doesn't end the level.** The same function sets health to 0, counts
  the death in that player's statistics (0x20B490 + 0x2A0 × player) instead of the
  single-player counters, puts **360 frames (6 s)** in hero+0x3B2 and enters **state 153
  (0x99)**, dead and waiting. It also ends the hero's damage power-up (hero+0x2FB0 back
  to 1, its timer +0x40A to 0) and the other power-up (+0x2FB4, timer +0x408).
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
- **Enemies that test both heroes' health** (for example a boss at 0x4E2FD8) keep
  attacking while either player is alive; they don't fail the mission themselves.

## What the players share [code; checked in `dlntsc-dreadzone-coop.p2s` where marked]

| Shared by both | Per player |
|---|---|
| Bolts: one purse, 0x171B40. A bolt pickup (0x46AE78) adds its value with no player involved; the vendor and mission rewards use the same purse. | Health (hero+0x2E20). |
| XP: one total, 0x171B48, added by every kill. | The item in hand (hero+0x2318), saved each frame to 0x220810 + 4 × player and given back at the next level. |
| Maximum health: one level, 0x171B50 (10 in a new game), worked out from the XP (table 0x3B6458); when it rises both heroes are healed and both get the effect (0x68F398, 0x3E0AB0). | Quick-select: 3 slots each at 0x20C690 + 0xC × player (libdl's `WEAPON_EQUIPSLOT`) [verified: player 1's were 3, 2, 0 and player 2's 2, 3, 0]. |
| **One gadget box** (0x1D3F90): in the campaign every hero's box pointer (hero+0x26E4) is set to it (0x6909C8) [verified: both heroes], so guns, ammo, weapon XP, levels and mods are shared. Multiplayer levels give each player a box of their own, 0x12B0 bytes apart. | The skin (`skin_p1`, `skin_p2`). |
| The vendor: one shop and one purse; a weapon bought goes into the shared box and is equipped for the player who bought it (`vendor_buy`, 0x632CA0). | Statistics: 0x2A0 bytes per player from about 0x20B480 (ammo picked up, kills by gadget, mod and enemy class, deaths), and a kill count at 0x35944C + 4 × player. The statistics screen shows player 1's plus player 2's (0x617FE8). |
| Power-ups: the damage power-up (×4 damage, hero+0x2FB0, for 1800 frames, 30 s; 0x400100) and a 180-frame one (+0x41C; 0x3FFC88) are given to both heroes in co-op. | The camera setting and three other settings ("Starting co-op"). |

- **A new co-op box** (0x6909C8) gets the same starting kit as single player (the wrench,
  Dual Vipers, Magma Cannon, items 0xC and 0xE, an alpha mod) plus item 7, libdl's B6
  Obliterator, which only co-op gives.
- **Ammo pickups go to the nearest living player** within his pickup radius (hero+0x2E24,
  height +0x2E28) whose box isn't full for that gun (0x3D7900, 0x3D81E8). It goes into
  the shared box; the collector's statistics and HUD show it.
- **Weapon XP from a kill** (0x590990) goes to the gun in the hand of the player who
  fired it. As read, the loop runs from the killer's index to the last player, so with
  one shared box a gun kill by player 1 would add the XP twice and one by player 2 once.
  Check this in a trace before relying on it.
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
  - **Ammo pickups give 2.4 times as much** (0x3D7900), as two players draw on one box.
  - **Enemy hit points** are multiplied by the float 0x2204C0 at the level's moby set-up
    (0x64ED20, from 0x578940), only in co-op. It is 1.0 in level 1's image and in level
    21's (`dlntsc-dreadzone-coop.p2s`), and nothing in level 1's code writes it, so it
    may come from level data. Read it in a co-op level with enemies.
  - Nothing else read so far scales with the second player: no change was found to
    enemy damage, enemy counts or the difficulty.

## Co-op's own moves [code]

- **A team meter:** each hero's byte hero+0x2FB8 is set once the XP earned since the last
  reset (0x171B54 − 0x171B70) reaches one health level's span (0x171B78), with the
  progress in +0x2FBC (0x68F238). In co-op only (`hero_update_timers`, 0x684EF0), with
  the meter full:
  - **both players clicking L3 within 30 frames of each other** (`pad_pressed_within`
    on both pads) gives the hero the damage power-up, ×4 for 1800 frames (30 s);
  - **both clicking R3** sets +0x2FB4 to 3 for 1800 frames (timer +0x408); what that does
    isn't read yet.
  - Then that hero's meter resets (0x68F2D8 clears his +0x2FB8 and +0x2FBC and moves the
    shared XP baseline 0x171B70). Each hero makes the check in his own update, and both
    run in the same frame, so both should get the power-up [inferred].

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
  again. In our savestate, whose port 2 is empty, player 2 is on 0x1EEBC0.

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

## Not read yet / next steps

1. **Play the co-op savestate.** `dlntsc-dreadzone-coop.p2s` is stuck in the pause menu.
   Give `boot.sh` a second keyboard pad on port 2 (`pine-probe headless-config`), load
   it, and save a state in normal play; then trace one player's death (the 360-frame
   wait, the respawn spot near the partner), both players' deaths (the restart), an
   ammo pickup (×2.4) and one gun kill by each player (the weapon XP counted twice?).
2. **Where player 2 is placed** when the level loads.
3. **The enemy hit-point factor** 0x2204C0 in a co-op level with enemies, and what writes
   it.
4. **Unread values:** what +0x2FB4 = 3 does (the R3 power-up); HUD bit 0x08; the
   four-entry screen of exclusive choices; the three per-player settings beyond the
   camera; items 11, 12, 13 and 16; 0x21E904; player 2's moby marks (+0x74, flag 0x8000);
   whether co-op ever sets 0x2400A8; the difficulty table's other columns.
5. **Gladiator PAL** (SCES-53285): check that its co-op is the same (RE.10 names both).


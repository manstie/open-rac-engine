# Reverse-engineering setup

How the reverse-engineering tools are installed and configured (PLAN.md task RE.1). The
layout below is on the owner's Linux machine. Follow the same steps on another machine.

Nothing here needs sudo. Everything lives under `~/.local`.

## What is installed

| Tool | Version | Location | Launcher |
|---|---|---|---|
| Eclipse Temurin JDK | 21.0.12.1 | `~/.local/opt/jdk-21.0.12.1+1/` | Used only by the Ghidra launchers (not on `PATH`) |
| Ghidra | 12.1.4 | `~/.local/opt/ghidra_12.1.4_PUBLIC/` | `ghidra` (GUI), `ghidra-headless` (`analyzeHeadless`) |
| ghidra-emotionengine-reloaded | v2.1.38 (build for Ghidra 12.1.4) | `…/ghidra_12.1.4_PUBLIC/Ghidra/Extensions/ghidra-emotionengine-reloaded/` | Loaded automatically |
| PCSX2 | 2.8.2 (official AppImage, extracted) | `~/.local/opt/pcsx2-v2.8.2/squashfs-root/` | `pcsx2` |
| xdotool | system package | `/usr/bin/xdotool` | Drives PCSX2's menus during replays |
| Xvfb | 21.1.12 (Ubuntu `xvfb` package, installed by the owner on 2026-10-12) | `/usr/bin/Xvfb` | Virtual display for replays and headless boots |

- **Launchers:** they are small shell scripts in `~/.local/bin`. On Ubuntu, `~/.profile` adds
  that folder to `PATH` at login. Until your next login, call them by their full path.
- **App menu:** `~/.local/share/applications/` also has entries for PCSX2 and Ghidra.
- **Sources:**

  | Tool | Where it came from |
  |---|---|
  | Ghidra | `NationalSecurityAgency/ghidra` GitHub releases |
  | PS2 extension | `chaoticgd/ghidra-emotionengine-reloaded` GitHub releases |
  | PCSX2 | `PCSX2/pcsx2` GitHub releases |
  | JDK | The Adoptium API |

- **Verification:** every download was checked against its published SHA-256 before it was
  unpacked.

### Two install quirks

1. **Ghidra needs a `java` on `PATH` or a `JAVA_HOME` to bootstrap.** That is true even with
   `JAVA_HOME_OVERRIDE` set in `support/launch.properties`, because the launcher needs Java
   just to read that setting. The wrapper scripts export `JAVA_HOME` to the private JDK,
   which keeps Java off the global `PATH`.
2. **The PCSX2 AppImage needs `libfuse.so.2`**, which Ubuntu 24.04 doesn't ship by default.
   Instead of installing FUSE with sudo, the AppImage was unpacked with `--appimage-extract`,
   and the launcher runs `squashfs-root/AppRun`.

## PCSX2 configuration

- **Config and data folder:** `~/.config/PCSX2/`.
  - The settings file is `inis/PCSX2.ini`.
  - It was created without opening a window by running `pcsx2 -testconfig`, which writes
    the full default config and exits.
- **Changes from the defaults:**

  | Section | Key | Value | Why |
  |---|---|---|---|
  | `[EmuCore]` | `EnablePINE` | `true` | Lets our tools read and write emulated memory (slot 28011) |
  | `[GameList]` | `RecursivePaths` | `~/GitHub/open-rac-engine/source` (absolute path) | The game discs show up in the game list |

- **The first-run setup wizard is left on.** It asks you to choose the BIOS and map a
  controller. Everything else uses PCSX2's defaults.
- **BIOS:** dumps live in **`~/.config/PCSX2/bios/`**. Since 2026-10-11 the owner's
  EUR (SCPH-77004, the selected one), USA (SCPH-90001) and JAP (SCPH-77000) dumps are
  installed.
  - A dump usually includes a 4 MB `.bin` plus optional `.rom1`, `.rom2`, `.erom`, `.nvm`
    and `.mec` files. Keep them together with the same base name.
  - The BIOS must come from your own console; see PCSX2's
    [BIOS guide](https://pcsx2.net/docs/setup/bios).
  - A BIOS from any region runs all of our discs.
  - **Never copy a BIOS into this repository.**
- **Savestates** go in `~/.config/PCSX2/sstates/`, named `<serial> (<crc>).<slot>.p2s`. They
  are zip archives whose members are Zstandard-compressed (zip method 93), which Python's
  `zipfile` and Ghidra cannot read. `pine-probe savestate-list`, `savestate-extract` and
  `savestate-peek` read them directly.
- **Default hotkeys:**

  | Key | Action |
  |---|---|
  | F1 | Save state to the current slot |
  | F2 | Next slot (Shift+F2 for the previous slot) |
  | F3 | Load state |
  | F8 | Screenshot |
  | Space | Pause |
  | Esc | Pause menu |
  | Shift+R | Toggle input-recording mode |

- **PINE** listens on the Unix socket `$XDG_RUNTIME_DIR/pcsx2.sock` while a game is running.
  `tools/pine-probe` (RE.5) will connect to it with the `pine-ipc` crate.
- The debugger is documented at <https://pcsx2.net/docs/advanced/debugger>.

## Ghidra usage

**Headless analysis** is what Claude sessions use. For example:

```sh
openrac-tool unpack-elf --iso "<UYA disc>.iso" --out cache/elf/SCUS_973.53.elf
ghidra-headless cache/ghidra/elf uyantsc-boot -import cache/elf/SCUS_973.53.elf
```

- **Unpacked boot programs** (verified 2026-10-15 on UYA NTSC-U; about a minute with
  analysis): `unpack-elf` turns the packed boot executable of R&C3, UYA, DL or Gladiator
  into a plain ELF (`--file I5BOOTN.ELF` for the network start-up). Ghidra takes `$gp`
  from its `.reginfo`, keeps its section names and finds the functions from the entry. It
  holds the engine core and the title screen only: a level's code still needs a savestate
  (below) until P2.6. R&C1's and GC's boot ELFs import as they are.
- **Processor:** the PS2 extension detects R5900 ELFs automatically as language
  `r5900:LE:32:default:default`.
- **Harmless warning:** "Failed to add language defined memory block … rom0" just means the
  extension's BIOS ROM region overlapped a block the importer had already created.
- **Where projects go:** keep Ghidra projects and anything exported from them (function
  lists, decompiled code) in `/cache/ghidra/` or outside the repo. They are derived from the
  game, so they are never committed.
- **Savestates:** the extension advertises a savestate importer, but headless import of a
  `.p2s` fails with "No load spec found", even after re-packing it with Deflate. What works
  (verified 2026-10-11, 7 s) is importing the raw EE memory, which holds the unpacked game
  plus the current level's overlay:

  ```sh
  pine-probe savestate-extract "<state>.p2s" eeMemory.bin -o cache/ghidra/<name>.ee.bin
  ghidra-headless cache/ghidra <project> -import cache/ghidra/<name>.ee.bin \
    -processor r5900:LE:32:default -loader BinaryLoader -loader-baseAddr 0 -noanalysis
  ```

  No functions are defined after a raw import, so on its own, analysis has nowhere to
  start. `tools/ghidra/import-savestate.sh` does the whole job; see the next section.
- **The GUI** (`ghidra`) is optional. It shows a user agreement on first launch.

## Static analysis of a savestate (RE.3)

How the hero code was found and read on 2026-10-12. The scripts are in `tools/ghidra/`.

```sh
tools/ghidra/import-savestate.sh "<state>.p2s" <name> [re/<serial>/symbols.toml]
ghidra-headless cache/ghidra/proj <name> -process <name>.ee.bin -noanalysis -readOnly \
  -scriptPath tools/ghidra -postScript DecompileAt.java cache/ghidra/<out>.c <address hex> ...
```

- **`import-savestate.sh`** (about 3 minutes) does these steps:
  1. Extracts `eeMemory.bin` to `cache/ghidra/ee/<name>.ee.bin`.
  2. Reads `$gp` with `pine-probe savestate-regs`.
  3. Imports the memory as a raw R5900 binary into `cache/ghidra/proj/<name>`.
  4. Runs `SeedFunctions.java` before auto-analysis, which:
     - sets `$gp` everywhere, so gp-relative globals resolve;
     - finds the code regions: 16 KiB blocks with at least three `jr ra` and three
       stack-frame prologues;
     - creates functions at every prologue that follows a return, and at every target
       called twice or more.
  5. If a `symbols.toml` is given, `ApplySymbols.java` names its functions and data.
- **`DecompileAt.java`** decompiles the functions containing the given addresses into one
  file, with each function's callers. **That output is decompiled game code: keep it in
  `cache/`, never in the repository.** What we learn goes into `docs/research/` in our own
  words and into `re/<serial>/symbols.toml`.
- **`RefsTo.java`** (same arguments: an output file, then addresses) lists every
  instruction that reads or writes each address, with the function it is in. Hero fields
  are gp-relative or based on the hero's address, so the import's `$gp` must be set.
  - That is how the wall-contact window and the ledge block were found on 2026-10-12.
  - The listing names game addresses only, but still keep it in `cache/`.
- **`ListingAt.java`** (an output file, then start and end address pairs) writes the
  disassembly of each range. Use it where the decompiler loses a value: arguments set
  just before a call through the hero's vtable, or VU0 macro code. It is disassembled game
  code, so it stays in `cache/` too.
  - DL's state tables are jump tables of 154 code pointers, indexed by state: entry,
    update and transitions (`re/SCUS-97465/symbols.toml`). Reading a table entry gives the
    case for one state where the decompiler only shows its address.
- **Finding things:**
  - The fastest way in is a value from a trace. For example, find the code that writes a
    measured constant (search the image for the `lui`/`ori` pair that loads it), or the
    code that reads or writes a field of a known struct.
  - Compare the two R&C3 PAL images (or a PAL and an NTSC image) to separate code from
    data.
- **Gotchas:**
  - Code addresses are per level. Each level overlay holds its own build of the game
    program, so an address found on Veldin is wrong anywhere else. The title screen's
    image has no hero code at all.
  - R&C3/UYA address the hero as a static global at 0x1A4BE0, so the decompiler shows
    its fields as `DAT_001a....` globals rather than struct offsets.
  - Gladiator/DL pass the hero as a pointer argument.
  - Gladiator's `$gp` is odd (0x2259FF) on purpose; its gp-relative offsets are odd too.
  - Ghidra misses switch cases on some jump tables (Gladiator's `hero_change_state`).
    Read the table from memory: one word per case.
  - Float constants usually load as `lui` plus `ori`. A bare `lui` match on the upper
    half also hits unrelated values, so check the lower half.
  - Two headless runs can't open the same Ghidra project at once. Give each image its own
    project, as the script does.
  - `pgrep -f` or `pkill -f` with a pattern from the PCSX2 command line also matches the
    shell running it. Kill PCSX2 by PID.

### Extracting a model (P3.7)

**From the disc** (the engine's own path): `openrac-tool hero` reads the hero's skeleton
and every animation straight from a UYA disc image, through the level's core
(`docs/research/disc-layout.md`, "Level data"). It takes 0.4 s.

```sh
cargo run -p rac_tool -- hero --level 1 --sequences \
  --iso "source/Ratchet & Clank 3/Ratchet & Clank - Up Your Arsenal (USA) (En,Fr,Es).iso" \
  --glb cache/models/uyantsc-disc-veldin-hero.glb \
  --check-ee cache/ghidra/ee/uyantsc-veldin.ee.bin
```

`--check-ee` compares the result with the hero in a memory image of the same level; on
Veldin every joint and keyframe matches. His mesh isn't in the level, so the `.glb` has
the skeleton and animations only.

**From memory**, for anything the disc path can't read yet:

`openrac-tool moby` reads a moby class out of an EE image made by `import-savestate.sh`
and describes it. With `--glb` it also writes the class as a binary glTF: the skeleton
(joints `j000`…), the high-detail mesh in gold with flat shading, and every sequence as
an animation. Blender and any glTF viewer open it. The format is in
`docs/research/moby-format.md`.

```sh
cargo run -p rac_tool -- moby --ee cache/ghidra/ee/uyantsc-veldin.ee.bin \
  --moby 1d36d00 --sequences --glb cache/models/uyantsc-veldin-hero.glb
```

- The output is derived from the disc, so it goes in `cache/models/`, never the
  repository. Regenerate it from the savestate whenever it's needed.
- `--moby` takes a moby's address and reads its class; `--class` takes a class directly.
  On the Veldin image, UYA's hero is 0x1D36D00, the wrench 0x1D63E00, Clank on his back
  0x1D66100 and the heli-pack (Clank's arms and rotors) 0x1D66000.
- The mesh is complete, with its own skin weights (`moby-format.md`, "Mesh"), since
  2026-10-13. The game itself shows the wrench's mesh from the armor WAD, which the
  image's wrench class carries once loaded.

## NTSC savestates without a capture session (RE.8)

The NTSC discs can be booted headlessly and played with xdotool, so savestates of a level
don't need the owner. Recordings of moves still do.

```sh
tools/pine-probe/boot.sh "<disc>.iso" <name>     # leave it running
export DISPLAY=:98
W=$(xdotool search --onlyvisible --classname . | tail -n 1)   # the render window
xdotool windowfocus --sync "$W"
xdotool keydown Return; sleep 0.15; xdotool keyup Return      # press Start
xdotool key F8    # screenshot -> cache/pcsx2/boot/<name>/xdg/PCSX2/snaps/
xdotool key F1    # savestate  -> cache/pcsx2/boot/<name>/xdg/PCSX2/sstates/
```

- **What `boot.sh` does:** it starts a private PCSX2 on Xvfb `:98` (PINE slot 28013), with
  the owner's emulation settings, copies of their memory cards, and controller 1 bound to
  the keyboard (`pine-probe headless-config --keyboard-pad`):

  | Pad | Key | Pad | Key |
  |---|---|---|---|
  | D-pad | arrow keys | Start, Select | Return, Backspace |
  | Cross, Circle | X, C | Square, Triangle | S, T |
  | L1, R1 | Q, E | L2, R2 | Z, V |
  | Left stick | I, J, K, L | Right stick | R, F, D, G (up, down, left, right) |
  | L3, R3 | comma, slash | | |

- **Controller 2** (`PAD2=1 boot.sh …`, `--keyboard-pad2`), for Deadlocked's co-op
  (`research/coop.md`). Without it port 2 is empty, and a co-op level's pause menu
  wouldn't close:

  | Pad | Key | Pad | Key |
  |---|---|---|---|
  | D-pad | Home, End, Delete, Page Down (`Next` in xdotool) | Start, Select | P, O |
  | Cross, Circle | N, M | Square, Triangle | B, Y |
  | L1, R1 | 1, 2 | L2, R2 | 3, 4 |
  | L3, R3 | 5, 6 | | |
  | Left stick | U, H, A, W (up, down, left, right) | Right stick | 7, 8, 9, 0 |

- **Hold each key for about 150 ms** (`keydown`, sleep, `keyup`). A bare `xdotool key`
  can be too short for the game to see.
- **Look before every step:** take a screenshot (F8) and look at it.
- **Paths used on 2026-10-12.** Both took about two minutes, and both games accept a new
  game without creating a save:
  - **UYA (SCUS-97353):**
    1. Wait about 60 s, then press Start at "Press START".
    2. Down to New Game, Cross.
    3. Triangle (don't create a save), then Cross (continue anyway).
    4. Start skips the opening video.
    5. Veldin loads in about 15 s.
  - **Deadlocked (SCUS-97465):**
    1. Start at the title.
    2. Single Player, then New Game.
    3. Cross on No (create a save?), then Right and Cross (continue anyway).
    4. Cross for the default difficulty.
    5. Start skips the intro.
    6. DreadZone Station loads, and an in-engine cutscene plays.
    7. **The opening tutorial** then ignores the movement buttons until each step is done:
       look left and right with the right stick, fire (R1), then jump and double jump.
       After that the charge (L2 twice) works.
    - DL's default controls keep the hero facing the camera: the left stick strafes, and
      the right stick turns him.
    - The charge boots (gadget 0x11) aren't owned yet. For research,
      `pine-probe poke --slot 28013 0x1D4E44 0` and the same for 0x1D4E45 give them (the
      gadget box at 0x1D3F90 + 0xA30 + 0x44 × 0x11). Only ever poke the private boot
      instance.
- **Saving:** a savestate taken during a level's opening cutscene already holds that
  level's overlay. Copy it out of the boot folder straight away, before the next boot
  overwrites it.
  - Copy it to `source/<game>/<release>-<level>-<what>.p2s`, which is where files worth
    keeping live.
  - Make a working copy in `cache/pcsx2/ntsc-states/` if you need one.
- **Coming back:** `boot.sh` keeps `cache/pcsx2/boot/<name>/` between runs. After
  booting, F3 loads savestate slot 1, so a saved gameplay state skips the menus.

### Going straight to a level

Found 2026-10-13 in UYA's and DL's code (`re/<serial>/symbols.toml`: `level_next`,
`level_change_request`, `level_current`, `game_mode`).
- **How the games change level:** each game's main loop checks a request flag every
  frame. When it is set, the loop saves the level's state and loads the level whose
  number is in `level_next`.
- **So two writes send a running game anywhere:**

  | Release | `level_next` | `level_change_request` | `level_current` | `game_mode` |
  |---|---|---|---|---|
  | UYA NTSC-U (SCUS-97353) | 0x1DFBC4 | 0x1D6674 | 0x1D545C | 0x1D6694 |
  | DL NTSC-U (SCUS-97465) | 0x222B80 | 0x21E80C | 0x21DE10 | 0x21DDB4 |

- **`tools/pine-probe/warp.sh <level> --from-state 1`** does it on a `boot.sh` instance:
  1. it loads savestate slot 1;
  2. it writes the level number, then the request;
  3. it waits until that level is in normal play (game mode 0) and the hero stands
     still.
  - Then look around and take savestates with F1, or `pine-probe save-state <n>`.
  - pine-probe's `peek`, `poke --word`, `load-state` and `save-state` commands do the
    single steps.
- **Each level has its own build of the game, and some globals move with it.** The hero
  struct (0x1A4BC0) and the addresses in this section stayed put in every level tried,
  but the rail list is at 0x1DFD10 on Veldin and 0x1DE690 on Florana. Check an address
  in the level's own savestate before trusting it, or read the data from the disc (for
  rails, `openrac-tool rails`).
- **UYA: warp from Veldin's savestate (slot 1 in the `uya` boot), not from a planet you
  warped to.**
  - Arriving by the flight in leaves the game in ship mode. The next change then goes
    through space (level 0, game mode 6) and waits there for a destination the ship
    menu never chose.
  - From the Veldin state, every level number that is a place loaded (table below).
- **What you arrive with:** a new game's items, so the heli-pack and thruster-pack and
  little else. Items are bytes at 0x142CA0 + the item number; write 1 to own one.
  - 0x142CAE (item 0x0E) is the flag the grind code checks, so probably the grind boots.
  - RCMemory's PAL list matches these addresses: 0x0B Hypershot, 0x0D gravity boots,
    0x1D charge boots.
- **Arrival:** the ship flies in and lands, and the hero jumps out. A first visit may
  play a cutscene; Start skips it.
- **UYA's levels**, numbered as the game does. Each was loaded this way from Veldin on
  2026-10-13, and screenshots of the arrivals are in `cache/pcsx2/levels/`. Names come
  from the game's transition table and the screenshots:

  | Level | Planet | On arrival |
  |---|---|---|
  | 1 | Veldin | A new game starts here |
  | 2 | Florana | Plays |
  | 3 | Starship Phoenix | Plays (the ship flies in from space) |
  | 4 | Marcadia | Plays |
  | 5 | Daxx | Plays |
  | 6 | Starship Phoenix, under attack | Plays |
  | 7 | Annihilation Nation | Plays |
  | 8 | Aquatos | Plays |
  | 9 | Tyhrranosis | Plays |
  | 10 | Zeldrin Starport | Plays |
  | 11 | Obani Gemini | Plays |
  | 12 | Blackwater City | Battle Missions menu (named on screen) |
  | 13 | Holostar Studios | Plays |
  | 14 | Koros | Plays |
  | 15 | none | Stays in the loading mode |
  | 16 | Metropolis | Plays |
  | 17 | Crash Site | Plays |
  | 18 | Aridia | Battle Missions menu ("Tunnels of Outpost X12") |
  | 19 | Qwark's Hideout | Plays; `game_mode` reads 5101 here, so `warp.sh` times out |
  | 20 | Launch Site | Battle Missions menu ("Defeat Dr. Nefarious") |
  | 21 | Obani Draco | Plays |
  | 22 | Command Center | Plays |
  | 23 | Holostar Studios, as Clank | A cutscene |
  | 24 | Insomniac Museum | Plays |
  | 25 | none | Stays in the loading mode |
  | 26 | Metropolis, Ranger missions | Battle Missions menu ("Urban Storm") |
  | 27 | Aquatos, as Clank | Plays |
  | 28 | Aquatos sewers | Plays, swimming (`uyantsc-sewers-swim.p2s`) |
  | 29 | Tyhrranosis, Ranger missions | Battle Missions menu ("Island Strike") |
  | 30–36 | Qwark vid-comics, entered from the Phoenix (3) or the museum (24) | Not tried |

  - **The names** are the usual numbering. They fit the game's transition table: the
    Clank and Ranger versions lead to their planets, and the vid-comics go to and from
    the Phoenix. Only Blackwater City (12) is named on screen. The others come from the
    look of the arrivals and the missions' names.
  - **On a Battle Missions menu,** X starts the mission on that planet. Triangle opens the
    galactic map.
  - Levels from 37 up and level 0 weren't tried. Level 0 is where a change waits in space
    mode.
- **DL's levels** (tried 2026-10-13 from `dlntsc-dreadzone-open.p2s`, slot 1 in the `dl`
  boot):
  - **Each planet arrives at its "Challenges" menu, which names it.** Screenshots are in
    `cache/pcsx2/levels-dl/`.
  - **A new game has no challenges there, so the planets can't be played yet.** Triangle
    goes to the Planet Select screen, where everything past DreadZone Station is
    locked. Unlocking challenges would need their progress flags, which aren't found
    yet.
  - `warp.sh` waits for game mode 0, so on DL it times out at those menus. The level has
    loaded anyway.

  | Level | Planet |
  |---|---|
  | 1 | DreadZone Station (playable; new games start here) |
  | 2 | Catacrom Four |
  | 3, 9, 12, 16 | Stay on DreadZone's look; probably the tournaments held in its arena |
  | 4 | Sarathos |
  | 5 | Kronos |
  | 6 | Shaar |
  | 7 | The Valix Belt |
  | 8 | Orxon |
  | 10 | Torval |
  | 11 | A planet whose fly-in shows no name; libdl's map list makes it Stygia |
  | 13 | Maraxus |
  | 14 | Ghost Station |
  | 15 | DreadZone Station Interior |

  - **libdl's multiplayer map numbers are these plus 40.** Its `MAP_ID_TORVAL` is 50 and
    `MAP_ID_MARAXUS` 53, for example.

### Making traces with scripted play

With a gameplay savestate loaded, a trace of scripted moves takes a minute:

```sh
target/release/pine-probe capture --layout re/SCUS-97353/hero.toml --slot 28013 \
  --seconds 60 --out cache/pcsx2/traces/<name>.trace &
# then drive the keys with xdotool: keydown, sleep, keyup (see the table above)
```

- **Speed:** with the software renderer on Xvfb the game ran at about 28 frames per real
  second. The capture still gets every frame, and xdotool timings simply stretch in game
  time.
- **The layout records the pad:** UYA's `hero.toml` has a `pad` region, so the trace
  shows what the game saw on each frame.
- **Moves on UYA:**
  - X jumps; R1 crouches; Square swings the wrench.
  - Run, then R1 and X: long jump. R1, then X: heli jump.
  - Hold X after a double jump: glide.
  - L2 or R2 with left, right or back, then X: flips.
  - R1, then left, right or back pushed at about the same time as X: the crouch flip.
  - L1 is first-person look.
- **Part tilt from the keyboard** (2026-10-17): start `boot.sh` with `AXIS_SCALE=0.5`
  (PCSX2's `AxisScale` for both of controller 1's sticks) and every stick key gives the
  byte 191 or 64 instead of 255 or 0, which the pad layer turns into 0.2105 or 0.1974.
  A scale stays for the whole boot, so each magnitude needs its own boot.
- **Capture in gameplay only:** UYA's frame counter runs faster on the title screen.
- **Writes and the capture can't overlap.** PCSX2's PINE server serves one client at a
  time, so a `poke` sent while `capture` is running waits until the capture ends. To
  place the hero for a trace, write first and start the capture straight after, as
  `uyantsc-florana-zip.sh` does.
- **Write a position in one request:** `pine-probe poke --f32 0x1A4C60 x y z`. Written
  one coordinate at a time, the game ran between the writes and snapped the hero back.
- **Mapping the ground by dropping the hero:** write his position high over a point,
  wait a moment, and read his height above the ground (0x1A4F1C); his height less that
  is the ground there. A grid of drops 1-4 u apart maps an area in a minute or two. It
  found a flat strip north of Veldin's start (y 118, x 342-368, at 77.98) on 2026-10-15.
- **Don't grind on UYA, even on a rail of your own.** A rail written into Veldin's empty
  rail list is caught, and PCSX2 aborts two frames later (`hero-controller.md`,
  "Grinding").
- **Square is S** on the keyboard pad (`tools/pine-probe/src/pcsx2_config.rs` has the
  whole list).
- **Old traces hold more than their layout's fields.** A trace stores each region's
  bytes whole, so a field inside a region can be read later: copy the layout, add the
  field, and `pine-probe export` the trace with the copy (the regions must stay the
  same). The hero regions cover most of the hero (UYA 0x1A4BC0 + 0x2940, DL hero +
  0x3000). That is how DL's flips and the wrench's draw were timed on 2026-10-24 without
  new captures.
- **Keep the script and the trace together.** Write the key sequence as a script, not
  ad-hoc commands, and save it as `source/<game>/<trace name>.sh`.
  - Give it a header saying which savestate and boot it ran from.
  - Copy the trace beside it. xdotool timing isn't reproducible, so the trace is the only
    exact record of what happened.

## Captures from the owner

### What exists (2026-10-12)

The owner captured the **PAL** releases first. Everything below is game-derived, so it
stays out of the repository.

| Release | Where | What |
|---|---|---|
| R&C3 PAL (SCES-52456), Veldin | `~/.config/PCSX2/sstates/SCES-52456 (17125698).0{1..5}.p2s` | 1: open ground with enemies ahead. 2: next to crates. 3: hanging from a crate's ledge (state 24). 4: at a cliff edge. 5: enemy in view, weapon out. |
| R&C3 PAL | `source/Ratchet & Clank 3/save-state-{5,6}-inputs` plus `_SaveState.p2s` | Input recordings of free play from the cliff edge: 1,823 and 1,383 frames of running, jumps, double jumps, crouch jumps, wrench combos and firing. |
| Gladiator PAL (SCES-53285), DreadZone Station | `~/.config/PCSX2/sstates/SCES-53285 (D697D204).0{1..5}.p2s` | 1: open floor. 2: next to a wall. 3: hanging from a ledge (state 24). 4 and 5: another open area. |
| Gladiator PAL | `source/Ratchet Gladiator/save-state-{4,5}-inputs` plus `_SaveState.p2s` | Input recordings of free play: 2,512 and 2,329 frames. |
| UYA NTSC-U (SCUS-97353), Veldin | `source/Ratchet & Clank 3/uyantsc-veldin-intro.p2s` | Not from the owner: a headless new game (`tools/pine-probe/boot.sh`), saved during Veldin's opening cutscene. Used for static analysis; no recording. |
| Deadlocked NTSC-U (SCUS-97465), DreadZone Station | `source/Ratchet Gladiator/dlntsc-dreadzone-intro.p2s` | The same, saved during DreadZone Station's opening cutscene. |
| UYA NTSC-U, Veldin gameplay | `source/Ratchet & Clank 3/uyantsc-veldin-play.p2s` and `uyantsc-veldin-play-2.p2s` | Headless, at the start of Veldin's gameplay (enemies ahead). The second was taken 76 frames later, standing still, to find the frame counter. The first is the starting point for the traces below. |
| UYA NTSC-U traces | `source/Ratchet & Clank 3/uyantsc-veldin-moves-{1,2,3}.trace`, each with the `.sh` input script that drove it | Self-made with scripted keyboard play: 1, jumps (tapped and held), double jump, glide, heli jump, walk and stop, run jump and long jump; 2, wrench combo, jump attack and L1 look; 3, strafe walk, side and back flips. 2,069, 1,195 and 1,075 frames. Each holds the whole hero block, so later questions can be answered from them: on 2026-10-15, 3 verified strafing through the export layout `uyantsc-veldin-strafe.toml` (`hero.toml` plus the lock-on block's fields). |
| DL NTSC-U, DreadZone Station gameplay | `source/Ratchet Gladiator/dlntsc-dreadzone-play.p2s`, `-boots.p2s`, `-open.p2s` | Headless, 2026-10-12. `play`: just after the opening cutscene. `boots`: after the tutorial's look and fire steps, with the charge boots poked in. `open`: after its jump step too, facing the longest open floor; the start for the charge takes. |
| DL NTSC-U charge traces | `source/Ratchet Gladiator/dlntsc-dreadzone-charge-1.trace` (script `-charge-1.sh`) and `-charge-3-{1..5}.trace` (script `-charge-3.sh <take>`) | Self-made, 2026-10-12, layout `re/SCUS-97465/hero.toml`. Take 1: three charges that bounce off a nearby block. Takes 3-1 to 3-5: a charge from standing, a steered one, a missed double tap (16 frames apart), single taps, and a full 61-frame charge ending in the skid. |
| DL NTSC-U, DreadZone Station in third person | `source/Ratchet Gladiator/dlntsc-dreadzone-third.p2s` | Made 2026-10-15 in the `dl` boot (slot 2 there) from `dlntsc-dreadzone-open.p2s` (slot 1) by writing 0 to the camera option word at 0x1711D0 (`pine-probe poke --word`), so the hero's camera type is third person and the stick turns him as in UYA. Only ever poke the private boot instance. |
| DL NTSC-U moves traces | `source/Ratchet Gladiator/dlntsc-dreadzone-moves-{1..7}.trace`, script `dlntsc-dreadzone-moves.sh <take>`, layout `dlntsc-dreadzone-moves.toml` | Self-made, 2026-10-15, for the kit's DL profile (`hero-controller.md`, "Deadlocked's moves"). Takes 1, 2, 4, 5 and 7 from the third-person state, 3 and 6 from `-open.p2s` (lock-strafe). 1: a walk and stop, held and tapped jumps, a run jump, a late second press, R1 and X, R1 while running then X. 2: strafing with L2, jumps to the sides and back. 3: lock-strafe walks and jumps. 4: second presses at eight delays. 5: early second presses, a held double jump, R1 while walking. 6, 7: flip attempts with L2 or R1 held. 1,559, 924, 713, 1,320, 1,020, 837 and 754 frames. The layout adds the camera type, the strafe flag, the facing spring, the glide rate and the jump block's flags to `re/SCUS-97465/hero.toml`. |
| DL NTSC-U melee traces | `source/Ratchet Gladiator/dlntsc-dreadzone-melee-{1..5}.trace`, script `dlntsc-dreadzone-melee.sh <take>`, layout `dlntsc-dreadzone-melee.toml` | Self-made, 2026-10-15, for DL's melee (`hero-controller.md`, "Deadlocked's melee"), every take from the third-person state (slot 2 of the `dl` boot), whose first Square swaps the gun in his hand for the wrench. 1: single standing swings and the combo at two paces. 2: swings from a run and with the stick held. 3: jump attacks from a standing jump, a run jump and double jumps. 4: throws with R1 and with L1, and late presses after swings. 5: R1 with Square, X, and R1 with X into swing 1 at frame 25-28, and Square early in double jumps. 932, 826, 1,261, 1,176 and 1,126 frames. The layout adds the animation frame, rate and blending flag, the attack row, the clank latch, the reach scale, the wrench's line and the hand item to the moves layout. |
| DL NTSC-U camera traces | `source/Ratchet Gladiator/dlntsc-dreadzone-cam-{1,2,3}.trace`, script `dlntsc-dreadzone-cam.sh <take>`, layout `dlntsc-dreadzone-cam.toml` | Self-made, 2026-10-16, for DL's camera (`camera.md`, "Deadlocked's camera"), on a `boot.sh` instance named `dl-camB` (`DISPLAY_NUM=94 PINE_SLOT=28014`) whose slot 1 is `dlntsc-dreadzone-open.p2s` (takes 1, 2) and slot 2 `dlntsc-dreadzone-third.p2s` (take 3). 1: lock-strafe right-stick taps and long turns, pitch up and down, walks, a turn while walking, a standing, a double and a run jump. 2: L2 first person with turns, an L2 tap, then the wrench, a walk and a turn. 3: third person turns, runs sideways and towards the camera, jumps. 1,256, 597 and 496 frames; the capture slowed the game, so some samples are torn (a value repeats, then jumps two frames). The layout adds the hero's look block, camera 0, its update cameras and the follow blocks to the moves layout. |
| UYA NTSC-U melee trace | `source/Ratchet & Clank 3/uyantsc-veldin-melee-1.trace`, script `uyantsc-veldin-melee-1.sh`, layout `uyantsc-veldin-melee.toml` | Self-made, 2026-10-13, from the uya boot's slot 1 (P1.8). It holds two standing throws, a throw from a walking crouch, a combo with the stick held and a run-jump attack, with the hero's hand and wrench tip, the animation frame, the attack row and the wrench object (at 0x1D63E00 in that boot). 2,383 frames. Older traces export with the layout once its `wrench` region is removed. |
| UYA NTSC-U animation trace | `source/Ratchet & Clank 3/uyantsc-veldin-anim-1.trace`, script `uyantsc-veldin-anim-1.sh`, layout `uyantsc-veldin-anim.toml` | Self-made, 2026-10-13, from the uya boot's slot 1 (P1.17). Idle, walking, jumps, double jump and glide, run jumps, crouching, a heli jump, three flips, the combo, a jump attack, a throw, and walking off the cliff, with the hero's moby's sequence, keyframe and blend (moby at 0x1D36D00 in that boot). 4,425 frames. Gives `hero-controller.md`, "Animations per state". |
| UYA NTSC-U camera trace | `source/Ratchet & Clank 3/uyantsc-veldin-cam-1.trace`, with its script `uyantsc-veldin-cam-1.sh` and layout `uyantsc-veldin-cam.toml` | Self-made, 2026-10-12, for the camera (RE.4): left stick only (runs in each direction and a circle, a held jump, a double jump, a run jump), with the camera, its update object and its follow block captured. The hero ran off the cliff ahead of the start twice and respawned. 2,673 frames. |
| UYA NTSC-U, Aquatos sewers swimming | `source/Ratchet & Clank 3/uyantsc-sewers-swim.p2s` | Headless, 2026-10-13: warped from `uyantsc-veldin-play.p2s` to level 28 (`warp.sh 28 --from-state 1`), then walked forward into the water. The hero floats on the surface (state 54). A new game's items only. The water here flows and carries him along the channel. |
| UYA NTSC-U, Aquatos sewers diving | `source/Ratchet & Clank 3/uyantsc-sewers-dive.p2s` | The swim savestate a few seconds later, with the sewer object's no-dive store patched so dives work: `pine-probe poke --word 0x323C30 0x24020000` (it was `addiu v0, zero, 2`; that address is in the sewers' overlay only). See `hero-controller.md`, "Swimming". |
| UYA NTSC-U, Veldin at a ledge | `source/Ratchet & Clank 3/uyantsc-veldin-ledge.p2s` | Made 2026-10-13 from `uyantsc-veldin-play.p2s` in the `uya-place` boot (slot 5 there) by writing to the research instance. The two attackers of class 5821 (mobys at 0x1D51900 and 0x1D51A00 in that boot) are moved to (100, 100, 0), because they knock the hero away within seconds. The hero is placed at (345.73, 117.25, 78.0), yaw −π/2: 0.11 u from the north face of the two stacked crates at (345.73, 116.19) (class 500, top at 79.98), facing it. The camera looks west. Positions are floats written with `pine-probe poke --word` (hero position at 0x1A4C60, yaw at 0x1A4C78, a moby's position at +0x10). |
| UYA NTSC-U ledge traces | `source/Ratchet & Clank 3/uyantsc-veldin-ledge-{1,2,3}.trace`, each with its `.sh` script; layout `uyantsc-veldin-ledge.toml` | Self-made, 2026-10-13, from the ledge savestate. 1: a held jump into a grab and hang, then shimmies west to the stack's edge, back and a fall. 2: grabs from tapped jumps, letting go with R1 and with X while pulling away. 3: a grab and the climb onto the stack. 1,091, 957 and 199 frames. The layout adds the ledge block (0x1A50E0) and the animation frame to `re/SCUS-97353/hero.toml`. |
| UYA NTSC-U, Starship Phoenix at a wall | `source/Ratchet & Clank 3/uyantsc-phoenix-wall.p2s` | Made 2026-10-13 in the `uya-place` boot (slot 8 there): warped from Veldin to level 3 (`warp.sh 3 --from-state 1`, Start to skip the first-visit cutscene). Then the hero was placed at (172.65, 100.26), yaw −0.7325, against a side wall of the bridge corridor between two low blocks, by writing his position and yaw. A jump there opens the wall window. |
| UYA NTSC-U wall-jump traces | `source/Ratchet & Clank 3/uyantsc-phoenix-wall-{1,2}.trace`, each with its `.sh` script; layout `uyantsc-phoenix-wall.toml` | Self-made, 2026-10-13, from the wall savestate: a standing jump, then X 0.13 s later, tapped (1) or held (2), giving a wall jump each time. 351 and 350 frames. The layout adds the wall window and wall normals to `re/SCUS-97353/hero.toml`. |
| UYA NTSC-U swim traces | `source/Ratchet & Clank 3/uyantsc-sewers-swim-{1..4}.trace`, each with its `.sh` script; layout `uyantsc-sewers-swim.toml` | Self-made, 2026-10-13, from a `boot.sh` instance named `uya-place` (`DISPLAY_NUM=96 PINE_SLOT=28014`, slot 2 the swim state, slot 3 the dive state). 1 (swim state): surface idle, a surface swim and a water jump that landed on the bank (then a cutscene). 2 (swim state): a water jump and the bob after it, a dive refused, a surface swim. 3 (dive state): dives, underwater swimming, surfacing. 4 (dive state): a dive into the fast swim, a fast dive, underwater idle. 1,193, 608, 1,394 and 948 frames. The layout adds the water, float, swim-block and animation fields to `re/SCUS-97353/hero.toml`. |
| UYA NTSC-U, Florana | `source/Ratchet & Clank 3/uyantsc-florana-play.p2s` | Made 2026-10-15 in the `uya-place` boot (slot 9 there): warped from Veldin to level 2 (`warp.sh 2 --from-state 1 --slot 28014`), with the grind boots byte 0x142CAE set to 1. The hero stands idle at (478.85, 597.55, 315.44). |
| UYA NTSC-U zip-line traces, Florana | `source/Ratchet & Clank 3/uyantsc-florana-zip-{1,2}.trace`, script `uyantsc-florana-zip.sh <n>`, layout `uyantsc-florana-zip.toml` | Self-made, 2026-10-15, from the Florana savestate. The script places the hero over the first zip line and starts the capture itself. 1: no input; the catch, the ride and the drop at the end. 2: X, Square and the stick during the ride. 376 and 374 frames. The layout adds the zip-line block (0x1A5250) and the no-catch timer to `re/SCUS-97353/hero.toml`. |
| UYA NTSC-U, Crash Site | `source/Ratchet & Clank 3/uyantsc-crash-play.p2s` | Made 2026-10-15 in the `uya-place` boot (slot 6 there): warped from Veldin to level 17, grind boots byte set, saved once the arrival's scripted walk (state 101) had ended. The hero stands idle on the landing pad at (575.74, 456.20, 115.17). |
| UYA NTSC-U zip-line trace, Crash Site | `source/Ratchet & Clank 3/uyantsc-crash-zip-1.trace`, script `uyantsc-crash-zip.sh 1`, layout `uyantsc-florana-zip.toml` | Self-made, 2026-10-15, from the Crash Site savestate with the grind boots cleared. Dropped 1 u above the cable, the hero lands on top of it and slides off into the chasm, then comes back at the landing pad. 1,728 frames. **Don't drop him on this cable with the grind boots set:** the grind starts, the level has no grind animations, and PCSX2 aborts. |
| UYA NTSC-U crouch-flip trace | `source/Ratchet & Clank 3/uyantsc-veldin-crouchflip-1.trace`, script `uyantsc-veldin-crouchflip-1.sh`, layout `uyantsc-veldin-crouchflip.toml` | Self-made, 2026-10-15, for the crouch flip (R1 + stick + X; `hero-controller.md`, "Flips"), from a `boot.sh` instance named `uya-flip` (`DISPLAY_NUM=95 PINE_SLOT=28015`) whose slot 1 is a copy of `uyantsc-veldin-play.p2s`, captured with `re/SCUS-97353/hero.toml`. Seven takes: 1 to 3, a crouch, then left, right or back with X; 4, X from an old crouch (a heli jump) and the stick 6 frames later, which flipped; 5, the stick held through a second of crouch, then X (a heli jump, and the crouch turn); 6, a run, a crouch, then left and X (a side flip carrying the run); 7, R1, left and X together from idle. 1,348 frames. The layout adds the pad's pressed mask, the ground counters, the landing countdown, the flip block (0x1A5070-0x1A50DD) and the last ground point to `hero.toml`. |
| UYA NTSC-U look and lock-strafe camera traces | `source/Ratchet & Clank 3/uyantsc-veldin-camlock-{1,2,3,4}.trace`, script `uyantsc-veldin-camlock.sh <take>`, layout `uyantsc-veldin-camlock.toml` | Self-made, 2026-10-16, for the look, lock-strafe and first-person cameras (RE.4; `camera.md`, "Lock-strafe and first person"), from a `boot.sh` instance named `uya-camA` (`DISPLAY_NUM=95 PINE_SLOT=28015`) whose slot 1 is a copy of `uyantsc-veldin-play.p2s`; the script loads it and starts the capture itself. Take 1 (setting 0): L1 look held with the right stick turned both ways and tilted, a 3-frame tap, a second look. 494 frames. Takes 2-4 (2026-10-17) write the camera setting (0x1439F4) to 1 or 2 with the offset, limits and flag bit `hero_camera_set` would give (only ever on the private instance); 4 also gives and draws the Shock Blaster. 2: lock-strafe turns, tilts, walks, jumps and L2; 3: the first-person setting; 4: lock-strafe with the gun. 1,661, 1,569 and 605 frames. `-2-flag1.trace` is a first run of take 2 that left flag bit 0 set (the left stick turned the view); don't use it. The layout adds the hero's free-look block (0x1A6100), the setting, the camera request, the camera's update objects and blend, the field of view and the right stick to `uyantsc-veldin-cam.toml`. |
| UYA NTSC-U firing traces | `source/Ratchet & Clank 3/uyantsc-veldin-fire-{1,2}.trace`, script `uyantsc-veldin-fire.sh <take>`, layout `uyantsc-veldin-fire.toml` | Self-made, 2026-10-20, for the hero's side of firing (RE.7; `weapons.md`, "The hero's side of firing"), from a `boot.sh` instance named `uya-fire` (`DISPLAY_NUM=92 PINE_SLOT=28017`) whose slot 1 is a copy of `uyantsc-veldin-play.p2s`; the script loads it, moves the two attackers away, gives the gun and starts the capture itself. 1: the Shock Blaster (30 rounds): single shots standing, a second tap inside the cooldown, Circle held, a shot while walking, a shot then X twice (a double jump while firing), the same jump without firing, L1 look with the gun, R1 and Circle. 796 frames. 2: the N60 Storm (150 rounds): Circle held standing, walking forward with firing held, the stick turned while firing and without, a walk let go with the gun out and while firing, X twice while firing, then Square, which swapped to the wrench and swung it (state 19), and a 4-frame walk with the wrench. 790 frames. |
| UYA NTSC-U gun effects video | `source/Ratchet & Clank 3/uyantsc-veldin-fxref.sh <take>` (videos regenerable, in `cache/pcsx2/fxref/uyantsc-veldin-fxref-<take>.mkv`) | Self-made, 2026-10-28, reference video of the six first guns' effects at V3 (PLAN P3.18), from a `boot.sh` instance named `uya-fire` (`DISPLAY_NUM=92 PINE_SLOT=28017`) whose slot 1 is a copy of `uyantsc-veldin-play.p2s`; the script loads it, moves the two attackers away, gives the gun at its V3 level row and records display `:92` with ffmpeg (lossless x264, 30 fps). 1: the Shock Blaster (item 0x27, row 0x29, 30 rounds): three single shots. 2: the N60 Storm (0x2F, row 0x31, 150 rounds): Circle held 3.0 s, twice. 3: the Nitro Launcher (0x77, row 0x79, 20 rounds): three single shots. 4: the Lava Gun (0x11, row 0xAE, 300 rounds): Circle held 3.0 s, twice. Its row comes from the game's tables: the word at `0x2C6674` is 0x00A10038, so V2 row R2 = 0xA1, and the V2 entry at `0x2C5640 + R2 * 0xF0 + 0x44` is 0x00AE0038, so V3 row R3 = 0xAE. 5: the Annihilator (0x3F, row 0x41, 30 rounds): three single shots. 6: the Flux Rifle (0x6F, row 0x71, 20 rounds): three single shots. Each take draws the gun with one Circle tap first. Videos run 12.1 s (13.7 s for the holds in takes 2 and 4). |
| Kit demo arena replays | `source/Ratchet & Clank 3/kitdemo-arena-coil.orin`, made by `kitdemo-arena-orin.py 90 16 6 60 <out>`, played by `kitdemo-arena-replay.sh coil 45` | Self-made, 2026-10-28, for checking the kit's gun effects on screen with the disc (PLAN P3.18): an input recording of the kit's own demo, not of the game. From the start it takes 16 next-gun presses to the Plasma Coil (V3), turns the camera 118° right toward the arena, runs 90 ticks and fires six shots 66 ticks apart; the first charger comes at him and the balls stick to it, arcing to the shooters near it. 834 ticks. Also `-turret.orin` (`90 18 3 200`, the Miniturret, 1,082 ticks), `-agents.orin` (`90 9 3 200`, the Agents of Doom, 965) and `-hydra.orin` (`90 7 7 40 <out> 60 140`, the Spitting Hydra held 60 ticks at a time, turned 140°, 1,028; it never marks) and `-disc.orin` (`90 8 6 45`, the Disc Blade Gun, 640; its discs glance off the enemies and burst blue). The script runs Xvfb `:97` with lavapipe and records it with ffmpeg to `$OUT/run10.mkv` (`cache/fxrun` by default); the generator's optional sixth and seventh arguments are the hold and the turn. Replays only match while the demo's level, enemies and guns stay as they are. |
| UYA NTSC-U death traces | `source/Ratchet & Clank 3/uyantsc-veldin-death-{1,2,3,4}.trace`, script `uyantsc-veldin-death.sh <take>`, layout `uyantsc-veldin-death.toml` | Self-made, 2026-10-23, for the death and the respawn (RE.3, P1.5; `hero-controller.md`, "Getting hit and dying"), from a `boot.sh` instance named `uya-death` (`DISPLAY_NUM=91 PINE_SLOT=28018`) whose slot 1 is a copy of `uyantsc-veldin-play.p2s`; the script loads it, writes the health and starts the capture itself. 1: health 1 and the hero written 4 u south of the two attackers, who hit and kill him; he respawns at the level's start. 2: the attackers moved away, a held jump, health 0 written in the air, so he dies on landing. 3: the hero written over the drop east of the start, so he falls out of the level (state 121) and is put back; 4: the same at health 5. 1,215, 1,055, 766 and 769 frames. The layout adds health, the death state's timer, flag and report, the drowning counter, the game mode and Veldin's `screen_fade` to `uyantsc-veldin-anim.toml`. |
| DL NTSC-U death traces | `source/Ratchet Gladiator/dlntsc-dreadzone-death-{1,2}.trace`, script `dlntsc-dreadzone-death.sh <take>`, layout `dlntsc-dreadzone-death.toml` | Self-made, 2026-10-23, for DL's death, death fall and respawn (RE.3, P1.5; `hero-controller.md`, "Getting hit and dying"), from a `boot.sh` instance named `dl-death` (`DISPLAY_NUM=91 PINE_SLOT=28018`) whose slot 1 is a copy of `dlntsc-dreadzone-open.p2s`; the script loads it, writes to the game and starts the capture itself. 1: a held jump with health (hero+0x2E20) written to 0 in the air. 2: `level_kill_height` (0x222BFC) written to 200, then a double jump, into the death fall (118). Both reload the station: the frame counter restarts at 0 after a pause of about 3 s. Writing the hero's position doesn't hold in DL (the next frame puts it back). 1,104 and 893 frames. The layout adds health, libdl's `HeroTimers` death fields, the death bytes, the report (0x368BE8), the game mode and the kill height to the melee layout. |
| UYA NTSC-U camera volume and flythrough traces, Florana | `source/Ratchet & Clank 3/uyantsc-florana-camvol-{1..4}.trace`, script `uyantsc-florana-camvol.sh a|b`, layout `uyantsc-florana-camera.toml` | Self-made, 2026-10-17, for camera volumes and path cameras (RE.4; `camera.md`, "Camera volumes and splines"), from the `uya-camA` boot with slot 2 a copy of `uyantsc-florana-play.p2s`. The script writes the hero's position (private instance only) and captures between writes, since PINE serves one client at a time. 1: dropped into volume cuboid 21 (he fell into water and drowned); 2: into the flythrough's cuboid 77, which played; 3: south of the volume (water again); 4 (`b`): into the volume, then walked north out of it. 339, 357, 356 and 799 frames. The layout is `uyantsc-camera-obstacles.toml` with Florana's bases (camera data 0x1740 lower than Veldin's, core globals 0x1680 lower) plus the volume slots, the level cameras' update objects, the blend and the fade. |
| UYA NTSC-U camera ceiling, sweep and blocked-view traces | `source/Ratchet & Clank 3/uyantsc-veldin-camceil-{1,2,3}.trace` and `uyantsc-veldin-camblock-1.trace`, scripts `uyantsc-veldin-camceil.sh [3]` and `uyantsc-veldin-camblock.sh`, layout `uyantsc-camera-obstacles.toml` | Self-made, 2026-10-17, for the camera's clearance point, arm sweep and blocked-view reset (RE.4; `camera.md`, "Obstacles and collision"), from the `uya-half` boot with slot 2 a copy of `uyantsc-veldin-ledge.p2s`. Veldin has no low ceiling, so the top crate of the stack (moby 0x1D4A300 in that state, position at +0x10) is written over the hero's head, 2.6 u (1) and 1.9 u (2) above his feet, and beside the camera's arm (3); for the reset the hero is written 1.5 u under the floor. Only ever poke the private instance. 400, 407, 269 and 244 frames. |
| UYA NTSC-U half-tilt camera traces | `source/Ratchet & Clank 3/uyantsc-veldin-camhalf-{0.5,0.75}-{1,2}.trace`, script `uyantsc-veldin-camhalf.sh <take> <scale>`, layout `uyantsc-veldin-camlock.toml` | Self-made, 2026-10-17, for the cameras' stick response (RE.4), from a `boot.sh` instance started with `AXIS_SCALE=0.5`, named `uya-half` (`DISPLAY_NUM=94 PINE_SLOT=28016`), whose slot 1 is a copy of `uyantsc-veldin-play.p2s`. Every stick key gives half tilt ("Making traces with scripted play"). 1: setting 0, the right stick each way (the follow camera), L1 with the right stick (the look), the left stick forward; 2: the same in lock-strafe. The `0.75` takes are the same on a boot with `AXIS_SCALE=0.75`. |
| UYA NTSC-U diagonal part-tilt traces | `source/Ratchet & Clank 3/uyantsc-veldin-diag-{0.5,0.75}-{1,2}.trace`, script `uyantsc-veldin-diag.sh <take> <scale>`, layout `uyantsc-veldin-camlock.toml` | Self-made, 2026-10-24, for the stick's magnitude on a diagonal (RE.3; `hero-controller.md`, "Input", item 3), from a `boot.sh` instance started with `AXIS_SCALE=0.5` or `0.75`, named `uya-half` (`DISPLAY_NUM=94 PINE_SLOT=28016`), whose slot 1 is `uyantsc-veldin-play.p2s`. Each take loads slot 1 and holds two stick keys together for 3.0 s: 1, I and L (forward-right); 2, I and J (forward-left). Frames: 257 (0.5, 1), 287 (0.5, 2), 267 (0.75, 1), 252 (0.75, 2). |
| DL NTSC-U half-tilt camera traces | `source/Ratchet Gladiator/dlntsc-dreadzone-camhalf-0.5-{1,2}.trace`, script `dlntsc-dreadzone-camhalf.sh <take> <scale>`, layout `dlntsc-dreadzone-cam.toml` | Self-made, 2026-10-17, for DL's stick response (RE.4), on the `dl-camB` boot (`DISPLAY_NUM=94 PINE_SLOT=28014`) started with `AXIS_SCALE=0.5`, slot 1 `dlntsc-dreadzone-open.p2s` and slot 2 `dlntsc-dreadzone-third.p2s`: the right stick each way and the left stick forward, in lock-strafe (1) and third person (2). |
| DL NTSC-U flip traces | `source/Ratchet Gladiator/dlntsc-dreadzone-flip-{1,2}.trace`, script `dlntsc-dreadzone-flip.sh <take>`, layout `dlntsc-dreadzone-flip.toml` | Self-made, 2026-10-20, for where DL's flips come from (`hero-controller.md`, "Flips" and "Deadlocked's moves"), on the `dl` boot (slot 2, `dlntsc-dreadzone-third.p2s`). 1: L2 held with the stick left, right, then back, and X tapped every 0.15 s, so the double jumps land into flips. 2: the controls, strafing forward and the stick to the side without L2. 1,849 and 951 frames. The layout adds the frames off the ground, the strafe sector, the flip's side, the gadget in hand and the glide bar (hero+0x2E2C) to the moves layout. |
| DL NTSC-U wall traces | `source/Ratchet Gladiator/dlntsc-dreadzone-wall-{1,2}.trace`, script `dlntsc-dreadzone-wall.sh <take>`, layout `dlntsc-dreadzone-flip.toml` | Self-made, 2026-10-20, looking for a wall jump in DL (`hero-controller.md`, "Deadlocked's moves"), on the `dl` boot. 1 (slot 2, third person): a run and a turn towards the steps the charge takes bounced off; the camera swung with him and he circled short of them. 2 (`dlntsc-dreadzone-boots.p2s` copied into slot 3, lock-strafe): forward up the steps with X tapped, then against a wall above them; double jumps only. 989 and 1,026 frames. Inconclusive: the wall wasn't a clean, tall one. |
| DL NTSC-U tall-wall traces | `source/Ratchet Gladiator/dlntsc-dreadzone-tallwall.p2s` (savestate), `dlntsc-dreadzone-tallwall-{1..10}.trace`, script `dlntsc-dreadzone-tallwall.sh <take>`, layout `dlntsc-dreadzone-tallwall.toml` (`melee` fields only) | Self-made, 2026-10-20, looking for DL's wall jump (state 17; `hero-controller.md`, "Deadlocked's moves"), on a `boot.sh` instance named `dl-ledge` (`DISPLAY_NUM=93 PINE_SLOT=28016`). The savestate is `dlntsc-dreadzone-open.p2s` (slot 1 there) walked to DreadZone Station's back wall (about 0.33 u back from its contact point at x 658.05, hero at (657.73, 327.83, 183.95) facing +x), saved in slot 4 and copied here. 1-4: a standing jump into the wall, X held 0.15 s, then X again after the release (onset gaps 0.25, 0.28, 0.35, 0.45 s). 5-6: back off 0.6 s, then a short walk into the wall with X re-pressed (onset gaps 0.25 and 0.35 s). 7-10: standing, the first X a 0.05 s tap, onset gaps 0.10, 0.13, 0.20 and 0.30 s. Every take: a double jump (state 14), no state 17. In takes 7-10 the second press reached the game 7, 9, 13 and 19 frames into the jump, with the wall-contact flag set from the jump's second frame; he edged 0.3 u further into the wall as he rose, so its foot may slope a little. Frames per trace: takes 1-4 and 7-10 about 255-285 (the 5 s capture), takes 5-6 about 340 (6 s). |
| DL NTSC-U ledge traces | `source/Ratchet Gladiator/dlntsc-dreadzone-ledge.p2s` (savestate), `dlntsc-dreadzone-ledge-{1,2,3}.trace`, script `dlntsc-dreadzone-ledge.sh <take>`, layout `dlntsc-dreadzone-ledge.toml` | Self-made, 2026-10-20, for DL's ledges and shimmy (`hero-controller.md`, "Deadlocked's ledges"), on a `boot.sh` instance named `dl-shimmy` (`DISPLAY_NUM=92 PINE_SLOT=28017`). The savestate is `dlntsc-dreadzone-boots.p2s` walked forward into the tutorial's ledge block on DreadZone Station: lock-strafe, standing against its wall at (655.16, 338.39, 183.95), yaw 1.343; the wall's face is at y 338.87 and its top at z 186.17, 2.22 u up, from x about 650.4 eastwards. 1: a held jump into the grab and hang, a shimmy left for five cycles and right for six, the climb. 2: a grab, L2 lets go (R1 does nothing in lock-strafe), a grab, the stick pulled back with X lets go. 3: a grab, then a shimmy left to the block's west end, where he was carried round its corner. 759, 566 and 834 frames. The layout is the melee layout plus the ledge block (hero+0x1DA0: hang point, lip, wall yaw, found flag at +0x28), the grab's gravity and feet height (+0x1DCC, +0x1DD0) and the no-grab timer (hero+0x388). |
| DL NTSC-U, DreadZone Station's co-op build (level 21) | `source/Ratchet Gladiator/dlntsc-dreadzone-coop.p2s` | Headless, 2026-10-28, for DL's co-op (RE.10; `research/coop.md`), on a `boot.sh` instance named `dl-coop` (`DISPLAY_NUM=89 PINE_SLOT=28019`) whose slot 1 was a copy of `dlntsc-dreadzone-open.p2s`: `warp.sh 21 --from-state 1`, then saved to slot 2 and copied here. The level number alone starts co-op: `local_player_count` 2, `coop_flag` 1, both heroes in memory (0x368F60 and 0x36C150) with 10 health. **It is paused** (game mode 3) over the widescreen split screen (player 1 top left in red, player 2 bottom right in blue, each with a HUD, the strips beside them tinted in their colours; `coop.md`, "Split screen"), and neither Start, Circle nor Cross closed it while port 2 was empty (player 2 then reads the pad block 0x1EEBC0). On a `PAD2=1` boot, Cross on BACK closes it. |
| DL NTSC-U, DreadZone Station's co-op build in play | `source/Ratchet Gladiator/dlntsc-dreadzone-coop-play.p2s` | Headless, 2026-10-28, on the `dl-coop` boot started with `PAD2=1` (both pads on the keyboard), from `dlntsc-dreadzone-coop.p2s`: Cross on BACK, then the opening tutorial on both pads at once (right stick left, right and up, R1, a double jump), then each player walked forward a little; saved in slot 3. Normal play (game mode 0), player 1 at (652.95, 333.96, 183.95), player 2 at (556.61, 333.78, 183.94), 10 health each, a new game's items. |
| DL NTSC-U co-op death traces | `source/Ratchet Gladiator/dlntsc-dreadzone-coop-death-{1,2,3}.trace`, script `dlntsc-dreadzone-coop-death.sh <take>`, layout `dlntsc-dreadzone-coop.toml` | Self-made, 2026-10-28, for co-op's death, respawn and restart (RE.10; `research/coop.md`, "Death, respawn and failure"), on the `PAD2=1` `dl-coop` boot (`DISPLAY_NUM=89 PINE_SLOT=28019`) whose slot 3 is `dlntsc-dreadzone-coop-play.p2s`; the script loads it, writes the health and starts the capture itself (writes go before the capture: PINE serves one client at a time). 1: player 2 dies in a jump, player 1 stands. 2: player 1 dies, then player 2, so the level restarts. 3: player 2 dies while player 1 walks forward. 1,095, 953 and 1,115 frames. The layout holds both heroes' position, yaw, state, health, respawn countdown, safe time, index and fallback point, their safe-spot rings, the death report, `coop_both_dead`, the game mode, bolts and XP, in small regions. |
| UYA NTSC-U camera vertical-mode traces | `source/Ratchet & Clank 3/uyantsc-veldin-camvert-{1..5}.trace`, script `uyantsc-veldin-camvert.sh <take>`, layout `uyantsc-camera-obstacles.toml` | Self-made, 2026-10-16, for the follow point's height (RE.4; `camera.md`, "The follow point's height"), from a `boot.sh` instance named `uya-camC` (`DISPLAY_NUM=93 PINE_SLOT=28017`) whose slot 1 is a copy of `uyantsc-veldin-ledge.p2s`. 1: held, double and heli jumps, a back flip, a jump attack, then jumps and run jumps on the ground. 2: a grab, the climb onto the crate stack, a walk off its edge. 3: a grab, the climb, a run jump down. 4: (the hero poked 3 u north first, see the script) a run jump past the stack. 5: another grab and climb. 1,926, 582, 556, 303 and 281 frames. The layout adds the follow block's vertical-mode, clearance, look-spring and pull-in fields, the camera's ground counters and past-apex byte, the carried move and the right stick to `uyantsc-veldin-cam.toml`. |
| UYA NTSC-U camera wall trace, Starship Phoenix | `source/Ratchet & Clank 3/uyantsc-phoenix-camwall-1.trace`, script `uyantsc-phoenix-camwall.sh 1`, layout `uyantsc-phoenix-camera-obstacles.toml` | Self-made, 2026-10-16, for the camera's obstacles (RE.4; `camera.md`, "Obstacles and collision"), from the `uya-camC` boot with slot 2 a copy of `uyantsc-phoenix-wall.p2s`: the right stick turning the camera into the corridor's walls and back, runs along and away from the wall. 1,278 frames. On Phoenix the camera blocks sit lower than on Veldin (GameCamera 0x1E2540, update object 0x1E2E40, follow block 0x1E5010), so this layout is `uyantsc-camera-obstacles.toml` with those bases. |
| R&C3 PAL camera trace | `cache/pcsx2/traces/rc3pal-veldin-5-cam.trace`, layout `source/Ratchet & Clank 3/rc3pal-veldin-5-cam.toml` | A replay of the owner's recording 5 with the camera block added (RE.4, `docs/research/camera.md`). It can be regenerated from the layout. |

Working copies of the self-made savestates and traces are also in `cache/pcsx2/ntsc-states/`
and `cache/pcsx2/traces/`.

For the replay harness, copies live in `cache/pcsx2/recordings/` as
`<release>-<level>-<n>.p2m2` with `<name>.p2m2_SaveState.p2s` beside them (PCSX2 looks for
`<recording file name>_SaveState.p2s`). Their traces are in `cache/pcsx2/traces/`.

### Still wanted

Since 2026-10-12 the movement constants come from the game code
(`docs/research/hero-controller.md`), so recordings are now for **checking** the code
reading, not for measuring. On UYA NTSC we can also make our own traces with scripted
keyboard play ("Making traces with scripted play"), part-tilted sticks included. From the
owner, these still help:

1. **Moves that need a particular place:** wall jump, ledge grab and ledge jump,
   grinding, swimming and charge boots, on any release. For grinding it has to be DL or
   GC: UYA has no rail to grind on (`hero-controller.md`, "Grinding"). A savestate next to each is
   enough, and we can script the move from there. A recording is better.
2. **DL NTSC gameplay:** a savestate on DreadZone Station in play rather than in a
   cutscene. Booting a new game gets there too, but the owner's would save time.

### How to make a recording

1. Load a savestate, then **Tools → Input Recording → New**, choose "from savestate", and
   name the file `<something>.p2m2`.
2. Play the move, then **Tools → Input Recording → Stop**.
3. Leave the `.p2m2` and its `_SaveState.p2s` together in `source/<game>/` or
   `cache/pcsx2/recordings/`.

## Replaying input recordings headlessly (RE.5)

`tools/pine-probe/replay.sh` replays a recording in a private PCSX2 and records the hero
every frame:

```sh
tools/pine-probe/replay.sh "<disc>.iso" cache/pcsx2/recordings/<name>.p2m2 \
  re/<serial>/hero.toml cache/pcsx2/traces/<name>.trace
target/release/pine-probe export cache/pcsx2/traces/<name>.trace --layout re/<serial>/hero.toml > <name>.csv
target/release/pine-probe recording cache/pcsx2/recordings/<name>.p2m2 > <name>.rec.csv
```

- **Requirements:** `xdotool` and Xvfb (`sudo apt install xvfb`; installed on the owner's
  machine since 2026-10-12). Without sudo, `apt download xvfb` and `dpkg -x xvfb_*.deb
  <dir>` give a working `<dir>/usr/bin/Xvfb` (the other libraries it needs are already on a
  desktop install); pass it as `XVFB=`.
- **What it does:**
  1. Starts Xvfb on `:99`, so nothing appears on your desktop and your keyboard focus is
     never touched.
  2. Writes a private `PCSX2.ini` (`pine-probe headless-config`) into
     `cache/pcsx2/headless/xdg/PCSX2/` from yours: same BIOS, CPU settings, patches and
     game fixes (the replay must match the recording), but no sound, no controllers, no
     prompts, OpenGL through Mesa, PINE on slot 28012 and half speed.
  3. Copies your memory cards and patches the copies' checksums (`pine-probe memcard-sync`).
  4. Boots the disc, opens **Tools → Input Recording → Play** with xdotool, starts
     `pine-probe capture`, then unpauses with Space.
  5. The capture stops 5 s after PCSX2 pauses at the end of the recording.
- **Takes** about 80 s for a 1,400-frame recording at half speed on the owner's machine.
- **Verified 2026-10-11:** all four PAL recordings replayed with no missing frames, and
  replaying the same recording three times gave identical hero data on every frame.

Things that broke the first attempts, and how the harness avoids them:

- **Memory-card eject.** A savestate stores each card's checksum. If the card was written
  since (autosave does this), PCSX2 ejects the cards on load and R&C3 stops on a "memory
  card removed" message. `memcard-sync` writes the savestate's checksum into the copied
  card (PCSX2 keeps it at byte 0x210 of the card file). See `tools/pine-probe/src/memcard.rs`.
- **Paused start.** Playback loads the savestate and leaves the emulator paused. The
  harness unpauses only after the capture is running, so frame 0 is not missed.
- **Missed frames at full speed.** With the software renderer using every core, the
  capture thread sometimes went 20 ms or more without running. Half speed, plus `nice` on
  PCSX2, removed every gap.
- **Torn reads.** PINE answers while the game runs. The capture rereads the frame counter
  after each sample and keeps the settled version of each frame (see
  `tools/pine-probe/src/capture.rs`).
- **Rows read mid-update (DL).** The frame counter is settled, but a DL row can still be
  read part way through the hero's update: its animation frame is already moved on while
  its state, and mostly its motion, are the frame before's (`hero-controller.md`,
  "Deadlocked's melee", "Reading the traces"). Before reading a one-frame difference
  between two fields as a rule, check where in the update each is written. UYA's rows
  are read after its whole update, transitions included: no split. So a DL row's state
  timer is one less than its transitions see, and a UYA row's is the same
  (`hero-controller.md`, "Reading the traces").
- **Alignment.** The first trace record is the paused frame 0. A button pressed in
  recording frame *n* changes the hero state in trace record *n* + 1 to *n* + 3 (one or
  two frames of game-side input latency, depending on the move), so analyses should
  calibrate against a known press.
- **Settings drift.** PCSX2 rewrites its ini on exit, so the harness regenerates the
  private ini before every run.

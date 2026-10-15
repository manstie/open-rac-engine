# Reverse-engineering knowledge base

Machine-readable facts about each release, written by us (PLAN.md task RE.2). There is one
folder per disc, named by its serial (see `docs/research/disc-layout.md`). Prose findings
live in `docs/research/`. This folder holds the data that tools read.

**Never put game data here**: no dumps, no code, no extracted assets. Addresses, offsets,
sizes, names, constants and checksums are fine.

## Files

| File | Read by | Contents |
|---|---|---|
| `<serial>/hero.toml` | `tools/pine-probe` | Where the hero lives in EE memory, a per-frame counter, and typed fields |
| `<serial>/symbols.toml` | `tools/ghidra/ApplySymbols.java`, and `tools/pine-probe`'s tests | Our names for code and data addresses, each with a confidence and how it was found |

Folders so far: SCES-52456 (R&C3 PAL), SCES-53285 (Gladiator PAL), SCUS-97353 (UYA) and
SCUS-97465 (Deadlocked), each with both files. UYA's hero layout adds a `pad` region, so
its traces record the inputs too.

Planned (RE.2): `structs.toml` and `memory-map.md` per release.

## `hero.toml`

```toml
release = "SCES-52456"     # must match the folder name
title = "..."
refresh_hz = 50            # 50 for PAL, 60 for NTSC
frame_counter = 0x0015BC80 # a u32 the game increments exactly once per frame
notes = "..."              # how the addresses were found and checked

[[region]]                 # memory captured every frame, in order
name = "hero"
base = 0x001A4BC0
size = 0x2940

[[field]]
name = "pos"
region = "hero"
offset = 0xA0              # from the region's base
type = "vec3"              # u8 i8 u16 i16 u32 i32 f32 vec3 vec4
key = true                 # used to pick the settled sample in each frame
note = "how this field was identified and verified"
```

- Addresses are EE physical addresses (0x00000000–0x01FFFFFF), as PINE and PCSX2's
  debugger use them.
- Each field's `note` says how it was verified. A field without verification says so.
- `key` fields should be ones the game writes once per frame (position, velocity, state).
  The capture keeps, per frame, the version of the key fields it saw most often.
- `cargo test -p pine-probe` checks that every `hero.toml` here parses and is consistent.

## `symbols.toml`

```toml
release = "SCES-52456"   # must match the folder name
image = "Veldin, ..."    # the memory image the addresses come from
notes = """..."""        # optional

[[symbol]]
name = "hero_update"     # ours: letters, digits and underscores, unique in the file
address = 0x004EE4E0
kind = "function"        # function or data
confidence = "high"      # high, medium or low (below)
note = "What it does and how it was identified, on one line."
```

- **Confidence:**
  - `high`: the behaviour was checked against the running game (a trace or a
    cross-check).
  - `medium`: read from the code but not checked.
  - `low`: inferred from context.
- **Overlays:** code in R&C3, UYA, Gladiator and Deadlocked lives in per-level overlays,
  so a code address holds only for the level named in `image`.
- **One line per value:** keep each symbol's values on one line. `ApplySymbols.java`
  reads the file line by line rather than with a full TOML parser.
- `cargo test -p pine-probe` checks that every `symbols.toml` here parses, sits in the
  right folder, and has unique names and addresses and a note on every symbol.

## Sources

- Our own measurements: savestates and input-recording replays, see `docs/re-setup.md`.
- [libuya and libdl](https://github.com/Horizon-Private-Server) (MIT, © 2021 Daniel
  Gerendasy) for struct layouts, enum numbering and addresses. We cite them and do not
  copy their headers.
  - Each `symbols.toml` has a section of symbols seeded from them, with `low` confidence
    until someone reads the code. Every seeded address was checked against our own
    memory image: a function must be called there, a global used by the code.
  - libdl's resident addresses hold in Deadlocked's single player. libuya's are the
    multiplayer program's and don't hold in UYA's single player, so UYA, R&C3 and
    Gladiator got libdl's library functions by matching code
    (`docs/research/engine-notes.md`, "Code layout and overlays").
- [Programie/RCMemory](https://github.com/Programie/RCMemory) for PAL addresses. It has no
  licence, so only individual addresses (facts) are used, each re-verified.

# Contributing

Thanks for helping. Read [PLAN.md](PLAN.md) for where the project stands and what comes
next. Read [CLAUDE.md](CLAUDE.md) for the full working rules, which apply to people and AI
assistants alike. This page is the short version.

## The legal rules (non-negotiable)

The repository is public and MIT-licensed, so everything committed here is published.

**Never commit:**
- disc images;
- anything extracted, decompressed or converted from a disc: WADs, textures, models,
  audio, video, ELFs, overlays, VU microcode;
- memory dumps, savestates, memory cards, input recordings or captured traces;
- game screenshots;
- disassembly or decompiled game code, even from a public decompilation project;
- Sony SDK or NDA material.

**Fine to commit:**
- your own code;
- facts about the discs written in your own words: offsets, sizes, hashes, struct
  layouts, symbol names and addresses;
- test fixtures that the tests generate themselves.

**Where disc-derived data goes:**
- Your discs go in `source/`, which git ignores.
- Anything derived from them goes in `cache/`, which git also ignores.

**Copyleft code is reference only.** GPL and LGPL projects (Wrench, PCSX2, OpenRAC,
rac1-decomp and others) may be read to learn facts, never copied or translated line by line.
Permissively licensed code may be reused with attribution, both in the file header and in
[docs/research/prior-art.md](docs/research/prior-art.md).

**The guard.** `tools/check-data-hygiene.sh` rejects disc-derived file types and files
over 1 MB. CI runs it on every push. To run it before every commit too, enable the hook
once with `git config core.hooksPath .githooks`.

## Building

1. Install Rust with [rustup](https://rustup.rs). `rust-toolchain.toml` pins the version.
2. Install the platform dependencies:
   - Debian/Ubuntu:
     `sudo apt install build-essential pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`.
   - Windows: the Visual Studio Build Tools with the C++ workload.
3. Run `cargo run`.

## Definition of done

A change is ready when:
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace` all pass;
- new behaviour has tests on synthetic data, or the change says why it has none;
- tests that need game discs read `OPENRAC_SOURCE_DIR` and are marked
  `#[ignore = "needs game discs"]`;
- PLAN.md and the docs are updated: tick the task by its ID, record findings in
  `docs/research/` with how they were verified, and record decisions in PLAN.md §5;
- the diff contains no game data (the guard above checks).

Commit messages describe the change and mention the PLAN.md task IDs they touch.

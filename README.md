# Open Ratchet & Clank Engine

A character controller for Ratchet as he plays in *Ratchet & Clank 3* and *Ratchet:
Gladiator*, written in Rust as plugins for the [Bevy](https://bevy.org) game engine, with
a demo to play it in.

> [!NOTE]
> **No game content is included.** The kit and its demo run without the games; with your
> own copy of a disc, the demo can also use Ratchet's real models and sounds. This is an
> unofficial fan project; see the [disclaimer](#disclaimer).

## Goals

- **A hero kit for your own projects.** Ratchet's moves, camera, melee and weapons from
  *Ratchet & Clank 3* and *Ratchet: Gladiator*, each game's with its own numbers read from
  that game's code, rebuilt to feel as close to the originals as possible.
  - It runs **without any game disc**, using a placeholder character made of simple shapes.
  - It is packaged as Bevy plugins you can drop into your own projects and mash-ups.
  - With your own disc of either game, the demo uses Ratchet's real models, animations,
    sounds and weapon effects, read from the disc as it starts.
- **Controller, keyboard and mouse support**, with every action remappable.
- **Two-player online co-op, connecting the players directly** with no servers to host.

## Games

| Game | Also released as | Disc versions | Status |
|---|---|---|---|
| *Ratchet & Clank 3* (2004) | *Ratchet & Clank: Up Your Arsenal* | SCUS-97353 (USA), SCES-52456 (Europe/Australia) | Playable in the demo: the moves, camera, melee and all twenty weapons, tuned to the USA release |
| *Ratchet: Gladiator* (2005) | *Ratchet: Deadlocked* | SCUS-97465 (USA), SCES-53285 (Europe/Australia) | Playable in the demo: the moves, camera and melee, and four of its ten weapons |

Each disc is identified by its contents, not its file name. The details are in
[docs/research/disc-layout.md](docs/research/disc-layout.md).

## How it works

1. **The kit stands alone.** Its crates hold the hero's movement, camera, weapons and
   co-op, and depend on no game data. Without a disc he is a placeholder made of simple
   shapes.
2. **The numbers come from the games' code.** Jump heights, speeds, timings and weapon
   stats are read from each game with reverse-engineering tools and written down, in our
   own words, as that game's profile.
3. **Your disc, if you have one, dresses him.** The demo reads Ratchet's models,
   animations, sounds and effect textures from your own disc image as it starts. Nothing
   it reads is saved.

Nothing from the games is stored in or distributed by this repository.

## Running a release build

Builds for Windows and Linux (x86-64) are attached to the
[releases](https://github.com/manstie/open-rac-engine/releases). Unpack one and run
`openrac.exe` or `openrac` from its folder.

- Your disc images go in a `source` folder in the folder you start it from;
  `--discs <folder>` points it elsewhere. Without discs it plays its own placeholder hero.
  With *Ratchet & Clank 3* or *Up Your Arsenal* (either release in the table above) the
  hero is Ratchet, with his animations, suits and guns from the disc; *Ratchet:
  Gladiator* or *Deadlocked* adds that game's skins.
- `--help` lists the other options. The game prints its log to the terminal it was started
  from, if any; on Windows it opens no console window of its own.
- Linux needs glibc 2.35 or newer (Ubuntu 22.04, Debian 12 and later) and a Vulkan driver.

## Building from source

1. **Install Rust** through [rustup](https://rustup.rs). The exact compiler version is
   pinned in `rust-toolchain.toml` and installed automatically.
2. **Install the platform dependencies:**
   - **Linux** (Debian/Ubuntu):

     ```sh
     sudo apt install build-essential pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
     ```

   - **Windows:** install the
     [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
     with the C++ workload.
3. **Build and run:**

   ```sh
   cargo run
   ```

   The first build takes several minutes; later ones are much faster.
   - The renderer uses Vulkan when a Vulkan driver is available. On Windows it falls back to
     DirectX 12. To choose explicitly, set `WGPU_BACKEND=vulkan` or `WGPU_BACKEND=dx12`.
   - Developer commands (tests, lints, smoke tests) are listed in
     [CLAUDE.md](CLAUDE.md#commands).

**Your disc images** go in the `source/` folder, in any subfolder. Git ignores everything
in that folder, so they can't be committed by accident.

## Project documents

| Document | What's in it |
|---|---|
| [PLAN.md](PLAN.md) | Goals, decisions, the task-by-task roadmap and the current status. |
| [CLAUDE.md](CLAUDE.md) | Working rules for contributors, human or AI: legal rules, setup, commands and conventions. |
| [docs/research/](docs/research/) | What we know so far: how the hero moves, his camera and weapons, the disc formats, and related projects with their licences. |

## Contributing

If you'd like to help, read [CONTRIBUTING.md](CONTRIBUTING.md), and [PLAN.md](PLAN.md) for
where things stand, before opening a pull request. The legal rules matter most:

- **Never commit game data:** no disc images, nothing extracted from a disc, no savestates.
- **Never commit decompiled game code.**
- **Reused code keeps its notices.** The project is GPL-licensed, so code from projects
  under compatible licences (Wrench, PCSX2, the permissive ones) can be reused with its
  copyright and licence notices kept.

## Acknowledgements

This project builds on years of work by the Ratchet & Clank modding community and the
PlayStation 2 preservation community. We have taken facts or code from these projects:

- [Wrench](https://github.com/chaoticgd/wrench): modding tools and file-format research for
  the PS2 games, including the level and gameplay layouts.
- [Horizon](https://github.com/Horizon-Private-Server): the *Up Your Arsenal* and
  *Deadlocked* online revival. Its reverse-engineered game headers (libdl and libuya)
  seeded our symbol tables and moby layouts.
- [OpenRAC](https://github.com/OpenRAC): decompilation projects for the series, and their
  notes on the engine.
- [ReRAC](https://github.com/re-rac/rerac): a native Rust/Bevy port of *Ratchet & Clank*
  (2002). Its notes on mobys guided our model reader.
- [OpenGOAL](https://github.com/open-goal/jak-project): the native PC port of *Jak and
  Daxter*. Our sound engine ports its 989snd sound effects.
- [PCSX2](https://github.com/PCSX2/pcsx2): the PS2 emulator we reverse engineer with. Our
  probe follows its PINE protocol and its savestate formats.

None of these projects is affiliated with this one.

## Disclaimer

Open Ratchet & Clank Engine is an unofficial fan project. It is not affiliated with,
endorsed by or sponsored by Sony Interactive Entertainment or Insomniac Games. *Ratchet &
Clank* is a trademark of Sony Interactive Entertainment LLC.

This repository contains no game assets or game code. To use it you need your own
legitimately obtained copies of the games.

## License

This program is free software: you can redistribute it and/or modify it under the terms of
the [GNU General Public License](LICENSE) as published by the Free Software Foundation,
either version 3 of the License, or (at your option) any later version. It is distributed
in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied
warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the license for
details.

The license covers this project's own code only, not the games or their content. It covers
the hero kit's crates too, so a project that reuses them must be GPL-compatible.

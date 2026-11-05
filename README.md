# Open Ratchet & Clank Engine

An open-source reimplementation of Insomniac Games' PlayStation 2 *Ratchet & Clank* engine,
written in Rust with the [Bevy](https://bevy.org) game engine.

> [!NOTE]
> **No game content is included.** You need your own copies of the games. This is an
> unofficial fan project; see the [disclaimer](#disclaimer).

## Goals

- **A standalone hero kit.** The *Ratchet & Clank 3* / *Gladiator*-era player controller,
  camera, melee and weapons, rebuilt to feel as close to the originals as possible.
  - It runs **without any game disc**, using a placeholder character made of simple shapes.
  - It is packaged as Bevy plugins you can drop into your own projects and mash-ups.
- **Run the original games natively on Windows and Linux**, with DirectX 12 and Vulkan,
  using disc images you supply.
- **Controller, keyboard and mouse support**, with every action remappable.
- **Two-player online co-op in every game, connecting the players directly** with no
  servers to host, and with mechanics based on *Ratchet: Gladiator*'s co-op. Local
  split-screen follows.

## Games

| Game | Also released as | Disc versions being worked with | Status |
|---|---|---|---|
| *Ratchet & Clank 3* (2004) | *Ratchet & Clank: Up Your Arsenal* | SCUS-97353 (USA), SCES-52456 (Europe/Australia) | First target; the hero controller is playable in the demo, tuned to the USA release |
| *Ratchet: Gladiator* (2005) | *Ratchet: Deadlocked* | SCUS-97465 (USA), SCES-53285 (Europe/Australia) | First target; research |
| *Ratchet & Clank* (2002) | | SCUS-97199 (USA), SCES-50916 (Europe/Australia, v2.00) | Planned |
| *Ratchet & Clank 2* (2003) | *Ratchet & Clank: Going Commando* | SCUS-97268 (USA, v2.00), SCES-51607 (Europe/Australia) | Planned |

Other disc versions may be added later. Each disc is identified by its contents, not its
file name. The details are in [docs/research/disc-layout.md](docs/research/disc-layout.md).

## How it works

1. **You supply the game.** Point the engine at an `.iso` image of a disc you own.
2. **The game's data is converted on your machine.** On first run, the engine identifies the
   disc and converts its levels, models, textures, sound and video into a local cache.
3. **The game's code is converted on your machine too.** The original gameplay logic is
   translated from PlayStation 2 machine code into native code ("static recompilation"),
   so each game keeps its own behaviour.
4. **A new engine runs it.** Rendering, audio, input, the player character, the camera,
   co-op networking and the remaster features are reimplemented natively in Rust.

Nothing from the games is stored in or distributed by this repository.

## Running a release build

Builds for Windows and Linux (x86-64) are attached to the
[releases](https://github.com/manstie/open-rac-engine/releases). Unpack one and run
`openrac.exe` or `openrac` from its folder.

- Your disc images go in a `source` folder in the folder you start it from;
  `--discs <folder>` points it elsewhere. Without discs it plays its own placeholder hero.
- `--help` lists the other options. The game prints its log to the terminal it was started
  from, if any; on Windows it opens no console window of its own.
- Linux needs glibc 2.35 or newer (Ubuntu 22.04, Debian 12 and later) and a Vulkan driver.

## Building from source

There is little to see yet: the app opens a window with a placeholder scene.

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
| [PLAN.md](PLAN.md) | Goals, strategy, decisions, the task-by-task roadmap and the current status. |
| [CLAUDE.md](CLAUDE.md) | Working rules for contributors, human or AI: legal rules, setup, commands and conventions. |
| [docs/research/](docs/research/) | What we know so far: the disc layouts, the engine's internals, and related projects with their licences. |

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
  Daxter*, and a model for this project's architecture. Our sound engine ports its 989snd
  sound effects.
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

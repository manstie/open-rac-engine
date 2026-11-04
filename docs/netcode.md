# Netcode: rollback co-op in the demo

How the hero kit's simulation is kept the same on two peers, and the rules simulation
code follows for it (PLAN.md P1.18, section 4.2). The design follows GGPO: both peers
simulate everything, only inputs cross the network, and a peer that guessed a remote
input wrong loads the snapshot of that tick and simulates forward again.

## How a session drives the simulation

- Outside a session, Bevy's fixed loop runs one tick (`FixedFirst` … `FixedLast`) per
  1/60 s of game time.
- In a session, `rac_net` swaps Bevy's `FixedMainScheduleOrder` for its own schedule, so
  each 1/60 s step asks `ggrs` what to do instead. `ggrs` answers with requests to save a
  snapshot, load one, or advance a tick with every player's input. An advance runs the
  same five schedules a normal tick does, with `PlayerInputs` filled from `ggrs`, so no
  system moves for co-op.
- Ticks run again after a load are *resimulated*. Presentation that runs on the tick
  (effects, shot models, sparks) skips them, because what it shows was already shown
  once and isn't part of the simulation.

## What a snapshot holds (`rac_sim::rollback`)

- Every component and resource a kit plugin registers with `RollbackApp`, cloned.
- Only entities with a `SimId`. Components of simulated things require `SimId`; it is
  numbered in spawn order by a hook, so it is the same on every peer.
- Entities spawned after the snapshot are despawned on load. Entities the simulation
  removed after it are brought back: in a session `retire()` only disables them (Bevy's
  `Disabled`, recursively), so they come back under the same `Entity` and every component
  that refers to them still does. They are buried once no snapshot before their
  retirement can be loaded.
- The checksum covers the parts registered with a hash: the tick, the RNG, every
  simulated entity's `SimTransform`, health and hit points. It leaves out `Entity`
  values, which differ between peers.

## Rules for simulation code

1. **Register** every component and resource a tick changes (`roll_back_component`,
   `roll_back_resource`). Anything left out is not rolled back, and the sync test finds
   it as a checksum mismatch.
2. **Order by `SimId`, never by `Entity`.** `Entity` values depend on everything the app
   has spawned, menus, effects and sounds included, so they differ between peers.
   Looking an entity up by `Entity` is fine; sorting by it is not.
3. **Remove simulated entities with `retire()`**, not `despawn()`.
4. **Read messages in the simulation with `SimMessageReader`**, not `MessageReader`.
   Bevy keeps a reader's position in the system, where a snapshot can't reach it. A
   message written after its reader in the tick (say, a blast's `HurtHero`, read by the
   heroes next tick) would be lost after a load, and one from the abandoned future read
   in its place. `SimMessageReader` keeps what is still to be read in the snapshot.
5. **Nothing from outside the simulation:** no wall clock or frame time, no render
   state (`GlobalTransform`, cameras' frusta, the rig's pose), no `Local` state, no
   randomness but `SimRng`, and no hash-map order.
6. **Presentation on the tick** (effects, shot models) goes in systems that don't run
   while resimulating, and its entities have no `SimId`.
7. **Settings that change the simulation** (movement profile, guns, gun level, each
   player's camera setting) are agreed in the handshake and can't change during a
   session.

## Hazards found in the code (2026-11-04)

- `CollisionScene` is built in `SimSet::Hero` and read by `fire_guns` in `SimSet::World`
  on the next tick, so it is state carried between ticks and is snapshotted.
- Messages written after their reader in the tick: `HurtHero` from `grow_blasts`
  (read by `move_heroes`), and `Struck` and `Destroyed` from `strike_targets` and
  `grow_blasts` (read by `run_enemies`).
- Systems in one set with no order between them, such as `revive_dummies` and
  `grow_blasts` (both on `Destroyed`), run in either order under Bevy's multi-threaded
  executor. The tick runs on the single-threaded executor, whose order is fixed by the
  schedule.
- Effects in `FixedUpdate` share `FxRng` with `Update`, read the rendered camera, and
  spawn entities with `SimTransform`: presentation (rule 6).
- `choose_camera` and the pause menu's Moves and Camera choices change the simulation
  from `Update` (rule 7).

## Starting a session (`openrac`, `coop.rs`)

1. Both peers spawn the level with both heroes, in player order, so its state is the same.
   The tick's schedules are taken out of Bevy's fixed loop, so it waits at tick 0.
2. The guest says hello every 250 ms with its build, a digest of the rules (movement
   profile, guns, gun level) and its camera setting; the host welcomes it into slot 1, or
   refuses it with the reason (`rac_net::handshake`). Both share one UDP port with
   `ggrs`, each packet tagged with whose it is (`rac_net::link`).
3. Each starts a `ggrs` session with the other's address; `ggrs` synchronises, then the
   ticks run. A peer silent for 3 s is dropped, and the other plays on alone.

## How it was checked

- `rac_sim`'s tests: loading undoes changes, spawns and retirements; a message written
  after its reader survives a rollback once.
- `rac_net`'s tests, on a small simulation that spawns, retires, draws random numbers and
  sends a message across ticks: a sync test; two peers on a memory network with 3 steps
  of latency and 5% loss whose confirmed checksum matches a plain run's; a refused
  handshake; a dropped peer. Leaving a component unregistered fails the sync test.
- The app's `--sync-test` on the demo, 1500 ticks off-screen, walking, jumping, swinging
  and firing, with no mismatch; a host and a guest on one machine, each seeing the
  other's hero move.

## Known gaps

- Messages that presentation reads (`GunEvent`, `PickupSound`, `Destroyed` for the HUD)
  are written again when ticks are resimulated, so a rollback can repeat a sound or an
  effect. A presentation reader that skips resimulated ticks would fix it.
- No headless test runs the whole demo under the sync test yet; the app's `--sync-test`
  is the check.
- Join codes, port mapping and hole punching (D8) aren't in; the link binds IPv4 unless
  joining an IPv6 address.

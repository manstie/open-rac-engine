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
  once and isn't part of the simulation. Presentation that reads the simulation's
  messages shows each once ("Presentation of resimulated ticks").

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

## Presentation of resimulated ticks (`rac_sim::shown`)

- Effects, sounds and the HUD read messages the simulation writes: `GunEvent`,
  `PickupSound`, `CrateBlast`, `Destroyed`, `Afflicted`, `HelpAsked` and
  `HeroRespawned`. A rollback runs ticks again, and they write them again. Leaving out
  resimulated ticks' messages won't do: the other player's shot is first known when a
  rollback runs its tick with their real input.
- So presentation reads them with `ShownReader`, which shows a message the first time
  any run of its tick writes it. `add_shown_message` notes in `FixedFirst` where each
  run's messages begin, so the reader knows a message's tick and run from its id. It
  matches the message one for one against what earlier runs of the tick showed (two alike
  in a run are both shown).
- A resimulated tick that writes what it wrote before shows nothing new. One that writes
  something else, such as the other player's shot now known, shows it, as late as the
  rollback reached back. What an earlier run showed and the corrected one didn't write (a
  shot predicted that never happened) stays shown: the effect has played.
- Outside a session every tick runs once, and the reader reads every message, as
  `MessageReader` does. A reader remembers 64 ticks, more than a rollback reaches (8).

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
   while resimulating, and its entities have no `SimId`. Presentation reads the
   simulation's messages with `ShownReader`, and their types are added with
   `add_shown_message`.
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
2. Each side learns where it can be reached and makes its join code (`rac_net::code`):
   its public address, which public STUN servers report for the session's own socket
   (`rac_net::stun`, RFC 5389), then its address on its own network. The host first asks
   its router to forward its UDP port (UPnP IGD, `rac_net::upnp`, a two-hour lease given
   back on leaving the level or quitting; `--no-port-mapping` skips it), and puts the
   forwarded address first. The code is shown on the status line, logged, and copied to
   the clipboard.
3. The guest, given the host's code or address by `--join` or from the clipboard by the
   main menu's Co-op page, says hello every 250 ms, at every address in the code at once,
   with its build, a digest of the rules (movement profile, guns, gun level), its camera
   setting and a random id; the host welcomes it
   into slot 1, or refuses it with the reason (`rac_net::handshake`). The id makes hellos
   reaching the host by two paths (the LAN's and the NAT's) one guest's. Every packet
   shares the one UDP port with `ggrs`, tagged with whose it is, and STUN's untagged,
   known by its magic cookie (`rac_net::link`).
4. Through NATs (D8): a host whose NAT drops the guest's hellos is given the guest's code
   (pasted with Ctrl+V), and sends it punches every 250 ms until it arrives. Each NAT
   lets the other peer in once its own side has sent to it, which holds for the common
   cone NATs; two symmetric NATs need a forwarded port.
5. Each starts a `ggrs` session with the other's address; `ggrs` synchronises, then the
   ticks run. A peer silent for 3 s is dropped, and the other plays on alone.

## How it was checked

- `rac_sim`'s tests: loading undoes changes, spawns and retirements; a message written
  after its reader survives a rollback once; a shown reader shows what resimulated ticks
  write again once, two alike in a tick both, and what only the rerun writes.
- `rac_net`'s tests, on a small simulation that spawns, retires, draws random numbers and
  sends a message across ticks: a sync test; two peers on a memory network with 3 steps
  of latency and 5% loss whose confirmed checksum matches a plain run's; a refused
  handshake; a dropped peer. Leaving a component unregistered fails the sync test.
- `rac_demo`'s `tests/rollback.rs`, on the whole demo as the app builds it (HUD, rig and
  effects included, without a window or a renderer), two heroes with different camera
  settings scripted round the level: onto the hazard pad, through the yard's crates with
  the wrench, every gun fired at the dummies, the explosive crates, the quick-select ring,
  the vendor (a purchase), then the arena's enemies, locked on. Under the sync test it
  runs about 3800 ticks with UYA's moves and guns at V5, and again with DL's, with no
  mismatch; the test also checks that each hero fired every gun, crates broke, bolts were
  picked up, enemies died, both heroes were hurt and opened the ring, and that effects and
  sounds were given exactly the gun events, pickup sounds and crate blasts a plain run of
  the script gives them, though the sync test wrote each about eight times. Two peers on
  a memory network 3 steps slow losing 5% of packets, whose `Entity` values differ (each
  spawns its own HUD), agree on the confirmed checksum. A system that moves a hero by a
  count kept outside the snapshot is caught within 30 ticks, so the checks have teeth.
- `rac_net`'s tests of D8: the STUN reader on RFC 5769's sample answer; join codes read
  back, in either case, with look-alike letters, and catch a mistyped letter; two peers
  each behind a port-restricted cone NAT on the memory network learn their public
  addresses from a STUN server there, swap codes, meet and play 120 ticks agreeing with
  a plain run; a host behind one hears nobody until it punches.
- The app on one machine (2026-11-04): a host learned its public address from Google's
  STUN server through the owner's router and showed its code; a guest given the code
  (`--join CODE`) reached the host on its LAN address and they played with no desync; a
  code pasted into the host with Ctrl+V set it punching.
- The app's `--sync-test` on the demo, 1500 ticks off-screen, walking, jumping, swinging
  and firing, with no mismatch; a host and a guest on one machine, each seeing the
  other's hero move.

## Known gaps

- A shown message naming an entity (`Destroyed` for the HUD, `Afflicted` for the gun
  sounds) shows again if a rollback reaches back past the entity's spawn, which spawns it
  again under another `Entity`.
- No test reaches a hero's death and respawn under rollback: the demo level has no pit
  or deadly floor, and its enemies seldom kill him.
- Port mapping is UPnP IGD's only, and hasn't been tried on a real router (no test can
  stand in for one); NAT-PMP and PCP aren't in. A host whose router doesn't forward needs
  the guest's code, or a port forwarded by hand. Codes carry IPv4 unless the socket is
  IPv6, which it is only when every address the guest was given is.

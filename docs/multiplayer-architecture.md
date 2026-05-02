# Multiplayer Architecture — Phase 10

Two human players, each running their own colony, on a shared world. Builds on Phase 8 (grid map) + Phase 9.3 (AI bundle system) — both are prereqs.

**Status:** designed. Not started. Substrate prereqs are still being stabilized.

---

## The pitch

> A two-player ant-colony showdown where every worker is a real ACO agent, every pheromone trail decays in real time, and you're not commanding the colony — you're nudging it via beacons while a queen-LLM plots strategy against your friend's queen-LLM.

Two players each control one starter colony on opposite sides of a Phase 8 grid. They expand via daughter foundings, raid each other's outposts, and the first to wipe the opponent's last queen wins. Matches last hours-to-days at Timelapse scale.

---

## Why deterministic lockstep

The sim is already:
- **Deterministic** — `ChaCha8Rng` seeded from `env.seed`, no platform-dependent floats, all timing in tick units
- **Headless-capable** — colony_diag runs the same code path as the renderer
- **Already commands-driven** — beacons, possession, recruit/dismiss are all discrete events

Lockstep deterministic networking matches this perfectly:

1. Both clients run the **identical** simulation in lockstep
2. Per-tick, each client sends its **player commands** for that tick to the other client
3. Each client applies its own + the opponent's commands at the start of the next tick, then advances exactly 1 tick
4. The simulation state stays bit-identical on both clients without ever transmitting state — only commands

Bandwidth is trivial (commands are tiny, often empty). Server is optional (can be peer-to-peer or relay). Replays come for free (just record the command stream and replay).

Compared to authoritative server + state sync:
- ✅ ~99% less bandwidth (commands only, no state)
- ✅ Replays trivial (already deterministic)
- ✅ Cheating is hard (sim runs identical on both ends; desync = rejection)
- ❌ Requires absolute determinism (any platform-dependent float ruins it)
- ❌ Worst-case latency is the slower client's frame time (lockstep waits)

---

## Command model

```rust
pub enum PlayerCommand {
    PlaceBeacon { kind: BeaconKind, square: GridCoord, cell: (u32, u32), ticks: u32 },
    Possess { square: GridCoord, ant_id: u32 },
    SetAvatarHeading { rad: f32 },
    Recruit { radius: f32, max: u32 },
    DismissFollowers,
    AdjustCasteRatio { square: GridCoord, ratio: CasteRatio },
    AdjustBehaviorWeights { square: GridCoord, weights: BehaviorWeights },
    DigOrder { square: GridCoord, target: (u32, u32) },
    NuptialFlightOverride { square: GridCoord, target: GridCoord },
}

pub struct CommandFrame {
    pub player_id: u8,        // 0 or 1
    pub tick: u64,             // sim tick this command applies on
    pub commands: Vec<PlayerCommand>,
}
```

Per outer tick: each client sends its `CommandFrame` (often empty). Both clients apply both frames at the start of the next tick. Sim advances. Repeat.

---

## Determinism boundaries

The sim must be bit-identical on both ends. Audit list:

### Already deterministic ✅
- `ChaCha8Rng` per `Simulation` — seeded once
- All ant decisions go through `choose_direction(rng, ...)` with the per-substep deterministic seed (per-ant rayon parallelism uses pre-drawn deterministic seeds)
- Substep architecture — fixed N substeps per outer tick based on config, no real-time gating
- Pheromone math — pure f32 arithmetic, no `f32::sin/cos` differences across platforms (we use glam `Vec2.atan2` etc which are `libm`-based and consistent)

### Needs audit / fixing
- **f32 NaN/inf propagation** — could vary across platforms (x86 vs ARM). Add `debug_assert!(value.is_finite())` in hot paths.
- **HashMap iteration order** — used in spatial hash, blackboard, etc. Replace with `BTreeMap` or sorted-vec where iteration affects sim state.
- **rayon work-stealing order** — currently per-ant decisions write to a `Vec<(f32, Option<AntState>)>` with deterministic per-ant seeds, which is safe. But any rayon `for_each` that mutates shared state needs to be sorted post-collection. Audit needed.
- **Compiler optimization differences** — `-C target-cpu=native` could produce different f32 results. Lock to `-C target-cpu=x86-64-v2` (or equivalent ARM baseline) for shipped multiplayer builds.
- **bevy ECS iteration order** — render-side; doesn't affect sim. But any system that reads from sim state and writes back must be sim-side.

### Mandatory test: a desync canary
Add a CRC32 of `Simulation::ants` + `Simulation::colonies` + `topology.modules[*].pheromones` every N ticks. Both clients exchange the CRC; mismatch = desync detected, halt and dump state for diff.

---

## Connection model

### MVP: peer-to-peer over WebRTC
- Players exchange a join code (pre-shared text string)
- Direct WebRTC data channel via STUN, UDP-encapsulated
- ~50ms latency between west-coast US and Europe is fine; ~200ms is also fine because we run at 30Hz outer ticks
- Library: `web-rtc` Rust crate or `rust-quinn` for QUIC

### Better: optional relay server
- For NAT-traversal failures, a tiny relay server (rust + tokio, deployable as a docker container) ships commands between clients
- Stateless — relays never read commands, just forwards them
- Could be Matt-hosted on cnc-server or self-hostable by groups

### LAN
- mDNS discovery + direct TCP. Easy mode.

---

## Lockstep tick budget

Both clients must agree to advance to tick T+1 only after both have sent their CommandFrame for tick T. If a client falls behind:

- **2-tick grace** — fast client buffers up to 2 outgoing CommandFrames before pausing
- **Drop-in catch-up** — slow client's render runs at lower framerate while sim catches up (sim never falls behind real-time advancement when the player is alone — only matters when BOTH are at Timelapse and one is on a slow CPU)

If a client disconnects mid-game, the remaining client gets a "your opponent disconnected" prompt and can choose to keep playing solo (sim continues with the AI bundle taking over the disconnected slot) or end the match.

---

## Time scale handling

Both clients agree on the time scale at match start. **Timelapse 1440× is fine** because lockstep is per-outer-tick — substeps run inside a single tick exchange, so substep_count=24 doesn't bloat network traffic.

If players want to change time scale mid-match, it's a synchronized command both must acknowledge (or the higher of the two pre-agreed maximums).

---

## AI bundle integration

Phase 9.3's AI bundles plug in seamlessly:

- **Empty slot 1 + AI slot 2** = single-player keeper mode with bundled AI opponent
- **Player slot 1 + Player slot 2** = real PvP
- **AI slot 1 + AI slot 2** = AI vs AI spectator mode (what Phase 9.4 is)
- **Player slot 1 + Player slot 2 + spectator slots** = future feature, players can invite spectators with read-only sim view

The blackboard arbiter for an AI bundle outputs `Vec<PlayerCommand>` per outer tick — exactly the same type as the network command stream. So the lockstep code path is the same regardless of whether commands come from a remote player or a local AI bundle.

---

## Replay format

Replay = command stream (both players' CommandFrames) + initial config (seed, species, starter map). Replay file size: a 4-hour Timelapse match is ~14M outer ticks × maybe 100 bytes/frame avg = 1.4 GB raw. Compressed (most frames are empty): ~20-50 MB.

Store as `replays/<timestamp>_<player1>_vs_<player2>.acz` — custom binary format with a magic header + zstd-compressed command stream.

Replay viewer is just the sim running with the recorded command stream applied at the recorded ticks. Same code path as live multiplayer — no special "replay mode" sim.

---

## What doesn't fit yet

- **Synchronous voice chat** — out of scope for MVP. Players use Discord.
- **Anti-cheat beyond determinism** — desync detection is enough for casual play. Tournament-grade anti-cheat (verified clients, signed replays) is later.
- **Authoritative server with rollback** — the alternative to lockstep. Higher latency tolerance, more bandwidth, more complex. Skip until we have evidence lockstep doesn't work.
- **>2 players** — could go to 4 in a free-for-all on a larger grid. Lockstep scales reasonably to 4 players (4× the wait time on the slowest client). Skip until 2-player works.

---

## Implementation order

### Phase 10.1 — Determinism audit + canary
- Audit codebase for non-determinism (HashMap iteration, target-cpu flags, etc.)
- Implement CRC32 canary that fires every 100 outer ticks
- Test: run two headless instances of the sim with the same seed, compare CRCs every 100 ticks for 100k ticks. Must match exactly.
- 1-2 sessions.

### Phase 10.2 — Local lockstep harness
- Single-process, two `Simulation` instances running in lockstep
- Command stream from a hardcoded script (e.g. "place beacon at tick 5000")
- Verify both instances stay synced for full 2y-equivalent run
- 1 session.

### Phase 10.3 — Network transport (peer-to-peer WebRTC)
- Connect two browser-clients-style instances over WebRTC
- Latency-tolerant lockstep (the 2-tick grace buffer)
- Drop-in catch-up
- 1-2 sessions.

### Phase 10.4 — Match flow + UI
- Lobby (join code, species selection, scenario picker)
- Match-end screen (who won, replay download, rematch button)
- Replay viewer integration
- 1-2 sessions.

### Phase 10.5 — Spectator mode + replay sharing
- Read-only sim viewer for spectators joining mid-match
- Cloud-hosted replay sharing (probably free Cloudflare R2 storage)
- 1 session + ongoing infra

**Total Phase 10: 5-8 sessions** to ship 10.1-10.4. 10.5 ships incrementally.

---

## Cross-references

- `docs/phases-roadmap.md` — Phase 10 status
- `docs/phase-8-full-game-mode.md` — Phase 10 runs on the Phase 8 grid
- `docs/ai-architecture.md` — Phase 10 reuses Phase 9.3 bundle slots for command stream
- `docs/biology.md` — sim must be deterministic for replay parity, biology decisions go through the rng

---

## What this enables

- Real PvP ant colony battles
- Persistent matches that span hours-days at Timelapse
- Spectatable AI vs AI tournaments (already designed in Phase 9.4)
- Replay-based learning — watch your past matches, share with friends
- Eventually: ranked play, leaderboards, tournament mode (Phase 13 distribution polish)

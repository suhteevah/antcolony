# PvP Mode Design — Antcolony

**Status:** Draft 1, 2026-05-03
**Owner:** Matt Gates
**Reference template:** WC3 SimAnt / Ant Colony custom maps (canonical multiplayer ant-RTS)

---

## North Star

Two players each control a colony. They share a surface overworld with food, hazards, and contested chokepoints. Their nests are SEPARATE underground bases connected to the overworld through a small number of tunnels each. **The match ends when one player's queen dies.** Combat happens organically on the surface (skirmishes over food, scout encounters) and decisively underground (raid parties breaching enemy tunnels to reach the queen).

This is the WC3 SimAnt model with one twist: instead of pre-baked species-agnostic blocks, both players pick a SPECIES at match start, and the species choice meaningfully changes the playstyle (Camponotus = slow/durable carpenter; Formica rufa = mass raids; Aphaenogaster = fast solitary forager; etc.).

---

## Map Topology

```
       ┌────────────────────────────────────────────┐
       │                                            │
       │            SURFACE / OVERWORLD             │
       │   (shared, food sources, hazards, scouts) │
       │                                            │
       │  ┌─[T1]──[T2]──┐         ┌──[T3]──[T4]─┐ │
       │  │   Player 1   │         │   Player 2   │ │
       │  │  ENTRY ZONE  │         │  ENTRY ZONE  │ │
       │  └──────┬───────┘         └──────┬───────┘ │
       └─────────┼─────────────────────────┼────────┘
                 │                         │
        ┌────────▼─────────┐    ┌──────────▼──────┐
        │ PLAYER 1 NEST    │    │ PLAYER 2 NEST   │
        │ (private UG)     │    │ (private UG)    │
        │                  │    │                 │
        │  Tunnels         │    │  Tunnels        │
        │  └─Brood chamber │    │  └─Brood chamber│
        │  └─Food cache    │    │  └─Food cache   │
        │  └─QUEEN CHAMBER │    │  └─QUEEN CHAMBER│
        │     (deep)       │    │     (deep)      │
        └──────────────────┘    └─────────────────┘
```

**Three distinct regions per match:**

1. **Surface / overworld (shared).** Both teams' workers forage here. Food sources, predators, weather. Combat is opportunistic. Surface deaths drop corpses + alarm pheromone but don't end the match.
2. **Entry zone (per-team, semi-public).** The 2-4 tunnel mouths connecting surface → that team's UG. These are the chokepoints. Both teams know roughly where the enemy entry zone is. Defending these = "stop raids before they get in." Attacking = "force entry to push raids deeper."
3. **Private UG (per-team, hidden until breached).** The team's nest interior. Workers go here to deposit food, nurse brood, transit to/from the queen. Geometry is randomized per match seed. **This is where the queen lives** — at the deepest, most-protected position.

---

## Win Condition

**Kill the enemy queen.**

Secondary stalemate-breakers (in case of full timeout):
- Enemy adult population reduced to 0 (queen alive but defenseless)
- 30-minute hard cap → win goes to player with more total adults + brood

The MAIN goal is queen-kill; the stalemate breakers exist so matches resolve.

---

## What's Missing in the Current Sim

Inventory of what we have vs what PvP needs:

| Need | Current | Gap |
|---|---|---|
| Per-team underground modules | Sim has UG layer (P5 shipped) but only ONE UG instance shared between colonies | Need TWO UG modules + per-team scoping |
| Limited entry tunnels | Surface↔UG entrances spawn anywhere | Need per-team designated entry zones with 2-4 tunnel mouths |
| Queen-kill detection | `match_status()` returns `Won` when colony loses last queen ✓ | Already works, just needs reachable queen |
| Queen placement deep in nest | Queen spawns at colony origin | Need queen-chamber generation deep in UG |
| Asymmetric private UG layout | UG is generated procedurally but symmetric | Need per-team randomized layout with queen at the center/deepest |
| Squad/raid abstraction | Each ant is independent | Need a "raid party" higher-level command (player tells N workers to attack target X) |
| Player input layer | Only AI brains drive colonies | Need a `PlayerBrain` that takes actual player commands (place food, target raid, switch caste ratio, etc.) |
| Network multiplayer | Single-process only | Need lobby + lockstep or rollback netcode |
| Match timer + UI | Bench has tick cap, no UI | Need a match timer + score display |

---

## Phased Build

**Phase P1 — local hot-seat 2-colony PvP (no network, both players on same machine).**
- Two `PlayerBrain` instances driven by keyboard/mouse
- Per-team UG modules (2 instances, scoped)
- Per-team entry zones (designate 2-4 tunnels at match start)
- Queen placed at deepest UG position
- Win/lose UI on queen kill
- Single shared surface
- ~1 week of work

**Phase P2 — squad/raid commands.**
- Player can right-click a target → spawn a raid party of N workers heading there
- Workers in a raid follow a leader and ignore normal foraging FSM transitions
- Returns to FSM control when raid ends (target dead, returned to nest, or commanded to disband)
- ~3-4 days

**Phase P3 — species selection at match start.**
- Pre-match lobby UI: pick from 7 (now 8) species
- Species's biology + brain baseline applies to your colony
- Asymmetric matchups are part of the meta (Camponotus's wood gallery base vs Formica's open-mound base)
- ~3 days

**Phase P4 — netcode + matchmaking.**
- Pick: lockstep deterministic sim (smaller bandwidth, harder to debug) or rollback (more responsive)
- Recommend: lockstep with deterministic seeded RNG (sim is already mostly pure-functional + seeded)
- Steamworks integration for matchmaking (or self-hosted with a simple server)
- ~2-3 weeks

**Phase P5 — replay + spectator + tournament infrastructure.**
- Out of scope for v1; design with replay-friendly state in mind from P1

---

## Key Design Decisions That Need Input

These are the non-obvious calls that affect every downstream decision:

1. **Deterministic netcode vs rollback?** Lockstep is much easier to ship if our sim is genuinely pure-functional + seeded. We should verify by running the same seed twice on different machines and checking byte-equal final states. If it works → lockstep. If not → rollback (much harder, requires full state-snapshot system).

2. **Squad commands or pure macro AI?** WC3 SimAnt had explicit unit selection + raid orders. Pure-macro (player only sets caste ratios + behavior weights, AI handles tactics) is closer to our current brain architecture and would let our trained MLPs drive ant tactics while the player handles strategy. **Recommend: pure-macro for V1.** Cheaper to ship, our existing AI work pays off.

3. **Symmetric or asymmetric maps?** Both teams have IDENTICAL UG layouts, or random per-team layouts? Symmetric is simpler + more competitive-fair; asymmetric is more "biology-realistic" and matches species variation. **Recommend: symmetric for V1, asymmetric as a toggle later.**

4. **Resource model.** Surface food spawns randomly throughout the match, OR pre-seeded at match start? **Recommend: pre-seeded with periodic respawns**, so map control matters but late-game doesn't starve.

5. **Ant cap per team.** Hard cap to prevent the late-game performance explosion (we're targeted at 10k ants total). With 2 teams that's 5k each. Or: 8k each with shared 16k cap? **Recommend: 5k per team** for sim performance + tactical clarity.

6. **Queen vulnerability.** Should queens be passive (no combat ability) or active (queen can fight back)? Real biology: most queens are sedentary egg-layers, but Formica queens can defend themselves modestly. **Recommend: queens are passive but have very high HP (~500 vs worker 10).** Killing one requires sustained raid.

---

## Technical Architecture Notes

**Crate structure (no changes to existing for V1):**
- `antcolony-sim` — already has match-end detection, AI brain trait. PvP adds `PlayerBrain` here.
- `antcolony-game` — Bevy integration. PvP adds player-input system + match-state UI.
- `antcolony-render` — adds per-team team-color tints + queen-chamber highlight + UI overlays
- (eventually) `antcolony-netcode` — new crate for P4

**Multi-module topology:**
- Already supports multi-module sims (P5 layer traversal). PvP extends to:
  - Module 0: shared surface (overworld)
  - Module 1: player 1's private UG
  - Module 2: player 2's private UG
- Tunnel transitions handled by existing layer-traversal code

**State sync (for P1 hotseat, trivial):** both players read the same `Simulation` state. Their inputs go through a per-player `PlayerBrain` that emits `AiDecision`s.

**State sync (for P4 net):** lockstep — every N ticks both players exchange their `AiDecision`s, sim advances deterministically. Need to ensure all RNG paths are seeded and the sim is deterministic across machines (Windows vs Linux float behavior is the usual gotcha; consider fixed-point math for critical paths).

---

## Risk Inventory

| Risk | Likelihood | Mitigation |
|---|---|---|
| Sim non-determinism breaks lockstep | Medium | Audit all `f32`/`f64` ops + RNG before P4 |
| 5k+ ants per team tanks framerate | Medium | Already designed for 10k @ 30Hz; verify with real PvP traffic |
| Combat doesn't decisively resolve (current diagnosis: 70% timeouts on bench) | High | Tune combat damage + queen-vulnerability + arena size BEFORE shipping P1 |
| Players can't tell what their colony is doing | High (UI complexity) | Heavy investment in pheromone overlay + per-caste color coding + status panel |
| Species asymmetry breaks competitive balance | Medium | Ship P3 with symmetric matchups only initially; rebalance per-species after data |

---

## Why This Sequence

P1-P3 shipped in any order is a complete single-machine PvP game. P4 (netcode) is the gate to actual multiplayer over the internet, and it's the highest-risk piece — designing for it now (deterministic seeds, lockstep-friendly state) means P4 isn't a rewrite when we get there.

The combat-doesn't-resolve risk is the most important to address EARLY, since every other system assumes "matches end in queen-kill." If they don't, the game has no satisfying conclusions. The big-arena dominance audit currently running will tell us whether 64×64+50 ants makes combat actually matter; if YES, that's the V1 PvP arena scale. If NO, we need balance tuning before ANY of P1-P5 ships.

---

## Open Questions for Matt

1. **Species selection at match start, or just pick at lobby?** (P3 design)
2. **Single shared surface vs multiple "biome" surface tiles like WC3?** (multi-biome adds variety + species advantages but complicates map design)
3. **AI co-op / 1v1v1v1 / 2v2 / etc.** scope for V1?
4. **Replay system** — must-have for a competitive game; defer to P5 or build into P1 from day 1?
5. **Ranked vs casual** — do we want ELO/MMR from launch or just custom games?

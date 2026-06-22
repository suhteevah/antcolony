# Arena Nest Layer — Underground as a Competitive Strategic Dimension

**Date:** 2026-06-22
**Status:** Design spec (brainstorming → pending implementation plan)
**Composes with:** `pvp-mode-design.md` (win condition, map topology), cross-species spec (species dig multipliers), N-colony spec (multi-nest topology). Reference those; don't redefine.

---

## Problem

Combat in the current PvP arena is decided almost exclusively on the surface: armies
meet in the shared outworld, the larger colony wins, and the queen in the underground
module is simply a loot objective that gets deleted once the attacker floods in
unchecked. The underground module (`topology.rs::attach_underground`) exists, chambers
are pre-carved, but it has zero bearing on combat outcome because:

1. `combat_tick()` (`simulation.rs` ~line 1992) imposes **no per-terrain
   `max_simultaneous_attackers` cap** — a 200-ant swarm can all melee one defender
   simultaneously in a 1-tile tunnel exactly as if they were on open surface.
2. There is no `Blocking` / phragmosis ant state; no corridor-width–gated force
   multiplier.
3. `two_colony_arena` topology (`topology.rs`) gives each colony a private underground
   module, but raid pathing from the surface nest into the enemy underground does not
   model the narrow-entrance constraint that real nest architecture creates.

**Consequence:** colony size determines outcome. Defense depth and nest architecture
are irrelevant. The emergent strategy space that makes biological ant warfare
interesting — small specialized defenders holding a fortified nest against a large
generalist swarm — is absent.

---

## Goal

Make nest architecture a first-class strategic variable in PvP:

- Building a defensible tunnel layout should meaningfully extend a smaller colony's
  survival.
- Placing the queen deeper should be a costly but durable tradeoff (longer raid path,
  more chokepoints).
- A colony that loses the surface battle can still win by defending its nest until the
  attacker's foragers starve.
- The trainer's MlpBrain must develop a concept of "nest defensibility" — observable
  and trainable.

---

## Non-Goals

- Multiplayer netcode (deferred to PvP P4).
- Full procedural nest generation beyond the existing `attach_underground` pre-carved
  layout.
- CO₂/dig-priority pheromone fields (Phase C of `digging-design.md`, deferred).
- Per-species substrate materials (`SubstrateKind`) — referenced in `digging-design.md`
  Phase B, compose with that spec.
- N-colony generalization of raid pathing (compose with cross-species / N-colony spec).

---

## Biology Grounding

### B1 — Chokepoints flip Lanchester's Law

In open terrain, combat outcome scales with the **square** of force size (Lanchester
Square Law: each fighter reaches every enemy, so 10 vs 5 → advantage = 10² − 5² = 75
equivalent to 75% of the larger force). Narrow tunnels compress this to **Lanchester
Linear Law**: only the fighters at the front can engage, so advantage scales linearly
with force size and **individual fighting ability matters**.

> "Lymbery et al. (2023) found that 20 meat ants (*Iridomyrmex purpureus*) could defeat
> up to 200 Argentine ants (*Linepithema humile*) in narrow 10 mm corridors where only
> 1–3 attackers could engage simultaneously, despite the Argentine ants winning
> decisively in open-field encounters." — `02-combat-mechanics.md` §2

The transition exponent θ:
- Open surface: θ ≈ 1.04 (near-square law; size dominates)
- Complex terrain (10 mm corridors): θ ≈ 0.87 (near-linear law; quality matters)
- Single-file entrance: θ → 0.5 (pure linear; only front-rank fighter counts)

**Sim implication (B1):** `combat_tick()` must apply a `max_simultaneous_attackers`
cap that is a function of the terrain type at the combat tile. The cap determines
which Lanchester regime is active.

**Source:** `02-combat-mechanics.md` §2 — Lymbery et al. 2023; Champer & Schlenoff
2024 (chokepoint square→linear law transition).

---

### B2 — Terrain-gated simultaneous attacker caps

The biology docs already encode the correct values in the "Key sim levers" table
(`02-combat-mechanics.md` end of doc):

| Terrain type         | `max_simultaneous_attackers` | Effective law       |
|----------------------|------------------------------|---------------------|
| Surface (open)       | unlimited                    | near-square         |
| Underground tunnel   | 3                            | near-linear (θ≈0.87)|
| Nest entrance tile   | 1                            | pure linear (θ=0.5) |

These values reflect the biological corridor widths (10 mm corridor → 3 attackers
max; single-file entrance → 1). The sim grid uses abstract tile units; the mapping
is behavioral, not physical-scale.

**Source:** `02-combat-mechanics.md` §2, sim levers table.

---

### B3 — Defense-in-depth and chamber specialization

Raiding armies must traverse the full tube network to reach the queen. Each tube
junction is an independent chokepoint. A colony with a queen chamber at maximum
depth behind multiple tunnel bends creates **serial chokepoints** — each one resets
the attacker to the linear law, and the attacker must defeat defenders at each stage.

> "Nest architecture is a force-multiplier. Species that construct narrow-entrance
> nests (Temnothorax, Pheidole with phragmotic soldiers) systematically outperform
> expectations from colony size alone in interspecific encounters." —
> `05-raiding-usurpation-and-who-wins.md` Part III "Territorial wars"

Champer & Schlenoff 2024 (cited in `05-raiding-usurpation-and-who-wins.md`) framed
this as: "nest architecture as a force-multiplier that converts square-law combat to
linear-law combat."

---

### B4 — Queen never self-evacuates; evacuation is worker-triggered

Temnothorax data (`04-temnothorax-defense-dornhaus.md` §3): under internal threat,
the queen is **always passively carried** by workers to an alternate chamber or
nest exit — she never changes `AntState` herself. Queen aggression coefficient = 0.15
(near-zero fighting contribution).

> "Peripheral threat → withdrawal to deep chambers. Internal threat (raiders reached
> the brood chamber) → full evacuation; queen transported, never self-moves." —
> `04-temnothorax-defense-dornhaus.md` §3

**Sim implication (B4):** Queen has `AntState::Idle`; during a raid she may be
`AntState::Fleeing` carried by a worker escort, transitioning to a fallback
`QueenChamber` tile if one exists. This is the queen-evacuation subsystem (Phase 2
of this spec; deferred if complex).

---

### B5 — Queen-kill is gated on attacker dominance

`05-raiding-usurpation-and-who-wins.md` Finding 8: queen killing does not happen on
first contact. It is **gated** — attackers must first achieve spatial dominance over
the queen chamber (occupy it for N ticks) before the kill can trigger. Finding 9
adds a two-phase model (channel + CHC disguise acquisition) that is too biologically
detailed for V1 but motivates the gating gate.

**Sim implication (B5):** `match_status()` already checks `colony.queen_health == 0`.
The queen-kill should not be an instantaneous on-contact event; add an
`occupation_ticks` counter per colony queen chamber that must reach threshold before
queen begins taking melee damage. This prevents early queen-snipe before the nest
is truly overrun.

---

### B6 — Raid pathing follows pheromone into the nest

`05-raiding-usurpation-and-who-wins.md` Part I: scout ants first penetrate the
nest entrance and lay an alarm+recruitment pheromone trail that subsequent raiders
follow. The raid is not a blob that teleports to the queen — it flows through the
tube network along the existing pheromone gradient.

> "4-stage pipeline: scout → recruit → fight → haul. Scouts lay alarm pheromone into
> the enemy nest; the raiding column follows that gradient." —
> `05-raiding-usurpation-and-who-wins.md` Part I

**Sim implication (B6):** Raiding ants use the existing food-trail/alarm pheromone
FSM, but inside an enemy underground module. This requires the tube traversal code
(`underground_for_colony`, `surface_nest_for_colony` in `topology.rs`) to permit
inter-colony module travel for ants in `AntState::Fighting` or `AntState::Exploring`
when `alarm > threshold`.

---

### B7 — Lazy workers as reserve defenders

`04-temnothorax-defense-dornhaus.md` §1: 60.7% of colony workers are persistently
inactive, corpulent, and young. Under threat these are **replacement fighters** —
active workers are replaced by reserve workers (not vice versa). Inactivity is not
laziness; it is reserve capacity.

**Sim implication (B7):** Workers in `AntState::Idle` inside the underground module
should transition to `AntState::Fighting` when `alarm_at_position > idle_alarm_threshold`.
This is already partially supported by the alarm pheromone FSM; the key is that
underground-idle ants should respond to alarm at a lower threshold than surface ants
(they are the last-resort defenders).

---

## The Emergent Strategy Space

The four strategic decisions that underground nest architecture unlocks:

### S1 — Queen Depth vs. Worker Efficiency Tradeoff

Placing the queen in the deepest chamber maximizes her safety (more chokepoints
between her and raiders) but maximizes the commute distance for nurse workers
(`AntState::Nursing`) and for the egg-to-larva-to-adult pipeline
(`colony_economy_system`). A shallow queen is fast to nurse but easy to assassinate.

### S2 — Tunnel Topology as Defense Architecture

A colony with digging workers can extend the pre-carved tunnel layout (Phase A of
`digging-design.md`). Narrow branching tunnels force raiders into serial chokepoints.
A wide direct corridor to the queen chamber is fast for defenders but lets attackers
flood in. The colony brain must weigh these tradeoffs — both are locally rational
pheromone-gradient behaviors, but topology choice is a macro-level decision.

### S3 — Fortified Small Colony Holds Against Swarm

The core dynamic unlocked by B1+B2: a colony with 200 workers can hold a
single-entrance nest against a 2000-worker attacker indefinitely if the entrance
caps attackers at 1. The attacker's numerical advantage collapses to a 1v1 at the
entrance. The defender's **individual ant quality** (soldier caste, soldier_attack
= 3.0 vs worker_attack = 1.0) determines who holds the chokepoint.

This creates a **time-based siege** dynamic: the attacker must either outlast the
defender's food supply, find an alternate entrance, or dig a new tunnel (if digging
is enabled). The smaller colony can win a war of attrition from behind a choke.

### S4 — Raid Column Interdiction

Because raiders follow pheromone trails through tubes (B6), a defending colony can
disrupt the raid by:
- Depositing alarm pheromone in false directions (not yet implemented, future).
- Killing the scout that laid the trail before it returns (prevents recruitment column).
- Stationing soldiers at tube entrances to kill the first-in raider before it deposits
  a trail for the column.

---

## Architecture

### A1 — Terrain-gated `max_simultaneous_attackers`

**File:** `crates/antcolony-sim/src/simulation.rs` — `combat_tick()` (~line 1992)

**Current state:** no cap. All ants within `interaction_radius` of a target attack it.

**Change:** Before accumulating damage on a target ant, count how many enemy ants
are already dealing damage to it this tick. If `attackers_this_tick >= max_cap`, skip.

```rust
// Proposed addition to combat_tick() inner loop:
fn terrain_attacker_cap(terrain: &Terrain) -> usize {
    match terrain {
        Terrain::NestEntrance(_) => 1,
        Terrain::Empty if in_underground_module => 3,
        _ => usize::MAX,  // surface: unlimited
    }
}
```

The `in_underground_module` flag requires knowing which module a tile belongs to.
`Topology` already maps module bounds — add a `Topology::module_at(world_pos) ->
Option<ModuleId>` helper, then check `module.kind == ModuleKind::UndergroundNest`.

**Implementation note:** The cap is per-*target*, not per-tile. Each target ant
accumulates an `attackers_this_tick: u8` counter within the combat pass. Reset at
start of each tick. This is O(ants²) in the worst case but already is — the cap
doesn't change complexity, only exits the inner loop early.

**Files touched:** `simulation.rs::combat_tick()`, `topology.rs` (new
`module_at()` helper), `world.rs` (`Terrain` — no change needed, cap is derived
from module kind + terrain variant).

---

### A2 — Queen-Chamber Occupation Gating

**File:** `simulation.rs::combat_tick()` and `colony.rs`

Add `queen_chamber_occupation: [u32; MAX_COLONIES]` to `Simulation` (or per-colony
in `ColonyState`). Each tick: if enemy ants occupy the queen's tile (or the
`QueenChamber` terrain cell she is on), increment the counter; otherwise decay toward
0. Queen begins taking melee damage only when `queen_chamber_occupation >= OCCUPATION_THRESHOLD`
(proposed: 30 ticks = 1 sim-second at 30Hz FixedUpdate).

This gates the queen-kill on actual nest penetration + sustained occupation, not
first-frame contact (B5).

**Files touched:** `simulation.rs`, `colony.rs` (add `queen_chamber_occupation` field
or keep in simulation), `config.rs` (add `OCCUPATION_THRESHOLD` tunable).

---

### A3 — Raid Pathing Through Tubes (Inter-Colony Underground Travel)

**File:** `simulation.rs` FSM decision logic; `topology.rs`

The traversal helpers `Topology::underground_for_colony()` and
`Topology::surface_nest_for_colony()` are colony-scoped. Currently a red-colony ant
cannot be assigned to the black colony's underground module because there is no
routing for it.

**Change:** Ants in `AntState::Fighting` or alarm-triggered `AntState::Exploring`
that reach a `NestEntrance(colony_id)` tile where `colony_id != self.colony_id`
should enter the enemy underground module via the teleport mechanic (mirroring the
existing home-colony traversal in the Bevy game crate's systems).

Specifically:
- In the FSM decision pass, when a fighting/raiding ant steps onto
  `NestEntrance(enemy_id)`, assign it to the enemy module's coordinate space
  (existing teleport mechanic).
- Raiding ants then navigate via the existing alarm pheromone gradient (B6) —
  no special pathfinding needed; the pheromone trail laid by the scout already
  leads toward the queen chamber.

**Files touched:** `simulation.rs` (FSM decision for cross-colony NestEntrance
handling), `topology.rs` (verify `two_colony_arena` exposes enemy nest entrance
coordinates to all ants, not just home colony).

---

### A4 — Queen Depth Configuration in `two_colony_arena`

**File:** `topology.rs::two_colony_arena()` and `attach_underground()`

`attach_underground()` pre-carves a `QueenChamber` at `top_center` of the underground
module (~line 698 area). "Top" in underground coordinates is closest to the surface
entrance.

**Change:** Add a `queen_depth: QueenDepth` parameter to `attach_underground()`:

```rust
pub enum QueenDepth {
    Shallow,  // queen chamber 1 row below entrance (current default)
    Mid,      // queen chamber at 40% module depth
    Deep,     // queen chamber at 80% module depth (maximum protection)
}
```

For V1 PvP, both colonies use `QueenDepth::Deep` for symmetry. Future: let the brain
choose queen depth as a setup action, or expose it as a scenario parameter.

The tube carving in `attach_underground()` must connect entrance → (optional mid
chambers) → queen chamber as a continuous `carve_tunnel()` path. Multiple depth
tiers mean multiple serial chokepoints — each `carve_tunnel()` call creates one
linear corridor that is itself a chokepoint.

**Files touched:** `topology.rs::attach_underground()`, `two_colony_arena()`.

---

### A5 — Underground-Idle Alarm Response (Lazy Worker Defenders)

**File:** `simulation.rs` — FSM decision pass, `AntState::Idle` transition

Currently `AntState::Idle` ants wait for a transition trigger. Add:

```rust
// In decision_pass, for underground Idle ants:
if ant.state == AntState::Idle
    && ant.module == underground_module
    && pheromone.alarm_at(ant.pos) > cfg.ant.idle_alarm_wake_threshold
{
    ant.state = AntState::Fighting;
}
```

The `idle_alarm_wake_threshold` for underground ants should be lower than for surface
ants (they are the last-resort reserve, per B7). Add as `config.rs` TOML parameter
`ant.underground_idle_alarm_threshold` (proposed default: 0.3, vs surface threshold
1.5).

**Files touched:** `simulation.rs` (FSM), `config.rs` (new param), `simulation.toml`
(new TOML key).

---

### A6 — Topology Integration: `two_colony_arena` with Deep Underground

**File:** `topology.rs::two_colony_arena()`

Current topology: `black_nest (mod 0)` + `shared_outworld (mod 1)` + `red_nest (mod 2)`,
2 tubes. Each surface nest has a `NestEntrance` tile. The underground modules
(`attach_underground`) are attached separately in `Simulation::new_two_colony_with_topology`.

The missing wiring: `two_colony_arena` must be extended (or a new
`two_colony_arena_with_underground()` constructor added) that:

1. Attaches a private underground module per colony.
2. Connects each colony's surface nest entrance to its underground module via a tube.
3. Sets queen initial position to the deepest `QueenChamber` tile.
4. Exposes the enemy nest entrance coordinate to both colonies' world views (so
   raiding ants can locate the enemy entrance via pheromone gradient, not
   omniscience).

**Files touched:** `topology.rs` (new constructor or extended `two_colony_arena`),
`simulation.rs::new_two_colony_with_topology()` (wire deep underground + queen
placement), `world.rs` (`carve_chamber` + `carve_tunnel` already support this).

---

### A7 — `SimConfig` / TOML Parameters

All new tunable values go in `config.rs` and `assets/config/simulation.toml`.
New entries:

```toml
[combat]
underground_tunnel_max_attackers = 3      # Biology B2: Lymbery et al. 2023
nest_entrance_max_attackers = 1           # Biology B2: single-file entrance
queen_occupation_threshold = 30           # Biology B5: ticks before queen takes damage

[ant]
underground_idle_alarm_threshold = 0.3   # Biology B7: lazy worker wake threshold
surface_idle_alarm_threshold = 1.5       # (existing or new)

[colony]
queen_depth = "Deep"                     # QueenDepth enum variant
```

---

## Training / Eval Implications

### T1 — New Observations for MlpBrain

The current observation vector does not encode nest state. Add:

| Observation | Source | Rationale |
|---|---|---|
| `enemy_ants_in_my_underground` | spatial hash filtered by module | Raid detection |
| `my_ants_at_entrance` | spatial hash | Chokepoint defense density |
| `queen_occupation_ticks` | simulation state | Danger urgency signal |
| `queen_depth_normalized` | colony config | Tradeoff awareness |
| `tunnel_chokepoint_count` | topology | Defense depth |

These go in `crates/antcolony-trainer/src/obs.rs` (or equivalent observation
builder) as new fields. The MlpBrain hidden layers are wide enough (current
architecture from `project_rust_trainer.md` memory note) to absorb new inputs
without architectural change for V1.

### T2 — Reward Shaping for Nest Defense

Without reward signal, the brain will not learn to value chokepoint defense. Add
dense reward components:

- `+0.01` per tick a defender holds the entrance tile against an enemy (measured
  as: friendly ant at `NestEntrance(own_id)` while `enemy_ants_in_module > 0`).
- `-0.05` per tick `queen_occupation_ticks > 0` (urgency penalty for letting raiders
  reach the queen).
- Existing `queen_kill = +1.0` terminal reward unchanged.

These are **shaping rewards** — they must be zeroed out for eval/tournament runs
(only terminal reward for Elo purity). The `reward_mode: ShapingRewards | TerminalOnly`
enum already in the trainer config (`antcolony-trainer`) handles this.

### T3 — Eval: Does the Brain Learn Chokepoint Defense?

Add a headless eval scenario (`tests/headless_sim.rs` or `tests/nest_defense.rs`):

```
Scenario: 1 defender colony (100 workers, 5 soldiers at entrance) vs.
          1 attacker colony (1000 workers, 0 soldiers).
Expected: defender holds entrance for ≥ 200 ticks (linear law predicts this).
Failure mode: attacker floods in unrestricted (cap not working), queen dies <50 ticks.
```

This test validates the `max_simultaneous_attackers` implementation independently
of brain quality.

---

## Testing

| Test | File | Assert |
|---|---|---|
| Entrance cap enforced | `tests/nest_defense.rs` | At most 1 attacker deals damage per tick at `NestEntrance` tile |
| Tunnel cap enforced | `tests/nest_defense.rs` | At most 3 attackers deal damage per tick in underground tunnel tile |
| Queen-kill gated | `tests/nest_defense.rs` | Queen alive at tick 29 even when raiders occupy queen chamber (gating = 30) |
| 20-vs-200 choke scenario | `tests/nest_defense.rs` | 20 defenders at entrance hold against 200 attackers for ≥ 100 ticks |
| Raid pathing enters enemy UG | `tests/headless_sim.rs` | Raiding ant assigned to enemy underground module within 500 ticks of alarm spike |
| Lazy worker wake | `tests/headless_sim.rs` | Underground Idle ants transition to Fighting when alarm > 0.3 |

---

## Success Criteria

1. **Chokepoint math works:** A 1:10 colony-size disadvantage is survivable for ≥ 500
   ticks when the smaller colony holds a single-entrance nest (headless test passes).
2. **Queen-kill requires penetration:** Raiders cannot one-shot the queen on first
   contact; `queen_occupation_threshold` ticks of occupation required.
3. **Raid pathing is coherent:** Raiding ants enter enemy underground module via
   `NestEntrance` tile, not by teleporting directly to the queen.
4. **MlpBrain learns to defend:** After training with shaping rewards, brain assigns
   soldiers to entrance tiles at higher frequency when `enemy_ants_in_my_underground > 0`
   than when `== 0` (measurable via action-distribution logging).
5. **No regression:** Existing PvP win-rate distribution (benchmark from tournament
   yardstick `2026-06-20-pvp-tournament-yardstick-design.md`) does not shift by
   more than ±0.05 on the frozen archetype pool after the combat cap is added (the
   cap changes the surface game minimally since surface cap = unlimited).

---

## Implementation Phases

### Phase 1 — Chokepoint Math (sim core, no rendering required)

1. Add `Topology::module_at(WorldPos) -> Option<ModuleId>` to `topology.rs`.
2. Add `terrain_attacker_cap()` to `simulation.rs::combat_tick()`.
3. Add `queen_chamber_occupation` gating to `combat_tick()` and `match_status()`.
4. Add underground-idle alarm wake in FSM decision pass (`simulation.rs`).
5. Add TOML params to `config.rs` + `simulation.toml`.
6. Write `tests/nest_defense.rs` (all 6 tests above).

Deliverable: all 6 tests pass; existing tournament suite shows ≤ ±0.05 win-rate shift.

### Phase 2 — Raid Pathing (cross-colony underground travel)

1. Extend `topology.rs::two_colony_arena` (or new constructor) to attach private
   underground modules per colony with deep queen placement.
2. Wire cross-colony `NestEntrance` traversal in `simulation.rs` FSM.
3. Validate raid pathing headless test.

Deliverable: raiding ants coherently traverse enemy nest and pressure queen chamber.

### Phase 3 — Training Integration

1. Add new observation fields to `obs.rs` in trainer.
2. Add shaping reward components (gate on `reward_mode`).
3. Run 1 training round (150 iters) with underground observations; verify brain
   assigns soldiers to entrance at higher frequency under raid.

Deliverable: action-distribution log shows entrance-defense specialization.

### Phase 4 — Deep Underground Topology (deferred, compose with digging-design.md)

Wire Phase A of `digging-design.md` (actual dig FSM, multi-tick progress, pellet
carry) with the chokepoint combat system so dynamically dug tunnels inherit the
underground `max_simultaneous_attackers` cap. Queen-depth enum exposed to player.

---

## Open Questions

1. **Asymmetric map:** Should one colony get a shallower nest and the other deeper
   for asymmetric PvP scenarios? Or always symmetric for V1 tournament fairness?
   **Tentative:** symmetric for V1 (match `pvp-mode-design.md` recommendation).

2. **Phragmosis / Blocking state:** `02-combat-mechanics.md` §4 (Pheidole) describes
   soldiers using their heads to physically block tunnel entrances, making the
   attacker cap effectively 0 (attacker cannot pass until blocker dies). This is the
   extreme version of the chokepoint mechanic. Model as `AntState::Blocking` that
   sets the entrance cap to 0 for all enemies except those in melee with the blocker?
   Deferred — adds complexity, compose with combat spec.

3. **Alternative entrances:** Can the attacker dig a new entrance to bypass the
   defended chokepoint? Biologically yes (army ants do this). Requires Phase 4
   (digging). Deferred.

4. **Queen evacuation:** B4 describes passive queen transport by workers to a fallback
   chamber. Implementing this requires a worker-queen escort FSM and fallback chamber
   designation. High biological fidelity but adds significant FSM complexity. Deferred
   to Phase 2 or later; for V1 the queen stays at her chamber and gating (A2) is the
   defense.

5. **Raid vs. pheromone disruption:** B6 says raiders follow the scout's pheromone
   trail. If the scout is killed before returning, no trail is laid and the raiding
   column may not penetrate. Should killing the first raider at the entrance reset the
   raid? This is emergent from the existing pheromone/alarm system if the scout doesn't
   deposit a trail before dying — no special logic needed, but needs verification.

---

## Biology Citations Index

| ID | Claim | Source |
|---|---|---|
| B1 | Square→linear Lanchester law at chokepoints; θ=0.87 in 10mm corridors | `02-combat-mechanics.md` §2 — Lymbery et al. 2023; Champer & Schlenoff 2024 |
| B2 | Terrain-gated simultaneous attacker caps (unlimited/3/1) | `02-combat-mechanics.md` sim levers table |
| B3 | Nest architecture as force-multiplier; Temnothorax/Pheidole narrow-entrance advantage | `05-raiding-usurpation-and-who-wins.md` Part III |
| B4 | Queen never self-evacuates; passive transport by workers | `04-temnothorax-defense-dornhaus.md` §3 |
| B5 | Queen-kill gated on attacker dominance/occupation; two-phase model | `05-raiding-usurpation-and-who-wins.md` Findings 8–9 |
| B6 | Raid pathing follows scout-laid alarm pheromone into nest (4-stage pipeline) | `05-raiding-usurpation-and-who-wins.md` Part I |
| B7 | 60.7% inactive workers as reserve defenders; replaced by reserve under threat | `04-temnothorax-defense-dornhaus.md` §1 |

Total biology citations: **7**

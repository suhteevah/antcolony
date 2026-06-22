# N-Colony Free-For-All Arena — Design Spec

**Date:** 2026-06-22
**Status:** Draft — pending spec review → implementation plan
**Author:** Claude Sonnet 4.6
**Composes with:** `2026-06-22-arena-cross-species-design.md` (cross-species axis is separate;
this spec is independent of species choice — FFA works on same-species too).

---

## Problem

The existing PvP engine (`new_two_colony_with_topology`, `two_colony_arena`) hardcodes
exactly two colonies. Generalizing to N ≥ 3 introduces:

1. **Non-pairwise dynamics** — the strongest colony may not win; mutual attrition between #1
   and #3 can hand the match to #2 (the "kingmaker" effect). Pairwise-Elo cannot rate FFA
   outcomes correctly.
2. **Target-selection strategy space** — which enemy to attack is now a decision variable.
   Gang-up logic, de-facto non-coordinated alliances, and "don't-over-commit" reserve
   management become observable behaviors rather than fixed constraints.
3. **Arena layout for N nests** — two-colony east/west symmetry breaks at N ≥ 3; spatial
   arrangement becomes a design variable that affects competitive balance.
4. **Elimination bookkeeping** — mid-match colony deaths must be handled cleanly without
   stopping the sim; the remaining colonies continue until last-queen-standing.

This spec addresses all four. It does **not** address cross-species parameters (those live in
the cross-species spec); it does **not** redefine the Ladder League (which stays pairwise
1v1); it does **not** require network multiplayer.

---

## Goal

A headless-capable N-colony FFA mode (N = 3, 4 as priority; architecture scales to 8) where:

- Any mix of AI brains, human players, or archetypes can occupy each colony slot.
- The match ends when exactly one queen remains alive (last-queen-standing) or at a tick cap
  (score-by-adults tiebreak, matching existing `match_status()` stalemate logic).
- Match outcomes are recorded as **placements** (1st, 2nd, 3rd, …) not binary win/loss.
- The sim is byte-deterministic for replay parity (existing guarantee; must be preserved).

---

## Non-Goals (YAGNI)

- No networked multiplayer for FFA — local AI-vs-AI and single-human-vs-AI-x2 are the
  MVP use cases. Network FFA is a Phase 10 extension.
- No formal matchmaking or persistent FFA leaderboard for V1 — the goal is the arena
  + training infrastructure, not a shipping ranked mode.
- No explicit alliance or diplomacy mechanic — alliances must emerge from local incentives
  only (see §Biology). Coded alliances would defeat the point.
- No change to pairwise Ladder League or existing 1v1 tournament code paths (additive only).
- No terrain/biome asymmetry beyond colony placement (addressed in terrain spec).

---

## Biology Grounding

### Multi-party dominance hierarchies are real and non-transitive

In 9-species Maryland woodlot ant communities, species cluster into dominance tiers where
the #1 species does not always beat every subordinate species directly — encounter context
(temperature, resource type, local density) determines per-encounter outcomes. The
strongest species often suffers the most total attrition because it is targeted by multiple
subordinates simultaneously.
[cite: Fellers 1987, *Ecology* 68:1466 — discovery-dominance tradeoff; inter-tier hierarchy]

The 2025 Nelson & Mooney meta-analysis confirms that behavioral dominance (winning individual
encounters), numerical dominance (raw worker count), and ecological dominance (resource
capture rate) are **three independent axes** — a colony that ranks #1 on one axis may rank
#3 on another. In a 3-colony FFA, the worker-count leader, the best-fighter, and the fastest
forager can each win via a different path depending on map state.
[cite: Nelson & Mooney 2025, *Ecology and Evolution* 15(9): e72207]

### Mutual attrition: the "weak wins by watching" effect

Bertelsmeier et al. (2015) ran 4-invasive-species arenas in controlled conditions. Even
among species where the pairwise winner is unambiguous (W. auropunctata beats everyone
1v1), multi-party outcomes diverged: *Pheidole megacephala* won the largest fraction of
multi-party trials by arriving late at resources after dominant species had already fought
each other down. This is not "play defensively" — it is a structural consequence of the
discovery–dominance tradeoff: fast foragers accumulate resources while slow-but-dominant
species are still engaging each other.
[cite: Bertelsmeier et al. 2015, *Ecology and Evolution* 5(13):2673]

**Sim implication:** The AI strategy space in FFA should allow a colony to benefit from
NOT engaging the nearest enemy if the other pair of enemies is already in combat. This
emerges naturally if ants follow local pheromone and alarm signals rather than seeking the
nearest enemy by global knowledge. No explicit "wait" instruction is needed.

### The 10:1 annihilation threshold in mature-colony wars

Palmer (2004) showed that in African acacia ant guild wars, colony size is the #1 causal
predictor of competitive outcomes, with an approximate square-law (open combat: strength
∝ workers²). A 3:1 worker advantage is strongly decisive; a 10:1 advantage produces near-
certain annihilation before the defender can meaningfully respond.
[cite: Palmer 2004, *Animal Behaviour* 68:993 — rank-reversal experimental demonstration]

In a 3-colony FFA with colonies of sizes A > B > C (e.g., 1000:600:200), colony A can
annihilate C but doing so costs workers. If A fights C while B recovers, the post-fight
ratio A':B may be near 1:1, making the formerly safe #1 vulnerable. This is the structural
basis for the "don't over-commit" strategy — it emerges from the square-law attrition math
without any AI module needing to know it explicitly.
[cite: Palmer 2004; Champer & Schlenoff 2024, *J. Insect Science* 24(3):25 — Lanchester
square vs linear law]

### Spatial overdispersion and territory as competitive currency

Wiernasz & Cole (1995) showed that competing *Pogonomyrmex occidentalis* colonies self-thin
into overdispersed (regularly-spaced) distributions via competitive attrition: colonies
whose foraging territories overlap grow slower and die sooner. The competitive mechanism is
primarily **foraging territory overlap** — resource depletion in the contested zone — not
direct combat, especially in early-game.
[cite: Wiernasz & Cole 1995, *J. Animal Ecology* 64:519]

In a 3-colony arena, a centrally-placed colony has overlapping territory with all others;
corner-placed colonies have one direct neighbor. Starting position is therefore a
strategic variable in FFA, not just a cosmetic layout choice.
[cite: Uhey et al. 2025, *Environmental Entomology* 54(4):764 — 10.6–13.6 m nearest-
neighbor distances at 37 nests/ha; ~270-tile spacing at standard grid scale]

### Priority/residency effects: first-established colonies have a real edge

Gordon & Kulig (1996) found that founding queens preferentially settle near small young
neighbors rather than large mature ones — behavioral evidence that early establishment
creates a durable competitive advantage. Home-ground combat bonuses (fighting in own
ColonyScent territory) and preemptive interference (nest-entrance plugging) are real,
measurable, and independently documented.
[cite: Gordon & Kulig 1996, *Ecology* 77:2393; Gordon 1992, *Oecologia* 92:1]

FFA arenas with staggered starts (colonies enter at different ticks) produce richer
dynamics than simultaneous starts because early-established colonies get priority effects
that later arrivals must overcome. Both simultaneous and staggered modes should be
supported.

---

## The Emergent Strategy This Unlocks

### What cannot and must not be coded

- "Alliance" as a formal game mechanic would require explicit inter-colony communication
  that does not exist in real ants and would reduce the strategy space to coalition theory
  rather than emergent behavior.
- "Target the leader" as an AI instruction hardcodes a macro strategy that should emerge
  from local pheromone gradients and worker encounter rates.
- "Kingmaker" behavior as a special mode would be a design smell — it means the AI has
  a concept of "the match situation" that individual ants don't have.

### What will emerge from correct local rules

**De-facto non-coordinated alliances.** If colonies B and C each independently follow
their alarm pheromone toward colony A's workers (because A is the one attacking both),
B and C will converge on A's territory at the same time without communicating. From the
outside this looks like coordination. It is purely local pheromone response. This
matches the biology: there is no ant equivalent of a "truce signal"; what looks like
temporary coalitions is local threat avoidance. [cite: Bertelsmeier et al. 2015]

**Don't-over-commit.** A colony that has just suffered heavy losses will have low alarm
pheromone near its nest and high home-trail pheromone returning workers from the battle
zone. The economics of continuing an attack vs consolidating naturally flip as the
attacker's local worker density drops. This is the ACO equivalent of "quit while you're
ahead." No explicit threshold check needed.

**Gangup on the leader.** The discovery-dominance tradeoff means that the dominant colony
deposits the most trail pheromone on shared food sources. In a 3-colony world, both
subordinate colonies' scouts will frequently encounter the dominant colony's foragers first
(it's everywhere). This increases the alarm/home-trail feedback toward the dominant's
territory from two directions independently — a structural gangup.
[cite: Fellers 1987; Nelson & Mooney 2025]

**The honest claim about AI novelty.** The interesting AI question in FFA is not whether
an agent "knows" to gang up on the leader — it's whether a trained brain develops
target-selection behavior (differential aggression toward different colonies) that is
strategically sound, emergent from pheromone gradients, and not achievable by a
hard-coded heuristic. The brain currently sees `colony_scent` values per cell; in FFA
it must distinguish *which* enemy's scent it is following. That requires the per-colony
scent channel described below. Without per-colony scent disambiguation, no FFA-specific
strategy can emerge.

---

## Architecture

### Arena layout for N colonies

The existing `two_colony_arena` (module layout: black nest — outworld — red nest, linear)
does not generalize beyond N=2 without degenerating to a long chain where end-colonies
never meet center-colonies. N-colony FFA requires a **hub-and-spoke** or **ring** layout.

**Recommended: hub-and-spoke (N nests, 1 shared outworld)**

```
        Nest-0
           |
    Nest-3-+-Nest-1
           |
        Nest-2
```

For N=3: equilateral triangle of nests around a central outworld.
For N=4: square arrangement (or diamond) around a central outworld.
For N ≥ 5: regular polygon inscribed around the outworld.

Each nest module connects to the shared outworld via exactly one tube (matching the
existing 2-colony pattern). Outworld is a single shared module; all inter-colony combat
occurs there.

Port assignment for N=3 (equilateral, symmetric):

```
Nest-0 (north-center): tube connects to outworld top-center port
Nest-1 (south-east):   tube connects to outworld bottom-right port
Nest-2 (south-west):   tube connects to outworld bottom-left port
```

The tube length is the same for all nests in a symmetric layout (equal travel time from
nest to outworld — symmetric balance). Asymmetric layouts (e.g., one colony spawns closer
to the food cluster) are a named variant for map-design purposes, not the default.

**New function:** `Topology::n_colony_arena(n: usize, nest_dim, outworld_dim) -> Self`

This replaces the special-cased `two_colony_arena` for N ≥ 3. For N=2 it is
backward-compatible (produces same module/tube layout as `two_colony_arena`).

Module id convention: modules 0..N-1 are nest modules (colony i → nest module i);
module N is the shared outworld. Tube i connects nest module i to the outworld.

Total module count: N + 1. Total tube count: N.

### Generalizing `new_two_colony_with_topology` → `new_n_colony_with_topology`

**File:** `crates/antcolony-sim/src/simulation.rs`

Current signature:
```rust
pub fn new_two_colony_with_topology(
    config: SimConfig,
    mut topology: Topology,
    seed: u64,
    nest_black_module: ModuleId,
    nest_red_module: ModuleId,
) -> Self
```

Generalized signature:
```rust
pub fn new_n_colony_with_topology(
    config: SimConfig,
    mut topology: Topology,
    seed: u64,
    nest_modules: &[ModuleId],   // one per colony; len = N
    species_configs: &[SimConfig], // optional per-colony overrides; empty = use config for all
) -> Self
```

The body generalizes the two-colony body by iterating over `nest_modules`:
- For each `(colony_id, nest_module_id)` pair: create a `ColonyState::new(colony_id, ...)`,
  spawn initial ants on that module, set `is_ai_controlled = true` for colony_id ≥ 1
  (player is always colony 0 in single-player; all AI in AI-vs-AI mode).
- Avenger assignment: one avenger per colony ≥ 1 (same logic as current red avenger).
- `colonies: vec![ c_0, c_1, ..., c_N ]`.
- Logging: `tracing::info!(n_colonies = N, ants = total, ...)`.

The existing `new_two_colony_with_topology` becomes a thin wrapper:
```rust
pub fn new_two_colony_with_topology(
    config, topology, seed, nest_black_module, nest_red_module
) -> Self {
    Self::new_n_colony_with_topology(
        config, topology, seed,
        &[nest_black_module, nest_red_module],
        &[],
    )
}
```

This preserves the existing call sites (tests, benches, trainer) without changes.

### Per-colony scent channels

**Critical requirement.** Currently, `PheromoneLayer::ColonyScent` is a single f32 grid
shared by all colonies (indexed by `colony_id` in the deposit logic). For FFA to produce
emergent target-selection behavior, an ant on the outworld must be able to distinguish
"this cell smells like colony 1" from "this cell smells like colony 2."

Two implementation paths:

**Option A — Per-colony scent grids (recommended).**
The `PheromoneGrid` on the outworld module carries N separate `colony_scent_N` layers
(one per colony). Each colony's workers deposit only on their own layer. Each ant's
sensing loop reads all N layers, and the brain receives a per-colony scent vector rather
than a scalar.

Cost: N × outworld-grid-size f32 values. At N=4, outworld 64×64: 4 × 4096 × 4 bytes =
64 KB. Acceptable.

**Option B — Bitmask encoding.** A single f32 carries intensity; a separate u8 grid
carries `colony_id` of the strongest depositor. Cheaper memory, but loses multi-colony
overlap information in the same cell.

For N ≤ 4: go with Option A. For N ≥ 5 with large ouworlds, revisit.

**Brain input change.** The AI brain's observation vector currently includes a fixed
number of pheromone channels. For FFA, the number of colony-scent channels is N-1 (all
enemies). This means FFA brains are a different input shape than 1v1 brains.

Two resolution strategies:
- **Fixed max-colony input (pad with zeros).** Always allocate `MAX_COLONIES - 1` scent
  channels; unused channels are zero-padded. Allows reusing 1v1 brains in FFA (they will
  simply not see colonies they cannot sense). **Recommended for V1.**
- **Variable-width input.** N-colony brains are retrained for each N. More accurate,
  harder to manage.

### Spatial hash: per-colony scent already implicit

`crates/antcolony-sim/src/spatial.rs` stores `(i32, i32) → Vec<EntityId>`. Colony
membership is read from `ant.colony_id` during combat resolution. No changes needed here
for FFA — the spatial hash is colony-agnostic.

### Elimination and win conditions

**Current `match_status()` logic (simulation.rs):** checks whether any colony has
`queen_alive = false` or `adult_total() == 0` and returns `Won(colony_id)` or `Ongoing`.

For FFA this becomes:

```rust
pub enum MatchStatus {
    Ongoing,
    Eliminated(u8),          // a colony just died; match continues
    Finished(Vec<u8>),       // final placement list, winner first
}
```

`match_status()` now returns `MatchStatus`. On each tick after colony elimination,
the eliminated colony's ants are removed (or set to `state = Dead`) and its queen is
marked `queen_alive = false`. The sim continues with the remaining colonies.

**Win condition:** `Finished` is returned when `colonies.iter().filter(|c| c.queen_alive).count() == 1`.

**Tick-cap tiebreak:** if tick ≥ `config.combat.match_tick_cap`, all surviving colonies
are ranked by `adult_total() + brood.total()` (same as existing 2-colony stalemate
tiebreak, extended to N surviving colonies).

**Elimination handling rules:**
1. On colony C being eliminated: remove all ants with `colony_id == C` from `self.ants`.
   Any food they were carrying is dropped on the outworld tile at their last position.
2. The eliminated colony's nest module remains in the topology (tiles still exist) but
   is now unclaimed territory — other colonies' workers can enter and forage there.
3. Pheromone decay continues on the eliminated colony's nest tiles at normal rate.
   No special pheromone wipe; the scent will decay naturally.

**Placement recording:**
```rust
pub struct FfaResult {
    pub placements: Vec<(u8, u64)>, // (colony_id, elimination_tick); winner has largest tick
    pub final_tick: u64,
    pub seed: u64,
}
```

`placements[0]` is the first colony eliminated; `placements.last()` is the winner.

### Performance budget at N=4

Current target: 10,000 ants at 30Hz FixedUpdate (10k × N=2: 5k per colony).

FFA at N=4 with same total ant budget: 2,500 ants per colony. This fits within the
10k budget.

If the total budget needs to scale with N, the ant cap per colony must drop linearly:
`per_colony_cap = total_cap / N`. For N=3: ~3,333/colony; N=4: 2,500/colony.

The per-colony scent grid cost (Option A) at N=4 with a 128×128 outworld:
4 × 128 × 128 × 4 bytes = 262 KB. Within VRAM/RAM budget.

Pheromone diffusion cost scales as `(N+1)` modules (N nests + 1 outworld). Nests are
small (32×32 typical); outworld is the large module. No asymptotic performance concern
at N ≤ 8.

Rayon parallelism: ants are already processed in parallel by `rayon::par_iter`. N-colony
ants are an identical data layout — no structural change to the parallel loop.

---

## Training and Eval Implications

### Pairwise Elo does not apply to FFA

The existing tournament and Ladder League operate on pairwise 1v1 matches where
`result ∈ {0, 1}`. FFA produces placements `(1st, 2nd, 3rd, ...)` for N players.
Applying pairwise Elo to FFA by extracting all C(N, 2) pairwise results is technically
possible but methodologically unsound: the "pairwise result" in a FFA (colony A
eliminated colony B) is confounded by colony C's actions.

**Recommended FFA rating: placement-based TrueSkill or Openskill.**

TrueSkill (Herbrich et al. 2007) and its open-source variant OpenSkill are designed
for multi-player ranked outcomes. Each player has a `(mu, sigma)` Gaussian belief;
a placement is used directly to update beliefs. Both handle partial orderings (ties
at time of capture) and are robust to kingmaker effects.

For V1, a simpler approximation suffices: **placement-scored points** (N points for
1st, N-1 for 2nd, ..., 1 for last), averaged over a match set. This is not a proper
statistical rating but is interpretable and sufficient for early eval.

**The Ladder League stays pairwise 1v1.** FFA training is a separate loop that does
not replace the ladder. The two modes train different skills:
- 1v1: pure head-to-head optimization; cleaner signal; existing infrastructure works.
- FFA: target-selection, multi-threat management, resource priority under N-party contest.

### FFA eval harness design

```
FfaEval::run(
    brains: &[(BrainSpec, u8)],  // brain + colony_id
    n_matches: usize,
    seed_base: u64,
) -> FfaLeaderboard
```

Plays `n_matches` FFA rounds, rotating which colony slot each brain occupies (position
fairness: each brain starts in each slot `n_matches / N` times on average).

Output: `FfaLeaderboard` — mean placement per brain, with confidence intervals.

Minimum matches for reliable FFA eval: at minimum `50 × N` (vs `mpe=50` for 1v1).
N=3 → 150 matches. N=4 → 200 matches. At ~30s per FFA match on cnc (estimate; actual
to be measured), N=4, 200 matches = ~100 minutes. Acceptable overnight budget.

### Training signal for FFA brains

The pairwise terminal reward (`assets/reward/terminal.toml`) does not directly apply to
FFA because there is no single opponent. Two approaches:

**Option 1 — Pairwise projection.** Within each FFA match, compute a reward for each
colony as: `sum over all opponents j of (j's queen was killed before mine)`. This
reduces FFA to a sum of pairwise outcomes and allows reuse of the existing reward
shaping. Less expressive — the brain cannot learn "eliminate weakest first" from this
signal alone.

**Option 2 — Placement reward.** Reward = `(N - placement) / (N - 1)` mapped to
`[0, 1]`. 1st place → 1.0; last place → 0.0. Cleaner multi-party signal. Requires
retraining from scratch or warm-starting from a 1v1 brain.

**Recommendation:** start with Option 1 (projection) for warm-starting from existing
1v1 brains; develop Option 2 when FFA-specific behaviors start to plateau.

---

## Testing

### N-colony headless smoke test

**File:** `tests/n_colony_ffa_smoke.rs`

```rust
#[test]
fn three_colony_ffa_resolves() {
    let cfg = small_config();
    let topo = Topology::n_colony_arena(3, (24, 24), (64, 64));
    let mut sim = Simulation::new_n_colony_with_topology(cfg, topo, 42, &[0, 2, 4], &[]);
    for _ in 0..50_000 {
        sim.tick();
        if let MatchStatus::Finished(_) = sim.match_status() { break; }
    }
    assert!(matches!(sim.match_status(), MatchStatus::Finished(_)));
}

#[test]
fn four_colony_ffa_resolves() {
    // same pattern, N=4
}

#[test]
fn eliminated_colony_ants_removed() {
    // force-kill a colony's queen; assert ants with that colony_id are gone
}

#[test]
fn ffa_placement_ordering() {
    // first eliminated colony is last in placements; winner is first
}
```

### Determinism regression

FFA must stay byte-deterministic. Extend the existing determinism harness:
`examples/det_check.rs` → add a 3-colony FFA variant that runs the sim twice with
identical seed and compares final state CRC32. Must pass before any FFA code merges
to `main`.

### Per-colony scent channel unit test

```rust
#[test]
fn per_colony_scent_independent() {
    // set up 3-colony sim; deposit colony-0 scent at (10,10), colony-1 at (20,20)
    // read the outworld's pheromone grid at both cells
    // assert colony_0_scent[10*w+10] > 0.0 and colony_1_scent[10*w+10] == 0.0
}
```

---

## Files to Change

| File | Change |
|------|--------|
| `crates/antcolony-sim/src/topology.rs` | Add `fn n_colony_arena(n: usize, nest_dim, outworld_dim) -> Self`. Keep `two_colony_arena` as a compat alias. |
| `crates/antcolony-sim/src/simulation.rs` | Add `fn new_n_colony_with_topology(config, topology, seed, nest_modules, species_configs)`. Refactor `new_two_colony_with_topology` to delegate to it. Add `MatchStatus::Eliminated(u8)` variant; generalize `match_status()`. Add `FfaResult` struct. |
| `crates/antcolony-sim/src/pheromone.rs` | Add `colony_scent_n: Vec<Vec<f32>>` (or `[Vec<f32>; MAX_COLONIES]`) alongside existing `colony_scent`. Update deposit and diffusion to write to the colony-indexed layer. Update `PheromoneGrid::new` to allocate N layers. |
| `crates/antcolony-sim/src/colony.rs` | Confirm `ColonyState` has no hardcoded `colony_id == 0` or `colony_id == 1` checks; generalize any that exist. Add `eliminated_at_tick: Option<u64>`. |
| `crates/antcolony-trainer/src/eval.rs` | Add `FfaEval::run(...)` alongside existing `evaluate_h2h`. |
| `tests/n_colony_ffa_smoke.rs` | New file: smoke tests above. |
| `examples/det_check.rs` | Extend with FFA determinism test. |
| `scripts/run_ffa_bench.ps1` | New script: run N-colony FFA headless bench, report placements + wall-clock. |

---

## Success Criteria

1. `three_colony_ffa_resolves` passes in ≤ 50,000 ticks on a 64×64 outworld + 3 × 24×24 nests.
2. `four_colony_ffa_resolves` passes in ≤ 75,000 ticks.
3. FFA determinism: two runs with same seed produce identical `FfaResult` (same placements,
   same elimination ticks, same tick).
4. `new_two_colony_with_topology` call sites (all existing tests + benches + trainer) pass
   with no changes (backward-compat wrapper is sufficient).
5. The FFA eval harness runs 150 matches (N=3) in under 120 minutes on cnc P100.
6. Per-colony scent channels: a colony's workers deposit only on their own scent layer;
   confirmed by unit test.

---

## Open Questions

1. **Symmetric vs asymmetric nest placement.** Hub-and-spoke with equal tube lengths is
   the fairest default. Should we support a seeded-random placement variant (some colonies
   start closer to food clusters) as a named map type? This maps to the biology finding
   that founding queens choose placement strategically [Gordon & Kulig 1996], but it adds
   balance complexity.

2. **Brain input width.** Fixed max-colony input (pad with zeros) allows 1v1 brains to
   run in FFA without retraining. But the zero-padding means those brains have never
   learned to use the extra scent channels. Is the "plug 1v1 brain into FFA" evaluation
   useful as a baseline, or should FFA brains always be retrained from scratch?

3. **Eliminated colony territory.** After colony C is eliminated, its nest module is
   unclaimed. Should the surviving colonies be able to colonize (dig/nest) that module,
   or is it permanently neutral territory? Biology supports colonization [Finding 23,
   Tschinkel 1992 brood raiding]; game balance may prefer neutral-territory status for V1.

4. **FFA ant cap.** `per_colony_cap = 10000 / N` at fixed total budget means N=4 colonies
   fight with 2,500 ants each. Does that produce sufficiently decisive outcomes, or is the
   ant count too low to sustain interesting combat phases? The 1v1 bench data at 5,000/side
   is the closest reference; 2,500/side may need validation.

5. **Position fairness in training.** In a 3-colony FFA, the colony placed at the "corner"
   of the triangle has one tube exit facing a different direction than the "center" (if
   layout is non-symmetric). Does brain performance vary by slot? If so, training should
   rotate colony slot assignments per match (already addressed in eval harness design, but
   training loop also needs this).

6. **Rating system.** OpenSkill/TrueSkill vs placement-points for V1. TrueSkill is better
   statistics but requires a dependency. Placement-points are interpretable but not a
   proper Bayesian model. Decision deferred to implementation planning; flag for spec
   reviewer.

---

## Biology Citations Index

| # | Citation | Section used |
|---|----------|--------------|
| 1 | Fellers, J. H. (1987). *Ecology* 68:1466. | Multi-party dynamics, gangup emergence |
| 2 | Nelson & Mooney (2025). *Ecology and Evolution* 15(9):e72207. | Three independent dominance axes |
| 3 | Bertelsmeier et al. (2015). *Ecology and Evolution* 5(13):2673. | Mutual attrition, weak-wins-by-watching |
| 4 | Palmer (2004). *Animal Behaviour* 68:993. | 10:1 threshold, square-law, don't-over-commit |
| 5 | Champer & Schlenoff (2024). *J. Insect Science* 24(3):25. | Lanchester square vs linear law |
| 6 | Wiernasz & Cole (1995). *J. Animal Ecology* 64:519. | Overdispersion, territory as competitive currency |
| 7 | Uhey et al. (2025). *Environmental Entomology* 54(4):764. | 10.6–13.6 m nearest-neighbor spacing |
| 8 | Gordon & Kulig (1996). *Ecology* 77:2393. | Priority/residency effects, founding site selection |
| 9 | Gordon (1992). *Oecologia* 92:1. | Preemptive interference (nest-plugging) |
| 10 | Tschinkel (1992). *Ann. Ent. Soc. Am.* 85(5):638. | Eliminated colony territory reuse |

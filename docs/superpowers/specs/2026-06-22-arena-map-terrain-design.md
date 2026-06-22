# Spec: Arena Map / Terrain / Resource-Distribution Variety

**Status:** Draft  
**Date:** 2026-06-22  
**Author:** Claude Code (Sonnet 4.6)  
**Composes with:** `2026-06-21-ladder-league-design.md`, Phase 8 full-game-mode, Phase 4 multi-colony combat  
**Scope:** Procedural arena generation for 2-colony PvP matches — terrain variety, resource layout, asymmetric starts. Does NOT redesign the topology/module system or the phase-8 grid.

---

## 1. Problem

The current PvP arena is `two_colony_arena()` in `crates/antcolony-sim/src/topology.rs` (line ~`starter_formicarium_with_feeder` block): one fixed world grid, symmetric nests flanking a shared outworld, food clusters placed identically every match. Brains trained exclusively on this layout overfit to it. A brain that has only ever seen symmetric, open-ground, centered-food will:

- Fail on chokepoint maps where Lanchester linear-law math flips the force-ratio advantage.
- Fail on asymmetric food fields where the correct opener is aggressive trail-racing rather than efficient harvesting.
- Fail to exploit priority-establishment advantages when given a founding head-start.

This is a curriculum-diversity problem. The cheapest lever to expand the training distribution is **map variety**: parameterize what `two_colony_arena` builds, then sample from that parameter space per match.

---

## 2. Goal

Add a `ProceduralArena` generator that produces seeded, deterministic, varied arenas within the existing `WorldGrid + Topology` system. Each generated arena specifies:

1. **Terrain features** — obstacles and chokepoints that alter combat force ratios.
2. **Food cluster layout** — number, size, and asymmetry of food patches.
3. **Start positions and optional handicaps** — symmetric, asymmetric distance, or population/food handicap.

Downstream consumers: the training loop (sample a new arena each match or curriculum batch), the tournament gate (held-out map set for eval), and eventually Phase 8's biome system.

---

## 3. Non-Goals

- This spec does not change the Topology module system, tube bore physics, or underground dig layers.
- It does not add new Terrain variants (no water, lava, etc.) — uses existing `Obstacle`, `Food(u32)`, `Empty`, `NestEntrance`, `Solid`.
- It does not implement Phase 8's biome-grid or aggregate-fidelity sim.
- It does not add new rendering. Existing `WorldGrid` renders correctly with any layout.
- It does not alter `SimConfig` pheromone or combat numbers — terrain is a structural modifier, not a stat modifier.

---

## 4. Biology Grounding

### 4.1 Terrain Modifies the Lanchester Combat Law

> "In complex terrain (10mm corridors), the Lanchester exponent θ dropped from 1.0 (open) to 0.87 — meaning 20 large ants defeated 200 small ants." — `02-combat-mechanics.md`, §2 Group combat tactics, Lymbery et al. 2023 PNAS

In the current flat arena all combat is open-ground: the larger (or more numerous) force wins quadratically. Chokepoint tiles — a wall narrowing the shared zone to 2–4 cells — reduce effective simultaneous attackers from `interaction_radius`-wide swarms to a queue. This shifts θ toward 1.0 (linear law), giving qualitatively smaller armies the ability to hold a front. **Implementation implication:** terrain structures must be placed in the shared central zone, not near nests; width of the narrowest passage is the key parameter.

### 4.2 Resource Spatial Distribution Drives Trail-Racing Dynamics

> "Founding queens avoid zones with high colony-scent pheromone. Inter-colony competition = trail geometry race for seed patches; first-mover trail advantage." — `03-harvester-competition-cole-wiernasz.md`, §Ecosystem engineering and §Spatial competition

*Pogonomyrmex occidentalis* field data shows nests are overdispersed (regularly spaced, `nearest_neighbor_distance` 10.6–13.6m at 37 nests/ha). Competition resolves via who lays trail first, not who has more ants. **Implementation implication:** placing food clusters closer to one colony's nest, or in a ring around the arena rather than centered, creates a trail-racing game rather than a symmetric war of attrition. Asymmetric food layouts are therefore biologically grounded.

### 4.3 Priority Effects and Asymmetric Establishment

> "Priority effects — who arrives/establishes first — are the primary determinant in resource-discovery vs interference competition regimes." — `01-competition-and-displacement.md`, §Discovery-dominance tradeoff and §B. chinensis priority via cold-tolerance

Early colony establishment drives long-run competitive outcomes more than peak population. *B. chinensis* wins via cold-tolerance priority (active 4–6 weeks before competitors). **Implementation implication:** asymmetric start handicaps (one colony begins with fewer workers or less food) create legitimate biological scenarios and are the training seed for comeback strategies. A brain that can win from behind is more robust than one that only knows symmetric play.

### 4.4 Nest Overdispersion and Territory Spacing

> "Real-world nearest-neighbor distance 10.6–13.6m at 37 nests/ha; density-dependent mortality post-founding; founding queens avoid colony-scent zones." — `03-harvester-competition-cole-wiernasz.md`, §Spatial competition

In the sim, the nest-to-nest distance sets the effective territory radius. Compressing that distance increases contest intensity; stretching it creates a no-man's-land where food placement dominates over direct combat. **Implementation implication:** `nest_separation` should be a generator parameter, not fixed.

---

## 5. Emergent Strategies Unlocked

| Arena Type | New Strategy | Training Signal |
|---|---|---|
| Chokepoint center | Defend the pass; don't waste soldiers on wide flank | Learns to concentrate soldiers at narrow tiles |
| Asymmetric food (near colony A) | Colony B must commit raiders early before A entrenches | Aggression timing policy |
| Wide separation (open field) | Pheromone trail network planning; no early contact | Long-horizon foraging efficiency |
| Small arena (high density) | Permanent contact; alarm → combat loop dominant | Combat micro |
| Population handicap start | Comeback: prioritize food → growth over early raids | Delayed-gratification policy |

**Auto-curriculum seed:** As the brain pool strengthens, generator difficulty can be tuned by opponent. A brain that wins symmetric maps but loses asymmetric ones can be retrained on asymmetric batches. This is the POET (Paired Open-Ended Trailblazer) pattern — co-evolve environments with agents — as the natural extension once the generator is parameterized. The generator parameter vector is the "environment genome."

---

## 6. Architecture

### 6.1 New File: `crates/antcolony-sim/src/arena_gen.rs`

All procedural arena generation lives here. Exported types:

```rust
/// Full parameter vector for one arena instance.
/// Serializable for logging, replay, and curriculum scheduling.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArenaParams {
    pub seed: u64,
    pub width: usize,
    pub height: usize,
    pub nest_separation: NestSeparation,   // Close/Medium/Wide
    pub terrain_preset: TerrainPreset,     // Open/SingleChokepoint/DoubleChokepoint/ScatteredRocks
    pub food_layout: FoodLayout,           // Symmetric/AsymmetricNear/AsymmetricFar/Ring/Scattered
    pub start_handicap: StartHandicap,     // Symmetric / PopHandicap(f32) / FoodHandicap(f32)
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum NestSeparation { Close, Medium, Wide }

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum TerrainPreset {
    Open,               // Current behavior — no obstacles
    SingleChokepoint,   // One wall across center, 3-cell gap
    DoubleChokepoint,   // Two offset walls, zigzag path
    ScatteredRocks,     // N random 3×3 obstacle clusters in shared zone
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum FoodLayout {
    Symmetric,          // Current behavior — equal clusters each side
    AsymmetricNear,     // Cluster(s) biased toward colony 0's nest
    AsymmetricFar,      // Cluster(s) biased toward colony 1's nest
    Ring,               // Food around perimeter, contested center empty
    Scattered,          // N random clusters, seeded placement
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum StartHandicap {
    Symmetric,
    PopHandicap { colony: u8, fraction: f32 },   // colony starts with fraction of normal workers
    FoodHandicap { colony: u8, fraction: f32 },  // colony starts with fraction of normal food
}
```

Primary entry point:

```rust
pub fn build_arena(params: &ArenaParams) -> (WorldGrid, Topology, [ColonyStartState; 2])
```

This replaces direct calls to `two_colony_arena` at match setup. It calls the existing `Topology::two_colony_arena()` for the module/tube structure, then:
1. Runs `WorldGrid::new(params.width, params.height)` for terrain.
2. Places nest entrances per `NestSeparation`.
3. Applies `TerrainPreset` obstacle placement.
4. Applies `FoodLayout` cluster placement via `WorldGrid::place_food_cluster()`.
5. Returns per-colony `ColonyStartState { initial_workers, initial_food }` modified by `StartHandicap`.

Determinism: all RNG is `rand::rngs::SmallRng::seed_from_u64(params.seed)`. Same seed → same arena, guaranteed. Mirrors the existing `det_check` example in `crates/antcolony-sim`.

### 6.2 `ColonyStartState` (new, `arena_gen.rs`)

```rust
pub struct ColonyStartState {
    pub initial_workers: u32,
    pub initial_food: f32,
}
```

The match initializer reads these instead of hardcoded `SimConfig::colony.initial_workers` / `initial_food`. `Symmetric` handicap passes through `SimConfig` defaults unchanged.

### 6.3 Terrain Obstacle Placement (inside `build_arena`)

Obstacles are placed in the **shared outworld region** only (center 40% of world width). Nest-proximate zones (outer 30% each side) are never blocked — prevents founding traps.

**SingleChokepoint:** Horizontal wall at `y = height/2`, from `x = 0` to `x = width - gap_start` and `x = gap_end` to `x = width`. `gap_width = max(3, width / 20)`. Wall is 1 cell thick. Uses `WorldGrid::fill_rect` (new helper, 4 lines) with `Terrain::Obstacle`.

**DoubleChokepoint:** Two staggered horizontal walls at `y = height/3` and `y = 2*height/3`, gaps offset by `width/2`. Forces ants into a zigzag path.

**ScatteredRocks:** `N = rng.gen_range(3..=8)` placements of 3×3 `Obstacle` blocks in the shared zone, positions sampled uniformly, clamped to avoid nest areas and food-cluster centers.

### 6.4 Food Cluster Placement

Uses existing `WorldGrid::place_food_cluster(cx, cy, radius, amount)`.

**Symmetric:** same as current `two_colony_arena` default — one cluster each side at `(nest_x ± separation/3, height/2)`.

**AsymmetricNear (colony 0):** two clusters on colony 0's side, one on colony 1's. Total food identical.

**AsymmetricFar:** one cluster each side but colony 0's cluster is placed near the center (farther from colony 0's nest).

**Ring:** 8 clusters evenly spaced on a circle of `radius = min(width, height) * 0.35` centered at world center.

**Scattered:** N clusters (N from rng) at random positions in shared + contested zones.

### 6.5 Wiring Into Match Startup

Current call site in the trainer / match runner (likely `crates/antcolony-trainer/src/bin/` or equivalent match setup):

```rust
// OLD
let (world, topology) = Topology::two_colony_arena(w, h, ...);

// NEW
let params = ArenaParams { seed, ..ArenaParams::default() };  // or sampled from curriculum
let (world, topology, starts) = arena_gen::build_arena(&params);
// apply starts[0] and starts[1] to ColonyState initialization
```

### 6.6 `ArenaParams::default()`

Returns `Open` terrain, `Symmetric` food, `Symmetric` start — bit-identical to the current arena. Zero behavior change unless a non-default `ArenaParams` is passed.

### 6.7 Determinism Guarantee

`ArenaParams` implements `serde::Serialize`. Every match log must record the params that produced it. The trainer writes `params.seed` alongside reward/outcome rows. To replay any match: deserialize params, call `build_arena`, restore initial agent states. This composes with the existing `det_check` infrastructure (`crates/antcolony-sim`, `Sim is byte-deterministic` memory note).

---

## 7. Training and Evaluation Implications

### 7.1 Curriculum of Maps

Training loop: sample `ArenaParams` per match according to a curriculum schedule:

- **Phase A (baseline):** 100% `Open + Symmetric` — establish that the ladder-league brain isn't degraded.
- **Phase B (terrain mix):** 50% Open, 25% SingleChokepoint, 25% ScatteredRocks.
- **Phase C (full mix):** uniform over all `TerrainPreset × FoodLayout` combinations (16 combos), handicap disabled.
- **Phase D (handicap):** add `PopHandicap(0.7)` and `FoodHandicap(0.6)` variants at 20% frequency — requires comeback strategy.

Curriculum gate: advance from phase A→B→C→D when the brain's winrate on the current phase reaches ladder-league threshold (≥ SOTA winrate-vs-pool, per `2026-06-21-ladder-league-design.md` gate logic).

### 7.2 Held-Out Eval Map Set

Fix 20 arena seeds at project creation. These seeds are NEVER used during training. Eval runs the current brain against the fixed opponent pool on these 20 maps. Reported as `eval_winrate_heldout`. A brain that scores ≥ `eval_winrate_heldout` on held-out maps generalizes; one that regresses on held-out despite training gains is overfitting the training map distribution.

Seeds stored in `crates/antcolony-trainer/src/eval_maps.rs` as a `const [u64; 20]`.

### 7.3 POET Extension (future)

Once the generator is live, the natural extension is to co-evolve `ArenaParams` with the brain pool:
1. Mutate `ArenaParams` vectors (flip terrain preset, nudge food asymmetry, add handicap).
2. Test candidate environment against current SOTA — keep it if SOTA winrate drops below 0.65 (hard enough to be useful) and above 0.35 (not so hard it's unlearnable).
3. Add accepted environments to a rotating training pool.

This is POET (Stanley et al. 2019, "Paired Open-Ended Trailblazer"). The generator param vector is the environment genome. The infrastructure to support this is the serializable `ArenaParams` struct — no additional code changes required at that stage.

---

## 8. Concrete Files and Functions

| File | Change |
|---|---|
| `crates/antcolony-sim/src/arena_gen.rs` | **New.** `ArenaParams`, `TerrainPreset`, `FoodLayout`, `NestSeparation`, `StartHandicap`, `ColonyStartState`, `build_arena()` |
| `crates/antcolony-sim/src/world.rs` | **Add** `fill_rect(x0, y0, x1, y1, terrain)` helper (4 lines) — obstacle placement needs it |
| `crates/antcolony-sim/src/lib.rs` | **Add** `pub mod arena_gen;` |
| `crates/antcolony-trainer/src/eval_maps.rs` | **New.** `pub const EVAL_MAP_SEEDS: [u64; 20]` |
| `crates/antcolony-trainer/src/bin/ladder_league.rs` | **Modify** match setup to call `build_arena()` with sampled `ArenaParams`, log params with match outcome |
| `crates/antcolony-trainer/src/bin/pvp_tournament.rs` | **Modify** eval loop to run held-out map seeds, report `eval_winrate_heldout` |

---

## 9. Testing

### Unit Tests (`crates/antcolony-sim/src/arena_gen.rs`)

```rust
#[test]
fn build_arena_default_is_deterministic() {
    let p = ArenaParams { seed: 42, ..Default::default() };
    let (w1, _, _) = build_arena(&p);
    let (w2, _, _) = build_arena(&p);
    assert_eq!(w1.terrain, w2.terrain);
}

#[test]
fn chokepoint_reduces_passable_width() {
    let p = ArenaParams { seed: 0, terrain_preset: TerrainPreset::SingleChokepoint, ..Default::default() };
    let (world, _, _) = build_arena(&p);
    // Count passable cells at x = world.width / 2, assert ≤ gap_width
    let center_x = world.width / 2;
    let passable = (0..world.height).filter(|&y| world.terrain[y * world.width + center_x] != Terrain::Obstacle).count();
    assert!(passable <= world.width / 20 + 4);
}

#[test]
fn asymmetric_food_total_equals_symmetric() {
    // Build symmetric and asym-near arenas, sum food, assert equal totals
}

#[test]
fn pop_handicap_respected() {
    let p = ArenaParams { start_handicap: StartHandicap::PopHandicap { colony: 0, fraction: 0.5 }, ..Default::default() };
    let (_, _, starts) = build_arena(&p);
    let base = ArenaParams::default();
    let (_, _, base_starts) = build_arena(&base);
    assert_eq!(starts[0].initial_workers, base_starts[0].initial_workers / 2);
    assert_eq!(starts[1].initial_workers, base_starts[1].initial_workers);
}
```

### Integration Test (`tests/headless_sim.rs`)

Add: run 10 ticks on each of the 4 `TerrainPreset` variants, assert no panic, assert ant count > 0.

---

## 10. Success Criteria

- `build_arena(params)` is byte-deterministic: same seed → same `WorldGrid::terrain` vector.
- `ArenaParams::default()` produces a world grid bit-identical to the current `two_colony_arena` output.
- All 16 `TerrainPreset × FoodLayout` combinations build without panic.
- Chokepoint maps: passable width at world center ≤ `width / 20 + 4` cells.
- `eval_winrate_heldout` is reported alongside `eval_winrate_training` in every tournament run.
- After Phase B curriculum training: brain's `eval_winrate_heldout` does not regress vs Phase A baseline (generalization check).

---

## 11. Open Questions

1. **Chokepoint width tuning:** `width / 20` is a guess. Should be calibrated so that a 50-ant army cannot flank — needs a playtest or quick headless trial with ant interaction_radius (currently 1.2 cells from `config.rs`).

2. **ScatteredRocks collision with food clusters:** Need a minimum-distance check between rock placements and food cluster centers. Current `place_food_cluster` uses a circle; a post-placement pass that clears obstacles inside food circles may be simpler than pre-placement avoidance.

3. **Topology nest_separation vs world grid:** `NestSeparation` currently only moves nest entrance cells in the `WorldGrid`; `Topology::two_colony_arena` builds nest *modules* at fixed relative positions. If the tube length should also scale with separation, `Topology` needs a `tube_length` parameter. For now, keep tube length fixed; only shift the surface entrance cells.

4. **Handicap initialization:** `ColonyStartState.initial_workers` feeds into spawning_system. Confirm that the spawning path reads from `ColonyState` (which is initialized from `ColonyStartState`) rather than directly from `SimConfig::colony.initial_workers` before wiring. If not, a one-line change in `spawning.rs` is required.

5. **POET readiness gate:** mutation operators for `ArenaParams` (flip terrain preset, scale food clusters ±20%, add/remove handicap) are not specced here. Defer to a follow-on spec when the brain pool is strong enough to benefit.

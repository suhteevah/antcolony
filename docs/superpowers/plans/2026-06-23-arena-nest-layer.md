# Underground Nest Layer Arena Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the cross-species 1v1 arena a real **underground nest** — an `UndergroundNest` module with tunnels (carved `Empty` cells) + chambers and a **deep-placed queen** behind a single-file entrance and a tunnel chokepoint — so the already-built `terrain_attacker_cap` (entrance=1 / tunnel=3) actually bites and "defense-in-depth" becomes real. A small, defensive colony must be able to hold the chokepoints against a numerically/combat-superior swarm it would lose to on open ground. The research question this answers: **does adding the nest layer break the strict cross-species dominance hierarchy** (e.g. `formica_rufa` all-win, `temnothorax` all-lose, 0 cycles in the current flat arena) **into something intransitive?**

**Architecture:** This is *additive topology + raid-traversal wiring*, NOT new combat math. The combat caps, the venom matrix, the gated two-phase `usurp_tick`/`AntState::Usurping` queen-kill, the `colony_configs: Vec<ColonySimConfig>` per-colony slices, and `Simulation::new_two_colony_cross_species` are **already built and shipped** (cross-species plan `2026-06-22-arena-cross-species.md`). We add:

1. A new topology constructor `Topology::two_colony_nest_arena(...)` that builds the existing 3-module `two_colony_arena` **and** attaches a private `UndergroundNest` per colony (via a generalized `attach_underground_deep` with a `QueenDepth`), connecting surface entrance → tunnel → deep queen chamber.
2. A new sim constructor `Simulation::new_two_colony_nest_arena(...)` that delegates the colony/ant build to the existing `new_two_colony_cross_species` body, then **relocates each queen** into the deep underground `QueenChamber` (sets her `module_id` + `position`).
3. A widened **raid-descent gate** in the *existing* `surface_underground_traversal()` so ants in `Fighting`/`Usurping` (or alarm-woken explorers) can descend an *enemy* entrance and flow through the tunnel toward the deep queen on the existing alarm-pheromone gradient — no new pathfinder.
4. An underground-idle **lazy-worker alarm wake** in the FSM decision pass (B7).
5. A new `MatchEnv::new_cross_species_nest_arena(...)` + a `--nest` flag on `cross_species_matrix.rs` so the win-matrix can run in the nest arena, plus a focused **defensive-inversion** test.

The shared `SimConfig` (world/pheromone/hazards) and per-colony `ColonySimConfig` (ant/colony/combat) stay the source of truth. No `combat_tick`/`usurp_tick`/`terrain_attacker_cap`/`apply_colony`/`From<&SimConfig>` semantics change — they are reused verbatim.

**Tech Stack:** Rust (edition 2024, MSRV 1.85; `rand::Rng::gen` → `rng.r#gen::<…>()`). `antcolony-sim` (no Bevy; `tracing`, `serde`, `thiserror`, `rand`/`rand_chacha`, `glam::Vec2`). `antcolony-trainer` (`anyhow`, `rayon`). Headless deterministic tests via `cargo test`. Toolchain pinned to `stable-gnu` on kokonoe (MEMORY `project_toolchain`).

## Global Constraints

- **STRICT BACK-COMPAT / byte-identical determinism.** The existing **~225 sim + ~80 trainer tests** AND the cross-process / cross-rayon-thread-count determinism guarantee (MEMORY `project_determinism`) MUST stay byte-identical. The legacy `Topology::two_colony_arena((24,24),(32,32))` path and `Simulation::new_two_colony_with_topology` / `new_two_colony_cross_species` are **UNCHANGED** — the nest arena is a brand-new constructor pair (`two_colony_nest_arena` + `new_two_colony_nest_arena`) and a brand-new `MatchEnv::new_cross_species_nest_arena`. Nothing in the existing harness path (`new_cross_species` flat bench, `new_cross_species_arena` chokepoint) is touched.
- **Do NOT modify the semantics of** `terrain_attacker_cap`, `usurp_tick`, `combat_tick`, `apply_colony`, `Species::apply`, or `impl From<&SimConfig> for ColonySimConfig`. They are reused. The ONLY edit to a hot path is the additive descent-gate widening in `surface_underground_traversal()` and the additive idle-wake arm in the decision pass — both gated behind a config flag that defaults OFF so existing sims are byte-identical.
- **No `.unwrap()` / `.expect()` in simulation (non-test) code paths.** Use `Result`/`Option`. Existing constructors use `assert!(!topology.is_empty(), …)` — match that established pattern; do not add new unwraps in hot paths. (Tests may use `.expect()`.)
- **Verbose `tracing` everywhere.** Every new constructor, every queen relocation, every raid descent, every idle-wake, and the nest-arena harness path gets a structured `tracing` line (`info!`/`debug!`/`trace!` with fields). Never `println!` (except the harness binary's matrix dump, which mirrors the existing `cross_species_matrix.rs` `println!` reporting).
- **Edition 2024:** any `rand::Rng::gen` call is `rng.r#gen::<…>()`. No new RNG is required by this plan; if any is added it MUST be seeded from `self.tick` + a fixed salt (mirror `age_mortality_tick`), never drawn from `self.rng` in a way that perturbs the existing sequence.
- **Determinism discipline for new iteration:** the queen relocation, raid descent, and idle-wake passes MUST iterate ants in index order (the existing `for (i, ant) in self.ants.iter_mut().enumerate()` discipline). No `HashMap`-iteration-order may affect an outcome. (`combat_tick` already builds a `HashMap<ModuleId, SpatialHash>` for bucketing but resolves damage by sorted attacker index — do not break that.)
- **Additive-only on shared types.** New `AntConfig`/`ColonyConfig`/`CombatConfig` fields are `#[serde(default)]` with behavior-neutral defaults so existing TOMLs and `SimConfig::default()` produce identical sims. New `ColonyState` fields default to neutral. The raid-descent + idle-wake behavior is gated behind a new `combat.raid_underground_enabled: bool` (default `false`).
- **ECS / locality purity (CLAUDE.md rule 4):** individual ants still read only local state. Raiders descend because they are physically on an enemy `NestEntrance` cell while alarm is high — not because they are told where the queen is. The deep queen is reached by following the existing alarm gradient laid by the scout/column, not by global knowledge.
- **Flat-file rule (CLAUDE.md):** topology stays in `topology.rs`; combat/traversal/FSM stay in `simulation.rs`. No new module files in the sim crate except where a pure helper warrants it (none required here).

---

## Reconciliation: what the spec said is un-built but is ACTUALLY already built

The spec (`2026-06-22-arena-nest-layer-design.md`) predates the cross-species build. **FOLLOW THE REAL CODE.** Verified against current `simulation.rs` / `topology.rs` / `config.rs` / `env.rs` / `cross_species_matrix.rs`:

| Spec claim (treated as un-built) | Reality (already built — DO NOT re-plan) | Where |
|---|---|---|
| A1: `combat_tick()` has "no per-terrain cap"; add `terrain_attacker_cap()` | **BUILT.** `terrain_attacker_cap(module, gx, gy, &combat)` returns `max_simultaneous_attackers_entrance` on `NestEntrance(_)`, `_tunnel` if `module.kind == UndergroundNest`, else `_open`. `combat_tick` already sorts candidates by attacker index and caps. | `simulation.rs:2160-2181`, `2516-2537` |
| A1 helper: add `Topology::module_at(world_pos)` | **NOT NEEDED.** The real cap path resolves the module via the defender ant's `module_id` (`self.ants[j].module_id`), not a world-pos lookup. No `module_at` exists; do not add it. | `simulation.rs:2172-2177` |
| A2: add queen-chamber occupation gating | **BUILT (different model).** Queen-kill is gated by `usurp_tick`: a two-phase channel keyed on `usurp_gate_attacker_ratio` / `usurp_gate_defender_floor` / `usurp_channel_ticks`, with `AntState::Usurping` + `ColonyState.usurp_progress_ticks`. The queen ant is invulnerable until the channel completes (sets `queen_health = 0`). | `simulation.rs:2378-2514`; `colony.rs:293-297` |
| A7/CombatConfig caps + occupation are new TOML | **BUILT.** `CombatConfig` already has `max_simultaneous_attackers_{open,tunnel,entrance}` (default 255), `usurp_gate_attacker_ratio`, `usurp_gate_defender_floor`, `usurp_channel_ticks`, `usurp_corpse_to_killer_frac`, `venom_resistance`. | `config.rs` CombatConfig + Default |
| A3: cross-colony underground travel must be added; traversal "exists in the Bevy game crate's systems" | **PARTLY BUILT, in the SIM crate (not Bevy).** `surface_underground_traversal()` is sim-side (`simulation.rs:3137`), called each substep. BUT it only allows **descent when `state == Digging`** and ascent for soil-carriers / foraging workers. **Raiders in `Fighting` cannot descend, and there is no enemy-entrance descent at all** — descent only pairs a colony's OWN surface+underground. This is THE GAP for raid pathing. | `simulation.rs:3137-3239` |
| A4/A6: `two_colony_arena` must attach underground + place queen deep; add `QueenDepth` | **NOT built.** `two_colony_arena` builds 3 `TestTubeNest`/`Outworld` modules with **no** underground. `attach_underground` exists but is never called by any harness/cross-species path, and it carves the queen chamber **shallow** (`top-center`, 1 row below entrance). The queen is spawned at **surface nest center** (`Vec2(bw/2, bh/2)`, `module_id = nest_black_module`) — never relocated underground. THIS IS THE GAP. | `topology.rs:218-300`, `313-385`; `simulation.rs:264-328` |
| `MatchEnv::new_cross_species_arena` + matrix bin inject caps entrance=1/tunnel=3 | **BUILT.** `new_cross_species_arena` uses `two_colony_arena`; `cross_species_matrix.rs` sets `_open=255`, `_tunnel=3`, `_entrance=1`, and `usurp_corpse_to_killer_frac=0.5` for predators, per match. | `env.rs:156-199`; `cross_species_matrix.rs:98-108` |

**`ModuleKind::UndergroundNest` and `Terrain` tunnel/chamber geometry already EXIST** — they do NOT need creating:
- `ModuleKind::UndergroundNest` exists (`module.rs`).
- `Terrain` has `Empty`, `Solid`, `NestEntrance(u8)`, `Chamber(ChamberType)`, `SoilPile(u32)`, `Food(u32)`, `Obstacle`. **Tunnels are carved `Empty` cells** (there is no `Tunnel` variant — and the cap keys off `module.kind == UndergroundNest`, so any cell in an `UndergroundNest` that is not a `NestEntrance` is "tunnel" for the cap). `carve_tunnel((x,y),(x,y))` sets path cells to `Empty` (preserving `NestEntrance`/`Chamber`); `carve_chamber(cx,cy,half_w,half_h,kind)` sets a rectangle to `Chamber(kind)` (preserving `NestEntrance`).
- `ChamberType::{QueenChamber, BroodNursery, FoodStorage, Waste}` exists.
- `attach_underground(surface_nest_id, colony_id, w, h)` exists and `fill_solid()`s then carves the starter rooms/tunnels.

So the GAP is: **(a) attach undergrounds to the arena with a DEEP queen chamber behind a tunnel; (b) relocate the queens into them; (c) let raiders descend the enemy entrance and follow alarm to the deep queen; (d) wake underground idlers; (e) wire it into the win-matrix + a defensive-inversion test.** This plan addresses exactly that and reuses everything in the "BUILT" rows untouched.

---

## File Structure

| File | Created/Modified | Single responsibility |
|------|------------------|------------------------|
| `crates/antcolony-sim/src/config.rs` | Modify | Add `CombatConfig.raid_underground_enabled: bool` (`#[serde(default)]`, default `false`); add `AntConfig.underground_idle_alarm_threshold: f32` (default a high/neutral value so the wake is inert by default); add a `QueenDepth` enum re-exported from `topology` is NOT here — see topology. |
| `crates/antcolony-sim/src/topology.rs` | Modify | Add `pub enum QueenDepth { Shallow, Mid, Deep }`; add `attach_underground_deep(surface_nest_id, colony_id, w, h, depth) -> (ModuleId, (usize,usize))` returning the new module id + the deep `QueenChamber` grid cell; add `two_colony_nest_arena(nest_dim, outworld_dim, ug_dim, depth) -> Topology`. Legacy `two_colony_arena` + `attach_underground` UNCHANGED. |
| `crates/antcolony-sim/src/simulation.rs` | Modify | Add `new_two_colony_nest_arena(...)` (delegates colony/ant build, then relocates queens deep + records UG module ids); widen `surface_underground_traversal()` descent gate (enemy-entrance raid descent, gated on `raid_underground_enabled`); add underground-idle alarm-wake arm in the decision pass. No combat/usurp/cap edits. |
| `crates/antcolony-sim/src/colony.rs` | Modify | Add `ColonyState.underground_module: Option<ModuleId>` (`#[serde(default)]`) so the sim can find each colony's UG module for raid pairing + queen relocation. Default `None` (back-compat). |
| `crates/antcolony-sim/src/lib.rs` | Modify | Re-export `topology::QueenDepth`. |
| `crates/antcolony-trainer/src/env.rs` | Modify | Add `MatchEnv::new_cross_species_nest_arena(species_a, species_b, seed)` — mirrors `new_cross_species_arena` but builds the nest topology + enables raid. `new_cross_species` / `new_cross_species_arena` UNCHANGED. |
| `crates/antcolony-trainer/src/bin/cross_species_matrix.rs` | Modify | Add a `--nest` flag selecting `new_cross_species_nest_arena` (default stays `new_cross_species_arena`); print which arena ran. Cap-injection loop unchanged (it also sets `raid_underground_enabled = true` under `--nest`). |
| `crates/antcolony-sim/tests/nest_arena.rs` | **Create** | Headless integration tests: tunnel-cap fires in the UG module; deep queen is reachable only via the entrance+tunnel; raiders descend and pressure the deep queen; **defensive-inversion** test (small defender holds in the nest arena vs an attacker it loses to on the flat arena). |
| `scripts/run_cross_species_nest_matrix.ps1` | **Create** | PowerShell wrapper: `cargo run -p antcolony-trainer --bin cross_species_matrix -- --nest --mpe 50`. |

### Reused interfaces (exact, verified against current code)

```rust
// topology.rs
impl Topology {
    pub fn two_colony_arena(nest_dim: (usize,usize), outworld_dim: (usize,usize)) -> Self; // UNCHANGED
    pub fn attach_underground(&mut self, surface_nest_id: ModuleId, colony_id: u8, w: usize, h: usize) -> ModuleId; // UNCHANGED
    pub fn module(&self, id: ModuleId) -> &Module;            // linear scan, panics if missing
    pub fn module_mut(&mut self, id: ModuleId) -> &mut Module;
    pub fn try_module(&self, id: ModuleId) -> Option<&Module>;
    pub fn next_module_id(&self) -> ModuleId;
    pub fn underground_for_colony(&self, colony_id: u8) -> Option<ModuleId>;
    pub fn surface_nest_for_colony(&self, colony_id: u8) -> Option<ModuleId>;
    pub fn fit_bore_to_species(&mut self, worker_size_mm: f32, polymorphic: bool);
    pub fn is_empty(&self) -> bool;
}
pub type ModuleId = u16;

// world.rs (on Module.world: WorldGrid)
pub enum Terrain { Empty, Food(u32), Obstacle, NestEntrance(u8), Solid, Chamber(ChamberType), SoilPile(u32) }
pub enum ChamberType { QueenChamber, BroodNursery, FoodStorage, Waste }
impl WorldGrid {
    pub fn in_bounds(&self, x: i64, y: i64) -> bool;
    pub fn get(&self, x: usize, y: usize) -> Terrain;
    pub fn set(&mut self, x: usize, y: usize, t: Terrain);
    pub fn world_to_grid(&self, pos: Vec2) -> (i64, i64);     // (pos.x.floor(), pos.y.floor())
    pub fn grid_to_world(&self, x: usize, y: usize) -> Vec2;  // (x+0.5, y+0.5)
    pub fn place_nest(&mut self, x: usize, y: usize, colony_id: u8);
    pub fn set_nest_entrance(&mut self, x: usize, y: usize, colony_id: u8);
    pub fn find_nest_entrance(&self, colony_id: u8) -> Option<(usize, usize)>;
    pub fn fill_solid(&mut self);
    pub fn carve_chamber(&mut self, cx: usize, cy: usize, half_w: usize, half_h: usize, kind: ChamberType) -> u32;
    pub fn carve_tunnel(&mut self, from: (usize,usize), to: (usize,usize)) -> u32;
}

// module.rs
pub enum ModuleKind { TestTubeNest, Outworld, /*…*/ UndergroundNest }
pub struct Module { pub id: ModuleId, pub kind: ModuleKind, pub world: WorldGrid,
    pub formicarium_origin: Vec2, /*…*/ }
impl Module { pub fn width(&self) -> usize; pub fn height(&self) -> usize; }

// ant.rs
pub enum AntState { Idle, Exploring, FollowingTrail, PickingUpFood, ReturningHome, StoringFood,
    Fighting, Fleeing, Nursing, Digging, Diapause, NuptialFlight, Usurping }
pub enum AntCaste { Worker, Soldier, Queen, Breeder }
pub struct Ant { pub id: u32, pub position: Vec2, pub state: AntState, pub caste: AntCaste,
    pub colony_id: u8, pub health: f32, pub module_id: ModuleId, /*…*/ }
impl Ant { pub fn transition(&mut self, to: AntState); }

// simulation.rs
pub struct Simulation { pub config: SimConfig, pub colony_configs: Vec<ColonySimConfig>,
    pub topology: Topology, pub ants: Vec<Ant>, pub colonies: Vec<ColonyState>, pub tick: u64,
    pub rng: ChaCha8Rng, /*…*/ }
impl Simulation {
    pub fn new_two_colony_cross_species(world_pheromone_hazards: SimConfig,
        cfg_black: ColonySimConfig, cfg_red: ColonySimConfig, topology: Topology, seed: u64,
        nest_black_module: ModuleId, nest_red_module: ModuleId) -> Self;        // UNCHANGED
    pub fn colony_cfg(&self, colony_id: u8) -> &ColonySimConfig;
    pub fn match_status(&self) -> crate::ai::MatchStatus;
    pub fn run(&mut self, ticks: u64);
    pub fn tick(&mut self);                 // calls surface_underground_traversal + combat_tick + usurp_tick
    fn surface_underground_traversal(&mut self);  // private; descent gate widened in Task 4
    fn terrain_attacker_cap(&self, module: ModuleId, gx: i64, gy: i64, combat: &CombatConfig) -> u32; // UNCHANGED
}
pub fn spawn_initial_ants(config: &SimConfig, rng: &mut ChaCha8Rng, nest: Vec2, colony_id: u8,
    distribution: CasteRatio, id_offset: u32) -> Vec<Ant>;   // queen at `nest`, module_id 0

// colony.rs
pub struct ColonyState { pub id: u8, pub queen_health: f32, pub nest_entrance_positions: Vec<Vec2>,
    pub usurp_progress_ticks: u32, pub usurp_attacker_colony: Option<u8>, /*…*/ }
impl ColonyState { pub fn adult_total(&self) -> u32; }

// env.rs (trainer)
impl MatchEnv {
    pub fn new_cross_species_arena(species_a: &Species, species_b: &Species, seed: u64) -> Self; // UNCHANGED
    pub fn observe(&self, colony_id: u8) -> Option<ColonyAiState>;
    pub max_ticks: u64; pub sim: Simulation;
}

// ai.rs
pub enum MatchStatus { InProgress, Won { winner: u8, loser: u8, ended_at_tick: u64 }, Draw { ended_at_tick: u64 } }
```

**Key real-code facts (FOLLOW THESE, not the spec):**
1. The queen is spawned by `spawn_initial_ants` at `nest` (the surface nest center), `module_id` then set to `nest_black_module`/`nest_red_module` in `new_two_colony_cross_species`. To place her deep we relocate her **after** the existing ctor builds everything (set `module_id` to the UG module + `position` to the deep `QueenChamber` cell's `grid_to_world`).
2. `match_status` win = "enemy lost its last Queen OR has 0 adults". The deep queen sitting in the UG module still counts as colony N's queen (filtered by `colony_id`, not module). Relocating her does NOT change win detection.
3. `terrain_attacker_cap` keys off the **defender's** `module_id` + the defender cell's terrain. So a defender soldier standing on the UG `NestEntrance` cell caps incoming attackers at `_entrance`; a defender in a tunnel `Empty` cell of the `UndergroundNest` caps at `_tunnel`. The deep queen is reached only after attackers traverse those capped cells. No new cap code needed.
4. `surface_underground_traversal` pairs a colony's OWN surface+UG entrances. For raids we add an enemy-entrance descent: a non-queen ant on colony X's *surface* entrance, belonging to colony Y≠X, in `Fighting`/`Usurping`, descends into colony X's UG module. This is the ONLY hot-path behavioral addition, gated by `raid_underground_enabled`.
5. `carve_tunnel`/`carve_chamber` preserve `NestEntrance` and (for tunnel) `Chamber` cells, so we can carve the entrance first then carve through it safely.

---

### Task 1: `ColonyState.underground_module` field + `QueenDepth` enum + config flags (additive, behavior-neutral)

**Files:**
- Modify: `crates/antcolony-sim/src/colony.rs`
- Modify: `crates/antcolony-sim/src/topology.rs`
- Modify: `crates/antcolony-sim/src/config.rs`
- Modify: `crates/antcolony-sim/src/lib.rs`
- Test: in-module `#[cfg(test)]`

**Interfaces:**
```rust
// topology.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QueenDepth { Shallow, Mid, #[default] Deep }

// colony.rs — new field on ColonyState
pub underground_module: Option<crate::topology::ModuleId>,  // #[serde(default)] => None

// config.rs — new fields, behavior-neutral defaults
// CombatConfig:
pub raid_underground_enabled: bool,            // #[serde(default)] => false
// AntConfig:
pub underground_idle_alarm_threshold: f32,     // #[serde(default = "default_ug_idle_alarm")] => 1.0e9 (inert)
```

- [ ] **Step 1: Write the failing test**

In `config.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn nest_arena_config_flags_are_behavior_neutral_by_default() {
    let sc = SimConfig::default();
    // Raid descent is OFF by default => existing sims byte-identical.
    assert!(!sc.combat.raid_underground_enabled);
    // Idle-wake threshold defaults to an effectively-unreachable value so the
    // underground idle-wake arm never fires for existing configs.
    assert!(sc.ant.underground_idle_alarm_threshold >= 1.0e8);
}

#[test]
fn nest_arena_flags_round_trip_via_toml_with_neutral_defaults() {
    // A combat/ant block omitting the new keys keeps the neutral defaults.
    let toml = "[combat]\nworker_attack = 2.0\n[ant]\nspeed_worker = 3.0\n";
    let cfg = SimConfig::load_from_str(toml).expect("parse");
    assert_eq!(cfg.combat.worker_attack, 2.0);
    assert!(!cfg.combat.raid_underground_enabled);
    assert_eq!(cfg.ant.speed_worker, 3.0);
    assert!(cfg.ant.underground_idle_alarm_threshold >= 1.0e8);
}
```

In `colony.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn colony_state_underground_module_defaults_none() {
    let c = ColonyState::new(0, 100.0, glam::Vec2::new(5.0, 5.0));
    assert_eq!(c.underground_module, None);
}
```

In `topology.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn queen_depth_default_is_deep() {
    assert_eq!(QueenDepth::default(), QueenDepth::Deep);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-sim nest_arena_config_flags queen_depth_default colony_state_underground 2>&1 | tail -20`
Expected: FAIL — `raid_underground_enabled` / `underground_idle_alarm_threshold` / `underground_module` / `QueenDepth` not found (won't compile).

- [ ] **Step 3: Implement the additive fields + enum**

In `topology.rs`, add near the top (after the existing `pub type ModuleId` / imports):

```rust
/// Where in an `UndergroundNest` module the queen chamber is carved, relative
/// to the surface-aligned entrance. Deeper = more tunnel between the entrance
/// chokepoint and the queen (serial chokepoints; spec S1/B3). `Deep` is the
/// V1 arena default for symmetric PvP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QueenDepth {
    /// Queen chamber 1 row below the entrance (≈ legacy `attach_underground`).
    Shallow,
    /// Queen chamber at ~40% module depth.
    Mid,
    /// Queen chamber near the module floor (~80% depth) — maximum protection.
    #[default]
    Deep,
}
```

In `config.rs`, add to `CombatConfig` (additive):

```rust
    /// Arena nest layer (spec A3/B6): when true, enemy raiders in
    /// `Fighting`/`Usurping` may descend an enemy `NestEntrance` into that
    /// colony's `UndergroundNest` and flow toward the deep queen along the
    /// alarm gradient. `false` = legacy traversal (own-colony descent only),
    /// so all existing sims are byte-identical.
    #[serde(default)]
    pub raid_underground_enabled: bool,
```

Add to `CombatConfig`'s `Default` impl: `raid_underground_enabled: false,`.

Add to `AntConfig` (additive):

```rust
    /// Arena nest layer (spec A5/B7): underground `Idle` workers wake to
    /// `Fighting` when local alarm exceeds this. Defaults to an effectively
    /// unreachable value so the wake arm is INERT for existing configs (the
    /// nest arena lowers it, e.g. to 0.3). Distinct, lower than the surface
    /// alarm response by design (reserve "lazy worker" defenders).
    #[serde(default = "default_underground_idle_alarm")]
    pub underground_idle_alarm_threshold: f32,
```

Add the default fn near the other `default_*` fns in `config.rs`:

```rust
fn default_underground_idle_alarm() -> f32 { 1.0e9 }
```

Add to `AntConfig`'s `Default` impl: `underground_idle_alarm_threshold: default_underground_idle_alarm(),`.

In `colony.rs`, add to `ColonyState`:

```rust
    /// The colony's private `UndergroundNest` module, if the arena built one
    /// (nest-arena only). Used to pair raid descent + find the deep queen.
    /// `None` for surface-only / legacy sims (back-compat).
    #[serde(default)]
    pub underground_module: Option<crate::topology::ModuleId>,
```

Add to `ColonyState::new`'s struct literal: `underground_module: None,`.

In `lib.rs`, add to the re-exports near the other `pub use topology::…`:

```rust
pub use topology::QueenDepth;
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-sim nest_arena_config_flags queen_depth_default colony_state_underground nest_arena_flags_round_trip 2>&1 | tail -20`
Expected: PASS (4 tests).

- [ ] **Step 5: Confirm no existing config/colony test regressed**

Run: `cargo test -p antcolony-sim config:: colony:: 2>&1 | tail -20`
Expected: all green (existing `defaults_populated`, `partial_config_uses_defaults`, colony tests unchanged).

- [ ] **Step 6: Commit**

```bash
git add crates/antcolony-sim/src/config.rs crates/antcolony-sim/src/colony.rs crates/antcolony-sim/src/topology.rs crates/antcolony-sim/src/lib.rs
git commit -m "feat(sim): additive QueenDepth + ColonyState.underground_module + nest-arena config flags (neutral defaults)"
```

---

### Task 2: `Topology::attach_underground_deep` + `two_colony_nest_arena`

**Files:**
- Modify: `crates/antcolony-sim/src/topology.rs`
- Test: in-module `#[cfg(test)]`

**Interfaces:**
```rust
impl Topology {
    /// Like `attach_underground` but carves the `QueenChamber` at a depth set
    /// by `QueenDepth`, with a continuous entrance→(mid)→queen tunnel so the
    /// only path to the queen passes through the single-file entrance and the
    /// `UndergroundNest` tunnel cells (which the combat cap throttles).
    /// Returns the new module id AND the deep queen-chamber grid cell `(qx, qy)`
    /// so the sim ctor can relocate the queen there.
    pub fn attach_underground_deep(
        &mut self, surface_nest_id: ModuleId, colony_id: u8, w: usize, h: usize, depth: QueenDepth,
    ) -> (ModuleId, (usize, usize));

    /// Phase-5 arena: the existing `two_colony_arena` (black/outworld/red, ids
    /// 0/1/2) PLUS a private `UndergroundNest` per colony (ids 3 = black UG,
    /// 4 = red UG), each with a deep queen chamber. The surface↔underground
    /// pairing is by `find_nest_entrance(colony_id)` on both modules (the
    /// existing traversal contract). Returns the topology; the UG module ids
    /// are discoverable via `underground_for_colony`.
    pub fn two_colony_nest_arena(
        nest_dim: (usize, usize), outworld_dim: (usize, usize),
        ug_dim: (usize, usize), depth: QueenDepth,
    ) -> Self;
}
```

> **Geometry note.** In an `UndergroundNest`, grid `y` increases downward in our carving convention; the legacy `attach_underground` puts the entrance near `top = h-2` and carves *upward* (smaller y) toward the queen. We keep the entrance at `top` and place the queen chamber at a `queen_y` computed from `depth`. The entrance must remain the ONLY non-Solid boundary so attackers cannot bypass the tunnel. The tunnel is a single carved `Empty` column from the entrance to the queen chamber (with one mid bend for `Mid`/`Deep` so it is a genuine corridor, not a 1-cell room).

- [ ] **Step 1: Write the failing test**

In `topology.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn attach_underground_deep_places_queen_chamber_far_from_entrance() {
    use crate::module::ModuleKind;
    use crate::world::{ChamberType, Terrain};
    let mut topo = Topology::two_colony_arena((24, 24), (32, 32));
    let (ug_id, (qx, qy)) = topo.attach_underground_deep(0, 0, 24, 24, QueenDepth::Deep);

    let m = topo.module(ug_id);
    assert_eq!(m.kind, ModuleKind::UndergroundNest);
    // Queen chamber cell is a QueenChamber.
    assert_eq!(m.world.get(qx, qy), Terrain::Chamber(ChamberType::QueenChamber));
    // There is exactly one nest entrance for colony 0.
    let entrance = m.world.find_nest_entrance(0).expect("ug entrance");
    // Deep queen is genuinely far from the entrance (manhattan >= ~half the height).
    let dist = (entrance.0 as i64 - qx as i64).abs() + (entrance.1 as i64 - qy as i64).abs();
    assert!(dist >= (24 / 3) as i64, "deep queen should be far from entrance, dist={dist}");
}

#[test]
fn attach_underground_deep_shallow_is_near_entrance() {
    let mut topo = Topology::two_colony_arena((24, 24), (32, 32));
    let (ug_id, (qx, qy)) = topo.attach_underground_deep(0, 0, 24, 24, QueenDepth::Shallow);
    let m = topo.module(ug_id);
    let entrance = m.world.find_nest_entrance(0).expect("ug entrance");
    let dist = (entrance.0 as i64 - qx as i64).abs() + (entrance.1 as i64 - qy as i64).abs();
    assert!(dist <= 3, "shallow queen should be adjacent to entrance, dist={dist}");
}

#[test]
fn queen_reachable_from_entrance_through_empty_tunnel() {
    // The entrance→queen path must be a connected run of passable cells
    // (Empty / Chamber / NestEntrance), never blocked by Solid. We flood-fill
    // from the entrance over passable cells and require the queen cell reached.
    use crate::world::{ChamberType, Terrain};
    let mut topo = Topology::two_colony_arena((24, 24), (32, 32));
    let (ug_id, (qx, qy)) = topo.attach_underground_deep(0, 0, 24, 24, QueenDepth::Deep);
    let m = topo.module(ug_id);
    let (ex, ey) = m.world.find_nest_entrance(0).expect("entrance");

    let passable = |t: Terrain| matches!(
        t, Terrain::Empty | Terrain::NestEntrance(_) | Terrain::Chamber(_) | Terrain::SoilPile(_) | Terrain::Food(_)
    );
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![(ex, ey)];
    while let Some((x, y)) = stack.pop() {
        if !seen.insert((x, y)) { continue; }
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if !m.world.in_bounds(nx, ny) { continue; }
            let (nx, ny) = (nx as usize, ny as usize);
            if passable(m.world.get(nx, ny)) { stack.push((nx, ny)); }
        }
    }
    assert!(seen.contains(&(qx, qy)), "deep queen chamber must be reachable from the entrance");
    assert_eq!(m.world.get(qx, qy), Terrain::Chamber(ChamberType::QueenChamber));
}

#[test]
fn two_colony_nest_arena_has_two_underground_modules() {
    let topo = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
    // 3 surface modules + 2 underground.
    assert_eq!(topo.len(), 5);
    let black_ug = topo.underground_for_colony(0).expect("black ug");
    let red_ug = topo.underground_for_colony(1).expect("red ug");
    assert_ne!(black_ug, red_ug);
    use crate::module::ModuleKind;
    assert_eq!(topo.module(black_ug).kind, ModuleKind::UndergroundNest);
    assert_eq!(topo.module(red_ug).kind, ModuleKind::UndergroundNest);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-sim attach_underground_deep two_colony_nest_arena queen_reachable 2>&1 | tail -25`
Expected: FAIL — `attach_underground_deep` / `two_colony_nest_arena` not found.

- [ ] **Step 3: Implement `attach_underground_deep` + `two_colony_nest_arena`**

Append to `impl Topology` in `topology.rs` (do NOT touch the existing `attach_underground` / `two_colony_arena`):

```rust
/// Like `attach_underground`, but the `QueenChamber` is carved at a depth
/// set by `depth`, behind a single-file entrance and a tunnel corridor, so
/// the combat cap (entrance=1, tunnel=N) gates any assault on the queen.
/// Returns `(new_module_id, queen_chamber_grid_cell)`.
pub fn attach_underground_deep(
    &mut self,
    surface_nest_id: ModuleId,
    colony_id: u8,
    w: usize,
    h: usize,
    depth: QueenDepth,
) -> (ModuleId, (usize, usize)) {
    use crate::world::ChamberType;

    let surface = self.module(surface_nest_id);
    let origin = Vec2::new(
        surface.formicarium_origin.x,
        surface.formicarium_origin.y - h as f32 - 20.0,
    );
    let id = self.next_module_id();
    let label = format!("Underground-deep (colony {colony_id})");
    let mut module = Module::new(id, ModuleKind::UndergroundNest, w, h, origin, label);
    module.world.fill_solid();

    let cx = (w / 2).clamp(1, w.saturating_sub(2));
    // Entrance near the module top (surface-aligned), matching the legacy
    // `attach_underground` convention so the traversal pairing works.
    let entrance_y = h.saturating_sub(2);
    module.world.set_nest_entrance(cx, entrance_y, colony_id);

    // Queen depth: distance (in rows) BELOW the entrance toward the floor.
    // (entrance_y is near the bottom; "deeper" = smaller y, toward the top
    // of the module grid — the legacy carve direction.)
    let span = entrance_y.saturating_sub(2); // keep ≥1 row margin from y=0..1
    let queen_y = match depth {
        QueenDepth::Shallow => entrance_y.saturating_sub(1),
        QueenDepth::Mid => entrance_y.saturating_sub((span as f32 * 0.4) as usize).max(2),
        QueenDepth::Deep => entrance_y.saturating_sub((span as f32 * 0.8) as usize).max(2),
    };

    // Carve the queen chamber (1×1 half-extent => 3×3) at (cx, queen_y).
    module.world.carve_chamber(cx, queen_y, 1, 1, ChamberType::QueenChamber);

    // Carve a continuous corridor entrance → queen. For Mid/Deep, route
    // through a single mid-point bend so the corridor is a genuine tunnel
    // (multiple `UndergroundNest` Empty cells -> tunnel cap bites), not a
    // 1-cell adjacency. carve_tunnel sets path cells to Empty but preserves
    // the NestEntrance + the QueenChamber it touches.
    match depth {
        QueenDepth::Shallow => {
            module.world.carve_tunnel((cx, entrance_y), (cx, queen_y));
        }
        QueenDepth::Mid | QueenDepth::Deep => {
            let mid_y = (entrance_y + queen_y) / 2;
            let bend_x = cx.saturating_sub(w / 6).max(1);
            module.world.carve_tunnel((cx, entrance_y), (cx, mid_y));
            module.world.carve_tunnel((cx, mid_y), (bend_x, mid_y));
            module.world.carve_tunnel((bend_x, mid_y), (bend_x, queen_y));
            module.world.carve_tunnel((bend_x, queen_y), (cx, queen_y));
        }
    }

    // A brood nursery + food store one bend off the corridor so nurse/forage
    // economy has somewhere to go (does not open a second entrance).
    let nursery_y = (queen_y + 1).min(entrance_y.saturating_sub(1));
    module.world.carve_chamber(cx, nursery_y, 1, 1, ChamberType::BroodNursery);
    module.world.carve_tunnel((cx, queen_y), (cx, nursery_y));

    tracing::info!(
        id, surface_nest_id, colony_id, w, h, depth = ?depth,
        queen_cell = ?(cx, queen_y), entrance_cell = ?(cx, entrance_y),
        "Topology::attach_underground_deep"
    );
    self.modules.push(module);
    (id, (cx, queen_y))
}

/// Phase-5 arena topology: `two_colony_arena` + a private deep `UndergroundNest`
/// per colony. Surface ids stay 0 (black nest) / 1 (outworld) / 2 (red nest);
/// underground ids are assigned by `attach_underground_deep` (3 = black UG,
/// 4 = red UG with the current id allocator). The deep queen chambers are
/// reachable only through each nest's single-file entrance + tunnel.
pub fn two_colony_nest_arena(
    nest_dim: (usize, usize),
    outworld_dim: (usize, usize),
    ug_dim: (usize, usize),
    depth: QueenDepth,
) -> Self {
    let mut topo = Self::two_colony_arena(nest_dim, outworld_dim);
    let (uw, uh) = ug_dim;
    // Black colony 0 surface nest is module 0; red colony 1 surface nest is module 2.
    let (black_ug, black_q) = topo.attach_underground_deep(0, 0, uw, uh, depth);
    let (red_ug, red_q) = topo.attach_underground_deep(2, 1, uw, uh, depth);
    tracing::info!(
        modules = topo.len(), black_ug, red_ug,
        black_queen = ?black_q, red_queen = ?red_q, depth = ?depth,
        "Topology::two_colony_nest_arena built (5 modules)"
    );
    topo
}
```

> **Determinism / safety:** all index math uses `saturating_sub`/`.clamp`/`.max(…)` — no panics on small `w`/`h`. The `find_nest_entrance` traversal contract is preserved (one entrance per UG, set with the same `colony_id`). No RNG. `attach_underground` and `two_colony_arena` are untouched, so every existing test that uses them is byte-identical.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-sim attach_underground_deep two_colony_nest_arena queen_reachable 2>&1 | tail -25`
Expected: PASS (5 tests: the 4 above + `attach_underground_deep_shallow_is_near_entrance`).

- [ ] **Step 5: Confirm no existing topology test regressed**

Run: `cargo test -p antcolony-sim topology:: 2>&1 | tail -20`
Expected: all green (legacy `two_colony_arena` / `attach_underground` tests unchanged).

- [ ] **Step 6: Commit**

```bash
git add crates/antcolony-sim/src/topology.rs
git commit -m "feat(sim): attach_underground_deep + two_colony_nest_arena (deep queen behind entrance+tunnel chokepoint)"
```

---

### Task 3: `Simulation::new_two_colony_nest_arena` — deep queen relocation

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs`
- Test: in-module `#[cfg(test)]`

**Interfaces:**
```rust
impl Simulation {
    /// Cross-species nest arena. Builds the colonies/ants via the EXISTING
    /// `new_two_colony_cross_species` (so spawn + caste counts + avenger logic
    /// are unchanged), using a `two_colony_nest_arena` topology, then relocates
    /// each queen into her deep `UndergroundNest` `QueenChamber` and records the
    /// UG module on the colony. Surface ants stay on the surface; raiders reach
    /// the queen via the entrance+tunnel (Task 4).
    pub fn new_two_colony_nest_arena(
        world_pheromone_hazards: SimConfig,
        cfg_black: crate::config::ColonySimConfig,
        cfg_red: crate::config::ColonySimConfig,
        topology: Topology,           // expected: two_colony_nest_arena(...)
        seed: u64,
        nest_black_module: ModuleId,  // surface nest, e.g. 0
        nest_red_module: ModuleId,    // surface nest, e.g. 2
        black_underground: ModuleId,  // e.g. 3
        red_underground: ModuleId,    // e.g. 4
    ) -> Self;
}
```

- [ ] **Step 1: Write the failing test**

In `simulation.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn nest_arena_relocates_queens_into_deep_underground_chambers() {
    use crate::config::ColonySimConfig;
    use crate::world::{ChamberType, Terrain};
    use crate::topology::QueenDepth;

    let global = SimConfig::default();
    let slice = ColonySimConfig::from(&global);
    let topo = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
    let black_ug = topo.underground_for_colony(0).expect("black ug");
    let red_ug = topo.underground_for_colony(1).expect("red ug");

    let sim = Simulation::new_two_colony_nest_arena(
        global, slice.clone(), slice, topo, 99, 0, 2, black_ug, red_ug,
    );

    // Each colony records its UG module.
    assert_eq!(sim.colonies[0].underground_module, Some(black_ug));
    assert_eq!(sim.colonies[1].underground_module, Some(red_ug));

    // Each queen now lives in her UG module, on a QueenChamber cell.
    for (cid, ug) in [(0u8, black_ug), (1u8, red_ug)] {
        let q = sim.ants.iter().find(|a| a.colony_id == cid && matches!(a.caste, AntCaste::Queen))
            .expect("queen exists");
        assert_eq!(q.module_id, ug, "colony {cid} queen should be underground");
        let m = sim.topology.module(ug);
        let (gx, gy) = m.world.world_to_grid(q.position);
        assert!(m.world.in_bounds(gx, gy));
        assert_eq!(m.world.get(gx as usize, gy as usize), Terrain::Chamber(ChamberType::QueenChamber),
            "colony {cid} queen should stand on her QueenChamber cell");
    }

    // Workers are still on the surface nest modules (raid pathing moves them
    // later; at t=0 only the queen is relocated).
    let surface_workers = sim.ants.iter()
        .filter(|a| a.colony_id == 0 && !matches!(a.caste, AntCaste::Queen) && a.module_id == 0)
        .count();
    assert!(surface_workers > 0, "black workers should still be on the surface nest at t=0");
}

#[test]
fn nest_arena_match_status_in_progress_at_start() {
    use crate::config::ColonySimConfig;
    use crate::topology::QueenDepth;
    let global = SimConfig::default();
    let slice = ColonySimConfig::from(&global);
    let topo = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
    let (bug, rug) = (topo.underground_for_colony(0).unwrap(), topo.underground_for_colony(1).unwrap());
    let mut sim = Simulation::new_two_colony_nest_arena(global, slice.clone(), slice, topo, 7, 0, 2, bug, rug);
    assert!(matches!(sim.match_status(), crate::ai::MatchStatus::InProgress));
    sim.run(50); // does not panic; both queens still alive deep underground.
    assert!(matches!(sim.match_status(), crate::ai::MatchStatus::InProgress));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-sim nest_arena_relocates_queens nest_arena_match_status 2>&1 | tail -25`
Expected: FAIL — `new_two_colony_nest_arena` not found.

- [ ] **Step 3: Implement `new_two_colony_nest_arena`**

Add to `impl Simulation` in `simulation.rs`, right after `new_two_colony_cross_species` (do NOT modify that function):

```rust
/// Cross-species **nest arena** constructor. Reuses
/// `new_two_colony_cross_species` for the entire colony/ant/spawn build
/// (byte-identical to a cross-species match on the same surface modules),
/// then relocates each queen into her deep `UndergroundNest` `QueenChamber`
/// and records the underground module on the colony so raid pathing + queen
/// economy can find it.
#[allow(clippy::too_many_arguments)]
pub fn new_two_colony_nest_arena(
    world_pheromone_hazards: SimConfig,
    cfg_black: crate::config::ColonySimConfig,
    cfg_red: crate::config::ColonySimConfig,
    topology: Topology,
    seed: u64,
    nest_black_module: ModuleId,
    nest_red_module: ModuleId,
    black_underground: ModuleId,
    red_underground: ModuleId,
) -> Self {
    // Build the full cross-species sim on the (5-module) nest topology.
    let mut sim = Self::new_two_colony_cross_species(
        world_pheromone_hazards,
        cfg_black,
        cfg_red,
        topology,
        seed,
        nest_black_module,
        nest_red_module,
    );

    // Record each colony's underground module.
    if let Some(c) = sim.colonies.iter_mut().find(|c| c.id == 0) {
        c.underground_module = Some(black_underground);
    }
    if let Some(c) = sim.colonies.iter_mut().find(|c| c.id == 1) {
        c.underground_module = Some(red_underground);
    }

    // Relocate each queen into the deep QueenChamber of her UG module.
    // Iterate ants in index order (determinism); find the first QueenChamber
    // cell in the module and stand the queen on its world center.
    for (cid, ug) in [(0u8, black_underground), (1u8, red_underground)] {
        let Some(qcell) = sim.queen_chamber_cell(ug) else {
            tracing::warn!(colony = cid, ug, "nest arena: no QueenChamber cell found; queen left on surface");
            continue;
        };
        let world_pos = sim.topology.module(ug).world.grid_to_world(qcell.0, qcell.1);
        for a in sim.ants.iter_mut() {
            if a.colony_id == cid && matches!(a.caste, AntCaste::Queen) {
                tracing::info!(colony = cid, ant = a.id, ug, cell = ?qcell, "nest arena: queen relocated deep");
                a.module_id = ug;
                a.position = world_pos;
            }
        }
        // Keep colony.nest_entrance_positions consistent with the UG entrance
        // so any code reading it (obs/logging) sees the defended choke.
        if let Some((ex, ey)) = sim.topology.module(ug).world.find_nest_entrance(cid) {
            let epos = sim.topology.module(ug).world.grid_to_world(ex, ey);
            if let Some(c) = sim.colonies.iter_mut().find(|c| c.id == cid) {
                c.nest_entrance_positions = vec![epos];
            }
        }
    }

    tracing::info!(
        modules = sim.topology.modules.len(),
        black_ug = black_underground,
        red_ug = red_underground,
        "Simulation::new_two_colony_nest_arena (queens relocated deep)"
    );
    sim
}

/// First `QueenChamber` grid cell in a module, scanned in row-major order
/// (deterministic). Used to place the queen at nest-arena construction.
fn queen_chamber_cell(&self, module: ModuleId) -> Option<(usize, usize)> {
    use crate::world::{ChamberType, Terrain};
    let m = self.topology.try_module(module)?;
    let (w, h) = (m.width(), m.height());
    for y in 0..h {
        for x in 0..w {
            if m.world.get(x, y) == Terrain::Chamber(ChamberType::QueenChamber) {
                return Some((x, y));
            }
        }
    }
    None
}
```

> **Back-compat:** this constructor is brand-new and only ever called by the new `MatchEnv::new_cross_species_nest_arena` (Task 6) and tests. Nothing existing calls it. `new_two_colony_cross_species` is unchanged, so its byte-identical guard from the cross-species plan still holds.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-sim nest_arena_relocates_queens nest_arena_match_status 2>&1 | tail -25`
Expected: PASS (2 tests).

- [ ] **Step 5: Confirm no existing sim test regressed (the keystone guard)**

Run: `cargo test -p antcolony-sim cross_species_with_equal_cfg new_ai_vs_ai_still_byte_identical 2>&1 | tail -20`
Expected: PASS — the byte-identical guards from the cross-species build are untouched.

- [ ] **Step 6: Commit**

```bash
git add crates/antcolony-sim/src/simulation.rs
git commit -m "feat(sim): new_two_colony_nest_arena delegates cross-species build + relocates queens into deep UG chambers"
```

---

### Task 4: Raid descent — widen `surface_underground_traversal` for enemy-entrance descent

> **This is the load-bearing behavioral task.** It is the ONLY hot-path change, gated behind `combat.raid_underground_enabled` (default `false`) so all existing sims are byte-identical.

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs` (`surface_underground_traversal`)
- Test: in-module `#[cfg(test)]`

**Interfaces:** no signature change. The existing private `fn surface_underground_traversal(&mut self)` gains an additive branch:
- Existing behavior (own-colony descent when `state == Digging`; ascent for soil-carry/foraging workers) is UNCHANGED.
- New behavior (only when `self.config.combat.raid_underground_enabled`): a non-queen ant of colony Y standing on colony X's **surface** `NestEntrance` cell (X ≠ Y), in `AntState::Fighting` or `AntState::Usurping`, descends into colony X's `UndergroundNest`, teleporting to that UG's entrance cell.

- [ ] **Step 1: Write the failing test**

In `simulation.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn raider_descends_enemy_surface_entrance_into_enemy_underground() {
    use crate::config::ColonySimConfig;
    use crate::topology::QueenDepth;

    let mut global = SimConfig::default();
    global.combat.raid_underground_enabled = true;
    let slice = ColonySimConfig::from(&global);
    let topo = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
    let (bug, rug) = (topo.underground_for_colony(0).unwrap(), topo.underground_for_colony(1).unwrap());
    let mut sim = Simulation::new_two_colony_nest_arena(global, slice.clone(), slice, topo, 5, 0, 2, bug, rug);

    // Place a colony-1 (red) fighter on colony-0's (black) surface entrance cell.
    let (ex, ey) = sim.topology.module(0).world.find_nest_entrance(0).expect("black surface entrance");
    let epos = sim.topology.module(0).world.grid_to_world(ex, ey);
    // Reuse an existing red worker; force it onto the enemy entrance + Fighting.
    let idx = sim.ants.iter().position(|a| a.colony_id == 1 && !matches!(a.caste, AntCaste::Queen))
        .expect("a red ant");
    sim.ants[idx].module_id = 0;            // standing in black's surface nest
    sim.ants[idx].position = epos;
    sim.ants[idx].transition(AntState::Fighting);

    sim.surface_underground_traversal_for_test(); // thin test shim, see Step 3

    assert_eq!(sim.ants[idx].module_id, bug,
        "red fighter on black's surface entrance should descend into black's underground");
}

#[test]
fn raid_descent_is_inert_when_disabled() {
    use crate::config::ColonySimConfig;
    use crate::topology::QueenDepth;
    let global = SimConfig::default(); // raid_underground_enabled == false
    let slice = ColonySimConfig::from(&global);
    let topo = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
    let (bug, rug) = (topo.underground_for_colony(0).unwrap(), topo.underground_for_colony(1).unwrap());
    let mut sim = Simulation::new_two_colony_nest_arena(global, slice.clone(), slice, topo, 5, 0, 2, bug, rug);

    let (ex, ey) = sim.topology.module(0).world.find_nest_entrance(0).unwrap();
    let epos = sim.topology.module(0).world.grid_to_world(ex, ey);
    let idx = sim.ants.iter().position(|a| a.colony_id == 1 && !matches!(a.caste, AntCaste::Queen)).unwrap();
    sim.ants[idx].module_id = 0;
    sim.ants[idx].position = epos;
    sim.ants[idx].transition(AntState::Fighting);

    sim.surface_underground_traversal_for_test();
    assert_eq!(sim.ants[idx].module_id, 0, "with raid disabled, the fighter must NOT descend");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-sim raider_descends_enemy raid_descent_is_inert 2>&1 | tail -25`
Expected: FAIL — `surface_underground_traversal_for_test` not found / raider does not descend.

- [ ] **Step 3: Implement the additive raid-descent branch + a test shim**

First add a thin test-only shim so tests can drive the private pass directly (place it in the same `impl Simulation` block, NOT inside `#[cfg(test)]` module — it must call the private method):

```rust
/// Test-only: drive the surface↔underground traversal pass directly.
#[doc(hidden)]
pub fn surface_underground_traversal_for_test(&mut self) {
    self.surface_underground_traversal();
}
```

Now widen `surface_underground_traversal`. The existing function snapshots per-colony entrance pairs `(colony_id, surf_mod, surf_pos, ug_mod, ug_pos)` and then loops ants. Add the raid descent inside the per-ant loop, BEFORE or AFTER the existing own-colony arms (it is mutually exclusive: it only matches enemy ants on a surface entrance, which the own-colony arm's `ant.module_id == surf_mod` for the SAME colony's pair does not). Insert this block (the `entrance_pairs` snapshot is the existing local `Vec`; each element is the tuple described in the function's comment):

```rust
        // ── Raid descent (spec A3/B6): enemy fighters/usurpers flow DOWN an
        // enemy nest entrance into that colony's UndergroundNest, then follow
        // the alarm gradient toward the deep queen. Gated; default OFF so all
        // existing sims are byte-identical. Iterate ants in index order.
        if self.config.combat.raid_underground_enabled {
            for ant in self.ants.iter_mut() {
                if matches!(ant.caste, AntCaste::Queen) {
                    continue;
                }
                if !matches!(ant.state, AntState::Fighting | AntState::Usurping) {
                    continue;
                }
                // Is this ant standing on the SURFACE entrance of an ENEMY colony
                // that has an underground module? If so, descend into it.
                for &(ecid, surf_mod, surf_pos, ug_mod, ug_pos) in entrance_pairs.iter() {
                    if ecid == ant.colony_id {
                        continue; // own nest is handled by the existing arms
                    }
                    if ant.module_id != surf_mod {
                        continue;
                    }
                    let (gx, gy) = self
                        .topology
                        .module(surf_mod)
                        .world
                        .world_to_grid(ant.position);
                    if (gx, gy) == (surf_pos.x as i64, surf_pos.y as i64) {
                        tracing::debug!(
                            tick = self.tick, ant = ant.id, raider_colony = ant.colony_id,
                            target_colony = ecid, from_module = surf_mod, to_module = ug_mod,
                            "raid: enemy entrance descent"
                        );
                        ant.module_id = ug_mod;
                        ant.position = ug_pos;
                        break;
                    }
                }
            }
        }
```

> **Notes.**
> - `entrance_pairs` is the existing snapshot Vec built at the top of `surface_underground_traversal` (verified at `simulation.rs:3140-3164`). If the local is named differently in the current source, use that name — do NOT rebuild the snapshot (borrow-checker + determinism). If the existing snapshot does not already include `ug_pos` (the UG entrance world position), reuse the existing `find_nest_entrance(cid)` + `grid_to_world` it already computes for the own-colony arms.
> - This branch only TELEPORTS the raider to the enemy UG entrance. From there the raider moves via the normal movement/decision systems following the alarm gradient the scouts/column laid (B6) — no special pathfinder, exactly as the spec requires. The deep queen is reached only after traversing entrance + tunnel cells (cap-gated).
> - Ascent for raiders is intentionally NOT added: a raider that wants to retreat is in `Fleeing`, which the existing ascent arm already does not move out of an enemy UG — acceptable for V1 (raiders commit). Note this as a known V1 scope edge in the harness doc.
> - No RNG, no `HashMap` iteration affecting outcome; ants iterated in index order.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-sim raider_descends_enemy raid_descent_is_inert 2>&1 | tail -25`
Expected: PASS (2 tests).

- [ ] **Step 5: Confirm traversal + determinism regression suite green**

Run: `cargo test -p antcolony-sim traversal surface_underground determinism det_ 2>&1 | tail -25`
Expected: all green (existing own-colony traversal tests + determinism guards unaffected — the new branch is behind a default-off flag).

- [ ] **Step 6: Commit**

```bash
git add crates/antcolony-sim/src/simulation.rs
git commit -m "feat(sim): raid descent — enemy fighters/usurpers descend enemy nest entrance (gated, default off)"
```

---

### Task 5: Underground-idle lazy-worker alarm wake (B7)

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs` (FSM decision pass)
- Test: in-module `#[cfg(test)]`

**Interfaces:** no signature change. In the decision pass, underground `Idle` ants transition to `Fighting` when local alarm exceeds `cfg.ant.underground_idle_alarm_threshold`. Default threshold is `1.0e9` (inert) so existing sims are byte-identical; the nest arena lowers it (Task 6).

> **Reuse note.** The decision pass already reads per-module alarm pheromone for state transitions. Read the alarm at the ant's `(module_id, position)` via the existing per-module pheromone read used elsewhere in the decision pass (`self.topology.module(ant.module_id).pheromones.read(gx, gy, PheromoneLayer::Alarm)`). Add the arm only for ants whose `module_id` is an `UndergroundNest` module, in `AntState::Idle`.

- [ ] **Step 1: Write the failing test**

In `simulation.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn underground_idle_worker_wakes_to_fighting_on_alarm() {
    use crate::config::ColonySimConfig;
    use crate::topology::QueenDepth;
    use crate::pheromone::PheromoneLayer;

    let mut global = SimConfig::default();
    global.ant.underground_idle_alarm_threshold = 0.3; // nest-arena style low reserve threshold
    let slice = ColonySimConfig::from(&global);
    let topo = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
    let (bug, rug) = (topo.underground_for_colony(0).unwrap(), topo.underground_for_colony(1).unwrap());
    let mut sim = Simulation::new_two_colony_nest_arena(global, slice.clone(), slice, topo, 3, 0, 2, bug, rug);

    // Put a black worker, Idle, in the black UG on an Empty tunnel cell.
    let (ex, ey) = sim.topology.module(bug).world.find_nest_entrance(0).unwrap();
    let cell = (ex, ey.saturating_sub(1)); // one step into the tunnel
    let pos = sim.topology.module(bug).world.grid_to_world(cell.0, cell.1);
    let idx = sim.ants.iter().position(|a| a.colony_id == 0 && !matches!(a.caste, AntCaste::Queen)).unwrap();
    sim.ants[idx].module_id = bug;
    sim.ants[idx].position = pos;
    sim.ants[idx].transition(AntState::Idle);

    // Raise alarm above the threshold at that cell.
    sim.topology.module_mut(bug).pheromones.deposit(cell.0, cell.1, PheromoneLayer::Alarm, 5.0, 10.0);

    sim.run(1); // one tick runs the decision pass.
    assert_eq!(sim.ants[idx].state, AntState::Fighting,
        "underground idle worker should wake to Fighting when alarm > threshold");
}

#[test]
fn underground_idle_wake_inert_by_default() {
    // Default threshold (1e9) => never wakes, so the surface game is unchanged.
    use crate::config::ColonySimConfig;
    use crate::topology::QueenDepth;
    use crate::pheromone::PheromoneLayer;
    let global = SimConfig::default();
    let slice = ColonySimConfig::from(&global);
    let topo = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
    let (bug, rug) = (topo.underground_for_colony(0).unwrap(), topo.underground_for_colony(1).unwrap());
    let mut sim = Simulation::new_two_colony_nest_arena(global, slice.clone(), slice, topo, 3, 0, 2, bug, rug);
    let (ex, ey) = sim.topology.module(bug).world.find_nest_entrance(0).unwrap();
    let cell = (ex, ey.saturating_sub(1));
    let pos = sim.topology.module(bug).world.grid_to_world(cell.0, cell.1);
    let idx = sim.ants.iter().position(|a| a.colony_id == 0 && !matches!(a.caste, AntCaste::Queen)).unwrap();
    sim.ants[idx].module_id = bug;
    sim.ants[idx].position = pos;
    sim.ants[idx].transition(AntState::Idle);
    sim.topology.module_mut(bug).pheromones.deposit(cell.0, cell.1, PheromoneLayer::Alarm, 5.0, 10.0);
    sim.run(1);
    assert_eq!(sim.ants[idx].state, AntState::Idle, "default threshold must keep the worker Idle");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-sim underground_idle_worker_wakes underground_idle_wake_inert 2>&1 | tail -25`
Expected: FAIL — worker stays `Idle` (no wake arm yet).

- [ ] **Step 3: Implement the underground idle-wake arm**

In the decision pass (`sense_and_decide` and/or `decide_next_state` — wherever per-ant state transitions are computed; the FSM lives in `simulation.rs`), add, for an ant currently `Idle` whose `module_id` resolves to an `UndergroundNest` module:

```rust
        // B7 underground lazy-worker reserve wake: an Idle worker in an
        // UndergroundNest tile wakes to Fighting when local alarm exceeds the
        // (low) reserve threshold. Default threshold is effectively infinite, so
        // this is inert for every existing sim. Read alarm at the ant's own
        // (module, cell) — local information only (CLAUDE.md rule 4).
        if ant.state == AntState::Idle && !matches!(ant.caste, AntCaste::Queen) {
            if let Some(m) = self.topology.try_module(ant.module_id) {
                if m.kind == crate::module::ModuleKind::UndergroundNest {
                    let (gx, gy) = m.world.world_to_grid(ant.position);
                    if m.world.in_bounds(gx, gy) {
                        let alarm = m.pheromones.read(
                            gx as usize, gy as usize, crate::pheromone::PheromoneLayer::Alarm,
                        );
                        if alarm > self.config.ant.underground_idle_alarm_threshold {
                            tracing::debug!(
                                tick = self.tick, ant = ant.id, colony = ant.colony_id,
                                alarm, threshold = self.config.ant.underground_idle_alarm_threshold,
                                "B7: underground idle worker woke to Fighting"
                            );
                            ant.transition(AntState::Fighting);
                        }
                    }
                }
            }
        }
```

> **Borrow note.** If the decision pass borrows `self.ants` mutably while needing `&self.topology`/`&self.config`, follow the pattern already used in that pass (e.g. read pheromone/config into locals before the mutable ant loop, or use indexed access). Do NOT introduce a borrow that breaks the existing structure — mirror the surrounding code. The values read (`config.ant.underground_idle_alarm_threshold`, module kind, alarm at cell) are all already-available local reads in that pass.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-sim underground_idle_worker_wakes underground_idle_wake_inert 2>&1 | tail -25`
Expected: PASS (2 tests).

- [ ] **Step 5: Confirm FSM + full sim suite green**

Run: `cargo test -p antcolony-sim 2>&1 | tail -25`
Expected: all green (~225+ tests). The default-off threshold means no FSM regression.

- [ ] **Step 6: Commit**

```bash
git add crates/antcolony-sim/src/simulation.rs
git commit -m "feat(sim): B7 underground lazy-worker alarm wake (gated by neutral-default threshold)"
```

---

### Task 6: `MatchEnv::new_cross_species_nest_arena` + `--nest` flag on the matrix harness

**Files:**
- Modify: `crates/antcolony-trainer/src/env.rs`
- Modify: `crates/antcolony-trainer/src/bin/cross_species_matrix.rs`
- Create: `scripts/run_cross_species_nest_matrix.ps1`
- Test: in-module `#[cfg(test)]` in `env.rs`

**Interfaces:**
```rust
impl MatchEnv {
    /// Cross-species match routed through the underground **nest** arena
    /// (`two_colony_nest_arena`, deep queens). Mirrors `new_cross_species_arena`
    /// but: (1) topology has a private UG per colony, (2) queens start deep,
    /// (3) `raid_underground_enabled` + the underground idle-wake threshold are
    /// set on BOTH colony configs so the cap + raid + reserve mechanics engage.
    pub fn new_cross_species_nest_arena(species_a: &Species, species_b: &Species, seed: u64) -> Self;
}
```

- [ ] **Step 1: Write the failing test**

In `env.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn nest_arena_env_builds_five_modules_with_deep_queens() {
    use antcolony_sim::species::Species;
    use antcolony_sim::AntCaste;
    let s = Species::load_from_str(SAMPLE_SPECIES_TOML).expect("parse"); // existing test helper const
    let env = MatchEnv::new_cross_species_nest_arena(&s, &s, 1234);
    // 3 surface + 2 underground.
    assert_eq!(env.sim.topology.modules.len(), 5);
    // Both colonies recorded an underground module.
    assert!(env.sim.colonies[0].underground_module.is_some());
    assert!(env.sim.colonies[1].underground_module.is_some());
    // Raid mechanics armed on both colony configs.
    assert!(env.sim.colony_configs[0].combat.raid_underground_enabled);
    assert!(env.sim.colony_configs[1].combat.raid_underground_enabled);
    // Each queen is in her UG module at construction.
    for cid in [0u8, 1u8] {
        let ug = env.sim.colonies[cid as usize].underground_module.unwrap();
        let q = env.sim.ants.iter().find(|a| a.colony_id == cid && matches!(a.caste, AntCaste::Queen)).unwrap();
        assert_eq!(q.module_id, ug);
    }
}

#[test]
fn nest_arena_env_runs_a_short_match_without_panic() {
    use antcolony_sim::species::Species;
    let s = Species::load_from_str(SAMPLE_SPECIES_TOML).expect("parse");
    let mut env = MatchEnv::new_cross_species_nest_arena(&s, &s, 42);
    env.max_ticks = 500;
    env.sim.run(200);
    // Mirror-match: still in progress or a clean terminal — never a panic.
    let _ = env.sim.match_status();
}
```

(If `env.rs` lacks an existing `SAMPLE_SPECIES_TOML`/`sample_toml()` helper, reuse the same species-loading idiom the existing `new_cross_species` tests use, e.g. `Species::load_from_str(include_str!("../../../assets/species/formica_rufa.toml"))`.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-trainer nest_arena_env 2>&1 | tail -25`
Expected: FAIL — `new_cross_species_nest_arena` not found.

- [ ] **Step 3: Implement `new_cross_species_nest_arena`**

Add to `impl MatchEnv` in `env.rs`, right after `new_cross_species_arena` (do NOT modify the existing constructors):

```rust
/// Cross-species match in the underground **nest** arena. Same per-colony
/// biology as `new_cross_species_arena`, but the topology is
/// `two_colony_nest_arena` (private deep UG per colony) and the raid + idle
/// reserve mechanics are armed on both colonies. The win-matrix harness uses
/// this under `--nest` to test whether the nest layer breaks the flat-arena
/// dominance hierarchy into intransitivity.
pub fn new_cross_species_nest_arena(species_a: &Species, species_b: &Species, seed: u64) -> Self {
    use antcolony_sim::QueenDepth;
    let env = Environment { world_width: 32, world_height: 32, ..Environment::default() };

    let mut global = species_a.apply(&env);
    global.world = WorldConfig { width: 32, height: 32, ..WorldConfig::default() };

    let mut cfg_a: ColonySimConfig = species_a.apply_colony(&env);
    let mut cfg_b: ColonySimConfig = species_b.apply_colony(&env);

    // Arm raid + reserve-wake on both colonies (the cap injection still happens
    // in the harness; here we only enable the descent + low idle threshold so
    // the nest mechanics engage). Surface combat is unchanged because surface
    // cells use the open cap.
    for c in [&mut cfg_a, &mut cfg_b] {
        c.combat.raid_underground_enabled = true;
        c.ant.underground_idle_alarm_threshold = 0.3; // B7 reserve threshold
    }

    let topology = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
    let black_ug = topology.underground_for_colony(0).expect("black ug");
    let red_ug = topology.underground_for_colony(1).expect("red ug");

    let mut sim = Simulation::new_two_colony_nest_arena(
        global, cfg_a, cfg_b, topology, seed, 0, 2, black_ug, red_ug,
    );
    if let Some(c0) = sim.colonies.get_mut(0) {
        c0.is_ai_controlled = true;
    }

    let prev_workers = [
        sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0),
        sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0),
    ];
    let prev_queens_alive = [1, 1];
    let prev_food = [
        sim.colonies.get(0).map(|c| c.food_stored).unwrap_or(0.0),
        sim.colonies.get(1).map(|c| c.food_stored).unwrap_or(0.0),
    ];
    tracing::info!(
        species_a = %species_a.id, species_b = %species_b.id, seed,
        "MatchEnv::new_cross_species_nest_arena constructed (underground nest topology)"
    );
    Self { sim, max_ticks: 10_000, prev_workers, prev_queens_alive, prev_food }
}
```

(Ensure `WorldConfig`, `ColonySimConfig`, `Topology`, `Simulation`, `Environment` are already imported in `env.rs` — they are, used by `new_cross_species_arena`.)

- [ ] **Step 4: Add the `--nest` flag to the harness**

In `cross_species_matrix.rs` `main()`:
- Add a `let mut nest = false;` near the other flag vars and a match arm `"--nest" => nest = true,`.
- In the parallel closure, select the constructor:

```rust
                let mut env = if nest {
                    MatchEnv::new_cross_species_nest_arena(sp_left, sp_right, seed)
                } else {
                    MatchEnv::new_cross_species_arena(sp_left, sp_right, seed)
                };
                env.max_ticks = max_ticks;
```

  `nest` is `Copy` so it moves into the rayon closure cleanly (`move` closures capture by copy).
- The existing cap-injection loop (lines 98-108) is unchanged; it sets `_open=255/_tunnel=3/_entrance=1` and predator corpse-loot on BOTH arenas. Under `--nest` the tunnel cap now actually fires (the UG module exists). Add one log line after the loop documenting the arena:

```rust
                tracing::debug!(nest, "arena selected for match");
```
- After printing the matrix, add a banner line so the output records the arena:

```rust
    println!("# arena: {}", if nest { "underground-nest (5-module)" } else { "flat chokepoint (3-module)" });
```

- [ ] **Step 5: Run to verify env tests + harness build**

Run: `cargo test -p antcolony-trainer nest_arena_env 2>&1 | tail -25`
Expected: PASS (2 tests).
Run: `cargo build -p antcolony-trainer --bin cross_species_matrix 2>&1 | tail -10`
Expected: builds clean (no warnings about unused `nest`).

- [ ] **Step 6: Create the PowerShell wrapper**

Create `scripts/run_cross_species_nest_matrix.ps1`:

```powershell
# Run the cross-species win matrix in the UNDERGROUND NEST arena.
# Compares against the flat chokepoint arena to test the intransitivity
# hypothesis: does defense-in-depth break the strict dominance hierarchy?
$ErrorActionPreference = "Stop"
$env:RUST_LOG = "info"
cargo run -p antcolony-trainer --bin cross_species_matrix --release -- `
    --species-dir assets/species --mpe 50 --max-ticks 8000 --nest
```

- [ ] **Step 7: Commit**

```bash
git add crates/antcolony-trainer/src/env.rs crates/antcolony-trainer/src/bin/cross_species_matrix.rs scripts/run_cross_species_nest_matrix.ps1
git commit -m "feat(trainer): nest-arena MatchEnv + --nest flag on cross_species_matrix harness"
```

---

### Task 7: Defensive-inversion integration test (the intransitivity demonstration)

> The payoff test. It validates the WHOLE feature end-to-end: a small/defensive colony in a tunneled nest survives/holds against a numerically/combat-superior attacker that beats it on open ground. This is the mechanism by which the dominance hierarchy can become intransitive.

**Files:**
- Create: `crates/antcolony-sim/tests/nest_arena.rs`

**Interfaces:** integration test only (uses the public `Simulation` API + `Topology::two_colony_nest_arena`). No production code change. The test constructs two extreme configs:
- **Attacker (colony 0):** large worker count, high `worker_attack`/`soldier_attack` — wins decisively on the flat arena.
- **Defender (colony 1):** small worker count but soldiers (`soldier_attack` high) and the deep nest.

It runs the SAME pairing on (a) the flat chokepoint arena and (b) the nest arena, and asserts the outcome differs in the defender's favor (or at least: defender survives far longer / is not wiped) in the nest arena.

- [ ] **Step 1: Write the failing test**

Create `crates/antcolony-sim/tests/nest_arena.rs`:

```rust
//! End-to-end: the underground nest layer lets a small/defensive colony hold
//! a chokepoint against a swarm it loses to on open ground. This is the
//! mechanism behind the cross-species intransitivity hypothesis.

use antcolony_sim::config::{ColonySimConfig, SimConfig};
use antcolony_sim::topology::{QueenDepth, Topology};
use antcolony_sim::{AntCaste, Simulation};
use antcolony_sim::ai::MatchStatus;

/// Build an attacker (big, hard-hitting) and a defender (small, soldier-heavy)
/// pair of per-colony configs. Returns (global, attacker_cfg, defender_cfg).
fn lopsided_pair() -> (SimConfig, ColonySimConfig, ColonySimConfig) {
    let global = SimConfig::default();
    let mut attacker = ColonySimConfig::from(&global);
    attacker.ant.initial_count = 120;
    attacker.combat.worker_attack = 3.0;
    attacker.combat.soldier_attack = 6.0;

    let mut defender = ColonySimConfig::from(&global);
    defender.ant.initial_count = 24;        // ~1:5 numerical disadvantage
    defender.combat.worker_attack = 1.0;
    defender.combat.soldier_attack = 6.0;   // quality holds the choke
    (global, attacker, defender)
}

/// How long colony 1 (defender) survives, and whether it was wiped, on a
/// given arena. Returns (defender_alive_at_end, ticks_run).
fn run_match(global: SimConfig, attacker: ColonySimConfig, defender: ColonySimConfig, nest: bool)
    -> (bool, u64)
{
    let max_ticks = 2000u64;
    let mut sim = if nest {
        let mut g = global;
        g.combat.raid_underground_enabled = true;
        let mut atk = attacker; let mut def = defender;
        atk.combat.raid_underground_enabled = true;
        def.combat.raid_underground_enabled = true;
        def.ant.underground_idle_alarm_threshold = 0.3;
        // Caps that make the choke bite (mirror the harness injection).
        for c in [&mut atk, &mut def] {
            c.combat.max_simultaneous_attackers_open = 255;
            c.combat.max_simultaneous_attackers_tunnel = 3;
            c.combat.max_simultaneous_attackers_entrance = 1;
        }
        let topo = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
        let bug = topo.underground_for_colony(0).unwrap();
        let rug = topo.underground_for_colony(1).unwrap();
        Simulation::new_two_colony_nest_arena(g, atk, def, topo, 7, 0, 2, bug, rug)
    } else {
        // Flat chokepoint arena: caps injected, but NO underground => the
        // entrance/tunnel cap can only bite at the surface NestEntrance cell.
        let mut atk = attacker; let mut def = defender;
        for c in [&mut atk, &mut def] {
            c.combat.max_simultaneous_attackers_open = 255;
            c.combat.max_simultaneous_attackers_tunnel = 3;
            c.combat.max_simultaneous_attackers_entrance = 1;
        }
        let topo = Topology::two_colony_arena((24, 24), (32, 32));
        Simulation::new_two_colony_cross_species(global, atk, def, topo, 7, 0, 2)
    };

    let mut ticks = 0u64;
    while ticks < max_ticks {
        sim.tick();
        ticks += 1;
        if !matches!(sim.match_status(), MatchStatus::InProgress) {
            break;
        }
    }
    let def_alive = sim.ants.iter().any(|a| a.colony_id == 1 && matches!(a.caste, AntCaste::Queen))
        && sim.colonies[1].adult_total() > 0;
    (def_alive, ticks)
}

#[test]
fn defender_holds_in_nest_arena_longer_than_on_flat_arena() {
    let (g, atk, def) = lopsided_pair();
    let (flat_alive, flat_ticks) = run_match(g.clone(), atk.clone(), def.clone(), false);
    let (nest_alive, nest_ticks) = run_match(g, atk, def, true);

    // The defensive inversion: in the nest arena the small colony survives at
    // least as long, and meaningfully longer, than on the flat arena where the
    // swarm overruns it. (If the flat arena already lets it survive, the nest
    // arena must not make it WORSE — the strict assertion is the survival edge.)
    assert!(
        nest_ticks >= flat_ticks,
        "defender should survive at least as long in the nest arena \
         (flat={flat_ticks}, nest={nest_ticks})"
    );
    // The headline: the nest layer changes the outcome in the defender's favor.
    // Either it survives in the nest arena when it didn't on the flat arena, OR
    // it holds for a substantially longer siege.
    assert!(
        (nest_alive && !flat_alive) || nest_ticks >= flat_ticks + 200,
        "nest layer should flip/extend the defender's outcome \
         (flat_alive={flat_alive} @ {flat_ticks}, nest_alive={nest_alive} @ {nest_ticks})"
    );
}

#[test]
fn tunnel_cap_caps_attackers_in_underground_module() {
    // Independent of brain quality: pile many enemy attackers onto ONE defender
    // standing in an UndergroundNest tunnel cell and assert the cap (3) limits
    // simultaneous damage. We assert via survival: with cap=3 the defender (high
    // health) survives a tick that an uncapped pile would kill.
    let global = SimConfig::default();
    let mut atk = ColonySimConfig::from(&global);
    atk.combat.worker_attack = 2.0;
    atk.combat.max_simultaneous_attackers_tunnel = 3;   // cap bites
    atk.combat.max_simultaneous_attackers_open = 255;
    atk.combat.raid_underground_enabled = true;
    let mut def = ColonySimConfig::from(&global);
    def.combat.worker_health = 50.0;                    // survives 3×2=6 dmg/tick easily
    def.combat.max_simultaneous_attackers_tunnel = 3;
    def.combat.raid_underground_enabled = true;

    let topo = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
    let bug = topo.underground_for_colony(0).unwrap();
    let rug = topo.underground_for_colony(1).unwrap();
    let mut sim = Simulation::new_two_colony_nest_arena(global, atk, def, topo, 11, 0, 2, bug, rug);

    // Stand a defender (colony 1) on a black-UG tunnel cell and surround it with
    // many colony-0 attackers on the SAME cell (all within interaction radius).
    let (ex, ey) = sim.topology.module(bug).world.find_nest_entrance(0).unwrap();
    let cell = (ex, ey.saturating_sub(2));
    let pos = sim.topology.module(bug).world.grid_to_world(cell.0, cell.1);

    let def_idx = sim.ants.iter().position(|a| a.colony_id == 1 && !matches!(a.caste, AntCaste::Queen)).unwrap();
    sim.ants[def_idx].module_id = bug;
    sim.ants[def_idx].position = pos;
    sim.ants[def_idx].health = 50.0;

    let mut placed = 0;
    for a in sim.ants.iter_mut() {
        if a.colony_id == 0 && !matches!(a.caste, AntCaste::Queen) && placed < 10 {
            a.module_id = bug;
            a.position = pos; // co-located => all candidate attackers on the defender
            a.transition(antcolony_sim::AntState::Fighting);
            placed += 1;
        }
    }
    assert!(placed >= 6, "need a pile of attackers to exceed the cap");

    let hp_before = sim.ants[def_idx].health;
    sim.combat_tick();
    let hp_after = sim.ants.iter().find(|a| a.colony_id == 1 && !matches!(a.caste, AntCaste::Queen))
        .map(|a| a.health).unwrap_or(0.0);
    let dmg = hp_before - hp_after;
    // Cap=3 attackers × 2.0 attack = 6.0 max; assert NOT the uncapped 10×2=20.
    assert!(dmg <= 3.0 * 2.0 + 1e-3, "tunnel cap should limit damage to 3 attackers, got {dmg}");
    assert!(dmg > 0.0, "some damage should land");
}
```

> **Calibration caveat (note inline in the test file):** the exact survival numbers depend on movement/recruitment timing. Step 4 may require tuning `initial_count`, attack stats, `max_ticks`, or the `+200` margin so the test is robust but still demonstrates the inversion. Tune the *fixture*, never the production code. If the flat arena's surface entrance cap alone already saves the defender, increase the attacker's numerical edge until the flat arena overruns it, so the nest layer's contribution is isolated.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-sim --test nest_arena 2>&1 | tail -25`
Expected: FAIL at first compile if any symbol is missing, then assertion-fail if the fixture isn't yet calibrated. (The `tunnel_cap_caps_attackers_in_underground_module` test should PASS once Tasks 1-4 are in — it exercises only built mechanics; if it fails the cap path regressed and must be investigated FIRST.)

- [ ] **Step 3: Calibrate the fixture until the inversion is demonstrated**

Run the test, read the printed `flat=… nest=…` survival numbers from the assert messages, and adjust ONLY the test fixture (`lopsided_pair` counts/stats, `max_ticks`, the `+200` margin) until both assertions hold deterministically (seed is fixed at 7). Use `RUST_LOG=antcolony_sim=info cargo test -p antcolony-sim --test nest_arena -- --nocapture 2>&1 | tail -40` to see the raid/queen-relocation logs while tuning.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-sim --test nest_arena 2>&1 | tail -25`
Expected: PASS (2 tests).

- [ ] **Step 5: Full regression sweep (sim + trainer)**

Run: `cargo test -p antcolony-sim 2>&1 | tail -15`
Expected: all green (~225+ sim tests).
Run: `cargo test -p antcolony-trainer 2>&1 | tail -15`
Expected: all green (~80 trainer tests).
Run (determinism guard, both thread counts): `RAYON_NUM_THREADS=1 cargo test -p antcolony-sim determinism det_ 2>&1 | tail -10`
Expected: green and identical to default thread count.

- [ ] **Step 6: Commit**

```bash
git add crates/antcolony-sim/tests/nest_arena.rs
git commit -m "test(sim): defensive-inversion + tunnel-cap integration tests for the nest arena"
```

---

### Task 8: Run the win-matrix in both arenas and record the intransitivity result

> Not code — the experiment that answers the research question. Produces the artifact the feature exists for.

**Files:**
- Create: `scratch/nest_arena_matrix_<date>.txt` (output capture; gitignored scratch per CLAUDE.md output discipline)

- [ ] **Step 1: Baseline (flat arena) for comparison**

Run: `cargo run -p antcolony-trainer --bin cross_species_matrix --release -- --species-dir assets/species --mpe 50 --max-ticks 8000 2>&1 | tee scratch/nest_arena_matrix_flat.txt`
Capture: the matrix, `# intransitive 3-cycles: N`, all-win / all-lose rows. (Per MEMORY `project_ai_ceiling` / handoff, expect ~0 cycles, `formica_rufa` all-win, `temnothorax` all-lose on the flat arena.)

- [ ] **Step 2: Nest arena**

Run: `pwsh -ExecutionPolicy Bypass -File scripts/run_cross_species_nest_matrix.ps1 2>&1 | tee scratch/nest_arena_matrix_nest.txt`
Capture the same fields plus the `# arena: underground-nest (5-module)` banner.

- [ ] **Step 3: Compare + record the verdict**

Lead with data presence then verdict (MEMORY `feedback_distinguish_data_vs_verdict`): report the row count (N×N populated), THEN the cycle count delta and whether any prior all-win/all-lose row (e.g. `temnothorax`) gained ≥1 win in the nest arena. The hypothesis is supported if the nest arena shows more 3-cycles than the flat arena, OR a previously-degenerate row stops being all-lose/all-win. Write the comparison to `scratch/nest_arena_matrix_verdict.md`.

- [ ] **Step 4: Notify**

```bash
bash "/j/baremetal claude/tools/notify-telegram.sh" "antcolony nest-arena matrix done: flat cycles=<X>, nest cycles=<Y>; temnothorax all-lose flipped=<yes/no>"
```

---

## Self-Review (spec coverage vs the GAP, no placeholders, type consistency)

- **Spec coverage of the GAP:** A4/A6 (deep underground topology + arena wiring) → Tasks 2, 3. A3/B6 (raid pathing through tubes to the deep queen, no special pathfinder) → Task 4. A5/B7 (underground lazy-worker alarm wake) → Task 5. S3 (fortified small colony holds the choke) → Task 7. Win-matrix integration + intransitivity measurement → Tasks 6, 8. The already-built A1/A2/A7 (caps, gated usurp, CombatConfig knobs) are reused untouched — explicitly reconciled in the table above; the plan does NOT re-plan them.
- **Spec items correctly NOT planned:** A1 `terrain_attacker_cap`/`module_at` (built/unneeded), A2 occupation gating (superseded by the built `usurp_tick`), CombatConfig caps TOML (built). The spec's MlpBrain observation/reward work (T1/T2/T3) is deferred — the matrix uses `HeuristicBrain` on both sides (matching the existing harness), so the intransitivity question is answered without retraining. Queen evacuation (B4) and propaganda/phragmosis (open questions) remain deferred, consistent with the spec's own "deferred" markings.
- **No placeholders:** every code step is complete real Rust (constructors, the additive descent branch, the idle-wake arm, both integration tests). The one explicit borrow caveat in Task 5 Step 3 instructs mirroring the surrounding decision-pass pattern rather than leaving a gap.
- **Type consistency:** `QueenDepth` (topology) re-exported via `lib.rs`; `ModuleId = u16`; `attach_underground_deep` returns `(ModuleId, (usize,usize))`; `ColonyState.underground_module: Option<ModuleId>`; `new_two_colony_nest_arena` arg order matches `new_two_colony_cross_species` + the two UG ids; `Terrain`/`ChamberType`/`ModuleKind` variants match the real enums; `MatchStatus`/`match_status` win condition (queen by `colony_id`, not module) verified compatible with deep-queen relocation.
- **Determinism:** no new RNG; all new passes iterate ants in index order; the queen-chamber scan is row-major (deterministic); raid + idle-wake gated behind default-off config so existing byte-identical guards (`cross_species_with_equal_cfg_is_byte_identical_to_legacy_two_colony`) and cross-process/cross-thread determinism stay green.

## Known open question for the human

**Symmetric vs asymmetric depth (spec open Q1).** This plan builds **symmetric `QueenDepth::Deep`** for both colonies (matching the spec's V1 recommendation and tournament fairness). For the intransitivity experiment, a symmetric arena tests "does the choke mechanic alone reshuffle the hierarchy." If the symmetric nest arena does NOT produce cycles, the natural next experiment is asymmetric depth (let the smaller/weaker species sit deeper) — but that introduces a per-species depth parameter and a fairness question for the win-matrix (is a depth advantage "species biology" or "scenario handicap"?). **Confirm before Task 8 whether the experiment should also sweep asymmetric depth**, or whether a null result on the symmetric arena is itself the reportable finding.

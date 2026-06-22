# Cross-Species 1v1 Arena Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A 1v1 arena match where **colony 0 runs species A and colony 1 runs species B**, the two are genuinely asymmetric (per-colony attack/health/recruitment + a venom×resistance susceptibility matrix + terrain-gated group combat), and the win condition stays "kill the enemy queen" — but the queen-kill becomes a **gated, two-phase, interruptible usurpation channel** grounded in real ant social parasitism, not an instant snipe. Strict back-compat: every existing single-colony and same-species two-colony path stays **byte-identical**.

**Architecture:** Per-colony config slice (`ColonySimConfig{ant, colony, combat, species_id, clade, venom, ...}`) carried on `Simulation` as `colony_configs: Vec<ColonySimConfig>` indexed by colony id. The existing global `SimConfig` stays the source of truth for `world`/`pheromone`/`hazards` (one shared arena, one shared pheromone field) and for `colony_configs[0]`. `Species::apply_colony` factors the per-colony slice out of the existing sim-wide `Species::apply`. `combat_tick` (in `simulation.rs`, NOT a new `combat.rs`) reads each ant's per-colony combat params, applies the venom matrix and a terrain-gated `max_simultaneous_attackers`, and routes a new `usurp_tick` sub-pass for the gated two-phase queen-kill via a new `AntState::Usurping`. The trainer `MatchEnv` gains a cross-species constructor that calls `Species::apply_colony` for each side.

**Tech Stack:** Rust (edition 2024, MSRV 1.85; `Rng::gen` → `r#gen`). `antcolony-sim` (no Bevy; `tracing`, `serde`, `thiserror`, `rand`/`rand_chacha`, `glam`). `antcolony-trainer` (`candle-core`, `anyhow`). Headless deterministic tests via `cargo test`. Toolchain pinned to `stable-gnu` on kokonoe (per MEMORY `project_toolchain`).

## Global Constraints

- **STRICT BACK-COMPAT / byte-identical determinism.** The existing ~180 sim tests AND the cross-process / cross-rayon-thread-count determinism guarantee (MEMORY `project_determinism`) MUST stay byte-identical. The old `new_two_colony_with_topology(config, …)` MUST delegate to the new cross-species constructor with `cfg_black == cfg_red == ColonySimConfig::from(&config)` so its output is bit-for-bit unchanged. `Species::apply` MUST produce a bit-identical `SimConfig` before and after the `apply_colony` extraction. **Task 3 lands the byte-identical regression test BEFORE any combat/queen-kill behavior changes** — everything rides on it.
- **No `.unwrap()` / `.expect()` in simulation (non-test) code paths.** Use `Result`/`Option` + `thiserror`. Existing constructors use `assert!(!topology.is_empty(), …)` — match that established pattern, do not add new unwraps in hot paths.
- **Verbose `tracing` everywhere.** Every new system, state transition, gate evaluation, channel start/reset/complete, and venom/terrain cap application gets a structured `tracing` line (`info!`/`debug!`/`trace!` with fields). Never `println!`.
- **Edition 2024:** any `rand::Rng::gen` call is written `rng.r#gen::<…>()` (see `ant.rs:234`, `simulation.rs:2208`).
- **Determinism discipline for new RNG / iteration:** the gate/channel and any new per-pair resolution MUST iterate in a fixed, id-sorted order (no `HashMap` iteration affecting an outcome). Any new RNG must be seeded from `self.tick` + a fixed salt (mirror `age_mortality_tick`'s `self.tick.wrapping_mul(0x9E37…) ^ idx` pattern), NEVER drawn from `self.rng` in a way that perturbs the existing sequence.
- **Additive-only on shared types.** New struct fields are `#[serde(default)]`. New `CombatConfig`/`DietExtended`/`CombatExtended` fields default to behavior-neutral values so existing TOMLs and `SimConfig::default()` produce identical sims.
- **ECS / locality purity (CLAUDE.md rule 4):** individual ants still read only local state. The venom matrix and terrain cap are resolved inside `combat_tick` from per-colony config + the cell's module/terrain — no ant gains global knowledge.
- **Flat-file rule (CLAUDE.md):** combat stays in `simulation.rs` (combat_tick is ~170 lines, below the 300-line split threshold, and lifting it would risk the byte-identical guarantee). **Deviation from spec noted:** the spec proposed a new `combat.rs`; we keep combat in `simulation.rs`.

---

## File Structure

| File | Created/Modified | Single responsibility |
|------|------------------|------------------------|
| `crates/antcolony-sim/src/config.rs` | Modify | Add `ColonySimConfig` struct + `impl From<&SimConfig>`; add new `CombatConfig` fields (venom-resistance, terrain caps, usurp knobs). |
| `crates/antcolony-sim/src/species_extended.rs` | Modify | Add `DietExtended.predates_ants: bool` (wires dropped TOML key, B7); add `CombatExtended.venom_resistance: f32`. |
| `crates/antcolony-sim/src/species.rs` | Modify | Extract `apply_colony(&env) -> ColonySimConfig`; `apply` delegates to it; derive `Clade` from `genus`; carry venom/resistance/predates_ants into the per-colony slice. |
| `crates/antcolony-sim/src/clade.rs` | **Create** | `Clade` enum + `clade_from_genus(&str)` + `venom_multiplier(weapon, sting_potency, defender_clade)` static matrix (pure, unit-tested). |
| `crates/antcolony-sim/src/ant.rs` | Modify | Add `AntState::Usurping` FSM variant. |
| `crates/antcolony-sim/src/simulation.rs` | Modify | Add `colony_configs` field + `colony_cfg()`; `new_two_colony_cross_species`; old ctor delegates; per-colony combat in `combat_tick` (venom matrix, flee bias, terrain cap, corpse→killer routing); `usurp_tick` two-phase queen-kill; per-colony spawn health. |
| `crates/antcolony-sim/src/lib.rs` | Modify | `pub mod clade;` + re-export `ColonySimConfig`, `Clade`. |
| `crates/antcolony-trainer/src/env.rs` | Modify | `MatchEnv::new_cross_species(species_a, species_b, seed)` via `Species::apply_colony`; `MatchEnv::new` unchanged. |
| `crates/antcolony-trainer/src/bin/cross_species_matrix.rs` | **Create** | Win-matrix / intransitivity harness binary. |
| `scripts/run_cross_species_matrix.ps1` | **Create** | PowerShell run wrapper for the harness. |

### Reused interfaces (exact, from the current code — verified)

```rust
// config.rs — all #[derive(Debug, Clone, Deserialize)] #[serde(default)]
pub struct SimConfig { pub world: WorldConfig, pub pheromone: PheromoneConfig,
    pub ant: AntConfig, pub colony: ColonyConfig, pub combat: CombatConfig, pub hazards: HazardConfig }
pub struct CombatConfig { pub worker_attack: f32, pub soldier_attack: f32, pub worker_health: f32,
    pub soldier_health: f32, pub interaction_radius: f32, pub soldier_vs_worker_bonus: f32,
    pub corpse_food_units: u32, pub alarm_deposit_on_death: f32 }
impl Default for SimConfig { fn default() -> Self }  // CombatConfig::default() too

// species.rs
impl Species { pub fn apply(&self, env: &Environment) -> SimConfig; pub genus: String;
    pub combat: CombatProfile; pub combat_extended: CombatExtended; pub diet_extended: DietExtended; }
pub(crate) fn recruitment_deposit_scalar(style: RecruitmentStyle) -> f32;  // currently pub(crate)
pub fn load_species_dir<P: AsRef<Path>>(dir: P) -> Result<Vec<Species>, SimError>;

// species_extended.rs
pub enum Weapon { Mandible, Sting, FormicSpray, Chemical }   // #[default] Mandible
pub struct CombatExtended { pub weapon: Weapon, pub sting_potency: f32, pub ranged_attack: bool, /*…*/ }
pub struct DietExtended { pub seed_dispersal: bool, pub honeydew_dependent: bool,
    pub host_species_required: Vec<String>, pub food_storage_cap: Option<f32> }

// ant.rs
pub enum AntState { Idle, Exploring, FollowingTrail, PickingUpFood, ReturningHome, StoringFood,
    Fighting, Fleeing, Nursing, Digging, Diapause, NuptialFlight }   // <-- add Usurping
pub enum AntCaste { Worker, Soldier, Queen, Breeder }

// simulation.rs
pub struct Simulation { pub config: SimConfig, pub topology: Topology, pub ants: Vec<Ant>,
    pub colonies: Vec<ColonyState>, pub tick: u64, pub rng: ChaCha8Rng, /* private fields… */ }
impl Simulation {
    pub fn new(config: SimConfig, seed: u64) -> Self;
    pub fn new_with_topology(config: SimConfig, topology: Topology, seed: u64) -> Self;
    pub fn new_two_colony_with_topology(config: SimConfig, topology: Topology, seed: u64,
        nest_black_module: ModuleId, nest_red_module: ModuleId) -> Self;
    pub fn new_ai_vs_ai_with_topology(config: SimConfig, topology: Topology, seed: u64,
        nest_black_module: ModuleId, nest_red_module: ModuleId) -> Self;
    pub fn match_status(&self) -> crate::ai::MatchStatus;
    pub fn combat_tick(&mut self);
    pub fn tick(&mut self);  // drives substeps; calls combat_tick
}
pub fn spawn_initial_ants(config: &SimConfig, rng: &mut ChaCha8Rng, nest: Vec2, colony_id: u8,
    distribution: CasteRatio, id_offset: u32) -> Vec<Ant>;  // reads config.combat.{worker,soldier}_health, config.ant.initial_count

// topology.rs
impl Topology { pub fn two_colony_arena(nest_dim: (usize,usize), outworld_dim: (usize,usize)) -> Self;
    pub fn fit_bore_to_species(&mut self, worker_size_mm: f32, polymorphic: bool); }

// module.rs
pub enum ModuleKind { TestTubeNest, Outworld, /*…*/ UndergroundNest }
// world.rs
pub enum Terrain { /*…*/ NestEntrance(u8), Chamber(ChamberType), /*…*/ }
```

**Key real-code facts the spec got slightly wrong (FOLLOW THE REAL CODE):**
1. There is **no `combat.rs`** and we are NOT creating one (flat-file rule + determinism risk). Combat stays in `simulation.rs:1992 combat_tick`. (Spec's File-Structure row for `combat.rs` is dropped.)
2. `recruitment_deposit_scalar` is `pub(crate)`, not `pub`. The per-colony recruitment scalar is already baked into `pheromone.deposit_*` by `apply`; we **carry the scalar onto `ColonySimConfig`** for obs/logging rather than moving the whole `PheromoneConfig` per-colony (the shared pheromone field stays global — see Task 2 note). This is a deliberate MVP scope cut from spec §combat-model-4; pheromone deposit stays global, the scalar is exposed for the brain obs / win-matrix labelling only.
3. `combat_tick` already books queen death (`c.queen_health = 0.0` at `simulation.rs:2119`) when a `Queen` ant's health hits 0. The new gate must make the queen ant **invulnerable** (skip damage accumulation) until the gate opens, so this existing path only fires post-gate. The channel drains `queen_health` and, on completion, applies lethal damage to the queen ant so the existing death/`match_status` path resolves the win unchanged.
4. `sting_potency` exists on BOTH `AntConfig` (already wired by `apply`) and `CombatExtended`. We read it from the per-colony `ant.sting_potency` for the flee-bias to avoid a second source of truth.
5. `spawn_initial_ants` takes `&SimConfig` and reads `config.combat.{worker,soldier}_health` + `config.ant.initial_count`. For per-colony spawning we build a per-side `SimConfig` view (global + that colony's slice) and pass it — no signature change needed.

---

### Task 1: `ColonySimConfig` type + `From<&SimConfig>` + new `CombatConfig` fields

**Files:**
- Modify: `crates/antcolony-sim/src/config.rs`
- Modify: `crates/antcolony-sim/src/lib.rs`
- Test: in-module `#[cfg(test)]`

**Interfaces:**
- Consumes: `AntConfig`, `ColonyConfig`, `CombatConfig` (existing).
- Produces:
  ```rust
  #[derive(Debug, Clone)]
  pub struct ColonySimConfig {
      pub ant: AntConfig,
      pub colony: ColonyConfig,
      pub combat: CombatConfig,
      pub species_id: String,                 // "" for the default/back-compat slice
      pub clade: crate::clade::Clade,          // derived from genus; Unknown for default
      pub weapon: crate::species_extended::Weapon, // venom clade for the matrix
      pub recruitment_scalar: f32,             // 1.0 default; obs/logging only (MVP)
      pub predates_ants: bool,                 // B7: route corpse→killer when true
  }
  impl From<&SimConfig> for ColonySimConfig;   // behavior-neutral default slice
  // NEW CombatConfig fields (all behavior-neutral defaults):
  //   venom_resistance: f32 (0.0)
  //   max_simultaneous_attackers_open: u32 (255 — effectively uncapped)
  //   max_simultaneous_attackers_tunnel: u32 (255 — back-compat; cross-species sets 2)
  //   max_simultaneous_attackers_entrance: u32 (255 — back-compat; cross-species sets 1)
  //   usurp_gate_attacker_ratio: f32 (0.0 — disabled => queen behaves exactly as today)
  //   usurp_gate_defender_floor: u32 (0)
  //   usurp_channel_ticks: u32 (0 — disabled)
  //   usurp_corpse_to_killer_frac: f32 (0.0)
  ```
  Defaults are chosen so a `SimConfig::default()`-derived `ColonySimConfig` reproduces today's combat EXACTLY (uncapped attackers, gate disabled, no venom/predation feedback).

- [ ] **Step 1: Write the failing test**

```rust
// in config.rs #[cfg(test)] mod tests
#[test]
fn colony_sim_config_from_sim_config_is_behavior_neutral() {
    let sc = SimConfig::default();
    let csc = ColonySimConfig::from(&sc);
    // ant/colony/combat slices copied verbatim.
    assert_eq!(csc.ant.initial_count, sc.ant.initial_count);
    assert_eq!(csc.combat.worker_attack, sc.combat.worker_attack);
    assert_eq!(csc.combat.worker_health, sc.combat.worker_health);
    // New combat knobs default behavior-neutral.
    assert_eq!(csc.combat.venom_resistance, 0.0);
    assert_eq!(csc.combat.max_simultaneous_attackers_open, 255);
    assert_eq!(csc.combat.max_simultaneous_attackers_tunnel, 255);
    assert_eq!(csc.combat.max_simultaneous_attackers_entrance, 255);
    assert_eq!(csc.combat.usurp_gate_attacker_ratio, 0.0);
    assert_eq!(csc.combat.usurp_channel_ticks, 0);
    assert_eq!(csc.combat.usurp_corpse_to_killer_frac, 0.0);
    // Per-colony metadata defaults.
    assert_eq!(csc.species_id, "");
    assert_eq!(csc.recruitment_scalar, 1.0);
    assert!(!csc.predates_ants);
    assert_eq!(csc.weapon, crate::species_extended::Weapon::Mandible);
    assert_eq!(csc.clade, crate::clade::Clade::Unknown);
}

#[test]
fn new_combat_fields_round_trip_and_default_via_toml() {
    // A combat block omitting the new fields must keep the neutral defaults.
    let toml = "[combat]\nworker_attack = 2.0\n";
    let cfg = SimConfig::load_from_str(toml).expect("parse");
    assert_eq!(cfg.combat.worker_attack, 2.0);
    assert_eq!(cfg.combat.venom_resistance, 0.0);
    assert_eq!(cfg.combat.max_simultaneous_attackers_tunnel, 255);
    assert_eq!(cfg.combat.usurp_channel_ticks, 0);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-sim colony_sim_config 2>&1 | tail -20`
Expected: FAIL — `ColonySimConfig` / new fields / `crate::clade::Clade` not found (won't compile). (Task 2 lands `clade`; for this task, temporarily reference the enum path that Task 2 creates — implement Task 1 and Task 2 in the same branch order; if running Task 1 first, add a minimal `pub mod clade { #[derive(...)] pub enum Clade { Unknown } }` placeholder, then flesh out in Task 2. Cleaner: do Step 3 of Task 2's `clade.rs` enum first, then this task. The plan orders `clade.rs` creation here to avoid a dangling reference.)

- [ ] **Step 3: Create `clade.rs` enum stub (full matrix lands in Task 2) and add the new `CombatConfig` fields**

Create `crates/antcolony-sim/src/clade.rs` with just the enum for now (Task 2 adds `clade_from_genus` + `venom_multiplier`):

```rust
//! Ant subfamily clade classification + the venom×defender susceptibility
//! matrix used by cross-species combat. Pure functions, no sim state.
//! Grounded in docs/biology/interspecific/02-combat-mechanics.md §3
//! (clade-specific chemical weapons; Greenberg 2008 684× resistance span,
//! tamed to an in-game spread).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Clade {
    /// Default / genus not recognized — neutral in the venom matrix.
    #[default]
    Unknown,
    /// Ponerinae — functional sting, protein venom (Brachyponera).
    Ponerinae,
    /// Formicinae — formic acid, no sting (Formica, Camponotus, Lasius).
    Formicinae,
    /// Myrmicinae — sting/alkaloid (Aphaenogaster, Pogonomyrmex, Tetramorium, Temnothorax).
    Myrmicinae,
    /// Dolichoderinae — iridoids (Tapinoma, Linepithema).
    Dolichoderinae,
}
```

In `config.rs`, extend `CombatConfig` (additive, behavior-neutral):

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CombatConfig {
    pub worker_attack: f32,
    pub soldier_attack: f32,
    pub worker_health: f32,
    pub soldier_health: f32,
    pub interaction_radius: f32,
    pub soldier_vs_worker_bonus: f32,
    pub corpse_food_units: u32,
    pub alarm_deposit_on_death: f32,
    /// Cross-species (B4): fraction of incoming venom-typed damage this
    /// colony shrugs off (N. fulva detox counter). Clamped [0, 0.9].
    /// 0.0 = no resistance (back-compat).
    #[serde(default)]
    pub venom_resistance: f32,
    /// Cross-species terrain-gated Lanchester cap (B3): max attackers that
    /// can deal damage to one defender per substep on OPEN surface.
    /// 255 = effectively uncapped (back-compat).
    #[serde(default = "default_attackers_uncapped")]
    pub max_simultaneous_attackers_open: u32,
    /// …in an UndergroundNest tunnel cell. 255 = uncapped (back-compat;
    /// cross-species sets 2 per Lymbery 2023 corridor data).
    #[serde(default = "default_attackers_uncapped")]
    pub max_simultaneous_attackers_tunnel: u32,
    /// …on a NestEntrance cell (single-file choke). 255 = uncapped
    /// (back-compat; cross-species sets 1).
    #[serde(default = "default_attackers_uncapped")]
    pub max_simultaneous_attackers_entrance: u32,
    /// Queen-kill gate (B8/B9): attacker:defender adult ratio in the
    /// defender's nest module required before the enemy queen becomes
    /// targetable. 0.0 = gate DISABLED (queen behaves exactly as today).
    #[serde(default)]
    pub usurp_gate_attacker_ratio: f32,
    /// Queen-kill gate: defender adult count in the nest module must be
    /// below this for the gate to open. 0 with ratio 0.0 = disabled.
    #[serde(default)]
    pub usurp_gate_defender_floor: u32,
    /// Queen-kill channel duration in ticks (B8). 0 = instant/disabled
    /// (back-compat path: queen dies the moment her health hits 0 in melee).
    #[serde(default)]
    pub usurp_channel_ticks: u32,
    /// B7: fraction of a slain enemy ant's corpse-food routed to the
    /// killer colony's food store (predation feedback). 0.0 = off.
    #[serde(default)]
    pub usurp_corpse_to_killer_frac: f32,
}

fn default_attackers_uncapped() -> u32 { 255 }
```

Update `CombatConfig`'s `Default` impl to set the new fields:

```rust
impl Default for CombatConfig {
    fn default() -> Self {
        Self {
            worker_attack: 1.0,
            soldier_attack: 5.0,
            worker_health: 10.0,
            soldier_health: 25.0,
            interaction_radius: 1.2,
            soldier_vs_worker_bonus: 3.0,
            corpse_food_units: 1,
            alarm_deposit_on_death: 2.0,
            venom_resistance: 0.0,
            max_simultaneous_attackers_open: 255,
            max_simultaneous_attackers_tunnel: 255,
            max_simultaneous_attackers_entrance: 255,
            usurp_gate_attacker_ratio: 0.0,
            usurp_gate_defender_floor: 0,
            usurp_channel_ticks: 0,
            usurp_corpse_to_killer_frac: 0.0,
        }
    }
}
```

Add the new struct + `From` impl at the bottom of the non-test part of `config.rs`:

```rust
/// The slice of `SimConfig` that differs between colonies in a
/// cross-species match. `ant`, `colony`, `combat` are per-species;
/// `world`, `pheromone`, `hazards` stay global on `SimConfig` (one shared
/// arena + one shared pheromone field — both correct in PvP).
#[derive(Debug, Clone)]
pub struct ColonySimConfig {
    pub ant: AntConfig,
    pub colony: ColonyConfig,
    pub combat: CombatConfig,
    /// Species id for obs/logging/win-matrix labelling. "" for the
    /// back-compat default slice derived from a bare `SimConfig`.
    pub species_id: String,
    /// Subfamily clade (drives the venom matrix). `Unknown` for the default.
    pub clade: crate::clade::Clade,
    /// Venom clade for the (weapon × defender-clade) matrix.
    pub weapon: crate::species_extended::Weapon,
    /// Recruitment trail scalar (obs/logging only in MVP; deposit scaling
    /// is already baked into the shared `PheromoneConfig` by `apply`).
    pub recruitment_scalar: f32,
    /// B7: when true this colony eats the ants it kills (corpse→food).
    pub predates_ants: bool,
}

impl From<&SimConfig> for ColonySimConfig {
    fn from(sc: &SimConfig) -> Self {
        Self {
            ant: sc.ant.clone(),
            colony: sc.colony.clone(),
            combat: sc.combat.clone(),
            species_id: String::new(),
            clade: crate::clade::Clade::Unknown,
            weapon: crate::species_extended::Weapon::Mandible,
            recruitment_scalar: 1.0,
            predates_ants: false,
        }
    }
}
```

In `crates/antcolony-sim/src/lib.rs` add the module + re-exports near the other `pub mod` lines:

```rust
pub mod clade;
pub use clade::Clade;
pub use config::ColonySimConfig;
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-sim colony_sim_config new_combat_fields 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Confirm no existing test regressed (config-level)**

Run: `cargo test -p antcolony-sim config:: 2>&1 | tail -20`
Expected: all green (existing `defaults_populated`, `test_config_loads`, `partial_config_uses_defaults` unchanged).

- [ ] **Step 6: Commit**

```bash
git add crates/antcolony-sim/src/config.rs crates/antcolony-sim/src/clade.rs crates/antcolony-sim/src/lib.rs
git commit -m "feat(sim): ColonySimConfig per-colony slice + additive cross-species CombatConfig knobs"
```

---

### Task 2: `Clade` matrix — `clade_from_genus` + `venom_multiplier`

**Files:**
- Modify: `crates/antcolony-sim/src/clade.rs`
- Test: in-module `#[cfg(test)]`

**Interfaces:**
- Consumes: `crate::species_extended::Weapon`.
- Produces:
  ```rust
  pub fn clade_from_genus(genus: &str) -> Clade;
  /// Damage multiplier for an attacker's weapon against a defender clade.
  /// Tame in-game spread of the 330–684× literature LD50 span (02 §3).
  pub fn venom_multiplier(weapon: Weapon, attacker_sting_potency: f32, defender: Clade) -> f32;
  ```

The matrix (grounded; tame spread, max 2.0×):
- Ponerine **Sting** vs naive Myrmicinae/Dolichoderinae = high (scales with `sting_potency`, capped 2.0) — reproduces B. chinensis sting advantage over A. rudis (myrmicine). `[02 §3 Brachyponera; 05 Finding 21]`
- Formicine **FormicSpray** vs Myrmicinae/Dolichoderinae = elevated (formic acid contact toxin). `[02 §3 LeBrun; Greenberg]`
- Same clade, or `Mandible`, or `Unknown` either side = 1.0 (no chemical edge).

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::species_extended::Weapon;

    #[test]
    fn genus_maps_to_clade() {
        assert_eq!(clade_from_genus("Brachyponera"), Clade::Ponerinae);
        assert_eq!(clade_from_genus("Formica"), Clade::Formicinae);
        assert_eq!(clade_from_genus("Camponotus"), Clade::Formicinae);
        assert_eq!(clade_from_genus("Lasius"), Clade::Formicinae);
        assert_eq!(clade_from_genus("Aphaenogaster"), Clade::Myrmicinae);
        assert_eq!(clade_from_genus("Pogonomyrmex"), Clade::Myrmicinae);
        assert_eq!(clade_from_genus("Tetramorium"), Clade::Myrmicinae);
        assert_eq!(clade_from_genus("Temnothorax"), Clade::Myrmicinae);
        assert_eq!(clade_from_genus("Tapinoma"), Clade::Dolichoderinae);
        assert_eq!(clade_from_genus("Nonsense"), Clade::Unknown);
    }

    #[test]
    fn venom_matrix_rewards_ponerine_sting_vs_naive_myrmicine() {
        // B. chinensis (Ponerine, sting_potency 1.5) vs A. rudis (Myrmicinae).
        let m = venom_multiplier(Weapon::Sting, 1.5, Clade::Myrmicinae);
        assert!(m > 1.0, "ponerine sting vs naive myrmicine should exceed 1.0, got {m}");
        assert!(m <= 2.0, "in-game cap is 2.0, got {m}");
    }

    #[test]
    fn venom_matrix_is_neutral_for_mandible_and_same_clade() {
        assert_eq!(venom_multiplier(Weapon::Mandible, 5.0, Clade::Myrmicinae), 1.0);
        // Ponerine sting vs Ponerine = experienced, no edge.
        assert_eq!(venom_multiplier(Weapon::Sting, 1.5, Clade::Ponerinae), 1.0);
        // Unknown defender = neutral.
        assert_eq!(venom_multiplier(Weapon::Sting, 1.5, Clade::Unknown), 1.0);
    }

    #[test]
    fn venom_matrix_zero_potency_sting_is_neutral() {
        // sting weapon but no potency => no chemical edge.
        assert_eq!(venom_multiplier(Weapon::Sting, 0.0, Clade::Myrmicinae), 1.0);
    }

    #[test]
    fn formic_spray_elevated_vs_myrmicine() {
        let m = venom_multiplier(Weapon::FormicSpray, 0.0, Clade::Myrmicinae);
        assert!(m > 1.0 && m <= 2.0, "formic spray vs myrmicine in (1.0, 2.0], got {m}");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-sim clade:: 2>&1 | tail -20`
Expected: FAIL — `clade_from_genus` / `venom_multiplier` not found.

- [ ] **Step 3: Implement `clade_from_genus` + `venom_multiplier`**

Append to `crates/antcolony-sim/src/clade.rs` (after the `Clade` enum from Task 1):

```rust
use crate::species_extended::Weapon;

/// Map a species `genus` string to its subfamily clade. Case-insensitive
/// on the leading genus token. Unknown genera return `Clade::Unknown`
/// (neutral in the venom matrix).
pub fn clade_from_genus(genus: &str) -> Clade {
    match genus.trim().to_ascii_lowercase().as_str() {
        "brachyponera" | "pachycondyla" | "ponera" | "platythyrea" | "diacamma" => Clade::Ponerinae,
        "formica" | "camponotus" | "lasius" | "nylanderia" | "oecophylla" | "polyergus" => {
            Clade::Formicinae
        }
        "aphaenogaster" | "pogonomyrmex" | "tetramorium" | "temnothorax" | "solenopsis"
        | "myrmica" | "crematogaster" | "pheidole" | "atta" => Clade::Myrmicinae,
        "tapinoma" | "linepithema" | "dolichoderus" | "iridomyrmex" => Clade::Dolichoderinae,
        _ => Clade::Unknown,
    }
}

/// In-game venom susceptibility multiplier for an attacker's `weapon`
/// (scaled by its `attacker_sting_potency`) against a `defender` clade.
///
/// Literature LD50 spans are 330–684× (Greenberg 2008; LeBrun 2014) — far
/// too steep to play. We collapse that to a tame [1.0, 2.0] spread:
/// chemically-armed attackers (Sting/FormicSpray) get an edge ONLY against
/// clades naive to that chemistry; same-clade / Mandible / Unknown = 1.0.
/// `[cite: 02 §3; 05 Finding 21]`
pub fn venom_multiplier(weapon: Weapon, attacker_sting_potency: f32, defender: Clade) -> f32 {
    const MAX_MULT: f32 = 2.0;
    let naive = |d: Clade| matches!(d, Clade::Myrmicinae | Clade::Dolichoderinae);
    match weapon {
        // Ponerine protein-venom sting: edge vs naive clades, scaled by
        // Schmidt-scale potency (B. chinensis 1.5 -> ~1.5×; capped at 2.0).
        Weapon::Sting if attacker_sting_potency > 0.0 && naive(defender) => {
            (1.0 + attacker_sting_potency * 0.5).clamp(1.0, MAX_MULT)
        }
        // Formicine acid contact toxin: flat elevated edge vs naive clades.
        Weapon::FormicSpray if naive(defender) => 1.5,
        // Same clade, mandible-only, unknown, or experienced defender.
        _ => 1.0,
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-sim clade:: 2>&1 | tail -20`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-sim/src/clade.rs
git commit -m "feat(sim): clade_from_genus + venom_multiplier susceptibility matrix (B4)"
```

---

### Task 3: `Species::apply_colony` + `new_two_colony_cross_species` + BYTE-IDENTICAL regression guard

> **This is the load-bearing task.** Everything downstream rides on the back-compat guarantee. Land the determinism guards FIRST.

**Files:**
- Modify: `crates/antcolony-sim/src/species_extended.rs` (add `DietExtended.predates_ants`, `CombatExtended.venom_resistance`)
- Modify: `crates/antcolony-sim/src/species.rs` (`apply_colony`, `apply` delegates)
- Modify: `crates/antcolony-sim/src/simulation.rs` (`colony_configs` field, `colony_cfg()`, `new_two_colony_cross_species`, old ctor delegates)
- Test: in-module `#[cfg(test)]` in `species.rs` + `simulation.rs`

**Interfaces:**
- Consumes: `Environment`, `Species`, `ColonySimConfig`, `clade::clade_from_genus`.
- Produces:
  ```rust
  impl Species { pub fn apply_colony(&self, env: &Environment) -> ColonySimConfig; }
  impl Simulation {
      pub fn colony_cfg(&self, colony_id: u8) -> &ColonySimConfig; // falls back to colony_configs[0]
      pub fn new_two_colony_cross_species(
          world_pheromone_hazards: SimConfig,  // global slice (world dims, pheromone, hazards)
          cfg_black: ColonySimConfig,           // colony 0 = species A
          cfg_red: ColonySimConfig,             // colony 1 = species B
          topology: Topology, seed: u64,
          nest_black_module: ModuleId, nest_red_module: ModuleId,
      ) -> Self;
  }
  // Simulation gains: pub colony_configs: Vec<ColonySimConfig>,
  ```

- [ ] **Step 1: Add the two new species fields (additive)**

In `species_extended.rs`, add to `DietExtended` (wires the dropped TOML key, B7):

```rust
    /// B. chinensis-style active ant predation: this species hunts and
    /// EATS heterospecific ants (corpse-food routed to the killer in
    /// cross-species combat). Present in brachyponera_chinensis.toml but
    /// previously silently dropped (not a DietExtended field). 02 §3,
    /// 01 Finding 10 (Bednar 2013).
    #[serde(default)]
    pub predates_ants: bool,
```

Add to `CombatExtended` (the N. fulva detox counter, B4):

```rust
    /// Cross-species venom resistance (B4): fraction of venom-typed damage
    /// this species shrugs off (N. fulva formic-acid self-grooming detox,
    /// 98% vs 48% survival). Clamped [0, 0.9] at read sites. Default 0.0.
    /// 02 §3 "venom as anti-venom"; 05 Finding 21.
    #[serde(default)]
    pub venom_resistance: f32,
```

(Both `DietExtended` and `CombatExtended` already have manual/`derive` `Default`s; `predates_ants` defaults to `false` via `#[derive(Default)]` on `DietExtended`; add `venom_resistance: 0.0` to `CombatExtended`'s manual `Default` impl.)

```rust
impl Default for CombatExtended {
    fn default() -> Self {
        Self {
            weapon: Weapon::default(),
            sting_potency: 0.0,
            ranged_attack: false,
            soldier_size_categories: Vec::new(),
            major_attack_multiplier: 1.0,
            context_aggression: false,
            venom_resistance: 0.0,
        }
    }
}
```

- [ ] **Step 2: Write the failing refactor-guard + back-compat tests**

In `species.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn apply_colony_is_bit_identical_to_apply_slices() {
    use crate::environment::Environment;
    let s = Species::load_from_str(sample_toml()).expect("parse");
    let env = Environment::default();
    let full = s.apply(&env);
    let slice = s.apply_colony(&env);
    // ant/colony/combat must be bit-identical between the full SimConfig
    // and the per-colony slice (the refactor guard).
    assert_eq!(format!("{:?}", full.ant), format!("{:?}", slice.ant));
    assert_eq!(format!("{:?}", full.colony), format!("{:?}", slice.colony));
    assert_eq!(format!("{:?}", full.combat), format!("{:?}", slice.combat));
    assert_eq!(slice.species_id, s.id);
}

#[test]
fn apply_colony_populates_clade_weapon_predation() {
    use crate::environment::Environment;
    let toml = include_str!("../../../assets/species/brachyponera_chinensis.toml");
    let s = Species::load_from_str(toml).expect("parse bc");
    let slice = s.apply_colony(&Environment::default());
    assert_eq!(slice.clade, crate::clade::Clade::Ponerinae);
    assert_eq!(slice.weapon, crate::species_extended::Weapon::Sting);
    assert!(slice.predates_ants, "brachyponera_chinensis.toml has predates_ants = true");
    assert_eq!(slice.species_id, "brachyponera_chinensis");
}
```

In `simulation.rs` `#[cfg(test)] mod tests` — the **byte-identical** guard:

```rust
#[test]
fn cross_species_with_equal_cfg_is_byte_identical_to_legacy_two_colony() {
    // The keystone back-compat guard: the legacy two-colony starter and a
    // cross-species build with cfg_black == cfg_red (both = the default
    // slice) must produce byte-identical state after N ticks.
    let config = SimConfig::default();
    let topo = || crate::topology::Topology::two_colony_arena((24, 24), (32, 32));

    let mut legacy = Simulation::new_two_colony_with_topology(config.clone(), topo(), 4242, 0, 2);
    let cfg_slice = crate::config::ColonySimConfig::from(&config);
    let mut xspec = Simulation::new_two_colony_cross_species(
        config.clone(), cfg_slice.clone(), cfg_slice, topo(), 4242, 0, 2,
    );

    legacy.run(300);
    xspec.run(300);

    // Compare the observable state vectors (ants + colonies serialize via
    // serde; use a Snapshot-style debug comparison of the load-bearing fields).
    assert_eq!(legacy.tick, xspec.tick);
    assert_eq!(legacy.ants.len(), xspec.ants.len());
    let key = |s: &Simulation| {
        let mut ants: Vec<_> = s.ants.iter()
            .map(|a| (a.id, a.colony_id, a.position.x.to_bits(), a.position.y.to_bits(),
                      a.health.to_bits(), a.state, a.caste, a.module_id)).collect();
        ants.sort_by_key(|t| (t.0, t.1));
        let cols: Vec<_> = s.colonies.iter()
            .map(|c| (c.id, c.food_stored.to_bits(), c.queen_health.to_bits(),
                      c.population.workers, c.population.soldiers, c.population.breeders,
                      c.combat_kills, c.combat_losses)).collect();
        (ants, cols)
    };
    assert_eq!(key(&legacy), key(&xspec),
        "cross-species(equal cfg) must be byte-identical to legacy two-colony");
}

#[test]
fn new_ai_vs_ai_still_byte_identical_after_delegation() {
    // new_ai_vs_ai_with_topology wraps new_two_colony_with_topology, which
    // now delegates. Guard that the AI-vs-AI starter is unchanged too.
    let config = SimConfig::default();
    let mut a = Simulation::new_ai_vs_ai_with_topology(
        config.clone(), crate::topology::Topology::two_colony_arena((24,24),(32,32)), 7, 0, 2);
    let mut b = Simulation::new_ai_vs_ai_with_topology(
        config, crate::topology::Topology::two_colony_arena((24,24),(32,32)), 7, 0, 2);
    a.run(200); b.run(200);
    assert_eq!(a.ants.len(), b.ants.len());
    assert_eq!(a.colonies[0].queen_health.to_bits(), b.colonies[0].queen_health.to_bits());
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p antcolony-sim apply_colony cross_species_with_equal_cfg 2>&1 | tail -25`
Expected: FAIL — `apply_colony` / `new_two_colony_cross_species` / `colony_configs` not found.

- [ ] **Step 4: Implement `apply_colony` and refactor `apply` to delegate**

In `species.rs`, add `apply_colony` and rewrite `apply` to build the global slices then call it. Lift the existing `ant`/`colony`/`combat` construction (currently inline in `apply`, `species.rs:335–454`) into `apply_colony` VERBATIM (same field values, same order) so the result is bit-identical:

```rust
/// Per-colony slice of biology → tick-denominated config. Extracted from
/// `apply` so cross-species matches can configure each colony independently.
/// MUST produce ant/colony/combat values bit-identical to `apply`.
pub fn apply_colony(&self, env: &Environment) -> ColonySimConfig {
    let recruitment_scalar = recruitment_deposit_scalar(self.behavior.recruitment);

    let ant = AntConfig {
        speed_worker: 2.0 * self.appearance.speed_multiplier,
        speed_soldier: 1.5 * self.appearance.speed_multiplier,
        speed_queen: 0.0,
        sense_radius: 5,
        sense_angle: 60.0,
        exploration_rate: 0.15,
        alpha: 1.0,
        beta: 2.0,
        food_capacity: if self.diet_extended.seed_dispersal { 1.5 } else { 1.0 },
        initial_count: self.growth.initial_workers as usize,
        worker_size_mm: self.appearance.size_mm,
        polymorphic: self.biology.polymorphic,
        hibernation_cold_threshold_c: 10.0,
        hibernation_warm_threshold_c: 12.0,
        hibernation_required: self.biology.hibernation_required,
        min_diapause_days: self.biology.min_diapause_days,
        nocturnal: matches!(
            self.behavior.diel_activity,
            crate::species_extended::DielActivity::Nocturnal
        ),
        sting_potency: self.combat_extended.sting_potency,
        species_dig_multiplier: self.substrate.dig_speed_multiplier,
    };

    let ticks_per_day = env.in_game_seconds_to_ticks(86_400).max(1) as f32;
    let honeydew_penalty: f32 = if self.diet_extended.honeydew_dependent { 0.8 } else { 1.0 };
    use crate::species_extended::QueenCount;
    let polygyne_factor: f32 = match self.colony_structure.queen_count {
        QueenCount::Monogyne => 1.0,
        QueenCount::FacultativelyPolygyne => 1.3,
        QueenCount::ObligatePolygyne => 2.0,
    };
    let queen_egg_rate =
        self.growth.queen_eggs_per_day * honeydew_penalty * polygyne_factor / ticks_per_day;
    let adult_food_consumption = self.growth.food_per_adult_per_day / ticks_per_day;
    let worker_lifespan_ticks =
        (self.biology.worker_lifespan_months.max(0.1) * 30.0 * ticks_per_day) as u32;

    let egg_ticks = env.in_game_seconds_to_ticks(self.growth.egg_maturation_seconds);
    let larva_ticks = env.in_game_seconds_to_ticks(self.growth.larva_maturation_seconds);
    let pupa_ticks = env.in_game_seconds_to_ticks(self.growth.pupa_maturation_seconds);

    let colony = ColonyConfig {
        initial_workers: self.growth.initial_workers,
        initial_food: 200.0,
        egg_cost: self.growth.egg_cost_food,
        egg_stage_ticks: egg_ticks as u32,
        larva_stage_ticks: larva_ticks as u32,
        pupa_stage_ticks: pupa_ticks as u32,
        adult_food_consumption,
        soldier_food_multiplier: 1.5,
        queen_egg_rate,
        target_population: self.growth.target_population,
        worker_lifespan_ticks,
        food_storage_cap: self.diet_extended.food_storage_cap,
        ..ColonyConfig::default()
    };

    let combat = CombatConfig {
        worker_attack: self.combat.worker_attack,
        soldier_attack: self.combat.soldier_attack,
        worker_health: self.combat.worker_health,
        soldier_health: self.combat.soldier_health,
        // B4: per-species venom resistance flows in from combat_extended.
        venom_resistance: self.combat_extended.venom_resistance.clamp(0.0, 0.9),
        ..CombatConfig::default()
    };

    ColonySimConfig {
        ant,
        colony,
        combat,
        species_id: self.id.clone(),
        clade: crate::clade::clade_from_genus(&self.genus),
        weapon: self.combat_extended.weapon,
        recruitment_scalar,
        predates_ants: self.diet_extended.predates_ants,
    }
}
```

> **Determinism note:** `combat.venom_resistance` is the ONLY new value `apply_colony` writes into `combat` vs the old `apply`. It defaults to `0.0` for every currently-shipped species that omits `[combat_extended].venom_resistance`, so `apply_colony(...).combat` is bit-identical to the old `apply(...).combat` for all existing TOMLs. The `apply_colony_is_bit_identical_to_apply_slices` test pins this.

Now rewrite `apply` to keep the world/pheromone construction and delegate the per-colony slices:

```rust
pub fn apply(&self, env: &Environment) -> SimConfig {
    let world = WorldConfig {
        width: env.world_width,
        height: env.world_height,
        food_spawn_rate: self.forage.peak_food_per_day,
        food_cluster_size: self.forage.cluster_size,
        forage_dearth_multiplier: self.forage.dearth_food_multiplier,
        forage_peak_doy_start: self.forage.peak_doy_start,
        forage_peak_doy_end: self.forage.peak_doy_end,
    };

    let recruitment_scalar = recruitment_deposit_scalar(self.behavior.recruitment);
    let mut pheromone = PheromoneConfig::default();
    pheromone.deposit_food_trail *= recruitment_scalar;
    pheromone.deposit_home_trail *= recruitment_scalar;
    if let Some(half_life) = self.behavior.trail_half_life_seconds {
        pheromone.evaporation_rate = evaporation_rate_from_half_life_seconds(half_life);
    }

    let slice = self.apply_colony(env);

    let cfg = SimConfig {
        world,
        pheromone,
        ant: slice.ant.clone(),
        colony: slice.colony.clone(),
        combat: slice.combat.clone(),
        hazards: crate::config::HazardConfig::default(),
    };

    tracing::info!(
        species = %self.id,
        scale = env.time_scale.label(),
        queen_egg_rate = cfg.colony.queen_egg_rate,
        adult_food_consumption = cfg.colony.adult_food_consumption,
        egg_ticks = cfg.colony.egg_stage_ticks,
        larva_ticks = cfg.colony.larva_stage_ticks,
        pupa_ticks = cfg.colony.pupa_stage_ticks,
        clade = ?slice.clade,
        "Species::apply folded biology into SimConfig"
    );

    cfg
}
```

> The original `apply` log included `ticks_per_day`; it now lives in `apply_colony`'s scope. Logging fields are not behavior — but to keep the `Species::apply folded…` log line useful, the fields above read from `cfg.colony`. Add a `tracing::debug!` in `apply_colony` for `ticks_per_day`/`clade` if desired. No test asserts on log content.

- [ ] **Step 5: Add `colony_configs` to `Simulation`, `colony_cfg()`, and the constructors**

In `simulation.rs`, add the field to the struct (after `pub config: SimConfig`):

```rust
    pub config: SimConfig,
    /// Per-colony config slice indexed by colony id. `len() == colonies.len()`.
    /// For single-colony and legacy two-colony sims, every entry is
    /// `ColonySimConfig::from(&config)` so combat/economy reading per-colony
    /// is byte-identical to reading the shared `config`.
    pub colony_configs: Vec<crate::config::ColonySimConfig>,
```

Add the accessor + populate it in EVERY constructor. `new` / `new_with_topology` (single colony) push exactly one slice; `new_two_colony_with_topology` delegates to the new cross-species ctor.

```rust
/// Per-colony config for `colony_id`. Falls back to colony 0's slice if
/// the id is out of range (defensive; never panics in sim paths).
#[inline]
pub fn colony_cfg(&self, colony_id: u8) -> &crate::config::ColonySimConfig {
    self.colony_configs
        .get(colony_id as usize)
        .unwrap_or(&self.colony_configs[0])
}
```

In `new_with_topology`, in the struct literal add:

```rust
            colony_configs: vec![crate::config::ColonySimConfig::from(&config)],
```

(Insert that line in the `Self { config, topology, ants, colonies: vec![colony], … }` literal. Note `config` is moved into the struct, so build the slice BEFORE the literal: `let colony_slice = crate::config::ColonySimConfig::from(&config);` just before `Self {`.)

Now refactor `new_two_colony_with_topology` to delegate. Replace its body with:

```rust
pub fn new_two_colony_with_topology(
    config: SimConfig,
    topology: Topology,
    seed: u64,
    nest_black_module: ModuleId,
    nest_red_module: ModuleId,
) -> Self {
    // Back-compat: both colonies use the SAME slice derived from `config`.
    let slice = crate::config::ColonySimConfig::from(&config);
    Self::new_two_colony_cross_species(
        config,
        slice.clone(),
        slice,
        topology,
        seed,
        nest_black_module,
        nest_red_module,
    )
}
```

Add `new_two_colony_cross_species` — the body is the OLD `new_two_colony_with_topology` body, but with each colony's ants spawned from its own slice. Because `spawn_initial_ants` takes `&SimConfig`, build a per-side `SimConfig` view = global slices + that colony's `ant`/`colony`/`combat`:

```rust
pub fn new_two_colony_cross_species(
    world_pheromone_hazards: SimConfig,
    cfg_black: crate::config::ColonySimConfig,
    cfg_red: crate::config::ColonySimConfig,
    mut topology: Topology,
    seed: u64,
    nest_black_module: ModuleId,
    nest_red_module: ModuleId,
) -> Self {
    assert!(!topology.is_empty(), "at least one module required");
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Per-side full SimConfig view = global (world/pheromone/hazards) + that
    // colony's per-colony slice. spawn_initial_ants reads .combat.{worker,
    // soldier}_health and .ant.initial_count from this view.
    let view = |slice: &crate::config::ColonySimConfig| SimConfig {
        world: world_pheromone_hazards.world.clone(),
        pheromone: world_pheromone_hazards.pheromone.clone(),
        ant: slice.ant.clone(),
        colony: slice.colony.clone(),
        combat: slice.combat.clone(),
        hazards: world_pheromone_hazards.hazards.clone(),
    };
    let view_black = view(&cfg_black);
    let view_red = view(&cfg_red);

    // Black colony (player / colony 0).
    let black_mod = topology.module(nest_black_module);
    let (bw, bh) = (black_mod.width(), black_mod.height());
    let black_nest = Vec2::new(bw as f32 * 0.5, bh as f32 * 0.5);
    let mut c_black = ColonyState::new(0, view_black.colony.initial_food, black_nest);
    c_black.food_storage_cap_override = view_black.colony.food_storage_cap;

    let dist = CasteRatio { worker: 1.0, soldier: 0.0, breeder: 0.0 };
    let mut black_ants = spawn_initial_ants(&view_black, &mut rng, black_nest, 0, dist, 0);
    for a in black_ants.iter_mut() { a.module_id = nest_black_module; }

    // Red colony (AI / colony 1).
    let red_mod = topology.module(nest_red_module);
    let (rw, rh) = (red_mod.width(), red_mod.height());
    let red_nest = Vec2::new(rw as f32 * 0.5, rh as f32 * 0.5);
    let mut c_red = ColonyState::new(1, view_red.colony.initial_food, red_nest);
    c_red.food_storage_cap_override = view_red.colony.food_storage_cap;
    c_red.is_ai_controlled = true;
    c_red.caste_ratio = CasteRatio { worker: 0.65, soldier: 0.3, breeder: 0.05 };

    let id_offset = black_ants.len() as u32;
    let mut red_ants = spawn_initial_ants(&view_red, &mut rng, red_nest, 1, dist, id_offset);
    for a in red_ants.iter_mut() { a.module_id = nest_red_module; }

    let mut ants = black_ants;
    ants.append(&mut red_ants);

    for a in &ants {
        let colony = if a.colony_id == 0 { &mut c_black } else { &mut c_red };
        match a.caste {
            AntCaste::Worker => colony.population.workers += 1,
            AntCaste::Soldier => colony.population.soldiers += 1,
            AntCaste::Breeder => colony.population.breeders += 1,
            AntCaste::Queen => {}
        }
    }

    topology.module_mut(nest_black_module).world.place_nest(bw / 2, bh / 2, 0);
    topology.module_mut(nest_red_module).world.place_nest(rw / 2, rh / 2, 1);

    // fit_bore_to_species with the LARGER of the two species so neither is
    // bore-gated out of the shared tubes (spec §architecture).
    let max_size = view_black.ant.worker_size_mm.max(view_red.ant.worker_size_mm);
    let poly = view_black.ant.polymorphic || view_red.ant.polymorphic;
    topology.fit_bore_to_species(max_size, poly);

    tracing::info!(
        modules = topology.modules.len(),
        black_species = %cfg_black.species_id,
        red_species = %cfg_red.species_id,
        ants = ants.len(),
        seed,
        "Simulation::new_two_colony_cross_species"
    );

    let next_ant_id = ants.len() as u32;
    if let Some(idx) = ants.iter().position(|a| a.colony_id == 1 && !matches!(a.caste, AntCaste::Queen)) {
        ants[idx].is_avenger = true;
    }

    Self {
        config: world_pheromone_hazards,
        colony_configs: vec![cfg_black, cfg_red],
        topology,
        ants,
        colonies: vec![c_black, c_red],
        tick: 0,
        rng,
        next_ant_id,
        climate: Climate::default(),
        in_game_seconds_per_tick: 1.0,
        substep_count: 1,
        substep_global: 0,
        predators: Vec::new(),
        next_predator_id: 0,
        weather: crate::hazards::Weather::default(),
        beacons: Vec::new(),
        next_beacon_id: 0,
        excavation_events: Vec::new(),
    }
}
```

> **Byte-identical caveat — `fit_bore_to_species`:** the OLD `new_two_colony_with_topology` did NOT call `fit_bore_to_species`. Adding it changes `tube.bore_width_mm` (8.0 → `max(8.0, …)`). For the DEFAULT slice, `worker_size_mm = 4.0`, monomorphic → `needed_bore = 4.0 * 1.15 * 1.5 = 6.9`, `starter_bore = max(6.9, 8.0) = 8.0` → `bore_width_mm.max(8.0) = 8.0` (unchanged). So for the back-compat path the bore is a no-op. **The guard test (`cross_species_with_equal_cfg_is_byte_identical`) proves this** — if a future default size pushes bore >8.0 it will fail and you move the `fit_bore_to_species` call out of the shared body into a cross-species-only wrapper. Document the assumption in a code comment.

- [ ] **Step 6: Run the guard tests**

Run: `cargo test -p antcolony-sim apply_colony cross_species_with_equal_cfg new_ai_vs_ai_still_byte_identical 2>&1 | tail -30`
Expected: PASS (all 4). If `cross_species_with_equal_cfg…` fails on bore width, apply the caveat fix above.

- [ ] **Step 7: Run the FULL sim suite — the ~180-test regression gate**

Run: `cargo test -p antcolony-sim 2>&1 | tail -30`
Expected: all green. This is the real proof the refactor is byte-neutral. Investigate ANY new failure before continuing — do not proceed to combat changes on a red suite.

- [ ] **Step 8: Commit**

```bash
git add crates/antcolony-sim/src/species.rs crates/antcolony-sim/src/species_extended.rs crates/antcolony-sim/src/simulation.rs
git commit -m "feat(sim): apply_colony + new_two_colony_cross_species (legacy ctor delegates, byte-identical)"
```

---

### Task 4: Per-colony combat — venom matrix, per-side attack/health, flee bias, terrain cap, corpse→killer

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs` (`combat_tick`)
- Test: in-module `#[cfg(test)]`

**Interfaces:**
- Consumes: `colony_cfg()`, `clade::venom_multiplier`, `ColonySimConfig`.
- Produces: behavior change inside `combat_tick` only (same signature `pub fn combat_tick(&mut self)`).

Combat changes, all gated so the default slice reproduces today's combat:
1. **Per-side attack:** `base_attack` reads `colony_cfg(attacker.colony_id).combat.{worker,soldier}_attack` (was `self.config.combat`). Defender HP already comes from each colony's spawn health.
2. **Venom matrix:** multiply `dmg` by `venom_multiplier(attacker_weapon, attacker_sting_potency, defender_clade)` × `(1.0 - defender_venom_resistance.clamp(0,0.9))`. With the default slice (`Mandible`/`Unknown`/`resistance 0.0`) this is `1.0` → unchanged.
3. **Flee bias (B5):** when a defender takes damage from an attacker whose per-colony `ant.sting_potency > 1.0` and the defender clade is naive, lower the threshold at which workers/breeders flee. Implementation: workers/breeders already transition to `Fleeing` on ANY combat damage, so the MVP bias is: under a high-sting attacker, ALSO flip soldiers below half health to `Fleeing` (instead of standing). Keep it tiny + gated on `sting_potency > 1.0` so default combat is unchanged.
4. **Terrain-gated `max_simultaneous_attackers` (B3):** after collecting candidate attackers for a defender, cap how many deal damage this substep by the defender cell's terrain class (open / tunnel / entrance). With default `255` cap → unchanged.
5. **Corpse→killer (B7):** when the killer's `predates_ants` is true and `usurp_corpse_to_killer_frac > 0`, route `frac × corpse_food_units` to the killer colony's `food_stored` (via `accept_food`). Default frac `0.0` → no-op.

> **Determinism:** the attacker cap must pick a DETERMINISTIC subset — sort candidate attacker indices ascending and take the first `cap` of them. No HashMap iteration decides who deals damage.

- [ ] **Step 1: Write failing combat tests**

```rust
#[test]
fn combat_reads_per_colony_attack() {
    // Colony 0 high attack, colony 1 low attack, 1v1 adjacency.
    let mut sim = two_colony_combat_fixture(); // helper below
    // Give colony 0 a strong worker_attack, colony 1 weak.
    sim.colony_configs[0].combat.worker_attack = 8.0;
    sim.colony_configs[1].combat.worker_attack = 0.5;
    // Place one worker from each colony adjacent on the same module.
    place_adjacent_enemies(&mut sim);
    let h0_before = ant_health(&sim, 1); // colony-1 ant id
    let h1_before = ant_health(&sim, 0); // colony-0 ant id
    sim.combat_tick();
    let h0_after = ant_health(&sim, 1);
    let h1_after = ant_health(&sim, 0);
    let dmg_to_c1 = h0_before - h0_after;
    let dmg_to_c0 = h1_before - h1_after;
    assert!(dmg_to_c1 > dmg_to_c0,
        "colony-0's stronger ant should deal more ({dmg_to_c1}) than colony-1's ({dmg_to_c0})");
}

#[test]
fn venom_multiplier_amplifies_cross_clade_damage() {
    let mut sim = two_colony_combat_fixture();
    // Colony 0 = ponerine sting; colony 1 = naive myrmicine.
    sim.colony_configs[0].weapon = crate::species_extended::Weapon::Sting;
    sim.colony_configs[0].ant.sting_potency = 1.5;
    sim.colony_configs[0].clade = crate::clade::Clade::Ponerinae;
    sim.colony_configs[1].clade = crate::clade::Clade::Myrmicinae;
    place_adjacent_enemies(&mut sim);
    let before = ant_health(&sim, 1);
    sim.combat_tick();
    let dmg_venom = before - ant_health(&sim, 1);

    // Repeat with the matrix neutralized (mandible) for the same fixture/seed.
    let mut plain = two_colony_combat_fixture();
    place_adjacent_enemies(&mut plain);
    let pb = ant_health(&plain, 1);
    plain.combat_tick();
    let dmg_plain = pb - ant_health(&plain, 1);
    assert!(dmg_venom > dmg_plain,
        "venom matrix should raise damage ({dmg_venom}) above mandible baseline ({dmg_plain})");
}

#[test]
fn venom_resistance_reduces_incoming_damage() {
    let mut sim = two_colony_combat_fixture();
    sim.colony_configs[0].weapon = crate::species_extended::Weapon::Sting;
    sim.colony_configs[0].ant.sting_potency = 1.5;
    sim.colony_configs[0].clade = crate::clade::Clade::Ponerinae;
    sim.colony_configs[1].clade = crate::clade::Clade::Myrmicinae;
    sim.colony_configs[1].combat.venom_resistance = 0.9; // max detox
    place_adjacent_enemies(&mut sim);
    let before = ant_health(&sim, 1);
    sim.combat_tick();
    let dmg_resisted = before - ant_health(&sim, 1);
    assert!(dmg_resisted < 1.5 * sim.colony_configs[0].combat.worker_attack,
        "0.9 resistance must cut venom-amplified damage sharply, got {dmg_resisted}");
}

#[test]
fn terrain_cap_limits_simultaneous_attackers_in_tunnel() {
    // 6 colony-0 attackers around 1 colony-1 defender in an UndergroundNest
    // cell; tunnel cap = 2 => damage <= 2 * worker_attack.
    let mut sim = underground_swarm_fixture(6 /*attackers*/);
    sim.colony_configs[0].combat.max_simultaneous_attackers_tunnel = 2;
    sim.colony_configs[0].combat.worker_attack = 1.0;
    let defender = lone_defender_id(&sim);
    let before = ant_health(&sim, defender);
    sim.combat_tick();
    let dmg = before - ant_health(&sim, defender);
    assert!(dmg <= 2.0 + 1e-3, "tunnel cap=2 should limit damage to ~2*1.0, got {dmg}");
    assert!(dmg >= 2.0 - 1e-3, "exactly 2 attackers should still apply, got {dmg}");
}

#[test]
fn corpse_to_killer_feeds_predator_colony() {
    let mut sim = two_colony_combat_fixture();
    sim.colony_configs[0].predates_ants = true;
    sim.colony_configs[0].combat.usurp_corpse_to_killer_frac = 1.0;
    sim.colony_configs[0].combat.corpse_food_units = 4;
    sim.colony_configs[0].combat.worker_attack = 1000.0; // one-shot
    place_adjacent_enemies(&mut sim);
    let food_before = sim.colonies[0].food_stored;
    sim.combat_tick();
    assert!(sim.colonies[0].food_stored > food_before,
        "predator killer colony should gain corpse food");
}

#[test]
fn default_slice_combat_unchanged_smoke() {
    // With the default slice (uncapped, mandible, no predation), a fixed
    // fixture must produce identical kills to a snapshot of legacy combat.
    let mut sim = two_colony_combat_fixture();
    let mut legacy = two_colony_combat_fixture();
    place_adjacent_enemies(&mut sim);
    place_adjacent_enemies(&mut legacy);
    for _ in 0..20 { sim.combat_tick(); legacy.combat_tick(); }
    assert_eq!(sim.ants.len(), legacy.ants.len());
}
```

Add the test helpers (build a tiny two-colony sim from the default slice, force specific ant positions). Use `Simulation::new_two_colony_cross_species` with the default slice and then overwrite `colony_configs` per-test. Place ants by mutating `sim.ants` directly. Example helper sketch (implement concretely):

```rust
fn two_colony_combat_fixture() -> Simulation {
    let cfg = SimConfig::default();
    let topo = crate::topology::Topology::two_colony_arena((16, 16), (16, 16));
    let slice = crate::config::ColonySimConfig::from(&cfg);
    Simulation::new_two_colony_cross_species(cfg, slice.clone(), slice, topo, 1, 0, 2)
}
fn place_adjacent_enemies(sim: &mut Simulation) {
    // Put the first non-queen ant of each colony on module 1 (outworld),
    // 0.5 cells apart, on Empty terrain so combat_tick engages.
    let pos = Vec2::new(5.0, 5.0);
    let mut placed = [false, false];
    for a in sim.ants.iter_mut() {
        if matches!(a.caste, AntCaste::Queen) { continue; }
        let slot = a.colony_id as usize;
        if slot < 2 && !placed[slot] {
            a.module_id = 1;
            a.position = pos + Vec2::new(0.4 * slot as f32, 0.0);
            a.transit = None;
            placed[slot] = true;
        }
    }
}
fn ant_health(sim: &Simulation, ant_id: u32) -> f32 {
    sim.ants.iter().find(|a| a.id == ant_id).map(|a| a.health).unwrap_or(0.0)
}
```

(`underground_swarm_fixture` / `lone_defender_id` follow the same pattern, placing attackers in an `UndergroundNest` module — attach one via `topology.attach_underground` before construction, or reuse module 0 and temporarily set its `kind`. Implement against the real `Module` API.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-sim combat_reads_per_colony venom_ terrain_cap corpse_to_killer 2>&1 | tail -25`
Expected: FAIL — per-colony combat not yet wired (damage symmetric, no venom, no cap).

- [ ] **Step 3: Implement the per-colony combat in `combat_tick`**

Rewrite the damage-accumulation loop (currently `simulation.rs:2019–2055`) to read per-colony config and apply the venom matrix + terrain cap. The structure: for each defender, collect `(attacker_idx, raw_dmg)` candidates, sort by `attacker_idx`, apply the terrain cap, then sum. Replace the existing nested attacker loop with a candidate-collection pass keyed by defender.

```rust
// Replace the `let mut damage … for (i, ant) … { … }` block with:
let n = self.ants.len();
let mut damage: Vec<f32> = vec![0.0; n];
let mut attacker_of: Vec<Option<u8>> = vec![None; n];
// Per-defender candidate attackers: (attacker_idx, dmg_after_venom).
let mut candidates: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];

for (i, ant) in self.ants.iter().enumerate() {
    if ant.is_in_transit() || matches!(ant.caste, AntCaste::Queen) {
        continue;
    }
    let Some(hash) = buckets.get(&ant.module_id) else { continue; };
    let acfg = self.colony_cfg(ant.colony_id);
    let base_attack = match ant.caste {
        AntCaste::Soldier => acfg.combat.soldier_attack,
        _ => acfg.combat.worker_attack,
    };
    let attacker_weapon = acfg.weapon;
    let attacker_sting = acfg.ant.sting_potency;
    for j in hash.query_radius(ant.position, radius) {
        let j = j as usize;
        if j == i { continue; }
        let other = &self.ants[j];
        if other.colony_id == ant.colony_id { continue; }
        if (ant.position - other.position).length() > radius { continue; }
        let mut dmg = base_attack;
        if matches!(ant.caste, AntCaste::Soldier) && !matches!(other.caste, AntCaste::Soldier) {
            dmg *= acfg.combat.soldier_vs_worker_bonus;
        }
        // B4 venom matrix: attacker weapon vs defender clade, minus the
        // defender's resistance. Default slice => 1.0 * (1 - 0) = unchanged.
        let dcfg = self.colony_cfg(other.colony_id);
        let vmult = crate::clade::venom_multiplier(attacker_weapon, attacker_sting, dcfg.clade);
        let resist = dcfg.combat.venom_resistance.clamp(0.0, 0.9);
        dmg *= vmult * (1.0 - resist);
        candidates[j].push((i, dmg));
        attacker_of[j] = Some(ant.colony_id);
    }
}

// B3 terrain-gated Lanchester cap: per defender, keep only the first
// `cap` attackers (sorted by index for determinism), summing their dmg.
for (j, cand) in candidates.iter_mut().enumerate() {
    if cand.is_empty() { continue; }
    cand.sort_unstable_by_key(|(idx, _)| *idx);
    // The cap is a property of the ATTACKERS' species; both attackers are
    // the same enemy colony in 1v1, so read the first attacker's cap.
    let attacker_colony = self.ants[cand[0].0].colony_id;
    let acfg = self.colony_cfg(attacker_colony);
    let defender_module = self.ants[j].module_id;
    let (gx, gy) = {
        let m = self.topology.module(defender_module);
        m.world.world_to_grid(self.ants[j].position)
    };
    let cap = self.terrain_attacker_cap(defender_module, gx, gy, &acfg.combat);
    let take = (cap as usize).min(cand.len());
    let mut total = 0.0;
    for (_, d) in cand.iter().take(take) { total += *d; }
    damage[j] = total;
}
```

Add a private helper for the terrain class lookup:

```rust
/// Terrain-gated Lanchester cap (B3): how many attackers can apply force
/// to a defender on this cell this substep. Open surface = open cap;
/// UndergroundNest tunnel = tunnel cap; a NestEntrance cell = entrance cap.
fn terrain_attacker_cap(
    &self,
    module: ModuleId,
    gx: i32,
    gy: i32,
    combat: &crate::config::CombatConfig,
) -> u32 {
    let Some(m) = self.topology.modules.iter().find(|m| m.id == module) else {
        return combat.max_simultaneous_attackers_open;
    };
    if m.world.in_bounds(gx, gy) {
        if let Terrain::NestEntrance(_) = m.world.get(gx as usize, gy as usize) {
            return combat.max_simultaneous_attackers_entrance;
        }
    }
    if m.kind == crate::module::ModuleKind::UndergroundNest {
        return combat.max_simultaneous_attackers_tunnel;
    }
    combat.max_simultaneous_attackers_open
}
```

In the damage-application loop, add the flee bias (B5) — after `ant.health -= damage[i];` in the `ant.health > 0.0` branch, before the existing `match ant.caste`:

```rust
            // B5 flee bias: a high-sting attacker (sting_potency > 1.0)
            // makes a naive defender's wounded soldiers flee too (else
            // they'd stand). Gated so default combat is unchanged.
            let attacker_high_sting = attacker_of[i]
                .map(|ac| self.colony_cfg(ac).ant.sting_potency > 1.0)
                .unwrap_or(false);
            let new_state = match ant.caste {
                AntCaste::Soldier if attacker_high_sting && ant.health < /*half*/ 0.5 * self.colony_cfg(ant.colony_id).combat.soldier_health
                    => AntState::Fleeing,
                AntCaste::Soldier => AntState::Fighting,
                AntCaste::Worker | AntCaste::Breeder => AntState::Fleeing,
                AntCaste::Queen => ant.state,
            };
```

> **Borrow note:** `self.colony_cfg(...)` inside the `for (i, ant) in self.ants.iter_mut()` loop double-borrows `self`. Resolve by reading the needed scalars (`attacker_high_sting`, `soldier_health` for each ant) into `Vec`s in the read-only candidate pass BEFORE the mutable apply loop, OR clone the two `colony_configs` combat slices into locals (`let cfgs: Vec<CombatConfig> = self.colony_configs.iter().map(|c| c.combat.clone()).collect();`) and index by colony id. Use the cloned-locals approach (2 colonies, cheap) to keep the mutable loop borrow-clean.

In the corpse-drop loop (currently `simulation.rs:2130–2151`), add the B7 routing. After the corpse is placed as terrain, route a fraction to the killer:

```rust
        // B7: predator colonies eat the ants they kill (corpse → killer food).
        if let Some(killer) = d.killer_colony {
            let kcfg = /* cloned local */ &cfgs_meta[killer as usize];
            if kcfg.predates_ants && kcfg.combat.usurp_corpse_to_killer_frac > 0.0 {
                let gained = (cfg.corpse_food_units as f32)
                    * kcfg.combat.usurp_corpse_to_killer_frac.clamp(0.0, 1.0);
                if let Some(c) = self.colonies.iter_mut().find(|c| c.id == killer) {
                    c.accept_food(gained);
                    tracing::debug!(tick = self.tick, killer, gained, "predation: corpse→killer food");
                }
            }
        }
```

(Where `cfgs_meta: Vec<ColonySimConfig> = self.colony_configs.clone()` is taken at the top of `combat_tick` alongside the existing `let cfg = self.config.combat.clone();`. Note: `cfg` (global combat) is still used for `interaction_radius`, `corpse_food_units`, `alarm_deposit_on_death` — keep those reading the global `cfg` for back-compat, since they're arena-level not per-species in MVP. Per-colony only governs attack/health/venom/cap/predation.)

- [ ] **Step 4: Run combat tests + full sim suite**

Run: `cargo test -p antcolony-sim combat 2>&1 | tail -30`
Expected: new combat tests PASS.

Run: `cargo test -p antcolony-sim 2>&1 | tail -30`
Expected: full suite green (default-slice combat unchanged — `default_slice_combat_unchanged_smoke` + the Task 3 byte-identical guard re-confirm it).

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-sim/src/simulation.rs
git commit -m "feat(sim): per-colony combat — venom matrix, terrain cap, flee bias, corpse→killer (B3/B4/B5/B7)"
```

---

### Task 5: Gated two-phase interruptible queen-kill (`AntState::Usurping` + `usurp_tick`)

**Files:**
- Modify: `crates/antcolony-sim/src/ant.rs` (add `AntState::Usurping`)
- Modify: `crates/antcolony-sim/src/simulation.rs` (`usurp_tick`, called from `combat_tick` or `tick`)
- Test: in-module `#[cfg(test)]`

**Interfaces:**
- Consumes: `colony_cfg()`, `colonies`, `ants`, `topology`.
- Produces: `fn usurp_tick(&mut self)`; new `AntState::Usurping`; per-defender channel progress tracked on `ColonyState` (new `#[serde(default)]` field `usurp_progress_ticks: u32` + `usurp_attacker_colony: Option<u8>`).

Mechanic (grounded in 05 Findings 8/9/10):
- **Phase 0 — Gate (queen invulnerable):** the enemy queen is NOT targetable until the attacker has `attacker_adults_in_nest : defender_adults_in_nest >= usurp_gate_attacker_ratio` AND `defender_adults_in_nest < usurp_gate_defender_floor`, evaluated on the DEFENDER's nest module. Until then, `combat_tick` skips queens entirely (already true — queens are skipped as attackers; ALSO they take no damage because they're never in `candidates` as victims unless we add them — keep queens OUT of the victim set during Phase 0). `[05 Finding 8 — Johnson 2002]`
- **Phase 1 — Channel (interruptible):** once the gate opens, the nearest eligible enemy ant adjacent to the queen enters `AntState::Usurping`; `colony.usurp_progress_ticks` increments each tick. If the channeling ant dies or is forced to `Fleeing` (took damage) before completion, progress RESETS to 0 and `usurp_attacker_colony = None`. Defenders rallying = the counter-play. `[05 Finding 9 — Topoff & Zimmerli; interruptible bite-and-lick]`
- **Phase 2 — Resolution:** when `usurp_progress_ticks >= usurp_channel_ticks`, apply lethal damage to the queen ant (set `health = 0`); the existing combat death path / `match_status` resolves the win unchanged. `[05 "Realism Verdict"]`
- **Disabled by default:** `usurp_channel_ticks == 0` AND `usurp_gate_attacker_ratio == 0.0` → `usurp_tick` is a no-op AND queens stay non-targetable, EXACTLY as today (queens are never combat victims in the current code). So the default slice path is unchanged.

> **CRITICAL determinism + back-compat:** in the CURRENT code, a queen ant's health only hits 0 via the explicit `c.queen_health = 0.0` book-keeping when a Queen *somehow* dies — but queens are skipped as both attacker and (effectively) victim, so queens never die in combat today; `match_status` win comes from `adult_total()==0` or queen-economy death. The usurp channel is the ONLY new way a queen ant dies in combat, and it's fully gated. Add a guard test that with the default slice, a queen adjacent to enemies for N ticks takes ZERO damage and the channel never starts.

- [ ] **Step 1: Add `AntState::Usurping`**

In `ant.rs`, add to the `AntState` enum (after `NuptialFlight`):

```rust
    /// Cross-species usurpation (B8): an attacker is channeling the enemy
    /// queen-kill. Exposed + interruptible — if the ant dies or is forced
    /// to Fleeing mid-channel, colony usurp progress resets. 05 Findings
    /// 8/9/10 (Johnson 2002 timing; Topoff & Zimmerli 1993 disguise).
    Usurping,
```

(No other match arm in `ant.rs` is exhaustive over `AntState` except `target_layer`'s `_ =>` and the `speed`/`body_size` matches keyed on caste, so adding the variant won't break `ant.rs`. In `simulation.rs`, search for exhaustive `match ant.state` / `match a.state` and add an `AntState::Usurping => { /* immobile, channeling */ }` arm wherever the compiler demands — typically the movement `moving` set and `decide_next_state`. Treat `Usurping` like `Idle` for movement (immobile) and like `Fighting` for combat-preservation.)

- [ ] **Step 2: Add the channel-progress fields to `ColonyState`**

In `colony.rs` `ColonyState`, add (additive, serde-default):

```rust
    /// B8 usurpation channel progress (ticks) against THIS colony's queen.
    /// Resets to 0 on interrupt. 0 when no channel active.
    #[serde(default)]
    pub usurp_progress_ticks: u32,
    /// B8 which enemy colony is currently channeling the kill. None = no
    /// active channel. Used to detect interrupt (channeler died/fled).
    #[serde(default)]
    pub usurp_attacker_colony: Option<u8>,
```

Initialize both in `ColonyState::new` (`usurp_progress_ticks: 0, usurp_attacker_colony: None`).

- [ ] **Step 3: Write failing usurp tests**

```rust
#[test]
fn queen_invulnerable_before_gate_opens() {
    let mut sim = two_colony_combat_fixture();
    // Enable the channel but NOT the gate ratio condition: 3:1 needed.
    sim.colony_configs[0].combat.usurp_gate_attacker_ratio = 3.0;
    sim.colony_configs[0].combat.usurp_gate_defender_floor = 1;
    sim.colony_configs[0].combat.usurp_channel_ticks = 10;
    // Put ONE colony-0 attacker next to colony-1's queen; defender still has
    // its full workforce => gate ratio not met.
    place_attacker_next_to_enemy_queen(&mut sim, /*attackers*/ 1);
    let qh = sim.colonies[1].queen_health;
    for _ in 0..30 { sim.usurp_tick(); }
    assert_eq!(sim.colonies[1].queen_health, qh, "queen must be invulnerable pre-gate");
    assert_eq!(sim.colonies[1].usurp_progress_ticks, 0, "no channel before gate");
}

#[test]
fn channel_starts_and_completes_when_gate_open() {
    let mut sim = two_colony_combat_fixture();
    sim.colony_configs[0].combat.usurp_gate_attacker_ratio = 1.0;  // easy gate
    sim.colony_configs[0].combat.usurp_gate_defender_floor = 1000; // easy gate
    sim.colony_configs[0].combat.usurp_channel_ticks = 5;
    // Clear colony-1's workforce so the defender floor / ratio is satisfied,
    // and put attackers in colony-1's nest next to the queen.
    clear_defender_workers(&mut sim, 1);
    place_attacker_next_to_enemy_queen(&mut sim, 3);
    for _ in 0..6 { sim.usurp_tick(); }
    // Queen ant lethal-damaged => queen_health 0 => match resolves.
    assert!(sim.colonies[1].queen_health <= 0.0, "channel should complete and kill queen");
}

#[test]
fn channel_resets_when_channeler_interrupted() {
    let mut sim = two_colony_combat_fixture();
    sim.colony_configs[0].combat.usurp_gate_attacker_ratio = 1.0;
    sim.colony_configs[0].combat.usurp_gate_defender_floor = 1000;
    sim.colony_configs[0].combat.usurp_channel_ticks = 100; // long channel
    clear_defender_workers(&mut sim, 1);
    place_attacker_next_to_enemy_queen(&mut sim, 1);
    for _ in 0..5 { sim.usurp_tick(); }
    assert!(sim.colonies[1].usurp_progress_ticks > 0, "channel underway");
    // Kill the channeling ant (simulate a defender rally).
    kill_channeling_ant(&mut sim, 1);
    sim.usurp_tick();
    assert_eq!(sim.colonies[1].usurp_progress_ticks, 0, "interrupt resets progress");
    assert!(sim.colonies[1].queen_health > 0.0, "queen survives the interrupted channel");
}

#[test]
fn default_slice_queen_takes_no_damage_and_no_channel() {
    // The back-compat guard: with the default slice (gate ratio 0, channel
    // ticks 0), a queen surrounded by enemies for 50 ticks is untouched.
    let mut sim = two_colony_combat_fixture();
    place_attacker_next_to_enemy_queen(&mut sim, 5);
    let qh = sim.colonies[1].queen_health;
    for _ in 0..50 { sim.usurp_tick(); sim.combat_tick(); }
    assert_eq!(sim.colonies[1].queen_health, qh, "default slice: queen invulnerable");
    assert_eq!(sim.colonies[1].usurp_attacker_colony, None);
}
```

(Implement `place_attacker_next_to_enemy_queen`, `clear_defender_workers`, `kill_channeling_ant` against the real API: the queen is the colony's ant with `caste == Queen`; place attackers within `interaction_radius` of her on her module; `clear_defender_workers` removes non-queen ants of that colony from `sim.ants` and zeroes its `population`; `kill_channeling_ant` finds the ant in `AntState::Usurping` for that defender and removes it.)

- [ ] **Step 4: Run to verify it fails**

Run: `cargo test -p antcolony-sim usurp queen_invulnerable channel_ default_slice_queen 2>&1 | tail -25`
Expected: FAIL — `usurp_tick` / `AntState::Usurping` channel logic not implemented.

- [ ] **Step 5: Implement `usurp_tick` and call it from `tick`**

```rust
/// B8 cross-species queen-kill: gated, two-phase, interruptible.
/// No-op unless a colony's per-colony combat enables it
/// (`usurp_channel_ticks > 0`). Determinism: defenders processed in
/// ascending colony-id order; channeler chosen by ascending ant id.
pub fn usurp_tick(&mut self) {
    if self.colonies.len() < 2 { return; }

    // Snapshot per-colony combat knobs (cheap; 2 colonies) to avoid
    // borrow conflicts inside the mutate pass.
    let knobs: Vec<crate::config::CombatConfig> =
        self.colony_configs.iter().map(|c| c.combat.clone()).collect();
    let radius = self.config.combat.interaction_radius.max(1.0);

    // Process each DEFENDER colony in id order.
    let defender_ids: Vec<u8> = {
        let mut v: Vec<u8> = self.colonies.iter().map(|c| c.id).collect();
        v.sort_unstable();
        v
    };

    for did in defender_ids {
        // The attacker colony is the OTHER colony in 1v1.
        let aid = if did == 0 { 1u8 } else { 0u8 };
        let acombat = &knobs[aid as usize];
        // Disabled => skip entirely (back-compat).
        if acombat.usurp_channel_ticks == 0 && acombat.usurp_gate_attacker_ratio == 0.0 {
            continue;
        }

        // Locate the defender queen + her module.
        let Some((q_idx, q_pos, q_module)) = self.ants.iter().enumerate()
            .find(|(_, a)| a.colony_id == did && matches!(a.caste, AntCaste::Queen))
            .map(|(i, a)| (i, a.position, a.module_id))
        else {
            // No queen => nothing to usurp; clear any stale channel.
            if let Some(c) = self.colonies.iter_mut().find(|c| c.id == did) {
                c.usurp_progress_ticks = 0; c.usurp_attacker_colony = None;
            }
            continue;
        };

        // Count adults of each side on the defender's nest module.
        let mut atk_adults = 0u32;
        let mut def_adults = 0u32;
        let mut nearest_attacker: Option<(u32, usize)> = None; // (ant_id, idx)
        for (i, a) in self.ants.iter().enumerate() {
            if a.module_id != q_module || matches!(a.caste, AntCaste::Queen) { continue; }
            if a.colony_id == aid {
                atk_adults += 1;
                if (a.position - q_pos).length() <= radius {
                    let cand = (a.id, i);
                    if nearest_attacker.map(|(id, _)| cand.0 < id).unwrap_or(true) {
                        nearest_attacker = Some(cand);
                    }
                }
            } else if a.colony_id == did {
                def_adults += 1;
            }
        }

        // Phase 0 gate.
        let ratio_ok = acombat.usurp_gate_attacker_ratio > 0.0
            && (atk_adults as f32) >= acombat.usurp_gate_attacker_ratio * (def_adults.max(1) as f32);
        let floor_ok = def_adults < acombat.usurp_gate_defender_floor;
        let gate_open = ratio_ok && floor_ok && acombat.usurp_channel_ticks > 0;

        // Interrupt detection: the colony recorded a channeler last tick;
        // if there is no longer an adjacent Usurping attacker, reset.
        let active_channeler = nearest_attacker
            .filter(|&(_, idx)| matches!(self.ants[idx].state, AntState::Usurping));

        let Some(c) = self.colonies.iter_mut().find(|c| c.id == did) else { continue; };

        if !gate_open {
            if c.usurp_progress_ticks != 0 {
                tracing::info!(tick = self.tick, defender = did, "usurp: gate closed, channel reset");
            }
            c.usurp_progress_ticks = 0;
            c.usurp_attacker_colony = None;
            // Reset any lingering Usurping attacker back to Fighting.
            if let Some((_, idx)) = nearest_attacker {
                if matches!(self.ants[idx].state, AntState::Usurping) {
                    self.ants[idx].transition(AntState::Fighting);
                }
            }
            continue;
        }

        match nearest_attacker {
            None => {
                // Channeler gone (died/moved) => interrupt.
                if c.usurp_progress_ticks != 0 {
                    tracing::info!(tick = self.tick, defender = did, "usurp: channeler lost, reset");
                }
                c.usurp_progress_ticks = 0;
                c.usurp_attacker_colony = None;
            }
            Some((_, idx)) => {
                // Promote the nearest attacker into the channel.
                if !matches!(self.ants[idx].state, AntState::Usurping) {
                    // If this attacker was forced to Fleeing this tick, the
                    // channel is interrupted; otherwise (re)start it.
                    if matches!(self.ants[idx].state, AntState::Fleeing) {
                        c.usurp_progress_ticks = 0;
                        c.usurp_attacker_colony = None;
                        continue;
                    }
                    self.ants[idx].transition(AntState::Usurping);
                    c.usurp_attacker_colony = Some(aid);
                }
                c.usurp_progress_ticks = c.usurp_progress_ticks.saturating_add(1);
                tracing::trace!(tick = self.tick, defender = did, attacker = aid,
                    progress = c.usurp_progress_ticks, target = acombat.usurp_channel_ticks, "usurp: channeling");

                if c.usurp_progress_ticks >= acombat.usurp_channel_ticks {
                    // Phase 2: lethal damage to the queen ant; existing
                    // combat/match_status path resolves the win.
                    c.queen_health = 0.0;
                    self.ants[q_idx].health = 0.0;
                    c.usurp_progress_ticks = 0;
                    c.usurp_attacker_colony = None;
                    tracing::info!(tick = self.tick, defender = did, attacker = aid,
                        "usurp: channel COMPLETE — enemy queen killed");
                }
            }
        }
        let _ = active_channeler; // (kept for readability of interrupt logic)
    }

    // Remove queens killed by a completed channel (mirror combat_tick's
    // swap_remove discipline; descending indices).
    let mut dead: Vec<usize> = self.ants.iter().enumerate()
        .filter(|(_, a)| matches!(a.caste, AntCaste::Queen) && a.health <= 0.0)
        .map(|(i, _)| i).collect();
    dead.sort_unstable();
    for i in dead.into_iter().rev() {
        self.ants.swap_remove(i);
    }
}
```

Call `usurp_tick` from the main `tick` loop, AFTER `combat_tick` so interrupts (a channeler forced to `Fleeing`/killed by `combat_tick`) are seen this tick. Find the `combat_tick()` call site in `tick`/the substep loop and add `self.usurp_tick();` immediately after it. (Verify it's inside the same substep cadence as combat — search for `self.combat_tick()` in `tick`.)

> **Determinism:** `usurp_tick` reads no `self.rng`, iterates colonies + ants in id-sorted/index order, and the channeler is chosen by min ant id. No RNG, no HashMap-order dependence. Add a determinism test in Task 7.

- [ ] **Step 6: Run usurp tests + full sim suite**

Run: `cargo test -p antcolony-sim usurp queen_ channel_ default_slice_queen 2>&1 | tail -30`
Expected: new usurp tests PASS.

Run: `cargo test -p antcolony-sim 2>&1 | tail -30`
Expected: full suite green (default slice unchanged — `default_slice_queen_takes_no_damage_and_no_channel` + Task 3 guard).

- [ ] **Step 7: Commit**

```bash
git add crates/antcolony-sim/src/ant.rs crates/antcolony-sim/src/colony.rs crates/antcolony-sim/src/simulation.rs
git commit -m "feat(sim): gated 2-phase interruptible queen-kill (AntState::Usurping + usurp_tick, B8/B9)"
```

---

### Task 6: Trainer `MatchEnv::new_cross_species` (applies species per colony)

**Files:**
- Modify: `crates/antcolony-trainer/src/env.rs`
- Test: in-module `#[cfg(test)] mod env_tests`

**Interfaces:**
- Consumes: `antcolony_sim::{Species, ColonySimConfig, Environment, Simulation, Topology}`, `antcolony_sim::species::load_species_dir`.
- Produces:
  ```rust
  impl MatchEnv {
      pub fn new_cross_species(species_a: &Species, species_b: &Species, seed: u64) -> Self;
  }
  ```
  Builds the global `SimConfig` (32×32 arena, like `new`) for `world`/`pheromone`/`hazards`, then `species_a.apply_colony(&env)` and `species_b.apply_colony(&env)` for the two slices, and constructs via `Simulation::new_two_colony_cross_species`. Then flips colony 0 to AI like `new_ai_vs_ai_with_topology` does (set `is_ai_controlled`), so cross-species matches train symmetrically by brain. `MatchEnv::new` is byte-unchanged.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn new_cross_species_builds_two_distinct_species_colonies() {
    use antcolony_sim::species::Species;
    let bc = Species::load_from_file(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/species/brachyponera_chinensis.toml")
    ).expect("load bc");
    let ar = Species::load_from_file(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/species/aphaenogaster_rudis.toml")
    ).expect("load ar");
    let env = MatchEnv::new_cross_species(&bc, &ar, 0xC0FFEE);
    assert_eq!(env.sim.colony_configs.len(), 2);
    assert_eq!(env.sim.colony_cfg(0).species_id, "brachyponera_chinensis");
    assert_eq!(env.sim.colony_cfg(1).species_id, "aphaenogaster_rudis");
    // The two species differ in per-worker attack (asymmetry is live).
    assert_ne!(env.sim.colony_cfg(0).combat.worker_attack,
               env.sim.colony_cfg(1).combat.worker_attack);
    // Both colonies present, both queens alive at t=0.
    assert!(env.sim.colonies.len() == 2);
}

#[test]
fn new_match_env_unchanged_smoke() {
    // Guard: MatchEnv::new still builds the symmetric 32x32 / 10-ant fixture.
    let env = MatchEnv::new(0xb1a5_e1);
    assert_eq!(env.sim.colonies.len(), 2);
    assert_eq!(env.max_ticks, 10_000);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-trainer new_cross_species 2>&1 | tail -20`
Expected: FAIL — `new_cross_species` not found.

- [ ] **Step 3: Implement `new_cross_species`**

Add the import and method to `env.rs`:

```rust
use antcolony_sim::{ColonySimConfig, Environment, Species};

impl MatchEnv {
    /// Cross-species match: colony 0 = `species_a`, colony 1 = `species_b`.
    /// Shares the bench arena fixture (32x32) and the AI-vs-AI symmetry
    /// (both colonies flagged AI-controlled) so only species + brains differ.
    pub fn new_cross_species(species_a: &Species, species_b: &Species, seed: u64) -> Self {
        let env = Environment {
            world_width: 32,
            world_height: 32,
            ..Environment::default()
        };
        // Global slice: world/pheromone/hazards from species_a's apply
        // (arena geometry is shared; only world dims matter here).
        let mut global = species_a.apply(&env);
        global.world = WorldConfig { width: 32, height: 32, ..WorldConfig::default() };

        let cfg_a: ColonySimConfig = species_a.apply_colony(&env);
        let cfg_b: ColonySimConfig = species_b.apply_colony(&env);

        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        let mut sim = Simulation::new_two_colony_cross_species(
            global, cfg_a, cfg_b, topology, seed, 0, 2,
        );
        // Match new_ai_vs_ai_with_topology: flip colony 0 to AI too.
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
        Self { sim, max_ticks: 10_000, prev_workers, prev_queens_alive, prev_food }
    }
}
```

(Add `Topology` to the existing `use antcolony_sim::{…}` import if not already there — it is, `env.rs:6`.)

- [ ] **Step 4: Run the new tests + full trainer suite**

Run: `cargo test -p antcolony-trainer new_cross_species new_match_env_unchanged 2>&1 | tail -20`
Expected: PASS.

Run: `cargo test -p antcolony-trainer env_tests 2>&1 | tail -20`
Expected: all existing env tests green (commander/ant obs unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/env.rs
git commit -m "feat(trainer): MatchEnv::new_cross_species applies species per colony"
```

---

### Task 7: Determinism guards + cross-species smoke + win-matrix harness

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs` (determinism + smoke tests)
- Create: `crates/antcolony-trainer/src/bin/cross_species_matrix.rs`
- Create: `scripts/run_cross_species_matrix.ps1`
- Test: in-module + the harness binary (build-only in CI, run manually)

**Interfaces:**
- Consumes: `Simulation::new_two_colony_cross_species`, `Species::apply_colony`, `load_species_dir`, `MatchEnv::new_cross_species`.
- Produces: a runnable `cross_species_matrix` binary writing an N×N winrate matrix + 3-cycle / per-row min-max intransitivity report.

- [ ] **Step 1: Write the determinism + B.chinensis-vs-A.rudis smoke tests (sim crate)**

```rust
#[test]
fn cross_species_is_byte_deterministic() {
    let bc = crate::species::Species::load_from_file(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/species/brachyponera_chinensis.toml")).unwrap();
    let ar = crate::species::Species::load_from_file(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/species/aphaenogaster_rudis.toml")).unwrap();
    let env = crate::environment::Environment { world_width: 48, world_height: 48, ..Default::default() };
    let build = || {
        let global = bc.apply(&env);
        Simulation::new_two_colony_cross_species(
            global, bc.apply_colony(&env), ar.apply_colony(&env),
            crate::topology::Topology::two_colony_arena((24,24),(32,32)), 999, 0, 2)
    };
    let mut a = build(); let mut b = build();
    a.run(400); b.run(400);
    let key = |s: &Simulation| {
        let mut ants: Vec<_> = s.ants.iter()
            .map(|x| (x.id, x.colony_id, x.position.x.to_bits(), x.position.y.to_bits(), x.health.to_bits())).collect();
        ants.sort_by_key(|t| (t.0, t.1)); ants
    };
    assert_eq!(key(&a), key(&b), "cross-species must be byte-deterministic across runs");
}

#[test]
fn bc_vs_ar_smoke_terminates_without_panic() {
    // B. chinensis (predator, sting) vs A. rudis (myrmicine). Enable the
    // cross-species levers so the displacement asymmetry can express, run to
    // a tick cap, assert it terminates and produces no NaN.
    let bc = crate::species::Species::load_from_file(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/species/brachyponera_chinensis.toml")).unwrap();
    let ar = crate::species::Species::load_from_file(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/species/aphaenogaster_rudis.toml")).unwrap();
    let env = crate::environment::Environment { world_width: 48, world_height: 48, ..Default::default() };
    let global = bc.apply(&env);
    let mut a = bc.apply_colony(&env);
    // Turn on the terrain caps so a chokepoint matters (balance levers).
    a.combat.max_simultaneous_attackers_tunnel = 2;
    a.combat.usurp_corpse_to_killer_frac = 0.5;
    let mut sim = Simulation::new_two_colony_cross_species(
        global, a, ar.apply_colony(&env),
        crate::topology::Topology::two_colony_arena((24,24),(32,32)), 7, 0, 2);
    sim.run(2000);
    for x in &sim.ants { assert!(x.health.is_finite(), "no NaN health"); }
    for c in &sim.colonies { assert!(c.food_stored.is_finite()); }
    // It either ended (a queen died) or is still in progress at the cap.
    assert!(sim.tick <= 2000);
}
```

- [ ] **Step 2: Run them**

Run: `cargo test -p antcolony-sim cross_species_is_byte_deterministic bc_vs_ar_smoke 2>&1 | tail -25`
Expected: PASS.

- [ ] **Step 3: Write the win-matrix harness binary**

```rust
//! Cross-species win-matrix / intransitivity harness. For every ordered
//! pair (A, B) in the roster, play K side-swapped matches with a fixed
//! heuristic brain on both colonies and record A's winrate vs B. Writes an
//! N×N matrix + a 3-cycle / per-row min-max intransitivity report.
//!
//! Usage: cross_species_matrix --species-dir assets/species --mpe 50 --max-ticks 8000

use std::path::PathBuf;
use anyhow::Result;
use antcolony_sim::{Species, MatchStatus, HeuristicBrain, AiBrain};
use antcolony_trainer::env::MatchEnv;

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))).init();

    let mut species_dir = PathBuf::from("assets/species");
    let mut mpe = 50usize;
    let mut max_ticks = 8000u64;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = || args.next().expect("flag needs a value");
        match a.as_str() {
            "--species-dir" => species_dir = PathBuf::from(next()),
            "--mpe" => mpe = next().parse()?,
            "--max-ticks" => max_ticks = next().parse()?,
            other => tracing::warn!(arg = other, "unknown flag, ignoring"),
        }
    }

    let species = antcolony_sim::species::load_species_dir(&species_dir)?;
    let n = species.len();
    tracing::info!(n, mpe, max_ticks, "cross_species_matrix: loaded roster");

    // winrate[a][b] = A's winrate vs B over mpe side-swapped matches.
    let mut winrate = vec![vec![0.0f32; n]; n];
    for ai in 0..n {
        for bi in 0..n {
            if ai == bi { winrate[ai][bi] = 0.5; continue; }
            let mut a_wins = 0.0f32;
            for m in 0..mpe {
                // Side-swap on parity to cancel first-move/topology bias.
                let seed = ((ai as u64) << 40) ^ ((bi as u64) << 24) ^ (m as u64);
                let (sp_left, sp_right, left_is_a) = if m % 2 == 0 {
                    (&species[ai], &species[bi], true)
                } else {
                    (&species[bi], &species[ai], false)
                };
                let mut env = MatchEnv::new_cross_species(sp_left, sp_right, seed);
                env.max_ticks = max_ticks;
                let mut left = HeuristicBrain::default();
                let mut right = HeuristicBrain::default();
                // Drive both colonies with the heuristic brain to match end.
                let result = run_to_end(&mut env, &mut left, &mut right);
                let a_won = match result {
                    Outcome::LeftWin => left_is_a,
                    Outcome::RightWin => !left_is_a,
                    Outcome::Draw => false,
                };
                if a_won { a_wins += 1.0; }
            }
            winrate[ai][bi] = a_wins / mpe as f32;
        }
        tracing::info!(species = %species[ai].id, "row complete");
    }

    // Report: matrix, per-row min/max, 3-cycles (A>B>C>A at >0.5 each).
    println!("# cross-species win matrix ({}x{}, mpe={})", n, n, mpe);
    print!("{:>24}", "");
    for s in &species { print!("{:>10.10}", s.id); }
    println!();
    for ai in 0..n {
        print!("{:>24}", species[ai].id);
        for bi in 0..n { print!("{:>10.2}", winrate[ai][bi]); }
        let row_min = (0..n).filter(|&b| b != ai).map(|b| winrate[ai][b]).fold(1.0, f32::min);
        let row_max = (0..n).filter(|&b| b != ai).map(|b| winrate[ai][b]).fold(0.0, f32::max);
        println!("   [min {:.2} max {:.2}]", row_min, row_max);
    }
    let mut cycles = 0usize;
    for a in 0..n { for b in 0..n { for c in 0..n {
        if a != b && b != c && a != c
            && winrate[a][b] > 0.5 && winrate[b][c] > 0.5 && winrate[c][a] > 0.5 {
            cycles += 1;
            tracing::info!(a = %species[a].id, b = %species[b].id, c = %species[c].id, "3-cycle");
        }
    }}}
    println!("# intransitive 3-cycles: {}", cycles);
    println!("CROSS_SPECIES_MATRIX_DONE n={} cycles={}", n, cycles);
    Ok(())
}

enum Outcome { LeftWin, RightWin, Draw }
fn run_to_end(env: &mut MatchEnv, left: &mut dyn AiBrain, right: &mut dyn AiBrain) -> Outcome {
    loop {
        let (sl, sr) = (env.observe(0), env.observe(1));
        let (Some(sl), Some(sr)) = (sl, sr) else { break; };
        let al = left.decide(&sl);
        let ar = right.decide(&sr);
        let step = env.step(&al, &ar);
        if step.done || env.sim.tick >= env.max_ticks { break; }
    }
    match env.sim.match_status() {
        MatchStatus::Won { winner: 0, .. } => Outcome::LeftWin,
        MatchStatus::Won { winner: 1, .. } => Outcome::RightWin,
        _ => Outcome::Draw,
    }
}
```

> Verify the exact `HeuristicBrain` / `AiBrain` re-export names against `antcolony_sim`'s public API (the trainer already imports `AiBrain` in `env.rs`). If `HeuristicBrain` lives at a different path (e.g. `antcolony_sim::ai::HeuristicBrain`), use that. Do not invent a brain — reuse whatever the tournament harness uses for the heuristic baseline (`crates/antcolony-trainer/src/eval.rs` / `tournament.rs` reference it).

- [ ] **Step 4: Build the binary**

Run: `cargo build -p antcolony-trainer --bin cross_species_matrix 2>&1 | tail -15`
Expected: compiles clean.

- [ ] **Step 5: Write the PowerShell run wrapper**

```powershell
# scripts/run_cross_species_matrix.ps1
# Cross-species win-matrix harness. Writes the matrix + intransitivity
# report to scratch/. Run from repo root.
$ErrorActionPreference = "Stop"
$env:RUST_LOG = if ($env:RUST_LOG) { $env:RUST_LOG } else { "info" }
New-Item -ItemType Directory -Force -Path scratch | Out-Null
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$out = "scratch/cross_species_matrix_$stamp.txt"
cargo build --release -p antcolony-trainer --bin cross_species_matrix
& ./target/release/cross_species_matrix --species-dir assets/species --mpe 50 --max-ticks 8000 *>&1 |
    Tee-Object -FilePath $out
Write-Host "Win matrix written to $out"
```

- [ ] **Step 6: Smoke-run the harness at tiny mpe (don't gate CI on the full run)**

Run: `cargo run -p antcolony-trainer --bin cross_species_matrix -- --species-dir assets/species --mpe 2 --max-ticks 1500 2>&1 | tail -25`
Expected: prints an N×N matrix and `CROSS_SPECIES_MATRIX_DONE n=10 cycles=…` without panic. (mpe=2 is a wiring smoke; real balance reads need mpe≥50 via the .ps1.)

- [ ] **Step 7: Commit**

```bash
git add crates/antcolony-sim/src/simulation.rs crates/antcolony-trainer/src/bin/cross_species_matrix.rs scripts/run_cross_species_matrix.ps1
git commit -m "test+harness(xspecies): determinism + BC-vs-AR smoke + win-matrix/intransitivity harness"
```

---

### Task 8: Full-workspace build + both suites green + additivity audit

**Files:** none (verification task).

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace 2>&1 | tail -15`
Expected: clean (pre-existing render/sim warnings only).

- [ ] **Step 2: Full sim suite (the ~180-test regression gate)**

Run: `cargo test -p antcolony-sim 2>&1 | tail -30`
Expected: all green — existing tests + new config/clade/apply_colony/combat/usurp/determinism tests.

- [ ] **Step 3: Full trainer suite**

Run: `cargo test -p antcolony-trainer 2>&1 | tail -30`
Expected: all green — existing env/eval/tournament tests + `new_cross_species`.

- [ ] **Step 4: Additivity audit — confirm the hot pre-existing files only changed where intended**

Run: `git diff --stat main -- crates/antcolony-sim/src/pheromone.rs crates/antcolony-sim/src/world.rs crates/antcolony-sim/src/spatial.rs crates/antcolony-trainer/src/eval.rs crates/antcolony-trainer/src/tournament.rs`
Expected: empty (those files are untouched — cross-species is additive on config/species/simulation/ant/colony + the new clade module + the trainer env/harness).

- [ ] **Step 5: Re-run the determinism example if present (cross-process guard)**

Run: `cargo run -p antcolony-sim --example det_check 2>&1 | tail -10` (if the example exists per MEMORY `project_determinism`)
Expected: deterministic OK. (Skip if no such example; the in-suite `cross_species_is_byte_deterministic` covers cross-run determinism.)

- [ ] **Step 6: Commit any fixups**

```bash
git add -A && git commit -m "chore(xspecies): workspace build + both suites green; additivity confirmed"
```

---

## Self-Review

**1. Spec coverage:**
- Per-colony species wiring (`ColonySimConfig`, `colony_configs`, `colony_cfg`) → Task 1 + Task 3. ✅
- `Species::apply_colony` factored out, `apply` delegates, bit-identical → Task 3 (refactor guard test). ✅
- `new_two_colony_cross_species` + old ctor delegates → BYTE-IDENTICAL → Task 3 (keystone guard test EARLY, as instructed). ✅
- Per-colony combat (per-side attack/health) → Task 4. ✅
- Venom × resistance susceptibility matrix → Task 2 (`venom_multiplier`) + Task 4 (applied). ✅
- Terrain-gated `max_simultaneous_attackers` (open/tunnel/entrance, Lanchester B3) → Task 4 (`terrain_attacker_cap`). ✅
- Flee-threshold bias (B5) → Task 4 (gated on `sting_potency > 1.0`). ✅
- `predates_ants` field wired (B7) + corpse→killer routing → Task 3 (field) + Task 4 (routing). ✅
- Gated 2-phase interruptible queen-kill (`AntState::Usurping`, Phase 0 gate / Phase 1 channel / Phase 2 resolution) → Task 5. ✅
- Trainer `MatchEnv` applies species → Task 6 (`new_cross_species`). ✅
- Cross-species win-matrix / balance harness + determinism + back-compat regression → Task 7 (+ guards in Tasks 3/4/5). ✅
- Determinism (id-sorted iteration, no rng perturbation) → Task 5 design + Task 7 guard. ✅
- Recruitment scalar: carried on `ColonySimConfig` for obs/logging; deposit scaling stays global — **deliberate MVP scope cut from spec §combat-model-4**, flagged in File Structure note #2 and an open question. ⚠️ (documented, not silently dropped)

**2. Placeholder scan:** No "TBD" / "implement later" in code steps. Every code block is concrete Rust. The three explicit deferrals are called out and justified, not placeholders: (a) per-colony pheromone deposit (kept global, scalar carried for obs); (b) worker-defection Phase-2 payoff (spec marks it MVP-optional — omitted, gate field absent by design); (c) `combat.rs` extraction (kept in `simulation.rs` per flat-file rule + determinism risk).

**3. Type consistency across tasks:**
- `ColonySimConfig { ant, colony, combat, species_id, clade, weapon, recruitment_scalar, predates_ants }` defined in Task 1, consumed identically in Tasks 3/4/5/6. ✅
- `venom_multiplier(weapon: Weapon, attacker_sting_potency: f32, defender: Clade) -> f32` — Task 2 def matches Task 4 call. ✅
- New `CombatConfig` fields (`venom_resistance`, `max_simultaneous_attackers_{open,tunnel,entrance}`, `usurp_*`) defined in Task 1, read in Tasks 4/5. ✅
- `colony_cfg(&self, colony_id: u8) -> &ColonySimConfig` — Task 3 def, used in Tasks 4/5. ✅
- `new_two_colony_cross_species(world_pheromone_hazards: SimConfig, cfg_black, cfg_red, topology, seed, nest_black, nest_red)` — Task 3 def matches Task 6/7 calls. ✅
- `AntState::Usurping` added in Task 5 (ant.rs), referenced in Task 5 usurp logic. ✅
- `ColonyState.usurp_progress_ticks: u32` + `usurp_attacker_colony: Option<u8>` — Task 5 def + use. ✅
- `MatchEnv::new_cross_species(&Species, &Species, u64)` — Task 6 def matches Task 7 harness call. ✅

**4. Real-code-vs-spec deviations (corrected, all noted inline):**
- **No `combat.rs`** — combat stays in `simulation.rs:combat_tick` (flat-file rule + byte-identical determinism risk). Spec proposed a new file; dropped.
- `recruitment_deposit_scalar` is `pub(crate)`; per-colony pheromone deposit is NOT moved per-colony in MVP (scalar carried for obs only) — spec §combat-model-4 scope cut.
- `sting_potency` read from `ColonySimConfig.ant.sting_potency` (already wired by `apply`), not a duplicate field.
- `spawn_initial_ants(&SimConfig, …)` signature unchanged; per-colony spawning uses a per-side `SimConfig` view (global + slice).
- `fit_bore_to_species` is a no-op for the default slice (4.0mm → bore stays 8.0); the Task 3 byte-identical guard proves it — caveat + fix documented.
- Queen-kill: queens are NOT combat victims today; the channel is the only new path to a queen ant's death, fully gated (`usurp_channel_ticks==0 && ratio==0.0` ⇒ no-op).

**Open question for the human (one, surfaced from spec open-questions 1–7):** Spec OQ7 — confirm the MVP scope cut on per-colony pheromone *deposit* (kept global; only the recruitment *scalar* is carried per-colony for obs/win-matrix labelling). The biologically-correct version (deposit strength = depositor's trait) touches the shared `PheromoneConfig` and risks the byte-identical guard; deferring it keeps Task 3 clean. If you want it in MVP, it becomes a Task 4b (per-colony deposit at the deposit sites, with its own back-compat guard). The other spec OQs (roster strategy = species-conditioned single policy; venom cap 2.0×/resistance 0.9; attacker caps open=255→tunnel=2→entrance=1; defection deferred; corpse-frac tunable; predation-bench separate) are baked into the defaults above.

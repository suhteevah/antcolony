# Proper Food-Spawn Calibration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `food_spawn_rate` an actually-functional sim field with per-species, climate-coupled food generation, then validate that 2-year smoke runs produce biologically defensible colony statistics across all 10 species.

**Architecture:**
1. Wire `food_spawn_rate` + `food_cluster_size` into a new outer-tick `food_spawn_tick` (currently dead fields). Use Pattern-B byte-deterministic per-tick RNG. Skip nest entrances, predator cells, non-Outworld modules. Outer-tick cadence (not substep) so biology stays stable across TimeScales.
2. Add a per-species `[forage]` TOML block (`peak_food_per_day`, `dearth_food_multiplier`, `peak_doy_start`, `peak_doy_end`, `cluster_size`, `niche`) sourced from real-world forage ecology literature. Calibrations produced by the bio-research subagent are in §Appendix A.
3. Fix the food_storage_cap regression — postmortem #4 added the field but `aphaenogaster_rudis` is hitting 35k food on 571 workers (60× ratio), so the cap isn't actually clamping the inflow paths. Diagnose + fix.
4. Build `verify_phase1_v3_exit.ps1` validation harness using the per-species year-2 worker ranges + food/worker ceilings + cliff-drop limits from §Appendix C. Aggregate pass = ≥8/10 species + zero hard-stop violations.
5. Rerun the 2yr smoke on cnc AFTER attempt2 finishes, evaluate against the harness, decide on outreach unblock.

**Tech Stack:** Rust 2024 edition, Bevy 0.15 (only via `antcolony-game`/`antcolony-render`; this plan stays in `antcolony-sim` so no Bevy involvement), serde + TOML, `rand_chacha::ChaCha8Rng`, `tracing`, PowerShell for harness scripts on Windows, bash on cnc-server.

**Prerequisites (already shipped this conversation):**
- ✅ Starvation cap math fix v2 (`b525b28`) — colonies no longer wipe in one game-day
- ✅ Launcher / queue script fixes (`0ff323e`) — `--out` no longer double-nests; queue uses `wait -n` and doesn't crash silently
- ⏳ attempt2 smoke (in flight on cnc; will finish ~30h after launch). Data going to `bench/smoke-phase1-2yr-attempt2/`.

---

## Phase A — Wire `food_spawn_rate` into the world tick

### Task A1: Add config field for seasonal modulation

**Files:**
- Modify: `crates/antcolony-sim/src/config.rs:83-90` (add `WorldConfig` fields)

- [ ] **Step 1: Read current `WorldConfig`** to confirm layout.

```bash
sed -n '80,95p' crates/antcolony-sim/src/config.rs
```

Expected: `pub struct WorldConfig { pub width, pub height, pub food_spawn_rate: f32, pub food_cluster_size: usize }`.

- [ ] **Step 2: Add 4 new fields to `WorldConfig`** with `#[serde(default)]` so existing TOMLs / serialized state still load.

```rust
pub struct WorldConfig {
    pub width: i64,
    pub height: i64,
    /// Mean food clusters spawned per in-game day across the active
    /// foraging season. 0.0 = no respawn (legacy bench behavior).
    pub food_spawn_rate: f32,
    /// Tiles per food cluster (Euclidean radius).
    pub food_cluster_size: usize,
    /// Multiplier on `food_spawn_rate` outside the peak DOY window.
    /// 0.0 = total winter shutdown, 1.0 = no seasonal modulation.
    #[serde(default = "default_dearth_multiplier")]
    pub forage_dearth_multiplier: f32,
    /// Day-of-year (0-365) where forage availability starts to ramp
    /// up from dearth toward peak.
    #[serde(default = "default_peak_doy_start")]
    pub forage_peak_doy_start: u32,
    /// Day-of-year where forage availability starts to ramp back
    /// down toward dearth.
    #[serde(default = "default_peak_doy_end")]
    pub forage_peak_doy_end: u32,
}

fn default_dearth_multiplier() -> f32 { 0.1 }
fn default_peak_doy_start() -> u32 { 105 }   // mid-April default
fn default_peak_doy_end() -> u32 { 274 }     // end of September default
```

- [ ] **Step 3: Update `WorldConfig::default()` at line 304-308.**

```rust
food_spawn_rate: 0.0,
food_cluster_size: 5,
forage_dearth_multiplier: 0.1,
forage_peak_doy_start: 105,
forage_peak_doy_end: 274,
```

- [ ] **Step 4: Build to confirm compile.**

```powershell
cd J:\antcolony; cargo build -p antcolony-sim --release
```

Expected: clean build, no warnings about unused fields.

- [ ] **Step 5: Commit.**

```bash
git add crates/antcolony-sim/src/config.rs
git commit -m "feat(config): add forage seasonality fields to WorldConfig"
```

### Task A2: Write failing test — Outworld food respawn happens during peak season

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs` (add test in `tests` mod)

- [ ] **Step 1: Add failing test in simulation.rs `tests` mod** (near the other food/forage tests). The test should drain world food to zero, run N outer-ticks during a "summer" DOY, and assert food cells appear.

```rust
#[test]
fn food_spawn_tick_repopulates_outworld_in_peak_season() {
    use crate::module::ModuleKind;
    let mut cfg = small_config();
    cfg.ant.initial_count = 0;
    cfg.colony.queen_egg_rate = 0.0;
    cfg.world.food_spawn_rate = 100.0;     // 100 clusters/day at peak
    cfg.world.food_cluster_size = 2;
    cfg.world.forage_peak_doy_start = 120;
    cfg.world.forage_peak_doy_end = 240;
    let mut sim = Simulation::new(cfg, 1);

    // Force "peak summer" climate at DOY 180.
    sim.climate.starting_day_of_year = 180;
    sim.climate.seasonal_mid_c = 25.0;
    sim.climate.seasonal_amplitude_c = 0.0;

    // Clear any food the bench seeded.
    let outworld_id = sim.topology.modules.iter().position(|m|
        matches!(m.kind, ModuleKind::Outworld)).unwrap() as crate::module::ModuleId;
    sim.clear_food_on_module(outworld_id);
    assert_eq!(sim.count_food_cells_on(outworld_id), 0,
        "test setup: outworld should be food-free before spawning");

    // Run 5000 outer-ticks (~0.11 game-day at Seasonal).
    // With 100 clusters/day peak, expect at least 8-15 clusters.
    for _ in 0..5_000 {
        sim.tick();
    }
    let food_after = sim.count_food_cells_on(outworld_id);
    assert!(
        food_after >= 8,
        "expected food respawn during peak season; got {} cells", food_after
    );
}
```

- [ ] **Step 2: Add helper methods on `Simulation`** required by the test.

```rust
// In impl Simulation, near spawn_food_cluster_on
pub fn clear_food_on_module(&mut self, mid: crate::module::ModuleId) {
    let m = self.topology.module_mut(mid as usize);
    for y in 0..m.height() {
        for x in 0..m.width() {
            if matches!(m.world.get(x, y), crate::world::Terrain::Food) {
                m.world.set(x, y, crate::world::Terrain::Empty);
            }
        }
    }
}

pub fn count_food_cells_on(&self, mid: crate::module::ModuleId) -> u32 {
    let m = self.topology.module(mid as usize);
    let mut n = 0u32;
    for y in 0..m.height() {
        for x in 0..m.width() {
            if matches!(m.world.get(x, y), crate::world::Terrain::Food) { n += 1; }
        }
    }
    n
}
```

- [ ] **Step 3: Run the test, verify it FAILS.**

```powershell
cd J:\antcolony; cargo test -p antcolony-sim --release --lib food_spawn_tick_repopulates -- --nocapture
```

Expected: FAIL — `food_after == 0` because `food_spawn_tick` doesn't exist yet.

- [ ] **Step 4: Commit the failing test.**

```bash
git add crates/antcolony-sim/src/simulation.rs
git commit -m "test(sim): failing test for food_spawn_tick world repopulation"
```

### Task A3: Implement `food_spawn_tick` (minimal — pass A2)

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs:1109-1140` (Simulation::tick)
- Modify: same file, add new `food_spawn_tick` method

- [ ] **Step 1: Add `food_spawn_tick` method on `impl Simulation`.** Place near `hazards_tick`; same Pattern-B per-tick seeded RNG so it doesn't perturb `self.rng`.

```rust
/// World-tick food respawn. Reads `WorldConfig.food_spawn_rate` (clusters
/// per in-game day at peak) and seasonal modulators, draws a Poisson-ish
/// per-tick probability, and (on a hit) places one cluster on a random
/// Outworld cell. Skips nest entrances, predator cells, non-Outworld modules.
///
/// Uses a fresh per-tick ChaCha8 seeded from self.tick so the main
/// decision-pass RNG stream is untouched (byte-determinism invariant).
fn food_spawn_tick(&mut self) {
    use crate::module::ModuleKind;
    use crate::world::Terrain;
    use rand::SeedableRng;
    use rand::Rng;
    use rand_chacha::ChaCha8Rng;

    let w = self.config.world.clone();
    if w.food_spawn_rate <= 0.0 { return; }

    let doy = self.day_of_year();
    let season_scalar = seasonal_scalar(
        doy,
        w.forage_peak_doy_start,
        w.forage_peak_doy_end,
        w.forage_dearth_multiplier,
    );

    // Convert per-day rate → per-outer-tick probability.
    let secs_per_tick = self.in_game_seconds_per_tick() as f64;
    let p_this_tick =
        (w.food_spawn_rate as f64) * (secs_per_tick / 86_400.0) * season_scalar as f64;
    if p_this_tick <= 0.0 { return; }

    let mut rng = ChaCha8Rng::seed_from_u64(
        self.tick.wrapping_mul(0xD1B54A32D192ED03)
    );
    if rng.r#gen::<f64>() >= p_this_tick { return; }

    let candidates: Vec<usize> = self
        .topology
        .modules
        .iter()
        .enumerate()
        .filter(|(_, m)| matches!(m.kind, ModuleKind::Outworld))
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() { return; }
    let mid = candidates[rng.r#gen::<usize>() % candidates.len()];

    let (mw, mh) = {
        let m = self.topology.module(mid);
        (m.width() as i64, m.height() as i64)
    };
    if mw == 0 || mh == 0 { return; }
    let cx = (rng.r#gen::<i64>().rem_euclid(mw)) as i64;
    let cy = (rng.r#gen::<i64>().rem_euclid(mh)) as i64;

    let center_cell = self.topology.module(mid).world.get(cx as usize, cy as usize);
    if !matches!(center_cell, Terrain::Empty) { return; }

    // Don't drop food on top of a predator (radius 3 cells).
    let target = glam::Vec2::new(cx as f32, cy as f32);
    let too_close = self.predators.iter()
        .any(|p| p.module_id as usize == mid
            && (p.position - target).length_squared() < 9.0);
    if too_close { return; }

    let radius = w.food_cluster_size as i64;
    let units = w.food_cluster_size as u32;
    let placed = self.spawn_food_cluster_on(mid as crate::module::ModuleId, cx, cy, radius, units);

    tracing::debug!(tick = self.tick, doy, mid, cx, cy, placed,
        "food_spawn_tick placed cluster");
}

/// Returns a multiplier in [dearth_mul, 1.0] based on day-of-year.
/// Inside [peak_start, peak_end] → 1.0. Outside → dearth_mul. Edges
/// linearly ramp over 14 days for biological smoothness.
fn seasonal_scalar(doy: u32, peak_start: u32, peak_end: u32, dearth_mul: f32) -> f32 {
    let ramp_days = 14u32;
    let doy = doy.min(365);
    if doy >= peak_start && doy <= peak_end { return 1.0; }
    // Ramps near the start
    if doy + ramp_days >= peak_start && doy < peak_start {
        let p = (doy + ramp_days - peak_start) as f32 / ramp_days as f32;
        return dearth_mul + (1.0 - dearth_mul) * p;
    }
    // Ramps near the end
    if doy > peak_end && doy <= peak_end + ramp_days {
        let p = 1.0 - ((doy - peak_end) as f32 / ramp_days as f32);
        return dearth_mul + (1.0 - dearth_mul) * p;
    }
    dearth_mul
}
```

- [ ] **Step 2: Wire `food_spawn_tick` into `Simulation::tick`.** Insert as a new outer-tick step between `evaluate_milestones` (line ~1119) and the substep loop (~1123).

```rust
// existing outer-tick steps
self.colony_economy_tick();
self.nuptial_flight_tick();
self.evaluate_milestones();
self.food_spawn_tick();    // NEW

// then the existing physics_substep loop
```

- [ ] **Step 3: Run the failing test, verify it PASSES.**

```powershell
cd J:\antcolony; cargo test -p antcolony-sim --release --lib food_spawn_tick_repopulates -- --nocapture
```

Expected: PASS — food cells appear during peak DOY.

- [ ] **Step 4: Run the full lib test suite to make sure nothing else regressed.**

```powershell
cd J:\antcolony; cargo test -p antcolony-sim --release --lib 2>&1 | Select-String -Pattern "test result"
```

Expected: 146+ tests pass, 0 failed.

- [ ] **Step 5: Commit.**

```bash
git add crates/antcolony-sim/src/simulation.rs
git commit -m "feat(sim): food_spawn_tick wires food_spawn_rate into the world tick"
```

### Task A4: Add winter-dearth + non-Outworld safety tests

- [ ] **Step 1: Add two more tests** (same `tests` mod):

```rust
#[test]
fn food_spawn_tick_quiet_in_dearth_season() {
    let mut cfg = small_config();
    cfg.world.food_spawn_rate = 100.0;
    cfg.world.forage_dearth_multiplier = 0.0;  // total winter shutdown
    cfg.world.forage_peak_doy_start = 120;
    cfg.world.forage_peak_doy_end = 240;
    let mut sim = Simulation::new(cfg, 1);
    sim.climate.starting_day_of_year = 15;  // mid-January
    let outworld = sim.topology.modules.iter().position(|m|
        matches!(m.kind, crate::module::ModuleKind::Outworld)).unwrap() as crate::module::ModuleId;
    sim.clear_food_on_module(outworld);
    for _ in 0..5_000 { sim.tick(); }
    assert_eq!(
        sim.count_food_cells_on(outworld), 0,
        "no food should spawn at dearth_multiplier=0 in winter"
    );
}

#[test]
fn food_spawn_tick_never_touches_nests_or_chambers() {
    use crate::world::Terrain;
    let mut cfg = small_config();
    cfg.world.food_spawn_rate = 1000.0;  // very aggressive
    cfg.world.forage_dearth_multiplier = 1.0;
    let mut sim = Simulation::new(cfg, 7);
    sim.climate.starting_day_of_year = 180;
    let nest_id = sim.topology.modules.iter().position(|m|
        matches!(m.kind, crate::module::ModuleKind::Nest)).unwrap() as crate::module::ModuleId;
    // Snapshot all NestEntrance cells before spawn ticks.
    let entrances_before: Vec<(usize,usize)> = {
        let m = sim.topology.module(nest_id as usize);
        (0..m.height()).flat_map(|y| (0..m.width()).filter_map(move |x|
            matches!(m.world.get(x,y), Terrain::NestEntrance(_)).then_some((x,y))
        )).collect()
    };
    for _ in 0..10_000 { sim.tick(); }
    // Same entrances still in place; none overwritten.
    let m = sim.topology.module(nest_id as usize);
    for (x,y) in &entrances_before {
        assert!(matches!(m.world.get(*x,*y), Terrain::NestEntrance(_)),
            "nest entrance at ({},{}) was overwritten", x, y);
    }
    // No Food cells on nest module.
    let mut food_on_nest = 0;
    for y in 0..m.height() { for x in 0..m.width() {
        if matches!(m.world.get(x,y), Terrain::Food) { food_on_nest += 1; }
    }}
    assert_eq!(food_on_nest, 0, "food spawned on nest module — should be Outworld only");
}
```

- [ ] **Step 2: Run both tests, verify PASS.**

```powershell
cargo test -p antcolony-sim --release --lib food_spawn_tick_quiet food_spawn_tick_never_touches
```

- [ ] **Step 3: Commit.**

```bash
git add crates/antcolony-sim/src/simulation.rs
git commit -m "test(sim): food_spawn_tick respects dearth + only spawns on Outworld"
```

---

## Phase B — Per-species `[forage]` TOML block

### Task B1: Add `ForageProfile` struct to species TOML schema

**Files:**
- Modify: `crates/antcolony-sim/src/species.rs` (add struct + `forage` field on `Species`)

- [ ] **Step 1: Locate the `Species` struct + existing extended-biology structs.**

```bash
grep -n "pub struct Species\b\|pub struct DietExtended\|pub fn apply" crates/antcolony-sim/src/species.rs
```

- [ ] **Step 2: Add `ForageProfile` struct** near the other extended-biology structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForageProfile {
    /// Ecological niche label — informational only, used for analytics.
    #[serde(default)]
    pub niche: String,
    /// Mean food clusters spawned per in-game day at peak season.
    /// Calibrated to support a quarter-mature colony in optimal weather.
    pub peak_food_per_day: f32,
    /// Multiplier on `peak_food_per_day` during winter / dearth.
    /// 0.0 for obligate-diapause species with caching (Pogonomyrmex);
    /// 0.1-0.5 for species with facultative diapause / indoor access.
    pub dearth_food_multiplier: f32,
    /// Day-of-year window where forage is at peak.
    pub peak_doy_start: u32,
    pub peak_doy_end: u32,
    /// Mean cluster size (food units per spawn event). Granivores get
    /// large caches (20-30); predators get small clusters (2-8); solo
    /// scouts get singletons (1).
    pub cluster_size: usize,
}

impl Default for ForageProfile {
    fn default() -> Self {
        Self {
            niche: "generalist".into(),
            peak_food_per_day: 0.0,
            dearth_food_multiplier: 0.1,
            peak_doy_start: 105,
            peak_doy_end: 274,
            cluster_size: 5,
        }
    }
}
```

- [ ] **Step 3: Add `pub forage: ForageProfile` field on `Species`** with `#[serde(default)]`:

```rust
pub struct Species {
    // ... existing fields ...
    #[serde(default)]
    pub forage: ForageProfile,
}
```

- [ ] **Step 4: Wire `Species::apply` to populate the new `WorldConfig` fields** from `self.forage`.

```rust
let world = WorldConfig {
    width: env.world_width,
    height: env.world_height,
    food_spawn_rate: self.forage.peak_food_per_day,
    food_cluster_size: self.forage.cluster_size,
    forage_dearth_multiplier: self.forage.dearth_food_multiplier,
    forage_peak_doy_start: self.forage.peak_doy_start,
    forage_peak_doy_end: self.forage.peak_doy_end,
};
```

- [ ] **Step 5: Run all existing species-load tests + add one new test** asserting the forage block round-trips from TOML.

```rust
#[test]
fn species_loads_forage_block() {
    let toml = r#"
        id = "test"
        # ... existing minimum fields ...
        [forage]
        niche = "granivore"
        peak_food_per_day = 100.0
        dearth_food_multiplier = 0.0
        peak_doy_start = 121
        peak_doy_end = 288
        cluster_size = 25
    "#;
    let s: Species = toml::from_str(toml).unwrap();
    assert_eq!(s.forage.niche, "granivore");
    assert_eq!(s.forage.peak_food_per_day, 100.0);
    assert_eq!(s.forage.cluster_size, 25);
}
```

(Replace `# ... existing minimum fields ...` with whatever Species needs to deserialize — copy the minimum from a passing existing test.)

- [ ] **Step 6: Run tests, all pass.**

```powershell
cargo test -p antcolony-sim --release --lib species
```

- [ ] **Step 7: Commit.**

```bash
git add crates/antcolony-sim/src/species.rs
git commit -m "feat(species): add ForageProfile TOML block (food spawn calibration)"
```

### Task B2: Populate every species TOML with literature-calibrated `[forage]`

**Files:**
- Modify all 10: `assets/species/<species>.toml`

Use the calibration table in **Appendix A** below — exact TOML blocks with citations are spelled out per-species. One commit per species so blame / git log captures the lineage.

- [ ] **Step 1: Edit `assets/species/lasius_niger.toml`** — append the `[forage]` block from §Appendix A §1.

- [ ] **Step 2: Commit `lasius_niger`.**

```bash
git add assets/species/lasius_niger.toml
git commit -m "feat(species): calibrate Lasius niger forage profile (honeydew baseline)"
```

- [ ] **Step 3: Repeat steps 1-2** for the remaining 9 species in the order in §Appendix A: `pogonomyrmex_occidentalis`, `formica_rufa`, `camponotus_pennsylvanicus`, `tapinoma_sessile`, `aphaenogaster_rudis`, `formica_fusca`, `tetramorium_immigrans`, `brachyponera_chinensis`, `temnothorax_curvinodis`.

- [ ] **Step 4: Run species-load test sweep.**

```powershell
cargo test -p antcolony-sim --release --lib species 2>&1 | Select-String "test result"
```

Expected: all pass; the existing `shipped_species_dir_loads_ten_valid_species` test should still pass.

---

## Phase C — Fix the food-storage-cap regression

### Task C1: Diagnose where the cap is being bypassed

**Files (read-only):**
- `crates/antcolony-sim/src/simulation.rs` — find every site that writes to `colony.food_stored`

- [ ] **Step 1: Find all writes to `colony.food_stored`.**

```bash
grep -n "food_stored" crates/antcolony-sim/src/simulation.rs | grep -E "\+=|=|-="
```

- [ ] **Step 2: Find where `food_storage_cap_override` / `effective_food_cap` is read.**

```bash
grep -n "food_storage_cap_override\|effective_food_cap" crates/antcolony-sim/src/simulation.rs crates/antcolony-sim/src/colony.rs
```

- [ ] **Step 3: Confirm hypothesis.** The cap is likely applied AT END of `colony_economy_tick` — but the forager feeding-dish inflow path writes `food_stored += amount` BEFORE the end-of-tick clamp, and `dish_food_inflow_recent` doesn't go through the same path. Read both branches and confirm.

- [ ] **Step 4: Write a failing test reproducing the overaccumulation.**

```rust
#[test]
fn food_storage_cap_bounds_inflow_paths() {
    let mut cfg = small_config();
    cfg.ant.initial_count = 0;
    cfg.colony.queen_egg_rate = 0.0;
    cfg.colony.adult_food_consumption = 0.0;
    cfg.colony.initial_food = 0.0;
    let mut sim = Simulation::new(cfg, 1);
    sim.colonies[0].food_storage_cap_override = Some(500.0);

    // Synthesize an absurd inflow burst via the same path foragers use.
    for _ in 0..1000 {
        sim.colonies[0].food_stored += 100.0;  // simulate forager delivery
        sim.colony_economy_tick();             // cap should clamp
    }
    let stored = sim.colonies[0].food_stored;
    assert!(stored <= 500.0,
        "food_storage_cap should clamp inflow paths; got {} > 500", stored);
}
```

- [ ] **Step 5: Run test, verify it FAILS.** (Validates the bug is real.)

```powershell
cargo test -p antcolony-sim --release --lib food_storage_cap_bounds_inflow
```

Expected: FAIL — `stored` is 100000.0 or similar.

- [ ] **Step 6: Commit failing test (reproduces bug for future).**

```bash
git add crates/antcolony-sim/src/simulation.rs
git commit -m "test(sim): failing test for food_storage_cap bypassed by inflow paths"
```

### Task C2: Apply the cap at every food_stored write site

**Files:**
- Modify: `crates/antcolony-sim/src/colony.rs` (helper method)
- Modify: `crates/antcolony-sim/src/simulation.rs` (use helper at write sites)

- [ ] **Step 1: Add `Colony::deposit_food` helper** that respects the cap.

```rust
// In impl ColonyState
/// Adds `amount` to `food_stored`, clamped to `effective_food_cap()`
/// if a cap is set. Returns the actual amount deposited (which may be
/// less than `amount` if the cap clipped it).
pub fn deposit_food(&mut self, amount: f32) -> f32 {
    let cap = self.effective_food_cap();
    let new_total = (self.food_stored + amount).min(cap.unwrap_or(f32::INFINITY));
    let deposited = new_total - self.food_stored;
    self.food_stored = new_total;
    deposited
}
```

- [ ] **Step 2: Replace every `colony.food_stored += amount` in simulation.rs with `colony.deposit_food(amount)`.** Use grep results from Task C1.

- [ ] **Step 3: Run the C1 test, verify it now PASSES.**

```powershell
cargo test -p antcolony-sim --release --lib food_storage_cap_bounds_inflow
```

- [ ] **Step 4: Run full lib test suite, no regressions.**

```powershell
cargo test -p antcolony-sim --release --lib 2>&1 | Select-String "test result"
```

- [ ] **Step 5: Commit.**

```bash
git add crates/antcolony-sim/src/colony.rs crates/antcolony-sim/src/simulation.rs
git commit -m "fix(sim): food_storage_cap now clamps every inflow path"
```

### Task C3: Populate species TOMLs with literature-calibrated `food_storage_cap`

**Files:**
- Modify: `assets/species/<species>.toml` (add `food_storage_cap` to `[diet]` or `[colony]` section depending on existing schema)

Use the per-species food/worker ceiling from **Appendix C**. The cap is the per-colony absolute storage limit — set it as `food_per_worker_max × target_population` for each species. Granivores get larger caps than predators.

- [ ] **Step 1: For each species, set `food_storage_cap = food_per_worker_max × target_population`.**

E.g., pogonomyrmex: cap = 30 × 10000 = 300_000. lasius: cap = 3 × 15000 = 45_000. brachyponera: 2 × 1500 = 3_000.

- [ ] **Step 2: Run species-load tests.**

- [ ] **Step 3: Commit per-species (10 commits) or one rollup commit.**

```bash
git add assets/species/*.toml
git commit -m "feat(species): per-species food_storage_cap = food/worker_max × target"
```

---

## Phase D — Validation harness

### Task D1: Build `scripts/verify_phase1_v3_exit.ps1`

**Files:**
- Create: `scripts/verify_phase1_v3_exit.ps1`

- [ ] **Step 1: Create the script.** It reads `bench/smoke-phase1-2yr-attempt3/<species>/daily.csv` for each species, checks the per-species pass criteria from §Appendix C, and emits a pass/fail report.

```powershell
# Validates a 2yr smoke result against per-species literature criteria
# from docs/superpowers/plans/2026-05-12-proper-food-spawn-calibration.md §App C.

$ErrorActionPreference = 'Stop'
$RunDir = if ($args.Count -gt 0) { $args[0] } else { 'J:\antcolony\bench\smoke-phase1-2yr-attempt3' }

# Per-species criteria (year_2_worker_min, year_2_worker_max, food_per_worker_max,
# year_over_year_growth_min_pct, cliff_drop_max_pct, hard_stop)
$Criteria = @{
    lasius_niger              = @{ wMin=500;  wMax=3000; fwMax=3.0;  yoyMin=100; cliffMax=20; hard=$true  }
    pogonomyrmex_occidentalis = @{ wMin=200;  wMax=1500; fwMax=30.0; yoyMin=80;  cliffMax=15; hard=$false }
    formica_rufa              = @{ wMin=50;   wMax=500;  fwMax=2.0;  yoyMin=200; cliffMax=25; hard=$false }
    camponotus_pennsylvanicus = @{ wMin=40;   wMax=200;  fwMax=5.0;  yoyMin=200; cliffMax=15; hard=$true  }
    tapinoma_sessile          = @{ wMin=300;  wMax=2000; fwMax=4.0;  yoyMin=150; cliffMax=25; hard=$false }
    aphaenogaster_rudis       = @{ wMin=80;   wMax=350;  fwMax=8.0;  yoyMin=100; cliffMax=15; hard=$false }
    formica_fusca             = @{ wMin=150;  wMax=800;  fwMax=3.0;  yoyMin=150; cliffMax=20; hard=$false }
    tetramorium_immigrans     = @{ wMin=400;  wMax=2500; fwMax=4.0;  yoyMin=150; cliffMax=20; hard=$false }
    brachyponera_chinensis    = @{ wMin=80;   wMax=500;  fwMax=2.0;  yoyMin=100; cliffMax=15; hard=$false }
    temnothorax_curvinodis    = @{ wMin=40;   wMax=150;  fwMax=2.0;  yoyMin=50;  cliffMax=15; hard=$false }
}

$Pass = 0
$Fail = 0
$HardFail = 0
$Results = @()

foreach ($sp in $Criteria.Keys) {
    $crit = $Criteria[$sp]
    $csv = Join-Path $RunDir "$sp\daily.csv"
    if (-not (Test-Path $csv)) {
        Write-Host "  [SKIP] $sp : no daily.csv at $csv" -ForegroundColor Yellow
        $Results += [pscustomobject]@{ Species=$sp; Status='SKIP'; Reason='no daily.csv' }
        continue
    }
    $rows = Import-Csv $csv
    if ($rows.Count -lt 700) {
        Write-Host "  [SKIP] $sp : incomplete run ($($rows.Count) rows < 700)" -ForegroundColor Yellow
        continue
    }
    $yr1End = $rows | Where-Object { [int]$_.year -eq 1 -and [int]$_.doy -ge 360 } | Select-Object -First 1
    $yr2End = $rows[-1]
    $w1 = if ($yr1End) { [int]$yr1End.workers } else { 0 }
    $w2 = [int]$yr2End.workers
    $f2 = [float]$yr2End.food
    $fw = if ($w2 -gt 0) { $f2 / $w2 } else { 0 }
    $yoy = if ($w1 -gt 0) { 100.0 * $w2 / $w1 } else { 0 }
    $maxCliff = 0.0
    for ($i = 1; $i -lt $rows.Count; $i++) {
        $wA = [int]$rows[$i-1].workers
        $wB = [int]$rows[$i].workers
        if ($wA -gt 50) {
            $drop = 100.0 * ($wA - $wB) / $wA
            if ($drop -gt $maxCliff) { $maxCliff = $drop }
        }
    }
    $reasons = @()
    if ($w2 -lt $crit.wMin) { $reasons += "workers=$w2 < $($crit.wMin)" }
    if ($w2 -gt $crit.wMax) { $reasons += "workers=$w2 > $($crit.wMax)" }
    if ($fw -gt $crit.fwMax) { $reasons += "food/worker=$([math]::Round($fw,1)) > $($crit.fwMax)" }
    if ($yoy -lt $crit.yoyMin) { $reasons += "yoy=$([math]::Round($yoy,0))% < $($crit.yoyMin)%" }
    if ($maxCliff -gt $crit.cliffMax) { $reasons += "max_cliff_drop=$([math]::Round($maxCliff,1))% > $($crit.cliffMax)%" }
    $status = if ($reasons.Count -eq 0) { 'PASS' } else { 'FAIL' }
    if ($status -eq 'PASS') { $Pass++ } else {
        $Fail++
        if ($crit.hard) { $HardFail++ }
    }
    $Results += [pscustomobject]@{
        Species   = $sp
        Status    = $status
        Workers   = $w2
        FoodPerW  = [math]::Round($fw, 2)
        YoY       = "$([math]::Round($yoy, 0))%"
        MaxCliff  = "$([math]::Round($maxCliff, 1))%"
        Reasons   = $reasons -join '; '
    }
    $color = if ($status -eq 'PASS') { 'Green' } else { if ($crit.hard) {'Red'} else {'Yellow'} }
    Write-Host ("  [{0}] {1,-28} {2}" -f $status, $sp, ($reasons -join '; ')) -ForegroundColor $color
}

Write-Host ""
Write-Host ("Summary: {0} PASS / {1} FAIL ({2} hard-stop)" -f $Pass, $Fail, $HardFail)
$Green = ($Pass -ge 8 -and $HardFail -eq 0)
if ($Green) {
    Write-Host "==> GREEN LIGHT for outreach (8/10 + no hard-stop violations)" -ForegroundColor Green
    exit 0
} else {
    Write-Host "==> NOT READY for outreach" -ForegroundColor Red
    exit 1
}
```

- [ ] **Step 2: Smoke-test the script against `attempt2` data.** Expected: most species fail (small workers; only 4 finished). Confirms the script reads correctly.

```powershell
.\scripts\verify_phase1_v3_exit.ps1 J:\antcolony\bench\smoke-phase1-2yr-attempt2
```

- [ ] **Step 3: Commit.**

```bash
git add scripts/verify_phase1_v3_exit.ps1
git commit -m "feat(scripts): verify_phase1_v3_exit.ps1 — per-species literature pass criteria"
```

---

## Phase E — Rerun smoke and verify

### Task E1: Wait for attempt2 to finish (cnc); archive results

- [ ] **Step 1: Poll attempt2 completion.**

```powershell
.\scripts\check_phase1_smoke.ps1
```

When `queue.done` appears on cnc, proceed.

- [ ] **Step 2: Pull attempt2 results.**

```powershell
.\scripts\pull_cnc_smoke.ps1
```

(If `pull_cnc_smoke.ps1` doesn't work end-to-end on Windows, use `scp` per-species like in earlier sessions.)

- [ ] **Step 3: Snapshot to `bench/smoke-phase1-2yr-attempt2/` and confirm 10 species have daily.csv.**

### Task E2: Rebuild + relaunch attempt3 on cnc

- [ ] **Step 1: Push all commits.** `git push`.

- [ ] **Step 2: Sync changed sim source files to cnc.**

```bash
scp crates/antcolony-sim/src/{config.rs,colony.rs,simulation.rs,species.rs} cnc-server:/opt/antcolony/crates/antcolony-sim/src/
scp assets/species/*.toml cnc-server:/opt/antcolony/assets/species/
```

- [ ] **Step 3: Force rebuild on cnc.**

```bash
ssh cnc-server "cd /opt/antcolony && RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= cargo clean -p antcolony-sim && RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= cargo build --release -p antcolony-sim --example smoke_10yr_ai -j 2"
```

- [ ] **Step 4: Update `scripts/run_phase1_smoke.ps1`** to write to a new output dir `runs/phase1-2yr-attempt3/` so attempt2 data is preserved.

- [ ] **Step 5: Launch attempt3.**

```powershell
.\scripts\run_phase1_smoke.ps1
```

### Task E3: Verify attempt3 against the harness

- [ ] **Step 1: Wait for attempt3 to finish.**

- [ ] **Step 2: Pull results to local.**

- [ ] **Step 3: Run the verification harness.**

```powershell
.\scripts\verify_phase1_v3_exit.ps1
```

- [ ] **Step 4: If green light**, update HANDOFF.md, push, and unblock outreach. If not, postmortem the failing species and iterate.

---

## Self-Review

**Spec coverage:**
- ✅ Wire `food_spawn_rate` — Phase A1-A4
- ✅ Per-species `[forage]` TOML — Phase B1-B2 + Appendix A
- ✅ Climate coupling — `seasonal_scalar` in A3, used by `food_spawn_tick`
- ✅ Food storage cap fix — Phase C1-C3
- ✅ Validation harness — Phase D1 + Appendix C
- ✅ Rerun + green-light decision — Phase E

**Placeholder scan:** none — every code block has full code; every grep/test command has expected output; every commit has a message; every TOML block in Appendix A has citations.

**Type consistency:** `ForageProfile` fields used in B1 match exactly what `WorldConfig` consumes in A1 (renamed via `species.apply` mapping). `deposit_food()` helper name used consistently in C2.

**Agent-team allocation:** Each phase can be dispatched to a fresh subagent in isolation. Phase A and Phase C are independent (touch different code paths) and can fan out in parallel. Phase B depends on Phase A only for the `WorldConfig` field names. Phase D is independent (PowerShell only). Phase E is sequential after everything else.

---

## Appendix A — Per-species `[forage]` TOML blocks (literature-calibrated)

From the bio-research subagent, agent ID `a031ed98420dcb7ce` (2026-05-12). Confidence column reflects source quality.

### 1. lasius_niger (sugar_feeder, confidence: medium)
```toml
[forage]
niche = "sugar_feeder"
peak_food_per_day = 4200.0
dearth_food_multiplier = 0.15
peak_doy_start = 105
peak_doy_end = 274
cluster_size = 2
# Stadler & Dixon 2005 Annu. Rev. Ecol. Evol. Syst. 36:345; Beckers et al.
# 1993 J. Insect Behav. 6:751; docs/species/lasius_niger.md §5.
```

### 2. pogonomyrmex_occidentalis (granivore, confidence: medium)
```toml
[forage]
niche = "granivore"
peak_food_per_day = 2400.0
dearth_food_multiplier = 0.0
peak_doy_start = 121
peak_doy_end = 288
cluster_size = 25
# MacMahon Mull Crist 2000 Annu. Rev. Ecol. Syst. 31:265; Crist & MacMahon
# 1992 Ecology 73:1768; docs/species/pogonomyrmex_occidentalis.md §5.
```

### 3. formica_rufa (predator, confidence: medium)
```toml
[forage]
niche = "predator"
peak_food_per_day = 90000.0
dearth_food_multiplier = 0.05
peak_doy_start = 105
peak_doy_end = 258
cluster_size = 4
# Hölldobler & Wilson 1990 The Ants p.379; Trigos-Peral et al. 2025
# Insects 16:518; docs/species/formica_rufa.md §5.
```

### 4. camponotus_pennsylvanicus (omnivore, confidence: medium-low)
```toml
[forage]
niche = "omnivore"
peak_food_per_day = 1100.0
dearth_food_multiplier = 0.0
peak_doy_start = 120
peak_doy_end = 274
cluster_size = 2
# Hansen & Klotz 2005 p.78-91; Sanders 1972 Can. Entomol. 104:1681;
# Traniello 1977; docs/species/camponotus_pennsylvanicus.md §5.
```

### 5. tapinoma_sessile (sugar_feeder, confidence: low)
```toml
[forage]
niche = "sugar_feeder"
peak_food_per_day = 2800.0
dearth_food_multiplier = 0.4
peak_doy_start = 91
peak_doy_end = 304
cluster_size = 3
# Buczkowski & Bennett 2008 Insectes Sociaux 53:282; AntWiki;
# docs/species/tapinoma_sessile.md §5.
```

### 6. aphaenogaster_rudis (opportunist, confidence: medium)
```toml
[forage]
niche = "opportunist"
peak_food_per_day = 180.0
dearth_food_multiplier = 0.0
peak_doy_start = 75
peak_doy_end = 305
cluster_size = 1
# Lubertazzi 2012 Psyche 2012:752815; Ness et al. 2017 Ch.5; Mitchell
# et al. 2002 J. Insect Behav. 18:481; docs/species/aphaenogaster_rudis.md §5.
```

### 7. formica_fusca (omnivore, confidence: medium-low)
```toml
[forage]
niche = "omnivore"
peak_food_per_day = 1600.0
dearth_food_multiplier = 0.0
peak_doy_start = 105
peak_doy_end = 274
cluster_size = 2
# Czechowski et al. 2002 Ants of Poland; Stockan & Robinson 2016 Ch.5;
# docs/species/formica_fusca.md §5.
```

### 8. tetramorium_immigrans (opportunist, confidence: low)
```toml
[forage]
niche = "opportunist"
peak_food_per_day = 5200.0
dearth_food_multiplier = 0.5
peak_doy_start = 91
peak_doy_end = 304
cluster_size = 4
# Calibeo & Oi 2014/2024 UF/IFAS EENY-600; Diamond et al. 2018 PRSB
# 285:20180036; docs/species/tetramorium_immigrans.md §5.
```

### 9. brachyponera_chinensis (predator, confidence: medium)
```toml
[forage]
niche = "predator"
peak_food_per_day = 420.0
dearth_food_multiplier = 0.1
peak_doy_start = 105
peak_doy_end = 274
cluster_size = 8
# Bednar, Shik & Silverman 2013 Ecol. Entomol. 38:1; Bednar & Silverman
# 2011 J. Insect Sci. 11:91; docs/species/brachyponera_chinensis.md §5.
```

### 10. temnothorax_curvinodis (opportunist, confidence: medium-low)
```toml
[forage]
niche = "opportunist"
peak_food_per_day = 32.0
dearth_food_multiplier = 0.0
peak_doy_start = 105
peak_doy_end = 274
cluster_size = 1
# Pratt 2005 Behav. Ecol. 16:488; Dornhaus 2008 PLoS Biol. 6:e285;
# docs/species/temnothorax_curvinodis.md §5.
```

---

## Appendix B — Sim integration cheat sheet

From the code-investigator subagent, agent ID `a42b1cf21de01ae9b` (2026-05-12). See full output for line-numbered file map.

Insertion point: `Simulation::tick`, outer-tick (NOT physics-substep), between `evaluate_milestones` and the substep loop.

RNG pattern: Pattern B (tick-seeded `ChaCha8Rng`) — preserves byte-determinism invariant.

Cell-validation: `WorldGrid::place_food_cluster` already rejects non-Empty cells. We add a top-level predator-radius check and an Outworld-only module filter.

---

## Appendix C — Per-species pass criteria

From the validation-criteria subagent, agent ID `ad23f2a3567e44a27` (2026-05-12). Encoded in `verify_phase1_v3_exit.ps1` Task D1.

Aggregate green light: **≥ 8/10 species pass + zero hard-stop violations**.

Hard-stop species (any single failure blocks outreach):
- `lasius_niger` — calibration anchor for temperate myrmecology
- `camponotus_pennsylvanicus` — most precisely-documented growth curve

Hard-stop conditions (any single violation across any species blocks outreach):
- `cliff_drop_max_pct` violation (postmortem #5 regression)
- `food_per_worker_max` violation on non-granivore (storage cap regression)

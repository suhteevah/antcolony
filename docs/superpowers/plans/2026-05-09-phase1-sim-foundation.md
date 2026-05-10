# Phase 1 — Sim Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land all colony-economy correctness work as one cohesive block so that a 2-year HeuristicBrain smoke across all 10 species passes the Phase 1 exit gate (10/10 alive, food/worker ratio < 5, no >20% population drops, all unit tests green).

**Architecture:** Five edits to `crates/antcolony-sim/src/simulation.rs` plus one new TOML field on `species_extended.rs`, plus regression tests. All changes are additive or surgical (no architectural rewrites). Tests follow the existing inline-`mod tests` pattern at the bottom of `simulation.rs` and use the existing `small_config()` + `Simulation::new()` + `colony_economy_tick()` harness.

**Tech Stack:** Rust 2024 edition, MSRV 1.85; `tracing` for structured logs; `serde` for TOML; `rand` for RNG; sim is byte-deterministic (use `self.rng`, never `thread_rng()`).

**Predecessor spec:** `docs/superpowers/specs/2026-05-09-outreach-roadmap-design.md` (Phase 1 section).

---

## File Structure

| File | Role | Change type |
|---|---|---|
| `crates/antcolony-sim/src/simulation.rs` | Sim core + colony economy + inline tests | 4 surgical edits + 4 new tests |
| `crates/antcolony-sim/src/species_extended.rs` | Species TOML schema | 1 new field added to `DietExtended` |
| `assets/species/*.toml` | Per-species biology data | No edits required (default cap applies) |
| `scripts/cnc_provision.ps1` | One-time cnc setup wrapper | New file |
| `scripts/cnc_smoke.ps1` | Smoke runner wrapper (rsync, build, launch, monitor, pull) | New file |
| `HANDOFF.md` | Session log roll-forward | Updated at end |

All other files unchanged.

---

## Pre-flight verification

### Task 0: Verify formica_fusca expected.rs entry compiles + passes

The prior session added `formica_fusca` to `crates/antcolony-sim/src/bench/expected.rs` but never test-ran it. Verify before starting any other work.

**Files:** none (read-only verification)

- [ ] **Step 0.1: Run the bench::expected unit tests**

```powershell
cd J:\antcolony
cargo test -p antcolony-sim --lib bench::expected
```

Expected: 4 tests pass, including any test that iterates the species allowlist with formica_fusca.

- [ ] **Step 0.2: Run the full lib test suite as a baseline**

```powershell
cargo test -p antcolony-sim --lib 2>&1 | Tee-Object -FilePath baseline_tests.log
```

Expected: 137+ tests pass (per HANDOFF.md). Save output for diff comparison after Phase 1 edits.

- [ ] **Step 0.3: If anything fails, STOP and report**

If formica_fusca tests fail, fix that first (the prior session may have missed an allowlist registration). If unrelated tests fail, the baseline is broken and we can't trust regression results.

---

## Task 1: Add `food_storage_cap` field to `DietExtended`

The TOML schema needs a new optional field. When unset, fall back to a computed default.

**Files:**
- Modify: `crates/antcolony-sim/src/species_extended.rs:254-355` (struct + Default impl)
- Test: same file, inline tests near line 355

- [ ] **Step 1.1: Read the current `DietExtended` struct + Default impl**

```powershell
cargo expand -p antcolony-sim 2>$null | Out-Null  # warm cache; not required
```

Read `crates/antcolony-sim/src/species_extended.rs` lines 254-360 to see the current field list and Default pattern. Confirm the struct uses `#[serde(default)]` for missing-field tolerance.

- [ ] **Step 1.2: Write a failing serde-roundtrip test**

Add to the inline `mod tests` block in `species_extended.rs`:

```rust
#[test]
fn diet_extended_food_storage_cap_optional() {
    // Loading a TOML without food_storage_cap should succeed and yield None.
    let toml_no_cap = r#"
        food_categories = ["seed"]
        prefers_animal_protein = false
    "#;
    let d: DietExtended = toml::from_str(toml_no_cap).expect("parse");
    assert_eq!(d.food_storage_cap, None);

    // Loading with an explicit cap should round-trip the value.
    let toml_with_cap = r#"
        food_categories = ["seed"]
        prefers_animal_protein = false
        food_storage_cap = 2500.0
    "#;
    let d: DietExtended = toml::from_str(toml_with_cap).expect("parse");
    assert_eq!(d.food_storage_cap, Some(2500.0));
}
```

- [ ] **Step 1.3: Run the test, verify it fails**

```powershell
cargo test -p antcolony-sim --lib species_extended::tests::diet_extended_food_storage_cap_optional -- --nocapture
```

Expected: compile error (`food_storage_cap` field doesn't exist).

- [ ] **Step 1.4: Add the field to `DietExtended`**

In `crates/antcolony-sim/src/species_extended.rs`, add to the `DietExtended` struct definition:

```rust
    /// Maximum food units this species' colonies can store. None = use
    /// the runtime default (`target_population * egg_cost * 10`). Caps
    /// realistic per-colony reserves; pre-cap, A. rudis colonies grew
    /// 21,000+ food storage in the 2yr smoke (1-2 OOM above field-
    /// realistic). See docs/postmortems/2026-05-09-seasonal-transition-cliffs.md.
    #[serde(default)]
    pub food_storage_cap: Option<f32>,
```

Add to the Default impl:

```rust
            food_storage_cap: None,
```

- [ ] **Step 1.5: Run the test, verify it passes**

```powershell
cargo test -p antcolony-sim --lib species_extended::tests::diet_extended_food_storage_cap_optional
```

Expected: PASS.

- [ ] **Step 1.6: Commit**

```powershell
git add crates/antcolony-sim/src/species_extended.rs
git commit -m "feat(species): add optional food_storage_cap field to DietExtended"
```

---

## Task 2: Enforce `food_storage_cap` in colony deposit path

Need to find every site where `colony.food_stored += <delta>` happens and clamp it. Then add a regression test.

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs` (every `food_stored +=` site)
- Test: same file, new test in inline `mod tests`

- [ ] **Step 2.1: Find every food_stored deposit site**

```powershell
cd J:\antcolony
Select-String -Path crates/antcolony-sim/src/simulation.rs -Pattern "food_stored\s*\+="
```

Expected: a small list of line numbers. Note them. (You'll wrap each with the cap.)

- [ ] **Step 2.2: Write a failing food-cap regression test**

Add to inline `mod tests`:

```rust
#[test]
fn food_storage_cap_clamps_deposits() {
    // Default cap = target_population * egg_cost * 10. With small_config
    // defaults (target=50, egg_cost=5), default cap = 2500. Stuffing
    // 10000 into food_stored should clamp.
    let mut cfg = small_config();
    cfg.ant.initial_count = 0;
    cfg.colony.target_population = 50;
    cfg.colony.egg_cost = 5.0;
    cfg.colony.queen_egg_rate = 0.0;
    cfg.colony.adult_food_consumption = 0.0;
    let mut sim = Simulation::new(cfg, 1);

    // Manually inject a huge food deposit and re-run economy tick.
    sim.colonies[0].food_stored = 10_000.0;
    sim.colony_economy_tick();
    assert!(
        sim.colonies[0].food_stored <= 2500.0 + 1e-3,
        "food_stored should clamp to default cap (2500), got {}",
        sim.colonies[0].food_stored
    );
}

#[test]
fn food_storage_cap_respects_species_override() {
    // When species TOML sets food_storage_cap explicitly, use that value.
    let mut cfg = small_config();
    cfg.ant.initial_count = 0;
    cfg.colony.target_population = 50;
    cfg.colony.egg_cost = 5.0;
    cfg.colony.queen_egg_rate = 0.0;
    cfg.colony.adult_food_consumption = 0.0;
    // species_extended override mechanism — assume there's a runtime path
    // to set this. If not, plumb it through. For test, set on colony.
    let mut sim = Simulation::new(cfg, 1);
    sim.colonies[0].food_storage_cap_override = Some(500.0);
    sim.colonies[0].food_stored = 10_000.0;
    sim.colony_economy_tick();
    assert!(
        sim.colonies[0].food_stored <= 500.0 + 1e-3,
        "food_stored should respect species override cap (500), got {}",
        sim.colonies[0].food_stored
    );
}
```

- [ ] **Step 2.3: Run, verify both fail**

```powershell
cargo test -p antcolony-sim --lib tests::food_storage_cap
```

Expected: compile errors for `food_storage_cap_override` field on Colony, then logic failures once that compiles.

- [ ] **Step 2.4: Add `food_storage_cap_override` to `Colony`**

In `crates/antcolony-sim/src/colony.rs`, find the `Colony` struct and add:

```rust
    /// Per-colony storage cap override. Populated from species
    /// TOML's `diet.food_storage_cap` at colony creation. None =
    /// use the runtime default in `effective_food_cap()`.
    pub food_storage_cap_override: Option<f32>,
```

Add to any `Colony::new(...)` constructor: `food_storage_cap_override: None,`. (Search `Colony \{` for any direct struct literal.)

- [ ] **Step 2.5: Add an `effective_food_cap()` helper on Colony**

In `colony.rs`:

```rust
impl Colony {
    /// Returns the effective storage cap for this colony. Uses
    /// per-species override if set, otherwise default of
    /// `target_population * egg_cost * 10`.
    pub fn effective_food_cap(&self, target_population: u32, egg_cost: f32) -> f32 {
        self.food_storage_cap_override
            .unwrap_or((target_population.max(1) as f32) * egg_cost * 10.0)
    }
}
```

- [ ] **Step 2.6: Wire the cap into colony_economy_tick**

In `crates/antcolony-sim/src/simulation.rs` `colony_economy_tick`, find the per-colony body (the loop that iterates `for colony in &mut self.colonies`). At the END of that per-colony body — AFTER all in-block `colony.food_stored +=` / `-=` / `=` mutations (consumption, cannibalism, trophic-egg deposits, egg-laying cost), and BEFORE the loop moves on to the next colony — add the cap clamp. Single clamp per colony per tick is sufficient because all forager-deposit sites accumulate into `food_stored` between consecutive economy ticks:

```rust
            // Clamp food_stored to the per-colony cap. Prevents the
            // food-overaccumulation pathology observed in the 2yr
            // smoke (rudis hit 44k food / 960 workers, 1-2 OOM above
            // field-realistic). Cap = species TOML override OR
            // target_population * egg_cost * 10 by default.
            let cap = colony.effective_food_cap(ccfg.target_population, ccfg.egg_cost);
            if colony.food_stored > cap {
                colony.food_stored = cap;
            }
```

- [ ] **Step 2.7: Run the cap tests, verify they pass**

```powershell
cargo test -p antcolony-sim --lib tests::food_storage_cap
```

Expected: both tests PASS.

- [ ] **Step 2.8: Run full lib suite to catch regressions**

```powershell
cargo test -p antcolony-sim --lib 2>&1 | Tee-Object -FilePath after_task2_tests.log
```

Compare against `baseline_tests.log` from Step 0.2. No regressions allowed (any pre-existing test that fails was passing before — must investigate).

- [ ] **Step 2.9: Commit**

```powershell
git add crates/antcolony-sim/src/simulation.rs crates/antcolony-sim/src/colony.rs
git commit -m "feat(sim): per-colony food_storage_cap (postmortem fix #4)"
```

---

## Task 3: Egg-lay food gate decoupling (postmortem fix #1)

The autumn cliff. Currently `simulation.rs:3208`:

```rust
if queen_alive && !colony.fertility_suppressed && colony.food_stored >= ccfg.egg_cost {
```

binary-gates the queen. Replace with `food_stored > 0.0`, scale lay rate by stored-food ratio.

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs:3208` and surrounding lay rate math
- Test: inline `mod tests`

- [ ] **Step 3.1: Write a failing autumn-cliff regression test**

Add to inline `mod tests`:

```rust
#[test]
fn queen_lays_at_throttled_rate_when_food_below_egg_cost() {
    // Pre-fix: queen stops laying entirely when food_stored < egg_cost (5.0).
    // Post-fix: queen lays at a reduced rate proportional to food
    // availability, respecting the existing throttle's ENDOGENOUS_FLOOR.
    // This prevents the autumn pre-diapause cliff where colonies
    // chopped between food=2 and food=8 and never recovered.
    let mut cfg = small_config();
    cfg.ant.initial_count = 0;
    cfg.colony.queen_egg_rate = 1.0;
    cfg.colony.adult_food_consumption = 0.0;
    cfg.colony.egg_cost = 5.0;
    let mut sim = Simulation::new(cfg, 1);

    // Set sustained low-food state: small reserve, no inflow.
    sim.colonies[0].food_stored = 2.0;        // BELOW egg_cost
    sim.colonies[0].food_inflow_recent = 0.0; // throttle to floor

    let eggs_before = sim.colonies[0].eggs;
    for _ in 0..50 {
        sim.colony_economy_tick();
        // Re-top food each tick to maintain "low but non-zero"
        // sustenance — models a colony bringing in trickle food.
        sim.colonies[0].food_stored = 2.0;
    }
    let laid = sim.colonies[0].eggs - eggs_before;
    assert!(
        laid > 0,
        "queen must lay SOME eggs even when food < egg_cost \
         (post-fix throttle should yield trickle laying)"
    );
}
```

- [ ] **Step 3.2: Run, verify it fails**

```powershell
cargo test -p antcolony-sim --lib tests::queen_lays_at_throttled_rate_when_food_below_egg_cost
```

Expected: FAIL with `laid = 0`.

- [ ] **Step 3.3: Apply the fix at simulation.rs:3208**

Replace the current block:

```rust
            if queen_alive && !colony.fertility_suppressed && colony.food_stored >= ccfg.egg_cost {
                colony.egg_accumulator += effective_egg_rate;
                let mut laid_this_tick: u32 = 0;
                while colony.egg_accumulator >= 1.0
                    && colony.food_stored >= ccfg.egg_cost
                    && laid_this_tick < MAX_EGGS_PER_TICK
                {
                    colony.egg_accumulator -= 1.0;
                    colony.food_stored -= ccfg.egg_cost;
                    let caste = sample_caste(&mut self.rng, colony.caste_ratio);
                    colony.brood.push(Brood::new_egg(caste));
                    colony.eggs += 1;
```

With (note the removed binary check on the outer guard, plus a new
food_factor that scales effective_egg_rate when reserves are below egg_cost):

```rust
            if queen_alive && !colony.fertility_suppressed && colony.food_stored > 0.0 {
                // Soft food-gate: when reserves are below egg_cost, scale
                // the lay rate by what fraction of an egg we can afford.
                // This decouples from the binary slam-shut that caused
                // the autumn pre-diapause cliff (postmortem fix #1).
                let food_factor = (colony.food_stored / ccfg.egg_cost).min(1.0);
                colony.egg_accumulator += effective_egg_rate * food_factor;
                let mut laid_this_tick: u32 = 0;
                while colony.egg_accumulator >= 1.0
                    && colony.food_stored > 0.0
                    && laid_this_tick < MAX_EGGS_PER_TICK
                {
                    // Pay what we have — partial-cost eggs are biologically
                    // a poorly-nourished cohort but better than zero. Real
                    // queens reduce egg size under stress (Bourke & Franks
                    // 1995). Floor cost at 10% of egg_cost to avoid free
                    // eggs at food = epsilon.
                    let actual_cost = ccfg.egg_cost.min(colony.food_stored).max(ccfg.egg_cost * 0.1);
                    if colony.food_stored < actual_cost {
                        break;
                    }
                    colony.egg_accumulator -= 1.0;
                    colony.food_stored -= actual_cost;
                    let caste = sample_caste(&mut self.rng, colony.caste_ratio);
                    colony.brood.push(Brood::new_egg(caste));
                    colony.eggs += 1;
```

- [ ] **Step 3.4: Run the new test + the existing throttle test**

```powershell
cargo test -p antcolony-sim --lib -- tests::queen_lays_at_throttled_rate_when_food_below_egg_cost tests::queen_lay_rate_throttled_by_food_inflow
```

Expected: BOTH pass. The existing `queen_lay_rate_throttled_by_food_inflow` at line 4602 must remain green.

- [ ] **Step 3.5: Run full lib suite, compare to baseline**

```powershell
cargo test -p antcolony-sim --lib 2>&1 | Tee-Object -FilePath after_task3_tests.log
```

No regressions vs `baseline_tests.log`.

- [ ] **Step 3.6: Commit**

```powershell
git add crates/antcolony-sim/src/simulation.rs
git commit -m "fix(sim): decouple egg-lay food gate from egg_cost (postmortem #1, autumn cliff)"
```

---

## Task 4: `food_inflow_recent` diapause-skip (postmortem fix #2)

The spring cliff. Currently `simulation.rs:3012`:

```rust
            colony.food_inflow_recent *= 0.993;
```

runs every tick — including all 90+ days of diapause. Skip during diapause.

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs:3012`
- Test: inline `mod tests`

- [ ] **Step 4.1: Write a failing spring-cliff regression test**

Add to inline `mod tests`:

```rust
#[test]
fn food_inflow_recent_preserved_through_diapause() {
    // Pre-fix: a 90-day winter decays food_inflow_recent from any value
    // to ~0 via *= 0.993/tick over ~3.9M ticks — leaving the queen-lay
    // throttle pinned at ENDOGENOUS_FLOOR=0.2 for weeks after spring
    // wakeup, while adults try to ramp foraging back up. Result: 4
    // species died at year-1 DOY 75-80 in the smoke.
    // Post-fix: the *= 0.993 decay is skipped while in_diapause, so
    // the throttle resumes spring at last-active value.
    let mut cfg = small_config();
    cfg.ant.hibernation_required = true;
    cfg.ant.hibernation_cold_threshold_c = 10.0;
    cfg.ant.hibernation_warm_threshold_c = 12.0;
    cfg.ant.initial_count = 0;
    cfg.colony.queen_egg_rate = 0.0;  // isolate the inflow decay
    cfg.colony.adult_food_consumption = 0.0;
    let mut sim = Simulation::new(cfg, 1);

    // Force always-cold climate so colony enters diapause.
    sim.climate.seasonal_mid_c = 5.0;
    sim.climate.seasonal_amplitude_c = 0.0;
    sim.climate.starting_day_of_year = 0;
    let winter_amb = sim.ambient_temp_c();
    let m = sim.topology.module_mut(0);
    for v in m.temperature.iter_mut() {
        *v = winter_amb;
    }

    // Pre-load food_inflow_recent at a healthy active-season value.
    sim.colonies[0].food_inflow_recent = 50.0;

    // Run 1000 ticks of "winter". Pre-fix this would decay to
    // 50 * 0.993^1000 ≈ 5e-2. Post-fix it should stay at ~50.
    for _ in 0..1000 {
        sim.colony_economy_tick();
    }

    let preserved = sim.colonies[0].food_inflow_recent;
    assert!(
        preserved > 25.0,
        "food_inflow_recent should not decay during diapause (was 50.0, now {})",
        preserved
    );
}
```

- [ ] **Step 4.2: Run, verify it fails**

```powershell
cargo test -p antcolony-sim --lib tests::food_inflow_recent_preserved_through_diapause
```

Expected: FAIL — `preserved ≈ 0.045` (decayed to near zero).

- [ ] **Step 4.3: Apply the fix at simulation.rs:3012**

Replace:

```rust
            colony.food_inflow_recent *= 0.993;
```

With:

```rust
            // Decay the food-inflow running average toward zero — half
            // life ~100 ticks. ONLY decay while colony is active; during
            // diapause, foragers are not bringing in food but workers
            // also aren't consuming, so the throttle baseline carries
            // over to spring. Pre-fix, 90+ days of decay left the
            // throttle pinned at ENDOGENOUS_FLOOR for weeks post-thaw,
            // killing 4 species at year-1 DOY 75-80 in the 2yr smoke.
            // Postmortem fix #2; see docs/postmortems/2026-05-09-*.md.
            if !in_diapause {
                colony.food_inflow_recent *= 0.993;
            }
```

- [ ] **Step 4.4: Run new test + the existing within-diapause test**

```powershell
cargo test -p antcolony-sim --lib -- tests::food_inflow_recent_preserved_through_diapause tests::diapausing_adults_dont_starve_when_reserves_run_out
```

Expected: BOTH pass.

- [ ] **Step 4.5: Run full lib suite, compare to baseline**

```powershell
cargo test -p antcolony-sim --lib 2>&1 | Tee-Object -FilePath after_task4_tests.log
```

No regressions.

- [ ] **Step 4.6: Commit**

```powershell
git add crates/antcolony-sim/src/simulation.rs
git commit -m "fix(sim): preserve food_inflow_recent through diapause (postmortem #2, spring cliff)"
```

---

## Task 5: Smooth adult-starvation cap (postmortem fix #3)

`simulation.rs:3118` currently:

```rust
                    let cap = ((adult_total as f32 * 0.05).ceil() as u32).max(1);
```

5%/tick × ~75 ticks per log interval = total wipe in <1 in-game day. Smooth to ~1%/day equivalent.

At 30Hz × 1440 sec/day Seasonal scale = 43,200 ticks/day. 1%/day = 1/43,200/100 = ~2.31e-5/tick.

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs:3118`
- Test: inline `mod tests`

- [ ] **Step 5.1: Write a failing smooth-starvation test**

Add to inline `mod tests`:

```rust
#[test]
fn starvation_deaths_smooth_not_cliff() {
    // Pre-fix: 5%/tick of adults die when food_stored < 0 — wipes
    // 500-adult colony in ~75 ticks (single log interval). Post-fix:
    // ~1%/day = ~2.31e-5/tick smooths the cliff so deaths smear over
    // weeks. A 500-adult colony should not lose more than ~5% over
    // 100 ticks of starvation.
    let mut cfg = small_config();
    cfg.ant.initial_count = 0;
    cfg.colony.queen_egg_rate = 0.0;
    cfg.colony.adult_food_consumption = 1.0;
    let mut sim = Simulation::new(cfg, 1);

    // Spawn 500 adult workers (active colony, not diapause).
    for i in 0..500u32 {
        let a = Ant::new_worker(20_000 + i, 0, Vec2::new(5.0, 5.0), 0.0, 10.0);
        sim.ants.push(a);
        sim.colonies[0].population.workers += 1;
    }
    sim.colonies[0].food_stored = 0.0;
    sim.colonies[0].brood.clear();
    sim.colonies[0].eggs = 0;
    sim.colonies[0].larvae = 0;
    sim.colonies[0].pupae = 0;

    let initial = sim.colonies[0].population.workers;
    for _ in 0..100 {
        sim.colony_economy_tick();
    }
    let after = sim.colonies[0].population.workers;
    let lost = initial - after;
    let lost_frac = lost as f32 / initial as f32;
    assert!(
        lost_frac < 0.10,
        "starvation cap should smooth deaths (lost {}/{} = {:.1}% in 100 ticks)",
        lost, initial, lost_frac * 100.0
    );
}
```

- [ ] **Step 5.2: Run, verify it fails**

```powershell
cargo test -p antcolony-sim --lib tests::starvation_deaths_smooth_not_cliff
```

Expected: FAIL — pre-fix loses ~5% per tick = ~99% over 100 ticks.

- [ ] **Step 5.3: Apply the fix at simulation.rs:3118**

Replace:

```rust
                    let cap = ((adult_total as f32 * 0.05).ceil() as u32).max(1);
```

With:

```rust
                    // Smooth starvation: per-tick cap at ~1%/day equivalent
                    // (1% per 43200 Seasonal ticks/day = ~2.31e-5/tick).
                    // ceil(...).max(1) preserves the floor so a tiny
                    // colony still loses one ant per few ticks when starving.
                    // Pre-fix, 5%/tick wiped a 500-adult cohort in ~75 ticks
                    // (one log interval), making the seasonal cliff invisible
                    // until it had already happened. Postmortem fix #3.
                    const STARVATION_PER_TICK: f32 = 1.0 / 43_200.0 / 100.0; // 1%/day
                    let cap = ((adult_total as f32 * STARVATION_PER_TICK).ceil() as u32).max(1);
```

- [ ] **Step 5.4: Run new test + the diapause-survival test (regression check)**

```powershell
cargo test -p antcolony-sim --lib -- tests::starvation_deaths_smooth_not_cliff tests::diapausing_adults_dont_starve_when_reserves_run_out
```

Expected: BOTH pass.

- [ ] **Step 5.5: Run full lib suite vs baseline**

```powershell
cargo test -p antcolony-sim --lib 2>&1 | Tee-Object -FilePath after_task5_tests.log
```

No regressions.

- [ ] **Step 5.6: Commit**

```powershell
git add crates/antcolony-sim/src/simulation.rs
git commit -m "fix(sim): smooth adult-starvation cap to ~1%/day (postmortem #3)"
```

---

## Task 6: Stochastic worker mortality (postmortem fix #5)

`worker_lifespan_months` exists in `species.rs:83` but is never read for mortality decisions. Wire it in: per-tick `1/lifespan_ticks` death probability per adult.

Lifespan 3 months × 30 days × 43,200 ticks/day = 3,888,000 ticks. Per-tick prob ≈ 2.57e-7. Over a 200-tick test with 50 workers: expected deaths ≈ 0.0026 (negligible — won't break existing tests).

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs` (find the per-tick adult update site near line 1328 where `ant.age = ant.age.saturating_add(1)`)
- Test: inline `mod tests`

- [ ] **Step 6.1: Locate the per-tick adult age increment**

```powershell
Select-String -Path crates/antcolony-sim/src/simulation.rs -Pattern "ant\.age\s*="
```

Expected: line 1328 area. Read +/- 30 lines for context — this is likely inside an ant-update loop. Mortality check should fire at the same site.

- [ ] **Step 6.2: Write a failing stochastic-mortality test**

Add to inline `mod tests`:

```rust
#[test]
fn workers_die_stochastically_at_lifespan_rate() {
    // Wire worker_lifespan_months into per-tick mortality. With
    // lifespan = 1 month = 30 days = 1.296M ticks at Seasonal scale,
    // per-tick death prob ≈ 7.7e-7. Over 100k ticks with 1000 workers,
    // expected deaths ≈ 1000 * 100_000 * 7.7e-7 = 77 deaths.
    // Allow ±50% tolerance for stochastic variance.
    let mut cfg = small_config();
    cfg.ant.initial_count = 0;
    cfg.species.worker_lifespan_months = 1.0; // short for fast test
    cfg.colony.queen_egg_rate = 0.0;
    cfg.colony.adult_food_consumption = 0.0;
    let mut sim = Simulation::new(cfg, 42); // seed 42 for determinism

    // Spawn 1000 adult workers.
    for i in 0..1000u32 {
        let a = Ant::new_worker(50_000 + i, 0, Vec2::new(5.0, 5.0), 0.0, 10.0);
        sim.ants.push(a);
        sim.colonies[0].population.workers += 1;
    }
    sim.colonies[0].food_stored = 1_000_000.0; // never starvation

    let initial = sim.colonies[0].population.workers;
    for _ in 0..100_000 {
        sim.tick(); // full tick to advance ages
    }
    let after = sim.colonies[0].population.workers;
    let died = initial - after;
    assert!(
        died >= 30 && died <= 150,
        "expected ~77 stochastic deaths over 100k ticks at 1mo lifespan; got {}",
        died
    );
}
```

- [ ] **Step 6.3: Run, verify it fails**

```powershell
cargo test -p antcolony-sim --lib tests::workers_die_stochastically_at_lifespan_rate
```

Expected: FAIL — currently `died = 0` (no mortality wired).

- [ ] **Step 6.4: Implement stochastic mortality**

Find the per-tick ant update loop (around line 1328 from Step 6.1). Add a mortality check inside the per-ant body, using the colony's species config. Pseudocode (adapt to actual surrounding code):

```rust
            // Stochastic worker mortality. Per-tick death probability =
            // 1 / lifespan_ticks. Replaces deterministic age-out (which
            // never existed — worker_lifespan_months was an unused TOML
            // field). Smooths cohort dynamics: instead of all workers
            // born in week N dying simultaneously in week N+12, deaths
            // smear evenly. Postmortem fix #5.
            //
            // NOTE: Uses self.rng (seeded), NOT thread_rng() — sim
            // determinism is byte-identical and depends on RNG consumption
            // being deterministic.
            if matches!(ant.caste, AntCaste::Worker | AntCaste::Soldier) {
                let lifespan_months = self.config.species.worker_lifespan_months.max(0.1);
                // 30 days/month * 43200 ticks/day = 1.296M ticks/month
                let lifespan_ticks = lifespan_months * 1_296_000.0;
                let p_die = 1.0 / lifespan_ticks;
                use rand::Rng;
                if self.rng.r#gen::<f32>() < p_die {
                    ant.health = 0.0; // existing death-handling path will reap
                }
            }
```

(NOTE: `r#gen` is the edition-2024 raw-identifier form — see project memory `feedback_edition2024_gen_keyword.md`. If the surrounding code already uses a different RNG-call pattern, mirror that.)

You'll need to ensure the death-cleanup path (where `ant.health <= 0` removes the ant from `self.ants` and decrements `colony.population.workers`/`soldiers`) actually runs in the same `tick()`. Check the combat path or the dedicated cleanup pass. If not, decrement the population counter inline:

```rust
            if ant_died_from_aging {
                match ant.caste {
                    AntCaste::Worker => {
                        if let Some(c) = self.colonies.iter_mut().find(|c| c.id == ant.colony_id) {
                            c.population.workers = c.population.workers.saturating_sub(1);
                        }
                    }
                    AntCaste::Soldier => {
                        if let Some(c) = self.colonies.iter_mut().find(|c| c.id == ant.colony_id) {
                            c.population.soldiers = c.population.soldiers.saturating_sub(1);
                        }
                    }
                    _ => {}
                }
            }
```

- [ ] **Step 6.5: Run new test, verify it passes**

```powershell
cargo test -p antcolony-sim --lib tests::workers_die_stochastically_at_lifespan_rate
```

Expected: PASS — `died` in 30-150 range.

- [ ] **Step 6.6: Run full lib suite vs baseline**

```powershell
cargo test -p antcolony-sim --lib 2>&1 | Tee-Object -FilePath after_task6_tests.log
```

Compare against `baseline_tests.log`. **Some existing tests may now fail** if they depend on exact RNG state or assume zero mortality over many ticks. Investigate any new failures:
- If a test runs <1000 ticks and asserts no deaths → likely fine (expected mortality is sub-1)
- If a test asserts exact ant counts after long runs → may need updating
- If a test fails on RNG state divergence → expected; the new RNG draws shifted the sequence

Fix or update the failing tests inline, documenting "test updated to allow expected stochastic mortality."

- [ ] **Step 6.7: Commit**

```powershell
git add crates/antcolony-sim/src/simulation.rs
git commit -m "feat(sim): stochastic worker mortality from worker_lifespan_months (postmortem #5)"
```

---

## Task 7: Verify all unit tests green + capture diff

Hard checkpoint before moving to cnc.

- [ ] **Step 7.1: Run the entire workspace test suite**

```powershell
cd J:\antcolony
cargo test --workspace 2>&1 | Tee-Object -FilePath phase1_final_tests.log
```

Expected: ALL pass (137+ baseline + new tests added in Phase 1, minus any deliberately deleted).

- [ ] **Step 7.2: If anything fails, STOP and triage**

Do not proceed to the cnc smoke until tests are green. The smoke is expensive (3-5 days wall-clock); running it on a broken sim wastes time.

---

## Task 8: Provision cnc-server for smoke

One-time setup. Establishes a clean `/opt/antcolony/` workspace on cnc with trimmed Cargo.toml (no Bevy, no render, no trainer, no net) and a release-built `smoke_10yr_ai` binary.

**Files:**
- Create: `scripts/cnc_provision.ps1`

- [ ] **Step 8.1: Create the provisioning script**

`scripts/cnc_provision.ps1`:

```powershell
# Provisions cnc-server (192.168.168.100) with the antcolony source
# trimmed to sim-only, then builds the smoke harness binary.
# One-time per source-tree refresh.

$ErrorActionPreference = 'Stop'
$LocalRoot = 'J:\antcolony'
$RemoteRoot = '/opt/antcolony'
$RemoteHost = 'cnc-server'

Write-Host "==> Ensuring /opt/antcolony exists on cnc..."
ssh $RemoteHost "sudo mkdir -p $RemoteRoot && sudo chown root:root $RemoteRoot"

Write-Host "==> Rsyncing source (excluding target/ and bench/)..."
# Use scp+tar fallback since rsync may not be in Git Bash on Windows
Push-Location $LocalRoot
tar -cf - --exclude='target' --exclude='bench' --exclude='.git' --exclude='node_modules' . | ssh $RemoteHost "sudo tar -xf - -C $RemoteRoot"
Pop-Location

Write-Host "==> Trimming workspace Cargo.toml on cnc to sim-only..."
$TrimmedCargo = @'
[workspace]
members = ["crates/antcolony-sim"]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.85"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "1"
anyhow = "1"
rand = "0.8"
glam = { version = "0.27", features = ["serde"] }
'@

$TrimmedCargoEscaped = $TrimmedCargo -replace "'", "''"
ssh $RemoteHost "sudo bash -c `"cat > $RemoteRoot/Cargo.toml << 'EOF'
$TrimmedCargo
EOF`""

Write-Host "==> Building smoke_10yr_ai release binary..."
ssh $RemoteHost "cd $RemoteRoot && cargo build --release --example smoke_10yr_ai"

Write-Host "==> Verifying binary exists..."
ssh $RemoteHost "ls -lh $RemoteRoot/target/release/examples/smoke_10yr_ai"

Write-Host "==> Provision complete."
```

- [ ] **Step 8.2: Run the provisioning script**

```powershell
cd J:\antcolony
.\scripts\cnc_provision.ps1
```

Expected: source rsync'd, Cargo.toml trimmed, release build completes (~10-15 min on cnc), binary at `/opt/antcolony/target/release/examples/smoke_10yr_ai`.

If build fails:
- Check `swapon --show` on cnc — confirm 8GB swap is active
- Check rustc version (`ssh cnc-server rustc --version`) — should be 1.93.1+
- Look for missing workspace deps that the trimmed Cargo.toml omits (may need to copy-paste from full Cargo.toml)

- [ ] **Step 8.3: Commit the script**

```powershell
git add scripts/cnc_provision.ps1
git commit -m "infra(cnc): one-time provisioning script for sim smoke runs"
```

---

## Task 9: Run 2yr 10-species smoke on cnc

Long wall-clock task (3-5 days). Run 2-at-a-time to keep fleet headroom.

**Files:**
- Create: `scripts/cnc_smoke.ps1`

- [ ] **Step 9.1: Create the smoke launcher script**

`scripts/cnc_smoke.ps1`:

```powershell
# Launches a 2yr HeuristicBrain smoke for all 10 species on cnc-server,
# 2 species at a time. Polls completion, pulls daily.csv outputs back
# to local bench/smoke-phase1-2yr/ when each species finishes.

$ErrorActionPreference = 'Stop'
$LocalOutDir = 'J:\antcolony\bench\smoke-phase1-2yr'
$RemoteRoot = '/opt/antcolony'
$RemoteOutRoot = "$RemoteRoot/runs/phase1-2yr"
$RemoteHost = 'cnc-server'
$Species = @(
    'lasius_niger',
    'pogonomyrmex_occidentalis',
    'formica_rufa',
    'camponotus_pennsylvanicus',
    'tapinoma_sessile',
    'tetramorium_immigrans',
    'aphaenogaster_rudis',
    'formica_fusca',
    'brachyponera_chinensis',
    'temnothorax_curvinodis'
)

Write-Host "==> Setting up remote output directory..."
ssh $RemoteHost "sudo mkdir -p $RemoteOutRoot && sudo chown -R `$(whoami) $RemoteOutRoot"

Write-Host "==> Launching smoke runs (2 at a time)..."
for ($i = 0; $i -lt $Species.Count; $i += 2) {
    $batch = $Species[$i..([Math]::Min($i+1, $Species.Count-1))]
    Write-Host "  Batch: $($batch -join ', ')"

    foreach ($sp in $batch) {
        $cmd = "cd $RemoteRoot && nohup ./target/release/examples/smoke_10yr_ai --years 2 --no-mlp --species $sp --seed 42 --out $RemoteOutRoot/$sp > $RemoteOutRoot/$sp.log.out 2> $RemoteOutRoot/$sp.log.err &"
        ssh $RemoteHost $cmd
        Write-Host "    Launched $sp"
    }

    # Poll every 5 min until BOTH in this batch finish
    Write-Host "  Waiting for batch to finish..."
    do {
        Start-Sleep -Seconds 300
        $running = 0
        foreach ($sp in $batch) {
            $check = ssh $RemoteHost "test -f $RemoteOutRoot/$sp/summary.json && echo done || echo running"
            if ($check.Trim() -eq 'running') { $running++ }
        }
        Write-Host "    [$(Get-Date -Format HH:mm)] $running/$($batch.Count) still running"
    } while ($running -gt 0)

    Write-Host "  Batch done. Pulling daily.csv outputs..."
    foreach ($sp in $batch) {
        $localSpeciesDir = Join-Path $LocalOutDir $sp
        New-Item -ItemType Directory -Force -Path $localSpeciesDir | Out-Null
        scp -r "${RemoteHost}:$RemoteOutRoot/$sp/*" "$localSpeciesDir/"
    }
}

Write-Host "==> All 10 species complete. Outputs at $LocalOutDir"
```

- [ ] **Step 9.2: Launch the smoke**

```powershell
cd J:\antcolony
.\scripts\cnc_smoke.ps1 *>&1 | Tee-Object -FilePath bench\smoke-phase1-2yr.launcher.log
```

This will run for 3-5 days. Leave the PowerShell window open OR run in a background detached process (recommended):

```powershell
Start-Job -ScriptBlock { Set-Location 'J:\antcolony'; .\scripts\cnc_smoke.ps1 } -Name 'antcolony-smoke'
```

To check on it later:

```powershell
Get-Job antcolony-smoke
Receive-Job antcolony-smoke -Keep
```

- [ ] **Step 9.3: Periodic health check (daily during run)**

```powershell
ssh cnc-server "cd /opt/antcolony/runs/phase1-2yr && for d in */; do
  echo \$d; tail -1 \$d/daily.csv 2>/dev/null || echo '  (no daily.csv yet)'
done"
```

Verify each species is making progress (latest daily.csv row tick number going up). If any process appears hung (no progress in 2+ hours), investigate:

```powershell
ssh cnc-server "ps aux | grep smoke_10yr"
ssh cnc-server "free -h && uptime"
```

- [ ] **Step 9.4: Commit the smoke script**

```powershell
git add scripts/cnc_smoke.ps1
git commit -m "infra(cnc): 2yr 10-species smoke launcher with batched parallelism"
```

---

## Task 10: Verify Phase 1 exit criterion

Once all 10 species finish, evaluate against the gate.

**Files:**
- Create: `scripts/verify_phase1_exit.ps1`

- [ ] **Step 10.1: Write the verification script**

`scripts/verify_phase1_exit.ps1`:

```powershell
# Verifies Phase 1 exit criterion against bench/smoke-phase1-2yr/ outputs.
# Hard gate: 10/10 alive at year-2, food/worker < 5 across all daily
# samples, no single-day adult-pop drop > 20%.

$ErrorActionPreference = 'Stop'
$BenchDir = 'J:\antcolony\bench\smoke-phase1-2yr'

$Species = Get-ChildItem -Directory $BenchDir | ForEach-Object { $_.Name }
$failures = @()

Write-Host "Phase 1 Exit Criterion Check"
Write-Host "============================="
Write-Host "Bench dir: $BenchDir"
Write-Host "Species evaluated: $($Species.Count)"
Write-Host ""

foreach ($sp in $Species) {
    $dailyCsv = Join-Path $BenchDir "$sp\daily.csv"
    if (-not (Test-Path $dailyCsv)) {
        $failures += "$sp : no daily.csv"
        continue
    }

    $rows = Import-Csv $dailyCsv
    if ($rows.Count -eq 0) {
        $failures += "$sp : daily.csv is empty"
        continue
    }

    $lastRow = $rows[-1]
    $lastWorkers = [int]$lastRow.workers
    $lastFood = [float]$lastRow.food

    # Gate 1: alive at year-2 end
    if ($lastWorkers -le 0) {
        $failures += "$sp : extinct at year-2 (workers=$lastWorkers)"
        continue
    }

    # Gate 2: food/worker ratio < 5 across ALL daily samples
    $maxRatio = 0.0
    foreach ($r in $rows) {
        $w = [float]$r.workers
        if ($w -le 0) { continue }
        $f = [float]$r.food
        $ratio = $f / $w
        if ($ratio -gt $maxRatio) { $maxRatio = $ratio }
    }
    if ($maxRatio -ge 5.0) {
        $failures += "$sp : food/worker ratio $($maxRatio.ToString('F1')) >= 5 (food-overaccumulation bug not fixed)"
    }

    # Gate 3: no single-day adult-pop drop > 20%
    $adultCols = @('workers', 'soldiers', 'breeders')
    for ($i = 1; $i -lt $rows.Count; $i++) {
        $prevAdults = 0
        $thisAdults = 0
        foreach ($col in $adultCols) {
            if ($rows[$i-1].PSObject.Properties.Name -contains $col) {
                $prevAdults += [int]$rows[$i-1].$col
                $thisAdults += [int]$rows[$i].$col
            }
        }
        if ($prevAdults -gt 10) {
            $drop = ($prevAdults - $thisAdults) / [float]$prevAdults
            if ($drop -gt 0.20) {
                $failures += "$sp : >20% adult-pop drop on day $i (prev=$prevAdults, now=$thisAdults)"
                break # one report per species
            }
        }
    }

    Write-Host "  $sp : OK (workers=$lastWorkers, food=$lastFood, max-ratio=$($maxRatio.ToString('F2')))"
}

Write-Host ""
if ($failures.Count -gt 0) {
    Write-Host "FAIL ($($failures.Count) failures):" -ForegroundColor Red
    $failures | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
    exit 1
} else {
    Write-Host "PASS — all $($Species.Count) species cleared the Phase 1 exit gate." -ForegroundColor Green
    exit 0
}
```

- [ ] **Step 10.2: Run the verification**

```powershell
.\scripts\verify_phase1_exit.ps1
```

Expected: PASS. If FAIL:
- Re-read the failure list. Each line names the species + which gate failed.
- Triage by failure type:
  - **Extinct**: cliff fix didn't hold for that species. Check daily.csv last 10 rows for the death pattern (autumn vs spring). Re-do the fix targeted at that mode.
  - **Food/worker > 5**: food cap not effective. Check whether the species TOML has a `food_storage_cap` override that's too high, or whether the deposit-time clamp isn't running. Add diagnostic logging.
  - **>20% drop**: a new cliff. Investigate via decisions.csv if available.
- Once root cause identified, write a regression test for it, fix the code, rerun the smoke (this time only for the failing species — don't redo the whole 10 unless the fix could affect all).

- [ ] **Step 10.3: Commit verification script regardless of outcome**

```powershell
git add scripts/verify_phase1_exit.ps1
git commit -m "infra: Phase 1 exit-criterion verification script"
```

---

## Task 11: Roll forward HANDOFF.md, push

- [ ] **Step 11.1: Update HANDOFF.md with Phase 1 completion entry**

Add a new top section to `HANDOFF.md` matching the existing date-prefixed pattern. Cover:

- Phase 1 status: SHIPPED (cite commit SHAs)
- 5 sim fixes applied: list + 1-line each
- 2yr 10-species smoke: PASSED (link to bench/smoke-phase1-2yr/)
- Phase 1 exit gate cleared (cite verify script output)
- Next session: Phase 2 (parallel sim features) — link to spec

- [ ] **Step 11.2: Stage and commit**

```powershell
git add HANDOFF.md
git commit -m "docs(handoff): Phase 1 sim foundation shipped, 2yr smoke 10/10 green"
```

- [ ] **Step 11.3: Push to origin**

```powershell
git push origin main
```

---

## Self-review notes

- All five postmortem fixes (#1, #2, #3, #4, #5) covered in Tasks 1-6 (#4 is Tasks 1+2 split: schema + enforcement).
- Determinism preserved: all RNG goes through `self.rng`, no `thread_rng()`.
- Existing tests are unlikely to break under stochastic mortality (sub-1 expected death over typical 100-500-tick test horizons), but Step 6.6 explicitly catches and triages any that do.
- cnc swap is already in place from the brainstorming session; provisioning can skip that step.
- Smoke is the longest wall-clock item (3-5 days) and runs entirely on cnc — does not block kokonoe dev. After Task 9 starts, you can move to Phase 2 plan-writing in parallel.
- Phase 1 ships a 2yr smoke. The 7yr Pogonomyrmex run for Cole/Wiernasz is Phase 3 work, not Phase 1.

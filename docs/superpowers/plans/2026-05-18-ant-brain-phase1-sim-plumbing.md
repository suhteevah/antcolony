# Hierarchical Ant Brain — Phase 1: Sim-Side Observation/Action Plumbing

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add observation and action types + `Simulation` API needed by the hierarchical brain trainer, with defaults that reproduce current sim behavior bit-for-bit. No trainer-side or NN code in this phase — that's Phase 2.

**Architecture:** Five new public types in `antcolony-sim`. The `Ant` struct grows one `modulators: AntModulators` field whose default is the identity transform of the existing ACO math at `ant.rs:198-246`. Four new `Simulation` methods expose richer obs + accept per-ant modulators + commander intent. Existing single-tier trainer at `crates/antcolony-trainer/` is left untouched — it stays as the 47% Nash regression baseline. The phase ends with a regression test that runs the sim with the new plumbing in default mode and asserts byte-identical output to a seed-matched run before the plumbing.

**Tech Stack:** Rust 2024 (MSRV 1.85, but this crate compiles fine on stable per the project memory). `serde` for type derives, `arrayvec::ArrayVec` for the bounded history ring buffer (already in workspace per pheromone.rs's existing use), `glam::Vec2` (already imported), `rand` / `rand_chacha` for deterministic test seeds (already imported across the crate). No new dependencies needed.

**Predecessor:** `docs/superpowers/specs/2026-05-18-ant-brain-hierarchical-design.md` (approved 2026-05-18).
**Successor:** Phase 2 plan — trainer-side hierarchical policy net (`CommanderPolicy`, `AntPolicy`, `HierarchicalActorCritic`, joint PPO).

---

## File map

| File | Status | Responsibility |
|---|---|---|
| `crates/antcolony-sim/src/ai/observation.rs` | **CREATE** | All new types: `PheromoneSnapshot`, `HistoryToken`, `RichObservation`, `AntObservation`, `AntModulators` |
| `crates/antcolony-sim/src/ai/mod.rs` | MODIFY | Add `pub mod observation;` + public re-exports |
| `crates/antcolony-sim/src/ant.rs` | MODIFY | Add `modulators: AntModulators` field to `Ant`; init in `new_with_caste`; apply in `choose_direction` |
| `crates/antcolony-sim/src/colony.rs` | MODIFY | Add `commander_history: ArrayVec<HistoryToken, 8>` and `commander_intent: [f32; 64]` to `ColonyState` (initialized to empty + zero) |
| `crates/antcolony-sim/src/simulation.rs` | MODIFY | Four new methods: `colony_rich_observation`, `per_ant_observations`, `apply_ant_modulators`, `apply_commander_intent` |
| `crates/antcolony-sim/src/pheromone.rs` | MODIFY | Add `downsample_to(w, h)` helper for adaptive average pooling to 32×32 |
| `crates/antcolony-sim/src/lib.rs` | MODIFY | Re-export the 5 new types from `antcolony_sim` root |
| `crates/antcolony-sim/tests/phase1_plumbing.rs` | **CREATE** | Cross-module integration tests (regression + non-default modulators changing behavior) |

The implementer should **find `ColonyState`** before Task 6 to confirm it lives in `crates/antcolony-sim/src/colony.rs` (per `lib.rs:36` re-export `pub use colony::{..., ColonyState, ...}`). The struct definition there is where the two new fields go.

---

### Task 1: Create `observation.rs` with `AntModulators` + identity default

**Files:**
- Create: `crates/antcolony-sim/src/ai/observation.rs`
- Modify: `crates/antcolony-sim/src/ai/mod.rs`

- [ ] **Step 1: Write the failing test**

In a new file `crates/antcolony-sim/src/ai/observation.rs`:

```rust
//! Observation + action types for the hierarchical brain (Phase 1).
//!
//! The hierarchical commander/ant brain trainer consumes the types in this
//! module via the four new `Simulation` methods (see simulation.rs). All
//! types here are pure data carriers — no behavior — and serde-roundtrip
//! cleanly so they can be saved with sim snapshots.
//!
//! Defaults are chosen so that a sim run with no trainer attached is
//! byte-identical to today's behavior. See `AntModulators::default`.

use serde::{Deserialize, Serialize};

/// Per-ant ACO knobs the per-ant brain (Phase 2) outputs each tick.
///
/// Defaults are the **identity** for the existing ACO math in
/// `ant.rs::choose_direction`: alpha_mult and beta_mult multiply by 1.0,
/// exploration_mod adds 0.0, deposit_mult multiplies by 1.0, state_bias
/// adds 0.0 to the FSM transition logit it gates. With defaults the sim
/// produces byte-identical output to the pre-plumbing version.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AntModulators {
    /// Multiplier on the pheromone-intensity exponent. Clamped [0.1, 5.0]
    /// at apply time. Default 1.0 (no modulation).
    pub alpha_mult: f32,
    /// Multiplier on the desirability/forward-bias exponent. Clamped
    /// [0.1, 5.0] at apply time. Default 1.0.
    pub beta_mult: f32,
    /// Additive offset to `AntConfig::exploration_rate`. Clamped [-0.1,
    /// 0.1] at apply time. Default 0.0.
    pub exploration_mod: f32,
    /// Multiplier on pheromone deposit strength. Clamped [0.1, 5.0] at
    /// apply time. Default 1.0.
    pub deposit_mult: f32,
    /// Additive logit bias on FSM transition probabilities. Clamped
    /// [-2.0, 2.0] at apply time. Default 0.0.
    pub state_bias: f32,
}

impl Default for AntModulators {
    fn default() -> Self {
        Self {
            alpha_mult: 1.0,
            beta_mult: 1.0,
            exploration_mod: 0.0,
            deposit_mult: 1.0,
            state_bias: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modulators_default_is_identity() {
        let m = AntModulators::default();
        assert_eq!(m.alpha_mult, 1.0);
        assert_eq!(m.beta_mult, 1.0);
        assert_eq!(m.exploration_mod, 0.0);
        assert_eq!(m.deposit_mult, 1.0);
        assert_eq!(m.state_bias, 0.0);
    }

    #[test]
    fn modulators_serde_roundtrip() {
        let m = AntModulators {
            alpha_mult: 2.5,
            beta_mult: 0.5,
            exploration_mod: -0.05,
            deposit_mult: 3.0,
            state_bias: 1.25,
        };
        let json = serde_json::to_string(&m).unwrap();
        let parsed: AntModulators = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, m);
    }
}
```

Add to `crates/antcolony-sim/src/ai/mod.rs` (at the bottom of the existing `pub mod ...` block):

```rust
pub mod observation;

pub use observation::AntModulators;
```

- [ ] **Step 2: Run the test to verify it fails to compile, then passes**

Run: `cargo test -p antcolony-sim --lib ai::observation::tests`
Expected first attempt: COMPILES (the module is straightforward), passes both tests. If the project uses `serde_json` as a dev-dep we're good. If not, the second test errors — in which case add `serde_json = "1"` under `[dev-dependencies]` in `crates/antcolony-sim/Cargo.toml` and re-run. Expected on second attempt: PASS.

- [ ] **Step 3: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/ai/observation.rs crates/antcolony-sim/src/ai/mod.rs crates/antcolony-sim/Cargo.toml
git commit -m "ai/observation: AntModulators with identity default

Phase 1 of hierarchical ant-brain plumbing. AntModulators is the per-ant
action carrier the ant tier outputs each tick. Default is the identity
transform of ant.rs::choose_direction so the sim with no trainer
attached behaves identically to today.

Refs: docs/superpowers/specs/2026-05-18-ant-brain-hierarchical-design.md
Refs: docs/superpowers/plans/2026-05-18-ant-brain-phase1-sim-plumbing.md
"
```

---

### Task 2: Add `modulators` field to `Ant` struct

**Files:**
- Modify: `crates/antcolony-sim/src/ant.rs:46` (`Ant` struct), `:114` (`new_with_caste`)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `crates/antcolony-sim/src/ant.rs`:

```rust
#[test]
fn new_ant_has_default_modulators() {
    let a = Ant::new_worker(1, 0, Vec2::new(5.0, 5.0), 0.0, 10.0);
    assert_eq!(a.modulators, crate::ai::observation::AntModulators::default());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p antcolony-sim --lib ant::tests::new_ant_has_default_modulators`
Expected: COMPILE ERROR — `no field 'modulators' on Ant`.

- [ ] **Step 3: Add the field**

In `crates/antcolony-sim/src/ant.rs`, locate the `Ant` struct (line 46-99). Add a new field at the end of the struct, just before the closing `}`:

```rust
    /// Per-ant ACO modulators set by the per-ant brain (Phase 2). With
    /// default values (1.0, 1.0, 0.0, 1.0, 0.0), `choose_direction`
    /// reduces to the pre-Phase-1 ACO formula bit-for-bit. See
    /// `ai::observation::AntModulators`.
    #[serde(default)]
    pub modulators: crate::ai::observation::AntModulators,
```

In `Ant::new_with_caste` (around line 106-134), add to the struct-init block before the closing `}`:

```rust
            modulators: crate::ai::observation::AntModulators::default(),
```

- [ ] **Step 4: Run the test + the whole crate's existing tests to verify nothing regressed**

Run: `cargo test -p antcolony-sim --lib ant::tests`
Expected: ALL PASS including the new `new_ant_has_default_modulators` and the pre-existing `test_fsm_transitions` + `direction_biased_toward_pheromone`.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/ant.rs
git commit -m "ant: add modulators field to Ant struct, default-init in new_with_caste

#[serde(default)] so older snapshots load cleanly. Field is read by
choose_direction in the next task.
"
```

---

### Task 3: Wire `modulators` into `choose_direction` (regression-safe)

**Files:**
- Modify: `crates/antcolony-sim/src/ant.rs:198-246` (`choose_direction` fn)

This is the critical injection point. The function currently reads `cfg.alpha`, `cfg.beta`, `cfg.exploration_rate` from `AntConfig`. After this task it also reads `ant.modulators.{alpha_mult, beta_mult, exploration_mod}`. With defaults the math is unchanged.

- [ ] **Step 1: Write the failing test — defaults reproduce existing behavior**

Add to `crates/antcolony-sim/src/ant.rs`'s `#[cfg(test)] mod tests`:

```rust
#[test]
fn default_modulators_reproduce_baseline_direction() {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    // Set up the same fixture as the existing `direction_biased_toward_pheromone`
    // test (line 267 of this file): strong trail east of an ant at (20.5, 20.5).
    let mut grid = PheromoneGrid::new(40, 40);
    for dx in 1..8 {
        grid.deposit(20 + dx, 20, PheromoneLayer::FoodTrail, 8.0, 10.0);
    }
    let ant = Ant::new_worker(1, 0, Vec2::new(20.5, 20.5), 0.0, 10.0);
    let mut cfg = AntConfig::default();
    cfg.exploration_rate = 0.0;

    // Baseline: 100 picks with default modulators (1.0, 1.0, 0.0, _, _)
    // must majority-trend east (heading near 0) given the trail layout.
    let mut rng = ChaCha8Rng::seed_from_u64(0xa17_de_f);
    let mut eastward = 0;
    for _ in 0..100 {
        let h = choose_direction(&ant, &grid, &cfg, &mut rng);
        // east = heading near 0, within ±π/2
        if h.cos() > 0.0 { eastward += 1; }
    }
    assert!(eastward >= 70, "default modulators should give baseline behavior (≥70/100 eastward), got {}", eastward);
}

#[test]
fn high_alpha_mult_strengthens_pheromone_following() {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    let mut grid = PheromoneGrid::new(40, 40);
    // Weak trail (intensity 0.5 not 8.0) — baseline shouldn't strongly follow.
    for dx in 1..8 {
        grid.deposit(20 + dx, 20, PheromoneLayer::FoodTrail, 0.5, 10.0);
    }
    let mut ant = Ant::new_worker(1, 0, Vec2::new(20.5, 20.5), 0.0, 10.0);
    let mut cfg = AntConfig::default();
    cfg.exploration_rate = 0.0;

    // High alpha_mult should amplify the weak trail's pull.
    ant.modulators.alpha_mult = 5.0;
    let mut rng = ChaCha8Rng::seed_from_u64(0x57e1_a17);
    let mut eastward_high = 0;
    for _ in 0..200 {
        let h = choose_direction(&ant, &grid, &cfg, &mut rng);
        if h.cos() > 0.0 { eastward_high += 1; }
    }

    ant.modulators.alpha_mult = 1.0;  // baseline
    let mut rng = ChaCha8Rng::seed_from_u64(0x57e1_a17);
    let mut eastward_base = 0;
    for _ in 0..200 {
        let h = choose_direction(&ant, &grid, &cfg, &mut rng);
        if h.cos() > 0.0 { eastward_base += 1; }
    }

    assert!(
        eastward_high > eastward_base,
        "alpha_mult=5 should pull east more strongly than alpha_mult=1 with a weak trail (got {} vs {})",
        eastward_high, eastward_base,
    );
}

#[test]
fn exploration_mod_zero_preserves_exploration() {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    // With exploration_rate=1.0 and exploration_mod=0.0, every call should
    // return a uniform-random heading (the cone math is skipped).
    let grid = PheromoneGrid::new(10, 10);
    let ant = Ant::new_worker(1, 0, Vec2::new(5.0, 5.0), 0.0, 10.0);
    let mut cfg = AntConfig::default();
    cfg.exploration_rate = 1.0;
    let mut rng = ChaCha8Rng::seed_from_u64(0x6e0_77);

    let mut headings = Vec::new();
    for _ in 0..50 {
        headings.push(choose_direction(&ant, &grid, &cfg, &mut rng));
    }
    // Should be roughly uniform — variance large.
    let mean = headings.iter().sum::<f32>() / headings.len() as f32;
    let variance: f32 = headings.iter().map(|h| (h - mean).powi(2)).sum::<f32>() / headings.len() as f32;
    assert!(variance > 1.0, "uniform-random heading should have wide variance, got {}", variance);
}
```

- [ ] **Step 2: Run the tests to verify they fail (or partially pass) before edit**

Run: `cargo test -p antcolony-sim --lib ant::tests::default_modulators_reproduce_baseline_direction ant::tests::high_alpha_mult_strengthens_pheromone_following ant::tests::exploration_mod_zero_preserves_exploration`
Expected: `default_modulators_reproduce_baseline_direction` PASSES (current code is the baseline). `high_alpha_mult_strengthens_pheromone_following` FAILS because modulators aren't yet applied. `exploration_mod_zero_preserves_exploration` PASSES (no change needed yet).

- [ ] **Step 3: Wire modulators into `choose_direction`**

In `crates/antcolony-sim/src/ant.rs`, replace the body of `choose_direction` (lines 198-246). The signature stays the same — modulators come from the `ant: &Ant` parameter.

```rust
pub fn choose_direction(
    ant: &Ant,
    grid: &PheromoneGrid,
    cfg: &AntConfig,
    rng: &mut impl Rng,
) -> f32 {
    let mods = &ant.modulators;
    let alpha_eff = (cfg.alpha * mods.alpha_mult).clamp(0.1, 10.0);
    let beta_eff  = (cfg.beta  * mods.beta_mult ).clamp(0.1, 10.0);
    let explore_eff = (cfg.exploration_rate + mods.exploration_mod).clamp(0.0, 1.0);

    if rng.r#gen::<f32>() < explore_eff {
        return rng.gen_range(0.0..std::f32::consts::TAU);
    }

    let samples = grid.sample_cone(
        ant.position,
        ant.heading,
        cfg.sense_angle.to_radians(),
        cfg.sense_radius as f32,
        ant.target_layer(),
    );

    let mut weighted: Vec<(f32, f32)> = Vec::with_capacity(samples.len());
    let mut total = 0.0f32;
    for (cell, intensity) in &samples {
        let delta = *cell - ant.position;
        if delta.length_squared() < 1e-6 {
            continue;
        }
        let angle = delta.y.atan2(delta.x);
        let bias = (1.0 + (angle - ant.heading).cos()).max(0.01);
        let tau = intensity.max(0.0);
        let w = (tau + 0.01).powf(alpha_eff) * bias.powf(beta_eff);
        weighted.push((angle, w));
        total += w;
    }

    if total <= 1e-6 || weighted.is_empty() {
        let jitter = rng.gen_range(-0.6..0.6);
        return ant.heading + jitter;
    }

    let mut pick = rng.r#gen::<f32>() * total;
    for (angle, w) in &weighted {
        pick -= *w;
        if pick <= 0.0 {
            return *angle;
        }
    }
    weighted.last().map(|(a, _)| *a).unwrap_or(ant.heading)
}
```

The only changes from the original are: (a) the four `_eff` bindings at the top, (b) replacing `cfg.alpha` / `cfg.beta` / `cfg.exploration_rate` in the math with `alpha_eff` / `beta_eff` / `explore_eff`. The rest is byte-identical.

- [ ] **Step 4: Run all three tests + existing direction_biased_toward_pheromone**

Run: `cargo test -p antcolony-sim --lib ant::tests`
Expected: ALL PASS. The existing `direction_biased_toward_pheromone` test must still pass — this is the regression check.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/ant.rs
git commit -m "ant: choose_direction reads modulators (regression-safe at defaults)

alpha_eff = cfg.alpha * modulators.alpha_mult (clamped)
beta_eff  = cfg.beta  * modulators.beta_mult  (clamped)
explore_eff = cfg.exploration_rate + modulators.exploration_mod (clamped)

Default modulators (1.0, 1.0, 0.0, ...) reduce to the pre-plumbing math
bit-for-bit. Verified by direction_biased_toward_pheromone + new
default_modulators_reproduce_baseline_direction.
"
```

---

### Task 4: `PheromoneSnapshot` type + 32×32 downsample

**Files:**
- Modify: `crates/antcolony-sim/src/ai/observation.rs` (append `PheromoneSnapshot`)
- Modify: `crates/antcolony-sim/src/pheromone.rs` (add `downsample_to` helper)

- [ ] **Step 1: Write the failing tests**

Append to `crates/antcolony-sim/src/ai/observation.rs` (after the `AntModulators` block, before the `#[cfg(test)]` mod):

```rust
/// Snapshot of all four pheromone channels at a single tick. Used by
/// the commander tier's CNN-encoded spatial input. The trainer
/// downsamples this to a fixed 32×32 via `Simulation::pheromone_snapshot`
/// before feeding the policy net.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PheromoneSnapshot {
    pub width: u16,
    pub height: u16,
    /// Row-major, length = width*height. Indexed as [y * width + x].
    pub food_trail: Box<[f32]>,
    pub home_trail: Box<[f32]>,
    pub alarm: Box<[f32]>,
    pub colony_scent: Box<[f32]>,
}
```

Then in `crates/antcolony-sim/src/pheromone.rs`, add a test in its `#[cfg(test)]` mod:

```rust
#[test]
fn downsample_to_32x32_preserves_sum() {
    let mut grid = PheromoneGrid::new(64, 64);
    // Sprinkle some signal
    for y in 0..64 {
        for x in 0..64 {
            grid.deposit(x, y, PheromoneLayer::FoodTrail, (x as f32 + y as f32) * 0.01, 100.0);
        }
    }
    let down = grid.downsample_to(32, 32, PheromoneLayer::FoodTrail);
    assert_eq!(down.len(), 32 * 32);
    let full_sum: f32 = (0..64 * 64).map(|i| {
        let x = i % 64; let y = i / 64;
        grid.read(x, y, PheromoneLayer::FoodTrail)
    }).sum();
    let down_sum: f32 = down.iter().sum();
    // 2×2 average pooling — sum should equal full_sum / 4 (each input cell
    // contributes 1/4 to one output cell).
    let expected = full_sum / 4.0;
    let rel_err = (down_sum - expected).abs() / expected.max(1e-6);
    assert!(rel_err < 1e-3, "downsample sum {} should be ~{} (rel_err {})", down_sum, expected, rel_err);
}

#[test]
fn downsample_passthrough_same_size() {
    let mut grid = PheromoneGrid::new(32, 32);
    grid.deposit(5, 5, PheromoneLayer::FoodTrail, 7.0, 10.0);
    let down = grid.downsample_to(32, 32, PheromoneLayer::FoodTrail);
    assert_eq!(down.len(), 32 * 32);
    assert!((down[5 * 32 + 5] - 7.0).abs() < 1e-6);
}
```

(The two test helpers `PheromoneGrid::read(x, y, layer)` may not exist yet — if not, the implementer adds them or uses the existing layer-access method. Look in `pheromone.rs` for the current accessor pattern and use whatever's there.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p antcolony-sim --lib pheromone::tests::downsample_to_32x32_preserves_sum pheromone::tests::downsample_passthrough_same_size`
Expected: COMPILE ERROR — `no method 'downsample_to' on PheromoneGrid`.

- [ ] **Step 3: Implement `PheromoneGrid::downsample_to`**

In `crates/antcolony-sim/src/pheromone.rs`, add this method to the existing `impl PheromoneGrid` block:

```rust
/// Adaptive average-pool of the given layer down to `out_w × out_h`.
/// When `out_w == self.width && out_h == self.height` returns a clone.
/// Output is row-major, length = out_w * out_h.
///
/// Used by `Simulation::pheromone_snapshot` to give the commander
/// brain a fixed-size spatial input regardless of the arena resolution.
pub fn downsample_to(&self, out_w: u16, out_h: u16, layer: PheromoneLayer) -> Box<[f32]> {
    let in_w = self.width as usize;
    let in_h = self.height as usize;
    let out_w_us = out_w as usize;
    let out_h_us = out_h as usize;
    let layer_data = self.layer_slice(layer);

    if in_w == out_w_us && in_h == out_h_us {
        return layer_data.to_vec().into_boxed_slice();
    }

    let mut out = vec![0.0f32; out_w_us * out_h_us];
    // For each output cell, average the input cells that map into it.
    for oy in 0..out_h_us {
        let y_lo = (oy * in_h) / out_h_us;
        let y_hi = ((oy + 1) * in_h) / out_h_us;
        for ox in 0..out_w_us {
            let x_lo = (ox * in_w) / out_w_us;
            let x_hi = ((ox + 1) * in_w) / out_w_us;
            let mut sum = 0.0f32;
            let mut n = 0u32;
            for iy in y_lo..y_hi.max(y_lo + 1) {
                for ix in x_lo..x_hi.max(x_lo + 1) {
                    sum += layer_data[iy * in_w + ix];
                    n += 1;
                }
            }
            out[oy * out_w_us + ox] = if n > 0 { sum / n as f32 } else { 0.0 };
        }
    }
    out.into_boxed_slice()
}
```

This needs a `layer_slice(layer: PheromoneLayer) -> &[f32]` accessor. If it doesn't exist, add a private helper at the top of the same impl block:

```rust
fn layer_slice(&self, layer: PheromoneLayer) -> &[f32] {
    match layer {
        PheromoneLayer::FoodTrail   => &self.food_trail,
        PheromoneLayer::HomeTrail   => &self.home_trail,
        PheromoneLayer::Alarm       => &self.alarm,
        PheromoneLayer::ColonyScent => &self.colony_scent,
    }
}
```

(If `PheromoneGrid`'s field names differ, adjust — the implementer should look at the struct definition first.)

- [ ] **Step 4: Run the tests, expect PASS**

Run: `cargo test -p antcolony-sim --lib pheromone::tests::downsample_to_32x32_preserves_sum pheromone::tests::downsample_passthrough_same_size`
Expected: BOTH PASS.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/ai/observation.rs crates/antcolony-sim/src/pheromone.rs
git commit -m "pheromone: adaptive-avg-pool downsample_to + PheromoneSnapshot type

downsample_to(out_w, out_h, layer) lets the commander brain take a
fixed 32x32 spatial input regardless of arena size. Sum-preserving (up to
float roundoff). PheromoneSnapshot in ai/observation.rs is the
serializable carrier the trainer reads.
"
```

---

### Task 5: `HistoryToken` + `ColonyState` history ring

**Files:**
- Modify: `crates/antcolony-sim/src/ai/observation.rs` (append `HistoryToken`)
- Modify: `crates/antcolony-sim/src/colony.rs` (add two fields to `ColonyState`)

- [ ] **Step 1: Write the failing test**

Append to `crates/antcolony-sim/src/ai/observation.rs`:

```rust
/// One entry in the commander's history ring buffer (last 8 decision
/// cycles). The commander backbone consumes K=8 of these as 96-d tokens
/// alongside the state and pheromone inputs. Pad fields are unused by
/// Phase 1 — they're reserved for auxiliary features in later phases.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HistoryToken {
    pub state: [f32; 17],
    pub action: [f32; 6],
    pub reward: f32,
    pub pad: [f32; 72],
}

impl Default for HistoryToken {
    fn default() -> Self {
        Self {
            state: [0.0; 17],
            action: [0.0; 6],
            reward: 0.0,
            pad: [0.0; 72],
        }
    }
}

impl HistoryToken {
    /// Total float count when flattened — used as a shape check by Phase
    /// 2 trainer code. Must equal 17 + 6 + 1 + 72 = 96.
    pub const FLAT_LEN: usize = 96;
}
```

Add a test in the same file's `#[cfg(test)]` mod:

```rust
#[test]
fn history_token_flat_len_is_96() {
    assert_eq!(HistoryToken::FLAT_LEN, 96);
    let t = HistoryToken::default();
    let total = t.state.len() + t.action.len() + 1 + t.pad.len();
    assert_eq!(total, HistoryToken::FLAT_LEN);
}
```

Add a test to whichever `#[cfg(test)] mod tests` block exists in `crates/antcolony-sim/src/colony.rs`:

```rust
#[test]
fn colony_state_starts_with_empty_history_and_zero_intent() {
    let cs = ColonyState::default();
    assert_eq!(cs.commander_history.len(), 0);
    assert_eq!(cs.commander_intent, [0.0f32; 64]);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p antcolony-sim --lib ai::observation::tests::history_token_flat_len_is_96 colony::tests::colony_state_starts_with_empty_history_and_zero_intent`
Expected: first test PASSES (it's pure data), second test FAILS — `no field 'commander_history'`.

- [ ] **Step 3: Add fields to `ColonyState`**

In `crates/antcolony-sim/src/colony.rs`, locate the `ColonyState` struct (find it by `grep -n "pub struct ColonyState" crates/antcolony-sim/src/colony.rs` — the file is small enough to read whole). Add two new fields:

```rust
    /// Ring buffer of the last 8 commander decision tokens. Populated by
    /// `Simulation::apply_ai_decision` once the hierarchical trainer
    /// (Phase 2) is wired. Empty for runs without a trainer attached.
    #[serde(default)]
    pub commander_history: arrayvec::ArrayVec<crate::ai::observation::HistoryToken, 8>,
    /// 64-d intent vector broadcast by the commander tier to the ant
    /// tier. Zeros when no trainer is attached (ants treat zero intent
    /// as "no special bias", consistent with the identity-default
    /// AntModulators). Updated via `Simulation::apply_commander_intent`.
    #[serde(default = "default_commander_intent")]
    pub commander_intent: [f32; 64],
```

`[f32; 64]` doesn't have a serde-friendly `Default` impl. Add a free function above the struct:

```rust
fn default_commander_intent() -> [f32; 64] { [0.0; 64] }
```

If `ColonyState` has a manual `Default` impl, add `commander_history: ArrayVec::new(), commander_intent: [0.0; 64]` to its initializer. If it uses `#[derive(Default)]`, the per-field `#[serde(default = "...")]` plus the `Default` impl trickery isn't enough — instead, **manually implement `Default` for `ColonyState`**. The implementer should look at how the struct is currently constructed (Default-derived vs manual) and follow the existing pattern.

If `arrayvec` isn't yet a dependency of `antcolony-sim`, add it to `crates/antcolony-sim/Cargo.toml` under `[dependencies]`:

```toml
arrayvec = { version = "0.7", features = ["serde"] }
```

(Check first — it may already be there for other uses.)

- [ ] **Step 4: Run the tests, expect PASS**

Run: `cargo test -p antcolony-sim --lib ai::observation::tests::history_token_flat_len_is_96 colony::tests::colony_state_starts_with_empty_history_and_zero_intent`
Expected: BOTH PASS.

Then run the full crate to confirm nothing else broke:

Run: `cargo test -p antcolony-sim --lib`
Expected: ALL PRE-EXISTING TESTS PASS.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/ai/observation.rs crates/antcolony-sim/src/colony.rs crates/antcolony-sim/Cargo.toml
git commit -m "colony: HistoryToken + commander_history ring + commander_intent

8-deep ring buffer per colony, holds last 8 commander decisions for the
brain's history-conditioning input. commander_intent is the 64-d vector
broadcast to ants. Both zero/empty for runs without a trainer attached.
Snapshots from before the field bump load cleanly via serde(default).
"
```

---

### Task 6: `RichObservation` + `Simulation::colony_rich_observation`

**Files:**
- Modify: `crates/antcolony-sim/src/ai/observation.rs` (append `RichObservation`)
- Modify: `crates/antcolony-sim/src/simulation.rs` (add method)

- [ ] **Step 1: Write the failing test**

Append to `crates/antcolony-sim/src/ai/observation.rs`:

```rust
/// Bundle of everything the commander brain reads at decision time.
/// The `state` field is the existing `ColonyAiState`; the other two are
/// new (pheromone field as 32×32×4 tensor, last 8 commander tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichObservation {
    pub state: crate::ai::brain::ColonyAiState,
    pub pheromone_field: PheromoneSnapshot,
    pub history: arrayvec::ArrayVec<HistoryToken, 8>,
}
```

Add an integration test in a new file `crates/antcolony-sim/tests/phase1_plumbing.rs`:

```rust
//! Phase 1 plumbing integration tests — exercise the new Simulation API
//! end-to-end. These tests intentionally bypass the unit-test layer to
//! catch wiring mistakes (e.g. forgetting to populate a snapshot field).

use antcolony_sim::ai::observation::{RichObservation, AntObservation, AntModulators, HistoryToken};

#[test]
fn rich_observation_shape_for_default_match_env() {
    use antcolony_sim::{Simulation, Topology};
    use antcolony_sim::config::{SimConfig, WorldConfig, AntConfig, PheromoneConfig, ColonyConfig, CombatConfig, HazardConfig};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 10, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    let rich = sim.colony_rich_observation(0).expect("colony 0 exists");
    assert_eq!(rich.pheromone_field.width, 32);
    assert_eq!(rich.pheromone_field.height, 32);
    assert_eq!(rich.pheromone_field.food_trail.len(), 32 * 32);
    assert_eq!(rich.pheromone_field.home_trail.len(), 32 * 32);
    assert_eq!(rich.pheromone_field.alarm.len(), 32 * 32);
    assert_eq!(rich.pheromone_field.colony_scent.len(), 32 * 32);
    assert_eq!(rich.history.len(), 0); // fresh sim, no commander decisions yet
}

#[test]
fn rich_observation_returns_none_for_nonexistent_colony() {
    use antcolony_sim::{Simulation, Topology};
    use antcolony_sim::config::{SimConfig, WorldConfig, AntConfig, PheromoneConfig, ColonyConfig, CombatConfig, HazardConfig};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 10, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    assert!(sim.colony_rich_observation(99).is_none());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p antcolony-sim --test phase1_plumbing rich_observation_shape_for_default_match_env`
Expected: COMPILE ERROR — `no method 'colony_rich_observation' on Simulation`.

- [ ] **Step 3: Implement `Simulation::colony_rich_observation`**

In `crates/antcolony-sim/src/simulation.rs`, find the existing `colony_ai_state` method (grep for `pub fn colony_ai_state`). Add the new method nearby — same `impl Simulation` block:

```rust
/// Rich observation bundle for the hierarchical commander brain. Wraps
/// the existing `colony_ai_state` (17-d) with a downsampled pheromone
/// snapshot + the colony's commander history ring. Returns `None` when
/// `colony_id` does not exist.
///
/// Pheromone field is always downsampled to a fixed 32×32, regardless
/// of the actual arena size — the commander brain's CNN encoder expects
/// that shape.
pub fn colony_rich_observation(&self, colony_id: u8) -> Option<crate::ai::observation::RichObservation> {
    use crate::ai::observation::{RichObservation, PheromoneSnapshot};
    use crate::pheromone::PheromoneLayer;

    let state = self.colony_ai_state(colony_id)?;
    let colony = self.colonies.get(colony_id as usize)?;

    let pheromone_field = PheromoneSnapshot {
        width: 32,
        height: 32,
        food_trail:   self.pheromone.downsample_to(32, 32, PheromoneLayer::FoodTrail),
        home_trail:   self.pheromone.downsample_to(32, 32, PheromoneLayer::HomeTrail),
        alarm:        self.pheromone.downsample_to(32, 32, PheromoneLayer::Alarm),
        colony_scent: self.pheromone.downsample_to(32, 32, PheromoneLayer::ColonyScent),
    };

    Some(RichObservation {
        state,
        pheromone_field,
        history: colony.commander_history.clone(),
    })
}
```

The implementer should check whether `Simulation`'s pheromone field is called `pheromone` (singular) or `pheromone_grid` — adjust the field access. Same for `colonies` (might be `pub colonies: Vec<ColonyState>` or accessed via a method).

- [ ] **Step 4: Run the tests, expect PASS**

Run: `cargo test -p antcolony-sim --test phase1_plumbing`
Expected: BOTH integration tests PASS.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/ai/observation.rs crates/antcolony-sim/src/simulation.rs crates/antcolony-sim/tests/phase1_plumbing.rs
git commit -m "sim: colony_rich_observation method + RichObservation type

Bundles existing colony_ai_state with a 32x32 downsampled pheromone
snapshot and the per-colony commander history ring. Returns None for
nonexistent colony_id. Phase-1 commander brain input is now plumbed
end-to-end on the read side.
"
```

---

### Task 7: `AntObservation` + `Simulation::per_ant_observations`

**Files:**
- Modify: `crates/antcolony-sim/src/ai/observation.rs` (append `AntObservation`)
- Modify: `crates/antcolony-sim/src/simulation.rs` (add method)

- [ ] **Step 1: Write the failing test**

Append to `crates/antcolony-sim/src/ai/observation.rs`:

```rust
/// Per-ant local observation, one entry per adult ant in the colony.
/// Batched form so the trainer can stack into a single GPU tensor. The
/// commander's intent vector is NOT included here — the trainer reads
/// it once per decision window via the colony's `commander_intent`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AntObservation {
    pub ant_id: u32,
    /// 5 forward steps × 3 lateral cells × 4 pheromone channels = 60.
    /// Sampled along the ant's heading. Same cone geometry as
    /// `PheromoneGrid::sample_cone` uses for `choose_direction`.
    pub pheromone_cone: [f32; 60],
    /// food_carried, heading_sin, heading_cos, caste_onehot[3],
    /// state_timer_norm, age_norm. Exact layout fixed for trainer
    /// compatibility:
    ///   internal[0] = food_carried
    ///   internal[1] = heading.sin()
    ///   internal[2] = heading.cos()
    ///   internal[3] = caste_is_worker  (0.0 or 1.0)
    ///   internal[4] = caste_is_soldier
    ///   internal[5] = caste_is_breeder
    ///   internal[6] = (state_timer as f32 / 1000.0).clamp(0, 1)
    ///   internal[7] = (age as f32 / 10000.0).clamp(0, 1)
    pub internal: [f32; 8],
}
```

Add a test in `crates/antcolony-sim/tests/phase1_plumbing.rs`:

```rust
#[test]
fn per_ant_observations_count_matches_colony_population() {
    use antcolony_sim::{Simulation, Topology};
    use antcolony_sim::config::{SimConfig, WorldConfig, AntConfig, PheromoneConfig, ColonyConfig, CombatConfig, HazardConfig};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 7, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    let obs = sim.per_ant_observations(0);
    assert_eq!(obs.len(), 7, "should match initial_count=7");
    for o in &obs {
        // pheromone_cone has fixed 60-d shape
        assert_eq!(o.pheromone_cone.len(), 60);
        // internal[1]^2 + internal[2]^2 ≈ 1 (heading_sin² + heading_cos²)
        let h2 = o.internal[1] * o.internal[1] + o.internal[2] * o.internal[2];
        assert!((h2 - 1.0).abs() < 1e-4, "heading sin/cos should be unit, got {}", h2);
        // caste onehot sums to 1
        let caste_sum = o.internal[3] + o.internal[4] + o.internal[5];
        assert!((caste_sum - 1.0).abs() < 1e-4, "caste onehot should sum to 1, got {}", caste_sum);
    }
}

#[test]
fn per_ant_observations_empty_for_nonexistent_colony() {
    use antcolony_sim::{Simulation, Topology};
    use antcolony_sim::config::{SimConfig, WorldConfig, AntConfig, PheromoneConfig, ColonyConfig, CombatConfig, HazardConfig};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 5, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    let obs = sim.per_ant_observations(99);
    assert_eq!(obs.len(), 0);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p antcolony-sim --test phase1_plumbing per_ant_observations`
Expected: COMPILE ERROR — `no method 'per_ant_observations'`.

- [ ] **Step 3: Implement `Simulation::per_ant_observations`**

Add to the same `impl Simulation` block in `crates/antcolony-sim/src/simulation.rs`:

```rust
/// Collect a per-ant local observation for every adult ant in this
/// colony. Empty vec when the colony does not exist. The pheromone
/// cone is sampled along each ant's heading using the existing
/// `PheromoneGrid::sample_cone` (same geometry as choose_direction).
pub fn per_ant_observations(&self, colony_id: u8) -> Vec<crate::ai::observation::AntObservation> {
    use crate::ai::observation::AntObservation;
    use crate::ant::AntCaste;
    use crate::pheromone::PheromoneLayer;

    let Some(_colony) = self.colonies.get(colony_id as usize) else {
        return Vec::new();
    };

    // Iterate the global ant pool filtered by colony_id. Use whichever
    // accessor is canonical — if `Simulation::ants` is a Vec, filter by
    // colony_id and adult-only (skip eggs/larvae/pupae which aren't in
    // the Ant pool to begin with, but verify against the actual struct).
    let mut out = Vec::new();
    for ant in self.ants.iter().filter(|a| a.colony_id == colony_id) {
        let mut cone = [0.0f32; 60];
        // Sample each of the 4 pheromone layers along the ant's heading.
        // 5 forward steps × 3 lateral cells × 4 channels = 60 floats.
        // Layout: channel * 15 + step * 3 + lateral
        for (ch_idx, layer) in [
            PheromoneLayer::FoodTrail,
            PheromoneLayer::HomeTrail,
            PheromoneLayer::Alarm,
            PheromoneLayer::ColonyScent,
        ].iter().enumerate() {
            let samples = self.pheromone.sample_cone(
                ant.position,
                ant.heading,
                60.0_f32.to_radians(),
                5.0,
                *layer,
            );
            // Layout the (up to 15) samples deterministically into the slots.
            // If sample_cone returns fewer than 15, pad with zeros (already-init).
            for (i, (_pos, intensity)) in samples.iter().take(15).enumerate() {
                cone[ch_idx * 15 + i] = *intensity;
            }
        }

        let mut internal = [0.0f32; 8];
        internal[0] = ant.food_carried;
        internal[1] = ant.heading.sin();
        internal[2] = ant.heading.cos();
        internal[3] = if ant.caste == AntCaste::Worker  { 1.0 } else { 0.0 };
        internal[4] = if ant.caste == AntCaste::Soldier { 1.0 } else { 0.0 };
        internal[5] = if ant.caste == AntCaste::Breeder { 1.0 } else { 0.0 };
        internal[6] = (ant.state_timer as f32 / 1000.0).clamp(0.0, 1.0);
        internal[7] = (ant.age as f32 / 10000.0).clamp(0.0, 1.0);

        out.push(AntObservation {
            ant_id: ant.id,
            pheromone_cone: cone,
            internal,
        });
    }
    out
}
```

Field-name caveats again:
- `self.ants` might be `self.workers + self.soldiers + ...` or a single pool; check the Simulation struct.
- `AntCaste` variants: `Worker`, `Soldier`, `Queen`, `Breeder` per `ant.rs:38-43`. Queens never appear here (they don't have observations to score; the trainer doesn't control them).
- `60.0_f32.to_radians()` matches the literal `cfg.sense_angle.to_radians()` from `ant.rs:211` (default sense_angle=60). Hard-code 60 here — the per-ant obs is fixed-geometry; the AntConfig change wouldn't propagate to trained weights anyway.

Also: the caste onehot test fails for Queens because their flags are all zero. **Queens should be excluded** from `per_ant_observations` — they have no decisions to make. Add `.filter(|a| !matches!(a.caste, AntCaste::Queen))` to the iterator.

- [ ] **Step 4: Run the tests, expect PASS**

Run: `cargo test -p antcolony-sim --test phase1_plumbing per_ant_observations`
Expected: BOTH PASS.

Also rerun all tests as a regression check:

Run: `cargo test -p antcolony-sim`
Expected: ALL PASS.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/ai/observation.rs crates/antcolony-sim/src/simulation.rs crates/antcolony-sim/tests/phase1_plumbing.rs
git commit -m "sim: per_ant_observations method + AntObservation type

Per-ant 60-d pheromone cone (5 forward x 3 lateral x 4 channels) plus
an 8-d internal state vector. Queens excluded. Uses the same cone
geometry as choose_direction so the brain sees what the ant 'sees'.
"
```

---

### Task 8: `Simulation::apply_ant_modulators`

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/antcolony-sim/tests/phase1_plumbing.rs`:

```rust
#[test]
fn apply_ant_modulators_writes_through_to_pool() {
    use antcolony_sim::{Simulation, Topology};
    use antcolony_sim::config::{SimConfig, WorldConfig, AntConfig, PheromoneConfig, ColonyConfig, CombatConfig, HazardConfig};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 5, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    // Grab the first two ants of colony 0 by id.
    let obs = sim.per_ant_observations(0);
    assert!(obs.len() >= 2);
    let target_id_a = obs[0].ant_id;
    let target_id_b = obs[1].ant_id;

    sim.apply_ant_modulators(0, &[
        AntModulators { alpha_mult: 3.0, beta_mult: 0.5, exploration_mod: 0.05, deposit_mult: 2.0, state_bias: -1.0 },
    ], &[target_id_a]);

    // The targeted ant should have those modulators; the other should still be default.
    let ant_a = sim.ants.iter().find(|a| a.id == target_id_a).unwrap();
    assert_eq!(ant_a.modulators.alpha_mult, 3.0);
    let ant_b = sim.ants.iter().find(|a| a.id == target_id_b).unwrap();
    assert_eq!(ant_b.modulators, AntModulators::default());
}

#[test]
fn apply_ant_modulators_clamps_to_safe_ranges() {
    use antcolony_sim::{Simulation, Topology};
    use antcolony_sim::config::{SimConfig, WorldConfig, AntConfig, PheromoneConfig, ColonyConfig, CombatConfig, HazardConfig};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 3, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);
    let target = sim.per_ant_observations(0)[0].ant_id;

    sim.apply_ant_modulators(0, &[
        AntModulators { alpha_mult: 999.0, beta_mult: -10.0, exploration_mod: 5.0, deposit_mult: 1000.0, state_bias: 100.0 },
    ], &[target]);

    let ant = sim.ants.iter().find(|a| a.id == target).unwrap();
    assert!((0.1..=5.0).contains(&ant.modulators.alpha_mult));
    assert!((0.1..=5.0).contains(&ant.modulators.beta_mult));
    assert!((-0.1..=0.1).contains(&ant.modulators.exploration_mod));
    assert!((0.1..=5.0).contains(&ant.modulators.deposit_mult));
    assert!((-2.0..=2.0).contains(&ant.modulators.state_bias));
}

#[test]
fn apply_ant_modulators_unknown_id_is_noop() {
    use antcolony_sim::{Simulation, Topology};
    use antcolony_sim::config::{SimConfig, WorldConfig, AntConfig, PheromoneConfig, ColonyConfig, CombatConfig, HazardConfig};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 3, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    // ant id 0xFFFFFFFF doesn't exist — must not panic.
    sim.apply_ant_modulators(0, &[AntModulators::default()], &[0xFFFFFFFF]);
}
```

Note the signature change: `apply_ant_modulators(colony_id, mods, ant_ids)` — three args, parallel slices. The plan-doc spec at the top showed a single `&[AntModulators]` with embedded ant_id. The parallel-slices form is cleaner because trainer code typically has the ids and the modulators in separate tensors. The implementation should use parallel slices.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p antcolony-sim --test phase1_plumbing apply_ant_modulators`
Expected: COMPILE ERROR — `no method 'apply_ant_modulators'`.

- [ ] **Step 3: Implement `Simulation::apply_ant_modulators`**

Add to the same `impl Simulation` block:

```rust
/// Write per-ant modulators for the next decision window. `ant_ids[i]`
/// receives `mods[i]`. Slices must be the same length (asserted in
/// debug builds). Ants not in `ant_ids` keep their current modulators
/// (which default to identity).
///
/// Unknown ant ids (deceased, not in this colony, or never existed)
/// are silently skipped — `apply_ant_modulators` is on the per-tick
/// hot path during training and must not panic on stale id batches.
///
/// Each component is clamped to its safe range:
///   alpha_mult, beta_mult, deposit_mult ∈ [0.1, 5.0]
///   exploration_mod                     ∈ [-0.1, 0.1]
///   state_bias                          ∈ [-2.0, 2.0]
pub fn apply_ant_modulators(
    &mut self,
    colony_id: u8,
    mods: &[crate::ai::observation::AntModulators],
    ant_ids: &[u32],
) {
    debug_assert_eq!(
        mods.len(), ant_ids.len(),
        "apply_ant_modulators: mods and ant_ids must be same length"
    );
    if self.colonies.get(colony_id as usize).is_none() {
        return;
    }
    for (m, &id) in mods.iter().zip(ant_ids.iter()) {
        if let Some(ant) = self.ants.iter_mut().find(|a| a.id == id && a.colony_id == colony_id) {
            ant.modulators = crate::ai::observation::AntModulators {
                alpha_mult:      m.alpha_mult.clamp(0.1, 5.0),
                beta_mult:       m.beta_mult.clamp(0.1, 5.0),
                exploration_mod: m.exploration_mod.clamp(-0.1, 0.1),
                deposit_mult:    m.deposit_mult.clamp(0.1, 5.0),
                state_bias:      m.state_bias.clamp(-2.0, 2.0),
            };
        }
        // Unknown id → silent skip.
    }
}
```

The `.iter_mut().find(...)` is O(N) per id — for 10k ants and 10k modulator updates that's O(N²). For Phase 1 with N=10 ants per colony in the training arena that's fine. The performance optimization (build a HashMap<id, idx> at start of tick) is a Phase 2+ concern; don't preempt.

- [ ] **Step 4: Run the tests, expect PASS**

Run: `cargo test -p antcolony-sim --test phase1_plumbing apply_ant_modulators`
Expected: ALL THREE PASS.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/simulation.rs crates/antcolony-sim/tests/phase1_plumbing.rs
git commit -m "sim: apply_ant_modulators with safe-range clamps + silent-skip on unknown ids

Parallel slices: mods[i] applies to ant_ids[i]. Per-component clamps
match the safe ranges from the design spec. Unknown ids (stale batches,
deceased ants) are skipped without panic — hot-path tolerance.
"
```

---

### Task 9: `Simulation::apply_commander_intent`

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/antcolony-sim/tests/phase1_plumbing.rs`:

```rust
#[test]
fn apply_commander_intent_roundtrips_through_rich_observation() {
    use antcolony_sim::{Simulation, Topology};
    use antcolony_sim::config::{SimConfig, WorldConfig, AntConfig, PheromoneConfig, ColonyConfig, CombatConfig, HazardConfig};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 3, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    let mut intent = [0.0f32; 64];
    intent[3] = 1.5;
    intent[42] = -2.7;
    sim.apply_commander_intent(0, &intent);

    // Read back via the colony struct.
    let colony = sim.colonies.get(0).unwrap();
    assert_eq!(colony.commander_intent[3], 1.5);
    assert_eq!(colony.commander_intent[42], -2.7);
    assert_eq!(colony.commander_intent[0], 0.0);

    // Unknown colony — must not panic.
    sim.apply_commander_intent(99, &intent);
}
```

- [ ] **Step 2: Run the test, expect failure**

Run: `cargo test -p antcolony-sim --test phase1_plumbing apply_commander_intent_roundtrips_through_rich_observation`
Expected: COMPILE ERROR — `no method 'apply_commander_intent'`.

- [ ] **Step 3: Implement the method**

Add to the same `impl Simulation` block:

```rust
/// Store the commander's intent vector for this colony. Ants in the
/// colony will see this through their per-ant brain input (Phase 2
/// trainer reads `colony.commander_intent` once per decision window).
/// Unknown colony_id is a silent no-op.
pub fn apply_commander_intent(&mut self, colony_id: u8, intent: &[f32; 64]) {
    if let Some(colony) = self.colonies.get_mut(colony_id as usize) {
        colony.commander_intent = *intent;
    }
}
```

- [ ] **Step 4: Run the test, expect PASS**

Run: `cargo test -p antcolony-sim --test phase1_plumbing apply_commander_intent_roundtrips_through_rich_observation`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/simulation.rs crates/antcolony-sim/tests/phase1_plumbing.rs
git commit -m "sim: apply_commander_intent stores 64-d vector per colony

Silent no-op on unknown colony_id. Read back by Phase 2 trainer via
colony.commander_intent each ant-decision tick.
"
```

---

### Task 10: lib.rs re-exports for the public API surface

**Files:**
- Modify: `crates/antcolony-sim/src/lib.rs`
- Modify: `crates/antcolony-sim/src/ai/mod.rs`

- [ ] **Step 1: Add the test that needs the re-exports**

Add to `crates/antcolony-sim/tests/phase1_plumbing.rs` (at the top, replacing the per-test `use antcolony_sim::ai::observation::...` lines with the now-cleaner short paths):

```rust
#[test]
fn all_phase1_types_reexported_at_crate_root() {
    // This test compiles only if all five types are re-exported at the
    // crate root — the public API surface the trainer crate consumes.
    let _: antcolony_sim::AntModulators = antcolony_sim::AntModulators::default();
    let _: antcolony_sim::HistoryToken = antcolony_sim::HistoryToken::default();
    // PheromoneSnapshot, RichObservation, AntObservation are non-Default;
    // existence-check by type assignment only — wrapping in fn-ptr binding.
    fn _check_phsnap(_: &antcolony_sim::PheromoneSnapshot) {}
    fn _check_rich(_: &antcolony_sim::RichObservation) {}
    fn _check_antobs(_: &antcolony_sim::AntObservation) {}
}
```

- [ ] **Step 2: Run the test, expect failure**

Run: `cargo test -p antcolony-sim --test phase1_plumbing all_phase1_types_reexported_at_crate_root`
Expected: COMPILE ERROR — `no type 'AntModulators' in root module`.

- [ ] **Step 3: Add the re-exports**

In `crates/antcolony-sim/src/ai/mod.rs`, expand the observation re-export line (already added in Task 1 with just `AntModulators`):

```rust
pub use observation::{
    AntModulators,
    AntObservation,
    HistoryToken,
    PheromoneSnapshot,
    RichObservation,
};
```

In `crates/antcolony-sim/src/lib.rs`, the existing block at line 28-34 is `pub use ai::{...}`. Append the new types to it (multi-line if it isn't already):

```rust
pub use ai::{
    // ... existing entries (Arbiter, Blackboard, Brain types, etc.) ...
    AntModulators,
    AntObservation,
    HistoryToken,
    PheromoneSnapshot,
    RichObservation,
};
```

- [ ] **Step 4: Run the test, expect PASS**

Run: `cargo test -p antcolony-sim --test phase1_plumbing all_phase1_types_reexported_at_crate_root`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/ai/mod.rs crates/antcolony-sim/src/lib.rs
git commit -m "sim: re-export all 5 Phase-1 types at the crate root

antcolony_sim::{AntModulators, AntObservation, HistoryToken,
PheromoneSnapshot, RichObservation}. This is the public API surface the
Phase-2 trainer crate consumes.
"
```

---

### Task 11: Regression integration test — defaults reproduce baseline sim trajectories

**Files:**
- Modify: `crates/antcolony-sim/tests/phase1_plumbing.rs`

This task is the **headline regression check**. It exercises the full Phase 1 plumbing in default mode and asserts that population trajectories, food levels, and pheromone field intensities are byte-identical to a baseline run without the plumbing exercised. If this test passes, Phase 1 is regression-safe by construction.

- [ ] **Step 1: Write the test**

Add to `crates/antcolony-sim/tests/phase1_plumbing.rs`:

```rust
#[test]
fn defaults_reproduce_baseline_population_trajectory() {
    use antcolony_sim::{Simulation, Topology};
    use antcolony_sim::config::{SimConfig, WorldConfig, AntConfig, PheromoneConfig, ColonyConfig, CombatConfig, HazardConfig};

    fn run_sim(seed: u64, ticks: u64, exercise_plumbing: bool) -> Vec<(u32, u32, f32)> {
        let cfg = SimConfig {
            world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
            pheromone: PheromoneConfig::default(),
            ant: AntConfig { initial_count: 10, ..AntConfig::default() },
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        };
        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, seed, 0, 2);
        for t in 0..ticks {
            if exercise_plumbing && t % 5 == 0 {
                // Call all the new read-side methods every 5 ticks. Their
                // outputs are unused (the trainer does this in Phase 2).
                // If they have side-effects this loop catches them.
                let _ = sim.colony_rich_observation(0);
                let _ = sim.colony_rich_observation(1);
                let _ = sim.per_ant_observations(0);
                let _ = sim.per_ant_observations(1);
                // apply_ant_modulators with DEFAULTS is the identity
                // transform — must not perturb the sim.
                let obs0 = sim.per_ant_observations(0);
                let mods: Vec<_> = obs0.iter().map(|_| antcolony_sim::AntModulators::default()).collect();
                let ids: Vec<_> = obs0.iter().map(|o| o.ant_id).collect();
                sim.apply_ant_modulators(0, &mods, &ids);
                // apply_commander_intent with zeros is also identity.
                sim.apply_commander_intent(0, &[0.0; 64]);
                sim.apply_commander_intent(1, &[0.0; 64]);
            }
            sim.tick();
        }
        // Snapshot final population + food per colony.
        let mut snap = Vec::new();
        for cid in 0..2 {
            if let Some(c) = sim.colonies.get(cid as usize) {
                snap.push((
                    c.population.workers,
                    c.population.soldiers,
                    c.food_stored,
                ));
            }
        }
        snap
    }

    let baseline = run_sim(0xb45_e11e, 500, false);
    let with_plumbing = run_sim(0xb45_e11e, 500, true);

    assert_eq!(
        baseline, with_plumbing,
        "defaults reproduce baseline trajectory: read-side methods + default modulators + zero intent must be the identity. \
         baseline = {:?}, with_plumbing = {:?}", baseline, with_plumbing,
    );
}
```

- [ ] **Step 2: Run the test, expect it to PASS immediately**

Run: `cargo test -p antcolony-sim --test phase1_plumbing defaults_reproduce_baseline_population_trajectory`
Expected: PASS. If it FAILS, that's a bug — somewhere a Phase 1 method is mutating state when it shouldn't. Common culprits: (a) `colony_rich_observation` accidentally taking `&mut self`, (b) `per_ant_observations` advancing an RNG, (c) `apply_ant_modulators` writing through even for default values and there's a side-channel through the clamp. Find and fix.

- [ ] **Step 3: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/tests/phase1_plumbing.rs
git commit -m "test: defaults reproduce baseline sim trajectories (regression check)

If this passes, Phase 1's plumbing is identity-by-construction: calling
colony_rich_observation, per_ant_observations, apply_ant_modulators with
defaults, and apply_commander_intent with zeros every 5 ticks does not
perturb the sim's population/food trajectories vs the pre-plumbing
baseline at the same seed.

This is the headline gate for Phase 1 'done'.
"
```

---

### Task 12: End-to-end behavioral test — non-default modulators do change behavior

**Files:**
- Modify: `crates/antcolony-sim/tests/phase1_plumbing.rs`

The companion to Task 11. If defaults are the identity (Task 11) AND non-defaults change behavior (this task), the plumbing is wired correctly through the whole pipeline.

- [ ] **Step 1: Write the test**

Add to `crates/antcolony-sim/tests/phase1_plumbing.rs`:

```rust
#[test]
fn high_alpha_modulators_change_sim_trajectory() {
    use antcolony_sim::{Simulation, Topology};
    use antcolony_sim::config::{SimConfig, WorldConfig, AntConfig, PheromoneConfig, ColonyConfig, CombatConfig, HazardConfig};

    fn run_sim(seed: u64, ticks: u64, force_high_alpha: bool) -> u32 {
        let cfg = SimConfig {
            world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
            pheromone: PheromoneConfig::default(),
            ant: AntConfig { initial_count: 10, ..AntConfig::default() },
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        };
        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, seed, 0, 2);

        for t in 0..ticks {
            if force_high_alpha && t % 5 == 0 {
                // Set every ant in colony 0 to alpha_mult=5 (max pheromone-following).
                let obs0 = sim.per_ant_observations(0);
                let mods: Vec<_> = obs0.iter().map(|_| antcolony_sim::AntModulators {
                    alpha_mult: 5.0,
                    beta_mult: 1.0,
                    exploration_mod: -0.1, // max suppression of random exploration
                    deposit_mult: 1.0,
                    state_bias: 0.0,
                }).collect();
                let ids: Vec<_> = obs0.iter().map(|o| o.ant_id).collect();
                sim.apply_ant_modulators(0, &mods, &ids);
            }
            sim.tick();
        }
        sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0)
    }

    let baseline_workers = run_sim(0xbeef_ace, 1000, false);
    let high_alpha_workers = run_sim(0xbeef_ace, 1000, true);

    // Behavior MUST differ — high_alpha-driven ants follow trails more
    // tightly, so colony-1's food intake (and downstream worker count)
    // is materially different from baseline. The test asserts difference,
    // not direction — either side could be larger depending on emergent
    // dynamics, and that's fine.
    assert_ne!(
        baseline_workers, high_alpha_workers,
        "non-default modulators must change the sim trajectory (got identical worker counts {} = {})",
        baseline_workers, high_alpha_workers,
    );
}
```

- [ ] **Step 2: Run the test, expect PASS**

Run: `cargo test -p antcolony-sim --test phase1_plumbing high_alpha_modulators_change_sim_trajectory`
Expected: PASS. The two worker counts must differ. If they're identical, the modulators aren't actually flowing through to `choose_direction` — go back to Task 3 and verify the wiring.

- [ ] **Step 3: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/tests/phase1_plumbing.rs
git commit -m "test: non-default modulators change sim trajectory (end-to-end wiring)

Companion to defaults_reproduce_baseline_population_trajectory. Together
they prove Phase 1's plumbing is correct: defaults are identity,
non-defaults flow through to choose_direction and affect behavior.
"
```

---

### Task 13: Phase 1 acceptance — full crate test sweep + clippy

**Files:** none modified — verification only.

- [ ] **Step 1: Run the full crate test suite**

Run: `cargo test -p antcolony-sim`
Expected: ALL PASS, including the existing 154 lib tests (per the project HANDOFF memo) plus the new phase1_plumbing integration tests.

- [ ] **Step 2: Run clippy on the changed crate**

Run: `cargo clippy -p antcolony-sim --lib --tests -- -D warnings`
Expected: NO warnings. Fix any that surface; common ones: unused imports in the test file, `expect` vs `unwrap` style, `Vec::new()` vs `vec![]`.

- [ ] **Step 3: Confirm `antcolony-trainer` still builds against the modified sim**

Run: `cargo build -p antcolony-trainer`
Expected: BUILDS clean. The trainer doesn't yet consume the new types — Phase 2's job — so the existing `ActorCritic` / `PpoTrainer` should not be touched by Phase 1. If the trainer fails to build, there's an accidental coupling that needs fixing.

- [ ] **Step 4: Confirm the full workspace still builds**

Run: `cargo build --workspace`
Expected: BUILDS clean across all four crates (sim, game, render, trainer).

- [ ] **Step 5: Update HANDOFF.md with Phase 1 completion**

Append a session entry to `J:/antcolony/HANDOFF.md` (at the top, above the existing 2026-05-18 entry):

```markdown
## Session [YYYY-MM-DD] — Phase 1 ant-brain sim plumbing landed

🟢 Project Status: Phase 1 ship-ready. All 5 new types + 4 new Simulation
methods + ACO modulator wiring complete. Defaults reproduce baseline
sim trajectories bit-for-bit (verified by
`defaults_reproduce_baseline_population_trajectory`). Non-defaults
change behavior end-to-end (verified by
`high_alpha_modulators_change_sim_trajectory`). Full workspace builds,
all tests pass.

Next: Phase 2 — trainer-side hierarchical policy net. Plan to be
written via superpowers:writing-plans from
`docs/superpowers/specs/2026-05-18-ant-brain-hierarchical-design.md`.
```

- [ ] **Step 6: Commit the HANDOFF update**

```bash
cd J:/antcolony
git add HANDOFF.md
git commit -m "handoff: phase 1 ant-brain sim plumbing complete

5 new types, 4 new Simulation methods, ACO modulator wiring.
Regression test passes (defaults are identity); non-defaults change
behavior end-to-end. Next: phase 2 trainer.
"
```

---

## Acceptance criteria (recap)

Phase 1 is **done** when all of the following are true:

1. `cargo test -p antcolony-sim` passes — all pre-existing tests plus the new `phase1_plumbing` integration suite.
2. `cargo clippy -p antcolony-sim --lib --tests -- -D warnings` is clean.
3. `cargo build --workspace` builds clean across all four crates.
4. `defaults_reproduce_baseline_population_trajectory` passes — proves regression safety.
5. `high_alpha_modulators_change_sim_trajectory` passes — proves wiring works end-to-end.
6. `crates/antcolony-trainer/` is **unchanged** — Phase 1 does not touch the trainer crate.

If any acceptance criterion fails, the corresponding task gets reopened. The plan does NOT proceed to Phase 2 until all six are green.

---

## Out-of-scope for Phase 1 (deferred to later phases)

- The `state_bias` modulator field flows through `apply_ant_modulators` and is clamped, but it isn't yet read anywhere in the sim. Wiring it into the FSM transition logic (the specific transition site mentioned in the design spec's "open questions") is a Phase 2 prerequisite when the trainer actually starts using it.
- The `deposit_mult` modulator similarly stored but not yet applied — Phase 2 deferred. The implementer of this plan should NOT wire those two fields in Phase 1; that scope creep would muddle the regression-safety guarantee. Leave them stored-but-unused for now.
- No new sim-side benchmarks. Phase 1 is plumbing; perf characterization is Phase 4 ahead of the cnc training run.
- No changes to `antcolony-game` or `antcolony-render` — they don't consume the new types.

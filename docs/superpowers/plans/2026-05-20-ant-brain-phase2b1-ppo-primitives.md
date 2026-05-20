# Hierarchical Ant Brain — Phase 2b-1: PPO Primitives + `state_bias` Sim Wiring

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the trainer-side PPO primitives — stochastic sampling + log-prob recomputation on `HierarchicalActorCritic`, batched `MatchEnv` accessors that pack two colonies' observations into single tensors, the per-ant `state_bias` modulator wired into the FSM, and DRY-extracted observation→tensor helpers. After 2b-1 ships, Phase 2b-2 can implement `JointPpoTrainer` on top without any sim-side or API-shape work.

**Architecture:** Three layers of plumbing. Sim layer: one new modulator field gets read inside `Exploring → FollowingTrail` (or whichever single FSM transition site the implementer identifies). Trainer plumbing layer: tensor conversion moves from a private test-fixture helper into a `pub(crate)` shared module. Trainer policy layer: `HierarchicalActorCritic` gains `sample_commander`/`sample_ant` (rollout-time stochastic actions with log-prob) and `log_prob_of_commander_action`/`log_prob_of_ant_modulator` (PPO importance-ratio recomputation). `MatchEnv` gets three batch accessors that wrap the Phase-1 sim API into trainer-shaped tensors.

**Tech Stack:** Rust 2024, candle-core + candle-nn (already deps), `rand_chacha::ChaCha8Rng` (already used in existing `ActorCritic::sample` — same pattern). No new crates.

**Predecessor:** `docs/superpowers/plans/2026-05-20-ant-brain-phase2a-policy-nets.md` (shipped on main at `0110551`).
**Spec:** `docs/superpowers/specs/2026-05-18-ant-brain-hierarchical-design.md` (sections "Joint PPO loss" + "Per-tick data flow").
**Successor:** Phase 2b-2 — `JointPpoTrainer` struct + two-buffer rollout + GAE per tier + joint loss + Adam update + `examples/joint_ppo_smoke.rs` 5-iter smoke run on kokonoe.

---

## File map

| File | Status | Responsibility |
|---|---|---|
| `crates/antcolony-sim/src/ant.rs` OR `src/simulation.rs` (TBD by impl) | MODIFY | Wire `ant.modulators.state_bias` into ONE specific FSM transition decision (likely `Exploring → FollowingTrail`); test changes its rate |
| `crates/antcolony-sim/tests/phase1_plumbing.rs` | MODIFY | New behavioral test: `state_bias_shifts_following_trail_transition_rate` |
| `crates/antcolony-trainer/src/hierarchical/obs_to_tensors.rs` | **CREATE** | `pub(crate)` helpers: `rich_to_tensors(rich, device, batch_size=1)` and `ant_obs_to_tensors(obs_slice, intent_per_ant, device)` |
| `crates/antcolony-trainer/src/hierarchical/mod.rs` | MODIFY | `pub mod obs_to_tensors;` |
| `crates/antcolony-trainer/tests/hierarchical_smoke.rs` | MODIFY | Replace inline helpers with calls to the new shared module |
| `crates/antcolony-trainer/src/env.rs` | MODIFY | Add `commander_obs_batch`, `all_ant_obs_batch`, `apply_commander_intents`, `apply_ant_modulators_batched` methods to `MatchEnv` |
| `crates/antcolony-trainer/src/hierarchical/actor_critic.rs` | MODIFY | Add `sample_commander`, `sample_ant`, `log_prob_of_commander_action`, `log_prob_of_ant_modulator` |
| `crates/antcolony-trainer/tests/hierarchical_sampling.rs` | **CREATE** | Integration tests for the 4 new HAC methods — shapes, finiteness, log-prob round-trip |
| `J:/antcolony/HANDOFF.md` | MODIFY | Phase 2b-1 session entry |

**File-size discipline:** `actor_critic.rs` grows from ~80 lines to ~250 lines after Tasks 7–10. Acceptable. `env.rs` grows by ~150 lines after Tasks 4–6. Acceptable.

---

### Task 1: Locate the FSM transition site + add the failing `state_bias` behavioral test

**Files:**
- Modify: `crates/antcolony-sim/tests/phase1_plumbing.rs` (add test)

This task is intentionally small — locate the site, write the failing test, commit. Task 2 wires the actual bias.

- [ ] **Step 1: Locate the FSM transition site**

The per-ant `state_bias` modulator field has been stored-and-clamped since Phase 1 Task 8 but no sim code reads it. We need to inject it into ONE specific FSM transition. Per the design spec, the natural site is **`Exploring → FollowingTrail`** — an ant decides whether the pheromone gradient is strong enough to commit to following it.

Run these to find candidate sites:

```bash
cd J:/antcolony
grep -n 'FollowingTrail' crates/antcolony-sim/src/ant.rs crates/antcolony-sim/src/simulation.rs | head -20
grep -n 'AntState::Exploring' crates/antcolony-sim/src/simulation.rs | head -20
```

Look for code that:
1. Reads pheromone intensity at the ant's position (`grid.sample_cone` or `grid.read(...)`)
2. Compares to a threshold OR computes a transition probability
3. Calls `ant.transition(AntState::FollowingTrail)` (or sets `ant.state` directly)

If multiple sites exist, pick the **one** that controls the bulk of `Exploring → FollowingTrail` transitions. Document the file:line in your commit message.

**If no clear single site exists** (e.g., transitions to `FollowingTrail` happen as implicit fallthroughs in `choose_direction`'s direction-selection logic), the simplest fix is to add a NEW explicit transition check inside the existing per-ant tick — sensible if the codebase doesn't already centralize the logic.

- [ ] **Step 2: Write the failing behavioral test**

Append to `crates/antcolony-sim/tests/phase1_plumbing.rs`:

```rust
#[test]
fn state_bias_shifts_following_trail_transition_rate() {
    use antcolony_sim::ai::observation::AntModulators;
    use antcolony_sim::ant::AntState;
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::Simulation;

    fn run_sim(seed: u64, ticks: u64, state_bias: f32) -> u32 {
        let cfg = SimConfig {
            world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
            pheromone: PheromoneConfig::default(),
            ant: AntConfig { initial_count: 20, ..AntConfig::default() },
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        };
        let mut sim = Simulation::new(cfg, seed);
        // Deposit a strong food trail so ants have something to follow.
        sim.spawn_food_cluster();
        // Apply state_bias to every ant on every decision tick.
        for t in 0..ticks {
            if t % 5 == 0 {
                let obs0 = sim.per_ant_observations(0);
                let mods: Vec<_> = obs0.iter().map(|_| AntModulators {
                    alpha_mult: 1.0,
                    beta_mult: 1.0,
                    exploration_mod: 0.0,
                    deposit_mult: 1.0,
                    state_bias,
                }).collect();
                let ids: Vec<_> = obs0.iter().map(|o| o.ant_id).collect();
                sim.apply_ant_modulators(0, &mods, &ids);
            }
            sim.tick();
        }
        // Count ants currently in AntState::FollowingTrail at the end of the run.
        sim.ants.iter()
            .filter(|a| a.colony_id == 0 && a.state == AntState::FollowingTrail)
            .count() as u32
    }

    let baseline = run_sim(0xb1a5_e1, 300, 0.0);
    let positive = run_sim(0xb1a5_e1, 300, 2.0);  // max positive bias
    let negative = run_sim(0xb1a5_e1, 300, -2.0); // max negative bias

    // Positive state_bias should make ants MORE likely to follow trails.
    // Negative should make them LESS likely. Compared to baseline:
    assert!(
        positive >= baseline,
        "positive state_bias should not reduce FollowingTrail count (got positive={}, baseline={})",
        positive, baseline,
    );
    assert!(
        negative <= baseline,
        "negative state_bias should not increase FollowingTrail count (got negative={}, baseline={})",
        negative, baseline,
    );
    // At least one direction must differ from baseline; both being equal means state_bias is being ignored.
    assert!(
        positive != baseline || negative != baseline,
        "state_bias has no effect — neither positive ({}) nor negative ({}) bias differs from baseline ({})",
        positive, negative, baseline,
    );
}
```

If `Simulation::spawn_food_cluster()` doesn't exist as named, grep for it (`grep -n 'fn spawn_food\|fn drop_food\|fn place_food_cluster' crates/antcolony-sim/src/simulation.rs`) and use the actual method name.

- [ ] **Step 3: Run the test, expect failure**

Run: `cd J:/antcolony && cargo test -p antcolony-sim --test phase1_plumbing state_bias_shifts_following_trail_transition_rate 2>&1 | tail -10`

Expected: FAIL on the third assertion (`positive != baseline || negative != baseline`) — currently `state_bias` is ignored, so all three runs produce identical FollowingTrail counts.

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/tests/phase1_plumbing.rs
git commit -m "test: failing test for state_bias modulator (pins Phase 2b-1 contract)

Asserts non-default state_bias shifts the count of ants in
AntState::FollowingTrail at the end of a 300-tick run vs baseline. Will
pass once Task 2 wires state_bias into the Exploring -> FollowingTrail
transition site.
"
```

---

### Task 2: Wire `state_bias` into the FSM transition site

**Files:**
- Modify: `crates/antcolony-sim/src/ant.rs` OR `crates/antcolony-sim/src/simulation.rs` (whichever owns the transition site you found in Task 1)

- [ ] **Step 1: Inject `state_bias` at the transition site**

At the site you found, locate the transition probability/threshold check. Three common patterns and how to inject:

**Pattern A — boolean threshold check:**
```rust
// BEFORE:
if pheromone_intensity > FOLLOW_THRESHOLD {
    ant.transition(AntState::FollowingTrail);
}
// AFTER (state_bias shifts the effective threshold):
// Positive state_bias LOWERS the threshold (easier to enter FollowingTrail).
// Negative state_bias RAISES it.
let effective_threshold = FOLLOW_THRESHOLD * (1.0 - 0.4 * ant.modulators.state_bias.clamp(-2.0, 2.0));
if pheromone_intensity > effective_threshold {
    ant.transition(AntState::FollowingTrail);
}
```

**Pattern B — probabilistic transition:**
```rust
// BEFORE:
let p_follow = compute_follow_probability(...);
if rng.r#gen::<f32>() < p_follow {
    ant.transition(AntState::FollowingTrail);
}
// AFTER (state_bias as additive logit):
let logit = (p_follow / (1.0 - p_follow + 1e-6)).ln();  // inverse sigmoid
let biased_logit = logit + ant.modulators.state_bias.clamp(-2.0, 2.0);
let p_follow_biased = 1.0 / (1.0 + (-biased_logit).exp());  // sigmoid
if rng.r#gen::<f32>() < p_follow_biased {
    ant.transition(AntState::FollowingTrail);
}
```

**Pattern C — no explicit transition, FollowingTrail is entered implicitly via heading-toward-pheromone:**
If `Exploring → FollowingTrail` isn't a discrete decision (the ant just heads toward the strongest pheromone and "is following" emergently), add an explicit check at the top of the per-ant per-tick logic:

```rust
// Phase 2b-1: explicit FollowingTrail transition gated by state_bias.
if ant.state == AntState::Exploring {
    let local_intensity = grid.read(
        ant.position.x as usize,
        ant.position.y as usize,
        ant.target_layer(),
    );
    let threshold = 0.5_f32 * (1.0 - 0.4 * ant.modulators.state_bias.clamp(-2.0, 2.0));
    if local_intensity > threshold {
        ant.transition(AntState::FollowingTrail);
    }
}
```

**Pick the pattern that fits your site.** The exact math doesn't matter (it'll be retuned in Phase 4 with real data); the load-bearing property is **non-default state_bias measurably changes the FollowingTrail transition rate** as the Task 1 test asserts.

- [ ] **Step 2: Run the test, expect PASS**

Run: `cd J:/antcolony && cargo test -p antcolony-sim --test phase1_plumbing state_bias_shifts_following_trail_transition_rate 2>&1 | tail -10`
Expected: PASS.

Run the regression check:

Run: `cargo test -p antcolony-sim --test phase1_plumbing defaults_reproduce_baseline_population_trajectory 2>&1 | tail -5`
Expected: PASS (default `state_bias = 0.0` must preserve baseline behavior bit-for-bit).

Run the deposit_mult regression too:

Run: `cargo test -p antcolony-sim --test phase1_plumbing deposit_mult_strengthens_pheromone_deposition 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/ant.rs crates/antcolony-sim/src/simulation.rs
git commit -m "sim: wire state_bias modulator into Exploring -> FollowingTrail transition

The state_bias field on AntModulators has been clamped-and-stored
since Phase 1 Task 8 but no FSM transition was reading it. Injected
into <FILE:LINE — fill in> as a threshold/logit bias. Default 0.0 is
the identity (verified by defaults_reproduce_baseline_population_
trajectory still passing); non-default values measurably shift the
FollowingTrail transition rate (verified by new
state_bias_shifts_following_trail_transition_rate test).

Refs: docs/superpowers/plans/2026-05-20-ant-brain-phase2b1-ppo-primitives.md
"
```

Replace `<FILE:LINE — fill in>` with the actual site you wired (e.g., `simulation.rs:1832` or `ant.rs:215`).

---

### Task 3: DRY tensor conversion — extract `obs_to_tensors` helper module

**Files:**
- Create: `crates/antcolony-trainer/src/hierarchical/obs_to_tensors.rs`
- Modify: `crates/antcolony-trainer/src/hierarchical/mod.rs`
- Modify: `crates/antcolony-trainer/tests/hierarchical_smoke.rs`

- [ ] **Step 1: Create the helper module**

Create `crates/antcolony-trainer/src/hierarchical/obs_to_tensors.rs`:

```rust
//! Observation → tensor conversion helpers shared between the trainer
//! and integration tests. Phase 2a inlined these in
//! `tests/hierarchical_smoke.rs`; Phase 2b extracted them so the
//! `JointPpoTrainer` rollout code can call them too.
//!
//! Layouts are pinned to the `Sizing` `FIXED_*` constants — if the
//! sim's `ColonyAiState` / `AntObservation` shapes ever change, the
//! `fixed_dims_match_phase1_sim_api` test in `sizing.rs` trips first.

use candle_core::{Device, Result, Tensor};

use antcolony_sim::ai::observation::{AntObservation, HistoryToken, RichObservation};

use crate::hierarchical::sizing::{
    FIXED_CONE_D, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D, FIXED_INTENT_D, FIXED_INTERNAL_D,
    FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W, FIXED_STATE_D,
};

/// Convert one [`RichObservation`] to (state, pheromone, history) tensors
/// with a leading batch dim of 1. The trainer batches multiple of these
/// by stacking the results (or use [`rich_batch_to_tensors`] for a
/// pre-batched form when you have multiple colonies in hand).
pub(crate) fn rich_to_tensors(
    rich: &RichObservation,
    device: &Device,
) -> Result<(Tensor, Tensor, Tensor)> {
    let state_v = state_flatten(rich);
    debug_assert_eq!(state_v.len(), FIXED_STATE_D);
    let state = Tensor::from_vec(state_v, (1, FIXED_STATE_D), device)?;

    let pher_v = pheromone_flatten(rich);
    let pheromone = Tensor::from_vec(
        pher_v,
        (1, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W),
        device,
    )?;

    let hist_v = history_flatten(rich);
    let history = Tensor::from_vec(hist_v, (1, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D), device)?;

    Ok((state, pheromone, history))
}

/// Batched form: stack N `RichObservation`s into a `(N, ...)` tensor triplet.
/// Used by the trainer for multi-colony matches and population-of-N rollouts.
pub(crate) fn rich_batch_to_tensors(
    riches: &[&RichObservation],
    device: &Device,
) -> Result<(Tensor, Tensor, Tensor)> {
    let n = riches.len();
    let mut state_v = Vec::with_capacity(n * FIXED_STATE_D);
    let mut pher_v = Vec::with_capacity(n * FIXED_PHEROMONE_C * FIXED_PHEROMONE_H * FIXED_PHEROMONE_W);
    let mut hist_v = Vec::with_capacity(n * FIXED_HISTORY_K * FIXED_HISTORY_TOK_D);
    for r in riches {
        state_v.extend_from_slice(&state_flatten(r));
        pher_v.extend_from_slice(&pheromone_flatten(r));
        hist_v.extend_from_slice(&history_flatten(r));
    }
    let state = Tensor::from_vec(state_v, (n, FIXED_STATE_D), device)?;
    let pheromone = Tensor::from_vec(
        pher_v,
        (n, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W),
        device,
    )?;
    let history = Tensor::from_vec(hist_v, (n, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D), device)?;
    Ok((state, pheromone, history))
}

/// Convert a slice of [`AntObservation`]s to batched `(cone, internal, intent)`
/// tensors. The intent tensor is broadcast from a `(1, FIXED_INTENT_D)` input
/// to `(N, FIXED_INTENT_D)` since all ants in a colony share the commander's
/// intent for the current decision window.
pub(crate) fn ant_obs_to_tensors(
    obs: &[AntObservation],
    intent_per_colony: &Tensor,
    device: &Device,
) -> Result<(Tensor, Tensor, Tensor)> {
    let b = obs.len();
    let mut cone_v = Vec::with_capacity(b * FIXED_CONE_D);
    let mut internal_v = Vec::with_capacity(b * FIXED_INTERNAL_D);
    for o in obs {
        cone_v.extend_from_slice(&o.pheromone_cone);
        internal_v.extend_from_slice(&o.internal);
    }
    let cone = Tensor::from_vec(cone_v, (b, FIXED_CONE_D), device)?;
    let internal = Tensor::from_vec(internal_v, (b, FIXED_INTERNAL_D), device)?;
    let intent = intent_per_colony.broadcast_as((b, FIXED_INTENT_D))?;
    Ok((cone, internal, intent))
}

// ───── private flatten helpers ─────

fn state_flatten(rich: &RichObservation) -> Vec<f32> {
    let s = &rich.state;
    let ed = if s.enemy_distance_min.is_finite() { s.enemy_distance_min } else { 1e6 };
    vec![
        s.food_stored, s.food_inflow_recent,
        s.worker_count as f32, s.soldier_count as f32, s.breeder_count as f32,
        s.brood_egg as f32, s.brood_larva as f32, s.brood_pupa as f32,
        s.queens_alive as f32, s.combat_losses_recent as f32,
        ed, s.enemy_worker_count as f32, s.enemy_soldier_count as f32,
        s.day_of_year as f32, s.ambient_temp_c,
        if s.diapause_active { 1.0 } else { 0.0 },
        if s.is_daytime { 1.0 } else { 0.0 },
    ]
}

fn pheromone_flatten(rich: &RichObservation) -> Vec<f32> {
    let p = &rich.pheromone_field;
    let mut v = Vec::with_capacity(FIXED_PHEROMONE_C * FIXED_PHEROMONE_H * FIXED_PHEROMONE_W);
    v.extend_from_slice(&p.food_trail);
    v.extend_from_slice(&p.home_trail);
    v.extend_from_slice(&p.alarm);
    v.extend_from_slice(&p.colony_scent);
    v
}

fn history_flatten(rich: &RichObservation) -> Vec<f32> {
    let mut v = Vec::with_capacity(FIXED_HISTORY_K * FIXED_HISTORY_TOK_D);
    for tok in rich.history.iter() {
        v.extend_from_slice(&tok.state);
        v.extend_from_slice(&tok.action);
        v.push(tok.reward);
        v.extend_from_slice(&tok.pad);
    }
    // Pad to full K tokens (rich.history is an ArrayVec that may have fewer entries).
    while v.len() < FIXED_HISTORY_K * FIXED_HISTORY_TOK_D {
        v.push(0.0);
    }
    let _ = HistoryToken::FLAT_LEN; // shape-check compile-time reference; keeps the use line alive
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    fn build_sim() -> Simulation {
        let cfg = SimConfig {
            world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
            pheromone: PheromoneConfig::default(),
            ant: AntConfig { initial_count: 10, ..AntConfig::default() },
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        };
        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2)
    }

    #[test]
    fn rich_to_tensors_shapes() {
        let device = Device::Cpu;
        let sim = build_sim();
        let rich = sim.colony_rich_observation(0).unwrap();
        let (s, p, h) = rich_to_tensors(&rich, &device).unwrap();
        assert_eq!(s.dims(), &[1, FIXED_STATE_D]);
        assert_eq!(p.dims(), &[1, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W]);
        assert_eq!(h.dims(), &[1, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D]);
    }

    #[test]
    fn rich_batch_to_tensors_stacks_two_colonies() {
        let device = Device::Cpu;
        let sim = build_sim();
        let rich0 = sim.colony_rich_observation(0).unwrap();
        let rich1 = sim.colony_rich_observation(1).unwrap();
        let (s, p, h) = rich_batch_to_tensors(&[&rich0, &rich1], &device).unwrap();
        assert_eq!(s.dims(), &[2, FIXED_STATE_D]);
        assert_eq!(p.dims(), &[2, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W]);
        assert_eq!(h.dims(), &[2, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D]);
    }

    #[test]
    fn ant_obs_to_tensors_broadcasts_intent() {
        let device = Device::Cpu;
        let sim = build_sim();
        let obs = sim.per_ant_observations(0);
        let intent_per_colony = Tensor::randn(0.0f32, 1.0, (1, FIXED_INTENT_D), &device).unwrap();
        let (c, i, intent_b) = ant_obs_to_tensors(&obs, &intent_per_colony, &device).unwrap();
        let n = obs.len();
        assert_eq!(c.dims(), &[n, FIXED_CONE_D]);
        assert_eq!(i.dims(), &[n, FIXED_INTERNAL_D]);
        assert_eq!(intent_b.dims(), &[n, FIXED_INTENT_D]);
    }
}
```

- [ ] **Step 2: Wire the module into mod.rs**

In `crates/antcolony-trainer/src/hierarchical/mod.rs`, add `pub mod obs_to_tensors;` to the existing module declarations. **Do not add to the `pub use` block** — the helpers are `pub(crate)` and only consumed inside the trainer crate.

- [ ] **Step 3: Run unit tests, expect PASS**

Run: `cd J:/antcolony && cargo test -p antcolony-trainer --lib hierarchical::obs_to_tensors::tests 2>&1 | tail -10`
Expected: 3 tests pass.

- [ ] **Step 4: Refactor the smoke test to use the shared helpers**

In `crates/antcolony-trainer/tests/hierarchical_smoke.rs`, delete the inline `rich_to_tensors` and `ant_obs_to_tensors` helpers. Replace the test body's calls to them with calls to the new module. The test file's imports change from:

```rust
use antcolony_sim::ai::observation::{AntObservation, RichObservation};
// ... other imports
use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::HierarchicalActorCritic;
```

to:

```rust
use antcolony_trainer::hierarchical::obs_to_tensors::{rich_to_tensors, ant_obs_to_tensors};
use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::HierarchicalActorCritic;
```

But `obs_to_tensors` is `pub(crate)` — INTEGRATION tests in `tests/` see the crate from OUTSIDE. So you have two options:

**Option A — promote to `pub`:** change `obs_to_tensors`'s items from `pub(crate)` to `pub` and re-export them from `lib.rs`. Note that this exposes them as part of the trainer's public API, which is a real commitment.

**Option B — duplicate in the test:** leave `obs_to_tensors` as `pub(crate)`, keep the integration test's inline helpers but mark them with a comment pointing at the canonical impl. Production code (Phase 2b-2's JointPpoTrainer) calls the `pub(crate)` version; tests call the inline copies.

**Option A is correct.** The shared helpers ARE the public API for "convert sim obs to trainer tensors" — making them `pub` makes that intent explicit. Change the visibility:

In `obs_to_tensors.rs`, replace every `pub(crate) fn` with `pub fn`.

In `crates/antcolony-trainer/src/lib.rs`, add:

```rust
pub use hierarchical::obs_to_tensors::{rich_to_tensors, ant_obs_to_tensors, rich_batch_to_tensors};
```

- [ ] **Step 5: Run the smoke test, expect PASS**

Run: `cd J:/antcolony && cargo test -p antcolony-trainer --test hierarchical_smoke 2>&1 | tail -10`
Expected: PASS (the test still validates the same end-to-end behavior, just via the shared helpers now).

- [ ] **Step 6: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/obs_to_tensors.rs \
        crates/antcolony-trainer/src/hierarchical/mod.rs \
        crates/antcolony-trainer/src/lib.rs \
        crates/antcolony-trainer/tests/hierarchical_smoke.rs
git commit -m "trainer: extract obs_to_tensors helpers from smoke test

DRY the 17-field state layout + pheromone/history packing that
hierarchical_smoke.rs previously inlined. Now lives at
crates/antcolony-trainer/src/hierarchical/obs_to_tensors.rs, exposed
as pub fn so the JointPpoTrainer (Phase 2b-2) can call them too.

Adds rich_batch_to_tensors for the multi-colony batched-rollout case.
Three unit tests verify single, batched, and ant-side shapes.
Integration smoke test refactored to use the shared module — same
test, less duplication.

Refs: docs/superpowers/plans/2026-05-20-ant-brain-phase2b1-ppo-primitives.md
"
```

---

### Task 4: `MatchEnv::commander_obs_batch`

**Files:**
- Modify: `crates/antcolony-trainer/src/env.rs`

`MatchEnv` already wraps a `Simulation` with two colonies. Phase 2b-1 adds three batched-accessor methods so the trainer doesn't have to call `colony_rich_observation` twice and stack the results manually.

- [ ] **Step 1: Write the failing test**

Add to `crates/antcolony-trainer/src/env.rs`'s `#[cfg(test)] mod tests` block (create the block if it doesn't exist yet):

```rust
#[cfg(test)]
mod env_tests {
    use super::*;
    use candle_core::Device;
    use crate::hierarchical::sizing::{
        FIXED_HISTORY_K, FIXED_HISTORY_TOK_D, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H,
        FIXED_PHEROMONE_W, FIXED_STATE_D,
    };

    #[test]
    fn commander_obs_batch_shape_is_two_colonies_stacked() {
        let env = MatchEnv::new(0xb1a5_e1);
        let device = Device::Cpu;
        let (state, pheromone, history) = env.commander_obs_batch(&device).unwrap();
        assert_eq!(state.dims(), &[2, FIXED_STATE_D]);
        assert_eq!(pheromone.dims(), &[2, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W]);
        assert_eq!(history.dims(), &[2, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D]);
    }
}
```

If `env.rs` already has a `#[cfg(test)] mod tests` with a different name, append the new test inside it (or pick a non-colliding inner mod name like `phase2b1_env_tests`).

- [ ] **Step 2: Run the test, expect FAIL**

Run: `cd J:/antcolony && cargo test -p antcolony-trainer --lib env_tests::commander_obs_batch 2>&1 | tail -10`
Expected: COMPILE ERROR — `no method 'commander_obs_batch' on MatchEnv`.

- [ ] **Step 3: Implement `commander_obs_batch`**

Add to `impl MatchEnv` in `crates/antcolony-trainer/src/env.rs`:

```rust
/// Batched commander observations across both colonies (shape leading
/// dim = 2). Returns (state, pheromone, history) ready to feed
/// `HierarchicalActorCritic::forward_commander`.
pub fn commander_obs_batch(
    &self,
    device: &candle_core::Device,
) -> anyhow::Result<(candle_core::Tensor, candle_core::Tensor, candle_core::Tensor)> {
    let rich0 = self
        .sim
        .colony_rich_observation(0)
        .ok_or_else(|| anyhow::anyhow!("MatchEnv: colony 0 missing"))?;
    let rich1 = self
        .sim
        .colony_rich_observation(1)
        .ok_or_else(|| anyhow::anyhow!("MatchEnv: colony 1 missing"))?;
    let tup = crate::hierarchical::obs_to_tensors::rich_batch_to_tensors(
        &[&rich0, &rich1],
        device,
    )?;
    Ok(tup)
}
```

You'll need `use anyhow::Context;` already in scope (check). The method's signature uses fully qualified types so you don't need extra `use` lines if `env.rs` doesn't already import them.

- [ ] **Step 4: Run the test, expect PASS**

Run: `cd J:/antcolony && cargo test -p antcolony-trainer --lib env_tests::commander_obs_batch 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/env.rs
git commit -m "trainer/env: MatchEnv::commander_obs_batch batches two colonies' rich obs

Returns (state[2,17], pheromone[2,4,32,32], history[2,8,96]) ready to
feed HierarchicalActorCritic::forward_commander. Replaces the manual
two-call-and-stack pattern the trainer would otherwise do per rollout
step.
"
```

---

### Task 5: `MatchEnv::all_ant_obs_batch`

**Files:**
- Modify: `crates/antcolony-trainer/src/env.rs`

- [ ] **Step 1: Write the failing test**

Add to the same test mod in `env.rs`:

```rust
#[test]
fn all_ant_obs_batch_shapes_and_index_map() {
    let env = MatchEnv::new(0xb1a5_e1);
    let device = Device::Cpu;
    let intent_per_colony = candle_core::Tensor::randn(0.0f32, 1.0, (2, FIXED_INTENT_D), &device).unwrap();

    let (cone, internal, intent_b, index_map) = env.all_ant_obs_batch(&intent_per_colony, &device).unwrap();
    let n_total = index_map.len();
    assert!(n_total >= 2, "expected at least 2 ants across both colonies");

    use crate::hierarchical::sizing::{FIXED_CONE_D, FIXED_INTERNAL_D, FIXED_INTENT_D};
    assert_eq!(cone.dims(), &[n_total, FIXED_CONE_D]);
    assert_eq!(internal.dims(), &[n_total, FIXED_INTERNAL_D]);
    assert_eq!(intent_b.dims(), &[n_total, FIXED_INTENT_D]);

    // index_map entries are (colony_id, ant_id) — both colonies must be represented.
    let colonies: std::collections::HashSet<u8> = index_map.iter().map(|(c, _)| *c).collect();
    assert!(colonies.contains(&0));
    assert!(colonies.contains(&1));
}
```

- [ ] **Step 2: Implement**

Add to `impl MatchEnv` in `env.rs`:

```rust
/// Batched per-ant observations across BOTH colonies. The `intent_per_colony`
/// argument is a `(2, FIXED_INTENT_D)` tensor where row 0 is colony-0's
/// commander intent and row 1 is colony-1's. The returned `intent_b` tensor
/// expands those rows so each ant sees its own colony's intent.
///
/// `index_map` maps each row of the returned tensors back to its source ant
/// — entry `i` is `(colony_id, ant_id)`. The trainer uses this when packing
/// modulator outputs back into per-ant write-back calls.
pub fn all_ant_obs_batch(
    &self,
    intent_per_colony: &candle_core::Tensor,
    device: &candle_core::Device,
) -> anyhow::Result<(
    candle_core::Tensor,
    candle_core::Tensor,
    candle_core::Tensor,
    Vec<(u8, u32)>,
)> {
    use candle_core::{IndexOp, Tensor};
    use crate::hierarchical::sizing::{FIXED_CONE_D, FIXED_INTENT_D, FIXED_INTERNAL_D};

    let obs0 = self.sim.per_ant_observations(0);
    let obs1 = self.sim.per_ant_observations(1);
    let n0 = obs0.len();
    let n1 = obs1.len();
    let n_total = n0 + n1;

    // Build the combined cone + internal tensors.
    let mut cone_v = Vec::with_capacity(n_total * FIXED_CONE_D);
    let mut internal_v = Vec::with_capacity(n_total * FIXED_INTERNAL_D);
    let mut index_map = Vec::with_capacity(n_total);
    for o in &obs0 {
        cone_v.extend_from_slice(&o.pheromone_cone);
        internal_v.extend_from_slice(&o.internal);
        index_map.push((0u8, o.ant_id));
    }
    for o in &obs1 {
        cone_v.extend_from_slice(&o.pheromone_cone);
        internal_v.extend_from_slice(&o.internal);
        index_map.push((1u8, o.ant_id));
    }
    let cone = Tensor::from_vec(cone_v, (n_total, FIXED_CONE_D), device)?;
    let internal = Tensor::from_vec(internal_v, (n_total, FIXED_INTERNAL_D), device)?;

    // intent_b: expand the per-colony row to every ant's row.
    // intent_per_colony is (2, FIXED_INTENT_D). Take row 0, repeat n0 times; take row 1, repeat n1 times.
    let intent0 = intent_per_colony.i(0..1)?.broadcast_as((n0, FIXED_INTENT_D))?;
    let intent1 = intent_per_colony.i(1..2)?.broadcast_as((n1, FIXED_INTENT_D))?;
    let intent_b = Tensor::cat(&[&intent0, &intent1], 0)?;

    Ok((cone, internal, intent_b, index_map))
}
```

- [ ] **Step 3: Run the test, expect PASS**

Run: `cd J:/antcolony && cargo test -p antcolony-trainer --lib env_tests::all_ant_obs_batch 2>&1 | tail -10`
Expected: PASS.

If `IndexOp` slicing with `0..1` fails — candle's IndexOp accepts ranges in some versions. If it doesn't, the equivalent is `.narrow(0, 0, 1)?` (dim=0, start=0, len=1). Adapt if needed.

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/env.rs
git commit -m "trainer/env: MatchEnv::all_ant_obs_batch packs both colonies + index map

Returns (cone[N,60], internal[N,8], intent[N,64], Vec<(colony_id, ant_id)>)
where N = obs0.len() + obs1.len(). The index_map is the inverse of the
packing — Phase 2b-2's trainer uses it to scatter modulator outputs
back to the right (colony, ant_id) pairs for apply_ant_modulators.
"
```

---

### Task 6: `MatchEnv::apply_commander_intents` + `apply_ant_modulators_batched`

**Files:**
- Modify: `crates/antcolony-trainer/src/env.rs`

Both write-back wrappers. Pair them in one task since they're sym­metric to Tasks 4 and 5.

- [ ] **Step 1: Write the failing tests**

Add to the env tests mod:

```rust
#[test]
fn apply_commander_intents_writes_both_colonies() {
    let mut env = MatchEnv::new(0xb1a5_e1);
    let device = Device::Cpu;
    // intent_per_colony: (2, FIXED_INTENT_D)
    use crate::hierarchical::sizing::FIXED_INTENT_D;
    let intent = candle_core::Tensor::randn(0.0f32, 1.0, (2, FIXED_INTENT_D), &device).unwrap();
    env.apply_commander_intents(&intent).unwrap();
    let c0 = env.sim.colonies.get(0).unwrap().commander_intent;
    let c1 = env.sim.colonies.get(1).unwrap().commander_intent;
    // Either both wrote nonzero, or the input was nonzero — verify they differ
    // (random input → row 0 ≠ row 1 with probability ~1).
    assert_ne!(c0, c1, "commander intents should differ across colonies after random write");
}

#[test]
fn apply_ant_modulators_batched_clamps_and_writes_through() {
    use antcolony_sim::ai::observation::AntModulators;
    let mut env = MatchEnv::new(0xb1a5_e1);
    let device = Device::Cpu;
    use crate::hierarchical::sizing::{FIXED_INTENT_D, FIXED_MODULATOR_D};

    // Build the index_map by calling all_ant_obs_batch first.
    let intent = candle_core::Tensor::zeros((2, FIXED_INTENT_D), candle_core::DType::F32, &device).unwrap();
    let (_, _, _, index_map) = env.all_ant_obs_batch(&intent, &device).unwrap();
    let n = index_map.len();

    // Modulators tensor: all 5s for alpha_mult (over the clamp; should clamp to 5.0),
    // -10s for beta_mult (under the clamp; should clamp to 0.1).
    // We'll just set a recognizable per-ant pattern and verify it landed.
    let mut mods_v = Vec::with_capacity(n * FIXED_MODULATOR_D);
    for _ in 0..n {
        mods_v.extend_from_slice(&[3.0_f32, 0.5, 0.05, 2.0, -1.0]);  // safe-range values
    }
    let mods_t = candle_core::Tensor::from_vec(mods_v, (n, FIXED_MODULATOR_D), &device).unwrap();

    env.apply_ant_modulators_batched(&mods_t, &index_map).unwrap();

    // Verify at least one ant got the values.
    let (cid, aid) = index_map[0];
    let ant = env.sim.ants.iter().find(|a| a.id == aid && a.colony_id == cid).unwrap();
    assert_eq!(ant.modulators.alpha_mult, 3.0);
    assert_eq!(ant.modulators.beta_mult, 0.5);
}
```

- [ ] **Step 2: Implement both methods**

Add to `impl MatchEnv` in `env.rs`:

```rust
/// Apply commander intent vectors to both colonies. `intent_per_colony`
/// is a (2, FIXED_INTENT_D) tensor — row 0 → colony 0, row 1 → colony 1.
pub fn apply_commander_intents(&mut self, intent_per_colony: &candle_core::Tensor) -> anyhow::Result<()> {
    use crate::hierarchical::sizing::FIXED_INTENT_D;
    let dims = intent_per_colony.dims();
    if dims != [2usize, FIXED_INTENT_D].as_slice() {
        anyhow::bail!(
            "apply_commander_intents: expected shape [2, {}], got {:?}",
            FIXED_INTENT_D, dims,
        );
    }
    let row0: Vec<f32> = intent_per_colony.i(0)?.to_vec1()?;
    let row1: Vec<f32> = intent_per_colony.i(1)?.to_vec1()?;
    let mut a0 = [0.0f32; FIXED_INTENT_D];
    a0.copy_from_slice(&row0);
    let mut a1 = [0.0f32; FIXED_INTENT_D];
    a1.copy_from_slice(&row1);
    self.sim.apply_commander_intent(0, &a0);
    self.sim.apply_commander_intent(1, &a1);
    Ok(())
}

/// Apply batched per-ant modulators to the right (colony, ant) pairs.
/// `mods_t` is a (N, FIXED_MODULATOR_D) tensor; `index_map[i]` tells us
/// which ant row `i` belongs to. The sim-side apply_ant_modulators
/// already write-clamps each component.
pub fn apply_ant_modulators_batched(
    &mut self,
    mods_t: &candle_core::Tensor,
    index_map: &[(u8, u32)],
) -> anyhow::Result<()> {
    use antcolony_sim::ai::observation::AntModulators;
    use crate::hierarchical::sizing::FIXED_MODULATOR_D;

    let dims = mods_t.dims();
    if dims.len() != 2 || dims[1] != FIXED_MODULATOR_D || dims[0] != index_map.len() {
        anyhow::bail!(
            "apply_ant_modulators_batched: expected shape [{}, {}], got {:?}",
            index_map.len(), FIXED_MODULATOR_D, dims,
        );
    }
    // Flatten to host. For ~10-100 ants this is cheap; Phase 2b-2 can
    // optimize if profiling shows it's hot.
    let flat: Vec<f32> = mods_t.flatten_all()?.to_vec1()?;

    // Group writes by colony so we make one apply_ant_modulators call per colony.
    let mut by_colony: [(Vec<AntModulators>, Vec<u32>); 2] = [
        (Vec::new(), Vec::new()),
        (Vec::new(), Vec::new()),
    ];
    for (i, &(cid, aid)) in index_map.iter().enumerate() {
        let off = i * FIXED_MODULATOR_D;
        let m = AntModulators {
            alpha_mult: flat[off],
            beta_mult: flat[off + 1],
            exploration_mod: flat[off + 2],
            deposit_mult: flat[off + 3],
            state_bias: flat[off + 4],
        };
        let slot = if cid == 0 { 0 } else { 1 };
        by_colony[slot].0.push(m);
        by_colony[slot].1.push(aid);
    }
    for cid in [0u8, 1u8] {
        let slot = cid as usize;
        if !by_colony[slot].1.is_empty() {
            self.sim.apply_ant_modulators(cid, &by_colony[slot].0, &by_colony[slot].1);
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Run the tests, expect PASS**

Run: `cd J:/antcolony && cargo test -p antcolony-trainer --lib env_tests 2>&1 | tail -15`
Expected: all env_tests pass (including the 3 from Tasks 4/5/6).

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/env.rs
git commit -m "trainer/env: MatchEnv write-back wrappers (intent + ant modulators batched)

apply_commander_intents — 2-row tensor in, two colonies updated.
apply_ant_modulators_batched — N-row modulator tensor + index_map in,
grouped per-colony apply_ant_modulators calls out. Phase 2b-2's
trainer will use the index_map returned from all_ant_obs_batch to drive
this — single forward pass through the ant policy, single write-back
per colony.
"
```

---

### Task 7: `HierarchicalActorCritic::sample_commander`

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/actor_critic.rs`

The pattern follows `crates/antcolony-trainer/src/policy.rs:92` (`ActorCritic::sample`) exactly — Box-Muller Gaussian sample from learnable per-dim std, tanh squash, log-prob with Jacobian correction.

- [ ] **Step 1: Write the failing test**

Create `crates/antcolony-trainer/tests/hierarchical_sampling.rs`:

```rust
//! Integration tests for the Phase 2b-1 sampling + log-prob methods on
//! HierarchicalActorCritic.

use candle_core::{Device, Tensor};
use candle_nn::{VarBuilder, VarMap};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::HierarchicalActorCritic;

fn cpu_hac() -> (VarMap, HierarchicalActorCritic, Device) {
    let varmap = VarMap::new();
    let device = Device::Cpu;
    let vb = VarBuilder::from_varmap(&varmap, candle_core::DType::F32, &device);
    let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
    (varmap, hac, device)
}

#[test]
fn sample_commander_shapes_and_finite() {
    let (_vm, hac, device) = cpu_hac();
    let b = 2usize;
    let state = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_state_d), &device).unwrap();
    let pheromone = Tensor::randn(
        0.0f32, 1.0,
        (b, A1.fixed_pheromone_c, A1.fixed_pheromone_h, A1.fixed_pheromone_w),
        &device,
    ).unwrap();
    let history = Tensor::randn(
        0.0f32, 1.0,
        (b, A1.fixed_history_k, A1.fixed_history_tok_d),
        &device,
    ).unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(0xfeed);
    let s = hac.sample_commander(&state, &pheromone, &history, &mut rng).unwrap();
    assert_eq!(s.action.dims(), &[b, A1.fixed_action_d]);
    assert_eq!(s.intent.dims(), &[b, A1.fixed_intent_d]);
    assert_eq!(s.value.dims(), &[b]);
    assert_eq!(s.log_prob.dims(), &[b]);

    let action_v: Vec<f32> = s.action.flatten_all().unwrap().to_vec1().unwrap();
    assert!(action_v.iter().all(|v| v.is_finite()));
    // action is post-squash to [0, 1] per the existing ActorCritic convention.
    assert!(action_v.iter().all(|v| (0.0..=1.0).contains(v)),
        "post-squash action should be in [0, 1], got {:?}", action_v);

    let lp_v: Vec<f32> = s.log_prob.flatten_all().unwrap().to_vec1().unwrap();
    assert!(lp_v.iter().all(|v| v.is_finite()),
        "log_prob non-finite: {:?}", lp_v);
}
```

- [ ] **Step 2: Add the sampling method**

In `crates/antcolony-trainer/src/hierarchical/actor_critic.rs`:

```rust
/// Bundle of outputs from a stochastic commander sample.
pub struct CommanderSample {
    pub action: Tensor,    // [B, 6] — post-squash to [0, 1]
    pub intent: Tensor,    // [B, 64] — same as forward; commander intent is deterministic
    pub value: Tensor,     // [B]
    pub log_prob: Tensor,  // [B] — log-prob of action under the Gaussian+tanh policy
}

impl HierarchicalActorCritic {
    /// Stochastic commander rollout step. Mirrors the Gaussian + tanh-squash
    /// + Jacobian-corrected log-prob recipe used by the existing flat
    /// `ActorCritic::sample` (see `crates/antcolony-trainer/src/policy.rs:92`).
    /// Uses the provided RNG so rollouts are reproducible.
    pub fn sample_commander(
        &self,
        state: &Tensor,
        pheromone: &Tensor,
        history: &Tensor,
        rng: &mut rand_chacha::ChaCha8Rng,
    ) -> candle_core::Result<CommanderSample> {
        use rand::Rng;

        let fwd = self.commander.forward(state, pheromone, history)?;
        // `fwd.action` is the pre-squash mean (the action_head's raw output).
        let mean = fwd.action;
        let (b, action_d) = mean.dims2()?;
        let std = self.commander.log_std.exp()?;  // [action_d]

        // Box-Muller noise per batch entry, per dim.
        let mut noise = Vec::with_capacity(b * action_d);
        for _ in 0..(b * action_d) {
            let u1: f32 = rng.r#gen_range(1e-6..1.0);
            let u2: f32 = rng.r#gen_range(0.0..1.0);
            noise.push((-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos());
        }
        let noise_t = Tensor::from_vec(noise, (b, action_d), mean.device())?;
        let scaled = noise_t.broadcast_mul(&std)?;
        let u = (&mean + &scaled)?;
        let action = squash_tanh_to_unit(&u)?;

        // log-prob under Normal(mean, std), with squash Jacobian correction.
        let diff = (&u - &mean)?;
        let std_sq = std.broadcast_mul(&std)?;
        let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
        let two_pi_log = 0.918_938_5_f64; // 0.5 * ln(2π)
        let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
        let log_pdf = log_pdf_part1.broadcast_sub(&self.commander.log_std)?;
        let tanh_u = u.tanh()?;
        let one = Tensor::ones_like(&tanh_u)?;
        let one_minus_tanh_sq = (&one - &(&tanh_u * &tanh_u)?)?;
        let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -0.693_147_2_f64)?;
        // Sum over action_d to get one scalar log-prob per batch entry.
        let log_prob = (log_pdf - log_jac)?.sum(candle_core::D::Minus1)?;

        Ok(CommanderSample {
            action,
            intent: fwd.intent,
            value: fwd.value,
            log_prob,
        })
    }
}

/// Map pre-squash `u: [...]` to post-squash action in `[0, 1]` per dim,
/// using `0.5 * (tanh(u) + 1)`. Matches the existing `ActorCritic::squash`
/// (policy.rs:75) so trained weights are deployment-compatible.
fn squash_tanh_to_unit(u: &Tensor) -> candle_core::Result<Tensor> {
    let t = u.tanh()?;
    let one = Tensor::ones_like(&t)?;
    (t + one)?.affine(0.5, 0.0)
}
```

The `commander.action_head` outputs the **pre-squash mean** (forward already does this — its `action` field is the linear head output before any tanh). Apply tanh+scale here in sampling code.

You'll need `rand` already in `Cargo.toml` (the existing `policy.rs` uses it — verify by grepping).

- [ ] **Step 3: Run the test, expect PASS**

Run: `cd J:/antcolony && cargo test -p antcolony-trainer --test hierarchical_sampling sample_commander_shapes_and_finite 2>&1 | tail -10`
Expected: PASS.

If you get a compile error on `r#gen_range` — newer `rand` uses `gen_range` without the raw prefix; older uses `r#gen_range`. The existing `policy.rs:97-101` uses `rng.gen_range(...)` directly. Match whichever form is in use.

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/actor_critic.rs crates/antcolony-trainer/tests/hierarchical_sampling.rs
git commit -m "hac: sample_commander with Gaussian + tanh-squash + Jacobian log-prob

Mirrors the flat ActorCritic::sample recipe (policy.rs:92). Returns
CommanderSample { action [B,6] in [0,1], intent [B,64], value [B],
log_prob [B] }. Box-Muller noise via ChaCha8Rng so rollouts are
reproducible. Test verifies shape + finiteness + post-squash range.
"
```

---

### Task 8: `HierarchicalActorCritic::log_prob_of_commander_action`

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/actor_critic.rs`

The PPO importance-ratio computation: given a STORED action from an earlier rollout, what's the log-prob under the CURRENT policy? Same math as the sampling log-prob, just running the squash backwards to recover `u`.

- [ ] **Step 1: Add the test**

Append to `crates/antcolony-trainer/tests/hierarchical_sampling.rs`:

```rust
#[test]
fn log_prob_round_trip_through_squash() {
    // If we sample an action via sample_commander, then ask log_prob_of_commander_action
    // for the SAME action under the SAME policy, we should get the SAME log_prob.
    // (Modulo numerical noise from the atanh round-trip near the [0, 1] edges.)
    let (_vm, hac, device) = cpu_hac();
    let b = 2usize;
    let state = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_state_d), &device).unwrap();
    let pheromone = Tensor::randn(
        0.0f32, 1.0,
        (b, A1.fixed_pheromone_c, A1.fixed_pheromone_h, A1.fixed_pheromone_w),
        &device,
    ).unwrap();
    let history = Tensor::randn(
        0.0f32, 1.0,
        (b, A1.fixed_history_k, A1.fixed_history_tok_d),
        &device,
    ).unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(0xcafe);
    let s = hac.sample_commander(&state, &pheromone, &history, &mut rng).unwrap();
    let lp_round = hac.log_prob_of_commander_action(&state, &pheromone, &history, &s.action).unwrap();

    let lp_sample: Vec<f32> = s.log_prob.flatten_all().unwrap().to_vec1().unwrap();
    let lp_recompute: Vec<f32> = lp_round.flatten_all().unwrap().to_vec1().unwrap();
    for (s, r) in lp_sample.iter().zip(lp_recompute.iter()) {
        assert!(
            (s - r).abs() < 1e-3,
            "log_prob round-trip mismatch: sample={s}, recompute={r}, diff={}",
            (s - r).abs(),
        );
    }
}
```

- [ ] **Step 2: Implement**

Add to `impl HierarchicalActorCritic` in `actor_critic.rs`:

```rust
/// Recompute the log-prob of a previously-sampled (post-squash) action
/// under the current policy. Used by PPO's importance ratio
/// (`r_θ = exp(log_prob_now - log_prob_old)`). Mirrors
/// `ActorCritic::log_prob_of` at policy.rs:125.
pub fn log_prob_of_commander_action(
    &self,
    state: &Tensor,
    pheromone: &Tensor,
    history: &Tensor,
    action_squashed: &Tensor,
) -> candle_core::Result<Tensor> {
    // Invert the squash: action = 0.5 * (tanh(u) + 1) ⇒ tanh(u) = 2*action - 1
    // u = atanh(z), z = 2*action - 1. Clamp to (-1+eps, 1-eps) for numerical stability.
    let fwd = self.commander.forward(state, pheromone, history)?;
    let mean = fwd.action;  // pre-squash mean
    let std = self.commander.log_std.exp()?;
    let two_a = action_squashed.affine(2.0, -1.0)?;
    let clamped = two_a.clamp(-0.999_999_f32, 0.999_999_f32)?;
    let one = Tensor::ones_like(&clamped)?;
    let plus = (&one + &clamped)?;
    let minus = (&one - &clamped)?;
    let u = (plus / minus)?.log()?.affine(0.5, 0.0)?;

    let diff = (&u - &mean)?;
    let std_sq = std.broadcast_mul(&std)?;
    let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
    let two_pi_log = 0.918_938_5_f64;
    let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
    let log_pdf = log_pdf_part1.broadcast_sub(&self.commander.log_std)?;
    let tanh_u = u.tanh()?;
    let one_t = Tensor::ones_like(&tanh_u)?;
    let one_minus_tanh_sq = (&one_t - &(&tanh_u * &tanh_u)?)?;
    let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -0.693_147_2_f64)?;
    let log_prob = (log_pdf - log_jac)?.sum(candle_core::D::Minus1)?;
    Ok(log_prob)
}
```

- [ ] **Step 3: Run the test**

Run: `cd J:/antcolony && cargo test -p antcolony-trainer --test hierarchical_sampling log_prob_round_trip 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/actor_critic.rs crates/antcolony-trainer/tests/hierarchical_sampling.rs
git commit -m "hac: log_prob_of_commander_action for PPO importance ratio

Inverts the squash via atanh(2*action - 1) to recover u, then evaluates
log Normal(u; mean, std) - log Jacobian. Round-trip test verifies
log_prob_of(sample) ≈ sample.log_prob within 1e-3 (the atanh edge
clamp introduces small numerical noise). Mirrors policy.rs::log_prob_of.
"
```

---

### Task 9: `HierarchicalActorCritic::sample_ant`

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/actor_critic.rs`

Same pattern as `sample_commander`, but on the ant tier. Modulator output dim is 5.

- [ ] **Step 1: Add the test**

Append to `hierarchical_sampling.rs`:

```rust
#[test]
fn sample_ant_shapes_and_finite() {
    let (_vm, hac, device) = cpu_hac();
    let b = 7usize;
    let cone = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_cone_d), &device).unwrap();
    let intern = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_internal_d), &device).unwrap();
    let intent = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_intent_d), &device).unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(0xc0ffee);
    let s = hac.sample_ant(&cone, &intern, &intent, &mut rng).unwrap();
    assert_eq!(s.modulator.dims(), &[b, A1.fixed_modulator_d]);
    assert_eq!(s.value.dims(), &[b]);
    assert_eq!(s.log_prob.dims(), &[b]);

    let mod_v: Vec<f32> = s.modulator.flatten_all().unwrap().to_vec1().unwrap();
    assert!(mod_v.iter().all(|v| v.is_finite()));
    // Post-squash modulator is in [0, 1]; the apply-side clamp in
    // Simulation::apply_ant_modulators rescales these into the safe
    // per-field ranges. Here we just check finiteness + [0, 1].
    assert!(mod_v.iter().all(|v| (0.0..=1.0).contains(v)));

    let lp_v: Vec<f32> = s.log_prob.flatten_all().unwrap().to_vec1().unwrap();
    assert!(lp_v.iter().all(|v| v.is_finite()));
}
```

- [ ] **Step 2: Implement**

Add to `actor_critic.rs`:

```rust
pub struct AntSample {
    pub modulator: Tensor,  // [B, 5] — post-squash to [0, 1]
    pub value: Tensor,      // [B]
    pub log_prob: Tensor,   // [B]
}

impl HierarchicalActorCritic {
    pub fn sample_ant(
        &self,
        cone: &Tensor,
        internal: &Tensor,
        intent: &Tensor,
        rng: &mut rand_chacha::ChaCha8Rng,
    ) -> candle_core::Result<AntSample> {
        use rand::Rng;
        let fwd = self.ant.forward(cone, internal, intent)?;
        let mean = fwd.modulator;
        let (b, mod_d) = mean.dims2()?;
        let std = self.ant.log_std.exp()?;

        let mut noise = Vec::with_capacity(b * mod_d);
        for _ in 0..(b * mod_d) {
            let u1: f32 = rng.r#gen_range(1e-6..1.0);
            let u2: f32 = rng.r#gen_range(0.0..1.0);
            noise.push((-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos());
        }
        let noise_t = Tensor::from_vec(noise, (b, mod_d), mean.device())?;
        let scaled = noise_t.broadcast_mul(&std)?;
        let u = (&mean + &scaled)?;
        let modulator = squash_tanh_to_unit(&u)?;

        let diff = (&u - &mean)?;
        let std_sq = std.broadcast_mul(&std)?;
        let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
        let two_pi_log = 0.918_938_5_f64;
        let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
        let log_pdf = log_pdf_part1.broadcast_sub(&self.ant.log_std)?;
        let tanh_u = u.tanh()?;
        let one = Tensor::ones_like(&tanh_u)?;
        let one_minus_tanh_sq = (&one - &(&tanh_u * &tanh_u)?)?;
        let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -0.693_147_2_f64)?;
        let log_prob = (log_pdf - log_jac)?.sum(candle_core::D::Minus1)?;

        Ok(AntSample {
            modulator,
            value: fwd.value,
            log_prob,
        })
    }
}
```

- [ ] **Step 3: Run the test**

Run: `cd J:/antcolony && cargo test -p antcolony-trainer --test hierarchical_sampling sample_ant 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/actor_critic.rs crates/antcolony-trainer/tests/hierarchical_sampling.rs
git commit -m "hac: sample_ant — Gaussian + tanh-squash log-prob for the ant tier

Same recipe as sample_commander but on the 5-d modulator action space.
Returns AntSample { modulator [B,5] in [0,1], value [B], log_prob [B] }.
Phase 2b-2's trainer will scale the post-squash modulators into the
safe per-field ranges before calling apply_ant_modulators_batched.
"
```

---

### Task 10: `HierarchicalActorCritic::log_prob_of_ant_modulator`

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/actor_critic.rs`

- [ ] **Step 1: Add the test**

Append to `hierarchical_sampling.rs`:

```rust
#[test]
fn ant_log_prob_round_trip() {
    let (_vm, hac, device) = cpu_hac();
    let b = 5usize;
    let cone = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_cone_d), &device).unwrap();
    let intern = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_internal_d), &device).unwrap();
    let intent = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_intent_d), &device).unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(0xdeed);
    let s = hac.sample_ant(&cone, &intern, &intent, &mut rng).unwrap();
    let lp_round = hac.log_prob_of_ant_modulator(&cone, &intern, &intent, &s.modulator).unwrap();

    let lp_sample: Vec<f32> = s.log_prob.flatten_all().unwrap().to_vec1().unwrap();
    let lp_recompute: Vec<f32> = lp_round.flatten_all().unwrap().to_vec1().unwrap();
    for (s, r) in lp_sample.iter().zip(lp_recompute.iter()) {
        assert!((s - r).abs() < 1e-3,
            "ant log_prob round-trip mismatch: sample={s}, recompute={r}");
    }
}
```

- [ ] **Step 2: Implement**

Add to `actor_critic.rs`:

```rust
impl HierarchicalActorCritic {
    pub fn log_prob_of_ant_modulator(
        &self,
        cone: &Tensor,
        internal: &Tensor,
        intent: &Tensor,
        modulator_squashed: &Tensor,
    ) -> candle_core::Result<Tensor> {
        let fwd = self.ant.forward(cone, internal, intent)?;
        let mean = fwd.modulator;
        let std = self.ant.log_std.exp()?;
        let two_m = modulator_squashed.affine(2.0, -1.0)?;
        let clamped = two_m.clamp(-0.999_999_f32, 0.999_999_f32)?;
        let one = Tensor::ones_like(&clamped)?;
        let plus = (&one + &clamped)?;
        let minus = (&one - &clamped)?;
        let u = (plus / minus)?.log()?.affine(0.5, 0.0)?;

        let diff = (&u - &mean)?;
        let std_sq = std.broadcast_mul(&std)?;
        let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
        let two_pi_log = 0.918_938_5_f64;
        let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
        let log_pdf = log_pdf_part1.broadcast_sub(&self.ant.log_std)?;
        let tanh_u = u.tanh()?;
        let one_t = Tensor::ones_like(&tanh_u)?;
        let one_minus_tanh_sq = (&one_t - &(&tanh_u * &tanh_u)?)?;
        let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -0.693_147_2_f64)?;
        let log_prob = (log_pdf - log_jac)?.sum(candle_core::D::Minus1)?;
        Ok(log_prob)
    }
}
```

- [ ] **Step 3: Run all sampling tests**

Run: `cd J:/antcolony && cargo test -p antcolony-trainer --test hierarchical_sampling 2>&1 | tail -10`
Expected: 4 tests pass (sample_commander, log_prob_round_trip, sample_ant, ant_log_prob_round_trip).

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/actor_critic.rs crates/antcolony-trainer/tests/hierarchical_sampling.rs
git commit -m "hac: log_prob_of_ant_modulator for PPO importance ratio

Same atanh inverse + Gaussian + Jacobian as the commander side, on
the 5-d ant modulator action space. Round-trip test confirms
log_prob_of(sample) ≈ sample.log_prob within 1e-3.

Phase 2b-1 PPO primitives are now complete: HAC can sample stochastically
and recompute log-probs for stored actions. Phase 2b-2 will use these
to compute the PPO clipped surrogate.
"
```

---

### Task 11: Phase 2b-1 acceptance + HANDOFF update

**Files:** verification + HANDOFF.

- [ ] **Step 1: Workspace test sweep**

Run: `cd J:/antcolony && cargo test --workspace 2>&1 | tail -15`
Expected: all tests pass. Tally vs prior state:
- antcolony-sim: 164 lib + 13 phase1_plumbing (Phase 1 + deposit_mult + state_bias) = 177
- antcolony-trainer: 14 hierarchical unit + 3 obs_to_tensors + N env_tests + 1 smoke + 4 sampling = ~25+ tests

If anything regresses, STOP and report BLOCKED.

- [ ] **Step 2: Clippy on Phase 2b-1 code**

Run: `cd J:/antcolony && cargo clippy -p antcolony-trainer --lib --tests -- -D warnings 2>&1 | tail -20`

Pre-existing warnings in `policy.rs`/`ppo.rs`/`env.rs` (the parts NOT touched by Phase 2b-1) are acceptable. NEW warnings in Phase 2b-1 code (`obs_to_tensors.rs`, the new env.rs methods, `actor_critic.rs` new methods, `hierarchical_sampling.rs`) must be zero.

Common candidates to fix inline:
- `manual_range_contains` — convert `x >= a && x <= b` to `(a..=b).contains(&x)`
- unused imports
- `dead_code` if any field added but not used yet

- [ ] **Step 3: Workspace build**

Run: `cd J:/antcolony && cargo build --workspace 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 4: HANDOFF update**

Append a new session entry at the very top of `J:/antcolony/HANDOFF.md` (above the existing 2026-05-20 Phase 2a entry):

```markdown
## Session <date> — Phase 2b-1 PPO primitives + state_bias sim wiring landed

🟢 Project Status: **Phase 2b-1 ship-ready.** Branch `feat/ant-brain-phase2b1` (final commit `<sha>`) ships the trainer-side PPO primitives + sim-side state_bias wiring. `HierarchicalActorCritic` now has `sample_commander`/`sample_ant` (Gaussian + tanh-squash + Jacobian log-prob) and `log_prob_of_commander_action`/`log_prob_of_ant_modulator` (PPO importance ratio inputs). `MatchEnv` has 4 new batch accessors (`commander_obs_batch`, `all_ant_obs_batch`, `apply_commander_intents`, `apply_ant_modulators_batched`). Sim-side `state_bias` modulator is now read at the Exploring → FollowingTrail transition site — non-default values measurably shift transition rates while defaults preserve baseline. Observation→tensor conversion DRY'd into `pub fn obs_to_tensors` module. Existing flat ActorCritic + PpoTrainer untouched.

### What's Next

- Phase 2b-2 plan: `JointPpoTrainer` struct + two-buffer rollout + per-tier GAE + joint loss + Adam update + 5-iter smoke training run on kokonoe (3070 Ti, fp16). All the primitives 2b-1 just shipped are the building blocks.
- Pre-existing tech debt unchanged (33 clippy warnings in non-Phase-2 code; species cap test was fixed earlier).

### Notes for Next Session

- All 4 sampling/log-prob methods use `rand_chacha::ChaCha8Rng` for reproducibility — Phase 2b-2 must thread a single RNG through the rollout loop, not create a fresh one per call.
- `MatchEnv::apply_ant_modulators_batched` groups writes by colony then calls `apply_ant_modulators` per colony — this avoids the O(N²) ant-id lookup penalty when N gets large. Acceptable today; revisit if 10k-ant matches surface a perf cliff.
- The `state_bias` injection site is at <FILE:LINE — fill in based on Task 2's commit message>. If the FSM grows new transitions in later phases, decide whether they should also read state_bias, or whether each transition gets its own bias field.
- `obs_to_tensors::history_flatten` pads with zeros when the colony's ring has fewer than K=8 tokens. Phase 2b-2's first 5 iterations will have empty rings → all-zero history. Worth observing whether the policy actually attends to the history dimension at all in those early iterations.
```

Fill in `<date>` (today, 2026-05-20 or later), `<sha>` (the SHA of the Task 11 commit), and the state_bias `<FILE:LINE>`.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add HANDOFF.md
git commit -m "handoff: phase 2b-1 PPO primitives + state_bias wiring complete

state_bias sim wiring + DRY obs_to_tensors + 4 MatchEnv batch
accessors + 4 HAC sampling/log-prob methods. Phase 2b-2 (JointPpoTrainer
+ smoke training run) can layer entirely on top — no further sim or
API-shape work needed.
"
```

---

## Acceptance criteria (recap)

Phase 2b-1 is **done** when ALL true:

1. `cargo test --workspace` passes (no regressions to Phase 1 / Phase 2a tests).
2. `cargo clippy -p antcolony-trainer --lib --tests -- -D warnings` is clean on Phase 2b-1 code (`obs_to_tensors.rs`, new `env.rs` methods, new `actor_critic.rs` methods, `hierarchical_sampling.rs`).
3. `cargo build --workspace` builds clean.
4. `state_bias_shifts_following_trail_transition_rate` passes.
5. `defaults_reproduce_baseline_population_trajectory` still passes (state_bias = 0 is the identity).
6. `deposit_mult_strengthens_pheromone_deposition` still passes (Phase 2a behavior unchanged).
7. All 4 tests in `hierarchical_sampling.rs` pass (sample_commander, log_prob round-trip, sample_ant, ant log_prob round-trip).
8. `crates/antcolony-trainer/src/policy.rs` and `ppo.rs` are unchanged (47% Nash regression baseline preserved).

---

## Out-of-scope for Phase 2b-1 (deferred to Phase 2b-2 or later)

- **`JointPpoTrainer` and the rollout loop** — Phase 2b-2.
- **GAE / advantage computation** — Phase 2b-2.
- **Joint loss + Adam optimizer step** — Phase 2b-2.
- **The 5-iter smoke training example binary** — Phase 2b-2.
- **CUDA / fp16 verification** — first GPU run is Phase 2b-2.
- **Multi-GPU rollout/train split** — Phase 3.
- **A3 cnc training** — Phase 4.
- **Distillation + quantization + deployment** — Phases 5–6.
- **33 pre-existing clippy warnings** — separate cleanup sweep.
- **Reward shaping changes** — `env.rs::step` r6 shaping stays unchanged.

---

## Open questions / known unknowns

- **The exact FSM transition site for `state_bias`.** Task 1's grep step should locate it; if no clear single site exists, Pattern C (explicit transition gate in the per-ant tick) is the fallback. The behavioral test asserts SOME shift in FollowingTrail count for non-default bias — implementer has license to choose the wiring math.
- **`rand` crate version and `gen_range` vs `r#gen_range`.** Existing `policy.rs:97-101` uses `rng.gen_range(...)` without `r#`. If the workspace's `rand` version is new enough that `gen` is reserved, candle's vendored deps may force `r#gen_range`. Match whichever form `policy.rs` uses.
- **`IndexOp::i(0..1)` syntax.** Candle's `IndexOp` accepts ranges in current versions; fallback is `.narrow(0, 0, 1)?` if it doesn't compile.
- **`Tensor::broadcast_as` behavior on `(1, D)` → `(N, D)`.** Used in `ant_obs_to_tensors` and `all_ant_obs_batch`. Candle handles this as a view (no copy) — verify by reading the candle docs if tests fail with shape errors.
- **Whether the integration test in `hierarchical_smoke.rs` (Phase 2a's a1_hac_drives_from_fresh_sim) needs updates after the DRY refactor.** Task 3 specifies the refactor — confirm the test still passes after the helper extraction.

None of the above is a blocker; they're places where the implementer may need a small adapt-and-retry cycle.

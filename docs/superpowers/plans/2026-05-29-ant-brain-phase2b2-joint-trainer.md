# Ant Brain Phase 2b-2 — Joint PPO Trainer (single-device smoke) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `JointPpoTrainer` — a two-tier (commander + ant) PPO trainer that drives a self-play `MatchEnv` through a two-buffer rollout, computes per-tier GAE, optimizes a single joint loss with an Adam step, and completes a 5-iteration smoke run on a single device without NaN/crash, measurably moving both tiers' weights.

**Architecture:** Reuse the Phase-2b-1 primitives (`HierarchicalActorCritic::{sample_commander, sample_ant, log_prob_of_commander_action, log_prob_of_ant_modulator}`) and the `MatchEnv` batch accessors (`commander_obs_batch`, `all_ant_obs_batch`, `apply_commander_intents`, `apply_ant_modulators_batched`). The trainer self-plays both colonies with one shared net, collects a commander buffer (1 record/colony/cycle) and an ant buffer (1 record/ant/tick), runs GAE separately per tier at the colony level, and combines `L_total = L_cmdr + α_balance · L_ant` for one `AdamW::backward_step`. Numerics (Gaussian + tanh-squash + Jacobian log-prob, clipped surrogate, value MSE, entropy bonus) mirror the existing flat `PpoTrainer::ppo_update` exactly.

**Tech Stack:** Rust (edition 2024, stable-gnu 1.95), candle-core / candle-nn (CPU f32 — see "Device & precision" below), rand_chacha for reproducible sampling, the existing `antcolony-sim` + `antcolony-trainer` crates.

---

## Device & precision (read before starting)

> **UPDATE 2026-05-29 — CUDA DOES build and train on kokonoe.** The original claim below (CPU-only, "CUDA blocked by missing MSVC") was a misdiagnosis. nvcc 12.6, VS2022 BuildTools (cl/link), and the msvc Rust toolchain are all installed. The trainer builds with `--features cuda` (15/15 candle kernels via nvcc) and the joint-PPO smoke **trains on the RTX 3070 Ti** (`cuda=true`, all losses finite). The real blockers were (1) Community VS missing `vcvarsall.bat` — use BuildTools vcvars64; (2) global `~/.cargo/config.toml` forcing uninstalled `lld-link` — override with `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=link.exe`; (3) candle CUDA matmul needs contiguous operands — fixed with `.contiguous()` in `commander.rs`/`ant.rs`. Recipe: `scripts/build_trainer_cuda.bat` / `scripts/run_joint_smoke_cuda.bat` (branch `feat/trainer-cuda-runtime`). The original CPU smoke is still valid; GPU is now also available locally for faster iteration, which changes the Phase-3 calculus (single-GPU dev can happen on kokonoe, not only on the cnc P100s).

The smoke as originally written runs on `Device::Cpu` in `DType::F32` — sufficient to validate the joint-loop mechanics (Gate 1: "runs without OOM/NaN/crash"). With the CUDA enablement above, the same trainer also runs on the 3070 Ti via `CandleBackend::new()` (CUDA device 0 under `--features cuda`, else CPU). fp16/bf16 mixed precision + multi-GPU split is still Phase 3 (cnc P100s).

## Smoke-scope simplifications (explicit — these are deliberate, not gaps)

This is a **mechanical smoke**, not a convergence run. The following are intentional simplifications, each flagged so they can be revisited in Phase 3:

1. **Self-play, both colonies = the trained net.** No league opponent. This exercises the 2-colony batch accessors directly and needs no opponent plumbing. (The flat `PpoTrainer` trains left-vs-league; the hierarchical design data-flow drives both colonies, so self-play is the natural fit.)
2. **Colony-level advantages at cycle cadence for BOTH tiers.** The design spec says "advantages are colony-level" and "one shared environment reward stream." We compute GAE per (colony, match) at the commander's 5-tick cycle cadence. The commander tier uses the commander value head; the ant tier uses a per-cycle **mean of that colony's ant value-head outputs** as its bootstrap value (a genuinely separate value function → genuinely "per-tier GAE"). Every ant sampled during a cycle inherits that cycle's ant-tier advantage/return. Per-tick ant credit assignment is deferred to Phase 3.
3. **Reward = r6, unchanged.** Identical shaping to `MatchEnv::step` (worker-delta ×0.01, food-delta ×0.002, queen-alive ±0.005, terminal ±1, timeout share). Colony 0 gets `reward_left`, colony 1 gets `reward_right = -reward_left`.
4. **Short horizon.** `rollout_cycles = 8`, `matches_per_iter = 2`, `iterations = 5`. Enough to fill buffers and step Adam 5×; not enough to learn anything. A1 sizing only.
5. **No grad-norm clipping.** candle's `AdamW` has no built-in clip and the flat trainer never applied it either (`max_grad_norm` is dead in `PpoConfig`). We omit it; note it for Phase 3.
6. **Two forward passes per tier in the update** (one for `log_prob_of_*`, one for `value`). Wasteful but simplest; the flat trainer does the same. Fusing is a Phase-3 optimization.

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `crates/antcolony-sim/src/simulation.rs` | Sim core | **Modify** — add `Simulation::push_commander_history` (append a `HistoryToken` to a colony's ring with FIFO eviction at capacity 8). |
| `crates/antcolony-trainer/src/joint_ppo.rs` | The whole joint trainer: `JointPpoConfig`, `JointPpoTrainer`, record/buffer types, `rollout`, GAE helpers, `joint_update`, `train`. | **Create** |
| `crates/antcolony-trainer/src/lib.rs` | Crate root | **Modify** — `pub mod joint_ppo;` + re-exports. |
| `crates/antcolony-trainer/src/bin/joint_smoke.rs` | CLI entry to run the 5-iter smoke on kokonoe and log per-iter losses. | **Create** |
| `crates/antcolony-trainer/tests/joint_ppo_smoke.rs` | Integration test: 5 iters complete, all losses finite, weights move. | **Create** |

Unit tests for the small pieces (config defaults, trainer build, record buffer, GAE bucketing, single rollout, single update) live inline in `joint_ppo.rs` under `#[cfg(test)] mod tests`. The end-to-end 5-iter run is the integration test.

---

## Task 1: `Simulation::push_commander_history` (sim-side ring append)

The Phase-1 commander history ring (`ColonyState.commander_history: ArrayVec<HistoryToken, 8>`) is *read* by `colony_rich_observation` but nothing appends to it. The rollout loop needs to push `(state, action, reward)` after each commander cycle so the next cycle's observation carries history. `ArrayVec::push` panics when full, so we evict the oldest first.

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs` (add method after `apply_commander_intent`, near line 586)
- Test: `crates/antcolony-sim/src/simulation.rs` (inline `#[cfg(test)] mod` — add a test fn; if no inline test module exists near the method, add the test to `crates/antcolony-sim/tests/phase1_plumbing.rs` instead)

- [ ] **Step 1: Write the failing test**

Add to `crates/antcolony-sim/tests/phase1_plumbing.rs`:

```rust
#[test]
fn push_commander_history_appends_and_evicts_oldest_at_capacity() {
    use antcolony_sim::ai::observation::HistoryToken;
    let mut sim = build_two_colony_sim(); // existing helper in this test file

    // Empty to start.
    assert_eq!(sim.colony_rich_observation(0).unwrap().history.len(), 0);

    // Push 10 tokens with a monotonically increasing reward marker.
    for i in 0..10u32 {
        sim.push_commander_history(0, [0.0; 17], [0.0; 6], i as f32);
    }

    let hist = sim.colony_rich_observation(0).unwrap().history;
    // Capacity is 8 — ring holds the last 8 (rewards 2..=9), oldest evicted.
    assert_eq!(hist.len(), 8);
    assert_eq!(hist.first().unwrap().reward, 2.0);
    assert_eq!(hist.last().unwrap().reward, 9.0);
    // Pad is zero-filled and FLAT_LEN-consistent.
    let _ = HistoryToken::FLAT_LEN;
    assert_eq!(hist.last().unwrap().pad, [0.0; 72]);

    // Unknown colony is a silent no-op (mirrors apply_ant_modulators).
    sim.push_commander_history(99, [0.0; 17], [0.0; 6], 1.0);
}
```

If `build_two_colony_sim` does not already exist in `phase1_plumbing.rs`, add this helper to that file:

```rust
fn build_two_colony_sim() -> antcolony_sim::Simulation {
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p antcolony-sim --test phase1_plumbing push_commander_history_appends_and_evicts_oldest_at_capacity`
Expected: FAIL to compile — `no method named push_commander_history found for struct Simulation`.

- [ ] **Step 3: Implement the method**

In `crates/antcolony-sim/src/simulation.rs`, immediately after the `apply_commander_intent` method (ends near line ~600), add:

```rust
    /// Append a commander history token to a colony's ring buffer, evicting
    /// the oldest entry when the ring is at capacity (8). Read back via
    /// `colony_rich_observation(..).history`. Unknown `colony_id` is a
    /// silent no-op (mirrors `apply_ant_modulators`) so stale id batches
    /// from the trainer never panic. The `pad` field is zero-filled — it is
    /// reserved space in the 96-d history token (see design spec §"Open
    /// questions / History token contents").
    pub fn push_commander_history(
        &mut self,
        colony_id: u8,
        state: [f32; 17],
        action: [f32; 6],
        reward: f32,
    ) {
        use crate::ai::observation::HistoryToken;
        let Some(colony) = self.colonies.iter_mut().find(|c| c.id == colony_id) else {
            return;
        };
        let token = HistoryToken { state, action, reward, pad: [0.0; 72] };
        if colony.commander_history.is_full() {
            colony.commander_history.remove(0);
        }
        colony.commander_history.push(token);
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p antcolony-sim --test phase1_plumbing push_commander_history_appends_and_evicts_oldest_at_capacity`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-sim/src/simulation.rs crates/antcolony-sim/tests/phase1_plumbing.rs
git commit -m "sim: push_commander_history ring append with FIFO eviction"
```

---

## Task 2: `JointPpoConfig` + `smoke_default()`

**Files:**
- Create: `crates/antcolony-trainer/src/joint_ppo.rs`
- Modify: `crates/antcolony-trainer/src/lib.rs`

- [ ] **Step 1: Register the module + write the failing test**

Add to `crates/antcolony-trainer/src/lib.rs` after the `pub mod hierarchical;` line (line 26):

```rust
pub mod joint_ppo;
```

And after the existing `pub use` block (after line 34), add:

```rust
pub use joint_ppo::{JointPpoConfig, JointPpoTrainer, JointRollout, JointLossStats};
```

Create `crates/antcolony-trainer/src/joint_ppo.rs` with just the config + a test:

```rust
//! Joint PPO trainer for the hierarchical (commander + ant) brain.
//!
//! Self-plays both colonies of a `MatchEnv` with one shared
//! `HierarchicalActorCritic`, collects a two-buffer rollout (commander
//! @ cycle cadence, ant @ tick cadence), computes per-tier GAE at the
//! colony level, and optimizes `L_total = L_cmdr + α_balance · L_ant`
//! with a single AdamW step. Phase 2b-2 scope: single-device (CPU f32)
//! smoke. See docs/superpowers/plans/2026-05-29-ant-brain-phase2b2-joint-trainer.md.

#[derive(Clone, Debug)]
pub struct JointPpoConfig {
    pub iterations: usize,
    pub matches_per_iter: usize,
    /// Hard cap on commander decision cycles collected per match. Keeps
    /// the smoke rollout short (one cycle = DECISION_CADENCE outer ticks).
    pub rollout_cycles: usize,
    pub gamma: f32,
    pub gae_lambda: f32,
    pub clip: f32,
    pub epochs_per_batch: usize,
    pub lr: f64,
    pub value_coef: f64,
    pub cmdr_entropy_coef: f64,
    pub ant_entropy_coef: f64,
    /// Down-weights the ant-tier loss so ~10× more ant samples per cycle
    /// don't drown the commander gradient. Design spec §"Joint PPO loss".
    pub alpha_balance: f64,
    pub seed: u64,
}

impl JointPpoConfig {
    /// Minimal config for the mechanical smoke: 5 iters, 2 matches/iter,
    /// 8 cycles/match. Tuned for "runs without NaN/crash in under a
    /// minute on CPU", not for convergence.
    pub fn smoke_default() -> Self {
        Self {
            iterations: 5,
            matches_per_iter: 2,
            rollout_cycles: 8,
            gamma: 0.99,
            gae_lambda: 0.95,
            clip: 0.2,
            epochs_per_batch: 1,
            lr: 3e-4,
            value_coef: 0.5,
            cmdr_entropy_coef: 0.005,
            ant_entropy_coef: 0.01,
            alpha_balance: 0.1,
            seed: 0xa17_c01_2b2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_default_has_five_iters_and_balanced_coefs() {
        let c = JointPpoConfig::smoke_default();
        assert_eq!(c.iterations, 5);
        assert_eq!(c.matches_per_iter, 2);
        assert_eq!(c.rollout_cycles, 8);
        assert!(c.alpha_balance > 0.0 && c.alpha_balance <= 1.0);
        assert!(c.ant_entropy_coef >= c.cmdr_entropy_coef);
    }
}
```

- [ ] **Step 2: Run test to verify it fails, then compiles green**

Run: `cargo test -p antcolony-trainer --lib joint_ppo::tests::smoke_default_has_five_iters_and_balanced_coefs`
Expected: PASS (this task only adds the config; the test is self-contained). If it fails to compile, fix the `lib.rs` re-export (the later types `JointPpoTrainer`, `JointRollout`, `JointLossStats` don't exist yet — temporarily reduce the re-export to `pub use joint_ppo::JointPpoConfig;` and restore the full list in Task 8).

> **Note:** Because `lib.rs` re-exports types created in later tasks, either (a) add types as you go and keep the re-export minimal until Task 8, or (b) stub-declare them. Cleanest: in this task, set the re-export to `pub use joint_ppo::JointPpoConfig;` only, and widen it in Task 8 Step 1.

- [ ] **Step 3: Commit**

```bash
git add crates/antcolony-trainer/src/joint_ppo.rs crates/antcolony-trainer/src/lib.rs
git commit -m "joint: JointPpoConfig + smoke_default"
```

---

## Task 3: `JointPpoTrainer::new` + `make_optimizer`

**Files:**
- Modify: `crates/antcolony-trainer/src/joint_ppo.rs`

- [ ] **Step 1: Write the failing test**

Add the imports at the top of `joint_ppo.rs` (below the module doc comment):

```rust
use crate::hierarchical::sizing::Sizing;
use crate::HierarchicalActorCritic;
use candle_core::{DType, Device};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
```

Add the struct + impl:

```rust
pub struct JointPpoTrainer {
    pub hac: HierarchicalActorCritic,
    pub varmap: VarMap,
    pub device: Device,
    pub config: JointPpoConfig,
    pub rng: rand_chacha::ChaCha8Rng,
}

impl JointPpoTrainer {
    pub fn new(device: Device, sizing: Sizing, config: JointPpoConfig) -> anyhow::Result<Self> {
        use rand::SeedableRng;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, sizing)?;
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(config.seed);
        Ok(Self { hac, varmap, device, config, rng })
    }

    /// AdamW over every VarMap parameter (both tiers).
    pub fn make_optimizer(&self) -> anyhow::Result<AdamW> {
        let params = ParamsAdamW {
            lr: self.config.lr,
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-8,
            weight_decay: 0.0,
        };
        Ok(AdamW::new(self.varmap.all_vars(), params)?)
    }
}
```

Add to the `#[cfg(test)] mod tests`:

```rust
    use crate::hierarchical::sizing::A1;
    use candle_core::Device;

    #[test]
    fn trainer_builds_at_a1_with_nonzero_params() {
        let t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let total: usize = t.varmap.all_vars().iter()
            .map(|v| v.dims().iter().product::<usize>()).sum();
        assert!(total > 1_000_000, "A1 HAC should have >1M params, got {}", total);
        let _opt = t.make_optimizer().unwrap();
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p antcolony-trainer --lib joint_ppo::tests::trainer_builds_at_a1_with_nonzero_params`
Expected: FAIL to compile (`JointPpoTrainer` newly added — should compile and pass once the struct is in). If a borrow/type error appears, fix imports.

- [ ] **Step 3: (implementation is in Step 1) — Run to verify pass**

Run: `cargo test -p antcolony-trainer --lib joint_ppo::tests::trainer_builds_at_a1_with_nonzero_params`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/antcolony-trainer/src/joint_ppo.rs
git commit -m "joint: JointPpoTrainer::new + make_optimizer (A1, CPU f32)"
```

---

## Task 4: Rollout record + buffer types

**Files:**
- Modify: `crates/antcolony-trainer/src/joint_ppo.rs`

- [ ] **Step 1: Write the failing test**

Add `use candle_core::Tensor;` to the imports. Add the types:

```rust
/// One commander decision for one colony in one cycle of one match.
pub struct CommanderRecord {
    pub match_idx: usize,
    pub colony: u8,
    pub cycle: usize,
    pub state: Tensor,     // [1, 17]
    pub pheromone: Tensor, // [1, 4, 32, 32]
    pub history: Tensor,   // [1, 8, 96]
    pub action: Tensor,    // [1, 6] post-squash
    pub log_prob: f32,
    pub value: f32,
    pub reward: f32,
    pub done: bool,
}

/// One ant decision for one ant in one tick of one cycle.
pub struct AntRecord {
    pub match_idx: usize,
    pub colony: u8,
    pub cycle: usize,
    pub cone: Tensor,      // [1, 60]
    pub internal: Tensor,  // [1, 8]
    pub intent: Tensor,    // [1, 64]
    pub modulator: Tensor, // [1, 5] post-squash
    pub log_prob: f32,
    pub value: f32,
}

#[derive(Default)]
pub struct JointRollout {
    pub commander: Vec<CommanderRecord>,
    pub ant: Vec<AntRecord>,
}

/// Per-iteration loss breakdown for logging + the smoke assertion.
#[derive(Clone, Debug)]
pub struct JointLossStats {
    pub total: f32,
    pub commander: f32,
    pub ant: f32,
}
```

Add to tests:

```rust
    #[test]
    fn joint_rollout_defaults_empty() {
        let r = JointRollout::default();
        assert!(r.commander.is_empty());
        assert!(r.ant.is_empty());
    }
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p antcolony-trainer --lib joint_ppo::tests::joint_rollout_defaults_empty`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/antcolony-trainer/src/joint_ppo.rs
git commit -m "joint: rollout record + buffer types"
```

---

## Task 5: `JointPpoTrainer::rollout` — self-play one match into the two buffers

This is the load-bearing task. It drives one self-play match for up to `rollout_cycles` commander cycles, sampling both tiers, applying their outputs to the sim, collecting records, computing the r6 per-cycle reward, and appending history tokens.

**Files:**
- Modify: `crates/antcolony-trainer/src/joint_ppo.rs`

- [ ] **Step 1: Write the failing test**

Add to tests:

```rust
    #[test]
    fn rollout_fills_both_buffers_with_finite_records() {
        let mut t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let roll = t.rollout(0xfeed_1, 0).unwrap();

        assert!(!roll.commander.is_empty(), "commander buffer must be non-empty");
        assert!(!roll.ant.is_empty(), "ant buffer must be non-empty");

        // Commander: 2 colonies per cycle, all rows the right shape + finite.
        for r in &roll.commander {
            assert_eq!(r.state.dims(), &[1, 17]);
            assert_eq!(r.pheromone.dims(), &[1, 4, 32, 32]);
            assert_eq!(r.history.dims(), &[1, 8, 96]);
            assert_eq!(r.action.dims(), &[1, 6]);
            assert!(r.log_prob.is_finite() && r.value.is_finite() && r.reward.is_finite());
            assert!(r.colony == 0 || r.colony == 1);
        }
        // Ant rows finite + shaped.
        for a in &roll.ant {
            assert_eq!(a.cone.dims(), &[1, 60]);
            assert_eq!(a.internal.dims(), &[1, 8]);
            assert_eq!(a.intent.dims(), &[1, 64]);
            assert_eq!(a.modulator.dims(), &[1, 5]);
            assert!(a.log_prob.is_finite() && a.value.is_finite());
        }
        // Both colonies represented in the commander buffer.
        assert!(roll.commander.iter().any(|r| r.colony == 0));
        assert!(roll.commander.iter().any(|r| r.colony == 1));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p antcolony-trainer --lib joint_ppo::tests::rollout_fills_both_buffers_with_finite_records`
Expected: FAIL to compile — `no method named rollout`.

- [ ] **Step 3: Implement `rollout` + the free helpers**

Add these imports to the top of `joint_ppo.rs`:

```rust
use crate::env::{MatchEnv, DECISION_CADENCE};
use antcolony_sim::{AiDecision, MatchStatus};
```

Add the free helper functions (module level, below the `tests` module or above the impl):

```rust
fn colony_workers(env: &MatchEnv, k: u8) -> u32 {
    env.sim.colonies.get(k as usize).map(|c| c.population.workers).unwrap_or(0)
}
fn colony_food(env: &MatchEnv, k: u8) -> f32 {
    env.sim.colonies.get(k as usize).map(|c| c.food_stored).unwrap_or(0.0)
}
fn colony_queen_alive(env: &MatchEnv, k: u8) -> f32 {
    env.sim.colonies.get(k as usize)
        .map(|c| if c.queen_health > 0.0 { 1.0 } else { 0.0 })
        .unwrap_or(0.0)
}

/// Decode one row of a [B, 6] post-squash action tensor into an AiDecision.
fn row_to_decision(action_batch: &candle_core::Tensor, row: usize) -> anyhow::Result<AiDecision> {
    let r = action_batch.narrow(0, row, 1)?.squeeze(0)?; // [6]
    let v: Vec<f32> = r.to_vec1()?;
    Ok(AiDecision {
        caste_ratio_worker: v[0],
        caste_ratio_soldier: v[1],
        caste_ratio_breeder: v[2],
        forage_weight: v[3],
        dig_weight: v[4],
        nurse_weight: v[5],
        research_choice: None,
    })
}
```

Add the method inside `impl JointPpoTrainer`:

```rust
    /// Self-play one match for up to `rollout_cycles` commander cycles.
    /// Both colonies are driven by the shared HAC. Returns the two-buffer
    /// rollout. `match_idx` stamps every record so per-match GAE never
    /// bleeds across match boundaries in `joint_update`.
    pub fn rollout(&mut self, seed: u64, match_idx: usize) -> anyhow::Result<JointRollout> {
        let dev = self.device.clone();
        let mut env = MatchEnv::new(seed);
        let mut out = JointRollout::default();

        let mut prev_workers = [colony_workers(&env, 0), colony_workers(&env, 1)];
        let mut prev_food = [colony_food(&env, 0), colony_food(&env, 1)];

        for cycle in 0..self.config.rollout_cycles {
            // ── Commander decision (both colonies, batch leading dim 2) ──
            // If a colony has been eliminated, commander_obs_batch errors;
            // treat that as match end.
            let (state_b, pher_b, hist_b) = match env.commander_obs_batch(&dev) {
                Ok(t) => t,
                Err(_) => break,
            };
            let cmdr = self.hac.sample_commander(&state_b, &pher_b, &hist_b, &mut self.rng)?;
            let dec0 = row_to_decision(&cmdr.action, 0)?;
            let dec1 = row_to_decision(&cmdr.action, 1)?;
            env.sim.apply_ai_decision(0, &dec0);
            env.sim.apply_ai_decision(1, &dec1);
            env.apply_commander_intents(&cmdr.intent)?; // intent is [2, 64]

            // ── Outer tick loop with per-tick batched ant decisions ──
            let mut done = false;
            for _ in 0..DECISION_CADENCE {
                let (cone, internal, intent_b, index_map) =
                    env.all_ant_obs_batch(&cmdr.intent, &dev)?;
                if !index_map.is_empty() {
                    let ant = self.hac.sample_ant(&cone, &internal, &intent_b, &mut self.rng)?;
                    env.apply_ant_modulators_batched(&ant.modulator, &index_map)?;
                    let lp: Vec<f32> = ant.log_prob.to_vec1()?;
                    let val: Vec<f32> = ant.value.to_vec1()?;
                    for (i, &(cid, _aid)) in index_map.iter().enumerate() {
                        out.ant.push(AntRecord {
                            match_idx,
                            colony: cid,
                            cycle,
                            cone: cone.narrow(0, i, 1)?.detach(),
                            internal: internal.narrow(0, i, 1)?.detach(),
                            intent: intent_b.narrow(0, i, 1)?.detach(),
                            modulator: ant.modulator.narrow(0, i, 1)?.detach(),
                            log_prob: lp[i],
                            value: val[i],
                        });
                    }
                }
                env.sim.tick();
                if !matches!(env.sim.match_status(), MatchStatus::InProgress)
                    || env.sim.tick >= env.max_ticks
                {
                    done = true;
                    break;
                }
            }

            // ── Per-cycle r6 reward (mirrors MatchEnv::step) ──
            let workers_now = [colony_workers(&env, 0), colony_workers(&env, 1)];
            let food_now = [colony_food(&env, 0), colony_food(&env, 1)];
            let q = [colony_queen_alive(&env, 0), colony_queen_alive(&env, 1)];
            let dl = workers_now[0] as i32 - prev_workers[0] as i32;
            let dr = workers_now[1] as i32 - prev_workers[1] as i32;
            let dfl = food_now[0] - prev_food[0];
            let dfr = food_now[1] - prev_food[1];
            let mut reward_left = (dl as f32) * 0.01 - (dr as f32) * 0.01
                + dfl * 0.002 - dfr * 0.002
                + (q[0] - q[1]) * 0.005;
            let mut reward_right = -reward_left;
            if done {
                match env.sim.match_status() {
                    MatchStatus::Won { winner: 0, .. } => { reward_left += 1.0; reward_right -= 1.0; }
                    MatchStatus::Won { winner: 1, .. } => { reward_left -= 1.0; reward_right += 1.0; }
                    MatchStatus::InProgress => {
                        let total = (workers_now[0] + workers_now[1]).max(1) as f32;
                        let share = workers_now[0] as f32 / total;
                        reward_left += (share - 0.5) * 2.0;
                        reward_right += (0.5 - share) * 2.0;
                    }
                    _ => {}
                }
            }
            prev_workers = workers_now;
            prev_food = food_now;

            // ── Commander records (split the [2, ..] batch per colony) ──
            let cmdr_lp: Vec<f32> = cmdr.log_prob.to_vec1()?;
            let cmdr_val: Vec<f32> = cmdr.value.to_vec1()?;
            let rewards = [reward_left, reward_right];
            for k in 0..2usize {
                out.commander.push(CommanderRecord {
                    match_idx,
                    colony: k as u8,
                    cycle,
                    state: state_b.narrow(0, k, 1)?.detach(),
                    pheromone: pher_b.narrow(0, k, 1)?.detach(),
                    history: hist_b.narrow(0, k, 1)?.detach(),
                    action: cmdr.action.narrow(0, k, 1)?.detach(),
                    log_prob: cmdr_lp[k],
                    value: cmdr_val[k],
                    reward: rewards[k],
                    done,
                });
            }

            // ── Append history token to each colony's ring for next cycle ──
            let state_rows = state_b.to_vec2::<f32>()?;   // [2, 17]
            let action_rows = cmdr.action.to_vec2::<f32>()?; // [2, 6]
            for k in 0..2usize {
                let mut st = [0.0f32; 17];
                st.copy_from_slice(&state_rows[k]);
                let mut ac = [0.0f32; 6];
                ac.copy_from_slice(&action_rows[k]);
                env.sim.push_commander_history(k as u8, st, ac, rewards[k]);
            }

            if done {
                break;
            }
        }
        Ok(out)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p antcolony-trainer --lib joint_ppo::tests::rollout_fills_both_buffers_with_finite_records`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/joint_ppo.rs
git commit -m "joint: rollout — self-play both colonies into two buffers"
```

---

## Task 6: Per-tier GAE helpers

Commander GAE: bucket commander records by `(colony, match_idx)`, GAE each stream at cycle cadence using the commander value head. Ant GAE: per `(colony, match_idx)` build a per-cycle stream where reward/done come from the matching commander record and the bootstrap value is the **mean of that colony+cycle's ant values**; redistribute the resulting per-cycle advantage/return to every ant record in that cycle. Both reuse `PpoTrainer::compute_gae`.

**Files:**
- Modify: `crates/antcolony-trainer/src/joint_ppo.rs`

- [ ] **Step 1: Write the failing test**

Add `use std::collections::BTreeMap;` to imports. Add to tests:

```rust
    #[test]
    fn gae_helpers_align_to_records_and_are_finite() {
        let mut t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let roll = t.rollout(0xfeed_2, 0).unwrap();

        let (cadv, cret) = t.commander_advantages(&roll.commander);
        assert_eq!(cadv.len(), roll.commander.len());
        assert_eq!(cret.len(), roll.commander.len());
        assert!(cadv.iter().chain(cret.iter()).all(|x| x.is_finite()));

        let (aadv, aret) = t.ant_advantages(&roll.commander, &roll.ant);
        assert_eq!(aadv.len(), roll.ant.len());
        assert_eq!(aret.len(), roll.ant.len());
        assert!(aadv.iter().chain(aret.iter()).all(|x| x.is_finite()));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p antcolony-trainer --lib joint_ppo::tests::gae_helpers_align_to_records_and_are_finite`
Expected: FAIL to compile — `no method named commander_advantages`.

- [ ] **Step 3: Implement the helpers**

Inside `impl JointPpoTrainer`:

```rust
    /// Commander GAE per (colony, match). Output is index-aligned 1:1 with
    /// `recs`. Returns (advantages, returns).
    pub fn commander_advantages(&self, recs: &[CommanderRecord]) -> (Vec<f32>, Vec<f32>) {
        use crate::ppo::PpoTrainer;
        let mut adv = vec![0.0f32; recs.len()];
        let mut ret = vec![0.0f32; recs.len()];
        let keys: std::collections::BTreeSet<(usize, u8)> =
            recs.iter().map(|r| (r.match_idx, r.colony)).collect();
        for (m, colony) in keys {
            // Records for this stream, in push order (== cycle order).
            let idxs: Vec<usize> = recs.iter().enumerate()
                .filter(|(_, r)| r.match_idx == m && r.colony == colony)
                .map(|(i, _)| i).collect();
            let rewards: Vec<f32> = idxs.iter().map(|&i| recs[i].reward).collect();
            let values: Vec<f32> = idxs.iter().map(|&i| recs[i].value).collect();
            let dones: Vec<bool> = idxs.iter().map(|&i| recs[i].done).collect();
            let (a, r) = PpoTrainer::compute_gae(
                &rewards, &values, &dones, self.config.gamma, self.config.gae_lambda,
            );
            for (j, &i) in idxs.iter().enumerate() {
                adv[i] = a[j];
                ret[i] = r[j];
            }
        }
        (adv, ret)
    }

    /// Ant GAE per (colony, match) at cycle cadence. Reward/done come from
    /// the commander record for the same (match, colony, cycle); the
    /// bootstrap value is the mean of that cycle's ant value-head outputs.
    /// Each ant record inherits its cycle's advantage/return. Output is
    /// index-aligned 1:1 with `ant`.
    pub fn ant_advantages(
        &self,
        cmdr: &[CommanderRecord],
        ant: &[AntRecord],
    ) -> (Vec<f32>, Vec<f32>) {
        use crate::ppo::PpoTrainer;
        let mut adv = vec![0.0f32; ant.len()];
        let mut ret = vec![0.0f32; ant.len()];
        let keys: std::collections::BTreeSet<(usize, u8)> =
            ant.iter().map(|a| (a.match_idx, a.colony)).collect();
        for (m, colony) in keys {
            // reward/done per cycle from the commander records.
            let mut cyc_reward: BTreeMap<usize, f32> = BTreeMap::new();
            let mut cyc_done: BTreeMap<usize, bool> = BTreeMap::new();
            for r in cmdr.iter().filter(|r| r.match_idx == m && r.colony == colony) {
                cyc_reward.insert(r.cycle, r.reward);
                cyc_done.insert(r.cycle, r.done);
            }
            // mean ant value + ant indices per cycle.
            let mut cyc_val: BTreeMap<usize, (f32, usize)> = BTreeMap::new();
            let mut cyc_idxs: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for (i, a) in ant.iter().enumerate()
                .filter(|(_, a)| a.match_idx == m && a.colony == colony)
            {
                let e = cyc_val.entry(a.cycle).or_insert((0.0, 0));
                e.0 += a.value;
                e.1 += 1;
                cyc_idxs.entry(a.cycle).or_default().push(i);
            }
            // Ordered cycles that actually have ant samples.
            let cycles: Vec<usize> = cyc_idxs.keys().copied().collect();
            let rewards: Vec<f32> = cycles.iter()
                .map(|c| *cyc_reward.get(c).unwrap_or(&0.0)).collect();
            let values: Vec<f32> = cycles.iter()
                .map(|c| { let (s, n) = cyc_val[c]; if n > 0 { s / n as f32 } else { 0.0 } })
                .collect();
            let dones: Vec<bool> = cycles.iter()
                .map(|c| *cyc_done.get(c).unwrap_or(&false)).collect();
            let (a, r) = PpoTrainer::compute_gae(
                &rewards, &values, &dones, self.config.gamma, self.config.gae_lambda,
            );
            for (j, c) in cycles.iter().enumerate() {
                for &i in &cyc_idxs[c] {
                    adv[i] = a[j];
                    ret[i] = r[j];
                }
            }
        }
        (adv, ret)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p antcolony-trainer --lib joint_ppo::tests::gae_helpers_align_to_records_and_are_finite`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/joint_ppo.rs
git commit -m "joint: per-tier GAE (commander + ant, per colony/match)"
```

---

## Task 7: `joint_update` — joint loss + one Adam step

Mirrors `PpoTrainer::ppo_update` numerics for each tier, then combines `L_total = L_cmdr + α_balance · L_ant` and does one `backward_step`.

**Files:**
- Modify: `crates/antcolony-trainer/src/joint_ppo.rs`

- [ ] **Step 1: Write the failing test**

Add `use candle_core::Tensor;` (already added in Task 4) and add to tests a weight-movement check:

```rust
    fn first_var_flat(vm: &candle_nn::VarMap) -> Vec<f32> {
        let vars = vm.all_vars();
        vars[0].as_tensor().flatten_all().unwrap().to_vec1::<f32>().unwrap()
    }

    #[test]
    fn joint_update_returns_finite_loss_and_moves_weights() {
        let mut t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let mut opt = t.make_optimizer().unwrap();
        let roll = t.rollout(0xfeed_3, 0).unwrap();

        let before = first_var_flat(&t.varmap);
        let stats = t.joint_update(&mut opt, &roll).unwrap();
        let after = first_var_flat(&t.varmap);

        assert!(stats.total.is_finite(), "total loss must be finite: {}", stats.total);
        assert!(stats.commander.is_finite());
        assert!(stats.ant.is_finite());
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-9),
            "at least one parameter must change after an Adam step"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p antcolony-trainer --lib joint_ppo::tests::joint_update_returns_finite_loss_and_moves_weights`
Expected: FAIL to compile — `no method named joint_update`.

- [ ] **Step 3: Implement `joint_update` + the `normalize_adv` free helper**

Add the free helper (module level):

```rust
/// Standard PPO advantage normalization (zero mean, unit std).
fn normalize_adv(adv: &[f32]) -> Vec<f32> {
    let n = adv.len().max(1) as f32;
    let mean = adv.iter().sum::<f32>() / n;
    let var = adv.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
    let std = (var + 1e-8).sqrt();
    adv.iter().map(|x| (x - mean) / std).collect()
}
```

Inside `impl JointPpoTrainer`:

```rust
    /// One joint PPO update over the rollout. Returns the per-tier loss
    /// breakdown. Numerics per tier mirror `PpoTrainer::ppo_update`
    /// (clipped surrogate + value MSE + Gaussian entropy bonus). Runs
    /// `epochs_per_batch` passes; the smoke uses 1.
    pub fn joint_update(
        &self,
        opt: &mut AdamW,
        rollout: &JointRollout,
    ) -> anyhow::Result<JointLossStats> {
        use candle_core::{Tensor, D};
        let dev = self.device.clone();
        let two_pi = std::f64::consts::PI * 2.0;
        let ent_const = 0.5 * (1.0 + two_pi.ln());

        // ── advantages/returns (computed once, reused across epochs) ──
        let (cadv_raw, cret) = self.commander_advantages(&rollout.commander);
        let cadv = normalize_adv(&cadv_raw);
        let (aadv_raw, aret) = self.ant_advantages(&rollout.commander, &rollout.ant);
        let aadv = normalize_adv(&aadv_raw);

        let cn = rollout.commander.len();
        let an = rollout.ant.len();
        anyhow::ensure!(cn > 0, "joint_update: empty commander buffer");

        // ── pre-cat commander tensors (constant across epochs) ──
        let c_state = Tensor::cat(
            &rollout.commander.iter().map(|r| r.state.clone()).collect::<Vec<_>>(), 0)?;
        let c_pher = Tensor::cat(
            &rollout.commander.iter().map(|r| r.pheromone.clone()).collect::<Vec<_>>(), 0)?;
        let c_hist = Tensor::cat(
            &rollout.commander.iter().map(|r| r.history.clone()).collect::<Vec<_>>(), 0)?;
        let c_act = Tensor::cat(
            &rollout.commander.iter().map(|r| r.action.clone()).collect::<Vec<_>>(), 0)?;
        let c_oldlp = Tensor::from_slice(
            &rollout.commander.iter().map(|r| r.log_prob).collect::<Vec<_>>(), cn, &dev)?;
        let c_adv = Tensor::from_slice(&cadv, cn, &dev)?;
        let c_ret = Tensor::from_slice(&cret, cn, &dev)?;

        // ── pre-cat ant tensors (may be empty) ──
        let ant_tensors = if an > 0 {
            let cone = Tensor::cat(
                &rollout.ant.iter().map(|a| a.cone.clone()).collect::<Vec<_>>(), 0)?;
            let internal = Tensor::cat(
                &rollout.ant.iter().map(|a| a.internal.clone()).collect::<Vec<_>>(), 0)?;
            let intent = Tensor::cat(
                &rollout.ant.iter().map(|a| a.intent.clone()).collect::<Vec<_>>(), 0)?;
            let modulator = Tensor::cat(
                &rollout.ant.iter().map(|a| a.modulator.clone()).collect::<Vec<_>>(), 0)?;
            let oldlp = Tensor::from_slice(
                &rollout.ant.iter().map(|a| a.log_prob).collect::<Vec<_>>(), an, &dev)?;
            let adv = Tensor::from_slice(&aadv, an, &dev)?;
            let ret = Tensor::from_slice(&aret, an, &dev)?;
            Some((cone, internal, intent, modulator, oldlp, adv, ret))
        } else {
            None
        };

        let clip_lo = 1.0 - self.config.clip;
        let clip_hi = 1.0 + self.config.clip;

        let mut last = JointLossStats { total: 0.0, commander: 0.0, ant: 0.0 };
        for _epoch in 0..self.config.epochs_per_batch {
            // ── Commander loss ──
            let new_lp = self.hac.log_prob_of_commander_action(&c_state, &c_pher, &c_hist, &c_act)?;
            let value_pred = self.hac.forward_commander(&c_state, &c_pher, &c_hist)?.value;
            let ratio = (&new_lp - &c_oldlp)?.exp()?;
            let surr1 = (&ratio * &c_adv)?;
            let surr2 = (&ratio.clamp(clip_lo, clip_hi)? * &c_adv)?;
            let policy_loss = surr1.minimum(&surr2)?.mean_all()?.affine(-1.0, 0.0)?;
            let value_loss = (&value_pred - &c_ret)?.sqr()?.mean_all()?;
            let entropy = self.hac.commander.log_std.affine(1.0, ent_const)?.sum_all()?;
            let cmdr_total = ((&policy_loss + value_loss.affine(self.config.value_coef, 0.0)?)?
                - entropy.affine(self.config.cmdr_entropy_coef, 0.0)?)?;

            // ── Ant loss (optional) ──
            let (ant_total, ant_scalar) = if let Some((cone, internal, intent, modulator, oldlp, adv, ret)) = &ant_tensors {
                let new_lp = self.hac.log_prob_of_ant_modulator(cone, internal, intent, modulator)?;
                let value_pred = self.hac.forward_ant(cone, internal, intent)?.value;
                let ratio = (&new_lp - oldlp)?.exp()?;
                let surr1 = (&ratio * adv)?;
                let surr2 = (&ratio.clamp(clip_lo, clip_hi)? * adv)?;
                let policy_loss = surr1.minimum(&surr2)?.mean_all()?.affine(-1.0, 0.0)?;
                let value_loss = (&value_pred - ret)?.sqr()?.mean_all()?;
                let entropy = self.hac.ant.log_std.affine(1.0, ent_const)?.sum_all()?;
                let at = ((&policy_loss + value_loss.affine(self.config.value_coef, 0.0)?)?
                    - entropy.affine(self.config.ant_entropy_coef, 0.0)?)?;
                let scalar = at.to_scalar::<f32>()?;
                (Some(at), scalar)
            } else {
                (None, 0.0)
            };

            // ── Combine + step ──
            let total = match &ant_total {
                Some(at) => (&cmdr_total + at.affine(self.config.alpha_balance, 0.0)?)?,
                None => cmdr_total.clone(),
            };
            let total_scalar = total.to_scalar::<f32>()?;
            let cmdr_scalar = cmdr_total.to_scalar::<f32>()?;
            let _ = D::Minus1; // silence unused import on builds that don't need it
            opt.backward_step(&total)?;
            last = JointLossStats { total: total_scalar, commander: cmdr_scalar, ant: ant_scalar };
        }
        Ok(last)
    }
```

> **Note on `D::Minus1`:** the `use candle_core::{..., D}` import is only needed if a later edit references it; the `let _ = D::Minus1;` line keeps the import live. If clippy complains about an unused import instead, delete both the import and that line.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p antcolony-trainer --lib joint_ppo::tests::joint_update_returns_finite_loss_and_moves_weights`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/joint_ppo.rs
git commit -m "joint: joint_update — per-tier PPO loss + alpha-balanced Adam step"
```

---

## Task 8: `train` driver + end-to-end 5-iter smoke (integration test)

**Files:**
- Modify: `crates/antcolony-trainer/src/joint_ppo.rs`
- Modify: `crates/antcolony-trainer/src/lib.rs` (widen the re-export)
- Create: `crates/antcolony-trainer/tests/joint_ppo_smoke.rs`

- [ ] **Step 1: Implement `train` + widen re-export**

Inside `impl JointPpoTrainer`:

```rust
    /// Run the full smoke loop: for each iteration, collect
    /// `matches_per_iter` self-play rollouts into one buffer, then do one
    /// joint update. Returns per-iteration loss stats. Logs each iter via
    /// tracing.
    pub fn train(&mut self) -> anyhow::Result<Vec<JointLossStats>> {
        let mut opt = self.make_optimizer()?;
        let mut history = Vec::with_capacity(self.config.iterations);
        for it in 0..self.config.iterations {
            let mut roll = JointRollout::default();
            for m in 0..self.config.matches_per_iter {
                let seed = self.config.seed
                    ^ ((it as u64) << 32)
                    ^ ((m as u64).wrapping_mul(0x9E3779B97F4A7C15));
                let r = self.rollout(seed, m)?;
                roll.commander.extend(r.commander);
                roll.ant.extend(r.ant);
            }
            let stats = self.joint_update(&mut opt, &roll)?;
            tracing::info!(
                iter = it,
                total = stats.total,
                commander = stats.commander,
                ant = stats.ant,
                cmdr_records = roll.commander.len(),
                ant_records = roll.ant.len(),
                "joint ppo iteration"
            );
            history.push(stats);
        }
        Ok(history)
    }
```

In `crates/antcolony-trainer/src/lib.rs`, set the re-export (from Task 2) to the full list:

```rust
pub use joint_ppo::{JointPpoConfig, JointPpoTrainer, JointRollout, JointLossStats};
```

- [ ] **Step 2: Write the integration smoke test**

Create `crates/antcolony-trainer/tests/joint_ppo_smoke.rs`:

```rust
//! End-to-end Phase 2b-2 smoke: the joint trainer completes 5 iterations
//! on CPU f32 without NaN/crash and measurably moves both tiers' weights.

use antcolony_trainer::{JointPpoConfig, JointPpoTrainer};
use antcolony_trainer::hierarchical::sizing::A1;
use candle_core::Device;

fn flat(vm: &candle_nn::VarMap, name_contains: &str) -> Option<Vec<f32>> {
    let data = vm.data().lock().unwrap();
    for (name, var) in data.iter() {
        if name.contains(name_contains) {
            return Some(var.as_tensor().flatten_all().ok()?.to_vec1::<f32>().ok()?);
        }
    }
    None
}

#[test]
fn joint_ppo_smoke_five_iters_finite_and_moves_both_tiers() {
    let cfg = JointPpoConfig::smoke_default();
    let mut trainer = JointPpoTrainer::new(Device::Cpu, A1, cfg).unwrap();

    // Snapshot one commander param and one ant param before training.
    let cmdr_before = flat(&trainer.varmap, "commander").expect("commander var");
    let ant_before = flat(&trainer.varmap, "ant").expect("ant var");

    let stats = trainer.train().unwrap();

    assert_eq!(stats.len(), 5, "should run exactly 5 iterations");
    for (i, s) in stats.iter().enumerate() {
        assert!(s.total.is_finite(), "iter {} total loss NaN/inf: {}", i, s.total);
        assert!(s.commander.is_finite(), "iter {} commander loss NaN/inf", i);
        assert!(s.ant.is_finite(), "iter {} ant loss NaN/inf", i);
    }

    let cmdr_after = flat(&trainer.varmap, "commander").unwrap();
    let ant_after = flat(&trainer.varmap, "ant").unwrap();

    let cmdr_moved = cmdr_before.iter().zip(&cmdr_after).any(|(a, b)| (a - b).abs() > 1e-9);
    let ant_moved = ant_before.iter().zip(&ant_after).any(|(a, b)| (a - b).abs() > 1e-9);
    assert!(cmdr_moved, "commander tier weights should change after training");
    assert!(ant_moved, "ant tier weights should change after training");
}
```

> **Visibility note:** the test imports `antcolony_trainer::hierarchical::sizing::A1`. `hierarchical` is `pub mod` (lib.rs:26) and `sizing` is a public submodule, so `A1` is reachable. If the path resolves differently, fall back to the re-exported `antcolony_trainer::Sizing` and a local `A1` const — but prefer the direct path.

- [ ] **Step 3: Run the smoke test**

Run: `cargo test -p antcolony-trainer --test joint_ppo_smoke`
Expected: PASS — 5 iters, all losses finite, both tiers moved. (CPU f32; should complete in well under a minute.)

- [ ] **Step 4: Run the whole trainer suite to confirm no regressions**

Run: `cargo test -p antcolony-trainer`
Expected: all prior tests (26 from Phase 2b-1) + the new joint_ppo unit tests + the smoke test pass, 0 failures.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/joint_ppo.rs crates/antcolony-trainer/src/lib.rs crates/antcolony-trainer/tests/joint_ppo_smoke.rs
git commit -m "joint: train driver + 5-iter end-to-end smoke (CPU f32, A1)"
```

---

## Task 9: `joint_smoke` binary — the actual on-kokonoe run

The integration test validates mechanics; this binary is the runnable artifact for "5-iter smoke training run on kokonoe" with readable per-iteration logs.

**Files:**
- Create: `crates/antcolony-trainer/src/bin/joint_smoke.rs`

- [ ] **Step 1: Write the binary**

Create `crates/antcolony-trainer/src/bin/joint_smoke.rs`:

```rust
//! Phase 2b-2 joint-PPO smoke runner. Builds the A1 hierarchical brain,
//! runs `JointPpoConfig::smoke_default()` (5 iters, 2 matches/iter,
//! 8 cycles/match) on CPU f32, and logs per-iteration losses.
//!
//! Usage:
//!   cargo run --release --bin joint_smoke
//!
//! CUDA is intentionally NOT used: kokonoe has no MSVC linker so the
//! candle `cuda` feature does not build (see the Phase 2b-2 plan,
//! "Device & precision"). Multi-GPU fp16 is Phase 3 on the cnc P100s.

use antcolony_trainer::{JointPpoConfig, JointPpoTrainer};
use antcolony_trainer::hierarchical::sizing::A1;
use candle_core::Device;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_trainer=info,joint_smoke=info")
        .with_target(false)
        .init();

    let cfg = JointPpoConfig::smoke_default();
    tracing::info!(?cfg, "starting joint PPO smoke (CPU f32, A1)");

    let mut trainer = JointPpoTrainer::new(Device::Cpu, A1, cfg)?;
    let stats = trainer.train()?;

    for (i, s) in stats.iter().enumerate() {
        tracing::info!(iter = i, total = s.total, commander = s.commander, ant = s.ant, "iter summary");
    }
    let all_finite = stats.iter().all(|s| s.total.is_finite());
    tracing::info!(iters = stats.len(), all_finite, "joint PPO smoke complete");
    anyhow::ensure!(all_finite, "smoke produced a non-finite loss");
    Ok(())
}
```

- [ ] **Step 2: Build the binary**

Run: `cargo build -p antcolony-trainer --bin joint_smoke`
Expected: compiles clean.

> If `JointPpoConfig` does not derive enough to be used in `tracing::info!(?cfg, ...)`, it already derives `Debug` (Task 2) — `?cfg` works. If the `hierarchical::sizing` path is private from a bin target, switch the import to `use antcolony_trainer::Sizing;` and define `const A1` locally by copying the preset from `sizing.rs`; prefer the direct path if it compiles.

- [ ] **Step 3: Run the smoke on kokonoe**

Run: `RUST_LOG=info cargo run --release --bin joint_smoke`
Expected: 5 `iter summary` log lines with finite `total`/`commander`/`ant`, then `joint PPO smoke complete iters=5 all_finite=true`. Capture the output to `bench/joint-smoke-phase2b2.log` for the handoff:

Run: `cargo run --release --bin joint_smoke 2>&1 | tee bench/joint-smoke-phase2b2.log`

- [ ] **Step 4: Commit**

```bash
git add crates/antcolony-trainer/src/bin/joint_smoke.rs
git commit -m "joint: joint_smoke bin — runnable 5-iter CPU smoke"
```

---

## Task 10: Acceptance sweep + HANDOFF update

**Files:**
- Modify: `HANDOFF.md`

- [ ] **Step 1: Full workspace acceptance**

Run each and confirm green:

```bash
cargo build --workspace
cargo test -p antcolony-trainer
cargo test -p antcolony-sim --lib
cargo test -p antcolony-sim --test phase1_plumbing
```

Expected:
- Workspace builds.
- Trainer: all Phase-2b-1 tests + new joint_ppo unit tests + `joint_ppo_smoke` pass, 0 failures.
- Sim lib: 165 pass (was 164 + the new `push_commander_history` is a `--test` integration test, so lib count unchanged at 164; phase1_plumbing gains 1 → 14).
- phase1_plumbing: 14 pass.

- [ ] **Step 2: Confirm the on-box smoke ran**

Verify `bench/joint-smoke-phase2b2.log` exists and ends with `all_finite=true`.

- [ ] **Step 3: Prepend a new session entry to HANDOFF.md**

Add a `## Session 2026-05-29 — Phase 2b-2 joint PPO trainer + smoke landed` section at the top (below the title), following the existing entry format. Cover:
- 🟢 status, branch (`feat/ant-brain-phase2b2`), final commit SHA.
- What shipped: `JointPpoTrainer` (self-play two-buffer rollout, per-tier colony-level GAE, α-balanced joint loss, AdamW step), `Simulation::push_commander_history`, `joint_smoke` bin, end-to-end 5-iter CPU smoke green.
- The explicit smoke-scope simplifications (from this plan's "Smoke-scope simplifications" — they are deferred work, not done work: per-tick ant credit assignment, league opponents, fp16/CUDA, multi-GPU).
- What's next: Phase 3 — `parallel_env.rs` (N-env batched stepper) + `multi_gpu.rs` (RolloutTrainSplit) on cnc P100s; first real convergence run (Gates 1–2 from the design spec). The cnc CUDA build needs the MSVC/nvcc toolchain that kokonoe lacks — that's a cnc-side build, not local.
- Notes: CPU-f32 is the only local path; `cuda` feature is inert on kokonoe. Two-forward-per-tier-per-update and mean-ant-value GAE bootstrap are the first optimization/refinement targets.

- [ ] **Step 4: Commit**

```bash
git add HANDOFF.md
git commit -m "handoff: phase 2b-2 joint PPO trainer + smoke complete"
```

---

## Self-Review

**1. Spec coverage** (against the design spec §"Training loop" and the handoff's Phase 2b-2 definition: `JointPpoTrainer` + two-buffer rollout + per-tier GAE + joint loss + Adam update + 5-iter smoke):
- `JointPpoTrainer` struct → Task 3. ✓
- Two-buffer rollout (commander @ cycle, ant @ tick) → Task 5 (`JointRollout.commander` / `.ant`). ✓
- Per-tier GAE → Task 6 (`commander_advantages`, `ant_advantages` — two separate value functions). ✓
- Joint loss `L_total = L_cmdr + α_balance · L_ant` → Task 7. ✓
- Adam update → Task 7 (`opt.backward_step`), optimizer in Task 3. ✓
- 5-iter smoke on kokonoe → Task 8 (test) + Task 9 (bin). ✓
- `state_bias`/`deposit_mult`/intent already wired in 2a/2b-1 and exercised through `apply_*` in the rollout. ✓
- History ring append (required by the spec's per-tick data flow "Append … → HistoryToken in each colony's commander ring") → Task 1 + used in Task 5. ✓
- Deviations are listed in "Smoke-scope simplifications" and flagged for Phase 3 — no silent gaps.

**2. Placeholder scan:** No "TBD"/"handle edge cases"/"similar to Task N" — every code step is complete. The two "if the path resolves differently" notes (Task 8 Step 2, Task 9 Step 2) give a concrete fallback, not a placeholder.

**3. Type consistency:**
- `JointPpoConfig` field names (`rollout_cycles`, `cmdr_entropy_coef`, `ant_entropy_coef`, `alpha_balance`) used identically in Tasks 5/7/8/9. ✓
- `CommanderRecord`/`AntRecord` field names (`match_idx`, `colony`, `cycle`, `state`, `pheromone`, `history`, `action`, `cone`, `internal`, `intent`, `modulator`, `log_prob`, `value`, `reward`, `done`) consistent between Task 4 (def), Task 5 (construction), Task 6 (read), Task 7 (read). ✓
- `JointLossStats { total, commander, ant }` consistent Task 4/7/8/9. ✓
- HAC methods called with the signatures verified in source: `sample_commander(state, pheromone, history, rng)`, `sample_ant(cone, internal, intent, rng)`, `log_prob_of_commander_action(state, pheromone, history, action)`, `log_prob_of_ant_modulator(cone, internal, intent, modulator)`, `forward_commander(...).value`, `forward_ant(...).value`. ✓
- `MatchEnv` accessors called as defined: `commander_obs_batch(&dev)→(state,pher,hist)`, `all_ant_obs_batch(&intent,&dev)→(cone,internal,intent_b,index_map)`, `apply_commander_intents(&intent)`, `apply_ant_modulators_batched(&mods,&index_map)`. ✓
- `Simulation::push_commander_history(colony_id, [f32;17], [f32;6], f32)` — Task 1 def matches Task 5 call. ✓
- `PpoTrainer::compute_gae(rewards, values, dones, gamma, lambda)` reused unchanged. ✓

**4. Risks the implementer should watch:**
- `commander_obs_batch` errors if either colony is gone — handled by `break` in Task 5 (defensive).
- `log_std` is `pub(crate)` — reachable from `joint_ppo.rs` (same crate), confirmed.
- The smoke is CPU-only by design; do not add `--features cuda` on kokonoe (won't link).

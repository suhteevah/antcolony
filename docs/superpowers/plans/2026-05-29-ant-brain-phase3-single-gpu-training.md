# Ant Brain Phase 3 — Single-GPU Parallel-Env Training + Eval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Determine whether the hierarchical brain (HAC) beats the 47.1% Nash plateau, by training it **left-vs-league** on the RTX 3070 Ti with **parallel environments** and a **tunable reward shaper**, and evaluating it (deterministic) against the same **7-archetype bench** that measured MlpBrain v1 at 47.1%.

**Architecture:** The sim is CPU-only; only the HAC runs on CUDA. A `ParallelEnv` holds N `MatchEnv`s — each with the left colony driven by the HAC and the right by a league-sampled `AiBrain` — and **batches the left colony's observations across all active envs into single GPU forwards** (commander every 5 ticks, ants every tick), scattering outputs back. It emits the existing `JointRollout` (left-colony records only, bucketed by `env_idx`), so the Phase-2b-2 `joint_update` + per-tier GAE are reused **unchanged**. Reward is computed by a serde-tunable `RewardConfig` (defaults reproduce r6 exactly → apples-to-apples with the 47.1% baseline; opt-in "smartness" levers default to 0.0). A deterministic `evaluate_hac` plays the HAC vs the 7 archetypes for the headline win-rate. A `phase3_train` bin ties it together on CUDA with periodic eval + checkpoints.

**Tech Stack:** Rust (edition 2024, stable-msvc for the CUDA build via `scripts/build_trainer_cuda.bat`, stable-gnu for CPU), candle-core/candle-nn (CUDA on the 3070 Ti — see the Phase-2b-2 plan's "Device & precision" UPDATE), rand_chacha, the existing `antcolony-sim` + `antcolony-trainer` crates.

---

## Reuse map (what already exists — do NOT rebuild)

- `JointPpoTrainer { hac, varmap, device, config, rng }`, `JointPpoTrainer::{new, make_optimizer, joint_update, commander_advantages, ant_advantages}` — `crates/antcolony-trainer/src/joint_ppo.rs`. **`joint_update` and the GAE helpers are reused verbatim.** They bucket by `(match_idx, colony)`; Phase 3 sets `match_idx = env_idx` and `colony = 0` for every left record, so no GAE change is needed.
- Record types `CommanderRecord`, `AntRecord`, `JointRollout`, `JointLossStats` — same file.
- `HierarchicalActorCritic::{sample_commander, sample_ant, forward_commander, forward_ant}` and `pub(crate) fn squash_tanh_to_unit` — `crates/antcolony-trainer/src/hierarchical/actor_critic.rs`.
- `obs_to_tensors::{rich_to_tensors, rich_batch_to_tensors, ant_obs_to_tensors}` — `crates/antcolony-trainer/src/hierarchical/obs_to_tensors.rs`.
- `MatchEnv::{new, sim, max_ticks}`, `DECISION_CADENCE = 5` — `crates/antcolony-trainer/src/env.rs`.
- `League::{default_pool, make_brain}` (7 archetype specs: `heuristic, defender, aggressor, economist, breeder, forager, conservative`) — `crates/antcolony-trainer/src/league.rs`.
- `Simulation::{colony_rich_observation, per_ant_observations, apply_ai_decision, apply_commander_intent, apply_ant_modulators, colony_ai_state, match_status, tick}` and field `colonies` — `antcolony-sim`.
- `CandleBackend::new()` → CUDA device 0 with `--features cuda`, else CPU — `crates/antcolony-trainer/src/backend.rs`.
- `AiBrain::decide(&ColonyAiState) -> AiDecision`, `MatchStatus::{InProgress, Won{winner,..}, Draw{..}}`, `AntModulators { alpha_mult, beta_mult, exploration_mod, deposit_mult, state_bias }` — `antcolony-sim`.

`ColonyAiState` fields used for reward (verified in `obs_to_tensors::state_flatten`): `worker_count: u32`, `food_stored: f32`, `food_inflow_recent: f32`, `brood_egg/brood_larva/brood_pupa: u32`, `queens_alive: u32`, `combat_losses_recent: u32`.

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `crates/antcolony-trainer/src/reward.rs` | `RewardConfig` (serde, tunable) + `ColonyMetrics` snapshot + `compute_step_reward`. Default = exact r6; smartness levers default 0.0. | **Create** |
| `crates/antcolony-trainer/src/hierarchical/actor_critic.rs` | Add deterministic `mean_commander_action` + `mean_ant_modulator` (mean, no sampling) for eval. | **Modify** |
| `crates/antcolony-trainer/src/parallel_env.rs` | `ParallelEnv`: N envs (left=HAC / right=league), cross-env batched rollout → `JointRollout`. | **Create** |
| `crates/antcolony-trainer/src/eval.rs` | `EvalReport` + `evaluate_hac` (deterministic HAC vs 7 archetypes → per-archetype + mean win-rate). | **Create** |
| `crates/antcolony-trainer/src/phase3.rs` | `Phase3Config` + `run_phase3` driver: loop {parallel rollout → joint_update → periodic eval+checkpoint+log}. | **Create** |
| `crates/antcolony-trainer/src/bin/phase3_train.rs` | CLI: load reward TOML, config knobs, run on CUDA. | **Create** |
| `crates/antcolony-trainer/src/lib.rs` | module decls + re-exports. | **Modify** |
| `crates/antcolony-trainer/Cargo.toml` | add `toml` dep (reward config file parsing). | **Modify** |

---

## Task 1: `RewardConfig` + `ColonyMetrics` + `compute_step_reward`

**Files:**
- Create: `crates/antcolony-trainer/src/reward.rs`
- Modify: `crates/antcolony-trainer/src/lib.rs`

- [ ] **Step 1: Register module + re-export**

In `crates/antcolony-trainer/src/lib.rs`, after the existing `pub mod joint_ppo;` line add:

```rust
pub mod reward;
```

and after the joint_ppo re-export line add:

```rust
pub use reward::{ColonyMetrics, RewardConfig, compute_step_reward};
```

- [ ] **Step 2: Write the failing test (create the file with the test + types)**

Create `crates/antcolony-trainer/src/reward.rs`:

```rust
//! Tunable reward shaping for the hierarchical-brain trainer.
//!
//! `RewardConfig::default()` reproduces the established "r6" shaping
//! EXACTLY (the conditions under which MlpBrain v1 was measured at 47.1%),
//! so the headline Phase-3 run stays apples-to-apples. The extra
//! "smartness" levers (`brood_growth`, `food_inflow`, `combat_loss_penalty`)
//! default to 0.0 — set them > 0 to bias the colony toward those behaviors.
//! Reward is zero-sum between the two colonies for the shaping terms
//! (own minus enemy), plus a terminal win/timeout bonus.

use antcolony_sim::{MatchStatus, Simulation};
use serde::{Deserialize, Serialize};

/// Tunable reward weights. Defaults == r6.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RewardConfig {
    // ── r6 baseline terms (defaults reproduce the 47%-comparable shaping) ──
    /// Per-cycle worker-count delta weight (own minus enemy). r6 = 0.01.
    pub worker_delta: f32,
    /// Per-cycle food-stored delta weight (own minus enemy). r6 = 0.002.
    pub food_delta: f32,
    /// Queen-alive level bonus (own minus enemy, each 0/1). r6 = 0.005.
    pub queen_bonus: f32,
    /// Terminal win/loss magnitude (±). r6 = 1.0.
    pub terminal_win: f32,
    /// Timeout worker-share scale: reward += (share-0.5)*2*timeout_share. r6 = 1.0.
    pub timeout_share: f32,
    // ── opt-in "smartness" levers (default 0.0 = off → exact r6) ──
    /// Per-cycle brood (egg+larva+pupa) delta weight (own minus enemy).
    pub brood_growth: f32,
    /// Food-inflow level weight (own minus enemy) — rewards active foraging.
    pub food_inflow: f32,
    /// Combat-losses level penalty (own minus enemy) — subtracted.
    pub combat_loss_penalty: f32,
}

impl Default for RewardConfig {
    fn default() -> Self {
        Self {
            worker_delta: 0.01,
            food_delta: 0.002,
            queen_bonus: 0.005,
            terminal_win: 1.0,
            timeout_share: 1.0,
            brood_growth: 0.0,
            food_inflow: 0.0,
            combat_loss_penalty: 0.0,
        }
    }
}

/// Snapshot of one colony's reward-relevant metrics at a point in time.
#[derive(Clone, Copy, Debug, Default)]
pub struct ColonyMetrics {
    pub workers: f32,
    pub food: f32,
    pub queen_alive: f32,
    pub brood: f32,
    pub food_inflow: f32,
    pub combat_losses: f32,
}

impl ColonyMetrics {
    /// Read a colony's metrics from the sim. Returns all-zeros if the
    /// colony no longer exists (eliminated) — matches the "dead colony
    /// contributes nothing" convention.
    pub fn from_sim(sim: &Simulation, colony_id: u8) -> Self {
        match sim.colony_ai_state(colony_id) {
            Some(s) => Self {
                workers: s.worker_count as f32,
                food: s.food_stored,
                queen_alive: if s.queens_alive > 0 { 1.0 } else { 0.0 },
                brood: (s.brood_egg + s.brood_larva + s.brood_pupa) as f32,
                food_inflow: s.food_inflow_recent,
                combat_losses: s.combat_losses_recent as f32,
            },
            None => Self::default(),
        }
    }
}

/// Compute the per-cycle shaped reward for both colonies. `prev`/`cur` are
/// `[left, right]` metrics at the start vs end of the cycle. Shaping is
/// zero-sum (`reward_right = -reward_left` for the shaping terms); on `done`,
/// the terminal win/loss or timeout-share bonus is added per side.
pub fn compute_step_reward(
    cfg: &RewardConfig,
    prev: &[ColonyMetrics; 2],
    cur: &[ColonyMetrics; 2],
    done: bool,
    status: MatchStatus,
) -> (f32, f32) {
    let dwl = cur[0].workers - prev[0].workers;
    let dwr = cur[1].workers - prev[1].workers;
    let dfl = cur[0].food - prev[0].food;
    let dfr = cur[1].food - prev[1].food;
    let dbl = cur[0].brood - prev[0].brood;
    let dbr = cur[1].brood - prev[1].brood;

    let mut reward_left = cfg.worker_delta * (dwl - dwr)
        + cfg.food_delta * (dfl - dfr)
        + cfg.queen_bonus * (cur[0].queen_alive - cur[1].queen_alive)
        + cfg.brood_growth * (dbl - dbr)
        + cfg.food_inflow * (cur[0].food_inflow - cur[1].food_inflow)
        - cfg.combat_loss_penalty * (cur[0].combat_losses - cur[1].combat_losses);
    let mut reward_right = -reward_left;

    if done {
        match status {
            MatchStatus::Won { winner: 0, .. } => {
                reward_left += cfg.terminal_win;
                reward_right -= cfg.terminal_win;
            }
            MatchStatus::Won { winner: 1, .. } => {
                reward_left -= cfg.terminal_win;
                reward_right += cfg.terminal_win;
            }
            MatchStatus::InProgress => {
                // Timeout: graded by worker share, scaled to ±timeout_share.
                let total = (cur[0].workers + cur[1].workers).max(1.0);
                let share = cur[0].workers / total;
                reward_left += (share - 0.5) * 2.0 * cfg.timeout_share;
                reward_right += (0.5 - share) * 2.0 * cfg.timeout_share;
            }
            _ => {}
        }
    }
    (reward_left, reward_right)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(workers: f32, food: f32, queen: f32) -> ColonyMetrics {
        ColonyMetrics { workers, food, queen_alive: queen, ..Default::default() }
    }

    #[test]
    fn default_reproduces_r6_shaping_numbers() {
        let cfg = RewardConfig::default();
        // left +10 workers, right +0; left +100 food, right +0; both queens alive.
        let prev = [m(50.0, 200.0, 1.0), m(50.0, 200.0, 1.0)];
        let cur = [m(60.0, 300.0, 1.0), m(50.0, 200.0, 1.0)];
        let (l, r) = compute_step_reward(&cfg, &prev, &cur, false, MatchStatus::InProgress);
        // 10*0.01 + 100*0.002 + 0*0.005 = 0.1 + 0.2 = 0.3
        assert!((l - 0.3).abs() < 1e-6, "expected 0.3, got {l}");
        assert!((r + 0.3).abs() < 1e-6, "zero-sum, got {r}");
    }

    #[test]
    fn smartness_levers_off_by_default() {
        let cfg = RewardConfig::default();
        assert_eq!(cfg.brood_growth, 0.0);
        assert_eq!(cfg.food_inflow, 0.0);
        assert_eq!(cfg.combat_loss_penalty, 0.0);
        // Brood growth contributes nothing under defaults.
        let mut prev = [ColonyMetrics::default(); 2];
        let mut cur = [ColonyMetrics::default(); 2];
        prev[0].brood = 0.0; cur[0].brood = 100.0; // big brood swing
        let (l, _) = compute_step_reward(&cfg, &prev, &cur, false, MatchStatus::InProgress);
        assert_eq!(l, 0.0, "brood swing must not affect reward under defaults");
    }

    #[test]
    fn brood_growth_lever_rewards_brood_when_enabled() {
        let cfg = RewardConfig { brood_growth: 0.01, ..Default::default() };
        let mut prev = [ColonyMetrics::default(); 2];
        let mut cur = [ColonyMetrics::default(); 2];
        cur[0].brood = 100.0; // left grew 100 brood, right 0
        let (l, r) = compute_step_reward(&cfg, &prev, &cur, false, MatchStatus::InProgress);
        assert!((l - 1.0).abs() < 1e-6, "100*0.01 = 1.0, got {l}");
        assert!((r + 1.0).abs() < 1e-6);
    }

    #[test]
    fn terminal_win_adds_pm_one_by_default() {
        let cfg = RewardConfig::default();
        let prev = [ColonyMetrics::default(); 2];
        let cur = [ColonyMetrics::default(); 2];
        let (l, r) = compute_step_reward(&cfg, &prev, &cur, true,
            MatchStatus::Won { winner: 0, ended_at_tick: 100 });
        assert!((l - 1.0).abs() < 1e-6);
        assert!((r + 1.0).abs() < 1e-6);
    }
}
```

> **CHECK before writing:** confirm `MatchStatus::Won` field names by reading `crates/antcolony-sim/src/ai/brain.rs` (~line 1367). The plan assumes `Won { winner: u8, ended_at_tick: u64 }` and `Draw { ended_at_tick }`. If the field name differs, fix the test's `Won { .. }` construction. If `MatchStatus` does not derive `Copy`, take `status: &MatchStatus` and match on `*status`/refs.

- [ ] **Step 3: Run tests to verify they fail then pass**

Run: `cargo test -p antcolony-trainer --lib reward`
Expected: after creating the file, all 4 tests PASS. (They are self-contained — no failing-first stub needed beyond the module not existing.) If `MatchStatus` isn't `Copy`, adjust the signature per the CHECK note and re-run.

- [ ] **Step 4: Commit**

```bash
git add crates/antcolony-trainer/src/reward.rs crates/antcolony-trainer/src/lib.rs
git commit -m "trainer: tunable RewardConfig (default=r6) + compute_step_reward"
```

---

## Task 2: Deterministic HAC forward helpers (for eval)

Eval must run the HAC **deterministically** (policy mean, no sampling). Add mean-action helpers mirroring `ActorCritic::mean_action`.

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/actor_critic.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `actor_critic.rs`:

```rust
    #[test]
    fn mean_helpers_are_deterministic_and_squashed() {
        use crate::hierarchical::sizing::{
            A1, FIXED_CONE_D, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D, FIXED_INTENT_D,
            FIXED_INTERNAL_D, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W, FIXED_STATE_D,
        };
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();

        let state = Tensor::zeros((1, FIXED_STATE_D), DType::F32, &device).unwrap();
        let pher = Tensor::zeros((1, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W), DType::F32, &device).unwrap();
        let hist = Tensor::zeros((1, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D), DType::F32, &device).unwrap();

        let (a1, _i1, _v1) = hac.mean_commander_action(&state, &pher, &hist).unwrap();
        let (a2, _i2, _v2) = hac.mean_commander_action(&state, &pher, &hist).unwrap();
        assert_eq!(a1.dims(), &[1, 6]);
        let va1: Vec<f32> = a1.flatten_all().unwrap().to_vec1().unwrap();
        let va2: Vec<f32> = a2.flatten_all().unwrap().to_vec1().unwrap();
        assert_eq!(va1, va2, "mean action must be deterministic");
        assert!(va1.iter().all(|x| (0.0..=1.0).contains(x)), "squashed to [0,1]");

        let cone = Tensor::zeros((3, FIXED_CONE_D), DType::F32, &device).unwrap();
        let internal = Tensor::zeros((3, FIXED_INTERNAL_D), DType::F32, &device).unwrap();
        let intent = Tensor::zeros((3, FIXED_INTENT_D), DType::F32, &device).unwrap();
        let mods = hac.mean_ant_modulator(&cone, &internal, &intent).unwrap();
        assert_eq!(mods.dims(), &[3, 5]);
        assert!(mods.flatten_all().unwrap().to_vec1::<f32>().unwrap().iter().all(|x| (0.0..=1.0).contains(x)));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-trainer --lib mean_helpers_are_deterministic_and_squashed`
Expected: FAIL — no method `mean_commander_action`.

- [ ] **Step 3: Implement the helpers**

Add inside `impl HierarchicalActorCritic` (after `sample_ant`):

```rust
    /// Deterministic commander action (policy mean, no sampling) for eval.
    /// Returns (action[B,6] squashed to [0,1], intent[B,64], value[B]).
    pub fn mean_commander_action(
        &self,
        state: &Tensor,
        pheromone: &Tensor,
        history: &Tensor,
    ) -> Result<(Tensor, Tensor, Tensor)> {
        let fwd = self.commander.forward(state, pheromone, history)?;
        let action = squash_tanh_to_unit(&fwd.action)?;
        Ok((action, fwd.intent, fwd.value))
    }

    /// Deterministic ant modulator (policy mean, no sampling) for eval.
    /// Returns modulator[B,5] squashed to [0,1].
    pub fn mean_ant_modulator(
        &self,
        cone: &Tensor,
        internal: &Tensor,
        intent: &Tensor,
    ) -> Result<Tensor> {
        let fwd = self.ant.forward(cone, internal, intent)?;
        squash_tanh_to_unit(&fwd.modulator)
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-trainer --lib mean_helpers_are_deterministic_and_squashed`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/hierarchical/actor_critic.rs
git commit -m "hac: deterministic mean_commander_action + mean_ant_modulator for eval"
```

---

## Task 3: `ParallelEnv` struct + per-env setup + a shared modulator-decode helper

**Files:**
- Create: `crates/antcolony-trainer/src/parallel_env.rs`
- Modify: `crates/antcolony-trainer/src/lib.rs`

- [ ] **Step 1: Register module + re-export**

In `lib.rs` add `pub mod parallel_env;` (after `pub mod reward;`) and `pub use parallel_env::ParallelEnv;`.

- [ ] **Step 2: Write the failing test (create file with struct + new + test)**

Create `crates/antcolony-trainer/src/parallel_env.rs`:

```rust
//! Parallel-env rollout for single-GPU Phase-3 training.
//!
//! Holds N `MatchEnv`s, each with the left colony driven by the HAC and the
//! right by a league-sampled `AiBrain`. The sim steps on CPU; the left
//! colony's observations are batched across all *active* envs into single
//! GPU forwards (commander every DECISION_CADENCE ticks, ants every tick),
//! and outputs scattered back. Emits the existing `JointRollout` with
//! left-colony records only, `match_idx = env_idx`, `colony = 0`, so the
//! Phase-2b-2 `joint_update` + GAE are reused unchanged.

use anyhow::Result;
use candle_core::{Device, Tensor};
use rand_chacha::ChaCha8Rng;

use antcolony_sim::ai::observation::AntModulators;
use antcolony_sim::{AiBrain, AiDecision, MatchStatus};

use crate::env::{MatchEnv, DECISION_CADENCE};
use crate::hierarchical::obs_to_tensors::{ant_obs_to_tensors, rich_batch_to_tensors};
use crate::hierarchical::sizing::{FIXED_INTENT_D, FIXED_MODULATOR_D};
use crate::joint_ppo::{AntRecord, CommanderRecord, JointRollout};
use crate::reward::{compute_step_reward, ColonyMetrics, RewardConfig};
use crate::HierarchicalActorCritic;
use crate::League;

pub struct ParallelEnv {
    pub n_envs: usize,
    pub rollout_cycles: usize,
    pub league: League,
}

impl ParallelEnv {
    pub fn new(n_envs: usize, rollout_cycles: usize) -> Self {
        Self { n_envs, rollout_cycles, league: League::default_pool() }
    }
}

/// Decode one row of a [B, 6] squashed-action tensor into an AiDecision.
fn row_to_decision(action: &Tensor, row: usize) -> Result<AiDecision> {
    let v: Vec<f32> = action.narrow(0, row, 1)?.squeeze(0)?.to_vec1()?;
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

/// One row of a [B, 64] intent tensor into a fixed [f32; 64] array.
fn row_to_intent(intent: &Tensor, row: usize) -> Result<[f32; FIXED_INTENT_D]> {
    let v: Vec<f32> = intent.narrow(0, row, 1)?.flatten_all()?.to_vec1()?;
    let mut arr = [0.0f32; FIXED_INTENT_D];
    arr.copy_from_slice(&v);
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_env_constructs_with_default_league() {
        let pe = ParallelEnv::new(4, 8);
        assert_eq!(pe.n_envs, 4);
        assert_eq!(pe.rollout_cycles, 8);
        assert_eq!(pe.league.entries.len(), 7, "default pool = 7 archetypes");
    }
}
```

- [ ] **Step 3: Run to verify it compiles + the test passes**

Run: `cargo test -p antcolony-trainer --lib parallel_env::tests::parallel_env_constructs_with_default_league`
Expected: PASS. Some imports (`AntRecord`, `compute_step_reward`, etc.) are unused until Task 4 — if the compiler warns `unused import`, that is acceptable for this task (they're consumed in Task 4). Do not delete them.

> If `AntRecord`/`CommanderRecord`/`JointRollout` are not importable from `crate::joint_ppo`, confirm they're declared `pub` in `joint_ppo.rs` (they are, per Phase 2b-2). `League` is re-exported at the crate root (`crate::League`).

- [ ] **Step 4: Commit**

```bash
git add crates/antcolony-trainer/src/parallel_env.rs crates/antcolony-trainer/src/lib.rs
git commit -m "trainer: ParallelEnv skeleton + row decode helpers"
```

---

## Task 4: `ParallelEnv::collect_rollout` — the batched left-vs-league rollout

The load-bearing task. Drives N envs for `rollout_cycles` commander cycles; left = HAC (batched GPU forwards across active envs), right = league brain (CPU); reward via `RewardConfig`; emits a `JointRollout`.

**Files:**
- Modify: `crates/antcolony-trainer/src/parallel_env.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests`:

```rust
    use crate::hierarchical::sizing::A1;
    use crate::reward::RewardConfig;
    use candle_core::{DType, Device};
    use candle_nn::{VarBuilder, VarMap};
    use rand::SeedableRng;

    #[test]
    fn collect_rollout_fills_buffer_left_only_env_bucketed() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0xpa1);
        let reward = RewardConfig::default();

        let mut pe = ParallelEnv::new(3, 4);
        let roll = pe.collect_rollout(&hac, &device, &mut rng, &reward, 0xfeed).unwrap();

        assert!(!roll.commander.is_empty());
        assert!(!roll.ant.is_empty());
        // Left only.
        assert!(roll.commander.iter().all(|r| r.colony == 0));
        assert!(roll.ant.iter().all(|a| a.colony == 0));
        // match_idx is the env index, so it spans [0, n_envs).
        let envs_seen: std::collections::HashSet<usize> =
            roll.commander.iter().map(|r| r.match_idx).collect();
        assert!(envs_seen.iter().all(|&e| e < 3));
        assert!(envs_seen.len() >= 1);
        // Finite + shaped.
        for r in &roll.commander {
            assert_eq!(r.state.dims(), &[1, 17]);
            assert!(r.reward.is_finite() && r.value.is_finite() && r.log_prob.is_finite());
        }
        for a in &roll.ant {
            assert_eq!(a.modulator.dims(), &[1, 5]);
            assert!(a.value.is_finite() && a.log_prob.is_finite());
        }
    }
```

(Note: `0xpa1`/`0xfeed` — use valid hex; replace `0xpa1` with `0xa1` if needed.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-trainer --lib parallel_env::tests::collect_rollout_fills_buffer_left_only_env_bucketed`
Expected: FAIL — no method `collect_rollout`.

- [ ] **Step 3: Implement `collect_rollout`**

Add inside `impl ParallelEnv`:

```rust
    /// Collect one parallel rollout. Creates `n_envs` fresh matches (left =
    /// HAC, right = league-sampled opponent), runs up to `rollout_cycles`
    /// commander cycles, and returns left-colony records bucketed by env.
    /// `base_seed` decorrelates this rollout's env/opponent seeds.
    pub fn collect_rollout(
        &mut self,
        hac: &HierarchicalActorCritic,
        device: &Device,
        rng: &mut ChaCha8Rng,
        reward: &RewardConfig,
        base_seed: u64,
    ) -> Result<JointRollout> {
        let mut envs: Vec<MatchEnv> = Vec::with_capacity(self.n_envs);
        let mut opponents: Vec<Box<dyn AiBrain>> = Vec::with_capacity(self.n_envs);
        for i in 0..self.n_envs {
            let seed = base_seed ^ ((i as u64).wrapping_mul(0x9E3779B97F4A7C15));
            envs.push(MatchEnv::new(seed));
            // Round-robin the 7 archetypes across envs so every opponent
            // is represented; offset by base_seed so it varies per rollout.
            let pick = (i + (base_seed as usize)) % self.league.entries.len();
            let spec = self.league.entries[pick].spec.clone();
            opponents.push(League::make_brain(&spec, seed.wrapping_add(1)));
        }

        let mut out = JointRollout::default();
        let mut done = vec![false; self.n_envs];
        // prev metrics per env: [left, right]
        let mut prev: Vec<[ColonyMetrics; 2]> = (0..self.n_envs)
            .map(|i| [ColonyMetrics::from_sim(&envs[i].sim, 0), ColonyMetrics::from_sim(&envs[i].sim, 1)])
            .collect();

        for cycle in 0..self.rollout_cycles {
            // Active envs whose left colony still exists.
            let active: Vec<usize> = (0..self.n_envs)
                .filter(|&i| !done[i] && envs[i].sim.colony_rich_observation(0).is_some())
                .collect();
            if active.is_empty() {
                break;
            }

            // ── Commander forward, batched across active envs ──
            let riches: Vec<_> = active.iter()
                .map(|&i| envs[i].sim.colony_rich_observation(0).expect("active => Some"))
                .collect();
            let rich_refs: Vec<_> = riches.iter().collect();
            let (state_b, pher_b, hist_b) = rich_batch_to_tensors(&rich_refs, device)?;
            let cmdr = hac.sample_commander(&state_b, &pher_b, &hist_b, rng)?;
            let cmdr_lp: Vec<f32> = cmdr.log_prob.to_vec1()?;
            let cmdr_val: Vec<f32> = cmdr.value.to_vec1()?;

            // Scatter commander outputs + right-colony league decision.
            for (j, &i) in active.iter().enumerate() {
                let dec = row_to_decision(&cmdr.action, j)?;
                envs[i].sim.apply_ai_decision(0, &dec);
                let intent = row_to_intent(&cmdr.intent, j)?;
                envs[i].sim.apply_commander_intent(0, &intent);
                if let Some(sr) = envs[i].sim.colony_ai_state(1) {
                    let dr = opponents[i].decide(&sr);
                    envs[i].sim.apply_ai_decision(1, &dr);
                }
            }

            // ── Tick loop with per-tick batched ant decisions over active envs ──
            for _ in 0..DECISION_CADENCE {
                // Gather left-colony ant obs across active, still-alive envs.
                let mut cones: Vec<Tensor> = Vec::new();
                let mut internals: Vec<Tensor> = Vec::new();
                let mut intents: Vec<Tensor> = Vec::new();
                // index_map: (env_idx, ant_id) per row, in the same order.
                let mut index_map: Vec<(usize, u32)> = Vec::new();
                for (j, &i) in active.iter().enumerate() {
                    if done[i] {
                        continue;
                    }
                    let obs = envs[i].sim.per_ant_observations(0);
                    if obs.is_empty() {
                        continue;
                    }
                    // This env's commander intent (row j of the batch).
                    let intent_row = cmdr.intent.narrow(0, j, 1)?; // [1, 64]
                    let (cone, internal, intent_b) = ant_obs_to_tensors(&obs, &intent_row, device)?;
                    for o in &obs {
                        index_map.push((i, o.ant_id));
                    }
                    cones.push(cone);
                    internals.push(internal);
                    intents.push(intent_b);
                }
                if !index_map.is_empty() {
                    let cone = Tensor::cat(&cones, 0)?;
                    let internal = Tensor::cat(&internals, 0)?;
                    let intent = Tensor::cat(&intents, 0)?;
                    let ant = hac.sample_ant(&cone, &internal, &intent, rng)?;
                    let lp: Vec<f32> = ant.log_prob.to_vec1()?;
                    let val: Vec<f32> = ant.value.to_vec1()?;
                    // Scatter modulators back per env, grouped by env.
                    let mut row = 0usize;
                    for &i in &active {
                        if done[i] {
                            continue;
                        }
                        // Collect this env's contiguous rows from index_map.
                        let mut mods: Vec<AntModulators> = Vec::new();
                        let mut ids: Vec<u32> = Vec::new();
                        while row < index_map.len() && index_map[row].0 == i {
                            let m: Vec<f32> = ant.modulator.narrow(0, row, 1)?.flatten_all()?.to_vec1()?;
                            mods.push(AntModulators {
                                alpha_mult: m[0], beta_mult: m[1], exploration_mod: m[2],
                                deposit_mult: m[3], state_bias: m[4],
                            });
                            ids.push(index_map[row].1);
                            out.ant.push(AntRecord {
                                match_idx: i,
                                colony: 0,
                                cycle,
                                cone: cone.narrow(0, row, 1)?.detach(),
                                internal: internal.narrow(0, row, 1)?.detach(),
                                intent: intent.narrow(0, row, 1)?.detach(),
                                modulator: ant.modulator.narrow(0, row, 1)?.detach(),
                                log_prob: lp[row],
                                value: val[row],
                            });
                            row += 1;
                        }
                        if !ids.is_empty() {
                            envs[i].sim.apply_ant_modulators(0, &mods, &ids);
                        }
                    }
                    let _ = FIXED_MODULATOR_D; // layout anchor
                }
                // Step all active envs one tick; mark newly-done.
                for &i in &active {
                    if done[i] {
                        continue;
                    }
                    envs[i].sim.tick();
                    if !matches!(envs[i].sim.match_status(), MatchStatus::InProgress)
                        || envs[i].sim.tick >= envs[i].max_ticks
                    {
                        done[i] = true;
                    }
                }
            }

            // ── Per-cycle reward + commander records for active envs ──
            for (j, &i) in active.iter().enumerate() {
                let cur = [ColonyMetrics::from_sim(&envs[i].sim, 0), ColonyMetrics::from_sim(&envs[i].sim, 1)];
                let status = envs[i].sim.match_status();
                let (reward_left, _reward_right) =
                    compute_step_reward(reward, &prev[i], &cur, done[i], status);
                prev[i] = cur;
                out.commander.push(CommanderRecord {
                    match_idx: i,
                    colony: 0,
                    cycle,
                    state: state_b.narrow(0, j, 1)?.detach(),
                    pheromone: pher_b.narrow(0, j, 1)?.detach(),
                    history: hist_b.narrow(0, j, 1)?.detach(),
                    action: cmdr.action.narrow(0, j, 1)?.detach(),
                    log_prob: cmdr_lp[j],
                    value: cmdr_val[j],
                    reward: reward_left,
                    done: done[i],
                });
                // Append history token to the left colony's ring for next cycle.
                let st_row: Vec<f32> = state_b.narrow(0, j, 1)?.flatten_all()?.to_vec1()?;
                let ac_row: Vec<f32> = cmdr.action.narrow(0, j, 1)?.flatten_all()?.to_vec1()?;
                let mut st = [0.0f32; 17];
                st.copy_from_slice(&st_row);
                let mut ac = [0.0f32; 6];
                ac.copy_from_slice(&ac_row);
                envs[i].sim.push_commander_history(0, st, ac, reward_left);
            }
        }
        Ok(out)
    }
```

> **Implementation notes for the engineer:**
> - The ant scatter relies on `index_map` being grouped by env in `active` order (it is — we push all of env i's ants before moving to the next env). The `while index_map[row].0 == i` walk consumes exactly env i's contiguous block.
> - `done[i]` set mid-tick-loop means later ticks in the same cycle skip that env; the cycle's commander record still records the final reward with `done=true`.
> - Reward uses only `reward_left` (left = the trained colony). The right colony is the opponent; its reward isn't collected.
> - All stored tensors are `.detach()`-ed (no autograd graph retained in the buffer), exactly as Phase 2b-2 does.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-trainer --lib parallel_env::tests::collect_rollout_fills_buffer_left_only_env_bucketed`
Expected: PASS.

- [ ] **Step 5: Run the whole parallel_env + joint_ppo suite (no regressions)**

Run: `cargo test -p antcolony-trainer --lib parallel_env joint_ppo`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/antcolony-trainer/src/parallel_env.rs
git commit -m "trainer: ParallelEnv::collect_rollout — batched left-vs-league rollout"
```

---

## Task 5: `eval.rs` — deterministic HAC vs the 7-archetype bench

**Files:**
- Create: `crates/antcolony-trainer/src/eval.rs`
- Modify: `crates/antcolony-trainer/src/lib.rs`

- [ ] **Step 1: Register module + re-export**

In `lib.rs` add `pub mod eval;` and `pub use eval::{EvalReport, evaluate_hac};`.

- [ ] **Step 2: Write the failing test (create file with impl + test)**

Create `crates/antcolony-trainer/src/eval.rs`:

```rust
//! Deterministic evaluation of the HAC against the 7-archetype bench — the
//! same opponents and metric used to measure MlpBrain v1 at 47.1%. The HAC
//! plays the left colony with policy-MEAN actions (no sampling); each
//! archetype plays the right. Win = left win (1.0), loss (0.0), draw (0.5),
//! timeout graded by worker share.

use anyhow::Result;
use candle_core::{Device, Tensor};

use antcolony_sim::ai::observation::AntModulators;
use antcolony_sim::MatchStatus;

use crate::env::{MatchEnv, DECISION_CADENCE};
use crate::hierarchical::obs_to_tensors::{ant_obs_to_tensors, rich_to_tensors};
use crate::HierarchicalActorCritic;
use crate::League;

/// The 7 fixed archetype specs that define the bench (== League::default_pool).
pub const BENCH_ARCHETYPES: [&str; 7] = [
    "heuristic", "defender", "aggressor", "economist", "breeder", "forager", "conservative",
];

#[derive(Clone, Debug)]
pub struct EvalReport {
    /// (archetype name, win-rate in [0,1]) per opponent.
    pub per_archetype: Vec<(String, f32)>,
    /// Mean win-rate across all archetypes — the headline 47.1%-comparable number.
    pub mean_win_rate: f32,
}

/// Play one deterministic match: HAC (left, mean actions) vs `opp_spec`
/// (right). Returns the left score in [0,1] (1 win / 0.5 draw / 0 loss /
/// graded timeout).
fn play_match(
    hac: &HierarchicalActorCritic,
    device: &Device,
    opp_spec: &str,
    seed: u64,
) -> Result<f32> {
    let mut env = MatchEnv::new(seed);
    let mut opp = League::make_brain(opp_spec, seed.wrapping_add(1));

    loop {
        let rich = match env.sim.colony_rich_observation(0) {
            Some(r) => r,
            None => break, // left eliminated
        };
        let (s, p, h) = rich_to_tensors(&rich, device)?;
        let (action, intent, _value) = hac.mean_commander_action(&s, &p, &h)?;
        // Left decision.
        let av: Vec<f32> = action.flatten_all()?.to_vec1()?;
        let dec = antcolony_sim::AiDecision {
            caste_ratio_worker: av[0], caste_ratio_soldier: av[1], caste_ratio_breeder: av[2],
            forage_weight: av[3], dig_weight: av[4], nurse_weight: av[5], research_choice: None,
        };
        env.sim.apply_ai_decision(0, &dec);
        let iv: Vec<f32> = intent.flatten_all()?.to_vec1()?;
        let mut intent_arr = [0.0f32; 64];
        intent_arr.copy_from_slice(&iv);
        env.sim.apply_commander_intent(0, &intent_arr);
        // Right opponent decision.
        if let Some(sr) = env.sim.colony_ai_state(1) {
            let dr = opp.decide(&sr);
            env.sim.apply_ai_decision(1, &dr);
        }

        let mut done = false;
        for _ in 0..DECISION_CADENCE {
            let obs = env.sim.per_ant_observations(0);
            if !obs.is_empty() {
                let (cone, internal, intent_b) = ant_obs_to_tensors(&obs, &intent, device)?;
                let mods_t = hac.mean_ant_modulator(&cone, &internal, &intent_b)?;
                let flat: Vec<f32> = mods_t.flatten_all()?.to_vec1()?;
                let mut mods = Vec::with_capacity(obs.len());
                let mut ids = Vec::with_capacity(obs.len());
                for (k, o) in obs.iter().enumerate() {
                    let off = k * 5;
                    mods.push(AntModulators {
                        alpha_mult: flat[off], beta_mult: flat[off + 1], exploration_mod: flat[off + 2],
                        deposit_mult: flat[off + 3], state_bias: flat[off + 4],
                    });
                    ids.push(o.ant_id);
                }
                env.sim.apply_ant_modulators(0, &mods, &ids);
            }
            env.sim.tick();
            if !matches!(env.sim.match_status(), MatchStatus::InProgress)
                || env.sim.tick >= env.max_ticks
            {
                done = true;
                break;
            }
        }
        if done {
            break;
        }
    }

    Ok(match env.sim.match_status() {
        MatchStatus::Won { winner: 0, .. } => 1.0,
        MatchStatus::Won { winner: 1, .. } => 0.0,
        MatchStatus::Draw { .. } => 0.5,
        MatchStatus::InProgress => {
            // Timeout — grade by worker share.
            let lw = env.sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0) as f32;
            let rw = env.sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0) as f32;
            let share = lw / (lw + rw).max(1.0);
            if share > 0.5 { 1.0 } else if share < 0.5 { 0.0 } else { 0.5 }
        }
        _ => 0.5,
    })
}

/// Evaluate the HAC against all 7 archetypes, `matches_per_opp` each (fixed
/// seeds). Returns per-archetype + mean win-rate.
pub fn evaluate_hac(
    hac: &HierarchicalActorCritic,
    device: &Device,
    matches_per_opp: usize,
) -> Result<EvalReport> {
    let mut per_archetype = Vec::with_capacity(BENCH_ARCHETYPES.len());
    for spec in BENCH_ARCHETYPES {
        let mut score = 0.0f32;
        for m in 0..matches_per_opp {
            // Deterministic per (opp, match) seed so eval is reproducible.
            let seed = 0xE_VA1_u64.wrapping_mul(spec.len() as u64 + 1)
                ^ ((m as u64).wrapping_mul(0x9E3779B97F4A7C15));
            score += play_match(hac, device, spec, seed)?;
        }
        let wr = score / matches_per_opp.max(1) as f32;
        tracing::info!(archetype = spec, win_rate = wr, "eval vs archetype");
        per_archetype.push((spec.to_string(), wr));
    }
    let mean = per_archetype.iter().map(|(_, w)| *w).sum::<f32>()
        / per_archetype.len().max(1) as f32;
    Ok(EvalReport { per_archetype, mean_win_rate: mean })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchical::sizing::A1;
    use candle_core::{DType, Device};
    use candle_nn::{VarBuilder, VarMap};

    #[test]
    fn evaluate_hac_one_match_per_opp_produces_valid_rates() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
        // 1 match per opp keeps the test fast (full matches run to completion).
        let report = evaluate_hac(&hac, &device, 1).unwrap();
        assert_eq!(report.per_archetype.len(), 7);
        for (name, wr) in &report.per_archetype {
            assert!(!name.is_empty());
            assert!((0.0..=1.0).contains(wr), "{name} win-rate out of range: {wr}");
        }
        assert!((0.0..=1.0).contains(&report.mean_win_rate));
    }
}
```

> **CHECK before writing:** `0xE_VA1_u64` is NOT valid hex — replace the seed expression with a clean one, e.g. `let seed = 0xEVA1u64 ...` is also invalid; use `let seed = 0xE7A1_u64.wrapping_mul(...)`. Pick any valid hex literal. Also confirm `MatchStatus::Draw { .. }` and `Won { winner, .. }` field shapes against `brain.rs`.

- [ ] **Step 3: Run to verify it passes**

Run: `cargo test -p antcolony-trainer --lib eval::tests::evaluate_hac_one_match_per_opp_produces_valid_rates`
Expected: PASS. (7 full matches on CPU — may take 10-30s; be patient.)

- [ ] **Step 4: Commit**

```bash
git add crates/antcolony-trainer/src/eval.rs crates/antcolony-trainer/src/lib.rs
git commit -m "trainer: deterministic evaluate_hac vs 7-archetype bench"
```

---

## Task 6: `phase3.rs` — training driver (rollout → update → eval → checkpoint)

**Files:**
- Create: `crates/antcolony-trainer/src/phase3.rs`
- Modify: `crates/antcolony-trainer/src/lib.rs`

- [ ] **Step 1: Register module + re-export**

In `lib.rs` add `pub mod phase3;` and `pub use phase3::{Phase3Config, run_phase3, Phase3Report};`.

- [ ] **Step 2: Write the failing test (create file with impl + test)**

Create `crates/antcolony-trainer/src/phase3.rs`:

```rust
//! Phase-3 single-GPU training driver. Ties ParallelEnv (left-vs-league
//! rollout) + the Phase-2b-2 joint_update + the deterministic eval harness
//! into a training loop with periodic eval + checkpoints. Reuses
//! JointPpoTrainer for the HAC + varmap + joint_update.

use anyhow::Result;
use std::path::PathBuf;

use crate::eval::{evaluate_hac, EvalReport};
use crate::joint_ppo::{JointLossStats, JointPpoConfig, JointPpoTrainer};
use crate::parallel_env::ParallelEnv;
use crate::reward::RewardConfig;
use crate::hierarchical::sizing::{Sizing, A1};
use candle_core::Device;

#[derive(Clone, Debug)]
pub struct Phase3Config {
    pub iterations: usize,
    pub n_envs: usize,
    pub rollout_cycles: usize,
    pub eval_every: usize,
    pub matches_per_eval: usize,
    pub reward: RewardConfig,
    pub joint: JointPpoConfig,
    /// Directory for weight checkpoints (`varmap.save`).
    pub out_dir: PathBuf,
}

impl Phase3Config {
    /// Small config for the mechanical smoke (fast on CPU).
    pub fn smoke(out_dir: PathBuf) -> Self {
        Self {
            iterations: 2,
            n_envs: 4,
            rollout_cycles: 4,
            eval_every: 1,
            matches_per_eval: 1,
            reward: RewardConfig::default(),
            joint: JointPpoConfig::smoke_default(),
            out_dir,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Phase3Report {
    pub losses: Vec<JointLossStats>,
    /// (iteration, mean win-rate) at each eval point.
    pub evals: Vec<(usize, f32)>,
    pub final_eval: Option<EvalReport>,
}

/// Run Phase-3 training. `sizing` is the model preset (A1 for the first run).
pub fn run_phase3(device: Device, sizing: Sizing, cfg: Phase3Config) -> Result<Phase3Report> {
    std::fs::create_dir_all(&cfg.out_dir).ok();
    let mut trainer = JointPpoTrainer::new(device.clone(), sizing, cfg.joint.clone())?;
    let mut opt = trainer.make_optimizer()?;
    let mut pe = ParallelEnv::new(cfg.n_envs, cfg.rollout_cycles);

    let mut report = Phase3Report { losses: Vec::new(), evals: Vec::new(), final_eval: None };

    for it in 0..cfg.iterations {
        // Disjoint field borrows: &hac + &mut rng in one call is allowed.
        let base_seed = cfg.joint.seed ^ ((it as u64) << 40);
        let roll = pe.collect_rollout(
            &trainer.hac, &trainer.device, &mut trainer.rng, &cfg.reward, base_seed,
        )?;
        let stats = trainer.joint_update(&mut opt, &roll)?;
        tracing::info!(
            iter = it, total = stats.total, commander = stats.commander, ant = stats.ant,
            cmdr_records = roll.commander.len(), ant_records = roll.ant.len(),
            "phase3 iter"
        );
        report.losses.push(stats);

        if cfg.eval_every > 0 && it % cfg.eval_every == 0 {
            let ev = evaluate_hac(&trainer.hac, &trainer.device, cfg.matches_per_eval)?;
            tracing::info!(iter = it, mean_win_rate = ev.mean_win_rate, "phase3 eval");
            report.evals.push((it, ev.mean_win_rate));
            let ckpt = cfg.out_dir.join(format!("hac_iter{it:04}.safetensors"));
            if let Err(e) = trainer.varmap.save(&ckpt) {
                tracing::warn!(error = %e, path = %ckpt.display(), "checkpoint save failed");
            }
        }
    }

    // Final full eval + checkpoint.
    let final_ev = evaluate_hac(&trainer.hac, &trainer.device, cfg.matches_per_eval)?;
    tracing::info!(mean_win_rate = final_ev.mean_win_rate, "phase3 final eval");
    let _ = trainer.varmap.save(cfg.out_dir.join("hac_final.safetensors"));
    report.final_eval = Some(final_ev);

    let _ = A1; // A1 is the intended first-run preset; referenced by the bin.
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchical::sizing::A1;
    use candle_core::Device;

    #[test]
    fn phase3_smoke_runs_and_evals() {
        let tmp = std::env::temp_dir().join("antcolony_phase3_smoke");
        let cfg = Phase3Config::smoke(tmp);
        let report = run_phase3(Device::Cpu, A1, cfg).unwrap();
        assert_eq!(report.losses.len(), 2);
        for s in &report.losses {
            assert!(s.total.is_finite(), "loss must be finite: {}", s.total);
        }
        assert!(!report.evals.is_empty(), "should have at least one eval");
        let fe = report.final_eval.expect("final eval");
        assert!((0.0..=1.0).contains(&fe.mean_win_rate));
    }
}
```

> **CHECK before writing:** confirm `candle_nn::VarMap::save(&self, path)` exists in candle 0.10 (it does — saves safetensors). If the signature differs (e.g., `save<P: AsRef<Path>>`), adjust the call. If `VarMap` has no `save`, fall back to the existing `PpoTrainer::export_mlp_weights` pattern is NOT applicable (different shape) — instead serialize via `candle_core::safetensors::save(&varmap.data()...)`; but `VarMap::save` is the expected API.

- [ ] **Step 3: Run to verify it passes**

Run: `cargo test -p antcolony-trainer --lib phase3::tests::phase3_smoke_runs_and_evals`
Expected: PASS. (2 iters × 4 envs + 2 evals × 7 matches on CPU — may take ~1 min; be patient, do not kill early.)

- [ ] **Step 4: Commit**

```bash
git add crates/antcolony-trainer/src/phase3.rs crates/antcolony-trainer/src/lib.rs
git commit -m "trainer: run_phase3 driver — rollout + joint_update + eval + checkpoint"
```

---

## Task 7: `phase3_train` bin (CUDA run on kokonoe) + `toml` reward config

**Files:**
- Create: `crates/antcolony-trainer/src/bin/phase3_train.rs`
- Modify: `crates/antcolony-trainer/Cargo.toml`

- [ ] **Step 1: Add the `toml` dependency**

In `crates/antcolony-trainer/Cargo.toml`, under `[dependencies]`, add:

```toml
toml = "0.8"
```

(Confirm `toml` 0.8 resolves in the workspace lockfile; if the workspace pins a different version via `[workspace.dependencies]`, use `toml.workspace = true` instead and add it to the root `[workspace.dependencies]`.)

- [ ] **Step 2: Write the binary**

Create `crates/antcolony-trainer/src/bin/phase3_train.rs`:

```rust
//! Phase-3 single-GPU training runner. Trains the A1 hierarchical brain
//! left-vs-league on CUDA (kokonoe 3070 Ti), evaluating vs the 7-archetype
//! bench, with a tunable reward shaper loaded from a TOML file.
//!
//! Build/run (CUDA): see scripts/build_trainer_cuda.bat. Example:
//!   cargo +stable-x86_64-pc-windows-msvc run --release --features cuda \
//!     --bin phase3_train -- --iters 200 --envs 64 --eval-every 25 \
//!     --reward assets/reward/default.toml --out bench/phase3-a1
//!
//! The reward TOML mirrors RewardConfig fields; omitted fields take r6
//! defaults. To "thumb up smartness", set e.g. brood_growth / food_inflow.

use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::{Backend, CandleBackend, Phase3Config, RewardConfig, run_phase3};
use antcolony_trainer::JointPpoConfig;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_trainer=info,phase3_train=info")
        .with_target(false)
        .init();

    // Minimal flag parsing (no clap dep): --key value pairs.
    let mut iters = 200usize;
    let mut envs = 64usize;
    let mut rollout_cycles = 32usize;
    let mut eval_every = 25usize;
    let mut matches_per_eval = 50usize;
    let mut reward_path: Option<PathBuf> = None;
    let mut out_dir = PathBuf::from("bench/phase3-a1");

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let next = || args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--iters" => { iters = next().parse().unwrap_or(iters); i += 2; }
            "--envs" => { envs = next().parse().unwrap_or(envs); i += 2; }
            "--rollout-cycles" => { rollout_cycles = next().parse().unwrap_or(rollout_cycles); i += 2; }
            "--eval-every" => { eval_every = next().parse().unwrap_or(eval_every); i += 2; }
            "--matches-per-eval" => { matches_per_eval = next().parse().unwrap_or(matches_per_eval); i += 2; }
            "--reward" => { reward_path = Some(PathBuf::from(next())); i += 2; }
            "--out" => { out_dir = PathBuf::from(next()); i += 2; }
            other => { tracing::warn!(arg = other, "unknown flag, ignoring"); i += 1; }
        }
    }

    let reward = match &reward_path {
        Some(p) => {
            let txt = std::fs::read_to_string(p)?;
            let r: RewardConfig = toml::from_str(&txt)?;
            tracing::info!(path = %p.display(), ?r, "loaded reward config");
            r
        }
        None => {
            tracing::info!("no --reward file; using r6 defaults");
            RewardConfig::default()
        }
    };

    let backend = CandleBackend::new()?;
    let device = backend.device().clone();
    tracing::info!(cuda = backend.cuda_available(), iters, envs, rollout_cycles, "phase3 start (A1)");

    let cfg = Phase3Config {
        iterations: iters,
        n_envs: envs,
        rollout_cycles,
        eval_every,
        matches_per_eval,
        reward,
        joint: JointPpoConfig::smoke_default(),
        out_dir,
    };

    let report = run_phase3(device, A1, cfg)?;

    tracing::info!("=== Phase 3 win-rate curve (iter, mean) ===");
    for (it, wr) in &report.evals {
        tracing::info!(iter = it, mean_win_rate = wr, "curve");
    }
    if let Some(fe) = &report.final_eval {
        tracing::info!(mean_win_rate = fe.mean_win_rate, baseline = 0.471, "FINAL vs 47.1% baseline");
        for (name, wr) in &fe.per_archetype {
            tracing::info!(archetype = name, win_rate = wr, "final per-archetype");
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Add a default reward TOML**

Create `assets/reward/default.toml`:

```toml
# r6 baseline (reproduces the 47.1%-comparison conditions). Edit the
# smartness levers below to bias colony behavior, then retrain + eval.
worker_delta = 0.01
food_delta = 0.002
queen_bonus = 0.005
terminal_win = 1.0
timeout_share = 1.0
# Smartness levers (0.0 = off). Thumb these up to reward "smart" behavior:
brood_growth = 0.0          # reward egg->larva->pupa pipeline growth
food_inflow = 0.0           # reward active foraging (food inflow rate)
combat_loss_penalty = 0.0   # penalize losing workers in combat
```

- [ ] **Step 4: Build (CPU first, then confirm it parses args)**

Run (CPU, fast sanity build): `cargo build -p antcolony-trainer --bin phase3_train`
Expected: compiles clean.

Then a tiny CPU dry-run to confirm wiring (small iters/envs):
Run: `cargo run -p antcolony-trainer --bin phase3_train -- --iters 1 --envs 2 --rollout-cycles 2 --eval-every 1 --matches-per-eval 1 --out bench/phase3-smoke`
Expected: exits 0, logs `phase3 start (cuda=false ...)`, one `phase3 iter`, one `phase3 eval`, a `FINAL vs 47.1% baseline` line, and writes checkpoints to `bench/phase3-smoke/`.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/bin/phase3_train.rs crates/antcolony-trainer/Cargo.toml assets/reward/default.toml
git commit -m "trainer: phase3_train bin + tunable reward TOML"
```

---

## Task 8: CUDA run on kokonoe + acceptance + HANDOFF

**Files:**
- Modify: `HANDOFF.md`
- (no source changes)

- [ ] **Step 1: Full acceptance on CPU**

Run:
```bash
cargo test -p antcolony-trainer
cargo build --workspace
```
Expected: all trainer tests pass (Phase 2b-1 + 2b-2 + reward + parallel_env + eval + phase3 smoke), workspace builds.

- [ ] **Step 2: Build + run the real training on the GPU**

Build the CUDA binary via the established recipe (a new bin needs the cuda feature build). Add `phase3_train` to the run path — run:

```bash
# From a clean shell (so lld-link is on PATH); the script handles vcvars.
cmd /c "J:\antcolony\scripts\build_trainer_cuda.bat"
```

Then run the real Phase-3 training on the 3070 Ti (start modest, tune up):
```bash
# In a BuildTools-vcvars shell with LLVM on PATH (see run_joint_smoke_cuda.bat for the pattern):
cargo +stable-x86_64-pc-windows-msvc run --release --features cuda --bin phase3_train -- \
  --iters 200 --envs 64 --rollout-cycles 32 --eval-every 25 --matches-per-eval 50 \
  --reward assets/reward/default.toml --out bench/phase3-a1 2>&1 | tee bench/phase3-a1.log
```
Expected: `cuda=true`, win-rate curve logged every 25 iters, a `FINAL vs 47.1% baseline` line, checkpoints in `bench/phase3-a1/`. **Gate 1:** runs 200 iters, no NaN. **Gate 2:** win-rate trends up from iter-0 (random-init) baseline.

> Tuning: if VRAM is tight, lower `--envs`. If CPU sim throughput dominates wall-clock, lower `--rollout-cycles` or `--envs`. If eval is too slow during training, lower `--matches-per-eval` (keep 50 for the final number). These are the tunable knobs Matt asked for.

- [ ] **Step 3: Record the result + HANDOFF entry**

Prepend a `## Session <date> — Phase 3 single-GPU training landed` entry to `HANDOFF.md` covering: what shipped (RewardConfig, ParallelEnv, eval, run_phase3, phase3_train bin), the A1 win-rate-vs-iters result (did it approach/beat 47.1%?), the reward-shaping levers and how to use them, gate outcomes, and what's next (A2 if A1 shows signal; multi-GPU on cnc only if single-GPU throughput limits A3; smartness-lever experiments). Be honest about whether the brain beat the plateau — report the actual final win-rate, not an aspiration.

- [ ] **Step 4: Commit**

```bash
git add HANDOFF.md bench/phase3-a1.log
git commit -m "handoff: phase 3 single-GPU training results"
```

---

## Self-Review

**1. Spec coverage** (against the approved Phase-3 design + Matt's reward-shaping addition):
- Parallel-env stepper (N envs, batched GPU forwards) → Task 3 + 4 (`ParallelEnv`). ✓
- Left-vs-league training → Task 4 (left=HAC, right=`League::make_brain`). ✓
- Tunable reward shaping (default=r6, opt-in smartness levers) → Task 1 (`RewardConfig`) + Task 7 (TOML + bin). ✓ (Matt's explicit ask.)
- Eval vs 7-archetype bench, deterministic, win-rate → Task 2 (mean helpers) + Task 5 (`evaluate_hac`). ✓
- Training driver + checkpoints + win-rate curve → Task 6 (`run_phase3`) + Task 7 (bin). ✓
- Single-GPU / CUDA on kokonoe → Task 7 (`CandleBackend`) + Task 8 (build/run recipe). ✓
- Reuse joint_update + GAE unchanged (env_idx as match_idx, colony 0) → Task 4 record construction. ✓
- Out of scope (multi-GPU, A2/A3, distillation, self-snapshots) → not present. ✓
- Gates 1/2 → Task 8 run criteria. ✓

**2. Placeholder scan:** Two intentional `CHECK before writing` notes (MatchStatus field shapes; the eval seed hex literal `0xE_VA1`/`0xEVA1` are INVALID hex and MUST be replaced with a valid literal like `0xE7A1_u64` — flagged explicitly so the implementer fixes them, not a silent placeholder). The `VarMap::save` CHECK gives a concrete fallback. No "TBD"/"add error handling"/"similar to Task N".

**3. Type consistency:**
- `RewardConfig` field names identical across Task 1 (def), Task 7 (TOML keys + load). ✓
- `ColonyMetrics` fields consistent Task 1 ↔ used in Task 4. ✓
- `compute_step_reward(cfg, &[ColonyMetrics;2], &[ColonyMetrics;2], bool, MatchStatus)` — same call shape in Task 4. ✓
- `CommanderRecord`/`AntRecord` fields (`match_idx, colony, cycle, state, pheromone, history, action, log_prob, value, reward, done` / `... cone, internal, intent, modulator ...`) match the Phase-2b-2 definitions used by `joint_update`. ✓
- `mean_commander_action -> (Tensor, Tensor, Tensor)`, `mean_ant_modulator -> Tensor` — same usage in Task 5. ✓
- `Phase3Config` fields consistent Task 6 ↔ Task 7. ✓
- `evaluate_hac(hac, device, matches_per_opp)` / `EvalReport { per_archetype, mean_win_rate }` — consistent Task 5 ↔ Task 6 ↔ Task 7. ✓

**4. Watch-items for the implementer:**
- The eval/47%-comparison is apples-to-apples ONLY with `RewardConfig::default()` (r6). Smartness-lever runs are separate experiments — the bench (fixed archetypes) stays a valid metric, but don't compare a shaped-reward run's win-rate to 47.1% as if it were the same experiment.
- `ParallelEnv::collect_rollout` relies on `index_map` being grouped by env in `active` order — verify the scatter walk if you refactor the ant gather.
- The first run is A1 (~12M). If A1 plateaus below 47%, that's signal capacity matters → try A2 (still fits 8 GB) before concluding the plateau is environmental.

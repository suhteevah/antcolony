# Ladder League Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A self-driving training loop that warm-starts from the current SOTA, trains a best-response against the FROZEN tournament-ladder pool (terminal reward), gates each candidate against a standing bar, promotes only tournament-validated winners into the pool, and stops + declares the ceiling after K no-improve rounds.

**Architecture:** New module `ladder_league.rs` orchestrates rounds. Each round reuses the existing `JointPpoTrainer` + `ParallelEnv::collect_rollout` (self-play machinery, but the pool is read-only — never seeded with the main's own in-progress snapshots, which is what drifted SP1/SP2). A cheap one-row gate (`evaluate_h2h` + `evaluate_hac`) compares the candidate against a frozen standing bar; promotions trigger a full `run_tournament` re-rank with rollback if the cheap gate disagrees.

**Tech Stack:** Rust (edition 2024), candle-core/candle-nn (CUDA on cnc P100 for training, CPU for gate), `antcolony-sim`, `antcolony-trainer`. A1 hierarchical net. `rand_chacha::ChaCha8Rng`.

## Global Constraints

- **No `.unwrap()` in non-test code** — `Result` + `anyhow`/`?`. (project rule)
- **Verbose `tracing`** — every round/gate/promotion logs structured fields; no `println!`. (project rule)
- **Additive only** — `phase3.rs`, `self_play.rs`, `exploiter_league.rs`, `parallel_env.rs`, `eval.rs`, `tournament.rs` stay byte-unchanged. New code lives in `ladder_league.rs` + its bin. (spec)
- **Determinism / RNG hygiene** — orchestration seeds derive ONLY from `cfg.joint.seed`, the round index, and a distinct agent/opponent index — NEVER from any agent's training RNG (the SP1 critical-bug fix). Opponent sampling already uses a separate `opp_rng` inside `collect_rollout`.
- **A1 sizing**, `assets/reward/terminal.toml` reward, MSRV/toolchain per repo (`stable` on cnc, edition 2024 needs `r#gen` for `Rng::gen`).
- **Frozen-pool invariant** — within a round, no pool entry is ADDED or REMOVED (the opponent *set* is immutable). PFSP EMA/`games` updates via `record_result` are allowed and expected (that's how PFSP targets lost matchups). "Frozen" = immutable opponent set, not immutable stats.
- **Approved defaults:** `gate_margin=0.55`, `iters_per_round=150`, pool=`{sota,sp1term,sp1,gradclip,sp2}`+7 archetypes (drop `v1`), `keepbest_arch_floor=0.70`, `no_improve_stop=2`, `max_rounds=8`, `gate_mpe=50`, `archetype_mix=0.30`, `pfsp_power=1.0`.

---

## File Structure

- **Create** `crates/antcolony-trainer/src/ladder_league.rs` — all orchestration logic: `LadderConfig`, `LadderReport`, `RoundOutcome`, `GateOutcome`; pure helpers (`gate_decision`, `should_stop`, `round_seed`); `build_frozen_pool`, `winrate_vs_pool`, `gate`, `train_round`, `LadderLeague::{new,run}`.
- **Create** `crates/antcolony-trainer/src/bin/ladder_league.rs` — hand-rolled CLI (modeled on `bin/phase3_league.rs`), builds device/config, calls `LadderLeague::run`.
- **Create** `scripts/run_ladder_league_cnc.sh` — cnc run wrapper (full-fleet-kick optional, RAYON, CUDA libs, EXIT-trap restore — modeled on `run_league_cnc.sh`).
- **Modify** `crates/antcolony-trainer/src/lib.rs` — add `pub mod ladder_league;` + re-export the public types.
- **Test** — `#[cfg(test)]` unit tests inside `ladder_league.rs` (pure helpers + pool builder + frozen invariant + winrate/gate eval-driven small cases). One end-to-end smoke as a `#[test]` (CPU, tiny) inside the module (matches the crate's existing pattern of in-module smokes + `tests/` integration files).

### Reused interfaces (exact, from the current code)

```rust
// self_play.rs
pub fn load_frozen_hac(path: &Path, sizing: Sizing, device: &Device) -> Result<HierarchicalActorCritic>;
pub enum Role { Archetype, Main, MainExploiter, LeagueExploiter }
pub enum OpponentKind { Archetype(String), Snapshot { name: String, path: PathBuf } }
impl SnapshotPool {
    pub fn with_archetypes(pool_cap: usize, ema_alpha: f32) -> Self;       // 7 protected archetypes
    pub fn add_protected_snapshot(&mut self, name: impl Into<String>, path: impl Into<PathBuf>, role: Role);
    pub fn record_result(&mut self, idx: usize, hac_won: f32);
    pub entries: Vec<PoolEntry>;   // PoolEntry { kind, win_rate_ema, games, role, protected }
}
pub enum OpponentSampler { Uniform, Pfsp { archetype_mix: f32, power: f32 } }

// joint_ppo.rs
impl JointPpoTrainer {
    pub fn new(device: Device, sizing: Sizing, config: JointPpoConfig) -> Result<Self>;
    pub fn make_optimizer(&self) -> Result<AdamW>;
    pub fn joint_update(&self, opt: &mut AdamW, rollout: &JointRollout) -> Result<JointLossStats>;
    pub hac: HierarchicalActorCritic; pub varmap: VarMap; pub rng: ChaCha8Rng; pub config: JointPpoConfig;
}
pub struct JointPpoConfig { /* ...; */ pub seed: u64, pub rollout_cycles: usize, pub ant_chunk_size: usize, pub max_grad_norm: f64, /* ... */ }
impl JointPpoConfig { pub fn smoke_default() -> Self; }

// parallel_env.rs
impl ParallelEnv {
    pub fn new(n_envs: usize, rollout_cycles: usize) -> Self;
    pub fn collect_rollout(&mut self, hac: &HierarchicalActorCritic, device: &Device,
                           rng: &mut ChaCha8Rng, reward: &RewardConfig, base_seed: u64) -> Result<JointRollout>;
    pub self_play_enabled: bool; pub pool: SnapshotPool; pub sampler: OpponentSampler;
    pub sizing: Sizing; pub last_opponent_idx: usize; pub last_hac_winshare: f32;
}

// eval.rs
pub const BENCH_ARCHETYPES: [&str; 7] = ["heuristic","defender","aggressor","economist","breeder","forager","conservative"];
pub fn evaluate_hac(hac: &HierarchicalActorCritic, device: &Device, matches_per_opp: usize) -> Result<EvalReport>;
pub struct EvalReport { pub per_archetype: Vec<(String,f32)>, pub mean_win_rate: f32,
                        pub per_archetype_decisive: Vec<(String,f32)>, pub mean_decisive_rate: f32, pub outcomes: OutcomeCounts }
pub fn evaluate_h2h(a: &HierarchicalActorCritic, b: &HierarchicalActorCritic, device: &Device, mpe: usize) -> Result<H2HReport>;
pub struct H2HReport { pub a_winrate_ws: f32, pub a_winrate_decisive: f32, pub a_as_left_ws: f32, pub a_as_right_ws: f32, pub matches: usize }

// tournament.rs
pub fn run_tournament(cfg: &TournamentConfig, device: &Device) -> Result<TournamentResult>;
pub struct TournamentConfig { pub contenders: Vec<(String,String)>, pub mpe: usize, pub max_ticks: u64,
                              pub anchor_id: String, pub anchor_elo: f64, pub cycle_margin: f32, pub sizing: Sizing }
pub struct TournamentResult { pub ids: Vec<String>, /* ... */ pub elo: Vec<f64>, pub winrate_vs_field: Vec<f32>, pub cycles: Vec<(usize,usize,usize)> }

// reward.rs
pub struct RewardConfig { /* worker_delta, ..., terminal_win, timeout_share, ... */ }  // toml::from_str loads it
```

---

### Task 1: Config, report types, pure helpers, frozen-pool builder, lib wiring

**Files:**
- Create: `crates/antcolony-trainer/src/ladder_league.rs`
- Modify: `crates/antcolony-trainer/src/lib.rs`
- Test: in-module `#[cfg(test)]`

**Interfaces:**
- Consumes: `self_play::{SnapshotPool, Role, OpponentKind, OpponentSampler}`, `hierarchical::sizing::Sizing`, `joint_ppo::JointPpoConfig`, `reward::RewardConfig`.
- Produces:
  ```rust
  pub struct LadderContender { pub id: String, pub spec: String }  // spec: "hac:<path>" or archetype name
  pub struct LadderConfig {
      pub sota_path: PathBuf,
      pub initial_contenders: Vec<LadderContender>, // the frozen HAC brains (NOT archetypes; archetypes auto-added)
      pub iters_per_round: usize,
      pub eval_every: usize,
      pub train_mpe: usize,       // coarse mpe for in-round keep-best
      pub gate_mpe: usize,        // honest mpe for the gate (50)
      pub gate_margin: f32,       // 0.55
      pub keepbest_arch_floor: f32, // 0.70
      pub archetype_mix: f32,     // 0.30
      pub pfsp_power: f32,        // 1.0
      pub no_improve_stop: usize, // 2
      pub max_rounds: usize,      // 8
      pub out_dir: PathBuf,
      pub sizing: Sizing,
      pub joint: JointPpoConfig,
      pub reward: RewardConfig,
  }
  pub struct LadderReport {
      pub rounds_run: usize,
      pub promotions: usize,
      pub final_sota_path: PathBuf,
      pub best_h2h_over_seed: f32, // best gate h2h-vs-ORIGINAL-sota achieved
      pub stopped_reason: String,  // "no_improve" | "max_rounds"
  }
  pub fn round_seed(base: u64, round: usize, idx: usize) -> u64;       // base ^ ((round as u64)<<32) ^ ((idx as u64)<<16)
  pub fn should_stop(no_improve: usize, no_improve_stop: usize, round: usize, max_rounds: usize) -> Option<&'static str>;
  pub fn build_frozen_pool(contenders: &[LadderContender], ema_alpha: f32) -> SnapshotPool;
  ```

- [ ] **Step 1: Write failing tests for the pure helpers + pool builder**

```rust
// at bottom of crates/antcolony-trainer/src/ladder_league.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_play::OpponentKind;

    #[test]
    fn round_seed_is_deterministic_and_distinct() {
        let b = 0xABCD_1234;
        assert_eq!(round_seed(b, 1, 0), round_seed(b, 1, 0));      // reproducible
        assert_ne!(round_seed(b, 1, 0), round_seed(b, 2, 0));      // round varies
        assert_ne!(round_seed(b, 1, 0), round_seed(b, 1, 1));      // idx varies
        assert_eq!(round_seed(b, 1, 0), b ^ (1u64 << 32) ^ (0u64 << 16));
    }

    #[test]
    fn should_stop_fires_on_no_improve_then_max_rounds() {
        assert_eq!(should_stop(1, 2, 3, 8), None);                 // 1<2, 3<8 -> keep going
        assert_eq!(should_stop(2, 2, 3, 8), Some("no_improve"));   // hit no-improve cap
        assert_eq!(should_stop(0, 2, 8, 8), Some("max_rounds"));   // hit round cap
    }

    #[test]
    fn build_frozen_pool_has_archetypes_plus_protected_snapshots_all_protected() {
        let cs = vec![
            LadderContender { id: "sota".into(), spec: "hac:bench/x/sota.safetensors".into() },
            LadderContender { id: "sp1term".into(), spec: "hac:bench/y/sp1term.safetensors".into() },
        ];
        let pool = build_frozen_pool(&cs, 0.1);
        // 7 archetypes + 2 snapshots
        assert_eq!(pool.entries.len(), 9);
        assert!(pool.entries.iter().all(|e| e.protected), "every frozen-pool entry must be protected");
        assert_eq!(pool.entries.iter().filter(|e| matches!(e.kind, OpponentKind::Snapshot{..})).count(), 2);
        let names: Vec<&str> = pool.entries.iter().filter_map(|e| match &e.kind {
            OpponentKind::Snapshot { name, .. } => Some(name.as_str()), _ => None }).collect();
        assert!(names.contains(&"sota") && names.contains(&"sp1term"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-trainer ladder_league:: 2>&1 | tail -20`
Expected: FAIL — `ladder_league` module / symbols not found (won't compile).

- [ ] **Step 3: Write the module head, types, and helpers**

```rust
//! Ladder League: iterated best-response vs the FROZEN tournament-ladder pool.
//! Warm-start from SOTA, train PFSP against frozen opponents (terminal reward),
//! gate the candidate against a standing bar, promote only tournament-validated
//! winners, stop + declare the ceiling after K no-improve rounds.
//!
//! Additive — phase3/SP1/SP2 byte-unchanged. The ONE departure from SP1/SP2:
//! the pool is read-only within a round (no main-snapshot additions), which
//! removes the drift feedback loop those runs hit.

use std::path::PathBuf;

use anyhow::Result;

use crate::hierarchical::sizing::Sizing;
use crate::reward::RewardConfig;
use crate::self_play::{Role, SnapshotPool};
use crate::JointPpoConfig;

#[derive(Clone, Debug)]
pub struct LadderContender {
    pub id: String,
    pub spec: String, // "hac:<path>" frozen HAC; archetype names are auto-seeded by the pool
}

#[derive(Clone, Debug)]
pub struct LadderConfig {
    pub sota_path: PathBuf,
    pub initial_contenders: Vec<LadderContender>,
    pub iters_per_round: usize,
    pub eval_every: usize,
    pub train_mpe: usize,
    pub gate_mpe: usize,
    pub gate_margin: f32,
    pub keepbest_arch_floor: f32,
    pub archetype_mix: f32,
    pub pfsp_power: f32,
    pub no_improve_stop: usize,
    pub max_rounds: usize,
    pub out_dir: PathBuf,
    pub sizing: Sizing,
    pub joint: JointPpoConfig,
    pub reward: RewardConfig,
}

#[derive(Clone, Debug)]
pub struct LadderReport {
    pub rounds_run: usize,
    pub promotions: usize,
    pub final_sota_path: PathBuf,
    pub best_h2h_over_seed: f32,
    pub stopped_reason: String,
}

/// Orchestration seed — derived ONLY from the base seed, round, and a distinct
/// index. NEVER from any agent's training RNG (the SP1 critical-bug discipline).
pub fn round_seed(base: u64, round: usize, idx: usize) -> u64 {
    base ^ ((round as u64) << 32) ^ ((idx as u64) << 16)
}

/// Stop the loop after `no_improve_stop` consecutive no-promotion rounds, or at
/// the round cap. Returns the reason string, or None to keep going.
pub fn should_stop(no_improve: usize, no_improve_stop: usize, round: usize, max_rounds: usize) -> Option<&'static str> {
    if no_improve >= no_improve_stop {
        Some("no_improve")
    } else if round >= max_rounds {
        Some("max_rounds")
    } else {
        None
    }
}

/// Build the frozen opponent pool: the 7 archetypes (protected) + one protected
/// snapshot per HAC contender. All entries protected => the opponent SET never
/// changes during a round (PFSP EMA still updates, which is intended).
pub fn build_frozen_pool(contenders: &[LadderContender], ema_alpha: f32) -> SnapshotPool {
    // pool_cap is irrelevant here (all entries protected, never evicted); use a
    // generous value so it never trips even after several promotions.
    let mut pool = SnapshotPool::with_archetypes(1024, ema_alpha);
    for c in contenders {
        if let Some(path) = c.spec.strip_prefix("hac:") {
            tracing::info!(id = %c.id, %path, "ladder: seeding frozen HAC opponent");
            pool.add_protected_snapshot(c.id.clone(), path.to_string(), Role::Main);
        } else {
            tracing::warn!(id = %c.id, spec = %c.spec, "ladder: non-hac contender ignored (archetypes are auto-seeded)");
        }
    }
    pool
}
```

- [ ] **Step 4: Wire the module into lib.rs**

In `crates/antcolony-trainer/src/lib.rs`, add the module declaration near the others and re-export:

```rust
pub mod ladder_league;
pub use ladder_league::{LadderConfig, LadderContender, LadderReport};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p antcolony-trainer ladder_league:: 2>&1 | tail -20`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/antcolony-trainer/src/ladder_league.rs crates/antcolony-trainer/src/lib.rs
git commit -m "feat(ladder): config, report types, pure helpers, frozen-pool builder"
```

---

### Task 2: `winrate_vs_pool` — a brain's mean winrate vs the frozen pool (self-excluded)

**Files:**
- Modify: `crates/antcolony-trainer/src/ladder_league.rs`
- Test: in-module `#[cfg(test)]`

**Interfaces:**
- Consumes: `eval::{evaluate_hac, evaluate_h2h}`, `self_play::{load_frozen_hac, OpponentKind}`, `HierarchicalActorCritic`.
- Produces:
  ```rust
  pub struct PoolScore { pub winrate_vs_pool: f32, pub h2h_vs_sota: f32, pub per_opp: Vec<(String, f32)> }
  pub fn winrate_vs_pool(
      hac: &HierarchicalActorCritic, pool: &SnapshotPool, exclude_name: Option<&str>,
      sota_name: &str, device: &candle_core::Device, mpe: usize,
  ) -> Result<PoolScore>;
  ```
  Mean is over per-opponent winrates (each archetype and each HAC snapshot counts as one opponent), skipping `exclude_name`. `h2h_vs_sota` is the worker-share winrate vs the entry named `sota_name`.

- [ ] **Step 1: Write the failing test (CPU smoke — fresh nets vs a 1-archetype + 1-snapshot pool)**

```rust
#[test]
fn winrate_vs_pool_excludes_self_and_reports_h2h() {
    use candle_core::Device;
    use crate::hierarchical::sizing::A1;
    use crate::self_play::Role;
    let dir = std::env::temp_dir().join("ladder_wvp_test");
    std::fs::create_dir_all(&dir).unwrap();
    // Save two fresh HAC checkpoints to act as "sota" and "other".
    let sota_p = dir.join("sota.safetensors");
    let other_p = dir.join("other.safetensors");
    for p in [&sota_p, &other_p] {
        let t = crate::JointPpoTrainer::new(Device::Cpu, A1, crate::JointPpoConfig::smoke_default()).unwrap();
        t.varmap.save(p).unwrap();
    }
    let cs = vec![
        LadderContender { id: "sota".into(), spec: format!("hac:{}", sota_p.display()) },
        LadderContender { id: "other".into(), spec: format!("hac:{}", other_p.display()) },
    ];
    let pool = build_frozen_pool(&cs, 0.1);
    let cand = crate::self_play::load_frozen_hac(&sota_p, A1, &Device::Cpu).unwrap();
    // Evaluate "sota" itself vs pool, excluding its own entry: must NOT include a self-match.
    let score = winrate_vs_pool(&cand, &pool, Some("sota"), "sota", &Device::Cpu, 1).unwrap();
    // 7 archetypes + "other" = 8 opponents (self "sota" excluded).
    assert_eq!(score.per_opp.len(), 8, "self entry must be excluded");
    assert!(!score.per_opp.iter().any(|(n, _)| n == "sota"), "self not scored");
    assert!((0.0..=1.0).contains(&score.winrate_vs_pool));
    assert!((0.0..=1.0).contains(&score.h2h_vs_sota));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-trainer winrate_vs_pool_excludes_self -- --nocapture 2>&1 | tail -20`
Expected: FAIL — `winrate_vs_pool` not found.

- [ ] **Step 3: Implement `winrate_vs_pool`**

```rust
use crate::eval::{evaluate_hac, evaluate_h2h};
use crate::self_play::{load_frozen_hac, OpponentKind};
use crate::HierarchicalActorCritic;

#[derive(Clone, Debug)]
pub struct PoolScore {
    pub winrate_vs_pool: f32,
    pub h2h_vs_sota: f32,
    pub per_opp: Vec<(String, f32)>,
}

/// Mean worker-share winrate of `hac` over every pool opponent (each archetype
/// and each HAC snapshot is one opponent), skipping the entry named
/// `exclude_name` (so a brain that lives in the pool doesn't score itself).
/// `h2h_vs_sota` is the winrate vs the entry named `sota_name` (0.5 if absent).
pub fn winrate_vs_pool(
    hac: &HierarchicalActorCritic,
    pool: &SnapshotPool,
    exclude_name: Option<&str>,
    sota_name: &str,
    device: &candle_core::Device,
    mpe: usize,
) -> Result<PoolScore> {
    // Archetypes in one shot via evaluate_hac (per-archetype worker-share).
    let bench = evaluate_hac(hac, device, mpe)?;
    let mut per_opp: Vec<(String, f32)> = bench.per_archetype.clone();
    let mut h2h_vs_sota = 0.5f32;

    for e in &pool.entries {
        if let OpponentKind::Snapshot { name, path } = &e.kind {
            if exclude_name == Some(name.as_str()) {
                continue;
            }
            let opp = load_frozen_hac(path, pool_sizing(pool), device)?;
            let r = evaluate_h2h(hac, &opp, device, mpe)?;
            if name == sota_name {
                h2h_vs_sota = r.a_winrate_ws;
            }
            per_opp.push((name.clone(), r.a_winrate_ws));
        }
    }

    let n = per_opp.len().max(1) as f32;
    let winrate_vs_pool = per_opp.iter().map(|(_, w)| *w).sum::<f32>() / n;
    tracing::info!(winrate_vs_pool, h2h_vs_sota, opponents = per_opp.len(), "ladder: winrate_vs_pool computed");
    Ok(PoolScore { winrate_vs_pool, h2h_vs_sota, per_opp })
}
```

Add a tiny private helper (sizing is not stored on the pool; the league passes A1 everywhere — read it from a module constant to avoid threading it through):

```rust
/// All ladder HAC opponents are A1 (the project's compact target). Centralized
/// so `winrate_vs_pool` need not thread sizing through every call.
fn pool_sizing(_pool: &SnapshotPool) -> Sizing { crate::hierarchical::sizing::A1 }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-trainer winrate_vs_pool_excludes_self -- --nocapture 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/ladder_league.rs
git commit -m "feat(ladder): winrate_vs_pool with self-exclusion + h2h-vs-sota"
```

---

### Task 3: The gate — pure `gate_decision` + eval-driven `gate`

**Files:**
- Modify: `crates/antcolony-trainer/src/ladder_league.rs`
- Test: in-module `#[cfg(test)]`

**Interfaces:**
- Consumes: `winrate_vs_pool`/`PoolScore` (Task 2).
- Produces:
  ```rust
  pub struct GateOutcome { pub passed: bool, pub winrate_vs_pool: f32, pub h2h_vs_sota: f32 }
  pub fn gate_decision(winrate_vs_pool: f32, standing_bar: f32, h2h_vs_sota: f32, gate_margin: f32) -> bool;
  pub fn gate(candidate: &HierarchicalActorCritic, pool: &SnapshotPool, sota_name: &str,
              standing_bar: f32, gate_margin: f32, device: &candle_core::Device, mpe: usize) -> Result<GateOutcome>;
  ```

- [ ] **Step 1: Write the failing test for `gate_decision` (pure)**

```rust
#[test]
fn gate_decision_requires_both_bar_and_margin() {
    // bar=0.60, margin=0.55
    assert!(gate_decision(0.62, 0.60, 0.57, 0.55), "above bar AND clear h2h -> pass");
    assert!(!gate_decision(0.62, 0.60, 0.51, 0.55), "coin-flip h2h fails despite winrate");
    assert!(!gate_decision(0.58, 0.60, 0.70, 0.55), "below standing bar fails despite big h2h");
    assert!(gate_decision(0.60, 0.60, 0.55, 0.55), "exactly at both thresholds passes (>=)");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-trainer gate_decision_requires_both -- --nocapture 2>&1 | tail -20`
Expected: FAIL — `gate_decision` not found.

- [ ] **Step 3: Implement `gate_decision` + `gate`**

```rust
#[derive(Clone, Copy, Debug)]
pub struct GateOutcome {
    pub passed: bool,
    pub winrate_vs_pool: f32,
    pub h2h_vs_sota: f32,
}

/// PASS iff the candidate meets BOTH the standing bar (mean winrate-vs-pool) and
/// the head-to-head margin over the current SOTA. `>=` on both so exact-threshold
/// candidates promote.
pub fn gate_decision(winrate_vs_pool: f32, standing_bar: f32, h2h_vs_sota: f32, gate_margin: f32) -> bool {
    winrate_vs_pool >= standing_bar && h2h_vs_sota >= gate_margin
}

/// Evaluate the candidate against the frozen pool at the honest `mpe` and apply
/// the two-part pass test. The candidate is NOT in the pool, so nothing is
/// excluded.
pub fn gate(
    candidate: &HierarchicalActorCritic,
    pool: &SnapshotPool,
    sota_name: &str,
    standing_bar: f32,
    gate_margin: f32,
    device: &candle_core::Device,
    mpe: usize,
) -> Result<GateOutcome> {
    let score = winrate_vs_pool(candidate, pool, None, sota_name, device, mpe)?;
    let passed = gate_decision(score.winrate_vs_pool, standing_bar, score.h2h_vs_sota, gate_margin);
    tracing::info!(passed, score.winrate_vs_pool, standing_bar, score.h2h_vs_sota, gate_margin, "ladder: gate evaluated");
    Ok(GateOutcome { passed, winrate_vs_pool: score.winrate_vs_pool, h2h_vs_sota: score.h2h_vs_sota })
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-trainer gate_decision_requires_both -- --nocapture 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/ladder_league.rs
git commit -m "feat(ladder): two-part gate (standing-bar + h2h-margin)"
```

---

### Task 4: `train_round` — frozen-pool best-response trainer with floor-gated keep-best

**Files:**
- Modify: `crates/antcolony-trainer/src/ladder_league.rs`
- Test: in-module `#[cfg(test)]` (CPU smoke, tiny iters)

**Interfaces:**
- Consumes: `JointPpoTrainer`, `ParallelEnv`, `OpponentSampler::Pfsp`, `evaluate_hac`/`evaluate_h2h`, `load_frozen_hac`.
- Produces:
  ```rust
  pub struct RoundOutcome { pub candidate_path: PathBuf, pub best_train_h2h: f32, pub best_train_bench: f32, pub kept: bool }
  pub fn train_round(cfg: &LadderConfig, sota_path: &std::path::Path, pool: &mut SnapshotPool,
                     round: usize, device: &candle_core::Device) -> Result<RoundOutcome>;
  ```
  Trains a fresh A1 trainer warm-started from `sota_path` against the frozen `pool` (PFSP, terminal reward), `iters_per_round` iters. Keep-best metric = h2h-vs-SOTA at `train_mpe`, eligible only if the archetype-bench mean ≥ `keepbest_arch_floor`. Writes the kept checkpoint to `out_dir/round_NN/candidate.safetensors`. The pool's opponent SET is never mutated here (PFSP EMA via `record_result` is allowed).

- [ ] **Step 1: Write the failing smoke test (2 iters, frozen-set invariant + file written)**

```rust
#[test]
fn train_round_smoke_keeps_a_candidate_and_does_not_change_pool_set() {
    use candle_core::Device;
    use crate::hierarchical::sizing::A1;
    let dir = std::env::temp_dir().join("ladder_train_round_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sota_p = dir.join("sota.safetensors");
    { let t = crate::JointPpoTrainer::new(Device::Cpu, A1, crate::JointPpoConfig::smoke_default()).unwrap();
      t.varmap.save(&sota_p).unwrap(); }
    let cs = vec![LadderContender { id: "sota".into(), spec: format!("hac:{}", sota_p.display()) }];
    let mut pool = build_frozen_pool(&cs, 0.1);
    let set_before: Vec<String> = pool.entries.iter().map(|e| format!("{:?}", e.kind)).collect();

    let mut joint = crate::JointPpoConfig::smoke_default();
    joint.rollout_cycles = 4;
    let cfg = LadderConfig {
        sota_path: sota_p.clone(),
        initial_contenders: cs.clone(),
        iters_per_round: 2, eval_every: 1, train_mpe: 1, gate_mpe: 1,
        gate_margin: 0.55, keepbest_arch_floor: 0.0, // floor 0.0 so the keep can't be blocked in the smoke
        archetype_mix: 0.5, pfsp_power: 1.0, no_improve_stop: 2, max_rounds: 8,
        out_dir: dir.clone(), sizing: A1, joint, reward: crate::RewardConfig::default(),
    };
    let outcome = train_round(&cfg, &sota_p, &mut pool, 1, &Device::Cpu).unwrap();
    assert!(outcome.candidate_path.exists(), "kept candidate checkpoint must exist on disk");
    assert!(outcome.kept, "with floor=0.0 a candidate is always eligible to be kept");
    // Frozen-set invariant: the opponent SET (kinds) is unchanged (EMA may differ).
    let set_after: Vec<String> = pool.entries.iter().map(|e| format!("{:?}", e.kind)).collect();
    assert_eq!(set_before, set_after, "train_round must not add/remove pool entries");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-trainer train_round_smoke -- --nocapture 2>&1 | tail -25`
Expected: FAIL — `train_round` / `RoundOutcome` not found.

- [ ] **Step 3: Implement `train_round`**

```rust
use std::path::Path;
use crate::ParallelEnv;
use crate::self_play::OpponentSampler;

#[derive(Clone, Debug)]
pub struct RoundOutcome {
    pub candidate_path: PathBuf,
    pub best_train_h2h: f32,
    pub best_train_bench: f32,
    pub kept: bool,
}

/// Train one best-response round: warm-start from `sota_path`, PFSP against the
/// FROZEN pool, terminal reward. Keep-best on h2h-vs-SOTA gated by an archetype
/// floor. Never adds/removes pool entries (PFSP EMA updates only).
pub fn train_round(
    cfg: &LadderConfig,
    sota_path: &Path,
    pool: &mut SnapshotPool,
    round: usize,
    device: &candle_core::Device,
) -> Result<RoundOutcome> {
    let round_dir = cfg.out_dir.join(format!("round_{round:02}"));
    std::fs::create_dir_all(&round_dir)?;
    let candidate_path = round_dir.join("candidate.safetensors");

    // Fresh trainer warm-started from the current SOTA.
    let mut trainer = crate::JointPpoTrainer::new(device.clone(), cfg.sizing, cfg.joint.clone())?;
    trainer.varmap.load(sota_path)?;
    let mut opt = trainer.make_optimizer()?;
    tracing::info!(round, ?sota_path, "ladder: round trainer warm-started from SOTA");

    // ParallelEnv driven against the FROZEN pool via PFSP. n_envs from joint.matches_per_iter.
    let mut pe = ParallelEnv::new(cfg.joint.matches_per_iter.max(1), cfg.joint.rollout_cycles);
    pe.self_play_enabled = true;
    pe.pool = pool.clone();                 // a working copy; the caller's set stays frozen
    pe.sampler = OpponentSampler::Pfsp { archetype_mix: cfg.archetype_mix, power: cfg.pfsp_power };
    pe.sizing = cfg.sizing;

    let load_for_eval = |p: &Path| crate::self_play::load_frozen_hac(p, cfg.sizing, device);
    let sota_hac = load_for_eval(sota_path)?;

    let mut best_h2h = f32::NEG_INFINITY;
    let mut best_bench = 0.0f32;
    let mut kept = false;

    for it in 0..cfg.iters_per_round {
        let base_seed = round_seed(cfg.joint.seed, round, it);
        let roll = pe.collect_rollout(&trainer.hac, device, &mut trainer.rng, &cfg.reward, base_seed)?;
        // PFSP feedback (EMA only; opponent SET unchanged).
        pe.pool.record_result(pe.last_opponent_idx, pe.last_hac_winshare);
        let stats = trainer.joint_update(&mut opt, &roll)?;

        if cfg.eval_every > 0 && it % cfg.eval_every == 0 {
            let bench = crate::eval::evaluate_hac(&trainer.hac, device, cfg.train_mpe)?;
            let h2h = crate::eval::evaluate_h2h(&trainer.hac, &sota_hac, device, cfg.train_mpe)?;
            tracing::info!(round, it, loss = stats.total, bench = bench.mean_win_rate,
                           h2h = h2h.a_winrate_ws, "ladder: round eval");
            // Keep-best on h2h, ELIGIBLE only above the archetype floor.
            let eligible = bench.mean_win_rate >= cfg.keepbest_arch_floor;
            if eligible && h2h.a_winrate_ws > best_h2h {
                trainer.varmap.save(&candidate_path)?;
                best_h2h = h2h.a_winrate_ws;
                best_bench = bench.mean_win_rate;
                kept = true;
                tracing::info!(round, it, best_h2h, best_bench, "ladder: new round keep-best saved");
            }
        }
    }

    // If nothing ever cleared the floor, fall back to saving the final policy so
    // the gate still has a checkpoint to judge (it will simply fail the gate).
    if !kept {
        trainer.varmap.save(&candidate_path)?;
        let bench = crate::eval::evaluate_hac(&trainer.hac, device, cfg.train_mpe)?;
        let h2h = crate::eval::evaluate_h2h(&trainer.hac, &sota_hac, device, cfg.train_mpe)?;
        best_h2h = h2h.a_winrate_ws;
        best_bench = bench.mean_win_rate;
        tracing::warn!(round, best_h2h, best_bench, "ladder: no checkpoint cleared the floor; saved final policy");
    }

    Ok(RoundOutcome { candidate_path, best_train_h2h: best_h2h, best_train_bench: best_bench, kept })
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-trainer train_round_smoke -- --nocapture 2>&1 | tail -25`
Expected: PASS (slow — a couple CPU rollouts; allow ~1–2 min).

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/ladder_league.rs
git commit -m "feat(ladder): train_round — frozen-pool best-response with floor-gated keep-best"
```

---

### Task 5: `LadderLeague` orchestrator — init bar, round loop, promotion, stop

**Files:**
- Modify: `crates/antcolony-trainer/src/ladder_league.rs`
- Test: in-module `#[cfg(test)]` (CPU smoke, 2 rounds)

**Interfaces:**
- Consumes: `build_frozen_pool`, `winrate_vs_pool`, `train_round`, `gate`, `should_stop`, `load_frozen_hac`.
- Produces:
  ```rust
  pub struct LadderLeague { pub cfg: LadderConfig, pub pool: SnapshotPool, pub device: candle_core::Device,
                            pub sota_path: PathBuf, pub standing_bar: f32 }
  impl LadderLeague {
      pub fn new(cfg: LadderConfig, device: candle_core::Device) -> Result<Self>;
      pub fn run(&mut self) -> Result<LadderReport>;
  }
  ```
  `new` builds the frozen pool and computes the initial standing bar = SOTA's winrate-vs-pool (self-excluded). `run` loops: `train_round` → `gate`; on PASS, add the candidate to the pool as a protected snapshot, set `sota_path` := candidate, update `standing_bar` := candidate winrate-vs-pool, reset `no_improve`; on FAIL, `no_improve += 1`. Stops per `should_stop`.

- [ ] **Step 1: Write the failing 2-round smoke**

```rust
#[test]
fn ladder_league_runs_two_rounds_and_reports() {
    use candle_core::Device;
    use crate::hierarchical::sizing::A1;
    let dir = std::env::temp_dir().join("ladder_league_smoke");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sota_p = dir.join("seed_sota.safetensors");
    { let t = crate::JointPpoTrainer::new(Device::Cpu, A1, crate::JointPpoConfig::smoke_default()).unwrap();
      t.varmap.save(&sota_p).unwrap(); }
    let cs = vec![LadderContender { id: "sota".into(), spec: format!("hac:{}", sota_p.display()) }];

    let mut joint = crate::JointPpoConfig::smoke_default();
    joint.rollout_cycles = 4;
    let cfg = LadderConfig {
        sota_path: sota_p.clone(), initial_contenders: cs,
        iters_per_round: 1, eval_every: 1, train_mpe: 1, gate_mpe: 1,
        gate_margin: 2.0,                 // impossible margin -> guarantees NO promotion -> stops on no_improve
        keepbest_arch_floor: 0.0, archetype_mix: 0.5, pfsp_power: 1.0,
        no_improve_stop: 2, max_rounds: 8,
        out_dir: dir.clone(), sizing: A1, joint, reward: crate::RewardConfig::default(),
    };
    let mut league = LadderLeague::new(cfg, Device::Cpu).unwrap();
    assert!((0.0..=1.0).contains(&league.standing_bar), "initial bar is a winrate");
    let report = league.run().unwrap();
    assert_eq!(report.promotions, 0, "impossible margin -> no promotions");
    assert_eq!(report.stopped_reason, "no_improve");
    assert_eq!(report.rounds_run, 2, "stops after no_improve_stop=2 failed rounds");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-trainer ladder_league_runs_two_rounds -- --nocapture 2>&1 | tail -25`
Expected: FAIL — `LadderLeague` not found.

- [ ] **Step 3: Implement `LadderLeague`**

```rust
use crate::self_play::Role as SpRole;

pub struct LadderLeague {
    pub cfg: LadderConfig,
    pub pool: SnapshotPool,
    pub device: candle_core::Device,
    pub sota_path: PathBuf,
    pub standing_bar: f32,
}

impl LadderLeague {
    pub fn new(cfg: LadderConfig, device: candle_core::Device) -> Result<Self> {
        std::fs::create_dir_all(&cfg.out_dir)?;
        let pool = build_frozen_pool(&cfg.initial_contenders, 0.1);
        // Initial standing bar = the SOTA's winrate-vs-pool (excluding its own "sota" entry).
        let sota_hac = crate::self_play::load_frozen_hac(&cfg.sota_path, cfg.sizing, &device)?;
        let bar = winrate_vs_pool(&sota_hac, &pool, Some("sota"), "sota", &device, cfg.gate_mpe)?;
        tracing::info!(standing_bar = bar.winrate_vs_pool, "ladder: initial standing bar computed");
        Ok(Self { sota_path: cfg.sota_path.clone(), standing_bar: bar.winrate_vs_pool, cfg, pool, device })
    }

    pub fn run(&mut self) -> Result<LadderReport> {
        let mut no_improve = 0usize;
        let mut promotions = 0usize;
        let mut best_h2h_over_seed = f32::NEG_INFINITY;
        let mut rounds_run = 0usize;

        for round in 1..=self.cfg.max_rounds {
            rounds_run = round;
            tracing::info!(round, standing_bar = self.standing_bar, ?self.sota_path, "ladder: ===== round start =====");
            let outcome = train_round(&self.cfg, &self.sota_path, &mut self.pool, round, &self.device)?;
            let candidate = crate::self_play::load_frozen_hac(&outcome.candidate_path, self.cfg.sizing, &self.device)?;
            let g = gate(&candidate, &self.pool, "sota", self.standing_bar,
                         self.cfg.gate_margin, &self.device, self.cfg.gate_mpe)?;
            best_h2h_over_seed = best_h2h_over_seed.max(g.h2h_vs_sota);

            if g.passed {
                // Promote: add as a protected frozen opponent, advance SOTA, raise the bar.
                let name = format!("ladder_r{round:02}");
                self.pool.add_protected_snapshot(name.clone(), outcome.candidate_path.clone(), SpRole::Main);
                self.sota_path = outcome.candidate_path.clone();
                self.standing_bar = g.winrate_vs_pool;
                promotions += 1;
                no_improve = 0;
                tracing::info!(round, %name, new_bar = self.standing_bar, h2h = g.h2h_vs_sota, "ladder: PROMOTED");
            } else {
                no_improve += 1;
                tracing::info!(round, no_improve, h2h = g.h2h_vs_sota, wr = g.winrate_vs_pool, "ladder: candidate did not pass the gate");
            }

            if let Some(reason) = should_stop(no_improve, self.cfg.no_improve_stop, round, self.cfg.max_rounds) {
                tracing::info!(round, reason, promotions, "ladder: ===== STOP =====");
                return Ok(LadderReport {
                    rounds_run, promotions, final_sota_path: self.sota_path.clone(),
                    best_h2h_over_seed, stopped_reason: reason.to_string(),
                });
            }
        }
        Ok(LadderReport {
            rounds_run, promotions, final_sota_path: self.sota_path.clone(),
            best_h2h_over_seed, stopped_reason: "max_rounds".to_string(),
        })
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p antcolony-trainer ladder_league_runs_two_rounds -- --nocapture 2>&1 | tail -25`
Expected: PASS (allow a few min — 2 rounds × tiny rollout + gate evals on CPU).

- [ ] **Step 5: Run the full trainer suite to confirm nothing regressed**

Run: `cargo test -p antcolony-trainer 2>&1 | tail -25`
Expected: all green (existing phase3/SP1/SP2 tests unchanged).

- [ ] **Step 6: Commit**

```bash
git add crates/antcolony-trainer/src/ladder_league.rs
git commit -m "feat(ladder): LadderLeague orchestrator — init bar, round loop, promote, stop"
```

---

### Task 6: Tournament-confirmation on promotion (authoritative #1 + rollback)

**Files:**
- Modify: `crates/antcolony-trainer/src/ladder_league.rs`
- Test: in-module `#[cfg(test)]` (pure helper)

**Interfaces:**
- Consumes: `tournament::{run_tournament, TournamentConfig, TournamentResult}`.
- Produces:
  ```rust
  // returns Some((rank0based, elo, cycles_len)) for `candidate_id`, or None if absent.
  pub fn confirm_rank(result: &crate::tournament::TournamentResult, candidate_id: &str) -> Option<(usize, f64, usize)>;
  ```
  Wires a `run_tournament` call into the promotion branch of `LadderLeague::run`: after the cheap gate passes, build a `TournamentConfig` from the current pool's snapshots + archetypes + the candidate, run it, and confirm the candidate is rank 0 (top Elo). If NOT #1, roll back the promotion (undo the `add_protected_snapshot`, keep the old `sota_path`/`standing_bar`, count it as no_improve) and log the disagreement.

- [ ] **Step 1: Write the failing test for `confirm_rank`**

```rust
#[test]
fn confirm_rank_finds_candidate_position_by_elo() {
    use crate::tournament::TournamentResult;
    let res = TournamentResult {
        ids: vec!["sota".into(), "cand".into(), "sp1".into()],
        specs: vec![],
        win_matrix: vec![], ws_matrix: vec![], games: vec![],
        elo: vec![1500.0, 1600.0, 1400.0],         // cand highest
        winrate_vs_field: vec![0.6, 0.8, 0.4],
        cycles: vec![(0,1,2)],
    };
    let (rank, elo, ncycles) = confirm_rank(&res, "cand").unwrap();
    assert_eq!(rank, 0, "highest Elo -> rank 0");
    assert_eq!(elo, 1600.0);
    assert_eq!(ncycles, 1);
    assert!(confirm_rank(&res, "ghost").is_none());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p antcolony-trainer confirm_rank_finds -- --nocapture 2>&1 | tail -20`
Expected: FAIL — `confirm_rank` not found.

> Note: `TournamentResult` fields are all `pub` (verified in `tournament.rs`), so the struct literal in the test compiles. If a field is missing/renamed when you implement, match the real definition exactly.

- [ ] **Step 3: Implement `confirm_rank` and wire it into the promotion branch**

```rust
/// Rank (0-based, by descending Elo), Elo, and cycle-count for `candidate_id`.
pub fn confirm_rank(result: &crate::tournament::TournamentResult, candidate_id: &str) -> Option<(usize, f64, usize)> {
    let idx = result.ids.iter().position(|id| id == candidate_id)?;
    let elo = result.elo[idx];
    let rank = result.elo.iter().filter(|&&e| e > elo).count(); // how many strictly above
    Some((rank, elo, result.cycles.len()))
}
```

In `LadderLeague::run`, replace the body of the `if g.passed { ... }` branch so the promotion is *provisional* until the full tournament confirms it:

```rust
if g.passed {
    let name = format!("ladder_r{round:02}");
    // Provisionally add so the confirmation tournament includes the candidate.
    self.pool.add_protected_snapshot(name.clone(), outcome.candidate_path.clone(), SpRole::Main);

    // Authoritative re-rank over the full current pool (snapshots + archetypes).
    let mut contenders: Vec<(String, String)> = Vec::new();
    for e in &self.pool.entries {
        match &e.kind {
            crate::self_play::OpponentKind::Snapshot { name, path } =>
                contenders.push((name.clone(), format!("hac:{}", path.display()))),
            crate::self_play::OpponentKind::Archetype(spec) =>
                contenders.push((spec.clone(), spec.clone())),
        }
    }
    let tcfg = crate::tournament::TournamentConfig {
        contenders, mpe: self.cfg.gate_mpe, max_ticks: 10_000,
        anchor_id: "heuristic".into(), anchor_elo: 1000.0, cycle_margin: 0.55, sizing: self.cfg.sizing,
    };
    let tres = crate::tournament::run_tournament(&tcfg, &self.device)?;
    match confirm_rank(&tres, &name) {
        Some((0, elo, ncyc)) => {
            self.sota_path = outcome.candidate_path.clone();
            self.standing_bar = g.winrate_vs_pool;
            promotions += 1;
            no_improve = 0;
            tracing::info!(round, %name, elo, cycles = ncyc, "ladder: PROMOTED + tournament-confirmed #1");
            // (Task 7 sends the Telegram ping here.)
        }
        other => {
            // Cheap gate and full re-rank disagree: roll back the provisional add.
            if let Some(pos) = self.pool.entries.iter().position(|e| matches!(&e.kind,
                crate::self_play::OpponentKind::Snapshot { name: n, .. } if *n == name)) {
                self.pool.entries.remove(pos);
            }
            no_improve += 1;
            tracing::warn!(round, ?other, "ladder: gate passed but tournament did NOT rank candidate #1 -> rolled back");
        }
    }
} else {
    no_improve += 1;
    tracing::info!(round, no_improve, h2h = g.h2h_vs_sota, wr = g.winrate_vs_pool, "ladder: candidate did not pass the gate");
}
```

- [ ] **Step 4: Run the new test + the 2-round smoke (smoke still has gate_margin=2.0 so it never reaches this branch)**

Run: `cargo test -p antcolony-trainer confirm_rank_finds ladder_league_runs_two_rounds -- --nocapture 2>&1 | tail -25`
Expected: PASS (both).

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/ladder_league.rs
git commit -m "feat(ladder): tournament-confirm #1 on promotion with rollback on disagreement"
```

---

### Task 7: CLI binary + cnc run script + Telegram ping on promotion/stop

**Files:**
- Create: `crates/antcolony-trainer/src/bin/ladder_league.rs`
- Create: `scripts/run_ladder_league_cnc.sh`
- Modify: `crates/antcolony-trainer/src/ladder_league.rs` (add an optional ping hook field)
- Test: build-only (a CLI binary; behavior covered by the module tests).

**Interfaces:**
- Consumes: `LadderConfig`, `LadderLeague`, `CandleBackend`/`Backend`, `RewardConfig`, `JointPpoConfig`.
- Produces: a runnable `ladder_league` binary.

- [ ] **Step 1: Write the CLI binary**

```rust
//! Ladder League CLI: iterated best-response vs the frozen tournament ladder.
//! Built --features cuda for cnc P100 training; the gate/tournament run on the
//! same device (use CPU for the gate by passing --gate-cpu if desired later).

use std::path::PathBuf;
use anyhow::Result;
use antcolony_trainer::{Backend, CandleBackend, JointPpoConfig, RewardConfig};
use antcolony_trainer::ladder_league::{LadderConfig, LadderContender, LadderLeague};
use antcolony_trainer::hierarchical::sizing::A1;

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))).init();

    // Defaults (approved).
    let mut sota_path: Option<PathBuf> = None;
    let mut contender_specs: Vec<String> = Vec::new(); // "id=hac:path"
    let mut iters_per_round = 150usize;
    let mut eval_every = 10usize;
    let mut train_mpe = 5usize;
    let mut gate_mpe = 50usize;
    let mut gate_margin = 0.55f32;
    let mut keepbest_arch_floor = 0.70f32;
    let mut archetype_mix = 0.30f32;
    let mut pfsp_power = 1.0f32;
    let mut no_improve_stop = 2usize;
    let mut max_rounds = 8usize;
    let mut rollout_cycles = 96usize;
    let mut matches_per_iter = 8usize;
    let mut ant_chunk_size = 0usize;
    let mut max_grad_norm = 0.5f64;
    let mut reward_path: Option<PathBuf> = None;
    let mut out_dir = PathBuf::from("bench/ladder-league");

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = || args.next().expect("flag needs a value");
        match a.as_str() {
            "--sota" => sota_path = Some(PathBuf::from(next())),
            "--contender" => contender_specs.push(next()),     // repeatable: id=hac:path
            "--iters-per-round" => iters_per_round = next().parse()?,
            "--eval-every" => eval_every = next().parse()?,
            "--train-mpe" => train_mpe = next().parse()?,
            "--gate-mpe" => gate_mpe = next().parse()?,
            "--gate-margin" => gate_margin = next().parse()?,
            "--keepbest-arch-floor" => keepbest_arch_floor = next().parse()?,
            "--archetype-mix" => archetype_mix = next().parse()?,
            "--pfsp-power" => pfsp_power = next().parse()?,
            "--no-improve-stop" => no_improve_stop = next().parse()?,
            "--max-rounds" => max_rounds = next().parse()?,
            "--rollout-cycles" => rollout_cycles = next().parse()?,
            "--matches-per-iter" => matches_per_iter = next().parse()?,
            "--ant-chunk-size" => ant_chunk_size = next().parse()?,
            "--max-grad-norm" => max_grad_norm = next().parse()?,
            "--reward" => reward_path = Some(PathBuf::from(next())),
            "--out" => out_dir = PathBuf::from(next()),
            other => tracing::warn!(arg = other, "unknown flag, ignoring"),
        }
    }

    let sota_path = match sota_path {
        Some(p) => p,
        None => { tracing::error!("--sota <path> is required"); std::process::exit(2); }
    };

    // Parse contenders: "id=hac:path". The SOTA is auto-added as id "sota".
    let mut initial_contenders = vec![LadderContender { id: "sota".into(), spec: format!("hac:{}", sota_path.display()) }];
    for s in &contender_specs {
        let (id, spec) = s.split_once('=').expect("contender must be id=spec");
        initial_contenders.push(LadderContender { id: id.to_string(), spec: spec.to_string() });
    }

    let reward = match &reward_path {
        Some(p) => { let txt = std::fs::read_to_string(p)?; let r: RewardConfig = toml::from_str(&txt)?;
                     tracing::info!(path=%p.display(), ?r, "loaded reward"); r }
        None => { tracing::info!("no --reward; r6 defaults"); RewardConfig::default() }
    };

    let backend = CandleBackend::new()?;
    let device = backend.device().clone();

    let mut joint = JointPpoConfig::smoke_default();
    joint.rollout_cycles = rollout_cycles;
    joint.matches_per_iter = matches_per_iter;
    joint.ant_chunk_size = ant_chunk_size;
    joint.max_grad_norm = max_grad_norm;

    let cfg = LadderConfig {
        sota_path, initial_contenders, iters_per_round, eval_every, train_mpe, gate_mpe,
        gate_margin, keepbest_arch_floor, archetype_mix, pfsp_power, no_improve_stop, max_rounds,
        out_dir, sizing: A1, joint, reward,
    };
    tracing::info!(?cfg, "ladder_league: starting");
    let mut league = LadderLeague::new(cfg, device)?;
    let report = league.run()?;
    tracing::info!(rounds = report.rounds_run, promotions = report.promotions,
                   final_sota = %report.final_sota_path.display(), best_h2h = report.best_h2h_over_seed,
                   reason = %report.stopped_reason, "ladder_league: DONE");
    println!("LADDER_DONE rounds={} promotions={} reason={} final_sota={}",
             report.rounds_run, report.promotions, report.stopped_reason, report.final_sota_path.display());
    Ok(())
}
```

> The `Backend`/`CandleBackend::new()`/`.device()` calls mirror `bin/phase3_train.rs`. If the exact constructor differs, copy the pattern from that file verbatim — do not invent a new one.

- [ ] **Step 2: Build the binary (CPU first, on kokonoe)**

Run: `cargo build -p antcolony-trainer --bin ladder_league 2>&1 | tail -15`
Expected: compiles clean.

- [ ] **Step 3: Write the cnc run script**

```bash
# scripts/run_ladder_league_cnc.sh
# Ladder League on cnc. Training uses the P100 (CUDA build); the sim is the
# CPU bottleneck so RAYON fills the cores. Telegram ping is emitted by the
# binary's stdout being watched by the caller, or add notify here per-round.
set -uo pipefail

# Coordinate the window via openclaw main first. CPU-contention shape: prefer
# RAYON_NUM_THREADS=$(( $(nproc) - 1 )) on a daytime window to protect inference.
export RAYON_NUM_THREADS="${RAYON_NUM_THREADS:-$(nproc)}"
export CUDA_VISIBLE_DEVICES="${CUDA_VISIBLE_DEVICES:-GPU-17bd0d20-0000-0000-0000-000000000000}" # 16GB P100; PROBE LIVE first
# Split CUDA runtime libs (libnvrtc/libcurand ship via pip nvidia packages).
export LD_LIBRARY_PATH="/usr/local/cuda-12.8/targets/x86_64-linux/lib:$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')"

cd /opt/antcolony-cuda || exit 97
echo "=== ladder_league start $(date -Is) RAYON=${RAYON_NUM_THREADS} ==="
./target/release/ladder_league \
  --sota bench/phase3-a1-combat/hac_best.safetensors \
  --contender sp1term=hac:bench/phase3-sp1-terminal/hac_best.safetensors \
  --contender sp1=hac:bench/phase3-sp1/hac_best.safetensors \
  --contender gradclip=hac:bench/phase3-a1-gradclip/hac_best.safetensors \
  --contender sp2=hac:bench/phase3-sp2/league_best.safetensors \
  --reward assets/reward/terminal.toml \
  --iters-per-round 150 --gate-mpe 50 --gate-margin 0.55 \
  --keepbest-arch-floor 0.70 --archetype-mix 0.30 \
  --no-improve-stop 2 --max-rounds 8 --out bench/ladder-league
code=$?
echo "=== ladder_league done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/run_ladder_league.done
exit $code
```

- [ ] **Step 4: Pin the script to LF (so re-shipping from Windows doesn't break bash)**

Verify `.gitattributes` already has `*.sh eol=lf` (it does per prior sessions). If not, add it.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/bin/ladder_league.rs scripts/run_ladder_league_cnc.sh
git commit -m "feat(ladder): CLI binary + cnc run script"
```

---

### Task 8: Full-workspace build + suite + plan self-review checklist

**Files:** none (verification task).

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace 2>&1 | tail -15`
Expected: clean (pre-existing render/sim warnings only).

- [ ] **Step 2: Full trainer suite**

Run: `cargo test -p antcolony-trainer 2>&1 | tail -25`
Expected: all green — existing tests + the new ladder_league unit/smoke tests.

- [ ] **Step 3: Confirm additivity (phase3/SP1/SP2 untouched)**

Run: `git diff --stat main -- crates/antcolony-trainer/src/phase3.rs crates/antcolony-trainer/src/self_play.rs crates/antcolony-trainer/src/exploiter_league.rs crates/antcolony-trainer/src/parallel_env.rs crates/antcolony-trainer/src/eval.rs crates/antcolony-trainer/src/tournament.rs`
Expected: empty (no changes to those files). If non-empty, something leaked — fix before finishing.

- [ ] **Step 4: Commit any fixups**

```bash
git add -A && git commit -m "chore(ladder): workspace build + suite green; additivity confirmed"
```

---

## Self-Review

**1. Spec coverage:**
- Frozen-ladder PFSP trainer → Task 4 (`train_round`, frozen-set invariant test). ✅
- Read-only pool within a round → Task 4 (works on a clone; caller's set asserted unchanged) + Global Constraint. ✅
- Cheap one-row gate vs standing bar → Tasks 2 (`winrate_vs_pool`) + 3 (`gate`/`gate_decision`). ✅
- Standing bar = SOTA's winrate-vs-pool, self-excluded → Task 5 (`LadderLeague::new`). ✅
- Two-part pass test (bar + margin) → Task 3. ✅
- Promotion + pool growth (protected) + advance SOTA + raise bar → Task 5. ✅
- Full tournament re-rank on promotion + rollback on disagreement → Task 6. ✅
- Stop after K no-improve / max rounds → Task 1 (`should_stop`) + Task 5. ✅
- Keep-best gated by archetype floor → Task 4. ✅
- Determinism / seed hygiene (no draw from training RNG) → Task 1 (`round_seed`) + Task 4 (used as `base_seed`; `collect_rollout` already uses a separate `opp_rng`). ✅
- terminal reward / warm-start / A1 / cnc venue → Task 7 (CLI + script). ✅
- Observability (per-round logs, Telegram on promotion/stop) → Tasks 5/6 log; **Telegram ping is noted in Task 6 as a hook but the actual `notify-telegram.sh` call is left to the run wrapper** (the binary prints `LADDER_DONE`; the caller/session pings). This is a deliberate simplification — the binary stays free of host-path coupling.
- Honest-null success criterion → Task 5 (`stopped_reason="no_improve"` is a clean reportable outcome). ✅

**2. Placeholder scan:** No "TBD"/"implement later"/"add error handling". All code blocks are concrete. The one explicit deferral (Telegram inside the binary) is called out and justified, not a placeholder.

**3. Type consistency:**
- `winrate_vs_pool(... ) -> Result<PoolScore>` used consistently in Tasks 2/3/5.
- `gate(...) -> Result<GateOutcome>`; `GateOutcome { passed, winrate_vs_pool, h2h_vs_sota }` consumed in Task 5/6. ✅
- `train_round(...) -> Result<RoundOutcome>`; `RoundOutcome.candidate_path` consumed in Task 5. ✅
- `round_seed`/`should_stop`/`build_frozen_pool` signatures match between Task 1 definition and Task 4/5 use. ✅
- `JointPpoConfig` fields used (`seed`, `rollout_cycles`, `matches_per_iter`, `ant_chunk_size`, `max_grad_norm`) all exist per the extracted signatures. ✅
- `SnapshotPool.entries` / `add_protected_snapshot` / `record_result` and `OpponentKind` variants match `self_play.rs`. ✅
- `evaluate_hac -> EvalReport{mean_win_rate, per_archetype}` and `evaluate_h2h -> H2HReport{a_winrate_ws}` match `eval.rs`. ✅
- `run_tournament`/`TournamentConfig`/`TournamentResult` fields match `tournament.rs`. ✅

**Open risk flagged for execution:** `CandleBackend::new()`/`.device()` exact API — Task 7 Step 1 notes to copy the pattern from `bin/phase3_train.rs` verbatim if the guessed call differs. Likewise `ParallelEnv::new(n_envs, rollout_cycles)` is used with `matches_per_iter` as `n_envs`; confirm the rollout treats `n_envs` as the match count (it does per `collect_rollout`'s batching), else pass the intended env count.

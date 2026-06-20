# Self-Play League (SP1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the HAC train against frozen snapshots of itself (recursive self-play) via a snapshot pool + PFSP-ready opponent sampler, on a single 16GB P100.

**Architecture:** A new pure-logic module `self_play.rs` (SnapshotPool + OpponentSampler) decides which opponent each rollout faces. `parallel_env.rs` gains a frozen-HAC opponent path: when the chosen opponent is a self-snapshot, the right colony is driven by that frozen HAC (no gradient) instead of a cheap sim brain. `phase3.rs` saves snapshots periodically and feeds outcomes back into the pool. A master switch reproduces today's archetype-only training bit-for-bit when off.

**Tech Stack:** Rust (edition 2024), candle (HAC nets), antcolony-sim (CPU sim), rayon, rand_chacha. CUDA build on cnc P100s.

## Global Constraints

- **MSRV / toolchain:** builds under `stable-x86_64-pc-windows-gnu` (kokonoe) and `stable-x86_64-unknown-linux-gnu` + `--features cuda` (cnc). No new non-workspace deps without adding to root `Cargo.toml [workspace.dependencies]`.
- **No `.unwrap()` in sim/training paths** — `Result` + `anyhow`/`thiserror`. Bad snapshot loads fall back, never panic.
- **`tracing` for all logging**, never `println!` (except bin stdout summaries).
- **Backward-compat is sacred:** with self-play OFF, `collect_rollout` / `joint_update` must be byte-identical to today. These regression tests MUST stay green: `chunked_ant_update_matches_monolithic`, `chunked_matches_monolithic_with_grad_clip`, `phase3_smoke_runs_and_evals`, the 47%-baseline numerics.
- **Logging/backup discipline (definition of done for any RUN, not a code task):** every self-play training run gets a `J:\llm-wiki\projects\antcolony-rl-training-log.md` entry (config, curve, both eval metrics, self-play health, checkpoint path + git SHA) and `scripts/backup_checkpoints.ps1` is run after. A run is not "done" until logged + backed up.
- **Spec:** `docs/superpowers/specs/2026-06-19-self-play-league-sp1-design.md`.

---

### Task 1: `SnapshotPool` — the opponent pool (archetypes + capped self-snapshots)

**Files:**
- Create: `crates/antcolony-trainer/src/self_play.rs`
- Modify: `crates/antcolony-trainer/src/lib.rs` (add `pub mod self_play;` + re-export `SnapshotPool`, `OpponentKind`)
- Test: in-file `#[cfg(test)] mod tests` in `self_play.rs`

**Interfaces:**
- Produces:
  - `enum OpponentKind { Archetype(String), Snapshot { name: String, path: std::path::PathBuf } }`
  - `struct PoolEntry { kind: OpponentKind, win_rate_ema: f32, games: u32 }`
  - `struct SnapshotPool { entries: Vec<PoolEntry>, pool_cap: usize, ema_alpha: f32 }`
  - `SnapshotPool::with_archetypes(pool_cap: usize, ema_alpha: f32) -> Self` — seeds the 7 `BENCH_ARCHETYPES` as `Archetype` entries (win_rate_ema 0.5, games 0).
  - `SnapshotPool::add_snapshot(&mut self, name: impl Into<String>, path: impl Into<PathBuf>)` — append a `Snapshot` entry (win_rate_ema 0.5); evict the OLDEST `Snapshot` entry (lowest index among snapshots) once `Snapshot` count > `pool_cap`. Archetypes are never evicted.
  - `SnapshotPool::record_result(&mut self, idx: usize, hac_won: f32)` — `ema = (1-alpha)*ema + alpha*hac_won`; `games += 1`. No-op on out-of-range idx.
  - `SnapshotPool::snapshot_count(&self) -> usize`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_archetypes_seeds_seven() {
        let p = SnapshotPool::with_archetypes(8, 0.1);
        assert_eq!(p.entries.len(), 7);
        assert!(p.entries.iter().all(|e| matches!(e.kind, OpponentKind::Archetype(_))));
        assert_eq!(p.snapshot_count(), 0);
        assert!(p.entries.iter().all(|e| (e.win_rate_ema - 0.5).abs() < 1e-6));
    }

    #[test]
    fn add_snapshot_evicts_oldest_keeps_archetypes() {
        let mut p = SnapshotPool::with_archetypes(2, 0.1);
        p.add_snapshot("s0", "a/0.safetensors");
        p.add_snapshot("s1", "a/1.safetensors");
        p.add_snapshot("s2", "a/2.safetensors"); // cap=2 -> evict s0
        assert_eq!(p.snapshot_count(), 2);
        // 7 archetypes still present
        assert_eq!(p.entries.iter().filter(|e| matches!(e.kind, OpponentKind::Archetype(_))).count(), 7);
        let names: Vec<&str> = p.entries.iter().filter_map(|e| match &e.kind {
            OpponentKind::Snapshot { name, .. } => Some(name.as_str()), _ => None }).collect();
        assert_eq!(names, vec!["s1", "s2"], "oldest snapshot s0 evicted");
    }

    #[test]
    fn record_result_updates_ema() {
        let mut p = SnapshotPool::with_archetypes(8, 0.5);
        p.record_result(0, 1.0); // 0.5 -> 0.75
        assert!((p.entries[0].win_rate_ema - 0.75).abs() < 1e-6);
        assert_eq!(p.entries[0].games, 1);
        p.record_result(999, 1.0); // out of range = no-op (no panic)
    }
}
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p antcolony-trainer --lib self_play`
Expected: FAIL — `self_play` module / `SnapshotPool` not found.

- [ ] **Step 3: Implement the module**

```rust
//! SP1 self-play league: the opponent pool (the 7 fixed archetypes, always
//! present, + a capped FIFO of frozen HAC self-snapshots) and the opponent
//! sampler. Pure logic — no candle, no sim — so it is fully unit-testable.

use std::path::PathBuf;

use crate::eval::BENCH_ARCHETYPES;

#[derive(Clone, Debug)]
pub enum OpponentKind {
    Archetype(String),
    Snapshot { name: String, path: PathBuf },
}

#[derive(Clone, Debug)]
pub struct PoolEntry {
    pub kind: OpponentKind,
    pub win_rate_ema: f32,
    pub games: u32,
}

#[derive(Clone, Debug)]
pub struct SnapshotPool {
    pub entries: Vec<PoolEntry>,
    pub pool_cap: usize,
    pub ema_alpha: f32,
}

impl SnapshotPool {
    pub fn with_archetypes(pool_cap: usize, ema_alpha: f32) -> Self {
        let entries = BENCH_ARCHETYPES
            .iter()
            .map(|a| PoolEntry {
                kind: OpponentKind::Archetype((*a).to_string()),
                win_rate_ema: 0.5,
                games: 0,
            })
            .collect();
        Self { entries, pool_cap, ema_alpha }
    }

    pub fn snapshot_count(&self) -> usize {
        self.entries.iter().filter(|e| matches!(e.kind, OpponentKind::Snapshot { .. })).count()
    }

    pub fn add_snapshot(&mut self, name: impl Into<String>, path: impl Into<PathBuf>) {
        self.entries.push(PoolEntry {
            kind: OpponentKind::Snapshot { name: name.into(), path: path.into() },
            win_rate_ema: 0.5,
            games: 0,
        });
        while self.snapshot_count() > self.pool_cap {
            // evict the oldest snapshot = lowest-index Snapshot entry
            if let Some(pos) = self.entries.iter().position(|e| matches!(e.kind, OpponentKind::Snapshot { .. })) {
                self.entries.remove(pos);
            } else {
                break;
            }
        }
    }

    pub fn record_result(&mut self, idx: usize, hac_won: f32) {
        if let Some(e) = self.entries.get_mut(idx) {
            e.win_rate_ema = (1.0 - self.ema_alpha) * e.win_rate_ema + self.ema_alpha * hac_won;
            e.games = e.games.saturating_add(1);
        }
    }
}
```

Add to `lib.rs`: `pub mod self_play;` and `pub use self_play::{SnapshotPool, OpponentKind};` next to the other `pub mod`/`pub use` lines.

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p antcolony-trainer --lib self_play`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/self_play.rs crates/antcolony-trainer/src/lib.rs
git commit -m "feat(trainer): SnapshotPool — self-play opponent pool (archetypes + capped snapshots)"
```

---

### Task 2: `OpponentSampler` — Uniform + PFSP

**Files:**
- Modify: `crates/antcolony-trainer/src/self_play.rs` (add sampler + tests)

**Interfaces:**
- Consumes: `SnapshotPool` (Task 1).
- Produces:
  - `enum OpponentSampler { Uniform, Pfsp { archetype_mix: f32, power: f32 } }`
  - `OpponentSampler::sample(&self, pool: &SnapshotPool, rng: &mut rand_chacha::ChaCha8Rng) -> usize` — returns an index into `pool.entries`.
    - `Uniform`: uniform over all entries.
    - `Pfsp`: with probability `archetype_mix` (or if `snapshot_count()==0`), pick a uniform random Archetype entry; else pick a Snapshot entry weighted by `(1 - win_rate_ema).max(1e-3).powf(power)` (favor opponents currently beating the HAC). Deterministic given `rng`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn uniform_sampler_covers_all_entries() {
    use rand::SeedableRng;
    let mut p = SnapshotPool::with_archetypes(8, 0.1);
    p.add_snapshot("s0", "a/0.safetensors");
    let s = OpponentSampler::Uniform;
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
    let mut seen = std::collections::HashSet::new();
    for _ in 0..500 { seen.insert(s.sample(&p, &mut rng)); }
    assert_eq!(seen.len(), p.entries.len());
}

#[test]
fn pfsp_favors_losing_matchups_and_honors_mix() {
    use rand::SeedableRng;
    let mut p = SnapshotPool::with_archetypes(8, 0.1);
    p.add_snapshot("strong", "a/strong.safetensors"); // HAC loses to it
    p.add_snapshot("weak", "a/weak.safetensors");     // HAC beats it
    let strong_idx = p.entries.len() - 2;
    let weak_idx = p.entries.len() - 1;
    p.entries[strong_idx].win_rate_ema = 0.1; // HAC mostly loses -> high priority
    p.entries[weak_idx].win_rate_ema = 0.9;   // HAC mostly wins  -> low priority
    let s = OpponentSampler::Pfsp { archetype_mix: 0.0, power: 1.0 }; // snapshots only
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
    let (mut strong_n, mut weak_n) = (0u32, 0u32);
    for _ in 0..2000 {
        match s.sample(&p, &mut rng) {
            i if i == strong_idx => strong_n += 1,
            i if i == weak_idx => weak_n += 1,
            _ => {}
        }
    }
    assert!(strong_n > weak_n * 3, "PFSP must oversample the matchup we lose: strong={strong_n} weak={weak_n}");
}

#[test]
fn pfsp_empty_pool_returns_archetype() {
    use rand::SeedableRng;
    let p = SnapshotPool::with_archetypes(8, 0.1); // no snapshots
    let s = OpponentSampler::Pfsp { archetype_mix: 0.5, power: 1.0 };
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(3);
    for _ in 0..50 {
        let i = s.sample(&p, &mut rng);
        assert!(matches!(p.entries[i].kind, OpponentKind::Archetype(_)));
    }
}
```

- [ ] **Step 2: Run tests, verify fail** — `cargo test -p antcolony-trainer --lib self_play` → FAIL (`OpponentSampler` not found).

- [ ] **Step 3: Implement**

```rust
use rand::Rng;

#[derive(Clone, Copy, Debug)]
pub enum OpponentSampler {
    Uniform,
    Pfsp { archetype_mix: f32, power: f32 },
}

impl OpponentSampler {
    pub fn sample(&self, pool: &SnapshotPool, rng: &mut rand_chacha::ChaCha8Rng) -> usize {
        match self {
            OpponentSampler::Uniform => rng.gen_range(0..pool.entries.len().max(1)),
            OpponentSampler::Pfsp { archetype_mix, power } => {
                let arche: Vec<usize> = pool.entries.iter().enumerate()
                    .filter(|(_, e)| matches!(e.kind, OpponentKind::Archetype(_)))
                    .map(|(i, _)| i).collect();
                let snaps: Vec<usize> = pool.entries.iter().enumerate()
                    .filter(|(_, e)| matches!(e.kind, OpponentKind::Snapshot { .. }))
                    .map(|(i, _)| i).collect();
                let use_archetype = snaps.is_empty() || rng.gen::<f32>() < *archetype_mix;
                if use_archetype && !arche.is_empty() {
                    arche[rng.gen_range(0..arche.len())]
                } else if !snaps.is_empty() {
                    let weights: Vec<f32> = snaps.iter()
                        .map(|&i| (1.0 - pool.entries[i].win_rate_ema).max(1e-3).powf(*power))
                        .collect();
                    let total: f32 = weights.iter().sum();
                    let mut r = rng.gen::<f32>() * total;
                    for (k, &w) in weights.iter().enumerate() {
                        r -= w;
                        if r <= 0.0 { return snaps[k]; }
                    }
                    *snaps.last().unwrap()
                } else {
                    0
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run tests, verify pass** — PASS (3 new).
- [ ] **Step 5: Commit** — `git commit -m "feat(trainer): PFSP-ready OpponentSampler"`

---

### Task 3: Frozen-HAC loader

**Files:**
- Modify: `crates/antcolony-trainer/src/self_play.rs` (add `load_frozen_hac`)
- Test: in-file test that saves a fresh HAC then loads it back.

**Interfaces:**
- Consumes: `JointPpoTrainer::new`, `varmap.load` (the existing checkpoint round-trip used by `bin/eval_winrate.rs`).
- Produces: `pub fn load_frozen_hac(path: &std::path::Path, sizing: Sizing, device: &Device) -> anyhow::Result<HierarchicalActorCritic>` — builds a trainer at `sizing`, `varmap.load(path)`, returns the loaded `hac` (moved out). Used only for forward (mean actions); no optimizer.

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn load_frozen_hac_round_trips_a_saved_checkpoint() {
    use crate::hierarchical::sizing::A1;
    use candle_core::Device;
    let dir = std::env::temp_dir().join("sp1_frozen_hac_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hac.safetensors");
    // save a fresh trainer's varmap
    let t = crate::JointPpoTrainer::new(Device::Cpu, A1, crate::JointPpoConfig::smoke_default()).unwrap();
    t.varmap.save(&path).unwrap();
    // load it back as a frozen HAC + run one mean forward to prove it's usable
    let hac = super::load_frozen_hac(&path, A1, &Device::Cpu).unwrap();
    let mut env = crate::env::MatchEnv::new(1);
    let rich = env.sim.colony_rich_observation(0).unwrap();
    let (s, p, h) = crate::hierarchical::obs_to_tensors::rich_to_tensors(&rich, &Device::Cpu).unwrap();
    let (_a, _i, _v) = hac.mean_commander_action(&s, &p, &h).unwrap();
}
```

- [ ] **Step 2: Run, verify fail** — `load_frozen_hac` not found.

- [ ] **Step 3: Implement** (model on `bin/eval_winrate.rs` lines that do `JointPpoTrainer::new` + `varmap.load`):

```rust
use crate::hierarchical::sizing::Sizing;
use crate::{HierarchicalActorCritic, JointPpoConfig, JointPpoTrainer};
use candle_core::Device;
use std::path::Path;

pub fn load_frozen_hac(path: &Path, sizing: Sizing, device: &Device) -> anyhow::Result<HierarchicalActorCritic> {
    let mut trainer = JointPpoTrainer::new(device.clone(), sizing, JointPpoConfig::smoke_default())?;
    trainer.varmap.load(path)?;
    Ok(trainer.hac)
}
```

(If `hac` can't be moved out of the trainer, return the whole trainer or add a `JointPpoTrainer::into_hac(self)`; the implementer picks the least-invasive option and notes it.)

- [ ] **Step 4: Run, verify pass.**
- [ ] **Step 5: Commit** — `git commit -m "feat(trainer): load_frozen_hac checkpoint loader for self-play opponents"`

---

### Task 4: Frozen-HAC opponent in the rollout (LOAD-BEARING — two-stage review)

**Files:**
- Modify: `crates/antcolony-trainer/src/parallel_env.rs` — `ParallelEnv` gets `sampler: OpponentSampler`; `collect_rollout` selects ONE opponent per rollout via the sampler and, when it's a `Snapshot`, drives the right colony with a frozen HAC; returns the chosen opponent index so phase3 can `record_result`.
- Test: `crates/antcolony-trainer/tests/self_play_rollout.rs` (new integration test).

**Interfaces:**
- Consumes: `SnapshotPool`, `OpponentSampler`, `load_frozen_hac` (Tasks 1-3); the existing right-side drive sites in `collect_rollout` (archetype `opp.decide(&sr)` → `apply_ai_decision(1, …)`).
- Produces: `ParallelEnv.pool: SnapshotPool`, `ParallelEnv.sampler: OpponentSampler`; `collect_rollout` returns `(JointRollout, usize /* opponent idx */, f32 /* hac win-share this rollout */)` OR keeps `JointRollout` and exposes `last_opponent_idx`/`last_hac_winshare` fields — implementer picks; phase3 (Task 5) needs both the idx and the win outcome to call `pool.record_result`.

**Design notes for the implementer (the hard part):**
- ONE opponent per rollout (not per-env): call `self.sampler.sample(&self.pool, rng)` ONCE before the env loop. All `n_envs` share it. This keeps the right-side HAC forward a single batched forward (same weights), mirroring the left.
- If the chosen entry is `Archetype(spec)` → current code path unchanged (build `make_brain(spec)` per env). Backward-compat: when the pool is archetypes-only and the sampler is the default, behavior must match today closely enough that `phase3_smoke` still passes (exact byte-identity is NOT required here since opponent SELECTION changes — but the LEFT training path / `joint_update` numerics are untouched).
- If the chosen entry is `Snapshot { path, .. }` → `load_frozen_hac(path, sizing, device)` ONCE; each cycle, after the left forward, run the SAME commander+ant mean forward for the RIGHT colony (`colony_rich_observation(1)`, `per_ant_observations(1)`) and apply via `apply_ai_decision(1,…)`, `apply_commander_intent(1,…)`, `apply_ant_modulators(1,…)`. Reuse `eval.rs::play_match`'s right-side-by-HAC structure as the reference (but batched across envs like the left side already is).
- `ParallelEnv::new` must keep working (default `pool = SnapshotPool::with_archetypes(...)`, `sampler = OpponentSampler::Uniform`) so existing callers/tests compile.
- The frozen-HAC needs the `Sizing`; thread it via a `ParallelEnv.sizing` field (default A1).

- [ ] **Step 1: Write failing integration test**

```rust
// tests/self_play_rollout.rs
use antcolony_trainer::{JointPpoTrainer, JointPpoConfig, SnapshotPool, OpponentSampler};
use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::ParallelEnv;
use antcolony_trainer::reward::RewardConfig;
use candle_core::Device;
use rand::SeedableRng;

#[test]
fn rollout_with_snapshot_opponent_drives_both_colonies_finite() {
    let device = Device::Cpu;
    // save a snapshot to disk
    let t = JointPpoTrainer::new(device.clone(), A1, JointPpoConfig::smoke_default()).unwrap();
    let dir = std::env::temp_dir().join("sp1_rollout_test");
    std::fs::create_dir_all(&dir).unwrap();
    let snap = dir.join("opp.safetensors");
    t.varmap.save(&snap).unwrap();

    // a pool that ONLY offers the snapshot (force the HAC-opponent path)
    let mut pool = SnapshotPool::with_archetypes(8, 0.1);
    pool.entries.clear(); // remove archetypes for this test
    pool.add_snapshot("opp", &snap);

    let mut pe = ParallelEnv::new(2, 4);
    pe.pool = pool;
    pe.sampler = OpponentSampler::Uniform;

    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
    let (rollout, _opp_idx, winshare) =
        pe.collect_rollout(&t.hac, &device, &mut rng, &RewardConfig::default(), 99);
    // both buffers non-empty, all finite
    assert!(!rollout.commander.is_empty() && !rollout.ant.is_empty());
    assert!((0.0..=1.0).contains(&winshare));
}
```

(If Task 4's chosen return shape differs, adapt the destructuring — keep the assertions.)

- [ ] **Step 2: Run, verify fail** — signature mismatch / `pool` field missing.
- [ ] **Step 3: Implement the frozen-HAC opponent path** in `collect_rollout` per the design notes. Keep the archetype path intact.
- [ ] **Step 4: Run the new test + the regression set:**

```
cargo test -p antcolony-trainer --test self_play_rollout
cargo test -p antcolony-trainer --lib joint_ppo::tests::chunked_ant_update_matches_monolithic
cargo test -p antcolony-trainer --lib joint_ppo::tests::chunked_matches_monolithic_with_grad_clip
```
Expected: new test PASS; regressions PASS (left path untouched).

- [ ] **Step 5: Commit** — `git commit -m "feat(trainer): frozen-HAC opponent in collect_rollout (self-play)"`

**Review:** this task gets the full spec-review + code-quality-review tier (load-bearing rollout hot-loop change).

---

### Task 5: Phase3 self-play wiring (config + snapshot save + outcome feedback)

**Files:**
- Modify: `crates/antcolony-trainer/src/phase3.rs` — `Phase3Config` gains self-play fields; `run_phase3` builds the pool/sampler, samples per rollout, saves snapshots on cadence, and records outcomes.
- Test: extend `phase3::tests` with a self-play smoke.

**Interfaces:**
- Consumes: `ParallelEnv.pool/sampler` (Task 4), `SnapshotPool::add_snapshot/record_result`.
- Produces: `Phase3Config` fields: `self_play_enabled: bool` (default false), `snapshot_every: usize` (25), `pool_cap: usize` (8), `opponent_sampling: OpponentSampler` (Pfsp{archetype_mix:0.5, power:1.0}), `warm_start_snapshot: Option<PathBuf>`. `Phase3Report` gains `self_play_winrate: Option<f32>` (HAC win-rate vs the pool, the health metric).

**Behavior:**
- If `self_play_enabled`: build `env.pool = SnapshotPool::with_archetypes(pool_cap, 0.1)`; if `warm_start_snapshot` set, `pool.add_snapshot("sota", path)`; set `env.sampler`. Every `snapshot_every` iters, `env.varmap.save(out_dir/snapshot_NNNN.safetensors)` (the TRAINING hac) then `pool.add_snapshot`. After each rollout, `pool.record_result(opp_idx, winshare)`. Report `self_play_winrate` = mean `win_rate_ema` over snapshot entries.
- If `!self_play_enabled`: unchanged (archetype-only, today's behavior) — `phase3_smoke` stays green.

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn phase3_self_play_smoke_adds_snapshots() {
    // tiny config: self_play on, snapshot_every=1, a few iters
    let tmp = std::env::temp_dir().join("sp1_phase3_smoke");
    std::fs::create_dir_all(&tmp).unwrap();
    let mut cfg = Phase3Config::smoke_default(); // existing helper
    cfg.self_play_enabled = true;
    cfg.snapshot_every = 1;
    cfg.out_dir = tmp.clone();
    let report = run_phase3(cfg).unwrap();
    // at least one snapshot file written
    let snaps = std::fs::read_dir(&tmp).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("snapshot_"))
        .count();
    assert!(snaps >= 1, "self-play should have saved >=1 snapshot");
    assert!(report.self_play_winrate.is_some());
}
```

- [ ] **Step 2: Run, verify fail** — fields missing.
- [ ] **Step 3: Implement the wiring** in `run_phase3`.
- [ ] **Step 4: Run the new smoke + the existing one:**
```
cargo test -p antcolony-trainer --lib phase3::tests::phase3_self_play_smoke_adds_snapshots
cargo test -p antcolony-trainer --lib phase3::tests::phase3_smoke_runs_and_evals
```
Expected: both PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat(trainer): phase3 self-play wiring (snapshots + PFSP feedback + health metric)"`

---

### Task 6: CLI flags + cnc run script

**Files:**
- Modify: `crates/antcolony-trainer/src/bin/phase3_train.rs` — add `--self-play`, `--snapshot-every N`, `--pool-cap N`, `--opponent-sampling uniform|pfsp`, `--archetype-mix F`, `--warm-start-snapshot PATH`; flow into `Phase3Config` (mirror the existing hand-rolled `--max-grad-norm`/`--sizing` parsing).
- Create: `scripts/run_selfplay_cnc.sh` — copy `scripts/run_combat_cnc.sh`, set the self-play flags, `--reward assets/reward/combat.toml`, `--warm-start-snapshot bench/phase3-a1-combat/hac_best.safetensors`, `--out bench/phase3-sp1`. Keep the EXIT-trap service restore.

**Interfaces:** Consumes Task 5's `Phase3Config` fields.

- [ ] **Step 1: Add the flags** (no separate unit test — covered by a build + `--help`-style smoke; the parser is hand-rolled and untested today). Add each flag arm; default off so existing invocations are unchanged.
- [ ] **Step 2: Build** — `cargo build --release -p antcolony-trainer --bin phase3_train`. Expected: clean.
- [ ] **Step 3: Smoke the flag plumbing locally (CPU, 2 iters):**
```
cargo run --release -p antcolony-trainer --bin phase3_train -- \
  --self-play --snapshot-every 1 --iters 2 --envs 2 --rollout-cycles 4 \
  --eval-every 99 --matches-per-eval 1 --out bench/sp1-smoke
```
Expected: runs, logs `self_play=true`, writes a `snapshot_0000.safetensors`.
- [ ] **Step 4: Write `scripts/run_selfplay_cnc.sh`** (copy run_combat_cnc.sh + self-play flags + warm-start).
- [ ] **Step 5: Commit** — `git commit -m "feat(trainer): phase3_train self-play CLI flags + run_selfplay_cnc.sh"`

---

## Self-Review

**Spec coverage:**
- §4 self_play.rs (SnapshotPool + OpponentSampler) → Tasks 1-2 ✓
- §4 frozen-HAC opponent in parallel_env → Task 4 ✓ (loader Task 3)
- §4/§5 phase3 wiring (snapshot save, outcome feedback) → Task 5 ✓
- §6 config defaults → Tasks 5-6 ✓ (`self_play_enabled=false` default = backward-compat)
- §8 self-play health metric → Task 5 (`self_play_winrate`) ✓; 7-archetype forgetting guard = the existing eval, unchanged ✓
- §9 error handling (bad snapshot → fallback) → Task 4 design note (reuse existing fallback) ✓
- §10 testing → each task is TDD ✓; regression set named in Global Constraints ✓
- §11 logging discipline → Global Constraints "definition of done" + Task 6 run script ✓

**Placeholder scan:** Task 4's exact rollout diff is intentionally described (notes + test) rather than pre-written line-by-line — it's a 200-line hot-loop modification best done test-first by the implementer, and is flagged for two-stage review. All other tasks have complete code.

**Type consistency:** `OpponentKind`/`PoolEntry`/`SnapshotPool`/`OpponentSampler` names + signatures are consistent across Tasks 1-5. `load_frozen_hac(path, sizing, device)` consistent Tasks 3-4. `collect_rollout` return-shape change (Task 4) is consumed in Task 5's `record_result(opp_idx, winshare)`.

## Notes
- **Run on cnc** (P100), coordinate via openclaw `main`, free the 16GB card (signal-trapped restore) — same playbook as the combat run; verify the live card↔service mapping first (it flipped once).
- **First real SP1 run** = warm-start from `phase3-a1-combat/hac_best` (0.874), combat.toml reward, then watch the self-play health metric (~0.5 = healthy) AND the 7-archetype eval (must not regress = no forgetting). Log it in the ledger + back up.

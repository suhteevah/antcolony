# SP2 Exploiter League Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Beat the 0.874 SOTA via an AlphaStar-style exploiter league (main + main-exploiters + league-exploiters), single-GPU round-robin, that hunts and fixes the main agent's weaknesses.

**Architecture:** A `LeagueManager` orchestrates several `LeagueAgent`s (each a thin wrapper over the existing `JointPpoTrainer` + a `ParallelEnv`), round-robin on one GPU, over a shared role-tagged `SnapshotPool`. Exploiters train against the main (or the league), and when they beat their target their snapshots join the league + they reset — forcing the main past the peer plateau. Pure orchestration; reuses SP1's PPO/rollout/eval unchanged.

**Tech Stack:** Rust (edition 2024), candle, antcolony-sim (CPU sim), rand_chacha. CUDA on cnc P100.

## Global Constraints

- **Reuse, don't rebuild:** `self_play.rs` (`SnapshotPool`, `PoolEntry`, `OpponentKind`, `OpponentSampler`, `load_frozen_hac`), `parallel_env::ParallelEnv` (fields: `n_envs, rollout_cycles, league, self_play_enabled, pool, sampler, sizing, last_opponent_idx, last_hac_winshare`; `collect_rollout(&mut self, hac, device, rng, reward, base_seed) -> Result<JointRollout>`), `JointPpoTrainer` (fields `hac, varmap, device, rng`; `new(device, sizing, config)`), `eval::{evaluate_hac, evaluate_h2h, H2HReport}`, `assets/reward/terminal.toml`.
- **Reward:** terminal-dominant (`terminal.toml`) for all agents.
- **No `.unwrap()`** in production paths (`Result` + `?`); tests may. `tracing`, never `println!` (bin stdout summaries OK). Edition 2024 (`gen` is reserved → `r#gen`).
- **Backward-compat:** SP2 is additive — `phase3.rs` / SP1 / existing tests stay byte-unchanged. New module + new bin only; the one change to `self_play.rs` (Task 1) must keep existing `SnapshotPool` callers compiling (`add_snapshot` gains a role param — update the 2 existing call sites in `phase3.rs`/tests, or default via a second method).
- **Single-GPU round-robin** (multi-GPU = SP3, out of scope). A1 sizing.
- **Logging/backup is definition-of-done for any RUN** (not a code task): ledger entry + `backup_checkpoints.ps1`.
- **Spec:** `docs/superpowers/specs/2026-06-20-self-play-league-sp2-design.md`.

---

### Task 1: Role-tagged pool + protected entries

**Files:** Modify `crates/antcolony-trainer/src/self_play.rs`; update call sites in `crates/antcolony-trainer/src/phase3.rs` + any `parallel_env.rs` pool construction. Test: in-file `self_play` tests.

**Interfaces — Produces:**
- `enum Role { Archetype, Main, MainExploiter, LeagueExploiter }` (derive Clone, Copy, Debug, PartialEq, Eq).
- `PoolEntry` gains `pub role: Role` and `pub protected: bool`.
- `SnapshotPool::with_archetypes` tags entries `role: Archetype, protected: true`.
- `SnapshotPool::add_snapshot(&mut self, name, path, role: Role)` — appends `protected: false`; eviction now evicts the oldest **non-protected, non-archetype** entry when snapshot count > cap.
- `SnapshotPool::add_protected_snapshot(&mut self, name, path, role: Role)` — same but `protected: true` (for the SOTA seed + best-main).

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn add_snapshot_carries_role_and_evicts_unprotected_only() {
    let mut p = SnapshotPool::with_archetypes(2, 0.1);
    p.add_protected_snapshot("sota", "s/sota.st", Role::Main);
    p.add_snapshot("e0", "s/e0.st", Role::MainExploiter);
    p.add_snapshot("e1", "s/e1.st", Role::MainExploiter);
    p.add_snapshot("e2", "s/e2.st", Role::LeagueExploiter); // cap=2 snapshots beyond protected? evict oldest UNPROTECTED snapshot
    // protected sota + archetypes never evicted; oldest unprotected (e0) gone
    let names: Vec<&str> = p.entries.iter().filter_map(|e| match &e.kind {
        OpponentKind::Snapshot { name, .. } => Some(name.as_str()), _ => None }).collect();
    assert!(names.contains(&"sota"), "protected snapshot kept");
    assert!(!names.contains(&"e0"), "oldest unprotected evicted");
    assert_eq!(p.entries.iter().filter(|e| e.role == Role::Archetype).count(), 7);
    // role tag present
    assert!(p.entries.iter().any(|e| e.role == Role::MainExploiter));
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p antcolony-trainer --lib self_play` → FAIL (Role / role field / add_protected_snapshot missing).
- [ ] **Step 3: Implement** — add `Role`, the two fields, the two add methods (eviction skips `protected || role==Archetype`). Update `with_archetypes` to set role/protected. Update existing `add_snapshot` callers (phase3.rs, parallel_env tests) to pass a role (`Role::Main` for the existing self-play snapshot saves).
- [ ] **Step 4: Run** — `cargo test -p antcolony-trainer --lib self_play` PASS; `cargo build -p antcolony-trainer` clean.
- [ ] **Step 5: Commit** — `feat(trainer): role-tagged pool entries + protected snapshots`

---

### Task 2: Exploiter promote/reset decision (pure fn)

**Files:** `crates/antcolony-trainer/src/exploiter_league.rs` (new) + `pub mod exploiter_league;` in `lib.rs`. Test: in-file.

**Interfaces — Produces:**
- `enum ExploiterAction { Continue, Promote, ForcedReset }`
- `pub fn exploiter_decision(winrate_ema: f32, iters_done: usize, promote_winrate: f32, max_iters: usize) -> ExploiterAction` — `Promote` if `winrate_ema >= promote_winrate`; else `ForcedReset` if `iters_done >= max_iters`; else `Continue`.

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn exploiter_decision_promotes_resets_continues() {
    use super::{exploiter_decision, ExploiterAction::*};
    assert_eq!(exploiter_decision(0.80, 40, 0.70, 100), Promote);     // beat target
    assert_eq!(exploiter_decision(0.30, 100, 0.70, 100), ForcedReset);// gave up at budget
    assert_eq!(exploiter_decision(0.30, 40, 0.70, 100), Continue);    // keep hunting
    assert_eq!(exploiter_decision(0.70, 10, 0.70, 100), Promote);     // exactly at threshold
}
```

- [ ] **Step 2: Run, verify fail.**
- [ ] **Step 3: Implement** the enum + fn + `pub mod exploiter_league;` in lib.rs.
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** — `feat(trainer): exploiter promote/reset decision fn`

---

### Task 3: LeagueAgent — a trainable agent over an opponent source

**Files:** Modify `exploiter_league.rs`. Test: integration `tests/league_agent.rs`.

**Interfaces:**
- Consumes: `JointPpoTrainer::new`, `ParallelEnv` (+ its `pool`/`sampler`/`self_play_enabled` fields + `collect_rollout`), `JointPpoTrainer`'s update path (the same call `run_phase3` uses after `collect_rollout` — find it: `joint_update`/GAE; reuse verbatim), `load_frozen_hac`, `Role`, `RewardConfig`.
- Produces:
  - `struct LeagueAgent { pub role: Role, pub trainer: JointPpoTrainer, pub env: ParallelEnv, pub winrate_ema: f32, pub iters_since_reset: usize }`
  - `LeagueAgent::new(role, sizing, device, joint_cfg, warm_start: Option<&Path>) -> Result<Self>` — builds the trainer (warm-start its varmap if given), an env with `self_play_enabled=true`.
  - `LeagueAgent::train_iters(&mut self, n: usize, pool: &SnapshotPool, sampler: OpponentSampler, reward: &RewardConfig, base_seed: u64) -> Result<()>` — sets `self.env.pool = pool.clone(); self.env.sampler = sampler;` then runs `n` rollout→update iters (mirroring `run_phase3`'s inner loop), updating `self.winrate_ema` from `env.last_hac_winshare` (EMA) and `self.iters_since_reset += n`.
  - `LeagueAgent::snapshot_to(&self, path: &Path) -> Result<()>` — `self.trainer.varmap.save(path)`.
  - `LeagueAgent::reset_from(&mut self, path: &Path) -> Result<()>` — `self.trainer.varmap.load(path)`; `self.winrate_ema = 0.5; self.iters_since_reset = 0;`.

**Implementation note:** `train_iters` is the SP1 `run_phase3` inner loop minus eval/snapshot — extract/mirror it. For a main-exploiter, the caller passes a 1-entry `pool` (the current main) + a sampler that always returns it; for main/league-exploiter, the shared league pool + PFSP.

- [ ] **Step 1: Write failing integration test** (`tests/league_agent.rs`): build a `LeagueAgent` (Role::Main, A1, warm-start from a saved fresh varmap), make a 1-snapshot pool, `train_iters(2, &pool, Uniform, &terminal_reward, 7)`, assert it returns Ok, `iters_since_reset == 2`, `winrate_ema` in `[0,1]`; then `snapshot_to(tmp)` + `reset_from(tmp)` resets `iters_since_reset` to 0. Keep FAST (2 iters, eval-light — no eval here).
- [ ] **Step 2: Run, verify fail.**
- [ ] **Step 3: Implement** `LeagueAgent` (reuse the run_phase3 inner loop for `train_iters`).
- [ ] **Step 4: Run** the new test + the regression set (`cargo test -p antcolony-trainer --lib joint_ppo::tests::chunked_ant_update_matches_monolithic`). FAST test only locally; heavy on cnc.
- [ ] **Step 5: Commit** — `feat(trainer): LeagueAgent (trainable agent over an opponent source)`

---

### Task 4: LeagueManager — round-robin + promotion/reset (LOAD-BEARING, two-stage review)

**Files:** Modify `exploiter_league.rs`. Test: `tests/league_manager.rs`.

**Interfaces:**
- Consumes: `LeagueAgent` (T3), `exploiter_decision` (T2), `SnapshotPool` + `add_snapshot`/`add_protected_snapshot` (T1), `OpponentSampler`.
- Produces:
  - `struct LeagueConfig { league_steps, iters_main, iters_exploiter, n_main_exploiters, n_league_exploiters, pool_cap, exploiter_promote_winrate, exploiter_max_iters, main_snapshot_every, archetype_mix, eval_every_steps, success_mpe, snapshot_dir, sota_path, sizing, joint, reward }` with a `smoke()` default.
  - `struct LeagueManager { pool: SnapshotPool, main: LeagueAgent, main_exploiters: Vec<LeagueAgent>, league_exploiters: Vec<LeagueAgent>, cfg: LeagueConfig, step: usize }`
  - `LeagueManager::new(cfg, device) -> Result<Self>` — pool seeded with archetypes + the SOTA as a **protected** `Role::Main` snapshot; main warm-started from `sota_path`; exploiters warm-started from `sota_path`.
  - `LeagueManager::run_step(&mut self) -> Result<()>` — one league-step:
    1. snapshot the main to `snapshot_dir/current_main.safetensors`.
    2. `main.train_iters(iters_main, &pool, Pfsp{archetype_mix,1.0}, reward, seed)`.
    3. each main-exploiter: build a 1-entry pool of `current_main.safetensors` (Role::Main), `train_iters(iters_exploiter, &that, Uniform, …)`.
    4. each league-exploiter: `train_iters(iters_exploiter, &pool, Pfsp, …)`.
    5. for each exploiter: `match exploiter_decision(winrate_ema, iters_since_reset, promote_winrate, max_iters)` → `Promote`: `snapshot_to(snapshot_dir/exp_<step>_<i>.st)`, `pool.add_snapshot(name, path, role)`, `reset_from(current_main)` (main-exploiter) / `reset_from(sota)` (league-exploiter); `ForcedReset`: reset only; `Continue`: nothing.
    6. every `main_snapshot_every` steps: `pool.add_snapshot(main_<step>, path, Role::Main)`.
  - `LeagueManager::run(&mut self) -> Result<LeagueReport>` — loops `run_step` `league_steps` times, calling the success eval (Task 5) every `eval_every_steps`.

**Implementation notes (the hard part):** all agents share the single device sequentially. The `pool` is the manager's shared league; main + league-exploiters PFSP over it; main-exploiters get a fresh 1-entry pool each step. Seeds: derive each agent's `base_seed` from `step` + agent index (deterministic, distinct). Do NOT draw from any agent's training RNG for orchestration.

- [ ] **Step 1: Write failing integration test** (`tests/league_manager.rs`): `LeagueConfig::smoke()` (2 steps, 1 main-exploiter, 1 league-exploiter, iters_main=2, iters_exploiter=2, exploiter_max_iters=2 so a forced reset fires, eval_every_steps=99 to skip eval). `LeagueManager::new` with a saved fresh varmap as the "sota". `run()`. Assert: completes Ok; ≥1 `exp_*` or `main_*` snapshot file written to snapshot_dir; the forced-reset path executed (main-exploiter `iters_since_reset` reset to 0 at least once — expose via the report or a counter). Keep FAST/eval-light.
- [ ] **Step 2: Run, verify fail.**
- [ ] **Step 3: Implement** `LeagueConfig`, `LeagueManager`, `run_step`, `run`.
- [ ] **Step 4: Run** the new test + regressions (`self_play`, `joint_ppo::tests::chunked_*`). FAST locally; heavy on cnc.
- [ ] **Step 5: Commit** — `feat(trainer): LeagueManager round-robin + promotion/reset`

**Review:** full spec + code-quality review (load-bearing orchestration touching seeds/training).

---

### Task 5: Success eval (h2h vs SOTA + forgetting guard) + keep-best

**Files:** Modify `exploiter_league.rs` (integrate into `LeagueManager::run`). Test: extend `tests/league_manager.rs`.

**Interfaces:**
- Consumes: `eval::evaluate_h2h(&main.hac, &sota_hac, device, mpe)`, `eval::evaluate_hac(&main.hac, device, mpe)`, `load_frozen_hac` (load the SOTA once for h2h).
- Produces: `struct LeagueReport { steps: usize, best_h2h_vs_sota: f32, best_step: usize, final_bench: f32, snapshots_added: usize, exploiter_resets: usize }`. On each eval, if `h2h_vs_sota > best_h2h_vs_sota`, save the main to `snapshot_dir/league_best.safetensors` (keep-best). Log per-eval: step, h2h-vs-SOTA, 7-archetype mean (forgetting guard), pool size, snapshot count.

- [ ] **Step 1: Write failing test** — extend the manager smoke with `eval_every_steps=1, success_mpe=1`; assert `report.best_h2h_vs_sota` is set (in `[0,1]`) and `league_best.safetensors` exists. FAST.
- [ ] **Step 2: Run, verify fail.**
- [ ] **Step 3: Implement** the eval hook in `run` + `LeagueReport` + keep-best.
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** — `feat(trainer): league success-eval (h2h vs SOTA) + keep-best`

---

### Task 6: `phase3_league` bin + CLI + cnc run script

**Files:** Create `crates/antcolony-trainer/src/bin/phase3_league.rs`; create `scripts/run_league_cnc.sh`. Test: build + fast CLI smoke.

**Interfaces:** Consumes `LeagueManager`, `LeagueConfig`. CLI flags (mirror `phase3_train`'s hand-rolled parser): `--league-steps --iters-main --iters-exploiter --n-main-exploiters --n-league-exploiters --pool-cap --promote-winrate --exploiter-max-iters --main-snapshot-every --archetype-mix --eval-every-steps --success-mpe --sota <path> --reward <path> --out <dir>`. Builds `LeagueConfig`, runs `LeagueManager::run`, prints the `LeagueReport`.

- [ ] **Step 1: Add the bin** (model on `phase3_train.rs`): parse flags → `LeagueConfig` → `LeagueManager::new` → `run` → print report. Log all config in the startup `tracing::info!`.
- [ ] **Step 2: Build** — `cargo build --release -p antcolony-trainer --bin phase3_league`. Clean.
- [ ] **Step 3: FAST eval-light CLI smoke** (local): `cargo run --release -p antcolony-trainer --bin phase3_league -- --league-steps 2 --iters-main 2 --iters-exploiter 2 --eval-every-steps 99 --sota <a-saved-fresh-ckpt> --reward assets/reward/terminal.toml --out bench/league-smoke` — completes, writes snapshots. Do NOT run a real (eval-on) league locally — cnc only.
- [ ] **Step 4: Write `scripts/run_league_cnc.sh`** — copy `scripts/run_selfplay_cnc.sh` (signal-trapped service restore, GPU-UUID pin, LD_LIBRARY_PATH, RAYON=3); swap the invocation to `phase3_league` with `--sota bench/phase3-a1-combat/hac_best.safetensors --reward assets/reward/terminal.toml --out bench/phase3-sp2` + the pilot config (`--league-steps 15` first).
- [ ] **Step 5: Commit** — `feat(trainer): phase3_league bin + CLI + run_league_cnc.sh`

---

## Self-Review

**Spec coverage:** §3 roles → Tasks 1,3,4. §4 units (`exploiter_league.rs`, `LeagueManager`, `LeagueAgent`) → Tasks 2-5. §5 round-robin → Task 4. §6 config → Tasks 4,6. §7 success metric (h2h vs SOTA + keep-best) → Task 5. §8 safety (protected entries, no-RNG-for-orchestration, fallback) → Tasks 1,3,4. §9 testing → each task TDD. §10 scope (single-GPU, A1) → honored. Open questions §11 resolved by the approved defaults (reset-from-current-main, 1+1 exploiters, 15-step pilot → in the run script).

**Placeholder scan:** Tasks 3-4's `train_iters`/`run_step` bodies are described (interfaces + tests + notes) rather than pre-written line-by-line — they orchestrate the existing `run_phase3` loop + trainer, best done test-first; Task 4 flagged for two-stage review. Tasks 1-2 have complete code.

**Type consistency:** `Role`, `PoolEntry.role/protected`, `add_snapshot(name,path,role)`/`add_protected_snapshot`, `exploiter_decision(winrate_ema,iters_done,promote_winrate,max_iters)`, `LeagueAgent{role,trainer,env,winrate_ema,iters_since_reset}` + `train_iters/snapshot_to/reset_from`, `LeagueManager{pool,main,main_exploiters,league_exploiters,cfg,step}` + `new/run_step/run`, `LeagueReport`, `LeagueConfig` — consistent across Tasks 1-6.

## Notes
- **First run = 15-step pilot on cnc** (validate the league machinery), then the 40-step run. Coordinate the window via openclaw main + nightdrive-clear + warm-start keeper present; free only the 16GB card; ledger + backup after.
- **Success = `league_best.safetensors` beats the SOTA on `eval_h2h` at mpe=50.** If it does → new SOTA + SP3 (multi-GPU) becomes worth it. If the league stalls at peer → bump to 2 main-exploiters / longer, or conclude A1 is the plateau.

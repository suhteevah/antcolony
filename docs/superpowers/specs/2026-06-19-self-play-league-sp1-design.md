# SP1 — Self-Play League Foundation (Design Spec)

**Date:** 2026-06-19
**Status:** design — awaiting approval before writing-plans
**Project:** antcolony hierarchical brain (HAC) RL
**Part of:** the full AlphaStar-style league program. SP1 = foundation; SP2 = exploiter roles; SP3 = multi-GPU. This spec covers **SP1 only**.

## 1. Goal

Let the HAC train against **frozen snapshots of itself** — recursive self-improvement — instead of only the 7 fixed hand-coded archetypes it has now saturated (0.871 worker-share / 0.874 decisive, 87% wins-by-kill). SP1 delivers the recursive loop with a PFSP-ready opponent sampler, on a single 16GB P100. Exploiter roles and multi-player training are explicitly out of scope (SP2).

## 2. Why (the ceiling we're breaking)

`ParallelEnv` trains the right colony against `League::default_pool()` — exactly the 7 archetypes used for eval. The brain has solved them: the bench can no longer measure improvement, and 0.871 is partly in-distribution (beating bots it trained on). The only way to keep getting stronger is a moving target — itself.

## 3. Scope (YAGNI for SP1)

IN:
- Frozen-HAC opponent in the rollout (right colony driven by a snapshot HAC).
- Snapshot pool: periodically save the training HAC, add to the league, cap the pool.
- PFSP-ready sampler: opponents sampled by win-rate priority over a mix of {archetypes + self-snapshots}.
- Self-play health metric + the existing 7-archetype eval (forgetting guard).
- Ledger + checkpoint-backup discipline wired into the run.

OUT (later sub-projects):
- Per-env distinct opponents (SP1 = **one opponent per rollout**, batched).
- Exploiter roles / multiple concurrent league players (SP2).
- Multi-GPU data-parallel (SP3).

## 4. Architecture

New module **`crates/antcolony-trainer/src/self_play.rs`** (pure logic, fully unit-testable):
- `SnapshotPool` — the league entries: the 7 archetypes (always present) + a capped FIFO of HAC snapshot handles `{name, checkpoint_path, win_rate_ema}`.
  - `add_snapshot(name, path)` — append; evict oldest HAC snapshot beyond `pool_cap` (archetypes never evicted).
  - `record_result(opp_idx, hac_won: f32)` — update that opponent's win-rate EMA.
- `OpponentSampler` — picks the next opponent index:
  - `Uniform` — uniform over all entries (baseline/control).
  - `Pfsp { archetype_mix }` — with prob `archetype_mix` sample an archetype uniformly (coverage/anti-collapse); else sample a HAC snapshot weighted by `(1 - win_rate_ema)^p` (favor opponents currently beating us). Deterministic given an RNG.
- Both are plain structs over `Vec`/`HashMap` — no candle, no sim. Testable in isolation.

Changed: **`parallel_env.rs`** gains a frozen-HAC opponent path. Currently: left colony = training HAC (batched forward), right colony = cheap sim `AiBrain`. SP1 adds: per rollout, the sampler picks ONE opponent for all N envs:
- If it's an **archetype** → current path unchanged (cheap sim brain per env).
- If it's a **HAC snapshot** → load it into a frozen `HierarchicalActorCritic` (no VarMap grad) once for the rollout, and drive the **right** colony each tick with its mean actions — mirroring the left-side forward but writing to colony 1 (`apply_ai_decision(1,…)`, `apply_commander_intent(1,…)`, `apply_ant_modulators(1,…)`). Because all envs share one snapshot, the right side batches into a single forward, same shape as the left.

Changed: **`phase3.rs`** wires the new config + the snapshot-save + eval cadence.

## 5. Data flow (one training iteration)

1. Sampler picks opponent O (archetype or HAC snapshot) for this rollout.
2. `collect_rollout`: left = training HAC (records, gradient source); right = O (frozen — archetype sim brain OR frozen-HAC snapshot, **no gradient**).
3. `joint_update` on the left records (unchanged — the training path numerics are untouched when O is an archetype, preserving the 47%-baseline + chunked-equivalence tests).
4. Record the match outcome into `pool.record_result(O, hac_won)`.
5. Every `snapshot_every` iters: save training HAC → `pool.add_snapshot`; run eval; **write ledger entry + run `backup_checkpoints.ps1`**.

## 6. Config (all tunable, sensible defaults)

```
self_play_enabled: bool       = false   // off = byte-identical to today's archetype-only training
snapshot_every:    usize       = 25      // iters between snapshots
pool_cap:          usize       = 8       // max HAC snapshots (archetypes always kept)
opponent_sampling: enum        = Pfsp    // Uniform | Pfsp
archetype_mix:     f32         = 0.5     // P(sample an archetype vs a snapshot) under Pfsp
pfsp_power:        f32         = 1.0     // weight = (1 - winrate)^power
snapshot_dir:      PathBuf               // where snapshots are saved
```

Default `self_play_enabled=false` ⇒ existing runs reproduce bit-for-bit (critical: the grad-clip/combat numerics must not move).

## 7. Memory / perf

- One opponent per rollout ⇒ only **one** snapshot HAC resident at a time (load-on-demand for the rollout's opponent). ~45MB. No pool-wide GPU residency.
- Frozen-HAC opponent roughly **doubles the rollout's net-forward cost** (right side now also forwards) — acceptable; no backward pass on the right. Fits the 16GB card (A1-sized, like the 10.7GB combat run).

## 8. Measurement

- **7-archetype eval (mpe=50, dual-metric)** stays — the forgetting guard. If self-play tanks the bench score, we're over-specializing to self-play.
- **Self-play health metric:** training-HAC win-rate vs the current snapshot pool. Steady ~50% = healthy continual improvement (each new self beats the old by a bit); runaway →1.0 or →0.0 = a problem (opponents too weak / policy collapse).

## 9. Error handling

- Bad/missing snapshot load → log + fall back to a `heuristic` opponent for that rollout (mirrors the existing bad-league-spec fallback in `parallel_env`).
- Empty snapshot pool early (before the first snapshot) → sampler returns archetypes only.

## 10. Testing (TDD)

Unit (fast, no sim):
- `SnapshotPool` add/evict: cap respected, oldest snapshot evicted, archetypes never evicted.
- `record_result`: EMA updates correctly.
- `OpponentSampler::Pfsp`: with synthetic win-rates, low-win-rate opponents are sampled more; `archetype_mix` honored; deterministic given a seeded RNG.
- `Uniform`: covers all entries.

Integration (smoke):
- Rollout with a HAC-snapshot opponent: both colonies receive HAC-driven decisions; all records finite; `joint_update` runs.
- `self_play_enabled=false` ⇒ rollout/update identical to current (regression).

Regression (must stay green): the 47%-baseline numerics, `chunked_ant_update_matches_monolithic`, `chunked_matches_monolithic_with_grad_clip`, `phase3_smoke`.

## 11. Hard requirement — logging discipline

Per Matt (2026-06-19): no training forgotten. Every self-play run gets a `projects/antcolony-rl-training-log.md` entry (config, curve, both eval metrics, self-play health, checkpoint location + git SHA), and `scripts/backup_checkpoints.ps1` is run after — snapshots included. A run is not "done" until logged + backed up.

## 12. Open questions for review

1. **Snapshot trigger:** fixed cadence (`snapshot_every`) vs gated on improvement (only snapshot if the new policy beats the pool average)? Default = fixed cadence (simpler); gating is a small add.
2. **Reward for self-play:** keep `combat.toml` (the 0.874 winner) as the self-play reward, or r6? Default = `combat.toml` (it's our strongest policy; self-play should build on it).
3. **Warm-start:** seed the pool with the current SOTA (`phase3-a1-combat/hac_best`) as snapshot #0 so self-play starts from strength, or start fresh? Default = warm-start from the SOTA.

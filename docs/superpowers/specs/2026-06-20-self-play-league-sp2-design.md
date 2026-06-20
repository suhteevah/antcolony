# SP2 — Exploiter League (Design Spec)

**Date:** 2026-06-20
**Status:** design — awaiting approval before writing-plans
**Project:** antcolony hierarchical brain (HAC) RL
**Goal:** Beat the 0.874 combat SOTA via an AlphaStar-style exploiter league — main agent + main-exploiters + league-exploiters — that hunts and fixes the main's weaknesses, pushing past the peer plateau SP1 hit. **Single-GPU round-robin** (multi-GPU concurrency = SP3, deferred — the CPU sim is the bottleneck, so round-robin ≈ concurrent for now).

## 1. Why (what SP1 proved, what SP2 fixes)

SP1 established: plain PFSP self-play with the **terminal-dominant reward** (combat-shaping OFF) is *viable* — it stopped the collapse and reached a **near-peer** (0.395 head-to-head vs the SOTA, ties the bench) — but it **plateaus at peer**, oscillating around the SOTA without exceeding it. The textbook lever to push a peer *past* the incumbent is **exploiters**: dedicated agents that hunt the main's specific weaknesses, whose snapshots then join the main's opponent pool, forcing the main to fix those weaknesses. That is SP2.

## 2. Reuse from SP1 (do NOT rebuild)

`self_play.rs` (`SnapshotPool`, `OpponentSampler` PFSP, `load_frozen_hac`), the frozen-HAC opponent path in `parallel_env::collect_rollout`, `--warm-start-policy`, `assets/reward/terminal.toml`, `eval.rs::{evaluate_hac, evaluate_h2h}`, the phase3 self-play loop. SP2 is mostly orchestration *on top* of these.

## 3. Agent roles

- **Main agent (×1):** the policy we ship. Warm-starts from the 0.874 SOTA. Trains via **PFSP over the whole league** (archetypes + all snapshots). Its snapshots periodically join the league (role-tagged `Main`).
- **Main exploiters (×`n_main_exploiters`, default 1):** each trains **only against the current main agent** (a frozen snapshot of the main, refreshed each league-step). Purpose: find the main's specific weaknesses. **Promotion:** when its win-rate vs the main ≥ `exploiter_promote_winrate` (default 0.70) over the last `exploiter_eval_window` matches, snapshot it into the league (role `MainExploiter`) **and reset** (re-init from the main's *current* weights, so the next hunt targets the improved main). **Forced reset:** also reset after `exploiter_max_iters` even if it never hit threshold (abandon a dead-end exploit, try fresh).
- **League exploiters (×`n_league_exploiters`, default 1):** train via **PFSP over the whole league**. Purpose: find *systemic* weaknesses any league member exploits. **Promotion/reset:** same pattern but measured vs the league (win-rate over its sampled opponents ≥ threshold), reset from a random top-quartile league member (or SOTA).

The main improves because the league it trains against keeps gaining snapshots that *specifically beat it* — it can't plateau; it must learn to beat its own exploiters.

## 4. Architecture / new units

- **`crates/antcolony-trainer/src/exploiter_league.rs`** (new) — the orchestration:
  - `Role { Main, MainExploiter, LeagueExploiter }`.
  - `LeagueEntry` already in `self_play.rs` gains a `role: Role` (extend `OpponentKind::Snapshot` with a role tag, or add a parallel field).
  - `LeagueAgent` — wraps a `JointPpoTrainer` + its `Role` + its opponent-sourcing policy + win-rate tracker + reset/promote bookkeeping. Pure orchestration over the existing trainer; it does NOT reimplement PPO.
  - `LeagueManager` — owns the shared `SnapshotPool`, the main `LeagueAgent`, and the `Vec<LeagueAgent>` of exploiters. Runs the round-robin, refreshes the "current main" frozen snapshot each league-step, evaluates promotion/reset per agent, and periodically runs the success eval. Single source of truth for the league.
- **`crates/antcolony-trainer/src/bin/phase3_league.rs`** (new) — the SP2 driver bin + CLI (mirrors `phase3_train`).
- **Opponent sourcing per role** reuses `collect_rollout`'s frozen-HAC path: main-exploiter's opponent = the frozen current-main snapshot; main + league-exploiter = PFSP over the pool. The "drive right colony with a specific frozen HAC" already exists; SP2 just chooses *which* snapshot.

## 5. Round-robin schedule (single-GPU)

One **league-step** = (a) snapshot the current main to a `current_main.safetensors` the main-exploiters use as their fixed opponent; (b) train **main** for `iters_main` iters (PFSP over league); (c) train each **main-exploiter** for `iters_exploiter` iters (vs current_main); (d) train each **league-exploiter** for `iters_exploiter` iters (PFSP over league); (e) run promotion/reset checks for every exploiter; (f) every `main_snapshot_every` league-steps, add the main's snapshot to the league; (g) every `eval_every_steps`, run the success eval. Repeat for `league_steps`. All agents share the one 16GB card sequentially.

## 6. Config (defaults)

```
league_steps:            usize  = 40
iters_main:              usize  = 25      // main iters per league-step
iters_exploiter:         usize  = 15      // per exploiter per league-step
n_main_exploiters:       usize  = 1
n_league_exploiters:     usize  = 1
pool_cap:                usize  = 16      // bigger than SP1's 8 (more diverse archive)
exploiter_promote_winrate: f32  = 0.70
exploiter_eval_window:   usize  = 20      // matches to measure exploiter win-rate
exploiter_max_iters:     usize  = 100     // forced reset budget
main_snapshot_every:     usize  = 2       // league-steps between main snapshots
opponent_sampling:       Pfsp { archetype_mix: 0.5, power: 1.0 }
reward:                  terminal.toml
warm_start_policy:       <the 0.874 SOTA>
eval_every_steps:        usize  = 5       // success-eval cadence
success_mpe:             usize  = 20      // h2h vs SOTA matches per eval
```

## 7. Success metric (the win condition)

Every `eval_every_steps`, run **`evaluate_h2h(main, SOTA, success_mpe)`** + the 7-archetype `evaluate_hac(main)` (forgetting guard). **SP2 succeeds when the main beats the SOTA head-to-head > 0.5** (confirm the final best at mpe=50). Keep-best persists the main checkpoint with the highest h2h-vs-SOTA. The 0.874 SOTA stays the keeper until a checkpoint clears it.

## 8. Error handling / safety

- Bad/missing snapshot load → skip that opponent, log, continue (reuse SP1's fallback).
- Pool eviction: FIFO within each role, but **never evict the SOTA seed or the current best-main**; archetypes never evicted.
- Single-GPU: one agent trains at a time; the GPU is freed/restored via the proven signal-trapped wrapper. ~hours per run → coordinate the window via openclaw main; nightdrive-clear check.
- Every run: ledger entry + checkpoint backup (the standing discipline).

## 9. Testing (TDD)

Unit (fast, no sim): `Role` tagging in the pool; promotion logic (win-rate ≥ threshold → promote+reset; max-iters → forced reset); reset re-inits from the right source; round-robin visits every agent; pool eviction never drops SOTA/best-main/archetypes.
Integration (smoke, eval-light): a 2-league-step run with 1 main + 1 main-exploiter + 1 league-exploiter completes, snapshots accumulate with correct role tags, the main trains, a promotion+reset fires when forced. `self_play_enabled=false` / non-league paths unchanged (the existing phase3 + SP1 tests stay green).

## 10. Scope (YAGNI)

IN: full 3-role league, single-GPU round-robin, A1, terminal reward, h2h-vs-SOTA success metric.
OUT (later): multi-GPU concurrency (SP3 — only worth it if single-GPU SP2 beats the SOTA and we want it faster/bigger); A2 sizing (needs SP3); learned matchmaking beyond PFSP; >2 of any exploiter role.

## 11. Open questions for review

1. **Exploiter reset source:** main-exploiter resets from the *main's current weights* (default — specializes to the current main) vs from the SOTA (more diverse). Default: current main.
2. **n exploiters:** 1 main + 1 league (default, cheapest round-robin) vs 2+1. Default 1+1; bump if it stalls.
3. **First run length:** 40 league-steps (~40×(25+15+15)=2200 iters, multi-hour) — start there, or a shorter 15-step pilot first to validate the machinery before the long run? Default: 15-step pilot → then 40.

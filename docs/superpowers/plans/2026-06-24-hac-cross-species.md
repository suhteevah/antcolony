# Plan — Cross-species curriculum on the HAC (joint_ppo)

**Date:** 2026-06-24
**Motivation:** The flat MlpBrain, now that its trainer is fixed, plateaus dead-even
with HeuristicBrain in the cross-species intransitive meta (0.502 overall, flat
across all snapshots; breeder-heavy/shrinking strategy — see HANDOFF 2026-06-24).
The HAC (hierarchical, `joint_ppo.rs`) is the capacity lever (produced the 0.871
SOTA). Goal: train the HAC in the cross-species nest+venom arena and see whether a
more expressive brain learns non-degenerate play (beats the heuristic).

**⚠ Key risk:** the HAC has BOTH critical bugs from the flat path, currently LATENT
(it works only because the same-species bench keeps inputs bounded → no
saturation). They WILL fire in the cross-species arena (1e6 features). Fixing them
means editing the SOTA-producing code → must protect the 0.871 HAC from regression.

## Findings (from parallel research agents, 2026-06-24)

- **Training entry:** `bin/phase3_train.rs:159 run_phase3` → loop `phase3.rs:142-196`
  → `ParallelEnv::collect_rollout` + `JointPpoTrainer::joint_update`.
- **Env construction (parallel):** `parallel_env.rs:197-224` — every env is
  `MatchEnv::new(seed)` (same-species 32×32). Per-env seed
  `base_seed ^ (i * 0x9E3779B97F4A7C15)`. Self-play rollout (`joint_ppo.rs:130`)
  also uses `MatchEnv::new(seed)`.
- **Determinism guards to preserve:** opponent-selection RNG is SEPARATE from the
  training RNG (`parallel_env.rs:144-157`) → byte-identical when self-play off;
  default `cross_species=None` must reproduce the legacy path exactly.
- **HAC obs (NOT normalized):** commander 17-dim `obs_to_tensors.rs:92-105`
  (food_stored ~1e6, counts, enemy_distance_min clamped 1e6); ant cone+internal.
  No input_mean/std anywhere in `hierarchical/*`.
- **HAC log-prob mismatch:** `sample_commander` true-u logprob (`actor_critic.rs:95-119`)
  vs `log_prob_of_commander_action` clamped-atanh recompute (`actor_critic.rs:251-256`);
  ratio built from the mismatch at `joint_ppo.rs:489` (commander) and `:500` (ant).
- **Small-init heads:** PRESENT (commander.rs:122, ant.rs:74, std 0.01) → protects
  from init-collapse but NOT from saturation-driven ratio collapse.

## Tasks (TDD/SDD; each gated additive, same-species numerics preserved)

1. **HAC log-prob consistency.** Store the true pre-squash `u` for commander +
   ant in the rollout buffer; compute BOTH old and new log-probs via the same
   path from `u` (no atanh round-trip), mirroring the flat fix. Regression test:
   fresh-rollout importance ratio ≈ 1 for all samples. **Must preserve same-species
   numerics** (prove ratio path equivalent when not saturated, or gate).
2. **HAC observation normalization.** Add input_mean/std buffers (commander state,
   ant cone, ant internal) to `HierarchicalActorCritic`; apply z-score in
   `commander.forward`/`ant.forward`; add `fit_observation_normalization` (warm-up
   rollouts, std floor 1e-2). Default buffers = identity (mean0/std1) so existing
   checkpoints/tests are byte-identical until fit. Export/import with the checkpoint.
3. **CrossSpeciesCurriculum on the HAC.** Add `cross_species: Option<...>` to
   `JointPpoConfig` + `ParallelEnv`; branch env ctor at `parallel_env.rs:199` and
   `joint_ppo.rs:130` to `new_cross_species_nest_arena` + venom (copy the flat
   seam, ppo.rs:144-161). Default None = byte-identical legacy.
4. **Driver flags.** `--cross-species-nest <dir>`, `--venom-cycle`, fit-norm on
   cold start, in `phase3_train.rs` (or a thin new bin).
5. **Tests + smoke.** Byte-identical-when-off guard (collect_rollout with
   cross_species=None matches baseline); cold cross-species smoke moves the HAC
   actor + ratio≈1; a short real run.
6. **Regression gate.** Re-score the existing 0.871 SOTA via the existing
   harness/eval to confirm no regression from tasks 1-2.
7. **Run + eval.** Train the HAC cross-species (CPU kokonoe or GPU/P100 via
   ParallelEnv batch — coordinate per cnc gate); score with `eval_mlp_vs_heuristic`
   (works on any MlpBrain JSON; HAC export path may need a thin adapter or use the
   HAC's own eval). Verdict: does the HAC beat HeuristicBrain in the meta?

## Cheaper alt to try first (optional, ~30 min)
Small-init the FLAT `ActorCritic` mean head (actor_l3) — the proven HAC fix —
and retrain. Lower probability (flat outputs are already varied, not collapsed),
but cheap and might break parity without the big HAC build.

# PPO r5 Postmortem — Population-Based RL + Curriculum

**Date:** 2026-05-04
**Question:** Can population-based RL (warm-start MLP_v1 in league + self-snapshots) and curriculum opponent sampling break the 45.7% mean win rate ceiling vs the 7-archetype bench?

## TL;DR

**No** — but the "45.7% ceiling" was largely an eval-noise artifact. With 50 matches per opponent (vs the previous 20), the MLP_v1 baseline measures at **47.1%**, not 45.7%. PPO r5 (warm-start + pop + curriculum) holds 47.1% through 40 iterations, then drifts ~1pp lower as the league grows. Cold-start r5b (150 iter, no warm-start) lands at **38.6%** — the value-head burns most of its budget converging from random init.

**New SOTA: 47.1% (MLP_v1, re-measured at 50 matches per opp). Same weights as before — no improvement, only better measurement.**

The real finding: **gradient updates with current PPO settings are too small to flip discrete decisions through the MlpBrain inference path.** Multiple snapshots (different file hashes, weights actually changed) produced **identical match outcomes** under the deterministic evaluator. The policy is moving in weight space but not in behavior space.

## Setup

New flags added to `crates/antcolony-trainer/src/bin/ppo_train.rs`:

- `--include-baseline <path>` — adds an MLP weights JSON to the league as a tier-2 entry. Forces the trainee to differentiate from itself if warm-started against the same weights.
- `--snapshot-every N` — every N iterations, dump current weights to `snapshots/snap_itNNNN.json` and add to the league. Classic population-based RL.
- `--curriculum` — opponent sampler weights tiers by training progress: tier 0 (heuristic) fades from 1.0 → 0.3, tier 1 (single-axis archetypes) flat at 1.0, tier 2 (MLP / self-snapshots) ramps 0.2 → 2.0.

`League.sample_curriculum(progress, rng)` implements the weighted draw; `LeagueEntry.tier` is the new field driving it.

## r5: Warm-start + pop + curriculum (60 iter × 12 matches)

```
--start mlp_weights_v1.json
--include-baseline mlp_weights_v1.json
--snapshot-every 10
--curriculum
```

**Eval @ 20 matches per opp (noisy):**

| snapshot | result |
|---|---|
| MLP_v1 (baseline) | 64/140 (45.7%) |
| snap_it0010 | 64/140 (45.7%) |
| snap_it0020 | 64/140 (45.7%) |
| snap_it0030 | 64/140 (45.7%) |
| snap_it0040 | 64/140 (45.7%) |
| snap_it0050 | 57/140 (40.7%) |
| current (it60) | 57/140 (40.7%) |

**Eval @ 50 matches per opp (tighter):**

| snapshot | result |
|---|---|
| MLP_v1 (baseline) | 165/350 (47.1%) |
| snap_it0010 | 165/350 (47.1%) |
| snap_it0040 | 165/350 (47.1%) |
| current (it60) | 162/350 (46.3%) |

## r5b: Cold-start + pop + curriculum (150 iter × 16 matches)

```
--include-baseline mlp_weights_v1.json
--snapshot-every 25
--curriculum
(no --start)
```

Loss decay was clean (it1: 2.9B → it80: 250k) but the policy never reached MLP_v1 quality. Final eval at 50 matches: **135/350 = 38.6%** (vs 47.1% baseline). Cold-start needs either way more iterations, a stronger curriculum (heuristic-only warm-up phase), or shaped reward to bootstrap.

## Findings

### 1. The "45.7% ceiling" is eval-noise

Standard error on 20-match-per-opp = sqrt(p*(1-p)/n) ≈ 11% per opp = ~4pp on the 7-opp aggregate. The 45.7% / 47.1% gap is within that noise. The 50-match number (47.1%) is the more reliable baseline going forward.

### 2. Identical aggregate results across non-identical weights

`snap_it0010`, `snap_it0040`, and `MLP_v1` all produced **the exact same 165/350** with **different file hashes**. `MlpBrain` inference is deterministic; with identical seeds, decisions only diverge if weights change enough to flip a `softmax` argmax or a clamp boundary. PPO at `lr=5e-4` with `entropy_coef=0.005` is making weight-space moves too small to matter behaviorally.

### 3. Curriculum sampler works as designed

By iteration 60, opponent draws were dominated by tier-2 (`baseline_0` + `snap_it*`) — log line:
```
opps=baseline_0=1,breeder=1,conservative=2,forager=2,
     snap_it0020=1,snap_it0030=1,snap_it0040=1,snap_it0050=3
```
Mechanic is correct; it just doesn't help when gradients can't move the policy.

### 4. Loss spikes (115k, 170k, 77k) point to value-head divergence

When new opponent types enter the league mid-training, the value function gets stale predictions vs novel returns. Spike → drift → 1pp degradation. A value-loss clip (PPO's value-clipping trick) would likely help here.

## What didn't break the ceiling

- Adding MLP_v1 itself to the league as tier-2 opponent (per literature pop-based RL recipe)
- Adding self-snapshots every 10 iterations (5 snapshots in league by it60)
- Curriculum-weighted opponent sampling

## What might

- **Bigger gradient steps without divergence:** value-loss clipping + slightly higher entropy coefficient
- **Wider eval bench:** 7 deterministic archetypes may have ~47% Nash; add stochastic / mixed brains
- **Action-space mismatch:** MLP outputs 6 colony-level params; ant-level tactics may demand finer control
- **Reward shape:** current `worker_delta * 0.01 + terminal ±1` is loosely informative; perhaps reward food-stored, queen-survival, territory-area as additional signals
- **Different network:** 17→64→64→6 might just be undersized for the action space — but JSON format is fixed at 64-64

## Files

- Trainer: `crates/antcolony-trainer/src/league.rs`, `bin/ppo_train.rs`
- Run output: `bench/ppo-rust-r5/`
- Eval script: `scripts/eval_ppo_r5.ps1`

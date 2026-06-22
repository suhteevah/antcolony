# Ladder League — Iterated Best-Response vs the Frozen Tournament Ladder

**Date:** 2026-06-21
**Status:** Design approved (brainstorming), pending spec review → implementation plan
**Branch:** `feat/ladder-league`

## Problem

The 0.874 combat SOTA (`bench/phase3-a1-combat/hac_best.safetensors`) tops the PvP
tournament ladder (Elo 1522), but `sp1term` (the terminal-reward self-play brain) is a
dead-even #2 (Elo 1483; `sota` wins their head-to-head only **0.517**). Three prior
attempts to *exceed* the SOTA via recursive learning all failed the same way:

- **SP1 #1 (combat reward, fresh init):** catastrophic forgetting — warm-start seeded
  only the opponent pool, not the training policy; the fresh policy ground itself against
  strong opponents and shed general skill (bench 0.314, h2h 0.01).
- **SP1 #2 (combat reward, warm-started policy):** still degraded (0.829→0.286); keep-best
  kept iter-0. The self-play *gradient* pulled off the fixed objective.
- **SP2 (exploiter league):** exploiters never engaged (0 promotions in 15 steps) → the
  league degenerated into plain PFSP self-play against the main's **own drifting
  snapshots** → 3rd drift confirmation.
- **SP1 #3 (terminal reward) — the one that held:** warm-started, terminal.toml,
  oscillated *around* the SOTA without collapsing, reached near-peer (h2h 0.395–0.48). It
  proved recursion is *viable* but didn't *exceed* peer in cheap runs.

**Common root cause:** every run trained the main against opponents that were *themselves
moving* (its own evolving snapshots) → a feedback loop that let the objective drift.

**The unlock the tournament provides:** a strong, diverse, **frozen** opponent set. If the
opponents can't move, drift is structurally impossible. This design trains best-responses
against that frozen ladder and only admits new opponents that the tournament *validates*
as stronger — iterated fictitious play, gated by the tournament.

## Goal

A self-driving loop that:
1. Produces a brain ranking **#1** on the tournament ladder with a **clear** head-to-head
   margin over the prior SOTA (≥ 0.55 at mpe=50 — not the 0.517 coin-flip), AND
2. **Stops itself** and declares the ceiling when it cannot improve for K consecutive
   rounds (an honest, publishable "A1 + this objective is at its ceiling" result, not a
   silent failure).

## Non-Goals (YAGNI)

- No multi-GPU data-parallel training (deferred `multi_gpu.rs`); A1-sized, single P100.
- No N-colony / FFA — the engine stays hard-2-colony (as the tournament already is).
- No new reward shaping — `assets/reward/terminal.toml` is the proven non-collapsing reward.
- No classic mutating-population PBT — rejected in brainstorming (non-stationary opponents
  are the failure mode we're escaping).
- No change to the existing flat trainer, phase3, SP1, or SP2 code paths — this is additive.

## Architecture — the round loop

```
SOTA  := sota                                    # 0.874 combat keeper
POOL  := frozen { sota, sp1term, sp1, gradclip, sp2 } + 7 archetypes   # all FROZEN
no_improve := 0

loop round = 1 .. MAX_ROUNDS:
  1. TRAIN candidate
       warm-start training policy = SOTA
       PFSP vs the FROZEN POOL (read-only for the whole round)
       reward = terminal.toml
       keep-best on h2h-vs-SOTA, eligible only if archetype-bench ≥ FLOOR
  2. GATE candidate
       h2h candidate vs every POOL member (side-swapped, mpe=GATE_MPE)  # one row
       PASS iff  candidate winrate-vs-pool ≥ SOTA's standing winrate-vs-pool
             AND  h2h(candidate, SOTA) ≥ GATE_MARGIN
  3a. PASS → add candidate to POOL (frozen, protected); SOTA := candidate
            run a full tournament re-rank (Elo + cycles) as the milestone artifact
            Telegram ping; no_improve := 0
  3b. FAIL → no_improve += 1; if no_improve ≥ NO_IMPROVE_STOP → STOP (ceiling)
```

The pool only ever grows with **tournament-validated-stronger** brains, and every opponent
is frozen. That is the entire mechanism by which this avoids the drift that killed SP1/SP2.

## Components

### C1. Round trainer (mostly reuse)
- **Reuses:** `self_play.rs` `SnapshotPool` + `OpponentSampler::Pfsp{archetype_mix, power}`,
  `load_frozen_hac`; `parallel_env.rs` batched rollout (left = training HAC, right =
  PFSP-sampled frozen opponent); `joint_ppo.rs` update; the `phase3.rs` driver shape
  (rollout → update → periodic eval + checkpoint).
- **The one substantive change vs SP1/SP2:** the pool is **read-only within a round** — the
  trainer NEVER calls `add_snapshot` with the main's in-progress weights. New brains enter
  the pool ONLY via the gate, between rounds. (SP2's degradation feedback loop is removed by
  construction.)
- `archetype_mix ≈ 0.30`: PFSP spends most gradient on the strong HAC matchups it must beat
  (`sota`, `sp1term`) while archetypes preserve general skill (anti-forgetting; SP1's miss).
- **Keep-best eligibility floor:** the per-round keep-best metric is h2h-vs-current-SOTA, but
  a checkpoint is only *eligible* to be kept if it still scores ≥ `KEEPBEST_ARCH_FLOOR`
  (default 0.70) on the 7-archetype bench. Prevents "win by forgetting everything to beat
  one opponent."

### C2. Tournament gate (new, thin)
- **Reuses:** `eval.rs::play_pair` / `evaluate_h2h` (side-swapped, both metrics).
- The gate is **one row** — candidate vs each current pool member — not a full N² round-robin.
  Cheap (N matches, not N²). It deliberately does NOT recompute the full ladder; instead it
  compares the candidate against a **standing bar**.
- **Standing bar:** at loop init, compute the current SOTA's winrate-vs-pool (one row vs the
  exact frozen pool). Because the pool is frozen, that bar stays valid until the pool changes
  (i.e., until a promotion), at which point the newly-promoted SOTA's own gate row becomes the
  new bar. So the bar is always available without an extra N² pass.
- **Pass test (both required):**
  1. candidate's mean winrate-vs-pool ≥ the SOTA's standing winrate-vs-pool, AND
  2. `h2h(candidate, SOTA) ≥ GATE_MARGIN` (default 0.55) at `GATE_MPE` (default 50).
- A **full** `run_tournament` re-rank (authoritative Elo + #1 confirmation + 3-cycle report)
  runs ONLY on a successful promotion — the milestone artifact + Telegram payload, not the
  per-round gate. If the full re-rank ever disagrees with the cheap gate (candidate not
  actually #1), that's logged and the promotion is rolled back (gate tightened).

### C3. Promotion + pool management
- On PASS: `pool.add_protected_snapshot(candidate)` (frozen, never evicted), `SOTA :=
  candidate`, next round warm-starts from it. The pool grows by exactly one validated brain
  per successful round (iterated FSP).
- The original SOTA seed and all archetypes are `protected` (never evicted).
- Pool growth is unbounded by `pool_cap` for *protected* entries (they don't count toward
  the cap) — but in practice rounds are few; if growth ever matters, cap the count of
  promoted brains kept and log the drop (no silent truncation).

### C4. Orchestrator (new binary)
- `crates/antcolony-trainer/src/bin/ladder_league.rs` + `scripts/run_ladder_league_cnc.sh`.
  Additive — phase3 / SP1 / SP2 byte-unchanged.
- **Determinism:** a separate orchestration RNG; per-round/opponent seed =
  `base_seed ^ (round << 32) ^ (idx << 16)` — the same discipline that fixed SP1's
  shared-training-RNG critical bug. The training RNG is never drawn from for orchestration.
- **Stopping:** stop after `NO_IMPROVE_STOP` (default 2) consecutive no-promotion rounds; also
  a `MAX_ROUNDS` cap and an optional wall-clock/compute cap. Stops are logged as the ceiling
  result, not errors.
- **Observability:** per-round log + checkpoint (`bench/ladder-league/round_NN/`);
  **Telegram ping on every promotion and on final stop** (new SOTA's Elo, h2h margin, cycles).

## Data flow

```
frozen POOL ──PFSP──▶ parallel_env rollout (left=training HAC, right=frozen opp)
                          │
                          ▼
                    joint_ppo update ──▶ candidate checkpoint (keep-best, arch-floor-gated)
                          │
                          ▼
                    GATE: evaluate_h2h(candidate, each pool member)  [CPU+rayon]
                          │
              ┌───────────┴───────────┐
            PASS                     FAIL
              │                        │
   add to POOL (protected)     no_improve++ ; stop if ≥ K
   SOTA := candidate
   full run_tournament re-rank
   Telegram ping
```

## Venue / compute

- **cnc P100, A1-sized.** Training uses the GPU (the proven self-play path, `--features
  cuda`); the gate eval runs CPU + rayon (like the tournament — the sim is the bottleneck,
  the GPU loafs at ~40% during these runs).
- Reward: `assets/reward/terminal.toml`. ~150 iters/round (run #3 peaked by iter50, so this
  is margin). Full-fleet-kick + `RAYON_NUM_THREADS=nproc` for the CPU-bound sim — coordinate
  the cnc window via openclaw `main` (CPU-contention shape: see the tournament run; main may
  recommend `nproc-1` on a daytime window).
- A few rounds ≈ overnight; the machine never sleeps.
- ⚠ A ship to cnc wipes `/opt/antcolony-cuda/bench/` — restore keepers (and the pool's
  frozen checkpoints) from `/opt/antcolony-archive/` first. The CUDA build needs the
  `g++-13`/`CUDA_COMPUTE_CAP=60`/split-`LD_LIBRARY_PATH` recipe (cnc gotchas Bonus 5).

## Testing

- **Unit:**
  - frozen-pool invariant: the pool is byte-unchanged across a full round's rollout/update.
  - gate pass/fail logic: rank-#1 AND margin both required; coin-flip (0.51) fails.
  - keep-best eligibility: a checkpoint below the archetype floor is never kept even if its
    h2h-vs-SOTA is highest.
  - seed determinism: orchestration seed derivation is reproducible and never perturbs the
    training RNG (additive-byte-identical guard, like SP1/SP2).
- **Smoke:** 2 rounds at tiny scale (few iters, small mpe) — proves train → gate →
  promote/stop wiring end-to-end, exit 0, artifacts written.
- **Regression:** existing trainer suite stays green (phase3/SP1/SP2 byte-unchanged).

## Success criteria

- **Win:** a promoted brain that re-ranks **#1** on the full tournament with
  `h2h(new, original-SOTA) ≥ 0.55` at mpe=50.
- **Honest null:** the loop stops after `NO_IMPROVE_STOP` no-improve rounds → a real,
  reportable "A1 + the frozen-ladder objective is at its ceiling" answer. This is a valid,
  publishable outcome, not a failure to hide.

## Tunable knobs (defaults; all CLI-overridable)

| Knob | Default | Why |
|---|---|---|
| `archetype_mix` | 0.30 | focus PFSP gradient on strong HAC matchups, keep archetype breadth |
| `power` (PFSP) | 1.0 | SP1/SP2 default; oversample lost matchups |
| `keepbest_arch_floor` | 0.70 | can't "win" by forgetting the archetypes |
| `gate_margin` | 0.55 | promote real winners, not 0.51 coin-flips |
| `gate_mpe` | 50 | the honest-measure mpe (mpe=5 training evals are noisy) |
| `no_improve_stop` | 2 | loop-until-dry; stop when 2 rounds fail to promote |
| `iters_per_round` | 150 | run #3 peaked by iter50 → margin |
| `max_rounds` | 8 | hard cap |
| initial pool | `{sota, sp1term, sp1, gradclip, sp2}` + 7 archetypes | drop `v1` (rank 10, adds little) |

## Open questions for spec review

- Gate margin 0.55 — right bar, or tighter/looser?
- iters/round 150 — or longer rounds for a better best-response (at more compute)?
- Include `v1` / the weak HAC checkpoints in the frozen pool for breadth, or keep it lean?

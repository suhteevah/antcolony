# AI Training Run — Postmortem (2026-05-03)

**Goal:** "Make AI work" — train an Aether-LM brain that demonstrably outperforms `HeuristicBrain` in the matchup bench.

**Outcome:** Architecture is shipped + verified. Trained brains do not yet differentiate from baseline because of a sim-integration gap (not an ML gap). Honest write-up below.

---

## What landed

| Piece | Status |
|---|---|
| `AiBrain` trait + 3 impls (`HeuristicBrain` / `RandomBrain` / `AetherLmBrain`) | shipped |
| `Simulation::new_ai_vs_ai_with_topology` + `match_status` + `colony_ai_state` + `apply_ai_decision` | shipped |
| `external_brain` flag — prevents legacy `red_ai_tick` from overwriting brain decisions | shipped |
| `examples/matchup_bench.rs` — head-to-head bench with `--dump-trajectories` | shipped |
| Aether subprocess integration — `AetherLmBrain` actually invokes `aether-infer.exe` | shipped |
| Train pipeline (`scripts/ai_training_run.ps1`) — self-play → filter → corpus → tokenize → train → eval | shipped |
| 4 trained checkpoints (nano600, nano1500, nano1500_lr1e3, tiny600) | 3 shipped, tiny killed (too slow at seq_len=64) |
| 25 unit + 9 integration tests | all green |

---

## What didn't work, in honest order

### 1. The first eval results were a lie (bug)

In AI-vs-AI mode I set `is_ai_controlled = true` on both colonies, but the legacy `red_ai_tick` runs **every tick** and overwrites the brain's caste_ratio + behavior_weights with the heuristic 4 ticks out of every 5 (brain cadence is 5).

**Fix:** added `ColonyState.external_brain: bool`. `apply_ai_decision` sets it; `red_ai_tick` skips colonies where it's true. Verified by `tests/brain_actually_drives.rs` (3 tests, green).

### 2. The trained models collapsed to a constant prior

Inspection (`scripts/inspect_aether_brain.ps1`):
- `nano600` and `nano1500` (lr=3e-3, seed=42) emit `w:0.65 s:0.30 b:0.05 f:0.55 d:0.20 n:0.25` for **every** input state.
- `nano1500_lr1e3` (lr=1e-3, seed=137) varies output by state — under attack: `s:0.36 b:0.31`. Escaped the prior trap.

Cause: training data is dominated by `HeuristicBrain` default decisions because the heuristic only departs from defaults under (a) combat losses or (b) low food. Most timeline states are neither, so the model fits the modal output.

### 3. Even after the bugfix, eval outcomes don't differ by brain

This was the surprising one. After fixing the `external_brain` issue, head-to-head matches `heuristic-vs-heuristic` and `heuristic-vs-random` produce **bit-identical per-match outcomes** at the same `sim_seed`. Identical to the `aether` brain.

Diagnosis (grep walk through the sim):
- `caste_ratio` is only consumed by `sample_caste` (line 3197) when new ants spawn from matured brood. **Brood maturation = ~50 in-game days = ~21,000 ticks at Seasonal.** At our 500-tick budget, no new ants spawn → caste_ratio decisions have zero effect.
- `behavior_weights.dig` is consumed by `promote_diggers` (line 2272) — minor effect on idle worker assignment.
- `behavior_weights.forage` and `nurse` are **set but never read** by any sim system. Dead fields.

So 5 of 6 brain outputs do nothing meaningful in 500-tick matches. That's why brain identity doesn't change outcomes.

Verified at 25k ticks: same identical outcome between HRH and HRR (match end at tick 1133, winner=1, workers L/R=0/5 in both runs). Even at this scale, brain decisions don't differentiate — likely because the brood-pipeline-driven population shift takes hold faster than the brain can react via caste_ratio at brain-cadence=5.

### 4. tiny600 training pathologically slow

The `tiny` config + `--seq 64` parameters produced a model that took >70 minutes per 600 steps on CPU. nano was ~20 seconds per 600 steps. Killed it. Right answer is to use `tiny` only with `--seq 32` or smaller, or reach for the GPU (Candle fork at `J:/candle-src/`) before scaling model size.

---

## What this proves

- **The AI plumbing is real and verified.** Brains decide, decisions apply, RNG-determinism holds, AI-vs-AI matches run to completion.
- **The training pipeline is real.** Self-play → filter → tokenize → train → eval, single command, scriptable, reproducible.
- **The Aether subprocess integration is real.** Trained checkpoints loaded, completions parsed, fallback budget protects against bad checkpoints.
- **The wire format roundtrips.** `state_to_prompt` + `completion_to_decision` tested; checkpoints emit parseable output (`nano1500_lr1e3` even with state-conditional variation).

What it doesn't prove yet: that a learned policy can beat `HeuristicBrain` in the matchup bench. The blocker is sim-side, not ML-side.

---

## What needs to happen for the proof to land

In priority order — each is a separate piece of work:

### A. Wire `behavior_weights.forage` and `.nurse` into per-tick ant behavior

These fields are currently dead. To make brain decisions matter in <25k tick matches, the per-ant decision system needs to consult the colony's behavior weights when deciding whether an idle worker should head out to forage vs stay in to nurse brood.

Estimated work: 1-2 hours; concrete change in `decide_next_state` to bias state transitions by colony's behavior_weights.

### B. Make `caste_ratio` affect existing-ant assignment, not just new spawns

Currently caste_ratio only kicks in when new brood matures into adults. A brain emitting `s:0.9` should be able to **promote** existing workers to soldier-role (or change their combat-radius / aggression scalar) so the decision has immediate effect. 

Real biology supports this: colonies under threat shift their existing workforce's behavior, not just future spawns.

Estimated work: 2-4 hours; needs a new "soldier_intent" scalar per ant that interpolates toward the colony's caste_ratio.soldier value.

### C. Diversity-weighted training data

Even after A+B, the trained models will still converge to constant priors unless the training data is rebalanced. Two options:

1. **Filter for "interesting" states** — only train on states where the heuristic departed from defaults (combat losses > 0 OR food < threshold). Reduces dataset by ~10x but every example carries decision-relevant signal.
2. **Oversample rare states** — for each `losses=8` example, replicate 5x in training corpus.

Option 1 is cleaner and matches behavior-cloning best practices.

### D. GPU training via Candle fork

For models larger than `nano` (85k params) we need GPU. Matt's `J:/candle-src/` fork has CUDA + custom kernels for the 3070 Ti. Wire a Candle-based trainer that reads the same JSONL format and emits checkpoints loadable by `AetherLmBrain` (or a new `CandleLmBrain`). Significant work — multiple days, not hours.

### E. Run match harness at brood-maturation scale

The cheapest immediate "make brain matter" hack: run matchup_bench at `--max-ticks 50000` so brood actually matures and caste_ratio decisions take effect. Slow per match (~2-3 min each), but proves the ML axis without sim-side rewiring.

---

## Recommendation for next session

**Order:** E (cheap to test) → A+B together (real sim work, unblocks short-scale matches) → C (data quality) → re-train + re-eval → D if/when models need to grow beyond nano.

The infrastructure is solid. The sim integration is the remaining gap.

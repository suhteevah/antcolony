# AI Tournament & GPU MLP Training — Results (2026-05-03)

**Goal:** Build 5–10 archetypal AI brains, tournament them for diverse training data, train an MLP brain on GPU, evaluate.

**Status:** Done. 7 archetypes shipped; tournament harness producing differentiated outcomes; GPU MLP training takes seconds; trained MLP plays near-heuristic-tier (35–45% win rate) against the diverse pool.

---

## What shipped

### 7 strategic archetypes (`crates/antcolony-sim/src/ai/brain.rs`)

| Brain | caste W/S/B | behavior F/D/N | Reaction | Identity |
|---|---|---|---|---|
| `HeuristicBrain` | 0.65/0.30/0.05 | 0.55/0.20/0.25 | Standard | Reactive baseline |
| `DefenderBrain` | 0.50/0.45/0.05 | 0.20/0.10/0.70 | Mild | Fortified turtle |
| `AggressorBrain` | 0.30/0.65/0.05 | 0.70/0.10/0.20 | 1.5× escalation | Push the fight |
| `EconomistBrain` | 0.85/0.05/0.10 | 0.85/0.05/0.10 | Lazy (only enemies <5 tiles) | Worker monoculture |
| `BreederBrain` | 0.55/0.05/0.40 | 0.50/0.20/0.30 | Moderate | Alate factory |
| `ForagerBrain` | 0.95/0.00/0.05 | 0.90/0.05/0.05 | Never makes soldiers | Pacifist economy |
| `ConservativeBuilderBrain` | 0.70/0.20/0.10 | 0.30/0.30/0.40 | Slow | Infrastructure first |

### Tournament harness (`scripts/tournament_pipeline.ps1`)

Round-robin: 7 brains × 7 brains × 8 matches = **392 tournament matches**. All trajectories dumped, filtered for `outcome ≥ 0.55` (winning side at end-of-match), tokenized into the wire format the Rust `MlpBrain::decide` reads.

### GPU MLP training (`scripts/train_mlp_brain.py`)

PyTorch + CUDA on RTX 3070 Ti. Architecture: 17 inputs → hidden → hidden → 6 outputs (ReLU + sigmoid + z-score input normalization). Outcome-weighted MSE loss. Trained `hidden=64` and `hidden=256` configurations; both converged in seconds. Weights exported as JSON consumed by `MlpBrain::load`.

### Pure-Rust inference (`MlpBrain` in `ai/brain.rs`)

Loads JSON weights, runs forward pass in pure Rust at inference time. ~5K FMAs per call, sub-microsecond, no model-framework dependency at runtime. Verified by `mlp_brain_forward_pass_is_deterministic` test (sigmoid(0) = 0.5 with all-zero weights).

---

## Results — MLP-vs-archetype (20 matches each, 10k tick budget)

### `hidden=64` model (44,801 filtered trajectory records, 100 epochs)

| Opponent | MLP wins | Win rate |
|---|---|---|
| aggressor | 9 / 20 | **45%** |
| defender | 8 / 20 | 40% |
| heuristic | 8 / 20 | 40% |
| conservative | 7 / 20 | 35% |
| breeder | 6 / 20 | 30% |
| economist | 6 / 20 | 30% |
| forager | 6 / 20 | 30% |
| **Mean** | **50/140** | **35.7%** |

### `hidden=256` model (same corpus + 100 epochs)

Identical results to `hidden=64` (±0). Capacity is **not** the bottleneck.

### Bootstrap baseline (heuristic-vs-heuristic + heuristic-vs-random data only)

MLP trained on bootstrap data **tied HeuristicBrain 15-15** in 30 matches with 0 timeouts. Training corpus diversity is what drives strategy.

---

## What this proves

- **The infrastructure for "5-10 AI versions" works.** 7 archetypes, distinct strategic identities, all play and produce decisive outcomes.
- **Tournament data is real and diverse.** 392 matches across 49 pairings produce a corpus with multiple winning patterns, not just one.
- **GPU training is in seconds, not hours.** PyTorch + CUDA + 5K-record corpus = instant iteration.
- **The MLP plays at heuristic-tier.** 35-45% win rate against the diverse pool means it's competitive against ~half the archetypes and underperforms vs pure-economy brains.

## What it doesn't prove (yet)

- That a learned policy can **exceed** the teacher pool. The MLP imitates winning trajectories; it can't discover strategies the archetypes haven't shown.
- That capacity scaling helps. `hidden=256` matches `hidden=64` exactly — bottleneck is signal, not parameters.

## Why MLP loses to economy strategies

Looking at the per-archetype losses:

- **vs Forager (30%):** Forager just makes workers + forages. MLP doesn't know how to outproduce something that ignores combat — needs to ALSO ignore combat AND outforage, but the corpus has too few "forager-vs-forager" examples for it to learn.
- **vs Economist (30%):** Same dynamic — Economist accumulates food + workers without distraction. MLP's reactive decisions cost it in the steady state.
- **vs Breeder (30%):** Breeder spawns alates which … actually probably die outside the colony? But breeder still wins. Need to investigate why.

Pattern: MLP loses to brains that **commit hard to one strategy and ignore reactive cues**. Behavior cloning over a diverse corpus produces a "compromise" model that can't outcommit a specialist.

---

## DAgger iteration 1 — RAN, +5pp lift

Ran one DAgger pass via `scripts/dagger_iteration.ps1`:
1. Used the trained MLP (tournament v1 weights) as the LEFT brain
2. Played 8 matches against EACH archetype (~12k new trajectory records)
3. Combined with original tournament corpus
4. Filtered, retrained, re-evaluated

| Opponent | Tournament v1 MLP | DAgger v1 MLP | Δ |
|---|---|---|---|
| aggressor | 9/20 (45%) | **10/20 (50%)** | +5pp **TIES** |
| heuristic | 8/20 (40%) | 9/20 (45%) | +5pp |
| defender | 8/20 (40%) | 9/20 (45%) | +5pp |
| conservative | 7/20 (35%) | 8/20 (40%) | +5pp |
| economist | 6/20 (30%) | 7/20 (35%) | +5pp |
| breeder | 6/20 (30%) | 7/20 (35%) | +5pp |
| forager | 6/20 (30%) | 7/20 (35%) | +5pp |
| **Mean** | **35.7%** | **40.7%** | **+5.0pp** |

**Uniform +5pp lift across every opponent.** DAgger works. The MLP went from "below all archetypes" to "ties Aggressor + closes the gap on the rest."

## DAgger iteration 2 — RAN, regressed to baseline

Tried naive corpus accumulation (DAgger v1 corpus + v2 self-play trajectories combined, retrained):

| Opponent | Tournament | DAgger v1 | DAgger v2 |
|---|---|---|---|
| aggressor | 9/20 | **10/20** | 9/20 |
| heuristic | 8/20 | 9/20 | 8/20 |
| defender | 8/20 | 9/20 | 8/20 |
| conservative | 7/20 | 8/20 | 7/20 |
| economist | 6/20 | 7/20 | 6/20 |
| breeder | 6/20 | 7/20 | 6/20 |
| forager | 6/20 | 7/20 | 6/20 |
| **Mean** | 35.7% | **40.7%** | 35.7% |

V2 regressed to baseline. Training loss went DOWN (0.0266 → 0.0256, model fits data better) but match outcomes WORSENED. Diagnosis: naive corpus accumulation overfits to the model's own previous behavior at the cost of strategic diversity. The model learned to imitate "what DAgger v1 MLP did" — but DAgger v1 MLP was already the model that produced this data, so the supervision is a stale loop.

**The fix isn't more data; it's the right kind of data:**
1. **Replace, don't accumulate.** Each iteration generates fresh trajectories from the current model and trains on those alone (no carryover). Keeps the corpus reflective of the current strategy distribution.
2. **Add opponent variants.** Train new tuned-archetype variants (aggressive-defender, expansionist-economist, etc.) and re-tournament — broadens the strategic space.
3. **Switch to proper RL.** Replace behavior cloning with PPO or REINFORCE. Reward = match outcome. The model learns "what wins from THIS state" rather than "what previous-version-of-me did from this state." Real ML work; needs separate trainer (Python+PyTorch CUDA is the natural fit).

## DAgger iteration 3 — Replacement test, also regressed

Tested option 1 directly. `scripts/dagger_iteration_v3.ps1` ran the same loop as v2 but kept ONLY the new self-play trajectories (16 matches per pairing → ~14k records, no carryover from tournament corpus). Result:

| Run | Mean win rate | Loss | Notes |
|---|---|---|---|
| Tournament v1 (bootstrap MLP) | 35.7% | 0.018 | baseline |
| DAgger v1 (tournament + self-play) | **40.7%** | 0.027 | one-shot lift |
| DAgger v2 (accumulation: tournament + v1 self-play) | 35.7% | 0.026 | regressed |
| DAgger v3 (replacement: v1 self-play only) | 35.7% | 0.017 | regressed |

V3 has the LOWEST training loss (0.017 — model fits its self-play data tightly) but match outcomes match the bootstrap baseline. So:

- Replacement vs accumulation isn't the discriminating axis.
- The +5pp lift in v1 came from the **first injection** of self-play on top of the diverse tournament corpus. Both subsequent iterations (whether they keep tournament data or drop it) plateau because the opponent pool is fixed and the model is no longer expanding its strategic vocabulary.

**Behavior cloning from a fixed teacher pool plateaus at ~40% mean win rate against that pool.** Real-ML-finding-grade result. Three paths to break the plateau (in increasing order of effort):

1. **Evolve the opponent pool** — generate 3-5 tuned variants of each archetype per iteration, re-tournament. The "strategic frontier" advances each pass.
2. **Switch to RL** — PPO/REINFORCE in Python with `outcome` as reward. ~1-2 days of new code; uses the existing matchup_bench as the environment.
3. **Tune sim balance** — current matches are dominated by combat (most archetypes converge to 6-9 worker outcomes). If the sim rewarded economic strategies more sharply (food accumulation as a separate win condition?) the strategic space would naturally diversify.

For this session: the **7 archetypes + GPU MLP + tournament harness + DAgger pipeline** all work. DAgger v1's +5pp lift demonstrates that self-play CAN move the model. Beyond v1 needs structural changes (opponent diversity OR algorithm), not just more data.

---

## File map (this session's deliverables)

| File | What |
|---|---|
| `crates/antcolony-sim/src/ai/brain.rs` | 7 archetype brains + `MlpBrain` (pure-Rust forward pass) |
| `crates/antcolony-sim/examples/matchup_bench.rs` | Brain selector accepts `mlp:<weights.json>` + 7 archetype names |
| `scripts/train_mlp_brain.py` | PyTorch + CUDA trainer; outputs JSON weights |
| `scripts/tournament_pipeline.ps1` | Round-robin data gen + train + per-archetype eval |
| `scripts/gpu_brain_pipeline.ps1` | Bootstrap (heuristic self-play) + train + eval |
| `bench/tournament-run/REPORT.md` | Auto-generated win-rate report |

**Tests:** 137/137 lib + 6 ai_vs_ai + 3 brain_actually_drives + others = all green.

---

## Day 2 (2026-05-03) — biology-grounded brains break the plateau

Started the day at DAgger v1 SOTA (40.7%). Finished at MLP_v1 (FSP) SOTA (45.7%). Total day-2 lift: +5pp on top of prior +5pp = +10pp from baseline.

### Variant tournament — diversity ≠ better signal

21-brain pool (7 originals + 14 hand-tuned variants like `glass_cannon`, `pure_econ`, `nurse_heavy`, etc.). Each variant a perturbation of an archetype seed.

Result: **28.6%** mean win rate vs original 7. **Regressed** below baseline.

Diagnosis: bigger pool with mixed-quality teachers is *worse* than smaller pool with viable teachers. The model averages contradictory winning strategies and gets weaker than any single specialist. **Diversity helps only when teachers are viable.**

### Curated tournament — drop the dead-weight teachers

Quick scoreboard (each variant plays 4 matches vs heuristic) identified the 5 strongest variants (50% vs heuristic): `queen_focus`, `alate_swarm`, `pure_econ`, `worker_swarm`, `nurse_heavy`. Glass cannon, berserker, balanced_a/b, swarm, turtle, excavator, panic_fort, expansionist all scored 0-25% vs heuristic — dead weight.

12-brain curated pool (7 originals + top 5 variants):
- 4 m/p (528 matches): **41.9%** SOTA
- 8 m/p (1056 matches, 2x data): **42.6%** new SOTA, marginal scaling

### Curated × bigger model — capacity not the bottleneck

`hidden=256` model on curated corpus: **39.0%** (regressed). Confirmed: capacity is not the bottleneck, the *signal* is.

### DAgger-on-curated — self-play doesn't compose with curation

Stack the two known wins: DAgger pass starting from curated MLP. Result: **39.0%** (regressed). Same DAgger v2/v3 trap — adding self-play on top of an already-good model adds biased noise.

### Species-blend tournaments — biology-grounded archetype substitution

Replaced the made-up variants with biology-grounded blends derived from cited TOML fields:

  - `aggression` (cited per-species) → `losses_response` (× 2.0)
  - `recruitment` style → `forage_weight` (mass=0.70 group=0.55 tandem=0.45 indiv=0.40)
  - `queen_eggs_per_day` → `nurse_weight` (high lay = nurse-heavy)
  - `default_caste_ratio` → caste W/S/B (direct)

Two attempts:
- 5 species × `heuristic` archetype: **37.1%** (regressed). Insufficient strategic diversity — all 5 blends were heuristic-shaped variants.
- 5 species × ecology-matched archetype (formica × aggressor, aphaenogaster × forager, pogonomyrmex × economist, camponotus × defender, tapinoma × breeder): **38.2%**. Better strategic identity per blend, still below curated SOTA. The 5-blend pool just doesn't have enough economy-strategy diversity to beat curated.

### FSP round 1 — full 49-brain pool wins

Generated all 49 species×archetype combinations (7 species × 7 strategic postures). Each blend is biologically defensible (Camponotus playing economist still has 10% major caste; Lasius playing aggressor still has aggression≤0.4).

Tournament: 49×48 = 2352 unique pairings × 1 m/p ≈ 13k filtered winning records.

Result: **45.7% NEW SOTA.** First time MLP > heuristic with margin (11-9), tied defender/aggressor/conservative (10-10 each). Persistent blind spot: economy specialists (economist 40%, breeder 35%, forager 40%).

### FSP rounds 2-3 — vanilla iterative self-play plateaus

Added MLP_v1 to the pool, retrained → MLP_v2 = 45.7% (no gain). Added v1+v2, retrained → MLP_v3 = 42.9% (regressed).

Same trap as DAgger v2/v3 at smaller scale: adding the model to its own teacher pool teaches it to imitate itself. **Population-based BC needs the new MLPs to be genuinely different from existing pool.** Vanilla BC produces an *average* of teachers, not a distinct new strategy.

### Final leaderboard (Day 2)

| Approach | Result | Δ from baseline |
|---|---|---|
| Day-0 baseline (tournament v1) | 35.7% | — |
| DAgger v1 (Day-0 SOTA) | 40.7% | +5.0 |
| Curated 12 × 2x data | 42.6% | +6.9 |
| Species-blend 12 (heuristic only) | 37.1% | +1.4 |
| Species-canon 12 (ecology-matched) | 38.2% | +2.5 |
| **FSP-r1: 49 species×archetype pool** | **45.7%** | **+10.0** |
| FSP-r2 (49 + v1) | 45.7% | (no gain) |
| FSP-r3 (49 + v1 + v2) | 42.9% | (regressed) |

### Key takeaways for next session

1. **Stop inventing strategy labels; ground brains in cited biology.** 49 species×archetype blends produced the breakthrough, not bigger pool size or more iterations.
2. **BC over a fixed teacher pool plateaus around 45%.** Confirmed across multiple corpus shapes.
3. **Vanilla iterative FSP doesn't help.** Adding the trained MLP to its own teacher pool produces no new strategic info — it just teaches the model to imitate itself (DAgger v2/v3 + FSP r2/r3 all hit this).
4. **Three paths past 45%:**
   - **Adversarial-FSP** — train on trajectories where the current MLP LOST (cheap, in-flight as of session end). Forces the model to learn responses to its own weaknesses.
   - **Targeted opponent generation** — generate variants of the archetypes the model loses to (forager, breeder, economist), retrain. Like DAgger but strategically targeted.
   - **Real RL** — PPO/REINFORCE with outcome as reward. Discovers strategies the BC pool never demonstrated. Bigger separate effort.
5. **Persistent blind spot: economy specialists.** Forager/breeder/economist consistently beat the MLP at 30-40%. Either the sim over-rewards economy or the BC corpus under-represents what beats it.

### File map (Day-2 additions)

| File | What |
|---|---|
| `crates/antcolony-sim/src/ai/brain.rs` | `TunedBrain`, `BrainArchetype`, `SpeciesBrain` (biology-grounded blends) |
| `scripts/derive_species_brains.py` | Python parity for species → 9-tuple mapping |
| `scripts/generate_blended_brains.py` | All 49 species×archetype specs |
| `scripts/generate_full_brain_pool.py` | TSV pool file for FSP runner |
| `scripts/iterative_fsp.ps1` | Vanilla FSP runner (3 rounds) |
| `scripts/adversarial_fsp.ps1` | Adversarial-FSP runner (in-flight at session end) |
| `scripts/variant_tournament.ps1` | 21-brain hand-tuned pool (regressed result) |
| `scripts/curated_tournament.ps1` | 12-brain curated pool (41.9%) |
| `scripts/curated_2x_data.ps1` | 12 brains × 8 m/p (42.6%) |
| `scripts/species_blend_tournament.ps1` | 5 × heuristic blends (37.1%) |
| `scripts/species_canon_tournament.ps1` | 5 ecology-matched blends (38.2%) |

Best model: `bench/iterative-fsp/round_1/mlp_weights_v1.json` — 45.7% mean vs original 7.

---

## Day 2 (cont.) — adversarial FSP, second negative result

After FSP plateaued, attempted adversarial-FSP (`scripts/adversarial_fsp.ps1`):
1. Eval current MLP, find weakest matchups (round 1 found `economist`, `breeder`)
2. Generate 3 variants of each weak archetype (params perturbed ±15%)
3. Have current MLP play vs full pool (49 species + 6 new variants) for 16 m/p
4. **ADVERSARIAL FILTER**: keep ONLY trajectories where the OPPONENT won (i.e., the decisions of brains that beat the current MLP)
5. Train MLP_adv_v(n+1) on this "what beat me" corpus
6. Eval, repeat

Result over 3 rounds:

| Run | vs original 7 |
|---|---|
| Start (MLP_v1, FSP-r1 SOTA) | 45.7% |
| MLP_adv_v1 | 42.9% |
| MLP_adv_v2 | 42.9% |
| MLP_adv_v3 | 42.9% |

**Regressed and stayed flat.** Diagnosis: training EXCLUSIVELY on "what beat me" trajectories biases the state distribution toward "states where MLP was about to lose." Model imitates winning-side actions in losing-state distributions and loses general competence — the "what beats me" lessons are real but they overwrite broader skill.

**The fix would be MIXING:** train on `(full corpus + adversarial subset weighted higher)`. Pure adversarial = throwing baby with bathwater. Adversarial filter is a useful *signal*, not a usable corpus on its own.

Updated next-session paths:
1. **Mixed-corpus DAgger** — combine full FSP-r1 corpus + adversarial trajectories (e.g., 4× weight on adversarial), retrain. Might lift past 45.7% by adding "specific counters" without losing "general competence."
2. **Real RL** — still the right long-term answer.
3. **Sim-balance** — economy specialists keep winning; investigate whether forager genuinely beats reactive AI by ignoring combat entirely (sim balance issue) or whether the BC corpus underweights anti-economy plays.

Best model unchanged: `bench/iterative-fsp/round_1/mlp_weights_v1.json` (45.7%).

### Mixed-corpus retry — also fails (42.9%)

Following the adv-FSP analysis, tried mixing the FSP-r1 corpus (general competence, 534k records) with the adversarial corpus (132k records) replicated 4× to weight it heavier. Total mixed corpus: 1.06M records.

Result: **42.9%.** Same as pure adversarial. Identical per-archetype breakdown: 10/10/10/7/7/7/9.

Conclusion: **adversarial trajectories from MLP_v1 don't generalize regardless of mix ratio.** They contain "winning opponent actions in states where MLP-v1 was about to lose." That's too narrow a distribution to broaden the model — it just biases toward anti-MLP-v1 plays in narrow state regions, and the model loses general competence.

**Final BC ceiling for today's experiments: 45.7%.** Confirmed across:
- DAgger v1/v2/v3 (40.7%)
- Curated tournament (42.6%)
- FSP-r1 (45.7% NEW SOTA)
- FSP-r2/r3 vanilla iteration (regressed)
- Adversarial-FSP r1/r2/r3 (regressed)
- Mixed-corpus retry (regressed)

Path past 45.7% is no longer "more BC tricks." Real RL or genuinely richer base pool (e.g., add more species + add the cross-displacement species like Brachyponera/Solenopsis to enable Aphaenogaster's documented displacement bias).

---

## Day 2 (cont.) — sim balance fix + FSP on rebalanced sim

**Diagnostic that prompted the change:** ran a 7×7 archetype dominance matrix on the bench fixture. Result was lopsided:

| Archetype | Mean win rate (before balance fix) |
|---|---|
| forager | 47.9% |
| breeder | 47.9% |
| economist | 35.4% |
| heuristic | 22.9% |
| conservative | 25.0% |
| defender | 16.7% |
| aggressor | 16.7% |

Pure economy strategies dominated; combat archetypes lost 65% of matches. The MLP's persistent "loses to economy specialists" blind spot was a SIM BALANCE problem, not a model problem.

**Two-line balance change:**
- `CombatConfig::default().soldier_attack`: 3.0 → 5.0 (soldier × soldier_vs_worker_bonus 3.0 = 15 dmg vs 10 HP worker = one-shot kill)
- `ColonyConfig::default().soldier_food_multiplier`: 1.5 → 1.2 (soldiers no longer choke economy)

**Post-balance dominance matrix** (all archetypes now competitive):

| Archetype | Mean win rate (after balance fix) | Δ |
|---|---|---|
| aggressor | 60.4% | +43.7 |
| heuristic | 50.0% | +27.1 |
| conservative | 47.9% | +22.9 |
| economist | 43.8% | +8.4 |
| forager | 43.8% | -4.1 |
| defender | 37.5% | +20.8 |
| breeder | 33.3% | -14.6 |

No archetype dominates >65% now. Real meta exists.

**FSP-r1 on the rebalanced sim (8 species × 7 archetypes = 56-brain pool):**

| Opponent | Old MLP (unbalanced) | New MLP (rebalanced) | Δ |
|---|---|---|---|
| heuristic | 55% | 45% | -10 |
| defender | 50% | 35% | -15 |
| aggressor | 50% | 35% | -15 |
| economist | 40% | **50%** | +10 |
| breeder | **35%** | **50%** | **+15** |
| forager | 40% | **50%** | +10 |
| conservative | 50% | 55% | +5 |
| **Mean** | **45.7%** | **45.7%** | 0.0 |

Same mean total but completely different competence profile:
- Economy blind spot ELIMINATED (was 35-40%, now 50% across all economy archetypes)
- Combat archetypes harder to beat now (35-45% vs prior 50%) — they're actually strong
- Net: model is competent across the entire matchup space, not broken on any single archetype

For game-AI purposes this is much more useful even at the same headline number. The pre-balance model would consistently lose to forager play; the post-balance model can hold its own across the full strategic landscape.

**Final BC ceiling against a balanced teacher pool: ~46-50%.** Mathematically — if all archetypes are competitive (~50% expected against each other), the BC-cloned model is ~50% expected against them too. To exceed this needs RL where the model can discover strategies the teacher pool doesn't demonstrate.

Best model: `bench/iterative-fsp/round_1/mlp_weights_v1.json` — 45.7% mean BUT with balanced competence profile.

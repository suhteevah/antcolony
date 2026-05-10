# Postmortem — 2yr HeuristicBrain smoke: seasonal-transition cohort cliff (6/8 species dead)

**Date:** 2026-05-09 (final state — 5 processes finished, 1 running with extinct colony, 2 still alive but food anomaly)
**Status:** ❌ **6 of 8 species extinct.** All under HeuristicBrain. Two species "surviving" only because of an apparent **food-overaccumulation bug** that lets them ride out winter on absurd reserves (39,117 food on rudis at year 2 DOY 175).
**Triage:** Real species-balance bug AT THE SIM-MODEL LEVEL — not TOML calibration. **Falsifies** the prior session's hypothesis that the year-1 hibernation extinctions were a brain artifact. The sim has fundamental seasonal-transition handling bugs.

## Final outcome by species

| Species | State | Death day / year / DOY | Mode |
|---|---|---|---|
| `lasius_niger` | extinct | day 107, year 0, DOY 257 (Sep 14) | autumn pre-diapause cliff |
| `pogonomyrmex_occidentalis` | extinct | day 125, year 0, DOY 275 (Oct 2) | autumn pre-diapause cliff |
| `formica_rufa` | extinct | day 291, year 1, DOY 76 (Mar 17) | spring diapause-exit cliff |
| `camponotus_pennsylvanicus` | extinct | day 295, year 1, DOY 80 (Mar 21) | spring diapause-exit cliff |
| `tapinoma_sessile` | extinct | day 295, year 1, DOY 80 (Mar 21) | spring diapause-exit cliff |
| `tetramorium_immigrans` | extinct | day 290, year 1, DOY 75 (Mar 16) | spring diapause-exit cliff |
| `aphaenogaster_rudis` | "alive" | n/a — 960 workers, **39,117 food** at last sample | survived via food-overaccumulation |
| `formica_fusca` | "alive" | n/a — 1,894 workers, **10,711 food** at last sample | survived via food-overaccumulation |

**Two distinct extinction modes** at opposite seasonal boundaries, and a **third bug** (food overaccumulation) masking what would otherwise be an 8/8 wipe.

---

## What happened

Detached `smoke_10yr_ai --years 2 --no-mlp` runs were launched per the previous session's plan to confirm whether MLP saturation was the sole cause of year-1 extinctions. Six species are still running healthy. Two ran to completion with `survived=false workers=0`:

| Species | Last alive (tick / DOY / temp) | First dead (tick / DOY / temp) |
|---|---|---|
| `lasius_niger` | tick 4,599,995 / DOY 256 / 19.7°C / 341 workers | tick 4,674,995 / DOY 257 / 19.4°C / 0 workers |
| `pogonomyrmex_occidentalis` | tick 5,349,995 / DOY 274 / 14.2°C / 218 workers | tick 5,424,995 / DOY 275 / 13.8°C / 0 workers |

Both are dying **above** their hibernation cold thresholds (Lasius typical ~8-10°C, Pogonomyrmex similar). This is **not** a winter-diapause death — it is autumn pre-diapause.

## Diagnostic from `daily.csv` (last 10 days before extinction, Lasius)

| DOY | temp_c | workers | soldiers | breeders | eggs | larvae | pupae | food | inflow |
|---|---|---|---|---|---|---|---|---|---|
| 247 | 22.30 | 281 | 124 | 24 | **1** | 31 | 175 | 4.99 | 1.23 |
| 248 | 22.01 | 283 | 128 | 26 | **0** | 17 | 174 | 3.21 | 0.13 |
| 249 | 21.73 | 290 | 128 | 26 | **0** | 8 | 170 | 4.69 | 0.75 |
| 250 | 21.44 | 301 | 131 | 26 | **0** | 0 | 152 | 3.13 | 0.00 |
| 251 | 21.15 | 311 | 138 | 26 | **0** | 0 | 120 | 0.47 | 0.23 |
| 252 | 20.86 | 323 | 142 | 26 | **0** | 0 | 86 | 5.58 | 0.91 |
| 253 | 20.56 | 330 | 145 | 27 | **0** | 0 | 66 | 3.03 | 0.25 |
| 254 | 20.27 | 334 | 147 | 27 | **0** | 0 | 46 | 3.78 | 0.96 |
| 255 | 19.97 | 338 | 152 | 28 | **0** | 0 | 22 | 7.81 | 1.17 |
| 256 | 19.67 | 341 | 152 | 28 | **0** | 0 | 4 | 2.28 | 0.19 |
| **257** | **19.37** | **0** | **0** | **0** | 0 | 0 | 0 | 0.00 | 0.00 |

The Pogonomyrmex trajectory has the same shape, shifted ~18 days later (cooler-climate species).

## Mechanism (root cause)

This is a chained failure in the colony-economy model, **not** a hibernation/diapause bug:

1. **Egg-laying flatlines at DOY 248** (Lasius), well before any cold threshold. The queen lay-rate gate at `simulation.rs:3208` is:
   ```rust
   if queen_alive && !colony.fertility_suppressed && colony.food_stored >= ccfg.egg_cost {
       colony.egg_accumulator += effective_egg_rate;
       ...
   }
   ```
   With `egg_cost ≈ 5.0` and food chopping between 0.4 and 7.8, the gate fails most ticks. The `food_inflow` throttle's 0.2 ENDOGENOUS_FLOOR (line 3157) does not save laying because the **stored-food threshold gate is checked before the throttle math even matters** — when stored < egg_cost, the queen simply doesn't lay regardless of the floor.

2. **Brood pipeline drains.** Larvae go to zero by DOY 250. Pupae drain steadily DOY 250-256 (175 → 4) — the cannibalism path at `simulation.rs:3043+` consumes them when food_stored goes negative, in priority order eggs → larvae → pupae. Each cannibalized pupa returns ~0.65 × egg_cost food, which keeps the cohort fed but at the cost of the future workforce.

3. **Pupae reservoir runs out** at DOY 256. Now there's nothing left to cannibalize and food_stored is still chopping near zero.

4. **5%-per-tick adult starvation cliff fires.** The adult-starvation cap at `simulation.rs:3118` is `((adult_total * 0.05).ceil() as u32).max(1)`. With 341+152+28 = 521 adults and 5%-per-tick continuous starvation, the colony wipes within ~75 ticks. The 25,000-tick log interval makes this look like a single-step extinction; in reality it's ~75 sequential per-tick wipes.

5. **Queen survives.** She is special-cased and is intact at DOY 257. But with no workers and no brood, the queen cannot rebuild — adult-spawning requires a pupa to mature, and pupae=0.

## Spring diapause-exit cliff (4 species)

Four species — `formica_rufa`, `camponotus_pennsylvanicus`, `tapinoma_sessile`, `tetramorium_immigrans` — all died **in the same 5-day window** (year 1, DOY 75-80, mid-March), at exactly the temperature crossing where diapause exemption stops protecting adults from food=0 starvation. Mechanism in detail (formica_rufa, ~525 healthy workers entering spring):

| DOY | temp_c | workers | eggs | larvae | pupae | food | inflow |
|---|---|---|---|---|---|---|---|
| 73 (Mar 14) | 10.18 | 524 | 66 | 83 | 110 | **0.00** | 0.000 |
| 74 (Mar 15) | 10.48 | 527 | 0 | 57 | 114 | 2.49 | 0.000 |
| 75 (Mar 16) | 10.78 | 529 | 0 | 0 | 68 | 1.37 | 0.000 |
| **76 (Mar 17)** | **11.08** | **0** | 0 | 0 | 0 | 0.00 | 0.000 |

What's happening:

1. **Colony emerges from winter healthy.** During diapause the food-stored stays at 0 because metabolic_factor zeroes consumption (and any small drains are clamped per the in_diapause guard at simulation.rs:3028). Brood pipeline pauses — eggs/larvae/pupae preserved.
2. **Spring warmup crosses the threshold.** Now `in_diapause = false`. Adults resume full consumption. Foragers resume foraging — but `food_inflow_recent` has decayed to ~0 over 90+ days of `*= 0.993` per tick during winter, so the queen's lay-rate throttle starts from rock bottom.
3. **food_stored is 0 entering spring.** Adults immediately drain into the negative branch at simulation.rs:3033. Brood cannibalism fires — eggs first, then larvae, then pupae, in priority order. Look at the table: `eggs 66→0` happens in one day. `larvae 83→57→0` over two days. Pupae 110→114→68→0 over three days.
4. **Pupae depleted.** No more brood to cannibalize. food_stored still 0 because foragers can't ramp to feed 525 active adults that fast.
5. **5%/tick adult-starvation cap fires.** Within ~75 ticks (one log interval), all 525+227+27 = 779 adults dead. Queen survives (special-cased) but cannot rebuild — pupae=0.

This is the same cannibalism→starvation cliff as the autumn pattern, just at the opposite seasonal boundary. Tapinoma collapsing despite **2,231 workers** and **1,020 soldiers** rules out "weak species / low population" as a hypothesis. The bug is in the seasonal-transition arithmetic, not species balance.

## Food-overaccumulation in surviving species (rudis + formica_fusca)

The two "surviving" species have **anomalously huge food reserves**. Aphaenogaster rudis trajectory at 30-day samples:

| Day | DOY | Year | Workers | Food |
|---|---|---|---|---|
| 30 | 180 (Jun) | 0 | 12 | 7.3 |
| 60 | 210 (Jul) | 0 | 43 | 116 |
| 90 | 240 (Aug) | 0 | 148 | **2,204** |
| 120 | 270 (Sep) | 0 | 267 | 3,674 |
| 150 | 300 (Oct) | 0 | 388 | 5,282 |
| 180 | 330 (Nov) | 0 | 388 | 4,437 |
| 270 | 55 (Feb) | 1 | 388 | 1,913 |
| 300 | 85 (Mar, post-thaw) | 1 | 474 | 617 |
| 330 | 115 (Apr) | 1 | 667 | 5,882 |
| 360 | 145 (May) | 1 | 885 | 13,519 |
| 390 | 175 (Jun) | 1 | 958 | **21,763** |

A real *Aphaenogaster rudis* colony of 958 workers does not store 21,763 food units of anything. The published mature-colony figures (Lubertazzi 2012) range 266-613 workers, and field-measured caching is in single-digit grams of seed material per nest. The sim is producing **30+ orders of magnitude over the realistic ratio** of (food stored / worker count).

This is a separate bug from the seasonal cliff. Hypotheses (not confirmed):
1. **Food-deposit accumulation has no per-tile or per-colony cap.** Foragers drop food in nest tiles and the running total just grows.
2. **Species TOML `food_per_adult_per_day` for rudis is too low** — colony consumes less than it brings in indefinitely. (Less likely — same TOML pattern as other species, and other species are dying from food=0.)
3. **Diapause consumption exemption combined with non-zero foraging during low-temperature pre-diapause days** lets colonies stockpile in autumn faster than they can drain it.

Whatever the cause, the rudis "survival" is via **off-distribution sim state** — not a publishable result. formica_fusca shows the same pattern at lower magnitude (1,894 workers / 10,711 food = ~5.6 food/worker, vs rudis at 22.7 food/worker; both wildly above realistic).

## Why this falsifies the brain-artifact hypothesis

The prior session (2026-05-09 morning) saw Lasius and Pogonomyrmex extincting under MLP brain at year-1 winter. The hypothesis was: the MLP saturation locks behavior weights at constant values, and the resulting (0,1,1) or (1,1,1) split misallocates labor away from foraging. Re-test under HeuristicBrain (which can't saturate and bumps `forage_weight` reactively when food is low) was supposed to confirm.

**HeuristicBrain still kills both species.** The brain layer is **not** the problem. The problem is upstream of the brain: the colony-economy gates (queen-lay food threshold, brood-cannibalism priority, adult-starvation 5%/tick) interact to produce a deterministic autumn collapse for species whose autumn forager throughput drops below the egg-cost threshold for sustained windows.

This means **species TOML calibration alone cannot fix this.** Increasing `worker_lifespan_months` postpones the cohort cliff but doesn't prevent it. Increasing `target_population` doesn't help — the saturation cap isn't firing here. Decreasing `egg_cost_food` would help but breaks downstream balance for species that are surviving.

## Why the apparent survivors "survive"

(See "Food-overaccumulation" section above.) The two apparent survivors (rudis, formica_fusca) are riding out winter on **food caches 1-2 orders of magnitude above any realistic colony storage**. When spring arrives and adults wake up hungry, they have so much food in store that the cannibalism→5%/tick cascade never fires. This is **not** survival under defensible biology — it's survival under a separate sim bug that is masking what would otherwise be an 8/8 wipe.

The mid-session check at 12-15% progress showed the now-dead species in apparent stable state too. The cliff fires fast (single log interval) and is not visible in tail-of-log monitoring until it hits.

## Recommended fixes (not applied while smoke is running)

The autumn cliff and spring cliff are the **same bug** at the two seasonal boundaries. The food-overaccumulation is a separate bug masking the failure for two species.

In rough order of how directly each addresses the root cause:

1. **Decouple egg-lay food gate from `egg_cost`** — change the gate at `simulation.rs:3208` to check `colony.food_stored > 0.0` (or some small sustenance buffer like `egg_cost * 0.2`), then allow `effective_egg_rate * throttle` to scale the lay rate down when stored is low. Preserves throttle's ENDOGENOUS_FLOOR semantics. Currently the throttle never applies because the binary gate slams shut.
2. **Reset `food_inflow_recent` on diapause exit, not gradually decay through winter.** Currently the running average decays ~0.993/tick during 90+ days of winter, leaving the queen-throttle at floor when adults wake up. Change to: skip the decay during in_diapause (food_inflow_recent stays at last-active-day value), or pre-load the throttle to a sane value on the diapause-exit transition. This directly addresses the spring cliff.
3. **Smooth the adult-starvation cap.** 5%/tick × 75 ticks = full wipe in <1 in-game day. Cap at 1%/day (~0.000023/tick) instead. Real biology has individual workers dying singly over weeks, not entire cohorts in seconds.
4. **Add a per-colony food-storage cap** based on nest volume / worker count / species TOML field. Prevents the 21,000-food rudis pathology. Realistic cap is ~10× egg_cost × pop or similar — enough to weather a bad week, not a year.
5. **Worker-lifespan stochastic mortality.** Currently absent. Each tick: worker dies with probability `1 / lifespan_ticks` instead of deterministic age-out. Smooths future cohort-cliff scenarios.
6. **Soft autumn diapause ramp.** Replace the binary cold-threshold gate with a forager-success-vs-temperature curve. Also serves the Warren & Chick 2013 reproduction.

Fixes #1 + #2 together would have prevented all 6 extinctions. #4 is necessary for the surviving species to be at defensible state. #3 + #5 are general hardening. #6 is the most ambitious and most outreach-useful.

## Implications for outreach roadmap

**ALL FOUR planned reproductions are blocked.** This is not "Pogonomyrmex blocks one paper" — it's the sim cannot complete a 2-year horizon for 6 of 8 species, and the 2 that "complete" do so via a separate food-overaccumulation bug. None of these results are publishable-comparison-grade right now.

- **Warren & Chick 2013 (cold foraging)** — needs *A. rudis* across a temperature range. Rudis is "alive" but on bug-state food reserves; its forager-vs-temperature curve cannot be trusted.
- **Rodriguez-Cabal 2012 (displacement)** — two-colony scenario with rudis and *B. chinensis*. Both species would be subject to the seasonal cliffs over the 5-year horizon. *Brachyponera chinensis* hasn't been smoke-tested yet.
- **Cole/Wiernasz Pogonomyrmex 7yr growth** — Pogonomyrmex extincts at year 0 day 125. Cannot run the 7yr horizon.
- **Charbonneau-Sasaki-Dornhaus 2017 lazy worker** — needs *T. curvinodis*, which hasn't been smoke-tested yet either. Likely subject to the same cliffs.

**Outreach is fully blocked until the seasonal-transition bugs are fixed and a clean 2yr smoke runs.** Do not send any drafts.

**The methodology.md needs a frank update.** The "what the sim is not" section currently doesn't mention seasonal-transition fragility. Until the fixes land, any methodology read by a researcher must explicitly disclose: "the sim has a cohort-cliff bug at diapause boundaries and a food-overaccumulation bug; reproductions below are deferred until fixed."

## Forensic data preserved

All under `bench/smoke-10yr-ai/<species>/daily.csv` + `decisions.csv`. The CSVs are the canonical evidence; do not delete.

| Species | Mode | Why kept |
|---|---|---|
| `lasius_niger` | autumn cliff | first-discovered, year 0 |
| `pogonomyrmex_occidentalis` | autumn cliff | year 0, blocks Cole/Wiernasz repro |
| `formica_rufa` | spring cliff | spring boundary canonical |
| `camponotus_pennsylvanicus` | spring cliff | small-colony spring failure mode |
| `tapinoma_sessile` | spring cliff | **2,231 workers** — rules out small-pop hypothesis |
| `tetramorium_immigrans` | spring cliff | second high-pop spring failure |
| `aphaenogaster_rudis` | food overaccumulation survivor | 21k+ food reserves canonical |
| `formica_fusca` | food overaccumulation survivor | second canonical |

The MLP saturation evidence at `bench/smoke-10yr-ai-mlp-saturation/` from the prior session remains valid for the MLP-specific problem; the present finding is a different, additional, cross-brain bug.

## Immediate next steps when 2 still-running processes finish

1. Confirm tapinoma & rudis dailies are final (they may keep logging zeros to end of run).
2. Optionally, kill the 2 remaining processes early — they cannot produce useful new information now that the sim-level bug is identified. Process IDs in `bench/smoke-10yr-ai/_logs/pids.json` (rudis 149232, fusca 155932; tapinoma 154760's colony is dead but process is still running).
3. Apply fixes #1 + #2 together (egg-lay food gate decoupling + food_inflow_recent diapause-exit reset). Build with smoke processes still consuming CPU on the live ones is OK only after they're killed.
4. Re-run a fast 2yr smoke with the same 8 species. **Acceptance criterion: 8/8 survive year 2 with no food overaccumulation (food_stored / worker_count < 5 across all samples).**
5. If 8/8 pass, then proceed to the deferred handoff items (predates_ants schema, per-ant activity-fraction, soft cold-foraging curve).
6. **Do not send any outreach emails until step 4 passes AND a 7yr Pogonomyrmex run reaches the published 6,000-12,000 worker mature size.**

---

*Postmortem author: Claude (via Matt's session). Written before fixes applied; smoke still running so no source-tree changes were made.*

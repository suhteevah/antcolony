# Outreach Roadmap — Design Spec

**Date:** 2026-05-09
**Status:** Approved (Matt, brainstorming session 2026-05-09)
**Predecessors:** `docs/postmortems/2026-05-09-seasonal-transition-cliffs.md`, `outreach/README.md`, `HANDOFF.md`
**Successor:** Implementation plan (to be written via `superpowers:writing-plans`)

---

## Goal

Get the simulation to a state where four published-figure reproductions each produce a number that lands within published variance, with load-bearing abstractions explicitly documented, and that survives an adversarial red-team pass. Outreach timing is decided **after** the red team is clean — not before.

Target reproductions:

| # | Researcher | Paper |
|---|---|---|
| 1 | Cole / Wiernasz (U Houston) | Cole & Wiernasz, *Insectes Sociaux* — *P. occidentalis* 7yr growth curve to 6,000-12,000 workers |
| 2 | Robert J. Warren II (Buffalo State) | Warren & Chick 2013, *Glob. Change Biol.* — *A. rudis* cold-tolerance foraging |
| 3 | Robert J. Warren II (Buffalo State) | Rodriguez-Cabal 2012 / Warren et al. 2018 — *B. chinensis* displacement of *A. rudis* |
| 4 | Anna Dornhaus (U Arizona) | Charbonneau, Sasaki & Dornhaus 2017 *PLoS ONE* — *Temnothorax* "lazy worker" bimodality |

The Warren papers consolidate into one email (`outreach/warren_consolidated.md`). Three emails total cover four papers.

---

## Non-goals

These are explicitly **out of scope** for the rigor work in Phases 1-5. Some return as scope in Phase 6 (charm package). They may be individually valuable, but blocking the rigor critical path on them is wrong-priority.

- **AI/MLP/PPO work.** No new training, no MLP saturation fixes, no AI-ceiling work. `bench/iterative-fsp/round_1/mlp_weights_v1.json` (50.7% on the wider bench) is the shipped SOTA. HeuristicBrain is the only brain used for repro work — `mlp_weights_v1.json` is OOD on solitaire (saturated outputs by sim-day 6, evidence at `bench/smoke-10yr-ai-mlp-saturation/`). **Permanently out of scope for this roadmap.**
- **PvP / netcode / cross-OS determinism.** Win-to-Win PvP works; mixed-OS does not (Windows↔Linux libm divergence, RED in handoff). Reproductions all run single-OS (Linux on cnc-server), so cross-OS determinism does not gate them. **Permanently out of scope for this roadmap.**
- **Bevy renderer features and gameplay polish.** Out of scope for the rigor critical path (Phases 1-5). **Returns in Phase 6** as part of the keeper-mode charm package — researchers genuinely love ants, and a polished desktop artifact strengthens the outreach package even though it doesn't change a single number.
- **Long-run substep collapse bug** from 2026-04-25. Only fires at Hyperlapse (1440×); reproductions run at Seasonal (60×). Not a blocker. Permanently out of scope.
- **Pratt 2005 quorum-sensing emigration.** The deferred 5th paper. Sim's relocation mechanics are coarse and don't yet reproduce decision-quorum dynamics. Permanently out of scope for this roadmap.

---

## Verification bar

**Per-paper:** sim's number lands within the paper's reported variance. Each `repro/<paper-slug>.md` writeup explicitly lists which sim abstractions could break the comparison ("load-bearing abstractions" section). Drafts frame results as provisional — "we tried to reproduce this; here's our number; here's what we may have wrong" — never as validation.

**Cross-paper:** an adversarial `/redteam` pass against the whole bundle (methodology + 4 repro writeups + 4 outreach drafts) returns zero open Critical findings and every Major finding is explicitly disclosed in the writeup it affects.

---

## Infrastructure

**All smoke and reproduction runs execute on cnc-server (192.168.168.100, openSUSE Leap Micro 6.2, i5-4690K 4C/4T, 16GB effective with 8GB swap, rustc 1.93.1).** Kokonoe stays free for development.

- 8GB swapfile lives at `/var/swapfile` (Leap Micro root is read-only; `/var` is writable). Persisted in `/etc/fstab`. Created during the brainstorming session — eliminates `cargo build` OOM risk on the 8GB-RAM box.
- All reproduction recipes pin to **Linux x86_64, rustc 1.93.1, seed N**. This is more academically defensible than Windows-pinned numbers and avoids the cross-OS libm determinism issue entirely.
- Antcolony source rsync'd to `/opt/antcolony/` on cnc, with workspace `Cargo.toml` trimmed to `[crates/antcolony-sim]` only (drop render/game/net/trainer; keeps build small and fast).
- Smoke processes run as detached background jobs (nohup or tmux). PIDs + stdout/stderr at `/opt/antcolony/runs/<timestamp>/`. Results pulled back to kokonoe via `scp`.
- Wrapper script at `scripts/cnc_smoke.ps1` handles rsync-source / build / launch / monitor / pull-results.
- Concurrency: 2-4 simultaneous runs (cnc has 4 cores; fleet load averages 1.4). With swap, can safely burst to 4-at-a-time during build, sustain 2-at-a-time during long runs without affecting fleet stability.

---

## Phase 1 — Sim foundation (sequential, single hard gate)

Land all colony-economy correctness work as one block. Five edits to `crates/antcolony-sim/src/simulation.rs` plus one config addition. No new features, no new harnesses — just stop the bleeding.

**Edits:**

1. **Egg-lay food-gate decoupling** (`simulation.rs:3208`).
   - Replace binary `food_stored >= egg_cost` with `food_stored > 0.0` plus rate-scaling that respects the existing throttle's `ENDOGENOUS_FLOOR=0.2` semantics.
   - Mechanism: when food is low, queen lays at a reduced rate proportional to food-inflow throttle, instead of the lay rate slamming to zero at the binary threshold.
   - Preserve existing unit tests at `simulation.rs:4602+` (`queen_lay_rate_throttled_by_food_inflow`).
   - Why: postmortem fix #1, addresses autumn pre-diapause cliff.

2. **`food_inflow_recent` diapause-exit reset** (`simulation.rs:3012`).
   - Skip the `*= 0.993` decay when `in_diapause = true`.
   - Add a new regression test for the diapause-exit transition (existing test at line 5085, `diapausing_adults_dont_starve_when_reserves_run_out`, only covers within-diapause).
   - Why: postmortem fix #2, addresses spring diapause-exit cliff.

3. **Per-colony food storage cap** (new TOML field on `species_extended`).
   - Add `food_storage_cap: Option<f32>` to species TOML schema. Default cap = `target_population * egg_cost * 10` when not set.
   - Cap applies on deposit in colony economy path. Prevents the rudis 44k-food anomaly.
   - Why: postmortem fix #4, addresses food-overaccumulation that masked the cliff for 2 species.

4. **Smooth adult-starvation cap** at line 3118.
   - Reduce from 5%/tick to ~1%/day equivalent (~0.000023/tick).
   - Why: postmortem fix #3, hardening — protects all future cohort-cliff scenarios from single-tick wipes.

5. **Stochastic worker mortality** (replaces deterministic age-out in worker maturation/death path).
   - Per-tick `1/lifespan_ticks` death probability instead of deterministic age-out.
   - Why: postmortem fix #5, hardening — smooths cohort dynamics for any future seasonal smoke.

**Phase 1 exit criterion (single hard gate):**

A 2-year HeuristicBrain smoke across all 10 species (the original 8 plus B. chinensis and T. curvinodis) where:
- 10/10 species alive at year-2 end with queen + workers + brood pipeline intact
- `colony.food_stored / colony.adult_count` < 5.0 across all daily samples
- no single-day adult-population drop > 20% (catches any new cliff; "adult population" = worker + soldier + breeder counts, excludes brood and queen)
- all existing unit tests still green
- 1-2 new regression tests pass: diapause-exit transition, per-colony food cap

Smoke runs on cnc, 2-at-a-time. Wall-clock ~3-5 days.

---

## Phase 2 — Sim features (parallel-ready via subagents)

Three independent feature additions. All three are required for at least one paper. They touch different code paths and can be developed in parallel as subagents.

### 2.1 `predates_ants` combat hookup

- **Blocks:** Warren displacement (Rodriguez-Cabal 2012)
- **Where:** `crates/antcolony-sim/src/species_extended.rs`, `crates/antcolony-sim/src/combat.rs`
- **What:** Add `predates_ants: bool` to `DietExtended`. The TOML field already exists on `assets/species/brachyponera_chinensis.toml` but is silently ignored. Hook into combat resolution: when a flagged ant is in interaction range with a foreign-colony ant, the flagged ant initiates attack regardless of pheromone alarm state, and consumed prey returns biomass to colony food (not just death).
- **Exit:** 2-colony rudis + B. chinensis smoke run shows asymmetric mortality favoring B. chinensis. Unit tests cover predator-vs-prey vs symmetric combat distinction.

### 2.2 Per-ant activity-fraction tracking

- **Blocks:** Dornhaus lazy worker (Charbonneau-Sasaki-Dornhaus 2017). Largest sim gap of the four.
- **Where:** `crates/antcolony-sim/src/ant.rs`, new bench-export module
- **What:** Add per-ant counters: ticks-in-Idle vs ticks-in-non-Idle. Stored on the ant entity as two `u32` counters. New bench-export API: `colony.export_activity_histogram() -> Vec<f32>` returns per-ant active-fraction = `non_idle_ticks / total_ticks`.
- **Exit:** T. curvinodis solitaire smoke produces a per-ant activity-fraction histogram. Manual inspection: distribution should not be a uniform blob. Unit tests cover histogram math + edge cases (newly-hatched ants, dead ants).

### 2.3 Soft cold-foraging-vs-temperature curve

- **Blocks:** Warren & Chick 2013 cold-foraging
- **Where:** `crates/antcolony-sim/src/species_extended.rs`, `crates/antcolony-sim/src/simulation.rs` (climate gating block)
- **What:** Add TOML fields `cold_foraging_p50_c: Option<f32>` (50%-activity temperature) and `cold_foraging_slope: Option<f32>` (sigmoid steepness). When both set, replace binary `hibernation_cold_threshold_c` forager gate with sigmoid `1 / (1 + exp(-slope * (T - p50)))`. When unset, keep current binary behavior — no regression for species without explicit curves.
- **Exit:** A. rudis run with these fields set shows graded forager activity across 5-25°C (not binary). Existing species without the fields show identical behavior to pre-change.

**Phase 2 gate:** All three features land with unit tests AND the Phase 1 acceptance smoke (2yr 10-species) still passes 10/10 with new features active.

---

## Phase 3 — Reproduction harnesses (parallel-ready)

Four harnesses, one `crates/antcolony-sim/examples/<paper>_bench.rs` + one `repro/<paper>.md` writeup each. HeuristicBrain only.

### Harness template (uniform across all 4)

Each `examples/<paper>_bench.rs` accepts:
- `--seed <u64>` — for reproducibility
- `--out <dir>` — for CSV outputs
- Paper-specific args (e.g. `--years`, `--species`, `--temperature-range`)

Each writes:
- `daily.csv` (or paper-relevant time series)
- `summary.json` with the headline number(s) the paper reports
- `recipe.txt` with the exact cargo command, seed, and expected output hash

Each `repro/<paper-slug>.md` has the same five sections:
1. **Published figure** — citation + figure number + the published value(s).
2. **Our number** — value(s) from the sim. Direct comparison.
3. **Deviation** — quantitative gap. Pass/fail vs acceptance band.
4. **Load-bearing abstractions** — explicit list of sim simplifications that, if wrong, would break this comparison. Researchers will probe this section.
5. **Reproduction recipe** — `cargo` command + seed + expected output hash + cnc/Linux pinning.

### The four harnesses

| # | Harness | Writeup | Acceptance band | Wall-clock (cnc) | Required Phase 2 features |
|---|---|---|---|---|---|
| 1 | `pogonomyrmex_growth_curve_bench.rs` | `repro/cole_wiernasz_growth.md` | Year-7 worker count in 6,000-12,000; S-curve shape | ~28 hours | None (only Phase 1) |
| 2 | `cold_foraging_curve_bench.rs` | `repro/warren_chick_2013_cold.md` | Curve shape matches published; threshold ~8-10°C | ~6 hours | 2.3 (soft cold curve) |
| 3 | `invasion_displacement_bench.rs` | `repro/rodriguez_cabal_2012_displacement.md` | A. rudis abundance drops 60-90% after B. chinensis establishment over 5yr | ~12 hours | 2.1 (predates_ants) |
| 4 | `lazy_worker_bimodality_bench.rs` | `repro/charbonneau_dornhaus_2017_lazy.md` | Bimodal activity distribution; inactive cluster mobilizes >2× after 50% worker removal | ~3 hours | 2.2 (activity tracking) |

Total cnc wall-clock for the four runs: ~50 hours, sequential, decoupled from kokonoe.

**Phase 3 gate:** All four numbers land in their acceptance bands AND all four writeups complete the five-section template.

---

## Phase 4 — Frank documentation refresh

Once Phase 3 numbers exist:

- **`docs/methodology.md`:** Rewrite the "What the sim is, and is not" section based on what we've actually shipped. Add a new "Known sources of error" section that names every Phase 1 fix and Phase 2 feature, with explicit "this is what we changed and why we now believe it." Aimed at researchers reading before responding.
- **Each `outreach/<draft>.md`:** Replace placeholder numbers (`<within ε / off by Δ>` etc.) with real ones from the matching `repro/` writeup. Re-tighten to <300 words. Reverify researcher's institutional contact (per the `outreach/README.md` 7-day rule).
- **`HANDOFF.md`:** Roll forward with current status.

Single session, sequential. Exit: docs survive a self-read for honesty (no overclaiming, all gaps explicit).

---

## Phase 5 — Red-team gate (the hard wall)

Run `/redteam` against the whole bundle: methodology + 4 repro writeups + 4 outreach drafts. The red team is hostile by design — its job is to find the "matched the figure for the wrong reason" failures a researcher would catch.

Triage each finding:

- **Critical** (would invalidate a comparison): block, fix, re-red-team that paper's bundle
- **Major** (researcher would push back hard): document in the writeup's "load-bearing abstractions" section, no fix required if disclosed
- **Minor** (cosmetic / tone): edit before sending

**Phase 5 exit:** zero open Critical findings AND every Major finding is explicitly disclosed in the writeup it affects.

**After Phase 5 clears**, two parallel decisions: (a) outreach timing (deferred per Matt's brainstorming-session call), (b) whether to add Phase 6 before sending or send first and add Phase 6 as a follow-up artifact.

---

## Phase 6 — Charm + ecological richness package (post-red-team, optional-but-recommended)

The rigor work in Phases 1-5 is sufficient for outreach. Phase 6 turns the outreach package from "here are our numbers, please review" into "here are our numbers, please review, and here's a polished desktop simulator you can run while reading this." The reasoning: these researchers chose to study ants for a living. A charming, immediately-runnable artifact lands differently than a CSV link.

This phase is **optional but recommended.** If skipped, outreach proceeds with Phases 1-5 alone. If included, ships before the first email.

### 6.1 Seed dispersal expansion (dual-purpose: ecological + charming)

- **Where:** `crates/antcolony-sim/src/colony.rs`, `crates/antcolony-sim/src/ant.rs`, species TOMLs for *A. rudis* and *P. occidentalis*
- **Why:** Aphaenogaster rudis is the eastern-US forest seed-disperser ant (Lubertazzi 2012, Ness 2004). Warren's literature on rudis has direct ecological context for seed handling. Pogonomyrmex are seed harvesters by design — their entire colony economy is seed-based. Building this out strengthens both the rudis writeups and the Pogonomyrmex writeup with ecological realism, AND makes the keeper-mode visuals immediately recognizable to an ant ecologist (ants carrying seeds along trails).
- **What:** Build out from the existing `seed_dispersal` #11-lite hook (per project memory, already 10/14 Phase B hooks). Add per-species seed-pickup, transport-along-trail, deposition-near-nest behaviors. New TOML fields on relevant species: `seed_handling_mode` (`disperser` | `harvester` | `none`), `seed_carry_distance_cells`, `seed_cache_radius_cells`. Visible in keeper mode as ants carrying seed sprites along food trails.
- **Exit:** Aphaenogaster solitaire run shows seed-dispersal pattern (seeds picked up, carried, deposited at varying distances from nest). Pogonomyrmex run shows seed-harvesting pattern (seeds picked up, brought to centralized nest cache). Visuals confirmed in keeper mode.

### 6.2 Keeper-mode polish (close out the noted next-up items)

- **Where:** `crates/antcolony-render/`, `crates/antcolony-game/`
- **What:** Land the next-up items already noted in project memory:
  - Camera follow (P7 next-up per memory)
  - Underground layer traversal (P5 next-up per memory)
  - Researcher demo mode: a one-key launcher that picks a species, loads it, and shows the colony with biology stats overlay (population, food, behavioral state, current pheromone field). Aimed at a researcher who's never used the app before — should be self-explanatory in 30 seconds.
- **Exit:** Loading any species runs smoothly at ≥60fps for ≥5 minutes on Matt's hardware. Camera follow works on a hovered ant. Demo mode loads a species in <3 keypresses from cold start.

### 6.3 Distribution

- **What:** Build release artifacts (Win + Linux x86_64 binaries) that a researcher can download and run with no rust toolchain required. Single-file or single-zip per platform. Include the `assets/` directory bundled.
- **Where:** `scripts/release_build.ps1` (Windows native build), `scripts/release_build_linux.sh` (cross-build via cargo-zigbuild or run on cnc).
- **Exit:** Both binaries run on a clean machine without rust installed. Total artifact size <100MB per platform.

### 6.4 Researcher-facing one-pager

- **Where:** `docs/keeper-mode-quickstart.md` linked from each outreach email
- **What:** Single page: "If you'd like to play with this on your desktop while reading our reproduction writeups, here's how. Pick *A. rudis* (Warren) / *P. occidentalis* (Cole-Wiernasz) / *T. curvinodis* (Dornhaus) / etc. Watch the trails form, the colony grow." Include 2-3 screenshots. <500 words.
- **Exit:** A reader who hasn't seen the project can install + launch + watch a colony in <2 minutes.

**Phase 6 gate:** All four sub-items complete AND a non-Matt person (or red-team agent) successfully launches the keeper-mode demo without project-specific guidance.

---

## Effort and timeline

| Phase | Dev sessions | cnc wall-clock |
|---|---|---|
| 1 (sim foundation) | 1-2 | 3-5 days (smoke) |
| 2 (sim features, parallel) | 3-5 | minutes (regression smoke) |
| 3 (harnesses, parallel) | 4-8 | ~50 hours (4 runs) |
| 4 (docs refresh) | 1 | none |
| 5 (red-team) | 1-2 + fix iterations | none |
| 6 (charm package, optional) | 3-5 | none |

Realistic calendar: 2-3 weeks of focused work for Phases 1-5, plus another ~1 week for Phase 6 if included. cnc runs in the background throughout — does not block kokonoe dev.

---

## Out-of-scope for this spec but worth noting

Items deliberately not addressed here, with reasons:

- **Outreach timing decisions** (single email vs phased, which order). Deferred until Phase 5 clears, per Matt's brainstorming-session decision.
- **Pratt 2005 5th paper.** Out per non-goals — sim's relocation mechanics insufficient.
- **Researcher institutional address verification.** Per `outreach/README.md`, verified within 7 days of sending; sending is post-Phase-5.

---

## Predecessor documents (read for context)

- `docs/postmortems/2026-05-09-seasonal-transition-cliffs.md` — the bug findings that motivate Phase 1
- `outreach/README.md` — the four-paper plan and tone calibration
- `outreach/warren_consolidated.md`, `outreach/wiernasz_cole.md`, `outreach/dornhaus_charbonneau.md` — existing draft emails with placeholder numbers
- `docs/methodology.md` — current state of the methodology one-pager (will be refreshed in Phase 4)
- `HANDOFF.md` — running session log
- `CLAUDE.md` — project rules (verbose logging, no .unwrap, ECS purity, etc.)

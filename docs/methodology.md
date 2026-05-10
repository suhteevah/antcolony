# Methodology — antcolony Simulation

> One-pager: what the simulation models, what is abstracted, where the figures come from. Companion to per-species docs in `docs/species/` and the project [`CLAUDE.md`](../CLAUDE.md). Aimed at researchers evaluating whether the sim's outputs are publishable-comparison-grade for their paper, and at engineers reviewing whether sim choices are defensible.

---

## 1. Engine and architecture

- **Language / framework.** Rust (edition 2024, MSRV 1.85), Bevy 0.15+ ECS for the rendered binary. Simulation core is rendering-free (`crates/antcolony-sim`), so headless reproductions and trainer integration do not pay rendering cost. Bevy glue lives in `crates/antcolony-game`; renderer in `crates/antcolony-render`.
- **Tick rate.** 30 Hz `FixedUpdate`. All sim systems run on this fixed timestep. Rendering decouples at display refresh.
- **Determinism.** Verified byte-identical across same-process repeats, separate processes, and different `RAYON_NUM_THREADS` settings on the same OS (`crates/antcolony-sim/examples/det_check.rs`). Cross-OS determinism (Windows ↔ Linux) is currently **broken** at the platform-libm boundary (transcendental f32 ops differ by ~1-2 ulps); this affects mixed-OS PvP only and does not affect the headless reproduction harnesses, which we run single-OS.
- **Performance budget.** Designed for 10,000 ants at 30 Hz on commodity hardware (i9-11900K reference). Spatial hashing for neighbor queries; dense pheromone grids; SoA component layout where it matters.

## 2. Ant agent model

Each ant is an entity with:

- A **finite-state machine** over `{Idle, Exploring, FollowingTrail, PickingUpFood, ReturningHome, StoringFood, Fighting, Fleeing, Nursing, Digging}` (`crates/antcolony-sim/src/ant.rs`). State transitions are local — an ant sees only its own pheromone-cell readings and a small spatial-hash neighborhood. **No ant has global colony or world knowledge.**
- A **caste** (worker / soldier / breeder), assigned at hatch from a colony-level ratio set by the species TOML and modulated by the active brain's `caste` decision.
- A **species** with biology parameters loaded from `assets/species/<id>.toml` (lifespan, attack, health, recruitment style, diapause requirement, diet preferences, mound type, etc.). Per-species docs in `docs/species/<id>.md`.

### Sensing

Each tick, an ant samples the pheromone grid in a **forward cone** (configurable `sense_radius`, `sense_angle` — defaults 5 cells / ±60°). Direction selection for foragers uses an **ACO-style probability rule**:

```
P(dir_j) ∝ pheromone(j)^α  ·  desirability(j)^β
```

with α = 1.0 (pheromone weight), β = 2.0 (heuristic weight), 5 candidate directions sampled, and a **15% random-exploration fraction** per decision to break trail lock-in. This is a direct port of standard Ant Colony Optimization (Dorigo 1992; Dorigo & Stützle 2004), with the heuristic term repurposed as nest-distance-inverse for returning ants and forward-bias for outbound ants.

### Pheromone grid

Four channels: `food_trail`, `home_trail`, `alarm`, `colony_scent`. Per-cell behavior:

- **Deposit:** ants add a per-state strength on each tick they're in a depositing state.
- **Evaporation:** every tick, `cell *= 1 - EVAP_RATE` (default 0.02).
- **Diffusion:** every 4th tick, 5-point Laplacian stencil, double-buffered.
- **Threshold:** values below 0.001 are clamped to 0 to avoid numerical drift.

This is the load-bearing piece for emergent trail formation — colony-level behavior is the closed-loop interaction of many independent local agents and the slow-decay shared pheromone field.

## 3. Colony model

Colony state (`crates/antcolony-sim/src/colony.rs`) holds **food stored, queen health, eggs/larvae/pupae counts, caste ratio targets, behavior weights (forage/dig/nurse), per-caste population, and nest-entrance positions**. The colony economy ticks once per FixedUpdate:

1. **Adult food consumption** — each adult worker/soldier consumes a per-tick fraction (TOML `food_per_adult_per_day` divided by ticks/day).
2. **Egg laying** — if food > `egg_cost_food`, the queen lays eggs at `queen_eggs_per_day` rate (or scaled if multiply-fed).
3. **Brood maturation** — eggs → larvae → pupae → adults on per-stage tick counters from the species TOML.
4. **Adult lifespan attrition** — workers/soldiers age and die at `worker_lifespan_months` / soldier-equivalent.

Caste assignment at adult emergence is sampled from the colony's current `caste_ratio` (worker/soldier/breeder), which can be overridden each decision tick by an external **brain** (heuristic, MLP, or scripted) via the `apply_ai_decision` API.

## 4. Climate and seasonality

- **Per-day temperature curve.** Loaded from species-region climate data; controls foraging intensity and diapause gating.
- **Diapause / hibernation gate.** Currently a **binary cutoff**: when daily mean temperature drops below `hibernation_cold_threshold_c`, foragers stay in nest and the queen stops laying. Re-warming above the threshold reactivates the colony. Per-species `hibernation_required` flag and `min_diapause_days` (Palearctic species) enforce that monogyne queens actually overwinter.
- **Soft cold-foraging-vs-temperature curve.** Not yet implemented. The Warren & Chick 2013 reproduction (planned) requires this to be added; at the time of writing, the sim only has the binary cutoff. See [`docs/biology-roadmap.md`](biology-roadmap.md) for the schema-extension plan.

## 5. Combat

Symmetric per-tick combat resolution (`crates/antcolony-sim/src/combat.rs`):

- Two ants from different `colony_id` within interaction range engage.
- Damage per tick is the attacker's `worker_attack` or `soldier_attack` modulated by per-species multipliers.
- Health drops to zero → ant dies, body remains as a one-off food source for scavengers.
- **Predation (one ant species hunting another as prey)** is **not yet implemented**. The species TOML now carries a `predates_ants: bool` flag for *Brachyponera chinensis*, but the schema and combat hookup are pending. This is a blocking gap for the *A. rudis* displacement reproduction (Rodriguez-Cabal 2012); see HANDOFF.md for tracking.

## 6. Worker behavioral heterogeneity

Per-ant **activity-fraction tracking** (the fraction of an ant's lifetime spent in non-`Idle` states) is **not yet implemented**. This is a blocking gap for the Charbonneau-Sasaki-Dornhaus 2017 reproduction on *Temnothorax* "lazy worker" bimodality. The plan is to add a per-ant counter of ticks-spent-in-each-state and expose it via a new bench-export API; see HANDOFF.md.

In the present sim, all workers of the same caste behave with identical decision rules — there is no worker-level personality, no Hamiltonian heterogeneity, no fixed-action-pattern individuality. This is appropriate for emergent-trail studies but inadequate for individual-variation studies of the kind Dornhaus's group publishes.

## 7. AI / brain layer

Three brains can be plugged in via the `Brain` trait:

1. **HeuristicBrain.** Hand-tuned reactive rules — bumps `forage_weight` when food < `egg_cost * 4`, increases `nurse_weight` when brood is large relative to adults, etc. Cannot saturate. **The recommended brain for solitaire bench work and reproduction harnesses.**
2. **MlpBrain.** A small feed-forward MLP loaded from `bench/iterative-fsp/round_*/mlp_weights_*.json`. Inputs are the bench observation vector (food, populations, enemy distance, recent combat losses, etc.); outputs are the AiDecision weights. The current SOTA brain (`mlp_weights_v1.json`) was trained PvP-only and **saturates on solitaire** (constant outputs by sim-day 6). Evidence preserved at `bench/smoke-10yr-ai-mlp-saturation/`. **Do not use for solitaire bench work.**
3. **ScriptedBrain.** Test-only — emits a fixed sequence of decisions for unit-test reproducibility.

Brain decisions are applied every `DECISION_CADENCE` (default 5) sim ticks. At all other ticks, ants run on FSM + pheromone alone — the brain layer is a **slow nudge** to colony-level allocation, not a moment-by-moment ant-control layer. This is by design: an MLP that can't decide caste-allocation in any reasonable way still produces a working colony because the FSM and pheromone trails handle foraging on their own. (This is also why the saturation bug went undetected for several sessions — the colonies survived despite the brain.)

## 8. Time scaling and reproduction targets

Per-species TOML times are split into:

- **Biology-faithful** values where field data exist (queen lifespan years; egg-larva-pupa stage durations; mature colony-size targets).
- **Game-pacing** values where the field figure would make sim sessions tedious (worker lifespan, founding-period shortcut where the sim starts post-nanitic with `initial_workers ≈ 20`).

Each value in the TOML is annotated `# game-pacing — <reason>` or with an inline citation. Where a value is biology-faithful, the citation is to AntWiki, a primary paper, or a reference work (Hölldobler & Wilson 1990; Czechowski et al. 2002; Stockan & Robinson 2016 for European *Formica*; Tschinkel 2006 for *Solenopsis* methodology).

For reproductions intended for researcher-facing comparison, the **sim's tick-to-real-time mapping is documented per harness**. The default is 1 tick = 1 second (30 Hz × 30 Hz physical second), but headless harness harnesses may scale this for tractable wall-clock — the scaling factor is reported in the harness output alongside the figure.

## 9. What the sim is, and is not

**The sim is.** A tractable, deterministic, ECS-shaped substrate for studying **emergent trail formation, multi-species competitive scenarios, and colony-economy lifecycle** at species-faithful sociometric scales. Each species is a TOML+docs pair grounded in field references; brain choice, climate, and feeding regimes are configurable at runtime.

**The sim is not.** A genome model, a behavioral-genetics model, a microbial-symbiosis model, a 3D nest-architecture model, or an individual-physiology model. Air humidity, soil moisture, photoperiod, and chemical signaling beyond the four pheromone channels are not modeled. Nest geometry is planar; underground chambers are categorical, not topological. Sex determination, alate development, and nuptial flight are abstracted to a single `breeder` caste with no explicit mating mechanic.

For any paper-comparison reproduction, the harness's own `repro/<paper>.md` writeup documents which of the above abstractions are load-bearing for that figure and why we believe the comparison is fair.

## 10. Reproducibility and provenance

- **Source.** [github.com/suhteevah/antcolony](https://github.com/suhteevah/antcolony). Branch `main` is canonical; commit hashes are stable.
- **Determinism flags.** Same OS + same seed + same species TOML + same Cargo.lock = bit-identical sim trajectory.
- **Reproduction harnesses.** Each researcher-facing comparison lives at `crates/antcolony-sim/examples/<paper>_bench.rs`, produces `repro/<paper>.md` with the figure, our number, the published number, the deviation, and the load-bearing-abstractions caveat.
- **Smoke evidence.** Long-horizon multi-species smoke runs are preserved at `bench/smoke-10yr-ai*/` for failure-mode forensics. The MLP saturation evidence at `bench/smoke-10yr-ai-mlp-saturation/` is canonical.

---

*Last updated 2026-05-09. For latest status see [`HANDOFF.md`](../HANDOFF.md).*

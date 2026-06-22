# Arena: Environmental Hazards & Predators

**Date:** 2026-06-22
**Status:** Design (brainstorming → spec review → implementation plan)
**Branch:** `feat/arena-hazards`
**Composes with:** `2026-06-21-ladder-league-design.md`, `2026-06-20-pvp-tournament-yardstick-design.md`

---

## Problem

The current 2-colony arena is symmetric and deterministic: both colonies start in the same
world, face the same food density, and the only uncertainty is opponent behavior. A brain
trained and evaluated only in calm conditions generalizes poorly:

- A colony that wins by pure numerical flooding (e.g., Argentine-ant strategy) is optimal in
  a stable environment but catastrophically brittle when food density collapses or a predator
  thins workers mid-raid.
- The Nash plateau measured at ~47% (MEMORY: `project_ai_ceiling.md`) may partly reflect the
  symmetric payoff structure, not a true skill ceiling — asymmetric environmental pressure
  creates situations where the equilibrium depends on robustness, not just combat.
- The existing seasonal/food machinery (`food_spawn_tick`, `seasonal_scalar`, diapause) is
  already live but the trainer treats it as background noise, not as a strategic variable.
  Hazards make it a decision dimension.

**Core tension to unlock:** *Fight the enemy vs survive the threat.* When a predator is
active or an extreme weather event suppresses foraging, the colony must choose whether to
(a) commit soldiers to the enemy queen raid, (b) divert them to predator defense/alarm,
or (c) pull workers back into the nest and starve the enemy out. No single policy dominates
across all hazard seeds — that's the design goal.

---

## Goal

Add a focused first hazard set to the PvP arena:

1. **One mobile predator type** (spider) — already scaffolded in `hazards.rs`, needs behavior
   and brain observability.
2. **Weather event** (heavy rain) — already scaffolded in `hazards.rs`; needs arena
   activation, seasonal coupling, and species-differentiated impact.
3. **Thermal niche suppression** — reuse `ambient_temp_c()` + `hibernation_cold_threshold_c`
   to create temporal windows where one species is thermally suppressed while the other
   remains active; this is the primary competitive timing lever from the biology.

Eval requirement: every PvP match must be evaluated across ≥3 hazard seeds (calm / moderate
/ intense) so a brain cannot overfit to calm conditions.

---

## Non-Goals

- No antlion (stationary, too similar to nest blocking; deferred).
- No lawnmower (too binary / too lethal; deferred).
- No full predator ecosystem (predator-vs-predator, prey chains) — this is one predator type.
- No player-controlled hazard intervention (Phase 7 / direct control is separate).
- No underground hazard layer (nest flooding, cave-ins) — deferred to Phase 5 spec.
- No change to the pheromone math, FSM transitions, or colony economy structure — this is
  additive on top of existing systems.
- Do not redefine species configs; extend them via the `[hazard_response]` block described
  in `biology-roadmap.md` Phase A schema extensions.

---

## Biology Grounding

### 1. Thermal Niche → Competitive Timing Window

*Source: `docs/biology/interspecific/01-competition-and-displacement.md`, findings B5–B8*

**Brachyponera chinensis displaces Aphaenogaster rudis** because it exploits low-temperature
activity: *B. chinensis* forages at 5–15 °C while *A. rudis* is suppressed; by spring the
*chinensis* colony has already seized resource patches before *rudis* workers emerge.
Empirically: 96% displacement in 10 years (Warren & Chick 2013, cited in Finding B6).

**Sim implication (cited):** The thermal suppression window is the primary lever for
asymmetric matchups. When `ambient_temp_c()` drops below `hibernation_cold_threshold_c` for
species A but not species B, species B has exclusive foraging access. A brain that fails to
exploit this window (or fails to defend against it) loses matchups it should win.

**Temporal niche partitioning** also applies within a day: nocturnal species (`nocturnal:
true` in `AntConfig`) have activity windows that shift when `ambient_temp_c()` peaks above
heat-avoidance thresholds (Finding B3 — activity at intermediate temperatures). Rain events
can override nocturnal suppression by cooling the surface.

### 2. Reserve Labor as Resilience Buffer

*Source: `docs/biology/interspecific/04-temnothorax-defense-dornhaus.md`, Section 1*

~60% of workers in small colonies are persistently inactive (Charbonneau et al. 2015,
Dornhaus group). These "reserve" workers are not lazy — they are available for asymmetric
replacement when task-load spikes unexpectedly. A colony with zero reserve labor (every
worker committed to forage/raid) has no buffer for a sudden predator event.

**Sim implication (cited):** The brain must learn to maintain a reserve: not committing
100% of caste allocation to forage or combat. The hazard system creates the pressure that
makes reserve labor valuable — without hazards, full commitment to forage always dominates.
The `behavior_weights` in `ColonyState` (Nursing/Digging/Forage/Combat weights) are the
control surface.

### 3. Alarm Pheromone → Predator Response Cascade

*Source: `docs/biology/interspecific/02-combat-mechanics.md`, Section 1*

Alarm pheromone is clade-specific (Dufour's gland, venom gland, tibial gland), propagates
at ~15 cm/s equivalent in the sim, and triggers two concentration tiers:
- Low concentration → recruitment to a location (workers approach)
- High concentration → mass evacuation (workers flee)

**Sim implication (cited):** A spider interacting with a worker must trigger alarm deposit.
Nearby workers either reinforce (soldier-heavy colony with high alarm alpha) or flee (small
colony with flight response). This is already encoded in the pheromone grid's `alarm: Vec<f32>`
channel. The predator must use it, not bypass it.

### 4. Group Defense vs a Large Attacker

*Source: `docs/biology/interspecific/02-combat-mechanics.md`, Sections 2–3*

Against a large predator, ants use "spreadeagling" (limb-pulling by multiple workers
simultaneously). Below-linear Lanchester scaling applies in tunnels (θ=0.87, Wilson 1971).
Chemical weapons (formic acid in Formicinae, alkaloids in *Solenopsis invicta*) can knockdown
predators at close range (gaster-flagging Welzel times: 153–187 s).

**Sim implication (cited):** Spider health does not scale linearly with the number of
attacking ants — it scales sub-linearly (θ < 1) in open terrain. If the spider retreats
into a chokepoint (near nest entrance), tunnel geometry gives defenders a multiplicative
advantage. Species with chemical weapons (Formica, Solenopsis-equivalent) get a predator
damage bonus.

### 5. Evacuation Behavior Under Threat

*Source: `docs/biology/interspecific/04-temnothorax-defense-dornhaus.md`, Section 3*

Temnothorax colonies evacuate when threat exceeds quorum threshold. Queen is always
passively carried — never self-evacuates. Aggression during evacuation = 0.15 (very low).
Two-phase emigration: scouts find new site → quorum recruits mass migration.

**Sim implication (cited):** Under extreme predator pressure the colony transitions to
`Fleeing` state at the colony level (not just individual ants). This is distinct from
individual ant `AntState::Fleeing`. A colony-level flight decision: (a) moves nest entrance
spawn point, (b) suppresses foraging, (c) reroutes home-trail pheromone. This is the
"abandon the contested territory" strategy and it costs the colony the foraging ground it
held.

---

## Architecture

### Existing Hooks (do not reinvent)

| Existing machinery | File | Used by this spec |
|--------------------|------|-------------------|
| `food_spawn_tick()` + `seasonal_scalar()` | `simulation.rs:2370–2461` | Rain suppresses food_spawn_rate directly |
| `ambient_temp_c()` sinusoidal | `simulation.rs:1120–1167` | Thermal niche window drives forager suppression |
| `hibernation_cold_threshold_c` per species | `config.rs: AntConfig` | Gating forager activity during thermal suppression |
| `PredatorKind::Spider`, `PredatorState` | `hazards.rs` | Extend, don't replace |
| `Weather` struct (rain timers) | `hazards.rs` | Extend with arena activation |
| `HazardConfig` (spider/rain periods) | `config.rs` | Add `hazard_seed` field; match runner sets it |
| `alarm: Vec<f32>` pheromone channel | `pheromone.rs` | Spider triggers alarm on ant kill |
| `AntState::Fleeing` | `ant.rs` | Individual level; colony-level evacuation is new |
| `temperature_tick()` thermal grid | `simulation.rs:3459` | Spider avoids thermally extreme cells |

**CRITICAL:** `food_spawn_tick()` (seasonal-cliff postmortem fix) already uses a
SplitMix64-seeded ChaCha8 sub-RNG isolated from the main stream. All hazard RNG must use
the same isolation pattern — never touch the main sim RNG stream, or determinism breaks.

---

### New: Spider Predator Behavior (`crates/antcolony-sim/src/hazards.rs`)

`PredatorKind::Spider` is already in the enum. Add a behavior FSM to `Predator` struct:

```rust
// Add to hazards.rs
pub enum SpiderBehaviorState {
    Patrol { heading: f32, ticks_on_heading: u32 },
    Stalk { target_ant_id: AntId, patience: u32 },
    Lunge { target_ant_id: AntId },
    Eating { remaining_ticks: u32 },
    Retreat { toward: Vec2, ticks: u32 }, // flee high-alarm areas
    Dormant { until_tick: u64 },           // thermal suppression
}
```

**Target selection:** Spider targets the nearest ant regardless of colony — it is a shared
threat. Priority: forager workers (non-soldier) in open terrain. If alarm concentration at
the spider's cell exceeds `alarm_retreat_threshold` (config), spider enters `Retreat`.

**Thermal suppression:** Spider is ectothermic. If `ambient_temp_c()` < 8 °C or > 38 °C,
spider enters `Dormant`. This means spiders are inactive during winter/diapause season,
which removes the hazard precisely when both colonies are thermally suppressed anyway. The
competitive pressure from spiders is a spring/summer/autumn phenomenon.

**Respawn:** After kill or escape, spider respawns at a random Outworld edge after
`spider_respawn_ticks` (default: 1800 ticks = ~60 sim-seconds at 30Hz). A match can have
1–3 spiders depending on `hazard_seed`.

**Damage model (sub-linear Lanchester):**
- Spider base health: 200
- Per-ant damage per tick: `base_dmg * colony_chem_bonus * (n_attackers ^ theta)`, where
  `theta = 0.87` (Wilson 1971, cited above) in open terrain, `theta = 1.15` at NestEntrance
  cells (chokepoint advantage)
- Spider kills ant: health -= ant.health * 0.8 (one-hit for workers, 2 hits for soldiers)
- On kill: alarm deposited at kill position, magnitude 3.0 (above mass-evacuation threshold)

**Files to touch:**
- `crates/antcolony-sim/src/hazards.rs` — `SpiderBehaviorState` enum, `spider_tick()`
- `crates/antcolony-sim/src/simulation.rs` — add `hazard_tick()` call in system order
  (after `deposit_system`, before `evaporate_system` — so alarm pheromone gets deposited
  this tick and evaporated next tick)
- `crates/antcolony-sim/src/config.rs` — `HazardConfig` extension fields

### New: Weather / Rain Event (`crates/antcolony-sim/src/hazards.rs`)

The `Weather` struct already tracks `rain_ticks_remaining`. Extend it:

```rust
// Extend Weather in hazards.rs
pub struct RainEvent {
    pub duration_ticks: u32,
    pub intensity: f32,        // 0.0–1.0; drives food suppression + pheromone wash
    pub pheromone_wash_rate: f32, // extra evap multiplier during rain
}
```

**Effects (each tick while raining):**
1. `food_spawn_rate` effective = `base_rate * (1.0 - intensity)` — heavy rain wipes foraging.
2. Pheromone evaporation multiplied: `evap_rate * (1.0 + pheromone_wash_rate * intensity)` —
   rain erodes trails. This is a major strategic disruption: an established food trail is
   degraded. The colony that re-scouts fastest after rain recovers first.
3. Surface temperature drops by `intensity * 4.0 °C` added to `ambient_temp_c()` for the
   rain duration — which can push thermally-sensitive species below their foraging threshold.
4. Spider enters `Dormant` for the rain duration (spiders hide from rain).

**Arena activation:** Rain events are currently dormant (period=0). The arena runner sets
`HazardConfig.rain_event_period_ticks` based on `hazard_seed`. Seeds:
- `calm (0)`: no rain
- `moderate (1)`: rain every ~30 sim-minutes (54,000 ticks at 30Hz), duration 900 ticks
- `intense (2)`: rain every ~15 sim-minutes, duration 1800 ticks, intensity 0.9

**Files to touch:**
- `crates/antcolony-sim/src/hazards.rs` — `RainEvent` struct, `rain_tick()` which writes to
  both the pheromone grid and the food spawn rate override
- `crates/antcolony-sim/src/simulation.rs` — pass food spawn rate override from `Weather`
  into `food_spawn_tick()` (add an `override_rate: Option<f32>` parameter)
- `crates/antcolony-sim/src/config.rs` — add `hazard_seed: u8` to `HazardConfig`

### New: Thermal Niche Suppression (no new files — wire existing)

The machinery is fully live. What's missing is the arena matchup runner asserting a
biologically-grounded starting DOY that creates a meaningful thermal window.

For the *B. chinensis* / *A. rudis* matchup:
- `starting_day_of_year = 60` (early March) — *chinensis* can forage, *rudis* cannot
- `hibernation_cold_threshold_c` for *rudis* ≈ 10 °C (from `species.rs` or `species_extended.rs`)
- `ambient_temp_c()` at DOY 60 with default climate = ~8 °C → *rudis* suppressed
- *chinensis* forages freely for ~30 sim-days before *rudis* wakes up

**This requires zero new code.** It requires the arena matchup config (the TOML or the
`MatchupConfig` struct in the trainer) to set `starting_day_of_year` per matchup rather than
defaulting to 0.

**Files to touch:**
- `crates/antcolony-trainer/src/env.rs` — expose `starting_day_of_year` in `EnvConfig`,
  pass through to `SimConfig`
- `crates/antcolony-sim/src/config.rs` — verify `WorldConfig::starting_day_of_year` is
  plumbed to `day_of_year()` (it already exists; confirm the plumbing in `simulation.rs`)

---

### Brain Observability — New Observation Features

The brain must see hazards or it cannot respond. Add to the blackboard
(`crates/antcolony-sim/src/ai/blackboard.rs`):

```rust
// In ColonyBlackboard or equivalent obs struct
pub spider_nearest_dist_normalized: f32,  // 0.0 = on top of nest, 1.0 = off-screen
pub spider_nearest_heading: f32,          // angle from nest to spider, normalized 0..1
pub spider_count_visible: u8,             // spiders within sense_radius of any forager
pub alarm_pheromone_max: f32,             // max alarm in colony's territory, normalized
pub is_raining: bool,
pub rain_intensity: f32,
pub ambient_temp_normalized: f32,         // (temp - 0) / 50, so 0.0=freezing, 0.6=30°C
pub own_forager_suppression: f32,         // fraction of own foragers currently below thermal threshold
pub opp_forager_suppression: f32,         // same for opponent — from sim state (not real hidden info; it's observable via pheromone silence)
```

`opp_forager_suppression` is a derived signal, not a hidden state cheat: it is estimated
from the opponent's pheromone trail activity rate (already observable). A brain that
correlates `ambient_temp_normalized` + `opp_forager_suppression` can learn to exploit the
thermal window without being given explicit species identity.

**Files to touch:**
- `crates/antcolony-sim/src/ai/blackboard.rs` — add fields above
- `crates/antcolony-trainer/src/hierarchical/obs_to_tensors.rs` — serialize to tensor
  (append at end of obs vector; don't reorder existing features or weights break)

---

### System Execution Order Addition

Insert into `simulation.rs` fixed-update loop between step 4 (deposit) and step 6
(evaporate):

```
4. deposit_system        — Ants write pheromone
4.5 hazard_tick()        — Spider moves, attacks, deposits alarm; rain applies wash multiplier
5. combat_system         — Ant-vs-ant (unchanged)
6. evaporate_system      — NOW includes rain wash multiplier if raining
```

`hazard_tick()` is a single function that dispatches `spider_tick()` and `rain_tick()`
internally. It takes `&mut PheromoneGrid`, `&mut Vec<Predator>`, `&Weather`, `&HazardConfig`,
`ambient_temp: f32`, `doy: u32`, and the ant entity list.

---

## Emergent Strategies Unlocked

### 1. Thermal Window Exploitation
A brain playing *B. chinensis* (low cold threshold) learns to raid aggressively in early
spring before *A. rudis* wakes up. A brain playing *A. rudis* learns to stockpile food
during summer (when both are active) to survive the winter gap where *chinensis* can still
forage but *rudis* cannot — a defensive resource strategy.

### 2. Predator-Diversion Tactic
When a spider is active near the opponent's foraging area, a brain can *delay* committing
soldiers to a queen raid — letting the spider thin the opponent's workers — and then rush
the raid with numerical advantage. This requires the brain to correlate `spider_nearest_dist`
for the opponent's territory with its own attack timing.

### 3. Rain-After-Trail
If a rain event washes the opponent's pheromone trails, the opponent must re-scout food
sources from scratch. A brain that immediately sends fast scouts post-rain can claim food
patches before the opponent's explorers find them again. This requires sensing `is_raining`
→ suppress current activity → sense `is_raining = false` → burst-scout.

### 4. Reserve Labor Pays Off
A brain that keeps 20–30% of workers as un-tasked reserve survives a surprise spider event
with minimal disruption — the reserve absorbs the alarm-driven redirect without collapsing
foraging. A brain with 0% reserve chaos-routes all workers into alarm response and loses
its food trail entirely. This pressure did not exist before hazards.

### 5. Robustness Across Seeds
A brain that wins in `calm` seed only is brittle and will fail arena promotion. The gate
condition requires positive EV across all three seeds (see Success Criteria). This forces
the brain to learn policies that are neither exploitatively aggressive (brittle under hazard)
nor purely defensive (loses in calm).

---

## Training / Eval Implications

### Hazard Seeds in Evaluation

Every match in the PvP tournament and ladder league must be run across 3 hazard seeds:

```toml
# In match config
hazard_seeds = [0, 1, 2]   # calm, moderate, intense
```

Win rate = mean winrate across seeds. A brain with `[1.0, 0.5, 0.0]` across seeds has
effective winrate 0.50 — same as coin-flip. This is intentional: the hazard seeds are
difficulty axes, not just randomness.

### Training Curriculum

Do not add hazards on day 1 of training. Use a curriculum:
- Rounds 1–N_warmup: `hazard_seed = 0` only (existing behavior, no regression)
- Round N_warmup+1: introduce `hazard_seed = 1` in 50% of training matches
- Round N_warmup+K: promote to full 3-seed evaluation

`N_warmup` = the round at which the candidate first achieves `h2h ≥ 0.55` in the calm seed
(per ladder league gate). Only then does hazard training begin. This prevents hazards from
destabilizing a still-forming policy.

### Observation Vector Compatibility

New obs fields are appended at the end of the tensor. Existing pretrained weights remain
valid — the new input channels initialize to zero-weight and learn from scratch. Do not
insert fields in the middle of the obs vector (breaks `obs_to_tensors.rs` indexing).

Verify tensor dimension in `obs_to_tensors.rs` after adding fields; assert in
`hierarchical_smoke` test.

---

## Testing

### Unit Tests (add to `crates/antcolony-sim/tests/`)

1. `spider_deposits_alarm_on_kill` — spawn one spider, one worker ant, tick until kill,
   assert `pheromone.alarm[kill_cell] > 2.5`.
2. `rain_suppresses_food_spawn` — run 1000 `food_spawn_tick()` calls with `is_raining=true,
   intensity=1.0`, assert food count is within 5% of `base_rate * 0 = 0`.
3. `rain_accelerates_evaporation` — deposit pheromone, run 100 ticks with rain, compare
   remaining pheromone to baseline (non-rain), assert rain residual < 0.5 * baseline.
4. `spider_retreats_from_high_alarm` — inject alarm > retreat threshold at spider cell,
   tick spider, assert it transitions to `SpiderBehaviorState::Retreat`.
5. `spider_dormant_below_thermal_threshold` — set `ambient_temp = 5.0`, tick spider, assert
   state = `Dormant`.
6. `thermal_window_suppresses_rudis_foragers` — configure rudis at DOY 60 default climate,
   assert `own_forager_suppression > 0.9` in blackboard.

### Integration Tests (add to `tests/`)

7. `hazard_seed_2_both_colonies_survive_100_ticks` — sanity: intense hazards must not kill
   both colonies within 100 ticks (would indicate tuning is too lethal).
8. `calm_seed_matches_baseline_winrate` — run 20 matches at `hazard_seed=0`, assert winrate
   vs. heuristic is within ±0.05 of the pre-hazard baseline (no regression).

---

## Success Criteria

1. **No regression in calm:** SOTA brain winrate vs. HeuristicBrain at `hazard_seed=0`
   within ±3% of pre-hazard baseline.
2. **Hazards are non-lethal tuned:** Both colonies survive ≥ 95% of 500-tick runs at
   `hazard_seed=2` with HeuristicBrain (no training) — hazards stress, they don't instantly
   kill.
3. **Hazard observability works:** After 10K training steps with hazards enabled, the new
   obs features (`spider_nearest_dist_normalized`, `is_raining`) have non-zero gradient
   (confirmed via parameter grad norm logging in trainer).
4. **Thermal window is exploitable:** In a `rudis` vs. `chinensis` matchup at
   `starting_day_of_year=60`, a trained brain (any brain, including HeuristicBrain+species
   config) achieves colony food at tick 500 ≥ 1.5× the opponent's food — confirming the
   thermal window produces asymmetric resource access before training even begins.
5. **Promoted hazard-trained brain:** Achieves mean winrate ≥ SOTA across all 3 hazard seeds
   (not just seed 0) in the ladder-league gate evaluation.

---

## Open Questions

1. **Spider lethality tuning:** At what spider health / respawn rate does the predator become
   a nuisance vs. a genuine threat? Start with `spider_health=200, respawn=1800` and tune
   against criterion 2 above. Do not tune by feel — use the integration test.

2. **Alarm pheromone cascade distance:** The current `alarm` evaporation rate (0.02/tick,
   same as food/home trail) may not propagate alarm fast enough to warn foragers before the
   spider reaches them. Consider a species-specific alarm decay that is slower than food
   trail decay. Verify with unit test 1.

3. **`opp_forager_suppression` derivation:** The cleanest implementation is to directly read
   the opponent colony's active forager count from `ColonyState` (it's in-process sim state
   during training). This is not a "cheat" in self-play (both sides read it). In eval against
   a deployed brain, it's estimated from pheromone silence. Document which is used where.

4. **Rain and diapause interaction:** If it rains during winter (when both colonies are in
   diapause), rain has no competitive effect. Is that correct? Biologically, winter rain does
   not restart foraging. Confirm that the `hibernation_required` gate fires before
   `food_spawn_tick()` regardless of rain — trace the order in `simulation.rs`.

5. **Multiple spider colony targeting:** With 2 spiders (hazard_seed=2), should they target
   the same colony or different ones? Purely random target selection naturally distributes
   them, but may create runs where both spiders hammer one colony. Consider adding a soft
   repulsion between spiders (spiders avoid cells already occupied by another spider).

6. **Species-differentiated chemical defense bonus:** The `colony_chem_bonus` in the damage
   model needs a per-species config field. Where does it live? Candidate: `AntConfig` or
   `HazardConfig`. Prefer `AntConfig` (it's a species trait, not a world parameter).

---

## File Change Summary

| File | Change |
|------|--------|
| `crates/antcolony-sim/src/hazards.rs` | `SpiderBehaviorState` enum; `spider_tick()`; `RainEvent` struct; `rain_tick()` |
| `crates/antcolony-sim/src/simulation.rs` | Add `hazard_tick()` between deposit and evaporate; pass rain multiplier to evaporate and food spawn |
| `crates/antcolony-sim/src/config.rs` | `HazardConfig`: add `hazard_seed: u8`, `spider_respawn_ticks`, `alarm_retreat_threshold`; `AntConfig`: add `colony_chem_bonus: f32` |
| `crates/antcolony-sim/src/ai/blackboard.rs` | Append 8 new hazard obs fields |
| `crates/antcolony-trainer/src/hierarchical/obs_to_tensors.rs` | Serialize new obs fields; assert tensor dim |
| `crates/antcolony-trainer/src/env.rs` | Expose `starting_day_of_year` in `EnvConfig` |
| `crates/antcolony-sim/tests/` | 6 new unit tests |
| `tests/` | 2 new integration tests |
| `assets/config/simulation.toml` | Default `HazardConfig` values for all three hazard seeds |

**No new crates. No new binaries. No changes to pheromone math, ant FSM, or colony economy.**

# Arena Cross-Species 1v1 — Asymmetric Two-Species Combat

**Date:** 2026-06-22
**Status:** Design draft (brainstorming) — pending spec review → implementation plan
**Branch:** `feat/arena-cross-species`
**Companion biology:** `docs/biology/interspecific/{01,02,03,04,05}-*.md` (every design choice below cites a finding)
**Builds on:** `docs/pvp-mode-design.md` (win = kill enemy queen), `docs/superpowers/specs/2026-06-21-ladder-league-design.md` (training format)

---

## Problem

Today's PvP arena is **within-species**: `MatchEnv`/`new_ai_vs_ai_with_topology` build *both* colonies from a single `SimConfig`, and `species::apply(&env)` configures the whole sim (ant speed/attack/health, brood timings, forage, pheromone deposit) — colony 0 and colony 1 are biologically identical and only the **brains** differ. That makes the tournament a pure policy contest on a symmetric board.

The on-roadmap science (Warren's *Brachyponera chinensis* displacing *Aphaenogaster rudis*) and the entire `docs/biology/interspecific/` corpus are about **asymmetric** matchups: two *different* species with different bodies, weapons, recruitment, and colony sizes clashing. The sim cannot express that. Every species param funnels through one `SimConfig`, so there is no way to say "colony 0 is *B. chinensis*, colony 1 is *A. rudis*."

## Goal

Make a 1v1 arena match where **colony 0 runs species A and colony 1 runs species B**, the two are genuinely asymmetric, and the win condition stays "kill the enemy queen" — but the queen-kill becomes a **gated, two-phase, interruptible usurpation channel** grounded in real ant social parasitism, not an instant snipe.

Concretely:
1. **Per-colony species wiring** — `Simulation` carries one `SimConfig` *per colony*; `new_two_colony_with_topology` accepts two species (or two pre-baked configs).
2. **Cross-species combat resolution** — per-species attack/health, a venom×resistance susceptibility matrix, recruitment-gated reinforcement, Lanchester Linear-Law group math with a terrain-gated `max_simultaneous_attackers`.
3. **Gated two-phase queen-kill / usurpation channel** — no early rush-snipe; defenders can interrupt.
4. **Measurable intransitivity** — a cross-species win matrix harness so the roster isn't a foregone "*B. chinensis* always wins."
5. **Brain/training extension** — species-conditioned observation so the HAC (or per-species champions) can play a cross-species roster, extending the Ladder League.

## Non-Goals (YAGNI)

- **N-colony / FFA.** Hard 1v1, exactly two colonies, exactly two species (which may be equal for back-compat). N-colony is a separate spec. The engine stays 2-colony (matches the tournament, Ladder League non-goal).
- **Full social-parasite content.** Brood theft, slave-rebellion, propaganda, matricide-by-proxy, infiltration-stealth (biology 05 Findings 5–7, 13–14) are *flavor/Phase 2+*. MVP ships exactly ONE queen-kill template (gated brute-force channel) with the gate + interrupt that make it balanced.
- **New netcode / lobby UI.** The sim already round-trips byte-deterministically (MEMORY: `project_determinism`); per-colony species must preserve that. Lobby/species-select UI is `docs/pvp-mode-design.md` P3, out of scope here.
- **New reward shaping.** Training reuses `assets/reward/terminal.toml` (Ladder League non-goal).
- **Per-tick venom chemistry.** Venom is a damage scalar + susceptibility matrix + a flee-threshold bias, NOT a simulated chemical field. `[cite: 02 §3 sim implication — "model as a state flag, no chemistry sim needed"; 05 Finding 9 sim implication]`

---

## Biology grounding (the findings that shape the design)

Each row is a load-bearing design driver. Citations are `file:Finding` into `docs/biology/interspecific/`.

| # | Finding | Design consequence |
|---|---------|--------------------|
| B1 | **Colony size / worker count is the #1 *causal* combat predictor** — rank *reverses* when worker counts are experimentally swapped; open combat is a war of attrition near 1:1 mortality. | Worker count is the **primary** input to open-field resolution; everything else is a modifier. Get this first. `[cite: 05 Finding 18 (Palmer 2004 — experimental rank reversal)]` |
| B2 | **Body size vs numbers are orthogonal axes** — behavioral (big-worker) and numerical dominance are *negatively* correlated; a species wins by big workers OR by flooding, two different counters. | `worker_attack`/`worker_health` (per-worker power, from body size) and `target_population`/recruitment (numbers) must be **independent** species params. Don't collapse them. `[cite: 01 Finding 3 (Nelson & Mooney 2025); 01 Finding 15]` |
| B3 | **Ant combat follows Lanchester's *Linear* Law (θ≈1.0), not Square** — 2× numbers ≈ 2× power; **terrain shifts θ below 1.0**: 20 large ants beat up to 200 small ants in 10mm corridors. | Open terrain = linear group math (numbers ≈ linear); tunnels/entrances cap **how many can apply force at once** (`max_simultaneous_attackers`), letting a small fortified colony hold. `[cite: 02 §2 Lanchester (Plowes & Adams 2005 θ=1.04; Lymbery 2023 θ=0.87 complex); 05 Finding 17]` |
| B4 | **Chemical weapons are clade-specific and categorically asymmetric** — fire-ant venom one-shots Argentine ants (0.50µg flag > 0.489µg LD50) but is ~330× weaker on fire ants themselves; **counterable by detox** (*N. fulva* formic-acid self-grooming: 98% vs 48% survival). | A `(attacker_clade venom) → (defender species)` **susceptibility matrix** + optional per-species `venom_resistance`. Source of rock-paper-scissors. `[cite: 02 §3 (LeBrun 2014; Chen 2012; Greenberg 2008 — 684× resistance span); 05 Finding 21]` |
| B5 | **Sting potency → behavioral aversion, not just damage** — *A. rudis* (myrmicine, reduced sting, `aggression≈0.25`) flees ponerine *B. chinensis* at a lower threat threshold; reproduces the 96% displacement without killing every ant. | High `sting_potency` attacker applies a **flee-threshold multiplier** on a naive defender (lowers the alarm level at which it flees). `[cite: 01 Finding 7 (novel weapons, Callaway 2004) + Lever 2; 01 Finding 11 (Rodriguez-Cabal 2012 — 96%)]` |
| B6 | **Recruitment mode is decoupled from per-worker power** — mass recruiters (*A. rudis* group-recruits) flood a resource fast; individual scouts (*B. chinensis*) win per-encounter but can't flood. Below a colony-size threshold, mass recruitment can't be maintained → fight 1v1. | `recruitment_deposit_scalar` (already in `species.rs`) governs how fast a colony brings bodies to a contested cell; gate mass-reinforcement on min colony size. `[cite: 02 §1; 01 Finding 14; 05 Finding 19 (Bertelsmeier 2015)]` |
| B7 | **Displacement mechanism for B. chinensis is lethal predation, not flooding** — it *kills and consumes* the resident ant (`predates_ants`) rather than evicting it; the 96% drop needs this hookup. | The cross-species combat *consumes* the loser (corpse → attacker food), feeding back into B. chinensis growth. (MVP: corpse-food already exists; route it to the killer's store as a tunable.) `[cite: 01 Finding 10 (Bednar 2013); 01 Lever 1; 01 Finding 11]` |
| B8 | **Cross-species queen-kill is a real, well-documented social-parasitic usurpation** done five ways, and it is **timed + interruptible + disguise-gated** — parasites do NOT snipe young queens (zero aggression through 190 days; 100% attack by ~204 days), and acceptance requires a post-kill chemical-disguise step; takeover fails if interrupted. | The win condition = a **gated, two-phase, interruptible channel** (see "Queen-kill" §). This is the central mechanic AND its balance solution in one. `[cite: 05 Finding 8 (Johnson 2002 — timing gate); 05 Finding 9 (Topoff & Zimmerli 1993 — disguise); 05 Finding 10 (brute-force template); 05 "Realism Verdict"]` |
| B9 | **Founding/incipient colonies are the maximum-vulnerability window** — incumbents suppress small heterospecific colonies; a mature fortified nest's queen is *not* reachable. | The gate (B8) ties queen-vulnerability to the attacker out-dominating/occupying the defender locally; mirrors "win while vulnerable, or out-grow into invulnerability." `[cite: 05 Finding 23 (Tschinkel 2017); 05 Finding 8]` |
| B10 | **Home-ground / residency advantage** — a colony fighting inside its own `colony_scent` field wins fights it would lose elsewhere. | Optional combat multiplier inside own `ColonyScent`. Counterweight to pure numbers so attacking an established nest is meaningfully harder. `[cite: 05 Finding 22 (Gordon 1992); 05 Finding 16]` |

---

## Architecture — per-colony species wiring

### Current state (verified in code)

- `Species::apply(&Environment) -> SimConfig` (`species.rs:291`) folds biology → one tick-denominated `SimConfig`.
- `Simulation` holds **one** `pub config: SimConfig` (`simulation.rs:58`).
- `new_two_colony_with_topology(config, topology, seed, nest_black, nest_red)` builds **both** colonies from that single `config` (`simulation.rs:191`). `new_ai_vs_ai_with_topology` wraps it.
- `combat_tick` reads `self.config.combat` **once** for both sides (`simulation.rs:1993`) — both colonies share `worker_attack`, `worker_health`, etc.
- Economy/brood/forage all read `self.config.colony` / `self.config.world` — also shared.
- `MatchEnv::new` (`trainer/src/env.rs:60`) builds a hand-rolled default `SimConfig` and never calls `species::apply` at all — the trainer currently has **no species** in it.
- **Stub to flag:** `predates_ants = true` is present in `assets/species/brachyponera_chinensis.toml:77` but **is NOT a field on `DietExtended`** (`species_extended.rs:253`) — serde silently drops it. The displacement mechanism (B7) is currently unwired. Must be added.

### The change: a per-colony config

Introduce a per-colony slice of the config that combat/economy read by colony id, leaving the world/pheromone grid global (one shared arena, one pheromone field — both correct: the surface is shared in PvP).

**`config.rs` — split out the per-colony fields (additive, no behavior change for single-colony):**

```rust
/// The slice of SimConfig that differs between colonies in a cross-species match.
/// `ant`, `colony`, `combat` are per-species; `world`, `pheromone`, `hazards`
/// stay global (one shared arena + one pheromone field).
#[derive(Debug, Clone)]
pub struct ColonySimConfig {
    pub ant: AntConfig,
    pub colony: ColonyConfig,
    pub combat: CombatConfig,
    pub species_id: String,   // for obs/logging/win-matrix labelling
}
```

**`simulation.rs` — `Simulation` carries per-colony configs:**

```rust
pub struct Simulation {
    pub config: SimConfig,                 // KEEP: global world/pheromone/hazards + colony[0] back-compat
    pub colony_configs: Vec<ColonySimConfig>,  // NEW: indexed by colony id; len()==colonies.len()
    // ...
}
```

- `config` stays the source of truth for `world`/`pheromone`/`hazards` and remains a valid full config for single-colony sims (`colony_configs[0]` mirrors it). All existing single-colony constructors push exactly one `ColonySimConfig` derived from `config`, so `Simulation::new`/`new_with_topology` are byte-identical.
- Add `fn colony_cfg(&self, colony_id: u8) -> &ColonySimConfig`.

**`Species::apply` split (back-compat preserved):**
- Keep `apply(&env) -> SimConfig` exactly as is (it's tested + relied upon).
- Add `apply_colony(&env) -> ColonySimConfig` that returns just the per-colony slice (it already computes `ant`/`colony`/`combat` internally — `species.rs:335–454` — so this is a refactor that extracts those three into the new struct, with `apply` calling it). **Determinism guard:** `apply` must produce a bit-identical `SimConfig` before and after the refactor (snapshot test).

**Two-species constructor:**

```rust
pub fn new_two_colony_cross_species(
    world_pheromone_hazards: SimConfig, // global slice; world dims, pheromone, hazards
    cfg_black: ColonySimConfig,         // colony 0 = species A
    cfg_red: ColonySimConfig,           // colony 1 = species B
    topology: Topology,
    seed: u64,
    nest_black_module: ModuleId,
    nest_red_module: ModuleId,
) -> Self
```

- Body is the existing `new_two_colony_with_topology` with each colony's ants spawned from its own `cfg_*.ant`/`cfg_*.colony` (spawn count, caste ratio, food cap) and `colony_configs = vec![cfg_black, cfg_red]`.
- **Refactor old constructor to delegate:** `new_two_colony_with_topology(config, ...)` calls the new one with `cfg_black == cfg_red == ColonySimConfig::from(&config)` — identical behavior, so all current callers/tests pass unchanged.
- `fit_bore_to_species` (topology.rs:474) currently takes one species' worker size; in cross-species, call it with `max(size_A, size_B)` (or per-tube based on which colony's nest the tube serves) so neither species is bore-gated out of its own tunnels.

### Files / functions to change

| File | Change |
|------|--------|
| `crates/antcolony-sim/src/config.rs` | Add `ColonySimConfig`; `impl From<&SimConfig>`. |
| `crates/antcolony-sim/src/species.rs` | Extract `apply_colony(&env) -> ColonySimConfig`; `apply` delegates. Add new combat/diet fields to the per-colony combat (venom clade, resistance, predates_ants) — see "Combat model" + "Species TOML additions". |
| `crates/antcolony-sim/src/species_extended.rs` | Add fields: `DietExtended.predates_ants: bool` (wires the dropped TOML key, B7); `CombatExtended.venom_resistance: f32`; reuse existing `weapon: Weapon` as the venom *clade* for the matrix; add `flee_threshold_multiplier` derivation (B5). |
| `crates/antcolony-sim/src/simulation.rs` | Add `colony_configs`; `colony_cfg()`; `new_two_colony_cross_species`; old ctor delegates; `combat_tick` reads per-attacker/per-defender config; queen-kill channel (new state); corpse-food→killer routing (B7). |
| `crates/antcolony-sim/src/combat.rs` (NEW — extract from `simulation.rs`) | Per CLAUDE.md's flat-file rule, lift `combat_tick` (~170 lines) + the new susceptibility matrix + Lanchester grouping into `combat.rs`. (CLAUDE.md's architecture lists `combat.rs`; it doesn't exist yet — this is the right time.) |
| `crates/antcolony-trainer/src/env.rs` | `MatchEnv::new_cross_species(species_a, species_b, seed)`; `MatchEnv::new` keeps default (back-compat). Load species via `species::load_species_dir`. |

---

## Cross-species combat model

All combat is resolved per substep in `combat.rs` (lifted from `combat_tick`). The current model is symmetric melee with a shared config; the changes are localized to (a) **which config each ant uses** and (b) **damage scaling**.

### 1. Per-side attack/health
For attacker ant `i` (colony `ci`) hitting defender `j` (colony `cj`):
- `base_attack = colony_cfg(ci).combat.{worker,soldier}_attack`
- defender health/HP pool comes from `colony_cfg(cj).combat.{worker,soldier}_health` (set on the ant at spawn from its own config — already how `spawn_initial_ants` works, just sourced per-colony).

This alone makes a *B. chinensis* worker (`worker_attack=1.4`, `worker_health=7.0`) trade differently against an *A. rudis* worker than the symmetric default. `[cite: 01 Finding 15]`

### 2. Venom × resistance susceptibility matrix `[cite: 02 §3; 05 Finding 21]`
A small clade-indexed function (NOT a chemistry sim):

```
effective_dmg = base_attack
              * venom_multiplier(attacker.weapon, attacker.sting_potency, defender.species_clade)
              * (1.0 - defender.venom_resistance.clamp(0,0.9))
```

- `venom_multiplier`: a static matrix keyed on `(Weapon, defender_clade)`. Ponerine **Sting** vs a naive myrmicine/dolichodorine = high (e.g. 1.5–2.0, reflecting the asymmetric LD50 span — Greenberg's 684× is the upper bound, we use a tame in-game spread); same-clade or sting-experienced = 1.0. Formicine **FormicSpray** vs fire-ant-clade = high; etc. `[cite: 02 §3 (LeBrun 2014; Greenberg 2008)]`
- `venom_resistance` (new `CombatExtended` field): the *N. fulva* detox counter — a high-formic-acid formicine takes ≤0.5 from alkaloid attacks, flipping an otherwise-lost matchup. Default 0.0. `[cite: 02 "venom as anti-venom"; 05 Finding 21 sim impl]`
- **Clade** is derived from `genus`/`Weapon` (e.g. Ponerinae = `Brachyponera`; Formicinae = `Formica`/`Camponotus`/`Lasius`; Myrmicinae = `Aphaenogaster`/`Pogonomyrmex`/`Tetramorium`; Dolichoderinae = `Tapinoma`). Store as an enum on `ColonySimConfig` baked at `apply_colony`.

### 3. Flee-threshold bias (behavioral displacement) `[cite: 01 Finding 7, Lever 2; B5]`
When a defender takes damage from an attacker with `sting_potency > 1.0` and the defender's clade is *naive* to that venom, lower the alarm threshold at which workers/breeders transition to `Fleeing` (in the existing `combat_tick` state assignment, `simulation.rs:2072–2086`). This reproduces the 96% displacement as *retreat*, not just kills — *A. rudis* abandons contested ground to *B. chinensis*. Soldiers still stand (Fighting).

### 4. Recruitment-gated reinforcement `[cite: 02 §1; 05 Finding 19; B6]`
Reinforcement to a contested cell already emerges from the alarm pheromone layer + foraging FSM. Per-species `recruitment_deposit_scalar` (already wired, `species.rs:49`) makes mass recruiters bring bodies faster; individual scouts (*B. chinensis* `recruitment="individual"` → scalar 0.0) can't flood. No new system — just confirm the per-colony pheromone deposit uses the *depositing colony's* scalar. (It already bakes into `deposit_food_trail`/`deposit_home_trail`, which are in the *global* `PheromoneConfig` today — **this must move to per-colony**: deposit strength is a property of the *depositor's* species. Route alarm/trail deposit through `colony_cfg(ant.colony_id).` Add `deposit_*` to a per-colony pheromone slice, or carry the scalar on `ColonySimConfig` and multiply at deposit sites.)

### 5. Lanchester Linear Law + terrain-gated `max_simultaneous_attackers` `[cite: 02 §2; 05 Finding 17; B3]`
The current model already approximates **Linear Law** for free: damage accumulates per attacking ant within `interaction_radius`, so N attackers ≈ N× damage (θ≈1.0) — *as long as all N can reach the target*. The terrain modifier is the missing piece:

- Add `combat.max_simultaneous_attackers_open` (default high, e.g. 8 — effectively uncapped on surface) and `combat.max_simultaneous_attackers_tunnel` (default 2) and `..._entrance` (default 1).
- In `combat_tick`, after collecting candidate attackers for a defender, **cap the number that deal damage this substep** by the cell's terrain class (look up the defender's module kind / whether the cell is a nest-entrance or tunnel/underground cell — `module.kind == UndergroundNest`, or on a `NestEntrance` cell). Excess attackers wait (still occupy the cell). This makes a chokepoint convert numbers→stalemate (20 elites hold vs 200 floods). `[cite: 02 §2 (Lymbery 2023 θ=0.87); 05 Finding 17 (Champer & Schlenoff 2024)]`
- This is the lever that lets the **smaller, individually-stronger** species (*B. chinensis*) survive a numerical *A. rudis*/flooder by fighting in its own tunnels — and conversely lets a flooder win on the open surface. It is the structural source of intransitivity (see Balance §).

### 6. Lethal predation / corpse routing `[cite: 01 Finding 10; B7]`
When `colony_cfg(killer).diet_ext.predates_ants` is true and the victim is an enemy ant, route the corpse-food (already dropped at the death site, `simulation.rs:2138`) to the **killer colony's `food_stored`** (a tunable fraction) instead of (or in addition to) leaving it as terrain. This closes the displacement feedback loop: *B. chinensis* kills → eats → grows → kills more. Gate behind the new `predates_ants` field so non-predatory species are unaffected.

---

## The queen-kill / usurpation mechanic (gated, two-phase, interruptible)

This is the central new mechanic and its balance solution. It replaces "queen HP hits 0 → win" with a channel that is **gated** (B9), **two-phase** (B8), and **interruptible**. `[cite: 05 Part II "Realism Verdict & Recommended Model"; 05 Finding 8; 05 Finding 9; 05 Finding 10]`

### Phase 0 — Gate (queen not targetable)
The enemy queen is **invulnerable** until the attacker has established local dominance:
- Condition (tunable): attacker has ≥ `usurp_gate_attacker_ratio` (default 3:1) live attacker ants within the defender's **queen-chamber module** AND the defender's adult population in that module < `usurp_gate_defender_floor` (default a small N), i.e. the attacker has *occupied* the nest.
- Mirrors "parasite waits until the host workforce is large enough / defenders cleared" — no lone infiltrator snipes the queen at t=0. `[cite: 05 Finding 8 (Johnson 2002 — young queens not attacked; 100% by ~204 days); 05 Finding 23]`

### Phase 1 — Channel (attacker exposed, defenders can interrupt)
Once gated open, an attacker adjacent to the queen enters a new `AntState::Usurping` and channels for `usurp_channel_ticks` (default ~N ticks, the "~25–30 min repeated bite-and-lick" of *Polyergus*):
- The queen's `queen_health` drains over the channel (not instant).
- The **channeling ant is exposed**: if it dies or is forced to `Fleeing` (took enough damage from defenders) before the channel completes, the channel **resets** (queen_health does NOT continue draining; partial progress decays). Defenders rallying to the queen chamber is the counter-play. `[cite: 05 Finding 9 (interruptible bite-and-lick); 05 Finding 10 (high risk if defenders present)]`
- A species with high `sting_potency`/a future Dufour "repellent" trait gets a longer protected window (extends channel survivability) — flavor hook, not MVP-required. `[cite: 05 Finding 9 (Dufour repellent); 05 Finding 14]`

### Phase 2 — Resolution (acquire colony identity → workers defect)
On channel completion (`queen_health <= 0`):
- `match_status()` resolves `Won{winner=attacker}` exactly as today (queen dead → win), so the existing detection (`simulation.rs:315`) is reused.
- **Payoff (MVP-optional, strong flavor):** surviving defender workers in the queen-chamber module stop attacking and **defect** to the attacker (flip `colony_id`), modeling post-kill CHC disguise / "the enemy's workers are now yours." Gate behind a tunable so the pure "queen dead = win" path is unaffected for tournament scoring. `[cite: 05 Finding 9 (Topoff & Zimmerli 1993 — workers accept the disguised killer); 05 Finding 15 (traitor defection)]`

### Implementation
- New `AntState::Usurping` (extends the FSM in `ant.rs`).
- New `combat.rs` sub-pass `usurp_tick`: detect gate per defender colony; promote an eligible adjacent attacker into `Usurping`; drain queen_health; handle interrupt/reset.
- New `CombatConfig` knobs: `usurp_gate_attacker_ratio`, `usurp_gate_defender_floor`, `usurp_channel_ticks`, `usurp_defect_workers: bool`, `usurp_corpse_to_killer_frac`.
- **Determinism:** the gate/channel must be evaluated in a fixed, id-sorted order (no HashMap iteration affecting outcome) to preserve byte-determinism (MEMORY `project_determinism`).

### Variants (Phase 2+, flavor only — NOT MVP)
The five real mechanisms (brute-mandible / slow-throttle / decapitation / post-kill-disguise / matricide-by-proxy) are *re-skins of the same gated channel* with different risk profiles and entry tactics; the matricide-by-proxy "turncoat" (mark the enemy queen → her own workers kill her) is the showcase chemical archetype. Defer all but the brute-force template. `[cite: 05 Findings 10–13; 05 Finding 9; 05 "Realism Verdict" 1–5]`

---

## Balance / intransitivity

The goal is a roster with **complementary strengths** (rock-paper-scissors), measured, not assumed. The biology gives three orthogonal win-paths that naturally counter each other:

| Win-path | Mechanism | Beaten by | Cite |
|----------|-----------|-----------|------|
| **Big-workers / venom specialist** (*B. chinensis*: high per-worker, sting, predation) | wins per-encounter; lethal predation feedback | **Flooding on open terrain** (Linear Law: enough bodies overwhelm before sting accumulates) | `[cite: 01 Finding 15; 02 §2 Lymbery open θ=1.05; 05 Finding 18]` |
| **Flooding / fast recruitment** (mass recruiter, high `target_population`) | numbers dominate open ground | **Chokepoint defense** (`max_simultaneous_attackers_tunnel=2` neutralizes numbers; 20 hold vs 200) AND **venom resistance** | `[cite: 02 §2 (Lymbery complex θ=0.87); 05 Finding 17; 02 §3 detox]` |
| **Fortified / home-ground** (durable, tunnel-fighter, e.g. Camponotus) | holds chokepoints, home-ground bonus | **Out-growing** (B9: out-produce → reach the gate while still vulnerable) OR ranged/venom | `[cite: 05 Finding 22; 05 Finding 23; 02 §3]` |

The **terrain-gated `max_simultaneous_attackers` (B3)** is the keystone intransitivity lever: it makes "where the fight happens" (open surface vs your own tunnels) decisive, so no single species dominates everywhere.

**Avoid foregone matchups:** the gate (B8/B9) prevents a strong species from instantly sniping; the chokepoint math prevents a flooder from auto-winning; venom resistance prevents a venom species from auto-winning. Tune the venom matrix entries and `max_simultaneous_attackers_*` so the **win matrix has no all-100%/all-0% rows**.

**How to measure — cross-species win matrix harness:**
- New harness (analogous to the tournament): for every ordered pair (A, B) in the roster, play `K` matches (≥50, side-swapped to cancel first-move/topology bias — same discipline as Ladder League `gate_mpe=50`), record A's winrate vs B.
- Output an N×N matrix. **Success = intransitivity present** (at least one 3-cycle A>B>C>A) and **no row is ≥95% or ≤5% across the board** (every species is viable somewhere).
- The *B. chinensis* vs *A. rudis* cell must reproduce the displacement asymmetry (B. chinensis winrate high, target ~60–90% per the displacement bench acceptance criterion). `[cite: 01 Finding 11 (96%); 01 invasion_displacement_bench acceptance]`

---

## Observation / brain implications

The HAC currently plays a symmetric board and never sees species. Cross-species needs the policy to **know who it is and who it's fighting**.

### Species-conditioned observation
- Extend the per-colony obs vector (`trainer/src/hierarchical/obs_to_tensors.rs`) with: **own species one-hot** + **opponent species one-hot** (roster size = 10 → 20 extra dims), plus a few continuous species summary stats (own/opp `worker_attack`, `worker_health`, `target_population`, `recruitment_scalar`, `sting_potency`, mean size). This lets one policy condition its strategy on the matchup. `[cite: design — Nelson & Mooney 2025 three-axis dominance means strategy must read the opponent's axis]`
- This is the cheaper path than per-matchup champions and matches the Ladder League's single-policy-vs-frozen-pool shape.

### Two roster strategies (pick at spec review)
1. **Species-conditioned single policy** (recommended): one HAC reads the species one-hots and plays any A-vs-B. Train via PFSP across a matchup distribution. Reuses Ladder League wholesale — the "frozen pool" becomes `{frozen brain} × {species assignment}` and the gate measures winrate across the *matchup* distribution, not just one board.
2. **Per-species champions:** train/keep a best brain per species; the win matrix is brain-vs-brain *and* species-vs-species entangled. More artifacts, clearer per-species story, but N² training. Defer unless single-policy underfits.

### Ladder League extension
- The Ladder League (`2026-06-21-ladder-league-design.md`) trains a best-response vs a frozen pool on a *fixed symmetric* board. Cross-species adds a **matchup axis**: a round samples (own species, opp species) pairs; keep-best and the gate are computed **per-matchup-distribution** (mean winrate across sampled matchups vs the frozen pool), with the same anti-drift "frozen-within-a-round" invariant.
- **MEMORY note:** the SOTA `mlp_weights_v1.json` brain saturates on solitaire (trained PvP-only) — cross-species training must run in the 2-colony arena (it does; `MatchEnv` is 2-colony), so this is fine, but the obs change means the existing frozen pool brains have a *different obs width* → either (a) pad old brains' species dims to zero (back-compat: "species-unaware" opponents) or (b) re-bench from scratch. Recommend (a): zero-pad legacy brains so the frozen pool stays usable.

---

## Testing

Headless, deterministic, no Bevy — per CLAUDE.md rule 7 and the existing sim test discipline.

**Unit (`antcolony-sim`):**
- `apply_colony` produces a `ColonySimConfig` whose `ant`/`colony`/`combat` are **bit-identical** to what `apply` puts in `SimConfig` (refactor guard).
- `new_two_colony_with_topology(config,...)` is byte-identical to `new_two_colony_cross_species` with `cfg_black==cfg_red` (back-compat guard).
- Venom matrix: ponerine-sting vs naive myrmicine > 1.0; same-clade == 1.0; `venom_resistance` clamps and reduces.
- Combat reads per-side config: a high-`worker_attack` colony 0 ant deals more than a low-attack colony 1 ant in a 1v1 fixture.
- `max_simultaneous_attackers`: in a tunnel cell, ≤2 attackers deal damage even when 10 are adjacent (Lanchester terrain gate).
- Queen-kill gate: with attacker:defender < gate ratio, queen is invulnerable (queen_health unchanged) even when an enemy is adjacent; once the ratio is met, `Usurping` begins.
- Queen-kill interrupt: a channeling attacker that dies/flees mid-channel resets progress (queen survives).
- Determinism: two `new_two_colony_cross_species(A,B,seed)` runs to N ticks produce byte-identical final state (extends the existing det_check discipline; cross-rayon-thread-count too — MEMORY `project_determinism`).

**Cross-species smoke (`tests/` integration):**
- *B. chinensis* (colony 0) vs *A. rudis* (colony 1) on the 2-colony arena, run to match end or tick cap; assert (a) it terminates, (b) asymmetric mortality favors *B. chinensis* (`combat_kills[0] > combat_kills[1]`), (c) no panic / no NaN. `[cite: 01 Finding 10 acceptance — "asymmetric mortality favoring B. chinensis"]`

**Balance / win-matrix harness (new binary, analogous to the tournament):**
- `crates/antcolony-trainer/src/bin/cross_species_matrix.rs` (or a sim example) + `scripts/run_cross_species_matrix.ps1`. N×N, ≥50 mpe side-swapped, writes the matrix + intransitivity report (3-cycles, per-row min/max). Gate for outreach: the *B. chinensis*/*A. rudis* cell in the documented displacement band.

**Regression:** existing sim + trainer suites stay green (single-colony and same-species 2-colony paths byte-unchanged).

---

## Success criteria

- **Wiring:** a match runs with colony 0 = species A, colony 1 = species B, each ant carrying its own species' attack/health/speed/recruitment; old constructors byte-identical (guards green).
- **Mechanic:** the queen-kill is gated (no t=0 snipe), channeled (drains over time), and interruptible (defenders can reset it) — proven by the gate/interrupt unit tests.
- **Biology fidelity:** the *B. chinensis* vs *A. rudis* smoke shows asymmetric mortality favoring *B. chinensis*; the win-matrix cell lands in the displacement band.
- **Balance:** the cross-species win matrix shows ≥1 intransitive 3-cycle and no all-win/all-lose row (every species viable somewhere).
- **Determinism:** cross-species matches are byte-deterministic cross-process and cross-thread-count.
- **Training:** a species-conditioned HAC trains in the cross-species arena and the Ladder League round loop runs over the matchup distribution without drift.

---

## Open questions for the human

1. **Roster strategy:** species-conditioned single policy (recommended, cheap, reuses Ladder League) vs per-species champions (clearer per-species story, N² cost)? `[design choice; affects obs + training budget]`
2. **Venom matrix granularity:** a coarse clade×clade matrix (4×4-ish, easy to balance) vs per-species `venom_resistance` only? How aggressive should the asymmetry be — the literature span is 330–684× `[cite: 02 §3]`, but that's unplayable; what's the in-game cap (proposed: 2.0× attack, 0.9 max resistance)?
3. **`max_simultaneous_attackers` defaults:** open=8 / tunnel=2 / entrance=1 — start there and tune from the win matrix, or set from the Lymbery numbers more literally (20-vs-200 ⇒ ~0.1 effective ratio in 10mm corridors)? `[cite: 02 §2]`
4. **Worker defection on queen-kill** (Phase 2 payoff): include in MVP (great flavor, but changes scoring — defected workers inflate the winner's count) or defer? `[cite: 05 Finding 9, 15]`
5. **Corpse-to-killer feedback fraction** for `predates_ants`: full corpse value to the killer (strong B. chinensis snowball, matches biology) or a fraction to keep matches from running away? `[cite: 01 Finding 10, 11]`
6. **`predates_ants` scope:** MVP only routes corpse-food to the killer, or also the full predation-displacement bench (Rodriguez-Cabal reproduction) in this spec? The bench is its own deliverable (`outreach-roadmap-design.md` Phase 2.1/3) — recommend keep separate, this spec just adds the field + combat hookup. `[cite: 01 Lever 1; 01 Finding 11]`
7. **Per-colony pheromone deposit:** confirm moving `deposit_food_trail`/`deposit_home_trail`/`deposit_alarm` scaling to per-colony is acceptable (it's biologically correct — deposit strength is the *depositor's* trait — but it touches the shared `PheromoneConfig`). Diffusion/evaporation stay global (one field).

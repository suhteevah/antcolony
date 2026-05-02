# Biology Fidelity Roadmap

**Goal:** Make antcolony the simulator that PhD myrmecologists can use to test species-specific theories, while staying playable as a game. Real biology is the differentiator vs every "ants in a sandbox" clone. The TOML schema must be extensible enough that contributors can add species via PRs.

**Authoring constraint.** Every species claim in TOML or sim code must be sourced. The 7 new docs in `docs/species/*.md` are the source of truth; this roadmap closes the gap between those docs and the running sim.

---

## Current State (2026-05-02)

- **Schema:** `Species` struct in `crates/antcolony-sim/src/species.rs`. Fields: `biology` (lifespan, founding, polymorphic, hibernation), `growth` (4 maturation timings, eggs/day, target pop, food/day, egg cost), `diet` (string lists), `combat` (2 attack/health pairs + aggression scalar), `appearance` (color, size, speed multiplier), `encyclopedia` (text only).
- **Difficulty enum:** `Beginner | Intermediate | Advanced | Expert` — defined, parsed, surfaced in picker UI tagline. **Not yet** gating mechanics, AI difficulty, or tech-unlock requirements.
- **7 species shipped:** lasius_niger, camponotus_pennsylvanicus, formica_rufa, pogonomyrmex_occidentalis, tetramorium_immigrans, tapinoma_sessile, aphaenogaster_rudis. All currently labelled `beginner` or `intermediate` regardless of how exotic their biology is.
- **PhD docs:** `docs/species/{id}.md` — written 2026-05-02, ~1,500–2,400 words each, every quantitative claim sourced.

---

## Difficulty Bucketing (proposed, based on PhD docs)

How much novel sim machinery each species needs *before* it can be played truthfully (vs the generic "Lasius-shaped" sim it gets today):

| Species | Proposed Difficulty | Novel mechanics required | Currently lives in sim as |
|---|---|---|---|
| **Lasius niger** | **Beginner** | None. Substrate=loam, mass recruit, claustral, monomorphic, formic. The reference baseline. | ✅ Authentic. |
| **Tetramorium immigrans** | **Beginner** | Ritualised territorial battles (monoamine clock, ~3-min decay) — but degrades gracefully to plain combat. | ⚠️ Plays as small Lasius. |
| **Tapinoma sessile** | **Intermediate** | Polydomy + nest relocation; population-structure toggle (forest 100 / urban 100k+). | ⚠️ Plays as small Lasius. |
| **Aphaenogaster rudis** | **Intermediate** | Myrmecochory — seeds-as-food with elaiosome reward + viable-seed midden (ecological loop). | ⚠️ Plays as small Lasius. |
| **Camponotus pennsylvanicus** | **Advanced** | Wood substrate (defining), worker polymorphism size buckets, tandem (not mass) recruitment, nocturnal. | ❌ Plays wrong — mass recruit on loam is the opposite of this species. |
| **Pogonomyrmex occidentalis** | **Advanced** | Sand substrate, granivore food class, southeast-oriented surface disc, sting damage curve. | ❌ Plays wrong — no granivory means it starves on a non-feeder map. |
| **Formica rufa** | **Expert** | Parasitic founding (host: Formica fusca — needs a second species in-world), thatch-mound construction, polygyne supercolony, aphid honeydew columns. | ❌ Currently flagged `parasitic` but founding falls back to claustral; thatch absent. |

**If all 7 feel too hard for the next milestone**, three "easier than Lasius" candidates ship cleanly with the current schema *plus* one or two of the Phase A additions:

- **Myrmica rubra** — Beginner. Similar to Lasius but stings; tests the sting axis without exotic substrate. Polygyne-tolerant, gives us a second monogyne→polygyne data point.
- **Prenolepis imparis** ("winter ant") — Beginner-Intermediate. Active at temperatures other ants are not. Tests the diapause-threshold axis with an inversion (cold-active species).
- **Solenopsis molesta** ("thief ant") — Beginner. Tiny, simple, lestobiotic raids on other colonies. Tests the inter-species interaction axis cheaply.

These are **not** required — listed only as a fallback if Phase B reveals the existing 7 are blocking.

---

## TOML Numerics Audit (Phase 0 — lossless, no schema change)

Per the 7 docs, the following TOML numbers either disagree with sourced literature or sit at the upper bound with no `# game-pacing` justification. Fix with TOML edits + inline comments only — no code changes.

| Species | Field | Current | Doc-recommended | Action |
|---|---|---|---|---|
| lasius_niger | `queen_lifespan_years` | 28.0 | 28.7 (Appel record) — keep. | Add `# Hermann Appel record, Kutter & Stumper 1969 / Keller & Genoud 1997` comment |
| lasius_niger | `worker_lifespan_months` | 24.0 | Field workers ~1–2 mo; nest workers up to 12 mo; 24 is gameplay-pacing | Document as `# game-pacing — natural ~1-12mo, scaled up so adults persist visibly` |
| camponotus_pennsylvanicus | `target_population` | 8000 | 2,000–15,000 (Hansen & Klotz). Keep. | Add citation comment |
| camponotus_pennsylvanicus | `min_diapause_days` | (default 60?) | 120 (cold-temperate) | Set explicitly to 120 |
| formica_rufa | `queen_lifespan_years` | 18.0 | Literature centroid 8–15, max ~25 | Document as upper-bound choice |
| formica_rufa | `target_population` | 300000 | Within range (100k–500k+) | Add citation |
| pogonomyrmex_occidentalis | `queen_lifespan_years` | 16.0 | Cole & Wiernasz 2025: 5–40, mean ~13. Keep. | Document |
| pogonomyrmex_occidentalis | difficulty | (likely beginner/intermediate) | **advanced** per bucketing above | Bump |
| aphaenogaster_rudis | `target_population` | 800 | Lubertazzi mean 266–613, max ~2k. Upper-end gameplay choice. | Document |
| tetramorium_immigrans | difficulty | (current?) | **beginner** | Verify |
| tapinoma_sessile | difficulty | (current?) | **intermediate** (polydomy needs Phase B) | Bump |
| ALL | (no field) | — | — | Audit against doc, add `# source: <citation>` comments next to every numeric |

**Estimated effort:** 1–2 hours, pure TOML editing + comments. Zero risk to running sim.

---

## Schema Extensions (Phase A — additive, back-compat)

All fields default to current behavior so existing TOMLs keep working. New optional sections in `Species`:

```toml
[behavior]
recruitment = "mass" | "tandem_run" | "group" | "individual"   # default "mass"
foraging_mode = ["generalist", "honeydew", "granivore", "myrmecochore", "predator", "scavenger"]
diel_activity = "diurnal" | "nocturnal" | "crepuscular" | "cathemeral"   # default diurnal
trail_half_life_seconds = 2820   # Beckers/Deneubourg Lasius default; species-specific where measured

[colony_structure]
queen_count = "monogyne" | "facultatively_polygyne" | "obligate_polygyne"
polydomous = false               # multi-nest with shared workers
relocation_tendency = 0.0        # 0..1, probability per month nest moves
supercolony_capable = false      # if true, exposes a population-scale toggle in editor
budding_reproduction = false     # founds new colonies by fission, not nuptial flight only

[substrate]
preferred = ["loam", "sand", "wood", "leaf_litter", "rock_crevice", "thatch"]
incompatible = []                # species refuses to nest in these
dig_speed_multiplier = 1.0       # tiles/tick scaling vs baseline Lasius
mound_construction = "none" | "kickout" | "thatch" | "crater_disc" | "lid_dome"

[combat_extended]
weapon = "mandible" | "sting" | "formic_spray" | "chemical"
sting_potency = 0.0              # 0..5, Schmidt-style; affects damage vs predators
ranged_attack = false            # formic spray etc — gives ranged combat option
soldier_size_categories = []     # ["minor", "media", "major"] for polymorphic species
major_attack_multiplier = 2.5    # damage scalar for the largest caste
context_aggression = false       # Boulay 2024 — aggression varies by intruder identity

[diet_extended]
seed_dispersal = false           # myrmecochory — colony processes elaiosome seeds
honeydew_dependent = false       # if true, must have aphid source nearby to thrive
host_species_required = []       # for parasitic founding — list of host species ids

[ecological_role]
keystone = false                 # surface in encyclopedia + scoring
invasive_status = "native" | "introduced" | "invasive_pest"
displaces = []                   # species ids this one outcompetes
displaced_by = []                # species ids that outcompete this one
```

**Migration:** every new field is `#[serde(default)]`. Existing TOMLs parse unchanged. Difficulty bumps to Advanced/Expert when any of these fields require a new gameplay system to play truthfully.

---

## Sim Hooks (Phase B — flag-driven, mostly branches in existing systems)

Each schema field maps to a system change. Order is roughly cheapest → most invasive.

1. **`recruitment` → trail-deposit strength.** `mass` = current behavior, `tandem_run` = drop trail strength to ~10%, queue followers, `individual` = no deposit. One branch in `deposit_system`.
2. **`diel_activity` → tick-gated foraging.** Nocturnal species suppress `Exploring`/`FollowingTrail` during sim "day". One filter in `decision_system`.
3. **`trail_half_life_seconds` → per-species evaporation override.** Already supported by config; just plumb the per-species number through `Species::apply`.
4. **`combat_extended.weapon` + `sting_potency` → damage curve.** Stinger species do extra damage to predators in `hazards.rs`; spray species get ranged hit in `combat.rs`.
5. **`soldier_size_categories` + `major_attack_multiplier` → polymorphism size buckets.** Replaces the binary `polymorphic: bool` with a real enum. Existing combat reads "soldier_attack" — gain a "major" bucket at 2.5×.
6. **`substrate.preferred` + `incompatible` → editor placement gate.** Editor refuses to place a Camponotus colony on Sand. Sim treats wrong-substrate as a steady debuff.
7. **`substrate.dig_speed_multiplier` → already proposed in `digging-design.md`; wire it.** Closes the Camponotus-in-wood gap halfway without needing real wood substrate.
8. **`substrate.mound_construction` → render hook.** `kickout` = current Lasius behavior, `thatch` = Formica dome (procedural needle texture), `crater_disc` = Pogonomyrmex cleared-vegetation disc oriented SE.
9. **`colony_structure.polydomous` + `relocation_tendency` → relocation tick.** New `relocation_system` runs once per sim day; if rolled, designates a new nest entrance and gradually shifts brood/queen.
10. **`colony_structure.supercolony_capable` → editor toggle.** Player picks "Forest" (small) vs "Urban" (supercolony) at colony placement; latter spawns multiple cooperating queens with `queen_count = "obligate_polygyne"` flag asserted.
11. **`diet_extended.seed_dispersal` → new `Food::Seed` variant.** Aphaenogaster carries seeds back; nest emits `Food::Seedling` after N ticks in midden (visual + nothing-else for now; could grow into real plant later).
12. **`diet_extended.honeydew_dependent` → starvation modifier.** Without honeydew source nearby (aphid colony entity, separate Phase C), `food_inflow_recent` floor is lower → slower growth.
13. **`diet_extended.host_species_required` → Formica founding loop.** If list non-empty, founding queen seeks an existing colony of those species. Gates Formica rufa on Formica fusca being available — likely add fusca as a non-player-controllable host species.
14. **`ecological_role.invasive_status` → AI behavior bias in main game.** Invasive species AI more aggressive at expansion + displacement.

Every hook is degraded gracefully: missing flag = today's behavior. Difficulty enum gates which hooks run — Beginner mode can skip nocturnality, polydomy, host-species, etc. so the player isn't fighting biology they didn't sign up for.

---

## New Mechanics (Phase C — heavier work)

Things that need real new code, not just branches:

- **Substrate types** (`Sand`, `Wood`, `LeafLitter`, `Loam`, `RockCrevice`, `Thatch`) as a `WorldGrid` field. Editor supports placing patches. Dig speed + chamber stability vary. Already roadmapped in `docs/digging-design.md` — formalize.
- **Aphid colonies as world entities.** Spawn on plants in editor, produce honeydew over time, can be defended/raided. Required for Lasius/Formica honeydew-dependent realism.
- **Thatch mound construction** as a stigmergic build process — Formica workers carry "thatch material" tiles outward from the central pile, building dome geometry. Render as a procedural dome; provides own microclimate (warmer than ambient).
- **Supercolony budding reproduction.** New colony founded by fission of existing one rather than nuptial flight. Requires a second `Colony` entity sharing pheromone identity for some grace period.
- **Granivory food class.** `Food::Seed` distinct from `Food::Sugar`/`Food::Protein`; some species refuse non-seeds, others ignore seeds. Pogonomyrmex needs this to play truthfully on a non-feeder map.
- **Predator interactions per species.** Antlions vs small species different from antlions vs Pogonomyrmex (sting deters). Predator AI consults victim's `combat_extended`.
- **Host-species machinery for parasitic founding.** Formica rufa founding queen seeks a Formica fusca colony, kills/displaces fusca queen, fusca workers raise rufa brood until rufa workers replace them. Multi-tick cinematic worth watching.

**Phase C is the "PhD-grade" jump.** Without it, Camponotus/Pogonomyrmex/Formica/Aphaenogaster all play as same-flavor Lasius variants and the differentiator collapses.

---

## Community Contribution Path (Phase D)

The whole point of TOML extensibility is "Dr. X studies *Atta cephalotes* — they should be able to PR a TOML and have leafcutter ants playable."

- **Schema versioning.** Add `schema_version = 2` field; reject TOMLs with unknown versions. Every Phase A field bump creates schema 3, 4, etc.
- **JSON Schema export.** Generate `assets/species/schema.json` from the `Species` struct via `schemars`. Editors (VS Code, Zed) get autocomplete + validation for free.
- **Validation CLI.** `cargo run --bin validate-species -- assets/species/atta_cephalotes.toml` — checks schema, runs species through a 1-year headless sim, reports population trajectory and starvation events. CI gate for new species PRs.
- **CONTRIBUTING-SPECIES.md.** Per-species template: required citations, biology-doc structure, "what hooks does this species need" checklist, test colony health bench output to attach to PR.
- **Reference TOMLs as living examples.** Each shipped species TOML carries inline comments explaining the *why* behind each non-default value — newcomers learn by reading.
- **Species pack plugins (longer term).** `assets/species/` is a default; `--species-dir` CLI flag loads from anywhere. A user could ship "Brazilian Atta Pack" as a folder of TOMLs + sprites.

---

## Calibration & CI (Phase E — keeps biology honest forever)

- **Per-species smoke harness.** `cargo run --example colony_diag -- --species <id> --years 25 --scale timelapse`. Outputs CSV: year, adult_pop, food_returned, food_inflow, queen_age, brood_counts. PR diff includes before/after for each affected species.
- **Health score.** Composite: population stability, food-return ratio, time-to-first-major (polymorphic), survival to year N. Per-species expected ranges from the docs.
- **Regression bench in CI.** GitHub Actions banned per global rules — run locally via `scripts/bench_species.ps1` before PR. Pre-commit hook offered.
- **Doc/TOML/sim three-way diff.** Script that asserts every numeric in a species TOML has a matching citation in `docs/species/{id}.md` and that no sim code path overrides species values silently. Fail PR if drift detected.

---

## Phasing Recommendation

| Phase | Scope | Risk | Time | Value |
|---|---|---|---|---|
| 0 | TOML number audit + citation comments | None — pure TOML edits | 1–2 h | Honesty |
| A | Schema extensions (additive, defaulted) | Low — back-compat by default | 4–6 h | Unblocks B |
| B | Flag-driven sim hooks (1–14 above) | Medium — touches existing systems | 2–4 days | Closes the wrong-flavor-Lasius gap for 5/7 species |
| C | New mechanics (substrate, aphids, thatch, supercolony, granivory, hosts) | High — new systems, render work | 1–2 weeks | Delivers Camponotus / Pogonomyrmex / Formica truthful |
| D | Schema versioning, JSON Schema, validation CLI, CONTRIBUTING | Low–medium | 2–3 days | Opens contributor PRs |
| E | Per-species health bench + regression script | Low | 1 day | Keeps biology from drifting silently |

**Recommended order:** 0 → A → B → E (so we can measure) → C → D.

---

## What This Buys Us

1. **Differentiator.** "PhD-grade species fidelity, every claim cited, contributor-extensible" is a positioning no SimAnt clone has touched.
2. **Sustained content velocity.** A working contribution path means species count grows independently of core dev time.
3. **Player-facing depth.** Difficulty enum becomes meaningful: Beginner = play with safety rails, Expert = play the species' real biology with no shortcuts.
4. **Educational use.** Once Phase E is in, a researcher can bench a species variant ("what if Aphaenogaster trail decay was 2x faster?") and the harness produces comparable output.

---

## Open Questions for Matt

1. **Difficulty as gameplay gate.** Should Beginner mode actively *hide* hard-biology mechanics (e.g., disable parasitic founding for Formica, fall back to claustral) or just warn? Current proposal: Beginner = mechanics simplified, Expert = mechanics full.
2. **Host species for Formica.** Add Formica fusca as an 8th species TOML (non-player-controllable, only spawns as host)? Or skip parasitic founding for now and flag Formica rufa as "Beginner-mode-claustral, Expert-mode-parasitic"?
3. **Substrate scope.** Phase C calls for 6 substrate types. Do we ship all at once or start with Loam + Wood + Sand and add LeafLitter / RockCrevice / Thatch later?
4. **The Lasius extras the docs flagged** (Beckers/Deneubourg 47-min trail, Boulay context-dependent aggression, Khuong stigmergic chambers) — are those in scope for Phase B or do they go in a separate "Lasius reference-implementation polish" pass?
5. **Easy fallback species.** If Phase B reveals the existing 7 are too biology-heavy, do we add Myrmica/Prenolepis/Solenopsis as Beginner anchors? Or keep the seven and accept that Beginner = Lasius + Tetramorium only?

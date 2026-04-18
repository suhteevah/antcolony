# Ant Biology Research Log

**Purpose.** This is the canonical log of real-ant biology facts used to ground the simulation. Every time we learn something new about real ant behavior that could inform (or correct) sim mechanics, it goes here. Sources cited inline. Entries are append-only — if a claim is superseded, mark the old one `[SUPERSEDED ← see dd-mmm-yyyy entry]` and add the new one at the bottom.

**Audience.** Future contributors (human + agent). When picking how a sim mechanic should behave, read this file first. When adding a species, check this file for species-specific notes.

**Related artifacts.**
- `assets/species/*.toml` — species-specific numbers (lifespan, egg rate, diet). When a fact here is species-specific, the species TOML should carry the concrete number and this file should explain the *mechanism*.
- `crates/antcolony-render/src/encyclopedia.rs` — in-game encyclopedia panel. Fun facts from the species TOML are surfaced to the player here.
- `crates/antcolony-sim/src/simulation.rs` — sim behaviors that should mirror the mechanics described here.

---

## Colony-Level Food Regulation (logged 2026-04-18)

### Endogenous reserves — queens maintain a lay floor from their own body

Even under zero inflow, a queen lays at a reduced but nonzero rate. Founding queens run entirely off metabolized wing muscles and stored fat for weeks or months; established queens draw on stored lipids and glycogen during short shortages. There is no "zero-food → zero-eggs" cliff in nature — the queen slows down, but she keeps going as long as she's alive.

**Sim implication.** The food-inflow throttle (below) must have a floor, representing the queen's endogenous reserves. Current sim uses `0.2` (20% of max rate) as the floor — queens always lay at least one-fifth of their theoretical maximum while alive and with any `food_stored > 0`.

**Source.** [AntKeepers — Lifecycle of an ant colony](https://antkeepers.com/pages/lifecycle-of-an-ant-colony) (founding queens); queen-lifespan evidence in [Keller & Genoud 1997, Nature — monogyne queen lifespan](https://www.nature.com/articles/38894) (cited throughout ant-keeping literature).

### Queen egg-laying is throttled by recent food intake, not static reserve

Real queens cannot "lay on empty." Egg production requires continuous vitellogenin synthesis, which requires a steady supply of amino acids (protein). When worker-delivered protein drops, vitellogenin output drops, and the queen's lay rate falls automatically — it's a physiological constraint, not a decision.

**Mechanism.** Workers carry protein to the queen via trophallaxis. Queen metabolizes it into vitellogenin → yolk → eggs. Throughput through this pipeline is the real lay-rate cap. A queen with abundant protein can lay at the species' theoretical maximum; a queen whose workers aren't delivering protein makes fewer eggs the same day.

**Sim implication.** Queen lay rate should be `base_rate × clamp(food_inflow_recent / consumption_rate, 0, 1)`, not a check against `food_stored >= egg_cost`. Track a rolling window of food returned (e.g., last 100 ticks).

**Source.** [Mankin et al. 2022, "Effect of queen number on colony-level nutrient regulation"](https://www.sciencedirect.com/science/article/abs/pii/S0022191022000117) — workers scale protein/carb collection to queen demand; queens scale output to incoming protein.

### Survival cannibalism of brood is normal, not pathological

Under food stress, workers consume the colony's own eggs and young larvae to feed themselves and the queen. This is routine during winter dormancy, drought, or short-term food shortages — not a sign the colony is failing. Nutrient is recycled, not lost.

**Mechanism.** Eggs and early-instar larvae are high-protein, low-mass. Workers cannibalize them preferentially over older brood (which has already had more nutrient invested). The recovered protein keeps adult workers alive long enough to find fresh food.

**Sim implication.** When `food_stored` drops below a critical threshold (scales with colony size), workers should consume eggs → young larvae → older larvae in that order, each conversion recovering ~70-90% of the original food invested. This **replaces** most worker starvation — workers don't die while brood is available to eat.

**Source.** Brood cannibalism under stress is broadly documented; see [Direct evidence for cannibalistic necrophagy as nitrogen recycling in ants (Czaczkes et al., PMC)](https://pmc.ncbi.nlm.nih.gov/articles/PMC12522066/) and the extreme-case [Targeted cannibalism of virgin queens in fire ants](https://www.cell.com/current-biology/fulltext/S0960-9822(25)01391-0).

### Queen filial cannibalism (disease response) is a nutrient recycler

Founding *Lasius niger* queens cannibalized 92% of larvae infected with a lethal fungus (*Metarhizium brunneum*), leaving no remains, vs only 6% of healthy control larvae. The queens that cannibalized **laid 55% more eggs** than non-cannibalizing controls — the recouped nutrients went straight back into reproduction.

**Sim implication.** The disease-detection angle is a Phase 6+ hazard feature (not yet implemented). But the underlying principle — queens can recycle nutrients from brood into new eggs — is worth mirroring in the starvation response above. Trophic eggs (below) are a non-disease version of the same recycling pathway.

**Source.** [Pull et al. 2024, Current Biology — ant queens cannibalise infected brood to contain disease spread and recycle nutrients](https://www.cell.com/current-biology/fulltext/S0960-9822(24)01001-7).

### Trophic eggs — queens produce nutritive non-viable eggs as colony food

Many ant species produce "trophic" eggs — eggs that cannot develop into ants, laid specifically as food for larvae and workers. The queen is therefore **both a reproduction engine and a small food source**. In founding queens whose wing muscles are metabolizing down, trophic eggs are how stored body reserves get into the larvae.

**Mechanism.** Trophic eggs are yolky but lack certain reproductive requirements (often the queen intentionally withholds fertilization; details vary by species). Workers and larvae eat them whole.

**Sim implication.** Queen should produce a low background rate of trophic eggs (converts stored food + a fixed metabolic output into small food packets that workers can deliver to the brood/colony pile). Gated behind the queen having any food at all. Especially important in founding colonies where no workers are foraging yet.

**Gameplay hook (Matt's PvP design):** trophic-egg production should be a **tech-tree unlock** in versus mode, not a freebie. See `Tech Unlocks for PvP` below.

**Source.** [AntKeepers — Lifecycle of an ant colony](https://antkeepers.com/pages/lifecycle-of-an-ant-colony); [AntWiki — The Ant Life Cycle](https://www.antwiki.org/wiki/The_Ant_Life_Cycle).

### Queen pheromones regulate worker foraging via negative feedback

Queen-produced cuticular hydrocarbons (notably *3-MeC31* in several studied species) signal reproductive status to workers. Workers respond by scaling their protein/carbohydrate collection rate to the queen's output. In polygyne colonies, multiple queens' pheromones also **mutually inhibit** each other's fecundity — the more queens present, the less each one lays. This is why colony productivity plateaus rather than scaling linearly with queen count.

**Sim implication.** A pheromone-inhibition sub-model is overkill for current sim scope, but the *behavioral consequence* — that workers dynamically re-allocate forage effort based on queen demand — is already roughly mirrored by our `behavior_weights` system. Making forage weight auto-scale up when food_stored drops is a cheap imitation of this feedback loop.

**Source.** [Smith et al., Pheromonal regulation of reproductive division of labor in social insects (Frontiers in Cell & Dev Bio)](https://www.frontiersin.org/journals/cell-and-developmental-biology/articles/10.3389/fcell.2020.00837/full); [Holman et al., "Are queen ants inhibited by their own pheromone?"](https://academic.oup.com/beheco/article/24/2/380/249316).

### Claustral vs semi-claustral founding

Fully-claustral founding queens (e.g. *Lasius niger*, most *Camponotus*) seal themselves in a founding chamber and raise their first workers entirely from metabolized body reserves (primarily wing muscles). They eat nothing until the first nanitics (first-generation workers) emerge and start foraging.

Semi-claustral queens (e.g. some *Pogonomyrmex*) leave the founding chamber briefly to forage during the founding period.

**Sim implication.** Founding-stage queens are currently modeled as a queen sitting on a nest entrance with an already-spawned initial worker cohort. True founding-stage simulation (queen alone, no workers, slow ramp-up) is a future content add. For now: species TOMLs carry a `founding = "claustral" | "semi_claustral" | "social_parasite"` tag that future mechanics can branch on.

**Source.** [AntKeepers — Lifecycle of an ant colony](https://antkeepers.com/pages/lifecycle-of-an-ant-colony).

---

## Tech Unlocks for PvP (design note, 2026-04-18)

In a planned versus mode (two human players, two colonies), biological mechanics that would otherwise trivialize the match or grant one side a snowball advantage should be gated behind tech unlocks earned during play. Rough shortlist of what to gate:

- **Trophic eggs** — free-ish food from the queen. Gate behind a "Reproductive Investment" research node. Until unlocked, queen only lays fertile eggs.
- **Brood cannibalism under starvation** — lets the colony survive food shortages. Gate behind "Nutrient Recycling" research. Until unlocked, starvation just kills workers.
- **Nuptial flight / daughter colony founding** — already exists (K5). In PvP, gate daughter-colony placement behind "Territorial Expansion" research so early-game can't snowball via free new colonies.
- **Caste specializations** — soldier/major production could be gated behind "Polymorphism" research for monomorphic species, or granted by default for species that are already polymorphic in the TOML.
- **Alarm-pheromone steering** — soldiers auto-converging on alarm should be gated behind "Pheromone Communication" research in PvP so early ants are simpler agents.

**Research currency.** Either food-over-time (colony ticks × food_returned) or a dedicated "pheromone study" resource. TBD when PvP is scoped.

**Keeper-mode default.** All tech unlocks should be **on by default** in single-player Keeper mode. PvP is the only place the tech gates apply. Species-specific biology (hibernation required, polymorphism, etc.) stays unaffected by tech unlocks — it's species identity, not a research outcome.

---

## How to Use This File

1. **Reading.** Before implementing or modifying a sim mechanic that touches ant behavior, grep this file for relevant terms.
2. **Writing.** When you pick up a new fact (research paper, expert forum, keeper source), append it to the appropriate section (or add a new section). Always include a cited source with a link. Use the same format as existing entries: *what it is → mechanism → sim implication → source*.
3. **Cross-referencing.** If a fact becomes species-specific, add a pointer from the species TOML's `encyclopedia.fun_facts` to this file. If a fact becomes a gameplay mechanic, reference this file in the relevant sim code comment.

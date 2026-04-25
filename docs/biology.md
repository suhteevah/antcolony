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

## Diapause Biology (logged 2026-04-25)

### Metabolic depression in hibernation
Hibernating insects don't just stop foraging — their metabolic rate drops to ~5-10% of active levels. Respiration, heart rate, and energy consumption all crash. A diapausing colony's food needs over winter are roughly 1/10 to 1/20 of summer needs. The reserves accumulated in late autumn (visible as engorged crops and full food chambers) are calibrated against this dramatically reduced rate.

**Sim implication.** `colony_economy_tick` multiplies per-tick consumption by `DIAPAUSE_METABOLIC_DEPRESSION = 0.10` when `in_diapause` is true. Without this scaling, colonies that grow before their first winter consume their stored food at summer rate, exhaust brood via cannibalism, and collapse — observed empirically in a 6mo Lasius Timelapse smoke that grew 21→918 ants and then crashed in 5000 ticks once winter hit.

**Source.** [Hahn, D.A. & Denlinger, D.L. (2007). Meeting the energetic demands of insect diapause: nutrient storage and utilization. *Journal of Insect Physiology* 53: 760-773.](https://www.sciencedirect.com/science/article/abs/pii/S0022191007000753) — definitive review of insect diapause metabolic depression. Ant-specific work: Lighton & Bartholomew (1988) on Pogonomyrmex respiratory physiology measured an ~8× drop from active to resting metabolic rate in clustered overwintering harvesters.

### Autumn retreat — ants go inside before winter
Ants don't get caught outside in the cold. As ambient temperature drops in autumn (typically when daily mean falls below ~12-15°C, which corresponds to local cold-snap onset for temperate species), workers stop foraging and return to the nest. They cluster in the deepest chambers — often the queen chamber or a dedicated hibernation chamber on the underground side of the formicarium — and remain there until spring warming. Foragers caught outside when a sudden frost hits do die, but in normal seasonal cooling all workers shelter before the freezing temperatures arrive.

**Sim implication.** When an ant transitions into `AntState::Diapause`, it should snap to its colony's nest entrance position (representing autumn retreat). Real biology has this happening over several days; the sim teleports it for simplicity. Matches the visible outcome: all ants inside, none stuck on surface tiles. Without this, surface foragers freeze in place wherever they were when ambient dropped below `cold_threshold`, which looked weird AND meant they didn't drop their food at the nest before becoming inactive.

**Source.** [Heinze, J. & Hölldobler, B. (1994). Ants in the cold. *Memorabilia Zoologica* 48: 99-108.](https://www.researchgate.net/publication/247880033_Ants_in_the_cold) — covers cold-tolerance and seasonal-retreat behavior across temperate ants. Keeper observations corroborate: every formicarium-keeping forum confirms workers retreat to nest interior in autumn ahead of the planned hibernation period.

---

## Excavation & Nest Architecture (logged 2026-04-25)

Notes for the dig-pipeline implementation. Real ant excavation is structurally different from "ant standing on Solid tile, tile becomes Empty." Capturing the actual workflow is what gives an ant farm its unmistakable look.

### Soil pellets, not grains
Workers don't transport substrate one grain at a time. They use mandibles + saliva + (in some species) prothoracic legs to roll a small **pellet** of cohesive soil — typically several to dozens of grains — and carry the pellet whole. The pellet is roughly the size of the worker's head. Lasius niger pellets are ~0.3-0.7mm; Pogonomyrmex (large workers, sandy substrate) can carry pellets up to 1.5mm.

**Sim implication.** When a `Solid` tile is excavated, the digger gains a "carrying soil pellet" state — visually a small dark blob in the mandibles (we already have a food-carry indicator pattern; mirror it for soil). The pellet exists as a transferable load the ant must dispose of, not as an instant disappearance.

**Source.** [Sudd, J.H. (1969). The excavation of soil by ants. *Zeitschrift für Tierpsychologie* 26: 257-276.](https://onlinelibrary.wiley.com/doi/10.1111/j.1439-0310.1969.tb01952.x) — foundational study of pellet rolling and carry behavior in Formica & Lasius. Also [Tschinkel, W.R. (2004). The nest architecture of the Florida harvester ant, *Pogonomyrmex badius*.](https://www.jstor.org/stable/25086323) for pellet sizes in granivores.

### Kickout mound — the diagnostic visual
When a pellet-carrying worker reaches the nest entrance, she does NOT just drop it on the threshold. Workers drop pellets a body-length or two **outside** the entrance, building a characteristic **donut-shaped mound** around the hole. The mound is the single most recognizable visual feature of an active ant nest — both in nature and in glass-front formicarium setups. In substrate ant farms (the iconic between-the-glass type), an equivalent **dump pile** accumulates against the inner wall closest to the entrance.

**Sim implication.** Excavated pellets must accumulate in a visible pile — either at the surface entrance cell (outdoor mound logic) or along the underground module's "exit" wall. A `Terrain::SoilPile(intensity)` variant or a per-cell counter incremented by dump events gives the visual feedback. Without this the player doesn't see *why* digging matters; with it, the mound growing over time becomes a satisfying progress indicator.

**Source.** [Tschinkel, W.R. (2003). Subterranean ant nests: trace fossils past and future?](https://www.sciencedirect.com/science/article/abs/pii/S0031018202005583) — extensive treatment of nest entrance architecture and kickout deposition. Also keeper visual reference: virtually any Uncle Milton sand-between-glass ant farm builds a visible dump pile within 24-48 hours of activity.

### Chain-gang pipeline
For tunnels longer than a few body lengths, excavation is **not solo work**. Digging is a pipeline: a frontline digger loosens material at the dig face, intermediate carriers shuttle pellets back along the tunnel, and entrance workers (often older / smaller individuals) take pellets the final distance to the kickout. Workers self-organize into these roles based on local task demand — there's no central dispatcher.

**Sim implication.** A single ant doing the entire dig→carry→dump cycle works for short tunnels, but as the underground expands into long passages, the per-ant round trip becomes the bottleneck. The natural emergent behavior is for multiple workers to cluster at the dig face and chain pellets back. Easiest sim approximation: when an ant in `AntState::Returning` (carrying soil) encounters another worker in `Idle`/`Exploring`, hand off the pellet — a trophallaxis-style transfer. Skip in MVP; add when tunnels get long enough to see the queue.

**Source.** [Buhl, J., Gautrais, J., Solé, R.V., Kuntz, P., Valverde, S., Deneubourg, J.L., & Theraulaz, G. (2004). Efficiency and robustness in ant networks of galleries. *European Physical Journal B* 42: 123-129.](https://link.springer.com/article/10.1140/epjb/e2004-00366-7) — formal analysis of worker pipelining in *Messor sancta*. Less formal but vivid: keeper Forum threads on Camponotus build-outs frequently note "two diggers up front, three carriers behind, one or two at the entrance" patterns visible in glass setups.

### CO₂ and humidity gradients drive dig direction
Where ants dig is not random. Multiple species are demonstrably attracted to **higher CO₂** (a proxy for "deeper / less ventilated / where the colony already is") and **higher humidity** when choosing a dig face. They are simultaneously repelled by exposed surface conditions. This produces nests that branch laterally at each chamber level and trend downward overall — the iconic vertical-shaft-plus-side-chambers architecture.

**Sim implication.** Big design opportunity: the existing `PheromoneGrid` already supports per-cell scalar layers (Alarm, FoodTrail, HomeTrail, ColonyScent). Add a **`Dig` priority field** that ants deposit on Solid tiles adjacent to chambers/tunnels with high colony-scent or near brood. Diggers select the cell with the highest dig score. Result: nest expansion is biased toward existing population density — exactly the right look. Add a `Humidity` scalar field if we want CO₂/water gradients later (ties into K3 thermoregulation).

**Source.** [Kleineidam, C. & Roces, F. (2000). Carbon dioxide concentrations and nest ventilation in nests of the leaf-cutting ant *Atta vollenweideri*. *Insectes Sociaux* 47: 241-248.](https://link.springer.com/article/10.1007/PL00001710). Also [Bollazzi, M. & Roces, F. (2002). Thermal preference for fungus culturing and brood location in *Acromyrmex ambiguus*.](https://link.springer.com/article/10.1007/s00040-002-8302-2) for humidity-driven chamber siting.

### Chamber siting is functional, not random
Ants don't excavate generically — different chambers serve different functions and get sited at predictable depths and humidities. **Brood nurseries** sit at the warmest, most humid level (typically near the surface in summer, deeper in winter). **Food storage / granaries** sit lower, drier, cooler. **Waste / midden** chambers sit OFF the main spine, away from brood, often near the lowest point. **Queen chamber** is the deepest, most defensible point. Workers actively move brood between chambers across seasons to track the optimal microclimate.

**Sim implication.** P5's `attach_underground` already pre-carves QueenChamber, BroodNursery, FoodStorage, and Waste chambers — but workers don't choose to dig new chambers, and don't move brood between chambers. Phase B of dig: when colony population exceeds chamber capacity, designate a "dig new chamber" target with appropriate type based on what the colony lacks. When seasonal temperature shifts (K3 climate), workers shuttle brood from cold chambers to warm ones. This is the layer that turns the underground from "static carved environment" into "living architecture."

**Source.** [Tschinkel, W.R. (2015). The architecture of subterranean ant nests: beauty and mystery underfoot. *Journal of Bioeconomics* 17: 271-291.](https://link.springer.com/article/10.1007/s10818-015-9203-6) — definitive review covering chamber function, depth-stratification, and seasonal brood relocation.

### Excavation rate is slow
A small *Lasius niger* founding colony excavates roughly **1-2 cm³ of substrate per day** under good conditions. A mature *Camponotus* colony in soft wood can excavate galleries totaling several liters but over many months. In sim terms, even compressed 60× time scale, this should be visible-but-not-frantic: a single digger should take dozens of ticks to remove a single tile, with the rate tunable per species via biology TOMLs (large-bodied species dig faster per worker; cohesive substrate slows everyone down).

**Sim implication.** The current `dig_tick` excavates one neighbor per ant per tick (instantaneous from a player's perspective at 30Hz). That's too fast and breaks the "watch them work" appeal. Introduce a **dig progress counter** per (ant, target_cell) pair: each tick the digger spends adjacent to the target adds N progress; when progress crosses a threshold (e.g. 60 ticks ≈ 2 sim seconds at default scale), the tile flips. Threshold scales with `worker_size_mm` and a per-species `dig_speed_multiplier` to be added to `appearance` block.

**Source.** [Sudd, J.H. (1972). The absence of social enhancement of digging in pairs of ants (Formica lemani). *Animal Behaviour* 20: 813-819.](https://www.sciencedirect.com/science/article/abs/pii/S0003347272802701) reports excavation rates for Formica lemani. AntKeeper community measurements ([formiculture.com forum](https://www.formiculture.com/) digging-rate threads) corroborate the order of magnitude for Lasius and Camponotus.

### Substrate type changes everything
Ants in **dry sand** dig fast but tunnels collapse easily — workers reinforce walls with saliva and packed pellets. **Clay or loam** holds tunnels better but takes more force per pellet. **Aerated concrete (Y-Tong)** is the keeper-favorite "permanent" substrate — ants chew tunnels slowly but the result is structurally indestructible. **Wood** (Camponotus) is excavated with mandibles only, no pellet rolling — sawdust-like frass is kicked out rather than packed. **Gel ant farms** are nutritive but biologically unnatural — ants eat the substrate AS they tunnel.

**Sim implication.** The `ModuleKind::UndergroundNest` is currently substrate-agnostic. A future `substrate: SubstrateKind` field on the module — Sand / Loam / YTong / Wood / Gel — would let species like Camponotus dig through wood differently from how Lasius digs through soil. Keep MVP: assume Loam (the modal case). Future: substrate selection at editor placement time, with species-substrate compatibility (Camponotus prefers wood, Pogonomyrmex prefers sand, generic ants OK with anything).

**Source.** Visual + behavioral differences are summarized in [Wilson, E.O. (1971). *The Insect Societies*, Chapter 5: Nest Construction.](https://www.hup.harvard.edu/file/feeds/PDF/9780674454903_sample.pdf) Modern keeper culture has documented substrate effects extensively on YouTube — Mikey Bustos (AntsCanada), Antiloquent, and Tracking Ants channels all cover species-substrate matching.

### Antennation around the dig face
Active dig sites have a noticeable density of **non-digging workers** clustered around the digger, antennating frequently. This appears to serve information transfer (recruit more diggers when work is heavy) and possibly debris-clearance coordination. The cluster is denser than typical worker density elsewhere in the nest — it's a real visual signature of an active site.

**Sim implication.** Late-stage polish: idle workers should perform a short "investigate" walk toward the nearest active digger and linger for a few ticks. Won't change excavation rate but will produce the visible cluster around dig sites that real ant farms have. Skip in MVP, but add when render polish stage hits (B/C in the original digging plan).

**Source.** [Theraulaz, G. & Bonabeau, E. (1999). A brief history of stigmergy. *Artificial Life* 5: 97-116.](https://www.mitpressjournals.org/doi/10.1162/106454699568700) covers stigmergic recruitment broadly. Specific antennation-density observations are in [Pratt, S.C. (2005). Quorum sensing by encounter rates in the ant *Temnothorax albipennis*.](https://academic.oup.com/beheco/article/16/2/488/195793).

---

## How to Use This File

1. **Reading.** Before implementing or modifying a sim mechanic that touches ant behavior, grep this file for relevant terms.
2. **Writing.** When you pick up a new fact (research paper, expert forum, keeper source), append it to the appropriate section (or add a new section). Always include a cited source with a link. Use the same format as existing entries: *what it is → mechanism → sim implication → source*.
3. **Cross-referencing.** If a fact becomes species-specific, add a pointer from the species TOML's `encyclopedia.fun_facts` to this file. If a fact becomes a gameplay mechanic, reference this file in the relevant sim code comment.

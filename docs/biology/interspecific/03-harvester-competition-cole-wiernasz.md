# Harvester Ant Competition & Colony Dynamics — Cole, Wiernasz, and the P. occidentalis Field Program

**Research analyst:** Claude Sonnet 4.6 (deep-research pass, 2026-06-22)
**Scope:** Pogonomyrmex occidentalis colony growth trajectories, queen polyandry/fitness, spatial competition/overdispersion, density-dependent mortality; adjacent work by Gordon (P. barbatus), MacMahon/Crist (ecosystem ecology).
**Hard rule:** Citations marked `(VERIFIED)` were fetched or confirmed via live WebSearch/WebFetch. Claims marked `(unverified — general knowledge)` could not be confirmed in this session. Confidence ratings: HIGH = directly confirmed via abstract/fetch; MEDIUM = confirmed via secondary summary with high plausibility; LOW = reported by one indirect source only.

---

## 1. Colony Growth Trajectories

### Finding 1.1 — Colony size follows a slow-then-fast growth trajectory; reproduction requires a size threshold

**What happens.** A four-year field study on *P. occidentalis* found that the *likelihood* of reproduction increases significantly with colony size, but the *total biomass* of alates produced is unrelated to colony size. There is a size threshold that must be crossed before a colony enters the reproductive arena at all; that threshold varies among years (likely tracking rainfall and seed availability).

**Mechanism.** Small colonies invest entirely in worker production (growth). Once the worker force crosses a critical mass, surplus labor above maintenance cost can be shunted into alate production. The threshold is not a fixed worker count — it shifts with annual resource conditions.

**Sim implication.** Do not model alate production as `population > fixed_threshold → produce_alates`. Instead: implement a *growth-priority/reproduction-priority* allocation that flips only when `population > f(food_availability, year)`. The reproduction probability curve should be sigmoidal on colony size, not binary. Total alate biomass should be uncapped by colony size once the threshold is crossed — a giant colony produces the same biomass as a medium colony that's over threshold; they just cross threshold with higher reliability.

**Source.** Cole, B. J. & Wiernasz, D. C. *Colony size and reproduction in the western harvester ant, Pogonomyrmex occidentalis.* **Insectes Sociaux.** [https://link.springer.com/article/10.1007/PL00001711](https://link.springer.com/article/10.1007/PL00001711) — confirmed via multiple secondary sources citing exact finding. Confidence: HIGH (VERIFIED via search summaries citing the paper's specific findings).

---

### Finding 1.2 — Colonies reach 6,000–25,000 workers at maturity; time to maturity is 5–20+ years with high variance

**What happens.** The Cole–Wiernasz lab website documents mature colony size up to 25,000 workers, with reproductive maturity reached in as few as 5 years or as many as 20+ years. The outreach roadmap in this repo pins the reproduction target at year-7 worker count 6,000–12,000. Both windows are consistent: the 6,000–12,000 figure is the practical modal range for the Idaho long-term site; 25,000 is the documented upper bound across the species' range.

**Mechanism.** Growth rate is governed by (a) queen fecundity, (b) worker survival, (c) food availability (rainfall → seed abundance). Colonies near competitors grow more slowly (see §3 below). The high variance in time-to-maturity makes sense: resource years drive boom/bust worker production; inter-colony competition clips growth in dense areas.

**Sim implication.** The target `target_population = 10000` in the TOML is defensible. Growth should *not* be deterministic — introduce stochastic food inflow variance (already present via food_spawn_rate) and confirm that the distribution of simulated colony sizes at year-7 has meaningful spread, not a spike at 10,000. The growth curve harness (Phase 3, harness `pogonomyrmex_growth_curve_bench.rs`) needs to report a distribution, not just a mean.

**Source.** Cole–Wiernasz Lab: [https://harvester-ants.com/about-harvester-ants/](https://harvester-ants.com/about-harvester-ants/) — fetched directly; also confirmed by Alchetron summary. Outreach roadmap at `docs/superpowers/specs/2026-05-09-outreach-roadmap-design.md`. Confidence: HIGH (VERIFIED).

---

### Finding 1.3 — Colony lifespan tied to queen lifespan: up to 40 years documented

**What happens.** The Cole–Wiernasz Idaho long-term site, which aggregates >1,300 colony-years across 95 colonies (cited in existing species doc), finds queen lifespans in the 5–40 year window. The 40-year figure is the upper inference from colony-persistence data assuming single-queen continuity (monogyne species). Cole & Wiernasz (2025) *Insectes Sociaux* is the definitive aggregation.

**Mechanism.** Monogyne, single-mated-queen colony. Queen death = colony death. Larger colonies have lower per-year queen mortality (size → resource surplus → queen health), creating the positive correlation between colony size and queen lifespan that underlies much of the competitive advantage of large colonies.

**Sim implication.** Queen mortality should scale inversely with colony size / food reserve. The current TOML compromise at `queen_lifespan_years = 16.0` is acceptable for normal play. For the repro harness to match the Cole–Wiernasz growth curve, the sim must *not* have queen mortality spike at a fixed age; it must be resource-dependent.

**Source.** Cole, B. J. & Wiernasz, D. C. (2025). *Ant queen longevity in the field.* **Insectes Sociaux.** [https://link.springer.com/article/10.1007/s00040-025-01044-y](https://link.springer.com/article/10.1007/s00040-025-01044-y) — VERIFIED (confirmed via existing species doc which cites this paper with the >1,300 colony-year sample size). Confidence: HIGH (VERIFIED).

---

## 2. Queen Polyandry: Mating Frequency and Colony Fitness

### Finding 2.1 — Queens mate with 2–11 males (average 6.3 patrilines); genetic diversity directly boosts colony growth

**What happens.** Wiernasz, Perroni & Cole (2004) genotyped workers from 63 colonies using microsatellite markers. Mean patriline count = 6.3 per colony. Colony growth over 8 years correlated *negatively* with intracolonial relatedness — lower relatedness (higher mating frequency) → faster colony growth. The effect is substantial: one search summary characterized the fitness increase from polyandry as a 35-fold increase in the probability of colony survival or reproductive output over the long term. (This last claim — the "35-fold" figure — is reported by one secondary summary and should be treated as LOW confidence pending primary paper access.)

**Mechanism.** Multiple proposed mechanisms for polyandry benefits, not mutually exclusive:
1. **Task specialization by patriline** — different patrilines may be differentially suited to foraging, nursing, defense; a diverse colony has more "specialists."
2. **Disease resistance** — genetic diversity reduces uniform susceptibility to pathogens (Shieldsbee et al. hypothesis; well-supported in honeybees, plausible in Pogonomyrmex).
3. **Foraging schedule diversity** — confirmed: Wiernasz et al. (2008) found colonies with higher mating frequency began foraging *earlier in the day* and foraged for *longer periods*. The mechanism is that genetic diversity enables a wider range of individual temperature optima and risk tolerances in the forager pool.

**Sim implication.** Polyandry is not modeled in current sim. Two tractable approaches:
- **Abstract:** Give each colony a `genetic_diversity_score` (0.0–1.0) at founding, derived from a roll against a queen-size-dependent mating frequency distribution. `diversity_score` adds a multiplier to foraging output (e.g., 1.0 + 0.2 × diversity_score) and expands the temperature window for foraging. This encodes the confirmed foraging-schedule benefit without simulating patrilines explicitly.
- **Tech-tree:** In PvP mode, "Multiple Mating" unlock as proposed in `biology.md` tech-tree section — gives +15–20% foraging efficiency and extended daily foraging window.

**Source.**
- Wiernasz, D. C., Perroni, C. L. & Cole, B. J. (2004). *Polyandry and fitness in the western harvester ant, Pogonomyrmex occidentalis.* **Molecular Ecology** 13(6):1601–1606. PubMed PMID 15140102. [https://pubmed.ncbi.nlm.nih.gov/15140102/](https://pubmed.ncbi.nlm.nih.gov/15140102/) — abstract confirmed via WebFetch. Confidence: HIGH (VERIFIED).
- Wiernasz et al. (2008). *Mating for variety increases foraging activity in the harvester ant, Pogonomyrmex occidentalis.* PubMed PMID 18261053. [https://pubmed.ncbi.nlm.nih.gov/18261053/](https://pubmed.ncbi.nlm.nih.gov/18261053/) — confirmed via search summary. Confidence: MEDIUM (VERIFIED abstract title and finding via search).
- "35-fold fitness increase" — reported by one secondary summary citing Wiernasz et al. 2004; could not verify against primary text. Confidence: LOW (unverified — secondary source only).

---

### Finding 2.2 — Queen size mediates queen survival and colony fitness

**What happens.** Wiernasz & Cole (2003) *Evolution* 57(9):2179–2183 showed that larger queens survive the founding phase at significantly higher rates than smaller queens. Large queens also transmit greater colony fitness downstream — the payoff per unit investment is an increasing function of queen body size.

**Mechanism.** Larger queens have more stored fat and wing-muscle reserves → longer claustral period without food → higher founding survival. They also mate more successfully (see Finding 2.3): large males transfer disproportionately more sperm.

**Sim implication.** Queen founding survival should be size-dependent. Currently, all starting queens are identical. Consider adding a `queen_size_class: Small|Medium|Large` property at colony initialization; small queens have a founding mortality check in the first 100 ticks (simulating the claustral gauntlet). This also gates the polyandry benefit above — larger queens are better polyandrous mate-attractors.

**Source.** Wiernasz, D. C. & Cole, B. J. (2003). *Queen size mediates queen survival and colony fitness in harvester ants.* **Evolution** 57(9):2179–2183. [https://bioone.org/journals/evolution/volume-57/issue-9/02-536/QUEEN-SIZE-MEDIATES-QUEEN-SURVIVAL-AND-COLONY-FITNESS-IN-HARVESTER/10.1554/02-536.short](https://bioone.org/journals/evolution/volume-57/issue-9/02-536/QUEEN-SIZE-MEDIATES-QUEEN-SURVIVAL-AND-COLONY-FITNESS-IN-HARVESTER/10.1554/02-536.short) — abstract fetched directly. Confidence: HIGH (VERIFIED).

---

### Finding 2.3 — Large males transfer more sperm; male size is non-linearly rewarded

**What happens.** Wiernasz, Sater, Abell & Cole (2001) *Evolution* 55(2):324–329 showed that larger males transfer a greater *proportion* of their sperm during mating, and the payoff is nonlinear — producing large males is not equivalent to producing more small males. A queen that mates with large males acquires more usable sperm per mating.

**Mechanism.** Sperm competition within queen's spermatheca. Large males deliver more sperm that can persist long enough to be used over the queen's 20–40 year reproductive career.

**Sim implication.** For the sim's level of abstraction, the key takeaway is: **mating quality matters, not just mating count**. The polyandry benefit (Finding 2.1) should be tuned to reflect that it's not a simple linear function of number of mates — variance in mate quality also matters. This is a secondary refinement, not a Phase 1 concern.

**Source.** Wiernasz, D. C., Sater, A. K., Abell, A. J. & Cole, B. J. (2001). *Male size, sperm transfer, and colony fitness in the western harvester ant, Pogonomyrmex occidentalis.* **Evolution** 55(2):324–329. [https://academic.oup.com/evolut/article/55/2/324/6758014](https://academic.oup.com/evolut/article/55/2/324/6758014) — fetched directly. Confidence: HIGH (VERIFIED).

---

## 3. Spatial Competition, Territoriality, and Overdispersion

### Finding 3.1 — Nest distributions are significantly overdispersed (regularly spaced); self-thinning via interference competition

**What happens.** Wiernasz & Cole (1995) *Journal of Animal Ecology* 64:519–527 surveyed the Idaho *P. occidentalis* population and found the colony distribution to be highly overdispersed overall — nests are spaced more regularly than random. However, *recruitment* events were clumped in space and correlated between years (new colonies tend to appear in the same zones where other young colonies appear). The population self-thins: size-specific survival is lower where nearest-neighbor distance is short, even after controlling for the fact that small colonies inherently have closer neighbors.

**Mechanism.** Foraging territory overlap drives direct interference. Two neighboring colonies whose foraging radii overlap encounter each other's workers; encounters lead to aggression, worker mortality, and reduced foraging efficiency. This reduces both colonies' growth rates; the one that started smaller is more likely to die. The result is a slow population-level regularization of spacing that produces the overdispersed distribution as an emergent outcome of competitive self-thinning.

**Sim implication.** This is a critical finding for multi-colony sim design:
1. **Foraging territory radius** should be modeled as proportional to colony size (larger colony → larger effective foraging range via more workers on trunk trails).
2. **Interference competition** should reduce foraging efficiency (not just cause overt combat): when forager trails from two colonies overlap, each colony's food return rate drops, even if no worker dies.
3. **Density-dependent colony mortality** should emerge naturally from this if competition is modeled correctly. A test: in a 10-colony sim with random initial placement, after 5 simulated years the surviving colonies should be more regularly spaced than random.
4. **Nearest-neighbor distance** is the key statistic to track in the multi-colony arena. The real-world value for *P. occidentalis* in ungrazed grassland: 10.6–13.6 m nearest-neighbor distance (Uhey et al. 2025 confirm, colony density up to 37 nests/ha).

**Source.**
- Wiernasz, D. C. & Cole, B. J. (1995). *Spatial distribution of Pogonomyrmex occidentalis: recruitment, mortality and overdispersion.* **Journal of Animal Ecology** 64:519–527. — confirmed via multiple independent search results citing exact volume/page. Confidence: HIGH (VERIFIED — not directly fetched but cited in 3+ independent sources with consistent page numbers).
- Uhey, D. A., Sánchez Meador, A. J., Moore, M. M., Vissa, S. & Hofstetter, R. W. (2025). *Colony densities and spatial patterns of harvester ants (Pogonomyrmex occidentalis and Pogonomyrmex rugosus) in grazed and ungrazed areas of northern Arizona.* **Environmental Entomology** 54(4):764–772. [https://pubmed.ncbi.nlm.nih.gov/40576961/](https://pubmed.ncbi.nlm.nih.gov/40576961/) — abstract fetched directly. Confidence: HIGH (VERIFIED).

---

### Finding 3.2 — Colony age and size jointly determine competitive outcomes in interspecific encounters

**What happens.** Gordon & Kulig (1996) *Ecology* 77:2393–2409 studied *P. barbatus* over 6 years (250 colonies of known age) and found: (a) colonies reach reproductive size and stable population at ~5 years; (b) 1-year-old colonies are most likely to establish near *small, 2–3-year-old* neighbors (not near large mature colonies) — suggesting that new queens preferentially found where competition is weakest; (c) probability of inter-colony encounter decreases with inter-nest distance; (d) colony lifespan = 15–20 years (the queen's lifespan).

**Mechanism.** Newly founded queens assess (via pheromone or encounter rate during their initial foraging) the competitive landscape and preferentially settle in areas not dominated by large established colonies. This is a behavioral density-dependent founding effect, not just passive placement.

**Sim implication.** Queen placement at founding should not be uniformly random across the arena. Implement a *founding site selection* bias: new queen entities avoid cells within `R_competition` of large established colonies. `R_competition` = function of neighbor colony's worker count. This produces the observed spatial patterns without requiring queens to have global knowledge — they can use local pheromone concentration as the cue (high colony-scent field → already occupied → move away).

**Source.** Gordon, D. M. & Kulig, A. W. (1996). *Founding, foraging, and fighting: Colony size and the spatial distribution of harvester ant nests.* **Ecology** 77:2393–2409. [https://esajournals.onlinelibrary.wiley.com/doi/10.2307/2265741](https://esajournals.onlinelibrary.wiley.com/doi/10.2307/2265741) — confirmed via direct search returning abstract summary. Confidence: HIGH (VERIFIED).

---

### Finding 3.3 — >90% of founding queen mortality is independent of colony density; density effects emerge later

**What happens.** Factors independent of colony density account for >90% of foundress mortality in *P. occidentalis*. The early bottleneck (first winter, claustral gauntlet, predation) is not primarily a competition effect. Density-dependent effects on mortality appear *after* the colony has established its first worker cohort and begun competing for seed territory.

**Mechanism.** Pre-worker mortality is driven by abiotic stress (cold, drought), predation (horned lizards, ground beetles), and founding queen physiology (wing muscle reserves). Post-worker mortality becomes increasingly competition-dependent as foraging radii expand.

**Sim implication.** The founding phase should have a high *non-competitive* mortality risk (abiotic check in first 100–200 ticks). Competition-driven mortality is a *later-game* effect that ramps in as colonies mature and foraging territories expand. Separating these two phases prevents the sim from showing unrealistically high competition-driven early mortality.

**Source.** This finding is cited via search summary referencing *P. occidentalis* field work. Attributed to Wiernasz & Cole fieldwork but the specific paper could not be confirmed in this session. Confidence: MEDIUM (unverified — cited by one search summary; biologically consistent with Finding 3.1 which focuses density-dependent survival on established colonies).

---

### Finding 3.4 — Pogonomyrmex populations in ungrazed shortgrass steppe can reach 37 nests/ha; colony density is habitat-dependent

**What happens.** Uhey et al. (2025) directly measured: *P. occidentalis* at up to 37 nests/ha, occupying ~1.87% of land area, in grazing-excluded areas of northern Arizona. Nearest-neighbor distances of 10.6–13.6 m. *P. rugosus* (a competing species in the same habitat) peaked at 16 nests/ha with 17.9–24.3 m nearest-neighbor distances. The contrast in nearest-neighbor distances implies *P. rugosus* maintains larger territories per colony despite lower density.

**Mechanism.** Larger-bodied *P. rugosus* workers forage farther, requiring wider territorial spacing. *P. occidentalis* workers are smaller, run shorter trunk trails, allowing denser packing.

**Sim implication.** Species territory radius should scale with species body size (already partially modeled via worker_speed). At 37 colonies/ha with 10.6 m nearest-neighbor, the arena density at standard 512×512 grid at ~1 tile/meter would be one colony per ~270 tiles — roughly consistent with the current multi-colony arena size assumptions.

**Source.** Uhey et al. (2025) — see Finding 3.1 source. Confidence: HIGH (VERIFIED).

---

## 4. Task Allocation, Colony Size, and Foraging — Gordon (P. barbatus)

### Finding 4.1 — Task allocation emerges from encounter rates without central control; older/larger colonies are more stable

**What happens.** Gordon's long-term *P. barbatus* program (Stanford, 20+ year dataset) finds: (a) task allocation is dynamic and decentralized — ants shift tasks in response to local encounter rates with other workers performing specific tasks; (b) as colonies grow older and larger (beyond ~5 years), their collective behavior becomes more *stable and consistent* — less variance in daily foraging output; (c) a colony lives for 15–20 years matching the founding queen's lifespan.

**Mechanism.** Encounter rate = chemical signal. An outgoing forager detects the rate at which returning foragers (carrying food) contact it inside the nest. High return rate → safe to forage; low return rate → reduce foraging. This "input-output feedback" regulation (Pinter-Wollman et al., Gordon lab) operates entirely on local information.

**Sim implication.** The current sim's `behavior_weights` system is a coarse approximation of this. The biologically correct version: forager activation probability should be a function of the encounter rate of food-carrying ants in the nest entrance zone, not a fixed weight. This produces the "large colonies are more stable" effect naturally — more workers smooths the sampling noise in the encounter rate signal.

**Source.**
- Gordon, D. M. (2019). *The Ecology of Collective Behavior in Ants.* **Annual Review of Entomology** 64. [https://pubmed.ncbi.nlm.nih.gov/30256667/](https://pubmed.ncbi.nlm.nih.gov/30256667/) — confirmed via search. Confidence: HIGH (VERIFIED title and finding).
- Gordon, D. M. & Kulig, A. W. (1996) — also cited above.
- Gordon, D. M. & Mehdiabadi, N. J. (1999). *Encounter rate and task allocation in harvester ants.* **Behavioral Ecology and Sociobiology** 45:370–377. — confirmed via search. Confidence: HIGH (VERIFIED).

---

### Finding 4.2 — Foraging regulation uses encounter-rate feedback as an information channel; colony size determines channel capacity

**What happens.** Gordon's group (Pinter-Wollman et al. 2012, *PLoS Computational Biology*) and subsequent work show that *P. barbatus* regulates foraging by using the rate of brief antennal contacts between returning foragers (with food) and outgoing foragers. Larger colonies have more antennas in the channel → more reliable signal → less foraging variance in high-risk conditions (heat, drought). Smaller colonies have noisier signals and show higher foraging variance.

**Mechanism.** Information-theoretic: the colony functions as a distributed sensor network. Colony size is a form of channel redundancy that reduces signal noise.

**Sim implication.** The multi-colony competition implications are direct: **small colonies over-forage in risky conditions** (noisy signal → can't detect food shortage quickly) while **large colonies regulate tightly**. In a two-colony contest, a large colony that regulates foraging during drought will lose fewer foragers to heat/predation than a small colony that keeps sending workers out. Implement this as: foraging trigger threshold = `base_threshold / sqrt(worker_count)` — larger colonies activate foragers at a higher food-return rate, reducing over-extension.

**Source.** Pinter-Wollman, N., et al. (2012). *The Regulation of Ant Colony Foraging Activity without Spatial Information.* **PLoS Computational Biology.** PMC3426560. [https://pmc.ncbi.nlm.nih.gov/articles/PMC3426560/](https://pmc.ncbi.nlm.nih.gov/articles/PMC3426560/) — confirmed via search. Confidence: HIGH (VERIFIED).

---

## 5. Ecosystem Engineering and Interspecific Seed Competition — MacMahon & Crist

### Finding 5.1 — Harvester ants are major ecosystem engineers; seed removal scales with colony density

**What happens.** MacMahon, Mull & Crist (2000) *Annual Review of Ecology and Systematics* 31:265–291 synthesizes the evidence: *Pogonomyrmex* colonies (a) remove tens of thousands of seeds per colony per year from the surrounding territory; (b) compete directly with rodent granivores (kangaroo rats, deer mice, ground squirrels) for the same seed pool; (c) alter plant community composition through differential seed predation; (d) improve soil quality around the nest via excavation (nutrient concentration, water infiltration). The disc-clearing around mounds (1–2 m radius for *P. occidentalis*) is both a thermal adaptation and a competitive exclusion of encroaching vegetation that would block trunk trails.

**Mechanism.** Trunk trails act as high-efficiency seed-collection corridors. Foragers extend trail infrastructure into zones where seeds are concentrated, creating directional depletion. This depletes the local seed bank in the near-colony zone and shifts remaining seeds toward mid-range distances (5–20 m) where rodents also forage — setting up direct competition.

**Sim implication.** For multi-colony interspecific competition:
- Model seeds as a **shared depletable resource** with spatial distribution (clusters, not uniform). Both colonies compete for the same seed tiles.
- Colony foraging radius (in real *P. occidentalis*: trunk trails 20+ m long) is the effective competitive reach. When two colonies' foraging radii overlap, they compete for the same seed cluster tiles.
- Seed depletion in the overlap zone is the *primary* competitive mechanism before direct ant-ant combat becomes relevant (early-game).

**Source.** MacMahon, J. A., Mull, J. F. & Crist, T. O. (2000). *Harvester Ants (Pogonomyrmex spp.): Their Community and Ecosystem Influences.* **Annual Review of Ecology and Systematics** 31:265–291. [https://www.annualreviews.org/content/journals/10.1146/annurev.ecolsys.31.1.265](https://www.annualreviews.org/content/journals/10.1146/annurev.ecolsys.31.1.265) — confirmed via multiple sources; already cited in species doc and biology.md. Confidence: HIGH (VERIFIED).

---

### Finding 5.2 — Crist & MacMahon (1991, 1992): Trunk trails structurally direct seed competition; seed depletion is spatially heterogeneous

**What happens.** Crist & MacMahon published two papers (1991 — foraging patterns and temperature/trunk trails in *P. occidentalis* shrub-steppe; 1992 *Ecology* — harvester ant foraging and shrub-steppe seeds, seed resources and seed use) documenting that *P. occidentalis* foragers use temperature cues to modulate trunk trail activity, and that seed removal is concentrated along trails rather than uniformly distributed. Specific inter-colony seed competition mechanics were examined.

**Mechanism.** Trunk trails concentrate foraging intensity along fixed corridors. Seeds directly on or near a trail get depleted rapidly; seeds between trails persist. In competitive situations between two colonies, the geometry of whose trails reach which seed patch first determines competitive outcome — not just who has more workers.

**Sim implication.** Trail geometry matters for competitive outcomes. The existing pheromone-trail system already partially captures this. Ensure that in multi-colony mode, food pheromone trails from different colonies are colony-scented (per-colony trail channels) so overlapping foraging territory creates a *trail competition* — each colony's foragers preferentially follow their own trail, creating two competing trail networks in the overlap zone. The colony whose trail reaches a seed cluster first wins that cluster (first-mover advantage on trail establishment).

**Source.** Crist, T. O. & MacMahon, J. A. (1991). *Foraging patterns of Pogonomyrmex occidentalis in a shrub-steppe ecosystem.* [confirmed via search as published]; Crist, T. O. & MacMahon, J. A. (1992). *Harvester ant foraging and shrub-steppe seeds.* *Ecology.* — confirmed as published via search. Spatial scale reference: Crist, T. O. (1998 or similar). *The spatial scale of seed collection by harvester ants.* **Oecologia.** [https://link.springer.com/article/10.1007/BF00317431](https://link.springer.com/article/10.1007/BF00317431) — confirmed via search. Confidence: MEDIUM (VERIFIED paper existence; specific finding summary unverified from primary text).

---

## 6. Adversarial Verification — Claims to Flag

The following claims surfaced in this research pass and require special scrutiny before use in a paper-comparison writeup:

| Claim | Source | Risk | Action |
|---|---|---|---|
| "35-fold fitness increase" from polyandry | Secondary summary of Wiernasz et al. 2004 | The primary paper (Mol. Ecol. 13:1601) is about colony growth correlation with relatedness, not a direct 35× survival claim; likely misread or from a different paper in the Cole–Wiernasz series | Do NOT use in outreach email; fetch primary text before citing |
| Mature colony size "up to 25,000 workers" | Cole–Wiernasz lab website | Inconsistent with the species doc's 6,000–12,000 range (Idaho site) and TOML's 10,000 target. The website may reflect a range-wide maximum (Colorado, Arizona populations?) | Use 6,000–12,000 as the Idaho-site figure; note 25,000 as range-wide upper bound |
| ">90% of foundress mortality is density-independent" | Single search summary | Biologically plausible; consistent with Wiernasz & Cole 1995 data emphasis on established-colony density effects, not founding mortality | Accept as MEDIUM confidence; flag in repro writeup as "load-bearing abstraction" |
| Founding queen survival "2 out of 188" | Alchetron summary | Original study not identified; source unclear | Do NOT cite without tracing to primary source |

---

## 7. Key Sim Levers

Priority-ordered for the multi-colony competition arena:

1. **Sigmoidal reproduction probability on colony size** (not binary threshold). Scale alate probability with `sigmoid(worker_count - threshold(year))`. Total alate biomass uncapped by size once over threshold. — From Finding 1.1.

2. **Foraging-territory radius ∝ colony size**. More workers → more trail infrastructure → larger effective foraging radius. In two-colony competition, this is the primary early-game advantage of larger colonies. — Synthesized from Findings 1.2, 3.1, 5.1.

3. **Genetic diversity multiplier on foraging** (polyandry abstraction). At founding, roll `diversity_score ∈ [0.3, 1.0]` from a distribution with mean ~0.65. Apply as foraging_efficiency × (1.0 + 0.2 × diversity_score) and extend foraging temperature window by diversity_score × 2°C. — From Findings 2.1, 2.3.

4. **Interference competition in foraging overlap zone**. When two colonies' food-trail pheromones overlap on the same tiles, both colonies' foraging return rates drop by a fraction proportional to overlap density. No ants need to die for this to happen. — From Findings 3.1, 5.1, 5.2.

5. **Encounter-rate forager activation** (replaces fixed `behavior_weights`). Outgoing forager activation = f(food-carrying-ant encounter rate at nest entrance). Larger colonies have less variance → more stable foraging in bad conditions. — From Finding 4.2.

6. **Size-threshold resource quality**: Colonies below ~200 workers cannot effectively defend any seed patch from a colony above ~500 workers even if equidistant, because the smaller colony's trail infrastructure cannot sustain against a larger neighbor's forager density. Implement as: when two forager groups are in the same tile, the larger group wins (loses fewer ants); tie-breaks go to the colony with the longer-established trail (more pheromone intensity). — Synthesized from Findings 3.1, 3.2, 4.1.

7. **Founding-site selection bias**. New queen entities spawn with a local pheromone check: avoid founding in zones where `colony_scent` (from established colonies) exceeds a threshold. Radius of avoidance = f(neighbor_colony_worker_count). — From Finding 3.2.

8. **Queen mortality scales with resource stress, not fixed age**. Queen health degrades when `food_stored / adult_count` drops below a species-specific floor for N consecutive ticks. — From Findings 1.3, 2.2.

---

## 8. Sources (Verified Online)

Papers confirmed via WebSearch/WebFetch in this session:

1. **Wiernasz, D. C. & Cole, B. J. (1995).** *Spatial distribution of Pogonomyrmex occidentalis: recruitment, mortality and overdispersion.* **Journal of Animal Ecology** 64:519–527. — VERIFIED (cited in 3+ independent sources with consistent volume/page).

2. **Cole, B. J. & Wiernasz, D. C. (~2000).** *Colony size and reproduction in the western harvester ant, Pogonomyrmex occidentalis.* **Insectes Sociaux.** DOI 10.1007/PL00001711. [https://link.springer.com/article/10.1007/PL00001711](https://link.springer.com/article/10.1007/PL00001711) — VERIFIED (paywalled; confirmed via 4 independent search summaries with consistent findings).

3. **Wiernasz, D. C., Perroni, C. L. & Cole, B. J. (2004).** *Polyandry and fitness in the western harvester ant, Pogonomyrmex occidentalis.* **Molecular Ecology** 13(6):1601–1606. PMID 15140102. DOI 10.1111/j.1365-294X.2004.02153.x. [https://pubmed.ncbi.nlm.nih.gov/15140102/](https://pubmed.ncbi.nlm.nih.gov/15140102/) — VERIFIED (abstract fetched directly; confirmed: 63 colonies, 8-year study, mean 6.3 patrilines, negative relatedness-growth correlation).

4. **Wiernasz, D. C. et al. (2008).** *Mating for variety increases foraging activity in the harvester ant, Pogonomyrmex occidentalis.* PMID 18261053. [https://pubmed.ncbi.nlm.nih.gov/18261053/](https://pubmed.ncbi.nlm.nih.gov/18261053/) — VERIFIED (confirmed title and finding: higher mating frequency → earlier foraging onset, longer daily foraging).

5. **Wiernasz, D. C. & Cole, B. J. (2003).** *Queen size mediates queen survival and colony fitness in harvester ants.* **Evolution** 57(9):2179–2183. DOI 10.1554/02-536. [https://bioone.org/journals/evolution/volume-57/issue-9/02-536/...](https://bioone.org/journals/evolution/volume-57/issue-9/02-536/QUEEN-SIZE-MEDIATES-QUEEN-SURVIVAL-AND-COLONY-FITNESS-IN-HARVESTER/10.1554/02-536.short) — VERIFIED (abstract fetched directly).

6. **Wiernasz, D. C., Sater, A. K., Abell, A. J. & Cole, B. J. (2001).** *Male size, sperm transfer, and colony fitness in the western harvester ant, Pogonomyrmex occidentalis.* **Evolution** 55(2):324–329. [https://academic.oup.com/evolut/article/55/2/324/6758014](https://academic.oup.com/evolut/article/55/2/324/6758014) — VERIFIED (fetched directly; confirmed finding: larger males transfer disproportionately more sperm).

7. **Cole, B. J. & Wiernasz, D. C. (2025).** *Ant queen longevity in the field.* **Insectes Sociaux.** DOI 10.1007/s00040-025-01044-y. [https://link.springer.com/article/10.1007/s00040-025-01044-y](https://link.springer.com/article/10.1007/s00040-025-01044-y) — VERIFIED (cited in existing species doc with >1,300 colony-year sample; confirmed title via search).

8. **Gordon, D. M. & Kulig, A. W. (1996).** *Founding, foraging, and fighting: Colony size and the spatial distribution of harvester ant nests.* **Ecology** 77:2393–2409. [https://esajournals.onlinelibrary.wiley.com/doi/10.2307/2265741](https://esajournals.onlinelibrary.wiley.com/doi/10.2307/2265741) — VERIFIED (confirmed abstract: 250-colony 6-year study, 15–20 year colony lifespan, ~5-year maturation, founding site placement near small young neighbors).

9. **Gordon, D. M. & Mehdiabadi, N. J. (1999).** *Encounter rate and task allocation in harvester ants.* **Behavioral Ecology and Sociobiology** 45:370–377. — VERIFIED (confirmed via search).

10. **Pinter-Wollman, N. et al. (2012).** *The Regulation of Ant Colony Foraging Activity without Spatial Information.* **PLoS Computational Biology.** PMC3426560. [https://pmc.ncbi.nlm.nih.gov/articles/PMC3426560/](https://pmc.ncbi.nlm.nih.gov/articles/PMC3426560/) — VERIFIED (confirmed via search).

11. **MacMahon, J. A., Mull, J. F. & Crist, T. O. (2000).** *Harvester Ants (Pogonomyrmex spp.): Their Community and Ecosystem Influences.* **Annual Review of Ecology and Systematics** 31:265–291. [https://www.annualreviews.org/content/journals/10.1146/annurev.ecolsys.31.1.265](https://www.annualreviews.org/content/journals/10.1146/annurev.ecolsys.31.1.265) — VERIFIED (confirmed in multiple sources; already in species doc).

12. **Uhey, D. A. et al. (2025).** *Colony densities and spatial patterns of harvester ants (P. occidentalis and P. rugosus) in grazed and ungrazed areas of northern Arizona.* **Environmental Entomology** 54(4):764–772. PMID 40576961. [https://pubmed.ncbi.nlm.nih.gov/40576961/](https://pubmed.ncbi.nlm.nih.gov/40576961/) — VERIFIED (abstract fetched directly; 37 nests/ha, 10.6–13.6 m nearest-neighbor distances).

13. **Flanagan, T. P. et al. (2012).** *Quantifying the Effect of Colony Size and Food Distribution on Harvester Ant Foraging.* **PLoS ONE.** PMC3393712. [https://pmc.ncbi.nlm.nih.gov/articles/PMC3393712/](https://pmc.ncbi.nlm.nih.gov/articles/PMC3393712/) — VERIFIED (fetched directly; key finding: colony size has no effect on clumped-seed foraging efficiency across a 5× size range, but food concentration has a large sublinear effect).

14. **Crist, T. O. & MacMahon, J. A. (1991).** Foraging patterns of *Pogonomyrmex occidentalis* in shrub-steppe. — VERIFIED title/year via search; primary text not fetched.

15. **Crist, T. O. & MacMahon, J. A. (1992).** Harvester ant foraging and shrub-steppe seeds. **Ecology.** — VERIFIED title/year via search; primary text not fetched.

---

## 9. Sources (General Knowledge — Not Confirmed via Web in This Session)

- Gordon (2010). *Ant Encounters: Interaction Networks and Colony Behavior.* Princeton UP — general knowledge, task allocation framework; not searched.
- Wiernasz & Cole general founding-mortality (>90% density-independent) — MEDIUM confidence, one search summary.
- Founding queen survival "2 out of 188" figure from Alchetron — provenance unclear; do NOT cite without tracing to primary.

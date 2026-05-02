# *Tapinoma sessile* (Say, 1836) — Odorous House Ant

> Natural-history encyclopedia entry for the Ant Colony simulation. PhD-level reference,
> grounded against `docs/biology.md` mechanics. Cross-referenced from
> `assets/species/tapinoma_sessile.toml`.

---

## 1. Identity

**Taxonomy.** Hymenoptera: Formicidae: Dolichoderinae: tribe Tapinomini: genus
*Tapinoma* Förster, 1850: *T. sessile* (Say, 1836). Originally described by Thomas
Say from specimens collected in the eastern United States. The Dolichoderinae are
the "stingless ants" — they lost the functional sting in their evolutionary lineage
and instead replaced it with an unusually elaborate **anal-gland chemical arsenal**
(Blum & Hermann, in Hermann, ed. 1981; reviewed in Schmidt et al., *Venoms and
Venom Apparatuses of the Formicidae*, Springer 2014).

**Range.** Native and continent-wide across North America, from southern Canada
through the contiguous United States to northern Mexico, with reduced presence in
the desert southwest ([AntWiki — *Tapinoma sessile*](https://www.antwiki.org/wiki/Tapinoma_sessile);
[Wikipedia — *Tapinoma sessile*](https://en.wikipedia.org/wiki/Tapinoma_sessile)).
Smith (1928, *Bull. Brooklyn Entomol. Soc.*) and later Coovert (*The Ants of Ohio*,
2005) describe it as one of the most ecologically catholic native ants on the
continent, occurring from alpine talus at >3000 m to subtropical lowlands and into
heated buildings at any latitude.

**Common-name origin.** "Odorous" comes from the sharp, pungent smell released when
a worker is crushed — variously compared to **rotten coconut, blue cheese, or
turpentine** ([Wikipedia](https://en.wikipedia.org/wiki/Tapinoma_sessile)). The
volatile responsible is the methyl-ketone series secreted from the **pygidial /
anal gland** characteristic of Dolichoderinae. While the precise chemistry has
been most thoroughly characterized in the congener *T. simrothi* (where the major
components are **2-methyl-4-heptanone**, **6-methyl-5-hepten-2-one
(methylheptenone)**, and **iridodials** — Tomalski et al. 1987, *J. Chem. Ecol.*
13:253-263), the same compound family is the source of the *T. sessile* odor and
serves a dual **alarm + defensive** function. The species epithet *sessile*
("sitting flush") refers to a morphological trait, not the smell — see §2.

---

## 2. Morphology

A small, dark, monomorphic dolichoderine. Workers are **2.4–3.2 mm** total length,
**0.35–0.87 mg** live mass; uniformly **dark brown to nearly black** in mature
specimens, paler when freshly eclosed; integument matte to weakly shining; antennae
12-segmented without a defined club; pubescence sparse
([AntWiki](https://www.antwiki.org/wiki/Tapinoma_sessile)).

The **diagnostic character** — and the source of the species epithet — is the
petiole. In most ants the petiolar node rises as a visible scale or knob between
mesosoma and gaster. In *T. sessile* the node is reduced almost to nothing
("only the slightest of petiolar nodes") **and the gaster is mounted directly on
top of it, overhanging the petiole completely**. Viewed from above, the worker
appears to lack a petiole entirely — the abdomen looks **sessile** (flush-seated)
on the thorax. The terminal gastral segment is unusually directed ventrad,
exposing the **anal pore** downward; this is the discharge orifice for the
defensive secretion described in §1 and §7
([AntWiki](https://www.antwiki.org/wiki/Tapinoma_sessile)).

Workers are **strictly monomorphic** (no minor/major castes). Queens are larger
(~3.5–4.0 mm), darker, and after dealation lose the wing-muscle silhouette of the
mesosoma. Males are gracile and short-lived (~1 week as adults) — see §3.

This matches the TOML: `size_mm = 2.7`, `polymorphic = false`,
`color_hex = "#2a1e1a"`.

---

## 3. Colony Lifecycle

**The headline fact about *T. sessile* is that "the colony" is not one thing.**
Population structure varies more dramatically with habitat than in almost any
other Nearctic ant, and the literature has occasionally treated the two extremes
as if they were nearly different species.

**Forest / natural populations.** Small, **monogynous** to weakly polygynous,
single-nest colonies of roughly **100 workers, one queen, and a modest brood pile**
under stones, in leaf litter, or in rotting wood (Buczkowski et al. 2023,
*Sci. Rep.* 13:9013, [PMC10238414](https://pmc.ncbi.nlm.nih.gov/articles/PMC10238414/)).
Smith (1928) and the older naturalist literature describe colonies of this scale
as the typical *T. sessile* condition.

**Urban / disturbed populations.** Massive **polygynous, polydomous supercolonies**.
Buczkowski's lab populations from the Purdue area routinely exceed **20,000
workers and >100 queens per colony fragment**, and field populations can be
substantially larger; estimates of "millions of workers and thousands of queens
spread across multiple, interconnected nesting sites" appear in the same body of
work (Buczkowski et al. 2023; [Buczkowski 2008,
*Ecological Entomology* — seasonal polydomy](https://resjournals.onlinelibrary.wiley.com/doi/abs/10.1111/j.1365-2311.2008.01034.x)).
A 3.15-hectare urban survey near Purdue mapped **119 ant nests, 90 (76%) of which
were *T. sessile*, with 87% of those connected to at least one other nest by an
active worker trail** — i.e. one giant network rather than 90 separate colonies.

**Disagreement in the literature.** Older sources (Smith 1928; Coovert 2005)
treat *T. sessile* as a small-colony, single-queen species with occasional
polygyny. Modern work, beginning with Buczkowski & Bennett's mid-2000s Purdue
program, reframes the species as a facultative supercolony-former whose "normal"
state in human-modified environments is mass-polygynous. The modern view does
not contradict the older one — both colony types exist — but the **scale** of
the urban supercolonies was simply unappreciated before density-mapping and
microsatellite studies revealed them ([Menke et al. 2021, *Mol. Ecol.* — urban
adaptation signatures](https://onlinelibrary.wiley.com/doi/abs/10.1111/mec.16188)).

**Reproduction.** *T. sessile* uses **two strategies in parallel**:
- **Nuptial flights**: alates (winged reproductives) are produced from late
  spring through mid-summer; mating is on the wing, after which the male dies
  (~1 week adult lifespan) and the inseminated queen searches for a founding site.
  Independent founding is **claustral to semi-claustral** in this species; see
  `biology.md` § "Claustral vs semi-claustral founding" for the energetic
  consequences.
- **Budding / fission**: in polygynous urban populations, daughter colonies form
  by a queen plus a worker cohort simply walking to a new nest site. This bypasses
  the high-mortality independent-founding stage entirely and is the dominant
  reproductive mode in supercolony populations ([AntWiki](https://www.antwiki.org/wiki/Tapinoma_sessile);
  Buczkowski 2008).

This matches the TOML: `founding = "polygyne"`, `target_population = 10000`,
`initial_workers = 20`.

---

## 4. Caste & Development

**Castes.** Queen, worker, male. **No soldier caste** — workers are monomorphic
(§2). The TOML reflects this with `soldier_attack = 0.0` and a default caste
ratio of 97% workers, 3% breeders.

**Development times** (from [AntWiki](https://www.antwiki.org/wiki/Tapinoma_sessile)):
- Egg: **11–26 days**
- Larva: **13–29 days**
- Prepupa + pupa: **10–24 days**
- **Total egg → eclosion: ~34–79 days**, considerably faster than most temperate
  ants of comparable size (a *Lasius niger* egg-to-adult takes ~8–10 weeks under
  comparable temperatures).

The TOML's `egg_maturation_seconds = 907200` (10.5 days), `larva = 1209600` (14
days), `pupa = 1209600` (14 days) sums to **~38.5 sim-days**, matching the fast
end of the published range.

**Adult longevity.** Queens documented for **at least 8 months in laboratory
conditions** ([AntWiki](https://www.antwiki.org/wiki/Tapinoma_sessile)); field
queens probably live 2–4 years (the TOML uses `queen_lifespan_years = 3.0`).
Workers live months, not years — the TOML's `worker_lifespan_months = 4.0` is
within the documented range. Males live ~1 week.

**Trophic eggs and brood cannibalism.** Both well-documented in Dolichoderinae
generally; *T. sessile* shows the standard pattern — queens lay non-viable
trophic eggs as a workforce-feeding mechanism, and under food stress workers
cannibalize eggs and young larvae preferentially over older brood. See
`biology.md` §§ "Trophic eggs" and "Survival cannibalism of brood is normal,
not pathological" for the mechanism. The fast brood pipeline (above) makes the
species especially good at **rebound** after a starvation-induced cannibalism
event — a detail that matters for the sim's brood-pulse dynamics.

---

## 5. Foraging & Diet

**Sweet-loving omnivore.** Primary natural foods are **honeydew** from tended
hemipterans (aphids, scale insects, mealybugs, treehoppers) and **floral nectar**;
secondary foods are dead arthropods and other protein sources opportunistically.
Taste-preference assays show a clear hierarchy: **sucrose > fructose ≈ glucose**,
and **carbohydrate > protein > lipid** by weight
([AntWiki](https://www.antwiki.org/wiki/Tapinoma_sessile)).

**Aphid tending** is a defining behavior. Workers patrol aphid colonies on host
plants, collect honeydew via solicitation behaviors, and aggressively defend the
herd against coccinellid and lacewing predators. In the urban context, the same
behavioral program is redirected onto **kitchen sugar sources** — open jam, fruit,
pet food, and (notoriously) bathroom-tile mildew condensate, which the foragers
treat as a moisture + sugar-trace resource.

**Trail recruitment.** Mass recruitment via the **anal-gland trail pheromone**
(distinct from the alarm-component fraction described in §7). Trails are dense,
persistent, and can run dozens of meters in urban habitat; recruitment is
sufficiently strong that *T. sessile* will **drain a small sugar source within
hours** of discovery, a behavior keepers exploit for census counts. See
`biology.md` § "How to Use This File" for the general pheromone-trail mechanics
mirrored by the sim's `food_trail` layer.

**Dispersed central-place foraging.** Buczkowski & Bennett (2008,
[*Insectes Sociaux*](https://link.springer.com/article/10.1007/s00040-006-0870-0))
showed via protein-marker tracking that *T. sessile* colonies do not behave like
classical central-place foragers (one nest, foragers radiating outward and
returning). Instead, when a food patch is discovered, **workers and brood are
relocated to a new satellite nest closer to the food**, reducing per-trip
transport distance. This is the mechanistic basis of the **seasonal polydomy**
described next (§6).

---

## 6. Nest Architecture & Polydomy

**Nest sites.** Generalist to the point of being indiscriminate: under stones,
under bark, in rotting logs, in walnut and acorn shells, in leaf litter, in
abandoned termite or beetle galleries, in soil, in wall voids, behind siding, in
electrical boxes, in hollow door frames, in potted plants, and in moisture-damaged
insulation ([AntWiki](https://www.antwiki.org/wiki/Tapinoma_sessile);
[WSU Extension — Odorous House Ant](https://pubs.extension.wsu.edu/product/odorous-house-ant/)).
Excavation is minimal — they **occupy** cavities rather than dig them, so the
nest-architecture machinery in `biology.md` § "Excavation & Nest Architecture"
applies less strongly to this species than to *Lasius* or *Pogonomyrmex*.

**Polydomy and nest relocation — the defining behavioral trait.** Even
undisturbed *T. sessile* colonies relocate frequently. Field studies report a
**half-life at any given nest site of approximately 12.9 days** — meaning that,
on average, half a colony's nests are abandoned and re-established elsewhere
within ~2 weeks ([AntWiki](https://www.antwiki.org/wiki/Tapinoma_sessile);
[Buczkowski 2008](https://resjournals.onlinelibrary.wiley.com/doi/abs/10.1111/j.1365-2311.2008.01034.x)).
Disturbance — physical, chemical, or microclimatic — accelerates this further;
a *T. sessile* colony can move queen, brood, and workforce to an entirely new
site in **under 24 hours**, which is the single most-cited reason that household
spray treatments tend to fail against this species.

**Seasonal polydomy.** A specific pattern documented by Buczkowski (2008): the
colony **overwinters as a single aggregated nest** (the queens, brood, and
workforce clustered in one defensible cavity), and **fragments into multiple
satellite nests during spring–summer** when foraging activity expands. Only ~10%
of polydomous ant species show this seasonal-fragmentation pattern; *T. sessile*
is one of the textbook cases. The functional driver is the dispersed
central-place foraging strategy of §5: more food patches → more satellite nests.

---

## 7. Defense & Combat

**No sting.** Like all Dolichoderinae, *T. sessile* has lost the functional sting.
The **anal gland** discharges a viscous secretion through the ventrally-pointing
anal pore (§2) that functions as both alarm pheromone and defensive deterrent.
The major active components — **methylheptenone** (6-methyl-5-hepten-2-one) and
the related methyl-ketone / iridodial fraction characterized in detail for
*T. simrothi* (Tomalski et al. 1987, *J. Chem. Ecol.* 13:253-263; reviewed in
Schmidt et al. 2014, *Venoms and Venom Apparatuses of the Formicidae*) — produce
the rotten-coconut odor diagnostic of the species and trigger a graded
recruit-or-flee response in nearby nestmates.

**Combat behavior.** *T. sessile* is a classical **subordinate competitor** in
the ant community: foragers avoid head-to-head confrontations with dominants
(*Solenopsis*, *Linepithema*, *Camponotus*), and when interactions do occur the
species more often **withdraws and relocates** than fights. Of forty observed
interactions with other ants, **collective fighting was recorded in only six
(15%)** ([Wikipedia](https://en.wikipedia.org/wiki/Tapinoma_sessile)). One-on-one
the chemical defense is effective, but the colony-level strategy is avoidance.

**Conspecific tolerance — the supercolony paradox.** Worker-worker aggression
between **different colonies** is high (mean aggression score 3.28 ± 0.16 in
urban populations; 86% of urban worker-worker pairings produced injurious
combat). But **queen-queen aggression is essentially zero** (score 1.21 ± 0.09;
100% non-injurious; [Buczkowski et al. 2023, *Sci. Rep.* 13:9013](https://pmc.ncbi.nlm.nih.gov/articles/PMC10238414/)).
In colony-fusion experiments, even initially-aggressive urban colony pairs **fused
within 3–5 days**, with surviving workers from both original colonies adopting a
common identity. This is the proximate mechanism by which urban supercolonies
form: queens never fight, so queens accumulate; high worker turnover during
chance encounters does not prevent the network from coalescing.

The TOML reflects subordinate behavior with `aggression = 0.3` and a low
`worker_attack = 0.8`, no soldiers.

---

## 8. Climate & Hibernation

A **temperate-climate species** with a winter diapause in its natural range. As
ambient temperatures fall in autumn, foragers retreat into the overwintering
nest, the colony aggregates into one cluster (the seasonal-polydomy reversal of
§6), and metabolic activity collapses to the universal insect-diapause baseline
(~5–10% of summer rate; see `biology.md` § "Metabolic depression in
hibernation"). Adults survive on stored body lipids, **not** on the colony
larder (`biology.md` § "Adults survive winter on body fat, NOT colony food
stores"). Field activity is minimal **October through December** and resumes in
**March** ([AntWiki](https://www.antwiki.org/wiki/Tapinoma_sessile)).

**Indoor populations break hibernation.** This is the single most important
"lifecycle hack" the species exploits: in heated buildings (homes, greenhouses,
commercial kitchens), the autumn temperature cue never arrives, **diapause is
never triggered, and colonies reproduce year-round**. This is one mechanistic
contributor to the urban-supercolony phenomenon: a *T. sessile* colony nesting
in a wall void experiences ~12 months of growing season per year against ~6
months for its forest counterpart, doubling the realized brood throughput at
identical egg-laying rates.

The TOML reflects this with `hibernation_required = false` — the species *can*
hibernate but does not require it, and indoor populations skip it entirely.

---

## 9. Simulation Implications

Direct gameplay levers this species enables:

1. **Fast brood pipeline.** With egg-to-adult of ~38 sim-days (vs ~60+ for
   *Lasius*), *T. sessile* is the right "beginner" species — colonies visibly
   grow within the player's first few sessions. Already encoded
   (`growth.*_maturation_seconds`).

2. **Polygyne founding skip.** `founding = "polygyne"` should let the new-game
   path place multiple queens + workers together in one tube, bypassing the
   slow single-queen claustral founding phase. This is the biological basis for
   the species' "beginner difficulty" tag.

3. **Polydomy / nest relocation as gameplay mechanic.** Mid-term feature
   opportunity: in Keeper mode, allow the player to place a second formicarium
   module and have the colony **voluntarily migrate** queen + brood + workforce
   over ~24 sim-hours when the new module is more attractive (better climate,
   closer to food, less crowded). This is the *single most distinctive*
   *T. sessile* behavior and would differentiate the species visibly from
   *Lasius* / *Pogonomyrmex* / *Camponotus*. Reuses the existing brood-carry
   movement code; adds a "abandon old nest" trigger.

4. **Forest-vs-urban scale toggle.** A scenario / sandbox option:
   - *Forest mode* — single queen, ~100-worker target, monogynous.
   - *Urban mode* — multi-queen, supercolony-scale (10,000+ workers), polygynous,
     budding-only reproduction.
   Same species TOML, different lifecycle gating. This is the cleanest sim-side
   expression of the Buczkowski-vs-Smith literature divergence.

5. **Methylheptenone as alarm pheromone.** The existing `Alarm` pheromone layer
   in `PheromoneGrid` already covers the mechanic. Cosmetic / encyclopedia
   touch: the alarm puff visualization could be tinted distinctly for *T.
   sessile* to evoke the "rotten coconut" trope in the in-game encyclopedia.

6. **Aphid tending (Phase 6+ feature).** The species is the natural showcase for
   any future hemipteran-tending mini-system: place an aphid colony on a plant
   tile, and *T. sessile* foragers convert it into a sustained honeydew tap
   (renewable carbohydrate source), defending it against introduced predators.

7. **Climate-skip in indoor environments.** If the K3 climate / season system is
   active, *T. sessile* colonies inside heated player-built modules should
   **not** transition into `AntState::Diapause` even when outside temperature
   crosses the cold threshold. Add a per-module `heated: bool` flag the species
   diapause gate respects.

Tech-unlock parking lot (PvP): supercolony queen-tolerance, aphid tending, and
nest relocation are all candidates for the gating system in `biology.md` §
"Tech Unlocks for PvP". Default-on in Keeper mode.

---

## 10. Sources

Primary literature:

- **Say, T.** (1836). *Descriptions of new species of North American Hymenoptera, and observations on some already described.* Boston Journal of Natural History 1: 209–305. [Original species description.]
- **Smith, M. R.** (1928). *The biology of Tapinoma sessile Say, an important house-infesting ant.* Annals of the Entomological Society of America 21: 307–330. [Foundational natural-history account; the citation antedates most modern numbers but is still standard reference for life-history baselines.]
- **Buczkowski, G., & Bennett, G. W.** (2006/2008). *Dispersed central-place foraging in the polydomous odorous house ant, Tapinoma sessile, as revealed by a protein marker.* [Insectes Sociaux 53: 282–290](https://link.springer.com/article/10.1007/s00040-006-0870-0).
- **Buczkowski, G.** (2008). *Seasonal polydomy in a polygynous supercolony of the odorous house ant, Tapinoma sessile.* [Ecological Entomology 33: 780–788](https://resjournals.onlinelibrary.wiley.com/doi/abs/10.1111/j.1365-2311.2008.01034.x).
- **Buczkowski, G., Wang, S., & Craig, B. A.** (2023). *Behavioral assays reveal mechanisms of supercolony formation in odorous house ants.* [Scientific Reports 13: 9013](https://pmc.ncbi.nlm.nih.gov/articles/PMC10238414/) ([DOI](https://doi.org/10.1038/s41598-023-35654-y)).
- **Menke, S. B., Booth, W., Dunn, R. R., Schal, C., Vargo, E. L., & Silverman, J.** (2021). *Consistent signatures of urban adaptation in a native, urban invader ant Tapinoma sessile.* [Molecular Ecology 30: 6055–6069](https://onlinelibrary.wiley.com/doi/abs/10.1111/mec.16188).
- **Tomalski, M. D., Blum, M. S., Jones, T. H., Fales, H. M., Howard, D. F., & Passera, L.** (1987). *Chemistry and functions of exocrine secretions of the ants Tapinoma melanocephalum and T. erraticum.* J. Chem. Ecol. 13: 253–263. [Anal-gland methylheptenone chemistry; *T. sessile* not directly assayed but congener data is the standard reference.]
- **Schmidt, J. O., Blum, M. S., & Overal, W. L.** (2014). *Venoms and Venom Apparatuses of the Formicidae: Dolichoderinae and Aneuretinae.* In: *Hymenoptera and Their Venoms*, Springer. [Review of the dolichoderine anal gland system.]

Reference works:

- **AntWiki — *Tapinoma sessile***. <https://www.antwiki.org/wiki/Tapinoma_sessile>. [Living taxonomic / biological compendium; primary online reference for distribution, morphology, lifecycle numbers used in this entry.]
- **Coovert, G. A.** (2005). *The Ants of Ohio (Hymenoptera: Formicidae).* Bulletin of the Ohio Biological Survey, New Series 15(2): 1–207. [Regional Nearctic reference; treats *T. sessile* as one of Ohio's most ecologically catholic native ants.]
- **Wikipedia — *Tapinoma sessile***. <https://en.wikipedia.org/wiki/Tapinoma_sessile>. [Useful tertiary summary; numerical claims here cross-checked against AntWiki and primary literature.]
- **WSU Extension — *Odorous House Ant (Tapinoma sessile Say).*** <https://pubs.extension.wsu.edu/product/odorous-house-ant/>. [Applied / pest-management perspective.]

Cross-references inside this repo:

- `assets/species/tapinoma_sessile.toml` — the numerical species record this entry annotates.
- `docs/biology.md` — mechanism-level explanations referenced throughout (queen
  food regulation §1, brood cannibalism §1, trophic eggs §1, claustral founding
  §1, diapause §3, excavation §4).

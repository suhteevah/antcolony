# *Tetramorium immigrans* Santschi, 1927 — Immigrant Pavement Ant

**Status in sim.** Beginner-difficulty species, monogyne, semi-claustral, no obligate hibernation. TOML: `assets/species/tetramorium_immigrans.toml`. Cross-references to colony-level mechanisms in `docs/biology.md`.

---

## 1. Identity and taxonomic history

*Tetramorium immigrans* is a small, dark, omnivorous myrmicine ant native to the Pontomediterranean region of southern Europe and now a near-cosmopolitan synanthrope of temperate-zone cities. For most of the twentieth century the species was taxonomically invisible: North American populations were treated as conspecific with the European *T. caespitum* (Linnaeus, 1758), and a sprawling literature on "*T. caespitum*" in Ohio, Pennsylvania, and Ontario actually concerns the species now recognised as *T. immigrans*.

The split was formalised by Wagner et al. (2017), "Light at the end of the tunnel: integrative taxonomy delimits cryptic species in the *Tetramorium caespitum* complex," *Myrmecological News* 25: 95–129. Using 1,428 nest samples across mitochondrial DNA (COI), nuclear markers, geometric morphometrics of workers and males, and ecological niche data, Wagner and colleagues resolved ten cryptic European species in the complex and assigned the introduced North American pavement ant the name *T. immigrans* Santschi, 1927 — a name previously buried in synonymy. Earlier provisional designations (notably "species E" of Schlick-Steiner et al. 2006) map onto *T. immigrans*. **Practical consequence for readers of older literature: any pre-2017 paper referring to "*T. caespitum*" in North America almost certainly studied *T. immigrans*, and quantitative claims should be re-attributed accordingly.** Schlick-Steiner et al. (2006) remains the foundational integrative-taxonomy paper for the complex; Cordonnier et al. (2019) revisit the urban ecology of the split species.

In North America the species has been established since at least the early 1800s, almost certainly arriving as ballast-soil stowaways in trans-Atlantic shipping (Wetterer & Schultz 1999). It now occurs in 39 of the contiguous U.S. states and three Canadian provinces, with the densest populations along sidewalks, foundations, curb-stones, and the cracks of any sufficiently warm hardscape (MacGown — AntWiki 2024; OSU BYGL 2021).

## 2. Morphology

Workers are monomorphic, 2.5–4.0 mm in length (modal ≈ 2.8–3.2 mm; UF/IFAS EENY-600), uniformly dark brown to nearly black, with appendages slightly paler. Diagnostic features (Wagner et al. 2017; UF/IFAS):

- **Sculpture.** Head and mesosoma covered in fine, parallel, longitudinal *rugae* — the distinguishing "fingerprint" texture of *Tetramorium*.
- **Antennae.** 12-segmented with a 3-segmented club; raised cuticular ridge between the antennal insertions and clypeus.
- **Waist.** Two-segmented (petiole + postpetiole), both nodes boxy and roughly equal in size, postpetiole slightly larger and glossier than petiole.
- **Propodeum.** A single pair of short propodeal spines (vs. the longer, more divergent spines of the introduced *T. tsushimae*).
- **Sting.** Functional but small; the apex bears the triangular "flag" structure characteristic of *Tetramorium*, used more for pheromone painting during mass recruitment than for envenomation.

Alates are roughly twice the length of workers; males are smaller, slimmer, and conspicuously dark. Separation from sister species in the complex (*T. caespitum* s. str., *T. alpestre*, *T. impurum*) requires either male genitalic dissection or molecular work — workers alone cannot be reliably keyed without geometric morphometrics (Wagner et al. 2017; Csősz et al. 2018).

## 3. Colony lifecycle

Colonies are monogyne (a single inseminated queen) in the great majority of well-studied populations, with rare polygyne nests reported (King & Phillips 2019). Mature colonies contain 3,000 to >10,000 workers; some long-established urban supercolonies exceed 30,000 (Bantam.earth 2023; AntWiki 2024). Surface area of a mature nest is typically 1.2–4.8 m², and the defended foraging territory averages ~43 m² around the entrance (UF/IFAS EENY-600).

Nuptial flights occur in mid-June through early July across the U.S. Midwest and Northeast, often timed to the first warm humid evening following rain. The flights are of considerable size and visibility — and crucially, they coincide seasonally with the spring/early-summer **territorial battles** (Section 7) that earned the species its popular reputation. Founding is **semi-claustral**: queens shed wings, excavate a small chamber under a stone or pavement crack, and may emerge briefly to forage during the founding period rather than relying solely on metabolised wing musculature (cf. fully-claustral *Lasius niger*; see `biology.md`, "Claustral vs semi-claustral founding"). Colony lifespan is dictated by queen longevity; queens are reported to live up to ~15 years in captivity, though robust field longevity data remain unpublished (UF/IFAS notes longevity as "unknown").

## 4. Caste, development, and brood economy

The species is strictly **monomorphic**: no soldier caste exists. Castes are limited to workers, gynes (future queens), and males. Egg → larva → pupa development at 21–24 °C requires 42–63 days total (UF/IFAS EENY-600), with the pupal stage naked (no cocoon) — a *Tetramorium* trait that distinguishes brood piles visually from the silken cocoons of *Formica* or *Camponotus*. Worker lifespan in captivity is on the order of 12–18 months.

Trophic-egg production and survival cannibalism follow the general myrmicine pattern documented in `biology.md` — the queen's lay rate tracks recent protein inflow via vitellogenin throughput (Mankin et al. 2022), and brood is preferentially cannibalised under shortfall (Czaczkes et al. 2024). The species' hardiness in captivity is partly attributable to robust filial-cannibalism response: starved colonies recycle brood efficiently rather than losing adult workers.

## 5. Foraging and diet

*T. immigrans* is a thoroughgoing omnivorous opportunist. Documented food items include arthropod prey (live and scavenged), hemipteran honeydew (the species actively tends aphids and mealybugs), seeds, and a comprehensive list of human refuse — bread, cheese, meats, ice-cream, pet kibble, fruit (Wikipedia 2025; AntWiki 2024). The characteristic foraging signature is **mass recruitment via a strong food-trail pheromone**, painted with the modified sting; recruitment is rapid (a discovered crumb attracts dozens of recruits within minutes) and scales nearly linearly with food size, a textbook ACO-style stigmergic system. Forager activity extends well past sunset on warm evenings because pavement and foundation stones retain heat hours into the night — a measurable urban-heat-island effect on activity windows (Diamond et al. 2018, "Evolution of thermal tolerance and its fitness consequences: parallel and non-parallel responses to urban heat islands across three cities," *Proc. R. Soc. B* 285).

## 6. Nest architecture

Nests are characteristically **shallow**: most chambers lie within 0.45–0.90 m of the surface (UF/IFAS EENY-600), exploiting the thermal mass and rain-shedding of hardscape above. Construction is opportunistic — under pavement slabs, curb-stones, foundation seams, the soil column beside building footings, or in unmown turf in the absence of pavement. The diagnostic above-ground feature is a small **crater of excavated soil pellets** ringing the nest entrance — the kickout-mound pattern described generally in `biology.md` ("Excavation & nest architecture"). Substrate flexibility is high: sand, loam, and clay are all colonised, with the species' small worker size translating to rapid per-pellet excavation rates relative to body mass (cf. Sudd 1969).

## 7. Defense and the pavement-ant wars

The species is famous for **conspecific territorial battles** — the "sidewalk wars" of late spring through early summer, in which thousands of workers from adjacent colonies converge along a contested boundary and grapple in a writhing dark mass that can span a metre or more of pavement. The behaviour is well documented but only recently mechanistically dissected.

Plowes & Adler ("A mechanistic model of ant battles and its consequences for territory scaling," *American Naturalist*) and the Greene-lab series (Hoover, Bubak, Law, Yaeger, Renner, Swallow & Greene 2016, "The organization of societal conflicts by pavement ants *Tetramorium caespitum* [= *immigrans*]: an agent-based model of amine-mediated decision making," *Current Zoology*) provide the experimental backbone. Key findings:

- **Trigger.** Encounters between non-nestmates during foraging trigger antennation and cuticular-hydrocarbon assessment. A single hostile encounter spikes brain serotonin (5-HT, ~10.5 pg/µg protein) and octopamine (~8.2 pg/µg protein) to physiological maxima within seconds; concentrations decay linearly back to baseline over ~3 minutes — a "monoamine clock" priming the worker for further aggression during the decay window (Hoover et al. 2016).
- **Recruitment.** A subset of workers do *not* fight; instead they break contact and recruit additional nestmates via trail and tactile signalling. Positive-feedback recruitment escalates participation from dozens to thousands within an hour; the agent-based model predicts ≥91% engagement of available workers after 60 minutes.
- **Combat style.** Conspecific battles are largely **ritualised** — paired workers lock mandibles and grasp with legs, pushing against one another for hours. **Mortality is low relative to spectacle.** Plowes' field observations of "fights last for many hours during which few, if any, ants die" are widely confirmed by amateur and academic observers; the visible piles of corpses on sidewalks build up over days, not within a single battle. By contrast, **raids** (one colony entering another's nest) escalate to true mortal combat, with mandible-disarticulated bodies accumulating quickly.
- **Outcome scaling.** Larger colonies win disproportionately — territorial gains scale super-linearly with worker-force ratio (Plowes & Adler), an example of Lanchester's square law operating at the colony level.

Defence against heterospecifics and vertebrates is less remarkable: the small sting is barely felt by humans, and the species relies on numbers rather than individual venom potency.

## 8. Climate and seasonality

*T. immigrans* is temperate-zone but unusually flexible. Hibernation is **facultative, not obligate** — a critical distinction from most temperate Holarctic ants. Urban populations nesting against heated building foundations frequently maintain year-round brood production, while exurban populations enter a shallow winter quiescence. Diapause-onset behaviour follows the general pattern in `biology.md` ("Diapause biology") — autumn retreat to interior chambers, metabolic depression to ~10% of summer rates, and brood pause — but the trigger threshold is lower and the duration shorter than in *Lasius niger* or *Pogonomyrmex*. Urban heat-island effects measurably extend the active season at both ends of the year (Diamond et al. 2018; Menke et al. 2011 on urban-rural ant assemblages).

## 9. Simulation implications

Concrete behaviours the sim should reflect, mapped to existing systems and TOML parameters:

1. **Small, fast recruiters.** Worker speed is at the low end of the range (TOML `speed_multiplier = 0.95`) but the recruitment-trail strength should be unusually high — tune `deposit_food_trail` upward by ~25% relative to the *Lasius* baseline. Mass-recruitment dynamics should be visibly stronger and more decisive than for cryptic foragers.
2. **No soldier caste.** TOML correctly sets `soldier = 0.0` and `polymorphic = false`. Combat is a numbers game; all-worker armies.
3. **Territorial battles as a versus-mode hook.** The sidewalk-war mechanic — adjacent colonies converge on a boundary, recruit additively, fight ritualised low-mortality engagements — is a near-perfect fit for the planned PvP mode (`biology.md`, "Tech Unlocks for PvP"). Implement as a boundary-detection event that triggers high alarm-pheromone deposition along the contact line; a "ritual battle" mode where worker damage per tick is reduced ~5× relative to predator combat; and a "raid" escalation triggered when one side's force ratio exceeds a threshold.
4. **Monoamine clock.** The 3-minute aggression-decay window from Hoover et al. (2016) maps cleanly onto a per-ant `aggression_timer` field decremented each tick; while non-zero, the worker preferentially attacks any non-nestmate in sense range. Cheap, biologically grounded, and produces the characteristic "wave of fighting that propagates and then dissipates."
5. **Urban substrate flexibility.** No special soil restriction — set `substrate` (when added per `biology.md` excavation notes) to accept Sand / Loam / YTong equally. Shallow-chamber bias: the dig-pipeline cost function should weight the top 0.4–0.9 m of the underground module favourably for new chamber siting.
6. **Facultative diapause.** Override the global `hibernation_required = true` default — *T. immigrans* should enter shallow diapause only when ambient temperature stays below the cold-threshold for an extended period, *not* by calendar. Already encoded as `hibernation_required = false` in the TOML; the sim's diapause-onset logic must respect this flag.
7. **Lanchester scaling in PvP.** When two colonies engage, expected territory gain per tick should scale with `(force_ratio)^k` where `k > 1` (Plowes' super-linear result). A `k ≈ 1.4–1.7` reproduces the empirical pattern.

## 10. Sources

- **Species delimitation (primary).** Wagner, H.C., Arthofer, W., Seifert, B., Muster, C., Steiner, F.M. & Schlick-Steiner, B.C. (2017). *Light at the end of the tunnel: Integrative taxonomy delimits cryptic species in the* Tetramorium caespitum *complex (Hymenoptera: Formicidae).* Myrmecological News 25: 95–129. [ResearchGate](https://www.researchgate.net/publication/322663055)
- **Earlier complex revision.** Schlick-Steiner, B.C., Steiner, F.M., Moder, K., Seifert, B., Sanetra, M., Dyreson, E., Stauffer, C. & Christian, E. (2006). *A multidisciplinary approach reveals cryptic diversity in Western Palearctic* Tetramorium *ants. Mol. Phylogenet. Evol.* 40: 259–273.
- **Battle behaviour — mechanism.** Hoover, K.M., Bubak, A.N., Law, I.J., Yaeger, J.D.W., Renner, K.J., Swallow, J.G. & Greene, M.J. (2016). *The organization of societal conflicts by pavement ants* Tetramorium caespitum [=immigrans]*: an agent-based model of amine-mediated decision making.* Current Zoology 62(3). [PMC5829439](https://pmc.ncbi.nlm.nih.gov/articles/PMC5829439/)
- **Battle behaviour — ecology.** Plowes, N.J.R. & Adler, F.R. — *A mechanistic model of ant battles and its consequences for territory scaling.* American Naturalist (Plowes-lab series on *Tetramorium* battle organisation; see also dissertation Plowes 2008, ASU). [AmNat preview](https://www.amnat.org/an/newpapers/AugAdler.html)
- **North American natural history.** Calibeo, D. & Oi, F. (2014, rev. 2024). *Immigrant Pavement Ant,* Tetramorium immigrans *Santschi (Insecta: Hymenoptera: Formicidae).* UF/IFAS EENY-600/IN1047. [EDIS](https://edis.ifas.ufl.edu/publication/IN1047)
- **AntWiki species page.** [Tetramorium immigrans — AntWiki](https://www.antwiki.org/wiki/Tetramorium_immigrans).
- **OSU outreach summary of pavement-ant wars.** Boggs, J. (2021). *Immigrant Pavement Ant.* Ohio State University BYGL. [bygl.osu.edu/node/2389](https://bygl.osu.edu/node/2389); also *Ant Wars* [bygl.osu.edu/node/1578](https://bygl.osu.edu/node/1578).
- **Urban thermal ecology.** Diamond, S.E., Chick, L.D., Perez, A., Strickler, S.A. & Martin, R.A. (2018). *Evolution of thermal tolerance and its fitness consequences: parallel and non-parallel responses to urban heat islands across three cities.* Proc. R. Soc. B 285: 20180036.
- **Introduction history.** Wetterer, J.K. & Schultz, T.R. (1999). Discussion of *Tetramorium* introductions to North America (cited in MacGown, AntWiki).
- **Cross-references in this repository.** `docs/biology.md` — sections on claustral vs semi-claustral founding, vitellogenin-throttled lay rate, brood cannibalism, kickout mounds, diapause metabolic depression. `assets/species/tetramorium_immigrans.toml` — concrete numeric parameters used by the sim.

*Caveat on older sources:* literature published before Wagner et al. 2017 referring to "*T. caespitum*" in the Americas describes *T. immigrans*. European pre-2017 "*T. caespitum*" papers may refer to any of the ten species in the complex and require species-level re-evaluation before their numbers are imported into this simulation.

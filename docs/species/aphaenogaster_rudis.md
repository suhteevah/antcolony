# *Aphaenogaster rudis* — Eastern Woodland Winnow Ant

> Encyclopedia entry for the ant colony simulation. Companion to `assets/species/aphaenogaster_rudis.toml` and the mechanism log in `docs/biology.md`. Style: PhD-level natural history, every quantitative claim sourced.

---

## 1. Identity and Taxonomy — the "rudis Complex"

*Aphaenogaster rudis* Enzmann, 1947 is a myrmicine ant (Formicidae: Myrmicinae: Stenammini) and the nominal member of what working myrmecologists call the **rudis-group** or **rudis complex** — a cluster of closely related, morphologically near-identical species whose boundaries have been argued over for nearly a century. As currently parsed, the complex includes at minimum *A. rudis* sensu stricto, *A. picea* (Wheeler 1908), *A. fulva* Roger 1863, *A. carolinensis* Wheeler 1915, and *A. miamiana* Wheeler 1932; multiple additional cryptic lineages remain undescribed ([AntWiki — *A. rudis*](https://www.antwiki.org/wiki/Aphaenogaster_rudis); [AntWiki key](https://www.antwiki.org/wiki/Key_to_US_Aphaenogaster_species)).

The taxonomic friction is real and worth flagging. Field workers routinely report "*A. rudis*" from the entire eastern deciduous biome, but molecular work using the CAD intron and other markers shows that light-to-medium-brown forms lacking the CAD intron, distributed from North Carolina to Mississippi, are better assigned to *A. carolinensis*, while medium-to-dark-brown CAD-positive forms (Georgia to Massachusetts, west to Minnesota) are *A. rudis* s. str. ([Lubertazzi 2012, *Psyche*](https://onlinelibrary.wiley.com/doi/10.1155/2012/752815); [DeMarco & Cognato 2015 in Bernstein et al. 2022, *Insectes Sociaux*](https://link.springer.com/article/10.1007/s00040-022-00865-5)). For this simulation we treat the species as the broad complex unless and until a downstream feature requires finer resolution — most published ecology before ~2010 cannot be cleanly assigned to a single member.

**Range.** Eastern North American deciduous forest, southern Ontario through the Appalachians, west to roughly the Mississippi River, south into the Piedmont; *A. rudis* s. str. occupies the southern portion of that range, with *A. picea* taking over at higher latitudes and elevations ([Lubertazzi 2012](https://onlinelibrary.wiley.com/doi/10.1155/2012/752815)). It is a forest-floor specialist of mature, mesic, leaf-littered hardwood stands.

## 2. Morphology

Workers are **monomorphic**, slender, 3.5–5 mm in total length, and a translucent red- to chocolate-brown ([AntWiki](https://www.antwiki.org/wiki/Aphaenogaster_rudis)). The propodeum bears the **long, paired propodeal spines** that are diagnostic of the genus — together with a long, narrow petiole and postpetiole and the distinctively elongated head, these give *Aphaenogaster* its informal name "gypsy ants" ([Mackay & Mackay, *The New World Gypsy Ants*](https://www.academia.edu/69430270/THE_NEW_WORLD_GYPSY_ANTS_OF_THE_GENERA_APHAENOGASTER_AND_NOVOMESSOR_HYMENOPTERA_FORMCIDAE)). The TOML's `size_mm = 4.5` and `color_hex = "#6b3a22"` sit squarely in the documented worker range.

## 3. Colony Lifecycle

*A. rudis* is **monogyne** (single egg-laying queen) and **fully claustral** during founding — matching `founding = "claustral"` in the TOML and the mechanism described under "Claustral vs semi-claustral founding" in `biology.md`. Nuptial flights occur from late summer into early autumn and vary by species in the complex: *A. rudis* s. str. typically flies in August–September, *A. picea* somewhat earlier ([Lubertazzi 2012](https://onlinelibrary.wiley.com/doi/10.1155/2012/752815)).

Mature colonies are **modest by ant standards**. Surveyed populations in Connecticut and Ohio average **266–613 workers per nest**, with rare nests reaching ~2,000 ([Lubertazzi 2012](https://onlinelibrary.wiley.com/doi/10.1155/2012/752815); [Skidmore NorthWoods notes](https://academics.skidmore.edu/wikis/NorthWoods/index.php/Aphaenogaster_rudis)). The simulation's `target_population = 800` is on the high end of typical but well within the empirical envelope.

**Queen lifespan** is exceptional for an ant of this size: Haskins's laboratory cohort of 11 queens had a median lifespan of **8 years, maximum 13** ([cited in Lubertazzi 2012](https://onlinelibrary.wiley.com/doi/10.1155/2012/752815)). The TOML's `queen_lifespan_years = 12.0` is plausible for a long-lived individual but above the median — flagged as a deliberate game-pacing decision rather than a strict typical.

## 4. Caste and Development

Brood passes through egg → larva → pupa as in all holometabolous Hymenoptera. *Aphaenogaster* species in this complex are **monomorphic** — there is no distinct soldier caste — and the TOML correctly carries `polymorphic = false`, `soldier = 0.0`. Worker lifespan is poorly constrained in the wild but laboratory observations suggest roughly one season to a year for workers in temperate *Aphaenogaster*; the TOML's `worker_lifespan_months = 12.0` is at the upper bound. Brood cannibalism under nutrient stress is the standard Myrmicine pattern (see "Survival cannibalism of brood is normal" in `biology.md`); no rudis-specific exception is reported.

## 5. Foraging and Diet — the Defining Trait

This is where *A. rudis* earns its disproportionate ecological reputation. **Aphaenogaster of the rudis complex are the single most important seed-dispersing ants in eastern North American deciduous forests.** Approximately **30–40 % of understory herbaceous plant species** in this biome bear **elaiosomes** — lipid-and-protein-rich appendages whose chemistry mimics insect cuticle and triggers the ants' prey-retrieval response ([Beattie 1985, *The Evolutionary Ecology of Ant–Plant Mutualisms*; reviewed in Ness, Bronstein et al., Cambridge *Ant–Plant Interactions* Ch. 5](https://www.cambridge.org/core/books/abs/antplant-interactions/global-change-impacts-on-antmediated-seed-dispersal-in-eastern-north-american-forests/52408411902E8F6134BDE64CE0A6B074); [Rutgers Ecological Preserve summary](https://ecopreserve.rutgers.edu/2020/09/01/2254/)).

The interaction — **myrmecochory** — works as follows. A forager encounters an elaiosome-bearing seed (*Trillium grandiflorum*, *Sanguinaria canadensis*, *Asarum canadense*, *Hexastylis arifolia*, *Anemone acutiloba*, *Viola* spp., *Dicentra*, *Jeffersonia*, and dozens more), grasps it by the elaiosome, and carries it to the nest. The elaiosome is removed and fed to brood; the intact, viable seed is discarded into the nest midden, a site enriched in organic matter and protected from rodent seed predators. This single behavior structures the spring-ephemeral wildflower distribution of the eastern hardwood biome ([Ness et al., reviewed in Bernstein et al. 2022](https://link.springer.com/article/10.1007/s00040-022-00865-5); [Insectes Sociaux blog summary](https://insectessociaux.com/2022/09/05/more-than-meets-the-eye-hidden-variation-affects-how-ants-plant-seeds-of-forest-wildflowers/)).

Foraging is otherwise generalist — small live and dead arthropods, extrafloral nectar, and occasional sugary exudates ([Clark & King 2012, on *A. picea*](https://sciences.ucf.edu/biology/king/wp-content/uploads/sites/14/2011/08/Clark-and-King-2012.pdf)). Workers exhibit **diaspore satiation**: a colony's seed-collection rate saturates at moderate seed densities, an important constraint for ecological models ([Mitchell et al. 2002, *J. Insect Behav.*](https://link.springer.com/article/10.1007/s10905-005-8743-3)). The TOML's `prefers = ["seeds", "protein", "insects", "sugar"]` captures the dietary breadth correctly; seeds belong at the head of the list.

## 6. Nest Architecture

Nests are **shallow, simple, and substrate-cued** — under decomposing logs, beneath rocks, in compacted leaf litter, or in soft soil under a small entrance hidden by litter. Summer nests are typically **less than 15 cm deep** with a single small circular entrance and a tightly clustered central chamber containing queen, brood, and most idle workers within ~20 cm of the queen ([Lubertazzi 2012](https://onlinelibrary.wiley.com/doi/10.1155/2012/752815)). Nest density in mature eastern hardwoods runs ~**0.5 nests/m²** in both Ohio and Connecticut surveys (ibid.). Decaying wood is preferred substrate when available — the species is functionally a saproxylic forest-floor specialist. This is consistent with the `biology.md` "Substrate type changes everything" entry: future sim work could differentiate `Loam` and `RottingWood` substrate types, with *A. rudis* preferring the latter.

## 7. Defense and Combat

Workers possess a small, functional sting but are **conspicuously non-aggressive**. The species' default response to disturbance is to flee into litter or scatter brood deeper into the nest ([AntWiki](https://www.antwiki.org/wiki/Aphaenogaster_rudis)). The TOML's `aggression = 0.25` and `worker_attack = 1.1` reflect this correctly — these are not combat ants.

This timidity carries an ecological cost. *A. rudis* is being **measurably displaced** by two invasive ants:

- **Asian needle ant, *Brachyponera chinensis*.** In paired plots, *B. chinensis* presence is associated with a **96 % reduction in *A. rudis* abundance, a 70 % reduction in seed removal, and a 50 % reduction in the focal myrmecochore *Hexastylis arifolia*** ([Rodriguez-Cabal et al. 2012; reviewed in Warren et al., Ecosphere & PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC11739460/); [Bednar & Silverman; Spicer Rice et al. summarized in Springer 2015](https://link.springer.com/article/10.1007/s10530-015-0942-z)).
- **Red imported fire ant, *Solenopsis invicta*.** Documented displacement at the southern edge of the rudis range, with similar collapse of seed-dispersal mutualisms.

Crucially, neither invader replaces *A. rudis* as a seed disperser — *B. chinensis* in particular is a poor mutualist, so its arrival decouples the wildflower–ant link that has structured these forests since the Pleistocene ([Warren et al., Ecosphere](https://esajournals.onlinelibrary.wiley.com/doi/10.1002/ecs2.2547)).

## 8. Climate and Hibernation

*A. rudis* is a temperate-deciduous species and **obligately hibernates**: workers cluster in the deepest chamber of the nest from roughly November through March, with `hibernation_required = true` and `min_diapause_days = 60` in the TOML matching the field record. The mechanisms in `biology.md` (body-fat dependence, ~10× metabolic depression, autumn retreat) all apply.

A subtle but consequential trait: *A. rudis* is **active at lower ambient temperatures than most sympatric ants**, with foraging detectable down to ~5 °C and sustained activity from early spring well before competitors emerge ([Warren et al., on upward range shifts driven by minimum-temperature tolerance, *Global Change Biology* 2013](https://onlinelibrary.wiley.com/doi/abs/10.1111/gcb.12169)). This early-spring window is precisely when ant-dispersed wildflowers (trillium, bloodroot, hepatica) set seed — the phenological match is not accidental but co-evolved.

## 9. Sim Implications

- **Myrmecochory as a unique mechanic.** Seeds should be a distinct food class: a `FoodKind::ElaiosomeSeed` that workers prioritize even over equivalent-calorie protein, that yields modest food on retrieval (the elaiosome only — perhaps 10–20 % of the seed's mass), and that **persists in the midden** as a non-consumable object. Discarded seeds in the midden could grow into *plant entities* that produce more seeds at the next seasonal tick — a closed mutualistic loop, and a uniquely *Aphaenogaster* gameplay verb.
- **Substrate preference.** Tag *A. rudis* as preferring `RottingWood`/`LeafLitter` substrates when the substrate model from `biology.md` lands; nest excavation rate should be high in these substrates and lower in mineral soil.
- **Cold-tolerance edge.** Lower the diapause-exit temperature for *A. rudis* relative to default temperate species. In a multi-species map, this gives the species a real-spring head start on food, modeling the wildflower-dispersal phenology directly.
- **Invasive-ant hazard hook.** A future Phase 6 hazard slot for *Brachyponera chinensis* / *Solenopsis invicta* would interact strongly with this species' low `aggression = 0.25` — a built-in narrative beat about ecological displacement.
- **Small mature population.** `target_population = 800` is correct; the species should not snowball, and PvP balance should reflect that *A. rudis* trades raw colony size for an ecological-engineering verb that other species lack.

## 10. Sources

Primary literature and authoritative references used above:

- [Lubertazzi, D. (2012). *The Biology and Natural History of Aphaenogaster rudis.* **Psyche** 2012:752815.](https://onlinelibrary.wiley.com/doi/10.1155/2012/752815) — the single most useful synthesis; sociometry, lifespan, nest architecture, seed-dispersal, taxonomic complex.
- [Bernstein et al. (2022). *Uncovering how behavioral variation underlying mutualist partner quality is partitioned within a species complex of keystone seed-dispersing ants.* **Insectes Sociaux** 69.](https://link.springer.com/article/10.1007/s00040-022-00865-5) — current molecular taxonomy of the rudis complex.
- [AntWiki — *Aphaenogaster rudis*](https://www.antwiki.org/wiki/Aphaenogaster_rudis) and [Key to US *Aphaenogaster*](https://www.antwiki.org/wiki/Key_to_US_Aphaenogaster_species).
- Beattie, A. J. (1985). *The Evolutionary Ecology of Ant–Plant Mutualisms.* Cambridge University Press. (Foundational myrmecochory monograph; see also Beattie & Culver 1981, *Ecology*.)
- [Ness, J. H. & Bronstein, J. L., reviewed in *Ant–Plant Interactions* (Cambridge), Ch. 5: "Global change impacts on ant-mediated seed dispersal in eastern North American forests."](https://www.cambridge.org/core/books/abs/antplant-interactions/global-change-impacts-on-antmediated-seed-dispersal-in-eastern-north-american-forests/52408411902E8F6134BDE64CE0A6B074)
- [Warren, R. J. et al. *Species Distribution Models Reveal Varying Degrees of Refugia from the Invasive Asian Needle Ant.* PMC.](https://pmc.ncbi.nlm.nih.gov/articles/PMC11739460/)
- [Spicer Rice, E. et al. (2015). *Forest invader replaces predation but not dispersal services by a keystone species.* **Biological Invasions** 17.](https://link.springer.com/article/10.1007/s10530-015-0942-z)
- [Warren, R. J. & Chick, L. (2013). *Upward ant distribution shift corresponds with minimum, not maximum, temperature tolerance.* **Global Change Biology** 19.](https://onlinelibrary.wiley.com/doi/abs/10.1111/gcb.12169)
- [Mitchell, C. E. et al. *Satiation in Collection of Myrmecochorous Diaspores by Colonies of Aphaenogaster rudis.* **J. Insect Behav.** 18.](https://link.springer.com/article/10.1007/s10905-005-8743-3)
- [Clark, R. E. & King, J. R. (2012). *The ant, Aphaenogaster picea, benefits from plant elaiosomes.*](https://sciences.ucf.edu/biology/king/wp-content/uploads/sites/14/2011/08/Clark-and-King-2012.pdf)
- Coovert, G. A. (2005). *The Ants of Ohio (Hymenoptera: Formicidae).* Ohio Biological Survey. — regional faunistic treatment cited for nest density and habitat in the Midwest.
- [Mackay, W. P. & Mackay, E. E. *The New World Gypsy Ants of the Genera Aphaenogaster and Novomessor.*](https://www.academia.edu/69430270/THE_NEW_WORLD_GYPSY_ANTS_OF_THE_GENERA_APHAENOGASTER_AND_NOVOMESSOR_HYMENOPTERA_FORMCIDAE)

Cross-references in this repo: [`docs/biology.md`](../biology.md) sections on *Claustral founding*, *Survival cannibalism*, *Diapause biology* (body-fat dependence, metabolic depression, autumn retreat), and *Substrate type changes everything*. Species data block: [`assets/species/aphaenogaster_rudis.toml`](../../assets/species/aphaenogaster_rudis.toml).

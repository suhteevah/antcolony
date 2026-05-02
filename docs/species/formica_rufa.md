# *Formica rufa* — European Red Wood Ant

**Compiled:** 2026-05-02. PhD-level natural-history entry to ground the in-game species. Cross-references `docs/biology.md` for shared mechanisms and `assets/species/formica_rufa.toml` for the concrete sim numbers. Where the literature disagrees, both figures are cited.

---

## 1. Identity

*Formica rufa* Linnaeus, 1761 (Hymenoptera: Formicidae: Formicinae) is the type species of the genus *Formica* and the nominal member of the **Formica rufa species group** — a closely related complex of mound-building wood ants that includes *F. polyctena*, *F. pratensis*, *F. lugubris*, *F. paralugubris*, and *F. aquilonia*. The group is morphologically conservative; reliable separation often requires gular-pubescence counts and male genitalia, and field workers routinely collapse it as "*F. rufa* s.l." ([AntWiki — *Formica rufa*](https://www.antwiki.org/wiki/Formica_rufa); [Wikipedia — *Formica rufa* species group](https://en.wikipedia.org/wiki/Formica_rufa_species_group)).

**Range.** Palaearctic. Distributed across temperate and boreal Europe from the British Isles east into Siberia, with the strongest densities in coniferous and mixed conifer-broadleaf forests of central, northern, and montane Europe. Largely absent from lowland Mediterranean basin and from open steppe ([MDPI 2025 review of *F. rufa* group ecology](https://www.mdpi.com/2075-4450/16/5/518)).

**Conservation.** Mounds and the species are legally protected in **Germany** (Bundesnaturschutzgesetz Anlage 1), **Switzerland**, **Austria**, **Luxembourg**, and several Nordic states. Destroying or significantly disturbing a mound is a prosecutable offence in most of these jurisdictions; relocation requires a permit and a trained translocator. The legal status is keystone-driven — the ant is protected because it suppresses forest-pest Lepidoptera and supports avian and mammalian biodiversity around the mound ([National Geographic 2023 — red wood ants as forest superheroes](https://www.nationalgeographic.com/animals/article/european-red-wood-ants-impact)).

## 2. Morphology

Workers are **bicoloured red and black**: head dorsum and gaster matte black, mesosoma and the appendages dull rust-red to reddish-brown, with a characteristic dark patch on the pronotum/promesonotum. Body length **4.5–9 mm** with the largest individuals occasionally reaching ~10 mm ([AntWiki](https://www.antwiki.org/wiki/Formica_rufa); [AntKeepers — *F. rufa*](https://antkeepers.com/pages/formica-rufa-red-wood-ant)). The species is **monomorphic to weakly size-variable** — there is a continuous size cline among workers but **no discrete soldier caste**, in contrast with polymorphic genera such as *Camponotus* or *Pheidole*. The TOML reflects this with `polymorphic = false` and `soldier = 0.0`.

Queens are notably larger and stockier: **9–12 mm** depending on source, with a more brown-red mesosoma and darker gaster. Sources disagree on the upper bound — AntKeepers gives ~9 mm; Best Ants UK and several keeper care sheets cite up to 12 mm. The discrepancy likely reflects population-level size variation across the European range and possibly conflation with *F. polyctena* gynes. Males are smaller, fully winged, ephemeral, and present only during the mating season.

## 3. Colony Lifecycle

**Founding is obligately socially parasitic** on members of the *F. fusca* group (typically *Formica fusca* itself, sometimes *F. lemani* or *F. cunicularia*). Newly-mated *F. rufa* gynes have lost the capacity for independent (claustral) founding entirely. A queen locates a host nest, infiltrates it, kills or displaces the host queen, and the *F. fusca* workers raise her first cohort of *F. rufa* brood. Once *F. rufa* nanitics emerge in sufficient numbers, the host workforce dies off through normal attrition and the colony transitions to pure *F. rufa* ([Borowiec et al. 2021, *PNAS* — phylogeny of social parasitism in *Formica*](https://www.pnas.org/doi/10.1073/pnas.2026029118)). Cross-reference `biology.md` "Claustral vs semi-claustral founding" — the species TOML uses `founding = "parasitic"` precisely so future sim mechanics can branch on this distinct third pathway.

**Polygyny is the rule, not the exception.** Mature mounds frequently re-adopt post-nuptial daughter queens from the natal colony rather than dispersing them, producing **secondary polygyny** with **>100 egg-laying queens** in the largest nests (Borowiec et al. 2021; AntWiki). This re-adoption pathway is the proximate mechanism behind **polydomous supercolonies** — interconnected networks of mounds sharing workers and brood across hundreds of metres of forest, founded by budding rather than independent flight.

**Colony lifespan is decadal.** Individual mounds persist 20–80+ years; the queen turnover within polygyne supercolonies effectively makes the colony itself potentially immortal as long as the mound persists. Mature populations span **100,000 to 400,000 workers per single large mound**, with whole supercolony complexes documented at well over 10⁶ individuals (AntWiki; Cambridge — [*Wood Ant Ecology and Conservation*, Ch. 4](https://resolve.cambridge.org/core/books/abs/wood-ant-ecology-and-conservation/where-and-why-wood-ant-population-ecology/1F8560CF38EB9CEA59DE58D5591474CF)). The TOML's `target_population = 300000` sits squarely in the middle of this band.

## 4. Caste & Development

Brood progresses through **egg → larva → pupa (cocooned)** in 6–10 weeks under summer conditions; *Formica* pupae are notably enclosed in silken cocoons (the so-called "ant eggs" sold as fish food are in fact *Formica* pupae). Worker lifespan in the field is typically **1–3 years** (~2.5 years modal), substantially longer than most temperate ant workers and reflecting the species' extreme cold-hardiness and the deep, stable hibernation refuge inside the mound. Queens average **~10 years**, with credible reports up to **15+ years** in polygyne colonies (AntKeepers; AntWiki). The TOML's `queen_lifespan_years = 18.0` and `worker_lifespan_months = 30.0` are at the upper bound of credible field measurements — defensible for a sim where colony-scale persistence is desired but at the optimistic edge of the literature.

Within polygyne nests the per-queen egg-laying rate is **suppressed** by mutual queen-pheromone inhibition (see `biology.md` "Queen pheromones regulate worker foraging via negative feedback") — total colony fecundity scales sub-linearly with queen count. Sim models that simply multiply per-queen output by queen number will over-estimate output by an order of magnitude in large polygyne colonies.

## 5. Foraging & Diet

*Formica rufa* is a **mutualist-predator hybrid forager** with two strongly differentiated income streams:

1. **Aphid honeydew (the carbohydrate base).** Workers tend and defend dense **aphid herds** in the conifer canopy — primarily *Cinara* and *Lachnus* spp. on spruce and pine. Honeydew represents the dominant caloric input: a single mature mound consumes an estimated **~200 kg of honeydew per year** (Hölldobler & Wilson 1990, *The Ants*; reviewed in [PMC — ecological consequences of ant–honeydew interactions](https://pmc.ncbi.nlm.nih.gov/articles/PMC1685857/)).
2. **Predation on forest insects (the protein base).** A single large mound takes an estimated **~100,000 prey items per day** during peak summer activity — Lepidoptera larvae, sawfly larvae, beetles, dipterans (Hölldobler & Wilson 1990; MDPI 2025 review). This pest-suppression service is the basis for the species' legal protection.

Trail organization is **mass-recruitment-based** with persistent, well-trodden physical paths radiating from the mound up to **100+ m** to reliable food sources, particularly to favoured aphid-bearing trees. These trails are reinforced both chemically (recruitment trail pheromone) and mechanically (cleared substrate). Cross-reference `biology.md` "Excavation" — surface trails are the above-ground analogue of the chamber-pheromone gradients that drive underground architecture.

## 6. Nest Architecture — the Thatch Mound

The diagnostic feature: a **dome of conifer needles, twigs, resin, and plant fragments** assembled by workers above an underground stump-and-soil substructure. Domes routinely reach **1–2 m tall** and **2 m+ in diameter**, with old monumental nests in protected forest reserves exceeding **2 m height** and persisting for many decades. Coarser material (twigs, bark) sits in the interior near brood chambers; finer material (needles, resinous fragments) packs the dense outer thatch ([Frouz & Jílková 2008 *Myrmecological News*; AntWiki](https://www.antwiki.org/wiki/Formica_rufa)).

**Thermoregulation** is the single most studied aspect of the architecture and is achieved by three superimposed mechanisms (reviewed in [Kadochová & Frouz 2014, *F1000Research* — thermoregulation in the *F. rufa* group](https://pubmed.ncbi.nlm.nih.gov/24715967/)):

- **Solar gain via dome geometry.** Mounds are typically asymmetric, with the gentler slope facing south (or south-east in northern populations) to maximise insolation. Surface temperatures can rise **+12–17 °C above ambient within 12 h** of clear-sky exposure.
- **Metabolic heat from worker clustering.** Active worker aggregations in upper galleries generate measurable heat; densely populated mounds maintain elevated brood-chamber temperatures even on cool days.
- **Decomposition of organic thatch.** Microbial breakdown of the needle layer contributes a slow background heat source.

Crucially, **only mounds exceeding ~1.1 m diameter achieve true thermoregulatory homeostasis** — smaller nests track ambient too closely (Kadochová & Frouz 2014). Workers actively shuttle brood between depth strata across the day and across seasons (cf. `biology.md` "Chamber siting is functional, not random"). The underground component descends ~1–2 m below the mound base and houses the queen chamber and overwintering cluster.

## 7. Defense & Combat

*Formica rufa* is the **eponymous formic-acid sprayer** — formic acid was first isolated in 1671 by John Ray via destructive distillation of crushed *F. rufa* workers, and the molecule and the ant share the name. Workers can **eject formic acid in a fine spray several centimetres** from the gaster while simultaneously biting with their mandibles, creating an acid-into-wound delivery system. A disturbed mound emits a sharp vinegar-like odour detectable downwind. Aggression is high (`aggression = 0.9` in the TOML) and inter-colony raiding, particularly against host *F. fusca* nests during founding, is well documented.

Defense extends to the species' **aphid herds** — workers attack predatory ladybird larvae, lacewings, and parasitoid wasps that threaten the herd, in clear analogy to vertebrate pastoralism.

The acid is also applied to nest material as a **disinfectant**: workers chemically treat tree-collected resin with formic acid to enhance its antimicrobial activity ([Brütsch et al. 2017, *Ecology & Evolution* — wood ants produce a potent antimicrobial agent](https://pmc.ncbi.nlm.nih.gov/articles/PMC5383563/)).

## 8. Climate & Hibernation

A **temperate-to-boreal** species with deep cold-tolerance. Workers retreat into the lower chambers and underground galleries below the mound from approximately **October through April** (varies with latitude); the mound's thatched insulation and metabolic mass produce a stable winter refuge several degrees above the surrounding soil, and the colony enters genuine **diapause** rather than mere torpor. Cross-reference `biology.md` "Diapause Biology" — adults survive on body lipids, not stored colony food, and per-tick metabolic demand drops to ~5–10% of summer baseline. The TOML's `min_diapause_days = 150` (~5 months) and `hibernation_required = true` reflect this.

In autumn, **active thermoregulation switches off** — workers stop the heat-management behaviours that drive summer brood incubation and let mound temperature track ambient downward into the diapause range ([Kadochová & Frouz 2015 — switch-off of active thermoregulation in autumn](https://www.researchgate.net/publication/271797041)).

## 9. Sim Implications

Concrete hooks the simulation should expose for *F. rufa*:

- **Thatch-dome rendering.** A `Terrain::ThatchMound(volume, asymmetry_axis)` variant with a south-facing solar bias surface. Volume drives a thermoregulation lookup — only mounds above the 1.1 m equivalent threshold get the constant-temperature interior bonus. Compare `biology.md` "Kickout mound — the diagnostic visual" — the thatch is the above-ground sibling of the soil mound.
- **Polygyny scaling.** Queens count should be capable of >100; per-queen egg output must scale **sub-linearly** (`per_queen_rate = base / sqrt(queen_count)` is a defensible cheap approximation of the pheromone-inhibition mechanism in `biology.md`).
- **Aphid mutualism.** A `Resource::AphidHerd(tree_id, productivity)` node that produces a steady honeydew trickle and requires worker defence against `Predator::AphidPredator` entities. Loss of aphid herds should starve the colony of carbs, distinct from prey-protein supply.
- **Founding parasitism.** When `founding = "parasitic"`, the queen-spawn flow must place an *F. fusca* host nest first, transition workforce composition over time, and fail loudly if the host is unavailable. This is a distinct third path beyond claustral and semi-claustral founding.
- **Formic acid as ranged combat.** Worker attack should support a short-range **spray** primary in addition to melee bite, with area-of-effect damage on dense enemy clusters near the nest. Maps cleanly onto the alarm-pheromone steering already in the design.
- **Pest-suppression score.** In Keeper mode, the colony's prey-take-per-day could be exposed as an ecological score — players are running a forest service, not just a colony, which fits the species' real-world cultural framing.

## 10. Sources

Primary literature:

- **Gösswald, K. (1989).** *Die Waldameise: Biologische Grundlagen, Ökologie und Verhalten.* AULA-Verlag, Wiesbaden. The classical two-volume monograph; the ground truth for *F. rufa* biology, mound architecture, and historical European forestry use. Cited via the MDPI 2025 review where the German original is inaccessible.
- **Hölldobler, B. & Wilson, E. O. (1990).** *The Ants.* Harvard University Press / Belknap. Chapters on Formicinae trail recruitment, supercolonies, and aphid mutualism.
- **Borowiec, M. L., Cover, S. P., & Rabeling, C. (2021).** "The evolution of social parasitism in *Formica* ants revealed by a global phylogeny." *PNAS* 118(38). [10.1073/pnas.2026029118](https://www.pnas.org/doi/10.1073/pnas.2026029118).
- **Kadochová, Š. & Frouz, J. (2014).** "Thermoregulation strategies in ants in comparison to other social insects, with a focus on red wood ants (*Formica rufa* group)." *F1000Research*. [PubMed 24715967](https://pubmed.ncbi.nlm.nih.gov/24715967/).
- **Kadochová, Š. & Frouz, J. (2015).** "Red wood ants *Formica polyctena* switch off active thermoregulation of the nest in autumn." [ResearchGate 271797041](https://www.researchgate.net/publication/271797041).
- **Brütsch, T. et al. (2017).** "Wood ants produce a potent antimicrobial agent by applying formic acid on tree-collected resin." *Ecology & Evolution*. [PMC 5383563](https://pmc.ncbi.nlm.nih.gov/articles/PMC5383563/).
- **Stockan, J. A. & Robinson, E. J. H. eds. (2016).** *Wood Ant Ecology and Conservation.* Cambridge University Press. Modern review of population ecology, supercolony structure, and conservation status. [Cambridge Core](https://resolve.cambridge.org/core/books/abs/wood-ant-ecology-and-conservation/where-and-why-wood-ant-population-ecology/1F8560CF38EB9CEA59DE58D5591474CF).
- **Trigos-Peral, G. et al. (2025).** "The Role of Red Wood Ants (*Formica rufa* Species Group) in Central European Forest Ecosystems — A Literature Review." *Insects* 16(5):518. [MDPI](https://www.mdpi.com/2075-4450/16/5/518) / [PMC 12111979](https://pmc.ncbi.nlm.nih.gov/articles/PMC12111979/).

Reference databases & keeper sources:

- [AntWiki — *Formica rufa*](https://www.antwiki.org/wiki/Formica_rufa) — taxonomy, morphology, distribution map.
- [AntWiki — *Formica rufa* species group](https://en.wikipedia.org/wiki/Formica_rufa_species_group).
- [AntKeepers — *Formica rufa* care sheet](https://antkeepers.com/pages/formica-rufa-red-wood-ant) — keeper-scale lifespan and founding observations.
- [National Geographic (2023) — Why red wood ants are the forest's tiny but mighty superheroes](https://www.nationalgeographic.com/animals/article/european-red-wood-ants-impact) — popular summary of ecological role and protection status.

**Literature disagreements noted in this entry.** Worker upper size (9 vs 10 mm; AntWiki vs keeper sources). Queen size (9 vs 12 mm; AntKeepers vs Best Ants UK) — likely population-level variation and possibly cross-contamination with *F. polyctena* gynes. Queen lifespan (10 vs 15+ years; modal field vs polygyne colony reports). Per-mound prey take (the "100,000 insects per day" figure originates with Gösswald and is reproduced through Hölldobler & Wilson and the modern Insects review; primary measurement methodology is dated and the figure should be treated as order-of-magnitude). Honeydew tonnage (~200 kg/yr to "a quarter tonne" depending on source) — same order of magnitude.

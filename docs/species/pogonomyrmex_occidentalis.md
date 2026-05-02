# *Pogonomyrmex occidentalis* — Western Harvester Ant

A natural-history reference for the simulation. Cross-references `docs/biology.md` for general mechanisms; this file carries species-specific facts and the literature behind them. Concrete numbers used by the sim live in `assets/species/pogonomyrmex_occidentalis.toml` — when those disagree with the citations below, the citations are the ground truth and the TOML is a designer's compromise.

---

## 1. Identity

- **Order / Family / Subfamily.** Hymenoptera : Formicidae : Myrmicinae
- **Tribe.** Pogonomyrmecini
- **Genus.** *Pogonomyrmex* Mayr, 1868 — Greek *pōgōn* ("beard") + *myrmēx* ("ant"). The "beard" is the **psammophore**: a basket of long, curved hairs on the ventral surface of the head used to scoop and carry loose sand pellets during excavation ([AntWiki — *Pogonomyrmex*](https://www.antwiki.org/wiki/Pogonomyrmex); [Wild, *Myrmecos* — "The bearded ladies"](https://myrmecos.wordpress.com/2008/03/19/the-bearded-ladies/)).
- **Species.** *P. occidentalis* (Cresson, 1865).
- **Common names.** Western harvester ant; red harvester ant (shared loosely with *P. barbatus*).
- **Range.** Arid grassland, shortgrass prairie, sagebrush steppe, and Great Basin shrubland of western North America — Idaho, Wyoming, Montana, Colorado, Utah, Nevada, eastern Oregon, western Nebraska/Kansas, and northern Arizona/New Mexico ([Idaho Fish & Game species catalog](https://idfg.idaho.gov/species/taxa/1476529); [AntWiki — *P. occidentalis*](https://www.antwiki.org/wiki/Pogonomyrmex_occidentalis)).

The genus epithet is doing double work in the sim: the psammophore is *both* the morphological feature that justifies preferential sandy-substrate digging (§9) *and* the etymological tag that distinguishes this lineage from generic ants visually (a player who hovers a worker in the encyclopedia panel should learn the word).

---

## 2. Morphology

Workers are **5–7 mm** in total length, monomorphic in caste (no major/minor split — see §4) but visibly larger and more robust than typical Holarctic ants the player will have seen. Coloration is uniform red-orange to rust-brown; a darker gaster is common but variable. The head is square in dorsal view with prominent mandibles and the diagnostic **psammophore** beard underneath. Queens are 9–11 mm and noticeably bulkier; males are smaller, dark, and short-lived.

The psammophore is more than a curiosity — Pogonomyrmex workers can carry **sand pellets up to ~1.5 mm**, larger than the ~0.3–0.7 mm pellets typical of *Lasius niger* (see `biology.md` § *Soil pellets, not grains*; [Tschinkel 2004 on *P. badius*](https://www.jstor.org/stable/25086323)). This is what lets a colony excavate the deep vertical shafts of §6 in coarse desert substrate that would defeat a smaller-bodied ant.

There is **no soldier caste** — defense is performed by ordinary workers using the sting (§7). The species TOML correctly sets `soldier = 0.0` and routes all combat output through the worker stat block.

---

## 3. Colony lifecycle

**Nuptial flight.** Synchronized swarms of alates (winged reproductives) emerge from many colonies simultaneously **after the first heavy summer monsoon rains** that soften the soil enough for a new queen to dig her founding chamber. Timing varies by latitude — late June to early August across most of the range. Synchrony is regional and weather-cued: rain plus a warm afternoon, and dozens of colonies release at once ([AntWiki — *P. occidentalis*](https://www.antwiki.org/wiki/Pogonomyrmex_occidentalis)).

**Founding.** *P. occidentalis* is **claustral**: after mating once or several times in the air (see polyandry below), the queen sheds her wings, digs a sealed founding chamber, and raises her first nanitic workers entirely from metabolized wing muscles and stored fat (general mechanism in `biology.md` § *Claustral vs semi-claustral founding*). She does not forage during this period.

**Polyandry.** Unusually for ants, *P. occidentalis* queens mate with **multiple males** during the nuptial flight. Wiernasz & Cole's work shows that queens with higher mating frequencies establish colonies that grow faster and produce more reproductive output — polyandry directly correlates with colony fitness ([Wiernasz et al. 2004, *Behav Ecol Sociobiol* — Polyandry and fitness in *P. occidentalis*](https://pubmed.ncbi.nlm.nih.gov/15140102/)).

**Queen lifespan.** This is the species' headline statistic. Mortality rates from Wiernasz & Cole's long-running Idaho field site imply lifespans of roughly **5 years for small colonies and up to ~40 years for large ones**, placing *P. occidentalis* queens among the longest-lived insects ever documented ([Wiernasz & Cole 1995, summarized in Cole & Wiernasz 2025, *Insectes Sociaux* — Ant queen longevity in the field](https://link.springer.com/article/10.1007/s00040-025-01044-y)). The literature commonly cites "up to 30 years" as a conservative point estimate; the 40-year figure comes from colony-persistence data and assumes single-queen continuity. Cole & Wiernasz's reanalysis aggregates **>1,300 colony-years** of observation across 95 colonies — exceptional sample depth for any insect demographic study. The TOML's `queen_lifespan_years = 16.0` is a deliberate sim-pacing compromise (full 30-year reign would be untestable in playtime); flag if extended for a longevity-focused mode.

**Social structure.** Strictly **monogyne** (one queen per colony) and territorial. There is no daughter-queen adoption.

**Mature size.** Field colonies reach **6,000–12,000** workers; the TOML's `target_population = 10000` lands squarely in this band ([Cole & Wiernasz, *Insectes Sociaux* — Colony size and reproduction in *P. occidentalis*](https://link.springer.com/article/10.1007/PL00001711)). Time to maturity is **4–7 years** depending on rainfall and seed availability.

---

## 4. Caste & development

**Castes.** Egg → larva → pupa → adult, with adults expressed as worker, gyne (future queen), or male. Workers are **monomorphic** in body size (no minor/major split as in *Pheidole* or *Atta*). Allocation to reproductive castes is heavily seasonal and tied to colony age — small colonies produce only workers; mature colonies invest substantial fraction of biomass in alates timed to the nuptial-flight window.

**Worker lifespan.** Approximately **one year** in the field, which the TOML reflects directly. This is high for a worker but normal for a temperate granivore — winter diapause (§8) effectively pauses physiological aging.

**Brood cannibalism & trophic eggs.** Both behaviors are present and follow the general mechanisms documented in `biology.md` (§ *Survival cannibalism of brood is normal* and § *Trophic eggs*). Granaries (§5) reduce reliance on cannibalism in *P. occidentalis* compared to non-granivores — a colony with full seed chambers can ride out drought without consuming brood as long as workers can husk seeds in the nest.

**Aging biology — recent finding.** A 2025 study using ovarian transcriptomics in *P. barbatus* (very closely related congeneric) shows that queen ovaries do not exhibit the senescence signatures that limit lifespan in solitary insects, suggesting *Pogonomyrmex* longevity reflects evolved suppression of reproductive aging rather than just low extrinsic mortality ([Friedman et al. 2025, *npj Aging*](https://www.nature.com/articles/s41514-025-00278-1)). Sim-relevant indirectly: justifies modeling queen egg-rate as roughly constant over decades rather than declining with age.

---

## 5. Foraging & diet

**Granivory is the defining trait.** *P. occidentalis* is a **dedicated seed harvester** — the bulk of intake by mass is grass and forb seeds, with arthropod prey and other protein taken opportunistically. Foragers run **trunk trails** radiating from the mound, sometimes 20+ m long, and individual workers cover impressive distances (recruitment is partly trail-mediated and partly individual; see [MacMahon, Mull & Crist 2000, *Annu. Rev. Ecol. Syst.* — Harvester ants and ecosystem influences](https://www.annualreviews.org/content/journals/10.1146/annurev.ecolsys.31.1.265)).

Seed selection is non-random: foragers prefer mid-sized grass seeds (e.g. *Bouteloua*, *Bromus*) and reject seeds outside a workable size range. A mature colony harvests **tens of thousands of seeds per year** and stockpiles them in dedicated **granary chambers** at moderate depth, where humidity is low enough that seeds remain viable for years (the colony is effectively a living seed bank — fact already in TOML `fun_facts`).

Granivory makes the colony a major **ecosystem engineer**: harvester ants are simultaneously seed predators (seeds eaten), seed dispersers (seeds dropped en route or rejected at the mound), and competitors with rodent granivores ([MacMahon et al. 2000](https://www.annualreviews.org/content/journals/10.1146/annurev.ecolsys.31.1.265); Crist & MacMahon — *Granivores, exclosures, and seed banks in sagebrush-steppe*, [ResearchGate](https://www.researchgate.net/publication/223684617_Granivores_exclosures_and_seed_banks_Harvester_ants_and_rodents_in_sagebrush-steppe)).

Navigation across featureless prairie uses polarized-skylight cues and visual landmarks — relevant for any future "trail visualization" sim work.

---

## 6. Nest architecture

The nest is the species' visual signature. Three components matter for the renderer:

1. **The disc / clearing.** Workers actively remove vegetation in a circle around the entrance, often **1–2 m in diameter** and visible from the air. The clearing is functional: it raises soil temperature in spring (faster brood development), reduces fire hazard near the mound, and keeps trunk-trail starts unobstructed ([entomologytoday — harvester ant nest rims](https://entomologytoday.org/2024/01/25/harvester-ant-nest-rims-native-nonnative-plants-invasion-ecology-restoration/pogonomyrmex-occidentalis/)).

2. **The conical pebble mound.** A central mound up to **89 cm in diameter**, decorated with small pebbles and seed chaff that the colony has carried up. The mound's longest slope and entrance face **southeast**, an empirically demonstrated thermal adaptation that maximizes morning solar gain in cold-desert climates ([AntWiki — *P. occidentalis*](https://www.antwiki.org/wiki/Pogonomyrmex_occidentalis); Lavigne 1969 — *Nest architecture in the western harvester ant*, [*Insectes Sociaux*](https://link.springer.com/article/10.1007/BF01240643)).

3. **The deep vertical shaft.** Mature nests reach **3–5 m below ground**, with a central spine of vertical galleries and side chambers — brood near the surface in summer, granaries lower and drier, queen chamber deepest. Substrate preference is sandy / sandy-loam; the psammophore is an excavation specialization for exactly this kind of soil. Chamber siting follows the general rules in `biology.md` § *Chamber siting is functional*.

**Sim-relevant micro-behavior.** The pebble decoration of the mound is unusual — workers actively select small stones during excavation and place them on the cone's surface. This is a clear render hook (see §9).

---

## 7. Defense & combat

The sting is the species' other headline.

**Pain.** Justin Schmidt's pain-index work places *Pogonomyrmex* stings at **3.0 on the 1–4 scale**, tying the genus with paper wasps for the upper plateau of common North American Hymenoptera. Schmidt's own description of a *P. badius* sting: "*ripping muscles and tendons*"; pain "*lasted 4–8 hours*" ([Schmidt — *Pain and Lethality Induced by Insect Stings*, PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC6669698/); [Entomology Today — Painful Stings of Harvester Ants, 2025](https://entomologytoday.org/2025/04/17/painful-fascinating-stings-harvester-ants/)). For comparison, *Paraponera clavata* (bullet ant) is rated 4.0; honey bee is 2.0.

**Toxicity.** This is where the genus is genuinely extreme. *P. maricopa* venom has a **mouse LD50 of 0.12 mg/kg** — the most toxic insect venom ever measured, ~20× more potent than honey-bee venom (2.8 mg/kg) ([UF Book of Insect Records, Ch. 23 — Most Toxic Insect Venom](https://entnemdept.ufl.edu/walker/ufbir/chapters/chapter_23.shtml)). *P. barbatus* sits at ~1.9 mg/kg; *P. occidentalis* has not been measured to the same precision but falls within the same order of magnitude. **The original prompt's framing — "most toxic of any insect by LD50" — is correct for the genus but technically belongs to *P. maricopa*, not *P. occidentalis*.** Treat them as similar in sim numbers but flag the distinction.

**Mechanism.** *Pogonomyrmex* venom is unusually simple chemically — small hydrophobic peptides that target mammalian voltage-gated sodium channels (NaV), lowering activation threshold and blocking inactivation, which produces the long-lasting deep pain ([Robinson et al. 2024, PMC — Peptide toxins and harvester ant stings](https://pmc.ncbi.nlm.nih.gov/articles/PMC10821600/)). The targeting of *vertebrate* NaV channels is the giveaway: the venom evolved against vertebrate predators (horned lizards, badgers, humans), not against rival insects.

**Behavior.** Defensive but not unprovoked. Foragers ignore passing humans; mound disturbance triggers immediate stinging response. Colonies coordinate alarm via pheromone (general mechanism in `biology.md` and project `pheromone.rs`).

---

## 8. Climate & hibernation

Arid-adapted, cold-tolerant. Active foraging window is **roughly April–October** in the core range, with diurnal pattern strongly suppressed during midday heat (>35 °C surface temperature drives foragers underground — they re-emerge in late afternoon).

**Hibernation.** Required. Workers retreat to the deepest chambers when daily mean drops below ~10–12 °C and remain in metabolic depression until spring warming, following the general mechanisms in `biology.md` § *Diapause Biology*. The TOML's `min_diapause_days = 90` and `hibernation_required = true` are correct — *P. occidentalis* simply will not produce viable brood without a winter, which is why hobbyist keepers must provide a refrigerated hibernation period or watch the colony fail in year two.

**Seed storage and diapause** are linked: the granary is filled in autumn explicitly to restart brood production in spring, *not* to feed adults through winter (see `biology.md` § *Adults survive winter on body fat*). This biology is what the post-fix diapause code in the sim is mirroring.

---

## 9. Sim implications

| Real biology (this doc) | Sim feature |
|---|---|
| Sandy / sandy-loam substrate preference | Tag colony placement to prefer a `SubstrateKind::Sand` tile when that field exists; psammophore justifies a small `dig_speed_multiplier` bonus on sand and a penalty in clay/loam |
| Granivore — diet is **seeds**, not generic food | Already reflected in `[diet] prefers = ["seeds", ...]`. Need a distinct `FoodKind::Seed` resource (current sim uses generic food); seeds should be storable at high density in granary chambers and not spoil over diapause |
| Disc-shaped vegetation clearing around mound | Render feature: when a colony is established, periodically clear/desaturate vegetation tiles within a radius of the entrance. Visually unique — players should recognize a Pogonomyrmex colony at a glance |
| Pebble-decorated conical mound, southeast-facing entrance | Render: seed chaff + small pebble sprites on the surface mound; entrance asset oriented southeast (cosmetic but reinforces species identity) |
| Sting toxicity (genus among most toxic insect venoms by LD50) | `worker_attack = 2.8` already higher than baseline; consider adding a `venom_dot` (damage-over-time) component on hit so combats feel different from generic mandible bites |
| Queen lifespan up to 30+ years | TOML compromises at 16 — fine for normal play. Document so a "long-game" mode can lift the cap |
| Polyandry → fitness | Not modeled; possible future tech-tree node ("multiple mating") that boosts long-term colony output, mirroring Wiernasz/Cole findings |
| Hibernation required | Already mirrored; `min_diapause_days = 90` and the diapause-respecting starvation skip from `biology.md` apply directly |
| Trunk trails 20+ m long | Pheromone deposit/evaporation balance should support persistent multi-tick trails; current `EVAP_RATE = 0.02` is already in this regime |

The disc-clearing render feature and the seed-as-distinct-resource model are the two changes that would most clearly differentiate *P. occidentalis* from a generic ant in the sim. Sting venom-DoT is a smaller but flavorful third.

---

## 10. Sources

Primary scientific literature:

- Cole, B. J. & Wiernasz, D. C. (2025). *Ant queen longevity in the field*. **Insectes Sociaux**. [link](https://link.springer.com/article/10.1007/s00040-025-01044-y) — definitive aggregation of >1,300 colony-years from the Idaho long-term site; basis for the 5–40 year lifespan window.
- Cole, B. J. & Wiernasz, D. C. *Colony size and reproduction in the western harvester ant, Pogonomyrmex occidentalis*. **Insectes Sociaux**. [link](https://link.springer.com/article/10.1007/PL00001711) — mature-colony population and reproductive output.
- Wiernasz, D. C. et al. (2004). *Polyandry and fitness in the western harvester ant, Pogonomyrmex occidentalis*. **Behav Ecol Sociobiol** / PubMed. [link](https://pubmed.ncbi.nlm.nih.gov/15140102/) — fitness consequences of multiple mating.
- Lavigne, R. J. (1969). *Nest architecture in the western harvester ant, Pogonomyrmex occidentalis (Cresson)*. **Insectes Sociaux**. [link](https://link.springer.com/article/10.1007/BF01240643) — foundational nest-morphology paper; conical pebble mound, vertical shafts, southeast orientation.
- MacMahon, J. A., Mull, J. F. & Crist, T. O. (2000). *Harvester Ants (Pogonomyrmex spp.): Their Community and Ecosystem Influences*. **Annual Review of Ecology and Systematics** 31: 265–291. [link](https://www.annualreviews.org/content/journals/10.1146/annurev.ecolsys.31.1.265) — synthesis of disc clearing, seed predation/dispersal, and ecosystem-engineer role.
- Schmidt, J. O. (2019). *Pain and Lethality Induced by Insect Stings* — **PMC**. [link](https://pmc.ncbi.nlm.nih.gov/articles/PMC6669698/) — pain-index methodology and Pogonomyrmex sting descriptions.
- Robinson, S. D. et al. (2024). *Peptide toxins that target vertebrate voltage-gated sodium channels underly the painful stings of harvester ants* — **PMC**. [link](https://pmc.ncbi.nlm.nih.gov/articles/PMC10821600/) — venom mechanism at the molecular level.
- Schmidt, J. O. & Blum, M. S. (1978). *Pharmacological and toxicological properties of harvester ant, Pogonomyrmex badius, venom*. **Toxicon**. [link](https://www.sciencedirect.com/science/article/abs/pii/0041010178901927) — original venom-toxicity quantification for the genus.
- Tschinkel, W. R. (2004). *The nest architecture of the Florida harvester ant, Pogonomyrmex badius*. **JStor**. [link](https://www.jstor.org/stable/25086323) — congeneric nest-architecture reference; used for pellet-size figures.
- Friedman, D. A. et al. (2025). *Age, caste, and social context shape ovarian morphology and transcriptomic profiles in red harvester ants*. **npj Aging**. [link](https://www.nature.com/articles/s41514-025-00278-1) — molecular underpinnings of queen longevity.

Reference / encyclopedic:

- AntWiki — [*Pogonomyrmex occidentalis*](https://www.antwiki.org/wiki/Pogonomyrmex_occidentalis); [*Pogonomyrmex* (genus)](https://www.antwiki.org/wiki/Pogonomyrmex).
- University of Florida Book of Insect Records, Ch. 23 — [*Most Toxic Insect Venom*](https://entnemdept.ufl.edu/walker/ufbir/chapters/chapter_23.shtml).
- Idaho Fish & Game — [Species catalog entry](https://idfg.idaho.gov/species/taxa/1476529).
- Entomology Today (2025) — [*The Painful but Fascinating Stings of Harvester Ants*](https://entomologytoday.org/2025/04/17/painful-fascinating-stings-harvester-ants/).
- Alex Wild, *Myrmecos* — [*The bearded ladies*](https://myrmecos.wordpress.com/2008/03/19/the-bearded-ladies/) (psammophore primer).

Disagreements in the literature noted above:
- Queen lifespan ranges across sources from "~15 years" (general ant-keeping) through "up to 30 years" (popular references) to "up to ~40 years" (Cole & Wiernasz colony-persistence inference). All three are defensible; the 40-year figure assumes single-queen continuity over the field-monitoring window and is the upper bound, not the modal observation.
- "Most toxic insect venom" properly belongs to *P. maricopa* (LD50 0.12 mg/kg), not *P. occidentalis* per se — the genus is uniformly extreme but *occidentalis* has not been measured to the same decimal place as *maricopa* or *barbatus*.

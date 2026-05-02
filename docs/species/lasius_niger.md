# *Lasius niger* — The Black Garden Ant

**Companion to** `assets/species/lasius_niger.toml` (sim parameters) and `docs/biology.md` (cross-species mechanisms).
**Audience.** Sim designers grounding parameters in real biology, and the in-game encyclopedia layer (`crates/antcolony-render/src/encyclopedia.rs`) which surfaces excerpts to the player.
**Discipline.** Every quantitative claim cites a source. Where the literature disagrees, both positions are stated.

---

## 1. Identity

**Taxonomy.** *Lasius niger* (Linnaeus, 1758). Subfamily Formicinae, tribe Lasiini. The "*niger* group" was extensively revised by Seifert (1992) and split: many records historically called *L. niger* in dry, sandy, or open habitats are now assigned to *L. platythorax* (woodland) or *L. psammophilus* (xeric / sandy). Modern keepers and field workers should treat the name in the strict post-Seifert sense.

**Common names.** Black garden ant (UK), common black ant, garden ant. German *Schwarze Wegameise*; French *fourmi noire des jardins*.

**Range.** Native across the Palearctic — the entire European subcontinent, the Maghreb, and east through temperate Asia to Mongolia and northern China. Introduced and established across much of North America, primarily as a synanthropic urban ant ([AntWiki — *Lasius niger*](https://www.antwiki.org/wiki/Lasius_niger); [GBIF 144095179](https://www.gbif.org/species/144095179)).

**Where keepers actually find them.** The single most-collected ant in European keeping. Founding queens are picked up by hand or off windowsills and pavements during synchronized summer nuptial flights. Mature colonies are encountered under paving slabs, at the base of garden walls, under loose bark, and in the characteristic crater-mound lawn nests visible after summer rain.

---

## 2. Morphology

**Worker.** 3–5 mm body length; uniformly dark brown to jet black, weakly shining; pubescence sparse. **Monomorphic** — there is no soldier caste and only modest size variation across a colony's worker pool ([AntWiki](https://www.antwiki.org/wiki/Lasius_niger)). Twelve-segmented antennae, no spines on the propodeum, single petiolar node — the diagnostic Formicinae profile.

**Queen.** ~9 mm body length, broadly built thorax with the wing-muscle bulk visible until the muscles are metabolized down during claustral founding. Wings shed at mating. Same dark coloration as workers but with a noticeably more sclerotized gaster.

**Drone (male).** ~3.5–4.5 mm. Slender, paler than workers, with very large eyes and ocelli. Short-lived; dies within hours to days of the nuptial flight.

**Distinguishing features vs sympatric *Lasius*.** The *L. niger* complex is notoriously hard. *L. platythorax* (Seifert) is nearly identical externally and is separated by mesosomal-shape ratios and habitat (deadwood / forest floor). *L. psammophilus* prefers dry sandy heath. *L. flavus* is unambiguous: yellow, smaller eyes, almost entirely subterranean, aphid-on-roots specialist. *L. emarginatus* is bicoloured (red mesosoma, dark gaster). For sim purposes we assume strict *L. niger sensu stricto*.

---

## 3. Colony Lifecycle

**Nuptial flight.** Single, regionally-synchronized summer event, typically late July through late August in temperate Europe ([AntWiki](https://www.antwiki.org/wiki/Lasius_niger); [AntFlights.com aggregated keeper data](https://antflights.com/stats/flights/Lasius/niger)). Triggered by warm humid afternoons following rain; "flight days" can release alates from thousands of colonies across a region simultaneously, sometimes registering on weather radar.

**Founding mode.** Strictly **claustral** ([Sommer & Hölldobler 1995](https://www.sciencedirect.com/topics/biochemistry-genetics-and-molecular-biology/lasius); cross-referenced in `biology.md` *Claustral vs semi-claustral founding*). Queen sheds wings, excavates a small chamber 2–10 cm deep, seals it, and raises the first nanitic worker cohort entirely from metabolized wing muscles and stored fat over roughly 6–10 weeks. She does not feed during this period.

**Founding success.** Field founding success is low — most authors estimate well under 1% of mated queens produce a colony that survives the first winter, owing to predation, desiccation, and territorial workers from established nests. Body condition at flight time is the dominant predictor: experimentally fed queens produced significantly more pupae and workers, confirming that founding-stage brood production is reserve-limited ([Romiguier et al. 2024, *Biological Journal of the Linnean Society*](https://academic.oup.com/biolinnean/article/142/4/397/7342048)).

**Polygyny.** *L. niger* is canonically **monogyne** at maturity. However, **pleometrotic founding** (multiple queens cooperating in a single founding chamber) is common; once nanitic workers eclose, all but one queen is killed ([AntsDavey keeper reference](https://www.antsdavey.co.uk/product-page/lasius-niger-queen-ant-with-brood); [Aron et al. 2024 bioRxiv on a hyper-dense pleometrotic-tolerant population](https://www.biorxiv.org/content/10.1101/2024.07.16.603683v1.full)). The literature notes occasional populations with stable secondary polygyny, but these are exceptional.

**Mature colony size.** Typically 5,000–15,000 workers; well-resourced colonies can reach 30,000+. Colony lifespan equals queen lifespan — see §4.

---

## 4. Caste & Development

**Egg → adult.** At ~25 °C, total development is roughly 6–10 weeks: egg ~2 weeks, larva ~3 weeks, pupa ~2 weeks, with substantial temperature dependence. Below 18 °C development slows dramatically; below ~10 °C it halts and the colony enters diapause (see §8). The TOML encodes 14/21/14 day stages as the warm-season default.

**Worker lifespan.** 1–2 years in the lab, with significant adaptive plasticity: nanitic workers raised by a founding queen live measurably *longer* than workers of established colonies, an evolved trait helping the fragile founding stage survive ([Kramer, Schaible & Scheuerlein 2016, *Experimental Gerontology*](https://www.sciencedirect.com/science/article/pii/S0531556516303345)).

**Queen lifespan.** This is the headline trait. Hermann Appel's captive *L. niger* queen lived **28 years 8 months** in captivity, the longest documented lifespan of any individual eusocial insect ([Kutter & Stumper 1969, original record; reviewed in Keller & Genoud 1997, *Nature*](https://www.nature.com/articles/38894); [HAGR senescence database entry](https://genomics.senescence.info/species/entry.php?species=Lasius_niger)). This figure requires properly cycled hibernation; queens denied diapause die within a few years.

**Trophic eggs.** Documented across Formicinae and used by *Lasius* queens; mechanism described in `biology.md` *Trophic eggs*. Particularly important during claustral founding when the queen is the only nutrient source for first-instar larvae.

**Brood cannibalism.** *L. niger* queens are the model organism for **filial cannibalism as disease defense**: founding queens cannibalized 92% of *Metarhizium*-infected larvae versus 6% of healthy controls and laid 55% more eggs afterward ([Pull et al. 2024, *Current Biology*](https://www.cell.com/current-biology/fulltext/S0960-9822(24)01001-7)). See `biology.md` *Queen filial cannibalism* and *Survival cannibalism of brood is normal*.

---

## 5. Foraging & Diet

**The headline trait — aphid mutualism.** *L. niger* is the textbook honeydew-tending temperate ant. Workers shepherd aphid colonies (especially *Aphis fabae* on broad bean, and root aphids tended underground), drumming the aphids' abdomens with antennae to solicit honeydew droplets, defending herds from coccinellid and syrphid predators, and in some cases relocating aphids to better feeding sites ([Wikipedia — *Lasius niger* §Diet, with Stadler & Dixon refs](https://en.wikipedia.org/wiki/Black_garden_ant)). Honeydew is the dominant carbohydrate income for many colonies.

**Other diet.** Generalist omnivores — extrafloral nectar, dead arthropods, live small insect prey, household sweets and fats. Protein intake is dominated by insect prey delivered to the brood; carbohydrate flow goes overwhelmingly to adult workers.

**Trail pheromone chemistry.** Trail substance is produced in the **hindgut** (Formicinae characteristic) rather than the Dufour's or poison gland used by Myrmicinae. The exact identified compounds for *L. niger* remain incompletely published; pyrazines have been identified as trail components in many Formicinae ([Cerdá & Dejean 2014 review, PMID 25233585](https://pubmed.ncbi.nlm.nih.gov/25233585/); [Stökl et al. 2018 on convergent pyrazine use](https://www.nature.com/articles/s41598-018-20953-6)).

**Trail dynamics.** Mean lifetime of a single trail mark is **~47 minutes** under lab conditions ([Beckers, Deneubourg & Goss 1993, *Journal of Insect Behavior*](https://link.springer.com/article/10.1007/BF01201674)). Foragers exploiting a 1 M sucrose source laid 43% more trail marks than foragers on weaker sources, and this single modulation is sufficient to explain collective selection of the richer source.

**Recruitment style.** **Mass recruitment via trail pheromone**, not tandem running — *L. niger* is a canonical mass-recruiter, contrasting with *Temnothorax* (tandem) or *Pachycondyla* (group raids). Workers also use visual landmarks integratively with the trail ([Grüter et al. 2008, *Behavioral Ecology and Sociobiology*](https://link.springer.com/article/10.1007/s00265-008-0657-6)).

---

## 6. Nest Architecture

**Substrate preference.** Loam, garden soil, sandy loam, under stones and pavement slabs. Tolerant of urban substrate. Avoids waterlogged soil and pure sand (the latter is the niche of sister species *L. psammophilus*).

**Depth and chamber organization.** Soil nests reach **1–2 m deep**. The entrance chamber is surrounded by 1–6 satellite chambers, with 2–3 main vertical tunnels descending to widely-spaced lower chambers. Vertical chamber distribution, worker age, and brood type correlate strongly with the soil CO₂ gradient ([AntWiki *Lasius niger*; Tschinkel methodology summary](https://link.springer.com/article/10.1007/s10818-015-9203-6)). See `biology.md` *CO₂ and humidity gradients drive dig direction* and *Chamber siting is functional, not random*.

**Dig rate.** A small founding colony excavates ~1–2 cm³ of substrate per day under good conditions ([Sudd 1972, *Animal Behaviour*](https://www.sciencedirect.com/science/article/abs/pii/S0003347272802701); cross-ref `biology.md` *Excavation rate is slow*). Pellet size ~0.3–0.7 mm — small relative to *Pogonomyrmex*. Mound at the entrance is the diagnostic visual (`biology.md` *Kickout mound*).

**Thatch / dome.** Unlike *Formica rufa*, *L. niger* does **not** build a thatched dome. Surface expression is a low conical crater-mound of excavated fines, often re-exposed after rain. In lawns the species does build characteristic raised earth domes a few cm high, especially in midsummer.

**Plasticity.** Architecture is highly plastic: experimental manipulation produces measurably different chamber-layout patterns, demonstrating stigmergic construction shaped by topochemical cues ([Khuong et al. 2016, *PNAS*](https://www.pnas.org/content/113/5/1303)).

---

## 7. Defense & Combat

**No soldier caste.** Monomorphic — every worker is a generalist. The TOML correctly encodes `soldier = 0.0`. All defensive load falls on standard workers.

**Formic acid.** Formicinae trait. Workers spray formic acid from the acidopore at the gaster tip during defense, paired with mandibular biting ([antnest.co.uk keeper reference](https://www.antnest.co.uk/lasius-niger/)). Range and volume are modest compared to *Formica rufa*; the spray functions primarily at point-blank range against single attackers and as a contact deterrent to mites and predators near the nest entrance.

**Territoriality.** Aggressive towards conspecific non-nestmates and other ground-dwelling ants in its size class. Intraspecific aggression varies with population structure: closely-related neighboring nests in dense urban populations show *low* worker-on-worker aggression, while more distant population comparisons elicit strong attack responses ([Boulay et al. 2024 bioRxiv preprint on low aggression in dense populations](https://www.biorxiv.org/content/10.1101/2024.07.23.604725v1.full.pdf)). Will dismember and consume founding queens of other species and conspecifics that wander into established territory — a major selection pressure on the timing and synchrony of nuptial flights.

**Alarm response.** Worker alarm pheromone (formicine undecane and related hydrocarbons) recruits nearby nestmates to a disturbance and induces gaping mandibles + raised gaster posture. See `biology.md` *Alarm-pheromone steering* (PvP gating note).

---

## 8. Climate & Hibernation

Strictly temperate. *L. niger* **requires** an annual hibernation period — without it, queen lifespan collapses from decades to a few years and egg viability drops within 2–3 cycles. This is the single most important husbandry fact for keepers.

**Diapause window.** October through March in northern Europe; ~5 months at 5–15 °C is the keeper-standard regime. Brood production halts before diapause onset; the queen overwinters with no brood in the nest, and oviposition resumes in March/April as temperatures rise above ~12 °C. Adult workers survive winter on body fat, not on stored colony food (`biology.md` *Adults survive winter on body fat*).

**Cold tolerance.** Survives sustained 2–8 °C without issue. Brief exposure to sub-zero is tolerated by clustered workers in deep chambers but kills exposed individuals. Native populations occur as far north as central Scandinavia.

**Autumn retreat.** Workers cease foraging and retreat to deep chambers when daily mean temperature falls below ~12–15 °C — the trigger our sim uses for `AntState::Diapause` (`biology.md` *Autumn retreat*).

---

## 9. Sim Implications

Bullet list of how this biology should plug into our sim parameters. Cross-references to `docs/biology.md` mechanism sections in *italics*.

- **Monomorphic — no soldiers.** TOML already correct. Combat code must not branch on soldier caste for this species; defensive load is uniform across the worker pool.
- **Strict claustral founding.** `founding = "claustral"` already set. When founding-stage simulation is added (`biology.md` *Claustral vs semi-claustral founding*), the queen consumes only her body reserves for the first ~6–10 weeks; no foraging, no food inflow.
- **Pleometrotic founding** is a future content add: multiple-queen start chambers with one survivor after first nanitics. PvP-relevant if we add a "found a daughter colony" mechanic.
- **Queen lifespan 28 y, worker lifespan 1–2 y.** TOML values (28 / 24 mo) match the literature record. The huge Q:W lifespan ratio is the headline single-player progression hook — a *L. niger* queen can plausibly outlive the player's interest in a single save file.
- **Hibernation REQUIRED.** `hibernation_required = true`, `min_diapause_days = 60`. Plug straight into the diapause subsystem (`biology.md` *Diapause Biology*). Skipping diapause should accelerate queen aging — a future quality-of-keeping mechanic.
- **Aphid mutualism is the foraging hook.** Currently we have generic `["sugar", "protein", "honeydew"]` diet. A future `Aphid` resource type on the world grid would let *L. niger* establish persistent honeydew-flow lanes — a much more visually distinctive forage pattern than scatter-found sugar tiles.
- **Trail pheromone half-life ~47 min real-time.** At our default sim time scale, this should map to an evaporation rate that is *moderate* — not the very fast decay used for short-lived alarm signals, not the near-permanent colony-scent layer. See `biology.md` references and tune `pheromone.evaporation_rate` per-species when species-specific overrides are added.
- **Mass recruitment, not tandem.** Existing `FollowingTrail` FSM state is correct for *L. niger*. Tandem-running species (*Temnothorax*) will need a different state, but not for this species.
- **Modulated trail strength.** Foragers deposit *more* pheromone for richer food. Currently we deposit a flat amount; a future enhancement is `deposit_strength = base × (food_quality / reference_quality)`. This is the mechanism Beckers et al. proved sufficient for collective source selection.
- **Filial cannibalism is canonical here.** The queen-cannibalism-of-infected-brood feature, when added (Phase 6 hazards), should default-activate for *L. niger*. The species is the published model organism for this behavior.
- **Nest depth 1–2 m, no thatched dome.** When the underground module gets per-species architectural priors, *L. niger* should bias toward *deep, narrow, vertical* layouts with small surface kickout mounds rather than wide dome-builders like *Formica rufa*.
- **Aggression is context-dependent.** `aggression = 0.2` in the TOML is a reasonable scalar. If we add neighbor-recognition logic, *L. niger* should be the species that *tolerates close kin neighbors* but *attacks distant population non-nestmates* — the inverse of the simple "all non-colony ants hostile" rule.

---

## 10. Sources

**Peer-reviewed.**
- Keller, L. & Genoud, M. (1997). Extraordinary lifespans in ants: a test of evolutionary theories of ageing. *Nature* 389: 958–960. <https://www.nature.com/articles/38894>
- Kutter, H. & Stumper, R. (1969). Hermann Appel, ein leidgeadelter Entomologe. *Proc. VI Congress IUSSI, Bern* — original 28y-8mo *L. niger* queen record.
- Kramer, B.H., Schaible, R. & Scheuerlein, A. (2016). Worker lifespan is an adaptive trait during colony establishment in the long-lived ant *Lasius niger*. *Experimental Gerontology* 85: 18–23. <https://www.sciencedirect.com/science/article/pii/S0531556516303345>
- Pull, C.D. et al. (2024). Ant queens cannibalise infected brood to contain disease spread and recycle nutrients. *Current Biology*. <https://www.cell.com/current-biology/fulltext/S0960-9822(24)01001-7>
- Beckers, R., Deneubourg, J.L. & Goss, S. (1993). Modulation of trail laying in the ant *Lasius niger* and its role in the collective selection of a food source. *Journal of Insect Behavior* 6: 751–759. <https://link.springer.com/article/10.1007/BF01201674>
- Czaczkes, T.J. (2017). Pheromone trail following in the ant *Lasius niger*: high accuracy and variability but no effect of task state. *Physiological Entomology*. <https://resjournals.onlinelibrary.wiley.com/doi/10.1111/phen.12174>
- Grüter, C., Czaczkes, T.J. & Ratnieks, F.L.W. (2008). Combined use of pheromone trails and visual landmarks by the common garden ant *Lasius niger*. *Behavioral Ecology and Sociobiology* 63: 277–284. <https://link.springer.com/article/10.1007/s00265-008-0657-6>
- Khuong, A. et al. (2016). Stigmergic construction and topochemical information shape ant nest architecture. *PNAS* 113: 1303–1308. <https://www.pnas.org/content/113/5/1303>
- Romiguier, J. et al. (2024). Claustral colony founding is limited by body condition: experimental feeding increases brood size of *Lasius niger* queens. *Biological Journal of the Linnean Society* 142: 397. <https://academic.oup.com/biolinnean/article/142/4/397/7342048>
- Cerdá, X. & Dejean, A. (2014). A list of and some comments about the trail pheromones of ants. PMID 25233585. <https://pubmed.ncbi.nlm.nih.gov/25233585/>
- Stökl, J. et al. (2018). Pyrazines from bacteria and ants: convergent chemistry within an ecological niche. *Scientific Reports*. <https://www.nature.com/articles/s41598-018-20953-6>
- Sudd, J.H. (1972). The absence of social enhancement of digging in pairs of ants. *Animal Behaviour* 20: 813–819. <https://www.sciencedirect.com/science/article/abs/pii/S0003347272802701>
- Tschinkel, W.R. (2015). The architecture of subterranean ant nests: beauty and mystery underfoot. *Journal of Bioeconomics* 17: 271–291. <https://link.springer.com/article/10.1007/s10818-015-9203-6>
- Hahn, D.A. & Denlinger, D.L. (2011). Energetics of insect diapause. *Annual Review of Entomology* 56: 103–121. <https://www.annualreviews.org/doi/10.1146/annurev-ento-112408-085436>

**Preprints.**
- Boulay et al. (2024). Low aggression between workers from different zones of a *Lasius niger* nest complex. bioRxiv. <https://www.biorxiv.org/content/10.1101/2024.07.23.604725v1.full.pdf>
- Aron et al. (2024). Queens from a unique hyper-dense *L. niger* population tolerate pleometrosis better than queens from a 'normal' population. bioRxiv. <https://www.biorxiv.org/content/10.1101/2024.07.16.603683v1.full>

**Reference works & databases.**
- Hölldobler, B. & Wilson, E.O. (1990). *The Ants*. Belknap/Harvard. — general Formicinae biology, trail and recruitment chapters.
- Wilson, E.O. (1971). *The Insect Societies*. Belknap/Harvard. — nest construction reference.
- AntWiki — *Lasius niger* species page. <https://www.antwiki.org/wiki/Lasius_niger>
- AntWeb — *Lasius niger* specimen records. <https://www.antweb.org/>
- HAGR senescence database, *L. niger* entry. <https://genomics.senescence.info/species/entry.php?species=Lasius_niger>
- GBIF backbone taxonomy, *L. niger* (Linnaeus, 1758). <https://www.gbif.org/species/144095179>

**Keeper community.**
- AntFlights.com aggregated keeper-submitted nuptial-flight observations. <https://antflights.com/stats/flights/Lasius/niger>
- Myrm's Ant Nest — *Lasius niger* husbandry. <https://www.antnest.co.uk/lasius-niger/>
- AntKeepers — Lifecycle of an ant colony. <https://antkeepers.com/pages/lifecycle-of-an-ant-colony>
- Formiculture.com — *L. niger* nuptial-flight and keeping threads. <https://www.formiculture.com/topic/1534-nuptial-flights-for-lasius-niger/>

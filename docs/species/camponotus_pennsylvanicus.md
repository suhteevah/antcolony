# Camponotus pennsylvanicus — Eastern Black Carpenter Ant

**Scope.** Natural-history reference for the in-game encyclopedia and for grounding sim parameters in `assets/species/camponotus_pennsylvanicus.toml`. Companion to `docs/biology.md`, which holds the *general* mechanisms (claustral founding, diapause metabolic depression, kickout mounds, brood cannibalism). This file holds the *Camponotus-specific* numbers and behaviors that plug into those mechanisms. Cross-references to biology.md sections are noted inline as "→ biology.md §X".

---

## 1. Identity

- **Taxonomy.** Hymenoptera: Formicidae: Formicinae: Camponotini: *Camponotus* (Mayr, 1861), subgenus *Camponotus s. str.* Type species described by De Geer (1773) as *Formica pennsylvanica*; the binomial *Camponotus pennsylvanicus* is the modern combination.
- **Common names.** Eastern black carpenter ant; black carpenter ant; (regionally) sugar ant — the latter is a misnomer shared with *Camponotus* spp. generally.
- **Range.** Native to eastern North America, from the southern fringe of the Canadian boreal (southern Ontario, Quebec, Maritimes) south through the entire eastern United States to northern Florida, and west to roughly the 100th meridian — eastern Texas, Oklahoma, Kansas, the eastern Dakotas. Allopatric or marginally sympatric with the related *Camponotus modoc* (western North America) and *C. herculeanus* (boreal/montane). [AntWiki — *C. pennsylvanicus*](https://www.antwiki.org/wiki/Camponotus_pennsylvanicus); [Animal Diversity Web — *C. pennsylvanicus*](https://animaldiversity.org/accounts/Camponotus_pennsylvanicus/).
- **Habitat.** Closed-canopy and edge deciduous and mixed forest, riparian woodland, suburban yards with mature trees, and structural timber in human dwellings. Obligately associated with **decayed or moisture-damaged wood** for nest initiation; once established, mature colonies may extend galleries into adjacent sound wood. [Cornell/Maine extension — *Carpenter Ants*](https://www.maine.gov/dacf/php/gotpests/bugs/factsheets/carpenter-ants-cornell.pdf); Hansen & Klotz (2005), *Carpenter Ants of the United States and Canada*, Cornell University Press.

## 2. Morphology

Polymorphic — a headline trait of the genus and a primary visual ID. Worker size is *continuously* distributed (not bimodal), but ant-keeping and entomological literature conventionally split the distribution at the size where mandibles and head capsule become disproportionately enlarged.

| Caste | Body length | Notes |
|---|---|---|
| Minor worker | **6–9 mm** | Aphid-tending, brood care, short-range forage. |
| Major worker | **10–13 mm**, super-majors to ~14–17 mm | Defense, gallery excavation, long-range forage; oversized mandibles and head capsule. |
| Gyne (queen) | **~17–20 mm** | Largest caste; dealated post-flight; massive gaster for endogenous reserve storage (→ biology.md §Claustral founding). |
| Male | **~9–11 mm** | Slender, winged, short-lived. |

[AntWiki — *C. pennsylvanicus*](https://www.antwiki.org/wiki/Camponotus_pennsylvanicus); [Black carpenter ant — Wikipedia](https://en.wikipedia.org/wiki/Black_carpenter_ant).

**Distinguishing features.** Uniformly matte black integument with a characteristic **golden/yellowish pubescence on the gaster** that catches direct light — the single best field mark separating *C. pennsylvanicus* from sympatric *C. herculeanus* (which shows reddish mesosomal coloration) and *C. nearcticus* (smaller, smoother). The mesosomal dorsum bears erect setae; the cheeks (gena) and scape do *not* — a diagnostic absence used in keys (AntWiki). The propodeum is evenly arched in profile.

## 3. Colony Lifecycle

- **Nuptial flight.** Late spring to early summer, **mid-May through late July** depending on latitude — flights begin in April/May in the southern range (TN, GA) and as late as June/early July at the northern edge (ON, QC, ME). Flights are often crepuscular and triggered by warm afternoons following rain. [AntFlights.com — *C. pennsylvanicus*](https://antflights.com/stats/flights/Camponotus/pennsylvanicus); Hansen & Klotz (2005).
- **Founding.** Fully **claustral** (→ biology.md §Claustral vs semi-claustral founding). The newly-mated gyne dealates, locates a small cavity in decayed wood (under bark, in a rotting log, in a dead branch), seals herself in, and raises the first nanitic cohort entirely on metabolized wing musculature and abdominal lipid stores. She does not forage and does not drink. [Buckeye Myrmecology — *C. pennsylvanicus* caresheet](https://buckeyemyrmecology.com/camponotus-pennsylvanicus-caresheet/).
- **Notoriously slow growth.** First-year colonies in captivity typically reach only **5–25 workers**; second year **40–200**; third year **350–1,500** (Buckeye Myrmecology; AntShack caresheet). Wild colonies are slower still — Hansen & Klotz report **3–6 years to a "large" colony** and **6–10 years before the first true majors appear**.
- **Mature population.** Hansen & Klotz (2005) cite mature colonies at **2,000–2,500 workers** as typical, with very old established nests reaching **10,000–15,000**. Sources disagree on the upper bound: extension-service literature commonly quotes "~3,000 average," while keeper observations of decade-old wild colonies push higher. The sim TOML's `target_population = 8000` sits in the upper-middle of the published range and is defensible.
- **Colony lifespan.** Bounded by queen lifespan. Founding queens have been documented at **15+ years** in captivity, and Hansen & Klotz report estimates of up to ~25 years for exceptional individuals. Once the queen dies the colony cannot re-queen (monogyne — see §4) and decays over the following season.

## 4. Caste & Development

- **Egg → larva → pupa → adult.** Egg ~20–30 d, larva ~10–15 d (more for majors), pupa ~18–25 d. Total **6–8 weeks** from egg to eclosion under summer temperatures (Buckeye Myrmecology; AntShack). This is **substantially longer than *Lasius niger*** (~4–6 weeks total) and is the proximate reason Camponotus colony growth is so visibly slow. Sim values of 21 d / 28 d / 21 d (in `growth` block) sit at the upper end of this range — appropriate for a temperate population at average rather than peak summer temperatures.
- **Worker lifespan.** Minors live **~1 year** in field conditions; majors live longer, often **1–2 years**. The TOML's 9-month worker lifespan averages over both castes and underrepresents majors slightly — acceptable for now.
- **Queen lifespan.** 15–20+ years; see §3.
- **Social structure.** Predominantly **monogyne** (one queen per colony), but the literature notes occasional facultative oligyny in dense suburban populations (Pratt & co-authors' suburban-sprawl work; [Stanton et al. — *Suburban sprawl: environmental features affect colony social and spatial structure in C. pennsylvanicus*](https://www.researchgate.net/publication/227683463_Suburban_sprawl_Environmental_features_affect_colony_social_and_spatial_structure_in_the_black_carpenter_ant_Camponotus_pennsylvanicus)). Treat as monogyne for sim purposes.
- **Polydomy.** Mature colonies are routinely **polydomous** — a parent nest plus one to several **satellite nests** in adjacent trees, stumps, or wall voids, connected by trunk trails. Stanton et al. document colonies spanning **6–28 m² occupying 1–6 trees**. Satellites lack a queen and house older brood (pupae), majors, and males.
- **Brood cannibalism / trophic eggs.** Both occur and follow the genus-typical pattern documented in → biology.md §Survival cannibalism of brood and §Trophic eggs. No species-specific deviation needs to be encoded.

## 5. Foraging & Diet

- **Diet.** Omnivorous. Carbohydrate from **aphid and scale honeydew** (the dominant calorie source through summer), floral and extra-floral nectar, and occasional fruit; protein from live and scavenged arthropods. Camponotus do **not** digest cellulose — the "carpenter" name refers to nest excavation only, not diet (Hansen & Klotz 2005; [Smithsonian BugInfo — Carpenter Ants](https://www.si.edu/spotlight/buginfo/carpenter-ants)).
- **Recruitment.** Predominantly **individual scouting + tandem running and short trail recruitment**, *not* sustained mass-recruitment trails of the *Lasius* / *Linepithema* type. Traniello (1977) showed scouts use alerting motor displays at the nest entrance, then lay a recruitment trail composed of **hindgut material** (long-lasting orientation cue) overlaid with **poison-gland formic acid** (short-lived attractant) — the trail is real but transient. [Traniello, J.F.A. (1977). Recruitment behavior, orientation, and the organization of foraging in the carpenter ant *Camponotus pennsylvanicus* DeGeer. *Behav. Ecol. Sociobiol.* 2: 61–79.](https://link.springer.com/article/10.1007/BF00299289); [Bestmann et al. (2000), *Annals ESA* — chemistry of rectal and accessory-gland contents in *C. pennsylvanicus*](https://academic.oup.com/aesa/article/93/6/1294/161470). **Sim implication:** the species' `food_trail` deposit rate should be set lower than for *Lasius* — Camponotus do not paint persistent freeway trails across the map.
- **Activity rhythm.** Mainly **nocturnal and crepuscular**; peak forager outflow within ~1 hour of sunset. Sanders (1972, *Canadian Entomologist*) showed activity onset is temperature-gated and seasonal activity peaks in midsummer. [Sanders (1972). Trail-laying behaviour of the carpenter ant, *Camponotus pennsylvanicus*. *Can. Entomol.*](https://www.cambridge.org/core/journals/canadian-entomologist/article/abs/traillaying-behaviour-of-the-carpenter-ant-camponotus-pennsylvanicus-hymenoptera-formicidae/974045D90A4866CFEEF466FE603C38F9). Foragers use celestial / canopy-pattern cues for orientation in low light ([Klotz & Reid (1993), nocturnal orientation in *C. pennsylvanicus*](https://link.springer.com/article/10.1007/BF01338835)).
- **Forage range.** Individual workers regularly travel **>30 m** from the nest in a single nightly trip; published observations have tracked individuals to **>100 m** (Hansen & Klotz 2005).

## 6. Nest Architecture

This is the species' defining trait and the reason it deserves its own sim substrate. Carpenter ants **excavate galleries in wood without consuming it**.

- **Substrate.** Strongly biased toward **moisture-softened or fungus-decayed wood** (heartwood-rotted standing trees, fallen logs, structural timbers compromised by leaks, condensation, or carpenter-bee galleries). The founding queen virtually requires soft substrate; mature colonies will then **extend galleries into adjacent sound wood** as the population grows. This is the proximate mechanism by which they damage human structures — they don't initiate in sound lumber, but they will eat into it from a wet starting point. [UMass Amherst — *C. pennsylvanicus* fact sheet](https://www.umass.edu/agriculture-food-environment/landscape/publications-resources/insect-mite-guide/camponotus-pennsylvanicus); Hansen & Klotz (2005).
- **Gallery morphology.** Smooth-walled, sandpapered-looking galleries running with the wood grain, typically 4–10 mm wide, branching into chambers at irregular intervals. *C. pennsylvanicus* galleries are notably **cleaner and smoother** than termite workings (which are mud-packed and run with grain crudely). The sim's existing chamber types (QueenChamber, BroodNursery, FoodStorage, Waste) map well onto observed Camponotus internal architecture (→ biology.md §Chamber siting is functional).
- **Frass kickout.** Excavated material is kicked out of gallery openings as **sawdust-like frass** containing wood fibers, dead nestmates, and arthropod cuticle. This is the diagnostic sign for both pest control and field ecology — see → biology.md §Kickout mound for the general mechanism. Camponotus frass is dry and powdery rather than the cohesive soil pellets of *Lasius* or *Pogonomyrmex* (→ biology.md §Soil pellets, not grains), and accumulates as a fan rather than a donut mound.
- **Excavation rate.** Slow even by ant standards. Wood is mechanically tougher than loam, and Camponotus rely on mandible attrition with no chemical or saliva softening. → biology.md §Excavation rate is slow gives the general framework; for *Camponotus pennsylvanicus* specifically, expect roughly **half the per-worker dig rate of equivalent-sized *Lasius* in soil**. Sim implication: a dedicated `dig_speed_multiplier` of ~0.5 in the species TOML's `appearance` block (currently absent) when substrate = Wood would match observations.

## 7. Defense & Combat

- **Mandibles.** Large and sharp in majors; capable of drawing blood from human skin and severing other ants. Defensive bite is the primary close-range weapon.
- **Formic acid.** Like all Formicinae, *C. pennsylvanicus* has lost the sting and instead **sprays formic acid from the acidopore** at the gaster tip. A startled worker will assume a defensive posture — gaster tucked forward under the body — and discharge a fine mist of formic acid that is both a chemical irritant and an alarm pheromone amplifier. The acid is the same poison-gland secretion that decorates the recruitment trail (§5). [Bestmann et al. (2000)](https://academic.oup.com/aesa/article/93/6/1294/161470).
- **No true soldier caste.** Majors function as guards and combat specialists but are morphologically a continuation of the worker polymorphism, not a separate caste with distinct development as in *Pheidole* or *Atta*. Sim's `soldier` slot in the caste ratio should be read as "majors functioning in defensive role," not a discrete morph.
- **Phragmosis.** Majors can plug small entrance galleries with their oversized heads — a low-energy passive defense (Hansen & Klotz 2005).
- **Territoriality.** Inter-colony aggression is moderate; conspecific colony fights at trail boundaries are documented but rarely lethal at the colony level. Aggression toward other ant species (*Tapinoma*, *Crematogaster*) is notably higher than toward conspecifics.

## 8. Climate & Hibernation

- **Climate envelope.** Cold-temperate to warm-temperate; the species crosses USDA hardiness zones ~3 through ~9. Northern populations face 4–5 month winters; southern Florida populations may experience only brief diapause.
- **Diapause is obligate** in northern populations. Skipping diapause in captivity reliably causes queens to cease laying and eventually die (universal keeper consensus; Buckeye Myrmecology, AntShack, Canada Ant Colony caresheets). Recommended captive hibernation: **5–12 °C for ~16–20 weeks** (the TOML's `min_diapause_days = 120` matches the lower bound).
- **Cold tolerance.** Adults survive sustained sub-freezing temperatures inside galleries, where the wood substrate buffers the temperature swing. The colony retreats deep into the parent nest in autumn (→ biology.md §Autumn retreat). All adult survival during diapause is on body fat (→ biology.md §Adults survive winter on body fat) — colony food stores are NOT consumed at active-season rates (→ biology.md §Metabolic depression).
- **Brood pause.** Queens stop laying for the duration of cold; existing larvae overwinter as larvae rather than maturing, and resume development on spring warming. This is the source of the well-known "spring brood pulse" in keeper colonies.

## 9. Sim Implications

Concrete handles for tuning the species inside the existing simulation systems:

- **Substrate preference (gap in current sim).** The sim defaults underground modules to **Loam** (→ biology.md §Substrate type changes everything). *C. pennsylvanicus* needs a **Wood** substrate variant — slower dig, frass kickout instead of pellet mound, no saliva-pellet wall reinforcement. This is the single largest species-fidelity gap right now.
- **Polymorphism gameplay hooks.** Majors are visually and mechanically distinct (1.5× linear size, ~3× volume, ~2× combat). Render layer can use the existing major sprite slot; sim layer should give majors higher `attack`, `health`, slower `speed`, and bias them toward Defend / Excavate behaviors over Forage. Caste ratio in the TOML (`soldier = 0.10`) reads as "fraction of population that is a major functioning as defender" — leave as is.
- **Slow-growth implications for the player.** Year-1 in real time is ~30–50 workers. At default sim time-compression this should still feel deliberate; the species is the project's flagship "patience" pick. Don't accelerate growth to make it "fun" — the slow ramp is the species identity. Pair with mid-game payoff: a Camponotus colony that survives 3+ in-game years should dwarf any *Lasius* colony of the same age.
- **Recruitment style.** Cap `food_trail` deposit and decay so trails are short-lived and do not visually dominate the map (vs *Lasius*). Encourage individual scouts and short tandem chains. The pheromone math in `pheromone.rs` already supports this via per-species deposit constants.
- **Nocturnal bias.** If/when day-night cycle ships, scale forager outflow ~3× higher in the night quarter and ~0.2× in midday. Sanders (1972) is the citation.
- **Diapause length floor.** TOML's `min_diapause_days = 120` is correct and should not be reduced — skipping diapause kills queens both in nature and in the sim's existing diapause model.
- **Forage range.** Sense and forage radii in `ant.rs` should permit Camponotus workers to wander further than the *Lasius* default — single-trip ranges of 30+ m are normal.

## 10. Sources

**Canonical reference.**
- Hansen, L.D. & Klotz, J.H. (2005). *Carpenter Ants of the United States and Canada*. Cornell University Press, Ithaca, NY. [OCLC 56753925]. The standard work on Nearctic Camponotus biology, ecology, and pest status; cited throughout extension and pest-control literature.

**Primary literature.**
- [Pricer, J.L. (1908). The life history of the carpenter ant. *Biological Bulletin* 14: 177–218.](https://www.jstor.org/stable/1535816) Foundational behavioral observations, including early evidence of trail-laying.
- [Sanders, C.J. (1972). Trail-laying behaviour of the carpenter ant, *Camponotus pennsylvanicus*. *The Canadian Entomologist*.](https://www.cambridge.org/core/journals/canadian-entomologist/article/abs/traillaying-behaviour-of-the-carpenter-ant-camponotus-pennsylvanicus-hymenoptera-formicidae/974045D90A4866CFEEF466FE603C38F9) Temperature-gated activity and trail behavior.
- [Traniello, J.F.A. (1977). Recruitment behavior, orientation, and the organization of foraging in the carpenter ant *Camponotus pennsylvanicus* DeGeer. *Behavioral Ecology and Sociobiology* 2: 61–79.](https://link.springer.com/article/10.1007/BF00299289) The definitive study of recruitment chemistry and individual-scout-plus-trail organization.
- [Klotz, J.H. & Reid, B.L. (1993). Nocturnal orientation in the black carpenter ant *Camponotus pennsylvanicus* (DeGeer). *Insectes Sociaux*.](https://link.springer.com/article/10.1007/BF01338835) Low-light celestial-cue orientation.
- [Bestmann, H.J. et al. (2000). Chemistry and behavioral significance of rectal and accessory gland contents in *Camponotus pennsylvanicus*. *Annals of the Entomological Society of America* 93(6): 1294–1303.](https://academic.oup.com/aesa/article/93/6/1294/161470) Trail and defensive chemistry.
- [Stanton et al. — *Suburban sprawl: environmental features affect colony social and spatial structure in C. pennsylvanicus*.](https://www.researchgate.net/publication/227683463_Suburban_sprawl_Environmental_features_affect_colony_social_and_spatial_structure_in_the_black_carpenter_ant_Camponotus_pennsylvanicus) Polydomy, satellite-nest extents, social structure variation.

**Reference & extension.**
- [AntWiki — *Camponotus pennsylvanicus*.](https://www.antwiki.org/wiki/Camponotus_pennsylvanicus) Taxonomy, morphology key, distribution.
- [Animal Diversity Web — *Camponotus pennsylvanicus*.](https://animaldiversity.org/accounts/Camponotus_pennsylvanicus/) Synthesis, range, diet.
- [Smithsonian BugInfo — Carpenter Ants.](https://www.si.edu/spotlight/buginfo/carpenter-ants) General-audience summary.
- [UMass Amherst CAFE — *Camponotus pennsylvanicus*.](https://www.umass.edu/agriculture-food-environment/landscape/publications-resources/insect-mite-guide/camponotus-pennsylvanicus) Decay-association, pest biology.
- [Cornell Cooperative Extension / State of Maine — *Carpenter Ants* fact sheet (PDF).](https://www.maine.gov/dacf/php/gotpests/bugs/factsheets/carpenter-ants-cornell.pdf) Wood-damage mechanism, control.
- [USDA Forest Service — Carpenter Ants and Wood Decay (general).](https://www.fs.usda.gov) USFS publishes multiple regional pest bulletins on Camponotus / wood-decay association; cited generally where extension literature concurs with Hansen & Klotz.

**Keeper / hobbyist (used only for captive-husbandry numbers and growth-rate observations, never as primary biology citations).**
- [Buckeye Myrmecology — *C. pennsylvanicus* caresheet.](https://buckeyemyrmecology.com/camponotus-pennsylvanicus-caresheet/)
- [AntShack — *C. pennsylvanicus* care sheet.](https://www.ant-shack.com/blogs/ant-care-sheets-1/camponotus-pennsylvanicus-black-carpenter-ant-care-sheet)
- [Canada Ant Colony — *C. pennsylvanicus* care sheet.](https://canada-ant-colony.com/blogs/articles/camponotus-pennsylvanicus-eastern-black-carpenter-ant-care-sheet)
- [AntFlights.com — *C. pennsylvanicus* nuptial flight observations.](https://antflights.com/stats/flights/Camponotus/pennsylvanicus)

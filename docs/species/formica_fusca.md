# *Formica fusca* — Silky Wood Ant

> Encyclopedia entry for the ant colony simulation. Companion to `assets/species/formica_fusca.toml`. *Formica fusca* is the host species for the parasitic *Formica* radiation — its biology is what makes *F. rufa*'s and *F. sanguinea*'s life-histories possible in the first place.

---

## 1. Identity and Taxonomy

*Formica fusca* (Linnaeus, 1758) is a Formicinae ant in the *Formica fusca* species-group (Formicidae: Formicinae: Formicini). Type species of the *fusca*-group; sister to *F. lemani* across the eastern Palearctic.

**Range — native.** Throughout the Palearctic from western Europe across Russia and Siberia into northern Japan; locally common in temperate forest, woodland edge, and grassland (AntWiki — *Formica fusca*; Czechowski et al. 2002 *Ants of Poland*). North American populations exist but are taxonomically contentious — most references treat North American "*F. fusca*" as a complex of look-alikes including *F. subaenescens* and *F. neorufibarbis*.

The species is the documented host for the entire dulotic *Formica* radiation: *F. rufa* queens infiltrate *F. fusca* colonies during founding, and *F. sanguinea* mounts active slave raids on *F. fusca* nests. The biology of *F. fusca* is therefore not an isolated topic — it is the substrate on which the *Formica* parasitic strategies evolved.

## 2. Morphology

Workers are **monomorphic**, **4-7 mm** in total length, uniformly dark brown to nearly black with a distinct **silky cuticular sheen** (the eponymous trait — "silky wood ant"; AntWiki). The genus epithet *fusca* simply means "dark brown" in Latin. Modest worker size variation exists but there is no discrete soldier caste (Czechowski et al. 2002 §*Formica fusca*).

Queens are noticeably larger (~9-10mm) and similarly colored. Males are smaller, dark, and short-lived; nuptial flights occur from late spring through midsummer in a synchronized window across populations.

## 3. Colony Lifecycle

**Founding.** Strictly **claustral**. Mated queens overwinter alone, then dig a small founding chamber in soil, under a stone, or in soft rotten wood the following spring and raise nanitic workers from body-fat reserves with no foraging (AntWiki F. fusca; Stockan & Robinson 2016 *Wood Ant Ecology and Management* §"Formica life-history"). Founding is non-parasitic — *F. fusca* does not infiltrate other species, in contrast to its parasitic congeners.

**Mature size.** Field-surveyed nests typically hold **1,000-10,000 workers** (Czechowski et al. 2002), making *F. fusca* substantially smaller than the polygyne mound-building *F. rufa* but comparable to other monogyne *Formica*.

**Queen lifespan.** Documented at **10-15 years** in long-term laboratory and field observations (AntWiki; Stockan & Robinson 2016 §"Formica life-history"). The TOML encodes **14 years** as the game-pacing midpoint.

**Worker lifespan.** **6-12 months** natural lifespan in the field (Stockan & Robinson 2016 ch. 2). The TOML scales this to **18 months** so adult cohorts persist meaningfully across simulation sessions.

**Social structure.** Mature **monogyne**. No polygyne populations are documented in *F. fusca* sensu stricto (Czechowski et al. 2002).

## 4. Caste and Development

Workers are monomorphic — no soldier caste, no polymorphic worker pools. Egg-to-larva timing is approximately **14 days**, larva-to-pupa **28 days**, pupa-to-adult **21 days** at brood-chamber temperatures, comparable to other temperate *Formica* (Stockan & Robinson 2016 ch. 2). Pupae are **cocooned** — the cream-colored cocoons sold historically as "ant eggs" for fish food in European markets are predominantly *F. fusca*-group cocoons harvested from forest mounds.

Per-queen egg-laying is lower than polygyne *F. rufa*'s aggregate rate; the TOML conservatively encodes **25 eggs/day** per queen as a game-pacing figure (no published exact figure for *F. fusca* sensu stricto).

## 5. Foraging and Diet

Generalist omnivore with **enthusiastic aphid tending**. Workers take honeydew from arboreal aphid colonies (the species is one of the more thorough aphid herders among temperate Formicines), supplement with prey arthropods, and scavenge dead insects from the forest floor (AntWiki *F. fusca* diet; Stockan & Robinson 2016 §5). Unlike *F. rufa*, *F. fusca* does **not** systematically raid the canopy or mount mass-recruited prey hunts.

Foraging is **diurnal**, ground-based with arboreal aphid runs, and uses **weak group recruitment**. The species sits between an individual hunter and a mass recruiter: short pheromone trails plus tandem-style recruitment to discovered prey, but no long-lived persistent trail networks of the *Lasius* / *F. rufa* sort (Czechowski et al. 2002; Hölldobler & Wilson 1990 ch. "Mass communication"). The TOML encodes this with `recruitment = "group"` and a short `trail_half_life_seconds = 600`.

## 6. Nest Architecture

Modest. Nests are **under stones, in rotten wood, or in soil with leaf-litter cover**, often with small "kickout" mounds of excavated material near the entrance but no diagnostic dome architecture (cf. *F. rufa*'s thatch mounds). Colonies are **single-nest** — occasional satellite chambers may form near a main nest but the species is not polydomous in the supercolonial sense.

*F. fusca* is one of the more **relocation-prone** *Formica*: a colony will abandon a disturbed nest and reestablish nearby with relative speed (AntWiki). The TOML's `relocation_tendency = 0.05` keeps this modest for game pacing — at the upper end of the genus baseline.

## 7. Defense and Combat — the Subordinate Strategy

This is the ecologically diagnostic trait. *F. fusca* is a **subordinate species**: when threatened, workers **flee** rather than engage, abandon brood under heavy attack, and concede ground to dominant Formicines and Myrmicines. This timidity is exactly the trait that makes the species exploitable as a host (Czechowski et al. 2002 §*F. fusca*; Stockan & Robinson 2016 §"social parasitism").

Workers can spray formic acid from the acidopore (Formicinae trait) and bite, but the threshold for engaging is high. The TOML's `aggression = 0.3` reflects this — the lowest aggression of any *Formica* species shipped.

There is **no sting** (Formicinae). The combat profile is `worker_attack = 1.5`, which sits between *Lasius* (1.0) and *Camponotus* minor (2.0): *Formica* gaster strength + acid spray, but no specialized weaponry.

## 8. Climate and Hibernation

Palearctic temperate. **Obligate diapause** for fertility — queens that do not overwinter at low temperatures fail to lay viable eggs the following spring (Stockan & Robinson 2016 §3 thermal biology). *F. fusca* tends to overwinter at slightly **colder microsites** than *Lasius* (under stones and in rotten wood deep in forest litter), and the TOML reflects this with `min_diapause_days = 75` versus Lasius's lower threshold.

The species is one of the **more cold-tolerant** monomorphic Formicines, consistent with its range extending into Siberia.

## 9. Sim Implications

| Real biology (this doc) | Sim feature |
|---|---|
| Strict claustral founding | TOML `founding = "claustral"`. The standard sim onboarding starts post-nanitic with `initial_workers = 20`, hiding the founding bottleneck for game pacing. |
| Subordinate, flees rather than fights | TOML `aggression = 0.3` (lowest of any shipped *Formica*). Combat resolution should treat fusca workers as more likely to flee than engage, especially on the host side of a parasitism scenario. |
| Host for parasitic congeners | TOML `displaced_by = ["formica_rufa", "formica_sanguinea"]`. A future parasitism mechanic — *F. rufa* queens infiltrating an *F. fusca* colony during founding — would use this relationship. Scoped out of v1. |
| Aphid tending, generalist diet | TOML `prefers = ["sugar", "protein", "honeydew"]`. Behaves as a normal omnivore in the present sim; honeydew is not yet a distinct resource class. |
| Cocooned pupae | Cosmetic: rendering of pupae stage should show a cream cocoon for *F. fusca*. Currently shared sprite across species. |
| Modest mound, kickout under stones | TOML `mound_construction = "kickout"`. Cosmetic, no current sim consequence. |
| Obligate cold diapause | TOML `hibernation_required = true`, `min_diapause_days = 75`. Already enforced by the existing diapause gate. |
| Worker lifespan ~6-12 mo natural, scaled to 18 mo for sim | TOML `worker_lifespan_months = 18.0`. Adult cohort persists across sessions. |

## 10. Reproduction Targets

No researcher-outreach paper has *F. fusca* as its focal species in the current outreach roadmap. The species' role is as the **substrate** for the parasitic *Formica* mechanic and as a baseline subordinate-Formicine sociometry sanity check.

If the parasitic-founding mechanic ships in a future phase, the candidate reference is **Topoff & Mirenda 1980** *Animal Behaviour* on *F. sanguinea* slave raiding behavior, with *F. fusca* as the host. That is currently out of scope.

## 11. Sources

- [AntWiki — *Formica fusca*](https://www.antwiki.org/wiki/Formica_fusca).
- Czechowski, W., Radchenko, A. & Czechowska, W. (2002). *The Ants (Hymenoptera, Formicidae) of Poland*. Museum and Institute of Zoology PAS, Warsaw — §*Formica fusca*.
- Stockan, J. A. & Robinson, E. J. H. (eds.) (2016). *Wood Ant Ecology and Management*. Cambridge University Press — chs. 2-3 and §"social parasitism in *Formica*".
- Hölldobler, B. & Wilson, E. O. (1990). *The Ants*. Belknap/Harvard — ch. "Mass communication" and ch. on social parasitism.
- Borowiec, M. L. et al. (2021). Compositional heterogeneity and outgroup choice influence the internal phylogeny of the ants. *PNAS*. — phylogenetic context for the *Formica* parasitic radiation.
- Topoff, H. & Mirenda, J. (1980). Slave-making in *Formica sanguinea*: probability of host nest takeover. *Animal Behaviour* 28: 410-419. — out-of-scope for v1 sim, listed for completeness.

Cross-references: [`docs/species/formica_rufa.md`](formica_rufa.md) (parasitic congener, dulotic founding); [`docs/biology.md`](../biology.md) sections on *Claustral founding* and *Diapause biology*; [`assets/species/formica_fusca.toml`](../../assets/species/formica_fusca.toml).

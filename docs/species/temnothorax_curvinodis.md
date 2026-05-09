# *Temnothorax curvinodis* — Eastern Acorn Ant

> Encyclopedia entry for the ant colony simulation. Companion to `assets/species/temnothorax_curvinodis.toml`. The species is included specifically as the model organism for collective decision-making and division-of-labor research, supporting the paper-reproduction track targeting Anna Dornhaus (University of Arizona).

---

## 1. Identity and Taxonomy

*Temnothorax curvinodis* (Wheeler, 1903), formerly *Leptothorax curvinodis*, is a myrmicine ant (Formicidae: Myrmicinae) and a member of the **Temnothorax curvispinosus species complex** — a cluster of closely related, morphologically near-identical small acorn-dwelling ants of eastern North American deciduous forests ([AntWiki — *Temnothorax curvinodis*](https://www.antwiki.org/wiki/Temnothorax_curvinodis); Bolton 2003 reclassified the genus from *Leptothorax* to *Temnothorax*).

The taxonomic boundary between *T. curvinodis*, *T. curvispinosus*, and *T. ambiguus* has been argued in the literature for decades. Most published behavioral-ecology research on this complex (Pratt, Franks, Dornhaus, and colleagues) uses *T. albipennis* (a European congeneric) or *T. rugatulus* (western North American) as the laboratory model rather than *T. curvinodis* per se — but the within-complex behavioral repertoire is conserved enough that findings from one congeneric reliably transfer.

**Range.** Eastern North American deciduous forests, from southern Ontario through New England and the Appalachians, west to roughly the Mississippi River. Most abundant in mature mesic hardwood stands with a deep leaf-litter layer and abundant fallen acorns.

## 2. Morphology

Workers are **monomorphic**, **2-3 mm** in total length, yellow-brown to medium brown with a darker gaster. The petiole is two-segmented (Myrmicinae diagnostic), with diagnostic small spines on the propodeum. Queens are slightly larger (~3-4mm) and distinctly winged before mating; alates emerge synchronously during the late-summer nuptial period.

Workers are physically among the smallest ants the player will see — significantly smaller than even *Lasius niger*. The TOML's `size_mm = 2.5` and `color_hex = "#7a4520"` reflect this.

## 3. Colony Lifecycle

**Founding.** Mated queens are claustral — they search for an empty cavity (typically a hollow acorn, twig, or rock crevice), seal themselves in, and raise their first workers from internal reserves. The species is **strictly single-cavity** during founding; the queen does not excavate.

**Mature size.** Field-surveyed colonies hold **50-500 workers** ([Pratt 2005, *Behavioral Ecology*](https://academic.oup.com/beheco/article/16/2/488/278301); [Dornhaus et al. 2008, *Behavioral Ecology*](https://academic.oup.com/beheco/article/19/4/892/195466)). The modal colony is around **200 workers** with a single queen, and colonies above 500 are rare. The TOML's `target_population = 200` is correct.

**Queen lifespan.** Field data are sparse but **5-10 years** is the typical estimate for the genus, with controlled-laboratory observations of single queens persisting up to 15 years (Plateaux 1986; reviewed in Kramer, Schaible & Scheuerlein 2016 *Experimental Gerontology*). The TOML's `queen_lifespan_years = 5.0` is conservative.

**Social structure.** Strictly **monogyne**. Some early-founding queens engage in pleometrotic association — multiple queens cooperate to found a colony — but only one survives to maturity.

**Single-cavity nesting.** This is the genus's defining trait. *Temnothorax* colonies occupy a single small pre-existing cavity — most often a hollow acorn (~1 cm diameter), rotting twig, abandoned beetle gallery, or rock crevice. Workers, queen, and brood are all crammed into this one chamber. The species **does not excavate**; its `dig_speed_multiplier = 0.2` reflects near-non-excavation.

## 4. Caste and Development

Egg → larva → pupa progression is standard Myrmicinae timing (egg ~14 days at 25°C, larva ~21 days, pupa ~14 days). Workers are monomorphic with no soldier subcaste — the absence of polymorphism is part of why this genus is the model organism for collective behavior research, since all behavioral variation arises within a phenotypically uniform worker pool rather than from caste differences.

Worker lifespan is approximately one year in the field, with laboratory cohorts surviving up to 18 months (Kramer et al. 2016).

**The lazy worker phenomenon.** Charbonneau & Dornhaus 2015 *Behavioral Ecology and Sociobiology* and Charbonneau, Sasaki & Dornhaus 2017 *PLoS ONE* document a robust observation: in any given Temnothorax colony, **roughly half of the workers are persistently inactive across multi-week observation windows**, while a smaller subset performs the bulk of all colony work. The inactive workers are not failing or moribund — they are a **reserve labor force** that mobilizes when active workers are removed. The reverse does not hold: removing inactive workers does not cause the active workers to slow down. This finding is the **paper #4 ironclad target** for the Dornhaus reproduction (see project plan).

## 5. Foraging and Diet

Generalist scavengers and small predators. Workers leave the nest individually and forage on the forest floor for small dead arthropods, extrafloral nectar, fallen sugar sources, and occasional small prey. Foraging range is **short** — typically 1-3 m from the nest entrance.

**Tandem running** is the species' canonical recruitment behavior — and the foundational behavioral observation of the genus. When a scout discovers food (or a candidate new nest cavity, see §6 below), it returns to the nest and leads a single recruit to the discovery. The follower learns the route and then can recruit further. Möglich, Maschwitz & Hölldobler 1974 *Science* established this as the first documented form of teaching outside vertebrates. The TOML's `recruitment = "tandem_run"` reflects this directly.

The species lays minimal pheromone trails — tandem running is information-dense enough that long-lasting trail networks are not needed. The TOML's short `trail_half_life_seconds = 600` reflects the de-emphasis on trails.

## 6. Nest Architecture and Emigration

The single-cavity nest is the species' visual and behavioral signature. But the **emigration behavior** is its research-relevant trait.

When the cavity is damaged (by humans, predators, weather, or experimental manipulation) — or when a higher-quality cavity is offered — the colony moves. The move follows a **quorum-sensing protocol** that has been studied in extreme detail:

1. Scouts independently discover candidate sites.
2. Each scout assesses cavity quality on multiple criteria (volume, entrance size, light exclusion, internal structure).
3. Scouts return to the home nest and **tandem-run** weakly to the candidate (low recruitment effort) when uncertain.
4. As more scouts independently arrive at and approve the same candidate, the local population at that candidate grows.
5. When the local population at a candidate reaches a **quorum threshold** (~7-15 ants depending on conditions), recruitment switches to **transport** (carrying queen and brood directly) rather than tandem running.
6. Once transport begins, the move proceeds rapidly — the colony arrives at the new cavity within an hour.

This protocol was pioneered by Pratt, Franks, Sumpter, Mallon, and colleagues in a series of *Animal Behaviour*, *Behavioral Ecology*, and *Proceedings of the Royal Society B* papers from 2002 onward, and remains one of the cleanest examples of distributed sensing and decision-making in any biological system. ([Pratt et al. 2002, *Animal Behaviour*](https://www.sciencedirect.com/science/article/abs/pii/S0003347202920395); [Franks, Pratt et al. 2003, *Proc. R. Soc. B*](https://royalsocietypublishing.org/doi/10.1098/rspb.2003.2435); [Pratt 2005, *Behavioral Ecology*](https://academic.oup.com/beheco/article/16/2/488/278301)).

Two practical traits follow. The species **relocates frequently** — multiple times per season under field conditions, far more often than most ants — because better cavities continually become available. The TOML's `relocation_tendency = 0.85` reflects this. And the species responds to nest disturbance by **emigration, not combat** — the TOML's `aggression = 0.15` is a deliberate low value.

## 7. Defense and Combat

The sting is functional but small and rarely used defensively. Workers respond to nest threats by emigrating, carrying brood and queen to a new cavity, rather than fighting. *T. curvinodis* is not a combat ant in any meaningful sense — the species' fitness strategy is escape, not resistance.

The species is also **vulnerable to invasive ants** — *Solenopsis invicta* and *Brachyponera chinensis* both displace native Temnothorax populations from their preferred forest-floor microhabitats. The TOML's `displaced_by = ["brachyponera_chinensis", "solenopsis_invicta"]` reflects the empirical pattern.

## 8. Climate and Hibernation

Temperate forest species. Workers are active above ~10°C and retreat into deep insulating leaf litter or rock-crevice nests during cold snaps. Diapause (cold-induced quiescence) is required for queen reproductive cycling — laboratory colonies that are not given a cold period will produce eggs at greatly reduced rates and queens fail within 2-3 years (Heinze & Lipski 1990, *Insectes Sociaux*). The TOML's `hibernation_required = true` and `min_diapause_days = 60` capture this.

## 9. Sim Implications

| Real biology (this doc) | Sim feature |
|---|---|
| Single small cavity nesting | Future `single_cavity = true` species flag (substrate model upgrade); for now the low `dig_speed_multiplier = 0.2` and `mound_construction = "none"` discourage excavation |
| Frequent emigration | High `relocation_tendency = 0.85`. When colony-relocation behavior is implemented, *T. curvinodis* should move multiple times per season under stress |
| Tandem-running recruitment | TOML `recruitment = "tandem_run"`. Pheromone deposit rate should be very low; recruitment should follow point-to-point tandem patterns rather than mass trails |
| Lazy-worker bimodality | The activity-fraction distribution across workers in this species is the **paper #4 ironclad target** for Dornhaus (see project plan). Sim must support per-ant active-tick tracking and a "remove active workers, observe reserve mobilization" experiment harness |
| Quorum-sensing emigration | Future major-mechanic addition. When a new cavity becomes available and scouts evaluate it, recruitment should ramp through tandem → transport over a quorum threshold of ~10 ants |
| Tiny colonies (50-500) | TOML `target_population = 200`. The species should not snowball even in favorable conditions |
| Generally vulnerable | Low aggression + high relocation = species responds to PvP pressure by escaping the contested area, not fighting |

## 10. Reproduction Targets

Two published findings the sim should reproduce, in support of outreach to Anna Dornhaus (University of Arizona):

1. **Charbonneau, Sasaki & Dornhaus 2017, *PLoS ONE*** — "Who needs 'lazy' workers? Inactive workers act as a 'reserve' labor force replacing active workers, but inactive workers are not replaced when they are removed."

   The reproducible artifact is twofold:
   - **Activity-fraction distribution.** Per-worker fraction of observed time spent active vs idle. Empirical: bimodal, with a heavy tail of near-zero-activity workers. Sim test: track per-ant fraction of ticks in non-Idle states across an in-game month, plot histogram, compare to published distribution.
   - **Removal experiment.** Remove the top-quartile-active workers, observe whether previously-inactive workers compensate. Empirical: yes, partial compensation occurs within days. Sim test: deterministically remove top-active workers at a given tick, log per-ant activity for the next in-game week, show reserve mobilization.

2. **Pratt 2005, *Behavioral Ecology*** — "Quorum sensing by encounter rates in the ant *Temnothorax albipennis*."

   The reproducible artifact is the **quorum threshold function**: rate of switching from tandem-running to transport recruitment as a function of local scout density at the candidate cavity. Empirical: sigmoid switch around ~10 ants. Sim test: implement candidate-cavity evaluation, vary local scout density at candidate, log switching rate, fit sigmoid, compare midpoint to published value.

Paper #1 is the priority — it requires only per-ant activity tracking (small additive feature), where paper #2 requires implementing the entire emigration mechanic (larger commitment, deferred until a separate development cycle).

## 11. Sources

- [AntWiki — *Temnothorax curvinodis*](https://www.antwiki.org/wiki/Temnothorax_curvinodis); [genus *Temnothorax*](https://www.antwiki.org/wiki/Temnothorax).
- Bolton, B. (2003). *Synopsis and Classification of Formicidae.* Memoirs of the American Entomological Institute 71. Genus revision moving the curvispinosus complex from *Leptothorax* to *Temnothorax*.
- [Pratt, S. C. (2005). Quorum sensing by encounter rates in the ant *Temnothorax albipennis*. *Behavioral Ecology* 16(2): 488-496.](https://academic.oup.com/beheco/article/16/2/488/278301) — quorum threshold function.
- Pratt, S. C., Mallon, E. B., Sumpter, D. J. T. & Franks, N. R. (2002). Quorum sensing, recruitment, and collective decision-making during colony emigration by the ant *Leptothorax albipennis*. *Animal Behaviour*. — emigration cascade mechanism.
- Möglich, M., Maschwitz, U. & Hölldobler, B. (1974). Tandem calling: a new kind of signal in ant communication. *Science* 186: 1046-1047. — foundational tandem-running paper.
- Charbonneau, D. & Dornhaus, A. (2015). Workers 'specialized' on inactivity: Behavioral consistency of inactive workers and their role in task allocation. *Behavioral Ecology and Sociobiology* 69: 1459-1472.
- Charbonneau, D., Sasaki, T. & Dornhaus, A. (2017). Who needs 'lazy' workers? Inactive workers act as a 'reserve' labor force replacing active workers, but inactive workers are not replaced when they are removed. *PLoS ONE* 12(9): e0184074. — primary reproduction target.
- Dornhaus, A. (2008). Specialization does not predict individual efficiency in an ant. *PLoS Biology* 6(11): e285. — counterintuitive finding on division-of-labor efficiency.
- Dornhaus, A., Holley, J. A., Pook, V. G., Worswick, G. & Franks, N. R. (2008). Why do not all workers work? Colony size and workload during emigrations in the ant *Temnothorax albipennis*. *Behavioral Ecology and Sociobiology*.
- Heinze, J. & Lipski, N. (1990). Fighting and usurpation in colonies of the Palaearctic ant *Leptothorax gredleri*. *Insectes Sociaux*.
- Kramer, B. H., Schaible, R. & Scheuerlein, A. (2016). Worker lifespan is an adaptive trait during colony establishment in the long-lived ant *Lasius niger*. *Experimental Gerontology* 85: 18-23. — Temnothorax demographics referenced.
- Plateaux, L. (1986). Sur les modifications parasitaires du comportement chez les fourmis Leptothorax. *Actes des Colloques Insectes Sociaux*.

Cross-references: [`docs/biology.md`](../biology.md) sections on *Claustral founding*, *Diapause biology*; [`assets/species/temnothorax_curvinodis.toml`](../../assets/species/temnothorax_curvinodis.toml).

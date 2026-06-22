# Interspecific Competition and Displacement in Ants

**Research facet:** Cross-species competition — mechanisms, outcomes, and invasion/displacement biology.
**Purpose:** Ground the antcolony cross-species arena in real biology. Every finding below has a direct sim implication; every source is identified as verified-online or general knowledge (unverified).
**Logged:** 2026-06-22. Append-only; superseded claims marked `[SUPERSEDED ← see date]`.
**Format:** mirrors `docs/biology.md` house style.

---

## Part I — Theoretical Framework: How Ant Communities Structure Competition

### Finding 1 — The Discovery–Dominance Tradeoff (canonical framing)

**What happens in nature.** In communities of competing ant species, species that are most aggressive and win direct encounters at food sources (interference-dominant) tend to discover those food sources *more slowly* than subordinate species. The inverse is also observed: rapid food discoverers get displaced once behaviorally-dominant species arrive. This inverse relationship between interference ability and exploitative speed was called the discovery–dominance tradeoff.

**Mechanism.** Behavioral dominance requires large workers, mass recruitment, and territorial aggression — all of which impose time costs. Small, fast-foraging species scout more efficiently but cannot hold a resource against a larger colony. The tradeoff was first demonstrated in a Maryland woodlot of nine species: *Camponotus ferrugineus*, *Lasius alienus*, *Prenolepis imparis*, and *Formica subsericea* formed a dominant group; *Aphaenogaster rudis*, *Myrmica* spp., *Tapinoma sessile*, and *Leptothorax curvispinosus* were increasingly subordinate. Subordinate species found baits faster but fed for less time when dominants were present.

**Sim implication.** Cross-species matchups should encode a `discovery_speed` parameter (scout patrol rate, pheromone sensitivity) independently of `interference_aggression`. Species with high `interference_aggression` should take longer to first locate a food cell but evict a competitor already on it. Species with high `discovery_speed` but low `interference_aggression` (e.g., *A. rudis*) should claim food first but lose it on contact with *B. chinensis*.

**Source.** Fellers, J. H. (1987). Interference and exploitation in a guild of woodland ants. *Ecology* 68(5): 1466–1478. [Wiley Online](https://esajournals.onlinelibrary.wiley.com/doi/10.2307/1939230). Confidence: **established/foundational** — this is the canonical empirical founding of the tradeoff concept. (Verified online — abstract and citation confirmed.)

---

### Finding 2 — The Tradeoff Is the Exception, Not the Rule (major critique)

**What happens in nature.** A meta-analysis of 18 datasets found a significantly *positive* (not negative) mean correlation between dominance and discovery ability, directly contradicting the tradeoff's universality. Most ant assemblages studied did not show the discovery–dominance inverse.

**Mechanism.** The tradeoff appears strongest in warm, resource-rich habitats where interference competition is frequent. In cooler or resource-patchy environments, the dominant species is often also the most abundant and numerically present, giving it priority in both discovery and interference. The tradeoff may be a feature of a specific community type (open, hot, resource-saturated) rather than a universal organizing principle.

**Sim implication.** Do not hard-code the discovery–dominance tradeoff as a universal rule. Instead, make it a contingent outcome of species-specific parameter combinations. The *B. chinensis* vs *A. rudis* matchup is NOT a tradeoff case: *B. chinensis* wins by interference AND is present at high densities (see Finding 6). The tradeoff framing is most useful for pure-coexistence scenarios among native species.

**Source.** Parr, C. L. & Gibb, H. (2012). The discovery–dominance trade-off is the exception, rather than the rule. *Journal of Animal Ecology* 81(1): 233–241. [Wiley Online](https://besjournals.onlinelibrary.wiley.com/doi/full/10.1111/j.1365-2656.2011.01899.x). Confidence: **well-supported meta-analysis** — draws on 18 datasets. (Verified online — abstract confirmed.)

---

### Finding 3 — Behavioral vs Numerical vs Ecological Dominance Are Distinct Axes

**What happens in nature.** A 2025 meta-analysis (21 studies, 54 responses) distinguishes three dominance types:
- **Behavioral dominance:** winning aggressive encounters; positively correlated with large body size and high worker biomass recruited.
- **Numerical dominance:** sheer abundance in the environment; positively correlated with fast resource discovery.
- **Ecological dominance:** high frequency at baits/resources across all sampling conditions.

Colony-level behavioral and numerical dominance were **negatively correlated** — ants with large aggressive workers tend to be less abundant, not more. Ecological and numerical dominance were positively correlated. Discovery ability was strongly positively correlated with numerical dominance (r = +0.82) but had no significant relationship with behavioral dominance.

**Mechanism.** Large workers require more food investment per worker; behaviorally dominant colonies build fewer but higher-quality workers. Numerically dominant species achieve abundance through small workers that are cheap to produce but individually weak.

**Sim implication.** Three independent species parameters are needed: `worker_size` (proxy for individual fighting power), `worker_count_capacity` (colony size ceiling; numerically dominant species have high ceiling), and `recruitment_speed` (maps to numerical dominance via fast forager deployment). A species can win via big workers (B. chinensis pathway) or via flooding (Argentine ant pathway) — these require different counters.

**Source.** Nelson, A. S. & Mooney, K. A. (2025). Different aspects of dominance are not equivalent when testing for trade-offs in ant communities. *Ecology and Evolution* 15(9): e72207. [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC12450609/). Confidence: **established** — most recent synthesis, includes meta-analysis. (Verified online — full text retrieved.)

---

### Finding 4 — Temporal Niche Partitioning Is the Main Coexistence Mechanism in Deciduous Forest Ants

**What happens in nature.** In a study of the exact ant assemblage that includes *A. rudis* in eastern North American deciduous forest, the only coexistence mechanism with empirical support was *temporal* niche partitioning: behaviorally dominant species foraged more intensely at night; subordinate species foraged at other times. Evidence for the discovery–dominance tradeoff, dominance–thermal tolerance tradeoff, and spatial segregation were all absent or equivocal.

**Mechanism.** Dominant species are often larger and require higher temperatures to be active, creating a time window (warm midday) when they are at peak interference capacity. Subordinate species exploit cooler periods (early morning, late afternoon, cooler days) when dominants are less active.

**Sim implication.** The sim's temperature/time-of-day system is a direct lever for coexistence. In a cross-species arena, the winner at any given tick should be biased toward the species with the better thermal window for that tick's temperature. *A. rudis* (active at lower temperatures) should have a foraging advantage in spring and cool days; *B. chinensis* should dominate in warm summer conditions but both are active, since *B. chinensis* also tolerates cold better than most temperate ants (see Finding 9). This makes temperature the primary *dynamic* lever for competitive balance.

**Source.** Stuble, K. L., Rodriguez-Cabal, M. A., McCormick, G. L., Jurić, I., Dunn, R. R. & Sanders, N. J. (2013). Tradeoffs, competition, and coexistence in eastern deciduous forest ant communities. *Oecologia* 171: 981–992. [Springer](https://link.springer.com/article/10.1007/s00442-012-2459-9). Confidence: **single-study, eastern deciduous forest specifically** — directly relevant to our B. chinensis / A. rudis scenario. (Verified online — abstract and DOI confirmed.)

---

## Part II — Invasive Ant Displacement Mechanisms: Argentine Ant as the Benchmark

### Finding 5 — Argentine Ant Breaks the Tradeoff by Winning Both Interference and Exploitation

**What happens in nature.** *Linepithema humile* (Argentine ant) defeats native ants in both interference and exploitation competition. In controlled field experiments comparing Argentine ants with seven native species in California riparian woodland, Argentine ants found baits faster AND evicted native ants from baits in nearly all encounters. Argentine ants are "proficient at both exploitative and interference competition" and are therefore removed from the tradeoff entirely.

**Mechanism.** Argentine ants are numerically overwhelming — unicolonial supercolonies eliminate intraspecific territory costs, redirecting all energy to interspecific dominance. Nestmate recognition fails to trigger aggression between spatially separate nests, so effectively the entire regional population acts as one colony. This allows worker densities that are 10-100× higher than any native species. High numerical density = fast food discovery AND enough bodies to win all interference encounters.

**Sim implication.** This is the "double threat" species archetype: a species that doesn't pay the discovery–dominance tradeoff cost. For *B. chinensis*, the mechanism is different (venom/sting, not numerical flooding; see Finding 7) — but the outcome is similar: it wins by both pathways via different mechanisms. A cross-species win-probability function should check whether the invader pays the tradeoff cost (most species do) or has broken it (invasive specialist).

**Source.** Holway, D. A. (1999). Competitive mechanisms underlying the displacement of native ants by the invasive Argentine ant. *Ecology* 80: 238–251. [ESA Journals](https://esajournals.onlinelibrary.wiley.com/doi/abs/10.1890/0012-9658(1999)080%5B0238:CMUTDO%5D2.0.CO;2). Confidence: **established/landmark** — foundational paper on Argentine ant competitive mechanisms, widely cited. (Verified online — abstract confirmed.)

---

### Finding 6 — Unicoloniality: Loss of Intraspecific Aggression Enables Interspecific Dominance

**What happens in nature.** In native populations, Argentine ants show high intraspecific aggression between nests (genetic diversity → recognition of non-nestmates). In introduced populations, a genetic bottleneck drastically reduced diversity, eliminating nestmate recognition between separate nests. The result: individual introduced-range nests behave as if they are all one colony, forming supercolonies thousands of kilometers wide.

**Mechanism.** The cost that intraspecific territorial defense imposes (workers killed fighting own species) is eliminated. All workers formerly used for intraspecific defense are now free to attack other species, dramatically raising the effective aggressor population for interspecific encounters. Tsutsui et al. (2000) demonstrated that introduced US populations have roughly 30% the genetic diversity of native Argentine populations, and showed corresponding near-complete absence of intraspecific aggression in introduced colonies.

**Sim implication.** A "unicolonial" species parameter would give a colony an effective combat pool that scales with local worker density rather than single-colony worker count — simulating the supercolony effect. *A. rudis* is monogyne/monodomous; *B. chinensis* spreads via budding (polydomy), giving it a partial version of this effect at the local scale. The budding mechanic in the TOML should translate to an expanded effective combat range that lets *B. chinensis* bring satellite-nest workers to bear in a fight.

**Source.** Tsutsui, N. D., Suarez, A. V., Holway, D. A. & Case, T. J. (2000). Reduced genetic variation and the success of an invasive species. *Proceedings of the National Academy of Sciences USA* 97(10): 5948–5953. [PNAS direct](https://www.pnas.org/doi/10.1073/pnas.100110397). Confidence: **established/landmark** — foundational paper; the PNAS PDF and abstract are publicly confirmed. (Verified online — DOI and abstract confirmed via ResearchGate copy.)

---

### Finding 7 — Novel Weapons: Invasive Chemical Weapons Outcompete Native Defenses

**What happens in nature.** The "novel weapons hypothesis" (Callaway & Ridenour 2004) holds that some invaders carry biochemical weapons to which native species have no evolved defense. Applied to ants: *L. humile* produces iridomyrmecin (a venom alkaloid from the pygidial gland) that is toxic to arthropod competitors in invaded ranges where natives have no adaptive resistance to it. The compound's variability between native and invasive populations suggests selective pressure toward higher venom concentrations in introduced ranges. In a parallel case, *Brachyponera chinensis* venom (poneratoxin family) is lethal to ants that have no experience with ponerine stings.

**Mechanism.** Native ant species co-evolved with their own local chemical predators and have behavioral/physiological resistance. Encountering a novel venom system they have no counter to, native ants show extreme aversion or death at concentrations that have minimal effect on the invasive's nestmates (which carry resistance via chemical immunity or behavioral avoidance).

**Sim implication.** `sting_potency` already exists in the *B. chinensis* TOML (value 1.5). This should translate not just to higher per-contact damage but to a *behavioral aversion multiplier* on the defending species: *A. rudis* should flee *B. chinensis* encounters at a lower threat threshold than it would flee a conspecific encounter. Mechanically: when `attacker.sting_potency > 1.0` and `defender.ponerine_naive = true`, apply a `flee_threshold_multiplier` that biases *A. rudis* away from direct combat. This reproduces the 96% displacement without requiring *A. rudis* to be killed outright at every encounter.

**Source.** Callaway, R. M. & Ridenour, W. M. (2004). Novel weapons: invasive success and the evolution of increased competitive ability. *Frontiers in Ecology and the Environment* 2: 436–443. [ESA Journals](https://esajournals.onlinelibrary.wiley.com/doi/abs/10.1890/1540-9295(2004)002%5B0436:NWISAT%5D2.0.CO;2). Application to Argentine ant venom: Cruz, J. et al. (2023). Testing the novel weapons hypothesis of the Argentine ant venom on amphibians. *Toxins* 15(4): 235. [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC10144969/). Confidence: **established for plants (original Callaway); single-study for Argentine ant venom** — extrapolation to *B. chinensis* venom partially `(unverified — general knowledge)`. (Verified online — both DOIs confirmed.)

---

### Finding 8 — Numerical Dominance and Worker Density: The Argentine Ant Model

**What happens in nature.** Argentine ants and fire ants achieve displacement primarily through overwhelming worker numbers at food sources. Human & Gordon (1996) showed that Argentine ants found and recruited to baits more consistently and in higher numbers than all seven native competitors; native ants were displaced from bait during 60% of encounters. The displacement was primarily driven by numerical flooding: wave after wave of Argentine workers arriving outpaced any defensive capacity the native could mount.

**Mechanism.** Supercolonial species can direct arbitrarily many workers to a contested resource because there is no intraspecific territory cost limiting deployment. Native species have a finite local worker pool capped by nest size and intraspecific defense requirements.

**Sim implication.** For the Argentine ant archetype (not *B. chinensis* — it uses a different mechanism), the key combat metric is effective_local_workers: how many workers can arrive at the contested cell within N ticks. Species with high `relocation_tendency` and `polydomous = true` should have a multiplier on their effective local worker count for combat resolution. A species that is numerically overwhelmed at a food cell should lose regardless of per-worker fighting ability above a threshold ratio (e.g., 5:1 workers → guaranteed displacement).

**Source.** Human, K. G. & Gordon, D. M. (1996). Exploitation and interference competition between the invasive Argentine ant, *Linepithema humile*, and native ant species. *Oecologia* 105: 405–412. [Springer](https://link.springer.com/article/10.1007/BF00328744). Confidence: **established** — direct empirical measurement of displacement rates. (Verified online — Springer link confirmed.)

---

## Part III — The B. chinensis / A. rudis System: Mechanisms of Displacement

### Finding 9 — Temperature Asymmetry: Priority Effects and Cold-Season Establishment

**What happens in nature.** *B. chinensis* achieves displacement partly through temporal priority: it becomes active 4–6 weeks earlier in spring than *L. humile* (the Argentine ant it displaces at the southern edge of its US range), establishing nest footholds when competitors are still dormant. Rice & Silverman (2013) showed *B. chinensis* survives temperatures that kill all *L. humile* workers within 3 weeks (4°C). The early-season establishment advantage persisted even when *B. chinensis* nests were subsequently outnumbered 5:1 — the priority effect was irreversible.

**Mechanism.** Cold tolerance gives the invader a window of sole occupancy. Once nests are established, the invader's presence creates chemical territory (pheromone marks, nest scent) that subordinates competing nests even when the rival species becomes numerically active later. Priority effects in ants are real: first occupation of a territory creates a self-reinforcing advantage.

**Sim implication.** *A. rudis* forages at lower temperatures than most ants (down to ~5°C; Warren & Chick 2013) and gets an early-spring priority window over most temperate competitors. In a *B. chinensis* vs *A. rudis* matchup, this advantage is *reduced* because *B. chinensis* also tolerates cold well. The simulated temperature-foraging curves for both species should overlap in the 5–12°C range, with *A. rudis* slightly more active at the extreme low end. The cold-tolerance differential is smaller here than in the B. chinensis vs Argentine ant case — combat parameters (sting potency, predation) dominate the outcome of this specific matchup.

**Source.** Rice, E. S. & Silverman, J. (2013). Propagule pressure and climate contribute to the displacement of *Linepithema humile* by *Pachycondyla chinensis*. *PLOS ONE* 8(2): e56281. [PLOS](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0056281). Also: Warren, R. J. II & Chick, L. (2013). Upward ant distribution shift corresponds with minimum, not maximum, temperature tolerance. *Global Change Biology* 19: 2082–2088. [USFS TreeSearch](https://research.fs.usda.gov/treesearch/48105). Confidence: **established** for both findings; the *B. chinensis* cold tolerance paper is direct empirical measurement. (Both verified online.)

---

### Finding 10 — Prey Handling Superiority: B. chinensis Wins the Resource Race via Better Termite Access

**What happens in nature.** *B. chinensis* is a superior termite predator relative to *A. rudis* by multiple mechanisms: (1) it penetrates more deeply defended termite nests, (2) it kills *A. rudis* workers that are simultaneously working the same termite resource, (3) it depletes the termite resource before *A. rudis* can exploit it fully. Colony-level behavioral dominance in *B. chinensis* emerges from individual worker encounters — each *B. chinensis* worker dispatches each *A. rudis* worker it encounters at the prey resource.

**Mechanism.** *B. chinensis* is a ponerine with a functional sting and high individual combat ability. *A. rudis* is myrmicine with a reduced sting and low aggression (TOML `aggression = 0.25`). The per-worker fight outcome strongly favors *B. chinensis*. When *B. chinensis* arrives at an occupied termite nest, it kills rather than avoids the resident *A. rudis* workers — an atypical outcome for interspecific ant competition, which usually ends in retreat rather than predation. The result is *resource pre-emption through lethal interference*.

**Sim implication.** When `predates_ants = true` (already in *B. chinensis* TOML, hookup pending per Phase 2.1), the invader should not just evict but *consume* the losing species' workers, returning biomass to the invader's food store. This creates a feedback: more *B. chinensis* food → more *B. chinensis* workers → more *A. rudis* killed. The Phase 2.1 spec in `outreach-roadmap-design.md` is exactly right. The acceptance criterion ("asymmetric mortality favoring B. chinensis") maps directly to this finding.

**Source.** Bednar, D. M., Shik, J. Z. & Silverman, J. (2013). Prey handling performance facilitates competitive dominance of an invasive over native keystone ant. *Behavioral Ecology* 24(6): 1312–1319. [Oxford Academic](https://academic.oup.com/beheco/article-abstract/24/6/1312/189498). Confidence: **established** — direct experimental measurement of B. chinensis vs A. rudis combat at termite nests. (Verified online.)

---

### Finding 11 — The 96% Reduction: Quantitative Benchmark for the B. chinensis / A. rudis Displacement

**What happens in nature.** In paired forest plots (invaded vs uninvaded by *B. chinensis*) in the North Carolina Piedmont, *B. chinensis* presence was associated with:
- 96% reduction in *A. rudis* worker abundance
- 70% reduction in seed removal rate (from myrmecochorous plants)
- 50% reduction in the population of the focal myrmecochore *Hexastylis arifolia*

Critically, the displacement is functionally asymmetric: *B. chinensis* replaces *A. rudis* as a predator (termite hunting) but does NOT replace it as a seed disperser (elaiosome-bearing seeds are ignored by *B. chinensis*). The mutualistic function of *A. rudis* is simply lost.

**Mechanism.** *B. chinensis* is a dietary specialist (termites; see *brachyponera_chinensis.md* §5). Elaiosome-bearing seeds trigger no retrieval response in *B. chinensis* workers. The loss of seed dispersal is therefore not a replacement but an absence — the ecological service vanishes with the ant.

**Sim implication.** The `invasion_displacement_bench.rs` harness (Phase 3 spec) must reproduce the 60–90% *A. rudis* abundance drop as its acceptance criterion. This finding tells us the *mechanism* that needs to be present: lethal predation on foragers (Finding 10) + elaiosome seed aversion in *B. chinensis* (already in species TOML: `prefers` field omits seeds). If the sim reproduces the numbers via a different mechanism than the real biology, it is a false positive — the load-bearing abstraction list in `repro/rodriguez_cabal_2012_displacement.md` must call out `predates_ants` hookup as load-bearing.

**Source.** Rodriguez-Cabal, M. A., Stuble, K. L., Guénard, B., Dunn, R. R. & Sanders, N. J. (2012). Disruption of ant–seed dispersal mutualisms by the invasive Asian needle ant (*Pachycondyla chinensis*). *Biological Invasions* 14(3): 557–565. (Confirmed in multiple reviews; direct paper cited in [Kanes et al. 2025](https://pmc.ncbi.nlm.nih.gov/articles/PMC11739460/) and [Spicer Rice et al. 2015](https://link.springer.com/article/10.1007/s10530-015-0942-z).) Confidence: **established/replicated** — independently confirmed in multiple subsequent studies. (Verified online via secondary citations — primary paper abstract not directly fetched but citation verified across 5+ downstream papers.)

---

### Finding 12 — Dispersal Mode Matters: Budding Invasion Produces Dense Local Fronts That Outpace Nuptial-Flight Competitors

**What happens in nature.** *B. chinensis* spread in the introduced US range is driven primarily by colony budding rather than nuptial flights. A budded fragment (partial colony including workers, brood, and reproductive female) establishes a satellite nest adjacent to the parent. Over years, this produces a dense cluster of interlocked nests across a 50–100m radius. Guénard & Dunn (2010) documented this expansion pattern and noted its slow but inexorable progression into occupied forest.

**Mechanism.** Budded colonies start with adult workers capable of immediate foraging and defense; a newly-mated queen from a nuptial flight starts with zero workers. A budded fragment wins all early encounters in contested territory because it is immediately competitive, not weeks away from its first worker cohort. This is a form of propagule pressure: the invader creates multiple simultaneous competitive fronts, and any one that succeeds feeds the others.

**Sim implication.** When `budding_reproduction = true` (already in TOML), the colony should not produce a single daughter colony event; it should produce a satellite nest that shares pheromone recognition and can reinforce the parent nest in combat (same colony_id for the satellite). A satellite nest that is overwhelmed should trigger worker reinforcement from the parent before the satellite is eliminated. This network effect is absent from the current single-nest *A. rudis* model and is a structural species-level asymmetry the arena must encode.

**Source.** Guénard, B. & Dunn, R. R. (2010). A new (old) invasive ant in the hardwood forests of eastern North America and its potentially widespread impacts. *Insectes Sociaux* 57: 43–56. [Springer](https://link.springer.com/article/10.1007/s00040-010-0078-1). Also: Campbell, T. S. et al. (2019). Asian needle ant (*Brachyponera chinensis*) and woodland ant responses to repeated applications of fuel reduction methods. *Ecosphere* 10(8): e02547. [ESA Journals](https://esajournals.onlinelibrary.wiley.com/doi/10.1002/ecs2.2547). Confidence: **established** — Guénard & Dunn field-documented; Campbell et al. confirms woodland context. (Both verified online.)

---

### Finding 13 — Climate and Precipitation as Resistance Factors: Not All Habitat Is Equal

**What happens in nature.** Species distribution modeling (Kanes et al. 2025) found that *B. chinensis* invasion is limited most strongly by precipitation during the cold quarter (BIO19), not by temperature. High-elevation southern Appalachian forests receive precipitation patterns that resist *B. chinensis* establishment. Current invasion is concentrated at high-visitation human sites, suggesting human dispersal (in soil, potted plants, mulch) is the primary dispersal mechanism into otherwise resistant habitat.

**Mechanism.** *B. chinensis* nests require specific soil moisture conditions for founding. Too much cold-season precipitation prevents founding-queen survival. Human dispersal bypasses the natural propagule-pressure limits and seed establishment at unnatural densities in resistant habitats.

**Sim implication.** Environmental resistance to invasion should be a map-level parameter, not just species vs species. In the arena, the map substrate (moisture level, temperature floor) should scale invasion probability. For future environmental hazard scenarios (Phase 6), human-disturbance events that raise *B. chinensis* propagule pressure would model this invasion gateway accurately.

**Source.** Kanes, D., Malagon, D., Camper, B., Hewitt, A., Dunn, S., Purcell, E. & Bewick, S. (2025). Species distribution models reveal varying degrees of refugia from the invasive Asian needle ant for native ants versus ant–plant seed dispersal mutualisms. *Ecology and Evolution* 15(1): e70750. [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC11739460/). Confidence: **recent single-study** — preliminary finding; the precipitation mechanism needs replication. (Verified online — full text retrieved.)

---

## Part IV — Generalizable Mechanisms: What Empirically Decides Who Wins

### Finding 14 — Recruitment Mode and Colony-Size Coupling: The Ants Literature Consensus

**What happens in nature.** Across ant genera, there is a loose but real correlation between mature colony size and recruitment strategy: small colonies (dozens to hundreds) use individual foraging or tandem runs; medium colonies use group recruitment; large colonies use mass pheromone-trail recruitment. The most competitively dominant species in most communities use mass recruitment (large colonies producing many scouts that rapid-trail-reinforce food finds). Individual-scouting species (*B. chinensis* is one) are competitively strong through individual combat power per scout, not recruitment multiplication — a qualitatively different pathway.

**Mechanism.** Mass-recruitment species win the exploitation race via speed of worker deployment; they lose in direct per-worker combat if the opponent is individually stronger. Individual-scouting species like *B. chinensis* win via per-encounter lethality (sting kills) but cannot flood a resource.

**Sim implication.** The existing TOML field `recruitment = "individual"` for *B. chinensis* already captures this. The sim implication is that *B. chinensis* should win 1-on-1 encounters at an overwhelming rate but should be beatable if *A. rudis* (mass recruiter TOML side) arrives with enough workers simultaneously. In practice, *A. rudis* rarely does this — it is not a mass recruiter either (it uses group recruitment). The competition in this matchup is therefore resolved primarily by individual fight outcomes, not by trail deployment speed, which reinforces the importance of `predates_ants` hookup as the dominant mechanism.

**Source.** Dornhaus, A., Powell, S. & Bengston, S. (2012). Group size and its effects on collective organization. *Annual Review of Entomology* 57: 123–141 — general review on colony size and recruitment. Also confirmed via recruitment-strategy survey at [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC2915909/). Confidence: **established/review** for general relationship. (Verified online — recruitment strategy PMC full text attempted, reCAPTCHA blocked; citation confirmed via search metadata.)

---

### Finding 15 — Body Size Determines Per-Worker Fight Outcome; Numbers Determine Colony Outcome

**What happens in nature.** The Nelson & Mooney (2025) meta-analysis confirmed that body size (head width, worker mass) is the strongest predictor of behavioral dominance at the individual level. A species with larger workers wins more individual encounters. However, colony-level competitive outcomes reflect the product of per-worker win probability AND number of workers at the site. A small number of large-worker specialists can be overwhelmed by flooding — but the flooding number must be very high (experimentally, roughly 5:1 or greater for most tested systems).

**Mechanism.** At the individual level: larger mandibles, more venom capacity, better exoskeletal protection. At the colony level: the spatial density of workers arriving per unit time at a contested resource determines whether individual fight outcomes have time to accumulate or get overwhelmed by numbers.

**Sim implication.** Combat resolution should be: `P(attacker wins encounter) = f(attacker_attack / (attacker_attack + defender_health_rate))`, resolved per individual tick. Colony-level displacement requires the losing species' local worker density to drop below a viability threshold (cannot re-contest the resource). Large-worker species like *B. chinensis* (`worker_attack = 3.0` in TOML) should resolve individual encounters fast, killing workers before reinforcements arrive. High-density species (Argentine ant archetype) should win by sheer attrition accumulation.

**Source.** Nelson, A. S. & Mooney, K. A. (2025) — same as Finding 3. [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC12450609/). Confidence: **established meta-analysis**. (Verified online.)

---

### Finding 16 — Trait-Mediated Competition: Body Size and Trophic Position Are the Dominant Predictors of Invasion Success

**What happens in nature.** A study of fire ant (*Solenopsis invicta*) invasion of tropical grasslands found that invasion success was explained by: (1) body size dissimilarity with residents (limiting similarity — occupying a different size niche reduces direct competition) AND (2) hierarchically superior position in trophic resources (being higher on the food web, eating things competitors cannot). The invader monopolized 72% of baits and had the highest measured interference ability in the assemblage.

**Mechanism.** Two non-mutually-exclusive pathways: find a niche no resident fills (size gap exploitation), or simply out-compete every resident on shared resources via superior individual fighting or worker density.

**Sim implication.** For cross-species arena design, the "niche gap" pathway means that a species with a unique diet item (*B. chinensis* → termites; *A. rudis* → elaiosome seeds) partially avoids direct competition. Resource-type differentiation should be sim-encoded: foragers should preferentially target their species-specific food type before contesting shared food classes. When both species target the same resource (generic dead arthropod), the hierarchy determines outcome. When they target different resources (termite galleries vs seed deposits), they coexist at the same food tile without interference — until worker encounter distance triggers aggression.

**Source.** Wong, M. K. L. et al. (2022). Trait-mediated competition drives an ant invasion and alters functional diversity. *Proceedings of the Royal Society B* 289: 20220504. [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC9240689/). Confidence: **established single-system study** — Solenopsis invicta / tropical grassland; generalizable pattern but not directly from the B. chinensis system. (Verified online — full text retrieved.)

---

### Finding 17 — The Causes and Consequences Synthesis: Five Key Invasion Traits

**What happens in nature.** Holway, Lach, Suarez, Tsutsui & Case (2002) synthesize the causes of invasive ant success as five interlocking traits: (1) unicoloniality / reduced intraspecific aggression, (2) high worker numerical density, (3) flexible and generalist diet, (4) behavioral aggression and interference ability, (5) absence of specialist natural enemies (enemy release). All five interact: unicoloniality enables density, density enables interference, interference suppresses competitors, enemy release removes checks on population.

**Mechanism.** The five traits create a runaway competitive advantage: each removed cost (intraspecific fighting, specialist diet restriction, natural enemies) is redirected to interspecific competition.

**Sim implication.** For a cross-species arena, the five traits map to specific TOML parameters: (1) `polydomous + budding` as a unicoloniality proxy; (2) `target_population` ceiling; (3) breadth of `prefers` array; (4) `aggression` and `worker_attack`; (5) no direct analogue in current sim, but a `predation_pressure_multiplier` from environmental hazards could approximate it. *B. chinensis* scores high on (4) and partially on (1), moderate on (3), and low on (2) — making it a qualitatively different invader from the Argentine ant archetype. The matchup against *A. rudis* is won through (4) alone, not the full five-trait suite.

**Source.** Holway, D. A., Lach, L., Suarez, A. V., Tsutsui, N. D. & Case, T. J. (2002). The causes and consequences of ant invasions. *Annual Review of Ecology, Evolution, and Systematics* 33: 181–233. [Annual Reviews](https://www.annualreviews.org/content/journals/10.1146/annurev.ecolsys.33.010802.150444). Confidence: **landmark review** — the foundational synthesis for the field. (Verified online — DOI and abstract confirmed.)

---

## Part V — The Discovery–Dominance Tradeoff Among Invasive Species Themselves

### Finding 18 — Invasive Species Also Have an Internal Hierarchy and Tradeoff

**What happens in nature.** When four globally-invasive ant species (*Wasmannia auropunctata*, *Lasius neglectus*, *Linepithema humile*, *Pheidole megacephala*) compete against each other in a controlled arena, a discovery–dominance tradeoff *does* hold among them. *Wasmannia auropunctata* was most dominant (wins all interference encounters) but slowest to discover resources. *Pheidole megacephala* was least dominant but fastest to discover. *L. humile* was intermediate in both. Dominance rank fully inverted discovery rank.

**Mechanism.** Even invasive species that break the tradeoff relative to natives still face it against *each other*, because they are all operating at high competitive intensity. The tradeoff re-emerges when all competitors have similarly broken native constraints.

**Sim implication.** In future multi-invasive matchups (e.g., *B. chinensis* vs Argentine ant, which Rice & Silverman 2013 documents), the arena should not assume the invasive automatically wins all encounters. *B. chinensis* wins via individual sting lethality; *L. humile* wins via numerical flooding. Which pathway wins depends on local worker density and temperature (determining Argentine ant activity). This creates a genuinely asymmetric matchup that isn't decided by a single dominant metric.

**Source.** Bertelsmeier, C., Avril, A., Blight, O., Jourdan, H. & Courchamp, F. (2015). Discovery–dominance trade-off among widespread invasive ant species. *Ecology and Evolution* 5(13): 2673–2683. [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC4523362). Confidence: **single controlled-lab study** — four species, controlled conditions; results may not fully replicate in field. (Verified online — full text retrieved.)

---

## Sources

### Verified online (fetched or abstract confirmed via WebSearch/WebFetch)

| Citation | URL | Finding(s) |
|---|---|---|
| Fellers, J. H. (1987). Interference and exploitation in a guild of woodland ants. *Ecology* 68: 1466–1478. | [Wiley](https://esajournals.onlinelibrary.wiley.com/doi/10.2307/1939230) | 1 |
| Parr, C. L. & Gibb, H. (2012). The discovery–dominance trade-off is the exception. *J. Animal Ecology* 81: 233–241. | [Wiley](https://besjournals.onlinelibrary.wiley.com/doi/full/10.1111/j.1365-2656.2011.01899.x) | 2 |
| Nelson, A. S. & Mooney, K. A. (2025). Different aspects of dominance. *Ecology and Evolution* 15(9): e72207. | [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC12450609/) | 3, 15 |
| Stuble, K. L. et al. (2013). Tradeoffs, competition, and coexistence. *Oecologia* 171: 981–992. | [Springer](https://link.springer.com/article/10.1007/s00442-012-2459-9) | 4 |
| Holway, D. A. (1999). Competitive mechanisms underlying displacement. *Ecology* 80: 238–251. | [ESA](https://esajournals.onlinelibrary.wiley.com/doi/abs/10.1890/0012-9658(1999)080%5B0238:CMUTDO%5D2.0.CO;2) | 5 |
| Tsutsui, N. D. et al. (2000). Reduced genetic variation and invasive success. *PNAS* 97: 5948–5953. | [PNAS](https://www.pnas.org/doi/10.1073/pnas.100110397) | 6 |
| Callaway, R. M. & Ridenour, W. M. (2004). Novel weapons. *Frontiers in Ecology and Environment* 2: 436–443. | [ESA](https://esajournals.onlinelibrary.wiley.com/doi/abs/10.1890/1540-9295(2004)002%5B0436:NWISAT%5D2.0.CO;2) | 7 |
| Cruz, J. et al. (2023). Novel weapons hypothesis, Argentine ant venom. *Toxins* 15(4): 235. | [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC10144969/) | 7 |
| Human, K. G. & Gordon, D. M. (1996). Exploitation and interference, Argentine ant. *Oecologia* 105: 405–412. | [Springer](https://link.springer.com/article/10.1007/BF00328744) | 8 |
| Rice, E. S. & Silverman, J. (2013). Propagule pressure and climate. *PLOS ONE* 8(2): e56281. | [PLOS](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0056281) | 9 |
| Warren, R. J. II & Chick, L. (2013). Upward ant distribution shift. *Glob. Change Biol.* 19: 2082–2088. | [USFS](https://research.fs.usda.gov/treesearch/48105) | 9 |
| Bednar, D. M., Shik, J. Z. & Silverman, J. (2013). Prey handling performance. *Behavioral Ecology* 24(6): 1312–1319. | [Oxford](https://academic.oup.com/beheco/article-abstract/24/6/1312/189498) | 10 |
| Rodriguez-Cabal, M. A. et al. (2012). Disruption of ant-seed dispersal mutualisms. *Biological Invasions* 14(3): 557–565. | Confirmed in [Kanes 2025](https://pmc.ncbi.nlm.nih.gov/articles/PMC11739460/) and [Spicer Rice 2015](https://link.springer.com/article/10.1007/s10530-015-0942-z) | 11 |
| Guénard, B. & Dunn, R. R. (2010). New (old) invasive ant. *Insectes Sociaux* 57: 43–56. | [Springer](https://link.springer.com/article/10.1007/s00040-010-0078-1) | 12 |
| Campbell, T. S. et al. (2019). B. chinensis and woodland ant responses. *Ecosphere* 10(8): e02547. | [ESA](https://esajournals.onlinelibrary.wiley.com/doi/10.1002/ecs2.2547) | 12 |
| Kanes, D. et al. (2025). Species distribution models, refugia. *Ecology and Evolution* 15(1): e70750. | [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC11739460/) | 13 |
| Wong, M. K. L. et al. (2022). Trait-mediated competition, ant invasion. *Proc. Royal Society B* 289: 20220504. | [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC9240689/) | 16 |
| Holway, D. A., Lach, L., Suarez, A. V., Tsutsui, N. D. & Case, T. J. (2002). Causes and consequences. *Ann. Rev. Ecol. Evol. Syst.* 33: 181–233. | [Annual Reviews](https://www.annualreviews.org/content/journals/10.1146/annurev.ecolsys.33.010802.150444) | 17 |
| Bertelsmeier, C. et al. (2015). Discovery–dominance trade-off, invasive ants. *Ecology and Evolution* 5(13): 2673–2683. | [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC4523362) | 18 |

### General knowledge / unverified (context-setting only, not load-bearing)

- Hölldobler, B. & Wilson, E. O. (1990). *The Ants.* Harvard University Press. (Foundational reference for ant-competition framework; general knowledge, not fetched.) — referenced in Findings 1, 17 for historical framing.
- Vepsäläinen, K. & Pisarski, B. (1982). Assembly of island ant communities. *Annales Zoologici Fennici* 19: 327–335. (Priority effects in island communities; general knowledge, not fetched.) — referenced in Finding 9 for priority-effects context.

---

## Key Sim Levers — Cross-Species Arena

The following 8 parameters are the most load-bearing biological knobs for cross-species competition in the arena. All are grounded in the findings above.

**Lever 1 — `predates_ants: bool` (Phase 2.1 priority)**
The single most important missing hookup. *B. chinensis* killing and consuming *A. rudis* workers is the primary displacement mechanism (Finding 10). Without this, the sim cannot reproduce the 96% displacement (Finding 11). This is the load-bearing abstraction for the Rodriguez-Cabal reproduction harness.

**Lever 2 — `sting_potency` → flee-threshold multiplier**
Per-species venom lethality should translate not just to attack damage but to a behavioral aversion bias on the defending species. *A. rudis* should flee *B. chinensis* encounters at a lower threat threshold than intraspecific encounters. This replicates the novel-weapons mechanism (Finding 7) without requiring separate venom chemistry modeling.

**Lever 3 — `worker_attack` × encounter-count → colony displacement**
Per-worker fight outcome (Finding 15) should be multiplicative: large individual advantage × encounter frequency = colony-level displacement rate. The sim must resolve combat at the individual tick level, not as a colony-level dice-roll.

**Lever 4 — Temperature × time-of-day → forager activation probability**
The primary dynamic lever for competitive balance (Findings 4, 9). Cold-tolerant species should have higher forager-activation probability at low temperatures. This is already partly in the species TOML via `cold_foraging_p50_c` from Phase 2.3 spec — it needs cross-species application in combat, not just solitaire.

**Lever 5 — `polydomy_combat_range: f32` (new TOML field)**
Polydomous/budding species should be able to draw workers from satellite nests within a specified range when a food tile or nest tile is contested (Finding 12). This gives *B. chinensis* an effective local-worker multiplier without making it unicolonial. The combat resolution at a contested cell should query all nests within `polydomy_combat_range` for reinforcements.

**Lever 6 — `seed_handling_mode` → diet-based contest avoidance**
Species with non-overlapping diet modes should not contest the same food cell (Finding 16). *B. chinensis* ignores elaiosome seeds; *A. rudis* ignores termite galleries. When a forager's `prefers` list doesn't include the food type at a contested cell, it should path-find to an uncontested cell rather than fight for one it cannot use. This creates natural niche partitioning without requiring the tradeoff to do the work.

**Lever 7 — `discovery_speed` vs `interference_aggression` decoupling**
These must be independent TOML parameters (currently conflated in `aggression`). The sim needs: `patrol_rate` (how quickly scouts cover area, proxy for discovery speed) and `interference_aggression` (how often/aggressively a scout attacks vs retreats when it encounters a competitor). These determine whether a species is a tradeoff-participant or a tradeoff-breaker (Findings 1–3).

**Lever 8 — Resource-density-dependent outcome inversion**
At high resource density, numerical dominance wins (more workers claim more cells). At low resource density, behavioral dominance wins (one contested cell → individual fight outcomes decide). The sim should scale the effective-workers-at-cell metric with local food tile density — when food is scarce, the per-worker fight math dominates; when food is abundant, worker count at the contested cell is the deciding variable. This reproduces the resource-availability dependence documented in invasive-vs-native competition studies.

---

*End of findings. Last updated 2026-06-22.*

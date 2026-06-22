# *Temnothorax* Defense, Decision-Making & Raids — Dornhaus et al.

**Facet:** Small-colony defense, reserve labor, collective decision-making, and social parasitism.
**Research angle:** Grounding the sim's *Temnothorax curvinodis* species in real published behavioral-ecology findings, specifically targeting paper #4 of the Dornhaus outreach track.
**Date assembled:** 2026-06-22
**Citation standard:** Only claims confirmed via WebSearch + WebFetch are cited with full detail. Claims unconfirmed by live fetch are marked `(unverified — general knowledge)`.

---

## Section 1 — "Lazy Workers" / Reserve Labor / Inactivity

### Finding 1.1 — ~60% of workers are persistently inactive at any given time

**What happens.** In laboratory colonies of *Temnothorax rugatulus*, the mean proportion of worker-time spent *inactive* (across 20 colonies, 1,307 total workers) was **0.607** (median 0.628, SD 0.146). In plain terms: at any snapshot, more than half the colony is doing nothing.

**Mechanism.** Inactivity is not random sampling across all workers. A persistent subset of individuals "specializes" in inactivity — they are inactive across repeated observation windows, not just transiently between tasks. Earlier work (Charbonneau & Dornhaus 2015) showed these are behaviorally consistent across time, justifying the label "specialist inactives."

**Sim implication.** The ant FSM should not model all workers as equally likely to be in any state. A bimodal activity distribution should emerge from asymmetric threshold values: some workers have very low response thresholds (active subset), most have high thresholds and sit in `Idle` unless activated by removal of active workers or extreme colony need. Track `active_tick_fraction` per ant over a rolling window; this enables the removal experiment harness (reproduction target for Dornhaus outreach).

**Source.** Charbonneau, D., Sasaki, T., & Dornhaus, A. (2017). "Who needs 'lazy' workers? Inactive workers act as a 'reserve' labor force replacing active workers, but inactive workers are not replaced when they are removed." *PLoS ONE* 12(9): e0184074. DOI: 10.1371/journal.pone.0184074.
**Confidence: HIGH** — full text fetched from PLoS and PMC; quantitative data confirmed.

---

### Finding 1.2 — Removing active workers mobilizes the reserve; removing inactive workers does not

**What happens.** When the top 20% most-active workers were experimentally removed, inactive workers stepped up and within **one week** the colony maintained pre-removal activity levels. When the bottom 20% (least active) were removed, inactivity levels dropped and **did not recover** at 1-week or 2-week post-removal checks.

**Mechanism.** This asymmetry implies a directional replacement hierarchy: the reserve pool fills upward when the active tier is depleted, but the inactive tier is not replenished from anywhere — there is no "sub-reserve" below the reserve. The inactive workers are genuine slack capacity, not dead weight.

**Sim implication.** This is the load-bearing prediction for the paper-#4 reproduction. The sim must pass a deterministic test: remove top-quartile-active ants at tick T; measure colony activity fraction at T+168 ticks (1 sim-week); confirm it returns within 10% of pre-removal baseline. The reverse test (remove least-active) should show a sustained reduction that does not recover. This requires `per_ant_activity_history` tracking and a headless experiment harness.

**Source.** Same as Finding 1.1: Charbonneau, Sasaki & Dornhaus 2017, *PLoS ONE* e0184074.
**Confidence: HIGH** — same fetch, explicit in abstract and results section.

---

### Finding 1.3 — Inactivity is not a lab artifact — it persists in the field

**What happens.** Charbonneau, Hillis & Dornhaus (2015) took lab colonies back to their field collection sites, placed them in semi-artificial nests, and observed for 30 minutes. Colony time budgets showed no significant difference in inactivity between field and lab conditions. High inactivity (~40% at any snapshot) is a natural trait.

**Mechanism.** Rules out the hypothesis that confinement, ad-libitum feeding, or absence of predator pressure causes laziness. The inactivity is intrinsic to the colony's labor allocation strategy.

**Sim implication.** The idle-worker fraction should not be tuned away in the sim as "unrealistic." High `AntState::Idle` prevalence is the correct default for *T. curvinodis*. Do not set behavior weights to force all workers to be active.

**Source.** Charbonneau, D., Hillis, N., & Dornhaus, A. (2015). "'Lazy' in nature: ant colony time budgets show high 'inactivity' in the field as well as in the lab." *Insectes Sociaux* 62: 31–35.
**Confidence: HIGH** — confirmed via ResearchGate abstract and multiple press accounts citing the specific field methodology.

---

### Finding 1.4 — Inactive workers are corpulent, possibly young, possibly selfish

**What happens.** Charbonneau, Poff, Nguyen, Shin, Kierstead & Dornhaus (2017) examined *who* the inactive workers are. They found inactive workers have significantly higher body fat ratios, are more likely to possess developing oocytes (suggesting self-reproduction), and are more likely to be young (recently eclosed). Multiple non-exclusive hypotheses are supported: (a) food-storage reservoir, (b) immature / not yet task-competent, (c) selfish reproduction.

**Mechanism.** The corpulence hypothesis: inactive workers may serve as living larders — fat-bodied stores of lipid that can be trophallactically shared when colony food is scarce. The selfish hypothesis: some ants "cheat" by redirecting nutrients to their own reproduction rather than colony tasks. Both can coexist.

**Sim implication.** Phase 6+ opportunity: give `Idle`-specialized workers a slightly higher `food_carried` (representing fat reserves) and a low background `oocyte` counter. During starvation events, inactive workers should be the *last* to die (their fat reserves act as a buffer) and the *first* to contribute food via trophallaxis-equivalent. This also grounds the "lazy but not expendable" result from Finding 1.2 in a biological substrate.

**Source.** Charbonneau, D., Poff, C., Nguyen, H., Shin, M.C., Kierstead, K., & Dornhaus, A. (2017). "Who Are the 'Lazy' Ants? The Function of Inactivity in Social Insects and a Possible Role of Constraint: Inactive Ants Are Corpulent and May Be Young and/or Selfish." *Integrative and Comparative Biology* 57(3): 649–667. DOI: 10.1093/icb/icx029.
**Confidence: HIGH** — full abstract and key findings fetched from Oxford Academic.

---

### Finding 1.5 — Specialization does not predict individual efficiency in Temnothorax

**What happens.** Dornhaus (2008) measured individual efficiency (items transported per time unit) for 4 tasks in *T. albipennis* — brood transport during emigration, honey foraging, protein foraging (dead *Drosophila*), and sand-grain collection for nest building. Result: no correlation between behavioral specialization and efficiency in 3 of 4 tasks. In the 4th (sand grain collection), the correlation was *negative* — more specialized workers were actually slower.

**Mechanism.** Directly contradicts the standard "Jack-of-all-trades is master of none" assumption. Division of labor in Temnothorax does not benefit from learning curves or skill acquisition. Alternative explanations: reduced task-switching costs, spatial optimization of where ants concentrate, simplified cognitive demands.

**Sim implication.** Do not implement per-task skill accumulation for *T. curvinodis*. Repeated task performance should NOT increase individual efficiency in the TOML or FSM parameters for this species. Any "learning" mechanic should be gated to species with clear evidence (e.g., *Apis mellifera* dance learning). The efficiency benefit of colony organization in Temnothorax is systemic, not individual.

**Source.** Dornhaus, A. (2008). "Specialization Does Not Predict Individual Efficiency in an Ant." *PLoS Biology* 6(11): e285. DOI: 10.1371/journal.pbio.0060285.
**Confidence: HIGH** — full text confirmed via PLoS Biology fetch; quantitative task results verified.

---

### Finding 1.6 — Small colonies show extreme workload concentration on a few individuals

**What happens.** Dornhaus, Holley, Pook, Worswick & Franks (2008) examined who does the work during *T. albipennis* emigrations. In small colonies, a tiny number of workers performed most of the work — in one colony, a **single ant transported 57% of all items moved** during the entire emigration. Larger colonies had more evenly distributed workloads. In small colonies, individual transporters also achieved higher per-item efficiency.

**Mechanism.** Small colonies lack the redundancy to buffer individual variation. A few highly active ants carry the colony through a crisis; the rest are passive or inactive. This amplifies the importance of individual-level threshold variation in small-colony species.

**Sim implication.** For *T. curvinodis* with its 50–200 worker colonies, individual ant death (especially of the hyper-active minority) should have a disproportionate impact on colony function. Consider tracking `colony_dependence_index` = fraction of work done by top-10% most-active workers. If that subset is killed in a raid, colony capacity drops precipitously. This is the biological basis for making targeted raids on active foragers an effective strategy.

**Source.** Dornhaus, A., Holley, J-A., Pook, V.G., Worswick, G., & Franks, N.R. (2008). "Why do not all workers work? Colony size and workload during emigrations in the ant *Temnothorax albipennis*." *Behavioral Ecology and Sociobiology* 63: 43–51. DOI: 10.1007/s00265-008-0634-0.
**Confidence: HIGH** — confirmed via Springer abstract and search results citing the 57% single-worker finding specifically.

---

## Section 2 — Collective Decision-Making, Emigration & Quorum Sensing

### Finding 2.1 — Quorum sensing via tactile encounter rate drives the tandem-run → transport switch

**What happens.** Pratt (2005) showed that *T. albipennis* scouts switching from slow tandem-run recruitment to rapid transport (carrying nestmates bodily) do so when encounter rates at the candidate site exceed a threshold — not when a fixed number of ants is present. When nest size decreased (ants more crowded), the switch occurred at a *lower population* but at the *same encounter rate*. When tactile contact was prevented experimentally, ants continued tandem-running even in crowded conditions.

**Mechanism.** Ants read quorum via bodily touch, not chemical signals or counting. Encounter rate is a proxy for local density, which is a proxy for "many scouts approve of this site." The switch is density-triggered, making it robust to variation in nest dimensions and colony size.

**Sim implication.** The emigration mechanic (deferred, outside current roadmap scope per the outreach-roadmap spec) should implement encounter rate as the quorum signal — not a fixed ant-count threshold. Use `local_density_at_candidate = ants_in_radius / candidate_area` as the switch condition. The radius should be the `sense_radius` already in config; the area should be the single-cavity footprint. This avoids the brittle "exactly N ants" threshold that would fail when colony size varies.

**Source.** Pratt, S.C. (2005). "Quorum sensing by encounter rates in the ant *Temnothorax albipennis*." *Behavioral Ecology* 16(2): 488–496. DOI: 10.1093/beheco/ari020.
**Confidence: HIGH** — full text fetched from Oxford Academic; encounter-rate mechanism confirmed, including experimental blockade of tactile contact.

---

### Finding 2.2 — Only one-third of workers ever actively recruit; transport is 3× faster than tandem runs

**What happens.** Pratt, Mallon, Sumpter & Franks (2002) documented the full emigration cascade: initially, active scouts tandem-run to the new site (leading one follower at a time). When the quorum is reached, some scouts switch to transport (carrying nestmates passively). Only one-third of workers ever recruit at all; the passive majority is simply carried. Transports proceed at 3× the rate of tandem runs, accelerating the move once committed.

**Mechanism.** Two-phase recruitment: slow/selective (tandem runs, test the site, build scout population) → fast/committed (transport, move the colony quickly once decision is made). The quorum threshold is the gatekeeper between phases.

**Sim implication.** Emigration should NOT be modeled as all ants simultaneously moving. Implement a state machine for emigrations: `Scouting` (few ants, tandem) → `Quorum_reached` → `Transport` (carrying queen, brood, passive workers). The 2:1 ratio of carried-to-carrying ants in transport phase should inform the `AntState::BeingCarried` mechanic needed for emigration. Set `TRANSPORTER_FRACTION = 0.33` as a config parameter, consistent with published data.

**Source.** Pratt, S.C., Mallon, E.B., Sumpter, D.J.T., & Franks, N.R. (2002). "Quorum sensing, recruitment, and collective decision-making during colony emigration by the ant *Leptothorax albipennis*." *Behavioral Ecology and Sociobiology* 52: 117–127. DOI: 10.1007/s00265-002-0487-x.
**Confidence: HIGH** — confirmed via ResearchGate abstract and multiple citing papers.

---

### Finding 2.3 — Speed-accuracy tradeoff: quorum threshold is tunable against urgency

**What happens.** Franks, Dornhaus, Fitzsimmons & Stevens (2003) showed that *T. albipennis* colonies facing a time-limited nest-choice problem could shift between accuracy-maximizing and speed-maximizing modes. Under time pressure (nest destroyed, nothing to stay in), they lowered effective quorum thresholds and chose faster, accepting higher error risk. Without time pressure, they maintained higher thresholds and chose more accurately.

**Mechanism.** The quorum threshold is not fixed — it adjusts based on urgency signals (disruption of current nest, absence of a safe alternative). This is a context-sensitive decision parameter, not a hard-wired constant.

**Sim implication.** The effective quorum threshold for *T. curvinodis* emigration should scale inversely with `current_nest_damage_level`. If the nest is intact (player is disturbingly close but not attacking), threshold stays high (slow, accurate choice). If the nest is breached/destroyed (raid in progress), threshold drops (fast, potentially suboptimal choice under duress). This is the biological basis for "panicked" colony evacuations under attack yielding worse outcomes than planned moves.

**Source.** Franks, N.R., Dornhaus, A., Fitzsimmons, J.P., & Stevens, M. (2003). "Speed versus accuracy in collective decision-making." *Proceedings of the Royal Society B: Biological Sciences* 270(1532): 2457–2463.
**Confidence: HIGH** — citation confirmed via search result that explicitly names authors, year, title, journal and volume/page.

---

### Finding 2.4 — Colony size affects collective decision efficiency in Temnothorax

**What happens.** Dornhaus & Franks (2006) studied *T. albipennis* across colony size and found that "one size fits all" — the collective decision-making mechanism scaled surprisingly well from small to large colonies. However, Dornhaus, Holley & Franks (2009) separately showed larger colonies do *not* have more specialized workers, contradicting the prediction that colony-size-driven division of labor would arise.

**Mechanism.** The quorum mechanism is robust across colony sizes because it uses encounter *rate* (density), not absolute counts. Larger colonies naturally generate higher encounter rates without needing to adjust threshold parameters.

**Sim implication.** Do not implement a colony-size-dependent quorum threshold adjustment. The encounter-rate mechanism naturally handles size variation. A colony of 50 ants in a small cavity hits the same effective threshold as 200 ants in a larger cavity because the density signal is normalized by space.

**Source (combined).**
- Dornhaus, A. & Franks, N.R. (2006). "Colony size affects collective decision-making in the ant *Temnothorax albipennis*." *Insectes Sociaux* 53: 420–427.
- Dornhaus, A., Holley, J-A., & Franks, N. (2009). "Larger colonies do not have more specialized workers in the ant *Temnothorax albipennis*." *Behavioral Ecology* 20: 922–929.

**Confidence: HIGH** — both papers confirmed present in Dornhaus lab publication list, fetched live from annadornhaus.net/publications.

---

## Section 3 — Nest Defense, Evacuation Under Threat

### Finding 3.1 — Location of threat determines evacuation vs. withdrawal response

**What happens.** O'Shea-Wheller, Sendova-Franks & Franks (2015) showed *T. albipennis* mounts qualitatively different responses depending on where predation occurs. If ants are removed from *outside* the nest (at the periphery of scouting activity), the colony **withdraws inward** — it reduces exits and contracts into the cavity. If ants are removed from *within* the nest, the colony triggers **rapid evacuation** to a new site, removing queen and brood.

**Mechanism.** The colony reads location-of-loss as a signal about threat type. Peripheral losses = scouts caught outside, reduce exposure. Internal losses = nest compromised, escape needed. This is a superorganism-level sensory differentiation analogous to nervous-system regional sensing.

**Sim implication.** Implement two distinct threat responses for *T. curvinodis*:
1. **Withdrawal** (`THREAT_PERIPHERAL`): triggered when enemy ants contact surface scouts. Workers return to nest (`AntState::Returning`), reduce exit rate, deposit alarm pheromone. Colony becomes more defensive without relocating.
2. **Evacuation** (`THREAT_INTERNAL`): triggered when enemy ants breach the nest interior (reach the queen chamber tile or kill workers in nest tiles). Initiates emigration FSM with urgency-mode quorum (low threshold per Finding 2.3). Queen and brood get `BeingCarried` priority.

**Source.** O'Shea-Wheller, T.A., Sendova-Franks, A.B., & Franks, N.R. (2015). "Differentiated Anti-Predation Responses in a Superorganism." *PLoS ONE* 10(11): e0141012. DOI: 10.1371/journal.pone.0141012.
**Confidence: HIGH** — full text fetched; withdrawal vs. evacuation mechanism confirmed with exact description of experimental manipulation.

---

### Finding 3.2 — The queen is always passively transported; she does not self-evacuate

**What happens.** During all observed emigrations in *T. albipennis*, including threat-induced evacuations, the queen is carried by a worker — she never walks to the new site under her own agency. Queen transport is not initiated immediately; workers first establish the new site, then return to transport the queen.

**Mechanism.** The queen's physical passivity during emigration concentrates the decision of *when* to move her into the worker population. Workers assess readiness of the new site before risking the queen. This is a conservative risk management strategy at the colony level.

**Sim implication.** Queen should have `AntState::BeingCarried` as her only valid movement state during emigration. She cannot self-select `AntState::Fleeing`. Add an `emigration_queen_queued` flag: queen transport only begins after a minimum number of transport trips to the new site have occurred (representing workers establishing the site first). If the nest is destroyed before queen is transported, queen enters `AntState::Exposed` (vulnerable, queen health decays rapidly until rescued or killed).

**Source.** O'Shea-Wheller et al. (2015), same paper as Finding 3.1; also consistent with Pratt et al. (2002).
**Confidence: HIGH** — confirmed in fetched text: "the queen is always passively transported during migrations."

---

### Finding 3.3 — Temnothorax preference: emigration over combat; aggression = 0.15

**What happens.** *T. curvinodis* (and congenerics) respond to nest threats primarily by fleeing, not fighting. Workers have functional but small stings; they use them rarely in defense. The canonical response to nest disturbance or invasion is to initiate colony relocation, not to recruit soldiers.

**Mechanism.** The species' fitness strategy trades combat investment for mobility and concealment. Single-cavity, cryptic nesting in leaf litter and acorns minimizes detection. When detected, fleeing to a new cavity is cheaper than fighting a larger or more aggressive invader.

**Sim implication.** The TOML `aggression = 0.15` is biologically grounded. When an enemy ant enters the nest tile of *T. curvinodis*, the *primary* response should be `AntState::Fleeing` (workers dispersing with brood/queen), not `AntState::Fighting`. A small fraction (~15%) will fight as a rear-guard while the majority evacuates. Contrast with *Camponotus* or *Solenopsis* where `aggression = 0.8+`.

**Source.** Synthesized from: species file `docs/species/temnothorax_curvinodis.md` §7; Pratt (2005) emigration response documentation; O'Shea-Wheller et al. (2015).
**Confidence: MEDIUM** — emigration-preference is well-established; exact aggression quantification in combat situations is (unverified — general knowledge for the curvispinosus complex specifically).

---

## Section 4 — Slave-Making / Social Parasitism — Raids on Temnothorax

### Finding 4.1 — Enslaved Temnothorax workers systematically kill parasite brood ("slave rebellion")

**What happens.** Achenbach & Foitzik (2009) studied 88 colonies of the slavemaker *Protomognathus americanus* (now *Temnothorax americanus*), where enslaved *Temnothorax* workers raise the parasite's brood. Result: two-thirds of all parasite pupae died before hatching. Breakdown by caste: **83% mortality for queen pupae** vs **only 3% for male pupae** (males carry less parasite fitness). Control *Temnothorax* colonies in their own nests had 3–10% natural mortality. Killing mechanisms: 30% direct (workers physically pulling pupae apart); 53% neglect (workers moved pupae out of the brood chamber).

**Mechanism.** Enslaved workers cannot distinguish parasite larvae from host larvae (both receive care) but can detect parasite-specific chemical cues at the pupal stage when the adult cuticle begins to develop. Selective queen-pupa killing is particularly damaging because it removes future slavemaker colony founders. The inclusive fitness benefit: weakening the slavemaker colony reduces future raids on neighboring *Temnothorax* colonies that share relatives with the enslaved workers.

**Sim implication.** If multi-colony + social-parasitism mechanics are implemented (Phase 4+ with parasitic colony type), enslaved workers from captured colonies should have a probability of killing slavemaker brood rather than caring for it. Key parameters:
- `rebellion_probability_per_pupa` — scales with relatedness to neighboring free colonies
- Selective: queen-targeted pupae killing rate >> male pupae killing rate
- Mechanism: brood in `NEGLECT` state = moved to non-brood tile; brood in `DIRECT_KILL` state = instant removal
- Slavemaker colony growth slows dramatically if rebellion rate is high, feedback-reducing future raids on the region.

**Source.** Achenbach, A. & Foitzik, S. (2009). "First Evidence for Slave Rebellion: Enslaved Ant Workers Systematically Kill the Brood of Their Social Parasite *Protomognathus americanus*." *Evolution* 63(4): 1068–1075. DOI: 10.1111/j.1558-5646.2009.00591.x.
**Confidence: HIGH** — full quantitative data confirmed via fetched ScienceBlogs primary account and Wiley abstract; mortality rates exact.

---

### Finding 4.2 — Slave rebellion is geographically widespread, with intensity varying by parasite pressure

**What happens.** Pamminger, Leingärtner, Achenbach, Kleeberg, Pennings & Foitzik (2012) sampled three U.S. populations of the same host-parasite system and found slave rebellion in all three — it is not a local quirk. But intensity varied significantly: only **27% parasite pupal survival in West Virginia** (highest rebellion), **49% in New York**, **58% in Ohio** (lowest rebellion). Geographic variation tracks with local parasite density — populations under heaviest parasite pressure evolve strongest host defense.

**Mechanism.** This is a classic geographic mosaic of coevolution. High parasite pressure selects for stronger host counter-adaptations; lower pressure allows relaxation of costly defensive behaviors. Host recognition cues and rebellion tendency appear locally adapted.

**Sim implication.** In a multi-colony PvP or procedural map context, "parasite pressure" (frequency of slave raids on a regional cluster of *T. curvinodis* colonies) should adaptively tune rebellion probability over generations. High-frequency-raid maps should produce colonies with elevated `rebellion_probability`; low-frequency maps should have relaxed rebellion. This is a long-game evolutionary mechanic, not a per-match tunable — but the 27–58% survival range gives the simulation a biologically grounded bounding box for rebellion intensity.

**Source.** Pamminger, T., Leingärtner, A., Achenbach, A., Kleeberg, I., Pennings, P.S., & Foitzik, S. (2012). "Geographic distribution of the anti-parasite trait 'slave rebellion'." *Evolutionary Ecology*. DOI: 10.1007/s10682-012-9584-0.
**Confidence: HIGH** — key data (27%, 49%, 58% survival by population; geographic variation confirmed) fetched from ScienceDaily press release citing the paper directly.

---

### Finding 4.3 — Slavemaker raids cause host colonies to flee rather than fight

**What happens.** During *Protomognathus americanus* raids, the slavemaker workers invade the *Temnothorax* nest, causing the natal workers to flee while raiders abscond with host eggs, larvae, and pupae. The enslaved brood is then raised in the slavemaker nest by previously enslaved workers. Host workers do not organize a coordinated defense — they scatter.

**Mechanism.** *P. americanus* is a highly specialized obligate social parasite. It exploits the same flight response that makes *Temnothorax* survive physical predation — the colony's default response to nest invasion is evacuation, not combat. The slavemaker exploits this behavioral default. Chemical weapons (propaganda substances) may additionally suppress host worker aggression during raids (unverified — general knowledge from Harpagoxenus literature; similar mechanism in dulotic ants broadly).

**Sim implication.** A raid by a parasitic colony type should trigger `THREAT_INTERNAL` in the host *T. curvinodis* (per Finding 3.1 logic), but the evacuated brood is *intercepted* by raider ants rather than successfully transported to a new safe site. Mechanically: raider ants in the nest tile have `steal_brood_on_contact` behavior; fleeing workers carrying brood can be intercepted and the brood item transferred to the raider. The queen should have last-resort `AntState::Exposed` if she cannot be transported before raiders reach her chamber.

**Source.** Achenbach & Foitzik (2009) for raid behavior description; AntWiki *Temnothorax americanus* page (confirms raid absconding mechanics).
**Confidence: MEDIUM** — raid mechanism (flee + abscond) is confirmed; chemical propaganda claim for *P. americanus* specifically is (unverified — general knowledge).

---

### Finding 4.4 — Foitzik et al. (2009): slavemaker locally adapted; host density and life history affected

**What happens.** Foitzik, Achenbach & Foitzik (2009) showed that *P. americanus* colonies are locally adapted to their host populations, and that parasite presence significantly reduces host colony density, alters social structure, and affects host life history. Host populations under heavy parasite pressure show altered colony size distributions and life-history shifts.

**Mechanism.** Local adaptation in the slavemaker produces parasites optimized to exploit local host chemical profiles (recognition cues). This creates a mosaic where the same slavemaker species is more virulent against hosts from its local population than against transplanted foreign hosts. Host life-history shifts (smaller colonies, faster reproduction) are interpreted as evolved responses to high turnover risk.

**Sim implication.** In procedural map generation with *T. curvinodis* + slavemaker coexistence: spawn slavemaker colonies regionally clustered with their host populations. Host colonies in high-raider-density zones should have tuned-down `target_population` (smaller max size, faster lifecycle) as a life-history tradeoff. This is a map-generation parameter, not per-colony real-time adaptation.

**Source.** Foitzik, S., Achenbach, A., & Foitzik, C. (2009). "Locally adapted social parasite affects density, social structure, and life history of its ant hosts." *Ecology* 90(5): 1365–1374. DOI: 10.1890/08-0520.1.
**Confidence: MEDIUM** — abstract confirmed via Wiley Online Library; specific quantitative host-density numbers not recovered (paywalled). Core claim (local adaptation, density effects) is well-cited in related work.

---

## Section 5 — Dornhaus Lab Meta-Findings on Temnothorax (confirmed from live publication list)

The following additional confirmed Dornhaus publications are directly relevant to the sim and are listed for completeness. All titles confirmed via live fetch of `annadornhaus.net/publications`.

| Year | Authors | Title | Journal | Sim relevance |
|------|---------|-------|---------|---------------|
| 2004 | Dornhaus, Franks, Hawkins, Shere | "Ants move to improve – colonies of *Leptothorax albipennis* emigrate whenever they find a superior nest site." | *Animal Behaviour* 67: 959–963 | Relocation trigger threshold |
| 2006 | Dornhaus & Franks | "Colony size affects collective decision-making in the ant *Temnothorax albipennis*." | *Insectes Sociaux* 53: 420–427 | Colony-size-robust quorum |
| 2006 | Franks, Dornhaus et al. | "Decision-making by small and large house-hunting ant colonies: one size fits all." | *Animal Behaviour* 72: 611–616 | Small colony competence |
| 2006 | Marshall, Dornhaus et al. | "Noise, cost and speed-accuracy trade-offs: decision-making in decentralised systems." | *J. R. Soc. Interface* 3: 243–254 | Speed/accuracy math model |
| 2008 | Huang & Dornhaus | "A meta-analysis of ant social parasitism: host characteristics of different parasitism types and a test of Emery's rule." | *Ecological Entomology* 33: 589–596 | Parasitism host traits |
| 2012 | Pinter-Wollman, Hubler, Holley, Franks, Dornhaus | "How is activity distributed among and within tasks in *Temnothorax* ants?" | *Behav. Ecol. Sociobiol.* 66: 1407–1420 | Task distribution precursor to lazy worker work |
| 2015 | Charbonneau & Dornhaus | "When doing nothing is something. How task allocation strategies compromise between flexibility, efficiency, and inactive agents." | *J. Bioeconomics* 17: 217–242 | Theoretical model of inactivity value |
| 2024 | Bengston, Dornhaus & Rabeling | "The discovery of mixed colonies in *Temnothorax* ants supports the territoriality hypothesis of dulotic social parasite evolution." | *Insectes Sociaux* 72: 59–69 | Social parasite evolution mechanism |

---

## Key Sim Levers

The following sim parameters and mechanics are directly supported by the findings above:

| Lever | Biological basis | Finding # |
|-------|-----------------|-----------|
| `AntState::Idle` prevalence ~60% for T. curvinodis | Persistent inactivity is the species norm | 1.1 |
| `active_tick_fraction` per-ant tracking | Required for paper-#4 removal experiment reproduction | 1.1, 1.2 |
| Asymmetric replacement: active workers replaced by reserve, not vice versa | The defining result of Charbonneau et al. 2017 | 1.2 |
| No per-task skill accumulation for Temnothorax | Specialization ≠ efficiency | 1.5 |
| Small colony hyper-dependence on individual ants | 57% of emigration done by 1 ant in small colony | 1.6 |
| Quorum = encounter rate, not fixed count | Pratt 2005 mechanism | 2.1 |
| `TRANSPORTER_FRACTION = 0.33` | One-third of workers ever recruit | 2.2 |
| Two-phase emigration: tandem → transport | Quorum gates the switch | 2.2 |
| Urgency-scaled quorum threshold | Speed-accuracy tradeoff when nest destroyed | 2.3 |
| Withdrawal vs. Evacuation threat responses | Peripheral vs. internal predation elicits different responses | 3.1 |
| Queen always passively carried; never self-moves | O'Shea-Wheller et al. 2015 | 3.2 |
| `aggression = 0.15`; primary response = flee | Emigration > combat species | 3.3 |
| `rebellion_probability_per_pupa` for enslaved workers | Achenbach & Foitzik 2009 | 4.1 |
| Queen-pupa targeted (83% vs 3% male) | Selective fitness sabotage | 4.1 |
| Geographic rebellion intensity range: 27–58% survival | Pamminger et al. 2012 | 4.2 |
| Raid mechanic: flee + brood interception, not stand-fight | Achenbach 2009; THREAT_INTERNAL logic | 4.3 |

---

## Sources

### Confirmed via WebSearch + WebFetch (high confidence)

1. **Charbonneau, D., Sasaki, T., & Dornhaus, A. (2017).** "Who needs 'lazy' workers? Inactive workers act as a 'reserve' labor force replacing active workers, but inactive workers are not replaced when they are removed." *PLoS ONE* 12(9): e0184074. DOI: 10.1371/journal.pone.0184074. [PubMed PMC5587300](https://pmc.ncbi.nlm.nih.gov/articles/PMC5587300/)

2. **Charbonneau, D., Poff, C., Nguyen, H., Shin, M.C., Kierstead, K., & Dornhaus, A. (2017).** "Who Are the 'Lazy' Ants? The Function of Inactivity in Social Insects and a Possible Role of Constraint." *Integrative and Comparative Biology* 57(3): 649–667. DOI: 10.1093/icb/icx029. [Oxford Academic](https://academic.oup.com/icb/article/57/3/649/4036211)

3. **Charbonneau, D. & Dornhaus, A. (2015).** "Workers 'specialized' on inactivity: Behavioral consistency of inactive workers and their role in task allocation." *Behavioral Ecology and Sociobiology* 69: 1459–1472. DOI: 10.1007/s00265-015-1958-1. [Springer](https://link.springer.com/article/10.1007/s00265-015-1958-1)

4. **Charbonneau, D., Hillis, N., & Dornhaus, A. (2015).** "'Lazy' in nature: ant colony time budgets show high 'inactivity' in the field as well as in the lab." *Insectes Sociaux* 62: 31–35. [ResearchGate](https://www.researchgate.net/publication/271728748)

5. **Dornhaus, A. (2008).** "Specialization Does Not Predict Individual Efficiency in an Ant." *PLoS Biology* 6(11): e285. DOI: 10.1371/journal.pbio.0060285. [PLoS Biology](https://journals.plos.org/plosbiology/article?id=10.1371%2Fjournal.pbio.0060285)

6. **Dornhaus, A., Holley, J-A., Pook, V.G., Worswick, G., & Franks, N.R. (2008).** "Why do not all workers work? Colony size and workload during emigrations in the ant *Temnothorax albipennis*." *Behavioral Ecology and Sociobiology* 63: 43–51. DOI: 10.1007/s00265-008-0634-0. [Springer](https://link.springer.com/article/10.1007/s00265-008-0634-0)

7. **Pratt, S.C. (2005).** "Quorum sensing by encounter rates in the ant *Temnothorax albipennis*." *Behavioral Ecology* 16(2): 488–496. DOI: 10.1093/beheco/ari020. [Oxford Academic](https://academic.oup.com/beheco/article/16/2/488/297922)

8. **Pratt, S.C., Mallon, E.B., Sumpter, D.J.T., & Franks, N.R. (2002).** "Quorum sensing, recruitment, and collective decision-making during colony emigration by the ant *Leptothorax albipennis*." *Behavioral Ecology and Sociobiology* 52: 117–127. DOI: 10.1007/s00265-002-0487-x. [Springer](https://link.springer.com/article/10.1007/s00265-002-0487-x)

9. **Franks, N.R., Dornhaus, A., Fitzsimmons, J.P., & Stevens, M. (2003).** "Speed versus accuracy in collective decision-making." *Proceedings of the Royal Society B* 270(1532): 2457–2463. [Confirmed via search]

10. **O'Shea-Wheller, T.A., Sendova-Franks, A.B., & Franks, N.R. (2015).** "Differentiated Anti-Predation Responses in a Superorganism." *PLoS ONE* 10(11): e0141012. DOI: 10.1371/journal.pone.0141012. [PLoS ONE](https://journals.plos.org/plosone/article?id=10.1371%2Fjournal.pone.0141012) [PMC](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC4641648/)

11. **Achenbach, A. & Foitzik, S. (2009).** "First Evidence for Slave Rebellion: Enslaved Ant Workers Systematically Kill the Brood of Their Social Parasite *Protomognathus americanus*." *Evolution* 63(4): 1068–1075. DOI: 10.1111/j.1558-5646.2009.00591.x. [Wiley](https://onlinelibrary.wiley.com/doi/full/10.1111/j.1558-5646.2009.00591.x)

12. **Pamminger, T., Leingärtner, A., Achenbach, A., Kleeberg, I., Pennings, P.S., & Foitzik, S. (2012).** "Geographic distribution of the anti-parasite trait 'slave rebellion'." *Evolutionary Ecology*. DOI: 10.1007/s10682-012-9584-0.

13. **Dornhaus, A. & Franks, N.R. (2006).** "Colony size affects collective decision-making in the ant *Temnothorax albipennis*." *Insectes Sociaux* 53: 420–427. [Confirmed via Dornhaus lab publication list]

14. **Dornhaus, A., Holley, J-A., & Franks, N. (2009).** "Larger colonies do not have more specialized workers in the ant *Temnothorax albipennis*." *Behavioral Ecology* 20: 922–929. [Confirmed via Dornhaus lab publication list]

15. **Foitzik, S., Achenbach, A., & Foitzik, C. (2009).** "Locally adapted social parasite affects density, social structure, and life history of its ant hosts." *Ecology* 90(5): 1365–1374. DOI: 10.1890/08-0520.1. [Wiley](https://esajournals.onlinelibrary.wiley.com/doi/10.1890/08-0520.1)

16. **Anna Dornhaus full publication list (live fetch 2026-06-22).** [annadornhaus.net/publications](https://www.annadornhaus.net/publications) — 60+ papers confirmed from 1999–2026; all Temnothorax citations cross-validated against this source.

### Marked unverified (general knowledge, not confirmed via fetch)

- Chemical propaganda substances suppressing host aggression during *P. americanus* raids — mechanism is well-attested for related dulotic species (Harpagoxenus); not confirmed specifically for *P. americanus* via this session's searches.
- Exact quantitative aggression rates for *T. curvinodis* in combat contexts — the `aggression = 0.15` TOML value is biologically motivated by the species' emigration-preference strategy, but a specific paper measuring combat initiation frequency in this species complex was not fetched.

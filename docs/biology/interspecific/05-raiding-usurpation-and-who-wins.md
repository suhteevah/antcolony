# Raiding, Slave-Making, Queen-Killing / Usurpation — and Who Wins When Two Species Clash

**Research facet:** Cross-species raiding, dulosis (slave-making), social-parasitic usurpation, queen-killing — plus a synthesis of the determinants of victory in interspecific ant conflict.
**Purpose:** Ground the antcolony cross-species combat arena, whose central new mechanic is **cross-species queen-killing** (win = kill the enemy queen). Real ant biology supplies a rich, literally-applicable template: ants *do* kill heterospecific queens, and they do it by several distinct, well-studied routes. This file maps each route onto a sim mechanic.
**Logged:** 2026-06-21. Append-only; superseded claims marked `[SUPERSEDED ← see date]`.
**Format:** mirrors `docs/biology.md` and sibling `docs/biology/interspecific/01-competition-and-displacement.md` house style — each finding is **What happens → Mechanism → Sim implication → Source (+confidence, verified-online vs general-knowledge)**.

**Companion files:** `docs/biology/interspecific/01-competition-and-displacement.md` (competition/displacement framework), `03-harvester-competition-cole-wiernasz.md`, `04-temnothorax-defense-dornhaus.md`. Species hosts/parasites: `docs/species/formica_fusca.md` (host), `docs/species/formica_rufa.md` (parasitic founder).

**Headline for the cross-species queen-kill mechanic.** The mechanic is *strongly* grounded — arguably the best-documented part of this whole facet. Ants kill heterospecific queens routinely, by at least five distinct mechanisms (direct mandible kill, slow throttling, decapitation, antenna-amputation, and the remarkable *chemically-induced worker matricide* where the parasite makes the host's own workers kill their mother). Crucially, real biology also says queen-killing is **conditional and timed**, not a random instant-win: parasites delay the kill until the host workforce is large enough to be useful, and acceptance afterward depends on a **chemical-disguise step** (acquiring the dead queen's cuticular hydrocarbons). Both of those are directly translatable to balanced game mechanics.

---

## Part I — Slave-Making Raids (Dulosis): the Raid Pipeline

### Finding 1 — All dulotic raids share a four-stage pipeline: scout → recruit → fight → brood-transport

**What happens in nature.** Across every slave-making genus studied — *Polyergus*, *Formica sanguinea*-group, *Harpagoxenus*, *Protomognathus* — raids decompose into the same four sequential phases: (1) **scouting** (individuals search for a host nest), (2) **recruitment** (scout returns and mobilizes raiders), (3) **fighting** (raiders enter and overcome host defenders), (4) **brood transport** (raiders carry host larvae/pupae home). The genera differ in how *sharp* the transition between phases is, not in the phases themselves.

**Mechanism.** In obligate slave-makers (*Polyergus*, *Harpagoxenus*) scouts are specialist individuals whose sole job is to locate host nests; recruitment is rapid and produces dense raiding columns. In facultative slave-makers (*Formica sanguinea*) the scout role is diffuse — raids develop as an extension of ordinary predatory foraging, with no sharp "raid-mode" switch.

**Sim implication.** Model a raid as an explicit four-state FSM at the squad level: `Scouting → Recruiting → Raiding → Hauling`. A scout entity that locates an enemy nest lays a recruitment trail home; arrival above a recruiter-quorum threshold flips the colony into Raiding and spawns a column that follows the scout's trail to the target. This reuses the existing pheromone-trail + quorum machinery. Make the scout→recruit sharpness a per-species parameter (`raid_organization = "obligate" | "facultative"`): obligate = fast mass column, facultative = trickle that grows out of normal foraging.

**Source.** Buschinger, A., Ehrhardt, W. & Winter, U. (1980). The organization of slave raids in dulotic ants — a comparative study. *Zeitschrift für Tierpsychologie* 53: 245–264. Confidence: **established/foundational** — the canonical behavioral typology of dulosis. (Verified online — citation confirmed; corroborated by Brandt et al. 2006 and Mori et al. 2001 which cite it directly.)

---

### Finding 2 — Scouts navigate by trail + landmarks; recruitment is context-gated

**What happens in nature.** *Polyergus breviceps* scouts locate a *Formica* host nest, then return home using **optical landmark cues**, laying a **chemical trail** on the way. The subsequent raiding column follows almost the identical route. Recruitment behavior (jerky running among nestmates, likely a recruitment pheromone) is intense *during* active raiding but virtually absent during the pre-raid scouting/return phase — the signal is context-specific, not constant.

**Mechanism.** Dual-cue navigation (trail pheromone + visual landmarks) makes the route robust; gating recruitment to the raid phase prevents the colony from mobilizing on every scouting trip.

**Sim implication.** The scout's recruitment signal should be a *discrete event* emitted on return, not a continuous emission — only a returning scout that actually found a target broadcasts "raid now." This avoids constant false mobilization. If/when a landmark/vision layer exists, scouts can path home faster than pure trail-following; until then, trail-only is an acceptable approximation.

**Source.** Topoff, H., Lamon, B., Goodloe, L. & Goldstein, M. (1984). Social and orientation behavior of *Polyergus breviceps* during slave-making raids. *Behavioral Ecology and Sociobiology* 15: 273–279; and Topoff, H. & Cover, S. (1989). Behavioral adaptations for raiding in the slave-making ant *Polyergus breviceps*. *Journal of Insect Behavior* 2: 545–556. Confidence: **established** — two independent Topoff-lab field studies. (Verified online — citations confirmed.)

---

### Finding 3 — Raids steal BROOD (pupae/larvae), not adults; only brood can be re-imprinted

**What happens in nature.** Obligate slave-maker raiding parties penetrate the host nest and carry off **pupae and larvae** — never adult workers. Adults that resist are killed or driven off. Captured brood is reared by the resident enslaved workers; newly-eclosed adults chemically imprint on the mixed-colony odor and integrate as functional slave workers.

**Mechanism.** Adult ants have a fixed cuticular-hydrocarbon (CHC) "nest odor" learned at eclosion and cannot be re-imprinted; brood is still chemically plastic, so a pupa raised in the raider nest adopts the raider's odor and works for it. This is *why* the target is brood.

**Sim implication.** A raid's payoff is **brood theft**, modeled as a transfer: raided eggs/larvae/pupae move from the victim's brood pool to the raider's, where they mature into raider-colony workers (optionally tagged `enslaved` cosmetically). Stolen brood is the resource analogue of food — it directly buys future population. Adults in the raided nest are *not* convertible: they only flee, fight, or die. This gives raiding a clean economic role distinct from a queen-kill: you don't have to kill the queen to profit from a raid.

**Source.** Hölldobler, B. & Wilson, E. O. (1990). *The Ants*, Ch. 12 "Slavery", pp. 444–478. Harvard/Belknap. Confidence: **established/foundational** — the canonical synthesis of dulosis. (Verified online — chapter/page range confirmed; corroborated by Hare & Alloway 2001 and Mori et al. 2000.)

---

### Finding 4 — Raid frequency & success are quantified: ~69% success for *F. sanguinea*; weather-gated

**What happens in nature.** A continuous 78-day field study of two *Formica sanguinea* colonies recorded **26 raids on 23 days; 18 of 26 (69%) succeeded** in sacking nests of *F. cunicularia*, *F. fusca*, and *Lasius emarginatus*. Eight failed. **No raids occurred on rainy or overcast days.** Obligate *Polyergus rufescens* colonies, by contrast, run only ~4–8 raids per *season*, concentrated in mid-to-late afternoon, and are likewise suppressed on overcast days.

**Mechanism.** Facultative raiders (*F. sanguinea*) raid opportunistically and often (raiding is folded into foraging); obligate raiders (*Polyergus*) stage rarer, more stereotyped mass raids. Weather gates raiding because trail-laying and thermal activity windows constrain mass movement.

**Sim implication.** Give raids a **non-trivial failure rate** (~30% baseline) modulated by the matchup (defender strength, see Part III) — raids are not auto-wins. Gate raid initiation on environmental state (the existing temperature/weather system): suppress raiding during rain and outside the species' thermal activity window. Set per-species raid cadence: facultative species raid frequently as part of foraging; obligate raiders stage rarer, larger set-piece raids.

**Source.** Mori, A., Grasso, D. A. & Le Moli, F. (2000). Raiding and foraging behavior of the blood-red ant, *Formica sanguinea*. *Journal of Insect Behavior* 13: 421–438; Le Moli, F., Mori, A., Grasso, D. A. & Ugolini, A. (1994). Eco-ethological factors affecting the scouting and raiding behaviour of *Polyergus rufescens*. *Ethology* 96: 289–302. Confidence: **established** — quantified multi-season field data. (Verified online — citations confirmed.)

---

### Finding 5 — Chemical "propaganda": weaponizing the host's own alarm system

**What happens in nature.** *Formica subintegra* (an obligate slave-maker in the *F. sanguinea* group) carries grossly **hypertrophied Dufour's glands holding ~700 µg** of a mix of **decyl, dodecyl, and tetradecyl acetates**, sprayed at defending host workers during raids. These long-chain acetates are alarm-substance mimics that evaporate slowly and produce a *stronger and longer-lasting* panic than the host's own alarm pheromones — defenders scatter and turn on each other rather than mount a coordinated defense. Regnier & Wilson coined the term **"propaganda substances."** The same strategy evolved **convergently** in the unrelated *Temnothorax* slave-makers *Protomognathus* and *Harpagoxenus*: their Dufour's-gland secretion, applied to a host nest, causes agitation; applied to a single host worker, it makes her nestmates attack *her*.

**Mechanism.** Propaganda hijacks the host's existing communication channel: by over-saturating the alarm signal, the raider converts the host's coordinated-defense machinery into self-destructive chaos. It is distinct from chemical *mimicry* (passing as a nestmate) — propaganda doesn't disguise the raider, it disorganizes the defender.

**Sim implication.** Model propaganda as a raid ability that, on raid initiation, **dumps a large transient spike into the defender's Alarm pheromone field** — but with *inverted* effect on the defenders: instead of converging to defend, propaganda-affected defenders get a flee bias and a friendly-fire/disorganization debuff (reduced effective defender count for the fight resolution). This reuses the existing Alarm grid. Gate it as a per-species/tech ability (`propaganda = true`) so only slave-maker archetypes get it — a clean asymmetric mechanic. Suggested PvP tech-gate per `biology.md` "Tech Unlocks for PvP".

**Source.** Regnier, F. E. & Wilson, E. O. (1971). Chemical communication and "propaganda" in slave-maker ants. *Science* 172(3980): 267–269; Brandt, M., Fischer-Blass, B., Heinze, J. & Foitzik, S. (2006). Convergent evolution of the Dufour's gland secretion as a propaganda substance in the slave-making ant genera *Protomognathus* and *Harpagoxenus*. *Insectes Sociaux* 53(4). Confidence: **established/foundational** (Regnier & Wilson is the origin of the concept) + **established** (Brandt et al. for convergence). (Verified online — both citations confirmed.)

---

### Finding 6 — "Prudent" vs "despotic" parasites: raiders that don't over-harvest win long-term

**What happens in nature.** *Protomognathus americanus* (now often placed in *Temnothorax americanus*) is a **prudent** parasite: it seldom kills adult host workers during raids, never occupies the raided nest, and even rears captured host queen/male pupae to adulthood. The despotic *Leptothorax duloticus*, by contrast, kills most adults, occupies raided nests, and crashes host colonies (workers ~302 → 113). The prudent parasite reaches **2–3× higher population densities** than the despotic one — restraint is ecologically *adaptive* because it doesn't exhaust the host supply.

**Mechanism.** Over-harvesting collapses the local host population the parasite depends on; a parasite that leaves the host nest functional can re-raid it later. This is a tragedy-of-the-commons / sustainable-yield dynamic at the landscape scale.

**Sim implication.** If raiding is repeatable against the same enemy nest, a raider that always kills the queen and razes the nest removes its own future income. Consider rewarding **repeat partial raids** (brood theft without queen-kill) over a single annihilation in survival/Keeper modes — a raider that keeps the victim alive-but-weak can farm it. In PvP the annihilation (queen-kill) win condition dominates, but the prudent/despotic axis is a nice species-flavor parameter (`raid_lethality`).

**Source.** Hare, J. F. & Alloway, T. M. (2001). Prudent *Protomognathus* and despotic *Leptothorax duloticus*: differential costs of ant slavery. *PNAS* 98(21). Confidence: **established** — quantified comparative colony-demography study. (Verified online — citation confirmed.)

---

### Finding 7 — Hosts fight back: "slave rebellion" — enslaved workers kill the parasite's brood

**What happens in nature.** Enslaved *Temnothorax longispinosus* workers **systematically kill up to two-thirds of their *Protomognathus americanus* captor's female brood** — but are highly caste-selective: ~**83% mortality of female pupae** (future queens/workers) vs only ~3% of male pupae. Slaves nurse *larvae* normally (~95% survival); the killing switches on at the **pupal** stage, when species- and caste-specific CHC profiles appear on the cuticle. Across three U.S. populations parasite pupal survival under slave care was **27% / 49% / 58%**, all far below the **85%** in free-living host nests.

**Mechanism.** Slaves gain no *direct* fitness (they're sterile and won't reproduce), but killing parasite queens weakens the parasite colony and reduces future raid pressure on *neighboring related* host colonies — an **indirect (kin-selected)** benefit. The trigger is chemical: CHC cues distinguishing parasite-female pupae appear only at pupation.

**Sim implication.** Optional depth mechanic: stolen/enslaved brood that matures in the raider nest can have a small chance to **sabotage** the raider's own brood (especially queen-destined brood), scaling with how many enslaved workers are present. This makes large slave forces a double-edged sword and gives the victim a passive counter-play even after losing brood. Skip for MVP; strong flavor for a "host counter-adaptation" tech.

**Source.** Achenbach, A. & Foitzik, S. (2009). First evidence for slave rebellion: enslaved ant workers systematically kill the brood of their social parasite *Protomognathus americanus*. *Evolution* 63(4): 1068–1075; Pamminger, T. et al. (2012). Geographic distribution of the anti-parasite trait "slave rebellion". *Evolutionary Ecology*. Confidence: **established** — landmark finding + geographic replication. (Verified online — both citations confirmed.)

---

## Part II — Queen-Killing & Usurpation: the Cross-Species Queen-Kill, Done Five Ways

> This is the heart of the facet. The sim's win condition (kill the enemy queen) has a direct, well-documented biological analog in social-parasitic colony usurpation. Real ants kill heterospecific queens by at least five distinct mechanisms, and — critically — the kill is **conditional, timed, and followed by a chemical-disguise step**, not an instant win. All five mechanisms below are realistic templates; the timing/disguise findings (8, 9, 13) are what make a *balanced* mechanic.

### Finding 8 — Queen-killing is TIMED, not opportunistic: parasites wait until the host workforce is worth taking

**What happens in nature.** A founding *Polyergus breviceps* queen does **not** attack every *Formica* queen she meets. **Young, recently-mated host gynes are not attacked at all** — through 190 days post-mating, newly-mated *F. gnava* queens elicited zero aggression; at 204 days (≈29 weeks) 100% were attacked. The parasite waits until the host colony has accumulated **enough workers to be a useful slave force** before killing its queen. Killing a queen with no workers is maladaptive — the parasite would starve.

**Mechanism.** The attack-*trigger* cue is a time-dependent **maturation signal correlated with the host queen's reproductive/ovarian maturity — NOT her cuticular hydrocarbons** (CHC profiles of young vs mature host queens were similar). The parasite is effectively reading "does this queen come with a workforce attached?" before committing to the kill.

**Sim implication.** This is the single most important balance lever for the cross-species queen-kill mechanic. **Do not make the enemy queen a one-hit instant-win target from t=0.** Gate the queen-kill on the attacker first establishing presence/strength relative to the defender (mirrors "wait until the host workforce is large enough"). Equivalently: a lone infiltrator cannot snipe the enemy queen early-game; the queen becomes vulnerable only once the attacker has overwhelmed/occupied enough of the nest. This prevents degenerate rush strategies and matches biology exactly.

**Source.** Johnson, C. A., Topoff, H., Vander Meer, R. K. & Lavine, B. (2002). Host queen killing by a slave-maker ant queen: when is a host queen worth attacking? *Animal Behaviour* 64: 807–815 (doi:10.1006/anbe.2002.1971). Confidence: **established** — verified against the full-text USDA-ARS PDF; the "young gynes not attacked / attacks begin ~204 days" figures are quoted verbatim from the paper. (Verified online — full text confirmed.)

---

### Finding 9 — After the kill, the killer must DISGUISE as the dead queen (CHC acquisition) to be accepted

**What happens in nature.** The invading *Polyergus breviceps* queen kills the *Formica* host queen with sickle/dagger-shaped piercing mandibles (a ~25–30 min attack of repeated biting), and **between bites licks the host queen's cuticle**, absorbing her cuticular hydrocarbons. The parasite's own CHC profile then shifts from her native straight-chain alkanes toward the host queen's methyl-/dimethyl-branched alkanes. Host workers, now deprived of their queen's pheromone signal and faced with an intruder who "smells right," switch from attacking to grooming/feeding her. The acquired disguise stays effective for **at least a week**. Takeover **fails in queenless colonies** — the kill *is* the disguise-acquisition step.

**Mechanism.** Two separable steps: (1) remove the incumbent queen's primer-pheromone signal (kill her) and (2) acquire her colony-identity CHCs by direct contact, so the workers accept the new reproductive. The Dufour's-gland secretion of the *Polyergus* gyne additionally repels workers *during* the attack (only Dufour's-gland extract protected her; poison/pygidial extracts did not).

**Sim implication.** Make the queen-kill a **two-phase action, not an instant event**: (a) a vulnerable channeling "attack/usurp" phase during which the attacker is exposed (workers can interrupt/kill her — the Dufour's "repellent" buys a limited protection window), then (b) on success, the attacker acquires the dead queen's "colony identity," after which surviving defender workers stop attacking and may be co-opted. This gives the defender a real chance to interrupt a queen-kill in progress, and gives a successful usurpation a satisfying "the enemy workers are now yours" payoff. Models the disguise as a state flag, no chemistry sim needed.

**Source.** Topoff, H. & Zimmerli, E. (1993). Colony takeover by a socially parasitic ant, *Polyergus breviceps*: the role of chemicals obtained during host-queen killing. *Animal Behaviour* 46: 479–486 (doi:10.1006/anbe.1993.1216); Johnson, C. A., Vander Meer, R. K. & Lavine, B. (2001). Changes in the cuticular hydrocarbon profile of the slave-maker ant queen *Polyergus breviceps* after killing a *Formica* host queen. *Journal of Chemical Ecology* 27(9): 1787–1804; Topoff, H. et al. (1988). Colony founding by queens of *Polyergus breviceps*: the role of the Dufour's gland. *Ethology* 78: 209–218. Confidence: **established** — the mechanism is confirmed by the 1993 behavioral study and the 2001 GC chemical analysis. Minor nuance: primary sources describe repeated "bite-and-lick" rather than a sustained "clamp." (Verified online — all three citations confirmed; 1993 DOI verified via Wikidata.)

---

### Finding 10 — Direct mandible kill (fast, brute-force): *Formica rufa* / *Polyergus*

**What happens in nature.** Temporary social parasites of the *Formica rufa* group found their colonies by invading a host *F. fusca* (or *F. lemani*) nest, **killing the host queen by direct physical attack (biting)**, after which host workers rear the invader's brood; as host workers die of attrition the colony becomes pure parasite. *F. rufa* in particular is described as an **"inept" parasite** (Wilson 1971) — it relies on speed and physical dominance rather than chemical deception, plunging directly into the host colony.

**Mechanism.** Brute-force usurpation: overpower the host queen quickly; the parasite's brood then converts the workforce over time. Higher risk than the chemically-sophisticated routes (no pre-invasion disguise), offset by physical dominance.

**Sim implication.** This is the **default cross-species queen-kill template** for the sim's *Formica* matchups (already foreshadowed by `formica_fusca.toml`'s `displaced_by = ["formica_rufa", "formica_sanguinea"]` and `formica_rufa.toml`'s `founding = "parasitic"`). A founding-queen usurpation path: place a host nest, the parasite queen infiltrates and attempts the kill (high risk if defenders are present), and on success the host workforce gradually re-rolls to the parasite. The "inept/brute-force" flavor = no pre-kill disguise window, so this route is riskier than Finding 9's chemically-assisted kill.

**Source.** Borowiec, M. L., Cover, S. P. & Rabeling, C. (2021). The evolution of social parasitism in *Formica* ants revealed by a global phylogeny. *PNAS* 118(38): e2026029118; Wilson, E. O. (1971). *The Insect Societies*, p. 356 (the "inept parasite" characterization). Confidence: **established** (Borowiec et al. phylogeny) + **foundational** (Wilson). (Verified online — both citations confirmed; cross-referenced in `docs/species/formica_rufa.md`.)

---

### Finding 11 — Slow throttling (days to weeks): *Lasius reginae*, *Myrmoxenus* (*Epimyrma*)

**What happens in nature.** Several parasites kill the host queen by **slow mechanical strangulation**. *Lasius reginae* (on *L. alienus*) flips the host queen onto her back, grasps her thorax/neck in her mandibles, and strangles her. *Myrmoxenus* (formerly *Epimyrma*) queens, parasitizing *Temnothorax*, **throttle the host queen with their mandibles over days to weeks** — no sting, no acid, pure mechanical pressure. (*Myrmoxenus* additionally conducts ongoing slave raids to replenish workers, making it technically a permanent parasite with dulosis.)

**Mechanism.** Prolonged grip restricts the host queen until she dies; the extended timescale lets the parasite stay attached and acquire host odor gradually while host workers habituate.

**Sim implication.** A **slow-channel usurpation variant**: the attacker latches onto the enemy queen and drains her health over an extended duration while remaining attached and vulnerable. Higher total commitment than the fast kill (Finding 10) but with a built-in disguise-acquisition window (Finding 9). Good as a distinct species/archetype flavor — "grappler" parasite vs "assassin" parasite.

**Source.** Buschinger, A. (2009). Social parasitism among ants: a review. *Myrmecological News* 12: 219–235; Heinze, J., Buschinger, A., Poettinger, T. & Suefuji, M. (2015). Multiple convergent origins of workerlessness and inbreeding in the socially parasitic ant genus *Myrmoxenus*. *PLoS ONE* 10(7): e0131023. Confidence: **established** — Buschinger 2009 is the modern review of record; Heinze et al. for *Myrmoxenus* mechanics. (Verified online — both citations confirmed. *Lasius reginae* throttling described in Buschinger's review/secondary syntheses — confidence **moderate** on the precise mechanical detail.)

---

### Finding 12 — Decapitation & antenna-amputation (the gruesome extremes): *Bothriomyrmex*, *Leptothorax goesswaldi*

**What happens in nature.** *Bothriomyrmex decapitans* (North Africa) parasitizes *Tapinoma*: the small parasite queen lets host workers drag her into the nest, then **mounts the host queen's back and slowly saws off her head with her mandibles** (the species epithet is literal). Chemical camouflage (shared pygidial-gland ketones with the host) eases her entry. *Leptothorax goesswaldi* (a workerless permanent inquiline in *L. acervorum*) cohabits through winter, then in spring **kills the host queen by cutting off her antennae** followed by further mutilation — a deliberate, slow incapacitation.

**Mechanism.** Targeted dismemberment of the host queen. Decapitation is a fast lethal strike once positioned; antenna-amputation is a slow disabling that removes the queen's sensory/signaling capacity before death.

**Sim implication.** Mostly flavor/variety — these are alternative *animations/framings* of the same "attacker kills enemy queen" outcome, plus the camouflage-entry step (shared chemistry → infiltrate without triggering defense, see Finding 13). Useful if the sim ever wants distinct kill-styles per species for visual variety. The "let yourself be carried in" entry tactic (Bothriomyrmex) is a real alternative to fighting to the queen chamber — could model as a low-aggression infiltration that trades speed for stealth.

**Source.** Wilson, E. O. (1971). *The Insect Societies* (the *Bothriomyrmex decapitans* account, after Santschi's ~1906 observations); Buschinger, A. & Klump, B. (1988). Novel strategy of host-queen removal in a permanently parasitic ant, *Leptothorax goesswaldi*. *Naturwissenschaften* 75: 422–423. Confidence: **established** (Wilson/Santschi for *Bothriomyrmex*, a textbook case) + **established** (Buschinger & Klump for *L. goesswaldi*). (Verified online — Buschinger & Klump citation confirmed; *Bothriomyrmex* account is a long-standing textbook example, **verified via secondary syntheses**.)

---

### Finding 13 — Chemical-induced MATRICIDE: the parasite makes the host's OWN workers kill their queen (2025)

**What happens in nature.** A 2025 study found a previously-unknown queen-elimination route: parasite queens of *Lasius umbratus* (on *L. japonicus*) and *Lasius orientalis* (on *L. flavus*) first acquire host-colony odor by overnight cohabitation with a few host workers, then covertly approach the resident host queen and **spray her with jets of acidopore fluid (formic acid)**. The contaminated queen is then perceived by her own workers as a colony threat, and **the host workers themselves dismember and kill their mother (matricide)**. The two parasites differ: *L. umbratus* needs only **2 targeted sprays → immediate fatal attack**, while *L. orientalis* sprays **~15 times over ~20 hours**, with workers killing the queen after **~4 days**. The parasite then takes over reproduction; orphaned workers rear her brood.

**Mechanism.** The parasite never directly fights the host queen — it weaponizes the host's *own* nestmate-recognition/threat-response system. Marking the queen with a danger-signal chemical (formic acid, which both genus-mates produce and recognize) flips the workers' loyalty into lethal aggression. Lowest-risk known usurpation: no direct combat with the queen.

**Sim implication.** A spectacular, distinctive cross-species mechanic: a **"sabotage/turncoat" usurpation** in which the attacker doesn't kill the enemy queen directly but applies a debuff that makes the *enemy's own workers* attack and kill her. Reuses the Alarm/recognition machinery: the attacker "marks" the enemy queen (a channeled application), after which defender workers re-target their own queen. Strong asymmetric flavor for a chemically-specialized parasite archetype; an excellent showcase mechanic. Gate behind a tech/species trait. NOTE: very recent (2025) — single study, so treat the *specific* mechanism as cutting-edge rather than textbook-settled, but the citation is solid.

**Source.** Shimada, T., Tanaka, Y. & Takasuka, K. (2025). Socially parasitic ant queens chemically induce queen-matricide in host workers. *Current Biology* 35(22). doi:10.1016/j.cub.2025.09.037. Confidence: **strong-but-recent** — verified via CrossRef DOI metadata, the Kyushu University release, and multiple independent press (Science, CNN, Science News). Single primary study (2025); mechanism is new to the literature. (Verified online — DOI and species pairings confirmed.)

---

### Finding 14 — Infiltration chemistry: how parasite queens get *in* without being killed

**What happens in nature.** Parasite queens use one of three chemical strategies to survive infiltration before they can reach/kill the host queen: (1) **chemical insignificance** — a reduced, near-blank CHC profile so workers neither recognize a threat nor a nestmate and largely ignore the intruder; (2) **chemical mimicry** — actively synthesizing host-matching CHCs *before/during* invasion; (3) **chemical camouflage** — passively acquiring host CHCs by contact *after* entry. Some also deploy **appeasement substances** that suppress worker aggression during the critical entry window. (Slave-maker *scouts* use a related trick: convergently evolved "chemical transparency" — more straight-chain n-alkanes, fewer recognition-relevant methyl-branched alkanes — so a lone scout outnumbered ~10:1 isn't mobbed.)

**Mechanism.** Nestmate recognition keys on CHC profiles; the three strategies each defeat it differently (blank vs match vs borrow). This is the *entry* problem, logically prior to the *kill* (Part II) and the *acceptance* (Finding 9).

**Sim implication.** Infiltration can be its own pre-combat phase: an infiltrator with a "disguise" trait can move through enemy territory toward the queen chamber with reduced aggression-triggering, vs a brute-force attacker who fights every defender on the way in. This makes "assassin" (stealth to the queen) and "army" (fight through) distinct viable routes to the same queen-kill win. Model disguise as an aggression-suppression flag with a decay timer (camouflage wears off / can be detected).

**Source.** Lenoir, A., D'Ettorre, P., Errard, C. & Hefetz, A. (2001). Chemical ecology and social parasitism in ants. *Annual Review of Entomology* 46: 573–599; Kleeberg, I., Menzel, F. & Foitzik, S. (2017). The influence of slavemaking lifestyle, caste and sex on chemical profiles in *Temnothorax* ants. *Proceedings of the Royal Society B* 284: 20162249. Confidence: **established** — Lenoir et al. is the review of record for parasite infiltration chemistry. (Verified online — both citations confirmed.)

---

## Part III — Territorial Wars & Brood Raiding Between Mature/Founding Colonies

### Finding 15 — Founding-stage brood raiding: the larger incipient nest wins, and absorbs the loser's brood + traitor workers

**What happens in nature.** In *Solenopsis invicta*, newly-founded colonies raid each other's brood during the incipient stage. Raids are initiated by workers from the **larger** nest via trail-laying/following and last minutes to days; resistance is "generally low and often absent," and workers of the losing nest ("traitor raiders") even **join the winning force**. The colony with **more workers reliably wins** — queen number, brood quantity, and worker relatedness had **no significant effect**. One field raid spanned 38 days, >100 m of trail, and absorbed ≥80 incipient nests. Brood raiding is "a major cause of incipient colony mortality."

**Mechanism.** Pure numerical contest at the founding stage: the bigger nest's workers overrun the smaller, carry its brood home, and the loser's surviving workers defect to the winner (callow workers re-imprint on whichever colony they end up in). Worker number is the sole significant predictor.

**Sim implication.** Founding/early-game is the **decisive vulnerability window** and brood raiding is the early-game weapon. Implement incipient-stage brood raids where outcome is dominated by **worker count** (not queen count or relatedness), the winner gains the loser's brood, and surviving losing workers can **defect** to the winner. This gives a strong early-game tempo mechanic and matches biology precisely. Pairs with Finding 8: the queen-kill is gated to later, but brood raids are the early game.

**Source.** Tschinkel, W. R. (1992). Brood raiding in the fire ant, *Solenopsis invicta*. *Annals of the Entomological Society of America* 85(5): 638–646; Adams, E. S. & Tschinkel, W. R. (1995). Effects of foundress number on brood raids and queen survival in the fire ant *Solenopsis invicta*. *Behavioral Ecology and Sociobiology* 37: 233–242. Confidence: **established** — Tschinkel's lab + field studies are the definitive source on fire-ant founding-stage raiding. (Verified online — both citations confirmed.) Note: fire-ant founding raids are documented as *intraspecific*, but the size-asymmetry-decides-outcome mechanism applies directly to cross-species encounters.

---

### Finding 16 — Mature-colony territorial war is run by a two-tier chemical escalation system

**What happens in nature.** *Oecophylla* weaver ants maintain absolute territories via a dual-gland system: a **rectal-gland trail** recruits nestmates into unoccupied space (expansion) and persistently **marks owned territory with a colony-specific pheromone**; a **sternal-gland attractant-arrestant** assembles defensive clusters when intruders appear. Under sustained threat the rectal-gland substance escalates the response, mobilizing reinforcements to the combat zone. Workers respond to **alien-colony territorial marks** with intensified aggression, and a colony fighting on its **own marked ground has a competitive advantage**. *Oecophylla* and *Azteca trigona* both concentrate **larger major workers at territory borders** and recruit **selectively** — stronger responses to direct competitors/predatory ants than to harmless species.

**Mechanism.** Territory is held by a layered chemical system: persistent ownership marks (priority/home-ground signal) + short-range alarm clustering + long-range mass-recruitment escalation. Major workers garrison the borders; recruitment is threat-graded.

**Sim implication.** Mature-colony territory = the existing **ColonyScent** pheromone field. Workers on their own scent get a combat/morale bonus (home-ground advantage, Finding 18); detecting enemy scent triggers graded escalation (alarm cluster → mass recruitment). Bias larger/soldier-caste workers toward the colony-scent boundary. Make recruitment threat-graded: a stronger response to species flagged as direct competitors than to neutrals. All of this layers onto the existing pheromone grid with no new subsystems.

**Source.** Hölldobler, B. & Wilson, E. O. (1977). Weaver ants: social establishment and maintenance of territory. *Science* 195(4281): 900–902; Hölldobler, B. & Wilson, E. O. (1977). Colony-specific territorial pheromone in the African weaver ant *Oecophylla longinoda*. *PNAS* 74(5): 2072–2075; Adams, E. S. (1994). Territory defense by the ant *Azteca trigona*. *Oecologia* 97(2): 202–208. Confidence: **established/foundational**. (Verified online — citations confirmed.)

---

### Finding 17 — Nest architecture as a force-multiplier: chokepoints, plug-soldiers, and entrance-sealing flip the numbers

**What happens in nature.** Nest defensibility can **negate numerical disadvantage**. Underground passages enforce **Lanchester's *linear* law** — one-on-one fighting in a tunnel where individual combat ability matters more than total numbers (vs the open-field **square** law that rewards numbers). *Cephalotes* and *Carebara* deploy **phragmotic (plug-headed) soldiers** whose enlarged heads physically block entrances, converting a numbers deficit into a stalemate. *Dorymyrmex bicolor* drops stones into rival entrances to seal them; harvester ants plug entrances with soil when Argentine-ant raids begin (buying time); large *Atta* colonies repel army-ant raids by responding rapidly while the attack is still forming at the entrance. Staged *Iridomyrmex* (meat-ant) experiments confirmed that **restricted terrain improves the outnumbered defender's outcome**.

**Mechanism.** A narrow entrance forces sequential (linear-law) combat, so a defender with a chokepoint + a few elite blockers can hold against a much larger force that would win in the open. Architecture changes *which Lanchester law applies*.

**Sim implication.** This is the single most important *defensive* lever. Make nest **entrance width / tunnel topology** matter: combat resolved inside a narrow tunnel uses linear-law math (1v1, individual strength dominates) while open-surface combat uses square-law math (numbers dominate). Give certain species/castes an entrance-plug ability (a phragmotic soldier or a "seal the entrance" action) that halts attackers at a chokepoint. This lets a smaller, well-fortified colony survive a numerically superior raid — essential for balance and a faithful model of real ant warfare. Directly leverages the existing underground-nest tunnel layer.

**Source.** Champer, J. & Schlenoff, D. (2024). Battles between ants (Hymenoptera: Formicidae): a review. *Journal of Insect Science* 24(3): 25, doi:10.1093/jisesa/ieae064 (Lanchester linear vs square law; phragmosis; restricted-terrain effects synthesized here). Confidence: **established** — peer-reviewed 2024 review; verified authorship (an earlier draft of this research mis-attributed it to "Gokcekus et al." — that attribution was a hallucination and is discarded; correct authors are Champer & Schlenoff). (Verified online — OUP article + PMC mirror confirmed.)

---

## Part IV — Synthesis: Who Wins When Two Species Clash

### Finding 18 — Colony size / worker number is the strongest single predictor (causally, not just correlationally)

**What happens in nature.** In a guild of African *Acacia* ants (*Crematogaster*, *Tetraponera*), interspecific fights are **wars of attrition with ~1:1 mortality** (no individual-quality edge), and **average colony size is the best predictor of competitive rank** across all pairwise matchups. Critically, **experimentally inflating/deflating worker counts on individual trees *reversed* dominance rank** — establishing causation, not correlation. Invasive ants tend to have *smaller* workers, which lets a colony field **more combatants per unit biomass** — colony size (superorganism mass), not individual worker size, is the relevant scale.

**Mechanism.** When combat is attritional and roughly 1:1, the side with more bodies wins by simple arithmetic (Lanchester square law in the open). Building many cheap small workers beats building few expensive large ones for *interspecific* war on open ground.

**Sim implication.** Worker count must be the **primary** input to open-field combat resolution, with a square-law weighting (effective strength ∝ numbers², roughly) on open terrain. Make it a *causal* lever: a colony that out-produces its rival should win open battles even with individually weaker workers. This is the #1 ranked determinant — get this right first; everything else is a modifier on top.

**Source.** Palmer, T. M. (2004). Wars of attrition: colony size determines competitive outcomes in a guild of African acacia ants. *Animal Behaviour* 68: 993–1004. Confidence: **established/landmark** — the rare *experimental* (rank-reversal) demonstration of causation. (Verified online — citation confirmed.)

---

### Finding 19 — Recruitment speed & mass recruitment: a discovery–dominance trade-off, context-dependent

**What happens in nature.** Across invasive species, behavioral **dominance and discovery/recruitment speed are negatively correlated** (e.g., *Wasmannia auropunctata* won all fights but discovered resources slowest; *Linepithema humile* and *Pheidole megacephala* discovered fastest but lost fights; discovery-time vs dominance-rank r² = 0.905). Mass-recruitment systems also have a **minimum colony-size threshold** to function — below critical mass, pheromone trails can't be maintained, so small colonies fight on linear-law terms where numbers fully dominate. But the trade-off is **context-dependent / not universal** (see companion file 01, Findings 1–2): in many assemblages dominance and discovery correlate *positively*.

**Mechanism.** Behavioral dominance costs time (big workers, territorial aggression); fast discoverers are small and quick but lose contests. Which axis wins depends on resource type: contestable persistent patches reward dominators; ephemeral patches reward fast recruiters.

**Sim implication.** Keep `discovery_speed` and `interference_aggression` as **independent** species parameters (already established in companion file 01). Recruitment speed determines *who gets to a contested resource (or a forming raid) first*; dominance determines *who wins the ensuing fight*. Below a colony-size threshold, a species loses access to mass recruitment (trails decay) — small colonies fight 1v1. Don't hard-code the trade-off as universal.

**Source.** Bertelsmeier, C., Avril, A., Blight, O., Jourdan, H. & Courchamp, F. (2015). Discovery–dominance trade-off among widespread invasive ant species. *Ecology and Evolution* 5(13): 2673–2683; Planqué, R., van den Berg, J. B. & Franks, N. R. (2010). Recruitment strategies and colony size in ants. *PLOS ONE* 5(8): e11664. Confidence: **established**. (Verified online — both citations confirmed; trade-off universality critique cross-referenced to Parr & Gibb 2012 in companion file 01.)

---

### Finding 20 — Body size, polymorphism & coordinated caste tactics multiply effective fighting power

**What happens in nature.** Groups with higher mean worker mass are more likely to inflict the first casualty — **but this advantage vanishes in restricted terrain** (linear law). *Pheidole pallidula* minors immobilize enemies while majors deliver killing blows (coordinated caste division multiplies power beyond individual size). *Formica xerophila* uses **team fighting** (several workers gang one opponent), converting numerical edge directly into lethality. Some species (*Crematogaster striatula*) **spray venom at range**, forcing retreat without close engagement.

**Mechanism.** Soldier castes and coordinated tactics let a colony concentrate force; ganging converts numbers into kills super-linearly; ranged chemical weapons let a side deal damage without exposure. But chokepoints (linear law) neutralize the size/mass edge.

**Sim implication.** Body size / soldier caste = a per-worker `combat_power` multiplier that matters most in **open** combat and is **damped in tunnels** (linear law). Implement team-fighting: when multiple attackers focus one defender, apply a focus-fire bonus. Ranged-venom species (Finding 21) attack from a cell away. Polymorphic species (`polymorphic = true` in the TOML) can field majors with higher combat_power — a distinct path to winning vs raw numbers.

**Source.** Champer, J. & Schlenoff, D. (2024). Battles between ants. *Journal of Insect Science* 24(3): 25 (synthesizing Detrain & Pasteels 1992 on *Pheidole pallidula*; Tanner 2006 on *Formica xerophila* team-fighting). Confidence: **established** — peer-reviewed review synthesizing the primary combat-tactics literature. (Verified online — review confirmed; correct authorship Champer & Schlenoff.)

---

### Finding 21 — Chemical weapons (venom toxicity) can be categorically decisive — and counterable

**What happens in nature.** *Solenopsis invicta* venom is >90% piperidine alkaloids (solenopsins); a single gaster-flagging event disperses ~0.50 µg, slightly **exceeding the lethal dose for Argentine ants (0.489 µg)** — each flagging is reliably lethal to *L. humile*, which is **~330× more susceptible** to fire-ant venom than fire ants are to their own. But chemical weapons are **counterable**: *Nylanderia fulva* (tawny crazy ant), when stung, secretes **formic acid and applies it to its own cuticle as an antidote** — 98% survival with detoxification vs 48% without — which is the proximate mechanism letting crazy ants currently displace fire ants. And alkaloid venom can disrupt nestmate recognition: *Megalomyrmex* guests' pyrrolizidine venom made *Gnamptogenys* raiders attack their own colony-mates.

**Mechanism.** Toxic venom kills heterospecifics far below the dose tolerated by the producer (asymmetric susceptibility) — a force-multiplier independent of numbers. But a counter-adapted opponent can detoxify, neutralizing the weapon. Some venoms also act as anti-recognition agents (friendly-fire inducers).

**Sim implication.** Give species a `chemical_weapon` profile with (a) a damage value and (b) a **target-specific susceptibility matrix** (species A's venom is devastating to B but weak vs C). This lets a small colony with a potent venom punch above its weight (matches `brachyponera_chinensis` sting flavor in companion file 01). Add an optional `detox` counter-trait that reduces incoming chemical damage (the crazy-ant counter). Venom can also be a friendly-fire inducer (ties to propaganda, Finding 5). Asymmetric chemical matchups are a great source of rock-paper-scissors balance.

**Source.** Xu, G. & Chen, L. (2023). Biological activities and ecological significance of fire ant venom alkaloids. *Toxins* 15(7): 439; LeBrun, E. G. et al. (2014). Chemical warfare among invaders: a detoxification interaction facilitates an ant invasion. *Science* 343(6174): 1014–1017; Adams, R. M. M. et al. (2013). Chemically armed mercenary ants protect fungus-farming societies. *PNAS* 110(39). Confidence: **established**. (Verified online — all three citations confirmed.)

---

### Finding 22 — Priority / residency effects: the incumbent has a real, measurable edge

**What happens in nature.** *Novomessor cockerelli* colonies **plug the entrances of neighboring *Pogonomyrmex barbatus* nests before sunrise**, delaying the larger rival's foraging by 1–2 hours — a resident preemptively neutralizing a bigger competitor from a fixed territory. More generally, resource patches discovered first are aggressively defended and tend to stay with the discoverer; emigrating colonies actively choose nest sites *furthest* from established colonies, reflecting that residency imposes a real cost on challengers. *Oecophylla* on its own pheromone-marked ground wins fights it might lose elsewhere (Finding 16).

**Mechanism.** The incumbent knows the terrain, has its territory chemically marked (home-ground combat bonus), and can act first (preemptive interference). Priority effects mean *order of arrival* partly determines outcome independent of strength.

**Sim implication.** Implement a **home-ground bonus**: workers fighting within their own ColonyScent field get a combat/morale multiplier; attackers in enemy scent get a penalty. Add a preemptive-interference option (a resident can sabotage/seal a rival's entrance to delay its activity). This makes attacking an established nest meaningfully harder than meeting in neutral territory — a key balance counterweight to the pure-numbers determinant (Finding 18).

**Source.** Gordon, D. M. (1992). Nest-plugging: interference competition in desert ants (*Novomessor cockerelli* and *Pogonomyrmex barbatus*). *Oecologia* 92: 1–7; plus the *Oecophylla* home-ground advantage of Finding 16. Confidence: **established**. (Verified online — Gordon citation confirmed.)

---

### Finding 23 — Founding-stage vulnerability: incipient colonies are displaced by incumbents across species lines

**What happens in nature.** Colony founding fails at rates **up to ~99%** in some species; the claustral/incipient phase (1–50 workers) is the maximum-vulnerability window. Workers from **mature** *S. invicta* colonies cause the majority of foundress-queen mortality. Even dominant invasive *S. invicta* queens preferentially found in **disturbed habitats lacking established native ant communities** — where native communities are intact, new fire-ant colonies are suppressed before reaching maturity. Pleometrosis (cooperative multi-queen founding) measurably raises survival through the bottleneck.

**Mechanism.** A founding colony below critical size can't run mass recruitment, can't simultaneously defend + tend brood + forage, and is fought on linear-law terms where numbers dominate — so any established neighbor crushes it. Incumbents suppress incipient heterospecific colonies before they mature.

**Sim implication.** The founding/early colony is the **decisive strategic vulnerability** — and the natural place to put the queen-kill payoff (a founding colony's queen is reachable; a mature fortified colony's is not, per Findings 8 & 17). Make founding colonies fragile: low worker count → linear-law combat → easily overrun; nearby mature enemy colonies can suppress a new colony before it matures. Pleometrosis (multi-queen start) as a risk/reward option that boosts early survival. This frames the whole arena: **win by killing the enemy queen while the enemy is still vulnerable, or by out-growing them into invulnerability.**

**Source.** Tschinkel, W. R. et al. (2017). Ant community and habitat limit colony establishment by the fire ant, *Solenopsis invicta*. *Functional Ecology*; Jerome, C. A., McInnes, D. A. & Adams, E. S. (1998). Group defense by colony-founding queens in the fire ant *Solenopsis invicta*. *Behavioral Ecology* 9(3): 301–308. Confidence: **established**. (Verified online — both citations confirmed.)

---

### Finding 24 — Unicoloniality (invasive-specialist trait): loss of intraspecific aggression frees all force for interspecific war

**What happens in nature.** Introduced Argentine ants from thousands of km apart treat each other as nestmates (no intraspecific aggression), effectively pooling a continental supercolony; energy formerly spent on intraspecific war is redirected to interspecific combat. This is identified as a root cause of invasive dominance, alongside numerical abundance (1–2 orders of magnitude denser than natives) and superior recruitment. (Detailed in companion file 01, Findings 5–6.)

**Mechanism.** Eliminating nestmate-recognition boundaries between nests means the entire regional population can be brought to bear on any single interspecific fight — effectively unbounded numbers (Finding 18 taken to its limit).

**Sim implication.** A `unicolonial` species trait whose effective combat pool scales with **local density across satellite nests**, not single-colony worker count — the supercolony force-multiplier. Polydomy/budding species (e.g., *B. chinensis* per companion file 01) get a partial version: satellite-nest workers can reinforce a fight. This is the "flooding" win-path, distinct from the big-workers path (Finding 20).

**Source.** Holway, D. A., Suarez, A. V. & Case, T. J. (1998). Loss of intraspecific aggression in the success of a widespread invasive social insect. *Science* 282: 949–952; Holway, D. A. et al. (2002). The causes and consequences of ant invasions. *Annual Review of Ecology, Evolution, and Systematics* 33: 181–233. Confidence: **established/landmark**. (Verified online — both citations confirmed; see companion file 01 for fuller treatment.)

---

## "Who Wins" Synthesis Table

| Factor | Effect on outcome | How to model it in antcolony | Strongest source |
|---|---|---|---|
| **Colony size / worker count** | #1 predictor; causal (rank reverses when sizes swapped). Open combat ≈ war of attrition, 1:1 mortality → more bodies win. | Primary input to combat resolution; **square-law** weighting on open terrain (strength ∝ numbers²). Make production→numbers the core win-path. | Palmer 2004 (experimental reversal) |
| **Nest architecture / chokepoints** | Negates numerical disadvantage; tunnel = **linear law** (1v1), open = square law. Plug-soldiers + entrance-sealing flip the numbers. | Entrance width / tunnel topology selects Lanchester law. Phragmotic "plug" + "seal entrance" abilities let a small fortified colony hold. | Champer & Schlenoff 2024 |
| **Chemical weapons (venom)** | Can be categorically decisive (fire-ant venom 1-shot kills Argentine ants); **asymmetric** susceptibility; counterable by detox. | `chemical_weapon` damage + **species susceptibility matrix** + optional `detox` counter-trait. Source of rock-paper-scissors. | Xu & Chen 2023; LeBrun et al. 2014 |
| **Recruitment / discovery speed** | Determines who reaches a contested resource/forming-raid first; trades off vs dominance (context-dependent). Below colony-size threshold, no mass recruitment. | Independent `discovery_speed` param; mass-recruitment gated on min colony size; first-arrival claims, dominance decides the fight. | Bertelsmeier et al. 2015; Planqué et al. 2010 |
| **Body size / polymorphism / team-tactics** | Higher mean mass → first casualty (open only; nullified in tunnels). Soldier castes + ganging multiply force. | Per-worker `combat_power` (matters in open, damped in tunnels); focus-fire bonus when several gang one; majors for polymorphic species. | Champer & Schlenoff 2024 |
| **Priority / residency (home ground)** | Incumbent edge: own marked territory = combat bonus; preemptive interference (entrance-plugging) delays rivals. | Home-ground combat/morale multiplier inside own ColonyScent; attacker penalty in enemy scent; preemptive-sabotage option. | Palmer 2004; Gordon 1992; Hölldobler & Wilson 1977 |
| **Founding-stage vulnerability** | Incipient colonies (1–50 workers) crushed by incumbents across species lines; founding failure up to ~99%. | Founding colonies fragile (linear-law, easily overrun); mature neighbors suppress new colonies; pleometrosis = risk/reward survival boost. | Tschinkel et al. 2017; Jerome et al. 1998 |
| **Unicoloniality / polydomy** | Loss of intraspecific aggression → entire regional population fights as one (Finding 18 to the limit). | `unicolonial` trait: combat pool scales with local multi-nest density; budding species get partial reinforcement. | Holway et al. 1998, 2002 |
| **Propaganda / chemical deception** | Disorganizes the defender (panic, friendly-fire) rather than disguising the attacker; force-multiplier on offense. | Raid ability: spike defender Alarm field with *inverted* effect (flee bias + disorganization debuff → fewer effective defenders). | Regnier & Wilson 1971; Brandt et al. 2006 |
| **Queen vulnerability gating** | Queen-kill is **timed/conditional** (parasite waits for a useful workforce); acceptance needs post-kill disguise; interruptible. | Queen-kill gated on attacker dominance/occupation; 2-phase channeled usurpation (vulnerable → disguise-acquire); defenders can interrupt. | Johnson et al. 2002; Topoff & Zimmerli 1993 |

---

## Cross-Species Queen-Kill Mechanic — Realism Verdict & Recommended Model

The win-by-killing-the-enemy-queen mechanic is **biologically excellent** — it is, almost exactly, ant social-parasitic colony usurpation. Five real mechanisms map onto it, in increasing chemical sophistication:

1. **Direct mandible kill** (brute force) — *Formica rufa*, *Polyergus* (Finding 10). Default template; high risk if defenders present.
2. **Slow throttle** (channeled grapple over time) — *Lasius reginae*, *Myrmoxenus* (Finding 11).
3. **Decapitation / antenna-amputation** (targeted dismemberment, ± stealth entry) — *Bothriomyrmex*, *L. goesswaldi* (Finding 12).
4. **Post-kill CHC disguise** (kill → acquire the dead queen's identity → workers accept the new reproductive) — *Polyergus* (Finding 9).
5. **Chemical-induced matricide** (don't fight the queen — make her *own workers* kill her) — *Lasius umbratus/orientalis*, 2025 (Finding 13).

**The two findings that make it a *balanced* mechanic, not a degenerate instant-win:**
- **Timing gate (Finding 8):** parasites do NOT snipe a queen early — they wait until the host workforce is large enough to be worth taking. → The enemy queen should be *invulnerable until the attacker has established enough dominance/occupation*. Kills no rush-snipe at t=0.
- **Disguise/acceptance step (Finding 9) + interruptibility:** the kill is a multi-second channel during which the attacker is exposed (defenders can interrupt), and success requires acquiring the dead queen's colony identity to win the workforce. → Two-phase, defender-counterable usurpation with a satisfying "the enemy's workers are now yours" payoff.

**Recommended sim model:** queen-kill = a **gated, two-phase, interruptible channel**. Phase 0 (gate): enemy queen only becomes targetable once attacker occupies the nest / out-dominates defenders locally (Finding 8, 17, 23). Phase 1 (channel): attacker engages the queen over N ticks, exposed to interruption; a Dufour-style "repellent"/disguise trait extends the protection window (Finding 9, 14). Phase 2 (resolution): on success, attacker "acquires colony identity" → surviving defender workers stop fighting and may defect (Finding 9, 15); colony flips to attacker over time as host workers attrit (Finding 10). Species variants swap the *flavor* (brute kill / throttle / decapitate / matricide-by-proxy) and the *risk profile* (assassin-stealth vs army-brute, Finding 14). This is faithful to biology AND solves the obvious balance problem (no early queen-snipe; defenders get counter-play).

---

## Sources

**Slave-making / dulosis raids:**
- Buschinger, A., Ehrhardt, W. & Winter, U. (1980). The organization of slave raids in dulotic ants — a comparative study. *Zeitschrift für Tierpsychologie* 53: 245–264. (Verified online)
- Topoff, H., Lamon, B., Goodloe, L. & Goldstein, M. (1984). Social and orientation behavior of *Polyergus breviceps* during slave-making raids. *Behavioral Ecology and Sociobiology* 15: 273–279. (Verified online)
- Topoff, H. & Cover, S. (1989). Behavioral adaptations for raiding in the slave-making ant *Polyergus breviceps*. *Journal of Insect Behavior* 2: 545–556. (Verified online)
- Hölldobler, B. & Wilson, E. O. (1990). *The Ants*, Ch. 12 "Slavery", pp. 444–478. Harvard/Belknap. (Verified online — chapter/pages)
- Mori, A., Grasso, D. A. & Le Moli, F. (2000). Raiding and foraging behavior of the blood-red ant, *Formica sanguinea*. *Journal of Insect Behavior* 13: 421–438. (Verified online)
- Mori, A., Grasso, D. A., Visicchio, R. & Le Moli, F. (2001). Comparison of reproductive strategies and raiding behaviour in facultative and obligatory slave-making ants: *Formica sanguinea* and *Polyergus rufescens*. *Insectes Sociaux* 48: 302–314. (Verified online)
- Le Moli, F., Mori, A., Grasso, D. A. & Ugolini, A. (1994). Eco-ethological factors affecting scouting and raiding behaviour of *Polyergus rufescens*. *Ethology* 96: 289–302. (Verified online)
- Regnier, F. E. & Wilson, E. O. (1971). Chemical communication and "propaganda" in slave-maker ants. *Science* 172(3980): 267–269. (Verified online)
- Brandt, M., Fischer-Blass, B., Heinze, J. & Foitzik, S. (2006). Convergent evolution of the Dufour's gland secretion as a propaganda substance in *Protomognathus* and *Harpagoxenus*. *Insectes Sociaux* 53(4). (Verified online)
- Hare, J. F. & Alloway, T. M. (2001). Prudent *Protomognathus* and despotic *Leptothorax duloticus*: differential costs of ant slavery. *PNAS* 98(21). (Verified online)
- Achenbach, A. & Foitzik, S. (2009). First evidence for slave rebellion. *Evolution* 63(4): 1068–1075. (Verified online)
- Pamminger, T. et al. (2012). Geographic distribution of the anti-parasite trait "slave rebellion". *Evolutionary Ecology*. (Verified online)
- Kleeberg, I., Menzel, F. & Foitzik, S. (2017). The influence of slavemaking lifestyle, caste and sex on chemical profiles in *Temnothorax* ants. *Proc. R. Soc. B* 284: 20162249. (Verified online)

**Social parasitism / usurpation / queen-killing:**
- Buschinger, A. (2009). Social parasitism among ants: a review. *Myrmecological News* 12: 219–235. (Verified online)
- Lenoir, A., D'Ettorre, P., Errard, C. & Hefetz, A. (2001). Chemical ecology and social parasitism in ants. *Annual Review of Entomology* 46: 573–599. (Verified online)
- Johnson, C. A., Topoff, H., Vander Meer, R. K. & Lavine, B. (2002). Host queen killing by a slave-maker ant queen: when is a host queen worth attacking? *Animal Behaviour* 64: 807–815. (Verified online — full text)
- Topoff, H. & Zimmerli, E. (1993). Colony takeover by a socially parasitic ant, *Polyergus breviceps*: the role of chemicals obtained during host-queen killing. *Animal Behaviour* 46: 479–486. (Verified online — DOI 10.1006/anbe.1993.1216)
- Johnson, C. A., Vander Meer, R. K. & Lavine, B. (2001). Changes in the cuticular hydrocarbon profile of *Polyergus breviceps* after killing a *Formica* host queen. *Journal of Chemical Ecology* 27(9): 1787–1804. (Verified online)
- Topoff, H. et al. (1988). Colony founding by queens of *Polyergus breviceps*: the role of the Dufour's gland. *Ethology* 78: 209–218. (Verified online)
- Borowiec, M. L., Cover, S. P. & Rabeling, C. (2021). The evolution of social parasitism in *Formica* ants revealed by a global phylogeny. *PNAS* 118(38): e2026029118. (Verified online)
- Heinze, J., Buschinger, A., Poettinger, T. & Suefuji, M. (2015). Multiple convergent origins of workerlessness and inbreeding in *Myrmoxenus*. *PLoS ONE* 10(7): e0131023. (Verified online)
- Buschinger, A. & Klump, B. (1988). Novel strategy of host-queen removal in *Leptothorax goesswaldi*. *Naturwissenschaften* 75: 422–423. (Verified online)
- Wilson, E. O. (1971). *The Insect Societies*. Harvard/Belknap. (*Bothriomyrmex decapitans* account, after Santschi ~1906; *F. rufa* "inept parasite") (Verified online — secondary syntheses; foundational text)
- Savolainen, R. & Vepsäläinen, K. (2003). Sympatric speciation through intraspecific social parasitism. *PNAS* 100(12): 7169–7174. (Verified online)
- **Shimada, T., Tanaka, Y. & Takasuka, K. (2025). Socially parasitic ant queens chemically induce queen-matricide in host workers. *Current Biology* 35(22). doi:10.1016/j.cub.2025.09.037.** (Verified online — CrossRef DOI + Kyushu release + multiple press)

**Territorial war / brood raiding / who-wins synthesis:**
- Tschinkel, W. R. (1992). Brood raiding in the fire ant, *Solenopsis invicta*. *Annals of the Entomological Society of America* 85(5): 638–646. (Verified online)
- Adams, E. S. & Tschinkel, W. R. (1995). Effects of foundress number on brood raids and queen survival in *Solenopsis invicta*. *Behavioral Ecology and Sociobiology* 37: 233–242. (Verified online)
- Jerome, C. A., McInnes, D. A. & Adams, E. S. (1998). Group defense by colony-founding queens in *Solenopsis invicta*. *Behavioral Ecology* 9(3): 301–308. (Verified online)
- Tschinkel, W. R. et al. (2017). Ant community and habitat limit colony establishment by *Solenopsis invicta*. *Functional Ecology*. (Verified online)
- Hölldobler, B. & Wilson, E. O. (1977). Weaver ants: social establishment and maintenance of territory. *Science* 195(4281): 900–902. (Verified online)
- Hölldobler, B. & Wilson, E. O. (1977). Colony-specific territorial pheromone in *Oecophylla longinoda*. *PNAS* 74(5): 2072–2075. (Verified online)
- Adams, E. S. (1994). Territory defense by the ant *Azteca trigona*. *Oecologia* 97(2): 202–208. (Verified online)
- Champer, J. & Schlenoff, D. (2024). Battles between ants (Hymenoptera: Formicidae): a review. *Journal of Insect Science* 24(3): 25. doi:10.1093/jisesa/ieae064. (Verified online — authorship corrected from an earlier mis-attribution to "Gokcekus et al.")
- Palmer, T. M. (2004). Wars of attrition: colony size determines competitive outcomes in a guild of African acacia ants. *Animal Behaviour* 68: 993–1004. (Verified online)
- Bertelsmeier, C., Avril, A., Blight, O., Jourdan, H. & Courchamp, F. (2015). Discovery–dominance trade-off among widespread invasive ant species. *Ecology and Evolution* 5(13): 2673–2683. (Verified online)
- Planqué, R., van den Berg, J. B. & Franks, N. R. (2010). Recruitment strategies and colony size in ants. *PLOS ONE* 5(8): e11664. (Verified online)
- Xu, G. & Chen, L. (2023). Biological activities and ecological significance of fire ant venom alkaloids. *Toxins* 15(7): 439. (Verified online)
- LeBrun, E. G. et al. (2014). Chemical warfare among invaders: a detoxification interaction facilitates an ant invasion. *Science* 343(6174): 1014–1017. (Verified online)
- Adams, R. M. M. et al. (2013). Chemically armed mercenary ants protect fungus-farming societies. *PNAS* 110(39). (Verified online)
- Gordon, D. M. (1992). Nest-plugging: interference competition in desert ants. *Oecologia* 92: 1–7. (Verified online)
- Holway, D. A., Suarez, A. V. & Case, T. J. (1998). Loss of intraspecific aggression in the success of a widespread invasive social insect. *Science* 282: 949–952. (Verified online)
- Holway, D. A., Lach, L., Suarez, A. V., Tsutsui, N. D. & Case, T. J. (2002). The causes and consequences of ant invasions. *Annual Review of Ecology, Evolution, and Systematics* 33: 181–233. (Verified online)
- Hölldobler, B. & Wilson, E. O. (1990). *The Ants* — Ch. 11 community ecology / dominance hierarchies. Harvard/Belknap. (Verified online — foundational)
- Lach, L., Parr, C. L. & Abbott, K. L. (eds.) (2010). *Ant Ecology*. Oxford University Press. (Verified online — interspecific competition chapter)
- Savolainen, R. & Vepsäläinen, K. (1988). A competition hierarchy among boreal ants. *Oikos* 51(2): 135–155. (Verified online)

**Confidence summary.** All findings rest on **verified-online** primary or review literature (citations confirmed against publisher pages, DOIs, or full text). Three claims got dedicated adversarial re-verification: Shimada et al. 2025 (VERIFIED via CrossRef DOI), Champer & Schlenoff 2024 authorship (CORRECTED — "Gokcekus et al." was a hallucination), and the *Polyergus* queen-kill papers Topoff & Zimmerli 1993 + Johnson et al. 2002 (both VERIFIED full-text; refinements folded in — the *Polyergus* attack *trigger* is a maturation signal, NOT CHCs; CHCs govern post-kill *acceptance*). General-knowledge / textbook-via-secondary items (flagged inline): the *Bothriomyrmex decapitans* decapitation account (Santschi/Wilson — long-standing textbook case) and the precise *Lasius reginae* throttling detail (Buschinger review/secondary). No claims were refuted.

---

## Key Sim Levers (priority-ordered)

1. **Open-field combat = numbers-dominant (square law); tunnel combat = individual-dominant (linear law).** Worker count is the #1 causal determinant (Palmer 2004) — get this first. Entrance/topology selects which law applies (Champer & Schlenoff 2024).
2. **Cross-species queen-kill = gated, two-phase, interruptible channel.** Gate on attacker dominance/occupation (no early snipe — Johnson et al. 2002); channel exposes the attacker (defenders interrupt); success = acquire colony identity → defender workers defect (Topoff & Zimmerli 1993). This is the central mechanic and its balance solution in one.
3. **Brood raiding as the early-game weapon.** Steal brood (pupae/larvae) → matures into raider workers; outcome dominated by worker count; losing workers can defect (Tschinkel 1992). Separate economic win-path from the queen-kill.
4. **Raid pipeline FSM:** scout → recruit (quorum) → raid (column on scout trail) → haul. Per-species `raid_organization` (obligate mass / facultative trickle); ~30% baseline failure; weather/thermal-gated (Buschinger et al. 1980; Mori et al. 2000).
5. **Founding-stage = decisive vulnerability window.** Incipient colonies fragile (linear-law, overrun by incumbents); mature neighbors suppress new colonies; pleometrosis as risk/reward (Tschinkel et al. 2017; Jerome et al. 1998). Frames the arena: kill the queen while vulnerable, or out-grow into invulnerability.
6. **Chemical weapons = asymmetric force-multiplier with a susceptibility matrix + optional detox counter** (Xu & Chen 2023; LeBrun et al. 2014). Source of rock-paper-scissors balance.
7. **Propaganda ability** (slave-maker archetypes): spike the defender's Alarm field with inverted effect (flee + disorganization debuff → fewer effective defenders) (Regnier & Wilson 1971).
8. **Home-ground advantage:** combat/morale bonus inside own ColonyScent; penalty in enemy scent; preemptive-sabotage option (entrance-plug) (Gordon 1992; Hölldobler & Wilson 1977).
9. **Independent `discovery_speed` vs `interference_aggression`** (already in companion file 01); mass recruitment gated on min colony size (Bertelsmeier 2015; Planqué 2010).
10. **Species win-archetypes** (each needs a different counter): big-workers/polymorphism (Palmer/Champer), flooding/unicoloniality (Holway), venom-specialist (Xu & Chen), assassin/parasite (Topoff). Plus the matricide-by-proxy "turncoat" usurpation (Shimada 2025) as a showcase chemical archetype.
11. **Optional host counter-adaptations:** slave rebellion (enslaved brood sabotages raider queen-brood) (Achenbach & Foitzik 2009); venom detox (LeBrun et al. 2014). Give victims passive counter-play.

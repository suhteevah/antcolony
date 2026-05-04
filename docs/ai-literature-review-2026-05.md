# AI Literature Review — Putting an Ant Colony's Collective Mind into a Computational Model

**Compiled:** 2026-05-03
**Scope:** Latest research (priority Q4 2025 → May 2026) on neural / RL / active-inference / QD methods for modeling ant-colony collective cognition.
**Caller:** antcolony Rust/Bevy simulation. Current state: 17→64→64→6 MLP, behavior-cloned on archetype-tournament data. Hard ceiling at ~45.7% mean win rate (BC asymptotes near teacher mean by construction).

---

## Executive Summary

1. **Behavior cloning is not the bottleneck — the *teacher distribution* is.** The 2025 literature is unanimous that BC over a balanced expert pool cannot exceed the average expert; you have to either (a) move to interactive / hybrid IL with a recovery-cost framing (Ren et al., NeurIPS 2025), (b) move to self-play / fictitious play (SPIRAL, MARSHAL, AlphaStar-style league), or (c) move to QD-RL (PGA-MAP-Elites, Dominated Novelty Search) so the agent can *exceed* the archetypes it was bootstrapped from.
2. **Stigmergic MARL is the dominant 2025 paradigm for ant-like collectives.** Three highly-relevant arXiv preprints (S-MADRL Oct 2025, "From Pheromones to Policies" Sep 2025, "Emergent Collective Memory" Dec 2025) all formalize what we already have — the pheromone grid as the communication medium — but treat it as a learned *observation channel* for a deep policy rather than a hand-tuned ACO heuristic. This is the architectural change with highest expected payoff.
3. **The "colony mind" has a precise mathematical definition now.** Soma et al. (Bridging Swarm Intelligence and RL, NeurIPS-track 2024/2025, arXiv 2410.17517 — "The Hive Mind is a Single Reinforcement Learning Agent") prove an equivalence between colony-level behavior (emergent from local imitation) and a single online RL agent over many parallel envs. The colony **is** the agent. This reframes our entire approach: train one colony-level policy whose execution is decentralized.
4. **Hypernetworks make the colony-mind / per-ant split tractable.** HyperMARL (Tessera et al., Dec 2024 → 2025) uses an agent-conditioned hypernetwork to emit per-ant actor parameters from a single colony brain — exactly the architecture you need for caste/role specialization without training N separate networks.
5. **Real ant-brain neuroscience has matured into transferable circuit models.** Collett, Graham & Heinze (Current Biology, Feb 2025) is the new canonical neuroethology review; Frank & Kronauer (Annual Review of Neuroscience, 2024) is the social-behavior counterpart. The MB+CX (mushroom body + central complex) circuit is now concrete enough to drop into a sim as a 50-200 neuron module. This is the path to "actually putting an ant brain in" instead of an abstract MLP.

**Top headline recommendation:** Replace the BC-on-archetypes bootstrap with **Stigmergic Multi-Agent Deep RL using a colony-level hypernetwork actor + centralized critic**, trained via league-style self-play seeded from the existing archetypes as initial league members. Detailed below.

---

## 1. Architecture for Collective Cognition

### 1.1 The colony is one RL agent — formal result

- **Soma, Bouteiller, Hamann & Beltrame — "The Hive Mind is a Single Reinforcement Learning Agent" / "Bridging Swarm Intelligence and Reinforcement Learning"** — arXiv 2410.17517, latest revision 2025. <https://arxiv.org/abs/2410.17517>
  - Proves the weighted voter model used in honeybee nest selection aggregates exactly to a single online RL agent with parallel environments. Derives **Maynard-Cross Learning** as a biologically-plausible update rule.
  - **Why it matters for us:** This invalidates the framing "train per-ant policy and hope colony intelligence emerges." The math says the colony itself is the agent and the individual ants are the *exploration distribution*. This is a deep architectural prior for how we should set up reward attribution.
  - **Status:** *Directly applicable, foundational.*

### 1.2 Centralized-critic / decentralized-actor (CTDE) — current best practice

- **Yu et al. — "The Surprising Effectiveness of MAPPO in Cooperative Multi-Agent Games"** (BAIR 2021, still the SOTA reference baseline in 2025). <https://bair.berkeley.edu/blog/2021/07/14/mappo/>
  - MAPPO with a shared centralized critic + decentralized actors matches/beats QMIX, VDN, MADDPG on most benchmarks.
- **Lyu et al. — "On Centralized Critics in Multi-Agent RL"** (JAIR 2023). <https://www.khoury.northeastern.edu/home/abaisero/assets/publications/repository/lyu_centralized_2023.pdf>
  - Shows centralized critics reduce variance more than they help bias under most conditions.
- **2025 survey: "Centralized Training for Decentralized Execution in MARL"** — scisimple.com summary of 2025 advances including OPT-QMIX and MAGPO. <https://scisimple.com/en/articles/2025-06-17-centralized-training-for-decentralized-execution-in-multi-agent-reinforcement-learning--a3766dm>
  - **MAGPO** (2025): autoregressive guider for joint coordinated exploration during training.
- **STACCA** — arXiv 2511.13103 (Nov 2025). <https://arxiv.org/abs/2511.13103>
  - Shared Transformer Actor + Graph Transformer Critic for long-range MARL on networked systems. Closest off-the-shelf architecture to "spatial pheromone grid + many ants".

### 1.3 Hypernetworks — per-ant policy specialization without parameter explosion

- **Tessera, Mahjoub, et al. — "HyperMARL: Adaptive Hypernetworks for Multi-Agent RL"** — arXiv 2412.04233 (Dec 2024, refined 2025). <https://arxiv.org/abs/2412.04233>
  - Agent-conditioned hypernetwork emits actor + critic params per agent. Decouples observation- and agent-conditioned gradients to prevent interference. Specifically built to balance shared-parameter vs per-agent-specialization tradeoffs.
  - **Why it matters for us:** This is *the* architecture for putting a `caste` and a `colony_id` into a single colony brain that emits a per-ant policy. Solves "do we have one network or seven (one per caste)" cleanly: one network conditioned on caste/species token.
  - **Status:** *Directly applicable, top-3 read.*
- **Hypernetwork-Based Approach for Optimal Composition Design in Partially Controlled MAS** — arXiv 2502.12605 (Feb 2025). <https://arxiv.org/abs/2502.12605>
  - First framework to use hypernetworks for *system-composition design* in MAS — directly relevant if we ever want to evolve caste ratios as part of training.
- **Generalizable Agent Modeling with Multi-Retrieval and Dynamic Generation (MRDG)** — arXiv 2506.16718 (Jun 2025). <https://arxiv.org/abs/2506.16718>
  - Hypernetwork module generates policy parameters for collaboration-competition adaptation. Useful for the "colony A vs colony B" tournament setting.

### 1.4 Population-of-policies vs single-policy

- **Pierrot et al. — "Evolving Populations of Diverse RL Agents with MAP-Elites"** — arXiv 2303.12803 (still the canonical reference, ICLR 2023, used as baseline through 2025). <https://arxiv.org/abs/2303.12803>
- **TLeague: Competitive Self-Play Distributed MARL** — arXiv 2011.12895; AlphaStar-style league, still the architecture template for population-based competitive training.
- **A Survey on Population-Based Deep RL** (MDPI 2023). <https://www.mdpi.com/2227-7390/11/10/2234>

**Architecture takeaway:** The 2025 consensus is **CTDE + hypernetwork actor + transformer critic + league/QD outer loop**. Each piece has matured independently; combining them is now standard.

---

## 2. Recent Neuroscience-Grounded Ant Cognition Models

### 2.1 The two flagship 2024–2025 reviews — read both

- **Collett, Graham & Heinze — "The neuroethology of ant navigation"** — *Current Biology* 35, R76–R91 (Feb 3, 2025). <https://www.cell.com/current-biology/fulltext/S0960-9822(24)01702-0> | <https://pubmed.ncbi.nlm.nih.gov/39904309/>
  - Comprehensive review of central complex (CX), mushroom bodies (MB), lateral accessory lobes (LAL) and their roles in path integration, view-based homing, multimodal cue integration.
  - **Why it matters:** This is the *blueprint* for a biologically-grounded ant brain module. Every circuit needed to replace our 17-feature MLP is described here at sufficient detail to implement.
  - **Status:** *Read first.*
- **Frank & Kronauer — "The Budding Neuroscience of Ant Social Behavior"** — *Annual Review of Neuroscience* 47 (Aug 2024). <https://www.annualreviews.org/content/journals/10.1146/annurev-neuro-083023-102101> | <https://pubmed.ncbi.nlm.nih.gov/38603564/>
  - Maps ant-specific neural specializations onto the conserved insect-brain plan. Covers Ooceraea biroi (clonal raider ant) which has become the genetic model. Discusses neuropeptidergic and cuticular-hydrocarbon circuits driving caste, task, and recognition.
  - **Why it matters:** Closest you can get to a "circuit-level wiring diagram for sociality." Not just navigation — task allocation, nestmate recognition, caste-specific sensorimotor profiles.
  - **Status:** *Read first.*

### 2.2 Mushroom-body computational models

- **Buckley, Webb, Nowotny et al. — "Investigating visual navigation using spiking neural network models of the insect mushroom bodies"** — *Frontiers in Physiology* 15 (Jun 2024). <https://www.frontiersin.org/journals/physiology/articles/10.3389/fphys.2024.1379977/full>
  - GeNN-based GPU-accelerated SNN. ~14% drop-off real-world route error of 14 cm on 6.5 m route with a *biologically plausible MB model*. Embodied on robots.
  - **Why it matters:** Working code + working biological model. We can essentially port this into Rust as the visual/scent module of an ant.
- **"Hybrid neural networks in the mushroom body drive olfactory preference in Drosophila"** — *Science Advances* (2024). <https://www.science.org/doi/10.1126/sciadv.adq9893>
  - Updates the long-held "random projection" KC model: connections are *non-random and food-/pheromone-segregated*. Implies our single-pheromone-channel sense should be split into multiple labelled channels feeding distinct downstream KCs.
- **Ardin et al. (foundational, 2016, still cited heavily in 2025) — "Using an Insect Mushroom Body Circuit to Encode Route Memory"** — PMC. <https://pmc.ncbi.nlm.nih.gov/articles/PMC4750948/>
- **Le Moël et al. — "Reinforcement Learning as a Robotics-Inspired Framework for Insect Navigation"** — arXiv 2406.01501 (2024). <https://arxiv.org/pdf/2406.01501>
  - Maps the MB+CX circuit onto SARSA / Q-learning explicitly; the insect navigation base model can support TD updates.
  - **Why it matters:** Bridges the biology and our RL framing — same neurons, same math.

### 2.3 Central complex / path integration

- **Stone et al. (foundational 2017, confirmed 2024-2025 as the working CX model)** — CPU4 + pontine neurons form the home-vector memory loop.
- **"A decentralised neural model explaining optimal integration of navigational strategies in insects"** — *eLife* (2020), confirmed canonical in 2025 reviews. <https://elifesciences.org/articles/54026>
- **"A Neural Model for Insect Steering Applied to Olfaction and Path Integration"** — *Neural Computation* 34(11) (Nov 2022). <https://direct.mit.edu/neco/article/34/11/2205/112953/A-Neural-Model-for-Insect-Steering-Applied-to>
  - Single steering circuit handles both odor plume tracking and path integration — directly relevant to ants combining home-trail following with food-trail gradient climbing.
- **"A Vector-Based Computational Model of Multimodal Insect Learning Walks"** — *Biomimetics* 10(11) 736 (2025). <https://www.mdpi.com/2313-7673/10/11/736>
  - 2025 model integrating MB visual learning, lateral horn olfaction, and CX path integration as a *unified learning-walk system*. Closest published model to the brain we want to ship.
- **"Emergent spatial goals in an integrative model of the insect central complex"** — PMC 10760860 (2024). <https://pmc.ncbi.nlm.nih.gov/articles/PMC10760860/>
- **"Route-centric ant-inspired memories enable panoramic route-following in a car-like robot"** — *Nature Communications* 16 (2025). <https://www.nature.com/articles/s41467-025-62327-3>
  - One-shot panoramic route learning. Useful if we ever expose visual sensing on the surface layer.

### 2.4 Connectome-driven whole-brain models (Drosophila, transferable)

- **FlyWire connectome (Dorkenwald, Matsliah, Sterling et al.)** — *Nature* 634 (Oct 2024). 139,255 neurons, 50M synapses, fully released. <https://flywire.ai/>
- **"A Drosophila computational brain model reveals sensorimotor processing"** — *Nature* (Aug 2024). <https://www.nature.com/articles/s41586-024-07763-9>
  - Full-brain simulation that *predicts neural responses to stimuli* with no free parameters beyond connectivity — runs on a laptop.
- **"Whole-Brain Connectomic Graph Model Enables Whole-Body Locomotion Control in Fruit Fly"** — arXiv 2602.17997 (early 2026). <https://arxiv.org/html/2602.17997>
  - **FlyGM**: graph-network whose topology is the FlyWire wiring diagram, replacing hand-crafted policy networks. *Body controlled by graph structured exactly like a brain.*
  - **Why it matters:** This is the proof-of-concept for "policy network = species' literal connectome." No ant connectome exists yet at this resolution, but the methodology transfers — and you can use the Drosophila wiring as a base, then prune/relabel for ants (MB and CX are conserved).
- **"Neuromorphic Simulation of Drosophila Melanogaster Brain Connectome on Loihi 2"** — arXiv 2508.16792 (Aug 2025). <https://arxiv.org/abs/2508.16792>
- **DeepMind virtual fruit fly (Vaxenburg, Hsu, Tassa, Merel, et al.) — Janelia/DeepMind**, *Nature* 2025. <https://www.janelia.org/news/artificial-intelligence-brings-a-virtual-fly-to-life>
  - End-to-end ANN policy on biomechanical fly model. Vision-controlled flight + walking from RL.
- **DeepMind virtual rodent — Aldarondo et al., *Nature* 2024.** <https://pmc.ncbi.nlm.nih.gov/articles/PMC12080270/>
  - Virtual rodent ANN policy *predicts the structure of neural activity across behaviors* — strong hint that RL-trained policies discover internal representations matching biology.

### 2.5 Per-species cognition

- **Lasius niger:** Czaczkes et al. 2024 — "Lasius niger deposit more pheromone close to food sources... but do not attempt to update erroneous trails." <https://www.researchgate.net/publication/383595389> — quantitative parameters directly usable for our deposition rules.
- **Pogonomyrmex (harvester ants):** Encounter-rate task allocation (Gordon — foundational, still cited in 2025 robotics work). 2024 finding (Tschinkel) on vertical division of labor in *P. badius* nests — older workers carry items downward, foragers deposit shallow. <https://link.springer.com/article/10.1007/s00040-024-01014-w>
- **Formica rufa:** Lesion experiments showing CX critical for innate↔learned navigation switching (cited in Collett 2025 review). Treadmill/VR systems now established for *F. rufa*.
- **Camponotus (carpenter ants):** Mushroom bodies contain ~260,000 Kenyon cells — high cognitive bandwidth. Topochemical learning, sucrose-concentration nutritional decision-making at distance. Older but solid baseline literature.
- **Tetramorium / Tapinoma:** Specific trail pheromone chemistry (DMP/EDMP for *T. caespitum*, methyl 6-methylsalicylate for *T. impurum*). Argentine-ant trail formation stochastic model — *Swarm Intelligence* (Apr 2024). <https://link.springer.com/article/10.1007/s11721-024-00237-8>
- **Aphaenogaster / slave-makers / Formica fusca:** No 2024–2026 cognition-modeling papers found. Most recent primary research on raid behavior is 2014. **Gap — see §10.**

---

## 3. MARL for Ant / Insect Collective Behavior

### 3.1 Stigmergic MARL — the directly-applicable core

- **(S-MADRL) "Deep Reinforcement Learning for Multi-Agent Coordination"** — arXiv 2510.03592 (Oct 2025). <https://arxiv.org/abs/2510.03592>
  - **Stigmergic Multi-Agent Deep Reinforcement Learning** framework. Virtual pheromones model local + social interactions. Achieves decentralized emergent coordination *without explicit communication*. Up to 8 agents self-organize into asymmetric workload distribution that reduces congestion.
  - **Why it matters:** Closest published architecture to what we want. Pheromone field is in the policy's observation space, deposit rate is in its action space, no explicit messaging.
  - **Status:** *Directly applicable, top-3 read.*
- **"From Pheromones to Policies: RL for Engineered Biological Swarms"** — arXiv 2509.20095 (Sep 2025). <https://arxiv.org/html/2509.20095>
  - **Establishes a theoretical equivalence between pheromone-mediated aggregation and reinforcement learning** — stigmergic signals function as distributed reward mechanisms.
  - **Why it matters:** Theoretical justification for treating pheromone deposition as an action shaping the long-horizon reward landscape — not just a communication primitive.
- **"Emergent Collective Memory in Decentralized Multi-Agent AI Systems"** — arXiv 2512.10166 (Dec 2025). <https://arxiv.org/abs/2512.10166>
  - Phase-transition result: stigmergic coordination *dominates above critical agent density ~0.20*; below that, individual memory wins. At our 10k-ant target on 512×512 (density ~0.04), this predicts we will need *both* a per-ant memory state *and* the pheromone field. Quantitative target.
  - **Why it matters:** Tells us we cannot rely on stigmergy alone at our planned density — confirms keeping per-ant hidden state in addition to the field.
- **PILOC** — arXiv 2507.07376 (Jul 2025). <https://arxiv.org/abs/2507.07376>
  - **Pheromone Inverse Guidance**: pheromone embedded in observation space *and inverted* to push agents to *less-visited* regions. Beats IPPO/MASAC/QMIX (95.6% vs 87.2/74.0/58.4%) on dynamic search tasks.
  - **Why it matters:** Directly applicable to scout / explorer caste. We could implement an "anti-pheromone" channel for exploration drive.
- **PooL — Pheromone-inspired Communication for Large-Scale MARL** — arXiv 2202.09722 (ICANN 2022, still cited 2025). <https://arxiv.org/abs/2202.09722>
- **Stigmergic Independent Reinforcement Learning** — arXiv 1911.12504 (still referenced as baseline). <https://arxiv.org/pdf/1911.12504>
- **Atanasov & Mordatch — "Scalable, Decentralized MARL Inspired by Stigmergy and Ant Colonies"** — arXiv 2105.03546 (foundational). <https://arxiv.org/abs/2105.03546>

### 3.2 Ant-specific MARL

- **"Ant-inspired Walling Strategies for Scalable Swarm Separation: RL with FSMs"** — arXiv 2510.22524 (Oct 2025). <https://arxiv.org/html/2510.22524v1>
  - Army-ant inspired wall formation; RL on top of finite-state-machines (matches our existing AntState FSM). Heterogeneous swarms.
  - **Why it matters:** A worked example of *layering RL on top of FSMs* — almost exactly our refactor path: keep the existing AntState enum, learn the transition policy.
- **"Signaling and Social Learning in Swarms of Robots"** — arXiv 2411.11616 (Nov 2024). <https://arxiv.org/pdf/2411.11616>
  - Decentralized simultaneous learn-and-execute. Communication strategies *evolve* from optimization, not designed.

### 3.3 Cooperative MARL benchmarks / canonical methods

- **MAPPO** (Yu et al., 2021/2022, still SOTA reference).
- **QMIX** (Rashid et al.) — value decomposition with hypernetwork mixing — *itself uses hypernetworks* for monotonic factorization.
- **VDN** (Sunehag et al.) — additive value decomposition.
- **PyMARL2** (Hu et al.) — fine-tuned MARL implementations achieving 100% win on most SMAC scenarios. <https://github.com/hijkzzz/pymarl2>
- **Recent survey of cooperative MARL** — arXiv 2503.13415 (Mar 2025). <https://arxiv.org/html/2503.13415v1>

### 3.4 Multi-objective / Pareto

- No dominant 2025 paper found on explicit Pareto-front MARL for colonies. The standard practice is reward shaping with weighted sums; QD methods (§5) are the principled alternative.

---

## 4. Active Inference / Free Energy for Colonies

- **Friedman, Tschantz, Ramstead, Friston, Constant — "Active Inferants: An Active Inference Framework for Ant Colony Behavior"** — *Frontiers in Behavioral Neuroscience* (2021). <https://www.frontiersin.org/journals/behavioral-neuroscience/articles/10.3389/fnbeh.2021.647732/full> | code: <https://github.com/ActiveInferenceInstitute/ActiveInferAnts>
  - Foundational AI-for-ants paper. Generative model + variational free energy minimization at colony level. *Code is public.*
  - **Why it matters:** If we want to spike a parallel "explainable colony brain" against the RL approach, this is the off-the-shelf comparator.
- **DR-FREE — "Distributionally robust free energy principle for decision-making"** — *Nature Communications* (2025). <https://www.nature.com/articles/s41467-025-67348-6>
  - Wires distributional robustness into FEP-based decision agents.
- **"Active Inference-Driven World Modeling for Adaptive UAV Swarm Trajectory Design"** — arXiv 2601.12939 (Jan 2026). <https://arxiv.org/html/2601.12939>
- **"Flying by Inference: Active Inference World Models for Adaptive UAV Swarms"** — arXiv 2604.27935 (Apr 2026). <https://arxiv.org/html/2604.27935>
  - Two recent applications of AI to swarm trajectory planning. Same machinery transfers to ant colony foraging vs defense allocation.

**Honest assessment:** No paper between Friedman 2021 and now has produced a *better* AI model of ant colonies. The framework is theoretically attractive but compute-heavy and harder to ship than RL. **Recommend: keep on radar, don't bet on it as primary architecture.**

---

## 5. Quality-Diversity for Behavioral Repertoires

- **Pierrot et al. — "Evolving Populations of Diverse RL Agents with MAP-Elites"** — arXiv 2303.12803 (still canonical). <https://arxiv.org/abs/2303.12803>
- **"Dominated Novelty Search: Rethinking Local Competition in Quality-Diversity"** — arXiv 2502.00593 (Feb 2025). <https://arxiv.org/html/2502.00593v1> | GECCO 2025. <https://dl.acm.org/doi/pdf/10.1145/3712256.3726310>
  - Reframes QD's local-competition step. Strong empirical gains in 2025.
  - **Why it matters:** Replaces our hand-designed "heuristic, defender, aggressor, economist, breeder, forager, conservative" archetypes with a *machine-discovered* archive of diverse colony strategies. This is the principled fix to the BC ceiling.
  - **Status:** *Read.*
- **PGA-MAP-Elites** (Nilsson & Cully, GECCO 2021) and **DCG-MAP-Elites** — gradient-assisted QD; standard in 2025.
- **QD papers index — Mouret/Cully maintained list:** <https://quality-diversity.github.io/papers.html>
- **"Empirical analysis of PGA-MAP-Elites for Neuroevolution in Uncertain Domains"** — ACM TELO (2023, baseline reference for 2025 work). <https://dl.acm.org/doi/10.1145/3577203>

**The QD-replacement-for-archetypes story:** Define a 2D or 3D feature space for colony behavior (e.g. forager-fraction, aggression-vs-defense, expansion-rate). Run MAP-Elites with PGA over a colony-level reward. Output: a literal grid of colony strategies, each genuinely high-performing in its niche. Tournament play those instead of the hand-designed seven.

---

## 6. Foundation Models for Animal/Insect Behavior

- **AmadeusGPT (Ye et al., EPFL) — NeurIPS 2024 / used through 2025.** Natural-language querying of animal behavior video. Not directly applicable but indicates the ecology community is moving toward foundation-model interfaces.
- **DeepMind virtual rodent** (Aldarondo et al., *Nature* 2024) — see §2.4. ANN policy → predicts neural activity. <https://pmc.ncbi.nlm.nih.gov/articles/PMC12080270/>
- **DeepMind virtual fly** (Tassa, Merel, Janelia, *Nature* 2025) — see §2.4. <https://www.janelia.org/news/artificial-intelligence-brings-a-virtual-fly-to-life>
- **"Systematic Review of AI Use in Behavioral Analysis of Invertebrate and Larval Model Organisms"** — bioRxiv 2025.10.16.682789 (Oct 2025). <https://www.biorxiv.org/content/10.1101/2025.10.16.682789v1>
  - 2 papers in 2015 → 97 by mid-2025. CNNs / DeepLabCut / YOLO dominate. *Cross-species behavioral transfer is just emerging.*
- **"Flexible inference for animal learning rules using neural networks"** — Liu et al., NeurIPS 2025 (Princeton Pillow lab). <https://pillowlab.princeton.edu/pubs/Liu2025neurips_learningrules.pdf>
  - RNNs + LLM-suggested rule structures used to *infer* what learning rule an animal is using from behavior. Reverse direction from us, but tells us our RL choice can be validated against real ant data later.

**No bug-policy LLM exists.** No "Ant-GPT". This is genuine open territory and a clear gap.

---

## 7. Stigmergy Modeling — State of the Art

- **"Automatic design of stigmergy-based behaviours for robot swarms"** — *Communications Engineering* (Nature, 2024). <https://www.nature.com/articles/s44172-024-00175-7>
- **"Brain-Inspired Stigmergy Learning"** — Xing et al., IEEE TETCI 2019, still referenced as the bridge between synaptic dynamics and pheromone fields. <https://ieeexplore.ieee.org/document/8698894/>
- **Boi & Trianni — "A single-pheromone model accounts for empirical patterns of ant colony foraging previously modeled using two pheromones"** — *Biosystems* (2023). <https://www.sciencedirect.com/science/article/pii/S1389041723000207>
  - Important: argues ONE pheromone field suffices for foraging patterns — challenges the standard food/home double-pheromone setup. Worth knowing as a Chesterton's-fence check on our four-channel grid.
- **Boi et al. — "A stochastic model of ant trail formation and maintenance in static and dynamic environments"** — *Swarm Intelligence* 2024. <https://link.springer.com/article/10.1007/s11721-024-00237-8>
- **Theraulaz, Bonabeau (foundational)** — "Ant algorithms and stigmergy" — still THE canonical reference, every modern paper cites it.
- **"Spatiotemporal organization of ant foraging from a complex systems perspective"** — *Scientific Reports* 14 (2024). <https://www.nature.com/articles/s41598-024-63307-1>
- **"Ledger-State Stigmergy: A Formal Framework for Indirect Coordination Grounded in Distributed Ledger State"** — arXiv 2604.03997 (Apr 2026). <https://arxiv.org/abs/2604.03997>
  - Formalizes stigmergy abstractly. Ledger application is unrelated to us, but the formal framework (signal lifetime, write semantics, read semantics) is a useful lens for designing our pheromone API.
- **"The Coordinate System Problem in Persistent Structural Memory for Neural Architectures" — DPPN**: arXiv 2603.22858 (Mar 2026). <https://arxiv.org/abs/2603.22858>
  - **Dual-View Pheromone Pathway Network** — sparse attention routed through a persistent pheromone field over latent slot transitions. Identifies three obstacles: *pheromone saturation, surface-structure entanglement, coordinate incompatibility.*
  - **Why it matters:** First neural-architecture paper to import pheromone-field semantics directly into the model's internal memory. Useful design language even though task is different from ours.

**Information-theoretic stigmergy bounds:** No single canonical bound paper found. Closest is the phase-transition density result in Khushiyant 2025 (§3.1) — informal but useful target.

---

## 8. Per-Species Cognition Papers (2023–2026 only)

| Species (sim) | 2023–2026 paper | Topic | Use |
|---|---|---|---|
| *Lasius niger* | Czaczkes et al. 2024 | Pheromone deposition gradients | Quantitative sim params |
| *Lasius niger* | Boi & Trianni 2023 | Single vs double pheromone | Architecture critique |
| *Camponotus pennsylvanicus* | (no 2023+ neural paper found) | — | Use older Traniello/Hölldobler refs |
| *Formica rufa* | Collett, Graham, Heinze 2025 (review covers wood ants extensively) | CX/MB navigation | Brain module spec |
| *Formica rufa* | "The Neuro-ethology of Collective Decision-Making in Ant Colonies: A Case Study on Formica Rufa" — IJMRP 2024. <https://www.chandigarhphilosophers.com/index.php/ijmrp/article/view/303> | Collective cognition | Direct case study |
| *Pogonomyrmex badius* | Tschinkel 2024 (vertical labor division). <https://link.springer.com/article/10.1007/s00040-024-01014-w> | Underground task allocation | Nest-layer logic |
| *Tetramorium / Tapinoma* | Boi et al. 2024 (Argentine ant trail model) | Trail formation | Pheromone model |
| *Aphaenogaster rudis* | None recent | — | Gap |
| *Formica fusca* (host) | None recent (latest: 2014, Pamminger) | Slave defense | Gap |
| All ants (review) | Frank & Kronauer 2024 (Annu. Rev. Neurosci.) | Social-behavior neuroscience | Foundation |
| All ants (carry) | "Ants engaged in cooperative food transport show anticipatory and nest-oriented clearing of obstacles" — *Frontiers Behav Neurosci* 2025. <https://www.frontiersin.org/journals/behavioral-neuroscience/articles/10.3389/fnbeh.2025.1533372/full> | Goal-directed collective cognition | Emergent-behavior validation target |
| All ants (cognition) | "The economic strategies of superorganisms" — bioRxiv 2025.02.21.639603 (Feb 2025). <https://www.biorxiv.org/content/10.1101/2025.02.21.639603v2.full.pdf> | Colony-economy theory | Reward design |

---

## 9. Recommendations — Ranked by (Impact × Feasibility)

### R1. Replace BC bootstrap with Stigmergic CTDE + Hypernetwork actor + League self-play **[HIGHEST]**
- **Architecture:** One colony-level transformer/MLP "brain" → hypernetwork (HyperMARL-style, agent-conditioned on `caste`, `species`, `colony_id`) emits per-ant actor weights. Centralized critic sees full pheromone grid + colony stats. Decentralized actors see only local pheromone window + own state.
- **Training:** League/AlphaStar-style. Seed the league with the existing 7 archetypes as fixed exploiters. Main agent and main exploiters are trained. Drop BC entirely after the seeding stage.
- **Reward:** Colony-level scalar (composite of food, brood, survival, territory). Apply Maynard-Cross intuition: the colony is the agent; per-ant actions are the exploration distribution.
- **Why first:** Single change addresses (a) the BC ceiling, (b) the per-caste / per-species specialization need, (c) the emergent-behavior requirement. All component pieces have 2024–2025 working implementations.
- **Feasibility:** Medium — needs an MARL training loop (currently we don't have one). ~3–6 weeks engineering.

### R2. Swap hand-designed archetypes for a MAP-Elites archive **[HIGH]**
- **Architecture:** PGA-MAP-Elites (or Dominated Novelty Search per arXiv 2502.00593) over a 2D feature space: e.g. (forager_fraction, aggression_score). Each cell = a colony policy.
- **Use:** League opponents drawn from the QD archive instead of the seven manual archetypes.
- **Why second:** Removes the human-designer ceiling on strategy diversity. The BC plateau is fundamentally a teacher-pool diversity problem, and QD is the principled fix.
- **Feasibility:** Medium-Easy — QD wraps any RL inner loop. Pick after R1's training loop exists.

### R3. Replace abstract MLP with MB+CX-shaped modules **[MEDIUM-HIGH]**
- **Architecture:** Instead of 17→64→64→6 MLP, structure the policy as: `[Sensory inputs] → MB module (sparse Kenyon-cell-like layer, ~200 neurons, food/home/alarm/scent labelled channels per Sci. Adv. 2024) → CX module (ring-attractor heading + path-integration accumulator) → action head.`
- **Why:** Biologically grounded, smaller, more interpretable, and the 2025 literature shows this circuit is *exactly* the substrate for the Q-learning-style updates we want.
- **Sources:** Collett 2025; Le Moël 2024; Buckley 2024; MDPI Biomimetics 2025.
- **Feasibility:** Medium. Easier if we treat MB as "sparse-coded random projection + Hebbian readout" and CX as "small RNN with sinusoidal-weight init."

### R4. Add an anti-pheromone (PILOC-style) exploration channel **[MEDIUM]**
- Single new channel in the grid that decays slower and is *minimized* by scout caste. Solves the "no scouts because trails dominate" failure mode common in stigmergic systems.
- **Source:** PILOC arXiv 2507.07376.
- **Feasibility:** Easy. ~1 day on the existing grid.

### R5. Build an Active-Inference parallel comparator (low priority, longer-term) **[LOW-MEDIUM]**
- Fork ActiveInferAnts code, scaffold a colony with the same observation/action interface, run side-by-side. Useful as an explainability comparator and a check on whether our RL policy is doing free-energy-like behavior implicitly.
- **Source:** Friedman et al. 2021 + DR-FREE 2025.
- **Feasibility:** Medium-High effort, low-medium payoff. Defer.

---

## 10. Gap Analysis — What's NOT in the Literature

1. **No ant connectome** at FlyWire resolution. Drosophila MB has ~2k Kenyon cells; *Camponotus* has ~260k. Best we can do is structurally analogous models, not literal connectome simulation. **Action: use Drosophila connectome as base, scale & relabel.**
2. **No "Ant-GPT" foundation model.** Behavioral data exists (DeepLabCut tracks of every species), but no one has trained a cross-species behavior transformer. Open opportunity.
3. **No 2024–2026 cognition papers on slave-making / dulosis.** Our *Formica fusca* host + raid behavior will need to be hand-designed against pre-2015 behavioral literature.
4. **No 2024–2026 paper on *Aphaenogaster rudis* cognition.** Same gap.
5. **No published Pareto-front MARL for colony objectives.** Multi-objective reward shaping remains a hand-tuning problem; QD is the workaround.
6. **No quantitative information-theoretic bound on stigmergy bandwidth.** Phase-transition results (Khushiyant 2025) are the closest substitute. We could publish here ourselves — stigmergy bandwidth as a function of grid resolution × evaporation × diffusion is computable.
7. **No biologically validated "colony economy" reward function.** "Economic strategies of superorganisms" (bioRxiv 2025) is theoretical; nobody has back-fit a reward to observed colony lifetime fitness.
8. **No comparison of RL-trained colonies vs real-ant behavioral statistics at scale.** Cleanest win for academic credibility *and* sim accuracy: train, then validate against published trail-formation curves (Boi 2024, Czaczkes 2024).

---

## 11. "Read First" Priority List (top 5)

1. **Soma et al. — "The Hive Mind is a Single RL Agent"** — arXiv 2410.17517. *Reframes the entire problem.*
2. **Collett, Graham & Heinze — "The neuroethology of ant navigation"** — *Current Biology* Feb 2025. *The biological blueprint.*
3. **S-MADRL — "Deep RL for Multi-Agent Coordination"** — arXiv 2510.03592. *The closest off-the-shelf MARL architecture.*
4. **Tessera et al. — "HyperMARL"** — arXiv 2412.04233. *The mechanism for per-ant specialization in one colony brain.*
5. **Frank & Kronauer — "The Budding Neuroscience of Ant Social Behavior"** — Annu. Rev. Neurosci. 2024. *The social-behavior neural circuits.*

**Honorable mentions worth a second pass:** "From Pheromones to Policies" (arXiv 2509.20095), "Emergent Collective Memory" (arXiv 2512.10166), Le Moël et al. "RL as a Robotics-Inspired Framework for Insect Navigation" (arXiv 2406.01501), Vaxenburg/Tassa virtual fly (Janelia/DeepMind 2025), Pierrot "Evolving Populations of Diverse RL Agents with MAP-Elites" (arXiv 2303.12803).

---

## 12. Does anything INVALIDATE the current BC approach?

**Yes — explicitly.** Three independent results:

1. **Soma et al. 2024/2025**: shows the colony-level cognition equals a *single* online RL agent. BC over a balanced expert pool of *individual policies* is the wrong reduction; you should be doing online RL on the *colony* as the unit.
2. **Ren et al., NeurIPS 2025 — "Interactive and Hybrid Imitation Learning: Provably Beating Behavior Cloning."** <https://neurips.cc/virtual/2025/loc/san-diego/poster/115694> | <https://arxiv.org/html/2412.07057v3>
   Provides formal lower bounds on BC and matching upper bounds for interactive (DAgger-family) and hybrid algorithms — *proves* you cannot beat the expert with vanilla BC under standard assumptions.
3. **Liu et al., R2BC (Multi-Agent IL from Single-Agent Demonstrations)**: shows single-agent demonstrations are insufficient for multi-agent imitation in general.

**Bottom line:** The 45.7% plateau is exactly the predicted ceiling. Drop BC as the primary loss; use it only to seed a self-play league (R1 above).

---

## Appendix A — Full Citation Index (alphabetical)

(Selected — full URLs above in body)

- Active Inferants — Frontiers Behav. Neurosci. 2021
- Aldarondo et al. — Virtual rodent, *Nature* 2024
- Ant-inspired Walling Strategies — arXiv 2510.22524
- Atanasov & Mordatch — Stigmergic decentralized MARL — arXiv 2105.03546
- Boi et al. — Argentine ant trail stochastic model — *Swarm Intelligence* 2024
- Boi & Trianni — Single-pheromone model — *Biosystems* 2023
- Buckley/Webb/Nowotny — SNN MB visual nav — *Front. Physiol.* 2024
- Collett, Graham, Heinze — Ant navigation neuroethology — *Curr. Biol.* 2025
- Czaczkes et al. — Lasius niger pheromone deposition — 2024
- DeepMind / Janelia virtual fly — *Nature* 2025
- Dominated Novelty Search — arXiv 2502.00593
- DPPN — arXiv 2603.22858
- DR-FREE — *Nature Comm.* 2025
- Emergent Collective Memory — arXiv 2512.10166
- Flexible inference for animal learning rules — Liu et al., NeurIPS 2025
- FlyGM connectomic graph model — arXiv 2602.17997
- FlyWire connectome — *Nature* 2024
- Frank & Kronauer — Annu. Rev. Neurosci. 2024
- From Pheromones to Policies — arXiv 2509.20095
- Hybrid neural networks in MB — *Sci. Adv.* 2024
- HyperMARL — arXiv 2412.04233
- Hypernetwork composition design — arXiv 2502.12605
- Interactive and Hybrid IL — arXiv 2412.07057, NeurIPS 2025
- Le Moël et al. — RL framework for insect nav — arXiv 2406.01501
- Ledger-State Stigmergy — arXiv 2604.03997
- MAGPO / OPT-QMIX — 2025 CTDE advances
- MAPPO — Yu et al. 2021/2022
- MRDG — arXiv 2506.16718
- MB route memory — Ardin et al. PMC 4750948
- Multimodal insect learning walks — *Biomimetics* 2025 (MDPI 10.11.736)
- Neuromorphic Drosophila on Loihi 2 — arXiv 2508.16792
- PGA-MAP-Elites — Nilsson & Cully GECCO 2021
- Pierrot — MAP-Elites for diverse RL agents — arXiv 2303.12803
- PILOC — arXiv 2507.07376
- PooL — arXiv 2202.09722
- R2BC — Multi-Agent IL from Single-Agent Demos
- Route-centric ant memories for car robot — *Nature Comm.* 2025
- Signaling and Social Learning — arXiv 2411.11616
- Soma et al. — Hive Mind is a Single RL Agent — arXiv 2410.17517
- Spatiotemporal foraging organization — *Sci. Rep.* 2024
- S-MADRL — arXiv 2510.03592
- STACCA — arXiv 2511.13103
- Stigmergic Independent RL — arXiv 1911.12504
- Stigmergy automatic design — *Comm. Eng.* 2024
- Superorganism economic strategies — bioRxiv 2025.02.21.639603
- Systematic Review AI for Invertebrate Behavior — bioRxiv 2025.10.16.682789
- TLeague — arXiv 2011.12895
- Tschinkel — *P. badius* vertical labor — *Insectes Soc.* 2024
- UAV Active Inference world models — arXiv 2601.12939, 2604.27935

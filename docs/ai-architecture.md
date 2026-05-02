# AI Architecture — Narrator, Blackboard, AI vs AI

Parking the AI feature plans here so they survive session boundaries. Two related but distinct ideas, building on each other in phases.

**Status:** design only. Nothing implemented yet. Currently behind: economy stabilization (in progress), dig system (queued), Phase 8 full game mode (queued). Don't start building this until those substrates are solid.

---

## Why this is parked, not built

A sophisticated AI feature on top of a colony that dies the first winter is atmospheric narration for a corpse. Wait until:

1. 7-species 25y validation sweep is fully green
2. Dig system from `docs/digging-design.md` ships (so keeper mode has gameplay beyond "watch")
3. Phase 8 (full grid game mode, 12×16 squares) ships

Estimated 2-3 months from now. This doc is so we can move fast when we get there.

---

## Phase 9 — The progressive AI ladder

Each phase is independently shippable and adds value without requiring later phases.

### Phase 9.0 — Narrator (charm, low risk)

Pure flavor layer. Generates per-colony status feed at major events:

> *Day 287: first snow. Queen Marigold retreated to the deep chamber. The brood is huddled together — three pupae are nearly ready.*

> *Day 401: a major worker emerged today, the colony's first soldier. Sigrid by name. The workers are bringing her sugar water in tribute.*

**Triggers:** milestone events, per-day wakeups, on-request. Async — doesn't touch sim mechanics. Zero correctness risk.

**Output:** scrolling text panel in the UI; optional speech-bubble pop-ups over individual ants.

**Storage:** rolling per-colony chronicle in `assets/saves/<colony>/chronicle.md`. Auto-numbered, timestamped.

**Effort:** 1 week.

**ROI:** very high. Distinguishes this from every other colony sim. Queen and notable workers get procedural names + biographies. Replays become readable stories.

### Phase 9.1 — Blackboard (real AI, no LLM yet)

Replaces the existing scripted red-team AI with a transparent rule-based blackboard architecture. **No LLM required.** The win is that the AI's reasoning is human-readable in a side panel, and the architecture is modular for later LLM integration.

```rust
// crates/antcolony-sim/src/ai/blackboard.rs
pub struct Blackboard {
    pub colony_id: ColonyId,
    pub facts: Vec<Fact>,
    pub commitments: Vec<Directive>,
}

pub enum Fact {
    Observation { what: ObservationKind, tick: u64, source: KnowledgeSource, confidence: f32 },
    Threat      { entity: ThreatRef, severity: f32, expires_tick: u64 },
    Goal        { directive: Directive, priority: f32, by: KnowledgeSource },
    Hypothesis  { proposition: String, support: Vec<FactRef> },
}

pub enum Directive {
    AdjustCasteRatio(CasteRatio),
    AdjustBehaviorWeights(BehaviorWeights),
    PlaceBeacon { kind: BeaconKind, location: BeaconTarget },
    Retreat { from: ModuleId, to: ModuleId },
    Excavate { target: CellPos },
}

pub trait KnowledgeSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn observe(&self, sim: &Simulation, blackboard: &Blackboard) -> Vec<Contribution>;
    fn cadence(&self) -> Cadence;  // per-tick / per-N-ticks / on-event / on-request
}
```

**Rule-based KS (Phase 9.1 ships these):**
- `Strategist` — colony state + opponent intel → high-level objectives ("expand south", "build soldier ratio", "prepare for winter")
- `Forager` — pheromone/food state → recommended forager allocation
- `Combat` — enemy proximity → defensive/offensive postures
- `Architect` — population vs chamber capacity → dig priorities
- `Diplomat` (versus mode only) — opponent behavior pattern → threat assessment

**Control / arbiter:** per-tick decides which contributions become commitments. Initially priority queue by confidence × priority. Replaceable with LLM later.

**UI:** side panel showing the colony's current Goals + Threats + recent Observations, attributable to each KS.

**Effort:** 2-3 sessions.

**ROI:** real upgrade over scripted red AI. Player can SEE why the AI is acting. Modular — adding a `Disease` or `Weather` KS later is easy.

### Phase 9.2 — Obsidian memory backend

Each colony gets a vault of markdown notes that persist across sessions. The key novel idea: the AI's memory is **introspectable** — you can pop the vault open in Obsidian and read what the AI thinks.

**Vault layout:**

```
saves/<colony_id>/vault/
├── _index.md                      # entry point with backlinks summary
├── queen.md                       # queen biography, lay rate history, personality
├── workers/
│   ├── sigrid.md                  # notable workers (named via narrator)
│   └── ...
├── opponents/
│   ├── colony_zara.md             # what we know about each rival
│   └── lessons/zara-flank-east.md # post-mortem of past matches
├── decisions/
│   ├── 2026-day-287-retreat.md    # why we retreated, outcome, blame KS
│   └── ...
├── enemies/
│   ├── spider-pit-east.md         # ongoing environmental threats
│   └── ...
├── chambers/
│   ├── queen.md
│   ├── brood-1.md
│   └── ...
└── milestones.md                  # auto-appended highlights
```

**Note format:** YAML frontmatter for queryable metadata; markdown body for narrative.

```markdown
---
title: "Day 287 — Winter retreat"
type: decision
tick: 12_398_400
in_game_day: 287
tags: [retreat, winter, combat-defensive]
confidence: 0.7
by_ks: Strategist
backlinks: [[winter-2026]] [[chambers/queen]]
---

The first frost arrived three days earlier than last year. The Strategist
KS recommended pulling all workers home; the Forager KS objected (food
stores were only at 60% target). I sided with Strategist — last year's
Day 31 lesson [[lessons/day-31-frostbite]] was costly.

[[Outcome will be filled by post-mortem]]
```

**Crate:** new `colony-vault` workspace member. Atomic file writes, frontmatter parsing, backlink graph maintenance, query helpers (`find_notes_with_tag`, `notes_referencing(entity)`, etc.).

**Read-only at first:** AI just journals. No LLM consuming notes yet. Player can already get value reading the journal.

**Effort:** 1-2 sessions.

**ROI:** even read-only is a feature — players keep journals of their colonies. Vault becomes shareable / git-trackable. Sets up Phase 9.3.

### Phase 9.3 — LLM sidecar (the actual sidecar question)

LLM gets wired into specific KS, not the whole architecture. Most KS stay rule-based for speed; the LLM-backed ones run on a longer cadence and produce richer reasoning.

**Model target:** Qwen 2.5 1.5B Instruct. ~2GB int8 in VRAM. Permissive license. Strong structured-output behavior. Llama 3.2 1B and Gemma 2 2B are alternatives.

**Inference engine choice:**

| Option | Pros | Cons |
|---|---|---|
| **candle-rs** in-process | Self-contained binary, no extra install for player, Rust-native | Pre-flight quantization, smaller ecosystem |
| **ollama localhost** | Easier to develop against, swap models freely, hot-reload | Player has to install ollama; second process |
| **mistral.rs** in-process | Good throughput, tokio-async | Heavier dep tree |

Recommendation: **ollama for development, candle for ship**. Same prompt + JSON schema works against both.

**LLM-backed KS:**

- `Strategist` — once per minute (real-time) or once per in-game week:
  - Input: blackboard snapshot + colony state summary + opponent intel + relevant Obsidian notes (via tag/backlink lookup)
  - Output: structured JSON contribution to blackboard (new Goals + revised Threats + a Hypothesis or two)
  - Plus: a markdown journal entry for the vault

- `Memorist` — on-demand:
  - Input: a query like "what's our history with Colony Zara?"
  - Output: synthesized summary pulling from `opponents/colony_zara.md` + relevant `decisions/*` + `lessons/*`
  - Used by Strategist to ground its reasoning in past events

- `Narrator` (Phase 9.0 retro-fitted) — uses the same sidecar

**Schema enforcement:** every LLM call uses constrained JSON output (grammars supported by both candle-rs's `LogitsProcessor` and ollama's structured output). Validates against rust types. Hallucinated entity references fall through to no-op.

**Cadence budget:** assume 100-300ms per Strategist call on a 3070 Ti at int8. Once per minute = 0.5% CPU/GPU overhead. Scales to AI-vs-AI mode (2 colonies × 1/min = still <1%).

**Effort:** 3-4 sessions.

**ROI:** real character emerges. AI genuinely thinks about its situation. Player-readable reasoning chain.

### Phase 9.4 — AI vs AI mode

Two AI colonies, asymmetric Obsidian vaults, no human player. Spectator camera + replay.

**Setup:**
- Player picks two AI personalities ("Aggressive Defender", "Patient Builder", "Opportunistic Raider", etc.) = different KS configs + starter Obsidian seed notes
- Match runs at chosen time scale; player watches
- Replay mode reads both vaults to reconstruct match narrative

**Asymmetric vaults:**
- Each AI has its own vault: `saves/match_<id>/vault_a/` and `vault_b/`
- Different starter notes encoding personality biases
- Persistent across matches — the same AI personality remembers prior opponents

**Personality presets** (stored as starter vaults):

```
ai_personalities/aggressive_defender/
├── queen.md          (biased toward soldier ratios, aggression)
├── doctrine.md       (tactical preferences)
└── starter_lessons.md (canned "experiences")

ai_personalities/patient_builder/
├── queen.md          (biased toward economy first, military second)
├── ...
```

**Rivalry meta:** after enough matches between the same personalities, each vault accumulates a `rivals/<other-personality>.md` file with observed patterns. Match #20 between Aggressive Defender and Patient Builder is genuinely different from match #1.

**Effort:** 2-3 sessions.

**ROI:** spectator feature. Tournament potential. Streamable / sharable replays.

### Phase 9.5 — Polish + expansion (ongoing)

- Additional KS: Disease, Weather, Diplomat (real for versus), Memorist (Phase 9.3)
- Tournament mode: round-robin AI personalities
- LoRA fine-tunes for AI personalities (per-personality model heads, all sharing a base 1.5B)
- Cross-match meta: AI develops genuine "rivalries" through repeated play

---

## Why this fits ant colonies specifically

Real ant colonies don't have central planning. Intelligence emerges from many specialized sub-minds none of which is the queen. The blackboard architecture mirrors this: the colony's "mind" is the integration of many KS contributions, with no single decision-maker. This is biologically accurate AND player-legible — the side panel literally shows "many minds, one colony."

Compare to a single-LLM-controls-everything design: that's a dictator, not a colony. The blackboard model preserves the emergent-intelligence flavor that makes ant colonies fascinating.

---

## Storage: where do vaults live?

In `assets/saves/<save_id>/vault/` for keeper-mode single-player. In `assets/saves/match_<match_id>/vault_a/` and `vault_b/` for AI vs AI. Vaults are git-trackable (`assets/saves/.gitignore` excludes them by default but we can document the export workflow for players who want to share).

The K4 save/load (`Snapshot` JSON) coexists with the vault — sim state in the snapshot, narrative state in the vault. Loading restores both.

---

## Cross-references

- `docs/biology.md` — real-ant biology that should inform KS rules (food regulation, pheromone communication, alarm response). The AI's KS should be grounded in the same research.
- `docs/digging-design.md` — Architect KS hooks into the dig pipeline once it ships.
- HANDOFF.md — current status of the substrate work that needs to land before this can start.

---

## Cost honesty

Phase 9.0 (narrator only): 1 week. Realistic.

Phase 9.1 (blackboard, no LLM): 2-3 sessions. Realistic — it's just data structures and rule-based KS.

Phase 9.2 (Obsidian writer): 1-2 sessions. Realistic.

Phase 9.3 (LLM wired into KS): 3-4 sessions. **Realistic if we use ollama + 1.5B; could blow up if we go straight to candle-rs in-process.** Recommend ollama path first.

Phase 9.4 (AI vs AI + replay): 2-3 sessions. Realistic.

Phase 9.5+: indefinite — this is the "game system" tier.

**Total to ship 9.0-9.4: ~10-15 working sessions.** Spread over a few months alongside other features.

# AI Brain — Pluggable Per-Colony Decision Policies

**Implementation status: shipped (Phase 9.x extension).** Companion to `docs/ai-architecture.md` (the long-form narrator + blackboard plan). This doc covers what's actually in the code today: the `AiBrain` trait, three concrete impls (heuristic / random / aether-LM), and the train-loop pipeline that turns sim trajectories into a learned policy.

---

## The shape of an AI decision

Every AI-controlled colony has a **brain** that decides, once per outer tick (or every Nth tick — current cadence is 5), what the colony's caste mix and behavior weights should be. The brain reads a 17-feature snapshot of colony state and returns a 6-scalar decision plus an optional research choice.

```
ColonyAiState (17 features)  ─►  AiBrain.decide()  ─►  AiDecision (6 scalars + research)
```

The decision is then auto-renormalized and applied to the colony's `caste_ratio` + `behavior_weights` via `Simulation::apply_ai_decision`.

### Why this shape, not direct unit control

Real ants don't get told "you, go forage." They self-recruit based on local task demand. The AI brain models the *colony's hormonal/pheromonal balance* — caste ratios + behavior weights — and the per-ant decision system already in `decide_next_state` does the rest. This means:

- Brains output 6 floats per tick, not N×ants commands → cheap.
- The same brain trait works for any colony size from 50 to 500,000 ants.
- Behavior emergence is preserved — the brain doesn't override the ACO trail-following or the FSM transitions.

---

## The trait

```rust
pub trait AiBrain: Send + Sync {
    fn name(&self) -> &str;
    fn decide(&mut self, state: &ColonyAiState) -> AiDecision;
}
```

Defined in `crates/antcolony-sim/src/ai/brain.rs`. Three concrete impls ship.

### `HeuristicBrain` — baseline

Reactive rules, no model. Refactor of the legacy `red_ai_tick`:

- Combat losses last tick → escalate soldier ratio (capped 0.5).
- Food stored below threshold → forage-everything mode.

Deterministic. Not strong. Useful as the matchup-bench baseline and as a teacher for behavior cloning.

### `RandomBrain` — noise floor

Uniformly random valid decisions, seeded for reproducibility. Not a strong opponent — by design — it's the matchup-bench's "noise floor" so we can measure how much any other brain is actually beating chance.

### `AetherLmBrain` — learned policy

Wraps `J:/aether/target/release/aether-infer.exe`. Per `decide()`:

1. Serialize `ColonyAiState` to a single-line text prompt:
   ```
   state food=100.0 inflow=0.5 workers=30 soldiers=5 ... action=
   ```
2. Spawn `aether-infer.exe --ckpt <path> --prompt "<above>" --max-new 40` with cwd set to aether root and the ckpt path made relative (aether refuses absolute paths that "escape cwd").
3. Parse the completion as space-separated `key:value` pairs:
   ```
   w:0.65 s:0.30 b:0.05 f:0.55 d:0.20 n:0.25 r:none
   ```
4. On any failure (exe missing, ckpt missing, non-zero exit, unparseable completion), fall back to a safe-default decision and log a warning. After 3 consecutive failures, the brain stops shelling out for the rest of the run.

The fallback budget protects matchup-bench runs from hanging on a bad checkpoint. The integration is real; what's missing is a **trained checkpoint that knows the wire format** (the README's nano-on-synthetic-text checkpoint does not).

---

## Match-end detection

```rust
pub enum MatchStatus {
    InProgress,
    Won { winner: u8, loser: u8, ended_at_tick: u64 },
    Draw { ended_at_tick: u64 },
}
```

`Simulation::match_status()` computes this in O(colonies + ants). A colony "loses" when it has zero queens AND zero adult ants. Both colonies dying on the same tick = `Draw`. The AI-vs-AI bench harness ticks until status transitions out of `InProgress` or hits a tick budget.

**Current limitation:** at default sim balance, decisive matches are rare in <2000 ticks. The matchup bench supplements end-state scoring with a graded *worker-share-at-end* outcome so trajectories from non-decisive matches still carry training signal.

---

## The train loop

Pipeline (single command via `scripts/ai_training_run.ps1`):

```
matchup_bench --dump-trajectories  →  trajectories.jsonl
                                          │  filter outcome ≥ 0.55
                                          ▼
                                    corpus.txt (one prompt+completion per line)
                                          │  aether-prepare
                                          ▼
                                    prepared (token stream)
                                          │  aether-train --data prepared --out checkpoint
                                          ▼
                                    checkpoint.{weights,meta}
                                          │  matchup_bench --right aether:checkpoint
                                          ▼
                                    eval SUMMARY.md
```

**Why behavior cloning, not RL:** Aether is a small language model trainer (next-token prediction). RL would need an external trainer + a much richer state encoding. Behavior cloning the *winning side* of self-play matches is the pragmatic path with what we have:

1. Run heuristic vs heuristic + heuristic vs random self-play
2. Tag each (state, decision) tuple with the colony's outcome score
3. Filter to "winning" trajectories (outcome ≥ 0.55)
4. Train a tiny LM to predict `decision` given `state`
5. The model learns to imitate decisions that historically correlated with winning

**Limitations of this approach:**
- Cannot exceed the teacher's strategic ceiling (heuristic is the strategy bound)
- High variance — small datasets + small models = overfit risk
- No exploration — model never sees out-of-distribution states it would create itself in deployment

These are the right limitations for a *first cut*. The next iteration goal is to use the trained model itself as a self-play opponent (`aether vs aether`), filter for its winning trajectories, and re-train — DAgger-style iterative imitation that does cross the teacher's ceiling.

---

## Wire format

Every byte tied to the model's tokenizer. Don't change without a checkpoint regeneration.

**Prompt** (one line, ~140 chars):
```
state food=<f1> inflow=<f2> workers=<u> soldiers=<u> breeders=<u> eggs=<u> larvae=<u> pupae=<u> queens=<u> losses=<u> ed=<f1>|inf ew=<u> es=<u> doy=<u> t=<f1> dia=<0|1> day=<0|1> action=
```

`<fN>` = float with N decimal places. Trailing `action=` cues the model to complete.

**Completion** (~40 chars):
```
w:<f2> s:<f2> b:<f2> f:<f2> d:<f2> n:<f2> r:<choice|none>
```

Robust to extra whitespace. Missing fields default to safe values. All values clamped to `[0,1]`.

---

## File map

| File | What it holds |
|---|---|
| `crates/antcolony-sim/src/ai/brain.rs` | trait + 3 impls + state/decision/match-status types + serialization |
| `crates/antcolony-sim/src/ai/blackboard.rs` etc. | (Phase 9.1, separate) Blackboard reasoning + KS arch — not yet wired |
| `crates/antcolony-sim/src/simulation.rs` | `new_ai_vs_ai_with_topology`, `match_status`, `colony_ai_state`, `apply_ai_decision` |
| `crates/antcolony-sim/examples/matchup_bench.rs` | head-to-head bench runner with trajectory dumper |
| `crates/antcolony-sim/tests/ai_vs_ai.rs` | 6 integration tests covering the plumbing |
| `scripts/train_aether_smoke.ps1` | 30-second smoke of the train pipeline |
| `scripts/train_aether_brain.ps1` | full pipeline (50 matches × 600 train steps) |
| `scripts/ai_training_run.ps1` | **multi-checkpoint experiment** — 200 matches, 4 checkpoints, eval each |

---

## What's not built yet

- **Per-colony species** — the matchup bench currently uses one shared SimConfig. Real PvP requires per-colony Phase A trait overrides (recruitment, sting, nocturnal, etc.). Today both colonies share the same biology.
- **In-process aether binding** — subprocess overhead is ~10-50ms per call. For tight per-tick decisions on a large bench, an in-process Rust binding to `aether_runtime` would help. Aether doesn't expose a Rust API yet.
- **DAgger / iterative self-play** — trained model fights heuristic, winning trajectories train V2 model, loop. Not automated.
- **RL** — no replay buffer, no value head, no PPO. Aether is a language model, not an RL trainer.
- **Tech-tree decisions** — `AiDecision.research_choice` exists but no brain currently emits anything but `None`.

These all are unlocked once the bench → train → eval loop reliably produces a model that beats `RandomBrain` (the noise floor) — that's the proof point for the architecture.

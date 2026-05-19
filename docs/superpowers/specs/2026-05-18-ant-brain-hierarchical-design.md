# Hierarchical Ant Brain — Design Spec

**Date:** 2026-05-18
**Status:** Approved (Matt, brainstorming session 2026-05-18)
**Predecessors:** `crates/antcolony-sim/src/ai/brain.rs` (current `MlpBrain` + heuristic brains), `crates/antcolony-trainer/` (existing PPO pipeline), memory `project_ai_ceiling.md` (47.1% Nash plateau), memory `project_mlp_brain_solitaire_ood.md`
**Successor:** Implementation plan (to be written via `superpowers:writing-plans`)
**Target architecture:** approach **A — Compact-Hierarchical** at sizing point **A2 (~95M total params)** as the primary deployment target, with **A3 (~160M)** as the cnc-only research teacher.

---

## Goal

Design a neural policy net for antcolony that:

1. Breaks the **47.1% Nash plateau** the current `MlpBrain v1` (17→64→64→6, ~5K params) sits at on the 7-archetype bench.
2. Fits on an **8 GB consumer GPU at inference time** alongside the running game — so the trained brain ships as part of the game, not just an internal research artifact.
3. **Maxes out the cnc P100s during training** — not by parameter count, but by parallel-env throughput, big-batch PPO, and rich observations. The compact architecture is a feature, not a constraint relaxed away from.
4. Stays **biologically motivated in spirit** — the brain biases the existing ACO pheromone math rather than replacing it. Closer to how real insect cognition works (continuous parameter modulation over fixed neural circuits) than a discrete state-machine override would be. This is the "ant brain" answer to Matt's hope for a "bee brain" — not biomimetic at the neuron level, but biology-faithful at the **mechanism** level.

---

## Non-goals

- **No spiking neural networks, mushroom-body anatomical models, or computational-neuroscience faithfulness.** The biology motivation stops at "brain modulates ACO knobs"; we don't try to simulate Kenyon cells.
- **No replacement of the FSM or pheromone math.** The brain stays in bias-mode (per Matt's explicit choice in brainstorming). Defaults reduce the system to current behavior — regression-safe.
- **No per-ant identity tracking across training.** The shared ant policy is evaluated per-ant per-tick stateless; the only memory is the commander's history token ring buffer.
- **No imitation learning from observed real-ant trajectories.** Pure PPO from environment reward. Distillation from A3 → A2/A1 is policy-distillation (teacher's distribution), not behavior-cloning from external data.
- **No multi-arena curriculum or domain randomization in this phase.** Stay on the 32×32 fixture so results are directly comparable to the MlpBrain v1 47% baseline. Curriculum work is a separate research thread.
- **No new sim-side mechanics.** Pheromone math, FSM, combat, colony economy are all unchanged. Only new sim surface area is the obs/action pipeline (Section 2).

---

## Architecture

### Tier 1 — Commander policy

One instance per colony. Decides every `DECISION_CADENCE = 5` outer ticks (unchanged from `crates/antcolony-trainer/src/env.rs:10`). Replaces the current `MlpBrain`'s flat MLP.

**Inputs:**

| Field | Shape | Source |
|---|---|---|
| `state_17d` | `f32[17]` | Existing `ColonyAiState` (`crates/antcolony-sim/src/ai/brain.rs:35-71`) |
| `pheromone_field` | `f32[4, 32, 32]` | New: full pheromone grid (food/home/alarm/colony_scent), `AdaptiveAvgPool2d` downsample to 32×32 from the actual arena size |
| `history_tokens` | `f32[K=8, 96]` | New: ring buffer of last 8 commander `(state, action, reward, _pad)` tokens |

**Backbone (concrete dims at the A2 primary sizing point, ~70M params):**

```
pheromone_encoder : Conv2d(4 → 64,  kernel=3, stride=1) → ReLU
                  → Conv2d(64 → 128, kernel=3, stride=2) → ReLU
                  → AvgPool2d(2)
                  → Flatten
                  → Linear(→ 384)

state_encoder     : Linear(17 → 384)

history_encoder   : Linear(96 → 384) applied token-wise (K=8 outputs)

concat            : 1 + 1 + K tokens of d=384 → reproject to d_model=768
transformer       : 8 layers, d_model=768, n_heads=12, ffn=3072
pool              : learned [CLS]-style token → 768-d colony feature
```

**Heads:**

| Head | Output | Used by |
|---|---|---|
| `action_head` | `Linear(768 → 6)` → tanh-squash to [0,1] | Becomes the existing `AiDecision` (`crates/antcolony-sim/src/ai/brain.rs:76-95`) unchanged in shape |
| `intent_head` | `Linear(768 → 64)` | New: broadcast as conditioning input to the ant tier for the duration of this decision window |
| `value_head` | `Linear(768 → 1)` | Critic V(s) for PPO |
| `log_std` | learnable `f32[6]` parameter | Per-dim Gaussian policy std (matches the existing `ActorCritic::log_std` pattern at `crates/antcolony-trainer/src/policy.rs:42`) |

### Tier 2 — Ant policy (shared)

One shared instance per colony. Evaluated **per-ant per-tick**, batched across all ants in the colony into a single GPU call. ~25M params at A2.

**Inputs (per ant):**

| Field | Shape | Source |
|---|---|---|
| `pheromone_cone` | `f32[5, 3, 4]` = 60 floats | New: 5 forward steps × 3 lateral cells × 4 channels along the ant's heading, matching the existing `sense_radius=5` + `sense_angle=60°` cone |
| `internal_state` | `f32[8]` | New: `food_carried`, `heading_sin`, `heading_cos`, `caste_onehot[3]`, `state_timer_norm`, `age_norm` |
| `intent_vec` | `f32[64]` | From commander, latched at the start of each decision window |

**Backbone (A2 dims, ~25M params):**

```
cone_encoder      : Linear(60 → 256) → ReLU → Linear(256 → 256)
state_encoder     : Linear(8 → 128)
intent_encoder    : Linear(64 → 128)
concat            : → 512-d
transformer       : 6 layers, d_model=512, n_heads=8, ffn=2048
pool              : mean over tokens → 512-d ant feature
```

**Heads:**

| Head | Output | Applied to |
|---|---|---|
| `modulator_head` | `Linear(512 → 5)` → `softplus`/`sigmoid` | `(α_mult, β_mult, exploration_mod, deposit_mult, state_bias)` — biases the existing ACO math |
| `value_head` | `Linear(512 → 1)` | Local critic for ant-tier GAE (advantages are colony-level; this is for variance reduction) |
| `log_std` | learnable `f32[5]` | Per-dim Gaussian std |

### Sizing table

Three variants of the **same architecture**, scaling `d_model` / layer count / ffn:

| Variant | Commander dims | Ant dims | Commander | Ant | Total | Target |
|---|---|---|---:|---:|---:|---|
| A1 (compact) | `d=384, 4 layers, ffn=1536` | `d=256, 4 layers, ffn=1024` | ~9M | ~3M | **~12M** | Ultra-compact — older GPUs, CPU inference, distilled from A2 or A3 |
| **A2 (primary)** | `d=768, 8 layers, ffn=3072` | `d=512, 6 layers, ffn=2048` | **~70M** | **~25M** | **~95M** | **8 GB-consumer deployment, kokonoe fp16 training. Distillation target from A3 or trained from scratch.** |
| A3 (teacher) | `d=1024, 10 layers, ffn=4096` | `d=640, 8 layers, ffn=2560` | ~120M | ~40M | **~160M** | cnc P100 research teacher. Distilled to A2 + A1 if it beats A2-from-scratch (Gate 3 below). |

> **Sizing decision after the brainstorm:** A2 is the deployment target — fits 8 GB consumer GPU at inference, trains in fp16 on kokonoe. A3 exists to test whether the 47% Nash plateau is capacity-limited or environment-limited; if A3 doesn't meaningfully beat A2-from-scratch, the A3 spend was the research that proved we don't need it.

---

## Observation + action pipelines

### New sim-side types (`antcolony-sim`)

```rust
/// Commander input.
pub struct RichObservation {
    pub state: ColonyAiState,                  // existing 17-d, unchanged
    pub pheromone_field: PheromoneSnapshot,    // 4 × H × W, downsampled to 32×32
    pub history: ArrayVec<HistoryToken, 8>,    // ring buffer of last 8 commander ticks
}

pub struct PheromoneSnapshot {
    pub food_trail: Box<[f32]>,
    pub home_trail: Box<[f32]>,
    pub alarm:      Box<[f32]>,
    pub colony_scent: Box<[f32]>,
    pub width: u16,
    pub height: u16,
}

pub struct HistoryToken {
    pub state: [f32; 17],
    pub action: [f32; 6],
    pub reward: f32,
    pub _pad: [f32; 72],   // → 96 floats total to match commander backbone d
}

/// Ant input — one entry per adult ant in the colony.
pub struct AntObservation {
    pub ant_id: u32,
    pub pheromone_cone: [f32; 60],    // 5 × 3 × 4
    pub internal: [f32; 8],
    // Commander intent is broadcast separately by apply_commander_intent.
}

/// Ant output — one per ant. Defaults make this a no-op (regression-safe).
pub struct AntModulators {
    pub ant_id: u32,
    pub alpha_mult:      f32,   // clamped [0.1, 5.0],  default 1.0
    pub beta_mult:       f32,   // clamped [0.1, 5.0],  default 1.0
    pub exploration_mod: f32,   // clamped [-0.1, 0.1], default 0.0
    pub deposit_mult:    f32,   // clamped [0.1, 5.0],  default 1.0
    pub state_bias:      f32,   // clamped [-2.0, 2.0], default 0.0
}
```

### New `Simulation` methods

```rust
impl Simulation {
    pub fn colony_rich_observation(&self, colony_id: u8) -> Option<RichObservation>;
    pub fn per_ant_observations(&self, colony_id: u8) -> Vec<AntObservation>;
    pub fn apply_ant_modulators(&mut self, colony_id: u8, mods: &[AntModulators]);
    pub fn apply_commander_intent(&mut self, colony_id: u8, intent: &[f32; 64]);
}
```

### Integration into the existing ACO math

The current direction-selection formula at `crates/antcolony-sim/src/ai/forager.rs` (and adjacent) uses:

```
probability(direction_j) = [pheromone(j)^α × desirability(j)^β] / Σ
```

After modulator wiring (per-ant lookup, no change to the math shape):

```rust
let mods = ant.modulators;  // Default::default() if no brain decision yet
let alpha_eff   = (ALPHA_BASE * mods.alpha_mult).clamp(0.1, 10.0);
let beta_eff    = (BETA_BASE  * mods.beta_mult ).clamp(0.1, 10.0);
let explore_eff = (config.exploration_rate + mods.exploration_mod).clamp(0.0, 1.0);
let deposit_eff = config.deposit_food_trail * mods.deposit_mult;
// state_bias adds to one specific FSM transition logit (e.g. Exploring → FollowingTrail)
```

`AntModulators::default()` returns `(1.0, 1.0, 0.0, 1.0, 0.0)` — the no-op. Running the sim with all defaults reproduces today's behavior bit-for-bit.

### Per-tick data flow in `MatchEnv::step` (hierarchical)

```
═════ COMMANDER DECISION (every 5 outer ticks) ═════
  observe_rich(0)  → RichObservation
  observe_rich(1)  → RichObservation
  Commander.forward(rich_0, rich_1) → (action_0, intent_0, action_1, intent_1)
  sim.apply_ai_decision(0, action_0)
  sim.apply_ai_decision(1, action_1)
  sim.apply_commander_intent(0, intent_0)
  sim.apply_commander_intent(1, intent_1)

═════ OUTER TICK LOOP (5 iterations) ═════
  ── ANT DECISIONS (each tick, batched) ──
     for colony in [0, 1]:
        obs_batch  = sim.per_ant_observations(colony)
        mods_batch = AntPolicy.forward(obs_batch)   # one GPU call per colony per tick
        sim.apply_ant_modulators(colony, &mods_batch)

  ── sim.tick() ──   (uses per-ant modulators inside the existing ACO math)

═════ REWARD + HISTORY ═════
  Compute step reward (existing r6 shaping at env.rs:117-145, unchanged)
  Append (state, action, reward, _pad) → HistoryToken in each colony's commander ring
```

### Memory + perf check

| Item | Per-call size | Cadence |
|---|---:|---|
| `RichObservation` (32×32×4 + 17 + 8×96) | ~17 KB | every 5 ticks × 2 colonies |
| `AntObservation` × 10 ants | ~2.7 KB / colony | every tick × 2 colonies |
| `AntObservation` × 10k ants (perf budget) | ~2.7 MB | same cadence |
| `AntModulators` × ants | ~20 B / ant | per tick |
| Pheromone snapshot copy | 16 KB / 32×32 arena | per 5 ticks |

Nothing here is a hot-path concern at sim scale. The `apply_ant_modulators` write is the only inner-loop new cost — five `f32` per ant — trivial.

---

## Training loop + multi-GPU + VRAM

### Joint PPO loss

Two policies, two rollout buffers (different cadences), one shared environment reward stream.

```
                   cadence      buffer entry / env       horizon
Commander          5 ticks      1 per decision cycle     256 cycles
Ant                1 tick       N_ants per tick          1280 tick-steps × N_ants
```

```
L_cmdr_actor   = -E[ min( r_θ · A_t,  clip(r_θ, 1 ± ε) · A_t ) ]      # ε = 0.2
L_cmdr_critic  =  MSE( V_φ(s_t),  R_t )
L_cmdr_ent     = -β_c · H(π_θ)                                        # β_c = 0.005

L_ant_actor    = -E[ min( r_ψ · A_local_t,  clip(r_ψ, 1 ± ε) · A_local_t ) ]
L_ant_critic   =  MSE( V_ξ(s_local_t),  R_local_t )
L_ant_ent      = -β_a · H(π_ψ)                                        # β_a = 0.01

L_total = (L_cmdr_actor + 0.5 · L_cmdr_critic + L_cmdr_ent)
        + α_balance · (L_ant_actor + 0.5 · L_ant_critic + L_ant_ent)
```

`α_balance` starts at `1 / N_ants_avg ≈ 0.1` so commander gradients aren't drowned out by ~10× more ant samples per cycle. Tuned during training. If joint training is unstable, fall back to the staged schedule (freeze ant for first 20% of training).

### Multi-GPU split (asymmetric, matches P100 hardware)

P100s have **no NVLink** — DDP all-reduce over PCIe is slow. The rollout/train split syncs weights only every N iters, much cheaper than per-step all-reduce.

```
P100-PCIE-12GB  (Rollout GPU)              P100-PCIE-16GB  (Train GPU)
─────────────────────────                  ─────────────────────────
Policy weights (fp16)                      Policy weights (fp32 + fp16 cast)
256–1024 parallel envs                     Optimizer states (Adam fp32)
Inference-only forward passes              Gradients (fp16)
Rollout staging tensors                    Backward + PPO update
                                           Eval bench
        ─ pushes rollouts every ~1k envsteps ─
        ─ pulls fresh weights every N iters ─
                    PCIe Gen3 ×16 (~12 GB/s)
```

### VRAM budget — for A3 teacher (~160M params)

**P100-16GB (train):**

| Item | Size |
|---|---:|
| Weights fp32 (canonical) | 640 MB |
| Weights fp16 (forward cast) | 320 MB |
| Gradients fp16 | 320 MB |
| Adam m, v (fp32) | 1,280 MB |
| Activations (backward, batch 512 envs × 50 steps) | ~7,000 MB |
| Framework / cudnn overhead | ~500 MB |
| **Total** | **~10 GB** (6 GB headroom) |

**P100-12GB (rollout):**

| Item | Size |
|---|---:|
| Weights fp16 (read-only) | 320 MB |
| 1024 parallel env sim states | ~2,000 MB |
| Inference activations | ~800 MB |
| Rollout staging tensors | ~4,000 MB |
| **Total** | **~7 GB** (5 GB headroom) |

For A2 (95M) and A1 (12M), everything scales proportionally — fit on either P100 alone, comfortable on kokonoe (3070 Ti 8GB) with fp16.

### Precision

- **cnc P100s**: fp16 mixed precision for weights / grads / activations, fp32 for optimizer state. Pascal has no tensor cores so this is a memory win, not a speed win — but the memory halving enables a 2× larger batch, which is what actually moves PPO sample efficiency.
- **kokonoe 3070 Ti**: native bf16 (Ampere). Real speedup + memory win. Use for fast distillation iteration.

### Parallel envs + rollout shape (starting config)

```
N_envs                  = 512
rollout_steps_per_env   = 256       # outer ticks per slice
total_samples_per_iter  = 131,072
minibatch_size          = 4096
epochs_per_minibatch    = 4
```

Compared to the current trainer's `matches_per_iter = 32`, single-env (`crates/antcolony-trainer/src/ppo.rs:39-71`), that's a ~64× increase in samples per iteration. Most of the "P100 max-out" comes from this.

### Trainer crate changes

- `policy.rs` gains `CommanderPolicy`, `AntPolicy`, `HierarchicalActorCritic`. Existing `ActorCritic` stays as the regression baseline.
- `ppo.rs` gains `JointPpoTrainer` parallel to the existing `PpoTrainer`. Existing single-tier trainer is unchanged so we keep our 47% baseline reproducible.
- `env.rs` extends `MatchEnv` with `rich_obs()`, `per_ant_obs()`, `apply_ant_modulators()`, `apply_commander_intent()` — mirrors the sim-side API from Section 2.
- New `parallel_env.rs` to wrap N envs into a single batched stepper.
- New `multi_gpu.rs` with `RolloutTrainSplit` driver coordinating the two devices.
- New `distillation.rs` for the A3 → A2 → A1 student-training loop (used after A3 converges).

---

## Compression / ship-down tracks

Three orthogonal tracks, applied after A3 is trained on cnc.

### Track 1 — Policy distillation (gets A2/A1 as separate networks)

Train a smaller student to match the teacher's action distribution + value estimate. Standard policy-distillation recipe (Rusu et al. 2015):

```
L_student = KL(teacher_action_dist || student_action_dist)
          + MSE(teacher_value, student_value)
          + λ · L_PPO_on_env             (optional, small, for stability)
```

Dense supervision → far fewer environment samples needed than from-scratch RL. Student training fits on kokonoe (fp16, 8 GB). Each distillation step is ~1/10 the compute of the A3 teacher run.

### Track 2 — Quantization (smaller versions of each)

Orthogonal to distillation:

| Precision | Memory | Quality cost | Hardware |
|---|---:|---|---|
| fp32 | 4 B/param | none | universal |
| fp16 | 2 B/param | minimal at inference | Ampere+ |
| int8 (calibrated) | 1 B/param | 1–2% policy drop | broad |
| int4 (GPTQ-style) | 0.5 B/param | 3–5% drop | edge / CPU |

For a 95M A2 model: fp16 → 190 MB, int8 → 95 MB. Both trivially fit 8 GB with the game running.

### Track 3 — Slimmable training (one network, multiple widths)

Optional. Train A3 with a width curriculum (each step samples a random sub-width: 100%, 60%, 25%). The same trained weights can be **sliced** at inference. Less mature in RL than supervised settings; deferred unless Track 1 distillation hits issues.

---

## Evaluation methodology

**Primary metric:** mean win rate against the existing 7-archetype bench (HeuristicBrain, DefenderBrain, AggressorBrain, EconomistBrain, BreederBrain, ForagerBrain, ConservativeBuilderBrain), 50 matches per opponent, 350 total per checkpoint. **Same harness used to measure MlpBrain v1's 47.1% Nash plateau** — apples-to-apples.

**Comparison ablations (run as separate training runs):**

| Comparison | Tests |
|---|---|
| Hierarchical-A3 vs MlpBrain v1 | Headline: did we beat the plateau? |
| Hierarchical-A3 vs Flat-A3 (same params, no tier split) | Did the *hierarchy* contribute? |
| Hierarchical-A3 vs A3-commander-only (ant tier disabled = defaults) | Did the *ant tier* contribute? |
| Distilled-A1 vs Scratch-A1 | Does distillation pay off? |
| Distilled-A2 vs Scratch-A2 | Same question one tier up |

Without these, "beat the plateau" doesn't tell us *why* — they're the difference between a result and a paper for the outreach roadmap.

**Secondary metrics (logged every eval):**
- Per-archetype win-rate breakdown (catches specialization brittleness)
- Mean match length (decisiveness)
- Action-distribution entropy on both tiers (collapse → stagnation alarm)
- KL between consecutive checkpoints (instability alarm)

---

## Risks + mitigations

| Risk | Mitigation |
|---|---|
| Bigger model + new obs doesn't beat 47% — plateau is environment-limited, not capacity-limited | Built into the ship sequence as Gate 3 (below). Worst case is finding out the environment needs work (longer matches, richer rewards, more terrain). Worth knowing — and exactly what the ablations isolate. |
| Joint PPO unstable (commander drowned out by ant gradients) | `α_balance` is the first knob. Fallback: staged schedule — freeze ant tier for first 20% of training, then unfreeze. |
| Rollout/train weight staleness causes off-policy bias | Configurable sync frequency. Start with broadcast every 5 iters; tighten if KL between rollout-policy and train-policy exceeds threshold. |
| A3 OOMs on P100-16GB | Halve `N_envs` to 256, double `minibatch_size`. Same samples per iter, half activation memory. |
| cnc has 8 GB system RAM per `fleet/CNC-Server.md` — limits CPU-side rollout staging | **First action: verify actual RAM** (wiki may be stale). If genuinely 8 GB, keep rollout buffer GPU-resident; we have 28 GB of VRAM, more than CPU RAM. |
| Pascal sm_60 has no FlashAttention → attention is slow | Transformer is shallow (6 + 4 layers). Standard scaled-dot-product attention is fine on sm_60 at our sequence lengths. |
| Reward shaping (r6, `env.rs:117-145`) was tuned for flat MlpBrain | Keep r6 as the baseline first run. If hierarchical training shows sparse-reward symptoms (entropy collapse + flat win rate), revisit shaping as a separate research thread — out of scope for this design. |
| Ant brain over-modulates into pathological regions | Tight clamp ranges. High β_a entropy bonus. If still pathological, add `λ · ‖modulators − defaults‖²` regularizer. |
| 32×32 arena too small for the richer obs to matter | Scale to 64×64 in training if A3 underperforms — memory budget still fits. Note for second run. |
| Distillation doesn't help (teacher and student plateau identically) | Tells us capacity isn't the differentiator at A1/A2 sizes. Ship A2 from scratch, skip Track 1 / 2. Cheap to find out — distill runs are fast. |
| `MatchEnv` arena size assumptions break — sim is 32×32, but other entry points (`Simulation::new_*`) use bigger arenas. Pheromone snapshot downsampling needs to be parameterized. | Hard-code downsample target = 32×32. Document the downsample step explicitly so any future arena change knows to update it. |

---

## Go / no-go gates

```
GATE 1 — A3 runs without OOM/NaN/crash, 50 iters
   ✓ go: continue training
   ✗ no-go: debug numerics / shapes, reduce batch, re-launch

GATE 2 — A3 hits ≥ 60% mean win rate at iter 200
   ✓ go: continue to convergence
   ✗ no-go: try reward shaping change OR scale arena to 64×64 OR freeze-ant warm-start

GATE 3 — A3 final win rate at convergence
   > 65%   → A3 is the headline; run Track 1 distillation to A2 + A1
   55–65%  → A3 helps marginally. Ship A2 from scratch; skip distillation.
   < 55%   → plateau is environmental, not capacity. Write up findings, defer A3 deployment.

GATE 4 — Distilled A2 retains ≥ 95% of A3 win rate
   ✓ go: ship A2 + A1 distilled
   ✗ no-go: A1 only; A2 distilled isn't better than A2 scratch — drop the middle tier

GATE 5 — A1 retains ≥ 90% of A3 win rate after fp16/int8 quantization
   ✓ go: that's the consumer deployment binary
   ✗ no-go: A2 is the smallest deployable; A1 dropped
```

---

## Ship sequence

1. **Implementation plan** — via `superpowers:writing-plans` (next step after this spec).
2. **Phase 1: sim-side observation/action plumbing.** New types, new `Simulation` methods, modulator integration in ACO math. Defaults preserve current behavior (regression tests stay green).
3. **Phase 2: trainer-side hierarchical policy.** `CommanderPolicy`, `AntPolicy`, `HierarchicalActorCritic`. Single-GPU first (kokonoe CPU/CUDA) to debug shapes + numerics.
4. **Phase 3: parallel-env stepper + rollout/train split driver.** Verifies the multi-GPU coordination on cnc P100s with a tiny A1-sized model first.
5. **Phase 4: A3 training run on cnc.** Full 500–1000 iter run. Gates 1 and 2 evaluated en route.
6. **Phase 5: Gate 3 decision + downstream tracks.** Either distillation track or A2-from-scratch fallback.
7. **Phase 6: quantization + deployment package.** Ship-ready binaries.

---

## Open questions / known unknowns

- **cnc system RAM ground truth.** Wiki says 8 GB; the running workload (Postgres + multiple Podman containers + llama-rpc + ...) suggests more. **Verify before Phase 4.** If actually low, rollout buffer must live GPU-side.
- **`Simulation` constructor surface for arenas larger than 32×32.** The current `MatchEnv` hard-codes 32×32. Larger arenas (e.g., 64×64) will need a different `MatchEnv::new` variant. Defer until Gate 2 says we need it.
- **`state_bias` semantics in the FSM.** Documented as "logit bias for one specific transition" — exact transition site (`Exploring → FollowingTrail` vs something else) chosen during implementation by inspecting the FSM transition code.
- **Whether to warm-start commander `action_head` from MlpBrain v1 weights.** Round-trip-compatible (same output shape and range), would speed up early training. Decision deferred to implementation; default is fresh init.
- **History token contents.** Spec says 17 (state) + 6 (action) + 1 (reward) + 72 (pad) = 96. The pad is unused space; if useful auxiliary features surface (pheromone aggregate stats, opponent action estimate, etc.), we can spend pad bytes on them without changing the architecture.

---

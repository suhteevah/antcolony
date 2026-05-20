# Hierarchical Ant Brain — Phase 2a: Forward-Only Policy Nets + `deposit_mult` Sim Wiring

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the **forward pass** of the hierarchical commander + ant policy nets in `antcolony-trainer`, sized for the A1 smoke target (~12M params), with a smoke test that drives them end-to-end from a fresh `Simulation` instance. Also wire the per-ant `deposit_mult` modulator into the sim's pheromone-deposit math so the modulator pipeline is no longer dead on that field. No training loop yet — that's Phase 2b.

**Architecture:** Two parallel tier classes — `CommanderPolicy` (CNN + state + history → transformer → action/intent/value heads) and `AntPolicy` (small transformer over local cone + internal + intent → modulator/value heads). A combined `HierarchicalActorCritic` holds both. A shared `TransformerBlock` primitive is implemented from `candle_nn::Linear` + `LayerNorm` primitives (no `candle-transformers` dep — we control the size). Sizing constants for A1/A2/A3 live in a sibling module so the smoke-and-training tracks share one source of truth. All work is in a new `crates/antcolony-trainer/src/hierarchical/` module; the existing `policy.rs` (flat `ActorCritic` MLP) stays untouched as the 47% Nash regression baseline.

**Tech Stack:** Rust 2024 (workspace MSRV 1.85 per Cargo.toml but the project memory notes `stable-gnu` is in use). `candle-core` + `candle-nn` from the workspace (already deps of `antcolony-trainer`). No new crates needed for Phase 2a.

**Predecessor:** `docs/superpowers/plans/2026-05-18-ant-brain-phase1-sim-plumbing.md` (shipped on main at `c3ba378`).
**Spec:** `docs/superpowers/specs/2026-05-18-ant-brain-hierarchical-design.md`.
**Successor:** Phase 2b plan — `JointPpoTrainer` joint loss + GAE, full `MatchEnv` integration, `state_bias` wiring into FSM transition logits, first 5-iter smoke training run.

---

## File map

| File | Status | Responsibility |
|---|---|---|
| `crates/antcolony-sim/src/simulation.rs` | MODIFY (~2 lines) | Wire `ant.modulators.deposit_mult` into the two pheromone-deposit sites |
| `crates/antcolony-sim/tests/phase1_plumbing.rs` | MODIFY | Add behavioral test: high `deposit_mult` strengthens pheromone deposition |
| `crates/antcolony-trainer/src/lib.rs` | MODIFY | Add `pub mod hierarchical;` + re-export public types |
| `crates/antcolony-trainer/src/hierarchical/mod.rs` | **CREATE** | Module declarations + re-exports |
| `crates/antcolony-trainer/src/hierarchical/sizing.rs` | **CREATE** | `Sizing` struct + A1/A2/A3 const instances + `est_params` helper |
| `crates/antcolony-trainer/src/hierarchical/transformer.rs` | **CREATE** | `TransformerBlock` — multi-head self-attention + LayerNorm + FFN |
| `crates/antcolony-trainer/src/hierarchical/commander.rs` | **CREATE** | `CommanderPolicy` — pheromone CNN + state/history encoders + transformer + 3 heads |
| `crates/antcolony-trainer/src/hierarchical/ant.rs` | **CREATE** | `AntPolicy` — cone/internal/intent encoders + transformer + 2 heads |
| `crates/antcolony-trainer/src/hierarchical/actor_critic.rs` | **CREATE** | `HierarchicalActorCritic` — composes both tiers, single `new()` builder |
| `crates/antcolony-trainer/tests/hierarchical_smoke.rs` | **CREATE** | End-to-end: spin sim, observe, forward through HAC, assert tensor shapes |

**File-size discipline:** keep each new file under ~250 lines. The transformer block is ~100 lines on its own; commander.rs and ant.rs are ~150 lines each. If `commander.rs` grows past 250 lines (e.g., to factor an `Encoders` substruct), split into a directory — but defer that decision until it actually grows.

---

### Task 1: Wire `deposit_mult` into pheromone deposit math

**Files:**
- Modify: `crates/antcolony-sim/src/simulation.rs` (two deposit sites)
- Modify: `crates/antcolony-sim/tests/phase1_plumbing.rs` (add behavioral test)

This is a small sim-side warmup before policy-net work. The `ant.modulators.deposit_mult` field has been clamped-and-stored since Phase 1 Task 8, but the sim ignores it. Wire it in.

- [ ] **Step 1: Locate the deposit sites**

Run: `grep -n 'deposit_food_trail\|deposit_home_trail\|\.deposit(' J:/antcolony/crates/antcolony-sim/src/simulation.rs | head -20`

You should find pheromone deposit calls (the Phase 1 plan noted them at `simulation.rs:1542, 1561`). After the cargo-fmt sweep in commit `da3538e` the line numbers may have shifted — find them by content, not by line number. The call shape is roughly `self.pheromones_mut().deposit(x, y, PheromoneLayer::FoodTrail, deposit_strength, max_intensity)` (or similar). There may be ant-context available at each site (i.e., the calling code has access to the depositing ant). If not — if the deposit is inside a system that doesn't carry the ant ref — note that as `BLOCKED` and report; we'd need a different wiring approach.

- [ ] **Step 2: Write the failing behavioral test**

Add to `crates/antcolony-sim/tests/phase1_plumbing.rs`:

```rust
#[test]
fn deposit_mult_strengthens_pheromone_deposition() {
    use antcolony_sim::ai::observation::AntModulators;
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    fn run_sim(seed: u64, ticks: u64, deposit_mult: f32) -> f32 {
        let cfg = SimConfig {
            world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
            pheromone: PheromoneConfig::default(),
            ant: AntConfig { initial_count: 10, ..AntConfig::default() },
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        };
        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, seed, 0, 2);

        for t in 0..ticks {
            if t % 5 == 0 {
                let obs0 = sim.per_ant_observations(0);
                let mods: Vec<_> = obs0.iter().map(|_| AntModulators {
                    alpha_mult: 1.0,
                    beta_mult: 1.0,
                    exploration_mod: 0.0,
                    deposit_mult,
                    state_bias: 0.0,
                }).collect();
                let ids: Vec<_> = obs0.iter().map(|o| o.ant_id).collect();
                sim.apply_ant_modulators(0, &mods, &ids);
            }
            sim.tick();
        }
        // Sum total food_trail pheromone intensity across the arena.
        let rich = sim.colony_rich_observation(0).unwrap();
        rich.pheromone_field.food_trail.iter().sum()
    }

    // Same seed, same ticks, only deposit_mult differs.
    // Higher deposit_mult should produce more pheromone in the world.
    let low = run_sim(0xdeb05_a, 500, 1.0);
    let high = run_sim(0xdeb05_a, 500, 5.0);

    assert!(
        high > low * 1.5,
        "deposit_mult=5.0 should produce noticeably more pheromone than 1.0 \
         (got high={}, low={}, ratio={:.2})",
        high, low, high / low.max(1e-6),
    );
}
```

- [ ] **Step 3: Run the test, expect failure**

Run: `cd J:/antcolony && cargo test -p antcolony-sim --test phase1_plumbing deposit_mult_strengthens_pheromone_deposition 2>&1 | tail -10`
Expected: FAIL — `high` should approximately equal `low` because `deposit_mult` is currently ignored.

- [ ] **Step 4: Wire `deposit_mult` into the deposit sites**

At each deposit site you found in Step 1, multiply the deposit strength by `ant.modulators.deposit_mult` (clamped values from `apply_ant_modulators` are already in the safe range `[0.1, 5.0]`):

Example transform (adapt to the actual call shape you find):
```rust
// BEFORE:
self.pheromones_mut().deposit(x, y, PheromoneLayer::FoodTrail, pcfg.deposit_food_trail, ...);

// AFTER:
let deposit_strength = pcfg.deposit_food_trail * ant.modulators.deposit_mult;
self.pheromones_mut().deposit(x, y, PheromoneLayer::FoodTrail, deposit_strength, ...);
```

If the call site doesn't have a direct `ant` reference (e.g., the deposit happens in a helper that takes only `position` + `layer`), thread the `deposit_mult` as a parameter instead of trying to pass the whole ant.

Apply at BOTH deposit sites (food_trail and home_trail). The clamp on read is unnecessary — the field is already write-clamped by `apply_ant_modulators`.

- [ ] **Step 5: Run test + full sim sweep**

Run: `cargo test -p antcolony-sim --test phase1_plumbing deposit_mult_strengthens_pheromone_deposition 2>&1 | tail -10`
Expected: PASS.

Run: `cargo test -p antcolony-sim 2>&1 | tail -5`
Expected: All 164 tests still pass (no regressions to the existing tests).

Also confirm the **baseline-regression test** still passes (defaults still produce identity behavior — `deposit_mult=1.0` is the identity so it should):

Run: `cargo test -p antcolony-sim --test phase1_plumbing defaults_reproduce_baseline_population_trajectory 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-sim/src/simulation.rs crates/antcolony-sim/tests/phase1_plumbing.rs
git commit -m "sim: wire deposit_mult modulator into pheromone deposit math

The deposit_mult field on AntModulators has been clamped-and-stored
since Phase 1 Task 8 but the sim was ignoring it. Multiplied the two
deposit-strength sites in simulation.rs by ant.modulators.deposit_mult.
Default 1.0 is the identity (verified by
defaults_reproduce_baseline_population_trajectory still passing); high
values produce proportionally stronger pheromone trails (verified by
new deposit_mult_strengthens_pheromone_deposition test).

Refs: docs/superpowers/plans/2026-05-20-ant-brain-phase2a-policy-nets.md
"
```

---

### Task 2: Hierarchical module skeleton

**Files:**
- Create: `crates/antcolony-trainer/src/hierarchical/mod.rs`
- Modify: `crates/antcolony-trainer/src/lib.rs`

- [ ] **Step 1: Create the module skeleton**

Create `crates/antcolony-trainer/src/hierarchical/mod.rs`:

```rust
//! Hierarchical commander + per-ant policy nets for the ant-brain Phase 2.
//!
//! - [`sizing`] — A1/A2/A3 dim presets shared by both tiers
//! - [`transformer`] — minimal transformer block primitive (multi-head attn +
//!   LayerNorm + FFN), used by both tier backbones
//! - [`commander::CommanderPolicy`] — outer-tick commander brain
//! - [`ant::AntPolicy`] — per-ant brain (shared instance per colony)
//! - [`actor_critic::HierarchicalActorCritic`] — composes both tiers
//!
//! The existing flat [`crate::policy::ActorCritic`] MLP is unchanged — it
//! remains the 47% Nash regression baseline.

pub mod actor_critic;
pub mod ant;
pub mod commander;
pub mod sizing;
pub mod transformer;

pub use actor_critic::HierarchicalActorCritic;
pub use ant::AntPolicy;
pub use commander::CommanderPolicy;
pub use sizing::{Sizing, A1, A2, A3};
```

- [ ] **Step 2: Add the module + re-exports to lib.rs**

In `crates/antcolony-trainer/src/lib.rs`, find the existing module declarations near the top. Add:

```rust
pub mod hierarchical;
```

Add to the public re-exports section:

```rust
pub use hierarchical::{HierarchicalActorCritic, CommanderPolicy, AntPolicy, Sizing};
```

- [ ] **Step 3: Stub the sub-modules so the crate still compiles**

The `mod.rs` references five sub-modules. Each must exist as a file (even if empty) for the crate to compile. Create empty placeholders:

```bash
cd J:/antcolony
touch crates/antcolony-trainer/src/hierarchical/sizing.rs
touch crates/antcolony-trainer/src/hierarchical/transformer.rs
touch crates/antcolony-trainer/src/hierarchical/commander.rs
touch crates/antcolony-trainer/src/hierarchical/ant.rs
touch crates/antcolony-trainer/src/hierarchical/actor_critic.rs
```

Each is now an empty file. The `pub use` lines in `mod.rs` reference items that don't exist yet — comment them out for now so the crate compiles. Add a TODO comment noting they'll be uncommented as each sub-module lands:

```rust
// pub use actor_critic::HierarchicalActorCritic;  // uncommented in Task 8
// pub use ant::AntPolicy;                          // uncommented in Task 7
// pub use commander::CommanderPolicy;              // uncommented in Task 6
pub use sizing::{Sizing, A1, A2, A3};               // landed in Task 3
```

Wait — `sizing` lands in Task 3, not yet. Comment ALL the `pub use` lines for now; they'll be uncommented in their owning tasks. Add a single TODO at top of mod.rs noting the staged uncommenting.

Also do the same in `lib.rs` — comment out the `pub use hierarchical::{...}` line; uncomment progressively.

- [ ] **Step 4: Verify build**

Run: `cd J:/antcolony && cargo build -p antcolony-trainer 2>&1 | tail -5`
Expected: BUILDS clean. May emit `unused_imports` warnings on `pub mod` lines whose modules are empty — that's expected and OK.

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/ crates/antcolony-trainer/src/lib.rs
git commit -m "trainer: hierarchical module skeleton (Phase 2a setup)

Empty sub-module placeholders for sizing/transformer/commander/ant/
actor_critic. pub mod hierarchical added to lib.rs. Re-exports
commented out and will be uncommented as each sub-module lands.
Existing crate::policy::ActorCritic stays intact as the regression
baseline.
"
```

---

### Task 3: Sizing constants module (A1/A2/A3)

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/sizing.rs`

- [ ] **Step 1: Write failing tests**

In `crates/antcolony-trainer/src/hierarchical/sizing.rs`:

```rust
//! Sizing presets for the hierarchical brain. A1 is the smoke target
//! (~12M params, fits on kokonoe 3070 Ti 8GB easily); A2 is the
//! 8GB-consumer deployment target (~95M params); A3 is the cnc P100
//! research teacher (~160M params). See the design spec for context.

/// Sizing preset for the hierarchical policy net. Holds dims for both
/// commander and ant tiers. The `est_*_params` methods give a rough
/// parameter-count estimate used as a sanity-check assertion in tests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizing {
    // Commander tier
    pub cmdr_d_model: usize,
    pub cmdr_layers: usize,
    pub cmdr_heads: usize,
    pub cmdr_ffn: usize,
    pub cmdr_pheromone_channels_mid: usize,  // first Conv2d out channels
    pub cmdr_pheromone_channels_hi: usize,   // second Conv2d out channels
    pub cmdr_encoder_dim: usize,             // dim each encoder maps to before reproject
    // Ant tier
    pub ant_d_model: usize,
    pub ant_layers: usize,
    pub ant_heads: usize,
    pub ant_ffn: usize,
    pub ant_cone_hidden: usize,    // cone_encoder hidden dim
    pub ant_internal_hidden: usize, // state_encoder out dim
    pub ant_intent_hidden: usize,   // intent_encoder out dim
    // Shared fixed shapes (NOT scaled by sizing — these match the sim API)
    pub fixed_state_d: usize,        // 17 — ColonyAiState
    pub fixed_action_d: usize,       // 6 — AiDecision
    pub fixed_intent_d: usize,       // 64 — broadcast vector
    pub fixed_history_k: usize,      // 8 — ring buffer depth
    pub fixed_history_tok_d: usize,  // 96 — HistoryToken FLAT_LEN
    pub fixed_cone_d: usize,         // 60 — AntObservation pheromone_cone
    pub fixed_internal_d: usize,     // 8 — AntObservation internal
    pub fixed_modulator_d: usize,    // 5 — AntModulators output
    pub fixed_pheromone_w: usize,    // 32 — downsampled pheromone field width
    pub fixed_pheromone_h: usize,    // 32 — downsampled pheromone field height
    pub fixed_pheromone_c: usize,    // 4 — pheromone channels
}

pub const FIXED_STATE_D: usize = 17;
pub const FIXED_ACTION_D: usize = 6;
pub const FIXED_INTENT_D: usize = 64;
pub const FIXED_HISTORY_K: usize = 8;
pub const FIXED_HISTORY_TOK_D: usize = 96;
pub const FIXED_CONE_D: usize = 60;
pub const FIXED_INTERNAL_D: usize = 8;
pub const FIXED_MODULATOR_D: usize = 5;
pub const FIXED_PHEROMONE_W: usize = 32;
pub const FIXED_PHEROMONE_H: usize = 32;
pub const FIXED_PHEROMONE_C: usize = 4;

const fn fixed_defaults() -> Sizing {
    // Filled by each preset; this is a base with all fixed_* slots populated.
    Sizing {
        cmdr_d_model: 0, cmdr_layers: 0, cmdr_heads: 0, cmdr_ffn: 0,
        cmdr_pheromone_channels_mid: 0, cmdr_pheromone_channels_hi: 0,
        cmdr_encoder_dim: 0,
        ant_d_model: 0, ant_layers: 0, ant_heads: 0, ant_ffn: 0,
        ant_cone_hidden: 0, ant_internal_hidden: 0, ant_intent_hidden: 0,
        fixed_state_d: FIXED_STATE_D,
        fixed_action_d: FIXED_ACTION_D,
        fixed_intent_d: FIXED_INTENT_D,
        fixed_history_k: FIXED_HISTORY_K,
        fixed_history_tok_d: FIXED_HISTORY_TOK_D,
        fixed_cone_d: FIXED_CONE_D,
        fixed_internal_d: FIXED_INTERNAL_D,
        fixed_modulator_d: FIXED_MODULATOR_D,
        fixed_pheromone_w: FIXED_PHEROMONE_W,
        fixed_pheromone_h: FIXED_PHEROMONE_H,
        fixed_pheromone_c: FIXED_PHEROMONE_C,
    }
}

/// A1 — compact smoke target. ~12M total params (~9M commander + ~3M ant).
pub const A1: Sizing = Sizing {
    cmdr_d_model: 384,
    cmdr_layers: 4,
    cmdr_heads: 6,
    cmdr_ffn: 1536,
    cmdr_pheromone_channels_mid: 32,
    cmdr_pheromone_channels_hi: 64,
    cmdr_encoder_dim: 192,
    ant_d_model: 256,
    ant_layers: 4,
    ant_heads: 4,
    ant_ffn: 1024,
    ant_cone_hidden: 128,
    ant_internal_hidden: 64,
    ant_intent_hidden: 64,
    ..fixed_defaults()
};

/// A2 — 8GB-consumer deployment target. ~95M total (~70M commander + ~25M ant).
pub const A2: Sizing = Sizing {
    cmdr_d_model: 768,
    cmdr_layers: 8,
    cmdr_heads: 12,
    cmdr_ffn: 3072,
    cmdr_pheromone_channels_mid: 64,
    cmdr_pheromone_channels_hi: 128,
    cmdr_encoder_dim: 384,
    ant_d_model: 512,
    ant_layers: 6,
    ant_heads: 8,
    ant_ffn: 2048,
    ant_cone_hidden: 256,
    ant_internal_hidden: 128,
    ant_intent_hidden: 128,
    ..fixed_defaults()
};

/// A3 — cnc P100 research teacher. ~160M total (~120M commander + ~40M ant).
pub const A3: Sizing = Sizing {
    cmdr_d_model: 1024,
    cmdr_layers: 10,
    cmdr_heads: 16,
    cmdr_ffn: 4096,
    cmdr_pheromone_channels_mid: 64,
    cmdr_pheromone_channels_hi: 128,
    cmdr_encoder_dim: 512,
    ant_d_model: 640,
    ant_layers: 8,
    ant_heads: 10,
    ant_ffn: 2560,
    ant_cone_hidden: 384,
    ant_internal_hidden: 192,
    ant_intent_hidden: 192,
    ..fixed_defaults()
};

impl Sizing {
    /// Rough param-count estimate for the commander transformer backbone
    /// (excluding encoders and heads). Used for sanity-check assertions.
    /// Formula: layers × (4·d² + 2·d·ffn) — accounts for QKV+O projections
    /// (4·d²) and the FFN (d→ffn→d = 2·d·ffn).
    pub fn est_cmdr_transformer_params(&self) -> usize {
        self.cmdr_layers * (4 * self.cmdr_d_model * self.cmdr_d_model
            + 2 * self.cmdr_d_model * self.cmdr_ffn)
    }

    pub fn est_ant_transformer_params(&self) -> usize {
        self.ant_layers * (4 * self.ant_d_model * self.ant_d_model
            + 2 * self.ant_d_model * self.ant_ffn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a1_total_transformer_params_in_smoke_range() {
        // A1 commander transformer ≈ 4×(4×384² + 2×384×1536) = 4×(589824 + 1179648)
        //                          ≈ 4×1.77M = 7.07M
        // A1 ant transformer       ≈ 4×(4×256² + 2×256×1024) = 4×(262144 + 524288)
        //                          ≈ 4×0.79M = 3.15M
        // Total ~10.2M — within the ~12M total A1 ballpark once you add
        // encoders + heads.
        let cmdr = A1.est_cmdr_transformer_params();
        let ant = A1.est_ant_transformer_params();
        assert!(cmdr >= 5_000_000 && cmdr <= 10_000_000,
            "A1 commander transformer expected ~7M, got {}", cmdr);
        assert!(ant >= 2_000_000 && ant <= 5_000_000,
            "A1 ant transformer expected ~3M, got {}", ant);
    }

    #[test]
    fn a2_total_transformer_params_in_8gb_range() {
        // A2 commander ≈ 8×(4×768² + 2×768×3072) = 8×(2.36M + 4.72M) = 56.6M
        // A2 ant       ≈ 6×(4×512² + 2×512×2048) = 6×(1.05M + 2.10M) = 18.9M
        let cmdr = A2.est_cmdr_transformer_params();
        let ant = A2.est_ant_transformer_params();
        assert!(cmdr >= 40_000_000 && cmdr <= 80_000_000,
            "A2 commander transformer expected ~57M, got {}", cmdr);
        assert!(ant >= 12_000_000 && ant <= 30_000_000,
            "A2 ant transformer expected ~19M, got {}", ant);
    }

    #[test]
    fn fixed_dims_match_phase1_sim_api() {
        // These constants must match the shapes that antcolony-sim's
        // observation types actually carry. Pinning them here as
        // a contract — if the sim API changes, this test fails first.
        assert_eq!(FIXED_STATE_D, 17);    // ColonyAiState
        assert_eq!(FIXED_ACTION_D, 6);    // AiDecision
        assert_eq!(FIXED_INTENT_D, 64);   // commander → ant intent
        assert_eq!(FIXED_HISTORY_K, 8);   // ring depth
        assert_eq!(FIXED_HISTORY_TOK_D, antcolony_sim::HistoryToken::FLAT_LEN);
        assert_eq!(FIXED_CONE_D, 60);     // AntObservation.pheromone_cone
        assert_eq!(FIXED_INTERNAL_D, 8);  // AntObservation.internal
        assert_eq!(FIXED_MODULATOR_D, 5); // AntModulators outputs
        assert_eq!(FIXED_PHEROMONE_W, 32);
        assert_eq!(FIXED_PHEROMONE_H, 32);
        assert_eq!(FIXED_PHEROMONE_C, 4);
    }
}
```

- [ ] **Step 2: Uncomment the sizing re-export in mod.rs**

In `crates/antcolony-trainer/src/hierarchical/mod.rs`, uncomment:

```rust
pub use sizing::{Sizing, A1, A2, A3};
```

Also re-emit the partial re-export in `crates/antcolony-trainer/src/lib.rs`:

```rust
pub use hierarchical::{Sizing};
```

(Keep the other re-exports commented for now.)

- [ ] **Step 3: Run tests**

Run: `cargo test -p antcolony-trainer --lib hierarchical::sizing::tests 2>&1 | tail -10`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/sizing.rs crates/antcolony-trainer/src/hierarchical/mod.rs crates/antcolony-trainer/src/lib.rs
git commit -m "trainer: hierarchical Sizing struct + A1/A2/A3 presets

Const presets for A1 (smoke ~12M), A2 (8GB-deploy ~95M), A3 (cnc
teacher ~160M). FIXED_* constants pin the sim-side observation shapes
so changes to ColonyAiState / AntObservation will trip the
fixed_dims_match_phase1_sim_api test first.
"
```

---

### Task 4: `TransformerBlock` primitive

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/transformer.rs`

- [ ] **Step 1: Write the file**

Write `crates/antcolony-trainer/src/hierarchical/transformer.rs`:

```rust
//! Minimal transformer block: multi-head self-attention + post-attention
//! LayerNorm + FFN + post-FFN LayerNorm. Used by both [`crate::hierarchical::commander`]
//! and [`crate::hierarchical::ant`] backbones.
//!
//! Built from `candle_nn::Linear` + `LayerNorm` primitives — we don't
//! pull in `candle-transformers` because we control the size and don't
//! need exotic features (RoPE, KV cache, FlashAttention). On Pascal
//! sm_60 we'd lose FlashAttention anyway.

use candle_core::{Result, Tensor, D};
use candle_nn::{LayerNorm, Linear, Module, VarBuilder};

/// One transformer block: pre-norm style.
///   x = x + self_attn(LN(x))
///   x = x + ffn(LN(x))
pub struct TransformerBlock {
    pub d_model: usize,
    pub n_heads: usize,
    pub d_head: usize,

    pub norm_attn: LayerNorm,
    pub q_proj: Linear,
    pub k_proj: Linear,
    pub v_proj: Linear,
    pub o_proj: Linear,

    pub norm_ffn: LayerNorm,
    pub ffn_up: Linear,
    pub ffn_down: Linear,
}

impl TransformerBlock {
    /// Build a transformer block.
    ///
    /// - `vb` — VarBuilder rooted at this block's namespace
    /// - `d_model` — hidden dim (must be divisible by `n_heads`)
    /// - `n_heads` — number of attention heads
    /// - `d_ffn` — feed-forward inner dim (usually 4×d_model in vanilla transformers)
    pub fn new(vb: VarBuilder, d_model: usize, n_heads: usize, d_ffn: usize) -> Result<Self> {
        assert!(
            d_model % n_heads == 0,
            "d_model={} must be divisible by n_heads={}",
            d_model, n_heads,
        );
        let d_head = d_model / n_heads;

        Ok(Self {
            d_model,
            n_heads,
            d_head,
            norm_attn: candle_nn::layer_norm(d_model, 1e-5, vb.pp("norm_attn"))?,
            q_proj: candle_nn::linear(d_model, d_model, vb.pp("q_proj"))?,
            k_proj: candle_nn::linear(d_model, d_model, vb.pp("k_proj"))?,
            v_proj: candle_nn::linear(d_model, d_model, vb.pp("v_proj"))?,
            o_proj: candle_nn::linear(d_model, d_model, vb.pp("o_proj"))?,
            norm_ffn: candle_nn::layer_norm(d_model, 1e-5, vb.pp("norm_ffn"))?,
            ffn_up: candle_nn::linear(d_model, d_ffn, vb.pp("ffn_up"))?,
            ffn_down: candle_nn::linear(d_ffn, d_model, vb.pp("ffn_down"))?,
        })
    }

    /// Forward pass on `x: [B, T, d_model]`. Returns same shape.
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // Self-attention sub-block (pre-norm + residual).
        let attn_in = self.norm_attn.forward(x)?;
        let attn_out = self.self_attention(&attn_in)?;
        let x = (x + attn_out)?;

        // FFN sub-block (pre-norm + residual).
        let ffn_in = self.norm_ffn.forward(&x)?;
        let ffn_mid = self.ffn_up.forward(&ffn_in)?;
        // GELU activation. Candle has `gelu()` on Tensor.
        let ffn_mid = ffn_mid.gelu()?;
        let ffn_out = self.ffn_down.forward(&ffn_mid)?;
        let x = (x + ffn_out)?;

        Ok(x)
    }

    fn self_attention(&self, x: &Tensor) -> Result<Tensor> {
        let (b, t, _) = x.dims3()?;

        let q = self.q_proj.forward(x)?;
        let k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;

        // Reshape to [B, T, H, D/H] then transpose to [B, H, T, D/H].
        let q = q.reshape((b, t, self.n_heads, self.d_head))?.transpose(1, 2)?.contiguous()?;
        let k = k.reshape((b, t, self.n_heads, self.d_head))?.transpose(1, 2)?.contiguous()?;
        let v = v.reshape((b, t, self.n_heads, self.d_head))?.transpose(1, 2)?.contiguous()?;

        // scores = Q @ K^T / sqrt(d_head)
        let scale = 1.0 / (self.d_head as f64).sqrt();
        let scores = q.matmul(&k.transpose(D::Minus2, D::Minus1)?.contiguous()?)?;
        let scores = (scores * scale)?;

        let attn = candle_nn::ops::softmax(&scores, D::Minus1)?;
        let out = attn.matmul(&v)?;  // [B, H, T, D/H]

        // Back to [B, T, D].
        let out = out.transpose(1, 2)?.contiguous()?.reshape((b, t, self.d_model))?;
        self.o_proj.forward(&out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;

    fn cpu_vb() -> (VarMap, Device) {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        (varmap, device)
    }

    #[test]
    fn block_preserves_shape() {
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let block = TransformerBlock::new(vb, 64, 4, 128).unwrap();

        let x = Tensor::randn(0.0f32, 1.0, (2, 5, 64), &device).unwrap();
        let y = block.forward(&x).unwrap();
        assert_eq!(y.dims(), &[2, 5, 64]);
    }

    #[test]
    fn block_d_model_must_divide_by_heads() {
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        // 64 / 5 doesn't divide evenly — should panic.
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            TransformerBlock::new(vb, 64, 5, 128).unwrap()
        }));
        assert!(r.is_err(), "expected panic for non-divisible d_model/n_heads");
    }

    #[test]
    fn block_param_count_matches_estimate() {
        // 4 layers worth of (4·d² + 2·d·ffn) — but this is ONE layer so divide by 4.
        // d=128, ffn=256: per-layer = 4·128² + 2·128·256 = 65536 + 65536 = 131072.
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let _block = TransformerBlock::new(vb, 128, 4, 256).unwrap();
        // Sum the size of every f32 variable currently tracked by varmap.
        // The 4 projection matrices = 4 × 128² = 65536.
        // The 2 FFN matrices = 2 × 128 × 256 = 65536.
        // Plus biases (~negligible) and 2 LayerNorm pairs (~negligible).
        let total: usize = varmap.all_vars().iter().map(|v| v.dims().iter().product::<usize>()).sum();
        let core = 131_072; // 4·d² + 2·d·ffn
        // Allow ~10% headroom for biases and LayerNorm weights.
        assert!(
            total >= core && total <= (core as f64 * 1.15) as usize + 4096,
            "param count {} should be approximately core {} (within +10% + LN slack)",
            total, core,
        );
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p antcolony-trainer --lib hierarchical::transformer::tests 2>&1 | tail -10`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/transformer.rs
git commit -m "trainer: TransformerBlock primitive (pre-norm + MHA + FFN)

Multi-head self-attention with shape [B,T,d_model] → [B,T,d_model].
Pre-norm style: x = x + attn(LN(x)) + ffn(LN(x)). Built from
candle_nn::Linear/LayerNorm primitives; no candle-transformers dep.
GELU activation in FFN. d_model must divide by n_heads (panic on
violation). Three smoke tests verify shape preservation, division
assertion, and parameter count within 10% of (4d² + 2d·ffn).
"
```

---

### Task 5: `CommanderPolicy::new` (struct + builder, no forward yet)

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/commander.rs`

- [ ] **Step 1: Write the struct + new()**

Write `crates/antcolony-trainer/src/hierarchical/commander.rs`:

```rust
//! Commander tier policy net — outer-tick brain (one per colony).
//!
//! Inputs (per decision tick, batch dim implicit):
//!   state_17d        : f32[B, 17]                — ColonyAiState flat
//!   pheromone_field  : f32[B, 4, 32, 32]         — downsampled snapshot
//!   history_tokens   : f32[B, K=8, 96]           — last 8 commander tokens
//!
//! Outputs:
//!   action  : f32[B, 6]   — pre-tanh; squashed by caller for AiDecision
//!   intent  : f32[B, 64]  — broadcast to ant tier this decision window
//!   value   : f32[B]      — V(s) for PPO critic
//!   log_std : f32[6]      — Gaussian policy std (learnable parameter)
//!
//! Backbone (A1 dims shown; A2/A3 scale per Sizing presets):
//!   pheromone_encoder : Conv2d(4→32, k=3) → ReLU → Conv2d(32→64, k=3, s=2) → ReLU → AvgPool2d → Linear(→192)
//!   state_encoder     : Linear(17 → 192)
//!   history_encoder   : Linear(96 → 192)   (applied per-token)
//!   concat → Linear(192 → d_model=384) → [1+1+K=10 tokens]
//!   transformer       : L=4 layers, d=384, heads=6, ffn=1536
//!   pool              : learned [CLS]-style first-token output → 384
//!   heads             : action(384→6), intent(384→64), value(384→1)

use candle_core::{DType, Device, Result, Tensor};
use candle_nn::{Conv2d, Linear, Module, VarBuilder};

use crate::hierarchical::sizing::Sizing;
use crate::hierarchical::transformer::TransformerBlock;

pub struct CommanderPolicy {
    pub sizing: Sizing,

    // Pheromone CNN
    pub pher_conv1: Conv2d,
    pub pher_conv2: Conv2d,
    pub pher_proj: Linear,

    // Token encoders (each produces a single d_model-dim token)
    pub state_encoder: Linear,
    pub history_encoder: Linear,

    // Reproject 3 streams' encoder outputs from cmdr_encoder_dim → d_model
    pub stream_proj: Linear,

    // Learned [CLS]-style token prepended to the sequence
    pub cls_token: Tensor,

    // Transformer backbone
    pub blocks: Vec<TransformerBlock>,

    // Heads
    pub action_head: Linear,
    pub intent_head: Linear,
    pub value_head: Linear,

    // Learnable per-dim policy std
    pub log_std: Tensor,
}

impl CommanderPolicy {
    pub fn new(vb: VarBuilder, sizing: Sizing) -> Result<Self> {
        let d_enc = sizing.cmdr_encoder_dim;
        let d_model = sizing.cmdr_d_model;

        // Pheromone CNN — Conv2d(4 → mid, k=3, pad=1) → Conv2d(mid → hi, k=3, stride=2) → AvgPool2d(2) → Flatten → Linear(→d_enc)
        let conv_cfg_1 = candle_nn::Conv2dConfig { padding: 1, stride: 1, ..Default::default() };
        let conv_cfg_2 = candle_nn::Conv2dConfig { padding: 1, stride: 2, ..Default::default() };
        let pher_conv1 = candle_nn::conv2d(
            sizing.fixed_pheromone_c, sizing.cmdr_pheromone_channels_mid, 3, conv_cfg_1,
            vb.pp("pher_conv1"),
        )?;
        let pher_conv2 = candle_nn::conv2d(
            sizing.cmdr_pheromone_channels_mid, sizing.cmdr_pheromone_channels_hi, 3, conv_cfg_2,
            vb.pp("pher_conv2"),
        )?;
        // After conv1 (32→32×32×mid), conv2 stride=2 (→16×16×hi), AvgPool2d(2) (→8×8×hi). Flatten → 8·8·hi.
        let pher_flat_in = 8 * 8 * sizing.cmdr_pheromone_channels_hi;
        let pher_proj = candle_nn::linear(pher_flat_in, d_enc, vb.pp("pher_proj"))?;

        let state_encoder = candle_nn::linear(sizing.fixed_state_d, d_enc, vb.pp("state_encoder"))?;
        let history_encoder = candle_nn::linear(sizing.fixed_history_tok_d, d_enc, vb.pp("history_encoder"))?;

        let stream_proj = candle_nn::linear(d_enc, d_model, vb.pp("stream_proj"))?;

        // Learnable CLS token shape [1, 1, d_model]. Initialized to zeros + small noise.
        let cls_token = vb.get((1, 1, d_model), "cls_token")?;

        let mut blocks = Vec::with_capacity(sizing.cmdr_layers);
        for i in 0..sizing.cmdr_layers {
            blocks.push(TransformerBlock::new(
                vb.pp(&format!("block_{i}")),
                d_model,
                sizing.cmdr_heads,
                sizing.cmdr_ffn,
            )?);
        }

        let action_head = candle_nn::linear(d_model, sizing.fixed_action_d, vb.pp("action_head"))?;
        let intent_head = candle_nn::linear(d_model, sizing.fixed_intent_d, vb.pp("intent_head"))?;
        let value_head = candle_nn::linear(d_model, 1, vb.pp("value_head"))?;

        // log_std is a learnable parameter shape [fixed_action_d], initialized to -1.0.
        let log_std = vb.get_with_hints(
            sizing.fixed_action_d,
            "log_std",
            candle_nn::Init::Const(-1.0),
        )?;

        Ok(Self {
            sizing,
            pher_conv1, pher_conv2, pher_proj,
            state_encoder, history_encoder,
            stream_proj, cls_token,
            blocks,
            action_head, intent_head, value_head,
            log_std,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_nn::VarMap;
    use crate::hierarchical::sizing::A1;

    #[test]
    fn a1_commander_builds() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let policy = CommanderPolicy::new(vb, A1).unwrap();
        assert_eq!(policy.blocks.len(), A1.cmdr_layers);
        assert_eq!(policy.sizing.cmdr_d_model, 384);
    }

    #[test]
    fn a1_commander_param_count_is_in_ballpark() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let _ = CommanderPolicy::new(vb, A1).unwrap();
        let total: usize = varmap.all_vars().iter().map(|v| v.dims().iter().product::<usize>()).sum();
        // A1 commander total spec ≈ 9M (transformer ~7M + encoders/heads ~2M).
        // Allow a wide band; tighter checks happen in the forward-shape tests.
        assert!(total >= 5_000_000 && total <= 15_000_000,
            "A1 commander total params ~9M expected, got {}", total);
    }
}
```

- [ ] **Step 2: Uncomment the commander re-export in mod.rs**

Uncomment in `crates/antcolony-trainer/src/hierarchical/mod.rs`:

```rust
pub use commander::CommanderPolicy;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p antcolony-trainer --lib hierarchical::commander::tests 2>&1 | tail -10`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/commander.rs crates/antcolony-trainer/src/hierarchical/mod.rs
git commit -m "trainer: CommanderPolicy::new (struct + builder, no forward yet)

Builds the pheromone CNN, state/history encoders, stream reprojection,
[CLS] token, L-layer transformer, and 3 heads + log_std parameter.
A1 size builds and reports ~5-15M total params (within ballpark of the
~9M A1 commander spec). Forward pass lands in Task 6.
"
```

---

### Task 6: `CommanderPolicy::forward`

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/commander.rs`

- [ ] **Step 1: Write the test**

Add to `crates/antcolony-trainer/src/hierarchical/commander.rs`'s `#[cfg(test)] mod tests`:

```rust
#[test]
fn a1_commander_forward_shapes() {
    let varmap = VarMap::new();
    let device = Device::Cpu;
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let policy = CommanderPolicy::new(vb, A1).unwrap();

    let b = 2usize;
    let state = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_state_d), &device).unwrap();
    let pheromone = Tensor::randn(
        0.0f32, 1.0,
        (b, A1.fixed_pheromone_c, A1.fixed_pheromone_h, A1.fixed_pheromone_w),
        &device,
    ).unwrap();
    let history = Tensor::randn(
        0.0f32, 1.0,
        (b, A1.fixed_history_k, A1.fixed_history_tok_d),
        &device,
    ).unwrap();

    let out = policy.forward(&state, &pheromone, &history).unwrap();
    assert_eq!(out.action.dims(), &[b, A1.fixed_action_d]);
    assert_eq!(out.intent.dims(), &[b, A1.fixed_intent_d]);
    assert_eq!(out.value.dims(), &[b]);
}
```

- [ ] **Step 2: Add the `CommanderForwardOut` struct + `forward` method**

In `crates/antcolony-trainer/src/hierarchical/commander.rs`, add:

```rust
/// Bundle of forward-pass outputs from CommanderPolicy.
pub struct CommanderForwardOut {
    pub action: Tensor,   // [B, 6] — pre-tanh
    pub intent: Tensor,   // [B, 64]
    pub value: Tensor,    // [B]
}

impl CommanderPolicy {
    pub fn forward(
        &self,
        state: &Tensor,      // [B, 17]
        pheromone: &Tensor,  // [B, 4, 32, 32]
        history: &Tensor,    // [B, K=8, 96]
    ) -> Result<CommanderForwardOut> {
        let (b, _) = state.dims2()?;
        let d_enc = self.sizing.cmdr_encoder_dim;
        let d_model = self.sizing.cmdr_d_model;

        // ── Pheromone CNN ──
        // Conv2d → ReLU → Conv2d → ReLU → AvgPool2d(2) → Flatten → Linear
        let p = self.pher_conv1.forward(pheromone)?.relu()?;
        let p = self.pher_conv2.forward(&p)?.relu()?;
        // AvgPool2d kernel=2, stride=2 → halves spatial dims.
        let p = p.avg_pool2d((2, 2))?;
        let p = p.flatten_from(1)?;  // [B, 8*8*hi]
        let pher_tok = self.pher_proj.forward(&p)?;  // [B, d_enc]

        // ── State encoder ──
        let state_tok = self.state_encoder.forward(state)?;  // [B, d_enc]

        // ── History encoder (per-token) ──
        // history: [B, K, 96] → reshape to [B*K, 96] → Linear → [B*K, d_enc] → reshape [B, K, d_enc]
        let (b_h, k, _) = history.dims3()?;
        debug_assert_eq!(b_h, b);
        let h_flat = history.reshape((b * k, self.sizing.fixed_history_tok_d))?;
        let h_enc = self.history_encoder.forward(&h_flat)?;
        let history_toks = h_enc.reshape((b, k, d_enc))?;  // [B, K, d_enc]

        // ── Stack tokens [pher, state, history_0..K-1] then reproject to d_model ──
        // pher_tok and state_tok need a token dim. Unsqueeze to [B, 1, d_enc].
        let pher_tok = pher_tok.unsqueeze(1)?;    // [B, 1, d_enc]
        let state_tok = state_tok.unsqueeze(1)?;  // [B, 1, d_enc]
        let concat = Tensor::cat(&[&pher_tok, &state_tok, &history_toks], 1)?;  // [B, 2+K, d_enc]
        let tokens = self.stream_proj.forward(&concat)?;  // [B, 2+K, d_model]

        // ── Prepend learnable CLS token: [1, 1, d_model] expanded to [B, 1, d_model] ──
        let cls = self.cls_token.expand((b, 1, d_model))?;
        let mut x = Tensor::cat(&[&cls, &tokens], 1)?;  // [B, 1+2+K, d_model]

        // ── Transformer backbone ──
        for block in &self.blocks {
            x = block.forward(&x)?;
        }

        // ── Pool: take the CLS-position output (index 0). ──
        let cls_out = x.i((.., 0, ..))?;  // [B, d_model]

        // ── Heads ──
        let action = self.action_head.forward(&cls_out)?;        // [B, 6] — pre-tanh
        let intent = self.intent_head.forward(&cls_out)?;        // [B, 64]
        let value = self.value_head.forward(&cls_out)?.squeeze(1)?;  // [B]

        Ok(CommanderForwardOut { action, intent, value })
    }
}
```

You'll need `use candle_core::IndexOp;` at the top of the file for the `.i(..)` slicing.

- [ ] **Step 3: Run the test**

Run: `cargo test -p antcolony-trainer --lib hierarchical::commander::tests::a1_commander_forward_shapes 2>&1 | tail -10`
Expected: PASS. If you get a shape mismatch, the most likely culprit is the AvgPool2d output dims — verify with a `dbg!(p.dims())` after the avg_pool2d call. The math is: input 32×32, conv1 stride=1 padding=1 → 32×32, conv2 stride=2 padding=1 → 16×16, avg_pool2d kernel=2 → 8×8.

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/commander.rs
git commit -m "trainer: CommanderPolicy::forward with shape-correct backbone

CNN over pheromone field → flatten + project → stack [pher, state, K
history tokens] → prepend CLS → L transformer blocks → pool CLS → 3
heads. Shape test verifies action [B,6], intent [B,64], value [B]
outputs at A1 size with batch=2.
"
```

---

### Task 7: `AntPolicy` struct + new + forward

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/ant.rs`

- [ ] **Step 1: Write the full module**

Write `crates/antcolony-trainer/src/hierarchical/ant.rs`:

```rust
//! Ant tier policy net — per-ant brain (one shared instance per colony,
//! evaluated once per ant per tick, batched).
//!
//! Inputs:
//!   cone   : f32[B, 60]   — AntObservation.pheromone_cone
//!   intern : f32[B, 8]    — AntObservation.internal
//!   intent : f32[B, 64]   — broadcast from commander (same value for all ants)
//!
//! Outputs:
//!   modulator : f32[B, 5]  — pre-squash; trainer applies tanh/sigmoid per field
//!   value     : f32[B]     — local critic for ant-tier GAE (Phase 2b)
//!   log_std   : f32[5]     — learnable per-dim std

use candle_core::{DType, Device, IndexOp, Result, Tensor};
use candle_nn::{Linear, Module, VarBuilder};

use crate::hierarchical::sizing::Sizing;
use crate::hierarchical::transformer::TransformerBlock;

pub struct AntPolicy {
    pub sizing: Sizing,

    pub cone_encoder1: Linear,
    pub cone_encoder2: Linear,
    pub state_encoder: Linear,
    pub intent_encoder: Linear,

    // Reproject concatenated [cone, internal, intent] streams to d_model
    pub stream_proj: Linear,

    pub blocks: Vec<TransformerBlock>,

    pub modulator_head: Linear,
    pub value_head: Linear,
    pub log_std: Tensor,
}

pub struct AntForwardOut {
    pub modulator: Tensor,  // [B, 5] — pre-squash
    pub value: Tensor,      // [B]
}

impl AntPolicy {
    pub fn new(vb: VarBuilder, sizing: Sizing) -> Result<Self> {
        let d_model = sizing.ant_d_model;

        let cone_encoder1 = candle_nn::linear(sizing.fixed_cone_d, sizing.ant_cone_hidden, vb.pp("cone_encoder1"))?;
        let cone_encoder2 = candle_nn::linear(sizing.ant_cone_hidden, sizing.ant_cone_hidden, vb.pp("cone_encoder2"))?;
        let state_encoder = candle_nn::linear(sizing.fixed_internal_d, sizing.ant_internal_hidden, vb.pp("state_encoder"))?;
        let intent_encoder = candle_nn::linear(sizing.fixed_intent_d, sizing.ant_intent_hidden, vb.pp("intent_encoder"))?;

        let concat_dim = sizing.ant_cone_hidden + sizing.ant_internal_hidden + sizing.ant_intent_hidden;
        let stream_proj = candle_nn::linear(concat_dim, d_model, vb.pp("stream_proj"))?;

        let mut blocks = Vec::with_capacity(sizing.ant_layers);
        for i in 0..sizing.ant_layers {
            blocks.push(TransformerBlock::new(
                vb.pp(&format!("block_{i}")),
                d_model,
                sizing.ant_heads,
                sizing.ant_ffn,
            )?);
        }

        let modulator_head = candle_nn::linear(d_model, sizing.fixed_modulator_d, vb.pp("modulator_head"))?;
        let value_head = candle_nn::linear(d_model, 1, vb.pp("value_head"))?;

        let log_std = vb.get_with_hints(
            sizing.fixed_modulator_d,
            "log_std",
            candle_nn::Init::Const(-1.0),
        )?;

        Ok(Self {
            sizing,
            cone_encoder1, cone_encoder2,
            state_encoder, intent_encoder,
            stream_proj,
            blocks,
            modulator_head, value_head,
            log_std,
        })
    }

    pub fn forward(&self, cone: &Tensor, internal: &Tensor, intent: &Tensor) -> Result<AntForwardOut> {
        // ── Encoders ──
        let cone_h = self.cone_encoder1.forward(cone)?.relu()?;
        let cone_h = self.cone_encoder2.forward(&cone_h)?;

        let state_h = self.state_encoder.forward(internal)?;
        let intent_h = self.intent_encoder.forward(intent)?;

        // ── Concatenate along feature dim ──
        let combined = Tensor::cat(&[&cone_h, &state_h, &intent_h], 1)?;  // [B, concat_dim]
        let projected = self.stream_proj.forward(&combined)?;             // [B, d_model]

        // The ant tier is a single-token sequence — the transformer adds inter-feature
        // mixing per layer via attention with T=1. Effectively it's a residual MLP
        // with attention "noop" but the structure is reusable when we later expand
        // to multi-token inputs (e.g., temporal history).
        let mut x = projected.unsqueeze(1)?;  // [B, 1, d_model]
        for block in &self.blocks {
            x = block.forward(&x)?;
        }
        let pooled = x.i((.., 0, ..))?;  // [B, d_model]

        let modulator = self.modulator_head.forward(&pooled)?;       // [B, 5]
        let value = self.value_head.forward(&pooled)?.squeeze(1)?;   // [B]

        Ok(AntForwardOut { modulator, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_nn::VarMap;
    use crate::hierarchical::sizing::A1;

    fn cpu_vb() -> (VarMap, Device) {
        (VarMap::new(), Device::Cpu)
    }

    #[test]
    fn a1_ant_builds() {
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let policy = AntPolicy::new(vb, A1).unwrap();
        assert_eq!(policy.blocks.len(), A1.ant_layers);
        assert_eq!(policy.sizing.ant_d_model, 256);
    }

    #[test]
    fn a1_ant_param_count_is_in_ballpark() {
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let _ = AntPolicy::new(vb, A1).unwrap();
        let total: usize = varmap.all_vars().iter().map(|v| v.dims().iter().product::<usize>()).sum();
        // A1 ant ≈ 3M total. Allow 1-6M band.
        assert!(total >= 1_000_000 && total <= 6_000_000,
            "A1 ant total params ~3M expected, got {}", total);
    }

    #[test]
    fn a1_ant_forward_shapes() {
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let policy = AntPolicy::new(vb, A1).unwrap();

        let b = 7usize;  // 7 ants in a colony
        let cone = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_cone_d), &device).unwrap();
        let intern = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_internal_d), &device).unwrap();
        let intent = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_intent_d), &device).unwrap();

        let out = policy.forward(&cone, &intern, &intent).unwrap();
        assert_eq!(out.modulator.dims(), &[b, A1.fixed_modulator_d]);
        assert_eq!(out.value.dims(), &[b]);
    }
}
```

- [ ] **Step 2: Uncomment the ant re-export in mod.rs**

In `crates/antcolony-trainer/src/hierarchical/mod.rs`:

```rust
pub use ant::AntPolicy;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p antcolony-trainer --lib hierarchical::ant::tests 2>&1 | tail -10`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/ant.rs crates/antcolony-trainer/src/hierarchical/mod.rs
git commit -m "trainer: AntPolicy with shape-correct backbone

Per-ant brain — 3 encoders (cone/internal/intent) → concat → stream
projection → L transformer blocks (single-token sequence) → 2 heads
(modulator [B,5], value [B]). A1 builds at ~1-6M params, forward
shapes verified with batch=7 (mimicking 7 ants in a colony).
"
```

---

### Task 8: `HierarchicalActorCritic` composes both tiers

**Files:**
- Modify: `crates/antcolony-trainer/src/hierarchical/actor_critic.rs`

- [ ] **Step 1: Write the module**

Write `crates/antcolony-trainer/src/hierarchical/actor_critic.rs`:

```rust
//! HierarchicalActorCritic — composes CommanderPolicy + AntPolicy under
//! a single builder so rollout/training code holds one object.
//!
//! Variable namespacing under the shared VarBuilder:
//!   commander.* → CommanderPolicy variables
//!   ant.*       → AntPolicy variables
//!
//! Phase 2b will add rollout and PPO-update methods that drive both
//! tiers from the joint trainer. Phase 2a just builds the composition.

use candle_core::{Result, Tensor};
use candle_nn::VarBuilder;

use crate::hierarchical::ant::{AntForwardOut, AntPolicy};
use crate::hierarchical::commander::{CommanderForwardOut, CommanderPolicy};
use crate::hierarchical::sizing::Sizing;

pub struct HierarchicalActorCritic {
    pub commander: CommanderPolicy,
    pub ant: AntPolicy,
    pub sizing: Sizing,
}

impl HierarchicalActorCritic {
    pub fn new(vb: VarBuilder, sizing: Sizing) -> Result<Self> {
        let commander = CommanderPolicy::new(vb.pp("commander"), sizing)?;
        let ant = AntPolicy::new(vb.pp("ant"), sizing)?;
        Ok(Self { commander, ant, sizing })
    }

    /// Forward through the commander tier only. Convenience wrapper.
    pub fn forward_commander(
        &self,
        state: &Tensor,
        pheromone: &Tensor,
        history: &Tensor,
    ) -> Result<CommanderForwardOut> {
        self.commander.forward(state, pheromone, history)
    }

    /// Forward through the ant tier only. Convenience wrapper.
    pub fn forward_ant(
        &self,
        cone: &Tensor,
        internal: &Tensor,
        intent: &Tensor,
    ) -> Result<AntForwardOut> {
        self.ant.forward(cone, internal, intent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;
    use crate::hierarchical::sizing::A1;

    #[test]
    fn a1_hac_builds() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
        assert_eq!(hac.commander.blocks.len(), A1.cmdr_layers);
        assert_eq!(hac.ant.blocks.len(), A1.ant_layers);
    }

    #[test]
    fn a1_hac_total_param_count_is_sum_of_tiers() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let _ = HierarchicalActorCritic::new(vb, A1).unwrap();
        let total: usize = varmap.all_vars().iter().map(|v| v.dims().iter().product::<usize>()).sum();
        // A1 total ≈ 12M (9M commander + 3M ant). Wide band.
        assert!(total >= 6_000_000 && total <= 20_000_000,
            "A1 HAC total params ~12M expected, got {}", total);
    }
}
```

- [ ] **Step 2: Uncomment the HAC re-export in mod.rs**

In `crates/antcolony-trainer/src/hierarchical/mod.rs`:

```rust
pub use actor_critic::HierarchicalActorCritic;
```

And in `crates/antcolony-trainer/src/lib.rs`, expand the re-export:

```rust
pub use hierarchical::{HierarchicalActorCritic, CommanderPolicy, AntPolicy, Sizing};
```

- [ ] **Step 3: Run tests + workspace build**

Run: `cargo test -p antcolony-trainer --lib hierarchical 2>&1 | tail -10`
Expected: all hierarchical tests pass (sizing 3 + transformer 3 + commander 3 + ant 3 + hac 2 = 14 total).

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/src/hierarchical/actor_critic.rs crates/antcolony-trainer/src/hierarchical/mod.rs crates/antcolony-trainer/src/lib.rs
git commit -m "trainer: HierarchicalActorCritic composes both tiers

Single builder under shared VarBuilder; commander.* and ant.* namespaces
keep the trainer's optimizer state clean. forward_commander +
forward_ant convenience wrappers proxy to the tier policies. A1 builds
at ~6-20M total params.
"
```

---

### Task 9: End-to-end smoke test — drive HAC from a fresh `Simulation`

**Files:**
- Create: `crates/antcolony-trainer/tests/hierarchical_smoke.rs`

This task is the integration check: the policy nets must accept tensors derived from `RichObservation` and `AntObservation` (the Phase 1 carrier types) without shape mismatches.

- [ ] **Step 1: Write the integration test**

Create `crates/antcolony-trainer/tests/hierarchical_smoke.rs`:

```rust
//! Phase 2a end-to-end smoke: build a fresh Simulation, collect a
//! RichObservation + per-ant AntObservations, build a HierarchicalActorCritic
//! at A1 size, and run forward through both tiers.
//!
//! No training, no gradients — just shape + numerics correctness. If this
//! passes, Phase 2a's plumbing is end-to-end correct and Phase 2b can
//! layer PPO on top.

use candle_core::{DType, Device, Tensor};
use candle_nn::{VarBuilder, VarMap};

use antcolony_sim::ai::observation::{AntObservation, RichObservation};
use antcolony_sim::config::{
    AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig,
};
use antcolony_sim::{Simulation, Topology};

use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::HierarchicalActorCritic;

fn build_sim() -> Simulation {
    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 10, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2)
}

/// Convert one RichObservation to (state, pheromone, history) tensors with batch=1.
fn rich_to_tensors(rich: &RichObservation, device: &Device) -> (Tensor, Tensor, Tensor) {
    use antcolony_sim::ai::brain::ColonyAiState;
    // 17-d state vector — match the layout in antcolony-trainer/src/backend.rs::state_to_tensor.
    let s = &rich.state;
    let ed = if s.enemy_distance_min.is_finite() { s.enemy_distance_min } else { 1e6 };
    let state_v: Vec<f32> = vec![
        s.food_stored, s.food_inflow_recent,
        s.worker_count as f32, s.soldier_count as f32, s.breeder_count as f32,
        s.brood_egg as f32, s.brood_larva as f32, s.brood_pupa as f32,
        s.queens_alive as f32, s.combat_losses_recent as f32,
        ed, s.enemy_worker_count as f32, s.enemy_soldier_count as f32,
        s.day_of_year as f32, s.ambient_temp_c,
        if s.diapause_active { 1.0 } else { 0.0 },
        if s.is_daytime { 1.0 } else { 0.0 },
    ];
    debug_assert_eq!(state_v.len(), 17);
    let state = Tensor::from_vec(state_v, (1, 17), device).unwrap();

    // Pheromone field: [1, 4, 32, 32] from 4 Box<[f32]> channels each length 32*32.
    let p = &rich.pheromone_field;
    let mut pher_v: Vec<f32> = Vec::with_capacity(4 * 32 * 32);
    pher_v.extend_from_slice(&p.food_trail);
    pher_v.extend_from_slice(&p.home_trail);
    pher_v.extend_from_slice(&p.alarm);
    pher_v.extend_from_slice(&p.colony_scent);
    let pheromone = Tensor::from_vec(pher_v, (1, 4, 32, 32), device).unwrap();

    // History tokens: pad to K=8 with zero tokens if the colony's ring has fewer.
    // Token layout: 17 + 6 + 1 + 72 = 96 floats.
    let mut hist_v: Vec<f32> = Vec::with_capacity(8 * 96);
    for tok in rich.history.iter() {
        hist_v.extend_from_slice(&tok.state);
        hist_v.extend_from_slice(&tok.action);
        hist_v.push(tok.reward);
        hist_v.extend_from_slice(&tok.pad);
    }
    // Pad to full K=8 tokens.
    while hist_v.len() < 8 * 96 {
        hist_v.push(0.0);
    }
    let history = Tensor::from_vec(hist_v, (1, 8, 96), device).unwrap();

    (state, pheromone, history)
}

/// Convert a Vec<AntObservation> to (cone, internal, intent) tensors batched along ants.
/// All ants in the colony share the same intent (broadcast from commander) — we tile it.
fn ant_obs_to_tensors(
    obs: &[AntObservation],
    intent_per_ant: &Tensor,
    device: &Device,
) -> (Tensor, Tensor, Tensor) {
    let b = obs.len();
    let mut cone_v: Vec<f32> = Vec::with_capacity(b * 60);
    let mut internal_v: Vec<f32> = Vec::with_capacity(b * 8);
    for o in obs {
        cone_v.extend_from_slice(&o.pheromone_cone);
        internal_v.extend_from_slice(&o.internal);
    }
    let cone = Tensor::from_vec(cone_v, (b, 60), device).unwrap();
    let internal = Tensor::from_vec(internal_v, (b, 8), device).unwrap();
    // Broadcast intent: [1, 64] → [b, 64]
    let intent = intent_per_ant.broadcast_as((b, 64)).unwrap();
    (cone, internal, intent)
}

#[test]
fn a1_hac_drives_from_fresh_sim() {
    let device = Device::Cpu;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let hac = HierarchicalActorCritic::new(vb, A1).unwrap();

    let sim = build_sim();

    // Commander forward
    let rich = sim.colony_rich_observation(0).expect("colony 0 exists");
    let (state, pheromone, history) = rich_to_tensors(&rich, &device);
    let cmdr_out = hac.forward_commander(&state, &pheromone, &history).unwrap();
    assert_eq!(cmdr_out.action.dims(), &[1, 6]);
    assert_eq!(cmdr_out.intent.dims(), &[1, 64]);
    assert_eq!(cmdr_out.value.dims(), &[1]);

    // Ant forward (broadcast commander intent to each ant)
    let ant_obs = sim.per_ant_observations(0);
    assert!(ant_obs.len() >= 1, "expected ants in colony 0");
    let (cone, internal, intent_b) = ant_obs_to_tensors(&ant_obs, &cmdr_out.intent, &device);
    let ant_out = hac.forward_ant(&cone, &internal, &intent_b).unwrap();
    assert_eq!(ant_out.modulator.dims(), &[ant_obs.len(), 5]);
    assert_eq!(ant_out.value.dims(), &[ant_obs.len()]);

    // Numerics sanity: nothing is NaN/Inf.
    let action_v: Vec<f32> = cmdr_out.action.flatten_all().unwrap().to_vec1().unwrap();
    assert!(action_v.iter().all(|v| v.is_finite()),
        "commander action contained non-finite values: {:?}", action_v);
    let mod_v: Vec<f32> = ant_out.modulator.flatten_all().unwrap().to_vec1().unwrap();
    assert!(mod_v.iter().all(|v| v.is_finite()),
        "ant modulator contained non-finite values: {:?}", mod_v);
}
```

- [ ] **Step 2: Run the integration test**

Run: `cargo test -p antcolony-trainer --test hierarchical_smoke 2>&1 | tail -15`
Expected: PASS.

If you hit shape mismatches:
- `state` should be `[1, 17]` — verify the state_v Vec has 17 entries.
- `pheromone` should be `[1, 4, 32, 32]` — verify the 4 layer slices are each 32×32=1024 floats.
- `history` should be `[1, 8, 96]` — for a fresh sim, `rich.history` is empty, so the test pads with zeros to fill K=8 tokens.

If numerics blow up (NaN/Inf):
- Check that LayerNorm eps is non-zero (we set 1e-5 in TransformerBlock).
- Check that random input tensors aren't extreme; Tensor::randn with σ=1 should be fine.

- [ ] **Step 3: Commit**

```bash
cd J:/antcolony
git add crates/antcolony-trainer/tests/hierarchical_smoke.rs
git commit -m "trainer: end-to-end smoke — HAC drives from fresh Simulation

Builds Phase-1 sim, pulls RichObservation + AntObservation via the
Phase-1 API, feeds tensors through CommanderPolicy and AntPolicy at A1
size on CPU. Asserts output tensor shapes and finite-only numerics.

If this passes, Phase 2a is end-to-end correct. Phase 2b can layer
joint PPO on top.
"
```

---

### Task 10: Phase 2a acceptance — full sweep + HANDOFF update

**Files:** verification + HANDOFF.md.

- [ ] **Step 1: Run the full workspace test suite**

Run: `cargo test --workspace 2>&1 | tail -10`
Expected: All tests pass across `antcolony-sim` (165+ now with the deposit_mult test), `antcolony-trainer` (all hierarchical unit + smoke integration), `antcolony-game`, `antcolony-render`. If any test outside Phase 2a's scope fails, STOP and report as `BLOCKED`.

- [ ] **Step 2: Run clippy on antcolony-trainer**

Run: `cargo clippy -p antcolony-trainer --lib --tests -- -D warnings 2>&1 | tail -15`
Expected: NO warnings on Phase 2a code. Pre-existing warnings (if any) in the existing `policy.rs` / `ppo.rs` / `env.rs` are allowed — Phase 2a should add ZERO new clippy errors. If clippy flags Phase 2a code (`hierarchical/*` or `hierarchical_smoke.rs`), fix inline.

- [ ] **Step 3: Confirm workspace still builds**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 4: Update HANDOFF.md**

Append a new session entry at the top of `J:/antcolony/HANDOFF.md` (above the existing 2026-05-19 entry):

```markdown
## Session <today> — Phase 2a hierarchical policy forward pass landed

🟢 Project Status: **Phase 2a ship-ready.** Forward-only hierarchical policy nets (`CommanderPolicy`, `AntPolicy`, `HierarchicalActorCritic`) in `crates/antcolony-trainer/src/hierarchical/`. A1 sizing target (~12M params) builds and runs forward on CPU; A2 and A3 dim presets defined but only smoke-tested at the param-count level. End-to-end smoke (`tests/hierarchical_smoke.rs`) drives the HAC from a fresh `Simulation`'s RichObservation + per-ant AntObservations and asserts output tensor shapes + finite-only numerics. Sim-side `deposit_mult` modulator wired into pheromone deposit math. Existing flat `ActorCritic` MLP untouched (47% Nash regression baseline preserved).

### What's Next

- Phase 2b plan: `JointPpoTrainer` joint loss + per-tier GAE + first 5-iter smoke training run + `state_bias` wiring into FSM transition logits + `MatchEnv` extensions.
- Optional cleanup: 33 pre-existing clippy errors still pending sweep.

### Notes for Next Session

- Phase 2a is **forward-only**. No backward, no PPO, no MatchEnv extensions yet — those land in Phase 2b.
- The Phase 2a HAC builds on CPU at A1 size. CUDA path is untested in this phase (kokonoe 3070 Ti or cnc P100s); will be exercised in Phase 2b's first training run.
- `state_bias` is still stored-but-unused on the sim side. Phase 2b wires it into one specific FSM transition (likely `Exploring → FollowingTrail` — implementer should locate the FSM transition decision site and inject the logit bias there).
- The integration test in `hierarchical_smoke.rs` duplicates the `state_to_tensor` layout from `antcolony-trainer/src/backend.rs::state_to_tensor`. Phase 2b should DRY this — extract a shared helper.
```

- [ ] **Step 5: Commit**

```bash
cd J:/antcolony
git add HANDOFF.md
git commit -m "handoff: phase 2a hierarchical policy forward pass complete

Forward-only CommanderPolicy + AntPolicy + HierarchicalActorCritic at A1
size. Sim-side deposit_mult wired. End-to-end smoke passes. Next:
Phase 2b — joint PPO + state_bias + first training run.
"
```

---

## Acceptance criteria (recap)

Phase 2a is **done** when ALL of the following are true:

1. `cargo test --workspace` passes (no regressions).
2. `cargo clippy -p antcolony-trainer --lib --tests -- -D warnings` passes for Phase 2a code (existing `policy.rs` / `ppo.rs` / `env.rs` clippy state unchanged).
3. `cargo build --workspace` builds clean.
4. `hierarchical_smoke::a1_hac_drives_from_fresh_sim` passes — proves the HAC accepts Phase 1's observation carriers.
5. `deposit_mult_strengthens_pheromone_deposition` passes — proves the new sim wiring.
6. `defaults_reproduce_baseline_population_trajectory` still passes — proves Phase 2a's sim change doesn't break Phase 1's regression invariant.
7. `crates/antcolony-trainer/src/policy.rs` (the existing `ActorCritic`) is unchanged — the 47% Nash baseline is preserved.

If any criterion fails, the corresponding task gets reopened.

---

## Out-of-scope for Phase 2a (deferred to Phase 2b or later)

- **`state_bias` FSM wiring** — the modulator flows through `apply_ant_modulators` and is clamped, but no FSM transition reads it yet. Phase 2b implementer must locate the right transition site (likely `Exploring → FollowingTrail`) and inject the logit bias.
- **Joint PPO trainer** — `JointPpoTrainer`, GAE per tier, joint loss, rollout buffer per cadence.
- **`MatchEnv` extensions** — methods to fetch rich + per-ant obs in trainer-friendly batched form; apply modulators back to sim.
- **Multi-GPU rollout/train split** — Phase 3 territory.
- **CUDA path verification** — first GPU run is in Phase 2b. Phase 2a is CPU-only.
- **First training run** — Phase 2b's deliverable.
- **A3 cnc training** — Phase 4.
- **33 pre-existing clippy errors** — separate cleanup sweep.

---

## Open questions / known unknowns

- **`candle-nn` `Conv2dConfig::padding` semantics on edge cases.** The plan assumes `padding: 1` keeps spatial dims the same with `kernel=3, stride=1`. If candle treats padding differently (e.g., as a tuple) the test will fail and the implementer adjusts.
- **`VarBuilder::get_with_hints` API stability.** Used for `log_std` initialization with `Init::Const(-1.0)`. The exact API varies across candle versions. If it's renamed or moved, fall back to `vb.get(N, "log_std").unwrap_or_else(|_| Tensor::full(-1.0_f32, N, device).unwrap())` per the pattern in the existing `policy.rs::ActorCritic::new`.
- **GELU vs SiLU vs ReLU in the FFN.** GELU chosen as a modern transformer default. If kernel costs are a concern on Pascal (likely fine for A1), switch to ReLU. The choice doesn't affect Phase 2a tests.
- **`expand` vs `broadcast_as` for the CLS token.** Both work; `expand` is the idiomatic candle call. If shape errors surface, try `broadcast_as` instead.
- **`Tensor::cat` along the token dim.** Used in commander forward to stack pheromone, state, and history tokens. Verify the call signature in current candle (some versions take `&[&Tensor]`, others take a `Vec<Tensor>`).

These are not blockers — they're places where the implementer may need a small adjustment if the candle API has drifted.

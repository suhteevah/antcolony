//! AI brain trait — pluggable per-colony decision policies.
//!
//! # Why a trait
//!
//! The pre-Phase-B AI was a single hardcoded heuristic baked into
//! `Simulation::red_ai_tick` — fine for "AI vs human" but not extensible.
//! For AI-vs-AI matchups, learned policies (Aether LM checkpoints), and
//! eventually external-model integration, we need a clean swap point.
//!
//! Three implementations ship here:
//! - [`HeuristicBrain`] — the original `red_ai_tick` logic refactored as
//!   a brain. Baseline; deterministic; no model.
//! - [`RandomBrain`] — uniformly random valid decisions. Useful as the
//!   noise-floor opponent in matchup benches and as a smoke test for
//!   the trait itself.
//! - [`AetherLmBrain`] — *stub*. Loads an Aether-LM checkpoint and runs
//!   inference per decision tick. Integration point exists; the actual
//!   model call is `todo!()` until we have a trained checkpoint.
//!
//! # Decision cadence
//!
//! Brains decide once per outer tick (not per substep). The
//! [`AiDecision`] is then applied to the colony's caste ratio + behavior
//! weights + tech-unlock pipeline.

use crate::colony::TechUnlock;

/// Compact game-state snapshot the brain consumes per decision tick.
/// Fixed-size feature vector — model-friendly, no allocation per tick.
///
/// Add fields with care: every `AiBrain` impl reads this struct, and
/// learned-model checkpoints are tied to its layout. Bumping the layout
/// invalidates trained checkpoints.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColonyAiState {
    /// Stored food units in this colony's storage chamber.
    pub food_stored: f32,
    /// Rolling average of food returned to nest by foragers (sim-internal).
    pub food_inflow_recent: f32,
    /// Adult workers on roster.
    pub worker_count: u32,
    /// Adult soldiers on roster.
    pub soldier_count: u32,
    /// Adult breeders (alates).
    pub breeder_count: u32,
    /// Eggs in brood pool.
    pub brood_egg: u32,
    /// Larvae in brood pool.
    pub brood_larva: u32,
    /// Pupae in brood pool.
    pub brood_pupa: u32,
    /// Live queens in this colony.
    pub queens_alive: u32,
    /// Combat losses suffered by this colony in the last tick.
    pub combat_losses_recent: u32,
    /// Distance (in tiles) to the nearest known enemy ant. f32::INFINITY
    /// when no enemy has been sensed.
    pub enemy_distance_min: f32,
    /// Number of enemy workers currently on the same module.
    pub enemy_worker_count: u32,
    /// Number of enemy soldiers currently on the same module.
    pub enemy_soldier_count: u32,
    /// In-game day of year (0-364).
    pub day_of_year: u32,
    /// In-game ambient temperature (°C).
    pub ambient_temp_c: f32,
    /// True if the colony is currently in seasonal diapause.
    pub diapause_active: bool,
    /// True if it's daytime in-game (06:00-18:00 sim time).
    pub is_daytime: bool,
}

/// What the brain decides per tick. All fields are bounded; values
/// outside the documented range are clamped by the caller.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AiDecision {
    /// Worker share of newly-laid brood [0.0..1.0].
    pub caste_ratio_worker: f32,
    /// Soldier share [0.0..1.0]. worker+soldier+breeder must sum to 1.0
    /// (the caller renormalizes).
    pub caste_ratio_soldier: f32,
    /// Breeder share [0.0..1.0].
    pub caste_ratio_breeder: f32,
    /// Behavior weight: how much of the workforce prioritizes foraging
    /// [0.0..1.0]. The three weights are renormalized by the caller.
    pub forage_weight: f32,
    /// Behavior weight: digging.
    pub dig_weight: f32,
    /// Behavior weight: nursing brood.
    pub nurse_weight: f32,
    /// Optional tech the brain wants to research/unlock this tick.
    /// Most brains return None most ticks; PvP-mode brains may pace
    /// research as a strategic decision.
    pub research_choice: Option<TechUnlock>,
}

impl AiDecision {
    /// Trivial sanity check used by tests.
    pub fn is_valid(&self) -> bool {
        let castes = [
            self.caste_ratio_worker,
            self.caste_ratio_soldier,
            self.caste_ratio_breeder,
        ];
        let weights = [self.forage_weight, self.dig_weight, self.nurse_weight];
        castes.iter().chain(weights.iter()).all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    }
}

/// Pluggable per-colony AI brain. One instance per AI-controlled colony;
/// `decide` is called once per outer tick.
///
/// Implementations MUST be deterministic given a fixed RNG seed when
/// they consume randomness, so AI-vs-AI bench runs are reproducible.
pub trait AiBrain: Send + Sync {
    /// Plain-English brain identifier. Surfaces in logs, bench reports,
    /// and self-play data files.
    fn name(&self) -> &str;

    /// Compute the decision for this tick from the colony's state.
    fn decide(&mut self, state: &ColonyAiState) -> AiDecision;
}

// ============================================================
// HeuristicBrain — the legacy `red_ai_tick` rules as a brain.
// ============================================================

/// Reactive rule-based brain. Mirrors the pre-trait `Simulation::red_ai_tick`
/// semantics: under combat losses → escalate soldier ratio; under low
/// food → all-hands-foraging.
///
/// Deterministic; no RNG.
#[derive(Debug, Clone)]
pub struct HeuristicBrain {
    /// Snapshot of the brain's CURRENT internal weights (mutated
    /// every `decide()` so subsequent decisions build on the prior
    /// state, just like `red_ai_tick` does).
    state: BrainInternalState,
    /// Soft food threshold below which the colony shifts toward
    /// forage-everything. Auto-derived from colony egg cost in the
    /// constructor; can be overridden.
    pub low_food_threshold: f32,
}

#[derive(Debug, Clone)]
struct BrainInternalState {
    caste_ratio_worker: f32,
    caste_ratio_soldier: f32,
    caste_ratio_breeder: f32,
    forage_weight: f32,
    dig_weight: f32,
    nurse_weight: f32,
}

impl Default for BrainInternalState {
    fn default() -> Self {
        Self {
            caste_ratio_worker: 0.65,
            caste_ratio_soldier: 0.30,
            caste_ratio_breeder: 0.05,
            forage_weight: 0.55,
            dig_weight: 0.20,
            nurse_weight: 0.25,
        }
    }
}

impl HeuristicBrain {
    /// Build with sensible defaults. `egg_cost` is the colony's
    /// `egg_cost_food` value; the brain treats `egg_cost * 4` as
    /// the "low food" threshold (matches legacy `red_ai_tick`).
    pub fn new(egg_cost: f32) -> Self {
        Self {
            state: BrainInternalState::default(),
            low_food_threshold: egg_cost * 4.0,
        }
    }
}

impl AiBrain for HeuristicBrain {
    fn name(&self) -> &str {
        "heuristic"
    }

    fn decide(&mut self, state: &ColonyAiState) -> AiDecision {
        // Rule 1: combat losses → push soldier ratio up.
        if state.combat_losses_recent > 0 {
            let shift = 0.01 * state.combat_losses_recent as f32;
            let target = (self.state.caste_ratio_soldier + shift).min(0.5);
            let delta = target - self.state.caste_ratio_soldier;
            self.state.caste_ratio_soldier = target;
            self.state.caste_ratio_worker =
                (self.state.caste_ratio_worker - delta).max(0.05);
        }

        // Rule 2: food low → forage-everything.
        if state.food_stored < self.low_food_threshold {
            let shift = 0.02;
            let target = (self.state.forage_weight + shift).min(0.9);
            let delta = target - self.state.forage_weight;
            self.state.forage_weight = target;
            self.state.nurse_weight = (self.state.nurse_weight - delta * 0.5).max(0.05);
            self.state.dig_weight = (self.state.dig_weight - delta * 0.5).max(0.02);
        }

        AiDecision {
            caste_ratio_worker: self.state.caste_ratio_worker,
            caste_ratio_soldier: self.state.caste_ratio_soldier,
            caste_ratio_breeder: self.state.caste_ratio_breeder,
            forage_weight: self.state.forage_weight,
            dig_weight: self.state.dig_weight,
            nurse_weight: self.state.nurse_weight,
            research_choice: None,
        }
    }
}

// ============================================================
// RandomBrain — noise-floor opponent / smoke test.
// ============================================================

/// Uniformly-random decisions in the legal value bands. Seeded for
/// reproducibility. NOT a strong opponent — by design — it's the
/// matchup-bench's "noise floor" so we can measure how much any other
/// brain is actually beating chance.
#[derive(Debug)]
pub struct RandomBrain {
    rng: rand_chacha::ChaCha8Rng,
}

impl RandomBrain {
    pub fn new(seed: u64) -> Self {
        use rand::SeedableRng;
        Self {
            rng: rand_chacha::ChaCha8Rng::seed_from_u64(seed),
        }
    }
}

impl AiBrain for RandomBrain {
    fn name(&self) -> &str {
        "random"
    }

    fn decide(&mut self, _state: &ColonyAiState) -> AiDecision {
        use rand::Rng;
        // Caste ratios: pick three random draws and renormalize so they sum to 1.
        let a: f32 = self.rng.r#gen();
        let b: f32 = self.rng.r#gen();
        let c: f32 = self.rng.r#gen();
        let sum = (a + b + c).max(1e-6);
        // Behavior weights: same trick.
        let f: f32 = self.rng.r#gen();
        let d: f32 = self.rng.r#gen();
        let n: f32 = self.rng.r#gen();
        let wsum = (f + d + n).max(1e-6);
        AiDecision {
            caste_ratio_worker: a / sum,
            caste_ratio_soldier: b / sum,
            caste_ratio_breeder: c / sum,
            forage_weight: f / wsum,
            dig_weight: d / wsum,
            nurse_weight: n / wsum,
            research_choice: None,
        }
    }
}

// ============================================================
// AetherLmBrain — STUB. Trained checkpoint integration.
// ============================================================

/// Aether-LM-backed brain. Loads a checkpoint produced by
/// `J:/aether/target/release/aether-train.exe` and runs inference per
/// decision tick.
///
/// # Status: STUB
///
/// The integration point and signature are in place; the actual model
/// call is `todo!()` because we have no trained checkpoint yet. To wire
/// this for real:
/// 1. Define a state-feature serializer (`ColonyAiState` → fixed-length
///    f32 vector). Aether's tokenizer expects either text or a numeric
///    sequence — pick the format you train against.
/// 2. Define a decision deserializer (model output → `AiDecision`).
///    Recommended: 7 sigmoid'd outputs (3 caste + 3 weight + 1 research
///    softmax index) → renormalize → AiDecision.
/// 3. Replace `decide`'s `todo!()` with a call to either:
///    - In-process: link `aether_runtime` and call its inference API
///      directly (best perf; requires aether to expose a Rust API).
///    - Out-of-process: shell out to `aether-infer.exe` per tick (slow;
///      easy to get wrong; only good for prototyping).
///
/// # Self-play data
///
/// Once this brain is in place, the AI-vs-AI bench harness's
/// `--dump-trajectories <path>` flag should be added to write
/// `(state, decision, outcome)` triples to JSONL — that's the training
/// corpus for the next iteration.
pub struct AetherLmBrain {
    pub checkpoint_path: std::path::PathBuf,
    pub label: String,
}

impl AetherLmBrain {
    /// Construct a brain pointing at a checkpoint. Doesn't load until
    /// first `decide()` call (lazy — keeps Simulation construction cheap).
    pub fn new(checkpoint_path: impl Into<std::path::PathBuf>, label: impl Into<String>) -> Self {
        Self {
            checkpoint_path: checkpoint_path.into(),
            label: label.into(),
        }
    }
}

impl AiBrain for AetherLmBrain {
    fn name(&self) -> &str {
        &self.label
    }

    fn decide(&mut self, _state: &ColonyAiState) -> AiDecision {
        // TODO(ai): wire to aether-runtime / aether-infer. See doc above.
        // Until we have a trained checkpoint, fall back to a deterministic
        // safe default so a misconfigured PvP setup doesn't crash.
        tracing::warn!(
            checkpoint = %self.checkpoint_path.display(),
            "AetherLmBrain::decide — STUB; returning default. Wire integration before relying on this brain."
        );
        AiDecision {
            caste_ratio_worker: 0.7,
            caste_ratio_soldier: 0.25,
            caste_ratio_breeder: 0.05,
            forage_weight: 0.5,
            dig_weight: 0.2,
            nurse_weight: 0.3,
            research_choice: None,
        }
    }
}

// ============================================================
// Match-end detection.
// ============================================================

/// Outcome of an AI-vs-AI (or AI-vs-player) match at the moment of
/// inspection. The headless bench harness ticks until this transitions
/// out of `InProgress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MatchStatus {
    /// Both colonies still have at least one queen and at least one
    /// adult ant of any caste.
    InProgress,
    /// Exactly one colony lost (queens=0 OR adults=0). The survivor wins.
    Won { winner: u8, loser: u8, ended_at_tick: u64 },
    /// Both colonies died on the same tick (rare; only happens with
    /// simultaneous queen kills or environmental sweeps).
    Draw { ended_at_tick: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral_state() -> ColonyAiState {
        ColonyAiState {
            food_stored: 100.0,
            food_inflow_recent: 0.5,
            worker_count: 30,
            soldier_count: 5,
            breeder_count: 1,
            brood_egg: 10,
            brood_larva: 5,
            brood_pupa: 5,
            queens_alive: 1,
            combat_losses_recent: 0,
            enemy_distance_min: f32::INFINITY,
            enemy_worker_count: 0,
            enemy_soldier_count: 0,
            day_of_year: 150,
            ambient_temp_c: 22.0,
            diapause_active: false,
            is_daytime: true,
        }
    }

    #[test]
    fn heuristic_brain_idle_state_returns_defaults() {
        let mut b = HeuristicBrain::new(5.0);
        let d = b.decide(&neutral_state());
        assert!(d.is_valid());
        assert_eq!(d.caste_ratio_soldier, 0.30);
        assert_eq!(d.forage_weight, 0.55);
    }

    #[test]
    fn heuristic_brain_escalates_soldier_under_losses() {
        let mut b = HeuristicBrain::new(5.0);
        let initial = b.decide(&neutral_state()).caste_ratio_soldier;
        let mut state = neutral_state();
        state.combat_losses_recent = 5;
        let after = b.decide(&state).caste_ratio_soldier;
        assert!(after > initial, "soldier ratio should rise after losses ({initial} → {after})");
    }

    #[test]
    fn heuristic_brain_escalates_forage_when_food_low() {
        let mut b = HeuristicBrain::new(5.0);
        let initial = b.decide(&neutral_state()).forage_weight;
        let mut state = neutral_state();
        state.food_stored = 2.0; // well below low_food_threshold (= 5.0 * 4 = 20)
        let after = b.decide(&state).forage_weight;
        assert!(after > initial, "forage weight should rise when food low ({initial} → {after})");
    }

    #[test]
    fn random_brain_outputs_are_in_band() {
        let mut b = RandomBrain::new(42);
        for _ in 0..100 {
            let d = b.decide(&neutral_state());
            assert!(d.is_valid(), "RandomBrain produced out-of-band decision: {d:?}");
        }
    }

    #[test]
    fn random_brain_is_deterministic_given_seed() {
        let mut a = RandomBrain::new(7);
        let mut b = RandomBrain::new(7);
        for _ in 0..10 {
            let s = neutral_state();
            let da = a.decide(&s);
            let db = b.decide(&s);
            assert_eq!(da.caste_ratio_worker, db.caste_ratio_worker);
            assert_eq!(da.forage_weight, db.forage_weight);
        }
    }

    #[test]
    fn aether_brain_stub_returns_safe_default() {
        let mut b = AetherLmBrain::new("nonexistent/checkpoint", "aether-test");
        let d = b.decide(&neutral_state());
        assert!(d.is_valid(), "stub must return a valid decision");
        assert_eq!(b.name(), "aether-test");
    }
}

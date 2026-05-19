//! Observation + action types for the hierarchical brain (Phase 1).
//!
//! The hierarchical commander/ant brain trainer consumes the types in this
//! module via the four new `Simulation` methods (see simulation.rs). All
//! types here are pure data carriers — no behavior — and serde-roundtrip
//! cleanly so they can be saved with sim snapshots.
//!
//! Defaults are chosen so that a sim run with no trainer attached is
//! byte-identical to today's behavior. See `AntModulators::default`.

use serde::{Deserialize, Serialize};

/// Per-ant ACO knobs the per-ant brain (Phase 2) outputs each tick.
///
/// Defaults are the **identity** for the existing ACO math in
/// `ant.rs::choose_direction`: alpha_mult and beta_mult multiply by 1.0,
/// exploration_mod adds 0.0, deposit_mult multiplies by 1.0, state_bias
/// adds 0.0 to the FSM transition logit it gates. With defaults the sim
/// produces byte-identical output to the pre-plumbing version.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AntModulators {
    /// Multiplier on the pheromone-intensity exponent. Clamped [0.1, 5.0]
    /// at apply time. Default 1.0 (no modulation).
    pub alpha_mult: f32,
    /// Multiplier on the desirability/forward-bias exponent. Clamped
    /// [0.1, 5.0] at apply time. Default 1.0.
    pub beta_mult: f32,
    /// Additive offset to `AntConfig::exploration_rate`. Clamped [-0.1,
    /// 0.1] at apply time. Default 0.0.
    pub exploration_mod: f32,
    /// Multiplier on pheromone deposit strength. Clamped [0.1, 5.0] at
    /// apply time. Default 1.0.
    pub deposit_mult: f32,
    /// Additive logit bias on FSM transition probabilities. Clamped
    /// [-2.0, 2.0] at apply time. Default 0.0.
    pub state_bias: f32,
}

impl Default for AntModulators {
    fn default() -> Self {
        Self {
            alpha_mult: 1.0,
            beta_mult: 1.0,
            exploration_mod: 0.0,
            deposit_mult: 1.0,
            state_bias: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modulators_default_is_identity() {
        let m = AntModulators::default();
        assert_eq!(m.alpha_mult, 1.0);
        assert_eq!(m.beta_mult, 1.0);
        assert_eq!(m.exploration_mod, 0.0);
        assert_eq!(m.deposit_mult, 1.0);
        assert_eq!(m.state_bias, 0.0);
    }

    #[test]
    fn modulators_serde_roundtrip() {
        let m = AntModulators {
            alpha_mult: 2.5,
            beta_mult: 0.5,
            exploration_mod: -0.05,
            deposit_mult: 3.0,
            state_bias: 1.25,
        };
        let json = serde_json::to_string(&m).unwrap();
        let parsed: AntModulators = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, m);
    }
}

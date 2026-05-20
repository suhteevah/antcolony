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

/// Serde helper for `[f32; 72]` — serde's built-in array impls only cover
/// up to N=32; larger arrays need a manual helper.
mod serde_f32_72 {
    use serde::{Deserializer, Serializer, de::SeqAccess, de::Visitor, ser::SerializeTuple};
    use std::fmt;

    pub fn serialize<S: Serializer>(arr: &[f32; 72], s: S) -> Result<S::Ok, S::Error> {
        let mut tup = s.serialize_tuple(72)?;
        for v in arr {
            tup.serialize_element(v)?;
        }
        tup.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[f32; 72], D::Error> {
        struct Arr72;
        impl<'de> Visitor<'de> for Arr72 {
            type Value = [f32; 72];
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "array of 72 f32 values")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<[f32; 72], A::Error> {
                let mut arr = [0.0f32; 72];
                for slot in &mut arr {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                }
                Ok(arr)
            }
        }
        d.deserialize_tuple(72, Arr72)
    }
}

/// Per-ant ACO knobs the per-ant brain (Phase 2) outputs each tick.
///
/// Defaults are the **identity** for the existing ACO math in
/// `ant.rs::choose_direction`: alpha_mult and beta_mult multiply by 1.0,
/// exploration_mod adds 0.0, deposit_mult multiplies by 1.0, state_bias
/// adds 0.0 to the FSM transition logit it gates. With defaults the sim
/// produces byte-identical output to the pre-plumbing version.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AntModulators {
    /// Multiplier on the pheromone-intensity exponent. Clamped to
    /// `[0.1, 5.0]` on the **write side** by `Simulation::apply_ant_modulators`
    /// (trainer-facing contract). `choose_direction` applies a wider
    /// read-side safety clamp of `[0.1, 10.0]` as defense-in-depth.
    /// Default 1.0 (no modulation).
    pub alpha_mult: f32,
    /// Multiplier on the desirability/forward-bias exponent. Clamped to
    /// `[0.1, 5.0]` on the **write side** by `Simulation::apply_ant_modulators`.
    /// Read-side safety clamp in `choose_direction` is `[0.1, 10.0]`.
    /// Default 1.0.
    pub beta_mult: f32,
    /// Additive offset to `AntConfig::exploration_rate`. Clamped to
    /// `[-0.1, 0.1]` on the **write side** by `Simulation::apply_ant_modulators`.
    /// Read-side safety clamp in `choose_direction` is `[0.0, 1.0]`.
    /// Default 0.0.
    pub exploration_mod: f32,
    /// Multiplier on pheromone deposit strength. Clamped to `[0.1, 5.0]`
    /// on the **write side** by `Simulation::apply_ant_modulators`.
    /// Default 1.0.
    pub deposit_mult: f32,
    /// Additive logit bias on FSM transition probabilities. Clamped to
    /// `[-2.0, 2.0]` on the **write side** by `Simulation::apply_ant_modulators`.
    /// Default 0.0.
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

/// Snapshot of all four pheromone channels at a single tick. Used by
/// the commander tier's CNN-encoded spatial input. The trainer
/// downsamples this to a fixed 32×32 via `Simulation::pheromone_snapshot`
/// before feeding the policy net.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PheromoneSnapshot {
    pub width: u16,
    pub height: u16,
    /// Row-major, length = width*height. Indexed as [y * width + x].
    pub food_trail: Box<[f32]>,
    pub home_trail: Box<[f32]>,
    pub alarm: Box<[f32]>,
    pub colony_scent: Box<[f32]>,
}

/// One entry in the commander's history ring buffer (last 8 decision
/// cycles). The commander backbone consumes K=8 of these as 96-d tokens
/// alongside the state and pheromone inputs. Pad fields are unused by
/// Phase 1 — they're reserved for auxiliary features in later phases.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HistoryToken {
    pub state: [f32; 17],
    pub action: [f32; 6],
    pub reward: f32,
    #[serde(with = "serde_f32_72")]
    pub pad: [f32; 72],
}

impl Default for HistoryToken {
    fn default() -> Self {
        Self {
            state: [0.0; 17],
            action: [0.0; 6],
            reward: 0.0,
            pad: [0.0; 72],
        }
    }
}

impl HistoryToken {
    /// Total float count when flattened — used as a shape check by Phase
    /// 2 trainer code. Must equal 17 + 6 + 1 + 72 = 96.
    pub const FLAT_LEN: usize = 96;
}

/// Per-ant observation produced by the simulation each tick. Consumed by the
/// per-ant brain tier (Phase 2). Fields are placeholders until the per-ant
/// policy net is specified; for now the struct carries the ant's FSM state
/// index and the pheromone readings in its sensing cone.
///
/// Phase 1 only wires the commander tier — this type is declared here so
/// integration tests can import the module without breakage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AntObservation {
    /// Ant entity index within the simulation's flat ant Vec.
    pub ant_index: u32,
    /// Encoded FSM state (matches `AntState as u8`).
    pub fsm_state: u8,
    /// Pheromone readings in the forward sensing cone (5 cells × 4 channels).
    pub cone_pheromone: [f32; 20],
    /// Food carried normalized to `[0, 1]`.
    pub food_carried_norm: f32,
    /// Health normalized to `[0, 1]`.
    pub health_norm: f32,
}

impl Default for AntObservation {
    fn default() -> Self {
        Self {
            ant_index: 0,
            fsm_state: 0,
            cone_pheromone: [0.0; 20],
            food_carried_norm: 0.0,
            health_norm: 1.0,
        }
    }
}

/// Bundle of everything the commander brain reads at decision time.
/// The `state` field is the existing `ColonyAiState`; the other two are
/// new (pheromone field as 32×32×4 tensor, last 8 commander tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichObservation {
    pub state: crate::ai::brain::ColonyAiState,
    pub pheromone_field: PheromoneSnapshot,
    pub history: arrayvec::ArrayVec<HistoryToken, 8>,
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
    fn history_token_flat_len_is_96() {
        assert_eq!(HistoryToken::FLAT_LEN, 96);
        let t = HistoryToken::default();
        let total = t.state.len() + t.action.len() + 1 + t.pad.len();
        assert_eq!(total, HistoryToken::FLAT_LEN);
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

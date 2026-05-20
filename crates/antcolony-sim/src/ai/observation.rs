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

/// Serde helper for `[f32; 60]` — serde's built-in array impls only cover
/// up to N=32; larger arrays need a manual helper.
mod serde_f32_60 {
    use serde::{Deserializer, Serializer, de::SeqAccess, de::Visitor, ser::SerializeTuple};
    use std::fmt;

    pub fn serialize<S: Serializer>(arr: &[f32; 60], s: S) -> Result<S::Ok, S::Error> {
        let mut tup = s.serialize_tuple(60)?;
        for v in arr {
            tup.serialize_element(v)?;
        }
        tup.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[f32; 60], D::Error> {
        struct Arr60;
        impl<'de> Visitor<'de> for Arr60 {
            type Value = [f32; 60];
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "array of 60 f32 values")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<[f32; 60], A::Error> {
                let mut arr = [0.0f32; 60];
                for slot in &mut arr {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                }
                Ok(arr)
            }
        }
        d.deserialize_tuple(60, Arr60)
    }
}

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

/// Per-ant local observation, one entry per adult ant in the colony.
/// Batched form so the trainer can stack into a single GPU tensor. The
/// commander's intent vector is NOT included here — the trainer reads
/// it once per decision window via the colony's `commander_intent`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AntObservation {
    pub ant_id: u32,
    /// Up to 15 pheromone-cone samples × 4 channels = 60 floats.
    /// Sampled via `PheromoneGrid::sample_cone` with the same geometry
    /// `choose_direction` uses (60° forward cone, radius 5).
    ///
    /// **Layout: channel-major, position-within-channel is unordered.**
    /// `cone[ch * 15 + i]` is the i-th hit of channel `ch`, where i runs
    /// 0..N with N = `sample_cone`'s actual hit count (≤ 15). Trailing
    /// slots are zero. `sample_cone` iterates cells in raster order, so
    /// `i` does NOT correspond to a fixed `(step, lateral)` grid
    /// position — only to "i-th cell of the cone visited." The Phase-2
    /// trainer must treat these as a bag of samples, not a structured
    /// 5×3 spatial input. If positional structure is needed later, add
    /// a separate `sample_cone_structured` helper to `PheromoneGrid`.
    #[serde(with = "serde_f32_60")]
    pub pheromone_cone: [f32; 60],
    /// food_carried, heading_sin, heading_cos, caste_onehot[3],
    /// state_timer_norm, age_norm. Exact layout fixed for trainer
    /// compatibility:
    ///   internal[0] = food_carried
    ///   internal[1] = heading.sin()
    ///   internal[2] = heading.cos()
    ///   internal[3] = caste_is_worker  (0.0 or 1.0)
    ///   internal[4] = caste_is_soldier
    ///   internal[5] = caste_is_breeder
    ///   internal[6] = (state_timer as f32 / 1000.0).clamp(0, 1)
    ///   internal[7] = (age as f32 / 10000.0).clamp(0, 1)
    pub internal: [f32; 8],
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

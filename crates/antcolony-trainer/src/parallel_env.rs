//! Parallel-env rollout for single-GPU Phase-3 training.
//!
//! Holds N `MatchEnv`s, each with the left colony driven by the HAC and the
//! right by a league-sampled `AiBrain`. The sim steps on CPU; the left
//! colony's observations are batched across all *active* envs into single
//! GPU forwards (commander every DECISION_CADENCE ticks, ants every tick),
//! and outputs scattered back. Emits the existing `JointRollout` with
//! left-colony records only, `match_idx = env_idx`, `colony = 0`, so the
//! Phase-2b-2 `joint_update` + GAE are reused unchanged.

use anyhow::Result;
use candle_core::{Device, Tensor};
use rand_chacha::ChaCha8Rng;

use antcolony_sim::ai::observation::AntModulators;
use antcolony_sim::{AiBrain, AiDecision, MatchStatus};

use crate::env::{MatchEnv, DECISION_CADENCE};
use crate::hierarchical::obs_to_tensors::{ant_obs_to_tensors, rich_batch_to_tensors};
use crate::hierarchical::sizing::{FIXED_INTENT_D, FIXED_MODULATOR_D};
use crate::joint_ppo::{AntRecord, CommanderRecord, JointRollout};
use crate::reward::{compute_step_reward, ColonyMetrics, RewardConfig};
use crate::HierarchicalActorCritic;
use crate::League;

pub struct ParallelEnv {
    pub n_envs: usize,
    pub rollout_cycles: usize,
    pub league: League,
}

impl ParallelEnv {
    pub fn new(n_envs: usize, rollout_cycles: usize) -> Self {
        Self { n_envs, rollout_cycles, league: League::default_pool() }
    }
}

/// Decode one row of a [B, 6] squashed-action tensor into an AiDecision.
fn row_to_decision(action: &Tensor, row: usize) -> Result<AiDecision> {
    let v: Vec<f32> = action.narrow(0, row, 1)?.squeeze(0)?.to_vec1()?;
    Ok(AiDecision {
        caste_ratio_worker: v[0],
        caste_ratio_soldier: v[1],
        caste_ratio_breeder: v[2],
        forage_weight: v[3],
        dig_weight: v[4],
        nurse_weight: v[5],
        research_choice: None,
    })
}

/// One row of a [B, 64] intent tensor into a fixed [f32; 64] array.
fn row_to_intent(intent: &Tensor, row: usize) -> Result<[f32; FIXED_INTENT_D]> {
    let v: Vec<f32> = intent.narrow(0, row, 1)?.flatten_all()?.to_vec1()?;
    let mut arr = [0.0f32; FIXED_INTENT_D];
    arr.copy_from_slice(&v);
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_env_constructs_with_default_league() {
        let pe = ParallelEnv::new(4, 8);
        assert_eq!(pe.n_envs, 4);
        assert_eq!(pe.rollout_cycles, 8);
        assert_eq!(pe.league.entries.len(), 7, "default pool = 7 archetypes");
    }
}

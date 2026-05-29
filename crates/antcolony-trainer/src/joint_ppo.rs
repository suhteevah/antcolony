//! Joint PPO trainer for the hierarchical (commander + ant) brain.
//!
//! Self-plays both colonies of a `MatchEnv` with one shared
//! `HierarchicalActorCritic`, collects a two-buffer rollout (commander
//! @ cycle cadence, ant @ tick cadence), computes per-tier GAE at the
//! colony level, and optimizes `L_total = L_cmdr + α_balance · L_ant`
//! with a single AdamW step. Phase 2b-2 scope: single-device (CPU f32)
//! smoke. See docs/superpowers/plans/2026-05-29-ant-brain-phase2b2-joint-trainer.md.

use crate::hierarchical::sizing::Sizing;
use crate::HierarchicalActorCritic;
use candle_core::{DType, Device, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};

#[derive(Clone, Debug)]
pub struct JointPpoConfig {
    pub iterations: usize,
    pub matches_per_iter: usize,
    /// Hard cap on commander decision cycles collected per match. Keeps
    /// the smoke rollout short (one cycle = DECISION_CADENCE outer ticks).
    pub rollout_cycles: usize,
    pub gamma: f32,
    pub gae_lambda: f32,
    pub clip: f32,
    pub epochs_per_batch: usize,
    pub lr: f64,
    pub value_coef: f64,
    pub cmdr_entropy_coef: f64,
    pub ant_entropy_coef: f64,
    /// Down-weights the ant-tier loss so ~10× more ant samples per cycle
    /// don't drown the commander gradient. Design spec §"Joint PPO loss".
    pub alpha_balance: f64,
    pub seed: u64,
}

impl JointPpoConfig {
    /// Minimal config for the mechanical smoke: 5 iters, 2 matches/iter,
    /// 8 cycles/match. Tuned for "runs without NaN/crash in under a
    /// minute on CPU", not for convergence.
    pub fn smoke_default() -> Self {
        Self {
            iterations: 5,
            matches_per_iter: 2,
            rollout_cycles: 8,
            gamma: 0.99,
            gae_lambda: 0.95,
            clip: 0.2,
            epochs_per_batch: 1,
            lr: 3e-4,
            value_coef: 0.5,
            cmdr_entropy_coef: 0.005,
            ant_entropy_coef: 0.01,
            alpha_balance: 0.1,
            seed: 0xa17_c01_2b2,
        }
    }
}

pub struct JointPpoTrainer {
    pub hac: HierarchicalActorCritic,
    pub varmap: VarMap,
    pub device: Device,
    pub config: JointPpoConfig,
    pub rng: rand_chacha::ChaCha8Rng,
}

impl JointPpoTrainer {
    pub fn new(device: Device, sizing: Sizing, config: JointPpoConfig) -> anyhow::Result<Self> {
        use rand::SeedableRng;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, sizing)?;
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(config.seed);
        Ok(Self { hac, varmap, device, config, rng })
    }

    /// AdamW over every VarMap parameter (both tiers).
    pub fn make_optimizer(&self) -> anyhow::Result<AdamW> {
        let params = ParamsAdamW {
            lr: self.config.lr,
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-8,
            weight_decay: 0.0,
        };
        Ok(AdamW::new(self.varmap.all_vars(), params)?)
    }
}

/// One commander decision for one colony in one cycle of one match.
pub struct CommanderRecord {
    pub match_idx: usize,
    pub colony: u8,
    pub cycle: usize,
    pub state: Tensor,     // [1, 17]
    pub pheromone: Tensor, // [1, 4, 32, 32]
    pub history: Tensor,   // [1, 8, 96]
    pub action: Tensor,    // [1, 6] post-squash
    pub log_prob: f32,
    pub value: f32,
    pub reward: f32,
    pub done: bool,
}

/// One ant decision for one ant in one tick of one cycle.
pub struct AntRecord {
    pub match_idx: usize,
    pub colony: u8,
    pub cycle: usize,
    pub cone: Tensor,      // [1, 60]
    pub internal: Tensor,  // [1, 8]
    pub intent: Tensor,    // [1, 64]
    pub modulator: Tensor, // [1, 5] post-squash
    pub log_prob: f32,
    pub value: f32,
}

#[derive(Default)]
pub struct JointRollout {
    pub commander: Vec<CommanderRecord>,
    pub ant: Vec<AntRecord>,
}

/// Per-iteration loss breakdown for logging + the smoke assertion.
#[derive(Clone, Debug)]
pub struct JointLossStats {
    pub total: f32,
    pub commander: f32,
    pub ant: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchical::sizing::A1;

    #[test]
    fn smoke_default_has_five_iters_and_balanced_coefs() {
        let c = JointPpoConfig::smoke_default();
        assert_eq!(c.iterations, 5);
        assert_eq!(c.matches_per_iter, 2);
        assert_eq!(c.rollout_cycles, 8);
        assert!(c.alpha_balance > 0.0 && c.alpha_balance <= 1.0);
        assert!(c.ant_entropy_coef >= c.cmdr_entropy_coef);
    }

    #[test]
    fn trainer_builds_at_a1_with_nonzero_params() {
        let t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let total: usize = t.varmap.all_vars().iter()
            .map(|v| v.dims().iter().product::<usize>()).sum();
        assert!(total > 1_000_000, "A1 HAC should have >1M params, got {}", total);
        let _opt = t.make_optimizer().unwrap();
    }

    #[test]
    fn joint_rollout_defaults_empty() {
        let r = JointRollout::default();
        assert!(r.commander.is_empty());
        assert!(r.ant.is_empty());
    }
}

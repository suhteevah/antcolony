//! Joint PPO trainer for the hierarchical (commander + ant) brain.
//!
//! Self-plays both colonies of a `MatchEnv` with one shared
//! `HierarchicalActorCritic`, collects a two-buffer rollout (commander
//! @ cycle cadence, ant @ tick cadence), computes per-tier GAE at the
//! colony level, and optimizes `L_total = L_cmdr + α_balance · L_ant`
//! with a single AdamW step. Phase 2b-2 scope: single-device (CPU f32)
//! smoke. See docs/superpowers/plans/2026-05-29-ant-brain-phase2b2-joint-trainer.md.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_default_has_five_iters_and_balanced_coefs() {
        let c = JointPpoConfig::smoke_default();
        assert_eq!(c.iterations, 5);
        assert_eq!(c.matches_per_iter, 2);
        assert_eq!(c.rollout_cycles, 8);
        assert!(c.alpha_balance > 0.0 && c.alpha_balance <= 1.0);
        assert!(c.ant_entropy_coef >= c.cmdr_entropy_coef);
    }
}

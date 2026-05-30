//! Phase-3 single-GPU training driver. Ties ParallelEnv (left-vs-league
//! rollout) + the Phase-2b-2 joint_update + the deterministic eval harness
//! into a training loop with periodic eval + checkpoints. Reuses
//! JointPpoTrainer for the HAC + varmap + joint_update.

use anyhow::Result;
use std::path::PathBuf;

use crate::eval::{evaluate_hac, EvalReport};
use crate::joint_ppo::{JointLossStats, JointPpoConfig, JointPpoTrainer};
use crate::parallel_env::ParallelEnv;
use crate::reward::RewardConfig;
use crate::hierarchical::sizing::{Sizing, A1};
use candle_core::Device;

#[derive(Clone, Debug)]
pub struct Phase3Config {
    pub iterations: usize,
    pub n_envs: usize,
    pub rollout_cycles: usize,
    pub eval_every: usize,
    pub matches_per_eval: usize,
    pub reward: RewardConfig,
    pub joint: JointPpoConfig,
    pub out_dir: PathBuf,
}

impl Phase3Config {
    /// Small config for the mechanical smoke (fast on CPU).
    pub fn smoke(out_dir: PathBuf) -> Self {
        Self {
            iterations: 2,
            n_envs: 4,
            rollout_cycles: 4,
            eval_every: 1,
            matches_per_eval: 1,
            reward: RewardConfig::default(),
            joint: JointPpoConfig::smoke_default(),
            out_dir,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Phase3Report {
    pub losses: Vec<JointLossStats>,
    pub evals: Vec<(usize, f32)>,
    pub final_eval: Option<EvalReport>,
}

/// Run Phase-3 training. `sizing` is the model preset (A1 for the first run).
pub fn run_phase3(device: Device, sizing: Sizing, cfg: Phase3Config) -> Result<Phase3Report> {
    std::fs::create_dir_all(&cfg.out_dir).ok();
    let mut trainer = JointPpoTrainer::new(device.clone(), sizing, cfg.joint.clone())?;
    let mut opt = trainer.make_optimizer()?;
    let mut pe = ParallelEnv::new(cfg.n_envs, cfg.rollout_cycles);

    let mut report = Phase3Report { losses: Vec::new(), evals: Vec::new(), final_eval: None };

    for it in 0..cfg.iterations {
        let base_seed = cfg.joint.seed ^ ((it as u64) << 40);
        let roll = pe.collect_rollout(
            &trainer.hac, &trainer.device, &mut trainer.rng, &cfg.reward, base_seed,
        )?;
        let stats = trainer.joint_update(&mut opt, &roll)?;
        tracing::info!(
            iter = it, total = stats.total, commander = stats.commander, ant = stats.ant,
            cmdr_records = roll.commander.len(), ant_records = roll.ant.len(),
            "phase3 iter"
        );
        report.losses.push(stats);

        if cfg.eval_every > 0 && it % cfg.eval_every == 0 {
            let ev = evaluate_hac(&trainer.hac, &trainer.device, cfg.matches_per_eval)?;
            tracing::info!(iter = it, mean_win_rate = ev.mean_win_rate, "phase3 eval");
            report.evals.push((it, ev.mean_win_rate));
            let ckpt = cfg.out_dir.join(format!("hac_iter{it:04}.safetensors"));
            if let Err(e) = trainer.varmap.save(&ckpt) {
                tracing::warn!(error = %e, path = %ckpt.display(), "checkpoint save failed");
            }
        }
    }

    let final_ev = evaluate_hac(&trainer.hac, &trainer.device, cfg.matches_per_eval)?;
    tracing::info!(mean_win_rate = final_ev.mean_win_rate, "phase3 final eval");
    let _ = trainer.varmap.save(cfg.out_dir.join("hac_final.safetensors"));
    report.final_eval = Some(final_ev);

    let _ = A1;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchical::sizing::A1;
    use candle_core::Device;

    #[test]
    fn phase3_smoke_runs_and_evals() {
        let tmp = std::env::temp_dir().join("antcolony_phase3_smoke");
        let cfg = Phase3Config::smoke(tmp);
        let report = run_phase3(Device::Cpu, A1, cfg).unwrap();
        assert_eq!(report.losses.len(), 2);
        for s in &report.losses {
            assert!(s.total.is_finite(), "loss must be finite: {}", s.total);
        }
        assert!(!report.evals.is_empty(), "should have at least one eval");
        let fe = report.final_eval.expect("final eval");
        assert!((0.0..=1.0).contains(&fe.mean_win_rate));
    }
}

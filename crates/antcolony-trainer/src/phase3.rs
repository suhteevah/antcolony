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
use crate::self_play::{OpponentSampler, SnapshotPool};
use crate::hierarchical::sizing::{Sizing, A1};
use candle_core::Device;

#[derive(Clone, Debug)]
pub struct Phase3Config {
    pub iterations: usize,
    pub n_envs: usize,
    pub rollout_cycles: usize,
    pub eval_every: usize,
    pub matches_per_eval: usize,
    /// Stop training early if no eval improves on the best-so-far for this many
    /// consecutive evals. `0` disables early-stop (run all `iterations`). The
    /// best checkpoint is kept regardless (see `hac_best.safetensors`), so this
    /// only trades wall-clock for the tail of a flat/declining curve.
    pub early_stop_patience: usize,
    pub reward: RewardConfig,
    pub joint: JointPpoConfig,
    pub out_dir: PathBuf,

    // --- SP1 self-play knobs ---
    /// Master switch. When `false` (default), behaviour is identical to pre-SP1.
    pub self_play_enabled: bool,
    /// Save a HAC snapshot every this many iterations (when self-play is on).
    pub snapshot_every: usize,
    /// Maximum number of frozen-HAC snapshots kept in the pool (FIFO eviction).
    pub pool_cap: usize,
    /// Opponent-sampling strategy passed to the pool.
    pub opponent_sampling: OpponentSampler,
    /// If set, pre-load this checkpoint as the first "warm-start SOTA" snapshot.
    pub warm_start_snapshot: Option<PathBuf>,
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
            early_stop_patience: 0,
            reward: RewardConfig::default(),
            joint: JointPpoConfig::smoke_default(),
            out_dir,
            // SP1 defaults — off so existing tests are unaffected
            self_play_enabled: false,
            snapshot_every: 25,
            pool_cap: 8,
            opponent_sampling: OpponentSampler::Pfsp { archetype_mix: 0.5, power: 1.0 },
            warm_start_snapshot: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Phase3Report {
    pub losses: Vec<JointLossStats>,
    pub evals: Vec<(usize, f32)>,
    pub final_eval: Option<EvalReport>,
    /// `(iter, mean_win_rate)` of the best eval seen (periodic evals + the
    /// final eval). The weights at that point are persisted to
    /// `hac_best.safetensors`. `None` only if no eval ran at all.
    pub best_eval: Option<(usize, f32)>,
    /// Mean `win_rate_ema` over all pool entries when self-play is on.
    /// `None` when `self_play_enabled = false`.
    pub self_play_winrate: Option<f32>,
}

/// Run Phase-3 training. `sizing` is the model preset (A1 for the first run).
pub fn run_phase3(device: Device, sizing: Sizing, cfg: Phase3Config) -> Result<Phase3Report> {
    std::fs::create_dir_all(&cfg.out_dir).ok();
    let mut trainer = JointPpoTrainer::new(device.clone(), sizing, cfg.joint.clone())?;
    let mut opt = trainer.make_optimizer()?;
    let mut pe = ParallelEnv::new(cfg.n_envs, cfg.rollout_cycles);

    // SP1: wire self-play pool + sampler before the loop
    if cfg.self_play_enabled {
        tracing::info!(
            pool_cap = cfg.pool_cap,
            snapshot_every = cfg.snapshot_every,
            warm_start = ?cfg.warm_start_snapshot,
            "phase3: self-play enabled"
        );
        pe.self_play_enabled = true;
        pe.pool = SnapshotPool::with_archetypes(cfg.pool_cap, 0.1);
        pe.sampler = cfg.opponent_sampling;
        if let Some(ref p) = cfg.warm_start_snapshot {
            pe.pool.add_snapshot("sota", p.clone());
            tracing::info!(path = %p.display(), "phase3: warm-start snapshot added to pool");
        }
    }

    let mut report = Phase3Report {
        losses: Vec::new(), evals: Vec::new(), final_eval: None, best_eval: None,
        self_play_winrate: None,
    };
    // Best eval seen so far + its weights snapshot (hac_best.safetensors), and a
    // staleness counter for optional early-stop. Keeping the best checkpoint is
    // the fix for the curve that peaks then regresses — the periodic
    // hac_iterNNNN checkpoints already capture it, but hac_best makes "the one
    // to ship" unambiguous and lets the CLI report it.
    let mut best: Option<(usize, f32)> = None;
    let mut stale = 0usize;
    let best_path = cfg.out_dir.join("hac_best.safetensors");
    let save_best = |it: usize, wr: f32, vm: &candle_nn::VarMap, tag: &str| {
        match vm.save(&best_path) {
            Ok(()) => tracing::info!(iter = it, mean_win_rate = wr, tag, "new best checkpoint -> hac_best.safetensors"),
            Err(e) => tracing::warn!(error = %e, path = %best_path.display(), "best checkpoint save failed"),
        }
    };

    let mut last_it = 0usize;
    for it in 0..cfg.iterations {
        last_it = it;
        let base_seed = cfg.joint.seed ^ ((it as u64) << 40);
        // SP1: save a snapshot of the training HAC every `snapshot_every` iters
        if cfg.self_play_enabled && cfg.snapshot_every > 0 && it % cfg.snapshot_every == 0 {
            let snap_path = cfg.out_dir.join(format!("snapshot_{it:04}.safetensors"));
            match trainer.varmap.save(&snap_path) {
                Ok(()) => {
                    tracing::info!(iter = it, path = %snap_path.display(), "phase3: snapshot saved");
                    pe.pool.add_snapshot(format!("snap{it:04}"), snap_path);
                }
                Err(e) => tracing::warn!(iter = it, error = %e, "phase3: snapshot save failed"),
            }
        }

        let roll = pe.collect_rollout(
            &trainer.hac, &trainer.device, &mut trainer.rng, &cfg.reward, base_seed,
        )?;
        // SP1: record rollout outcome back into the pool for PFSP EMA tracking
        if cfg.self_play_enabled {
            pe.pool.record_result(pe.last_opponent_idx, pe.last_hac_winshare);
        }
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

            if best.is_none_or(|(_, b)| ev.mean_win_rate > b) {
                best = Some((it, ev.mean_win_rate));
                stale = 0;
                save_best(it, ev.mean_win_rate, &trainer.varmap, "periodic");
            } else {
                stale += 1;
                if cfg.early_stop_patience > 0 && stale >= cfg.early_stop_patience {
                    tracing::info!(
                        iter = it, stale, patience = cfg.early_stop_patience,
                        best_iter = best.map(|(i, _)| i), best_win_rate = best.map(|(_, w)| w),
                        "early stop: no eval improvement"
                    );
                    break;
                }
            }
        }
    }

    // Final eval runs UNCONDITIONALLY when self-play is OFF — preserving the
    // exact pre-self-play behavior (the byte-identical-when-off guarantee). When
    // self-play is ON it is gated on `eval_every > 0`, so the eval-light self-play
    // smoke (which sets eval_every = 0) skips full-match eval and stays fast.
    if !cfg.self_play_enabled || cfg.eval_every > 0 {
        let final_ev = evaluate_hac(&trainer.hac, &trainer.device, cfg.matches_per_eval)?;
        tracing::info!(mean_win_rate = final_ev.mean_win_rate, "phase3 final eval");
        let _ = trainer.varmap.save(cfg.out_dir.join("hac_final.safetensors"));
        // The final-state weights are also a best candidate. Mark its "iter" just
        // past the last training iter so it's distinguishable from a periodic eval.
        let final_iter = last_it + 1;
        if best.is_none_or(|(_, b)| final_ev.mean_win_rate > b) {
            best = Some((final_iter, final_ev.mean_win_rate));
            save_best(final_iter, final_ev.mean_win_rate, &trainer.varmap, "final");
        }
        report.final_eval = Some(final_ev);
    }
    report.best_eval = best;
    if let Some((bi, bw)) = best {
        tracing::info!(best_iter = bi, best_win_rate = bw, "phase3 best (kept in hac_best.safetensors)");
    }

    // SP1: compute health metric = mean win_rate_ema over all pool entries
    if cfg.self_play_enabled {
        let entries = &pe.pool.entries;
        if entries.is_empty() {
            report.self_play_winrate = Some(0.5);
        } else {
            let mean = entries.iter().map(|e| e.win_rate_ema).sum::<f32>() / entries.len() as f32;
            report.self_play_winrate = Some(mean);
            tracing::info!(
                pool_size = entries.len(),
                snapshot_count = pe.pool.snapshot_count(),
                self_play_winrate = mean,
                "phase3: self-play health metric"
            );
        }
    }

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
        let cfg = Phase3Config::smoke(tmp.clone());
        let report = run_phase3(Device::Cpu, A1, cfg).unwrap();
        assert_eq!(report.losses.len(), 2);
        for s in &report.losses {
            assert!(s.total.is_finite(), "loss must be finite: {}", s.total);
        }
        assert!(!report.evals.is_empty(), "should have at least one eval");
        let fe = report.final_eval.expect("final eval");
        assert!((0.0..=1.0).contains(&fe.mean_win_rate));

        // Keep-best: best_eval is set and is >= every periodic eval (the final
        // eval is also a candidate, so best can exceed any recorded periodic
        // value but never fall below the periodic max).
        let (_, best_wr) = report.best_eval.expect("best_eval set after >=1 eval");
        let periodic_max = report.evals.iter().map(|(_, w)| *w).fold(f32::MIN, f32::max);
        assert!(
            best_wr >= periodic_max - 1e-6,
            "best_wr {best_wr} should be >= periodic max {periodic_max}"
        );
        assert!(best_wr >= fe.mean_win_rate - 1e-6, "best must also cover final eval");
        // The best checkpoint file must have been written.
        assert!(
            tmp.join("hac_best.safetensors").exists(),
            "hac_best.safetensors should exist after training"
        );
    }

    /// Fast self-play smoke: self_play on, snapshot_every=1, 3 iters, 2 envs,
    /// eval_every > iters so no full match eval runs. Verifies >=1 snapshot file
    /// and `self_play_winrate.is_some()`. Must finish in well under a minute.
    #[test]
    fn phase3_self_play_smoke_adds_snapshots() {
        let tmp = std::env::temp_dir().join("sp1_phase3_self_play_smoke");
        std::fs::create_dir_all(&tmp).unwrap();
        let mut cfg = Phase3Config::smoke(tmp.clone());
        // Minimal: 3 iters, 2 envs, 2 rollout cycles — no full-match eval
        cfg.iterations = 3;
        cfg.n_envs = 2;
        cfg.rollout_cycles = 2;
        cfg.eval_every = 0; // 0 → skip all evals (periodic + final) for speed
        cfg.matches_per_eval = 1;
        // SP1 knobs
        cfg.self_play_enabled = true;
        cfg.snapshot_every = 1;
        cfg.pool_cap = 8;
        cfg.warm_start_snapshot = None;

        let report = run_phase3(Device::Cpu, A1, cfg).unwrap();

        // At least one snapshot_*.safetensors file written
        let snaps: Vec<_> = std::fs::read_dir(&tmp)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("snapshot_"))
            .collect();
        assert!(!snaps.is_empty(), "expected >=1 snapshot_*.safetensors in {}", tmp.display());

        // Health metric must be present
        assert!(report.self_play_winrate.is_some(), "self_play_winrate should be Some when self_play_enabled");
        let wr = report.self_play_winrate.unwrap();
        assert!((0.0..=1.0).contains(&wr), "self_play_winrate={wr} out of [0,1]");
    }
}

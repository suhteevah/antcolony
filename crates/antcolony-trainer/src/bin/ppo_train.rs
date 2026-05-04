//! PPO training driver — pure-Rust trainer. Runs the colony sim
//! IN-PROCESS (no subprocess overhead), trains via PPO with GAE,
//! exports weights in MlpBrain JSON format (Rust sim loads them via
//! the existing `mlp:<path>` spec).
//!
//! Usage:
//!   cargo run --release --bin ppo-train -- --iterations 30 --out bench/ppo-rust-r1
//!
//! With CUDA:
//!   cargo run --release --features antcolony-trainer/cuda --bin ppo-train -- ...

use antcolony_trainer::{CandleBackend, PpoConfig, PpoTrainer, Backend};
use antcolony_trainer::ppo::RolloutBatch;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_trainer=info,ppo_train=info")
        .with_target(false)
        .init();

    let mut iterations = 5_usize;
    let mut matches_per_iter = 8_usize;
    let mut out_dir = PathBuf::from("bench/ppo-rust-r1");
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--iterations" => { iterations = raw[i+1].parse()?; i += 2; }
            "--matches-per-iter" => { matches_per_iter = raw[i+1].parse()?; i += 2; }
            "--out" => { out_dir = PathBuf::from(&raw[i+1]); i += 2; }
            other => anyhow::bail!("unknown arg `{other}`"),
        }
    }
    std::fs::create_dir_all(&out_dir)?;

    let backend = CandleBackend::new()?;
    tracing::info!(cuda = backend.cuda_available(), "backend ready");

    let mut config = PpoConfig::default();
    config.iterations = iterations;
    config.matches_per_iter = matches_per_iter;

    let mut trainer = PpoTrainer::new(backend.device().clone(), config.clone())?;

    // Initial export so we can verify shape immediately.
    let weights_path = out_dir.join("current.json");
    trainer.export_mlp_weights(&weights_path)?;
    tracing::info!(path = %weights_path.display(), "initial weights exported");

    // === Training loop: rollout → GAE → PPO update → log ===
    let mut optimizer = trainer.make_optimizer()?;
    let n_opps = trainer.league.entries.len();
    for it in 1..=iterations {
        // Aggregate rollouts across all matches in this iteration
        let mut all_states: Vec<candle_core::Tensor> = Vec::new();
        let mut all_actions: Vec<candle_core::Tensor> = Vec::new();
        let mut all_returns: Vec<f32> = Vec::new();
        let mut all_advantages: Vec<f32> = Vec::new();
        let mut all_log_probs: Vec<f32> = Vec::new();
        let mut total_reward = 0.0_f32;

        for m in 0..matches_per_iter {
            let opp_idx = (it * matches_per_iter + m) % n_opps;
            let opp_spec = trainer.league.entries[opp_idx].spec.clone();
            let seed = (10_000u64 * it as u64) + m as u64;
            let batch: RolloutBatch = trainer.rollout(&opp_spec, seed)?;
            total_reward += batch.rewards.iter().sum::<f32>();
            // GAE per episode
            let (adv, ret) = PpoTrainer::compute_gae(
                &batch.rewards, &batch.values, &batch.dones,
                trainer.config.gamma, trainer.config.gae_lambda,
            );
            all_states.extend(batch.states);
            all_actions.extend(batch.actions);
            all_log_probs.extend(batch.log_probs);
            all_advantages.extend(adv);
            all_returns.extend(ret);
        }

        let n = all_states.len();
        if n == 0 {
            tracing::warn!(it, "no trajectories — skipping update");
            continue;
        }

        let loss = trainer.ppo_update(
            &mut optimizer,
            &all_states, &all_actions,
            &all_returns, &all_advantages, &all_log_probs,
        )?;

        tracing::info!(it, n_samples = n, avg_reward = total_reward / matches_per_iter as f32, loss, "iter done");

        // Export weights so the league can sample them as opponents next round
        trainer.export_mlp_weights(&weights_path)?;
    }

    trainer.export_mlp_weights(&weights_path)?;
    tracing::info!(path = %weights_path.display(), "final weights exported");
    Ok(())
}

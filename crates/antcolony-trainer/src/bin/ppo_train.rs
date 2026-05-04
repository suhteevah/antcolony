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

    // === Smoke loop: run one rollout per opponent and report shapes ===
    use rand::seq::SliceRandom;
    let mut rng = rand_chacha::ChaCha8Rng::from_seed([7u8; 32]);
    use rand::SeedableRng;
    let _ = rng;  // prep

    for it in 1..=iterations {
        let mut total_steps = 0_usize;
        let mut total_reward = 0.0_f32;
        let n_opps = trainer.league.entries.len();
        for m in 0..matches_per_iter {
            let opp_idx = (it * matches_per_iter + m) % n_opps;
            let opp_spec = trainer.league.entries[opp_idx].spec.clone();
            let seed = (10_000u64 * it as u64) + m as u64;
            let batch = trainer.rollout(&opp_spec, seed)?;
            total_steps += batch.states.len();
            total_reward += batch.rewards.iter().sum::<f32>();
        }
        tracing::info!(it, total_steps, avg_reward = total_reward / matches_per_iter as f32, "rollout iteration done");
        // PPO update placeholder — to be wired next.
    }

    trainer.export_mlp_weights(&weights_path)?;
    tracing::info!(path = %weights_path.display(), "final weights exported");
    Ok(())
}

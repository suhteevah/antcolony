//! Phase 2b-2 joint-PPO smoke runner. Builds the A1 hierarchical brain,
//! runs `JointPpoConfig::smoke_default()` (5 iters, 2 matches/iter,
//! 8 cycles/match) on CPU f32, and logs per-iteration losses.
//!
//! Usage:
//!   cargo run --release --bin joint_smoke
//!
//! CUDA is intentionally NOT used: kokonoe has no MSVC linker so the
//! candle `cuda` feature does not build (see the Phase 2b-2 plan,
//! "Device & precision"). Multi-GPU fp16 is Phase 3 on the cnc P100s.

use antcolony_trainer::{JointPpoConfig, JointPpoTrainer};
use antcolony_trainer::hierarchical::sizing::A1;
use candle_core::Device;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_trainer=info,joint_smoke=info")
        .with_target(false)
        .init();

    let cfg = JointPpoConfig::smoke_default();
    tracing::info!(?cfg, "starting joint PPO smoke (CPU f32, A1)");

    let mut trainer = JointPpoTrainer::new(Device::Cpu, A1, cfg)?;
    let stats = trainer.train()?;

    for (i, s) in stats.iter().enumerate() {
        tracing::info!(iter = i, total = s.total, commander = s.commander, ant = s.ant, "iter summary");
    }
    let all_finite = stats.iter().all(|s| s.total.is_finite());
    tracing::info!(iters = stats.len(), all_finite, "joint PPO smoke complete");
    anyhow::ensure!(all_finite, "smoke produced a non-finite loss");
    Ok(())
}

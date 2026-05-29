//! Phase 2b-2 joint-PPO smoke runner. Builds the A1 hierarchical brain,
//! runs `JointPpoConfig::smoke_default()` (5 iters, 2 matches/iter,
//! 8 cycles/match), and logs per-iteration losses.
//!
//! Device is chosen by `CandleBackend::new()`: CUDA device 0 when built
//! with `--features cuda`, otherwise CPU f32.
//!
//! Usage:
//!   CPU:  cargo run --release --bin joint_smoke
//!   CUDA: scripts/build_trainer_cuda.bat then run the cuda binary, or
//!         scripts/run_joint_smoke_cuda.bat (sets up the BuildTools MSVC
//!         env + link.exe override that the candle cuda build needs on
//!         kokonoe — see that script for why).

use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::{Backend, CandleBackend, JointPpoConfig, JointPpoTrainer};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_trainer=info,joint_smoke=info")
        .with_target(false)
        .init();

    let cfg = JointPpoConfig::smoke_default();
    // CUDA device 0 when compiled with --features cuda, else CPU.
    let backend = CandleBackend::new()?;
    let device = backend.device().clone();
    tracing::info!(cuda = backend.cuda_available(), ?cfg, "starting joint PPO smoke (A1)");

    let mut trainer = JointPpoTrainer::new(device, A1, cfg)?;
    let stats = trainer.train()?;

    for (i, s) in stats.iter().enumerate() {
        tracing::info!(iter = i, total = s.total, commander = s.commander, ant = s.ant, "iter summary");
    }
    let all_finite = stats.iter().all(|s| s.total.is_finite());
    tracing::info!(iters = stats.len(), all_finite, "joint PPO smoke complete");
    anyhow::ensure!(all_finite, "smoke produced a non-finite loss");
    Ok(())
}

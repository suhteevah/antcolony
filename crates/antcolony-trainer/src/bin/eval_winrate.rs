//! Score a trained A1 HAC checkpoint on the eval.rs 7-archetype win-rate metric
//! at an arbitrary matches-per-opp (default 50 = the "honest", de-noised count;
//! training evals use mpe=5 which is noisy). Same harness/metric that produced
//! the in-training win-rate numbers, so it is directly comparable. CPU-only —
//! needs no GPU, safe to run without freeing the fleet's P100.
//!
//! Usage:
//!   eval_winrate <checkpoint.safetensors> [matches_per_opp]
//!   (defaults: bench/phase3-a1-combat/hac_best.safetensors, mpe=50)

use antcolony_trainer::eval::evaluate_hac;
use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::{JointPpoConfig, JointPpoTrainer};
use candle_core::Device;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_trainer=info,eval_winrate=info")
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let ckpt = args
        .first()
        .cloned()
        .unwrap_or_else(|| "bench/phase3-a1-combat/hac_best.safetensors".to_string());
    let mpe: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(50);

    let device = Device::Cpu;
    let mut trainer = JointPpoTrainer::new(device.clone(), A1, JointPpoConfig::smoke_default())?;
    trainer.varmap.load(&ckpt)?;
    tracing::info!(ckpt, mpe, "loaded A1 checkpoint; scoring win-rate vs 7-archetype bench");

    let report = evaluate_hac(&trainer.hac, &device, mpe)?;

    println!("=== HONEST eval (mpe={mpe}) :: {ckpt} ===");
    for (a, wr) in &report.per_archetype {
        println!("{a:<14} {wr:.4}");
    }
    println!("{:<14} {:.4}", "MEAN", report.mean_win_rate);
    tracing::info!(
        mean_win_rate = report.mean_win_rate,
        mpe,
        ckpt,
        "HONEST eval done"
    );
    Ok(())
}

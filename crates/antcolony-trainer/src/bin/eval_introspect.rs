//! Introspect a trained A1 HAC checkpoint: play it vs combat archetypes and
//! log what it COMMANDS (caste ratios — does it ever build soldiers?) and HOW
//! it loses (final caste counts, queen health, win/loss/timeout). CPU-only, so
//! it needs no GPU — safe to run without freeing the fleet's P100.
//!
//! Usage:
//!   eval_introspect <checkpoint.safetensors> [matches] [opp1,opp2,...]
//!   (default 8 matches vs aggressor,defender)

use antcolony_trainer::eval::evaluate_hac_introspect;
use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::{JointPpoConfig, JointPpoTrainer};
use candle_core::Device;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_trainer=info,eval_introspect=info")
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let ckpt = args
        .first()
        .cloned()
        .unwrap_or_else(|| "bench/phase3-a1-fullhorizon/hac_iter0075.safetensors".to_string());
    let matches: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(8);
    let opps: Vec<String> = args
        .get(2)
        .map(|s| s.split(',').map(|x| x.to_string()).collect())
        .unwrap_or_else(|| vec!["aggressor".to_string(), "defender".to_string()]);

    // CPU device: introspection is a handful of matches; no GPU needed.
    let device = Device::Cpu;
    let mut trainer = JointPpoTrainer::new(device.clone(), A1, JointPpoConfig::smoke_default())?;
    trainer.varmap.load(&ckpt)?;
    tracing::info!(ckpt, matches, ?opps, "loaded A1 checkpoint; introspecting");

    for opp in &opps {
        evaluate_hac_introspect(&trainer.hac, &device, opp, matches)?;
    }
    Ok(())
}

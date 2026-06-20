//! Head-to-head eval: play two trained A1 HAC checkpoints directly against
//! each other and report A's win-rate vs B under both metrics.
//!
//! Usage:
//!   eval_h2h <ckptA> <ckptB> [mpe=50]
//!
//! Plays `mpe` matches A=left/B=right, then `mpe` matches B=left/A=right (to
//! cancel positional bias). Reports A's combined win-rate. CPU-only — no GPU
//! needed. >0.5 means A beats B.

use antcolony_trainer::eval::{evaluate_h2h, H2HReport};
use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::self_play::load_frozen_hac;
use candle_core::Device;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_trainer=info,eval_h2h=info")
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let ckpt_a = args.first().ok_or_else(|| anyhow::anyhow!("Usage: eval_h2h <ckptA> <ckptB> [mpe]"))?;
    let ckpt_b = args.get(1).ok_or_else(|| anyhow::anyhow!("Usage: eval_h2h <ckptA> <ckptB> [mpe]"))?;
    let mpe: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(50);

    let device = Device::Cpu;

    tracing::info!(ckpt_a, ckpt_b, mpe, "eval_h2h: loading checkpoints");
    let hac_a = load_frozen_hac(std::path::Path::new(ckpt_a), A1, &device)?;
    let hac_b = load_frozen_hac(std::path::Path::new(ckpt_b), A1, &device)?;
    tracing::info!("both checkpoints loaded; starting head-to-head eval");

    let report: H2HReport = evaluate_h2h(&hac_a, &hac_b, &device, mpe)?;

    println!("A={ckpt_a}  B={ckpt_b}  mpe={mpe}");
    println!(
        "A head-to-head win-rate vs B:  worker_share={:.3}  decisive={:.3}",
        report.a_winrate_ws, report.a_winrate_decisive
    );
    println!(
        "  (A as left: ws={:.3}, A as right: ws={:.3})  [>0.5 means A beats B]",
        report.a_as_left_ws, report.a_as_right_ws
    );
    println!("  total matches: {}", report.matches);

    tracing::info!(
        ckpt_a,
        ckpt_b,
        mpe,
        a_winrate_ws = report.a_winrate_ws,
        a_winrate_decisive = report.a_winrate_decisive,
        a_as_left_ws = report.a_as_left_ws,
        a_as_right_ws = report.a_as_right_ws,
        matches = report.matches,
        "eval_h2h complete"
    );

    Ok(())
}

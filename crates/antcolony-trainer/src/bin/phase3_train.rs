//! Phase-3 single-GPU training runner. Trains the A1 hierarchical brain
//! left-vs-league on CUDA (kokonoe 3070 Ti), evaluating vs the 7-archetype
//! bench, with a tunable reward shaper loaded from a TOML file.
//!
//! Build/run (CUDA): see scripts/build_trainer_cuda.bat. Example:
//!   cargo +stable-x86_64-pc-windows-msvc run --release --features cuda \
//!     --bin phase3_train -- --iters 200 --envs 64 --eval-every 25 \
//!     --reward assets/reward/default.toml --out bench/phase3-a1
//!
//! The reward TOML mirrors RewardConfig fields; omitted fields take r6
//! defaults. To "thumb up smartness", set e.g. brood_growth / food_inflow.
//!
//! `--ant-chunk-size N` (default 0 = off) bounds peak GPU memory in the ant
//! tier's update by forwarding/backward-ing at most N ant rows at a time and
//! accumulating their gradients before one optimizer step (identical math).
//! This is what lets `--rollout-cycles` span full-match horizons — so the
//! policy experiences combat + terminal reward — without the update OOMing.
//! (Note: the *stored* rollout still grows with envs × cycles, so trade those
//! off against card memory; the chunk size only fixes the update-side ceiling.)

use antcolony_trainer::hierarchical::sizing::{A1, A2};
use antcolony_trainer::{Backend, CandleBackend, Phase3Config, RewardConfig, run_phase3};
use antcolony_trainer::JointPpoConfig;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_trainer=info,phase3_train=info")
        .with_target(false)
        .init();

    let mut iters = 200usize;
    let mut envs = 64usize;
    let mut rollout_cycles = 32usize;
    let mut eval_every = 25usize;
    let mut matches_per_eval = 50usize;
    let mut ant_chunk_size = 0usize; // 0 = monolithic ant update; >0 bounds peak GPU memory
    let mut sizing_name = "a1".to_string(); // a1 (default) | a2
    let mut reward_path: Option<PathBuf> = None;
    let mut out_dir = PathBuf::from("bench/phase3-a1");

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let next = || args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--iters" => { iters = next().parse().unwrap_or(iters); i += 2; }
            "--envs" => { envs = next().parse().unwrap_or(envs); i += 2; }
            "--rollout-cycles" => { rollout_cycles = next().parse().unwrap_or(rollout_cycles); i += 2; }
            "--eval-every" => { eval_every = next().parse().unwrap_or(eval_every); i += 2; }
            "--matches-per-eval" => { matches_per_eval = next().parse().unwrap_or(matches_per_eval); i += 2; }
            "--ant-chunk-size" => { ant_chunk_size = next().parse().unwrap_or(ant_chunk_size); i += 2; }
            "--sizing" => { sizing_name = next(); i += 2; }
            "--reward" => { reward_path = Some(PathBuf::from(next())); i += 2; }
            "--out" => { out_dir = PathBuf::from(next()); i += 2; }
            other => { tracing::warn!(arg = other, "unknown flag, ignoring"); i += 1; }
        }
    }

    let reward = match &reward_path {
        Some(p) => {
            let txt = std::fs::read_to_string(p)?;
            let r: RewardConfig = toml::from_str(&txt)?;
            tracing::info!(path = %p.display(), ?r, "loaded reward config");
            r
        }
        None => {
            tracing::info!("no --reward file; using r6 defaults");
            RewardConfig::default()
        }
    };

    let backend = CandleBackend::new()?;
    let device = backend.device().clone();
    let sizing = match sizing_name.as_str() {
        "a2" | "A2" => A2,
        _ => A1,
    };
    tracing::info!(
        cuda = backend.cuda_available(), iters, envs, rollout_cycles, ant_chunk_size,
        sizing = %sizing_name,
        "phase3 start"
    );

    let mut joint = JointPpoConfig::smoke_default();
    joint.ant_chunk_size = ant_chunk_size;

    let cfg = Phase3Config {
        iterations: iters,
        n_envs: envs,
        rollout_cycles,
        eval_every,
        matches_per_eval,
        reward,
        joint,
        out_dir,
    };

    let report = run_phase3(device, sizing, cfg)?;

    tracing::info!("=== Phase 3 win-rate curve (iter, mean) ===");
    for (it, wr) in &report.evals {
        tracing::info!(iter = it, mean_win_rate = wr, "curve");
    }
    if let Some(fe) = &report.final_eval {
        tracing::info!(mean_win_rate = fe.mean_win_rate, baseline = 0.471, "FINAL vs 47.1% baseline");
        for (name, wr) in &fe.per_archetype {
            tracing::info!(archetype = name, win_rate = wr, "final per-archetype");
        }
    }
    Ok(())
}

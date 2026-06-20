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
use antcolony_trainer::{Backend, CandleBackend, OpponentSampler, Phase3Config, RewardConfig, run_phase3};
use antcolony_trainer::JointPpoConfig;
use std::path::PathBuf;

/// Parse a CLI flag value or exit(2) loudly. L8: prevents a typo'd value from
/// silently running the default config.
fn parse_or_exit<T>(flag: &str, raw: &str) -> T
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match raw.parse() {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(flag, value = raw, error = %e, "failed to parse CLI flag value");
            std::process::exit(2);
        }
    }
}

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
    let mut max_grad_norm = 0.5f64; // PPO-standard global grad-norm clip; 0 = off
    let mut early_stop_patience = 0usize; // 0 = run all iters; N = stop after N evals w/o improvement
    let mut sizing_name = "a1".to_string(); // a1 (default) | a2
    let mut reward_path: Option<PathBuf> = None;
    let mut out_dir = PathBuf::from("bench/phase3-a1");
    // SP1 self-play flags (all default to Phase3Config defaults so existing invocations are unchanged)
    let mut self_play_enabled = false;
    let mut snapshot_every = 25usize;
    let mut pool_cap = 8usize;
    let mut opponent_sampling_str = "pfsp".to_string();
    let mut archetype_mix = 0.5f32;
    let mut warm_start_snapshot: Option<PathBuf> = None;
    let mut warm_start_policy: Option<PathBuf> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let next = || args.get(i + 1).cloned().unwrap_or_default();
        // L8: a typo'd value (e.g. `--iters twenty`) must NOT silently fall back
        // to the default — that would run a different config than intended.
        // `parse_or_exit` logs loudly and exit(2)s on a parse failure.
        match args[i].as_str() {
            "--iters" => { iters = parse_or_exit("--iters", &next()); i += 2; }
            "--envs" => { envs = parse_or_exit("--envs", &next()); i += 2; }
            "--rollout-cycles" => { rollout_cycles = parse_or_exit("--rollout-cycles", &next()); i += 2; }
            "--eval-every" => { eval_every = parse_or_exit("--eval-every", &next()); i += 2; }
            "--matches-per-eval" => { matches_per_eval = parse_or_exit("--matches-per-eval", &next()); i += 2; }
            "--ant-chunk-size" => { ant_chunk_size = parse_or_exit("--ant-chunk-size", &next()); i += 2; }
            "--max-grad-norm" => { max_grad_norm = parse_or_exit("--max-grad-norm", &next()); i += 2; }
            "--early-stop-patience" => { early_stop_patience = parse_or_exit("--early-stop-patience", &next()); i += 2; }
            "--sizing" => { sizing_name = next(); i += 2; }
            "--reward" => { reward_path = Some(PathBuf::from(next())); i += 2; }
            "--out" => { out_dir = PathBuf::from(next()); i += 2; }
            // SP1 self-play flags
            "--self-play" => { self_play_enabled = true; i += 1; }
            "--snapshot-every" => { snapshot_every = parse_or_exit("--snapshot-every", &next()); i += 2; }
            "--pool-cap" => { pool_cap = parse_or_exit("--pool-cap", &next()); i += 2; }
            "--opponent-sampling" => { opponent_sampling_str = next(); i += 2; }
            "--archetype-mix" => { archetype_mix = parse_or_exit("--archetype-mix", &next()); i += 2; }
            "--warm-start-snapshot" => { warm_start_snapshot = Some(PathBuf::from(next())); i += 2; }
            "--warm-start-policy" => { warm_start_policy = Some(PathBuf::from(next())); i += 2; }
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

    let opponent_sampling = match opponent_sampling_str.as_str() {
        "uniform" => OpponentSampler::Uniform,
        "pfsp" => OpponentSampler::Pfsp { archetype_mix, power: 1.0 },
        other => {
            tracing::error!(value = other, "--opponent-sampling must be 'uniform' or 'pfsp'");
            std::process::exit(2);
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
        max_grad_norm, early_stop_patience, sizing = %sizing_name,
        self_play = self_play_enabled, snapshot_every, pool_cap,
        opponent_sampling = %opponent_sampling_str, archetype_mix,
        warm_start_snapshot = ?warm_start_snapshot,
        warm_start_policy = ?warm_start_policy,
        "phase3 start"
    );

    let mut joint = JointPpoConfig::smoke_default();
    joint.ant_chunk_size = ant_chunk_size;
    joint.max_grad_norm = max_grad_norm;

    let cfg = Phase3Config {
        iterations: iters,
        n_envs: envs,
        rollout_cycles,
        eval_every,
        matches_per_eval,
        early_stop_patience,
        reward,
        joint,
        out_dir,
        self_play_enabled,
        snapshot_every,
        pool_cap,
        opponent_sampling,
        warm_start_snapshot,
        warm_start_policy,
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
    if let Some((bi, bw)) = report.best_eval {
        tracing::info!(
            best_iter = bi, best_win_rate = bw, baseline = 0.471,
            "BEST checkpoint -> hac_best.safetensors (ship this one)"
        );
    }
    Ok(())
}

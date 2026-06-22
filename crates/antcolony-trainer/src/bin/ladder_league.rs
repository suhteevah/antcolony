//! Ladder League CLI: iterated best-response vs the frozen tournament ladder.
//! Built --features cuda for cnc P100 training; the gate/tournament run on the
//! same device (use CPU for the gate by passing --gate-cpu if desired later).

use std::path::PathBuf;
use anyhow::Result;
use antcolony_trainer::{Backend, CandleBackend, JointPpoConfig, RewardConfig};
use antcolony_trainer::ladder_league::{LadderConfig, LadderContender, LadderLeague};
use antcolony_trainer::hierarchical::sizing::A1;

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))).init();

    // Defaults (approved).
    let mut sota_path: Option<PathBuf> = None;
    let mut contender_specs: Vec<String> = Vec::new(); // "id=hac:path"
    let mut iters_per_round = 150usize;
    let mut eval_every = 10usize;
    let mut train_mpe = 5usize;
    let mut gate_mpe = 50usize;
    let mut gate_margin = 0.55f32;
    let mut keepbest_arch_floor = 0.70f32;
    let mut archetype_mix = 0.30f32;
    let mut pfsp_power = 1.0f32;
    let mut no_improve_stop = 2usize;
    let mut max_rounds = 8usize;
    let mut rollout_cycles = 96usize;
    let mut matches_per_iter = 8usize;
    let mut ant_chunk_size = 0usize;
    let mut max_grad_norm = 0.5f64;
    let mut reward_path: Option<PathBuf> = None;
    let mut out_dir = PathBuf::from("bench/ladder-league");

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = || args.next().expect("flag needs a value");
        match a.as_str() {
            "--sota" => sota_path = Some(PathBuf::from(next())),
            "--contender" => contender_specs.push(next()),     // repeatable: id=hac:path
            "--iters-per-round" => iters_per_round = next().parse()?,
            "--eval-every" => eval_every = next().parse()?,
            "--train-mpe" => train_mpe = next().parse()?,
            "--gate-mpe" => gate_mpe = next().parse()?,
            "--gate-margin" => gate_margin = next().parse()?,
            "--keepbest-arch-floor" => keepbest_arch_floor = next().parse()?,
            "--archetype-mix" => archetype_mix = next().parse()?,
            "--pfsp-power" => pfsp_power = next().parse()?,
            "--no-improve-stop" => no_improve_stop = next().parse()?,
            "--max-rounds" => max_rounds = next().parse()?,
            "--rollout-cycles" => rollout_cycles = next().parse()?,
            "--matches-per-iter" => matches_per_iter = next().parse()?,
            "--ant-chunk-size" => ant_chunk_size = next().parse()?,
            "--max-grad-norm" => max_grad_norm = next().parse()?,
            "--reward" => reward_path = Some(PathBuf::from(next())),
            "--out" => out_dir = PathBuf::from(next()),
            other => tracing::warn!(arg = other, "unknown flag, ignoring"),
        }
    }

    let sota_path = match sota_path {
        Some(p) => p,
        None => { tracing::error!("--sota <path> is required"); std::process::exit(2); }
    };

    // Parse contenders: "id=hac:path". The SOTA is auto-added as id "sota".
    let mut initial_contenders = vec![LadderContender { id: "sota".into(), spec: format!("hac:{}", sota_path.display()) }];
    for s in &contender_specs {
        let (id, spec) = s.split_once('=').expect("contender must be id=spec");
        initial_contenders.push(LadderContender { id: id.to_string(), spec: spec.to_string() });
    }

    let reward = match &reward_path {
        Some(p) => { let txt = std::fs::read_to_string(p)?; let r: RewardConfig = toml::from_str(&txt)?;
                     tracing::info!(path=%p.display(), ?r, "loaded reward"); r }
        None => { tracing::info!("no --reward; r6 defaults"); RewardConfig::default() }
    };

    let backend = CandleBackend::new()?;
    let device = backend.device().clone();

    let mut joint = JointPpoConfig::smoke_default();
    joint.rollout_cycles = rollout_cycles;
    joint.matches_per_iter = matches_per_iter;
    joint.ant_chunk_size = ant_chunk_size;
    joint.max_grad_norm = max_grad_norm;

    let cfg = LadderConfig {
        sota_path, initial_contenders, iters_per_round, eval_every, train_mpe, gate_mpe,
        gate_margin, keepbest_arch_floor, archetype_mix, pfsp_power, no_improve_stop, max_rounds,
        out_dir, sizing: A1, joint, reward,
    };
    tracing::info!(?cfg, "ladder_league: starting");
    let mut league = LadderLeague::new(cfg, device)?;
    let report = league.run()?;
    tracing::info!(rounds = report.rounds_run, promotions = report.promotions,
                   final_sota = %report.final_sota_path.display(), best_h2h = report.best_h2h_over_seed,
                   reason = %report.stopped_reason, "ladder_league: DONE");
    println!("LADDER_DONE rounds={} promotions={} reason={} final_sota={}",
             report.rounds_run, report.promotions, report.stopped_reason, report.final_sota_path.display());
    Ok(())
}

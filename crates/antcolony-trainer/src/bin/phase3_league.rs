//! Phase-3 SP2 exploiter-league runner. Runs a [`LeagueManager`] round-robin
//! on a single GPU (or CPU for smoke tests), warm-starting all agents from a
//! SOTA checkpoint and promoting exploiters that beat the target win-rate.
//!
//! Build/run (CUDA): see scripts/run_league_cnc.sh. Example (15-step pilot):
//!   ./target/release/phase3_league \
//!     --league-steps 15 --iters-main 25 --iters-exploiter 15 \
//!     --n-main-exploiters 1 --n-league-exploiters 1 --pool-cap 16 \
//!     --promote-winrate 0.70 --exploiter-max-iters 100 --main-snapshot-every 2 \
//!     --archetype-mix 0.5 --eval-every-steps 5 --success-mpe 20 \
//!     --rollout-cycles 96 --ant-chunk-size 8192 --max-grad-norm 0.5 \
//!     --sota bench/phase3-a1-combat/hac_best.safetensors \
//!     --reward assets/reward/terminal.toml --out bench/phase3-sp2
//!
//! `--sota` is REQUIRED (error + exit(2) if absent).
//! `--out` defaults to `bench/phase3-sp2`.
//!
//! `--rollout-cycles 96 --ant-chunk-size 8192` (proven SP1 values) ensure the
//! policy experiences terminal reward in every rollout without OOMing the ant
//! tier update.

use antcolony_trainer::exploiter_league::{LeagueConfig, LeagueManager};
use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::{Backend, CandleBackend, JointPpoConfig, RewardConfig};
use std::path::PathBuf;

/// Parse a CLI flag value or exit(2) loudly. Prevents a typo'd value (e.g.
/// `--league-steps twenty`) from silently running the default config.
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
        .with_env_filter(
            "antcolony_sim=warn,antcolony_trainer=info,phase3_league=info",
        )
        .with_target(false)
        .init();

    // ── Defaults (spec §6) ────────────────────────────────────────────────
    let mut league_steps: usize = 40;
    let mut iters_main: usize = 25;
    let mut iters_exploiter: usize = 15;
    let mut n_main_exploiters: usize = 1;
    let mut n_league_exploiters: usize = 1;
    let mut pool_cap: usize = 16;
    let mut promote_winrate: f32 = 0.70;
    let mut exploiter_max_iters: usize = 100;
    let mut main_snapshot_every: usize = 2;
    let mut archetype_mix: f32 = 0.5;
    let mut eval_every_steps: usize = 5;
    let mut success_mpe: usize = 20;
    // Joint PPO controller flags (proven SP1 values as defaults).
    let mut rollout_cycles: usize = 32;
    let mut ant_chunk_size: usize = 0; // 0 = monolithic; >0 bounds peak GPU memory
    let mut max_grad_norm: f64 = 0.5;
    // Paths.
    let mut sota_path: Option<PathBuf> = None; // REQUIRED — error+exit(2) if absent
    let mut reward_path: Option<PathBuf> = None;
    let mut out_dir = PathBuf::from("bench/phase3-sp2");

    // ── Hand-rolled parser (mirrors phase3_train idiom) ───────────────────
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let next = || args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--league-steps" => {
                league_steps = parse_or_exit("--league-steps", &next());
                i += 2;
            }
            "--iters-main" => {
                iters_main = parse_or_exit("--iters-main", &next());
                i += 2;
            }
            "--iters-exploiter" => {
                iters_exploiter = parse_or_exit("--iters-exploiter", &next());
                i += 2;
            }
            "--n-main-exploiters" => {
                n_main_exploiters = parse_or_exit("--n-main-exploiters", &next());
                i += 2;
            }
            "--n-league-exploiters" => {
                n_league_exploiters = parse_or_exit("--n-league-exploiters", &next());
                i += 2;
            }
            "--pool-cap" => {
                pool_cap = parse_or_exit("--pool-cap", &next());
                i += 2;
            }
            "--promote-winrate" => {
                promote_winrate = parse_or_exit("--promote-winrate", &next());
                i += 2;
            }
            "--exploiter-max-iters" => {
                exploiter_max_iters = parse_or_exit("--exploiter-max-iters", &next());
                i += 2;
            }
            "--main-snapshot-every" => {
                main_snapshot_every = parse_or_exit("--main-snapshot-every", &next());
                i += 2;
            }
            "--archetype-mix" => {
                archetype_mix = parse_or_exit("--archetype-mix", &next());
                i += 2;
            }
            "--eval-every-steps" => {
                eval_every_steps = parse_or_exit("--eval-every-steps", &next());
                i += 2;
            }
            "--success-mpe" => {
                success_mpe = parse_or_exit("--success-mpe", &next());
                i += 2;
            }
            // Controller flags (wired into joint).
            "--rollout-cycles" => {
                rollout_cycles = parse_or_exit("--rollout-cycles", &next());
                i += 2;
            }
            "--ant-chunk-size" => {
                ant_chunk_size = parse_or_exit("--ant-chunk-size", &next());
                i += 2;
            }
            "--max-grad-norm" => {
                max_grad_norm = parse_or_exit("--max-grad-norm", &next());
                i += 2;
            }
            // Paths.
            "--sota" => {
                sota_path = Some(PathBuf::from(next()));
                i += 2;
            }
            "--reward" => {
                reward_path = Some(PathBuf::from(next()));
                i += 2;
            }
            "--out" => {
                out_dir = PathBuf::from(next());
                i += 2;
            }
            other => {
                tracing::warn!(arg = other, "unknown flag, ignoring");
                i += 1;
            }
        }
    }

    // ── --sota is REQUIRED ────────────────────────────────────────────────
    let sota = match sota_path {
        Some(p) => p,
        None => {
            tracing::error!("--sota <path> is required (warm-start + h2h baseline)");
            std::process::exit(2);
        }
    };

    // ── Reward config ─────────────────────────────────────────────────────
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

    // ── Backend + device ──────────────────────────────────────────────────
    let backend = CandleBackend::new()?;
    let device = backend.device().clone();

    // ── Joint PPO config ──────────────────────────────────────────────────
    let mut joint = JointPpoConfig::smoke_default();
    joint.rollout_cycles = rollout_cycles;
    joint.ant_chunk_size = ant_chunk_size;
    joint.max_grad_norm = max_grad_norm;

    // ── Log all config up-front ───────────────────────────────────────────
    tracing::info!(
        cuda = backend.cuda_available(),
        league_steps,
        iters_main,
        iters_exploiter,
        n_main_exploiters,
        n_league_exploiters,
        pool_cap,
        promote_winrate,
        exploiter_max_iters,
        main_snapshot_every,
        archetype_mix,
        eval_every_steps,
        success_mpe,
        rollout_cycles,
        ant_chunk_size,
        max_grad_norm,
        sota = %sota.display(),
        reward = ?reward_path,
        out = %out_dir.display(),
        "phase3_league start"
    );

    // ── Create output dir ─────────────────────────────────────────────────
    std::fs::create_dir_all(&out_dir).ok();

    // ── Build LeagueConfig ────────────────────────────────────────────────
    let cfg = LeagueConfig {
        league_steps,
        iters_main,
        iters_exploiter,
        n_main_exploiters,
        n_league_exploiters,
        pool_cap,
        exploiter_promote_winrate: promote_winrate,
        exploiter_max_iters,
        main_snapshot_every,
        archetype_mix,
        eval_every_steps,
        success_mpe,
        snapshot_dir: out_dir.clone(),
        sota_path: sota,
        sizing: A1,
        joint,
        reward,
    };

    // ── Warn on eval-cadence misconfiguration ─────────────────────────────
    // If league_steps is not a multiple of eval_every_steps the final step will
    // not be evaluated and league_best.safetensors may not reflect the final
    // (potentially best) weights. Make league_steps a multiple of eval_every_steps
    // to avoid missing the last-step eval.
    if eval_every_steps > 0 && league_steps % eval_every_steps != 0 {
        tracing::warn!(
            league_steps,
            eval_every_steps,
            "league_steps is not a multiple of eval_every_steps — the final \
             league-step will not be evaluated; league_best.safetensors may not \
             reflect the final/best weights. Recommend setting league_steps to a \
             multiple of eval_every_steps."
        );
    }

    // ── Run the league ────────────────────────────────────────────────────
    let mut mgr = LeagueManager::new(cfg, device)?;
    let report = mgr.run()?;

    // ── Print results ─────────────────────────────────────────────────────
    tracing::info!(
        steps = report.steps,
        snapshots_added = report.snapshots_added,
        exploiter_resets = report.exploiter_resets,
        best_h2h_vs_sota = report.best_h2h_vs_sota,
        best_step = report.best_step,
        final_bench = report.final_bench,
        "=== SP2 LEAGUE COMPLETE ==="
    );
    tracing::info!(
        best_h2h_vs_sota = report.best_h2h_vs_sota,
        beats_sota = report.best_h2h_vs_sota > 0.5,
        "BEST -> league_best.safetensors beats SOTA if best_h2h_vs_sota > 0.5"
    );

    Ok(())
}

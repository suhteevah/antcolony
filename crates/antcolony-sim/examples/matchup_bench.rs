//! Matchup bench — head-to-head brain evaluation.
//!
//! Runs N AI-vs-AI matches between two configurable brains. Reports
//! win-rate, average tick-to-end, and (optionally) dumps per-tick
//! `(state, decision, outcome)` trajectories as JSONL for training.
//!
//! # Usage
//!
//! ```text
//! cargo run --release --example matchup_bench -- \
//!     --left heuristic --right random --matches 20 --max-ticks 5000
//!
//! cargo run --release --example matchup_bench -- \
//!     --left heuristic --right heuristic --matches 50 \
//!     --dump-trajectories bench/matchup-trajectories.jsonl
//! ```
//!
//! # Brain selection
//!
//! - `heuristic` — `HeuristicBrain` (baseline)
//! - `random` — `RandomBrain` (noise floor; needs `--right-seed`)
//! - `aether:<path>` — `AetherLmBrain` pointing at a checkpoint (STUB
//!   for now; will log a warning + use safe-default decisions)
//!
//! # Output
//!
//! - stdout: per-match summary line + final aggregate
//! - `--out <dir>`: per-match CSV (tick, ant counts, food, status) +
//!   `SUMMARY.md` with win-rate matrix
//! - `--dump-trajectories <path>`: JSONL of training tuples

use std::path::PathBuf;

use antcolony_sim::{
    AetherLmBrain, AiBrain, AiDecision, ColonyAiState, HeuristicBrain, MatchStatus,
    RandomBrain, Simulation, Topology,
    config::{AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig},
};
use serde::Serialize;

// Decision cadence: brains decide every N outer ticks. 5 is a good
// compromise — fine-grained enough to react to combat losses, coarse
// enough that brain-decide cost is amortized.
const DECISION_CADENCE: u64 = 5;

#[derive(Debug, Clone)]
struct CliArgs {
    left: String,
    right: String,
    left_seed: u64,
    right_seed: u64,
    matches: u32,
    max_ticks: u64,
    out_dir: Option<PathBuf>,
    dump_trajectories: Option<PathBuf>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            left: "heuristic".into(),
            right: "heuristic".into(),
            left_seed: 1,
            right_seed: 2,
            matches: 10,
            max_ticks: 5_000,
            out_dir: None,
            dump_trajectories: None,
        }
    }
}

fn parse_args() -> anyhow::Result<CliArgs> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut a = CliArgs::default();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--left" => { a.left = raw.get(i + 1).cloned().unwrap_or(a.left); i += 2; }
            "--right" => { a.right = raw.get(i + 1).cloned().unwrap_or(a.right); i += 2; }
            "--left-seed" => { a.left_seed = raw.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(a.left_seed); i += 2; }
            "--right-seed" => { a.right_seed = raw.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(a.right_seed); i += 2; }
            "--matches" => { a.matches = raw.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(a.matches); i += 2; }
            "--max-ticks" => { a.max_ticks = raw.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(a.max_ticks); i += 2; }
            "--out" => { a.out_dir = raw.get(i + 1).map(PathBuf::from); i += 2; }
            "--dump-trajectories" => { a.dump_trajectories = raw.get(i + 1).map(PathBuf::from); i += 2; }
            "--help" | "-h" => { print_help(); std::process::exit(0); }
            other => anyhow::bail!("unknown arg `{other}` — try --help"),
        }
    }
    Ok(a)
}

fn print_help() {
    println!(
        "matchup_bench — head-to-head AI brain evaluation\n\n\
         FLAGS:\n  \
           --left <brain>            Brain for colony 0 (default: heuristic)\n  \
           --right <brain>           Brain for colony 1 (default: heuristic)\n  \
           --left-seed <n>           Seed for left brain RNG (default: 1)\n  \
           --right-seed <n>          Seed for right brain RNG (default: 2)\n  \
           --matches <n>             Number of matches to run (default: 10)\n  \
           --max-ticks <n>           Tick cap per match (default: 5000)\n  \
           --out <dir>               Per-match CSV + SUMMARY.md output dir\n  \
           --dump-trajectories <p>   Write training tuples as JSONL\n  \
           -h, --help                Show this help\n\n\
         BRAIN NAMES:\n  \
           heuristic                 HeuristicBrain (baseline)\n  \
           random                    RandomBrain (noise floor)\n  \
           aether:<checkpoint>       AetherLmBrain (STUB)\n"
    );
}

fn build_brain(spec: &str, seed: u64) -> Box<dyn AiBrain> {
    if let Some(rest) = spec.strip_prefix("aether:") {
        return Box::new(AetherLmBrain::new(rest, format!("aether-{seed}")));
    }
    match spec {
        "heuristic" => Box::new(HeuristicBrain::new(5.0)),
        "random" => Box::new(RandomBrain::new(seed)),
        other => panic!("unknown brain spec `{other}` — try heuristic / random / aether:<path>"),
    }
}

fn small_two_colony_config() -> SimConfig {
    SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 10, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    }
}

#[derive(Debug, Clone, Serialize)]
struct Trajectory {
    match_id: u32,
    tick: u64,
    colony: u8,
    state: ColonyAiState,
    decision: AiDecision,
    /// Filled in at match end when we know who won.
    /// 1.0 for the eventual winner, 0.0 for loser, 0.5 on draw.
    outcome_for_this_colony: f32,
}

#[derive(Debug, Clone, Serialize)]
struct MatchSummary {
    match_id: u32,
    seed: u64,
    end_tick: u64,
    status: String,
    winner: Option<u8>,
    final_workers_left: u32,
    final_workers_right: u32,
    final_food_left: f32,
    final_food_right: f32,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,matchup_bench=info")
        .with_target(false)
        .init();

    let args = parse_args()?;
    println!(
        "matchup_bench: {} matches, left={} (seed={}) vs right={} (seed={}), max-ticks={}",
        args.matches, args.left, args.left_seed, args.right, args.right_seed, args.max_ticks,
    );

    if let Some(d) = &args.out_dir {
        std::fs::create_dir_all(d)?;
    }

    let mut summaries: Vec<MatchSummary> = Vec::new();
    let mut all_trajectories: Vec<Trajectory> = Vec::new();
    let mut left_wins = 0u32;
    let mut right_wins = 0u32;
    let mut draws = 0u32;
    let mut timeouts = 0u32;

    for m in 0..args.matches {
        // Per-match seed varies so each match has different RNG, while
        // brain seeds stay tied to the brain side (reproducibility per side).
        let sim_seed = 100 + m as u64;
        let mut left = build_brain(&args.left, args.left_seed);
        let mut right = build_brain(&args.right, args.right_seed);

        let cfg = small_two_colony_config();
        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, sim_seed, 0, 2);

        let mut trajectories: Vec<Trajectory> = Vec::new();
        let mut final_status = MatchStatus::InProgress;

        for _ in 0..args.max_ticks {
            if sim.tick % DECISION_CADENCE == 0 {
                if let Some(s0) = sim.colony_ai_state(0) {
                    let d = left.decide(&s0);
                    if args.dump_trajectories.is_some() {
                        trajectories.push(Trajectory {
                            match_id: m,
                            tick: sim.tick,
                            colony: 0,
                            state: s0,
                            decision: d.clone(),
                            outcome_for_this_colony: 0.5, // patched after match end
                        });
                    }
                    sim.apply_ai_decision(0, &d);
                }
                if let Some(s1) = sim.colony_ai_state(1) {
                    let d = right.decide(&s1);
                    if args.dump_trajectories.is_some() {
                        trajectories.push(Trajectory {
                            match_id: m,
                            tick: sim.tick,
                            colony: 1,
                            state: s1,
                            decision: d.clone(),
                            outcome_for_this_colony: 0.5,
                        });
                    }
                    sim.apply_ai_decision(1, &d);
                }
            }
            sim.tick();
            final_status = sim.match_status();
            if !matches!(final_status, MatchStatus::InProgress) {
                break;
            }
        }

        // Patch trajectory outcomes now that we know the result.
        // Decisive winner: 1.0 / 0.0 split. Timeout / draw: graded by
        // end-state worker ratio so trajectories still carry supervision
        // signal even when no colony died (which is the common case at
        // current sim balance — matches usually timeout rather than
        // resolve). The graded signal means a colony that ended a
        // timeout with 12 vs 4 workers gets outcome = 0.75 (still
        // a "winner" signal even without a queen kill).
        let (status_str, winner) = match final_status {
            MatchStatus::Won { winner, .. } => ("won".to_string(), Some(winner)),
            MatchStatus::Draw { .. } => ("draw".to_string(), None),
            MatchStatus::InProgress => ("timeout".to_string(), None),
        };
        let workers_left = sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0);
        let workers_right = sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0);
        let total_workers = (workers_left + workers_right).max(1) as f32;
        let timeout_outcome_left = workers_left as f32 / total_workers;
        for t in &mut trajectories {
            t.outcome_for_this_colony = match winner {
                Some(w) if w == t.colony => 1.0,
                Some(_) => 0.0,
                None => {
                    // Graded by end-state worker share for this colony.
                    if t.colony == 0 { timeout_outcome_left }
                    else { 1.0 - timeout_outcome_left }
                }
            };
        }

        match winner {
            Some(0) => left_wins += 1,
            Some(1) => right_wins += 1,
            None if matches!(final_status, MatchStatus::Draw { .. }) => draws += 1,
            _ => timeouts += 1,
        }

        let summary = MatchSummary {
            match_id: m,
            seed: sim_seed,
            end_tick: sim.tick,
            status: status_str.clone(),
            winner,
            final_workers_left: sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0),
            final_workers_right: sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0),
            final_food_left: sim.colonies.get(0).map(|c| c.food_stored).unwrap_or(0.0),
            final_food_right: sim.colonies.get(1).map(|c| c.food_stored).unwrap_or(0.0),
        };
        println!(
            "  match {:>3}: tick={:>5} {} winner={:?} workers L/R={}/{}",
            m, sim.tick, status_str, winner, summary.final_workers_left, summary.final_workers_right,
        );
        summaries.push(summary);
        all_trajectories.extend(trajectories);
    }

    // ---- Aggregate ----
    let n = args.matches as f32;
    println!();
    println!("=== AGGREGATE ===");
    println!("left  ({}) wins:  {}/{}  ({:.1}%)", args.left, left_wins, args.matches, 100.0 * left_wins as f32 / n);
    println!("right ({}) wins:  {}/{}  ({:.1}%)", args.right, right_wins, args.matches, 100.0 * right_wins as f32 / n);
    println!("draws:                  {}/{}", draws, args.matches);
    println!("timeouts (no winner):   {}/{}", timeouts, args.matches);
    let avg_end_tick: f32 = summaries.iter().map(|s| s.end_tick as f32).sum::<f32>() / n;
    println!("avg end-tick:           {:.0}", avg_end_tick);

    // ---- Outputs ----
    if let Some(dir) = &args.out_dir {
        write_summary_md(dir, &args, &summaries, left_wins, right_wins, draws, timeouts)?;
        println!();
        println!("wrote SUMMARY.md to {}", dir.display());
    }
    if let Some(path) = &args.dump_trajectories {
        write_trajectories_jsonl(path, &all_trajectories)?;
        println!("wrote {} trajectory records to {}", all_trajectories.len(), path.display());
    }
    Ok(())
}

fn write_summary_md(
    dir: &std::path::Path,
    args: &CliArgs,
    summaries: &[MatchSummary],
    left_wins: u32,
    right_wins: u32,
    draws: u32,
    timeouts: u32,
) -> anyhow::Result<()> {
    use std::fmt::Write as _;
    let mut out = String::new();
    writeln!(out, "# Matchup Bench Summary").ok();
    writeln!(out).ok();
    writeln!(out, "**Left (colony 0):** `{}` (seed {})", args.left, args.left_seed).ok();
    writeln!(out, "**Right (colony 1):** `{}` (seed {})", args.right, args.right_seed).ok();
    writeln!(out, "**Matches:** {} (max-ticks each: {})", args.matches, args.max_ticks).ok();
    writeln!(out).ok();
    writeln!(out, "## Win record").ok();
    writeln!(out).ok();
    writeln!(out, "| Side | Brain | Wins | Win Rate |").ok();
    writeln!(out, "|------|-------|-----:|---------:|").ok();
    let n = args.matches as f32;
    writeln!(out, "| Left  | `{}` | {} | {:.1}% |", args.left, left_wins, 100.0 * left_wins as f32 / n).ok();
    writeln!(out, "| Right | `{}` | {} | {:.1}% |", args.right, right_wins, 100.0 * right_wins as f32 / n).ok();
    writeln!(out, "| Draw  | — | {} | {:.1}% |", draws, 100.0 * draws as f32 / n).ok();
    writeln!(out, "| Timeout | — | {} | {:.1}% |", timeouts, 100.0 * timeouts as f32 / n).ok();
    writeln!(out).ok();
    writeln!(out, "## Per-match results").ok();
    writeln!(out).ok();
    writeln!(out, "| # | seed | end_tick | status | winner | workers L/R | food L/R |").ok();
    writeln!(out, "|--:|-----:|---------:|--------|-------:|------------:|---------:|").ok();
    for s in summaries {
        writeln!(
            out,
            "| {} | {} | {} | {} | {:?} | {}/{} | {:.1}/{:.1} |",
            s.match_id, s.seed, s.end_tick, s.status, s.winner,
            s.final_workers_left, s.final_workers_right,
            s.final_food_left, s.final_food_right,
        ).ok();
    }
    std::fs::write(dir.join("SUMMARY.md"), out)?;
    Ok(())
}

fn write_trajectories_jsonl(path: &std::path::Path, records: &[Trajectory]) -> anyhow::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::File::create(path)?;
    for r in records {
        let line = serde_json::to_string(r)?;
        writeln!(f, "{}", line)?;
    }
    Ok(())
}

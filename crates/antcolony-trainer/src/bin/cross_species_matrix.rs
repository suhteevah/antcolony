//! Cross-species win-matrix / intransitivity harness. For every ordered
//! pair (A, B) in the roster, play K side-swapped matches with a fixed
//! heuristic brain on both colonies and record A's winrate vs B. Writes an
//! N×N matrix + a 3-cycle / per-row min-max intransitivity report.
//!
//! Usage: cross_species_matrix [--species-dir assets/species] [--mpe 50] [--max-ticks 8000]
//!
//! # Caste-ratio asymmetry note
//! MatchEnv::new_cross_species sets colony 0 as AI-controlled (flip from
//! new_ai_vs_ai). Both colonies are driven by HeuristicBrain which reads
//! the same ColonyAiState API — the AI/heuristic distinction only gates
//! `apply_ai_decision`; HeuristicBrain decisions are applied via the normal
//! step() path. Species biology (caste_ratio, combat stats, etc.) comes from
//! the per-colony ColonySimConfig, so any asymmetry is species-derived, not
//! caste-artifact. Side-swapping (even/odd match parity) cancels any residual
//! first-move/topology bias.

use std::path::PathBuf;

use anyhow::Result;
use antcolony_sim::{AiBrain, HeuristicBrain, MatchStatus};
use antcolony_trainer::env::MatchEnv;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mut species_dir = PathBuf::from("assets/species");
    let mut mpe = 50usize;
    let mut max_ticks = 8000u64;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = || args.next().expect("flag needs a value");
        match a.as_str() {
            "--species-dir" => species_dir = PathBuf::from(next()),
            "--mpe" => mpe = next().parse()?,
            "--max-ticks" => max_ticks = next().parse()?,
            other => tracing::warn!(arg = other, "unknown flag, ignoring"),
        }
    }

    let species = antcolony_sim::species::load_species_dir(&species_dir)?;
    let n = species.len();
    tracing::info!(n, mpe, max_ticks, "cross_species_matrix: loaded roster");

    // winrate[a][b] = A's winrate vs B over `mpe` side-swapped matches.
    // Self-matches are set to 0.5 (not played).
    let mut winrate = vec![vec![0.0f32; n]; n];

    for ai in 0..n {
        for bi in 0..n {
            if ai == bi {
                winrate[ai][bi] = 0.5;
                continue;
            }
            let mut a_wins = 0.0f32;
            for m in 0..mpe {
                // Side-swap on parity to cancel first-move/topology bias.
                let seed = ((ai as u64) << 40) ^ ((bi as u64) << 24) ^ (m as u64);
                let (sp_left, sp_right, left_is_a) = if m % 2 == 0 {
                    (&species[ai], &species[bi], true)
                } else {
                    (&species[bi], &species[ai], false)
                };
                let mut env = MatchEnv::new_cross_species(sp_left, sp_right, seed);
                env.max_ticks = max_ticks;

                let mut left_brain = HeuristicBrain::new(5.0);
                let mut right_brain = HeuristicBrain::new(5.0);

                let outcome = run_to_end(&mut env, &mut left_brain, &mut right_brain);
                let a_won = match outcome {
                    Outcome::LeftWin => left_is_a,
                    Outcome::RightWin => !left_is_a,
                    Outcome::Draw => false,
                };
                if a_won {
                    a_wins += 1.0;
                }
            }
            winrate[ai][bi] = a_wins / mpe as f32;
        }
        tracing::info!(species = %species[ai].id, ai, "row complete");
    }

    // ── Print matrix ──────────────────────────────────────────────────────
    println!("# cross-species win matrix ({}x{}, mpe={})", n, n, mpe);
    print!("{:>24}", "");
    for s in &species {
        print!("{:>10.10}", s.id);
    }
    println!();

    for ai in 0..n {
        print!("{:>24}", species[ai].id);
        for bi in 0..n {
            print!("{:>10.2}", winrate[ai][bi]);
        }
        let row_min = (0..n)
            .filter(|&b| b != ai)
            .map(|b| winrate[ai][b])
            .fold(1.0f32, f32::min);
        let row_max = (0..n)
            .filter(|&b| b != ai)
            .map(|b| winrate[ai][b])
            .fold(0.0f32, f32::max);
        println!("   [min {:.2} max {:.2}]", row_min, row_max);
    }

    // ── 3-cycle intransitivity check ─────────────────────────────────────
    // A > B > C > A (each pairwise winrate > 0.5) constitutes a 3-cycle.
    let mut cycles = 0usize;
    for a in 0..n {
        for b in 0..n {
            for c in 0..n {
                if a != b && b != c && a != c
                    && winrate[a][b] > 0.5
                    && winrate[b][c] > 0.5
                    && winrate[c][a] > 0.5
                {
                    cycles += 1;
                    tracing::info!(
                        a = %species[a].id,
                        b = %species[b].id,
                        c = %species[c].id,
                        "intransitive 3-cycle"
                    );
                }
            }
        }
    }

    // ── Per-row domination check ──────────────────────────────────────────
    // All-win (row_min > 0.5) = degenerate dominator; all-lose (row_max < 0.5) = degenerate prey.
    let all_win_rows: Vec<_> = (0..n)
        .filter(|&ai| {
            (0..n)
                .filter(|&b| b != ai)
                .all(|b| winrate[ai][b] > 0.5)
        })
        .collect();
    let all_lose_rows: Vec<_> = (0..n)
        .filter(|&ai| {
            (0..n)
                .filter(|&b| b != ai)
                .all(|b| winrate[ai][b] < 0.5)
        })
        .collect();

    println!("# intransitive 3-cycles: {}", cycles);
    if !all_win_rows.is_empty() {
        println!(
            "# WARNING: all-win rows (degenerate): {}",
            all_win_rows
                .iter()
                .map(|&i| species[i].id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !all_lose_rows.is_empty() {
        println!(
            "# WARNING: all-lose rows (degenerate): {}",
            all_lose_rows
                .iter()
                .map(|&i| species[i].id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    println!(
        "CROSS_SPECIES_MATRIX_DONE n={} cycles={}",
        n, cycles
    );
    Ok(())
}

enum Outcome {
    LeftWin,
    RightWin,
    Draw,
}

fn run_to_end(
    env: &mut MatchEnv,
    left: &mut dyn AiBrain,
    right: &mut dyn AiBrain,
) -> Outcome {
    loop {
        let sl = env.observe(0);
        let sr = env.observe(1);
        let (Some(sl), Some(sr)) = (sl, sr) else {
            break;
        };
        let al = left.decide(&sl);
        let ar = right.decide(&sr);
        let step = env.step(&al, &ar);
        if step.done || env.sim.tick >= env.max_ticks {
            break;
        }
    }
    match env.sim.match_status() {
        MatchStatus::Won { winner: 0, .. } => Outcome::LeftWin,
        MatchStatus::Won { winner: 1, .. } => Outcome::RightWin,
        _ => Outcome::Draw,
    }
}

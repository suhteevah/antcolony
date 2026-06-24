//! Cross-species win-matrix / intransitivity harness. For every ordered
//! pair (A, B) in the roster, play K side-swapped matches with a fixed
//! heuristic brain on both colonies and record A's winrate vs B. Writes an
//! N×N matrix + a 3-cycle / per-row min-max intransitivity report.
//!
//! Usage: cross_species_matrix [--species-dir assets/species] [--mpe 50] [--max-ticks 8000]
//!
//! # Parallelism
//! All N×N ordered pairs are built into a flat Vec and processed in parallel
//! via rayon. Each closure owns its own MatchEnv and HeuristicBrain instances
//! (no shared mutable state across threads). Results are collected by index into
//! a flat Vec<(usize, usize, f32)> and reassembled into the winrate matrix after
//! the parallel join.
//!
//! # Determinism
//! Per-match seeds are derived solely from `(ai, bi, m)` — identical to the
//! sequential version — so results are bit-identical regardless of thread count.
//! Verified via RAYON_NUM_THREADS=1 vs default.
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
use rayon::prelude::*;

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
    let mut nest = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = || args.next().expect("flag needs a value");
        match a.as_str() {
            "--species-dir" => species_dir = PathBuf::from(next()),
            "--mpe" => mpe = next().parse()?,
            "--max-ticks" => max_ticks = next().parse()?,
            "--nest" => nest = true,
            other => tracing::warn!(arg = other, "unknown flag, ignoring"),
        }
    }

    let species = antcolony_sim::species::load_species_dir(&species_dir)?;
    let n = species.len();
    tracing::info!(n, mpe, max_ticks, "cross_species_matrix: loaded roster");

    // Build flat list of all ordered pairs (ai, bi), including self-pairs.
    // Self-pairs are resolved to 0.5 without simulation inside the closure.
    let pairs: Vec<(usize, usize)> = (0..n)
        .flat_map(|ai| (0..n).map(move |bi| (ai, bi)))
        .collect();

    // Run all pairs in parallel. Each element returns (ai, bi, winrate).
    // Determinism: seed is a pure function of (ai, bi, m) — no shared RNG.
    // Each closure owns its own MatchEnv + HeuristicBrain (no &mut crossing threads).
    let flat_results: Vec<(usize, usize, f32)> = pairs
        .into_par_iter()
        .map(|(ai, bi)| {
            if ai == bi {
                return (ai, bi, 0.5f32);
            }
            let mut a_wins = 0.0f32;
            for m in 0..mpe {
                // Side-swap on parity to cancel first-move/topology bias.
                // Seed is deterministic: identical to sequential version.
                let seed = ((ai as u64) << 40) ^ ((bi as u64) << 24) ^ (m as u64);
                let (sp_left, sp_right, left_is_a) = if m % 2 == 0 {
                    (&species[ai], &species[bi], true)
                } else {
                    (&species[bi], &species[ai], false)
                };
                // ── Arena selection ──────────────────────────────────────────
                // Default: three-module chokepoint arena (terrain_attacker_cap
                // fires on NestEntrance=1 / tunnel=3 cells).
                // With --nest: five-module underground nest arena (raid descent
                // + UG lazy-worker reserve mechanics engaged).
                let mut env = if nest {
                    MatchEnv::new_cross_species_nest_arena(sp_left, sp_right, seed)
                } else {
                    MatchEnv::new_cross_species_arena(sp_left, sp_right, seed)
                };
                env.max_ticks = max_ticks;
                tracing::debug!(nest, "arena selected for match");

                // Inject chokepoint attacker-cap + predation identically on
                // every match so results remain thread-count-independent.
                for i in 0..env.sim.colony_configs.len() {
                    // Terrain-gated attacker cap (Champer & Schlenoff square→linear law).
                    // Surface stays uncapped (255); tunnel capped at 3; entrance at 1.
                    env.sim.colony_configs[i].combat.max_simultaneous_attackers_open = 255;
                    env.sim.colony_configs[i].combat.max_simultaneous_attackers_tunnel = 3;
                    env.sim.colony_configs[i].combat.max_simultaneous_attackers_entrance = 1;
                    // Predation corpse-loot: only fires when predates_ants == true.
                    if env.sim.colony_configs[i].predates_ants {
                        env.sim.colony_configs[i].combat.usurp_corpse_to_killer_frac = 0.5;
                    }
                }

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
            let wr = a_wins / mpe as f32;
            tracing::debug!(
                ai,
                bi,
                species_a = %species[ai].id,
                species_b = %species[bi].id,
                winrate = wr,
                "pair complete"
            );
            (ai, bi, wr)
        })
        .collect();

    // Reassemble into winrate matrix. The collect() above joins all threads;
    // results arrive in any order but are indexed by (ai, bi).
    let mut winrate = vec![vec![0.0f32; n]; n];
    for (ai, bi, wr) in flat_results {
        winrate[ai][bi] = wr;
    }

    // Log one line per row for progress visibility (mirrors sequential "row complete").
    for ai in 0..n {
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

    println!("# arena: {}", if nest { "underground-nest (5-module)" } else { "flat chokepoint (3-module)" });
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use antcolony_sim::{AiBrain, HeuristicBrain, MatchStatus};
    use antcolony_trainer::env::MatchEnv;
    use rayon::prelude::*;

    enum Outcome {
        LeftWin,
        RightWin,
        Draw,
    }

    fn run_to_end_t(
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

    fn compute_matrix_sequential(
        species: &[antcolony_sim::species::Species],
        mpe: usize,
        max_ticks: u64,
    ) -> Vec<Vec<f32>> {
        let n = species.len();
        let mut winrate = vec![vec![0.0f32; n]; n];
        for ai in 0..n {
            for bi in 0..n {
                if ai == bi {
                    winrate[ai][bi] = 0.5;
                    continue;
                }
                let mut a_wins = 0.0f32;
                for m in 0..mpe {
                    let seed = ((ai as u64) << 40) ^ ((bi as u64) << 24) ^ (m as u64);
                    let (sp_left, sp_right, left_is_a) = if m % 2 == 0 {
                        (&species[ai], &species[bi], true)
                    } else {
                        (&species[bi], &species[ai], false)
                    };
                    // Use arena variant to mirror the production harness.
                    let mut env = MatchEnv::new_cross_species_arena(sp_left, sp_right, seed);
                    env.max_ticks = max_ticks;
                    let mut lb = HeuristicBrain::new(5.0);
                    let mut rb = HeuristicBrain::new(5.0);
                    let outcome = run_to_end_t(&mut env, &mut lb, &mut rb);
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
        }
        winrate
    }

    fn compute_matrix_parallel(
        species: &[antcolony_sim::species::Species],
        mpe: usize,
        max_ticks: u64,
    ) -> Vec<Vec<f32>> {
        let n = species.len();
        let pairs: Vec<(usize, usize)> = (0..n)
            .flat_map(|ai| (0..n).map(move |bi| (ai, bi)))
            .collect();

        let flat_results: Vec<(usize, usize, f32)> = pairs
            .into_par_iter()
            .map(|(ai, bi)| {
                if ai == bi {
                    return (ai, bi, 0.5f32);
                }
                let mut a_wins = 0.0f32;
                for m in 0..mpe {
                    let seed = ((ai as u64) << 40) ^ ((bi as u64) << 24) ^ (m as u64);
                    let (sp_left, sp_right, left_is_a) = if m % 2 == 0 {
                        (&species[ai], &species[bi], true)
                    } else {
                        (&species[bi], &species[ai], false)
                    };
                    // Use arena variant to mirror the production harness.
                    let mut env = MatchEnv::new_cross_species_arena(sp_left, sp_right, seed);
                    env.max_ticks = max_ticks;
                    let mut lb = HeuristicBrain::new(5.0);
                    let mut rb = HeuristicBrain::new(5.0);
                    let outcome = run_to_end_t(&mut env, &mut lb, &mut rb);
                    let a_won = match outcome {
                        Outcome::LeftWin => left_is_a,
                        Outcome::RightWin => !left_is_a,
                        Outcome::Draw => false,
                    };
                    if a_won {
                        a_wins += 1.0;
                    }
                }
                (ai, bi, a_wins / mpe as f32)
            })
            .collect();

        let mut winrate = vec![vec![0.0f32; n]; n];
        for (ai, bi, wr) in flat_results {
            winrate[ai][bi] = wr;
        }
        winrate
    }

    /// Confirms parallel == sequential for small roster / small mpe.
    /// Determinism comes from per-(ai,bi,m) seeds, not thread scheduling,
    /// so results are bit-identical at any RAYON_NUM_THREADS value.
    #[test]
    fn parallel_equals_sequential() {
        let species_dir = PathBuf::from(
            std::env::var("ANTCOLONY_SPECIES_DIR")
                .unwrap_or_else(|_| "../../assets/species".to_owned()),
        );
        let species = match antcolony_sim::species::load_species_dir(&species_dir) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "parallel_equals_sequential: skipping — could not load species dir \
                     {:?}: {}",
                    species_dir, e
                );
                return;
            }
        };

        let mpe = 4;
        let max_ticks = 400;

        let seq = compute_matrix_sequential(&species, mpe, max_ticks);
        let par = compute_matrix_parallel(&species, mpe, max_ticks);

        assert_eq!(
            seq.len(),
            par.len(),
            "matrix row count mismatch: seq={} par={}",
            seq.len(),
            par.len()
        );
        for (ai, (row_s, row_p)) in seq.iter().zip(par.iter()).enumerate() {
            for (bi, (vs, vp)) in row_s.iter().zip(row_p.iter()).enumerate() {
                assert_eq!(
                    vs.to_bits(),
                    vp.to_bits(),
                    "bit mismatch at [{ai}][{bi}]: seq={vs} par={vp}"
                );
            }
        }
    }
}

//! eval_mlp_vs_heuristic — score a trained `MlpBrain` (the `ppo-train`
//! `current.json` / snapshot weights format) against the fixed `HeuristicBrain`
//! across the cross-species arena. This is the missing measurement the
//! cross-species curriculum run was training toward: `cross_species_matrix`
//! only runs heuristic-vs-heuristic (it measures the SPECIES win-matrix to find
//! intransitive cycles), so it cannot tell us whether a TRAINED brain learned
//! the intransitive meta. This bin does.
//!
//! For every ordered pair (mlp_species A, opp_species B) in the roster we play
//! `mpe` side-swapped matches with the MLP driving colony-A and the heuristic
//! driving colony-B, and record the MLP's winrate. Three headline numbers fall
//! out:
//!   * OVERALL mean — MLP winrate vs heuristic across the whole cross-species
//!     meta (>0.5 ⇒ the trained brain beats the heuristic on average).
//!   * DIAGONAL mean (A == B) — same-species MLP-vs-heuristic, so species
//!     advantage cancels and only BRAIN SKILL remains. This is the cleanest
//!     "did training make it better than the heuristic" number; heuristic-vs-
//!     heuristic would score 0.5 here by construction.
//!   * per-MLP-species row means — which species the brain pilots well.
//!
//! Arena selection (`--nest`), the cyclic clade type-chart (`--venom-cycle`),
//! and the chokepoint attacker-cap + predation knobs are applied IDENTICALLY to
//! `cross_species_matrix`, so to evaluate a checkpoint in the same arena it
//! trained in, pass the matching flags (the 2026-06-23 venom3 run trained with
//! `--nest --venom-cycle 3.0`).
//!
//! CPU-only; no GPU needed. Parallel over the N×N grid via rayon. Per-match
//! seeds are a pure function of `(ai, bi, m)` and each match loads a fresh,
//! deterministic MLP (explore_std defaults to 0.0), so results are bit-identical
//! regardless of thread count (asserted by the `parallel_equals_sequential`
//! test).
//!
//! Usage:
//!   eval_mlp_vs_heuristic --weights bench/xspecies-nest-venom3/current.json \
//!       --nest --venom-cycle 3.0 --mpe 20 --max-ticks 4000

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use antcolony_sim::species::Species;
use antcolony_sim::{AiBrain, HeuristicBrain, MatchStatus, MlpBrain};
use antcolony_trainer::env::MatchEnv;
use rayon::prelude::*;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let mut weights = PathBuf::from("bench/xspecies-nest-venom3/current.json");
    let mut species_dir = PathBuf::from("assets/species");
    let mut mpe = 20usize;
    let mut max_ticks = 4000u64;
    let mut nest = false;
    let mut venom_cycle = 0.0f32;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = || args.next().expect("flag needs a value");
        match a.as_str() {
            "--weights" => weights = PathBuf::from(next()),
            "--species-dir" => species_dir = PathBuf::from(next()),
            "--mpe" => mpe = next().parse()?,
            "--max-ticks" => max_ticks = next().parse()?,
            "--nest" => nest = true,
            "--venom-cycle" => venom_cycle = next().parse()?,
            other => tracing::warn!(arg = other, "unknown flag, ignoring"),
        }
    }

    let species = antcolony_sim::species::load_species_dir(&species_dir)?;
    let n = species.len();
    tracing::info!(
        n, mpe, max_ticks, nest, venom_cycle,
        weights = %weights.display(),
        "eval_mlp_vs_heuristic: loaded roster"
    );

    let winrate = eval_grid(&species, &weights, mpe, max_ticks, nest, venom_cycle, true)?;

    // ── Print matrix (rows = MLP species, cols = heuristic species) ──────────
    println!(
        "# MLP-vs-Heuristic winrate ({}x{}, mpe={}) — rows: MLP plays, cols: heuristic plays",
        n, n, mpe
    );
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
        let row_mean: f32 = winrate[ai].iter().sum::<f32>() / n as f32;
        println!("   [mlp_row_mean {:.3}]", row_mean);
    }

    // ── Headline aggregates ──────────────────────────────────────────────────
    let mut sum = 0.0f32;
    let mut count = 0usize;
    let mut diag_sum = 0.0f32;
    let mut wins = 0usize; // cells where MLP > 0.5
    for ai in 0..n {
        for bi in 0..n {
            sum += winrate[ai][bi];
            count += 1;
            if winrate[ai][bi] > 0.5 {
                wins += 1;
            }
        }
        diag_sum += winrate[ai][ai];
    }
    let overall = sum / count as f32;
    let diag = diag_sum / n as f32;

    println!("# arena: {}", if nest { "underground-nest (5-module)" } else { "flat chokepoint (3-module)" });
    println!(
        "# venom_cycle_strength: {venom_cycle} ({})",
        if venom_cycle > 0.0 { "cyclic clade type-chart" } else { "legacy venom matrix" }
    );
    println!("# OVERALL MLP winrate vs heuristic: {:.4}  (>0.5 ⇒ brain beats heuristic across the meta)", overall);
    println!("# DIAGONAL (same-species, pure brain skill): {:.4}  (heuristic-vs-heuristic = 0.5 here)", diag);
    println!("# cells MLP wins (>0.5): {wins}/{count}");
    println!("EVAL_MLP_DONE overall={:.4} diag={:.4} wins={}/{} n={}", overall, diag, wins, count, n);

    tracing::info!(overall, diag, wins, cells = count, "eval_mlp_vs_heuristic done");
    Ok(())
}

enum Outcome {
    LeftWin,
    RightWin,
    Draw,
}

/// Drive a match to completion: `left` controls colony 0, `right` controls
/// colony 1. Identical loop to `cross_species_matrix::run_to_end`.
fn run_to_end(env: &mut MatchEnv, left: &mut dyn AiBrain, right: &mut dyn AiBrain) -> Outcome {
    loop {
        let (Some(sl), Some(sr)) = (env.observe(0), env.observe(1)) else {
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

/// Apply the chokepoint attacker-cap + predation + cyclic-clade knobs to a
/// freshly-built cross-species arena, IDENTICALLY to `cross_species_matrix`, so
/// the trained brain is scored in the exact arena it trained in.
fn apply_arena_knobs(env: &mut MatchEnv, venom_cycle: f32) {
    // Cyclic clade type-chart is read from the GLOBAL combat config in
    // combat_tick, so set it on env.sim.config.
    env.sim.config.combat.venom_cycle_strength = venom_cycle;
    for i in 0..env.sim.colony_configs.len() {
        // Terrain-gated attacker cap (Champer & Schlenoff square→linear law).
        env.sim.colony_configs[i].combat.max_simultaneous_attackers_open = 255;
        env.sim.colony_configs[i].combat.max_simultaneous_attackers_tunnel = 3;
        env.sim.colony_configs[i].combat.max_simultaneous_attackers_entrance = 1;
        // Predation corpse-loot: only fires when predates_ants == true.
        if env.sim.colony_configs[i].predates_ants {
            env.sim.colony_configs[i].combat.usurp_corpse_to_killer_frac = 0.5;
        }
    }
}

/// MLP winrate for one grid cell: the MLP pilots `species[ai]`, the heuristic
/// pilots `species[bi]`, over `mpe` side-swapped matches. Seeds match
/// `cross_species_matrix` for cross-comparability.
#[allow(clippy::too_many_arguments)]
fn cell_winrate(
    species: &[Species],
    weights: &Path,
    ai: usize,
    bi: usize,
    mpe: usize,
    max_ticks: u64,
    nest: bool,
    venom_cycle: f32,
) -> f32 {
    let mut mlp_wins = 0.0f32;
    for m in 0..mpe {
        let seed = ((ai as u64) << 40) ^ ((bi as u64) << 24) ^ (m as u64);
        // Side-swap on parity so the MLP plays both physical colonies equally,
        // cancelling first-move/topology bias. The MLP's species (ai) and the
        // heuristic's species (bi) stay attached to their respective players.
        let mlp_on_left = m % 2 == 0;
        let (sp_left, sp_right) = if mlp_on_left {
            (&species[ai], &species[bi])
        } else {
            (&species[bi], &species[ai])
        };

        let mut env = if nest {
            MatchEnv::new_cross_species_nest_arena(sp_left, sp_right, seed)
        } else {
            MatchEnv::new_cross_species_arena(sp_left, sp_right, seed)
        };
        env.max_ticks = max_ticks;
        apply_arena_knobs(&mut env, venom_cycle);

        // Fresh, deterministic brains per match (explore_std defaults to 0.0).
        // Weights were pre-validated by the caller, so a reload failure here is
        // not a realistic runtime condition (file deleted mid-eval).
        let mut mlp = MlpBrain::load(weights, format!("mlp-{ai}-{bi}-{m}"))
            .expect("pre-validated MLP weights failed to reload mid-eval");
        let mut heur = HeuristicBrain::new(5.0);

        let outcome = if mlp_on_left {
            run_to_end(&mut env, &mut mlp, &mut heur)
        } else {
            run_to_end(&mut env, &mut heur, &mut mlp)
        };
        let mlp_won = match outcome {
            Outcome::LeftWin => mlp_on_left,
            Outcome::RightWin => !mlp_on_left,
            Outcome::Draw => false,
        };
        if mlp_won {
            mlp_wins += 1.0;
        }
    }
    mlp_wins / mpe as f32
}

/// Compute the full N×N MLP-vs-heuristic winrate grid. `parallel` selects rayon
/// vs sequential; both produce bit-identical results (seeds are pure functions
/// of `(ai, bi, m)`).
fn eval_grid(
    species: &[Species],
    weights: &Path,
    mpe: usize,
    max_ticks: u64,
    nest: bool,
    venom_cycle: f32,
    parallel: bool,
) -> Result<Vec<Vec<f32>>> {
    let n = species.len();
    // Fail fast with a clear error if the weights don't load — turns a
    // mid-run panic in the hot loop into an up-front, contextual failure.
    MlpBrain::load(weights, "validate")
        .with_context(|| format!("failed to load MLP weights `{}`", weights.display()))?;

    let pairs: Vec<(usize, usize)> = (0..n).flat_map(|ai| (0..n).map(move |bi| (ai, bi))).collect();

    let compute = |&(ai, bi): &(usize, usize)| -> (usize, usize, f32) {
        let wr = cell_winrate(species, weights, ai, bi, mpe, max_ticks, nest, venom_cycle);
        (ai, bi, wr)
    };

    let flat: Vec<(usize, usize, f32)> = if parallel {
        pairs.par_iter().map(compute).collect()
    } else {
        pairs.iter().map(compute).collect()
    };

    let mut winrate = vec![vec![0.0f32; n]; n];
    for (ai, bi, wr) in flat {
        winrate[ai][bi] = wr;
    }
    Ok(winrate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Write a valid 17→64→64→6 MLP weights JSON (all-zero weights/biases,
    /// unit input normalization) to `path`. A zero MLP emits a constant
    /// decision — deterministic, which is exactly what the determinism test
    /// wants, and a valid load target for `MlpBrain::load`.
    fn write_zero_mlp(path: &Path) {
        let zeros_row = |cols: usize| vec![0.0f32; cols];
        let w1: Vec<Vec<f32>> = (0..64).map(|_| zeros_row(17)).collect();
        let w2: Vec<Vec<f32>> = (0..64).map(|_| zeros_row(64)).collect();
        let w3: Vec<Vec<f32>> = (0..6).map(|_| zeros_row(64)).collect();
        let json = serde_json::json!({
            "input_dim": 17,
            "hidden_dim": 64,
            "output_dim": 6,
            "input_mean": vec![0.0f32; 17],
            "input_std": vec![1.0f32; 17],
            "w1": w1, "b1": vec![0.0f32; 64],
            "w2": w2, "b2": vec![0.0f32; 64],
            "w3": w3, "b3": vec![0.0f32; 6],
        });
        let mut f = std::fs::File::create(path).expect("create temp weights");
        f.write_all(serde_json::to_string(&json).unwrap().as_bytes())
            .expect("write temp weights");
    }

    fn species_dir() -> PathBuf {
        PathBuf::from(
            std::env::var("ANTCOLONY_SPECIES_DIR")
                .unwrap_or_else(|_| "../../assets/species".to_owned()),
        )
    }

    /// Bit-identical parallel vs sequential — determinism comes from per-(ai,bi,m)
    /// seeds and fresh deterministic MLPs, not thread scheduling.
    #[test]
    fn parallel_equals_sequential() {
        let species = match antcolony_sim::species::load_species_dir(&species_dir()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("skipping — could not load species dir: {e}");
                return;
            }
        };
        let tmp = std::env::temp_dir().join("eval_mlp_vs_heuristic_zero.json");
        write_zero_mlp(&tmp);

        let mpe = 2;
        let max_ticks = 300;
        let seq = eval_grid(&species, &tmp, mpe, max_ticks, false, 0.0, false).unwrap();
        let par = eval_grid(&species, &tmp, mpe, max_ticks, false, 0.0, true).unwrap();

        assert_eq!(seq.len(), par.len());
        for (ai, (rs, rp)) in seq.iter().zip(par.iter()).enumerate() {
            for (bi, (vs, vp)) in rs.iter().zip(rp.iter()).enumerate() {
                assert_eq!(
                    vs.to_bits(),
                    vp.to_bits(),
                    "bit mismatch at [{ai}][{bi}]: seq={vs} par={vp}"
                );
                assert!((0.0..=1.0).contains(vs), "winrate out of range at [{ai}][{bi}]: {vs}");
            }
        }
    }

    /// A missing weights file fails fast with context, not a hot-loop panic.
    #[test]
    fn missing_weights_errors_cleanly() {
        let species = match antcolony_sim::species::load_species_dir(&species_dir()) {
            Ok(s) => s,
            Err(_) => return,
        };
        let bogus = std::env::temp_dir().join("eval_mlp_definitely_missing_xyz.json");
        let _ = std::fs::remove_file(&bogus);
        let r = eval_grid(&species, &bogus, 1, 100, false, 0.0, false);
        assert!(r.is_err(), "expected an error for a missing weights file");
    }
}

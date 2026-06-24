//! eval_hac_vs_heuristic — score a trained HAC checkpoint (`hac_*.safetensors`)
//! against `HeuristicBrain` across the CROSS-SPECIES (nest) arena. The HAC analog
//! of `eval_mlp_vs_heuristic`: the phase3 eval only scores on the SAME-species
//! 7-archetype bench (a proxy); this answers the actual question — does the HAC
//! learn the intransitive cross-species meta (beat the heuristic across species)?
//!
//! For every ordered (hac_species, heur_species) pair we play `mpe` side-swapped
//! matches with the HAC driving one colony and the heuristic the other, and
//! record the HAC's DECISIVE winrate. Headlines:
//!   * OVERALL — HAC winrate vs heuristic across the whole meta (>0.5 ⇒ beats it).
//!   * DIAGONAL (same species both sides) — pure brain skill (heuristic-vs-
//!     heuristic = 0.5 there).
//!
//! Arena flags (`--nest`, `--venom-cycle`) must match the training arena
//! (the 2026-06-24 HAC run trained with `--cross-species-nest --venom-cycle 3.0`).
//! CPU-only; parallel over the grid.
//!
//! Usage:
//!   eval_hac_vs_heuristic --weights bench/hac-xspecies-venom3/hac_best.safetensors \
//!       --nest --venom-cycle 3.0 --mpe 20 --sizing a1

use std::path::PathBuf;

use anyhow::Result;
use antcolony_trainer::evaluate_hac_cross_species;
use antcolony_trainer::hierarchical::sizing::{A1, A2};
use antcolony_trainer::self_play::load_frozen_hac;
use candle_core::Device;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    let mut weights = PathBuf::from("bench/hac-xspecies-venom3/hac_best.safetensors");
    let mut species_dir = PathBuf::from("assets/species");
    let mut mpe = 20usize;
    let mut max_ticks = 4000u64;
    let mut nest = false;
    let mut venom_cycle = 0.0f32;
    let mut sizing_name = "a1".to_string();
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
            "--sizing" => sizing_name = next(),
            other => tracing::warn!(arg = other, "unknown flag, ignoring"),
        }
    }
    let sizing = match sizing_name.as_str() {
        "a2" => A2,
        _ => A1,
    };

    let device = Device::Cpu;
    let species = antcolony_sim::species::load_species_dir(&species_dir)?;
    let n = species.len();
    let hac = load_frozen_hac(&weights, sizing, &device)?;
    tracing::info!(n, mpe, max_ticks, nest, venom_cycle, sizing = sizing_name, weights = %weights.display(), "eval_hac_vs_heuristic: loaded HAC + roster");

    let winrate = evaluate_hac_cross_species(&hac, &device, &species, mpe, max_ticks, nest, venom_cycle)?;

    println!(
        "# HAC-vs-Heuristic winrate ({}x{}, mpe={}) — rows: HAC plays, cols: heuristic plays",
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
        println!("   [hac_row_mean {:.3}]", row_mean);
    }

    let mut sum = 0.0f32;
    let mut diag_sum = 0.0f32;
    let mut wins = 0usize;
    let count = n * n;
    for ai in 0..n {
        for bi in 0..n {
            sum += winrate[ai][bi];
            if winrate[ai][bi] > 0.5 {
                wins += 1;
            }
        }
        diag_sum += winrate[ai][ai];
    }
    let overall = sum / count as f32;
    let diag = diag_sum / n as f32;

    println!("# arena: {}", if nest { "underground-nest (5-module)" } else { "flat chokepoint (3-module)" });
    println!("# venom_cycle_strength: {venom_cycle}");
    println!("# OVERALL HAC winrate vs heuristic: {:.4}  (>0.5 ⇒ HAC beats heuristic across the meta)", overall);
    println!("# DIAGONAL (same-species, pure brain skill): {:.4}  (heuristic-vs-heuristic = 0.5 here)", diag);
    println!("# cells HAC wins (>0.5): {wins}/{count}");
    println!("EVAL_HAC_DONE overall={:.4} diag={:.4} wins={}/{} n={}", overall, diag, wins, count, n);
    tracing::info!(overall, diag, wins, cells = count, "eval_hac_vs_heuristic done");
    Ok(())
}

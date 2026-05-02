//! Species bench CLI.
//!
//! Runs one or all shipped species through the bench harness and writes
//! per-species CSV + Markdown to a target directory.
//!
//! # Usage
//!
//! ```text
//! cargo run --release --example species_bench -- --all --years 5 --out bench/2026-05-02/
//! cargo run --release --example species_bench -- --species lasius_niger --years 1 --out bench/lasius/
//! cargo run --release --example species_bench -- --all --scale seasonal --years 5 --out bench/run/
//! ```
//!
//! # Output
//!
//! For each species the runner writes:
//! - `<out>/<species_id>.csv` — full per-tick telemetry (audit data)
//! - `<out>/<species_id>.md` — human-readable scored report (ecologist review)
//!
//! Plus a top-level `<out>/SUMMARY.md` with all-species composite scores.
//!
//! # Audit notes
//!
//! - Default time scale is Seasonal (the only calibrated scale).
//! - Default seed is 42 — runs are deterministic.
//! - Each species is run independently; no cross-species interaction.

use std::path::PathBuf;

use antcolony_sim::{
    Species, TimeScale, bench::report, bench::run::{self, BenchResult}, load_species_dir,
};

fn parse_scale(s: &str) -> Option<TimeScale> {
    match s.to_ascii_lowercase().as_str() {
        "realtime" | "real" | "1x" => Some(TimeScale::Realtime),
        "brisk" | "10x" => Some(TimeScale::Brisk),
        "seasonal" | "60x" => Some(TimeScale::Seasonal),
        "timelapse" | "1440x" => Some(TimeScale::Timelapse),
        _ => None,
    }
}

struct CliArgs {
    species_id: Option<String>,
    all: bool,
    years: f32,
    scale: TimeScale,
    seed: u64,
    out_dir: PathBuf,
    sample_every_days: u32,
}

fn parse_args() -> anyhow::Result<CliArgs> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut species_id: Option<String> = None;
    let mut all = false;
    let mut years: f32 = 5.0;
    let mut scale: TimeScale = TimeScale::Seasonal;
    let mut seed: u64 = 42;
    let mut out_dir = PathBuf::from("bench/run");
    let mut sample_every_days: u32 = 1;
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--species" => {
                species_id = raw.get(i + 1).cloned();
                i += 2;
            }
            "--all" => {
                all = true;
                i += 1;
            }
            "--years" => {
                years = raw
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5.0);
                i += 2;
            }
            "--scale" => {
                let s = raw.get(i + 1).cloned().unwrap_or_default();
                scale = parse_scale(&s).ok_or_else(|| {
                    anyhow::anyhow!("unknown --scale `{s}` (expected realtime|brisk|seasonal|timelapse)")
                })?;
                i += 2;
            }
            "--seed" => {
                seed = raw.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(42);
                i += 2;
            }
            "--out" => {
                out_dir = PathBuf::from(raw.get(i + 1).cloned().unwrap_or_default());
                i += 2;
            }
            "--sample-every-days" => {
                sample_every_days = raw
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                i += 2;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                anyhow::bail!("unknown arg `{other}` — try --help");
            }
        }
    }
    if species_id.is_none() && !all {
        anyhow::bail!("specify --species <id> or --all");
    }
    Ok(CliArgs {
        species_id,
        all,
        years,
        scale,
        seed,
        out_dir,
        sample_every_days,
    })
}

fn print_help() {
    println!(
        "species_bench — run species through the audit harness\n\n\
         USAGE:\n  \
           cargo run --release --example species_bench -- [FLAGS]\n\n\
         FLAGS:\n  \
           --species <id>           Run one species by id\n  \
           --all                    Run all loaded species\n  \
           --years <n>              In-game years to simulate (default 5)\n  \
           --scale <name>           realtime|brisk|seasonal|timelapse (default seasonal)\n  \
           --seed <n>               Random seed (default 42)\n  \
           --out <dir>              Output directory (default bench/run)\n  \
           --sample-every-days <n>  Sampling cadence (default 1)\n  \
           -h, --help               Show this help\n"
    );
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,species_bench=info")
        .with_target(false)
        .init();

    let args = parse_args()?;
    std::fs::create_dir_all(&args.out_dir)?;

    let species_list = load_species_dir("assets/species")?;
    let to_run: Vec<Species> = if args.all {
        species_list
    } else {
        let id = args.species_id.as_deref().unwrap();
        species_list
            .into_iter()
            .filter(|s| s.id == id)
            .collect::<Vec<_>>()
    };
    if to_run.is_empty() {
        anyhow::bail!("no species matched the selection");
    }

    println!(
        "species_bench: {} species, {:.1} years at {} scale, seed={}, out={}",
        to_run.len(),
        args.years,
        args.scale.label(),
        args.seed,
        args.out_dir.display(),
    );

    let mut summary_rows: Vec<(String, Option<f64>, Vec<String>)> = Vec::new();
    for species in to_run {
        let id = species.id.clone();
        println!("  running {id}...");
        let cfg = run::BenchRunConfig {
            species,
            years: args.years,
            time_scale: args.scale,
            seed: args.seed,
            sample_every_days: args.sample_every_days,
        };
        let result = run::run_one(cfg);
        write_outputs(&args.out_dir, &result)?;
        let composite = result.score.composite_0_to_100();
        summary_rows.push((id, composite, result.caveats.clone()));
    }

    write_summary(&args.out_dir, &summary_rows)?;
    println!("species_bench: done. Reports in {}", args.out_dir.display());
    Ok(())
}

fn write_outputs(dir: &std::path::Path, result: &BenchResult) -> anyhow::Result<()> {
    let csv = report::render_csv(result);
    let md = report::render_markdown(result);
    std::fs::write(dir.join(format!("{}.csv", result.species_id)), csv)?;
    std::fs::write(dir.join(format!("{}.md", result.species_id)), md)?;
    Ok(())
}

fn write_summary(
    dir: &std::path::Path,
    rows: &[(String, Option<f64>, Vec<String>)],
) -> anyhow::Result<()> {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(2048);
    writeln!(out, "# Species Bench Run Summary").ok();
    writeln!(out).ok();
    writeln!(
        out,
        "Generated by `antcolony-sim::bench`. One row per species. Open the \
         per-species `.md` next to this file for full breakdown."
    )
    .ok();
    writeln!(out).ok();
    writeln!(out, "| Species | Composite Score | Status |").ok();
    writeln!(out, "|---------|----------------:|--------|").ok();
    for (id, score, _caveats) in rows {
        let (score_s, status) = match score {
            Some(s) if *s >= 80.0 => (format!("{s:.1}"), "HEALTHY"),
            Some(s) if *s >= 60.0 => (format!("{s:.1}"), "MARGINAL"),
            Some(s) if *s >= 30.0 => (format!("{s:.1}"), "CONCERNING"),
            Some(s) => (format!("{s:.1}"), "FAILED"),
            None => ("n/a".into(), "n/a"),
        };
        writeln!(out, "| `{id}` | {score_s} | {status} |").ok();
    }
    writeln!(out).ok();
    let any_caveats = rows.iter().any(|(_, _, c)| !c.is_empty());
    if any_caveats {
        writeln!(out, "## Caveats").ok();
        writeln!(out).ok();
        for (id, _, caveats) in rows {
            for c in caveats {
                writeln!(out, "- `{id}`: {c}").ok();
            }
        }
        writeln!(out).ok();
    }
    std::fs::write(dir.join("SUMMARY.md"), out)?;
    Ok(())
}

//! PvP tournament CLI — round-robin over any mix of HAC + scripted brains,
//! producing a Bradley-Terry/Elo ladder, win/ws matrices, and a ratings JSON.
//!
//! Build (CPU, no cuda feature):
//!   cargo build --release -p antcolony-trainer --bin tournament
//!
//! Example invocation (cnc, see scripts/run_tournament_cnc.sh):
//!   ./target/release/tournament \
//!     --contenders sota=hac:bench/phase3-a1-combat/hac_best.safetensors,v1=mlp:bench/iterative-fsp/round_1/mlp_weights_v1.json \
//!     --add-archetypes --mpe 15 --anchor v1 --out bench/tournament
//!
//! Flags (all optional):
//!   --contenders <comma-list of id=spec>   each "id=spec" split on FIRST '='
//!   --add-archetypes                       append 7 BENCH_ARCHETYPES if absent
//!   --mpe <N>                              matches per ordered pair (default 15)
//!   --max-ticks <N>                        tick limit per match (default 10000)
//!   --anchor <id>                          Elo anchor contender id (default "v1")
//!   --anchor-elo <f>                       Elo value for anchor (default 1000.0)
//!   --cycle-margin <f>                     decisive margin for 3-cycle (default 0.55)
//!   --out <dir>                            output directory (default bench/tournament)

use antcolony_trainer::eval::BENCH_ARCHETYPES;
use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::tournament::{run_tournament, TournamentConfig};
use std::io::Write as IoWrite;
use std::path::PathBuf;

/// Parse a CLI flag value or exit(2) loudly. Prevents a typo'd value from
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
        .with_env_filter(
            "antcolony_sim=warn,antcolony_trainer=info,tournament=info",
        )
        .with_target(false)
        .init();

    // --- defaults ---
    let mut contenders: Vec<(String, String)> = Vec::new();
    let mut add_archetypes = false;
    let mut mpe = 15usize;
    let mut max_ticks = 10_000u64;
    let mut anchor_id = "v1".to_string();
    let mut anchor_elo = 1000.0f64;
    let mut cycle_margin = 0.55f32;
    let mut out_dir = PathBuf::from("bench/tournament");

    // --- hand-rolled parser (mirrors phase3_train.rs parse_or_exit pattern) ---
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let next = || args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--contenders" => {
                // Split comma-delimited entries; each entry split on FIRST '=' only
                // so that specs like "hac:path/to/file" (with ':' but no '=') work,
                // and HAC paths that might theoretically contain '=' also work.
                let raw = next();
                for entry in raw.split(',') {
                    let entry = entry.trim();
                    if entry.is_empty() { continue; }
                    match entry.splitn(2, '=').collect::<Vec<_>>().as_slice() {
                        [id, spec] => {
                            contenders.push((id.to_string(), spec.to_string()));
                        }
                        _ => {
                            tracing::error!(entry, "--contenders entry missing '='; expected id=spec");
                            std::process::exit(2);
                        }
                    }
                }
                i += 2;
            }
            "--add-archetypes" => {
                add_archetypes = true;
                i += 1;
            }
            "--mpe" => {
                mpe = parse_or_exit("--mpe", &next());
                i += 2;
            }
            "--max-ticks" => {
                max_ticks = parse_or_exit("--max-ticks", &next());
                i += 2;
            }
            "--anchor" => {
                anchor_id = next();
                i += 2;
            }
            "--anchor-elo" => {
                anchor_elo = parse_or_exit("--anchor-elo", &next());
                i += 2;
            }
            "--cycle-margin" => {
                cycle_margin = parse_or_exit("--cycle-margin", &next());
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

    // Append archetypes if requested, deduplicating by id.
    if add_archetypes {
        let existing_ids: std::collections::HashSet<String> =
            contenders.iter().map(|(id, _)| id.clone()).collect();
        for &name in BENCH_ARCHETYPES.iter() {
            if !existing_ids.contains(name) {
                contenders.push((name.to_string(), name.to_string()));
            }
        }
    }

    if contenders.is_empty() {
        tracing::error!("no contenders specified; use --contenders or --add-archetypes");
        std::process::exit(2);
    }

    let cfg = TournamentConfig {
        contenders: contenders.clone(),
        mpe,
        max_ticks,
        anchor_id: anchor_id.clone(),
        anchor_elo,
        cycle_margin,
        sizing: A1,
    };

    tracing::info!(
        contenders = cfg.contenders.len(),
        mpe,
        max_ticks,
        anchor_id = %anchor_id,
        anchor_elo,
        cycle_margin,
        out_dir = %out_dir.display(),
        "tournament: starting"
    );
    for (id, spec) in &cfg.contenders {
        tracing::info!(id, spec, "tournament: enrolled contender");
    }

    // CPU device — no CandleBackend, no cuda feature required.
    let device = candle_core::Device::Cpu;

    let r = run_tournament(&cfg, &device)?;

    // --- write outputs ---
    std::fs::create_dir_all(&out_dir)?;

    // ladder.md — sorted by Elo descending
    {
        let mut order: Vec<usize> = (0..r.ids.len()).collect();
        order.sort_by(|&a, &b| {
            r.elo[b]
                .partial_cmp(&r.elo[a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut s = String::from(
            "# Tournament Ladder\n\n| rank | id | elo | winrate_vs_field | spec |\n|---|---|---:|---:|---|\n",
        );
        for (rank, &idx) in order.iter().enumerate() {
            s.push_str(&format!(
                "| {} | {} | {:.0} | {:.3} | `{}` |\n",
                rank + 1,
                r.ids[idx],
                r.elo[idx],
                r.winrate_vs_field[idx],
                r.specs[idx]
            ));
        }
        let cycle_note = if r.cycles.is_empty() {
            "\n_No 3-cycles detected._\n".to_string()
        } else {
            format!("\ncycles found: {}\n", r.cycles.len())
        };
        s.push_str(&cycle_note);
        std::fs::write(out_dir.join("ladder.md"), s)?;
        tracing::info!(path = %out_dir.join("ladder.md").display(), "tournament: wrote ladder.md");
    }

    // win_matrix.csv — decisive scores; NaN → empty string
    {
        let path = out_dir.join("win_matrix.csv");
        let mut f = std::fs::File::create(&path)?;
        // header row: blank + id columns
        write!(f, "")?;
        for id in &r.ids {
            write!(f, ",{}", id)?;
        }
        writeln!(f)?;
        for (i, id) in r.ids.iter().enumerate() {
            write!(f, "{}", id)?;
            for j in 0..r.ids.len() {
                let v = r.win_matrix[i][j];
                if v.is_finite() {
                    write!(f, ",{:.4}", v)?;
                } else {
                    write!(f, ",")?;
                }
            }
            writeln!(f)?;
        }
        tracing::info!(path = %path.display(), "tournament: wrote win_matrix.csv");
    }

    // ws_matrix.csv — worker-share scores; NaN → empty string
    {
        let path = out_dir.join("ws_matrix.csv");
        let mut f = std::fs::File::create(&path)?;
        write!(f, "")?;
        for id in &r.ids {
            write!(f, ",{}", id)?;
        }
        writeln!(f)?;
        for (i, id) in r.ids.iter().enumerate() {
            write!(f, "{}", id)?;
            for j in 0..r.ids.len() {
                let v = r.ws_matrix[i][j];
                if v.is_finite() {
                    write!(f, ",{:.4}", v)?;
                } else {
                    write!(f, ",")?;
                }
            }
            writeln!(f)?;
        }
        tracing::info!(path = %path.display(), "tournament: wrote ws_matrix.csv");
    }

    // ratings.json — NaN-free fields only (serde would error on NaN)
    {
        let path = out_dir.join("ratings.json");
        // Build per-contender rows; skip NaN winrates
        let ids_json: Vec<serde_json::Value> =
            r.ids.iter().map(|s| serde_json::Value::String(s.clone())).collect();
        let specs_json: Vec<serde_json::Value> =
            r.specs.iter().map(|s| serde_json::Value::String(s.clone())).collect();
        // elo is f64 — always finite from BT (log10 of positive numbers + shift)
        let elo_json: Vec<serde_json::Value> =
            r.elo.iter().map(|&e| serde_json::json!(e)).collect();
        // winrate_vs_field: substitute null for NaN
        let wvf_json: Vec<serde_json::Value> = r
            .winrate_vs_field
            .iter()
            .map(|&w| {
                if w.is_finite() {
                    serde_json::json!(w)
                } else {
                    serde_json::Value::Null
                }
            })
            .collect();
        // cycles as [[i,j,k], ...]
        let cycles_json: Vec<serde_json::Value> = r
            .cycles
            .iter()
            .map(|&(ci, cj, ck)| serde_json::json!([ci, cj, ck]))
            .collect();

        let out_json = serde_json::json!({
            "ids": ids_json,
            "specs": specs_json,
            "elo": elo_json,
            "winrate_vs_field": wvf_json,
            "cycles": cycles_json,
        });
        std::fs::write(&path, serde_json::to_string_pretty(&out_json)?)?;
        tracing::info!(path = %path.display(), "tournament: wrote ratings.json");
    }

    // --- tracing::info! summary ---
    let mut order: Vec<usize> = (0..r.ids.len()).collect();
    order.sort_by(|&a, &b| {
        r.elo[b]
            .partial_cmp(&r.elo[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let anchor_rank = order
        .iter()
        .position(|&idx| r.ids[idx] == anchor_id)
        .map(|p| p + 1);

    tracing::info!("=== Tournament Results ===");
    for (rank, &idx) in order.iter().take(3).enumerate() {
        tracing::info!(
            rank = rank + 1,
            id = %r.ids[idx],
            elo = r.elo[idx],
            winrate_vs_field = r.winrate_vs_field[idx],
            "top-{}", rank + 1
        );
    }
    match anchor_rank {
        Some(rank) => tracing::info!(
            anchor_id = %anchor_id,
            anchor_rank = rank,
            "anchor rank"
        ),
        None => tracing::warn!(anchor_id = %anchor_id, "anchor not found in results"),
    }
    tracing::info!(
        cycles = r.cycles.len(),
        "3-cycles detected"
    );

    Ok(())
}

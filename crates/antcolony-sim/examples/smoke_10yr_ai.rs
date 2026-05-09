//! 10-year AI-controlled smoke test for every shipped species.
//!
//! Builds the same starter-formicarium-with-feeder topology the bench
//! harness uses, attaches an external `MlpBrain` (SOTA `mlp_weights_v1.json`
//! by default — heuristic fallback if the weights file is missing),
//! runs 10 in-game years at Seasonal scale, and emits a verbose
//! per-decision log so the run can be studied like a game replay.
//!
//! # Usage
//!
//! ```text
//! cargo run --release -p antcolony-sim --example smoke_10yr_ai -- \
//!     --years 10 --out bench/smoke-10yr-ai/
//! ```
//!
//! Per species the runner writes:
//! - `<out>/<species>/decisions.csv` — one row per brain decision (every
//!   `DECISION_CADENCE` sim ticks). Columns cover the full ColonyAiState
//!   inputs and AiDecision outputs so the run is replayable from disk.
//! - `<out>/<species>/daily.csv` — one row per in-game day with population,
//!   brood pipeline, food economy.
//! - `<out>/<species>/summary.md` — final-state report + final score.
//!
//! Plus `<out>/SUMMARY.md` across all species.
//!
//! # Determinism
//!
//! Seed default 42. Same source + Cargo.lock + same OS produces byte-
//! identical outputs (cross-OS desync documented in HANDOFF.md).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use antcolony_sim::ai::{AiBrain, AiDecision, ColonyAiState, HeuristicBrain, MlpBrain};
use antcolony_sim::{Environment, Simulation, Species, TimeScale, Topology, load_species_dir};

const DECISION_CADENCE: u64 = 5; // mirrors antcolony-net::DECISION_CADENCE

struct CliArgs {
    years: f32,
    seed: u64,
    out_dir: PathBuf,
    weights_path: Option<PathBuf>,
    species_filter: Option<String>,
}

fn parse_args() -> anyhow::Result<CliArgs> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut years = 10.0f32;
    let mut seed = 42u64;
    let mut out_dir = PathBuf::from("bench/smoke-10yr-ai");
    let mut weights_path: Option<PathBuf> =
        Some(PathBuf::from("bench/iterative-fsp/round_1/mlp_weights_v1.json"));
    let mut species_filter: Option<String> = None;
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--years" => {
                years = raw.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(10.0);
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
            "--weights" => {
                weights_path = raw.get(i + 1).map(PathBuf::from);
                i += 2;
            }
            "--no-mlp" => {
                weights_path = None;
                i += 1;
            }
            "--species" => {
                species_filter = raw.get(i + 1).cloned();
                i += 2;
            }
            "-h" | "--help" => {
                eprintln!(
                    "smoke_10yr_ai — 10-year AI smoke test on every species\n\n\
                     FLAGS:\n  \
                       --years <n>        years per species (default 10)\n  \
                       --seed <n>         RNG seed (default 42)\n  \
                       --out <dir>        output directory (default bench/smoke-10yr-ai)\n  \
                       --weights <path>   MlpBrain weights JSON (default SOTA v1)\n  \
                       --no-mlp           use HeuristicBrain instead of MlpBrain\n  \
                       --species <id>     run only one species (default: all)\n"
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown arg `{other}` — try --help"),
        }
    }
    Ok(CliArgs { years, seed, out_dir, weights_path, species_filter })
}

fn make_brain(weights_path: Option<&Path>, label: &str) -> Box<dyn AiBrain> {
    if let Some(p) = weights_path {
        match MlpBrain::load(p, label.to_string()) {
            Ok(b) => {
                tracing::info!(path = %p.display(), "loaded MlpBrain");
                return Box::new(b);
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %p.display(),
                    "MlpBrain load failed — falling back to HeuristicBrain");
            }
        }
    }
    Box::new(HeuristicBrain::new(5.0))
}

struct SpeciesRun {
    species_id: String,
    target_ticks: u64,
    final_tick: u64,
    final_workers: u32,
    final_soldiers: u32,
    final_breeders: u32,
    final_queens: u32,
    final_food: f32,
    decisions_logged: u64,
    survived: bool,
}

fn run_species(
    species: Species,
    args: &CliArgs,
    out_root: &Path,
) -> anyhow::Result<SpeciesRun> {
    let species_id = species.id.clone();
    let species_dir = out_root.join(&species_id);
    std::fs::create_dir_all(&species_dir)?;

    let env = Environment {
        time_scale: TimeScale::Seasonal,
        seed: args.seed,
        ..Environment::default()
    };
    let sim_cfg = species.apply(&env);

    let nest_w = (env.world_width / 4).max(24);
    let nest_h = (env.world_height / 3).max(20);
    let out_w = env.world_width;
    let out_h = env.world_height;
    let dish_w = (out_w / 3).max(18);
    let dish_h = (out_h / 3).max(14);
    let mut topology = Topology::starter_formicarium_with_feeder(
        (nest_w, nest_h),
        (out_w, out_h),
        (dish_w, dish_h),
    );
    let _underground = topology.attach_underground(0, 0, nest_w.max(32), nest_h.max(24));
    topology.fit_bore_to_species(species.appearance.size_mm, species.biology.polymorphic);

    let mut sim = Simulation::new_with_topology(sim_cfg, topology, env.seed);
    sim.set_environment(&env);

    let ow = out_w as i64;
    let oh = out_h as i64;
    sim.spawn_food_cluster_on(1, ow / 5, oh / 5, 4, 40);
    sim.spawn_food_cluster_on(1, ow - ow / 5, oh - oh / 5, 4, 40);
    sim.spawn_food_cluster_on(1, ow - ow / 5, oh / 5, 3, 30);

    let target_ticks = (args.years as f64
        * 31_536_000.0
        * env.tick_rate_hz as f64
        / TimeScale::Seasonal.multiplier() as f64) as u64;
    let ticks_per_day = env.in_game_seconds_to_ticks(86_400).max(1) as u64;

    let mut brain = make_brain(args.weights_path.as_deref(), &format!("mlp-{species_id}"));

    let decisions_path = species_dir.join("decisions.csv");
    let daily_path = species_dir.join("daily.csv");
    let mut dec_w = BufWriter::new(File::create(&decisions_path)?);
    let mut day_w = BufWriter::new(File::create(&daily_path)?);

    writeln!(
        dec_w,
        "tick,doy,temp_c,is_day,diapause,\
         food,inflow,workers,soldiers,breeders,eggs,larvae,pupae,queens,\
         losses,enemy_dist,enemy_w,enemy_s,\
         out_cw,out_cs,out_cb,out_wf,out_wd,out_wn"
    )?;
    writeln!(
        day_w,
        "tick,day,year,doy,temp_c,workers,soldiers,breeders,queens,eggs,larvae,pupae,food,inflow"
    )?;

    let _span = tracing::info_span!("smoke", species = %species_id, ticks = target_ticks).entered();
    tracing::info!("start");

    let mut decisions_logged: u64 = 0;
    let mut next_day_sample: u64 = ticks_per_day;
    let mut last_log_real = std::time::Instant::now();

    for _ in 0..target_ticks {
        if sim.tick % DECISION_CADENCE == 0 {
            if let Some(state) = sim.colony_ai_state(0) {
                let decision = brain.decide(&state);
                sim.apply_ai_decision(0, &decision);
                log_decision(&mut dec_w, sim.tick, &state, &decision)?;
                decisions_logged += 1;

                if decisions_logged % 5_000 == 0 {
                    tracing::info!(
                        tick = sim.tick,
                        decisions = decisions_logged,
                        workers = state.worker_count,
                        food = state.food_stored,
                        "progress"
                    );
                    let now = std::time::Instant::now();
                    if now.duration_since(last_log_real).as_secs() > 60 {
                        let pct = (sim.tick as f64 / target_ticks as f64) * 100.0;
                        eprintln!(
                            "  [{species_id}] tick={} / {} ({:.1}%) workers={} food={:.1}",
                            sim.tick, target_ticks, pct, state.worker_count, state.food_stored
                        );
                        last_log_real = now;
                    }
                }
            }
        }
        sim.tick();

        if sim.tick >= next_day_sample {
            log_daily(&mut day_w, &sim)?;
            next_day_sample = next_day_sample.saturating_add(ticks_per_day);
        }
    }
    log_daily(&mut day_w, &sim)?;
    dec_w.flush()?;
    day_w.flush()?;

    let (workers, soldiers, breeders, queens, food, survived) = match sim.colonies.first() {
        Some(c) => (
            c.population.workers,
            c.population.soldiers,
            c.population.breeders,
            sim.ants
                .iter()
                .filter(|a| a.colony_id == 0 && matches!(a.caste, antcolony_sim::AntCaste::Queen))
                .count() as u32,
            c.food_stored,
            c.population.workers > 0,
        ),
        None => (0, 0, 0, 0, 0.0, false),
    };

    write_species_summary(
        &species_dir,
        &species_id,
        target_ticks,
        sim.tick,
        workers,
        soldiers,
        breeders,
        queens,
        food,
        decisions_logged,
        survived,
    )?;

    tracing::info!(
        species = %species_id,
        decisions = decisions_logged,
        workers,
        soldiers,
        breeders,
        queens,
        food,
        survived,
        "done"
    );

    Ok(SpeciesRun {
        species_id,
        target_ticks,
        final_tick: sim.tick,
        final_workers: workers,
        final_soldiers: soldiers,
        final_breeders: breeders,
        final_queens: queens,
        final_food: food,
        decisions_logged,
        survived,
    })
}

fn log_decision<W: Write>(
    w: &mut W,
    tick: u64,
    s: &ColonyAiState,
    d: &AiDecision,
) -> std::io::Result<()> {
    let ed = if s.enemy_distance_min.is_finite() {
        s.enemy_distance_min
    } else {
        -1.0
    };
    writeln!(
        w,
        "{tick},{doy},{temp:.2},{is_day},{diapause},\
         {food:.3},{inflow:.3},{w_count},{s_count},{b_count},{eggs},{larvae},{pupae},{queens},\
         {losses},{ed:.2},{ew},{es},\
         {cw:.4},{cs:.4},{cb:.4},{wf:.4},{wd:.4},{wn:.4}",
        doy = s.day_of_year,
        temp = s.ambient_temp_c,
        is_day = s.is_daytime as u8,
        diapause = s.diapause_active as u8,
        food = s.food_stored,
        inflow = s.food_inflow_recent,
        w_count = s.worker_count,
        s_count = s.soldier_count,
        b_count = s.breeder_count,
        eggs = s.brood_egg,
        larvae = s.brood_larva,
        pupae = s.brood_pupa,
        queens = s.queens_alive,
        losses = s.combat_losses_recent,
        ew = s.enemy_worker_count,
        es = s.enemy_soldier_count,
        cw = d.caste_ratio_worker,
        cs = d.caste_ratio_soldier,
        cb = d.caste_ratio_breeder,
        wf = d.forage_weight,
        wd = d.dig_weight,
        wn = d.nurse_weight,
    )
}

fn log_daily<W: Write>(w: &mut W, sim: &Simulation) -> std::io::Result<()> {
    let day = sim.in_game_total_days();
    let year = sim.in_game_year();
    let doy = sim.day_of_year();
    let temp = sim.ambient_temp_c();
    let queens = sim
        .ants
        .iter()
        .filter(|a| a.colony_id == 0 && matches!(a.caste, antcolony_sim::AntCaste::Queen))
        .count() as u32;
    if let Some(c) = sim.colonies.first() {
        writeln!(
            w,
            "{tick},{day},{year},{doy},{temp:.2},{w},{s},{b},{q},{eggs},{larvae},{pupae},{food:.3},{inflow:.3}",
            tick = sim.tick,
            w = c.population.workers,
            s = c.population.soldiers,
            b = c.population.breeders,
            q = queens,
            eggs = c.eggs,
            larvae = c.larvae,
            pupae = c.pupae,
            food = c.food_stored,
            inflow = c.food_inflow_recent,
        )
    } else {
        writeln!(w, "{},{day},{year},{doy},{temp:.2},0,0,0,0,0,0,0,0.0,0.0", sim.tick)
    }
}

fn write_species_summary(
    dir: &Path,
    id: &str,
    target_ticks: u64,
    final_tick: u64,
    workers: u32,
    soldiers: u32,
    breeders: u32,
    queens: u32,
    food: f32,
    decisions: u64,
    survived: bool,
) -> std::io::Result<()> {
    let mut s = String::new();
    use std::fmt::Write as _;
    writeln!(s, "# {id} — 10yr AI smoke summary").ok();
    writeln!(s).ok();
    writeln!(s, "- target_ticks: {target_ticks}").ok();
    writeln!(s, "- final_tick:   {final_tick}").ok();
    writeln!(s, "- decisions logged: {decisions}").ok();
    writeln!(s, "- survived: **{}**", if survived { "YES" } else { "NO" }).ok();
    writeln!(s).ok();
    writeln!(s, "## Final state").ok();
    writeln!(s, "- workers: {workers}").ok();
    writeln!(s, "- soldiers: {soldiers}").ok();
    writeln!(s, "- breeders: {breeders}").ok();
    writeln!(s, "- queens alive: {queens}").ok();
    writeln!(s, "- food stored: {food:.2}").ok();
    std::fs::write(dir.join("summary.md"), s)
}

fn write_top_summary(out_dir: &Path, runs: &[SpeciesRun]) -> std::io::Result<()> {
    let mut s = String::new();
    use std::fmt::Write as _;
    writeln!(s, "# 10yr AI Smoke Test Summary").ok();
    writeln!(s).ok();
    writeln!(
        s,
        "| species | survived | workers | soldiers | breeders | queens | food | decisions | ticks |"
    ).ok();
    writeln!(
        s,
        "|---------|----------|--------:|---------:|---------:|-------:|-----:|----------:|------:|"
    ).ok();
    for r in runs {
        writeln!(
            s,
            "| `{id}` | {sv} | {w} | {sd} | {b} | {q} | {f:.1} | {d} | {t}/{tt} |",
            id = r.species_id,
            sv = if r.survived { "✅" } else { "❌" },
            w = r.final_workers,
            sd = r.final_soldiers,
            b = r.final_breeders,
            q = r.final_queens,
            f = r.final_food,
            d = r.decisions_logged,
            t = r.final_tick,
            tt = r.target_ticks,
        ).ok();
    }
    std::fs::write(out_dir.join("SUMMARY.md"), s)
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_|
                "antcolony_sim=warn,smoke_10yr_ai=info".into()),
        )
        .with_target(false)
        .init();

    let args = parse_args()?;
    std::fs::create_dir_all(&args.out_dir)?;

    let species_list = load_species_dir("assets/species")?;
    let to_run: Vec<Species> = match &args.species_filter {
        Some(id) => species_list.into_iter().filter(|s| &s.id == id).collect(),
        None => species_list,
    };
    if to_run.is_empty() {
        anyhow::bail!("no species matched");
    }

    eprintln!(
        "smoke_10yr_ai: {} species, {} years each, scale=Seasonal, seed={}, out={}",
        to_run.len(),
        args.years,
        args.seed,
        args.out_dir.display(),
    );
    if let Some(p) = &args.weights_path {
        eprintln!("brain: MlpBrain ({})", p.display());
    } else {
        eprintln!("brain: HeuristicBrain (no MLP)");
    }

    let mut runs = Vec::new();
    for species in to_run {
        let id = species.id.clone();
        eprintln!("→ running {id}…");
        let t0 = std::time::Instant::now();
        let r = run_species(species, &args, &args.out_dir)?;
        let dt = t0.elapsed();
        eprintln!(
            "  {id}: survived={} workers={} food={:.1} decisions={} ({:.1}s)",
            r.survived, r.final_workers, r.final_food, r.decisions_logged, dt.as_secs_f32(),
        );
        runs.push(r);
        write_top_summary(&args.out_dir, &runs)?;
    }
    eprintln!("smoke_10yr_ai: done. Reports in {}", args.out_dir.display());
    Ok(())
}

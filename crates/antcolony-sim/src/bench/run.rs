//! Runs a species through the simulation and produces a `BenchResult`.
//!
//! # Audit notes for ecologists
//!
//! - **Time scale.** Default is `TimeScale::Seasonal` (60× real-time), the
//!   only scale the sim is currently calibrated for. Higher scales
//!   (`Timelapse`, 1440×) are known to produce starvation collapse due to
//!   per-tick consumption scaling outpacing pheromone trail establishment.
//!   See `HANDOFF.md` "Open Bug — Long-run colony collapse at non-Seasonal
//!   time scales." The harness emits a loud warning if you pick a non-default
//!   scale.
//!
//! - **Topology.** Uses the same starter-formicarium-with-feeder topology
//!   that Keeper mode picker creates. Auto-attached underground layer.
//!   This is the *standard* setup; results from different topologies are
//!   not directly comparable.
//!
//! - **Seed.** Default seed is fixed (=42). Re-runs with the same seed
//!   produce byte-identical telemetry — see `tests/bench_determinism.rs`.
//!
//! - **Sample interval.** Defaults to 1 sample per in-game day at Seasonal
//!   scale. Storage cost ~80 bytes per sample → 25-year run ≈ 730KB.

use crate::bench::expected::{self, SpeciesExpectations};
use crate::bench::metrics::{self, SpeciesScore, TickSample};
use crate::{
    AntCaste, Environment, Simulation, Species, TimeScale, Topology,
};

/// Configuration for one bench run.
#[derive(Debug, Clone)]
pub struct BenchRunConfig {
    /// Species to run (must be one of the loaded species).
    pub species: Species,
    /// How many in-game years to simulate.
    pub years: f32,
    /// Time scale. Defaults to `Seasonal` — see module-level audit notes.
    pub time_scale: TimeScale,
    /// Random seed.
    pub seed: u64,
    /// Sample every N in-game days. Default 1 = daily.
    pub sample_every_days: u32,
}

impl BenchRunConfig {
    /// Default config for a 5-year Seasonal run with seed=42 and daily sampling.
    pub fn standard_5yr(species: Species) -> Self {
        Self {
            species,
            years: 5.0,
            time_scale: TimeScale::Seasonal,
            seed: 42,
            sample_every_days: 1,
        }
    }
}

/// Output of one bench run. Carries everything the report layer needs
/// to write CSV + Markdown + score.
#[derive(Debug, Clone)]
pub struct BenchResult {
    pub species_id: String,
    pub config_summary: String,
    pub samples: Vec<TickSample>,
    pub score: SpeciesScore,
    pub expectations: Option<SpeciesExpectations>,
    /// Caveats / warnings raised during the run. Surfaced in the report.
    pub caveats: Vec<String>,
}

/// Minimum number of samples required to attempt scoring. Below this
/// threshold the run is too short to produce meaningful metrics and we
/// return all-`None` rather than a tautologically perfect score from a
/// single tick-0 sample.
const MIN_SAMPLES_FOR_SCORING: usize = 30;

/// Hard upper bound on `years` to prevent runaway sample-vector allocation.
/// 1000 years at daily sampling is ~365k samples × ~80 bytes ≈ 29MB.
const MAX_YEARS_PER_RUN: f32 = 1000.0;

/// Run a single species bench. Produces a `BenchResult` ready for reporting.
///
/// This is the public entry point — `examples/species_bench.rs` calls
/// it and so do the determinism / regression tests.
pub fn run_one(cfg: BenchRunConfig) -> BenchResult {
    let mut caveats = Vec::new();

    // Validate config — reject silently-bogus values BEFORE we run anything.
    // A run with years=0 or NaN would otherwise produce a single tick-0
    // sample and report a tautological "100% survival" score.
    if !cfg.years.is_finite() || cfg.years <= 0.0 || cfg.years > MAX_YEARS_PER_RUN {
        return invalid_config_result(
            &cfg,
            format!(
                "Invalid `years = {}` — must be finite, > 0, and ≤ {} \
                 (this prevents zero-tick perfect-score runs and OOM allocation).",
                cfg.years, MAX_YEARS_PER_RUN
            ),
        );
    }
    if cfg.sample_every_days == 0 {
        return invalid_config_result(
            &cfg,
            "Invalid `sample_every_days = 0` — must be ≥ 1.".to_string(),
        );
    }
    let scale_mult = cfg.time_scale.multiplier();
    if !scale_mult.is_finite() || scale_mult <= 0.0 {
        return invalid_config_result(
            &cfg,
            format!("Invalid time-scale multiplier {scale_mult}."),
        );
    }

    if !matches!(cfg.time_scale, TimeScale::Seasonal) {
        caveats.push(format!(
            "Non-default time scale `{}` — sim is only calibrated at Seasonal (60×). \
             Higher scales (Timelapse, 1440×) suffer the long-run-collapse bug \
             documented in HANDOFF.md (per-tick consumption auto-scales but trail \
             throughput does not). Treat results with extreme skepticism.",
            cfg.time_scale.label()
        ));
    }

    // Build env + sim using the same recipe as the picker (mirrors
    // examples/colony_diag.rs). Kept inline rather than refactoring
    // the picker setup so we don't change observable game behavior
    // while building the audit harness.
    let env = Environment {
        time_scale: cfg.time_scale,
        seed: cfg.seed,
        ..Environment::default()
    };
    let sim_cfg = cfg.species.apply(&env);

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
    let _underground_id = topology.attach_underground(0, 0, nest_w.max(32), nest_h.max(24));
    topology.fit_bore_to_species(
        cfg.species.appearance.size_mm,
        cfg.species.biology.polymorphic,
    );

    let mut sim = Simulation::new_with_topology(sim_cfg, topology, env.seed);
    sim.set_environment(&env);

    // Same food seeding as the picker.
    let ow = out_w as i64;
    let oh = out_h as i64;
    sim.spawn_food_cluster_on(1, ow / 5, oh / 5, 4, 40);
    sim.spawn_food_cluster_on(1, ow - ow / 5, oh - oh / 5, 4, 40);
    sim.spawn_food_cluster_on(1, ow - ow / 5, oh / 5, 3, 30);

    // Compute target ticks the same way colony_diag does.
    let target_ticks = (cfg.years as f64
        * 31_536_000.0
        * env.tick_rate_hz as f64
        / cfg.time_scale.multiplier() as f64) as u64;

    // Precompute sample interval in ticks.
    let ticks_per_in_game_day = env.in_game_seconds_to_ticks(86_400).max(1) as u64;
    let sample_interval_ticks = ticks_per_in_game_day * cfg.sample_every_days as u64;

    let mut samples: Vec<TickSample> = Vec::with_capacity(
        (target_ticks / sample_interval_ticks.max(1)) as usize + 8,
    );

    // Track starvation deaths cumulatively. ColonyState only exposes
    // `pending_starvation_deaths` (the per-tick draw-down), so we
    // accumulate them ourselves.
    let mut starvation_total: u32 = 0;

    // Track whether colony 0 was ever missing — distinguishes a sim-init
    // failure (colony 0 never existed) from a biological extinction.
    let mut missing_colony_seen_at_init = false;

    // Initial sample at tick 0.
    match snapshot(&sim, &env, starvation_total) {
        SnapshotOutcome::Present(s) => samples.push(s),
        SnapshotOutcome::Missing(s, reason) => {
            missing_colony_seen_at_init = true;
            caveats.push(format!(
                "Sim-init failure (NOT extinction): {reason}. \
                 Colony 0 was missing at the very first sample, which means the \
                 starter formicarium did not produce a colony — this is a sim-side \
                 bug, not a biological outcome. Scores are unreliable."
            ));
            samples.push(s);
        }
    }

    let mut next_sample_tick = sample_interval_ticks;

    for _ in 0..target_ticks {
        // Capture pending_starvation_deaths BEFORE tick — Simulation::tick
        // applies them and resets the field. (This is a sim-internal
        // detail but the only way to count without modifying core sim code.)
        if let Some(c) = sim.colonies.get(0) {
            starvation_total = starvation_total.saturating_add(c.pending_starvation_deaths);
        }
        sim.tick();

        if sim.tick >= next_sample_tick {
            match snapshot(&sim, &env, starvation_total) {
                SnapshotOutcome::Present(s) => samples.push(s),
                SnapshotOutcome::Missing(s, _) => samples.push(s),
            }
            next_sample_tick = next_sample_tick.saturating_add(sample_interval_ticks);
        }
    }

    // Always capture a final sample so the last data point reflects end-of-run state.
    let last_tick_sampled = samples.last().map(|s| s.tick);
    if last_tick_sampled != Some(sim.tick) {
        match snapshot(&sim, &env, starvation_total) {
            SnapshotOutcome::Present(s) => samples.push(s),
            SnapshotOutcome::Missing(s, _) => samples.push(s),
        }
    }

    // If colony was missing at init, suppress scoring entirely — a metric
    // result here would be a lie.
    let score = if missing_colony_seen_at_init || samples.len() < MIN_SAMPLES_FOR_SCORING {
        if samples.len() < MIN_SAMPLES_FOR_SCORING && !missing_colony_seen_at_init {
            caveats.push(format!(
                "Run produced only {} sample(s); minimum {} required to score. \
                 Increase --years or decrease --sample-every-days.",
                samples.len(),
                MIN_SAMPLES_FOR_SCORING,
            ));
        }
        SpeciesScore {
            species_id: cfg.species.id.clone(),
            colony_survival: None,
            queen_survival: None,
            brood_pipeline_health: None,
            population_stability: None,
            food_economy: None,
            hibernation_compliance: None,
        }
    } else {
        score_run(&cfg, &samples)
    };

    let expectations = expected::for_species_id(&cfg.species.id);
    if expectations.is_none() {
        caveats.push(format!(
            "No expected-range table for species `{}` — score reported but \
             not validated against literature. Add a SpeciesExpectations entry \
             in src/bench/expected.rs to enable comparison.",
            cfg.species.id
        ));
    }

    let config_summary = format!(
        "species={} years={:.1} scale={} seed={} sample_every_days={} target_ticks={}",
        cfg.species.id,
        cfg.years,
        cfg.time_scale.label(),
        cfg.seed,
        cfg.sample_every_days,
        target_ticks,
    );

    BenchResult {
        species_id: cfg.species.id.clone(),
        config_summary,
        samples,
        score,
        expectations,
        caveats,
    }
}

/// Result of attempting a snapshot. `Missing` means colony 0 doesn't
/// exist — distinguished from `Present` with all-zero counts so the
/// caller can tell "sim-init failure" from "biological extinction."
enum SnapshotOutcome {
    Present(TickSample),
    Missing(TickSample, String),
}

fn snapshot(sim: &Simulation, _env: &Environment, starvation_total: u32) -> SnapshotOutcome {
    let day = sim.in_game_total_days();
    let year = sim.in_game_year();
    let doy = sim.day_of_year();
    let temp = sim.ambient_temp_c();
    let total_ant_entities = sim.ants.len() as u32;
    let queens_alive = sim
        .ants
        .iter()
        .filter(|a| a.caste == AntCaste::Queen && a.colony_id == 0)
        .count() as u32;
    match sim.colonies.get(0) {
        Some(c) => SnapshotOutcome::Present(metrics::snapshot_from_colony(
            sim.tick,
            day,
            year,
            doy,
            temp,
            c,
            queens_alive,
            total_ant_entities,
            starvation_total,
        )),
        None => SnapshotOutcome::Missing(
            TickSample {
                tick: sim.tick,
                in_game_day: day,
                in_game_year: year,
                day_of_year: doy,
                ambient_temp_c: temp,
                workers: 0,
                soldiers: 0,
                breeders: 0,
                queens_alive: 0,
                total_ant_entities,
                eggs: 0,
                larvae: 0,
                pupae: 0,
                food_returned_cumulative: 0,
                food_stored: 0.0,
                food_inflow_recent: 0.0,
                starvation_deaths_cumulative: starvation_total,
            },
            format!("colony 0 missing at tick {}", sim.tick),
        ),
    }
}

/// Build an early-return BenchResult for a config that failed validation.
fn invalid_config_result(cfg: &BenchRunConfig, reason: String) -> BenchResult {
    let summary = format!(
        "species={} years={} scale={} seed={} INVALID",
        cfg.species.id,
        cfg.years,
        cfg.time_scale.label(),
        cfg.seed,
    );
    BenchResult {
        species_id: cfg.species.id.clone(),
        config_summary: summary,
        samples: Vec::new(),
        score: SpeciesScore {
            species_id: cfg.species.id.clone(),
            colony_survival: None,
            queen_survival: None,
            brood_pipeline_health: None,
            population_stability: None,
            food_economy: None,
            hibernation_compliance: None,
        },
        expectations: expected::for_species_id(&cfg.species.id),
        caveats: vec![format!("CONFIG REJECTED: {reason}")],
    }
}

fn score_run(cfg: &BenchRunConfig, samples: &[TickSample]) -> SpeciesScore {
    if samples.is_empty() {
        return SpeciesScore {
            species_id: cfg.species.id.clone(),
            colony_survival: None,
            queen_survival: None,
            brood_pipeline_health: None,
            population_stability: None,
            food_economy: None,
            hibernation_compliance: None,
        };
    }

    let Some(last) = samples.last() else {
        return SpeciesScore {
            species_id: cfg.species.id.clone(),
            colony_survival: None,
            queen_survival: None,
            brood_pipeline_health: None,
            population_stability: None,
            food_economy: None,
            hibernation_compliance: None,
        };
    };
    let colony_survival = if last.workers > 0 && last.queens_alive > 0 {
        Some(1.0)
    } else {
        Some(0.0)
    };
    let queen_survival = Some(if last.queens_alive > 0 { 1.0 } else { 0.0 });

    // Late-window samples = final 25% of the run.
    let split = (samples.len() * 3) / 4;
    let late = &samples[split..];

    let brood_pipeline_health = metrics::compute_brood_pipeline_health(late);
    let population_stability = metrics::compute_population_stability(late);

    // Food economy: compare food_returned in the final year to food consumed.
    // Food consumed = workers * food_per_adult_per_day * 365.
    let food_economy = compute_food_economy(cfg, samples);

    // Hibernation: only meaningful for species with hibernation_required.
    let hibernation_compliance = if cfg.species.biology.hibernation_required {
        // Cheap proxy: count years where workers count dropped below 50%
        // of annual peak for at least min_diapause_days. Without per-day
        // diapause flag exposed by Simulation, this is the best we can
        // do non-invasively.
        compute_hibernation_compliance_proxy(cfg, samples)
    } else {
        Some(1.0)
    };

    SpeciesScore {
        species_id: cfg.species.id.clone(),
        colony_survival,
        queen_survival,
        brood_pipeline_health,
        population_stability,
        food_economy,
        hibernation_compliance,
    }
}

fn compute_food_economy(cfg: &BenchRunConfig, samples: &[TickSample]) -> Option<f64> {
    if samples.len() < 2 {
        return None;
    }
    let final_year = samples.last()?.in_game_year;
    if final_year == 0 {
        // Run never crossed a year boundary — no full-year window available.
        return None;
    }
    // The numerator and denominator MUST cover the same window. Use the
    // first sample of `final_year` as the start, the last sample as the
    // end. Any prior off-by-2× window (using `final_year - 1`) inflated
    // sustainability ~2×.
    let year_start = samples
        .iter()
        .find(|s| s.in_game_year == final_year)?;
    let year_end = samples.last()?;

    let returned_in_year = year_end
        .food_returned_cumulative
        .saturating_sub(year_start.food_returned_cumulative) as f64;

    // Approximate consumed via mean worker count over the same final-year
    // window × food/day × days. Note: this excludes soldiers/breeders/queens
    // from the consumption denominator, which understates true demand for
    // polymorphic / queen-heavy species (Camponotus, Formica polygyne).
    let final_year_samples: Vec<&TickSample> = samples
        .iter()
        .filter(|s| s.in_game_year == final_year)
        .collect();
    if final_year_samples.is_empty() {
        return None;
    }
    let mean_workers: f64 = final_year_samples
        .iter()
        .map(|s| s.workers as f64)
        .sum::<f64>()
        / final_year_samples.len() as f64;
    if !mean_workers.is_finite() {
        return None;
    }
    let food_per_day = cfg.species.growth.food_per_adult_per_day as f64;
    if !food_per_day.is_finite() || food_per_day <= 0.0 {
        return None;
    }
    let consumed = mean_workers * food_per_day * 365.0;
    if !consumed.is_finite() || consumed <= 0.0 {
        return None;
    }
    let ratio = returned_in_year / consumed;
    if !ratio.is_finite() {
        return None;
    }
    Some(ratio)
}

/// Hibernation cold threshold (°C). Sourced from
/// `AntConfig::hibernation_cold_threshold_c`'s default in `species::apply`.
/// If a future schema lets species override this per-species, plumb that
/// value in instead of the constant — see SF-H6 in the bench audit.
const HIBERNATION_COLD_THRESHOLD_C: f32 = 10.0;

/// Minimum consecutive in-game days below threshold required to count a
/// year as "compliant." Matches `Biology::min_diapause_days` semantics
/// but the sim's default rule of thumb is 30+ days.
fn compute_hibernation_compliance_proxy(
    cfg: &BenchRunConfig,
    samples: &[TickSample],
) -> Option<f64> {
    let last = samples.last()?;
    let max_year = last.in_game_year;
    if max_year == 0 {
        return None;
    }
    // The user's `min_diapause_days` from the species TOML is the right
    // threshold (already in `cfg.species.biology`).
    let required_days = cfg.species.biology.min_diapause_days.max(1);
    // cold_run is incremented per *sample*, but the threshold is in days.
    // Convert: 1 sample == `sample_every_days` days, so sample_count needed
    // = ceil(required_days / sample_every_days). Pre-fix this was hardcoded
    // to `>= 30` which silently undercounted at sample_every_days > 1.
    let required_samples = required_days.div_ceil(cfg.sample_every_days.max(1));

    let mut compliant_years = 0u32;
    for year in 1..=max_year {
        let mut cold_run = 0u32;
        let mut max_cold_run = 0u32;
        for s in samples.iter().filter(|s| s.in_game_year == year) {
            if s.ambient_temp_c < HIBERNATION_COLD_THRESHOLD_C {
                cold_run += 1;
                max_cold_run = max_cold_run.max(cold_run);
            } else {
                cold_run = 0;
            }
        }
        if max_cold_run >= required_samples {
            compliant_years += 1;
        }
    }
    Some(compliant_years as f64 / max_year as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Species;

    /// Smoke test: bench Lasius for a tiny fraction of a year, assert
    /// the harness produces samples and emits a sensible caveat. Uses
    /// Timelapse + 0.01 years (~3.65 in-game days, ~6,500 ticks) purely
    /// to keep the unit test fast — the long-run-collapse bug at
    /// non-Seasonal scales only shows up over many simulated years.
    ///
    /// The MIN_SAMPLES_FOR_SCORING gate (30 samples) means this short
    /// run will report `None` for all metrics + a "too short to score"
    /// caveat. That is the *correct* behavior — we explicitly verify it.
    #[test]
    fn lasius_smoke_runs_and_emits_too_short_caveat() {
        let species = load_test_species("lasius_niger");
        let cfg = BenchRunConfig {
            species,
            years: 0.01,
            time_scale: TimeScale::Timelapse,
            seed: 42,
            sample_every_days: 1,
        };
        let result = run_one(cfg);
        assert!(!result.samples.is_empty(), "no samples produced");
        assert!(
            result.expectations.is_some(),
            "lasius should have expectations",
        );
        // Non-default scale should emit a caveat.
        assert!(
            result.caveats.iter().any(|c| c.contains("Timelapse")),
            "non-default scale should warn",
        );
        // Below the MIN_SAMPLES gate, the harness must report no score
        // (rather than a tautological perfect score from tick-0 sample).
        assert!(
            result.score.composite_0_to_100().is_none(),
            "below-min-samples runs must NOT produce a score",
        );
        assert!(
            result.caveats.iter().any(|c| c.contains("minimum")),
            "below-min-samples runs must emit a 'too short' caveat",
        );
    }

    /// Validates that an invalid config (years=0) is rejected with a
    /// clear caveat rather than silently producing a perfect score.
    #[test]
    fn invalid_years_zero_is_rejected() {
        let species = load_test_species("lasius_niger");
        let cfg = BenchRunConfig {
            species,
            years: 0.0,
            time_scale: TimeScale::Seasonal,
            seed: 42,
            sample_every_days: 1,
        };
        let result = run_one(cfg);
        assert!(result.samples.is_empty(), "rejected config should not run");
        assert!(result.score.composite_0_to_100().is_none());
        assert!(
            result
                .caveats
                .iter()
                .any(|c| c.contains("CONFIG REJECTED")),
            "invalid years should produce CONFIG REJECTED caveat",
        );
    }

    #[test]
    fn invalid_years_nan_is_rejected() {
        let species = load_test_species("lasius_niger");
        let cfg = BenchRunConfig {
            species,
            years: f32::NAN,
            time_scale: TimeScale::Seasonal,
            seed: 42,
            sample_every_days: 1,
        };
        let result = run_one(cfg);
        assert!(result.samples.is_empty());
        assert!(
            result
                .caveats
                .iter()
                .any(|c| c.contains("CONFIG REJECTED")),
        );
    }

    #[test]
    fn invalid_sample_every_days_zero_is_rejected() {
        let species = load_test_species("lasius_niger");
        let cfg = BenchRunConfig {
            species,
            years: 1.0,
            time_scale: TimeScale::Seasonal,
            seed: 42,
            sample_every_days: 0,
        };
        let result = run_one(cfg);
        assert!(result.samples.is_empty());
        assert!(
            result
                .caveats
                .iter()
                .any(|c| c.contains("CONFIG REJECTED")),
        );
    }

    fn load_test_species(id: &str) -> Species {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("assets")
            .join("species");
        let species_list = crate::load_species_dir(&dir).unwrap();
        species_list
            .into_iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("species {id} not found"))
    }
}

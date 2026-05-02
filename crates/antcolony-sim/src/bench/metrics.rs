//! Bench harness metrics — what we observe per tick / per year, and how we score.
//!
//! # Audit discipline
//!
//! Every metric carries:
//! - `human_name`: plain-English label that an ecologist (not a programmer) can read.
//! - `human_definition`: one-paragraph explanation of exactly what is computed.
//! - `units`: explicit measurement unit (e.g. "workers", "in-game days", "food units/year").
//! - `interpretation`: how to read the value (e.g. "higher is better, but >X is suspicious").
//!
//! When in doubt, **add words, not numbers**. A confused ecologist
//! reading a CSV column header should be able to look up its definition
//! here and understand it without reading Rust source.
//!
//! # No silent failures
//!
//! All metric computation that involves division, NaN possibility, or
//! empty-collection edge cases must return `Option<f64>` and let the
//! report layer render `n/a` rather than producing a bogus number.

use crate::ColonyState;

/// Per-tick observation snapshot. Cheap to record (a few u32/f32),
/// captured at every `sample_interval_ticks` step of the bench run.
///
/// Field naming is verbose on purpose — `total_adult_ants` not
/// `total`, `food_returned_cumulative` not `food`. CSV column headers
/// are derived directly from these names.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TickSample {
    /// Sim tick at which the sample was taken. Zero-based.
    pub tick: u64,
    /// Whole in-game days elapsed since sim start.
    pub in_game_day: u32,
    /// Whole in-game years elapsed (in_game_day / 365).
    pub in_game_year: u32,
    /// Day-of-year (0-364), useful for spotting seasonality.
    pub day_of_year: u32,
    /// Ambient temperature (°C) — drives diapause logic.
    pub ambient_temp_c: f32,
    /// Number of adult workers in colony 0.
    pub workers: u32,
    /// Number of adult soldiers (always 0 for monomorphic species).
    pub soldiers: u32,
    /// Number of adult breeders / alates.
    pub breeders: u32,
    /// Number of queen ants alive in colony 0 only.
    pub queens_alive: u32,
    /// Total ant entities across ALL colonies (different scope from the per-caste
    /// fields above, which are colony-0 only). For single-colony bench runs the
    /// difference is `queens_alive`; for future multi-colony runs this counts every ant.
    pub total_ant_entities: u32,
    /// Eggs in the brood pool.
    pub eggs: u32,
    /// Larvae in the brood pool.
    pub larvae: u32,
    /// Pupae in the brood pool.
    pub pupae: u32,
    /// Cumulative food units returned to nest by foragers since sim start.
    pub food_returned_cumulative: u32,
    /// Food units currently in storage.
    pub food_stored: f32,
    /// Recent rolling-average food inflow (sim-internal smoothing).
    pub food_inflow_recent: f32,
    /// Cumulative ants killed by starvation since sim start.
    pub starvation_deaths_cumulative: u32,
}

impl TickSample {
    /// Headers for CSV output, one per field, in the order written.
    /// Single source of truth — `to_csv_row` MUST match this.
    pub fn csv_header() -> &'static [&'static str] {
        &[
            "tick",
            "in_game_day",
            "in_game_year",
            "day_of_year",
            "ambient_temp_c",
            "workers",
            "soldiers",
            "breeders",
            "queens_alive",
            "total_ant_entities",
            "eggs",
            "larvae",
            "pupae",
            "food_returned_cumulative",
            "food_stored",
            "food_inflow_recent",
            "starvation_deaths_cumulative",
        ]
    }

    /// Render as a CSV row. Caller is responsible for newline.
    /// Non-finite floats render as empty cells (CSV-canonical "missing"),
    /// never as `NaN` / `inf` strings — those would silently coerce in
    /// downstream tools (Excel, R) and break audit reproducibility.
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.tick,
            self.in_game_day,
            self.in_game_year,
            self.day_of_year,
            csv_f32(self.ambient_temp_c, 2),
            self.workers,
            self.soldiers,
            self.breeders,
            self.queens_alive,
            self.total_ant_entities,
            self.eggs,
            self.larvae,
            self.pupae,
            self.food_returned_cumulative,
            csv_f32(self.food_stored, 2),
            csv_f32(self.food_inflow_recent, 4),
            self.starvation_deaths_cumulative,
        )
    }
}

fn csv_f32(v: f32, precision: usize) -> String {
    if v.is_finite() {
        format!("{v:.*}", precision)
    } else {
        String::new()
    }
}

/// Health metric definition. Used by the report layer to render
/// per-species score breakdowns with human-readable explanations.
///
/// We carry the definition with the value so an ecologist reading
/// the report doesn't have to look anything up — the metric *explains
/// itself* in the output.
#[derive(Debug, Clone)]
pub struct MetricDefinition {
    /// Plain-English label. Example: "Colony survival".
    pub human_name: &'static str,
    /// One-paragraph definition. Example: "True if at least one queen
    /// and at least one worker are alive at the end of the run."
    pub human_definition: &'static str,
    /// Unit string. Example: "boolean", "workers", "food units/year".
    pub units: &'static str,
    /// How to read the value. Example: "1.0 = passed, 0.0 = failed".
    pub interpretation: &'static str,
}

/// Built-in metric catalogue. Each metric appears in the score report.
///
/// # Adding a metric
///
/// 1. Add a constant here with full human descriptors.
/// 2. Compute it in [`crate::bench::run`] and emit it in the score struct.
/// 3. Document the citation justifying its threshold in
///    [`crate::bench::expected`] under the relevant species.
pub const METRIC_COLONY_SURVIVAL: MetricDefinition = MetricDefinition {
    human_name: "Colony survival",
    human_definition:
        "True (1.0) if at least one queen ant is alive and the colony has at least one worker \
         at the end of the simulation run; false (0.0) otherwise. This is the most basic \
         viability check — a colony that fails this metric has gone extinct in-sim.",
    units: "boolean (0.0 or 1.0)",
    interpretation: "1.0 = colony survived to end of run; 0.0 = colony went extinct",
};

pub const METRIC_QUEEN_SURVIVAL: MetricDefinition = MetricDefinition {
    human_name: "Queen survival",
    human_definition:
        "True (1.0) if at least one queen is alive in colony 0 at the end of the run. For polygyne \
         species (Tapinoma, Formica) this only requires ≥1 queen — it does NOT track queen-count \
         trajectory, which would be a separate metric. A 0.0 result means the colony has no \
         reproductive female remaining (irrecoverable).",
    units: "boolean (0.0 or 1.0)",
    interpretation: "1.0 = queen line intact; 0.0 = colony irrecoverable. Note: this overlaps \
                     with `Colony survival` (which also requires queens_alive>0) — they are NOT \
                     fully independent.",
};

pub const METRIC_BROOD_PIPELINE_HEALTH: MetricDefinition = MetricDefinition {
    human_name: "Brood pipeline health",
    human_definition:
        "Fraction of sampled ticks in the final 25% of the run that had ALL THREE brood stages \
         (eggs, larvae, pupae) non-zero simultaneously. A healthy queen-led colony in non-diapause \
         conditions should have all three stages flowing. Persistent gaps indicate a stalled \
         pipeline — possible causes include food shortage or queen failure (the harness does NOT \
         determine which; it only measures the gap).",
    units: "fraction (0.0 to 1.0)",
    interpretation: "1.0 = always all 3 stages present; 0.0 = never. Diapause-inclusive runs may \
                     legitimately score 0.5-0.8 even when healthy.",
};

pub const METRIC_POPULATION_STABILITY: MetricDefinition = MetricDefinition {
    human_name: "Adult population stability",
    human_definition:
        "Coefficient of variation (standard deviation / mean) of the worker count across the \
         final 25% of the run. Lower is more stable. A value above 0.5 indicates the population \
         is oscillating wildly, which usually signals a foraging-vs-consumption mismatch or \
         broken hibernation logic.",
    units: "coefficient of variation (dimensionless, ≥0)",
    interpretation: "<0.3 = stable; 0.3-0.5 = noisy but healthy; >0.5 = unstable",
};

pub const METRIC_FOOD_ECONOMY: MetricDefinition = MetricDefinition {
    human_name: "Food economy ratio",
    human_definition:
        "Ratio of food units returned by foragers to food units consumed by adults in the final \
         year of the run. >1.0 means the colony brings in more than it consumes (sustainable). \
         <1.0 means the colony is eating into its reserves (will eventually starve). \
         Brood costs are NOT included in the denominator (they are built into egg_cost_food).",
    units: "ratio (dimensionless)",
    interpretation: "≥1.0 = sustainable; 0.5-1.0 = surviving on reserves; <0.5 = collapsing",
};

pub const METRIC_HIBERNATION_COMPLIANCE: MetricDefinition = MetricDefinition {
    human_name: "Hibernation compliance",
    human_definition:
        "For species with hibernation_required=true: fraction of in-game years where the colony \
         accumulated at least min_diapause_days of cold-period activity suppression. Skipped \
         hibernation in real biology causes queen fertility collapse — we mirror that.",
    units: "fraction of years (0.0 to 1.0)",
    interpretation: "1.0 = every year had adequate diapause; <1.0 = some years missed it. \
                     Always 1.0 for species without hibernation_required.",
};

/// All built-in metrics, in display order.
pub const ALL_METRICS: &[MetricDefinition] = &[
    METRIC_COLONY_SURVIVAL,
    METRIC_QUEEN_SURVIVAL,
    METRIC_BROOD_PIPELINE_HEALTH,
    METRIC_POPULATION_STABILITY,
    METRIC_FOOD_ECONOMY,
    METRIC_HIBERNATION_COMPLIANCE,
];

/// Score for one species after a bench run. Each field corresponds
/// to a `METRIC_*` constant above.
///
/// `Option<f64>` everywhere: a metric that cannot be computed (e.g.
/// food_economy in a zero-day run) returns `None`. The report layer
/// renders `None` as "n/a" rather than fabricating zeros.
#[derive(Debug, Clone)]
pub struct SpeciesScore {
    pub species_id: String,
    pub colony_survival: Option<f64>,
    pub queen_survival: Option<f64>,
    pub brood_pipeline_health: Option<f64>,
    pub population_stability: Option<f64>,
    pub food_economy: Option<f64>,
    pub hibernation_compliance: Option<f64>,
}

impl SpeciesScore {
    /// Composite score 0-100. A weighted average of available metrics
    /// (missing metrics excluded from both numerator and denominator).
    /// Weights reflect "how much does an ecologist care if this is wrong":
    /// - colony_survival: 30 (death = total failure)
    /// - queen_survival: 20
    /// - brood_pipeline_health: 15
    /// - population_stability: 10 (interpreted: 1.0 - clamp(cv, 0, 1.0))
    /// - food_economy: 15 (interpreted: clamp(ratio - 0.5, 0, 1.0) * 2)
    /// - hibernation_compliance: 10
    ///
    /// Returns `None` if zero metrics are computable.
    pub fn composite_0_to_100(&self) -> Option<f64> {
        let entries: &[(Option<f64>, f64, fn(f64) -> f64)] = &[
            (self.colony_survival, 30.0, identity),
            (self.queen_survival, 20.0, identity),
            (self.brood_pipeline_health, 15.0, identity),
            // Stability: a CV of 0.0 is perfect (1.0), CV of 1.0+ is bad (0.0).
            (self.population_stability, 10.0, stability_to_score),
            // Food economy: ratio 1.0 = good (1.0), 0.5 = barely (0.0).
            (self.food_economy, 15.0, food_economy_to_score),
            (self.hibernation_compliance, 10.0, identity),
        ];
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        for (val, weight, mapper) in entries {
            if let Some(v) = val {
                numerator += mapper(*v) * *weight;
                denominator += *weight;
            }
        }
        if denominator == 0.0 {
            None
        } else {
            Some((numerator / denominator) * 100.0)
        }
    }
}

// All score mappers must be NaN-safe — Rust's `clamp` returns NaN for NaN
// input, which then propagates into the composite. We force non-finite
// inputs to 0.0 explicitly before clamping.

fn identity(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    x.clamp(0.0, 1.0)
}

fn stability_to_score(cv: f64) -> f64 {
    if !cv.is_finite() {
        return 0.0;
    }
    (1.0 - cv).clamp(0.0, 1.0)
}

fn food_economy_to_score(ratio: f64) -> f64 {
    if !ratio.is_finite() {
        return 0.0;
    }
    // Ratio 0.5 → 0.0, ratio 1.0 → 1.0, ratio 1.5+ → 1.0.
    ((ratio - 0.5) * 2.0).clamp(0.0, 1.0)
}

/// Compute brood-pipeline-health from a slice of late-run samples.
/// Exposed so tests can verify the math.
pub fn compute_brood_pipeline_health(samples: &[TickSample]) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let count = samples
        .iter()
        .filter(|s| s.eggs > 0 && s.larvae > 0 && s.pupae > 0)
        .count();
    Some(count as f64 / samples.len() as f64)
}

/// Compute the coefficient of variation of worker counts.
/// Returns None if the mean is 0 (would be div-by-zero) or if fewer
/// than 2 samples (no meaningful std-dev).
pub fn compute_population_stability(samples: &[TickSample]) -> Option<f64> {
    if samples.len() < 2 {
        return None;
    }
    let n = samples.len() as f64;
    let mean = samples.iter().map(|s| s.workers as f64).sum::<f64>() / n;
    if mean == 0.0 {
        // Colony is dead — stability is undefined, NOT zero.
        return None;
    }
    let variance = samples
        .iter()
        .map(|s| {
            let d = s.workers as f64 - mean;
            d * d
        })
        .sum::<f64>() / n;
    let std_dev = variance.sqrt();
    let cv = std_dev / mean;
    if !cv.is_finite() {
        return None;
    }
    Some(cv)
}

/// Pull a `TickSample` out of a colony state plus environmental context.
/// Pure function — separated from the runner so it's testable in isolation.
pub fn snapshot_from_colony(
    tick: u64,
    in_game_day: u32,
    in_game_year: u32,
    day_of_year: u32,
    ambient_temp_c: f32,
    colony: &ColonyState,
    queens_alive: u32,
    total_ant_entities: u32,
    starvation_deaths_cumulative: u32,
) -> TickSample {
    TickSample {
        tick,
        in_game_day,
        in_game_year,
        day_of_year,
        ambient_temp_c,
        workers: colony.population.workers,
        soldiers: colony.population.soldiers,
        breeders: colony.population.breeders,
        queens_alive,
        total_ant_entities,
        eggs: colony.eggs,
        larvae: colony.larvae,
        pupae: colony.pupae,
        food_returned_cumulative: colony.food_returned,
        food_stored: colony.food_stored,
        food_inflow_recent: colony.food_inflow_recent,
        starvation_deaths_cumulative,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(workers: u32, eggs: u32, larvae: u32, pupae: u32) -> TickSample {
        TickSample {
            tick: 0,
            in_game_day: 0,
            in_game_year: 0,
            day_of_year: 0,
            ambient_temp_c: 20.0,
            workers,
            soldiers: 0,
            breeders: 0,
            queens_alive: 1,
            total_ant_entities: workers + 1,
            eggs,
            larvae,
            pupae,
            food_returned_cumulative: 0,
            food_stored: 0.0,
            food_inflow_recent: 0.0,
            starvation_deaths_cumulative: 0,
        }
    }

    #[test]
    fn brood_pipeline_health_all_full() {
        let samples = vec![sample(100, 5, 5, 5); 10];
        assert_eq!(compute_brood_pipeline_health(&samples), Some(1.0));
    }

    #[test]
    fn brood_pipeline_health_half_missing_one_stage() {
        let mut samples = vec![sample(100, 5, 5, 5); 5];
        samples.extend(vec![sample(100, 5, 0, 5); 5]); // larvae empty
        assert_eq!(compute_brood_pipeline_health(&samples), Some(0.5));
    }

    #[test]
    fn brood_pipeline_health_empty_returns_none() {
        let samples: Vec<TickSample> = vec![];
        assert_eq!(compute_brood_pipeline_health(&samples), None);
    }

    #[test]
    fn population_stability_constant_population_is_zero_cv() {
        let samples = vec![sample(100, 0, 0, 0); 10];
        assert_eq!(compute_population_stability(&samples), Some(0.0));
    }

    #[test]
    fn population_stability_dead_colony_is_none() {
        let samples = vec![sample(0, 0, 0, 0); 10];
        assert_eq!(compute_population_stability(&samples), None);
    }

    #[test]
    fn population_stability_single_sample_is_none() {
        let samples = vec![sample(100, 0, 0, 0)];
        assert_eq!(compute_population_stability(&samples), None);
    }

    #[test]
    fn population_stability_known_cv() {
        // 50, 100, 150 → mean=100, var=((50)^2+0+50^2)/3 ≈ 1666.67, std≈40.82, cv≈0.408
        let samples = vec![sample(50, 0, 0, 0), sample(100, 0, 0, 0), sample(150, 0, 0, 0)];
        let cv = compute_population_stability(&samples).unwrap();
        assert!((cv - 0.408).abs() < 0.01, "cv was {cv}");
    }

    #[test]
    fn composite_score_all_perfect_is_100() {
        let s = SpeciesScore {
            species_id: "test".into(),
            colony_survival: Some(1.0),
            queen_survival: Some(1.0),
            brood_pipeline_health: Some(1.0),
            population_stability: Some(0.0), // perfect stability
            food_economy: Some(1.0),         // exactly sustainable
            hibernation_compliance: Some(1.0),
        };
        assert!((s.composite_0_to_100().unwrap() - 100.0).abs() < 0.01);
    }

    #[test]
    fn composite_score_all_failed_is_zero() {
        let s = SpeciesScore {
            species_id: "test".into(),
            colony_survival: Some(0.0),
            queen_survival: Some(0.0),
            brood_pipeline_health: Some(0.0),
            population_stability: Some(2.0), // very unstable
            food_economy: Some(0.0),         // collapsing
            hibernation_compliance: Some(0.0),
        };
        assert_eq!(s.composite_0_to_100().unwrap(), 0.0);
    }

    #[test]
    fn composite_score_no_metrics_is_none() {
        let s = SpeciesScore {
            species_id: "test".into(),
            colony_survival: None,
            queen_survival: None,
            brood_pipeline_health: None,
            population_stability: None,
            food_economy: None,
            hibernation_compliance: None,
        };
        assert!(s.composite_0_to_100().is_none());
    }

    #[test]
    fn csv_header_count_matches_row_field_count() {
        let s = sample(10, 1, 2, 3);
        let row = s.to_csv_row();
        let comma_count = row.matches(',').count();
        assert_eq!(comma_count + 1, TickSample::csv_header().len());
    }

    #[test]
    fn all_metrics_have_human_descriptors() {
        for m in ALL_METRICS {
            assert!(!m.human_name.is_empty(), "metric missing human_name");
            assert!(
                !m.human_definition.is_empty(),
                "metric '{}' missing human_definition",
                m.human_name
            );
            assert!(
                !m.units.is_empty(),
                "metric '{}' missing units",
                m.human_name
            );
            assert!(
                !m.interpretation.is_empty(),
                "metric '{}' missing interpretation",
                m.human_name
            );
        }
    }
}

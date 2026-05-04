//! Species definition — the biology and encyclopedia data that drives how
//! a colony behaves. Authored in TOML, one file per species.
//!
//! The `Species` is **species-authentic biology** (14-day larval period,
//! 25-year queen lifespan, etc.) expressed in in-game seconds. At simulation
//! startup, `Species::apply(&environment)` folds those durations and the
//! player-chosen `TimeScale` into a `SimConfig` (tick-denominated), so the
//! sim itself doesn't need to care about real vs in-game time.

use serde::{Deserialize, Serialize};

use crate::colony::CasteRatio;
use crate::config::{
    AntConfig, ColonyConfig, CombatConfig, PheromoneConfig, SimConfig, WorldConfig,
};
use crate::environment::Environment;
use crate::error::SimError;
use crate::species_extended::{
    Behavior, ColonyStructure, CombatExtended, DietExtended, EcologicalRole, RecruitmentStyle,
    Substrate, default_schema_version,
};

/// Phase B hook #1 — recruitment-style → trail-deposit-strength scalar.
///
/// Mass recruiters lay strong trails that self-amplify into chemical
/// "roads"; tandem-running species drop little or no trail at all.
/// Multiplying the calibrated `PheromoneConfig.deposit_*` defaults by
/// this factor at config-bake time delivers the per-species behavior
/// without per-tick branching in the deposit loop.
///
/// Values are sim-pacing — no published "X% as much pheromone" figure
/// for any genus because tandem species typically don't deposit
/// detectable food trails at all (Hölldobler & Wilson 1990 ch. 7).
/// Phase B hook #3 — convert a species-specified trail half-life
/// (in in-game seconds) to the per-substep evaporation rate that
/// `pheromone::evaporate` reads. Calibration: each sim substep is
/// 2 in-game seconds (Seasonal baseline). The default
/// `PheromoneConfig.evaporation_rate = 0.02` corresponds to a
/// half-life of ~68.6s; species like Lasius niger have measured
/// half-lives of ~2820s (Beckers et al. 1992), which produces a
/// much smaller per-substep rate (~0.00049).
pub(crate) fn evaporation_rate_from_half_life_seconds(half_life_seconds: u32) -> f32 {
    const SUBSTEP_IN_GAME_SECONDS: f32 = 2.0;
    let h = half_life_seconds.max(1) as f32;
    let substeps_per_halflife = h / SUBSTEP_IN_GAME_SECONDS;
    1.0 - (0.5_f32).powf(1.0 / substeps_per_halflife.max(1.0))
}

pub(crate) fn recruitment_deposit_scalar(style: RecruitmentStyle) -> f32 {
    match style {
        RecruitmentStyle::Mass => 1.0,
        RecruitmentStyle::Group => 0.5,
        RecruitmentStyle::TandemRun => 0.1,
        RecruitmentStyle::Individual => 0.0,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FoundingType {
    /// Queen seals herself in; first workers hatch from her stored reserves.
    Claustral,
    /// Queen forages during founding.
    SemiClaustral,
    /// Queen takes over a host colony.
    Parasitic,
    /// Multiple cooperating queens.
    Polygyne,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Biology {
    pub queen_lifespan_years: f32,
    pub worker_lifespan_months: f32,
    pub founding: FoundingType,
    pub polymorphic: bool,
    /// Does the queen REQUIRE a winter diapause to lay viable eggs?
    #[serde(default)]
    pub hibernation_required: bool,
    /// Minimum in-game days of diapause required per year for the
    /// queen's fertility to remain viable. Only consulted when
    /// `hibernation_required = true`. Defaults to 60 if absent — that
    /// covers Lasius and most temperate beginners. Cold-temperate
    /// species (Camponotus, Formica) want higher (~90-120); short-cycle
    /// mediterranean species (Aphaenogaster) can run lower.
    #[serde(default = "default_min_diapause_days")]
    pub min_diapause_days: u32,
}

fn default_min_diapause_days() -> u32 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Growth {
    pub egg_maturation_seconds: u64,
    pub larva_maturation_seconds: u64,
    pub pupa_maturation_seconds: u64,
    pub queen_eggs_per_day: f32,
    pub initial_workers: u32,
    pub target_population: u32,
    pub egg_cost_food: f32,
    /// Food consumed per adult per in-game day (divided across ticks).
    #[serde(default = "default_food_per_day")]
    pub food_per_adult_per_day: f32,
}

fn default_food_per_day() -> f32 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diet {
    pub prefers: Vec<String>,
    pub forages_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatProfile {
    pub worker_attack: f32,
    pub soldier_attack: f32,
    pub worker_health: f32,
    pub soldier_health: f32,
    /// 0..1 — chase radius multiplier for main-game AI. Ignored in keeper mode.
    pub aggression: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appearance {
    pub color_hex: String,
    pub size_mm: f32,
    /// Speed multiplier applied to the base `AntConfig` worker speed.
    #[serde(default = "default_speed_mult")]
    pub speed_multiplier: f32,
}

fn default_speed_mult() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Encyclopedia {
    pub tagline: String,
    pub description: String,
    pub real_world_range: String,
    #[serde(default)]
    pub fun_facts: Vec<String>,
    #[serde(default)]
    pub keeper_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Species {
    pub id: String,
    pub common_name: String,
    pub genus: String,
    pub species_epithet: String,
    pub difficulty: Difficulty,
    pub biology: Biology,
    pub growth: Growth,
    pub diet: Diet,
    pub combat: CombatProfile,
    pub appearance: Appearance,
    pub encyclopedia: Encyclopedia,
    /// Default caste distribution new eggs are drawn from.
    #[serde(default = "default_caste_ratio")]
    pub default_caste_ratio: CasteRatio,

    // ---- Phase A schema extensions (additive, all optional) ----
    // See `species_extended.rs` for full semantics. Existing TOMLs that
    // omit these sections continue to load with defaults.

    /// Schema version of the extended sections below. Defaults to 1.
    /// Bumped only on breaking changes to the extended schema.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Optional behavior section (recruitment style, diel activity,
    /// trail half-life override). Defaults to mass-recruiting diurnal.
    #[serde(default)]
    pub behavior: Behavior,

    /// Optional colony structure (queen count, polydomy, supercolony).
    /// Defaults to monogyne, single-nest, non-supercolony.
    #[serde(default)]
    pub colony_structure: ColonyStructure,

    /// Optional substrate preferences + dig-rate scaling + mound type.
    /// Defaults to no substrate constraint, baseline dig speed, kickout mound.
    #[serde(default)]
    pub substrate: Substrate,

    /// Optional weaponry + polymorphism size buckets + sting potency.
    /// Defaults to mandible-only, monomorphic, no sting.
    #[serde(default)]
    pub combat_extended: CombatExtended,

    /// Optional dietary specializations (myrmecochory, honeydew dependence,
    /// host species for parasitic founding).
    #[serde(default)]
    pub diet_extended: DietExtended,

    /// Optional ecological role (keystone status, invasive status,
    /// inter-species displacement relations).
    #[serde(default)]
    pub ecological_role: EcologicalRole,
}

fn default_caste_ratio() -> CasteRatio {
    CasteRatio {
        worker: 0.8,
        soldier: 0.15,
        breeder: 0.05,
    }
}

impl Species {
    pub fn load_from_str(toml_str: &str) -> Result<Self, SimError> {
        let s: Self = toml::from_str(toml_str).map_err(SimError::from)?;
        Ok(s)
    }

    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, SimError> {
        let contents = std::fs::read_to_string(path)?;
        Self::load_from_str(&contents)
    }

    pub fn scientific_name(&self) -> String {
        format!("{} {}", self.genus, self.species_epithet)
    }

    /// Convert species biology + player environment into a tick-denominated `SimConfig`.
    ///
    /// This is where in-game seconds meet sim ticks. All downstream sim code
    /// operates in ticks; nothing inside the sim loop needs to know about
    /// TimeScale or real-time seconds.
    pub fn apply(&self, env: &Environment) -> SimConfig {
        let world = WorldConfig {
            width: env.world_width,
            height: env.world_height,
            food_spawn_rate: 0.0,
            food_cluster_size: 5,
        };

        // Pheromone defaults stay calibrated to a single sim-substep
        // (= 2 in-game seconds). The substep loop in Simulation::tick()
        // runs N substeps per outer tick at higher time scales, so the
        // per-substep rates here remain biologically correct at any
        // player-selected scale.
        //
        // Phase B hook #1 — recruitment style scales trail deposit
        // strength. Mass recruiters (Lasius, Tetramorium, Tapinoma)
        // get the calibrated baseline; tandem-runners (Camponotus) and
        // individual foragers deposit much less. This is what makes
        // trails self-amplify into "roads" for mass recruiters and
        // stay faint individual breadcrumbs for tandem species.
        // Cross-ref: docs/biology.md "Trail pheromone half-life and
        // species variance"; biology-roadmap.md §"Phase B sim hooks" #1.
        let recruitment_scalar = recruitment_deposit_scalar(self.behavior.recruitment);
        let mut pheromone = PheromoneConfig::default();
        pheromone.deposit_food_trail *= recruitment_scalar;
        pheromone.deposit_home_trail *= recruitment_scalar;

        // Phase B hook #3 — per-species trail evaporation override.
        // When a species TOML supplies `behavior.trail_half_life_seconds`,
        // convert that to per-substep evaporation rate; otherwise leave
        // the default. Matches measured species variance (Lasius ~2820s
        // per Beckers/Deneubourg vs. ~60s for tandem-recruiters).
        if let Some(half_life) = self.behavior.trail_half_life_seconds {
            pheromone.evaporation_rate = evaporation_rate_from_half_life_seconds(half_life);
        }

        let ant = AntConfig {
            speed_worker: 2.0 * self.appearance.speed_multiplier,
            speed_soldier: 1.5 * self.appearance.speed_multiplier,
            speed_queen: 0.0,
            sense_radius: 5,
            sense_angle: 60.0,
            exploration_rate: 0.15,
            alpha: 1.0,
            beta: 2.0,
            food_capacity: 1.0,
            initial_count: self.growth.initial_workers as usize,
            worker_size_mm: self.appearance.size_mm,
            polymorphic: self.biology.polymorphic,
            hibernation_cold_threshold_c: 10.0,
            hibernation_warm_threshold_c: 12.0,
            hibernation_required: self.biology.hibernation_required,
            min_diapause_days: self.biology.min_diapause_days,
            nocturnal: matches!(
                self.behavior.diel_activity,
                crate::species_extended::DielActivity::Nocturnal
            ),
            sting_potency: self.combat_extended.sting_potency,
            // Phase B hook #7 — species-specific dig rate. Camponotus
            // (0.5) digs slower in any substrate; Pogonomyrmex (1.3)
            // digs faster. Compounds with the per-tile substrate rate
            // already applied in `dig_tick`.
            species_dig_multiplier: self.substrate.dig_speed_multiplier,
        };

        // queen_egg_rate is fraction-of-egg-per-tick.
        // eggs_per_day / in_game_seconds_per_day / in_game_seconds_per_tick
        // Simplified: rate_per_tick = eggs_per_day / ticks_per_in_game_day.
        let ticks_per_day = env.in_game_seconds_to_ticks(86_400).max(1) as f32;
        // Phase B hook #12 — honeydew_dependent species without nearby
        // aphid sources grow more slowly. Aphid-colony entities are a
        // Phase C feature; until then, the simplest defensible
        // interpretation is a flat 20% growth penalty for honeydew-
        // obligates (Formica rufa documented as ~200kg honeydew/yr at
        // mature mound from Cinara/Lachnus aphid herds — losing that
        // resource collapses the carb supply driving brood production).
        // Cross-ref: docs/biology-roadmap.md §"Phase B sim hooks" #12.
        let honeydew_penalty: f32 = if self.diet_extended.honeydew_dependent { 0.8 } else { 1.0 };
        let queen_egg_rate = self.growth.queen_eggs_per_day * honeydew_penalty / ticks_per_day;
        let adult_food_consumption =
            self.growth.food_per_adult_per_day / ticks_per_day;

        // Maturation: each stage runs from egg→larva→pupa→adult.
        let egg_ticks = env.in_game_seconds_to_ticks(self.growth.egg_maturation_seconds);
        let larva_ticks = env.in_game_seconds_to_ticks(self.growth.larva_maturation_seconds);
        let pupa_ticks = env.in_game_seconds_to_ticks(self.growth.pupa_maturation_seconds);

        let colony = ColonyConfig {
            initial_workers: self.growth.initial_workers,
            initial_food: 200.0,
            egg_cost: self.growth.egg_cost_food,
            // Three independent stage durations, mapped 1:1 from the
            // species TOML's egg/larva/pupa_maturation_seconds. Pre-fix
            // these were folded into two fields and the pupa stage
            // reused the larva duration, compressing the egg→adult
            // pipeline by ~30%.
            egg_stage_ticks: egg_ticks as u32,
            larva_stage_ticks: larva_ticks as u32,
            pupa_stage_ticks: pupa_ticks as u32,
            adult_food_consumption,
            soldier_food_multiplier: 1.5,
            queen_egg_rate,
            target_population: self.growth.target_population,
            ..ColonyConfig::default()
        };

        let combat = CombatConfig {
            worker_attack: self.combat.worker_attack,
            soldier_attack: self.combat.soldier_attack,
            worker_health: self.combat.worker_health,
            soldier_health: self.combat.soldier_health,
            ..CombatConfig::default()
        };

        let cfg = SimConfig {
            world,
            pheromone,
            ant,
            colony,
            combat,
            hazards: crate::config::HazardConfig::default(),
        };

        tracing::info!(
            species = %self.id,
            scale = env.time_scale.label(),
            ticks_per_day,
            queen_egg_rate,
            adult_food_consumption,
            egg_ticks = cfg.colony.egg_stage_ticks,
            larva_ticks = cfg.colony.larva_stage_ticks,
            pupa_ticks = cfg.colony.pupa_stage_ticks,
            "Species::apply folded biology into SimConfig"
        );

        cfg
    }
}

/// Load every `*.toml` under a directory as a species. Sorted by `id` for stable ordering.
pub fn load_species_dir<P: AsRef<std::path::Path>>(
    dir: P,
) -> Result<Vec<Species>, SimError> {
    let dir = dir.as_ref();
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        match Species::load_from_file(&path) {
            Ok(s) => {
                tracing::info!(path = %path.display(), id = %s.id, "loaded species");
                out.push(s);
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "skipped invalid species file");
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_toml() -> &'static str {
        r##"
id = "lasius_niger"
common_name = "Black Garden Ant"
genus = "Lasius"
species_epithet = "niger"
difficulty = "beginner"

[biology]
queen_lifespan_years = 29.0
worker_lifespan_months = 3.0
founding = "claustral"
polymorphic = false
hibernation_required = true

[growth]
egg_maturation_seconds = 1209600
larva_maturation_seconds = 1814400
pupa_maturation_seconds = 1209600
queen_eggs_per_day = 30.0
initial_workers = 20
target_population = 15000
egg_cost_food = 5.0
food_per_adult_per_day = 0.4

[diet]
prefers = ["sugar", "protein"]
forages_on = ["ground"]

[combat]
worker_attack = 1.0
soldier_attack = 2.0
worker_health = 8.0
soldier_health = 12.0
aggression = 0.2

[appearance]
color_hex = "#1a1a1a"
size_mm = 4.0
speed_multiplier = 1.0

[encyclopedia]
tagline = "The perfect beginner ant."
description = "Widespread across Eurasia..."
real_world_range = "Palearctic"
fun_facts = ["Queens can live nearly 30 years.", "Famous for nuptial flights on warm summer evenings."]
keeper_notes = "Docile, hardy, forgiving."
"##
    }

    #[test]
    fn evaporation_rate_scales_inversely_with_half_life() {
        let baseline = evaporation_rate_from_half_life_seconds(60); // ~similar to default
        let lasius = evaporation_rate_from_half_life_seconds(2820); // Beckers 1992
        let very_short = evaporation_rate_from_half_life_seconds(10);
        assert!(very_short > baseline, "shorter half-life ⇒ higher decay rate");
        assert!(baseline > lasius, "Lasius long half-life ⇒ low decay rate");
        // Half-life formula sanity: ~0.5 of pheromone left after `H/2`
        // substeps when rate matches.
        let r = evaporation_rate_from_half_life_seconds(20); // 10 substeps to half-life
        let after = (1.0_f32 - r).powi(10);
        assert!((after - 0.5).abs() < 0.01, "after H/SUBSTEP substeps should be ~0.5, got {after}");
    }

    #[test]
    fn apply_overrides_evaporation_when_half_life_set() {
        use crate::environment::Environment;
        use crate::species_extended::Behavior;
        let mut s = Species::load_from_str(sample_toml()).expect("parse");
        let baseline = s.apply(&Environment::default()).pheromone.evaporation_rate;

        s.behavior = Behavior {
            trail_half_life_seconds: Some(2820),
            ..s.behavior.clone()
        };
        let with_override = s.apply(&Environment::default()).pheromone.evaporation_rate;
        assert!(
            with_override < baseline,
            "Lasius half-life override should LOWER evaporation rate vs default ({with_override} vs {baseline})",
        );
    }

    #[test]
    fn recruitment_scalar_orders_correctly() {
        use crate::species_extended::RecruitmentStyle as RS;
        // Mass > Group > TandemRun > Individual.
        assert!(recruitment_deposit_scalar(RS::Mass) > recruitment_deposit_scalar(RS::Group));
        assert!(recruitment_deposit_scalar(RS::Group) > recruitment_deposit_scalar(RS::TandemRun));
        assert!(recruitment_deposit_scalar(RS::TandemRun) > recruitment_deposit_scalar(RS::Individual));
        // Individual is no-trail.
        assert_eq!(recruitment_deposit_scalar(RS::Individual), 0.0);
    }

    #[test]
    fn apply_scales_deposit_for_tandem_recruiter() {
        use crate::environment::Environment;
        use crate::species_extended::{Behavior, RecruitmentStyle};
        // Build two species identical except for recruitment style.
        let mut mass = Species::load_from_str(sample_toml()).expect("parse");
        mass.behavior = Behavior {
            recruitment: RecruitmentStyle::Mass,
            ..Behavior::default()
        };
        let mut tandem = mass.clone();
        tandem.behavior.recruitment = RecruitmentStyle::TandemRun;

        let env = Environment::default();
        let cfg_mass = mass.apply(&env);
        let cfg_tandem = tandem.apply(&env);

        assert!(
            cfg_mass.pheromone.deposit_food_trail > cfg_tandem.pheromone.deposit_food_trail,
            "mass recruiter must deposit more trail than tandem ({} vs {})",
            cfg_mass.pheromone.deposit_food_trail,
            cfg_tandem.pheromone.deposit_food_trail,
        );
        assert!(
            cfg_mass.pheromone.deposit_home_trail > cfg_tandem.pheromone.deposit_home_trail,
        );
    }

    #[test]
    fn loads_sample_species() {
        let s = Species::load_from_str(sample_toml()).expect("parse");
        assert_eq!(s.id, "lasius_niger");
        assert_eq!(s.scientific_name(), "Lasius niger");
        assert!(s.biology.hibernation_required);
        assert_eq!(s.difficulty, Difficulty::Beginner);
    }

    #[test]
    fn apply_produces_positive_durations_at_timelapse() {
        use crate::environment::{Environment, TimeScale};
        let s = Species::load_from_str(sample_toml()).expect("parse");
        let env = Environment {
            time_scale: TimeScale::Timelapse,
            tick_rate_hz: 30.0,
            ..Environment::default()
        };
        let cfg = s.apply(&env);
        assert!(cfg.colony.egg_stage_ticks > 0);
        assert!(cfg.colony.larva_stage_ticks > 0);
        assert!(cfg.colony.pupa_stage_ticks > 0);
        assert!(cfg.colony.queen_egg_rate > 0.0);
        assert!(cfg.colony.adult_food_consumption > 0.0);
    }

    #[test]
    fn shipped_species_dir_loads_seven_valid_species() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("assets")
            .join("species");
        let species = load_species_dir(&dir)
            .unwrap_or_else(|e| panic!("load_species_dir failed: {e}"));
        assert_eq!(
            species.len(),
            7,
            "expected exactly 7 shipped species, got {}: {:?}",
            species.len(),
            species.iter().map(|s| &s.id).collect::<Vec<_>>()
        );
        for s in &species {
            assert!(
                !s.encyclopedia.tagline.trim().is_empty(),
                "species {} has empty tagline",
                s.id
            );
            assert!(
                !s.encyclopedia.description.trim().is_empty(),
                "species {} has empty description",
                s.id
            );
        }
    }

    #[test]
    fn realtime_stage_periods_map_to_separate_durations() {
        // Sample TOML has egg=14d, larva=21d, pupa=14d. Each stage maps
        // to its own ticks field at realtime/30Hz. Pre-fix this test
        // checked the combined 35d sum because egg + larva were folded
        // into a single field; now each stage is independent.
        use crate::environment::{Environment, TimeScale};
        let s = Species::load_from_str(sample_toml()).expect("parse");
        let env = Environment {
            time_scale: TimeScale::Realtime,
            tick_rate_hz: 30.0,
            ..Environment::default()
        };
        let cfg = s.apply(&env);
        // 14d * 86400s * 30Hz = 36_288_000 ticks
        assert_eq!(cfg.colony.egg_stage_ticks, 36_288_000);
        // 21d * 86400s * 30Hz = 54_432_000 ticks
        assert_eq!(cfg.colony.larva_stage_ticks, 54_432_000);
        // 14d * 86400s * 30Hz = 36_288_000 ticks
        assert_eq!(cfg.colony.pupa_stage_ticks, 36_288_000);
    }
}

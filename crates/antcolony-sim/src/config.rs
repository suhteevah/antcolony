//! All tunable simulation parameters. Loaded from TOML, with sane defaults
//! so tests can run without a config file.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SimConfig {
    pub world: WorldConfig,
    pub pheromone: PheromoneConfig,
    pub ant: AntConfig,
    pub colony: ColonyConfig,
    pub combat: CombatConfig,
    #[serde(default)]
    pub hazards: HazardConfig,
}

/// Phase 6: environmental hazards — predator and weather-event tuning.
/// Defaults are relatively gentle (no auto-spawns unless the sim
/// explicitly asks) so pre-P6 tests and Keeper-mode sessions stay
/// unchanged until hazards are opted into.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HazardConfig {
    /// Spider maximum speed (world cells per tick). Defaults faster than
    /// a worker ant so the chase feels tense.
    pub spider_speed: f32,
    /// Damage dealt by a spider per tick while adjacent to its target.
    pub spider_attack: f32,
    /// Health of a newly-spawned spider.
    pub spider_health: f32,
    /// Sensing radius (cells) used by spiders to find a target ant.
    pub spider_sense_radius: f32,
    /// Ticks a spider spends in the `Eat` state before resuming patrol.
    pub spider_eat_ticks: u32,
    /// Ticks until a dead spider respawns (0 = never).
    pub spider_respawn_ticks: u32,
    /// Units of terrain food left by a dead spider (larger than an ant
    /// corpse).
    pub spider_corpse_food_units: u32,
    /// Ticks between rain events (0 = never rains).
    pub rain_period_ticks: u64,
    /// Duration of a single rain event (during which pheromones are
    /// continually wiped and underground floor cells flood).
    pub rain_duration_ticks: u32,
    /// Extra damage per tick dealt to ants standing in a flooded
    /// (bottom-row) underground cell during rain.
    pub rain_flood_damage: f32,
    /// Ticks between lawnmower events (0 = never).
    pub lawnmower_period_ticks: u64,
    /// Warning period before a sweep starts, in ticks.
    pub lawnmower_warning_ticks: u32,
    /// Sweep travel speed — cells per tick the blade advances south→north.
    pub lawnmower_speed: f32,
    /// Blade half-width (cells) — any surface ant whose |y - blade_y| <=
    /// half-width at the sweep time dies.
    pub lawnmower_half_width: f32,
}

impl Default for HazardConfig {
    fn default() -> Self {
        Self {
            spider_speed: 3.0,
            spider_attack: 4.0,
            spider_health: 40.0,
            spider_sense_radius: 8.0,
            spider_eat_ticks: 60,
            spider_respawn_ticks: 600,
            spider_corpse_food_units: 6,
            // Rain + mower are OPT-IN: 0 here means never fire. Callers
            // building a hazard-enabled sim must set these explicitly.
            rain_period_ticks: 0,
            rain_duration_ticks: 120,
            rain_flood_damage: 0.5,
            lawnmower_period_ticks: 0,
            lawnmower_warning_ticks: 60,
            lawnmower_speed: 1.0,
            lawnmower_half_width: 1.2,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WorldConfig {
    pub width: usize,
    pub height: usize,
    pub food_spawn_rate: f32,
    pub food_cluster_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PheromoneConfig {
    pub evaporation_rate: f32,
    pub diffusion_rate: f32,
    pub diffusion_interval: u32,
    pub max_intensity: f32,
    pub min_threshold: f32,
    pub deposit_food_trail: f32,
    pub deposit_home_trail: f32,
    pub deposit_alarm: f32,
    /// Per-tick fraction by which the two port-cells of a tube
    /// equilibrate their pheromone intensities. Models trail propagation
    /// across tube boundaries without simulating tube interior. Scales
    /// with the time-scale multiplier so cross-module signal speed stays
    /// constant in in-game-time terms. Capped at 0.95.
    #[serde(default = "default_port_bleed_rate")]
    pub port_bleed_rate: f32,
}

fn default_port_bleed_rate() -> f32 {
    0.35
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AntConfig {
    pub speed_worker: f32,
    pub speed_soldier: f32,
    pub speed_queen: f32,
    pub sense_radius: u32,
    pub sense_angle: f32,
    pub exploration_rate: f32,
    pub alpha: f32,
    pub beta: f32,
    pub food_capacity: f32,
    pub initial_count: usize,
    /// Worker/base body length in mm. Used for tube-bore gating (K2.2).
    #[serde(default = "default_worker_size_mm")]
    pub worker_size_mm: f32,
    /// Species has a major caste (soldiers much bigger than workers).
    #[serde(default)]
    pub polymorphic: bool,
    /// K3: local temp below this (°C) → ant enters Diapause.
    #[serde(default = "default_cold_threshold")]
    pub hibernation_cold_threshold_c: f32,
    /// K3: local temp above this (°C) → a Diapause ant wakes up.
    #[serde(default = "default_warm_threshold")]
    pub hibernation_warm_threshold_c: f32,
    /// K3: species requires a real winter diapause for queen fertility.
    #[serde(default)]
    pub hibernation_required: bool,
    /// K3: minimum in-game days of diapause required per year before
    /// the year-rollover fertility check is satisfied. Only consulted
    /// when `hibernation_required = true`. Default 60 (matches Lasius
    /// niger and most temperate beginners); cold-temperate species
    /// (Camponotus, Formica) want higher.
    #[serde(default = "default_min_diapause_days")]
    pub min_diapause_days: u32,

    /// Phase B hook #2 — diel activity. When true, foraging-state
    /// transitions (Idle/Exploring/FollowingTrail) are suppressed
    /// during in-game daylight (06:00-18:00 sim time). Workers stay
    /// in the nest by day and emerge at night. Defaults to false
    /// (diurnal — original behavior). Set by `species::apply` from
    /// `Species.behavior.diel_activity == Nocturnal`.
    /// Cross-ref: docs/biology-roadmap.md §"Phase B sim hooks" #2.
    #[serde(default)]
    pub nocturnal: bool,

    /// Phase B hook #4 — sting potency on the Schmidt scale (0..5).
    /// 0 = no sting (Formicinae). Higher values mean each successful
    /// predator bite costs the predator more health, so stinging
    /// species are harder to consume. Set by `species::apply` from
    /// `Species.combat_extended.sting_potency`.
    /// Cross-ref: docs/biology-roadmap.md §"Phase B sim hooks" #4.
    #[serde(default)]
    pub sting_potency: f32,
}

fn default_min_diapause_days() -> u32 {
    60
}

fn default_target_population() -> u32 {
    5_000
}

fn default_worker_size_mm() -> f32 {
    4.0
}

fn default_cold_threshold() -> f32 {
    10.0
}

fn default_warm_threshold() -> f32 {
    12.0
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ColonyConfig {
    pub initial_workers: u32,
    pub initial_food: f32,
    pub egg_cost: f32,
    /// Duration of the egg stage in ticks (egg → larva).
    /// Previously misnamed `larva_maturation_ticks`.
    #[serde(alias = "larva_maturation_ticks")]
    pub egg_stage_ticks: u32,
    /// Duration of the larva stage in ticks (larva → pupa).
    /// Previously misnamed `pupa_maturation_ticks` — and that same value
    /// was incorrectly reused as the pupa-stage duration too, compressing
    /// the egg→adult pipeline by ~30%. See `pupa_stage_ticks` below.
    #[serde(alias = "pupa_maturation_ticks")]
    pub larva_stage_ticks: u32,
    /// Duration of the pupa stage in ticks (pupa → adult). Pre-fix this
    /// was conflated with `larva_stage_ticks`; species TOMLs already had
    /// a separate `pupa_maturation_seconds` field that was being ignored
    /// because the code reused the larva field.
    pub pupa_stage_ticks: u32,
    pub adult_food_consumption: f32,
    pub soldier_food_multiplier: f32,
    pub queen_egg_rate: f32,
    /// Species-level soft population ceiling (matches biology.target_population).
    /// Queen lay rate ramps down as colony adult_total approaches this number,
    /// reaching zero around 1.5× target. Models real biology where established
    /// queens slow egg production once the colony has saturated its niche
    /// (Hölldobler & Wilson 1990 — colony-size-dependent fertility regulation).
    /// Without this cap, the food-inflow throttle floor (0.2) keeps the queen
    /// laying past sustainable food balance, the colony overshoots its
    /// foraging support, then dies the first winter.
    #[serde(default = "default_target_population")]
    pub target_population: u32,
    /// K5 nuptial flight: breeders needed before a launch is triggered.
    pub nuptial_breeder_min: u32,
    /// Breeder must be at least this many ticks old to participate.
    pub nuptial_breeder_min_age: u32,
    /// Flight duration in ticks (time each breeder spends airborne).
    pub nuptial_flight_ticks: u32,
    /// Per-tick probability that a flying breeder gets picked off by a bird.
    pub nuptial_predation_per_tick: f32,
    /// Chance that a surviving breeder successfully founds a daughter colony.
    pub nuptial_founding_chance: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CombatConfig {
    pub worker_attack: f32,
    pub soldier_attack: f32,
    pub worker_health: f32,
    pub soldier_health: f32,
    /// P4: grid-distance (cells) at which two cross-colony ants start
    /// dealing damage to each other each tick.
    pub interaction_radius: f32,
    /// P4: multiplier on soldier attack when the target is a worker.
    pub soldier_vs_worker_bonus: f32,
    /// P4: units of terrain food left when an ant dies. Applied at the
    /// grid cell where the ant was standing, iff that cell is Empty.
    pub corpse_food_units: u32,
    /// P4: alarm pheromone amount deposited at a death site.
    pub alarm_deposit_on_death: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            world: WorldConfig::default(),
            pheromone: PheromoneConfig::default(),
            ant: AntConfig::default(),
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        }
    }
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            width: 256,
            height: 256,
            food_spawn_rate: 0.0,
            food_cluster_size: 5,
        }
    }
}

impl Default for PheromoneConfig {
    fn default() -> Self {
        Self {
            evaporation_rate: 0.02,
            diffusion_rate: 0.1,
            diffusion_interval: 4,
            max_intensity: 10.0,
            min_threshold: 0.001,
            deposit_food_trail: 1.0,
            deposit_home_trail: 0.8,
            deposit_alarm: 2.0,
            port_bleed_rate: 0.35,
        }
    }
}

impl Default for AntConfig {
    fn default() -> Self {
        Self {
            speed_worker: 2.0,
            speed_soldier: 1.5,
            speed_queen: 0.0,
            sense_radius: 5,
            sense_angle: 60.0,
            exploration_rate: 0.15,
            alpha: 1.0,
            beta: 2.0,
            food_capacity: 1.0,
            initial_count: 20,
            worker_size_mm: 4.0,
            polymorphic: false,
            hibernation_cold_threshold_c: 10.0,
            hibernation_warm_threshold_c: 12.0,
            hibernation_required: false,
            min_diapause_days: 60,
            nocturnal: false,
            sting_potency: 0.0,
        }
    }
}

impl Default for ColonyConfig {
    fn default() -> Self {
        Self {
            initial_workers: 20,
            initial_food: 100.0,
            egg_cost: 5.0,
            egg_stage_ticks: 300,
            larva_stage_ticks: 300,
            pupa_stage_ticks: 200,
            adult_food_consumption: 0.01,
            soldier_food_multiplier: 1.5,
            queen_egg_rate: 0.05,
            target_population: 5_000,
            nuptial_breeder_min: 3,
            nuptial_breeder_min_age: 600,
            nuptial_flight_ticks: 180,
            nuptial_predation_per_tick: 0.02,
            nuptial_founding_chance: 0.5,
        }
    }
}

impl Default for CombatConfig {
    fn default() -> Self {
        Self {
            worker_attack: 1.0,
            soldier_attack: 3.0,
            worker_health: 10.0,
            soldier_health: 25.0,
            interaction_radius: 1.2,
            soldier_vs_worker_bonus: 3.0,
            corpse_food_units: 1,
            alarm_deposit_on_death: 2.0,
        }
    }
}

impl SimConfig {
    pub fn load_from_str(toml_str: &str) -> Result<Self, crate::SimError> {
        let cfg: Self = toml::from_str(toml_str)?;
        Ok(cfg)
    }

    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, crate::SimError> {
        let contents = std::fs::read_to_string(path)?;
        Self::load_from_str(&contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_populated() {
        let cfg = SimConfig::default();
        assert_eq!(cfg.world.width, 256);
        assert!((cfg.pheromone.evaporation_rate - 0.02).abs() < 1e-6);
        assert_eq!(cfg.ant.sense_radius, 5);
    }

    #[test]
    fn test_config_loads() {
        let toml = r#"
[world]
width = 512
height = 512
food_spawn_rate = 0.1
food_cluster_size = 5

[pheromone]
evaporation_rate = 0.05
diffusion_rate = 0.2
diffusion_interval = 4
max_intensity = 8.0
min_threshold = 0.001
deposit_food_trail = 1.5
deposit_home_trail = 1.2
deposit_alarm = 3.0

[ant]
speed_worker = 2.5
speed_soldier = 2.0
speed_queen = 0.0
sense_radius = 6
sense_angle = 70.0
exploration_rate = 0.2
alpha = 1.0
beta = 2.0
food_capacity = 1.0
initial_count = 30

[colony]
initial_workers = 25
initial_food = 150.0
egg_cost = 5.0
egg_stage_ticks = 300
larva_stage_ticks = 300
pupa_stage_ticks = 200
adult_food_consumption = 0.01
soldier_food_multiplier = 1.5
queen_egg_rate = 0.05

[combat]
worker_attack = 1.0
soldier_attack = 3.0
worker_health = 10.0
soldier_health = 25.0
"#;
        let cfg = SimConfig::load_from_str(toml).expect("parse");
        assert_eq!(cfg.world.width, 512);
        assert_eq!(cfg.ant.initial_count, 30);
        assert!((cfg.pheromone.evaporation_rate - 0.05).abs() < 1e-6);
    }

    #[test]
    fn partial_config_uses_defaults() {
        let toml = r#"
[world]
width = 100
height = 100
"#;
        let cfg = SimConfig::load_from_str(toml).expect("parse");
        assert_eq!(cfg.world.width, 100);
        assert_eq!(cfg.ant.initial_count, 20);
    }
}

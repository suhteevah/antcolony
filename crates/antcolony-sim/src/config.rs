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
    pub larva_maturation_ticks: u32,
    pub pupa_maturation_ticks: u32,
    pub adult_food_consumption: f32,
    pub soldier_food_multiplier: f32,
    pub queen_egg_rate: f32,
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
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            world: WorldConfig::default(),
            pheromone: PheromoneConfig::default(),
            ant: AntConfig::default(),
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
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
        }
    }
}

impl Default for ColonyConfig {
    fn default() -> Self {
        Self {
            initial_workers: 20,
            initial_food: 100.0,
            egg_cost: 5.0,
            larva_maturation_ticks: 300,
            pupa_maturation_ticks: 200,
            adult_food_consumption: 0.01,
            soldier_food_multiplier: 1.5,
            queen_egg_rate: 0.05,
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
larva_maturation_ticks = 300
pupa_maturation_ticks = 200
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

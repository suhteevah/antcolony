//! Colony-level state: economy, brood pipeline, caste/behavior allocation.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::ant::AntCaste;
use crate::milestones::Milestone;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BroodStage {
    Egg,
    Larva,
    Pupa,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Brood {
    pub stage: BroodStage,
    pub caste: AntCaste,
    /// Ticks spent in the current stage.
    pub age: u32,
}

impl Brood {
    pub fn new_egg(caste: AntCaste) -> Self {
        Self {
            stage: BroodStage::Egg,
            caste,
            age: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CasteRatio {
    pub worker: f32,
    pub soldier: f32,
    pub breeder: f32,
}

impl Default for CasteRatio {
    fn default() -> Self {
        Self {
            worker: 0.8,
            soldier: 0.15,
            breeder: 0.05,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BehaviorWeights {
    pub forage: f32,
    pub dig: f32,
    pub nurse: f32,
}

impl Default for BehaviorWeights {
    fn default() -> Self {
        Self {
            forage: 0.7,
            dig: 0.1,
            nurse: 0.2,
        }
    }
}

#[derive(Debug, Clone, Default, Copy, Serialize, Deserialize)]
pub struct PopulationCounts {
    pub workers: u32,
    pub soldiers: u32,
    pub breeders: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonyState {
    pub id: u8,
    pub food_stored: f32,
    pub food_returned: u32,
    pub queen_health: f32,
    pub eggs: u32,
    pub larvae: u32,
    pub pupae: u32,
    pub caste_ratio: CasteRatio,
    pub behavior_weights: BehaviorWeights,
    pub population: PopulationCounts,
    pub nest_entrance_positions: Vec<Vec2>,
    /// Active brood records (eggs/larvae/pupae). Stage counts above
    /// are derived from this each economy tick.
    pub brood: Vec<Brood>,
    /// Fractional egg accumulator; queen lays an egg once this reaches 1.0.
    pub egg_accumulator: f32,
    /// Pending ant deaths from starvation, applied by the simulation
    /// (oldest-first) since ColonyState cannot touch the ant vector.
    pub pending_starvation_deaths: u32,
    /// Whether the colony has ever laid an egg (used for first-egg logging).
    pub has_laid_egg: bool,
    /// Last population milestone announced (multiples of 50).
    pub last_population_milestone: u32,
    /// Was the queen alive last tick? Detects queen-death events.
    pub queen_alive_last_tick: bool,
    /// K3: whole in-game days this year spent in colony-wide diapause.
    pub days_in_diapause_this_year: u32,
    /// K3: fractional-day accumulator feeding `days_in_diapause_this_year`.
    /// Stored in *in-game seconds*, rolled over at 86400.
    pub diapause_seconds_this_year: f32,
    /// K3: last `sim.in_game_year()` we evaluated the fertility gate for.
    pub last_year_evaluated: u32,
    /// K3: queen won't lay eggs this year (missed hibernation requirement).
    pub fertility_suppressed: bool,
    /// K4: awarded milestones in order they fired.
    #[serde(default)]
    pub milestones: Vec<Milestone>,
    /// K4: last observed season, used for Winter→Spring transition detection.
    #[serde(default)]
    pub last_season_idx: u8,
    /// K5: cumulative count of breeders this colony has sent on nuptial flights.
    #[serde(default)]
    pub nuptial_launches: u32,
    /// K5: cumulative count of daughter colonies successfully founded.
    #[serde(default)]
    pub daughter_colonies_founded: u32,
    /// K5: cumulative breeders lost to predation mid-flight.
    #[serde(default)]
    pub nuptial_predation_deaths: u32,
    /// K5: tick of the most recent nuptial launch (0 = never).
    #[serde(default)]
    pub last_nuptial_flight_tick: u64,
}

impl ColonyState {
    pub fn new(id: u8, initial_food: f32, entrance: Vec2) -> Self {
        Self {
            id,
            food_stored: initial_food,
            food_returned: 0,
            queen_health: 100.0,
            eggs: 0,
            larvae: 0,
            pupae: 0,
            caste_ratio: CasteRatio::default(),
            behavior_weights: BehaviorWeights::default(),
            population: PopulationCounts::default(),
            nest_entrance_positions: vec![entrance],
            brood: Vec::new(),
            egg_accumulator: 0.0,
            pending_starvation_deaths: 0,
            has_laid_egg: false,
            last_population_milestone: 0,
            queen_alive_last_tick: true,
            days_in_diapause_this_year: 0,
            diapause_seconds_this_year: 0.0,
            last_year_evaluated: 0,
            fertility_suppressed: false,
            milestones: Vec::new(),
            last_season_idx: u8::MAX, // sentinel "unset"
            nuptial_launches: 0,
            daughter_colonies_founded: 0,
            nuptial_predation_deaths: 0,
            last_nuptial_flight_tick: 0,
        }
    }

    /// Was this milestone already awarded?
    pub fn has_milestone(&self, kind: crate::milestones::MilestoneKind) -> bool {
        self.milestones.iter().any(|m| m.kind == kind)
    }

    pub fn accept_food(&mut self, amount: f32) {
        self.food_stored += amount;
        self.food_returned += amount as u32;
    }

    /// Sum of all live adult ants tracked in `population`.
    pub fn adult_total(&self) -> u32 {
        self.population.workers + self.population.soldiers + self.population.breeders
    }
}

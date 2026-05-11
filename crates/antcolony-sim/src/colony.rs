//! Colony-level state: economy, brood pipeline, caste/behavior allocation.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::ant::AntCaste;
use crate::milestones::Milestone;

/// Phase-7+ tech unlocks: biology mechanics that are ON by default in
/// Keeper mode and can be withheld/unlocked in a future PvP/versus
/// mode. See `docs/biology.md` → "Tech Unlocks for PvP".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TechUnlock {
    /// Queen produces non-viable nutritive eggs. See biology.md →
    /// "Trophic eggs".
    TrophicEggs,
    /// Workers cannibalize brood to feed adults when food_stored runs
    /// low. See biology.md → "Survival cannibalism of brood is normal".
    BroodCannibalism,
    /// Queen lay rate scales with incoming food over a rolling window.
    /// See biology.md → "Queen egg-laying is throttled by recent food
    /// intake, not static reserve".
    FoodInflowThrottle,
}

impl TechUnlock {
    /// The full set of unlocks, used as the default in Keeper mode.
    pub fn all_defaults() -> Vec<TechUnlock> {
        vec![
            TechUnlock::TrophicEggs,
            TechUnlock::BroodCannibalism,
            TechUnlock::FoodInflowThrottle,
        ]
    }
}

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
    /// Per-colony storage cap override. Populated from species
    /// TOML's `diet.food_storage_cap` at colony creation. None =
    /// use the runtime default in `effective_food_cap()`.
    ///
    /// TODO: wire from species TOML → Colony at colony creation time.
    #[serde(default)]
    pub food_storage_cap_override: Option<f32>,
    /// K5: tick of the most recent nuptial launch (0 = never).
    #[serde(default)]
    pub last_nuptial_flight_tick: u64,
    /// P4: this colony is driven by the red-colony AI loop instead of a
    /// human player. Flips the economy's behavior/caste auto-adjust on.
    #[serde(default)]
    pub is_ai_controlled: bool,
    /// AI-vs-AI MVP: this colony has an EXTERNAL `AiBrain` controlling
    /// its caste_ratio + behavior_weights via `apply_ai_decision`. When
    /// true, the legacy `red_ai_tick` SKIPS this colony so the brain's
    /// per-N-tick decisions aren't overwritten every tick by the
    /// heuristic loop. Set automatically the first time
    /// `apply_ai_decision` is called for this colony.
    #[serde(default)]
    pub external_brain: bool,
    /// P4: cross-colony kills the sim has resolved against this colony
    /// so far. Cumulative.
    #[serde(default)]
    pub combat_losses: u32,
    /// P4: cross-colony kills this colony has inflicted. Cumulative.
    #[serde(default)]
    pub combat_kills: u32,
    /// P4: combat deaths observed THIS tick (cleared at the end of every
    /// tick). Red AI reads this to escalate soldier production.
    #[serde(default)]
    pub combat_losses_this_tick: u32,
    /// P7+: exponentially-decaying running average of food delivered
    /// per tick. Units = food/tick. Drives queen-laying throttle
    /// (`TechUnlock::FoodInflowThrottle`) and is a cheap mirror of the
    /// real-biology trophallaxis pipeline — the queen can only lay as
    /// many eggs as the protein pipeline supports.
    #[serde(default)]
    pub food_inflow_recent: f32,
    /// Fractional-deaths accumulator for the smooth-starvation cap.
    /// Each starving tick adds `adult_total * STARVATION_PER_TICK`;
    /// `floor()` of this value is the max deaths permitted that tick,
    /// and is debited on death. Resets to 0 when food_stored >= 0.
    /// Replaces the `cap.max(1)` floor that bottomed out at 1/tick
    /// (43200 deaths/day for any colony size, not the intended 1%/day).
    /// See postmortem `2026-05-09-seasonal-transition-cliffs.md` fix #3.
    #[serde(default)]
    pub starvation_accumulator: f32,
    /// P7+: unlocked biology mechanics. Defaults to "everything on"
    /// (Keeper mode). In a PvP sim, construct the colony with a subset
    /// (e.g. via a research tree). See `docs/biology.md`.
    #[serde(default)]
    pub tech_unlocks: Vec<TechUnlock>,
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
            is_ai_controlled: false,
            external_brain: false,
            combat_losses: 0,
            combat_kills: 0,
            combat_losses_this_tick: 0,
            food_inflow_recent: 0.0,
            tech_unlocks: TechUnlock::all_defaults(),
            food_storage_cap_override: None,
            starvation_accumulator: 0.0,
        }
    }

    /// Returns the effective food-storage cap for this colony.
    ///
    /// Uses the per-species override if set, otherwise falls back to
    /// `target_population * egg_cost * 10` — a biologically-grounded
    /// ceiling that scales with colony size. See postmortem
    /// 2026-05-09: rudis hit 44k food / 960 workers (1-2 OOM above
    /// field-realistic) via food-overaccumulation bug.
    pub fn effective_food_cap(&self, target_population: u32, egg_cost: f32) -> f32 {
        self.food_storage_cap_override
            .unwrap_or((target_population.max(1) as f32) * egg_cost * 10.0)
    }

    /// P7+: is this mechanic unlocked for this colony?
    pub fn has_tech(&self, tech: TechUnlock) -> bool {
        self.tech_unlocks.iter().any(|t| *t == tech)
    }

    /// Was this milestone already awarded?
    pub fn has_milestone(&self, kind: crate::milestones::MilestoneKind) -> bool {
        self.milestones.iter().any(|m| m.kind == kind)
    }

    pub fn accept_food(&mut self, amount: f32) {
        self.food_stored += amount;
        self.food_returned += amount as u32;
        // Food-inflow pulse: instantaneous bump. The exponential decay
        // is applied per-tick in colony_economy_tick so a steady
        // stream of deliveries keeps the running average high.
        self.food_inflow_recent += amount;
    }

    /// Sum of all live adult ants tracked in `population`.
    pub fn adult_total(&self) -> u32 {
        self.population.workers + self.population.soldiers + self.population.breeders
    }
}

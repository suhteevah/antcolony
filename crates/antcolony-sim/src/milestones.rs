//! K4 Keeper progression — milestones awarded as the colony grows.
//!
//! Each milestone fires exactly once per colony. Call
//! [`Simulation::evaluate_milestones`] once per tick (it's cheap — just a
//! handful of integer comparisons) and it will append any newly earned
//! milestones to each `ColonyState.milestones`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MilestoneKind {
    FirstEgg,
    /// Polymorphic species' first-ever Soldier (major) spawn.
    FirstMajor,
    PopulationTen,
    PopulationFifty,
    PopulationOneHundred,
    PopulationFiveHundred,
    /// 365 in-game days since colony founding.
    FirstColonyAnniversary,
    /// Transitioned from Winter → Spring with live adults.
    SurvivedFirstWinter,
}

impl MilestoneKind {
    pub fn label(&self) -> &'static str {
        match self {
            MilestoneKind::FirstEgg => "First Egg",
            MilestoneKind::FirstMajor => "First Major",
            MilestoneKind::PopulationTen => "Population 10",
            MilestoneKind::PopulationFifty => "Population 50",
            MilestoneKind::PopulationOneHundred => "Population 100",
            MilestoneKind::PopulationFiveHundred => "Population 500",
            MilestoneKind::FirstColonyAnniversary => "First Anniversary",
            MilestoneKind::SurvivedFirstWinter => "Survived First Winter",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub kind: MilestoneKind,
    pub tick_awarded: u64,
    pub in_game_day: u32,
}

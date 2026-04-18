//! Phase 7 player-interaction state: pheromone beacons.
//!
//! A beacon is a player-placed pheromone emitter on a specific module
//! cell. Each tick while `ticks_remaining > 0` it deposits its `amount`
//! of the chosen layer at its cell. Soldiers biased toward Alarm
//! will converge (via the existing alarm-response helper); ants biased
//! toward FoodTrail / HomeTrail will follow the standard ACO gradient.
//!
//! Two built-in beacon flavours:
//! - `gather`: `FoodTrail` layer — summons foragers.
//! - `attack`: `Alarm` layer — summons soldiers, scatters workers.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::module::ModuleId;
use crate::pheromone::PheromoneLayer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeaconKind {
    /// Deposits `FoodTrail` — pulls foragers.
    Gather,
    /// Deposits `Alarm` — pulls soldiers, scatters workers.
    Attack,
}

impl BeaconKind {
    pub fn layer(self) -> PheromoneLayer {
        match self {
            BeaconKind::Gather => PheromoneLayer::FoodTrail,
            BeaconKind::Attack => PheromoneLayer::Alarm,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beacon {
    pub id: u32,
    pub kind: BeaconKind,
    pub module_id: ModuleId,
    pub position: Vec2,
    /// Amount deposited per tick while active.
    pub amount_per_tick: f32,
    /// Ticks until the beacon fades out. Decremented every tick.
    pub ticks_remaining: u32,
    /// Colony that owns this beacon — used to ignore beacons from
    /// rival colonies if we ever teach AI to respect them. For now,
    /// deposits are colony-agnostic (pheromones are shared).
    pub owner_colony: u8,
}

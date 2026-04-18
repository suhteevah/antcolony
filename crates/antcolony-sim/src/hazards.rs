//! Phase 6 environmental hazards: predators (spider + antlion) and
//! weather events (rain + lawnmower).
//!
//! Predators are independent agents that live on a single module and
//! pressure the colony. Spiders move around hunting; antlions are
//! stationary pit-traps that kill any ant unlucky enough to wander in.
//!
//! Weather events are sim-wide state toggles with timer fields. They're
//! orchestrated by `Simulation::hazards_tick`.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::module::ModuleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredatorKind {
    /// Fast mobile predator. Patrols, hunts ants in range, eats the
    /// kill over several ticks, then resumes patrolling. Can be killed
    /// by soldier ants (corpse = large food source). Respawns after a
    /// cooldown.
    Spider,
    /// Stationary pit trap. Any ant stepping onto the cell dies. No
    /// respawn on destruction — clearing antlions is permanent progress.
    Antlion,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PredatorState {
    Patrol,
    /// Chasing a specific ant by id; if the target is gone, predator
    /// falls back to Patrol.
    Hunt { target_ant_id: u32 },
    /// Currently consuming a kill; blocks all other behavior for the
    /// configured duration.
    Eat { remaining_ticks: u32 },
    /// Spider is dead — waiting to respawn. Antlions don't use this.
    Dead { respawn_in_ticks: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Predator {
    pub id: u32,
    pub kind: PredatorKind,
    pub module_id: ModuleId,
    pub position: Vec2,
    pub heading: f32,
    pub state: PredatorState,
    pub health: f32,
}

/// Weather timeline. All fields are tick counters driven by
/// `Simulation::hazards_tick`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Weather {
    /// Remaining ticks of active rainfall (0 = clear).
    pub rain_ticks_remaining: u32,
    /// Tick of the last rain event start; used by the period check.
    pub last_rain_start_tick: u64,
    /// Remaining warning ticks before the next lawnmower sweep (0 = no
    /// warning active).
    pub lawnmower_warning_remaining: u32,
    /// Remaining sweep ticks — while > 0, the mower is actively moving
    /// across the map.
    pub lawnmower_sweep_remaining: u32,
    /// Which module the lawnmower is sweeping this pass.
    pub lawnmower_module: ModuleId,
    /// Current y-line of the lawnmower blade (world-space cell y). When
    /// sweep_remaining hits 0 the pass ends.
    pub lawnmower_y: f32,
    /// Cumulative lifetime lawnmower kills.
    pub total_mower_kills: u32,
    /// Cumulative lifetime rain events triggered.
    pub total_rain_events: u32,
}

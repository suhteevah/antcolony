//! Core ant colony simulation. No rendering, no Bevy.
//!
//! All game logic lives here. The `antcolony-game` crate wraps these types
//! in Bevy ECS components; `antcolony-render` paints them.

pub mod ant;
pub mod colony;
pub mod config;
pub mod environment;
pub mod error;
pub mod milestones;
pub mod module;
pub mod persist;
pub mod pheromone;
pub mod simulation;
pub mod spatial;
pub mod species;
pub mod topology;
pub mod tube;
pub mod unlocks;
pub mod world;

pub use ant::{Ant, AntCaste, AntState};
pub use colony::{BehaviorWeights, Brood, BroodStage, CasteRatio, ColonyState, PopulationCounts};
pub use config::{
    AntConfig, ColonyConfig, CombatConfig, PheromoneConfig, SimConfig, WorldConfig,
};
pub use environment::{Climate, Environment, Season, TimeScale};
pub use error::SimError;
pub use module::{Module, ModuleId, ModuleKind, PortPos};
pub use pheromone::{PheromoneGrid, PheromoneLayer};
pub use simulation::Simulation;
pub use spatial::SpatialHash;
pub use species::{
    Appearance, Biology, CombatProfile, Diet, Difficulty, Encyclopedia, FoundingType, Growth,
    Species, load_species_dir,
};
pub use milestones::{Milestone, MilestoneKind};
pub use persist::{
    Snapshot, compute_catchup_ticks, load_snapshot, now_unix_secs, save_snapshot,
    SNAPSHOT_FORMAT_VERSION,
};
pub use topology::Topology;
pub use tube::{Tube, TubeEnd, TubeId, TubeTransit};
pub use unlocks::{module_kind_unlocked, unlock_hint};
pub use world::{Terrain, WorldGrid};

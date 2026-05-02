//! Core ant colony simulation. No rendering, no Bevy.
//!
//! All game logic lives here. The `antcolony-game` crate wraps these types
//! in Bevy ECS components; `antcolony-render` paints them.

pub mod ai;
pub mod ant;
pub mod bench;
pub mod species_extended;
pub mod colony;
pub mod config;
pub mod environment;
pub mod error;
pub mod hazards;
pub mod milestones;
pub mod module;
pub mod persist;
pub mod pheromone;
pub mod player;
pub mod simulation;
pub mod spatial;
pub mod species;
pub mod topology;
pub mod tube;
pub mod unlocks;
pub mod world;

pub use ai::{
    Arbiter, Blackboard, BlackboardSnapshot, Cadence, CombatKs, Contribution, Directive, Fact,
    FactRef, ForagerKs, KnowledgeSource, KsName, StrategistKs,
};
pub use ant::{Ant, AntCaste, AntState};
pub use colony::{BehaviorWeights, Brood, BroodStage, CasteRatio, ColonyState, PopulationCounts, TechUnlock};
pub use config::{
    AntConfig, ColonyConfig, CombatConfig, PheromoneConfig, SimConfig, WorldConfig,
};
pub use environment::{Climate, Environment, Season, TimeScale};
pub use error::SimError;
pub use hazards::{Predator, PredatorKind, PredatorState, Weather};
pub use player::{Beacon, BeaconKind};
pub use module::{Module, ModuleId, ModuleKind, PortPos, SubstrateKind};
pub use pheromone::{PheromoneGrid, PheromoneLayer};
pub use simulation::Simulation;
pub use spatial::SpatialHash;
pub use species::{
    Appearance, Biology, CombatProfile, Diet, Difficulty, Encyclopedia, FoundingType, Growth,
    Species, load_species_dir,
};
pub use species_extended::{
    Behavior, ColonyStructure, CombatExtended, DielActivity, DietExtended, EcologicalRole,
    InvasiveStatus, MoundConstruction, QueenCount, RecruitmentStyle, Substrate, SubstrateType,
    Weapon, WorkerSizeBucket, CURRENT_SCHEMA_VERSION,
};
pub use milestones::{Milestone, MilestoneKind};
pub use persist::{
    Snapshot, compute_catchup_ticks, load_snapshot, now_unix_secs, save_snapshot,
    SNAPSHOT_FORMAT_VERSION,
};
pub use topology::Topology;
pub use tube::{Tube, TubeEnd, TubeId, TubeTransit};
pub use unlocks::{module_kind_unlocked, unlock_hint};
pub use world::{ChamberType, Terrain, WorldGrid};

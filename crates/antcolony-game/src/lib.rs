//! Bevy ECS integration for the ant colony simulation.

pub mod plugin;
pub mod resources;

pub use plugin::{SimSet, SimulationPlugin, tick_simulation_system_marker};
pub use resources::SimulationState;

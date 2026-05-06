use bevy::prelude::*;

use crate::resources::SimulationState;

/// SystemSet labeling the sim tick. Other plugins (e.g. PvP netcode)
/// can order their work against `SimSet::Tick` -- typically `.before()`
/// to inject decisions before the sim advances.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimSet {
    Tick,
}

/// Public no-op marker function. Lets external plugins reference the
/// sim tick by function pointer when SystemSet ordering is impractical.
/// The actual sim tick lives in `tick_simulation_system` (private) but
/// is members-of `SimSet::Tick` so `.before(SimSet::Tick)` works.
pub fn tick_simulation_system_marker() {}

/// Registers the FixedUpdate tick that advances the simulation.
///
/// The `SimulationState` resource is NOT created here — the keeper-mode
/// picker inserts it after the player chooses a species. The tick system
/// is gated on the resource existing so it is a no-op while the player is
/// still in the picker screen.
#[derive(Default, Clone, Copy)]
pub struct SimulationPlugin {
    pub ticks_per_second: f32,
}

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        let hz = if self.ticks_per_second <= 0.0 { 30.0 } else { self.ticks_per_second };

        tracing::info!(hz, "SimulationPlugin::build (picker owns SimulationState)");

        app.insert_resource(Time::<Fixed>::from_hz(hz as f64))
            .add_systems(
                FixedUpdate,
                tick_simulation_system
                    .in_set(SimSet::Tick)
                    .run_if(resource_exists::<SimulationState>),
            );
    }
}

fn tick_simulation_system(mut sim: ResMut<SimulationState>) {
    sim.sim.tick();
}

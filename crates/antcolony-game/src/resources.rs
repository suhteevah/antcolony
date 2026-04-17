use antcolony_sim::{Environment, Simulation, Species};
use bevy::prelude::Resource;

/// Resource wrapper around the headless `Simulation`.
/// Renderer reads this every frame. Created by the Keeper-mode picker
/// after the player chooses species + environment.
#[derive(Resource, Debug, Clone)]
pub struct SimulationState {
    pub sim: Simulation,
    pub species: Species,
    pub environment: Environment,
}

impl SimulationState {
    /// Build a SimulationState from a chosen species and environment.
    /// Seeds a few demo food clusters so there is something to forage.
    pub fn from_species(species: &Species, env: &Environment) -> Self {
        let cfg = species.apply(env);
        let mut sim = Simulation::new(cfg, env.seed);

        let w = sim.world.width as i64;
        let h = sim.world.height as i64;
        sim.spawn_food_cluster(w / 6, h / 6, 4, 40);
        sim.spawn_food_cluster(w - w / 6, h - h / 6, 4, 40);
        sim.spawn_food_cluster(w / 6, h - h / 6, 3, 30);

        tracing::info!(
            species = %species.id,
            scale = env.time_scale.label(),
            seed = env.seed,
            "SimulationState::from_species initialized"
        );

        Self {
            sim,
            species: species.clone(),
            environment: env.clone(),
        }
    }
}

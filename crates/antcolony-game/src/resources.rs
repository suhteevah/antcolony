use antcolony_sim::{Environment, Simulation, Species, Topology};
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
    ///
    /// Keeper Mode ships with a two-module starter formicarium — a
    /// TestTubeNest (module 0) where ants spawn, and an Outworld
    /// (module 1) where food is placed. One tube connects them.
    pub fn from_species(species: &Species, env: &Environment) -> Self {
        let cfg = species.apply(env);

        // Nest ≈ 20% of the world-config size; outworld fills the rest.
        let nest_w = (env.world_width / 4).max(24);
        let nest_h = (env.world_height / 3).max(20);
        let out_w = env.world_width;
        let out_h = env.world_height;
        // K2.2: include an auto-refilling FeedingDish as a third module.
        let dish_w = (out_w / 3).max(18);
        let dish_h = (out_h / 3).max(14);
        let topology = Topology::starter_formicarium_with_feeder(
            (nest_w, nest_h),
            (out_w, out_h),
            (dish_w, dish_h),
        );
        let mut sim = Simulation::new_with_topology(cfg, topology, env.seed);
        sim.set_environment(env);

        // Place food clusters across the outworld (module 1).
        let ow = out_w as i64;
        let oh = out_h as i64;
        sim.spawn_food_cluster_on(1, ow / 5, oh / 5, 4, 40);
        sim.spawn_food_cluster_on(1, ow - ow / 5, oh - oh / 5, 4, 40);
        sim.spawn_food_cluster_on(1, ow - ow / 5, oh / 5, 3, 30);

        tracing::info!(
            species = %species.id,
            scale = env.time_scale.label(),
            seed = env.seed,
            modules = sim.topology.modules.len(),
            "SimulationState::from_species initialized (starter formicarium)"
        );

        Self {
            sim,
            species: species.clone(),
            environment: env.clone(),
        }
    }
}

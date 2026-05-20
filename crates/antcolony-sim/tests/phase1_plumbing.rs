//! Phase 1 plumbing integration tests — exercise the new Simulation API
//! end-to-end. These tests intentionally bypass the unit-test layer to
//! catch wiring mistakes (e.g. forgetting to populate a snapshot field).

use antcolony_sim::ai::observation::{AntModulators, AntObservation, HistoryToken, RichObservation};

#[test]
fn rich_observation_shape_for_default_match_env() {
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    let cfg = SimConfig {
        world: WorldConfig {
            width: 32,
            height: 32,
            ..WorldConfig::default()
        },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig {
            initial_count: 10,
            ..AntConfig::default()
        },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    let rich = sim.colony_rich_observation(0).expect("colony 0 exists");
    assert_eq!(rich.pheromone_field.width, 32);
    assert_eq!(rich.pheromone_field.height, 32);
    assert_eq!(rich.pheromone_field.food_trail.len(), 32 * 32);
    assert_eq!(rich.pheromone_field.home_trail.len(), 32 * 32);
    assert_eq!(rich.pheromone_field.alarm.len(), 32 * 32);
    assert_eq!(rich.pheromone_field.colony_scent.len(), 32 * 32);
    assert_eq!(rich.history.len(), 0); // fresh sim, no commander decisions yet
}

#[test]
fn rich_observation_returns_none_for_nonexistent_colony() {
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    let cfg = SimConfig {
        world: WorldConfig {
            width: 32,
            height: 32,
            ..WorldConfig::default()
        },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig {
            initial_count: 10,
            ..AntConfig::default()
        },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    assert!(sim.colony_rich_observation(99).is_none());
}

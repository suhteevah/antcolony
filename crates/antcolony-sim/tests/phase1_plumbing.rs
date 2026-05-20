//! Phase 1 plumbing integration tests — exercise the new Simulation API
//! end-to-end. These tests intentionally bypass the unit-test layer to
//! catch wiring mistakes (e.g. forgetting to populate a snapshot field).

// The other Phase-1 observation types (AntModulators, AntObservation,
// HistoryToken) are imported per-test as they're added in later tasks.
use antcolony_sim::ai::observation::RichObservation;

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

#[test]
fn per_ant_observations_count_matches_colony_population() {
    use antcolony_sim::ai::observation::AntObservation;
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 7, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    let obs: Vec<AntObservation> = sim.per_ant_observations(0);
    assert_eq!(obs.len(), 7, "should match initial_count=7");
    for o in &obs {
        // pheromone_cone has fixed 60-d shape
        assert_eq!(o.pheromone_cone.len(), 60);
        // internal[1]^2 + internal[2]^2 ≈ 1 (heading_sin² + heading_cos²)
        let h2 = o.internal[1] * o.internal[1] + o.internal[2] * o.internal[2];
        assert!((h2 - 1.0).abs() < 1e-4, "heading sin/cos should be unit, got {}", h2);
        // caste onehot sums to 1
        let caste_sum = o.internal[3] + o.internal[4] + o.internal[5];
        assert!((caste_sum - 1.0).abs() < 1e-4, "caste onehot should sum to 1, got {}", caste_sum);
    }
}

#[test]
fn per_ant_observations_empty_for_nonexistent_colony() {
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 5, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    let obs = sim.per_ant_observations(99);
    assert_eq!(obs.len(), 0);
}

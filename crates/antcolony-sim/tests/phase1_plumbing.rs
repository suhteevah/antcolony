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

#[test]
fn apply_ant_modulators_writes_through_to_pool() {
    use antcolony_sim::ai::observation::AntModulators;
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
    let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    let obs = sim.per_ant_observations(0);
    assert!(obs.len() >= 2);
    let target_id_a = obs[0].ant_id;
    let target_id_b = obs[1].ant_id;

    sim.apply_ant_modulators(
        0,
        &[AntModulators {
            alpha_mult: 3.0,
            beta_mult: 0.5,
            exploration_mod: 0.05,
            deposit_mult: 2.0,
            state_bias: -1.0,
        }],
        &[target_id_a],
    );

    let ant_a = sim.ants.iter().find(|a| a.id == target_id_a).unwrap();
    assert_eq!(ant_a.modulators.alpha_mult, 3.0);
    let ant_b = sim.ants.iter().find(|a| a.id == target_id_b).unwrap();
    assert_eq!(ant_b.modulators, AntModulators::default());
}

#[test]
fn apply_ant_modulators_clamps_to_safe_ranges() {
    use antcolony_sim::ai::observation::AntModulators;
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 3, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);
    let target = sim.per_ant_observations(0)[0].ant_id;

    sim.apply_ant_modulators(
        0,
        &[AntModulators {
            alpha_mult: 999.0,
            beta_mult: -10.0,
            exploration_mod: 5.0,
            deposit_mult: 1000.0,
            state_bias: 100.0,
        }],
        &[target],
    );

    let ant = sim.ants.iter().find(|a| a.id == target).unwrap();
    assert!((0.1..=5.0).contains(&ant.modulators.alpha_mult));
    assert!((0.1..=5.0).contains(&ant.modulators.beta_mult));
    assert!((-0.1..=0.1).contains(&ant.modulators.exploration_mod));
    assert!((0.1..=5.0).contains(&ant.modulators.deposit_mult));
    assert!((-2.0..=2.0).contains(&ant.modulators.state_bias));
}

#[test]
fn apply_ant_modulators_unknown_id_is_noop() {
    use antcolony_sim::ai::observation::AntModulators;
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 3, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    // ant id 0xFFFFFFFF doesn't exist — must not panic.
    sim.apply_ant_modulators(0, &[AntModulators::default()], &[0xFFFFFFFF]);
}

#[test]
fn apply_commander_intent_roundtrips_through_rich_observation() {
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 3, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2);

    let mut intent = [0.0f32; 64];
    intent[3] = 1.5;
    intent[42] = -2.7;
    sim.apply_commander_intent(0, &intent);

    let colony = sim.colonies.get(0).unwrap();
    assert_eq!(colony.commander_intent[3], 1.5);
    assert_eq!(colony.commander_intent[42], -2.7);
    assert_eq!(colony.commander_intent[0], 0.0);

    // Unknown colony — must not panic.
    sim.apply_commander_intent(99, &intent);
}

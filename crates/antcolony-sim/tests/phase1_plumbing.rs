//! Phase 1 plumbing integration tests — exercise the new Simulation API
//! end-to-end. These tests intentionally bypass the unit-test layer to
//! catch wiring mistakes (e.g. forgetting to populate a snapshot field).

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

#[test]
fn defaults_reproduce_baseline_population_trajectory() {
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    fn run_sim(seed: u64, ticks: u64, exercise_plumbing: bool) -> Vec<(u32, u32, f32)> {
        let cfg = SimConfig {
            world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
            pheromone: PheromoneConfig::default(),
            ant: AntConfig { initial_count: 10, ..AntConfig::default() },
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        };
        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, seed, 0, 2);
        for t in 0..ticks {
            if exercise_plumbing && t % 5 == 0 {
                // Call all the new read-side methods every 5 ticks. Their
                // outputs are unused (the trainer does this in Phase 2).
                // If they have side-effects this loop catches them.
                let _ = sim.colony_rich_observation(0);
                let _ = sim.colony_rich_observation(1);
                let _ = sim.per_ant_observations(0);
                let _ = sim.per_ant_observations(1);
                // apply_ant_modulators with DEFAULTS is the identity
                // transform — must not perturb the sim.
                let obs0 = sim.per_ant_observations(0);
                let mods: Vec<_> = obs0.iter().map(|_| antcolony_sim::AntModulators::default()).collect();
                let ids: Vec<_> = obs0.iter().map(|o| o.ant_id).collect();
                sim.apply_ant_modulators(0, &mods, &ids);
                // apply_commander_intent with zeros is also identity.
                sim.apply_commander_intent(0, &[0.0; 64]);
                sim.apply_commander_intent(1, &[0.0; 64]);
            }
            sim.tick();
        }
        // Snapshot final population + food per colony.
        let mut snap = Vec::new();
        for cid in 0..2 {
            if let Some(c) = sim.colonies.get(cid as usize) {
                snap.push((
                    c.population.workers,
                    c.population.soldiers,
                    c.food_stored,
                ));
            }
        }
        snap
    }

    let baseline = run_sim(0xb45_e11e, 500, false);
    let with_plumbing = run_sim(0xb45_e11e, 500, true);

    assert_eq!(
        baseline, with_plumbing,
        "defaults reproduce baseline trajectory: read-side methods + default modulators + zero intent must be the identity. \
         baseline = {:?}, with_plumbing = {:?}", baseline, with_plumbing,
    );
}

#[test]
fn high_alpha_modulators_change_sim_trajectory() {
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    fn run_sim(seed: u64, ticks: u64, force_high_alpha: bool) -> u32 {
        let cfg = SimConfig {
            world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
            pheromone: PheromoneConfig::default(),
            ant: AntConfig { initial_count: 10, ..AntConfig::default() },
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        };
        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, seed, 0, 2);

        for t in 0..ticks {
            if force_high_alpha && t % 5 == 0 {
                // Set every ant in colony 0 to alpha_mult=5 (max pheromone-following).
                let obs0 = sim.per_ant_observations(0);
                let mods: Vec<_> = obs0.iter().map(|_| antcolony_sim::AntModulators {
                    alpha_mult: 5.0,
                    beta_mult: 1.0,
                    exploration_mod: -0.1, // max suppression of random exploration
                    deposit_mult: 1.0,
                    state_bias: 0.0,
                }).collect();
                let ids: Vec<_> = obs0.iter().map(|o| o.ant_id).collect();
                sim.apply_ant_modulators(0, &mods, &ids);
            }
            sim.tick();
        }
        sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0)
    }

    let baseline_workers = run_sim(0xbeef_ace, 1000, false);
    let high_alpha_workers = run_sim(0xbeef_ace, 1000, true);

    // Behavior MUST differ — high_alpha-driven ants follow trails more
    // tightly, so colony-1's food intake (and downstream worker count)
    // is materially different from baseline. The test asserts difference,
    // not direction — either side could be larger depending on emergent
    // dynamics, and that's fine.
    assert_ne!(
        baseline_workers, high_alpha_workers,
        "non-default modulators must change the sim trajectory (got identical worker counts {} = {})",
        baseline_workers, high_alpha_workers,
    );
}

#[test]
fn all_phase1_types_reexported_at_crate_root() {
    // This test compiles only if all five types are re-exported at the
    // crate root — the public API surface the trainer crate consumes.
    let _: antcolony_sim::AntModulators = antcolony_sim::AntModulators::default();
    let _: antcolony_sim::HistoryToken = antcolony_sim::HistoryToken::default();
    // PheromoneSnapshot, RichObservation, AntObservation are non-Default;
    // existence-check by type assignment only — wrapping in fn-ptr binding.
    fn _check_phsnap(_: &antcolony_sim::PheromoneSnapshot) {}
    fn _check_rich(_: &antcolony_sim::RichObservation) {}
    fn _check_antobs(_: &antcolony_sim::AntObservation) {}
}

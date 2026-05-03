//! Sanity test: after apply_ai_decision, colony state actually reflects
//! the decision. If this passes but matchup_bench results don't differ
//! by brain, the bug is downstream (in what the sim consumes).

use antcolony_sim::{AiDecision, RandomBrain, Simulation, Topology, AiBrain};

fn small_two_colony_sim() -> Simulation {
    use antcolony_sim::config::{AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig};
    let cfg = SimConfig {
        world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: 8, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    let topology = Topology::two_colony_arena((24, 24), (32, 32));
    Simulation::new_ai_vs_ai_with_topology(cfg, topology, 7, 0, 2)
}

#[test]
fn applying_decision_changes_colony_caste_ratio() {
    let mut sim = small_two_colony_sim();
    let initial = sim.colonies[1].caste_ratio.clone();
    let decision = AiDecision {
        caste_ratio_worker: 0.05,
        caste_ratio_soldier: 0.90,
        caste_ratio_breeder: 0.05,
        forage_weight: 0.1,
        dig_weight: 0.1,
        nurse_weight: 0.8,
        research_choice: None,
    };
    sim.apply_ai_decision(1, &decision);
    let after = &sim.colonies[1].caste_ratio;
    assert!(after.soldier > initial.soldier, "soldier should rise: {} -> {}", initial.soldier, after.soldier);
    assert!((after.soldier - 0.9).abs() < 0.01, "soldier should be ~0.9, got {}", after.soldier);
    assert!(sim.colonies[1].external_brain, "external_brain flag should be set");
}

#[test]
fn external_brain_persists_across_ticks() {
    let mut sim = small_two_colony_sim();
    let decision = AiDecision {
        caste_ratio_worker: 0.05, caste_ratio_soldier: 0.90, caste_ratio_breeder: 0.05,
        forage_weight: 0.1, dig_weight: 0.1, nurse_weight: 0.8,
        research_choice: None,
    };
    sim.apply_ai_decision(1, &decision);
    let soldier_after_decision = sim.colonies[1].caste_ratio.soldier;

    // Run a single tick — red_ai_tick fires within. If external_brain
    // is honored, soldier ratio should be UNCHANGED.
    sim.tick();
    let soldier_after_tick = sim.colonies[1].caste_ratio.soldier;
    assert!(
        (soldier_after_tick - soldier_after_decision).abs() < 0.01,
        "external_brain should prevent red_ai_tick from overwriting (was {}, now {})",
        soldier_after_decision, soldier_after_tick,
    );
}

#[test]
fn random_brain_decisions_actually_vary() {
    // Two RandomBrains with different seeds should produce different
    // decisions for the same state — proves the brain is non-degenerate.
    let mut a = RandomBrain::new(1);
    let mut b = RandomBrain::new(2);
    let sim = small_two_colony_sim();
    let s = sim.colony_ai_state(1).expect("state");
    let da = a.decide(&s);
    let db = b.decide(&s);
    assert_ne!(da.caste_ratio_worker, db.caste_ratio_worker);
}

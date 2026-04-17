//! Integration test: run a headless simulation, assert emergent food delivery.

use antcolony_sim::{SimConfig, Simulation};

#[test]
fn headless_delivers_food() {
    let mut cfg = SimConfig::default();
    cfg.world.width = 64;
    cfg.world.height = 64;
    cfg.ant.initial_count = 40;
    // Disable economy so this test focuses purely on emergent foraging;
    // Phase 3 economy tests live in the sim crate.
    cfg.colony.adult_food_consumption = 0.0;
    cfg.colony.queen_egg_rate = 0.0;
    let mut sim = Simulation::new(cfg, 42);
    sim.spawn_food_cluster(12, 12, 3, 20);
    sim.spawn_food_cluster(52, 52, 3, 20);
    sim.run(5000);
    assert!(
        sim.colonies[0].food_returned > 0,
        "expected some food delivered after 5000 ticks, got {}",
        sim.colonies[0].food_returned
    );
}

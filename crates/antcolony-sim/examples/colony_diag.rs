//! Headless colony diagnostic runner.
//!
//! Builds a Keeper-mode starter (Lasius niger default) exactly as the
//! picker does, runs it at max tick rate, logs colony telemetry every
//! N ticks. Use this to reason about food/brood/population dynamics
//! without firing up the renderer.
//!
//! Usage: cargo run --release --example colony_diag -- [ticks] [log_every]
//!
//! Defaults: 20000 ticks, log every 500.

use antcolony_sim::{
    AntCaste, AntState, Environment, Simulation, Species, TimeScale, Topology, load_species_dir,
};

fn main() -> anyhow::Result<()> {
    // Plain stdout tracing so we see every important log from the sim.
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=info,colony_diag=info")
        .with_target(false)
        .init();

    let mut args = std::env::args().skip(1);
    let ticks: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let log_every: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(500);

    // Load species catalog the same way the picker does. We just pick
    // Lasius niger because it's the default keeper starter.
    let species_list: Vec<Species> = load_species_dir("assets/species")?;
    let species = species_list
        .iter()
        .find(|s| s.id == "lasius_niger")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("lasius_niger not found in assets/species"))?;

    let env = Environment {
        time_scale: TimeScale::Seasonal,
        ..Environment::default()
    };
    let cfg = species.apply(&env);

    // Replicate SimulationState::from_species topology + food seeding.
    let nest_w = (env.world_width / 4).max(24);
    let nest_h = (env.world_height / 3).max(20);
    let out_w = env.world_width;
    let out_h = env.world_height;
    let dish_w = (out_w / 3).max(18);
    let dish_h = (out_h / 3).max(14);
    // Nuclear-starve mode: build a topology with NO feeding dish so
    // the colony can't silently drink from the auto-refill.
    let nuclear = std::env::var("NUKE").map(|s| s == "1").unwrap_or(false);
    let mut topology = if nuclear {
        println!("colony_diag: NUKE mode — no feeding dish in topology");
        Topology::starter_formicarium((nest_w, nest_h), (out_w, out_h))
    } else {
        Topology::starter_formicarium_with_feeder(
            (nest_w, nest_h),
            (out_w, out_h),
            (dish_w, dish_h),
        )
    };
    let _underground_id = topology.attach_underground(0, 0, nest_w.max(32), nest_h.max(24));
    let mut sim = Simulation::new_with_topology(cfg, topology, env.seed);
    sim.set_environment(&env);

    // Food clusters in the outworld (module 1), same as from_species.
    // Skip seeding if STARVE=1 is set — lets the runner verify the
    // starvation-path biology (brood cannibalism, throttle floor).
    let starve = std::env::var("STARVE").map(|s| s == "1").unwrap_or(false);
    if !starve {
        let ow = out_w as i64;
        let oh = out_h as i64;
        sim.spawn_food_cluster_on(1, ow / 5, oh / 5, 4, 40);
        sim.spawn_food_cluster_on(1, ow - ow / 5, oh - oh / 5, 4, 40);
        sim.spawn_food_cluster_on(1, ow - ow / 5, oh / 5, 3, 30);
    } else {
        println!("colony_diag: STARVE mode — no food seeded in outworld");
    }

    println!(
        "colony_diag: species={} seed={} time_scale={} target_ticks={} log_every={}",
        species.id,
        env.seed,
        env.time_scale.label(),
        ticks,
        log_every,
    );
    println!(
        "colony_diag: starter ants={} modules={} tubes={}",
        sim.ants.len(),
        sim.topology.modules.len(),
        sim.topology.tubes.len(),
    );

    println!(
        "{:>7}  {:>7}  {:>7}  {:>7}  {:>7}  {:>7}  {:>8}  {:>9}  {:>6}  {:>6}  {:>6}  {:>6}  {:>8}",
        "tick", "workers", "soldrs", "breedrs", "queens", "total", "food", "inflow", "eggs", "larva", "pupa", "fret", "state"
    );

    let mut prev_workers = 0u32;
    let mut dead_worker_log_last = 0u64;
    for t in 0..ticks {
        let pre_workers = sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0);
        sim.tick();
        let post_workers = sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0);
        // Transition logging: the tick workers crossed zero.
        if prev_workers == 0 && post_workers == 0 && pre_workers > 0 && t - dead_worker_log_last > 100 {
            println!("tick={} EVENT: worker population hit 0", t);
            dead_worker_log_last = t;
        }
        prev_workers = post_workers;

        if t % log_every == 0 || t + 1 == ticks {
            let c = &sim.colonies[0];
            let workers = c.population.workers;
            let soldiers = c.population.soldiers;
            let breeders = c.population.breeders;
            let queens = sim
                .ants
                .iter()
                .filter(|a| a.caste == AntCaste::Queen && a.colony_id == 0)
                .count() as u32;
            let total = sim.ants.iter().filter(|a| a.colony_id == 0).count();
            let state_summary = format_state_histogram(&sim);
            println!(
                "{:>7}  {:>7}  {:>7}  {:>7}  {:>7}  {:>7}  {:>7.2}  {:>9.3}  {:>6}  {:>6}  {:>6}  {:>6}  {:>8}",
                sim.tick,
                workers,
                soldiers,
                breeders,
                queens,
                total,
                c.food_stored,
                c.food_inflow_recent,
                c.eggs,
                c.larvae,
                c.pupae,
                c.food_returned,
                state_summary,
            );
        }
    }

    let c = &sim.colonies[0];
    println!();
    println!("final: tick={} ants={} workers={} food={:.2} food_returned={} eggs={} larvae={} pupae={}",
        sim.tick,
        sim.ants.iter().filter(|a| a.colony_id == 0).count(),
        c.population.workers,
        c.food_stored,
        c.food_returned,
        c.eggs,
        c.larvae,
        c.pupae,
    );
    Ok(())
}

fn format_state_histogram(sim: &Simulation) -> String {
    let mut explore = 0;
    let mut follow = 0;
    let mut pickup = 0;
    let mut ret = 0;
    let mut store = 0;
    let mut other = 0;
    for a in &sim.ants {
        if a.colony_id != 0 {
            continue;
        }
        match a.state {
            AntState::Exploring => explore += 1,
            AntState::FollowingTrail => follow += 1,
            AntState::PickingUpFood => pickup += 1,
            AntState::ReturningHome => ret += 1,
            AntState::StoringFood => store += 1,
            _ => other += 1,
        }
    }
    // Format: E/F/P/R/S/O
    format!("{}/{}/{}/{}/{}/{}", explore, follow, pickup, ret, store, other)
}

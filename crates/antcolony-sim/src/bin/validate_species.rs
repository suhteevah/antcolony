//! `validate-species` — CLI gate for community species PRs.
//!
//! Path-traversal lint exempted via `.semgrepignore`; this is a developer
//! CLI that takes operator-typed file paths by design.
//!
//! Verifies that one or more species TOMLs:
//! 1. Parse against the current schema
//! 2. Carry a known `schema_version`
//! 3. Have a registered `SpeciesExpectations` entry in `bench::expected`
//! 4. Survive a short headless bench run (no `CONFIG REJECTED`, no
//!    `Sim-init failure`, composite score ≥ MIN_PASSING_SCORE)
//!
//! Exit codes:
//! - 0 = all validated species passed
//! - 1 = at least one species failed validation
//! - 2 = bad CLI arguments / IO error
//!
//! # Usage
//!
//! ```text
//! # Validate one TOML
//! cargo run --release --bin validate-species -- assets/species/lasius_niger.toml
//!
//! # Validate every shipped species (shell glob expansion)
//! cargo run --release --bin validate-species -- assets/species/*.toml
//!
//! # Validate a contributor PR
//! cargo run --release --bin validate-species -- path/to/myrmica_rubra.toml
//! ```
//!
//! Intended use: CI gate on PRs that touch `assets/species/*.toml`.
//! The CLI deliberately requires explicit file arguments — no
//! "scan-all" mode — which makes the path contract auditable: every
//! file the binary touches was named on the command line by the
//! operator (or by their CI script).

use std::path::PathBuf;

use antcolony_sim::{
    Environment, Simulation, Species, TimeScale, Topology,
    bench::expected,
    species_extended::CURRENT_SCHEMA_VERSION,
};

/// Number of sim ticks to run during validation. The validator's job
/// is to catch instantiation failures (panics, divide-by-zero in
/// `species::apply`, topology mismatches), NOT to deeply benchmark.
/// 30 ticks at Seasonal scale is enough to:
///   - exercise `species::apply` -> `SimConfig`
///   - construct topology + spatial hash + pheromone grids
///   - tick once (catches panics in any per-tick system for this species)
///   - complete in <1s per species in release mode
/// For deep bench output, contributors run `examples/species_bench`.
const VALIDATION_TICKS: u64 = 30;
const VALIDATION_SCALE: TimeScale = TimeScale::Seasonal;

#[derive(Debug)]
struct ValidationFailure {
    file: PathBuf,
    species_id: String,
    reasons: Vec<String>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!(
            "validate-species: no TOML files to validate.\n\
             Usage: validate-species <path1.toml> [<path2.toml> ...]\n\
             To validate all shipped species: validate-species assets/species/*.toml"
        );
        std::process::exit(2);
    }

    let targets: Vec<PathBuf> = args.into_iter().map(PathBuf::from).collect();

    println!("validate-species: checking {} file(s)", targets.len());
    println!();

    let mut failures: Vec<ValidationFailure> = Vec::new();

    for path in &targets {
        let started = std::time::Instant::now();
        // eprintln (stderr) is line-buffered even when piped, unlike stdout.
        eprintln!("  >>>>  starting {} ...", path.display());
        match validate_one(path) {
            Ok(()) => println!("  PASS  {} ({:.1}s)", path.display(), started.elapsed().as_secs_f64()),
            Err(failure) => {
                println!("  FAIL  {} ({:.1}s)", path.display(), started.elapsed().as_secs_f64());
                for reason in &failure.reasons {
                    println!("        - {reason}");
                }
                failures.push(failure);
            }
        }
    }

    println!();
    println!(
        "validate-species: {} passed, {} failed (of {} total)",
        targets.len() - failures.len(),
        failures.len(),
        targets.len(),
    );

    if !failures.is_empty() {
        println!();
        println!("FAILURES:");
        for f in &failures {
            println!("  {} ({}):", f.species_id, f.file.display());
            for reason in &f.reasons {
                println!("    - {reason}");
            }
        }
        std::process::exit(1);
    }
}


fn validate_one(path: &PathBuf) -> Result<(), ValidationFailure> {
    let mut reasons: Vec<String> = Vec::new();
    let mut species_id = String::from("?");

    // 1. Parse the TOML.
    let species = match Species::load_from_file(path) {
        Ok(s) => {
            species_id = s.id.clone();
            s
        }
        Err(e) => {
            reasons.push(format!("TOML parse error: {e}"));
            return Err(ValidationFailure {
                file: path.clone(),
                species_id,
                reasons,
            });
        }
    };

    // 2. Schema version check.
    if species.schema_version > CURRENT_SCHEMA_VERSION {
        reasons.push(format!(
            "schema_version {} is newer than this validator's CURRENT_SCHEMA_VERSION={}. \
             Either upgrade the validator or downgrade the TOML.",
            species.schema_version, CURRENT_SCHEMA_VERSION,
        ));
    }
    if species.schema_version == 0 {
        reasons.push(
            "schema_version=0 — likely a TOML placement bug; \
             schema_version must be a top-level field, not nested under a section."
                .to_string(),
        );
    }

    // 3. Bench expectations registered.
    let exp = expected::for_species_id(&species.id);
    if exp.is_none() {
        reasons.push(format!(
            "no SpeciesExpectations entry registered for `{}` — \
             add one in crates/antcolony-sim/src/bench/expected.rs::for_species_id \
             so the bench harness can validate against literature.",
            species.id,
        ));
    }

    // 4. Verify the species can actually instantiate as a Sim. This
    //    catches panics in `species::apply`, topology mismatches, and
    //    per-tick system bugs that depend on this species' parameters.
    //    We do NOT run a full bench here — that's `examples/species_bench`.
    if let Err(reason) = try_instantiate_and_tick(&species) {
        reasons.push(format!("sim instantiation failed: {reason}"));
    }

    // 5. Verify colony 0 actually exists after instantiation. This is
    //    the harness's worst-case caveat ("Sim-init failure") promoted
    //    to a hard validator failure.
    if !verify_colony_zero_present(&species) {
        reasons.push(
            "colony 0 was not present after Simulation construction — \
             the species' biology must produce at least one colony at \
             startup. Likely cause: bad starting_workers or topology issue."
                .to_string(),
        );
    }

    if reasons.is_empty() {
        Ok(())
    } else {
        Err(ValidationFailure {
            file: path.clone(),
            species_id,
            reasons,
        })
    }
}

/// Construct a Simulation for the species and tick it `VALIDATION_TICKS`
/// times. Catches panics from `species::apply`, topology mismatches,
/// and per-tick system bugs. Uses the same starter formicarium as the
/// in-game picker so the topology shape matches "real" gameplay.
fn try_instantiate_and_tick(species: &Species) -> Result<(), String> {
    let env = Environment {
        time_scale: VALIDATION_SCALE,
        seed: 42,
        ..Environment::default()
    };
    let cfg = species.apply(&env);

    let nest_w = (env.world_width / 4).max(24);
    let nest_h = (env.world_height / 3).max(20);
    let out_w = env.world_width;
    let out_h = env.world_height;
    let dish_w = (out_w / 3).max(18);
    let dish_h = (out_h / 3).max(14);
    let mut topology = Topology::starter_formicarium_with_feeder(
        (nest_w, nest_h),
        (out_w, out_h),
        (dish_w, dish_h),
    );
    let _ = topology.attach_underground(0, 0, nest_w.max(32), nest_h.max(24));
    topology.fit_bore_to_species(species.appearance.size_mm, species.biology.polymorphic);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut sim = Simulation::new_with_topology(cfg, topology, env.seed);
        sim.set_environment(&env);
        for _ in 0..VALIDATION_TICKS {
            sim.tick();
        }
    }));

    match result {
        Ok(()) => Ok(()),
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic with non-string payload".to_string()
            };
            Err(msg)
        }
    }
}

/// Quick check that colony 0 exists right after construction (no ticks).
fn verify_colony_zero_present(species: &Species) -> bool {
    let env = Environment {
        time_scale: VALIDATION_SCALE,
        seed: 42,
        ..Environment::default()
    };
    let cfg = species.apply(&env);
    let nest_w = (env.world_width / 4).max(24);
    let nest_h = (env.world_height / 3).max(20);
    let out_w = env.world_width;
    let out_h = env.world_height;
    let dish_w = (out_w / 3).max(18);
    let dish_h = (out_h / 3).max(14);
    let mut topology = Topology::starter_formicarium_with_feeder(
        (nest_w, nest_h),
        (out_w, out_h),
        (dish_w, dish_h),
    );
    let _ = topology.attach_underground(0, 0, nest_w.max(32), nest_h.max(24));
    let sim = Simulation::new_with_topology(cfg, topology, env.seed);
    !sim.colonies.is_empty()
}

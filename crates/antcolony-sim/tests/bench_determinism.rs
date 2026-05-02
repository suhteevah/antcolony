//! Determinism: two bench runs with the same config must produce
//! byte-identical samples and identical scores.
//!
//! Required for ecologist-grade reproducibility.

use antcolony_sim::{
    Species, TimeScale, bench::run::{BenchRunConfig, run_one}, load_species_dir,
};

fn load_species(id: &str) -> Species {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("species");
    load_species_dir(&dir)
        .unwrap()
        .into_iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("species {id} not found"))
}

#[test]
fn bench_is_deterministic_lasius_short_smoke() {
    // Uses Timelapse + 0.05 years (~18 in-game days, ~33k ticks). Non-default
    // scale chosen for test speed only — the long-run-collapse bug requires
    // many simulated years to manifest, so an 18-day smoke is safe.
    let species = load_species("lasius_niger");
    let cfg_a = BenchRunConfig {
        species: species.clone(),
        years: 0.05,
        time_scale: TimeScale::Timelapse,
        seed: 42,
        sample_every_days: 2,
    };
    let cfg_b = cfg_a.clone();

    let result_a = run_one(cfg_a);
    let result_b = run_one(cfg_b);

    assert_eq!(
        result_a.samples.len(),
        result_b.samples.len(),
        "sample counts diverged"
    );
    for (a, b) in result_a.samples.iter().zip(result_b.samples.iter()) {
        assert_eq!(a.tick, b.tick);
        assert_eq!(a.workers, b.workers);
        assert_eq!(a.eggs, b.eggs);
        assert_eq!(a.larvae, b.larvae);
        assert_eq!(a.pupae, b.pupae);
        assert_eq!(a.food_returned_cumulative, b.food_returned_cumulative);
        assert_eq!(a.queens_alive, b.queens_alive);
    }

    let score_a = result_a.score.composite_0_to_100();
    let score_b = result_b.score.composite_0_to_100();
    assert_eq!(score_a, score_b, "composite scores diverged");
}

#[test]
fn bench_different_seeds_produce_different_samples() {
    let species = load_species("lasius_niger");
    let cfg_seed42 = BenchRunConfig {
        species: species.clone(),
        years: 0.05,
        time_scale: TimeScale::Timelapse,
        seed: 42,
        sample_every_days: 2,
    };
    let mut cfg_seed99 = cfg_seed42.clone();
    cfg_seed99.seed = 99;

    let r42 = run_one(cfg_seed42);
    let r99 = run_one(cfg_seed99);

    // Same length (deterministic schedule), but at least ONE field
    // should differ across the run — RNG controls forager direction
    // and many other choices.
    assert_eq!(r42.samples.len(), r99.samples.len());
    let any_diff = r42
        .samples
        .iter()
        .zip(r99.samples.iter())
        .any(|(a, b)| {
            a.workers != b.workers
                || a.eggs != b.eggs
                || a.food_returned_cumulative != b.food_returned_cumulative
        });
    assert!(any_diff, "different seeds produced identical telemetry — RNG not in use?");
}

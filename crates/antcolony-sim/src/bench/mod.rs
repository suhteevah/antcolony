//! Species bench harness — run a species through the simulation, capture
//! per-tick telemetry, score against literature-cited expectations, render
//! audit-grade reports.
//!
//! # Why this exists
//!
//! The antcolony simulation aspires to ecologist-grade species fidelity.
//! That ambition is hollow without a way to **independently verify** the
//! sim is reproducing real biology. This module is the verifier.
//!
//! For each shipped species the harness:
//! 1. Runs a deterministic headless sim of N in-game years.
//! 2. Samples colony state on a regular cadence.
//! 3. Scores the run against literature-cited expected ranges in
//!    `expected.rs`.
//! 4. Emits CSV (raw audit trail) and Markdown (human review).
//!
//! Every threshold, every metric, every expected range is **cited** to
//! either (a) a peer-reviewed paper, (b) a canonical reference work, or
//! (c) explicitly tagged as a deliberate game-pacing choice with a
//! written rationale. There are no silent numbers.
//!
//! # Module layout
//!
//! - [`expected`] — per-species expected-range tables with citations.
//! - [`metrics`] — what gets observed, plus scoring math (testable in isolation).
//! - [`run`] — the actual runner.
//! - [`report`] — CSV + Markdown formatters.
//!
//! # Entry point
//!
//! ```no_run
//! use antcolony_sim::bench;
//! use antcolony_sim::{load_species_dir, Species};
//!
//! let species = load_species_dir("assets/species").unwrap()
//!     .into_iter().find(|s| s.id == "lasius_niger").unwrap();
//! let cfg = bench::run::BenchRunConfig::standard_5yr(species);
//! let result = bench::run::run_one(cfg);
//! let csv = bench::report::render_csv(&result);
//! let md = bench::report::render_markdown(&result);
//! println!("{md}");
//! ```
//!
//! # Determinism
//!
//! The harness uses a fixed default seed (42). Two runs with the same
//! config produce byte-identical samples. See `tests/bench_determinism.rs`.

pub mod expected;
pub mod metrics;
pub mod report;
pub mod run;

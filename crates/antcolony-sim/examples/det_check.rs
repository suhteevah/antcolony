//! Determinism gate for the netcode work.
//!
//! Runs the sim from a fixed seed for N ticks. Every K ticks, dumps a
//! Snapshot JSON (with the wall-clock-dependent fields zeroed) so two
//! runs can be byte-compared.
//!
//! # Workflow
//!
//! ```bash
//! cargo run -p antcolony-sim --release --example det_check -- --out bench/det/run1
//! cargo run -p antcolony-sim --release --example det_check -- --out bench/det/run2
//! diff -r bench/det/run1 bench/det/run2     # should be silent
//! ```
//!
//! Any output from `diff` is a determinism bug. The first divergent tick
//! file tells us where in the sim hot path the non-determinism creeps in.
//!
//! Default config = the same `two_colony_arena` that PvP will use, with
//! both colonies under hardcoded archetype brains. Same seed each run.

use std::path::PathBuf;

use antcolony_sim::{
    AggressorBrain, AiBrain, DefenderBrain, Simulation, Topology,
    config::{AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig},
    persist::Snapshot,
};

const DECISION_CADENCE: u64 = 5;

#[derive(Debug, Clone)]
struct Args {
    out_dir: PathBuf,
    ticks: u64,
    snapshot_every: u64,
    seed: u64,
    arena: u32,
    initial_ants: u32,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("bench/det/run1"),
            ticks: 1_000,
            snapshot_every: 50,
            seed: 0xa17c01,
            arena: 32,
            initial_ants: 10,
        }
    }
}

fn parse_args() -> anyhow::Result<Args> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut a = Args::default();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--out" => { a.out_dir = PathBuf::from(&raw[i+1]); i += 2; }
            "--ticks" => { a.ticks = raw[i+1].parse()?; i += 2; }
            "--snapshot-every" => { a.snapshot_every = raw[i+1].parse()?; i += 2; }
            "--seed" => { a.seed = raw[i+1].parse()?; i += 2; }
            "--arena" => { a.arena = raw[i+1].parse()?; i += 2; }
            "--initial-ants" => { a.initial_ants = raw[i+1].parse()?; i += 2; }
            other => anyhow::bail!("unknown arg `{other}`"),
        }
    }
    Ok(a)
}

fn make_sim(args: &Args) -> Simulation {
    let q = args.arena as usize;
    let cfg = SimConfig {
        world: WorldConfig { width: q, height: q, ..WorldConfig::default() },
        pheromone: PheromoneConfig::default(),
        ant: AntConfig { initial_count: args.initial_ants as usize, ..AntConfig::default() },
        colony: ColonyConfig::default(),
        combat: CombatConfig::default(),
        hazards: HazardConfig::default(),
    };
    // Same arena PvP will use.
    let topology = Topology::two_colony_arena((q, q), (q, q));
    Simulation::new_ai_vs_ai_with_topology(cfg, topology, args.seed, 0, 2)
}

/// Strip wall-clock-derived fields so two snapshots taken in different
/// processes can be byte-compared.
fn normalize_snapshot(snap: &mut Snapshot) {
    snap.saved_at_unix_secs = 0;
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,det_check=info")
        .with_target(false)
        .init();

    let args = parse_args()?;
    std::fs::create_dir_all(&args.out_dir)?;
    tracing::info!(?args, "det_check starting");

    let mut sim = make_sim(&args);

    // Two fixed brains -- same archetype both sides so neither side races
    // ahead, exposing more sim state to the determinism check.
    let mut left: Box<dyn AiBrain> = Box::new(DefenderBrain::new());
    let mut right: Box<dyn AiBrain> = Box::new(AggressorBrain::new());

    let env = antcolony_sim::Environment::default();

    for t in 0..args.ticks {
        if t % DECISION_CADENCE == 0 {
            if let Some(s) = sim.colony_ai_state(0) {
                let d = left.decide(&s);
                sim.apply_ai_decision(0, &d);
            }
            if let Some(s) = sim.colony_ai_state(1) {
                let d = right.decide(&s);
                sim.apply_ai_decision(1, &d);
            }
        }
        sim.tick();
        if t % args.snapshot_every == 0 || t + 1 == args.ticks {
            let mut snap = Snapshot::from_sim(&sim, "det_check", &env);
            normalize_snapshot(&mut snap);
            let path = args.out_dir.join(format!("tick_{:06}.json", sim.tick));
            std::fs::write(&path, serde_json::to_string_pretty(&snap)?)?;
        }
    }

    // Final summary line — quick sanity check across runs.
    let final_path = args.out_dir.join("FINAL.txt");
    let last_ant = sim.ants.last().map(|a| format!("id={} pos=({:.4},{:.4}) col={} caste={:?}", a.id, a.position.x, a.position.y, a.colony_id, a.caste)).unwrap_or_else(|| "<none>".into());
    let summary = format!(
        "ticks={} ants={} colonies={} last_ant={}\n",
        sim.tick, sim.ants.len(), sim.colonies.len(), last_ant,
    );
    std::fs::write(&final_path, &summary)?;
    tracing::info!(out = %args.out_dir.display(), %summary, "det_check done");

    Ok(())
}

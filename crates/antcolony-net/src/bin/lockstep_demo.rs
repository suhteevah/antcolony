//! Lockstep transport smoke test.
//!
//! Two processes run identical AI-vs-AI sims, exchanging AiDecisions
//! over TCP. State hashes are compared every decision tick; mismatch
//! aborts with a Desync error.
//!
//! ```text
//! # terminal A
//! cargo run -p antcolony-net --release --bin lockstep_demo -- host --port 17001 --role black --seed 42
//!
//! # terminal B
//! cargo run -p antcolony-net --release --bin lockstep_demo -- join --addr 127.0.0.1:17001 --role red --seed 42
//! ```
//!
//! Both peers run for `--ticks` (default 1000) or until match resolves,
//! whichever comes first. Final summary prints last tick + winner.

use antcolony_net::{
    sim_state_hash, DECISION_CADENCE,
    PeerRole, PeerConfig, LockstepPeer,
    transport::{host, connect},
};
use antcolony_sim::{
    AggressorBrain, AiBrain, DefenderBrain, MatchStatus, Simulation, Topology,
    config::{AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig},
};
use std::time::Duration;

#[derive(Debug, Clone)]
struct Args {
    mode: Mode,
    role: PeerRole,
    seed: u64,
    ticks: u64,
    arena: u32,
    initial_ants: u32,
}

#[derive(Debug, Clone)]
enum Mode {
    Host { port: u16 },
    Join { addr: String },
}

fn parse_args() -> anyhow::Result<Args> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() {
        anyhow::bail!("expected `host` or `join` subcommand");
    }
    let mut role = PeerRole::Black;
    let mut seed: u64 = 42;
    let mut ticks: u64 = 1000;
    let mut arena: u32 = 32;
    let mut initial_ants: u32 = 10;
    let mut port: u16 = 17001;
    let mut addr: Option<String> = None;
    let sub = raw[0].clone();
    let mut i = 1;
    while i < raw.len() {
        match raw[i].as_str() {
            "--role" => {
                role = match raw[i+1].as_str() {
                    "black" => PeerRole::Black,
                    "red" => PeerRole::Red,
                    "spider" => PeerRole::Spider,
                    other => anyhow::bail!("unknown role `{other}`"),
                };
                i += 2;
            }
            "--seed" => { seed = raw[i+1].parse()?; i += 2; }
            "--ticks" => { ticks = raw[i+1].parse()?; i += 2; }
            "--arena" => { arena = raw[i+1].parse()?; i += 2; }
            "--initial-ants" => { initial_ants = raw[i+1].parse()?; i += 2; }
            "--port" => { port = raw[i+1].parse()?; i += 2; }
            "--addr" => { addr = Some(raw[i+1].clone()); i += 2; }
            other => anyhow::bail!("unknown arg `{other}`"),
        }
    }
    let mode = match sub.as_str() {
        "host" => Mode::Host { port },
        "join" => Mode::Join { addr: addr.ok_or_else(|| anyhow::anyhow!("--addr required for join"))? },
        other => anyhow::bail!("unknown subcommand `{other}` (expected host or join)"),
    };
    Ok(Args { mode, role, seed, ticks, arena, initial_ants })
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
    let topology = Topology::two_colony_arena((q, q), (q, q));
    Simulation::new_ai_vs_ai_with_topology(cfg, topology, args.seed, 0, 2)
}

/// Hash the SimConfig + arena/topology choice. Cheap stable hash so
/// peers can verify they're running the same setup.
fn config_hash(args: &Args) -> u64 {
    // Just FNV the bytes of the config-affecting fields.
    let mut h: u64 = 0xcbf29ce484222325;
    let mul = |h: &mut u64| { *h = h.wrapping_mul(0x100000001b3); };
    let push = |h: &mut u64, b: &[u8]| {
        for x in b { *h ^= *x as u64; mul(h); }
    };
    push(&mut h, &args.arena.to_le_bytes());
    push(&mut h, &args.initial_ants.to_le_bytes());
    push(&mut h, b"two_colony_arena_v1");
    h
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_net=info,lockstep_demo=info")
        .with_target(false)
        .init();

    let args = parse_args()?;
    tracing::info!(?args, "lockstep_demo starting");

    let stream = match &args.mode {
        Mode::Host { port } => host(("0.0.0.0", *port))?,
        Mode::Join { addr } => connect(addr.as_str())?,
    };

    let cfg = PeerConfig {
        role: args.role,
        seed: args.seed,
        config_hash: config_hash(&args),
        display_name: format!("{:?}", args.role),
        recv_timeout: Some(Duration::from_secs(30)),
    };
    let mut peer = LockstepPeer::new(stream, cfg)?;
    peer.handshake()?;

    let mut sim = make_sim(&args);
    let mut local_brain: Box<dyn AiBrain> = match args.role {
        PeerRole::Black => Box::new(DefenderBrain::new()),
        PeerRole::Red => Box::new(AggressorBrain::new()),
        PeerRole::Spider => anyhow::bail!("spider role not supported in V1 lockstep_demo"),
    };
    let local_colony = args.role.colony_id();

    let mut last_status = MatchStatus::InProgress;
    let mut decision_tick: u64 = 0;
    while sim.tick < args.ticks {
        // Both peers must arrive at this branch identically.
        if sim.tick % DECISION_CADENCE == 0 {
            // Compute local decision and exchange with peer.
            let s = sim.colony_ai_state(local_colony)
                .ok_or_else(|| anyhow::anyhow!("missing local colony state"))?;
            let local_decision = local_brain.decide(&s);

            let local_hash = sim_state_hash(&sim);
            let ours = antcolony_net::TickInput {
                tick: decision_tick,
                decision: local_decision.clone(),
                state_hash: local_hash,
            };

            let remote = peer.exchange_tick(ours)?;

            // Apply BOTH decisions in a fixed order so both peers arrive
            // at the same post-apply state.
            sim.apply_ai_decision(local_colony, &local_decision);
            sim.apply_ai_decision(remote_colony(args.role), &remote.decision);
            decision_tick += 1;
        }
        sim.tick();

        last_status = sim.match_status();
        if !matches!(last_status, MatchStatus::InProgress) {
            tracing::info!(?last_status, tick = sim.tick, "match resolved");
            break;
        }
    }

    let _ = peer.send_disconnect(format!("done at tick {}, status {:?}", sim.tick, last_status));
    tracing::info!(
        final_tick = sim.tick,
        ?last_status,
        ants = sim.ants.len(),
        "lockstep_demo done"
    );
    Ok(())
}

fn remote_colony(local: PeerRole) -> u8 {
    match local {
        PeerRole::Black => 1,
        PeerRole::Red => 0,
        PeerRole::Spider => 0,
    }
}


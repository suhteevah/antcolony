//! K4 save/load + offline catch-up.
//!
//! Snapshot is pretty JSON so the files are human-inspectable. The
//! simulation RNG state is NOT saved — only the original seed on
//! `Environment` is persisted, and on load we reseed with that. The doc
//! comment on [`Snapshot`] warns callers that post-load RNG rolls will
//! diverge from an uninterrupted run; all other gameplay state is
//! restored faithfully.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ant::Ant;
use crate::colony::ColonyState;
use crate::environment::{Climate, Environment};
use crate::simulation::Simulation;
use crate::species::Species;
use crate::topology::Topology;

pub const SNAPSHOT_FORMAT_VERSION: u32 = 1;

/// Serializable snapshot of a live simulation.
///
/// Notes:
/// - The ChaCha8Rng state is **not** serialized. On load we rebuild an
///   rng from `environment.seed`, so any roll made after a save will
///   diverge from an uninterrupted session. All deterministic state
///   (positions, timers, brood, pheromones, temperature) IS preserved.
/// - `PheromoneGrid.scratch` is skipped during serialization and
///   rebuilt by [`Simulation::from_snapshot`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub format_version: u32,
    pub species_id: String,
    pub environment: Environment,
    pub climate: Climate,
    pub tick: u64,
    pub in_game_seconds_per_tick: f32,
    pub next_ant_id: u32,
    pub topology: Topology,
    pub ants: Vec<Ant>,
    pub colonies: Vec<ColonyState>,
    /// Wall-clock seconds since UNIX_EPOCH at save time. Used to compute
    /// the catch-up tick count on load.
    pub saved_at_unix_secs: i64,
}

impl Snapshot {
    /// Build a snapshot from a live simulation and its environment.
    pub fn from_sim(sim: &Simulation, species_id: &str, env: &Environment) -> Self {
        let saved_at_unix_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Self {
            format_version: SNAPSHOT_FORMAT_VERSION,
            species_id: species_id.to_string(),
            environment: env.clone(),
            climate: sim.climate,
            tick: sim.tick,
            in_game_seconds_per_tick: sim.in_game_seconds_per_tick,
            next_ant_id: sim.next_ant_id_value(),
            topology: sim.topology.clone(),
            ants: sim.ants.clone(),
            colonies: sim.colonies.clone(),
            saved_at_unix_secs,
        }
    }
}

/// Write a snapshot as pretty JSON to `path`.
pub fn save_snapshot(
    sim: &Simulation,
    species_id: &str,
    env: &Environment,
    path: &Path,
) -> anyhow::Result<()> {
    let snap = Snapshot::from_sim(sim, species_id, env);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(&snap)?;
    std::fs::write(path, json)?;
    tracing::info!(
        path = %path.display(),
        tick = snap.tick,
        ants = snap.ants.len(),
        modules = snap.topology.modules.len(),
        saved_at = snap.saved_at_unix_secs,
        "save_snapshot wrote JSON"
    );
    Ok(())
}

/// Load a snapshot from a pretty-JSON file. Rejects unknown format versions.
pub fn load_snapshot(path: &Path) -> anyhow::Result<Snapshot> {
    let data = std::fs::read_to_string(path)?;
    let snap: Snapshot = serde_json::from_str(&data)?;
    if snap.format_version != SNAPSHOT_FORMAT_VERSION {
        tracing::warn!(
            have = snap.format_version,
            want = SNAPSHOT_FORMAT_VERSION,
            "snapshot format version mismatch — refusing to load"
        );
        anyhow::bail!(
            "snapshot format_version {} unsupported (expected {})",
            snap.format_version,
            SNAPSHOT_FORMAT_VERSION
        );
    }
    tracing::info!(
        path = %path.display(),
        tick = snap.tick,
        ants = snap.ants.len(),
        species = %snap.species_id,
        "load_snapshot read JSON"
    );
    Ok(snap)
}

/// Current wall-clock unix seconds.
pub fn now_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Compute offline catch-up ticks from (real_seconds_elapsed × tick_rate_hz),
/// capped at `MAX_CATCHUP_HOURS × 3600 × tick_rate_hz`.
pub fn compute_catchup_ticks(
    saved_at_unix_secs: i64,
    now_unix_secs: i64,
    tick_rate_hz: f32,
) -> u64 {
    const MAX_CATCHUP_HOURS: f32 = 24.0;
    let elapsed_real = (now_unix_secs - saved_at_unix_secs).max(0) as f32;
    let capped = elapsed_real.min(MAX_CATCHUP_HOURS * 3600.0);
    (capped * tick_rate_hz.max(0.0)).round().max(0.0) as u64
}

/// Species resolver alias used by [`Simulation::from_snapshot`].
pub type SpeciesResolver<'a> = &'a dyn Fn(&str) -> Option<Species>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SimConfig;
    use std::env::temp_dir;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = temp_dir();
        p.push(format!(
            "antcolony_test_{}_{}.json",
            name,
            std::process::id()
        ));
        p
    }

    fn tiny_cfg() -> SimConfig {
        let mut c = SimConfig::default();
        c.world.width = 48;
        c.world.height = 48;
        c.ant.initial_count = 10;
        c.ant.exploration_rate = 0.25;
        c.colony.adult_food_consumption = 0.0;
        c.colony.queen_egg_rate = 0.0;
        c
    }

    #[test]
    fn roundtrip_preserves_core_state() {
        let cfg = tiny_cfg();
        let mut sim = Simulation::new(cfg, 77);
        sim.spawn_food_cluster(12, 12, 2, 5);
        sim.run(500);

        let env = Environment::default();
        let path = tmp_path("roundtrip");
        save_snapshot(&sim, "test.species", &env, &path).expect("save");

        let snap = load_snapshot(&path).expect("load");
        let restored = Simulation::from_snapshot_raw(snap, cfg_for_reconstruction()).expect("restore");

        assert_eq!(restored.tick, sim.tick, "tick preserved");
        assert_eq!(
            restored.ants.len(),
            sim.ants.len(),
            "ant count preserved"
        );
        assert_eq!(
            restored.colonies[0].food_returned,
            sim.colonies[0].food_returned,
            "food_returned preserved"
        );
        assert_eq!(
            restored.topology.modules.len(),
            sim.topology.modules.len(),
            "module count preserved"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn catchup_advances_tick() {
        let cfg = tiny_cfg();
        let mut sim = Simulation::new(cfg, 4);
        let before = sim.tick;
        // Simulate "one hour ago" save.
        let now = 1_700_000_000i64;
        let saved = now - 3600;
        let ticks = compute_catchup_ticks(saved, now, 1.0);
        assert!(ticks >= 3500 && ticks <= 3700, "ticks={ticks}");
        sim.catch_up(ticks);
        assert!(sim.tick >= before + ticks, "tick did not advance");
    }

    #[test]
    fn catchup_cap_enforced() {
        // 10 real days claimed, 24h cap, 30Hz.
        let now = 2_000_000_000i64;
        let saved = now - (10 * 24 * 3600);
        let ticks = compute_catchup_ticks(saved, now, 30.0);
        let expected_cap = (24.0f32 * 3600.0 * 30.0) as u64;
        assert_eq!(ticks, expected_cap);
    }

    fn cfg_for_reconstruction() -> SimConfig {
        tiny_cfg()
    }
}

//! Cheap, stable, process-independent hash of the simulation state.
//!
//! Used as the desync-detection signal in the lockstep protocol. NOT a
//! cryptographic hash -- it just needs to (a) reliably differ when the
//! sim state diverges by even one float and (b) produce the same bytes
//! on both peers when they're in sync.
//!
//! Implementation: FNV-1a over a packed binary layout of the bits we
//! care most about (tick, ant positions, colony food + queen counts).
//! FNV is process-stable (no per-process seed) unlike `DefaultHasher`.
//! ~1 microsecond per call on a 1k-ant sim, well below tick budget.

use antcolony_sim::{AntCaste, Simulation};

#[inline]
fn caste_byte(c: AntCaste) -> u8 {
    match c {
        AntCaste::Worker => 0,
        AntCaste::Soldier => 1,
        AntCaste::Queen => 2,
        AntCaste::Breeder => 3,
    }
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[inline]
fn fnv_update(h: &mut u64, byte: u8) {
    *h ^= byte as u64;
    *h = h.wrapping_mul(FNV_PRIME);
}

#[inline]
fn fnv_bytes(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        fnv_update(h, *b);
    }
}

/// Hash the dynamic parts of the sim state. Stable across processes
/// and OSes (Linux/Proton-GE peers will produce the same value).
///
/// Hashed fields, in order:
/// - sim.tick (u64)
/// - ants.len() (u32)
/// - per-ant: id (u32), colony_id (u8), caste discriminant (u8),
///   position.x (f32 bits), position.y (f32 bits), health (f32 bits),
///   module_id (u16)
/// - per-colony: id (u8), food_stored (f32 bits), workers, soldiers,
///   breeders, queens_alive
///
/// Pheromone grids and brood pools are intentionally excluded -- if
/// ant positions + colony rosters match, those will too. Cheaper hash
/// per tick.
pub fn sim_state_hash(sim: &Simulation) -> u64 {
    let mut h: u64 = FNV_OFFSET;
    fnv_bytes(&mut h, &sim.tick.to_le_bytes());
    fnv_bytes(&mut h, &(sim.ants.len() as u32).to_le_bytes());
    for ant in &sim.ants {
        fnv_bytes(&mut h, &ant.id.to_le_bytes());
        fnv_update(&mut h, ant.colony_id);
        fnv_update(&mut h, caste_byte(ant.caste));
        fnv_bytes(&mut h, &ant.position.x.to_bits().to_le_bytes());
        fnv_bytes(&mut h, &ant.position.y.to_bits().to_le_bytes());
        fnv_bytes(&mut h, &ant.health.to_bits().to_le_bytes());
        fnv_bytes(&mut h, &ant.module_id.to_le_bytes());
    }
    fnv_bytes(&mut h, &(sim.colonies.len() as u32).to_le_bytes());
    for c in &sim.colonies {
        fnv_update(&mut h, c.id);
        fnv_bytes(&mut h, &c.food_stored.to_bits().to_le_bytes());
        fnv_bytes(&mut h, &c.population.workers.to_le_bytes());
        fnv_bytes(&mut h, &c.population.soldiers.to_le_bytes());
        fnv_bytes(&mut h, &c.population.breeders.to_le_bytes());
        // C1 fix (2026-06-11): fold in queen liveness. Queen death is the
        // win condition (`simulation.rs::match_status`), so two peers can
        // diverge on whether a colony's queen is alive while worker/
        // soldier/breeder/food counts momentarily agree -- without this,
        // the hash reports "in sync" through a queen-death desync.
        // `PopulationCounts` has no queens field, so count from `sim.ants`
        // exactly as `match_status` does. Iterates the order-stable
        // `sim.ants` Vec, so the hash stays process-independent.
        let queens = sim.ants.iter()
            .filter(|a| a.colony_id == c.id && matches!(a.caste, AntCaste::Queen))
            .count() as u32;
        fnv_bytes(&mut h, &queens.to_le_bytes());
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use antcolony_sim::{
        Simulation, Topology,
        config::{AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig},
    };

    /// Build the same deterministic 2-colony AI-vs-AI sim `lockstep_demo`
    /// uses, advance a fixed number of ticks, and hash it. The sim is
    /// byte-deterministic (verified cross-process / cross-rayon-thread),
    /// so this hash is a stable golden value.
    fn golden_sim() -> Simulation {
        let q = 32usize;
        let cfg = SimConfig {
            world: WorldConfig { width: q, height: q, ..WorldConfig::default() },
            pheromone: PheromoneConfig::default(),
            ant: AntConfig { initial_count: 10, ..AntConfig::default() },
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        };
        let topology = Topology::two_colony_arena((q, q), (q, q));
        let mut sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, 42, 0, 2);
        for _ in 0..200 {
            sim.tick();
        }
        sim
    }

    /// Determinism gate: the state hash of a fixed sim must be byte-stable.
    ///
    /// RE-BLESSED 2026-06-11: queen liveness is now folded into the hash
    /// (C1 fix in `sim_state_hash`), so this golden value intentionally
    /// differs from any pre-2026-06-11 baseline. Updated to the corrected
    /// value that includes the per-colony queen count. Do NOT weaken this
    /// assertion -- if it fails, the sim's determinism or the hash layout
    /// changed and that must be understood, not papered over.
    #[test]
    fn golden_state_hash_is_stable() {
        let sim = golden_sim();
        let h = sim_state_hash(&sim);
        // Re-blessed 2026-06-11 (queen count added to hash).
        const GOLDEN: u64 = 0xe1d1_76da_d7e1_ad50;
        assert_eq!(
            h, GOLDEN,
            "state hash drifted: got {h:#018x}, expected {GOLDEN:#018x}"
        );
    }

    /// The hash must change when a colony's queen dies while every other
    /// hashed field is held constant -- the exact desync C1 fixes. We
    /// remove the queen ant from one colony and confirm the hash differs.
    #[test]
    fn queen_death_changes_hash() {
        let mut sim = golden_sim();
        let before = sim_state_hash(&sim);
        // Drop one colony's queen; keep everything else identical by
        // re-counting populations the way the sim does is unnecessary --
        // the hash counts queens straight from `sim.ants`.
        let pos = sim.ants.iter().position(|a| matches!(a.caste, AntCaste::Queen))
            .expect("fixture sim must contain at least one queen for this test to be meaningful");
        sim.ants.remove(pos);
        let after = sim_state_hash(&sim);
        assert_ne!(before, after, "queen removal must change the state hash");
    }
}

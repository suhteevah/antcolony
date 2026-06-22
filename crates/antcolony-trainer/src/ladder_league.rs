//! Ladder League: iterated best-response vs the FROZEN tournament-ladder pool.
//! Warm-start from SOTA, train PFSP against frozen opponents (terminal reward),
//! gate the candidate against a standing bar, promote only tournament-validated
//! winners, stop + declare the ceiling after K no-improve rounds.
//!
//! Additive — phase3/SP1/SP2 byte-unchanged. The ONE departure from SP1/SP2:
//! the pool is read-only within a round (no main-snapshot additions), which
//! removes the drift feedback loop those runs hit.

use std::path::PathBuf;

use crate::hierarchical::sizing::Sizing;
use crate::reward::RewardConfig;
use crate::self_play::{Role, SnapshotPool};
use crate::JointPpoConfig;

#[derive(Clone, Debug)]
pub struct LadderContender {
    pub id: String,
    pub spec: String, // "hac:<path>" frozen HAC; archetype names are auto-seeded by the pool
}

#[derive(Clone, Debug)]
pub struct LadderConfig {
    pub sota_path: PathBuf,
    pub initial_contenders: Vec<LadderContender>,
    pub iters_per_round: usize,
    pub eval_every: usize,
    pub train_mpe: usize,
    pub gate_mpe: usize,
    pub gate_margin: f32,
    pub keepbest_arch_floor: f32,
    pub archetype_mix: f32,
    pub pfsp_power: f32,
    pub no_improve_stop: usize,
    pub max_rounds: usize,
    pub out_dir: PathBuf,
    pub sizing: Sizing,
    pub joint: JointPpoConfig,
    pub reward: RewardConfig,
}

#[derive(Clone, Debug)]
pub struct LadderReport {
    pub rounds_run: usize,
    pub promotions: usize,
    pub final_sota_path: PathBuf,
    pub best_h2h_over_seed: f32,
    pub stopped_reason: String,
}

/// Orchestration seed — derived ONLY from the base seed, round, and a distinct
/// index. NEVER from any agent's training RNG (the SP1 critical-bug discipline).
pub fn round_seed(base: u64, round: usize, idx: usize) -> u64 {
    base ^ ((round as u64) << 32) ^ ((idx as u64) << 16)
}

/// Stop the loop after `no_improve_stop` consecutive no-promotion rounds, or at
/// the round cap. Returns the reason string, or None to keep going.
pub fn should_stop(no_improve: usize, no_improve_stop: usize, round: usize, max_rounds: usize) -> Option<&'static str> {
    if no_improve >= no_improve_stop {
        Some("no_improve")
    } else if round >= max_rounds {
        Some("max_rounds")
    } else {
        None
    }
}

/// Build the frozen opponent pool: the 7 archetypes (protected) + one protected
/// snapshot per HAC contender. All entries protected => the opponent SET never
/// changes during a round (PFSP EMA still updates, which is intended).
pub fn build_frozen_pool(contenders: &[LadderContender], ema_alpha: f32) -> SnapshotPool {
    // pool_cap is irrelevant here (all entries protected, never evicted); use a
    // generous value so it never trips even after several promotions.
    let mut pool = SnapshotPool::with_archetypes(1024, ema_alpha);
    for c in contenders {
        if let Some(path) = c.spec.strip_prefix("hac:") {
            tracing::info!(id = %c.id, %path, "ladder: seeding frozen HAC opponent");
            pool.add_protected_snapshot(c.id.clone(), path.to_string(), Role::Main);
        } else {
            tracing::warn!(id = %c.id, spec = %c.spec, "ladder: non-hac contender ignored (archetypes are auto-seeded)");
        }
    }
    pool
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_play::OpponentKind;

    #[test]
    fn round_seed_is_deterministic_and_distinct() {
        let b = 0xABCD_1234;
        assert_eq!(round_seed(b, 1, 0), round_seed(b, 1, 0));      // reproducible
        assert_ne!(round_seed(b, 1, 0), round_seed(b, 2, 0));      // round varies
        assert_ne!(round_seed(b, 1, 0), round_seed(b, 1, 1));      // idx varies
        assert_eq!(round_seed(b, 1, 0), b ^ (1u64 << 32) ^ (0u64 << 16));
    }

    #[test]
    fn should_stop_fires_on_no_improve_then_max_rounds() {
        assert_eq!(should_stop(1, 2, 3, 8), None);                 // 1<2, 3<8 -> keep going
        assert_eq!(should_stop(2, 2, 3, 8), Some("no_improve"));   // hit no-improve cap
        assert_eq!(should_stop(0, 2, 8, 8), Some("max_rounds"));   // hit round cap
    }

    #[test]
    fn build_frozen_pool_has_archetypes_plus_protected_snapshots_all_protected() {
        let cs = vec![
            LadderContender { id: "sota".into(), spec: "hac:bench/x/sota.safetensors".into() },
            LadderContender { id: "sp1term".into(), spec: "hac:bench/y/sp1term.safetensors".into() },
        ];
        let pool = build_frozen_pool(&cs, 0.1);
        // 7 archetypes + 2 snapshots
        assert_eq!(pool.entries.len(), 9);
        assert!(pool.entries.iter().all(|e| e.protected), "every frozen-pool entry must be protected");
        assert_eq!(pool.entries.iter().filter(|e| matches!(e.kind, OpponentKind::Snapshot{..})).count(), 2);
        let names: Vec<&str> = pool.entries.iter().filter_map(|e| match &e.kind {
            OpponentKind::Snapshot { name, .. } => Some(name.as_str()), _ => None }).collect();
        assert!(names.contains(&"sota") && names.contains(&"sp1term"));
    }
}

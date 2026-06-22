//! Ladder League: iterated best-response vs the FROZEN tournament-ladder pool.
//! Warm-start from SOTA, train PFSP against frozen opponents (terminal reward),
//! gate the candidate against a standing bar, promote only tournament-validated
//! winners, stop + declare the ceiling after K no-improve rounds.
//!
//! Additive — phase3/SP1/SP2 byte-unchanged. The ONE departure from SP1/SP2:
//! the pool is read-only within a round (no main-snapshot additions), which
//! removes the drift feedback loop those runs hit.

use std::path::PathBuf;

use anyhow::Result;
use crate::eval::{evaluate_hac, evaluate_h2h};
use crate::hierarchical::sizing::Sizing;
use crate::reward::RewardConfig;
use crate::self_play::{load_frozen_hac, OpponentKind, Role, SnapshotPool};
use crate::HierarchicalActorCritic;
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

/// Per-opponent winrate breakdown and aggregate pool score for a candidate brain.
#[derive(Clone, Debug)]
pub struct PoolScore {
    /// Mean worker-share winrate over all pool opponents (archetypes + non-excluded HAC snapshots).
    pub winrate_vs_pool: f32,
    /// Head-to-head worker-share winrate vs the entry named `sota_name` (0.5 if absent).
    pub h2h_vs_sota: f32,
    /// Per-opponent (name, winrate) pairs — one entry per archetype + one per included snapshot.
    pub per_opp: Vec<(String, f32)>,
}

/// Mean worker-share winrate of `hac` over every pool opponent (each archetype
/// and each HAC snapshot is one opponent), skipping the entry named
/// `exclude_name` (so a brain that lives in the pool doesn't score itself).
/// `h2h_vs_sota` is the winrate vs the entry named `sota_name` (0.5 if absent).
pub fn winrate_vs_pool(
    hac: &HierarchicalActorCritic,
    pool: &SnapshotPool,
    exclude_name: Option<&str>,
    sota_name: &str,
    device: &candle_core::Device,
    mpe: usize,
) -> Result<PoolScore> {
    // Archetypes in one shot via evaluate_hac (per-archetype worker-share).
    let bench = evaluate_hac(hac, device, mpe)?;
    let mut per_opp: Vec<(String, f32)> = bench.per_archetype.clone();
    let mut h2h_vs_sota = 0.5f32;

    for e in &pool.entries {
        if let OpponentKind::Snapshot { name, path } = &e.kind {
            if exclude_name == Some(name.as_str()) {
                continue;
            }
            let opp = load_frozen_hac(path, pool_sizing(pool), device)?;
            let r = evaluate_h2h(hac, &opp, device, mpe)?;
            if name == sota_name {
                h2h_vs_sota = r.a_winrate_ws;
            }
            per_opp.push((name.clone(), r.a_winrate_ws));
        }
    }

    let n = per_opp.len().max(1) as f32;
    let winrate_vs_pool = per_opp.iter().map(|(_, w)| *w).sum::<f32>() / n;
    tracing::info!(winrate_vs_pool, h2h_vs_sota, opponents = per_opp.len(), "ladder: winrate_vs_pool computed");
    Ok(PoolScore { winrate_vs_pool, h2h_vs_sota, per_opp })
}

/// All ladder HAC opponents are A1 (the project's compact target). Centralized
/// so `winrate_vs_pool` need not thread sizing through every call.
fn pool_sizing(_pool: &SnapshotPool) -> Sizing { crate::hierarchical::sizing::A1 }

/// Outcome of the two-part gate evaluation: candidate must meet BOTH the standing
/// bar (mean winrate-vs-pool) and the head-to-head margin over the current SOTA.
#[derive(Clone, Copy, Debug)]
pub struct GateOutcome {
    pub passed: bool,
    pub winrate_vs_pool: f32,
    pub h2h_vs_sota: f32,
}

/// PASS iff the candidate meets BOTH the standing bar (mean winrate-vs-pool) and
/// the head-to-head margin over the current SOTA. `>=` on both so exact-threshold
/// candidates promote.
pub fn gate_decision(winrate_vs_pool: f32, standing_bar: f32, h2h_vs_sota: f32, gate_margin: f32) -> bool {
    winrate_vs_pool >= standing_bar && h2h_vs_sota >= gate_margin
}

/// Evaluate the candidate against the frozen pool at the honest `mpe` and apply
/// the two-part pass test. The candidate is NOT in the pool, so nothing is
/// excluded.
pub fn gate(
    candidate: &HierarchicalActorCritic,
    pool: &SnapshotPool,
    sota_name: &str,
    standing_bar: f32,
    gate_margin: f32,
    device: &candle_core::Device,
    mpe: usize,
) -> Result<GateOutcome> {
    let score = winrate_vs_pool(candidate, pool, None, sota_name, device, mpe)?;
    let passed = gate_decision(score.winrate_vs_pool, standing_bar, score.h2h_vs_sota, gate_margin);
    tracing::info!(passed, score.winrate_vs_pool, standing_bar, score.h2h_vs_sota, gate_margin, "ladder: gate evaluated");
    Ok(GateOutcome { passed, winrate_vs_pool: score.winrate_vs_pool, h2h_vs_sota: score.h2h_vs_sota })
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

    #[test]
    fn winrate_vs_pool_excludes_self_and_reports_h2h() {
        use candle_core::Device;
        use crate::hierarchical::sizing::A1;
        let dir = std::env::temp_dir().join("ladder_wvp_test");
        std::fs::create_dir_all(&dir).unwrap();
        // Save two fresh HAC checkpoints to act as "sota" and "other".
        let sota_p = dir.join("sota.safetensors");
        let other_p = dir.join("other.safetensors");
        for p in [&sota_p, &other_p] {
            let t = crate::JointPpoTrainer::new(Device::Cpu, A1, crate::JointPpoConfig::smoke_default()).unwrap();
            t.varmap.save(p).unwrap();
        }
        let cs = vec![
            LadderContender { id: "sota".into(), spec: format!("hac:{}", sota_p.display()) },
            LadderContender { id: "other".into(), spec: format!("hac:{}", other_p.display()) },
        ];
        let pool = build_frozen_pool(&cs, 0.1);
        let cand = crate::self_play::load_frozen_hac(&sota_p, A1, &Device::Cpu).unwrap();
        // Evaluate "sota" itself vs pool, excluding its own entry: must NOT include a self-match.
        let score = winrate_vs_pool(&cand, &pool, Some("sota"), "sota", &Device::Cpu, 1).unwrap();
        // 7 archetypes + "other" = 8 opponents (self "sota" excluded).
        assert_eq!(score.per_opp.len(), 8, "self entry must be excluded");
        assert!(!score.per_opp.iter().any(|(n, _)| n == "sota"), "self not scored");
        assert!((0.0..=1.0).contains(&score.winrate_vs_pool));
        assert!((0.0..=1.0).contains(&score.h2h_vs_sota));
    }

    #[test]
    fn gate_decision_requires_both_bar_and_margin() {
        // bar=0.60, margin=0.55
        assert!(gate_decision(0.62, 0.60, 0.57, 0.55), "above bar AND clear h2h -> pass");
        assert!(!gate_decision(0.62, 0.60, 0.51, 0.55), "coin-flip h2h fails despite winrate");
        assert!(!gate_decision(0.58, 0.60, 0.70, 0.55), "below standing bar fails despite big h2h");
        assert!(gate_decision(0.60, 0.60, 0.55, 0.55), "exactly at both thresholds passes (>=)");
    }
}

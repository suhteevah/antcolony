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
use crate::self_play::{load_frozen_hac, OpponentKind, OpponentSampler, Role, SnapshotPool};
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

/// Outcome of one best-response training round.
#[derive(Clone, Debug)]
pub struct RoundOutcome {
    /// Path to the kept (or fallback-final) candidate checkpoint.
    pub candidate_path: PathBuf,
    /// Best h2h worker-share winrate vs SOTA seen during the round (or final if no checkpoint cleared the floor).
    pub best_train_h2h: f32,
    /// Archetype-bench mean winrate at the iteration that set `best_train_h2h` (or final if floor was never cleared).
    pub best_train_bench: f32,
    /// `true` iff at least one iteration cleared the `keepbest_arch_floor` and set a keep-best checkpoint.
    pub kept: bool,
}

/// Train one best-response round: warm-start from `sota_path`, PFSP against the
/// FROZEN pool, terminal reward. Keep-best on h2h-vs-SOTA gated by an archetype
/// floor. Never adds/removes pool entries (PFSP EMA updates only).
pub fn train_round(
    cfg: &LadderConfig,
    sota_path: &std::path::Path,
    pool: &mut SnapshotPool,
    round: usize,
    device: &candle_core::Device,
) -> Result<RoundOutcome> {
    let round_dir = cfg.out_dir.join(format!("round_{round:02}"));
    std::fs::create_dir_all(&round_dir)?;
    let candidate_path = round_dir.join("candidate.safetensors");

    // Fresh trainer warm-started from the current SOTA.
    let mut trainer = crate::JointPpoTrainer::new(device.clone(), cfg.sizing, cfg.joint.clone())?;
    trainer.varmap.load(sota_path)?;
    let mut opt = trainer.make_optimizer()?;
    tracing::info!(round, ?sota_path, "ladder: round trainer warm-started from SOTA");

    // ParallelEnv driven against the FROZEN pool via PFSP. n_envs from joint.matches_per_iter.
    let mut pe = crate::ParallelEnv::new(cfg.joint.matches_per_iter.max(1), cfg.joint.rollout_cycles);
    pe.self_play_enabled = true;
    pe.pool = pool.clone();                  // a working copy; the caller's set stays frozen
    pe.sampler = OpponentSampler::Pfsp { archetype_mix: cfg.archetype_mix, power: cfg.pfsp_power };
    pe.sizing = cfg.sizing;

    let sota_hac = load_frozen_hac(sota_path, cfg.sizing, device)?;

    let mut best_h2h = f32::NEG_INFINITY;
    let mut best_bench = 0.0f32;
    let mut kept = false;

    for it in 0..cfg.iters_per_round {
        let base_seed = round_seed(cfg.joint.seed, round, it);
        let roll = pe.collect_rollout(&trainer.hac, device, &mut trainer.rng, &cfg.reward, base_seed)?;
        // PFSP feedback (EMA only; opponent SET unchanged because we operate on the clone).
        pe.pool.record_result(pe.last_opponent_idx, pe.last_hac_winshare);
        let stats = trainer.joint_update(&mut opt, &roll)?;

        if cfg.eval_every > 0 && it % cfg.eval_every == 0 {
            let bench = evaluate_hac(&trainer.hac, device, cfg.train_mpe)?;
            let h2h = evaluate_h2h(&trainer.hac, &sota_hac, device, cfg.train_mpe)?;
            tracing::info!(round, it, loss = stats.total, bench = bench.mean_win_rate,
                           h2h = h2h.a_winrate_ws, "ladder: round eval");
            // Keep-best on h2h, ELIGIBLE only above the archetype floor.
            let eligible = bench.mean_win_rate >= cfg.keepbest_arch_floor;
            if eligible && h2h.a_winrate_ws > best_h2h {
                trainer.varmap.save(&candidate_path)?;
                best_h2h = h2h.a_winrate_ws;
                best_bench = bench.mean_win_rate;
                kept = true;
                tracing::info!(round, it, best_h2h, best_bench, "ladder: new round keep-best saved");
            }
        }
    }

    // If nothing ever cleared the floor, fall back to saving the final policy so
    // the gate still has a checkpoint to judge (it will simply fail the gate).
    if !kept {
        trainer.varmap.save(&candidate_path)?;
        let bench = evaluate_hac(&trainer.hac, device, cfg.train_mpe)?;
        let h2h = evaluate_h2h(&trainer.hac, &sota_hac, device, cfg.train_mpe)?;
        best_h2h = h2h.a_winrate_ws;
        best_bench = bench.mean_win_rate;
        tracing::warn!(round, best_h2h, best_bench, "ladder: no checkpoint cleared the floor; saved final policy");
    }

    Ok(RoundOutcome { candidate_path, best_train_h2h: best_h2h, best_train_bench: best_bench, kept })
}

pub struct LadderLeague {
    pub cfg: LadderConfig,
    pub pool: SnapshotPool,
    pub device: candle_core::Device,
    pub sota_path: PathBuf,
    pub standing_bar: f32,
}

impl LadderLeague {
    pub fn new(cfg: LadderConfig, device: candle_core::Device) -> Result<Self> {
        std::fs::create_dir_all(&cfg.out_dir)?;
        let pool = build_frozen_pool(&cfg.initial_contenders, 0.1);
        // Initial standing bar = the SOTA's winrate-vs-pool (excluding its own "sota" entry).
        let sota_hac = load_frozen_hac(&cfg.sota_path, cfg.sizing, &device)?;
        let bar = winrate_vs_pool(&sota_hac, &pool, Some("sota"), "sota", &device, cfg.gate_mpe)?;
        tracing::info!(standing_bar = bar.winrate_vs_pool, "ladder: initial standing bar computed");
        Ok(Self { sota_path: cfg.sota_path.clone(), standing_bar: bar.winrate_vs_pool, cfg, pool, device })
    }

    pub fn run(&mut self) -> Result<LadderReport> {
        let mut no_improve = 0usize;
        let mut promotions = 0usize;
        let mut best_h2h_over_seed = f32::NEG_INFINITY;
        let mut rounds_run = 0usize;

        for round in 1..=self.cfg.max_rounds {
            rounds_run = round;
            tracing::info!(round, standing_bar = self.standing_bar, sota_path = ?self.sota_path, "ladder: ===== round start =====");
            let outcome = train_round(&self.cfg, &self.sota_path, &mut self.pool, round, &self.device)?;
            let candidate = load_frozen_hac(&outcome.candidate_path, self.cfg.sizing, &self.device)?;
            let g = gate(&candidate, &self.pool, "sota", self.standing_bar,
                         self.cfg.gate_margin, &self.device, self.cfg.gate_mpe)?;
            best_h2h_over_seed = best_h2h_over_seed.max(g.h2h_vs_sota);

            if g.passed {
                let name = format!("ladder_r{round:02}");
                // Provisionally add so the confirmation tournament includes the candidate.
                self.pool.add_protected_snapshot(name.clone(), outcome.candidate_path.clone(), Role::Main);

                // Authoritative re-rank over the full current pool (snapshots + archetypes).
                let mut contenders: Vec<(String, String)> = Vec::new();
                for e in &self.pool.entries {
                    match &e.kind {
                        crate::self_play::OpponentKind::Snapshot { name: snap_name, path } =>
                            contenders.push((snap_name.clone(), format!("hac:{}", path.display()))),
                        crate::self_play::OpponentKind::Archetype(spec) =>
                            contenders.push((spec.clone(), spec.clone())),
                    }
                }
                let tcfg = crate::tournament::TournamentConfig {
                    contenders, mpe: self.cfg.gate_mpe, max_ticks: 10_000,
                    anchor_id: "heuristic".into(), anchor_elo: 1000.0, cycle_margin: 0.55, sizing: self.cfg.sizing,
                };
                let tres = crate::tournament::run_tournament(&tcfg, &self.device)?;
                match confirm_rank(&tres, &name) {
                    Some((0, elo, ncyc)) => {
                        self.sota_path = outcome.candidate_path.clone();
                        self.standing_bar = g.winrate_vs_pool;
                        promotions += 1;
                        no_improve = 0;
                        tracing::info!(round, %name, elo, cycles = ncyc, "ladder: PROMOTED + tournament-confirmed #1");
                        // (Task 7 sends the Telegram ping here.)
                    }
                    other => {
                        // Cheap gate and full re-rank disagree: roll back the provisional add.
                        if let Some(pos) = self.pool.entries.iter().position(|e| matches!(&e.kind,
                            crate::self_play::OpponentKind::Snapshot { name: n, .. } if *n == name)) {
                            self.pool.entries.remove(pos);
                        } else {
                            tracing::warn!(round, %name, "ladder: rollback could not find provisional entry — pool may be inconsistent");
                        }
                        no_improve += 1;
                        tracing::warn!(round, ?other, "ladder: gate passed but tournament did NOT rank candidate #1 -> rolled back");
                    }
                }
            } else {
                no_improve += 1;
                tracing::info!(round, no_improve, h2h = g.h2h_vs_sota, wr = g.winrate_vs_pool, "ladder: candidate did not pass the gate");
            }

            if let Some(reason) = should_stop(no_improve, self.cfg.no_improve_stop, round, self.cfg.max_rounds) {
                tracing::info!(round, reason, promotions, "ladder: ===== STOP =====");
                return Ok(LadderReport {
                    rounds_run, promotions, final_sota_path: self.sota_path.clone(),
                    best_h2h_over_seed, stopped_reason: reason.to_string(),
                });
            }
        }
        Ok(LadderReport {
            rounds_run, promotions, final_sota_path: self.sota_path.clone(),
            best_h2h_over_seed, stopped_reason: "max_rounds".to_string(),
        })
    }
}

/// Rank (0-based, by descending Elo), Elo, and cycle-count for `candidate_id`.
/// Returns `None` if `candidate_id` is absent from `result.ids`.
pub fn confirm_rank(result: &crate::tournament::TournamentResult, candidate_id: &str) -> Option<(usize, f64, usize)> {
    let idx = result.ids.iter().position(|id| id == candidate_id)?;
    let elo = result.elo[idx];
    let rank = result.elo.iter().filter(|&&e| e > elo).count(); // how many strictly above
    Some((rank, elo, result.cycles.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_play::OpponentKind;

    #[test]
    fn confirm_rank_finds_candidate_position_by_elo() {
        use crate::tournament::TournamentResult;
        let res = TournamentResult {
            ids: vec!["sota".into(), "cand".into(), "sp1".into()],
            specs: vec![],
            win_matrix: vec![], ws_matrix: vec![], games: vec![],
            elo: vec![1500.0, 1600.0, 1400.0],         // cand highest
            winrate_vs_field: vec![0.6, 0.8, 0.4],
            cycles: vec![(0,1,2)],
        };
        let (rank, elo, ncycles) = confirm_rank(&res, "cand").unwrap();
        assert_eq!(rank, 0, "highest Elo -> rank 0");
        assert_eq!(elo, 1600.0);
        assert_eq!(ncycles, 1);
        assert_eq!(confirm_rank(&res, "sota").unwrap().0, 1, "sota at 1500 is rank 1 (only cand at 1600 is above)");
        assert!(confirm_rank(&res, "ghost").is_none());
    }

    #[test]
    fn train_round_smoke_keeps_a_candidate_and_does_not_change_pool_set() {
        use candle_core::Device;
        use crate::hierarchical::sizing::A1;
        let dir = std::env::temp_dir().join("ladder_train_round_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sota_p = dir.join("sota.safetensors");
        { let t = crate::JointPpoTrainer::new(Device::Cpu, A1, crate::JointPpoConfig::smoke_default()).unwrap();
          t.varmap.save(&sota_p).unwrap(); }
        let cs = vec![LadderContender { id: "sota".into(), spec: format!("hac:{}", sota_p.display()) }];
        let mut pool = build_frozen_pool(&cs, 0.1);
        let set_before: Vec<String> = pool.entries.iter().map(|e| format!("{:?}", e.kind)).collect();

        let mut joint = crate::JointPpoConfig::smoke_default();
        joint.rollout_cycles = 4;
        let cfg = LadderConfig {
            sota_path: sota_p.clone(),
            initial_contenders: cs.clone(),
            iters_per_round: 2, eval_every: 1, train_mpe: 1, gate_mpe: 1,
            gate_margin: 0.55, keepbest_arch_floor: 0.0, // floor 0.0 so the keep can't be blocked in the smoke
            archetype_mix: 0.5, pfsp_power: 1.0, no_improve_stop: 2, max_rounds: 8,
            out_dir: dir.clone(), sizing: A1, joint, reward: crate::RewardConfig::default(),
        };
        let outcome = train_round(&cfg, &sota_p, &mut pool, 1, &Device::Cpu).unwrap();
        assert!(outcome.candidate_path.exists(), "kept candidate checkpoint must exist on disk");
        assert!(outcome.kept, "with floor=0.0 a candidate is always eligible to be kept");
        // Frozen-set invariant: the opponent SET (kinds) is unchanged (EMA may differ).
        let set_after: Vec<String> = pool.entries.iter().map(|e| format!("{:?}", e.kind)).collect();
        assert_eq!(set_before, set_after, "train_round must not add/remove pool entries");
    }

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

    #[test]
    fn ladder_league_runs_two_rounds_and_reports() {
        use candle_core::Device;
        use crate::hierarchical::sizing::A1;
        let dir = std::env::temp_dir().join("ladder_league_smoke");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sota_p = dir.join("seed_sota.safetensors");
        { let t = crate::JointPpoTrainer::new(Device::Cpu, A1, crate::JointPpoConfig::smoke_default()).unwrap();
          t.varmap.save(&sota_p).unwrap(); }
        let cs = vec![LadderContender { id: "sota".into(), spec: format!("hac:{}", sota_p.display()) }];

        let mut joint = crate::JointPpoConfig::smoke_default();
        joint.rollout_cycles = 4;
        let cfg = LadderConfig {
            sota_path: sota_p.clone(), initial_contenders: cs,
            iters_per_round: 1, eval_every: 1, train_mpe: 1, gate_mpe: 1,
            gate_margin: 2.0,                 // impossible margin -> guarantees NO promotion -> stops on no_improve
            keepbest_arch_floor: 0.0, archetype_mix: 0.5, pfsp_power: 1.0,
            no_improve_stop: 2, max_rounds: 8,
            out_dir: dir.clone(), sizing: A1, joint, reward: crate::RewardConfig::default(),
        };
        let mut league = LadderLeague::new(cfg, Device::Cpu).unwrap();
        assert!((0.0..=1.0).contains(&league.standing_bar), "initial bar is a winrate");
        let report = league.run().unwrap();
        assert_eq!(report.promotions, 0, "impossible margin -> no promotions");
        assert_eq!(report.stopped_reason, "no_improve");
        assert_eq!(report.rounds_run, 2, "stops after no_improve_stop=2 failed rounds");
    }
}

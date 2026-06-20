//! Deterministic evaluation of the HAC against the 7-archetype bench — the
//! same opponents and metric used to measure MlpBrain v1 at 47.1%. The HAC
//! plays the left colony with policy-MEAN actions (no sampling); each
//! archetype plays the right. Win = left win (1.0), loss (0.0), draw (0.5),
//! timeout graded by worker share.

use anyhow::Result;
use candle_core::Device;

use antcolony_sim::ai::observation::AntModulators;
use antcolony_sim::MatchStatus;

use crate::env::{MatchEnv, DECISION_CADENCE};
use crate::hierarchical::obs_to_tensors::{ant_obs_to_tensors, rich_to_tensors};
use crate::HierarchicalActorCritic;
use crate::League;

/// The 7 fixed archetype specs that define the bench (== League::default_pool).
pub const BENCH_ARCHETYPES: [&str; 7] = [
    "heuristic", "defender", "aggressor", "economist", "breeder", "forager", "conservative",
];

#[derive(Clone, Debug)]
pub struct EvalReport {
    /// eval.rs metric: timeout graded by worker-share (the harness that measured
    /// MlpBrain v1 ~0.517 and the combat HAC 0.871).
    pub per_archetype: Vec<(String, f32)>,
    pub mean_win_rate: f32,
    /// Decisive-win metric: a timeout is a DRAW (0.5) — only an actual queen-kill
    /// counts. The harder, matchup_bench-comparable number (v1 ~0.50).
    pub per_archetype_decisive: Vec<(String, f32)>,
    pub mean_decisive_rate: f32,
    /// How matches ended, summed across all archetypes (so worker-share wins can
    /// be told apart from actual kills).
    pub outcomes: OutcomeCounts,
}

/// How a single match ended, distinguishing an actual win (queen-kill) from a
/// timeout graded by worker share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchEnd {
    WonLeft,
    WonRight,
    Draw,
    TimeoutLeftMajority,
    TimeoutRightMajority,
    TimeoutEven,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutcomeCounts {
    pub won_left: usize,
    pub won_right: usize,
    pub draw: usize,
    pub timeout_left: usize,
    pub timeout_right: usize,
    pub timeout_even: usize,
}

impl OutcomeCounts {
    fn add(&mut self, end: MatchEnd) {
        match end {
            MatchEnd::WonLeft => self.won_left += 1,
            MatchEnd::WonRight => self.won_right += 1,
            MatchEnd::Draw => self.draw += 1,
            MatchEnd::TimeoutLeftMajority => self.timeout_left += 1,
            MatchEnd::TimeoutRightMajority => self.timeout_right += 1,
            MatchEnd::TimeoutEven => self.timeout_even += 1,
        }
    }

    fn merge(&mut self, o: &OutcomeCounts) {
        self.won_left += o.won_left;
        self.won_right += o.won_right;
        self.draw += o.draw;
        self.timeout_left += o.timeout_left;
        self.timeout_right += o.timeout_right;
        self.timeout_even += o.timeout_even;
    }
}

/// Score a finished match under BOTH metrics from its terminal status + worker
/// counts. Returns `(worker_share, decisive, how_it_ended)`.
/// - `worker_share`: a timeout is graded by which colony has more workers (the
///   eval.rs metric — flatters, since most matches time out near-even).
/// - `decisive`: a timeout is a DRAW (0.5); only an actual `Won` (queen-kill)
///   scores 1/0. This is the matchup_bench-comparable "real win" metric.
/// The `worker_share` branch is byte-identical to the prior `play_match` scoring
/// so the validated 0.871 reproduces exactly.
fn score_match(status: MatchStatus, lw: f32, rw: f32) -> (f32, f32, MatchEnd) {
    match status {
        MatchStatus::Won { winner: 0, .. } => (1.0, 1.0, MatchEnd::WonLeft),
        MatchStatus::Won { winner: 1, .. } => (0.0, 0.0, MatchEnd::WonRight),
        MatchStatus::Draw { .. } => (0.5, 0.5, MatchEnd::Draw),
        MatchStatus::InProgress => {
            // L9: 0-vs-0 timeout is a draw under both metrics.
            if lw == 0.0 && rw == 0.0 {
                return (0.5, 0.5, MatchEnd::TimeoutEven);
            }
            let share = lw / (lw + rw).max(1.0);
            if share > 0.5 {
                (1.0, 0.5, MatchEnd::TimeoutLeftMajority)
            } else if share < 0.5 {
                (0.0, 0.5, MatchEnd::TimeoutRightMajority)
            } else {
                (0.5, 0.5, MatchEnd::TimeoutEven)
            }
        }
        _ => (0.5, 0.5, MatchEnd::Draw),
    }
}

fn play_match(
    hac: &HierarchicalActorCritic,
    device: &Device,
    opp_spec: &str,
    seed: u64,
) -> Result<(f32, f32, MatchEnd)> {
    let mut env = MatchEnv::new(seed);
    let mut opp = League::make_brain(opp_spec, seed.wrapping_add(1))?;

    loop {
        let rich = match env.sim.colony_rich_observation(0) {
            Some(r) => r,
            None => break,
        };
        let (s, p, h) = rich_to_tensors(&rich, device)?;
        let (action, intent, _value) = hac.mean_commander_action(&s, &p, &h)?;
        let av: Vec<f32> = action.flatten_all()?.to_vec1()?;
        // L11: document the 6-dim action invariant (can't fire — dims pinned).
        debug_assert_eq!(av.len(), 6, "commander action expects 6 dims");
        let dec = antcolony_sim::AiDecision {
            caste_ratio_worker: av[0], caste_ratio_soldier: av[1], caste_ratio_breeder: av[2],
            forage_weight: av[3], dig_weight: av[4], nurse_weight: av[5], research_choice: None,
        };
        env.sim.apply_ai_decision(0, &dec);
        let iv: Vec<f32> = intent.flatten_all()?.to_vec1()?;
        let mut intent_arr = [0.0f32; 64];
        intent_arr.copy_from_slice(&iv);
        env.sim.apply_commander_intent(0, &intent_arr);
        if let Some(sr) = env.sim.colony_ai_state(1) {
            let dr = opp.decide(&sr);
            env.sim.apply_ai_decision(1, &dr);
        }

        let mut done = false;
        for _ in 0..DECISION_CADENCE {
            let obs = env.sim.per_ant_observations(0);
            if !obs.is_empty() {
                let (cone, internal, intent_b) = ant_obs_to_tensors(&obs, &intent, device)?;
                let mods_t = hac.mean_ant_modulator(&cone, &internal, &intent_b)?;
                let flat: Vec<f32> = mods_t.flatten_all()?.to_vec1()?;
                let mut mods = Vec::with_capacity(obs.len());
                let mut ids = Vec::with_capacity(obs.len());
                for (k, o) in obs.iter().enumerate() {
                    let off = k * 5;
                    mods.push(AntModulators {
                        alpha_mult: flat[off], beta_mult: flat[off + 1], exploration_mod: flat[off + 2],
                        deposit_mult: flat[off + 3], state_bias: flat[off + 4],
                    });
                    ids.push(o.ant_id);
                }
                env.sim.apply_ant_modulators(0, &mods, &ids);
            }
            env.sim.tick();
            if !matches!(env.sim.match_status(), MatchStatus::InProgress)
                || env.sim.tick >= env.max_ticks
            {
                done = true;
                break;
            }
        }
        if done {
            break;
        }
    }

    let lw = env.sim.colonies.first().map(|c| c.population.workers).unwrap_or(0) as f32;
    let rw = env.sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0) as f32;
    Ok(score_match(env.sim.match_status(), lw, rw))
}

/// L10: deterministic per-archetype seed salt. Folds a byte hash of the spec
/// (FNV-1a-ish) into the multiplier so same-LENGTH names (breeder/forager,
/// aggressor/economist/heuristic) no longer share sim seeds. Always non-zero
/// (`| 1`) so the seed multiplier never collapses to 0.
fn spec_seed_salt(spec: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in spec.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h | 1
}

pub fn evaluate_hac(
    hac: &HierarchicalActorCritic,
    device: &Device,
    matches_per_opp: usize,
) -> Result<EvalReport> {
    use rayon::prelude::*;
    // The 7 archetypes' matches are independent and deterministically seeded, so
    // fan them across CPU cores (the sim is CPU-bound and the loop was otherwise
    // single-threaded). Identical seeds => identical per-match results, so the
    // means are byte-for-byte what the sequential version produced — this only
    // changes speed, not the numbers. `par_iter().map().collect()` keeps order.
    let rows: Vec<(String, f32, f32, OutcomeCounts)> = BENCH_ARCHETYPES
        .par_iter()
        .map(|&spec| {
            let mut score = 0.0f32;
            let mut decisive = 0.0f32;
            let mut played = 0usize;
            let mut counts = OutcomeCounts::default();
            for m in 0..matches_per_opp {
                // L10: fold a per-archetype byte hash into the seed so same-length
                // archetype names (breeder/forager) don't share sim seeds.
                let seed = 0xE7A1_u64
                    .wrapping_mul(spec_seed_salt(spec))
                    ^ ((m as u64).wrapping_mul(0x9E3779B97F4A7C15));
                // M16: a single bad match shouldn't abort the eval — log+skip.
                match play_match(hac, device, spec, seed) {
                    Ok((ws, dec, end)) => { score += ws; decisive += dec; played += 1; counts.add(end); }
                    Err(e) => tracing::error!(archetype = spec, m, error = %e, "eval match failed; skipping"),
                }
            }
            let denom = played.max(1) as f32;
            let wr = score / denom;
            let dwr = decisive / denom;
            tracing::info!(archetype = spec, win_rate = wr, decisive_rate = dwr, played, "eval vs archetype");
            (spec.to_string(), wr, dwr, counts)
        })
        .collect();

    let mut per_archetype = Vec::with_capacity(rows.len());
    let mut per_archetype_decisive = Vec::with_capacity(rows.len());
    let mut outcomes = OutcomeCounts::default();
    for (name, wr, dwr, counts) in &rows {
        per_archetype.push((name.clone(), *wr));
        per_archetype_decisive.push((name.clone(), *dwr));
        outcomes.merge(counts);
    }
    let n = per_archetype.len().max(1) as f32;
    let mean = per_archetype.iter().map(|(_, w)| *w).sum::<f32>() / n;
    let mean_decisive = per_archetype_decisive.iter().map(|(_, w)| *w).sum::<f32>() / n;
    Ok(EvalReport {
        per_archetype,
        mean_win_rate: mean,
        per_archetype_decisive,
        mean_decisive_rate: mean_decisive,
        outcomes,
    })
}

/// Diagnostic eval: same play loop as `play_match`, but logs WHAT the HAC
/// commands (mean caste ratios — does it ever build soldiers?) and HOW the
/// match ends (final caste counts per colony, queen health, win/loss/timeout).
/// Answers "why does A1 lose to combat archetypes": is it failing to build
/// soldiers (defense), getting its queen killed (wiped), or losing the worker
/// race at timeout (economy)? Logs one line per match at INFO.
pub fn evaluate_hac_introspect(
    hac: &HierarchicalActorCritic,
    device: &Device,
    opp_spec: &str,
    matches: usize,
) -> Result<()> {
    for m in 0..matches {
        let seed = 0xE7A1_u64
            .wrapping_mul(spec_seed_salt(opp_spec))
            ^ ((m as u64).wrapping_mul(0x9E3779B97F4A7C15));
        let mut env = MatchEnv::new(seed);
        let mut opp = League::make_brain(opp_spec, seed.wrapping_add(1))?;
        let (mut sum_w, mut sum_s, mut sum_b, mut nd) = (0.0f32, 0.0f32, 0.0f32, 0u32);

        loop {
            let rich = match env.sim.colony_rich_observation(0) {
                Some(r) => r,
                None => break,
            };
            let (s, p, h) = rich_to_tensors(&rich, device)?;
            let (action, intent, _value) = hac.mean_commander_action(&s, &p, &h)?;
            let av: Vec<f32> = action.flatten_all()?.to_vec1()?;
            sum_w += av[0];
            sum_s += av[1];
            sum_b += av[2];
            nd += 1;
            let dec = antcolony_sim::AiDecision {
                caste_ratio_worker: av[0], caste_ratio_soldier: av[1], caste_ratio_breeder: av[2],
                forage_weight: av[3], dig_weight: av[4], nurse_weight: av[5], research_choice: None,
            };
            env.sim.apply_ai_decision(0, &dec);
            let iv: Vec<f32> = intent.flatten_all()?.to_vec1()?;
            let mut intent_arr = [0.0f32; 64];
            intent_arr.copy_from_slice(&iv);
            env.sim.apply_commander_intent(0, &intent_arr);
            if let Some(sr) = env.sim.colony_ai_state(1) {
                let dr = opp.decide(&sr);
                env.sim.apply_ai_decision(1, &dr);
            }

            let mut done = false;
            for _ in 0..DECISION_CADENCE {
                let obs = env.sim.per_ant_observations(0);
                if !obs.is_empty() {
                    let (cone, internal, intent_b) = ant_obs_to_tensors(&obs, &intent, device)?;
                    let mods_t = hac.mean_ant_modulator(&cone, &internal, &intent_b)?;
                    let flat: Vec<f32> = mods_t.flatten_all()?.to_vec1()?;
                    let mut mods = Vec::with_capacity(obs.len());
                    let mut ids = Vec::with_capacity(obs.len());
                    for (k, o) in obs.iter().enumerate() {
                        let off = k * 5;
                        mods.push(AntModulators {
                            alpha_mult: flat[off], beta_mult: flat[off + 1], exploration_mod: flat[off + 2],
                            deposit_mult: flat[off + 3], state_bias: flat[off + 4],
                        });
                        ids.push(o.ant_id);
                    }
                    env.sim.apply_ant_modulators(0, &mods, &ids);
                }
                env.sim.tick();
                if !matches!(env.sim.match_status(), MatchStatus::InProgress)
                    || env.sim.tick >= env.max_ticks
                {
                    done = true;
                    break;
                }
            }
            if done {
                break;
            }
        }

        let lw = env.sim.colonies.first().map(|c| c.population.workers).unwrap_or(0);
        let ls = env.sim.colonies.first().map(|c| c.population.soldiers).unwrap_or(0);
        let lq = env.sim.colonies.first().map(|c| c.queen_health).unwrap_or(0.0);
        let rw = env.sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0);
        let rs = env.sim.colonies.get(1).map(|c| c.population.soldiers).unwrap_or(0);
        let denom = nd.max(1) as f32;
        let status = env.sim.match_status();
        tracing::info!(
            opp = opp_spec, m,
            end_tick = env.sim.tick,
            ?status,
            cmd_soldier = sum_s / denom, cmd_worker = sum_w / denom, cmd_breeder = sum_b / denom,
            left_workers = lw, left_soldiers = ls, left_queen = lq,
            right_workers = rw, right_soldiers = rs,
            "introspect match"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchical::sizing::A1;
    use candle_core::{DType, Device};
    use candle_nn::{VarBuilder, VarMap};

    #[test]
    fn evaluate_hac_one_match_per_opp_produces_valid_rates() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
        let report = evaluate_hac(&hac, &device, 1).unwrap();
        assert_eq!(report.per_archetype.len(), 7);
        assert_eq!(report.per_archetype_decisive.len(), 7);
        for (name, wr) in &report.per_archetype {
            assert!(!name.is_empty());
            assert!((0.0..=1.0).contains(wr), "{name} win-rate out of range: {wr}");
        }
        for (name, dr) in &report.per_archetype_decisive {
            assert!((0.0..=1.0).contains(dr), "{name} decisive-rate out of range: {dr}");
        }
        assert!((0.0..=1.0).contains(&report.mean_win_rate));
        assert!((0.0..=1.0).contains(&report.mean_decisive_rate));
        // Every match counted exactly once across the outcome buckets (7 opps × 1).
        let o = &report.outcomes;
        let total = o.won_left + o.won_right + o.draw
            + o.timeout_left + o.timeout_right + o.timeout_even;
        assert_eq!(total, 7, "every played match must land in exactly one outcome bucket");
    }

    #[test]
    fn score_match_worker_share_vs_decisive() {
        use antcolony_sim::MatchStatus::*;
        // Decisive win/loss: both metrics agree, counted as a real kill.
        assert_eq!(
            score_match(Won { winner: 0, loser: 1, ended_at_tick: 9 }, 5.0, 0.0),
            (1.0, 1.0, MatchEnd::WonLeft)
        );
        assert_eq!(
            score_match(Won { winner: 1, loser: 0, ended_at_tick: 9 }, 0.0, 5.0),
            (0.0, 0.0, MatchEnd::WonRight)
        );
        // THE KEY CASE: a timeout where left out-grew right.
        // worker-share metric scores it a WIN (1.0); decisive metric a DRAW (0.5).
        assert_eq!(
            score_match(InProgress, 60.0, 40.0),
            (1.0, 0.5, MatchEnd::TimeoutLeftMajority)
        );
        assert_eq!(
            score_match(InProgress, 40.0, 60.0),
            (0.0, 0.5, MatchEnd::TimeoutRightMajority)
        );
        // Even / 0-vs-0 timeout: draw under both.
        assert_eq!(score_match(InProgress, 50.0, 50.0), (0.5, 0.5, MatchEnd::TimeoutEven));
        assert_eq!(score_match(InProgress, 0.0, 0.0), (0.5, 0.5, MatchEnd::TimeoutEven));
    }
}

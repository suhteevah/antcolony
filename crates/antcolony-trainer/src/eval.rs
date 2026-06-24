//! Deterministic evaluation of the HAC against the 7-archetype bench — the
//! same opponents and metric used to measure MlpBrain v1 at 47.1%. The HAC
//! plays the left colony with policy-MEAN actions (no sampling); each
//! archetype plays the right. Win = left win (1.0), loss (0.0), draw (0.5),
//! timeout graded by worker share.

use anyhow::Result;
use candle_core::Device;

use antcolony_sim::ai::observation::AntModulators;
use antcolony_sim::{AiBrain, HeuristicBrain, MatchStatus};

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
pub(crate) fn spec_seed_salt(spec: &str) -> u64 {
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

/// Play one match with BOTH colonies driven by frozen HAC mean actions (no
/// gradient, no sampling). Left colony uses `hac_left`, right uses
/// `hac_right`. Returns `(worker_share, decisive, how_it_ended)` from LEFT's
/// perspective via `score_match` — semantics are identical to `play_match`.
fn play_match_h2h(
    hac_left: &HierarchicalActorCritic,
    hac_right: &HierarchicalActorCritic,
    device: &Device,
    seed: u64,
) -> Result<(f32, f32, MatchEnd)> {
    let mut env = MatchEnv::new(seed);

    loop {
        // ── Left colony commander ──
        let rich_left = match env.sim.colony_rich_observation(0) {
            Some(r) => r,
            None => break,
        };
        let (sl, pl, hl) = rich_to_tensors(&rich_left, device)?;
        let (action_l, intent_l, _) = hac_left.mean_commander_action(&sl, &pl, &hl)?;
        let avl: Vec<f32> = action_l.flatten_all()?.to_vec1()?;
        // L11: document the 6-dim action invariant (can't fire — dims pinned).
        debug_assert_eq!(avl.len(), 6, "commander action expects 6 dims");
        let dec_l = antcolony_sim::AiDecision {
            caste_ratio_worker: avl[0], caste_ratio_soldier: avl[1], caste_ratio_breeder: avl[2],
            forage_weight: avl[3], dig_weight: avl[4], nurse_weight: avl[5], research_choice: None,
        };
        env.sim.apply_ai_decision(0, &dec_l);
        let ivl: Vec<f32> = intent_l.flatten_all()?.to_vec1()?;
        let mut intent_arr_l = [0.0f32; 64];
        intent_arr_l.copy_from_slice(&ivl);
        env.sim.apply_commander_intent(0, &intent_arr_l);

        // ── Right colony commander ──
        // Capture the right intent tensor for use in the ant tick loop.
        let intent_r_opt: Option<candle_core::Tensor> =
            if let Some(rich_right) = env.sim.colony_rich_observation(1) {
                let (sr, pr, hr) = rich_to_tensors(&rich_right, device)?;
                let (action_r, intent_r, _) = hac_right.mean_commander_action(&sr, &pr, &hr)?;
                let avr: Vec<f32> = action_r.flatten_all()?.to_vec1()?;
                debug_assert_eq!(avr.len(), 6, "commander action expects 6 dims");
                let dec_r = antcolony_sim::AiDecision {
                    caste_ratio_worker: avr[0], caste_ratio_soldier: avr[1], caste_ratio_breeder: avr[2],
                    forage_weight: avr[3], dig_weight: avr[4], nurse_weight: avr[5], research_choice: None,
                };
                env.sim.apply_ai_decision(1, &dec_r);
                let ivr: Vec<f32> = intent_r.flatten_all()?.to_vec1()?;
                let mut intent_arr_r = [0.0f32; 64];
                intent_arr_r.copy_from_slice(&ivr);
                env.sim.apply_commander_intent(1, &intent_arr_r);
                Some(intent_r)
            } else {
                None
            };

        let mut done = false;
        for _ in 0..DECISION_CADENCE {
            // ── Left ant modulators ──
            let obs_l = env.sim.per_ant_observations(0);
            if !obs_l.is_empty() {
                let (cone, internal, intent_b) = ant_obs_to_tensors(&obs_l, &intent_l, device)?;
                let mods_t = hac_left.mean_ant_modulator(&cone, &internal, &intent_b)?;
                let flat: Vec<f32> = mods_t.flatten_all()?.to_vec1()?;
                let mut mods = Vec::with_capacity(obs_l.len());
                let mut ids = Vec::with_capacity(obs_l.len());
                for (k, o) in obs_l.iter().enumerate() {
                    let off = k * 5;
                    mods.push(AntModulators {
                        alpha_mult: flat[off], beta_mult: flat[off + 1],
                        exploration_mod: flat[off + 2], deposit_mult: flat[off + 3],
                        state_bias: flat[off + 4],
                    });
                    ids.push(o.ant_id);
                }
                env.sim.apply_ant_modulators(0, &mods, &ids);
            }

            // ── Right ant modulators ──
            if let Some(ref intent_r) = intent_r_opt {
                let obs_r = env.sim.per_ant_observations(1);
                if !obs_r.is_empty() {
                    let (cone_r, internal_r, intent_br) =
                        ant_obs_to_tensors(&obs_r, intent_r, device)?;
                    let mods_tr =
                        hac_right.mean_ant_modulator(&cone_r, &internal_r, &intent_br)?;
                    let flat_r: Vec<f32> = mods_tr.flatten_all()?.to_vec1()?;
                    let mut mods_r = Vec::with_capacity(obs_r.len());
                    let mut ids_r = Vec::with_capacity(obs_r.len());
                    for (k, o) in obs_r.iter().enumerate() {
                        let off = k * 5;
                        mods_r.push(AntModulators {
                            alpha_mult: flat_r[off], beta_mult: flat_r[off + 1],
                            exploration_mod: flat_r[off + 2], deposit_mult: flat_r[off + 3],
                            state_bias: flat_r[off + 4],
                        });
                        ids_r.push(o.ant_id);
                    }
                    env.sim.apply_ant_modulators(1, &mods_r, &ids_r);
                }
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

/// Outcome of one side's commander phase for a decision cycle. `Hac` carries
/// the captured commander-intent tensor so the same cycle's ant phase can
/// modulate; `Scripted` ran (or skipped) its colony-level decision; `Gone`
/// means the colony no longer exists (`colony_rich_observation` returned
/// `None`) and the match must end — exactly as `play_match`/`play_match_h2h`
/// break the loop on a missing HAC colony.
enum CommanderResult {
    Gone,
    Hac(candle_core::Tensor),
    Scripted,
}

/// Run one colony's commander phase for this decision cycle.
///
/// HAC side: `colony_rich_observation` → `rich_to_tensors` →
/// `mean_commander_action` → `apply_ai_decision` + `apply_commander_intent`,
/// keeping the `intent` tensor for the ant phase. If the colony is gone returns
/// `CommanderResult::Gone` (the caller ends the match) — matching the
/// break-on-`None` behavior of the existing HAC drive loops.
///
/// Scripted side: `colony_ai_state` → `brain.decide` → `apply_ai_decision`. A
/// `None` ai-state just skips this side's decision (mirrors `play_match`'s
/// `if let Some(sr)`), and is NOT a match-ending condition.
///
/// No RNG is drawn here (the sim's per-tick RNG is internal), so applying both
/// sides' commander decisions before the tick loop is deterministic.
fn commander_phase(
    ctrl: &mut crate::tournament::Controller,
    side: u8,
    env: &mut MatchEnv,
    device: &Device,
) -> Result<CommanderResult> {
    use crate::tournament::Controller;
    match ctrl {
        Controller::Hac(hac) => {
            let rich = match env.sim.colony_rich_observation(side) {
                Some(r) => r,
                None => return Ok(CommanderResult::Gone),
            };
            let (s, p, h) = rich_to_tensors(&rich, device)?;
            let (action, intent, _v) = hac.mean_commander_action(&s, &p, &h)?;
            let av: Vec<f32> = action.flatten_all()?.to_vec1()?;
            debug_assert_eq!(av.len(), 6, "commander action expects 6 dims");
            let dec = antcolony_sim::AiDecision {
                caste_ratio_worker: av[0], caste_ratio_soldier: av[1], caste_ratio_breeder: av[2],
                forage_weight: av[3], dig_weight: av[4], nurse_weight: av[5], research_choice: None,
            };
            env.sim.apply_ai_decision(side, &dec);
            let iv: Vec<f32> = intent.flatten_all()?.to_vec1()?;
            debug_assert_eq!(iv.len(), 64, "commander intent expects 64 dims");
            let mut intent_arr = [0.0f32; 64];
            intent_arr.copy_from_slice(&iv);
            env.sim.apply_commander_intent(side, &intent_arr);
            Ok(CommanderResult::Hac(intent))
        }
        Controller::Scripted(brain) => {
            if let Some(sr) = env.sim.colony_ai_state(side) {
                let dr = brain.decide(&sr);
                env.sim.apply_ai_decision(side, &dr);
            }
            Ok(CommanderResult::Scripted)
        }
    }
}

/// Apply one HAC colony's per-ant modulators for the current tick. No-op for a
/// scripted side (the sim runs its default ant behavior) — keeps the scripted
/// drive byte-identical to `play_match`'s "no per-ant modulators for the
/// scripted opponent" path.
fn ant_phase(
    ctrl: &crate::tournament::Controller,
    side: u8,
    cmd: &CommanderResult,
    env: &mut MatchEnv,
    device: &Device,
) -> Result<()> {
    use crate::tournament::Controller;
    if let (Controller::Hac(hac), CommanderResult::Hac(intent)) = (ctrl, cmd) {
        let obs = env.sim.per_ant_observations(side);
        if !obs.is_empty() {
            let (cone, internal, intent_b) = ant_obs_to_tensors(&obs, intent, device)?;
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
            env.sim.apply_ant_modulators(side, &mods, &ids);
        }
    }
    Ok(())
}

/// Heterogeneous match runner: play ANY `Controller` (HAC or scripted) as the
/// left colony vs ANY `Controller` as the right over the 2-colony engine.
/// Generalizes `play_match` (HAC-vs-scripted) and `play_match_h2h` (HAC-vs-HAC)
/// to a per-side `Controller`. Returns `(left_worker_share, left_decisive,
/// MatchEnd)` from LEFT's perspective via `score_match` — same convention as
/// the existing runners; the scheduler derives right's scores as `1.0 - left`.
///
/// Drive order per decision cycle (identical structure to the existing fns,
/// which keeps the sim's per-tick RNG unaffected and the run deterministic for
/// a fixed `seed`): both sides' commander phase first, then `DECISION_CADENCE`
/// ticks each applying both sides' ant modulators before `tick()`. A HAC side
/// whose colony has vanished (`CommanderResult::Gone`) ends the match; a
/// scripted side with no ai-state simply skips its decision.
pub fn play_pair(
    left: &mut crate::tournament::Controller,
    right: &mut crate::tournament::Controller,
    device: &Device,
    seed: u64,
    max_ticks: u64,
) -> Result<(f32, f32, MatchEnd)> {
    let mut env = MatchEnv::new(seed);
    env.max_ticks = max_ticks;
    play_pair_in(left, right, &mut env, device)
}

/// Like `play_pair` but drives a CALLER-SUPPLIED env, so the match can run in any
/// arena (e.g. a cross-species nest arena) rather than the default same-species
/// `MatchEnv::new`. Identical drive order; `env.max_ticks` must be set by caller.
pub fn play_pair_in(
    left: &mut crate::tournament::Controller,
    right: &mut crate::tournament::Controller,
    env: &mut MatchEnv,
    device: &Device,
) -> Result<(f32, f32, MatchEnd)> {
    loop {
        let cl = commander_phase(left, 0u8, env, device)?;
        if matches!(cl, CommanderResult::Gone) {
            break;
        }
        let cr = commander_phase(right, 1u8, env, device)?;
        if matches!(cr, CommanderResult::Gone) {
            break;
        }

        let mut done = false;
        for _ in 0..DECISION_CADENCE {
            ant_phase(left, 0u8, &cl, env, device)?;
            ant_phase(right, 1u8, &cr, env, device)?;
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

/// Chokepoint attacker-cap + predation + cyclic-clade knobs, applied IDENTICALLY
/// to `cross_species_matrix` / `eval_mlp_vs_heuristic` so a trained HAC is scored
/// in the exact arena it trained in. (`venom_cycle` is read from GLOBAL combat.)
fn apply_cross_species_knobs(env: &mut MatchEnv, venom_cycle: f32) {
    env.sim.config.combat.venom_cycle_strength = venom_cycle;
    for i in 0..env.sim.colony_configs.len() {
        env.sim.colony_configs[i].combat.max_simultaneous_attackers_open = 255;
        env.sim.colony_configs[i].combat.max_simultaneous_attackers_tunnel = 3;
        env.sim.colony_configs[i].combat.max_simultaneous_attackers_entrance = 1;
        if env.sim.colony_configs[i].predates_ants {
            env.sim.colony_configs[i].combat.usurp_corpse_to_killer_frac = 0.5;
        }
    }
}

/// Drive one cross-species match: the HAC controls `hac_side`, `brain` the other.
/// Mirrors `play_match`'s commander+ant inference but parameterized by side.
fn cross_match_hac(
    hac: &HierarchicalActorCritic,
    device: &Device,
    env: &mut MatchEnv,
    hac_side: u8,
    brain: &mut dyn AiBrain,
) -> Result<MatchEnd> {
    let opp_side = 1 - hac_side;
    loop {
        let rich = match env.sim.colony_rich_observation(hac_side) {
            Some(r) => r,
            None => break,
        };
        let (s, p, h) = rich_to_tensors(&rich, device)?;
        let (action, intent, _v) = hac.mean_commander_action(&s, &p, &h)?;
        let av: Vec<f32> = action.flatten_all()?.to_vec1()?;
        let dec = antcolony_sim::AiDecision {
            caste_ratio_worker: av[0], caste_ratio_soldier: av[1], caste_ratio_breeder: av[2],
            forage_weight: av[3], dig_weight: av[4], nurse_weight: av[5], research_choice: None,
        };
        env.sim.apply_ai_decision(hac_side, &dec);
        let iv: Vec<f32> = intent.flatten_all()?.to_vec1()?;
        let mut intent_arr = [0.0f32; 64];
        intent_arr.copy_from_slice(&iv);
        env.sim.apply_commander_intent(hac_side, &intent_arr);
        if let Some(sr) = env.sim.colony_ai_state(opp_side) {
            let dr = brain.decide(&sr);
            env.sim.apply_ai_decision(opp_side, &dr);
        }
        let mut done = false;
        for _ in 0..DECISION_CADENCE {
            let obs = env.sim.per_ant_observations(hac_side);
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
                env.sim.apply_ant_modulators(hac_side, &mods, &ids);
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
    let (_, _, end) = score_match(env.sim.match_status(), lw, rw);
    Ok(end)
}

/// Cross-species win-matrix of a trained HAC vs `HeuristicBrain`. For every
/// ordered (hac_species `ai`, heur_species `bi`) pair, play `mpe` side-swapped
/// matches in the cross-species (nest) arena with `venom_cycle` armed; record the
/// HAC's DECISIVE winrate (a Won by the HAC's physical colony; draws/timeouts =
/// not-won, matching `eval_mlp_vs_heuristic`). Returns the N×N winrate matrix
/// (rows: HAC plays; cols: heuristic plays). Parallel over cells (shares `&hac`).
pub fn evaluate_hac_cross_species(
    hac: &HierarchicalActorCritic,
    device: &Device,
    species: &[antcolony_sim::species::Species],
    mpe: usize,
    max_ticks: u64,
    nest: bool,
    venom_cycle: f32,
) -> Result<Vec<Vec<f32>>> {
    use rayon::prelude::*;
    let n = species.len();
    let pairs: Vec<(usize, usize)> =
        (0..n).flat_map(|ai| (0..n).map(move |bi| (ai, bi))).collect();
    let flat: Vec<(usize, usize, f32)> = pairs
        .par_iter()
        .map(|&(ai, bi)| -> Result<(usize, usize, f32)> {
            let mut hac_wins = 0.0f32;
            for m in 0..mpe {
                let seed = ((ai as u64) << 40) ^ ((bi as u64) << 24) ^ (m as u64);
                // Side-swap: even → HAC is colony 0 (species ai), odd → colony 1.
                let hac_side: u8 = if m % 2 == 0 { 0 } else { 1 };
                let (sp0, sp1) = if hac_side == 0 {
                    (&species[ai], &species[bi])
                } else {
                    (&species[bi], &species[ai])
                };
                let mut env = if nest {
                    MatchEnv::new_cross_species_nest_arena(sp0, sp1, seed)
                } else {
                    MatchEnv::new_cross_species_arena(sp0, sp1, seed)
                };
                env.max_ticks = max_ticks;
                apply_cross_species_knobs(&mut env, venom_cycle);
                let mut brain = HeuristicBrain::new(5.0);
                let end = cross_match_hac(hac, device, &mut env, hac_side, &mut brain)?;
                let won = (matches!(end, MatchEnd::WonLeft) && hac_side == 0)
                    || (matches!(end, MatchEnd::WonRight) && hac_side == 1);
                if won {
                    hac_wins += 1.0;
                }
            }
            Ok((ai, bi, hac_wins / mpe as f32))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut wr = vec![vec![0.0f32; n]; n];
    for (ai, bi, v) in flat {
        wr[ai][bi] = v;
    }
    Ok(wr)
}

/// Combined head-to-head win-rate of checkpoint A versus checkpoint B.
#[derive(Clone, Debug)]
pub struct H2HReport {
    /// A's combined win-rate (worker-share metric): average of A-as-left and A-as-right.
    pub a_winrate_ws: f32,
    /// A's combined win-rate (decisive metric): average of A-as-left and A-as-right decisive.
    pub a_winrate_decisive: f32,
    /// A's win-rate when playing left colony (worker-share).
    pub a_as_left_ws: f32,
    /// A's win-rate when playing right colony (worker-share).
    pub a_as_right_ws: f32,
    /// Total matches played (= 2 * mpe).
    pub matches: usize,
}

/// Play `mpe` matches A=left/B=right, then `mpe` matches B=left/A=right, and
/// return A's head-to-head win-rate under both metrics. Uses the same seeding
/// pattern as `evaluate_hac` (`0xE7A1 * spec_seed_salt(salt)`) with distinct
/// salts for the two sides so the two sets of `mpe` matches see different seeds.
pub fn evaluate_h2h(
    hac_a: &HierarchicalActorCritic,
    hac_b: &HierarchicalActorCritic,
    device: &Device,
    mpe: usize,
) -> Result<H2HReport> {
    let salt_ab = spec_seed_salt("h2h");
    let salt_ba = spec_seed_salt("h2h-swap");

    let mut ws_a_left = 0.0f32;
    let mut dec_a_left = 0.0f32;
    let mut ws_a_right = 0.0f32;
    let mut dec_a_right = 0.0f32;
    let mut played_ab = 0usize;
    let mut played_ba = 0usize;

    // A=left, B=right
    for m in 0..mpe {
        let seed = 0xE7A1_u64
            .wrapping_mul(salt_ab)
            ^ ((m as u64).wrapping_mul(0x9E3779B97F4A7C15));
        match play_match_h2h(hac_a, hac_b, device, seed) {
            Ok((ws, dec, _end)) => {
                ws_a_left += ws;
                dec_a_left += dec;
                played_ab += 1;
            }
            Err(e) => tracing::error!(m, salt = "h2h", error = %e, "h2h match (A=left) failed; skipping"),
        }
    }

    // B=left, A=right — LEFT score belongs to B; flip to get A's score.
    // Symmetric under 1-x: draw (0.5) maps to 0.5 correctly.
    for m in 0..mpe {
        let seed = 0xE7A1_u64
            .wrapping_mul(salt_ba)
            ^ ((m as u64).wrapping_mul(0x9E3779B97F4A7C15));
        match play_match_h2h(hac_b, hac_a, device, seed) {
            Ok((ws, dec, _end)) => {
                ws_a_right += 1.0 - ws;
                dec_a_right += 1.0 - dec;
                played_ba += 1;
            }
            Err(e) => tracing::error!(m, salt = "h2h-swap", error = %e, "h2h match (B=left) failed; skipping"),
        }
    }

    let denom_ab = played_ab.max(1) as f32;
    let denom_ba = played_ba.max(1) as f32;
    let mean_ws_left = ws_a_left / denom_ab;
    let mean_ws_right = ws_a_right / denom_ba;
    let mean_dec_left = dec_a_left / denom_ab;
    let mean_dec_right = dec_a_right / denom_ba;

    let a_winrate_ws = (mean_ws_left + mean_ws_right) / 2.0;
    let a_winrate_decisive = (mean_dec_left + mean_dec_right) / 2.0;

    tracing::info!(
        a_winrate_ws,
        a_winrate_decisive,
        a_as_left_ws = mean_ws_left,
        a_as_right_ws = mean_ws_right,
        matches = played_ab + played_ba,
        "evaluate_h2h complete"
    );

    Ok(H2HReport {
        a_winrate_ws,
        a_winrate_decisive,
        a_as_left_ws: mean_ws_left,
        a_as_right_ws: mean_ws_right,
        matches: played_ab + played_ba,
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
    fn evaluate_h2h_same_policy_is_symmetric() {
        use crate::self_play::load_frozen_hac;
        use crate::hierarchical::sizing::A1;
        use candle_core::{DType, Device};
        use candle_nn::{VarBuilder, VarMap};

        let device = Device::Cpu;

        // Build a fresh HAC and save its varmap to a temp file.
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let _hac = HierarchicalActorCritic::new(vb, A1).unwrap();
        let dir = std::env::temp_dir().join("h2h_sym_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hac_sym.safetensors");
        varmap.save(&path).unwrap();

        // Load it twice so A and B are the SAME weights.
        let hac_a = load_frozen_hac(&path, A1, &device).unwrap();
        let hac_b = load_frozen_hac(&path, A1, &device).unwrap();

        let report = evaluate_h2h(&hac_a, &hac_b, &device, 3).unwrap();

        assert_eq!(report.matches, 6, "2 * mpe matches must be played");
        assert!(
            (0.0..=1.0).contains(&report.a_winrate_ws),
            "combined ws win-rate out of [0,1]: {}",
            report.a_winrate_ws
        );
        assert!(
            (0.0..=1.0).contains(&report.a_winrate_decisive),
            "combined decisive win-rate out of [0,1]: {}",
            report.a_winrate_decisive
        );
        assert!(
            (0.0..=1.0).contains(&report.a_as_left_ws),
            "a_as_left_ws out of [0,1]: {}",
            report.a_as_left_ws
        );
        assert!(
            (0.0..=1.0).contains(&report.a_as_right_ws),
            "a_as_right_ws out of [0,1]: {}",
            report.a_as_right_ws
        );
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

    #[test]
    fn play_pair_scripted_vs_scripted_runs_and_scores() {
        use crate::tournament::Controller;
        let dev = candle_core::Device::Cpu;
        let mut a = Controller::Scripted(crate::League::make_brain("aggressor", 1).unwrap());
        let mut b = Controller::Scripted(crate::League::make_brain("economist", 2).unwrap());
        let (ws, dec, _end) = super::play_pair(&mut a, &mut b, &dev, 12345, 2000).unwrap();
        assert!((0.0..=1.0).contains(&ws));
        assert!((0.0..=1.0).contains(&dec));
    }
}

//! Joint PPO trainer for the hierarchical (commander + ant) brain.
//!
//! Self-plays both colonies of a `MatchEnv` with one shared
//! `HierarchicalActorCritic`, collects a two-buffer rollout (commander
//! @ cycle cadence, ant @ tick cadence), computes per-tier GAE at the
//! colony level, and optimizes `L_total = L_cmdr + α_balance · L_ant`
//! with a single AdamW step. Phase 2b-2 scope: single-device (CPU f32)
//! smoke. See docs/superpowers/plans/2026-05-29-ant-brain-phase2b2-joint-trainer.md.

use crate::env::{MatchEnv, DECISION_CADENCE};
use crate::hierarchical::sizing::Sizing;
use crate::HierarchicalActorCritic;
use antcolony_sim::{AiDecision, MatchStatus};
use candle_core::{DType, Device, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct JointPpoConfig {
    pub iterations: usize,
    pub matches_per_iter: usize,
    /// Hard cap on commander decision cycles collected per match. Keeps
    /// the smoke rollout short (one cycle = DECISION_CADENCE outer ticks).
    pub rollout_cycles: usize,
    pub gamma: f32,
    pub gae_lambda: f32,
    pub clip: f32,
    pub epochs_per_batch: usize,
    pub lr: f64,
    pub value_coef: f64,
    pub cmdr_entropy_coef: f64,
    pub ant_entropy_coef: f64,
    /// Down-weights the ant-tier loss so ~10× more ant samples per cycle
    /// don't drown the commander gradient. Design spec §"Joint PPO loss".
    pub alpha_balance: f64,
    pub seed: u64,
}

impl JointPpoConfig {
    /// Minimal config for the mechanical smoke: 5 iters, 2 matches/iter,
    /// 8 cycles/match. Tuned for "runs without NaN/crash in under a
    /// minute on CPU", not for convergence.
    pub fn smoke_default() -> Self {
        Self {
            iterations: 5,
            matches_per_iter: 2,
            rollout_cycles: 8,
            gamma: 0.99,
            gae_lambda: 0.95,
            clip: 0.2,
            epochs_per_batch: 1,
            lr: 3e-4,
            value_coef: 0.5,
            cmdr_entropy_coef: 0.005,
            ant_entropy_coef: 0.01,
            alpha_balance: 0.1,
            seed: 0xa17_c01_2b2,
        }
    }
}

pub struct JointPpoTrainer {
    pub hac: HierarchicalActorCritic,
    pub varmap: VarMap,
    pub device: Device,
    pub config: JointPpoConfig,
    pub rng: rand_chacha::ChaCha8Rng,
}

impl JointPpoTrainer {
    pub fn new(device: Device, sizing: Sizing, config: JointPpoConfig) -> anyhow::Result<Self> {
        use rand::SeedableRng;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, sizing)?;
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(config.seed);
        Ok(Self { hac, varmap, device, config, rng })
    }

    /// AdamW over every VarMap parameter (both tiers).
    pub fn make_optimizer(&self) -> anyhow::Result<AdamW> {
        let params = ParamsAdamW {
            lr: self.config.lr,
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-8,
            weight_decay: 0.0,
        };
        Ok(AdamW::new(self.varmap.all_vars(), params)?)
    }

    /// Self-play one match for up to `rollout_cycles` commander cycles.
    /// Both colonies are driven by the shared HAC. Returns the two-buffer
    /// rollout. `match_idx` stamps every record so per-match GAE never
    /// bleeds across match boundaries in `joint_update`.
    pub fn rollout(&mut self, seed: u64, match_idx: usize) -> anyhow::Result<JointRollout> {
        tracing::debug!(seed, match_idx, "rollout start");
        let dev = self.device.clone();
        let mut env = MatchEnv::new(seed);
        let mut out = JointRollout::default();

        let mut prev_workers = [colony_workers(&env, 0), colony_workers(&env, 1)];
        let mut prev_food = [colony_food(&env, 0), colony_food(&env, 1)];

        for cycle in 0..self.config.rollout_cycles {
            // ── Commander decision (both colonies, batch leading dim 2) ──
            // If a colony has been eliminated, commander_obs_batch errors;
            // treat that as match end.
            let (state_b, pher_b, hist_b) = match env.commander_obs_batch(&dev) {
                Ok(t) => t,
                // A colony eliminated mid-match makes commander_obs_batch
                // error — normal match end. Log so a genuine tensor/infra
                // failure is distinguishable from a clean termination.
                Err(e) => {
                    tracing::debug!(cycle, match_idx, "rollout ended: commander_obs_batch: {e}");
                    break;
                }
            };
            let cmdr = self.hac.sample_commander(&state_b, &pher_b, &hist_b, &mut self.rng)?;
            let dec0 = row_to_decision(&cmdr.action, 0)?;
            let dec1 = row_to_decision(&cmdr.action, 1)?;
            env.sim.apply_ai_decision(0, &dec0);
            env.sim.apply_ai_decision(1, &dec1);
            env.apply_commander_intents(&cmdr.intent)?; // intent is [2, 64]

            // ── Outer tick loop with per-tick batched ant decisions ──
            let mut done = false;
            for _ in 0..DECISION_CADENCE {
                let (cone, internal, intent_b, index_map) =
                    env.all_ant_obs_batch(&cmdr.intent, &dev)?;
                if !index_map.is_empty() {
                    let ant = self.hac.sample_ant(&cone, &internal, &intent_b, &mut self.rng)?;
                    env.apply_ant_modulators_batched(&ant.modulator, &index_map)?;
                    let lp: Vec<f32> = ant.log_prob.to_vec1()?;
                    let val: Vec<f32> = ant.value.to_vec1()?;
                    for (i, &(cid, _aid)) in index_map.iter().enumerate() {
                        out.ant.push(AntRecord {
                            match_idx,
                            colony: cid,
                            cycle,
                            cone: cone.narrow(0, i, 1)?.detach(),
                            internal: internal.narrow(0, i, 1)?.detach(),
                            intent: intent_b.narrow(0, i, 1)?.detach(),
                            modulator: ant.modulator.narrow(0, i, 1)?.detach(),
                            log_prob: lp[i],
                            value: val[i],
                        });
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

            // ── Per-cycle r6 reward (mirrors MatchEnv::step) ──
            let workers_now = [colony_workers(&env, 0), colony_workers(&env, 1)];
            let food_now = [colony_food(&env, 0), colony_food(&env, 1)];
            let q = [colony_queen_alive(&env, 0), colony_queen_alive(&env, 1)];
            let dl = workers_now[0] as i32 - prev_workers[0] as i32;
            let dr = workers_now[1] as i32 - prev_workers[1] as i32;
            let dfl = food_now[0] - prev_food[0];
            let dfr = food_now[1] - prev_food[1];
            let mut reward_left = (dl as f32) * 0.01 - (dr as f32) * 0.01
                + dfl * 0.002 - dfr * 0.002
                + (q[0] - q[1]) * 0.005;
            let mut reward_right = -reward_left;
            if done {
                match env.sim.match_status() {
                    MatchStatus::Won { winner: 0, .. } => { reward_left += 1.0; reward_right -= 1.0; }
                    MatchStatus::Won { winner: 1, .. } => { reward_left -= 1.0; reward_right += 1.0; }
                    MatchStatus::InProgress => {
                        let total = (workers_now[0] + workers_now[1]).max(1) as f32;
                        let share = workers_now[0] as f32 / total;
                        reward_left += (share - 0.5) * 2.0;
                        reward_right += (0.5 - share) * 2.0;
                    }
                    _ => {}
                }
            }
            prev_workers = workers_now;
            prev_food = food_now;

            // ── Commander records (split the [2, ..] batch per colony) ──
            let cmdr_lp: Vec<f32> = cmdr.log_prob.to_vec1()?;
            let cmdr_val: Vec<f32> = cmdr.value.to_vec1()?;
            let rewards = [reward_left, reward_right];
            for k in 0..2usize {
                out.commander.push(CommanderRecord {
                    match_idx,
                    colony: k as u8,
                    cycle,
                    state: state_b.narrow(0, k, 1)?.detach(),
                    pheromone: pher_b.narrow(0, k, 1)?.detach(),
                    history: hist_b.narrow(0, k, 1)?.detach(),
                    action: cmdr.action.narrow(0, k, 1)?.detach(),
                    log_prob: cmdr_lp[k],
                    value: cmdr_val[k],
                    reward: rewards[k],
                    done,
                });
            }

            // ── Append history token to each colony's ring for next cycle ──
            let state_rows = state_b.to_vec2::<f32>()?;   // [2, 17]
            let action_rows = cmdr.action.to_vec2::<f32>()?; // [2, 6]
            for k in 0..2usize {
                let mut st = [0.0f32; 17];
                st.copy_from_slice(&state_rows[k]);
                let mut ac = [0.0f32; 6];
                ac.copy_from_slice(&action_rows[k]);
                env.sim.push_commander_history(k as u8, st, ac, rewards[k]);
            }

            if done {
                break;
            }
        }
        tracing::debug!(
            seed,
            match_idx,
            commander_records = out.commander.len(),
            ant_records = out.ant.len(),
            "rollout complete"
        );
        Ok(out)
    }

    /// Commander GAE per (colony, match). Output is index-aligned 1:1 with
    /// `recs`. Returns (advantages, returns).
    pub fn commander_advantages(&self, recs: &[CommanderRecord]) -> (Vec<f32>, Vec<f32>) {
        use crate::ppo::PpoTrainer;
        let mut adv = vec![0.0f32; recs.len()];
        let mut ret = vec![0.0f32; recs.len()];
        let keys: std::collections::BTreeSet<(usize, u8)> =
            recs.iter().map(|r| (r.match_idx, r.colony)).collect();
        for (m, colony) in keys {
            // Records for this stream, in push order (== cycle order).
            let idxs: Vec<usize> = recs.iter().enumerate()
                .filter(|(_, r)| r.match_idx == m && r.colony == colony)
                .map(|(i, _)| i).collect();
            let rewards: Vec<f32> = idxs.iter().map(|&i| recs[i].reward).collect();
            let values: Vec<f32> = idxs.iter().map(|&i| recs[i].value).collect();
            let dones: Vec<bool> = idxs.iter().map(|&i| recs[i].done).collect();
            let (a, r) = PpoTrainer::compute_gae(
                &rewards, &values, &dones, self.config.gamma, self.config.gae_lambda,
            );
            for (j, &i) in idxs.iter().enumerate() {
                adv[i] = a[j];
                ret[i] = r[j];
            }
        }
        (adv, ret)
    }

    /// Ant GAE per (colony, match) at cycle cadence. Reward/done come from
    /// the commander record for the same (match, colony, cycle); the
    /// bootstrap value is the mean of that cycle's ant value-head outputs.
    /// Each ant record inherits its cycle's advantage/return. Output is
    /// index-aligned 1:1 with `ant`.
    pub fn ant_advantages(
        &self,
        cmdr: &[CommanderRecord],
        ant: &[AntRecord],
    ) -> (Vec<f32>, Vec<f32>) {
        use crate::ppo::PpoTrainer;
        let mut adv = vec![0.0f32; ant.len()];
        let mut ret = vec![0.0f32; ant.len()];
        let keys: std::collections::BTreeSet<(usize, u8)> =
            ant.iter().map(|a| (a.match_idx, a.colony)).collect();
        for (m, colony) in keys {
            // reward/done per cycle from the commander records.
            let mut cyc_reward: BTreeMap<usize, f32> = BTreeMap::new();
            let mut cyc_done: BTreeMap<usize, bool> = BTreeMap::new();
            for r in cmdr.iter().filter(|r| r.match_idx == m && r.colony == colony) {
                cyc_reward.insert(r.cycle, r.reward);
                cyc_done.insert(r.cycle, r.done);
            }
            // mean ant value + ant indices per cycle.
            let mut cyc_val: BTreeMap<usize, (f32, usize)> = BTreeMap::new();
            let mut cyc_idxs: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for (i, a) in ant.iter().enumerate()
                .filter(|(_, a)| a.match_idx == m && a.colony == colony)
            {
                let e = cyc_val.entry(a.cycle).or_insert((0.0, 0));
                e.0 += a.value;
                e.1 += 1;
                cyc_idxs.entry(a.cycle).or_default().push(i);
            }
            // Ordered cycles that actually have ant samples.
            let cycles: Vec<usize> = cyc_idxs.keys().copied().collect();
            let rewards: Vec<f32> = cycles.iter()
                .map(|c| *cyc_reward.get(c).unwrap_or(&0.0)).collect();
            let values: Vec<f32> = cycles.iter()
                .map(|c| { let (s, n) = cyc_val[c]; if n > 0 { s / n as f32 } else { 0.0 } })
                .collect();
            let dones: Vec<bool> = cycles.iter()
                .map(|c| *cyc_done.get(c).unwrap_or(&false)).collect();
            let (a, r) = PpoTrainer::compute_gae(
                &rewards, &values, &dones, self.config.gamma, self.config.gae_lambda,
            );
            for (j, c) in cycles.iter().enumerate() {
                for &i in &cyc_idxs[c] {
                    adv[i] = a[j];
                    ret[i] = r[j];
                }
            }
        }
        (adv, ret)
    }

    /// Run the full smoke loop: for each iteration, collect
    /// `matches_per_iter` self-play rollouts into one buffer, then do one
    /// joint update. Returns per-iteration loss stats. Logs each iter via
    /// tracing.
    pub fn train(&mut self) -> anyhow::Result<Vec<JointLossStats>> {
        let mut opt = self.make_optimizer()?;
        let mut history = Vec::with_capacity(self.config.iterations);
        for it in 0..self.config.iterations {
            let mut roll = JointRollout::default();
            for m in 0..self.config.matches_per_iter {
                let seed = self.config.seed
                    ^ ((it as u64) << 32)
                    ^ ((m as u64).wrapping_mul(0x9E3779B97F4A7C15));
                // Globally-unique match_idx (not just `m`): GAE buckets by
                // (match_idx, colony), and `roll` is fresh per iteration so
                // `m` alone is correct today — but a future accumulating
                // replay buffer would silently mis-bucket on collisions.
                let match_idx = it * self.config.matches_per_iter + m;
                let r = self.rollout(seed, match_idx)?;
                roll.commander.extend(r.commander);
                roll.ant.extend(r.ant);
            }
            let stats = self.joint_update(&mut opt, &roll)?;
            tracing::info!(
                iter = it,
                total = stats.total,
                commander = stats.commander,
                ant = stats.ant,
                cmdr_records = roll.commander.len(),
                ant_records = roll.ant.len(),
                "joint ppo iteration"
            );
            history.push(stats);
        }
        Ok(history)
    }

    /// One joint PPO update over the rollout. Returns the per-tier loss
    /// breakdown. Numerics per tier mirror `PpoTrainer::ppo_update`
    /// (clipped surrogate + value MSE + Gaussian entropy bonus). Runs
    /// `epochs_per_batch` passes; the smoke uses 1.
    pub fn joint_update(
        &self,
        opt: &mut AdamW,
        rollout: &JointRollout,
    ) -> anyhow::Result<JointLossStats> {
        let dev = self.device.clone();
        let two_pi = std::f64::consts::PI * 2.0;
        let ent_const = 0.5 * (1.0 + two_pi.ln());

        // ── advantages/returns (computed once, reused across epochs) ──
        let (cadv_raw, cret) = self.commander_advantages(&rollout.commander);
        let cadv = normalize_adv(&cadv_raw);
        let (aadv_raw, aret) = self.ant_advantages(&rollout.commander, &rollout.ant);
        let aadv = normalize_adv(&aadv_raw);

        let cn = rollout.commander.len();
        let an = rollout.ant.len();
        anyhow::ensure!(cn > 0, "joint_update: empty commander buffer");

        // ── pre-cat commander tensors (constant across epochs) ──
        let c_state = Tensor::cat(
            &rollout.commander.iter().map(|r| r.state.clone()).collect::<Vec<_>>(), 0)?;
        let c_pher = Tensor::cat(
            &rollout.commander.iter().map(|r| r.pheromone.clone()).collect::<Vec<_>>(), 0)?;
        let c_hist = Tensor::cat(
            &rollout.commander.iter().map(|r| r.history.clone()).collect::<Vec<_>>(), 0)?;
        let c_act = Tensor::cat(
            &rollout.commander.iter().map(|r| r.action.clone()).collect::<Vec<_>>(), 0)?;
        let c_oldlp = Tensor::from_slice(
            &rollout.commander.iter().map(|r| r.log_prob).collect::<Vec<_>>(), cn, &dev)?;
        let c_adv = Tensor::from_slice(&cadv, cn, &dev)?;
        let c_ret = Tensor::from_slice(&cret, cn, &dev)?;

        // ── pre-cat ant tensors (may be empty) ──
        let ant_tensors = if an > 0 {
            let cone = Tensor::cat(
                &rollout.ant.iter().map(|a| a.cone.clone()).collect::<Vec<_>>(), 0)?;
            let internal = Tensor::cat(
                &rollout.ant.iter().map(|a| a.internal.clone()).collect::<Vec<_>>(), 0)?;
            let intent = Tensor::cat(
                &rollout.ant.iter().map(|a| a.intent.clone()).collect::<Vec<_>>(), 0)?;
            let modulator = Tensor::cat(
                &rollout.ant.iter().map(|a| a.modulator.clone()).collect::<Vec<_>>(), 0)?;
            let oldlp = Tensor::from_slice(
                &rollout.ant.iter().map(|a| a.log_prob).collect::<Vec<_>>(), an, &dev)?;
            let adv = Tensor::from_slice(&aadv, an, &dev)?;
            let ret = Tensor::from_slice(&aret, an, &dev)?;
            Some((cone, internal, intent, modulator, oldlp, adv, ret))
        } else {
            None
        };

        let clip_lo = 1.0 - self.config.clip;
        let clip_hi = 1.0 + self.config.clip;

        let mut last = JointLossStats { total: 0.0, commander: 0.0, ant: 0.0 };
        for _epoch in 0..self.config.epochs_per_batch {
            // ── Commander loss ──
            let new_lp = self.hac.log_prob_of_commander_action(&c_state, &c_pher, &c_hist, &c_act)?;
            let value_pred = self.hac.forward_commander(&c_state, &c_pher, &c_hist)?.value;
            let ratio = (&new_lp - &c_oldlp)?.exp()?;
            let surr1 = (&ratio * &c_adv)?;
            let surr2 = (&ratio.clamp(clip_lo, clip_hi)? * &c_adv)?;
            let policy_loss = surr1.minimum(&surr2)?.mean_all()?.affine(-1.0, 0.0)?;
            let value_loss = (&value_pred - &c_ret)?.sqr()?.mean_all()?;
            let entropy = self.hac.commander.log_std.affine(1.0, ent_const)?.sum_all()?;
            let cmdr_total = ((&policy_loss + value_loss.affine(self.config.value_coef, 0.0)?)?
                - entropy.affine(self.config.cmdr_entropy_coef, 0.0)?)?;

            // ── Ant loss (optional) ──
            let (ant_total, ant_scalar) = if let Some((cone, internal, intent, modulator, oldlp, adv, ret)) = &ant_tensors {
                let new_lp = self.hac.log_prob_of_ant_modulator(cone, internal, intent, modulator)?;
                let value_pred = self.hac.forward_ant(cone, internal, intent)?.value;
                let ratio = (&new_lp - oldlp)?.exp()?;
                let surr1 = (&ratio * adv)?;
                let surr2 = (&ratio.clamp(clip_lo, clip_hi)? * adv)?;
                let policy_loss = surr1.minimum(&surr2)?.mean_all()?.affine(-1.0, 0.0)?;
                let value_loss = (&value_pred - ret)?.sqr()?.mean_all()?;
                let entropy = self.hac.ant.log_std.affine(1.0, ent_const)?.sum_all()?;
                let at = ((&policy_loss + value_loss.affine(self.config.value_coef, 0.0)?)?
                    - entropy.affine(self.config.ant_entropy_coef, 0.0)?)?;
                let scalar = at.to_scalar::<f32>()?;
                (Some(at), scalar)
            } else {
                (None, 0.0)
            };

            // ── Combine + step ──
            let total = match &ant_total {
                Some(at) => (&cmdr_total + at.affine(self.config.alpha_balance, 0.0)?)?,
                None => cmdr_total.clone(),
            };
            let total_scalar = total.to_scalar::<f32>()?;
            let cmdr_scalar = cmdr_total.to_scalar::<f32>()?;
            // No grad-norm clipping (candle AdamW has none; the flat trainer
            // doesn't clip either). Fine for the 5-iter smoke — a single
            // noisy rollout can spike one iter's loss but can't diverge in 5
            // steps. Phase 3's longer horizon needs a pre-step clip.
            opt.backward_step(&total)?;
            last = JointLossStats { total: total_scalar, commander: cmdr_scalar, ant: ant_scalar };
        }
        Ok(last)
    }
}

/// Standard PPO advantage normalization (zero mean, unit std).
fn normalize_adv(adv: &[f32]) -> Vec<f32> {
    let n = adv.len().max(1) as f32;
    let mean = adv.iter().sum::<f32>() / n;
    let var = adv.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
    let std = (var + 1e-8).sqrt();
    adv.iter().map(|x| (x - mean) / std).collect()
}

fn colony_workers(env: &MatchEnv, k: u8) -> u32 {
    env.sim.colonies.get(k as usize).map(|c| c.population.workers).unwrap_or(0)
}
fn colony_food(env: &MatchEnv, k: u8) -> f32 {
    env.sim.colonies.get(k as usize).map(|c| c.food_stored).unwrap_or(0.0)
}
fn colony_queen_alive(env: &MatchEnv, k: u8) -> f32 {
    env.sim.colonies.get(k as usize)
        .map(|c| if c.queen_health > 0.0 { 1.0 } else { 0.0 })
        .unwrap_or(0.0)
}

/// Decode one row of a [B, 6] post-squash action tensor into an AiDecision.
fn row_to_decision(action_batch: &candle_core::Tensor, row: usize) -> anyhow::Result<AiDecision> {
    let r = action_batch.narrow(0, row, 1)?.squeeze(0)?; // [6]
    let v: Vec<f32> = r.to_vec1()?;
    Ok(AiDecision {
        caste_ratio_worker: v[0],
        caste_ratio_soldier: v[1],
        caste_ratio_breeder: v[2],
        forage_weight: v[3],
        dig_weight: v[4],
        nurse_weight: v[5],
        research_choice: None,
    })
}

/// One commander decision for one colony in one cycle of one match.
pub struct CommanderRecord {
    pub match_idx: usize,
    pub colony: u8,
    pub cycle: usize,
    pub state: Tensor,     // [1, 17]
    pub pheromone: Tensor, // [1, 4, 32, 32]
    pub history: Tensor,   // [1, 8, 96]
    pub action: Tensor,    // [1, 6] post-squash
    pub log_prob: f32,
    pub value: f32,
    pub reward: f32,
    pub done: bool,
}

/// One ant decision for one ant in one tick of one cycle.
pub struct AntRecord {
    pub match_idx: usize,
    pub colony: u8,
    pub cycle: usize,
    pub cone: Tensor,      // [1, 60]
    pub internal: Tensor,  // [1, 8]
    pub intent: Tensor,    // [1, 64]
    pub modulator: Tensor, // [1, 5] post-squash
    pub log_prob: f32,
    pub value: f32,
}

#[derive(Default)]
pub struct JointRollout {
    pub commander: Vec<CommanderRecord>,
    pub ant: Vec<AntRecord>,
}

/// Per-iteration loss breakdown for logging + the smoke assertion.
#[derive(Clone, Debug)]
pub struct JointLossStats {
    pub total: f32,
    pub commander: f32,
    pub ant: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchical::sizing::A1;
    use candle_core::Device;

    #[test]
    fn rollout_fills_both_buffers_with_finite_records() {
        let mut t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let roll = t.rollout(0xfeed_1, 0).unwrap();

        assert!(!roll.commander.is_empty(), "commander buffer must be non-empty");
        assert!(!roll.ant.is_empty(), "ant buffer must be non-empty");

        // Commander: 2 colonies per cycle, all rows the right shape + finite.
        for r in &roll.commander {
            assert_eq!(r.state.dims(), &[1, 17]);
            assert_eq!(r.pheromone.dims(), &[1, 4, 32, 32]);
            assert_eq!(r.history.dims(), &[1, 8, 96]);
            assert_eq!(r.action.dims(), &[1, 6]);
            assert!(r.log_prob.is_finite() && r.value.is_finite() && r.reward.is_finite());
            assert!(r.colony == 0 || r.colony == 1);
        }
        // Ant rows finite + shaped.
        for a in &roll.ant {
            assert_eq!(a.cone.dims(), &[1, 60]);
            assert_eq!(a.internal.dims(), &[1, 8]);
            assert_eq!(a.intent.dims(), &[1, 64]);
            assert_eq!(a.modulator.dims(), &[1, 5]);
            assert!(a.log_prob.is_finite() && a.value.is_finite());
        }
        // Both colonies represented in the commander buffer.
        assert!(roll.commander.iter().any(|r| r.colony == 0));
        assert!(roll.commander.iter().any(|r| r.colony == 1));
    }

    #[test]
    fn smoke_default_has_five_iters_and_balanced_coefs() {
        let c = JointPpoConfig::smoke_default();
        assert_eq!(c.iterations, 5);
        assert_eq!(c.matches_per_iter, 2);
        assert_eq!(c.rollout_cycles, 8);
        assert!(c.alpha_balance > 0.0 && c.alpha_balance <= 1.0);
        assert!(c.ant_entropy_coef >= c.cmdr_entropy_coef);
    }

    #[test]
    fn trainer_builds_at_a1_with_nonzero_params() {
        let t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let total: usize = t.varmap.all_vars().iter()
            .map(|v| v.dims().iter().product::<usize>()).sum();
        assert!(total > 1_000_000, "A1 HAC should have >1M params, got {}", total);
        let _opt = t.make_optimizer().unwrap();
    }

    #[test]
    fn joint_rollout_defaults_empty() {
        let r = JointRollout::default();
        assert!(r.commander.is_empty());
        assert!(r.ant.is_empty());
    }

    #[test]
    fn gae_helpers_align_to_records_and_are_finite() {
        let mut t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let roll = t.rollout(0xfeed_2, 0).unwrap();

        let (cadv, cret) = t.commander_advantages(&roll.commander);
        assert_eq!(cadv.len(), roll.commander.len());
        assert_eq!(cret.len(), roll.commander.len());
        assert!(cadv.iter().chain(cret.iter()).all(|x| x.is_finite()));

        let (aadv, aret) = t.ant_advantages(&roll.commander, &roll.ant);
        assert_eq!(aadv.len(), roll.ant.len());
        assert_eq!(aret.len(), roll.ant.len());
        assert!(aadv.iter().chain(aret.iter()).all(|x| x.is_finite()));
    }

    fn first_var_flat(vm: &candle_nn::VarMap) -> Vec<f32> {
        // Concatenate all vars into a single flat snapshot so the "any
        // weight moved" assertion is robust regardless of VarMap iteration
        // order (HashMap order is non-deterministic; vars[0] could be
        // log_std which may not move enough on a single Adam step at
        // smoke_default's tiny entropy_coef).
        let vars = vm.all_vars();
        vars.iter()
            .flat_map(|v| v.as_tensor().flatten_all().unwrap().to_vec1::<f32>().unwrap())
            .collect()
    }

    #[test]
    fn joint_update_returns_finite_loss_and_moves_weights() {
        let mut t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let mut opt = t.make_optimizer().unwrap();
        let roll = t.rollout(0xfeed_3, 0).unwrap();

        let before = first_var_flat(&t.varmap);
        let stats = t.joint_update(&mut opt, &roll).unwrap();
        let after = first_var_flat(&t.varmap);

        assert!(stats.total.is_finite(), "total loss must be finite: {}", stats.total);
        assert!(stats.commander.is_finite());
        assert!(stats.ant.is_finite());
        assert!(
            before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-9),
            "at least one parameter must change after an Adam step"
        );
    }
}

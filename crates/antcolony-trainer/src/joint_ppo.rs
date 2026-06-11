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
use crate::reward::{compute_step_reward, ColonyMetrics, RewardConfig};
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
    /// Max ant rows forwarded per backward pass in `joint_update`. `0` =
    /// no chunking (one forward over every ant row — the original
    /// monolithic path, exact-reproducible for the 47% baseline). A
    /// positive value splits the ant tier's forward/backward into chunks
    /// of this many rows and accumulates their gradients before a single
    /// optimizer step — mathematically identical, but it caps peak
    /// activation memory so `rollout_cycles` can span full matches on an
    /// 8 GB card without OOM. See Phase 3.5 (rollout-horizon fix).
    pub ant_chunk_size: usize,
    /// Global gradient-norm clip threshold applied before the optimizer step.
    /// `0.0` (or negative) disables clipping — exact-reproducible with the
    /// un-clipped 47% baseline and the prior smoke numerics. A positive value
    /// (PPO-standard ≈ 0.5) scales the combined gradient down to this L2 norm
    /// whenever it exceeds it, taming the late-training AdamW spikes that made
    /// the Phase-3 win-rate curve peak then regress (candle's AdamW has no
    /// built-in clip). Applied identically in the monolithic and chunked
    /// paths, so it does not break their gradient equivalence.
    pub max_grad_norm: f64,
    /// M13: optional PPO value-loss clipping range for BOTH tiers (mirrors the
    /// flat `PpoConfig.value_clip`). `None` (or `<= 0`) = plain MSE, which is
    /// exact-reproducible with the 47% baseline and keeps the chunked==
    /// monolithic equivalence (both feed identical old_values). When `Some(c)`,
    /// the value head's per-step move is clipped to ±c around its rollout-time
    /// prediction and the loss is `max(unclipped_mse, clipped_mse)` — the same
    /// pessimistic bound that stopped the 115k+/40M+ value-loss spikes on the
    /// flat path when novel pop-based opponents enter the league.
    pub value_clip: Option<f32>,
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
            ant_chunk_size: 0,
            max_grad_norm: 0.0,
            value_clip: None,
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

        // M15: r6 reward via the shared `compute_step_reward` (no inlined third
        // copy). Self-play uses RewardConfig::default() == r6, so the smoke
        // numerics are unchanged. Window combat losses = 0 at the baseline.
        let reward_cfg = RewardConfig::default();
        let mut prev = [
            ColonyMetrics::from_sim(&env.sim, 0, 0),
            ColonyMetrics::from_sim(&env.sim, 1, 0),
        ];

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

            // H7: snapshot cumulative combat losses before the tick loop so the
            // per-cycle reward sees the window-summed delta (the tick-local
            // counter is cleared every tick).
            let loss_base = [colony_combat_losses(&env, 0), colony_combat_losses(&env, 1)];

            // ── Outer tick loop with per-tick batched ant decisions ──
            let mut done = false;
            for _ in 0..DECISION_CADENCE {
                let (cone, internal, intent_b, index_map) =
                    env.all_ant_obs_batch(&cmdr.intent, &dev)?;
                if !index_map.is_empty() {
                    let ant = self.hac.sample_ant(&cone, &internal, &intent_b, &mut self.rng)?;
                    env.apply_ant_modulators_batched(&ant.modulator, &index_map)?;
                    // Store the whole tick batch (one tensor each), not per-ant.
                    out.ant.push(AntBatch {
                        match_idx: vec![match_idx; index_map.len()],
                        colony: index_map.iter().map(|&(cid, _)| cid).collect(),
                        cycle,
                        cone: cone.detach(),
                        internal: internal.detach(),
                        intent: intent_b.detach(),
                        modulator: ant.modulator.detach(),
                        log_prob: ant.log_prob.to_vec1()?,
                        value: ant.value.to_vec1()?,
                    });
                }
                env.sim.tick();
                if !matches!(env.sim.match_status(), MatchStatus::InProgress)
                    || env.sim.tick >= env.max_ticks
                {
                    done = true;
                    break;
                }
            }

            // ── Per-cycle r6 reward via the shared compute_step_reward (M15) ──
            // A genuinely vanished colony yields ColonyMetrics::default() (0
            // workers/food, queen_alive=0) — a clean terminal signal, NOT a
            // spurious negative worker-delta vs a stale `prev`. The terminal
            // win/share bonus then dominates.
            let win0 = colony_combat_losses(&env, 0).saturating_sub(loss_base[0]);
            let win1 = colony_combat_losses(&env, 1).saturating_sub(loss_base[1]);
            let cur = [
                ColonyMetrics::from_sim(&env.sim, 0, win0),
                ColonyMetrics::from_sim(&env.sim, 1, win1),
            ];
            let status = env.sim.match_status();
            let (reward_left, reward_right) =
                compute_step_reward(&reward_cfg, &prev, &cur, done, status);
            prev = cur;

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
            // H6: bootstrap a horizon-truncated tail from the last step's own
            // value (last_value = None) instead of 0. A genuine terminal
            // (dones[last] == true) still bootstraps from 0. Threading a real
            // post-rollout V(s_n) through the batched parallel rollout is
            // invasive, so we take the documented `values[n-1]` fallback here.
            let (a, r) = PpoTrainer::compute_gae_bootstrap(
                &rewards, &values, &dones, self.config.gamma, self.config.gae_lambda, None,
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
    /// Each ant row inherits its cycle's advantage/return. Output is a flat
    /// `Vec` of length `sum(batch.len())`, in (batch order, row order) — the
    /// same order `joint_update` cats the batch tensors, so element `k`
    /// aligns with cat row `k`.
    pub fn ant_advantages(
        &self,
        cmdr: &[CommanderRecord],
        ant: &[AntBatch],
    ) -> (Vec<f32>, Vec<f32>) {
        use crate::ppo::PpoTrainer;
        // Flattened length = total rows across all batches, in (batch order,
        // row order) — the SAME order `joint_update` cats the batch tensors,
        // so adv[k]/ret[k] align with cat row k.
        let total: usize = ant.iter().map(|b| b.len()).sum();
        let mut adv = vec![0.0f32; total];
        let mut ret = vec![0.0f32; total];

        // Mean ant value per (match, colony, cycle).
        let mut val_sum: BTreeMap<(usize, u8, usize), (f32, usize)> = BTreeMap::new();
        for b in ant {
            for j in 0..b.len() {
                let e = val_sum.entry((b.match_idx[j], b.colony[j], b.cycle)).or_insert((0.0, 0));
                e.0 += b.value[j];
                e.1 += 1;
            }
        }
        // Distinct (match, colony) streams and their cycles.
        let mut streams: BTreeMap<(usize, u8), Vec<usize>> = BTreeMap::new();
        for (m, c, cyc) in val_sum.keys() {
            streams.entry((*m, *c)).or_default().push(*cyc);
        }
        // Cycle-cadence GAE per (match, colony) → map (match,colony,cycle)→(adv,ret).
        let mut cyc_adv: BTreeMap<(usize, u8, usize), (f32, f32)> = BTreeMap::new();
        for ((m, c), cyc_list) in &streams {
            let mut cycles = cyc_list.clone();
            cycles.sort_unstable();
            cycles.dedup();
            let mut cyc_reward: BTreeMap<usize, f32> = BTreeMap::new();
            let mut cyc_done: BTreeMap<usize, bool> = BTreeMap::new();
            for r in cmdr.iter().filter(|r| r.match_idx == *m && r.colony == *c) {
                cyc_reward.insert(r.cycle, r.reward);
                cyc_done.insert(r.cycle, r.done);
            }
            let rewards: Vec<f32> = cycles.iter().map(|cy| *cyc_reward.get(cy).unwrap_or(&0.0)).collect();
            let values: Vec<f32> = cycles.iter()
                .map(|cy| {
                    let (s, n) = *val_sum.get(&(*m, *c, *cy)).unwrap_or(&(0.0, 0));
                    if n > 0 { s / n as f32 } else { 0.0 }
                })
                .collect();
            let dones: Vec<bool> = cycles.iter().map(|cy| *cyc_done.get(cy).unwrap_or(&false)).collect();
            // H6: truncated tail bootstraps from the last cycle's own mean ant
            // value (last_value = None) instead of 0; genuine terminal -> 0.
            let (a, r) = PpoTrainer::compute_gae_bootstrap(
                &rewards, &values, &dones, self.config.gamma, self.config.gae_lambda, None,
            );
            for (k, cy) in cycles.iter().enumerate() {
                cyc_adv.insert((*m, *c, *cy), (a[k], r[k]));
            }
        }
        // Fill the flat output in batch-row order.
        let mut row = 0usize;
        for b in ant {
            for j in 0..b.len() {
                let (a, r) = *cyc_adv.get(&(b.match_idx[j], b.colony[j], b.cycle)).unwrap_or(&(0.0, 0.0));
                adv[row] = a;
                ret[row] = r;
                row += 1;
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
        // Total ant rows across all batches (flattened) — matches the
        // cat row count and the ant_advantages output length.
        let an: usize = rollout.ant.iter().map(|b| b.len()).sum();
        anyhow::ensure!(cn > 0, "joint_update: empty commander buffer");

        // Memory-bounded path: chunk the ant tier so peak activation memory
        // stays flat regardless of rollout horizon. Only worth it when there
        // are more ant rows than the chunk budget; otherwise fall through to
        // the monolithic path (exact 47%-baseline numerics).
        if self.config.ant_chunk_size > 0 && an > self.config.ant_chunk_size {
            return self.joint_update_chunked(opt, rollout, &cadv, &cret, &aadv, &aret);
        }

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
        // M13: rollout-time value predictions (old_value) for value clipping.
        let c_oldv = Tensor::from_slice(
            &rollout.commander.iter().map(|r| r.value).collect::<Vec<_>>(), cn, &dev)?;

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
            // Flatten per-batch log_probs in batch-row order (== cat order
            // == ant_advantages order).
            let mut oldlp_v: Vec<f32> = Vec::with_capacity(an);
            let mut oldv_v: Vec<f32> = Vec::with_capacity(an);
            for b in &rollout.ant {
                oldlp_v.extend_from_slice(&b.log_prob);
                oldv_v.extend_from_slice(&b.value);
            }
            let oldlp = Tensor::from_slice(&oldlp_v, an, &dev)?;
            let oldv = Tensor::from_slice(&oldv_v, an, &dev)?;
            let adv = Tensor::from_slice(&aadv, an, &dev)?;
            let ret = Tensor::from_slice(&aret, an, &dev)?;
            Some((cone, internal, intent, modulator, oldlp, oldv, adv, ret))
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
            let value_loss = clipped_value_loss(&value_pred, &c_ret, &c_oldv, self.config.value_clip)?;
            let entropy = self.hac.commander.log_std.affine(1.0, ent_const)?.sum_all()?;
            let cmdr_total = ((&policy_loss + value_loss.affine(self.config.value_coef, 0.0)?)?
                - entropy.affine(self.config.cmdr_entropy_coef, 0.0)?)?;

            // ── Ant loss (optional) ──
            let (ant_total, ant_scalar) = if let Some((cone, internal, intent, modulator, oldlp, oldv, adv, ret)) = &ant_tensors {
                let new_lp = self.hac.log_prob_of_ant_modulator(cone, internal, intent, modulator)?;
                let value_pred = self.hac.forward_ant(cone, internal, intent)?.value;
                let ratio = (&new_lp - oldlp)?.exp()?;
                let surr1 = (&ratio * adv)?;
                let surr2 = (&ratio.clamp(clip_lo, clip_hi)? * adv)?;
                let policy_loss = surr1.minimum(&surr2)?.mean_all()?.affine(-1.0, 0.0)?;
                let value_loss = clipped_value_loss(&value_pred, ret, oldv, self.config.value_clip)?;
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
            // Manual backward → (optional) global-norm clip → step, instead of
            // `opt.backward_step(&total)`. With `max_grad_norm == 0.0` this is
            // bit-identical to the old `backward_step` (which is just
            // `backward()` then `step()`); a positive threshold tames the
            // late-training AdamW gradient spikes that made the win-rate curve
            // peak then regress. candle's AdamW has no built-in clip.
            let mut grads = total.backward()?;
            if self.config.max_grad_norm > 0.0 {
                let gn = clip_grad_norm(&mut grads, &self.varmap.all_vars(), self.config.max_grad_norm)?;
                tracing::debug!(grad_norm = gn, max = self.config.max_grad_norm, "grad clip (monolithic)");
            }
            opt.step(&grads)?;
            last = JointLossStats { total: total_scalar, commander: cmdr_scalar, ant: ant_scalar };
        }
        Ok(last)
    }

    /// Memory-bounded variant of `joint_update`: identical gradient, but the
    /// ant tier's forward/backward is split into `ant_chunk_size`-row chunks
    /// whose gradients are accumulated before one optimizer step. The
    /// commander tier (tiny) is done whole. Caps peak activation memory so
    /// long-horizon rollouts fit on small cards. `cadv/cret/aadv/aret` are the
    /// already-normalized advantages / returns from `joint_update`.
    #[allow(clippy::too_many_arguments)]
    fn joint_update_chunked(
        &self,
        opt: &mut AdamW,
        rollout: &JointRollout,
        cadv: &[f32],
        cret: &[f32],
        aadv: &[f32],
        aret: &[f32],
    ) -> anyhow::Result<JointLossStats> {
        let dev = self.device.clone();
        let two_pi = std::f64::consts::PI * 2.0;
        let ent_const = 0.5 * (1.0 + two_pi.ln());

        let cn = rollout.commander.len();
        let an: usize = rollout.ant.iter().map(|b| b.len()).sum();
        anyhow::ensure!(an > 0, "joint_update_chunked: empty ant buffer");

        // ── pre-cat commander tensors (small; forwarded whole) ──
        let c_state = Tensor::cat(&rollout.commander.iter().map(|r| r.state.clone()).collect::<Vec<_>>(), 0)?;
        let c_pher = Tensor::cat(&rollout.commander.iter().map(|r| r.pheromone.clone()).collect::<Vec<_>>(), 0)?;
        let c_hist = Tensor::cat(&rollout.commander.iter().map(|r| r.history.clone()).collect::<Vec<_>>(), 0)?;
        let c_act = Tensor::cat(&rollout.commander.iter().map(|r| r.action.clone()).collect::<Vec<_>>(), 0)?;
        let c_oldlp = Tensor::from_slice(
            &rollout.commander.iter().map(|r| r.log_prob).collect::<Vec<_>>(), cn, &dev)?;
        let c_adv = Tensor::from_slice(cadv, cn, &dev)?;
        let c_ret = Tensor::from_slice(cret, cn, &dev)?;
        let c_oldv = Tensor::from_slice(
            &rollout.commander.iter().map(|r| r.value).collect::<Vec<_>>(), cn, &dev)?;

        // ── pre-cat ant tensors (data only; chunks `narrow` into these) ──
        let a_cone = Tensor::cat(&rollout.ant.iter().map(|a| a.cone.clone()).collect::<Vec<_>>(), 0)?;
        let a_int = Tensor::cat(&rollout.ant.iter().map(|a| a.internal.clone()).collect::<Vec<_>>(), 0)?;
        let a_intent = Tensor::cat(&rollout.ant.iter().map(|a| a.intent.clone()).collect::<Vec<_>>(), 0)?;
        let a_mod = Tensor::cat(&rollout.ant.iter().map(|a| a.modulator.clone()).collect::<Vec<_>>(), 0)?;
        let mut oldlp_v: Vec<f32> = Vec::with_capacity(an);
        let mut oldv_v: Vec<f32> = Vec::with_capacity(an);
        for b in &rollout.ant {
            oldlp_v.extend_from_slice(&b.log_prob);
            oldv_v.extend_from_slice(&b.value);
        }
        let a_oldlp = Tensor::from_slice(&oldlp_v, an, &dev)?;
        let a_oldv = Tensor::from_slice(&oldv_v, an, &dev)?;
        let a_adv = Tensor::from_slice(aadv, an, &dev)?;
        let a_ret = Tensor::from_slice(aret, an, &dev)?;

        let clip_lo = 1.0 - self.config.clip;
        let clip_hi = 1.0 + self.config.clip;
        let vars = self.varmap.all_vars();
        let chunk = self.config.ant_chunk_size;
        let n = an as f64;

        let mut last = JointLossStats { total: 0.0, commander: 0.0, ant: 0.0 };
        for _epoch in 0..self.config.epochs_per_batch {
            // ── Commander loss (whole) → seeds the gradient accumulator ──
            let new_lp = self.hac.log_prob_of_commander_action(&c_state, &c_pher, &c_hist, &c_act)?;
            let value_pred = self.hac.forward_commander(&c_state, &c_pher, &c_hist)?.value;
            let ratio = (&new_lp - &c_oldlp)?.exp()?;
            let surr1 = (&ratio * &c_adv)?;
            let surr2 = (&ratio.clamp(clip_lo, clip_hi)? * &c_adv)?;
            let policy_loss = surr1.minimum(&surr2)?.mean_all()?.affine(-1.0, 0.0)?;
            let value_loss = clipped_value_loss(&value_pred, &c_ret, &c_oldv, self.config.value_clip)?;
            let entropy = self.hac.commander.log_std.affine(1.0, ent_const)?.sum_all()?;
            let cmdr_total = ((&policy_loss + value_loss.affine(self.config.value_coef, 0.0)?)?
                - entropy.affine(self.config.cmdr_entropy_coef, 0.0)?)?;
            let cmdr_scalar = cmdr_total.to_scalar::<f32>()?;
            let mut grads = cmdr_total.backward()?;

            // ── Ant entropy term: α·(−ant_entropy_coef)·entropy, added ONCE.
            // Batch-independent (depends only on ant.log_std), so it must NOT
            // be replicated per chunk. α_balance folded in so the gradient
            // lands pre-scaled in the accumulator.
            let ant_ent = self.hac.ant.log_std.affine(1.0, ent_const)?.sum_all()?;
            let ant_ent_val = ant_ent.to_scalar::<f32>()?;
            let ent_loss = ant_ent.affine(
                -(self.config.ant_entropy_coef * self.config.alpha_balance), 0.0)?;
            accumulate_grads(&mut grads, &ent_loss.backward()?, &vars)?;

            // ── Ant policy+value: per-chunk forward/backward, accumulated ──
            // Each chunk's mean is weighted by len/n so the sum of weighted
            // chunk means equals the full-batch mean → identical gradient to
            // one forward over all rows, but peak activation memory is capped
            // at one chunk. α_balance folded into each chunk loss.
            let mut ant_pv_scalar = 0.0f32; // reconstructs the whole-batch ant policy+value loss
            let mut start = 0usize;
            while start < an {
                let len = chunk.min(an - start);
                let cone_c = a_cone.narrow(0, start, len)?;
                let int_c = a_int.narrow(0, start, len)?;
                let intent_c = a_intent.narrow(0, start, len)?;
                let mod_c = a_mod.narrow(0, start, len)?;
                let oldlp_c = a_oldlp.narrow(0, start, len)?;
                let oldv_c = a_oldv.narrow(0, start, len)?;
                let adv_c = a_adv.narrow(0, start, len)?;
                let ret_c = a_ret.narrow(0, start, len)?;

                let new_lp = self.hac.log_prob_of_ant_modulator(&cone_c, &int_c, &intent_c, &mod_c)?;
                let value_pred = self.hac.forward_ant(&cone_c, &int_c, &intent_c)?.value;
                let ratio = (&new_lp - &oldlp_c)?.exp()?;
                let surr1 = (&ratio * &adv_c)?;
                let surr2 = (&ratio.clamp(clip_lo, clip_hi)? * &adv_c)?;
                let policy_loss = surr1.minimum(&surr2)?.mean_all()?.affine(-1.0, 0.0)?;
                let value_loss = clipped_value_loss(&value_pred, &ret_c, &oldv_c, self.config.value_clip)?;
                let pv = (&policy_loss + value_loss.affine(self.config.value_coef, 0.0)?)?;

                let w = (len as f64) / n;
                ant_pv_scalar += (w as f32) * pv.to_scalar::<f32>()?;
                let g_loss = pv.affine(w * self.config.alpha_balance, 0.0)?;
                accumulate_grads(&mut grads, &g_loss.backward()?, &vars)?;
                start += len;
            }

            // Clip the fully-accumulated gradient (same combined gradient the
            // monolithic path produces → identical clip result), then take ONE
            // optimizer step over commander + entropy + all ant chunks — NOT
            // one step per chunk (that would be a different Adam trajectory).
            if self.config.max_grad_norm > 0.0 {
                let gn = clip_grad_norm(&mut grads, &vars, self.config.max_grad_norm)?;
                tracing::debug!(grad_norm = gn, max = self.config.max_grad_norm, "grad clip (chunked)");
            }
            opt.step(&grads)?;

            let ant_scalar = ant_pv_scalar - self.config.ant_entropy_coef as f32 * ant_ent_val;
            let total_scalar = cmdr_scalar + self.config.alpha_balance as f32 * ant_scalar;
            last = JointLossStats { total: total_scalar, commander: cmdr_scalar, ant: ant_scalar };
        }
        Ok(last)
    }
}

/// Sum the gradients in `src` into `acc` for every var (adding where a var
/// already has an accumulated grad). Gradients are linear, so summing the
/// per-chunk backward passes is identical to one backward over the
/// concatenation — this is what lets `joint_update_chunked` take a single
/// optimizer step over many memory-bounded ant chunks.
fn accumulate_grads(
    acc: &mut candle_core::backprop::GradStore,
    src: &candle_core::backprop::GradStore,
    vars: &[candle_core::Var],
) -> anyhow::Result<()> {
    for v in vars {
        let t = v.as_tensor();
        if let Some(g) = src.get(t) {
            let merged = match acc.get(t) {
                Some(existing) => existing.add(g)?,
                None => g.clone(),
            };
            acc.insert(t, merged);
        }
    }
    Ok(())
}

/// Global-norm gradient clipping (PPO-standard, à la
/// `torch.nn.utils.clip_grad_norm_`). Computes the L2 norm over every var's
/// gradient in `grads`; if it exceeds `max_norm`, scales all of them in place
/// by `max_norm / (total_norm + 1e-6)` so the combined gradient has norm
/// `max_norm`. `max_norm <= 0` is a no-op (preserves the un-clipped baseline
/// numerics bit-for-bit). Returns the pre-clip global norm for logging.
///
/// Operates on the whole var set as ONE vector (not per-tensor), so it's
/// invariant to how the gradient was assembled — the monolithic and chunked
/// update paths build the identical combined gradient, hence clip to the
/// identical result.
fn clip_grad_norm(
    grads: &mut candle_core::backprop::GradStore,
    vars: &[candle_core::Var],
    max_norm: f64,
) -> anyhow::Result<f32> {
    // Sum of squares across every var's gradient.
    let mut sumsq = 0.0f64;
    for v in vars {
        let t = v.as_tensor();
        if let Some(g) = grads.get(t) {
            sumsq += g.sqr()?.sum_all()?.to_scalar::<f32>()? as f64;
        }
    }
    let total_norm = sumsq.sqrt();
    if max_norm > 0.0 && total_norm > max_norm {
        let scale = max_norm / (total_norm + 1e-6);
        for v in vars {
            let t = v.as_tensor();
            // Borrow of `g` ends inside the match arm (affine clones), so the
            // subsequent insert's mutable borrow doesn't conflict.
            let scaled = match grads.get(t) {
                Some(g) => Some(g.affine(scale, 0.0)?),
                None => None,
            };
            if let Some(s) = scaled {
                grads.insert(t, s);
            }
        }
    }
    Ok(total_norm as f32)
}

/// M13: value loss, optionally PPO-clipped. With `clip == None` (or `<= 0`)
/// this is plain MSE — BIT-IDENTICAL to the previous `(pred - ret).sqr().mean_all()`
/// so the 47% baseline and chunked==monolithic equivalence are unaffected. With
/// `Some(c)`, the predicted value is clamped to `old_value ± c` and the loss is
/// `max(unclipped_mse, clipped_mse)` per row, then averaged (same scheme as the
/// flat `PpoTrainer::ppo_update`). Mirrors `ppo.rs`.
fn clipped_value_loss(
    value_pred: &Tensor,
    ret: &Tensor,
    old_value: &Tensor,
    clip: Option<f32>,
) -> anyhow::Result<Tensor> {
    let unclipped = (value_pred - ret)?.sqr()?;
    match clip {
        Some(c) if c > 0.0 => {
            let delta = (value_pred - old_value)?;
            let clamped = delta.clamp(-c, c)?;
            let v_clipped = (old_value + &clamped)?;
            let clipped_mse = (&v_clipped - ret)?.sqr()?;
            Ok(unclipped.maximum(&clipped_mse)?.mean_all()?)
        }
        _ => Ok(unclipped.mean_all()?),
    }
}

/// Standard PPO advantage normalization (zero mean, unit std).
///
/// M14: with fewer than 2 elements the std is meaningless and `(x-mean)/std`
/// would collapse a singleton buffer's advantage to ~0, zeroing the policy
/// gradient while value loss still trains. In that degenerate case we
/// mean-subtract only (a 1-element buffer just becomes 0 after subtracting its
/// own mean — but we skip the spurious `/std` blow-up that depends on eps).
fn normalize_adv(adv: &[f32]) -> Vec<f32> {
    if adv.len() < 2 {
        let mean = adv.iter().sum::<f32>() / adv.len().max(1) as f32;
        return adv.iter().map(|x| x - mean).collect();
    }
    let n = adv.len() as f32;
    let mean = adv.iter().sum::<f32>() / n;
    let var = adv.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
    let std = (var + 1e-8).sqrt();
    adv.iter().map(|x| (x - mean) / std).collect()
}

/// Cumulative cross-colony losses (never reset by the sim). H7/M15: the
/// per-window delta is the window-summed combat losses fed to `ColonyMetrics`.
fn colony_combat_losses(env: &MatchEnv, k: u8) -> u32 {
    env.sim.colonies.get(k as usize).map(|c| c.combat_losses).unwrap_or(0)
}

/// Decode one row of a [B, 6] post-squash action tensor into an AiDecision.
fn row_to_decision(action_batch: &candle_core::Tensor, row: usize) -> anyhow::Result<AiDecision> {
    let r = action_batch.narrow(0, row, 1)?.squeeze(0)?; // [6]
    let v: Vec<f32> = r.to_vec1()?;
    // L11: document the 6-dim action invariant (can't fire — dims pinned).
    debug_assert_eq!(v.len(), 6, "row_to_decision expects 6 action dims");
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

/// One tick's batch of ant decisions — the whole `[Mt, …]` tensor from one
/// `sample_ant` call, kept batched rather than split per ant. This is the
/// load-bearing perf choice: with parallel envs a tick produces hundreds of
/// ants and a match tens of thousands; storing/`cat`-ing them per-ant did one
/// GPU tensor + handle per ant (≈50 min/iter on CUDA). One batch tensor per
/// tick (≈80 per iter) keeps the GPU `cat` cheap. Per-row metadata
/// (`match_idx`/`colony`/`log_prob`/`value`) is host-side `Vec`s.
pub struct AntBatch {
    /// Per-row source match/env index — the GAE bucket key (with `colony`).
    pub match_idx: Vec<usize>,
    /// Per-row colony id (self-play has both; left-vs-league is all 0).
    pub colony: Vec<u8>,
    /// Decision cycle this batch belongs to (shared by all rows).
    pub cycle: usize,
    pub cone: Tensor,      // [Mt, 60]
    pub internal: Tensor,  // [Mt, 8]
    pub intent: Tensor,    // [Mt, 64]
    pub modulator: Tensor, // [Mt, 5] post-squash
    pub log_prob: Vec<f32>, // [Mt]
    pub value: Vec<f32>,    // [Mt]
}

impl AntBatch {
    /// Number of ant rows in this batch.
    pub fn len(&self) -> usize {
        self.log_prob.len()
    }
    pub fn is_empty(&self) -> bool {
        self.log_prob.is_empty()
    }
}

#[derive(Default)]
pub struct JointRollout {
    pub commander: Vec<CommanderRecord>,
    /// Ant samples, grouped one `AntBatch` per `sample_ant` call (per tick).
    pub ant: Vec<AntBatch>,
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
        // Ant batches: tensors are [Mt, ..] with Mt == per-row metadata len,
        // all finite.
        for a in &roll.ant {
            let mt = a.len();
            assert!(mt >= 1);
            assert_eq!(a.cone.dims(), &[mt, 60]);
            assert_eq!(a.internal.dims(), &[mt, 8]);
            assert_eq!(a.intent.dims(), &[mt, 64]);
            assert_eq!(a.modulator.dims(), &[mt, 5]);
            assert_eq!(a.match_idx.len(), mt);
            assert_eq!(a.colony.len(), mt);
            assert_eq!(a.value.len(), mt);
            assert!(a.log_prob.iter().chain(a.value.iter()).all(|x| x.is_finite()));
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

        let total_ant_rows: usize = roll.ant.iter().map(|b| b.len()).sum();
        let (aadv, aret) = t.ant_advantages(&roll.commander, &roll.ant);
        assert_eq!(aadv.len(), total_ant_rows);
        assert_eq!(aret.len(), total_ant_rows);
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

    #[test]
    fn clip_grad_norm_scales_to_max_and_leaves_small_grads() {
        use candle_core::Var;
        let dev = Device::Cpu;
        // x = [3, 4]; loss = 0.5·Σx²  →  grad = x, global L2 norm = 5.
        let x = Var::new(&[3.0f32, 4.0], &dev).unwrap();
        let vars = vec![x.clone()];
        let loss = || x.as_tensor().sqr().unwrap().sum_all().unwrap().affine(0.5, 0.0).unwrap();

        // max_norm = 1 → grad scaled to [0.6, 0.8] (norm 1); pre-clip norm = 5.
        let mut g = loss().backward().unwrap();
        let pre = clip_grad_norm(&mut g, &vars, 1.0).unwrap();
        assert!((pre - 5.0).abs() < 1e-4, "pre-clip norm = {pre}");
        let gv = g.get(x.as_tensor()).unwrap().to_vec1::<f32>().unwrap();
        let post = (gv[0] * gv[0] + gv[1] * gv[1]).sqrt();
        assert!((post - 1.0).abs() < 1e-3, "post-clip norm = {post}, g = {gv:?}");
        assert!((gv[0] - 0.6).abs() < 1e-3 && (gv[1] - 0.8).abs() < 1e-3, "g = {gv:?}");

        // max_norm above the actual norm → untouched.
        let mut g2 = loss().backward().unwrap();
        clip_grad_norm(&mut g2, &vars, 100.0).unwrap();
        let gv2 = g2.get(x.as_tensor()).unwrap().to_vec1::<f32>().unwrap();
        assert!((gv2[0] - 3.0).abs() < 1e-5 && (gv2[1] - 4.0).abs() < 1e-5, "g2 = {gv2:?}");

        // max_norm = 0 (disabled) → untouched even when over threshold.
        let mut g3 = loss().backward().unwrap();
        clip_grad_norm(&mut g3, &vars, 0.0).unwrap();
        let gv3 = g3.get(x.as_tensor()).unwrap().to_vec1::<f32>().unwrap();
        assert!((gv3[0] - 3.0).abs() < 1e-5 && (gv3[1] - 4.0).abs() < 1e-5, "g3 = {gv3:?}");
    }

    #[test]
    fn chunked_matches_monolithic_with_grad_clip() {
        // Grad clipping must not break the chunked/monolithic equivalence:
        // both build the identical combined gradient, so clipping to a shared
        // global norm yields the identical post-clip gradient and Adam step.
        let mut t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        t.config.max_grad_norm = 0.5; // force a clip well below the raw norm
        let roll = t.rollout(0xc0ff_ee, 0).unwrap();
        let an: usize = roll.ant.iter().map(|b| b.len()).sum();
        assert!(an > 4, "need >4 ant rows to exercise chunking, got {an}");

        let vars = t.varmap.all_vars();
        let flat = |v: &candle_core::Var| -> Vec<f32> {
            v.as_tensor().flatten_all().unwrap().to_vec1::<f32>().unwrap()
        };
        let snap: Vec<Tensor> = vars.iter().map(|v| {
            let t = v.as_tensor();
            Tensor::from_vec(flat(v), t.dims().to_vec(), &Device::Cpu).unwrap()
        }).collect();

        t.config.ant_chunk_size = 0;
        let mut opt = t.make_optimizer().unwrap();
        t.joint_update(&mut opt, &roll).unwrap();
        let mono: Vec<Vec<f32>> = vars.iter().map(|v| flat(v)).collect();

        for (v, s) in vars.iter().zip(&snap) {
            v.set(s).unwrap();
        }
        t.config.ant_chunk_size = (an / 4).max(2);
        let mut opt2 = t.make_optimizer().unwrap();
        t.joint_update(&mut opt2, &roll).unwrap();
        let chunked: Vec<Vec<f32>> = vars.iter().map(|v| flat(v)).collect();

        let mut max_diff = 0.0f32;
        for (a, b) in mono.iter().zip(chunked.iter()) {
            for (x, y) in a.iter().zip(b.iter()) {
                max_diff = max_diff.max((x - y).abs());
            }
        }
        assert!(max_diff < 1e-3, "clipped chunked vs monolithic diverged: max_diff={max_diff}");
    }

    #[test]
    fn chunked_ant_update_matches_monolithic() {
        // The minibatched ant path must produce the SAME parameter update as
        // the monolithic one-forward-over-all-rows path — only its peak memory
        // differs. Run both from identical init + a fresh optimizer over the
        // same rollout, compare every weight.
        let mut t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let roll = t.rollout(0xc0ff_ee, 0).unwrap();
        let an: usize = roll.ant.iter().map(|b| b.len()).sum();
        assert!(an > 4, "need >4 ant rows to exercise chunking, got {an}");

        let vars = t.varmap.all_vars();
        let flat = |v: &candle_core::Var| -> Vec<f32> {
            v.as_tensor().flatten_all().unwrap().to_vec1::<f32>().unwrap()
        };
        // Snapshot as fully independent tensors (Var::set rejects a tensor
        // derived from the var's own value, so clone()/detach() won't do).
        let snap: Vec<Tensor> = vars.iter().map(|v| {
            let t = v.as_tensor();
            let dims = t.dims().to_vec();
            Tensor::from_vec(flat(v), dims, &Device::Cpu).unwrap()
        }).collect();

        // Monolithic update (chunking disabled).
        t.config.ant_chunk_size = 0;
        let mut opt = t.make_optimizer().unwrap();
        let s_mono = t.joint_update(&mut opt, &roll).unwrap();
        let mono: Vec<Vec<f32>> = vars.iter().map(|v| flat(v)).collect();

        // Restore init, redo with several ant chunks.
        for (v, s) in vars.iter().zip(&snap) {
            v.set(s).unwrap();
        }
        t.config.ant_chunk_size = (an / 4).max(2); // ~4 chunks
        let mut opt2 = t.make_optimizer().unwrap();
        let s_chunk = t.joint_update(&mut opt2, &roll).unwrap();
        let chunked: Vec<Vec<f32>> = vars.iter().map(|v| flat(v)).collect();

        // Loss scalars must match: the monolithic path computes the TRUE
        // full-batch mean independently, so a match proves the per-chunk
        // weighting (len/n) is correct in both the reported loss AND the
        // gradient (they share `w`). This is what catches a pure
        // gradient-scale error, which Adam's sign-only first step would hide
        // in the post-step weights.
        assert!((s_mono.commander - s_chunk.commander).abs() < 1e-4,
            "commander loss diverged: {} vs {}", s_mono.commander, s_chunk.commander);
        assert!((s_mono.ant - s_chunk.ant).abs() < 1e-3,
            "ant loss diverged: {} vs {}", s_mono.ant, s_chunk.ant);
        assert!((s_mono.total - s_chunk.total).abs() < 1e-3,
            "total loss diverged: {} vs {}", s_mono.total, s_chunk.total);

        // Post-Adam weights must match too (catches sign-structure /
        // multi-step bugs). Tolerance sits above the f32 + Adam near-zero
        // sign-flip floor (~2·lr) yet far below any real logic bug (O(0.01+)).
        let mut max_diff = 0.0f32;
        for (a, b) in mono.iter().zip(chunked.iter()) {
            for (x, y) in a.iter().zip(b.iter()) {
                max_diff = max_diff.max((x - y).abs());
            }
        }
        assert!(
            max_diff < 1e-3,
            "chunked vs monolithic param update diverged: max_diff={max_diff}"
        );
    }
}

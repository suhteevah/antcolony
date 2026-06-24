//! PPO trainer driver. Coordinates rollout collection, GAE computation,
//! and the policy update.

use crate::{ActorCritic, MatchEnv, League, INPUT_DIM, OUTPUT_DIM};
use crate::backend::state_to_tensor;
use antcolony_sim::AiDecision;
use candle_core::{DType, Device, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};

#[derive(Clone, Debug)]
pub struct PpoConfig {
    pub iterations: usize,
    pub matches_per_iter: usize,
    pub gamma: f32,
    pub gae_lambda: f32,
    pub clip: f32,
    pub epochs_per_batch: usize,
    pub minibatch_size: usize,
    pub lr: f64,
    pub value_coef: f64,
    pub entropy_coef: f64,
    pub max_grad_norm: f64,
    pub eval_every: usize,
    pub snapshot_every: usize,
    /// Range for value-loss clipping (PPO's value-clip trick). Limits
    /// how far value_pred can move from old_value in a single update,
    /// preventing the 115k+ loss spikes that destabilized r5 when
    /// novel pop-based opponents entered the league. Set 0 to disable.
    pub value_clip: f32,
    /// Hidden-layer width for the actor & critic MLPs. Default 64
    /// (matches mlp_weights_v1). Bumping to 128 doubles capacity and
    /// is one of the candidate architectural changes for breaking the
    /// 47% Nash plateau. The exported MLP JSON encodes hidden_dim, and
    /// `MlpBrain::load` reads dims from the matrix shapes — so any
    /// width round-trips into the existing inference path.
    pub hidden_dim: usize,
}

impl Default for PpoConfig {
    fn default() -> Self {
        Self {
            iterations: 50,
            matches_per_iter: 32,
            gamma: 0.99,
            gae_lambda: 0.95,
            clip: 0.2,
            // Tuning pass r2: dropped from 4 to 1. Multiple epochs on the
            // same batch were over-correcting the warm-start policy.
            epochs_per_batch: 1,
            minibatch_size: 256,
            // Tuning pass r4: 1e-4 -> 5e-4. r3's tiny LR moved weights
            // by max 0.015 over 500 iters — too small to escape the BC
            // local optimum. Bigger steps + accept some risk of
            // degradation to actually explore the policy space.
            lr: 5e-4,
            value_coef: 0.5,
            // Tuning pass r4: 0.003 -> 0.005. r3's middle ground still
            // didn't move eval — model needs a stronger exploration push
            // to escape the BC local optimum.
            entropy_coef: 0.005,
            max_grad_norm: 0.5,
            eval_every: 5,
            snapshot_every: 10,
            // Default off — opt-in via PpoConfig override. When enabled
            // (e.g. 0.2), the value head's per-step move is clipped to
            // ±value_clip around the rollout-time prediction.
            value_clip: 0.0,
            hidden_dim: crate::HIDDEN_DIM,
        }
    }
}

/// Cross-species curriculum: when set on a `PpoTrainer`, each rollout is played
/// in the cross-species arena instead of the default same-species `MatchEnv::new`.
/// The learner (colony 0) and opponent (colony 1) are assigned species drawn
/// deterministically from `roster` per rollout seed, so over an iteration the
/// policy faces the whole intransitive matchup cycle (it can't collapse onto a
/// single dominant counter-strategy). `venom_cycle_strength` arms the cyclic
/// clade type-chart; `nest` selects the 5-module underground-nest arena.
#[derive(Clone)]
pub struct CrossSpeciesCurriculum {
    pub roster: std::sync::Arc<Vec<antcolony_sim::species::Species>>,
    pub venom_cycle_strength: f32,
    pub nest: bool,
}

pub struct PpoTrainer {
    pub policy: ActorCritic,
    pub varmap: VarMap,
    pub device: Device,
    pub league: League,
    pub config: PpoConfig,
    pub rng: rand_chacha::ChaCha8Rng,
    /// `None` ⇒ legacy same-species training (byte-identical). `Some` ⇒ each
    /// rollout is a cross-species match drawn from the curriculum roster.
    pub cross_species: Option<CrossSpeciesCurriculum>,
}

impl PpoTrainer {
    pub fn new(device: Device, config: PpoConfig) -> anyhow::Result<Self> {
        use rand::SeedableRng;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let policy = ActorCritic::new(vb, config.hidden_dim, &device)?;
        Ok(Self {
            policy,
            varmap,
            device,
            league: League::default_pool(),
            config,
            rng: rand_chacha::ChaCha8Rng::seed_from_u64(0xa17c01),
            cross_species: None,
        })
    }

    /// Decode a 6-dim squashed-tanh action tensor into an AiDecision.
    /// The 3 caste params + 3 behavior params are softmaxed separately
    /// at the sim layer; here we just hand off the raw values.
    pub fn tensor_to_decision(action: &Tensor) -> anyhow::Result<AiDecision> {
        // action is [1, 6]; flatten to [6] before extracting
        let flat = if action.dims().len() == 2 { action.squeeze(0)? } else { action.clone() };
        let v: Vec<f32> = flat.to_vec1()?;
        // L11: document the invariant — the action head is fixed at 6 dims
        // (OUTPUT_DIM). Can't fire today, but per the no-panic rule.
        debug_assert_eq!(v.len(), OUTPUT_DIM, "tensor_to_decision expects {OUTPUT_DIM} action dims");
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

    /// Run one match against `opp_spec`, collect rollouts.
    /// Returns (states, actions, log_probs, rewards, values, dones).
    pub fn rollout(&mut self, opp_spec: &str, seed: u64) -> anyhow::Result<RolloutBatch> {
        // Cross-species curriculum: assign the learner (colony 0) and opponent
        // (colony 1) species deterministically from the roster per seed, so the
        // policy faces the whole intransitive cycle across an iteration. Default
        // (None) is the legacy same-species env — byte-identical.
        let mut env = match &self.cross_species {
            Some(cur) if !cur.roster.is_empty() => {
                let n = cur.roster.len() as u64;
                let a = (seed % n) as usize;
                // Offset so a != b whenever n > 1 (avoid mirror matches dominating).
                let b = (((seed / n) % (n.saturating_sub(1).max(1))) as usize + a + 1) % cur.roster.len();
                let sp_a = &cur.roster[a];
                let sp_b = &cur.roster[b];
                let mut e = if cur.nest {
                    MatchEnv::new_cross_species_nest_arena(sp_a, sp_b, seed)
                } else {
                    MatchEnv::new_cross_species_arena(sp_a, sp_b, seed)
                };
                e.sim.config.combat.venom_cycle_strength = cur.venom_cycle_strength;
                e
            }
            _ => MatchEnv::new(seed),
        };
        let mut opp = League::make_brain(opp_spec, seed.wrapping_add(1))?;
        let mut batch = RolloutBatch::default();

        loop {
            let s_left = match env.observe(0) {
                Some(s) => s,
                None => break,
            };
            let s_tensor = state_to_tensor(&s_left, &self.device)?;
            // Sample stochastic action
            let (action_t, log_prob_t) = self.policy.sample(&s_tensor, &mut self.rng)?;
            let value_t = self.policy.value(&s_tensor)?;
            let action_dec = Self::tensor_to_decision(&action_t)?;
            let s_right = env.observe(1);
            let action_right = match s_right.as_ref() {
                Some(sr) => opp.decide(sr),
                None => AiDecision { caste_ratio_worker: 0.65, caste_ratio_soldier: 0.30, caste_ratio_breeder: 0.05, forage_weight: 0.55, dig_weight: 0.20, nurse_weight: 0.25, research_choice: None },
            };
            let step = env.step(&action_dec, &action_right);
            batch.states.push(s_tensor.detach());
            batch.actions.push(action_t.detach());
            batch.log_probs.push(log_prob_t.detach().to_scalar::<f32>()?);
            // value_t may be rank 1 ([1]) or rank 0 depending on squeeze path; coerce.
            let v_flat = value_t.detach();
            let v_scalar = if v_flat.dims().len() == 0 { v_flat.to_scalar::<f32>()? }
                else { v_flat.squeeze(0)?.to_scalar::<f32>()? };
            batch.values.push(v_scalar);
            batch.rewards.push(step.reward_left);
            batch.dones.push(step.done);
            if step.done || env.sim.tick >= env.max_ticks {
                // H6: if we're stopping with the last step NON-terminal (a pure
                // horizon truncation — the sim is still InProgress), capture a
                // detached V(s_n) on the post-rollout left observation so GAE
                // bootstraps from a real estimate instead of 0. A genuine
                // terminal (step.done with a Won/Draw status) leaves last_value
                // None and GAE bootstraps from 0.
                if !step.done {
                    if let Some(s_next) = env.observe(0) {
                        let s_next_t = state_to_tensor(&s_next, &self.device)?;
                        let v = self.policy.value(&s_next_t)?.detach();
                        let v_scalar = if v.dims().is_empty() {
                            v.to_scalar::<f32>()?
                        } else {
                            v.squeeze(0)?.to_scalar::<f32>()?
                        };
                        batch.last_value = Some(v_scalar);
                    }
                }
                break;
            }
        }
        Ok(batch)
    }

    /// Warm-start the actor weights from an MlpBrain JSON file via VarMap.
    /// Critic stays at random init (BC has no value head). Names follow
    /// the VarBuilder.pp() prefixes: actor_l1.weight, actor_l1.bias, etc.
    pub fn warm_start_actor(&mut self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        let raw = std::fs::read_to_string(path.as_ref())?;
        let d: serde_json::Value = serde_json::from_str(&raw)?;
        let to_2d = |v: &serde_json::Value| -> Vec<Vec<f32>> {
            v.as_array().unwrap().iter().map(|row|
                row.as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect()).collect()
        };
        let to_1d = |v: &serde_json::Value| -> Vec<f32> {
            v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect()
        };
        let dev = &self.device;
        let im = to_1d(&d["input_mean"]);
        let isd = to_1d(&d["input_std"]);
        let w1 = to_2d(&d["w1"]); let b1 = to_1d(&d["b1"]);
        let w2 = to_2d(&d["w2"]); let b2 = to_1d(&d["b2"]);
        let w3 = to_2d(&d["w3"]); let b3 = to_1d(&d["b3"]);
        let h1 = w1.len(); let h2 = w2.len();
        if h1 != self.config.hidden_dim || h2 != self.config.hidden_dim {
            anyhow::bail!(
                "warm_start_actor: dim mismatch — model hidden_dim={} but file is {}x{}. \
                 Either retrain at hidden_dim={} or pass --hidden-dim {} to ppo-train.",
                self.config.hidden_dim, h1, h2, h1, h1
            );
        }
        let flat = |w: Vec<Vec<f32>>| -> Vec<f32> { w.into_iter().flatten().collect() };

        // Update normalization buffers (not Vars — direct field assign)
        self.policy.input_mean = Tensor::from_vec(im, (INPUT_DIM,), dev)?;
        self.policy.input_std = Tensor::from_vec(isd, (INPUT_DIM,), dev)?;

        // Update Var-backed weights via VarMap.set_one (path matches vb.pp() prefixes)
        let updates: Vec<(&str, Tensor)> = vec![
            ("actor_l1.weight", Tensor::from_vec(flat(w1), (h1, INPUT_DIM), dev)?),
            ("actor_l1.bias",   Tensor::from_vec(b1,        (h1,),          dev)?),
            ("actor_l2.weight", Tensor::from_vec(flat(w2), (h2, h1),       dev)?),
            ("actor_l2.bias",   Tensor::from_vec(b2,        (h2,),          dev)?),
            ("actor_l3.weight", Tensor::from_vec(flat(w3), (OUTPUT_DIM, h2), dev)?),
            ("actor_l3.bias",   Tensor::from_vec(b3,        (OUTPUT_DIM,), dev)?),
        ];
        for (name, t) in updates {
            self.varmap.set_one(name, &t)?;
        }
        // Re-bind the policy's Linear references to read from the updated VarMap.
        // VarMap.set_one mutates the underlying Var in place, so subsequent
        // forward passes through the same Linear pick up the new weights
        // automatically (Linear holds the Var, not a snapshot).
        Ok(())
    }

    /// Build an AdamW optimizer over all VarMap params.
    pub fn make_optimizer(&self) -> anyhow::Result<AdamW> {
        let params = ParamsAdamW {
            lr: self.config.lr,
            beta1: 0.9, beta2: 0.999, eps: 1e-8,
            weight_decay: 0.0,
        };
        Ok(AdamW::new(self.varmap.all_vars(), params)?)
    }

    /// One PPO update over a batch. Returns mean loss for logging.
    /// Stacks (states, actions, returns, advantages, old_log_probs)
    /// across the rollout, computes the clipped surrogate, value MSE,
    /// and entropy bonus, runs `epochs_per_batch` passes.
    pub fn ppo_update(
        &mut self,
        opt: &mut AdamW,
        states: &[Tensor],          // each [1, 17]
        actions: &[Tensor],         // each [1, 6]
        returns: &[f32],
        advantages: &[f32],
        old_log_probs: &[f32],
        old_values: &[f32],         // value head output captured at rollout time
    ) -> anyhow::Result<f32> {
        // Concat [N, 17] / [N, 6] / [N] tensors once
        let s = Tensor::cat(states, 0)?;        // [N, 17]
        let a = Tensor::cat(actions, 0)?;       // [N, 6]
        let n = states.len();
        let rt = Tensor::from_slice(returns, n, &self.device)?;
        // Normalize advantages — standard PPO trick. M14: with < 2 samples,
        // mean-subtract only; the `/std` would collapse a singleton buffer's
        // advantage to ~0 and zero the policy gradient.
        let mean_adv: f32 = advantages.iter().sum::<f32>() / n as f32;
        let normed: Vec<f32> = if n < 2 {
            advantages.iter().map(|x| x - mean_adv).collect()
        } else {
            let var_adv: f32 = advantages.iter().map(|x| (x - mean_adv).powi(2)).sum::<f32>() / n as f32;
            let std_adv = (var_adv + 1e-8).sqrt();
            advantages.iter().map(|x| (x - mean_adv) / std_adv).collect()
        };
        let adv = Tensor::from_slice(&normed, n, &self.device)?;
        let old_lp = Tensor::from_slice(old_log_probs, n, &self.device)?;
        let old_v = Tensor::from_slice(old_values, n, &self.device)?;

        let mut loss_sum = 0.0_f32;
        let mut steps = 0;
        for _epoch in 0..self.config.epochs_per_batch {
            // For PPO with our scalar log_prob (sum over action dims), we
            // recompute log_prob under current policy as a single tensor.
            // We process the WHOLE batch as one minibatch (batch sizes are
            // small ~thousands; minibatching can come later).
            let new_lp = self.batched_log_prob(&s, &a)?;       // [N]
            let value_pred = self.policy.value(&s)?;           // [N]
            // Ratio = exp(new_lp - old_lp)
            let log_ratio = (&new_lp - &old_lp)?;
            let ratio = log_ratio.exp()?;
            let surr1 = (&ratio * &adv)?;
            let lo = 1.0 - self.config.clip;
            let hi = 1.0 + self.config.clip;
            let clipped_ratio = ratio.clamp(lo, hi)?;
            let surr2 = (&clipped_ratio * &adv)?;
            // Negative because we minimize; PPO maximizes expected surrogate
            let policy_loss = surr1.minimum(&surr2)?.mean_all()?.affine(-1.0, 0.0)?;
            // Value loss — clipped (PPO-style) when value_clip > 0,
            // standard MSE otherwise. The clipped variant prevents the
            // value head from moving more than ±clip away from its
            // rollout-time prediction in a single update; we then take
            // the *max* of clipped vs unclipped MSE so the loss is a
            // pessimistic bound on how far we'd let the value head move.
            // Without this, novel pop-based opponents drove 115k+ loss
            // spikes on r5 and 40M+ spikes on r6.
            let val_diff = (&value_pred - &rt)?;
            let unclipped_mse = val_diff.sqr()?;
            let value_loss = if self.config.value_clip > 0.0 {
                let clip = self.config.value_clip;
                let delta = (&value_pred - &old_v)?;
                let clamped = delta.clamp(-clip, clip)?;
                let v_clipped = (&old_v + &clamped)?;
                let clipped_diff = (&v_clipped - &rt)?;
                let clipped_mse = clipped_diff.sqr()?;
                unclipped_mse.maximum(&clipped_mse)?.mean_all()?
            } else {
                unclipped_mse.mean_all()?
            };
            // Entropy bonus (encourage exploration)
            // entropy of Normal(mean, std) = sum(log_std) + 0.5 * D * (1 + log(2*pi))
            // log_std is shared across batch, so this is constant per pass —
            // still differentiable through log_std (a Var)
            let entropy_per_dim = self.policy.log_std.affine(1.0, 0.5_f64 * (1.0_f64 + (2.0_f64 * std::f64::consts::PI).ln()))?;
            let entropy = entropy_per_dim.sum_all()?;
            // Total loss: policy + 0.5*value - 0.01*entropy
            let total = ((&policy_loss + value_loss.affine(self.config.value_coef, 0.0)?)?
                - entropy.affine(self.config.entropy_coef, 0.0)?)?;
            opt.backward_step(&total)?;
            loss_sum += total.to_scalar::<f32>().unwrap_or(0.0);
            steps += 1;
        }
        Ok(loss_sum / steps.max(1) as f32)
    }

    /// Recompute log_prob over a BATCH of states + actions.
    fn batched_log_prob(&self, states: &Tensor, actions: &Tensor) -> anyhow::Result<Tensor> {
        let mean = self.policy.actor_mean(states)?;     // [N, 6]
        let std = self.policy.log_std.exp()?;            // [6]
        // Invert squash: u = atanh(2a - 1)
        let two_a = actions.affine(2.0, -1.0)?;
        let clamped = two_a.clamp(-0.999999_f32, 0.999999_f32)?;
        let one = Tensor::ones_like(&clamped)?;
        let plus = (&one + &clamped)?;
        let minus = (&one - &clamped)?;
        let u = (plus / minus)?.log()?.affine(0.5, 0.0)?;
        let diff = (&u - &mean)?;
        let std_sq = std.broadcast_mul(&std)?;
        let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
        let two_pi_log = 0.9189385332_f64;
        let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
        let log_pdf = log_pdf_part1.broadcast_sub(&self.policy.log_std)?;
        let tanh_u = u.tanh()?;
        let one_t = Tensor::ones_like(&tanh_u)?;
        let one_minus_tanh_sq = (&one_t - &(&tanh_u * &tanh_u)?)?;
        let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -0.6931472_f64)?;
        // Per-action log_prob = sum over dims
        let log_prob = (log_pdf - log_jac)?.sum(candle_core::D::Minus1)?;  // [N]
        Ok(log_prob)
    }

    /// Compute Generalized Advantage Estimation.
    ///
    /// Thin wrapper over [`compute_gae_bootstrap`] with `last_value = None`,
    /// i.e. the truncated tail bootstraps from the last collected step's own
    /// value estimate. Kept for callers that don't have a post-rollout value.
    pub fn compute_gae(rewards: &[f32], values: &[f32], dones: &[bool], gamma: f32, lambda: f32) -> (Vec<f32>, Vec<f32>) {
        Self::compute_gae_bootstrap(rewards, values, dones, gamma, lambda, None)
    }

    /// Compute GAE, correctly handling horizon-TRUNCATION (the common case in
    /// our rollouts) vs genuine TERMINATION.
    ///
    /// H6 fix: previously the final collected step always bootstrapped from
    /// `next_value = 0.0`. Because rollouts are almost always cut off by the
    /// cycle cap with `done == false` (a *truncation*, not a terminal), this
    /// injected a spurious `delta = r - V(s_last)` at every rollout tail and
    /// biased value targets low.
    ///
    /// Now: when the last collected step is non-terminal (`!dones[n-1]`), we
    /// bootstrap its `next_value` from a real estimate of `V(s_n)` (the
    /// post-rollout state) with `next_nonterminal = 1`. `last_value` supplies
    /// that estimate; when `None` we fall back to the last collected step's
    /// own value (`values[n-1]`) — a documented, less-invasive bootstrap that
    /// still avoids the V=0 pessimism. Genuine terminals (`dones[n-1]`) still
    /// bootstrap from 0 as before.
    ///
    /// Note: only the FINAL step's `next_value` is affected; interior steps are
    /// unchanged, so the chunked==monolithic equivalence (which feeds both
    /// paths the identical GAE output) is untouched.
    pub fn compute_gae_bootstrap(
        rewards: &[f32],
        values: &[f32],
        dones: &[bool],
        gamma: f32,
        lambda: f32,
        last_value: Option<f32>,
    ) -> (Vec<f32>, Vec<f32>) {
        let n = rewards.len();
        let mut advantages = vec![0.0_f32; n];
        let mut returns = vec![0.0_f32; n];
        let mut gae = 0.0_f32;
        for t in (0..n).rev() {
            let next_value = if t + 1 < n {
                values[t + 1]
            } else if dones[t] {
                // Genuine terminal: no future value.
                0.0
            } else {
                // Horizon-truncated tail: bootstrap from a real next-state
                // value estimate instead of 0. Prefer the supplied
                // post-rollout V(s_n); else fall back to this step's own value.
                last_value.unwrap_or_else(|| values[t])
            };
            let next_nonterminal = if dones[t] { 0.0 } else { 1.0 };
            let delta = rewards[t] + gamma * next_value * next_nonterminal - values[t];
            gae = delta + gamma * lambda * next_nonterminal * gae;
            advantages[t] = gae;
            returns[t] = gae + values[t];
        }
        (advantages, returns)
    }

    /// Export the current actor weights in the MlpBrain JSON format the
    /// Rust sim already loads (via `mlp:<path>` spec).
    pub fn export_mlp_weights(&self, path: &std::path::Path) -> anyhow::Result<()> {
        // Pull weights as Vec<Vec<f32>> for each Linear layer
        let extract = |lin: &candle_nn::Linear| -> anyhow::Result<(Vec<Vec<f32>>, Vec<f32>)> {
            let w = lin.weight().to_vec2::<f32>()?;
            let b: Vec<f32> = lin.bias().map(|b| b.to_vec1::<f32>()).transpose()?
                .unwrap_or_else(|| vec![0.0; w.len()]);
            Ok((w, b))
        };
        let (w1, b1) = extract(&self.policy.actor_l1)?;
        let (w2, b2) = extract(&self.policy.actor_l2)?;
        let (w3, b3) = extract(&self.policy.actor_l3)?;
        let input_mean = self.policy.input_mean.to_vec1::<f32>()?;
        let input_std = self.policy.input_std.to_vec1::<f32>()?;
        let out = serde_json::json!({
            "input_dim": INPUT_DIM,
            "hidden_dim": w1.len(),
            "output_dim": OUTPUT_DIM,
            "input_mean": input_mean,
            "input_std": input_std,
            "w1": w1, "b1": b1,
            "w2": w2, "b2": b2,
            "w3": w3, "b3": b3,
        });
        std::fs::write(path, serde_json::to_string(&out)?)?;
        tracing::info!(path = %path.display(), "exported mlp weights");
        Ok(())
    }
}

#[derive(Default)]
pub struct RolloutBatch {
    pub states: Vec<Tensor>,
    pub actions: Vec<Tensor>,
    pub log_probs: Vec<f32>,
    pub values: Vec<f32>,
    pub rewards: Vec<f32>,
    pub dones: Vec<bool>,
    /// H6: detached `V(s_n)` for the post-rollout state when the rollout ended
    /// by horizon TRUNCATION (last step non-terminal). `None` on a genuine
    /// terminal — GAE then bootstraps the tail from 0.
    pub last_value: Option<f32>,
}

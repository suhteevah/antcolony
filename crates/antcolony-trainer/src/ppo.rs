//! PPO trainer driver. Coordinates rollout collection, GAE computation,
//! and the policy update.

use crate::{ActorCritic, MatchEnv, League, INPUT_DIM, OUTPUT_DIM};
use crate::backend::state_to_tensor;
use antcolony_sim::AiDecision;
use candle_core::{DType, Device, Tensor};
use candle_nn::{VarBuilder, VarMap};

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
}

impl Default for PpoConfig {
    fn default() -> Self {
        Self {
            iterations: 50,
            matches_per_iter: 32,
            gamma: 0.99,
            gae_lambda: 0.95,
            clip: 0.2,
            epochs_per_batch: 4,
            minibatch_size: 256,
            lr: 3e-4,
            value_coef: 0.5,
            entropy_coef: 0.01,
            max_grad_norm: 0.5,
            eval_every: 5,
            snapshot_every: 10,
        }
    }
}

pub struct PpoTrainer {
    pub policy: ActorCritic,
    pub varmap: VarMap,
    pub device: Device,
    pub league: League,
    pub config: PpoConfig,
    pub rng: rand_chacha::ChaCha8Rng,
}

impl PpoTrainer {
    pub fn new(device: Device, config: PpoConfig) -> anyhow::Result<Self> {
        use rand::SeedableRng;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let policy = ActorCritic::new(vb, &device)?;
        Ok(Self {
            policy,
            varmap,
            device,
            league: League::default_pool(),
            config,
            rng: rand_chacha::ChaCha8Rng::seed_from_u64(0xa17c01),
        })
    }

    /// Decode a 6-dim squashed-tanh action tensor into an AiDecision.
    /// The 3 caste params + 3 behavior params are softmaxed separately
    /// at the sim layer; here we just hand off the raw values.
    pub fn tensor_to_decision(action: &Tensor) -> anyhow::Result<AiDecision> {
        let v: Vec<f32> = action.to_vec1()?;
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
        let mut env = MatchEnv::new(seed);
        let mut opp = League::make_brain(opp_spec, seed.wrapping_add(1));
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
            batch.values.push(value_t.detach().to_scalar::<f32>()?);
            batch.rewards.push(step.reward_left);
            batch.dones.push(step.done);
            if step.done || env.sim.tick >= env.max_ticks {
                break;
            }
        }
        Ok(batch)
    }

    /// Compute Generalized Advantage Estimation.
    pub fn compute_gae(rewards: &[f32], values: &[f32], dones: &[bool], gamma: f32, lambda: f32) -> (Vec<f32>, Vec<f32>) {
        let n = rewards.len();
        let mut advantages = vec![0.0_f32; n];
        let mut returns = vec![0.0_f32; n];
        let mut gae = 0.0_f32;
        for t in (0..n).rev() {
            let next_value = if t + 1 < n { values[t + 1] } else { 0.0 };
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
}

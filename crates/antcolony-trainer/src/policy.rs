//! ActorCritic — Tanh-squashed Gaussian policy for the 6-dim AiDecision
//! action space. Architecture mirrors MlpBrain (17 -> 64 -> 64 -> 6) so
//! trained weights round-trip into the existing Rust inference path.
//!
//! Output convention (post-tanh squash + scale to [0,1]):
//!   [0,1,2] = caste W/S/B
//!   [3,4,5] = behavior F/D/N
//! Log-prob computation includes the tanh-Jacobian correction (standard
//! SAC/PPO trick: log_prob = log N(u; mean, std) - sum log(1 - tanh^2(u))).

use candle_core::{DType, Device, Result, Tensor};
use candle_nn::{Linear, Module, VarBuilder};

use crate::{INPUT_DIM, OUTPUT_DIM};

pub struct ActorCritic {
    // Actor MLP (INPUT_DIM -> hidden -> hidden -> OUTPUT_DIM)
    pub actor_l1: Linear,
    pub actor_l2: Linear,
    pub actor_l3: Linear,
    // Critic MLP (INPUT_DIM -> hidden -> hidden -> 1)
    pub critic_l1: Linear,
    pub critic_l2: Linear,
    pub critic_l3: Linear,
    // Learnable per-dim log-std
    pub log_std: Tensor,
    // Z-score normalization (frozen post-fit, but we hold them as buffers)
    pub input_mean: Tensor,
    pub input_std: Tensor,
    pub hidden_dim: usize,
}

impl ActorCritic {
    pub fn new(vb: VarBuilder, hidden_dim: usize, device: &Device) -> Result<Self> {
        Ok(Self {
            actor_l1: candle_nn::linear(INPUT_DIM, hidden_dim, vb.pp("actor_l1"))?,
            actor_l2: candle_nn::linear(hidden_dim, hidden_dim, vb.pp("actor_l2"))?,
            actor_l3: candle_nn::linear(hidden_dim, OUTPUT_DIM, vb.pp("actor_l3"))?,
            critic_l1: candle_nn::linear(INPUT_DIM, hidden_dim, vb.pp("critic_l1"))?,
            critic_l2: candle_nn::linear(hidden_dim, hidden_dim, vb.pp("critic_l2"))?,
            critic_l3: candle_nn::linear(hidden_dim, 1, vb.pp("critic_l3"))?,
            log_std: vb.get(OUTPUT_DIM, "log_std").unwrap_or_else(|_| {
                Tensor::full(-1.0_f32, OUTPUT_DIM, device).unwrap()
            }),
            input_mean: Tensor::zeros(INPUT_DIM, DType::F32, device)?,
            input_std: Tensor::ones(INPUT_DIM, DType::F32, device)?,
            hidden_dim,
        })
    }

    /// Normalize input by stored mean/std (z-score).
    pub fn normalize(&self, x: &Tensor) -> Result<Tensor> {
        x.broadcast_sub(&self.input_mean)?.broadcast_div(&self.input_std)
    }

    /// Actor forward returns the pre-squash mean (raw network output).
    /// Squashing happens in `sample()` / `mean_action()`.
    pub fn actor_mean(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.normalize(x)?;
        let h1 = self.actor_l1.forward(&x)?.relu()?;
        let h2 = self.actor_l2.forward(&h1)?.relu()?;
        self.actor_l3.forward(&h2)
    }

    /// Critic forward returns scalar value V(s).
    pub fn value(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.normalize(x)?;
        let h1 = self.critic_l1.forward(&x)?.relu()?;
        let h2 = self.critic_l2.forward(&h1)?.relu()?;
        self.critic_l3.forward(&h2)?.squeeze(candle_core::D::Minus1)
    }

    /// Squash mean to [0,1] via 0.5 * (tanh(x) + 1). Matches MlpBrain's sigmoid
    /// output range so trained weights are deployment-compatible.
    pub fn squash(x: &Tensor) -> Result<Tensor> {
        let t = x.tanh()?;
        let one = Tensor::ones_like(&t)?;
        let half = (t + one)?.affine(0.5, 0.0)?;
        Ok(half)
    }

    /// Deterministic action for inference / eval (mean of policy distribution).
    pub fn mean_action(&self, x: &Tensor) -> Result<Tensor> {
        let mean = self.actor_mean(x)?;
        Self::squash(&mean)
    }

    /// Stochastic sample for training rollouts. Returns (action, log_prob).
    /// All internal tensors are batched [1, OUTPUT_DIM] to match the
    /// linear-layer output. log_std is [OUTPUT_DIM] so we use
    /// broadcast_* ops where shapes differ.
    pub fn sample(&self, x: &Tensor, rng: &mut rand_chacha::ChaCha8Rng) -> Result<(Tensor, Tensor)> {
        use rand::Rng;
        let mean = self.actor_mean(x)?;          // [1, 6]
        let std = self.log_std.exp()?;            // [6]
        // Sample noise via host RNG (Aether FR: native randn would avoid this round-trip)
        let noise: Vec<f32> = (0..OUTPUT_DIM).map(|_| {
            let u1: f32 = rng.gen_range(1e-6..1.0);
            let u2: f32 = rng.gen_range(0.0..1.0);
            (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
        }).collect();
        let noise_t = Tensor::from_vec(noise, (1, OUTPUT_DIM), mean.device())?;  // [1, 6]
        // Pre-squash sample: u = mean + noise * std (broadcast std over batch)
        let scaled_noise = noise_t.broadcast_mul(&std)?;
        let u = (&mean + &scaled_noise)?;
        let action = Self::squash(&u)?;
        // log_prob under Normal(mean, std)
        let diff = (&u - &mean)?;
        let std_sq = std.broadcast_mul(&std)?;     // [6]
        let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
        let two_pi_log = 0.9189385332_f64;
        let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
        let log_pdf = log_pdf_part1.broadcast_sub(&self.log_std)?;
        // Squash Jacobian: log(0.5) + log(1 - tanh^2(u))
        let tanh_u = u.tanh()?;
        let one = Tensor::ones_like(&tanh_u)?;
        let one_minus_tanh_sq = (&one - &(&tanh_u * &tanh_u)?)?;
        let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -0.6931472_f64)?;
        let log_prob = (log_pdf - log_jac)?.sum_all()?;
        Ok((action, log_prob))
    }

    /// Recompute log_prob of a given (potentially squashed) action under
    /// current policy. Used by PPO's importance-ratio computation.
    pub fn log_prob_of(&self, x: &Tensor, action_squashed: &Tensor) -> Result<Tensor> {
        // Invert the squash: action = 0.5 * (tanh(u) + 1) => tanh(u) = 2*action - 1
        // u = atanh(2*action - 1). Clamp to avoid NaN at boundaries.
        let mean = self.actor_mean(x)?;
        let std = self.log_std.exp()?;
        let two_a = action_squashed.affine(2.0, -1.0)?;
        // Clamp to (-1+eps, 1-eps) for numerical stability
        let clamped = two_a.clamp(-0.999999_f32, 0.999999_f32)?;
        // atanh(z) = 0.5 * ln((1+z)/(1-z))
        let one = Tensor::ones_like(&clamped)?;
        let plus = (&one + &clamped)?;
        let minus = (&one - &clamped)?;
        let u = (plus / minus)?.log()?.affine(0.5, 0.0)?;
        let diff = (&u - &mean)?;
        let std_sq = std.broadcast_mul(&std)?;
        let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
        let two_pi_log = 0.9189385332_f64;
        let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
        let log_pdf = log_pdf_part1.broadcast_sub(&self.log_std)?;
        let tanh_u = u.tanh()?;
        let one_t = Tensor::ones_like(&tanh_u)?;
        let one_minus_tanh_sq = (&one_t - &(&tanh_u * &tanh_u)?)?;
        let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -0.6931472_f64)?;
        let log_prob = (log_pdf - log_jac)?.sum_all()?;
        Ok(log_prob)
    }
}

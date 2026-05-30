//! HierarchicalActorCritic — composes CommanderPolicy + AntPolicy under
//! a single builder so rollout/training code holds one object.
//!
//! Variable namespacing under the shared VarBuilder:
//!   commander.* → CommanderPolicy variables
//!   ant.*       → AntPolicy variables
//!
//! Phase 2b will add rollout and PPO-update methods that drive both
//! tiers from the joint trainer. Phase 2a just builds the composition.

use candle_core::{Result, Tensor};
use candle_nn::VarBuilder;

/// Bundle of outputs from a stochastic ant sample.
pub struct AntSample {
    pub modulator: Tensor,  // [B, 5] — post-squash to [0, 1]
    pub value: Tensor,      // [B]
    pub log_prob: Tensor,   // [B]
}

/// Bundle of outputs from a stochastic commander sample.
pub struct CommanderSample {
    pub action: Tensor,    // [B, 6] — post-squash to [0, 1]
    pub intent: Tensor,    // [B, 64]
    pub value: Tensor,     // [B]
    pub log_prob: Tensor,  // [B] — log-prob of action under the Gaussian+tanh policy
}

use crate::hierarchical::ant::{AntForwardOut, AntPolicy};
use crate::hierarchical::commander::{CommanderForwardOut, CommanderPolicy};
use crate::hierarchical::sizing::Sizing;

pub struct HierarchicalActorCritic {
    pub commander: CommanderPolicy,
    pub ant: AntPolicy,
    pub sizing: Sizing,
}

impl HierarchicalActorCritic {
    pub fn new(vb: VarBuilder, sizing: Sizing) -> Result<Self> {
        let commander = CommanderPolicy::new(vb.pp("commander"), sizing)?;
        let ant = AntPolicy::new(vb.pp("ant"), sizing)?;
        Ok(Self { commander, ant, sizing })
    }

    /// Forward through the commander tier only. Convenience wrapper.
    pub fn forward_commander(
        &self,
        state: &Tensor,
        pheromone: &Tensor,
        history: &Tensor,
    ) -> Result<CommanderForwardOut> {
        self.commander.forward(state, pheromone, history)
    }

    /// Forward through the ant tier only. Convenience wrapper.
    pub fn forward_ant(
        &self,
        cone: &Tensor,
        internal: &Tensor,
        intent: &Tensor,
    ) -> Result<AntForwardOut> {
        self.ant.forward(cone, internal, intent)
    }

    /// Stochastic commander rollout step. Mirrors the Gaussian + tanh-squash +
    /// Jacobian-corrected log-prob recipe used by the existing flat
    /// `ActorCritic::sample` (see `crates/antcolony-trainer/src/policy.rs:92`).
    /// Uses the provided RNG so rollouts are reproducible.
    pub fn sample_commander(
        &self,
        state: &Tensor,
        pheromone: &Tensor,
        history: &Tensor,
        rng: &mut rand_chacha::ChaCha8Rng,
    ) -> candle_core::Result<CommanderSample> {
        use rand::Rng;

        let fwd = self.commander.forward(state, pheromone, history)?;
        let mean = fwd.action;                          // [B, action_d]
        let (b, action_d) = mean.dims2()?;
        let std = self.commander.log_std.exp()?;        // [action_d]

        // Box-Muller noise per batch entry, per dim.
        // TODO(aether-FR): replace with Tensor::randn when native op available
        // — avoids the host round-trip on this hot path.
        let mut noise = Vec::with_capacity(b * action_d);
        for _ in 0..(b * action_d) {
            let u1: f32 = rng.gen_range(1e-6_f32..1.0);
            let u2: f32 = rng.gen_range(0.0_f32..1.0);
            noise.push((-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos());
        }
        let noise_t = Tensor::from_vec(noise, (b, action_d), mean.device())?;
        let scaled = noise_t.broadcast_mul(&std)?;      // [B, action_d]
        let u = (&mean + &scaled)?;                     // [B, action_d] pre-squash sample
        let action = squash_tanh_to_unit(&u)?;          // [B, action_d] in [0, 1]

        // log-prob under Normal(mean, std), with squash Jacobian correction.
        // diff: [B, action_d], std_sq: [action_d] — broadcast_div handles rank mismatch.
        let diff = (&u - &mean)?;
        let std_sq = std.broadcast_mul(&std)?;          // [action_d]
        let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
        let two_pi_log = 0.9189385332_f64;              // 0.5 * ln(2π) — matches policy.rs:111
        let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
        // broadcast_sub: [B, action_d] - [action_d] — candle handles the rank diff.
        let log_pdf = log_pdf_part1.broadcast_sub(&self.commander.log_std)?;

        // Squash Jacobian: -log(1 - tanh²(u) + ε) − log(2)
        // The plan formulation writes log_jac = log(1 - tanh²(u) + ε) + affine(-log2).
        // PPO uses (log_pdf - log_jac) = log_pdf - [log(1-tanh²+ε) - log2]
        //                              = log_pdf - log(1-tanh²+ε) + log2
        // which equals the standard SAC Jacobian correction.
        let tanh_u = u.tanh()?;
        let one = Tensor::ones_like(&tanh_u)?;
        let one_minus_tanh_sq = (&one - &(&tanh_u * &tanh_u)?)?;
        let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -std::f64::consts::LN_2)?;

        // Sum over action_d → [B] scalar log-prob per batch entry.
        let log_prob = (log_pdf - log_jac)?.sum(candle_core::D::Minus1)?;

        Ok(CommanderSample {
            action,
            intent: fwd.intent,
            value: fwd.value,
            log_prob,
        })
    }

    /// Stochastic ant rollout step. Mirrors `sample_commander` but on the
    /// 5-d modulator action space.
    pub fn sample_ant(
        &self,
        cone: &Tensor,
        internal: &Tensor,
        intent: &Tensor,
        rng: &mut rand_chacha::ChaCha8Rng,
    ) -> candle_core::Result<AntSample> {
        use rand::Rng;
        let fwd = self.ant.forward(cone, internal, intent)?;
        let mean = fwd.modulator;
        let (b, mod_d) = mean.dims2()?;
        let std = self.ant.log_std.exp()?;

        // TODO(aether-FR): replace with Tensor::randn when native op available.
        let mut noise = Vec::with_capacity(b * mod_d);
        for _ in 0..(b * mod_d) {
            let u1: f32 = rng.gen_range(1e-6_f32..1.0);
            let u2: f32 = rng.gen_range(0.0_f32..1.0);
            noise.push((-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos());
        }
        let noise_t = Tensor::from_vec(noise, (b, mod_d), mean.device())?;
        let scaled = noise_t.broadcast_mul(&std)?;
        let u = (&mean + &scaled)?;
        let modulator = squash_tanh_to_unit(&u)?;

        let diff = (&u - &mean)?;
        let std_sq = std.broadcast_mul(&std)?;
        let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
        let two_pi_log = 0.9189385332_f64;
        let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
        let log_pdf = log_pdf_part1.broadcast_sub(&self.ant.log_std)?;
        let tanh_u = u.tanh()?;
        let one = Tensor::ones_like(&tanh_u)?;
        let one_minus_tanh_sq = (&one - &(&tanh_u * &tanh_u)?)?;
        let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -std::f64::consts::LN_2)?;
        let log_prob = (log_pdf - log_jac)?.sum(candle_core::D::Minus1)?;

        Ok(AntSample {
            modulator,
            value: fwd.value,
            log_prob,
        })
    }

    /// Deterministic commander action (policy mean, no sampling) for eval.
    /// Returns (action[B,6] squashed to [0,1], intent[B,64], value[B]).
    pub fn mean_commander_action(
        &self,
        state: &Tensor,
        pheromone: &Tensor,
        history: &Tensor,
    ) -> Result<(Tensor, Tensor, Tensor)> {
        let fwd = self.commander.forward(state, pheromone, history)?;
        let action = squash_tanh_to_unit(&fwd.action)?;
        Ok((action, fwd.intent, fwd.value))
    }

    /// Deterministic ant modulator (policy mean, no sampling) for eval.
    /// Returns modulator[B,5] squashed to [0,1].
    pub fn mean_ant_modulator(
        &self,
        cone: &Tensor,
        internal: &Tensor,
        intent: &Tensor,
    ) -> Result<Tensor> {
        let fwd = self.ant.forward(cone, internal, intent)?;
        squash_tanh_to_unit(&fwd.modulator)
    }

    /// Recompute the log-prob of a previously-sampled (post-squash) ant
    /// modulator under the current policy. Used by PPO's importance ratio
    /// on the ant tier. Mirrors `log_prob_of_commander_action` on the 5-d
    /// modulator action space.
    pub fn log_prob_of_ant_modulator(
        &self,
        cone: &Tensor,
        internal: &Tensor,
        intent: &Tensor,
        modulator_squashed: &Tensor,
    ) -> candle_core::Result<Tensor> {
        let fwd = self.ant.forward(cone, internal, intent)?;
        let mean = fwd.modulator;
        let std = self.ant.log_std.exp()?;
        let two_m = modulator_squashed.affine(2.0, -1.0)?;
        let clamped = two_m.clamp(-0.999_999_f32, 0.999_999_f32)?;
        let one = Tensor::ones_like(&clamped)?;
        let plus = (&one + &clamped)?;
        let minus = (&one - &clamped)?;
        let u = (plus / minus)?.log()?.affine(0.5, 0.0)?;

        let diff = (&u - &mean)?;
        let std_sq = std.broadcast_mul(&std)?;
        let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
        let two_pi_log = 0.9189385332_f64;
        let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
        let log_pdf = log_pdf_part1.broadcast_sub(&self.ant.log_std)?;
        let tanh_u = u.tanh()?;
        let one_t = Tensor::ones_like(&tanh_u)?;
        let one_minus_tanh_sq = (&one_t - &(&tanh_u * &tanh_u)?)?;
        let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -std::f64::consts::LN_2)?;
        let log_prob = (log_pdf - log_jac)?.sum(candle_core::D::Minus1)?;
        Ok(log_prob)
    }

    /// Recompute the log-prob of a previously-sampled (post-squash) action
    /// under the current policy. Used by PPO's importance ratio
    /// (`r_θ = exp(log_prob_now - log_prob_old)`). Mirrors
    /// `ActorCritic::log_prob_of` at policy.rs:125.
    pub fn log_prob_of_commander_action(
        &self,
        state: &Tensor,
        pheromone: &Tensor,
        history: &Tensor,
        action_squashed: &Tensor,
    ) -> candle_core::Result<Tensor> {
        // Invert the squash: action = 0.5 * (tanh(u) + 1) ⇒ tanh(u) = 2*action - 1
        // u = atanh(z), z = 2*action - 1. Clamp to (-1+eps, 1-eps) for numerical stability.
        let fwd = self.commander.forward(state, pheromone, history)?;
        let mean = fwd.action;  // pre-squash mean
        let std = self.commander.log_std.exp()?;
        let two_a = action_squashed.affine(2.0, -1.0)?;
        let clamped = two_a.clamp(-0.999_999_f32, 0.999_999_f32)?;
        let one = Tensor::ones_like(&clamped)?;
        let plus = (&one + &clamped)?;
        let minus = (&one - &clamped)?;
        let u = (plus / minus)?.log()?.affine(0.5, 0.0)?;

        let diff = (&u - &mean)?;
        let std_sq = std.broadcast_mul(&std)?;
        let neg_log_pdf = ((&diff * &diff)?.broadcast_div(&std_sq)? * 0.5_f64)?;
        let two_pi_log = 0.9189385332_f64;
        let log_pdf_part1 = neg_log_pdf.affine(-1.0, -two_pi_log)?;
        let log_pdf = log_pdf_part1.broadcast_sub(&self.commander.log_std)?;
        let tanh_u = u.tanh()?;
        let one_t = Tensor::ones_like(&tanh_u)?;
        let one_minus_tanh_sq = (&one_t - &(&tanh_u * &tanh_u)?)?;
        let log_jac = (one_minus_tanh_sq + 1e-6_f64)?.log()?.affine(1.0, -std::f64::consts::LN_2)?;
        let log_prob = (log_pdf - log_jac)?.sum(candle_core::D::Minus1)?;
        Ok(log_prob)
    }
}

/// Map pre-squash `u: [...]` to post-squash action in `[0, 1]` per dim.
/// `pub(crate)` so [`sample_ant`] (Task 9) can reuse without duplicating.
pub(crate) fn squash_tanh_to_unit(u: &Tensor) -> candle_core::Result<Tensor> {
    let t = u.tanh()?;
    let one = Tensor::ones_like(&t)?;
    (t + one)?.affine(0.5, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;
    use crate::hierarchical::sizing::A1;

    #[test]
    fn a1_hac_builds() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
        assert_eq!(hac.commander.blocks.len(), A1.cmdr_layers);
        assert_eq!(hac.ant.blocks.len(), A1.ant_layers);
    }

    #[test]
    fn mean_helpers_are_deterministic_and_squashed() {
        use crate::hierarchical::sizing::{
            A1, FIXED_CONE_D, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D, FIXED_INTENT_D,
            FIXED_INTERNAL_D, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W, FIXED_STATE_D,
        };
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();

        let state = Tensor::zeros((1, FIXED_STATE_D), DType::F32, &device).unwrap();
        let pher = Tensor::zeros((1, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W), DType::F32, &device).unwrap();
        let hist = Tensor::zeros((1, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D), DType::F32, &device).unwrap();

        let (a1, _i1, _v1) = hac.mean_commander_action(&state, &pher, &hist).unwrap();
        let (a2, _i2, _v2) = hac.mean_commander_action(&state, &pher, &hist).unwrap();
        assert_eq!(a1.dims(), &[1, 6]);
        let va1: Vec<f32> = a1.flatten_all().unwrap().to_vec1().unwrap();
        let va2: Vec<f32> = a2.flatten_all().unwrap().to_vec1().unwrap();
        assert_eq!(va1, va2, "mean action must be deterministic");
        assert!(va1.iter().all(|x| (0.0..=1.0).contains(x)), "squashed to [0,1]");

        let cone = Tensor::zeros((3, FIXED_CONE_D), DType::F32, &device).unwrap();
        let internal = Tensor::zeros((3, FIXED_INTERNAL_D), DType::F32, &device).unwrap();
        let intent = Tensor::zeros((3, FIXED_INTENT_D), DType::F32, &device).unwrap();
        let mods = hac.mean_ant_modulator(&cone, &internal, &intent).unwrap();
        assert_eq!(mods.dims(), &[3, 5]);
        assert!(mods.flatten_all().unwrap().to_vec1::<f32>().unwrap().iter().all(|x| (0.0..=1.0).contains(x)));
    }

    #[test]
    fn a1_hac_total_param_count_is_sum_of_tiers() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let _ = HierarchicalActorCritic::new(vb, A1).unwrap();
        let total: usize = varmap.all_vars().iter().map(|v| v.dims().iter().product::<usize>()).sum();
        // A1 total ≈ 12M (9M commander + 3M ant). Wide band.
        assert!((6_000_000..=20_000_000).contains(&total),
            "A1 HAC total params ~12M expected, got {}", total);
    }
}

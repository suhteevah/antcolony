//! Commander tier policy net — outer-tick brain (one per colony).
//!
//! Inputs (per decision tick, batch dim implicit):
//!   state_17d        : f32[B, 17]                — ColonyAiState flat
//!   pheromone_field  : f32[B, 4, 32, 32]         — downsampled snapshot
//!   history_tokens   : f32[B, K=8, 96]           — last 8 commander tokens
//!
//! Outputs (Task 6 forward — this task is struct + builder only):
//!   action  : f32[B, 6]   — pre-tanh; squashed by caller for AiDecision
//!   intent  : f32[B, 64]  — broadcast to ant tier this decision window
//!   value   : f32[B]      — V(s) for PPO critic
//!   log_std : f32[6]      — Gaussian policy std (learnable parameter)
//!
//! Backbone (A1 dims shown; A2/A3 scale per Sizing presets):
//!   pheromone_encoder : Conv2d(4→32, k=3) → ReLU → Conv2d(32→64, k=3, s=2) → ReLU → AvgPool2d → Linear(→192)
//!   state_encoder     : Linear(17 → 192)
//!   history_encoder   : Linear(96 → 192)   (applied per-token)
//!   concat → Linear(192 → d_model=384) → [1+1+K=10 tokens]
//!   transformer       : L=4 layers, d=384, heads=6, ffn=1536
//!   pool              : learned [CLS]-style first-token output → 384
//!   heads             : action(384→6), intent(384→64), value(384→1)

use candle_core::{IndexOp, Result, Tensor};
use candle_nn::{Conv2d, Linear, Module, VarBuilder};

use crate::hierarchical::sizing::Sizing;
use crate::hierarchical::transformer::TransformerBlock;

pub struct CommanderPolicy {
    pub sizing: Sizing,

    // Pheromone CNN
    pub(crate) pher_conv1: Conv2d,
    pub(crate) pher_conv2: Conv2d,
    pub(crate) pher_proj: Linear,

    // Token encoders (each produces a single d_enc-dim feature)
    pub(crate) state_encoder: Linear,
    pub(crate) history_encoder: Linear,

    // Reproject 3 streams' encoder outputs from cmdr_encoder_dim → d_model
    pub(crate) stream_proj: Linear,

    // Learned [CLS]-style token prepended to the sequence
    pub(crate) cls_token: Tensor,

    // Transformer backbone
    pub(crate) blocks: Vec<TransformerBlock>,

    // Heads
    pub(crate) action_head: Linear,
    pub(crate) intent_head: Linear,
    pub(crate) value_head: Linear,

    // Learnable per-dim policy std — read by HierarchicalActorCritic::sample_commander.
    pub(crate) log_std: Tensor,
}

/// Bundle of forward-pass outputs from CommanderPolicy.
pub struct CommanderForwardOut {
    pub action: Tensor,   // [B, 6] — pre-tanh
    pub intent: Tensor,   // [B, 64]
    pub value: Tensor,    // [B]
}

impl CommanderPolicy {
    pub fn new(vb: VarBuilder, sizing: Sizing) -> Result<Self> {
        let d_enc = sizing.cmdr_encoder_dim;
        let d_model = sizing.cmdr_d_model;

        // Pheromone CNN — Conv2d(4 → mid, k=3, pad=1, s=1) → Conv2d(mid → hi, k=3, pad=1, s=2)
        // → AvgPool2d(2) → Flatten → Linear(→d_enc)
        let conv_cfg_1 = candle_nn::Conv2dConfig { padding: 1, stride: 1, ..Default::default() };
        let conv_cfg_2 = candle_nn::Conv2dConfig { padding: 1, stride: 2, ..Default::default() };
        let pher_conv1 = candle_nn::conv2d(
            sizing.fixed_pheromone_c,
            sizing.cmdr_pheromone_channels_mid,
            3,
            conv_cfg_1,
            vb.pp("pher_conv1"),
        )?;
        let pher_conv2 = candle_nn::conv2d(
            sizing.cmdr_pheromone_channels_mid,
            sizing.cmdr_pheromone_channels_hi,
            3,
            conv_cfg_2,
            vb.pp("pher_conv2"),
        )?;
        // After conv1 (32×32×mid), conv2 stride=2 (→16×16×hi), AvgPool2d(2) (→8×8×hi). Flatten → 8·8·hi.
        let pher_flat_in = 8 * 8 * sizing.cmdr_pheromone_channels_hi;
        let pher_proj = candle_nn::linear(pher_flat_in, d_enc, vb.pp("pher_proj"))?;

        let state_encoder = candle_nn::linear(sizing.fixed_state_d, d_enc, vb.pp("state_encoder"))?;
        let history_encoder = candle_nn::linear(sizing.fixed_history_tok_d, d_enc, vb.pp("history_encoder"))?;

        let stream_proj = candle_nn::linear(d_enc, d_model, vb.pp("stream_proj"))?;

        // Learnable CLS token shape [1, 1, d_model]. VarBuilder.get with default init.
        let cls_token = vb.get((1, 1, d_model), "cls_token")?;

        let mut blocks = Vec::with_capacity(sizing.cmdr_layers);
        for i in 0..sizing.cmdr_layers {
            blocks.push(TransformerBlock::new(
                vb.pp(format!("block_{i}")),
                d_model,
                sizing.cmdr_heads,
                sizing.cmdr_ffn,
            )?);
        }

        // Small-init the policy MEAN head (std 0.01) so the initial pre-tanh
        // action is ~0 and the tanh squash starts UNSATURATED. Default Kaiming
        // init produced large outputs that saturated the caste-ratio head to a
        // degenerate ~100%-breeder policy (0 workers/soldiers -> colony
        // collapse) which never recovered — gradient ≈ 0 at tanh saturation.
        // Standard RL practice: tiny final-layer init for the policy mean.
        let action_head = {
            let vba = vb.pp("action_head");
            let w = vba.get_with_hints(
                (sizing.fixed_action_d, d_model),
                "weight",
                candle_nn::Init::Randn { mean: 0.0, stdev: 0.01 },
            )?;
            let b = vba.get_with_hints(sizing.fixed_action_d, "bias", candle_nn::Init::Const(0.0))?;
            Linear::new(w, Some(b))
        };
        let intent_head = candle_nn::linear(d_model, sizing.fixed_intent_d, vb.pp("intent_head"))?;
        let value_head = candle_nn::linear(d_model, 1, vb.pp("value_head"))?;

        // log_std is a learnable parameter shape [fixed_action_d], initialized to -1.0.
        let log_std = vb.get_with_hints(
            sizing.fixed_action_d,
            "log_std",
            candle_nn::Init::Const(-1.0),
        )?;

        Ok(Self {
            sizing,
            pher_conv1, pher_conv2, pher_proj,
            state_encoder, history_encoder,
            stream_proj, cls_token,
            blocks,
            action_head, intent_head, value_head,
            log_std,
        })
    }

    pub fn forward(
        &self,
        state: &Tensor,      // [B, 17]
        pheromone: &Tensor,  // [B, 4, 32, 32]
        history: &Tensor,    // [B, K=8, 96]
    ) -> Result<CommanderForwardOut> {
        let (b, _) = state.dims2()?;
        let d_enc = self.sizing.cmdr_encoder_dim;
        let d_model = self.sizing.cmdr_d_model;

        // ── Pheromone CNN ──
        // Conv2d(pad=1,s=1) → ReLU: [B,4,32,32] → [B,mid,32,32]
        // Conv2d(pad=1,s=2) → ReLU: [B,mid,32,32] → [B,hi,16,16]
        // AvgPool2d(2,2):           [B,hi,16,16] → [B,hi,8,8]
        // Flatten → [B, 8*8*hi]
        // Linear → [B, d_enc]
        let p = self.pher_conv1.forward(pheromone)?.relu()?;
        let p = self.pher_conv2.forward(&p)?.relu()?;
        let p = p.avg_pool2d((2, 2))?;
        let p = p.flatten_from(1)?;  // [B, 8*8*hi]
        let pher_tok = self.pher_proj.forward(&p)?;  // [B, d_enc]

        // ── State encoder ──
        let state_tok = self.state_encoder.forward(state)?;  // [B, d_enc]

        // ── History encoder (per-token) ──
        // history: [B, K, 96] → reshape [B*K, 96] → Linear → [B*K, d_enc] → reshape [B, K, d_enc]
        let (b_h, k, _) = history.dims3()?;
        if b_h != b {
            // Batch mismatch is always a caller bug — silent wrong-output in release
            // builds would be very expensive to debug, so make this an Err in all builds.
            candle_core::bail!(
                "CommanderPolicy::forward batch mismatch: state batch={b}, history batch={b_h}"
            );
        }
        let h_flat = history.reshape((b * k, self.sizing.fixed_history_tok_d))?;
        let h_enc = self.history_encoder.forward(&h_flat)?;
        let history_toks = h_enc.reshape((b, k, d_enc))?;  // [B, K, d_enc]

        // ── Stack tokens [pher, state, history_0..K-1] → reproject to d_model ──
        // Use Tensor::stack to fuse unsqueeze+cat for the two prefix tokens — one
        // allocation instead of three. The history tokens already have the token dim.
        let prefix = Tensor::stack(&[&pher_tok, &state_tok], 1)?;          // [B, 2, d_enc]
        let concat = Tensor::cat(&[&prefix, &history_toks], 1)?;           // [B, 2+K, d_enc]
        let tokens = self.stream_proj.forward(&concat)?;                   // [B, 2+K, d_model]

        // ── Prepend learnable CLS token: [1, 1, d_model] → [B, 1, d_model] ──
        let cls = self.cls_token.expand((b, 1, d_model))?;
        let mut x = Tensor::cat(&[&cls, &tokens], 1)?;  // [B, 1+2+K, d_model]

        // ── Transformer backbone ──
        for block in &self.blocks {
            x = block.forward(&x)?;
        }

        // ── Pool: CLS-position output (index 0) ──
        // `.i` over the sequence dim yields a strided (non-contiguous) view;
        // candle's CUDA matmul requires contiguous operands (CPU tolerates
        // the stride). `.contiguous()` is a no-op on values — CPU results are
        // unchanged — and is required before the head matmuls on GPU.
        let cls_out = x.i((.., 0, ..))?.contiguous()?;  // [B, d_model]

        // ── Heads ──
        let action = self.action_head.forward(&cls_out)?;        // [B, 6] — pre-tanh
        let intent = self.intent_head.forward(&cls_out)?;        // [B, 64]
        let value = self.value_head.forward(&cls_out)?.squeeze(1)?;  // [B]

        Ok(CommanderForwardOut { action, intent, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;
    use crate::hierarchical::sizing::A1;

    #[test]
    fn a1_commander_builds() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let policy = CommanderPolicy::new(vb, A1).unwrap();
        assert_eq!(policy.blocks.len(), A1.cmdr_layers);
        assert_eq!(policy.sizing.cmdr_d_model, 384);
    }

    #[test]
    fn a1_commander_forward_shapes() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let policy = CommanderPolicy::new(vb, A1).unwrap();

        let b = 2usize;
        let state = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_state_d), &device).unwrap();
        let pheromone = Tensor::randn(
            0.0f32, 1.0,
            (b, A1.fixed_pheromone_c, A1.fixed_pheromone_h, A1.fixed_pheromone_w),
            &device,
        ).unwrap();
        let history = Tensor::randn(
            0.0f32, 1.0,
            (b, A1.fixed_history_k, A1.fixed_history_tok_d),
            &device,
        ).unwrap();

        let out = policy.forward(&state, &pheromone, &history).unwrap();
        assert_eq!(out.action.dims(), &[b, A1.fixed_action_d]);
        assert_eq!(out.intent.dims(), &[b, A1.fixed_intent_d]);
        assert_eq!(out.value.dims(), &[b]);
    }

    #[test]
    fn a1_commander_param_count_is_in_ballpark() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let _ = CommanderPolicy::new(vb, A1).unwrap();
        let total: usize = varmap.all_vars().iter().map(|v| v.dims().iter().product::<usize>()).sum();
        // A1 commander total spec ≈ 9M (transformer ~7M + encoders/heads ~2M).
        // Allow a wide band; tighter checks happen in the forward-shape tests.
        assert!((5_000_000..=15_000_000).contains(&total),
            "A1 commander total params ~9M expected, got {}", total);
    }
}

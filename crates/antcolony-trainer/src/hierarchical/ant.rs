//! Ant tier policy net — per-ant brain (one shared instance per colony,
//! evaluated once per ant per tick, batched along ants).
//!
//! Inputs:
//!   cone   : f32[B, 60]   — AntObservation.pheromone_cone
//!   intern : f32[B, 8]    — AntObservation.internal
//!   intent : f32[B, 64]   — broadcast from commander (same value for all ants in colony)
//!
//! Outputs:
//!   modulator : f32[B, 5]  — pre-squash; trainer applies tanh/sigmoid per field
//!   value     : f32[B]     — local critic for ant-tier GAE (Phase 2b)
//!   log_std   : f32[5]     — learnable per-dim std

use candle_core::{IndexOp, Result, Tensor};
use candle_nn::{Linear, Module, VarBuilder};

use crate::hierarchical::sizing::Sizing;
use crate::hierarchical::transformer::TransformerBlock;

pub struct AntPolicy {
    pub sizing: Sizing,

    pub(crate) cone_encoder1: Linear,
    pub(crate) cone_encoder2: Linear,
    pub(crate) state_encoder: Linear,
    pub(crate) intent_encoder: Linear,

    // Reproject concatenated [cone, internal, intent] features to d_model
    pub(crate) stream_proj: Linear,

    pub(crate) blocks: Vec<TransformerBlock>,

    pub(crate) modulator_head: Linear,
    pub(crate) value_head: Linear,
    pub(crate) log_std: Tensor,
}

pub struct AntForwardOut {
    pub modulator: Tensor,  // [B, 5] — pre-squash
    pub value: Tensor,      // [B]
}

impl AntPolicy {
    pub fn new(vb: VarBuilder, sizing: Sizing) -> Result<Self> {
        let d_model = sizing.ant_d_model;

        let cone_encoder1 = candle_nn::linear(sizing.fixed_cone_d, sizing.ant_cone_hidden, vb.pp("cone_encoder1"))?;
        let cone_encoder2 = candle_nn::linear(sizing.ant_cone_hidden, sizing.ant_cone_hidden, vb.pp("cone_encoder2"))?;
        let state_encoder = candle_nn::linear(sizing.fixed_internal_d, sizing.ant_internal_hidden, vb.pp("state_encoder"))?;
        let intent_encoder = candle_nn::linear(sizing.fixed_intent_d, sizing.ant_intent_hidden, vb.pp("intent_encoder"))?;

        let concat_dim = sizing.ant_cone_hidden + sizing.ant_internal_hidden + sizing.ant_intent_hidden;
        let stream_proj = candle_nn::linear(concat_dim, d_model, vb.pp("stream_proj"))?;

        let mut blocks = Vec::with_capacity(sizing.ant_layers);
        for i in 0..sizing.ant_layers {
            blocks.push(TransformerBlock::new(
                vb.pp(format!("block_{i}")),
                d_model,
                sizing.ant_heads,
                sizing.ant_ffn,
            )?);
        }

        let modulator_head = candle_nn::linear(d_model, sizing.fixed_modulator_d, vb.pp("modulator_head"))?;
        let value_head = candle_nn::linear(d_model, 1, vb.pp("value_head"))?;

        let log_std = vb.get_with_hints(
            sizing.fixed_modulator_d,
            "log_std",
            candle_nn::Init::Const(-1.0),
        )?;

        Ok(Self {
            sizing,
            cone_encoder1, cone_encoder2,
            state_encoder, intent_encoder,
            stream_proj,
            blocks,
            modulator_head, value_head,
            log_std,
        })
    }

    pub fn forward(&self, cone: &Tensor, internal: &Tensor, intent: &Tensor) -> Result<AntForwardOut> {
        // Encoders
        let cone_h = self.cone_encoder1.forward(cone)?.relu()?;
        let cone_h = self.cone_encoder2.forward(&cone_h)?;

        let state_h = self.state_encoder.forward(internal)?;
        let intent_h = self.intent_encoder.forward(intent)?;

        // Concatenate along feature dim
        let combined = Tensor::cat(&[&cone_h, &state_h, &intent_h], 1)?;  // [B, concat_dim]
        let projected = self.stream_proj.forward(&combined)?;             // [B, d_model]

        // Single-token transformer — attention with T=1 is just inter-feature mixing
        // through the FFN sub-block; we keep the structure for symmetry with the
        // commander tier and easy future extension to temporal sequences.
        let mut x = projected.unsqueeze(1)?;  // [B, 1, d_model]
        for block in &self.blocks {
            x = block.forward(&x)?;
        }
        let pooled = x.i((.., 0, ..))?;  // [B, d_model]

        let modulator = self.modulator_head.forward(&pooled)?;       // [B, 5]
        let value = self.value_head.forward(&pooled)?.squeeze(1)?;   // [B]

        Ok(AntForwardOut { modulator, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;
    use crate::hierarchical::sizing::A1;

    fn cpu_vb() -> (VarMap, Device) {
        (VarMap::new(), Device::Cpu)
    }

    #[test]
    fn a1_ant_builds() {
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let policy = AntPolicy::new(vb, A1).unwrap();
        assert_eq!(policy.blocks.len(), A1.ant_layers);
        assert_eq!(policy.sizing.ant_d_model, 256);
    }

    #[test]
    fn a1_ant_param_count_is_in_ballpark() {
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let _ = AntPolicy::new(vb, A1).unwrap();
        let total: usize = varmap.all_vars().iter().map(|v| v.dims().iter().product::<usize>()).sum();
        assert!((1_000_000..=6_000_000).contains(&total),
            "A1 ant total params ~3M expected, got {}", total);
    }

    #[test]
    fn a1_ant_forward_shapes() {
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let policy = AntPolicy::new(vb, A1).unwrap();

        let b = 7usize;  // 7 ants in a colony
        let cone = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_cone_d), &device).unwrap();
        let intern = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_internal_d), &device).unwrap();
        let intent = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_intent_d), &device).unwrap();

        let out = policy.forward(&cone, &intern, &intent).unwrap();
        assert_eq!(out.modulator.dims(), &[b, A1.fixed_modulator_d]);
        assert_eq!(out.value.dims(), &[b]);
    }
}

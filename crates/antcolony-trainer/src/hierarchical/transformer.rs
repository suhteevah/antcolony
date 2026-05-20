//! Minimal transformer block: multi-head self-attention + post-attention
//! LayerNorm + FFN + post-FFN LayerNorm. Used by both
//! [`crate::hierarchical::commander`] and [`crate::hierarchical::ant`]
//! backbones.
//!
//! Built from `candle_nn::Linear` + `LayerNorm` primitives — we don't
//! pull in `candle-transformers` because we control the size and don't
//! need exotic features (RoPE, KV cache, FlashAttention). On Pascal
//! sm_60 we'd lose FlashAttention anyway.

use candle_core::{Result, Tensor, D};
use candle_nn::{LayerNorm, Linear, Module, VarBuilder};

/// One transformer block: pre-norm style.
///   x = x + self_attn(LN(x))
///   x = x + ffn(LN(x))
pub struct TransformerBlock {
    pub d_model: usize,
    pub n_heads: usize,
    pub d_head: usize,

    pub norm_attn: LayerNorm,
    pub q_proj: Linear,
    pub k_proj: Linear,
    pub v_proj: Linear,
    pub o_proj: Linear,

    pub norm_ffn: LayerNorm,
    pub ffn_up: Linear,
    pub ffn_down: Linear,
}

impl TransformerBlock {
    /// Build a transformer block.
    ///
    /// - `vb` — VarBuilder rooted at this block's namespace
    /// - `d_model` — hidden dim (must be divisible by `n_heads`)
    /// - `n_heads` — number of attention heads
    /// - `d_ffn` — feed-forward inner dim (usually 4×d_model in vanilla transformers)
    pub fn new(vb: VarBuilder, d_model: usize, n_heads: usize, d_ffn: usize) -> Result<Self> {
        assert!(
            d_model % n_heads == 0,
            "d_model={} must be divisible by n_heads={}",
            d_model,
            n_heads,
        );
        let d_head = d_model / n_heads;

        Ok(Self {
            d_model,
            n_heads,
            d_head,
            norm_attn: candle_nn::layer_norm(d_model, 1e-5, vb.pp("norm_attn"))?,
            q_proj: candle_nn::linear(d_model, d_model, vb.pp("q_proj"))?,
            k_proj: candle_nn::linear(d_model, d_model, vb.pp("k_proj"))?,
            v_proj: candle_nn::linear(d_model, d_model, vb.pp("v_proj"))?,
            o_proj: candle_nn::linear(d_model, d_model, vb.pp("o_proj"))?,
            norm_ffn: candle_nn::layer_norm(d_model, 1e-5, vb.pp("norm_ffn"))?,
            ffn_up: candle_nn::linear(d_model, d_ffn, vb.pp("ffn_up"))?,
            ffn_down: candle_nn::linear(d_ffn, d_model, vb.pp("ffn_down"))?,
        })
    }

    /// Forward pass on `x: [B, T, d_model]`. Returns same shape.
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // Self-attention sub-block (pre-norm + residual).
        let attn_in = self.norm_attn.forward(x)?;
        let attn_out = self.self_attention(&attn_in)?;
        let x = (x + attn_out)?;

        // FFN sub-block (pre-norm + residual).
        let ffn_in = self.norm_ffn.forward(&x)?;
        let ffn_mid = self.ffn_up.forward(&ffn_in)?;
        let ffn_mid = ffn_mid.gelu()?;
        let ffn_out = self.ffn_down.forward(&ffn_mid)?;
        let x = (x + ffn_out)?;

        Ok(x)
    }

    fn self_attention(&self, x: &Tensor) -> Result<Tensor> {
        let (b, t, _) = x.dims3()?;

        let q = self.q_proj.forward(x)?;
        let k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;

        // Reshape to [B, T, H, D/H] then transpose to [B, H, T, D/H].
        let q = q
            .reshape((b, t, self.n_heads, self.d_head))?
            .transpose(1, 2)?
            .contiguous()?;
        let k = k
            .reshape((b, t, self.n_heads, self.d_head))?
            .transpose(1, 2)?
            .contiguous()?;
        let v = v
            .reshape((b, t, self.n_heads, self.d_head))?
            .transpose(1, 2)?
            .contiguous()?;

        // scores = Q @ K^T / sqrt(d_head)
        let scale = 1.0 / (self.d_head as f64).sqrt();
        let scores =
            q.matmul(&k.transpose(D::Minus2, D::Minus1)?.contiguous()?)?;
        let scores = (scores * scale)?;

        let attn = candle_nn::ops::softmax(&scores, D::Minus1)?;
        let out = attn.matmul(&v)?; // [B, H, T, D/H]

        // Back to [B, T, D].
        let out = out
            .transpose(1, 2)?
            .contiguous()?
            .reshape((b, t, self.d_model))?;
        self.o_proj.forward(&out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;

    fn cpu_vb() -> (VarMap, Device) {
        (VarMap::new(), Device::Cpu)
    }

    #[test]
    fn block_preserves_shape() {
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let block = TransformerBlock::new(vb, 64, 4, 128).unwrap();

        let x = Tensor::randn(0.0f32, 1.0, (2, 5, 64), &device).unwrap();
        let y = block.forward(&x).unwrap();
        assert_eq!(y.dims(), &[2, 5, 64]);
    }

    #[test]
    fn block_d_model_must_divide_by_heads() {
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        // 64 / 5 doesn't divide evenly — should panic.
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            TransformerBlock::new(vb, 64, 5, 128).unwrap()
        }));
        assert!(r.is_err(), "expected panic for non-divisible d_model/n_heads");
    }

    #[test]
    fn block_param_count_matches_estimate() {
        // d=128, ffn=256: core = 4·128² + 2·128·256 = 65536 + 65536 = 131072.
        let (varmap, device) = cpu_vb();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let _block = TransformerBlock::new(vb, 128, 4, 256).unwrap();
        let total: usize = varmap
            .all_vars()
            .iter()
            .map(|v| v.dims().iter().product::<usize>())
            .sum();
        let core = 131_072;
        // Allow ~10% headroom for biases + LN weights, plus an absolute slack of 4096 for tiny weights.
        assert!(
            total >= core && total <= (core as f64 * 1.15) as usize + 4096,
            "param count {} should be approximately core {} (within +15% + LN slack)",
            total,
            core,
        );
    }
}

//! Sizing presets for the hierarchical brain. A1 is the smoke target
//! (~12M params, fits on kokonoe 3070 Ti 8GB easily); A2 is the
//! 8GB-consumer deployment target (~95M params); A3 is the cnc P100
//! research teacher (~160M params). See the design spec for context.

/// Sizing preset for the hierarchical policy net. Holds dims for both
/// commander and ant tiers. The `est_*_params` methods give a rough
/// parameter-count estimate used as a sanity-check assertion in tests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizing {
    // Commander tier
    pub cmdr_d_model: usize,
    pub cmdr_layers: usize,
    pub cmdr_heads: usize,
    pub cmdr_ffn: usize,
    pub cmdr_pheromone_channels_mid: usize,
    pub cmdr_pheromone_channels_hi: usize,
    pub cmdr_encoder_dim: usize,
    // Ant tier
    pub ant_d_model: usize,
    pub ant_layers: usize,
    pub ant_heads: usize,
    pub ant_ffn: usize,
    pub ant_cone_hidden: usize,
    pub ant_internal_hidden: usize,
    pub ant_intent_hidden: usize,
    // Shared fixed shapes (match sim API; NOT scaled by sizing)
    pub fixed_state_d: usize,
    pub fixed_action_d: usize,
    pub fixed_intent_d: usize,
    pub fixed_history_k: usize,
    pub fixed_history_tok_d: usize,
    pub fixed_cone_d: usize,
    pub fixed_internal_d: usize,
    pub fixed_modulator_d: usize,
    pub fixed_pheromone_w: usize,
    pub fixed_pheromone_h: usize,
    pub fixed_pheromone_c: usize,
}

pub const FIXED_STATE_D: usize = 17;
pub const FIXED_ACTION_D: usize = 6;
pub const FIXED_INTENT_D: usize = 64;
pub const FIXED_HISTORY_K: usize = 8;
pub const FIXED_HISTORY_TOK_D: usize = 96;
pub const FIXED_CONE_D: usize = 60;
pub const FIXED_INTERNAL_D: usize = 8;
pub const FIXED_MODULATOR_D: usize = 5;
pub const FIXED_PHEROMONE_W: usize = 32;
pub const FIXED_PHEROMONE_H: usize = 32;
pub const FIXED_PHEROMONE_C: usize = 4;

const fn fixed_defaults() -> Sizing {
    Sizing {
        cmdr_d_model: 0, cmdr_layers: 0, cmdr_heads: 0, cmdr_ffn: 0,
        cmdr_pheromone_channels_mid: 0, cmdr_pheromone_channels_hi: 0,
        cmdr_encoder_dim: 0,
        ant_d_model: 0, ant_layers: 0, ant_heads: 0, ant_ffn: 0,
        ant_cone_hidden: 0, ant_internal_hidden: 0, ant_intent_hidden: 0,
        fixed_state_d: FIXED_STATE_D,
        fixed_action_d: FIXED_ACTION_D,
        fixed_intent_d: FIXED_INTENT_D,
        fixed_history_k: FIXED_HISTORY_K,
        fixed_history_tok_d: FIXED_HISTORY_TOK_D,
        fixed_cone_d: FIXED_CONE_D,
        fixed_internal_d: FIXED_INTERNAL_D,
        fixed_modulator_d: FIXED_MODULATOR_D,
        fixed_pheromone_w: FIXED_PHEROMONE_W,
        fixed_pheromone_h: FIXED_PHEROMONE_H,
        fixed_pheromone_c: FIXED_PHEROMONE_C,
    }
}

/// A1 — compact smoke target. ~12M total params (~9M commander + ~3M ant).
pub const A1: Sizing = Sizing {
    cmdr_d_model: 384,
    cmdr_layers: 4,
    cmdr_heads: 6,
    cmdr_ffn: 1536,
    cmdr_pheromone_channels_mid: 32,
    cmdr_pheromone_channels_hi: 64,
    cmdr_encoder_dim: 192,
    ant_d_model: 256,
    ant_layers: 4,
    ant_heads: 4,
    ant_ffn: 1024,
    ant_cone_hidden: 128,
    ant_internal_hidden: 64,
    ant_intent_hidden: 64,
    ..fixed_defaults()
};

/// A2 — 8GB-consumer deployment target. ~95M total (~70M commander + ~25M ant).
pub const A2: Sizing = Sizing {
    cmdr_d_model: 768,
    cmdr_layers: 8,
    cmdr_heads: 12,
    cmdr_ffn: 3072,
    cmdr_pheromone_channels_mid: 64,
    cmdr_pheromone_channels_hi: 128,
    cmdr_encoder_dim: 384,
    ant_d_model: 512,
    ant_layers: 6,
    ant_heads: 8,
    ant_ffn: 2048,
    ant_cone_hidden: 256,
    ant_internal_hidden: 128,
    ant_intent_hidden: 128,
    ..fixed_defaults()
};

/// A3 — cnc P100 research teacher. ~160M total (~120M commander + ~40M ant).
pub const A3: Sizing = Sizing {
    cmdr_d_model: 1024,
    cmdr_layers: 10,
    cmdr_heads: 16,
    cmdr_ffn: 4096,
    cmdr_pheromone_channels_mid: 64,
    cmdr_pheromone_channels_hi: 128,
    cmdr_encoder_dim: 512,
    ant_d_model: 640,
    ant_layers: 8,
    ant_heads: 10,
    ant_ffn: 2560,
    ant_cone_hidden: 384,
    ant_internal_hidden: 192,
    ant_intent_hidden: 192,
    ..fixed_defaults()
};

impl Sizing {
    /// Rough param-count estimate for the commander transformer backbone
    /// (excluding encoders and heads). Formula: layers × (4·d² + 2·d·ffn)
    /// — accounts for QKV+O projections (4·d²) and the FFN (d→ffn→d = 2·d·ffn).
    pub fn est_cmdr_transformer_params(&self) -> usize {
        self.cmdr_layers * (4 * self.cmdr_d_model * self.cmdr_d_model
            + 2 * self.cmdr_d_model * self.cmdr_ffn)
    }

    pub fn est_ant_transformer_params(&self) -> usize {
        self.ant_layers * (4 * self.ant_d_model * self.ant_d_model
            + 2 * self.ant_d_model * self.ant_ffn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a1_total_transformer_params_in_smoke_range() {
        let cmdr = A1.est_cmdr_transformer_params();
        let ant = A1.est_ant_transformer_params();
        assert!((5_000_000..=10_000_000).contains(&cmdr),
            "A1 commander transformer expected ~7M, got {}", cmdr);
        assert!((2_000_000..=5_000_000).contains(&ant),
            "A1 ant transformer expected ~3M, got {}", ant);
    }

    #[test]
    fn a2_total_transformer_params_in_8gb_range() {
        let cmdr = A2.est_cmdr_transformer_params();
        let ant = A2.est_ant_transformer_params();
        assert!((40_000_000..=80_000_000).contains(&cmdr),
            "A2 commander transformer expected ~57M, got {}", cmdr);
        assert!((12_000_000..=30_000_000).contains(&ant),
            "A2 ant transformer expected ~19M, got {}", ant);
    }

    #[test]
    fn fixed_dims_match_phase1_sim_api() {
        assert_eq!(FIXED_STATE_D, 17);
        assert_eq!(FIXED_ACTION_D, 6);
        assert_eq!(FIXED_INTENT_D, 64);
        assert_eq!(FIXED_HISTORY_K, 8);
        assert_eq!(FIXED_HISTORY_TOK_D, antcolony_sim::HistoryToken::FLAT_LEN);
        assert_eq!(FIXED_CONE_D, 60);
        assert_eq!(FIXED_INTERNAL_D, 8);
        assert_eq!(FIXED_MODULATOR_D, 5);
        assert_eq!(FIXED_PHEROMONE_W, 32);
        assert_eq!(FIXED_PHEROMONE_H, 32);
        assert_eq!(FIXED_PHEROMONE_C, 4);
    }
}

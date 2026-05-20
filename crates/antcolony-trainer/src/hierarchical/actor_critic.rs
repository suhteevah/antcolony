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

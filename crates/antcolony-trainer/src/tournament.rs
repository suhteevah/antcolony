//! PvP round-robin tournament: rank any mix of HAC + scripted brains on a
//! Bradley-Terry/Elo ladder over the 2-colony engine.

use std::path::PathBuf;
use anyhow::Result;
use candle_core::Device;

use antcolony_sim::ai::brain::AiBrain;
use crate::hierarchical::sizing::Sizing;
use crate::self_play::load_frozen_hac;
use crate::{HierarchicalActorCritic, League};

/// How a contender drives its colony.
pub enum Controller {
    /// Hierarchical brain: commander intents + per-ant modulators.
    Hac(HierarchicalActorCritic),
    /// Scripted colony-level brain; the sim runs its default ant behavior.
    Scripted(Box<dyn AiBrain>),
}

/// One enrolled brain.
pub struct Contender {
    pub id: String,
    pub spec: String,
    pub controller: Controller,
}

/// Build a contender from a spec. `"hac:<path>"` loads a frozen HAC checkpoint;
/// any other spec (`"heuristic"`, `"mlp:<path>"`, `"mix:..."`, archetype names)
/// resolves via `League::make_brain`.
pub fn build_contender(id: &str, spec: &str, device: &Device, sizing: Sizing) -> Result<Contender> {
    let controller = if let Some(path) = spec.strip_prefix("hac:") {
        Controller::Hac(load_frozen_hac(&PathBuf::from(path), sizing, device)?)
    } else {
        Controller::Scripted(League::make_brain(spec, 0)?)
    };
    tracing::info!(id, spec, hac = matches!(controller, Controller::Hac(_)), "tournament: contender built");
    Ok(Contender { id: id.to_string(), spec: spec.to_string(), controller })
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;
    use crate::hierarchical::sizing::A1;

    #[test]
    fn build_contender_resolves_scripted_and_hac() {
        let dev = Device::Cpu;
        // scripted archetype
        let c = build_contender("aggro", "aggressor", &dev, A1).unwrap();
        assert_eq!(c.id, "aggro");
        assert!(matches!(c.controller, Controller::Scripted(_)));
        // hac from a freshly-saved A1 varmap
        let dir = std::env::temp_dir().join("tourney_build_contender");
        std::fs::create_dir_all(&dir).unwrap();
        let ck = dir.join("hac.safetensors");
        let t = crate::JointPpoTrainer::new(Device::Cpu, A1, crate::JointPpoConfig::smoke_default()).unwrap();
        t.varmap.save(&ck).unwrap();
        let h = build_contender("sota", &format!("hac:{}", ck.display()), &dev, A1).unwrap();
        assert_eq!(h.id, "sota");
        assert!(matches!(h.controller, Controller::Hac(_)));
    }
}

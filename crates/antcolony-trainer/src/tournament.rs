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

/// Bradley-Terry strengths via MM iteration, returned on the Elo scale.
/// `win_matrix[i][j]` = i's mean score vs j in [0,1] (diag ignored); `games[i][j]`
/// = number of games i-vs-j. `anchor_idx` (if set) is pegged to `anchor_elo`;
/// otherwise the mean Elo is centered at `anchor_elo`.
pub fn bradley_terry_elo(win_matrix: &[Vec<f32>], games: &[Vec<usize>], anchor_idx: Option<usize>, anchor_elo: f64) -> Vec<f64> {
    let n = win_matrix.len();
    if n == 0 { return Vec::new(); }
    // win credit W_i = Σ_j score[i][j] * games[i][j]  (draws already 0.5 in score)
    let mut wins = vec![0.0f64; n];
    for i in 0..n {
        for j in 0..n {
            if i == j { continue; }
            let s = win_matrix[i][j];
            if s.is_finite() { wins[i] += s as f64 * games[i][j] as f64; }
        }
    }
    let mut p = vec![1.0f64; n];
    for _ in 0..200 {
        let mut np = vec![0.0f64; n];
        for i in 0..n {
            let mut denom = 0.0f64;
            for j in 0..n {
                if i == j { continue; }
                let g = games[i][j] as f64;
                if g > 0.0 { denom += g / (p[i] + p[j]); }
            }
            np[i] = if denom > 0.0 { (wins[i] / denom).max(1e-12) } else { p[i] };
        }
        // normalize to keep numbers bounded
        let sum: f64 = np.iter().sum();
        if sum > 0.0 { for x in np.iter_mut() { *x /= sum; } }
        p = np;
    }
    let mut elo: Vec<f64> = p.iter().map(|&pi| 400.0 * pi.max(1e-12).log10()).collect();
    let shift = match anchor_idx {
        Some(a) if a < n => anchor_elo - elo[a],
        _ => anchor_elo - elo.iter().sum::<f64>() / n as f64,
    };
    for e in elo.iter_mut() { *e += shift; }
    elo
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

    #[test]
    fn bradley_terry_ranks_dominance_order_and_anchors() {
        // 3 brains, strict dominance A>B>C: W[i][j] = i's score vs j.
        let w = vec![
            vec![f32::NAN, 0.8, 0.9],
            vec![0.2, f32::NAN, 0.8],
            vec![0.1, 0.2, f32::NAN],
        ];
        let g = vec![vec![0, 10, 10], vec![10, 0, 10], vec![10, 10, 0]];
        let elo = bradley_terry_elo(&w, &g, Some(1), 1000.0);
        assert!(elo[0] > elo[1] && elo[1] > elo[2], "A>B>C: {elo:?}");
        assert!((elo[1] - 1000.0).abs() < 1e-6, "anchor pegged: {}", elo[1]);
        // symmetric (all 0.5) -> equal ratings
        let we = vec![vec![f32::NAN, 0.5], vec![0.5, f32::NAN]];
        let ge = vec![vec![0, 10], vec![10, 0]];
        let eloe = bradley_terry_elo(&we, &ge, None, 1000.0);
        assert!((eloe[0] - eloe[1]).abs() < 1e-3, "equal: {eloe:?}");
    }
}

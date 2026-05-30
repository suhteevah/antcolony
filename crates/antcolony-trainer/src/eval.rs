//! Deterministic evaluation of the HAC against the 7-archetype bench — the
//! same opponents and metric used to measure MlpBrain v1 at 47.1%. The HAC
//! plays the left colony with policy-MEAN actions (no sampling); each
//! archetype plays the right. Win = left win (1.0), loss (0.0), draw (0.5),
//! timeout graded by worker share.

use anyhow::Result;
use candle_core::Device;

use antcolony_sim::ai::observation::AntModulators;
use antcolony_sim::MatchStatus;

use crate::env::{MatchEnv, DECISION_CADENCE};
use crate::hierarchical::obs_to_tensors::{ant_obs_to_tensors, rich_to_tensors};
use crate::HierarchicalActorCritic;
use crate::League;

/// The 7 fixed archetype specs that define the bench (== League::default_pool).
pub const BENCH_ARCHETYPES: [&str; 7] = [
    "heuristic", "defender", "aggressor", "economist", "breeder", "forager", "conservative",
];

#[derive(Clone, Debug)]
pub struct EvalReport {
    pub per_archetype: Vec<(String, f32)>,
    pub mean_win_rate: f32,
}

fn play_match(
    hac: &HierarchicalActorCritic,
    device: &Device,
    opp_spec: &str,
    seed: u64,
) -> Result<f32> {
    let mut env = MatchEnv::new(seed);
    let mut opp = League::make_brain(opp_spec, seed.wrapping_add(1));

    loop {
        let rich = match env.sim.colony_rich_observation(0) {
            Some(r) => r,
            None => break,
        };
        let (s, p, h) = rich_to_tensors(&rich, device)?;
        let (action, intent, _value) = hac.mean_commander_action(&s, &p, &h)?;
        let av: Vec<f32> = action.flatten_all()?.to_vec1()?;
        let dec = antcolony_sim::AiDecision {
            caste_ratio_worker: av[0], caste_ratio_soldier: av[1], caste_ratio_breeder: av[2],
            forage_weight: av[3], dig_weight: av[4], nurse_weight: av[5], research_choice: None,
        };
        env.sim.apply_ai_decision(0, &dec);
        let iv: Vec<f32> = intent.flatten_all()?.to_vec1()?;
        let mut intent_arr = [0.0f32; 64];
        intent_arr.copy_from_slice(&iv);
        env.sim.apply_commander_intent(0, &intent_arr);
        if let Some(sr) = env.sim.colony_ai_state(1) {
            let dr = opp.decide(&sr);
            env.sim.apply_ai_decision(1, &dr);
        }

        let mut done = false;
        for _ in 0..DECISION_CADENCE {
            let obs = env.sim.per_ant_observations(0);
            if !obs.is_empty() {
                let (cone, internal, intent_b) = ant_obs_to_tensors(&obs, &intent, device)?;
                let mods_t = hac.mean_ant_modulator(&cone, &internal, &intent_b)?;
                let flat: Vec<f32> = mods_t.flatten_all()?.to_vec1()?;
                let mut mods = Vec::with_capacity(obs.len());
                let mut ids = Vec::with_capacity(obs.len());
                for (k, o) in obs.iter().enumerate() {
                    let off = k * 5;
                    mods.push(AntModulators {
                        alpha_mult: flat[off], beta_mult: flat[off + 1], exploration_mod: flat[off + 2],
                        deposit_mult: flat[off + 3], state_bias: flat[off + 4],
                    });
                    ids.push(o.ant_id);
                }
                env.sim.apply_ant_modulators(0, &mods, &ids);
            }
            env.sim.tick();
            if !matches!(env.sim.match_status(), MatchStatus::InProgress)
                || env.sim.tick >= env.max_ticks
            {
                done = true;
                break;
            }
        }
        if done {
            break;
        }
    }

    Ok(match env.sim.match_status() {
        MatchStatus::Won { winner: 0, .. } => 1.0,
        MatchStatus::Won { winner: 1, .. } => 0.0,
        MatchStatus::Draw { .. } => 0.5,
        MatchStatus::InProgress => {
            let lw = env.sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0) as f32;
            let rw = env.sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0) as f32;
            let share = lw / (lw + rw).max(1.0);
            if share > 0.5 { 1.0 } else if share < 0.5 { 0.0 } else { 0.5 }
        }
        _ => 0.5,
    })
}

pub fn evaluate_hac(
    hac: &HierarchicalActorCritic,
    device: &Device,
    matches_per_opp: usize,
) -> Result<EvalReport> {
    let mut per_archetype = Vec::with_capacity(BENCH_ARCHETYPES.len());
    for spec in BENCH_ARCHETYPES {
        let mut score = 0.0f32;
        for m in 0..matches_per_opp {
            let seed = 0xE7A1_u64
                .wrapping_mul(spec.len() as u64 + 1)
                ^ ((m as u64).wrapping_mul(0x9E3779B97F4A7C15));
            score += play_match(hac, device, spec, seed)?;
        }
        let wr = score / matches_per_opp.max(1) as f32;
        tracing::info!(archetype = spec, win_rate = wr, "eval vs archetype");
        per_archetype.push((spec.to_string(), wr));
    }
    let mean = per_archetype.iter().map(|(_, w)| *w).sum::<f32>()
        / per_archetype.len().max(1) as f32;
    Ok(EvalReport { per_archetype, mean_win_rate: mean })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchical::sizing::A1;
    use candle_core::{DType, Device};
    use candle_nn::{VarBuilder, VarMap};

    #[test]
    fn evaluate_hac_one_match_per_opp_produces_valid_rates() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
        let report = evaluate_hac(&hac, &device, 1).unwrap();
        assert_eq!(report.per_archetype.len(), 7);
        for (name, wr) in &report.per_archetype {
            assert!(!name.is_empty());
            assert!((0.0..=1.0).contains(wr), "{name} win-rate out of range: {wr}");
        }
        assert!((0.0..=1.0).contains(&report.mean_win_rate));
    }
}

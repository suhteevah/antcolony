//! Observation → tensor conversion helpers shared between the trainer
//! and integration tests. Phase 2a inlined these in
//! `tests/hierarchical_smoke.rs`; Phase 2b extracted them so the
//! `JointPpoTrainer` rollout code can call them too.
//!
//! Layouts are pinned to the `Sizing` `FIXED_*` constants — if the
//! sim's `ColonyAiState` / `AntObservation` shapes ever change, the
//! `fixed_dims_match_phase1_sim_api` test in `sizing.rs` trips first.

use candle_core::{Device, Result, Tensor};

use antcolony_sim::ai::observation::{AntObservation, HistoryToken, RichObservation};

use crate::hierarchical::sizing::{
    FIXED_CONE_D, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D, FIXED_INTENT_D, FIXED_INTERNAL_D,
    FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W, FIXED_STATE_D,
};

/// Convert one [`RichObservation`] to (state, pheromone, history) tensors
/// with a leading batch dim of 1.
pub fn rich_to_tensors(
    rich: &RichObservation,
    device: &Device,
) -> Result<(Tensor, Tensor, Tensor)> {
    let state_v = state_flatten(rich);
    debug_assert_eq!(state_v.len(), FIXED_STATE_D);
    let state = Tensor::from_vec(state_v, (1, FIXED_STATE_D), device)?;

    let pher_v = pheromone_flatten(rich);
    let pheromone = Tensor::from_vec(
        pher_v,
        (1, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W),
        device,
    )?;

    let hist_v = history_flatten(rich);
    let history = Tensor::from_vec(hist_v, (1, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D), device)?;

    Ok((state, pheromone, history))
}

/// Batched form: stack N `RichObservation`s into a `(N, ...)` tensor triplet.
pub fn rich_batch_to_tensors(
    riches: &[&RichObservation],
    device: &Device,
) -> Result<(Tensor, Tensor, Tensor)> {
    let n = riches.len();
    let mut state_v = Vec::with_capacity(n * FIXED_STATE_D);
    let mut pher_v = Vec::with_capacity(n * FIXED_PHEROMONE_C * FIXED_PHEROMONE_H * FIXED_PHEROMONE_W);
    let mut hist_v = Vec::with_capacity(n * FIXED_HISTORY_K * FIXED_HISTORY_TOK_D);
    for r in riches {
        state_v.extend_from_slice(&state_flatten(r));
        pher_v.extend_from_slice(&pheromone_flatten(r));
        hist_v.extend_from_slice(&history_flatten(r));
    }
    let state = Tensor::from_vec(state_v, (n, FIXED_STATE_D), device)?;
    let pheromone = Tensor::from_vec(
        pher_v,
        (n, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W),
        device,
    )?;
    let history = Tensor::from_vec(hist_v, (n, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D), device)?;
    Ok((state, pheromone, history))
}

/// Convert a slice of [`AntObservation`]s to batched `(cone, internal, intent)`
/// tensors. The intent tensor is broadcast from a `(1, FIXED_INTENT_D)` input
/// to `(N, FIXED_INTENT_D)`.
pub fn ant_obs_to_tensors(
    obs: &[AntObservation],
    intent_per_colony: &Tensor,
    device: &Device,
) -> Result<(Tensor, Tensor, Tensor)> {
    let b = obs.len();
    let mut cone_v = Vec::with_capacity(b * FIXED_CONE_D);
    let mut internal_v = Vec::with_capacity(b * FIXED_INTERNAL_D);
    for o in obs {
        cone_v.extend_from_slice(&o.pheromone_cone);
        internal_v.extend_from_slice(&o.internal);
    }
    let cone = Tensor::from_vec(cone_v, (b, FIXED_CONE_D), device)?;
    let internal = Tensor::from_vec(internal_v, (b, FIXED_INTERNAL_D), device)?;
    // `broadcast_as` yields a stride-0 view; candle's CUDA matmul (in the
    // ant intent_encoder) requires contiguous operands, so materialize it.
    // No-op on values; CPU never cared, GPU does.
    let intent = intent_per_colony.broadcast_as((b, FIXED_INTENT_D))?.contiguous()?;
    Ok((cone, internal, intent))
}

// ───── private flatten helpers ─────

fn state_flatten(rich: &RichObservation) -> Vec<f32> {
    let s = &rich.state;
    let ed = if s.enemy_distance_min.is_finite() { s.enemy_distance_min } else { 1e6 };
    vec![
        s.food_stored, s.food_inflow_recent,
        s.worker_count as f32, s.soldier_count as f32, s.breeder_count as f32,
        s.brood_egg as f32, s.brood_larva as f32, s.brood_pupa as f32,
        s.queens_alive as f32, s.combat_losses_recent as f32,
        ed, s.enemy_worker_count as f32, s.enemy_soldier_count as f32,
        s.day_of_year as f32, s.ambient_temp_c,
        if s.diapause_active { 1.0 } else { 0.0 },
        if s.is_daytime { 1.0 } else { 0.0 },
    ]
}

fn pheromone_flatten(rich: &RichObservation) -> Vec<f32> {
    let p = &rich.pheromone_field;
    let mut v = Vec::with_capacity(FIXED_PHEROMONE_C * FIXED_PHEROMONE_H * FIXED_PHEROMONE_W);
    v.extend_from_slice(&p.food_trail);
    v.extend_from_slice(&p.home_trail);
    v.extend_from_slice(&p.alarm);
    v.extend_from_slice(&p.colony_scent);
    v
}

fn history_flatten(rich: &RichObservation) -> Vec<f32> {
    let mut v = Vec::with_capacity(FIXED_HISTORY_K * FIXED_HISTORY_TOK_D);
    for tok in rich.history.iter() {
        v.extend_from_slice(&tok.state);
        v.extend_from_slice(&tok.action);
        v.push(tok.reward);
        v.extend_from_slice(&tok.pad);
    }
    while v.len() < FIXED_HISTORY_K * FIXED_HISTORY_TOK_D {
        v.push(0.0);
    }
    let _ = HistoryToken::FLAT_LEN; // compile-time anchor for the HistoryToken use
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use antcolony_sim::config::{
        AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig,
        WorldConfig,
    };
    use antcolony_sim::{Simulation, Topology};

    fn build_sim() -> Simulation {
        let cfg = SimConfig {
            world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
            pheromone: PheromoneConfig::default(),
            ant: AntConfig { initial_count: 10, ..AntConfig::default() },
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        };
        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        Simulation::new_ai_vs_ai_with_topology(cfg, topology, 0xa17, 0, 2)
    }

    #[test]
    fn rich_to_tensors_shapes() {
        let device = Device::Cpu;
        let sim = build_sim();
        let rich = sim.colony_rich_observation(0).unwrap();
        let (s, p, h) = rich_to_tensors(&rich, &device).unwrap();
        assert_eq!(s.dims(), &[1, FIXED_STATE_D]);
        assert_eq!(p.dims(), &[1, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W]);
        assert_eq!(h.dims(), &[1, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D]);
    }

    #[test]
    fn rich_batch_to_tensors_stacks_two_colonies() {
        let device = Device::Cpu;
        let sim = build_sim();
        let rich0 = sim.colony_rich_observation(0).unwrap();
        let rich1 = sim.colony_rich_observation(1).unwrap();
        let (s, p, h) = rich_batch_to_tensors(&[&rich0, &rich1], &device).unwrap();
        assert_eq!(s.dims(), &[2, FIXED_STATE_D]);
        assert_eq!(p.dims(), &[2, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W]);
        assert_eq!(h.dims(), &[2, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D]);
    }

    #[test]
    fn ant_obs_to_tensors_broadcasts_intent() {
        let device = Device::Cpu;
        let sim = build_sim();
        let obs = sim.per_ant_observations(0);
        let intent_per_colony = Tensor::randn(0.0f32, 1.0, (1, FIXED_INTENT_D), &device).unwrap();
        let (c, i, intent_b) = ant_obs_to_tensors(&obs, &intent_per_colony, &device).unwrap();
        let n = obs.len();
        assert_eq!(c.dims(), &[n, FIXED_CONE_D]);
        assert_eq!(i.dims(), &[n, FIXED_INTERNAL_D]);
        assert_eq!(intent_b.dims(), &[n, FIXED_INTENT_D]);
    }
}

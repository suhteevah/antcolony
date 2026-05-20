//! Phase 2a end-to-end smoke: build a fresh Simulation, collect a
//! RichObservation + per-ant AntObservations, build a HierarchicalActorCritic
//! at A1 size, and run forward through both tiers.
//!
//! No training, no gradients — just shape + numerics correctness. If this
//! passes, Phase 2a's plumbing is end-to-end correct and Phase 2b can
//! layer PPO on top.

use candle_core::{DType, Device, Tensor};
use candle_nn::{VarBuilder, VarMap};

use antcolony_sim::ai::observation::{AntObservation, RichObservation};
use antcolony_sim::config::{
    AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig,
};
use antcolony_sim::{Simulation, Topology};

use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::HierarchicalActorCritic;

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

/// Convert one RichObservation to (state, pheromone, history) tensors with batch=1.
fn rich_to_tensors(rich: &RichObservation, device: &Device) -> (Tensor, Tensor, Tensor) {
    // 17-d state vector — match the layout in antcolony-trainer/src/backend.rs::state_to_tensor.
    let s = &rich.state;
    let ed = if s.enemy_distance_min.is_finite() { s.enemy_distance_min } else { 1e6 };
    let state_v: Vec<f32> = vec![
        s.food_stored, s.food_inflow_recent,
        s.worker_count as f32, s.soldier_count as f32, s.breeder_count as f32,
        s.brood_egg as f32, s.brood_larva as f32, s.brood_pupa as f32,
        s.queens_alive as f32, s.combat_losses_recent as f32,
        ed, s.enemy_worker_count as f32, s.enemy_soldier_count as f32,
        s.day_of_year as f32, s.ambient_temp_c,
        if s.diapause_active { 1.0 } else { 0.0 },
        if s.is_daytime { 1.0 } else { 0.0 },
    ];
    debug_assert_eq!(state_v.len(), 17);
    let state = Tensor::from_vec(state_v, (1, 17), device).unwrap();

    // Pheromone field: [1, 4, 32, 32] from 4 Box<[f32]> channels each length 32*32.
    let p = &rich.pheromone_field;
    let mut pher_v: Vec<f32> = Vec::with_capacity(4 * 32 * 32);
    pher_v.extend_from_slice(&p.food_trail);
    pher_v.extend_from_slice(&p.home_trail);
    pher_v.extend_from_slice(&p.alarm);
    pher_v.extend_from_slice(&p.colony_scent);
    let pheromone = Tensor::from_vec(pher_v, (1, 4, 32, 32), device).unwrap();

    // History tokens: pad to K=8 with zero tokens if the colony's ring has fewer.
    // Token layout: 17 + 6 + 1 + 72 = 96 floats.
    let mut hist_v: Vec<f32> = Vec::with_capacity(8 * 96);
    for tok in rich.history.iter() {
        hist_v.extend_from_slice(&tok.state);
        hist_v.extend_from_slice(&tok.action);
        hist_v.push(tok.reward);
        hist_v.extend_from_slice(&tok.pad);
    }
    // Pad to full K=8 tokens.
    while hist_v.len() < 8 * 96 {
        hist_v.push(0.0);
    }
    let history = Tensor::from_vec(hist_v, (1, 8, 96), device).unwrap();

    (state, pheromone, history)
}

/// Convert a Vec<AntObservation> to (cone, internal, intent) tensors batched along ants.
/// All ants in the colony share the same intent (broadcast from commander) — we tile it.
fn ant_obs_to_tensors(
    obs: &[AntObservation],
    intent_per_colony: &Tensor,
    device: &Device,
) -> (Tensor, Tensor, Tensor) {
    let b = obs.len();
    let mut cone_v: Vec<f32> = Vec::with_capacity(b * 60);
    let mut internal_v: Vec<f32> = Vec::with_capacity(b * 8);
    for o in obs {
        cone_v.extend_from_slice(&o.pheromone_cone);
        internal_v.extend_from_slice(&o.internal);
    }
    let cone = Tensor::from_vec(cone_v, (b, 60), device).unwrap();
    let internal = Tensor::from_vec(internal_v, (b, 8), device).unwrap();
    // Broadcast intent: [1, 64] → [b, 64]
    let intent = intent_per_colony.broadcast_as((b, 64)).unwrap();
    (cone, internal, intent)
}

#[test]
fn a1_hac_drives_from_fresh_sim() {
    let device = Device::Cpu;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let hac = HierarchicalActorCritic::new(vb, A1).unwrap();

    let sim = build_sim();

    // Commander forward
    let rich = sim.colony_rich_observation(0).expect("colony 0 exists");
    let (state, pheromone, history) = rich_to_tensors(&rich, &device);
    let cmdr_out = hac.forward_commander(&state, &pheromone, &history).unwrap();
    assert_eq!(cmdr_out.action.dims(), &[1, 6]);
    assert_eq!(cmdr_out.intent.dims(), &[1, 64]);
    assert_eq!(cmdr_out.value.dims(), &[1]);

    // Ant forward (broadcast commander intent to each ant)
    let ant_obs = sim.per_ant_observations(0);
    assert!(!ant_obs.is_empty(), "expected ants in colony 0");
    let (cone, internal, intent_b) = ant_obs_to_tensors(&ant_obs, &cmdr_out.intent, &device);
    let ant_out = hac.forward_ant(&cone, &internal, &intent_b).unwrap();
    assert_eq!(ant_out.modulator.dims(), &[ant_obs.len(), 5]);
    assert_eq!(ant_out.value.dims(), &[ant_obs.len()]);

    // Numerics sanity: nothing is NaN/Inf.
    let action_v: Vec<f32> = cmdr_out.action.flatten_all().unwrap().to_vec1().unwrap();
    assert!(action_v.iter().all(|v| v.is_finite()),
        "commander action contained non-finite values: {:?}", action_v);
    let mod_v: Vec<f32> = ant_out.modulator.flatten_all().unwrap().to_vec1().unwrap();
    assert!(mod_v.iter().all(|v| v.is_finite()),
        "ant modulator contained non-finite values: {:?}", mod_v);
}

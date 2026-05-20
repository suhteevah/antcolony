//! Phase 2a end-to-end smoke: build a fresh Simulation, collect a
//! RichObservation + per-ant AntObservations, build a HierarchicalActorCritic
//! at A1 size, and run forward through both tiers.
//!
//! No training, no gradients — just shape + numerics correctness. If this
//! passes, Phase 2a's plumbing is end-to-end correct and Phase 2b can
//! layer PPO on top.

use candle_core::{DType, Device};
use candle_nn::{VarBuilder, VarMap};

use antcolony_sim::config::{
    AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig,
};
use antcolony_sim::{Simulation, Topology};

use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::{HierarchicalActorCritic, ant_obs_to_tensors, rich_to_tensors};

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
fn a1_hac_drives_from_fresh_sim() {
    let device = Device::Cpu;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let hac = HierarchicalActorCritic::new(vb, A1).unwrap();

    let sim = build_sim();

    // Commander forward
    let rich = sim.colony_rich_observation(0).expect("colony 0 exists");
    let (state, pheromone, history) = rich_to_tensors(&rich, &device).unwrap();
    let cmdr_out = hac.forward_commander(&state, &pheromone, &history).unwrap();
    assert_eq!(cmdr_out.action.dims(), &[1, 6]);
    assert_eq!(cmdr_out.intent.dims(), &[1, 64]);
    assert_eq!(cmdr_out.value.dims(), &[1]);

    // Ant forward (broadcast commander intent to each ant)
    let ant_obs = sim.per_ant_observations(0);
    assert!(!ant_obs.is_empty(), "expected ants in colony 0");
    let (cone, internal, intent_b) = ant_obs_to_tensors(&ant_obs, &cmdr_out.intent, &device).unwrap();
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

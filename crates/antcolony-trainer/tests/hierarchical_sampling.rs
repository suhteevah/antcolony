//! Integration tests for the Phase 2b-1 sampling + log-prob methods on
//! HierarchicalActorCritic.

use candle_core::{Device, Tensor};
use candle_nn::{VarBuilder, VarMap};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::HierarchicalActorCritic;

fn cpu_hac() -> (VarMap, HierarchicalActorCritic, Device) {
    let varmap = VarMap::new();
    let device = Device::Cpu;
    let vb = VarBuilder::from_varmap(&varmap, candle_core::DType::F32, &device);
    let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
    (varmap, hac, device)
}

#[test]
fn sample_commander_shapes_and_finite() {
    let (_vm, hac, device) = cpu_hac();
    let b = 2usize;
    let state = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_state_d), &device).unwrap();
    let pheromone = Tensor::randn(
        0.0f32, 1.0,
        (b, A1.fixed_pheromone_c, A1.fixed_pheromone_h, A1.fixed_pheromone_w),
        &device,
    ).unwrap();
    let history = Tensor::randn(
        0.0f32, 1.0,
        (b, A1.fixed_history_k, A1.fixed_history_tok_d),
        &device,
    ).unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(0xfeed);
    let s = hac.sample_commander(&state, &pheromone, &history, &mut rng).unwrap();
    assert_eq!(s.action.dims(), &[b, A1.fixed_action_d]);
    assert_eq!(s.intent.dims(), &[b, A1.fixed_intent_d]);
    assert_eq!(s.value.dims(), &[b]);
    assert_eq!(s.log_prob.dims(), &[b]);

    let action_v: Vec<f32> = s.action.flatten_all().unwrap().to_vec1().unwrap();
    assert!(action_v.iter().all(|v| v.is_finite()));
    // Post-squash action is in [0, 1] per the existing ActorCritic convention.
    assert!(action_v.iter().all(|v| (0.0..=1.0).contains(v)),
        "post-squash action should be in [0, 1], got {:?}", action_v);

    let lp_v: Vec<f32> = s.log_prob.flatten_all().unwrap().to_vec1().unwrap();
    assert!(lp_v.iter().all(|v| v.is_finite()),
        "log_prob non-finite: {:?}", lp_v);
}

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

/// HAC with all-zero weights. With mean=0, only Box-Muller noise determines u,
/// keeping pre-squash values in ±4 so tanh doesn't saturate and atanh round-trips.
fn zeros_hac() -> (HierarchicalActorCritic, Device) {
    let device = Device::Cpu;
    let vb = VarBuilder::zeros(candle_core::DType::F32, &device);
    let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
    (hac, device)
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

#[test]
fn log_prob_round_trip_through_squash() {
    // If we sample an action via sample_commander, then ask
    // log_prob_of_commander_action for the SAME action under the SAME policy,
    // we should get approximately the SAME log_prob. Tolerance 1e-3 accounts
    // for numerical noise from the atanh edge clamp.
    // Use zeros_hac so all network weights are zero, giving mean=0 for every
    // action dim. Box-Muller noise with the seeded RNG then produces u in ±4
    // (very unlikely to exceed), so tanh does not saturate and atanh inverts
    // cleanly within the 1e-3 tolerance stated in the plan.
    let (hac, device) = zeros_hac();
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

    let mut rng = ChaCha8Rng::seed_from_u64(0xcafe);
    let s = hac.sample_commander(&state, &pheromone, &history, &mut rng).unwrap();
    let lp_round = hac.log_prob_of_commander_action(&state, &pheromone, &history, &s.action).unwrap();

    let lp_sample: Vec<f32> = s.log_prob.flatten_all().unwrap().to_vec1().unwrap();
    let lp_recompute: Vec<f32> = lp_round.flatten_all().unwrap().to_vec1().unwrap();
    for (sample, recompute) in lp_sample.iter().zip(lp_recompute.iter()) {
        assert!(
            (sample - recompute).abs() < 1e-3,
            "log_prob round-trip mismatch: sample={sample}, recompute={recompute}, diff={}",
            (sample - recompute).abs(),
        );
    }
}

#[test]
fn sample_ant_shapes_and_finite() {
    let (_vm, hac, device) = cpu_hac();
    let b = 7usize;
    let cone = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_cone_d), &device).unwrap();
    let intern = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_internal_d), &device).unwrap();
    let intent = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_intent_d), &device).unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(0xc0ffee);
    let s = hac.sample_ant(&cone, &intern, &intent, &mut rng).unwrap();
    assert_eq!(s.modulator.dims(), &[b, A1.fixed_modulator_d]);
    assert_eq!(s.value.dims(), &[b]);
    assert_eq!(s.log_prob.dims(), &[b]);

    let mod_v: Vec<f32> = s.modulator.flatten_all().unwrap().to_vec1().unwrap();
    assert!(mod_v.iter().all(|v| v.is_finite()));
    assert!(mod_v.iter().all(|v| (0.0..=1.0).contains(v)),
        "post-squash modulator should be in [0, 1], got {:?}", mod_v);

    let lp_v: Vec<f32> = s.log_prob.flatten_all().unwrap().to_vec1().unwrap();
    assert!(lp_v.iter().all(|v| v.is_finite()));
}

#[test]
fn ant_log_prob_round_trip() {
    // Use zeros_hac so all network weights are zero → mean=0 for every
    // modulator dim. Box-Muller noise then produces u well within ±4, so
    // tanh does not saturate and atanh inverts cleanly within 1e-3.
    let (hac, device) = zeros_hac();
    let b = 5usize;
    let cone = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_cone_d), &device).unwrap();
    let intern = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_internal_d), &device).unwrap();
    let intent = Tensor::randn(0.0f32, 1.0, (b, A1.fixed_intent_d), &device).unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(0xdeed);
    let s = hac.sample_ant(&cone, &intern, &intent, &mut rng).unwrap();
    let lp_round = hac.log_prob_of_ant_modulator(&cone, &intern, &intent, &s.modulator).unwrap();

    let lp_sample: Vec<f32> = s.log_prob.flatten_all().unwrap().to_vec1().unwrap();
    let lp_recompute: Vec<f32> = lp_round.flatten_all().unwrap().to_vec1().unwrap();
    for (sample, recompute) in lp_sample.iter().zip(lp_recompute.iter()) {
        assert!((sample - recompute).abs() < 1e-3,
            "ant log_prob round-trip mismatch: sample={sample}, recompute={recompute}");
    }
}

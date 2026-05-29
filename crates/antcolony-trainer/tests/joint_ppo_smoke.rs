//! End-to-end Phase 2b-2 smoke: the joint trainer completes 5 iterations
//! on CPU f32 without NaN/crash and measurably moves both tiers' weights.

use antcolony_trainer::{JointPpoConfig, JointPpoTrainer};
use antcolony_trainer::hierarchical::sizing::A1;
use candle_core::Device;

/// Flatten ALL vars whose name contains `name_contains` into one Vec.
/// Concatenating all matching vars makes the "did any weight move" check
/// robust regardless of HashMap iteration order — a single log_std that
/// barely changes won't hide weight updates in the actual linear layers.
fn flat(vm: &candle_nn::VarMap, name_contains: &str) -> Option<Vec<f32>> {
    let data = vm.data().lock().unwrap();
    let mut out = Vec::new();
    for (name, var) in data.iter() {
        if name.contains(name_contains) {
            let v: Vec<f32> = var.as_tensor().flatten_all().ok()?.to_vec1::<f32>().ok()?;
            out.extend(v);
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

#[test]
fn joint_ppo_smoke_five_iters_finite_and_moves_both_tiers() {
    let cfg = JointPpoConfig::smoke_default();
    let mut trainer = JointPpoTrainer::new(Device::Cpu, A1, cfg).unwrap();

    // Snapshot one commander param and one ant param before training.
    let cmdr_before = flat(&trainer.varmap, "commander").expect("commander var");
    let ant_before = flat(&trainer.varmap, "ant").expect("ant var");

    let stats = trainer.train().unwrap();

    assert_eq!(stats.len(), 5, "should run exactly 5 iterations");
    for (i, s) in stats.iter().enumerate() {
        assert!(s.total.is_finite(), "iter {} total loss NaN/inf: {}", i, s.total);
        assert!(s.commander.is_finite(), "iter {} commander loss NaN/inf", i);
        assert!(s.ant.is_finite(), "iter {} ant loss NaN/inf", i);
    }

    let cmdr_after = flat(&trainer.varmap, "commander").unwrap();
    let ant_after = flat(&trainer.varmap, "ant").unwrap();

    let cmdr_moved = cmdr_before.iter().zip(&cmdr_after).any(|(a, b)| (a - b).abs() > 1e-9);
    let ant_moved = ant_before.iter().zip(&ant_after).any(|(a, b)| (a - b).abs() > 1e-9);
    assert!(cmdr_moved, "commander tier weights should change after training");
    assert!(ant_moved, "ant tier weights should change after training");
}

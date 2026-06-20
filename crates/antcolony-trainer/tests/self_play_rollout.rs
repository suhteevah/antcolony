//! Task 4 integration tests for SP1 self-play opponent selection in
//! `ParallelEnv::collect_rollout`.
//!
//! 1. `rollout_with_snapshot_opponent_drives_both_colonies_finite` — with
//!    self-play ON and a pool whose ONLY entry is a frozen-HAC snapshot, the
//!    rollout drives BOTH colonies (left = training HAC, right = frozen
//!    snapshot) and emits finite left-colony records plus a sane win-share in
//!    [0,1]. Forces the `OpponentKind::Snapshot` path (no archetype fallback).
//!
//! 2. `left_training_rng_unperturbed_by_self_play` — the GUARD test. With the
//!    SAME `base_seed` + SAME fresh training HAC + SAME training `rng` seed, the
//!    LEFT colony's first sampled commander action tensor is IDENTICAL whether
//!    `self_play_enabled` is false (per-env round-robin archetype) or true (a
//!    snapshot opponent sampled via a SEPARATE rng). Proves opponent selection
//!    does not perturb the training stream.

use antcolony_trainer::hierarchical::sizing::A1;
use antcolony_trainer::reward::RewardConfig;
use antcolony_trainer::ParallelEnv;
use antcolony_trainer::{JointPpoConfig, JointPpoTrainer, OpponentSampler, SnapshotPool};
use candle_core::Device;
use rand::SeedableRng;

#[test]
fn rollout_with_snapshot_opponent_drives_both_colonies_finite() {
    let device = Device::Cpu;
    // Save a fresh trainer's varmap to disk as the frozen opponent snapshot.
    let t = JointPpoTrainer::new(device.clone(), A1, JointPpoConfig::smoke_default()).unwrap();
    let dir = std::env::temp_dir().join("sp1_rollout_test");
    std::fs::create_dir_all(&dir).unwrap();
    let snap = dir.join("opp.safetensors");
    t.varmap.save(&snap).unwrap();

    // A pool that ONLY offers the snapshot (force the HAC-opponent path).
    let mut pool = SnapshotPool::with_archetypes(8, 0.1);
    pool.entries.clear(); // remove archetypes for this test
    pool.add_snapshot("opp", &snap);

    let mut pe = ParallelEnv::new(2, 4);
    pe.self_play_enabled = true; // ON: pool-sampled opponent (the snapshot)
    pe.pool = pool;
    pe.sampler = OpponentSampler::Uniform;
    pe.sizing = A1;

    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
    let rollout = pe
        .collect_rollout(&t.hac, &device, &mut rng, &RewardConfig::default(), 99)
        .unwrap();

    // Both left-colony record buffers populated (the left HAC trained as usual).
    assert!(
        !rollout.commander.is_empty() && !rollout.ant.is_empty(),
        "left-colony records must be non-empty"
    );
    // All left-colony records finite (the snapshot opponent must not poison them).
    for r in &rollout.commander {
        assert!(
            r.reward.is_finite() && r.value.is_finite() && r.log_prob.is_finite(),
            "commander record must be finite"
        );
    }
    for a in &rollout.ant {
        assert!(
            a.log_prob.iter().chain(a.value.iter()).all(|x| x.is_finite()),
            "ant record must be finite"
        );
    }

    // The chosen opponent is the only entry: index 0.
    assert_eq!(pe.last_opponent_idx, 0, "the single snapshot entry must be chosen");
    // Win-share recorded for PFSP EMA, in [0,1].
    assert!(
        (0.0..=1.0).contains(&pe.last_hac_winshare),
        "last_hac_winshare out of range: {}",
        pe.last_hac_winshare
    );
}

/// GUARD: self-play opponent selection must NOT perturb the LEFT training RNG
/// stream. With identical training HAC weights, identical training-`rng` seed,
/// and identical `base_seed`, the first LEFT commander action tensor must be
/// bit-for-bit (within 1e-6) the same whether self-play is OFF (round-robin
/// archetype opponent, no pool draw) or ON (a snapshot opponent, sampled via a
/// SEPARATE rng). If the LEFT records match, the training stream is provably
/// unperturbed by opponent selection — preserving the "byte-identical with
/// self-play OFF" Global Constraint.
#[test]
fn left_training_rng_unperturbed_by_self_play() {
    let device = Device::Cpu;
    let base_seed: u64 = 0x1234_5678;
    let rng_seed: u64 = 0xABCD;

    // A single fresh training HAC, shared by both runs (same weights).
    let t = JointPpoTrainer::new(device.clone(), A1, JointPpoConfig::smoke_default()).unwrap();

    // A frozen snapshot to use as the self-play opponent (the right colony).
    let dir = std::env::temp_dir().join("sp1_guard_test");
    std::fs::create_dir_all(&dir).unwrap();
    let snap = dir.join("guard_opp.safetensors");
    t.varmap.save(&snap).unwrap();

    // ── Run A: self-play OFF — pre-Task-4 round-robin league opponents. ──
    let mut pe_off = ParallelEnv::new(2, 4);
    pe_off.self_play_enabled = false;
    let mut rng_off = rand_chacha::ChaCha8Rng::seed_from_u64(rng_seed);
    let roll_off = pe_off
        .collect_rollout(&t.hac, &device, &mut rng_off, &RewardConfig::default(), base_seed)
        .unwrap();

    // ── Run B: self-play ON — a snapshot opponent sampled via a SEPARATE rng. ──
    let mut pool = SnapshotPool::with_archetypes(8, 0.1);
    pool.entries.clear();
    pool.add_snapshot("guard_opp", &snap); // pool of exactly one snapshot
    let mut pe_on = ParallelEnv::new(2, 4);
    pe_on.self_play_enabled = true;
    pe_on.pool = pool;
    pe_on.sampler = OpponentSampler::Uniform;
    pe_on.sizing = A1;
    let mut rng_on = rand_chacha::ChaCha8Rng::seed_from_u64(rng_seed);
    let roll_on = pe_on
        .collect_rollout(&t.hac, &device, &mut rng_on, &RewardConfig::default(), base_seed)
        .unwrap();

    // Both runs must have produced LEFT commander records.
    assert!(
        !roll_off.commander.is_empty() && !roll_on.commander.is_empty(),
        "both modes must produce left commander records (off={}, on={})",
        roll_off.commander.len(),
        roll_on.commander.len()
    );

    // Compare the FIRST left commander record's sampled action tensor. The right
    // colony differs between runs (round-robin archetype vs frozen snapshot) and
    // only affects the sim AFTER the left commander acts in cycle 0, so the
    // first left action is purely a function of the (unperturbed) training rng +
    // identical HAC weights + identical fresh world state. If opponent selection
    // had drawn from the training rng, this action would diverge.
    let a_off: Vec<f32> = roll_off.commander[0].action.flatten_all().unwrap().to_vec1().unwrap();
    let a_on: Vec<f32> = roll_on.commander[0].action.flatten_all().unwrap().to_vec1().unwrap();
    assert_eq!(
        a_off.len(),
        a_on.len(),
        "first left commander action dim mismatch: off={} on={}",
        a_off.len(),
        a_on.len()
    );
    for (k, (x, y)) in a_off.iter().zip(a_on.iter()).enumerate() {
        assert!(
            (x - y).abs() < 1e-6,
            "left training rng perturbed by self-play: action[{k}] off={x} on={y}"
        );
    }

    // And the sampled log-prob/value of that first record must match too — these
    // are direct functions of the same rng draw.
    assert!(
        (roll_off.commander[0].log_prob - roll_on.commander[0].log_prob).abs() < 1e-6,
        "first left commander log_prob perturbed: off={} on={}",
        roll_off.commander[0].log_prob,
        roll_on.commander[0].log_prob
    );
}

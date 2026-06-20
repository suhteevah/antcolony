//! Task 4 integration test: a rollout whose ONLY pool entry is a frozen-HAC
//! snapshot must drive BOTH colonies (left = training HAC, right = frozen
//! snapshot) and emit finite left-colony records plus a sane win-share in
//! [0,1]. This forces the `OpponentKind::Snapshot` path in `collect_rollout`
//! (no archetype available to fall back to).

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

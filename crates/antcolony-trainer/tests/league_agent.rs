//! Integration test for LeagueAgent (Task 3 / SP2).
//! Eval-light: 2 training iters, no full-match eval, runs in <1 min on CPU.

use antcolony_trainer::{
    exploiter_league::LeagueAgent,
    joint_ppo::JointPpoConfig,
    self_play::{OpponentSampler, Role, SnapshotPool},
    reward::RewardConfig,
    hierarchical::sizing::A1,
    JointPpoTrainer,
};
use candle_core::Device;

#[test]
fn league_agent_train_snapshot_reset() {
    let tmp = std::env::temp_dir().join("antcolony_league_agent_test");
    std::fs::create_dir_all(&tmp).unwrap();

    // Build a fresh varmap and save it as the warm-start checkpoint.
    let cfg = JointPpoConfig::smoke_default();
    let source = JointPpoTrainer::new(Device::Cpu, A1, cfg.clone())
        .expect("JointPpoTrainer::new failed");
    let warm_start_path = tmp.join("warm_start.safetensors");
    source.varmap.save(&warm_start_path).expect("save warm_start failed");

    // Build the LeagueAgent warm-started from that checkpoint.
    let mut agent = LeagueAgent::new(
        Role::Main,
        A1,
        Device::Cpu,
        cfg,
        Some(&warm_start_path),
    )
    .expect("LeagueAgent::new failed");

    // 1-snapshot pool: save the warm-start weights as the single opponent.
    let mut pool = SnapshotPool::with_archetypes(8, 0.1);
    pool.add_snapshot("warm_start", warm_start_path.clone(), Role::Main);

    // Train for 2 iters (eval-light — no eval).
    let reward = RewardConfig::default();
    agent
        .train_iters(2, &pool, OpponentSampler::Uniform, &reward, 7)
        .expect("train_iters failed");

    assert_eq!(agent.iters_since_reset, 2, "iters_since_reset should be 2");
    assert!(
        (0.0..=1.0).contains(&agent.winrate_ema),
        "winrate_ema={} out of [0,1]",
        agent.winrate_ema
    );

    // Snapshot current weights, then reset and verify.
    let snap = tmp.join("snap.safetensors");
    agent.snapshot_to(&snap).expect("snapshot_to failed");
    assert!(snap.exists(), "snapshot file must exist after snapshot_to");

    agent.reset_from(&snap).expect("reset_from failed");
    assert_eq!(agent.iters_since_reset, 0, "iters_since_reset should be 0 after reset_from");
}

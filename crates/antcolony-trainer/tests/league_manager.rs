//! Integration smoke for LeagueManager (Task 4 / SP2).
//!
//! Eval-light: 2 league-steps x 3 agents (main + 1 main-exploiter + 1
//! league-exploiter) x 2 iters, A1, CPU, no full-match eval
//! (`eval_every_steps = 99`). Verifies the orchestration loop runs, writes at
//! least one promoted/main snapshot file, and that the forced-reset path fires
//! (`exploiter_max_iters = 2` => a ForcedReset is guaranteed within the run).

use antcolony_trainer::{
    exploiter_league::{LeagueConfig, LeagueManager},
    hierarchical::sizing::A1,
    joint_ppo::JointPpoConfig,
    JointPpoTrainer,
};
use candle_core::Device;

#[test]
fn league_manager_round_robin_smoke() {
    let tmp = std::env::temp_dir().join("antcolony_league_manager_test");
    std::fs::create_dir_all(&tmp).unwrap();
    let snapshot_dir = tmp.join("snapshots");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    // Build + save a fresh varmap as the "sota" the league warm-starts from.
    let sota_path = tmp.join("sota.safetensors");
    let source = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default())
        .expect("JointPpoTrainer::new failed");
    source.varmap.save(&sota_path).expect("save sota failed");
    assert!(sota_path.exists(), "sota checkpoint must exist before new()");

    // Smoke config pointed at our temp dirs + sota.
    let cfg = LeagueConfig::smoke(snapshot_dir.clone(), sota_path.clone());

    let mut mgr = LeagueManager::new(cfg, Device::Cpu).expect("LeagueManager::new failed");
    let report = mgr.run().expect("LeagueManager::run failed");

    // Ran the configured number of steps.
    assert_eq!(report.steps, 2, "expected 2 league-steps, got {}", report.steps);

    // At least one exp_* or main_* snapshot file written to snapshot_dir.
    let snap_files: Vec<String> = std::fs::read_dir(&snapshot_dir)
        .expect("read snapshot_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("exp_") || n.starts_with("main_"))
        .collect();
    assert!(
        !snap_files.is_empty(),
        "expected >=1 exp_*/main_* snapshot in {}, found {:?}",
        snapshot_dir.display(),
        std::fs::read_dir(&snapshot_dir)
            .map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name()).collect::<Vec<_>>())
            .unwrap_or_default()
    );

    // The forced-reset path fired at least once (exploiter_max_iters = 2).
    assert!(
        report.exploiter_resets >= 1,
        "expected >=1 exploiter reset (forced), got {}",
        report.exploiter_resets
    );

    // Eval-derived fields are left untouched in Task 4 (populated by Task 5).
    assert_eq!(report.best_h2h_vs_sota, 0.0);
    assert_eq!(report.best_step, 0);
    assert_eq!(report.final_bench, 0.0);
}

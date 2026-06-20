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

/// Task 5: success eval (h2h vs SOTA + forgetting guard) + keep-best.
///
/// Uses `eval_every_steps = 1` so every league step triggers an eval.
/// `success_mpe = 1` for speed. Asserts:
///   - `report.best_h2h_vs_sota` is in [0.0, 1.0]
///   - `report.final_bench` is in [0.0, 1.0]
///   - `snapshot_dir/league_best.safetensors` exists (keep-best was written)
#[test]
fn league_manager_success_eval_smoke() {
    let tmp = std::env::temp_dir().join("antcolony_league_eval_test");
    std::fs::create_dir_all(&tmp).unwrap();
    let snapshot_dir = tmp.join("snapshots_eval");
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    // Build + save a fresh varmap as the SOTA the league warm-starts from.
    let sota_path = tmp.join("sota_eval.safetensors");
    let source = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default())
        .expect("JointPpoTrainer::new failed");
    source.varmap.save(&sota_path).expect("save sota failed");
    assert!(sota_path.exists(), "sota checkpoint must exist before new()");

    // Eval config: eval_every_steps=1 so every step triggers eval;
    // success_mpe=1 for speed.
    let mut cfg = LeagueConfig::smoke(snapshot_dir.clone(), sota_path.clone());
    cfg.eval_every_steps = 1;
    cfg.success_mpe = 1;

    let mut mgr = LeagueManager::new(cfg, Device::Cpu).expect("LeagueManager::new failed");
    let report = mgr.run().expect("LeagueManager::run failed");

    // best_h2h_vs_sota must be a valid probability in [0, 1].
    assert!(
        (0.0..=1.0).contains(&report.best_h2h_vs_sota),
        "best_h2h_vs_sota out of [0,1]: {}",
        report.best_h2h_vs_sota
    );

    // final_bench must be a valid probability in [0, 1].
    assert!(
        (0.0..=1.0).contains(&report.final_bench),
        "final_bench out of [0,1]: {}",
        report.final_bench
    );

    // keep-best file must be written.
    assert!(
        snapshot_dir.join("league_best.safetensors").exists(),
        "league_best.safetensors not found in {}",
        snapshot_dir.display()
    );
}

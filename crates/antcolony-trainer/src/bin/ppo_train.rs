//! PPO training driver — pure-Rust trainer. Runs the colony sim
//! IN-PROCESS (no subprocess overhead), trains via PPO with GAE,
//! exports weights in MlpBrain JSON format (Rust sim loads them via
//! the existing `mlp:<path>` spec).
//!
//! Usage:
//!   cargo run --release --bin ppo-train -- --iterations 30 --out bench/ppo-rust-r5
//!
//! Population-based RL flags (added 2026-05-04 to break the 45.7% BC ceiling):
//!   --include-baseline <path>   Add an MLP weights JSON to the league at startup.
//!                                Forces PPO to differentiate from its own warm-start.
//!   --snapshot-every N           Every N iterations, dump current weights and add
//!                                them to the league as a tier-2 self-snapshot.
//!   --curriculum                 Use weighted opponent sampling (heuristic-heavy
//!                                early, MLP/snapshot-heavy late) instead of
//!                                round-robin.
//!
//! With CUDA:
//!   cargo run --release --features antcolony-trainer/cuda --bin ppo-train -- ...

use antcolony_trainer::{Backend, CandleBackend, PpoConfig, PpoTrainer};
use antcolony_trainer::ppo::RolloutBatch;
use rand::SeedableRng;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("antcolony_sim=warn,antcolony_trainer=info,ppo_train=info")
        .with_target(false)
        .init();

    let mut iterations = 5_usize;
    let mut matches_per_iter = 8_usize;
    let mut out_dir = PathBuf::from("bench/ppo-rust-r1");
    let mut warm_start: Option<PathBuf> = None;
    let mut include_baselines: Vec<PathBuf> = Vec::new();
    let mut snapshot_every: usize = 0;  // 0 = disabled
    let mut curriculum = false;
    // --noisy-pool <weights_path>:<std1,std2,...>  e.g.
    //   --noisy-pool bench/iterative-fsp/round_1/mlp_weights_v1.json:0.05,0.1,0.2
    // adds 3 stochastic variants of that MLP to the league as tier-2.
    let mut noisy_pool: Option<(PathBuf, Vec<f32>)> = None;
    // --add-opp <name>:<spec>  e.g.
    //   --add-opp mix_da:mix:defender,aggressor
    //   --add-opp mix_eco:mix:economist=2,forager=1,heuristic=1
    // Pushes arbitrary opponent specs into the league (tier 1). Use this
    // to widen the bench past the deterministic 7-archetype Nash plateau.
    let mut extra_opps: Vec<(String, String)> = Vec::new();
    // --value-clip <f>  PPO value-loss clipping range. 0 disables.
    let mut value_clip: f32 = 0.0;
    // --hidden-dim <n>  Width of actor/critic MLP hidden layers. Default 64
    // (matches mlp_weights_v1). 128 doubles capacity and is one path to
    // break the 47% Nash plateau. Cold-start required when bumped — old
    // 64-dim weight files can't warm-start a 128-dim net.
    let mut hidden_dim: usize = antcolony_trainer::HIDDEN_DIM;
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--iterations" => { iterations = raw[i+1].parse()?; i += 2; }
            "--matches-per-iter" => { matches_per_iter = raw[i+1].parse()?; i += 2; }
            "--out" => { out_dir = PathBuf::from(&raw[i+1]); i += 2; }
            "--start" => { warm_start = Some(PathBuf::from(&raw[i+1])); i += 2; }
            "--include-baseline" => { include_baselines.push(PathBuf::from(&raw[i+1])); i += 2; }
            "--snapshot-every" => { snapshot_every = raw[i+1].parse()?; i += 2; }
            "--curriculum" => { curriculum = true; i += 1; }
            "--add-opp" => {
                let arg = &raw[i+1];
                // Split on FIRST colon: name:spec  (spec may itself contain colons)
                let (name, spec) = arg.split_once(':')
                    .unwrap_or_else(|| panic!("--add-opp expects `<name>:<spec>`, got `{arg}`"));
                extra_opps.push((name.to_string(), spec.to_string()));
                i += 2;
            }
            "--value-clip" => { value_clip = raw[i+1].parse()?; i += 2; }
            "--hidden-dim" => { hidden_dim = raw[i+1].parse()?; i += 2; }
            "--noisy-pool" => {
                let arg = &raw[i+1];
                let mut parts = arg.rsplitn(2, ':');
                let stds_s = parts.next().expect("--noisy-pool missing std list");
                let path_s = parts.next().expect("--noisy-pool missing path");
                let stds: Vec<f32> = stds_s.split(',').filter_map(|s| s.parse().ok()).collect();
                noisy_pool = Some((PathBuf::from(path_s), stds));
                i += 2;
            }
            other => anyhow::bail!("unknown arg `{other}`"),
        }
    }
    std::fs::create_dir_all(&out_dir)?;
    let snapshots_dir = out_dir.join("snapshots");
    std::fs::create_dir_all(&snapshots_dir)?;

    let backend = CandleBackend::new()?;
    tracing::info!(cuda = backend.cuda_available(), "backend ready");

    let mut config = PpoConfig::default();
    config.iterations = iterations;
    config.matches_per_iter = matches_per_iter;
    config.value_clip = value_clip;
    config.hidden_dim = hidden_dim;
    if value_clip > 0.0 {
        tracing::info!(value_clip, "PPO value-loss clipping enabled");
    }
    if hidden_dim != antcolony_trainer::HIDDEN_DIM {
        tracing::info!(hidden_dim, default = antcolony_trainer::HIDDEN_DIM, "non-default hidden width");
    }

    let mut trainer = PpoTrainer::new(backend.device().clone(), config.clone())?;
    if let Some(ws_path) = &warm_start {
        trainer.warm_start_actor(ws_path)?;
        tracing::info!(path = %ws_path.display(), "warm-started actor from MlpBrain weights");
    }

    // Inject baselines as tier-2 league members. These are typically previous
    // SOTAs (e.g. mlp_weights_v1.json) the new policy must learn to beat — the
    // canonical fix for the BC-ceiling-collapse failure mode.
    for (idx, path) in include_baselines.iter().enumerate() {
        let name = format!("baseline_{idx}");
        trainer.league.add_mlp_snapshot(&name, path);
        tracing::info!(name = %name, path = %path.display(), "added league baseline");
    }

    for (name, spec) in &extra_opps {
        // Tier 1 (mid-difficulty); curriculum sampler treats them as
        // archetype-class opponents — appropriate for a stochastic mix.
        trainer.league.add_spec(name, spec, 1);
        tracing::info!(name = %name, spec = %spec, "added extra opponent");
    }

    if let Some((path, stds)) = &noisy_pool {
        for (idx, std) in stds.iter().enumerate() {
            let name = format!("noisy_{idx}_std{}", (std * 1000.0) as i32);
            trainer.league.add_noisy_mlp(&name, path, *std);
            tracing::info!(name = %name, path = %path.display(), std, "added noisy MLP variant");
        }
    }

    // Initial export so we can verify shape immediately.
    let weights_path = out_dir.join("current.json");
    trainer.export_mlp_weights(&weights_path)?;
    tracing::info!(path = %weights_path.display(), "initial weights exported");

    // === Training loop: rollout → GAE → PPO update → log ===
    let mut optimizer = trainer.make_optimizer()?;
    let mut sample_rng = rand_chacha::ChaCha8Rng::seed_from_u64(0xc011a6e);
    for it in 1..=iterations {
        let progress = if iterations <= 1 { 1.0 } else { (it - 1) as f32 / (iterations - 1) as f32 };

        // Aggregate rollouts across all matches in this iteration
        let mut all_states: Vec<candle_core::Tensor> = Vec::new();
        let mut all_actions: Vec<candle_core::Tensor> = Vec::new();
        let mut all_returns: Vec<f32> = Vec::new();
        let mut all_advantages: Vec<f32> = Vec::new();
        let mut all_log_probs: Vec<f32> = Vec::new();
        let mut all_old_values: Vec<f32> = Vec::new();
        let mut total_reward = 0.0_f32;
        let mut opp_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        for m in 0..matches_per_iter {
            let n_opps = trainer.league.entries.len();
            let opp_idx = if curriculum {
                trainer.league.sample_curriculum(progress, &mut sample_rng)
            } else {
                (it * matches_per_iter + m) % n_opps
            };
            let opp_spec = trainer.league.entries[opp_idx].spec.clone();
            let opp_name = trainer.league.entries[opp_idx].name.clone();
            *opp_counts.entry(opp_name).or_insert(0) += 1;
            let seed = (10_000u64 * it as u64) + m as u64;
            let batch: RolloutBatch = trainer.rollout(&opp_spec, seed)?;
            total_reward += batch.rewards.iter().sum::<f32>();
            // GAE per episode
            let (adv, ret) = PpoTrainer::compute_gae(
                &batch.rewards, &batch.values, &batch.dones,
                trainer.config.gamma, trainer.config.gae_lambda,
            );
            all_states.extend(batch.states);
            all_actions.extend(batch.actions);
            all_log_probs.extend(batch.log_probs);
            all_old_values.extend(batch.values);
            all_advantages.extend(adv);
            all_returns.extend(ret);
        }

        let n = all_states.len();
        if n == 0 {
            tracing::warn!(it, "no trajectories — skipping update");
            continue;
        }

        let loss = trainer.ppo_update(
            &mut optimizer,
            &all_states, &all_actions,
            &all_returns, &all_advantages, &all_log_probs, &all_old_values,
        )?;

        let opp_dist: String = {
            let mut v: Vec<_> = opp_counts.iter().collect();
            v.sort_by(|a, b| a.0.cmp(b.0));
            v.iter().map(|(k, c)| format!("{k}={c}")).collect::<Vec<_>>().join(",")
        };
        tracing::info!(
            it, n_samples = n,
            avg_reward = total_reward / matches_per_iter as f32,
            loss, progress, opps = %opp_dist, "iter done");

        // Export weights so the league can sample them as opponents next round
        trainer.export_mlp_weights(&weights_path)?;

        // Periodic self-snapshot — adds the current policy to the league
        // as a future opponent. This is the population-based-RL move:
        // forces subsequent iterations to differentiate from past selves.
        if snapshot_every > 0 && it % snapshot_every == 0 {
            let snap_name = format!("snap_it{it:04}");
            let snap_path = snapshots_dir.join(format!("{snap_name}.json"));
            trainer.export_mlp_weights(&snap_path)?;
            trainer.league.add_mlp_snapshot(&snap_name, &snap_path);
            tracing::info!(snap = %snap_name, path = %snap_path.display(), "self-snapshot added to league");
        }
    }

    trainer.export_mlp_weights(&weights_path)?;
    tracing::info!(path = %weights_path.display(), "final weights exported");
    Ok(())
}

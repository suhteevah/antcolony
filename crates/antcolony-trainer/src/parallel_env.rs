//! Parallel-env rollout for single-GPU Phase-3 training.
//!
//! Holds N `MatchEnv`s, each with the left colony driven by the HAC and the
//! right by a league-sampled `AiBrain`. The sim steps on CPU; the left
//! colony's observations are batched across all *active* envs into single
//! GPU forwards (commander every DECISION_CADENCE ticks, ants every tick),
//! and outputs scattered back. Emits the existing `JointRollout` with
//! left-colony records only, `match_idx = env_idx`, `colony = 0`, so the
//! Phase-2b-2 `joint_update` + GAE are reused unchanged.

use anyhow::Result;
use candle_core::{Device, Tensor};
use rand_chacha::ChaCha8Rng;

use antcolony_sim::ai::observation::AntModulators;
use antcolony_sim::{AiBrain, AiDecision, MatchStatus};

use crate::env::{MatchEnv, DECISION_CADENCE};
use crate::hierarchical::obs_to_tensors::{ant_obs_to_tensors, rich_batch_to_tensors};
use crate::hierarchical::sizing::{FIXED_INTENT_D, FIXED_MODULATOR_D};
use crate::joint_ppo::{AntBatch, CommanderRecord, JointRollout};
use crate::reward::{compute_step_reward, ColonyMetrics, RewardConfig};
use crate::HierarchicalActorCritic;
use crate::League;

pub struct ParallelEnv {
    pub n_envs: usize,
    pub rollout_cycles: usize,
    pub league: League,
}

impl ParallelEnv {
    pub fn new(n_envs: usize, rollout_cycles: usize) -> Self {
        Self { n_envs, rollout_cycles, league: League::default_pool() }
    }
}

/// Decode one row of a [B, 6] squashed-action tensor into an AiDecision.
fn row_to_decision(action: &Tensor, row: usize) -> Result<AiDecision> {
    let v: Vec<f32> = action.narrow(0, row, 1)?.squeeze(0)?.to_vec1()?;
    Ok(AiDecision {
        caste_ratio_worker: v[0],
        caste_ratio_soldier: v[1],
        caste_ratio_breeder: v[2],
        forage_weight: v[3],
        dig_weight: v[4],
        nurse_weight: v[5],
        research_choice: None,
    })
}

/// One row of a [B, 64] intent tensor into a fixed [f32; 64] array.
fn row_to_intent(intent: &Tensor, row: usize) -> Result<[f32; FIXED_INTENT_D]> {
    let v: Vec<f32> = intent.narrow(0, row, 1)?.flatten_all()?.to_vec1()?;
    let mut arr = [0.0f32; FIXED_INTENT_D];
    arr.copy_from_slice(&v);
    Ok(arr)
}

impl ParallelEnv {
    /// Collect one parallel rollout. Creates `n_envs` fresh matches (left =
    /// HAC, right = league-sampled opponent), runs up to `rollout_cycles`
    /// commander cycles, and returns left-colony records bucketed by env.
    /// `base_seed` decorrelates this rollout's env/opponent seeds.
    pub fn collect_rollout(
        &mut self,
        hac: &HierarchicalActorCritic,
        device: &Device,
        rng: &mut ChaCha8Rng,
        reward: &RewardConfig,
        base_seed: u64,
    ) -> Result<JointRollout> {
        tracing::debug!(n_envs = self.n_envs, rollout_cycles = self.rollout_cycles, base_seed, "parallel rollout start");
        let mut envs: Vec<MatchEnv> = Vec::with_capacity(self.n_envs);
        let mut opponents: Vec<Box<dyn AiBrain>> = Vec::with_capacity(self.n_envs);
        for i in 0..self.n_envs {
            let seed = base_seed ^ ((i as u64).wrapping_mul(0x9E3779B97F4A7C15));
            envs.push(MatchEnv::new(seed));
            let pick = (i + (base_seed as usize)) % self.league.entries.len();
            let spec = self.league.entries[pick].spec.clone();
            opponents.push(League::make_brain(&spec, seed.wrapping_add(1)));
        }

        let mut out = JointRollout::default();
        let mut done = vec![false; self.n_envs];
        let mut prev: Vec<[ColonyMetrics; 2]> = (0..self.n_envs)
            .map(|i| {
                [
                    ColonyMetrics::from_sim(&envs[i].sim, 0),
                    ColonyMetrics::from_sim(&envs[i].sim, 1),
                ]
            })
            .collect();

        for cycle in 0..self.rollout_cycles {
            let active: Vec<usize> = (0..self.n_envs)
                .filter(|&i| !done[i] && envs[i].sim.colony_rich_observation(0).is_some())
                .collect();
            if active.is_empty() {
                break;
            }

            // ── Commander forward, batched across active envs ──
            let riches: Vec<_> = active
                .iter()
                .map(|&i| {
                    envs[i]
                        .sim
                        .colony_rich_observation(0)
                        .expect("active => Some: filtered by is_some() above")
                })
                .collect();
            let rich_refs: Vec<_> = riches.iter().collect();
            let (state_b, pher_b, hist_b) = rich_batch_to_tensors(&rich_refs, device)?;
            let cmdr = hac.sample_commander(&state_b, &pher_b, &hist_b, rng)?;
            let cmdr_lp: Vec<f32> = cmdr.log_prob.to_vec1()?;
            let cmdr_val: Vec<f32> = cmdr.value.to_vec1()?;

            for (j, &i) in active.iter().enumerate() {
                let dec = row_to_decision(&cmdr.action, j)?;
                envs[i].sim.apply_ai_decision(0, &dec);
                let intent = row_to_intent(&cmdr.intent, j)?;
                envs[i].sim.apply_commander_intent(0, &intent);
                if let Some(sr) = envs[i].sim.colony_ai_state(1) {
                    let dr = opponents[i].decide(&sr);
                    envs[i].sim.apply_ai_decision(1, &dr);
                }
            }

            // ── Tick loop with per-tick batched ant decisions over active envs ──
            for _ in 0..DECISION_CADENCE {
                let mut cones: Vec<Tensor> = Vec::new();
                let mut internals: Vec<Tensor> = Vec::new();
                let mut intents: Vec<Tensor> = Vec::new();
                let mut index_map: Vec<(usize, u32)> = Vec::new();
                for (j, &i) in active.iter().enumerate() {
                    if done[i] {
                        continue;
                    }
                    let obs = envs[i].sim.per_ant_observations(0);
                    if obs.is_empty() {
                        continue;
                    }
                    // Slice one intent row [1, 64] for this env's ants.
                    let intent_row = cmdr.intent.narrow(0, j, 1)?;
                    let (cone, internal, intent_b) =
                        ant_obs_to_tensors(&obs, &intent_row, device)?;
                    for o in &obs {
                        index_map.push((i, o.ant_id));
                    }
                    cones.push(cone);
                    internals.push(internal);
                    intents.push(intent_b);
                }
                if !index_map.is_empty() {
                    let cone = Tensor::cat(&cones, 0)?;
                    let internal = Tensor::cat(&internals, 0)?;
                    let intent = Tensor::cat(&intents, 0)?;
                    let ant = hac.sample_ant(&cone, &internal, &intent, rng)?;
                    // Pull the whole batch host-side ONCE (one GPU->CPU sync each)
                    // instead of per-ant narrow+to_vec1. On CUDA a per-ant sync per
                    // row costs ~ms; with tens of thousands of ant rows that was the
                    // dominant cost (≈50 min/iter). to_vec2 transfers [M,5] in one go.
                    let lp: Vec<f32> = ant.log_prob.to_vec1()?;
                    let val: Vec<f32> = ant.value.to_vec1()?;
                    let mod_rows: Vec<Vec<f32>> = ant.modulator.to_vec2()?;
                    // Scatter modulators back to each env's sim (per-env grouped).
                    let mut row = 0usize;
                    for &i in &active {
                        if done[i] {
                            continue;
                        }
                        let mut mods: Vec<AntModulators> = Vec::new();
                        let mut ids: Vec<u32> = Vec::new();
                        // index_map is grouped by env in active order; consume contiguous block.
                        while row < index_map.len() && index_map[row].0 == i {
                            let m = &mod_rows[row];
                            mods.push(AntModulators {
                                alpha_mult: m[0],
                                beta_mult: m[1],
                                exploration_mod: m[2],
                                deposit_mult: m[3],
                                state_bias: m[4],
                            });
                            ids.push(index_map[row].1);
                            row += 1;
                        }
                        if !ids.is_empty() {
                            envs[i].sim.apply_ant_modulators(0, &mods, &ids);
                        }
                    }
                    // Compile-time guard: the m[0..5] decode above assumes a 5-d modulator.
                    const _: () = assert!(FIXED_MODULATOR_D == 5);
                    // Store the WHOLE tick batch (one tensor each), not per-ant —
                    // keeps the GPU cat in joint_update cheap. Row order matches
                    // index_map (env-grouped); match_idx = env index, colony = 0.
                    out.ant.push(AntBatch {
                        match_idx: index_map.iter().map(|&(e, _)| e).collect(),
                        colony: vec![0u8; index_map.len()],
                        cycle,
                        cone: cone.detach(),
                        internal: internal.detach(),
                        intent: intent.detach(),
                        modulator: ant.modulator.detach(),
                        log_prob: lp,
                        value: val,
                    });
                }
                for &i in &active {
                    if done[i] {
                        continue;
                    }
                    envs[i].sim.tick();
                    if !matches!(envs[i].sim.match_status(), MatchStatus::InProgress)
                        || envs[i].sim.tick >= envs[i].max_ticks
                    {
                        done[i] = true;
                    }
                }
            }

            // ── Per-cycle reward + commander records for active envs ──
            // Host-side copies of the per-env state/action rows, pulled ONCE
            // (two GPU->CPU syncs) for the history tokens below — avoids a
            // per-record narrow+to_vec1 sync per active env.
            let state_rows: Vec<Vec<f32>> = state_b.to_vec2()?;
            let action_rows: Vec<Vec<f32>> = cmdr.action.to_vec2()?;
            for (j, &i) in active.iter().enumerate() {
                let cur = [
                    ColonyMetrics::from_sim(&envs[i].sim, 0),
                    ColonyMetrics::from_sim(&envs[i].sim, 1),
                ];
                let status = envs[i].sim.match_status();
                let (reward_left, _reward_right) =
                    compute_step_reward(reward, &prev[i], &cur, done[i], status);
                prev[i] = cur;
                out.commander.push(CommanderRecord {
                    match_idx: i,
                    colony: 0,
                    cycle,
                    state: state_b.narrow(0, j, 1)?.detach(),
                    pheromone: pher_b.narrow(0, j, 1)?.detach(),
                    history: hist_b.narrow(0, j, 1)?.detach(),
                    action: cmdr.action.narrow(0, j, 1)?.detach(),
                    log_prob: cmdr_lp[j],
                    value: cmdr_val[j],
                    reward: reward_left,
                    done: done[i],
                });
                let mut st = [0.0f32; 17];
                st.copy_from_slice(&state_rows[j]);
                let mut ac = [0.0f32; 6];
                ac.copy_from_slice(&action_rows[j]);
                envs[i].sim.push_commander_history(0, st, ac, reward_left);
            }
        }
        tracing::debug!(
            commander_records = out.commander.len(),
            ant_records = out.ant.len(),
            "parallel rollout complete"
        );
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchical::sizing::A1;
    use crate::reward::RewardConfig;
    use candle_core::{DType, Device};
    use candle_nn::{VarBuilder, VarMap};
    use rand::SeedableRng;

    #[test]
    fn parallel_env_constructs_with_default_league() {
        let pe = ParallelEnv::new(4, 8);
        assert_eq!(pe.n_envs, 4);
        assert_eq!(pe.rollout_cycles, 8);
        assert_eq!(pe.league.entries.len(), 7, "default pool = 7 archetypes");
    }

    #[test]
    fn collect_rollout_fills_buffer_left_only_env_bucketed() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0xa1);
        let reward = RewardConfig::default();

        let mut pe = ParallelEnv::new(3, 4);
        let roll = pe.collect_rollout(&hac, &device, &mut rng, &reward, 0xfeed).unwrap();

        assert!(!roll.commander.is_empty());
        assert!(!roll.ant.is_empty());
        assert!(roll.commander.iter().all(|r| r.colony == 0));
        // Left-vs-league: every ant row is colony 0, every match_idx is an env.
        assert!(roll.ant.iter().all(|a| a.colony.iter().all(|&c| c == 0)));
        assert!(roll.ant.iter().all(|a| a.match_idx.iter().all(|&e| e < 3)));
        let envs_seen: std::collections::HashSet<usize> =
            roll.commander.iter().map(|r| r.match_idx).collect();
        assert!(envs_seen.iter().all(|&e| e < 3));
        assert!(!envs_seen.is_empty());
        for r in &roll.commander {
            assert_eq!(r.state.dims(), &[1, 17]);
            assert!(r.reward.is_finite() && r.value.is_finite() && r.log_prob.is_finite());
        }
        for a in &roll.ant {
            let mt = a.len();
            assert!(mt >= 1);
            assert_eq!(a.modulator.dims(), &[mt, 5]);
            assert_eq!(a.match_idx.len(), mt);
            assert!(a.log_prob.iter().chain(a.value.iter()).all(|x| x.is_finite()));
        }
    }
}

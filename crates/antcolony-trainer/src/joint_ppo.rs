//! Joint PPO trainer for the hierarchical (commander + ant) brain.
//!
//! Self-plays both colonies of a `MatchEnv` with one shared
//! `HierarchicalActorCritic`, collects a two-buffer rollout (commander
//! @ cycle cadence, ant @ tick cadence), computes per-tier GAE at the
//! colony level, and optimizes `L_total = L_cmdr + α_balance · L_ant`
//! with a single AdamW step. Phase 2b-2 scope: single-device (CPU f32)
//! smoke. See docs/superpowers/plans/2026-05-29-ant-brain-phase2b2-joint-trainer.md.

use crate::env::{MatchEnv, DECISION_CADENCE};
use crate::hierarchical::sizing::Sizing;
use crate::HierarchicalActorCritic;
use antcolony_sim::{AiDecision, MatchStatus};
use candle_core::{DType, Device, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};

#[derive(Clone, Debug)]
pub struct JointPpoConfig {
    pub iterations: usize,
    pub matches_per_iter: usize,
    /// Hard cap on commander decision cycles collected per match. Keeps
    /// the smoke rollout short (one cycle = DECISION_CADENCE outer ticks).
    pub rollout_cycles: usize,
    pub gamma: f32,
    pub gae_lambda: f32,
    pub clip: f32,
    pub epochs_per_batch: usize,
    pub lr: f64,
    pub value_coef: f64,
    pub cmdr_entropy_coef: f64,
    pub ant_entropy_coef: f64,
    /// Down-weights the ant-tier loss so ~10× more ant samples per cycle
    /// don't drown the commander gradient. Design spec §"Joint PPO loss".
    pub alpha_balance: f64,
    pub seed: u64,
}

impl JointPpoConfig {
    /// Minimal config for the mechanical smoke: 5 iters, 2 matches/iter,
    /// 8 cycles/match. Tuned for "runs without NaN/crash in under a
    /// minute on CPU", not for convergence.
    pub fn smoke_default() -> Self {
        Self {
            iterations: 5,
            matches_per_iter: 2,
            rollout_cycles: 8,
            gamma: 0.99,
            gae_lambda: 0.95,
            clip: 0.2,
            epochs_per_batch: 1,
            lr: 3e-4,
            value_coef: 0.5,
            cmdr_entropy_coef: 0.005,
            ant_entropy_coef: 0.01,
            alpha_balance: 0.1,
            seed: 0xa17_c01_2b2,
        }
    }
}

pub struct JointPpoTrainer {
    pub hac: HierarchicalActorCritic,
    pub varmap: VarMap,
    pub device: Device,
    pub config: JointPpoConfig,
    pub rng: rand_chacha::ChaCha8Rng,
}

impl JointPpoTrainer {
    pub fn new(device: Device, sizing: Sizing, config: JointPpoConfig) -> anyhow::Result<Self> {
        use rand::SeedableRng;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, sizing)?;
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(config.seed);
        Ok(Self { hac, varmap, device, config, rng })
    }

    /// AdamW over every VarMap parameter (both tiers).
    pub fn make_optimizer(&self) -> anyhow::Result<AdamW> {
        let params = ParamsAdamW {
            lr: self.config.lr,
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-8,
            weight_decay: 0.0,
        };
        Ok(AdamW::new(self.varmap.all_vars(), params)?)
    }

    /// Self-play one match for up to `rollout_cycles` commander cycles.
    /// Both colonies are driven by the shared HAC. Returns the two-buffer
    /// rollout. `match_idx` stamps every record so per-match GAE never
    /// bleeds across match boundaries in `joint_update`.
    pub fn rollout(&mut self, seed: u64, match_idx: usize) -> anyhow::Result<JointRollout> {
        let dev = self.device.clone();
        let mut env = MatchEnv::new(seed);
        let mut out = JointRollout::default();

        let mut prev_workers = [colony_workers(&env, 0), colony_workers(&env, 1)];
        let mut prev_food = [colony_food(&env, 0), colony_food(&env, 1)];

        for cycle in 0..self.config.rollout_cycles {
            // ── Commander decision (both colonies, batch leading dim 2) ──
            // If a colony has been eliminated, commander_obs_batch errors;
            // treat that as match end.
            let (state_b, pher_b, hist_b) = match env.commander_obs_batch(&dev) {
                Ok(t) => t,
                Err(_) => break,
            };
            let cmdr = self.hac.sample_commander(&state_b, &pher_b, &hist_b, &mut self.rng)?;
            let dec0 = row_to_decision(&cmdr.action, 0)?;
            let dec1 = row_to_decision(&cmdr.action, 1)?;
            env.sim.apply_ai_decision(0, &dec0);
            env.sim.apply_ai_decision(1, &dec1);
            env.apply_commander_intents(&cmdr.intent)?; // intent is [2, 64]

            // ── Outer tick loop with per-tick batched ant decisions ──
            let mut done = false;
            for _ in 0..DECISION_CADENCE {
                let (cone, internal, intent_b, index_map) =
                    env.all_ant_obs_batch(&cmdr.intent, &dev)?;
                if !index_map.is_empty() {
                    let ant = self.hac.sample_ant(&cone, &internal, &intent_b, &mut self.rng)?;
                    env.apply_ant_modulators_batched(&ant.modulator, &index_map)?;
                    let lp: Vec<f32> = ant.log_prob.to_vec1()?;
                    let val: Vec<f32> = ant.value.to_vec1()?;
                    for (i, &(cid, _aid)) in index_map.iter().enumerate() {
                        out.ant.push(AntRecord {
                            match_idx,
                            colony: cid,
                            cycle,
                            cone: cone.narrow(0, i, 1)?.detach(),
                            internal: internal.narrow(0, i, 1)?.detach(),
                            intent: intent_b.narrow(0, i, 1)?.detach(),
                            modulator: ant.modulator.narrow(0, i, 1)?.detach(),
                            log_prob: lp[i],
                            value: val[i],
                        });
                    }
                }
                env.sim.tick();
                if !matches!(env.sim.match_status(), MatchStatus::InProgress)
                    || env.sim.tick >= env.max_ticks
                {
                    done = true;
                    break;
                }
            }

            // ── Per-cycle r6 reward (mirrors MatchEnv::step) ──
            let workers_now = [colony_workers(&env, 0), colony_workers(&env, 1)];
            let food_now = [colony_food(&env, 0), colony_food(&env, 1)];
            let q = [colony_queen_alive(&env, 0), colony_queen_alive(&env, 1)];
            let dl = workers_now[0] as i32 - prev_workers[0] as i32;
            let dr = workers_now[1] as i32 - prev_workers[1] as i32;
            let dfl = food_now[0] - prev_food[0];
            let dfr = food_now[1] - prev_food[1];
            let mut reward_left = (dl as f32) * 0.01 - (dr as f32) * 0.01
                + dfl * 0.002 - dfr * 0.002
                + (q[0] - q[1]) * 0.005;
            let mut reward_right = -reward_left;
            if done {
                match env.sim.match_status() {
                    MatchStatus::Won { winner: 0, .. } => { reward_left += 1.0; reward_right -= 1.0; }
                    MatchStatus::Won { winner: 1, .. } => { reward_left -= 1.0; reward_right += 1.0; }
                    MatchStatus::InProgress => {
                        let total = (workers_now[0] + workers_now[1]).max(1) as f32;
                        let share = workers_now[0] as f32 / total;
                        reward_left += (share - 0.5) * 2.0;
                        reward_right += (0.5 - share) * 2.0;
                    }
                    _ => {}
                }
            }
            prev_workers = workers_now;
            prev_food = food_now;

            // ── Commander records (split the [2, ..] batch per colony) ──
            let cmdr_lp: Vec<f32> = cmdr.log_prob.to_vec1()?;
            let cmdr_val: Vec<f32> = cmdr.value.to_vec1()?;
            let rewards = [reward_left, reward_right];
            for k in 0..2usize {
                out.commander.push(CommanderRecord {
                    match_idx,
                    colony: k as u8,
                    cycle,
                    state: state_b.narrow(0, k, 1)?.detach(),
                    pheromone: pher_b.narrow(0, k, 1)?.detach(),
                    history: hist_b.narrow(0, k, 1)?.detach(),
                    action: cmdr.action.narrow(0, k, 1)?.detach(),
                    log_prob: cmdr_lp[k],
                    value: cmdr_val[k],
                    reward: rewards[k],
                    done,
                });
            }

            // ── Append history token to each colony's ring for next cycle ──
            let state_rows = state_b.to_vec2::<f32>()?;   // [2, 17]
            let action_rows = cmdr.action.to_vec2::<f32>()?; // [2, 6]
            for k in 0..2usize {
                let mut st = [0.0f32; 17];
                st.copy_from_slice(&state_rows[k]);
                let mut ac = [0.0f32; 6];
                ac.copy_from_slice(&action_rows[k]);
                env.sim.push_commander_history(k as u8, st, ac, rewards[k]);
            }

            if done {
                break;
            }
        }
        Ok(out)
    }
}

fn colony_workers(env: &MatchEnv, k: u8) -> u32 {
    env.sim.colonies.get(k as usize).map(|c| c.population.workers).unwrap_or(0)
}
fn colony_food(env: &MatchEnv, k: u8) -> f32 {
    env.sim.colonies.get(k as usize).map(|c| c.food_stored).unwrap_or(0.0)
}
fn colony_queen_alive(env: &MatchEnv, k: u8) -> f32 {
    env.sim.colonies.get(k as usize)
        .map(|c| if c.queen_health > 0.0 { 1.0 } else { 0.0 })
        .unwrap_or(0.0)
}

/// Decode one row of a [B, 6] post-squash action tensor into an AiDecision.
fn row_to_decision(action_batch: &candle_core::Tensor, row: usize) -> anyhow::Result<AiDecision> {
    let r = action_batch.narrow(0, row, 1)?.squeeze(0)?; // [6]
    let v: Vec<f32> = r.to_vec1()?;
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

/// One commander decision for one colony in one cycle of one match.
pub struct CommanderRecord {
    pub match_idx: usize,
    pub colony: u8,
    pub cycle: usize,
    pub state: Tensor,     // [1, 17]
    pub pheromone: Tensor, // [1, 4, 32, 32]
    pub history: Tensor,   // [1, 8, 96]
    pub action: Tensor,    // [1, 6] post-squash
    pub log_prob: f32,
    pub value: f32,
    pub reward: f32,
    pub done: bool,
}

/// One ant decision for one ant in one tick of one cycle.
pub struct AntRecord {
    pub match_idx: usize,
    pub colony: u8,
    pub cycle: usize,
    pub cone: Tensor,      // [1, 60]
    pub internal: Tensor,  // [1, 8]
    pub intent: Tensor,    // [1, 64]
    pub modulator: Tensor, // [1, 5] post-squash
    pub log_prob: f32,
    pub value: f32,
}

#[derive(Default)]
pub struct JointRollout {
    pub commander: Vec<CommanderRecord>,
    pub ant: Vec<AntRecord>,
}

/// Per-iteration loss breakdown for logging + the smoke assertion.
#[derive(Clone, Debug)]
pub struct JointLossStats {
    pub total: f32,
    pub commander: f32,
    pub ant: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchical::sizing::A1;
    use candle_core::Device;

    #[test]
    fn rollout_fills_both_buffers_with_finite_records() {
        let mut t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let roll = t.rollout(0xfeed_1, 0).unwrap();

        assert!(!roll.commander.is_empty(), "commander buffer must be non-empty");
        assert!(!roll.ant.is_empty(), "ant buffer must be non-empty");

        // Commander: 2 colonies per cycle, all rows the right shape + finite.
        for r in &roll.commander {
            assert_eq!(r.state.dims(), &[1, 17]);
            assert_eq!(r.pheromone.dims(), &[1, 4, 32, 32]);
            assert_eq!(r.history.dims(), &[1, 8, 96]);
            assert_eq!(r.action.dims(), &[1, 6]);
            assert!(r.log_prob.is_finite() && r.value.is_finite() && r.reward.is_finite());
            assert!(r.colony == 0 || r.colony == 1);
        }
        // Ant rows finite + shaped.
        for a in &roll.ant {
            assert_eq!(a.cone.dims(), &[1, 60]);
            assert_eq!(a.internal.dims(), &[1, 8]);
            assert_eq!(a.intent.dims(), &[1, 64]);
            assert_eq!(a.modulator.dims(), &[1, 5]);
            assert!(a.log_prob.is_finite() && a.value.is_finite());
        }
        // Both colonies represented in the commander buffer.
        assert!(roll.commander.iter().any(|r| r.colony == 0));
        assert!(roll.commander.iter().any(|r| r.colony == 1));
    }

    #[test]
    fn smoke_default_has_five_iters_and_balanced_coefs() {
        let c = JointPpoConfig::smoke_default();
        assert_eq!(c.iterations, 5);
        assert_eq!(c.matches_per_iter, 2);
        assert_eq!(c.rollout_cycles, 8);
        assert!(c.alpha_balance > 0.0 && c.alpha_balance <= 1.0);
        assert!(c.ant_entropy_coef >= c.cmdr_entropy_coef);
    }

    #[test]
    fn trainer_builds_at_a1_with_nonzero_params() {
        let t = JointPpoTrainer::new(Device::Cpu, A1, JointPpoConfig::smoke_default()).unwrap();
        let total: usize = t.varmap.all_vars().iter()
            .map(|v| v.dims().iter().product::<usize>()).sum();
        assert!(total > 1_000_000, "A1 HAC should have >1M params, got {}", total);
        let _opt = t.make_optimizer().unwrap();
    }

    #[test]
    fn joint_rollout_defaults_empty() {
        let r = JointRollout::default();
        assert!(r.commander.is_empty());
        assert!(r.ant.is_empty());
    }
}

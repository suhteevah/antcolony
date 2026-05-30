//! Tunable reward shaping for the hierarchical-brain trainer.
//!
//! `RewardConfig::default()` reproduces the established "r6" shaping
//! EXACTLY (the conditions under which MlpBrain v1 was measured at 47.1%),
//! so the headline Phase-3 run stays apples-to-apples. The extra
//! "smartness" levers (`brood_growth`, `food_inflow`, `combat_loss_penalty`)
//! default to 0.0 — set them > 0 to bias the colony toward those behaviors.
//! Reward is zero-sum between the two colonies for the shaping terms
//! (own minus enemy), plus a terminal win/timeout bonus.

use antcolony_sim::{MatchStatus, Simulation};
use serde::{Deserialize, Serialize};

/// Tunable reward weights. Defaults == r6.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RewardConfig {
    pub worker_delta: f32,
    pub food_delta: f32,
    pub queen_bonus: f32,
    pub terminal_win: f32,
    pub timeout_share: f32,
    pub brood_growth: f32,
    pub food_inflow: f32,
    pub combat_loss_penalty: f32,
}

impl Default for RewardConfig {
    fn default() -> Self {
        Self {
            worker_delta: 0.01,
            food_delta: 0.002,
            queen_bonus: 0.005,
            terminal_win: 1.0,
            timeout_share: 1.0,
            brood_growth: 0.0,
            food_inflow: 0.0,
            combat_loss_penalty: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ColonyMetrics {
    pub workers: f32,
    pub food: f32,
    pub queen_alive: f32,
    pub brood: f32,
    pub food_inflow: f32,
    pub combat_losses: f32,
}

impl ColonyMetrics {
    pub fn from_sim(sim: &Simulation, colony_id: u8) -> Self {
        match sim.colony_ai_state(colony_id) {
            Some(s) => Self {
                workers: s.worker_count as f32,
                food: s.food_stored,
                queen_alive: if s.queens_alive > 0 { 1.0 } else { 0.0 },
                brood: (s.brood_egg + s.brood_larva + s.brood_pupa) as f32,
                food_inflow: s.food_inflow_recent,
                combat_losses: s.combat_losses_recent as f32,
            },
            None => Self::default(),
        }
    }
}

pub fn compute_step_reward(
    cfg: &RewardConfig,
    prev: &[ColonyMetrics; 2],
    cur: &[ColonyMetrics; 2],
    done: bool,
    status: MatchStatus,
) -> (f32, f32) {
    let dwl = cur[0].workers - prev[0].workers;
    let dwr = cur[1].workers - prev[1].workers;
    let dfl = cur[0].food - prev[0].food;
    let dfr = cur[1].food - prev[1].food;
    let dbl = cur[0].brood - prev[0].brood;
    let dbr = cur[1].brood - prev[1].brood;

    let mut reward_left = cfg.worker_delta * (dwl - dwr)
        + cfg.food_delta * (dfl - dfr)
        + cfg.queen_bonus * (cur[0].queen_alive - cur[1].queen_alive)
        + cfg.brood_growth * (dbl - dbr)
        + cfg.food_inflow * (cur[0].food_inflow - cur[1].food_inflow)
        - cfg.combat_loss_penalty * (cur[0].combat_losses - cur[1].combat_losses);
    let mut reward_right = -reward_left;

    if done {
        match status {
            MatchStatus::Won { winner: 0, .. } => {
                reward_left += cfg.terminal_win;
                reward_right -= cfg.terminal_win;
            }
            MatchStatus::Won { winner: 1, .. } => {
                reward_left -= cfg.terminal_win;
                reward_right += cfg.terminal_win;
            }
            MatchStatus::InProgress => {
                let total = (cur[0].workers + cur[1].workers).max(1.0);
                let share = cur[0].workers / total;
                reward_left += (share - 0.5) * 2.0 * cfg.timeout_share;
                reward_right += (0.5 - share) * 2.0 * cfg.timeout_share;
            }
            _ => {}
        }
    }
    (reward_left, reward_right)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(workers: f32, food: f32, queen: f32) -> ColonyMetrics {
        ColonyMetrics { workers, food, queen_alive: queen, ..Default::default() }
    }

    #[test]
    fn default_reproduces_r6_shaping_numbers() {
        let cfg = RewardConfig::default();
        let prev = [m(50.0, 200.0, 1.0), m(50.0, 200.0, 1.0)];
        let cur = [m(60.0, 300.0, 1.0), m(50.0, 200.0, 1.0)];
        let (l, r) = compute_step_reward(&cfg, &prev, &cur, false, MatchStatus::InProgress);
        assert!((l - 0.3).abs() < 1e-6, "expected 0.3, got {l}");
        assert!((r + 0.3).abs() < 1e-6, "zero-sum, got {r}");
    }

    #[test]
    fn smartness_levers_off_by_default() {
        let cfg = RewardConfig::default();
        assert_eq!(cfg.brood_growth, 0.0);
        assert_eq!(cfg.food_inflow, 0.0);
        assert_eq!(cfg.combat_loss_penalty, 0.0);
        let mut prev = [ColonyMetrics::default(); 2];
        let mut cur = [ColonyMetrics::default(); 2];
        prev[0].brood = 0.0; cur[0].brood = 100.0;
        let (l, _) = compute_step_reward(&cfg, &prev, &cur, false, MatchStatus::InProgress);
        assert_eq!(l, 0.0, "brood swing must not affect reward under defaults");
    }

    #[test]
    fn brood_growth_lever_rewards_brood_when_enabled() {
        let cfg = RewardConfig { brood_growth: 0.01, ..Default::default() };
        let prev = [ColonyMetrics::default(); 2];
        let mut cur = [ColonyMetrics::default(); 2];
        cur[0].brood = 100.0;
        let (l, r) = compute_step_reward(&cfg, &prev, &cur, false, MatchStatus::InProgress);
        assert!((l - 1.0).abs() < 1e-6, "100*0.01 = 1.0, got {l}");
        assert!((r + 1.0).abs() < 1e-6);
    }

    #[test]
    fn terminal_win_adds_pm_one_by_default() {
        let cfg = RewardConfig::default();
        let prev = [ColonyMetrics::default(); 2];
        let cur = [ColonyMetrics::default(); 2];
        let (l, r) = compute_step_reward(&cfg, &prev, &cur, true,
            MatchStatus::Won { winner: 0, loser: 1, ended_at_tick: 100 });
        assert!((l - 1.0).abs() < 1e-6);
        assert!((r + 1.0).abs() < 1e-6);
    }
}

//! In-process match environment. Wraps Simulation directly — no
//! subprocess, no JSONL serialization. Each step() advances the sim by
//! the decision cadence and returns the new state + reward.

use antcolony_sim::{
    AiBrain, AiDecision, ColonyAiState, MatchStatus, Simulation, Topology,
    config::{AntConfig, ColonyConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig},
};

pub const DECISION_CADENCE: u64 = 5;

#[derive(Clone, Debug)]
pub struct StepRecord {
    pub state_left: ColonyAiState,
    pub state_right: Option<ColonyAiState>,
    pub action_left: AiDecision,
    pub action_right: AiDecision,
    pub reward_left: f32,
    pub reward_right: f32,
    pub done: bool,
    pub tick: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Trajectory {
    pub steps: Vec<StepRecord>,
    pub final_status: Option<MatchStatusSummary>,
}

#[derive(Clone, Debug)]
pub enum MatchStatusSummary {
    LeftWin,
    RightWin,
    Draw,
    TimeoutLeftFavor(f32),  // workers_share for left
}

impl Trajectory {
    pub fn workers_share(&self) -> f32 {
        // Last recorded step's reward delta — placeholder until reward shaping lands
        if let Some(s) = self.steps.last() {
            (s.reward_left + 1.0) / 2.0
        } else {
            0.5
        }
    }
}

pub struct MatchEnv {
    pub sim: Simulation,
    pub max_ticks: u64,
    pub prev_workers: [u32; 2],
    pub prev_queens_alive: [u32; 2],
    pub prev_food: [f32; 2],
}

impl MatchEnv {
    /// Create a fresh match. Bench-matched fixture (32x32 arena, 10 ants/colony).
    pub fn new(seed: u64) -> Self {
        let cfg = SimConfig {
            world: WorldConfig { width: 32, height: 32, ..WorldConfig::default() },
            pheromone: PheromoneConfig::default(),
            ant: AntConfig { initial_count: 10, ..AntConfig::default() },
            colony: ColonyConfig::default(),
            combat: CombatConfig::default(),
            hazards: HazardConfig::default(),
        };
        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        let sim = Simulation::new_ai_vs_ai_with_topology(cfg, topology, seed, 0, 2);
        let prev_workers = [
            sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0),
            sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0),
        ];
        let prev_queens_alive = [1, 1];  // both colonies start with one queen
        let prev_food = [
            sim.colonies.get(0).map(|c| c.food_stored).unwrap_or(0.0),
            sim.colonies.get(1).map(|c| c.food_stored).unwrap_or(0.0),
        ];
        Self {
            sim,
            max_ticks: 10_000,
            prev_workers,
            prev_queens_alive,
            prev_food,
        }
    }

    pub fn observe(&self, colony_id: u8) -> Option<ColonyAiState> {
        self.sim.colony_ai_state(colony_id)
    }

    /// Advance one decision cycle (DECISION_CADENCE outer ticks). Apply
    /// the two brains' decisions, then tick. Compute per-step rewards
    /// from population deltas (worker losses + queen survival) so the
    /// gradient signal isn't sparse.
    pub fn step(&mut self, action_left: &AiDecision, action_right: &AiDecision) -> StepRecord {
        let state_left = self.observe(0).expect("left colony missing");
        let state_right = self.observe(1);

        self.sim.apply_ai_decision(0, action_left);
        self.sim.apply_ai_decision(1, action_right);

        let mut done = false;
        for _ in 0..DECISION_CADENCE {
            self.sim.tick();
            let status = self.sim.match_status();
            if !matches!(status, MatchStatus::InProgress) {
                done = true;
                break;
            }
            if self.sim.tick >= self.max_ticks {
                done = true;
                break;
            }
        }

        // Reward shaping (tuning pass r3): TINY per-step worker-delta
        // (×0.01) + terminal bonus. r2's pure-terminal reward gave
        // advantage≈0 everywhere → policy gradient≈0 → no learning.
        // r1's ×0.05 dominated the terminal. ×0.01 is the goldilocks
        // attempt: weak enough that terminal still matters, strong
        // enough that intermediate steps have a non-zero learning signal.
        let workers_now = [
            self.sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0),
            self.sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0),
        ];
        let dl = workers_now[0] as i32 - self.prev_workers[0] as i32;
        let dr = workers_now[1] as i32 - self.prev_workers[1] as i32;
        let mut reward_left = (dl as f32) * 0.01 - (dr as f32) * 0.01;
        let mut reward_right = -reward_left;
        if done {
            match self.sim.match_status() {
                MatchStatus::Won { winner: 0, .. } => { reward_left += 1.0; reward_right -= 1.0; }
                MatchStatus::Won { winner: 1, .. } => { reward_left -= 1.0; reward_right += 1.0; }
                MatchStatus::Draw { .. } => {}
                MatchStatus::InProgress => {
                    // Timeout: graded by worker share, scaled to [-1, 1].
                    let total = (workers_now[0] + workers_now[1]).max(1) as f32;
                    let share = workers_now[0] as f32 / total;
                    reward_left += (share - 0.5) * 2.0;
                    reward_right += (0.5 - share) * 2.0;
                }
                _ => {}
            }
        }
        self.prev_workers = workers_now;

        StepRecord {
            state_left,
            state_right,
            action_left: action_left.clone(),
            action_right: action_right.clone(),
            reward_left,
            reward_right,
            done,
            tick: self.sim.tick,
        }
    }

    /// Run a full match against `right_brain`, with `decide_left` as a
    /// closure that picks the left brain's action. Returns the trajectory.
    pub fn run_match<F>(&mut self, mut decide_left: F, right_brain: &mut dyn AiBrain) -> Trajectory
    where
        F: FnMut(&ColonyAiState) -> AiDecision,
    {
        let mut traj = Trajectory::default();
        loop {
            let s_left = match self.observe(0) {
                Some(s) => s,
                None => break,
            };
            let action_left = decide_left(&s_left);
            let s_right = self.observe(1);
            let action_right = match s_right.as_ref() {
                Some(sr) => right_brain.decide(sr),
                None => AiDecision { caste_ratio_worker: 0.65, caste_ratio_soldier: 0.30, caste_ratio_breeder: 0.05, forage_weight: 0.55, dig_weight: 0.20, nurse_weight: 0.25, research_choice: None },
            };
            let step = self.step(&action_left, &action_right);
            let done = step.done;
            traj.steps.push(step);
            if done || self.sim.tick >= self.max_ticks { break; }
        }
        traj.final_status = match self.sim.match_status() {
            MatchStatus::Won { winner: 0, .. } => Some(MatchStatusSummary::LeftWin),
            MatchStatus::Won { winner: 1, .. } => Some(MatchStatusSummary::RightWin),
            MatchStatus::Draw { .. } => Some(MatchStatusSummary::Draw),
            MatchStatus::InProgress => {
                let lw = self.sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0) as f32;
                let rw = self.sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0) as f32;
                let share = lw / (lw + rw).max(1.0);
                Some(MatchStatusSummary::TimeoutLeftFavor(share))
            }
            _ => None,
        };
        traj
    }
}

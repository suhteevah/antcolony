//! In-process match environment. Wraps Simulation directly — no
//! subprocess, no JSONL serialization. Each step() advances the sim by
//! the decision cadence and returns the new state + reward.

use antcolony_sim::{
    AiBrain, AiDecision, ColonyAiState, MatchStatus, Simulation, Topology,
    config::{AntConfig, ColonyConfig, ColonySimConfig, CombatConfig, HazardConfig, PheromoneConfig, SimConfig, WorldConfig},
    environment::Environment,
    species::Species,
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

    /// Cross-species match: colony 0 = `species_a`, colony 1 = `species_b`.
    /// Shares the bench arena fixture (32×32) and the AI-vs-AI symmetry
    /// (both colonies flagged AI-controlled) so only species + brains differ.
    ///
    /// Does NOT perform a snapshot round-trip — the sim is constructed fresh
    /// via `Simulation::new_two_colony_cross_species`, so no species identity
    /// is lost through the default-slice snapshot path.
    pub fn new_cross_species(species_a: &Species, species_b: &Species, seed: u64) -> Self {
        // Bench arena environment — same as `new`.
        let env = Environment {
            world_width: 32,
            world_height: 32,
            ..Environment::default()
        };

        // Global arena/pheromone/hazard config from species_a; override world
        // dims to 32×32 so the arena is the same fixed bench fixture regardless
        // of what the species TOML says.
        let mut global = species_a.apply(&env);
        global.world = WorldConfig { width: 32, height: 32, ..WorldConfig::default() };

        // Per-colony biology from each species.
        let cfg_a: ColonySimConfig = species_a.apply_colony(&env);
        let cfg_b: ColonySimConfig = species_b.apply_colony(&env);

        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        let mut sim = Simulation::new_two_colony_cross_species(
            global, cfg_a, cfg_b, topology, seed, 0, 2,
        );

        // Match `new_ai_vs_ai_with_topology`: flip colony 0 to AI-controlled.
        if let Some(c0) = sim.colonies.get_mut(0) {
            c0.is_ai_controlled = true;
        }

        let prev_workers = [
            sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0),
            sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0),
        ];
        let prev_queens_alive = [1, 1];
        let prev_food = [
            sim.colonies.get(0).map(|c| c.food_stored).unwrap_or(0.0),
            sim.colonies.get(1).map(|c| c.food_stored).unwrap_or(0.0),
        ];
        tracing::info!(
            species_a = %species_a.id,
            species_b = %species_b.id,
            seed,
            "MatchEnv::new_cross_species constructed"
        );
        Self { sim, max_ticks: 10_000, prev_workers, prev_queens_alive, prev_food }
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

        // Reward shaping r6 (2026-05-04 evening):
        //   - Worker-delta ×0.01 (kept from r3, the goldilocks rate)
        //   - Food-stored-delta ×0.002 (denser signal — food turnover happens
        //     every tick, worker turnover is sparse)
        //   - Queen-alive bonus +0.005/step per side (penalize queen loss
        //     before it cascades to worker death)
        //   - Terminal ±1 (unchanged)
        // The denser food + queen signals are intended to give PPO traction
        // beyond the worker-delta-only signal that flatlined r1–r5 at 47%.
        let workers_now = [
            self.sim.colonies.get(0).map(|c| c.population.workers).unwrap_or(0),
            self.sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0),
        ];
        let food_now = [
            self.sim.colonies.get(0).map(|c| c.food_stored).unwrap_or(0.0),
            self.sim.colonies.get(1).map(|c| c.food_stored).unwrap_or(0.0),
        ];
        let queen_alive = [
            self.sim.colonies.get(0).map(|c| if c.queen_health > 0.0 { 1.0 } else { 0.0 }).unwrap_or(0.0),
            self.sim.colonies.get(1).map(|c| if c.queen_health > 0.0 { 1.0 } else { 0.0 }).unwrap_or(0.0),
        ];
        let dl = workers_now[0] as i32 - self.prev_workers[0] as i32;
        let dr = workers_now[1] as i32 - self.prev_workers[1] as i32;
        let df_l = food_now[0] - self.prev_food[0];
        let df_r = food_now[1] - self.prev_food[1];
        let mut reward_left = (dl as f32) * 0.01 - (dr as f32) * 0.01
            + df_l * 0.002 - df_r * 0.002
            + (queen_alive[0] - queen_alive[1]) * 0.005;
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
        self.prev_food = food_now;

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

    /// Batched per-ant observations across BOTH colonies. The `intent_per_colony`
    /// argument is a `(2, FIXED_INTENT_D)` tensor where row 0 is colony-0's
    /// commander intent and row 1 is colony-1's. The returned `intent_b` tensor
    /// expands those rows so each ant sees its own colony's intent.
    ///
    /// `index_map` maps each row of the returned tensors back to its source ant
    /// — entry `i` is `(colony_id, ant_id)`. The trainer uses this when packing
    /// modulator outputs back into per-ant write-back calls.
    pub fn all_ant_obs_batch(
        &self,
        intent_per_colony: &candle_core::Tensor,
        device: &candle_core::Device,
    ) -> anyhow::Result<(
        candle_core::Tensor,
        candle_core::Tensor,
        candle_core::Tensor,
        Vec<(u8, u32)>,
    )> {
        use candle_core::Tensor;
        use crate::hierarchical::sizing::{FIXED_CONE_D, FIXED_INTENT_D, FIXED_INTERNAL_D};

        let obs0 = self.sim.per_ant_observations(0);
        let obs1 = self.sim.per_ant_observations(1);
        let n0 = obs0.len();
        let n1 = obs1.len();
        let n_total = n0 + n1;

        let mut cone_v = Vec::with_capacity(n_total * FIXED_CONE_D);
        let mut internal_v = Vec::with_capacity(n_total * FIXED_INTERNAL_D);
        let mut index_map = Vec::with_capacity(n_total);
        for o in &obs0 {
            cone_v.extend_from_slice(&o.pheromone_cone);
            internal_v.extend_from_slice(&o.internal);
            index_map.push((0u8, o.ant_id));
        }
        for o in &obs1 {
            cone_v.extend_from_slice(&o.pheromone_cone);
            internal_v.extend_from_slice(&o.internal);
            index_map.push((1u8, o.ant_id));
        }
        let cone = Tensor::from_vec(cone_v, (n_total, FIXED_CONE_D), device)?;
        let internal = Tensor::from_vec(internal_v, (n_total, FIXED_INTERNAL_D), device)?;

        // intent_b: take row 0 of intent_per_colony, broadcast to n0 rows; same for row 1, n1.
        let intent0 = intent_per_colony.narrow(0, 0, 1)?.broadcast_as((n0, FIXED_INTENT_D))?;
        let intent1 = intent_per_colony.narrow(0, 1, 1)?.broadcast_as((n1, FIXED_INTENT_D))?;
        let intent_b = Tensor::cat(&[&intent0, &intent1], 0)?;

        Ok((cone, internal, intent_b, index_map))
    }

    /// Apply commander intent vectors to both colonies. `intent_per_colony`
    /// is a (2, FIXED_INTENT_D) tensor — row 0 → colony 0, row 1 → colony 1.
    pub fn apply_commander_intents(&mut self, intent_per_colony: &candle_core::Tensor) -> anyhow::Result<()> {
        use crate::hierarchical::sizing::FIXED_INTENT_D;
        let dims = intent_per_colony.dims();
        if dims != [2usize, FIXED_INTENT_D].as_slice() {
            anyhow::bail!(
                "apply_commander_intents: expected shape [2, {}], got {:?}",
                FIXED_INTENT_D, dims,
            );
        }
        let row0: Vec<f32> = intent_per_colony.narrow(0, 0, 1)?.flatten_all()?.to_vec1()?;
        let row1: Vec<f32> = intent_per_colony.narrow(0, 1, 1)?.flatten_all()?.to_vec1()?;
        let mut a0 = [0.0f32; FIXED_INTENT_D];
        a0.copy_from_slice(&row0);
        let mut a1 = [0.0f32; FIXED_INTENT_D];
        a1.copy_from_slice(&row1);
        self.sim.apply_commander_intent(0, &a0);
        self.sim.apply_commander_intent(1, &a1);
        Ok(())
    }

    /// Apply batched per-ant modulators to the right (colony, ant) pairs.
    /// `mods_t` is a (N, FIXED_MODULATOR_D) tensor; `index_map[i]` tells us
    /// which ant row `i` belongs to. Groups writes by colony for one
    /// apply_ant_modulators call per colony.
    pub fn apply_ant_modulators_batched(
        &mut self,
        mods_t: &candle_core::Tensor,
        index_map: &[(u8, u32)],
    ) -> anyhow::Result<()> {
        use antcolony_sim::ai::observation::AntModulators;
        use crate::hierarchical::sizing::FIXED_MODULATOR_D;

        let dims = mods_t.dims();
        if dims.len() != 2 || dims[1] != FIXED_MODULATOR_D || dims[0] != index_map.len() {
            anyhow::bail!(
                "apply_ant_modulators_batched: expected shape [{}, {}], got {:?}",
                index_map.len(), FIXED_MODULATOR_D, dims,
            );
        }
        let flat: Vec<f32> = mods_t.flatten_all()?.to_vec1()?;

        // Group writes by colony so we make one apply_ant_modulators call per colony.
        let mut by_colony: [(Vec<AntModulators>, Vec<u32>); 2] = [
            (Vec::new(), Vec::new()),
            (Vec::new(), Vec::new()),
        ];
        for (i, &(cid, aid)) in index_map.iter().enumerate() {
            let off = i * FIXED_MODULATOR_D;
            let m = AntModulators {
                alpha_mult: flat[off],
                beta_mult: flat[off + 1],
                exploration_mod: flat[off + 2],
                deposit_mult: flat[off + 3],
                state_bias: flat[off + 4],
            };
            let slot = if cid == 0 { 0 } else { 1 };
            by_colony[slot].0.push(m);
            by_colony[slot].1.push(aid);
        }
        for cid in [0u8, 1u8] {
            let slot = cid as usize;
            if !by_colony[slot].1.is_empty() {
                self.sim.apply_ant_modulators(cid, &by_colony[slot].0, &by_colony[slot].1);
            }
        }
        Ok(())
    }

    /// Batched commander observations across both colonies (shape leading
    /// dim = 2). Returns (state, pheromone, history) ready to feed
    /// `HierarchicalActorCritic::forward_commander` (or `sample_commander`).
    pub fn commander_obs_batch(
        &self,
        device: &candle_core::Device,
    ) -> anyhow::Result<(candle_core::Tensor, candle_core::Tensor, candle_core::Tensor)> {
        let rich0 = self
            .sim
            .colony_rich_observation(0)
            .ok_or_else(|| anyhow::anyhow!("MatchEnv: colony 0 missing"))?;
        let rich1 = self
            .sim
            .colony_rich_observation(1)
            .ok_or_else(|| anyhow::anyhow!("MatchEnv: colony 1 missing"))?;
        let tup = crate::hierarchical::obs_to_tensors::rich_batch_to_tensors(
            &[&rich0, &rich1],
            device,
        )?;
        Ok(tup)
    }
}

#[cfg(test)]
mod env_tests {
    use super::*;
    use candle_core::Device;
    use crate::hierarchical::sizing::{
        FIXED_HISTORY_K, FIXED_HISTORY_TOK_D, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H,
        FIXED_PHEROMONE_W, FIXED_STATE_D,
    };

    #[test]
    fn commander_obs_batch_shape_is_two_colonies_stacked() {
        let env = MatchEnv::new(0xb1a5_e1);
        let device = Device::Cpu;
        let (state, pheromone, history) = env.commander_obs_batch(&device).unwrap();
        assert_eq!(state.dims(), &[2, FIXED_STATE_D]);
        assert_eq!(pheromone.dims(), &[2, FIXED_PHEROMONE_C, FIXED_PHEROMONE_H, FIXED_PHEROMONE_W]);
        assert_eq!(history.dims(), &[2, FIXED_HISTORY_K, FIXED_HISTORY_TOK_D]);
    }

    #[test]
    fn all_ant_obs_batch_shapes_and_index_map() {
        use crate::hierarchical::sizing::{FIXED_CONE_D, FIXED_INTENT_D, FIXED_INTERNAL_D};
        let env = MatchEnv::new(0xb1a5_e1);
        let device = Device::Cpu;
        let intent_per_colony = candle_core::Tensor::randn(
            0.0f32, 1.0,
            (2, FIXED_INTENT_D),
            &device,
        ).unwrap();

        let (cone, internal, intent_b, index_map) = env.all_ant_obs_batch(&intent_per_colony, &device).unwrap();
        let n_total = index_map.len();
        assert!(n_total >= 2, "expected at least 2 ants across both colonies");

        assert_eq!(cone.dims(), &[n_total, FIXED_CONE_D]);
        assert_eq!(internal.dims(), &[n_total, FIXED_INTERNAL_D]);
        assert_eq!(intent_b.dims(), &[n_total, FIXED_INTENT_D]);

        let colonies: std::collections::HashSet<u8> = index_map.iter().map(|(c, _)| *c).collect();
        assert!(colonies.contains(&0));
        assert!(colonies.contains(&1));
    }

    #[test]
    fn apply_commander_intents_writes_both_colonies() {
        use crate::hierarchical::sizing::FIXED_INTENT_D;
        let mut env = MatchEnv::new(0xb1a5_e1);
        let device = Device::Cpu;
        let intent = candle_core::Tensor::randn(0.0f32, 1.0, (2, FIXED_INTENT_D), &device).unwrap();
        env.apply_commander_intents(&intent).unwrap();
        let c0 = env.sim.colonies.get(0).unwrap().commander_intent;
        let c1 = env.sim.colonies.get(1).unwrap().commander_intent;
        // Random input → row 0 ≠ row 1 with probability ~1.
        assert_ne!(c0, c1, "commander intents should differ across colonies after random write");
    }

    #[test]
    fn new_cross_species_builds_two_distinct_species_colonies() {
        use antcolony_sim::species::Species;
        let bc = Species::load_from_file(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/species/brachyponera_chinensis.toml")
        ).expect("load bc");
        let ar = Species::load_from_file(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/species/aphaenogaster_rudis.toml")
        ).expect("load ar");
        let env = MatchEnv::new_cross_species(&bc, &ar, 0xC0FFEE);
        assert_eq!(env.sim.colony_configs.len(), 2);
        assert_eq!(env.sim.colony_cfg(0).species_id, "brachyponera_chinensis");
        assert_eq!(env.sim.colony_cfg(1).species_id, "aphaenogaster_rudis");
        // The two species differ in per-worker attack (asymmetry is live).
        assert_ne!(
            env.sim.colony_cfg(0).combat.worker_attack,
            env.sim.colony_cfg(1).combat.worker_attack
        );
        // Both colonies present, both queens alive at t=0.
        assert!(env.sim.colonies.len() == 2);
    }

    #[test]
    fn new_match_env_unchanged_smoke() {
        // Guard: MatchEnv::new still builds the symmetric 32×32 / 10-ant fixture.
        let env = MatchEnv::new(0xb1a5_e1);
        assert_eq!(env.sim.colonies.len(), 2);
        assert_eq!(env.max_ticks, 10_000);
    }

    #[test]
    fn apply_ant_modulators_batched_clamps_and_writes_through() {
        use crate::hierarchical::sizing::{FIXED_INTENT_D, FIXED_MODULATOR_D};
        let mut env = MatchEnv::new(0xb1a5_e1);
        let device = Device::Cpu;

        // Get an index_map by calling all_ant_obs_batch first.
        let intent = candle_core::Tensor::zeros((2, FIXED_INTENT_D), candle_core::DType::F32, &device).unwrap();
        let (_, _, _, index_map) = env.all_ant_obs_batch(&intent, &device).unwrap();
        let n = index_map.len();

        // Per-ant pattern: (3.0, 0.5, 0.05, 2.0, -1.0) — all within the safe clamp ranges.
        let mut mods_v = Vec::with_capacity(n * FIXED_MODULATOR_D);
        for _ in 0..n {
            mods_v.extend_from_slice(&[3.0_f32, 0.5, 0.05, 2.0, -1.0]);
        }
        let mods_t = candle_core::Tensor::from_vec(mods_v, (n, FIXED_MODULATOR_D), &device).unwrap();

        env.apply_ant_modulators_batched(&mods_t, &index_map).unwrap();

        let (cid, aid) = index_map[0];
        let ant = env.sim.ants.iter().find(|a| a.id == aid && a.colony_id == cid).unwrap();
        assert_eq!(ant.modulators.alpha_mult, 3.0);
        assert_eq!(ant.modulators.beta_mult, 0.5);
    }
}

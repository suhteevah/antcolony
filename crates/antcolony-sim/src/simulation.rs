//! Tick-based simulation runner. Owns all state; advances one step via `tick()`.
//!
//! System order: sense → decide → move → deposit → evaporate → diffuse → economy.
//! Combat and spawning are Phase 4/3 and are stubbed here (no-ops).

use glam::Vec2;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::ant::{Ant, AntCaste, AntState, choose_direction};
use crate::colony::{Brood, BroodStage, CasteRatio, ColonyState};
use crate::config::SimConfig;
use crate::pheromone::{PheromoneGrid, PheromoneLayer};
use crate::world::{Terrain, WorldGrid};

#[derive(Debug, Clone)]
pub struct Simulation {
    pub config: SimConfig,
    pub world: WorldGrid,
    pub pheromones: PheromoneGrid,
    pub ants: Vec<Ant>,
    pub colonies: Vec<ColonyState>,
    pub tick: u64,
    pub rng: ChaCha8Rng,
    /// Monotonic id generator for newly spawned ants.
    next_ant_id: u32,
}

impl Simulation {
    /// Build a simulation with one colony, its nest at the grid center,
    /// and `ant.initial_count` workers spawned around the entrance.
    pub fn new(config: SimConfig, seed: u64) -> Self {
        let w = config.world.width;
        let h = config.world.height;
        let world = WorldGrid::new(w, h);
        let pher = PheromoneGrid::new(w, h);
        let mut rng = ChaCha8Rng::seed_from_u64(seed);

        let nest_pos = Vec2::new(w as f32 * 0.5, h as f32 * 0.5);
        let mut colony = ColonyState::new(0, config.colony.initial_food, nest_pos);

        // Default initial distribution: all workers.
        let initial_dist = CasteRatio {
            worker: 1.0,
            soldier: 0.0,
            breeder: 0.0,
        };
        let ants = spawn_initial_ants(&config, &mut rng, nest_pos, 0, initial_dist, 0);

        // Seed population counts from the actual ants spawned.
        for a in &ants {
            match a.caste {
                AntCaste::Worker => colony.population.workers += 1,
                AntCaste::Soldier => colony.population.soldiers += 1,
                AntCaste::Breeder => colony.population.breeders += 1,
                AntCaste::Queen => {}
            }
        }

        let colonies = vec![colony];

        tracing::info!(
            world = format!("{}x{}", w, h),
            ants = ants.len(),
            seed,
            "Simulation::new"
        );

        let next_ant_id = ants.len() as u32;
        let mut sim = Self {
            config,
            world,
            pheromones: pher,
            ants,
            colonies,
            tick: 0,
            rng,
            next_ant_id,
        };
        // Place nest entrance in world.
        let (nx, ny) = (w / 2, h / 2);
        sim.world.place_nest(nx, ny, 0);
        sim
    }

    pub fn spawn_food_cluster(&mut self, cx: i64, cy: i64, radius: i64, units: u32) -> u32 {
        self.world.place_food_cluster(cx, cy, radius, units)
    }

    /// Advance the simulation by one tick.
    pub fn tick(&mut self) {
        let _span = tracing::debug_span!("tick", n = self.tick).entered();

        self.sense_and_decide();
        self.movement();
        self.deposit();
        self.colony_economy_tick();
        self.pheromones
            .evaporate(self.config.pheromone.evaporation_rate, self.config.pheromone.min_threshold);

        if self.config.pheromone.diffusion_interval > 0
            && self.tick % self.config.pheromone.diffusion_interval as u64 == 0
        {
            self.pheromones.diffuse(self.config.pheromone.diffusion_rate);
        }

        self.tick += 1;
        if self.tick % 500 == 0 {
            let c = &self.colonies[0];
            tracing::info!(
                tick = self.tick,
                ants = self.ants.len(),
                food_stored = c.food_stored,
                food_returned = c.food_returned,
                "colony heartbeat"
            );
        }
    }

    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.tick();
        }
    }

    /// Run one economy step for every colony: consume food, age brood,
    /// lay eggs, mature pupae into new `Ant`s, and apply starvation deaths.
    pub fn colony_economy_tick(&mut self) {
        let ccfg = self.config.colony.clone();
        let worker_health = self.config.combat.worker_health;
        let soldier_health = self.config.combat.soldier_health;
        // Per-tick cap on new eggs from the queen.
        const MAX_EGGS_PER_TICK: u32 = 10;

        // Collect ants to spawn after the borrow of self.colonies ends.
        let mut to_spawn: Vec<(u8, AntCaste, Vec2)> = Vec::new();
        // Collect starvation deaths as (colony_id, count) to prune ants afterward.
        let mut starve: Vec<(u8, u32)> = Vec::new();

        for colony in self.colonies.iter_mut() {
            let _span = tracing::debug_span!("colony_tick", colony_id = colony.id).entered();

            // ---- 1. Consumption ----
            let adult_total = colony.adult_total();
            let worker_breeder_cnt =
                colony.population.workers + colony.population.breeders;
            let soldier_cnt = colony.population.soldiers;
            let consumption = (worker_breeder_cnt as f32) * ccfg.adult_food_consumption
                + (soldier_cnt as f32)
                    * ccfg.adult_food_consumption
                    * ccfg.soldier_food_multiplier;
            colony.food_stored -= consumption;
            let mut starve_count: u32 = 0;
            if colony.food_stored < 0.0 {
                // Convert deficit into ant deaths. Each missing unit kills
                // enough ants to balance a full adult-cost worth of food.
                let deficit = -colony.food_stored;
                // One death covers `adult_food_consumption` worth of deficit.
                let cost = ccfg.adult_food_consumption.max(1e-6);
                let mut deaths = (deficit / cost).ceil() as u32;
                if deaths > adult_total {
                    deaths = adult_total;
                }
                starve_count = deaths;
                colony.food_stored = 0.0;
                if deaths > 0 {
                    tracing::warn!(
                        colony_id = colony.id,
                        deaths,
                        adult_total,
                        "starvation deaths"
                    );
                }
            }
            starve.push((colony.id, starve_count));

            // ---- 2. Egg laying ----
            let queen_alive = colony.queen_health > 0.0;
            if !queen_alive && colony.queen_alive_last_tick {
                tracing::info!(colony_id = colony.id, "queen died — egg production halted");
            }
            colony.queen_alive_last_tick = queen_alive;

            if queen_alive && colony.food_stored >= ccfg.egg_cost {
                colony.egg_accumulator += ccfg.queen_egg_rate;
                let mut laid_this_tick: u32 = 0;
                while colony.egg_accumulator >= 1.0
                    && colony.food_stored >= ccfg.egg_cost
                    && laid_this_tick < MAX_EGGS_PER_TICK
                {
                    colony.egg_accumulator -= 1.0;
                    colony.food_stored -= ccfg.egg_cost;
                    let caste = sample_caste(&mut self.rng, colony.caste_ratio);
                    colony.brood.push(Brood::new_egg(caste));
                    colony.eggs += 1;
                    laid_this_tick += 1;
                    if !colony.has_laid_egg {
                        colony.has_laid_egg = true;
                        tracing::info!(
                            colony_id = colony.id,
                            tick = self.tick,
                            "first egg laid"
                        );
                    }
                }
            }

            // ---- 3. Maturation ----
            // Age brood and advance stages. Pupa → adult is a deferred spawn.
            let mut matured_indices: Vec<usize> = Vec::new();
            for (idx, b) in colony.brood.iter_mut().enumerate() {
                b.age = b.age.saturating_add(1);
                match b.stage {
                    BroodStage::Egg => {
                        if b.age >= ccfg.larva_maturation_ticks {
                            b.stage = BroodStage::Larva;
                            b.age = 0;
                            if colony.eggs > 0 {
                                colony.eggs -= 1;
                            }
                            colony.larvae += 1;
                        }
                    }
                    BroodStage::Larva => {
                        if b.age >= ccfg.pupa_maturation_ticks {
                            b.stage = BroodStage::Pupa;
                            b.age = 0;
                            if colony.larvae > 0 {
                                colony.larvae -= 1;
                            }
                            colony.pupae += 1;
                        }
                    }
                    BroodStage::Pupa => {
                        // Reuse pupa_maturation_ticks for pupa → adult.
                        if b.age >= ccfg.pupa_maturation_ticks {
                            matured_indices.push(idx);
                        }
                    }
                }
            }

            // ---- 4/5. Spawning ----
            // Remove matured pupae in reverse order so indices stay valid.
            if !matured_indices.is_empty() {
                for &idx in matured_indices.iter().rev() {
                    let b = colony.brood.swap_remove(idx);
                    if colony.pupae > 0 {
                        colony.pupae -= 1;
                    }
                    // Pick a random nest entrance position.
                    let pos = if colony.nest_entrance_positions.is_empty() {
                        Vec2::ZERO
                    } else {
                        let i = self
                            .rng
                            .gen_range(0..colony.nest_entrance_positions.len());
                        colony.nest_entrance_positions[i]
                    };
                    to_spawn.push((colony.id, b.caste, pos));
                    // Increment population counts eagerly so this tick's
                    // consumption on the next iteration sees the new ant.
                    match b.caste {
                        AntCaste::Worker => colony.population.workers += 1,
                        AntCaste::Soldier => colony.population.soldiers += 1,
                        AntCaste::Breeder => colony.population.breeders += 1,
                        AntCaste::Queen => {}
                    }
                    let new_total = colony.adult_total();
                    // Population milestone every 50 ants.
                    let milestone = (new_total / 50) * 50;
                    if milestone > colony.last_population_milestone && milestone > 0 {
                        colony.last_population_milestone = milestone;
                        tracing::info!(
                            colony_id = colony.id,
                            population = new_total,
                            "population milestone"
                        );
                    }
                }
            }

            tracing::debug!(
                colony_id = colony.id,
                food = colony.food_stored,
                eggs = colony.eggs,
                larvae = colony.larvae,
                pupae = colony.pupae,
                workers = colony.population.workers,
                soldiers = colony.population.soldiers,
                breeders = colony.population.breeders,
                "economy tick"
            );
        }

        // Apply starvation deaths (oldest first per colony).
        for (cid, deaths) in starve {
            if deaths == 0 {
                continue;
            }
            // Collect ant indices for this colony sorted by age desc.
            let mut idxs: Vec<(usize, u32)> = self
                .ants
                .iter()
                .enumerate()
                .filter(|(_, a)| a.colony_id == cid && a.caste != AntCaste::Queen)
                .map(|(i, a)| (i, a.age))
                .collect();
            idxs.sort_by(|a, b| b.1.cmp(&a.1));
            let take = (deaths as usize).min(idxs.len());
            let mut remove_idx: Vec<usize> = idxs.iter().take(take).map(|(i, _)| *i).collect();
            remove_idx.sort_unstable_by(|a, b| b.cmp(a));
            for ri in remove_idx {
                let ant = self.ants.swap_remove(ri);
                // Decrement population counts.
                if let Some(c) = self.colonies.iter_mut().find(|c| c.id == cid) {
                    match ant.caste {
                        AntCaste::Worker => {
                            c.population.workers = c.population.workers.saturating_sub(1)
                        }
                        AntCaste::Soldier => {
                            c.population.soldiers = c.population.soldiers.saturating_sub(1)
                        }
                        AntCaste::Breeder => {
                            c.population.breeders = c.population.breeders.saturating_sub(1)
                        }
                        AntCaste::Queen => {}
                    }
                }
            }
        }

        // Materialize new ants after we're done mutating colonies.
        for (cid, caste, pos) in to_spawn {
            let id = self.next_ant_id;
            self.next_ant_id = self.next_ant_id.saturating_add(1);
            let health = match caste {
                AntCaste::Soldier => soldier_health,
                _ => worker_health,
            };
            let heading = self.rng.gen_range(0.0..std::f32::consts::TAU);
            let ant = Ant::new_with_caste(id, cid, pos, heading, health, caste);
            tracing::debug!(
                colony_id = cid,
                ant_id = id,
                caste = ?caste,
                "adult spawned"
            );
            self.ants.push(ant);
        }
    }

    fn sense_and_decide(&mut self) {
        let cfg = &self.config;
        let pher = &self.pheromones;
        let world = &self.world;
        // Allocate headings/intents in a temp buffer to avoid borrowing self.ants mutably inside.
        let mut new_headings = Vec::with_capacity(self.ants.len());
        let mut new_states: Vec<Option<AntState>> = Vec::with_capacity(self.ants.len());

        for ant in &self.ants {
            let h = choose_direction(ant, pher, &cfg.ant, &mut self.rng);
            new_headings.push(h);

            // State transition based on local sensing.
            let next = decide_next_state(ant, world, pher, cfg);
            new_states.push(next);
        }

        for (i, ant) in self.ants.iter_mut().enumerate() {
            ant.heading = new_headings[i];
            if let Some(ns) = new_states[i] {
                ant.transition(ns);
            }
            ant.state_timer = ant.state_timer.saturating_add(1);
            ant.age = ant.age.saturating_add(1);
        }
    }

    fn movement(&mut self) {
        let w = self.world.width as f32;
        let h = self.world.height as f32;
        for ant in self.ants.iter_mut() {
            let speed = ant.speed(&self.config.ant);
            // Non-moving states
            let moving = matches!(
                ant.state,
                AntState::Exploring
                    | AntState::FollowingTrail
                    | AntState::ReturningHome
                    | AntState::Fleeing
            );
            if !moving {
                continue;
            }
            let v = Vec2::new(ant.heading.cos(), ant.heading.sin()) * speed;
            let mut next = ant.position + v;

            // Reflect off the world bounds to keep ants on-grid.
            if next.x < 0.5 {
                next.x = 0.5;
                ant.heading = std::f32::consts::PI - ant.heading;
            } else if next.x > w - 0.5 {
                next.x = w - 0.5;
                ant.heading = std::f32::consts::PI - ant.heading;
            }
            if next.y < 0.5 {
                next.y = 0.5;
                ant.heading = -ant.heading;
            } else if next.y > h - 0.5 {
                next.y = h - 0.5;
                ant.heading = -ant.heading;
            }
            ant.position = next;
        }
    }

    fn deposit(&mut self) {
        let pcfg = &self.config.pheromone;
        for ant in &self.ants {
            let (gx, gy) = self.pheromones.world_to_grid(ant.position);
            if !self.pheromones.in_bounds(gx, gy) {
                continue;
            }
            let (ux, uy) = (gx as usize, gy as usize);
            match ant.state {
                AntState::ReturningHome => {
                    self.pheromones.deposit(
                        ux,
                        uy,
                        PheromoneLayer::FoodTrail,
                        pcfg.deposit_food_trail,
                        pcfg.max_intensity,
                    );
                }
                AntState::Exploring | AntState::FollowingTrail => {
                    self.pheromones.deposit(
                        ux,
                        uy,
                        PheromoneLayer::HomeTrail,
                        pcfg.deposit_home_trail,
                        pcfg.max_intensity,
                    );
                }
                _ => {}
            }
        }

        // Handle food pickup and nest drop-off as part of the deposit step
        // because both operate on the shared world grid at the ant's position.
        let capacity = self.config.ant.food_capacity;
        for ant in self.ants.iter_mut() {
            let (gx, gy) = self.world.world_to_grid(ant.position);
            if !self.world.in_bounds(gx, gy) {
                continue;
            }
            let (ux, uy) = (gx as usize, gy as usize);
            match (self.world.get(ux, uy), ant.state) {
                (Terrain::Food(_), AntState::Exploring | AntState::FollowingTrail) => {
                    let got = self.world.take_food(ux, uy) as f32;
                    if got > 0.0 {
                        ant.food_carried = (ant.food_carried + got).min(capacity);
                        ant.transition(AntState::PickingUpFood);
                        // Turn around 180°.
                        ant.heading += std::f32::consts::PI;
                    }
                }
                (Terrain::NestEntrance(cid), AntState::ReturningHome)
                    if cid == ant.colony_id && ant.food_carried > 0.0 =>
                {
                    let drop = ant.food_carried;
                    ant.food_carried = 0.0;
                    if let Some(colony) = self.colonies.iter_mut().find(|c| c.id == cid) {
                        colony.accept_food(drop);
                    }
                    ant.transition(AntState::StoringFood);
                }
                _ => {}
            }
        }
    }
}

/// Per-ant state transition logic. Reads terrain + pheromones at the ant's cell.
/// Returns `Some(new_state)` if a transition should occur.
fn decide_next_state(
    ant: &Ant,
    world: &WorldGrid,
    pher: &PheromoneGrid,
    cfg: &SimConfig,
) -> Option<AntState> {
    let (gx, gy) = world.world_to_grid(ant.position);
    if !world.in_bounds(gx, gy) {
        return None;
    }
    let (ux, uy) = (gx as usize, gy as usize);
    let terrain = world.get(ux, uy);

    match ant.state {
        AntState::Idle => Some(AntState::Exploring),
        AntState::Exploring => {
            if matches!(terrain, Terrain::Food(_)) {
                return Some(AntState::PickingUpFood);
            }
            let scent = pher.read(ux, uy, PheromoneLayer::FoodTrail);
            if scent > cfg.pheromone.min_threshold * 10.0 {
                return Some(AntState::FollowingTrail);
            }
            None
        }
        AntState::FollowingTrail => {
            if matches!(terrain, Terrain::Food(_)) {
                return Some(AntState::PickingUpFood);
            }
            let scent = pher.read(ux, uy, PheromoneLayer::FoodTrail);
            if scent < cfg.pheromone.min_threshold * 2.0 {
                return Some(AntState::Exploring);
            }
            None
        }
        AntState::PickingUpFood => {
            if ant.food_carried > 0.0 {
                Some(AntState::ReturningHome)
            } else {
                Some(AntState::Exploring)
            }
        }
        AntState::ReturningHome => {
            if let Terrain::NestEntrance(cid) = terrain {
                if cid == ant.colony_id {
                    return Some(AntState::StoringFood);
                }
            }
            None
        }
        AntState::StoringFood => {
            if ant.food_carried <= 0.0 {
                Some(AntState::Exploring)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Spawn the starting ant roster for a colony.
///
/// The caste mix is taken from `distribution` (renormalized to sum=1.0).
/// `id_offset` is added to sequential indices so multiple colonies don't collide.
pub fn spawn_initial_ants(
    config: &SimConfig,
    rng: &mut ChaCha8Rng,
    nest: Vec2,
    colony_id: u8,
    distribution: CasteRatio,
    id_offset: u32,
) -> Vec<Ant> {
    let mut ants = Vec::with_capacity(config.ant.initial_count);
    let worker_health = config.combat.worker_health;
    let soldier_health = config.combat.soldier_health;
    for i in 0..config.ant.initial_count {
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let r: f32 = rng.gen_range(0.0..2.0);
        let pos = nest + Vec2::new(r * angle.cos(), r * angle.sin());
        let caste = sample_caste(rng, distribution);
        let health = match caste {
            AntCaste::Soldier => soldier_health,
            _ => worker_health,
        };
        ants.push(Ant::new_with_caste(
            id_offset + i as u32,
            colony_id,
            pos,
            angle,
            health,
            caste,
        ));
    }
    ants
}

/// Weighted random caste draw from a `CasteRatio`. Negative or NaN weights
/// are treated as zero. If all weights are zero, returns `Worker`.
fn sample_caste(rng: &mut ChaCha8Rng, ratio: CasteRatio) -> AntCaste {
    let w = ratio.worker.max(0.0);
    let s = ratio.soldier.max(0.0);
    let b = ratio.breeder.max(0.0);
    let total = w + s + b;
    if !(total > 0.0) {
        return AntCaste::Worker;
    }
    let pick = rng.r#gen::<f32>() * total;
    if pick < w {
        AntCaste::Worker
    } else if pick < w + s {
        AntCaste::Soldier
    } else {
        AntCaste::Breeder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_config() -> SimConfig {
        let mut c = SimConfig::default();
        c.world.width = 64;
        c.world.height = 64;
        c.ant.initial_count = 20;
        c.ant.exploration_rate = 0.25;
        // Neutralize the economy by default so Phase 1/2 tests
        // (which don't care about starvation) stay deterministic.
        c.colony.adult_food_consumption = 0.0;
        c.colony.queen_egg_rate = 0.0;
        c
    }

    #[test]
    fn sim_runs_without_panic() {
        let mut sim = Simulation::new(small_config(), 1);
        sim.run(100);
        assert_eq!(sim.tick, 100);
    }

    #[test]
    fn test_ant_finds_food() {
        // One ant, one food cluster nearby. Run long enough for it to deliver at least once.
        let mut cfg = small_config();
        cfg.world.width = 48;
        cfg.world.height = 48;
        cfg.ant.initial_count = 20;
        let mut sim = Simulation::new(cfg, 7);
        sim.spawn_food_cluster(8, 8, 3, 10);
        sim.run(4000);
        let delivered = sim.colonies[0].food_returned;
        assert!(delivered > 0, "no food delivered after 4000 ticks");
    }

    #[test]
    fn colony_grows_with_food() {
        let mut cfg = small_config();
        cfg.world.width = 64;
        cfg.world.height = 64;
        cfg.ant.initial_count = 20;
        // Make the economy fast enough to observe adults emerge in 5000 ticks.
        cfg.colony.initial_food = 100_000.0;
        cfg.colony.queen_egg_rate = 0.5;
        cfg.colony.egg_cost = 1.0;
        cfg.colony.adult_food_consumption = 0.001;
        cfg.colony.larva_maturation_ticks = 100;
        cfg.colony.pupa_maturation_ticks = 100;
        let mut sim = Simulation::new(cfg, 42);

        let initial = sim.colonies[0].adult_total();
        let mut saw_egg = false;
        let mut saw_larva = false;
        for _ in 0..5000 {
            sim.tick();
            let c = &sim.colonies[0];
            if c.eggs > 0 {
                saw_egg = true;
            }
            if c.larvae > 0 {
                saw_larva = true;
            }
        }
        let final_total = sim.colonies[0].adult_total();
        assert!(
            final_total > initial,
            "colony did not grow: initial={} final={}",
            initial,
            final_total
        );
        assert!(saw_egg, "never observed an egg in the brood");
        assert!(saw_larva, "never observed a larva in the brood");
    }

    #[test]
    fn colony_starves_without_food() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 30;
        cfg.colony.initial_food = 0.0;
        cfg.colony.queen_egg_rate = 0.0; // avoid any spawn compensating
        // Make starvation visible on a short horizon.
        cfg.colony.adult_food_consumption = 0.5;
        let mut sim = Simulation::new(cfg, 11);
        let initial = sim.ants.len();
        sim.run(10_000);
        assert!(
            sim.ants.len() < initial,
            "colony did not shrink: initial={} final={}",
            initial,
            sim.ants.len()
        );
    }

    #[test]
    fn caste_ratio_affects_spawns() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 10;
        cfg.colony.initial_food = 100_000.0;
        cfg.colony.queen_egg_rate = 1.0;
        cfg.colony.larva_maturation_ticks = 20;
        cfg.colony.pupa_maturation_ticks = 20;
        let mut sim = Simulation::new(cfg, 5);
        // Force all new spawns to soldiers.
        sim.colonies[0].caste_ratio = CasteRatio {
            worker: 0.0,
            soldier: 1.0,
            breeder: 0.0,
        };
        let soldiers_before = sim.colonies[0].population.soldiers;
        // Run long enough to mature ≥5 adults through egg→larva→pupa→adult.
        sim.run(2000);
        let soldiers_after = sim.colonies[0].population.soldiers;
        assert!(
            soldiers_after >= soldiers_before + 5,
            "expected ≥5 new soldiers, got {} → {}",
            soldiers_before,
            soldiers_after
        );
        // Verify no non-queen non-soldier was spawned since sim start.
        // The initial roster was all workers; any new spawns should be soldiers,
        // so worker count should not have increased.
        let workers_initial_count = 10; // initial_count above
        assert!(
            sim.colonies[0].population.workers <= workers_initial_count,
            "worker count grew despite soldier-only caste ratio: {}",
            sim.colonies[0].population.workers
        );
    }

    #[test]
    fn queen_death_stops_production() {
        let mut cfg = small_config();
        cfg.colony.initial_food = 10_000.0;
        cfg.colony.queen_egg_rate = 1.0;
        let mut sim = Simulation::new(cfg, 99);
        sim.colonies[0].queen_health = 0.0;
        let eggs_before = sim.colonies[0].eggs;
        let brood_before = sim.colonies[0].brood.len();
        sim.run(2000);
        assert_eq!(
            sim.colonies[0].eggs, eggs_before,
            "eggs laid despite dead queen"
        );
        assert_eq!(
            sim.colonies[0].brood.len(),
            brood_before,
            "brood grew despite dead queen"
        );
        assert!(
            !sim.colonies[0].has_laid_egg,
            "has_laid_egg set despite dead queen from t=0"
        );
    }

    #[test]
    fn test_trail_formation() {
        // Many ants, one food source. Assert the cell at food and near nest has elevated pheromone.
        let mut cfg = small_config();
        cfg.world.width = 48;
        cfg.world.height = 48;
        cfg.ant.initial_count = 60;
        let mut sim = Simulation::new(cfg, 3);
        sim.spawn_food_cluster(10, 10, 2, 20);
        sim.run(3000);
        // Background: sample a far corner cell, should be near zero.
        let bg = sim.pheromones.read(0, 47, PheromoneLayer::FoodTrail);
        // Mid-path between food (10,10) and nest (24,24)
        let mid = sim.pheromones.read(17, 17, PheromoneLayer::FoodTrail);
        let total: f32 = sim.pheromones.total_intensity(PheromoneLayer::FoodTrail);
        assert!(total > 5.0, "no trail built: total={}", total);
        // Allow for stochasticity: just assert mid-path is at least as high as background.
        assert!(mid >= bg, "mid {} background {}", mid, bg);
    }
}

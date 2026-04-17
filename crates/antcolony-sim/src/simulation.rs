//! Tick-based simulation runner. Owns all state; advances one step via `tick()`.
//!
//! System order: sense → decide → move (incl. tube transits) → deposit →
//! economy → evaporate → diffuse → port-bleed.
//!
//! K2: the sim now owns a `Topology` (one or more `Module`s linked by
//! `Tube`s). Each module has its own `WorldGrid` and `PheromoneGrid`.
//! Ants carry `module_id`; when they walk onto a port cell, they enter
//! the attached tube and emerge on the far side a few ticks later.

use glam::Vec2;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::ant::{Ant, AntCaste, AntState, choose_direction};
use crate::colony::{Brood, BroodStage, CasteRatio, ColonyState};
use crate::config::SimConfig;
use crate::module::{ModuleId, ModuleKind, PortPos};
use crate::pheromone::{PheromoneGrid, PheromoneLayer};
use crate::topology::Topology;
use crate::tube::{TubeId, TubeTransit};
use crate::world::{Terrain, WorldGrid};

/// Fraction of per-layer pheromone that equilibrates across a tube each tick.
/// 0.0 = isolated modules (no scent leaks). 1.0 = instant average.
const PORT_BLEED_RATE: f32 = 0.35;

#[derive(Debug, Clone)]
pub struct Simulation {
    pub config: SimConfig,
    pub topology: Topology,
    pub ants: Vec<Ant>,
    pub colonies: Vec<ColonyState>,
    pub tick: u64,
    pub rng: ChaCha8Rng,
    /// Monotonic id generator for newly spawned ants.
    next_ant_id: u32,
}

impl Simulation {
    /// Build a simulation with one single-module topology (pre-K2 layout)
    /// sized from `config.world`. The nest entrance is placed at grid center.
    /// Callers that want a multi-module formicarium should use
    /// [`Simulation::new_with_topology`].
    pub fn new(config: SimConfig, seed: u64) -> Self {
        let topology = Topology::single(ModuleKind::Outworld, config.world.width, config.world.height);
        Self::new_with_topology(config, topology, seed)
    }

    /// Build a simulation with an arbitrary topology. The nest entrance
    /// defaults to module 0's center. Initial ants spawn on module 0.
    pub fn new_with_topology(config: SimConfig, mut topology: Topology, seed: u64) -> Self {
        assert!(
            !topology.is_empty(),
            "Simulation requires at least one module"
        );
        let primary = topology.module(0);
        let pw = primary.width();
        let ph = primary.height();
        let mut rng = ChaCha8Rng::seed_from_u64(seed);

        let nest_pos = Vec2::new(pw as f32 * 0.5, ph as f32 * 0.5);
        let mut colony = ColonyState::new(0, config.colony.initial_food, nest_pos);

        let initial_dist = CasteRatio {
            worker: 1.0,
            soldier: 0.0,
            breeder: 0.0,
        };
        let mut ants = spawn_initial_ants(&config, &mut rng, nest_pos, 0, initial_dist, 0);
        for a in ants.iter_mut() {
            a.module_id = 0;
        }

        for a in &ants {
            match a.caste {
                AntCaste::Worker => colony.population.workers += 1,
                AntCaste::Soldier => colony.population.soldiers += 1,
                AntCaste::Breeder => colony.population.breeders += 1,
                AntCaste::Queen => {}
            }
        }

        tracing::info!(
            modules = topology.modules.len(),
            tubes = topology.tubes.len(),
            ants = ants.len(),
            seed,
            "Simulation::new_with_topology"
        );

        // Place nest entrance on module 0.
        topology
            .module_mut(0)
            .world
            .place_nest(pw / 2, ph / 2, 0);

        let next_ant_id = ants.len() as u32;
        Self {
            config,
            topology,
            ants,
            colonies: vec![colony],
            tick: 0,
            rng,
            next_ant_id,
        }
    }

    // ---- Convenience accessors for pre-K2 callers ----

    /// Module-0 world (the "primary" habitat). Most single-module code
    /// should use this; multi-module code walks `self.topology.modules`.
    #[inline]
    pub fn world(&self) -> &WorldGrid {
        &self.topology.modules[0].world
    }

    #[inline]
    pub fn world_mut(&mut self) -> &mut WorldGrid {
        &mut self.topology.modules[0].world
    }

    #[inline]
    pub fn pheromones(&self) -> &PheromoneGrid {
        &self.topology.modules[0].pheromones
    }

    #[inline]
    pub fn pheromones_mut(&mut self) -> &mut PheromoneGrid {
        &mut self.topology.modules[0].pheromones
    }

    pub fn spawn_food_cluster(&mut self, cx: i64, cy: i64, radius: i64, units: u32) -> u32 {
        self.topology.module_mut(0).world.place_food_cluster(cx, cy, radius, units)
    }

    /// Like `spawn_food_cluster` but on a specific module.
    pub fn spawn_food_cluster_on(
        &mut self,
        module: ModuleId,
        cx: i64,
        cy: i64,
        radius: i64,
        units: u32,
    ) -> u32 {
        self.topology.module_mut(module).world.place_food_cluster(cx, cy, radius, units)
    }

    /// Advance the simulation by one tick.
    pub fn tick(&mut self) {
        let _span = tracing::debug_span!("tick", n = self.tick).entered();

        self.sense_and_decide();
        self.movement();
        self.deposit_and_interact();
        self.feeding_dish_tick();
        self.colony_economy_tick();

        let evap_rate = self.config.pheromone.evaporation_rate;
        let threshold = self.config.pheromone.min_threshold;
        for m in self.topology.modules.iter_mut() {
            m.pheromones.evaporate(evap_rate, threshold);
        }

        if self.config.pheromone.diffusion_interval > 0
            && self.tick % self.config.pheromone.diffusion_interval as u64 == 0
        {
            let diff_rate = self.config.pheromone.diffusion_rate;
            for m in self.topology.modules.iter_mut() {
                m.pheromones.diffuse(diff_rate);
            }
        }

        self.port_bleed();

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

    // ---- Per-tick systems ----

    fn sense_and_decide(&mut self) {
        let cfg = &self.config;
        let topology = &self.topology;
        let mut new_headings = Vec::with_capacity(self.ants.len());
        let mut new_states: Vec<Option<AntState>> = Vec::with_capacity(self.ants.len());

        for ant in &self.ants {
            if ant.is_in_transit() {
                new_headings.push(ant.heading);
                new_states.push(None);
                continue;
            }
            let module = topology.module(ant.module_id);
            let h = choose_direction(ant, &module.pheromones, &cfg.ant, &mut self.rng);
            new_headings.push(h);
            let next = decide_next_state(ant, &module.world, &module.pheromones, cfg);
            new_states.push(next);
        }

        for (i, ant) in self.ants.iter_mut().enumerate() {
            if !ant.is_in_transit() {
                ant.heading = new_headings[i];
                if let Some(ns) = new_states[i] {
                    ant.transition(ns);
                }
            }
            ant.state_timer = ant.state_timer.saturating_add(1);
            ant.age = ant.age.saturating_add(1);
        }
    }

    fn movement(&mut self) {
        let speed_cfg = &self.config.ant;
        let topology = &self.topology;

        // First pass: advance in-tube ants, collect emergences.
        let mut emerge: Vec<(usize, ModuleId, Vec2, f32)> = Vec::new();
        for (i, ant) in self.ants.iter_mut().enumerate() {
            let Some(transit) = ant.transit else {
                continue;
            };
            let tube = topology.tube(transit.tube);
            let speed = ant.speed(speed_cfg).max(0.1);
            let dprog = speed / tube.length_ticks.max(1) as f32;
            let new_progress = if transit.going_forward {
                transit.progress + dprog
            } else {
                transit.progress - dprog
            };
            if (transit.going_forward && new_progress >= 1.0)
                || (!transit.going_forward && new_progress <= 0.0)
            {
                // Emerge.
                let (exit_mod_id, exit_port) = topology.tube_exit(transit.tube, transit.going_forward);
                let exit_module = topology.module(exit_mod_id);
                let emerge_pos = exit_port.to_vec2();
                let emerge_heading = exit_module.port_interior_heading(exit_port);
                emerge.push((i, exit_mod_id, emerge_pos, emerge_heading));
            } else {
                ant.transit = Some(TubeTransit {
                    progress: new_progress.clamp(0.0, 1.0),
                    ..transit
                });
            }
        }
        for (i, mid, pos, heading) in emerge {
            let ant = &mut self.ants[i];
            ant.transit = None;
            ant.module_id = mid;
            ant.position = pos;
            ant.heading = heading;
            tracing::trace!(ant = ant.id, module = mid, "tube emergence");
        }

        // Second pass: normal per-module on-grid movement. At port cells
        // that have a tube attached, enter the tube instead of reflecting.
        let mut enter_tube: Vec<(usize, TubeId, bool)> = Vec::new();
        for (i, ant) in self.ants.iter_mut().enumerate() {
            if ant.is_in_transit() {
                continue;
            }
            let module = topology.module(ant.module_id);
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
            let speed = ant.speed(speed_cfg);
            let v = Vec2::new(ant.heading.cos(), ant.heading.sin()) * speed;
            let mut next = ant.position + v;

            let w = module.width() as f32;
            let h = module.height() as f32;

            // Before reflecting, check if the ant is about to exit through a port.
            // We look for a port cell within one tile of the next position.
            let (tx, ty) = (next.x.floor() as i64, next.y.floor() as i64);
            let mut entered_tube = false;
            for port in &module.ports {
                let (px, py) = (port.x as i64, port.y as i64);
                // Detect port entry when the ant is within half a cell of the port
                // AND moving in the direction the port faces.
                if (tx - px).abs() <= 1 && (ty - py).abs() <= 1 {
                    if let Some((tid, going_forward)) =
                        topology.tube_at_port(ant.module_id, *port)
                    {
                        // K2.2 bore-width gate: ants that are too big can't fit.
                        let tube = topology.tube(tid);
                        let size = ant.body_size_mm(&self.config.ant);
                        if size > tube.bore_width_mm {
                            // Reflect as if the port were a closed wall.
                            tracing::trace!(
                                ant = ant.id,
                                caste = ?ant.caste,
                                size_mm = size,
                                bore_mm = tube.bore_width_mm,
                                tube = tid,
                                "tube refused ant (too large)"
                            );
                            ant.heading += std::f32::consts::PI;
                            entered_tube = true; // reuse flag to skip bounds reflect + movement
                            break;
                        }
                        enter_tube.push((i, tid, going_forward));
                        entered_tube = true;
                        break;
                    }
                }
            }
            if entered_tube {
                continue;
            }

            // Bounds reflection — only if we did not enter a tube.
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

        for (i, tid, forward) in enter_tube {
            let ant = &mut self.ants[i];
            ant.transit = Some(TubeTransit::new(tid, forward));
            tracing::trace!(ant = ant.id, tube = tid, forward, "entered tube");
        }
    }

    /// Deposit pheromones at each ant's current module+cell, handle food
    /// pickup, and deliver food at nest entrances.
    fn deposit_and_interact(&mut self) {
        let pcfg = self.config.pheromone.clone();
        let capacity = self.config.ant.food_capacity;

        // 1) Pheromone deposits. Iterate ants, group by module.
        struct Deposit {
            module: ModuleId,
            x: usize,
            y: usize,
            layer: PheromoneLayer,
            amount: f32,
        }
        let mut deposits: Vec<Deposit> = Vec::new();
        for ant in &self.ants {
            if ant.is_in_transit() {
                continue;
            }
            let module = self.topology.module(ant.module_id);
            let (gx, gy) = module.pheromones.world_to_grid(ant.position);
            if !module.pheromones.in_bounds(gx, gy) {
                continue;
            }
            let (ux, uy) = (gx as usize, gy as usize);
            let layered = match ant.state {
                AntState::ReturningHome => Some((PheromoneLayer::FoodTrail, pcfg.deposit_food_trail)),
                AntState::Exploring | AntState::FollowingTrail => {
                    Some((PheromoneLayer::HomeTrail, pcfg.deposit_home_trail))
                }
                _ => None,
            };
            if let Some((layer, amount)) = layered {
                deposits.push(Deposit {
                    module: ant.module_id,
                    x: ux,
                    y: uy,
                    layer,
                    amount,
                });
            }
        }
        for d in deposits {
            self.topology
                .module_mut(d.module)
                .pheromones
                .deposit(d.x, d.y, d.layer, d.amount, pcfg.max_intensity);
        }

        // 2) Food pickup + nest drop-off. Iterate ants mutably.
        for ant in self.ants.iter_mut() {
            if ant.is_in_transit() {
                continue;
            }
            let module = self.topology.module_mut(ant.module_id);
            let (gx, gy) = module.world.world_to_grid(ant.position);
            if !module.world.in_bounds(gx, gy) {
                continue;
            }
            let (ux, uy) = (gx as usize, gy as usize);
            match (module.world.get(ux, uy), ant.state) {
                (Terrain::Food(_), AntState::Exploring | AntState::FollowingTrail) => {
                    let got = module.world.take_food(ux, uy) as f32;
                    if got > 0.0 {
                        ant.food_carried = (ant.food_carried + got).min(capacity);
                        ant.transition(AntState::PickingUpFood);
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

    /// Propagate pheromone across tube boundaries: port cells on either
    /// end of a tube equilibrate a fraction of their layer intensities
    /// each tick, so a food trail laid in the outworld bleeds into the
    /// nest via the connecting port (and vice versa).
    fn port_bleed(&mut self) {
        let rate = PORT_BLEED_RATE;
        // Snapshot tube endpoints to avoid borrowing topology while mutating it.
        let ends: Vec<(ModuleId, PortPos, ModuleId, PortPos)> = self
            .topology
            .tubes
            .iter()
            .map(|t| (t.from.module, t.from.port, t.to.module, t.to.port))
            .collect();
        for (ma, pa, mb, pb) in ends {
            for layer in [
                PheromoneLayer::FoodTrail,
                PheromoneLayer::HomeTrail,
                PheromoneLayer::Alarm,
            ] {
                let (ax, ay) = pa.as_usize();
                let (bx, by) = pb.as_usize();
                let a = self.topology.module(ma).pheromones.read(ax, ay, layer);
                let b = self.topology.module(mb).pheromones.read(bx, by, layer);
                if (a - b).abs() < 1e-6 {
                    continue;
                }
                let mix = (a + b) * 0.5;
                let new_a = a + (mix - a) * rate;
                let new_b = b + (mix - b) * rate;
                self.topology.module_mut(ma).pheromones.set_cell(ax, ay, layer, new_a);
                self.topology.module_mut(mb).pheromones.set_cell(bx, by, layer, new_b);
            }
        }
    }

    /// FeedingDish auto-refill: any module of kind `FeedingDish` whose
    /// food has dropped below the threshold regrows a small cluster at
    /// its center after a cooldown. Keeps keeper-mode colonies from
    /// fully starving if they exhaust the outworld food.
    pub fn feeding_dish_tick(&mut self) {
        const REFILL_THRESHOLD: u32 = 5;
        const REFILL_RADIUS: i64 = 2;
        const REFILL_UNITS: u32 = 8;
        const REFILL_COOLDOWN: u32 = 600;

        for mid in 0..self.topology.modules.len() {
            let module = &mut self.topology.modules[mid];
            if module.kind != ModuleKind::FeedingDish {
                continue;
            }
            // Decrement cooldown.
            if module.tick_cooldown > 0 {
                module.tick_cooldown -= 1;
                continue;
            }
            // Count total food units in terrain.
            let mut total: u32 = 0;
            for cell in module.world.cells.iter() {
                if let Terrain::Food(n) = cell {
                    total = total.saturating_add(*n);
                }
            }
            if total >= REFILL_THRESHOLD {
                continue;
            }
            // Refill at the module center.
            let cx = (module.width() / 2) as i64;
            let cy = (module.height() / 2) as i64;
            let placed = module
                .world
                .place_food_cluster(cx, cy, REFILL_RADIUS, REFILL_UNITS);
            module.tick_cooldown = REFILL_COOLDOWN;
            tracing::info!(
                module = mid,
                tick = self.tick,
                placed,
                total_before = total,
                "FeedingDish refilled"
            );
        }
    }

    /// Run one economy step for every colony: consume food, age brood,
    /// lay eggs, mature pupae into new `Ant`s, apply starvation deaths.
    pub fn colony_economy_tick(&mut self) {
        let ccfg = self.config.colony.clone();
        let worker_health = self.config.combat.worker_health;
        let soldier_health = self.config.combat.soldier_health;
        const MAX_EGGS_PER_TICK: u32 = 10;

        let mut to_spawn: Vec<(u8, AntCaste, Vec2)> = Vec::new();
        let mut starve: Vec<(u8, u32)> = Vec::new();

        for colony in self.colonies.iter_mut() {
            let _span = tracing::debug_span!("colony_tick", colony_id = colony.id).entered();

            let adult_total = colony.adult_total();
            let worker_breeder_cnt = colony.population.workers + colony.population.breeders;
            let soldier_cnt = colony.population.soldiers;
            let consumption = (worker_breeder_cnt as f32) * ccfg.adult_food_consumption
                + (soldier_cnt as f32)
                    * ccfg.adult_food_consumption
                    * ccfg.soldier_food_multiplier;
            colony.food_stored -= consumption;
            let mut starve_count: u32 = 0;
            if colony.food_stored < 0.0 {
                let deficit = -colony.food_stored;
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
                        if b.age >= ccfg.pupa_maturation_ticks {
                            matured_indices.push(idx);
                        }
                    }
                }
            }

            if !matured_indices.is_empty() {
                for &idx in matured_indices.iter().rev() {
                    let b = colony.brood.swap_remove(idx);
                    if colony.pupae > 0 {
                        colony.pupae -= 1;
                    }
                    let pos = if colony.nest_entrance_positions.is_empty() {
                        Vec2::ZERO
                    } else {
                        let i = self
                            .rng
                            .gen_range(0..colony.nest_entrance_positions.len());
                        colony.nest_entrance_positions[i]
                    };
                    to_spawn.push((colony.id, b.caste, pos));
                    match b.caste {
                        AntCaste::Worker => colony.population.workers += 1,
                        AntCaste::Soldier => colony.population.soldiers += 1,
                        AntCaste::Breeder => colony.population.breeders += 1,
                        AntCaste::Queen => {}
                    }
                    let new_total = colony.adult_total();
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

        for (cid, caste, pos) in to_spawn {
            let id = self.next_ant_id;
            self.next_ant_id = self.next_ant_id.saturating_add(1);
            let health = match caste {
                AntCaste::Soldier => soldier_health,
                _ => worker_health,
            };
            let heading = self.rng.gen_range(0.0..std::f32::consts::TAU);
            let mut ant = Ant::new_with_caste(id, cid, pos, heading, health, caste);
            ant.module_id = 0;
            tracing::debug!(
                colony_id = cid,
                ant_id = id,
                caste = ?caste,
                "adult spawned"
            );
            self.ants.push(ant);
        }
    }
}

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
    fn test_trail_formation() {
        let mut cfg = small_config();
        cfg.world.width = 48;
        cfg.world.height = 48;
        cfg.ant.initial_count = 60;
        let mut sim = Simulation::new(cfg, 3);
        sim.spawn_food_cluster(10, 10, 2, 20);
        sim.run(3000);
        let bg = sim.pheromones().read(0, 47, PheromoneLayer::FoodTrail);
        let mid = sim.pheromones().read(17, 17, PheromoneLayer::FoodTrail);
        let total: f32 = sim.pheromones().total_intensity(PheromoneLayer::FoodTrail);
        assert!(total > 5.0, "no trail built: total={}", total);
        assert!(mid >= bg, "mid {} background {}", mid, bg);
    }

    #[test]
    fn colony_grows_with_food() {
        let mut cfg = small_config();
        cfg.world.width = 64;
        cfg.world.height = 64;
        cfg.ant.initial_count = 20;
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
        cfg.colony.queen_egg_rate = 0.0;
        cfg.colony.adult_food_consumption = 0.5;
        let mut sim = Simulation::new(cfg, 11);
        let initial = sim.ants.len();
        sim.run(200);
        assert!(
            sim.ants.len() < initial,
            "ants did not die of starvation: initial={}, final={}",
            initial,
            sim.ants.len()
        );
    }

    #[test]
    fn caste_ratio_affects_spawns() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 5;
        cfg.colony.initial_food = 100_000.0;
        cfg.colony.queen_egg_rate = 1.0;
        cfg.colony.egg_cost = 1.0;
        cfg.colony.adult_food_consumption = 0.0;
        cfg.colony.larva_maturation_ticks = 10;
        cfg.colony.pupa_maturation_ticks = 10;
        let mut sim = Simulation::new(cfg, 99);
        sim.colonies[0].caste_ratio = CasteRatio {
            worker: 0.0,
            soldier: 1.0,
            breeder: 0.0,
        };
        let initial_ants = sim.ants.len();
        sim.run(1000);
        let new_ants = &sim.ants[initial_ants..];
        assert!(new_ants.len() >= 5, "not enough new adults: {}", new_ants.len());
        for a in new_ants {
            assert_eq!(
                a.caste,
                AntCaste::Soldier,
                "expected all soldiers in new spawns"
            );
        }
    }

    #[test]
    fn queen_death_stops_production() {
        let mut cfg = small_config();
        cfg.colony.initial_food = 100_000.0;
        cfg.colony.queen_egg_rate = 1.0;
        cfg.colony.egg_cost = 1.0;
        cfg.colony.adult_food_consumption = 0.0;
        let mut sim = Simulation::new(cfg, 77);
        sim.colonies[0].queen_health = 0.0;
        let eggs_before = sim.colonies[0].eggs;
        sim.run(2000);
        assert_eq!(
            sim.colonies[0].eggs, eggs_before,
            "queen produced eggs while dead"
        );
    }

    // ---- K2 multi-module tests ----

    #[test]
    fn starter_formicarium_constructs_two_modules() {
        let cfg = small_config();
        let topology = Topology::starter_formicarium((32, 24), (64, 64));
        let sim = Simulation::new_with_topology(cfg, topology, 1);
        assert_eq!(sim.topology.modules.len(), 2);
        assert_eq!(sim.topology.tubes.len(), 1);
        assert_eq!(sim.ants.len(), 20);
        for a in &sim.ants {
            assert_eq!(a.module_id, 0, "initial ants spawn on module 0");
        }
    }

    #[test]
    fn ant_traverses_tube_between_modules() {
        // Construct a 2-module topology and manually place one ant at the
        // nest-side port, walking eastward (into the tube). After enough
        // ticks it must arrive on module 1.
        let mut cfg = small_config();
        cfg.ant.exploration_rate = 0.0;
        let topology = Topology::starter_formicarium((32, 24), (64, 64));
        let mut sim = Simulation::new_with_topology(cfg, topology, 5);
        sim.ants.clear();
        let nest_port = sim.topology.module(0).ports[0];
        let mut probe = Ant::new_worker(
            1000,
            0,
            Vec2::new(nest_port.x as f32 - 0.5, nest_port.y as f32 + 0.5),
            0.0, // heading east
            10.0,
        );
        probe.state = AntState::Exploring;
        probe.module_id = 0;
        sim.ants.push(probe);

        // Enough ticks to cross a 30-tick tube plus some approach margin.
        sim.run(80);
        let ant = &sim.ants[0];
        assert!(
            !ant.is_in_transit(),
            "ant still in transit after generous budget: {:?}",
            ant.transit
        );
        assert_eq!(ant.module_id, 1, "ant did not emerge on module 1");
    }

    #[test]
    fn major_blocked_by_narrow_tube() {
        // Tube bore = 4mm; soldier on a polymorphic species with
        // worker_size_mm=4 has body size = 4*1.6 = 6.4mm > 4 → refused.
        let mut cfg = small_config();
        cfg.ant.exploration_rate = 0.0;
        cfg.ant.worker_size_mm = 4.0;
        cfg.ant.polymorphic = true;

        let mut topology = Topology::starter_formicarium((32, 24), (64, 64));
        // Narrow the only tube.
        topology.tubes[0].bore_width_mm = 4.0;

        let mut sim = Simulation::new_with_topology(cfg, topology, 11);
        sim.ants.clear();
        let nest_port = sim.topology.module(0).ports[0];
        let mut probe = Ant::new_with_caste(
            2001,
            0,
            Vec2::new(nest_port.x as f32 - 0.5, nest_port.y as f32 + 0.5),
            0.0, // east, into the port
            25.0,
            AntCaste::Soldier,
        );
        probe.state = AntState::Exploring;
        probe.module_id = 0;
        sim.ants.push(probe);

        sim.run(80);
        let ant = &sim.ants[0];
        assert!(
            !ant.is_in_transit(),
            "oversized ant entered the tube (transit={:?})",
            ant.transit
        );
        assert_eq!(ant.module_id, 0, "oversized ant left module 0");
    }

    #[test]
    fn feeding_dish_refills_food() {
        // Build a tiny 3-module formicarium with a FeedingDish; drain it
        // and verify it refills after the cooldown.
        let cfg = small_config();
        let topology = Topology::starter_formicarium_with_feeder((24, 20), (48, 48), (20, 16));
        let mut sim = Simulation::new_with_topology(cfg, topology, 3);

        // Seed the dish so the *first* refill event is deterministic and
        // then drain every cell.
        sim.spawn_food_cluster_on(2, 10, 8, 2, 3);
        let dish_idx = 2usize;
        let (w, h) = {
            let m = sim.topology.module(dish_idx as u16);
            (m.width(), m.height())
        };
        // Empty all food in the dish.
        for y in 0..h {
            for x in 0..w {
                let cur = sim.topology.module(dish_idx as u16).world.get(x, y);
                if let Terrain::Food(n) = cur {
                    for _ in 0..n {
                        let _ = sim
                            .topology
                            .module_mut(dish_idx as u16)
                            .world
                            .take_food(x, y);
                    }
                }
            }
        }
        // Also reset cooldown so the first empty tick can trigger refill.
        sim.topology.module_mut(dish_idx as u16).tick_cooldown = 0;

        let count_food = |sim: &Simulation| -> u32 {
            let mut t = 0u32;
            for c in sim.topology.module(dish_idx as u16).world.cells.iter() {
                if let Terrain::Food(n) = c {
                    t = t.saturating_add(*n);
                }
            }
            t
        };
        let before = count_food(&sim);
        assert_eq!(before, 0, "drain failed, dish still has food: {}", before);

        // Next tick should refill (cooldown was 0 and food < threshold).
        sim.run(1);
        let after_one = count_food(&sim);
        assert!(
            after_one > 0,
            "FeedingDish did not refill on first eligible tick: {}",
            after_one
        );

        // Run past the cooldown to ensure repeat refills are possible.
        // (Drain again, wait out cooldown, confirm another refill.)
        for y in 0..h {
            for x in 0..w {
                let cur = sim.topology.module(dish_idx as u16).world.get(x, y);
                if let Terrain::Food(n) = cur {
                    for _ in 0..n {
                        let _ = sim
                            .topology
                            .module_mut(dish_idx as u16)
                            .world
                            .take_food(x, y);
                    }
                }
            }
        }
        sim.run(700); // > 600-tick cooldown
        let after_cd = count_food(&sim);
        assert!(
            after_cd > 0,
            "FeedingDish did not refill after cooldown: {}",
            after_cd
        );
    }

    #[test]
    fn pheromone_bleeds_across_tube() {
        // Deposit a strong food trail right at the nest-side port. After
        // a few ticks of port-bleed, the matched port on the outworld
        // module must have accumulated some intensity.
        let cfg = small_config();
        let topology = Topology::starter_formicarium((32, 24), (64, 64));
        let mut sim = Simulation::new_with_topology(cfg, topology, 1);
        let nest_port = sim.topology.module(0).ports[0];
        let out_port = sim.topology.module(1).ports[0];

        for _ in 0..5 {
            sim.topology.module_mut(0).pheromones.deposit(
                nest_port.x as usize,
                nest_port.y as usize,
                PheromoneLayer::FoodTrail,
                9.0,
                10.0,
            );
            sim.port_bleed();
        }

        let leaked = sim
            .topology
            .module(1)
            .pheromones
            .read(out_port.x as usize, out_port.y as usize, PheromoneLayer::FoodTrail);
        assert!(leaked > 0.5, "pheromone did not bleed across tube: {}", leaked);
    }
}

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
use crate::environment::{Climate, Environment, Season};
use crate::milestones::{Milestone, MilestoneKind};
use crate::module::{ModuleId, ModuleKind, PortPos};
use crate::persist::Snapshot;
use crate::pheromone::{PheromoneGrid, PheromoneLayer};
use crate::topology::Topology;
use crate::tube::{TubeId, TubeTransit};
use crate::world::{Terrain, WorldGrid};

/// Fraction of per-layer pheromone that equilibrates across a tube each tick.
/// 0.0 = isolated modules (no scent leaks). 1.0 = instant average.
const PORT_BLEED_RATE: f32 = 0.35;

/// K3: per-tick relaxation rate of a cell toward its `ambient_target`.
const TEMP_DRIFT_RATE: f32 = 0.01;

/// K3: how many in-game days in diapause are required per year for a
/// species marked `hibernation_required` to keep queen fertility.
const MIN_DIAPAUSE_DAYS: u32 = 60;

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
    /// K3: annual climate knobs driving ambient temperature.
    pub climate: Climate,
    /// K3: in-game seconds elapsed per sim tick. Default 1.0; set by
    /// `set_environment` to `time_scale.multiplier() / tick_rate_hz`.
    pub in_game_seconds_per_tick: f32,
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
            climate: Climate::default(),
            in_game_seconds_per_tick: 1.0,
        }
    }

    /// Expose the internal ant-id counter for snapshotting.
    #[inline]
    pub fn next_ant_id_value(&self) -> u32 {
        self.next_ant_id
    }

    // ---- K4 save/load ----

    /// Reconstruct a simulation from a snapshot. `cfg` is the `SimConfig`
    /// built by `Species::apply(&env)` (or a plain `SimConfig::default()`
    /// for tests). The sim's RNG is reseeded from `snapshot.environment.seed`.
    pub fn from_snapshot_raw(snapshot: Snapshot, cfg: SimConfig) -> anyhow::Result<Self> {
        let Snapshot {
            format_version: _,
            species_id: _,
            environment,
            climate,
            tick,
            in_game_seconds_per_tick,
            next_ant_id,
            mut topology,
            ants,
            colonies,
            saved_at_unix_secs: _,
        } = snapshot;

        // Rebuild pheromone scratch buffers (not serialized).
        for m in topology.modules.iter_mut() {
            m.pheromones.rebuild_scratch();
        }

        let rng = ChaCha8Rng::seed_from_u64(environment.seed);
        let sim = Self {
            config: cfg,
            topology,
            ants,
            colonies,
            tick,
            rng,
            next_ant_id,
            climate,
            in_game_seconds_per_tick,
        };
        tracing::info!(
            tick = sim.tick,
            ants = sim.ants.len(),
            modules = sim.topology.modules.len(),
            seed = environment.seed,
            "Simulation::from_snapshot_raw restored"
        );
        Ok(sim)
    }

    /// Reconstruct a simulation from a snapshot, resolving species via a
    /// user-supplied lookup so biology is folded back into the config.
    /// Falls back to `SimConfig::default()` if the species resolver returns
    /// `None` (with a warn log).
    pub fn from_snapshot(
        snapshot: Snapshot,
        resolver: impl Fn(&str) -> Option<crate::species::Species>,
    ) -> anyhow::Result<Self> {
        let cfg = match resolver(&snapshot.species_id) {
            Some(species) => species.apply(&snapshot.environment),
            None => {
                tracing::warn!(
                    species = %snapshot.species_id,
                    "from_snapshot: species not resolvable — using default SimConfig"
                );
                SimConfig::default()
            }
        };
        Self::from_snapshot_raw(snapshot, cfg)
    }

    /// Advance the simulation by `ticks` steps. Used for offline catch-up
    /// after a save-load; suppresses per-500-tick heartbeat log spam by
    /// doing nothing special — the heartbeat will still fire, but on a
    /// dedicated catch-up run that's expected.
    pub fn catch_up(&mut self, ticks: u64) {
        let before = self.tick;
        for _ in 0..ticks {
            self.tick();
        }
        tracing::info!(
            from_tick = before,
            to_tick = self.tick,
            added = ticks,
            "catch_up complete"
        );
    }

    // ---- K4 progression helpers ----

    /// Is the given module kind currently unlocked for this simulation?
    /// Bases the decision on colony 0's population + in-game days elapsed.
    pub fn module_kind_unlocked(&self, kind: ModuleKind) -> bool {
        let days = self.in_game_total_days();
        let pop = self
            .colonies
            .first()
            .map(|c| c.adult_total())
            .unwrap_or(0);
        crate::unlocks::module_kind_unlocked(kind, days, pop)
    }

    /// Evaluate K4 milestones for each colony and append any newly earned
    /// ones to `colony.milestones`. Safe to call every tick.
    pub fn evaluate_milestones(&mut self) {
        let tick = self.tick;
        let day = self.in_game_total_days();
        let total_days = day;
        let season_idx = season_to_idx(self.season());
        let polymorphic = self.config.ant.polymorphic;
        for colony in self.colonies.iter_mut() {
            let push = |colony: &mut ColonyState, kind: MilestoneKind| {
                if colony.has_milestone(kind) {
                    return;
                }
                colony.milestones.push(Milestone {
                    kind,
                    tick_awarded: tick,
                    in_game_day: day,
                });
                tracing::info!(
                    colony_id = colony.id,
                    tick,
                    day,
                    kind = ?kind,
                    "milestone awarded"
                );
            };

            if colony.has_laid_egg {
                push(colony, MilestoneKind::FirstEgg);
            }

            if polymorphic && colony.population.soldiers > 0 {
                push(colony, MilestoneKind::FirstMajor);
            }

            let total = colony.adult_total();
            if total >= 10 {
                push(colony, MilestoneKind::PopulationTen);
            }
            if total >= 50 {
                push(colony, MilestoneKind::PopulationFifty);
            }
            if total >= 100 {
                push(colony, MilestoneKind::PopulationOneHundred);
            }
            if total >= 500 {
                push(colony, MilestoneKind::PopulationFiveHundred);
            }
            if total_days >= 365 {
                push(colony, MilestoneKind::FirstColonyAnniversary);
            }

            // Winter→Spring transition with live adults.
            let last = colony.last_season_idx;
            if last == 0 /* winter */ && season_idx == 1 /* spring */ && total > 0 {
                push(colony, MilestoneKind::SurvivedFirstWinter);
            }
            colony.last_season_idx = season_idx;
        }
    }

    // ---- K3 seasonal clock ----

    /// Fold an `Environment` into the per-tick time stride used by the
    /// seasonal clock. Call this once after construction if the sim should
    /// age faster than 1 in-game second per tick.
    pub fn set_environment(&mut self, env: &Environment) {
        let tick_rate = env.tick_rate_hz.max(0.01);
        let scale = env.time_scale.multiplier().max(0.001);
        // Derivation: in_game_seconds_per_tick = scale / tick_rate
        // (so in N real seconds at `tick_rate` Hz we advance
        //  N*tick_rate ticks = N*scale in-game seconds).
        self.in_game_seconds_per_tick = scale / tick_rate;
        tracing::info!(
            scale,
            tick_rate,
            seconds_per_tick = self.in_game_seconds_per_tick,
            "Simulation::set_environment folded env → in_game_seconds_per_tick"
        );
    }

    /// Total in-game days elapsed since tick 0.
    #[inline]
    pub fn in_game_total_days(&self) -> u32 {
        let secs = self.tick as f64 * self.in_game_seconds_per_tick as f64;
        (secs / 86_400.0).floor() as u32
    }

    /// Current day-of-year in [0, 365), starting from `climate.starting_day_of_year`.
    #[inline]
    pub fn day_of_year(&self) -> u32 {
        (self.climate.starting_day_of_year + self.in_game_total_days()) % 365
    }

    /// Full years elapsed since start (uses starting_day_of_year as offset).
    #[inline]
    pub fn in_game_year(&self) -> u32 {
        (self.climate.starting_day_of_year + self.in_game_total_days()) / 365
    }

    pub fn season(&self) -> Season {
        Season::from_day_of_year(self.day_of_year())
    }

    /// Sinusoidal ambient temperature. Peaks at `climate.peak_day`.
    /// `T(d) = mid + amp * cos(2π * (d - peak) / 365)`
    pub fn ambient_temp_c(&self) -> f32 {
        let d = self.day_of_year() as f32;
        let phase = (d - self.climate.peak_day as f32) / 365.0;
        self.climate.seasonal_mid_c
            + self.climate.seasonal_amplitude_c * (2.0 * std::f32::consts::PI * phase).cos()
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

    // ---- K2.3 live topology mutation helpers ----

    /// Add a new module to the live topology. Auto-seeds four edge-center
    /// ports (E/W/N/S) so the editor can draw tubes immediately without a
    /// separate port-placement step.
    pub fn add_module(
        &mut self,
        kind: crate::module::ModuleKind,
        width: usize,
        height: usize,
        origin: Vec2,
        label: impl Into<String>,
    ) -> ModuleId {
        let id = self.topology.add_module(kind, width, height, origin, label);
        // Default ports: one in the middle of each edge.
        let ports = vec![
            PortPos::new(width - 1, height / 2), // east
            PortPos::new(0, height / 2),         // west
            PortPos::new(width / 2, 0),          // south
            PortPos::new(width / 2, height - 1), // north
        ];
        self.topology.module_mut(id).ports = ports;
        id
    }

    /// Add a tube between two existing ports. Returns its id. Callers are
    /// responsible for ensuring the target ports actually exist on their
    /// modules.
    pub fn add_tube(
        &mut self,
        from_mod: ModuleId,
        from_port: PortPos,
        to_mod: ModuleId,
        to_port: PortPos,
        length_ticks: u32,
        bore_width_mm: f32,
    ) -> TubeId {
        use crate::tube::TubeEnd;
        self.topology.add_tube(
            TubeEnd {
                module: from_mod,
                port: from_port,
            },
            TubeEnd {
                module: to_mod,
                port: to_port,
            },
            length_ticks,
            bore_width_mm,
        )
    }

    /// Remove a module + all tubes connected to it. Evicts any ant whose
    /// `module_id` matched OR whose transit was on one of the removed tubes.
    /// Population counts are decremented accordingly. Returns the number of
    /// ants killed.
    pub fn remove_module(&mut self, id: ModuleId) -> usize {
        let removed_tubes = self.topology.remove_module(id);
        let removed_before = self.ants.len();
        let removed_tubes_set = removed_tubes.clone();
        self.ants.retain(|a| {
            let module_gone = a.module_id == id;
            let transit_gone = a
                .transit
                .as_ref()
                .map(|t| removed_tubes_set.contains(&t.tube))
                .unwrap_or(false);
            let kill = module_gone || transit_gone;
            if kill {
                // Decrement population counts inline.
                // (We can't capture self.colonies mutably in the closure —
                // we re-scan below.)
            }
            !kill
        });
        let killed = removed_before - self.ants.len();
        self.rebuild_population_counts();
        tracing::info!(module_id = id, killed, "Simulation::remove_module");
        killed
    }

    /// Remove a tube. Evicts any ant currently in transit on it. Returns
    /// the number of ants killed.
    pub fn remove_tube(&mut self, id: TubeId) -> usize {
        if !self.topology.remove_tube(id) {
            return 0;
        }
        let before = self.ants.len();
        self.ants.retain(|a| {
            a.transit
                .as_ref()
                .map(|t| t.tube != id)
                .unwrap_or(true)
        });
        let killed = before - self.ants.len();
        self.rebuild_population_counts();
        tracing::info!(tube_id = id, killed, "Simulation::remove_tube");
        killed
    }

    /// Recount `colony.population` from the current `self.ants`. Used after
    /// live topology edits kill ants.
    fn rebuild_population_counts(&mut self) {
        for c in self.colonies.iter_mut() {
            c.population = crate::colony::PopulationCounts::default();
        }
        for a in &self.ants {
            if let Some(c) = self.colonies.iter_mut().find(|c| c.id == a.colony_id) {
                match a.caste {
                    AntCaste::Worker => c.population.workers += 1,
                    AntCaste::Soldier => c.population.soldiers += 1,
                    AntCaste::Breeder => c.population.breeders += 1,
                    AntCaste::Queen => {}
                }
            }
        }
    }

    /// Advance the simulation by one tick.
    pub fn tick(&mut self) {
        let _span = tracing::debug_span!("tick", n = self.tick).entered();

        self.temperature_tick();
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

        self.nuptial_flight_tick();

        self.evaluate_milestones();

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
        let cold_t = cfg.ant.hibernation_cold_threshold_c;
        let warm_t = cfg.ant.hibernation_warm_threshold_c;
        let mut new_headings = Vec::with_capacity(self.ants.len());
        let mut new_states: Vec<Option<AntState>> = Vec::with_capacity(self.ants.len());

        for ant in &self.ants {
            if ant.is_in_transit() {
                new_headings.push(ant.heading);
                new_states.push(None);
                continue;
            }
            let module = topology.module(ant.module_id);
            // K3 diapause override: combat/flee states are preserved, but
            // every other state can flip to/from Diapause based on local temp.
            let temp = module.temp_at(ant.position);
            let preserve_combat = matches!(
                ant.state,
                AntState::Fighting | AntState::Fleeing | AntState::NuptialFlight
            );
            if !preserve_combat {
                if ant.state != AntState::Diapause && temp < cold_t {
                    new_headings.push(ant.heading);
                    new_states.push(Some(AntState::Diapause));
                    continue;
                }
                if ant.state == AntState::Diapause {
                    if temp > warm_t {
                        new_headings.push(ant.heading);
                        new_states.push(Some(AntState::Exploring));
                    } else {
                        new_headings.push(ant.heading);
                        new_states.push(None);
                    }
                    continue;
                }
            }
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
                AntState::Diapause => None,
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

    /// K5 nuptial flight.
    ///
    /// Each tick:
    /// 1. **Launch** — if enough eligible breeders exist (Exploring +
    ///    age ≥ `nuptial_breeder_min_age`), transition them all to
    ///    `AntState::NuptialFlight` and zero their `state_timer`. The
    ///    whole batch takes off together — no stragglers.
    /// 2. **Predation** — each flying breeder rolls against
    ///    `nuptial_predation_per_tick`; failures are removed.
    /// 3. **Resolution** — once a breeder's `state_timer` reaches
    ///    `nuptial_flight_ticks`, roll `nuptial_founding_chance`. On
    ///    success, increment the colony's `daughter_colonies_founded`
    ///    (the founder despawns either way — she left to start a new
    ///    colony, which is beyond the scope of K5).
    pub fn nuptial_flight_tick(&mut self) {
        use crate::ant::AntCaste;

        let col_cfg = self.config.colony.clone();
        let min_count = col_cfg.nuptial_breeder_min;
        let min_age = col_cfg.nuptial_breeder_min_age;
        let flight_ticks = col_cfg.nuptial_flight_ticks;
        let predation = col_cfg.nuptial_predation_per_tick;
        let founding = col_cfg.nuptial_founding_chance;
        let tick = self.tick;
        let day_of_year = self.day_of_year();

        if self.colonies.is_empty() {
            return;
        }

        // --- 1. Launch: any batch of ready breeders takes off together. ---
        let ready_indices: Vec<usize> = self
            .ants
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                a.caste == AntCaste::Breeder
                    && a.state == AntState::Exploring
                    && a.age >= min_age
                    && a.transit.is_none()
            })
            .map(|(i, _)| i)
            .collect();
        if ready_indices.len() as u32 >= min_count {
            for &i in &ready_indices {
                let ant = &mut self.ants[i];
                ant.state = AntState::NuptialFlight;
                ant.state_timer = 0;
            }
            // Track on colony 0 for now (single-colony K5).
            let c = &mut self.colonies[0];
            c.nuptial_launches += ready_indices.len() as u32;
            c.last_nuptial_flight_tick = tick;
            if !c.has_milestone(crate::milestones::MilestoneKind::FirstNuptialFlight) {
                let day = day_of_year;
                c.milestones.push(crate::milestones::Milestone {
                    kind: crate::milestones::MilestoneKind::FirstNuptialFlight,
                    tick_awarded: tick,
                    in_game_day: day,
                });
                tracing::info!(
                    tick,
                    count = ready_indices.len(),
                    "milestone: FirstNuptialFlight"
                );
            } else {
                tracing::info!(
                    tick,
                    count = ready_indices.len(),
                    "nuptial flight: batch launched"
                );
            }
        }

        // --- 2. Predation + 3. Resolution (pass over flying breeders). ---
        let mut predated: u32 = 0;
        let mut founded: u32 = 0;
        let mut flight_ended_empty: u32 = 0;
        // Work with indices so we can borrow `self.rng` and `self.ants`
        // separately. Collect in descending order so swap_remove is safe.
        let mut to_remove: Vec<usize> = Vec::new();
        for i in (0..self.ants.len()).rev() {
            if self.ants[i].state != AntState::NuptialFlight {
                continue;
            }
            if predation > 0.0 && self.rng.gen_range(0.0..1.0) < predation {
                predated += 1;
                to_remove.push(i);
                continue;
            }
            // state_timer is incremented by sense_and_decide earlier in the tick.
            if self.ants[i].state_timer >= flight_ticks {
                if self.rng.gen_range(0.0..1.0) < founding {
                    founded += 1;
                } else {
                    flight_ended_empty += 1;
                }
                to_remove.push(i);
            }
        }
        for i in to_remove {
            self.ants.swap_remove(i);
        }

        if predated + founded + flight_ended_empty > 0 {
            let c = &mut self.colonies[0];
            c.nuptial_predation_deaths += predated;
            c.daughter_colonies_founded += founded;
            if founded > 0
                && !c.has_milestone(crate::milestones::MilestoneKind::FirstDaughterColony)
            {
                let day = day_of_year;
                c.milestones.push(crate::milestones::Milestone {
                    kind: crate::milestones::MilestoneKind::FirstDaughterColony,
                    tick_awarded: tick,
                    in_game_day: day,
                });
                tracing::info!(tick, "milestone: FirstDaughterColony");
            }
            tracing::info!(
                tick,
                predated,
                founded,
                lost_no_mate = flight_ended_empty,
                "nuptial flight: resolution"
            );
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

    /// K3: update per-module temperature grids. Each module has a target
    /// temperature determined by its kind (HeatChamber 28°C, HibernationChamber
    /// 5°C, else ambient). Every cell relaxes toward that target by
    /// `TEMP_DRIFT_RATE` each tick. Every 8 ticks, a 5-point Laplacian
    /// diffusion spreads the scalar field between neighboring cells.
    pub fn temperature_tick(&mut self) {
        let ambient = self.ambient_temp_c();
        for module in self.topology.modules.iter_mut() {
            let target = match module.kind {
                ModuleKind::HeatChamber => 28.0,
                ModuleKind::HibernationChamber => 5.0,
                _ => ambient,
            };
            module.ambient_target = target;
            for v in module.temperature.iter_mut() {
                *v += (target - *v) * TEMP_DRIFT_RATE;
            }
        }

        if self.tick % 8 == 0 {
            for module in self.topology.modules.iter_mut() {
                let w = module.width();
                let h = module.height();
                diffuse_scalar_grid(&mut module.temperature, w, h, 0.1);
            }
        }
    }

    /// Run one economy step for every colony: consume food, age brood,
    /// lay eggs, mature pupae into new `Ant`s, apply starvation deaths.
    pub fn colony_economy_tick(&mut self) {
        let ccfg = self.config.colony.clone();
        let worker_health = self.config.combat.worker_health;
        let soldier_health = self.config.combat.soldier_health;
        let cold_t = self.config.ant.hibernation_cold_threshold_c;
        let hibernation_required = self.config.ant.hibernation_required;
        let seconds_per_tick = self.in_game_seconds_per_tick;
        let current_year = self.in_game_year();
        const MAX_EGGS_PER_TICK: u32 = 10;

        // Determine diapause status per colony — nest entrance 0 on module 0.
        // Simplest authoritative check: temp at that cell < cold threshold.
        let mut colony_diapause: Vec<(u8, bool)> = Vec::with_capacity(self.colonies.len());
        for c in &self.colonies {
            let in_diapause = if let Some(ne) = c.nest_entrance_positions.first() {
                let m = self.topology.module(0);
                m.temp_at(*ne) < cold_t
            } else {
                false
            };
            colony_diapause.push((c.id, in_diapause));
        }

        let mut to_spawn: Vec<(u8, AntCaste, Vec2)> = Vec::new();
        let mut starve: Vec<(u8, u32)> = Vec::new();

        for colony in self.colonies.iter_mut() {
            let _span = tracing::debug_span!("colony_tick", colony_id = colony.id).entered();

            let in_diapause = colony_diapause
                .iter()
                .find(|(cid, _)| *cid == colony.id)
                .map(|(_, b)| *b)
                .unwrap_or(false);

            // --- K3 diapause accumulator (for fertility-gate bookkeeping) ---
            if in_diapause {
                colony.diapause_seconds_this_year += seconds_per_tick;
                while colony.diapause_seconds_this_year >= 86_400.0 {
                    colony.diapause_seconds_this_year -= 86_400.0;
                    colony.days_in_diapause_this_year =
                        colony.days_in_diapause_this_year.saturating_add(1);
                }
            }

            // --- K3 fertility gate: evaluated on year rollover ---
            if current_year > colony.last_year_evaluated {
                // Boot safety: year 0 → first rollover we never suppress,
                // just snapshot + reset. After year 1 onward, check.
                if colony.last_year_evaluated == 0 && current_year == 1 {
                    // First real rollover — check only if hibernation required.
                    if hibernation_required {
                        let ok = colony.days_in_diapause_this_year >= MIN_DIAPAUSE_DAYS;
                        let newly_suppressed = !ok && !colony.fertility_suppressed;
                        colony.fertility_suppressed = !ok;
                        if newly_suppressed {
                            tracing::info!(
                                colony_id = colony.id,
                                year = current_year,
                                diapause_days = colony.days_in_diapause_this_year,
                                required = MIN_DIAPAUSE_DAYS,
                                "missed winter — queen fertility suppressed"
                            );
                        }
                    } else {
                        colony.fertility_suppressed = false;
                    }
                } else if hibernation_required {
                    let ok = colony.days_in_diapause_this_year >= MIN_DIAPAUSE_DAYS;
                    let newly_suppressed = !ok && !colony.fertility_suppressed;
                    colony.fertility_suppressed = !ok;
                    if newly_suppressed {
                        tracing::info!(
                            colony_id = colony.id,
                            year = current_year,
                            diapause_days = colony.days_in_diapause_this_year,
                            required = MIN_DIAPAUSE_DAYS,
                            "missed winter — queen fertility suppressed"
                        );
                    }
                } else {
                    colony.fertility_suppressed = false;
                }
                colony.days_in_diapause_this_year = 0;
                colony.diapause_seconds_this_year = 0.0;
                colony.last_year_evaluated = current_year;
            }

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

            if queen_alive && !colony.fertility_suppressed && colony.food_stored >= ccfg.egg_cost {
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
            if in_diapause {
                // Brood development pauses entirely during colony diapause.
                // Ages stay frozen until thaw.
            } else {
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

#[inline]
fn season_to_idx(s: Season) -> u8 {
    match s {
        Season::Winter => 0,
        Season::Spring => 1,
        Season::Summer => 2,
        Season::Autumn => 3,
    }
}

/// Generic scalar 5-point Laplacian diffusion with a scratch copy. Used by
/// the K3 temperature grid; reuses the same stencil as `PheromoneGrid::diffuse`
/// but operates on a single layer and does not clamp.
fn diffuse_scalar_grid(data: &mut Vec<f32>, width: usize, height: usize, rate: f32) {
    if data.is_empty() || width == 0 || height == 0 {
        return;
    }
    let src: Vec<f32> = data.clone();
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let c = src[i];
            let up = if y > 0 { src[i - width] } else { c };
            let dn = if y + 1 < height { src[i + width] } else { c };
            let lf = if x > 0 { src[i - 1] } else { c };
            let rt = if x + 1 < width { src[i + 1] } else { c };
            data[i] = c * (1.0 - 4.0 * rate) + rate * (up + dn + lf + rt);
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
        AntState::Diapause => None,
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
    let mut ants = Vec::with_capacity(config.ant.initial_count + 1);
    let worker_health = config.combat.worker_health;
    let soldier_health = config.combat.soldier_health;

    // Every founding colony gets a visible queen as ant #0. She sits on
    // the nest entrance (Idle is not in the `moving` match set, so she
    // does not walk around), is rendered at 1.3× worker scale with the
    // queen silhouette, and can be clicked in the inspector. Economy
    // continues to read `ColonyState.queen_health` for egg-laying —
    // `sync_queen_ant` keeps the two values in lockstep each tick.
    ants.push(Ant {
        id: id_offset,
        position: nest,
        heading: 0.0,
        state: AntState::Idle,
        caste: AntCaste::Queen,
        colony_id,
        health: 100.0,
        food_carried: 0.0,
        age: 0,
        state_timer: 0,
        module_id: 0,
        transit: None,
    });

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
            id_offset + 1 + i as u32,
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
        // 20 workers + 1 queen spawned at the nest.
        assert_eq!(sim.ants.len(), 21);
        assert_eq!(
            sim.ants.iter().filter(|a| a.caste == AntCaste::Queen).count(),
            1,
            "exactly one queen at spawn"
        );
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
        // Use the specific ports the tube uses: nest east ↔ outworld west.
        let nest_port = crate::PortPos::new(31, 12);
        let out_port = crate::PortPos::new(0, 32);

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

    // ---- K3 thermoregulation + hibernation tests ----

    #[test]
    fn ambient_temp_varies_with_day() {
        let cfg = small_config();
        let mut sim = Simulation::new(cfg, 1);
        sim.climate.peak_day = 180;
        sim.climate.starting_day_of_year = 60;
        // day 60 (start): winter-ish in a 180-peak climate.
        let t_winter = sim.ambient_temp_c();
        // Manually warp the clock to day 180 by advancing in_game_seconds_per_tick.
        sim.climate.starting_day_of_year = 180;
        let t_summer = sim.ambient_temp_c();
        assert!(
            t_summer > t_winter,
            "summer not warmer than winter: summer={:.2} winter={:.2}",
            t_summer,
            t_winter
        );
    }

    #[test]
    fn module_temp_drifts_toward_ambient() {
        let cfg = small_config();
        let mut sim = Simulation::new(cfg, 2);
        // Force ambient to 10°C by moving day-of-year to winter trough.
        sim.climate.peak_day = 180;
        sim.climate.seasonal_mid_c = 20.0;
        sim.climate.seasonal_amplitude_c = 10.0;
        sim.climate.starting_day_of_year = (180 + 365 / 2) as u32 % 365;
        // Seed all cells at 20.0 (default), run 200 ticks.
        for _ in 0..200 {
            sim.temperature_tick();
            sim.tick = sim.tick.wrapping_add(1);
        }
        let m = sim.topology.module(0);
        let mean: f32 = m.temperature.iter().sum::<f32>() / m.temperature.len() as f32;
        assert!(
            mean < 19.0,
            "mean temp did not drop toward cold ambient: {:.2}",
            mean
        );
    }

    #[test]
    fn ant_enters_diapause_when_cold() {
        let mut cfg = small_config();
        cfg.ant.hibernation_cold_threshold_c = 10.0;
        cfg.ant.hibernation_warm_threshold_c = 12.0;
        let mut sim = Simulation::new(cfg, 3);
        // Place a cold spot under the ant.
        sim.ants.clear();
        let mut probe = Ant::new_worker(42, 0, Vec2::new(5.5, 5.5), 0.0, 10.0);
        probe.module_id = 0;
        sim.ants.push(probe);
        // Hammer the cell temperature to 5.0.
        {
            let m = sim.topology.module_mut(0);
            for v in m.temperature.iter_mut() {
                *v = 5.0;
            }
        }
        // Freeze climate so temperature_tick doesn't immediately warm it.
        sim.climate.seasonal_mid_c = 5.0;
        sim.climate.seasonal_amplitude_c = 0.0;
        sim.tick();
        assert_eq!(
            sim.ants[0].state,
            AntState::Diapause,
            "ant did not enter diapause in a 5°C cell"
        );
    }

    #[test]
    fn fertility_suppressed_if_no_winter() {
        // Species requires hibernation but climate never dips below cold
        // threshold. After 1 year, fertility must be suppressed.
        let mut cfg = small_config();
        cfg.ant.hibernation_required = true;
        cfg.ant.hibernation_cold_threshold_c = 10.0;
        cfg.ant.hibernation_warm_threshold_c = 12.0;
        cfg.colony.queen_egg_rate = 0.0;
        cfg.colony.adult_food_consumption = 0.0;
        let mut sim = Simulation::new(cfg, 4);
        // Force always-warm climate.
        sim.climate.seasonal_mid_c = 25.0;
        sim.climate.seasonal_amplitude_c = 0.0;
        // 1 day per 2 ticks → 1 year = 730 ticks.
        sim.in_game_seconds_per_tick = 86_400.0 / 2.0;
        sim.run(800);
        assert!(
            sim.colonies[0].fertility_suppressed,
            "fertility not suppressed after a missed winter (days_in_diapause={})",
            sim.colonies[0].days_in_diapause_this_year
        );
    }

    #[test]
    fn fertility_ok_if_winter_observed() {
        // Same species requiring hibernation but with a real winter climate;
        // after a year fertility should NOT be suppressed.
        let mut cfg = small_config();
        cfg.ant.hibernation_required = true;
        cfg.ant.hibernation_cold_threshold_c = 10.0;
        cfg.ant.hibernation_warm_threshold_c = 12.0;
        cfg.colony.queen_egg_rate = 0.0;
        cfg.colony.adult_food_consumption = 0.0;
        let mut sim = Simulation::new(cfg, 5);
        // Cold climate: annual range -5 to 25. Winter trough is -5 → well below threshold.
        sim.climate.seasonal_mid_c = 10.0;
        sim.climate.seasonal_amplitude_c = 15.0;
        sim.climate.peak_day = 180;
        sim.climate.starting_day_of_year = 0; // winter at start
        // Force all module temperatures to match ambient fast by nudging drift.
        // 1 day per 2 ticks → 1 year = 730 ticks.
        sim.in_game_seconds_per_tick = 86_400.0 / 2.0;
        // Pre-cool the nest module to winter temp to avoid a 200-tick drift lag.
        {
            let winter_amb = sim.ambient_temp_c();
            let m = sim.topology.module_mut(0);
            for v in m.temperature.iter_mut() {
                *v = winter_amb;
            }
        }
        sim.run(800);
        assert!(
            !sim.colonies[0].fertility_suppressed,
            "fertility suppressed even with a proper winter (days_in_diapause={})",
            sim.colonies[0].days_in_diapause_this_year
        );
    }

    #[test]
    fn live_add_module_tube_round_trip() {
        // K2.3: the editor adds+removes modules + tubes at runtime. Verify
        // that add/remove is symmetrical and ants are cleaned up.
        let cfg = small_config();
        let topology = Topology::starter_formicarium((32, 24), (64, 64));
        let mut sim = Simulation::new_with_topology(cfg, topology, 1);
        assert_eq!(sim.topology.modules.len(), 2);

        let new_id = sim.add_module(
            ModuleKind::Hydration,
            24,
            24,
            Vec2::new(200.0, 0.0),
            "Test Hydration",
        );
        assert_eq!(sim.topology.modules.len(), 3);
        assert_eq!(sim.topology.module(new_id).ports.len(), 4, "auto-seeded 4 ports");

        // Place an ant on the new module, then remove it. Ant must be killed.
        let ants_before = sim.ants.len();
        let east_port = sim.topology.module(new_id).ports[0];
        let _tube_id = sim.add_tube(
            1, // outworld (has a west port)
            sim.topology.module(1).ports[0],
            new_id,
            east_port,
            20,
            8.0,
        );
        // Put an ant directly on the new module.
        sim.ants.push(Ant::new_worker(
            9_999,
            0,
            Vec2::new(5.0, 5.0),
            0.0,
            10.0,
        ));
        let idx = sim.ants.len() - 1;
        sim.ants[idx].module_id = new_id;

        let killed = sim.remove_module(new_id);
        assert_eq!(killed, 1, "ant on removed module was not killed");
        assert_eq!(sim.ants.len(), ants_before);
        assert_eq!(sim.topology.modules.len(), 2);
        assert!(
            sim.topology.tubes.iter().all(|t| t.from.module != new_id && t.to.module != new_id),
            "tube touching removed module still present"
        );
    }

    // ---- K4 milestone tests ----

    #[test]
    fn first_egg_milestone_awarded() {
        let mut cfg = small_config();
        cfg.colony.initial_food = 100_000.0;
        cfg.colony.queen_egg_rate = 1.0;
        cfg.colony.egg_cost = 1.0;
        cfg.colony.adult_food_consumption = 0.0;
        let mut sim = Simulation::new(cfg, 42);
        for _ in 0..300 {
            sim.tick();
            if sim.colonies[0].has_laid_egg {
                break;
            }
        }
        assert!(
            sim.colonies[0]
                .milestones
                .iter()
                .any(|m| m.kind == MilestoneKind::FirstEgg),
            "FirstEgg milestone missing: {:?}",
            sim.colonies[0].milestones
        );
    }

    #[test]
    fn population_ten_awarded_once() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 15; // start above 10 so pop10 fires immediately
        let mut sim = Simulation::new(cfg, 7);
        sim.tick();
        let count_pop10 = sim.colonies[0]
            .milestones
            .iter()
            .filter(|m| m.kind == MilestoneKind::PopulationTen)
            .count();
        assert_eq!(count_pop10, 1, "pop10 should fire once");
        // Now simulate an oscillation: population dropping and rising again.
        sim.ants.truncate(5);
        sim.colonies[0].population.workers = 5;
        sim.tick();
        sim.tick();
        // Push it back up past 10.
        while sim.colonies[0].adult_total() < 12 {
            sim.ants.push(Ant::new_worker(
                9000 + sim.ants.len() as u32,
                0,
                Vec2::new(5.0, 5.0),
                0.0,
                10.0,
            ));
            sim.colonies[0].population.workers += 1;
        }
        sim.tick();
        let count_pop10 = sim.colonies[0]
            .milestones
            .iter()
            .filter(|m| m.kind == MilestoneKind::PopulationTen)
            .count();
        assert_eq!(count_pop10, 1, "pop10 should only fire once across oscillation");
    }

    #[test]
    fn nuptial_flight_launches_and_resolves() {
        use crate::ant::AntCaste;

        let mut cfg = small_config();
        cfg.colony.nuptial_breeder_min = 2;
        cfg.colony.nuptial_breeder_min_age = 0;
        cfg.colony.nuptial_flight_ticks = 10;
        cfg.colony.nuptial_predation_per_tick = 0.0; // deterministic — no deaths mid-flight
        cfg.colony.nuptial_founding_chance = 1.0; // deterministic founding
        cfg.ant.initial_count = 0;

        let mut sim = Simulation::new(cfg, 42);
        // Seed three Breeders at the nest.
        for i in 0..3 {
            let mut a = Ant::new_with_caste(
                5000 + i,
                0,
                Vec2::new(3.0, 3.0),
                0.0,
                10.0,
                AntCaste::Breeder,
            );
            a.age = 100;
            a.state = AntState::Exploring;
            sim.ants.push(a);
        }

        // One tick: launch.
        sim.tick();
        let flying: usize = sim
            .ants
            .iter()
            .filter(|a| a.state == AntState::NuptialFlight)
            .count();
        assert_eq!(flying, 3, "all three breeders should be airborne");
        assert_eq!(sim.colonies[0].nuptial_launches, 3);

        // Advance past the flight window; all should resolve as founders.
        for _ in 0..20 {
            sim.tick();
        }
        assert!(
            sim.ants
                .iter()
                .all(|a| a.state != AntState::NuptialFlight),
            "no breeders should still be airborne after 20 ticks"
        );
        assert_eq!(
            sim.colonies[0].daughter_colonies_founded, 3,
            "all three should have founded daughter colonies (deterministic)"
        );
        assert!(
            sim.colonies[0]
                .milestones
                .iter()
                .any(|m| m.kind == MilestoneKind::FirstNuptialFlight),
            "FirstNuptialFlight milestone should fire"
        );
        assert!(
            sim.colonies[0]
                .milestones
                .iter()
                .any(|m| m.kind == MilestoneKind::FirstDaughterColony),
            "FirstDaughterColony milestone should fire"
        );
    }

    #[test]
    fn nuptial_flight_waits_for_threshold() {
        use crate::ant::AntCaste;

        let mut cfg = small_config();
        cfg.colony.nuptial_breeder_min = 4;
        cfg.colony.nuptial_breeder_min_age = 0;
        cfg.ant.initial_count = 0;

        let mut sim = Simulation::new(cfg, 42);
        for i in 0..2 {
            let mut a = Ant::new_with_caste(
                6000 + i,
                0,
                Vec2::new(3.0, 3.0),
                0.0,
                10.0,
                AntCaste::Breeder,
            );
            a.age = 100;
            sim.ants.push(a);
        }
        for _ in 0..5 {
            sim.tick();
        }
        // Below threshold: no one flies.
        assert!(
            sim.ants
                .iter()
                .all(|a| a.state != AntState::NuptialFlight),
            "below threshold — no launch"
        );
        assert_eq!(sim.colonies[0].nuptial_launches, 0);
    }
}

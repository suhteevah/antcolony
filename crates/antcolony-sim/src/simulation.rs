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
use rayon::prelude::*;
use crate::world::{Terrain, WorldGrid};

/// Fraction of per-layer pheromone that equilibrates across a tube each tick.
/// 0.0 = isolated modules (no scent leaks). 1.0 = instant average.
/// Default port bleed rate for snapshots loaded without a config field.
/// Live runs read from `SimConfig.pheromone.port_bleed_rate` which is
/// time-scaled by `Species::apply`.
const _DEFAULT_PORT_BLEED_RATE: f32 = 0.35;

/// K3: per-tick relaxation rate of a cell toward its `ambient_target`.
const TEMP_DRIFT_RATE: f32 = 0.01;

/// K3 fallback: how many in-game days of diapause are required per year
/// for a species marked `hibernation_required` to keep queen fertility.
/// Used only as a safety floor if `AntConfig::min_diapause_days` is
/// somehow zero. The active value comes from species TOML →
/// `Species.biology.min_diapause_days` → `AntConfig::min_diapause_days`.
const DEFAULT_MIN_DIAPAUSE_DAYS: u32 = 60;

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
    /// Number of physics substeps per outer tick. Set by
    /// `set_environment` to `max(1, round(multiplier / SEASONAL_BASELINE))`.
    /// At Seasonal (60×) baseline = 1 substep/tick (calibrated). At
    /// Timelapse (1440×) = 24 substeps/tick. Substepping keeps
    /// per-substep biology rates calibrated at any scale, fixing the
    /// long-run-collapse bug where higher scales starved colonies
    /// because foragers couldn't keep up with auto-scaled consumption.
    pub substep_count: u32,
    /// Monotonic counter advanced once per physics substep (vs `tick`
    /// which advances once per outer tick). Used to gate per-substep
    /// cadenced work like diffusion (fires every Nth substep).
    pub substep_global: u64,
    /// P6: predator agents (spiders, antlions) living on modules.
    pub predators: Vec<crate::hazards::Predator>,
    /// Monotonic id generator for newly spawned predators.
    next_predator_id: u32,
    /// P6: weather timers + cumulative event counters.
    pub weather: crate::hazards::Weather,
    /// P7: player-placed pheromone beacons.
    pub beacons: Vec<crate::player::Beacon>,
    /// Monotonic id generator for new beacons.
    next_beacon_id: u32,
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
            substep_count: 1,
            substep_global: 0,
            predators: Vec::new(),
            next_predator_id: 0,
            weather: crate::hazards::Weather::default(),
            beacons: Vec::new(),
            next_beacon_id: 0,
        }
    }

    /// Phase 4 entry point: build a sim with TWO colonies sharing a
    /// topology. Colony 0 (black) spawns on `nest_black_module` (default 0).
    /// Colony 1 (red) is AI-controlled and spawns on `nest_red_module`
    /// (default 2, i.e. the far nest in `two_colony_arena`). Initial
    /// populations for each come from `config.ant.initial_count`.
    pub fn new_two_colony_with_topology(
        config: SimConfig,
        mut topology: Topology,
        seed: u64,
        nest_black_module: ModuleId,
        nest_red_module: ModuleId,
    ) -> Self {
        assert!(!topology.is_empty(), "at least one module required");
        let mut rng = ChaCha8Rng::seed_from_u64(seed);

        // Black colony (player).
        let black_mod = topology.module(nest_black_module);
        let (bw, bh) = (black_mod.width(), black_mod.height());
        let black_nest = Vec2::new(bw as f32 * 0.5, bh as f32 * 0.5);
        let mut c_black = ColonyState::new(0, config.colony.initial_food, black_nest);

        let dist = CasteRatio { worker: 1.0, soldier: 0.0, breeder: 0.0 };
        let mut black_ants = spawn_initial_ants(&config, &mut rng, black_nest, 0, dist, 0);
        for a in black_ants.iter_mut() { a.module_id = nest_black_module; }

        // Red colony (AI).
        let red_mod = topology.module(nest_red_module);
        let (rw, rh) = (red_mod.width(), red_mod.height());
        let red_nest = Vec2::new(rw as f32 * 0.5, rh as f32 * 0.5);
        let mut c_red = ColonyState::new(1, config.colony.initial_food, red_nest);
        c_red.is_ai_controlled = true;
        // Red colonies lean defensive — more soldiers by default.
        c_red.caste_ratio = CasteRatio { worker: 0.65, soldier: 0.3, breeder: 0.05 };

        let id_offset = black_ants.len() as u32;
        let mut red_ants = spawn_initial_ants(&config, &mut rng, red_nest, 1, dist, id_offset);
        for a in red_ants.iter_mut() { a.module_id = nest_red_module; }

        let mut ants = black_ants;
        ants.append(&mut red_ants);

        for a in &ants {
            let colony = if a.colony_id == 0 { &mut c_black } else { &mut c_red };
            match a.caste {
                AntCaste::Worker => colony.population.workers += 1,
                AntCaste::Soldier => colony.population.soldiers += 1,
                AntCaste::Breeder => colony.population.breeders += 1,
                AntCaste::Queen => {}
            }
        }

        topology
            .module_mut(nest_black_module)
            .world
            .place_nest(bw / 2, bh / 2, 0);
        topology
            .module_mut(nest_red_module)
            .world
            .place_nest(rw / 2, rh / 2, 1);

        tracing::info!(
            modules = topology.modules.len(),
            tubes = topology.tubes.len(),
            ants = ants.len(),
            black = c_black.adult_total() + 1, // +queen
            red = c_red.adult_total() + 1,
            seed,
            "Simulation::new_two_colony_with_topology"
        );

        let next_ant_id = ants.len() as u32;

        // Promote one red avenger: first non-queen ant on the red nest.
        if let Some(idx) = ants
            .iter()
            .position(|a| a.colony_id == 1 && !matches!(a.caste, AntCaste::Queen))
        {
            ants[idx].is_avenger = true;
            tracing::info!(ant = ants[idx].id, "avenger assigned (red colony)");
        }

        Self {
            config,
            topology,
            ants,
            colonies: vec![c_black, c_red],
            tick: 0,
            rng,
            next_ant_id,
            climate: Climate::default(),
            in_game_seconds_per_tick: 1.0,
            substep_count: 1,
            substep_global: 0,
            predators: Vec::new(),
            next_predator_id: 0,
            weather: crate::hazards::Weather::default(),
            beacons: Vec::new(),
            next_beacon_id: 0,
        }
    }

    /// Expose the internal predator-id counter for snapshotting.
    #[inline]
    pub fn next_predator_id_value(&self) -> u32 {
        self.next_predator_id
    }

    /// Expose the internal beacon-id counter for snapshotting.
    #[inline]
    pub fn next_beacon_id_value(&self) -> u32 {
        self.next_beacon_id
    }

    /// P7: possess the nearest non-queen ant of the given colony to the
    /// given world position on the given module. Clears any prior
    /// `is_player` flag first. Returns the possessed ant's id, or
    /// `None` if no candidate exists.
    pub fn possess_nearest(
        &mut self,
        colony_id: u8,
        module: ModuleId,
        pos: Vec2,
    ) -> Option<u32> {
        // Clear any current avatar.
        for a in self.ants.iter_mut() {
            a.is_player = false;
        }
        let mut best: Option<(f32, usize)> = None;
        for (i, ant) in self.ants.iter().enumerate() {
            if ant.colony_id != colony_id
                || ant.module_id != module
                || ant.is_in_transit()
                || matches!(ant.caste, AntCaste::Queen)
            {
                continue;
            }
            let d2 = (ant.position - pos).length_squared();
            if best.map(|(bd, _)| d2 < bd).unwrap_or(true) {
                best = Some((d2, i));
            }
        }
        if let Some((_, idx)) = best {
            self.ants[idx].is_player = true;
            let id = self.ants[idx].id;
            tracing::info!(ant_id = id, colony_id, "possessed ant");
            Some(id)
        } else {
            None
        }
    }

    /// P7: the current player-avatar ant, if any. Just a helper so the
    /// render layer doesn't have to loop.
    pub fn player_ant_index(&self) -> Option<usize> {
        self.ants.iter().position(|a| a.is_player)
    }

    /// P7: set the player avatar's heading directly (WASD override).
    pub fn set_player_heading(&mut self, heading: f32) {
        if let Some(i) = self.player_ant_index() {
            self.ants[i].heading = heading;
        }
    }

    /// P7: recruit up to `max_count` nearby non-queen, non-transit ants
    /// of the leader's colony into a follow bond. Returns the number
    /// actually recruited. Already-bonded ants are replaced; the player
    /// avatar is never recruited (it's its own master).
    pub fn recruit_nearby(&mut self, leader_id: u32, radius: f32, max_count: u32) -> u32 {
        // Find leader first.
        let Some(leader) = self.ants.iter().find(|a| a.id == leader_id).cloned() else {
            return 0;
        };
        let r2 = radius * radius;
        // Collect candidate indices sorted by distance.
        let mut candidates: Vec<(f32, usize)> = self
            .ants
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                a.id != leader_id
                    && a.colony_id == leader.colony_id
                    && a.module_id == leader.module_id
                    && !a.is_in_transit()
                    && !a.is_player
                    && !matches!(a.caste, AntCaste::Queen)
            })
            .map(|(i, a)| ((a.position - leader.position).length_squared(), i))
            .filter(|(d2, _)| *d2 <= r2)
            .collect();
        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let take = (max_count as usize).min(candidates.len());
        for &(_, i) in &candidates[..take] {
            self.ants[i].follow_leader = Some(leader_id);
        }
        tracing::info!(leader_id, recruited = take, "recruit_nearby");
        take as u32
    }

    /// P7: dismiss any follower bond tied to the given leader.
    pub fn dismiss_followers(&mut self, leader_id: u32) {
        let mut released = 0;
        for a in self.ants.iter_mut() {
            if a.follow_leader == Some(leader_id) {
                a.follow_leader = None;
                released += 1;
            }
        }
        if released > 0 {
            tracing::info!(leader_id, released, "dismiss_followers");
        }
    }

    /// P7: place a pheromone beacon. Returns its id.
    pub fn place_beacon(
        &mut self,
        kind: crate::player::BeaconKind,
        module_id: ModuleId,
        pos: Vec2,
        amount_per_tick: f32,
        ticks_remaining: u32,
        owner_colony: u8,
    ) -> u32 {
        let id = self.next_beacon_id;
        self.next_beacon_id += 1;
        let beacon = crate::player::Beacon {
            id,
            kind,
            module_id,
            position: pos,
            amount_per_tick,
            ticks_remaining,
            owner_colony,
        };
        tracing::info!(
            id,
            ?kind,
            module_id,
            ticks = ticks_remaining,
            "place_beacon"
        );
        self.beacons.push(beacon);
        id
    }

    /// P7 beacon tick: deposit each active beacon's layer at its cell,
    /// tick down the counter, and drop expired beacons.
    fn beacon_tick(&mut self) {
        if self.beacons.is_empty() {
            return;
        }
        let max_intensity = self.config.pheromone.max_intensity;
        for b in self.beacons.iter_mut() {
            if b.ticks_remaining == 0 {
                continue;
            }
            let Some(module) = self.topology.try_module(b.module_id) else {
                b.ticks_remaining = 0;
                continue;
            };
            let (gx, gy) = module.pheromones.world_to_grid(b.position);
            if !module.pheromones.in_bounds(gx, gy) {
                b.ticks_remaining = 0;
                continue;
            }
            let (ux, uy) = (gx as usize, gy as usize);
            let _ = module; // release the immutable borrow before taking a mutable one
            self.topology.module_mut(b.module_id).pheromones.deposit(
                ux,
                uy,
                b.kind.layer(),
                b.amount_per_tick,
                max_intensity,
            );
            b.ticks_remaining -= 1;
        }
        self.beacons.retain(|b| b.ticks_remaining > 0);
    }

    /// P7 follower steering: followers' heading gets overridden to
    /// point at their leader's position each tick. Called between
    /// sense_and_decide and movement so recruits actually turn.
    fn follower_steering(&mut self) {
        // Snapshot leader positions first.
        use std::collections::HashMap;
        let mut leader_pos: HashMap<u32, (Vec2, ModuleId)> = HashMap::new();
        for a in &self.ants {
            leader_pos.insert(a.id, (a.position, a.module_id));
        }
        for ant in self.ants.iter_mut() {
            let Some(leader_id) = ant.follow_leader else {
                continue;
            };
            if ant.is_in_transit() || ant.is_player {
                continue;
            }
            let Some(&(lpos, lmod)) = leader_pos.get(&leader_id) else {
                // Leader gone — drop the bond.
                ant.follow_leader = None;
                continue;
            };
            if lmod != ant.module_id {
                // Leader left our module — keep bond but don't steer.
                continue;
            }
            let delta = lpos - ant.position;
            if delta.length_squared() > 0.25 {
                ant.heading = delta.y.atan2(delta.x);
            }
        }
    }

    /// P6: spawn a predator on the given module at the given cell.
    /// Returns the new predator id.
    pub fn spawn_predator(
        &mut self,
        kind: crate::hazards::PredatorKind,
        module_id: ModuleId,
        pos: Vec2,
    ) -> u32 {
        use crate::hazards::{Predator, PredatorState};
        let id = self.next_predator_id;
        self.next_predator_id += 1;
        let health = match kind {
            crate::hazards::PredatorKind::Spider => self.config.hazards.spider_health,
            // Antlions are indestructible from combat — only the game
            // mechanic of "an ant clears the pit" removes them. For MVP
            // antlions live forever.
            crate::hazards::PredatorKind::Antlion => f32::INFINITY,
        };
        let predator = Predator {
            id,
            kind,
            module_id,
            position: pos,
            heading: 0.0,
            state: PredatorState::Patrol,
            health,
        };
        tracing::info!(id, ?kind, module_id, ?pos, "predator spawned");
        self.predators.push(predator);
        id
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
            predators,
            next_predator_id,
            weather,
            beacons,
            next_beacon_id,
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
            substep_count: 1,
            substep_global: 0,
            predators,
            next_predator_id,
            weather,
            beacons,
            next_beacon_id,
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
        // Substep architecture: at higher time scales, run more physics
        // substeps per outer tick at calibrated rates. Seasonal (60×) is
        // the calibrated baseline = 1 substep. Timelapse (1440×) = 24.
        // Hyperlapse (any) = scale/60. Min 1 substep.
        const SEASONAL_BASELINE_MULTIPLIER: f32 = 60.0;
        let raw = scale / SEASONAL_BASELINE_MULTIPLIER;
        self.substep_count = raw.round().max(1.0) as u32;
        tracing::info!(
            scale,
            tick_rate,
            seconds_per_tick = self.in_game_seconds_per_tick,
            substep_count = self.substep_count,
            "Simulation::set_environment folded env → in_game_seconds_per_tick + substep_count"
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

    /// Advance the simulation by one outer tick. Inside, runs
    /// `substep_count` calibrated physics substeps so per-tick rates
    /// (worker speed, pheromone evap/diffuse, FSM transitions) stay
    /// biologically calibrated regardless of player-selected time scale.
    /// At Seasonal (60×) baseline = 1 substep (old behavior). At
    /// Timelapse = 24 substeps. Outer-tick work (colony economy, year
    /// rollover, milestones, nuptial flights) runs once per outer tick.
    pub fn tick(&mut self) {
        let _span = tracing::debug_span!("tick", n = self.tick).entered();

        // Outer-tick (low-frequency) work — runs once regardless of
        // substep_count. These advance against `in_game_seconds_per_tick`
        // (the outer-tick rate) so they correctly track in-game days
        // and years across all time scales.
        self.colony_economy_tick();
        self.nuptial_flight_tick();
        self.evaluate_milestones();

        // High-frequency physics substeps. Calibrated against the
        // Seasonal baseline (1 substep = original Seasonal tick worth
        // of biology). Faster scales run more substeps per outer tick.
        let n = self.substep_count.max(1);
        for _ in 0..n {
            self.physics_substep();
        }

        self.tick += 1;
        if self.tick % 500 == 0 {
            let c = &self.colonies[0];
            tracing::info!(
                tick = self.tick,
                ants = self.ants.len(),
                food_stored = c.food_stored,
                food_returned = c.food_returned,
                substeps = n,
                "colony heartbeat"
            );
        }
    }

    /// One calibrated physics substep — Seasonal-equivalent tick worth
    /// of ant motion + pheromone dynamics. Runs N times per outer tick
    /// where N = substep_count (1 at Seasonal, 24 at Timelapse, etc).
    fn physics_substep(&mut self) {
        self.temperature_tick();
        self.sense_and_decide();
        self.avenger_tick();
        self.follower_steering();
        self.beacon_tick();
        self.movement();
        self.combat_tick();
        self.deposit_and_interact();
        self.territory_deposit_tick();
        self.feeding_dish_tick();
        self.surface_underground_traversal();
        self.assign_diggers();
        self.dig_tick();
        self.red_ai_tick();
        self.hazards_tick();

        // Pheromone evap + diffuse parallelized across modules.
        let evap_rate = self.config.pheromone.evaporation_rate;
        let threshold = self.config.pheromone.min_threshold;
        self.topology
            .modules
            .par_iter_mut()
            .for_each(|m| m.pheromones.evaporate(evap_rate, threshold));

        // Tube pheromone evaporation runs in parallel with module evap.
        // Same rate / threshold so tube cells decay in lockstep with
        // module cells -- foragers laying trail in transit see the same
        // half-life as on module floors.
        self.topology
            .tubes
            .par_iter_mut()
            .for_each(|t| t.evaporate(evap_rate, threshold));

        // Diffusion fires every Nth substep (per-substep counter, NOT
        // outer-tick counter — diffusion should fire at substep cadence
        // so high time scales still get the same in-game-time spacing).
        if self.config.pheromone.diffusion_interval > 0
            && self.substep_global % self.config.pheromone.diffusion_interval as u64 == 0
        {
            let diff_rate = self.config.pheromone.diffusion_rate;
            self.topology
                .modules
                .par_iter_mut()
                .for_each(|m| m.pheromones.diffuse(diff_rate));
        }
        self.substep_global = self.substep_global.wrapping_add(1);

        self.port_bleed();
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
        let n = self.ants.len();

        // Per-ant deterministic RNG seeds, drawn in serial order from
        // the main RNG so the parallel decision phase below is fully
        // deterministic regardless of rayon scheduling. This preserves
        // test reproducibility while still letting all ants decide
        // their next heading + state in parallel.
        let per_ant_seeds: Vec<u64> = (0..n).map(|_| self.rng.r#gen::<u64>()).collect();

        // Parallel decision phase. Each ant gets a local ChaCha8Rng
        // seeded from its slot's pre-drawn seed; the only mutation is
        // into per-ant locals which we collect into vectors. No shared
        // mutable state -> safe to par_iter.
        let results: Vec<(f32, Option<AntState>)> = self
            .ants
            .par_iter()
            .zip(per_ant_seeds.par_iter())
            .map(|(ant, &seed)| {
                if ant.is_in_transit() {
                    return (ant.heading, None);
                }
                let module = topology.module(ant.module_id);
                let temp = module.temp_at(ant.position);
                let preserve_combat = matches!(
                    ant.state,
                    AntState::Fighting | AntState::Fleeing | AntState::NuptialFlight
                );
                if !preserve_combat {
                    if ant.state != AntState::Diapause && temp < cold_t {
                        return (ant.heading, Some(AntState::Diapause));
                    }
                    if ant.state == AntState::Diapause {
                        if temp > warm_t {
                            return (ant.heading, Some(AntState::Exploring));
                        } else {
                            return (ant.heading, None);
                        }
                    }
                }
                let mut local_rng = ChaCha8Rng::seed_from_u64(seed);
                let mut h = choose_direction(ant, &module.pheromones, &cfg.ant, &mut local_rng);
                if let Some(alarm_h) =
                    alarm_response_heading(ant, module, &cfg.ant, &cfg.pheromone)
                {
                    h = alarm_h;
                }
                let next = decide_next_state(ant, &module.world, &module.pheromones, cfg);
                (h, next)
            })
            .collect();

        // Snapshot per-colony nest entrance positions for diapause
        // teleport (ants retreating home as winter hits).
        let nest_entrances_per_colony: Vec<(u8, Vec<Vec2>)> = self
            .colonies
            .iter()
            .map(|c| (c.id, c.nest_entrance_positions.clone()))
            .collect();

        // Apply results in serial. Mutating `self.ants` by index avoids
        // any aliasing concerns and keeps state transitions deterministic
        // in the order ants were laid out.
        for (i, ant) in self.ants.iter_mut().enumerate() {
            let (h, ns) = results[i];
            if !ant.is_in_transit() {
                // P7: player avatar keeps its player-set heading; the FSM
                // still runs so food pickup / nest drop-off work.
                if !ant.is_player {
                    ant.heading = h;
                }
                if let Some(ns) = ns {
                    let was_not_in_diapause = ant.state != AntState::Diapause;
                    ant.transition(ns);
                    // Diapause retreat: when an ant first enters diapause
                    // (transitioning from any other state into Diapause),
                    // teleport it to one of its colony's nest entrance
                    // cells. Models the autumn behavior of workers
                    // returning to the nest before the cold front, then
                    // clustering in the deep chambers for the winter.
                    // Without this, surface ants freeze in place wherever
                    // they were when ambient dipped below cold_threshold,
                    // which is biologically wrong and visually weird.
                    if ns == AntState::Diapause && was_not_in_diapause {
                        if let Some((_, entries)) = nest_entrances_per_colony
                            .iter()
                            .find(|(cid, _)| *cid == ant.colony_id)
                        {
                            if let Some(pos) = entries.first() {
                                ant.position = *pos;
                                // Drop any food they were carrying — they
                                // got home, dropped it off, then settled.
                                ant.food_carried = 0.0;
                                // Snap module to the surface nest module
                                // (id 0 by convention; underground is
                                // attached separately and ants normally
                                // can't traverse the layer boundary yet).
                                ant.module_id = 0;
                            }
                        }
                    }
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

            // Phase 5: Solid/Obstacle cells block movement — reflect and
            // stay in place so an ant can't walk through unexcavated earth.
            let (nx, ny) = module.world.world_to_grid(next);
            if module.world.in_bounds(nx, ny) {
                let t = module.world.get(nx as usize, ny as usize);
                if matches!(t, Terrain::Solid | Terrain::Obstacle) {
                    ant.heading += std::f32::consts::PI;
                    continue; // skip position update — next tick re-computes
                }
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
        // Tube deposits — ants in transit lay trail on tube cells.
        // Layout matches Tube::deposit semantics: (tube_id, cell_idx, layer, amount).
        let mut tube_deposits: Vec<(TubeId, usize, usize, f32)> = Vec::new();
        for ant in &self.ants {
            if let Some(transit) = ant.transit {
                // Foragers in transit drop trail on the tube substrate.
                // Same state→layer mapping as on-grid ants: ReturningHome
                // ants drop FoodTrail (food-side recruitment), Exploring
                // and FollowingTrail drop HomeTrail (home-side recruitment).
                // Replaces the port_bleed hack with biology-correct
                // deposit-on-transit (real ants lay trail on tube walls
                // as they pass — `docs/biology.md` "chain-gang pipeline").
                let (layer_idx, amount) = match ant.state {
                    AntState::ReturningHome => (0usize, pcfg.deposit_food_trail),
                    AntState::Exploring | AntState::FollowingTrail => {
                        (1usize, pcfg.deposit_home_trail)
                    }
                    _ => continue,
                };
                let tube = self.topology.tube(transit.tube);
                let idx = tube.cell_index(transit.progress);
                tube_deposits.push((transit.tube, idx, layer_idx, amount));
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
        // Apply tube deposits — collected from transit ants above.
        for (tube_id, cell_idx, layer_idx, amount) in tube_deposits {
            self.topology
                .tube_mut(tube_id)
                .deposit(cell_idx, layer_idx, amount, pcfg.max_intensity);
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
        let rate = self.config.pheromone.port_bleed_rate;
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

    /// Phase 4: cross-colony combat. For each module, bucket non-transit
    /// ants into a spatial hash, find pairs of differing `colony_id`
    /// within `combat.interaction_radius`, and deal damage each tick.
    /// Soldiers get `soldier_vs_worker_bonus` against worker/breeder
    /// targets. Queens are non-combatants (they attack for 0 but can be
    /// damaged).
    ///
    /// Ants whose health drops to 0 are removed from `self.ants` at the
    /// end of the tick. Their grid cell becomes `Terrain::Food` (if
    /// Empty) and alarm pheromone is deposited at the death site.
    pub fn combat_tick(&mut self) {
        let cfg = self.config.combat.clone();
        let pcfg = self.config.pheromone.clone();
        let radius = cfg.interaction_radius;
        if radius <= 0.0 || self.ants.is_empty() || self.colonies.len() < 2 {
            // Nothing to fight — skip spatial hashing entirely.
            return;
        }

        // Build a per-module spatial hash of ants (by index into self.ants).
        use std::collections::HashMap;
        let mut buckets: HashMap<ModuleId, crate::spatial::SpatialHash> = HashMap::new();
        for (i, ant) in self.ants.iter().enumerate() {
            if ant.is_in_transit() {
                continue;
            }
            let hash = buckets
                .entry(ant.module_id)
                .or_insert_with(|| crate::spatial::SpatialHash::new((radius * 2.0).max(1.0)));
            hash.insert(i as u32, ant.position);
        }

        // Accumulate damage. Using a Vec<f32> aligned to self.ants so we
        // can safely borrow positions/castes of both attacker and target.
        let mut damage: Vec<f32> = vec![0.0; self.ants.len()];
        let mut attacker_of: Vec<Option<u8>> = vec![None; self.ants.len()];

        for (i, ant) in self.ants.iter().enumerate() {
            if ant.is_in_transit() {
                continue;
            }
            if matches!(ant.caste, AntCaste::Queen) {
                continue; // queens don't melee
            }
            let Some(hash) = buckets.get(&ant.module_id) else { continue };
            let candidates = hash.query_radius(ant.position, radius);
            let base_attack = match ant.caste {
                AntCaste::Soldier => cfg.soldier_attack,
                _ => cfg.worker_attack,
            };
            for j in candidates {
                let j = j as usize;
                if j == i {
                    continue;
                }
                let other = &self.ants[j];
                if other.colony_id == ant.colony_id {
                    continue;
                }
                if (ant.position - other.position).length() > radius {
                    continue;
                }
                let mut dmg = base_attack;
                if matches!(ant.caste, AntCaste::Soldier)
                    && !matches!(other.caste, AntCaste::Soldier)
                {
                    dmg *= cfg.soldier_vs_worker_bonus;
                }
                damage[j] += dmg;
                attacker_of[j] = Some(ant.colony_id);
            }
        }

        // Apply damage + flag states. Track death events for a post-pass.
        struct DeathEvent {
            idx: usize,
            module: ModuleId,
            pos: Vec2,
            victim_colony: u8,
            killer_colony: Option<u8>,
        }
        let mut deaths: Vec<DeathEvent> = Vec::new();
        for (i, ant) in self.ants.iter_mut().enumerate() {
            if damage[i] <= 0.0 {
                continue;
            }
            ant.health -= damage[i];
            // Soldiers stand and fight; workers/breeders flee.
            if ant.health > 0.0 {
                let new_state = match ant.caste {
                    AntCaste::Soldier => AntState::Fighting,
                    AntCaste::Worker | AntCaste::Breeder => AntState::Fleeing,
                    AntCaste::Queen => ant.state,
                };
                if ant.state != new_state {
                    ant.transition(new_state);
                }
            } else {
                deaths.push(DeathEvent {
                    idx: i,
                    module: ant.module_id,
                    pos: ant.position,
                    victim_colony: ant.colony_id,
                    killer_colony: attacker_of[i],
                });
            }
        }

        if deaths.is_empty() {
            return;
        }

        // Book-keep kills and losses per colony.
        for d in &deaths {
            for c in self.colonies.iter_mut() {
                if c.id == d.victim_colony {
                    c.combat_losses += 1;
                    c.combat_losses_this_tick += 1;
                    match self.ants[d.idx].caste {
                        AntCaste::Worker => c.population.workers = c.population.workers.saturating_sub(1),
                        AntCaste::Soldier => c.population.soldiers = c.population.soldiers.saturating_sub(1),
                        AntCaste::Breeder => c.population.breeders = c.population.breeders.saturating_sub(1),
                        AntCaste::Queen => { c.queen_health = 0.0; }
                    }
                }
                if Some(c.id) == d.killer_colony {
                    c.combat_kills += 1;
                }
            }
        }

        // Drop corpses as food + deposit alarm at each death site.
        for d in &deaths {
            let module = self.topology.module_mut(d.module);
            let (gx, gy) = module.world.world_to_grid(d.pos);
            if module.world.in_bounds(gx, gy) {
                let (ux, uy) = (gx as usize, gy as usize);
                if module.world.get(ux, uy) == Terrain::Empty && cfg.corpse_food_units > 0 {
                    module.world.set(ux, uy, Terrain::Food(cfg.corpse_food_units));
                }
                module.pheromones.deposit(
                    ux,
                    uy,
                    PheromoneLayer::Alarm,
                    cfg.alarm_deposit_on_death,
                    pcfg.max_intensity,
                );
            }
        }

        tracing::info!(
            tick = self.tick,
            deaths = deaths.len(),
            "combat resolved"
        );

        // Remove dead ants. Indices collected ascending; iterate descending
        // for swap_remove so later indices stay valid.
        let mut idxs: Vec<usize> = deaths.iter().map(|d| d.idx).collect();
        idxs.sort_unstable();
        for i in idxs.into_iter().rev() {
            self.ants.swap_remove(i);
        }
    }

    /// P6: drive every predator one tick + run weather events.
    ///
    /// Spiders: Patrol (random wander) → Hunt (steer toward nearest
    /// enemy ant, i.e. any ant on the same module) → Eat (hold for
    /// `spider_eat_ticks`, during which the spider is stationary and
    /// one ant has been removed). If a spider's health reaches 0 it
    /// enters `Dead` with `spider_respawn_ticks` cooldown, then respawns
    /// at its last position. A dead spider also drops food at its cell.
    ///
    /// Antlions: stationary. Any non-queen ant on the same cell dies.
    /// Antlions don't take damage from ants in this MVP (they're
    /// permanent environmental hazards).
    ///
    /// Weather: advances rain/lawnmower timers + triggers new events at
    /// their configured periods.
    pub fn hazards_tick(&mut self) {
        use crate::hazards::{PredatorKind, PredatorState};
        let cfg = self.config.hazards.clone();
        let pcfg = self.config.pheromone.clone();
        let combat = self.config.combat.clone();

        // --- 1. Advance predator FSMs. ---
        let mut killed_ants: Vec<usize> = Vec::new();

        for pi in 0..self.predators.len() {
            let predator = self.predators[pi].clone();
            match predator.kind {
                PredatorKind::Spider => self.spider_tick(pi, &cfg, &mut killed_ants),
                PredatorKind::Antlion => self.antlion_tick(pi, &mut killed_ants),
            }
        }

        // --- 2. Resolve ant deaths (antlion + spider eat). Duplicates
        //       are possible if two predators targeted the same ant; a
        //       HashSet de-dupes. ---
        if !killed_ants.is_empty() {
            killed_ants.sort_unstable();
            killed_ants.dedup();
            // Drop corpses + alarm at the victim cells before removing.
            for &idx in killed_ants.iter().rev() {
                if idx >= self.ants.len() {
                    continue;
                }
                let ant = &self.ants[idx];
                let module = self.topology.module_mut(ant.module_id);
                let (gx, gy) = module.world.world_to_grid(ant.position);
                if module.world.in_bounds(gx, gy) {
                    let (ux, uy) = (gx as usize, gy as usize);
                    if module.world.get(ux, uy) == Terrain::Empty && combat.corpse_food_units > 0 {
                        module.world.set(ux, uy, Terrain::Food(combat.corpse_food_units));
                    }
                    module.pheromones.deposit(
                        ux,
                        uy,
                        PheromoneLayer::Alarm,
                        combat.alarm_deposit_on_death,
                        pcfg.max_intensity,
                    );
                }
                // Decrement population counts on the victim's colony.
                let cid = ant.colony_id;
                let caste = ant.caste;
                for c in self.colonies.iter_mut() {
                    if c.id == cid {
                        match caste {
                            AntCaste::Worker => c.population.workers = c.population.workers.saturating_sub(1),
                            AntCaste::Soldier => c.population.soldiers = c.population.soldiers.saturating_sub(1),
                            AntCaste::Breeder => c.population.breeders = c.population.breeders.saturating_sub(1),
                            AntCaste::Queen => c.queen_health = 0.0,
                        }
                    }
                }
                self.ants.swap_remove(idx);
            }
            tracing::info!(
                tick = self.tick,
                deaths = killed_ants.len(),
                "hazards_tick: predator kills resolved"
            );
        }

        // --- 3. Tick spider respawn timers / drop corpses. ---
        for p in self.predators.iter_mut() {
            if let PredatorState::Dead { respawn_in_ticks } = p.state {
                if respawn_in_ticks > 0 {
                    p.state = PredatorState::Dead { respawn_in_ticks: respawn_in_ticks - 1 };
                } else if cfg.spider_respawn_ticks > 0 {
                    p.state = PredatorState::Patrol;
                    p.health = cfg.spider_health;
                    tracing::info!(id = p.id, "spider respawned");
                }
            }
        }

        // --- 4. Weather events. ---
        self.weather_tick(&cfg);
    }

    fn spider_tick(
        &mut self,
        idx: usize,
        cfg: &crate::config::HazardConfig,
        killed: &mut Vec<usize>,
    ) {
        use crate::hazards::PredatorState;
        let predator = self.predators[idx].clone();
        // Dead spiders skip — respawn is handled in step 3.
        if matches!(predator.state, PredatorState::Dead { .. }) {
            return;
        }

        // Currently eating? Just tick down the timer.
        if let PredatorState::Eat { remaining_ticks } = predator.state {
            let next = remaining_ticks.saturating_sub(1);
            self.predators[idx].state = if next == 0 {
                PredatorState::Patrol
            } else {
                PredatorState::Eat { remaining_ticks: next }
            };
            return;
        }

        // Find nearest ant on the spider's module.
        let module = self.topology.module(predator.module_id);
        let mw = module.width() as f32;
        let mh = module.height() as f32;
        let mut nearest: Option<(f32, usize, u32)> = None;
        for (ai, ant) in self.ants.iter().enumerate() {
            if ant.module_id != predator.module_id || ant.is_in_transit() {
                continue;
            }
            if matches!(ant.caste, AntCaste::Queen) {
                continue;
            }
            let d2 = (ant.position - predator.position).length_squared();
            if nearest.map(|(bd, _, _)| d2 < bd).unwrap_or(true) {
                nearest = Some((d2, ai, ant.id));
            }
        }

        let sense_r2 = cfg.spider_sense_radius * cfg.spider_sense_radius;
        let mut new_state = predator.state;
        let mut new_pos = predator.position;
        let mut new_heading = predator.heading;

        match nearest {
            Some((d2, ai, aid)) if d2 <= sense_r2 => {
                // Ant detected in range → hunt mode.
                let delta = self.ants[ai].position - predator.position;
                let dist = delta.length().max(0.001);
                new_heading = delta.y.atan2(delta.x);
                new_state = PredatorState::Hunt { target_ant_id: aid };

                if dist <= 1.0 {
                    // Close enough to bite — eat the ant.
                    killed.push(ai);
                    new_state = PredatorState::Eat {
                        remaining_ticks: cfg.spider_eat_ticks.max(1),
                    };
                    tracing::info!(
                        spider = predator.id,
                        ant_id = aid,
                        "spider ate an ant"
                    );
                } else {
                    // Chase.
                    let step = cfg.spider_speed.min(dist);
                    new_pos = predator.position + (delta / dist) * step;
                }
            }
            _ => {
                // No target — patrol: small random-wander step.
                if matches!(predator.state, PredatorState::Patrol) {
                    let turn = self.rng.gen_range(-0.3f32..0.3);
                    new_heading = predator.heading + turn;
                } else {
                    // Just dropped Hunt (target gone) — revert to patrol.
                    new_state = PredatorState::Patrol;
                }
                let step = cfg.spider_speed * 0.5;
                new_pos = predator.position
                    + Vec2::new(new_heading.cos(), new_heading.sin()) * step;
            }
        }

        // Clamp to module bounds.
        new_pos.x = new_pos.x.clamp(0.5, mw - 0.5);
        new_pos.y = new_pos.y.clamp(0.5, mh - 0.5);

        let p = &mut self.predators[idx];
        p.position = new_pos;
        p.heading = new_heading;
        p.state = new_state;
    }

    fn antlion_tick(&mut self, idx: usize, killed: &mut Vec<usize>) {
        let p = &self.predators[idx];
        let pos = p.position;
        let mod_id = p.module_id;
        // Any non-queen ant on the antlion's grid cell dies.
        for (ai, ant) in self.ants.iter().enumerate() {
            if ant.module_id != mod_id || ant.is_in_transit() {
                continue;
            }
            if matches!(ant.caste, AntCaste::Queen) {
                continue;
            }
            if (ant.position - pos).length() <= 0.75 {
                killed.push(ai);
                tracing::info!(
                    antlion = p.id,
                    ant_id = ant.id,
                    "antlion claimed an ant"
                );
            }
        }
    }

    /// P6 weather: drive rain and lawnmower timers + apply effects.
    fn weather_tick(&mut self, cfg: &crate::config::HazardConfig) {
        // --- Rain ---
        if cfg.rain_period_ticks > 0 {
            let time_to_rain = self.tick
                .saturating_sub(self.weather.last_rain_start_tick)
                >= cfg.rain_period_ticks;
            let no_rain_yet = self.weather.last_rain_start_tick == 0 && self.weather.total_rain_events == 0;
            let should_start = self.weather.rain_ticks_remaining == 0
                && cfg.rain_duration_ticks > 0
                && (time_to_rain || no_rain_yet);
            if should_start && self.tick > cfg.rain_period_ticks.saturating_sub(1) {
                self.weather.rain_ticks_remaining = cfg.rain_duration_ticks;
                self.weather.last_rain_start_tick = self.tick;
                self.weather.total_rain_events += 1;
                tracing::warn!(
                    tick = self.tick,
                    duration = cfg.rain_duration_ticks,
                    "rain event triggered — surface pheromones clearing"
                );
            }
        }

        if self.weather.rain_ticks_remaining > 0 {
            self.weather.rain_ticks_remaining -= 1;
            // Wipe all pheromones on surface (non-UndergroundNest) modules.
            for m in self.topology.modules.iter_mut() {
                if m.kind != crate::module::ModuleKind::UndergroundNest {
                    for slice in [
                        &mut m.pheromones.food_trail,
                        &mut m.pheromones.home_trail,
                        &mut m.pheromones.alarm,
                    ] {
                        for v in slice.iter_mut() {
                            *v = 0.0;
                        }
                    }
                }
            }
            // Flood: any ant on the bottom row of any UndergroundNest
            // module takes damage.
            let dmg = cfg.rain_flood_damage;
            if dmg > 0.0 {
                for ant in self.ants.iter_mut() {
                    let module = self.topology.module(ant.module_id);
                    if module.kind != crate::module::ModuleKind::UndergroundNest {
                        continue;
                    }
                    // Bottom row = y < 1.0 in local cell-space.
                    if ant.position.y < 1.0 {
                        ant.health -= dmg;
                    }
                }
            }
        }

        // --- Lawnmower ---
        if cfg.lawnmower_period_ticks > 0 {
            let period = cfg.lawnmower_period_ticks;
            let active = self.weather.lawnmower_warning_remaining > 0
                || self.weather.lawnmower_sweep_remaining > 0;
            if !active
                && self.tick > 0
                && self.tick % period == 0
            {
                // Pick a surface module with at least one port.
                let surface_mods: Vec<ModuleId> = self
                    .topology
                    .modules
                    .iter()
                    .filter(|m| m.kind != crate::module::ModuleKind::UndergroundNest)
                    .map(|m| m.id)
                    .collect();
                if let Some(&mid) = surface_mods.first() {
                    self.weather.lawnmower_warning_remaining = cfg.lawnmower_warning_ticks;
                    self.weather.lawnmower_sweep_remaining = 0;
                    self.weather.lawnmower_module = mid;
                    self.weather.lawnmower_y = 0.0;
                    tracing::warn!(
                        tick = self.tick,
                        module = mid,
                        warning_ticks = cfg.lawnmower_warning_ticks,
                        "lawnmower warning — sweep incoming"
                    );
                }
            }

            if self.weather.lawnmower_warning_remaining > 0 {
                self.weather.lawnmower_warning_remaining -= 1;
                if self.weather.lawnmower_warning_remaining == 0 {
                    // Warning over — start sweeping.
                    let mid = self.weather.lawnmower_module;
                    let h = self
                        .topology
                        .try_module(mid)
                        .map(|m| m.height() as f32)
                        .unwrap_or(0.0);
                    let sweep_ticks = if cfg.lawnmower_speed > 0.0 {
                        (h / cfg.lawnmower_speed).ceil() as u32
                    } else {
                        0
                    };
                    self.weather.lawnmower_sweep_remaining = sweep_ticks;
                    self.weather.lawnmower_y = 0.0;
                    tracing::warn!(
                        module = mid,
                        sweep_ticks,
                        "lawnmower sweep started"
                    );
                }
            } else if self.weather.lawnmower_sweep_remaining > 0 {
                // Advance blade + kill any surface ant under it.
                let mid = self.weather.lawnmower_module;
                let half = cfg.lawnmower_half_width;
                let blade_y = self.weather.lawnmower_y;
                let mut kills: Vec<usize> = Vec::new();
                for (ai, ant) in self.ants.iter().enumerate() {
                    if ant.module_id != mid || ant.is_in_transit() {
                        continue;
                    }
                    if matches!(ant.caste, AntCaste::Queen) {
                        continue;
                    }
                    if (ant.position.y - blade_y).abs() <= half {
                        kills.push(ai);
                    }
                }
                // Apply kills (descending).
                kills.sort_unstable();
                for &i in kills.iter().rev() {
                    self.ants.swap_remove(i);
                }
                self.weather.total_mower_kills += kills.len() as u32;
                self.weather.lawnmower_y += cfg.lawnmower_speed;
                self.weather.lawnmower_sweep_remaining -= 1;
                if self.weather.lawnmower_sweep_remaining == 0 {
                    tracing::warn!(
                        module = mid,
                        total_kills = self.weather.total_mower_kills,
                        "lawnmower sweep complete"
                    );
                }
            }
        }
    }

    /// Surface↔underground traversal. Any ant standing on a
    /// `NestEntrance(colony_id)` cell teleports to the matching entrance
    /// on the linked layer (surface ↔ underground) for its colony. This
    /// is what lets workers descend to dig, then return surfaceward to
    /// drop kickout pellets. Real ants traverse in seconds via the
    /// physical entrance hole; the sim simplifies to a teleport since
    /// modeling the literal hole as a 1-tile bottleneck would create
    /// pathological queuing.
    fn surface_underground_traversal(&mut self) {
        // Snapshot per-colony entrances so we can mutate ants while
        // reading topology. Layout: HashMap<colony_id, (surface_mod, surface_pos, ug_mod, ug_pos)>.
        let mut entrance_pairs: Vec<(u8, ModuleId, Vec2, ModuleId, Vec2)> = Vec::new();
        for c in &self.colonies {
            let cid = c.id;
            let Some(surf_mod) = self.topology.surface_nest_for_colony(cid) else { continue };
            let Some(ug_mod) = self.topology.underground_for_colony(cid) else { continue };
            let surf = self.topology.module(surf_mod);
            let Some((sx, sy)) = surf.world.find_nest_entrance(cid) else { continue };
            let ug = self.topology.module(ug_mod);
            let Some((ux, uy)) = ug.world.find_nest_entrance(cid) else { continue };
            // Center the teleport position on the cell.
            let surf_pos = Vec2::new(sx as f32 + 0.5, sy as f32 + 0.5);
            let ug_pos = Vec2::new(ux as f32 + 0.5, uy as f32 + 0.5);
            entrance_pairs.push((cid, surf_mod, surf_pos, ug_mod, ug_pos));
        }
        if entrance_pairs.is_empty() {
            return;
        }

        for ant in self.ants.iter_mut() {
            if ant.is_in_transit() {
                continue;
            }
            // Queens stay put — they don't traverse layers in real
            // biology either; the queen sits in the queen chamber on
            // the underground side once founding is complete.
            if ant.caste == AntCaste::Queen {
                continue;
            }
            let Some(pair) = entrance_pairs.iter().find(|p| p.0 == ant.colony_id) else { continue };
            let (_, surf_mod, surf_pos, ug_mod, ug_pos) = *pair;
            // Only fire the teleport when the ant arrives on the
            // entrance cell — a state_timer-based debounce is overkill;
            // we use the carrying_soil + state heuristic below.
            //
            // Surface-side teleport (descend): ant on surface entrance
            // cell whose state suggests it wants to be inside (Idle,
            // Exploring, Digging without a target, or carrying food
            // for storage in an underground chamber).
            //
            // For MVP: surface→underground only when ant.state == Digging
            // (it WANTS to dig but is on the surface).
            // Underground→surface only when ant.carrying_soil is true
            // (it MUST surface to dump the pellet).
            if ant.module_id == surf_mod {
                let (gx, gy) = self.topology.module(surf_mod).world.world_to_grid(ant.position);
                if (gx, gy) == (surf_pos.x as i64, surf_pos.y as i64)
                    && ant.state == AntState::Digging
                {
                    ant.module_id = ug_mod;
                    ant.position = ug_pos;
                    ant.dig_progress = 0;
                    ant.dig_target = None;
                    continue;
                }
            }
            if ant.module_id == ug_mod {
                let (gx, gy) = self.topology.module(ug_mod).world.world_to_grid(ant.position);
                if (gx, gy) == (ug_pos.x as i64, ug_pos.y as i64) && ant.carrying_soil {
                    ant.module_id = surf_mod;
                    ant.position = surf_pos;
                }
            }
        }
    }

    /// Promote idle workers to `Digging` based on the colony's
    /// `behavior_weights.dig`. Real ants don't get assigned tasks by a
    /// foreman; they sense local task demand (more brood = nurse, dig
    /// face exposed = excavate) and self-recruit. This is a coarse
    /// approximation for now: at low population each tick rolls a
    /// fraction of idle workers into Digging if they're on a module
    /// with Solid neighbors. Future: use the dig-priority pheromone
    /// (parked in `docs/digging-design.md` Phase C) for spatial bias.
    fn assign_diggers(&mut self) {
        // Snapshot per-colony dig weight + which modules have at least
        // one Solid cell — those are the only modules where promotion
        // makes sense (no point promoting an ant on an outworld where
        // there's nothing to dig).
        let mut dig_weight_by_colony: Vec<(u8, f32)> =
            self.colonies.iter().map(|c| (c.id, c.behavior_weights.dig)).collect();

        // Only promote on modules that have Solid cells (the underground
        // nests). Cheap precomputed bitmap: module_id → has_solid.
        let mut has_solid: Vec<(ModuleId, bool)> = self
            .topology
            .modules
            .iter()
            .map(|m| (m.id, m.world.cells.iter().any(|c| matches!(c, Terrain::Solid))))
            .collect();

        for ant in self.ants.iter_mut() {
            if ant.is_in_transit() || ant.caste != AntCaste::Worker || ant.carrying_soil {
                continue;
            }
            // Only idle/exploring workers are eligible — don't yank a
            // returning forager off her trail.
            if !matches!(ant.state, AntState::Idle | AntState::Exploring) {
                continue;
            }
            let Some((_, w)) = dig_weight_by_colony.iter().find(|p| p.0 == ant.colony_id) else { continue };
            let Some((_, on_solid_module)) = has_solid.iter().find(|p| p.0 == ant.module_id) else { continue };
            if !*on_solid_module {
                continue;
            }
            // Per-substep promotion probability scales with dig weight.
            // Clamp upper bound so an entire colony doesn't all flip in
            // one substep at high dig weight.
            let p = (*w * 0.05).min(0.05);
            if self.rng.r#gen::<f32>() < p {
                ant.state = AntState::Digging;
                ant.dig_progress = 0;
                ant.dig_target = None;
            }
        }
        let _ = (&mut dig_weight_by_colony, &mut has_solid); // suppress unused-mut-warnings
    }

    /// Multi-substep excavation. Diggers accumulate `dig_progress` per
    /// substep against an adjacent `Terrain::Solid` cell; when progress
    /// crosses the species-tunable threshold, the tile flips to Empty
    /// and the ant gains a `carrying_soil` pellet. Replaces the prior
    /// instant-flip behavior (which made digging invisible at any time
    /// scale). See docs/biology.md "Excavation rate is slow" for the
    /// biological grounding (1-2 cm³/day for a small Lasius colony).
    ///
    /// Pellet pickup → drop cycle:
    /// 1. Ant in `Digging` adjacent to `Solid` → progress accumulates
    /// 2. Threshold crossed → tile flips Empty + ant.carrying_soil = true
    /// 3. Ant exits Digging, FSM steers it back to nest entrance
    /// 4. At a `NestEntrance` cell, drop pellet → adjacent cell becomes
    ///    or grows `SoilPile` (the kickout mound), ant.carrying_soil = false
    fn dig_tick(&mut self) {
        // Per-species threshold. 60 substeps ≈ 2 in-game seconds at
        // calibrated rate — fast enough to be visible, slow enough to
        // feel like real work. Future: scale by an `appearance.dig_speed_multiplier`.
        const DIG_PROGRESS_THRESHOLD: u32 = 60;
        const KICKOUT_MAX_INTENSITY: u32 = 255;

        // Pass 1: Diggers — accumulate progress, flip tiles, pick up pellets.
        let mut flips: Vec<(usize, ModuleId, usize, usize)> = Vec::new(); // (ant_idx, module, x, y)
        for (i, ant) in self.ants.iter_mut().enumerate() {
            if ant.state != AntState::Digging || ant.is_in_transit() {
                ant.dig_progress = 0;
                ant.dig_target = None;
                continue;
            }
            // Skip if already carrying — can't dig with a pellet.
            if ant.carrying_soil {
                continue;
            }
            let module = self.topology.module(ant.module_id);
            let (gx, gy) = module.world.world_to_grid(ant.position);
            if !module.world.in_bounds(gx, gy) {
                continue;
            }
            let (ux, uy) = (gx as usize, gy as usize);
            // 4-neighbor lookup for a Solid target (deterministic order:
            // east, west, south, north — matches surface→underground
            // entrance bias in `attach_underground`).
            let candidates = [
                (ux + 1, uy),
                (ux.wrapping_sub(1), uy),
                (ux, uy + 1),
                (ux, uy.wrapping_sub(1)),
            ];
            let mut chosen: Option<(usize, usize)> = None;
            for (nx, ny) in candidates {
                if nx < module.world.width
                    && ny < module.world.height
                    && module.world.get(nx, ny) == Terrain::Solid
                {
                    chosen = Some((nx, ny));
                    break;
                }
            }
            let Some((tx, ty)) = chosen else {
                ant.dig_progress = 0;
                ant.dig_target = None;
                continue;
            };
            // If the target changed (ant shuffled), reset progress.
            let target_pair = (tx as u16, ty as u16);
            if ant.dig_target != Some(target_pair) {
                ant.dig_progress = 0;
                ant.dig_target = Some(target_pair);
            }
            ant.dig_progress = ant.dig_progress.saturating_add(1);
            if ant.dig_progress >= DIG_PROGRESS_THRESHOLD {
                flips.push((i, ant.module_id, tx, ty));
            }
        }
        if !flips.is_empty() {
            for (ant_idx, mid, x, y) in &flips {
                // Flip the tile.
                self.topology.module_mut(*mid).world.set(*x, *y, Terrain::Empty);
                let ant = &mut self.ants[*ant_idx];
                ant.carrying_soil = true;
                ant.dig_progress = 0;
                ant.dig_target = None;
                // Transition out of Digging — FSM picks up where to go
                // next; movement steering will pull them home.
                ant.state = AntState::ReturningHome;
            }
            tracing::debug!(excavated = flips.len(), "dig_tick: tile flips this substep");
        }

        // Pass 2: Diggers carrying pellets — drop at nest entrance, build
        // the kickout mound.
        let mut drops: Vec<(usize, ModuleId, usize, usize)> = Vec::new();
        for (i, ant) in self.ants.iter().enumerate() {
            if !ant.carrying_soil || ant.is_in_transit() {
                continue;
            }
            let module = self.topology.module(ant.module_id);
            let (gx, gy) = module.world.world_to_grid(ant.position);
            if !module.world.in_bounds(gx, gy) {
                continue;
            }
            let (ux, uy) = (gx as usize, gy as usize);
            // If standing ON a NestEntrance cell, drop the pellet on the
            // best adjacent walkable cell (prefer SoilPile; else Empty).
            // Real ants drop a body-length OUTSIDE the entrance hole.
            if matches!(module.world.get(ux, uy), Terrain::NestEntrance(_)) {
                let candidates = [
                    (ux + 1, uy),
                    (ux.wrapping_sub(1), uy),
                    (ux, uy + 1),
                    (ux, uy.wrapping_sub(1)),
                ];
                // Prefer growing an existing pile.
                let mut target: Option<(usize, usize)> = None;
                for (nx, ny) in candidates {
                    if nx < module.world.width
                        && ny < module.world.height
                        && matches!(module.world.get(nx, ny), Terrain::SoilPile(_))
                    {
                        target = Some((nx, ny));
                        break;
                    }
                }
                // Else seed a new pile on the first Empty neighbor.
                if target.is_none() {
                    for (nx, ny) in candidates {
                        if nx < module.world.width
                            && ny < module.world.height
                            && module.world.get(nx, ny) == Terrain::Empty
                        {
                            target = Some((nx, ny));
                            break;
                        }
                    }
                }
                if let Some((dx, dy)) = target {
                    drops.push((i, ant.module_id, dx, dy));
                }
            }
        }
        for (ant_idx, mid, x, y) in drops {
            let module = self.topology.module_mut(mid);
            let new_intensity = match module.world.get(x, y) {
                Terrain::SoilPile(n) => (n + 1).min(KICKOUT_MAX_INTENSITY),
                _ => 1,
            };
            module.world.set(x, y, Terrain::SoilPile(new_intensity));
            self.ants[ant_idx].carrying_soil = false;
            // Carrier returns to looking for work.
            self.ants[ant_idx].state = AntState::Exploring;
        }
    }

    /// P4 territory: each non-transit, non-diapause ant leaves a small
    /// signed mark on its cell's `ColonyScent` layer. Sign is determined
    /// by `colony_id` (0 = positive, any other = negative). Combined
    /// with the existing evaporate + diffuse, this produces smooth
    /// territory blobs that shrink when a colony pulls out of an area.
    fn territory_deposit_tick(&mut self) {
        const DEPOSIT_AMOUNT: f32 = 0.08;
        let cap = self.config.pheromone.max_intensity;
        for ant in &self.ants {
            if ant.is_in_transit() || ant.state == AntState::Diapause {
                continue;
            }
            let module = self.topology.module(ant.module_id);
            let (gx, gy) = module.pheromones.world_to_grid(ant.position);
            if !module.pheromones.in_bounds(gx, gy) {
                continue;
            }
            let (ux, uy) = (gx as usize, gy as usize);
            self.topology.module_mut(ant.module_id).pheromones.deposit_territory(
                ux,
                uy,
                ant.colony_id,
                DEPOSIT_AMOUNT,
                cap,
            );
        }
    }

    /// P4 Avenger.
    ///
    /// Each AI colony keeps exactly one avenger at any time. The avenger's
    /// heading is overridden each tick to point at the nearest enemy ant
    /// on its module. If the avenger is gone (combat-killed) the role
    /// transfers to a random surviving non-queen colony-mate.
    ///
    /// The avenger's *state* is left alone — the normal FSM still runs —
    /// so a healthy avenger still wanders via ACO when no enemy is in
    /// sight. This keeps trail laying + food return intact.
    pub fn avenger_tick(&mut self) {
        for cid in 0..self.colonies.len() {
            if !self.colonies[cid].is_ai_controlled {
                continue;
            }
            let colony_id = self.colonies[cid].id;
            // Ensure exactly one avenger exists. If none, promote.
            let avenger_idx = self
                .ants
                .iter()
                .position(|a| a.colony_id == colony_id && a.is_avenger);
            let avenger_idx = match avenger_idx {
                Some(i) => i,
                None => {
                    // No avenger — promote a random surviving non-queen.
                    let candidates: Vec<usize> = self
                        .ants
                        .iter()
                        .enumerate()
                        .filter(|(_, a)| {
                            a.colony_id == colony_id
                                && !matches!(a.caste, AntCaste::Queen)
                                && !a.is_in_transit()
                        })
                        .map(|(i, _)| i)
                        .collect();
                    if candidates.is_empty() {
                        continue;
                    }
                    let pick_idx = self.rng.gen_range(0..candidates.len());
                    let idx = candidates[pick_idx];
                    self.ants[idx].is_avenger = true;
                    tracing::info!(
                        colony = colony_id,
                        ant = self.ants[idx].id,
                        "avenger role transferred"
                    );
                    idx
                }
            };

            // Find nearest enemy ant on the avenger's module.
            let avenger_pos = self.ants[avenger_idx].position;
            let avenger_mod = self.ants[avenger_idx].module_id;
            let mut best: Option<(f32, Vec2)> = None;
            for a in &self.ants {
                if a.colony_id == colony_id
                    || a.module_id != avenger_mod
                    || a.is_in_transit()
                    || matches!(a.caste, AntCaste::Queen)
                {
                    continue;
                }
                let d2 = (a.position - avenger_pos).length_squared();
                if best.map(|(bd, _)| d2 < bd).unwrap_or(true) {
                    best = Some((d2, a.position));
                }
            }
            if let Some((_, target)) = best {
                let delta = target - avenger_pos;
                if delta.length_squared() > 1e-6 {
                    self.ants[avenger_idx].heading = delta.y.atan2(delta.x);
                }
            }
        }
    }

    /// Phase 4: simple red-colony AI. For any colony flagged
    /// `is_ai_controlled`, nudge its `caste_ratio` toward soldiers when
    /// it's taking casualties and nudge `behavior_weights` toward forage
    /// when food is low.
    ///
    /// Called once per tick; clears `combat_losses_this_tick` for every
    /// colony at the end (AI or not).
    pub fn red_ai_tick(&mut self) {
        let low_food = self.config.colony.egg_cost * 4.0;
        for c in self.colonies.iter_mut() {
            if c.is_ai_controlled {
                // Taking losses → shift toward soldiers (cap 0.5).
                if c.combat_losses_this_tick > 0 {
                    let shift = 0.01 * c.combat_losses_this_tick as f32;
                    let target_soldier = (c.caste_ratio.soldier + shift).min(0.5);
                    let delta = target_soldier - c.caste_ratio.soldier;
                    c.caste_ratio.soldier = target_soldier;
                    // Take it out of the worker share; leave breeders alone.
                    c.caste_ratio.worker = (c.caste_ratio.worker - delta).max(0.05);
                    tracing::debug!(
                        colony = c.id,
                        soldier = c.caste_ratio.soldier,
                        worker = c.caste_ratio.worker,
                        losses_this_tick = c.combat_losses_this_tick,
                        "red AI: escalated soldier ratio"
                    );
                }
                // Low food → all hands foraging.
                if c.food_stored < low_food {
                    let shift = 0.02;
                    let target_forage = (c.behavior_weights.forage + shift).min(0.9);
                    let delta = target_forage - c.behavior_weights.forage;
                    c.behavior_weights.forage = target_forage;
                    c.behavior_weights.nurse = (c.behavior_weights.nurse - delta * 0.5).max(0.05);
                    c.behavior_weights.dig = (c.behavior_weights.dig - delta * 0.5).max(0.02);
                }
            }
            c.combat_losses_this_tick = 0;
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
        // Per-module thermal relaxation parallelized — each module's
        // temperature grid is independent.
        self.topology.modules.par_iter_mut().for_each(|module| {
            let target = match module.kind {
                ModuleKind::HeatChamber => 28.0,
                ModuleKind::HibernationChamber => 5.0,
                _ => ambient,
            };
            module.ambient_target = target;
            for v in module.temperature.iter_mut() {
                *v += (target - *v) * TEMP_DRIFT_RATE;
            }
        });

        if self.tick % 8 == 0 {
            self.topology.modules.par_iter_mut().for_each(|module| {
                let w = module.width();
                let h = module.height();
                diffuse_scalar_grid(&mut module.temperature, w, h, 0.1);
            });
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
                // Boot safety: at the year 0 → year 1 rollover, only
                // check the diapause gate if year 0 was a FULL year
                // (i.e. `starting_day_of_year == 0`). The production
                // default of `starting_day_of_year = 150` (mid-spring)
                // means year 0 only spans ~215 days before rollover --
                // the cold window inside that partial year is ~67 days
                // at best, and nest-cell thermal drift typically pushes
                // the actual diapause-day count below the 60-day
                // threshold. Suppressing fertility for year 1 on a
                // partial year 0 is what caused the long-running
                // collapse: queen never lays an egg the entire second
                // year of play, workers attrit, colony dies.
                //
                // After year 1 → year 2 onward we always check, since
                // by then we've had a full year to accumulate diapause
                // days against the threshold.
                let partial_first_year = self.climate.starting_day_of_year != 0;
                if colony.last_year_evaluated == 0
                    && current_year == 1
                    && partial_first_year
                {
                    tracing::info!(
                        colony_id = colony.id,
                        diapause_days = colony.days_in_diapause_this_year,
                        starting_day = self.climate.starting_day_of_year,
                        "year 0 → 1: boot rollover skipped fertility check (partial first year)"
                    );
                    colony.fertility_suppressed = false;
                } else if colony.last_year_evaluated == 0 && current_year == 1 {
                    // Full first year — apply the same gate as later years.
                    if hibernation_required {
                        let required = if self.config.ant.min_diapause_days == 0 {
                            DEFAULT_MIN_DIAPAUSE_DAYS
                        } else {
                            self.config.ant.min_diapause_days
                        };
                        let ok = colony.days_in_diapause_this_year >= required;
                        let newly_suppressed = !ok && !colony.fertility_suppressed;
                        colony.fertility_suppressed = !ok;
                        if newly_suppressed {
                            tracing::info!(
                                colony_id = colony.id,
                                year = current_year,
                                diapause_days = colony.days_in_diapause_this_year,
                                required,
                                "missed winter — queen fertility suppressed"
                            );
                        }
                    } else {
                        colony.fertility_suppressed = false;
                    }
                } else if hibernation_required {
                    let required = if self.config.ant.min_diapause_days == 0 {
                        DEFAULT_MIN_DIAPAUSE_DAYS
                    } else {
                        self.config.ant.min_diapause_days
                    };
                    let ok = colony.days_in_diapause_this_year >= required;
                    let newly_suppressed = !ok && !colony.fertility_suppressed;
                    colony.fertility_suppressed = !ok;
                    if newly_suppressed {
                        tracing::info!(
                            colony_id = colony.id,
                            year = current_year,
                            diapause_days = colony.days_in_diapause_this_year,
                            required,
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
            // Diapause metabolic depression — ants in diapause have
            // dramatically reduced metabolism (~5-10% of normal in real
            // biology; see Hahn & Denlinger 2007 for general insect
            // diapause energetics). Without this scaling, a hibernating
            // colony continues burning food at full rate while unable
            // to forage, exhausts brood via cannibalism in a few outer
            // ticks, then collapses via the 5%/tick starvation cap.
            // Pre-fix this killed any Lasius colony that grew large
            // enough to need significant food before its first winter.
            const DIAPAUSE_METABOLIC_DEPRESSION: f32 = 0.10;
            let metabolic_factor = if in_diapause {
                DIAPAUSE_METABOLIC_DEPRESSION
            } else {
                1.0
            };
            let consumption = ((worker_breeder_cnt as f32) * ccfg.adult_food_consumption
                + (soldier_cnt as f32)
                    * ccfg.adult_food_consumption
                    * ccfg.soldier_food_multiplier)
                * metabolic_factor;
            colony.food_stored -= consumption;
            // Decay the food-inflow running average toward zero — half
            // life ~100 ticks. A colony that stopped delivering food
            // sees its running average fade, which in turn throttles
            // the queen (see biology.md → food-inflow throttle).
            colony.food_inflow_recent *= 0.993;

            let mut starve_count: u32 = 0;
            if colony.food_stored < 0.0 {
                // P7+ biology: before killing adults, cannibalize
                // brood if the tech is unlocked. Eggs first, then
                // larvae, then pupae — younger brood has less nutrient
                // invested, so cost-of-recovery is lowest.
                //
                // Recovery factors approximate real literature: eggs
                // and young larvae have near-complete protein recovery
                // when digested; older pupae have already put their
                // nutrients into structural tissue and give back less.
                if colony.has_tech(crate::colony::TechUnlock::BroodCannibalism) {
                    let deficit_before = -colony.food_stored;
                    let mut recovered = 0.0f32;
                    // Sort brood so we consume the earliest stages first.
                    // Using swap_remove on the Vec; iterate safely by
                    // collecting indices first.
                    let mut idx_by_priority: Vec<(u8, usize)> = colony
                        .brood
                        .iter()
                        .enumerate()
                        .map(|(i, b)| {
                            let pri = match b.stage {
                                crate::colony::BroodStage::Egg => 0u8,
                                crate::colony::BroodStage::Larva => 1,
                                crate::colony::BroodStage::Pupa => 2,
                            };
                            (pri, i)
                        })
                        .collect();
                    idx_by_priority.sort_by_key(|(p, _)| *p);
                    // Consume one-by-one until deficit covered.
                    let mut to_remove: Vec<usize> = Vec::new();
                    for (pri, idx) in idx_by_priority {
                        if recovered >= deficit_before {
                            break;
                        }
                        let recovery_factor = match pri {
                            0 => 0.90, // egg
                            1 => 0.80, // larva
                            _ => 0.65, // pupa
                        };
                        let recovered_here = ccfg.egg_cost * recovery_factor;
                        recovered += recovered_here;
                        to_remove.push(idx);
                        match pri {
                            0 => {
                                if colony.eggs > 0 {
                                    colony.eggs -= 1;
                                }
                            }
                            1 => {
                                if colony.larvae > 0 {
                                    colony.larvae -= 1;
                                }
                            }
                            _ => {
                                if colony.pupae > 0 {
                                    colony.pupae -= 1;
                                }
                            }
                        }
                    }
                    // Remove in descending-index order for swap_remove safety.
                    to_remove.sort_unstable();
                    for i in to_remove.iter().rev() {
                        colony.brood.swap_remove(*i);
                    }
                    if recovered > 0.0 {
                        colony.food_stored += recovered;
                        tracing::info!(
                            colony_id = colony.id,
                            recovered,
                            brood_consumed = to_remove.len(),
                            "brood cannibalism — adults spared"
                        );
                    }
                }
                // Any remaining deficit falls through to worker
                // starvation (capped per tick to avoid a single-tick
                // colony wipe).
                if colony.food_stored < 0.0 {
                    let deficit = -colony.food_stored;
                    let cost = ccfg.adult_food_consumption.max(1e-6);
                    let raw = (deficit / cost).ceil() as u32;
                    let cap = ((adult_total as f32 * 0.05).ceil() as u32).max(1);
                    let mut deaths = raw.min(cap);
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
                            raw,
                            "starvation deaths (capped per tick; brood exhausted)"
                        );
                    }
                }
            }
            starve.push((colony.id, starve_count));

            let queen_alive = colony.queen_health > 0.0;
            if !queen_alive && colony.queen_alive_last_tick {
                tracing::info!(colony_id = colony.id, "queen died — egg production halted");
            }
            colony.queen_alive_last_tick = queen_alive;

            // P7+ biology: queen lay rate is throttled by recent food
            // inflow when the tech is unlocked. Models the vitellogenin
            // pipeline — queens physiologically can't lay faster than
            // their protein pipeline supports. See biology.md →
            // "Queen egg-laying is throttled by recent food intake".
            //
            // throttle = clamp(inflow / (consumption * 2), FLOOR, 1).
            // Biologically: the FLOOR of ~0.2 mirrors endogenous
            // reserves — even a starving queen lays a trickle of eggs
            // from her own metabolized tissue (wing muscle catabolism
            // in founding queens, stored fat in established queens).
            // See biology.md → "Claustral vs semi-claustral founding".
            let throttle = if colony.has_tech(crate::colony::TechUnlock::FoodInflowThrottle) {
                const ENDOGENOUS_FLOOR: f32 = 0.2;
                let baseline = (consumption * 2.0).max(1e-4);
                (colony.food_inflow_recent / baseline).clamp(ENDOGENOUS_FLOOR, 1.0)
            } else {
                1.0
            };
            // Population-saturation soft cap. As adult_total approaches the
            // species target_population, the queen scales lay rate down.
            // Beyond 1.5× target she stops laying. Models established-colony
            // lay-rate decline (Hölldobler & Wilson 1990 — colony-size-
            // dependent fertility regulation). Without this, the food-
            // inflow throttle's 0.2 floor lets the queen lay past the
            // colony's foraging support, the colony overshoots, runs
            // chronic deficit, and dies the first or second winter.
            // Verified empirically: 5/7 species in the 2y validation
            // sweep collapsed exactly this way.
            let target = ccfg.target_population.max(1) as f32;
            let pop = adult_total as f32;
            let saturation = if pop <= target * 0.5 {
                1.0
            } else if pop >= target * 1.5 {
                0.0
            } else {
                // Linear ramp from full rate at 0.5× target down to 0 at 1.5× target.
                let t = (pop - target * 0.5) / target;
                (1.0 - t).clamp(0.0, 1.0)
            };
            let effective_egg_rate = ccfg.queen_egg_rate * throttle * saturation;

            // P7+ biology: trophic eggs — queen converts some stored
            // food into "free" food packets deposited directly into
            // storage, modelling the real-biology nutritive-egg pathway.
            // Rate is ~10% of the regular egg rate, always on while the
            // queen is alive and has any food.
            if queen_alive
                && colony.food_stored > 0.5
                && colony.has_tech(crate::colony::TechUnlock::TrophicEggs)
            {
                const TROPHIC_RATE: f32 = 0.1; // relative to queen_egg_rate
                const TROPHIC_YIELD: f32 = 0.4; // food returned per trophic egg
                const TROPHIC_COST: f32 = 0.2; // food consumed to produce one
                let trophic_attempt = ccfg.queen_egg_rate * TROPHIC_RATE;
                // Fold into an accumulator on food_stored directly for
                // simplicity (no new state field). Expected net effect
                // per tick: +0.1 * 0.05 * (0.4 - 0.2) = ~0.001 food/tick
                // at default rates — a small but real background income.
                if colony.food_stored >= TROPHIC_COST {
                    colony.food_stored += trophic_attempt * (TROPHIC_YIELD - TROPHIC_COST);
                }
            }

            if queen_alive && !colony.fertility_suppressed && colony.food_stored >= ccfg.egg_cost {
                colony.egg_accumulator += effective_egg_rate;
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
                        // Egg stage: age until it matures into a larva.
                        if b.age >= ccfg.egg_stage_ticks {
                            b.stage = BroodStage::Larva;
                            b.age = 0;
                            if colony.eggs > 0 {
                                colony.eggs -= 1;
                            }
                            colony.larvae += 1;
                        }
                    }
                    BroodStage::Larva => {
                        // Larva stage: age until it matures into a pupa.
                        if b.age >= ccfg.larva_stage_ticks {
                            b.stage = BroodStage::Pupa;
                            b.age = 0;
                            if colony.larvae > 0 {
                                colony.larvae -= 1;
                            }
                            colony.pupae += 1;
                        }
                    }
                    BroodStage::Pupa => {
                        // Pupa stage: age until it ecloses into an adult.
                        if b.age >= ccfg.pupa_stage_ticks {
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

/// P4 alarm-pheromone steering.
///
/// Returns `Some(heading)` when the ant's local alarm field is strong
/// enough to override its normal ACO steering, or `None` to leave the
/// default heading in place. Soldiers steer toward the strongest alarm
/// cell in their sensing cone (converging on the fight). Workers and
/// breeders pick the heading that best points AWAY from the mean alarm
/// source (flight). Queens are never moved by alarm (they're usually
/// Idle and don't walk anyway).
fn alarm_response_heading(
    ant: &Ant,
    module: &crate::module::Module,
    ant_cfg: &crate::config::AntConfig,
    pher_cfg: &crate::config::PheromoneConfig,
) -> Option<f32> {
    use crate::ant::AntCaste;
    if matches!(ant.caste, AntCaste::Queen) {
        return None;
    }
    let samples = module.pheromones.sample_cone(
        ant.position,
        ant.heading,
        ant_cfg.sense_angle.to_radians(),
        ant_cfg.sense_radius as f32,
        PheromoneLayer::Alarm,
    );
    // Pick the strongest sample as the alarm source.
    let mut best: Option<(Vec2, f32)> = None;
    let mut total = 0.0f32;
    for (cell, intensity) in &samples {
        total += intensity;
        if best.map(|(_, bi)| *intensity > bi).unwrap_or(true) {
            best = Some((*cell, *intensity));
        }
    }
    let (src, peak) = best?;
    // Ignore faint background alarm (close to the evap floor).
    let trigger = (pher_cfg.min_threshold * 8.0).max(0.1);
    if peak < trigger || total < trigger {
        return None;
    }
    let delta = src - ant.position;
    if delta.length_squared() < 1e-6 {
        return None;
    }
    let toward = delta.y.atan2(delta.x);
    match ant.caste {
        AntCaste::Soldier => Some(toward),
        AntCaste::Worker | AntCaste::Breeder => Some(toward + std::f32::consts::PI),
        AntCaste::Queen => None,
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
        is_avenger: false,
        is_player: false,
        follow_leader: None,
        dig_progress: 0,
        dig_target: None,
        carrying_soil: false,
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
        cfg.colony.egg_stage_ticks = 100;
        cfg.colony.larva_stage_ticks = 100;
        cfg.colony.pupa_stage_ticks = 100;
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
        cfg.colony.egg_stage_ticks = 10;
        cfg.colony.larva_stage_ticks = 10;
        cfg.colony.pupa_stage_ticks = 10;
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
        //
        // Force `starting_day_of_year = 0` so year 0 is a FULL year and
        // the year 0→1 rollover applies the diapause check directly
        // (the production default of 150 makes year 0 partial, in which
        // case we deliberately skip the boot check — see colony_economy_tick).
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
        sim.climate.starting_day_of_year = 0; // full first year
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
    fn fertility_not_suppressed_on_partial_first_year() {
        // Regression test for the year-1 collapse bug: a colony with the
        // default `starting_day_of_year=150` (mid-spring) only has ~215
        // days before the year 0→1 rollover. The pre-fix behavior was to
        // apply the 60-day diapause check at that rollover, which often
        // failed because nest-cell thermal drift kept the colony just
        // below the threshold even when the climate did go cold. Result
        // was queen fertility suppressed for an entire year, no eggs
        // laid, workers attrited, colony collapsed by year 1.
        //
        // Post-fix: the year 0→1 rollover skips the diapause check when
        // `starting_day_of_year != 0` (partial first year). This test
        // pins that behavior so we don't regress.
        let mut cfg = small_config();
        cfg.ant.hibernation_required = true;
        cfg.ant.hibernation_cold_threshold_c = 10.0;
        cfg.ant.hibernation_warm_threshold_c = 12.0;
        cfg.colony.queen_egg_rate = 0.0;
        cfg.colony.adult_food_consumption = 0.0;
        let mut sim = Simulation::new(cfg, 7);
        // Always-warm climate so no diapause days accumulate.
        sim.climate.seasonal_mid_c = 25.0;
        sim.climate.seasonal_amplitude_c = 0.0;
        sim.climate.starting_day_of_year = 150; // partial first year (default)
        sim.in_game_seconds_per_tick = 86_400.0 / 2.0;
        // Run past the year 0→1 rollover. With starting_day=150,
        // 215 in-game days = 430 ticks. Run 500 to give a margin.
        sim.run(500);
        assert!(
            !sim.colonies[0].fertility_suppressed,
            "fertility was suppressed on a partial first year (days_in_diapause={}, year={})",
            sim.colonies[0].days_in_diapause_this_year,
            sim.in_game_year(),
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

    // ===================== Phase 4 combat tests =====================

    fn two_colony_sim_for_combat() -> Simulation {
        let mut cfg = small_config();
        // Small initial spawn — needed so the Avenger role has a candidate.
        cfg.ant.initial_count = 3;
        cfg.combat.interaction_radius = 1.5;
        let topology = Topology::two_colony_arena((24, 24), (32, 32));
        // Module 0 = black nest, 1 = outworld, 2 = red nest.
        Simulation::new_two_colony_with_topology(cfg, topology, 7, 0, 2)
    }

    fn place_combatant(
        sim: &mut Simulation,
        id: u32,
        colony: u8,
        pos: Vec2,
        caste: AntCaste,
        health: f32,
    ) {
        let mut a = Ant::new_with_caste(id, colony, pos, 0.0, health, caste);
        a.module_id = 1; // outworld
        sim.ants.push(a);
    }

    #[test]
    fn two_colony_arena_starter_builds() {
        let sim = two_colony_sim_for_combat();
        assert_eq!(sim.topology.modules.len(), 3);
        assert_eq!(sim.topology.tubes.len(), 2);
        assert_eq!(sim.colonies.len(), 2);
        assert!(!sim.colonies[0].is_ai_controlled);
        assert!(sim.colonies[1].is_ai_controlled);
    }

    #[test]
    fn cross_colony_combat_kills_ants() {
        let mut sim = two_colony_sim_for_combat();
        let initial = sim.ants.len();

        // Two black workers vs one red soldier, standing toe-to-toe.
        place_combatant(&mut sim, 9001, 0, Vec2::new(16.0, 16.0), AntCaste::Worker, 5.0);
        place_combatant(&mut sim, 9002, 0, Vec2::new(16.5, 16.0), AntCaste::Worker, 5.0);
        place_combatant(&mut sim, 9003, 1, Vec2::new(16.25, 16.25), AntCaste::Soldier, 25.0);

        // Combat tick directly — bypass FSM/movement.
        for _ in 0..6 {
            sim.combat_tick();
        }

        let losses: u32 = sim.colonies.iter().map(|c| c.combat_losses).sum();
        assert!(losses > 0, "expected combat to kill at least one ant");
        assert!(
            sim.ants.len() < initial + 3,
            "at least one combatant should have died (initial+3={})",
            initial + 3
        );
    }

    #[test]
    fn combat_death_drops_food_and_alarm() {
        let mut sim = two_colony_sim_for_combat();
        place_combatant(&mut sim, 9101, 0, Vec2::new(10.0, 10.0), AntCaste::Worker, 1.0);
        place_combatant(&mut sim, 9102, 1, Vec2::new(10.0, 10.0), AntCaste::Soldier, 25.0);

        sim.combat_tick();
        sim.combat_tick();

        // One of the two must have died (soldier outhits worker).
        assert!(
            sim.ants.iter().find(|a| a.id == 9101).is_none(),
            "weak black worker should be dead"
        );
        let module = sim.topology.module(1);
        let (gx, gy) = module.world.world_to_grid(Vec2::new(10.0, 10.0));
        assert!(module.world.in_bounds(gx, gy));
        let cell = module.world.get(gx as usize, gy as usize);
        assert!(
            matches!(cell, Terrain::Food(_)),
            "corpse should leave food, got {:?}",
            cell
        );
        let alarm = module
            .pheromones
            .read(gx as usize, gy as usize, PheromoneLayer::Alarm);
        assert!(alarm > 0.0, "alarm pheromone should be deposited, got {}", alarm);
    }

    #[test]
    fn red_ai_escalates_soldier_ratio_under_attack() {
        let mut sim = two_colony_sim_for_combat();
        let before = sim.colonies[1].caste_ratio.soldier;

        // Simulate sustained losses: inject combat_losses_this_tick and
        // run red_ai_tick several times.
        for _ in 0..15 {
            sim.colonies[1].combat_losses_this_tick = 3;
            sim.red_ai_tick();
        }
        let after = sim.colonies[1].caste_ratio.soldier;
        assert!(
            after > before,
            "AI should escalate soldier ratio: before={} after={}",
            before, after
        );
        assert!(after <= 0.5, "cap at 0.5 (got {})", after);
        // And the tick-local counter is cleared.
        assert_eq!(sim.colonies[1].combat_losses_this_tick, 0);
    }

    #[test]
    fn soldier_steers_toward_alarm_worker_steers_away() {
        use crate::ant::AntCaste;
        let cfg = small_config();
        let mut module = crate::module::Module::new(
            0,
            ModuleKind::Outworld,
            40,
            40,
            Vec2::ZERO,
            "Test",
        );
        // Lay down a strong alarm blob to the east of the ant's cell.
        for dx in 0..3 {
            module.pheromones.deposit(
                12 + dx,
                10,
                PheromoneLayer::Alarm,
                5.0,
                cfg.pheromone.max_intensity,
            );
        }

        let soldier = Ant::new_with_caste(
            1,
            0,
            Vec2::new(10.5, 10.5),
            0.0, // heading east — alarm is in-cone
            25.0,
            AntCaste::Soldier,
        );
        let worker = Ant::new_with_caste(
            2,
            0,
            Vec2::new(10.5, 10.5),
            0.0,
            10.0,
            AntCaste::Worker,
        );

        let sh = alarm_response_heading(&soldier, &module, &cfg.ant, &cfg.pheromone)
            .expect("soldier should respond to strong alarm");
        let wh = alarm_response_heading(&worker, &module, &cfg.ant, &cfg.pheromone)
            .expect("worker should respond to strong alarm");

        // Alarm is east → soldier heads roughly east (cos > 0),
        // worker heads roughly west (cos < 0).
        assert!(sh.cos() > 0.3, "soldier heading should face east, got {}", sh);
        assert!(wh.cos() < -0.3, "worker heading should face west, got {}", wh);
    }

    // ===================== Biology-grounded tests =====================

    #[test]
    fn brood_cannibalism_spares_adults_under_starvation() {
        use crate::colony::{Brood, BroodStage};

        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        cfg.colony.adult_food_consumption = 0.5; // aggressive burn
        cfg.colony.queen_egg_rate = 0.0;
        let mut sim = Simulation::new(cfg, 1);
        // Seed 20 workers + a queen at the nest.
        let nest = Vec2::new(32.0, 32.0);
        for i in 0..20u32 {
            let mut a = Ant::new_worker(8000 + i, 0, nest, 0.0, 10.0);
            a.module_id = 0;
            sim.ants.push(a);
        }
        sim.colonies[0].population.workers = 20;
        sim.colonies[0].food_stored = 0.1; // one tick's deficit incoming
        // Stuff the brood pile with eggs so cannibalism has something to eat.
        for _ in 0..30 {
            sim.colonies[0].brood.push(Brood::new_egg(AntCaste::Worker));
            sim.colonies[0].eggs += 1;
        }

        let adults_before = sim.colonies[0].adult_total();
        let eggs_before = sim.colonies[0].eggs;
        sim.colony_economy_tick();
        let adults_after = sim.colonies[0].adult_total();
        let eggs_after = sim.colonies[0].eggs;
        assert_eq!(
            adults_before, adults_after,
            "adults should be spared when brood is cannibalized"
        );
        assert!(
            eggs_after < eggs_before,
            "some eggs should be consumed (before={}, after={})",
            eggs_before,
            eggs_after
        );
    }

    #[test]
    fn queen_lay_rate_throttled_by_food_inflow() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        cfg.colony.queen_egg_rate = 1.0; // fast — want to see throttle clearly
        cfg.colony.adult_food_consumption = 0.0; // isolate throttle
        let mut sim = Simulation::new(cfg, 1);
        sim.colonies[0].food_stored = 1000.0;

        // Zero food inflow → throttle to endogenous floor (0.2).
        sim.colonies[0].food_inflow_recent = 0.0;
        let eggs_before = sim.colonies[0].eggs;
        for _ in 0..10 {
            sim.colony_economy_tick();
        }
        let laid_slow = sim.colonies[0].eggs - eggs_before;

        // High food inflow → throttle to 1.0 — should lay significantly more.
        sim.colonies[0].food_inflow_recent = 100.0;
        let eggs_before = sim.colonies[0].eggs;
        for _ in 0..10 {
            sim.colony_economy_tick();
        }
        let laid_fast = sim.colonies[0].eggs - eggs_before;

        assert!(
            laid_fast > laid_slow,
            "throttled queen should lay fewer eggs with no inflow (slow={}, fast={})",
            laid_slow,
            laid_fast
        );
    }

    #[test]
    fn trophic_eggs_produce_small_net_food_income() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        cfg.colony.queen_egg_rate = 1.0; // fast so trophic is nontrivial
        cfg.colony.adult_food_consumption = 0.0;
        let mut sim = Simulation::new(cfg, 1);
        sim.colonies[0].food_stored = 10.0;
        // Suppress fertile-egg laying by zeroing food temporarily each
        // tick — or simpler: disable via fertility_suppressed.
        sim.colonies[0].fertility_suppressed = true;
        sim.colonies[0].food_inflow_recent = 0.0;

        let before = sim.colonies[0].food_stored;
        for _ in 0..500 {
            sim.colony_economy_tick();
        }
        let after = sim.colonies[0].food_stored;
        assert!(
            after > before,
            "trophic eggs should nudge food_stored up over time ({} -> {})",
            before,
            after
        );
    }

    #[test]
    fn tech_gate_disables_brood_cannibalism() {
        use crate::colony::{Brood, TechUnlock};
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        cfg.colony.adult_food_consumption = 0.5;
        cfg.colony.queen_egg_rate = 0.0;
        let mut sim = Simulation::new(cfg, 1);
        // Withhold the cannibalism tech — simulate a PvP colony that
        // hasn't researched Nutrient Recycling yet.
        sim.colonies[0]
            .tech_unlocks
            .retain(|t| *t != TechUnlock::BroodCannibalism);
        let nest = Vec2::new(32.0, 32.0);
        for i in 0..5u32 {
            let mut a = Ant::new_worker(8500 + i, 0, nest, 0.0, 10.0);
            a.module_id = 0;
            sim.ants.push(a);
        }
        sim.colonies[0].population.workers = 5;
        sim.colonies[0].food_stored = 0.1;
        for _ in 0..20 {
            sim.colonies[0].brood.push(Brood::new_egg(AntCaste::Worker));
            sim.colonies[0].eggs += 1;
        }
        let eggs_before = sim.colonies[0].eggs;
        sim.colony_economy_tick();
        // Without the tech, brood is untouched — adults starve directly.
        assert_eq!(sim.colonies[0].eggs, eggs_before, "brood must survive without the tech");
    }

    // ===================== Phase 7 player-interaction tests =====================

    #[test]
    fn possess_picks_nearest_non_queen() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        let mut sim = Simulation::new(cfg, 1);
        // Two workers and a queen.
        let mut a = Ant::new_worker(7301, 0, Vec2::new(10.0, 10.0), 0.0, 10.0);
        a.module_id = 0;
        sim.ants.push(a);
        let mut b = Ant::new_worker(7302, 0, Vec2::new(20.0, 20.0), 0.0, 10.0);
        b.module_id = 0;
        sim.ants.push(b);
        let mut q = Ant::new_with_caste(
            7303,
            0,
            Vec2::new(11.0, 11.0),
            0.0,
            100.0,
            AntCaste::Queen,
        );
        q.module_id = 0;
        sim.ants.push(q);

        let possessed = sim.possess_nearest(0, 0, Vec2::new(10.5, 10.5));
        assert_eq!(possessed, Some(7301));
        assert!(sim.ants.iter().filter(|a| a.is_player).count() == 1);
        // Repossess somewhere else — old avatar should be released.
        sim.possess_nearest(0, 0, Vec2::new(19.5, 19.5));
        assert_eq!(
            sim.ants.iter().filter(|a| a.is_player).count(),
            1,
            "only one avatar at a time"
        );
    }

    #[test]
    fn player_heading_is_not_overridden_by_fsm() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        let mut sim = Simulation::new(cfg, 1);
        let mut a = Ant::new_worker(7401, 0, Vec2::new(10.0, 10.0), 0.0, 10.0);
        a.module_id = 0;
        sim.ants.push(a);
        sim.possess_nearest(0, 0, Vec2::new(10.0, 10.0));
        sim.set_player_heading(std::f32::consts::FRAC_PI_2); // north
        sim.sense_and_decide();
        // Heading should still be pi/2 even though FSM ran.
        let pi = sim.player_ant_index().expect("avatar was possessed");
        let h = sim.ants[pi].heading;
        assert!(
            (h - std::f32::consts::FRAC_PI_2).abs() < 1e-4,
            "player heading must survive sense_and_decide, got {}",
            h
        );
    }

    #[test]
    fn recruit_nearby_bonds_workers_and_they_steer_to_leader() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        let mut sim = Simulation::new(cfg, 1);
        // Leader at origin-ish.
        let mut leader = Ant::new_worker(7501, 0, Vec2::new(10.0, 10.0), 0.0, 10.0);
        leader.module_id = 0;
        sim.ants.push(leader);
        // 4 nearby workers.
        for i in 0..4u32 {
            let mut w = Ant::new_worker(
                7600 + i,
                0,
                Vec2::new(12.0 + i as f32 * 0.5, 10.0),
                std::f32::consts::PI, // facing west, opposite of leader
                10.0,
            );
            w.module_id = 0;
            sim.ants.push(w);
        }

        let got = sim.recruit_nearby(7501, 5.0, 3);
        assert_eq!(got, 3, "should recruit max_count=3");
        let bonded = sim.ants.iter().filter(|a| a.follow_leader == Some(7501)).count();
        assert_eq!(bonded, 3);

        // After follower_steering, the bonded workers face east (toward leader).
        sim.follower_steering();
        let heads: Vec<f32> = sim
            .ants
            .iter()
            .filter(|a| a.follow_leader == Some(7501))
            .map(|a| a.heading)
            .collect();
        // All 3 should face roughly west (leader is west of them) — cos < 0.
        for h in heads {
            assert!(h.cos() < 0.0, "recruit should turn toward leader (west), got {}", h);
        }
    }

    #[test]
    fn beacon_deposits_pheromone_and_expires() {
        use crate::player::BeaconKind;
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        let mut sim = Simulation::new(cfg, 1);

        let bid = sim.place_beacon(
            BeaconKind::Attack,
            0,
            Vec2::new(5.5, 5.5),
            3.0,
            2, // expires after 2 ticks
            0,
        );
        assert_eq!(sim.beacons.len(), 1);
        sim.beacon_tick();
        sim.beacon_tick();
        let alarm = sim
            .topology
            .module(0)
            .pheromones
            .read(5, 5, PheromoneLayer::Alarm);
        assert!(alarm > 0.0, "alarm should be > 0 after beacon ticks");
        // Third tick — beacon should be gone.
        sim.beacon_tick();
        assert!(sim.beacons.iter().find(|b| b.id == bid).is_none(), "beacon expired");
    }

    // ===================== Phase 6 hazards tests =====================

    #[test]
    fn antlion_kills_ant_on_its_cell() {
        use crate::hazards::PredatorKind;
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        let mut sim = Simulation::new(cfg, 1);

        // Stand a worker on cell (10, 10).
        let mut ant = Ant::new_worker(7001, 0, Vec2::new(10.5, 10.5), 0.0, 10.0);
        ant.module_id = 0;
        sim.ants.push(ant);

        // Place an antlion on the same cell.
        sim.spawn_predator(PredatorKind::Antlion, 0, Vec2::new(10.5, 10.5));

        sim.hazards_tick();

        assert!(
            sim.ants.iter().find(|a| a.id == 7001).is_none(),
            "worker should have been claimed by the antlion"
        );
    }

    #[test]
    fn spider_hunts_and_eats_nearby_ant() {
        use crate::hazards::{PredatorKind, PredatorState};
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        cfg.hazards.spider_speed = 5.0;
        cfg.hazards.spider_sense_radius = 20.0;
        cfg.hazards.spider_eat_ticks = 3;
        let mut sim = Simulation::new(cfg, 1);

        let mut ant = Ant::new_worker(7101, 0, Vec2::new(15.0, 10.0), 0.0, 10.0);
        ant.module_id = 0;
        sim.ants.push(ant);

        let sid = sim.spawn_predator(PredatorKind::Spider, 0, Vec2::new(10.0, 10.0));

        // A couple of ticks should close the distance and bite.
        for _ in 0..6 {
            sim.hazards_tick();
        }
        assert!(
            sim.ants.iter().find(|a| a.id == 7101).is_none(),
            "spider should have eaten the worker"
        );
        let sp = sim.predators.iter().find(|p| p.id == sid).unwrap();
        assert!(
            matches!(sp.state, PredatorState::Eat { .. } | PredatorState::Patrol),
            "spider state should be Eat (recent kill) or Patrol (eat finished): {:?}",
            sp.state
        );
    }

    #[test]
    fn rain_wipes_surface_pheromones_and_leaves_underground() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        cfg.hazards.rain_period_ticks = 5;
        cfg.hazards.rain_duration_ticks = 3;
        let mut topology = Topology::starter_formicarium((24, 24), (24, 24));
        let ug = topology.attach_underground(0, 0, 24, 24);
        let mut sim = Simulation::new_with_topology(cfg, topology, 1);

        // Deposit pheromones on surface + underground.
        sim.topology
            .module_mut(0)
            .pheromones
            .deposit(5, 5, PheromoneLayer::FoodTrail, 5.0, 10.0);
        sim.topology
            .module_mut(ug)
            .pheromones
            .deposit(5, 5, PheromoneLayer::FoodTrail, 5.0, 10.0);
        let underground_before = sim
            .topology
            .module(ug)
            .pheromones
            .read(5, 5, PheromoneLayer::FoodTrail);
        assert!(underground_before > 0.0);

        // Run ticks past the rain trigger (period=5). Use sim.tick() so
        // the tick counter actually advances.
        for _ in 0..10 {
            sim.tick();
        }
        assert!(sim.weather.total_rain_events >= 1, "rain should have fired");
        let surface_after = sim
            .topology
            .module(0)
            .pheromones
            .read(5, 5, PheromoneLayer::FoodTrail);
        let underground_after = sim
            .topology
            .module(ug)
            .pheromones
            .read(5, 5, PheromoneLayer::FoodTrail);
        assert!(surface_after.abs() < 0.01, "surface should be wiped, got {}", surface_after);
        assert!(
            underground_after > 0.0,
            "underground should be untouched by rain, got {}",
            underground_after
        );
    }

    #[test]
    fn lawnmower_warns_then_sweeps_and_kills_surface_ants() {
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        cfg.hazards.lawnmower_period_ticks = 3;
        cfg.hazards.lawnmower_warning_ticks = 2;
        cfg.hazards.lawnmower_speed = 2.0;
        cfg.hazards.lawnmower_half_width = 1.5;
        let mut sim = Simulation::new(cfg, 1);

        // Surface workers spread out along y.
        for i in 0..5u32 {
            let mut a = Ant::new_worker(
                7200 + i,
                0,
                Vec2::new(5.0 + i as f32, i as f32 * 2.0 + 2.0),
                0.0,
                10.0,
            );
            a.module_id = 0;
            sim.ants.push(a);
        }
        let initial = sim.ants.len();

        // Run enough ticks for the warning + full sweep to complete.
        for _ in 0..60 {
            sim.tick();
        }
        assert!(
            sim.weather.total_mower_kills > 0,
            "lawnmower should have claimed at least one ant"
        );
        assert!(sim.ants.len() < initial, "ant count should drop");
    }

    #[test]
    fn dead_spider_respawns_after_cooldown() {
        use crate::hazards::{PredatorKind, PredatorState};
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        cfg.hazards.spider_respawn_ticks = 3;
        let mut sim = Simulation::new(cfg, 1);
        let sid = sim.spawn_predator(PredatorKind::Spider, 0, Vec2::new(5.0, 5.0));

        // Force the spider dead.
        if let Some(p) = sim.predators.iter_mut().find(|p| p.id == sid) {
            p.state = PredatorState::Dead { respawn_in_ticks: 3 };
            p.health = 0.0;
        }
        for _ in 0..5 {
            sim.hazards_tick();
        }
        let sp = sim.predators.iter().find(|p| p.id == sid).unwrap();
        assert!(
            matches!(sp.state, PredatorState::Patrol),
            "spider should have respawned, state = {:?}",
            sp.state
        );
        assert!(sp.health > 0.0);
    }

    #[test]
    fn underground_attaches_with_expected_chambers() {
        use crate::ChamberType;
        let mut topology = Topology::starter_formicarium((32, 24), (48, 48));
        let before = topology.modules.len();
        let ug = topology.attach_underground(0, 0, 40, 24);
        assert_eq!(topology.modules.len(), before + 1);
        let m = topology.module(ug);
        assert_eq!(m.kind, ModuleKind::UndergroundNest);
        // Quick count: expect at least one of every chamber type and a
        // meaningful Solid majority.
        let mut solid = 0;
        let mut queen = 0;
        let mut brood = 0;
        let mut store = 0;
        let mut waste = 0;
        for y in 0..m.world.height {
            for x in 0..m.world.width {
                match m.world.get(x, y) {
                    crate::Terrain::Solid => solid += 1,
                    crate::Terrain::Chamber(ChamberType::QueenChamber) => queen += 1,
                    crate::Terrain::Chamber(ChamberType::BroodNursery) => brood += 1,
                    crate::Terrain::Chamber(ChamberType::FoodStorage) => store += 1,
                    crate::Terrain::Chamber(ChamberType::Waste) => waste += 1,
                    _ => {}
                }
            }
        }
        assert!(queen > 0 && brood > 0 && store > 0 && waste > 0, "all 4 chambers present");
        assert!(
            solid as f32 > 0.5 * (m.world.width * m.world.height) as f32,
            "underground should be mostly Solid at start"
        );
    }

    #[test]
    fn dig_tick_excavates_adjacent_solid() {
        use crate::ant::AntCaste;
        let mut topology = Topology::starter_formicarium((32, 24), (48, 48));
        let ug = topology.attach_underground(0, 0, 40, 24);
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        let mut sim = Simulation::new_with_topology(cfg, topology, 1);

        // Build a deterministic mini-setup: one Empty cell surrounded by
        // Solid on all 4 sides, on the underground module. Ignore the
        // pre-carved starter chambers.
        let (cx, cy) = (5usize, 5usize);
        let m = sim.topology.module_mut(ug);
        m.world.set(cx, cy, crate::Terrain::Empty);
        for (nx, ny) in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
            m.world.set(nx, ny, crate::Terrain::Solid);
        }

        let mut digger = Ant::new_with_caste(
            8888,
            0,
            Vec2::new(cx as f32 + 0.5, cy as f32 + 0.5),
            0.0,
            10.0,
            AntCaste::Worker,
        );
        digger.module_id = ug;
        digger.state = AntState::Digging;
        sim.ants.push(digger);

        // Multi-substep excavation: digger needs ~60 ticks of progress
        // before the tile flips. Run 80 to be safe (covers both the
        // accumulation and the post-flip state-transition substep).
        for _ in 0..80 {
            sim.dig_tick();
        }

        let m = sim.topology.module(ug);
        let solid_left = [
            m.world.get(cx + 1, cy),
            m.world.get(cx - 1, cy),
            m.world.get(cx, cy + 1),
            m.world.get(cx, cy - 1),
        ]
        .into_iter()
        .filter(|t| *t == crate::Terrain::Solid)
        .count();
        assert_eq!(
            solid_left, 3,
            "exactly one Solid neighbor should be excavated after 80 substeps \
             (3 still Solid, 1 now Empty)"
        );
        // Digger should now be carrying a soil pellet (pickup happened
        // when the tile flipped). sim.ants[0] is the auto-spawned queen;
        // the digger we pushed is the last ant.
        let digger = sim.ants.last().expect("digger present");
        assert!(digger.carrying_soil, "digger should carry a soil pellet after excavation");
        assert_eq!(digger.state, AntState::ReturningHome,
            "digger should transition to ReturningHome after extracting the pellet");
    }

    #[test]
    fn surface_underground_traversal_descends_digger() {
        use crate::ant::AntCaste;
        // Build starter formicarium + underground for colony 0. The
        // surface module gets a NestEntrance from `spawn_initial_ants`;
        // the underground gets one from `attach_underground`.
        let mut topology = Topology::starter_formicarium((32, 24), (48, 48));
        let _ug = topology.attach_underground(0, 0, 40, 24);
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        let mut sim = Simulation::new_with_topology(cfg, topology, 1);

        // Find the surface nest entrance position.
        let surf_mod = sim.topology.surface_nest_for_colony(0).expect("surface nest");
        let ug_mod = sim.topology.underground_for_colony(0).expect("underground nest");
        let (sx, sy) = sim
            .topology
            .module(surf_mod)
            .world
            .find_nest_entrance(0)
            .expect("surface entrance");

        // Place a worker at the surface entrance in Digging state.
        let mut digger = Ant::new_with_caste(
            9999,
            0,
            Vec2::new(sx as f32 + 0.5, sy as f32 + 0.5),
            0.0,
            10.0,
            AntCaste::Worker,
        );
        digger.module_id = surf_mod;
        digger.state = AntState::Digging;
        sim.ants.push(digger);
        let digger_idx = sim.ants.len() - 1;

        sim.surface_underground_traversal();

        // Digger should now be on the underground module.
        assert_eq!(
            sim.ants[digger_idx].module_id, ug_mod,
            "digger should teleport to underground module on entering nest entrance while Digging"
        );
    }

    #[test]
    fn carrying_digger_returns_to_surface_at_underground_entrance() {
        use crate::ant::AntCaste;
        let mut topology = Topology::starter_formicarium((32, 24), (48, 48));
        let _ug = topology.attach_underground(0, 0, 40, 24);
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        let mut sim = Simulation::new_with_topology(cfg, topology, 1);

        let surf_mod = sim.topology.surface_nest_for_colony(0).expect("surface nest");
        let ug_mod = sim.topology.underground_for_colony(0).expect("underground nest");
        let (ux, uy) = sim
            .topology
            .module(ug_mod)
            .world
            .find_nest_entrance(0)
            .expect("ug entrance");

        // Place an ant on the underground entrance carrying a pellet.
        let mut carrier = Ant::new_with_caste(
            7777,
            0,
            Vec2::new(ux as f32 + 0.5, uy as f32 + 0.5),
            0.0,
            10.0,
            AntCaste::Worker,
        );
        carrier.module_id = ug_mod;
        carrier.carrying_soil = true;
        carrier.state = AntState::ReturningHome;
        sim.ants.push(carrier);
        let idx = sim.ants.len() - 1;

        sim.surface_underground_traversal();

        assert_eq!(
            sim.ants[idx].module_id, surf_mod,
            "carrying digger should surface at underground entrance"
        );
    }

    #[test]
    fn solid_blocks_ant_movement() {
        use crate::ant::AntCaste;
        let mut topology = Topology::starter_formicarium((32, 24), (48, 48));
        let ug = topology.attach_underground(0, 0, 40, 24);
        let mut cfg = small_config();
        cfg.ant.initial_count = 0;
        let mut sim = Simulation::new_with_topology(cfg, topology, 1);

        // Seed the center of the underground as Solid and place an ant
        // just east of it, heading west. One movement tick should NOT
        // advance the ant into the Solid cell.
        let ug_mod_mut = sim.topology.module_mut(ug);
        let (mx, my) = (10usize, 10usize);
        ug_mod_mut.world.set(mx, my, crate::Terrain::Solid);

        let mut ant = Ant::new_with_caste(
            9999,
            0,
            Vec2::new((mx + 1) as f32 + 0.5, my as f32 + 0.5),
            std::f32::consts::PI, // heading west, toward the Solid cell
            10.0,
            AntCaste::Worker,
        );
        ant.module_id = ug;
        sim.ants.push(ant);
        let start_x = sim.ants.last().unwrap().position.x;

        sim.movement();
        let end = &sim.ants[sim.ants.len() - 1];
        // Must NOT be inside the Solid cell (no x in [mx, mx+1)).
        assert!(
            end.position.x > (mx as f32 + 1.0) - 0.001 || end.position.x >= start_x - 0.01,
            "ant should be blocked by Solid terrain (start={}, end={})",
            start_x,
            end.position.x
        );
    }

    #[test]
    fn territory_deposits_signed_by_colony() {
        let mut sim = two_colony_sim_for_combat();
        // Stand a black worker on module 1 (outworld), cell (5,5).
        place_combatant(&mut sim, 9701, 0, Vec2::new(5.5, 5.5), AntCaste::Worker, 10.0);
        // Stand a red worker on module 1, cell (20, 20).
        place_combatant(&mut sim, 9702, 1, Vec2::new(20.5, 20.5), AntCaste::Worker, 10.0);

        for _ in 0..40 {
            sim.territory_deposit_tick();
        }
        let m = sim.topology.module(1);
        let black = m.pheromones.read(5, 5, PheromoneLayer::ColonyScent);
        let red = m.pheromones.read(20, 20, PheromoneLayer::ColonyScent);
        assert!(black > 0.0, "black cell should be positive, got {}", black);
        assert!(red < 0.0, "red cell should be negative, got {}", red);
    }

    #[test]
    fn avenger_is_assigned_and_tracks_enemy() {
        let mut sim = two_colony_sim_for_combat();
        // Starter already spawned both colonies — verify an avenger exists
        // on the red side.
        let avenger_count = sim
            .ants
            .iter()
            .filter(|a| a.colony_id == 1 && a.is_avenger)
            .count();
        assert_eq!(avenger_count, 1, "exactly one red avenger at spawn");

        // Put a black worker on the same module as the avenger and check
        // the avenger heads toward it after one avenger_tick.
        let av_idx = sim
            .ants
            .iter()
            .position(|a| a.is_avenger)
            .expect("avenger exists");
        sim.ants[av_idx].position = Vec2::new(5.0, 5.0);
        sim.ants[av_idx].module_id = 1;
        place_combatant(
            &mut sim,
            9501,
            0,
            Vec2::new(8.0, 5.0), // due east of avenger
            AntCaste::Worker,
            10.0,
        );
        sim.avenger_tick();
        let h = sim.ants[av_idx].heading;
        assert!(h.cos() > 0.7, "avenger heading should point east, got {}", h);
    }

    #[test]
    fn avenger_role_transfers_when_killed() {
        let mut sim = two_colony_sim_for_combat();
        // Kill the current avenger.
        let av_idx = sim
            .ants
            .iter()
            .position(|a| a.is_avenger)
            .expect("avenger exists");
        sim.ants.swap_remove(av_idx);

        // Next avenger_tick should promote a replacement.
        sim.avenger_tick();
        let count = sim
            .ants
            .iter()
            .filter(|a| a.colony_id == 1 && a.is_avenger)
            .count();
        assert_eq!(count, 1, "a replacement avenger must be promoted");
    }

    #[test]
    fn same_colony_ants_never_attack_each_other() {
        let mut sim = two_colony_sim_for_combat();
        place_combatant(&mut sim, 9201, 0, Vec2::new(8.0, 8.0), AntCaste::Soldier, 25.0);
        place_combatant(&mut sim, 9202, 0, Vec2::new(8.2, 8.0), AntCaste::Worker, 5.0);
        let before_losses = sim.colonies[0].combat_losses;

        for _ in 0..20 {
            sim.combat_tick();
        }
        assert_eq!(
            sim.colonies[0].combat_losses, before_losses,
            "friendly fire must not happen"
        );
    }
}

# HANDOFF.md — Phased Implementation Spec

This document contains everything needed to implement the ant colony simulation from scratch. Each phase is self-contained with clear inputs, outputs, and acceptance criteria. **Phases are sequential — do not skip ahead.**

---

## Current Status (2026-04-16)

**Phases 1, 2, 3 COMPLETE.**

- Workspace: `antcolony-sim`, `antcolony-game`, `antcolony-render`, root binary. Toolchain pinned to `stable-x86_64-pc-windows-gnu`. Edition 2024 (`rng.r#gen()` required).
- **Sim tests:** 19 unit + 1 integration, all passing. Covers emergent pathfinding (`test_ant_finds_food`, `test_trail_formation`, `headless_delivers_food`) plus economy (`colony_grows_with_food`, `colony_starves_without_food`, `caste_ratio_affects_spawns`, `queen_death_stops_production`).
- **Economy:** `Simulation::colony_economy_tick` runs every tick — consumption → egg-lay → brood maturation (egg→larva→pupa→adult) → spawning with caste drawn from `ColonyState.caste_ratio` → starvation deaths (oldest-first). `Brood { stage, caste, age }` vector on `ColonyState`. Queen death halts production and is logged; first-egg + population milestones (every 50) also logged.
- **Render:** 200 initial ants, live pheromone overlay, food/nest tiles. UI debug HUD shows tick/FPS/ants-by-caste/food/brood/queen HP, plus Worker-Soldier-Breeder and Forage-Dig-Nurse triangle sliders writing back to `ColonyState`.
- **Controls:** WASD/arrows pan, scroll zoom, `P` pheromone overlay, `1-4` sim speed (30/60/150/300 Hz), `Space` pause.
- **Run:** `cargo run --release` (verbose: `./scripts/run_dev.ps1`). First egg typically lays by tick 19.

**Next: Phase 4 — multi-colony + combat.** Add a second (red) `ColonyState` at the opposite corner; add a combat system using the existing `SpatialHash`; wire up `Ant.health` damage; spawn corpses as small food sources; alarm pheromone on death site; basic red-colony behavior-weight AI.

---

## Keeper Mode — Phase K1 COMPLETE

**Data-driven species + player-chosen time scale.** The sim no longer hardcodes a config; instead the player picks from 7 real species at startup and selects a time scale.

- `Species` struct (`crates/antcolony-sim/src/species.rs`) with biology, growth, diet, combat profile, appearance, encyclopedia. Authored as TOML per species under `assets/species/`.
- `Environment` + `TimeScale` (`crates/antcolony-sim/src/environment.rs`). Four scales: Realtime (1×), Brisk (10×), Seasonal (60× — default), Timelapse (1440×).
- All biological durations authored in **in-game seconds**. `Species::apply(&env)` folds them into tick-denominated `SimConfig` via `ticks = in_game_seconds × tick_rate / time_scale`. Sim loop itself is untouched — it operates in ticks, agnostic to real-time.
- 7 shipped species: Lasius niger, Camponotus pennsylvanicus, Tetramorium immigrans, Formica rufa, Pogonomyrmex occidentalis, Tapinoma sessile, Aphaenogaster rudis. Real biology numbers (28-yr Lasius queen, polymorphic Camponotus majors/minors, Formica rufa formic-acid aggression, etc.).
- Bevy `AppState { Picker, Running }`. Picker shows species list (color swatch + scientific name + difficulty badge + tagline), detail pane (description, fun facts, keeper notes, colony stats), time-scale toggles, confirm button. On confirm → `SimulationState::from_species(&species, &env)` → transitions to Running. In-game, `E` toggles an encyclopedia side panel.
- Test count: 28 sim + 1 integration, all green.
- Bevy feature `bevy_state` required for the state machine (added to root `Cargo.toml`).

## Keeper Mode — Phase K2.1 COMPLETE

**Modular formicarium topology core.** The single-world assumption is broken. `Simulation` now owns a `Topology { modules: Vec<Module>, tubes: Vec<Tube> }`. Each module has its own `WorldGrid` + `PheromoneGrid`.

- `Module { id, kind: ModuleKind, world, pheromones, formicarium_origin, ports, label }` (`crates/antcolony-sim/src/module.rs`). `ModuleKind` covers TestTubeNest, Outworld, YTongNest, AcrylicNest, Hydration, HeatChamber, HibernationChamber, FeedingDish, Graveyard (only TestTubeNest + Outworld wired into gameplay for now).
- `Tube { id, from, to, length_ticks, bore_width_mm }` (`crates/antcolony-sim/src/tube.rs`). `TubeTransit { tube, progress, going_forward }` on Ant.
- `Ant` gains `module_id: u16` + `transit: Option<TubeTransit>`.
- `Topology::single(...)` preserves pre-K2 behavior so all old tests pass unchanged.
- `Topology::starter_formicarium((nest_w, nest_h), (out_w, out_h))` builds the Keeper Mode starter: TestTubeNest east-wall port ↔ Outworld west-wall port, 30-tick tube. Ants spawn on module 0; food lands on module 1.
- Tick pipeline iterates modules. Tube transit: ants walking onto a port cell enter the attached tube, advance `progress` per tick based on speed / tube length, emerge on the far side with heading pointing into the destination module.
- **Port-scent bleed:** after evaporation/diffusion, the two port cells on each tube equilibrate a fraction (`PORT_BLEED_RATE = 0.35`) of their pheromone intensities. Result: trails carry across tubes naturally.
- `Simulation::world()` / `.pheromones()` accessor methods return module-0 grids for pre-K2 callers. New method `spawn_food_cluster_on(module_id, ...)` for multi-module seeding.
- Render: multi-module. Each module rendered at its `formicarium_origin × TILE` offset with dark panel background, border frame, independent pheromone overlay texture, port markers (yellow dots), and tube drawn as a rotated rectangle between ports. Ants in tube transit are hidden (TODO v2: interpolate along the tube).
- `SimulationState::from_species` builds a starter formicarium sized from `env.world_width/height` (nest ≈ 1/4 of world, outworld full size).
- **Tests:** 34 sim unit + 1 integration, all green (+6 from K2: topology constructors, tube_at_port lookup, starter-formicarium build, ant-traverses-tube kinematics, pheromone-bleeds-across-tube, multi-module initial-ant placement).

**Next Keeper phase: K2.2 — Module editor + variety.**
- Drag/drop module-board view (zoomed-out formicarium layout, add/remove modules, draw tubes).
- Additional module kinds with distinct gameplay properties (Hydration, FeedingDish, Graveyard).
- Bore-width caste restrictions (majors refused by narrow tubes).
- Tube transit interpolation in render (ant visible traveling along tube).
- `E` encyclopedia + HUD already adapt to topology since they only read `ColonyState`.

## Keeper Mode — Phase K2.2 COMPLETE

- **Tube transit interpolation:** `sync_ant_sprites` now lerps between the two port world-positions using `TubeTransit.progress`; ants stay visible while traveling and rotate to face the tube direction.
- **Bore-width gate:** `AntConfig` gained `worker_size_mm` + `polymorphic` (populated by `Species::apply` from `appearance.size_mm` / `biology.polymorphic`). `Ant::body_size_mm(&AntConfig)` returns Worker/Breeder = base, Queen = 1.3×, Soldier = 1.6× if polymorphic else 1.15×. In `Simulation::movement`, port-entry is now conditional on `body_size_mm ≤ tube.bore_width_mm`; too-big ants reflect (trace-level log, no spam).
- **FeedingDish auto-refill:** `Module` gained `tick_cooldown: u32`. `Simulation::feeding_dish_tick()` runs in the pipeline between `deposit_and_interact` and `colony_economy_tick`; refills each FeedingDish with a radius-2 / 8-unit cluster at the module center when terrain food < 5, then cooldown=600 ticks. Info log per refill event (not per tick).
- **3-module starter:** `Topology::starter_formicarium_with_feeder(nest, outworld, dish)` adds an outworld-south ↔ dish-north tube (tube id 1, 20 ticks, 8mm). `SimulationState::from_species` now builds the 3-module version by default (dish ≈ 1/3 outworld size).
- **`M` overview toggle:** Saves current camera + ortho scale, fits the full formicarium bounding box with 10% margin. Second press restores. Pan/zoom still works in overview.
- Render: FeedingDish renders with the same dark module panel + border + ports as other modules (no special casing needed); tubes drawn the same way.
- **Tests:** 36 sim unit (+2 new: `major_blocked_by_narrow_tube`, `feeding_dish_refills_food`). All green.

**Next Keeper phase: K2.3 — Module editor UI.**
- Drag/drop module-board view (zoomed-out formicarium layout, add/remove modules, draw tubes).
- Wire additional kinds (Hydration, Graveyard) into gameplay.
- Tube bore-width authoring UI (narrow-bore tubes = worker-only paths).

**Then K3:** thermoregulation + hibernation (temperature grids per module, annual clock, diapause-gated queen fertility for required species).

---

## Phase 1: Pheromone Grid + Ant Movement (Headless)

**Goal:** Pure simulation crate (`antcolony-sim`) with pheromone fields and ant agents that produce emergent trail formation. No rendering. Validated entirely through tests.

### 1.1 Scaffold the Workspace

Create the Cargo workspace with three crates. Phase 1 only touches `antcolony-sim`.

```toml
# Root Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/antcolony-sim",
    "crates/antcolony-game",
    "crates/antcolony-render",
]

[workspace.dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
anyhow = "1"
rand = "0.8"
glam = { version = "0.29", features = ["serde"] }
toml = "0.8"
```

```toml
# crates/antcolony-sim/Cargo.toml
[package]
name = "antcolony-sim"
version = "0.1.0"
edition = "2024"

[dependencies]
tracing.workspace = true
serde.workspace = true
thiserror.workspace = true
anyhow.workspace = true
rand.workspace = true
glam.workspace = true
toml.workspace = true

[dev-dependencies]
tracing-subscriber.workspace = true
```

### 1.2 Config System

All numeric constants in one place. Loaded from TOML, with sane defaults.

```rust
// crates/antcolony-sim/src/config.rs
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SimConfig {
    pub world: WorldConfig,
    pub pheromone: PheromoneConfig,
    pub ant: AntConfig,
    pub colony: ColonyConfig,
    pub combat: CombatConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WorldConfig {
    pub width: usize,
    pub height: usize,
    pub food_spawn_rate: f32,
    pub food_cluster_size: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PheromoneConfig {
    pub evaporation_rate: f32,
    pub diffusion_rate: f32,
    pub diffusion_interval: u32,
    pub max_intensity: f32,
    pub min_threshold: f32,
    pub deposit_food_trail: f32,
    pub deposit_home_trail: f32,
    pub deposit_alarm: f32,
}

// ... AntConfig, ColonyConfig, CombatConfig follow the same pattern
// See CLAUDE.md for all fields

impl Default for SimConfig {
    fn default() -> Self {
        // Hardcode the defaults from CLAUDE.md's [config] section
        // so tests work without a TOML file
    }
}

impl SimConfig {
    pub fn load_from_str(toml_str: &str) -> anyhow::Result<Self> { ... }
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> { ... }
}
```

### 1.3 Pheromone Grid

The core data structure. Dense flat arrays, double-buffered diffusion.

**Key implementation details:**

- Index formula: `y * width + x` — row-major for cache locality during horizontal sweeps
- Evaporation runs EVERY tick: `food_trail[i] *= 1.0 - evap_rate; if food_trail[i] < min_threshold { food_trail[i] = 0.0; }`
- Diffusion runs every `diffusion_interval` ticks using the scratch buffer
- Diffusion stencil (5-point Laplacian): `new[i] = old[i] * (1 - 4*d) + d * (old[up] + old[down] + old[left] + old[right])` where `d = diffusion_rate`
- Deposit caps at `max_intensity`
- Provide `fn sample_cone(&self, pos: Vec2, heading: f32, half_angle: f32, radius: f32, layer: PheromoneLayer) -> Vec<(Vec2, f32)>` for ant sensing

**Public API:**

```rust
pub enum PheromoneLayer { FoodTrail, HomeTrail, Alarm, ColonyScent }

impl PheromoneGrid {
    pub fn new(width: usize, height: usize) -> Self;
    pub fn deposit(&mut self, x: usize, y: usize, layer: PheromoneLayer, amount: f32);
    pub fn read(&self, x: usize, y: usize, layer: PheromoneLayer) -> f32;
    pub fn sample_cone(&self, pos: Vec2, heading: f32, half_angle: f32, radius: f32, layer: PheromoneLayer) -> Vec<(Vec2, f32)>;
    pub fn evaporate(&mut self, rate: f32, threshold: f32);
    pub fn diffuse(&mut self, rate: f32);
    pub fn world_to_grid(&self, pos: Vec2) -> (usize, usize);
    pub fn grid_to_world(&self, x: usize, y: usize) -> Vec2;
}
```

### 1.4 Ant Agent

Lightweight struct with enum FSM. No entity framework yet — just a `Vec<Ant>`.

**State machine transitions:**

```
Exploring:
  - IF sense food pheromone above threshold → FollowingTrail
  - IF at food source → PickingUpFood
  - ELSE → random walk with forward bias

FollowingTrail:
  - IF at food source → PickingUpFood
  - IF pheromone below threshold → Exploring
  - ELSE → follow gradient (ACO probability formula)

PickingUpFood:
  - Load food (instant in Phase 1)
  - → ReturningHome

ReturningHome:
  - Deposit food_trail pheromone each tick
  - Follow home_trail gradient toward nest
  - IF at nest entrance → StoringFood

StoringFood:
  - Add food to colony reserves
  - → Exploring
```

**Movement logic:**

```rust
fn choose_direction(ant: &Ant, grid: &PheromoneGrid, config: &AntConfig, rng: &mut impl Rng) -> f32 {
    // 1. exploration_rate% chance: pick random direction
    if rng.gen::<f32>() < config.exploration_rate {
        return rng.gen_range(0.0..std::f32::consts::TAU);
    }

    // 2. Sample 5 points in forward cone (±sense_angle)
    let samples = grid.sample_cone(
        ant.position,
        ant.heading,
        config.sense_angle.to_radians(),
        config.sense_radius as f32,
        ant.target_layer(), // FoodTrail when exploring, HomeTrail when returning
    );

    // 3. Weight by ACO formula: p(j) = τ^α × η^β / Σ(τ^α × η^β)
    //    η = forward bias (1.0 + cos(angle_to_sample - heading))
    // 4. Stochastic selection from weighted distribution
    // 5. Return selected heading
}
```

### 1.5 World Grid

Simple terrain grid for Phase 1. Just tracks: empty, food, obstacle, nest_entrance.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terrain {
    Empty,
    Food(u32),        // remaining food units
    Obstacle,
    NestEntrance(u8), // colony_id
}

pub struct WorldGrid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Terrain>,
}
```

### 1.6 Simulation Runner

A tick-based runner that owns all state and advances the simulation.

```rust
pub struct Simulation {
    pub config: SimConfig,
    pub world: WorldGrid,
    pub pheromones: PheromoneGrid,
    pub ants: Vec<Ant>,
    pub colonies: Vec<ColonyState>,
    pub tick: u64,
    pub rng: StdRng,
}

impl Simulation {
    pub fn new(config: SimConfig, seed: u64) -> Self;
    pub fn tick(&mut self);           // Advance one simulation step
    pub fn run(&mut self, ticks: u64); // Run N ticks
}
```

`tick()` executes the system pipeline in order: sense → decide → move → deposit → combat → evaporate → diffuse → economy → spawn.

### 1.7 Phase 1 Acceptance Criteria

All validated by `cargo test` in `antcolony-sim`:

- [ ] `test_pheromone_evaporation` — Deposit pheromone, run N evaporate ticks, assert exponential decay
- [ ] `test_pheromone_diffusion` — Deposit at center, diffuse, assert spread to neighbors
- [ ] `test_ant_finds_food` — Place ant at (0,0), food at (100,100), run 2000 ticks. Assert ant has delivered food to nest at least once. (This validates emergent pathfinding.)
- [ ] `test_trail_formation` — 50 ants, one food source, run 5000 ticks. Assert pheromone intensity between nest and food is significantly higher than background.
- [ ] `test_fsm_transitions` — Unit test each state transition with mock inputs
- [ ] `test_config_loads` — Parse the example TOML, assert all fields populated
- [ ] `test_spatial_hash` — Insert 1000 random positions, query radius, assert correctness vs brute-force

---

## Phase 2: Bevy Integration + Rendering

**Goal:** Ants rendered as sprites on screen, pheromone overlay visible, camera pan/zoom works.

### 2.1 Bevy Plugin Structure

```rust
// crates/antcolony-game/src/plugin.rs
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(SimulationState::new(SimConfig::default(), 42))
            .add_systems(FixedUpdate, (
                sensing_system,
                decision_system,
                movement_system,
                deposit_system,
                combat_system,
                evaporate_system,
                diffuse_system,
                colony_economy_system,
                spawning_system,
            ).chain())
            .insert_resource(Time::<Fixed>::from_hz(30.0));
    }
}
```

### 2.2 Components

```rust
#[derive(Component)]
pub struct AntComponent {
    pub sim_index: usize,  // Index into Simulation.ants
}

#[derive(Component)]
pub struct FoodSource {
    pub remaining: u32,
}

#[derive(Component)]
pub struct NestEntrance {
    pub colony_id: u8,
}
```

### 2.3 Rendering Layer

- **Ants:** 2D sprites colored by colony (black/red). Oriented by heading. Consider instanced rendering for 10K+ sprites.
- **Pheromone overlay:** Full-screen texture updated each frame from grid data. Toggle-able (key: `P`). Color channels: red = alarm, green = food trail, blue = home trail. Alpha = intensity.
- **Food:** Green circles sized by remaining units.
- **Nest entrances:** Brown circles with colony color border.
- **Camera:** 2D orthographic. WASD/arrow pan, scroll zoom, middle-mouse drag.

### 2.4 Debug UI

- **Colony stats panel:** Population (workers/soldiers/breeders), food stored, eggs/larvae/pupae, queen health
- **Sim speed controls:** Pause (Space), 1x/2x/5x/10x speed (1-4 keys)
- **Entity inspector:** Click ant → show state, heading, food carried, age
- **FPS counter + tick counter**
- Toggle pheromone overlay per layer (F1-F4)

### 2.5 Phase 2 Acceptance Criteria

- [ ] Window opens, ants visible as colored dots moving around
- [ ] Pheromone overlay shows trails forming between nest and food
- [ ] Camera pan/zoom works smoothly
- [ ] Debug UI shows colony stats updating in real-time
- [ ] Pause/speed controls work
- [ ] 1000 ants at 60fps rendering, 30Hz sim tick
- [ ] Clicking an ant shows its current state in the debug panel

---

## Phase 3: Colony Economy

**Goal:** Full food → eggs → larvae → pupae → adult lifecycle. Colony growth and starvation mechanics.

### 3.1 Economy Tick

Each colony tick (runs at sim rate):

1. **Consumption:** Each adult ant consumes `adult_food_consumption` food from colony stores. Soldiers consume `soldier_food_multiplier ×` that. If food < 0, ants start dying (oldest first).
2. **Egg laying:** If `food_stored > egg_cost` and queen is alive, queen produces eggs at `queen_egg_rate` per tick.
3. **Maturation:** Eggs → larvae after `larva_maturation_ticks`. Larvae → pupae after `pupa_maturation_ticks`. Pupae → adults (spawn new ant entity).
4. **Caste assignment:** New adults get caste based on `caste_ratio` weights (weighted random selection).

### 3.2 Caste Ratio UI

SimAnt-style triangle slider: three vertices = Workers / Soldiers / Breeders. Player drags the point inside the triangle to set production weights. Add behavior triangle too: Forage / Dig / Nurse.

### 3.3 Phase 3 Acceptance Criteria

- [ ] Colony grows from initial 20 workers when food is available
- [ ] Colony starves and shrinks when food is depleted
- [ ] Caste ratio slider visibly changes which ant types spawn
- [ ] Queen death = game over (colony stops producing)
- [ ] Colony population graph in debug UI shows growth curve

---

## Phase 4: Multi-Colony + Combat

**Goal:** Two colonies (player = black, AI = red) competing for food and territory.

### 4.1 Colony Warfare

- When a black ant meets a red ant (spatial hash query, interaction radius = 1 tile), combat initiates
- Combat resolution: each ant deals `attack` damage per tick to the other. First to 0 HP dies.
- Soldiers deal 3× damage vs workers
- Dead ants become food sources (small: 0.5 food units)
- Killing an ant releases alarm pheromone at death site

### 4.2 Red Colony AI

The red colony is autonomous:
- Same simulation systems, just no player control
- Behavior weights auto-adjust: if food < threshold → increase forage. If under attack → increase soldiers.
- Place red nest at opposite corner of map from player
- Red colony has an "Avenger" ant (SimAnt reference): one special unit that tracks toward the player's most-controlled ant and actively hunts it. When killed, a random red ant inherits the role.

### 4.3 Territory Display

- Colony scent pheromone creates territory visualization: translucent color wash over tiles dominated by each colony
- Contested borders show as mixed colors

### 4.4 Phase 4 Acceptance Criteria

- [ ] Two colonies visible on map, each foraging independently
- [ ] Ants from different colonies fight on contact
- [ ] Dead ants leave food-value corpses
- [ ] Alarm pheromone causes nearby soldiers to converge
- [ ] Red colony AI adjusts behavior to survive
- [ ] Territory overlay shows expansion/contraction
- [ ] The Avenger mechanic works (hunts player, transfers on death)

---

## Phase 5: Underground Nest Layer

**Goal:** Side-view underground cross-section with diggable tunnels, chambers, and the queen.

### 5.1 Nest Grid

Separate grid per colony. Cells are: `Solid` (unexcavated), `Tunnel`, `Chamber(ChamberType)`, `Entrance` (connects to surface).

```rust
pub enum ChamberType {
    FoodStorage,
    BroodNursery,
    QueenChamber,
    Waste,
}
```

Digging: ants in `Digging` state adjacent to `Solid` cells convert them to `Tunnel`. Chambers are created by player command (Phase 7) or AI heuristic.

### 5.2 View Switching

- Tab key toggles between Surface View and Underground View
- Underground shows side-view cross-section of the active colony's nest
- Ants moving underground are visible in the nest view
- Ants moving on surface are visible in surface view
- Nest entrances show traffic flow indicators

### 5.3 Phase 5 Acceptance Criteria

- [ ] Underground view renders tunnels and chambers
- [ ] Ants assigned to "dig" create new tunnels
- [ ] Queen sits in queen chamber, produces eggs in brood nursery
- [ ] Food storage chambers show food level
- [ ] Tab switches between surface and underground smoothly
- [ ] Ants transition between layers via nest entrances

---

## Phase 6: Environmental Hazards + Predators

**Goal:** Dynamic threats that pressure the colony.

### 6.1 Predators

- **Spider:** Fastest unit on map. Hunts ants, eats one at a time. Respawns when killed (corpse = large food source). Implement as a state machine: Patrol → Hunt → Eat → Patrol.
- **Antlion:** Stationary pit trap. Any ant entering the tile dies. Does NOT respawn when killed. Clearing antlions is permanent progress.

### 6.2 Environmental Events

- **Rain:** Periodic event. Washes away ALL surface pheromone trails. Floods lowest underground chambers (ants in flooded chambers take damage). Forces re-exploration.
- **Lawnmower:** Rare event. Sweeps across the map in a line, killing all surface ants in its path. Telegraphed with audio/visual warning 5 seconds before.

### 6.3 Phase 6 Acceptance Criteria

- [ ] Spider patrols and kills ants, drops food on death, respawns
- [ ] Antlion pits kill ants on contact, don't respawn
- [ ] Rain event clears pheromone, floods underground, ants rebuild trails
- [ ] Lawnmower event kills surface ants in its path
- [ ] Events are tunable in config (frequency, severity)

---

## Phase 7: Player Interaction

**Goal:** The player can inhabit and control a single ant (SimAnt yellow ant), issue colony commands, and place pheromone markers.

### 7.1 Yellow Ant (Player Avatar)

- Player possesses one ant (highlighted yellow)
- Direct WASD movement (overrides FSM)
- Click to pick up food, double-click to dig
- Press `0` to lay alarm pheromone manually
- Recruit command: `R` recruits 5 nearby idle ants to follow the yellow ant
- `Shift+R` recruits 10
- `E` exchanges into any nearby ant (click to select target)
- If yellow ant dies, auto-possess nearest worker

### 7.2 Colony Commands

- Behavior allocation triangle (Forage / Dig / Nurse) — affects all non-recruited ants
- Caste production triangle (Worker / Soldier / Breeder)
- Place marker commands: right-click to place a "gather here" or "attack here" pheromone beacon

### 7.3 Phase 7 Acceptance Criteria

- [ ] Yellow ant moves with WASD, distinct from AI ants
- [ ] Recruit command creates a visible ant army following the player
- [ ] Alarm pheromone placed by player attracts soldiers
- [ ] Exchange lets player jump between ants
- [ ] Colony sliders update behavior in real-time
- [ ] Pheromone beacons attract nearby ants to marked locations

---

## Phase 8: Full Game Mode

**Goal:** Grid-based map with 192 squares (12×16). Colonize the entire yard + house through mating flights.

### 8.1 Map Grid

- World is divided into a 12×16 grid of map squares
- Each square is a playable simulation area
- Player starts in one square with a founding colony
- Adjacent squares have their own food, obstacles, and possibly red colonies

### 8.2 Mating Flights

- When ~20 breeders exist, trigger mating flight event
- Breeders fly out of nest, mate in the air (mini-game or automated)
- Fertilized queens can colonize adjacent empty squares
- Birds eat breeders during flight (chance-based attrition)

### 8.3 Win Condition

- Eliminate all red colonies from all map squares
- Drive humans from the house (house squares have unique mechanics)

### 8.4 Phase 8 Acceptance Criteria

- [ ] Map overview shows grid of squares with colony presence
- [ ] Player can trigger mating flights when breeder threshold met
- [ ] New colonies establish in adjacent squares
- [ ] Red colonies exist in some squares as opposition
- [ ] Victory screen when all squares colonized

---

## Implementation Notes for Code Sessions

### Prioritize Correctness Over Performance (Phase 1-3)

In early phases, use straightforward implementations. `Vec<Ant>` is fine. HashMap-based spatial hash is fine. Optimize only when profiling shows a bottleneck. The architecture supports future optimization (SoA layout, SIMD pheromone sweeps, GPU compute) but don't prematurely complicate.

### Testing Strategy

```
Unit tests:     Every module in antcolony-sim gets #[cfg(test)] mod tests
Integration:    tests/ directory with headless sim scenarios
Visual:         Manual testing with debug overlay (Phase 2+)
Performance:    benches/ with criterion, target 10K ants at 30Hz
```

### Logging Conventions

```rust
// System entry/exit
tracing::debug!(tick = sim.tick, ant_count = sim.ants.len(), "Starting sensing_system");

// State transitions (IMPORTANT for debugging emergent behavior)
tracing::trace!(ant_id = ant.id, from = ?ant.state, to = ?new_state, "FSM transition");

// Economy events
tracing::info!(colony_id = colony.id, food = colony.food_stored, eggs = colony.eggs, "Colony economy tick");

// Rare events
tracing::warn!(event = "rain", "Rain event triggered — clearing surface pheromones");

// Errors
tracing::error!(error = %e, "Failed to load simulation config");
```

### Git Conventions

- Commit per completed sub-item within a phase
- Tag each completed phase: `v0.1.0` (Phase 1), `v0.2.0` (Phase 2), etc.
- Branch per phase: `phase/1-pheromone-grid`, `phase/2-bevy-rendering`

---

## Quick Reference: SimAnt Mechanics to Implement

| SimAnt Feature | Phase | Notes |
|---|---|---|
| Food foraging + pheromone trails | 1 | Core loop |
| Colony economy (food → eggs → ants) | 3 | Queen + brood cycle |
| Caste system (worker/soldier/breeder) | 3 | Triangle slider |
| Behavior allocation (forage/dig/nurse) | 3 | Triangle slider |
| Red colony enemy | 4 | AI-controlled opponent |
| Ant combat | 4 | Spatial proximity |
| The Avenger (red hunter ant) | 4 | Tracks player ant |
| Underground nest (tunnels/chambers) | 5 | Side-view layer |
| Spider predator | 6 | Fast, respawns |
| Antlion pit traps | 6 | Stationary, permanent kill |
| Rain (clears pheromones) | 6 | Environmental event |
| Lawnmower | 6 | Kills surface ants |
| Yellow ant (player avatar) | 7 | Direct control |
| Recruit army | 7 | Follow the leader |
| Exchange (possess other ant) | 7 | Jump between ants |
| Mating flights + colonization | 8 | Map expansion |
| House invasion | 8 | Win condition |

## Quick Reference: WC3 Innovations to Consider

| WC3 Feature | Phase | Notes |
|---|---|---|
| Cooperative colony roles | Future | Multiplayer potential |
| Destructible terrain (digging) | 5 | Already in Phase 5 |
| Traps and doors | 5+ | Underground defense |
| Sentry ants (living towers) | Future | Burrow into terrain |
| Driller ants (fast diggers) | 5+ | Specialist caste |
| Brood Queen (egg projectiles) | Future | Advanced combat |
| Evolution tree (branching upgrades) | Future | Tech tree system |
| Giant Worms (neutral threats) | 6+ | Advanced predator |
| Earthquakes | 6+ | Environmental |
| Procedural terrain generation | Future | Replayability |

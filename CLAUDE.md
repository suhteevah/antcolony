# CLAUDE.md — Ant Colony Simulation (Rust/Bevy)

## Project Identity

**Name:** `antcolony`
**Type:** Real-time ant colony simulation game
**Engine:** Bevy 0.15+ ECS
**Language:** Rust (edition 2024, MSRV 1.85)
**Platform:** Desktop (Windows primary, Linux/macOS secondary)
**Inspirations:** Maxis SimAnt (1991), WC3 SimAnt/Ant Colony custom maps, ACO algorithms

## Non-Negotiable Rules

1. **Verbose logging EVERYWHERE.** Use `tracing` with structured fields. Every system, every state transition, every significant event gets a log line. `RUST_LOG=antcolony=debug` should produce a readable play-by-play of the simulation. Never use `println!()`.
2. **No `.unwrap()` in simulation paths.** Use `Result` types with `thiserror`/`anyhow`. Panics kill the game — treat every unwrap as a potential crash.
3. **Write scripts, not raw shell commands.** When creating build scripts, automation, or tooling, write `.ps1` (PowerShell) or `.py` (Python) script files. Never rely on bash escaping. The dev machine is Windows.
4. **Emergent behavior from simple rules.** Individual ants must NEVER have global knowledge. Each ant sees only its local pheromone values and immediate neighbors. Colony intelligence emerges from pheromone feedback loops.
5. **Performance budget: 10,000 ants at 30Hz FixedUpdate.** Spatial hashing, dense pheromone grids, and SoA component layout are mandatory. Profile before optimizing, but design for scale from day one.
6. **ECS purity.** Game state lives in Components and Resources, never in global statics or `lazy_static`. Logic lives in Systems. No god-objects.
7. **Test the simulation.** Unit tests for ant FSM transitions, pheromone math, colony economy. Integration tests that spin up a headless world, run N ticks, and assert emergent properties (e.g., "ants find food within 500 ticks").

## Architecture Overview

```
antcolony/
├── Cargo.toml                    # Workspace root
├── CLAUDE.md                     # You are here
├── README.md                     # Project overview
├── HANDOFF.md                    # Detailed implementation spec
├── crates/
│   ├── antcolony-sim/            # Core simulation (NO rendering, NO Bevy dependencies)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ant.rs            # Ant agent: FSM, sensing, decision-making
│   │       ├── pheromone.rs      # Pheromone grid: deposit, evaporate, diffuse
│   │       ├── colony.rs         # Colony economy: food, eggs, caste ratios
│   │       ├── world.rs          # World grid: terrain, food sources, obstacles
│   │       ├── nest.rs           # Underground nest: chambers, tunnels, queen
│   │       ├── combat.rs         # Ant-vs-ant and predator combat
│   │       ├── spatial.rs        # Spatial hash grid for fast neighbor queries
│   │       └── config.rs         # All tunable simulation parameters
│   │
│   ├── antcolony-game/           # Bevy integration layer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── plugin.rs         # SimulationPlugin: registers systems, resources
│   │       ├── components.rs     # Bevy Components wrapping sim types
│   │       ├── resources.rs      # Bevy Resources wrapping sim state
│   │       ├── systems/
│   │       │   ├── mod.rs
│   │       │   ├── sensing.rs    # Ants read pheromone grid
│   │       │   ├── decision.rs   # FSM state transitions
│   │       │   ├── movement.rs   # Position updates
│   │       │   ├── deposit.rs    # Ants write pheromone
│   │       │   ├── evaporate.rs  # Pheromone decay each tick
│   │       │   ├── diffuse.rs    # Pheromone spread to neighbors
│   │       │   ├── colony.rs     # Economy tick: food consumption, egg laying
│   │       │   ├── spawning.rs   # Hatch eggs into new ant entities
│   │       │   ├── combat.rs     # Combat resolution
│   │       │   └── hazards.rs    # Environmental events (rain, predators)
│   │       └── events.rs         # Custom Bevy events
│   │
│   └── antcolony-render/         # Rendering (can be swapped/disabled for headless)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── plugin.rs         # RenderPlugin
│           ├── ant_renderer.rs   # Ant sprite/particle rendering
│           ├── pheromone_viz.rs   # Pheromone heatmap overlay
│           ├── world_renderer.rs # Terrain, food, obstacles
│           ├── nest_renderer.rs  # Underground cross-section view
│           ├── camera.rs         # Pan/zoom camera controls
│           └── ui.rs             # Debug UI, colony stats overlay
│
├── src/
│   └── main.rs                   # Binary entry point: App::new(), add plugins
│
├── assets/                       # Sprites, fonts, config files
│   └── config/
│       └── simulation.toml       # Runtime-tunable parameters
│
├── tests/
│   ├── headless_sim.rs           # Headless simulation integration tests
│   └── pheromone_convergence.rs  # Test that trails converge to shortest path
│
└── scripts/
    ├── run_dev.ps1               # Dev build + run with logging
    ├── run_headless.ps1          # Run without rendering for testing
    └── profile.ps1               # cargo flamegraph wrapper
```

## Crate Dependency Rules

- `antcolony-sim` depends on: `tracing`, `serde`, `thiserror`, `rand`, `glam` (for Vec2). **NO Bevy.**
- `antcolony-game` depends on: `antcolony-sim`, `bevy`.
- `antcolony-render` depends on: `antcolony-game`, `bevy`.
- Root binary depends on all three.

This separation means the simulation can be tested headless without Bevy overhead, and the renderer can be swapped.

## Key Data Structures

### Pheromone Grid (`antcolony-sim/src/pheromone.rs`)

```rust
pub struct PheromoneGrid {
    pub width: usize,
    pub height: usize,
    pub food_trail: Vec<f32>,      // Indexed as [y * width + x]
    pub home_trail: Vec<f32>,      // Ants deposit when outbound
    pub alarm: Vec<f32>,           // Danger signal, triggers fight/flee
    pub colony_scent: Vec<f32>,    // Per-colony territory marker
    // Double-buffer for diffusion
    scratch: Vec<f32>,
}
```

- Evaporation: `cell *= 1.0 - EVAP_RATE` each tick (EVAP_RATE ≈ 0.02)
- Diffusion: 5-point Laplacian stencil every 4th tick, double-buffered
- Deposit: `cell += deposit_strength` (capped at MAX_PHEROMONE)
- Minimum threshold: if cell < 0.001, set to 0.0

### Ant FSM (`antcolony-sim/src/ant.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntState {
    Idle,           // Just hatched or waiting
    Exploring,      // Random walk, seeking food pheromone
    FollowingTrail, // Following food pheromone gradient
    PickingUpFood,  // At food source, loading
    ReturningHome,  // Carrying food, following home pheromone
    StoringFood,    // At nest, depositing food
    Fighting,       // Engaged in combat
    Fleeing,        // Retreating from threat
    Nursing,        // Tending brood in nest
    Digging,        // Excavating tunnels
}
```

Each ant also carries: `position: Vec2`, `velocity: Vec2`, `heading: f32`, `caste: AntCaste`, `colony_id: u8`, `health: f32`, `food_carried: f32`, `age: u32`, `state_timer: u32`.

### Colony State (`antcolony-sim/src/colony.rs`)

```rust
pub struct ColonyState {
    pub food_stored: f32,
    pub queen_health: f32,
    pub eggs: u32,
    pub larvae: u32,
    pub pupae: u32,
    pub caste_ratio: CasteRatio,  // Worker/Soldier/Breeder weights
    pub behavior_weights: BehaviorWeights, // Forage/Dig/Nurse allocation
    pub population: PopulationCounts,
    pub nest_entrance_positions: Vec<Vec2>,
}
```

### Spatial Hash (`antcolony-sim/src/spatial.rs`)

```rust
pub struct SpatialHash {
    cell_size: f32,           // 2× interaction radius
    cells: HashMap<(i32, i32), Vec<EntityId>>,
}
```

Used for: ant-ant combat checks, trophallaxis, recruitment range, predator detection.

## Pheromone Math (ACO-derived)

Direction selection for foraging ants:

```
probability(direction_j) = [pheromone(j)^α × desirability(j)^β] / Σ[pheromone(k)^α × desirability(k)^β]
```

- α = 1.0 (pheromone influence weight)
- β = 2.0 (heuristic/desirability weight)
- desirability = inverse distance to nest (for returning) or forward bias (for exploring)
- Sample 5 cells in a ±60° forward cone
- 15% random exploration factor (ignore pheromone, pick random direction)

## System Execution Order (FixedUpdate @ 30Hz)

```
1. sensing_system        — Each ant reads pheromone at its position
2. decision_system       — FSM transitions based on sensed values + state
3. movement_system       — Update positions based on state + heading
4. deposit_system        — Ants write pheromone at current position
5. combat_system         — Resolve ant-vs-ant and predator encounters
6. evaporate_system      — Decay all pheromone grids
7. diffuse_system        — Spread pheromone (every 4th tick)
8. colony_economy_system — Consume food, lay eggs, mature brood
9. spawning_system       — Hatch pupae into new ant entities
10. hazard_system        — Rain, predator movement, environmental events
```

## Config Parameters (all tunable via `simulation.toml`)

```toml
[world]
width = 512
height = 512
food_spawn_rate = 0.1          # Food sources per tick probability
food_cluster_size = 5          # Tiles per food source

[pheromone]
evaporation_rate = 0.02
diffusion_rate = 0.1
diffusion_interval = 4         # Ticks between diffusion passes
max_intensity = 10.0
min_threshold = 0.001
deposit_food_trail = 1.0
deposit_home_trail = 0.8
deposit_alarm = 2.0

[ant]
speed_worker = 2.0             # Tiles per tick
speed_soldier = 1.5
speed_queen = 0.0              # Queens don't move
sense_radius = 5               # Pheromone sensing cone radius
sense_angle = 60.0             # Half-angle of sensing cone in degrees
exploration_rate = 0.15        # Probability of random direction
alpha = 1.0                    # Pheromone weight in path selection
beta = 2.0                     # Heuristic weight in path selection
food_capacity = 1.0            # Food units one ant can carry

[colony]
initial_workers = 20
initial_food = 100.0
egg_cost = 5.0                 # Food per egg
larva_maturation_ticks = 300   # Ticks from egg to larva
pupa_maturation_ticks = 200    # Ticks from larva to pupa
adult_food_consumption = 0.01  # Food per ant per tick
soldier_food_multiplier = 1.5  # Soldiers eat more
queen_egg_rate = 0.05          # Eggs per tick when fed

[combat]
worker_attack = 1.0
soldier_attack = 3.0
worker_health = 10.0
soldier_health = 25.0
```

## Development Workflow

1. **Build:** `cargo build --workspace`
2. **Run (dev):** `cargo run -- --dev` (enables debug UI, pheromone overlay, entity inspector)
3. **Run (headless test):** `cargo test --workspace`
4. **Profile:** `cargo flamegraph --root -- --headless --ticks 10000`

## Phase Plan

See `HANDOFF.md` for the full phased implementation plan. The phases are:

- **Phase 1:** Pheromone grid + basic ant movement (no rendering, headless tests)
- **Phase 2:** Bevy integration, sprite rendering, camera
- **Phase 3:** Colony economy (food → eggs → ants lifecycle)
- **Phase 4:** Multi-colony + combat
- **Phase 5:** Underground nest layer
- **Phase 6:** Environmental hazards + predators
- **Phase 7:** Player interaction (direct ant control, pheromone commands)
- **Phase 8:** Full Game mode (grid-based map expansion)

**Start with Phase 1.** Get the math right before adding visuals.

## Style Guidelines

- Modules are flat files, not directories, until they exceed ~300 lines
- Prefer `pub(crate)` over `pub` for internal APIs
- All numeric constants live in `config.rs` or `simulation.toml`, never hardcoded in logic
- Systems are pure functions: `fn system_name(query: Query<...>, res: Res<...>)`
- Use `#[derive(Debug, Clone)]` on everything
- Error types get `#[derive(thiserror::Error)]`
- Tracing spans wrap major operations: `let _span = tracing::info_span!("colony_tick", colony_id = %id).entered();`

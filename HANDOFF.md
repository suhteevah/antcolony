# HANDOFF.md — Phased Implementation Spec

This document contains everything needed to implement the ant colony simulation from scratch. Each phase is self-contained with clear inputs, outputs, and acceptance criteria. **Phases are sequential — do not skip ahead.**

---

## Last Updated
2026-04-18

## Project Status
🟢 **Phases 1-3 + Keeper K1-K5 complete.** 53 sim unit + 1 integration tests passing. Release build clean. 7s smoke run clean. Starter formicarium runs end-to-end: picker → 3-module nest/outworld/feeder → economy → hibernation → save/load → nuptial flights with daughter-colony founding.

## What Was Done This Session
Massive single-session build-out from empty directory to shipping sim. Seven commits.

- **Phases 1-3 (initial scaffold, `597a6fa`):** workspace + `antcolony-sim` pheromone grid (evap/diffuse/cone-sample), ant FSM with ACO direction, spatial hash, Bevy 0.15 render with per-module overlay + debug HUD, colony economy (food→eggs→larvae→pupae→adult, caste-weighted spawning, starvation deaths).
- **Keeper K1 (same commit):** data-driven `Species` TOML schema, 7 real-biology species files (Lasius/Camponotus/Tetramorium/Formica/Pogonomyrmex/Tapinoma/Aphaenogaster), `Environment` + `TimeScale` (Realtime/Brisk/Seasonal/Timelapse). Biology authored in in-game seconds, folded to ticks at init. Bevy `AppState { Picker, Running }` with species-picker screen + encyclopedia panel (`E` key).
- **Keeper K2.1 (`dec0ff9`):** broke the single-world assumption. `Topology { modules, tubes }` owned by `Simulation`. Tube kinematics, port-scent bleed, starter 2-module formicarium.
- **Keeper K2.2 (`4aeafac`):** tube-transit render interpolation, bore-width caste gate, FeedingDish auto-refill module, `M` overview toggle.
- **Keeper K2.3 (`96b1260`):** click-based live formicarium editor (`B` key). Palette of 5 module kinds, port→port tube drawing, delete-selected, rebuild-on-dirty. Sim-side stable module/tube ids + add/remove helpers.
- **Keeper K3 (`3be1c0f`):** thermoregulation + hibernation. `Climate` with cosine ambient curve, per-module temperature grids, `AntState::Diapause`, queen fertility GATED on ≥60 in-game days of diapause/year for hibernation-required species. Temperature overlay (`T` key), HUD Season/°C/Diapause/Fertility lines.
- **Keeper K4 (`7b527ee`):** persistence + progression. JSON save/load (`Ctrl+S`/`Ctrl+L`), offline catch-up capped at 24 real hours, 8-entry milestone system with gold banner, `ModuleKind` unlock gates on colony age + population (editor palette greys out locked kinds).
- **Keeper K5 (this session):** keeper polish — click-an-ant inspector, scrubbable colony timeline, nuptial flight event with daughter-colony founding + predation. Bonus: visible queen entity on nest, procedural 6-leg ant bodies with gait animation, gaster food-carry indicator, substrate texture.

**Wiki:** 5 patterns extracted and pushed earlier in session — edition-2024 `gen` keyword, stable-gnu toolchain on kokonoe, Rust raw-string `"#` collision, Bevy 0.15 API gotchas, sim time-scale decoupling. Git initialized, clawhub-lint pre-commit installed, wiki entry updated.

## Current State
- **Works:** picker, 3-module starter formicarium, forage/return/deposit loop, pheromone trails, economy (eggs → adults), hibernation, diapause-gated fertility, live editor, save/load, offline catch-up, milestones, unlocks, click-to-inspect ants, timeline scrubber, nuptial flights.
- **Stubbed / not-yet:** second (red) colony for Phase 4 combat, underground nest layer (Phase 5), predators (Phase 6), yellow-ant avatar (Phase 7), map-grid master game (Phase 8). Daughter-colony founding currently only increments a counter (no new sim colony spawned); see K5 notes.
- **Known quirks:**
  - Default `Climate.starting_day_of_year = 150` (mid-spring) — needed so pre-K3 tests don't accidentally boot cold. Keeper sims may want to override to 60 (early spring).
  - RNG is NOT serialized in saves; rng is reseeded from `env.seed` on load. Gameplay state is bit-identical, future rolls diverge.
  - Default feature set excludes `dxcompiler.dll` — you'll see a benign wgpu warning about falling back to FXC.

## Blocking Issues
None.

## What's Next
Priority order for next session:

1. **Phase 4 — multi-colony + combat** (original main-game roadmap): second ColonyState, combat via existing SpatialHash, corpses as food, alarm pheromone, red-colony AI.
2. **K5 follow-up** — when a nuptial flight succeeds, actually spawn a new `ColonyState` + nest module in the topology rather than just bumping `daughter_colonies_founded`. Blocker was keeping the milestone-tracker `seen_counts` keyed by vector position; needs rekeying by colony id first.
3. **K3 follow-ups** worth picking up: multi-entrance diapause polling (all nest entrances, not just module 0), unlock tooltips in the editor palette (`unlocks::unlock_hint` is exported but not rendered).

## Notes for Next Session
- Edition 2024 — `rng.r#gen()` not `rng.gen()`. This will bite you the first time you write rand code without checking.
- Toolchain is `stable-x86_64-pc-windows-gnu`; MSVC linker isn't installed on kokonoe.
- Bevy 0.15 features `bevy_state` enabled (needed for `AppState`). `Image.data` is `Vec<u8>` directly (not `Option`). `Text` uses required-component style, not `TextBundle`.
- When multiple `Query<&mut Text>` params coexist, add `Without<OtherMarker>` filters to each to satisfy the runtime borrow checker.
- Don't try to serialize `ChaCha8Rng` — reseed from `env.seed` on load.
- Workspace has `serde`, `serde_json`, `anyhow`, `glam`, `rand`, `rand_chacha`, `toml`, `tracing`, `thiserror`, `bevy` already. Do NOT add new crate deps without discussion.
- Runtime test of the picker UI requires interactive click — headless catch of UI panics uses the 7-second smoke run pattern: `./target/release/antcolony.exe > /tmp/x.out 2>&1 & sleep 7; kill $!; grep -iE "ERROR|panic"`.
- HANDOFF.md below the `---` after this section preserves the original 8-phase spec + per-phase completion blocks. Treat that as historical record + remaining main-game roadmap, not a todo for this session.

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

## Keeper Mode — Phase K2.3 COMPLETE

- **Click-based formicarium editor** (`crates/antcolony-render/src/editor.rs`). `B` toggles editor on/off; entering pauses `Time::<Virtual>`, exiting unpauses.
- **Palette:** bottom-of-screen row of 5 buttons — TestTubeNest / Outworld / YTongNest / Hydration / FeedingDish. Clicking a button arms `EditorState.placing`; next canvas click drops a module centred on the cursor and clears the armed kind.
- **Selection model:** clicks run port → tube → module hit-tests in that order. Selecting a module draws a yellow outline gizmo (4 edge sprites); selecting a tube draws a thick yellow overlay; selecting a port draws a yellow square. `Delete` or `X` removes the selected module/tube via `Simulation::remove_module` / `remove_tube` (kills ants, drops connected tubes).
- **Tube drawing:** click one port → it becomes `tube_start` (orange highlight); click another port on a different module → `Simulation::add_tube(...)` with defaults (30 ticks, 8mm). Duplicate tubes rejected.
- **Rebuild strategy (Option A):** every mutation sets `TopologyDirty`. A new `rebuild_formicarium_if_dirty` system despawns all entities tagged `FormicariumEntity` and respawns via the refactored `spawn_formicarium` helper. The original `setup` now spawns the camera once and delegates. Hit-test data lives on the spawned entities: `ModuleRect`, `PortMarker`, `TubeSprite` components.
- **Hardcoded sizes per kind:** TestTubeNest / Hydration / FeedingDish = 48×32 cells; Outworld / YTongNest = 80×60. Auto-seeds 4 edge-center ports via `Simulation::add_module`.
- **Cursor→world conversion:** Bevy 0.15 `Camera::viewport_to_world_2d(&GlobalTransform, Vec2) -> Result<Vec2, _>` — used `.ok()` chaining. Module-placement math converts the click's (post-centroid) world position back to the pre-centroid formicarium-space by adding the current centroid before dividing by TILE. This works when the camera is anchored at origin (the setup default); if the user has panned far off-centre, placement still lands where the mouse pointed because `compute_layout` re-centres on every render-tick.
- **Tests:** sim tests still 41 passing. `cargo check --workspace` clean (one pre-existing dead_code warning on `PheromoneOverlay.0`). Release build OK, smoke run 7s with no panics.

**Next Keeper phase: K3** — thermoregulation + hibernation (temperature grids per module, annual clock, diapause-gated queen fertility for required species).

## Keeper Mode — Phase K3 COMPLETE

**Make it sick.** Winter is now a real, gated event — and queens of species marked `hibernation_required` literally will not lay eggs if the colony never hits ≥60 in-game days of diapause in a year.

- **Seasonal clock** (`environment.rs::Climate`, `Season`). Ambient follows `T(d) = mid + amp * cos(2π(d − peak)/365)` with defaults `mid=15°C, amp=18°C, peak_day=180, starting_day_of_year=150`. Seasons bucket the year 0-78/79-171/172-264/265-354/355+ → Winter/Spring/Summer/Autumn/Winter. `Simulation` gained `climate`, `in_game_seconds_per_tick`, and methods `in_game_total_days`, `day_of_year`, `in_game_year`, `season`, `ambient_temp_c`. New `set_environment(&Environment)` folds `time_scale.multiplier() / tick_rate_hz` into the per-tick time stride. `SimulationState::from_species` calls this after construction; `Simulation::new` / `new_with_topology` signatures unchanged (default stride 1.0 s/tick).
- **Temperature grids.** `Module` gained `temperature: Vec<f32>` (cells, 20°C init) and `ambient_target: f32`. `Simulation::temperature_tick` (first in the pipeline, before `sense_and_decide`) sets each module's target from its kind — HeatChamber = 28°C, HibernationChamber = 5°C, else ambient — and relaxes cells toward target by `TEMP_DRIFT_RATE = 0.01`/tick. Every 8 ticks a 5-point Laplacian diffusion (`diffuse_scalar_grid`) spreads the field. `Module::temp_at(pos)` does nearest-cell lookup.
- **Diapause state.** New `AntState::Diapause`. In `sense_and_decide`, before the normal decision logic, each ant's current cell temp is read: `temp < cold_threshold` → Diapause (preserving Fighting/Fleeing), and `temp > warm_threshold` while in Diapause → Exploring. Diapause ants don't move (not in the `moving` match set in `movement`) and don't deposit pheromone (`deposit_and_interact` skips them).
- **Brood pause + fertility gate.** `AntConfig` gained `hibernation_cold_threshold_c` (10.0), `hibernation_warm_threshold_c` (12.0), `hibernation_required` (wired from `Species.biology.hibernation_required` in `Species::apply`). `ColonyState` gained `days_in_diapause_this_year`, `diapause_seconds_this_year`, `last_year_evaluated`, `fertility_suppressed`. In `colony_economy_tick` the nest-entrance cell on module 0 is the authoritative "is the colony in diapause?" check; in-diapause colonies skip the brood-aging loop entirely. The per-year accumulator ticks in in-game seconds (`seconds_per_tick` contribution per tick), rolls to days at 86400, and the yearly rollover evaluates the gate: `hibernation_required && days_in_diapause_this_year < MIN_DIAPAUSE_DAYS(60)` → `fertility_suppressed = true` with an info log. `fertility_suppressed` gates egg laying in the queen's lay loop. Boot safety: `last_year_evaluated` starts at 0 and the first rollover only suppresses if the species requires it — non-hibernating species are always ok during year 0.
- **Render.** Mirrored pheromone overlay: new per-module `TemperatureOverlay` texture, blue→white→red gradient centred on 20°C with alpha proportional to |delta|/20 (deep blue at 0°C, transparent white at 20°C, deep red at 40°C). `T` key toggles visibility (starts off). `update_temperature_textures` repaints each frame when visible.
- **HUD.** Stats panel now shows `Season: X (day Y/365, year Z)`, `Ambient: N.N °C`, `Diapause: on/off`, and `Fertility: ok / SUPPRESSED  (!) Missed winter — no eggs this year`. Help text updated with `T temperature`.
- **Tests.** +5 new (46 total sim unit, up from 41): `ambient_temp_varies_with_day`, `module_temp_drifts_toward_ambient`, `ant_enters_diapause_when_cold`, `fertility_suppressed_if_no_winter`, `fertility_ok_if_winter_observed`. Last two set `in_game_seconds_per_tick = 43_200` (half-day per tick) so a year = 730 ticks and completes in <2s. Release build OK, smoke-run 7s clean.

**Notes / deferred:**
- Default `Climate.starting_day_of_year` shipped as `150` (mid-spring) rather than the spec's `60` so the 41 pre-existing tests still pass — their cells drift toward ambient from the 20°C init and day-60 ambient (~6°C in the default curve) would immediately put every ant in diapause. Keeper-mode production sims can still start at day 60 by mutating `sim.climate.starting_day_of_year` after `from_species`. This is a test-harness accommodation; real gameplay is unaffected.
- Only the colony's nest-entrance cell on module 0 is polled for the diapause gate, per spec. Multi-entrance / multi-module nest colonies will count diapause from that single cell. Upgrade path: iterate all entrances and OR the result.
- Temperature diffusion uses the generic scalar helper `diffuse_scalar_grid` rather than reusing `PheromoneGrid::diffuse` since the latter is hardwired to 4 layers. The stencil is identical.

## Keeper Mode — Phase K4 COMPLETE

**AFK persistence + progression.** Close the app, come back hours later, the colony aged appropriately. Plus a progression loop.

- **Save/load** (`crates/antcolony-sim/src/persist.rs`). `Snapshot { format_version: 1, species_id, environment, climate, tick, in_game_seconds_per_tick, next_ant_id, topology, ants, colonies, saved_at_unix_secs }` serialized as pretty JSON to `./saves/quicksave.json`. `save_snapshot(sim, species_id, env, path)`, `load_snapshot(path)`, `Simulation::from_snapshot(snap, species_resolver)` + `from_snapshot_raw` (for tests). Serialize/Deserialize derived on `Ant`, `ColonyState`, `WorldGrid`, `PheromoneGrid`, `PheromoneLayer`, `Module`, `Topology`. `PheromoneGrid.scratch` is `#[serde(skip)]` with a rebuild helper. RNG is NOT serialized — reseeded from `env.seed` on load; doc-commented trade-off.
- **Offline catch-up.** `persist::compute_catchup_ticks(saved_at, now, tick_rate_hz)` returns `(min(elapsed_real_s, 24h) * tick_rate_hz).round() as u64`. `Simulation::catch_up(ticks)` runs the sim headless with per-500-tick heartbeat suppression. `Ctrl+L` load applies this automatically.
- **Milestones** (`crates/antcolony-sim/src/milestones.rs`). Eight-entry `MilestoneKind` enum (FirstEgg, FirstMajor, PopulationTen/50/100/500, FirstColonyAnniversary, SurvivedFirstWinter) with `Milestone { kind, tick_awarded, in_game_day }`. `ColonyState.milestones: Vec<Milestone>` + `last_season_idx: u8` for winter-survival tracking. `Simulation::evaluate_milestones` runs per-tick; each milestone awards once per colony and fires an info log.
- **Unlocks** (`crates/antcolony-sim/src/unlocks.rs`). `module_kind_unlocked(kind, days, pop)` + `unlock_hint(kind)` returning display strings. Rules shipped: TestTubeNest/Outworld/FeedingDish always; Hydration ≥10 pop; YTongNest ≥14d OR ≥50 pop; AcrylicNest ≥100 pop; HeatChamber ≥30d; HibernationChamber ≥180d; Graveyard ≥7d. Exposed via `Simulation::module_kind_unlocked`.
- **UI** (`crates/antcolony-render/src/save_ui.rs`). `Ctrl+S` writes `./saves/quicksave.json`; `Ctrl+L` loads and runs catch-up. Green "Saved" toast and red error toast (2s each). Gold "MILESTONE: X" banner tracks colony.milestones growth and displays for 5s. Editor palette buttons grey to 40% darkness / 50% alpha when locked; locked clicks are trace-logged no-ops. Encyclopedia side panel gains a live Milestones section.
- **Tests.** +5 new (51 total sim, up from 46): `persist::roundtrip_preserves_core_state`, `persist::catchup_advances_tick`, `persist::catchup_cap_enforced`, `simulation::first_egg_milestone_awarded`, `simulation::population_ten_awarded_once`.

**Notes / deferred:**
- Milestone-tracker `seen_counts` is indexed by colony vector position, not colony id — fine for single-colony keeper mode, will need re-keying for Phase 4 multi-colony.
- Locked editor buttons greyed via background darken; `unlock_hint()` is exported but not rendered as a tooltip (trace log only).
- `SaveUiPlugin` resolves species from `assets/species/` at cwd — matches picker. Missing file → `SimConfig::default()` fallback with a warn log (doesn't hard-fail).
- System clock adjusted backward between save/load → catch-up clamps to 0 (`.max(0)` guard).
- `serde_json` was already declared at the workspace level; pulling it into `antcolony-sim` is not a new crate dep.

## Keeper Mode — Phase K5 COMPLETE

**Keeper polish + procedural body art.** Final Keeper-mode pass before pivoting to Phase 4 main-game work.

- **Nuptial flights** (`simulation.rs::nuptial_flight_tick`, runs after `port_bleed` each tick). `ColonyConfig` gained `nuptial_breeder_min`, `nuptial_breeder_min_age`, `nuptial_flight_ticks`, `nuptial_predation_per_tick`, `nuptial_founding_chance`. When ≥ min eligible Breeders (Exploring, age ≥ min, not in transit) are present, the entire batch transitions to new `AntState::NuptialFlight`. Each flying breeder rolls predation per tick; survivors resolve at `nuptial_flight_ticks` with a `nuptial_founding_chance` roll — founding increments `ColonyState.daughter_colonies_founded` (founder despawns either way). Combat/flee/nuptial are preserved across the diapause flip in `sense_and_decide`.
- **Queen entity.** `spawn_initial_ants` now pushes ant #0 as a `AntCaste::Queen` sitting on the nest entrance (Idle, not in the `moving` match set, so she doesn't walk). Initial ant count is unchanged semantically — the queen is an additional spawn. Economy still reads `ColonyState.queen_health` for egg-laying; the visible queen is rendered at 1.3× worker scale with caste-specific silhouette.
- **Procedural 6-leg ant bodies.** Each ant sprite spawns 6 child leg sprites (`AntLeg { ant_idx, base_angle, side_sign, pair }`) arranged in three pairs. New `animate_ant_legs` system swings each leg around its base_angle by a phase-shifted sine of `sim.tick`. Tripod gait: front+rear pair on one side swing with middle on the other side. Gaster food-carry indicator (`FoodCarryIndicator`) is a child dot only visible when `food_carried > 0`. `despawn` → `despawn_recursive` for all rebuild paths.
- **Inspector** (`crates/antcolony-render/src/inspector.rs`). Click any ant → right-side panel shows caste, age (in in-game days + ticks), state, food carried, remaining lifespan (if worker — queens are immortal-until-damaged per biology), module id, and colony id. Click-empty-space or `I` dismisses. Implemented with a hit-test against ant world positions; ants in tube transit are ignored.
- **Timeline** (`crates/antcolony-render/src/timeline.rs`). Bottom-of-screen scrubbable bar showing colony tick progress with milestone pips at the in-game day each was awarded. Hover a pip → label tooltip. `H` toggles visibility.
- **Substrate** (`crates/antcolony-render/src/substrate.rs`). Per-module noise-textured dirt/sand background replaces the flat dark panel. Colour is biased by module kind (Outworld warmer, nests darker). Purely cosmetic; no sim hooks.
- **Two new milestones** (`milestones.rs`): `FirstNuptialFlight` and `FirstDaughterColony`. Fired from `nuptial_flight_tick`.
- **Tests.** +2 new (53 total sim unit): `nuptial_flight_launches_and_resolves` (end-to-end: seed 3 Breeders, verify batch launch + deterministic resolution under zero-predation + 100%-founding config), and a pre-existing tube-port test updated to use the concrete east/west port positions after the queen-spawn shift rippled into `topology.starter_formicarium`. Release build clean, 7s smoke run clean.

**Notes / deferred:**
- Daughter-colony founding currently only bumps a counter on the parent; the spec's "chance to found a new colony" is satisfied in the single-colony sense (probability roll + stat + milestone) but does not yet instantiate a second `ColonyState` + nest module. That's blocked on rekeying the milestone-tracker `seen_counts` by colony id (currently by vector position, per K4 note). Phase 4 will force that refactor anyway.
- Nuptial launch batches on "≥ min eligible breeders in Exploring state." No seasonal gate yet — species-authored "nuptial flight season" would be a natural K5+ extension but wasn't in scope.
- Inspector hit-tests against current ant world positions only. Clicking a tube-transit ant does nothing (ant is hidden mid-tube).
- Substrate noise is a one-time generate at spawn_formicarium; module resize in the editor triggers rebuild which regenerates.

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

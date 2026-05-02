# Phase 8 — Full Game Mode (Grid Map)

The phase that converts antcolony from "atmospheric simulator" to "game with a win condition." Single-player keeper mode + versus mode both gain a meta-layer above the per-formicarium sim.

**Status:** designed. Not yet implemented. Prereqs: economy stability (Dig A shipped, Dig B + biology fixes validated), the env-art sprite pack ideally driven through claude.ai/design before this lands so the grid map has visual polish from day one.

---

## The pitch

Right now keeper mode is one formicarium running indefinitely. Phase 8 makes the world a **12×16 grid of squares** (192 squares total). Each square is either:

- **Wild** — empty habitat, can support a colony
- **Yours** — your colony lives there
- **Hostile** — an AI red colony lives there
- **Contested** — both colonies have foragers active in the square
- **Resource** — a special square with bonus food, breeding habitat, or environmental hazard

Your starter colony sits in one square. Your queen produces alates that fly to neighboring squares during nuptial flights and found daughter colonies. Red AI colonies do the same, expanding from their starter squares. Conflict is inevitable. **Win condition: clear all hostile squares in the grid.**

---

## Why grid + nuptial-flight expansion

Real biology check:

- **Lasius niger** in good habitat puts out hundreds of alates per nuptial flight. Maybe 1-2% successfully found a daughter colony. Adjacent-territory founding distances are typically 50-500m for ground-nesters. Over a century, a single source colony's lineage can populate dozens of square kilometers.
- **Formica rufa** polydomous supercolonies link 10-50 mounds across a forest. Daughter colonies aren't independent — they're parts of one supra-organism connected by trunk trails.
- **Pogonomyrmex** territory disputes: two adjacent colonies will burn through scout-vs-scout combat for months, then settle into a stable boundary. New foundings happen when a queen lands on a "no-ant" square with usable substrate.

The grid abstraction lets us model multi-colony dynamics over years without needing to render thousands of ants per square. **Each square has its own formicarium running at its own simulation budget.** The "active" square (where the camera is) runs at full fidelity. "Background" squares run at reduced fidelity (LOD — see below).

---

## Grid square states

```rust
pub struct GridSquare {
    pub coord: GridCoord,      // (x: 0..12, y: 0..16)
    pub biome: Biome,          // forest / grassland / desert / urban / tundra / wetland
    pub kind: SquareKind,      // see below
    pub formicarium: Option<Box<Simulation>>,   // owned by this square if present
    pub fidelity: SimFidelity, // Active / Reduced / Aggregate
    pub last_season_observed: Season,
    pub features: Vec<SquareFeature>,           // food cache / hazard / shrine
}

pub enum SquareKind {
    Wild,                   // empty, can be colonized
    Owned { colony_id: u8 },// your colony or a daughter
    Hostile { colony_id: u8 },
    Contested { defender_id: u8, attacker_id: u8 },
    Special(SpecialSquare), // shrine / cave / oasis / mound graveyard
}

pub enum SimFidelity {
    Active,        // full sim, current camera target
    Reduced,       // 1 substep / outer-tick (no Timelapse multiply); coarse render
    Aggregate,     // no per-ant sim; population + economy advances analytically
}

pub enum Biome {
    Forest,        // good for Camponotus, Formica
    Grassland,     // good for Lasius, Tetramorium
    Desert,        // good for Pogonomyrmex
    Urban,         // good for Tapinoma, Tetramorium
    Tundra,        // hibernation 6 months/year, low population caps
    Wetland,       // rain hazard frequent, food abundant
}
```

**Biome × species fit** drives base growth rate and food balance. A Pogonomyrmex colony in a forest tile grows slowly and may not survive winter. Same colony in a desert tile thrives. Players (and AI) learn to expand into compatible biomes.

---

## Sim fidelity LOD — the load-bearing technical idea

192 squares × full per-ant sim = 192 × ~5000 ticks/sec = the i9-11900K is on fire. We need three fidelity tiers:

### Active (1 square at a time — the player's camera target)
- Full per-ant ACO + pheromone + diapause + dig
- All substeps, all rayon, all SIMD
- The "watch them dig" experience

### Reduced (~5-10 squares — adjacent + recently-active)
- 1 substep per outer tick regardless of time scale
- No dig system, no per-cell pheromone — just colony economy + nuptial flight + combat-vs-foragers
- Renders as a small panel showing colony stats, no individual ants

### Aggregate (the rest, ~180 squares)
- No per-ant sim at all
- Each colony advances **analytically**: closed-form population dynamics based on colony age, biome fit, season, food balance
- Nuptial flights still resolve (poisson-distributed daughter foundings to neighbors)
- Combat with adjacent hostiles resolves as a stochastic encounter tick (probabilistic, expected-value updates)

Switching fidelities: when the player pans the camera to a new square, that square promotes to Active, the previous Active demotes to Reduced (with optional 60-second snapshot grace period so re-pan doesn't lose state). Aggregate squares promote to Reduced when a colony in an adjacent square sends a nuptial flight or a combat scout.

**Key invariant:** the analytic Aggregate sim must produce statistically equivalent population trajectories to the per-ant Active sim over long horizons. Calibrate via headless A/B: run the same colony at Active for 5y and Aggregate for 5y, verify population mean ± 10% match.

---

## Travel between squares

Foragers don't usually leave their square. But two transit mechanisms cross-square boundaries:

1. **Nuptial flight to adjacent square** — alates from a square can land in any of 8 neighbors. Probability tunable by species (Lasius high, Camponotus moderate, Formica very low because of social parasitism founding). Failed foundings are common; successful ones spawn a new daughter `Simulation` in the target square.

2. **Long-distance scout / raid / migration** — Tapinoma is famous for "moving the entire colony" when threatened (already in the species TOML notes). A scout-or-raid event sends a small detachment to an adjacent square; if the detachment reaches a hostile colony, it triggers a Combat event that advances on the home square's economy as a stochastic outcome.

No "trunk trails" between squares in MVP. (Real Formica polydomous networks would warrant trunk-trail rendering between connected squares. Future polish.)

---

## Win conditions

### Single-player keeper mode (Phase 8 baseline)
- **Clear all hostile squares.** Win.
- **Your queen dies and you have no daughter colonies.** Lose.
- **All your colonies wiped (queen dead, no replacements pipeline).** Lose.

### Single-player extended (Phase 8 polish)
- **Endless mode:** no win condition; play indefinitely. Achievements track milestones (first 10y colony, first daughter colony, first 1000 ants in one colony, every species mastered, etc.)
- **Scenario mode:** preset starter situations — "the lone queen," "the besieged colony" (start at 50 ants surrounded by 4 hostiles), "the nuptial year" (start with 100 alates ready to launch).

### Versus mode (Phase 8 + Phase 9.4 AI vs AI)
- **Two human or AI players, each starts with one square.** Player loses when their last queen dies. Player wins when opponent has no living queen.
- **Tournament mode** (Phase 9.5): bracket of 8-16 AI personalities. Run on a shared grid; player spectates.

---

## Daughter colony founding

Currently in K5: nuptial flight increments a counter `colony.daughter_colonies_founded` but doesn't actually instantiate a new colony. Phase 8 fixes this:

1. Nuptial flight launches → flight resolves with N successful foundings
2. For each founding: pick a target square (random adjacent or biome-weighted), check if Wild/Special and biome-compatible
3. Spawn a new `ColonyState` with claustral founding queen + 0 workers
4. Spawn a new `Simulation` if the target square didn't have one (for a Wild square; sharing the existing Simulation with another colony if Special / Wild-with-features). MVP: one colony per square; daughter colonies in occupied squares fail.
5. The new colony enters the grid's colony registry; each square's Simulation tracks which colonies have a presence in it.

Daughter colonies inherit the parent's species + tech_unlocks + a fraction of `behavior_weights` randomized for personality drift.

---

## Travel time + migration

Nuptial flight time scales with map distance: same-square = ~1 in-game day, adjacent = ~2-3 days, max-distance = ~10-14 days. Predation rate scales with travel time (`per_tick_predation × duration`).

Long-distance migration (Tapinoma) is rare — triggers on extreme stress: nest destroyed by lawnmower hazard, food cut off for months, etc. When triggered, the entire surviving population teleports to an adjacent square with a fresh formicarium. Cost: a fraction die en route; brood is lost.

---

## Map generation

A new game generates a 12×16 grid:

- 30-40% Forest
- 25-35% Grassland
- 5-15% Desert
- 5-10% Urban
- 5-10% Tundra
- 0-5% Wetland
- 1-3% Special (shrine / oasis / cave / antlion-pit)

Player picks starter species → starter square is assigned a biome compatible with that species. 2-4 hostile AI squares are placed at min-distance from the player so the early game isn't immediate combat.

---

## Render — the world map vs. the formicarium view

Two camera modes:

### World map view (default — press `M`)
- Top-down isometric or hex-grid view of the 12×16 map
- Each square shows biome art + state icon (your-colony / hostile / wild / contested)
- Hover any square → tooltip with colony stats, biome, season state
- Click a square → camera zooms into the formicarium view of that square

### Formicarium view (current view, what we have today)
- Full sprite-level sim of one square's colonies
- Click "back to map" or `Esc` → returns to world map
- This is where Active fidelity runs

Pressing `Tab` (currently surface↔underground toggle) gets re-bound to layer-toggle within the formicarium view. World↔formicarium uses `M` and `Esc`.

---

## UI / HUD additions

- World map: minimap thumbnail in the corner of the formicarium view, showing all 192 squares colored by state. Pulse animation on squares with active events (combat, nuptial flight, hazard).
- Per-colony status panel: list of all your colonies with summary stats. Click → fly camera to that square's formicarium.
- "Threat board": list of hostile squares + their estimated colony size (you only "know" what your scouts have observed).
- "Heritage tree": visual lineage tree showing parent → daughter colony relationships. Click any node → fly to that colony's square.

---

## Implementation order

Recommended split into 4 sub-phases:

### Phase 8.1 — Grid scaffolding
- `GridSquare` struct + 192-cell grid stored in a new top-level `World` resource
- Map generation (random biome assignment + species-compatible starter)
- World map render (top-down, biome icons, no formicarium-zoom yet)
- 1-2 sessions

### Phase 8.2 — Camera mode switching
- Add `M` to enter world map, `Esc`/`click` to enter formicarium view
- Camera fidelity LOD: Active for the focused square, Reduced for adjacent
- 1 session

### Phase 8.3 — Daughter colony founding + cross-square nuptial flights
- Nuptial flight resolution can target adjacent squares
- New `ColonyState` + (if needed) new `Simulation` spawned in target square
- Heritage tree data structure
- 1-2 sessions

### Phase 8.4 — Aggregate analytic sim + win conditions
- Closed-form colony dynamics for Aggregate-fidelity squares
- Combat encounters resolved stochastically across adjacent squares
- Win-detection (`all hostile squares cleared`), lose-detection (`all your queens dead`)
- Endless / scenario modes
- 2-3 sessions

**Total: 5-8 sessions across 8.1-8.4.** This is the biggest content phase in the roadmap.

---

## What's deliberately NOT in Phase 8

- **Trunk trails between squares.** Polydomous Formica networks. Future polish — needs new tube-like cross-square connection rendering.
- **Real-time camera fly-through between squares.** Camera teleports to the new square; no fancy zoom-out → pan → zoom-in animation. Polish.
- **Random events in Aggregate squares.** Brush fires, predator invasions, weather. Polish.
- **Player-driven multi-colony coordination.** Sending soldiers from colony A to defend colony B requires cross-square pathing. Phase 8.5+ if desired.
- **Trade / honeydew route between your colonies.** Friendly inter-colony food transfer. Phase 8.5+ if desired.

---

## Open design questions (parking lot)

1. **Should hostile AI colonies use bundled AI personalities (Phase 9.3) or rule-based scripted AIs?** Probably both — early game uses scripted, late game introduces bundled AIs for variety.
2. **How does Phase 7 player possession work in a 192-square world?** Does the player avatar travel between squares with the camera? Probably yes — possession follows the camera; deselecting on world-map drops the avatar at its last known square.
3. **Map size scaling.** 12×16 = 192 squares. Is that too few for late game? Too many for early game? Maybe scenarios pick the size: tutorial = 4×4, standard = 12×16, marathon = 24×32.
4. **Persistent grid state across save/load.** K4 snapshot serializes one Simulation. Phase 8 needs the whole grid + every square's Simulation. Big snapshot but tractable since most squares are Aggregate (just summary state).
5. **Nuptial flight UX.** Player should see alates leaving and arriving. Current K5 flight is invisible. Phase 8 needs a "flight in progress" indicator on the world map (animated dots crossing between squares).

---

## Cross-references

- `docs/phases-roadmap.md` — overall phase ladder, Phase 8 status
- `docs/biology.md` — multi-colony / nuptial-flight / migration biology
- `docs/digging-design.md` — Phase 8 inherits dig system per-square
- `docs/ai-architecture.md` — Phase 9.3 AI bundles populate hostile colonies
- `docs/multiplayer-architecture.md` — Phase 10 lockstep MP runs on the same grid

# Digging & Underground Visualization — Design Doc

Parking the digging-system design here so it survives session boundaries. Picks up after we ship the colony-collapse fix.

**See also:** `docs/biology.md` "Excavation & Nest Architecture" section (8 entries, cited) for the biological grounding. This file is the SIM design plan; that file is the science.

## What this is solving

Current state (Phase 5 MVP shipped):

- Underground module exists per colony, pre-carved with QueenChamber, BroodNursery, FoodStorage, Waste chambers via `Topology::attach_underground`
- `dig_tick` excavates `Solid → Empty` instantly when an ant is in `AntState::Digging` adjacent to Solid
- **Nobody is ever in `AntState::Digging`.** `behavior_weights.dig` is set in config but never consumed by the FSM
- **Ants don't traverse surface↔underground.** The two layers are visually connected (underground panel sits below the surface module on the canvas) but no code moves ants through `Terrain::NestEntrance`
- Underground rendering: `Solid` = opaque dark-brown tile, `Chamber(kind)` = translucent kind-colored tile, `Empty` = substrate texture (same as surface)

Result: the underground is a static carved diorama, not a living dig. Player toggles `Tab` to look at it, sees the same chambers every time, never sees ants down there, never sees anything change.

## What "watching them dig" actually looks like

Per `biology.md`, real ant excavation is a **carry-and-dump cycle**, not a tile flip. The visual signature of an active nest is:

- Workers visibly excavating at a dig face (multi-tick progress per tile)
- Pellet-carrying foragers shuttling soil from dig face → surface entrance
- A growing **kickout mound** outside the entrance
- Pellets packed into tunnel walls as workers pass (lived-in look)
- Wider chambers vs narrower tunnels in the negative space
- Brood piled in BroodNursery, food stacked in FoodStorage — visible *inside* the chambers
- Tunnel network as **dark negative space** with packed-substrate edges (the diagnostic look of a sand-between-glass ant farm)

## Phased plan

### Phase A — Make digging happen (sim core)

Load-bearing piece. Without this, none of the visuals matter.

1. **Surface ↔ underground traversal.** Ant on a surface `Terrain::NestEntrance` cell whose colony has an underground module → teleport to the mirrored entrance on the underground module with `module_id` updated and position at the underground entrance cell. Same direction in reverse for ants returning. Pheromones do NOT bleed across layers (matches MVP underground isolation).

2. **Idle worker → Digging FSM hook.** In `sense_and_decide`: if ant is on the underground module, currently `Idle` or `Exploring`, has a `Solid` neighbor, and `random() < behavior_weights.dig`, transition to `AntState::Digging` with target = nearest Solid neighbor. The existing `behavior_weights.dig` field is finally consumed.

3. **Multi-tick dig progress.** Replace instant `dig_tick` excavation with a per-(ant, target_cell) progress counter. Each tick adjacent to a Solid target adds N progress; tile flips when progress crosses a per-species threshold. Default ~60 ticks (≈ 2 sim seconds at default scale = visible-but-not-frantic). Add `appearance.dig_speed_multiplier` to species TOMLs (default 1.0; Camponotus through wood is slower, Pogonomyrmex through sand is faster).

4. **Pellet carry cycle.** Tile flip → digger gains `carrying_soil: bool` flag → state changes to `ReturningHome` (existing state, reused). Walks to surface entrance, drops pellet at the kickout zone, returns to the underground to dig again. Mirrors the existing food-carry pipeline architecturally.

5. **Kickout mound state.** `Terrain::SoilPile(intensity: u8)` variant on the surface module's outworld cells adjacent to the nest entrance. Each pellet drop increments the nearest pile cell up to a cap. The pile becomes a permanent visible record of nest activity.

### Phase B — Make digging readable (render)

Bundle with A so the first run already feels alive.

6. **"See the tunnels."** Render `Terrain::Empty` cells inside an `UndergroundNest` module as **dark shadowed negative space** — not the substrate texture used on surface modules. `Solid` cells adjacent to `Empty` get a darker edge rim (wall shading). This single change is what gives the underground the iconic sand-between-glass look. Without it, the tunnel network reads as flat tile differences. With it, the player feels they're looking *into* a 3D nest cross-section.

7. **Substrate type per module.** New field on `Module`: `substrate: SubstrateKind` with variants `Loam` (default), `Sand`, `Ytong`, `Wood`, `Gel`. Drives the substrate base color palette and the `dig_speed_multiplier`. Loam = warm dark brown (default); Sand = pale tan; Ytong = pale gray-white (the keeper-favorite "permanent" formicarium look); Wood = orange-brown with grain texture; Gel = sci-fi blue. Editor palette gets per-substrate variants of the underground module.

8. **Soil-carry indicator.** Mirror the existing `FoodCarryIndicator` pattern — small dark-brown blob held in the digger's mandibles when `carrying_soil`. Render as a child sprite, hidden when not carrying.

9. **Kickout mound sprite.** When `Terrain::SoilPile(intensity)` is non-zero, render a brown mound sprite scaled by intensity. Multiple piles cluster around the entrance, building up over time. **This is the visible payoff.**

10. **Excavation pulse.** When a tile flips Solid → Empty, brief flash render on that cell (yellow/orange particle, ~30 ticks). Cheap reward for the player who's watching.

### Phase C — Deeper systems (deferred to a later session)

Polish that turns "watch them dig" into "actually plays well."

11. **CO₂ / dig-priority pheromone field.** Add a new `PheromoneLayer::DigPriority` (or repurpose the existing scalar slot). Workers deposit on Solid cells adjacent to active chambers + brood. Diggers select the cell with the highest dig score. Result: nest expansion biases toward existing population density — exactly the right look.

12. **Chamber-type siting.** When colony population exceeds chamber capacity (e.g. brood count > N → BroodNursery is full), designate a "dig new chamber" target with appropriate type. When seasonal temperature shifts, workers shuttle brood between chambers to track optimal microclimate (ties into K3).

13. **Wall-packing.** Pellets dropped inside tunnels (rather than carried all the way out) accumulate as darker patches on tunnel walls. Old tunnels get the lived-in look. Real biology: workers pack pellets into walls to reinforce them.

14. **Brood / food visible inside chambers.** Colored clusters in chamber cells: cream pile in BroodNursery scaled by larva count, green/tan stack in FoodStorage scaled by food units, etc. Chambers go from "labeled tile" to "look at the eggs piled up there."

15. **Antennation cluster around dig sites.** Idle workers do a short "investigate" walk toward the nearest active digger and linger for a few ticks. Doesn't change excavation rate but produces the visible cluster around dig sites that real ant farms have.

16. **Player Dig beacon.** Right-click on a Solid underground tile → drop a `Dig` beacon. Extends the existing P7 `BeaconKind { Gather, Attack, Dig }` pattern. Nearby workers prioritize that tile. Lets the player carve specific chamber shapes instead of just watching.

17. **Substrate selection at editor placement time.** Editor palette grows from "UndergroundNest" to "UndergroundNest (Loam) / (Sand) / (Ytong) / etc." Species-substrate compatibility hints (Camponotus prefers wood, Pogonomyrmex prefers sand, generic ants OK with anything).

18. **Tunnel collapse over time.** In Sand substrate, abandoned tunnels (no traffic for N ticks) gradually re-fill with adjacent material. Real biology — only Ytong holds permanently. Adds long-term dynamics for sand setups.

## Vocabulary cross-ref

Terminology validated against current keeper sites (Tar Heel Ants, AntsCanada, Antstore) — our existing terms are current.

| Sim term | Keeper-community term | Notes |
|---|---|---|
| `ModuleKind::TestTubeNest` | "founding formicarium" / "founding tube" | Test tube + cotton + water = how 95% of keepers start a queen |
| `ModuleKind::Outworld` | "outworld" / "foraging area" / "arena" | Universal term |
| `ModuleKind::YTongNest` | "Ytong" / "AAC" / "aerated concrete" | THE keeper-favorite permanent nest material |
| `ModuleKind::AcrylicNest` | "acrylic formicarium" | Smoother, sterile look; often layered |
| `ModuleKind::Hydration` | "water tower" / "hydration port" / "moisture chamber" | Wicks water into the nest |
| `ModuleKind::FeedingDish` | "feeder" / "feeding station" | Sugar water + protein |
| `ModuleKind::HeatChamber` | "heat cable" / "heat pad" / "heated zone" | Component, not module — could rename |
| `ModuleKind::HibernationChamber` | "hibernation tube" / "wine fridge zone" | Keeper-DIY: most use a wine fridge |
| `ModuleKind::Graveyard` | "midden" / "trash chamber" | Less common as an explicit module; many setups just leave waste in the outworld |
| `ModuleKind::UndergroundNest` | "diggable substrate" / "soil terrarium" / "natural setup" | Less common in modern keeping (Ytong dominates) but iconic for the dig-and-watch feel |

The substrate variants (Loam / Sand / Ytong / Wood / Gel) all appear on keeper sites' substrate-product pages or in their tutorials.

## Implementation order

When we come back to this:

1. Phase A items 1-5 in one commit (sim core; tests for traversal + dig progress + pellet cycle).
2. Phase B items 6-10 in one commit (render upgrade; the "see the tunnels" change is the headline).
3. Phase C items shipped one-or-two at a time as polish iterations.

Total estimated scope: A+B is one focused session. C is multiple sessions of polish.

## Why we paused

Colony collapse at in-game year 1 was observed in actual play. The dig system is irrelevant if colonies die before they have time to dig anything interesting. Triaging the collapse first.

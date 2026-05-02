# antcolony — Master Phases Roadmap

Single-page index of every phase. Per-phase deep dives live in their own
docs (linked). Status and effort columns reflect reality as of the last
session, not aspirational targets.

---

## Status legend

- ✅ **Shipped** — code in main, tests green, validated
- 🟡 **Partial** — meaningful chunks shipped, others deferred to follow-up phase
- 🟢 **Designed** — full design doc exists, no code yet
- 🔵 **Sketched** — bullet points in this doc only, no deep design yet
- 🔴 **Parked** — explicitly deprioritized, may never ship

---

## Phase ladder

| # | Phase | Status | Detail doc | Effort to ship |
|---|---|---|---|---|
| 1 | Pheromone grid + ACO ant FSM (headless) | ✅ | `HANDOFF.md` | done |
| 2 | Bevy integration + sprite render | ✅ | `HANDOFF.md` | done |
| 3 | Colony economy (food → eggs → ants) | ✅ | `HANDOFF.md` + `docs/biology.md` | done |
| K1 | Data-driven species + time scale picker | ✅ | `HANDOFF.md` | done |
| K2.1-2.3 | Modular formicarium + editor | ✅ | `HANDOFF.md` | done |
| K3 | Thermoregulation + diapause | ✅ | `HANDOFF.md` + `docs/biology.md` | done |
| K4 | Save/load + offline catch-up + milestones | ✅ | `HANDOFF.md` | done |
| K5 | Inspector + timeline + nuptial flights + procedural body art | ✅ | `HANDOFF.md` | done |
| 4 | Multi-colony + combat + Avenger + territory | ✅ | `HANDOFF.md` | done |
| 5 | Underground nest layer (MVP) | ✅ | `HANDOFF.md` | done |
| 6 | Hazards + predators + weather | ✅ | `HANDOFF.md` | done |
| 7 | Player interaction (possession + recruit + beacons) | ✅ | `HANDOFF.md` | done |
| Sprite atlas | claude.ai/design pixel-art pack for 7 species | ✅ | `assets/sprite_prompts/PROMPTS.md` | done |
| Perf | Substep architecture + per-ant rayon + SIMD evap + tube pheromone substrate | ✅ | inline comments in `simulation.rs` | done |
| Bio | Diapause biology fixes (metabolic depression, retreat to nest, body-fat survival, brood preservation) | ✅ | `docs/biology.md` | done |
| Balance | Bore-width auto-sizing + population-saturation queen lay cap | ✅ | inline | done |
| Dig A | Surface↔underground traversal + multi-substep dig + pellet carry + kickout mound | ✅ | `docs/digging-design.md` | done |
| Dig B (min) | Soil pellet sprite + kickout mound sprite | ✅ | inline | done |
| Env art pack | 95-prompt environment sprite pack (substrates, brood, predators, etc.) | 🟢 prompts ready | `assets/sprite_prompts/ENVIRONMENT_PROMPTS.md` | 1-2 sittings on claude.ai/design + 1 commit per category |
| **Dig B (full)** | **"See the tunnels" wall-rim shading + substrate variants per module** | 🟢 | `docs/digging-design.md` | 1 session |
| **Dig C** | **CO₂ dig-priority pheromone, chamber-type siting, wall-packing, brood/food piles in chambers, antennation cluster, player Dig beacon, substrate selection in editor, tunnel collapse** | 🟢 | `docs/digging-design.md` | 3-5 sessions |
| **Phase 8** | **Full game mode — 12×16 grid map, daughter colonies, win condition, biome/season variation** | 🟢 | `docs/phase-8-full-game-mode.md` | 4-6 sessions |
| Phase 9.0 | Narrator AI (procedural names + lore + chronicle.md per colony) | 🟢 | `docs/ai-architecture.md` | 1 week |
| Phase 9.1 | Blackboard + rule-based KS (no LLM) | 🟢 | `docs/ai-architecture.md` | 2-3 sessions |
| Phase 9.2 | Obsidian writer + per-colony vault | 🟢 | `docs/ai-architecture.md` | 1-2 sessions |
| Phase 9.3 | AI bundle system + LLM sidecar (OpenTTD model) | 🟢 | `docs/ai-architecture.md` | 4-6 sessions |
| Phase 9.4 | AI vs AI mode + spectator + replay | 🟢 | `docs/ai-architecture.md` | 1-2 sessions on top of 9.3 |
| Phase 9.5+ | Tournament, personality LoRAs, cross-match rivalries | 🔵 | `docs/ai-architecture.md` | indefinite |
| **Phase 10** | **Multiplayer — deterministic lockstep, per-tick command exchange, desync detection, replay format** | 🟢 | `docs/multiplayer-architecture.md` | 3-5 sessions |
| Phase 11 | Tutorial + onboarding flow | 🔵 | TBD | 1-2 sessions |
| Phase 12 | Audio (pheromone trail tones, ant chittering, ambient music, weather) | 🔵 | TBD | 2-3 sessions |
| Phase 13 | Distribution — Steam/itch.io page, demo build, marketing assets | 🔴 | TBD | 1-2 weeks externally-driven |

---

## Recommended near-term order

The substrate is now stable enough that gameplay-facing work is the right priority. Subject to economy validation landing clean (current overnight sweep):

1. **Dig B (full)** — visual upgrade. "See the tunnels" wall shading + substrate variants. Drives the env art pack into the renderer.
2. **Phase 8 grid map** — converts keeper mode into a real game with a win condition. Daughter colonies via nuptial flights now have somewhere to go.
3. **Phase 9.0 narrator** — charm layer. Cheap, high replay-value. Per-colony chronicles unlock storytelling.
4. **Phase 9.1 blackboard AI** — replaces scripted red-team AI. Player-legible reasoning side panel.
5. **Phase 9.2-9.4 AI bundle system** — VRAM-tier-aware LLM sidecars + AI vs AI mode.
6. **Phase 10 multiplayer** — once AI bundles work, the same blackboard slot accepts a remote player. Real PvP.
7. **Phase 11 tutorial** — required before distribution.
8. **Phase 12 audio** — required before distribution.
9. **Phase 13 distribution** — when 1-12 are solid.

---

## Cross-cutting ongoing work

These don't fit into a single phase; they extend across all of them:

- **biology log discipline** — `docs/biology.md` grows with every new behavior fact. Append-only with citations. Required reading before touching any behavior code.
- **art pack** — `assets/sprite_prompts/` accumulates per-content-type prompt packs (species ants, environment, future seasonal/event variations). Drive on claude.ai/design at Matt's pace.
- **performance** — opportunistic optimization as profiling reveals new hot paths. Current floor: ~150 ticks/sec/process at Timelapse with 8-way oversubscription. Future targets: GPU compute for pheromones, hierarchical sim (idle-module skip), HQ batch atlas swaps.
- **save format versioning** — every commit that adds a serialized field uses `#[serde(default)]` so old saves still load. Already established as a convention.

---

## What this doc is NOT

- **Not a release calendar.** No dates. Effort estimates are session counts, not weeks.
- **Not a contract.** Reordering happens when triage reveals new bugs (Pogonomyrmex collapse → emergency biology fixes mid-Phase-9-design).
- **Not the only living plan.** HANDOFF.md is the per-session snapshot; this is the cross-session view.

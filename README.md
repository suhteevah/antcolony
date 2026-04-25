# antcolony — Rust Ant Colony Simulation

A real-time ant colony simulation game built in Rust with Bevy ECS, inspired by Maxis SimAnt (1991) and Warcraft 3 ant colony custom maps.

## What Is This?

An emergent behavior simulation where thousands of individual ants — each following simple local rules — produce complex colony-level intelligence through pheromone-based communication. No ant knows the big picture. Colony intelligence arises from chemical feedback loops, just like real ants.

**Core Gameplay Loop:** Food appears on the surface → Foraging ants find it → They deposit pheromone trails back to the nest → More ants follow stronger trails → Food feeds the queen → Queen lays eggs → Eggs mature into new ants → Colony grows → Expand territory, fight rival colonies, survive predators.

## Design Pillars

1. **Emergence over scripting.** Individual ants are dumb. The colony is smart. All pathfinding, task allocation, and territory control emerge from pheromone feedback — no centralized planner, no A*.

2. **Two-layer world.** Surface (top-down: food, predators, rival nests) and Underground (side-view: tunnels, chambers, queen, brood). Connected by nest entrances.

3. **Colony economics.** Single resource (food) drives everything: feeding ants, producing eggs, growing soldiers. Caste ratios (worker/soldier/breeder) and behavior allocation (forage/dig/nurse) are the player's strategic levers.

4. **SimAnt DNA, modern execution.** SimAnt fit in 640KB. We target 10,000+ ants at 60fps with proper spatial indexing, dense pheromone grids, and Bevy's parallel ECS scheduling.

## Architecture

Three-crate workspace separating concerns:

```
antcolony-sim     Pure simulation logic. No rendering, no Bevy.
                  Testable headless. This is the brain.

antcolony-game    Bevy ECS integration. Wraps sim types in Components
                  and Resources. Runs sim systems in FixedUpdate.

antcolony-render  All visual output. Sprites, pheromone heatmaps,
                  camera, debug UI. Can be disabled for headless.
```

## Prerequisites

- Rust 1.85+ (edition 2024)
- Windows 10/11 (primary dev target), Linux/macOS also supported
- GPU with Vulkan or DX12 support (for Bevy rendering)

## Quick Start

```powershell
# Clone and build
git clone <repo-url>
cd antcolony
cargo build --workspace

# Run the simulation
cargo run

# Run with debug overlay
cargo run -- --dev

# Run tests
cargo test --workspace

# Run headless simulation (no window)
cargo test --test headless_sim
```

## Configuration

All simulation parameters are tunable at runtime via `assets/config/simulation.toml`. See `CLAUDE.md` for the full parameter reference.

Key knobs:
- `pheromone.evaporation_rate` — How fast trails fade (0.02 = slow, natural; 0.1 = fast, chaotic)
- `ant.exploration_rate` — Random walk probability (0.15 = good balance of exploration vs exploitation)
- `ant.alpha` / `ant.beta` — ACO path selection weights (α=1, β=2 = standard Dorigo parameters)
- `colony.queen_egg_rate` — Colony growth speed

## Project Status

See `HANDOFF.md` for the phased implementation plan and current progress.

## References

- **SimAnt** (Maxis, 1991) — Will Wright's ant colony simulation, based on E.O. Wilson's *The Ants*
- **WC3 Ant Colony** (Callex) — 10-player cooperative ant colony custom map for Warcraft 3
- **WC3 Ant Wars** (HiveWorkshop) — Procedurally generated terrain, evolution trees, destructible soil
- **ACO Algorithms** (Dorigo, 1992) — Ant Colony Optimization for probabilistic path selection
- **Empires of the Undergrowth** (Slug Disco, 2024) — Modern ant colony RTS citing WC3 maps as inspiration
- **bones-ai/rust-ants-colony-simulation** — Bevy-based reference implementation (~5K ants)
- **krABMaga** — Rust agent-based modeling framework with Bevy visualization

## License

TBD

---

---

---

---

---

---

---

---

## Support This Project

If you find this project useful, consider buying me a coffee! Your support helps me keep building and sharing open-source tools.

[![Donate via PayPal](https://img.shields.io/badge/Donate-PayPal-blue.svg?logo=paypal)](https://www.paypal.me/baal_hosting)

**PayPal:** [baal_hosting@live.com](https://paypal.me/baal_hosting)

Every donation, no matter how small, is greatly appreciated and motivates continued development. Thank you!

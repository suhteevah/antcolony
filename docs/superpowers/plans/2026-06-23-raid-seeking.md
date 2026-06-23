# Raid-Seeking Behavior Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Give attackers a local-information behavior that drives them to seek out and assault the enemy nest (surface entrance → underground → deep queen), so the underground nest layer produces a *contested siege* instead of an unreachable bunker.

**Architecture:** Mirror the existing `avenger_tick` heading-override pattern. A new gated `raid_seek_tick` runs between `avenger_tick` and `movement`; it designates a raid party per colony and overrides each raider's heading toward the steepest *enemy* `colony_scent` gradient (signed territory layer — colony 0 positive, others negative; every ant incl. the stationary queen deposits it each tick via `territory_deposit_tick`, so it peaks at the enemy nest on the surface and at the deep queen's chamber underground). The existing raid-descent arm (in `surface_underground_traversal`) is extended to fire for designated raiders so they descend the enemy entrance they've been steered onto. All gated; default OFF ⇒ byte-identical to every existing sim.

**Tech Stack:** Rust (antcolony-sim, antcolony-trainer), edition 2024.

## Global Constraints

- **Byte-identical default path.** New config flags default to inert (`raid_seeking_enabled = false`, `raid_party_size = 0`). With seeking OFF, `raid_seek_tick` early-returns before touching `self.rng` or any ant, and no ant is ever marked `is_raider`, so the descent-gate extension is also inert. The 300-tick `to_bits()` determinism guard, `bench_determinism`, and the full sim suite (~244) + trainer suite stay green.
- **Rule 4 — no global knowledge.** A raider chooses its heading using ONLY local pheromone reads at/around its own cell (the signed `ColonyScent` layer). It must NOT read the enemy's nest coordinate, queen position, or any global map. The gradient ascent IS the seek.
- **No `.unwrap()` in simulation paths** (`.expect("invariant")` acceptable for structural invariants; tests may use `.expect`). `tracing` not `println!`. All tunables in `config.rs`.
- **Determinism under raid-seeking ON.** When seeking is on (nest arena), matches must still be reproducible and thread-count-independent. Raider promotion uses a deterministic index-order pick (NOT `self.rng`) so it adds no RNG coupling.
- **Biology grounding:** raid orientation to target-colony nest odor is the documented mechanism (slave-maker / army-ant raids orient along scent toward the target nest). See `docs/biology/interspecific/05-raiding-usurpation-and-who-wins.md`. Add a one-line cite in the `raid_seek_tick` doc-comment.

---

### Task RS1: Raid-seeking behavior (sim crate)

**Files:**
- Modify: `crates/antcolony-sim/src/ant.rs` (add `is_raider` field)
- Modify: `crates/antcolony-sim/src/config.rs` (add two `CombatConfig` fields + defaults)
- Modify: `crates/antcolony-sim/src/simulation.rs` (add `enemy_scent_heading` helper + `raid_seek_tick`; wire into `tick()`; extend the raid-descent gate)
- Test: in-module `#[cfg(test)]` in `simulation.rs` + the existing config default tests

**Interfaces:**
- Consumes: `Ant { heading: f32, colony_id: u8, module_id: ModuleId, position: Vec2, caste, is_in_transit() }`; `Module { world, pheromones, kind }`; `PheromoneGrid::read(ux, uy, PheromoneLayer::ColonyScent) -> f32` (signed); `world.world_to_grid(Vec2)->(i64,i64)`, `in_bounds(i64,i64)->bool`, `grid_to_world(usize,usize)->Vec2`.
- Produces: `Simulation::raid_seek_tick(&mut self)`; `CombatConfig { raid_seeking_enabled: bool, raid_party_size: u32 }`; `Ant.is_raider: bool`.

- [ ] **Step 1: Add the `is_raider` field (mirror `is_avenger`)**

In `ant.rs`, immediately after the `is_avenger: bool` field (~line 72):

```rust
    /// Designated raider: steered toward the enemy nest by `raid_seek_tick`
    /// and permitted to descend the enemy entrance. Default false; only set
    /// when `combat.raid_seeking_enabled`. Mirrors `is_avenger`.
    #[serde(default)]
    pub is_raider: bool,
```

And in the constructor/default where `is_avenger: false` is set (~line 138), add `is_raider: false,`.

> If `is_avenger` has no `#[serde(default)]` (older field), match the surrounding style — but `is_raider` is new, so `#[serde(default)]` keeps save-compat. If the struct derives `Default`, ensure the manual `Ant::new` path sets it.

- [ ] **Step 2: Add the config knobs**

In `config.rs` `CombatConfig` (after `raid_underground_enabled`, ~line 381):

```rust
    /// When true, `raid_seek_tick` designates up to `raid_party_size` raiders
    /// per colony and steers them toward the enemy nest via the enemy
    /// `ColonyScent` gradient. Default false ⇒ inert (byte-identical).
    pub raid_seeking_enabled: bool,
    /// Number of ants per colony designated as raiders when seeking is on.
    /// 0 ⇒ no raiders. The raid party advances on the enemy nest while the
    /// rest of the colony forages/defends normally.
    pub raid_party_size: u32,
```

In the `Default for CombatConfig` impl (after `raid_underground_enabled: false,`, ~line 508):

```rust
            raid_seeking_enabled: false,
            raid_party_size: 0,
```

Update the existing config default tests (the ones asserting `!raid_underground_enabled` at ~571/583) to also assert `!sc.combat.raid_seeking_enabled` and `sc.combat.raid_party_size == 0`.

- [ ] **Step 3: Write the failing tests** (in `simulation.rs` `#[cfg(test)] mod tests`)

```rust
#[test]
fn raid_seek_is_inert_when_disabled() {
    // Default config: no ant is ever marked a raider, headings are unchanged
    // by raid_seek_tick (it early-returns). Guards the byte-identical default.
    let mut sim = Simulation::new_two_colony(SimConfig::default(), 7);
    sim.run(50);
    assert!(sim.ants.iter().all(|a| !a.is_raider),
        "no raiders may be designated when raid_seeking_enabled is false");
}

#[test]
fn raid_seek_designates_party_and_steers_toward_enemy_scent() {
    use crate::pheromone::PheromoneLayer;
    let mut g = SimConfig::default();
    g.combat.raid_seeking_enabled = true;
    g.combat.raid_party_size = 3;
    let mut sim = Simulation::new_two_colony(g, 7);

    // (a) Designate the party.
    sim.raid_seek_tick();
    let raider_idx = sim.ants.iter().position(|a| a.colony_id == 0 && a.is_raider)
        .expect("a colony-0 raider must be designated");
    assert_eq!(
        sim.ants.iter().filter(|a| a.colony_id == 0 && a.is_raider).count(), 3,
        "exactly raid_party_size raiders designated"
    );

    // (b) Steering: place that raider on a known interior cell and lay a stronger
    // ENEMY (colony 1 = negative-sign) scent one cell to its EAST so the local
    // gradient is unambiguous, then re-run and assert it steers east (cos>0).
    let m0 = sim.ants[raider_idx].module_id;
    // An interior cell with room for an east neighbour (two-colony modules are
    // >= 24x24). If your test arena is smaller, pick coords inside it.
    let (cx, cy) = (10i64, 10i64);
    let here = sim.topology.module(m0).world.grid_to_world(cx as usize, cy as usize);
    sim.ants[raider_idx].position = here;
    {
        let module = sim.topology.module_mut(m0);
        // weak here, strong to the east => enemy-sign extreme is east.
        module.pheromones.deposit_territory(cx as usize, cy as usize, 1, 1.0, 10.0);
        module.pheromones.deposit_territory((cx + 1) as usize, cy as usize, 1, 8.0, 10.0);
    }
    sim.raid_seek_tick();
    assert!(
        sim.ants[raider_idx].heading.cos() > 0.3,
        "raider should be steered east toward the stronger enemy scent (heading={})",
        sim.ants[raider_idx].heading
    );
    let _ = PheromoneLayer::ColonyScent;
}
```

> `new_two_colony(cfg, seed)` is the existing flat two-colony constructor used elsewhere in the test module — if its name differs, use the constructor those tests already call. `deposit_territory(ux, uy, colony_id, amount, cap)` is the signed-deposit method confirmed in `pheromone.rs`. If `world.width()/height()` are named differently, use the real accessors.

- [ ] **Step 4: Run to verify they fail**

Run: `cargo test -p antcolony-sim raid_seek 2>&1 | tail -25`
Expected: compile error (no `raid_seek_tick`, no `is_raider`) then fail.

- [ ] **Step 5: Implement the gradient helper + `raid_seek_tick`**

In `simulation.rs` (near `avenger_tick`):

```rust
    /// Local enemy-territory gradient for a raider. Reads the signed
    /// `ColonyScent` layer at the ant's 8 neighbour cells and returns a heading
    /// toward the neighbour that maximises ENEMY scent — most-negative scent for
    /// colony 0, most-positive for any other colony. Returns `None` when no
    /// neighbour improves on the current cell (no usable signal), so the caller
    /// leaves the ant's ACO heading intact and it keeps wandering until it picks
    /// up the enemy gradient. Local information only (CLAUDE.md rule 4).
    fn enemy_scent_heading(
        module: &crate::module::Module,
        position: Vec2,
        colony_id: u8,
    ) -> Option<f32> {
        let enemy_sign = if colony_id == 0 { -1.0_f32 } else { 1.0_f32 };
        let (cx, cy) = module.world.world_to_grid(position);
        let score = |gx: i64, gy: i64| -> Option<f32> {
            if !module.world.in_bounds(gx, gy) {
                return None;
            }
            Some(enemy_sign
                * module
                    .pheromones
                    .read(gx as usize, gy as usize, crate::pheromone::PheromoneLayer::ColonyScent))
        };
        let here = score(cx, cy)?;
        let mut best: Option<(f32, i64, i64)> = None;
        for (dx, dy) in [
            (1, 0), (-1, 0), (0, 1), (0, -1),
            (1, 1), (1, -1), (-1, 1), (-1, -1),
        ] {
            let (gx, gy) = (cx + dx, cy + dy);
            if let Some(s) = score(gx, gy) {
                if best.map(|(bs, _, _)| s > bs).unwrap_or(true) {
                    best = Some((s, gx, gy));
                }
            }
        }
        let (bs, bgx, bgy) = best?;
        // Strict ascent: only steer if a neighbour beats the current cell.
        if bs <= here + 1e-6 {
            return None;
        }
        let target = module.world.grid_to_world(bgx as usize, bgy as usize);
        let delta = target - position;
        if delta.length_squared() <= 1e-9 {
            return None;
        }
        Some(delta.y.atan2(delta.x))
    }

    /// Raid-seeking (gated). Designates up to `raid_party_size` raiders per
    /// colony and overrides each raider's heading toward the enemy nest via the
    /// enemy `ColonyScent` gradient (`enemy_scent_heading`). Runs after
    /// `avenger_tick` and before `movement`, so the override is what `movement`
    /// consumes. Once a raider reaches an enemy surface entrance,
    /// `surface_underground_traversal`'s raid-descent arm carries it under; the
    /// same gradient then steers it toward the deep queen (she deposits scent
    /// every tick, so her chamber is the underground enemy-scent maximum).
    ///
    /// Biology: raids orient to target-colony nest odor (see
    /// docs/biology/interspecific/05-raiding-usurpation-and-who-wins.md).
    ///
    /// Default OFF: early-returns before touching any ant or `self.rng`, so the
    /// default sim path is byte-identical.
    pub fn raid_seek_tick(&mut self) {
        if !self.config.combat.raid_seeking_enabled || self.config.combat.raid_party_size == 0 {
            return;
        }
        let party = self.config.combat.raid_party_size as usize;
        for cid in 0..self.colonies.len() {
            let colony_id = self.colonies[cid].id;
            // Deterministic top-up of the raid party by ascending ant index
            // (no RNG — keeps determinism decoupled from this system).
            let current = self
                .ants
                .iter()
                .filter(|a| a.colony_id == colony_id && a.is_raider)
                .count();
            if current < party {
                let mut to_promote = party - current;
                for a in self.ants.iter_mut() {
                    if to_promote == 0 {
                        break;
                    }
                    if a.colony_id == colony_id
                        && !a.is_raider
                        && !matches!(a.caste, AntCaste::Queen)
                        && !a.is_in_transit()
                    {
                        a.is_raider = true;
                        to_promote -= 1;
                    }
                }
            }
        }
        // Steer every raider by its local enemy-scent gradient.
        for i in 0..self.ants.len() {
            if !self.ants[i].is_raider || self.ants[i].is_in_transit() {
                continue;
            }
            let (pos, mid, colony_id) =
                (self.ants[i].position, self.ants[i].module_id, self.ants[i].colony_id);
            let Some(module) = self.topology.try_module(mid) else {
                continue;
            };
            if let Some(h) = Self::enemy_scent_heading(module, pos, colony_id) {
                self.ants[i].heading = h;
            }
        }
    }
```

> Match the real `Module` path/type and pheromone-read signature. If `try_module` returns a different guard type, follow the surrounding code (e.g. `module()` vs `try_module()`). `enemy_scent_heading` is an associated fn (`Self::…`) to avoid borrowing `self` while iterating `self.ants` mutably — pass the `&Module` in.

- [ ] **Step 6: Wire into the tick + extend the descent gate**

In `tick()` insert after `self.avenger_tick();` (line ~1580) and before `self.movement();`:

```rust
        self.raid_seek_tick();
```

In `surface_underground_traversal`'s raid-descent arm, change the state gate so designated raiders descend even when not momentarily Fighting:

```rust
                if !matches!(ant.state, AntState::Fighting | AntState::Usurping) && !ant.is_raider {
                    continue;
                }
```

(was: `if !matches!(ant.state, AntState::Fighting | AntState::Usurping) { continue; }`)

- [ ] **Step 7: Run to verify the new tests pass**

Run: `cargo test -p antcolony-sim raid_seek 2>&1 | tail -25`
Expected: PASS (2 tests).

- [ ] **Step 8: Full regression + determinism guard**

Run: `cargo test -p antcolony-sim 2>&1 | tail -15` → all green (~246 tests).
Run: `cargo test -p antcolony-trainer 2>&1 | tail -15` → all green.
Run the byte-determinism guard the suite already has (the 300-tick `to_bits` test + `bench_determinism`) and confirm green — with seeking OFF by default the legacy path must be bit-identical.
Run: `RAYON_NUM_THREADS=1 cargo test -p antcolony-sim determinism det_ 2>&1 | tail -10` → green, identical to default thread count.

- [ ] **Step 9: Commit**

```bash
git add crates/antcolony-sim/src/ant.rs crates/antcolony-sim/src/config.rs crates/antcolony-sim/src/simulation.rs
git commit -m "feat(sim): raid-seeking behavior — raiders steer to the enemy nest via colony_scent gradient (gated, default off)"
```

---

### Task RS2: Arm raid-seeking in the nest arena + convert the inversion to a contested siege

**Files:**
- Modify: `crates/antcolony-trainer/src/env.rs` (`new_cross_species_nest_arena`)
- Modify: `crates/antcolony-sim/tests/nest_arena.rs` (the inversion test's nest branch + flip the tripwire)

**Interfaces:** consumes `CombatConfig.raid_seeking_enabled` + `raid_party_size` from RS1.

- [ ] **Step 1: Arm seeking in the trainer nest-arena constructor**

In `MatchEnv::new_cross_species_nest_arena`, where it already sets `raid_underground_enabled = true` and `underground_idle_alarm_threshold = 0.3` on both colony configs, also set:

```rust
            c.combat.raid_seeking_enabled = true;
            c.combat.raid_party_size = 12; // a raid column; tune in RS2 Step 3
```

Add/extend the existing arming test to assert `raid_seeking_enabled` and `raid_party_size > 0` on both colony configs.

- [ ] **Step 2: Arm seeking in the inversion test's nest branch**

In `crates/antcolony-sim/tests/nest_arena.rs` `run_match`, in the `if nest` branch where it sets `raid_underground_enabled` etc., also set on both `atk`/`def` combat configs:

```rust
            c.combat.raid_seeking_enabled = true;
            c.combat.raid_party_size = 12;
```

- [ ] **Step 3: Re-run and OBSERVE (experiment, not a fixed assertion)**

Run: `cargo test -p antcolony-sim --test nest_arena defender_holds -- --nocapture 2>&1 | grep -E "^flat:|peak"`
Read the printed `peak_enemy_in_def_ug`. It MUST now be > 0 (raiders descend). Three outcomes:
- **(A) peak > 0 AND defender still survives (nest_alive):** contested siege held — the choke works. This is the win. Change the tripwire from `assert_eq!(nest_enemy_ug, 0, …)` to `assert!(nest_enemy_ug > 0, "raiders must descend so the nest pass is a real siege")`, and keep the inversion asserts.
- **(B) peak > 0 AND defender now dies in the nest too (nest_alive false, fast):** the nest alone doesn't save a near-empty UG. The inversion was unreachability. Document this honestly: change the tripwire to `assert!(nest_enemy_ug > 0)`, and adjust the headline inversion assert to whatever the data supports (likely: defender survives *longer* in nest than flat, even if eventually wiped — `nest_ticks >= flat_ticks + MARGIN`). This is a real finding that feeds the next decision (initial UG garrison / stronger defenders).
- **(C) peak still 0:** raiders are not reaching the entrance through low-signal zones — RS1 gradient needs a fallback. STOP and report; do not paper over.

Tune `raid_party_size` and the defender's UG defensibility (NOT the production code) only as needed to characterise the regime. Record the final `flat=… nest=… peak=…` numbers and which outcome (A/B/C) held in the test's doc-comment.

- [ ] **Step 4: Full regression**

Run: `cargo test -p antcolony-sim 2>&1 | tail -15` and `cargo test -p antcolony-trainer 2>&1 | tail -15` → green.

- [ ] **Step 5: Commit**

```bash
git add crates/antcolony-trainer/src/env.rs crates/antcolony-sim/tests/nest_arena.rs
git commit -m "feat: arm raid-seeking in the nest arena; nest inversion is now a contested siege (peak_enemy_in_def_ug>0)"
```

---

## Self-Review

- Spec coverage: RS1 builds the behavior (field+config+helper+system+wiring+gate), RS2 arms it and converts the test from a bunker-survival to a contested-siege demonstration (or documents which regime holds). Both gated; default OFF byte-identical.
- Placeholder scan: none — full code given for RS1; RS2 Step 3 is intentionally an observe-and-branch experiment because the siege outcome is the scientific question and cannot be predicted.
- Type consistency: `is_raider`/`raid_seeking_enabled`/`raid_party_size`/`raid_seek_tick`/`enemy_scent_heading` used consistently across tasks; `deposit_territory`/`read(ColonyScent)` match `pheromone.rs`.

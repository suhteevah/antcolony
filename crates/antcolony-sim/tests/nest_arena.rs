//! End-to-end: the underground nest layer lets a small/defensive colony hold
//! a chokepoint against a swarm it loses to on open ground. This is the
//! mechanism behind the cross-species intransitivity hypothesis.

use antcolony_sim::ai::MatchStatus;
use antcolony_sim::config::{ColonySimConfig, SimConfig};
use antcolony_sim::topology::{QueenDepth, Topology};
use antcolony_sim::{AntCaste, AntState, Simulation};

/// Build an attacker (big, hard-hitting) and a defender (small, but with
/// the nest layer protecting the queen) pair of per-colony configs.
/// Returns (global, attacker_cfg, defender_cfg).
///
/// Calibration (2026-06-23, seed=7, max_ticks=3000):
///
/// The inversion mechanism: `colony_economy_tick` uses the GLOBAL config for
/// brood maturation (egg/larva/pupa timings are global, not per-colony). So
/// both colonies share the same brood pipeline speed. The KEY difference is
/// WHERE brood hatches:
///   - Flat arena (legacy spawn path): defender brood hatches in module 0
///     (the surface arena). The 500-strong swarm with attack=10 overwhelms
///     both the surface workers and any brood reinforcements. Defender dies.
///     flat_alive=false @ tick ~188.
///   - Nest arena: defender brood hatches in the UndergroundNest module
///     (because colony.underground_module is set). UG defenders fight raiders
///     at tunnel cap=3 (only 3 attackers per tick per defender). Queen is deep
///     underground behind entrance(cap=1)+tunnel(cap=3). The UG defender pool
///     builds up faster than raiders can kill them. nest_alive=true @ tick 3000
///     (queen alive, ~70 UG adults defending).
///
/// Global: brood fast (5+5+5=15 ticks egg→adult), high queen_egg_rate=0.5,
///   egg_cost=1.0 so the pipeline fills quickly.
/// Attacker: 500 workers, worker_attack=10.0 — overwhelming swarm.
/// Defender: 20 workers, worker_attack=1.0, soldier_attack=6.0 — small, ~1:25.
///
/// Final calibrated numbers: flat_alive=false @ 188, nest_alive=true @ 3000.
fn lopsided_pair() -> (SimConfig, ColonySimConfig, ColonySimConfig) {
    let mut global = SimConfig::default();
    // GLOBAL fast brood: 5+5+5 = 15 ticks egg→adult (vs default 300+300+200=800).
    // Both colonies share this speed; the nest arena is special because the
    // DEFENDER's brood hatches in the UG module (cap-protected), not on the open surface.
    global.colony.egg_stage_ticks = 5;
    global.colony.larva_stage_ticks = 5;
    global.colony.pupa_stage_ticks = 5;
    // Aggressive queen lay rate — fills the pipeline fast.
    global.colony.queen_egg_rate = 0.5;
    // Cheap eggs: food goes further.
    global.colony.egg_cost = 1.0;

    let mut attacker = ColonySimConfig::from(&global);
    // Very large swarm with very high attack — overwhelms defender on open ground
    // even accounting for the brood spawn behavior.
    attacker.ant.initial_count = 500;
    attacker.combat.worker_attack = 10.0;
    attacker.combat.soldier_attack = 20.0;
    // Attacker food: very large — prevents the attacker from losing on food alone.
    attacker.colony.initial_food = 50000.0;

    let mut defender = ColonySimConfig::from(&global);
    // Small colony — loses numerically on open ground.
    defender.ant.initial_count = 20;
    defender.combat.worker_attack = 1.0;
    defender.combat.soldier_attack = 6.0;
    // Defender food: large store so the queen keeps laying deep in the UG.
    defender.colony.initial_food = 500.0;

    (global, attacker, defender)
}

/// How long colony 1 (defender) survives, and whether it was wiped, on a
/// given arena. Returns (defender_alive_at_end, ticks_run).
fn run_match(
    global: SimConfig,
    attacker: ColonySimConfig,
    defender: ColonySimConfig,
    nest: bool,
) -> (bool, u64, u32) {
    let max_ticks = 3000u64;
    let mut sim = if nest {
        let mut g = global;
        g.combat.raid_underground_enabled = true;
        let mut atk = attacker;
        let mut def = defender;
        atk.combat.raid_underground_enabled = true;
        def.combat.raid_underground_enabled = true;
        def.ant.underground_idle_alarm_threshold = 0.3;
        // Caps that make the choke bite (mirror the harness injection).
        for c in [&mut atk, &mut def] {
            c.combat.max_simultaneous_attackers_open = 255;
            c.combat.max_simultaneous_attackers_tunnel = 3;
            c.combat.max_simultaneous_attackers_entrance = 1;
        }
        let topo =
            Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
        let bug = topo
            .underground_for_colony(0)
            .expect("black underground module must exist");
        let rug = topo
            .underground_for_colony(1)
            .expect("red underground module must exist");
        Simulation::new_two_colony_nest_arena(g, atk, def, topo, 7, 0, 2, bug, rug)
    } else {
        // Flat chokepoint arena: NO underground. The surface entrance cap bites
        // at module-2's NestEntrance cell, but the queen is ON the surface so
        // the swarm can reach her once all surface defenders are gone.
        // FRAGILE: the flat defender's fast death is amplified by the legacy
        // brood-spawn path (newborns hatch in module 0 when underground_module is
        // None). The attacker's 500/attack=10 edge was sized to overrun the
        // defender even so. If that spawn path is ever fixed, re-check flat_ticks
        // and re-widen the attacker edge to keep the open-ground overrun.
        let mut atk = attacker;
        let mut def = defender;
        for c in [&mut atk, &mut def] {
            c.combat.max_simultaneous_attackers_open = 255;
            c.combat.max_simultaneous_attackers_tunnel = 3;
            c.combat.max_simultaneous_attackers_entrance = 1;
        }
        let topo = Topology::two_colony_arena((24, 24), (32, 32));
        Simulation::new_two_colony_cross_species(global, atk, def, topo, 7, 0, 2)
    };

    // ENGAGEMENT INSTRUMENTATION (T7 review finding): a survival pass on the
    // nest arena is only a *siege* demonstration if attackers actually descend
    // into the defender's underground module. Track the peak number of enemy
    // (colony-0) ants present in the defender's UG across the run. None on the
    // flat arena (no UG => underground_module is None).
    let def_ug = sim.colonies[1].underground_module;
    let mut max_enemy_in_def_ug = 0u32;

    let mut ticks = 0u64;
    while ticks < max_ticks {
        sim.tick();
        ticks += 1;
        if let Some(ug) = def_ug {
            let n = sim
                .ants
                .iter()
                .filter(|a| a.colony_id == 0 && a.module_id == ug)
                .count() as u32;
            if n > max_enemy_in_def_ug {
                max_enemy_in_def_ug = n;
            }
        }
        if !matches!(sim.match_status(), MatchStatus::InProgress) {
            break;
        }
    }

    let def_alive = sim
        .ants
        .iter()
        .any(|a| a.colony_id == 1 && matches!(a.caste, AntCaste::Queen))
        && sim.colonies[1].adult_total() > 0;
    (def_alive, ticks, max_enemy_in_def_ug)
}

#[test]
fn defender_holds_in_nest_arena_longer_than_on_flat_arena() {
    let (g, atk, def) = lopsided_pair();
    let (flat_alive, flat_ticks, _flat_enemy_ug) =
        run_match(g.clone(), atk.clone(), def.clone(), false);
    let (nest_alive, nest_ticks, nest_enemy_ug) = run_match(g, atk, def, true);

    // Print for calibration visibility (visible with --nocapture). nest_enemy_ug
    // is the peak enemy presence inside the defender's UG: >0 means raiders
    // genuinely descended (contested siege); 0 means the deep queen was never
    // approached (survival is an unreached-bunker artifact, not a held siege).
    eprintln!(
        "flat: alive={flat_alive} ticks={flat_ticks} | nest: alive={nest_alive} ticks={nest_ticks} peak_enemy_in_def_ug={nest_enemy_ug}"
    );

    // The defensive inversion: in the nest arena the small colony survives at
    // least as long, and meaningfully longer, than on the flat arena where the
    // swarm overruns it.
    assert!(
        nest_ticks >= flat_ticks,
        "defender should survive at least as long in the nest arena \
         (flat={flat_ticks}, nest={nest_ticks})"
    );
    // The headline: the nest layer changes the outcome in the defender's favor.
    // Either it survives in the nest arena when it didn't on the flat arena, OR
    // it holds for a substantially longer siege.
    assert!(
        (nest_alive && !flat_alive) || nest_ticks >= flat_ticks + 200,
        "nest layer should flip/extend the defender's outcome \
         (flat_alive={flat_alive} @ {flat_ticks}, nest_alive={nest_alive} @ {nest_ticks})"
    );

    // CHARACTERIZATION / KNOWN GAP (raid-seeking). The inversion above is real
    // but currently rides on UNREACHABILITY, not a contested siege: peak enemy
    // presence inside the defender's UG is 0 because the raid-descent arm
    // (simulation.rs combat raid block) only fires for an attacker already
    // standing on the enemy SURFACE entrance cell while Fighting — and nothing
    // drives attackers there once they've wiped the defender's surface workers.
    // Until a raid-seeking behavior marches enemy fighters to the enemy nest
    // entrance, the deep queen is an impregnable bunker, not a sieged one.
    // This assert is a TRIPWIRE: when raid-seeking lands it will FAIL, forcing a
    // re-evaluation of this test as a real-siege demonstration AND of Task 8
    // (the nest-arena win-matrix would otherwise be a degenerate defender-always-
    // wins result). Do NOT "fix" this by deleting it — fix raid-seeking.
    assert_eq!(
        nest_enemy_ug, 0,
        "raid-seeking appears to have landed (peak enemy in defender UG = {nest_enemy_ug} > 0): \
         re-evaluate the nest inversion as a contested siege and re-scope Task 8"
    );
}

#[test]
fn tunnel_cap_caps_attackers_in_underground_module() {
    // Independent of brain quality: pile many enemy attackers onto ONE defender
    // standing in an UndergroundNest tunnel cell and assert the cap (3) limits
    // simultaneous damage. We assert via survival: with cap=3 the defender (high
    // health) survives a tick that an uncapped pile would kill.
    let global = SimConfig::default();
    let mut atk = ColonySimConfig::from(&global);
    atk.combat.worker_attack = 2.0;
    atk.combat.max_simultaneous_attackers_tunnel = 3; // cap bites
    atk.combat.max_simultaneous_attackers_open = 255;
    atk.combat.raid_underground_enabled = true;
    let mut def = ColonySimConfig::from(&global);
    def.combat.worker_health = 50.0; // survives 3×2=6 dmg/tick easily
    def.combat.max_simultaneous_attackers_tunnel = 3;
    def.combat.raid_underground_enabled = true;

    let topo = Topology::two_colony_nest_arena((24, 24), (32, 32), (24, 24), QueenDepth::Deep);
    let bug = topo
        .underground_for_colony(0)
        .expect("black underground module must exist");
    let rug = topo
        .underground_for_colony(1)
        .expect("red underground module must exist");
    let mut sim =
        Simulation::new_two_colony_nest_arena(global, atk, def, topo, 11, 0, 2, bug, rug);

    // Stand a defender (colony 1) on a black-UG tunnel cell and surround it with
    // many colony-0 attackers on the SAME cell (all within interaction radius).
    let (ex, ey) = sim
        .topology
        .module(bug)
        .world
        .find_nest_entrance(0)
        .expect("UG module must have a NestEntrance for colony 0");
    let cell = (ex, ey.saturating_sub(2));
    let pos = sim.topology.module(bug).world.grid_to_world(cell.0, cell.1);

    let def_idx = sim
        .ants
        .iter()
        .position(|a| a.colony_id == 1 && !matches!(a.caste, AntCaste::Queen))
        .expect("defender must have at least one non-queen ant");
    sim.ants[def_idx].module_id = bug;
    sim.ants[def_idx].position = pos;
    sim.ants[def_idx].health = 50.0;

    let mut placed = 0;
    for a in sim.ants.iter_mut() {
        if a.colony_id == 0 && !matches!(a.caste, AntCaste::Queen) && placed < 10 {
            a.module_id = bug;
            a.position = pos; // co-located => all candidate attackers on the defender
            a.transition(AntState::Fighting);
            placed += 1;
        }
    }
    assert!(
        placed >= 6,
        "need a pile of attackers to exceed the cap (placed={placed})"
    );

    let hp_before = sim.ants[def_idx].health;
    sim.combat_tick();
    let hp_after = sim
        .ants
        .iter()
        .find(|a| a.colony_id == 1 && !matches!(a.caste, AntCaste::Queen))
        .map(|a| a.health)
        .unwrap_or(0.0);
    let dmg = hp_before - hp_after;
    eprintln!("tunnel cap test: hp_before={hp_before} hp_after={hp_after} dmg={dmg}");
    // Cap=3 attackers × 2.0 attack = 6.0 max; assert NOT the uncapped 10×2=20.
    assert!(
        dmg <= 3.0 * 2.0 + 1e-3,
        "tunnel cap should limit damage to 3 attackers, got {dmg}"
    );
    assert!(dmg > 0.0, "some damage should land (dmg={dmg})");
}

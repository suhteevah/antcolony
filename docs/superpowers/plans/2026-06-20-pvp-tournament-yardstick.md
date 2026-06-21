# PvP Tournament Yardstick Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A reproducible round-robin PvP tournament over the existing 2-colony engine that ranks every brain (archetypes + v1 MLP + HAC checkpoints) on one Bradley-Terry/Elo scale, to answer whether the 0.874-bench SOTA is actually the best head-to-head and expose non-transitivity.

**Architecture:** A `Contender`/`Controller` abstraction unifies HAC and scripted brains; a heterogeneous `play_pair` runner (a faithful generalization of the existing `play_match`/`play_match_h2h` drive loops) plays any controller vs any controller; a round-robin scheduler (side-swapped, seeded, rayon-parallel over pairings) builds a win-matrix; Bradley-Terry MLE → Elo ranks them; a `tournament` bin writes the ladder + matrix + cycle report.

**Tech Stack:** Rust (edition 2024), candle (HAC forward), antcolony-sim (CPU sim), rayon. CUDA on cnc P100 for the run.

## Global Constraints

- **Reuse, don't rebuild:** `eval.rs::{score_match (fn), MatchEnd, OutcomeCounts, spec_seed_salt, play_match, play_match_h2h}`; `env.rs::{MatchEnv, DECISION_CADENCE}` and the `env.sim.*` drive methods (`colony_rich_observation`, `apply_ai_decision`, `apply_commander_intent`, `colony_ai_state`, `per_ant_observations`, `apply_ant_modulators`, `tick`, `match_status`, `tick` field, `colonies`); `hierarchical::obs_to_tensors::{rich_to_tensors, ant_obs_to_tensors}`; `HierarchicalActorCritic::{mean_commander_action, mean_ant_modulator}`; `self_play::load_frozen_hac(path, sizing, device)`; `League::make_brain(spec, seed)`; `hierarchical::sizing::A1`; `antcolony_sim::{AiDecision, MatchStatus, ai::observation::AntModulators}`.
- **Scoring:** the **decisive** metric is the ladder headline (`score_match` returns `(worker_share, decisive, MatchEnd)` from LEFT's perspective; right's score = `1.0 - left`). Worker-share is a secondary column.
- **Determinism:** seeds via `0xE7A1 * spec_seed_salt("{id_i}:{id_j}:{side}") ^ (m * 0x9E3779B97F4A7C15)` + side-swap. Tournament must be byte-reproducible for a fixed contender set; independent of `RAYON_NUM_THREADS`.
- **Additive / backward-compat:** refactoring `play_match`/`play_match_h2h` onto `play_pair` must keep them behaviorally **byte-identical** — the existing `eval` tests (and the validated 0.871/0.874 numbers) must reproduce. New code in a new `tournament.rs` module + new bin; `eval.rs` only gains `play_pair` + the thin re-wiring.
- **No `.unwrap()`/`.expect()` in production paths** (`Result` + `?`); tests may. `tracing`, never `println!` (bin stdout summary OK). Edition 2024 (`gen` reserved → `r#gen`).
- **Venue:** the full round-robin runs on **cnc** (CPU sim is the bottleneck; HAC forwards on the CUDA device). Logging/backup is definition-of-done for any RUN (ledger + result backup).
- **Spec:** `docs/superpowers/specs/2026-06-20-pvp-tournament-yardstick-design.md`.

---

### Task 1: `Contender` / `Controller` + spec resolution

**Files:** Create `crates/antcolony-trainer/src/tournament.rs`; add `pub mod tournament;` to `crates/antcolony-trainer/src/lib.rs`. Test: in-file.

**Interfaces — Produces:**
- `pub enum Controller { Hac(HierarchicalActorCritic), Scripted(Box<dyn antcolony_sim::ai::brain::AiBrain>) }`
- `pub struct Contender { pub id: String, pub spec: String, pub controller: Controller }`
- `pub fn build_contender(id: &str, spec: &str, device: &Device, sizing: Sizing) -> Result<Contender>` — `spec` starting with `"hac:"` → `Controller::Hac(load_frozen_hac(&spec[4..], sizing, device)?)`; anything else → `Controller::Scripted(League::make_brain(spec, 0)?)` (archetype names, `"mlp:<path>"`, `"mix:..."`).

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;
    use crate::hierarchical::sizing::A1;

    #[test]
    fn build_contender_resolves_scripted_and_hac() {
        let dev = Device::Cpu;
        // scripted archetype
        let c = build_contender("aggro", "aggressor", &dev, A1).unwrap();
        assert_eq!(c.id, "aggro");
        assert!(matches!(c.controller, Controller::Scripted(_)));
        // hac from a freshly-saved A1 varmap
        let dir = std::env::temp_dir().join("tourney_build_contender");
        std::fs::create_dir_all(&dir).unwrap();
        let ck = dir.join("hac.safetensors");
        let t = crate::JointPpoTrainer::new(Device::Cpu, A1, crate::JointPpoConfig::smoke_default()).unwrap();
        t.varmap.save(&ck).unwrap();
        let h = build_contender("sota", &format!("hac:{}", ck.display()), &dev, A1).unwrap();
        assert_eq!(h.id, "sota");
        assert!(matches!(h.controller, Controller::Hac(_)));
    }
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p antcolony-trainer --lib tournament` → FAIL (module/types missing).
- [ ] **Step 3: Implement** the module skeleton:

```rust
//! PvP round-robin tournament: rank any mix of HAC + scripted brains on a
//! Bradley-Terry/Elo ladder over the 2-colony engine.

use std::path::PathBuf;
use anyhow::Result;
use candle_core::Device;

use antcolony_sim::ai::brain::AiBrain;
use crate::hierarchical::sizing::Sizing;
use crate::self_play::load_frozen_hac;
use crate::{HierarchicalActorCritic, League};

/// How a contender drives its colony.
pub enum Controller {
    /// Hierarchical brain: commander intents + per-ant modulators.
    Hac(HierarchicalActorCritic),
    /// Scripted colony-level brain; the sim runs its default ant behavior.
    Scripted(Box<dyn AiBrain>),
}

/// One enrolled brain.
pub struct Contender {
    pub id: String,
    pub spec: String,
    pub controller: Controller,
}

/// Build a contender from a spec. `"hac:<path>"` loads a frozen HAC checkpoint;
/// any other spec (`"heuristic"`, `"mlp:<path>"`, `"mix:..."`, archetype names)
/// resolves via `League::make_brain`.
pub fn build_contender(id: &str, spec: &str, device: &Device, sizing: Sizing) -> Result<Contender> {
    let controller = if let Some(path) = spec.strip_prefix("hac:") {
        Controller::Hac(load_frozen_hac(&PathBuf::from(path), sizing, device)?)
    } else {
        Controller::Scripted(League::make_brain(spec, 0)?)
    };
    tracing::info!(id, spec, hac = matches!(controller, Controller::Hac(_)), "tournament: contender built");
    Ok(Contender { id: id.to_string(), spec: spec.to_string(), controller })
}
```
Add `pub mod tournament;` to `lib.rs`. (Verify the exact `League::make_brain` signature + `load_frozen_hac` arg types against the source; adjust the `make_brain` seed arg if its signature differs.)

- [ ] **Step 4: Run** — `cargo test -p antcolony-trainer --lib tournament` PASS; `cargo build -p antcolony-trainer` clean.
- [ ] **Step 5: Commit** — `feat(trainer): tournament Contender/Controller + spec resolution`

---

### Task 2: `play_pair` heterogeneous match runner + refactor (LOAD-BEARING, two-stage review)

**Files:** Modify `crates/antcolony-trainer/src/eval.rs` (add `play_pair` + helpers; re-wire `play_match`/`play_match_h2h` to call it). Test: in-file `eval` tests (existing must stay green) + a new `play_pair` scripted-vs-scripted test.

**Interfaces — Produces:**
- `pub fn play_pair(left: &mut crate::tournament::Controller, right: &mut crate::tournament::Controller, device: &Device, seed: u64, max_ticks: u64) -> Result<(f32, f32, MatchEnd)>` — returns `(left_worker_share, left_decisive, MatchEnd)` from LEFT's perspective (same as `score_match`); the scheduler derives right's scores as `1.0 - left`.

**Consumes:** `crate::tournament::Controller` (T1); all the `env.sim.*` drive calls + tensor helpers already used by `play_match`/`play_match_h2h` (see Global Constraints).

**Implementation note (the refactor):** `play_match` (HAC-left vs scripted-right) and `play_match_h2h` (HAC both) already share one structure: per decision cycle, run each side's **commander phase** (HAC: `colony_rich_observation`→`rich_to_tensors`→`mean_commander_action`→`apply_ai_decision`+`apply_commander_intent`, keep the `intent` tensor; scripted: `colony_ai_state`→`brain.decide`→`apply_ai_decision`), then `DECISION_CADENCE` ticks where each HAC side applies ant modulators (`per_ant_observations`→`ant_obs_to_tensors`→`mean_ant_modulator`→`apply_ant_modulators`) before `tick()`. `play_pair` generalizes this to a per-side `Controller`. **Preserve the exact break-on-missing-colony behavior:** both existing fns break the match loop when a HAC side's `colony_rich_observation` is `None` — `play_pair` must break then too (a `CommanderResult::Gone`), while a scripted side's `colony_ai_state == None` just skips that side's decision (as `play_match`'s `if let Some(sr)` does). This ordering keeps `play_match`/`play_match_h2h` byte-identical when re-wired (the sim's per-tick RNG is unaffected by which side applies decisions first; no RNG is drawn during commander/ant application).

- [ ] **Step 1: Write a failing test** (scripted-vs-scripted, the path neither existing fn covers):

```rust
#[test]
fn play_pair_scripted_vs_scripted_runs_and_scores() {
    use crate::tournament::Controller;
    let dev = candle_core::Device::Cpu;
    let mut a = Controller::Scripted(crate::League::make_brain("aggressor", 1).unwrap());
    let mut b = Controller::Scripted(crate::League::make_brain("economist", 2).unwrap());
    let (ws, dec, _end) = super::play_pair(&mut a, &mut b, &dev, 12345, 2000).unwrap();
    assert!((0.0..=1.0).contains(&ws));
    assert!((0.0..=1.0).contains(&dec));
}
```

- [ ] **Step 2: Run, verify fail** — `play_pair` missing.
- [ ] **Step 3: Implement** `play_pair` + two private helpers, then re-wire:

```rust
use crate::tournament::Controller;

enum CommanderResult { Gone, Hac(candle_core::Tensor), Scripted }

/// Run one colony's commander phase for this decision cycle.
fn commander_phase(ctrl: &mut Controller, side: usize, env: &mut MatchEnv, device: &Device) -> Result<CommanderResult> {
    match ctrl {
        Controller::Hac(hac) => {
            let rich = match env.sim.colony_rich_observation(side) { Some(r) => r, None => return Ok(CommanderResult::Gone) };
            let (s, p, h) = rich_to_tensors(&rich, device)?;
            let (action, intent, _v) = hac.mean_commander_action(&s, &p, &h)?;
            let av: Vec<f32> = action.flatten_all()?.to_vec1()?;
            debug_assert_eq!(av.len(), 6, "commander action expects 6 dims");
            let dec = antcolony_sim::AiDecision {
                caste_ratio_worker: av[0], caste_ratio_soldier: av[1], caste_ratio_breeder: av[2],
                forage_weight: av[3], dig_weight: av[4], nurse_weight: av[5], research_choice: None,
            };
            env.sim.apply_ai_decision(side, &dec);
            let iv: Vec<f32> = intent.flatten_all()?.to_vec1()?;
            let mut intent_arr = [0.0f32; 64];
            intent_arr.copy_from_slice(&iv);
            env.sim.apply_commander_intent(side, &intent_arr);
            Ok(CommanderResult::Hac(intent))
        }
        Controller::Scripted(brain) => {
            if let Some(sr) = env.sim.colony_ai_state(side) {
                let dr = brain.decide(&sr);
                env.sim.apply_ai_decision(side, &dr);
            }
            Ok(CommanderResult::Scripted)
        }
    }
}

/// Apply one HAC colony's ant modulators for the current tick (no-op for scripted).
fn ant_phase(ctrl: &Controller, side: usize, cmd: &CommanderResult, env: &mut MatchEnv, device: &Device) -> Result<()> {
    if let (Controller::Hac(hac), CommanderResult::Hac(intent)) = (ctrl, cmd) {
        let obs = env.sim.per_ant_observations(side);
        if !obs.is_empty() {
            let (cone, internal, intent_b) = ant_obs_to_tensors(&obs, intent, device)?;
            let mods_t = hac.mean_ant_modulator(&cone, &internal, &intent_b)?;
            let flat: Vec<f32> = mods_t.flatten_all()?.to_vec1()?;
            let mut mods = Vec::with_capacity(obs.len());
            let mut ids = Vec::with_capacity(obs.len());
            for (k, o) in obs.iter().enumerate() {
                let off = k * 5;
                mods.push(AntModulators {
                    alpha_mult: flat[off], beta_mult: flat[off + 1], exploration_mod: flat[off + 2],
                    deposit_mult: flat[off + 3], state_bias: flat[off + 4],
                });
                ids.push(o.ant_id);
            }
            env.sim.apply_ant_modulators(side, &mods, &ids);
        }
    }
    Ok(())
}

pub fn play_pair(
    left: &mut Controller, right: &mut Controller, device: &Device, seed: u64, max_ticks: u64,
) -> Result<(f32, f32, MatchEnd)> {
    let mut env = MatchEnv::new(seed);
    env.max_ticks = max_ticks;
    loop {
        let cl = commander_phase(left, 0, &mut env, device)?;
        if matches!(cl, CommanderResult::Gone) { break; }
        let cr = commander_phase(right, 1, &mut env, device)?;
        if matches!(cr, CommanderResult::Gone) { break; }

        let mut done = false;
        for _ in 0..DECISION_CADENCE {
            ant_phase(left, 0, &cl, &mut env, device)?;
            ant_phase(right, 1, &cr, &mut env, device)?;
            env.sim.tick();
            if !matches!(env.sim.match_status(), MatchStatus::InProgress) || env.sim.tick >= env.max_ticks {
                done = true; break;
            }
        }
        if done { break; }
    }
    let lw = env.sim.colonies.first().map(|c| c.population.workers).unwrap_or(0) as f32;
    let rw = env.sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0) as f32;
    Ok(score_match(env.sim.match_status(), lw, rw))
}
```
Then **re-wire** `play_match` and `play_match_h2h` to delegate to `play_pair` (build the two `Controller`s and call it) so their behavior is preserved by construction, e.g.:
```rust
fn play_match(hac: &HierarchicalActorCritic, device: &Device, opp_spec: &str, seed: u64) -> Result<(f32, f32, MatchEnd)> {
    // NOTE: load_frozen_hac needs a path; play_match holds a &HAC by ref. Either
    // (a) keep play_match's body as-is (do NOT re-wire it) and only ADD play_pair,
    // or (b) make play_pair take Controllers the caller already owns. Prefer (a)
    // if re-wiring would force a clone/reload of the in-memory HAC — the goal is
    // byte-identical behavior, NOT forced unification. The reviewer decides.
    ...
}
```
**Re-wiring caveat (resolve in review):** `play_match`/`play_match_h2h` receive `&HierarchicalActorCritic` (borrowed, in-memory), but `Controller::Hac` owns its HAC. If wrapping the borrow in a `Controller` is awkward, the byte-identical-preserving choice is to leave `play_match`/`play_match_h2h` bodies untouched and only ADD `play_pair` (sharing the helpers where a borrow works). Either way the existing eval tests must stay byte-identical green — that is the acceptance bar, not the re-wiring itself.

- [ ] **Step 4: Run** — the new test + the FULL existing eval suite: `cargo test -p antcolony-trainer --lib eval` (every prior eval test must still pass). `cargo build` clean.
- [ ] **Step 5: Commit** — `feat(trainer): play_pair heterogeneous match runner`

**Review:** full spec + code-quality review — determinism-critical (byte-identical `play_match`/`play_match_h2h`) and the shared sim drive loop.

---

### Task 3: Bradley-Terry → Elo (pure fn)

**Files:** Modify `tournament.rs`. Test: in-file.

**Interfaces — Produces:**
- `pub fn bradley_terry_elo(win_matrix: &[Vec<f32>], games: &[Vec<usize>], anchor_idx: Option<usize>, anchor_elo: f64) -> Vec<f64>` — MM iteration to a strength vector, returned on the Elo scale, anchored.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn bradley_terry_ranks_dominance_order_and_anchors() {
    // 3 brains, strict dominance A>B>C: W[i][j] = i's score vs j.
    let w = vec![
        vec![f32::NAN, 0.8, 0.9],
        vec![0.2, f32::NAN, 0.8],
        vec![0.1, 0.2, f32::NAN],
    ];
    let g = vec![vec![0, 10, 10], vec![10, 0, 10], vec![10, 10, 0]];
    let elo = bradley_terry_elo(&w, &g, Some(1), 1000.0);
    assert!(elo[0] > elo[1] && elo[1] > elo[2], "A>B>C: {elo:?}");
    assert!((elo[1] - 1000.0).abs() < 1e-6, "anchor pegged: {}", elo[1]);
    // symmetric (all 0.5) -> equal ratings
    let we = vec![vec![f32::NAN, 0.5], vec![0.5, f32::NAN]];
    let ge = vec![vec![0, 10], vec![10, 0]];
    let eloe = bradley_terry_elo(&we, &ge, None, 1000.0);
    assert!((eloe[0] - eloe[1]).abs() < 1e-3, "equal: {eloe:?}");
}
```

- [ ] **Step 2: Run, verify fail.**
- [ ] **Step 3: Implement**

```rust
/// Bradley-Terry strengths via MM iteration, returned on the Elo scale.
/// `win_matrix[i][j]` = i's mean score vs j in [0,1] (diag ignored); `games[i][j]`
/// = number of games i-vs-j. `anchor_idx` (if set) is pegged to `anchor_elo`;
/// otherwise the mean Elo is centered at `anchor_elo`.
pub fn bradley_terry_elo(win_matrix: &[Vec<f32>], games: &[Vec<usize>], anchor_idx: Option<usize>, anchor_elo: f64) -> Vec<f64> {
    let n = win_matrix.len();
    if n == 0 { return Vec::new(); }
    // win credit W_i = Σ_j score[i][j] * games[i][j]  (draws already 0.5 in score)
    let mut wins = vec![0.0f64; n];
    for i in 0..n {
        for j in 0..n {
            if i == j { continue; }
            let s = win_matrix[i][j];
            if s.is_finite() { wins[i] += s as f64 * games[i][j] as f64; }
        }
    }
    let mut p = vec![1.0f64; n];
    for _ in 0..200 {
        let mut np = vec![0.0f64; n];
        for i in 0..n {
            let mut denom = 0.0f64;
            for j in 0..n {
                if i == j { continue; }
                let g = games[i][j] as f64;
                if g > 0.0 { denom += g / (p[i] + p[j]); }
            }
            np[i] = if denom > 0.0 { (wins[i] / denom).max(1e-12) } else { p[i] };
        }
        // normalize to keep numbers bounded
        let sum: f64 = np.iter().sum();
        if sum > 0.0 { for x in np.iter_mut() { *x /= sum; } }
        p = np;
    }
    let mut elo: Vec<f64> = p.iter().map(|&pi| 400.0 * pi.max(1e-12).log10()).collect();
    let shift = match anchor_idx {
        Some(a) if a < n => anchor_elo - elo[a],
        _ => anchor_elo - elo.iter().sum::<f64>() / n as f64,
    };
    for e in elo.iter_mut() { *e += shift; }
    elo
}
```

- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** — `feat(trainer): Bradley-Terry -> Elo rating`

---

### Task 4: Non-transitivity (cycle) detection (pure fn)

**Files:** Modify `tournament.rs`. Test: in-file.

**Interfaces — Produces:**
- `pub fn find_cycles(win_matrix: &[Vec<f32>], margin: f32) -> Vec<(usize, usize, usize)>` — distinct ordered 3-cycles where each edge wins by `> margin`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn find_cycles_detects_rps_and_ignores_transitive() {
    // A beats B beats C beats A (rock-paper-scissors)
    let cyc = vec![
        vec![f32::NAN, 0.7, 0.3],
        vec![0.3, f32::NAN, 0.7],
        vec![0.7, 0.3, f32::NAN],
    ];
    assert_eq!(find_cycles(&cyc, 0.55).len(), 1, "one 3-cycle");
    // strict dominance A>B>C: no cycle
    let tr = vec![
        vec![f32::NAN, 0.8, 0.9],
        vec![0.2, f32::NAN, 0.8],
        vec![0.1, 0.2, f32::NAN],
    ];
    assert!(find_cycles(&tr, 0.55).is_empty(), "transitive: no cycle");
}
```

- [ ] **Step 2: Run, verify fail.**
- [ ] **Step 3: Implement**

```rust
/// Find distinct 3-cycles (i beats j beats k beats i), each edge by `> margin`.
/// Each cycle reported once with its smallest index first.
pub fn find_cycles(win_matrix: &[Vec<f32>], margin: f32) -> Vec<(usize, usize, usize)> {
    let n = win_matrix.len();
    let beats = |a: usize, b: usize| win_matrix[a][b].is_finite() && win_matrix[a][b] > margin;
    let mut out = Vec::new();
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                if i == j || j == k || i == k { continue; }
                // canonical: i is the smallest of the three
                if i < j && i < k && beats(i, j) && beats(j, k) && beats(k, i) {
                    out.push((i, j, k));
                }
            }
        }
    }
    out
}
```

- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** — `feat(trainer): tournament cycle detection`

---

### Task 5: Round-robin scheduler + `TournamentResult`

**Files:** Modify `tournament.rs`. Test: in-file integration (scripted-only, FAST).

**Interfaces:**
- Consumes: `build_contender` (T1), `play_pair` (T2), `bradley_terry_elo` (T3), `find_cycles` (T4), `eval::spec_seed_salt`.
- Produces:
  - `pub struct TournamentConfig { pub contenders: Vec<(String, String)>, pub mpe: usize, pub max_ticks: u64, pub anchor_id: String, pub anchor_elo: f64, pub cycle_margin: f32, pub sizing: Sizing }` with a `smoke()` default.
  - `pub struct TournamentResult { pub ids: Vec<String>, pub specs: Vec<String>, pub win_matrix: Vec<Vec<f32>>, pub ws_matrix: Vec<Vec<f32>>, pub games: Vec<Vec<usize>>, pub elo: Vec<f64>, pub winrate_vs_field: Vec<f32>, pub cycles: Vec<(usize,usize,usize)> }`
  - `pub fn run_tournament(cfg: &TournamentConfig, device: &Device) -> Result<TournamentResult>`

**Implementation note:** build the `N(N-1)/2` unordered pairings, run them with rayon `par_iter`. Each pairing **builds its own two `Contender`s** inside the closure (so no `&mut`/non-`Sync` controller crosses threads, and HAC is loaded per pairing — read-only, deterministic). For pair `(i,j)`: `mpe` matches i-left/j-right (seed salt `"{id_i}:{id_j}:L"`) + `mpe` j-left/i-right (salt `"{id_j}:{id_i}:L"`); accumulate i's decisive score (i's `left_dec` in the first half, `1.0 - left_dec` in the swapped half) → `W[i][j]` (decisive) + `ws_matrix[i][j]` (worker-share, same derivation); mirror `W[j][i] = 1 - W[i][j]`. A pairing whose contender fails to build → log + skip (leave that pair's cells NaN/0 games), do not abort. After the matrix is built, fill the diagonal with `f32::NAN`, compute `elo` via `bradley_terry_elo` (anchor = index of `anchor_id` if present), `winrate_vs_field[i]` = mean of finite `W[i][*]`, and `cycles` via `find_cycles(&win_matrix, cfg.cycle_margin)`.

- [ ] **Step 1: Write a failing integration test** (scripted-only so it's FAST — no HAC/candle):

```rust
#[test]
fn run_tournament_scripted_smoke() {
    let dev = candle_core::Device::Cpu;
    let cfg = TournamentConfig {
        contenders: vec![
            ("aggro".into(), "aggressor".into()),
            ("econ".into(), "economist".into()),
            ("def".into(), "defender".into()),
        ],
        mpe: 1, max_ticks: 1500, anchor_id: "econ".into(), anchor_elo: 1000.0,
        cycle_margin: 0.55, sizing: crate::hierarchical::sizing::A1,
    };
    let r = run_tournament(&cfg, &dev).unwrap();
    assert_eq!(r.ids.len(), 3);
    assert_eq!(r.elo.len(), 3);
    // every off-diagonal pair played 2*mpe games; symmetric
    for i in 0..3 { for j in 0..3 { if i != j {
        assert_eq!(r.games[i][j], 2);
        assert!((r.win_matrix[i][j] + r.win_matrix[j][i] - 1.0).abs() < 1e-5, "symmetric");
    }}}
    assert!(r.elo.iter().all(|e| e.is_finite()));
    assert!((r.elo[1] - 1000.0).abs() < 1e-6, "anchor econ pegged");
}
```

- [ ] **Step 2: Run, verify fail.**
- [ ] **Step 3: Implement** `TournamentConfig` (+ `smoke()`), `TournamentResult`, `run_tournament` per the note. Use `rayon::prelude::*` over the pairing list; seed each match `0xE7A1.wrapping_mul(spec_seed_salt(&salt)) ^ (m * 0x9E3779B97F4A7C15)`.
- [ ] **Step 4: Run** — the new test + regressions (`cargo test -p antcolony-trainer --lib tournament` + `--lib eval`). FAST locally (scripted-only); the HAC-laden full run is cnc-only.
- [ ] **Step 5: Commit** — `feat(trainer): round-robin scheduler + TournamentResult`

---

### Task 6: `tournament` bin + report writers + cnc script

**Files:** Create `crates/antcolony-trainer/src/bin/tournament.rs`; create `scripts/run_tournament_cnc.sh`. Test: build + a FAST scripted-only CLI smoke.

**Interfaces:** Consumes `TournamentConfig`, `run_tournament`, `TournamentResult`. CLI (mirror `phase3_train`'s hand-rolled parser): `--contenders <comma list of id=spec>` (e.g. `sota=hac:bench/phase3-a1-combat/hac_best.safetensors,v1=mlp:bench/iterative-fsp/round_1/mlp_weights_v1.json,aggressor=aggressor,...`); `--add-archetypes` (append the 7 `BENCH_ARCHETYPES` as `name=name` if absent); `--mpe <N>` (default 15); `--max-ticks <N>` (default 10000); `--anchor <id>` (default `v1`); `--anchor-elo <f>` (default 1000); `--cycle-margin <f>` (default 0.55); `--sizing a1`; `--out <dir>` (default `bench/tournament`).

- [ ] **Step 1: Add the bin** — parse flags → `TournamentConfig` → `CandleBackend::new()?` device → `run_tournament` → write `ladder.md` (rank/id/elo/winrate-vs-field/spec, sorted by Elo desc), `win_matrix.csv` (decisive) + `ws_matrix.csv` (worker-share), `ratings.json` (serde over the result), and a `tracing::info!` summary (top-3 + SOTA's rank + cycle count). `std::fs::create_dir_all(out)`. Log full config at startup. Example writer for the ladder:

```rust
use std::io::Write;
let mut order: Vec<usize> = (0..r.ids.len()).collect();
order.sort_by(|&a, &b| r.elo[b].partial_cmp(&r.elo[a]).unwrap_or(std::cmp::Ordering::Equal));
let mut s = String::from("# Tournament Ladder\n\n| rank | id | elo | winrate_vs_field | spec |\n|---|---|---:|---:|---|\n");
for (rank, &i) in order.iter().enumerate() {
    s.push_str(&format!("| {} | {} | {:.0} | {:.3} | `{}` |\n", rank + 1, r.ids[i], r.elo[i], r.winrate_vs_field[i], r.specs[i]));
}
std::fs::write(out.join("ladder.md"), s)?;
```

- [ ] **Step 2: Build** — `cargo build --release -p antcolony-trainer --bin tournament`. Clean.
- [ ] **Step 3: FAST scripted-only CLI smoke** (local): `cargo run --release -p antcolony-trainer --bin tournament -- --contenders aggro=aggressor,econ=economist --add-archetypes --mpe 1 --max-ticks 1500 --anchor heuristic --out bench/tourney-smoke` — completes, writes `ladder.md`/`win_matrix.csv`/`ratings.json`. (No HAC → fast, no GPU.) Do NOT run a HAC-laden tournament locally — cnc only.
- [ ] **Step 4: Write `scripts/run_tournament_cnc.sh`** — copy `scripts/run_selfplay_cnc.sh`'s service-restore scaffolding but **kick the FULL fleet** (the tournament is CPU-sim-bound; free all cores). Set `SERVICES="openclaw-inference-workhorse openclaw-inference-scout openclaw-inference-embed aether-vision aether-serve"`; keep the `trap restore EXIT/TERM/INT/HUP` pattern; `CUDA_VISIBLE_DEVICES=GPU-17bd0d20-...`; the split `LD_LIBRARY_PATH`; `export RAYON_NUM_THREADS=$(nproc)`; `cd /opt/antcolony-cuda`. Invocation: `./target/release/tournament --contenders sota=hac:bench/phase3-a1-combat/hac_best.safetensors,v1=mlp:bench/iterative-fsp/round_1/mlp_weights_v1.json,sp1=hac:bench/phase3-sp1/hac_best.safetensors,sp1term=hac:bench/phase3-sp1-terminal/hac_best.safetensors,sp2=hac:bench/phase3-sp2/league_best.safetensors,gradclip=hac:bench/phase3-a1-gradclip/hac_best.safetensors --add-archetypes --mpe 15 --anchor v1 --out bench/tournament` then write the exit code to `/opt/antcolony-cuda/run_tournament.done`. Header comment: full-fleet-kick rationale + checkpoint-presence prerequisite (pull sp1-terminal/sp2 from `/opt/antcolony-archive/` first). Make it executable.
- [ ] **Step 5: Commit** — `feat(trainer): tournament bin + CLI + run_tournament_cnc.sh`

---

## Self-Review

**Spec coverage:** §3 reuse → Global Constraints + every task consumes the named items. §4.1 Contender/Controller → T1. §4.2 play_pair → T2. §4.3 scheduler → T5. §4.4 Bradley-Terry → T3. §4.5 cycles → T4. §5 output → T6. §6 CLI → T6. §7 cost/venue → T6 cnc script. §8 safety (skip bad checkpoint, no panic, per-pairing controller ownership, determinism) → T1/T2/T5. §9 testing → each task TDD. §10 scope honored. §11 open questions resolved by defaults (curated contenders via CLI; both metrics; mpe=15) — encoded in T6.

**Placeholder scan:** Tasks 1,3,4 have complete code. Task 2 has complete `play_pair` + helpers; the only deliberately-open point is whether `play_match`/`play_match_h2h` are re-wired or left intact (resolved by the byte-identical-tests acceptance bar + flagged for review — a real engineering judgment, not a placeholder). Task 5's `run_tournament` body is described with the exact accumulation + seeding (best done test-first over the existing primitives). Task 6's writers are shown.

**Type consistency:** `Controller`/`Contender`/`build_contender(id,spec,device,sizing)`, `play_pair(&mut Controller, &mut Controller, device, seed, max_ticks) -> (f32,f32,MatchEnd)`, `bradley_terry_elo(win_matrix, games, anchor_idx, anchor_elo) -> Vec<f64>`, `find_cycles(win_matrix, margin)`, `TournamentConfig{contenders,mpe,max_ticks,anchor_id,anchor_elo,cycle_margin,sizing}`, `TournamentResult{ids,specs,win_matrix,ws_matrix,games,elo,winrate_vs_field,cycles}`, `run_tournament(&cfg, device)` — consistent across T1-T6.

## Notes
- **First run = curated ~6 HAC/MLP + 7 archetypes at mpe=15 on cnc** (full-fleet-kick, RAYON=nproc). Then a high-mpe top-N confirm. Coordinate the GPU window via openclaw main + confirm cloud fallback; ⚠ pull `sp1-terminal`/`sp2` checkpoints from `/opt/antcolony-archive/` into cnc `bench/` first; ledger + backup after.
- **Headline:** where does the 0.874-bench SOTA rank vs v1 and the self-play/league brains, and are there cycles among the strong contenders? That answers "was the bench the wrong target."

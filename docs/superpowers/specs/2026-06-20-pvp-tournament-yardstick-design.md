# PvP Tournament Yardstick — Design Spec

**Date:** 2026-06-20
**Status:** design — awaiting approval before writing-plans
**Project:** antcolony hierarchical-brain (HAC) RL evaluation
**Goal:** Replace/augment the saturated 7-archetype bench with a **round-robin PvP tournament** over the existing 2-colony engine that ranks every brain on one relative (Elo) scale — answering whether the 0.874-bench SOTA is *actually* the best in head-to-head play, or whether the fixed-archetype bench mis-ranked brains. Reusable as a standing ladder; the win-matrix exposes non-transitivity (rock-paper-scissors) for free.

## 1. Why (what the bench can't tell us)

The 7-archetype bench is saturated: the combat SOTA wins ~87% by queen-kill and the metric has Nash-collapsed. Every recursive-self-learning attempt (SP1 #1/#2, run #3 terminal, SP2 league) "loses" on that bench — but the bench only measures play against **7 fixed scripted opponents**, never against other *learned* brains. We cannot tell whether (a) the SOTA genuinely dominates, (b) a brain the bench scored low (e.g. the terminal-self-play near-peer, h2h 0.395 vs SOTA) is secretly competitive, or (c) the brains form non-transitive cycles that make any single fixed-opponent bench the wrong training target. A round-robin ladder over **all** brains answers all three.

## 2. Scope decisions (locked in brainstorming)

- **Shape:** 2-colony round-robin tournament + Elo. NOT N-colony free-for-all (the sim is hard 2-colony — FFA is a separate future project; see §10).
- **Primary question (#1):** is the SOTA really the best head-to-head? (reporting headline)
- **Secondary (#3):** build it reusable as a standing ladder (every future checkpoint enrolls).
- **Free bonus (#2):** non-transitivity / cycle detection from the win-matrix (zero extra build cost).
- **NOT in scope:** training against the ladder (population-based training) — a separate future project once the yardstick is trusted.

## 3. Reuse from the existing codebase (do NOT rebuild)

- **`AiBrain` trait** (`crates/antcolony-sim/src/ai/brain.rs:118`): `fn name(&self) -> &str; fn decide(&mut self, state: &ColonyAiState) -> AiDecision;`. All 7 archetypes + `MlpBrain` (v1) + `MixedBrain` etc. implement it.
- **`MatchEnv`** (`crates/antcolony-trainer/src/env.rs`): the 2-colony match environment. `new(seed)`, `observe(colony_id) -> Option<ColonyAiState>`, `step(action_left, action_right) -> StepRecord`. Holds the HAC tensor machinery (`all_ant_obs_batch` for `[0,1]`, `apply_commander_intents` `[2, FIXED_INTENT_D]`, `commander_obs_batch` over `colony_rich_observation(0)`/`(1)`).
- **`eval.rs`**: `score_match(status, lw, rw) -> (worker_share, decisive, MatchEnd)` (`eval.rs:90`); `play_match` (HAC-left vs scripted-right, `eval.rs:113`); `play_match_h2h` (HAC-left vs HAC-right, `eval.rs:258`); `spec_seed_salt` (FNV-1a, `eval.rs:187`); the seed pattern `0xE7A1 * salt ^ (m * 0x9E3779B97F4A7C15)`; `BENCH_ARCHETYPES` (`eval.rs:19`). The **decisive** metric (timeout = draw 0.5, only queen-kill scores 1/0) is the tournament's scoring.
- **`load_frozen_hac(path, sizing, device) -> Result<HierarchicalActorCritic>`** (`self_play.rs:23`) — load any HAC checkpoint.
- **`League::make_brain(spec, seed) -> Box<dyn AiBrain>`** (`league.rs`) — resolves `"mlp:<path>"`, `"heuristic"`, `"mix:..."`, and archetype names.
- **`Simulation::match_status() -> MatchStatus`** (`simulation.rs:315`): `Won { winner, loser, ended_at_tick }` / `InProgress`; timeout graded by `score_match`.
- **Determinism**: `ChaCha8Rng` from `u64`; side-swap cancels the structural left-colony advantage.

## 4. Architecture / new units

New module **`crates/antcolony-trainer/src/tournament.rs`** + bin **`crates/antcolony-trainer/src/bin/tournament.rs`**.

### 4.1 `Contender` — a uniform handle for any brain (the core abstraction)

```rust
pub enum Controller {
    Hac(HierarchicalActorCritic),     // hierarchical: commander intents + ant policy
    Scripted(Box<dyn AiBrain>),       // colony-level AiDecision + the sim's default ant behavior
}
pub struct Contender {
    pub id: String,        // unique label, e.g. "sota", "v1", "sp1-terminal", "aggressor"
    pub spec: String,      // how it was constructed (for the report)
    pub controller: Controller,
}
```

A `Contender` is built from a **spec string** (§4.4): `"hac:<path>"` → `Controller::Hac(load_frozen_hac(...))`; `"mlp:<path>"` / archetype name / `"mix:..."` → `Controller::Scripted(League::make_brain(...))`.

### 4.2 Heterogeneous match runner (the main build risk)

HAC and scripted brains drive a colony at **different levels**: a HAC controls the colony's commander intents AND its individual ants (hierarchical); a scripted `AiBrain` produces only a colony-level `AiDecision` and lets the sim's built-in ant behavior execute it. The existing code only covers two of the four side-combinations (`play_match` = HAC-vs-scripted, `play_match_h2h` = HAC-vs-HAC). The tournament needs **any controller on either side** (4 combos incl. scripted-vs-scripted and scripted-left/HAC-right).

Build one runner:
```rust
pub fn play_pair(
    left: &mut Controller,
    right: &mut Controller,
    device: &Device,
    seed: u64,
    max_ticks: u64,
) -> Result<(f32, f32, MatchEnd)>;   // (left_decisive_score, right_decisive_score, end)
```
Implementation: one `MatchEnv` seeded by `seed`; each decision cycle, for **each** side independently produce its control by its controller's path — HAC sides reuse the existing commander-forward + ant-policy-forward machinery from `env.rs`/`play_match*` (only consuming the outputs for that side); scripted sides call `brain.decide(env.observe(side))`. Step the sim with both sides' control until `match_status()` terminates or `max_ticks`. Score via `score_match`. **Refactor the shared loop out of `play_match`/`play_match_h2h`** (which become thin callers of `play_pair`) rather than duplicating — leaving their existing behavior byte-identical (guarded by the existing eval tests).

`play_pair` returns the **decisive** score per side (worker-share also available but the ladder uses decisive). Side-swap is handled by the scheduler, not here.

### 4.3 Scheduler — round-robin, side-swapped, parallel

```rust
pub struct TournamentConfig {
    pub contenders: Vec<String>,   // specs
    pub mpe: usize,                // matches per (pair, side) — total per pair = 2*mpe
    pub max_ticks: u64,            // default 10_000
    pub out_dir: PathBuf,
    pub anchor_id: String,         // brain pegged to anchor_elo (default "v1")
    pub anchor_elo: f64,           // default 1000.0
}
pub struct TournamentResult {
    pub ids: Vec<String>,
    pub win_matrix: Vec<Vec<f32>>, // [i][j] = i's mean decisive score vs j (0..1), diag = NaN
    pub games: Vec<Vec<usize>>,    // [i][j] = games played i-vs-j (= 2*mpe), symmetric
    pub elo: Vec<f64>,             // Bradley-Terry rating, Elo-scaled, anchored
    pub winrate_vs_field: Vec<f32>,// mean score across all opponents
    pub cycles: Vec<(usize,usize,usize)>, // notable 3-cycles (i>j>k>i)
}
```
For each unordered pair `(i,j)`: play `mpe` matches `i`-left/`j`-right + `mpe` matches `j`-left/`i`-right (side-swap), seed `= 0xE7A1 * pair_salt(i,j,side) ^ (m * 0x9E3779B97F4A7C15)` where `pair_salt` = FNV-1a over `"{id_i}:{id_j}:{side}"` (distinct per side so the two halves don't share seeds). `play_pair` returns BOTH sides' decisive scores, so accumulate `W[i][j]` = i's own mean decisive score over all `2*mpe` games — i's `left_score` in the i-left half, i's `right_score` in the swapped (j-left) half (no `1-x` flip needed since both sides' scores come back directly). It is symmetric by construction (`W[j][i] = 1 - W[i][j]` up to fp), so compute one triangle and mirror. Parallelize over the `N(N-1)/2` pairings with rayon (each pairing is independent; HAC forwards are read-only on a frozen net, so the loop is structured so each pairing owns its two controllers — see §8).

### 4.4 Rating — Bradley-Terry → Elo

Fit each contender a strength `p_i > 0` maximizing the likelihood of the observed scores via the standard **MM (minorization-maximization) iteration**:
```
W_i  = Σ_j  W[i][j] * games[i][j]        // i's total win-credit (draws = 0.5 already in W)
p_i ← W_i / Σ_{j≠i} ( games[i][j] / (p_i + p_j) )   // repeat to convergence
normalize Σ p_i = 1
```
Convert to Elo: `elo_i = 400 * log10(p_i) + C`, choosing `C` so `elo[anchor_id] = anchor_elo` (v1 = 1000). If `anchor_id` is not in the contender set, fall back to centering the mean Elo at `anchor_elo` (log a warning). Draws (decisive timeouts) are already 0.5 win-credit in `W`, which BT handles natively. Converges in a few dozen iterations on a dense round-robin.

**Why BT over sequential Elo:** the round-robin is a fixed batch; sequential Elo's result depends on match order. BT gives an order-independent maximum-likelihood rating — the standard for engine ladders.

### 4.5 Non-transitivity (free)

Scan ordered triples; flag 3-cycles where `W[i][j] > 0.5 && W[j][k] > 0.5 && W[k][i] > 0.5` (each edge a real margin, e.g. `> 0.55`). Report count + the notable cycles. Cycles among strong contenders are the headline evidence that a single fixed-opponent bench is the wrong target.

## 5. Output

`tournament.rs` bin writes to `out_dir`:
- **`ladder.md`** — ranked table: rank, id, Elo, win-rate-vs-field, W-L-D record, spec. The headline answer (where SOTA / v1 / the self-play & league brains land).
- **`win_matrix.csv`** — full `W[i][j]` matrix (+ a markdown rendering in `ladder.md`).
- **`ratings.json`** — machine-readable `TournamentResult` for re-use / future appends.
- Startup `tracing::info!` logs the full config; per-pairing progress at debug. **No `println!`** except the final summary to stdout.

Re-runnable as the standing ladder (#3): deterministic, so re-running with the same contenders reproduces; enroll new checkpoints by adding a spec / `--glob`.

## 6. CLI (`bin/tournament.rs`)

Mirror `phase3_train`'s hand-rolled parser: `--contenders <comma-list-of-specs>` OR `--glob <pattern>` (enumerate `bench/**/*.safetensors` as `hac:<path>` + always include the 7 archetypes + `--v1 <path>`); `--mpe <N>` (default 30); `--max-ticks <N>` (default 10000); `--anchor <id>` (default v1) / `--anchor-elo <f>` (default 1000); `--out <dir>` (default `bench/tournament`); `--sizing a1`. Builds `TournamentConfig`, runs the scheduler, computes BT/Elo + cycles, writes the report.

## 7. Cost / venue

~14 curated contenders × `mpe=30` → `91 pairs × 60 matches ≈ 5,500` full-length sim matches → a **cnc job** (CPU sim is the bottleneck; HAC contenders' forwards run on the CUDA device). Full-fleet-kickable per the standing training-run authorization. **First pass at `mpe=15`** for the ranking shape, then a high-`mpe` confirm of the top contenders. The contender checkpoints (incl. `sp1-terminal`, `sp2 league_best`, currently in `/opt/antcolony-archive/`) must be present on the run box first.

## 8. Error handling / safety / determinism

- **Bad/missing checkpoint** → skip that contender with a logged warning, continue (a tournament tolerates a missing entry); never panic. No `.unwrap()` in production paths.
- **Parallelism + HAC**: a frozen HAC forward is read-only, but candle tensors aren't trivially `Sync` for mutation; structure the rayon parallelism so each pairing **loads/owns its two controllers** (HAC re-loaded per pairing from path, or contenders cloned), so no shared mutable net crosses threads. Verify determinism is independent of `RAYON_NUM_THREADS` (existing guarantee).
- **Reproducibility**: FNV-salted per-(pair,side,match) seeds + side-swap → byte-reproducible ladder for a fixed contender set.
- **Definition-of-done for any RUN**: ledger entry + checkpoint/result backup (standing discipline).

## 9. Testing (TDD)

Unit (fast, no sim):
- Bradley-Terry: a hand-built win-matrix with a known dominance order → ratings in that order; a symmetric (all-0.5) matrix → equal ratings; anchor pegs the reference exactly.
- Cycle detection: a synthetic cyclic matrix (A>B>C>A) → the cycle is reported; a transitive matrix → zero cycles.
- Seed/side-swap: `pair_salt(i,j,side)` distinct across sides; the scheduler's accumulation is symmetric (`W[i][j] + W[j][i] == 1` within fp tolerance on a synthetic runner).

Integration (smoke, eval-light):
- A 3-contender tournament (2 archetypes + 1 HAC) at `mpe=1` completes, writes `ladder.md`/`win_matrix.csv`/`ratings.json`, every pairing played, Elo finite + anchored. FAST locally; heavy full run on cnc.
- Guard: `play_match`/`play_match_h2h` (refactored onto `play_pair`) stay byte-identical — the existing `eval` tests must remain green.

## 10. Scope (YAGNI)

IN: 2-colony round-robin, heterogeneous `play_pair`, side-swap + seeding, Bradley-Terry→Elo, win-matrix + cycle report, reusable `tournament` bin, decisive metric.
OUT (later, separately): N-colony free-for-all (needs new sim constructor, N-way termination, multi-enemy observation, HAC re-tensoring); training against the ladder (population-based training); online/incremental rating updates; >2 simultaneous colonies of any kind.

## 11. Open questions for review

1. **Contender set for the first run** — curated ~14 (SOTA, v1, 3 self-play/league brains, 7 archetypes, gradclip, fullhorizon) vs `--glob` everything in `bench/`. Default: curated (cheaper, the meaningful set), glob available.
2. **Worker-share alongside decisive** — report both metrics' ladders, or decisive only? Default: decisive headline, worker-share as a secondary column (cheap, both already computed by `score_match`).
3. **First-run `mpe`** — 15 (faster, noisier) then a high-mpe top-N confirm, or 30 straight. Default: 15-then-confirm.

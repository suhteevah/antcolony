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

/// Configuration for a round-robin tournament.
pub struct TournamentConfig {
    /// (id, spec) pairs — one per contender.
    pub contenders: Vec<(String, String)>,
    /// Matches per ordered pair (i-left/j-right direction). Total per unordered
    /// pair = `2 * mpe` (both sides played).
    pub mpe: usize,
    /// Tick limit per match (passed through to `play_pair`).
    pub max_ticks: u64,
    /// Id of the contender whose Elo is pegged to `anchor_elo`. If not found,
    /// the mean Elo is anchored instead.
    pub anchor_id: String,
    /// Elo value to pin the anchor to (e.g. 1000.0).
    pub anchor_elo: f64,
    /// Minimum decisive-win margin for a 3-cycle to be reported.
    pub cycle_margin: f32,
    /// Neural-net sizing preset (used when building HAC contenders).
    pub sizing: Sizing,
}

impl TournamentConfig {
    /// Lightweight smoke default: 3 scripted archetypes, mpe=1, 1500 ticks.
    pub fn smoke() -> Self {
        TournamentConfig {
            contenders: vec![
                ("aggro".into(), "aggressor".into()),
                ("econ".into(), "economist".into()),
                ("def".into(), "defender".into()),
            ],
            mpe: 1,
            max_ticks: 1500,
            anchor_id: "econ".into(),
            anchor_elo: 1000.0,
            cycle_margin: 0.55,
            sizing: crate::hierarchical::sizing::A1,
        }
    }
}

/// Full result of a round-robin tournament.
pub struct TournamentResult {
    /// Contender ids in enrollment order.
    pub ids: Vec<String>,
    /// Contender specs in enrollment order.
    pub specs: Vec<String>,
    /// `win_matrix[i][j]` = i's mean decisive score vs j over `2*mpe` games;
    /// diagonal is `f32::NAN`.
    pub win_matrix: Vec<Vec<f32>>,
    /// `ws_matrix[i][j]` = i's mean worker-share score vs j; same layout.
    pub ws_matrix: Vec<Vec<f32>>,
    /// `games[i][j]` = total games played between i and j (symmetric).
    pub games: Vec<Vec<usize>>,
    /// Bradley-Terry Elo ratings in enrollment order.
    pub elo: Vec<f64>,
    /// Mean of finite entries in `win_matrix[i][*]` for each contender.
    pub winrate_vs_field: Vec<f32>,
    /// Detected 3-cycles `(i, j, k)` where i beats j beats k beats i,
    /// each edge by `> cycle_margin`.
    pub cycles: Vec<(usize, usize, usize)>,
}

/// Run a full round-robin tournament. Every unordered pair `(i, j)` with
/// `i < j` is played `cfg.mpe` times i-left/j-right and `cfg.mpe` times
/// j-left/i-right (rayon-parallel over pairings). Each pairing builds its
/// own two `Contender`s so no `Controller` crosses thread boundaries.
///
/// A pairing whose `build_contender` fails is logged and skipped; the
/// corresponding matrix cells stay `f32::NAN` and `games 0`, so the
/// tournament continues. No `.unwrap()` or early abort for a single bad pair.
pub fn run_tournament(cfg: &TournamentConfig, device: &Device) -> Result<TournamentResult> {
    use rayon::prelude::*;

    let n = cfg.contenders.len();
    let ids: Vec<String> = cfg.contenders.iter().map(|(id, _)| id.clone()).collect();
    let specs: Vec<String> = cfg.contenders.iter().map(|(_, sp)| sp.clone()).collect();

    // Build the N*(N-1)/2 unordered pairings.
    let mut pairings: Vec<(usize, usize)> = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            pairings.push((i, j));
        }
    }

    tracing::info!(
        contenders = n,
        pairings = pairings.len(),
        mpe = cfg.mpe,
        max_ticks = cfg.max_ticks,
        "tournament: starting round-robin"
    );

    // Per-pairing result: (i, j, dec_i_sum, ws_i_sum, played).
    // `dec_i_sum` is i's total decisive score across 2*mpe matches;
    // `ws_i_sum` is the same for worker-share. `played` is how many
    // individual matches actually completed without error.
    let pairing_results: Vec<(usize, usize, f32, f32, usize)> = pairings
        .par_iter()
        .map(|&(i, j)| {
            let id_i = &ids[i];
            let id_j = &ids[j];
            let spec_i = &specs[i];
            let spec_j = &specs[j];

            // Each pairing builds its own controllers (no shared &mut across threads).
            let ci_res = build_contender(id_i, spec_i, device, cfg.sizing);
            let cj_res = build_contender(id_j, spec_j, device, cfg.sizing);

            let (mut ci, mut cj) = match (ci_res, cj_res) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(e), _) => {
                    tracing::warn!(id = id_i, spec = spec_i, error = %e,
                        "tournament: build_contender failed for left; skipping pair ({i},{j})");
                    return (i, j, f32::NAN, f32::NAN, 0);
                }
                (_, Err(e)) => {
                    tracing::warn!(id = id_j, spec = spec_j, error = %e,
                        "tournament: build_contender failed for right; skipping pair ({i},{j})");
                    return (i, j, f32::NAN, f32::NAN, 0);
                }
            };

            let mut dec_i = 0.0f32;
            let mut ws_i = 0.0f32;
            let mut played = 0usize;

            // Half 1: i is LEFT, j is RIGHT. Seed salt "{id_i}:{id_j}:L".
            let salt_ij = crate::eval::spec_seed_salt(&format!("{}:{}:L", id_i, id_j));
            for m in 0..cfg.mpe {
                let seed = 0xE7A1_u64
                    .wrapping_mul(salt_ij)
                    ^ ((m as u64).wrapping_mul(0x9E3779B97F4A7C15));
                match crate::eval::play_pair(
                    &mut ci.controller,
                    &mut cj.controller,
                    device,
                    seed,
                    cfg.max_ticks,
                ) {
                    Ok((ws, dec, _end)) => {
                        // i's scores come directly from play_pair (i is left).
                        dec_i += dec;
                        ws_i += ws;
                        played += 1;
                    }
                    Err(e) => tracing::warn!(
                        i, j, m, error = %e,
                        "tournament: play_pair (i-left) failed; skipping match"
                    ),
                }
            }

            // Half 2: j is LEFT, i is RIGHT. Seed salt "{id_j}:{id_i}:L".
            // play_pair returns LEFT's score; i is right, so i's score = 1 - left_score.
            let salt_ji = crate::eval::spec_seed_salt(&format!("{}:{}:L", id_j, id_i));
            for m in 0..cfg.mpe {
                let seed = 0xE7A1_u64
                    .wrapping_mul(salt_ji)
                    ^ ((m as u64).wrapping_mul(0x9E3779B97F4A7C15));
                match crate::eval::play_pair(
                    &mut cj.controller,
                    &mut ci.controller,
                    device,
                    seed,
                    cfg.max_ticks,
                ) {
                    Ok((ws, dec, _end)) => {
                        // i is right; flip left's score to get i's score.
                        dec_i += 1.0 - dec;
                        ws_i += 1.0 - ws;
                        played += 1;
                    }
                    Err(e) => tracing::warn!(
                        i, j, m, error = %e,
                        "tournament: play_pair (j-left) failed; skipping match"
                    ),
                }
            }

            let total = (cfg.mpe * 2) as f32;
            let mean_dec = if played > 0 { dec_i / total } else { f32::NAN };
            let mean_ws = if played > 0 { ws_i / total } else { f32::NAN };

            tracing::debug!(
                i, id = id_i, j, id_j = id_j,
                mean_decisive = mean_dec, mean_ws = mean_ws, played,
                "tournament: pairing done"
            );

            (i, j, mean_dec, mean_ws, played)
        })
        .collect();

    // Assemble matrices single-threaded.
    let nan_row = vec![f32::NAN; n];
    let mut win_matrix: Vec<Vec<f32>> = vec![nan_row.clone(); n];
    let mut ws_matrix: Vec<Vec<f32>> = vec![nan_row; n];
    let mut games: Vec<Vec<usize>> = vec![vec![0usize; n]; n];

    for (i, j, mean_dec, mean_ws, played) in pairing_results {
        // diagonal stays NAN; only fill off-diagonal.
        win_matrix[i][j] = mean_dec;
        win_matrix[j][i] = if mean_dec.is_finite() { 1.0 - mean_dec } else { f32::NAN };
        ws_matrix[i][j] = mean_ws;
        ws_matrix[j][i] = if mean_ws.is_finite() { 1.0 - mean_ws } else { f32::NAN };
        games[i][j] = played;
        games[j][i] = played;
    }

    // Diagonal: NAN (by construction from nan_row; games diagonal stays 0).

    // Bradley-Terry Elo — find anchor index.
    let anchor_idx = ids.iter().position(|id| id == &cfg.anchor_id);
    let elo = bradley_terry_elo(&win_matrix, &games, anchor_idx, cfg.anchor_elo);

    // winrate_vs_field[i] = mean of finite win_matrix[i][j] for j != i.
    let winrate_vs_field: Vec<f32> = (0..n)
        .map(|i| {
            let (sum, cnt) = (0..n)
                .filter(|&j| j != i)
                .filter_map(|j| {
                    let v = win_matrix[i][j];
                    if v.is_finite() { Some(v) } else { None }
                })
                .fold((0.0f32, 0usize), |(s, c), v| (s + v, c + 1));
            if cnt > 0 { sum / cnt as f32 } else { f32::NAN }
        })
        .collect();

    let cycles = find_cycles(&win_matrix, cfg.cycle_margin);

    tracing::info!(
        contenders = n,
        ?cycles,
        "tournament: complete"
    );

    Ok(TournamentResult {
        ids,
        specs,
        win_matrix,
        ws_matrix,
        games,
        elo,
        winrate_vs_field,
        cycles,
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;
    use crate::hierarchical::sizing::A1;

    #[test]
    fn run_tournament_scripted_smoke() {
        let dev = Device::Cpu;
        let cfg = TournamentConfig {
            contenders: vec![
                ("aggro".into(), "aggressor".into()),
                ("econ".into(), "economist".into()),
                ("def".into(), "defender".into()),
            ],
            mpe: 1,
            max_ticks: 1500,
            anchor_id: "econ".into(),
            anchor_elo: 1000.0,
            cycle_margin: 0.55,
            sizing: crate::hierarchical::sizing::A1,
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
}

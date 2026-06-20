//! SP1 self-play league: the opponent pool (the 7 fixed archetypes, always
//! present, + a capped FIFO of frozen HAC self-snapshots) and the opponent
//! sampler. Pure logic — no candle, no sim — so it is fully unit-testable.

use std::path::PathBuf;

use rand::Rng;

use crate::eval::BENCH_ARCHETYPES;

#[derive(Clone, Debug)]
pub enum OpponentKind {
    Archetype(String),
    Snapshot { name: String, path: PathBuf },
}

#[derive(Clone, Debug)]
pub struct PoolEntry {
    pub kind: OpponentKind,
    pub win_rate_ema: f32,
    pub games: u32,
}

#[derive(Clone, Debug)]
pub struct SnapshotPool {
    pub entries: Vec<PoolEntry>,
    pub pool_cap: usize,
    pub ema_alpha: f32,
}

impl SnapshotPool {
    pub fn with_archetypes(pool_cap: usize, ema_alpha: f32) -> Self {
        let entries = BENCH_ARCHETYPES
            .iter()
            .map(|a| PoolEntry {
                kind: OpponentKind::Archetype((*a).to_string()),
                win_rate_ema: 0.5,
                games: 0,
            })
            .collect();
        Self { entries, pool_cap, ema_alpha }
    }

    pub fn snapshot_count(&self) -> usize {
        self.entries.iter().filter(|e| matches!(e.kind, OpponentKind::Snapshot { .. })).count()
    }

    pub fn add_snapshot(&mut self, name: impl Into<String>, path: impl Into<PathBuf>) {
        self.entries.push(PoolEntry {
            kind: OpponentKind::Snapshot { name: name.into(), path: path.into() },
            win_rate_ema: 0.5,
            games: 0,
        });
        while self.snapshot_count() > self.pool_cap {
            // evict the oldest snapshot = lowest-index Snapshot entry
            if let Some(pos) = self.entries.iter().position(|e| matches!(e.kind, OpponentKind::Snapshot { .. })) {
                self.entries.remove(pos);
            } else {
                break;
            }
        }
    }

    pub fn record_result(&mut self, idx: usize, hac_won: f32) {
        if let Some(e) = self.entries.get_mut(idx) {
            e.win_rate_ema = (1.0 - self.ema_alpha) * e.win_rate_ema + self.ema_alpha * hac_won;
            e.games = e.games.saturating_add(1);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum OpponentSampler {
    Uniform,
    Pfsp { archetype_mix: f32, power: f32 },
}

impl OpponentSampler {
    pub fn sample(&self, pool: &SnapshotPool, rng: &mut rand_chacha::ChaCha8Rng) -> usize {
        match self {
            OpponentSampler::Uniform => rng.gen_range(0..pool.entries.len().max(1)),
            OpponentSampler::Pfsp { archetype_mix, power } => {
                let arche: Vec<usize> = pool.entries.iter().enumerate()
                    .filter(|(_, e)| matches!(e.kind, OpponentKind::Archetype(_)))
                    .map(|(i, _)| i).collect();
                let snaps: Vec<usize> = pool.entries.iter().enumerate()
                    .filter(|(_, e)| matches!(e.kind, OpponentKind::Snapshot { .. }))
                    .map(|(i, _)| i).collect();
                let use_archetype = snaps.is_empty() || rng.r#gen::<f32>() < *archetype_mix;
                if use_archetype && !arche.is_empty() {
                    arche[rng.gen_range(0..arche.len())]
                } else if !snaps.is_empty() {
                    let weights: Vec<f32> = snaps.iter()
                        .map(|&i| (1.0 - pool.entries[i].win_rate_ema).max(1e-3).powf(*power))
                        .collect();
                    let total: f32 = weights.iter().sum();
                    let mut r = rng.r#gen::<f32>() * total;
                    for (k, &w) in weights.iter().enumerate() {
                        r -= w;
                        if r <= 0.0 { return snaps[k]; }
                    }
                    *snaps.last().unwrap()
                } else {
                    0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_archetypes_seeds_seven() {
        let p = SnapshotPool::with_archetypes(8, 0.1);
        assert_eq!(p.entries.len(), 7);
        assert!(p.entries.iter().all(|e| matches!(e.kind, OpponentKind::Archetype(_))));
        assert_eq!(p.snapshot_count(), 0);
        assert!(p.entries.iter().all(|e| (e.win_rate_ema - 0.5).abs() < 1e-6));
    }

    #[test]
    fn add_snapshot_evicts_oldest_keeps_archetypes() {
        let mut p = SnapshotPool::with_archetypes(2, 0.1);
        p.add_snapshot("s0", "a/0.safetensors");
        p.add_snapshot("s1", "a/1.safetensors");
        p.add_snapshot("s2", "a/2.safetensors"); // cap=2 -> evict s0
        assert_eq!(p.snapshot_count(), 2);
        // 7 archetypes still present
        assert_eq!(p.entries.iter().filter(|e| matches!(e.kind, OpponentKind::Archetype(_))).count(), 7);
        let names: Vec<&str> = p.entries.iter().filter_map(|e| match &e.kind {
            OpponentKind::Snapshot { name, .. } => Some(name.as_str()), _ => None }).collect();
        assert_eq!(names, vec!["s1", "s2"], "oldest snapshot s0 evicted");
    }

    #[test]
    fn record_result_updates_ema() {
        let mut p = SnapshotPool::with_archetypes(8, 0.5);
        p.record_result(0, 1.0); // 0.5 -> 0.75
        assert!((p.entries[0].win_rate_ema - 0.75).abs() < 1e-6);
        assert_eq!(p.entries[0].games, 1);
        p.record_result(999, 1.0); // out of range = no-op (no panic)
    }

    #[test]
    fn uniform_sampler_covers_all_entries() {
        use rand::SeedableRng;
        let mut p = SnapshotPool::with_archetypes(8, 0.1);
        p.add_snapshot("s0", "a/0.safetensors");
        let s = OpponentSampler::Uniform;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..500 { seen.insert(s.sample(&p, &mut rng)); }
        assert_eq!(seen.len(), p.entries.len());
    }

    #[test]
    fn pfsp_favors_losing_matchups_and_honors_mix() {
        use rand::SeedableRng;
        let mut p = SnapshotPool::with_archetypes(8, 0.1);
        p.add_snapshot("strong", "a/strong.safetensors"); // HAC loses to it
        p.add_snapshot("weak", "a/weak.safetensors");     // HAC beats it
        let strong_idx = p.entries.len() - 2;
        let weak_idx = p.entries.len() - 1;
        p.entries[strong_idx].win_rate_ema = 0.1; // HAC mostly loses -> high priority
        p.entries[weak_idx].win_rate_ema = 0.9;   // HAC mostly wins  -> low priority
        let s = OpponentSampler::Pfsp { archetype_mix: 0.0, power: 1.0 }; // snapshots only
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
        let (mut strong_n, mut weak_n) = (0u32, 0u32);
        for _ in 0..2000 {
            match s.sample(&p, &mut rng) {
                i if i == strong_idx => strong_n += 1,
                i if i == weak_idx => weak_n += 1,
                _ => {}
            }
        }
        assert!(strong_n > weak_n * 3, "PFSP must oversample the matchup we lose: strong={strong_n} weak={weak_n}");
    }

    #[test]
    fn pfsp_empty_pool_returns_archetype() {
        use rand::SeedableRng;
        let p = SnapshotPool::with_archetypes(8, 0.1); // no snapshots
        let s = OpponentSampler::Pfsp { archetype_mix: 0.5, power: 1.0 };
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(3);
        for _ in 0..50 {
            let i = s.sample(&p, &mut rng);
            assert!(matches!(p.entries[i].kind, OpponentKind::Archetype(_)));
        }
    }
}

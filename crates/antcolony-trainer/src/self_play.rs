//! SP1 self-play league: the opponent pool (the 7 fixed archetypes, always
//! present, + a capped FIFO of frozen HAC self-snapshots) and the opponent
//! sampler. Pure logic — no candle, no sim — so it is fully unit-testable.

use std::path::PathBuf;

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
}

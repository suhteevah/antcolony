//! Cell-bucketed spatial hash for O(1) radius queries.

use glam::Vec2;
use std::collections::HashMap;

pub type EntityId = u32;

#[derive(Debug, Clone)]
pub struct SpatialHash {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<EntityId>>,
}

impl SpatialHash {
    pub fn new(cell_size: f32) -> Self {
        assert!(cell_size > 0.0, "cell_size must be > 0");
        Self {
            cell_size,
            cells: HashMap::new(),
        }
    }

    #[inline]
    fn bucket(&self, p: Vec2) -> (i32, i32) {
        (
            (p.x / self.cell_size).floor() as i32,
            (p.y / self.cell_size).floor() as i32,
        )
    }

    pub fn clear(&mut self) {
        for v in self.cells.values_mut() {
            v.clear();
        }
    }

    pub fn insert(&mut self, id: EntityId, pos: Vec2) {
        let b = self.bucket(pos);
        self.cells.entry(b).or_default().push(id);
    }

    /// All entities within `radius` of `pos`. Caller filters exact distance if needed.
    pub fn query_radius(&self, pos: Vec2, radius: f32) -> Vec<EntityId> {
        let min = self.bucket(pos - Vec2::splat(radius));
        let max = self.bucket(pos + Vec2::splat(radius));
        let mut out = Vec::new();
        for cy in min.1..=max.1 {
            for cx in min.0..=max.0 {
                if let Some(v) = self.cells.get(&(cx, cy)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rand::Rng;

    #[test]
    fn test_spatial_hash() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut hash = SpatialHash::new(4.0);
        let mut positions = Vec::new();
        for i in 0..1000 {
            let p = Vec2::new(rng.gen_range(0.0..100.0), rng.gen_range(0.0..100.0));
            positions.push(p);
            hash.insert(i, p);
        }
        let q = Vec2::new(50.0, 50.0);
        let r = 5.0;
        let candidates = hash.query_radius(q, r);
        let brute: Vec<u32> = positions
            .iter()
            .enumerate()
            .filter(|(_, p)| p.distance(q) <= r)
            .map(|(i, _)| i as u32)
            .collect();
        // Every brute-force match MUST appear in the candidate set (hash may return extras).
        for b in &brute {
            assert!(candidates.contains(b), "missed {}", b);
        }
    }
}

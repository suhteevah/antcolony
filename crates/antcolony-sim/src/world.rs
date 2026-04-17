//! World terrain grid: empty, food, obstacle, nest entrance.

use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Terrain {
    Empty,
    Food(u32),
    Obstacle,
    NestEntrance(u8),
}

#[derive(Debug, Clone)]
pub struct WorldGrid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Terrain>,
}

impl WorldGrid {
    pub fn new(width: usize, height: usize) -> Self {
        tracing::debug!(width, height, "WorldGrid::new");
        Self {
            width,
            height,
            cells: vec![Terrain::Empty; width * height],
        }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    #[inline]
    pub fn in_bounds(&self, x: i64, y: i64) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }

    pub fn get(&self, x: usize, y: usize) -> Terrain {
        self.cells[self.idx(x, y)]
    }

    pub fn set(&mut self, x: usize, y: usize, t: Terrain) {
        let i = self.idx(x, y);
        self.cells[i] = t;
    }

    pub fn world_to_grid(&self, pos: Vec2) -> (i64, i64) {
        (pos.x.floor() as i64, pos.y.floor() as i64)
    }

    pub fn grid_to_world(&self, x: usize, y: usize) -> Vec2 {
        Vec2::new(x as f32 + 0.5, y as f32 + 0.5)
    }

    /// Place a food cluster — returns number of cells turned into food.
    pub fn place_food_cluster(&mut self, cx: i64, cy: i64, radius: i64, units_per_cell: u32) -> u32 {
        let mut placed = 0u32;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                let x = cx + dx;
                let y = cy + dy;
                if self.in_bounds(x, y) {
                    let (ux, uy) = (x as usize, y as usize);
                    if self.get(ux, uy) == Terrain::Empty {
                        self.set(ux, uy, Terrain::Food(units_per_cell));
                        placed += 1;
                    }
                }
            }
        }
        tracing::debug!(cx, cy, radius, placed, "place_food_cluster");
        placed
    }

    pub fn place_nest(&mut self, x: usize, y: usize, colony_id: u8) {
        self.set(x, y, Terrain::NestEntrance(colony_id));
        tracing::info!(x, y, colony_id, "nest entrance placed");
    }

    /// Decrement food at cell, returns amount picked up (0 or 1).
    pub fn take_food(&mut self, x: usize, y: usize) -> u32 {
        let i = self.idx(x, y);
        match self.cells[i] {
            Terrain::Food(n) if n > 0 => {
                let new = n - 1;
                self.cells[i] = if new == 0 { Terrain::Empty } else { Terrain::Food(new) };
                1
            }
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn food_cluster_and_take() {
        let mut w = WorldGrid::new(20, 20);
        let placed = w.place_food_cluster(10, 10, 2, 3);
        assert!(placed > 0);
        let got = w.take_food(10, 10);
        assert_eq!(got, 1);
    }

    #[test]
    fn bounds() {
        let w = WorldGrid::new(10, 10);
        assert!(w.in_bounds(0, 0));
        assert!(w.in_bounds(9, 9));
        assert!(!w.in_bounds(-1, 0));
        assert!(!w.in_bounds(10, 5));
    }
}

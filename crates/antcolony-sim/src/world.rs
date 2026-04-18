//! World terrain grid: empty, food, obstacle, nest entrance.

use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Phase 5 chamber specialization — labels rooms carved out of an
/// `UndergroundNest`. Chambers are walkable (like `Empty`) but have
/// distinct behavior (queens only lay in `BroodNursery`, returning ants
/// dump food in `FoodStorage`) and render tints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChamberType {
    QueenChamber,
    BroodNursery,
    FoodStorage,
    Waste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Terrain {
    Empty,
    Food(u32),
    Obstacle,
    NestEntrance(u8),
    /// Phase 5: unexcavated earth. Diggable — an adjacent ant in state
    /// `Digging` will convert it to `Empty` over time.
    Solid,
    /// Phase 5: a specialized room inside an underground nest.
    Chamber(ChamberType),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Phase 5: fill every cell with `Solid` (unexcavated earth). Used
    /// when an `UndergroundNest` module is first built.
    pub fn fill_solid(&mut self) {
        for c in self.cells.iter_mut() {
            *c = Terrain::Solid;
        }
    }

    /// Phase 5: carve an axis-aligned rectangular room, overwriting any
    /// `Solid` (and any other non-nest-entrance terrain) with the given
    /// chamber specialization. `NestEntrance` cells are preserved.
    pub fn carve_chamber(
        &mut self,
        cx: usize,
        cy: usize,
        half_w: usize,
        half_h: usize,
        kind: ChamberType,
    ) -> u32 {
        let mut placed = 0u32;
        let x0 = cx.saturating_sub(half_w);
        let y0 = cy.saturating_sub(half_h);
        let x1 = (cx + half_w).min(self.width.saturating_sub(1));
        let y1 = (cy + half_h).min(self.height.saturating_sub(1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let i = self.idx(x, y);
                if !matches!(self.cells[i], Terrain::NestEntrance(_)) {
                    self.cells[i] = Terrain::Chamber(kind);
                    placed += 1;
                }
            }
        }
        tracing::info!(cx, cy, half_w, half_h, ?kind, placed, "carve_chamber");
        placed
    }

    /// Phase 5: carve a straight tunnel between two cells, overwriting
    /// any `Solid`. Uses Bresenham-ish stepping — good enough for the
    /// pre-carved starter nest layout.
    pub fn carve_tunnel(&mut self, (x0, y0): (usize, usize), (x1, y1): (usize, usize)) -> u32 {
        let mut placed = 0u32;
        let dx = x1 as i64 - x0 as i64;
        let dy = y1 as i64 - y0 as i64;
        let steps = dx.abs().max(dy.abs()).max(1) as usize;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let x = (x0 as f32 + t * dx as f32).round() as i64;
            let y = (y0 as f32 + t * dy as f32).round() as i64;
            if self.in_bounds(x, y) {
                let i = self.idx(x as usize, y as usize);
                if !matches!(self.cells[i], Terrain::NestEntrance(_) | Terrain::Chamber(_)) {
                    self.cells[i] = Terrain::Empty;
                    placed += 1;
                }
            }
        }
        placed
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

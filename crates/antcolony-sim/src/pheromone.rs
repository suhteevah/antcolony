//! Dense pheromone grid with evaporation + diffusion.
//!
//! Stores 4 layers as flat `Vec<f32>`s, row-major (idx = y*width + x).
//! Diffusion uses a scratch buffer for the 5-point Laplacian stencil.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use wide::{f32x8, CmpGt, CmpLt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PheromoneLayer {
    FoodTrail,
    HomeTrail,
    Alarm,
    ColonyScent,
}

fn default_scratch() -> Vec<f32> {
    Vec::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PheromoneGrid {
    pub width: usize,
    pub height: usize,
    pub food_trail: Vec<f32>,
    pub home_trail: Vec<f32>,
    pub alarm: Vec<f32>,
    pub colony_scent: Vec<f32>,
    #[serde(skip, default = "default_scratch")]
    scratch: Vec<f32>,
}

impl PheromoneGrid {
    /// Rebuild the diffusion scratch buffer after deserialization.
    pub fn rebuild_scratch(&mut self) {
        let n = self.width * self.height;
        if self.scratch.len() != n {
            self.scratch = vec![0.0; n];
        }
    }
}

impl PheromoneGrid {
    pub fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        tracing::debug!(width, height, cells = n, "PheromoneGrid::new");
        Self {
            width,
            height,
            food_trail: vec![0.0; n],
            home_trail: vec![0.0; n],
            alarm: vec![0.0; n],
            colony_scent: vec![0.0; n],
            scratch: vec![0.0; n],
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

    #[inline]
    fn layer_slice(&self, layer: PheromoneLayer) -> &[f32] {
        match layer {
            PheromoneLayer::FoodTrail => &self.food_trail,
            PheromoneLayer::HomeTrail => &self.home_trail,
            PheromoneLayer::Alarm => &self.alarm,
            PheromoneLayer::ColonyScent => &self.colony_scent,
        }
    }

    #[inline]
    fn layer_slice_mut(&mut self, layer: PheromoneLayer) -> &mut [f32] {
        match layer {
            PheromoneLayer::FoodTrail => &mut self.food_trail,
            PheromoneLayer::HomeTrail => &mut self.home_trail,
            PheromoneLayer::Alarm => &mut self.alarm,
            PheromoneLayer::ColonyScent => &mut self.colony_scent,
        }
    }

    pub fn read(&self, x: usize, y: usize, layer: PheromoneLayer) -> f32 {
        self.layer_slice(layer)[self.idx(x, y)]
    }

    pub fn deposit(&mut self, x: usize, y: usize, layer: PheromoneLayer, amount: f32, cap: f32) {
        let i = self.idx(x, y);
        let slice = self.layer_slice_mut(layer);
        let v = (slice[i] + amount).min(cap);
        slice[i] = v;
    }

    /// Direct overwrite of a cell. Used by port-scent bleed, which needs
    /// to set absolute values rather than accumulate.
    pub fn set_cell(&mut self, x: usize, y: usize, layer: PheromoneLayer, value: f32) {
        let i = self.idx(x, y);
        self.layer_slice_mut(layer)[i] = value;
    }

    pub fn world_to_grid(&self, pos: Vec2) -> (i64, i64) {
        (pos.x.floor() as i64, pos.y.floor() as i64)
    }

    pub fn grid_to_world(&self, x: usize, y: usize) -> Vec2 {
        Vec2::new(x as f32 + 0.5, y as f32 + 0.5)
    }

    /// Exponential decay per tick, clamp near-zero to zero for sparsity.
    ///
    /// Uses `v.abs() < threshold` so it works correctly for the signed
    /// `colony_scent` layer (P4 territory): negative red-colony territory
    /// values still decay toward zero rather than being clipped on the
    /// wrong side.
    pub fn evaporate(&mut self, rate: f32, threshold: f32) {
        let k = 1.0 - rate;
        for slice in [
            &mut self.food_trail,
            &mut self.home_trail,
            &mut self.alarm,
            &mut self.colony_scent,
        ] {
            evaporate_slice_simd(slice, k, threshold);
        }
    }

    /// P4 territory: colony-signed deposit into `colony_scent`. Colony 0
    /// adds positive, colony 1 adds negative — so a single f32 per cell
    /// resolves to "which colony dominates this tile" by sign.
    /// Amplitude is clamped to ±`cap`.
    pub fn deposit_territory(&mut self, x: usize, y: usize, colony_id: u8, amount: f32, cap: f32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = self.idx(x, y);
        let signed = if colony_id == 0 { amount } else { -amount };
        let v = (self.colony_scent[i] + signed).clamp(-cap, cap);
        self.colony_scent[i] = v;
    }

    /// 5-point Laplacian diffusion, double-buffered. Applied to every layer.
    pub fn diffuse(&mut self, rate: f32) {
        let w = self.width;
        let h = self.height;
        for layer in [
            PheromoneLayer::FoodTrail,
            PheromoneLayer::HomeTrail,
            PheromoneLayer::Alarm,
            PheromoneLayer::ColonyScent,
        ] {
            let src: Vec<f32> = self.layer_slice(layer).to_vec();
            self.scratch.copy_from_slice(&src);
            let dst = self.layer_slice_mut(layer);
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    let c = src[i];
                    let up = if y > 0 { src[i - w] } else { c };
                    let dn = if y + 1 < h { src[i + w] } else { c };
                    let lf = if x > 0 { src[i - 1] } else { c };
                    let rt = if x + 1 < w { src[i + 1] } else { c };
                    dst[i] = c * (1.0 - 4.0 * rate) + rate * (up + dn + lf + rt);
                }
            }
        }
    }

    /// Sample pheromone in a forward cone — returns `(world_pos, intensity)` for each cell inside.
    /// Used by ants for direction selection.
    pub fn sample_cone(
        &self,
        pos: Vec2,
        heading: f32,
        half_angle_rad: f32,
        radius: f32,
        layer: PheromoneLayer,
    ) -> Vec<(Vec2, f32)> {
        let slice = self.layer_slice(layer);
        let (cx, cy) = self.world_to_grid(pos);
        let r = radius.ceil() as i64;
        let cos_half = half_angle_rad.cos();
        let fwd = Vec2::new(heading.cos(), heading.sin());
        let r2 = radius * radius;
        let mut out = Vec::with_capacity(16);
        for dy in -r..=r {
            for dx in -r..=r {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let gx = cx + dx;
                let gy = cy + dy;
                if !self.in_bounds(gx, gy) {
                    continue;
                }
                let cell_world = Vec2::new(gx as f32 + 0.5, gy as f32 + 0.5);
                let delta = cell_world - pos;
                let d2 = delta.length_squared();
                if d2 > r2 || d2 < 1e-6 {
                    continue;
                }
                let dir = delta / d2.sqrt();
                let cos_a = dir.dot(fwd);
                if cos_a < cos_half {
                    continue;
                }
                let i = (gy as usize) * self.width + (gx as usize);
                let v = slice[i];
                out.push((cell_world, v));
            }
        }
        out
    }

    pub fn total_intensity(&self, layer: PheromoneLayer) -> f32 {
        self.layer_slice(layer).iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pheromone_evaporation() {
        let mut g = PheromoneGrid::new(8, 8);
        g.deposit(4, 4, PheromoneLayer::FoodTrail, 5.0, 10.0);
        assert!((g.read(4, 4, PheromoneLayer::FoodTrail) - 5.0).abs() < 1e-6);
        for _ in 0..10 {
            g.evaporate(0.1, 0.001);
        }
        let v = g.read(4, 4, PheromoneLayer::FoodTrail);
        // After 10 ticks of 10% decay: 5.0 * 0.9^10 ≈ 1.743
        assert!(v > 1.5 && v < 2.0, "got {}", v);
    }

    #[test]
    fn evaporation_zeroes_tiny_values() {
        let mut g = PheromoneGrid::new(4, 4);
        g.deposit(2, 2, PheromoneLayer::FoodTrail, 0.002, 10.0);
        g.evaporate(0.5, 0.01);
        assert_eq!(g.read(2, 2, PheromoneLayer::FoodTrail), 0.0);
    }

    #[test]
    fn test_pheromone_diffusion() {
        let mut g = PheromoneGrid::new(9, 9);
        g.deposit(4, 4, PheromoneLayer::FoodTrail, 10.0, 20.0);
        let center_before = g.read(4, 4, PheromoneLayer::FoodTrail);
        let neighbor_before = g.read(5, 4, PheromoneLayer::FoodTrail);
        g.diffuse(0.2);
        let center_after = g.read(4, 4, PheromoneLayer::FoodTrail);
        let neighbor_after = g.read(5, 4, PheromoneLayer::FoodTrail);
        assert!(center_after < center_before);
        assert!(neighbor_after > neighbor_before);
    }

    #[test]
    fn cone_samples_forward_only() {
        let mut g = PheromoneGrid::new(20, 20);
        // Deposit in front and behind an ant at (10, 10) heading east (0 rad).
        g.deposit(13, 10, PheromoneLayer::FoodTrail, 5.0, 10.0);
        g.deposit(7, 10, PheromoneLayer::FoodTrail, 5.0, 10.0);
        let samples = g.sample_cone(
            Vec2::new(10.5, 10.5),
            0.0,
            60f32.to_radians(),
            5.0,
            PheromoneLayer::FoodTrail,
        );
        let total: f32 = samples.iter().map(|(_, v)| v).sum();
        // Only the front cell should register.
        assert!(total > 4.0 && total < 6.0, "total={}", total);
    }
}

/// SIMD-accelerated evaporation pass for one layer.
///
/// Multiplies every cell by `k = 1.0 - rate` and zeroes any cell whose
/// absolute value is below `threshold`. Processes 8 cells per CPU
/// instruction via `wide::f32x8`. Falls back to a scalar tail loop
/// for the trailing cells when len isn't a multiple of 8.
///
/// Threshold is checked against `abs(v) < threshold` so the signed
/// `colony_scent` (territory) layer decays correctly toward zero from
/// either side.
fn evaporate_slice_simd(slice: &mut [f32], k: f32, threshold: f32) {
    let k_v = f32x8::splat(k);
    let thresh_v = f32x8::splat(threshold);
    let neg_thresh_v = f32x8::splat(-threshold);
    let zero_v = f32x8::splat(0.0);

    let len = slice.len();
    let remainder_len = len % 8;

    for chunk in slice.chunks_exact_mut(8) {
        let mut v = f32x8::new([
            chunk[0], chunk[1], chunk[2], chunk[3],
            chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
        // Multiply by k.
        v = v * k_v;
        // Zero out any element whose absolute value is below threshold.
        // wide::f32x8 has no built-in abs; emulate via two cmp masks
        // (v < threshold AND v > -threshold means abs(v) < threshold).
        let below_pos = v.cmp_lt(thresh_v);
        let above_neg = v.cmp_gt(neg_thresh_v);
        let in_band = below_pos & above_neg;
        v = in_band.blend(zero_v, v);
        let arr = v.to_array();
        chunk[0] = arr[0]; chunk[1] = arr[1]; chunk[2] = arr[2]; chunk[3] = arr[3];
        chunk[4] = arr[4]; chunk[5] = arr[5]; chunk[6] = arr[6]; chunk[7] = arr[7];
    }

    // Scalar tail for the remainder cells.
    if remainder_len > 0 {
        let start = len - remainder_len;
        for v in &mut slice[start..] {
            *v *= k;
            if v.abs() < threshold {
                *v = 0.0;
            }
        }
    }
}

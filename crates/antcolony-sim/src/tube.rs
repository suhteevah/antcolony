//! Tubes connect modules at their ports. Ants in transit occupy a `TubeTransit`
//! state: they're not on any grid, they're advancing down the tube.

use serde::{Deserialize, Serialize};

use crate::module::{ModuleId, PortPos};

pub type TubeId = u16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TubeEnd {
    pub module: ModuleId,
    pub port: PortPos,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tube {
    pub id: TubeId,
    pub from: TubeEnd,
    pub to: TubeEnd,
    /// Ticks to traverse end-to-end at worker base speed.
    pub length_ticks: u32,
    /// Internal diameter in mm. Ants with `size_mm > bore_width_mm` can't
    /// fit (used for caste-gated routing: majors refused by narrow tubes).
    pub bore_width_mm: f32,
    /// 1D pheromone substrate inside the tube. Each cell corresponds to
    /// one tick of transit progress (so a tube of length 30 ticks has
    /// 30 cells). Foragers in `TubeTransit` deposit pheromone on the
    /// cell matching their `progress * length_ticks` position. Cells
    /// evaporate at the same rate as module cells. This replaces the
    /// `port_bleed` hack with biologically correct trail-on-tube-walls
    /// behavior. Layout: row-major flat: `[layer * length_ticks + idx]`
    /// where layer ∈ {FoodTrail=0, HomeTrail=1, Alarm=2, ColonyScent=3}.
    /// Initialized empty by `new_with_pheromones` and rebuilt on load
    /// via `rebuild_pheromones` for old snapshots that lack this field.
    #[serde(default)]
    pub pheromones: Vec<f32>,
}

impl Tube {
    pub fn new(
        id: TubeId,
        from: TubeEnd,
        to: TubeEnd,
        length_ticks: u32,
        bore_width_mm: f32,
    ) -> Self {
        let cells = length_ticks.max(1) as usize * 4; // 4 layers
        Self {
            id,
            from,
            to,
            length_ticks,
            bore_width_mm,
            pheromones: vec![0.0; cells],
        }
    }

    /// Convert a transit progress (0.0..1.0) to a tube pheromone cell index.
    pub fn cell_index(&self, progress: f32) -> usize {
        let len = self.length_ticks.max(1) as usize;
        let idx = (progress * len as f32).floor() as usize;
        idx.min(len - 1)
    }

    /// Number of pheromone cells per layer (= length_ticks).
    pub fn cells_per_layer(&self) -> usize {
        self.length_ticks.max(1) as usize
    }

    /// Rebuild the pheromone substrate from scratch. Called on snapshot
    /// load when `pheromones` is empty (older save format).
    pub fn rebuild_pheromones(&mut self) {
        let cells = self.length_ticks.max(1) as usize * 4;
        if self.pheromones.len() != cells {
            self.pheromones = vec![0.0; cells];
        }
    }

    /// Deposit pheromone at a transit cell index, capped at `max`.
    pub fn deposit(&mut self, idx: usize, layer: usize, amount: f32, max: f32) {
        let len = self.cells_per_layer();
        if idx >= len || layer >= 4 {
            return;
        }
        let i = layer * len + idx;
        self.pheromones[i] = (self.pheromones[i] + amount).min(max);
    }

    /// Read pheromone at a transit cell index.
    pub fn read(&self, idx: usize, layer: usize) -> f32 {
        let len = self.cells_per_layer();
        if idx >= len || layer >= 4 {
            return 0.0;
        }
        let i = layer * len + idx;
        self.pheromones.get(i).copied().unwrap_or(0.0)
    }

    /// Per-substep evaporation across all 4 layers.
    pub fn evaporate(&mut self, rate: f32, threshold: f32) {
        let k = 1.0 - rate;
        for v in self.pheromones.iter_mut() {
            *v *= k;
            if v.abs() < threshold {
                *v = 0.0;
            }
        }
    }
}

/// Per-ant state while inside a tube.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TubeTransit {
    pub tube: TubeId,
    /// 0.0 = at `from` end, 1.0 = at `to` end.
    pub progress: f32,
    /// true: moving `from`→`to`. false: `to`→`from`.
    pub going_forward: bool,
}

impl TubeTransit {
    pub fn new(tube: TubeId, going_forward: bool) -> Self {
        Self {
            tube,
            // Start slightly past the entry so the ant doesn't immediately re-enter.
            progress: if going_forward { 0.02 } else { 0.98 },
            going_forward,
        }
    }
}

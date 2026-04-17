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

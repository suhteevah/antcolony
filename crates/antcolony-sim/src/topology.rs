//! A `Topology` is the formicarium wiring diagram — the list of modules
//! and the tubes connecting them. The `Simulation` owns a `Topology` and
//! every per-tick system iterates it.
//!
//! Pre-K2 code assumed one world + one pheromone grid. Backward
//! compatibility is preserved via `Topology::single` — a one-module
//! topology that looks and behaves exactly like the old single-grid sim.

use glam::Vec2;

use crate::module::{Module, ModuleId, ModuleKind, PortPos};
use crate::tube::{Tube, TubeEnd, TubeId};

#[derive(Debug, Clone)]
pub struct Topology {
    pub modules: Vec<Module>,
    pub tubes: Vec<Tube>,
}

impl Topology {
    pub fn empty() -> Self {
        Self {
            modules: vec![],
            tubes: vec![],
        }
    }

    /// Single-module topology — preserves pre-K2 behavior when an older
    /// test constructs a `Simulation` without specifying a topology.
    pub fn single(kind: ModuleKind, width: usize, height: usize) -> Self {
        let m = Module::new(0, kind, width, height, Vec2::ZERO, kind.label());
        Self {
            modules: vec![m],
            tubes: vec![],
        }
    }

    /// Keeper-mode starter formicarium: a small TestTubeNest (module 0)
    /// connected to a roomy Outworld (module 1) by one tube. The nest's
    /// port is on its east wall; the outworld's port is on its west wall.
    pub fn starter_formicarium(nest_dim: (usize, usize), outworld_dim: (usize, usize)) -> Self {
        let (nest_w, nest_h) = nest_dim;
        let (out_w, out_h) = outworld_dim;

        // Lay them out horizontally with a gap that visually represents the tube.
        let gap = 24.0;
        let nest_origin = Vec2::ZERO;
        let outworld_origin = Vec2::new(nest_w as f32 + gap, 0.0);

        let nest_port = PortPos::new(nest_w - 1, nest_h / 2);
        let outworld_port = PortPos::new(0, out_h / 2);

        let nest = Module::new(
            0,
            ModuleKind::TestTubeNest,
            nest_w,
            nest_h,
            nest_origin,
            "Founding Nest",
        )
        .with_ports(vec![nest_port]);
        let outworld = Module::new(
            1,
            ModuleKind::Outworld,
            out_w,
            out_h,
            outworld_origin,
            "Outworld",
        )
        .with_ports(vec![outworld_port]);

        let tube = Tube {
            id: 0,
            from: TubeEnd {
                module: 0,
                port: nest_port,
            },
            to: TubeEnd {
                module: 1,
                port: outworld_port,
            },
            length_ticks: 30,
            bore_width_mm: 8.0,
        };

        tracing::info!(
            nest_size = format!("{}x{}", nest_w, nest_h),
            outworld_size = format!("{}x{}", out_w, out_h),
            tube_length = tube.length_ticks,
            "starter_formicarium built (2 modules, 1 tube)"
        );

        Self {
            modules: vec![nest, outworld],
            tubes: vec![tube],
        }
    }

    pub fn len(&self) -> usize {
        self.modules.len()
    }
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    pub fn module(&self, id: ModuleId) -> &Module {
        &self.modules[id as usize]
    }
    pub fn module_mut(&mut self, id: ModuleId) -> &mut Module {
        &mut self.modules[id as usize]
    }
    pub fn tube(&self, id: TubeId) -> &Tube {
        &self.tubes[id as usize]
    }

    /// Find a tube attached at `(module, port)`. Returns `(tube_id, going_forward)`
    /// where `going_forward = true` if the ant entering here would traverse
    /// `tube.from`→`tube.to` (i.e. they're at the `from` end).
    pub fn tube_at_port(&self, module: ModuleId, port: PortPos) -> Option<(TubeId, bool)> {
        for (i, t) in self.tubes.iter().enumerate() {
            if t.from.module == module && t.from.port == port {
                return Some((i as TubeId, true));
            }
            if t.to.module == module && t.to.port == port {
                return Some((i as TubeId, false));
            }
        }
        None
    }

    /// The `(module_id, port)` at the END of a tube relative to traversal direction.
    pub fn tube_exit(&self, tube_id: TubeId, going_forward: bool) -> (ModuleId, PortPos) {
        let t = self.tube(tube_id);
        if going_forward {
            (t.to.module, t.to.port)
        } else {
            (t.from.module, t.from.port)
        }
    }

    /// The `(module_id, port)` at the START of a tube relative to traversal direction.
    pub fn tube_entry(&self, tube_id: TubeId, going_forward: bool) -> (ModuleId, PortPos) {
        let t = self.tube(tube_id);
        if going_forward {
            (t.from.module, t.from.port)
        } else {
            (t.to.module, t.to.port)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_is_a_one_module_zero_tube_topology() {
        let t = Topology::single(ModuleKind::Outworld, 32, 32);
        assert_eq!(t.modules.len(), 1);
        assert_eq!(t.tubes.len(), 0);
        assert_eq!(t.module(0).width(), 32);
    }

    #[test]
    fn starter_formicarium_has_two_modules_one_tube() {
        let t = Topology::starter_formicarium((32, 24), (96, 96));
        assert_eq!(t.modules.len(), 2);
        assert_eq!(t.tubes.len(), 1);
        assert_eq!(t.module(0).kind, ModuleKind::TestTubeNest);
        assert_eq!(t.module(1).kind, ModuleKind::Outworld);
        let tube = t.tube(0);
        assert_eq!(tube.from.module, 0);
        assert_eq!(tube.to.module, 1);
    }

    #[test]
    fn tube_at_port_finds_both_ends() {
        let t = Topology::starter_formicarium((32, 24), (96, 96));
        let nest_port = t.module(0).ports[0];
        let out_port = t.module(1).ports[0];
        let (_, fw1) = t.tube_at_port(0, nest_port).unwrap();
        let (_, fw2) = t.tube_at_port(1, out_port).unwrap();
        assert!(fw1);
        assert!(!fw2);
    }
}

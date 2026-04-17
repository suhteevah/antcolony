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

    /// Keeper-mode starter formicarium + auto-refilling FeedingDish.
    /// Nest (module 0) east↔Outworld (module 1) west, plus a FeedingDish
    /// (module 2) south of the outworld. Outworld's south-wall port
    /// connects to the dish's north-wall port via a second tube.
    pub fn starter_formicarium_with_feeder(
        nest_dim: (usize, usize),
        outworld_dim: (usize, usize),
        dish_dim: (usize, usize),
    ) -> Self {
        let (nest_w, nest_h) = nest_dim;
        let (out_w, out_h) = outworld_dim;
        let (dish_w, dish_h) = dish_dim;

        let gap = 24.0;
        let nest_origin = Vec2::ZERO;
        let outworld_origin = Vec2::new(nest_w as f32 + gap, 0.0);
        // Dish sits below the outworld with a vertical gap.
        let dish_origin = Vec2::new(
            outworld_origin.x + (out_w as f32 - dish_w as f32) * 0.5,
            -(dish_h as f32) - gap,
        );

        let nest_port = PortPos::new(nest_w - 1, nest_h / 2);
        let outworld_port_w = PortPos::new(0, out_h / 2);
        // Outworld south-wall port (y = 0), centered horizontally.
        let outworld_port_s = PortPos::new(out_w / 2, 0);
        // Dish north-wall port (y = dish_h - 1).
        let dish_port_n = PortPos::new(dish_w / 2, dish_h - 1);

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
        .with_ports(vec![outworld_port_w, outworld_port_s]);
        let dish = Module::new(
            2,
            ModuleKind::FeedingDish,
            dish_w,
            dish_h,
            dish_origin,
            "Feeding Dish",
        )
        .with_ports(vec![dish_port_n]);

        let tube_nest_out = Tube {
            id: 0,
            from: TubeEnd {
                module: 0,
                port: nest_port,
            },
            to: TubeEnd {
                module: 1,
                port: outworld_port_w,
            },
            length_ticks: 30,
            bore_width_mm: 8.0,
        };
        let tube_out_dish = Tube {
            id: 1,
            from: TubeEnd {
                module: 1,
                port: outworld_port_s,
            },
            to: TubeEnd {
                module: 2,
                port: dish_port_n,
            },
            length_ticks: 20,
            bore_width_mm: 8.0,
        };

        tracing::info!(
            nest_size = format!("{}x{}", nest_w, nest_h),
            outworld_size = format!("{}x{}", out_w, out_h),
            dish_size = format!("{}x{}", dish_w, dish_h),
            "starter_formicarium_with_feeder built (3 modules, 2 tubes)"
        );

        Self {
            modules: vec![nest, outworld, dish],
            tubes: vec![tube_nest_out, tube_out_dish],
        }
    }

    pub fn len(&self) -> usize {
        self.modules.len()
    }
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// K2.3: module and tube ids are now STABLE — they do NOT change when
    /// siblings are removed. Lookup is a linear scan, which is fine at the
    /// expected scale (<20 modules, <40 tubes).
    pub fn module(&self, id: ModuleId) -> &Module {
        self.modules
            .iter()
            .find(|m| m.id == id)
            .unwrap_or_else(|| panic!("Topology::module({}) — not found", id))
    }
    pub fn module_mut(&mut self, id: ModuleId) -> &mut Module {
        self.modules
            .iter_mut()
            .find(|m| m.id == id)
            .unwrap_or_else(|| panic!("Topology::module_mut({}) — not found", id))
    }
    pub fn try_module(&self, id: ModuleId) -> Option<&Module> {
        self.modules.iter().find(|m| m.id == id)
    }
    pub fn tube(&self, id: TubeId) -> &Tube {
        self.tubes
            .iter()
            .find(|t| t.id == id)
            .unwrap_or_else(|| panic!("Topology::tube({}) — not found", id))
    }
    pub fn try_tube(&self, id: TubeId) -> Option<&Tube> {
        self.tubes.iter().find(|t| t.id == id)
    }

    /// Smallest unused module id.
    pub fn next_module_id(&self) -> ModuleId {
        (0u16..u16::MAX)
            .find(|cand| !self.modules.iter().any(|m| m.id == *cand))
            .expect("module id space exhausted")
    }

    /// Smallest unused tube id.
    pub fn next_tube_id(&self) -> TubeId {
        (0u16..u16::MAX)
            .find(|cand| !self.tubes.iter().any(|t| t.id == *cand))
            .expect("tube id space exhausted")
    }

    /// Append a new module. Returns its id. Ports start empty unless the
    /// caller supplies them via `with_ports`.
    pub fn add_module(
        &mut self,
        kind: ModuleKind,
        width: usize,
        height: usize,
        origin: Vec2,
        label: impl Into<String>,
    ) -> ModuleId {
        let id = self.next_module_id();
        let module = Module::new(id, kind, width, height, origin, label);
        tracing::info!(id, kind = ?kind, width, height, "Topology::add_module");
        self.modules.push(module);
        id
    }

    /// Append a new tube between two (module, port) endpoints. Does NOT
    /// verify the ports exist on their modules — caller's responsibility.
    pub fn add_tube(
        &mut self,
        from: TubeEnd,
        to: TubeEnd,
        length_ticks: u32,
        bore_width_mm: f32,
    ) -> TubeId {
        let id = self.next_tube_id();
        tracing::info!(
            id,
            from_mod = from.module,
            to_mod = to.module,
            length_ticks,
            bore_width_mm,
            "Topology::add_tube"
        );
        self.tubes.push(Tube {
            id,
            from,
            to,
            length_ticks,
            bore_width_mm,
        });
        id
    }

    /// Remove a module and any tubes attached to it. Returns the list of
    /// removed tube ids so the caller can clean up ants in transit.
    ///
    /// **This does NOT touch ants** — callers that own the ant list (i.e.
    /// `Simulation`) must evict ants whose `module_id == id` or whose
    /// `transit.tube` is in the returned list.
    pub fn remove_module(&mut self, id: ModuleId) -> Vec<TubeId> {
        let removed_tubes: Vec<TubeId> = self
            .tubes
            .iter()
            .filter(|t| t.from.module == id || t.to.module == id)
            .map(|t| t.id)
            .collect();
        self.tubes.retain(|t| !removed_tubes.contains(&t.id));
        let before = self.modules.len();
        self.modules.retain(|m| m.id != id);
        let removed = before - self.modules.len();
        tracing::info!(
            id,
            removed_modules = removed,
            removed_tubes = removed_tubes.len(),
            "Topology::remove_module"
        );
        removed_tubes
    }

    /// Remove one tube by id. Callers must evict ants whose
    /// `transit.tube == id`.
    pub fn remove_tube(&mut self, id: TubeId) -> bool {
        let before = self.tubes.len();
        self.tubes.retain(|t| t.id != id);
        let removed = before != self.tubes.len();
        if removed {
            tracing::info!(id, "Topology::remove_tube");
        }
        removed
    }

    /// Find a tube attached at `(module, port)`. Returns `(tube_id, going_forward)`
    /// where `going_forward = true` if the ant entering here would traverse
    /// `tube.from`→`tube.to` (i.e. they're at the `from` end).
    pub fn tube_at_port(&self, module: ModuleId, port: PortPos) -> Option<(TubeId, bool)> {
        for t in self.tubes.iter() {
            if t.from.module == module && t.from.port == port {
                return Some((t.id, true));
            }
            if t.to.module == module && t.to.port == port {
                return Some((t.id, false));
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

    #[test]
    fn add_module_assigns_stable_id() {
        let mut t = Topology::starter_formicarium((32, 24), (96, 96));
        let id = t.add_module(ModuleKind::Outworld, 40, 40, Vec2::new(200.0, 0.0), "Annex");
        assert_eq!(id, 2);
        assert_eq!(t.module(2).kind, ModuleKind::Outworld);
        assert_eq!(t.module(2).label, "Annex");
    }

    #[test]
    fn remove_module_drops_connected_tubes() {
        let mut t = Topology::starter_formicarium_with_feeder((32, 24), (64, 64), (24, 24));
        // Starter has modules 0,1,2 and tubes 0 (nest<->out) + 1 (out<->dish).
        assert_eq!(t.modules.len(), 3);
        assert_eq!(t.tubes.len(), 2);
        let removed = t.remove_module(1); // outworld
        assert_eq!(removed.len(), 2, "both tubes touched outworld");
        assert_eq!(t.modules.len(), 2);
        assert_eq!(t.tubes.len(), 0);
        // Surviving modules still addressable by their original ids.
        assert_eq!(t.module(0).kind, ModuleKind::TestTubeNest);
        assert_eq!(t.module(2).kind, ModuleKind::FeedingDish);
    }

    #[test]
    fn ids_stay_stable_after_remove() {
        let mut t = Topology::starter_formicarium_with_feeder((32, 24), (64, 64), (24, 24));
        t.remove_module(0); // nest
        // Adding a new module should NOT reuse id 0 — it's still a "hole".
        // Actually with `next_module_id` it WILL reuse 0 because 0 is unused
        // once the nest is gone. Document that: ids are reused from the low end.
        let next = t.next_module_id();
        assert_eq!(next, 0);
    }

    #[test]
    fn remove_tube_by_id_works() {
        let mut t = Topology::starter_formicarium_with_feeder((32, 24), (64, 64), (24, 24));
        assert!(t.remove_tube(0));
        assert!(!t.remove_tube(0), "idempotent: second remove is a no-op");
        assert_eq!(t.tubes.len(), 1);
        assert_eq!(t.tubes[0].id, 1, "tube 1 should survive");
    }
}

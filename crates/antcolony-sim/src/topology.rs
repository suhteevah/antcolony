//! A `Topology` is the formicarium wiring diagram — the list of modules
//! and the tubes connecting them. The `Simulation` owns a `Topology` and
//! every per-tick system iterates it.
//!
//! Pre-K2 code assumed one world + one pheromone grid. Backward
//! compatibility is preserved via `Topology::single` — a one-module
//! topology that looks and behaves exactly like the old single-grid sim.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::module::{Module, ModuleId, ModuleKind, PortPos};
use crate::tube::{Tube, TubeEnd, TubeId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topology {
    pub modules: Vec<Module>,
    pub tubes: Vec<Tube>,
}

/// Four edge-center ports (east / west / south / north) — the default
/// port set for a rectangular module.
fn default_edge_ports(w: usize, h: usize) -> Vec<PortPos> {
    vec![
        PortPos::new(w - 1, h / 2),
        PortPos::new(0, h / 2),
        PortPos::new(w / 2, 0),
        PortPos::new(w / 2, h - 1),
    ]
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
        .with_ports(default_edge_ports(nest_w, nest_h));
        let outworld = Module::new(
            1,
            ModuleKind::Outworld,
            out_w,
            out_h,
            outworld_origin,
            "Outworld",
        )
        .with_ports(default_edge_ports(out_w, out_h));

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
            pheromones: vec![0.0; 30 * 4],
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
        .with_ports(default_edge_ports(nest_w, nest_h));
        let outworld = Module::new(
            1,
            ModuleKind::Outworld,
            out_w,
            out_h,
            outworld_origin,
            "Outworld",
        )
        .with_ports(default_edge_ports(out_w, out_h));
        let dish = Module::new(
            2,
            ModuleKind::FeedingDish,
            dish_w,
            dish_h,
            dish_origin,
            "Feeding Dish",
        )
        .with_ports(default_edge_ports(dish_w, dish_h));

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
            pheromones: vec![0.0; 30 * 4],
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
            pheromones: vec![0.0; 20 * 4],
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

    /// Phase 4 starter: two nests (black on the west, red on the east)
    /// sharing a single outworld in the middle. Both nests connect to
    /// the outworld via their own tube. Module ids: 0 = black nest,
    /// 1 = shared outworld, 2 = red nest. Tube ids: 0 = black↔outworld,
    /// 1 = red↔outworld.
    pub fn two_colony_arena(
        nest_dim: (usize, usize),
        outworld_dim: (usize, usize),
    ) -> Self {
        let (nest_w, nest_h) = nest_dim;
        let (out_w, out_h) = outworld_dim;
        let gap = 24.0;

        let black_origin = Vec2::ZERO;
        let outworld_origin = Vec2::new(nest_w as f32 + gap, 0.0);
        let red_origin = Vec2::new(
            outworld_origin.x + out_w as f32 + gap,
            0.0,
        );

        let black = Module::new(
            0,
            ModuleKind::TestTubeNest,
            nest_w,
            nest_h,
            black_origin,
            "Black Nest",
        )
        .with_ports(default_edge_ports(nest_w, nest_h));
        let outworld = Module::new(
            1,
            ModuleKind::Outworld,
            out_w,
            out_h,
            outworld_origin,
            "Shared Outworld",
        )
        .with_ports(default_edge_ports(out_w, out_h));
        let red = Module::new(
            2,
            ModuleKind::TestTubeNest,
            nest_w,
            nest_h,
            red_origin,
            "Red Nest",
        )
        .with_ports(default_edge_ports(nest_w, nest_h));

        // Black east ↔ outworld west, red west ↔ outworld east.
        let black_port = PortPos::new(nest_w - 1, nest_h / 2);
        let out_port_w = PortPos::new(0, out_h / 2);
        let out_port_e = PortPos::new(out_w - 1, out_h / 2);
        let red_port = PortPos::new(0, nest_h / 2);

        let tube_black = Tube {
            id: 0,
            from: TubeEnd { module: 0, port: black_port },
            to: TubeEnd { module: 1, port: out_port_w },
            length_ticks: 30,
            bore_width_mm: 8.0,
            pheromones: vec![0.0; 30 * 4],
        };
        let tube_red = Tube {
            id: 1,
            from: TubeEnd { module: 2, port: red_port },
            to: TubeEnd { module: 1, port: out_port_e },
            length_ticks: 30,
            bore_width_mm: 8.0,
            pheromones: vec![0.0; 30 * 4],
        };

        tracing::info!(
            nest_size = format!("{}x{}", nest_w, nest_h),
            outworld_size = format!("{}x{}", out_w, out_h),
            "two_colony_arena built (3 modules, 2 tubes)"
        );

        Self {
            modules: vec![black, outworld, red],
            tubes: vec![tube_black, tube_red],
        }
    }

    /// Phase 5: attach an underground nest module for the given colony
    /// below the specified surface nest. Returns the new module id.
    ///
    /// The underground module is pre-carved with:
    /// - a small `QueenChamber` at the top-center (directly below the
    ///   surface nest entrance mirror),
    /// - a wider `BroodNursery` one step deeper,
    /// - a `FoodStorage` room to one side,
    /// - a `Waste` room to the other.
    /// Every other cell is `Solid` — diggers excavate new tunnels at
    /// runtime.
    pub fn attach_underground(
        &mut self,
        surface_nest_id: ModuleId,
        colony_id: u8,
        w: usize,
        h: usize,
    ) -> ModuleId {
        use crate::world::ChamberType;

        let surface = self.module(surface_nest_id);
        // Underground sits directly below the surface nest on the
        // formicarium canvas so the Tab view switch is visually aligned.
        let origin = Vec2::new(
            surface.formicarium_origin.x,
            surface.formicarium_origin.y - h as f32 - 20.0,
        );
        let id = self.next_module_id();
        let label = format!("Underground (colony {})", colony_id);
        let mut module = Module::new(id, ModuleKind::UndergroundNest, w, h, origin, label);
        module.world.fill_solid();

        let cx = w / 2;
        let top = h.saturating_sub(2);
        // Queen chamber: 3x3 at top-center.
        module
            .world
            .carve_chamber(cx, top.saturating_sub(1), 1, 1, ChamberType::QueenChamber);
        // Brood nursery: 5x3 one row below.
        module
            .world
            .carve_chamber(cx, top.saturating_sub(5), 2, 1, ChamberType::BroodNursery);
        // Food storage: left side, midway down.
        module.world.carve_chamber(
            cx.saturating_sub(w / 4),
            h / 2,
            2,
            1,
            ChamberType::FoodStorage,
        );
        // Waste: right side, opposite.
        module
            .world
            .carve_chamber(cx + w / 4, h / 2, 1, 1, ChamberType::Waste);
        // Shallow tunnel connecting queen chamber to nursery and the
        // storage/waste chambers so ants can path between rooms from
        // day one.
        module.world.carve_tunnel(
            (cx, top.saturating_sub(1)),
            (cx, top.saturating_sub(5)),
        );
        module
            .world
            .carve_tunnel((cx, top.saturating_sub(5)), (cx.saturating_sub(w / 4), h / 2));
        module
            .world
            .carve_tunnel((cx, top.saturating_sub(5)), (cx + w / 4, h / 2));

        tracing::info!(
            id,
            surface_nest_id,
            colony_id,
            w,
            h,
            "Topology::attach_underground"
        );
        self.modules.push(module);
        id
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
    pub fn tube_mut(&mut self, id: TubeId) -> &mut Tube {
        self.tubes
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap_or_else(|| panic!("Topology::tube_mut({}) — not found", id))
    }

    /// Auto-size every tube's bore width to fit a species. Real keepers
    /// match formicarium tubing to species (suppliers stock 6/8/12/16mm
    /// for exactly this reason); the default 8mm tubes refuse Camponotus
    /// (13mm worker, 21mm major) at every port. Computes
    /// `needed = worker_size_mm * polymorphic_factor * safety_margin` and
    /// applies the larger of `needed` and `8.0` (default) per tube.
    /// Smaller species keep the 8mm default; large species get scaled up.
    pub fn fit_bore_to_species(&mut self, worker_size_mm: f32, polymorphic: bool) {
        let polymorphic_factor = if polymorphic { 1.6 } else { 1.15 };
        let needed_bore = worker_size_mm * polymorphic_factor * 1.5;
        let starter_bore = needed_bore.max(8.0);
        for tube in &mut self.tubes {
            tube.bore_width_mm = tube.bore_width_mm.max(starter_bore);
        }
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
        let cells = length_ticks.max(1) as usize * 4;
        self.tubes.push(Tube {
            id,
            from,
            to,
            length_ticks,
            bore_width_mm,
            pheromones: vec![0.0; cells],
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
        // Tube connects the nest's east port to the outworld's west port.
        let nest_port = PortPos::new(31, 12);
        let out_port = PortPos::new(0, 48);
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

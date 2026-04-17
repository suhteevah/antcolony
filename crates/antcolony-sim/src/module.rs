//! A **module** is a self-contained ant habitat — one world grid, one
//! pheromone field, and a set of ports where tubes can attach. Multiple
//! modules linked by tubes make a formicarium (the AntsCanada-style
//! modular ant-keeping setup that K2 models).
//!
//! A module's coordinate system is local: positions on `world` and
//! `pheromones` are both `0..width × 0..height`. `formicarium_origin` is
//! where the module's `(0,0)` sits in the larger formicarium plane used
//! by the renderer and the topology-board view.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::pheromone::PheromoneGrid;
use crate::world::WorldGrid;

pub type ModuleId = u16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModuleKind {
    /// Glass test tube with a cotton-plugged water reservoir — the
    /// classic founding nest every ant-keeper starts with.
    TestTubeNest,
    /// Open foraging chamber — where the colony finds food.
    Outworld,
    /// Y-Tong / aerated concrete nest; chambers are excavatable.
    YTongNest,
    /// Acrylic formicarium — glass-fronted chambered nest.
    AcrylicNest,
    /// Water/hydration chamber.
    Hydration,
    /// Heated rest chamber (K3).
    HeatChamber,
    /// Cold chamber for diapause (K3).
    HibernationChamber,
    /// Feeding tray.
    FeedingDish,
    /// Waste / cemetery.
    Graveyard,
}

impl ModuleKind {
    pub fn label(&self) -> &'static str {
        match self {
            ModuleKind::TestTubeNest => "Test Tube Nest",
            ModuleKind::Outworld => "Outworld",
            ModuleKind::YTongNest => "Y-Tong Nest",
            ModuleKind::AcrylicNest => "Acrylic Nest",
            ModuleKind::Hydration => "Hydration",
            ModuleKind::HeatChamber => "Heat Chamber",
            ModuleKind::HibernationChamber => "Hibernation Chamber",
            ModuleKind::FeedingDish => "Feeding Dish",
            ModuleKind::Graveyard => "Graveyard",
        }
    }
}

/// A cell on a module's border where a tube can attach.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PortPos {
    pub x: u16,
    pub y: u16,
}

impl PortPos {
    pub fn new(x: usize, y: usize) -> Self {
        Self {
            x: x as u16,
            y: y as u16,
        }
    }

    pub fn as_usize(&self) -> (usize, usize) {
        (self.x as usize, self.y as usize)
    }

    pub fn to_vec2(&self) -> Vec2 {
        Vec2::new(self.x as f32 + 0.5, self.y as f32 + 0.5)
    }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub id: ModuleId,
    pub kind: ModuleKind,
    pub world: WorldGrid,
    pub pheromones: PheromoneGrid,
    /// World-space offset of the module's (0,0) corner. Used by the
    /// renderer to place modules side-by-side in formicarium view.
    pub formicarium_origin: Vec2,
    /// Cells where tubes may attach. Usually one per edge the keeper
    /// has routed a tube to.
    pub ports: Vec<PortPos>,
    pub label: String,
}

impl Module {
    pub fn new(
        id: ModuleId,
        kind: ModuleKind,
        width: usize,
        height: usize,
        formicarium_origin: Vec2,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id,
            kind,
            world: WorldGrid::new(width, height),
            pheromones: PheromoneGrid::new(width, height),
            formicarium_origin,
            ports: Vec::new(),
            label: label.into(),
        }
    }

    pub fn with_ports(mut self, ports: Vec<PortPos>) -> Self {
        self.ports = ports;
        self
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.world.width
    }
    #[inline]
    pub fn height(&self) -> usize {
        self.world.height
    }

    /// Heading pointing *into* the module from the given port cell. Used
    /// to orient an ant emerging from a tube so it doesn't immediately
    /// try to walk back into the wall.
    pub fn port_interior_heading(&self, port: PortPos) -> f32 {
        let w = self.width();
        let h = self.height();
        let (px, py) = port.as_usize();
        let on_west = px == 0;
        let on_east = px + 1 >= w;
        let on_south = py == 0;
        let on_north = py + 1 >= h;
        // Priority: corners pick the longer axis arbitrarily.
        if on_west {
            0.0 // east
        } else if on_east {
            std::f32::consts::PI
        } else if on_south {
            std::f32::consts::FRAC_PI_2
        } else if on_north {
            -std::f32::consts::FRAC_PI_2
        } else {
            // Interior port (unusual) — no preferred direction.
            0.0
        }
    }
}

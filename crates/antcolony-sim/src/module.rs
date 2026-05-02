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
    /// Phase 5: side-view underground nest — all cells start `Solid`;
    /// ants in `Digging` state excavate tunnels and chambers.
    UndergroundNest,
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
            ModuleKind::UndergroundNest => "Underground",
        }
    }
}

/// Dig system Phase B: per-module diggable-substrate variant. Drives
/// the underground module's wall + tunnel rendering palette and
/// (eventually) the per-tile dig speed multiplier. Only meaningful for
/// `UndergroundNest` and `YTongNest` module kinds; surface modules
/// always use `Loam`-equivalent rendering and don't expose the field.
///
/// Real keepers match formicarium nest material to species — see
/// `docs/biology.md` "Substrate type changes everything" + the
/// `assets/sprite_prompts/ENVIRONMENT_PROMPTS.md` Tier 1 substrate
/// section for the visual reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SubstrateKind {
    /// Dark organic forest-floor mix. The default. Most temperate species
    /// (Lasius, Aphaenogaster, Formica, Tetramorium) suit Loam.
    #[default]
    Loam,
    /// Pale arid sand. Pogonomyrmex and other granivores. Tunnels collapse
    /// easily in nature; in sim, dig is fast but a future "tunnel decay"
    /// pass could revert idle Empty cells back toward Solid.
    Sand,
    /// Aerated autoclaved concrete (Y-Tong). The keeper-favorite "permanent"
    /// nest material. Pre-carved chambers; ants chew tunnels slowly. In
    /// sim: harder to dig (`dig_speed_multiplier` 0.7).
    Ytong,
    /// Soft-rotted wood. Camponotus excavates galleries through this.
    /// Mandible-only — no pellet rolling biology — but for the sim we
    /// still treat it as substrate with a different color palette.
    Wood,
    /// Translucent nutritive blue gel (NASA-style ant farm). Sci-fi
    /// novelty substrate; in nature it would be a slow death (no
    /// protein) but in sim it just gets a distinctive cool palette.
    Gel,
}

impl SubstrateKind {
    /// Multiplier applied to per-substep dig progress accumulation.
    /// Loam = 1.0 baseline; harder substrates take longer per tile.
    /// Future: combine with species `appearance.dig_speed_multiplier`.
    pub fn dig_speed_multiplier(self) -> f32 {
        match self {
            SubstrateKind::Loam => 1.0,
            SubstrateKind::Sand => 1.2,
            SubstrateKind::Ytong => 0.7,
            SubstrateKind::Wood => 0.5,
            SubstrateKind::Gel => 1.5,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Per-module cooldown counter. Used by e.g. FeedingDish for refill
    /// timing. Decremented each tick; behavior-specific systems check it.
    pub tick_cooldown: u32,
    /// Per-cell temperature in °C (K3). Row-major, same indexing as
    /// pheromone grids. Initialized to 20.0 and drifts each tick toward
    /// `ambient_target`.
    pub temperature: Vec<f32>,
    /// Target temperature the grid is drifting toward. Set by
    /// `Simulation::temperature_tick` based on module kind + ambient.
    pub ambient_target: f32,
    /// Dig system Phase B: which substrate this module's diggable cells
    /// are made of. Only meaningful for `UndergroundNest` (and YTongNest);
    /// surface modules ignore the field at render time. Defaults to
    /// `Loam` for snapshot compatibility.
    #[serde(default)]
    pub substrate: SubstrateKind,
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
        let n = width * height;
        Self {
            id,
            kind,
            world: WorldGrid::new(width, height),
            pheromones: PheromoneGrid::new(width, height),
            formicarium_origin,
            ports: Vec::new(),
            label: label.into(),
            tick_cooldown: 0,
            temperature: vec![20.0; n],
            ambient_target: 20.0,
            substrate: SubstrateKind::default(),
        }
    }

    /// Nearest-cell temperature lookup (K3).
    pub fn temp_at(&self, pos: Vec2) -> f32 {
        let w = self.width();
        let h = self.height();
        let x = (pos.x.floor() as i64).clamp(0, w as i64 - 1) as usize;
        let y = (pos.y.floor() as i64).clamp(0, h as i64 - 1) as usize;
        self.temperature[y * w + x]
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

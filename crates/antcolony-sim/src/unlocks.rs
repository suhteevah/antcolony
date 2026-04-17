//! K4 progression — module-kind unlocks gated by in-game days and
//! colony population.
//!
//! Keeps the keeper-mode palette honest: a brand-new founding queen can
//! only see basic modules; fancier modules unlock as the colony earns
//! them. Rules are pure functions, no side-effects.

use crate::module::ModuleKind;

pub fn module_kind_unlocked(kind: ModuleKind, total_days: u32, population: u32) -> bool {
    match kind {
        ModuleKind::TestTubeNest | ModuleKind::Outworld | ModuleKind::FeedingDish => true,
        ModuleKind::Hydration => population >= 10,
        ModuleKind::YTongNest => total_days >= 14 || population >= 50,
        ModuleKind::AcrylicNest => population >= 100,
        ModuleKind::HeatChamber => total_days >= 30,
        ModuleKind::HibernationChamber => total_days >= 180,
        ModuleKind::Graveyard => total_days >= 7,
    }
}

/// Human-friendly "when does this unlock?" string, used by the editor
/// tooltip on locked palette buttons.
pub fn unlock_hint(kind: ModuleKind) -> &'static str {
    match kind {
        ModuleKind::TestTubeNest | ModuleKind::Outworld | ModuleKind::FeedingDish => "Unlocked",
        ModuleKind::Hydration => "Unlocks at 10 ants",
        ModuleKind::YTongNest => "Unlocks at Day 14 or 50 ants",
        ModuleKind::AcrylicNest => "Unlocks at 100 ants",
        ModuleKind::HeatChamber => "Unlocks at Day 30",
        ModuleKind::HibernationChamber => "Unlocks at Day 180",
        ModuleKind::Graveyard => "Unlocks at Day 7",
    }
}

//! Extended (Phase A) species schema — additive optional sections.
//!
//! # Why a separate file
//!
//! `species.rs` is the original Phase 1 schema (biology / growth / diet /
//! combat / appearance / encyclopedia). It is shipped, tested, and stable.
//! This file adds new OPTIONAL sections that let TOMLs express biology
//! the original schema couldn't (substrate, recruitment, polydomy, etc.)
//! without forcing every existing TOML to fill them.
//!
//! Every type here is `#[serde(default)]`-friendly — a TOML that omits
//! the section parses as `None` (or the section's `Default`). Existing
//! TOMLs continue to load unchanged.
//!
//! # Citation discipline
//!
//! Every variant of every enum carries a doc comment that explains
//! *what* the variant means biologically, *not* the sim mechanic that
//! consumes it. Sim hooks live in Phase B.
//!
//! # Schema versioning
//!
//! `Species::schema_version` defaults to 1. Bumped to 2 when this
//! module's types acquire breaking changes. Backwards-compatible
//! additions do NOT bump the version.

use serde::{Deserialize, Serialize};

// ============================================================
// [behavior] — how the species moves and recruits.
// ============================================================

/// How workers recruit nestmates to a food source.
///
/// The biological meaning, not the sim mechanic:
/// - `Mass` — many workers deposit trail pheromone, leading to a
///   self-amplifying chemical road.
/// - `TandemRun` — one worker leads exactly one follower along a
///   landmark route, no chemical road; slow but precise.
/// - `Group` — a small group is led, intermediate between mass and
///   tandem.
/// - `Individual` — no recruitment; each worker forages alone.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecruitmentStyle {
    #[default]
    Mass,
    TandemRun,
    Group,
    Individual,
}

/// When the species is active.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DielActivity {
    #[default]
    Diurnal,
    Nocturnal,
    Crepuscular,
    /// Active in both day and night without strong preference.
    Cathemeral,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Behavior {
    #[serde(default)]
    pub recruitment: RecruitmentStyle,
    #[serde(default)]
    pub diel_activity: DielActivity,
    /// Trail-pheromone half-life in seconds. Lasius niger's
    /// classic Beckers/Deneubourg measurement is ~2820s (47 min);
    /// many other species are shorter or longer. `None` means the
    /// global default applies.
    #[serde(default)]
    pub trail_half_life_seconds: Option<u32>,
}

// ============================================================
// [colony_structure] — queen count, polydomy, supercoloniality.
// ============================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum QueenCount {
    #[default]
    Monogyne,
    /// Single queen typical but multi-queen tolerated under stress.
    FacultativelyPolygyne,
    /// Multiple cooperating queens are the species' normal state.
    ObligatePolygyne,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ColonyStructure {
    #[serde(default)]
    pub queen_count: QueenCount,
    /// Multi-nest with shared workers and brood transport.
    #[serde(default)]
    pub polydomous: bool,
    /// Probability per in-game month that a polydomous colony moves
    /// one of its nests. Ignored when `polydomous == false`.
    /// Clamped to 0..=1 by consumers.
    #[serde(default)]
    pub relocation_tendency: f32,
    /// Whether the editor exposes a "supercolony / urban scale" toggle
    /// for this species (Tapinoma, Formica, Linepithema).
    #[serde(default)]
    pub supercolony_capable: bool,
    /// Founds new colonies by fission (budding) in addition to / instead
    /// of nuptial flight.
    #[serde(default)]
    pub budding_reproduction: bool,
}

// ============================================================
// [substrate] — nest material and excavation behavior.
// ============================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubstrateType {
    Loam,
    Sand,
    Wood,
    LeafLitter,
    RockCrevice,
    Thatch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MoundConstruction {
    /// Workers carry tunnel spoil out of the entrance and dump it; the
    /// resulting mound is incidental, not engineered (Lasius default).
    #[default]
    Kickout,
    /// Engineered dome of conifer needles + twigs (Formica rufa).
    Thatch,
    /// Cleared circular pad with central crater entrance (Pogonomyrmex).
    CraterDisc,
    /// No surface mound at all.
    None,
    /// Stone or shell lid placed over a hole.
    LidDome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Substrate {
    /// Substrate types this species WILL nest in. Order = preference.
    #[serde(default)]
    pub preferred: Vec<SubstrateType>,
    /// Substrate types this species REFUSES to nest in.
    #[serde(default)]
    pub incompatible: Vec<SubstrateType>,
    /// Excavation rate scalar vs the Lasius baseline. Larger species
    /// or species adapted to softer substrates dig faster.
    #[serde(default = "one_f32")]
    pub dig_speed_multiplier: f32,
    #[serde(default)]
    pub mound_construction: MoundConstruction,
}

// Manual Default — the `#[serde(default = "one_f32")]` attribute only
// triggers during deserialization, not on `Substrate::default()`. Without
// this manual impl, `dig_speed_multiplier` would default to f32's zero.
impl Default for Substrate {
    fn default() -> Self {
        Self {
            preferred: Vec::new(),
            incompatible: Vec::new(),
            dig_speed_multiplier: 1.0,
            mound_construction: MoundConstruction::default(),
        }
    }
}

fn one_f32() -> f32 {
    1.0
}

// ============================================================
// [combat_extended] — weaponry and caste body-size details.
// ============================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Weapon {
    /// Mandible-only — bite damage, no chemistry.
    #[default]
    Mandible,
    /// Functional sting (Pogonomyrmex, Myrmica, Solenopsis).
    Sting,
    /// Acid spray from acidopore (Formica, large Lasius).
    FormicSpray,
    /// Other chemical defense (Crematogaster trail pheromone-as-deterrent etc.).
    Chemical,
}

/// Polymorphic worker size buckets, when `biology.polymorphic == true`.
/// Order is small → large.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerSizeBucket {
    Minor,
    Media,
    Major,
    /// Phragmotic super-major with head used as a living door.
    SuperMajor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatExtended {
    #[serde(default)]
    pub weapon: Weapon,
    /// Schmidt-style sting potency, 0..5. 0 = no sting. Reference points:
    /// honeybee≈2.0, bullet ant≈4.0, *Pogonomyrmex maricopa*≈3.0.
    #[serde(default)]
    pub sting_potency: f32,
    /// True if the species can deliver damage at distance (acid spray).
    #[serde(default)]
    pub ranged_attack: bool,
    /// Worker size buckets present in the colony. Empty = monomorphic.
    #[serde(default)]
    pub soldier_size_categories: Vec<WorkerSizeBucket>,
    /// Damage scalar for the largest caste vs a minor worker.
    #[serde(default = "one_f32")]
    pub major_attack_multiplier: f32,
    /// Boulay 2024 (L. niger) — aggression depends on intruder identity.
    #[serde(default)]
    pub context_aggression: bool,
}

// Manual Default — same reason as Substrate above (serde attribute does
// not affect the Default trait).
impl Default for CombatExtended {
    fn default() -> Self {
        Self {
            weapon: Weapon::default(),
            sting_potency: 0.0,
            ranged_attack: false,
            soldier_size_categories: Vec::new(),
            major_attack_multiplier: 1.0,
            context_aggression: false,
        }
    }
}

// ============================================================
// [diet_extended] — specialized food strategies.
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DietExtended {
    /// Aphaenogaster-style elaiosome seed dispersal. Workers carry
    /// elaiosome-bearing seeds home, eat the elaiosome, discard the
    /// viable seed in the midden where it germinates.
    #[serde(default)]
    pub seed_dispersal: bool,
    /// Workers depend on aphid honeydew for primary carbohydrate.
    /// Without nearby aphid colonies the species' growth rate drops.
    #[serde(default)]
    pub honeydew_dependent: bool,
    /// For parasitic founding: list of host species ids the founding
    /// queen takes over (Formica rufa requires Formica fusca).
    #[serde(default)]
    pub host_species_required: Vec<String>,
    /// Maximum food units this species' colonies can store. None = use
    /// the runtime default (`target_population * egg_cost * 10`). Caps
    /// realistic per-colony reserves; pre-cap, A. rudis colonies grew
    /// 21,000+ food storage in the 2yr smoke (1-2 OOM above field-
    /// realistic). See docs/postmortems/2026-05-09-seasonal-transition-cliffs.md.
    #[serde(default)]
    pub food_storage_cap: Option<f32>,
}

// ============================================================
// [ecological_role] — invasion ecology + interspecies relations.
// ============================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum InvasiveStatus {
    #[default]
    Native,
    /// Established outside native range but not aggressively displacing.
    Introduced,
    /// Aggressively displacing native fauna (Linepithema, Solenopsis invicta).
    InvasivePest,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EcologicalRole {
    /// Surfaced in encyclopedia and main-game scoring as a "keystone" species.
    #[serde(default)]
    pub keystone: bool,
    #[serde(default)]
    pub invasive_status: InvasiveStatus,
    /// Species ids this one outcompetes in shared range.
    #[serde(default)]
    pub displaces: Vec<String>,
    /// Species ids that outcompete this one.
    #[serde(default)]
    pub displaced_by: Vec<String>,
}

// ============================================================
// Schema version constant.
// ============================================================

/// Current Phase A schema version. Bumped only on breaking changes
/// to the types in this module. Additive changes do NOT bump.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Default for `Species::schema_version` when the TOML omits it.
/// Always 1 — we are at version 1 of the extended schema.
pub fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behavior_defaults_to_mass_diurnal_no_override() {
        let b = Behavior::default();
        assert_eq!(b.recruitment, RecruitmentStyle::Mass);
        assert_eq!(b.diel_activity, DielActivity::Diurnal);
        assert!(b.trail_half_life_seconds.is_none());
    }

    #[test]
    fn colony_structure_defaults_to_monogyne() {
        let c = ColonyStructure::default();
        assert_eq!(c.queen_count, QueenCount::Monogyne);
        assert!(!c.polydomous);
        assert!(!c.supercolony_capable);
    }

    #[test]
    fn substrate_defaults_to_kickout_loam_capable() {
        let s = Substrate::default();
        assert!(s.preferred.is_empty());
        assert!(s.incompatible.is_empty());
        assert!((s.dig_speed_multiplier - 1.0).abs() < f32::EPSILON);
        assert_eq!(s.mound_construction, MoundConstruction::Kickout);
    }

    #[test]
    fn combat_extended_defaults_to_monomorphic_mandible() {
        let c = CombatExtended::default();
        assert_eq!(c.weapon, Weapon::Mandible);
        assert_eq!(c.sting_potency, 0.0);
        assert!(!c.ranged_attack);
        assert!(c.soldier_size_categories.is_empty());
        assert!(!c.context_aggression);
    }

    #[test]
    fn diet_extended_defaults_to_no_specializations() {
        let d = DietExtended::default();
        assert!(!d.seed_dispersal);
        assert!(!d.honeydew_dependent);
        assert!(d.host_species_required.is_empty());
    }

    #[test]
    fn ecological_role_defaults_to_native_non_keystone() {
        let e = EcologicalRole::default();
        assert!(!e.keystone);
        assert_eq!(e.invasive_status, InvasiveStatus::Native);
        assert!(e.displaces.is_empty());
        assert!(e.displaced_by.is_empty());
    }

    #[test]
    fn schema_version_default_is_1() {
        assert_eq!(default_schema_version(), 1);
        assert_eq!(CURRENT_SCHEMA_VERSION, 1);
    }

    #[test]
    fn enums_round_trip_via_toml() {
        let toml_str = r#"
recruitment = "tandem_run"
diel_activity = "nocturnal"
"#;
        let b: Behavior = toml::from_str(toml_str).unwrap();
        assert_eq!(b.recruitment, RecruitmentStyle::TandemRun);
        assert_eq!(b.diel_activity, DielActivity::Nocturnal);
    }

    #[test]
    fn substrate_enum_round_trip() {
        let toml_str = r#"
preferred = ["wood", "leaf_litter"]
incompatible = ["sand"]
dig_speed_multiplier = 0.7
mound_construction = "none"
"#;
        let s: Substrate = toml::from_str(toml_str).unwrap();
        assert_eq!(s.preferred, vec![SubstrateType::Wood, SubstrateType::LeafLitter]);
        assert_eq!(s.incompatible, vec![SubstrateType::Sand]);
        assert!((s.dig_speed_multiplier - 0.7).abs() < f32::EPSILON);
        assert_eq!(s.mound_construction, MoundConstruction::None);
    }

    #[test]
    fn diet_extended_food_storage_cap_optional() {
        // Loading a TOML without food_storage_cap should succeed and yield None.
        let toml_no_cap = r#""#;
        let d: DietExtended = toml::from_str(toml_no_cap).expect("parse");
        assert_eq!(d.food_storage_cap, None);

        // Loading with an explicit cap should round-trip the value.
        let toml_with_cap = r#"food_storage_cap = 2500.0"#;
        let d: DietExtended = toml::from_str(toml_with_cap).expect("parse");
        assert_eq!(d.food_storage_cap, Some(2500.0));
    }
}

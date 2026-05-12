//! Verify the shipped TOMLs round-trip through the Phase A schema:
//! - parse cleanly
//! - load with `schema_version = 1` at top level (NOT nested under another section)
//! - Phase A extension sections are present and not all-defaults
//!
//! Species count was historically pinned to 7 (Phase A snapshot). The
//! pool has since grown — assert a floor instead of an exact match so
//! adding new TOMLs doesn't break this test.

use antcolony_sim::{
    DielActivity, MoundConstruction, QueenCount, RecruitmentStyle, Weapon, load_species_dir,
};

fn species_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("species")
}

#[test]
fn all_shipped_species_load() {
    let species = load_species_dir(&species_dir()).unwrap();
    assert!(
        species.len() >= 7,
        "expected at least 7 shipped species, got {}",
        species.len()
    );
    for s in &species {
        assert_eq!(
            s.schema_version, 1,
            "species `{}` schema_version should be 1, got {} \
             (likely a TOML placement bug — `schema_version` must be top-level)",
            s.id, s.schema_version,
        );
    }
}

#[test]
fn phase_a_extensions_are_actually_populated() {
    let species = load_species_dir(&species_dir()).unwrap();
    for s in &species {
        // Substrate.preferred should be non-empty for every shipped species —
        // a default-empty would mean the TOML's [substrate] section was lost.
        assert!(
            !s.substrate.preferred.is_empty(),
            "species `{}` has empty substrate.preferred — Phase A section likely missing or misplaced",
            s.id,
        );
    }
}

#[test]
fn camponotus_is_tandem_recruiter_and_polymorphic_with_wood_substrate() {
    let species = load_species_dir(&species_dir()).unwrap();
    let camp = species
        .iter()
        .find(|s| s.id == "camponotus_pennsylvanicus")
        .expect("camponotus not loaded");
    assert_eq!(camp.behavior.recruitment, RecruitmentStyle::TandemRun);
    assert_eq!(camp.behavior.diel_activity, DielActivity::Nocturnal);
    assert_eq!(camp.colony_structure.queen_count, QueenCount::Monogyne);
    assert!(camp.colony_structure.polydomous, "Camponotus is polydomous (satellite nests)");
    assert!(camp.substrate.preferred.iter().any(|s| matches!(s, antcolony_sim::SubstrateType::Wood)));
}

#[test]
fn formica_is_polygyne_supercolony_with_thatch_and_ranged_acid() {
    let species = load_species_dir(&species_dir()).unwrap();
    let f = species.iter().find(|s| s.id == "formica_rufa").expect("formica not loaded");
    assert_eq!(f.colony_structure.queen_count, QueenCount::ObligatePolygyne);
    assert!(f.colony_structure.polydomous);
    assert!(f.colony_structure.supercolony_capable);
    assert!(f.colony_structure.budding_reproduction);
    assert_eq!(f.substrate.mound_construction, MoundConstruction::Thatch);
    assert_eq!(f.combat_extended.weapon, Weapon::FormicSpray);
    assert!(f.combat_extended.ranged_attack);
    assert!(f.diet_extended.honeydew_dependent);
    assert!(f.diet_extended.host_species_required.iter().any(|h| h == "formica_fusca"));
    assert!(f.ecological_role.keystone);
}

#[test]
fn pogonomyrmex_is_granivore_with_sting_and_crater_disc() {
    let species = load_species_dir(&species_dir()).unwrap();
    let p = species.iter().find(|s| s.id == "pogonomyrmex_occidentalis").expect("pogo not loaded");
    assert_eq!(p.combat_extended.weapon, Weapon::Sting);
    assert!(
        p.combat_extended.sting_potency >= 2.5,
        "Pogo sting potency should reflect Schmidt 3.0; got {}",
        p.combat_extended.sting_potency,
    );
    assert_eq!(p.substrate.mound_construction, MoundConstruction::CraterDisc);
    assert!(!p.diet_extended.seed_dispersal, "Pogo eats seeds (granivory) ≠ disperses them (myrmecochory)");
    assert!(p.ecological_role.keystone);
}

#[test]
fn aphaenogaster_is_seed_disperser_with_keystone_role() {
    let species = load_species_dir(&species_dir()).unwrap();
    let a = species.iter().find(|s| s.id == "aphaenogaster_rudis").expect("aphae not loaded");
    assert!(a.diet_extended.seed_dispersal, "Aphaenogaster IS the myrmecochore");
    assert!(a.ecological_role.keystone);
    assert_eq!(a.combat_extended.weapon, Weapon::Sting);
}

#[test]
fn tapinoma_is_polydomous_supercolony_with_chemical_alarm() {
    let species = load_species_dir(&species_dir()).unwrap();
    let t = species.iter().find(|s| s.id == "tapinoma_sessile").expect("tapinoma not loaded");
    assert_eq!(t.colony_structure.queen_count, QueenCount::ObligatePolygyne);
    assert!(t.colony_structure.polydomous);
    assert!(t.colony_structure.supercolony_capable);
    assert!(t.colony_structure.relocation_tendency > 0.0, "Tapinoma should have non-zero relocation tendency");
    assert_eq!(t.combat_extended.weapon, Weapon::Chemical);
}

#[test]
fn lasius_baseline_recruitment_is_mass_with_long_trail_half_life() {
    let species = load_species_dir(&species_dir()).unwrap();
    let l = species.iter().find(|s| s.id == "lasius_niger").expect("lasius not loaded");
    assert_eq!(l.behavior.recruitment, RecruitmentStyle::Mass);
    assert_eq!(
        l.behavior.trail_half_life_seconds,
        Some(2820),
        "Lasius trail half-life should be the Beckers/Deneubourg ~47min figure",
    );
}

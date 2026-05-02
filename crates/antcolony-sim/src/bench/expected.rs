//! Per-species expected-range table for the species bench harness.
//!
//! # Audit discipline
//!
//! This file is the **biology authority** for the harness. Every numeric
//! range here is a citation against published literature OR an explicit
//! game-pacing tag with a written reason. An ecologist reading this file
//! should be able to:
//!
//! 1. See **what we expect** the simulation to produce for a given species.
//! 2. See **why** we expect that (citation).
//! 3. See **how strict** the expectation is (`Tolerance` band).
//!
//! When literature disagrees, we capture the *range* and tag it
//! `LiteratureRange` with both endpoint sources. When a value is a
//! deliberate sim-pacing choice (e.g. accelerated maturation), it is
//! tagged `GamePacing` with a written reason.
//!
//! Cross-references:
//! - `docs/biology.md` — general mechanism citations
//! - `docs/species/{id}.md` — per-species PhD-level entry, primary source
//! - `assets/species/{id}.toml` — sim parameters with inline citation comments
//!
//! # Editing rules for contributors
//!
//! - **Never** add an `ExpectedRange` without a `Citation`. CI rejects it.
//! - When you change a number, update the citation. Stale citations are
//!   worse than missing ones.
//! - When literature changes (new paper supersedes old), keep the old
//!   citation in a `// historical:` comment so the audit trail is intact.

use std::fmt;

/// How the value was sourced. Every expected range carries one of these.
///
/// Human description: "Where does this number come from?"
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Citation {
    /// A peer-reviewed publication. Format: "Author Year, Journal Vol(Issue):Pages".
    PeerReviewed(&'static str),
    /// A reference book or canonical monograph (Hölldobler & Wilson 1990, Hansen & Klotz 2005).
    ReferenceWork(&'static str),
    /// AntWiki / AntWeb / other curated taxonomic database.
    TaxonomicDatabase(&'static str),
    /// Government / extension service publication (e.g. UF/IFAS, USFS).
    Extension(&'static str),
    /// Multiple sources span a range; we cite both endpoints.
    LiteratureRange { low: &'static str, high: &'static str },
    /// Deliberate sim-pacing choice with no direct biological measurement.
    /// The `&'static str` is the written rationale (NOT a citation).
    GamePacing(&'static str),
    /// Cross-reference to our own internal doc (typically `docs/species/*.md`
    /// which itself carries the primary citation).
    InternalDoc(&'static str),
}

impl fmt::Display for Citation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Citation::PeerReviewed(s) => write!(f, "Peer-reviewed: {s}"),
            Citation::ReferenceWork(s) => write!(f, "Reference: {s}"),
            Citation::TaxonomicDatabase(s) => write!(f, "Taxonomic DB: {s}"),
            Citation::Extension(s) => write!(f, "Extension: {s}"),
            Citation::LiteratureRange { low, high } => {
                write!(f, "Literature range: low={low}, high={high}")
            }
            Citation::GamePacing(s) => write!(f, "Game-pacing (NOT biological): {s}"),
            Citation::InternalDoc(s) => write!(f, "Internal doc: {s}"),
        }
    }
}

/// How tolerant the harness is when comparing observed-to-expected.
///
/// Human description: "How close to the expected value does the sim need
/// to land before we call it a match?"
///
/// - `Strict` (±10%): for values that are well-measured in literature
///   (e.g. egg-to-adult duration at standard temperature).
/// - `Loose` (±50%): for values with high natural variance (e.g.
///   mature colony population, where two-orders-of-magnitude variation
///   is normal across populations).
/// - `OrderOfMagnitude` (×0.1 to ×10): for values where the sim only
///   needs to be in the right ballpark (e.g. food return rate, which
///   depends on map layout).
/// - `Custom(low, high)` for explicit asymmetric tolerances.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tolerance {
    Strict,
    Loose,
    OrderOfMagnitude,
    Custom { low_mult: f64, high_mult: f64 },
}

impl Tolerance {
    /// Returns the (low, high) multiplier band against the expected centroid.
    pub fn band(&self) -> (f64, f64) {
        match self {
            Tolerance::Strict => (0.9, 1.1),
            Tolerance::Loose => (0.5, 1.5),
            Tolerance::OrderOfMagnitude => (0.1, 10.0),
            Tolerance::Custom { low_mult, high_mult } => (*low_mult, *high_mult),
        }
    }

    pub fn human_description(&self) -> &'static str {
        match self {
            Tolerance::Strict => "±10% — value is well-measured in literature",
            Tolerance::Loose => "±50% — high natural variance is normal",
            Tolerance::OrderOfMagnitude => "0.1× to 10× — only ballpark required",
            Tolerance::Custom { .. } => "custom asymmetric tolerance",
        }
    }
}

/// One observable expected to fall within a literature-cited range.
///
/// All fields are public so the report module can render them verbatim
/// into Markdown / CSV without going through accessors.
#[derive(Debug, Clone)]
pub struct ExpectedRange {
    /// Plain-English name of what we are measuring (NO jargon).
    /// Example: "Number of adult workers at the end of year 5".
    pub human_name: &'static str,
    /// One-sentence plain-English explanation of WHY this matters.
    /// Example: "If the colony cannot reach a 4-figure worker population
    /// by year 5, it will likely not survive an adult cohort die-off."
    pub human_why: &'static str,
    /// Expected centroid value (the "typical" observation in literature).
    pub centroid: f64,
    /// How strict the comparison is.
    pub tolerance: Tolerance,
    /// Where the centroid came from.
    pub citation: Citation,
}

/// Per-species expected behavior bundle. One per shipped species.
///
/// Add a new species by creating one of these and registering it in
/// [`for_species_id`]. The harness will pick it up automatically.
#[derive(Debug, Clone)]
pub struct SpeciesExpectations {
    pub species_id: &'static str,
    /// Cross-link to the canonical PhD-level doc that explains all this.
    pub doc_path: &'static str,
    /// Canonical citation list summary — for the report header.
    pub key_sources: &'static [&'static str],
    /// What the sim should achieve.
    pub year_5_workers: ExpectedRange,
    pub year_5_brood_present: ExpectedRange,
    pub year_5_food_returned_per_year: ExpectedRange,
    pub queen_alive_at_year_5: ExpectedRange,
    /// Time (in-game days) from sim start to first egg laid post-warmup.
    /// For most claustral species this is 1-7 days after warming.
    pub days_to_first_egg: ExpectedRange,
}

/// Look up expectations by species id. Returns `None` if the species has
/// no expected-range table yet — the harness will then mark it as
/// "unverified" rather than failing.
pub fn for_species_id(id: &str) -> Option<SpeciesExpectations> {
    match id {
        "lasius_niger" => Some(lasius_niger()),
        "camponotus_pennsylvanicus" => Some(camponotus_pennsylvanicus()),
        "formica_rufa" => Some(formica_rufa()),
        "pogonomyrmex_occidentalis" => Some(pogonomyrmex_occidentalis()),
        "tetramorium_immigrans" => Some(tetramorium_immigrans()),
        "tapinoma_sessile" => Some(tapinoma_sessile()),
        "aphaenogaster_rudis" => Some(aphaenogaster_rudis()),
        _ => None,
    }
}

// ============================================================
// Per-species expected ranges. Order matches assets/species/.
// ============================================================

fn lasius_niger() -> SpeciesExpectations {
    SpeciesExpectations {
        species_id: "lasius_niger",
        doc_path: "docs/species/lasius_niger.md",
        key_sources: &[
            "Kutter & Stumper 1969 (queen lifespan record)",
            "Keller & Genoud 1997 Nature (longevity review)",
            "Beckers et al. 1992 (trail pheromone half-life)",
            "Khuong et al. 2016 PNAS (stigmergic nest construction)",
            "AntWiki Lasius niger",
        ],
        year_5_workers: ExpectedRange {
            human_name: "Workers alive at end of year 5",
            human_why: "A mature L. niger colony reaches 5,000-15,000 workers; \
                       year 5 is past the slow founding phase but not yet at peak. \
                       2,000-8,000 is the typical mid-life census.",
            centroid: 5000.0,
            tolerance: Tolerance::Loose,
            citation: Citation::InternalDoc(
                "docs/species/lasius_niger.md §3 (mature colony 5,000-15,000)",
            ),
        },
        year_5_brood_present: ExpectedRange {
            human_name: "Brood items present at end of year 5 (eggs+larvae+pupae)",
            human_why: "A healthy queen-led colony in spring should always have \
                       all three brood stages populated. Zero brood at year 5 means \
                       the queen has died or the colony has collapsed.",
            centroid: 200.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing(
                "no published per-stage census; sim expects non-zero across all 3 stages",
            ),
        },
        year_5_food_returned_per_year: ExpectedRange {
            human_name: "Food units returned to nest per simulated year (year 5)",
            human_why: "Net positive food economy is required for sustainability. \
                       Exact value depends on map layout, so we only check the \
                       ballpark order of magnitude.",
            centroid: 5000.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing(
                "abstract sim food units; check is order-of-magnitude only",
            ),
        },
        queen_alive_at_year_5: ExpectedRange {
            human_name: "Queen still alive at end of year 5",
            human_why: "L. niger queens routinely live 20+ years (record 28y 8mo). \
                       A queen dead at year 5 is a sim bug or extreme bad luck.",
            centroid: 1.0,
            tolerance: Tolerance::Strict,
            citation: Citation::PeerReviewed(
                "Keller & Genoud 1997, Nature 389:958-960 (eusocial insect longevity)",
            ),
        },
        days_to_first_egg: ExpectedRange {
            human_name: "In-game days from start to first egg laid (after warmup)",
            human_why: "A claustral queen begins laying within the first week of \
                       founding once warm. Sim starts post-nanitic so this should \
                       be near-zero.",
            centroid: 3.0,
            tolerance: Tolerance::Loose,
            citation: Citation::ReferenceWork(
                "Hölldobler & Wilson 1990, The Ants, ch.5 (claustral founding)",
            ),
        },
    }
}

fn camponotus_pennsylvanicus() -> SpeciesExpectations {
    SpeciesExpectations {
        species_id: "camponotus_pennsylvanicus",
        doc_path: "docs/species/camponotus_pennsylvanicus.md",
        key_sources: &[
            "Hansen & Klotz 2005 Carpenter Ants of US & Canada (canonical)",
            "Pricer 1908 (founding rate)",
            "Sanders 1972 (foraging behavior)",
            "Klotz & Reid 1993 (nocturnal activity)",
            "Traniello 1977 (recruitment trail chemistry)",
        ],
        year_5_workers: ExpectedRange {
            human_name: "Workers alive at end of year 5",
            human_why: "Camponotus is famously slow. Year 5 colonies are typically \
                       350-1,500 workers; majors first appear around years 6-10. \
                       A year-5 sim run should NOT show mature colony numbers.",
            centroid: 800.0,
            tolerance: Tolerance::Loose,
            citation: Citation::ReferenceWork(
                "Hansen & Klotz 2005 (yr1 5-25, yr2 40-200, yr3 350-1500)",
            ),
        },
        year_5_brood_present: ExpectedRange {
            human_name: "Brood items present at end of year 5",
            human_why: "Even a small Camponotus colony maintains brood year-round \
                       except during deep diapause.",
            centroid: 50.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("no published per-stage census"),
        },
        year_5_food_returned_per_year: ExpectedRange {
            human_name: "Food units returned to nest per simulated year (year 5)",
            human_why: "Camponotus foraging is individual + tandem (not mass), so \
                       trail-throughput is intrinsically lower than Lasius. Same \
                       order-of-magnitude check applies.",
            centroid: 1500.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("abstract sim food units"),
        },
        queen_alive_at_year_5: ExpectedRange {
            human_name: "Queen still alive at end of year 5",
            human_why: "Camponotus queens live 15-25 years. Year-5 mortality is bug-territory.",
            centroid: 1.0,
            tolerance: Tolerance::Strict,
            citation: Citation::ReferenceWork(
                "Hansen & Klotz 2005 (queen lifespan 15-25yr)",
            ),
        },
        days_to_first_egg: ExpectedRange {
            human_name: "In-game days from start to first egg laid",
            human_why: "Claustral founding; first egg within days of warming.",
            centroid: 7.0,
            tolerance: Tolerance::Loose,
            citation: Citation::PeerReviewed("Pricer 1908 (founding observations)"),
        },
    }
}

fn formica_rufa() -> SpeciesExpectations {
    SpeciesExpectations {
        species_id: "formica_rufa",
        doc_path: "docs/species/formica_rufa.md",
        key_sources: &[
            "Gösswald 1989-1990 (Formica rufa monograph)",
            "Hölldobler & Wilson 1990 The Ants",
            "Stockan & Robinson 2016 Cambridge (wood ants vol)",
            "Borowiec et al. 2021 PNAS (phylogenomics)",
            "Kadochová & Frouz 2015 (mound thermoregulation)",
        ],
        year_5_workers: ExpectedRange {
            human_name: "Workers alive at end of year 5",
            human_why: "Formica rufa colonies are slow to establish (parasitic founding); \
                       a year-5 colony is on the way to but not yet at the 100k-400k mature size. \
                       NOTE: until parasitic founding is implemented (Phase C), this species \
                       falls back to claustral and may grow faster than literature.",
            centroid: 20000.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::LiteratureRange {
                low: "Stockan & Robinson 2016 (yr1-2 minimal)",
                high: "Gösswald 1989 (mature 100k-400k)",
            },
        },
        year_5_brood_present: ExpectedRange {
            human_name: "Brood items present at end of year 5",
            human_why: "Polygyne supercolonies maintain large brood pools year-round.",
            centroid: 1000.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("no published per-stage census"),
        },
        year_5_food_returned_per_year: ExpectedRange {
            human_name: "Food units returned to nest per simulated year (year 5)",
            human_why: "Mature mounds eat ~100,000 insects/day during summer plus \
                       honeydew tonnage. Sim food units are abstract.",
            centroid: 50000.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("abstract sim food units"),
        },
        queen_alive_at_year_5: ExpectedRange {
            human_name: "Queen still alive at end of year 5",
            human_why: "Polygyne — multiple queens; \"alive\" here means at least one queen alive.",
            centroid: 1.0,
            tolerance: Tolerance::Strict,
            citation: Citation::InternalDoc("docs/species/formica_rufa.md §3 (polygyny)"),
        },
        days_to_first_egg: ExpectedRange {
            human_name: "In-game days from start to first egg laid",
            human_why: "Sim starts post-founding; egg flow begins immediately on warming.",
            centroid: 5.0,
            tolerance: Tolerance::Loose,
            citation: Citation::GamePacing("sim starts mid-spring with established queen"),
        },
    }
}

fn pogonomyrmex_occidentalis() -> SpeciesExpectations {
    SpeciesExpectations {
        species_id: "pogonomyrmex_occidentalis",
        doc_path: "docs/species/pogonomyrmex_occidentalis.md",
        key_sources: &[
            "Wiernasz & Cole 2003, Behavioral Ecology 14:43-50 (queen mating + fitness)",
            "Keller & Genoud 1997, Nature 389:958-960 (eusocial insect longevity, includes Pogonomyrmex)",
            "MacMahon, Mull & Crist 2000, Annual Review Ecol. Syst. 31:265-291 (community ecology)",
            "Lavigne 1969, Annals Entom. Soc. America 62:1166-1175 (nest architecture)",
            "Schmidt 1990, Hymenoptera Defensive Behavior (sting pain index)",
        ],
        year_5_workers: ExpectedRange {
            human_name: "Workers alive at end of year 5",
            human_why: "Slow growers; year 5 colonies are 1,000-3,000 workers, well \
                       below the 6,000-12,000 mature target.",
            centroid: 2000.0,
            tolerance: Tolerance::Loose,
            citation: Citation::InternalDoc(
                "docs/species/pogonomyrmex_occidentalis.md §3 (Cole/Wiernasz Idaho census; specific year/journal still pending verification by an ecologist)",
            ),
        },
        year_5_brood_present: ExpectedRange {
            human_name: "Brood items present at end of year 5",
            human_why: "Granivore — brood production peaks in monsoon season.",
            centroid: 100.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("no published per-stage census"),
        },
        year_5_food_returned_per_year: ExpectedRange {
            human_name: "Food units returned to nest per simulated year (year 5)",
            human_why: "Granivore food return is seed-cache replenishment; absolute \
                       value depends on what we count as a 'unit'.",
            centroid: 3000.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("abstract sim food units"),
        },
        queen_alive_at_year_5: ExpectedRange {
            human_name: "Queen still alive at end of year 5",
            human_why: "Cole & Wiernasz 2025: 5-40yr range, mean ~13yr. \
                       Year-5 mortality is uncommon but not impossible (lower tail).",
            centroid: 1.0,
            tolerance: Tolerance::Strict,
            citation: Citation::PeerReviewed(
                "Keller & Genoud 1997, Nature 389:958-960 (Pogonomyrmex queens documented to ~30 yr); see docs/species/pogonomyrmex_occidentalis.md for the 5-40 yr range source attributed to Cole/Wiernasz Idaho census (specific paper pending an ecologist's verification)",
            ),
        },
        days_to_first_egg: ExpectedRange {
            human_name: "In-game days from start to first egg laid",
            human_why: "Claustral founding; egg flow begins on warming.",
            centroid: 5.0,
            tolerance: Tolerance::Loose,
            citation: Citation::GamePacing("sim starts post-nanitic"),
        },
    }
}

fn tetramorium_immigrans() -> SpeciesExpectations {
    SpeciesExpectations {
        species_id: "tetramorium_immigrans",
        doc_path: "docs/species/tetramorium_immigrans.md",
        key_sources: &[
            "Wagner et al. 2017 Myrmecological News 25:95-129 (species delimitation)",
            "Hoover et al. 2016 Current Zoology (battle monoamine clock)",
            "Plowes et al. (territorial battle dynamics)",
            "UF/IFAS EENY-600 (life history)",
        ],
        year_5_workers: ExpectedRange {
            human_name: "Workers alive at end of year 5",
            human_why: "Tetramorium grows fast — year 5 colonies are typically at or \
                       near the mature 3,000-10,000 worker target.",
            centroid: 8000.0,
            tolerance: Tolerance::Loose,
            citation: Citation::Extension("UF/IFAS EENY-600 (3K-10K+ workers monogyne)"),
        },
        year_5_brood_present: ExpectedRange {
            human_name: "Brood items present at end of year 5",
            human_why: "Year-round laying in warm regions; large brood pool expected.",
            centroid: 500.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("no published per-stage census"),
        },
        year_5_food_returned_per_year: ExpectedRange {
            human_name: "Food units returned to nest per simulated year (year 5)",
            human_why: "Strong mass recruiter; high food throughput expected.",
            centroid: 10000.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("abstract sim food units"),
        },
        queen_alive_at_year_5: ExpectedRange {
            human_name: "Queen still alive at end of year 5",
            human_why: "Captive queens documented to ~15 years; year-5 mortality is bug-territory.",
            centroid: 1.0,
            tolerance: Tolerance::Strict,
            citation: Citation::TaxonomicDatabase("AntWiki Tetramorium immigrans"),
        },
        days_to_first_egg: ExpectedRange {
            human_name: "In-game days from start to first egg laid",
            human_why: "Semi-claustral; queens forage during founding so eggs flow quickly.",
            centroid: 3.0,
            tolerance: Tolerance::Loose,
            citation: Citation::Extension("UF/IFAS EENY-600 (semi-claustral)"),
        },
    }
}

fn tapinoma_sessile() -> SpeciesExpectations {
    SpeciesExpectations {
        species_id: "tapinoma_sessile",
        doc_path: "docs/species/tapinoma_sessile.md",
        key_sources: &[
            "Buczkowski 2010, Biological Invasions 12:3343-3349 (forest ~100 / urban ~20,000+ supercolony plasticity)",
            "Buczkowski & Bennett 2008, Ecological Entomology 33:780-788 (polydomy + nest relocation)",
            "Smith 1928 (classic small-colony observations)",
            "Tomalski et al. 1987 (alarm chemistry — characterized in T. simrothi, applied to genus)",
            "AntWiki Tapinoma sessile",
        ],
        year_5_workers: ExpectedRange {
            human_name: "Workers alive at end of year 5",
            human_why: "Wildly population-structure-dependent. Forest colonies stay \
                       small (~100); urban supercolonies reach 20,000+. Default sim \
                       run is forest mode, hence the lower centroid.",
            centroid: 5000.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::PeerReviewed(
                "Buczkowski 2010, Biological Invasions 12:3343-3349 (forest ~100 / urban ~20,000+ population plasticity)",
            ),
        },
        year_5_brood_present: ExpectedRange {
            human_name: "Brood items present at end of year 5",
            human_why: "Fast brood cycle (egg-to-adult 34-79 days at 21°C, AntWiki).",
            centroid: 300.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::TaxonomicDatabase("AntWiki Tapinoma sessile"),
        },
        year_5_food_returned_per_year: ExpectedRange {
            human_name: "Food units returned to nest per simulated year (year 5)",
            human_why: "Sweet-loving mass recruiter; high throughput on sugary food.",
            centroid: 8000.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("abstract sim food units"),
        },
        queen_alive_at_year_5: ExpectedRange {
            human_name: "At least one queen alive at end of year 5 (polygyne)",
            human_why: "Polygyne; supercolonies have many queens. \"Alive\" = ≥1 queen.",
            centroid: 1.0,
            tolerance: Tolerance::Strict,
            citation: Citation::InternalDoc("docs/species/tapinoma_sessile.md §3"),
        },
        days_to_first_egg: ExpectedRange {
            human_name: "In-game days from start to first egg laid",
            human_why: "Polygyne founding — egg flow is essentially immediate.",
            centroid: 2.0,
            tolerance: Tolerance::Loose,
            citation: Citation::GamePacing("polygyne, no founding bottleneck"),
        },
    }
}

fn aphaenogaster_rudis() -> SpeciesExpectations {
    SpeciesExpectations {
        species_id: "aphaenogaster_rudis",
        doc_path: "docs/species/aphaenogaster_rudis.md",
        key_sources: &[
            "Lubertazzi 2012 (sociometry, mean 266-613 workers)",
            "Beattie & Culver 1981 (myrmecochory keystone role)",
            "Ness et al. 2009 (seed dispersal ecology)",
            "Warren et al. 2010s (invasive displacement)",
            "Haskins (queen longevity)",
        ],
        year_5_workers: ExpectedRange {
            human_name: "Workers alive at end of year 5",
            human_why: "Small colonies — Lubertazzi 2012 mean 266-613 workers, max ~2,000. \
                       Year 5 should be near or at mature size.",
            centroid: 500.0,
            tolerance: Tolerance::Loose,
            citation: Citation::PeerReviewed(
                "Lubertazzi 2012 (mean 266-613, max ~2000)",
            ),
        },
        year_5_brood_present: ExpectedRange {
            human_name: "Brood items present at end of year 5",
            human_why: "Small forest colony — modest brood pool.",
            centroid: 50.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("no published per-stage census"),
        },
        year_5_food_returned_per_year: ExpectedRange {
            human_name: "Food units returned to nest per simulated year (year 5)",
            human_why: "Small colony, modest forager corps.",
            centroid: 1500.0,
            tolerance: Tolerance::OrderOfMagnitude,
            citation: Citation::GamePacing("abstract sim food units"),
        },
        queen_alive_at_year_5: ExpectedRange {
            human_name: "Queen still alive at end of year 5",
            human_why: "Haskins documented up to 13yr in this complex; year-5 mortality is bug-territory.",
            centroid: 1.0,
            tolerance: Tolerance::Strict,
            citation: Citation::InternalDoc("docs/species/aphaenogaster_rudis.md §4 (Haskins)"),
        },
        days_to_first_egg: ExpectedRange {
            human_name: "In-game days from start to first egg laid",
            human_why: "Claustral founding; egg flow begins on warming.",
            centroid: 5.0,
            tolerance: Tolerance::Loose,
            citation: Citation::GamePacing("sim starts post-nanitic"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_seven_species_have_expectations() {
        for id in [
            "lasius_niger",
            "camponotus_pennsylvanicus",
            "formica_rufa",
            "pogonomyrmex_occidentalis",
            "tetramorium_immigrans",
            "tapinoma_sessile",
            "aphaenogaster_rudis",
        ] {
            assert!(
                for_species_id(id).is_some(),
                "missing expectations for species: {id}",
            );
        }
    }

    #[test]
    fn unknown_species_returns_none() {
        assert!(for_species_id("nonexistent_species").is_none());
    }

    #[test]
    fn every_expectation_has_human_name_and_why() {
        for id in [
            "lasius_niger",
            "camponotus_pennsylvanicus",
            "formica_rufa",
            "pogonomyrmex_occidentalis",
            "tetramorium_immigrans",
            "tapinoma_sessile",
            "aphaenogaster_rudis",
        ] {
            let exp = for_species_id(id).unwrap();
            for er in [
                &exp.year_5_workers,
                &exp.year_5_brood_present,
                &exp.year_5_food_returned_per_year,
                &exp.queen_alive_at_year_5,
                &exp.days_to_first_egg,
            ] {
                assert!(
                    !er.human_name.is_empty(),
                    "{id}: empty human_name",
                );
                assert!(
                    !er.human_why.is_empty(),
                    "{id}: empty human_why for {}",
                    er.human_name,
                );
            }
        }
    }

    #[test]
    fn tolerance_band_is_well_formed() {
        for tol in [Tolerance::Strict, Tolerance::Loose, Tolerance::OrderOfMagnitude] {
            let (lo, hi) = tol.band();
            assert!(lo > 0.0 && lo < hi, "tolerance band malformed: {lo}..{hi}");
        }
    }
}

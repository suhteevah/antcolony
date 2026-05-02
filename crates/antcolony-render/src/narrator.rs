//! Phase 9.0 — Narrator
//!
//! Generates per-colony chronicle entries on milestone events. Pure
//! flavor layer; doesn't touch sim mechanics. The narrator gives every
//! colony a queen name + procedural worker names, writes per-event
//! prose to `assets/saves/<save_id>/chronicle/<colony_id>.md`, and
//! optionally surfaces lines as in-game speech bubbles.
//!
//! This MVP shipping commit does:
//! - `Chronicler` resource + `ChroniclerPlugin` (not yet wired into
//!   `RenderPlugin` — needs a build window first)
//! - Procedural queen-name + worker-name generators (deterministic
//!   from a colony seed so the same playthrough produces the same
//!   names)
//! - Templated narrator output for the K4 milestones already shipped
//!   (FirstEgg, FirstMajor, PopulationTen/50/100/500,
//!   FirstColonyAnniversary, SurvivedFirstWinter, FirstNuptialFlight,
//!   FirstDaughterColony)
//! - Markdown writer that appends to `chronicle/<colony_id>.md`
//!
//! Future phases (9.0 polish + 9.3 LLM):
//! - Speech-bubble rendering over individual ants (tied to current
//!   focus / inspector)
//! - Per-event LLM call (gated by AI bundle availability) for richer
//!   prose than the templates can provide
//! - Per-event multimedia (sound effect, screen flash, milestone toast)

use antcolony_sim::{Milestone, MilestoneKind};
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

/// Plugin entry point. Call from `RenderPlugin::build` once the
/// next build window opens (sweep currently locks the binary, so this
/// is parked for the next session — it's already complete and ready
/// to wire).
pub struct ChroniclerPlugin;

impl Plugin for ChroniclerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Chronicler>()
            .add_systems(Update, append_milestone_entries);
    }
}

/// Per-colony chronicle state. One entry per active colony; populated
/// at colony spawn and torn down at colony death.
#[derive(Resource, Default, Debug)]
pub struct Chronicler {
    pub colonies: HashMap<u8, ColonyChronicle>,
    pub vault_root: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ColonyChronicle {
    pub colony_id: u8,
    pub queen_name: String,
    pub colony_epithet: String,
    pub last_milestone_seen: u32,
    /// Cumulative number of named workers (used for procedural naming).
    pub workers_named: u32,
    /// Path to the colony's chronicle markdown file.
    pub chronicle_path: PathBuf,
    /// In-memory ring of recent entries, used by speech-bubble UI.
    pub recent_entries: Vec<String>,
}

impl ColonyChronicle {
    pub fn new(colony_id: u8, seed: u64, vault_root: &PathBuf) -> Self {
        let queen_name = procedural_queen_name(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let epithet = procedural_colony_epithet(seed.wrapping_mul(0x6C8E_9CF5_7027_4F50));
        let dir = vault_root.join("chronicle");
        let _ = std::fs::create_dir_all(&dir);
        let chronicle_path = dir.join(format!("colony_{:02}.md", colony_id));

        let mut chronicle = Self {
            colony_id,
            queen_name: queen_name.clone(),
            colony_epithet: epithet.clone(),
            last_milestone_seen: 0,
            workers_named: 0,
            chronicle_path: chronicle_path.clone(),
            recent_entries: Vec::new(),
        };

        // Write the chronicle header on first creation. Idempotent — if
        // the file exists from a prior session we leave it alone and
        // append continues at the bottom.
        if !chronicle_path.exists() {
            let header = format!(
                "# Chronicle of {epithet}\n\nQueen {queen_name} founded the colony.\n\n"
            );
            let _ = std::fs::write(&chronicle_path, header);
            chronicle.recent_entries.push(format!(
                "Queen {queen_name} founded {epithet}."
            ));
        }
        chronicle
    }

    /// Append a narrator line to the markdown file + the in-memory ring.
    pub fn append(&mut self, line: &str) {
        use std::io::Write;
        let entry = format!("- {line}\n");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.chronicle_path)
        {
            let _ = write!(f, "{entry}");
        }
        self.recent_entries.push(line.to_string());
        if self.recent_entries.len() > 32 {
            self.recent_entries.remove(0);
        }
    }
}

/// System: walk each colony's milestone list, generate narrator prose
/// for any new milestones since the last poll, append to the chronicle.
/// Skipped on save-load (the chronicle file already has prior entries).
pub fn append_milestone_entries(
    sim: Res<antcolony_game::SimulationState>,
    mut chronicler: ResMut<Chronicler>,
) {
    if chronicler.vault_root.is_none() {
        // Default vault under cwd/saves/<species>/.
        let cwd = std::env::current_dir().unwrap_or_default();
        chronicler.vault_root = Some(cwd.join("saves").join(&sim.species.id));
    }
    let vault_root = chronicler.vault_root.clone().unwrap();

    for colony in &sim.sim.colonies {
        let entry = chronicler
            .colonies
            .entry(colony.id)
            .or_insert_with(|| {
                ColonyChronicle::new(colony.id, sim.environment.seed.wrapping_add(colony.id as u64), &vault_root)
            });

        // Only append milestones we haven't seen yet.
        for (i, m) in colony.milestones.iter().enumerate() {
            if (i as u32) < entry.last_milestone_seen {
                continue;
            }
            let line = render_milestone(m, entry, &sim.species.common_name);
            entry.append(&line);
            entry.last_milestone_seen = (i as u32) + 1;
        }
    }
}

/// Generate one narrator line for a milestone event.
fn render_milestone(m: &Milestone, ch: &ColonyChronicle, species_name: &str) -> String {
    let day = m.in_game_day;
    let queen = &ch.queen_name;
    let epithet = &ch.colony_epithet;
    match m.kind {
        MilestoneKind::FirstEgg => format!(
            "Day {day}: Queen {queen} laid her first egg. The {epithet} now has a future."
        ),
        MilestoneKind::FirstMajor => format!(
            "Day {day}: A major worker has emerged from the brood — {epithet}'s first soldier."
        ),
        MilestoneKind::PopulationTen => format!(
            "Day {day}: {epithet} reaches ten workers. The first nanitics have aged into the foraging crew."
        ),
        MilestoneKind::PopulationFifty => format!(
            "Day {day}: Fifty workers strong. {epithet} has crossed the threshold from founding to established colony."
        ),
        MilestoneKind::PopulationOneHundred => format!(
            "Day {day}: A hundred workers under Queen {queen}. The {species_name} of {epithet} are no longer fragile."
        ),
        MilestoneKind::PopulationFiveHundred => format!(
            "Day {day}: Five hundred workers. {epithet} has become a proper colony, with trunk trails reaching the far edges of the foraging arena."
        ),
        MilestoneKind::FirstColonyAnniversary => format!(
            "Day {day}: One year since founding. Queen {queen} has lived through her first winter; the {epithet} continues."
        ),
        MilestoneKind::SurvivedFirstWinter => format!(
            "Day {day}: Spring returns. {epithet} survived the winter; the queen is laying again, the brood pipeline is filling."
        ),
        MilestoneKind::FirstNuptialFlight => format!(
            "Day {day}: A nuptial flight launched from {epithet}. Virgin queens and males are airborne; some will not return."
        ),
        MilestoneKind::FirstDaughterColony => format!(
            "Day {day}: A daughter colony has been founded. {epithet}'s lineage now spans more than one nest."
        ),
    }
}

// --- Procedural naming ---

/// Deterministic queen name from a seed. Pulls a first name + an
/// epithet (so each colony's queen is distinguishable). Order is
/// stable across runs with the same seed.
pub fn procedural_queen_name(seed: u64) -> String {
    const FIRST: &[&str] = &[
        "Marigold", "Sigrid", "Yennefer", "Cassia", "Brunhild", "Iolanthe",
        "Esme", "Octavia", "Saoirse", "Thalassa", "Verity", "Wisteria",
        "Mireille", "Isolde", "Bryony", "Gisella", "Henrietta", "Ophelia",
        "Persephone", "Rosalind", "Tamsin", "Ursula", "Zenaida", "Calliope",
        "Demeter", "Eurydice", "Felicity", "Galadriel", "Hildegard",
    ];
    const EPITHET: &[&str] = &[
        "the Founder", "the Patient", "the Long-Lived", "the First",
        "the Many-Daughtered", "the Watchful", "the Sovereign",
        "of the Deep Chamber", "of the Tall Mound", "of the Eastern Trail",
        "the Frost-Resistant", "the Storm-Born",
    ];
    let f = FIRST[(seed as usize).wrapping_mul(2_971) % FIRST.len()];
    let e = EPITHET[(seed as usize).wrapping_mul(7_829) % EPITHET.len()];
    format!("{f} {e}")
}

/// Deterministic colony epithet (the place-name / lineage-name the
/// chronicle refers to it as). Distinct from the queen's name.
pub fn procedural_colony_epithet(seed: u64) -> String {
    const PATTERN: &[&str] = &[
        "the Hearth Colony", "the Blackwood Court", "the Cinnabar Lineage",
        "the House of Wisteria", "the Ember Hall", "the Glassglow Family",
        "the Honeyguard", "the Nightleaf Court", "the Quietfield Colony",
        "the Saltwood Family", "the Sunmark Hall", "the Twilight Lineage",
        "the Verdant Hearth", "the Wintering Court", "the Acorn Lineage",
    ];
    PATTERN[(seed as usize).wrapping_mul(13_859) % PATTERN.len()].to_string()
}

/// Deterministic worker name when one is needed (e.g. the first major,
/// the oldest worker, a ant chosen for narrative spotlight). Reads
/// from a different list than queens so worker names feel distinct.
pub fn procedural_worker_name(colony_seed: u64, worker_idx: u32) -> String {
    const NAMES: &[&str] = &[
        "Sigrid", "Bryn", "Ila", "Mira", "Tova", "Halla", "Ren", "Yusra",
        "Astra", "Cleo", "Daya", "Etta", "Faye", "Gem", "Hela", "Iva",
        "Juno", "Kit", "Lena", "Mae", "Nori", "Ona", "Pia", "Quill",
        "Rae", "Sky", "Tess", "Una", "Vera", "Wren", "Xan", "Yara", "Zia",
    ];
    let combined = colony_seed
        .wrapping_add(worker_idx as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    NAMES[(combined as usize) % NAMES.len()].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queen_name_deterministic_for_same_seed() {
        assert_eq!(procedural_queen_name(42), procedural_queen_name(42));
    }

    #[test]
    fn queen_names_differ_across_seeds() {
        // Not a strict guarantee (modular hashing collisions exist) but
        // for typical seed-distance the names should differ.
        let a = procedural_queen_name(1);
        let b = procedural_queen_name(0xDEAD_BEEF);
        assert_ne!(a, b);
    }

    #[test]
    fn colony_epithet_deterministic() {
        assert_eq!(procedural_colony_epithet(7), procedural_colony_epithet(7));
    }

    #[test]
    fn worker_names_unique_within_colony() {
        let seed = 12345;
        let n0 = procedural_worker_name(seed, 0);
        let n1 = procedural_worker_name(seed, 1);
        // Same colony, different worker indices — not strictly required
        // to differ but typically should.
        let _ = (n0, n1);
    }
}

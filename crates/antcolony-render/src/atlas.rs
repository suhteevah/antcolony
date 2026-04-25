//! Sprite atlas: per-species pixel-art sprites under
//! `assets/gen/<species_id>/design/<caste>_96.png`.
//!
//! When an atlas entry exists for an ant's (species, caste) pair, the
//! renderer spawns a single textured `Sprite` instead of the procedural
//! head/thorax/gaster/legs primitives in `spawn_ant_parts`. The legs and
//! antennae are baked into the image, so leg animation is suppressed for
//! atlas-rendered ants — the trade-off for the much higher art quality.
//!
//! Falls back silently if a sprite isn't on disk: missing castes get the
//! procedural fallback. This means partial sprite packs are usable; you
//! don't have to ship every caste at once.

use crate::AppState;
use antcolony_game::SimulationState;
use antcolony_sim::AntCaste;
use bevy::prelude::*;
use std::collections::HashMap;

pub struct SpriteAtlasPlugin;

impl Plugin for SpriteAtlasPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpriteAtlas>()
            .add_systems(OnEnter(AppState::Running), load_atlas_for_species);
    }
}

/// Per-(species, caste) sprite handles. Empty until `load_atlas_for_species`
/// runs at `OnEnter(AppState::Running)`. Reset between species switches —
/// we clear and reload, so handle counts stay bounded.
#[derive(Resource, Default, Debug)]
pub struct SpriteAtlas {
    pub sprites: HashMap<(String, AntCaste), Handle<Image>>,
    pub species_id: Option<String>,
}

impl SpriteAtlas {
    pub fn lookup(&self, species_id: &str, caste: AntCaste) -> Option<&Handle<Image>> {
        self.sprites.get(&(species_id.to_string(), caste))
    }

    pub fn has_any(&self) -> bool {
        !self.sprites.is_empty()
    }
}

/// Map AntCaste → filename stem. We try the caste's primary stem first,
/// then a fallback (e.g. Soldier falls back to Worker if no `worker_major`
/// sprite is shipped for this species).
fn caste_filename_candidates(caste: AntCaste) -> &'static [&'static str] {
    match caste {
        AntCaste::Worker => &["worker", "worker_minor"],
        AntCaste::Soldier => &["worker_major", "worker"],
        AntCaste::Breeder => &["queen_alate", "worker"],
        AntCaste::Queen => &["queen_dealate", "queen_alate"],
    }
}

/// Load 96x96 atlas sprites for the active species' castes. Reads the
/// `assets/gen/<species_id>/design/` directory directly — Bevy's
/// `AssetServer` would also work, but file-existence checks are cleaner
/// against the filesystem since we want to know up front which castes
/// actually have art to fall back gracefully.
fn load_atlas_for_species(
    sim: Res<SimulationState>,
    asset_server: Res<AssetServer>,
    mut atlas: ResMut<SpriteAtlas>,
) {
    let species_id = sim.species.id.clone();

    // Reset atlas for the new species. Bevy will drop the old handles.
    atlas.sprites.clear();
    atlas.species_id = Some(species_id.clone());

    // Disk path: assets/gen/<species_id>/design/. AssetServer.load() takes
    // a path relative to `assets/`, so we use "gen/<id>/design/<file>".
    // For the disk-existence check we need the full path.
    let assets_root = std::env::current_dir().unwrap_or_default().join("assets");
    let design_dir = assets_root.join("gen").join(&species_id).join("design");

    if !design_dir.exists() {
        tracing::info!(
            species = %species_id,
            dir = %design_dir.display(),
            "no sprite atlas on disk; renderer will use procedural ants"
        );
        return;
    }

    let mut loaded = 0usize;
    for caste in [
        AntCaste::Worker,
        AntCaste::Soldier,
        AntCaste::Breeder,
        AntCaste::Queen,
    ] {
        for stem in caste_filename_candidates(caste) {
            let filename = format!("{stem}_96.png");
            if design_dir.join(&filename).exists() {
                let asset_path = format!("gen/{species_id}/design/{filename}");
                let handle: Handle<Image> = asset_server.load(asset_path);
                atlas
                    .sprites
                    .insert((species_id.clone(), caste), handle);
                loaded += 1;
                break;
            }
        }
    }

    // Optional: also load the carrying-pose variants. We don't wire them
    // into rendering yet, but pre-loading the handles means the texture is
    // hot in the asset cache the moment we add the carry-state hook.
    let bonus = ["worker_carrying_seed_96.png", "worker_carrying_larva_96.png"];
    let bonus_count = bonus
        .iter()
        .filter(|f| design_dir.join(f).exists())
        .count();

    tracing::info!(
        species = %species_id,
        castes_loaded = loaded,
        bonus_poses_on_disk = bonus_count,
        "sprite atlas loaded"
    );
}

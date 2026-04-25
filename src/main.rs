//! antcolony binary entry point.
//!
//! Wires the simulation + Bevy integration + rendering plugins together.
//! The render plugin owns the picker-then-running AppState flow; no
//! simulation is created until the player chooses a species.

use bevy::asset::AssetPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;

fn main() -> anyhow::Result<()> {
    // Resolve the assets directory once, up front. Bevy's default behavior
    // for `AssetPlugin::file_path` is "relative to the executable" — which
    // means a release build in `target/release/` looks for assets at
    // `target/release/assets/`. That breaks `cargo run --release` and the
    // distributed exe alike whenever assets live at the workspace root.
    //
    // Strategy: probe a short list of candidate roots (cwd, exe-dir parents)
    // and use the first one that contains `assets/`. This handles both
    // `cargo run` (cwd=workspace root) and a distributed release exe placed
    // next to a copied `assets/` dir.
    let asset_root = resolve_asset_root().unwrap_or_else(|| "assets".into());
    // Pre-Bevy: tracing has no subscriber yet, so use eprintln so the
    // resolved path is visible in the boot log either way.
    eprintln!("[antcolony] resolved asset_root = {asset_root}");
    // Stash for downstream code (atlas existence checks, save UI, etc).
    // SAFETY: set_var is unsafe in edition 2024; we set it once before any
    // threads are spawned.
    unsafe {
        std::env::set_var("ANTCOLONY_ASSET_ROOT", &asset_root);
    }

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "antcolony — ant colony simulation".into(),
                        resolution: (1280.0, 800.0).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root,
                    ..default()
                })
                .set(LogPlugin {
                    level: bevy::log::Level::INFO,
                    filter:
                        "wgpu=warn,naga=warn,antcolony=info,antcolony_sim=info,antcolony_game=info,antcolony_render=info"
                            .into(),
                    ..default()
                }),
        )
        .add_plugins(antcolony_game::SimulationPlugin::default())
        .add_plugins(antcolony_render::RenderPlugin)
        .run();

    Ok(())
}

/// Probe candidate roots for an `assets/` directory. Returns the first
/// candidate where `assets/species/lasius_niger.toml` exists — that's
/// our canary file (always shipped). Returns the path to the `assets`
/// folder itself (what `AssetPlugin::file_path` wants).
fn resolve_asset_root() -> Option<String> {
    use std::path::PathBuf;

    let canary = std::path::Path::new("species").join("lasius_niger.toml");

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("assets"));
    }
    if let Ok(exe) = std::env::current_exe() {
        // Walk up to 4 parents — handles target/release/, target/debug/,
        // and any reasonable installed layout with assets next to the exe.
        let mut p = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..4 {
            if let Some(dir) = &p {
                candidates.push(dir.join("assets"));
                p = dir.parent().map(|q| q.to_path_buf());
            }
        }
    }

    for cand in candidates {
        if cand.join(&canary).is_file() {
            return Some(cand.to_string_lossy().into_owned());
        }
    }
    None
}

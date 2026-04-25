//! K4: Ctrl+S save / Ctrl+L load + milestone banner UI.

use antcolony_game::SimulationState;
use antcolony_sim::{Simulation, Species, load_species_dir, load_snapshot,
    save_snapshot, compute_catchup_ticks, now_unix_secs};
use bevy::prelude::*;

use crate::AppState;

/// Default quicksave path. Relative to the process cwd.
pub const QUICKSAVE_PATH: &str = "saves/quicksave.json";

pub struct SaveUiPlugin;

impl Plugin for SaveUiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ToastState::default())
            .insert_resource(MilestoneTracker::default())
            .add_systems(OnEnter(AppState::Running), setup_save_ui)
            .add_systems(
                Update,
                (
                    handle_save_load_keys,
                    update_toast,
                    spawn_milestone_banner,
                    update_milestone_banner,
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

// --- Resources ---

#[derive(Resource, Default, Debug)]
pub struct ToastState {
    /// Message to display, empty when no toast visible.
    pub message: String,
    /// Seconds remaining before the toast disappears.
    pub ttl: f32,
    /// True = green (success), false = red (error).
    pub success: bool,
}

impl ToastState {
    pub fn show(&mut self, msg: impl Into<String>, success: bool, seconds: f32) {
        self.message = msg.into();
        self.ttl = seconds;
        self.success = success;
    }
}

#[derive(Resource, Default, Debug)]
struct MilestoneTracker {
    /// Per-colony last seen milestone count. Index = colony id (we assume
    /// colony ids are small and dense; resize as needed).
    seen_counts: Vec<usize>,
    /// Active banner (if any).
    active: Option<ActiveBanner>,
}

#[derive(Debug, Clone)]
struct ActiveBanner {
    text: String,
    ttl: f32,
}

// --- UI markers ---

#[derive(Component)]
struct ToastText;

#[derive(Component)]
struct MilestoneBannerText;

// --- Setup ---

fn setup_save_ui(mut commands: Commands) {
    // Toast pinned bottom-center.
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 1.0, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(60.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        ToastText,
    ));

    // Milestone banner pinned top-center below the season HUD.
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.85, 0.2)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        MilestoneBannerText,
    ));

    tracing::info!("SaveUiPlugin: save/load + milestone UI spawned");
}

// --- Systems ---

fn handle_save_load_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim_state: ResMut<SimulationState>,
    mut toast: ResMut<ToastState>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
        return;
    }

    if keys.just_pressed(KeyCode::KeyS) {
        let path = std::path::Path::new(QUICKSAVE_PATH);
        match save_snapshot(
            &sim_state.sim,
            &sim_state.species.id.clone(),
            &sim_state.environment.clone(),
            path,
        ) {
            Ok(()) => {
                tracing::info!("Ctrl+S save succeeded");
                toast.show("Saved", true, 2.0);
            }
            Err(e) => {
                tracing::error!(error = %e, "Ctrl+S save failed");
                toast.show(format!("Save failed: {}", e), false, 4.0);
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyL) {
        let path = std::path::Path::new(QUICKSAVE_PATH);
        match load_snapshot(path) {
            Ok(snap) => {
                // Try to resolve species from assets/species; fall back.
                let species_opt = resolve_species(&snap.species_id);
                let saved_at = snap.saved_at_unix_secs;
                let env = snap.environment.clone();
                let result = if let Some(species) = species_opt {
                    Simulation::from_snapshot(snap, |id| {
                        if id == species.id {
                            Some(species.clone())
                        } else {
                            None
                        }
                    })
                } else {
                    tracing::warn!(
                        "species not found in assets — loading snapshot with default cfg"
                    );
                    Simulation::from_snapshot(snap, |_| None)
                };
                match result {
                    Ok(mut new_sim) => {
                        let ticks = compute_catchup_ticks(
                            saved_at,
                            now_unix_secs(),
                            env.tick_rate_hz,
                        );
                        if ticks > 0 {
                            tracing::info!(ticks, "running offline catch-up");
                            new_sim.catch_up(ticks);
                        }
                        sim_state.sim = new_sim;
                        sim_state.environment = env;
                        toast.show(
                            format!("Loaded (catch-up: {} ticks)", ticks),
                            true,
                            3.0,
                        );
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "from_snapshot failed");
                        toast.show(format!("Load failed: {}", e), false, 4.0);
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Ctrl+L load failed");
                toast.show(format!("Load failed: {}", e), false, 4.0);
            }
        }
    }
}

fn resolve_species(id: &str) -> Option<Species> {
    // Resolve via ANTCOLONY_ASSET_ROOT (set in main.rs) so this works
    // regardless of cwd. Falls back to "assets/species" relative to cwd
    // for tooling that links the render crate without going through main.
    let species_dir = std::env::var("ANTCOLONY_ASSET_ROOT")
        .map(|root| format!("{root}/species"))
        .unwrap_or_else(|_| "assets/species".to_string());
    let path = std::path::Path::new(&species_dir);
    match load_species_dir(path) {
        Ok(list) => list.into_iter().find(|s| s.id == id),
        Err(e) => {
            tracing::warn!(error = %e, "load_species_dir failed");
            None
        }
    }
}

fn update_toast(
    time: Res<Time>,
    mut toast: ResMut<ToastState>,
    mut q: Query<(&mut Text, &mut TextColor), With<ToastText>>,
) {
    if toast.ttl > 0.0 {
        toast.ttl -= time.delta_secs();
        if toast.ttl < 0.0 {
            toast.ttl = 0.0;
            toast.message.clear();
        }
    }
    let color = if toast.success {
        Color::srgb(0.7, 1.0, 0.7)
    } else {
        Color::srgb(1.0, 0.6, 0.6)
    };
    for (mut t, mut c) in q.iter_mut() {
        **t = toast.message.clone();
        c.0 = color;
    }
}

fn spawn_milestone_banner(
    sim_state: Res<SimulationState>,
    mut tracker: ResMut<MilestoneTracker>,
) {
    // Grow the tracker as needed.
    let needed = sim_state.sim.colonies.len();
    if tracker.seen_counts.len() < needed {
        tracker.seen_counts.resize(needed, 0);
    }
    for (i, colony) in sim_state.sim.colonies.iter().enumerate() {
        let cur = colony.milestones.len();
        if cur > tracker.seen_counts[i] {
            // New milestone(s). Use the tail item.
            if let Some(tail) = colony.milestones.last() {
                let banner = format!("MILESTONE: {}!", tail.kind.label());
                tracing::info!(text = %banner, "milestone banner shown");
                tracker.active = Some(ActiveBanner {
                    text: banner,
                    ttl: 5.0,
                });
            }
            tracker.seen_counts[i] = cur;
        }
    }
}

fn update_milestone_banner(
    time: Res<Time>,
    mut tracker: ResMut<MilestoneTracker>,
    mut q: Query<&mut Text, With<MilestoneBannerText>>,
) {
    let (text, still_active) = match &mut tracker.active {
        Some(b) => {
            b.ttl -= time.delta_secs();
            if b.ttl <= 0.0 {
                (String::new(), false)
            } else {
                (b.text.clone(), true)
            }
        }
        None => (String::new(), false),
    };
    if !still_active {
        tracker.active = None;
    }
    for mut t in q.iter_mut() {
        **t = text.clone();
    }
}


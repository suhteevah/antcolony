//! Debug UI overlay: stats panel, triangle sliders for caste/behavior,
//! sim speed controls, keybinding help.
//!
//! The render crate owns this entirely — it only reads/writes public
//! fields of `ColonyState` that exist today (food_stored, food_returned,
//! queen_health, eggs, larvae, pupae, caste_ratio, behavior_weights,
//! population). Do NOT reference sim fields that may be added later
//! (e.g. `brood`).

use antcolony_game::SimulationState;
use antcolony_sim::AntCaste;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

use crate::AppState;

/// Plugin registering the debug UI.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .insert_resource(UiDragState::default())
            .insert_resource(SimPaused::default())
            .add_systems(OnEnter(AppState::Running), setup_ui)
            .add_systems(
                Update,
                (
                    update_stats_text,
                    handle_speed_keys,
                    triangle_drag_system,
                    update_triangle_handles,
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

// --- Markers ---

#[derive(Component)]
struct StatsText;

#[derive(Component)]
struct HelpText;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum TriangleKind {
    Caste,
    Behavior,
}

#[derive(Component)]
struct TrianglePanel {
    kind: TriangleKind,
    /// Screen-space triangle vertex positions in the UI node's local
    /// coordinates (0..TRI_SIZE). Order: corner0, corner1, corner2.
    verts: [Vec2; 3],
}

#[derive(Component)]
struct TriangleHandle {
    kind: TriangleKind,
}

// --- Resources ---

#[derive(Resource, Default, Debug)]
struct UiDragState {
    /// The slider currently being dragged, if any.
    active: Option<TriangleKind>,
}

#[derive(Resource, Default, Debug)]
pub struct SimPaused(pub bool);

// --- Constants ---

const TRI_SIZE: f32 = 160.0;
const HANDLE_RADIUS: f32 = 7.0;

/// Panel origin positions on screen (top-left corner of the panel, in px from
/// screen top-left). Caste panel goes below the stats HUD; behavior panel
/// below the caste panel.
const CASTE_PANEL_TOP: f32 = 220.0;
const BEHAVIOR_PANEL_TOP: f32 = 220.0 + TRI_SIZE + 60.0;
const PANEL_LEFT: f32 = 10.0;

// --- Setup ---

fn setup_ui(mut commands: Commands) {
    tracing::info!("UiPlugin: spawning debug UI");

    // --- Top-left stats HUD ---
    commands.spawn((
        Text::new("stats loading..."),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.95, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(10.0),
            ..default()
        },
        StatsText,
    ));

    // --- Bottom-right help text ---
    commands.spawn((
        Text::new(
            "WASD/arrows pan | scroll zoom | P pheromone | T temperature\n1/2/3/4 speed (30/60/150/300 Hz) | Space pause",
        ),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.8, 0.8)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            right: Val::Px(10.0),
            ..default()
        },
        HelpText,
    ));

    // --- Triangle panels (caste + behavior) ---
    spawn_triangle_panel(
        &mut commands,
        TriangleKind::Caste,
        "Caste (Workers / Soldiers / Breeders)",
        CASTE_PANEL_TOP,
    );
    spawn_triangle_panel(
        &mut commands,
        TriangleKind::Behavior,
        "Behavior (Forage / Dig / Nurse)",
        BEHAVIOR_PANEL_TOP,
    );
}

fn spawn_triangle_panel(
    commands: &mut Commands,
    kind: TriangleKind,
    title: &str,
    top: f32,
) {
    // Vertex layout (local, relative to panel top-left):
    //   corner 0 (top) = (TRI_SIZE/2, 0)
    //   corner 1 (bottom-left) = (0, TRI_SIZE)
    //   corner 2 (bottom-right) = (TRI_SIZE, TRI_SIZE)
    let v0 = Vec2::new(TRI_SIZE * 0.5, 0.0);
    let v1 = Vec2::new(0.0, TRI_SIZE);
    let v2 = Vec2::new(TRI_SIZE, TRI_SIZE);

    // Title.
    commands.spawn((
        Text::new(title.to_string()),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.9, 0.4)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(top - 18.0),
            left: Val::Px(PANEL_LEFT),
            ..default()
        },
    ));

    // Panel background (semi-transparent box sized to fit the triangle).
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(top),
                left: Val::Px(PANEL_LEFT),
                width: Val::Px(TRI_SIZE),
                height: Val::Px(TRI_SIZE),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.5)),
            TrianglePanel {
                kind,
                verts: [v0, v1, v2],
            },
            Interaction::default(),
        ))
        .with_children(|p| {
            // Corner labels.
            let (lbl0, lbl1, lbl2) = match kind {
                TriangleKind::Caste => ("Worker", "Soldier", "Breeder"),
                TriangleKind::Behavior => ("Forage", "Dig", "Nurse"),
            };
            p.spawn((
                Text::new(lbl0),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(-14.0),
                    left: Val::Px(TRI_SIZE * 0.5 - 20.0),
                    ..default()
                },
            ));
            p.spawn((
                Text::new(lbl1),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(-14.0),
                    left: Val::Px(-4.0),
                    ..default()
                },
            ));
            p.spawn((
                Text::new(lbl2),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(-14.0),
                    right: Val::Px(-4.0),
                    ..default()
                },
            ));

            // The draggable handle (just a small colored square positioned
            // absolutely within the panel). Initial position placed at
            // centroid; the sync system repositions it each frame.
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(HANDLE_RADIUS * 2.0),
                    height: Val::Px(HANDLE_RADIUS * 2.0),
                    top: Val::Px(TRI_SIZE * 0.5 - HANDLE_RADIUS),
                    left: Val::Px(TRI_SIZE * 0.5 - HANDLE_RADIUS),
                    ..default()
                },
                BackgroundColor(Color::srgb(1.0, 0.8, 0.2)),
                TriangleHandle { kind },
            ));
        });
}

// --- Systems ---

fn update_stats_text(
    sim: Res<SimulationState>,
    diagnostics: Res<DiagnosticsStore>,
    paused: Res<SimPaused>,
    mut q: Query<&mut Text, With<StatsText>>,
) {
    let Some(colony) = sim.sim.colonies.first() else {
        return;
    };

    // Count ants by caste from sim.sim.ants (population struct may not be
    // updated yet in early phases — count from live ants for accuracy).
    let mut workers = 0u32;
    let mut soldiers = 0u32;
    let mut breeders = 0u32;
    let mut queens = 0u32;
    for a in &sim.sim.ants {
        match a.caste {
            AntCaste::Worker => workers += 1,
            AntCaste::Soldier => soldiers += 1,
            AntCaste::Breeder => breeders += 1,
            AntCaste::Queen => queens += 1,
        }
    }

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);

    let pause_tag = if paused.0 { " [PAUSED]" } else { "" };

    // --- K3 season/temp/diapause HUD ---
    let season = sim.sim.season().label();
    let doy = sim.sim.day_of_year();
    let year = sim.sim.in_game_year();
    let ambient = sim.sim.ambient_temp_c();
    let cold_t = sim.sim.config.ant.hibernation_cold_threshold_c;
    let in_diapause = if let Some(ne) = colony.nest_entrance_positions.first() {
        let m = sim.sim.topology.module(0);
        m.temp_at(*ne) < cold_t
    } else {
        false
    };
    let fertility_line = if colony.fertility_suppressed {
        "Fertility: SUPPRESSED  (!) Missed winter — no eggs this year"
    } else {
        "Fertility: ok"
    };

    let text = format!(
        "Tick: {}{}\nFPS: {:.0}\nSeason: {} (day {}/365, year {})\nAmbient: {:.1} °C\nDiapause: {}\n{}\nAnts: {} (W {} / S {} / B {} / Q {})\nFood stored: {:.1}\nFood returned: {}\nBrood: eggs {} / larvae {} / pupae {}\nQueen HP: {:.1}",
        sim.sim.tick,
        pause_tag,
        fps,
        season,
        doy,
        year,
        ambient,
        if in_diapause { "ON" } else { "off" },
        fertility_line,
        sim.sim.ants.len(),
        workers,
        soldiers,
        breeders,
        queens,
        colony.food_stored,
        colony.food_returned,
        colony.eggs,
        colony.larvae,
        colony.pupae,
        colony.queen_health,
    );

    for mut t in q.iter_mut() {
        **t = text.clone();
    }
}

fn handle_speed_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut fixed: ResMut<Time<Fixed>>,
    mut virt: ResMut<Time<Virtual>>,
    mut paused: ResMut<SimPaused>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        *fixed = Time::<Fixed>::from_hz(30.0);
        tracing::info!("sim speed -> 30 Hz");
    }
    if keys.just_pressed(KeyCode::Digit2) {
        *fixed = Time::<Fixed>::from_hz(60.0);
        tracing::info!("sim speed -> 60 Hz");
    }
    if keys.just_pressed(KeyCode::Digit3) {
        *fixed = Time::<Fixed>::from_hz(150.0);
        tracing::info!("sim speed -> 150 Hz");
    }
    if keys.just_pressed(KeyCode::Digit4) {
        *fixed = Time::<Fixed>::from_hz(300.0);
        tracing::info!("sim speed -> 300 Hz");
    }
    if keys.just_pressed(KeyCode::Space) {
        paused.0 = !paused.0;
        if paused.0 {
            virt.pause();
        } else {
            virt.unpause();
        }
        tracing::info!(paused = paused.0, "simulation pause toggled");
    }
}

/// Convert a screen-space point inside a triangle to barycentric
/// coordinates relative to the triangle's three vertices.
/// Returns (a, b, c) with a+b+c=1 where a is weight of v0, etc.
fn barycentric(p: Vec2, v0: Vec2, v1: Vec2, v2: Vec2) -> (f32, f32, f32) {
    let denom = (v1.y - v2.y) * (v0.x - v2.x) + (v2.x - v1.x) * (v0.y - v2.y);
    if denom.abs() < 1e-6 {
        return (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0);
    }
    let a = ((v1.y - v2.y) * (p.x - v2.x) + (v2.x - v1.x) * (p.y - v2.y)) / denom;
    let b = ((v2.y - v0.y) * (p.x - v2.x) + (v0.x - v2.x) * (p.y - v2.y)) / denom;
    let c = 1.0 - a - b;
    (a, b, c)
}

/// Clamp a point into the triangle by clamping barycentrics to [0,1] and
/// renormalizing.
fn clamp_into_triangle(p: Vec2, v0: Vec2, v1: Vec2, v2: Vec2) -> (Vec2, (f32, f32, f32)) {
    let (mut a, mut b, mut c) = barycentric(p, v0, v1, v2);
    a = a.max(0.0);
    b = b.max(0.0);
    c = c.max(0.0);
    let s = a + b + c;
    if s < 1e-6 {
        a = 1.0 / 3.0;
        b = 1.0 / 3.0;
        c = 1.0 / 3.0;
    } else {
        a /= s;
        b /= s;
        c /= s;
    }
    let clamped = v0 * a + v1 * b + v2 * c;
    (clamped, (a, b, c))
}

fn triangle_drag_system(
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<UiDragState>,
    panels: Query<(&TrianglePanel, &Node, &GlobalTransform)>,
    mut sim: ResMut<SimulationState>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        if !mouse.pressed(MouseButton::Left) {
            drag.active = None;
        }
        return;
    };

    if !mouse.pressed(MouseButton::Left) {
        if drag.active.is_some() {
            tracing::debug!(kind = ?drag.active, "drag released");
        }
        drag.active = None;
        return;
    }

    // Determine which panel the cursor is in (on press) or continue the
    // active drag.
    let just_pressed = mouse.just_pressed(MouseButton::Left);

    for (panel, _node, gt) in panels.iter() {
        // Panel top-left in screen-space.
        let panel_origin = gt.translation().truncate()
            - Vec2::new(TRI_SIZE * 0.5, TRI_SIZE * 0.5);
        let local = cursor - panel_origin;

        let v0 = panel.verts[0];
        let v1 = panel.verts[1];
        let v2 = panel.verts[2];

        // On just-pressed: claim this panel if cursor is inside its bbox
        // AND inside the triangle (barycentrics all in [0,1]).
        if just_pressed && drag.active.is_none() {
            if local.x >= 0.0
                && local.x <= TRI_SIZE
                && local.y >= 0.0
                && local.y <= TRI_SIZE
            {
                let (a, b, c) = barycentric(local, v0, v1, v2);
                if a >= 0.0 && b >= 0.0 && c >= 0.0 {
                    drag.active = Some(panel.kind);
                    tracing::debug!(kind = ?panel.kind, "drag started");
                }
            }
        }

        // If this panel is the active one, update weights.
        if drag.active == Some(panel.kind) {
            let (_pt, (a, b, c)) = clamp_into_triangle(local, v0, v1, v2);
            // Apply to colony 0. Corner 0 = first weight, 1 = second, 2 = third.
            if let Some(colony) = sim.sim.colonies.first_mut() {
                match panel.kind {
                    TriangleKind::Caste => {
                        colony.caste_ratio.worker = a;
                        colony.caste_ratio.soldier = b;
                        colony.caste_ratio.breeder = c;
                        tracing::trace!(w = a, s = b, b = c, "caste ratio set");
                    }
                    TriangleKind::Behavior => {
                        colony.behavior_weights.forage = a;
                        colony.behavior_weights.dig = b;
                        colony.behavior_weights.nurse = c;
                        tracing::trace!(f = a, d = b, n = c, "behavior weights set");
                    }
                }
            }
        }
    }
}

/// Each frame, reposition the handle nodes to match the current weights
/// stored in the colony (so external changes are reflected, too).
fn update_triangle_handles(
    sim: Res<SimulationState>,
    panels: Query<&TrianglePanel>,
    mut handles: Query<(&TriangleHandle, &mut Node)>,
) {
    let Some(colony) = sim.sim.colonies.first() else {
        return;
    };

    for (handle, mut node) in handles.iter_mut() {
        // Find matching panel (for vert positions — they're the same constants
        // today, but future-proof).
        let Some(panel) = panels.iter().find(|p| p.kind == handle.kind) else {
            continue;
        };
        let (a, b, c) = match handle.kind {
            TriangleKind::Caste => {
                let r = colony.caste_ratio;
                let s = (r.worker + r.soldier + r.breeder).max(1e-6);
                (r.worker / s, r.soldier / s, r.breeder / s)
            }
            TriangleKind::Behavior => {
                let w = colony.behavior_weights;
                let s = (w.forage + w.dig + w.nurse).max(1e-6);
                (w.forage / s, w.dig / s, w.nurse / s)
            }
        };
        let p = panel.verts[0] * a + panel.verts[1] * b + panel.verts[2] * c;
        node.left = Val::Px(p.x - HANDLE_RADIUS);
        node.top = Val::Px(p.y - HANDLE_RADIUS);
    }
}

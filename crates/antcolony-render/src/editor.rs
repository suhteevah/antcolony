//! Keeper-mode formicarium editor (Phase K2.3).
//!
//! Press `B` to toggle. Inside the editor the simulation is paused and a
//! palette of module kinds is drawn along the bottom of the screen. Click
//! a palette button to arm placement, then click an empty area of the
//! canvas to drop a module. Click an existing module to select it
//! (yellow outline); `Delete` or `X` removes it. Click a port (yellow
//! dot) to start drawing a tube, then click another port on a different
//! module to finish it. Click a tube's rectangle to select it, then
//! `Delete` to remove.
//!
//! Rebuild strategy: every mutation sets `TopologyDirty`. The rebuild
//! system in `plugin.rs` despawns all `FormicariumEntity` and respawns
//! the scene. It's brute-force but the scales are tiny (<30 modules).

use antcolony_game::SimulationState;
use antcolony_sim::{ModuleId, ModuleKind, PortPos, TubeId, unlock_hint};
use bevy::prelude::*;

use crate::AppState;
use crate::plugin::{ModuleRect, PortMarker, TopologyDirty, TubeSprite, FormicariumEntity};
use crate::ui::SimPaused;

/// Plugin that wires in the editor.
pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EditorState::default())
            .add_systems(OnEnter(AppState::Running), setup_editor_ui)
            .add_systems(
                Update,
                (
                    toggle_editor_key,
                    palette_button_system,
                    editor_canvas_clicks,
                    editor_delete_keys,
                    sync_editor_visibility,
                    sync_selection_gizmos,
                    sync_palette_unlocks,
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

// --- Resources ---

#[derive(Resource, Default, Debug)]
pub struct EditorState {
    pub active: bool,
    /// Armed palette selection — next canvas click places a module of
    /// this kind. Auto-cleared after placement.
    pub placing: Option<ModuleKind>,
    pub selection: Selection,
    /// First port clicked in a tube-draw gesture, if any.
    pub tube_start: Option<(ModuleId, PortPos)>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    #[default]
    None,
    Module(ModuleId),
    Tube(TubeId),
    Port(ModuleId, PortPos),
}

// --- UI components ---

#[derive(Component)]
struct EditorRootNode;

#[derive(Component)]
struct EditorHintText;

#[derive(Component, Clone, Copy)]
struct PaletteButton(ModuleKind);

/// Marker for yellow outline gizmo entities on the selected module.
#[derive(Component)]
struct SelectionGizmo;

// --- Constants ---

/// Hardcoded sizes per module kind (width, height in grid cells).
fn kind_dim(kind: ModuleKind) -> (usize, usize) {
    match kind {
        ModuleKind::TestTubeNest | ModuleKind::Hydration | ModuleKind::FeedingDish => (48, 32),
        ModuleKind::Outworld | ModuleKind::YTongNest => (80, 60),
        _ => (48, 32),
    }
}

/// Palette swatch color (rough visual cue, not precise).
fn kind_color(kind: ModuleKind) -> Color {
    match kind {
        ModuleKind::TestTubeNest => Color::srgb(0.85, 0.78, 0.55),
        ModuleKind::Outworld => Color::srgb(0.25, 0.45, 0.22),
        ModuleKind::YTongNest => Color::srgb(0.80, 0.70, 0.50),
        ModuleKind::Hydration => Color::srgb(0.45, 0.70, 0.90),
        ModuleKind::FeedingDish => Color::srgb(0.95, 0.72, 0.25),
        _ => Color::srgb(0.5, 0.5, 0.5),
    }
}

const PALETTE_KINDS: [ModuleKind; 5] = [
    ModuleKind::TestTubeNest,
    ModuleKind::Outworld,
    ModuleKind::YTongNest,
    ModuleKind::Hydration,
    ModuleKind::FeedingDish,
];

// --- UI setup ---

fn setup_editor_ui(mut commands: Commands) {
    // Root container pinned to the bottom-center. Starts hidden; flipped
    // on when the editor toggles on.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(6.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Px(68.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            EditorRootNode,
            Visibility::Hidden,
        ))
        .with_children(|p| {
            for kind in PALETTE_KINDS {
                p.spawn((
                    Button,
                    Node {
                        width: Val::Px(96.0),
                        height: Val::Px(56.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(kind_color(kind)),
                    BorderColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                    PaletteButton(kind),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new(kind.label()),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::BLACK),
                    ));
                });
            }
        });

    // Status hint (top-center).
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.9, 0.4)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        EditorHintText,
        Visibility::Hidden,
    ));

    tracing::info!("EditorPlugin: palette + hint UI spawned (hidden until B pressed)");
}

// --- Systems ---

fn toggle_editor_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<EditorState>,
    mut paused: ResMut<SimPaused>,
    mut virt: ResMut<Time<Virtual>>,
) {
    if !keys.just_pressed(KeyCode::KeyB) {
        return;
    }
    editor.active = !editor.active;
    if editor.active {
        // Pause the sim on entry. Don't touch `paused` if already paused by
        // user — record that Space-pause state separately.
        if !paused.0 {
            virt.pause();
            paused.0 = true;
        }
        tracing::info!("editor toggled ON (B)");
    } else {
        editor.placing = None;
        editor.selection = Selection::None;
        editor.tube_start = None;
        // Un-pause on exit (we always unpause — the player wanted to edit,
        // so resuming play is the expected behavior).
        if paused.0 {
            virt.unpause();
            paused.0 = false;
        }
        tracing::info!("editor toggled OFF (B)");
    }
}

fn sync_editor_visibility(
    editor: Res<EditorState>,
    mut q_root: Query<&mut Visibility, (With<EditorRootNode>, Without<EditorHintText>)>,
    mut q_hint: Query<(&mut Visibility, &mut Text), With<EditorHintText>>,
) {
    let vis = if editor.active {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut v in q_root.iter_mut() {
        *v = vis;
    }
    for (mut v, mut t) in q_hint.iter_mut() {
        *v = vis;
        if editor.active {
            let line = match (editor.placing, editor.tube_start, editor.selection) {
                (Some(k), _, _) => format!("EDITOR | placing: {} — click canvas to drop", k.label()),
                (None, Some((m, p)), _) => format!("EDITOR | tube start at module {} port ({},{}) — click another port", m, p.x, p.y),
                (None, None, Selection::Module(id)) => format!("EDITOR | module {} selected — Delete/X removes", id),
                (None, None, Selection::Tube(id)) => format!("EDITOR | tube {} selected — Delete/X removes", id),
                (None, None, Selection::Port(m, p)) => format!("EDITOR | port ({},{}) on module {} selected", p.x, p.y, m),
                _ => "EDITOR | B exit | click palette to arm placement | click module/port/tube to select".to_string(),
            };
            **t = line;
        }
    }
}

fn palette_button_system(
    mut editor: ResMut<EditorState>,
    sim: Res<SimulationState>,
    mut q: Query<
        (&Interaction, &PaletteButton, &mut BorderColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, btn, mut border) in q.iter_mut() {
        let unlocked = sim.sim.module_kind_unlocked(btn.0);
        match *interaction {
            Interaction::Pressed => {
                if !unlocked {
                    tracing::trace!(kind = ?btn.0, hint = unlock_hint(btn.0), "palette: locked kind click ignored");
                    continue;
                }
                editor.placing = Some(btn.0);
                editor.selection = Selection::None;
                editor.tube_start = None;
                border.0 = Color::srgb(1.0, 0.9, 0.2);
                tracing::info!(kind = ?btn.0, "editor: palette armed");
            }
            Interaction::Hovered => {
                border.0 = Color::srgb(1.0, 0.95, 0.5);
            }
            Interaction::None => {
                border.0 = Color::srgba(0.0, 0.0, 0.0, 0.5);
            }
        }
    }
}

/// Grey out locked palette buttons based on current progression.
fn sync_palette_unlocks(
    sim: Res<SimulationState>,
    mut q: Query<(&PaletteButton, &mut BackgroundColor)>,
) {
    for (btn, mut bg) in q.iter_mut() {
        let unlocked = sim.sim.module_kind_unlocked(btn.0);
        let base = kind_color(btn.0);
        if unlocked {
            bg.0 = base;
        } else {
            // 50% opacity on black — visually greyed.
            let srgba = base.to_srgba();
            bg.0 = Color::srgba(srgba.red * 0.4, srgba.green * 0.4, srgba.blue * 0.4, 0.5);
        }
    }
}

/// Returns the cursor's world-space position (post-centroid transform, the
/// same space `ModuleRect.min/max` lives in). Returns `None` if the cursor
/// is over a UI element or outside the window.
fn cursor_world_pos(
    windows: &Query<&Window>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) -> Option<Vec2> {
    let window = windows.get_single().ok()?;
    let cursor = window.cursor_position()?;
    let (camera, cam_tf) = cameras.get_single().ok()?;
    camera.viewport_to_world_2d(cam_tf, cursor).ok()
}

#[allow(clippy::too_many_arguments)]
fn editor_canvas_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    mut editor: ResMut<EditorState>,
    mut sim: ResMut<SimulationState>,
    mut dirty: ResMut<TopologyDirty>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    q_modules: Query<&ModuleRect>,
    q_ports: Query<&PortMarker>,
    q_tubes: Query<&TubeSprite>,
    // Don't hit-test canvas clicks that start on a UI button.
    q_ui: Query<&Interaction, With<Button>>,
) {
    if !editor.active {
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    // Skip when the click started on a UI button.
    if q_ui.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(world) = cursor_world_pos(&windows, &cameras) else {
        return;
    };

    // 1. Port hit-test (priority — small target).
    if let Some(pm) = q_ports
        .iter()
        .min_by(|a, b| {
            a.world_pos
                .distance_squared(world)
                .partial_cmp(&b.world_pos.distance_squared(world))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .filter(|pm| pm.world_pos.distance(world) <= 6.0)
    {
        handle_port_click(pm, &mut editor, &mut sim, &mut dirty);
        return;
    }

    // 2. Tube hit-test.
    for tube in q_tubes.iter() {
        if point_to_segment_distance(world, tube.a, tube.b) <= 5.0 {
            editor.selection = Selection::Tube(tube.id);
            editor.tube_start = None;
            editor.placing = None;
            tracing::info!(tube_id = tube.id, "editor: tube selected");
            return;
        }
    }

    // 3. Module hit-test.
    let hit_module = q_modules.iter().find(|r| {
        world.x >= r.min.x && world.x <= r.max.x && world.y >= r.min.y && world.y <= r.max.y
    });

    // 4. If placing is armed AND no module was hit, place a new module.
    if let Some(kind) = editor.placing {
        if hit_module.is_none() {
            place_new_module(kind, world, &mut sim, &mut dirty);
            editor.placing = None;
            editor.selection = Selection::None;
            return;
        }
    }

    if let Some(m) = hit_module {
        editor.selection = Selection::Module(m.id);
        editor.tube_start = None;
        tracing::info!(module_id = m.id, "editor: module selected");
    } else {
        editor.selection = Selection::None;
        editor.tube_start = None;
    }
}

fn handle_port_click(
    pm: &PortMarker,
    editor: &mut EditorState,
    sim: &mut SimulationState,
    dirty: &mut TopologyDirty,
) {
    match editor.tube_start {
        None => {
            editor.tube_start = Some((pm.module, pm.port));
            editor.selection = Selection::Port(pm.module, pm.port);
            editor.placing = None;
            tracing::info!(module = pm.module, px = pm.port.x, py = pm.port.y, "editor: tube start anchored");
        }
        Some((from_mod, from_port)) => {
            if from_mod == pm.module {
                // Same-module click = cancel tube draw, reselect port.
                editor.tube_start = Some((pm.module, pm.port));
                editor.selection = Selection::Port(pm.module, pm.port);
                tracing::info!("editor: tube-draw retargeted (same module click)");
                return;
            }
            // Reject duplicate tube between these two ports.
            let already = sim
                .sim
                .topology
                .tubes
                .iter()
                .any(|t| {
                    (t.from.module == from_mod
                        && t.from.port == from_port
                        && t.to.module == pm.module
                        && t.to.port == pm.port)
                        || (t.to.module == from_mod
                            && t.to.port == from_port
                            && t.from.module == pm.module
                            && t.from.port == pm.port)
                });
            if already {
                editor.tube_start = None;
                tracing::warn!("editor: tube already exists between these ports — cancelled");
                return;
            }
            let id = sim
                .sim
                .add_tube(from_mod, from_port, pm.module, pm.port, 30, 8.0);
            tracing::info!(
                tube_id = id,
                from_mod,
                to_mod = pm.module,
                "editor: tube created"
            );
            editor.tube_start = None;
            editor.selection = Selection::Tube(id);
            dirty.0 = true;
        }
    }
}

fn place_new_module(
    kind: ModuleKind,
    world_pos: Vec2,
    sim: &mut SimulationState,
    dirty: &mut TopologyDirty,
) {
    let (w, h) = kind_dim(kind);
    // Convert cursor world-position (post-centroid) into
    // formicarium_origin space. Every frame `compute_layout` recomputes
    // the centroid from all module origins, so placing a module at a
    // specific world-pos is tricky: we need the new module's
    // `formicarium_origin` such that after recentring, its world-space
    // position matches the click. We approximate by using the current
    // centroid: origin = (click + centroid) / TILE, minus half-size so
    // the module is centered on the click.
    let centroid = compute_centroid(sim);
    let tile = crate::plugin::TILE;
    let origin_world = world_pos + centroid - Vec2::new(w as f32 * tile * 0.5, h as f32 * tile * 0.5);
    let formicarium_origin = origin_world / tile;
    let label = format!("{} {}", kind.label(), sim.sim.topology.next_module_id());
    let id = sim
        .sim
        .add_module(kind, w, h, formicarium_origin, label);
    tracing::info!(
        id,
        kind = ?kind,
        w,
        h,
        ox = formicarium_origin.x,
        oy = formicarium_origin.y,
        "editor: module placed"
    );
    let _ = id;
    dirty.0 = true;
}

fn compute_centroid(sim: &SimulationState) -> Vec2 {
    let tile = crate::plugin::TILE;
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for m in &sim.sim.topology.modules {
        let origin = m.formicarium_origin * tile;
        let far = origin + Vec2::new(m.width() as f32 * tile, m.height() as f32 * tile);
        min = min.min(origin);
        max = max.max(far);
    }
    if !min.x.is_finite() {
        return Vec2::ZERO;
    }
    (min + max) * 0.5
}

fn editor_delete_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<EditorState>,
    mut sim: ResMut<SimulationState>,
    mut dirty: ResMut<TopologyDirty>,
) {
    if !editor.active {
        return;
    }
    if !(keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::KeyX)) {
        return;
    }
    match editor.selection {
        Selection::Module(id) => {
            let killed = sim.sim.remove_module(id);
            tracing::info!(module_id = id, killed, "editor: module removed");
            editor.selection = Selection::None;
            dirty.0 = true;
        }
        Selection::Tube(id) => {
            let killed = sim.sim.remove_tube(id);
            tracing::info!(tube_id = id, killed, "editor: tube removed");
            editor.selection = Selection::None;
            dirty.0 = true;
        }
        Selection::Port(_, _) => {
            editor.selection = Selection::None;
            editor.tube_start = None;
        }
        Selection::None => {}
    }
}

/// Spawn yellow outline quads around the currently selected module and a
/// small highlight dot for selected ports. Despawn and respawn each frame
/// — the quantities are tiny (at most 4 sprites) so the churn is fine.
fn sync_selection_gizmos(
    mut commands: Commands,
    editor: Res<EditorState>,
    q_gizmos: Query<Entity, With<SelectionGizmo>>,
    q_modules: Query<&ModuleRect>,
    q_ports: Query<&PortMarker>,
    q_tubes: Query<&TubeSprite>,
) {
    for e in q_gizmos.iter() {
        commands.entity(e).despawn();
    }
    if !editor.active {
        return;
    }
    let yellow = Color::srgba(1.0, 0.9, 0.2, 0.95);
    let thickness = 2.5;

    let mut outline = |min: Vec2, max: Vec2| {
        let w = max.x - min.x;
        let h = max.y - min.y;
        let cx = (min.x + max.x) * 0.5;
        let cy = (min.y + max.y) * 0.5;
        for (w_size, h_size, dx, dy) in [
            (w + thickness, thickness, 0.0, -h * 0.5),
            (w + thickness, thickness, 0.0, h * 0.5),
            (thickness, h + thickness, -w * 0.5, 0.0),
            (thickness, h + thickness, w * 0.5, 0.0),
        ] {
            commands.spawn((
                Sprite {
                    color: yellow,
                    custom_size: Some(Vec2::new(w_size, h_size)),
                    ..default()
                },
                Transform::from_xyz(cx + dx, cy + dy, 5.0),
                SelectionGizmo,
            ));
        }
    };

    match editor.selection {
        Selection::Module(id) => {
            if let Some(r) = q_modules.iter().find(|r| r.id == id) {
                outline(r.min, r.max);
            }
        }
        Selection::Tube(id) => {
            if let Some(t) = q_tubes.iter().find(|t| t.id == id) {
                // Thin rectangle drawn as line of short segments.
                let mid = (t.a + t.b) * 0.5;
                let len = t.a.distance(t.b);
                let angle = (t.b - t.a).y.atan2((t.b - t.a).x);
                commands.spawn((
                    Sprite {
                        color: yellow,
                        custom_size: Some(Vec2::new(len + 4.0, thickness * 2.0)),
                        ..default()
                    },
                    Transform {
                        translation: mid.extend(5.0),
                        rotation: Quat::from_rotation_z(angle),
                        ..default()
                    },
                    SelectionGizmo,
                ));
            }
        }
        Selection::Port(m, p) => {
            if let Some(pm) = q_ports.iter().find(|pm| pm.module == m && pm.port == p) {
                commands.spawn((
                    Sprite {
                        color: yellow,
                        custom_size: Some(Vec2::splat(8.0)),
                        ..default()
                    },
                    Transform::from_translation(pm.world_pos.extend(5.0)),
                    SelectionGizmo,
                ));
            }
        }
        Selection::None => {}
    }

    // Also highlight the tube-start anchor.
    if let Some((m, p)) = editor.tube_start {
        if let Some(pm) = q_ports.iter().find(|pm| pm.module == m && pm.port == p) {
            commands.spawn((
                Sprite {
                    color: Color::srgba(1.0, 0.5, 0.1, 0.9),
                    custom_size: Some(Vec2::splat(10.0)),
                    ..default()
                },
                Transform::from_translation(pm.world_pos.extend(5.1)),
                SelectionGizmo,
            ));
        }
    }

    // Silence unused-import warning if the marker isn't referenced elsewhere.
    let _ = std::marker::PhantomData::<FormicariumEntity>;
}

/// Perpendicular distance from point `p` to the line segment `a..b`.
fn point_to_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    p.distance(proj)
}

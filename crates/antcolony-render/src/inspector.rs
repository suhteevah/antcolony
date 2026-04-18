//! Click-any-ant inspector (K5).
//!
//! Left-click on an ant when the editor is closed to select it. A yellow
//! ring follows the selection and a top-right panel prints the ant's
//! stable id, caste, state, age, health, and food-carry status. Click on
//! empty space to clear the selection.

use antcolony_game::SimulationState;
use antcolony_sim::{AntCaste, AntState};
use bevy::prelude::*;

use crate::AppState;
use crate::editor::EditorState;
use crate::plugin::{FormicariumEntity, ModuleLayout, TILE};

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SelectedAnt::default())
            .add_systems(OnEnter(AppState::Running), setup_inspector)
            .add_systems(
                Update,
                (
                    click_select_ant,
                    update_selection_ring,
                    update_inspector_panel,
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

/// The currently inspected ant, keyed by the stable `Ant.id` field (survives
/// swap_remove). `None` = no selection, panel shows a prompt.
#[derive(Resource, Default, Debug)]
pub struct SelectedAnt(pub Option<u32>);

#[derive(Component)]
struct SelectionRing;

#[derive(Component)]
struct InspectorText;

/// Pick radius in world pixels for click hit-testing (≈ 2 tiles).
const PICK_RADIUS: f32 = TILE * 2.0;

fn setup_inspector(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    // Selection ring: a yellow annulus-ish sprite (outer circle, darker inner
    // circle stacked on top gives the donut look).
    let outer = meshes.add(Circle::new(TILE * 2.2));
    let inner = meshes.add(Circle::new(TILE * 1.6));
    let ring_mat = materials.add(Color::srgba(1.0, 0.85, 0.15, 0.95));
    let hole_mat = materials.add(Color::srgba(0.0, 0.0, 0.0, 0.0));
    commands
        .spawn((
            SelectionRing,
            Transform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
            Visibility::Hidden,
        ))
        .with_children(|c| {
            c.spawn((
                Mesh2d(outer),
                MeshMaterial2d(ring_mat),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
            c.spawn((
                Mesh2d(inner),
                MeshMaterial2d(hole_mat),
                Transform::from_xyz(0.0, 0.0, 0.1),
            ));
        });
    let _ = FormicariumEntity; // ring intentionally survives topology rebuilds

    // Panel: top-right, above the encyclopedia toggle.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                right: Val::Px(12.0),
                width: Val::Px(240.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.06, 0.10, 0.82)),
            BorderColor(Color::srgba(1.0, 0.85, 0.15, 0.6)),
            BorderRadius::all(Val::Px(4.0)),
        ))
        .with_children(|p| {
            p.spawn((
                InspectorText,
                Text::new("Click any ant to inspect"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.95, 0.95)),
            ));
        });
}

fn click_select_ant(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    editor: Res<EditorState>,
    sim: Res<SimulationState>,
    layout: Option<Res<ModuleLayout>>,
    mut selected: ResMut<SelectedAnt>,
) {
    if editor.active || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(layout) = layout else {
        return;
    };
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_tf)) = cameras.get_single() else {
        return;
    };
    let Some(world) = camera.viewport_to_world_2d(cam_tf, cursor).ok() else {
        return;
    };

    // Recentre match (sync_ant_sprites does the same).
    let (_, centroid) = crate::plugin::compute_layout(&sim);

    let mut best: Option<(u32, f32)> = None;
    for ant in &sim.sim.ants {
        let origin = layout
            .0
            .iter()
            .find(|(id, _)| *id == ant.module_id)
            .map(|(_, o)| *o - centroid)
            .unwrap_or(Vec2::ZERO);
        let p = if let Some(transit) = ant.transit {
            let tube = sim.sim.topology.tube(transit.tube);
            let a_origin = layout
                .0
                .iter()
                .find(|(id, _)| *id == tube.from.module)
                .map(|(_, o)| *o - centroid)
                .unwrap_or(Vec2::ZERO);
            let b_origin = layout
                .0
                .iter()
                .find(|(id, _)| *id == tube.to.module)
                .map(|(_, o)| *o - centroid)
                .unwrap_or(Vec2::ZERO);
            let a = Vec2::new(
                a_origin.x + (tube.from.port.x as f32 + 0.5) * TILE,
                a_origin.y + (tube.from.port.y as f32 + 0.5) * TILE,
            );
            let b = Vec2::new(
                b_origin.x + (tube.to.port.x as f32 + 0.5) * TILE,
                b_origin.y + (tube.to.port.y as f32 + 0.5) * TILE,
            );
            a.lerp(b, transit.progress.clamp(0.0, 1.0))
        } else {
            Vec2::new(origin.x + ant.position.x * TILE, origin.y + ant.position.y * TILE)
        };
        let d = p.distance(world);
        if d <= PICK_RADIUS && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((ant.id, d));
        }
    }
    match best {
        Some((id, _)) => {
            tracing::info!(ant_id = id, "inspector: selected ant");
            selected.0 = Some(id);
        }
        None => {
            if selected.0.is_some() {
                tracing::info!("inspector: cleared selection");
            }
            selected.0 = None;
        }
    }
}

fn update_selection_ring(
    selected: Res<SelectedAnt>,
    sim: Res<SimulationState>,
    layout: Option<Res<ModuleLayout>>,
    mut q: Query<(&mut Transform, &mut Visibility), With<SelectionRing>>,
) {
    let Ok((mut tf, mut vis)) = q.get_single_mut() else {
        return;
    };
    let Some(layout) = layout else {
        *vis = Visibility::Hidden;
        return;
    };
    let Some(id) = selected.0 else {
        *vis = Visibility::Hidden;
        return;
    };
    let Some(ant) = sim.sim.ants.iter().find(|a| a.id == id) else {
        *vis = Visibility::Hidden;
        return;
    };

    let (_, centroid) = crate::plugin::compute_layout(&sim);
    let pos = if let Some(transit) = ant.transit {
        let tube = sim.sim.topology.tube(transit.tube);
        let a_origin = layout
            .0
            .iter()
            .find(|(mid, _)| *mid == tube.from.module)
            .map(|(_, o)| *o - centroid)
            .unwrap_or(Vec2::ZERO);
        let b_origin = layout
            .0
            .iter()
            .find(|(mid, _)| *mid == tube.to.module)
            .map(|(_, o)| *o - centroid)
            .unwrap_or(Vec2::ZERO);
        let a = Vec2::new(
            a_origin.x + (tube.from.port.x as f32 + 0.5) * TILE,
            a_origin.y + (tube.from.port.y as f32 + 0.5) * TILE,
        );
        let b = Vec2::new(
            b_origin.x + (tube.to.port.x as f32 + 0.5) * TILE,
            b_origin.y + (tube.to.port.y as f32 + 0.5) * TILE,
        );
        a.lerp(b, transit.progress.clamp(0.0, 1.0))
    } else {
        let origin = layout
            .0
            .iter()
            .find(|(mid, _)| *mid == ant.module_id)
            .map(|(_, o)| *o - centroid)
            .unwrap_or(Vec2::ZERO);
        Vec2::new(origin.x + ant.position.x * TILE, origin.y + ant.position.y * TILE)
    };
    tf.translation.x = pos.x;
    tf.translation.y = pos.y;
    tf.translation.z = 5.0;
    *vis = Visibility::Visible;
}

fn update_inspector_panel(
    selected: Res<SelectedAnt>,
    sim: Res<SimulationState>,
    mut q: Query<&mut Text, With<InspectorText>>,
) {
    let Ok(mut text) = q.get_single_mut() else {
        return;
    };
    let Some(id) = selected.0 else {
        text.0 = "Click any ant to inspect".to_string();
        return;
    };
    let Some(ant) = sim.sim.ants.iter().find(|a| a.id == id) else {
        text.0 = format!("Ant #{id}\n  (dead or despawned)");
        return;
    };

    let caste = match ant.caste {
        AntCaste::Worker => "Worker",
        AntCaste::Soldier => "Soldier",
        AntCaste::Queen => "Queen",
        AntCaste::Breeder => "Breeder",
    };
    let state = match ant.state {
        AntState::Idle => "Idle",
        AntState::Exploring => "Exploring",
        AntState::FollowingTrail => "Following trail",
        AntState::PickingUpFood => "Picking up food",
        AntState::ReturningHome => "Returning home",
        AntState::StoringFood => "Storing food",
        AntState::Fighting => "Fighting",
        AntState::Fleeing => "Fleeing",
        AntState::Nursing => "Nursing",
        AntState::Digging => "Digging",
        AntState::Diapause => "Diapause",
        AntState::NuptialFlight => "Nuptial flight",
    };

    let age_sec = ant.age as f32 * sim.sim.in_game_seconds_per_tick;
    let age_str = if age_sec < 120.0 {
        format!("{:.0}s", age_sec)
    } else if age_sec < 7200.0 {
        format!("{:.1}m", age_sec / 60.0)
    } else if age_sec < 172_800.0 {
        format!("{:.1}h", age_sec / 3600.0)
    } else {
        format!("{:.1}d", age_sec / 86_400.0)
    };

    let carry = if ant.food_carried > 0.0 {
        format!("\nCarrying: {:.2} food", ant.food_carried)
    } else {
        String::new()
    };
    let transit = if ant.transit.is_some() {
        "\nIn tube transit"
    } else {
        ""
    };

    text.0 = format!(
        "Ant #{id}\nCaste:  {caste}\nState:  {state}\nAge:    {age_str} ({} ticks)\nHealth: {:.1}\nModule: {}{}{}",
        ant.age, ant.health, ant.module_id, carry, transit
    );
}

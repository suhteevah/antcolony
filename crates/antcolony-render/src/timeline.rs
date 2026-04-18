//! Colony history timeline (K5).
//!
//! A thin horizontal bar pinned to the bottom of the screen that plots
//! every earned milestone from `ColonyState.milestones` against elapsed
//! tick count. Hovering a marker shows the label + in-game day in a
//! tooltip above the bar. Press `H` to toggle visibility.

use antcolony_game::SimulationState;
use antcolony_sim::MilestoneKind;
use bevy::prelude::*;

use crate::AppState;

pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TimelineVisible(true))
            .add_systems(OnEnter(AppState::Running), setup_timeline)
            .add_systems(
                Update,
                (
                    toggle_timeline_key,
                    sync_timeline_markers,
                    reposition_markers,
                    update_tooltip,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Resource)]
struct TimelineVisible(bool);

#[derive(Component)]
struct TimelineRoot;

/// The horizontal bar the markers sit on.
#[derive(Component)]
struct TimelineBar;

/// A marker node — one per awarded milestone.
#[derive(Component, Clone, Copy)]
struct TimelineMarker {
    kind: MilestoneKind,
    tick: u64,
    day: u32,
}

/// Floating label that follows the hovered marker.
#[derive(Component)]
struct TimelineTooltip;

const BAR_WIDTH: f32 = 640.0;
const BAR_HEIGHT: f32 = 8.0;
const MARKER_SIZE: f32 = 14.0;
const BOTTOM_OFFSET: f32 = 32.0;

fn setup_timeline(mut commands: Commands) {
    commands
        .spawn((
            TimelineRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(BOTTOM_OFFSET),
                left: Val::Percent(50.0),
                width: Val::Px(BAR_WIDTH),
                height: Val::Px(48.0),
                margin: UiRect::left(Val::Px(-BAR_WIDTH * 0.5)),
                ..default()
            },
        ))
        .with_children(|p| {
            // Section title, floating above-left of the bar.
            p.spawn((
                Text::new("Colony timeline"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.75, 0.4)),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(-16.0),
                    left: Val::Px(0.0),
                    ..default()
                },
            ));
            // The bar itself — markers will be added/removed as children
            // of this node by the sync system.
            p.spawn((
                TimelineBar,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(16.0),
                    left: Val::Px(0.0),
                    width: Val::Px(BAR_WIDTH),
                    height: Val::Px(BAR_HEIGHT),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.08, 0.12, 0.85)),
                BorderRadius::all(Val::Px(4.0)),
            ));
            // Tooltip — populated by `update_tooltip` when a marker is
            // hovered; otherwise hidden.
            p.spawn((
                TimelineTooltip,
                Text::new(""),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.97, 0.93, 0.65)),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(-14.0),
                    left: Val::Px(0.0),
                    ..default()
                },
                Visibility::Hidden,
            ));
        });

    tracing::info!("TimelinePlugin: bar spawned (hidden with H)");
}

fn toggle_timeline_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut vis: ResMut<TimelineVisible>,
    mut q: Query<&mut Visibility, With<TimelineRoot>>,
) {
    if !keys.just_pressed(KeyCode::KeyH) {
        return;
    }
    vis.0 = !vis.0;
    for mut v in q.iter_mut() {
        *v = if vis.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    tracing::info!(visible = vis.0, "timeline toggled");
}

fn sync_timeline_markers(
    mut commands: Commands,
    sim: Res<SimulationState>,
    bars: Query<Entity, With<TimelineBar>>,
    existing: Query<(Entity, &TimelineMarker)>,
) {
    let Ok(bar_entity) = bars.get_single() else {
        return;
    };
    let colony = match sim.sim.colonies.first() {
        Some(c) => c,
        None => return,
    };
    // Cheap: only respawn if the count diverges. Milestones are append-only.
    let current_count = existing.iter().count();
    if current_count == colony.milestones.len() {
        return;
    }
    for (e, _) in existing.iter() {
        commands.entity(e).despawn_recursive();
    }
    commands.entity(bar_entity).with_children(|p| {
        for m in &colony.milestones {
            let color = color_for(m.kind);
            p.spawn((
                TimelineMarker {
                    kind: m.kind,
                    tick: m.tick_awarded,
                    day: m.in_game_day,
                },
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(MARKER_SIZE),
                    height: Val::Px(MARKER_SIZE),
                    top: Val::Px((BAR_HEIGHT - MARKER_SIZE) * 0.5),
                    left: Val::Px(0.0), // reposition_markers sets this each frame
                    ..default()
                },
                BackgroundColor(color),
                BorderRadius::all(Val::Px(MARKER_SIZE * 0.5)),
                Interaction::default(),
            ));
        }
    });
}

fn reposition_markers(
    sim: Res<SimulationState>,
    mut q: Query<(&TimelineMarker, &mut Node)>,
) {
    // Use the current tick as the right edge so the timeline is always
    // "today at the far right, colony-birth at the far left". The first
    // few ticks can produce a divide-by-small, so clamp below.
    let span = sim.sim.tick.max(60) as f32;
    for (m, mut n) in q.iter_mut() {
        let t = (m.tick as f32 / span).clamp(0.0, 1.0);
        let x = t * (BAR_WIDTH - MARKER_SIZE);
        n.left = Val::Px(x);
    }
}

fn update_tooltip(
    markers: Query<(&TimelineMarker, &Interaction, &Node)>,
    mut tooltips: Query<(&mut Text, &mut Node, &mut Visibility), (With<TimelineTooltip>, Without<TimelineMarker>)>,
) {
    let Ok((mut text, mut node, mut vis)) = tooltips.get_single_mut() else {
        return;
    };
    // Any currently-hovered marker wins.
    let mut active: Option<(TimelineMarker, Val)> = None;
    for (m, i, n) in markers.iter() {
        if matches!(*i, Interaction::Hovered | Interaction::Pressed) {
            active = Some((*m, n.left));
            break;
        }
    }
    match active {
        Some((m, left)) => {
            text.0 = format!("{} — day {}, tick {}", m.kind.label(), m.day, m.tick);
            node.left = left;
            *vis = Visibility::Visible;
        }
        None => {
            *vis = Visibility::Hidden;
        }
    }
}

fn color_for(kind: MilestoneKind) -> Color {
    match kind {
        MilestoneKind::FirstEgg => Color::srgb(1.0, 0.85, 0.25),
        MilestoneKind::FirstMajor => Color::srgb(0.95, 0.35, 0.25),
        MilestoneKind::PopulationTen => Color::srgb(0.45, 0.70, 0.95),
        MilestoneKind::PopulationFifty => Color::srgb(0.30, 0.55, 0.90),
        MilestoneKind::PopulationOneHundred => Color::srgb(0.25, 0.80, 0.85),
        MilestoneKind::PopulationFiveHundred => Color::srgb(0.20, 0.95, 0.65),
        MilestoneKind::FirstColonyAnniversary => Color::srgb(1.0, 0.75, 0.20),
        MilestoneKind::SurvivedFirstWinter => Color::srgb(0.75, 0.85, 0.95),
        MilestoneKind::FirstNuptialFlight => Color::srgb(0.95, 0.55, 0.85),
        MilestoneKind::FirstDaughterColony => Color::srgb(0.75, 0.45, 0.95),
    }
}

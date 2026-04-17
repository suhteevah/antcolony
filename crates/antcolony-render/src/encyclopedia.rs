//! In-game encyclopedia side panel. Toggled with `E`. Shows the active
//! species' reference data (common/scientific name, difficulty badge,
//! tagline, description, fun facts, keeper notes).

use antcolony_game::SimulationState;
use bevy::prelude::*;

use crate::AppState;
use crate::picker::{difficulty_style, parse_hex};

pub struct EncyclopediaPlugin;

impl Plugin for EncyclopediaPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EncyclopediaVisible(false))
            .add_systems(OnEnter(AppState::Running), setup_encyclopedia)
            .add_systems(
                Update,
                (toggle_encyclopedia, apply_encyclopedia_visibility, update_milestone_list)
                    .run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Resource)]
struct EncyclopediaVisible(pub bool);

#[derive(Component)]
struct EncyclopediaPanel;

#[derive(Component)]
struct MilestoneList;

fn setup_encyclopedia(mut commands: Commands, sim: Res<SimulationState>) {
    let sp = &sim.species;
    let (badge_bg, badge_label) = difficulty_style(sp.difficulty);
    let swatch = parse_hex(&sp.appearance.color_hex);

    tracing::info!(species = %sp.id, "Encyclopedia: spawning side panel (hidden by default)");

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(10.0),
                right: Val::Px(10.0),
                width: Val::Px(360.0),
                height: Val::Percent(80.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(6.0),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.07, 0.10, 0.92)),
            Visibility::Hidden,
            EncyclopediaPanel,
        ))
        .with_children(|p| {
            // Header row with swatch + common name.
            p.spawn((Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            },))
                .with_children(|h| {
                    h.spawn((
                        Node {
                            width: Val::Px(18.0),
                            height: Val::Px(18.0),
                            ..default()
                        },
                        BackgroundColor(swatch),
                    ));
                    h.spawn((
                        Text::new(sp.common_name.clone()),
                        TextFont { font_size: 20.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            p.spawn((
                Text::new(sp.scientific_name()),
                TextFont { font_size: 13.0, ..default() },
                TextColor(Color::srgb(0.75, 0.80, 0.95)),
            ));
            p.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                    align_self: AlignSelf::FlexStart,
                    ..default()
                },
                BackgroundColor(badge_bg),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(badge_label.to_string()),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::BLACK),
                ));
            });
            p.spawn((
                Text::new(format!("\"{}\"", sp.encyclopedia.tagline)),
                TextFont { font_size: 13.0, ..default() },
                TextColor(Color::srgb(1.0, 0.85, 0.4)),
            ));
            p.spawn((
                Text::new(sp.encyclopedia.description.clone()),
                TextFont { font_size: 12.0, ..default() },
                TextColor(Color::srgb(0.88, 0.88, 0.88)),
            ));
            if !sp.encyclopedia.fun_facts.is_empty() {
                let mut s = String::from("Fun facts:\n");
                for f in &sp.encyclopedia.fun_facts {
                    s.push_str("  • ");
                    s.push_str(f);
                    s.push('\n');
                }
                p.spawn((
                    Text::new(s),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::srgb(0.78, 0.90, 0.78)),
                ));
            }
            if !sp.encyclopedia.keeper_notes.trim().is_empty() {
                p.spawn((
                    Text::new(format!("Keeper notes: {}", sp.encyclopedia.keeper_notes)),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::srgb(0.95, 0.82, 0.62)),
                ));
            }
            // Milestones (K4) — live-updated.
            p.spawn((
                Text::new("Milestones:\n  (none yet)"),
                TextFont { font_size: 12.0, ..default() },
                TextColor(Color::srgb(1.0, 0.85, 0.4)),
                MilestoneList,
            ));
            p.spawn((
                Text::new("(press E to close)"),
                TextFont { font_size: 10.0, ..default() },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ));
        });
}

fn toggle_encyclopedia(
    keys: Res<ButtonInput<KeyCode>>,
    mut vis: ResMut<EncyclopediaVisible>,
) {
    if keys.just_pressed(KeyCode::KeyE) {
        vis.0 = !vis.0;
        tracing::info!(visible = vis.0, "Encyclopedia: toggle");
    }
}

fn update_milestone_list(
    sim: Res<SimulationState>,
    mut q: Query<&mut Text, With<MilestoneList>>,
) {
    let Some(colony) = sim.sim.colonies.first() else {
        return;
    };
    let mut s = String::from("Milestones:\n");
    if colony.milestones.is_empty() {
        s.push_str("  (none yet)");
    } else {
        for m in &colony.milestones {
            s.push_str(&format!(
                "  - {} (day {}, tick {})\n",
                m.kind.label(),
                m.in_game_day,
                m.tick_awarded
            ));
        }
    }
    for mut t in q.iter_mut() {
        **t = s.clone();
    }
}

fn apply_encyclopedia_visibility(
    vis: Res<EncyclopediaVisible>,
    mut q: Query<&mut Visibility, With<EncyclopediaPanel>>,
) {
    if !vis.is_changed() {
        return;
    }
    for mut v in q.iter_mut() {
        *v = if vis.0 { Visibility::Visible } else { Visibility::Hidden };
    }
}

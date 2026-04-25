//! Keeper-mode species picker screen.
//!
//! Spawned on `OnEnter(AppState::Picker)`, despawned on `OnExit(AppState::Picker)`.
//! When the player confirms, inserts a `SimulationState` resource built from
//! the chosen species + time scale and transitions to `AppState::Running`.

use antcolony_game::SimulationState;
use antcolony_sim::{Difficulty, Environment, Species, TimeScale, load_species_dir};
use bevy::prelude::*;

use crate::AppState;

/// Path (relative to cwd / exe dir) where species TOML files live.
const SPECIES_DIR: &str = "assets/species";

pub struct PickerPlugin;

impl Plugin for PickerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PickerSelection>()
            .add_systems(OnEnter(AppState::Picker), (load_species_catalog, setup_picker_ui, autostart_from_env).chain())
            .add_systems(
                Update,
                (
                    species_button_system,
                    timescale_button_system,
                    confirm_button_system,
                    versus_key_launch_system,
                    update_detail_pane,
                    update_species_button_highlights,
                    update_timescale_button_highlights,
                    update_confirm_button_state,
                )
                    .run_if(in_state(AppState::Picker)),
            )
            .add_systems(OnExit(AppState::Picker), despawn_picker_ui);
    }
}

// --- Resources ---

#[derive(Resource, Default)]
pub struct SpeciesCatalog {
    pub species: Vec<Species>,
    pub load_error: Option<String>,
}

#[derive(Resource)]
pub struct PickerSelection {
    pub selected_index: Option<usize>,
    pub time_scale: TimeScale,
}

impl Default for PickerSelection {
    fn default() -> Self {
        Self {
            selected_index: None,
            time_scale: TimeScale::Seasonal,
        }
    }
}

// --- Markers ---

#[derive(Component)]
struct PickerRoot;

#[derive(Component)]
struct SpeciesButton {
    index: usize,
}

#[derive(Component)]
struct TimeScaleButton {
    scale: TimeScale,
}

#[derive(Component)]
struct ConfirmButton;

#[derive(Component)]
struct DetailPane;

#[derive(Component)]
struct DetailHeader;

#[derive(Component)]
struct DetailScientific;

#[derive(Component)]
struct DetailDifficulty;

#[derive(Component)]
struct DetailDifficultyText;

#[derive(Component)]
struct DetailTagline;

#[derive(Component)]
struct DetailDescription;

#[derive(Component)]
struct DetailFunFacts;

#[derive(Component)]
struct DetailKeeperNotes;

#[derive(Component)]
struct DetailStats;

// --- Setup ---

fn load_species_catalog(mut commands: Commands) {
    // Resolve via ANTCOLONY_ASSET_ROOT (set in main.rs) so this works
    // regardless of cwd — running the release exe by double-click or from
    // an installed location otherwise hits "io error 3" because
    // "assets/species" is relative to cwd.
    let species_path = std::env::var("ANTCOLONY_ASSET_ROOT")
        .map(|root| format!("{root}/species"))
        .unwrap_or_else(|_| SPECIES_DIR.to_string());
    match load_species_dir(&species_path) {
        Ok(species) if !species.is_empty() => {
            tracing::info!(count = species.len(), "Picker: loaded species catalog");
            commands.insert_resource(SpeciesCatalog {
                species,
                load_error: None,
            });
        }
        Ok(_) => {
            let msg = format!("No species TOML files found in {species_path}");
            tracing::error!("{}", msg);
            commands.insert_resource(SpeciesCatalog {
                species: Vec::new(),
                load_error: Some(msg),
            });
        }
        Err(e) => {
            let msg = format!("Failed to load species from {species_path}: {e}");
            tracing::error!("{}", msg);
            commands.insert_resource(SpeciesCatalog {
                species: Vec::new(),
                load_error: Some(msg),
            });
        }
    }
}

fn setup_picker_ui(
    mut commands: Commands,
    catalog: Res<SpeciesCatalog>,
    selection: Res<PickerSelection>,
) {
    tracing::info!("Picker: setup_picker_ui (AppState::Picker entered)");

    // Camera — needed so UI renders even before RenderPlugin's setup runs.
    commands.spawn((Camera2d, PickerRoot));

    // Root full-window panel.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.07, 0.10)),
            PickerRoot,
        ))
        .with_children(|root| {
            // Title.
            root.spawn((
                Text::new("Colony Keeper — Choose Your Species"),
                TextFont {
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.92, 0.55)),
                Node {
                    margin: UiRect::bottom(Val::Px(12.0)),
                    ..default()
                },
            ));

            if let Some(err) = &catalog.load_error {
                root.spawn((
                    Text::new(format!("ERROR: {err}\nPlace species TOMLs under assets/species/ and restart.")),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.4, 0.4)),
                ));
                return;
            }

            // Middle row: left list + right detail.
            root.spawn((Node {
                flex_direction: FlexDirection::Row,
                flex_grow: 1.0,
                column_gap: Val::Px(14.0),
                ..default()
            },))
                .with_children(|row| {
                    // Left pane — species list.
                    row.spawn((
                        Node {
                            width: Val::Px(340.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            padding: UiRect::all(Val::Px(8.0)),
                            overflow: Overflow::clip_y(),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.10, 0.12, 0.16, 0.9)),
                    ))
                    .with_children(|list| {
                        for (i, sp) in catalog.species.iter().enumerate() {
                            spawn_species_button(list, i, sp);
                        }
                    });

                    // Right pane — detail.
                    row.spawn((
                        Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(16.0)),
                            row_gap: Val::Px(8.0),
                            overflow: Overflow::clip_y(),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.10, 0.12, 0.16, 0.9)),
                        DetailPane,
                    ))
                    .with_children(|detail| {
                        detail.spawn((
                            Text::new("Select a species to see its details."),
                            TextFont { font_size: 20.0, ..default() },
                            TextColor(Color::srgb(0.95, 0.95, 0.95)),
                            DetailHeader,
                        ));
                        detail.spawn((
                            Text::new(""),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::srgb(0.75, 0.80, 0.95)),
                            DetailScientific,
                        ));
                        // Difficulty badge (coloured pill).
                        detail
                            .spawn((
                                Node {
                                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                                    align_self: AlignSelf::FlexStart,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                                DetailDifficulty,
                            ))
                            .with_children(|p| {
                                p.spawn((
                                    Text::new(""),
                                    TextFont { font_size: 12.0, ..default() },
                                    TextColor(Color::BLACK),
                                    DetailDifficultyText,
                                ));
                            });
                        detail.spawn((
                            Text::new(""),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::srgb(1.0, 0.85, 0.4)),
                            DetailTagline,
                        ));
                        detail.spawn((
                            Text::new(""),
                            TextFont { font_size: 13.0, ..default() },
                            TextColor(Color::srgb(0.88, 0.88, 0.88)),
                            DetailDescription,
                        ));
                        detail.spawn((
                            Text::new(""),
                            TextFont { font_size: 13.0, ..default() },
                            TextColor(Color::srgb(0.78, 0.90, 0.78)),
                            DetailFunFacts,
                        ));
                        detail.spawn((
                            Text::new(""),
                            TextFont { font_size: 13.0, ..default() },
                            TextColor(Color::srgb(0.95, 0.82, 0.62)),
                            DetailKeeperNotes,
                        ));
                        detail.spawn((
                            Text::new(""),
                            TextFont { font_size: 12.0, ..default() },
                            TextColor(Color::srgb(0.75, 0.75, 0.85)),
                            DetailStats,
                        ));
                    });
                });

            // Bottom bar — time-scale + confirm.
            root.spawn((Node {
                flex_direction: FlexDirection::Row,
                margin: UiRect::top(Val::Px(12.0)),
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            },))
                .with_children(|bar| {
                    bar.spawn((
                        Text::new("Time Scale:"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                    for scale in [
                        TimeScale::Realtime,
                        TimeScale::Brisk,
                        TimeScale::Seasonal,
                        TimeScale::Timelapse,
                    ] {
                        spawn_timescale_button(bar, scale, selection.time_scale);
                    }
                    // Spacer.
                    bar.spawn(Node {
                        flex_grow: 1.0,
                        ..default()
                    });
                    // Confirm button.
                    bar.spawn((
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.25, 0.25, 0.30)),
                        ConfirmButton,
                    ))
                    .with_children(|p| {
                        p.spawn((
                            Text::new("Start Colony"),
                            TextFont { font_size: 16.0, ..default() },
                            TextColor(Color::srgb(0.6, 0.6, 0.6)),
                        ));
                    });
                });
        });
}

fn spawn_species_button(parent: &mut ChildBuilder, index: usize, sp: &Species) {
    let swatch = parse_hex(&sp.appearance.color_hex);
    let (badge_bg, badge_label) = difficulty_style(sp.difficulty);

    parent
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Row,
                padding: UiRect::all(Val::Px(8.0)),
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.16, 0.18, 0.22, 1.0)),
            SpeciesButton { index },
        ))
        .with_children(|b| {
            // Color swatch.
            b.spawn((
                Node {
                    width: Val::Px(18.0),
                    height: Val::Px(18.0),
                    ..default()
                },
                BackgroundColor(swatch),
            ));
            // Text block.
            b.spawn((Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                row_gap: Val::Px(2.0),
                ..default()
            },))
                .with_children(|col| {
                    col.spawn((
                        Text::new(sp.common_name.clone()),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                    ));
                    col.spawn((
                        Text::new(sp.scientific_name()),
                        TextFont { font_size: 11.0, ..default() },
                        TextColor(Color::srgb(0.70, 0.78, 0.95)),
                    ));
                    col.spawn((
                        Text::new(truncate(&sp.encyclopedia.tagline, 54)),
                        TextFont { font_size: 11.0, ..default() },
                        TextColor(Color::srgb(0.85, 0.85, 0.85)),
                    ));
                });
            // Difficulty badge.
            b.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(badge_bg),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(badge_label),
                    TextFont { font_size: 10.0, ..default() },
                    TextColor(Color::BLACK),
                ));
            });
        });
}

fn spawn_timescale_button(parent: &mut ChildBuilder, scale: TimeScale, current: TimeScale) {
    let selected = scale == current;
    let bg = if selected {
        Color::srgb(0.35, 0.55, 0.85)
    } else {
        Color::srgb(0.20, 0.22, 0.26)
    };
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(bg),
            TimeScaleButton { scale },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(scale.label().to_string()),
                TextFont { font_size: 12.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
}

// --- Interaction systems ---

fn species_button_system(
    mut q: Query<(&Interaction, &SpeciesButton), (Changed<Interaction>, With<Button>)>,
    mut selection: ResMut<PickerSelection>,
) {
    for (interaction, btn) in q.iter_mut() {
        if *interaction == Interaction::Pressed {
            selection.selected_index = Some(btn.index);
            tracing::info!(index = btn.index, "Picker: species selected");
        }
    }
}

fn timescale_button_system(
    mut q: Query<(&Interaction, &TimeScaleButton), (Changed<Interaction>, With<Button>)>,
    mut selection: ResMut<PickerSelection>,
) {
    for (interaction, btn) in q.iter_mut() {
        if *interaction == Interaction::Pressed {
            selection.time_scale = btn.scale;
            tracing::info!(scale = btn.scale.label(), "Picker: time scale selected");
        }
    }
}

fn confirm_button_system(
    mut commands: Commands,
    mut q: Query<&Interaction, (Changed<Interaction>, With<ConfirmButton>)>,
    catalog: Res<SpeciesCatalog>,
    selection: Res<PickerSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for interaction in q.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(idx) = selection.selected_index else {
            tracing::warn!("Picker: confirm pressed with no species selected");
            continue;
        };
        let Some(species) = catalog.species.get(idx) else {
            tracing::error!(idx, "Picker: selected index out of range");
            continue;
        };
        let env = Environment {
            time_scale: selection.time_scale,
            ..Environment::default()
        };
        tracing::info!(
            species = %species.id,
            scale = env.time_scale.label(),
            "Picker: confirm -> transitioning to AppState::Running"
        );
        let state = SimulationState::from_species(species, &env);
        commands.insert_resource(state);
        next_state.set(AppState::Running);
    }
}

/// Debug helper: if `ANTCOLONY_AUTOSTART` env var is set to a species id,
/// boot straight into Running on that species. Used to reproduce
/// crashes that only manifest after picker confirm without needing an
/// interactive click. No-op when the var is unset.
fn autostart_from_env(
    mut commands: Commands,
    catalog: Res<SpeciesCatalog>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Ok(species_id) = std::env::var("ANTCOLONY_AUTOSTART") else {
        return;
    };
    let Some(species) = catalog.species.iter().find(|s| s.id == species_id) else {
        tracing::error!(species_id = %species_id, "ANTCOLONY_AUTOSTART: species not found in catalog");
        return;
    };
    let env = Environment::default();
    tracing::info!(species = %species.id, "ANTCOLONY_AUTOSTART: skipping picker, booting into Running");
    let state = SimulationState::from_species(species, &env);
    commands.insert_resource(state);
    next_state.set(AppState::Running);
}

/// Phase 4: pressing `V` on the picker launches the selected species
/// straight into the two-colony arena (black vs red AI) instead of the
/// solo keeper starter. No extra UI button — keeps the picker clean.
fn versus_key_launch_system(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    catalog: Res<SpeciesCatalog>,
    selection: Res<PickerSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }
    let Some(idx) = selection.selected_index else {
        tracing::warn!("Picker: V pressed with no species selected");
        return;
    };
    let Some(species) = catalog.species.get(idx) else {
        tracing::error!(idx, "Picker: V selected index out of range");
        return;
    };
    let env = Environment {
        time_scale: selection.time_scale,
        ..Environment::default()
    };
    tracing::info!(
        species = %species.id,
        scale = env.time_scale.label(),
        "Picker: V pressed -> launching two-colony arena (P4)"
    );
    let state = SimulationState::from_species_two_colony(species, &env);
    commands.insert_resource(state);
    next_state.set(AppState::Running);
}

// --- Display sync ---

#[allow(clippy::too_many_arguments)]
fn update_detail_pane(
    catalog: Res<SpeciesCatalog>,
    selection: Res<PickerSelection>,
    mut header: Query<&mut Text, (With<DetailHeader>, Without<DetailScientific>, Without<DetailTagline>, Without<DetailDescription>, Without<DetailFunFacts>, Without<DetailKeeperNotes>, Without<DetailStats>, Without<DetailDifficultyText>)>,
    mut scientific: Query<&mut Text, (With<DetailScientific>, Without<DetailHeader>, Without<DetailTagline>, Without<DetailDescription>, Without<DetailFunFacts>, Without<DetailKeeperNotes>, Without<DetailStats>, Without<DetailDifficultyText>)>,
    mut tagline: Query<&mut Text, (With<DetailTagline>, Without<DetailHeader>, Without<DetailScientific>, Without<DetailDescription>, Without<DetailFunFacts>, Without<DetailKeeperNotes>, Without<DetailStats>, Without<DetailDifficultyText>)>,
    mut description: Query<&mut Text, (With<DetailDescription>, Without<DetailHeader>, Without<DetailScientific>, Without<DetailTagline>, Without<DetailFunFacts>, Without<DetailKeeperNotes>, Without<DetailStats>, Without<DetailDifficultyText>)>,
    mut fun_facts: Query<&mut Text, (With<DetailFunFacts>, Without<DetailHeader>, Without<DetailScientific>, Without<DetailTagline>, Without<DetailDescription>, Without<DetailKeeperNotes>, Without<DetailStats>, Without<DetailDifficultyText>)>,
    mut keeper_notes: Query<&mut Text, (With<DetailKeeperNotes>, Without<DetailHeader>, Without<DetailScientific>, Without<DetailTagline>, Without<DetailDescription>, Without<DetailFunFacts>, Without<DetailStats>, Without<DetailDifficultyText>)>,
    mut stats: Query<&mut Text, (With<DetailStats>, Without<DetailHeader>, Without<DetailScientific>, Without<DetailTagline>, Without<DetailDescription>, Without<DetailFunFacts>, Without<DetailKeeperNotes>, Without<DetailDifficultyText>)>,
    mut badge_text: Query<&mut Text, (With<DetailDifficultyText>, Without<DetailHeader>, Without<DetailScientific>, Without<DetailTagline>, Without<DetailDescription>, Without<DetailFunFacts>, Without<DetailKeeperNotes>, Without<DetailStats>)>,
    mut badge_bg: Query<&mut BackgroundColor, With<DetailDifficulty>>,
) {
    let Some(idx) = selection.selected_index else {
        return;
    };
    let Some(sp) = catalog.species.get(idx) else {
        return;
    };

    for mut t in header.iter_mut() {
        **t = sp.common_name.clone();
    }
    for mut t in scientific.iter_mut() {
        **t = sp.scientific_name();
    }
    for mut t in tagline.iter_mut() {
        **t = format!("\"{}\"", sp.encyclopedia.tagline);
    }
    for mut t in description.iter_mut() {
        **t = sp.encyclopedia.description.clone();
    }
    for mut t in fun_facts.iter_mut() {
        if sp.encyclopedia.fun_facts.is_empty() {
            **t = String::new();
        } else {
            let mut s = String::from("Fun facts:\n");
            for f in &sp.encyclopedia.fun_facts {
                s.push_str("  • ");
                s.push_str(f);
                s.push('\n');
            }
            **t = s;
        }
    }
    for mut t in keeper_notes.iter_mut() {
        if sp.encyclopedia.keeper_notes.trim().is_empty() {
            **t = String::new();
        } else {
            **t = format!("Keeper notes: {}", sp.encyclopedia.keeper_notes);
        }
    }
    for mut t in stats.iter_mut() {
        **t = format!(
            "Range: {}\nQueen lifespan: {:.1} years | Worker lifespan: {:.1} months\nMature population: {} | Hibernation required: {}",
            sp.encyclopedia.real_world_range,
            sp.biology.queen_lifespan_years,
            sp.biology.worker_lifespan_months,
            sp.growth.target_population,
            if sp.biology.hibernation_required { "yes" } else { "no" },
        );
    }
    let (bg, label) = difficulty_style(sp.difficulty);
    for mut t in badge_text.iter_mut() {
        **t = label.to_string();
    }
    for mut b in badge_bg.iter_mut() {
        *b = BackgroundColor(bg);
    }
}

fn update_species_button_highlights(
    selection: Res<PickerSelection>,
    mut q: Query<(&SpeciesButton, &mut BackgroundColor)>,
) {
    for (btn, mut bg) in q.iter_mut() {
        let selected = Some(btn.index) == selection.selected_index;
        *bg = BackgroundColor(if selected {
            Color::srgba(0.28, 0.36, 0.50, 1.0)
        } else {
            Color::srgba(0.16, 0.18, 0.22, 1.0)
        });
    }
}

fn update_timescale_button_highlights(
    selection: Res<PickerSelection>,
    mut q: Query<(&TimeScaleButton, &mut BackgroundColor)>,
) {
    for (btn, mut bg) in q.iter_mut() {
        let selected = btn.scale == selection.time_scale;
        *bg = BackgroundColor(if selected {
            Color::srgb(0.35, 0.55, 0.85)
        } else {
            Color::srgb(0.20, 0.22, 0.26)
        });
    }
}

fn update_confirm_button_state(
    selection: Res<PickerSelection>,
    mut bg_q: Query<&mut BackgroundColor, With<ConfirmButton>>,
    mut text_q: Query<&mut TextColor, Without<ConfirmButton>>,
    children_q: Query<&Children, With<ConfirmButton>>,
) {
    let enabled = selection.selected_index.is_some();
    for mut bg in bg_q.iter_mut() {
        *bg = BackgroundColor(if enabled {
            Color::srgb(0.30, 0.70, 0.35)
        } else {
            Color::srgb(0.25, 0.25, 0.30)
        });
    }
    for children in children_q.iter() {
        for child in children.iter() {
            if let Ok(mut tc) = text_q.get_mut(*child) {
                *tc = TextColor(if enabled {
                    Color::srgb(1.0, 1.0, 1.0)
                } else {
                    Color::srgb(0.6, 0.6, 0.6)
                });
            }
        }
    }
}

fn despawn_picker_ui(mut commands: Commands, q: Query<Entity, With<PickerRoot>>) {
    let n = q.iter().count();
    for e in q.iter() {
        commands.entity(e).despawn_recursive();
    }
    tracing::info!(despawned = n, "Picker: despawned picker UI, leaving AppState::Picker");
}

// --- Helpers ---

pub(crate) fn difficulty_style(d: Difficulty) -> (Color, &'static str) {
    match d {
        Difficulty::Beginner => (Color::srgb(0.30, 0.80, 0.30), "Beginner"),
        Difficulty::Intermediate => (Color::srgb(0.95, 0.75, 0.20), "Intermediate"),
        Difficulty::Advanced => (Color::srgb(0.95, 0.55, 0.20), "Advanced"),
        Difficulty::Expert => (Color::srgb(0.90, 0.25, 0.25), "Expert"),
    }
}

pub(crate) fn parse_hex(hex: &str) -> Color {
    let s = hex.trim().trim_start_matches('#');
    if s.len() != 6 {
        return Color::srgb(0.5, 0.5, 0.5);
    }
    let parse_pair = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    match (parse_pair(0), parse_pair(2), parse_pair(4)) {
        (Some(r), Some(g), Some(b)) => Color::srgb_u8(r, g, b),
        _ => Color::srgb(0.5, 0.5, 0.5),
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

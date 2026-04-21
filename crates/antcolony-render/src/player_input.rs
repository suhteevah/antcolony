//! Phase 7 player-interaction input + render layer.
//!
//! Wires the sim-side helpers (`possess_nearest`, `set_player_heading`,
//! `recruit_nearby`, `dismiss_followers`, `place_beacon`) to keyboard
//! + mouse + a handful of overlay sprites.
//!
//! Keys:
//! - `F`                — possess-nearest worker at cursor (player colony = 0)
//! - `R`                — recruit nearby ants under the current avatar
//! - `Shift+R`          — dismiss all of the avatar's followers
//! - `Q`                — toggle beacon mode (Gather ↔ Attack)
//! - Right-click        — drop a beacon at the cursor (in the hovered module)
//! - `W`/`A`/`S`/`D`    — when possessed, steer the avatar (else pan camera)
//!
//! Render:
//! - `PlayerAvatarOverlay` child sprite on every ant — yellow ring, shown
//!   only when `ant.is_player` is true.
//! - `FollowerRing` child sprite — cyan ring, shown when the ant has a
//!   `follow_leader`.
//! - `BeaconSprite` synced against `Simulation::beacons` by id.
//! - `PlayerStatusText` HUD line (top-right) summarising avatar state.

use antcolony_game::SimulationState;
use antcolony_sim::{BeaconKind, ModuleId};
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;

use crate::AppState;
use crate::plugin::{FormicariumEntity, ModuleRect, TILE};

/// Which beacon flavour the next right-click will place.
#[derive(Resource, Debug, Clone, Copy)]
pub struct BeaconMode(pub BeaconKind);

impl Default for BeaconMode {
    fn default() -> Self {
        BeaconMode(BeaconKind::Gather)
    }
}

/// Phase-7 player-colony tag — the colony id the keyboard drives. For
/// the Keeper starter this is always 0; in versus mode the picker
/// leaves it at 0 too (the AI colony is colony 2 there).
#[derive(Resource, Debug, Clone, Copy)]
pub struct PlayerColony(pub u8);

impl Default for PlayerColony {
    fn default() -> Self {
        PlayerColony(0)
    }
}

/// Child sprite on every ant — yellow ring, visible only when the
/// ant is the player avatar.
#[derive(Component)]
pub struct PlayerAvatarOverlay {
    pub ant_idx: u32,
}

/// Child sprite on every ant — small cyan outline, visible only when
/// the ant has a `follow_leader`.
#[derive(Component)]
pub struct FollowerRing {
    pub ant_idx: u32,
}

/// One sprite per beacon id, synced each frame.
#[derive(Component)]
pub struct BeaconSprite(pub u32);

/// HUD line.
#[derive(Component)]
pub struct PlayerStatusText;

pub struct PlayerInputPlugin;

impl Plugin for PlayerInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BeaconMode>()
            .init_resource::<PlayerColony>()
            .add_systems(OnEnter(AppState::Running), setup_status_text)
            .add_systems(
                Update,
                (
                    possess_at_cursor,
                    toggle_beacon_mode,
                    place_beacon_at_cursor,
                    steer_avatar_with_wasd,
                    recruit_or_dismiss,
                    sync_player_overlay_visibility,
                    sync_follower_ring_visibility,
                    sync_beacon_sprites,
                    update_player_status_text,
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn setup_status_text(mut commands: Commands) {
    commands.spawn((
        Text::new("avatar: none   beacon: gather"),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.92, 0.45)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(10.0),
            ..default()
        },
        PlayerStatusText,
    ));
}

fn cursor_world_pos(
    windows: &Query<&Window>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) -> Option<Vec2> {
    let window = windows.get_single().ok()?;
    let cursor = window.cursor_position()?;
    let (camera, cam_tf) = cameras.get_single().ok()?;
    camera.viewport_to_world_2d(cam_tf, cursor).ok()
}

/// Translate a world-space cursor position into `(module_id, cell_pos)`
/// for the module whose `ModuleRect` contains it, if any.
fn cursor_to_module_cell(
    world: Vec2,
    q_modules: &Query<&ModuleRect>,
) -> Option<(ModuleId, Vec2)> {
    for m in q_modules.iter() {
        if world.x >= m.min.x
            && world.x <= m.max.x
            && world.y >= m.min.y
            && world.y <= m.max.y
        {
            let cell = Vec2::new(
                (world.x - m.min.x) / TILE,
                (world.y - m.min.y) / TILE,
            );
            return Some((m.id, cell));
        }
    }
    None
}

fn possess_at_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimulationState>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    q_modules: Query<&ModuleRect>,
    colony: Res<PlayerColony>,
) {
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Some(world) = cursor_world_pos(&windows, &cameras) else {
        return;
    };
    let Some((mid, cell)) = cursor_to_module_cell(world, &q_modules) else {
        tracing::info!("F: cursor not over any module");
        return;
    };
    match sim.sim.possess_nearest(colony.0, mid, cell) {
        Some(id) => tracing::info!(ant_id = id, module = mid, "F: possessed"),
        None => tracing::info!(module = mid, "F: no candidate ant to possess"),
    }
}

fn toggle_beacon_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<BeaconMode>,
) {
    if !keys.just_pressed(KeyCode::KeyQ) {
        return;
    }
    mode.0 = match mode.0 {
        BeaconKind::Gather => BeaconKind::Attack,
        BeaconKind::Attack => BeaconKind::Gather,
    };
    tracing::info!(mode = ?mode.0, "beacon mode toggled");
}

fn place_beacon_at_cursor(
    mouse: Res<ButtonInput<MouseButton>>,
    mode: Res<BeaconMode>,
    colony: Res<PlayerColony>,
    mut sim: ResMut<SimulationState>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    q_modules: Query<&ModuleRect>,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(world) = cursor_world_pos(&windows, &cameras) else {
        return;
    };
    let Some((mid, cell)) = cursor_to_module_cell(world, &q_modules) else {
        return;
    };
    // 2.0/tick for 600 ticks = 1200 pheromone units total, well above
    // the exploration-trail noise floor but not permanent.
    let id = sim.sim.place_beacon(mode.0, mid, cell, 2.0, 600, colony.0);
    tracing::info!(id, kind = ?mode.0, module = mid, "right-click beacon placed");
}

/// When an ant is possessed, WASD drives its heading instead of the
/// camera. The sim's movement system reads `ant.heading` and steps the
/// avatar along it each tick, so this is all we need to write.
fn steer_avatar_with_wasd(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimulationState>,
) {
    if sim.sim.player_ant_index().is_none() {
        return;
    }
    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        dir.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        dir.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        dir.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        dir.x += 1.0;
    }
    if dir.length_squared() < 0.01 {
        return;
    }
    let heading = dir.y.atan2(dir.x);
    sim.sim.set_player_heading(heading);
}

fn recruit_or_dismiss(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimulationState>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    let Some(idx) = sim.sim.player_ant_index() else {
        tracing::info!("R: no avatar possessed");
        return;
    };
    let leader_id = sim.sim.ants[idx].id;
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if shift {
        sim.sim.dismiss_followers(leader_id);
    } else {
        let n = sim.sim.recruit_nearby(leader_id, 6.0, 8);
        tracing::info!(leader_id, recruited = n, "R: recruited");
    }
}

fn sync_player_overlay_visibility(
    sim: Res<SimulationState>,
    mut q: Query<(&PlayerAvatarOverlay, &mut Visibility)>,
) {
    for (overlay, mut vis) in q.iter_mut() {
        let visible = sim
            .sim
            .ants
            .get(overlay.ant_idx as usize)
            .map(|a| a.is_player)
            .unwrap_or(false);
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn sync_follower_ring_visibility(
    sim: Res<SimulationState>,
    mut q: Query<(&FollowerRing, &mut Visibility)>,
) {
    for (ring, mut vis) in q.iter_mut() {
        let visible = sim
            .sim
            .ants
            .get(ring.ant_idx as usize)
            .map(|a| a.follow_leader.is_some())
            .unwrap_or(false);
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn sync_beacon_sprites(
    mut commands: Commands,
    sim: Res<SimulationState>,
    q_modules: Query<&ModuleRect>,
    existing: Query<(Entity, &BeaconSprite)>,
) {
    use std::collections::{HashMap, HashSet};
    let mut by_id: HashMap<u32, Entity> = HashMap::new();
    for (e, s) in existing.iter() {
        by_id.insert(s.0, e);
    }
    let live: HashSet<u32> = sim.sim.beacons.iter().map(|b| b.id).collect();
    for (e, s) in existing.iter() {
        if !live.contains(&s.0) {
            commands.entity(e).despawn_recursive();
        }
    }

    let module_origin = |mid: ModuleId| -> Option<Vec2> {
        q_modules
            .iter()
            .find(|m| m.id == mid)
            .map(|m| m.min)
    };

    for b in &sim.sim.beacons {
        let Some(origin) = module_origin(b.module_id) else {
            continue;
        };
        let pos = Vec2::new(
            origin.x + b.position.x * TILE,
            origin.y + b.position.y * TILE,
        );
        // Alpha fades with ticks_remaining so ageing beacons visibly dim.
        let fade = (b.ticks_remaining as f32 / 600.0).clamp(0.15, 1.0);
        let (color, outer) = match b.kind {
            // Gather: green-gold pulse.
            BeaconKind::Gather => (Color::srgba(0.3, 0.95, 0.45, 0.85 * fade), TILE * 2.2),
            // Attack: bright red-orange pulse.
            BeaconKind::Attack => (Color::srgba(0.98, 0.25, 0.15, 0.9 * fade), TILE * 2.2),
        };
        if let Some(&e) = by_id.get(&b.id) {
            commands.entity(e).insert((
                Sprite {
                    color,
                    custom_size: Some(Vec2::splat(outer)),
                    ..default()
                },
                Transform::from_translation(pos.extend(2.5)),
            ));
        } else {
            commands.spawn((
                Sprite {
                    color,
                    custom_size: Some(Vec2::splat(outer)),
                    ..default()
                },
                Transform::from_translation(pos.extend(2.5)),
                BeaconSprite(b.id),
                FormicariumEntity,
            ));
        }
    }
}

fn update_player_status_text(
    sim: Res<SimulationState>,
    mode: Res<BeaconMode>,
    mut q: Query<&mut Text, With<PlayerStatusText>>,
) {
    let Ok(mut text) = q.get_single_mut() else {
        return;
    };
    let beacon_label = match mode.0 {
        BeaconKind::Gather => "gather",
        BeaconKind::Attack => "attack",
    };
    let beacons_active = sim.sim.beacons.len();
    let avatar_line = match sim.sim.player_ant_index() {
        None => "avatar: none".to_string(),
        Some(i) => {
            let a = &sim.sim.ants[i];
            let leader_id = a.id;
            let followers = sim
                .sim
                .ants
                .iter()
                .filter(|x| x.follow_leader == Some(leader_id))
                .count();
            format!(
                "avatar: #{} {:?} hp={:.0} food={:.1} followers={}",
                a.id, a.state, a.health, a.food_carried, followers,
            )
        }
    };
    text.0 = format!(
        "{avatar_line}\nbeacon[{beacon_label}]  active={beacons_active}  (F possess · R recruit · Shift+R dismiss · Q toggle · RMB place)"
    );
}

//! PvP networked play -- Bevy integration of `antcolony-net`.
//!
//! # Lifecycle
//!
//! 1. Picker. Player presses `H` to host or `J` to join.
//!    - Host listens on `ANTCOLONY_PEER_PORT` (default 17001) and waits
//!      for a connection. **Bevy main thread blocks** for up to 60s.
//!    - Join reads `ANTCOLONY_PEER_ADDR` (default `127.0.0.1:17001`)
//!      and connects.
//! 2. Handshake -- exchange Hellos, validate version + seed + role.
//! 3. Both peers build `SimulationState::from_species_pvp` (same seed!)
//!    and transition to `AppState::Running`. The `PvpClient` resource
//!    is inserted holding the `LockstepPeer`.
//! 4. Each `FixedUpdate`, the `pvp_exchange_system` runs *before* the
//!    sim tick. On decision ticks (every `DECISION_CADENCE` ticks) it
//!    sends our `AiDecision` and blocks reading the partner's, then
//!    applies both. State hashes are compared every exchange; mismatch
//!    means desync and we abort to the picker with an error overlay.
//! 5. Match end (`match_status() != InProgress`) -> show full-screen
//!    overlay and disconnect cleanly.
//!
//! # Thread model
//!
//! For tonight's V1 we run net I/O on the Bevy main thread inside the
//! FixedUpdate schedule. LAN / Tailscale latency (~1-30ms) is well
//! below a 33ms tick budget so the freeze is invisible. WAN play would
//! need a worker-thread refactor; deferred to N4.
//!
//! # Input
//!
//! Crude but functional for V1: number-row keys nudge the local colony's
//! strategy. The buffered nudge becomes the next `AiDecision` sent over
//! the wire.
//!
//! - `1` / `2` / `3` -- bias caste ratio toward Worker / Soldier / Breeder
//! - `4` / `5` / `6` -- bias behavior weights toward Forage / Dig / Nurse
//!
//! Each press shifts the relevant slot by `INPUT_NUDGE_STEP` (default 0.05)
//! and renormalizes the triplet. So holding `2` ramps soldier production.

use std::env;
use std::time::Duration;

use antcolony_game::SimulationState;
use antcolony_net::{
    sim_state_hash, DECISION_CADENCE, LockstepPeer, PeerConfig, PeerRole, TickInput,
    transport::{connect, host},
};
use antcolony_sim::{AiDecision, MatchStatus, Species, Environment};
use bevy::prelude::*;

use crate::AppState;

const DEFAULT_PORT: u16 = 17001;
const DEFAULT_PEER_ADDR: &str = "127.0.0.1:17001";
const INPUT_NUDGE_STEP: f32 = 0.05;
const HANDSHAKE_TIMEOUT_SECS: u64 = 60;
const RECV_TIMEOUT_SECS: u64 = 30;

/// Per-match config hash. Just a stable byte derived from the agreed
/// arena setup. If we ever expose tunables in the picker, fold them in.
fn pvp_config_hash() -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in b"antcolony_pvp_v1_two_colony_arena" {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Active PvP connection + per-tick bookkeeping. Inserted on H/J,
/// removed on disconnect / match-end.
#[derive(Resource)]
pub struct PvpClient {
    pub peer: LockstepPeer,
    /// Our colony id (0 = Black/host, 1 = Red/joiner).
    pub local_colony: u8,
    /// Counter of decision exchanges. Bumped whenever sim.tick crosses
    /// a DECISION_CADENCE boundary.
    pub decision_tick: u64,
    /// True once the match has resolved -- skip further exchanges.
    pub match_ended: bool,
}

/// Buffered local input applied at the next decision tick. Number keys
/// nudge the values; the renormalized triple is sent over the wire.
#[derive(Resource, Debug, Clone)]
pub struct PvpInputBuffer {
    pub caste_worker: f32,
    pub caste_soldier: f32,
    pub caste_breeder: f32,
    pub forage: f32,
    pub dig: f32,
    pub nurse: f32,
}

impl Default for PvpInputBuffer {
    fn default() -> Self {
        Self {
            caste_worker: 0.65, caste_soldier: 0.30, caste_breeder: 0.05,
            forage: 0.55, dig: 0.20, nurse: 0.25,
        }
    }
}

impl PvpInputBuffer {
    fn renormalize(&mut self) {
        let cs = self.caste_worker + self.caste_soldier + self.caste_breeder;
        if cs > 1e-6 {
            self.caste_worker /= cs;
            self.caste_soldier /= cs;
            self.caste_breeder /= cs;
        }
        let bs = self.forage + self.dig + self.nurse;
        if bs > 1e-6 {
            self.forage /= bs;
            self.dig /= bs;
            self.nurse /= bs;
        }
    }

    fn to_decision(&self) -> AiDecision {
        AiDecision {
            caste_ratio_worker: self.caste_worker,
            caste_ratio_soldier: self.caste_soldier,
            caste_ratio_breeder: self.caste_breeder,
            forage_weight: self.forage,
            dig_weight: self.dig,
            nurse_weight: self.nurse,
            research_choice: None,
        }
    }
}

/// Shown after queen-kill resolves. Only one match per app-run for V1.
#[derive(Resource, Debug, Clone)]
pub struct PvpMatchOutcome {
    pub status: MatchStatus,
    pub local_colony: u8,
}

/// HUD anchor.
#[derive(Component)]
pub struct PvpHud;

#[derive(Component)]
pub struct PvpStatusText;

#[derive(Component)]
pub struct PvpOutcomeOverlay;

pub struct PvpClientPlugin;

impl Plugin for PvpClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PvpInputBuffer>()
            .add_systems(Update, picker_pvp_keys.run_if(in_state(AppState::Picker)))
            .add_systems(OnEnter(AppState::Running), setup_pvp_hud.run_if(resource_exists::<PvpClient>))
            .add_systems(
                FixedUpdate,
                pvp_exchange_system
                    .run_if(resource_exists::<PvpClient>)
                    .run_if(resource_exists::<SimulationState>)
                    .before(antcolony_game::SimSet::Tick),
            )
            .add_systems(
                Update,
                (
                    pvp_input_keys,
                    update_pvp_status_text,
                    show_outcome_overlay.run_if(resource_added::<PvpMatchOutcome>),
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

/// Pick a species deterministically for V1 -- first species in the
/// catalog. Both peers must agree, so we use index 0 unless the picker
/// has a selection.
fn pick_species_for_pvp(catalog: &crate::picker::SpeciesCatalog, sel: &crate::picker::PickerSelection) -> Option<Species> {
    let idx = sel.selected_index.unwrap_or(0);
    catalog.species.get(idx).cloned()
}

/// Read the H/J keys in the picker; on press, do the handshake and
/// transition to Running.
fn picker_pvp_keys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    catalog: Res<crate::picker::SpeciesCatalog>,
    selection: Res<crate::picker::PickerSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let host_pressed = keys.just_pressed(KeyCode::KeyH);
    let join_pressed = keys.just_pressed(KeyCode::KeyJ);
    if !host_pressed && !join_pressed {
        return;
    }
    let Some(species) = pick_species_for_pvp(&catalog, &selection) else {
        tracing::error!("PvP: no species in catalog -- cannot start");
        return;
    };
    let env = Environment {
        time_scale: selection.time_scale,
        ..Environment::default()
    };

    // Both peers must agree on the seed. For V1 we use the env seed
    // (default 0), so both Host and Join arrive at the same number
    // unless someone overrides via $ANTCOLONY_SEED.
    let seed = env::var("ANTCOLONY_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(env.seed);
    let env = Environment { seed, ..env };

    let role = if host_pressed { PeerRole::Black } else { PeerRole::Red };
    let display_name = env::var("ANTCOLONY_NAME").unwrap_or_else(|_| format!("{:?}", role));

    let stream = if host_pressed {
        let port: u16 = env::var("ANTCOLONY_PEER_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_PORT);
        tracing::info!(port, "PvP: hosting -- waiting for peer (Bevy will freeze briefly)");
        match host(("0.0.0.0", port)) {
            Ok(s) => s,
            Err(e) => { tracing::error!(?e, "PvP: host failed"); return; }
        }
    } else {
        let addr = env::var("ANTCOLONY_PEER_ADDR").unwrap_or_else(|_| DEFAULT_PEER_ADDR.into());
        tracing::info!(%addr, "PvP: joining");
        match connect(addr.as_str()) {
            Ok(s) => s,
            Err(e) => { tracing::error!(?e, "PvP: connect failed"); return; }
        }
    };

    let cfg = PeerConfig {
        role,
        seed,
        config_hash: pvp_config_hash(),
        display_name,
        recv_timeout: Some(Duration::from_secs(HANDSHAKE_TIMEOUT_SECS)),
    };
    let mut peer = match LockstepPeer::new(stream, cfg) {
        Ok(p) => p,
        Err(e) => { tracing::error!(?e, "PvP: peer init failed"); return; }
    };
    if let Err(e) = peer.handshake() {
        tracing::error!(?e, "PvP: handshake failed");
        return;
    }

    // Now lower the per-tick recv timeout for gameplay.
    if let Err(e) = peer.set_recv_timeout(Some(Duration::from_secs(RECV_TIMEOUT_SECS))) {
        tracing::warn!(?e, "PvP: failed to lower recv timeout post-handshake");
    }

    let state = SimulationState::from_species_pvp(&species, &env);
    let local_colony = role.colony_id();
    tracing::info!(local_colony, ?role, "PvP: handshake done -- entering match");
    commands.insert_resource(state);
    commands.insert_resource(PvpClient {
        peer,
        local_colony,
        decision_tick: 0,
        match_ended: false,
    });
    next_state.set(AppState::Running);
}

/// Number-row input -> nudge the local input buffer.
fn pvp_input_keys(
    keys: Res<ButtonInput<KeyCode>>,
    pvp: Option<Res<PvpClient>>,
    mut buf: ResMut<PvpInputBuffer>,
) {
    if pvp.is_none() { return; }
    let mut changed = false;
    if keys.pressed(KeyCode::Digit1) { buf.caste_worker += INPUT_NUDGE_STEP; changed = true; }
    if keys.pressed(KeyCode::Digit2) { buf.caste_soldier += INPUT_NUDGE_STEP; changed = true; }
    if keys.pressed(KeyCode::Digit3) { buf.caste_breeder += INPUT_NUDGE_STEP; changed = true; }
    if keys.pressed(KeyCode::Digit4) { buf.forage += INPUT_NUDGE_STEP; changed = true; }
    if keys.pressed(KeyCode::Digit5) { buf.dig += INPUT_NUDGE_STEP; changed = true; }
    if keys.pressed(KeyCode::Digit6) { buf.nurse += INPUT_NUDGE_STEP; changed = true; }
    if changed { buf.renormalize(); }
}

/// Run before sim.tick(): on decision ticks, exchange + apply decisions.
/// On any net error, log + drop the PvpClient resource (sim continues
/// solo so the player isn't kicked to a black screen).
fn pvp_exchange_system(
    mut commands: Commands,
    mut sim: ResMut<SimulationState>,
    mut pvp: ResMut<PvpClient>,
    buf: Res<PvpInputBuffer>,
) {
    if pvp.match_ended {
        return;
    }
    if sim.sim.tick % DECISION_CADENCE != 0 {
        return;
    }
    let local_decision = buf.to_decision();
    let local_hash = sim_state_hash(&sim.sim);
    let ours = TickInput {
        tick: pvp.decision_tick,
        decision: local_decision.clone(),
        state_hash: local_hash,
    };
    let remote = match pvp.peer.exchange_tick(ours) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(?e, tick = pvp.decision_tick, "PvP: exchange failed -- dropping connection");
            commands.remove_resource::<PvpClient>();
            return;
        }
    };
    let local_colony = pvp.local_colony;
    let remote_colony = if local_colony == 0 { 1 } else { 0 };
    sim.sim.apply_ai_decision(local_colony, &local_decision);
    sim.sim.apply_ai_decision(remote_colony, &remote.decision);
    pvp.decision_tick += 1;

    // Match-end check after applying both decisions.
    let status = sim.sim.match_status();
    if !matches!(status, MatchStatus::InProgress) {
        let _ = pvp.peer.send_disconnect(format!("match resolved: {:?}", status));
        pvp.match_ended = true;
        commands.insert_resource(PvpMatchOutcome { status, local_colony });
        tracing::info!(?status, local_colony, "PvP: match resolved");
    }
}

fn setup_pvp_hud(mut commands: Commands) {
    commands
        .spawn((
            PvpHud,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                left: Val::Px(8.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.85)),
        ))
        .with_children(|p| {
            p.spawn((
                PvpStatusText,
                Text::new("PvP: connecting..."),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.95, 0.95, 1.0)),
            ));
        });
}

fn update_pvp_status_text(
    sim: Option<Res<SimulationState>>,
    pvp: Option<Res<PvpClient>>,
    buf: Res<PvpInputBuffer>,
    mut q: Query<&mut Text, With<PvpStatusText>>,
) {
    let Ok(mut text) = q.get_single_mut() else { return; };
    let Some(sim) = sim else { return; };
    let role_label = match pvp.as_ref() {
        Some(p) if p.local_colony == 0 => "Black (host)",
        Some(p) if p.local_colony == 1 => "Red (joiner)",
        Some(_) => "?",
        None => "DISCONNECTED",
    };
    let (my_food, my_workers, my_soldiers) = sim.sim.colonies.iter()
        .find(|c| pvp.as_ref().map(|p| c.id == p.local_colony).unwrap_or(false))
        .map(|c| (c.food_stored, c.population.workers, c.population.soldiers))
        .unwrap_or((0.0, 0, 0));
    let tick = sim.sim.tick;
    let dec = pvp.as_ref().map(|p| p.decision_tick).unwrap_or(0);
    text.0 = format!(
        "PvP {role_label}\ntick {tick} / dec {dec}\nfood {my_food:.0} / W {my_workers} S {my_soldiers}\n\
         strat: W={:.2} S={:.2} B={:.2} | F={:.2} D={:.2} N={:.2}\n\
         keys 1/2/3 caste, 4/5/6 behavior",
        buf.caste_worker, buf.caste_soldier, buf.caste_breeder,
        buf.forage, buf.dig, buf.nurse,
    );
}

fn show_outcome_overlay(mut commands: Commands, outcome: Res<PvpMatchOutcome>) {
    let (msg, color) = match outcome.status {
        MatchStatus::Won { winner, .. } if winner == outcome.local_colony => ("VICTORY", Color::srgb(0.2, 0.95, 0.4)),
        MatchStatus::Won { .. } => ("DEFEAT", Color::srgb(0.95, 0.3, 0.3)),
        MatchStatus::Draw { .. } => ("DRAW", Color::srgb(0.9, 0.9, 0.4)),
        MatchStatus::InProgress => return,
    };
    commands.spawn((
        PvpOutcomeOverlay,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(40.0),
            left: Val::Percent(0.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
    )).with_children(|p| {
        p.spawn((
            Text::new(msg),
            TextFont { font_size: 96.0, ..default() },
            TextColor(color),
        ));
    });
}


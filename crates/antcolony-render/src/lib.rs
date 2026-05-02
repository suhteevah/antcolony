//! Rendering plugin for the ant colony simulation.

pub mod atlas;
pub mod editor;
pub mod encyclopedia;
pub mod inspector;
pub mod narrator;
pub mod picker;
pub mod player_input;
pub mod plugin;
pub mod save_ui;
pub mod substrate;
pub mod timeline;
pub mod ui;

use bevy::prelude::States;

pub use atlas::{SpriteAtlas, SpriteAtlasPlugin};
pub use narrator::{Chronicler, ChroniclerPlugin, ColonyChronicle};
pub use editor::EditorPlugin;
pub use encyclopedia::EncyclopediaPlugin;
pub use picker::PickerPlugin;
pub use player_input::PlayerInputPlugin;
pub use plugin::RenderPlugin;
pub use save_ui::SaveUiPlugin;
pub use ui::UiPlugin;

/// Top-level app flow: the player picks a species in `Picker`, then the
/// simulation runs in `Running`. Systems that touch `SimulationState` are
/// gated on `AppState::Running` via `.run_if(in_state(AppState::Running))`
/// or `OnEnter(AppState::Running)`.
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    Picker,
    Running,
}

//! antcolony binary entry point.
//!
//! Wires the simulation + Bevy integration + rendering plugins together.
//! The render plugin owns the picker-then-running AppState flow; no
//! simulation is created until the player chooses a species.

use bevy::log::LogPlugin;
use bevy::prelude::*;

fn main() -> anyhow::Result<()> {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "antcolony — ant colony simulation".into(),
                        resolution: (1280.0, 800.0).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(LogPlugin {
                    level: bevy::log::Level::INFO,
                    filter:
                        "wgpu=warn,naga=warn,antcolony=info,antcolony_sim=info,antcolony_game=info,antcolony_render=info"
                            .into(),
                    ..default()
                }),
        )
        .add_plugins(antcolony_game::SimulationPlugin::default())
        .add_plugins(antcolony_render::RenderPlugin)
        .run();

    Ok(())
}

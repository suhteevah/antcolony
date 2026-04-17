use antcolony_game::SimulationState;
use antcolony_sim::{AntState, PheromoneLayer, Terrain};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

use crate::AppState;

const TILE: f32 = 4.0;

pub struct RenderPlugin;

#[derive(Component)]
struct AntSprite(pub u32);

#[derive(Component)]
struct PheromoneOverlay;

#[derive(Resource)]
struct PheromoneTexture(pub Handle<Image>);

#[derive(Resource, Default)]
struct OverlayState {
    pub visible: bool,
}

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_plugins(crate::picker::PickerPlugin)
            .add_plugins(crate::ui::UiPlugin)
            .add_plugins(crate::encyclopedia::EncyclopediaPlugin)
            .insert_resource(OverlayState { visible: true })
            .add_systems(OnEnter(AppState::Running), setup)
            .add_systems(
                Update,
                (
                    sync_ant_sprites,
                    update_pheromone_texture,
                    toggle_overlay_input,
                    camera_controls,
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn setup(
    mut commands: Commands,
    sim: Res<SimulationState>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let w = sim.sim.world.width as u32;
    let h = sim.sim.world.height as u32;
    let world_w = sim.sim.world.width as f32 * TILE;
    let world_h = sim.sim.world.height as f32 * TILE;

    commands.spawn(Camera2d);

    // Pheromone overlay texture.
    let mut img = Image::new_fill(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0u8, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD
            | bevy::render::render_asset::RenderAssetUsages::MAIN_WORLD,
    );
    img.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    let tex = images.add(img);

    commands.spawn((
        Sprite {
            image: tex.clone(),
            custom_size: Some(Vec2::new(world_w, world_h)),
            color: Color::srgba(1.0, 1.0, 1.0, 0.7),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -1.0),
        PheromoneOverlay,
    ));

    commands.insert_resource(PheromoneTexture(tex));

    // Ant color from species appearance.
    let ant_color = crate::picker::parse_hex(&sim.species.appearance.color_hex);

    // Render food + nest entrances as static quads (Phase 1 MVP).
    let nest_mat = materials.add(Color::srgb(0.55, 0.35, 0.15));
    let food_mat = materials.add(Color::srgb(0.15, 0.85, 0.2));
    let tile_mesh = meshes.add(Rectangle::new(TILE * 1.5, TILE * 1.5));

    for y in 0..sim.sim.world.height {
        for x in 0..sim.sim.world.width {
            let t = sim.sim.world.get(x, y);
            let world_pos = cell_to_world(x, y, sim.sim.world.width, sim.sim.world.height);
            match t {
                Terrain::Food(_) => {
                    commands.spawn((
                        Mesh2d(tile_mesh.clone()),
                        MeshMaterial2d(food_mat.clone()),
                        Transform::from_translation(world_pos.extend(0.0)),
                    ));
                }
                Terrain::NestEntrance(_) => {
                    commands.spawn((
                        Mesh2d(meshes.add(Circle::new(TILE * 3.0))),
                        MeshMaterial2d(nest_mat.clone()),
                        Transform::from_translation(world_pos.extend(0.5)),
                    ));
                }
                _ => {}
            }
        }
    }

    // Spawn one sprite per ant.
    let ant_mesh = meshes.add(Circle::new(TILE * 0.4));
    let ant_mat = materials.add(ant_color);
    for ant in &sim.sim.ants {
        let pos = cell_to_world_f(ant.position.x, ant.position.y, sim.sim.world.width, sim.sim.world.height);
        commands.spawn((
            AntSprite(ant.id),
            Mesh2d(ant_mesh.clone()),
            MeshMaterial2d(ant_mat.clone()),
            Transform::from_translation(pos.extend(1.0)),
        ));
    }

    tracing::info!(
        ants = sim.sim.ants.len(),
        tiles = sim.sim.world.width * sim.sim.world.height,
        species = %sim.species.id,
        "RenderPlugin setup complete (AppState::Running)"
    );
}

fn cell_to_world(x: usize, y: usize, w: usize, h: usize) -> Vec2 {
    let cx = (x as f32 + 0.5 - w as f32 * 0.5) * TILE;
    let cy = (y as f32 + 0.5 - h as f32 * 0.5) * TILE;
    Vec2::new(cx, cy)
}

fn cell_to_world_f(x: f32, y: f32, w: usize, h: usize) -> Vec2 {
    let cx = (x - w as f32 * 0.5) * TILE;
    let cy = (y - h as f32 * 0.5) * TILE;
    Vec2::new(cx, cy)
}

fn sync_ant_sprites(
    sim: Res<SimulationState>,
    mut q: Query<(&AntSprite, &mut Transform)>,
) {
    let w = sim.sim.world.width;
    let h = sim.sim.world.height;
    for (sprite, mut tf) in q.iter_mut() {
        if let Some(ant) = sim.sim.ants.get(sprite.0 as usize) {
            let p = cell_to_world_f(ant.position.x, ant.position.y, w, h);
            tf.translation.x = p.x;
            tf.translation.y = p.y;
            let z = match ant.state {
                AntState::ReturningHome | AntState::StoringFood => 1.2,
                _ => 1.0,
            };
            tf.translation.z = z;
            tf.rotation = Quat::from_rotation_z(ant.heading);
        }
    }
}

fn update_pheromone_texture(
    sim: Res<SimulationState>,
    tex_handle: Res<PheromoneTexture>,
    mut images: ResMut<Assets<Image>>,
    overlay: Res<OverlayState>,
    mut q: Query<&mut Visibility, With<PheromoneOverlay>>,
) {
    for mut v in q.iter_mut() {
        *v = if overlay.visible { Visibility::Visible } else { Visibility::Hidden };
    }
    if !overlay.visible {
        return;
    }
    let Some(img) = images.get_mut(&tex_handle.0) else {
        return;
    };
    let w = sim.sim.pheromones.width;
    let h = sim.sim.pheromones.height;
    let food = &sim.sim.pheromones.food_trail;
    let home = &sim.sim.pheromones.home_trail;
    let alarm = &sim.sim.pheromones.alarm;
    let max = sim.sim.config.pheromone.max_intensity.max(0.001);
    let data = &mut img.data;
    data.resize(w * h * 4, 0);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let r = (alarm[i] / max * 255.0).clamp(0.0, 255.0) as u8;
            let g = (food[i] / max * 255.0).clamp(0.0, 255.0) as u8;
            let b = (home[i] / max * 255.0).clamp(0.0, 255.0) as u8;
            let a = r.max(g).max(b);
            let o = i * 4;
            data[o] = r;
            data[o + 1] = g;
            data[o + 2] = b;
            data[o + 3] = a;
        }
    }
}

fn toggle_overlay_input(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<OverlayState>) {
    if keys.just_pressed(KeyCode::KeyP) {
        overlay.visible = !overlay.visible;
        tracing::info!(visible = overlay.visible, "pheromone overlay toggled");
    }
}

fn camera_controls(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut scroll: EventReader<bevy::input::mouse::MouseWheel>,
    mut q: Query<(&mut Transform, &mut OrthographicProjection), With<Camera2d>>,
) {
    let dt = time.delta_secs();
    let mut pan = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        pan.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        pan.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        pan.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        pan.x += 1.0;
    }
    let mut zoom_delta = 0.0;
    for e in scroll.read() {
        zoom_delta -= e.y * 0.1;
    }
    for (mut tf, mut proj) in q.iter_mut() {
        tf.translation.x += pan.x * 400.0 * dt * proj.scale;
        tf.translation.y += pan.y * 400.0 * dt * proj.scale;
        proj.scale = (proj.scale * (1.0 + zoom_delta)).clamp(0.2, 6.0);
    }
}

#[allow(dead_code)]
fn _layer_color(layer: PheromoneLayer) -> (u8, u8, u8) {
    match layer {
        PheromoneLayer::FoodTrail => (0, 255, 0),
        PheromoneLayer::HomeTrail => (0, 0, 255),
        PheromoneLayer::Alarm => (255, 0, 0),
        PheromoneLayer::ColonyScent => (255, 255, 0),
    }
}

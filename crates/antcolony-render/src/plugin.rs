use antcolony_game::SimulationState;
use antcolony_sim::{AntState, ModuleId, PheromoneLayer, Terrain};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

use crate::AppState;

const TILE: f32 = 4.0;

pub struct RenderPlugin;

#[derive(Component)]
struct AntSprite(pub u32);

/// Pheromone overlay for a specific module.
#[derive(Component)]
struct PheromoneOverlay(pub ModuleId);

/// Texture handle for each module's pheromone overlay.
#[derive(Resource)]
struct PheromoneTextures(pub Vec<(ModuleId, Handle<Image>)>);

/// World-space (pixel) origin of each module's (0,0) corner, computed at setup.
#[derive(Resource)]
struct ModuleLayout(pub Vec<(ModuleId, Vec2)>);

#[derive(Resource, Default)]
struct OverlayState {
    pub visible: bool,
}

/// M-key overview toggle. Stores the pre-overview camera so we can restore.
#[derive(Resource, Default)]
struct OverviewState {
    pub saved: Option<(Vec3, f32)>,
}

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::ui::UiPlugin)
            .add_plugins(crate::picker::PickerPlugin)
            .add_plugins(crate::encyclopedia::EncyclopediaPlugin)
            .init_state::<AppState>()
            .insert_resource(OverlayState { visible: true })
            .insert_resource(OverviewState::default())
            .add_systems(OnEnter(AppState::Running), setup)
            .add_systems(
                Update,
                (
                    sync_ant_sprites,
                    update_pheromone_textures,
                    toggle_overlay_input,
                    toggle_overview_input,
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
    // Compute each module's world-space offset. Center the whole
    // formicarium around the camera origin.
    let (layout, centroid) = compute_layout(&sim);
    commands.spawn(Camera2d);

    let mut textures: Vec<(ModuleId, Handle<Image>)> = Vec::new();

    let nest_mat = materials.add(Color::srgb(0.55, 0.35, 0.15));
    let food_mat = materials.add(Color::srgb(0.15, 0.85, 0.2));
    let module_border_mat = materials.add(Color::srgba(0.25, 0.25, 0.28, 0.35));

    for module in &sim.sim.topology.modules {
        let mid = module.id;
        let (_, origin) = layout.iter().find(|(id, _)| *id == mid).copied().unwrap();
        let origin = origin - centroid;
        let mw = module.width() as u32;
        let mh = module.height() as u32;
        let mww = module.width() as f32 * TILE;
        let mhh = module.height() as f32 * TILE;

        // Module background panel (so the player can see where modules are).
        commands.spawn((
            Sprite {
                color: Color::srgba(0.12, 0.12, 0.15, 1.0),
                custom_size: Some(Vec2::new(mww, mhh)),
                ..default()
            },
            Transform::from_xyz(origin.x + mww * 0.5, origin.y + mhh * 0.5, -2.0),
        ));

        // Border frame.
        let frame_thickness = 2.0;
        for (w_size, h_size, dx, dy) in [
            (mww, frame_thickness, 0.0, -mhh * 0.5),
            (mww, frame_thickness, 0.0, mhh * 0.5),
            (frame_thickness, mhh, -mww * 0.5, 0.0),
            (frame_thickness, mhh, mww * 0.5, 0.0),
        ] {
            commands.spawn((
                Sprite {
                    color: Color::srgba(0.5, 0.5, 0.55, 1.0),
                    custom_size: Some(Vec2::new(w_size, h_size)),
                    ..default()
                },
                Transform::from_xyz(origin.x + mww * 0.5 + dx, origin.y + mhh * 0.5 + dy, -1.5),
            ));
            let _ = module_border_mat;
        }

        // Pheromone overlay texture.
        let mut img = Image::new_fill(
            Extent3d {
                width: mw,
                height: mh,
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
                custom_size: Some(Vec2::new(mww, mhh)),
                color: Color::srgba(1.0, 1.0, 1.0, 0.8),
                ..default()
            },
            Transform::from_xyz(origin.x + mww * 0.5, origin.y + mhh * 0.5, -1.0),
            PheromoneOverlay(mid),
        ));

        textures.push((mid, tex));

        // Tile overlays: food + nest entrances.
        let tile_mesh = meshes.add(Rectangle::new(TILE * 1.5, TILE * 1.5));
        for y in 0..module.height() {
            for x in 0..module.width() {
                let t = module.world.get(x, y);
                let world_pos = Vec2::new(
                    origin.x + (x as f32 + 0.5) * TILE,
                    origin.y + (y as f32 + 0.5) * TILE,
                );
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
                            Mesh2d(meshes.add(Circle::new(TILE * 2.5))),
                            MeshMaterial2d(nest_mat.clone()),
                            Transform::from_translation(world_pos.extend(0.5)),
                        ));
                    }
                    _ => {}
                }
            }
        }

        // Port markers (tiny yellow dots on module borders).
        let port_mat = materials.add(Color::srgb(0.95, 0.85, 0.2));
        let port_mesh = meshes.add(Circle::new(TILE * 0.6));
        for port in &module.ports {
            let p = Vec2::new(
                origin.x + (port.x as f32 + 0.5) * TILE,
                origin.y + (port.y as f32 + 0.5) * TILE,
            );
            commands.spawn((
                Mesh2d(port_mesh.clone()),
                MeshMaterial2d(port_mat.clone()),
                Transform::from_translation(p.extend(0.7)),
            ));
        }
    }

    // Draw tubes as rectangles connecting ports.
    let tube_mat = materials.add(Color::srgb(0.7, 0.6, 0.4));
    for tube in &sim.sim.topology.tubes {
        let a_origin = layout
            .iter()
            .find(|(id, _)| *id == tube.from.module)
            .map(|(_, o)| *o - centroid)
            .unwrap_or(Vec2::ZERO);
        let b_origin = layout
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
        let mid = (a + b) * 0.5;
        let dir = b - a;
        let length = dir.length().max(0.001);
        let angle = dir.y.atan2(dir.x);
        commands.spawn((
            Sprite {
                color: Color::srgb(0.7, 0.6, 0.4),
                custom_size: Some(Vec2::new(length, TILE * 1.6)),
                ..default()
            },
            Transform {
                translation: mid.extend(-0.8),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            },
        ));
        let _ = tube_mat;
    }

    // Ant sprites.
    let ant_color = crate::picker::parse_hex(&sim.species.appearance.color_hex);
    let ant_mesh = meshes.add(Circle::new(TILE * 0.4));
    let ant_mat = materials.add(ant_color);
    for (idx, ant) in sim.sim.ants.iter().enumerate() {
        let (_, origin) = layout
            .iter()
            .find(|(id, _)| *id == ant.module_id)
            .copied()
            .unwrap_or((0, Vec2::ZERO));
        let origin = origin - centroid;
        let pos = Vec2::new(
            origin.x + ant.position.x * TILE,
            origin.y + ant.position.y * TILE,
        );
        commands.spawn((
            AntSprite(idx as u32),
            Mesh2d(ant_mesh.clone()),
            MeshMaterial2d(ant_mat.clone()),
            Transform::from_translation(pos.extend(1.0)),
        ));
    }

    commands.insert_resource(PheromoneTextures(textures));
    commands.insert_resource(ModuleLayout(layout.clone()));

    tracing::info!(
        ants = sim.sim.ants.len(),
        modules = sim.sim.topology.modules.len(),
        tubes = sim.sim.topology.tubes.len(),
        species = %sim.species.id,
        "RenderPlugin setup complete (AppState::Running)"
    );
}

/// Compute each module's world-space origin (in pixels) from its
/// `formicarium_origin`, and return the centroid so we can recentre the
/// camera to (0,0).
fn compute_layout(sim: &SimulationState) -> (Vec<(ModuleId, Vec2)>, Vec2) {
    let mut out = Vec::new();
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for m in &sim.sim.topology.modules {
        let origin = m.formicarium_origin * TILE;
        let far = origin + Vec2::new(m.width() as f32 * TILE, m.height() as f32 * TILE);
        if origin.x < min.x {
            min.x = origin.x;
        }
        if origin.y < min.y {
            min.y = origin.y;
        }
        if far.x > max.x {
            max.x = far.x;
        }
        if far.y > max.y {
            max.y = far.y;
        }
        out.push((m.id, origin));
    }
    let centroid = (min + max) * 0.5;
    (out, centroid)
}

fn sync_ant_sprites(
    sim: Res<SimulationState>,
    layout: Option<Res<ModuleLayout>>,
    mut q: Query<(&AntSprite, &mut Transform, &mut Visibility)>,
) {
    let Some(layout) = layout else {
        return;
    };
    // Recompute centroid to match setup()'s transform convention.
    let (_, centroid) = compute_layout(&sim);
    for (sprite, mut tf, mut vis) in q.iter_mut() {
        let Some(ant) = sim.sim.ants.get(sprite.0 as usize) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Visible;
        if let Some(transit) = ant.transit {
            // Interpolate along the tube between its two endpoint world-positions.
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
            let t = transit.progress.clamp(0.0, 1.0);
            let p = a.lerp(b, t);
            let dir = if transit.going_forward { b - a } else { a - b };
            tf.translation.x = p.x;
            tf.translation.y = p.y;
            tf.translation.z = 1.1;
            tf.rotation = Quat::from_rotation_z(dir.y.atan2(dir.x));
            continue;
        }
        let origin = layout
            .0
            .iter()
            .find(|(id, _)| *id == ant.module_id)
            .map(|(_, o)| *o)
            .unwrap_or(Vec2::ZERO)
            - centroid;
        tf.translation.x = origin.x + ant.position.x * TILE;
        tf.translation.y = origin.y + ant.position.y * TILE;
        tf.translation.z = match ant.state {
            AntState::ReturningHome | AntState::StoringFood => 1.2,
            _ => 1.0,
        };
        tf.rotation = Quat::from_rotation_z(ant.heading);
    }
}

fn update_pheromone_textures(
    sim: Res<SimulationState>,
    textures: Option<Res<PheromoneTextures>>,
    mut images: ResMut<Assets<Image>>,
    overlay: Res<OverlayState>,
    mut q: Query<&mut Visibility, With<PheromoneOverlay>>,
) {
    for mut v in q.iter_mut() {
        *v = if overlay.visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !overlay.visible {
        return;
    }
    let Some(textures) = textures else {
        return;
    };
    let max = sim.sim.config.pheromone.max_intensity.max(0.001);
    for (mid, handle) in &textures.0 {
        let Some(img) = images.get_mut(handle) else {
            continue;
        };
        let module = sim.sim.topology.module(*mid);
        let w = module.pheromones.width;
        let h = module.pheromones.height;
        let food = &module.pheromones.food_trail;
        let home = &module.pheromones.home_trail;
        let alarm = &module.pheromones.alarm;
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
}

fn toggle_overlay_input(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<OverlayState>) {
    if keys.just_pressed(KeyCode::KeyP) {
        overlay.visible = !overlay.visible;
        tracing::info!(visible = overlay.visible, "pheromone overlay toggled");
    }
}

/// M-key: snap camera to formicarium overview (fit all modules with a
/// ~10% margin). Press again to restore the previous view.
fn toggle_overview_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut overview: ResMut<OverviewState>,
    sim: Res<SimulationState>,
    windows: Query<&bevy::window::Window>,
    mut q: Query<(&mut Transform, &mut OrthographicProjection), With<Camera2d>>,
) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }
    // Compute formicarium bounding box in world-space (same transform as setup()).
    let (layout, centroid) = compute_layout(&sim);
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for m in &sim.sim.topology.modules {
        let (_, origin) = layout.iter().find(|(id, _)| *id == m.id).copied().unwrap_or((0, Vec2::ZERO));
        let origin = origin - centroid;
        let far = origin + Vec2::new(m.width() as f32 * TILE, m.height() as f32 * TILE);
        min = min.min(origin);
        max = max.max(far);
    }
    if !min.x.is_finite() || !max.x.is_finite() {
        return;
    }
    let size = max - min;
    let center = (min + max) * 0.5;

    // Viewport dims for fit calculation.
    let (vw, vh) = windows
        .iter()
        .next()
        .map(|w| (w.width(), w.height()))
        .unwrap_or((1280.0, 720.0));

    for (mut tf, mut proj) in q.iter_mut() {
        if let Some((saved_pos, saved_scale)) = overview.saved.take() {
            // Restore previous view.
            tf.translation = saved_pos;
            proj.scale = saved_scale;
            tracing::info!("overview toggled OFF (restored view)");
        } else {
            overview.saved = Some((tf.translation, proj.scale));
            // Fit: scale so formicarium_width * 1.1 fits the viewport width.
            let sx = (size.x * 1.1) / vw.max(1.0);
            let sy = (size.y * 1.1) / vh.max(1.0);
            let fit = sx.max(sy).max(0.2);
            proj.scale = fit;
            tf.translation.x = center.x;
            tf.translation.y = center.y;
            tracing::info!(fit_scale = fit, "overview toggled ON (fit all modules)");
        }
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

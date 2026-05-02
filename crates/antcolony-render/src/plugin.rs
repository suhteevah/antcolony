use antcolony_game::SimulationState;
use antcolony_sim::{AntCaste, AntState, ModuleId, PheromoneLayer, Terrain};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

use crate::AppState;

pub(crate) const TILE: f32 = 4.0;

pub struct RenderPlugin;

#[derive(Component)]
pub(crate) struct AntSprite(pub u32);

/// Tag on each leg child sprite of an ant; the animation system swings
/// `rotation.z` around `base_angle` by a sine of the sim tick.
#[derive(Component)]
pub(crate) struct AntLeg {
    pub ant_idx: u32,
    pub base_angle: f32,
    /// `+1.0` or `-1.0` — which side of the body the leg sits on.
    pub side_sign: f32,
    /// 0 = front pair, 1 = middle pair, 2 = rear pair.
    pub pair: u8,
}

/// Child dot on the gaster that only shows when the ant is carrying food.
#[derive(Component)]
pub(crate) struct FoodCarryIndicator {
    pub ant_idx: u32,
}

/// Dig system: dark soil pellet held in mandibles, only visible when
/// the ant has `carrying_soil = true`. Same pattern as `FoodCarryIndicator`,
/// different position (forward of the head, not on the gaster) and color
/// (dusty brown, not green).
#[derive(Component)]
pub(crate) struct SoilCarryIndicator {
    pub ant_idx: u32,
}

/// Pheromone overlay for a specific module.
#[derive(Component)]
pub(crate) struct PheromoneOverlay(pub ModuleId);

/// Dig system Phase B: brief flash on the cell that just got excavated.
/// Spawned by `update_excavation_pulses` reading `sim.excavation_events`,
/// and despawned by the same system when its lifetime expires.
#[derive(Component)]
pub(crate) struct ExcavationPulse {
    /// Frames remaining until despawn. Counts down each frame.
    pub frames_left: u32,
    /// Total frames the pulse lives — used for alpha-fade.
    pub initial_frames: u32,
}

/// P6: one sprite per predator. Synced each frame against
/// `Simulation::predators` by id — new predators get spawned, dead ones
/// get despawned, live ones track position + state color.
#[derive(Component)]
pub(crate) struct PredatorSprite(pub u32);

/// P6: rain overlay sprite covering a surface module. Alpha fades in
/// with `weather.rain_ticks_remaining`.
#[derive(Component)]
pub(crate) struct RainOverlay(pub ModuleId);

/// P6: lawnmower blade indicator — thin horizontal rectangle at the
/// current blade y.
#[derive(Component)]
pub(crate) struct LawnmowerBlade;

/// K3: temperature overlay for a specific module.
#[derive(Component)]
pub(crate) struct TemperatureOverlay(pub ModuleId);

/// Marker tag on every entity spawned as part of the formicarium scene
/// (modules, borders, food/nest tiles, port markers, tubes, ant sprites,
/// pheromone overlays, editor selection gizmos). The editor despawns
/// everything with this tag when the topology mutates, then rebuilds.
#[derive(Component)]
pub(crate) struct FormicariumEntity;

/// Hit-test data attached to each module's background panel. `min`/`max`
/// are in world-space pixels (post-centroid) so editor input can check if
/// a cursor click landed inside a module.
#[derive(Component, Clone, Copy)]
pub(crate) struct ModuleRect {
    pub id: ModuleId,
    pub min: Vec2,
    pub max: Vec2,
}

/// Hit-test data for port markers.
#[derive(Component, Clone, Copy)]
pub(crate) struct PortMarker {
    pub module: ModuleId,
    pub port: antcolony_sim::PortPos,
    pub world_pos: Vec2,
}

/// Hit-test data for tube sprites.
#[derive(Component, Clone, Copy)]
pub(crate) struct TubeSprite {
    pub id: antcolony_sim::TubeId,
    pub a: Vec2,
    pub b: Vec2,
}

/// Flipped by the editor whenever topology (modules/tubes) changes. The
/// rebuild system consumes the flag, despawns all `FormicariumEntity`,
/// and respawns the scene.
#[derive(Resource, Default)]
pub(crate) struct TopologyDirty(pub bool);

/// Texture handle for each module's pheromone overlay.
#[derive(Resource)]
pub(crate) struct PheromoneTextures(pub Vec<(ModuleId, Handle<Image>)>);

/// K3: texture handle for each module's temperature overlay.
#[derive(Resource)]
pub(crate) struct TemperatureTextures(pub Vec<(ModuleId, Handle<Image>)>);

/// P4: texture handle for each module's colony-territory overlay.
#[derive(Resource)]
pub(crate) struct TerritoryTextures(pub Vec<(ModuleId, Handle<Image>)>);

/// P4: overlay sprite tag — visibility driven by `TerritoryOverlayState`.
#[derive(Component)]
pub(crate) struct TerritoryOverlay(pub ModuleId);

#[derive(Resource, Default)]
struct TerritoryOverlayState {
    pub visible: bool,
}

/// World-space (pixel) origin of each module's (0,0) corner, computed at setup.
#[derive(Resource)]
pub(crate) struct ModuleLayout(pub Vec<(ModuleId, Vec2)>);

#[derive(Resource, Default)]
struct OverlayState {
    pub visible: bool,
}

#[derive(Resource, Default)]
struct TempOverlayState {
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
            .add_plugins(crate::editor::EditorPlugin)
            .add_plugins(crate::save_ui::SaveUiPlugin)
            .add_plugins(crate::inspector::InspectorPlugin)
            .add_plugins(crate::timeline::TimelinePlugin)
            .add_plugins(crate::player_input::PlayerInputPlugin)
            .add_plugins(crate::atlas::SpriteAtlasPlugin)
            .init_state::<AppState>()
            .insert_resource(ClearColor(Color::srgb(0.09, 0.07, 0.05)))
            .insert_resource(OverlayState { visible: true })
            .insert_resource(TempOverlayState { visible: false })
            .insert_resource(TerritoryOverlayState { visible: false })
            .insert_resource(OverviewState::default())
            .insert_resource(TopologyDirty::default())
            .add_systems(OnEnter(AppState::Running), setup)
            .add_systems(
                Update,
                (
                    rebuild_formicarium_if_dirty,
                    sync_ant_sprites,
                    sync_predator_sprites,
                    update_rain_overlay,
                    update_lawnmower_blade,
                    animate_ant_legs,
                    update_food_indicators,
                    update_soil_carry_indicators,
                    update_excavation_pulses,
                    update_pheromone_textures,
                    update_temperature_textures,
                    update_territory_textures,
                    toggle_overlay_input,
                    toggle_temperature_input,
                    toggle_territory_input,
                    toggle_overview_input,
                    toggle_layer_view_input,
                    camera_controls,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn setup(
    mut commands: Commands,
    sim: Res<SimulationState>,
    atlas: Res<crate::atlas::SpriteAtlas>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);
    spawn_formicarium(&mut commands, &sim, &atlas, &mut images, &mut meshes, &mut materials);
}

/// Rebuild system: when topology has mutated, despawn all formicarium
/// entities and respawn from the current sim state.
fn rebuild_formicarium_if_dirty(
    mut commands: Commands,
    mut dirty: ResMut<TopologyDirty>,
    sim: Res<SimulationState>,
    atlas: Res<crate::atlas::SpriteAtlas>,
    q: Query<Entity, With<FormicariumEntity>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !dirty.0 {
        return;
    }
    let n = q.iter().count();
    for e in q.iter() {
        commands.entity(e).despawn_recursive();
    }
    tracing::info!(despawned = n, "rebuild_formicarium: topology dirty — respawning");
    spawn_formicarium(&mut commands, &sim, &atlas, &mut images, &mut meshes, &mut materials);
    dirty.0 = false;
}

/// Spawn everything that depends on the current topology. Every entity
/// spawned here gets a `FormicariumEntity` tag so the rebuild system can
/// wipe them on topology change.
pub(crate) fn spawn_formicarium(
    commands: &mut Commands,
    sim: &SimulationState,
    atlas: &crate::atlas::SpriteAtlas,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) {
    // Compute each module's world-space offset. Center the whole
    // formicarium around the camera origin.
    let (layout, centroid) = compute_layout(sim);

    let mut textures: Vec<(ModuleId, Handle<Image>)> = Vec::new();
    let mut temp_textures: Vec<(ModuleId, Handle<Image>)> = Vec::new();
    let mut territory_textures: Vec<(ModuleId, Handle<Image>)> = Vec::new();

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

        // Drop shadow: soft dark rectangle behind the module, offset down/right.
        commands.spawn((
            Sprite {
                color: Color::srgba(0.0, 0.0, 0.0, 0.55),
                custom_size: Some(Vec2::new(mww + 10.0, mhh + 10.0)),
                ..default()
            },
            Transform::from_xyz(origin.x + mww * 0.5 + 4.0, origin.y + mhh * 0.5 - 4.0, -2.5),
            FormicariumEntity,
        ));

        // Module background: procedural substrate texture per kind.
        let substrate_img = crate::substrate::make_substrate(module.kind, mw, mh, mid as u32 ^ 0xBEEF);
        let substrate_tex = images.add(substrate_img);
        commands.spawn((
            Sprite {
                image: substrate_tex,
                custom_size: Some(Vec2::new(mww, mhh)),
                ..default()
            },
            Transform::from_xyz(origin.x + mww * 0.5, origin.y + mhh * 0.5, -2.0),
            FormicariumEntity,
            ModuleRect { id: mid, min: Vec2::new(origin.x, origin.y), max: Vec2::new(origin.x + mww, origin.y + mhh) },
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
                FormicariumEntity,
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
            FormicariumEntity,
        ));

        textures.push((mid, tex));

        // K3 temperature overlay texture. Starts hidden; `T` toggles visibility.
        let mut timg = Image::new_fill(
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
        timg.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
        let ttex = images.add(timg);
        commands.spawn((
            Sprite {
                image: ttex.clone(),
                custom_size: Some(Vec2::new(mww, mhh)),
                color: Color::srgba(1.0, 1.0, 1.0, 0.65),
                ..default()
            },
            Transform::from_xyz(origin.x + mww * 0.5, origin.y + mhh * 0.5, -0.5),
            TemperatureOverlay(mid),
            Visibility::Hidden,
            FormicariumEntity,
        ));
        temp_textures.push((mid, ttex));

        // P4 territory overlay texture. Starts hidden; `G` toggles.
        let mut gimg = Image::new_fill(
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
        gimg.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
        let gtex = images.add(gimg);
        commands.spawn((
            Sprite {
                image: gtex.clone(),
                custom_size: Some(Vec2::new(mww, mhh)),
                color: Color::srgba(1.0, 1.0, 1.0, 0.55),
                ..default()
            },
            Transform::from_xyz(origin.x + mww * 0.5, origin.y + mhh * 0.5, -0.4),
            TerritoryOverlay(mid),
            Visibility::Hidden,
            FormicariumEntity,
        ));
        territory_textures.push((mid, gtex));

        // P6 rain overlay: translucent blue wash. Hidden by default,
        // becomes visible + full alpha while it's raining. Only on
        // surface modules — underground is sheltered.
        if module.kind != antcolony_sim::ModuleKind::UndergroundNest {
            commands.spawn((
                Sprite {
                    color: Color::srgba(0.15, 0.35, 0.75, 0.0),
                    custom_size: Some(Vec2::new(mww, mhh)),
                    ..default()
                },
                Transform::from_xyz(origin.x + mww * 0.5, origin.y + mhh * 0.5, -0.2),
                RainOverlay(mid),
                Visibility::Visible,
                FormicariumEntity,
            ));
        }

        // Food: berry-cluster (dark base + bright core + tiny highlight).
        // Nest entrance: crater (dark rim / shadow / pit / bright inner dot).
        let food_base = meshes.add(Circle::new(TILE * 0.95));
        let food_core = meshes.add(Circle::new(TILE * 0.65));
        let food_shine = meshes.add(Circle::new(TILE * 0.22));
        let food_base_mat = materials.add(Color::srgb(0.08, 0.45, 0.12));
        let food_core_mat = food_mat.clone();
        let food_shine_mat = materials.add(Color::srgb(0.75, 0.95, 0.55));

        // Dig system Phase B: per-substrate palette for the underground
        // module's Solid + Empty rendering. Loam = warm dark brown
        // (default), Sand = pale tan, Ytong = pale gray-white, Wood =
        // amber, Gel = cool blue. Empty cells in underground modules
        // get a slightly darker variant of the substrate base color to
        // create the "see the tunnels" cross-section look — without
        // this, Empty cells just show the surface substrate texture
        // and the tunnel network doesn't read as negative space.
        let (solid_color, tunnel_color) = match module.substrate {
            antcolony_sim::SubstrateKind::Loam => (
                Color::srgb(0.18, 0.12, 0.06),
                Color::srgb(0.04, 0.025, 0.015),
            ),
            antcolony_sim::SubstrateKind::Sand => (
                Color::srgb(0.62, 0.50, 0.32),
                Color::srgb(0.18, 0.13, 0.08),
            ),
            antcolony_sim::SubstrateKind::Ytong => (
                Color::srgb(0.82, 0.80, 0.74),
                Color::srgb(0.18, 0.16, 0.13),
            ),
            antcolony_sim::SubstrateKind::Wood => (
                Color::srgb(0.42, 0.28, 0.14),
                Color::srgb(0.10, 0.06, 0.03),
            ),
            antcolony_sim::SubstrateKind::Gel => (
                Color::srgb(0.30, 0.50, 0.62),
                Color::srgb(0.06, 0.14, 0.20),
            ),
        };
        let is_underground = module.kind == antcolony_sim::ModuleKind::UndergroundNest;

        let nest_rim = meshes.add(Circle::new(TILE * 3.0));
        let nest_shadow = meshes.add(Circle::new(TILE * 2.3));
        let nest_pit = meshes.add(Circle::new(TILE * 1.3));
        let nest_glow = meshes.add(Circle::new(TILE * 0.45));
        let nest_rim_mat = nest_mat.clone();
        let nest_shadow_mat = materials.add(Color::srgb(0.28, 0.17, 0.06));
        let nest_pit_mat = materials.add(Color::srgb(0.05, 0.03, 0.01));
        let nest_glow_mat = materials.add(Color::srgb(0.95, 0.78, 0.35));

        for y in 0..module.height() {
            for x in 0..module.width() {
                let t = module.world.get(x, y);
                let world_pos = Vec2::new(
                    origin.x + (x as f32 + 0.5) * TILE,
                    origin.y + (y as f32 + 0.5) * TILE,
                );
                match t {
                    Terrain::Food(_) => {
                        // Slight deterministic offset so clusters feel organic.
                        let jitter = Vec2::new(
                            ((x as i32 * 17 + y as i32 * 29) % 7) as f32 * 0.1 - 0.35,
                            ((x as i32 * 31 + y as i32 * 11) % 7) as f32 * 0.1 - 0.35,
                        );
                        let p = world_pos + jitter;
                        commands.spawn((
                            Mesh2d(food_base.clone()),
                            MeshMaterial2d(food_base_mat.clone()),
                            Transform::from_translation(p.extend(0.0)),
                            FormicariumEntity,
                        ));
                        commands.spawn((
                            Mesh2d(food_core.clone()),
                            MeshMaterial2d(food_core_mat.clone()),
                            Transform::from_translation(p.extend(0.05)),
                            FormicariumEntity,
                        ));
                        commands.spawn((
                            Mesh2d(food_shine.clone()),
                            MeshMaterial2d(food_shine_mat.clone()),
                            Transform::from_translation(
                                (p + Vec2::new(TILE * 0.15, TILE * 0.2)).extend(0.1),
                            ),
                            FormicariumEntity,
                        ));
                    }
                    Terrain::NestEntrance(_) => {
                        for (mesh, mat, z) in [
                            (&nest_rim, &nest_rim_mat, 0.3),
                            (&nest_shadow, &nest_shadow_mat, 0.4),
                            (&nest_pit, &nest_pit_mat, 0.5),
                            (&nest_glow, &nest_glow_mat, 0.55),
                        ] {
                            commands.spawn((
                                Mesh2d(mesh.clone()),
                                MeshMaterial2d(mat.clone()),
                                Transform::from_translation(world_pos.extend(z)),
                                FormicariumEntity,
                            ));
                        }
                    }
                    // Solid = unexcavated substrate. Color tinted by the
                    // module's `substrate` (Loam/Sand/Ytong/Wood/Gel)
                    // so different formicarium materials read distinctly.
                    Terrain::Solid => {
                        commands.spawn((
                            Sprite {
                                color: solid_color,
                                custom_size: Some(Vec2::splat(TILE)),
                                ..default()
                            },
                            Transform::from_translation(world_pos.extend(0.15)),
                            FormicariumEntity,
                        ));
                    }
                    // P5: Chamber cells — subtle per-kind coloured tile
                    // over the tunnel substrate so rooms read clearly.
                    Terrain::Chamber(kind) => {
                        let color = match kind {
                            antcolony_sim::ChamberType::QueenChamber => {
                                Color::srgba(0.85, 0.30, 0.60, 0.55)
                            }
                            antcolony_sim::ChamberType::BroodNursery => {
                                Color::srgba(0.95, 0.80, 0.30, 0.50)
                            }
                            antcolony_sim::ChamberType::FoodStorage => {
                                Color::srgba(0.30, 0.75, 0.30, 0.50)
                            }
                            antcolony_sim::ChamberType::Waste => {
                                Color::srgba(0.45, 0.35, 0.20, 0.55)
                            }
                        };
                        commands.spawn((
                            Sprite {
                                color,
                                custom_size: Some(Vec2::splat(TILE * 0.9)),
                                ..default()
                            },
                            Transform::from_translation(world_pos.extend(0.2)),
                            FormicariumEntity,
                        ));
                    }
                    // Dig system: kickout mound. Sized + tinted by
                    // accumulated pellet intensity so the mound visibly
                    // grows as the colony excavates. Cap at 1.4× tile so
                    // a mature mound visually overflows the entrance cell.
                    Terrain::SoilPile(intensity) => {
                        let n = intensity as f32;
                        // Saturating curve: small piles already visible,
                        // large piles cap rather than overflow the screen.
                        let scale = (0.5 + (n / 30.0).min(0.9)) * TILE;
                        // Warm dusty soil color, slightly brighter than
                        // unexcavated Solid so the mound reads as
                        // "ejected dirt" rather than "tunnel wall".
                        let darken = 1.0 - (n / 200.0).min(0.5);
                        commands.spawn((
                            Sprite {
                                color: Color::srgb(
                                    0.40 * darken,
                                    0.26 * darken,
                                    0.14 * darken,
                                ),
                                custom_size: Some(Vec2::splat(scale)),
                                ..default()
                            },
                            Transform::from_translation(world_pos.extend(0.16)),
                            FormicariumEntity,
                        ));
                    }
                    Terrain::Obstacle => {}
                    Terrain::Empty => {
                        // "See the tunnels" — in underground modules,
                        // an excavated Empty cell renders as the dark
                        // tunnel-color tile so the tunnel network reads
                        // as negative space carved through substrate.
                        // Surface modules leave Empty cells transparent
                        // (the substrate texture below shows through).
                        if is_underground {
                            commands.spawn((
                                Sprite {
                                    color: tunnel_color,
                                    custom_size: Some(Vec2::splat(TILE * 0.95)),
                                    ..default()
                                },
                                Transform::from_translation(world_pos.extend(0.10)),
                                FormicariumEntity,
                            ));
                        }
                    }
                }
            }
        }

        // Port markers: dark ring + bright inner dot, reading as tube
        // mouths rather than flat yellow stickers.
        let port_ring_mat = materials.add(Color::srgb(0.20, 0.14, 0.08));
        let port_inner_mat = materials.add(Color::srgb(0.95, 0.85, 0.28));
        let port_ring = meshes.add(Circle::new(TILE * 0.85));
        let port_inner = meshes.add(Circle::new(TILE * 0.45));
        for port in &module.ports {
            let p = Vec2::new(
                origin.x + (port.x as f32 + 0.5) * TILE,
                origin.y + (port.y as f32 + 0.5) * TILE,
            );
            commands.spawn((
                Mesh2d(port_ring.clone()),
                MeshMaterial2d(port_ring_mat.clone()),
                Transform::from_translation(p.extend(0.65)),
                FormicariumEntity,
            ));
            commands.spawn((
                Mesh2d(port_inner.clone()),
                MeshMaterial2d(port_inner_mat.clone()),
                Transform::from_translation(p.extend(0.7)),
                FormicariumEntity,
                PortMarker {
                    module: mid,
                    port: *port,
                    world_pos: p,
                },
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
        // Tube body: dark outline, warm tan core, thin bright sheen band
        // along the top for a glass-cylinder read.
        let body_thickness = TILE * 1.7;
        commands.spawn((
            Sprite {
                color: Color::srgb(0.22, 0.17, 0.10),
                custom_size: Some(Vec2::new(length + TILE * 0.3, body_thickness + TILE * 0.4)),
                ..default()
            },
            Transform {
                translation: mid.extend(-0.9),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            },
            FormicariumEntity,
        ));
        commands.spawn((
            Sprite {
                color: Color::srgb(0.72, 0.60, 0.38),
                custom_size: Some(Vec2::new(length, body_thickness)),
                ..default()
            },
            Transform {
                translation: mid.extend(-0.8),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            },
            FormicariumEntity,
            TubeSprite { id: tube.id, a, b },
        ));
        // Sheen band: sits ~20% above centre along the tube axis.
        let sheen_offset = Vec2::new(-dir.y, dir.x).normalize_or_zero() * body_thickness * 0.28;
        commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 0.95, 0.78, 0.55),
                custom_size: Some(Vec2::new(length * 0.95, body_thickness * 0.18)),
                ..default()
            },
            Transform {
                translation: (mid + sheen_offset).extend(-0.75),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            },
            FormicariumEntity,
        ));
        let _ = tube_mat;
    }

    // Ant bodies: parent entity carries AntSprite + Transform; children form
    // a head / thorax / gaster trio plus antennae and six legs, rotated by
    // the parent's heading.
    //
    // P4: per-colony palette. Colony 0 wears the chosen species' color;
    // every additional colony gets a distinctive rust-red tint so the
    // player can tell them apart at a glance.
    let species_color = crate::picker::parse_hex(&sim.species.appearance.color_hex);
    let colony_count = sim.sim.colonies.len().max(1);
    let mut body_mats: Vec<Handle<ColorMaterial>> = Vec::with_capacity(colony_count);
    let mut limb_colors: Vec<Color> = Vec::with_capacity(colony_count);
    for cid in 0..colony_count {
        let base = if cid == 0 {
            species_color
        } else {
            // Hostile red — bright enough to read against dark substrate,
            // distinct from green (food) and yellow (ports).
            Color::srgb(0.85, 0.18, 0.12)
        };
        body_mats.push(materials.add(darken(base, 0.55)));
        limb_colors.push(darken(base, 0.35));
    }
    let unit_circle = meshes.add(Circle::new(1.0));
    let food_carry_mat = materials.add(Color::srgb(0.25, 0.95, 0.35));
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
        commands
            .spawn((
                AntSprite(idx as u32),
                Transform::from_translation(pos.extend(1.0)),
                Visibility::default(),
                FormicariumEntity,
            ))
            .with_children(|c| {
                let cid = (ant.colony_id as usize).min(body_mats.len() - 1);
                let sprite = atlas.lookup(&sim.species.id, ant.caste).cloned();
                spawn_ant_parts(
                    c,
                    idx as u32,
                    ant.caste,
                    &unit_circle,
                    &body_mats[cid],
                    limb_colors[cid],
                    &food_carry_mat,
                    sprite.as_ref(),
                );
            });
    }

    commands.insert_resource(PheromoneTextures(textures));
    commands.insert_resource(TemperatureTextures(temp_textures));
    commands.insert_resource(TerritoryTextures(territory_textures));
    commands.insert_resource(ModuleLayout(layout.clone()));

    // P6: single lawnmower blade indicator, hidden by default. Width is
    // re-sized by the update system based on the target module.
    commands.spawn((
        Sprite {
            color: Color::srgba(0.95, 0.15, 0.12, 0.85),
            custom_size: Some(Vec2::new(8.0, 4.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 3.0),
        LawnmowerBlade,
        Visibility::Hidden,
        FormicariumEntity,
    ));

    tracing::info!(
        ants = sim.sim.ants.len(),
        modules = sim.sim.topology.modules.len(),
        tubes = sim.sim.topology.tubes.len(),
        species = %sim.species.id,
        "RenderPlugin setup complete (AppState::Running)"
    );
}

fn darken(c: Color, factor: f32) -> Color {
    let s = c.to_srgba();
    Color::srgb(s.red * factor, s.green * factor, s.blue * factor)
}

/// Build the child sprites that compose one ant: gaster, thorax, head (three
/// stacked ellipses formed by scaling a unit circle), two antennae, and six
/// legs. Facing direction is +X, so the parent's heading rotation orients it.
fn spawn_ant_parts(
    c: &mut ChildBuilder,
    ant_idx: u32,
    caste: AntCaste,
    unit_circle: &Handle<Mesh>,
    body_mat: &Handle<ColorMaterial>,
    limb_color: Color,
    food_carry_mat: &Handle<ColorMaterial>,
    atlas_sprite: Option<&Handle<Image>>,
) {
    // Caste-driven size + silhouette shaping.
    let (body_scale, head_boost, gaster_boost) = match caste {
        AntCaste::Worker => (1.0, 1.0, 1.0),
        AntCaste::Soldier => (1.25, 1.35, 1.0),
        AntCaste::Breeder => (1.15, 1.0, 1.15),
        AntCaste::Queen => (1.7, 1.0, 1.6),
    };
    let s = TILE * 0.35 * body_scale;

    // Atlas-rendered ants: spawn a single textured sprite covering the
    // body footprint instead of the procedural primitives. Legs and
    // antennae are baked into the image so leg animation is suppressed
    // for atlas ants — the trade-off for high-quality art. Food-carry
    // and player overlays still spawn procedurally further down.
    let use_atlas = atlas_sprite.is_some();
    if let Some(handle) = atlas_sprite {
        let sprite_size = TILE * 4.0 * body_scale;
        c.spawn((
            Sprite {
                image: handle.clone(),
                custom_size: Some(Vec2::splat(sprite_size)),
                ..default()
            },
            // -90deg: source images are drawn head-up (north); the parent
            // entity rotates the sprite by ant.heading where 0 rad = +X
            // (east). Pre-rotating -90deg maps north→east so heading
            // values line up with the in-game travel direction.
            Transform::from_translation(Vec3::new(0.0, 0.0, 0.0))
                .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
        ));
    }

    // Procedural body parts (gaster/thorax/head/antennae/legs). Skipped
    // when an atlas sprite is present — the textured sprite already
    // includes those features baked into the art.
    if !use_atlas {
    // Gaster (rear, largest, elongated along body axis).
    let gx = -1.35 * s * gaster_boost;
    c.spawn((
        Mesh2d(unit_circle.clone()),
        MeshMaterial2d(body_mat.clone()),
        Transform::from_translation(Vec3::new(gx, 0.0, 0.0))
            .with_scale(Vec3::new(1.55 * s * gaster_boost, 1.0 * s * gaster_boost, 1.0)),
    ));
    // Gaster sheen — soft highlight on the upper surface for a wet look.
    c.spawn((
        Sprite {
            color: Color::srgba(1.0, 1.0, 1.0, 0.18),
            custom_size: Some(Vec2::new(1.5 * s * gaster_boost, 0.25 * s * gaster_boost)),
            ..default()
        },
        Transform::from_translation(Vec3::new(gx + 0.15 * s, 0.32 * s * gaster_boost, 0.02)),
    ));
    // Thorax (middle).
    c.spawn((
        Mesh2d(unit_circle.clone()),
        MeshMaterial2d(body_mat.clone()),
        Transform::from_scale(Vec3::new(0.85 * s, 0.65 * s, 1.0)),
    ));
    // Head (front).
    c.spawn((
        Mesh2d(unit_circle.clone()),
        MeshMaterial2d(body_mat.clone()),
        Transform::from_translation(Vec3::new(1.25 * s, 0.0, 0.0))
            .with_scale(Vec3::new(0.8 * s * head_boost, 0.72 * s * head_boost, 1.0)),
    ));

    // Antennae — two thin rectangles sweeping forward from the head.
    let antenna_len = 1.3 * s;
    let antenna_thick = (s * 0.14).max(0.6);
    for sign in [-1.0_f32, 1.0] {
        c.spawn((
            Sprite {
                color: limb_color,
                custom_size: Some(Vec2::new(antenna_len, antenna_thick)),
                ..default()
            },
            Transform {
                translation: Vec3::new(1.7 * s, sign * 0.35 * s, -0.05),
                rotation: Quat::from_rotation_z(sign * 0.55),
                ..default()
            },
        ));
    }

    // Six legs: three pairs anchored around the thorax, splayed outward.
    let leg_len = 1.75 * s;
    let leg_thick = (s * 0.16).max(0.7);
    for (pair, lx) in [-0.55_f32, 0.0, 0.55].into_iter().enumerate() {
        let angle_base = 0.95 - pair as f32 * 0.35; // front legs lean forward, rear legs lean back
        for sign in [-1.0_f32, 1.0] {
            let base_angle = sign * angle_base;
            c.spawn((
                Sprite {
                    color: limb_color,
                    custom_size: Some(Vec2::new(leg_len, leg_thick)),
                    ..default()
                },
                Transform {
                    translation: Vec3::new(lx * s, sign * 0.45 * s, -0.1),
                    rotation: Quat::from_rotation_z(base_angle),
                    ..default()
                },
                AntLeg {
                    ant_idx,
                    base_angle,
                    side_sign: sign,
                    pair: pair as u8,
                },
            ));
        }
    }
    } // end if !use_atlas

    // Food carry indicator — bright green dot sitting on the gaster.
    c.spawn((
        Mesh2d(unit_circle.clone()),
        MeshMaterial2d(food_carry_mat.clone()),
        Transform::from_translation(Vec3::new(-1.35 * s * gaster_boost, 0.0, 0.1))
            .with_scale(Vec3::splat(s * 0.45)),
        Visibility::Hidden,
        FoodCarryIndicator { ant_idx },
    ));

    // Dig system: soil pellet indicator — dusty-brown dot held in
    // mandibles (forward of the head, not on the gaster). Hidden until
    // ant.carrying_soil is set; toggled by `update_soil_carry_indicators`.
    let soil_pellet_mat = body_mat.clone(); // re-use body color matrix; will tint via Sprite
    c.spawn((
        Sprite {
            color: Color::srgb(0.42, 0.28, 0.16),
            custom_size: Some(Vec2::splat(s * 0.55)),
            ..default()
        },
        Transform::from_translation(Vec3::new(1.95 * s, 0.0, 0.12)),
        Visibility::Hidden,
        SoilCarryIndicator { ant_idx },
    ));
    let _ = soil_pellet_mat;

    // P7 avatar overlay: bright yellow halo, hidden unless this ant is
    // the possessed player avatar. Rendered below the body so the
    // body silhouette still reads clearly — it bleeds past the edges
    // as a tinted ring.
    c.spawn((
        Sprite {
            color: Color::srgba(1.0, 0.92, 0.15, 0.9),
            custom_size: Some(Vec2::splat(s * 5.0)),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, -0.3)),
        Visibility::Hidden,
        crate::player_input::PlayerAvatarOverlay { ant_idx },
    ));

    // P7 follower ring: thin cyan tag, hidden unless this ant has
    // `follow_leader` set. Sits over the thorax so it reads as a
    // collar on the recruit.
    c.spawn((
        Sprite {
            color: Color::srgba(0.25, 0.85, 0.95, 0.85),
            custom_size: Some(Vec2::splat(s * 2.2)),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, -0.25)),
        Visibility::Hidden,
        crate::player_input::FollowerRing { ant_idx },
    ));
}

/// Compute each module's world-space origin (in pixels) from its
/// `formicarium_origin`, and return the centroid so we can recentre the
/// camera to (0,0).
pub(crate) fn compute_layout(sim: &SimulationState) -> (Vec<(ModuleId, Vec2)>, Vec2) {
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
            AntState::NuptialFlight => 3.0,
            _ => 1.0,
        };
        // Nuptial flight: lift the breeder off the substrate and scale it
        // down as she climbs, ending in a small dot as she exits the frame.
        if ant.state == AntState::NuptialFlight {
            let flight_ticks = sim.sim.config.colony.nuptial_flight_ticks.max(1) as f32;
            let t = (ant.state_timer as f32 / flight_ticks).clamp(0.0, 1.0);
            tf.translation.y += t * 40.0; // rise ~10 tiles over full flight
            let shrink = 1.0 - t * 0.6;
            tf.scale = Vec3::splat(shrink);
            tf.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2); // face up
        } else {
            tf.scale = Vec3::ONE;
            tf.rotation = Quat::from_rotation_z(ant.heading);
        }
    }
}

/// P6: sync predator sprite entities against `Simulation::predators`.
/// Spawns a new sprite for any predator without one, despawns orphans,
/// updates the transform + color of live entries.
fn sync_predator_sprites(
    mut commands: Commands,
    sim: Res<SimulationState>,
    layout: Option<Res<ModuleLayout>>,
    existing: Query<(Entity, &PredatorSprite)>,
) {
    use antcolony_sim::{PredatorKind, PredatorState};
    let Some(layout) = layout else {
        return;
    };
    let (_, centroid) = compute_layout(&sim);

    // Index existing sprites by predator id.
    let mut by_id: std::collections::HashMap<u32, Entity> = std::collections::HashMap::new();
    for (e, s) in existing.iter() {
        by_id.insert(s.0, e);
    }

    let live_ids: std::collections::HashSet<u32> =
        sim.sim.predators.iter().map(|p| p.id).collect();

    // Despawn sprites whose predator is gone.
    for (e, s) in existing.iter() {
        if !live_ids.contains(&s.0) {
            commands.entity(e).despawn_recursive();
        }
    }

    for predator in &sim.sim.predators {
        let origin = layout
            .0
            .iter()
            .find(|(id, _)| *id == predator.module_id)
            .map(|(_, o)| *o)
            .unwrap_or(Vec2::ZERO)
            - centroid;
        let pos = Vec2::new(
            origin.x + predator.position.x * TILE,
            origin.y + predator.position.y * TILE,
        );
        let (color, size) = match (predator.kind, predator.state) {
            (PredatorKind::Spider, PredatorState::Dead { .. }) => {
                (Color::srgba(0.2, 0.1, 0.08, 0.6), Vec2::splat(TILE * 1.1))
            }
            (PredatorKind::Spider, PredatorState::Eat { .. }) => {
                (Color::srgb(0.95, 0.15, 0.1), Vec2::splat(TILE * 1.4))
            }
            (PredatorKind::Spider, PredatorState::Hunt { .. }) => {
                (Color::srgb(0.85, 0.2, 0.15), Vec2::splat(TILE * 1.25))
            }
            (PredatorKind::Spider, _) => {
                (Color::srgb(0.6, 0.2, 0.2), Vec2::splat(TILE * 1.1))
            }
            (PredatorKind::Antlion, _) => {
                (Color::srgb(0.45, 0.25, 0.12), Vec2::splat(TILE * 1.6))
            }
        };
        let z = 1.2;
        if let Some(&e) = by_id.get(&predator.id) {
            // Existing — patch transform + sprite.
            commands.entity(e).insert((
                Transform::from_translation(pos.extend(z)),
                Sprite {
                    color,
                    custom_size: Some(size),
                    ..default()
                },
            ));
        } else {
            commands.spawn((
                Sprite {
                    color,
                    custom_size: Some(size),
                    ..default()
                },
                Transform::from_translation(pos.extend(z)),
                PredatorSprite(predator.id),
                FormicariumEntity,
            ));
        }
    }
}

/// P6: fade the per-module rain wash in during rain, out when clear.
fn update_rain_overlay(
    sim: Res<SimulationState>,
    mut q: Query<(&RainOverlay, &mut Sprite)>,
) {
    let cfg = &sim.sim.config.hazards;
    let remaining = sim.sim.weather.rain_ticks_remaining as f32;
    let duration = cfg.rain_duration_ticks.max(1) as f32;
    // Ramp alpha up toward the start, hold, ramp down at the end.
    let intensity = (remaining / duration).clamp(0.0, 1.0);
    let alpha = intensity * 0.35;
    for (_, mut sprite) in q.iter_mut() {
        let mut c = sprite.color.to_srgba();
        c.alpha = alpha;
        sprite.color = Color::Srgba(c);
    }
}

/// P6: snap the lawnmower blade to `(module.origin.x + width/2, blade_y)`
/// and show/hide based on whether a sweep is active.
fn update_lawnmower_blade(
    sim: Res<SimulationState>,
    layout: Option<Res<ModuleLayout>>,
    mut q: Query<(&mut Transform, &mut Visibility, &mut Sprite), With<LawnmowerBlade>>,
) {
    let Some(layout) = layout else {
        return;
    };
    let (_, centroid) = compute_layout(&sim);
    let w = &sim.sim.weather;
    let sweeping = w.lawnmower_sweep_remaining > 0;
    let warning = w.lawnmower_warning_remaining > 0;
    let active = sweeping || warning;

    for (mut tf, mut vis, mut sprite) in q.iter_mut() {
        if !active {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Visible;
        let Some(&(_, origin)) = layout
            .0
            .iter()
            .find(|(id, _)| *id == w.lawnmower_module)
        else {
            *vis = Visibility::Hidden;
            continue;
        };
        let Some(module) = sim.sim.topology.try_module(w.lawnmower_module) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let module_w = module.width() as f32 * TILE;
        let origin = origin - centroid;
        // During warning, park the blade at y=0 (just-spawning); during
        // sweep, place it at the live blade_y.
        let blade_y = if warning { 0.0 } else { w.lawnmower_y * TILE };
        tf.translation.x = origin.x + module_w * 0.5;
        tf.translation.y = origin.y + blade_y;
        tf.translation.z = 3.0;
        let (color, height) = if warning {
            // Pulsing warning stripe (dim red).
            (Color::srgba(0.95, 0.40, 0.15, 0.55), TILE * 0.6)
        } else {
            (Color::srgba(0.95, 0.15, 0.10, 0.90), TILE * 0.8)
        };
        sprite.color = color;
        sprite.custom_size = Some(Vec2::new(module_w, height));
    }
}

/// Swing each leg's rotation around its base angle. Tripod gait: legs
/// in tripod A (front-left, middle-right, rear-left) step together,
/// tripod B (the opposite three) are 180° out of phase. Still ants hold
/// their legs at the base angle.
fn animate_ant_legs(
    time: Res<Time>,
    sim: Res<SimulationState>,
    mut q: Query<(&AntLeg, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (leg, mut tf) in q.iter_mut() {
        let Some(ant) = sim.sim.ants.get(leg.ant_idx as usize) else {
            continue;
        };
        let moving = matches!(
            ant.state,
            AntState::Exploring
                | AntState::FollowingTrail
                | AntState::ReturningHome
                | AntState::Fleeing
                | AntState::Fighting
                | AntState::NuptialFlight
        ) || ant.transit.is_some();
        if !moving {
            tf.rotation = Quat::from_rotation_z(leg.base_angle);
            continue;
        }
        // Tripod gait: pair 0 & 2 on one side and pair 1 on the other step together.
        let tripod = (leg.pair as i32 + (if leg.side_sign > 0.0 { 0 } else { 1 })).rem_euclid(2);
        let phase_offset = if tripod == 0 { 0.0 } else { std::f32::consts::PI };
        let freq = 10.0; // steps per second visual
        let swing = (t * freq + phase_offset).sin() * 0.35;
        tf.rotation = Quat::from_rotation_z(leg.base_angle + swing * leg.side_sign);
    }
}

/// Toggle each ant's food dot visibility based on whether the ant is
/// carrying food right now.
fn update_food_indicators(
    sim: Res<SimulationState>,
    mut q: Query<(&FoodCarryIndicator, &mut Visibility)>,
) {
    for (ind, mut vis) in q.iter_mut() {
        let visible = sim
            .sim
            .ants
            .get(ind.ant_idx as usize)
            .map(|a| a.food_carried > 0.0)
            .unwrap_or(false);
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Dig system: toggle soil-pellet visibility on each ant based on
/// `carrying_soil`. Mirrors `update_food_indicators` but reads the
/// dig-system flag instead of food.
fn update_soil_carry_indicators(
    sim: Res<SimulationState>,
    mut q: Query<(&SoilCarryIndicator, &mut Visibility), Without<FoodCarryIndicator>>,
) {
    for (ind, mut vis) in q.iter_mut() {
        let visible = sim
            .sim
            .ants
            .get(ind.ant_idx as usize)
            .map(|a| a.carrying_soil)
            .unwrap_or(false);
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Dig system Phase B: drain `sim.excavation_events` and spawn brief
/// flash sprites at each cell that just got excavated. Existing pulses
/// fade alpha each frame and despawn when expired.
fn update_excavation_pulses(
    mut commands: Commands,
    mut sim: ResMut<SimulationState>,
    layout: Option<Res<ModuleLayout>>,
    mut pulses: Query<(Entity, &mut ExcavationPulse, &mut Sprite, &mut Transform)>,
) {
    // Fade + decrement existing pulses.
    for (e, mut pulse, mut sprite, mut tf) in pulses.iter_mut() {
        if pulse.frames_left == 0 {
            commands.entity(e).despawn();
            continue;
        }
        pulse.frames_left = pulse.frames_left.saturating_sub(1);
        let progress = pulse.frames_left as f32 / pulse.initial_frames as f32;
        // Pulse: bright at spawn, fades to nothing. Slight scale-up
        // gives the "puff" feel.
        sprite.color = Color::srgba(0.95, 0.78, 0.30, progress);
        tf.scale = Vec3::splat(1.0 + (1.0 - progress) * 0.4);
    }

    // Spawn fresh pulses from the sim's event buffer. Drain it so we
    // don't double-spawn next frame.
    let Some(layout) = layout else {
        sim.sim.excavation_events.clear();
        return;
    };
    if sim.sim.excavation_events.is_empty() {
        return;
    }
    // Recompute centroid to match setup() / sync_ant_sprites convention.
    let (_, centroid) = compute_layout(&sim);
    let events: Vec<_> = sim.sim.excavation_events.drain(..).collect();
    for (mid, x, y, _tick) in events {
        let Some((_, origin)) = layout.0.iter().find(|(id, _)| *id == mid) else {
            continue;
        };
        let pos = Vec2::new(
            origin.x - centroid.x + (x as f32 + 0.5) * TILE,
            origin.y - centroid.y + (y as f32 + 0.5) * TILE,
        );
        commands.spawn((
            Sprite {
                color: Color::srgba(0.95, 0.78, 0.30, 1.0),
                custom_size: Some(Vec2::splat(TILE * 1.1)),
                ..default()
            },
            Transform::from_translation(pos.extend(0.18)),
            ExcavationPulse {
                frames_left: 30,
                initial_frames: 30,
            },
            FormicariumEntity,
        ));
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
        // Resource may lag a frame behind topology rebuilds — skip entries
        // whose module has been removed.
        let Some(module) = sim.sim.topology.try_module(*mid) else {
            continue;
        };
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

/// K3: paint per-module temperature textures using a blue-white-red
/// gradient centred on 20°C. 0°C → deep blue, 20°C → white/transparent,
/// 40°C → deep red. Outside clamped. Visibility toggled by `T`.
fn update_temperature_textures(
    sim: Res<SimulationState>,
    textures: Option<Res<TemperatureTextures>>,
    mut images: ResMut<Assets<Image>>,
    overlay: Res<TempOverlayState>,
    mut q: Query<&mut Visibility, With<TemperatureOverlay>>,
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
    for (mid, handle) in &textures.0 {
        let Some(img) = images.get_mut(handle) else {
            continue;
        };
        let Some(module) = sim.sim.topology.try_module(*mid) else {
            continue;
        };
        let w = module.pheromones.width;
        let h = module.pheromones.height;
        let temps = &module.temperature;
        let data = &mut img.data;
        data.resize(w * h * 4, 0);
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let t = temps[i];
                // Map temp → (r,g,b,a). Midpoint 20°C.
                let (r, g, b, a) = temp_color_ramp(t);
                let o = i * 4;
                data[o] = r;
                data[o + 1] = g;
                data[o + 2] = b;
                data[o + 3] = a;
            }
        }
    }
}

/// Blue (cold) → white (mid) → red (hot) gradient. 0°C deep blue,
/// 20°C transparent white, 40°C deep red. Alpha scales with distance from 20.
fn temp_color_ramp(t: f32) -> (u8, u8, u8, u8) {
    let mid = 20.0f32;
    let delta = t - mid;
    let norm = (delta.abs() / 20.0).clamp(0.0, 1.0);
    let alpha = (norm * 200.0) as u8;
    if delta >= 0.0 {
        // white → red
        let k = norm;
        let r = 255u8;
        let g = ((1.0 - k) * 255.0) as u8;
        let b = ((1.0 - k) * 255.0) as u8;
        (r, g, b, alpha)
    } else {
        // white → blue
        let k = norm;
        let r = ((1.0 - k) * 255.0) as u8;
        let g = ((1.0 - k) * 255.0) as u8;
        let b = 255u8;
        (r, g, b, alpha)
    }
}

fn toggle_temperature_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<TempOverlayState>,
) {
    if keys.just_pressed(KeyCode::KeyT) {
        overlay.visible = !overlay.visible;
        tracing::info!(visible = overlay.visible, "temperature overlay toggled");
    }
}

/// P4: paint each module's territory overlay. Signed colony_scent maps
/// to a colour wash: positive (colony 0) → the species' chosen colour
/// at reduced saturation; negative (colony 1) → rust red. Alpha scales
/// with |value|/max.
fn update_territory_textures(
    sim: Res<SimulationState>,
    textures: Option<Res<TerritoryTextures>>,
    mut images: ResMut<Assets<Image>>,
    overlay: Res<TerritoryOverlayState>,
    mut q: Query<&mut Visibility, With<TerritoryOverlay>>,
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
    // Species colour for colony 0's tint, red for colony 1.
    let species = crate::picker::parse_hex(&sim.species.appearance.color_hex);
    let s0 = species.to_srgba();
    let (r0, g0, b0) = (s0.red, s0.green, s0.blue);
    // Colony 1 territory: bright rust.
    let (r1, g1, b1) = (0.85f32, 0.18, 0.12);

    for (mid, handle) in &textures.0 {
        let Some(img) = images.get_mut(handle) else {
            continue;
        };
        let Some(module) = sim.sim.topology.try_module(*mid) else {
            continue;
        };
        let w = module.pheromones.width;
        let h = module.pheromones.height;
        let scent = &module.pheromones.colony_scent;
        let data = &mut img.data;
        data.resize(w * h * 4, 0);
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let v = scent[i];
                let mag = (v.abs() / max).clamp(0.0, 1.0);
                let (r, g, b) = if v >= 0.0 { (r0, g0, b0) } else { (r1, g1, b1) };
                let alpha = (mag * 200.0).clamp(0.0, 200.0) as u8;
                let o = i * 4;
                data[o] = (r * 255.0) as u8;
                data[o + 1] = (g * 255.0) as u8;
                data[o + 2] = (b * 255.0) as u8;
                data[o + 3] = alpha;
            }
        }
    }
}

fn toggle_territory_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<TerritoryOverlayState>,
) {
    if keys.just_pressed(KeyCode::KeyG) {
        overlay.visible = !overlay.visible;
        tracing::info!(visible = overlay.visible, "territory overlay toggled");
    }
}

/// P5: Tab-key toggles between surface view (non-UndergroundNest
/// modules) and underground view (UndergroundNest modules). Each press
/// snaps the camera to the centroid of the other layer; the zoom is
/// left alone so the player keeps their scale.
fn toggle_layer_view_input(
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<SimulationState>,
    mut q: Query<&mut Transform, With<Camera2d>>,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    let (layout, centroid) = compute_layout(&sim);
    let mut surface_sum = Vec2::ZERO;
    let mut surface_n = 0u32;
    let mut under_sum = Vec2::ZERO;
    let mut under_n = 0u32;
    for m in &sim.sim.topology.modules {
        let Some(&(_, origin)) = layout.iter().find(|(id, _)| *id == m.id) else {
            continue;
        };
        let origin = origin - centroid;
        let center = origin
            + Vec2::new(m.width() as f32 * TILE * 0.5, m.height() as f32 * TILE * 0.5);
        if m.kind == antcolony_sim::ModuleKind::UndergroundNest {
            under_sum += center;
            under_n += 1;
        } else {
            surface_sum += center;
            surface_n += 1;
        }
    }
    if under_n == 0 {
        tracing::info!("Tab: no underground modules to switch to");
        return;
    }
    let surface_c = if surface_n > 0 {
        surface_sum / surface_n as f32
    } else {
        Vec2::ZERO
    };
    let under_c = under_sum / under_n as f32;
    for mut tf in q.iter_mut() {
        // Decide which layer we are currently on by proximity, then jump
        // to the other one.
        let to_surface = (tf.translation.truncate() - under_c).length()
            < (tf.translation.truncate() - surface_c).length();
        let target = if to_surface { surface_c } else { under_c };
        tf.translation.x = target.x;
        tf.translation.y = target.y;
        tracing::info!(
            layer = if to_surface { "surface" } else { "underground" },
            "Tab: layer view switched"
        );
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
    sim: Res<SimulationState>,
    mut scroll: EventReader<bevy::input::mouse::MouseWheel>,
    module_rects: Query<&ModuleRect>,
    mut q: Query<(&mut Transform, &mut OrthographicProjection), With<Camera2d>>,
) {
    let dt = time.delta_secs();
    // P7: when an ant is possessed, WASD steers the avatar instead of
    // panning the camera. Arrow keys still pan so the player can look
    // around while controlling an ant.
    let possessed_idx = sim.sim.player_ant_index();
    let possessed = possessed_idx.is_some();
    let mut pan = Vec2::ZERO;
    if (!possessed && keys.pressed(KeyCode::KeyW)) || keys.pressed(KeyCode::ArrowUp) {
        pan.y += 1.0;
    }
    if (!possessed && keys.pressed(KeyCode::KeyS)) || keys.pressed(KeyCode::ArrowDown) {
        pan.y -= 1.0;
    }
    if (!possessed && keys.pressed(KeyCode::KeyA)) || keys.pressed(KeyCode::ArrowLeft) {
        pan.x -= 1.0;
    }
    if (!possessed && keys.pressed(KeyCode::KeyD)) || keys.pressed(KeyCode::ArrowRight) {
        pan.x += 1.0;
    }
    let mut zoom_delta = 0.0;
    for e in scroll.read() {
        zoom_delta -= e.y * 0.1;
    }

    // P7 polish: when possessed and the user is not actively pan-overriding
    // with arrow keys, smoothly lerp the camera toward the avatar's world
    // position. The exponential-smoothing factor makes ant motion readable
    // (no jitter, no swing-around) while still tracking quickly enough that
    // the avatar doesn't leave the viewport.
    let arrow_panning = pan != Vec2::ZERO;
    let follow_target: Option<Vec2> = if possessed && !arrow_panning {
        possessed_idx.and_then(|i| {
            let ant = sim.sim.ants.get(i)?;
            let mid = ant.module_id;
            let rect = module_rects.iter().find(|r| r.id == mid)?;
            Some(rect.min + ant.position * TILE)
        })
    } else {
        None
    };

    for (mut tf, mut proj) in q.iter_mut() {
        tf.translation.x += pan.x * 400.0 * dt * proj.scale;
        tf.translation.y += pan.y * 400.0 * dt * proj.scale;
        proj.scale = (proj.scale * (1.0 + zoom_delta)).clamp(0.2, 6.0);

        if let Some(target) = follow_target {
            // Exponential decay toward target: stiffness 6.0 → reaches
            // ~95% of the gap in 0.5s. Tuned by feel; keep it gentle so
            // the ant's own motion is the visual story, not the camera.
            let stiffness = 6.0_f32;
            let alpha = 1.0 - (-stiffness * dt).exp();
            tf.translation.x += (target.x - tf.translation.x) * alpha;
            tf.translation.y += (target.y - tf.translation.y) * alpha;
        }
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

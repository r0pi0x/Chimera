//! The drawn phases of spawning a map (`map::spawn_map` calls them through
//! `map::MapDrawer`, at their places in its order): images, character
//! assets, lightmaps, the sky, water, world meshes (chunked, merged brush
//! entities), model materials, detail sprites, effects, sprites, beams,
//! ropes and shadows; and each placed prop's skeleton, meshes and skins,
//! ragdoll or rig. `MapAssets` (the asset stores) is the `map::MapDrawer`.

use std::{collections::HashMap, sync::Arc};

use bevy::{ecs::system::SystemParam, prelude::*};

#[allow(unused_imports)]
use crate::map::*;
use super::{
    bodies::{HeldAssets, HeldParts, PlayModels, body_assets},
    breakables::PanePart,
    props::{PropMeshSlot, PropVariants},
    dust, emitters, light_styles, merge, monitor, shadows,
    view_model::{build_assets as build_view_models, build_flash_assets},
    sky::{ActiveMapHas3dSky, MapSkybox, SkyCameraInfo, SkyVis},
    steam, water,
};
// Explicit: Bevy's prelude has a `LightProbe` too.
use crate::map::LightProbe;
#[allow(unused_imports)]
use super::{
    LightmapLayers, SOURCE_LIGHTMAP_SCALE, build_material, build_mesh, clamp_edges, cube_image, fog, fog_color,
    fog_range, lit_prop_material, material_fog, probe_lit, rope_material,
    prop_material::PropMaterial,
    rope_material::{RopeMaterial, RopeParams},
    set_prop_envmap, sprite_material,
    sprite_material::{SpriteMaterial, SpriteParams},
    to_image,
    world_material::{WorldMaterial, WorldParams},
};

/// The asset stores a map's drawn parts go into, as `map::spawn_map`'s
/// `MapDrawer`.
impl MapDrawer for MapAssets<'static> {
    type Drawn = Drawn;

    fn world_material(a: &MapAssets) -> bool {
        a.world_materials.is_some()
    }

    fn draws(a: &MapAssets) -> Draws {
        let drawn = a.meshes.is_some() && a.materials.is_some() && a.images.is_some();
        Draws {
            loose: drawn && a.bindposes.is_some(),
            gibs: drawn && a.prop_materials.is_some(),
        }
    }

    fn spawn_undrawn(a: &mut MapAssets, commands: &mut Commands, s: &MapSpawn) {
        if !(a.meshes.is_some() && a.materials.is_some() && a.images.is_some()) {
            super::steam::spawn_undrawn(commands, s);
        }
    }

    fn spawn_drawn(a: &mut MapAssets, commands: &mut Commands, s: &MapSpawn) -> Drawn {
        spawn_drawn(commands, s, a)
    }

    fn retire(world: &mut World, data: Arc<MapData>) {
        // The shadows keep a share of the data for the map's life: they
        // take this one (theirs goes) and drop what they never read (the
        // kit, the world's meshes and collision, most texels).
        if let Some(mut source) = world.get_resource_mut::<shadows::ShadowSource>() {
            drop(std::mem::replace(&mut source.data, data));
            if let Some(data) = Arc::get_mut(&mut source.data) {
                shadows::shed_unread(data);
            }
        }
    }

    fn spawn_prop(a: &mut MapAssets, commands: &mut Commands, s: &MapSpawn, drawn: &mut Drawn, prop: PropSpawn) {
        let mut shared = std::mem::take(&mut drawn.shared);
        spawn_prop(commands, s, a, drawn, &mut shared, prop);
        drawn.shared = shared;
    }

    fn textures(drawn: &Drawn) -> &[Handle<Image>] {
        &drawn.textures
    }

    fn insert_drawn(drawn: &Drawn, commands: &mut Commands, s: &MapSpawn, which: DrawnResource) {
        match which {
            DrawnResource::DecalSurfaces => commands.insert_resource(super::decal::DecalSurfaces::new(s.data)),
            DrawnResource::Particles => super::particles::insert_assets(commands, s.data, &drawn.textures),
            DrawnResource::Decals => super::decal::insert_assets(commands, s.data, &drawn.textures),
            DrawnResource::Fog => {
                commands.insert_resource(fog::SceneFog(fog::world_fog(s.data.fog.as_ref())));
            }
        }
    }
}

/// Signs for normal maps' red and green channels in the world material.
/// Source's normal maps are DirectX-style (x along texture u, y along v,
/// image-down) and used unflipped (specs/cs_source/shaders.md). Overridable
/// with CHIMERA_NORMAL_X_SIGN / CHIMERA_NORMAL_G_SIGN for experiments.
const NORMAL_G_SIGN: f32 = 1.0;
const NORMAL_X_SIGN: f32 = 1.0;

/// The asset stores a map spawns into; each is missing headless (then
/// the parts it would draw aren't spawned).
#[derive(SystemParam)]
pub struct MapAssets<'w> {
    meshes: Option<ResMut<'w, Assets<Mesh>>>,
    materials: Option<ResMut<'w, Assets<StandardMaterial>>>,
    images: Option<ResMut<'w, Assets<Image>>>,
    world_materials: Option<ResMut<'w, Assets<WorldMaterial>>>,
    rope_materials: Option<ResMut<'w, Assets<RopeMaterial>>>,
    sprite_materials: Option<ResMut<'w, Assets<SpriteMaterial>>>,
    prop_materials: Option<ResMut<'w, Assets<PropMaterial>>>,
    shadow_materials: Option<Res<'w, Assets<shadows::ShadowMaterial>>>,
    water_materials: Option<ResMut<'w, Assets<water::WaterMaterial>>>,
    bindposes: Option<ResMut<'w, Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>>,
    beam_materials: Option<ResMut<'w, Assets<super::beams::BeamMaterial>>>,
    detail_materials: Option<ResMut<'w, Assets<super::detail::DetailMaterial>>>,
    heat_materials: Option<ResMut<'w, Assets<steam::HeatWaveMaterial>>>,
    /// Which compressed texture formats the device takes (`bc_wanted`).
    compressed: Option<Res<'w, bevy::image::CompressedImageFormatSupport>>,
}

/// Render handles the drawn phases make and the props and resources use
/// (empty headless).
#[derive(Default)]
pub struct Drawn {
    /// Each map texture's image.
    textures: Vec<Handle<Image>>,
    /// Each baked cubemap's image.
    cubemaps: Vec<Handle<Image>>,
    /// Per model: each mesh's mesh and standard material.
    model_parts: Vec<Vec<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    /// Per model mesh: [in the world (fogged), in the 3D skybox].
    lit_model_materials: Vec<Vec<[Handle<PropMaterial>; 2]>>,
    /// Per model with several skins: each skin's materials per mesh.
    skin_model_materials: Vec<Vec<Vec<[Handle<PropMaterial>; 2]>>>,
    /// What the props' drawn parts share as they are spawned.
    shared: PropShared,
}

/// The lightmap images surfaces sample.
struct Lightmaps {
    /// For standard materials.
    plain: Option<Handle<Image>>,
    /// For the world material.
    world: Option<Handle<Image>>,
    /// The three bump pages (radiosity normal mapping).
    bumped: Option<[Handle<Image>; 3]>,
    source_ldr: bool,
}

/// Everything drawn, when rendering exists (meshes, standard materials
/// and images): nothing headless.
fn spawn_drawn(commands: &mut Commands, s: &MapSpawn, a: &mut MapAssets) -> Drawn {
    let mut drawn = Drawn::default();
    let MapAssets {
        meshes,
        materials,
        images,
        world_materials,
        rope_materials,
        sprite_materials,
        prop_materials,
        shadow_materials,
        water_materials,
        bindposes,
        beam_materials,
        detail_materials,
        heat_materials,
        compressed,
    } = a;
    let (Some(meshes), Some(materials), Some(images)) = (meshes.as_mut(), materials.as_mut(), images.as_mut()) else {
        return drawn;
    };
    let (data, view) = (s.data, s.view);
    // The images, built ahead on the loading thread (`prepare_images`)
    // when they were for this device's formats, else here.
    let bc = super::images::bc_wanted(compressed.as_deref());
    let parts = s
        .prepared
        .take::<ImageParts>()
        .filter(|p| p.bc == bc)
        .unwrap_or_else(|| image_parts(data, bc));
    let ImageParts {
        textures: texture_images,
        cubemaps: cubemap_images,
        lightmaps: lightmap_layers,
        ..
    } = parts;
    let (textures, cubemaps) = load_images(commands, data, images, texture_images, cubemap_images);
    drawn.cubemaps = cubemaps.clone();
    drawn.textures = textures.clone();
    // Character bodies, drawn by `attach_bodies`: skinned when the
    // model has a skeleton.
    if let Some(bindposes) = bindposes.as_mut() {
        build_character_assets(
            commands,
            s,
            &textures,
            &cubemaps,
            meshes,
            bindposes,
            prop_materials,
            sprite_materials,
        );
    }
    let lightmaps = build_lightmaps(commands, data, images, lightmap_layers, world_materials.is_none());
    let sky_handle = spawn_sky(commands, s, images, meshes);
    // Water surfaces have their own material (`water`).
    let water_drawn = water_materials.is_some() && view == MapDebugView::Normal;
    if water_drawn && let Some(water_materials) = water_materials.as_mut() {
        water::spawn_surfaces(
            commands,
            data,
            s.root,
            meshes,
            images,
            water_materials,
            &cubemaps,
            sky_handle.as_ref(),
            &|i| s.node(i),
        );
    }
    spawn_world_meshes(
        commands,
        s,
        meshes,
        materials,
        world_materials,
        &textures,
        &cubemaps,
        &lightmaps,
        water_drawn,
    );
    build_model_materials(&mut drawn, s, meshes, materials, prop_materials);

    if view == MapDebugView::Normal
        && let Some(props) = &data.source_fx().detail_props
        && let Some(detail_materials) = detail_materials.as_mut()
    {
        super::detail::spawn(
            commands,
            meshes,
            detail_materials,
            props,
            textures[props.texture].clone(),
            fog::world_fog(data.fog.as_ref()),
            s.statics,
            |e, lo, hi| {
                s.tag(
                    e,
                    s.visibility.map(|v| vis::box_clusters(v, lo, hi)).unwrap_or_default(),
                )
            },
        );
    }
    if view == MapDebugView::Normal {
        spawn_effects(commands, s, meshes, materials, heat_materials.as_deref_mut(), &textures);
    }
    if let Some(sprite_materials) = sprite_materials.as_mut()
        && view == MapDebugView::Normal
    {
        spawn_sprites(commands, s, meshes, sprite_materials, &textures);
    }
    // Beams (point_spotlight, env_laser, env_beam), spotlights' halos
    // and light glows (`beams`).
    if let (Some(beam_materials), Some(sprite_materials)) = (beam_materials.as_mut(), sprite_materials.as_mut())
        && view == MapDebugView::Normal
    {
        spawn_beams(commands, s, meshes, beam_materials, sprite_materials, &textures);
    }
    if let Some(rope_materials) = rope_materials.as_mut()
        && view == MapDebugView::Normal
    {
        spawn_ropes(commands, s, meshes, rope_materials, &textures);
    }
    // Dynamic shadows: built by `shadows::apply_shadow_detail` as
    // Video > Advanced's shadow detail asks (and again when it changes).
    if let (Some(settings), true) = (&data.shadows, shadow_materials.is_some())
        && view == MapDebugView::Normal
    {
        commands.insert_resource(shadows::ShadowSource {
            data: data.clone(),
            settings: settings.clone(),
            receivers: Arc::new(shadows::Receivers::new(data)),
            root: s.statics,
            fog_color: fog_color(data.fog.as_ref()),
            fog_range: fog_range(data.fog.as_ref()),
            tag: s.visibility.is_some(),
            built: None,
            blob_material: None,
        });
    }
    drawn
}

/// How many of `textures` are block compressed images, and their texel
/// bytes in all (what goes to the GPU).
fn image_sizes(images: &Assets<Image>, textures: &[Handle<Image>]) -> (usize, usize) {
    textures
        .iter()
        .filter_map(|h| images.get(h))
        .fold((0, 0), |(n, bytes), i| {
            (
                n + i.texture_descriptor.format.is_compressed() as usize,
                bytes + i.data.as_ref().map_or(0, Vec::len),
            )
        })
}

/// The map's textures and cubemaps as images (the monitor screen's
/// texture is its render target), and the resources that name them.
/// A map's images as its drawer builds them from the data alone: each
/// texture's (None for the monitor's screen, made at the spawn), the
/// baked cubemaps and the lightmap's layers, for a device that does or
/// doesn't take BC (`bc`).
pub struct ImageParts {
    bc: bool,
    textures: Vec<Option<Image>>,
    cubemaps: Vec<Image>,
    lightmaps: Option<LightmapLayers>,
}

/// The drawer's job for `MapHooks::prepare`: the images, for the device's
/// formats as the load starts.
pub fn prepare_images(world: &World) -> crate::map::prepare::PrepareJob {
    let bc = super::images::bc_wanted(world.get_resource::<bevy::image::CompressedImageFormatSupport>());
    Box::new(move |data, prepared| prepared.insert(image_parts(data, bc)))
}

fn image_parts(data: &MapData, bc: bool) -> ImageParts {
    // HUD pictures are clamped at their edges: wrapped, the scope
    // ring's outer edge blends with its clear centre and lets the
    // scene through at the screen's edges.
    let hud: std::collections::HashSet<usize> = data
        .kit.hud
        .iter()
        .flat_map(|h| h.sprites.values().map(|s| s.texture))
        .collect();
    let textures = data
        .textures
        .iter()
        .enumerate()
        .map(|(i, t)| {
            (Some(i) != data.camera_texture).then(|| {
                let mut image = to_image(t, &data.look, bc);
                if hud.contains(&i) {
                    clamp_edges(&mut image);
                }
                image
            })
        })
        .collect();
    let cubemaps = data.cubemaps.iter().map(cube_image).collect();
    let source_ldr = data.look.source_ldr_lightmaps;
    let lightmaps = data
        .lightmap
        .as_ref()
        .map(|l| LightmapLayers::build(&l.rgb, l.bumped.as_ref(), l.width, l.height, source_ldr));
    ImageParts {
        bc,
        textures,
        cubemaps,
        lightmaps,
    }
}

fn load_images(
    commands: &mut Commands,
    data: &MapData,
    images: &mut Assets<Image>,
    texture_images: Vec<Option<Image>>,
    cubemap_images: Vec<Image>,
) -> (Vec<Handle<Image>>, Vec<Handle<Image>>) {
    let textures: Vec<Handle<Image>> = texture_images
        .into_iter()
        .map(|image| match image {
            Some(image) => images.add(image),
            None => monitor::screen_image(images),
        })
        .collect();
    let (compressed, bytes) = image_sizes(images, &textures);
    info!(
        "textures: {compressed} of {} uploaded block compressed, {:.1} MiB",
        textures.len(),
        bytes as f32 / (1 << 20) as f32
    );
    if let Some(i) = data.camera_texture {
        commands.insert_resource(monitor::MonitorScreen(textures[i].clone()));
    }
    if !data.screen_overlays.is_empty() {
        commands.insert_resource(ScreenOverlayImages(
            data.screen_overlays
                .iter()
                .map(|(name, t)| (name.clone(), textures[*t].clone()))
                .collect(),
        ));
    }
    commands.insert_resource(super::texture_anim::TextureAnims::new(data, &textures));
    let cubemaps: Vec<Handle<Image>> = cubemap_images.into_iter().map(|c| images.add(c)).collect();
    (textures, cubemaps)
}

/// What characters draw: their bodies, what they hold (also dropped),
/// view models, muzzle flashes, shells and gibs. Bodies and what they
/// hold are lit from the light field where they are (probe_lit), as
/// Source lights models.
#[allow(clippy::too_many_arguments)]
fn build_character_assets(
    commands: &mut Commands,
    s: &MapSpawn,
    textures: &[Handle<Image>],
    cubemaps: &[Handle<Image>],
    meshes: &mut Assets<Mesh>,
    bindposes: &mut Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>,
    prop_materials: &mut Option<ResMut<Assets<PropMaterial>>>,
    sprite_materials: &mut Option<ResMut<Assets<SpriteMaterial>>>,
) {
    let (data, view) = (s.data, s.view);
    let Some(pm) = prop_materials.as_deref_mut() else {
        panic!("PropMaterial assets exist wherever StandardMaterial does");
    };
    let model_material = |m: &MapMesh| {
        let mut material = lit_prop_material(m, textures, data, view, false);
        if !m.unlit && view != MapDebugView::Albedo && data.light_field.is_some() {
            material.params.set_probe(&LightProbe::default(), 1.0);
        }
        material
    };
    let lighting_scale = if let MapDebugView::Lighting { scale } = view {
        scale
    } else {
        1.0
    };
    commands.insert_resource(probe_lit::ProbeLightScale(data.look.light_scale * lighting_scale));
    let bodies = data
        .kit.characters
        .iter()
        .map(|c| body_assets(&c.model, &c.bones, c.root, meshes, pm, bindposes, &model_material))
        .collect();
    let held = data
        .kit.held
        .iter()
        .map(|h| {
            let mut build = |model: &MapModel| -> Vec<(Handle<Mesh>, Handle<PropMaterial>)> {
                model
                    .meshes
                    .iter()
                    .map(|m| (meshes.add(build_mesh(m, false)), pm.add(model_material(m))))
                    .collect()
            };
            let whole = build(&h.model);
            let own = h.own.as_ref().map_or_else(|| whole.clone(), &mut build);
            let merged = h.merged.iter().map(|(b, m)| (b.clone(), build(m))).collect();
            (
                h.key.clone(),
                HeldParts {
                    bone: h.bone.clone(),
                    whole,
                    own,
                    muzzle: h.muzzle,
                    merged,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let view_models = prop_materials
        .as_mut()
        .map(|pm| build_view_models(data, textures, cubemaps, view, meshes, pm, bindposes));
    let flash = build_flash_assets(data, textures, meshes, sprite_materials.as_deref_mut());
    let shells = prop_materials
        .as_mut()
        .and_then(|pm| super::shells::build_assets(data, textures, view, meshes, pm));
    let gibs = prop_materials
        .as_mut()
        .map(|pm| super::breakables::build_gib_draws(data, textures, view, meshes, pm))
        .unwrap_or_default();
    // Built in that order (asset ids follow it), inserted in this one
    // (resource ids follow it): the gibs' meshes go with the bodies'.
    commands.insert_resource(PlayModels { bodies, gibs });
    // Loose (dropped) forms (`map::loose`), drawn with the held models'
    // whole meshes (`super::loose`).
    if let Some(loose) = loose::assets(data) {
        commands.insert_resource(loose);
    }
    commands.insert_resource(HeldAssets(held));
    // What held models added later are built with (`bodies::build_late_held`).
    commands.insert_resource(super::bodies::HeldLateBase {
        data: MapData {
            fog: data.fog.clone(),
            look: data.look.clone(),
            ..default()
        },
        lit: data.light_field.is_some(),
        view,
        root: data.kit.characters.first().map(|c| c.root),
    });
    if let Some(view_models) = view_models {
        commands.insert_resource(view_models);
    }
    if let Some(flash) = flash {
        commands.insert_resource(flash);
    }
    if let Some(shells) = shells {
        commands.insert_resource(shells);
    }
    // The gibs' simulation (`map::breakables`), drawn by `super::breakables`.
    if let Some(gibs) = breakables::build_assets(data) {
        commands.insert_resource(gibs);
    }
}

/// The lightmap images. Switchable and animated light styles relight
/// them (`light_styles`); the plain one only where surfaces sample it
/// (standard materials, used when the world material is missing).
fn build_lightmaps(
    commands: &mut Commands,
    data: &MapData,
    images: &mut Assets<Image>,
    built: Option<LightmapLayers>,
    no_world_material: bool,
) -> Lightmaps {
    let source_ldr = data.look.source_ldr_lightmaps;
    let (mut lightmap, mut world_lightmap, mut bumped_lightmaps) = (None, None, None::<[Handle<Image>; 3]>);
    if let Some(b) = built {
        let plain = images.add(b.plain);
        // Without the game's own encoding the world's copy is the plain one.
        world_lightmap = Some(b.world.map_or_else(|| plain.clone(), |w| images.add(w)));
        lightmap = Some(plain);
        bumped_lightmaps = b.bumped.map(|pages| pages.map(|p| images.add(p)));
    }
    if let Some(l) = data.lightmap.as_ref().filter(|l| !l.styles.is_empty()) {
        commands.insert_resource(light_styles::StyledLightmaps::new(
            Arc::new(l.clone()),
            source_ldr,
            lightmap.clone().filter(|_| no_world_material),
            world_lightmap.clone(),
            bumped_lightmaps.clone(),
        ));
    }
    Lightmaps {
        plain: lightmap,
        world: world_lightmap,
        bumped: bumped_lightmaps,
        source_ldr,
    }
}

/// The sky: its cubemap image (returned), the 3D skybox's camera, the
/// sky's visibility and its faces as occluders; and the playable area.
fn spawn_sky(
    commands: &mut Commands,
    s: &MapSpawn,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
) -> Option<Handle<Image>> {
    let (data, view) = (s.data, s.view);
    let mut sky_handle = None;
    if let Some(sky) = &data.sky
        && view == MapDebugView::Normal
    {
        let handle = images.add(sky_image(sky, &data.textures));
        sky_handle = Some(handle.clone());
        commands.insert_resource(MapSkybox(handle));
    }
    if let Some(bounds) = data.playable {
        commands.insert_resource(PlayableArea(bounds));
    }

    if let Some(cam) = &data.sky_camera
        && view == MapDebugView::Normal
    {
        commands.insert_resource(SkyCameraInfo(cam.clone()));
        commands.insert_resource(ActiveMapHas3dSky);
    }
    if let Some(vis) = &data.sky_vis
        && view == MapDebugView::Normal
    {
        commands.insert_resource(SkyVis(vis.clone()));
        // The sky is drawn by a sky camera even without a 3D skybox, so
        // it can be switched off where the map can't see it.
        if data.sky_camera.is_none() {
            commands.insert_resource(SkyCameraInfo(MapSkyCamera {
                origin: Vec3::ZERO,
                scale: 1.0,
                fog: None,
            }));
        }
    }
    // Sky faces, to hide what lies behind them (depth only, when
    // `chimera_skyocclude` is on: `sky_occluder`).
    if let Some(mesh) = super::sky_occluder::mesh(&data.sky_surfaces) {
        commands.spawn((
            Name::new("Sky surfaces"),
            MapPart,
            super::sky_occluder::SkyOccluder,
            Mesh3d(meshes.add(mesh)),
            Transform::default(),
            ChildOf(s.statics),
        ));
    }
    sky_handle
}

/// World meshes in chunks with tight bounds, tagged with the clusters
/// they touch (see `vis`). Meshes of entities with their own node
/// (movers, breakables) stay whole and untagged.
#[allow(clippy::too_many_arguments)]
fn spawn_world_meshes(
    commands: &mut Commands,
    s: &MapSpawn,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    world_materials: &mut Option<ResMut<Assets<WorldMaterial>>>,
    textures: &[Handle<Image>],
    cubemaps: &[Handle<Image>],
    lightmaps: &Lightmaps,
    water_drawn: bool,
) {
    let (data, view, root, statics) = (s.data, s.view, s.root, s.statics);
    let visibility = s.visibility;
    let chunk_size = vis::chunk_size();
    // One material asset per distinct material (brush entities have a
    // mesh per material each): meshes sharing one are drawn in batches
    // (one bind group, multi-draw) instead of one draw call apiece.
    // Keyed by the material's whole description, and by whether it is
    // in the 3D skybox (water fog skips skybox materials).
    // Areaportal windows' brushes keep materials of their own:
    // `super::vis::fade_windows` changes their alpha per brush.
    let window_brushes: std::collections::HashSet<usize> = data
        .visibility
        .as_deref()
        .map(|v| {
            v.areas
                .portals
                .iter()
                .filter_map(|p| p.fade.and_then(|f| f.brush))
                .collect()
        })
        .unwrap_or_default();
    let own = |m: &MapMesh| m.entity.filter(|e| window_brushes.contains(e));
    // Brush entities' opaque meshes are drawn merged per material and
    // chunk while they stay put (`merge`); their own meshes wait hidden.
    let merging = merge::enabled();
    // Windows with broken looks switch meshes when they break: never
    // merged.
    let paned: std::collections::HashSet<usize> = data
        .meshes
        .iter()
        .filter(|m| m.pane_look != PaneLook::Whole)
        .filter_map(|m| m.entity)
        .collect();
    let mut node_centres: HashMap<usize, (Vec3, Vec3)> = HashMap::new();
    for m in data.meshes.iter().filter(|m| !m.skybox && s.parent_of(m) != root) {
        let Some(i) = m.entity else { continue };
        let b = node_centres.entry(i).or_insert((Vec3::MAX, Vec3::MIN));
        for p in &m.positions {
            b.0 = b.0.min(Vec3::from(*p));
            b.1 = b.1.max(Vec3::from(*p));
        }
    }
    let merge_spot = |m: &MapMesh| -> Option<(usize, Entity, Transform, Vec3)> {
        let i = m.entity?;
        let node = s.parent_of(m);
        if !merging || node == root || own(m).is_some() || !merge::mergeable(m) || paned.contains(&i) {
            return None;
        }
        let home = s.node_home(i);
        let (lo, hi) = node_centres.get(&i)?;
        Some((i, node, home, home.transform_point((*lo + *hi) / 2.0)))
    };
    let mut world_merger: merge::Merger<(Handle<WorldMaterial>, bool)> = merge::Merger::new(chunk_size);
    let mut standard_merger: merge::Merger<(Handle<StandardMaterial>, bool)> = merge::Merger::new(chunk_size);
    let mut world_shared: HashMap<(bool, Option<usize>, String), Handle<WorldMaterial>> = HashMap::new();
    let mut standard_shared: HashMap<(bool, Option<usize>, String), Handle<StandardMaterial>> = HashMap::new();
    // Brush entity nodes' mesh bounds (node space), for culling them.
    let mut node_bounds: HashMap<Entity, (Vec3, Vec3)> = HashMap::new();
    // Overlays and infodecals draw in the map's order (`blend_order`).
    let mut decal_order = 0u32;
    let mut section = 0u32;
    for m in &data.meshes {
        // Fog-only water (another shader: de_nuke's pool) is drawn as the
        // world.
        if water_drawn && m.water.is_some_and(|i| !data.water_materials[i].fog_only) {
            continue;
        }
        let own_node = s.parent_of(m) != root;
        if own_node && visibility.is_some() && !m.skybox {
            let b = node_bounds
                .entry(s.parent_of(m))
                .or_insert((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)));
            for p in &m.positions {
                b.0 = b.0.min(Vec3::from(*p));
                b.1 = b.1.max(Vec3::from(*p));
            }
        }
        let mesh_vis = visibility.filter(|_| !own_node);
        // Chunks are placed at their centre (vertices relative to it,
        // small numbers), each a map part of its own.
        let chunks: Vec<(MapMesh, Vec<u32>, Vec3)> = vis::split_mesh(m, mesh_vis, chunk_size)
            .into_iter()
            .map(|(mut chunk, clusters)| {
                let centre = if mesh_vis.is_some() {
                    vis::recentre(&mut chunk)
                } else {
                    Vec3::ZERO
                };
                (chunk, clusters, centre)
            })
            .collect();
        let lit = lightmaps
            .plain
            .as_ref()
            .filter(|_| m.lightmap_uvs.len() == m.positions.len());
        // Lightmapped world surfaces: Source-style texture x baked light.
        if let (Some(_), Some(lm), Some(world_materials)) = (lit, lightmaps.world.as_ref(), world_materials.as_mut()) {
            let (material, blended) = lit_world_material(m, s, textures, cubemaps, lightmaps, lm);
            // A sprite section keeps its own material, so its clock is its
            // own (two sections with one sprite run apart).
            let animated = material.skin_frames.len() > 1;
            section += 1;
            let key = if animated { format!("{material:?}#{section}") } else { format!("{material:?}") };
            let material = world_shared
                .entry((m.skybox, own(m), key))
                .or_insert_with(|| world_materials.add(material))
                .clone();
            let merged = merge_spot(m).filter(|_| !animated);
            if let Some((_, node, home, centre)) = &merged {
                world_merger.add((material.clone(), blended), *node, home, *centre, m);
            }
            let decal = is_decal(m);
            for (chunk, clusters, centre) in chunks {
                let mut e = commands.spawn((
                    Name::new(chunk.material.clone()),
                    MapPart,
                    world_layer_of(chunk.skybox),
                    Mesh3d(meshes.add({
                        let mut mesh = build_mesh(&chunk, true);
                        if blended {
                            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, blend_colors(&chunk));
                        }
                        mesh
                    })),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(centre),
                    ChildOf(if own_node { s.parent_of(m) } else { statics }),
                ));
                if merged.is_some() {
                    e.insert((merge::MergedPiece, Visibility::Hidden));
                }
                if m.pane_look != PaneLook::Whole {
                    e.insert((PanePart(m.pane_look), Visibility::Hidden));
                }
                if decal {
                    e.insert(super::blend_order::DecalOrder(decal_order));
                    decal_order += 1;
                }
                s.tag(&mut e, clusters);
            }
            continue;
        }
        // Window edge pieces need the world shader's mask (a map without
        // lightmaps draws its broken windows without them).
        if matches!(m.pane_look, PaneLook::Edge(_)) {
            continue;
        }
        let material = build_material(m, textures, view, data.look.light_scale);
        let material = standard_shared
            .entry((m.skybox, own(m), format!("{material:?}")))
            .or_insert_with(|| materials.add(material))
            .clone();
        let merged = merge_spot(m);
        if let Some((_, node, home, centre)) = &merged {
            standard_merger.add((material.clone(), lit.is_some()), *node, home, *centre, m);
        }
        for (chunk, clusters, centre) in chunks {
            let mut part = commands.spawn((
                Name::new(chunk.material.clone()),
                MapPart,
                world_layer_of(chunk.skybox),
                Mesh3d(meshes.add(build_mesh(&chunk, lit.is_some()))),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(centre),
                ChildOf(if own_node { s.parent_of(m) } else { statics }),
            ));
            if let Some(image) = lit {
                part.insert(bevy::pbr::Lightmap {
                    image: image.clone(),
                    uv_rect: Rect::new(0.0, 0.0, 1.0, 1.0),
                    bicubic_sampling: false,
                });
            }
            if merged.is_some() {
                part.insert((merge::MergedPiece, Visibility::Hidden));
            }
            if !m.frames.is_empty() {
                part.insert(super::tint::TextureFrames(
                    m.frames.iter().map(|f| textures[*f].clone()).collect(),
                ));
            }
            if m.pane_look != PaneLook::Whole {
                part.insert((PanePart(m.pane_look), Visibility::Hidden));
            }
            s.tag(&mut part, clusters);
        }
    }
    spawn_merged(
        commands,
        s,
        meshes,
        world_merger,
        standard_merger,
        lightmaps.plain.as_ref(),
    );
    for (node, (min, max)) in node_bounds {
        commands
            .entity(node)
            .insert((BrushEntityBounds { min, max }, VisClusters::new(Vec::new())));
    }
}

/// A lightmapped world surface's material (Source-style texture x baked
/// light), and whether it blends two textures (the weights ride in the
/// vertex colour's alpha).
fn lit_world_material(
    m: &MapMesh,
    s: &MapSpawn,
    textures: &[Handle<Image>],
    cubemaps: &[Handle<Image>],
    lightmaps: &Lightmaps,
    lm: &Handle<Image>,
) -> (WorldMaterial, bool) {
    let (data, view) = (s.data, s.view);
    let bumped_lightmaps = &lightmaps.bumped;
    let [r, g, b] = m.color;
    // Calibration hook: CHIMERA_NORMAL_G_SIGN overrides the sign;
    // 0 turns radiosity normal mapping off.
    let g_sign = chimera_core::env::var("CHIMERA_NORMAL_G_SIGN")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(NORMAL_G_SIGN);
    let bumped = m.normal_map.is_some() && bumped_lightmaps.is_some() && g_sign != 0.0;
    // Two-texture blend: the weights ride in the vertex color's alpha.
    let blend = m.blend.unwrap_or_default();
    let blended = blend.texture.is_some() && m.blend_weights.len() == m.positions.len();
    let material = WorldMaterial {
        params: WorldParams {
            base_color: if m.texture.is_some() {
                Vec4::ONE
            } else {
                Vec4::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
            } * m.render_factor(),
            light_scale: data.look.light_scale,
            lightmap_scale: if lightmaps.source_ldr {
                SOURCE_LIGHTMAP_SCALE
            } else {
                1.0
            },
            bicubic: if data.look.bicubic_lightmaps { 1.0 } else { 0.0 },
            translucent: if m.modulate { 3.0 } else { m.alpha.shader_mode() },
            blend: if blended { 1.0 } else { 0.0 },
            blend_masked: if blend.mask.is_some() { 1.0 } else { 0.0 },
            blend_normal: if bumped && blend.normal_map.is_some() { 1.0 } else { 0.0 },
            detail: m.detail.map_or(0.0, |d| d.mode.shader_value()),
            detail_factor: m.detail.map_or(0.0, |d| d.factor),
            detail_scale: m.detail.map_or(Vec2::ONE, |d| Vec2::from_array(d.scale)),
            detail_tint: m.detail.map_or(Vec4::ONE, |d| Vec3::from_array(d.tint).extend(1.0)),
            base_uv_u: m.base_transform.shader_rows()[0],
            base_uv_v: m.base_transform.shader_rows()[1],
            base2_uv_u: blend.transform.shader_rows()[0],
            base2_uv_v: blend.transform.shader_rows()[1],
            selfillum: if m.selfillum.is_some() { 1.0 } else { 0.0 },
            unlit: if m.unlit { 1.0 } else { m.unlit_factor },
            modulation: m.tint.map_or(Vec4::ONE, |t| Vec3::from_array(t).extend(1.0)),
            pane_mask: if m.pane_mask.is_some() { 1.0 } else { 0.0 },
            selfillum_tint: m.selfillum.map_or(Vec4::ONE, |s| Vec3::from_array(s.tint).extend(1.0)),
            fog_color: fog_color(material_fog(data, view, m.skybox).as_ref()),
            fog_range: fog_range(material_fog(data, view, m.skybox).as_ref()),
            bumped: if bumped { 1.0 } else { 0.0 },
            normal_g_sign: g_sign,
            normal_x_sign: chimera_core::env::var("CHIMERA_NORMAL_X_SIGN")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(NORMAL_X_SIGN),
            alpha_cutoff: if let MapAlpha::Mask(c) = m.alpha { c } else { 0.0 },
            debug_view: match view {
                MapDebugView::Normal => 0.0,
                MapDebugView::Lighting { .. } => 1.0,
                MapDebugView::Albedo => 2.0,
            },
            envmap: match m.envmap {
                Some(e) if e.cubemap.is_some() && e.imposter => 2.0,
                Some(e) if e.cubemap.is_some() => 1.0,
                _ => 0.0,
            },
            envmap_mask: match m.envmap.map(|e| e.mask) {
                Some(EnvmapMask::NormalAlpha) if m.normal_map.is_some() => 1.0,
                Some(EnvmapMask::BaseAlphaInverted) => 2.0,
                Some(EnvmapMask::Texture(_)) => 3.0,
                _ => 0.0,
            },
            has_normal: if m.normal_map.is_some() && g_sign != 0.0 {
                1.0
            } else {
                0.0
            },
            envmap_contrast: m.envmap.map_or(0.0, |e| e.contrast),
            envmap_saturation: m.envmap.map_or(1.0, |e| e.saturation),
            envmap_fresnel: m.envmap.map_or(1.0, |e| e.fresnel),
            envmap_tint: m.envmap.map_or(Vec4::ONE, |e| Vec3::from_array(e.tint).extend(1.0)),
            blend_mode: blend.mode.shader_value(),
            blend_uv2: if blended && m.uvs2.len() == m.positions.len() {
                1.0
            } else {
                0.0
            },
            detail_rotation: m.detail.map_or(0.0, |d| d.rotation),
            flat_envmap: match m.flat_envmap {
                Some(e) if e.signed_add => 1.0,
                Some(_) => 2.0,
                None => 0.0,
            },
            flat_envmap_alpha: if m.flat_envmap.is_some_and(|e| e.by_base_alpha) {
                1.0
            } else {
                0.0
            },
            flat_envmap_scale: m.flat_envmap.map_or(1.0, |e| e.scale),
            ..WorldParams::identity_uv()
        },
        base: m.texture.map(|i| textures[i].clone()),
        // Bound for radiosity bump lighting, and for reflections
        // that follow the normal map.
        normal: m
            .normal_map
            .filter(|_| bumped || (m.envmap.is_some() && g_sign != 0.0))
            .map(|i| textures[i].clone()),
        envmap: m.envmap.and_then(|e| e.cubemap).map(|c| cubemaps[c].clone()),
        envmap_mask: match m.envmap.map(|e| e.mask) {
            Some(EnvmapMask::Texture(i)) => Some(textures[i].clone()),
            _ => None,
        },
        lightmap: Some(lm.clone()),
        lightmap_b0: bumped_lightmaps.as_ref().map(|b| b[0].clone()),
        lightmap_b1: bumped_lightmaps.as_ref().map(|b| b[1].clone()),
        lightmap_b2: bumped_lightmaps.as_ref().map(|b| b[2].clone()),
        base2: blend.texture.filter(|_| blended).map(|i| textures[i].clone()),
        normal2: blend
            .normal_map
            .filter(|_| blended && bumped)
            .map(|i| textures[i].clone()),
        blend_mask: match m.pane_mask {
            Some(i) => Some(textures[i].clone()),
            None => blend.mask.filter(|_| blended).map(|i| textures[i].clone()),
        },
        detail: m.detail.map(|d| textures[d.texture].clone()),
        flat_envmap: m.flat_envmap.map(|e| textures[e.texture].clone()),
        alpha_mode: m.alpha.shader_alpha_mode(),
        double_sided: m.double_sided,
        // Map overlays and infodecals (see WorldMaterial::decal).
        decal: is_decal(m),
        modulate: m.modulate,
        // A sprite section: its frames, shown on its own clock.
        skin_frames: m
            .skin_frames
            .as_ref()
            .map_or_else(Vec::new, |f| f.frames.iter().map(|&i| textures[i].clone()).collect()),
        skin_frame_ms: m.skin_frames.as_ref().map_or(0, |f| f.frame_ms),
    };
    (material, blended)
}

/// A blended mesh's vertex colours: the blend weight in alpha, and the
/// second coordinate set (`MapMesh::uvs2`) in red and green when it has
/// one (white otherwise).
fn blend_colors(m: &MapMesh) -> Vec<[f32; 4]> {
    if m.uvs2.len() == m.blend_weights.len() {
        m.uvs2.iter().zip(&m.blend_weights).map(|([u, v], w)| [*u, *v, 1.0, *w]).collect()
    } else {
        m.blend_weights.iter().map(|w| [1.0, 1.0, 1.0, *w]).collect()
    }
}

/// A map overlay or infodecal drawn as one (`WorldMaterial::decal`).
fn is_decal(m: &MapMesh) -> bool {
    m.material.starts_with("decal:") && m.alpha != MapAlpha::Opaque
}

/// Brush entities drawn merged (`merge`): the combined meshes, under the
/// static root (world space), and each node's list of its chunks.
fn spawn_merged(
    commands: &mut Commands,
    s: &MapSpawn,
    meshes: &mut Assets<Mesh>,
    world_merger: merge::Merger<(Handle<WorldMaterial>, bool)>,
    standard_merger: merge::Merger<(Handle<StandardMaterial>, bool)>,
    lightmap: Option<&Handle<Image>>,
) {
    let (statics, visibility) = (s.statics, s.visibility);
    let mut node_chunks: HashMap<Entity, Vec<Entity>> = HashMap::new();
    let mut spawn_chunk = |commands: &mut Commands,
                           mesh: Handle<Mesh>,
                           lightmap: Option<bevy::pbr::Lightmap>,
                           name: &str,
                           centre: Vec3,
                           parts: Vec<(Entity, std::ops::Range<usize>)>,
                           indices: Vec<u32>,
                           clusters: Vec<u32>|
     -> Entity {
        let mut e = commands.spawn((
            Name::new(format!("Merged brush entities ({name})")),
            MapPart,
            world_layer_of(false),
            Mesh3d(mesh),
            Transform::from_translation(centre),
            Visibility::default(),
            ChildOf(statics),
        ));
        if let Some(l) = lightmap {
            e.insert(l);
        }
        let id = e.id();
        for (node, _) in &parts {
            let list = node_chunks.entry(*node).or_default();
            if list.last() != Some(&id) {
                list.push(id);
            }
        }
        e.insert(merge::MergedChunk { parts, indices });
        if visibility.is_some() && !clusters.is_empty() {
            e.insert(VisClusters::new(clusters));
        }
        id
    };
    for ((material, blended), chunk, centre, parts, clusters) in world_merger.finish(visibility) {
        let mut mesh = build_mesh(&chunk, true);
        if blended {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, blend_colors(&chunk));
        }
        let mesh = meshes.add(mesh);
        let e = spawn_chunk(
            commands,
            mesh,
            None,
            &chunk.material,
            centre,
            parts,
            chunk.indices,
            clusters,
        );
        commands.entity(e).insert(MeshMaterial3d(material));
    }
    for ((material, lit), chunk, centre, parts, clusters) in standard_merger.finish(visibility) {
        let lm = lightmap.filter(|_| lit).map(|image| bevy::pbr::Lightmap {
            image: image.clone(),
            uv_rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            bicubic_sampling: false,
        });
        let mesh = meshes.add(build_mesh(&chunk, lit));
        let e = spawn_chunk(
            commands,
            mesh,
            lm,
            &chunk.material,
            centre,
            parts,
            chunk.indices,
            clusters,
        );
        commands.entity(e).insert(MeshMaterial3d(material));
    }
    let node_index: HashMap<Entity, usize> = s
        .entity_nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| n.map(|n| (n, i)))
        .collect();
    for (node, chunks) in node_chunks {
        let Some(&i) = node_index.get(&node) else { continue };
        commands.entity(node).insert(merge::MergedBrush {
            home: s.node_home(i),
            chunks,
            merged: true,
        });
    }
}

/// Prop models' meshes and materials, shared by their placements: standard
/// materials, and lit prop materials (per skin too) when they exist.
fn build_model_materials(
    drawn: &mut Drawn,
    s: &MapSpawn,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    prop_materials: &mut Option<ResMut<Assets<PropMaterial>>>,
) {
    let (data, view) = (s.data, s.view);
    let textures = &drawn.textures;
    drawn.model_parts = data
        .models
        .iter()
        .map(|model| {
            model
                .meshes
                .iter()
                .map(|m| {
                    (
                        meshes.add(build_mesh(m, false)),
                        materials.add(build_material(m, textures, view, data.look.light_scale)),
                    )
                })
                .collect()
        })
        .collect();
    if let Some(prop_materials) = prop_materials.as_mut() {
        drawn.lit_model_materials = data
            .models
            .iter()
            .map(|model| {
                model
                    .meshes
                    .iter()
                    .map(|m| {
                        [false, true]
                            .map(|skybox| prop_materials.add(lit_prop_material(m, textures, data, view, skybox)))
                    })
                    .collect()
            })
            .collect();
        drawn.skin_model_materials = data
            .models
            .iter()
            .map(|model| {
                model
                    .skins
                    .iter()
                    .map(|looks| {
                        model
                            .meshes
                            .iter()
                            .zip(looks)
                            .map(|(m, look)| {
                                let m = look.apply(m);
                                [false, true].map(|skybox| {
                                    prop_materials.add(lit_prop_material(&m, textures, data, view, skybox))
                                })
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();
    }
}

/// Map effects: dust, steam jets, sprite trails, particle systems, colour
/// correction, muzzle flashes, embers, teslas, strikes and smoke stacks.
fn spawn_effects(
    commands: &mut Commands,
    s: &MapSpawn,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    mut heat_materials: Option<&mut Assets<steam::HeatWaveMaterial>>,
    textures: &[Handle<Image>],
) {
    let (data, root, visibility) = (s.data, s.root, s.visibility);
    for (i, dust) in data.source_fx().dust.iter().enumerate() {
        let mesh = meshes.add(dust::empty_mesh());
        // Culled by its bounds grown by its largest mote (9).
        let grow = Vec3::splat(if dust.world_size { dust.size.1 } else { 0.0 });
        let clusters = visibility
            .map(|v| vis::box_clusters(v, dust.min - grow, dust.max + grow))
            .unwrap_or_default();
        let mut e = commands.spawn((
            Name::new(format!("Dust {i}")),
            MapPart,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: dust.texture.map(|t| textures[t].clone()),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            })),
            dust::DustEmitter::new(dust.clone(), mesh, i as u64 + 1),
            // The mesh moves with the motes; don't cull it by stale bounds.
            bevy::camera::visibility::NoFrustumCulling,
            bevy::light::NotShadowCaster,
            Transform::default(),
            ChildOf(root),
        ));
        s.tag(&mut e, clusters);
        if let Some(entity) = dust.entity {
            e.insert(EntityPart {
                entity,
                on: dust.start_on,
                exists: true,
            });
        }
    }
    // Steam jets: a puff mesh each, rebuilt per frame.
    for (i, jet) in data.source_fx().steam.iter().enumerate() {
        let mesh = meshes.add(dust::empty_mesh());
        let end = jet.origin + jet.forward * jet.length;
        let reach = Vec3::splat(jet.length * 0.5 + jet.end_size * 2.0);
        let clusters = visibility
            .map(|v| vis::box_clusters(v, jet.origin.min(end) - reach, jet.origin.max(end) + reach))
            .unwrap_or_default();
        let mut e = commands.spawn((
            Name::new(format!("Steam {i}")),
            MapPart,
            Mesh3d(mesh.clone()),
            steam::SteamEmitter::new(jet.clone(), mesh, i as u64 + 1),
            bevy::camera::visibility::NoFrustumCulling,
            bevy::light::NotShadowCaster,
            Transform::default(),
            ChildOf(root),
        ));
        if jet.heat_wave {
            if let Some(heat_materials) = heat_materials.as_mut() {
                e.insert(MeshMaterial3d(heat_materials.add(steam::HeatWaveMaterial::default())));
            }
        } else {
            e.insert(MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: jet.texture.map(|t| textures[t].clone()),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            })));
        }
        s.tag(&mut e, clusters);
        if let Some(entity) = jet.entity {
            e.insert(EntityPart {
                entity,
                on: jet.start_on,
                exists: true,
            });
        }
    }
    // Sprite trails and smoke stacks: an entity each where its map
    // entity is (moved with it), drawn through the particle pool.
    for (i, t) in data.trails.iter().enumerate() {
        let mut e = commands.spawn((
            Name::new(format!("Trail {i}")),
            MapPart,
            emitters::TrailEmitter::new(t.clone()),
            FollowsEntity,
            Transform::from_translation(t.position),
            Visibility::default(),
            ChildOf(root),
        ));
        if let Some(entity) = t.entity {
            e.insert(EntityPart {
                entity,
                on: true,
                exists: true,
            });
        }
    }
    for (i, p) in data.particle_systems.placed.iter().enumerate() {
        let reach = Vec3::splat(256.0 * data.particle_systems.scale);
        let clusters = visibility
            .map(|v| vis::box_clusters(v, p.position - reach, p.position + reach))
            .unwrap_or_default();
        let mut e = commands.spawn((
            Name::new(format!("Particle system {i}")),
            MapPart,
            super::psys::SystemEmitter::new(p.clone(), i as u64 + 1),
            FollowsEntity,
            Transform::from_translation(p.position).with_rotation(p.rotation),
            Visibility::default(),
            ChildOf(root),
        ));
        s.tag(&mut e, clusters);
        if let Some(entity) = p.entity {
            e.insert(EntityPart {
                entity,
                on: p.start_on,
                exists: true,
            });
        }
    }
    if !data.particle_systems.defs.is_empty() {
        commands.insert_resource(super::psys::ParticleSystemDefs(data.particle_systems.clone()));
    }
    super::color_correction::spawn(commands, data, root);
    for (i, m) in data.muzzle_flashes.iter().enumerate() {
        let mut e = commands.spawn((
            Name::new(format!("Muzzle flash {i}")),
            MapPart,
            emitters::MuzzleFlashEmitter(m.clone()),
            FollowsEntity,
            Transform::from_translation(m.position).with_rotation(m.rotation),
            Visibility::default(),
            ChildOf(root),
        ));
        if let Some(entity) = m.entity {
            e.insert(EntityPart {
                entity,
                on: true,
                exists: true,
            });
        }
    }
    for (i, m) in data.source_fx().embers.iter().enumerate() {
        let clusters = visibility
            .map(|v| vis::box_clusters(v, m.min, m.max))
            .unwrap_or_default();
        let mut e = commands.spawn((
            Name::new(format!("Embers {i}")),
            MapPart,
            emitters::EmbersEmitter::new(m.clone(), i as u64 + 1),
            FollowsEntity,
            Transform::from_translation(m.origin),
            Visibility::default(),
            ChildOf(root),
        ));
        s.tag(&mut e, clusters);
        if let Some(entity) = m.entity {
            e.insert(EntityPart {
                entity,
                on: m.start_on,
                exists: true,
            });
        }
    }
    for (i, t) in data.source_fx().teslas.iter().enumerate() {
        let mut e = commands.spawn((
            Name::new(format!("Tesla {i}")),
            MapPart,
            emitters::TeslaEmitter::new(t.clone(), i as u64 + 1),
            FollowsEntity,
            Transform::from_translation(t.position),
            Visibility::default(),
            ChildOf(root),
        ));
        if let Some(entity) = t.entity {
            e.insert(EntityPart {
                entity,
                on: true,
                exists: true,
            });
        }
    }
    for (i, strike) in data.source_fx().strikes.iter().enumerate() {
        commands.spawn((
            Name::new(format!("Strike beams {i}")),
            MapPart,
            emitters::StrikeEmitter::new(strike.clone(), i as u64 + 1),
            Transform::default(),
            Visibility::default(),
            ChildOf(root),
        ));
    }
    for (i, stack) in data.source_fx().smokestacks.iter().enumerate() {
        let reach = stack.length + stack.base_spread + stack.end_size * 2.0;
        let clusters = visibility
            .map(|v| {
                vis::box_clusters(
                    v,
                    stack.position - Vec3::splat(reach),
                    stack.position + Vec3::splat(reach),
                )
            })
            .unwrap_or_default();
        let mut e = commands.spawn((
            Name::new(format!("Smoke stack {i}")),
            MapPart,
            emitters::SmokeStackEmitter::new(stack.clone(), i as u64 + 1),
            FollowsEntity,
            Transform::from_translation(stack.position).with_rotation(stack.rotation),
            Visibility::default(),
            ChildOf(root),
        ));
        s.tag(&mut e, clusters);
        if let Some(entity) = stack.entity {
            e.insert(EntityPart {
                entity,
                on: stack.start_on,
                exists: true,
            });
        }
    }
}

/// Camera-facing sprites (lamp glows), switched by their entity.
fn spawn_sprites(
    commands: &mut Commands,
    s: &MapSpawn,
    meshes: &mut Assets<Mesh>,
    sprite_materials: &mut Assets<SpriteMaterial>,
    textures: &[Handle<Image>],
) {
    let (data, root, visibility) = (s.data, s.root, s.visibility);
    for (i, sprite) in data.sprites.iter().enumerate() {
        let t = &data.textures[sprite.texture];
        let mut e = commands.spawn((
            Name::new(format!("Sprite {i}")),
            MapPart,
            Mesh3d(meshes.add(sprite_material::sprite_mesh(UVec2::new(t.width, t.height)))),
            MeshMaterial3d(sprite_materials.add(SpriteMaterial {
                params: SpriteParams {
                    color: sprite.color,
                    size: sprite.size,
                },
                texture: Some(textures[sprite.texture].clone()),
                glow: sprite.glow,
                blend: sprite.blend,
            })),
            bevy::light::NotShadowCaster,
            // The quad is spread in the vertex shader, so the mesh's
            // own bounds are a point: culling would drop the sprite
            // as soon as its centre left the view.
            bevy::camera::visibility::NoFrustumCulling,
            FollowsEntity,
            Transform::from_translation(sprite.position),
            ChildOf(root),
        ));
        if let Some(v) = visibility {
            let r = Vec3::splat(sprite.size.max_element() / 2.0);
            s.tag(&mut e, vis::box_clusters(v, sprite.position - r, sprite.position + r));
        }
        if sprite.glow {
            e.insert(GlowSprite {
                color: sprite.color,
                proxy: sprite.proxy,
            });
        }
        if let Some(entity) = sprite.entity {
            e.insert(EntityPart {
                entity,
                on: sprite.start_on,
                exists: true,
            });
            if !sprite.start_on {
                e.insert((LogicHidden, Visibility::Hidden));
            }
        }
    }
}

/// Beams (point_spotlight, env_laser, env_beam), spotlights' halos and
/// light glows (`beams`).
fn spawn_beams(
    commands: &mut Commands,
    s: &MapSpawn,
    meshes: &mut Assets<Mesh>,
    beam_materials: &mut Assets<super::beams::BeamMaterial>,
    sprite_materials: &mut Assets<SpriteMaterial>,
    textures: &[Handle<Image>],
) {
    let (data, root, visibility) = (s.data, s.root, s.visibility);
    let part = |e: &mut EntityCommands, entity: Option<usize>, on: bool| {
        if let Some(entity) = entity {
            e.insert(EntityPart {
                entity,
                on,
                exists: true,
            });
        }
        if !on {
            e.insert((LogicHidden, Visibility::Hidden));
        }
    };
    let beam_mesh = meshes.add(super::beams::beam_mesh());
    // Hidden where the camera can't see them: the clusters along a
    // beam (a box every meter), around a glow.
    let segment_clusters = |a: Vec3, b: Vec3| {
        let Some(v) = visibility else { return Vec::new() };
        let steps = (a.distance(b).ceil() as usize).clamp(1, 4096);
        let mut out = Vec::new();
        for k in 0..=steps {
            let p = a.lerp(b, k as f32 / steps as f32);
            v.clusters_in_box(p - Vec3::splat(0.5), p + Vec3::splat(0.5), &mut out);
        }
        out.sort_unstable();
        out.dedup();
        out
    };
    for (i, b) in data.beams.iter().enumerate() {
        let mut e = commands.spawn((
            Name::new(format!("Beam {i}")),
            MapPart,
            Mesh3d(beam_mesh.clone()),
            MeshMaterial3d(beam_materials.add(super::beams::BeamMaterial {
                params: super::beams::beam_params(b),
                texture: Some(textures[b.texture].clone()),
            })),
            bevy::light::NotShadowCaster,
            // Placed in the vertex shader.
            bevy::camera::visibility::NoFrustumCulling,
            Transform::default(),
            ChildOf(root),
        ));
        part(&mut e, b.entity, b.start_on);
        // Clusters along the beam, out to its noise's reach (fractal
        // samples stay within ±2).
        let reach = 2.0 * b.noise * b.start.distance(b.end) / 100.0;
        let mut clusters = segment_clusters(b.start, b.end);
        if reach > 0.0
            && let Some(v) = visibility
        {
            let (lo, hi) = (
                b.start.min(b.end) - Vec3::splat(reach),
                b.start.max(b.end) + Vec3::splat(reach),
            );
            v.clusters_in_box(lo, hi, &mut clusters);
            clusters.sort_unstable();
            clusters.dedup();
        }
        s.tag(&mut e, clusters);
        if b.animated() {
            e.insert(super::beams::BeamMotion {
                beam: b.clone(),
                frames: b.frames.iter().map(|f| textures[*f].clone()).collect(),
                rng: ParticleRng::new(i as u64 + 1),
            });
        }
        if let Some(h) = &b.spot {
            let t = &data.textures[h.texture];
            let mut e = commands.spawn((
                Name::new(format!("Beam {i} halo")),
                MapPart,
                Mesh3d(meshes.add(sprite_material::sprite_mesh(UVec2::new(t.width, t.height)))),
                MeshMaterial3d(sprite_materials.add(SpriteMaterial {
                    params: SpriteParams {
                        color: h.color.extend(1.0),
                        size: Vec2::splat(h.scale * 2.0),
                    },
                    texture: Some(textures[h.texture].clone()),
                    glow: true,
                    blend: crate::render::sprite_material::SpriteBlend::Add,
                })),
                bevy::light::NotShadowCaster,
                bevy::camera::visibility::NoFrustumCulling,
                Transform::from_translation(b.start),
                GlowSprite {
                    color: h.color.extend(1.0),
                    proxy: h.proxy,
                },
                super::beams::EyeGlow::Halo {
                    halo: h.clone(),
                    dir: (b.end - b.start).normalize_or_zero(),
                },
                ChildOf(root),
            ));
            part(&mut e, b.entity, b.start_on);
            s.tag(&mut e, segment_clusters(b.start, b.start));
        }
    }
    for (i, g) in data.glows.iter().enumerate() {
        let t = &data.textures[g.texture];
        let mut e = commands.spawn((
            Name::new(format!("Light glow {i}")),
            MapPart,
            Mesh3d(meshes.add(sprite_material::sprite_mesh(UVec2::new(t.width, t.height)))),
            MeshMaterial3d(sprite_materials.add(SpriteMaterial {
                params: SpriteParams {
                    color: g.color.extend(1.0),
                    size: g.half * 2.0,
                },
                texture: Some(textures[g.texture].clone()),
                glow: true,
                blend: crate::render::sprite_material::SpriteBlend::Add,
            })),
            bevy::light::NotShadowCaster,
            bevy::camera::visibility::NoFrustumCulling,
            Transform::from_translation(g.position),
            GlowSprite {
                color: g.color.extend(1.0),
                proxy: g.proxy,
            },
            super::beams::EyeGlow::Light(g.clone()),
            ChildOf(root),
        ));
        part(&mut e, g.entity, true);
        s.tag(&mut e, segment_clusters(g.position, g.position));
    }
}

/// Ropes and cables: a strip each (and a back strip), swaying in the
/// wind (`rope_sway`).
fn spawn_ropes(
    commands: &mut Commands,
    s: &MapSpawn,
    meshes: &mut Assets<Mesh>,
    rope_materials: &mut Assets<RopeMaterial>,
    textures: &[Handle<Image>],
) {
    let (data, statics, visibility) = (s.data, s.statics, s.visibility);
    for (i, rope) in data.ropes.iter().enumerate() {
        let mesh = meshes.add(rope_material::rope_mesh(rope));
        let clusters = match visibility {
            Some(v) if !rope.points.is_empty() => {
                let lo = rope.points.iter().fold(Vec3::MAX, |a, p| a.min(*p));
                let hi = rope.points.iter().fold(Vec3::MIN, |a, p| a.max(*p));
                let r = Vec3::splat(rope.width);
                vis::box_clusters(v, lo - r, hi + r)
            }
            _ => Vec::new(),
        };
        let strips = rope
            .back
            .map(|(t, n)| (t, n, true))
            .into_iter()
            .chain([(rope.texture, rope.normal_map, false)]);
        let mut swaying = rope.sway.clone();
        for (texture, normal, back) in strips {
            let mut e = commands.spawn((
                Name::new(format!("Rope {i}{}", if back { " (back)" } else { "" })),
                MapPart,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(rope_materials.add(RopeMaterial {
                    params: RopeParams {
                        width: rope.width,
                        back: if back { 1.0 } else { 0.0 },
                        light_scale: data.look.light_scale,
                        has_normal_map: if normal.is_some() { 1.0 } else { 0.0 },
                        fog: fog::world_fog(data.fog.as_ref()),
                    },
                    base: texture.map(|t| textures[t].clone()),
                    normal: normal.map(|t| textures[t].clone()),
                    blend: back,
                })),
                bevy::light::NotShadowCaster,
                Transform::default(),
                ChildOf(statics),
            ));
            s.tag(&mut e, clusters.clone());
            // The strips share the mesh: one of them moves it.
            if let Some(sway) = swaying.take() {
                e.insert(super::rope_sway::RopeMotion {
                    gusts: super::rope_sway::Gusts::new(i as u64 + 1),
                    sway,
                    look: rope.clone(),
                    mesh: mesh.clone(),
                    pending: 0.0,
                    resting: true,
                });
            }
        }
    }
}

/// Assets props of one model share across its placements.
#[derive(Default)]
struct PropShared {
    /// Each animated model's inverse bind poses.
    rig_bindposes: HashMap<usize, Handle<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    /// Each model's levels of detail per mesh.
    lod_parts: HashMap<usize, Vec<Vec<Handle<Mesh>>>>,
    /// Prop materials with the nearest cubemap: (model, mesh, sky layer,
    /// cubemap).
    envmap_variants: std::collections::HashMap<(usize, usize, bool, usize), Handle<PropMaterial>>,
}

/// What a prop's meshes are spawned for: its node, model and look.
struct PropMeshes<'a> {
    /// The prop's node.
    id: Entity,
    prop: &'a MapProp,
    model: &'a MapModel,
    fade_range: Option<&'a bevy::camera::visibility::VisibilityRange>,
    /// With levels of detail (`lod`).
    leveled: bool,
}

impl PropMeshes<'_> {
    /// Body parts: only the meshes of the chosen choice are drawn.
    fn body_shown(&self, m: &MapMesh) -> bool {
        m.body
            .is_none_or(|(part, choice)| self.model.body_choice(self.prop.body, part as usize) == choice)
    }
}

/// A prop lit by its baked light: its own mesh copies with per-vertex
/// light and unlit materials (texture x light, like the lightmapped
/// world), skinned to its skeleton. Returns the materials it drew with.
#[allow(clippy::too_many_arguments)]
fn spawn_baked_prop_meshes(
    commands: &mut Commands,
    data: &MapData,
    p: &PropMeshes,
    meshes: &mut Assets<Mesh>,
    prop_materials: &mut Option<ResMut<Assets<PropMaterial>>>,
    drawn: &Drawn,
    shared: &mut PropShared,
    mats: &[[Handle<PropMaterial>; 2]],
    probe: &LightProbe,
    probe_scale: f32,
    inverse_bindposes: Option<Handle<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    joints: &[Entity],
) -> Vec<Handle<PropMaterial>> {
    let (id, prop, model) = (p.id, p.prop, p.model);
    let mut spawned_materials: Vec<Handle<PropMaterial>> = Vec::new();
    for (mesh_index, (m, material)) in model.meshes.iter().zip(mats).enumerate() {
        let mut material = material[prop.skybox as usize].clone();
        // `$envmap`: the cubemap it names, or for `env_cubemap` the one
        // baked nearest to the prop.
        if let (Some(env), Some(prop_materials)) = (m.envmap, prop_materials.as_mut())
            && let Some(cube) = env.cubemap.or_else(|| {
                data.cubemap_samples
                    .iter()
                    .min_by(|a, b| {
                        a.0.distance(prop.translation)
                            .total_cmp(&b.0.distance(prop.translation))
                    })
                    .map(|s| s.1)
            })
            && let Some(cube_handle) = drawn.cubemaps.get(cube)
        {
            let key = (prop.model, mesh_index, prop.skybox, cube);
            material = shared
                .envmap_variants
                .entry(key)
                .or_insert_with(|| {
                    let mut variant = prop_materials.get(&material).cloned().expect("prop material");
                    set_prop_envmap(&mut variant, &env, cube_handle.clone(), &drawn.textures, m.normal_map);
                    prop_materials.add(variant)
                })
                .clone();
        }
        spawned_materials.push(material.clone());
        let layer = layer_of(prop.skybox);
        // Light baked per vertex when the prop has it (and it
        // covers the mesh), else the probe on each normal.
        let colors = |m: &MapMesh| -> Vec<[f32; 4]> {
            let baked = prop.vertex_light.as_ref().filter(|v| {
                m.source_vertices.len() == m.normals.len() && m.source_vertices.iter().all(|&i| (i as usize) < v.len())
            });
            m.normals
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    if m.unlit {
                        return [1.0, 1.0, 1.0, 1.0];
                    }
                    let l = match baked {
                        Some(v) => Vec3::from(v[m.source_vertices[i] as usize]),
                        None => probe.eval(prop.rotation * Vec3::from(*n)),
                    } * probe_scale;
                    [l.x, l.y, l.z, 1.0]
                })
                .collect()
        };
        let mut mesh = build_mesh(m, false);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors(m));
        let skinned = inverse_bindposes.is_some() && m.joints.len() == m.positions.len();
        if skinned {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_JOINT_INDEX,
                bevy::mesh::VertexAttributeValues::Uint16x4(m.joints.clone()),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, m.joint_weights.clone());
        }
        let handle = meshes.add(mesh);
        // Its lower levels, lit the same way (a level without
        // this mesh keeps the finer one's).
        // A skinned one's keep the skeleton (the same joints).
        let levels = p.leveled.then(|| {
            let mut levels = vec![handle.clone()];
            for l in &model.lods {
                let h = match l.meshes.get(mesh_index).and_then(Option::as_ref) {
                    Some(lm) => {
                        let mut mesh = build_mesh(lm, false);
                        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors(lm));
                        if skinned && lm.joints.len() == lm.positions.len() {
                            mesh.insert_attribute(
                                Mesh::ATTRIBUTE_JOINT_INDEX,
                                bevy::mesh::VertexAttributeValues::Uint16x4(lm.joints.clone()),
                            );
                            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, lm.joint_weights.clone());
                        }
                        meshes.add(mesh)
                    }
                    None => levels.last().cloned().unwrap_or_default(),
                };
                levels.push(h);
            }
            super::lod::LodMeshes(levels)
        });
        let mut c = commands.spawn((
            Mesh3d(handle),
            MeshMaterial3d(material),
            layer,
            PropMeshSlot { mesh: mesh_index },
            if p.body_shown(m) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
            ChildOf(id),
        ));
        if let Some(range) = p.fade_range {
            c.insert(range.clone());
        }
        if let Some(levels) = levels {
            c.insert(levels);
        }
        if let (true, Some(inverse)) = (skinned, inverse_bindposes.clone()) {
            c.insert((
                bevy::mesh::skinning::SkinnedMesh {
                    inverse_bindposes: inverse,
                    joints: joints.to_vec(),
                },
                // Bounds of the bind pose don't follow the bones.
                bevy::camera::visibility::NoFrustumCulling,
            ));
        }
    }
    spawned_materials
}

/// A prop drawn with its model's shared meshes and standard materials
/// (and the model's levels of detail, built once).
fn spawn_shared_prop_meshes(
    commands: &mut Commands,
    p: &PropMeshes,
    meshes: Option<&mut Assets<Mesh>>,
    drawn: &Drawn,
    shared: &mut PropShared,
) {
    let (id, prop, model) = (p.id, p.prop, p.model);
    let Some(parts) = drawn.model_parts.get(prop.model) else {
        return;
    };
    // The model's levels, shared by its props (built once).
    // (An animated model's meshes are skinned, only the baked path's.)
    let levels = match (p.leveled && model.rig.is_none(), meshes) {
        (true, Some(meshes)) => Some(
            shared
                .lod_parts
                .entry(prop.model)
                .or_insert_with(|| {
                    parts
                        .iter()
                        .enumerate()
                        .map(|(mesh_index, (mesh, _))| {
                            let mut levels = vec![mesh.clone()];
                            for l in &model.lods {
                                let h = match l.meshes.get(mesh_index).and_then(Option::as_ref) {
                                    Some(lm) => meshes.add(build_mesh(lm, false)),
                                    None => levels.last().cloned().unwrap_or_default(),
                                };
                                levels.push(h);
                            }
                            levels
                        })
                        .collect()
                })
                .clone(),
        ),
        _ => None,
    };
    for (mesh_index, ((mesh, material), m)) in parts.iter().zip(&model.meshes).enumerate() {
        let mut c = commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            layer_of(prop.skybox),
            PropMeshSlot { mesh: mesh_index },
            if p.body_shown(m) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
            ChildOf(id),
        ));
        if let Some(range) = p.fade_range {
            c.insert(range.clone());
        }
        if let Some(levels) = levels.as_ref().and_then(|l| l.get(mesh_index)) {
            c.insert(super::lod::LodMeshes(levels.clone()));
        }
    }
}

/// A placed prop's drawn parts (`MapDrawer::spawn_prop`).
fn spawn_prop(
    commands: &mut Commands,
    s: &MapSpawn,
    a: &mut MapAssets,
    drawn: &Drawn,
    shared: &mut PropShared,
    p: PropSpawn,
) {
    let (data, view) = (s.data, s.view);
    let PropSpawn {
        prop,
        id,
        fade_range,
        home,
    } = p;
    let probe_scale = data.look.light_scale
        * if let MapDebugView::Lighting { scale } = view {
            scale
        } else {
            1.0
        };
    let probe = prop.lighting.as_ref().filter(|_| view != MapDebugView::Albedo);
    let model = &data.models[prop.model];
    // An animated prop's skeleton: a root in the skeleton's space under
    // the node, then each bone under its parent, in the reference pose.
    let rig = model
        .rig
        .as_ref()
        .filter(|_| a.meshes.is_some() && a.bindposes.is_some());
    let mut joints: Vec<Entity> = Vec::new();
    let mut inverse_bindposes = None;
    if let (Some(rig), Some(bindposes)) = (rig, a.bindposes.as_mut()) {
        let root = commands.spawn((rig.root, Visibility::Inherited, ChildOf(id))).id();
        for (i, b) in rig.bones.iter().enumerate() {
            let parent = b.parent.and_then(|p| joints.get(p).copied()).unwrap_or(root);
            let (rotation, position) = prop
                .pose
                .as_ref()
                .and_then(|pose| pose.get(i).copied())
                .unwrap_or((b.rotation, b.position));
            joints.push(
                commands
                    .spawn((
                        Transform::from_translation(position).with_rotation(rotation),
                        Visibility::Inherited,
                        ChildOf(parent),
                    ))
                    .id(),
            );
        }
        inverse_bindposes = Some(
            shared
                .rig_bindposes
                .entry(prop.model)
                .or_insert_with(|| bindposes.add(rig_inverse_bindposes(rig)))
                .clone(),
        );
    }
    // Levels of detail (`lod`): not for the 3D skybox's props. Skinned
    // models' levels share their skeleton.
    let leveled = !model.lods.is_empty() && !prop.skybox;
    if leveled {
        commands
            .entity(id)
            .insert(super::lod::LodGroup::new(model.bounds, &model.lods));
    }
    let mesh = PropMeshes {
        id,
        prop,
        model,
        fade_range,
        leveled,
    };
    let spawned_materials = match (probe, a.meshes.as_mut(), drawn.lit_model_materials.get(prop.model)) {
        // Baked: own mesh copy with per-vertex light, unlit material
        // (texture x light, like the lightmapped world).
        (Some(probe), Some(meshes), Some(mats)) => spawn_baked_prop_meshes(
            commands,
            data,
            &mesh,
            meshes,
            &mut a.prop_materials,
            drawn,
            shared,
            mats,
            probe,
            probe_scale,
            inverse_bindposes.clone(),
            &joints,
        ),
        (_, meshes_here, _) => {
            spawn_shared_prop_meshes(commands, &mesh, meshes_here.map(|m| &mut **m), drawn, shared);
            Vec::new()
        }
    };
    // What the logic may change: skin and body (entity props), sequences
    // (animated ones).
    if prop.entity.is_some() {
        let mut skins: Vec<Vec<Handle<PropMaterial>>> = drawn
            .skin_model_materials
            .get(prop.model)
            .map(|skins| {
                skins
                    .iter()
                    .map(|mats| mats.iter().map(|m| m[prop.skybox as usize].clone()).collect())
                    .collect()
            })
            .unwrap_or_default();
        if let Some(own) = skins.get_mut(prop.skin.max(0) as usize)
            && own.len() == spawned_materials.len()
        {
            *own = spawned_materials;
        }
        commands.entity(id).insert(PropVariants {
            skins,
            body: prop.body,
            parts: model.body_parts.clone(),
            mesh_bodies: model.meshes.iter().map(|m| m.body).collect(),
            shown: (prop.skin, prop.body),
        });
        // A simulated ragdoll's bones follow its bodies.
        if let (Some(rig), Some(ragdoll)) = (rig.filter(|_| !joints.is_empty() && !prop.skybox), &prop.ragdoll) {
            let flags = prop
                .entity
                .and_then(|i| data.entities.get(i))
                .and_then(|e| e.get("spawnflags"))
                .map_or(0, |v| v.trim().parse().unwrap_or(0));
            commands.entity(id).insert(placed_ragdoll::PlacedRagdoll::new(
                ragdoll.clone(),
                rig,
                prop.pose.clone(),
                joints,
                flags,
                home,
            ));
        // A posed ragdoll keeps its pose (no animation).
        } else if let Some(rig) = rig.filter(|_| !joints.is_empty() && prop.pose.is_none() && prop.ragdoll.is_none()) {
            commands.entity(id).insert(PropRig {
                animator: anim::Animator::new(rig.animations.clone()),
                joints,
                seen: None,
                default: None,
                done: false,
            });
        }
    }
}

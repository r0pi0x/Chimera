//! Combat Arms' maps: a world file from the install
//! (`lithtech_formats::world`, specs/combat_arms/world_dat.md) converted
//! into Chimera's `MapData`, the way `cs_source` converts a BSP. A map is
//! `combat_arms:<name>`, the file `worlds*/<name>.dat` in the mounted
//! archives (`mount`); the 33 maps of WORLDS2 and WORLDS3 read without
//! keys, the rest need the V2 key.
//!
//! - Axes and units ("Coordinate conversion"): Z mirrored into a
//!   right-handed Y-up frame, triangle order reversed to keep the front
//!   faces, and world units scaled by movement's `meters_per_unit` (the
//!   same scale the movement and weapons use).
//! - Drawn: the render data only ("What is drawn and what is collision
//!   only"): the main render world and the world models whose object is
//!   visible (`model_look`), one mesh per section texture and blend;
//!   sections with shader codes other than 1, 2, 4, 8 and 9 never (Q17).
//!   An object's `Visible`, `StartHidden`, `Solid`, `Rayhit` and
//!   `BlendMode` properties are read as their names say: volumes such as
//!   `EndlessFall` are invisible, light rays are additive, windows
//!   translucent.
//! - Lighting ("Lightmaps", Q7, Q9, Q10, Q12): lightmapped surfaces
//!   (shaders 4 and 9) take their lightmap from the shader-2 pass at the
//!   same place: each triangle from the shader-2 triangle with the same
//!   corners and winding (`lightmap_for`; the engine's equal-depth
//!   multiply), not by polygon index, which differs between the passes
//!   for most triangles (`heavy::world_install::
//!   texture_pass_triangles_have_lightmap_twins`; one polygon's triangles
//!   can have theirs on different pages, so a drawn vertex is shared only
//!   with the same lightmap place), packed into one atlas
//!   (`MapLightmap`) with a one-texel border copied from their edges; a
//!   triangle whose lightmap coordinates leave 0..1 is cut where they
//!   cross 0 and 1 and the parts outside take the edge (so it clamps per
//!   pixel, as the engine's sampler does). Vertex-lit sections (shaders 1
//!   and 8) light each vertex by its own colour: each triangle gets a
//!   2 x 2 atlas rectangle whose bilinear filtering is exactly the
//!   vertices' linear blend. Light groups start as the map's `LightGroup`
//!   objects say (`light_group_colours`: `StartColor` when `StartOn`,
//!   black when not, the stored colour without an object) and add
//!   min(255, base + ⌊colour × x⌋) to the lightmaps and vertex colours they
//!   cover (`add_light`). The light is the game's fixed-function multiply:
//!   2 (`Saturate`) × texture × light × `LightmapTFactor`² (`light_value`,
//!   the stored bytes as sRGB), with the map's look untonemapped (clamped,
//!   as the game's output).
//! - Textures come from the install's `.dtx` files (`lithtech_formats::
//!   dtx`, the level the shipped "DetailLevel" loads: `skipped_levels`;
//!   `alpharef` cuts out; DXT files also keep their blocks, mip levels
//!   included, which the GPU takes as BC: `dtx_bc`); the plain maps' world textures are in unkeyed
//!   archives. A sprite (`.spr`) on a map surface draws its first frame's
//!   texture (`effects::library::sprite_texture`); as a model's skin it
//!   plays its frames (`model_textures`: `MapMesh::skin_frames`, each
//!   drawn copy on its own clock, models_ltb.md "Sprite skins"), and a
//!   layered render style's slot-1 texture resolves there too. A texture that doesn't resolve
//!   (keyed archives without the V2 key; with it, a few files the install
//!   lacks or ships empty) leaves a flat colour of its own per name, named
//!   in the map's warnings.
//! - With the V2 key every map of WORLDS, WORLDS4 and WORLDS5 (72) converts
//!   too (`heavy::world`); TWINTOWER's file is a stub of 220 polygons.
//! - Second textures ("Second textures", Q13; `second_layers`), over the
//!   base texture before the lighting, in gamma space: dual-texture
//!   sections (8, 9) blend texture 1 at UV1 (`MapMesh::uvs2`) by a =
//!   vertex alpha x texture 0's alpha (`BlendMode::BaseAlpha`; added when
//!   texture 1 is fullbright, `BaseAlphaAdd`); a base texture's
//!   `detailtex` link is a detail texture at the base's detail scale and
//!   rotation (`DetailMode::SignedAdd`, `MapDetail::rotation`), its
//!   `envmap` / `envmapalpha` link a flat environment map
//!   (`MapFlatEnvmap`), both as the signed add (`DetailTextureAdd` and
//!   `EnvMapAdd` 1, the defaults). A link that doesn't resolve draws
//!   without (textures_dtx.md case 22).
//! - Sky (Q11): the last `DemoSkyWorldModel`'s world model is drawn by
//!   Chimera's sky camera (`MapMesh::skybox`, `MapSkyCamera`): it follows
//!   the player through the object's inner box (its position ±
//!   `SkyDims` × `InnerPercent`) as the player moves through the world's
//!   extents, behind the world. Fog (`WorldProperties`): `FogEnable`,
//!   `FogColor`, `FogNearZ`, `FogFarZ` as linear depth fog (`MapFog::
//!   linear`), the sky's with `SkyFogNearZ`/`SkyFogFarZ`.
//! - Collision (Q5, Q14): what players collide with
//!   (`collision_indices`): the physics BSP's polygons with the solid bit
//!   (the invisible bit plays no part) and those of the world models
//!   whose object is solid (`Solid`, or without it one of `SOLID_CLASSES`)
//!   where they stand at load. Collision sets (`MapCollisionSet`): the
//!   collision blockers, players only; the polygons without the solid bit
//!   (non-solid detail) and the world models that are ray-hit but not
//!   solid, traces only (shots, sight, cover, grenades). Polygons of the
//!   glass surface types (`GLASS_SURFACE_TYPES`) go to sets of their own,
//!   Chimera's `SeeThrough` (bots see through them, bots.md 5.2).
//! - Volumes (`volumes`): `Water`, `EndlessFall` and the other volume
//!   classes go to the map's entities (class, properties, the world
//!   model's faces as one hull) for `volumes::CaVolume`, which the map's
//!   objects switch and move (`objects`).
//! - World objects (world_objects.md 1; `world_model_nodes`): the world
//!   models that can move, hide or break (the active classes, and
//!   `WorldModel`s that are keyframed, start hidden, can be destroyed,
//!   attach others or are named in another object's commands) are the
//!   map's first entities, each a mover node of its own placed at its
//!   BSP's centre (its meshes relative to it, its polygons as hulls the
//!   node collides as triangles; where they stand at load they stay
//!   floors for the spawns' drop only); the logic objects (triggers, counters, start-up
//!   commands, keyframers with their keys' blind data, explosions,
//!   sounds, lights, cameras, points) pass through with every property
//!   (`object_keyvalues`), for `objects`.
//! - The world file's offset (`world_info`): one pass-through entity
//!   (`radar::WORLD_INFO`) for the radar's map data (radar_minimap.md R5, F3).
//! - AI objects (`objects`): the AI volumes, nodes and regions as map
//!   entities, every property as text, for the bots (`bots::map_ai`).
//! - Mode objects (`objects` too): every `GameStartPoint` (with its feet
//!   on the floor and its yaw added, `start_point_feet`), the item drop
//!   points (`ObjectSpawnPoint`), bomb sites (`BombZoneTrigger`), flags
//!   (`FlagItem`), flag zones (`FlagZoneTrigger`), use triggers
//!   (`ActivateTrigger`) and mode controllers (`TDMGameControl`), for the
//!   game modes' objectives and each mode's spawn family
//!   (`rules::objects`).
//! - Props: objects that name a model (`props`: the capture-the-flag
//!   flags, whose model needs the V2 key).
//! - View models: not with the map; each weapon's loads from the install
//!   when it is first wanted (`view_model::on_demand`).
//! - Spawns: the `GameStartPoint` objects that serve the team modes'
//!   family (`SPAWN_GAME_TYPE`, `Team Death Match`: game_modes.md S4,
//!   `qualifies`: unlocked, `Spawn` exactly 1) when it has one on a team,
//!   else the map's own family (`map_family`: the one with the most
//!   qualifying points, as HGOILRIG's `HungerGame`), in the map's order
//!   (the rules' spawn cursors walk it), else the map's first start point;
//!   dropped to the floor below them (never onto a sky portal, as the
//!   world's outer box: over the void, onto the surface a ray meets, as
//!   the sea's; else not at all), facing their yaw; `Teams` JSF is `Team(1)`, Creed `Team(2)`,
//!   `<none>` neither.
//! - Other apps: `MAP_LOADER` is these maps for an app's
//!   `map::MapSources`; `DRESSING` (`dress`) is Combat Arms' parts on
//!   another game's map (a mashup: a CS:S map played as Combat Arms).
//!   `MapLoadOptions::kit` limits the character models every map carries
//!   (`MapData::kit`): none for an app whose gameplay is another game's.
//! - No map file hash (Q15): the game checks none, a client loads its own
//!   copy by name, so the network handshake compares only the name.
//!
//! Loading (docs/performance.md, "Combat Arms map loads"): `load_from`
//! converts the world while the character models load on a thread of
//! their own (converted once per session, `characters::models::
//! shared_character_models`); lightmap texels are looked up per byte
//! (`LightLut`) and kept as bytes until the atlas is laid out; each
//! texture file is read once and decoded with its mip chain on threads
//! (`decode_all`, on `map::parallel::par_map`; none of it with
//! `MapLoadOptions::textures` off). Converted maps aren't cached on disk: a map converts in
//! 0.2-1.3 s, about what reading a cached copy of its textures and
//! lightmaps (hundreds of MB) would take.
//!
//! Known shortcuts (docs/tech-debt.md): the character models are copied
//! into every map (their textures with them); fullbright masks aren't drawn
//! (Q13 still open: whether the lightmap multiplies over them). A map
//! whose lightmaps don't fit one 8192 x 8192 atlas (`ATLAS_MAX`, the GPU's
//! limit; TERRORDOME's are 4096 x 19428) has them all at half resolution
//! (one atlas, not the game's pages). A sprite texture doesn't animate.
//! The spawns drop straight down by a ray (not the hull). HGOILRIG's
//! start points (its one team point and the `HungerGame` ones) have no
//! solid floor below them in the physics BSP, only the sliding water's
//! surface over the `Water` volume and the outer box: they land on the
//! water and float there (`movement`), as in the game: the map is a copy
//! of DARKHILLS with its terrain and barn removed, so no deck ever
//! existed (world_objects.md 16). The flat
//! environment map's world environment-map matrix is taken as the
//! identity and `EnvScale` as 1 (neither is in the specs), so it reads
//! (r.x, r.y) x -0.5 + 0.5 of the view-space reflection; the console
//! values (`DetailTextures`, `DetailTextureScale`, `EnvMapEnable`) are
//! at their defaults. Second layers are the world's only: props and view
//! models don't take their textures' links. The sky world model is drawn with
//! depth (the game draws it in order without), from one scale for all
//! three axes (the game's inner box scales each), over the whole screen
//! (the game only inside the sky portals' rectangle); `FarZ`, the sky's
//! far plane and fog-disable flag, panning skies and volumetric fog aren't
//! read. Light groups stay as they start (the objects keep their on/off
//! state only; the light grid's fix-ups aren't applied); volumes are
//! their world models' face planes.

use std::collections::HashMap;

use bevy::prelude::*;
use lithtech_formats::{
    dtx::{DETAIL_LEVEL, Dtx, FLAG_FULLBRIGHT, LinkKind, link_path},
    world::{self as lt, Object, RenderWorld, World, shader, surface_flags},
};

use crate::{
    core::Team,
    map::{
        BlendMode, DetailMode, MapAlpha, MapBlend, MapCollisionSet, MapData, MapDetail, MapDressing, MapEntity,
        MapFlatEnvmap, MapFog, MapHull, MapLightmap, MapLoadOptions, MapLoader, MapMesh, MapSkyCamera, MapTexture,
        entities::engine_to_entity,
    },
    mount::Mount,
};

/// The prefix of Combat Arms' map ids.
pub const MAP_PREFIX: &str = "combat_arms:";

/// The game type whose start points become the map's spawn points: the
/// team modes' family (TDM, Elimination Pro, game_modes.md S4); the other
/// modes pick their own family's from the passed-through start points
/// (`rules::spawns`).
pub const SPAWN_GAME_TYPE: &str = "Team Death Match";

/// Object classes whose world models collide where they stand without a
/// `Solid` property ("How common things are expressed": doors, moving
/// geometry, breakables); other world models (volumes such as
/// `EndlessFall`) don't.
pub const SOLID_CLASSES: &[&str] = &[
    "WorldModel",
    "Breakable",
    "RotatingDoor",
    "SlidingDoor",
    "RotatingWorldModel",
    "SlidingWorldModel",
    "SpinningWorldModel",
];

/// The surface types light passes (effects_sounds.md, "The surface
/// table": 40 Glass, 41 Bullet-Proof Glass): their polygons' collider is
/// Chimera's `SeeThrough` (sight passes it, bots.md 5.2).
pub const GLASS_SURFACE_TYPES: &[u16] = &[40, 41];

/// The console's `Saturate` 1 (the default): the lighting multiply is
/// doubled ("Lightmaps", Q7).
pub const SATURATE: f32 = 2.0;

/// Atlas texels between packed lightmaps' borders and its width.
const ATLAS_WIDTH: u32 = 4096;

/// The largest atlas side: the GPU's 2D texture limit (wgpu's default
/// 8192). A map whose lightmaps don't fit is laid out 8192 wide, then
/// with its lightmaps at half resolution until they do (TERRORDOME).
pub const ATLAS_MAX: u32 = 8192;

/// Combat Arms' maps from the install (`map::MapLoader`), for an app's
/// `map::MapSources`.
pub static MAP_LOADER: MapLoader = MapLoader {
    game: super::GAME,
    load: |name, options| load_with(name, options),
};

/// What Combat Arms' gameplay brings to another game's map
/// (`map::MapDressing`, a mashup's `map::MapSources`): `dress` with the
/// install's archives.
pub static DRESSING: MapDressing = MapDressing {
    game: super::GAME,
    dress: |map| {
        let mount = super::INFO.mount().ok_or_else(|| {
            "no Combat Arms install: set [games.combat_arms] path in chimera.local.toml".to_string()
        })?;
        dress(&mount, map);
        Ok(())
    },
};

/// Combat Arms' parts on any map (another game's, in a mashup): the
/// effect looks (`effects::library::add_looks`), the weapons' view models
/// and the character bodies and held models, as `load_from` adds them to
/// its own. The map's surfaces keep their own names (Combat Arms' effects
/// take an unknown surface's).
pub fn dress(mount: &Mount, map: &mut MapData) {
    let file = |path: &str| mount.read(path).ok();
    let data = super::data::CaData::install();
    super::effects::library::add_looks(map, &data, &file);
    super::view_model::add_view_models(map, &data, &file);
    super::characters::models::add_character_models(map, &data, &file);
}

/// Map `id` (`combat_arms:<name>`, or the bare name) from the install.
pub fn load(id: &str) -> Result<MapData, String> {
    load_with(id, &MapLoadOptions::default())
}

/// `load` with `options` (`MapLoadOptions::textures` off: the world's
/// textures aren't decoded; the dedicated server's).
pub fn load_with(id: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let name = map_name(id);
    let mount = mount().ok_or_else(|| {
        "no Combat Arms install: set [games.combat_arms] path in chimera.local.toml".to_string()
    })?;
    load_from_with(&mount, &name, options)
}

/// The install, mounted once per session (its archives' directories read
/// once; `None` when it doesn't open): map loads and on-demand loads
/// (`view_model::on_demand`) share it.
pub fn mount() -> Option<std::sync::Arc<Mount>> {
    static MOUNT: std::sync::OnceLock<Option<std::sync::Arc<Mount>>> = std::sync::OnceLock::new();
    MOUNT.get_or_init(|| super::INFO.mount().map(std::sync::Arc::new)).clone()
}

/// A map id's file name: lower case, without the game prefix.
pub fn map_name(id: &str) -> String {
    id.strip_prefix(MAP_PREFIX).unwrap_or(id).to_lowercase()
}

/// The world file of map `name` in the mounted archives
/// (`worlds*/<name>.dat`, the first archive in name order).
pub fn find(mount: &Mount, name: &str) -> Option<String> {
    let file = format!("/{}.dat", name.to_lowercase());
    let mut paths: Vec<String> = mount
        .entries()
        .into_iter()
        .map(|(e, _)| e.path)
        .filter(|p| p.starts_with("worlds") && p.ends_with(&file))
        .collect();
    paths.sort();
    paths.into_iter().next()
}

/// Every map in the mounted archives, by name.
pub fn map_names(mount: &Mount) -> Vec<String> {
    let mut names: Vec<String> = mount
        .entries()
        .into_iter()
        .filter_map(|(e, _)| {
            let (dir, file) = e.path.rsplit_once('/')?;
            (dir.starts_with("worlds") && !dir.contains('/')).then_some(())?;
            file.strip_suffix(".dat").map(str::to_string)
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Map `name` from a mounted install, with the character models every
/// map carries (the view models load on demand: `view_model::on_demand`).
/// The models (converted once per session, with the weapon data they
/// need) load on a thread of their own beside the world.
pub fn load_from(mount: &Mount, name: &str) -> Result<MapData, String> {
    load_from_with(mount, name, &MapLoadOptions::default())
}

/// `load_from` with `options`.
pub fn load_from_with(mount: &Mount, name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let file = |path: &str| mount.read(path).ok();
    // The character models are the kit (`MapData::kit`): not read for a
    // map whose kit isn't wanted (another game's gameplay on it).
    if !options.kit.parts() && !options.kit.any_models() {
        return load_world_with(mount, name, options);
    }
    let (map, models) = std::thread::scope(|s| {
        let models = s.spawn(|| {
            super::data::CaData::install().weapon_set();
            super::characters::models::shared_character_models(&file)
        });
        let map = load_world_with(mount, name, options);
        (map, models.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
    });
    let mut map = map?;
    super::characters::models::append_shared(&mut map, &models, &options.kit);
    Ok(map)
}

/// Map `name` from a mounted install: the world and its effects only
/// (`load_from` without the character models every map shares).
pub fn load_world(mount: &Mount, name: &str) -> Result<MapData, String> {
    load_world_with(mount, name, &MapLoadOptions::default())
}

/// `load_world` with `options`.
pub fn load_world_with(mount: &Mount, name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let path = find(mount, name).ok_or_else(|| format!("{name}: no such map (worlds*/{name}.dat)"))?;
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let world = World::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let scale = super::movement::movement_data().meters_per_unit;
    let file = |path: &str| mount.read(path).ok();
    let mut map = convert_with(&world, name, scale, &file, options.textures);
    map.warnings.extend(world.warnings.iter().map(|w| format!("{path}: {w}")));
    super::effects::library::add_to_map(&mut map, &super::data::CaData::install(), &world, &file);
    if let Ok(spec) = std::env::var(super::props::DEV_PROP_VAR) {
        super::props::dev_prop(&world, &mut map, &spec, scale, &file);
    }
    Ok(map)
}

/// A world as Chimera's map, `scale` metres per world unit; `file` reads
/// a texture by its normalized path (lower case, `/`), None when missing.
pub fn convert(world: &World, name: &str, scale: f32, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> MapData {
    convert_with(world, name, scale, file, true)
}

/// `convert`; `decode` off leaves the world's textures undecoded
/// (`MapLoadOptions::textures`: placeholders of the right size).
pub fn convert_with(
    world: &World,
    name: &str,
    scale: f32,
    file: &dyn Fn(&str) -> Option<Vec<u8>>,
    decode: bool,
) -> MapData {
    let to_engine = |p: lt::Vec3| Vec3::from(lt::to_right_handed(p)) * scale;
    let mut map = MapData {
        name: format!("{MAP_PREFIX}{name}"),
        ..default()
    };
    // The game's output is clamped, not tonemapped.
    map.look.tonemapping = false;

    let properties = world.objects_of("WorldProperties").map(|(_, o)| o).last();
    let t_factor = properties
        .and_then(|o| colour(o, "LightmapTFactor"))
        .unwrap_or([1.0; 3]);
    let groups = light_group_colours(world);
    let sky = sky_object(world);
    // World models that move, hide or break: a node each (the map's first
    // entities), drawn and collided there.
    let nodes = world_model_nodes(world, &mut map, scale);
    // Each render world drawn: its look, whether it's the sky, its node.
    type Drawn<'a> = (&'a RenderWorld, MapAlpha, bool, Option<(usize, Vec3)>);
    let mut worlds: Vec<Drawn> = vec![(&world.render, MapAlpha::Opaque, false, None)];
    worlds.extend(world.render.models.iter().filter_map(|(n, w)| {
        if sky.and_then(Object::name) == Some(n.as_str()) {
            return Some((w, MapAlpha::Opaque, true, None));
        }
        if let Some(&(i, centre)) = nodes.get(n.as_str()) {
            let o = world.objects.iter().find(|o| o.name() == Some(n.as_str()))?;
            return Some((w, blend_of(o), false, Some((i, centre))));
        }
        Some((w, model_look(world, n)?, false, None))
    }));
    let lut = LightLut::new(t_factor);
    let mut atlas = Atlas::new(lut);
    let mut meshes: HashMap<String, Building> = HashMap::new();
    for (render, alpha, skybox, entity) in &worlds {
        for block in &render.blocks {
            let lit = Lighting { groups: &groups, lut };
            add_block(block, *alpha, *skybox, *entity, &lit, &mut atlas, &mut meshes, &to_engine);
        }
    }
    let lightmap = atlas.finish();
    let mut built: Vec<(MapMesh, Option<String>, String)> = meshes
        .into_iter()
        .map(|(key, mut b)| {
            for (uv, rect) in b.mesh.lightmap_uvs.iter_mut().zip(b.rects) {
                *uv = lightmap.uv(rect, *uv);
            }
            (b.mesh, b.second, key)
        })
        .collect();
    // The mesh key ends the order: meshes of one texture that differ in
    // blend or world model compare equal otherwise, and the map's hash
    // order (random per run) then decided which came first.
    built.sort_by(|(a, a2, ak), (b, b2, bk)| (a.skybox, &a.material, a2, ak).cmp(&(b.skybox, &b.material, b2, bk)));
    let (mut meshes, seconds): (Vec<MapMesh>, Vec<Option<String>>) =
        built.into_iter().map(|(m, s, _)| (m, s)).unzip();
    textures_with(&mut meshes, &mut map, file, decode);
    second_layers(&mut meshes, &seconds, &mut map, file, decode);
    map.meshes = meshes;
    map.lightmap = Some(lightmap.map);

    map.fog = properties.and_then(|o| world_fog(o, scale, false));
    map.sky_camera = sky.and_then(|o| sky_camera(world, o, properties, scale));
    let sky = collision(world, &mut map, &to_engine, &nodes);
    volumes(world, &mut map, scale);
    objects(world, &mut map, scale, |o| {
        super::bots::map_ai::is_ai_object(&o.class)
            || super::rules::objects::is_mode_object(&o.class, o.name())
            || super::objects::is_logic_object(&o.class)
    });
    world_info(world, &mut map, scale);
    spawns(world, &mut map, &sky, &to_engine);
    start_point_feet(world, &mut map, &sky, scale, &to_engine);
    super::props::add_objects(world, &mut map, scale, file);
    if map.spawns.is_empty() {
        map.warnings.push("no GameStartPoint: spawning at the origin".into());
        map.spawns.push((Vec3::ZERO, None));
    }
    map
}

/// How world model `name` is drawn: None when its object is not visible
/// (`Visible` false or `StartHidden` true), else its `BlendMode`
/// (`Additive`, `Translucent`, opaque otherwise).
fn model_look(world: &World, name: &str) -> Option<MapAlpha> {
    let Some(o) = world.objects.iter().find(|o| o.name() == Some(name)) else {
        return Some(MapAlpha::Opaque);
    };
    if o.class == "DemoSkyWorldModel" || flag(o, "Visible") == Some(false) || flag(o, "StartHidden") == Some(true) {
        return None;
    }
    Some(blend_of(o))
}

/// A boolean property as the engine reads it: a bool, or a number that
/// is not zero (SHORTFUSE's `Filter` volumes store `Visible` as the real
/// 0, which a bool-only read took as absent, so their boxes were drawn
/// as flat slabs over the doors).
fn flag(o: &Object, key: &str) -> Option<bool> {
    match o.get(key)? {
        lt::Value::Bool(b) => Some(*b),
        lt::Value::Real(x) | lt::Value::Long(x) | lt::Value::Flags(x) => Some(*x != 0.0),
        _ => None,
    }
}

/// An object's `BlendMode` as drawn (`Additive`, `Translucent`, opaque
/// otherwise).
fn blend_of(o: &Object) -> MapAlpha {
    match o.string("BlendMode") {
        Some(b) if b.eq_ignore_ascii_case("Additive") => MapAlpha::Add,
        Some(b) if b.eq_ignore_ascii_case("Translucent") => MapAlpha::Blend,
        _ => MapAlpha::Opaque,
    }
}

/// The world-model objects that get a node of their own
/// (`objects::is_node_object`: the active classes, and plain
/// `WorldModel`s a keyframer, a command, an attachment or their own
/// properties can move, hide or break), each as a mover map entity
/// (world_objects.md 1): placed at its BSP's centre with no rotation,
/// every property as text (`object_keyvalues`, `Pos` kept as itself:
/// the default pivot), its box's half-extents, whether it collides and
/// stops rays (`model_flags`), and each polygon's corners as a hull
/// about the centre (`objects` collides the node as their triangles). By
/// name: its entity index and centre (engine axes).
fn world_model_nodes(world: &World, map: &mut MapData, scale: f32) -> HashMap<String, (usize, Vec3)> {
    use super::objects::runtime::keys;
    let named = super::objects::names_in_commands(world.objects.iter().flat_map(|o| {
        o.properties.iter().filter(|p| !p.name.eq_ignore_ascii_case("Name")).filter_map(|p| match &p.value {
            lt::Value::String(s) => Some(s.as_str()),
            _ => None,
        })
    }));
    let mut out = HashMap::new();
    for o in &world.objects {
        let Some(name) = o.name() else { continue };
        if out.contains_key(name) || o.class == "DemoSkyWorldModel" || super::volumes::is_volume(&o.class) {
            continue;
        }
        let Some((_, m)) = world.model(name) else { continue };
        if m.is_physics() || !super::objects::is_node_object(o, named.contains(&name.to_ascii_lowercase())) {
            continue;
        }
        let centre = Vec3::from(m.translation);
        let half = (Vec3::from(m.max) - Vec3::from(m.min)) / 2.0;
        let (solid, rayhit) = model_flags(world, name);
        let mut keyvalues = object_keyvalues(o, false);
        let e = |v: Vec3| format!("{} {} {}", v.x, v.z, v.y);
        keyvalues.push(("origin".into(), e(centre)));
        keyvalues.push((keys::HALF.into(), e(half)));
        keyvalues.push((keys::SOLID.into(), (solid as u8).to_string()));
        keyvalues.push((keys::RAYHIT.into(), (rayhit as u8).to_string()));
        // Each polygon as a hull of its corners (about the centre, entity
        // axes): the node collides as their triangles (`objects`).
        let hulls = (0..m.polygons.len())
            .map(|i| MapHull {
                planes: Vec::new(),
                points: m
                    .polygon_vertices(i)
                    .map(|p| {
                        let d = Vec3::from(p) - centre;
                        Vec3::new(d.x, d.z, d.y)
                    })
                    .collect(),
            })
            .filter(|h| h.points.len() >= 3)
            .collect();
        let index = map.entities.len();
        map.entities.push(MapEntity {
            keyvalues,
            hulls,
            mover: true,
            ..default()
        });
        out.insert(name.to_string(), (index, Vec3::from(lt::to_right_handed(centre.into())) * scale));
    }
    if !out.is_empty() {
        map.entity_scale = scale;
    }
    out
}

/// A colour property as 0..1 per channel (stored 0..255).
fn colour(o: &Object, key: &str) -> Option<[f32; 3]> {
    match o.get(key)? {
        lt::Value::Color(c) | lt::Value::Vector(c) => Some(c.map(|x| x / 255.0)),
        _ => None,
    }
}

/// Each light group's colour when the map has loaded (Q12): its
/// `LightGroup` object's `StartColor`/255 if `StartOn` (true unless set),
/// black if not; a group without an object keeps the colour stored with
/// it (the second argument).
pub fn light_group_colours(world: &World) -> impl Fn(&str, [f32; 3]) -> [f32; 3] + '_ {
    move |name: &str, stored: [f32; 3]| {
        let Some(o) = world
            .objects_of("LightGroup")
            .map(|(_, o)| o)
            .find(|o| o.name() == Some(name))
        else {
            return stored;
        };
        if o.bool("StartOn") == Some(false) {
            return [0.0; 3];
        }
        colour(o, "StartColor").unwrap_or([1.0; 3])
    }
}

/// A light group's addition to one channel: min(255, base + ⌊colour × x⌋).
pub fn add_light(base: u8, colour: f32, x: u8) -> u8 {
    (base as u32 + (colour * x as f32).floor().max(0.0) as u32).min(255) as u8
}

/// The light a stored lightmap or vertex-colour byte gives, linear, for a
/// texture multiplied by it: the game's 2 x texture x light x T²
/// (`Saturate`, the `LightmapTFactor` channel T on both passes), the
/// product taken as sRGB.
pub fn light_value(byte: u8, t: f32) -> f32 {
    let c = SATURATE * byte as f32 / 255.0 * t * t;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// `light_value` of every byte, per channel's `LightmapTFactor`: the
/// lightmaps' texels are looked up, not computed (TERRORDOME has some 80
/// million).
#[derive(Clone, Copy)]
struct LightLut([[f32; 256]; 3]);

impl LightLut {
    fn new(t_factor: [f32; 3]) -> Self {
        Self(std::array::from_fn(|k| std::array::from_fn(|b| light_value(b as u8, t_factor[k]))))
    }

    fn get(&self, rgb: [u8; 3]) -> [f32; 3] {
        std::array::from_fn(|k| self.0[k][rgb[k] as usize])
    }
}

/// How light groups and the lightmap factor light a block.
struct Lighting<'a> {
    groups: &'a dyn Fn(&str, [f32; 3]) -> [f32; 3],
    lut: LightLut,
}

impl Lighting<'_> {
    /// Each of the block's light groups with its colour, if on (a colour
    /// that is 0 in all three channels at 8 bits adds nothing).
    fn on<'b>(&self, block: &'b lt::RenderBlock) -> Vec<(&'b lt::BlockLightGroup, [f32; 3])> {
        block
            .light_groups
            .iter()
            .map(|g| (g, (self.groups)(&g.name, g.color)))
            .filter(|(_, c)| c.iter().any(|x| (x * 255.0) as u32 > 0))
            .collect()
    }

    fn texel(&self, rgb: [u8; 3]) -> [f32; 3] {
        self.lut.get(rgb)
    }
}

/// A lightmap rectangle's place in the atlas: index into `Atlas::rects`
/// and a coordinate in it (0..1).
type LightmapRef = (u32, [f32; 2]);

/// A triangle's corner positions (as bits), rotated to start at the
/// smallest: the same key for the same triangle whatever corner it starts
/// at, a different one for the reverse winding (the back face).
type TriangleKey = [[u32; 3]; 3];

/// `t`'s key and the rotation that made it (key corner k is `t`'s corner
/// (k + rotation) mod 3).
fn triangle_key(block: &lt::RenderBlock, t: &lt::Triangle) -> (TriangleKey, usize) {
    let p = t.indices.map(|i| block.vertices[i as usize].pos.map(f32::to_bits));
    let r = (0..3).min_by_key(|&k| p[k]).unwrap_or(0);
    ([p[r], p[(r + 1) % 3], p[(r + 2) % 3]], r)
}

/// A lightmap-pass triangle: its rectangle, its corners' lightmap
/// coordinates in key order, its polygon.
type LightmapTriangle = (u32, [[f32; 2]; 3], u32);

/// A corner of a drawn triangle (file axes).
#[derive(Clone, Copy, Debug)]
struct Corner {
    pos: lt::Vec3,
    normal: lt::Vec3,
    uv: [f32; 2],
    lm: [f32; 2],
    /// Dual-texture sections: UV1 and the second texture's weight (1 −
    /// the vertex alpha).
    uv1: [f32; 2],
    weight: f32,
}

impl Corner {
    fn lerp(a: &Corner, b: &Corner, t: f32) -> Corner {
        let l3 = |x: lt::Vec3, y: lt::Vec3| std::array::from_fn(|k| x[k] + (y[k] - x[k]) * t);
        let l2 = |x: [f32; 2], y: [f32; 2]| std::array::from_fn(|k| x[k] + (y[k] - x[k]) * t);
        Corner {
            pos: l3(a.pos, b.pos),
            normal: l3(a.normal, b.normal),
            uv: l2(a.uv, b.uv),
            lm: l2(a.lm, b.lm),
            uv1: l2(a.uv1, b.uv1),
            weight: a.weight + (b.weight - a.weight) * t,
        }
    }
}

/// A convex polygon cut by the line lm[axis] = at: (the part at or below,
/// the part at or above), each empty when it has nothing.
fn cut(poly: &[Corner], axis: usize, at: f32) -> (Vec<Corner>, Vec<Corner>) {
    let (mut below, mut above) = (Vec::new(), Vec::new());
    for i in 0..poly.len() {
        let (a, b) = (&poly[i], &poly[(i + 1) % poly.len()]);
        let (da, db) = (a.lm[axis] - at, b.lm[axis] - at);
        if da <= 0.0 {
            below.push(*a);
        }
        if da >= 0.0 {
            above.push(*a);
        }
        if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
            let mid = Corner::lerp(a, b, da / (da - db));
            below.push(mid);
            above.push(mid);
        }
    }
    let keep = |p: Vec<Corner>| if p.len() >= 3 { p } else { Vec::new() };
    (keep(below), keep(above))
}

/// A triangle cut where its lightmap coordinates cross 0 and 1, each part
/// clamped to 0..1: interpolating a part's clamped corners gives the
/// clamp of the interpolated coordinate (the sampler's per-pixel clamp).
fn clamp_cut(tri: [Corner; 3]) -> Vec<Vec<Corner>> {
    let mut parts = vec![tri.to_vec()];
    for (axis, at) in [(0, 0.0), (0, 1.0), (1, 0.0), (1, 1.0)] {
        parts = parts
            .into_iter()
            .flat_map(|p| {
                let (a, b) = cut(&p, axis, at);
                [a, b]
            })
            .filter(|p| !p.is_empty())
            .collect();
    }
    for p in &mut parts {
        for c in p.iter_mut() {
            c.lm = c.lm.map(|x| x.clamp(0.0, 1.0));
        }
    }
    parts
}

/// Texture-pass triangle `t`'s lightmap rectangle and coordinate per
/// corner: the lightmap-pass triangle with the same corners (the one of
/// the same polygon when there are several, else the next in turn); None
/// for every corner when there is none.
fn lightmap_for(
    lightmap_at: &HashMap<TriangleKey, Vec<LightmapTriangle>>,
    taken: &mut HashMap<TriangleKey, usize>,
    block: &lt::RenderBlock,
    t: &lt::Triangle,
) -> [Option<LightmapRef>; 3] {
    let (key, r) = triangle_key(block, t);
    let Some(candidates) = lightmap_at.get(&key) else { return [None; 3] };
    let (rect, uvs, _) = match candidates.iter().find(|c| c.2 == t.polygon) {
        Some(c) => *c,
        None => {
            let n = taken.entry(key).or_default();
            *n += 1;
            candidates[(*n - 1) % candidates.len()]
        }
    };
    // Key corner k is t's corner (k + r) mod 3.
    std::array::from_fn(|k| Some((rect, uvs[(k + 3 - r) % 3])))
}

/// One render block's drawn sections into the meshes (by texture; each
/// with its vertices' lightmap rectangles, whose coordinates the mesh's
/// `lightmap_uvs` hold until the atlas is laid out).
#[allow(clippy::too_many_arguments)]
fn add_block(
    block: &lt::RenderBlock,
    alpha: MapAlpha,
    skybox: bool,
    entity: Option<(usize, Vec3)>,
    lit: &Lighting,
    atlas: &mut Atlas,
    meshes: &mut HashMap<String, Building>,
    to_engine: &impl Fn(lt::Vec3) -> Vec3,
) {
    let groups = lit.on(block);
    // Vertex colours with the light groups that are on.
    let colours: Vec<[u8; 3]> = block
        .vertices
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let [r, g, b, _] = v.rgba();
            let mut c = [r, g, b];
            for (group, colour) in &groups {
                let x = group.intensities.get(i).copied().unwrap_or(0);
                c = std::array::from_fn(|k| add_light(c[k], colour[k], x));
            }
            c
        })
        .collect();
    // The lightmap pass's triangles, by their corners: the engine draws
    // the pass over the texture pass with an equal-depth test, so a
    // texture triangle takes the light of the lightmap triangle at the
    // same place. Not by polygon: the two passes' triangles of one surface
    // often carry different polygon indices (JUNK_FLEA2: 9 in 10), and
    // another polygon at a shared corner then lent its lightmap.
    let mut lightmap_at: HashMap<TriangleKey, Vec<LightmapTriangle>> = HashMap::new();
    for (si, (s, tris)) in block.section_triangles().enumerate() {
        if s.shader != shader::LIGHTMAP || s.lightmap.is_empty() {
            continue;
        }
        let mut rgb = s.lightmap.clone();
        let w = s.lightmap_width;
        for (group, colour) in &groups {
            for sub in group.sections.get(si).into_iter().flatten() {
                for y in 0..sub.height {
                    for x in 0..sub.width {
                        let (tx, ty) = (sub.left + x, sub.top + y);
                        let Some(&i) = sub.intensities.get((y * sub.width + x) as usize) else { continue };
                        if tx >= w || ty >= s.lightmap_height {
                            continue;
                        }
                        let at = ((ty * w + tx) * 3) as usize;
                        for k in 0..3 {
                            rgb[at + k] = add_light(rgb[at + k], colour[k], i);
                        }
                    }
                }
            }
        }
        let texels = Texels::Bytes(rgb.as_chunks::<3>().0.to_vec());
        let rect = atlas.add_texels(s.lightmap_width, s.lightmap_height, texels);
        for t in tris {
            let (key, r) = triangle_key(block, t);
            let uvs = std::array::from_fn(|k| block.vertices[t.indices[(k + r) % 3] as usize].uv0);
            lightmap_at.entry(key).or_default().push((rect, uvs, t.polygon));
        }
    }
    // Texture triangles matched so far per key: coincident surfaces (the
    // same triangle twice) take the lightmap triangles in turn.
    let mut taken: HashMap<TriangleKey, usize> = HashMap::new();
    for (s, tris) in block.section_triangles() {
        let lightmapped = match s.shader {
            shader::LIGHTMAP_TEXTURE | shader::LIGHTMAP_DUAL_TEXTURE => true,
            shader::GOURAUD | shader::DUAL_TEXTURE => false,
            // The lightmap pass, sky portals, occluders, and codes the
            // engine treats as invalid: not drawn.
            _ => continue,
        };
        let name = s.textures[0].to_lowercase();
        // Dual-texture sections (8, 9) draw their second texture at UV1
        // ("Second textures"); a mesh per pair.
        let dual = matches!(s.shader, shader::DUAL_TEXTURE | shader::LIGHTMAP_DUAL_TEXTURE);
        let second = (dual && !s.textures[1].is_empty()).then(|| s.textures[1].clone());
        let key = format!(
            "{name}|{}|{alpha:?}|{skybox}|{:?}",
            second.as_deref().unwrap_or("").to_lowercase(),
            entity.map(|(i, _)| i)
        );
        let origin = entity.map_or(Vec3::ZERO, |(_, o)| o);
        let Building { mesh, rects, .. } = meshes.entry(key).or_insert_with(|| Building {
            mesh: MapMesh {
                material: s.textures[0].clone(),
                color: flat_color(&name),
                alpha,
                skybox,
                entity: entity.map(|(i, _)| i),
                ..default()
            },
            rects: Vec::new(),
            second: second.clone(),
        });
        let has_second = second.is_some();
        let push = |mesh: &mut MapMesh, rects: &mut Vec<u32>, c: &Corner, rect: u32| {
            mesh.positions.push((to_engine(c.pos) - origin).to_array());
            mesh.normals.push(Vec3::from(lt::to_right_handed(c.normal)).normalize_or_zero().to_array());
            mesh.uvs.push(c.uv);
            mesh.lightmap_uvs.push(c.lm);
            if has_second {
                mesh.uvs2.push(c.uv1);
                mesh.blend_weights.push(c.weight);
            }
            rects.push(rect);
            (mesh.positions.len() - 1) as u32
        };
        // Drawn vertices by block vertex and lightmap place.
        let mut shared: HashMap<(u32, u32, [u32; 2]), u32> = HashMap::new();
        for t in tris {
            let corners = t.indices.map(|i| {
                let v = &block.vertices[i as usize];
                Corner {
                    pos: v.pos,
                    normal: v.normal,
                    uv: v.uv0,
                    lm: [0.5, 0.5],
                    uv1: v.uv1,
                    weight: 1.0 - v.rgba()[3] as f32 / 255.0,
                }
            });
            if !lightmapped {
                // Vertex lighting: a 2 x 2 rectangle whose bilinear blend
                // over the triangle is the corners' linear one.
                let [l0, l1, l2] = t.indices.map(|i| Vec3::from(lit.texel(colours[i as usize])));
                let (rect, lms) = if l0 == l1 && l1 == l2 {
                    (atlas.add_flat(l0.to_array()), [[0.5, 0.5]; 3])
                } else {
                    let l3 = l1 + l2 - l0;
                    let rect = atlas.add(2, 2, vec![l0.to_array(), l1.to_array(), l2.to_array(), l3.to_array()]);
                    (rect, [[0.25, 0.25], [0.75, 0.25], [0.25, 0.75]])
                };
                let mut tri = [0u32; 3];
                for k in 0..3 {
                    let c = Corner { lm: lms[k], ..corners[k] };
                    tri[k] = push(mesh, rects, &c, rect);
                }
                // Mirroring reverses the winding: swap two corners.
                mesh.indices.extend([tri[0], tri[2], tri[1]]);
                continue;
            }
            let found = lightmap_for(&lightmap_at, &mut taken, block, t);
            let rect = match found {
                [Some(a), Some(b), Some(c)] if a.0 == b.0 && b.0 == c.0 => Some(a.0),
                _ => None,
            };
            let outside = found.iter().flatten().any(|(_, uv)| uv.iter().any(|x| !(0.0..=1.0).contains(x)));
            if let (Some(rect), true) = (rect, outside) {
                // Cut where the coordinates leave 0..1 (the per-pixel
                // clamp).
                let corners: [Corner; 3] = std::array::from_fn(|k| Corner {
                    lm: found[k].unwrap().1,
                    ..corners[k]
                });
                for part in clamp_cut(corners) {
                    let ids: Vec<u32> = part.iter().map(|c| push(mesh, rects, c, rect)).collect();
                    for k in 1..ids.len() - 1 {
                        mesh.indices.extend([ids[0], ids[k + 1], ids[k]]);
                    }
                }
                continue;
            }
            let mut tri = [0u32; 3];
            for (k, &i) in t.indices.iter().enumerate() {
                let (rect, lm) = found[k].unwrap_or_else(|| (atlas.add_flat([1.0; 3]), [0.5, 0.5]));
                let lm = lm.map(|x| x.clamp(0.0, 1.0));
                // Shared only with the same lightmap place: one polygon's
                // triangles can have theirs on different pages.
                tri[k] = *shared.entry((i, rect, lm.map(f32::to_bits))).or_insert_with(|| {
                    let c = Corner { lm, ..corners[k] };
                    push(mesh, rects, &c, rect)
                });
            }
            // Mirroring reverses the winding: swap two corners.
            mesh.indices.extend([tri[0], tri[2], tri[1]]);
        }
    }
}

/// Each mesh's texture from the install (`.dtx`, the level the shipped
/// "DetailLevel" loads for its group; an alpha-tested one cut out at its
/// `alpharef`); a name that doesn't resolve keeps the mesh's flat colour,
/// with a warning. And as model skins
/// (specs/combat_arms/models_ltb.md "Sprite skins" and "Stock styles"):
/// a sprite (`.spr`) skin's frames, each its texture, on the drawn copy's
/// own clock (`MapMesh::skin_frames`, ⌊1000 / rate⌋ ms a frame; a frame
/// that doesn't resolve is left out, and a sprite that doesn't read draws
/// its first frame, as map surfaces do), and the slot-1 texture of a
/// piece's environment or rim map (`MapMesh::layer`; one that doesn't
/// resolve leaves the layer undrawn).
pub(crate) fn model_textures(meshes: &mut [MapMesh], map: &mut MapData, file: &dyn Fn(&str) -> Option<Vec<u8>>) {
    let norm = |p: &str| p.trim().replace('\\', "/").to_lowercase();
    // Each sprite skin's frames and frame time, by mesh.
    let sprites: Vec<Option<(Vec<String>, u32)>> = meshes
        .iter()
        .map(|m| {
            let name = norm(&m.material);
            if !name.ends_with(".spr") {
                return None;
            }
            let s = lithtech_formats::spr::Sprite::parse(&file(&name)?).ok()?;
            Some((s.frames.iter().map(|f| norm(f)).collect(), s.frame_ms()))
        })
        .collect();
    // The frames and layers as meshes of their own, resolved with the
    // rest in one pass (each file once).
    let mut extra: Vec<MapMesh> = Vec::new();
    let mut frame_at: Vec<Option<std::ops::Range<usize>>> = Vec::with_capacity(meshes.len());
    let mut layer_at: Vec<Option<usize>> = Vec::with_capacity(meshes.len());
    let mut names: Vec<String> = Vec::with_capacity(meshes.len());
    for (m, sprite) in meshes.iter_mut().zip(&sprites) {
        let start = extra.len();
        match sprite {
            Some((frames, _)) => {
                extra.extend(frames.iter().map(|f| MapMesh {
                    material: f.clone(),
                    alpha: m.alpha,
                    ..default()
                }));
                frame_at.push(Some(start..extra.len()));
                // Drawn from its frames: resolved there, not here.
                names.push(std::mem::take(&mut m.material));
            }
            None => {
                frame_at.push(None);
                names.push(String::new());
            }
        }
        layer_at.push(m.layer.as_ref().filter(|l| !l.material.is_empty()).map(|l| {
            extra.push(MapMesh {
                material: l.material.clone(),
                ..default()
            });
            extra.len() - 1
        }));
    }
    let base = meshes.len();
    let mut all: Vec<MapMesh> = meshes.iter().cloned().chain(extra).collect();
    textures_with(&mut all, map, file, true);
    let (own, extra) = all.split_at(base);
    for (k, m) in meshes.iter_mut().enumerate() {
        *m = own[k].clone();
        if let (Some(range), Some((_, frame_ms))) = (frame_at[k].clone(), &sprites[k]) {
            m.material = std::mem::take(&mut names[k]);
            let resolved: Vec<&MapMesh> = extra[range].iter().filter(|f| f.texture.is_some()).collect();
            if let Some(first) = resolved.first() {
                m.texture = first.texture;
                m.color = [255; 3];
                if m.alpha == MapAlpha::Opaque {
                    m.alpha = first.alpha;
                }
            }
            if resolved.len() > 1 {
                m.skin_frames = Some(crate::map::MapSkinFrames {
                    frames: resolved.iter().filter_map(|f| f.texture).collect(),
                    frame_ms: (*frame_ms).max(1),
                });
            }
        }
        if let (Some(i), Some(layer)) = (layer_at[k], m.layer.as_mut()) {
            layer.texture = extra[i].texture;
        }
    }
}

/// Resolve each mesh's texture (as `model_textures` says; a sprite: its
/// first frame); `decode` off keeps each texture's size only (no texels,
/// no mip levels: `MapLoadOptions::textures`).
pub(crate) fn textures_with(
    meshes: &mut [MapMesh],
    map: &mut MapData,
    file: &dyn Fn(&str) -> Option<Vec<u8>>,
    decode: bool,
) {
    let mut missing = std::collections::BTreeMap::<String, String>::new();
    // Each file once (meshes differ by blend and sky too), read here and
    // decoded on threads (`decode_all`).
    let mut names: Vec<(String, String)> = Vec::new();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut index: HashMap<String, Option<usize>> = HashMap::new();
    for m in meshes.iter() {
        if m.material.is_empty() || m.material.eq_ignore_ascii_case("default") {
            continue;
        }
        let name = m.material.replace('\\', "/").to_lowercase();
        if index.contains_key(&name) {
            continue;
        }
        let mut path = name.clone();
        // A sprite (world_dat.md: a name ending in `.spr`) draws its first
        // frame's texture.
        if path.ends_with(".spr")
            && let Some(first) = file(&path).and_then(|b| super::effects::library::sprite_texture(&b))
        {
            path = first;
        }
        match file(&path) {
            Some(bytes) => {
                index.insert(name, Some(files.len()));
                names.push((m.material.clone(), path.clone()));
                files.push((path, bytes));
            }
            None => {
                index.insert(name, None);
                missing.insert(path, "missing or locked".into());
            }
        }
    }
    let decoded = decode_all(files, decode);
    let mut slots: Vec<Option<(usize, Option<u8>)>> = Vec::with_capacity(decoded.len());
    for ((material, path), d) in names.into_iter().zip(decoded) {
        match d {
            Ok((texture, alpha_ref)) => {
                map.textures.push(MapTexture { name: material, ..texture });
                slots.push(Some((map.textures.len() - 1, alpha_ref)));
            }
            Err(e) => {
                missing.insert(path, format!("unreadable: {e}"));
                slots.push(None);
            }
        }
    }
    for m in meshes {
        let name = m.material.replace('\\', "/").to_lowercase();
        let Some((t, alpha_ref)) = index.get(&name).copied().flatten().and_then(|i| slots[i]) else { continue };
        if let (Some(r), MapAlpha::Opaque) = (alpha_ref, m.alpha) {
            m.alpha = MapAlpha::Mask(r as f32 / 255.0);
        }
        m.texture = Some(t);
        m.color = [255; 3];
    }
    if !missing.is_empty() {
        let names: Vec<String> = missing.iter().take(8).map(|(p, why)| format!("{p} ({why})")).collect();
        map.warnings.push(format!(
            "{} textures missing or unreadable (keyed archives need the V2 key), flat colours: {}",
            missing.len(),
            names.join(", ")
        ));
    }
}

/// `.dtx` files (path, bytes) decoded at the level the shipped
/// "DetailLevel" loads, with their box-filtered mip chains
/// (`MapTexture::box_mips`) and `alpharef`, on up to 8 threads (in
/// order).
/// With `decode` off only the sizes (and `alpharef`) are read.
fn decode_all(files: Vec<(String, Vec<u8>)>, decode: bool) -> Vec<Result<(MapTexture, Option<u8>), String>> {
    crate::map::parallel::par_map(files, |(_, bytes)| -> Result<(MapTexture, Option<u8>), String> {
        let dtx = Dtx::parse(bytes).map_err(|e| e.to_string())?;
        let level = dtx.skipped_levels(DETAIL_LEVEL, 0);
        let (width, height) = dtx.level_dims(level);
        let mut texture = MapTexture {
            name: String::new(),
            srgb: true,
            mips: Vec::new(),
            width,
            height,
            rgba8: if decode { dtx.rgba(level) } else { Vec::new() },
            bc: if decode { dtx_bc(&dtx, level) } else { None },
        };
        // The mip chain here, on these threads, not on the main thread when
        // the map spawns (the drawing's own box filter).
        if decode {
            texture.mips = texture.box_mips();
        }
        Ok((texture, dtx.alpha_ref()))
    })
}

/// The file's blocks from `level` down (`MapTexture::bc`): DXT1/3/5 as
/// stored, its own mip levels (the files stop at 4 x 4). None for BGRA
/// ones and for sizes that are not whole blocks.
pub(crate) fn dtx_bc(dtx: &Dtx, level: u32) -> Option<crate::map::MapBc> {
    use crate::map::{MapBc, MapBcFormat};
    use lithtech_formats::dtx::Format;

    let format = match dtx.format {
        Format::Dxt1 => MapBcFormat::Bc1,
        Format::Dxt3 => MapBcFormat::Bc2,
        Format::Dxt5 => MapBcFormat::Bc3,
        Format::Bgra32 => return None,
    };
    let levels = (level..dtx.mip_count)
        .map(|i| {
            let (w, h) = dtx.level_dims(i);
            let bytes = dtx.level_bytes(i);
            (w % 4 == 0 && h % 4 == 0 && bytes.len() == MapBc::level_bytes(format, w, h)).then(|| bytes.to_vec())
        })
        .collect::<Option<Vec<_>>>()?;
    Some(MapBc { format, levels })
}

/// A mesh being built from the sections: its vertices' lightmap
/// rectangles (`add_block`) and its second texture's name (dual-texture
/// sections).
struct Building {
    mesh: MapMesh,
    rects: Vec<u32>,
    second: Option<String>,
}

/// The console values the second layers follow, at their defaults
/// (world_dat.md "Second textures", textures_dtx.md "Detail textures"):
/// `DetailTextureScale` 1 and `EnvScale`, which the specs don't give
/// (taken as 1).
const DETAIL_TEXTURE_SCALE: f32 = 1.0;
const ENV_SCALE: f32 = 1.0;

/// The world meshes' second layers ("Second textures"): a dual-texture
/// mesh's texture 1 (`seconds`, by mesh) blended by the vertex alpha
/// (`BlendMode::BaseAlpha`, or `BaseAlphaAdd` when texture 1 is
/// fullbright), and the base texture's linked texture: a detail texture
/// (`detailtex`: the signed add at the base texture's detail scale and
/// rotation) or a flat environment map (`envmap`, `envmapalpha`: signed
/// add, weighted by the base alpha for the latter). Links that don't
/// resolve leave the surface without (textures_dtx.md case 22); the other
/// link kinds don't occur in the install. Each linked texture loads once.
pub(crate) fn second_layers(
    meshes: &mut [MapMesh],
    seconds: &[Option<String>],
    map: &mut MapData,
    file: &dyn Fn(&str) -> Option<Vec<u8>>,
    decode: bool,
) {
    /// A loaded texture: its index, and whether it is fullbright.
    #[derive(Clone)]
    struct Loaded {
        index: usize,
        fullbright: bool,
    }
    let mut loaded: HashMap<String, Option<Loaded>> = HashMap::new();
    let mut load = |map: &mut MapData, name: &str| -> Option<Loaded> {
        let path = link_path(name);
        loaded
            .entry(path.clone())
            .or_insert_with(|| {
                let dtx = Dtx::parse(file(&path)?).ok()?;
                let level = dtx.skipped_levels(DETAIL_LEVEL, 0);
                let (width, height) = dtx.level_dims(level);
                map.textures.push(MapTexture {
                    name: name.to_string(),
                    srgb: true,
                    mips: Vec::new(),
                    width,
                    height,
                    rgba8: if decode { dtx.rgba(level) } else { Vec::new() },
                    bc: None,
                });
                Some(Loaded {
                    index: map.textures.len() - 1,
                    fullbright: dtx.flags & FLAG_FULLBRIGHT != 0,
                })
            })
            .clone()
    };
    let mut bases: HashMap<String, Option<(lithtech_formats::dtx::Link, f32, i16)>> = HashMap::new();
    for (m, second) in meshes.iter_mut().zip(seconds) {
        if let Some(second) = second
            && let Some(t) = load(map, second)
        {
            m.blend = Some(MapBlend {
                texture: Some(t.index),
                mode: if t.fullbright {
                    BlendMode::BaseAlphaAdd
                } else {
                    BlendMode::BaseAlpha
                },
                ..default()
            });
        } else {
            // Without its second texture (or one that doesn't resolve) a
            // dual-texture mesh draws texture 0 alone.
            m.uvs2.clear();
            m.blend_weights.clear();
        }
        if m.texture.is_none() {
            continue;
        }
        // The base texture's link and detail fields (its pixels are
        // `textures_with`').
        let base = bases.entry(link_path(&m.material)).or_insert_with_key(|path| {
            let d = Dtx::parse(file(path)?).ok()?;
            Some((d.link()?, d.detail_scale, d.detail_rotation))
        });
        let Some((link, detail_scale, detail_rotation)) = base.clone() else { continue };
        match link.kind {
            LinkKind::DetailTex => {
                let Some(texture) = load(map, &link.names[0]).map(|t| t.index) else { continue };
                let scale = detail_scale * DETAIL_TEXTURE_SCALE;
                m.detail = Some(MapDetail {
                    texture,
                    scale: [scale; 2],
                    factor: 1.0,
                    mode: DetailMode::SignedAdd,
                    tint: [1.0; 3],
                    rotation: (detail_rotation as f32).to_radians(),
                });
            }
            LinkKind::EnvMap | LinkKind::EnvMapAlpha => {
                let Some(texture) = load(map, &link.names[0]).map(|t| t.index) else { continue };
                m.flat_envmap = Some(MapFlatEnvmap {
                    texture,
                    scale: ENV_SCALE,
                    signed_add: true,
                    by_base_alpha: link.kind == LinkKind::EnvMapAlpha,
                });
            }
            _ => {}
        }
    }
}

/// A muted colour of its own per texture name.
fn flat_color(name: &str) -> [u8; 3] {
    let h = lt::name_hash(name.as_bytes());
    let hue = (h % 360) as f32;
    let c = Color::hsl(hue, 0.18, 0.62).to_srgba();
    [c.red, c.green, c.blue].map(|x| (x * 255.0).round() as u8)
}

/// The sky object: the last `DemoSkyWorldModel` (they all take sky slot
/// 0, so the last one made is the sky).
fn sky_object(world: &World) -> Option<&Object> {
    world.objects_of("DemoSkyWorldModel").map(|(_, o)| o).last()
}

/// The sky camera (Q11): its position follows the player's through the
/// sky object's inner box V (position ± `SkyDims` × `InnerPercent`) as the
/// player moves through the world's extents E, c = V_min + (V_max −
/// V_min) ⊙ (p − E_min) / (E_max − E_min); Chimera's sky camera is
/// `origin + p / scale`, one scale for all axes (their mean). Its fog is
/// the world's with the sky's distances.
fn sky_camera(world: &World, sky: &Object, properties: Option<&Object>, scale: f32) -> Option<MapSkyCamera> {
    let pos = Vec3::from(sky.pos()?);
    let dims = sky.vector("SkyDims").map_or(Vec3::ZERO, Vec3::from);
    let inner = Vec3::new(
        sky.real("InnerPercentX").unwrap_or(0.0),
        sky.real("InnerPercentY").unwrap_or(0.0),
        sky.real("InnerPercentZ").unwrap_or(0.0),
    );
    let half_v = dims * inner;
    let (e_min, e_max) = (Vec3::from(world.extents_min), Vec3::from(world.extents_max));
    let ratios = (e_max - e_min) / (2.0 * half_v).max(Vec3::splat(1e-6));
    let sky_scale = ((ratios.x + ratios.y + ratios.z) / 3.0).min(1e6);
    let engine = |p: Vec3| Vec3::from(lt::to_right_handed(p.to_array())) * scale;
    let e_centre = (e_min + e_max) / 2.0;
    let fog = properties.and_then(|o| world_fog(o, scale, true)).map(|f| MapFog {
        // `material_fog` divides by the scale: the sky's own distances.
        start: f.start * sky_scale,
        end: f.end * sky_scale,
        ..f
    });
    Some(MapSkyCamera {
        origin: engine(pos) - engine(e_centre) / sky_scale,
        scale: sky_scale,
        fog,
    })
}

/// The world properties' fog (`FogEnable`, true unless set; `FogColor`
/// (127, 127, 127); `FogNearZ` 1, `FogFarZ` 5000 u, or for the sky
/// `SkyFogNearZ` 100, `SkyFogFarZ` 1000), linear in depth.
fn world_fog(o: &Object, scale: f32, sky: bool) -> Option<MapFog> {
    if o.bool("FogEnable") == Some(false) {
        return None;
    }
    let (near, far) = if sky {
        (
            o.real("SkyFogNearZ").unwrap_or(100.0),
            o.real("SkyFogFarZ").unwrap_or(1000.0),
        )
    } else {
        (o.real("FogNearZ").unwrap_or(1.0), o.real("FogFarZ").unwrap_or(5000.0))
    };
    Some(MapFog {
        color: colour(o, "FogColor").unwrap_or([127.0 / 255.0; 3]),
        start: near * scale,
        end: far * scale,
        max_density: 1.0,
        linear: true,
    })
}

/// Whether the object named `name` makes its world model solid, and
/// whether rays hit it (Q14): `Solid` (without it, `SOLID_CLASSES`) and
/// `Rayhit` (false without it).
fn model_flags(world: &World, name: &str) -> (bool, bool) {
    let Some(o) = world.objects.iter().find(|o| o.name() == Some(name)) else {
        return (false, false);
    };
    let solid = o.bool("Solid").unwrap_or_else(|| SOLID_CLASSES.contains(&o.class.as_str()));
    (solid, o.bool("Rayhit").unwrap_or(false))
}

/// The collision (Q5, Q14): solid polygons of the physics BSP and of
/// solid objects' world models for everything; their other polygons and
/// ray-hit-only world models for traces only; the collision blockers for
/// players only. Returns the solid sky-portal triangles (the world's outer
/// box among them), which are no floor to drop a spawn on.
fn collision(
    world: &World,
    map: &mut MapData,
    to_engine: &impl Fn(lt::Vec3) -> Vec3,
    nodes: &HashMap<String, (usize, Vec3)>,
) -> Vec<[u32; 3]> {
    let positions = &mut map.collision_positions;
    let mut fan = |points: &mut dyn Iterator<Item = lt::Vec3>, out: &mut Vec<[u32; 3]>| {
        let base = positions.len() as u32;
        let corners: Vec<Vec3> = points.map(to_engine).collect();
        for k in 1..corners.len().saturating_sub(1) {
            // Mirrored: the reversed order keeps the front face.
            let [a, b, c] = [0, k + 1, k];
            if (corners[b] - corners[a]).cross(corners[c] - corners[a]).length_squared() > 1e-12 {
                out.push([base + a as u32, base + b as u32, base + c as u32]);
            }
        }
        positions.extend(corners.iter().map(|p| p.to_array()));
    };
    let (mut solid, mut traces, mut blockers) = (Vec::new(), Vec::new(), Vec::new());
    let (mut glass, mut glass_traces) = (Vec::new(), Vec::new());
    let mut sky = Vec::new();
    // Nodes' world models collide by their nodes; where they stand at
    // load they are floors for dropping spawns only.
    let (mut node_floors, mut node_surfaces) = (Vec::new(), Vec::new());
    for m in &world.models {
        let node = nodes.contains_key(&m.name);
        let (is_solid, rayhit) = if m.is_physics() { (true, true) } else { model_flags(world, &m.name) };
        if !(is_solid || rayhit) {
            continue;
        }
        for i in 0..m.polygons.len() {
            let surface = m.polygon_surface(i);
            let see_through = GLASS_SURFACE_TYPES.contains(&surface.surface_type);
            let solid_face = is_solid && surface.flags & surface_flags::SOLID != 0;
            let out = match (solid_face, see_through) {
                _ if node && solid_face => &mut node_floors,
                _ if node => &mut node_surfaces,
                (true, false) => &mut solid,
                (true, true) => &mut glass,
                (false, false) => &mut traces,
                (false, true) => &mut glass_traces,
            };
            let before = out.len();
            fan(&mut m.polygon_vertices(i), out);
            if !node && is_solid && surface.flags & (surface_flags::SOLID | surface_flags::SKY_PORTAL) == (surface_flags::SOLID | surface_flags::SKY_PORTAL) {
                sky.extend_from_slice(&out[before..]);
            }
        }
    }
    for p in &world.blockers.polygons {
        fan(&mut p.vertices.iter().copied(), &mut blockers);
    }
    map.collision_indices = solid;
    map.collision_sets = vec![
        MapCollisionSet {
            name: "collision blockers".into(),
            indices: blockers,
            characters: true,
            traces: false,
            ..default()
        },
        MapCollisionSet {
            name: "non-solid detail".into(),
            indices: traces,
            characters: false,
            traces: true,
            ..default()
        },
        // Glass: solid as the rest, but sight passes it (bots.md 5.2).
        MapCollisionSet {
            name: "glass".into(),
            indices: glass,
            characters: true,
            traces: true,
            see_through: true,
        },
        MapCollisionSet {
            name: "non-solid glass".into(),
            indices: glass_traces,
            characters: false,
            traces: true,
            see_through: true,
        },
        // Neither: no collider (the nodes have theirs), floors for spawns.
        MapCollisionSet {
            name: NODE_FLOORS.into(),
            indices: node_floors,
            ..default()
        },
        MapCollisionSet {
            name: NODE_SURFACES.into(),
            indices: node_surfaces,
            ..default()
        },
    ];
    sky
}

/// The collision sets of the nodes' world models where they stand at load
/// (solid faces, other faces): no collider, only floors for dropping
/// spawns (`Floor`), as they were before they moved.
pub const NODE_FLOORS: &str = "world models at load (solid)";
pub const NODE_SURFACES: &str = "world models at load (ray-hit)";

/// The volume objects (`volumes::is_volume`) as map entities: their class
/// and properties (engine axes: `Current` with Z mirrored; a jump
/// volume's `Velocity` is a speed, and `JumpDirection` the forward vector
/// of its `Rotation`; `Hidden` as the map's start-up
/// commands leave it: `StartupCommand`'s `msg <name> off`) and their
/// world model's faces as one hull (planes facing out), in entity space.
fn volumes(world: &World, map: &mut MapData, scale: f32) {
    let mut any = false;
    for o in &world.objects {
        if !super::volumes::is_volume(&o.class) {
            continue;
        }
        let Some((_, m)) = o.name().and_then(|n| world.model(n)) else { continue };
        let points: Vec<Vec3> = (0..m.polygons.len())
            .flat_map(|i| m.polygon_vertices(i).collect::<Vec<_>>())
            .map(|p| Vec3::from(lt::to_right_handed(p)) * scale)
            .collect();
        if points.is_empty() {
            continue;
        }
        let centre = points.iter().sum::<Vec3>() / points.len() as f32;
        let planes = (0..m.polygons.len())
            .map(|i| {
                let p = m.polygon_plane(i);
                let n = Vec3::from(lt::to_right_handed(p.normal));
                let d = p.dist * scale;
                // Facing out of the volume.
                if n.dot(centre) > d { (-n, -d) } else { (n, d) }
            })
            .map(|(n, d)| (engine_to_entity(n, 1.0), d / scale))
            .collect();
        let mut keyvalues = vec![("classname".to_string(), o.class.clone())];
        if let Some(n) = o.name() {
            keyvalues.push(("targetname".into(), n.into()));
        }
        for key in ["Viscosity", "Damage", "PendingSec", "Friction", "PlayerSpeedFactor", "PlayerGravity", "Velocity"] {
            if let Some(v) = o.real(key) {
                keyvalues.push((key.into(), v.to_string()));
            }
        }
        for key in ["Current"] {
            if let Some(c) = o.vector(key) {
                let [x, y, z] = lt::to_right_handed(c);
                keyvalues.push((key.into(), format!("{x} {y} {z}")));
            }
        }
        // A jump pad's launch direction: its `Rotation`'s forward vector
        // (engine axes, as `Current`).
        if o.class == "JumpVolume" {
            let [pitch, yaw, roll] = o.rotation("Rotation").unwrap_or([0.0; 3]);
            let [x, y, z] = lt::to_right_handed(lt::rotation_axes(pitch, yaw, roll)[2]);
            keyvalues.push(("JumpDirection".into(), format!("{x} {y} {z}")));
        }
        if let Some(h) = o.bool("Hidden") {
            keyvalues.push(("Hidden".into(), (h as u8).to_string()));
        }
        map.entities.push(MapEntity {
            keyvalues,
            hulls: vec![MapHull {
                planes,
                points: points.iter().map(|p| engine_to_entity(*p, scale)).collect(),
            }],
            ..default()
        });
        any = true;
    }
    if any {
        map.entity_scale = scale;
    }
}

/// The world file's offset (editor = file + offset), passed through for
/// the radar's map data (radar_minimap.md R5, F3; `radar::WORLD_INFO`).
fn world_info(world: &World, map: &mut MapData, scale: f32) {
    let o = world.world_offset;
    map.entities.push(MapEntity {
        keyvalues: vec![
            ("classname".to_string(), super::radar::WORLD_INFO.to_string()),
            ("WorldOffsetX".to_string(), o[0].to_string()),
            ("WorldOffsetZ".to_string(), o[2].to_string()),
        ],
        ..default()
    });
    map.entity_scale = scale;
}

/// The objects whose class `wanted` names, passed through as map entities
/// for the parts that read them (the bots' AI volumes and nodes,
/// `bots::map_ai`): `classname` the class, `targetname` its `Name`,
/// `origin` its `Pos`, and every other property by its name as text:
/// vectors in entity axes (x, z, y of the file's: positions and sizes
/// alike), colours "r g b", numbers as numbers, booleans 1 or 0;
/// rotations and unknown values left out.
fn objects(world: &World, map: &mut MapData, scale: f32, wanted: fn(&lt::Object) -> bool) {
    let mut any = false;
    for o in world.objects.iter().filter(|o| wanted(o)) {
        let mut keyvalues = object_keyvalues(o, true);
        // A keyframer's keys: its blind-data chunk (world_objects.md 9).
        if o.class == "KeyFramer"
            && let Some(i) = o.real("KeyDataIndex").filter(|i| *i >= 0.0)
            && let Some(c) = world.blind_data.get(i as usize)
            && c.id == super::objects::keyframer::KEY_CHUNK_ID
        {
            let hex: String = c.data.iter().map(|b| format!("{b:02x}")).collect();
            keyvalues.push((super::objects::runtime::keys::KEYS.into(), hex));
        }
        map.entities.push(MapEntity { keyvalues, ..default() });
        any = true;
    }
    if any {
        map.entity_scale = scale;
    }
}

/// An object's properties as map-entity keyvalues: `classname` the class,
/// `targetname` its `Name`, `origin` its `Pos` (with `pos_as_origin`;
/// else `Pos` itself), and every other property by its name as text:
/// vectors in entity axes (x, z, y of the file's), colours "r g b",
/// numbers as numbers, booleans 1 or 0, rotations "pitch yaw roll"
/// (radians, the file's); unknown values left out.
fn object_keyvalues(o: &lt::Object, pos_as_origin: bool) -> Vec<(String, String)> {
    let mut keyvalues = vec![("classname".to_string(), o.class.clone())];
    for p in &o.properties {
        let key = match p.name.as_str() {
            n if n.eq_ignore_ascii_case("Name") => "targetname",
            n if pos_as_origin && n.eq_ignore_ascii_case("Pos") => "origin",
            n => n,
        };
        let value = match &p.value {
            lt::Value::String(s) => s.clone(),
            lt::Value::Vector(v) => format!("{} {} {}", v[0], v[2], v[1]),
            lt::Value::Color(c) => format!("{} {} {}", c[0], c[1], c[2]),
            lt::Value::Real(x) | lt::Value::Flags(x) | lt::Value::Long(x) => x.to_string(),
            lt::Value::Bool(b) => (*b as u8).to_string(),
            lt::Value::Rotation(r) => format!("{} {} {}", r[0], r[1], r[2]),
            lt::Value::Unknown { .. } => continue,
        };
        keyvalues.push((key.to_string(), value));
    }
    keyvalues
}

/// Whether a start point serves the versus modes' `family`
/// (game_modes.md S4): its `GameType`, not `Locked`, `Spawn` exactly 1.
pub fn qualifies(o: &lt::Object, family: &str) -> bool {
    o.string("GameType") == Some(family) && o.bool("Locked") != Some(true) && o.int("Spawn") == Some(1)
}

/// The family a map's spawns come from (game_modes.md S4): the team
/// modes' (`SPAWN_GAME_TYPE`) when it has a qualifying point on a team
/// (JSF or Creed: team modes leave `<none>` points out), else the map's
/// own: the family with the most qualifying unlocked points (the first in
/// map order on a tie; Hunted maps' `HungerGame`, say); None without any.
/// Each point is (`GameType`, unlocked, `Spawn` is 1, on a team).
pub fn map_family<'a>(points: impl Iterator<Item = (&'a str, bool, bool, bool)> + Clone) -> Option<&'a str> {
    if points
        .clone()
        .any(|(f, unlocked, q, team)| f == SPAWN_GAME_TYPE && q && unlocked && team)
    {
        return Some(SPAWN_GAME_TYPE);
    }
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for (f, unlocked, q, _) in points {
        if !(q && unlocked) {
            continue;
        }
        match counts.iter_mut().find(|(g, _)| *g == f) {
            Some((_, n)) => *n += 1,
            None => counts.push((f, 1)),
        }
    }
    let best = counts.iter().map(|(_, n)| *n).max()?;
    counts.into_iter().find(|(_, n)| *n == best).map(|(f, _)| f)
}

fn start_team(o: &lt::Object) -> Option<Team> {
    match o.string("Teams") {
        Some(t) if t.eq_ignore_ascii_case("JSF") => Some(Team(1)),
        Some(t) if t.eq_ignore_ascii_case("Creed") => Some(Team(2)),
        _ => None,
    }
}

/// The start points of the map's family (`map_family`) that qualify
/// (`qualifies`), in the map's order, feet on the floor below them; none:
/// the map's first start point (S4).
fn spawns(world: &World, map: &mut MapData, sky: &[[u32; 3]], to_engine: &impl Fn(lt::Vec3) -> Vec3) {
    let all: Vec<&lt::Object> = world.objects_of("GameStartPoint").map(|(_, o)| o).collect();
    let family = map_family(all.iter().map(|o| {
        (
            o.string("GameType").unwrap_or(""),
            o.bool("Locked") != Some(true),
            o.int("Spawn") == Some(1),
            start_team(o).is_some(),
        )
    }));
    let mut starts: Vec<&lt::Object> = match family {
        Some(f) => all.iter().copied().filter(|o| qualifies(o, f)).collect(),
        None => Vec::new(),
    };
    if starts.is_empty() {
        starts = all.into_iter().take(1).collect();
    }
    let floor = Floor::new(map, sky);
    let (mut spawns, mut yaws) = (Vec::new(), Vec::new());
    for o in starts {
        let Some(pos) = o.pos() else { continue };
        let at = to_engine(pos);
        let feet = floor.below(at).unwrap_or(at);
        let team = start_team(o);
        // The facing is (sin yaw, 0, cos yaw) in file axes, so (sin yaw,
        // 0, -cos yaw) here; an `Intent` yaw turns the other way.
        let yaw = o.rotation("Rotation").map_or(0.0, |r| r[1]);
        spawns.push((feet, team));
        yaws.push(-yaw);
    }
    map.spawns = spawns;
    map.spawn_yaws = yaws;
}

/// Each passed-through `GameStartPoint` and `TeleportPoint` entity
/// (`FEET_CLASSES`; in map order, as
/// `objects` passed them) gets its feet on the floor below it
/// (`FEET_KEY`, entity space, as `spawns` drops a point) and its facing
/// as an engine yaw (`YAW_KEY`), so a mode can spawn at any family's
/// points (game_modes.md S4).
fn start_point_feet(world: &World, map: &mut MapData, sky: &[[u32; 3]], scale: f32, to_engine: &impl Fn(lt::Vec3) -> Vec3) {
    let floor = Floor::new(map, sky);
    let feet: Vec<(Vec3, f32)> = world
        .objects
        .iter()
        .filter(|o| FEET_CLASSES.contains(&o.class.as_str()))
        .map(|o| {
            let at = o.pos().map(to_engine).unwrap_or(Vec3::ZERO);
            let yaw = o.rotation("Rotation").map_or(0.0, |r| r[1]);
            (floor.below(at).unwrap_or(at), -yaw)
        })
        .collect();
    let starts = map
        .entities
        .iter_mut()
        .filter(|e| FEET_CLASSES.contains(&e.classname()));
    for (e, (at, yaw)) in starts.zip(feet) {
        let v = engine_to_entity(at, scale);
        e.keyvalues.push((FEET_KEY.into(), format!("{} {} {}", v.x, v.y, v.z)));
        e.keyvalues.push((YAW_KEY.into(), yaw.to_string()));
    }
}

/// The feet of a passed-through start point (entity space), and its yaw
/// (engine radians), added by `start_point_feet`.
pub const FEET_KEY: &str = "chimera_feet";
/// The classes given feet and a yaw: start points and VIP Escort's
/// teleport points (game_modes_2.md M13.3: `MoveToFloor`, `ChangeRot`).
pub const FEET_CLASSES: &[&str] = &["GameStartPoint", "TeleportPoint"];
pub const YAW_KEY: &str = "chimera_yaw";

/// Downward rays against what players stand on (the collision triangles
/// and the players-only sets) but sky portals (the world's outer box is
/// one: below it is the void, not a floor); failing that, against what
/// rays meet (the trace-only sets: a liquid's surface, non-solid detail).
struct Floor<'a> {
    positions: &'a [[f32; 3]],
    triangles: Vec<([u32; 3], Vec2, Vec2)>,
    surfaces: Vec<([u32; 3], Vec2, Vec2)>,
}

impl<'a> Floor<'a> {
    fn new(map: &'a MapData, sky: &[[u32; 3]]) -> Self {
        let sky: std::collections::HashSet<[u32; 3]> = sky.iter().copied().collect();
        let p = |i: u32| Vec3::from(map.collision_positions[i as usize]);
        let boxed = |t: &[u32; 3]| {
            let [a, b, c] = t.map(p);
            (*t, a.xz().min(b.xz()).min(c.xz()), a.xz().max(b.xz()).max(c.xz()))
        };
        let players = map
            .collision_sets
            .iter()
            .filter(|s| s.characters || s.name == NODE_FLOORS)
            .flat_map(|s| &s.indices);
        let triangles = map
            .collision_indices
            .iter()
            .filter(|t| !sky.contains(*t))
            .chain(players)
            .map(boxed)
            .collect();
        let surfaces = map
            .collision_sets
            .iter()
            .filter(|s| (!s.characters && s.traces) || s.name == NODE_SURFACES)
            .flat_map(|s| &s.indices)
            .map(boxed)
            .collect();
        Self {
            positions: &map.collision_positions,
            triangles,
            surfaces,
        }
    }

    /// The highest floor point at or below `at`, however far down
    /// (movement.md "Spawn and respawn", case 47: an air spawn starts on
    /// the floor), but never a sky portal: over the void, the highest
    /// surface a ray meets (a liquid's), else None (no drop).
    fn below(&self, at: Vec3) -> Option<Vec3> {
        self.highest(&self.triangles, at)
            .or_else(|| self.highest(&self.surfaces, at))
            .map(|y| Vec3::new(at.x, y, at.z))
    }

    fn highest(&self, triangles: &[([u32; 3], Vec2, Vec2)], at: Vec3) -> Option<f32> {
        let p = |i: u32| Vec3::from(self.positions[i as usize]);
        let mut best: Option<f32> = None;
        for (t, lo, hi) in triangles {
            if at.x < lo.x || at.x > hi.x || at.z < lo.y || at.z > hi.y {
                continue;
            }
            let [a, b, c] = t.map(p);
            let n = (b - a).cross(c - a);
            if n.y.abs() < 1e-6 {
                continue;
            }
            // The plane's height at (x, z), then inside the triangle.
            let y = a.y - (n.x * (at.x - a.x) + n.z * (at.z - a.z)) / n.y;
            if y > at.y + 0.01 {
                continue;
            }
            let q = Vec3::new(at.x, y, at.z);
            let inside = [(a, b), (b, c), (c, a)]
                .iter()
                .map(|(u, v)| (*v - *u).cross(q - *u).dot(n))
                .all(|s| s >= -1e-6);
            if inside && best.is_none_or(|h| y > h) {
                best = Some(y);
            }
        }
        best
    }
}

/// A lightmap rectangle's texels: the stored bytes (lit by the atlas's
/// `LightLut` when laid out: a quarter of the memory), or light values.
enum Texels {
    Bytes(Vec<[u8; 3]>),
    Linear(Vec<[f32; 3]>),
}

/// Lightmaps gathered for one atlas.
struct Atlas {
    /// (width, height, texels) per rectangle.
    rects: Vec<(u32, u32, Texels)>,
    flats: HashMap<[u32; 3], u32>,
    lut: LightLut,
}

impl Atlas {
    fn new(lut: LightLut) -> Self {
        Self {
            rects: Vec::new(),
            flats: HashMap::new(),
            lut,
        }
    }

    /// Shelf places (tallest first) at `width`, and the height used.
    fn shelves(&self, width: u32) -> (Vec<(u32, u32)>, u32) {
        let mut order: Vec<usize> = (0..self.rects.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse((self.rects[i].1, self.rects[i].0)));
        let mut places = vec![(0u32, 0u32); self.rects.len()];
        let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
        for &i in &order {
            let (w, h) = (self.rects[i].0 + 2, self.rects[i].1 + 2);
            if x + w > width {
                (x, y, shelf) = (0, y + shelf, 0);
            }
            places[i] = (x, y);
            x += w;
            shelf = shelf.max(h);
        }
        (places, (y + shelf).max(1).next_multiple_of(4))
    }

    /// Every rectangle bigger than 2 x 2 at half its resolution (each
    /// texel the mean of the up to 2 x 2 it covers); the 2 x 2 ones are
    /// vertex lighting's exact blends and stay.
    fn halve(&mut self) {
        let lut = self.lut;
        for (w, h, stored) in &mut self.rects {
            if *w * *h <= 4 {
                continue;
            }
            let texels: Vec<[f32; 3]> = match stored {
                Texels::Bytes(b) => b.iter().map(|t| lut.get(*t)).collect(),
                Texels::Linear(l) => std::mem::take(l),
            };
            let (nw, nh) = (w.div_ceil(2), h.div_ceil(2));
            let mut out = Vec::with_capacity((nw * nh) as usize);
            for y in 0..nh {
                for x in 0..nw {
                    let mut sum = Vec3::ZERO;
                    let mut n = 0.0;
                    for (sx, sy) in [(2 * x, 2 * y), (2 * x + 1, 2 * y), (2 * x, 2 * y + 1), (2 * x + 1, 2 * y + 1)] {
                        if sx < *w && sy < *h {
                            sum += Vec3::from(texels[(sy * *w + sx) as usize]);
                            n += 1.0;
                        }
                    }
                    out.push((sum / n).to_array());
                }
            }
            (*w, *h, *stored) = (nw, nh, Texels::Linear(out));
        }
    }

    fn add(&mut self, width: u32, height: u32, texels: Vec<[f32; 3]>) -> u32 {
        self.add_texels(width, height, Texels::Linear(texels))
    }

    fn add_texels(&mut self, width: u32, height: u32, texels: Texels) -> u32 {
        self.rects.push((width, height, texels));
        (self.rects.len() - 1) as u32
    }

    /// One colour as a 2 x 2 rectangle (shared by equal colours).
    fn add_flat(&mut self, rgb: [f32; 3]) -> u32 {
        if let Some(&r) = self.flats.get(&rgb.map(f32::to_bits)) {
            return r;
        }
        let r = self.add(2, 2, vec![rgb; 4]);
        self.flats.insert(rgb.map(f32::to_bits), r);
        r
    }

    /// Lay the rectangles out in shelves (tallest first), each with a
    /// one-texel border copied from its edge.
    fn finish(mut self) -> LaidOut {
        let mut width = ATLAS_WIDTH;
        let (places, height) = loop {
            let (places, height) = self.shelves(width);
            if height <= ATLAS_MAX {
                break (places, height);
            }
            if width < ATLAS_MAX {
                width = ATLAS_MAX;
            } else {
                self.halve();
            }
        };
        let mut rgb = vec![[1.0f32; 3]; (width * height) as usize];
        let mut row = Vec::new();
        for (i, (w, h, texels)) in self.rects.iter().enumerate() {
            let (px, py) = places[i];
            let (w, h) = (*w as usize, *h as usize);
            for ty in 0..h + 2 {
                // The source row, its first and last texels repeated.
                let sy = ty.saturating_sub(1).min(h - 1);
                row.clear();
                match texels {
                    Texels::Bytes(b) => row.extend(b[sy * w..(sy + 1) * w].iter().map(|t| self.lut.get(*t))),
                    Texels::Linear(l) => row.extend_from_slice(&l[sy * w..(sy + 1) * w]),
                }
                let at = (py as usize + ty) * width as usize + px as usize;
                rgb[at] = row[0];
                rgb[at + 1..at + 1 + w].copy_from_slice(&row);
                rgb[at + w + 1] = row[w - 1];
            }
        }
        LaidOut {
            places,
            sizes: self.rects.iter().map(|r| (r.0, r.1)).collect(),
            map: MapLightmap {
                width,
                height,
                rgb,
                ..default()
            },
        }
    }
}

struct LaidOut {
    places: Vec<(u32, u32)>,
    sizes: Vec<(u32, u32)>,
    map: MapLightmap,
}

impl LaidOut {
    /// A coordinate in rectangle `rect` (clamped to 0..1) as atlas
    /// coordinates.
    fn uv(&self, rect: u32, uv: [f32; 2]) -> [f32; 2] {
        let (x, y) = self.places[rect as usize];
        let (w, h) = self.sizes[rect as usize];
        let [u, v] = uv.map(|c| c.clamp(0.0, 1.0));
        [
            (x as f32 + 1.0 + u * w as f32) / self.map.width as f32,
            (y as f32 + 1.0 + v * h as f32) / self.map.height as f32,
        ]
    }
}

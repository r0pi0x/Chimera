//! Halo's maps: the install's `maps/<name>.map` cache files as Chimera's
//! `MapData` (`MAP_LOADER`, `halo_ce:<name>`; specs/halo_ce/
//! cache_file.md, structure_bsp.md, collision.md, shaders.md,
//! scenario.md), and the list of maps the install holds (`map_names`).
//!
//! A load reads the file through the game's mount (`game::mount`, kept
//! per install in the app's `LoadContext`), hashes it (`file_hash`, for the
//! network handshake), and converts it (`convert`) through the
//! converted-map cache (`map::cache`, keyed by `CONVERTER`, this crate's
//! build.rs hash of its game sources, data and `blam_formats`, the load
//! options and the file's hash): a second load of the same map reads the
//! conversion back. The conversion, one concern per submodule:
//!
//! - `space`: Halo's world units and axes to the engine's metres and axes,
//!   the winding and the yaw;
//! - `lightmap`: the lightmap pages packed into one atlas, and the light's
//!   scale (`MapLook`: untonemapped, `light_scale` 1, bytes as sRGB);
//! - `geometry`: the structure BSP's materials as meshes;
//! - `materials`: the shaders as looks and their bitmaps as textures;
//! - `collision`: the level's collision BSP as collision triangles;
//! - `surfaces`: material types as the engine's surface names
//!   (data/surfaces.toml);
//! - `spawns`: the player starts as spawns and yaws;
//! - `sky`: fog, a flat background, and the seams for the sky model and
//!   scenery.
//!
//! `MapLoadOptions::textures` off (the lean load) keeps every texture a
//! placeholder of its size, everything else identical. Halo brings no kit
//! yet (`MapData::kit` default). Progress goes to the options' reporter.
//!
//! Known shortcuts: only the first structure BSP is converted (every
//! multiplayer map has one); the level's visibility (clusters, portals,
//! the potentially visible set) is not handed over, everything is drawn.

pub mod collision;
pub mod geometry;
pub mod lightmap;
pub mod materials;
pub mod sky;
pub mod space;
pub mod spawns;
pub mod surfaces;

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use blam_formats::{bitmap::Bitmap, cache::MapFile, scenario::Scenario, structure_bsp::StructureBsp};

use crate::{
    map::{
        LoadContext, MapData, MapLoadOptions, MapLoader,
        cache::{MapCache, MapKey, Outcome},
    },
    mount::Mount,
};

use super::{GAME, INFO, MULTIPLAYER_MAPS};

/// Halo's maps from the install (`map::MapLoader`), for an app's
/// `map::MapSources`.
pub static MAP_LOADER: MapLoader = MapLoader { game: GAME, load };

/// The converter's version for the converted-map cache: the hash of this
/// crate's game sources and data, `blam_formats`' sources and the
/// lockfile (build.rs).
pub const CONVERTER: &str = env!("CHIMERA_CONVERTER_SOURCES");

fn load(name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let mount = mount_for(&options.context)?;
    let cache = MapCache::from_env();
    load_in(&mount, name, options, cache.as_ref()).map(|(map, _)| map)
}

/// The mounted installs an app's loads share (`LoadContext`): the disc
/// image's directory is read once, not per map.
#[derive(Default)]
struct Mounts(Mutex<Vec<(PathBuf, Arc<Mount>)>>);

/// The install's mount for an app's loads (opened once per install path).
pub fn mount_for(context: &LoadContext) -> Result<Arc<Mount>, String> {
    let install = INFO.install().ok_or_else(|| {
        format!("no `{GAME}` install: set $CHIMERA_HALO_CE_ROOT or [games.{GAME}] path in chimera.local.toml")
    })?;
    let mounts = context.get_or_init(Mounts::default);
    let mut list = mounts.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, m)) = list.iter().find(|(p, _)| *p == install) {
        return Ok(m.clone());
    }
    let mount = Arc::new((INFO.open)(&install).map_err(|e| format!("{}: {e}", install.display()))?);
    list.push((install, mount.clone()));
    Ok(mount)
}

/// Map `name` from `mount` with `options`, through `cache` (None:
/// converted), and whether it came from there.
pub fn load_in(
    mount: &Mount,
    name: &str,
    options: &MapLoadOptions,
    cache: Option<&MapCache>,
) -> Result<(MapData, Outcome), String> {
    let start = std::time::Instant::now();
    options.progress.report(0.02, "Loading the map");
    let path = map_path(name);
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let file_hash: [u8; 32] = sha2::Digest::finalize(<sha2::Sha256 as sha2::Digest>::new_with_prefix(&bytes)).into();
    let key = crate::map::cache::key(GAME, CONVERTER)
        .part("map", name)
        .options(options)
        .part("file", file_hash);
    let (mut map, outcome) = crate::map::cache::cached(cache, mount, key, || {
        convert(&bytes, name, file_hash, options).map_err(|e| format!("{path}: {e}"))
    })?;
    let took = start.elapsed().as_secs_f32();
    if outcome == Outcome::Hit {
        bevy::log::info!("{name}: read from the map cache in {took:.2} s");
        map.load_times = vec![("map cache", took)];
    } else {
        map.load_times = vec![("convert", took)];
    }
    options.progress.report(0.95, "Loading the map");
    Ok((map, outcome))
}

/// A map cache file (`bytes`, as the disc stores it) as the engine's map
/// (module doc).
pub fn convert(bytes: &[u8], name: &str, file_hash: [u8; 32], options: &MapLoadOptions) -> Result<MapData, String> {
    let report = |f: f32, stage: &'static str| options.progress.report(f, stage);
    let decode = options.textures;
    let file = MapFile::load(bytes).map_err(|e| e.to_string())?;
    let scenario = Scenario::read(&file).map_err(|e| format!("scenario: {e}"))?;
    let bsps = Scenario::bsps(&file).map_err(|e| format!("structure BSPs: {e}"))?.len();
    if bsps == 0 {
        return Err("no structure BSP".into());
    }
    let bsp = StructureBsp::read(&file, 0).map_err(|e| format!("structure BSP 0: {e}"))?;
    let mut map = MapData {
        name: INFO.map_id(name),
        file_hash: Some(file_hash),
        ..Default::default()
    };
    if bsps > 1 {
        map.warnings.push(format!("{bsps} structure BSPs: only the first is converted"));
    }
    // The game's output is clamped, not tonemapped (`lightmap`).
    map.look.tonemapping = false;
    map.look.light_scale = 1.0;

    report(0.1, "Lighting");
    let (packed, pages) = lightmap_atlas(&file, &bsp, &mut map.warnings);
    report(0.3, "Geometry");
    let level = geometry::meshes(&bsp, &pages, packed.as_ref()).map_err(|e| format!("render geometry: {e}"))?;
    map.lightmap = packed.map(|p| p.map);

    report(0.45, "Textures");
    let mut looks: BTreeMap<u32, materials::Look> = BTreeMap::new();
    for m in &level {
        looks
            .entry(m.shader.id.0)
            .or_insert_with(|| materials::look(&file, &m.shader));
    }
    let mut requests = materials::Requests::default();
    let mut wanted: Vec<(Option<usize>, Option<usize>)> = Vec::with_capacity(level.len());
    for m in &level {
        let look = &looks[&m.shader.id.0];
        let base = look.base.map(|r| requests.want(r));
        let detail = look.detail.map(|d| requests.want(d.texture));
        wanted.push((base, detail));
    }
    let decoded = materials::decode_all(&file, &requests.list, decode);
    let mut slots: Vec<Option<usize>> = Vec::with_capacity(decoded.len());
    for d in decoded {
        match d {
            Ok(t) => {
                map.textures.push(t);
                slots.push(Some(map.textures.len() - 1));
            }
            Err(e) => {
                map.warnings.push(format!("texture unreadable, flat colour instead: {e}"));
                slots.push(None);
            }
        }
    }
    let mut reported = std::collections::BTreeSet::new();
    for (m, (base, detail)) in level.into_iter().zip(wanted) {
        let look = &looks[&m.shader.id.0];
        let mut mesh = m.mesh;
        if let Some(problem) = &look.problem
            && reported.insert(m.shader.id.0)
        {
            map.warnings.push(format!("shader `{}`: {problem}", mesh.material));
        }
        mesh.texture = base.and_then(|i| slots[i]);
        mesh.color = if mesh.texture.is_some() || (look.base.is_none() && look.problem.is_none()) {
            [255; 3]
        } else {
            materials::FLAT
        };
        if look.base.is_none() && look.problem.is_none() && reported.insert(m.shader.id.0) {
            map.warnings.push(format!("shader `{}`: no base map, drawn white", mesh.material));
        }
        if look.base.is_some() && mesh.texture.is_none() && reported.insert(m.shader.id.0) {
            map.warnings
                .push(format!("shader `{}`: its base map did not read, flat colour", mesh.material));
        }
        mesh.detail = look
            .detail
            .as_ref()
            .zip(detail.and_then(|i| slots[i]))
            .map(|(d, t)| materials::map_detail(d, t));
        mesh.alpha = look.alpha;
        mesh.double_sided = look.double_sided;
        // Without its lightmap a surface is drawn at its texture's own
        // brightness (the page-less group is the transparent one).
        mesh.unlit = look.unlit || !m.lit;
        mesh.tint = look.tint;
        mesh.base_transform = look.transform;
        mesh.surface = look.surface.clone();
        map.meshes.push(mesh);
    }

    report(0.8, "Collision");
    let level_collision =
        blam_formats::collision::read_level(&file, 0).map_err(|e| format!("collision BSP: {e}"))?;
    collision::add(&level_collision, &mut map).map_err(|e| format!("collision BSP: {e}"))?;

    report(0.9, "Spawns");
    spawns::add(&scenario, &mut map);
    let level_sky = sky::level_sky(&file, &scenario, &mut map.warnings);
    sky::add(level_sky.as_ref(), &bsp, &mut map, decode);
    sky::sky_model(&file, level_sky.as_ref(), &mut map);
    sky::scenery(&scenario, &mut map);
    Ok(map)
}

/// The BSP's lightmap pages packed into one atlas, and each group's page
/// in it (None: the group has no page, or it did not read: then a
/// warning).
fn lightmap_atlas(
    file: &MapFile,
    bsp: &StructureBsp,
    warnings: &mut Vec<String>,
) -> (Option<lightmap::Packed>, Vec<Option<usize>>) {
    let mut pages = vec![None; bsp.lightmap_groups.len()];
    let Some(tag) = file.tag(bsp.lightmap_bitmap.id) else {
        warnings.push("no lightmap bitmap: the level is drawn unlit".into());
        return (None, pages);
    };
    let bitmap = match Bitmap::read(file, tag) {
        Ok(b) => b,
        Err(e) => {
            warnings.push(format!("lightmap bitmap unreadable, the level is drawn unlit: {e}"));
            return (None, pages);
        }
    };
    let wanted: Vec<(usize, u16)> = bsp
        .lightmap_groups
        .iter()
        .enumerate()
        .filter_map(|(g, group)| group.bitmap_index.map(|i| (g, i)))
        .collect();
    let images: Vec<Option<blam_formats::bitmap::Image>> =
        wanted.iter().map(|&(_, i)| bitmap.images.get(i as usize).cloned()).collect();
    let decoded = crate::map::parallel::par_map(images, |image| {
        let image = image.ok_or_else(|| "no such image".to_string())?;
        image.decode_level(file, 0, 0).map_err(|e| e.to_string())
    });
    let mut atlas = lightmap::Atlas::default();
    for ((g, i), d) in wanted.into_iter().zip(decoded) {
        match d {
            Ok(level) => {
                let plane = &level.rgba[..(level.width * level.height * 4) as usize];
                pages[g] = Some(atlas.add(lightmap::Page::from_rgba8(level.width, level.height, plane)));
            }
            Err(e) => warnings.push(format!("lightmap page {i} (group {g}) unreadable, drawn unlit: {e}")),
        }
    }
    // Decoded whether textures are or not: the lightmap is not a texture.
    let packed = (!atlas.is_empty()).then(|| atlas.finish());
    (packed, pages)
}

/// A map file's path in the install.
pub fn map_path(name: &str) -> String {
    format!("maps/{name}.map")
}

/// The multiplayer maps the mount holds (`maps/<name>.map`), in the spec's
/// order (`MULTIPLAYER_MAPS`). The campaign maps and the menu's are not
/// listed.
pub fn held(mount: &Mount) -> Vec<String> {
    MULTIPLAYER_MAPS
        .iter()
        .filter(|name| mount.contains(&map_path(name)))
        .map(|name| name.to_string())
        .collect()
}

/// The maps the mounted install holds, by name, sorted (`INFO.map_names`).
pub fn map_names(mount: &Mount) -> Vec<String> {
    let mut names = held(mount);
    names.sort();
    names
}

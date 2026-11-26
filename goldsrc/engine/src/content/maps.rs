//! A BSP 30 map as Chimera's `MapData` (specs/goldsrc/bsp.md), for any
//! GoldSrc game (`GoldSrcGame`: its id, its converter's version, its
//! spawn classes and teams, its gravity, its kit). A load reads
//! `maps/<name>.bsp` through the game's mount, hashes it (`file_hash`, the
//! network handshake) and converts it (`convert`) through the
//! converted-map cache (`map::cache`, keyed by the game's converter, the
//! load options and the file's hash): a second load reads the conversion
//! back. The conversion, in order:
//!
//! 1. the clip hulls as plain data (`crate::hulls::trees`,
//!    `map::clip_trees::MapClipTrees` in the map's extras: the world's and
//!    the static brush entities'), then the entities (`entities::add`):
//!    the entity list, spawns, static brush entities' solids, ladders, and
//!    which brush models to draw;
//! 2. the textures the drawn faces use and their families (`textures`:
//!    an animation's frames, its alternate set, tiling variants), embedded
//!    or from the WADs `worldspawn`'s `wad` key names; one found nowhere
//!    is a placeholder and a warning `missing texture <name>`; each
//!    animation set of two or more frames a `MapTextureAnim` at
//!    `textures::FRAME_RATE`;
//! 3. the faces as meshes with the lightmap atlas (`brushes`; each light
//!    style beside 0 as the logic's spawn starts it, `style_start`),
//!    surfaces named from `sound/materials.txt`;
//! 4. the collision (`collision::add_world`) and the world's liquid
//!    leaves as water volumes (`water::add_world`; the liquid brush
//!    entities' came with the entities);
//! 5. the sky (`sky`, `skyname`, else `desert`) and the map's sprites
//!    (`sprites::add_map`);
//! 6. the sounds its entities play (`sounds::add_map`) and the step sets
//!    of the surfaces (`sounds::add_steps`);
//! 7. the game's kit (`GoldSrcGame::kit`: its bodies).
//!
//! The look: LDR, untonemapped, the lightmap as stored (`lightmap`'s
//! shortcut). Every step walks the file in its own order, so a map
//! converts to the same bytes each time (the cache and the served entries
//! rely on it).

use goldsrc_formats::{
    bsp::Bsp,
    materials::{Materials, Rule},
};

use super::{METERS_PER_UNIT, brushes, collision, entities, lightmap, sky, sounds, sprites, textures, water};
use crate::{
    core::Team,
    map::{
        KitLoad, MapData, MapLoadOptions,
        cache::{MapCache, MapKey, Outcome},
    },
    mount::Mount,
};

/// What a GoldSrc game gives the map conversion.
pub struct GoldSrcGame {
    /// Its id (`cs16`): the maps' ids (`<id>:<map>`) and the cache key.
    pub id: &'static str,
    /// Its converter's version (its build.rs: this crate's and the
    /// formats' sources among what it hashes).
    pub converter: &'static str,
    /// Its spawn classes and the team each is.
    pub spawns: &'static [(&'static str, Option<Team>)],
    /// Its `sv_gravity`, units/s² (loose bodies fall at it).
    pub gravity: f32,
    /// Its kit on its own map (bodies...), after the map's textures.
    pub kit: fn(&Mount, KitLoad, bool, &mut MapData),
    /// How its `materials.txt` is matched (`goldsrc_formats::materials`).
    pub materials: Rule,
}

/// A map's path on the mount.
pub fn map_path(name: &str) -> String {
    format!("maps/{name}.bsp")
}

/// The maps the mount holds (`maps/*.bsp`), by name, sorted.
pub fn map_names(mount: &Mount) -> Vec<String> {
    let mut names: Vec<String> = mount
        .files_in("maps")
        .iter()
        .filter_map(|f| f.rsplit('/').next()?.strip_suffix(".bsp").map(str::to_string))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Map `name` of `game` from `mount` with `options`, through `cache`
/// (None: converted), and whether it came from there.
pub fn load_in(
    game: &GoldSrcGame,
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
    let key = crate::map::cache::key(game.id, game.converter)
        .part("map", name)
        .options(options)
        .part("file", file_hash);
    let (mut map, outcome) = crate::map::cache::cached(cache, mount, key, || {
        convert(game, mount, &bytes, name, file_hash, options).map_err(|e| format!("{path}: {e}"))
    })?;
    let took = start.elapsed().as_secs_f32();
    map.load_times = vec![(if outcome == Outcome::Hit { "map cache" } else { "convert" }, took)];
    options.progress.report(0.95, "Loading the map");
    Ok((map, outcome))
}

/// Map `name` of `game` from `mount` through the cache the environment
/// names (`MapCache::from_env`).
pub fn load_level_with(game: &GoldSrcGame, mount: &Mount, name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let cache = MapCache::from_env();
    load_in(game, mount, name, options, cache.as_ref()).map(|(m, _)| m)
}

/// A map file (`bytes`) as the engine's map (module doc).
pub fn convert(
    game: &GoldSrcGame,
    mount: &Mount,
    bytes: &[u8],
    name: &str,
    file_hash: [u8; 32],
    options: &MapLoadOptions,
) -> Result<MapData, String> {
    let report = |f: f32, stage: &'static str| options.progress.report(f, stage);
    let decode = options.textures;
    let bsp = Bsp::parse(bytes).map_err(|e| e.to_string())?;
    let mut data = MapData {
        name: format!("{}:{name}", game.id),
        file_hash: Some(file_hash),
        ..Default::default()
    };
    data.look.tonemapping = false;
    data.look.light_scale = 1.0;
    data.look.source_ldr_lightmaps = false;
    data.gravity = Some(game.gravity * METERS_PER_UNIT);

    report(0.1, "Entities");
    // The clip hulls (`crate::hulls`): the world's here, the static brush
    // entities' as the entity pass places them.
    data.extras.insert(crate::hulls::trees(&bsp));
    let parts = entities::add(&bsp, game.spawns, &mut data);
    let worldspawn = bsp.entities().into_iter().next().unwrap_or_default();

    report(0.2, "Textures");
    let wads = textures::Wads::open(mount, &goldsrc_formats::bsp::wad_names(&worldspawn), &mut data.warnings);
    // The textures the drawn faces use, in the lump's order.
    let mut used = vec![false; bsp.textures.len()];
    for d in &parts.drawn {
        let Some(m) = bsp.models.get(d.model) else { continue };
        let first = m.first_face.max(0) as usize;
        for f in bsp.faces.iter().skip(first).take(m.face_count.max(0) as usize) {
            if let Some(ti) = bsp.texinfo.get(f.texinfo as usize)
                && let Some(u) = used.get_mut(ti.texture as usize)
            {
                *u = true;
            }
        }
    }
    // A used texture's family (frames, alternate set, tiling variants).
    let names: Vec<Option<&str>> = bsp.textures.iter().map(|t| t.as_ref().map(|t| t.name.as_str())).collect();
    let families = textures::Families::of(&names);
    for i in 0..used.len() {
        if used[i] {
            for m in families.members(i) {
                used[m] = true;
            }
        }
    }
    let mut slots = vec![None; bsp.textures.len()];
    for (i, tex) in bsp.textures.iter().enumerate() {
        let Some(tex) = tex else { continue };
        let k = textures::kind(&tex.name);
        if !used[i] || matches!(k, textures::Kind::Sky | textures::Kind::Tool) {
            continue;
        }
        let texture = match textures::resolve(tex, &wads) {
            Some(found) => textures::map_texture(&found, decode),
            None => {
                data.warnings.push(format!("missing texture {}", tex.name));
                textures::placeholder(&tex.name, tex.width, tex.height, decode)
            }
        };
        data.textures.push(texture);
        slots[i] = Some(data.textures.len() - 1);
    }

    // Each animation set's frames shown in turn (`render::texture_anim`).
    let mut animated = vec![false; bsp.textures.len()];
    for (main, alternate) in &families.frames {
        for set in [main, alternate] {
            if set.len() < 2 || set.iter().any(|&i| animated[i]) {
                continue;
            }
            let Some(frames) = set.iter().map(|&i| slots[i]).collect::<Option<Vec<usize>>>() else { continue };
            set.iter().for_each(|&i| animated[i] = true);
            data.texture_anims.push(crate::map::MapTextureAnim { frames, rate: textures::FRAME_RATE });
        }
    }

    report(0.4, "Geometry");
    let materials = mount
        .read("sound/materials.txt")
        .map(|b| Materials::parse_with(&String::from_utf8_lossy(&b), game.materials))
        .unwrap_or_default();
    // How the light styles start: the logic's own spawn (switchable lights
    // dark or lit, the presets' patterns).
    let logic = crate::logic::world::World::new(&data.entities, 0);
    let styles = |s: u8| style_start(&logic, s);
    brushes::add_meshes(&bsp, &parts.drawn, &slots, &families, &materials, &styles, &mut data);

    report(0.6, "Collision");
    let counts = collision::add_world(&bsp, &mut data);
    let liquids = water::add_world(&bsp, &mut data);
    bevy::log::debug!("{name}: collision {counts:?}, {liquids} liquid leaves, {} water volumes", data.water.len());

    report(0.8, "Sky");
    let skyname = worldspawn.get("skyname").filter(|s| !s.trim().is_empty()).unwrap_or("desert");
    sky::add(mount, skyname.trim(), decode, &mut data);

    sprites::add_map(mount, decode, &mut data);

    report(0.82, "Sounds");
    sounds::add_map(mount, &logic, std::sync::Arc::make_mut(&mut data.sounds), &mut data.warnings);
    sounds::add_steps(mount, std::sync::Arc::make_mut(&mut data.sounds), &mut data.warnings);

    report(0.85, "Kit");
    (game.kit)(mount, options.kit, decode, &mut data);
    Ok(data)
}

/// How light style `style` starts on a map whose logic is `logic`: a
/// switchable light's (32 and up) on unless its pattern is dark; a preset
/// (1-12) animated by its letters; others lit.
pub fn style_start(logic: &crate::logic::world::World, style: u8) -> lightmap::StyleStart {
    use crate::logic::tables;
    match logic.styles.get(&style) {
        Some(p) => lightmap::StyleStart { on: p != "a", pattern: Vec::new() },
        None => match tables::preset_style(style).filter(|p| p.len() > 1) {
            Some(p) => lightmap::StyleStart {
                on: true,
                pattern: p.bytes().map(tables::letter_brightness).collect(),
            },
            None => lightmap::StyleStart { on: true, pattern: Vec::new() },
        },
    }
}

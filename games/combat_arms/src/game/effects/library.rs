//! What the effects take from the install, at map load
//! (`add_to_map`, from `world::load_from`; `add_looks` on another game's
//! map, `world::dress`) and on first use:
//!
//! - the effect file (`clientfx/clientfx.fxf`, the unkeyed `CLIENTFX`
//!   archive, `lithtech_formats::fxf`), parsed once per process
//!   (`library`);
//! - the textures of the effect groups the tables and the weapons name
//!   (their view models' attached effects and their projectiles' groups
//!   in flight included), the faces' looping groups (`attached_groups`)
//!   and the projectiles' flare and smoke trail sprites (`SMOKE_TRAILS`;
//!   the `FX` archive, unkeyed), each a particle material of the map
//!   (`material_name`: texture and blend), its size kept for sprites
//!   (`FxLibrary::texture_size`);
//! - the world's surface types on the drawn meshes (`tag_surfaces`): each
//!   texture takes the type its physics polygons have most, so Chimera's
//!   `SurfaceGrid` answers a hit point's or a foot's type;
//! - sound files (`sound_clip`, the unkeyed `GUNS_SND_*`, `SND`,
//!   `CHARS_SND` and `UI_SND` archives), read through the install's mount
//!   when first played.
//!
//! A file that doesn't read (a keyed archive without its key, or a name
//! the install lacks) is left out with one warning: silence, or no
//! texture (the effect draws untextured).

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
};

use bevy::prelude::*;
use lithtech_formats::{
    dtx::Dtx,
    fxf::{FxFile, FxGroup, FxKey},
    world::{World, surface_flags},
};

use super::data::{CaFxTables, CaWeaponFx, EFFECTS};
use crate::game::special::{PROJECTILE, ProjectileLook, ProjectileParams};
use crate::{
    map::{
        MapData, MapSoundClip, MapTexture,
        particles::{ParticleBlend, ParticleMaterial},
    },
    mount::Mount,
};

/// The effect file inside the install.
pub const FX_FILE: &str = "clientfx/clientfx.fxf";

/// The effect file and what was loaded for it.
#[derive(Debug, Default)]
pub struct FxLibrary {
    pub file: FxFile,
    /// Texture path (normalized) -> size in texels.
    pub textures: HashMap<String, UVec2>,
    /// Sprite file (normalized) -> its first frame's texture.
    pub sprites: HashMap<String, String>,
}

impl FxLibrary {
    pub fn group(&self, name: &str) -> Option<&FxGroup> {
        self.file.group(name)
    }

    pub fn texture_size(&self, path: &str) -> Option<UVec2> {
        self.textures.get(&normalize(path)).copied()
    }

    /// The texture a key draws (as loaded: `key_texture`).
    pub fn texture_of(&self, key: &FxKey) -> Option<String> {
        match key.kind.as_str() {
            "ParticleSystem" | "PolyTrail" | "PolyTube" => key.path("Texture").map(normalize),
            "Sprite" => self.sprites.get(&normalize(key.path("Sprite")?)).cloned(),
            _ => None,
        }
    }
}

static LIBRARY: Mutex<Option<Arc<FxLibrary>>> = Mutex::new(None);

/// The effect file as last loaded with a map (None: no install, or not
/// loaded yet).
pub fn library() -> Option<Arc<FxLibrary>> {
    LIBRARY.lock().ok()?.clone()
}

/// Use `lib` as the effect library (tests, tools).
pub fn set_library(lib: Option<Arc<FxLibrary>>) {
    if let Ok(mut l) = LIBRARY.lock() {
        *l = lib;
    }
}

/// An archive path as the mount names it: lower case, `/`.
pub fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// The particle material name of a texture drawn with `blend`.
pub fn material_name(texture: &str, additive: bool) -> String {
    format!("ca_fx:{}|{}", normalize(texture), if additive { "add" } else { "alpha" })
}

/// The texture an effect key draws: a particle system's `Texture`, a
/// sprite's `Sprite` (`.spr`: its first frame's texture).
pub fn key_texture(key: &FxKey, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Option<String> {
    match key.kind.as_str() {
        "ParticleSystem" | "PolyTrail" | "PolyTube" => key.path("Texture").map(normalize),
        "Sprite" => {
            let spr = normalize(key.path("Sprite")?);
            sprite_texture(&file(&spr)?)
        }
        _ => None,
    }
}

/// A sprite file's first frame: the texture path it names (our reading of
/// the file: five 32-bit fields, then each frame as a 16-bit length and
/// the path).
pub fn sprite_texture(bytes: &[u8]) -> Option<String> {
    let len = u16::from_le_bytes(bytes.get(20..22)?.try_into().ok()?) as usize;
    let name = bytes.get(22..22 + len)?;
    Some(normalize(&String::from_utf8_lossy(name)))
}

/// Whether a key is drawn through an effect shader (`EffectShaderID` >
/// 0): in the stock file only the refraction keys (heat shimmer and the
/// blasts' shock rings, whose textures are distortion maps, not colours).
/// We have no effect shaders: such keys draw nothing.
pub fn shaded(key: &FxKey) -> bool {
    key.int("EffectShaderID").is_some_and(|id| id > 0)
}

/// Whether a key draws additively.
pub fn additive(key: &FxKey) -> bool {
    key.combo("BlendMode") == Some(1)
}

/// Every effect group the tables and the weapons name, with the groups
/// those create (`CreateFX`'s `FXName`, splats' `SplatEffect`).
pub fn wanted_groups(tables: &CaFxTables, weapons: &[CaWeaponFx], view_muzzles: &[String], fx: &FxFile) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for s in &tables.surface {
        names.extend([&s.impact_fx, &s.uw_impact_fx, &s.grenade_fx, &s.uw_grenade_fx].map(String::clone));
    }
    names.extend(tables.impact.values().map(|i| i.fx.clone()));
    names.extend(tables.fire.values().map(|f| f.fx.clone()));
    for a in &tables.attachment {
        names.extend(a.pv_muzzle.iter().chain(&a.hh_muzzle).cloned());
    }
    names.extend(weapons.iter().map(|w| w.hh_muzzle.clone()));
    names.extend(weapons.iter().filter_map(|w| w.impact_record.as_ref()).map(|i| i.fx.clone()));
    names.extend(weapons.iter().filter_map(|w| w.fire_record.as_ref()).map(|f| f.fx.clone()));
    names.extend(view_muzzles.iter().cloned());
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    while let Some(n) = names.pop() {
        if n.is_empty() || !seen.insert(n.to_lowercase()) {
            continue;
        }
        let Some(g) = fx.group(&n) else { continue };
        for k in &g.keys {
            names.extend(k.string("FXName").map(str::to_string));
            names.extend(k.string("SplatEffect").map(str::to_string));
        }
        out.push(g.name.clone());
    }
    out.sort();
    out
}

/// The effects' part of a map from the install: the effect file (once),
/// the wanted groups' textures as particle materials, the surface types
/// on the drawn meshes. `world` is the map's world file.
pub fn add_to_map(map: &mut MapData, data: &super::super::data::CaDataInner, world: &World, file: &dyn Fn(&str) -> Option<Vec<u8>>) {
    tag_surfaces(map, world);
    add_looks(map, data, file);
}

/// The effects' part of any map, a world file or not (another game's map
/// in a mashup, `world::dress`): the effect file (once) and the wanted
/// groups' textures as the map's particle materials.
pub fn add_looks(map: &mut MapData, data: &super::super::data::CaDataInner, file: &dyn Fn(&str) -> Option<Vec<u8>>) {
    let fx = match load_fx_file(file) {
        Ok(f) => f,
        Err(e) => {
            map.warnings.push(format!("effects: {e}: no effect looks"));
            set_library(None);
            return;
        }
    };
    // Combat Arms' impacts can be drawn on the map (`map::impact`).
    map.impacts.add(crate::game::GAME);
    let set = data.weapon_set();
    let weapons: Vec<CaWeaponFx> = set.weapons.iter().filter_map(|w| w.hook(EFFECTS)).collect();
    let muzzles: Vec<String> = set
        .weapons
        .iter()
        .filter_map(|w| w.hook::<super::super::view_model::CaViewParams>(super::super::view_model::VIEW))
        .flat_map(|p| std::iter::once(p.muzzle_fx).chain(p.attach_fx))
        .collect();
    let mut groups = wanted_groups(CaFxTables::built_in(), &weapons, &muzzles, &fx);
    // What follows an entity: projectiles' groups in flight and the
    // faces' looping groups (the infected's eyes).
    let looks: Vec<ProjectileLook> = set
        .weapons
        .iter()
        .filter_map(|w| w.hook::<ProjectileParams>(PROJECTILE))
        .map(|p| p.look)
        .collect();
    groups.extend(attached_groups(&looks, data.characters(), &fx));
    groups.sort();
    groups.dedup();
    let mut loaded = Loaded::default();
    for name in &groups {
        let Some(g) = fx.group(name) else { continue };
        for k in g.keys.iter().filter(|k| !shaded(k)) {
            let Some(path) = key_texture(k, file) else { continue };
            if let Some(spr) = k.path("Sprite").filter(|_| k.kind == "Sprite") {
                loaded.sprites.insert(normalize(spr), path.clone());
            }
            loaded.material(map, &path, additive(k), file);
        }
    }
    // Projectiles' flares (unlit, drawn additive) and smoke trails
    // (special_weapons.md Follow-up 3), from their sprite files.
    let mut sprites: Vec<(String, bool)> = looks
        .iter()
        .filter_map(|l| l.flare.as_ref())
        .map(|f| (f.sprite.clone(), true))
        .collect();
    if looks.iter().any(|l| l.smoke_trail.is_some()) {
        sprites.extend(SMOKE_TRAILS.iter().map(|t| (t.sprite.to_string(), false)));
    }
    for (spr, add) in sprites {
        let spr = normalize(&spr);
        let Some(path) = file(&spr).and_then(|b| sprite_texture(&b)) else {
            loaded.missing += 1;
            continue;
        };
        loaded.sprites.insert(spr, path.clone());
        loaded.material(map, &path, add, file);
    }
    let Loaded { textures, sprites, missing } = loaded;
    if missing > 0 {
        map.warnings.push(format!("effects: {missing} effect textures missing or unreadable: drawn untextured"));
    }
    set_library(Some(Arc::new(FxLibrary {
        file: fx,
        textures,
        sprites,
    })));
}

/// What a map's effect looks loaded: textures by path (their sizes),
/// sprite files' first frames, how many didn't read.
#[derive(Default)]
struct Loaded {
    textures: HashMap<String, UVec2>,
    sprites: HashMap<String, String>,
    missing: usize,
}

impl Loaded {
    /// `path` drawn with `add` (additive, else alpha) as the map's
    /// particle material `material_name`, its texture read once.
    fn material(&mut self, map: &mut MapData, path: &str, add: bool, file: &dyn Fn(&str) -> Option<Vec<u8>>) {
        let mat = material_name(path, add);
        if map.particles.find(&mat).is_some() {
            return;
        }
        let texture = match self.textures.get(path) {
            Some(_) => map.textures.iter().position(|t| t.name == path),
            None => match file(path).and_then(|b| Dtx::parse(b).ok()) {
                Some(dtx) => {
                    self.textures.insert(path.to_string(), UVec2::new(dtx.width, dtx.height));
                    map.textures.push(MapTexture {
                        name: path.to_string(),
                        srgb: true,
                        mips: Vec::new(),
                        width: dtx.width,
                        height: dtx.height,
                        rgba8: dtx.rgba(0),
                        bc: None,
                    });
                    Some(map.textures.len() - 1)
                }
                None => {
                    self.missing += 1;
                    None
                }
            },
        };
        map.particles.materials.push(ParticleMaterial {
            name: mat,
            texture,
            blend: if add { ParticleBlend::Additive } else { ParticleBlend::Alpha },
            vertex_color: true,
            vertex_alpha: true,
            ..default()
        });
    }
}

/// The groups that follow an entity: the projectiles' (`looks`) and the
/// faces' `ClientFX#` lists, with what they create.
pub fn attached_groups(looks: &[ProjectileLook], characters: &crate::game::characters::records::CharacterData, fx: &FxFile) -> Vec<String> {
    let mut names: Vec<String> = looks.iter().map(|l| l.fx.clone()).collect();
    names.extend(characters.face.iter().flat_map(|f| f.client_fx.iter().cloned()));
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    while let Some(n) = names.pop() {
        if n.is_empty() || !seen.insert(n.to_lowercase()) {
            continue;
        }
        let Some(g) = fx.group(&n) else { continue };
        for k in &g.keys {
            names.extend(k.string("FXName").map(str::to_string));
            names.extend(k.string("SplatEffect").map(str::to_string));
        }
        out.push(g.name.clone());
    }
    out
}

/// A smoke trail's look by `SmokeTrailType` (special_weapons.md
/// Follow-up 3): its sprite, segment life and emitting time (s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmokeTrailLook {
    pub sprite: &'static str,
    pub life: f32,
    pub emitting: f32,
}

/// Types 1 (and any other value), 2 and 3.
pub const SMOKE_TRAILS: [SmokeTrailLook; 3] = [
    SmokeTrailLook {
        sprite: "SFX\\Smoke\\Spr\\WhiteTrailShort.spr",
        life: 0.75,
        emitting: 0.2,
    },
    SmokeTrailLook {
        sprite: "SFX\\Smoke\\Spr\\WhiteTrailLong.spr",
        life: 1.5,
        emitting: 0.2,
    },
    SmokeTrailLook {
        sprite: "SFX\\Smoke\\Spr\\BlackTrailShort.spr",
        life: 0.75,
        emitting: 0.3,
    },
];

/// The smoke trail of type `t`.
pub fn smoke_trail(t: u32) -> SmokeTrailLook {
    match t {
        2 => SMOKE_TRAILS[1],
        3 => SMOKE_TRAILS[2],
        _ => SMOKE_TRAILS[0],
    }
}

/// The effect file, parsed once per process.
fn load_fx_file(file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<FxFile, String> {
    static PARSED: OnceLock<Result<FxFile, String>> = OnceLock::new();
    PARSED
        .get_or_init(|| {
            let bytes = file(FX_FILE).ok_or_else(|| format!("{FX_FILE} doesn't read"))?;
            FxFile::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| format!("{FX_FILE}: {e}"))
        })
        .clone()
}

/// Give each drawn mesh of the world (no entity) the surface type its
/// texture's physics polygons have most often (as its `surface`, the
/// number in text), so `SurfaceGrid` answers types; textures the physics
/// BSP doesn't use stay untyped (0, Unknown).
pub fn tag_surfaces(map: &mut MapData, world: &World) {
    let mut counts: HashMap<String, HashMap<u16, usize>> = HashMap::new();
    for m in world.models.iter().filter(|m| m.is_physics()) {
        for i in 0..m.polygons.len() {
            let s = m.polygon_surface(i);
            if s.flags & surface_flags::SOLID == 0 {
                continue;
            }
            let Some(t) = m.textures.get(s.texture as usize) else { continue };
            *counts.entry(normalize(t)).or_default().entry(s.surface_type).or_default() += 1;
        }
    }
    let types: HashMap<String, u16> = counts
        .into_iter()
        .filter_map(|(t, c)| c.into_iter().max_by_key(|(ty, n)| (*n, u16::MAX - *ty)).map(|(ty, _)| (t, ty)))
        .collect();
    for m in map.meshes.iter_mut().filter(|m| m.entity.is_none()) {
        if let Some(ty) = types.get(&normalize(&m.material)) {
            m.surface = Some(ty.to_string());
        }
    }
}

/// A sound file from the install, decoded (None: missing, keyed or not a
/// sound; warned once per file).
pub fn sound_clip(path: &str, mount: &Mount) -> Option<MapSoundClip> {
    match mount.read(path) {
        Ok(b) => match source_formats::wav::decode_any(&b) {
            Ok(c) => Some(c),
            Err(e) => {
                warn!("Combat Arms sound {path}: {e}");
                None
            }
        },
        Err(e) => {
            warn!("Combat Arms sound {path}: {e} (silent)");
            None
        }
    }
}

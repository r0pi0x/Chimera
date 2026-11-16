//! The Spartan body from a map file (`spartan`), on Halo's maps
//! (`add_own`, from the map's own file: the converted-map cache keeps it)
//! and on any other game's (`GUEST`, the bundle's dressing: from the donor
//! map, `DONOR_MAP`, read once per app through its `LoadContext`).
//!
//! - The biped `BIPED` names the model, the animation graph and the
//!   collision model (objects.md, the object base).
//! - Meshes: the map loader's model meshes (`maps::objects::
//!   skinned_meshes`: highest detail, first permutation, parts merged by
//!   shader) with joints, in the character's frame (metres, the engine's
//!   axes, feet at the origin: `maps::space::point` is the body's root
//!   applied); their looks are the loader's (`maps::apply_looks`: the model
//!   shader's minimum approximation), their textures the body's own.
//! - Armour colour: a mesh whose model shader takes its change colour from
//!   slot C (the armour: objects.md, "Spartan multiplayer colour") is
//!   tinted by the player's colour where its multipurpose map's alpha
//!   says (the change-colour mask, `MapMesh::tint_mask`, which the loader's
//!   look sets): one skin per colour of `body::COLOURS`, the meshes
//!   themselves showing skin 0 (white). A campaign body names its own
//!   slot and a colour for a player given none (`Tint`: the Elite's armour
//!   reads slot A, all over, and wears the minor's blue uncoloured, an
//!   extra skin after `COLOURS`'); the Spartan's slot C left the Elite
//!   untinted (its grey base map under the blue sheen: the white and teal
//!   of stage one).
//! - Skeleton, animations and hit volumes: `skeleton`, `anims`, `hits`.
//! - Any biped the same way (`biped_body`): the campaign's characters
//!   from their units (`campaign::unit`).
//!
//! Known shortcuts: the light-shield parts (transparent chicago shaders)
//! are drawn as the loader draws them, always.

use std::sync::{Arc, Mutex};

use blam_formats::{
    animation::AnimationGraph,
    cache::{MapFile, TagRef, class},
    collision,
    model::Model,
    object::Biped,
    shader::{AnimationSource, Shader},
};

use super::super::{
    GAME,
    maps::{
        self,
        objects::{self, Dest, Pending},
        space,
    },
};
use super::{COLOURS, anims, colour, hits, skeleton};
use crate::map::{
    LoadContext, MapCharacterModel, MapData, MapDressing, MapLoadOptions, MapMesh, MapMeshLook, MapModel, MapTexture,
};

/// The multiplayer Spartan's biped tag.
pub const BIPED: &str = "characters\\cyborg_mp\\cyborg_mp";

/// The map the body comes from on other games' maps (every multiplayer
/// map holds the same biped; the plan's choice).
pub const DONOR_MAP: &str = "bloodgulch";

/// The sounds of the Spartan's animation graph (its deaths' and others'),
/// for the effects library to decode (`fx::load::compile`); none when the
/// map has no Spartan.
pub fn sound_tags(file: &MapFile) -> Vec<TagRef> {
    let graph = file
        .find(class::BIPED, BIPED)
        .and_then(|t| Biped::read(file, t).ok())
        .and_then(|b| file.tag(b.object.animation_graph.id).and_then(|t| AnimationGraph::read(file, t).ok()));
    let mut tags: Vec<TagRef> = graph.map_or_else(Vec::new, |g| g.sounds.iter().map(|s| s.sound).collect());
    tags.extend(death_lines(file, BIPED).into_iter().flatten());
    tags
}

/// A biped's death lines (sounds.md "Unit dialogue": the unit's
/// dialogue, variant 0's else the first, its vocalizations 14 death quiet,
/// falling back to 15, and 15 death violent): quiet and violent, None where
/// empty (a character's unit without its voice set: its dialogue's bytes
/// are zero, so none).
pub fn death_lines(file: &MapFile, biped: &str) -> [Option<TagRef>; 2] {
    let dialogue = file
        .find(class::BIPED, biped)
        .and_then(|t| Biped::read(file, t).ok())
        .and_then(|b| {
            let v = &b.unit.dialogue_variants;
            v.iter().find(|d| d.variant_number == 0).or(v.first()).map(|d| d.dialogue)
        })
        .and_then(|r| file.tag(r.id));
    let Some(view) = dialogue.and_then(|t| file.tag_view(t, 0x10 + 20 * 0x10).ok()) else {
        return [None, None];
    };
    let line = |i: usize| view.tag_ref(0x10 + i * 0x10).ok().filter(|r| file.tag(r.id).is_some());
    let violent = line(15);
    [line(14).or(violent), violent]
}

/// The body and its textures (its meshes' texture indices point into
/// `textures`).
#[derive(Clone, Debug)]
pub struct Spartan {
    pub character: MapCharacterModel,
    pub textures: Vec<MapTexture>,
    pub warnings: Vec<String>,
}

fn tag<'a>(file: &'a MapFile, r: &TagRef, what: &str) -> Result<&'a blam_formats::cache::TagEntry, String> {
    file.tag(r.id).ok_or_else(|| format!("the biped's {what} is missing"))
}

/// Whether a shader takes its change colour from slot `source` (the
/// armour: C on the Spartan, A on the Elite).
fn armour(file: &MapFile, shader: &TagRef, source: AnimationSource) -> bool {
    file.tag(shader.id)
        .and_then(|t| Shader::read(file, t).ok())
        .is_some_and(|s| matches!(s, Shader::Model(m) if m.change_color_source == source))
}

/// How a body's armour takes the player's colour (`biped_body`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tint {
    /// The change colour slot its armour shaders read: the Spartan's C,
    /// the Elite's A (characters.md, "The Elite's colours").
    pub source: AnimationSource,
    /// The colour of a player given none (no team): an extra skin after
    /// `COLOURS`' (the Elite minor's blue); None: `COLOURS`' first.
    pub uncoloured: Option<[f32; 3]>,
}

/// The Spartan's: slot C, white without a colour.
pub const SPARTAN_TINT: Tint = Tint { source: AnimationSource::C, uncoloured: None };

/// A mesh's look with `tint`.
fn look(m: &MapMesh, tint: Option<[f32; 3]>) -> MapMeshLook {
    MapMeshLook {
        material: m.material.clone(),
        texture: m.texture,
        alpha: m.alpha,
        double_sided: m.double_sided,
        unlit: m.unlit,
        normal_map: m.normal_map,
        phong: m.phong.clone(),
        envmap: m.envmap,
        tint,
        detail: m.detail,
        base_transform: m.base_transform,
        selfillum: m.selfillum,
        skin_frames: None,
        layer: None,
    }
}

/// The Spartan of map `file` (module doc); `decode` false: textures as
/// placeholders of their size (a lean load).
pub fn spartan(file: &MapFile, decode: bool) -> Result<Spartan, String> {
    biped_body(file, BIPED, decode, SPARTAN_TINT)
}

/// The body of biped `biped` of map `file`, as `spartan` makes the
/// Spartan's (the campaign's characters' from their units:
/// `campaign::unit`): unnamed (the caller names a body not every Halo
/// player wears).
pub fn biped_body(file: &MapFile, biped_path: &str, decode: bool, tint: Tint) -> Result<Spartan, String> {
    let biped_tag = file
        .find(class::BIPED, biped_path)
        .ok_or_else(|| format!("no biped `{biped_path}`"))?;
    let biped = Biped::read(file, biped_tag).map_err(|e| e.to_string())?;
    let o = &biped.object;
    let model = Model::read(file, tag(file, &o.model, "model")?).map_err(|e| e.to_string())?;
    let graph =
        AnimationGraph::read(file, tag(file, &o.animation_graph, "animation graph")?).map_err(|e| e.to_string())?;
    let coll =
        collision::read_model(file, tag(file, &o.collision_model, "collision model")?).map_err(|e| e.to_string())?;
    let bones = skeleton::bones(&model);
    let found = objects::skinned_meshes(file, &model, &|p| space::point(p));
    let tinted: Vec<bool> = found.iter().map(|(_, s)| armour(file, s, tint.source)).collect();
    let pending = found
        .into_iter()
        .map(|(mesh, shader)| Pending {
            mesh,
            shader,
            lit: true,
            dest: Dest::Model(0),
        })
        .collect();
    let mut textures = Vec::new();
    let mut warnings = Vec::new();
    let meshes: Vec<MapMesh> = maps::apply_looks(file, pending, &mut textures, &mut warnings, decode)
        .into_iter()
        .map(|(m, _)| m)
        .collect();
    let colours: Vec<[f32; 3]> = (0..COLOURS.len()).map(colour).chain(tint.uncoloured).collect();
    let skins: Vec<Vec<MapMeshLook>> = colours
        .iter()
        .map(|c| meshes.iter().zip(&tinted).map(|(m, t)| look(m, t.then_some(*c))).collect())
        .collect();
    let (lo, hi) = meshes.iter().flat_map(|m| &m.positions).fold(
        (bevy::math::Vec3::splat(f32::MAX), bevy::math::Vec3::splat(f32::MIN)),
        |(lo, hi), p| (lo.min((*p).into()), hi.max((*p).into())),
    );
    let (mut set, left_out) = anims::build(&graph, &bones, model.node_list_checksum);
    warnings.extend(left_out);
    // The animations' sounds as the effects library names them
    // (`sound_tags`: the library decodes them).
    anims::name_sounds(&mut set, |i| {
        let r = graph.sounds.get(i)?.sound;
        Some(super::super::fx::name_of(&file.tag(r.id)?.path))
    });
    // The death lines ride on the bind sequence (`anims::DEATH_LINE_EVENT`).
    for (k, r) in death_lines(file, biped_path).iter().enumerate() {
        if let Some(t) = r.and_then(|r| file.tag(r.id)) {
            set.sequences[0].events.push(crate::map::anim::AnimEvent {
                cycle: 0.0,
                event: k as i32,
                name: anims::DEATH_LINE_EVENT.into(),
                options: super::super::fx::name_of(&t.path),
            });
        }
    }
    let root = skeleton::root();
    let boxes = hits::boxes(&coll, &bones);
    let regions = hits::regions(&bones, &boxes);
    let hitboxes = hits::hitboxes(&boxes, &regions, &bones, &set.defaults, &root);
    Ok(Spartan {
        character: MapCharacterModel {
            game: Some(GAME),
            team: None,
            name: None,
            class: None,
            model: MapModel {
                meshes,
                bounds: if lo.x <= hi.x { (lo, hi) } else { Default::default() },
                skins,
                ..Default::default()
            },
            hitboxes,
            bones,
            root,
            animations: Some(Arc::new(set)),
            boxes,
            ragdoll: None,
        },
        textures,
        warnings,
    })
}

/// `body`'s character added to `map`'s kit, its textures appended to the
/// map's (the indices moved past the map's own).
pub fn add(map: &mut MapData, body: &Spartan) {
    let offset = map.textures.len();
    let mut c = body.character.clone();
    let shift = |t: &mut Option<usize>| {
        if let Some(i) = t {
            *i += offset;
        }
    };
    for m in &mut c.model.meshes {
        shift(&mut m.texture);
        shift(&mut m.normal_map);
        if let Some(d) = &mut m.detail {
            d.texture += offset;
        }
    }
    for l in c.model.skins.iter_mut().flatten() {
        shift(&mut l.texture);
        shift(&mut l.normal_map);
        if let Some(d) = &mut l.detail {
            d.texture += offset;
        }
    }
    map.textures.extend(body.textures.iter().cloned());
    let who = body.character.name.as_deref().unwrap_or("the Spartan");
    map.warnings.extend(body.warnings.iter().map(|w| format!("{who}: {w}")));
    map.kit.characters.push(c);
}

/// The body of a Halo map's own file added to it (or a warning).
pub fn add_own(file: &MapFile, map: &mut MapData, decode: bool) {
    match spartan(file, decode) {
        Ok(b) => add(map, &b),
        Err(e) => map.warnings.push(format!("no Spartan body: {e}")),
    }
}

/// The donor's bodies an app's loads share (`LoadContext`): with and
/// without textures.
#[derive(Default)]
struct Donor(Mutex<[Option<Arc<Spartan>>; 2]>);

/// The donor map's body for an app (made once per app and `decode`).
pub fn donor(context: &LoadContext, decode: bool) -> Result<Arc<Spartan>, String> {
    let memo = context.get_or_init(Donor::default);
    let mut slots = memo.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(b) = &slots[usize::from(decode)] {
        return Ok(b.clone());
    }
    let mount = maps::mount_for(context)?;
    let path = maps::map_path(DONOR_MAP);
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let file = MapFile::load(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let body = Arc::new(spartan(&file, decode).map_err(|e| format!("{path}: {e}"))?);
    slots[usize::from(decode)] = Some(body.clone());
    Ok(body)
}

/// The Spartan on another game's map (the donor's).
pub fn dress(map: &mut MapData, options: &MapLoadOptions) -> Result<(), String> {
    let body = donor(&options.context, options.textures)?;
    add(map, &body);
    Ok(())
}

/// Halo's players on the maps of other games: the Spartan body
/// (`game::BUNDLE`'s guest dressing).
pub static GUEST: MapDressing = MapDressing { game: GAME, dress };

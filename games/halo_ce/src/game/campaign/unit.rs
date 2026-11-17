//! Each campaign character as its own unit (specs/halo_ce/characters.md,
//! section 8; docs/plans/active/ingest.md 3.5a and 3.6): the converter
//! `halo_ce:character_unit` that cuts a character out of its campaign map,
//! the paths its sets hold, and the body made from them on demand.
//!
//! **The converter** (`convert`, `CONVERTER` version `VERSION`): from the
//! donor map's file (`maps/<donor>.map`, `BodyDef::donor_map`) it reads
//! the biped's closure (`blam_formats::unit`: every tag reachable through
//! references but actor variants and actors, weapons and HUD interfaces)
//! and splits it in two units (the asset graph's `deferred` edge: a body
//! is playable before its lines arrive):
//! - the body, `character/<id>`: the closure without the biped's dialogue
//!   (`udlg`) and what only the dialogue reaches;
//! - the voice, `voice/<id>`: the dialogue and what only it reaches (its
//!   lines). A missing voice set is a silent body, never a failure. A
//!   kind with several voices (`BodyDef::voices`, the Marine's) has no
//!   such set: each of its dialogues is one, `voice/<id>_<name>`
//!   (`voice_set_of`, file `voice_<name>.map`: the dialogue and what only
//!   it reaches), and a body loads the one it speaks (`load_voice`).
//!
//! - its own weapons (`BodyDef::weapons`: the Hunter's cannon), each a
//!   file of the body's set (`weapon_path`, `weapon_<name>.map`: a body
//!   and its own gun are what "play as it" downloads, characters.md
//!   section 8; a set of its own would be named as the recorded
//!   `weapon/<id>` sets of Halo's guns are): the weapon's closure less HUD
//!   interfaces. The cannon's numbers are its data's
//!   (data/weapons/hunter_fuel_rod.toml); its effects join the effects'
//!   library at each map's load once the file is on the mount
//!   (`fx::load::own_weapons`), and its rod's flight model is a late held
//!   model made with the body (`load_body`, `view::load::projectile_models`).
//!
//! Each is written as a small map file (`unit::cut`: the donor's header,
//! index and paths, the unit's tags and raw data at their own addresses,
//! zeros elsewhere) that `MapFile::load` reads unchanged, so the body is
//! read by the same code as the Spartan's (`body::load::biped_body`). The
//! files live under `GEN_ROOT/v<VERSION>/<id>/` (`body.map`, `voice.map`,
//! and `recipe.txt`: the converter, its version and the donor file's
//! SHA-256, the input the outputs are keyed by), all in the install
//! build's manifest: `chimera_pack sets halo_ce --only character/` (and
//! `--only voice/`) cuts and stores them (`generated`). The output is
//! deterministic (`blam_formats::unit`); `VERSION` is bumped whenever
//! any output byte could change.
//!
//! **In the game** (`UnitsOnDemand`, `load_on_demand`): a body kind worn
//! by a character, or wanted by the host's draw while campaign bodies are
//! on, is read from the mount (`body.map` at `body_path`: the asset
//! store's generated set, layered under the install or standing in for
//! it) in the background (a kind wanted but not worn yet only while the
//! read-ahead has room, `chimera_core::tasks::ReadAhead`), once per
//! session, made into a character model
//! named `BodyKind::model_name` (`body::load::biped_body`) and added to
//! every map of Halo's (`map::add_character_model`). Without the file (no
//! set fetched, no store) it is never offered: the draw gives the Spartan
//! (`UnitsOnDemand::ready`). The campaign map itself is never read at run
//! time. Its held guns (`held_on`) go to the engine's `LateHeldModels`
//! and stay there across maps.
//!
//! Known shortcuts:
//! - On-demand fetching of a set the store lacks is the asset service's
//!   (`request_set`): a client without the files has no Elite.
//! - The unit keeps the donor's file offsets, so its image inflates to
//!   about 97 MB (the Elite's) for the moment of its conversion
//!   (`blam_formats::unit` "Known shortcuts").

use std::collections::{BTreeMap, BTreeSet};

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};
use blam_formats::{
    cache::{MapFile, class},
    unit::{self, character_excluded},
};

use super::{BodyDef, BodyKind, CampaignData};
use crate::{
    map::{CharacterModels, LoadContext, MapCharacterModel, MapTexture, add_character_model},
    mount::Mount,
};

/// The converter's name (ingest.md 3.6).
pub const CONVERTER: &str = "halo_ce:character_unit";

/// Its version: bumped by hand whenever any output byte could change.
pub const VERSION: u32 = 1;

/// Where its outputs live in Halo's manifest.
pub const GEN_ROOT: &str = "chimera/gen/halo_ce.character_unit";

/// The body unit's set (`character/elite`).
pub fn body_set(id: &str) -> String {
    format!("character/{id}")
}

/// The voice unit's set (`voice/elite`).
pub fn voice_set(id: &str) -> String {
    format!("voice/{id}")
}

fn dir(id: &str) -> String {
    format!("{GEN_ROOT}/v{VERSION}/{id}")
}

/// The body unit's file.
pub fn body_path(id: &str) -> String {
    format!("{}/body.map", dir(id))
}

/// The voice unit's file.
pub fn voice_path(id: &str) -> String {
    format!("{}/voice.map", dir(id))
}

/// A voice of a kind with several (`BodyDef::voices`), as its own set
/// (`voice/marine_bisenti`).
pub fn voice_set_of(id: &str, name: &str) -> String {
    format!("voice/{id}_{name}")
}

/// That voice's unit's file.
pub fn voice_path_of(id: &str, name: &str) -> String {
    format!("{}/voice_{name}.map", dir(id))
}

/// A character's own weapon's name (`BodyDef::weapons`): the tag's name,
/// spaces as underscores (`hunter_fuel_rod`).
pub fn weapon_name(tag: &str) -> String {
    dialogue_name(tag).replace(' ', "_")
}

/// That weapon's unit's file (beside its character's).
pub fn weapon_path(id: &str, tag: &str) -> String {
    format!("{}/weapon_{}.map", dir(id), weapon_name(tag))
}

/// A dialogue tag's name: its path's last part (`bisenti` of the
/// Marines' `...\bisenti`), lower case.
pub fn dialogue_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_ascii_lowercase()
}

/// The recipe: what the outputs were made from.
pub fn recipe_path(id: &str) -> String {
    format!("{}/recipe.txt", dir(id))
}

/// What `convert` made: the two units' files, their measures and the
/// recipe.
pub struct Converted {
    pub body: Vec<u8>,
    /// Its one voice (empty for a kind with several: `voices`).
    pub voice: Vec<u8>,
    pub recipe: String,
    pub body_unit: unit::Unit,
    pub voice_unit: unit::Unit,
    /// A kind with several voices (`BodyDef::voices`): each one's name,
    /// file and measure, in `BodyDef::voices`' order.
    pub voices: Vec<(String, Vec<u8>, unit::Unit)>,
    /// Its own weapons (`BodyDef::weapons`): each tag, file and measure.
    pub weapons: Vec<(String, Vec<u8>, unit::Unit)>,
}

/// The SHA-256 of `bytes`, as hex.
fn sha256_hex(bytes: &[u8]) -> String {
    let h: [u8; 32] = sha2::Digest::finalize(<sha2::Sha256 as sha2::Digest>::new_with_prefix(bytes)).into();
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// Character `id` (`def`) cut out of its donor map's file `donor` (module
/// doc).
pub fn convert(id: &str, def: &BodyDef, donor: &[u8]) -> Result<Converted, String> {
    let map = MapFile::load(donor).map_err(|e| format!("{}: {e}", def.donor_map))?;
    let biped = map
        .find(class::BIPED, &def.biped)
        .ok_or_else(|| format!("{}: no biped `{}`", def.donor_map, def.biped))?
        .index();
    let enter = |t: &blam_formats::cache::TagEntry| !character_excluded(t);
    let whole = unit::closure(&map, &[biped], enter);
    let body = unit::closure(&map, &[biped], |t| enter(t) && t.class != class::DIALOGUE);
    let dialogues: Vec<u16> =
        whole.iter().copied().filter(|&i| map.tags()[i as usize].class == class::DIALOGUE).collect();
    let e = |e: std::io::Error| format!("{}: {e}", def.donor_map);
    // Several voices: each dialogue of `def.voices` alone, less the body.
    let mut voices = Vec::new();
    for name in &def.voices {
        let d = dialogues
            .iter()
            .copied()
            .find(|&i| dialogue_name(&map.tags()[i as usize].path) == *name)
            .ok_or_else(|| format!("{}: no dialogue `{name}` for `{}`", def.donor_map, def.biped))?;
        let tags: BTreeSet<u16> = unit::closure(&map, &[d], enter).difference(&body).copied().collect();
        let u = unit::measure(&map, tags).map_err(e)?;
        voices.push((name.clone(), unit::cut(&map, &u).map_err(e)?, u));
    }
    // Its own weapons: each the weapon's closure (HUD interfaces left out,
    // characters.md section 8, "The weapons characters need").
    let mut weapons = Vec::new();
    for tag in &def.weapons {
        let w = map.find(class::WEAPON, tag).ok_or_else(|| format!("{}: no weapon `{tag}`", def.donor_map))?.index();
        let tags = unit::closure(&map, &[w], |t| t.index() == w || !character_excluded(t));
        let u = unit::measure(&map, tags).map_err(e)?;
        weapons.push((tag.clone(), unit::cut(&map, &u).map_err(e)?, u));
    }
    let voice: BTreeSet<u16> = if def.voices.is_empty() {
        unit::closure(&map, &dialogues, enter).difference(&body).copied().collect()
    } else {
        BTreeSet::new()
    };
    let body_unit = unit::measure(&map, body).map_err(e)?;
    let voice_unit = unit::measure(&map, voice).map_err(e)?;
    let mut recipe = format!(
        "converter {CONVERTER} {VERSION}\ncharacter {id}\ninput maps/{}.map {}\nbiped {}\nbody {} tags {} bytes\nvoice {} tags {} bytes\n",
        def.donor_map,
        sha256_hex(donor),
        def.biped,
        body_unit.tags.len(),
        body_unit.total(),
        voice_unit.tags.len(),
        voice_unit.total(),
    );
    for (name, _, u) in &voices {
        recipe.push_str(&format!("voice {name} {} tags {} bytes\n", u.tags.len(), u.total()));
    }
    Ok(Converted {
        body: unit::cut(&map, &body_unit).map_err(e)?,
        voice: unit::cut(&map, &voice_unit).map_err(e)?,
        recipe,
        body_unit,
        voice_unit,
        voices,
        weapons,
    })
}

/// The generated sets for `chimera_pack sets halo_ce` (those whose names
/// start with `only`): each character's body and voice sets with their
/// files, from the donor maps `mount` holds; a character whose donor map
/// is missing is said and skipped.
pub fn generated(mount: &Mount, only: &str) -> Vec<(String, Vec<(String, Vec<u8>)>)> {
    let data = CampaignData::builtin();
    let mut out = Vec::new();
    for (id, def) in &data.bodies {
        let (body_set, voice_set) = (body_set(id), voice_set(id));
        let wanted = |set: &str| set.starts_with(only) || only.starts_with(set);
        let named: Vec<String> = def.voices.iter().map(|n| voice_set_of(id, n)).collect();
        if !wanted(&body_set) && !wanted(&voice_set) && !named.iter().any(|s| wanted(s)) {
            continue;
        }
        let path = super::super::maps::map_path(&def.donor_map);
        let converted = mount.read(&path).map_err(|e| format!("{path}: {e}")).and_then(|b| convert(id, def, &b));
        match converted {
            Ok(c) => {
                if wanted(&body_set) {
                    // Its own weapons go with the body: "play as" it needs
                    // both (characters.md section 8).
                    let mut files = vec![(body_path(id), c.body), (recipe_path(id), c.recipe.into_bytes())];
                    files.extend(c.weapons.into_iter().map(|(tag, bytes, _)| (weapon_path(id, &tag), bytes)));
                    out.push((body_set, files));
                }
                if def.voices.is_empty() && wanted(&voice_set) {
                    out.push((voice_set, vec![(voice_path(id), c.voice)]));
                }
                for (name, bytes, _) in c.voices {
                    let set = voice_set_of(id, &name);
                    if wanted(&set) {
                        out.push((set, vec![(voice_path_of(id, &name), bytes)]));
                    }
                }

            }
            Err(e) => println!("{body_set}: {e}"),
        }
    }
    out
}

/// Body `kind`'s character model from its unit on `mount` (named
/// `BodyKind::model_name`), with its textures.
pub fn load_body(kind: BodyKind, def: &BodyDef, mount: &Mount, decode: bool, guns: &[&str]) -> Result<LoadedBody, String> {
    let path = body_path(kind.id());
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let file = MapFile::load(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let body = super::super::body::load::biped_body(&file, &def.biped, decode, def.tint()).map_err(|e| format!("{path}: {e}"))?;
    let mut character = body.character;
    character.name = kind.model_name();
    // An arm shield's disc: a body part of two choices, drawn on 0 and
    // hidden on 1 (`super::shield_disc`).
    if let Some(disc) = def.shield_disc.as_deref().map(str::to_ascii_lowercase) {
        let mut any = false;
        for (i, m) in character.model.meshes.iter_mut().enumerate() {
            if m.material.to_ascii_lowercase().contains(&disc) {
                m.body = Some((0, 0));
                // The energy shield's colour (`materials::generic_stage_tint`:
                // the looser reading, for this disc alone), on the mesh
                // and every skin's look of it.
                let path = m.material.replace('/', "\\");
                let shader = file
                    .find(class::SHADER_TRANSPARENT_GENERIC, &path)
                    .and_then(|t| blam_formats::shader::Shader::read(&file, t).ok());
                if let Some(blam_formats::shader::Shader::TransparentGeneric(s)) = shader
                    && m.tint.is_none()
                {
                    m.tint = super::super::maps::materials::generic_stage_tint(&s);
                    for skin in &mut character.model.skins {
                        if let Some(look) = skin.get_mut(i) {
                            look.tint = m.tint;
                        }
                    }
                }
                any = true;
            }
        }
        if any {
            character.model.body_parts = vec![2];
        }
    }
    for w in &body.warnings {
        debug!("halo_ce: {}: {w}", kind.id());
    }
    // Its guns by its own hand (none without the donor map: the Spartan's
    // placement then).
    let mut held = held_on(kind, def, &file, mount, guns, decode).unwrap_or_else(|e| {
        warn!("halo_ce: the {} holds guns by the Spartan's hand: {e}", kind.id());
        HeldOn::default()
    });
    // Its own weapons' projectiles in flight (the fuel rod), drawn as
    // shown items (`view::load::object_key`) from late held models.
    for tag in &def.weapons {
        let path = weapon_path(kind.id(), tag);
        let Some(w) = mount.read(&path).ok().and_then(|b| MapFile::load(&b).ok()) else {
            warn!("halo_ce: the {}'s {tag} shows no projectile model: no {path}", kind.id());
            continue;
        };
        let (models, textures, warnings) = super::super::view::load::projectile_models(&w, &[tag.as_str()], decode);
        for x in warnings {
            debug!("halo_ce: {} {tag}: {x}", kind.id());
        }
        // Their textures after the held guns' (each model's meshes name
        // indices into its own list: moved past them).
        let offset = held.textures.len();
        held.textures.extend(textures);
        held.models.extend(models.into_iter().map(|mut m| {
            super::super::view::load::shift_textures(&mut m.model, offset);
            m
        }));
    }
    // Its sounds: its footsteps from the body, its lines from the voice
    // set when there is one (a silent body without).
    let voice = if def.voices.is_empty() {
        mount.read(&voice_path(kind.id())).ok().and_then(|b| MapFile::load(&b).ok())
    } else {
        // A kind with several voices: its body's footsteps here, the
        // drawn voice's lines on their own (`load_voice`).
        None
    };
    let sounds = super::voice::compile(def, &file, voice.as_ref());
    Ok(LoadedBody { model: character, textures: body.textures, held, sounds })
}

/// A body loaded: its model and textures, and its held guns (`HeldOn`).
pub struct LoadedBody {
    pub model: MapCharacterModel,
    pub textures: Vec<MapTexture>,
    pub held: HeldOn,
    /// Its footsteps and lines (`voice`).
    pub sounds: super::voice::BodySounds,
}

/// A campaign body's held guns (`view::load::body_held`), with their
/// textures.
#[derive(Default)]
pub struct HeldOn {
    pub models: Vec<crate::map::MapHeldModel>,
    pub textures: Vec<MapTexture>,
}

/// Body `kind`'s held guns `guns`, by its own hand marker: the guns' world
/// models from the donor multiplayer map on `mount`, the body's model and
/// graph from its unit `file`.
pub fn held_on(kind: BodyKind, def: &BodyDef, file: &MapFile, mount: &Mount, guns: &[&str], decode: bool) -> Result<HeldOn, String> {
    use blam_formats::{animation::AnimationGraph, model::Model, object::Biped};
    let b = file.find(class::BIPED, &def.biped).ok_or("no biped")?;
    let biped = Biped::read(file, b).map_err(|e| e.to_string())?;
    let model = Model::read(file, file.tag(biped.object.model.id).ok_or("no model")?).map_err(|e| e.to_string())?;
    let graph = AnimationGraph::read(file, file.tag(biped.object.animation_graph.id).ok_or("no graph")?)
        .map_err(|e| e.to_string())?;
    let path = super::super::maps::map_path(super::super::sets::DONOR_MAP);
    let donor = MapFile::load(&mount.read(&path).map_err(|e| format!("{path}: {e}"))?).map_err(|e| format!("{path}: {e}"))?;
    let (models, textures, warnings) =
        super::super::view::load::body_held(&donor, &model, &graph, kind.id(), guns, decode);
    for w in warnings {
        debug!("halo_ce: {} held guns: {w}", kind.id());
    }
    Ok(HeldOn { models, textures })
}

/// A body's unit and its first-person views (`first_person::view_model`,
/// made on the task too: each is a walk of the body's meshes).
type Loaded = Result<(LoadedBody, Views), String>;

type Views = Vec<(std::sync::Arc<chimera_world::map::view_model::MapViewModel>, super::first_person::BodyView)>;

/// Voice `index` of body `kind` (`BodyDef::voices`): its lines from its
/// own set's file on `mount`, read with the body's biped.
pub fn load_voice(kind: BodyKind, index: u8, def: &BodyDef, mount: &Mount) -> Result<super::voice::BodySounds, String> {
    let name = def.voices.get(usize::from(index)).ok_or_else(|| format!("no voice {index}"))?;
    let read = |path: String| -> Result<MapFile, String> {
        let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
        MapFile::load(&bytes).map_err(|e| format!("{path}: {e}"))
    };
    let body = read(body_path(kind.id()))?;
    let voice = read(voice_path_of(kind.id(), name))?;
    Ok(super::voice::compile_lines(def, &body, &voice, Some(name)))
}

/// The bodies loading, loaded and failed (module doc): what the host's
/// draw asks (`ready`).
#[derive(Resource, Default)]
pub struct UnitsOnDemand {
    tasks: BTreeMap<BodyKind, Task<Loaded>>,
    tried: BTreeSet<BodyKind>,
    ready: BTreeSet<BodyKind>,
    failed: BTreeMap<BodyKind, String>,
    /// Kinds wanted though nobody wears them yet (the draw's, while
    /// campaign bodies are on).
    pub wanted: BTreeSet<BodyKind>,
    /// The voices of kinds with several (`load_voice`), each read once.
    voice_tasks: BTreeMap<(BodyKind, u8), Task<Result<super::voice::BodySounds, String>>>,
    voices_tried: BTreeSet<(BodyKind, u8)>,
    /// Kinds loading ahead of need (wanted, worn by nobody): counted in
    /// `chimera_core::tasks::ReadAhead`.
    ahead: BTreeSet<BodyKind>,
}

impl UnitsOnDemand {
    /// Whether body `kind` can be worn here: the Spartan always, another
    /// once its unit loaded.
    pub fn ready(&self, kind: BodyKind) -> bool {
        kind == BodyKind::Spartan || self.ready.contains(&kind)
    }

    /// Why body `kind` did not load, if it did not.
    pub fn failed(&self, kind: BodyKind) -> Option<&str> {
        self.failed.get(&kind).map(String::as_str)
    }

    /// One line on each kind: ready, loading, failed or not asked for.
    pub fn status(&self) -> String {
        BodyKind::ALL
            .iter()
            .map(|k| {
                let s = if self.ready(*k) {
                    "ready".to_string()
                } else if self.loading(*k) {
                    "loading".into()
                } else if let Some(e) = self.failed(*k) {
                    format!("failed: {e}")
                } else {
                    "not asked for".into()
                };
                format!("{}: {s}", k.id())
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Whether `kind` is loading.
    pub fn loading(&self, kind: BodyKind) -> bool {
        self.tasks.contains_key(&kind)
    }

    /// Mark `kind` loaded (a test's model added by hand).
    pub fn mark_ready(&mut self, kind: BodyKind) {
        self.tried.insert(kind);
        self.ready.insert(kind);
    }
}

/// Each body kind worn or wanted whose unit is not loaded: read in the
/// background once, added as a character model when done (module doc).
pub fn load_on_demand(
    worn: Query<(&BodyKind, Option<&super::BodyVoice>)>,
    models: Option<Res<CharacterModels>>,
    data: Res<CampaignData>,
    context: Option<Res<LoadContext>>,
    mut pending: ResMut<UnitsOnDemand>,
    wait: Option<Res<chimera_core::tasks::WaitForTasks>>,
    mut read_ahead: Option<ResMut<chimera_core::tasks::ReadAhead>>,
    mut commands: Commands,
) {
    const WHO: &str = "halo_ce campaign bodies";
    let mut done = Vec::new();
    pending.tasks.retain(|kind, task| {
        let Some(result) = chimera_core::tasks::poll(task, wait.is_some()) else { return true };
        done.push((*kind, result));
        false
    });
    for (kind, result) in done {
        match result {
            Ok((LoadedBody { model, textures, held, sounds }, views)) => {
                info!(
                    "halo_ce: the {} body loaded ({} hit boxes, {} guns in its hand, {} footstep kinds, {} voice lines)",
                    kind.id(),
                    model.hitboxes.len(),
                    held.models.len(),
                    sounds.set.library.footsteps.len(),
                    sounds.filled()
                );
                pending.ready.insert(kind);
                let textures = std::sync::Arc::new(textures);
                let held_textures = std::sync::Arc::new(held.textures);
                let sounds = std::sync::Arc::new(sounds);
                commands.queue(move |world: &mut World| {
                    for (m, view) in views {
                        info!("halo_ce: the {} body is the first-person view of {} ({} meshes kept)", kind.id(), m.key, m.model.meshes.len());
                        world.get_resource_or_init::<super::first_person::BodyViews>().0.insert(m.key.clone(), (m, textures.clone(), view));
                    }
                    world.get_resource_or_init::<super::voice::CampaignSounds>().0.insert(kind, sounds);
                    for h in held.models {
                        crate::map::add_held_model(world, h, held_textures.clone());
                    }
                    add_character_model(world, model, textures);
                });
            }
            Err(e) => {
                warn!("halo_ce: the {} body is not offered: {e}", kind.id());
                pending.failed.insert(kind, e);
            }
        }
    }
    // A model a map already carries (a test's) counts as loaded.
    if let Some(m) = &models {
        for kind in BodyKind::ALL {
            if let Some(name) = kind.model_name()
                && !pending.ready.contains(&kind)
                && m.0.iter().any(|x| x.name.as_deref() == Some(name.as_str()))
            {
                pending.mark_ready(kind);
            }
        }
    }
    // The voices: each one a body wears, once its unit is in.
    let mut heard = Vec::new();
    pending.voice_tasks.retain(|key, task| {
        let Some(result) = chimera_core::tasks::poll(task, wait.is_some()) else { return true };
        heard.push((*key, result));
        false
    });
    for ((kind, v), result) in heard {
        match result {
            Ok(lines) => {
                info!("halo_ce: the {} body's voice {v} loaded ({} lines)", kind.id(), lines.filled());
                let lines = std::sync::Arc::new(lines);
                commands.queue(move |world: &mut World| {
                    world.get_resource_or_init::<super::voice::CampaignVoices>().0.insert((kind, v), lines);
                });
            }
            Err(e) => warn!("halo_ce: the {} body speaks no voice {v}: {e}", kind.id()),
        }
    }
    let voices: Vec<(BodyKind, u8)> = worn.iter().filter_map(|(k, v)| Some((*k, v?.0))).collect();
    for (kind, v) in voices {
        if !pending.ready.contains(&kind) || !pending.voices_tried.insert((kind, v)) {
            continue;
        }
        let Some(def) = data.body(kind).cloned() else { continue };
        let context = context.as_deref().cloned().unwrap_or_default();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let mount = super::super::maps::mount_for(&context)?;
            load_voice(kind, v, &def, &mount)
        });
        pending.voice_tasks.insert((kind, v), task);
    }
    // Those worn now, then those wanted ahead of need (a ladder's kinds:
    // only while the read-ahead has room, so a worn body's cook or a
    // pipeline never queues behind five units).
    {
        let UnitsOnDemand { tasks, ahead, .. } = &mut *pending;
        ahead.retain(|k| tasks.contains_key(k));
    }
    if let Some(r) = read_ahead.as_mut() {
        r.set(WHO, pending.ahead.len());
    }
    let worn_now: BTreeSet<BodyKind> = worn.iter().map(|(k, _)| *k).collect();
    let wanted: Vec<(BodyKind, bool)> =
        worn_now.iter().map(|k| (*k, false)).chain(pending.wanted.iter().filter(|k| !worn_now.contains(k)).map(|k| (*k, true))).collect();
    for (kind, early) in wanted {
        if kind == BodyKind::Spartan || pending.tried.contains(&kind) {
            continue;
        }
        if early && !chimera_core::tasks::read_ahead_room(read_ahead.as_deref()) {
            break;
        }
        if early {
            pending.ahead.insert(kind);
            let n = pending.ahead.len();
            if let Some(r) = read_ahead.as_mut() {
                r.set(WHO, n);
            }
        }
        pending.tried.insert(kind);
        let Some(def) = data.body(kind).cloned() else {
            pending.failed.insert(kind, "not in data/campaign.toml".into());
            continue;
        };
        let context = context.as_deref().cloned().unwrap_or_default();
        let guns: Vec<String> = data.guns.iter().filter(|(_, w)| w.get(kind.id()).is_some_and(|x| *x > 0.0)).map(|(g, _)| g.clone()).collect();
        // Its first-person view from its own model (`first_person`), the
        // eye at its standing camera.
        let view = def.first_person.clone();
        let eye = {
            let base = crate::game::movement::MovementData::builtin();
            data.movement(kind, &base).map_or(base.camera_stand, |m| m.camera_stand) * crate::game::METERS_PER_UNIT
        };
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let mount = super::super::maps::mount_for(&context)?;
            let guns: Vec<&str> = guns.iter().map(String::as_str).collect();
            let body = load_body(kind, &def, &mount, true, &guns)?;
            let views: Views = view
                .map(|view| {
                    view.guns
                        .iter()
                        .map(|g| (std::sync::Arc::new(super::first_person::view_model(&body.model, &view, g, eye)), view.clone()))
                        .collect()
                })
                .unwrap_or_default();
            Ok((body, views))
        });
        pending.tasks.insert(kind, task);
    }
}

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
//!   lines). A missing voice set is a silent body, never a failure.
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
//! it) in the background, once per session, made into a character model
//! named `BodyKind::model_name` (`body::load::biped_body`) and added to
//! every map of Halo's (`map::add_character_model`). Without the file (no
//! set fetched, no store) it is never offered: the draw gives the Spartan
//! (`UnitsOnDemand::ready`). The campaign map itself is never read at run
//! time.
//!
//! Known shortcuts:
//! - The voice set is cut and stored but nothing plays it yet (the body
//!   speaks no pain or death line: the next stage).
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

/// The recipe: what the outputs were made from.
pub fn recipe_path(id: &str) -> String {
    format!("{}/recipe.txt", dir(id))
}

/// What `convert` made: the two units' files, their measures and the
/// recipe.
pub struct Converted {
    pub body: Vec<u8>,
    pub voice: Vec<u8>,
    pub recipe: String,
    pub body_unit: unit::Unit,
    pub voice_unit: unit::Unit,
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
    let voice: BTreeSet<u16> = unit::closure(&map, &dialogues, enter).difference(&body).copied().collect();
    let e = |e: std::io::Error| format!("{}: {e}", def.donor_map);
    let body_unit = unit::measure(&map, body).map_err(e)?;
    let voice_unit = unit::measure(&map, voice).map_err(e)?;
    let recipe = format!(
        "converter {CONVERTER} {VERSION}\ncharacter {id}\ninput maps/{}.map {}\nbiped {}\nbody {} tags {} bytes\nvoice {} tags {} bytes\n",
        def.donor_map,
        sha256_hex(donor),
        def.biped,
        body_unit.tags.len(),
        body_unit.total(),
        voice_unit.tags.len(),
        voice_unit.total(),
    );
    Ok(Converted {
        body: unit::cut(&map, &body_unit).map_err(e)?,
        voice: unit::cut(&map, &voice_unit).map_err(e)?,
        recipe,
        body_unit,
        voice_unit,
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
        if !wanted(&body_set) && !wanted(&voice_set) {
            continue;
        }
        let path = super::super::maps::map_path(&def.donor_map);
        let converted = mount.read(&path).map_err(|e| format!("{path}: {e}")).and_then(|b| convert(id, def, &b));
        match converted {
            Ok(c) => {
                if wanted(&body_set) {
                    out.push((body_set, vec![(body_path(id), c.body), (recipe_path(id), c.recipe.into_bytes())]));
                }
                if wanted(&voice_set) {
                    out.push((voice_set, vec![(voice_path(id), c.voice)]));
                }
            }
            Err(e) => println!("{body_set}: {e}"),
        }
    }
    out
}

/// Body `kind`'s character model from its unit on `mount` (named
/// `BodyKind::model_name`), with its textures.
pub fn load_body(kind: BodyKind, def: &BodyDef, mount: &Mount, decode: bool) -> Result<(MapCharacterModel, Vec<MapTexture>), String> {
    let path = body_path(kind.id());
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let file = MapFile::load(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let body = super::super::body::load::biped_body(&file, &def.biped, decode, def.tint()).map_err(|e| format!("{path}: {e}"))?;
    let mut character = body.character;
    character.name = kind.model_name();
    for w in &body.warnings {
        debug!("halo_ce: {}: {w}", kind.id());
    }
    Ok((character, body.textures))
}

type Loaded = Result<(MapCharacterModel, Vec<MapTexture>), String>;

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
    worn: Query<&BodyKind>,
    models: Option<Res<CharacterModels>>,
    data: Res<CampaignData>,
    context: Option<Res<LoadContext>>,
    mut pending: ResMut<UnitsOnDemand>,
    wait: Option<Res<chimera_core::tasks::WaitForTasks>>,
    mut commands: Commands,
) {
    let mut done = Vec::new();
    pending.tasks.retain(|kind, task| {
        let Some(result) = chimera_core::tasks::poll(task, wait.is_some()) else { return true };
        done.push((*kind, result));
        false
    });
    for (kind, result) in done {
        match result {
            Ok((model, textures)) => {
                info!("halo_ce: the {} body loaded ({} hit boxes)", kind.id(), model.hitboxes.len());
                pending.ready.insert(kind);
                let textures = std::sync::Arc::new(textures);
                commands.queue(move |world: &mut World| add_character_model(world, model, textures));
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
    let wanted: Vec<BodyKind> = worn.iter().copied().chain(pending.wanted.iter().copied()).collect();
    for kind in wanted {
        if kind == BodyKind::Spartan || pending.tried.contains(&kind) {
            continue;
        }
        pending.tried.insert(kind);
        let Some(def) = data.body(kind).cloned() else {
            pending.failed.insert(kind, "not in data/campaign.toml".into());
            continue;
        };
        let context = context.as_deref().cloned().unwrap_or_default();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let mount = super::super::maps::mount_for(&context)?;
            load_body(kind, &def, &mount, true)
        });
        pending.tasks.insert(kind, task);
    }
}

//! TF2's bodies that come late, built while the match plays (on the async
//! compute pool, never on a frame) and put in when ready:
//!
//! - **A class the map has no body for**: a TF2 player wears a class's
//!   model by name (`bodies::wear_bodies`), but a map carries only the
//!   bodies of the classes its match offered (`bodies::add_bodies`). A
//!   class picked later (the sandbox's target picker or `rung`, a gun
//!   handed out mid-match) has its body loaded (`load`) the first time
//!   someone wears its name, and added as a late character model
//!   (`map::add_character_model`): the wearer changes to it once it is
//!   built (`render::bodies::build_late_bodies`), wearing the stand-in
//!   (another TF2 body, `CharacterModels::for_identity`) until then. A
//!   class whose model isn't here yet (its `players/<class>` set still
//!   downloading: `Mount::held`) waits, and is tried again when one of
//!   TF2's units arrives (`chimera_net::net::units::UnitChanged`, Ready).
//! - **A body built while files of its look hadn't arrived**
//!   (`MapModel::late_files`: a lightwarp, a normal map, a Phong exponent
//!   texture the asset service hadn't brought, so its skin fell back to
//!   half-Lambert or the vertex normal): when one of TF2's units arrives,
//!   each such body whose late files are here now is built again and
//!   replaces the old one (`map::replace_character_model`).
//!
//! The files come from TF2's mount (`Tf2Files`: the install and/or the
//! asset store, `mount::open`; a test's own). Textures are decoded only
//! where something draws (`Assets<Image>`); a dedicated server builds the
//! bodies for their hitboxes, lean.
//!
//! Known shortcuts (docs/tech-debt.md): only bodies are built again; a
//! view model, held model or map prop built while a file of its look was
//! late keeps the fallback until the next map load; a body whose load
//! fails is tried once per session (its warning says why).

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};
use chimera_net::net::units::{UnitChanged, UnitState};
use source_engine::content::{material::MaterialLoader, surfaceprops::SurfaceProps};

use super::{
    GAME,
    classes::{Classes, DEFAULT},
};
use crate::{
    map::{
        BodyName, CharacterModels, LateCharacterModels, MapCharacterModel, MapTexture, REPLACED, add_character_model,
        replace_character_model,
    },
    mount::Mount,
    slots::GameIdentity,
};

/// Where the late bodies' files come from: TF2's mount, opened on the task
/// (the default: the install and/or the asset store, `mount::open`).
#[derive(Resource, Clone)]
pub struct Tf2Files(pub Arc<dyn Fn() -> Result<Mount, String> + Send + Sync>);

impl Default for Tf2Files {
    fn default() -> Self {
        Self(Arc::new(|| {
            let install = super::INFO
                .install()
                .ok_or_else(|| format!("no `{GAME}` install found (Steam, [games.tf2] path, or $CHIMERA_TF2_ROOT)"))?;
            super::mount::open(&install).map_err(|e| e.to_string())
        }))
    }
}

/// A body built on a task.
pub struct Built {
    pub model: MapCharacterModel,
    pub textures: Vec<MapTexture>,
}

/// What a task gave.
pub enum Outcome {
    /// Built: added, or in place of the old one.
    Built(Built),
    /// The model isn't here yet (its set not fetched): tried again when a
    /// unit arrives.
    NotHere,
    /// None of the late files is here yet: nothing to build again.
    Unchanged,
}

/// One body's load: what it is for.
struct Load {
    task: Task<Result<Outcome, String>>,
    replace: bool,
}

/// The bodies loading, waiting for their files and tried (module doc).
#[derive(Resource, Default)]
pub struct LateBodies {
    loads: BTreeMap<String, Load>,
    /// Models wanted whose files aren't here yet.
    waiting: BTreeSet<String>,
    /// Models loaded (or loading, or failed) as a class's body this session.
    tried: BTreeSet<String>,
    failed: BTreeMap<String, String>,
    /// How many bodies were added and replaced (tests, the log).
    pub added: usize,
    pub replaced: usize,
}

impl LateBodies {
    /// Whether `path`'s body is loading.
    pub fn loading(&self, path: &str) -> bool {
        self.loads.contains_key(path)
    }

    /// Whether `path`'s body waits for its files.
    pub fn waiting(&self, path: &str) -> bool {
        self.waiting.contains(path)
    }

    /// Bodies whose load failed, and why.
    pub fn failed(&self) -> &BTreeMap<String, String> {
        &self.failed
    }
}

/// Body `path` from `files`, named by its path (a late model is found by
/// name), its textures from 0 (`lean`: their sizes only). With `only_if`,
/// built only when one of those files (a body's late ones) is here now.
pub fn load(files: &Tf2Files, path: &str, lean: bool, only_if: Option<&[String]>) -> Result<Outcome, String> {
    let mount = (files.0)()?;
    if only_if.is_some_and(|late| !late.iter().any(|f| mount.held(f))) {
        return Ok(Outcome::Unchanged);
    }
    if !mount.held(path) {
        return Ok(Outcome::NotHere);
    }
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(lean);
    let surfaces = SurfaceProps::load(&mut materials);
    // Named whatever the class: the default class's too (`""` is no path).
    let model = super::bodies::load(&mut materials, &surfaces, path, "")?;
    materials.finish_textures();
    Ok(Outcome::Built(Built { model, textures: materials.textures }))
}

/// The model path of a TF2 body as `BodyName` names it (the default
/// class's unnamed one by its class's model).
fn path_of(m: &MapCharacterModel, default: &str) -> String {
    m.name.clone().unwrap_or_else(|| default.to_string())
}

/// Each frame: finished loads put in; on a TF2 unit's arrival the waiting
/// classes tried again and the bodies with late files built again; each
/// class body worn that the map lacks loaded (module doc).
#[allow(clippy::too_many_arguments)]
pub fn late_bodies(
    players: Query<(&GameIdentity, &BodyName)>,
    models: Option<Res<CharacterModels>>,
    late: Option<Res<LateCharacterModels>>,
    table: Option<Res<Classes>>,
    units: Option<Res<Messages<UnitChanged>>>,
    mut cursor: Local<MessageCursor<UnitChanged>>,
    files: Res<Tf2Files>,
    images: Option<Res<Assets<Image>>>,
    wait: Option<Res<chimera_core::tasks::WaitForTasks>>,
    mut state: ResMut<LateBodies>,
    mut commands: Commands,
) {
    // Finished loads.
    let mut failed = Vec::new();
    let mut waiting = Vec::new();
    let (mut added, mut replaced) = (0, 0);
    state.loads.retain(|path, load| {
        let Some(result) = chimera_core::tasks::poll(&mut load.task, wait.is_some()) else { return true };
        match result {
            Ok(Outcome::Built(Built { model, textures })) => {
                let (replace, textures) = (load.replace, Arc::new(textures));
                info!("TF2: {path}'s body {}", if replace { "built again (its late files are here)" } else { "loaded late" });
                if replace {
                    replaced += 1;
                } else {
                    added += 1;
                }
                commands.queue(move |world: &mut World| {
                    if replace {
                        replace_character_model(world, model, textures);
                    } else {
                        add_character_model(world, model, textures);
                    }
                });
            }
            Ok(Outcome::NotHere) => waiting.push(path.clone()),
            Ok(Outcome::Unchanged) => {}
            Err(e) => {
                warn!("TF2: {path}'s body didn't load: {e} (another TF2 body stays)");
                failed.push((path.clone(), e));
            }
        }
        false
    });
    state.added += added;
    state.replaced += replaced;
    state.failed.extend(failed);
    state.waiting.extend(waiting);
    let (Some(models), Some(table)) = (models, table) else { return };
    let default = table.get(DEFAULT).map(|c| c.model.clone()).unwrap_or_default();
    let lean = images.is_none();
    let spawn = |path: String, only_if: Option<Vec<String>>| {
        let files = files.clone();
        chimera_core::tasks::spawn(&AsyncComputeTaskPool::get(), async move { load(&files, &path, lean, only_if.as_deref()) })
    };
    // A TF2 unit arrived: the waiting classes again, and the bodies built
    // with files late.
    let arrived = units.is_some_and(|m| {
        cursor.read(&m).fold(false, |any, u| any | (u.game == GAME && u.status.state == UnitState::Ready))
    });
    if arrived {
        let again = std::mem::take(&mut state.waiting);
        state.tried.retain(|p| !again.contains(p));
        for m in models.0.iter().filter(|m| m.game == Some(GAME) && !m.model.late_files.is_empty()) {
            let path = path_of(m, &default);
            if path.ends_with(REPLACED) || state.loads.contains_key(&path) {
                continue;
            }
            let task = spawn(path.clone(), Some(m.model.late_files.clone()));
            state.loads.insert(path, Load { task, replace: true });
        }
    }
    // Class bodies worn that the map lacks (only on a map with TF2's
    // bodies: a late one joins those).
    if !models.0.iter().any(|m| m.game == Some(GAME)) {
        return;
    }
    let has = |n: &str| {
        models.0.iter().any(|m| m.game == Some(GAME) && m.name.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(n)))
            || (n.eq_ignore_ascii_case(&default) && models.0.iter().any(|m| m.game == Some(GAME) && m.name.is_none()))
            || late.as_ref().is_some_and(|l| l.get(n).is_some())
    };
    let worn: BTreeSet<&str> = players
        .iter()
        .filter(|(i, _)| i.0 == GAME)
        .map(|(_, n)| n.0.as_str())
        .filter(|n| table.classes.iter().any(|c| c.model.eq_ignore_ascii_case(n)))
        .collect();
    for n in worn {
        if has(n) || state.tried.contains(n) || state.loads.contains_key(n) {
            continue;
        }
        state.tried.insert(n.to_string());
        let task = spawn(n.to_string(), None);
        state.loads.insert(n.to_string(), Load { task, replace: false });
    }
}

/// The late bodies (`late_bodies`, after `bodies::wear_bodies`).
pub(super) fn add(app: &mut App) {
    app.init_resource::<LateBodies>()
        .init_resource::<Tf2Files>()
        .add_systems(Update, late_bodies.after(super::bodies::wear_bodies));
}

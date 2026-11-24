//! TF2's unlocks whose files come after the map loaded (`items`): a map's
//! dressing loads only the unlocks whose own models are here
//! (`items::on_mount_here`), so a load never fetches every unlock's set.
//! An unlock held later (the sandbox's pick, whose `weapon/<name>` set
//! the sandbox fetches as a unit) has its view models (on every class's
//! arms that are here, as the dressing makes them), held model and sound
//! entries built on the async compute pool (`load`) the first time a
//! view wants it and the map lacks it, then added (`map::add_view_model`,
//! `map::add_held_model`, its sounds into the map's `SoundBank`). An
//! unlock whose models aren't here yet waits and is tried again when one
//! of TF2's units arrives (`chimera_net::net::units::UnitChanged`, Ready).
//!
//! Known shortcuts: a view that wants an unlock a map's kit lacks is the
//! only trigger (a third-person holder alone doesn't load it); an unlock
//! loaded late is loaded again on the next map whose dressing lacks it.

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

use super::{GAME, late_bodies::Tf2Files};
use crate::map::{
    MapHeldModel, MapSounds, MapTexture, MapViewModel, ViewAnimator, ViewModels, add_held_model, add_view_model,
};

/// An unlock's looks built on a task.
pub struct Built {
    pub view_models: Vec<MapViewModel>,
    pub held: Vec<MapHeldModel>,
    pub textures: Vec<MapTexture>,
    pub sounds: MapSounds,
}

/// What a task gave.
pub enum Outcome {
    Built(Built),
    /// Its models aren't here yet (its set not fetched).
    NotHere,
}

/// The unlocks loading, waiting and tried (module doc).
#[derive(Resource, Default)]
pub struct LateItems {
    loads: BTreeMap<String, Task<Result<Outcome, String>>>,
    waiting: BTreeSet<String>,
    tried: BTreeSet<String>,
    /// How many were added (tests, the log).
    pub added: usize,
}

impl LateItems {
    /// Whether unlock `id` waits for its files.
    pub fn waiting(&self, id: &str) -> bool {
        self.waiting.contains(id)
    }
}

/// Unlock `id`'s looks from `files` (module doc), textures from 0
/// (`lean`: their sizes only).
pub fn load(files: &Tf2Files, id: &str, lean: bool) -> Result<Outcome, String> {
    let mount = (files.0)()?;
    let items = super::items::on_mount(&mount);
    let item = items
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| format!("{id}: no such unlock in the item pack"))?;
    if !item.models_here(&mount) {
        return Ok(Outcome::NotHere);
    }
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(lean);
    let surfaces = SurfaceProps::load(&mut materials);
    let (mut kit, mut warnings) = (crate::map::GameKit::default(), Vec::new());
    super::maps::add_guns_with(&mut materials, &|g| g == id, true, &items, &mut kit, &mut warnings);
    let names = item.looks.sounds.clone();
    let mut sounds =
        source_engine::content::sound::load_with(&mut materials, "", &surfaces, &[], names.iter().cloned(), true);
    super::maps::own_weapon_sounds(&mut sounds, true, &names);
    materials.finish_textures();
    for w in warnings {
        debug!("TF2 unlock {id}: {w}");
    }
    Ok(Outcome::Built(Built {
        view_models: kit.view_models,
        held: kit.held,
        textures: materials.textures,
        sounds,
    }))
}

/// The unlock a view model key is of (`tf2:winger`, `tf2:winger#soldier`),
/// if it names one in `items`.
fn unlock_of<'a>(items: &super::items::ItemsData, key: &'a str) -> Option<&'a str> {
    let id = key.split_once('#').map_or(key, |(g, _)| g);
    let name = id.strip_prefix("tf2:")?;
    items.items.get(name).filter(|d| !d.wearable).map(|_| id)
}

/// data/items.toml as `late_items` read it, and the data generation then
/// (`game_data::generation`: a dev build's reload reads it again). Parsed
/// for each view whose model the map lacked, every frame, it was 6.9 M
/// instructions a tick (docs/performance.md, "What the unlocks cost").
#[derive(Default)]
pub struct ItemsRead(Option<(u64, super::items::ItemsData)>);

impl ItemsRead {
    fn get(&mut self) -> &super::items::ItemsData {
        let now = chimera_core::game_data::generation();
        if !self.0.as_ref().is_some_and(|(at, _)| *at == now) {
            self.0 = Some((now, super::items::ItemsData::builtin()));
        }
        &self.0.as_ref().expect("read above").1
    }
}

/// Each frame: finished loads put in; on a TF2 unit's arrival the waiting
/// unlocks tried again; each unlock a view wants that the map lacks
/// loaded (module doc).
#[allow(clippy::too_many_arguments)]
pub fn late_items(
    views: Query<&ViewAnimator>,
    models: Option<Res<ViewModels>>,
    units: Option<Res<Messages<UnitChanged>>>,
    mut cursor: Local<MessageCursor<UnitChanged>>,
    files: Res<Tf2Files>,
    images: Option<Res<Assets<Image>>>,
    wait: Option<Res<chimera_core::tasks::WaitForTasks>>,
    mut state: ResMut<LateItems>,
    mut items: Local<ItemsRead>,
    mut commands: Commands,
) {
    let mut waiting = Vec::new();
    let mut added = 0;
    state.loads.retain(|id, task| {
        let Some(result) = chimera_core::tasks::poll(task, wait.is_some()) else {
            return true;
        };
        match result {
            Ok(Outcome::Built(Built {
                view_models,
                held,
                textures,
                sounds,
            })) => {
                info!(
                    "TF2: {id}'s looks loaded late ({} view models, {} held)",
                    view_models.len(),
                    held.len()
                );
                added += 1;
                let textures = Arc::new(textures);
                commands.queue(move |world: &mut World| {
                    for v in view_models {
                        add_view_model(world, v, textures.clone());
                    }
                    for h in held {
                        add_held_model(world, h, textures.clone());
                    }
                    if let Some(mut bank) = world.get_resource_mut::<crate::map::sound::SoundBank>() {
                        source_engine::content::bsp::add_guest_sounds(Arc::make_mut(&mut bank.0), sounds);
                    }
                });
            }
            Ok(Outcome::NotHere) => waiting.push(id.clone()),
            Err(e) => warn!("TF2: {id}'s looks didn't load: {e} (the stand-in stays)"),
        }
        false
    });
    state.added += added;
    state.waiting.extend(waiting);
    let arrived = units.is_some_and(|m| {
        cursor.read(&m).fold(false, |any, u| {
            any | (u.game == GAME && u.status.state == UnitState::Ready)
        })
    });
    if arrived {
        let again = std::mem::take(&mut state.waiting);
        state.tried.retain(|i| !again.contains(i));
    }
    let Some(models) = models else { return };
    let mut missing = views.iter().filter_map(|v| v.key.as_deref()).filter(|k| models.get(k).is_none()).peekable();
    if missing.peek().is_none() {
        return;
    }
    let items = items.get();
    let wanted: BTreeSet<String> = missing
        .filter_map(|k| unlock_of(items, k))
        .map(str::to_string)
        .collect();
    let lean = images.is_none();
    for id in wanted {
        if state.tried.contains(&id) || state.loads.contains_key(&id) {
            continue;
        }
        state.tried.insert(id.clone());
        let (files, key) = (files.clone(), id.clone());
        let task = chimera_core::tasks::spawn(&AsyncComputeTaskPool::get(), async move { load(&files, &key, lean) });
        state.loads.insert(id, task);
    }
}

/// The late unlocks (`late_items`).
pub(super) fn add(app: &mut App) {
    app.init_resource::<LateItems>()
        .init_resource::<Tf2Files>()
        .add_systems(Update, late_items);
}

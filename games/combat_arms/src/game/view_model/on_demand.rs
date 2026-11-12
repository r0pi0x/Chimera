//! View models loaded on demand (docs/performance.md, "Combat Arms map
//! loads"): a map loads without them (`world::load_from`), and a weapon's
//! view model loads in the background (`AsyncComputeTaskPool`) the first
//! time a weapon wants drawing: one in the local player's inventory or
//! shown by a character's `ViewAnimator`. Converted once per install
//! data (`cached`: the app's `CaData::install_for`'s `memo`, kept across
//! map changes and freed with the data), each with its own textures
//! (`map::add_view_model`; drawn on a map and on the greybox alike, where
//! Chimera's drawing has assets of its own: `render::view_model::
//! assets_without_map`, `tests/ui/heavy/view_model.rs`); until it is
//! ready the front end's placeholder box stands in (nothing in an app with
//! `game_parts::NoStandIns`), and `drive` starts the model afresh when it
//! arrives. The match's ladder's view models load the same way ahead of
//! time (`characters::preload`).
//!
//! A weapon with drawn parts shows its own view model (`parts::view_key`:
//! `<weapon>+<part>+...`), made here from the weapon's with the parts'
//! models on it (`load_with_parts`); the bare gun's stands in meanwhile.
//! A local player whose look's hands aren't the default character's
//! (`hands::key_for`: a ladder rung's body) gets each of his weapons'
//! again with his hands (`<key>#h<model>_<face>_<uniform>`), the default
//! one standing in meanwhile.
//!
//! Only Combat Arms' own weapons load here (`ours`): another game's gun in
//! a mashup (CS:S's in Lucker Party) has its own game's view model, which
//! that game's loader or Chimera's driver draws.
//!
//! Runs only where `LoadOnDemand` is (the front end inserts it,
//! `load_on_demand_in`): a dedicated server draws no view models. Where it
//! runs, the dressing of another game's map leaves the view models out
//! too (`ViewModelsOnDemand`): they load here, once per session, not again
//! with each map.
//!
//! Known shortcuts (docs/tech-debt.md): outside a ladder the placeholder
//! box shows for the frames a model takes to load (tens of milliseconds;
//! the first draw of each weapon); the cache keeps every model while the
//! install data lives (the app's session); outside a ladder nothing
//! preloads a weapon before it is in the local player's inventory or
//! shown (a spectated player's new weapon waits its load too).

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};

use super::{
    super::{
        attachments::CaMods,
        data::{CaData, CaDataInner},
    },
    hands, load_hands, load_sharing, parts,
};
use super::super::characters::records::Selection;
use crate::{
    core::LocalPlayer,
    map::{LoadContext, MapData, MapTexture, MapViewModel, ViewAnimator, ViewModels, add_view_model},
    weapon::{Inventory, Weapon},
};

/// Load view models when they are wanted (the front end's).
#[derive(Resource, Default)]
pub struct LoadOnDemand;

/// A memo of an app's `map::LoadContext`: its view models load on demand
/// (`LoadOnDemand`), so the dressing of another game's map carries none
/// (`world::DRESSING`), as Combat Arms' own maps never do: the ladder's
/// load ahead (`characters::preload`), anything else when first wanted.
/// The dressing converted all of ours again for each map, beside the
/// ones loaded here (docs/performance.md, "A client's memory").
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewModelsOnDemand;

/// `LoadOnDemand` for `app`, its map loads told so (`ViewModelsOnDemand`).
pub fn load_on_demand_in(app: &mut App) {
    app.init_resource::<LoadOnDemand>();
    app.world_mut().get_resource_or_init::<LoadContext>().insert(ViewModelsOnDemand);
}

/// A front end's map loads' context readied before its app builds
/// (`chimera_client`'s `GameAppHooks::load_context`: the start map's):
/// its view models load on demand (`ViewModelsOnDemand`), and the install
/// data its loads read is the app's (`CaData::install`, held by the
/// context: the start map's dressing and the app share one, its parsed
/// animation files and converted models, rather than reading them twice
/// while the first map loads).
pub fn front_end_context(context: &LoadContext) {
    context.insert(ViewModelsOnDemand);
    context.get_or_init(CaData::install);
}

/// Whether the app of load `context` loads its view models on demand
/// (`load_on_demand_in`).
pub fn on_demand_in(context: &LoadContext) -> bool {
    context.get::<ViewModelsOnDemand>().is_some()
}

/// A weapon's view model as loaded: its textures, which its meshes'
/// indices name.
pub struct LoadedViewModel {
    pub model: Arc<MapViewModel>,
    pub textures: Arc<Vec<MapTexture>>,
    /// What didn't load with it (the hands): the map's warnings would
    /// have said.
    pub warnings: Vec<String>,
}

type Loaded = Result<Arc<LoadedViewModel>, String>;

/// The view models install data `ca` loaded (or failed to), by key.
#[derive(Default)]
struct Cache(Mutex<BTreeMap<String, Loaded>>);

fn cache(ca: &CaDataInner) -> Arc<Cache> {
    ca.memo(Cache::default)
}

/// Weapon `id`'s view model if install data `ca` loaded it (or failed
/// to).
pub fn cached(ca: &CaDataInner, id: &str) -> Option<Loaded> {
    cache(ca).0.lock().ok()?.get(id).cloned()
}

/// Forget the view models not in `keep` (`characters::preload::
/// forget_old`), and a load that failed: they load again when next wanted.
pub fn forget(ca: &CaDataInner, keep: &std::collections::BTreeSet<String>) {
    if let Ok(mut c) = cache(ca).0.lock() {
        c.retain(|k, _| keep.contains(k));
    }
}

/// Weapon `id`'s view model from install data `ca`'s install through
/// `file` (`load`, with the hands its key names when they read: the
/// default character's without a hands part, `hands::key_for`), with its
/// own textures; Err when its data names none or it doesn't read.
pub fn load_weapon(ca: &CaDataInner, id: &str, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<LoadedViewModel, String> {
    // A weapon with fitted parts (`parts::view_key`): its own view model,
    // the parts on it (the bare gun's with the same hands under them).
    let body = hands::split_key(id).0;
    let (base, names) = parts::split_key(body);
    if !names.is_empty() {
        let bare = format!("{base}{}", &id[body.len()..]);
        return load_with_parts(ca, id, &bare, file);
    }
    let w = ca
        .weapon_set()
        .weapons
        .iter()
        .find(|w| w.id == base)
        .ok_or_else(|| format!("{id}: no such weapon"))?;
    let path = w.models.view.as_deref().ok_or_else(|| format!("{id}: no view model in its data"))?;
    let offset = w.models.view_offset.unwrap_or_default();
    // Its textures: indices into this map's list.
    let mut own = MapData::default();
    // Kept for later maps: its textures as their blocks.
    own.memos.insert(super::super::world::BlocksOnly);
    let characters = ca.characters();
    let choice = hands::choice_for_key(characters, id);
    let mut warnings = Vec::new();
    let hand = match choice.as_ref().map(|c| load_hands(c, &mut own, file)) {
        Some(Ok(h)) => Some(h),
        Some(Err(e)) => {
            warnings.push(format!("view-model hands: {e}"));
            None
        }
        None => None,
    };
    let mut v =
        load_sharing(path, &w.id, &w.models.view_skins, offset, &mut own, file, &ca.memo(super::ViewAnimations::default))
            .map_err(|e| format!("{id} view model: {e}"))?;
    // Its own key (the hands part with it).
    v.key = id.to_string();
    if let Some((meshes, bones)) = hand {
        let at = Vec3::from(lithtech_formats::ltb::to_rh_vec(offset)) * super::VIEW_SCALE;
        if let Err(e) = hands::attach(&mut v, meshes, &bones, super::VIEW_SCALE, at) {
            warnings.push(format!("{id} view-model hands: {e}"));
        }
    }
    Ok(LoadedViewModel {
        model: Arc::new(v),
        textures: Arc::new(own.textures),
        warnings,
    })
}

/// View-model key `key` (`parts::view_key`): weapon `bare`'s view model
/// (`load_cached`, its key with the same hands) with its parts' models on
/// it, its textures with theirs.
fn load_with_parts(ca: &CaDataInner, key: &str, bare: &str, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<LoadedViewModel, String> {
    let base = parts::split_key(bare).0;
    let bare = load_cached(ca, bare, file)?;
    let params = ca
        .weapon_set()
        .get(base)
        .and_then(|w| w.hook::<super::super::attachments::AttachParams>(super::super::attachments::ATTACHMENTS))
        .unwrap_or_default();
    let mut model = (*bare.model).clone();
    model.key = key.to_string();
    let mut own = MapData {
        textures: (*bare.textures).clone(),
        ..default()
    };
    own.memos.insert(super::super::world::BlocksOnly);
    let mut warnings = bare.warnings.clone();
    for part in parts::parts_of(key, ca.attachments(), &params) {
        if let Err(e) = parts::attach(&mut model, &part, &mut own, file) {
            warnings.push(format!("{base} view-model part: {e}"));
        }
    }
    Ok(LoadedViewModel {
        model: Arc::new(model),
        textures: Arc::new(own.textures),
        warnings,
    })
}

/// `load_weapon` once per install data: the cached result after the
/// first.
pub fn load_cached(ca: &CaDataInner, id: &str, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Loaded {
    if let Some(l) = cached(ca, id) {
        return l;
    }
    let loaded = load_weapon(ca, id, file).map(Arc::new);
    if let Ok(mut c) = cache(ca).0.lock() {
        c.insert(id.to_string(), loaded.clone());
    }
    loaded
}

/// Whether weapon `id` is Combat Arms' (an id of another game, `game:name`,
/// is not).
pub fn ours(id: &str) -> bool {
    id.split_once(':').is_none_or(|(game, _)| game == crate::game::GAME)
}

/// The view models loading.
#[derive(Default)]
pub struct Pending(BTreeMap<String, Task<Loaded>>);

/// Each wanted weapon's view model (`LoadOnDemand`): from the app's
/// install data's cache at once, else loaded in the background and added
/// when done.
pub fn load_on_demand(
    models: Option<Res<ViewModels>>,
    local: Query<(&Inventory, Option<&Selection>), With<LocalPlayer>>,
    shown: Query<&ViewAnimator>,
    weapons: Query<(&Weapon, Option<&CaMods>)>,
    context: Option<Res<LoadContext>>,
    ladder: Res<crate::game::characters::preload::LadderModels>,
    mut pending: Local<Pending>,
    wait: Option<Res<chimera_core::tasks::WaitForTasks>>,
    mut commands: Commands,
) {
    let ca = context.as_deref().map_or_else(CaData::install, CaData::install_for);
    let has = |k: &str| models.as_ref().is_some_and(|m| m.get(k).is_some());
    // Finished loads first.
    pending.0.retain(|id, task| {
        let Some(result) = chimera_core::tasks::poll(task, wait.is_some()) else { return true };
        match &result {
            Ok(l) => {
                for w in &l.warnings {
                    warn!("Combat Arms: {w}");
                }
            }
            Err(e) => warn!("Combat Arms: {e} (the placeholder stands in)"),
        }
        if let Ok(mut c) = cache(&ca).0.lock() {
            c.insert(id.clone(), result);
        }
        false
    });
    // The active weapon first, then the rest of the inventory, then what
    // characters show (a spectated player's).
    // A weapon with drawn parts: its bare view model first (the parts'
    // is made from it), then the one with its parts (`parts::view_key`).
    // The local player's worn look's hands (`hands::key_for`): each of
    // those again with them, after the default character's.
    let mut wanted: Vec<String> = Vec::new();
    for (inv, look) in &local {
        let hand = look.and_then(|l| hands::key_for(ca.characters(), l));
        for (w, mods) in inv.active.iter().chain(&inv.weapons).filter_map(|e| weapons.get(*e).ok()) {
            let (bare, with_parts) = (w.id.to_string(), parts::view_key(w.id, mods));
            if let Some(h) = hand.as_deref() {
                wanted.push(hands::with_hands(&bare, Some(h)));
                wanted.push(hands::with_hands(&with_parts, Some(h)));
            }
            wanted.push(bare);
            wanted.push(with_parts);
        }
    }
    wanted.extend(shown.iter().filter_map(|v| v.key.clone()));
    // Those wanted now, then what the ladder preloads (first in `wanted` wins).
    let wanted_now = wanted.len();
    wanted.extend(ladder.keys.iter().cloned());
    let mut seen: Vec<&str> = Vec::new();
    for (at, id) in wanted.iter().enumerate() {
        let id = id.as_str();
        if !ours(id) || seen.contains(&id) || has(id) || pending.0.contains_key(id) {
            continue;
        }
        seen.push(id);
        match cached(&ca, id) {
            // In use: added, the first one making `ViewModels` where no map
            // did (the greybox). A ladder's preload with no map yet stays in
            // the cache for the map coming.
            Some(Ok(l)) if models.is_some() || at < wanted_now => {
                let (model, textures) = (l.model.clone(), l.textures.clone());
                commands.queue(move |world: &mut World| add_view_model(world, model, textures));
            }
            Some(Ok(_)) => {}
            Some(Err(_)) => {}
            None => {
                let (key, ca) = (id.to_string(), ca.clone());
                let task = AsyncComputeTaskPool::get().spawn(async move {
                    let mount = ca.mount().ok_or_else(|| format!("{key}: no Combat Arms install"))?;
                    load_weapon(&ca, &key, &|p: &str| mount.read(p).ok()).map(Arc::new)
                });
                pending.0.insert(id.to_string(), task);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn another_games_weapons_are_not_loaded_here() {
        assert!(super::ours("combat_arms:m4a1"));
        assert!(!super::ours("cs_source:weapon_deagle"));
    }
}

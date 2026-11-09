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
//!
//! Only Combat Arms' own weapons load here (`ours`): another game's gun in
//! a mashup (CS:S's in Lucker Party) has its own game's view model, which
//! that game's loader or Chimera's driver draws.
//!
//! Runs only where `LoadOnDemand` is (the front end inserts it): a
//! dedicated server draws no view models.
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
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};

use super::{
    super::{
        attachments::CaMods,
        data::{CaData, CaDataInner},
    },
    hands, load, load_hands, parts,
};
use crate::{
    core::LocalPlayer,
    map::{LoadContext, MapData, MapTexture, MapViewModel, ViewAnimator, ViewModels, add_view_model},
    weapon::{Inventory, Weapon},
};

/// Load view models when they are wanted (the front end's).
#[derive(Resource, Default)]
pub struct LoadOnDemand;

/// A weapon's view model as loaded: its textures, which its meshes'
/// indices name.
pub struct LoadedViewModel {
    pub model: MapViewModel,
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

/// Weapon `id`'s view model from install data `ca`'s install through
/// `file` (`load`, with the default character's hands when they read),
/// with its own textures; Err when its data names none or it doesn't
/// read.
pub fn load_weapon(ca: &CaDataInner, id: &str, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<LoadedViewModel, String> {
    // A weapon with fitted parts (`parts::view_key`): its own view model,
    // the parts on it.
    let (base, names) = parts::split_key(id);
    if !names.is_empty() {
        return load_with_parts(ca, id, base, file);
    }
    let w = ca
        .weapon_set()
        .weapons
        .iter()
        .find(|w| w.id == id)
        .ok_or_else(|| format!("{id}: no such weapon"))?;
    let path = w.models.view.as_deref().ok_or_else(|| format!("{id}: no view model in its data"))?;
    let offset = w.models.view_offset.unwrap_or_default();
    // Its textures: indices into this map's list.
    let mut own = MapData::default();
    let choice = hands::choose(&hands::builtin(), &hands::HandProfile {
        character: true,
        ..default()
    });
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
        load(path, &w.id, &w.models.view_skins, offset, &mut own, file).map_err(|e| format!("{id} view model: {e}"))?;
    if let Some((meshes, bones)) = hand {
        let at = Vec3::from(lithtech_formats::ltb::to_rh_vec(offset)) * super::VIEW_SCALE;
        if let Err(e) = hands::attach(&mut v, meshes, &bones, super::VIEW_SCALE, at) {
            warnings.push(format!("{id} view-model hands: {e}"));
        }
    }
    Ok(LoadedViewModel {
        model: v,
        textures: Arc::new(own.textures),
        warnings,
    })
}

/// View-model key `key` (`parts::view_key`): weapon `base`'s view model
/// (`load_cached`) with its parts' models on it, its textures with
/// theirs.
fn load_with_parts(ca: &CaDataInner, key: &str, base: &str, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<LoadedViewModel, String> {
    let bare = load_cached(ca, base, file)?;
    let params = ca
        .weapon_set()
        .get(base)
        .and_then(|w| w.hook::<super::super::attachments::AttachParams>(super::super::attachments::ATTACHMENTS))
        .unwrap_or_default();
    let mut model = bare.model.clone();
    model.key = key.to_string();
    let mut own = MapData {
        textures: (*bare.textures).clone(),
        ..default()
    };
    let mut warnings = bare.warnings.clone();
    for part in parts::parts_of(key, ca.attachments(), &params) {
        if let Err(e) = parts::attach(&mut model, &part, &mut own, file) {
            warnings.push(format!("{base} view-model part: {e}"));
        }
    }
    Ok(LoadedViewModel {
        model,
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
    local: Query<&Inventory, With<LocalPlayer>>,
    shown: Query<&ViewAnimator>,
    weapons: Query<(&Weapon, Option<&CaMods>)>,
    context: Option<Res<LoadContext>>,
    ladder: Res<crate::game::characters::preload::LadderModels>,
    mut pending: Local<Pending>,
    mut commands: Commands,
) {
    let ca = context.as_deref().map_or_else(CaData::install, CaData::install_for);
    let has = |k: &str| models.as_ref().is_some_and(|m| m.get(k).is_some());
    // Finished loads first.
    pending.0.retain(|id, task| {
        let Some(result) = block_on(poll_once(task)) else { return true };
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
    let mut wanted: Vec<String> = Vec::new();
    for inv in &local {
        for (w, mods) in inv.active.iter().chain(&inv.weapons).filter_map(|e| weapons.get(*e).ok()) {
            wanted.push(w.id.to_string());
            wanted.push(parts::view_key(w.id, mods));
        }
    }
    wanted.extend(shown.iter().filter_map(|v| v.key.clone()));
    wanted.extend(ladder.keys.iter().cloned());
    let mut seen: Vec<&str> = Vec::new();
    for id in &wanted {
        let id = id.as_str();
        if !ours(id) || seen.contains(&id) || has(id) || pending.0.contains_key(id) {
            continue;
        }
        seen.push(id);
        match cached(&ca, id) {
            // No map yet: it stays in the cache for the map coming.
            Some(Ok(l)) if models.is_some() => {
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

//! Third-person (held) models loaded on demand: a map carries the held
//! models of our own weapon documents only (`builtin_weapon_ids`, its
//! kit: `models::add_character_models`; a map of another game only when
//! its kit loads ours), so a weapon the install adds (some 2,000;
//! Ultimate Gun Game's rungs and the party apps give them), or one of
//! ours the map lacks, loads its held model in the background
//! (`AsyncComputeTaskPool`) the
//! first time a character holds it or it lies loose, converted once per
//! session (`models::load_held_weapon`, kept across map changes in
//! `map::LateHeldModels`, which the drawing builds on each map:
//! `render::bodies::build_late_held`). Until it is ready the character's
//! hand is empty and a dropped one waits undrawn. A held weapon with
//! drawn parts (`characters::WantHeld`) loads its model with them the
//! same way; the bare gun's is held meanwhile. The match's ladder's held
//! models load the same way ahead of time (`preload`).
//!
//! An older ladder's are forgotten at a map's unload
//! (`preload::forget_old`) and load again if held again.
//!
//! Only Combat Arms' own weapons load here (`view_model::on_demand::ours`);
//! runs only where `view_model::on_demand::LoadOnDemand` is (the front
//! end: a dedicated server draws none).
//!
//! Known shortcuts (docs/tech-debt.md): a dropped install weapon's
//! physics box is the 0.2 m default when it drops before its model has
//! loaded (`map::loose::attach_loose` sizes it once); a weapon whose model
//! doesn't load is tried once per session (its warning says why).

use std::collections::{BTreeMap, BTreeSet};

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};

use super::{
    super::{data::CaData, view_model::on_demand::ours},
    models::{self, OnDemand, load_held_weapon},
};
use crate::map::{
    Held, LateHeldModels, LoadContext, MapHeldModel, add_held_model,
    loose::{LooseAssets, LooseItem},
};

type Loaded = Result<OnDemand<MapHeldModel>, String>;

/// The held models loading, and the weapons tried (loaded or not).
#[derive(Default)]
pub struct Pending {
    tasks: BTreeMap<String, Task<Loaded>>,
    tried: BTreeSet<String>,
}

/// Whether weapon `id`'s held model loads here: one of ours whose held
/// model the map has (`has`: the held models' loose forms,
/// `map::loose::LooseAssets`, the kit's and those added) is not there.
pub fn wanted(id: &str, has: impl Fn(&str) -> bool) -> bool {
    ours(id) && id.contains(':') && !has(id)
}

/// Each held or loose install weapon's held model: loaded in the
/// background once, added when done.
pub fn load_on_demand(
    held: Query<&Held>,
    want: Query<&super::WantHeld>,
    loose: Query<&LooseItem>,
    late: Option<Res<LateHeldModels>>,
    assets: Option<Res<LooseAssets>>,
    context: Option<Res<LoadContext>>,
    ladder: Res<super::preload::LadderModels>,
    forgotten: Option<Res<super::preload::Forgotten>>,
    mut pending: Local<Pending>,
    mut commands: Commands,
) {
    if let Some(f) = forgotten.filter(|f| f.is_changed()) {
        pending.tried.retain(|id| !f.keys.contains(id));
    }
    pending.tasks.retain(|_, task| {
        let Some(result) = block_on(poll_once(task)) else { return true };
        match result {
            Ok(OnDemand { model, textures, index }) => {
                let textures = std::sync::Arc::new(textures);
                commands.queue(move |world: &mut World| {
                    models::late_in(world).0.merge(&index);
                    add_held_model(world, model, textures);
                });
            }
            Err(e) => warn!("Combat Arms: {e} (the hand stays empty)"),
        }
        false
    });
    // The map's kit's held models, ours included where it carries them
    // (none on a map whose kit wasn't loaded), and those added.
    let has = |id: &str| assets.as_ref().is_some_and(|a| a.0.contains_key(id)) || late.as_ref().is_some_and(|l| l.has(id));
    // A ladder's, before the map is in: not those its kit will carry.
    let ca = context.as_deref().map_or_else(CaData::install, CaData::install_for);
    let kit = ca.builtin_weapon_ids();
    let keys = held
        .iter()
        .filter_map(|h| h.0.as_deref())
        .chain(want.iter().filter_map(|w| w.0.as_deref()))
        .chain(loose.iter().map(|l| l.0.as_str()))
        .chain(ladder.keys.iter().map(String::as_str).filter(|k| assets.is_some() || !kit.contains(*k)));
    for id in keys {
        if pending.tried.contains(id) || !wanted(id, has) {
            continue;
        }
        pending.tried.insert(id.to_string());
        let key = id.to_string();
        let ca = ca.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let mount = ca.mount().ok_or_else(|| format!("{key}: no Combat Arms install"))?;
            load_held_weapon(&ca, &key, &|p: &str| mount.read(p).ok())
        });
        pending.tasks.insert(id.to_string(), task);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_our_weapons_the_map_lacks_load_here() {
        let has = |id: &str| id == "combat_arms:m4a1";
        assert!(super::wanted("combat_arms:prismatic_eco_m416", has));
        assert!(!super::wanted("combat_arms:m4a1", has));
        assert!(!super::wanted("cs_source:weapon_ak47", has));
    }
}

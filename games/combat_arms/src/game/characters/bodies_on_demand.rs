//! Character bodies loaded on demand: a map carries the stock bodies'
//! groups only (`models::LOADED`), so a body record no group has (an
//! install specialist or mercenary a ladder rung dresses its players in,
//! `game::rungs`) loads in the background (`AsyncComputeTaskPool`) the
//! first time a character's selection wears it, converted once while
//! wanted as its own character model (`models::load_body`: its rig and
//! group registered), kept across map changes in
//! `map::LateCharacterModels` (`map::add_character_model`), which each
//! map's `CharacterModels` takes (`map::append_late_characters`) and the
//! drawing builds (`render::bodies::build_late_bodies`). Until it is
//! ready the character wears what it wore (its previous body, or none).
//!
//! The match's ladder's bodies load the same way ahead of time
//! (`preload`), with or without a map; a body worn is first.
//!
//! Only where `view_model::on_demand::LoadOnDemand` is (the front end: a
//! dedicated server draws none; its hit spheres pose on the stock rig,
//! `models::alias_rig`), and only on a map that has bodies of ours.
//!
//! What it loads and what failed is `BodiesOnDemand` (a resource: a wait
//! for a ladder's bodies reads it, `preload::ladder_bodies_waiting`).
//!
//! An older ladder's bodies are forgotten at a map's unload
//! (`preload::forget_old`) and load again if worn again.
//!
//! Known shortcuts (docs/tech-debt.md): a body that doesn't load is tried
//! once per session (its warning says why); a specialist loads its own
//! copy of its sex's animation library (a few megabytes each); a body
//! whose file the install doesn't have (two on the dev box's install,
//! `heavy::body_skins`) stays in the rungs' pool and its players keep
//! what they wore.

use std::collections::{BTreeMap, BTreeSet};

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};

use super::{
    super::{GAME, data::CaData},
    models,
    records::Selection,
};
use crate::map::{CharacterModels, LoadContext, MapCharacterModel, add_character_model};

type Loaded = Result<models::OnDemand<MapCharacterModel>, String>;

/// The bodies loading, the ones tried (loaded or not), the ones a stock
/// group has and the ones that didn't load (and why): what a wait for a
/// ladder's bodies reads (`preload::ladder_bodies_waiting`).
#[derive(Resource, Default)]
pub struct BodiesOnDemand {
    tasks: BTreeMap<i32, Task<Loaded>>,
    tried: BTreeSet<i32>,
    stock: BTreeSet<i32>,
    failed: BTreeMap<i32, String>,
}

impl BodiesOnDemand {
    /// Whether body record `id` is loading.
    pub fn loading(&self, id: i32) -> bool {
        self.tasks.contains_key(&id)
    }

    /// The bodies whose load failed this session, and why (each tried
    /// once: module doc).
    pub fn failed(&self) -> &BTreeMap<i32, String> {
        &self.failed
    }
}

/// Each worn body record no loaded group has: loaded in the background
/// once, added when done.
pub fn load_on_demand(
    looks: Query<&Selection>,
    models: Option<Res<CharacterModels>>,
    data: Res<CaData>,
    loaded: models::LoadedModels,
    context: Option<Res<LoadContext>>,
    ladder: Res<super::preload::LadderModels>,
    forgotten: Option<Res<super::preload::Forgotten>>,
    mut pending: ResMut<BodiesOnDemand>,
    wait: Option<Res<chimera_core::tasks::WaitForTasks>>,
    mut commands: Commands,
) {
    if let Some(f) = forgotten.filter(|f| f.is_changed()) {
        pending.tried.retain(|id| !f.bodies.contains(id));
        pending.failed.retain(|id, _| !f.bodies.contains(id));
    }
    let mut failed = Vec::new();
    pending.tasks.retain(|id, task| {
        let Some(result) = chimera_core::tasks::poll(task, wait.is_some()) else { return true };
        match result {
            Ok(models::OnDemand { model, textures, index }) => {
                let textures = std::sync::Arc::new(textures);
                commands.queue(move |world: &mut World| {
                    models::late_in(world).0.merge(&index);
                    add_character_model(world, model, textures);
                });
            }
            Err(e) => {
                warn!("Combat Arms: {e} (the stock body stays)");
                failed.push((*id, e));
            }
        }
        false
    });
    pending.failed.extend(failed);
    let chars = data.characters();
    // A map that wears none of ours (no bodies loaded, or another game's
    // kit) loads none for those worn; the ladder's are for the map coming.
    let ours = models.is_some_and(|m| m.0.iter().any(|m| m.game == Some(GAME)));
    let worn = looks.iter().filter(|_| ours).map(|look| chars.effective(look).0.id);
    let ladder = ladder.bodies.iter().copied().filter(|id| !models::LOADED.contains(id));
    let wanted: Vec<i32> = worn.chain(ladder).collect();
    for id in wanted {
        if pending.tried.contains(&id) || pending.stock.contains(&id) {
            continue;
        }
        if loaded.get().group_has(id) {
            pending.stock.insert(id);
            continue;
        }
        pending.tried.insert(id);
        let ca = context.as_deref().map_or_else(CaData::install, CaData::install_for);
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let mount = ca.mount().ok_or_else(|| format!("body record {id}: no Combat Arms install"))?;
            models::load_body(&ca, id, &|p: &str| mount.read(p).ok())
        });
        pending.tasks.insert(id, task);
    }
}

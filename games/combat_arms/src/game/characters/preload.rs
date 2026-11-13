//! The match's ladder loaded ahead (backlog "changing Combat Arms
//! characters hitches and garbles"): when a `ladder::Ladder` is made or a
//! client takes the server's, its rungs' Combat Arms bodies, held and view
//! models (the fitted parts' too: `plan`) are what `bodies_on_demand`,
//! `held_on_demand` and `view_model::on_demand` load next, in the
//! background, from this machine's own install, on host and client alike.
//! Only the ladder's: anything else still loads the first time it is worn
//! or held. The weapons' timing, read from the same view models, is read ahead
//! beside them (`weapons::timing::read_ahead`). CS:S's and HL2:DM's rungs need nothing here: their maps' kits
//! carry all their bodies, held and view models (`map::sources`).
//!
//! A ladder is made as its match starts, once the map is in (a client's
//! comes with the match after that): so the load runs in the first seconds
//! of play, off the main thread, not behind the loading screen, and the swap
//! holds (`setup`, `pose_bodies`) if a rung is reached sooner.
//!
//! Only where `view_model::on_demand::LoadOnDemand` is (the front end).
//! `ladder_bodies_waiting` and `ladder_keys_waiting`: the ladder's bodies,
//! held and view models not in yet (a stock body is in where the map's
//! group has it, a kit's held model where the map carries it; one whose
//! load failed is not waited for), what a test waits on
//! (`harness::ladder_models_in`).
//!
//! What older ladders brought is forgotten as a map unloads (`forget_old`,
//! a `map::MapHooks::unload` hook): everything on demand (`map::
//! LateCharacterModels`, `LateHeldModels`, the view models' cache) but the
//! ladder just played and what characters wear and hold, so a session holds
//! at most the ladder played and the next (each about 1.6 GB of memory, and
//! each map builds what is kept: docs/performance.md, "A session's frame
//! rate"); anything forgotten loads again when next wanted (`Forgotten`).
//!
//! Known shortcuts (docs/tech-debt.md): a player's own look that no rung
//! gives still loads when first worn; a forgotten body's rig and a held
//! model's sockets stay in the app's index (`CaLateModels`; at most one rig
//! per body record; forgetting the rigs too didn't move the process's
//! memory, docs/performance.md, "A session's frame rate").

use std::collections::BTreeSet;

use bevy::prelude::*;
use chimera_sim::minigame::{
    ActiveMinigame,
    ladder::Ladder,
};

use super::{
    super::{
        attachments::{ATTACHMENTS, AttachParams, CaModTable, fit, mods_for},
        data::CaData,
        rungs,
        view_model::{
            on_demand::{self, LoadOnDemand, ours},
            parts::view_key,
        },
    },
    WantHeld, models,
    records::{CharacterData, Selection},
};
use crate::{
    map::{Held, LateCharacterModels, LateHeldModels, LoadContext},
    weapon::data::WeaponCatalog,
};

/// What `forget_old` last dropped: the loaders may load it again (they
/// tried it once, `Pending`).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Forgotten {
    pub bodies: Vec<i32>,
    pub keys: Vec<String>,
}

/// What this ladder's rungs give that loads on demand: body records and
/// the keys of held and view models (a weapon's, and its own with parts).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct LadderModels {
    pub bodies: Vec<i32>,
    pub keys: Vec<String>,
}

fn push<T: PartialEq>(list: &mut Vec<T>, item: T) {
    if !list.contains(&item) {
        list.push(item);
    }
}

/// The body of a rung's extras (`rungs::MODEL`), as `rungs::apply` wears
/// it.
fn body_of(data: &CharacterData, x: &chimera_sim::minigame::ladder::RungExtras) -> Option<i32> {
    let id = x.value(rungs::MODEL)?.parse::<i32>().ok()?;
    let look = match x.value(rungs::INFECTED_VALUE) {
        Some(_) => data.infected_look(id),
        None => data.default_look(id),
    };
    Some(data.effective(&look).0.id)
}

/// The bodies `ladder`'s rungs give (`body_of`), each once, in rung
/// order: what `plan` loads, and the sets the asset offer names
/// (`rungs::body_set`).
pub fn ladder_bodies(data: &CharacterData, ladder: &Ladder) -> Vec<i32> {
    let mut out = Vec::new();
    for x in ladder.extras.iter().flatten() {
        if let Some(id) = body_of(data, x) {
            push(&mut out, id);
        }
    }
    out
}

/// The view (and held) model key of `weapon` fitted with `parts`
/// (`attachments::fit_given`, `parts::view_key`).
fn key_with_parts(catalog: &WeaponCatalog, table: &CaModTable, weapon: &str, parts: &[&str]) -> Option<String> {
    let params = catalog.get(weapon)?.hook::<AttachParams>(ATTACHMENTS)?;
    Some(fitted_key(table, &params, weapon, parts))
}

/// The view (and held) model key of `weapon`, of `params`, fitted with
/// `parts` (its default parts in the slots they leave): `key_with_parts`,
/// also what `chimera_pack sets` loads for a gun's `parts/<id>` set
/// (`rungs::parts_set`).
pub fn fitted_key(table: &CaModTable, params: &AttachParams, weapon: &str, parts: &[&str]) -> String {
    let fitted = fit(table, params, parts).map(|p| p.map(|(id, ..)| id));
    view_key(weapon, Some(&mods_for(table, params, &fitted)))
}

/// What `ladder` and the minigame's kit (`kit`) load here.
pub fn plan(data: &CharacterData, catalog: &WeaponCatalog, table: &CaModTable, ladder: &Ladder, kit: &[&str]) -> LadderModels {
    let mut out = LadderModels::default();
    let weapon = |out: &mut LadderModels, id: &str, parts: &[&str]| {
        if !ours(id) {
            return;
        }
        push(&mut out.keys, id.to_string());
        if let Some(key) = key_with_parts(catalog, table, id, parts) {
            push(&mut out.keys, key);
        }
    };
    for id in kit {
        weapon(&mut out, id, &[]);
    }
    for (rung, gun) in ladder.rungs.iter().enumerate() {
        let extras = ladder.extras(rung);
        let given = extras.and_then(|x| x.value(rungs::PARTS)).unwrap_or("");
        let parts: Vec<&str> = given.split(',').filter(|p| !p.is_empty()).collect();
        weapon(&mut out, gun, &parts);
        let Some(x) = extras else { continue };
        if let Some(m) = &x.melee {
            weapon(&mut out, m, &[]);
        }
        if let Some(c) = &x.character {
            weapon(&mut out, &c.weapon, &[]);
        }
    }
    out.bodies = ladder_bodies(data, ladder);
    out
}

/// The ladder's bodies (`LadderModels::bodies`) not in `world` yet: those
/// no loaded group has (a stock group of the map's, or one loaded on
/// demand: `models::models_in`) and whose load didn't fail
/// (`bodies_on_demand::BodiesOnDemand::failed`). Empty once every body
/// that can arrive has; a stock body counts as there only where the map
/// has its group.
pub fn ladder_bodies_waiting(world: &World) -> Vec<i32> {
    let Some(ladder) = world.get_resource::<LadderModels>() else { return Vec::new() };
    let failed = world.get_resource::<super::bodies_on_demand::BodiesOnDemand>();
    let loaded = models::models_in(world);
    ladder
        .bodies
        .iter()
        .copied()
        .filter(|&id| !loaded.group_has(id) && !failed.is_some_and(|f| f.failed().contains_key(&id)))
        .collect()
}

/// The ladder's held and view models (`LadderModels::keys`) not in `world`
/// yet, each named `held <key>` or `view <key>`: a held model that loads
/// here (`held_on_demand::wanted`: one of ours the map's kit doesn't carry)
/// not added and not failed (`held_on_demand::HeldOnDemand::failed`); a
/// view model of ours not in the map's `ViewModels` (in the cache where
/// there are none, the greybox's start) and not failed (the cache's
/// error). Empty once every one that can arrive has.
pub fn ladder_keys_waiting(world: &World) -> Vec<String> {
    let Some(ladder) = world.get_resource::<LadderModels>() else { return Vec::new() };
    let held_failed = world.get_resource::<super::held_on_demand::HeldOnDemand>();
    let loose = world.get_resource::<crate::map::loose::LooseAssets>();
    let late = world.get_resource::<LateHeldModels>();
    let has_held = |k: &str| loose.is_some_and(|a| a.0.contains_key(k)) || late.is_some_and(|l| l.has(k));
    let views = world.get_resource::<crate::map::ViewModels>();
    let context = world.get_resource::<LoadContext>();
    let ca = context.map_or_else(CaData::install, CaData::install_for);
    let mut out = Vec::new();
    for key in &ladder.keys {
        let failed = held_failed.is_some_and(|f| f.failed().contains_key(key));
        if super::held_on_demand::wanted(key, has_held) && !failed {
            out.push(format!("held {key}"));
        }
        if !ours(key) {
            continue;
        }
        let in_store = views.map(|v| v.get(key).is_some());
        let cached = on_demand::cached(&ca, key);
        let arrived = match (in_store, &cached) {
            (Some(true), _) | (_, Some(Err(_))) => true,
            (None, Some(Ok(_))) => true,
            _ => false,
        };
        if !arrived {
            out.push(format!("view {key}"));
        }
    }
    out
}

/// A new ladder (or the server's coming in): what it loads.
pub fn plan_ladder(
    ladder: Res<Ladder>,
    active: Option<Res<ActiveMinigame>>,
    data: Res<CaData>,
    catalog: Option<Res<WeaponCatalog>>,
    table: Option<Res<CaModTable>>,
    mut out: ResMut<LadderModels>,
) {
    let Some(catalog) = catalog else { return };
    let kit: Vec<&str> = active.iter().flat_map(|a| a.def.spawn.kit.all()).collect();
    let table = table.as_deref().unwrap_or(CaModTable::built_in());
    let next = plan(data.characters(), &catalog, table, &ladder, &kit);
    if *out != next {
        info!("Combat Arms: this ladder loads {} bodies and {} weapon models", next.bodies.len(), next.keys.len());
        *out = next;
    }
}

/// Forget the models loaded on demand that the ladder just played
/// (`LadderModels`) doesn't give and no character wears or holds: a map's
/// unload (module doc). Only where the front end loads them.
pub fn forget_old(world: &mut World) {
    if !world.contains_resource::<LoadOnDemand>() {
        return;
    }
    let Some(ladder) = world.get_resource::<LadderModels>().cloned() else { return };
    let chars = world.get_resource::<CaData>().cloned();
    let mut bodies: BTreeSet<i32> = ladder.bodies.iter().copied().collect();
    if let Some(data) = &chars {
        let data = data.characters();
        let mut looks = world.query::<&Selection>();
        bodies.extend(looks.iter(world).map(|look| data.effective(look).0.id));
    }
    let mut keys: BTreeSet<String> = ladder.keys.iter().cloned().collect();
    let mut held = world.query::<&Held>();
    keys.extend(held.iter(world).filter_map(|h| h.0.clone()));
    let mut want = world.query::<&WantHeld>();
    keys.extend(want.iter(world).filter_map(|w| w.0.clone()));

    let mut forgotten = Forgotten::default();
    if let Some(mut late) = world.get_resource_mut::<LateCharacterModels>() {
        late.0.retain(|(m, _)| {
            let id = m.name.as_deref().and_then(models::body_record);
            let keep = id.is_none_or(|id| bodies.contains(&id));
            if !keep {
                forgotten.bodies.extend(id);
            }
            keep
        });
    }
    if let Some(mut late) = world.get_resource_mut::<LateHeldModels>() {
        late.0.retain(|(m, _)| {
            let keep = keys.contains(&m.key);
            if !keep {
                forgotten.keys.push(m.key.clone());
            }
            keep
        });
    }
    if let Some(mut late) = world.get_resource_mut::<models::CaLateModels>() {
        late.0.forget_bodies(&forgotten.bodies);
    }
    let context = world.get_resource::<LoadContext>();
    let ca = context.map_or_else(CaData::install, CaData::install_for);
    on_demand::forget(&ca, &keys);
    if !forgotten.bodies.is_empty() || !forgotten.keys.is_empty() {
        info!(
            "Combat Arms: forgot {} bodies and {} held models older ladders loaded",
            forgotten.bodies.len(),
            forgotten.keys.len()
        );
        world.insert_resource(forgotten);
    }
}

//! The match's ladder loaded ahead (backlog "changing Combat Arms
//! characters hitches and garbles"): when a `ladder::Ladder` is made or a
//! client takes the server's, its rungs' Combat Arms bodies, held and view
//! models (the fitted parts' too: `plan`) are what `bodies_on_demand`,
//! `held_on_demand` and `view_model::on_demand` load next, in the
//! background, from this machine's own install, on host and client alike.
//! Only the ladder's: anything else still loads the first time it is worn
//! or held. CS:S's and HL2:DM's rungs need nothing here: their maps' kits
//! carry all their bodies, held and view models (`map::sources`).
//!
//! A ladder is made as its match starts, once the map is in (a client's
//! comes with the match after that): so the load runs in the first seconds
//! of play, off the main thread, not behind the loading screen, and the swap
//! holds (`setup`, `pose_bodies`) if a rung is reached sooner.
//!
//! Only where `view_model::on_demand::LoadOnDemand` is (the front end).
//!
//! Known shortcuts (docs/tech-debt.md): nothing is forgotten: a new ladder
//! adds its models to the session's (`map::LateCharacterModels`,
//! `LateHeldModels`, the view models' cache), each map builds them again;
//! a player's own look that no rung gives still loads when first worn.

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
        view_model::{on_demand::ours, parts::view_key},
    },
    records::CharacterData,
};
use crate::weapon::data::WeaponCatalog;

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

/// The view (and held) model key of `weapon` fitted with `parts`
/// (`attachments::fit_given`, `parts::view_key`).
fn key_with_parts(catalog: &WeaponCatalog, table: &CaModTable, weapon: &str, parts: &[&str]) -> Option<String> {
    let params = catalog.get(weapon)?.hook::<AttachParams>(ATTACHMENTS)?;
    let fitted = fit(table, &params, parts).map(|p| p.map(|(id, ..)| id));
    Some(view_key(weapon, Some(&mods_for(table, &params, &fitted))))
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
        if let Some(id) = body_of(data, x) {
            push(&mut out.bodies, id);
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

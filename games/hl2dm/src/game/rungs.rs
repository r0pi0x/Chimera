//! HL2:DM's rung extras (docs/DESIGN.md "Gun game rules"; Chimera's
//! `minigame::ladder::RungExtrasHook`, registered with its guns, so
//! wherever HL2:DM's guns are on a ladder): when a ladder is made, each
//! HL2:DM rung draws, from the ladder's seeded generator, one player model
//! everyone on the rung wears alike, among those allowed to hold its gun
//! (characters follow the gun: `HOLDERS`, data/rung_characters.toml, on
//! Chimera's `minigame::holders`: any of the 19, the Combine's four and
//! the citizens' fifteen, HL2:DM having no team weapons; a definition's
//! `rung_extras = { character = { weapons = .. } }` overrides it). The
//! model's kind gives the rung's melee weapon (`RungExtras::melee`: the
//! stunstick for the Combine and metro police; a citizen keeps the
//! crowbar, the game's knife), and goes on the player at arming as
//! `weapons::inventory::Hl2Model` (a rung of another game's gun takes it
//! off again). The rung's HUD line names it.
//!
//! On by default; `rung_extras = { character = false }` (or `rung_extras =
//! false`) leaves the players' own look and the crowbar. The key is the
//! one CS:S's and Combat Arms' rungs read for their bodies.
//!
//! Known shortcuts: HL2:DM's player bodies are M8's (specs/hl2dm/
//! player_models.md): until then the model is a name and a melee weapon,
//! and the body the app's stand-in (the capsule).

use bevy::prelude::*;
use chimera_sim::minigame::{
    auto::Rng,
    holders::Holders,
    ladder::{RungDraw, RungExtras, RungExtrasAppExt, RungExtrasHook},
};

use super::{
    GAME,
    weapons::{
        STUNSTICK,
        inventory::{Hl2Model, ModelKind},
    },
};

/// Who holds which gun (data/rung_characters.toml).
pub const HOLDERS: &str = include_str!("../../data/rung_characters.toml");

/// `HOLDERS` parsed (bad built-in data panics; the tests load it).
pub fn holders() -> Holders {
    Holders::parse(HOLDERS).unwrap_or_else(|e| panic!("rung_characters.toml: {e}"))
}

/// The models allowed to hold `weapon` under `holders` (model paths).
pub fn models_for(holders: &Holders, weapon: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in holders.of(weapon, None) {
        let path = n.to_ascii_lowercase().replace('\\', "/");
        if ModelKind::of(&path).is_none() {
            warn!("rung characters: `{n}` is no HL2:DM player model");
        } else if !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

/// A model's name in the HUD's rung line.
pub fn label(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".mdl");
    match file {
        "combine_soldier" => "Combine Soldier".into(),
        "combine_soldier_prisonguard" => "Combine Prison Guard".into(),
        "combine_super_soldier" => "Combine Elite".into(),
        "police" => "Metro Police".into(),
        _ => match file.split_once('_') {
            Some((sex, n)) => format!("Rebel ({sex} {})", n.trim_start_matches('0')),
            None => file.to_string(),
        },
    }
}

/// The option key (`rung_extras = { character = false }`).
pub const CHARACTER_KEY: &str = "character";

/// The extras' value: the model's path.
pub const MODEL: &str = "model";

/// The hook.
pub const HOOK: RungExtrasHook = RungExtrasHook {
    game: GAME,
    options: &[CHARACTER_KEY],
    melee: |_| Vec::new(),
    characters: |_| Vec::new(),
    draw,
    apply,
};

/// Marks a player whose `Hl2Model` is a rung's (so leaving the rung takes
/// it off).
#[derive(Component, Clone, Copy, Debug)]
#[component(storage = "SparseSet")]
pub struct RungModel;

/// An HL2:DM rung's draw.
fn draw(_: &World, d: &RungDraw, rng: &mut Rng) -> Option<RungExtras> {
    if d.melee || !d.options.on(CHARACTER_KEY) {
        return None;
    }
    let holders = holders().tuned(d.options);
    let pool = models_for(&holders, d.weapon);
    if pool.is_empty() {
        return None;
    }
    let model = pool[rng.below(pool.len())].clone();
    let melee = match ModelKind::of(&model) {
        Some(ModelKind::Citizen) | None => None,
        Some(_) => Some(STUNSTICK.to_string()),
    };
    Some(RungExtras {
        label: Some(label(&model)),
        values: vec![(MODEL.into(), model)],
        melee,
        ..RungExtras::default()
    })
}

/// `e` armed for a rung: the rung's model on it, or (another game's rung,
/// or the option off) taken off.
fn apply(world: &mut World, e: Entity, extras: Option<&RungExtras>) {
    match extras.and_then(|x| x.value(MODEL)) {
        Some(path) => {
            world.entity_mut(e).insert((Hl2Model(path.to_string()), RungModel));
        }
        None => {
            if world.entity_mut(e).take::<RungModel>().is_some() {
                world.entity_mut(e).remove::<Hl2Model>();
            }
        }
    }
}

/// HL2:DM's rung extras on the ladder.
pub struct Hl2RungsPlugin;

impl Plugin for Hl2RungsPlugin {
    fn build(&self, app: &mut App) {
        app.add_rung_extras(HOOK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gun_takes_any_of_the_19_models() {
        let h = holders();
        let models = models_for(&h, super::super::weapons::AR2);
        assert_eq!(models.len(), 19);
        assert_eq!(models.iter().filter(|m| ModelKind::of(m) == Some(ModelKind::Citizen)).count(), 15);
        assert_eq!(label("models/police.mdl"), "Metro Police");
        assert_eq!(label("models/humans/group03/female_04.mdl"), "Rebel (female 4)");
    }
}

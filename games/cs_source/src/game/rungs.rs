//! CS:S's rung extras (docs/DESIGN.md "Gun game rules"; Chimera's
//! `minigame::ladder::RungExtrasHook`, registered by `CsGunsPlugin`, so
//! wherever CS:S's guns are on a ladder): when a ladder is made, each CS:S
//! rung draws, from the ladder's seeded generator, one player model that
//! everyone on the rung wears alike:
//!
//! - `character`: a player class (`core::PlayerClass` 1-8: the four T and
//!   four CT models CS:S ships, `bsp::CLASS_MODELS`; they are the install's
//!   player models) among those allowed to hold the rung's gun
//!   (characters follow the gun: `HOLDERS`, data/rung_characters.toml, on
//!   Chimera's `minigame::holders`: a gun only Terrorists buy, the weapon
//!   data's `buy.team` 1, a T model; a CT-only one a CT model; any other
//!   either; a definition's `rung_extras = { character = { weapons = ..
//!   } }` overrides it), put on the player at arming (a respawn, a rung swap,
//!   a late join) over its own class and the team's model, and replicated
//!   as the class is (a network client sees it as the player's body). The
//!   class gives the body and its hitboxes through the class's model
//!   (`map::CharacterModels::for_identity`; in a free-for-all the class
//!   counts whatever the team). The rung's HUD line names the model.
//!
//! On by default; `rung_extras = { character = false }` (or `rung_extras =
//! false`) leaves the players' own look. The key is the same one Combat
//! Arms' rungs read for their bodies. A rung of another game's weapon
//! takes it off again.
//!
//! Known shortcuts (docs/tech-debt.md): the pool is the eight stock
//! models, not other player models an install may carry (only these are
//! loaded); a player whose own class was chosen from the class menu gets
//! it back only by choosing again after leaving a CS:S rung; the eight
//! models' hitboxes are the stock layout, which is not compared between
//! them here.

use bevy::prelude::*;
use chimera_sim::minigame::{
    auto::Rng,
    ladder::{RungDraw, RungExtras, RungExtrasAppExt, RungExtrasHook},
};

use chimera_sim::{minigame::holders::Holders, weapon::data::WeaponCatalog};

use super::GAME;
use crate::core::PlayerClass;

/// Who holds which gun (data/rung_characters.toml).
pub const HOLDERS: &str = include_str!("../../data/rung_characters.toml");

/// `HOLDERS` parsed (bad built-in data panics; the tests load it).
pub fn holders() -> Holders {
    Holders::parse(HOLDERS).unwrap_or_else(|e| panic!("rung_characters.toml: {e}"))
}

/// What the weapon data says of `weapon` for the mapping: its buy team
/// (`team:<n>`), if only one team buys it.
pub fn derived(catalog: Option<&WeaponCatalog>, weapon: &str) -> Option<String> {
    let team = catalog?.get(weapon)?.buy.as_ref()?.team?;
    Some(format!("team:{team}"))
}

/// The classes (1-8) allowed to hold `weapon` under `holders`: the
/// mapping's characters read as class numbers or model names; none read:
/// all eight.
pub fn classes_for(holders: &Holders, catalog: Option<&WeaponCatalog>, weapon: &str) -> Vec<u8> {
    let names = holders.of(weapon, derived(catalog, weapon).as_deref());
    let mut out: Vec<u8> = Vec::new();
    for n in &names {
        let class = n
            .parse::<u8>()
            .ok()
            .filter(|c| (1..=8).contains(c))
            .or_else(|| NAMES.iter().position(|m| m.eq_ignore_ascii_case(n)).map(|i| i as u8 + 1));
        match class {
            Some(c) if !out.contains(&c) => out.push(c),
            Some(_) => {}
            None => warn!("rung characters: `{n}` is no CS:S player model"),
        }
    }
    if out.is_empty() { (1..=8).collect() } else { out }
}

/// The option key (`rung_extras = { character = false }`).
pub const CHARACTER_KEY: &str = "character";

/// The extras' value: the class (1-8).
pub const CLASS: &str = "class";

/// The models' names by class, in the HUD's rung line.
pub const NAMES: [&str; 8] = [
    "Phoenix Connexion",
    "Elite Crew",
    "Arctic Avengers",
    "Guerilla Warfare",
    "SEAL Team 6",
    "GSG-9",
    "SAS",
    "GIGN",
];

/// The hook.
pub const HOOK: RungExtrasHook = RungExtrasHook {
    game: GAME,
    options: &[CHARACTER_KEY],
    melee: |_| Vec::new(),
    characters: |_| Vec::new(),
    draw,
    apply,
};

/// Marks a player whose `PlayerClass` is a rung's (so leaving the rung
/// takes it off).
#[derive(Component, Clone, Copy, Debug)]
pub struct RungClass;

/// A CS:S rung's draw.
fn draw(world: &World, d: &RungDraw, rng: &mut Rng) -> Option<RungExtras> {
    // No character of its own for a melee rung (it keeps its knife).
    if d.melee || !d.options.on(CHARACTER_KEY) {
        return None;
    }
    let holders = holders().tuned(d.options);
    let pool = classes_for(&holders, world.get_resource::<WeaponCatalog>(), d.weapon);
    let class = pool[rng.below(pool.len())];
    Some(RungExtras {
        values: vec![(CLASS.into(), class.to_string())],
        label: Some(NAMES[class as usize - 1].into()),
        ..RungExtras::default()
    })
}

/// `e` armed for a rung: the rung's class on it, or (another game's rung,
/// or the option off) taken off.
fn apply(world: &mut World, e: Entity, extras: Option<&RungExtras>) {
    let class = extras.and_then(|x| x.value(CLASS)?.parse::<u8>().ok()).filter(|c| (1..=8).contains(c));
    match class {
        Some(c) => {
            world.entity_mut(e).insert((PlayerClass(c), RungClass));
        }
        None => {
            if world.entity_mut(e).take::<RungClass>().is_some() {
                world.entity_mut(e).remove::<PlayerClass>();
            }
        }
    }
}

/// CS:S's rung extras on the ladder.
pub struct CsRungsPlugin;

impl Plugin for CsRungsPlugin {
    fn build(&self, app: &mut App) {
        app.add_rung_extras(HOOK);
    }
}

//! The automatic gun game on the party's simulation, for every test binary
//! (included by `#[path]`): `ugg()` and what reads its ladder and powers.

use bevy::prelude::*;
use chimera_sim::minigame::{Minigames, auto};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::harness::CaSim;
use party_roster::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

pub(crate) fn ugg() -> Sim {
    Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin::default()))
}

/// The automatic gun game's ladder as `world` has it.
#[allow(dead_code)] // not every binary reads them
pub(crate) fn rungs(world: &World) -> Vec<String> {
    let def = world.resource::<Minigames>().get(MINIGAME).expect("the automatic gun game is made");
    def.rules.params["ladder"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

/// The powers of the guns of `caps` (`auto::powers`) as `world` scores them.
#[allow(dead_code)] // not every binary reads them
pub(crate) fn powers(world: &World, caps: &[auto::Capabilities]) -> chimera_sim::minigame::power::Powers {
    use chimera_sim::minigame::power::{PowerCurve, PowerReaders};
    let catalog = world.resource::<chimera_sim::weapon::data::WeaponCatalog>();
    auto::powers(caps, catalog, world.resource::<PowerReaders>(), &PowerCurve::shipped())
}

#[allow(dead_code)] // not every binary reads them
pub(crate) fn of(rungs: &[String], game: &str) -> usize {
    rungs.iter().filter(|id| id.starts_with(&format!("{game}:"))).count()
}

//! Counter-Strike 1.6's guns as data (data/weapons; docs/weapon-data.md). One
//! stand-in so far (`STANDIN`), made by tools/new-game, so the game can be
//! played in the sandbox (`rung cs16:weapon_standin`).
//!
//! Known shortcuts:
//! - A stand-in gun with no models or sounds: replace it with the game's
//!   guns from specs/cs16/weapons.md.

use bevy::prelude::*;
use chimera_sim::weapon::data::{WeaponDataAppExt, WeaponSet};

macro_rules! data_file {
    ($name:literal) => {
        ($name, chimera_core::game_data!("data/weapons/", $name))
    };
}

/// The weapon documents, built in, in registration order.
pub const DATA: &[(&str, chimera_core::game_data::GameData)] = &[data_file!("_base.toml"), data_file!("standin.toml")];

/// The stand-in gun.
pub const STANDIN: &str = "cs16:weapon_standin";

/// Every gun of the game's data.
pub const GUNS: &[&str] = &[STANDIN];

/// The game's weapons as data, parsed (a fresh copy each call).
pub fn weapons() -> WeaponSet {
    WeaponSet::parse(&chimera_core::game_data::texts(DATA), &[]).unwrap_or_else(|e| panic!("Counter-Strike 1.6 weapon data: {e}"))
}

/// Every gun's display name by id (its data's `hud.name`).
pub fn display_names(_: &crate::mount::Mount) -> Vec<(String, String)> {
    weapons().weapons.into_iter().filter_map(|w| Some((w.id, w.hud.name?))).collect()
}

/// The game's guns in an app's weapon registry.
pub struct Cs16GunsPlugin;

impl Plugin for Cs16GunsPlugin {
    fn build(&self, app: &mut App) {
        if let Err(e) = app.add_weapon_data(&chimera_core::game_data::texts(DATA)) {
            panic!("Counter-Strike 1.6 weapon data: {e}");
        }
    }
}

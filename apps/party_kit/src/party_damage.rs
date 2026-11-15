//! Each game's weapons' damage in the party apps (data/party_damage.toml):
//! a scale by the dealing weapon's game (`chimera_sim::weapon::
//! GameDamage`, which the engine's armour step applies to every weapon's
//! hit and the ladder's power curve counts), one place to tune the games
//! against each other. `add` puts it in an app; a game's own app has none.

use bevy::prelude::*;
use chimera_sim::{minigame::power::PowerReaders, weapon::GameDamage};

/// The table, as shipped.
pub const PARTY_DAMAGE: &str = include_str!("../data/party_damage.toml");

/// The shipped table. Bad built-in data panics.
pub fn table() -> GameDamage {
    GameDamage::parse(PARTY_DAMAGE).unwrap_or_else(|e| panic!("party_kit data/party_damage.toml: {e}"))
}

/// The table in `app`: for the hits and for the power curve.
pub fn add(app: &mut App) {
    let table = table();
    app.world_mut().get_resource_or_init::<PowerReaders>().game_damage = table.clone();
    app.insert_resource(table);
}

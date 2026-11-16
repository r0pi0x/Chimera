//! The party's cross-game balance (data/party_balance.toml): one table of
//! every number that applies only where games meet (each game's
//! `outgoing` and `incoming`, a body's own incoming by its id, the pair
//! exceptions), read into the engine's `chimera_sim::weapon::balance::
//! Balance`, which the armour step applies to every weapon hit, in one
//! place, and the ladder's power curve counts (its outgoing column). Halo
//! against Halo is never changed (the same game on both sides is 1, and
//! Halo's own hits never reach the table). Tuning is by playtests: see
//! docs/plans/active/party-balance.md, "The table and the switch".
//!
//! **The host's switch** `mp_balance` (1, the default: the table; 0: a
//! raw match, every gun and body with its own game's numbers): a
//! replicated server cvar (its `mp_` prefix), so a client has the
//! server's value through the cvars every client gets
//! (`chimera_net::net::cvars`), with no message of its own. On the server
//! (and a game with no server) it is put in force when a match starts
//! (`minigame::Minigames::starts`), so a change during a match applies
//! from the next match, never to half of one; a network client takes the
//! server's value as it comes (it applies no damage). The server's
//! `status` shows the switch in force and a pending change.

use bevy::prelude::*;
use chimera_core::core::NetRole;
use chimera_sim::{
    minigame::{Minigames, power::PowerReaders},
    weapon::balance::Balance,
};

/// The table, as shipped.
pub const PARTY_BALANCE: &str = include_str!("../data/party_balance.toml");

/// The shipped table, on. Bad built-in data panics.
pub fn table() -> Balance {
    Balance::parse(PARTY_BALANCE).unwrap_or_else(|e| panic!("party_kit data/party_balance.toml: {e}"))
}

/// The table in `app`, its switch (`mp_balance`) and the power curve's
/// outgoing column.
pub fn add(app: &mut App) {
    let table = table();
    app.world_mut().get_resource_or_init::<PowerReaders>().game_damage = table.power_table();
    app.insert_resource(table);
    chimera_core::console::resource_cvar::<Balance, u8>(
        app,
        "mp_balance",
        "Server: the party's cross-game balance (apps/party_kit/data/party_balance.toml): 1 on (the default), 0 a raw \
         match, every gun and body with its own game's numbers. Applies from the next match.",
        |b| &mut b.switch,
    );
    app.add_systems(FixedFirst, put_in_force);
}

/// The switch put in force (module doc): on a match's start here, at once
/// on a network client.
fn put_in_force(minigames: Option<Res<Minigames>>, role: Option<Res<NetRole>>, mut balance: ResMut<Balance>, mut seen: Local<Option<u64>>) {
    let client = role.is_some_and(|r| *r == NetRole::Client);
    let starts = minigames.map(|m| m.starts);
    if !client && starts == *seen {
        return;
    }
    *seen = starts;
    if balance.on != (balance.switch != 0) {
        balance.take_switch();
    }
}

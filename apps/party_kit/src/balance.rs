//! The party's cross-game balance (data/party_balance.toml): one table of
//! every number that applies only where games meet (each game's
//! `outgoing` and `incoming`, a body's own incoming by its id), read into the engine's `chimera_sim::weapon::balance::
//! Balance`, which the armour step applies to every weapon hit, in one
//! place, and the ladder's power curve counts (its outgoing column). The same
//! game on both sides counts like any pair (HL2:DM's 1.5 on its own
//! bodies, Halo on Halo 1.125); no exceptions. Tuning is by playtests: see
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
use chimera_core::slots::GameIdentity;
use chimera_sim::{
    minigame::{Minigames, power::PowerReaders, targets::Target},
    weapon::balance::{Balance, Victim},
};

/// The table, as shipped.
pub const PARTY_BALANCE: chimera_core::game_data::GameData = chimera_core::game_data!("data/party_balance.toml");

/// The shipped table, on. Bad built-in data panics.
pub fn table() -> Balance {
    Balance::parse(&PARTY_BALANCE).unwrap_or_else(|e| panic!("party_kit data/party_balance.toml: {e}"))
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
    app.add_systems(FixedFirst, (put_in_force, keep_victims));
}

/// Each character's game and body for the table (`Victim`): its game
/// identity (else a sandbox target's game) and a sandbox target's body id.
/// `Victim` (a sparse set) is read apart: in the changes' query it would
/// make that query step over every character archetype each tick, not
/// over their tables (`archetypes` test).
#[allow(clippy::type_complexity)]
fn keep_victims(
    q: Query<(Entity, Option<&GameIdentity>, Option<&Target>), Or<(Changed<GameIdentity>, Changed<Target>)>>,
    victims: Query<&Victim>,
    mut commands: Commands,
) {
    for (e, identity, target) in &q {
        let now = victims.get(e).ok();
        let want = Victim {
            game: identity.map(|g| g.0.clone()).or_else(|| target.map(|t| t.game.clone())),
            body: target.map(|t| t.character.clone()),
        };
        if now != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
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

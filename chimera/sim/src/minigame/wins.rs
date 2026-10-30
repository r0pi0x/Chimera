//! Matches won, per player, for as long as the server (or the single
//! player game) runs: `MatchWins` counts them by the player's name (the
//! stable identity: it survives a disconnect and the character coming
//! back, a map rotation, `change_match` and a new automatic ladder, none of
//! which clear it), and `MatchesWon` carries each player's count on the
//! character, which `chimera_net::minigame_net` replicates for the
//! scoreboard (`chimera_client::client::minigame_scoreboard`). Counted
//! when a match is decided with a winner (`results::record`, at the
//! match's end), on the authoritative side only; put on the characters
//! each tick (`publish`).
//!
//! Known shortcuts: keyed by name, so a player who renames starts from
//! zero and two players with one name share a count (the server does not
//! make names unique); not saved to disk.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::runtime::{InMinigame, JoinOrder};
use crate::{
    core::{Connecting, Health, Intent, Npc, Spectating, Team},
    mode::plays,
};

/// Matches won so far, by player name.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MatchWins(pub BTreeMap<String, u32>);

impl MatchWins {
    pub fn of(&self, name: &str) -> u32 {
        self.0.get(name).copied().unwrap_or(0)
    }

    pub fn add(&mut self, name: &str) {
        *self.0.entry(name.to_string()).or_insert(0) += 1;
    }
}

/// A player's matches won (replicated on the character).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchesWon(pub u32);

/// Puts each playing character's count on it, each tick (`runtime::
/// plugin`'s chain): the players as `runtime::players` finds them, in
/// join order, a nameless one named by its place (`Player N`, as the
/// results name it).
pub(super) fn publish(
    wins: Option<Res<MatchWins>>,
    mut players: Query<
        (Entity, &Team, Option<&JoinOrder>, Option<&Name>, Option<&mut MatchesWon>),
        (With<Intent>, With<Health>, With<InMinigame>, Without<Connecting>, Without<Spectating>, Without<Npc>),
    >,
    mut commands: Commands,
) {
    let Some(wins) = wins else { return };
    let mut order: Vec<(u64, Entity)> = players
        .iter()
        .filter(|(_, t, ..)| plays(t.0))
        .map(|(e, _, o, ..)| (o.map_or(u64::MAX, |o| o.0), e))
        .collect();
    order.sort();
    for (i, (_, e)) in order.into_iter().enumerate() {
        let Ok((_, _, _, name, won)) = players.get_mut(e) else { continue };
        let n = match name {
            Some(name) => wins.of(name.as_str()),
            None => wins.of(&format!("Player {}", i + 1)),
        };
        match won {
            Some(mut m) => {
                m.set_if_neq(MatchesWon(n));
            }
            None => {
                commands.entity(e).insert(MatchesWon(n));
            }
        }
    }
}

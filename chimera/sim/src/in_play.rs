//! What puts a party's gated games in play from the simulation
//! (`chimera_core::games_in_play`): a weapon of a game made (its id's
//! `game:` prefix, an observer: the same tick a rung of a new game hands
//! its gun over), and the match's ladder naming a game's guns (when it is
//! set or changes). `watch` adds both; the composer calls it beside its
//! `gate_game`s.

use bevy::prelude::*;
use chimera_core::games_in_play::{GamesInPlay, enter_later, game_of};

use crate::{minigame::ladder::Ladder, weapon::Weapon};

/// Weapons and the ladder put their games in play (module doc).
pub fn watch(app: &mut App) {
    app.add_observer(weapon_enters).add_systems(First, ladder_enters);
}

fn weapon_enters(on: On<Add<Weapon>>, weapons: Query<&Weapon>, games: Option<Res<GamesInPlay>>, mut commands: Commands) {
    if let Ok(w) = weapons.get(on.event_target())
        && let Some(game) = game_of(w.id)
    {
        enter_later(&mut commands, games.as_deref(), game, || format!("weapon {} made", w.id));
    }
}

/// Every game the ladder's rungs draw from enters play when the ladder is
/// set or changes.
fn ladder_enters(ladder: Option<Res<Ladder>>, games: Option<Res<GamesInPlay>>, mut commands: Commands) {
    let Some(ladder) = ladder.filter(|l| l.is_changed()) else { return };
    for game in ladder.rungs.iter().filter_map(|r| game_of(r)) {
        enter_later(&mut commands, games.as_deref(), game, || "the ladder's rungs".to_string());
    }
}

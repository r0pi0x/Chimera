//! What a client needs of HL2:DM's rules to draw its scoreboard and its
//! team line (specs/hl2dm/hud.md 7.2, 13), replicated from the server:
//! each player's team number (`Hl2Team`: HL2:DM's, which in deathmatch is
//! not the engine's `core::Team`, every player being a team of its own
//! there) and one match entity (`Hl2MatchView`: team play, the
//! intermission, the team scores). A net part (`hl2dm_standing`); written
//! by the server (or a host or single player) while the rules play.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use chimera_net::net::parts::NetAppExt;
use chimera_sim::mode::Match;
use serde::{Deserialize, Serialize};

use super::rules::{Hl2Match, Hl2Player, rules_on};

/// A player's HL2:DM team number.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hl2Team(pub u8);

/// The match as a client shows it.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Hl2MatchView {
    pub teamplay: bool,
    /// The game is over: the intermission.
    pub over: bool,
    /// Team scores, Combine then Rebels.
    pub scores: [i32; 2],
}

pub struct Hl2StandingPlugin;

impl Plugin for Hl2StandingPlugin {
    fn build(&self, app: &mut App) {
        app.net_part("hl2dm_standing", register).add_systems(Update, write.run_if(rules_on));
    }
}

fn register(app: &mut App) {
    use chimera_net::net::ordered::OrderedAppExt;
    app.replicate::<Hl2Team>().ordered::<Hl2Team>().replicate::<Hl2MatchView>().ordered::<Hl2MatchView>();
}

/// The rules' numbers into the replicated components.
fn write(
    players: Query<(Entity, &Hl2Player, Option<&Hl2Team>)>,
    mut views: Query<&mut Hl2MatchView>,
    state: Res<Hl2Match>,
    scores: Option<Res<Match>>,
    mut commands: Commands,
) {
    for (e, p, have) in &players {
        if have.is_none_or(|t| t.0 != p.team) {
            commands.entity(e).insert(Hl2Team(p.team));
        }
    }
    let now = Hl2MatchView {
        teamplay: state.teamplay,
        over: state.over,
        scores: scores.map_or([0; 2], |m| [m.score[0] as i32, m.score[1] as i32]),
    };
    match views.iter_mut().next() {
        Some(mut v) => {
            v.set_if_neq(now);
        }
        None => {
            commands.spawn((Name::new("Hl2dmMatch"), Replicated, now));
        }
    }
}

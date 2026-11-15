//! What a client needs of Halo's match to draw its score, clock and
//! game-over line (specs/halo_ce/hud.md, multiplayer_rules.md 6), from the
//! server: one replicated entity, `HaloMatchView` (the game type's name,
//! base type and teams, the score to win in its unit, the two teams'
//! scores, the game time, the time limit, the postgame and the results,
//! the winner). Each player's own score and deaths ride the engine's
//! `core::Score` (`NetScore`), its team `core::Team`. The net part
//! `halo_ce_match`; written by the server (a host, a game on its own)
//! while a game is on, every tick it changes.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use chimera_net::net::parts::NetAppExt;
use chimera_sim::mode::Match;
use serde::{Deserialize, Serialize};

use super::{HaloGameType, HaloMatch, HaloRules, rules_on, variant};

/// The match as a client shows it.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct HaloMatchView {
    /// The game type's name ("Slayer", "CTF" ...).
    pub name: String,
    /// The base type (`BaseGameType::code`: 1 capture the flag, 2 slayer,
    /// 3 oddball, 4 king, 5 race).
    pub base: i16,
    pub team_play: bool,
    /// The score that wins, in the score's unit (ticks for a game in
    /// minutes: `minutes`).
    pub score_to_win: i32,
    pub minutes: bool,
    /// Red's and blue's scores.
    pub scores: [i32; 2],
    /// Game time in Halo ticks.
    pub time: i64,
    /// The time limit in seconds (0: none).
    pub time_limit: f32,
    /// The game is over (the postgame) and the results are up.
    pub over: bool,
    pub results: bool,
    /// The winner's team id (free for all: the player's own), once over;
    /// None: a tie or not over.
    pub winner: Option<u8>,
}

/// The view, replicated (`halo_ce_match`), written each tick.
pub struct HaloMatchViewPlugin;

impl Plugin for HaloMatchViewPlugin {
    fn build(&self, app: &mut App) {
        app.net_part("halo_ce_match", register).add_systems(
            FixedPostUpdate,
            write
                .run_if(rules_on)
                .run_if(crate::core::authoritative)
                .run_if(crate::gameplay::gameplay_is(super::super::GAME)),
        );
    }
}

fn register(app: &mut App) {
    use chimera_net::net::ordered::OrderedAppExt;
    app.replicate::<HaloMatchView>().ordered::<HaloMatchView>();
}

/// The view of the match now.
pub fn view_of(game: &HaloGameType, state: &HaloMatch, m: &Match, rules: Option<&HaloRules>) -> HaloMatchView {
    let r = game.0.rules;
    HaloMatchView {
        name: game.0.name.clone(),
        base: r.base().code(),
        team_play: r.team_play,
        score_to_win: variant::winning_score(&r),
        minutes: variant::score_in_minutes(&r),
        scores: m.score,
        time: state.time,
        time_limit: rules.map_or(0.0, |r| r.timelimit * 60.0),
        over: state.over,
        results: state.results,
        winner: if state.over { m.winner } else { None },
    }
}

/// The rules' numbers into the replicated entity.
fn write(
    mut views: Query<&mut HaloMatchView>,
    game: Res<HaloGameType>,
    state: Res<HaloMatch>,
    m: Res<Match>,
    rules: Option<Res<HaloRules>>,
    mut commands: Commands,
) {
    let now = view_of(&game, &state, &m, rules.as_deref());
    match views.iter_mut().next() {
        Some(mut v) => {
            v.set_if_neq(now);
        }
        None => {
            commands.spawn((Name::new("HaloMatch"), Replicated, now));
        }
    }
}

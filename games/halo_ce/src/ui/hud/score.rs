//! Halo's match on the HUD (specs/halo_ce/hud.md 13 "Other text": the
//! multiplayer score and the match timer are the engine's own text, not HUD
//! tags; multiplayer_rules.md 6): from the replicated match
//! (`game::rules::view::HaloMatchView`) and the players' `core::Score`
//! and `core::Team`, on every machine:
//!
//! - **The score line** (top centre): the game type, the clock (the game
//!   time, or the time left under a time limit), and the scores: red's and
//!   blue's in a team game, the local player's and the leader's in free for
//!   all, each with the score to win (minutes and seconds for a game scored
//!   in time: king of the hill, oddball's time types).
//! - **The game-over line** (centre): "Game over" in the postgame, with who
//!   won (a team, the local player, another player by name, or a tie).
//! - **The scoreboard** while `+showscores` is held (Tab): every player,
//!   best first, with its score, kills and deaths (`Score`: the score and
//!   the deaths) and its team.
//!
//! Its own root (`HudRoot` of Halo: shown while Halo's HUD is), plain text
//! nodes, no restyling of the shell's.
//!
//! Known shortcuts: the text is Bevy's default font sized by the view's
//! height, not the disc's interface fonts (hud.md leaves the engine's text unspecified);
//! the scoreboard shows the score and deaths only (no separate kill count
//! is replicated).

use bevy::prelude::*;
use chimera_client::client::{console::HeldActions, hud::HudRoot};

use crate::{
    core::{LocalPlayer, Score, Team},
    game::{
        GAME,
        rules::{BLUE, RED, view::HaloMatchView},
    },
};

/// The score line, the game-over line and the board.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreText {
    Line,
    GameOver,
    Board,
}

/// The match's text on Halo's HUD.
pub struct HaloScorePlugin;

impl Plugin for HaloScorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeldActions>()
            .add_systems(Startup, spawn)
            .add_systems(Update, update.run_if(crate::gameplay::hud_is(GAME)));
    }
}

fn spawn(mut commands: Commands) {
    let root = commands
        .spawn((
            Name::new("Halo score"),
            HudRoot(GAME),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
            Visibility::Inherited,
            Pickable::IGNORE,
        ))
        .id();
    let text = |kind: ScoreText, top: f32, size: f32| {
        (
            kind,
            Text::new(""),
            TextFont { font_size: bevy::text::FontSize::Vh(size), ..default() },
            TextColor(Color::srgb(0.46, 0.73, 1.0)),
            TextLayout::justify(Justify::Center),
            Node { position_type: PositionType::Absolute, top: percent(top), width: percent(100.0), justify_content: JustifyContent::Center, ..default() },
            Pickable::IGNORE,
            ChildOf(root),
        )
    };
    commands.spawn(text(ScoreText::Line, 2.0, 2.6));
    commands.spawn(text(ScoreText::GameOver, 38.0, 6.0));
    commands.spawn(text(ScoreText::Board, 20.0, 2.4));
}

/// Ticks as `m:ss`.
pub fn clock(ticks: i64) -> String {
    let s = ticks.max(0) / 30;
    format!("{}:{:02}", s / 60, s % 60)
}

/// A score in its unit (`minutes`: ticks as `m:ss`).
pub fn score_text(score: i32, minutes: bool) -> String {
    if minutes { clock(score as i64) } else { score.to_string() }
}

/// The score line of `v`; `me` the local player's score, `leader` the best
/// other's (free for all).
pub fn line(v: &HaloMatchView, me: Option<i32>, leader: Option<i32>) -> String {
    let time = if v.time_limit > 0.0 { clock((v.time_limit * 30.0) as i64 - v.time) } else { clock(v.time) };
    let s = |n: i32| score_text(n, v.minutes);
    let goal = s(v.score_to_win);
    let scores = if v.team_play {
        format!("Red {}  Blue {}", s(v.scores[0]), s(v.scores[1]))
    } else {
        format!("You {}  Leader {}", s(me.unwrap_or(0)), s(leader.unwrap_or(0).max(me.unwrap_or(0))))
    };
    format!("{}   {time}   {scores}   (to {goal})", v.name)
}

/// The game-over line: `winner` is `v.winner` resolved to text.
pub fn game_over(v: &HaloMatchView, my_team: Option<u8>, winner_name: Option<&str>) -> String {
    if !v.over {
        return String::new();
    }
    let who = match v.winner {
        None => "Tie".to_string(),
        Some(RED) if v.team_play => "Red team wins".into(),
        Some(BLUE) if v.team_play => "Blue team wins".into(),
        Some(t) if Some(t) == my_team => "You win".into(),
        Some(_) => format!("{} wins", winner_name.unwrap_or("Another player")),
    };
    format!("Game over\n{who}")
}

#[allow(clippy::type_complexity)]
fn update(
    views: Query<&HaloMatchView>,
    players: Query<(Entity, &Score, Option<&Team>, Option<&chimera_net::net::NetCharacter>, Option<&Name>, Has<LocalPlayer>)>,
    held: Res<HeldActions>,
    settings: Option<Res<chimera_net::net::NetSettings>>,
    mut texts: Query<(&ScoreText, &mut Text, &mut Visibility)>,
) {
    let view = views.iter().next();
    let mut rows: Vec<(i32, u32, String, u8, bool)> = players
        .iter()
        .map(|(e, s, t, ch, name, local)| {
            let shown = chimera_client::client::shown_name(e, local, ch, name, settings.as_deref());
            (s.kills, s.deaths, shown, t.map_or(0, |t| t.0), local)
        })
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let me = rows.iter().find(|r| r.4);
    let leader = rows.iter().filter(|r| !r.4).map(|r| r.0).max();
    for (kind, mut text, mut vis) in &mut texts {
        let (now, show) = match (kind, view) {
            (_, None) => (String::new(), false),
            (ScoreText::Line, Some(v)) => (line(v, me.map(|m| m.0), leader), true),
            (ScoreText::GameOver, Some(v)) => {
                let name = rows.iter().find(|r| Some(r.3) == v.winner).map(|r| r.2.as_str());
                (game_over(v, me.map(|m| m.3), name), v.over)
            }
            (ScoreText::Board, Some(v)) => {
                let lines: Vec<String> = rows
                    .iter()
                    .map(|(score, deaths, name, team, _)| {
                        let side = if !v.team_play { "" } else if *team == RED { "red  " } else if *team == BLUE { "blue " } else { "     " };
                        format!("{side}{name:<16} {:>6} {:>4}", score_text(*score, v.minutes), deaths)
                    })
                    .collect();
                (format!("{:<21} {:>6} {:>4}\n{}", "Player", "Score", "Dths", lines.join("\n")), held.scores())
            }
        };
        if text.0 != now {
            text.0 = now;
        }
        vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
}

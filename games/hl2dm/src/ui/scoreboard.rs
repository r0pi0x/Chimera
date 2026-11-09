//! The scoreboard (specs/hl2dm/hud.md 13), held with `+showscores` and up
//! at the intermission: a 480 by 360 unit panel re-centred on the screen,
//! a dim box with its corners cut in a staircase and a one pixel border,
//! the server's name, the column headings (Score, Deaths, Latency) and a
//! section per team (deathmatch: the free-for-all, then the spectators;
//! team deathmatch: Combine, Rebels, spectators), each player a row in its
//! team's colour sorted by frags, then deaths, then index. In deathmatch
//! the "Deathmatch - N players" text lands on the spectators' header and
//! the free-for-all's stays empty (a quirk kept).
//!
//! `Board` is what the game tells it (`systems` collects it); `frame` lays
//! it out in window pixels.

use bevy::prelude::*;

use super::{
    aux_power::team_colour,
    frame::{Align, Draw, Frame, Screen},
    look::Look,
};
use crate::game::rules::{COMBINE, REBELS, SPECTATOR, UNASSIGNED, team_name};

/// The panel's size (units), its list's left edge and the rows' pitch.
pub const WIDE: f32 = 480.0;
pub const TALL: f32 = 360.0;
pub const LIST_X: f32 = 8.0;
pub const ROW: f32 = 16.0;
/// Where the rows start under the server's name (ours: the file gives the
/// list the whole panel).
pub const LIST_Y: f32 = 26.0;
/// Column widths (units): avatar, name, score, deaths, latency.
pub const COLUMNS: [f32; 5] = [36.0, 284.0, 40.0, 46.0, 46.0];
/// The corner staircase's offsets, in pixels.
pub const STAIRS: [f32; 8] = [10.0, 9.0, 6.0, 4.0, 3.0, 2.0, 1.0, 0.0];

#[derive(Clone, Debug, PartialEq)]
pub struct BoardPlayer {
    pub name: String,
    pub team: u8,
    pub frags: i32,
    pub deaths: u32,
    /// Milliseconds; 0 for the host and bots.
    pub ping: u16,
    pub bot: bool,
    pub local: bool,
    /// The player's index, the last tie-break.
    pub index: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    pub teamplay: bool,
    pub server_name: String,
    pub players: Vec<BoardPlayer>,
    /// Team scores, Combine then Rebels.
    pub team_scores: [i32; 2],
}

/// The panel's rectangle: re-centred on the screen at every refresh.
pub fn panel_rect(s: Screen) -> Rect {
    let (w, h) = (s.scaled(WIDE), s.scaled(TALL));
    let min = Vec2::new(((s.w - w) / 2.0).floor(), ((s.h - h) / 2.0).floor());
    Rect::from_corners(min, min + Vec2::new(w, h))
}

fn sorted(players: &[BoardPlayer], team: u8) -> Vec<&BoardPlayer> {
    let mut v: Vec<&BoardPlayer> = players.iter().filter(|p| p.team == team).collect();
    v.sort_by(|a, b| b.frags.cmp(&a.frags).then(a.deaths.cmp(&b.deaths)).then(a.index.cmp(&b.index)));
    v
}

/// A section header's "<name>    -   N player(s)".
fn count_text(look: &Look, name: &str, n: usize) -> String {
    let token = if n == 1 { "#ScoreBoard_Player" } else { "#ScoreBoard_Players" };
    let fallback = if n == 1 { "%s1    -   %s2 player" } else { "%s1    -   %s2 players" };
    look.string(token, fallback).replace("%s1", name).replace("%s2", &n.to_string())
}

/// The scoreboard's draws.
pub fn frame(look: &Look, board: &Board) -> Frame {
    let mut f = Frame::default();
    let s = look.screen;
    let panel = panel_rect(s);
    let (bg, fg) = (look.colour("SectionedListPanel.BgColor"), look.colour("FgColor"));
    // The box: its middle full width, the corners' rows inset by the
    // staircase; the border follows the outline.
    let rows = STAIRS.len() as f32;
    f.push(Draw::Box { rect: Rect::new(panel.min.x, panel.min.y + rows, panel.max.x, panel.max.y - rows), color: bg, round: false });
    for (r, inset) in STAIRS.iter().enumerate() {
        let r = r as f32;
        for y in [panel.min.y + r, panel.max.y - r - 1.0] {
            f.push(Draw::Box { rect: Rect::new(panel.min.x + inset, y, panel.max.x - inset, y + 1.0), color: bg, round: false });
            f.push(Draw::Box { rect: Rect::new(panel.min.x + inset, y, panel.min.x + inset + 1.0, y + 1.0), color: fg, round: false });
            f.push(Draw::Box { rect: Rect::new(panel.max.x - inset - 1.0, y, panel.max.x - inset, y + 1.0), color: fg, round: false });
        }
    }
    for (a, b) in [(panel.min.x + 10.0, panel.max.x - 10.0)] {
        for y in [panel.min.y, panel.max.y - 1.0] {
            f.push(Draw::Box { rect: Rect::new(a, y, b, y + 1.0), color: fg, round: false });
        }
    }
    for x in [panel.min.x, panel.max.x - 1.0] {
        f.push(Draw::Box { rect: Rect::new(x, panel.min.y + 10.0, x + 1.0, panel.max.y - 10.0), color: fg, round: false });
    }
    let tall = look.tall("Default");
    f.push(Draw::text("Default", tall, board.server_name.clone(), fg, panel.min + Vec2::new(s.scaled(3.0), s.scaled(2.0)), Align::Left));
    // Column edges in pixels from the list's left.
    let x0 = panel.min.x + s.scaled(LIST_X);
    let mut edge = [0.0; 6];
    for (i, w) in COLUMNS.iter().enumerate() {
        edge[i + 1] = edge[i] + s.scaled(*w);
    }
    let col = |i: usize| x0 + edge[i];
    let mut y = panel.min.y + s.scaled(LIST_Y);
    // The heading row.
    for (i, token, fallback) in [(2, "#PlayerScore", "Score"), (3, "#PlayerDeath", "Deaths"), (4, "#PlayerPing", "Latency")] {
        f.push(Draw::text("Default", tall, look.string(token, fallback), fg, Vec2::new(col(i + 1), y), Align::Right));
    }
    y += s.scaled(ROW);
    let sections: Vec<(u8, bool)> = if board.teamplay {
        vec![(COMBINE, true), (REBELS, true), (SPECTATOR, true)]
    } else {
        vec![(UNASSIGNED, false), (SPECTATOR, true)]
    };
    for (team, has_header) in sections {
        let members = sorted(&board.players, team);
        let colour = if team == UNASSIGNED { None } else { Some(team_colour(team)) };
        let header_colour = colour.unwrap_or(fg);
        // The header: a team's name and count; in deathmatch the text is
        // the spectators' and the free-for-all's is empty.
        let header = if board.teamplay {
            Some(count_text(look, team_name(team), members.len()))
        } else if team == SPECTATOR {
            Some(count_text(look, "Deathmatch", board.players.len()))
        } else {
            None
        };
        if has_header || header.is_some() {
            if let Some(h) = &header {
                f.push(Draw::text("Default", tall, h.clone(), header_colour, Vec2::new(col(1), y), Align::Left));
            }
            if board.teamplay && team != SPECTATOR {
                let score = board.team_scores[(team == REBELS) as usize];
                f.push(Draw::text("Default", tall, score.to_string(), header_colour, Vec2::new(col(3), y), Align::Right));
                let avg = if members.is_empty() { 0 } else { members.iter().map(|p| p.ping as u32).sum::<u32>() / members.len() as u32 };
                f.push(Draw::text("Default", tall, avg.to_string(), header_colour, Vec2::new(col(5), y), Align::Right));
            }
            y += s.scaled(ROW);
        } else {
            y += s.scaled(ROW);
        }
        for p in members {
            let c = colour.unwrap_or(look.colour("FgColor"));
            let c = if team == UNASSIGNED { team_colour(UNASSIGNED) } else { c };
            if p.local {
                f.push(Draw::Box { rect: Rect::new(x0, y, col(5), y + s.scaled(ROW)), color: [fg[0], fg[1], fg[2], 36], round: false });
            }
            f.push(Draw::text("Default", tall, p.name.clone(), c, Vec2::new(col(1), y), Align::Left));
            if team != SPECTATOR {
                f.push(Draw::text("Default", tall, p.frags.to_string(), c, Vec2::new(col(3), y), Align::Right));
                f.push(Draw::text("Default", tall, p.deaths.to_string(), c, Vec2::new(col(4), y), Align::Right));
            }
            let ping = if p.ping >= 1 { p.ping.to_string() } else if p.bot { "BOT".into() } else { String::new() };
            if !ping.is_empty() {
                f.push(Draw::text("Default", tall, ping, c, Vec2::new(col(5), y), Align::Right));
            }
            y += s.scaled(ROW);
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::look::spec_look;

    fn player(name: &str, team: u8, frags: i32, deaths: u32, index: u32) -> BoardPlayer {
        BoardPlayer { name: name.into(), team, frags, deaths, ping: 0, bot: false, local: false, index }
    }

    fn order(f: &Frame, names: &[&str]) -> Vec<String> {
        f.texts().filter_map(|d| d.as_text()).filter(|t| names.contains(t)).map(str::to_string).collect()
    }

    /// Spec test 64: A 5/2, B 5/1, C 7/9 and a spectator S: C, B, A; the
    /// free-for-all header empty and "Deathmatch    -   4 players" over
    /// the spectators.
    #[test]
    fn deathmatch_orders_by_frags_then_deaths() {
        let look = spec_look(Screen::base());
        let board = Board {
            teamplay: false,
            players: vec![player("A", 0, 5, 2, 1), player("B", 0, 5, 1, 2), player("C", 0, 7, 9, 3), player("S", SPECTATOR, 0, 0, 4)],
            ..Default::default()
        };
        let f = frame(&look, &board);
        assert_eq!(order(&f, &["A", "B", "C", "S"]), ["C", "B", "A", "S"]);
        let header = f.find_text("Deathmatch    -   4 players").expect("the text");
        let s = f.find_text("S").unwrap();
        let (Draw::Text { at: h, .. }, Draw::Text { at: sp, .. }) = (header, s) else { panic!() };
        assert!(h.y < sp.y, "the header sits over the spectators");
    }

    /// Spec test 65: team deathmatch has its three sections in their
    /// colours, each header with its count.
    #[test]
    fn team_sections_have_their_headers() {
        let look = spec_look(Screen::base());
        let board = Board {
            teamplay: true,
            players: vec![player("A", COMBINE, 1, 0, 1), player("B", REBELS, 2, 0, 2), player("C", REBELS, 0, 1, 3)],
            team_scores: [4, 6],
            ..Default::default()
        };
        let f = frame(&look, &board);
        let colour = |text: &str| match f.find_text(text) {
            Some(Draw::Text { color, .. }) => *color,
            _ => panic!("{text}"),
        };
        assert_eq!(colour("Combine    -   1 player"), [153, 204, 255, 255]);
        assert_eq!(colour("Rebels    -   2 players"), [255, 64, 64, 255]);
        assert!(f.find_text("Spectator    -   0 players").is_some());
        assert_eq!(f.find_text("6").is_some() && f.find_text("4").is_some(), true);
    }

    /// Spec test 66: at 1920 by 1080 the panel is 1080 by 810 at (420, 135)
    /// and the rows are 36 pixels apart.
    #[test]
    fn the_panel_is_centred_and_scaled() {
        let s = Screen::new(1920.0, 1080.0);
        assert_eq!(panel_rect(s), Rect::new(420.0, 135.0, 1500.0, 945.0));
        let look = spec_look(s);
        let board = Board { players: vec![player("A", 0, 1, 0, 1), player("B", 0, 0, 0, 2)], ..Default::default() };
        let f = frame(&look, &board);
        let y = |n: &str| match f.find_text(n) {
            Some(Draw::Text { at, .. }) => at.y,
            _ => panic!(),
        };
        assert_eq!(y("B") - y("A"), 36.0);
    }
}

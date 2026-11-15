//! The scoreboard (specs/combat_arms/ui_system.md's team list, I2 and its
//! test case 18; game_modes.md S13 for its rows): while the score key is
//! held (`+showscores`, Tab) and no in-game popup is up (Corrections 9),
//! and from a round's result until the next round's setup (Corrections 1:
//! the score key does nothing meanwhile), with the result's banner (by
//! mode and winner, Corrections 3: `round_result`; none for a training
//! round without a winner) and the sound it played; a single-round mode's
//! result shows the banner alone, without the lists (Corrections 2).
//! Alpha's rows in the upper list, every other team's in the lower (a
//! free-for-all mode's in one list, "Players"), each in the mode's order
//! (`game::rules::scoreboard`); the team scores (the score panel's
//! numbers) in the lists' headers; the mode's column (captures, mission
//! points) where it has one.
//!
//! `CaScoreboardStatePlugin` reads the world into `CaScoreboard` with no
//! window (the tests'); `CaScoreboardPlugin` adds it and draws it with
//! the shell's UI nodes, stretched by x_r, y_r like the HUD's panels.
//!
//! With the install's layouts (V1 key) the board is the game's own
//! (`BoundBoard`: the common frame with the result banner, the team or
//! no-team list, one clone of the row template per player); without
//! them ours below.
//!
//! Known shortcuts: without the install the columns' places are ours
//! (name, kills, deaths, assists, ping across the 766 px row), the panel
//! and rows flat placeholder boxes and the banner its name; with it no
//! level icon, clan, nation flag, name background or infection mark
//! (hidden), the ping icon's buckets ours (`ping_icon`), the frame's room
//! and operation texts hidden (samples), the team title images the
//! layout's (its "win" ones); the result sound is counted
//! (`CaScoreboard::sound`), not played (the client interface attributes
//! are keyed); the match's result screen isn't there, so the last
//! round's result stays up once the match is over; "multi-round" is our
//! list of round modes (`hud::model::round_mode`: the room data's flag
//! isn't read); the team-change notice and hosted modes don't exist yet.

use bevy::prelude::*;

use super::{
    CaUi,
    hud::{
        HudScreen, PenParams, bound, hud_text, layer,
        model::{Screen, round_mode},
        pen, rect, rebuild,
        score::result_showing,
    },
    layout::{Effect, Layout},
    menu::InGameMenu,
};
use crate::{
    client::{
        canvas::{HAlign, Scale, VAlign, WidgetId},
        console::HeldActions,
    },
    game::rules::{
        CaMatchView, Panel, ScoreOrder, Teams,
        scoreboard::{ScoreRow, board_for, rows},
    },
};

/// The panel and its lists (ref px).
pub mod places {
    use bevy::math::Vec2;

    pub const PANEL: (Vec2, Vec2) = (Vec2::new(93.0, 161.0), Vec2::new(838.0, 432.0));
    /// The lists' first rows' top-left: Alpha, the rest.
    pub const LISTS: [Vec2; 2] = [Vec2::new(164.0, 192.0), Vec2::new(164.0, 392.0)];
    pub const ROW: Vec2 = Vec2::new(766.0, 25.0);
    /// Ours: each column's x offset in a row and width: name, the mode's
    /// column, kills, deaths, assists, ping.
    pub const COLUMNS: [(f32, f32); 6] = [
        (8.0, 320.0),
        (335.0, 80.0),
        (420.0, 80.0),
        (510.0, 80.0),
        (600.0, 80.0),
        (690.0, 70.0),
    ];
    /// Ours: the lists' headers (team and score) above their rows.
    pub const HEADERS: [Vec2; 2] = [Vec2::new(164.0, 168.0), Vec2::new(164.0, 368.0)];
}

/// The board as shown.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaScoreboard {
    pub shown: bool,
    /// Alpha's rows, then every other team's.
    pub lists: [Vec<ScoreRow>; 2],
    /// The lists (and the frame's `result_info`) show: not while a
    /// single-round mode's result shows (the banner alone, ui_system.md
    /// Corrections 2).
    pub lists_shown: bool,
    /// The team scores (Alpha, Bravo).
    pub scores: [i32; 2],
    /// The lists' names ("Alpha", "Bravo"; "Players" in a free-for-all
    /// mode).
    pub headers: [String; 2],
    /// The mode's order (its column).
    pub order: Option<ScoreOrder>,
    /// The mode's room key (its column's title, S13).
    pub mode: Option<String>,
    /// A free-for-all mode (one list).
    pub ffa: bool,
    /// The round's result banner while the result shows (none in a
    /// training round without a winner).
    pub result: Option<RoundResult>,
    /// The interface sound the result played when it came in
    /// (Corrections 3; counted, not played: the client interface
    /// attributes are keyed).
    pub sound: Option<&'static str>,
}

/// A round's result banner as the local player sees it (ui_system.md
/// Corrections 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundResult {
    Win,
    Lose,
    Draw,
    /// The free-for-all pair (Last Man Standing, One Man Army, Arms Race,
    /// and Spy Hunt for players not on Alpha).
    FfaWin,
    FfaLose,
    /// Quarantine's, by winner (Alpha the humans).
    HumanWin,
    InfectorWin,
}

/// Mode `key`'s banner and sound for a round won by `winner` (a team id;
/// none: no winner), the local player's team id `mine`, in a training
/// round or not (ui_system.md Corrections 3).
pub fn round_result(key: &str, winner: Option<u8>, mine: Option<u8>, training: bool) -> (Option<RoundResult>, Option<&'static str>) {
    let Some(w) = winner else {
        return ((!training).then_some(RoundResult::Draw), None);
    };
    let ffa = matches!(key, "LMS" | "FFA" | "VWRM") || (key == "BNT" && mine != Some(1));
    let banner = if key == "QRT" {
        if w == 1 { RoundResult::HumanWin } else { RoundResult::InfectorWin }
    } else if ffa {
        if Some(w) == mine { RoundResult::FfaWin } else { RoundResult::FfaLose }
    } else if Some(w) == mine {
        RoundResult::Win
    } else {
        RoundResult::Lose
    };
    let sound = match (key, w) {
        ("QRT", 1) => Some("QRTHumanWinSound"),
        ("QRT", _) => Some("QRTInfectorWinSound"),
        ("LMS" | "FFA" | "VWRM" | "BNT", _) => None,
        (_, 1) => Some("RoundAlphaTeamWinSound"),
        (_, 2) => Some("RoundBravoTeamWinSound"),
        _ => None,
    };
    (Some(banner), sound)
}

/// The result shown: its banner and sound, fixed when it came in.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
struct ResultShown(Option<(Option<RoundResult>, Option<&'static str>)>);

/// The board's state without a window.
pub struct CaScoreboardStatePlugin;

impl Plugin for CaScoreboardStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaScoreboard>()
            .init_resource::<ResultShown>()
            .init_resource::<HeldActions>()
            .init_resource::<CaMatchView>()
            .add_systems(Update, update);
    }
}

fn update(world: &mut World) {
    let view = world.resource::<CaMatchView>().clone();
    let mode = view.mode();
    // A kept query state (`query_cache`): this runs every frame.
    let mine = chimera_core::query_cache::cached_query::<&crate::core::Team, With<crate::core::LocalPlayer>, _>(
        world,
        |q, world| q.iter(world).next().map(|t| t.0),
    );
    // The result: from the round's result message until the next round's
    // setup (Corrections 1); its banner and sound fixed when it came in.
    let showing = mode.is_some() && result_showing(view.phase);
    let mut result = *world.resource::<ResultShown>();
    match (showing, result.0) {
        (true, None) => {
            let key = mode.map_or("", |m| m.key);
            result.0 = Some(round_result(key, view.winner, mine, view.warmup));
        }
        (false, Some(_)) => result.0 = None,
        _ => {}
    }
    world.insert_resource(result);
    // The score key, unless an in-game popup is up (Corrections 9); while
    // the result shows it does nothing (the board stays up).
    let popup = world.get_resource::<InGameMenu>().is_some_and(|m| m.open);
    // A minigame has its own scoreboard (the shell's).
    let minigame = world.get_resource::<chimera_sim::minigame::hud::MinigameHud>().is_some_and(|h| h.playing());
    let held = world.resource::<HeldActions>().scores() && !popup && !minigame;
    let shown = mode.is_some() && (showing || held);
    let order = mode.map_or(ScoreOrder::Kills, |m| m.order);
    let ffa = mode.is_some_and(|m| m.teams == Teams::FreeForAll);
    let multi = mode.is_some_and(|m| round_mode(m.key));
    let next = CaScoreboard {
        shown,
        ffa,
        result: result.0.and_then(|r| r.0),
        sound: result.0.and_then(|r| r.1),
        lists: if shown { board_for(rows(world), order, ffa) } else { Default::default() },
        lists_shown: !(showing && !multi),
        scores: if mode.is_some_and(|m| m.panel == Panel::RoundWins) {
            view.round_wins.map(|w| w as i32)
        } else {
            view.score
        },
        headers: if ffa {
            ["Players".into(), String::new()]
        } else {
            ["Alpha".into(), "Bravo".into()]
        },
        order: mode.map(|m| m.order),
        mode: mode.map(|m| m.key.to_string()),
    };
    if *world.resource::<CaScoreboard>() != next {
        world.insert_resource(next);
    }
}

// ---------------------------------------------------------------------------
// The game's own scoreboard layouts (ui_system.md H)

/// The scoreboard from the install's layouts, bound by name: the common
/// frame (`Result_Normal`: its `Game_Result` image the round's result
/// banner, the HUD layout attributes' `scoreboardwintex`,
/// `scoreboardlosetex`, `scoreboarddrawtex` and the free-for-all
/// `scoreboardffawintex` / `scoreboardffalosetex`) and the team list
/// (`Team_UserList`: Alpha's rows from `UserList_UP`'s template
/// `User_Column`, the others' from `UserList_Down`'s `DUser_Column`, the
/// team scores in `alpha_point` and `bravo_point`) or
/// the no-team list (`NoTeam_UserList`'s `UserList`), one clone of the
/// template per row, 25 ref px apart (the list's gap and blank, 0).
#[derive(Resource, Clone, Debug, Default)]
pub struct BoundBoard {
    pub frame: Option<Layout>,
    pub team: Option<Layout>,
    pub no_team: Option<Layout>,
    /// The result banners' textures: win, lose, draw, free-for-all win
    /// and lose, Quarantine's human and infector wins.
    pub banners: [Option<String>; 7],
}

impl BoundBoard {
    pub fn of(ui: &CaUi) -> Self {
        let Some(inst) = ui.install.as_ref() else { return Self::default() };
        let t = |k: &str| inst.hud_texture(k);
        Self {
            frame: inst.result.clone(),
            team: inst.team_list.clone(),
            no_team: inst.no_team_list.clone(),
            banners: [
                t("scoreboardwintex"),
                t("scoreboardlosetex"),
                t("scoreboarddrawtex"),
                t("scoreboardffawintex"),
                t("scoreboardffalosetex"),
                t("humanroundwintex"),
                t("infectorroundwintex"),
            ],
        }
    }

    /// The board drawn at `scale`; None without the lists' layout.
    pub fn build(&self, board: &CaScoreboard, scale: Scale) -> Option<Layout> {
        let (src, panel, lists): (&Layout, &str, &[(&str, &str)]) = if board.ffa {
            (self.no_team.as_ref()?, "NoTeam_UserList", &[("UserList", "User_Column")])
        } else {
            (
                self.team.as_ref()?,
                "Team_UserList",
                &[("UserList_UP", "User_Column"), ("UserList_Down", "DUser_Column")],
            )
        };
        let mut l = Layout::new();
        l.scale = scale;
        if let Some(frame) = &self.frame
            && let Some(f) = frame.find("Result_Normal")
        {
            let id = l.tree.graft(0, &frame.tree, f);
            let game_result = l.tree.find_from(id, "Game_Result");
            let banner = board.result.and_then(|r| {
                let i = match r {
                    RoundResult::Win => 0,
                    RoundResult::Lose => 1,
                    RoundResult::Draw => 2,
                    RoundResult::FfaWin => 3,
                    RoundResult::FfaLose => 4,
                    RoundResult::HumanWin => 5,
                    RoundResult::InfectorWin => 6,
                };
                self.banners[i].clone()
            });
            let root1 = game_result.and_then(|g| l.tree.get(g).parent);
            bound::show(&mut l, root1, banner.is_some());
            if let Some(b) = &banner {
                bound::set_texture(&mut l, game_result, b);
            }
            // A single-round mode's result: the banner alone.
            let info = l.tree.find_from(id, "result_info");
            bound::show(&mut l, info, board.lists_shown);
            bound::hide_samples(&mut l, id);
        }
        if !board.lists_shown {
            return Some(l);
        }
        let from = src.find(panel)?;
        let panel = l.tree.graft(0, &src.tree, from);
        for (i, (list_name, row_name)) in lists.iter().enumerate() {
            let Some(list) = l.tree.find_from(panel, list_name) else { continue };
            let Some(template) = src.find(row_name) else { continue };
            // The templates in the list: hidden.
            for c in l.tree.get(list).children.clone() {
                l.tree.get_mut(c).show = false;
            }
            for (k, row) in board.lists[i].iter().enumerate() {
                let id = l.tree.graft(list, &src.tree, template);
                let n = l.tree.get_mut(id);
                n.show = true;
                n.pos = IVec2::new(0, places::ROW.y as i32 * k as i32);
                bind_row(&mut l, id, k, row, board.order);
            }
        }
        // The team scores (`title_alpha`'s and `title_bravo`'s).
        for (i, name) in ["alpha_point", "bravo_point"].into_iter().enumerate() {
            let id = l.tree.find_from(panel, name);
            bound::set_text(&mut l, id, Some(&board.scores[i].to_string()), None);
        }
        // The headers: the mode's column only where it has one.
        let mission = board.order.and_then(|o| board.lists.iter().flatten().next().and_then(|r| r.mode_cell(o))).is_some();
        for name in ["menu_Mission"] {
            let text = l.tree.find_from(panel, name);
            let id = text.and_then(|t| l.tree.get(t).parent);
            bound::show(&mut l, id, mission);
            // S13's title of the mode's column.
            if let Some(title) = board.mode.as_deref().and_then(crate::game::rules::scoreboard::mode_column_title) {
                bound::set_text(&mut l, text, Some(title), None);
            }
        }
        for name in ["menu_SubMission", "menu_Clan"] {
            let id = l.tree.find_from(panel, name).and_then(|t| l.tree.get(t).parent);
            bound::show(&mut l, id, false);
        }
        if let Some(vip) = l.tree.find_from(panel, "menu_VIP_Eli") {
            l.tree.get_mut(vip).show = false;
        }
        for name in ["menu_Intels"] {
            let id = l.tree.find_from(panel, name).and_then(|t| l.tree.get(t).parent);
            bound::show(&mut l, id, false);
        }
        bound::hide_samples(&mut l, panel);
        Some(l)
    }
}

/// The ping icon for `ping` ms (ours: 4 under 50 ms, then 3, 2, 1 under
/// 100, 150, 250, else 0; the local player's, unknown, 4).
pub fn ping_icon(ping: Option<u32>) -> String {
    let n = match ping {
        None => 4,
        Some(p) if p < 50 => 4,
        Some(p) if p < 100 => 3,
        Some(p) if p < 150 => 2,
        Some(p) if p < 250 => 1,
        _ => 0,
    };
    format!("ui_hud_scoreboard/icon_ping_{n}.dtx")
}

/// One row's controls: name, kills, deaths, assists, the mode's column
/// (`User_Mission`, hidden where the mode has none), rank (its place, two
/// digits) and ping icon; what nothing drives hidden (level, clan, flag,
/// infection, name backgrounds, sub-mission).
fn bind_row(l: &mut Layout, row_id: WidgetId, k: usize, row: &ScoreRow, order: Option<ScoreOrder>) {
    let find = |l: &Layout, name: &str| l.tree.find_from(row_id, name);
    for (name, value) in [
        ("User_Name", row.name.clone()),
        ("User_Kill", row.kills.to_string()),
        ("User_Death", row.deaths.to_string()),
        ("User_Assist", row.assists.to_string()),
        ("User_Rank", format!("{:02}", k + 1)),
    ] {
        let id = find(l, name);
        bound::set_text(l, id, Some(&value), None);
    }
    let mission = order.and_then(|o| row.mode_cell(o));
    let id = find(l, "User_Mission");
    match mission {
        Some(v) => bound::set_text(l, id, Some(&v), None),
        None => {
            let parent = id.and_then(|i| l.tree.get(i).parent);
            bound::show(l, parent, false);
        }
    }
    let id = find(l, "User_Ping");
    bound::set_texture(l, id, &ping_icon(row.ping));
    for name in [
        "Infection",
        "User_LevelImg",
        "User_NationFlag",
        "User_ClanBG",
        "User_ClanEmblem",
        "User_Em/Sem",
        "NameBG_Camo",
        "NameBG_Line",
    ] {
        let id = find(l, name);
        bound::show(l, id, false);
    }
    for name in ["Clan_Name", "User_SubMission", "User_Spy_Kill", "User_Intel", "User_Score"] {
        let id = find(l, name).and_then(|i| l.tree.get(i).parent);
        bound::show(l, id, false);
    }
}

/// The board drawn (after its state).
pub struct CaScoreboardPlugin;

impl Plugin for CaScoreboardPlugin {
    fn build(&self, app: &mut App) {
        let data = crate::game::data::CaData::of(app);
        let ui = app.world().get_resource::<CaUi>().cloned().unwrap_or_else(|| CaUi::installed(&data));
        app.insert_resource(BoundBoard::of(&ui));
        app.add_plugins(CaScoreboardStatePlugin)
            .init_resource::<CaUi>()
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((BoardRoot, layer(45)));
            })
            .add_systems(Update, draw.after(update));
    }
}

#[derive(Component)]
struct BoardRoot;

#[allow(clippy::too_many_arguments)]
fn draw(
    board: Res<CaScoreboard>,
    screen: Option<Res<HudScreen>>,
    params: PenParams,
    art: Option<Res<super::draw::CaArt>>,
    bound_board: Res<BoundBoard>,
    root: Single<Entity, With<BoardRoot>>,
    mut last: Local<Option<(CaScoreboard, Screen, bool)>>,
    mut commands: Commands,
) {
    let screen = screen.map_or(Screen::new(1024.0, 768.0), |s| s.0);
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    if !rebuild(&mut last, (board.clone(), screen, loaded), *root, &mut commands) || !board.shown {
        return;
    }
    // The game's own scoreboard, bound by name (`BoundBoard`).
    if let Some(layout) = bound_board.build(&board, crate::ui::hud::scale(screen)) {
        let ui = &params.0;
        let draws = layout.draw(&ui.ctx());
        commands.entity(*root).with_children(|c| {
            super::draw::spawn_draws(c, &draws, &ui.stand_ins, art.as_deref(), params.1.as_deref(), params.2.as_deref());
        });
        return;
    }
    let pen = pen(&params, screen);
    let left = (HAlign::Default, VAlign::Centre);
    let centred = (HAlign::Centre, VAlign::Centre);
    let white = [230, 230, 230, 255];
    commands.entity(*root).with_children(|p| {
        // PLACEHOLDER for the result banner: its name over the panel.
        if let Some(r) = board.result {
            let at = (places::PANEL.0 - Vec2::Y * 40.0, Vec2::new(places::PANEL.1.x, 36.0), Vec2::ZERO);
            hud_text(p, pen, at, &format!("{r:?}").to_uppercase(), "bold 30", centred, Effect::Shadow, white);
        }
        if !board.lists_shown {
            return;
        }
        p.spawn(rect(screen.rect(places::PANEL.0, places::PANEL.1), Color::srgba(0.04, 0.05, 0.06, 0.82)));
        for (i, list) in board.lists.iter().enumerate() {
            if board.headers[i].is_empty() {
                continue;
            }
            let header = if board.headers[i] == "Players" {
                board.headers[i].clone()
            } else {
                format!("{}  {}", board.headers[i], board.scores[i])
            };
            hud_text(p, pen, (places::HEADERS[i], places::ROW, Vec2::ZERO), &header, "bold 16", left, Effect::Shadow, white);
            for (k, row) in list.iter().enumerate() {
                let at = places::LISTS[i] + Vec2::Y * places::ROW.y * k as f32;
                p.spawn(rect(screen.rect(at, places::ROW - Vec2::Y), Color::srgba(1.0, 1.0, 1.0, 0.05)));
                let ping = row.ping.map_or(String::new(), |p| p.to_string());
                let mode_column = board
                    .order
                    .and_then(|o| row.mode_cell(o))
                    .map_or(String::new(), |v| v.to_string());
                let cells = [
                    row.name.clone(),
                    mode_column,
                    row.kills.to_string(),
                    row.deaths.to_string(),
                    row.assists.to_string(),
                    ping,
                ];
                for (c, ((x, w), text)) in places::COLUMNS.iter().zip(cells).enumerate() {
                    let align = if c == 0 { left } else { centred };
                    let cell = (at + Vec2::X * *x, Vec2::new(*w, places::ROW.y), Vec2::ZERO);
                    hud_text(p, pen, cell, &text, "bold 14", align, Effect::Shadow, white);
                }
            }
        }
    });
}

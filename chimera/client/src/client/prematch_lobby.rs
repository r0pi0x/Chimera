//! The pre-match lobby's screen (`chimera_net::prematch`: a server holding
//! a new match until every player has its map), drawn by the shell over
//! the frozen map and its HUD, and over the loading panel of a player
//! still downloading (an app whose menu draws on its own camera moves
//! `LobbyRoot` there, as the party apps' main menu does with the console).
//!
//! - **The head**: the mode (the minigame's name), the map big, and what
//!   it waits for: "Waiting for 2 players..." with the time left of the
//!   longest wait ("starts anyway in 1:12"), or once counting down who
//!   it starts without.
//! - **The players**, one row each in the server's order (humans, then
//!   bots): the name, the status in its colour (connecting, downloading,
//!   loading the map, ready), for a downloader a bar with "MB done / total"
//!   and the percentage, and the ping (or "BOT"); the local player's row
//!   on the gold band the match-end screen gives it (found by its client
//!   id: `net::client::LocalClientId`, a listen host's `HOST_ID`).
//! - **The countdown** under it, big: "Match starts in" and the seconds.
//!
//! In the match-end screen's face and shades (`match_end`: the HUD's
//! overlay font, `widgets::display_alpha`), sized by the window's height
//! and the rows (`scale`). `LobbyView` is the state, headless too (tests
//! read it); drawing is Bevy UI rebuilt when it changes. Nothing shows
//! without a lobby. Part of `ClientPlugins`.
//!
//! Known shortcuts: no sound for the countdown.

use bevy::{prelude::*, window::PrimaryWindow};
use chimera_net::prematch::{NetLobby, status};
use chimera_sim::minigame::vote;

use crate::{
    core::{LocalPlayer, NetRole},
    net::HOST_ID,
};

/// Adds the screen's root and its state.
pub struct PreMatchLobbyPlugin;

impl Plugin for PreMatchLobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LobbyView>()
            .add_systems(Startup, spawn)
            .add_systems(Update, (follow, draw).chain());
    }
}

/// One player's row as shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LobbyViewRow {
    pub name: String,
    pub bot: bool,
    /// `prematch::status::*`.
    pub status: u8,
    /// The status's words.
    pub state: String,
    /// Downloading: the share done (None: no bar).
    pub fraction: Option<f32>,
    /// Downloading: "12.3 / 80.0 MB".
    pub bytes: String,
    /// "48 ms", "BOT" or empty.
    pub ping: String,
    pub local: bool,
}

/// What the screen shows (headless too).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LobbyView {
    pub shown: bool,
    /// The minigame's name.
    pub mode: String,
    /// The map's name (without its game).
    pub map: String,
    /// What it waits for.
    pub line: String,
    pub rows: Vec<LobbyViewRow>,
    /// Seconds left of the countdown, once it began.
    pub countdown: Option<u32>,
}

/// "1:12" or "45 s".
fn duration(secs: u32) -> String {
    if secs >= 60 { format!("{}:{:02}", secs / 60, secs % 60) } else { format!("{secs} s") }
}

/// The line under the map: what the lobby waits for.
pub fn waiting_line(l: &NetLobby) -> String {
    let not_ready = l.rows.iter().filter(|r| !r.ready()).count();
    if l.countdown.is_some() {
        return match not_ready {
            0 => "Everyone is ready".into(),
            1 => "Starting without 1 player still loading (they join when ready)".into(),
            n => format!("Starting without {n} players still loading (they join when ready)"),
        };
    }
    let who = match l.waiting {
        0 => "Waiting for the round...".to_string(),
        1 => "Waiting for 1 player...".to_string(),
        n => format!("Waiting for {n} players..."),
    };
    match l.wait_left {
        Some(s) => format!("{who}   starts anyway in {}", duration(s)),
        None => who,
    }
}

/// A lobby row as shown, `local` for this machine's player.
pub fn view_row(r: &chimera_net::prematch::LobbyRow, local: bool) -> LobbyViewRow {
    let downloading = r.status == status::DOWNLOADING;
    let mb = |b: u64| b as f64 / 1e6;
    let (fraction, bytes) = match (downloading, r.total) {
        (true, 0) => (Some(0.0), if r.done > 0 { format!("{:.1} MB", mb(r.done)) } else { String::new() }),
        (true, t) => (
            Some((r.done as f64 / t as f64).clamp(0.0, 1.0) as f32),
            format!("{:.1} / {:.1} MB", mb(r.done), mb(t)),
        ),
        _ => (None, String::new()),
    };
    LobbyViewRow {
        name: r.name.clone(),
        bot: r.bot(),
        status: r.status,
        state: match (r.bot(), downloading, fraction) {
            (true, ..) => "Ready".into(),
            (_, true, Some(f)) if r.total > 0 => format!("Downloading {:.0}%", f * 100.0),
            _ => status::name(r.status).into(),
        },
        fraction,
        bytes,
        ping: match (r.bot(), r.ping_ms) {
            (true, _) => "BOT".into(),
            (_, Some(p)) => format!("{p} ms"),
            _ => String::new(),
        },
        local,
    }
}

fn follow(
    lobby: Query<&NetLobby>,
    me: Option<Res<chimera_net::net::client::LocalClientId>>,
    role: Option<Res<NetRole>>,
    host: Query<(), With<LocalPlayer>>,
    mut view: ResMut<LobbyView>,
) {
    let Some(l) = lobby.iter().next() else {
        if view.shown {
            view.set_if_neq(LobbyView::default());
        }
        return;
    };
    let mine = match role.as_deref() {
        Some(NetRole::Client) => me.map(|m| m.0),
        Some(NetRole::Server) if !host.is_empty() => Some(HOST_ID),
        _ => None,
    };
    let v = LobbyView {
        shown: true,
        mode: l.mode.clone(),
        map: vote::map_name(&l.map).to_string(),
        line: waiting_line(l),
        rows: l.rows.iter().map(|r| view_row(r, mine.is_some() && r.id == mine)).collect(),
        countdown: l.countdown,
    };
    view.set_if_neq(v);
}

/// The screen's root (hidden while there is no lobby).
#[derive(Component)]
pub struct LobbyRoot;

/// What `draw` rebuilds.
#[derive(Component)]
struct LobbyContent;

/// Where it draws: over the HUDs, the match-end screen
/// (`match_end::MATCH_END_Z`) and an app's loading panel, under the Tab
/// scoreboard (`minigame_scoreboard::SCOREBOARD_Z`).
pub const LOBBY_Z: i32 = 77;

const TEXT: Color = Color::srgb(0.95, 0.95, 0.95);
const DIM: Color = Color::srgb(0.7, 0.7, 0.72);
const GOLD: Color = Color::srgb(1.0, 0.82, 0.25);
const LOCAL_BG: Color = Color::srgba(0.85, 0.62, 0.1, 0.32);
const READY: Color = Color::srgb(0.45, 0.88, 0.45);
const DOWNLOADING: Color = Color::srgb(1.0, 0.72, 0.25);
const LOADING: Color = Color::srgb(0.5, 0.75, 1.0);
const BAR_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.12);
/// Name, status, progress (bar and bytes), ping, on a 720-line screen.
const WIDTHS: [f32; 4] = [220.0, 170.0, 250.0, 70.0];
const GAP: f32 = 16.0;
const CELL: f32 = 17.0;

fn status_color(s: u8) -> Color {
    match s {
        status::READY => READY,
        status::DOWNLOADING => DOWNLOADING,
        status::LOADING => LOADING,
        _ => DIM,
    }
}

/// The screen's scale at a window `height` pixels tall with `rows` rows:
/// for a 720-line screen, less when that many rows wouldn't fit.
pub fn scale(height: f32, rows: usize) -> f32 {
    // From the top: the padding, the panel (its padding, three lines, the
    // table's margin and padding, the header and the rows with their
    // gaps), the countdown's box; a fifth over for the fonts' line boxes.
    let panel = 2.0 * 14.0 + (17.0 + 34.0 + 20.0) * 1.3 + 2.0 * 6.0 + 10.0 + 2.0 * 8.0 + (CELL * 1.3 + 4.0 + 4.0) * (rows + 1) as f32;
    let countdown = 12.0 + 2.0 * 4.0 + (22.0 + 64.0) * 1.3;
    let need = 0.15 * 720.0 + 1.2 * (panel + countdown) + 10.0;
    height.max(240.0) / 720f32.max(need)
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        LobbyRoot,
        Visibility::Hidden,
        GlobalZIndex(LOBBY_Z),
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            // Under the games' minigame panel (its title and rung lines).
            padding: UiRect::top(percent(15.0)),
            ..default()
        },
    ));
}

fn label(p: &mut ChildSpawnerCommands, text: impl Into<String>, font: TextFont, color: Color) {
    p.spawn((Text::new(text.into()), font, TextColor(color), TextLayout::justify(Justify::Center)));
}

fn cell(p: &mut ChildSpawnerCommands, text: String, width: f32, right: bool, color: Color, font: &TextFont, s: f32) {
    p.spawn((
        Text::new(text),
        font.clone(),
        TextColor(color),
        TextLayout::new(if right { Justify::Right } else { Justify::Left }, LineBreak::NoWrap),
        Node {
            width: px(width * s),
            flex_shrink: 0.0,
            overflow: Overflow::clip(),
            ..default()
        },
    ));
}

#[allow(clippy::type_complexity)]
fn draw(
    view: Res<LobbyView>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut root: Query<(Entity, &mut Visibility), With<LobbyRoot>>,
    content: Query<Entity, With<LobbyContent>>,
    fonts: Option<Res<super::fonts::UiFonts>>,
    mut drawn: Local<Option<(LobbyView, u32)>>,
    mut commands: Commands,
) {
    let Some((root, mut visibility)) = root.iter_mut().next() else { return };
    let height = window.iter().next().map_or(720.0, |w| w.height());
    let s = scale(height, view.rows.len());
    let key = (s * 1000.0) as u32;
    if drawn.as_ref().is_some_and(|(v, k)| v == &*view && *k == key) && !fonts.as_ref().is_some_and(|f| f.is_changed()) {
        return;
    }
    *drawn = Some((view.clone(), key));
    visibility.set_if_neq(if view.shown { Visibility::Inherited } else { Visibility::Hidden });
    for e in &content {
        commands.entity(e).despawn();
    }
    if !view.shown {
        return;
    }
    let fonts = fonts.as_deref();
    let font = |size: f32, bold: bool| super::fonts::overlay_font(fonts, size * s, bold);
    let panel = commands
        .spawn((
            LobbyContent,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(6.0 * s),
                padding: UiRect::axes(px(26.0 * s), px(14.0 * s)),
                ..default()
            },
            BackgroundColor(super::widgets::display_alpha(Color::srgba(0.02, 0.02, 0.03, 0.72))),
            ChildOf(root),
        ))
        .id();
    let cell_font = font(CELL, false);
    let cell_bold = font(CELL, true);
    commands.entity(panel).with_children(|p| {
        label(p, view.mode.to_uppercase(), font(17.0, false), DIM);
        label(p, view.map.clone(), font(34.0, true), GOLD);
        label(p, view.line.clone(), font(20.0, false), TEXT);
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4.0 * s),
            margin: UiRect::top(px(10.0 * s)),
            padding: UiRect::all(px(8.0 * s)),
            ..default()
        })
        .insert(BackgroundColor(super::widgets::display_alpha(Color::srgba(0.0, 0.0, 0.0, 0.45))))
        .with_children(|t| {
            t.spawn(Node { column_gap: px(GAP * s), ..default() }).with_children(|r| {
                for (i, (h, w)) in ["Player", "Status", "Download", "Ping"].iter().zip(WIDTHS).enumerate() {
                    cell(r, h.to_string(), w, i == 3, DIM, &cell_font, s);
                }
            });
            for row in &view.rows {
                t.spawn((
                    Node {
                        column_gap: px(GAP * s),
                        padding: UiRect::vertical(px(2.0 * s)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(if row.local { super::widgets::display_alpha(LOCAL_BG) } else { Color::NONE }),
                ))
                .with_children(|r| {
                    let f = if row.local { &cell_bold } else { &cell_font };
                    let name_color = if row.local { Color::WHITE } else if row.bot { DIM } else { TEXT };
                    cell(r, row.name.clone(), WIDTHS[0], false, name_color, f, s);
                    cell(r, row.state.clone(), WIDTHS[1], false, status_color(row.status), f, s);
                    // The download: a bar and its bytes.
                    r.spawn(Node {
                        width: px(WIDTHS[2] * s),
                        flex_shrink: 0.0,
                        column_gap: px(8.0 * s),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|d| {
                        if let Some(fr) = row.fraction {
                            d.spawn((
                                Node {
                                    width: px(90.0 * s),
                                    height: px(8.0 * s),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                BackgroundColor(super::widgets::display_alpha(BAR_BG)),
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Node {
                                        width: percent(100.0 * fr),
                                        height: percent(100.0),
                                        ..default()
                                    },
                                    BackgroundColor(DOWNLOADING),
                                ));
                            });
                            d.spawn((
                                Text::new(row.bytes.clone()),
                                font(CELL - 3.0, false),
                                TextColor(TEXT),
                                TextLayout::new(Justify::Left, LineBreak::NoWrap),
                            ));
                        }
                    });
                    cell(r, row.ping.clone(), WIDTHS[3], true, DIM, &cell_font, s);
                });
            }
        });
    });
    if let Some(left) = view.countdown {
        commands.spawn((
            LobbyContent,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                margin: UiRect::top(px(12.0 * s)),
                padding: UiRect::axes(px(36.0 * s), px(4.0 * s)),
                ..default()
            },
            BackgroundColor(super::widgets::display_alpha(Color::srgba(0.02, 0.02, 0.03, 0.72))),
            ChildOf(root),
        ))
        .with_children(|c| {
            label(c, "Match starts in", font(22.0, false), TEXT);
            label(c, left.to_string(), font(64.0, true), GOLD);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_net::prematch::LobbyRow;

    #[test]
    fn rows_and_lines_read_as_the_lobby_stands() {
        let row = |id: Option<u64>, name: &str, status: u8, done: u64, total: u64| LobbyRow {
            id,
            name: name.into(),
            status,
            done,
            total,
            ping_ms: id.map(|_| 48),
        };
        let mut l = NetLobby {
            map: "cs_source:de_dust2".into(),
            mode: "Ultimate Gun Game".into(),
            rows: vec![
                row(Some(1), "Ann", status::READY, 0, 0),
                row(Some(2), "Ben", status::DOWNLOADING, 25_000_000, 100_000_000),
                row(Some(3), "Cat", status::LOADING, 0, 0),
                row(None, "Bot Dan", status::READY, 0, 0),
            ],
            waiting: 2,
            wait_left: Some(72),
            countdown: None,
        };
        assert_eq!(waiting_line(&l), "Waiting for 2 players...   starts anyway in 1:12");
        let ben = view_row(&l.rows[1], true);
        assert_eq!((ben.state.as_str(), ben.bytes.as_str(), ben.fraction), ("Downloading 25%", "25.0 / 100.0 MB", Some(0.25)));
        assert!(ben.local);
        assert_eq!(view_row(&l.rows[3], false).ping, "BOT");
        assert_eq!(view_row(&l.rows[0], false).ping, "48 ms");
        assert_eq!(view_row(&l.rows[2], false).state, "Loading map");
        l.countdown = Some(5);
        assert_eq!(waiting_line(&l), "Starting without 2 players still loading (they join when ready)");
    }
}

//! The minigame scoreboard: hold `+showscores` (Tab) while a minigame
//! plays and every player and bot is listed, whatever game's gun or HUD
//! the local player holds: name, kills, deaths, the rung's gun (the
//! `chimera:ladder` state), ping and matches won
//! (`chimera_sim::minigame::wins`), best first (rung, then kills, then
//! fewest deaths). It is the shell's own overlay (not a `HudRoot`), so it
//! looks the same on CS:S's HUD, Combat Arms' and the minimal one; the
//! games' own scoreboards stay down while a minigame plays
//! (`MinigameHud::playing`), so plain CS:S keeps its own. An app adds
//! `MinigameScoreboardPlugin`.
//!
//! The gun is shown by its display name: `GunNames` (an app fills it from
//! the games' strings, as the minigame panel's `{weapon}` does), else the
//! weapon data's plain `hud.name` (`WeaponCatalog`), else the id's last
//! part in capitals. The columns have fixed widths (numbers right-aligned,
//! cells clipped to one line, however long Combat Arms' names run, and
//! centred in their row) so a header never runs into the next. The board
//! is drawn above the minigame panel with an opaque background.
//!
//! Known shortcuts: plain text, no team colours.

use std::collections::HashMap;

use bevy::prelude::*;
use chimera_sim::weapon::data::WeaponCatalog;
use chimera_sim::minigame::{
    hud::MinigameHud,
    ladder::{Ladder, Rung},
    runtime::InMinigame,
    wins::MatchesWon,
};

use super::console::HeldActions;
use crate::{
    core::{LocalPlayer, Score},
    net::{NetCharacter, NetScore, NetSettings, score_flags},
};

/// Adds the scoreboard overlay and its system.
pub struct MinigameScoreboardPlugin;

impl Plugin for MinigameScoreboardPlugin {
    fn build(&self, app: &mut App) {
        super::overlay::layer(app);
        app.init_resource::<HeldActions>().init_resource::<GunNames>().add_systems(Startup, spawn).add_systems(Update, draw);
        super::text_warm::TextWarmAppExt::warm_texts(app, warm_fonts);
    }
}

/// The rows' font, laid out ahead (`text_warm`) with the guns' names.
fn warm_fonts(names: Res<GunNames>) -> Vec<super::text_warm::WarmText> {
    vec![super::text_warm::WarmText { font: cell_font(), extra: names.0.values().flat_map(|v| v.chars()).collect() }]
}

/// The rows' cells' font.
fn cell_font() -> TextFont {
    TextFont { font_size: FontSize::Px(18.0), ..default() }
}

/// Guns' display names by weapon id (an app fills it; the minigame panel
/// names guns the same way).
#[derive(Resource, Clone, Debug, Default)]
pub struct GunNames(pub HashMap<String, String>);

/// What the board calls gun `id`: `names`, else the data's `hud.name`
/// (not a `#` string token), else the id's last part in capitals.
pub fn gun_name(id: &str, names: Option<&GunNames>, catalog: Option<&WeaponCatalog>) -> String {
    if let Some(n) = names.and_then(|n| n.0.get(id)) {
        return n.clone();
    }
    if let Some(n) = catalog.and_then(|c| c.get(id)).and_then(|d| d.hud.name.as_deref()).filter(|n| !n.starts_with('#')) {
        return n.to_string();
    }
    let last = id.rsplit(':').next().unwrap_or(id);
    let last = last.strip_prefix("weapon_").unwrap_or(last);
    last.chars().map(|c| if c == '_' || c == '-' { ' ' } else { c.to_ascii_uppercase() }).collect()
}

/// The overlay's root (hidden unless the key is held).
#[derive(Component)]
pub struct ScoreboardRoot;

/// The column titles' row.
#[derive(Component)]
pub struct ScoreboardHeader;

/// One player's row; the cells are its children.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ScoreboardRow {
    pub name: String,
    pub kills: i32,
    pub deaths: u32,
    /// The rung's gun (empty off a ladder).
    pub gun: String,
    /// Ping in ms (0: the host's, or not measured yet).
    pub ping: u16,
    pub bot: bool,
    pub wins: u32,
    pub local: bool,
}

/// Where it draws: over the HUDs and the match-end screen.
pub const SCOREBOARD_Z: i32 = 80;

const COLOR: Color = Color::srgb(0.95, 0.95, 0.95);
const LOCAL: Color = Color::srgb(1.0, 0.85, 0.3);
/// Name, Kills, Deaths, Gun, Ping, Wins.
pub const WIDTHS: [f32; 6] = [240.0, 80.0, 90.0, 260.0, 80.0, 80.0];
const GAP: f32 = 10.0;
/// A row's height: a line of the cells' font and a little.
const ROW_HEIGHT: f32 = 24.0;
const BOARD_WIDTH: f32 = 840.0 + 5.0 * GAP + 24.0;

fn row_node() -> Node {
    Node { column_gap: px(GAP), align_items: AlignItems::Center, min_height: px(ROW_HEIGHT), ..default() }
}

fn spawn(layer: Res<super::overlay::OverlayLayer>, mut commands: Commands) {
    commands.spawn((
        ScoreboardRoot,
        ChildOf(layer.0),
        Visibility::Hidden,
        GlobalZIndex(SCOREBOARD_Z),
        BackgroundColor(Color::srgb(0.04, 0.04, 0.05)),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50.0),
            top: percent(12.0),
            margin: UiRect::left(px(-BOARD_WIDTH / 2.0)),
            width: px(BOARD_WIDTH),
            padding: UiRect::all(px(12.0)),
            flex_direction: FlexDirection::Column,
            row_gap: px(4.0),
            ..default()
        },
    ));
}

fn cell(parent: &mut ChildSpawnerCommands, text: String, width: f32, right: bool, color: Color) {
    parent.spawn((
        Text::new(text),
        cell_font(),
        TextColor(color),
        TextLayout::new(if right { Justify::Right } else { Justify::Left }, LineBreak::NoWrap),
        Node { width: px(width), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() },
    ));
}

/// The rows, best first: rung, kills, fewest deaths, name.
pub fn sorted(mut rows: Vec<(usize, ScoreboardRow)>) -> Vec<ScoreboardRow> {
    rows.sort_by(|(ra, a), (rb, b)| {
        rb.cmp(ra)
            .then(b.kills.cmp(&a.kills))
            .then(a.deaths.cmp(&b.deaths))
            .then(a.name.cmp(&b.name))
    });
    rows.into_iter().map(|(_, r)| r).collect()
}

#[allow(clippy::type_complexity)]
fn draw(
    held: Res<HeldActions>,
    hud: Option<Res<MinigameHud>>,
    ladder: Option<Res<Ladder>>,
    settings: Option<Res<NetSettings>>,
    gun_names: Res<GunNames>,
    catalog: Option<Res<WeaponCatalog>>,
    players: Query<
        (
            Entity,
            Option<&Rung>,
            Option<&Score>,
            Option<&MatchesWon>,
            Option<&NetScore>,
            Option<&NetCharacter>,
            Option<&Name>,
            Has<LocalPlayer>,
        ),
        With<InMinigame>,
    >,
    mut root: Single<(Entity, &mut Visibility), With<ScoreboardRoot>>,
    old: Query<Entity, Or<(With<ScoreboardRow>, With<ScoreboardHeader>)>>,
    mut commands: Commands,
) {
    let show = held.showscores && hud.is_some_and(|h| h.playing());
    root.1.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    for e in &old {
        commands.entity(e).despawn();
    }
    if !show {
        return;
    }
    let rows: Vec<(usize, ScoreboardRow)> = players
        .iter()
        .map(|(e, rung, score, won, net, ch, name, local)| {
            let rung = rung.map_or(0, |r| r.0);
            let s = score.copied().unwrap_or_default();
            let gun = ladder
                .as_ref()
                .and_then(|l| l.shown_weapon(rung, catalog.as_deref()))
                .map(|id| gun_name(id, Some(&gun_names), catalog.as_deref()))
                .unwrap_or_default();
            (
                rung,
                ScoreboardRow {
                    name: super::shown_name(e, local, ch, name, settings.as_deref()),
                    kills: s.kills,
                    deaths: s.deaths,
                    gun,
                    ping: net.map_or(0, |n| n.ping),
                    bot: net.is_some_and(|n| n.flags & score_flags::BOT != 0),
                    wins: won.map_or(0, |w| w.0),
                    local,
                },
            )
        })
        .collect();
    let rows = sorted(rows);
    let root_e = root.0;
    commands.entity(root_e).with_children(|p| {
        p.spawn((ScoreboardHeader, row_node())).with_children(|r| {
            for (i, (h, w)) in ["Name", "Kills", "Deaths", "Gun", "Ping", "Wins"].iter().zip(WIDTHS).enumerate() {
                cell(r, h.to_string(), w, i != 0 && i != 3, COLOR);
            }
        });
        for row in rows {
            let color = if row.local { LOCAL } else { COLOR };
            let ping = if row.bot { "BOT".to_string() } else { row.ping.to_string() };
            let cells = [
                (row.name.clone(), false),
                (row.kills.to_string(), true),
                (row.deaths.to_string(), true),
                (row.gun.clone(), false),
                (ping, true),
                (row.wins.to_string(), true),
            ];
            p.spawn((row, row_node())).with_children(|r| {
                for ((text, right), w) in cells.into_iter().zip(WIDTHS) {
                    cell(r, text, w, right, color);
                }
            });
        }
    });
}

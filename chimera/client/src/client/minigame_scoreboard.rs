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
//! The gun is shown by its short name (`gun_short_name`). `GunNames` holds
//! every gun's full name (an app fills it from the games' strings, as the
//! minigame panel's `{weapon}` does; else the weapon data's plain
//! `hud.name`, `WeaponCatalog`, else the id's last part in capitals:
//! `gun_name`) and its short one (`fill_short_names`, from the games'
//! display_names.toml overrides and the automatic split,
//! `chimera_sim::weapon::names::GunNameRules`), for the narrow places:
//! this board's gun column, the callouts, the kill feeds, the match-end
//! rows. The `gun_names [game]` console command lists them. The columns
//! have fixed widths (numbers right-aligned, cells on one line and centred
//! in their row) so a header never runs into the next; a name too wide for
//! its cell is fitted with the font (`overlay::fit`: condensed, then cut
//! at a word with `…`). The board is drawn above the minigame panel with an
//! opaque background.
//!
//! Under the Signal look (`signal::signal_on`) a ladder's Tab board is the
//! race chip's (`race_chip`); this one stays for other minigames and apps.
//! `MinigameScoreboardPlugin` adds `RaceChipPlugin`.
//!
//! Known shortcuts: plain text, no team colours.

use std::collections::HashMap;

use bevy::prelude::*;
use chimera_sim::weapon::{data::WeaponCatalog, names::GunNameRules};
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
        app.init_resource::<HeldActions>()
            .init_resource::<GunNames>()
            .add_systems(Startup, spawn)
            .add_systems(PreUpdate, fill_short_names)
            .add_systems(Update, draw);
        if !app.is_plugin_added::<super::race_chip::RaceChipPlugin>() {
            app.add_plugins(super::race_chip::RaceChipPlugin);
        }
        super::text_warm::TextWarmAppExt::warm_texts(app, warm_fonts);
        chimera_core::console::ConsoleAppExt::console_command(
            app,
            "chimera:gun_names",
            "gun_names [game]: every gun's id, full name and short name (the narrow places': the scoreboard, \
             callouts, kill feeds) and where the short one came from (override, split or full); a game id keeps \
             its guns only.",
            |w, a| Ok(Some(gun_names_lines(w, a.first().map(String::as_str)))),
        );
    }
}

/// The rows' font, laid out ahead (`text_warm`) with the guns' names.
fn warm_fonts(names: Res<GunNames>) -> Vec<super::text_warm::WarmText> {
    vec![super::text_warm::WarmText { font: cell_font(), extra: names.short.values().flat_map(|v| v.chars()).collect() }]
}

/// The rows' cells' font.
fn cell_font() -> TextFont {
    TextFont { font_size: FontSize::Px(18.0), ..default() }
}

/// Guns' display names by weapon id: `full` (an app fills it from the
/// games' strings; the minigame panel names guns the same way) and `short`
/// (`fill_short_names` fills it for every gun the catalog or `full` knows).
#[derive(Resource, Clone, Debug, Default)]
pub struct GunNames {
    pub full: HashMap<String, String>,
    pub short: HashMap<String, String>,
}

impl GunNames {
    /// Full names alone (the short ones come with `fill_short_names`).
    pub fn from_full(names: impl IntoIterator<Item = (String, String)>) -> Self {
        GunNames { full: names.into_iter().collect(), short: HashMap::new() }
    }
}

/// Gun `id`'s full name: `names`' full one, else the data's `hud.name`
/// (not a `#` string token), else the id's last part in capitals.
pub fn gun_name(id: &str, names: Option<&GunNames>, catalog: Option<&WeaponCatalog>) -> String {
    if let Some(n) = names.and_then(|n| n.full.get(id)) {
        return n.clone();
    }
    game_name(id, catalog)
}

/// The data's `hud.name` (not a string token), else the id's last part in
/// capitals.
fn game_name(id: &str, catalog: Option<&WeaponCatalog>) -> String {
    if let Some(n) = catalog.and_then(|c| c.get(id)).and_then(|d| d.hud.name.as_deref()).filter(|n| !n.starts_with('#')) {
        return n.to_string();
    }
    chimera_sim::weapon::names::id_name(id)
}

/// Gun `id`'s short name, for the narrow places: `names`' short one, else
/// its full name (`gun_name`).
pub fn gun_short_name(id: &str, names: Option<&GunNames>, catalog: Option<&WeaponCatalog>) -> String {
    if let Some(n) = names.and_then(|n| n.short.get(id)) {
        return n.clone();
    }
    gun_name(id, names, catalog)
}

/// Every gun's short name (and an override's full one), from the games'
/// rules (`GunNameRules`), when the names, the rules or the catalog change:
/// each gun the catalog or the full names know.
fn fill_short_names(
    mut names: ResMut<GunNames>,
    rules: Option<Res<GunNameRules>>,
    catalog: Option<Res<WeaponCatalog>>,
) {
    let changed = names.is_changed()
        || rules.as_ref().is_some_and(|r| r.is_changed())
        || catalog.as_ref().is_some_and(|c| c.is_changed());
    if !changed {
        return;
    }
    let empty = GunNameRules::default();
    let rules = rules.as_deref().unwrap_or(&empty);
    let names = names.bypass_change_detection();
    fill(names, rules, catalog.as_deref());
}

/// `names`' short names (and overrides' full ones) by `rules`.
pub fn fill(names: &mut GunNames, rules: &GunNameRules, catalog: Option<&WeaponCatalog>) {
    let mut ids: Vec<String> = names.full.keys().cloned().collect();
    if let Some(c) = catalog {
        ids.extend(c.sets.iter().flat_map(|s| s.weapons.iter()).map(|w| w.id.clone()).filter(|id| !names.full.contains_key(id)));
    }
    for id in ids {
        let game = names.full.get(&id).cloned().unwrap_or_else(|| game_name(&id, catalog));
        let full = rules.full(&id, &game);
        if full != game {
            names.full.insert(id.clone(), full.clone());
        }
        let short = rules.short(&id, &full).0;
        names.short.insert(id, short);
    }
}

/// The `gun_names` command's lines: id, full, short and where it came
/// from, by id; `game` keeps that game's.
fn gun_names_lines(w: &mut World, game: Option<&str>) -> String {
    let empty = GunNameRules::default();
    let rules = w.get_resource::<GunNameRules>().unwrap_or(&empty);
    let catalog = w.get_resource::<WeaponCatalog>();
    let mut names = w.get_resource::<GunNames>().cloned().unwrap_or_default();
    fill(&mut names, rules, catalog);
    let mut ids: Vec<&String> = names.short.keys().filter(|id| game.is_none_or(|g| id.split_once(':').is_some_and(|(p, _)| p == g))).collect();
    ids.sort();
    if ids.is_empty() {
        return "no guns".into();
    }
    ids.into_iter()
        .map(|id| {
            let full = gun_name(id, Some(&names), catalog);
            let (short, from) = rules.short(id, &full);
            format!("{id} | {full} | {short} ({})", from.name())
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    (fonts, rules, race): (Option<Res<super::fonts::UiFonts>>, Option<Res<GunNameRules>>, Res<super::race_chip::RaceView>),
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
    // Under the Signal look a ladder's board is the race chip's.
    let show = held.showscores && hud.is_some_and(|h| h.playing()) && !race.active;
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
                .map(|id| gun_short_name(id, Some(&gun_names), catalog.as_deref()))
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
                (super::overlay::fit(fonts.as_deref(), rules.as_deref(), &cell_font(), &row.gun, WIDTHS[3]), false),
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

//! The match-end screen of a minigame (Ultimate Gun Game, Lucker Party),
//! drawn by the shell over whichever HUD is up (CS:S's, Combat Arms', the
//! minimal one) for the match-end hold: from the match decided with its
//! results (`chimera_sim::minigame::MatchResults`, through `hud::
//! MinigameHud::results`) while `minigame::NextMatch` is in, through the
//! map change (`MatchHold`) until the next match starts. The win itself
//! (`match_won`) has the round over to its own callout, everyone already
//! frozen (`minigame::runtime`'s end freeze); this comes up at the
//! round's end, after it.
//!
//! - **The winner** big ("You win the match!" under it for the winner
//!   itself), or "Draw".
//! - **The final standings**, best first, in the Tab scoreboard's
//!   columns (`minigame_scoreboard`: name as the board names it, kills,
//!   deaths, the final rung's gun by `minigame_scoreboard::gun_name`,
//!   matches won), with the place; the local player's row highlighted
//!   (found by its character, which the network maps to the client's).
//! - **The next map** and a countdown to the change ("Next map: de_dust2
//!   in 5", then "Changing map to de_dust2"), from `NextMatch` (a
//!   server's, replicated: `chimera_net::minigame_net`).
//! - **A winner sound** once as the screen comes up after a decided match:
//!   a file from an install, chosen by data, each game's
//!   `GameParts::match_won_sound`, or to a player who lost its
//!   `match_lost_sound` where it has one (`PartsRegistry::
//!   match_end_sounds`: the HUD's game first, then the others in the
//!   order registered; the first that loads plays, none if none does), an
//!   interface sound at the mixer's master.
//!
//! While it is up the games' minigame panels draw nothing
//! (`MinigameHud::results_shown`): their title and status lines sat over
//! the winner's name. Sized by the window's height (`scale`: for a
//! 720-line screen, as the kill feed; smaller when the standings have
//! too many rows to fit), in the HUD's face (`fonts::overlay_font`: CS:S's
//! centre-print Trebuchet MS; the winner and the highlighted rows bold),
//! its shades as they would be blended in display space
//! (`widgets::display_alpha`). `MatchEndView` is the state,
//! headless too (tests read it); drawing is Bevy UI. Nothing shows
//! without a minigame, so plain CS:S and Combat Arms are unchanged. Part of `ClientPlugins`.
//!
//! Known shortcuts: no map vote (the server's rotation picks the next
//! map); text and colours in CS:S's face, no game's art; no music pause.

use std::sync::Arc;

use bevy::{prelude::*, window::PrimaryWindow};
use chimera_core::game_parts::PartsRegistry;
use chimera_sim::{
    minigame::{NextMatch, hud::MinigameHud},
    weapon::data::WeaponCatalog,
};

use super::minigame_scoreboard::{GunNames, gun_name};
use crate::{
    audio::{
        clip_asset::SoundAsset,
        live_sound::{Gains, LiveClip},
    },
    core::{LocalPlayer, SimClock},
    gameplay::Gameplay,
    net::{NetCharacter, NetSettings},
};

/// Adds the screen's nodes, its state and its winner sound.
pub struct MatchEndPlugin;

impl Plugin for MatchEndPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MatchEndView>()
            .add_systems(Startup, spawn)
            .add_systems(Update, (follow, draw).chain());
        if app.world().contains_resource::<AssetServer>() {
            app.init_resource::<WinSound>()
                .add_systems(Update, play_sound.after(follow).run_if(resource_exists::<Assets<SoundAsset>>));
        }
    }
}

/// One player's line of the final standings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MatchEndRow {
    /// From 1.
    pub place: usize,
    pub name: String,
    pub kills: i32,
    pub deaths: u32,
    /// The final rung's gun's display name (empty off a ladder).
    pub gun: String,
    pub wins: u32,
    pub winner: bool,
    pub local: bool,
}

/// What the screen shows (headless too).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MatchEndView {
    pub shown: bool,
    /// The minigame's name.
    pub title: String,
    /// The winner's name; none for a draw.
    pub winner: Option<String>,
    /// The local player won.
    pub you_won: bool,
    pub rows: Vec<MatchEndRow>,
    /// The next map and the countdown.
    pub next: String,
    /// The winner sound's candidates `(game, file)` asked for when the
    /// screen came up, in order (the player takes them).
    pub sound: Vec<(String, String)>,
    /// How many times a winner sound was asked for.
    pub sounds_asked: u32,
}

/// The countdown line `left` seconds before the change to `next`'s map.
pub fn next_line(next: &NextMatch, left: f64) -> String {
    let left = left.ceil() as i64;
    match (next.map_name(), left > 0) {
        (Some(m), true) => format!("Next map: {m} in {left}"),
        (Some(m), false) => format!("Changing map to {m}"),
        (None, true) => format!("Next match in {left}"),
        (None, false) => "Next match starting".into(),
    }
}

/// The winner sound's candidates: each game's `match_won_sound`, `first`
/// (the HUD's game) before the others.
pub fn sound_candidates(parts: &PartsRegistry, first: Option<&str>) -> Vec<(String, String)> {
    parts.match_end_sounds(first, true)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn follow(
    mut hud: Option<ResMut<MinigameHud>>,
    next: Option<Res<NextMatch>>,
    clock: Option<Res<SimClock>>,
    gun_names: Option<Res<GunNames>>,
    catalog: Option<Res<WeaponCatalog>>,
    settings: Option<Res<NetSettings>>,
    parts: Option<Res<PartsRegistry>>,
    gameplay: Option<Res<Gameplay>>,
    chars: Query<(Option<&NetCharacter>, Option<&Name>, Has<LocalPlayer>)>,
    mut view: ResMut<MatchEndView>,
) {
    let results = hud.as_ref().filter(|h| h.playing()).and_then(|h| h.results.clone());
    let shown = results.is_some() && next.is_some();
    // The games' panels leave the results to this screen while it is up.
    if let Some(h) = hud.as_mut().filter(|h| h.results_shown != shown) {
        h.results_shown = shown;
    }
    let (Some(results), Some(next)) = (results, next) else {
        if view.shown {
            *view = MatchEndView {
                sounds_asked: view.sounds_asked,
                ..default()
            };
        }
        return;
    };
    let rows: Vec<MatchEndRow> = results
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let found = r.player.and_then(|e| chars.get(e).ok().map(|c| (e, c)));
            let local = found.is_some_and(|(_, (_, _, l))| l);
            let name = match found {
                Some((e, (net, name, l))) => super::shown_name(e, l, net, name, settings.as_deref()),
                None => r.name.clone(),
            };
            MatchEndRow {
                place: i + 1,
                name,
                kills: r.kills,
                deaths: r.deaths,
                gun: r.gun.as_deref().map(|g| gun_name(g, gun_names.as_deref(), catalog.as_deref())).unwrap_or_default(),
                wins: r.wins,
                winner: r.winner,
                local,
            }
        })
        .collect();
    let winner = rows.iter().find(|r| r.winner);
    let now = clock.map_or(0.0, |c| c.now);
    let line = next_line(&next, next.at - now);
    let came_up = !view.shown;
    let mut v = MatchEndView {
        shown: true,
        title: results.title.clone(),
        winner: winner.map(|w| w.name.clone()),
        you_won: winner.is_some_and(|w| w.local),
        rows: rows.clone(),
        next: line,
        sound: view.sound.clone(),
        sounds_asked: view.sounds_asked,
    };
    if came_up && v.winner.is_some() {
        v.sound = parts.map_or_else(Vec::new, |p| p.match_end_sounds(gameplay.as_ref().map(|g| g.hud), v.you_won));
        v.sounds_asked += 1;
    }
    view.set_if_neq(v);
}

/// The screen's root (hidden while there is nothing to show).
#[derive(Component)]
pub struct MatchEndRoot;

/// The headline, the line under it, the standings and the countdown.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchEndPart {
    Title,
    Winner,
    Subtitle,
    Standings,
    Next,
}

impl MatchEndPart {
    /// A text part's font size on a 720-line screen.
    pub fn size(self) -> f32 {
        match self {
            MatchEndPart::Title => 22.0,
            MatchEndPart::Winner => 64.0,
            MatchEndPart::Subtitle => 26.0,
            MatchEndPart::Standings => CELL,
            MatchEndPart::Next => 28.0,
        }
    }
}

/// Where it draws: over the HUDs, under the Tab scoreboard
/// (`minigame_scoreboard::SCOREBOARD_Z`) and an app's menu.
pub const MATCH_END_Z: i32 = 75;

const TEXT: Color = Color::srgb(0.95, 0.95, 0.95);
const DIM: Color = Color::srgb(0.7, 0.7, 0.72);
const GOLD: Color = Color::srgb(1.0, 0.82, 0.25);
/// The local player's row: white on a gold band (gold text on it read
/// poorly).
const LOCAL: Color = Color::WHITE;
const LOCAL_BG: Color = Color::srgba(0.85, 0.62, 0.1, 0.32);
/// Place, Name, Kills, Deaths, Gun, Wins (on a 720-line screen).
pub const WIDTHS: [f32; 6] = [50.0, 240.0, 80.0, 90.0, 260.0, 80.0];
const GAP: f32 = 18.0;
/// A standings cell's font size on a 720-line screen.
const CELL: f32 = 20.0;

/// The screen's scale at a window `height` pixels tall with `rows`
/// standings rows: sized for a 720-line screen (as the kill feed), less
/// when that many rows wouldn't fit (`layout_height`), never under a
/// third of it.
pub fn scale(height: f32, rows: usize) -> f32 {
    height.max(240.0) / 720f32.max(layout_height(rows))
}

/// The screen's height at scale 1 with `rows` standings rows (and the
/// header), a little over: the top padding (8 % of 720), the three lines
/// and the countdown at their line heights, the gaps, the board's
/// margins and padding, a row each, and room under it (seen at 1080
/// lines: ten rows at full size put the countdown on the screen's edge).
pub fn layout_height(rows: usize) -> f32 {
    const LINE: f32 = 1.3;
    let lines: f32 = [MatchEndPart::Title, MatchEndPart::Winner, MatchEndPart::Subtitle, MatchEndPart::Next]
        .iter()
        .map(|p| p.size() * LINE)
        .sum();
    let row = CELL * LINE + 4.0 + 4.0;
    57.6 + lines + 4.0 * 10.0 + 2.0 * 18.0 + 2.0 * 12.0 + row * (rows + 1) as f32 + 60.0
}

fn text(part: MatchEndPart, color: Color) -> impl Bundle {
    (
        part,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(part.size()),
            ..default()
        },
        TextColor(color),
        TextLayout::justify(Justify::Center),
    )
}

fn spawn(mut commands: Commands) {
    commands
        .spawn((
            MatchEndRoot,
            Visibility::Hidden,
            GlobalZIndex(MATCH_END_Z),
            // The shades as they would cover blended in display space
            // (`widgets::display_alpha`; Bevy's UI blends in linear light).
            BackgroundColor(super::widgets::display_alpha(Color::srgba(0.02, 0.02, 0.03, 0.72))),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                height: percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(percent(8.0)),
                row_gap: px(10.0),
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn(text(MatchEndPart::Title, DIM));
            c.spawn(text(MatchEndPart::Winner, GOLD));
            c.spawn(text(MatchEndPart::Subtitle, TEXT));
            c.spawn((
                MatchEndPart::Standings,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4.0),
                    margin: UiRect::vertical(px(18.0)),
                    padding: UiRect::all(px(12.0)),
                    ..default()
                },
                BackgroundColor(super::widgets::display_alpha(Color::srgba(0.0, 0.0, 0.0, 0.5))),
            ));
            c.spawn(text(MatchEndPart::Next, TEXT));
        });
}

/// One standings row's node.
#[derive(Component)]
struct StandingsRow;

fn cell(parent: &mut ChildSpawnerCommands, text: String, width: f32, right: bool, color: Color, font: &TextFont, s: f32) {
    parent.spawn((
        Text::new(text),
        font.clone(),
        TextColor(color),
        TextLayout::justify(if right { Justify::Right } else { Justify::Left }),
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
    view: Res<MatchEndView>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut root: Query<(&mut Visibility, &mut Node), (With<MatchEndRoot>, Without<MatchEndPart>)>,
    mut parts: Query<(Entity, &MatchEndPart, Option<&mut Text>, Option<&mut TextFont>, &mut Node), Without<MatchEndRoot>>,
    old: Query<Entity, With<StandingsRow>>,
    mut commands: Commands,
    mut drawn: Local<Option<(Option<String>, Vec<MatchEndRow>, u32)>>,
    mut sized: Local<Option<u32>>,
    fonts: Option<Res<super::fonts::UiFonts>>,
) {
    let height = window.iter().next().map_or(720.0, |w| w.height());
    let s = scale(height, view.rows.len());
    // The scale as a key: the window's height and the rows' count.
    let key = (s * 1000.0) as u32;
    let resized = *sized != Some(key) || fonts.as_ref().is_some_and(|f| f.is_changed());
    if !view.is_changed() && !resized {
        return;
    }
    *sized = Some(key);
    for (mut v, mut node) in &mut root {
        v.set_if_neq(if view.shown { Visibility::Inherited } else { Visibility::Hidden });
        if resized {
            node.row_gap = px(10.0 * s);
        }
    }
    let (headline, sub) = match (&view.winner, view.you_won) {
        (Some(w), true) => (w.clone(), "You win the match!".to_string()),
        (Some(w), false) => (w.clone(), "wins the match".to_string()),
        (None, _) => ("Draw".to_string(), "Nobody wins the match".to_string()),
    };
    let mut standings = None;
    for (e, part, t, font, mut node) in &mut parts {
        let want = match part {
            MatchEndPart::Title => view.title.clone(),
            MatchEndPart::Winner => headline.clone(),
            MatchEndPart::Subtitle => sub.clone(),
            MatchEndPart::Next => view.next.clone(),
            MatchEndPart::Standings => {
                standings = Some(e);
                if resized {
                    node.row_gap = px(4.0 * s);
                    node.margin = UiRect::vertical(px(18.0 * s));
                    node.padding = UiRect::all(px(12.0 * s));
                }
                continue;
            }
        };
        if let Some(mut f) = font.filter(|_| resized) {
            // In the HUD's face (`fonts::overlay_font`), the winner bold.
            *f = super::fonts::overlay_font(fonts.as_deref(), part.size() * s, *part == MatchEndPart::Winner);
        }
        if let Some(mut t) = t
            && t.0 != want
        {
            t.0 = want;
        }
    }
    // The standings again only when they changed (not each second).
    let now = (view.shown.then(|| view.title.clone()), view.rows.clone(), key);
    if drawn.as_ref() == Some(&now) && !resized {
        return;
    }
    *drawn = Some(now);
    for e in &old {
        commands.entity(e).despawn();
    }
    let Some(board) = standings.filter(|_| view.shown) else { return };
    let header = ["#", "Name", "Kills", "Deaths", "Gun", "Wins"];
    let font = super::fonts::overlay_font(fonts.as_deref(), CELL * s, false);
    let bold = super::fonts::overlay_font(fonts.as_deref(), CELL * s, true);
    commands.entity(board).with_children(|p| {
        p.spawn((StandingsRow, Node { column_gap: px(GAP * s), ..default() })).with_children(|r| {
            for (i, (h, w)) in header.iter().zip(WIDTHS).enumerate() {
                cell(r, h.to_string(), w, !matches!(i, 1 | 4), DIM, &font, s);
            }
        });
        for row in &view.rows {
            let color = if row.local { LOCAL } else if row.winner { GOLD } else { TEXT };
            let cells = [
                (row.place.to_string(), true),
                (row.name.clone(), false),
                (row.kills.to_string(), true),
                (row.deaths.to_string(), true),
                (row.gun.clone(), false),
                (row.wins.to_string(), true),
            ];
            p.spawn((
                StandingsRow,
                Node {
                    column_gap: px(GAP * s),
                    padding: UiRect::vertical(px(2.0 * s)),
                    ..default()
                },
                BackgroundColor(if row.local { LOCAL_BG } else { Color::NONE }),
            ))
            .with_children(|r| {
                for ((text, right), w) in cells.into_iter().zip(WIDTHS) {
                    cell(r, text, w, right, color, if row.local || row.winner { &bold } else { &font }, s);
                }
            });
        }
    });
}

/// The winner sound being loaded: the candidates left and the one
/// loading, since when.
#[derive(Resource, Default)]
struct WinSound {
    left: Vec<(String, String)>,
    loading: Option<(String, Handle<SoundAsset>, f64)>,
}

/// The first candidate that loads plays once (a missing file: the next;
/// none after 3 s of loading).
fn play_sound(
    mut view: ResMut<MatchEndView>,
    mut state: ResMut<WinSound>,
    server: Res<AssetServer>,
    time: Res<Time>,
    sounds: Res<Assets<SoundAsset>>,
    clips: Option<ResMut<Assets<LiveClip>>>,
    mut commands: Commands,
) {
    if !view.sound.is_empty() {
        state.left = std::mem::take(&mut view.bypass_change_detection().sound);
        state.loading = None;
    }
    let now = time.elapsed_secs_f64();
    loop {
        if state.loading.is_none() {
            if state.left.is_empty() {
                return;
            }
            let (game, file) = state.left.remove(0);
            let path = chimera_present::assets::install_path(&game, &file);
            state.loading = Some((path.clone(), server.load(path), now));
        }
        let Some((path, handle, since)) = state.loading.clone() else { return };
        if let Some(sound) = sounds.get(&handle) {
            if let Some(mut clips) = clips {
                let gains = Arc::new(Gains::new(1.0, 1.0));
                let clip = clips.add(LiveClip::new(sound.0.clone(), gains, default()));
                commands.spawn((AudioPlayer(clip), PlaybackSettings::DESPAWN));
                info!("match end: winner sound {path}");
            }
            *state = WinSound::default();
            return;
        }
        if server.load_state(&handle).is_failed() {
            info!("match end: no winner sound at {path}");
            state.loading = None;
            continue;
        }
        if now - since > 3.0 {
            *state = WinSound::default();
        }
        return;
    }
}

//! The match-end screen in the Signal look (docs/plans/active/ugg-ui.md
//! 3 and M4), drawn instead of the shell's (`match_end`) when the app's
//! theme turns the Signal HUD on (`signal::signal_on`: the Ultimate Gun
//! Game; Lucker Party and the plain apps keep theirs). Same state
//! (`match_end::MatchEndView`, from the results and the vote), same gate
//! (the logo pass in and out), same vote input; this is presentation.
//!
//! On the ink, a 16:9 frame (100 x 56.25 cq, centred). Blocks sit where
//! `[layout.matchend]` puts them (`EndLayout`):
//!
//! - **Leave** (`LeaveButton`), small, top left: an arrow and the word;
//!   leaves for the menu (`disconnect`, through the logo pass); the next
//!   match starts without you. The chat (`signal_chat`, docked with the
//!   same numbers) is the left column under it, down to the bottom:
//!   messages stacked at the bottom, the field below.
//! - **Standings** (`StandingsPanel`, `StandingsRow`), the right column
//!   (30 cq), top-aligned, down to above the footer: the winner's row
//!   first and taller (`WinnerRow`: trophy, accent frame, the name large
//!   with an accent slab sweeping across it, `WinnerName`/`WinnerSlab`:
//!   from `motion.dur.matchend.slab_at` over `slab`, the name shown at
//!   `name_at`; the sub-line "Winner . 14/14 . Knife", `winner_sub`; the
//!   kills), then the rest (position, the bot slot and the name with HOST
//!   (or the fused YOU+HOST chip) and the session wins (trophy and the
//!   count; "+1" on the winner's: the server's `ResultRow::wins`,
//!   `minigame::wins` per server run), rung/total, kills), shrinking to
//!   fit (down to 1.5 cq). Your row is tinted and outlined; hover is a
//!   faint fill; the selected row (a click; the winner's at first) has a
//!   stronger fill and a pointer.
//! - **Details** (`DetailsLine`), top centre: map (and its game), players,
//!   rungs and the match's time, each after an icon.
//! - **The climb** (`ClimbChart`), small (`graph_w` x `graph_h`) and
//!   centred in the middle column, with its **stat tiles**
//!   (`StatsRow`, `StatTile`; `stats_of`) directly under it: kills,
//!   deaths, K/D, best streak, fastest rung-up, longest stall; yours, else
//!   the winner's when you have no entry (`stats_subject`). Headshots are
//!   not tracked, so no tile.
//! - **The vote**: the cards (`VoteCard`), two to a row (a 2x2 grid for four maps; more maps add rows and the cards shrink to fit, picture first, down to a minimum height, then run on), fill the centre
//!   under the tiles (`vote_top`); the countdown is the footer, bottom right ("Next match 0:14 >", `VoteCountdownColumn`).
//!
//! More:
//!
//! - **The climb's chart**: rung against match time (to the
//!   match's length rounded up to the finest of 0:30, 1, 2, 5, 10 ...
//!   minute marks that leaves at most eight: `ChartAxes`) for the
//!   hovered row, else the selected one: the others' climbs faint, the
//!   chosen one's steps (`ClimbSeg`) drawn along their length in
//!   `climb_draw` s, its rung-ups, kills and deaths as ticks fading in
//!   along x over `climb_ticks` s (`ClimbTick`), its longest stall boxed
//!   (dashed) and labelled clear of the line on an ink plate
//!   (`StallLabel`), its final rung named.
//! - **The vote** (the footer): one countdown
//!   (`VoteCountdown` and its pixel-snapped tick bar, `VoteTicks`: lit as
//!   it runs, the accent from `vote_hot_from` of `vote` s), the vote's
//!   close and the next match's start at once (`seconds_left`); the map
//!   cards with their game, eight vote pips (yours in the accent) and the
//!   count (the leader's in the accent), your vote outlined in the
//!   accent and tagged; a click on a card or its number key votes
//!   (`match_end::VoteButton`, `vote_input`). At 0 the vote locks: the
//!   others dim, the winner is outlined and named under the countdown,
//!   and the logo pass takes everyone into the next match (`match_end`'s
//!   gate, a swap's length before `NextMatch::at`).
//! - **The ground** (`EndGround`): the shared ambient with the match
//!   end's bloom and the broad slanted planes over it at .6 % a layer
//!   (look book V's "ambient + planes"), both on the renderer's clock.
//!
//! Each part is rebuilt only when what it shows changes; the entrance
//! (fades and steps from the screen's coming up, `EndState::came_up`),
//! the slab and the climb's drawing run on the kit's clock (`UiClock`).
//! The cursor is free while the screen is up (`match_end::vote_cursor`);
//! the games' HUDs are hidden under it (`hide_huds`: `hud::HudCover`)
//! and shown again after.
//! `ui_scene matchend [fixture]` stages it (`stage`). Nodes carry the
//! look book's names (`UiParityId`: `end.*`).
//!
//! Known shortcuts: no gun on the standings rows (the winner's is in its
//! sub-line); no headshot tile (not tracked); over about 18 players the
//! standings clip (rows floor at 1.5 cq); map cards show a tinted
//! gradient (no map shots); a listen host's Leave gets no confirmation
//! (the Esc menu's warns about the people connected).

use bevy::{prelude::*, window::PrimaryWindow};
use chimera_sim::minigame::climb::Climb;
use chimera_ui::{Ellipsize, Prop, Role, SlantMaterial, Theme, TickBarMaterial, TickLayout, Timeline, Tone, Track, UiClock, UiParityId, parity};
use serde::Deserialize;

use super::{
    match_end::{MATCH_END_Z, MatchEndRow, MatchEndStaged, MatchEndView, MatchEndVote},
    signal::{self, Look, Mark, TagKind},
};

/// Whether the Signal screen draws (`flags.hud.signal`).
pub fn themed(theme: Option<Res<Theme>>) -> bool {
    signal::signal_on(theme.as_deref())
}

/// The screen's root.
#[derive(Component)]
pub struct SignalEndRoot;

/// The header, the winner and the standings (rebuilt together).
#[derive(Component)]
struct StaticPart;

/// The climb chart's part.
#[derive(Component)]
struct ClimbPart;

/// A standings row: its index in `MatchEndView::rows`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StandingsRow {
    pub index: usize,
    pub you: bool,
}

/// The standings' column.
#[derive(Component)]
pub struct StandingsPanel;

/// The winner's row, the standings' first.
#[derive(Component)]
pub struct WinnerRow;

/// The details line at the top: map, players, rungs, time.
#[derive(Component)]
pub struct DetailsLine;

/// The stat tiles' row under the climb.
#[derive(Component)]
pub struct StatsRow;

/// One stat tile.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatTile {
    pub label: &'static str,
}

/// A stat's label and value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndStat {
    pub label: &'static str,
    pub value: String,
}

/// Whose stats the tiles show: the local player's, else the winner's (a
/// spectator, or a player with no entry); true when yours.
pub fn stats_subject(view: &MatchEndView) -> Option<(&MatchEndRow, bool)> {
    view.rows.iter().find(|r| r.local).map(|r| (r, true)).or_else(|| view.rows.iter().find(|r| r.winner).map(|r| (r, false)))
}

/// The most kills in a row without a death (kills and deaths by time).
pub fn best_streak(climb: &Climb) -> u32 {
    let mut ev: Vec<(f32, bool)> = climb.kills.iter().map(|t| (*t, true)).chain(climb.deaths.iter().map(|t| (*t, false))).collect();
    ev.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut run, mut best) = (0u32, 0u32);
    for (_, kill) in ev {
        run = if kill { run + 1 } else { 0 };
        best = best.max(run);
    }
    best
}

/// The tiles for `row`: kills, deaths, K/D, best streak, the fastest
/// rung-up and the longest stall. (Headshots are not tracked.)
pub fn stats_of(row: &MatchEndRow, length: f32) -> Vec<EndStat> {
    let stat = |label, value: String| EndStat { label, value };
    let mut last = 0.0;
    let mut fastest: Option<f32> = None;
    for &(t, _) in &row.climb.steps {
        fastest = Some(fastest.map_or(t - last, |f: f32| f.min(t - last)));
        last = t;
    }
    let kd = if row.deaths == 0 { format!("{:.1}", row.kills.max(0) as f32) } else { format!("{:.2}", row.kills as f32 / row.deaths as f32) };
    vec![
        stat("Kills", row.kills.to_string()),
        stat("Deaths", row.deaths.to_string()),
        stat("K/D", kd),
        stat("Best streak", best_streak(&row.climb).to_string()),
        stat("Fastest rung", fastest.map_or("-".into(), |f| mmss(f.max(0.0)))),
        stat("Longest stall", row.climb.longest_stall(length).map_or("-".into(), |(a, b, _)| mmss(b - a))),
    ]
}

/// A row's selection pointer.
#[derive(Component)]
struct RowPointer;

/// The winner's name and the slab sweeping over it.
#[derive(Component)]
pub struct WinnerName;
#[derive(Component)]
pub struct WinnerSlab;

/// The climb chart: whose climb it shows (an index in the rows).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClimbChart {
    pub index: usize,
}

/// One segment of the chosen climb's step line, in the chart's units
/// (0.1 cq), and where it lies along the line (0-1).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ClimbSeg {
    pub a: Vec2,
    pub b: Vec2,
    pub from: f32,
    pub to: f32,
    pub stroke: f32,
}

/// A tick of the chosen climb (a rung-up, a kill, a death, the stall, the
/// final label), fading in `delay` s after the chart starts drawing.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ClimbTick {
    pub kind: TickKind,
    pub delay: f32,
    pub dur: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickKind {
    Up,
    Kill,
    Death,
    /// The stall's box (its fill), its outline's dashes and its label's
    /// ink plate.
    Stall,
    Dash,
    Plate,
    Final,
}

/// Where the stall's label goes (chart units): above the box with its
/// end at the box's right end (the line climbs on from there, so the
/// label sits in the empty space above-left of it), or, for a stall that
/// lasts to the match's end (the final rung's label is there), under the
/// box from its left end. On an ink plate (`PAD_X`, `PAD_Y` around the
/// text) over the faint climbs and the grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StallLabel {
    pub x: f32,
    /// The plate's top.
    pub top: f32,
    /// `x` is the label's right end (else its left end).
    pub anchor_end: bool,
}

impl StallLabel {
    pub const PAD_X: f32 = 1.6;
    pub const PAD_Y: f32 = 0.8;
    /// The plate's height: the 6.4-unit type's line and the padding.
    pub const H: f32 = 6.4 * 1.25 + 2.0 * Self::PAD_Y;
    /// Its gap to the box.
    const GAP: f32 = 1.2;
    /// Its end's gap to the line's step at the box's end (the line is 1.6
    /// wide).
    const INSET: f32 = 2.4;

    pub fn place(axes: &ChartAxes, a: f32, b: f32, rung: u32, end: f32) -> Self {
        let (x0, x1, y) = (axes.x(a), axes.x(b), axes.y(rung as f32));
        if x1 >= axes.x(end) - 0.5 {
            StallLabel { x: x0 + Self::INSET, top: y + 5.0 + Self::GAP, anchor_end: false }
        } else {
            StallLabel { x: x1 - Self::INSET, top: y - 5.0 - Self::GAP - Self::H, anchor_end: true }
        }
    }

    /// The plate's box for a label `w` units wide (the text's width and
    /// the padding).
    pub fn plate(&self, w: f32) -> Rect {
        let x0 = if self.anchor_end { self.x - w } else { self.x };
        Rect::new(x0, self.top, x0 + w, self.top + Self::H)
    }
}

/// The dashes of a box's outline from (`x0`, `y0`) to (`x1`, `y1`), each
/// `stroke` thick: 3-unit dashes 2 apart, each side starting on a dash.
pub fn stall_dashes(x0: f32, x1: f32, y0: f32, y1: f32, stroke: f32) -> Vec<Rect> {
    let (dash, gap) = (3.0, 2.0);
    let along = |from: f32, to: f32| {
        let mut out = Vec::new();
        let mut t = from;
        while t < to {
            out.push((t, (t + dash).min(to)));
            t += dash + gap;
        }
        out
    };
    let mut out = Vec::new();
    for (a, b) in along(x0, x1) {
        out.push(Rect::new(a, y0, b, y0 + stroke));
        out.push(Rect::new(a, y1 - stroke, b, y1));
    }
    for (a, b) in along(y0, y1) {
        out.push(Rect::new(x0, a, x0 + stroke, b));
        out.push(Rect::new(x1 - stroke, a, x1, b));
    }
    out
}

/// Which row's climb shows: the hovered one, else the selected one
/// (clicked; the winner's when the screen comes up).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct EndSelection {
    pub hovered: Option<usize>,
    pub pinned: usize,
}

impl EndSelection {
    pub fn shown(&self) -> usize {
        self.hovered.unwrap_or(self.pinned)
    }
}

/// The screen's own state: when it came up (`UiClock`), what each part
/// was built from, the chart's drawing.
#[derive(Resource, Clone, Debug, Default)]
pub struct EndState {
    pub came_up: Option<f64>,
    static_key: Option<(MatchEndView, (u32, u32), String)>,
    climb_key: Option<(usize, MatchEndView, (u32, u32))>,
    vote_key: Option<(Option<MatchEndVote>, bool, (u32, u32))>,
    /// The chart's drawing began (`UiClock`); none: drawn at once.
    pub drawing: Option<f64>,
    /// The left-pointing triangle by height in physical px.
    pointer: Option<(u32, Handle<Image>)>,
}

/// The screen's systems (added by `MatchEndPlugin`).
pub fn add(app: &mut App) {
    super::signal::setup(app);
    super::hud::hud_cover(app);
    app.init_resource::<EndSelection>().init_resource::<EndState>().add_systems(Startup, spawn).add_systems(
        Update,
        (show, ground, hide_huds, pick, pointer_image, build_static, build_climb, build_vote, decorate_rows, animate, countdown, leave)
            .chain()
            .after(super::match_end::MatchEndFollow)
            .run_if(themed),
    );
}

fn spawn(layer: Res<super::overlay::OverlayLayer>, mut commands: Commands) {
    commands.spawn((
        SignalEndRoot,
        Name::new("Signal match end"),
        ChildOf(layer.0),
        Visibility::Hidden,
        BackgroundColor(Color::NONE),
        GlobalZIndex(MATCH_END_Z),
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            ..default()
        },
    ));
}

/// The root shown with the view; the clock of its coming up.
fn show(
    view: Res<MatchEndView>,
    look: Look,
    clock: Res<UiClock>,
    mut root: Query<(&mut Visibility, &mut BackgroundColor), With<SignalEndRoot>>,
    mut state: ResMut<EndState>,
    mut sel: ResMut<EndSelection>,
) {
    for (mut v, mut bg) in &mut root {
        v.set_if_neq(if view.shown { Visibility::Inherited } else { Visibility::Hidden });
        let ink = look.tone(Tone::Ink);
        if bg.0 != ink {
            bg.0 = ink;
        }
    }
    match (view.shown, state.came_up) {
        (true, None) => {
            state.came_up = Some(clock.now);
            state.drawing = None;
            *sel = EndSelection { hovered: None, pinned: view.rows.iter().position(|r| r.winner).unwrap_or(0) };
        }
        (false, Some(_)) => {
            state.came_up = None;
            state.static_key = None;
            state.climb_key = None;
            state.vote_key = None;
        }
        _ => {}
    }
}

/// The ground: the shared ambient (`AmbientMaterial`, the match end's
/// bloom, no calm column) and the broad slanted planes over it
/// (`LinesMaterial`, `PLANES_ALPHA` a layer): the look book's "ambient +
/// planes". Made once a theme (again when it changes), under every part.
#[derive(Component)]
pub struct EndGround;

fn ground(
    theme: Res<Theme>,
    root: Single<Entity, With<SignalEndRoot>>,
    old: Query<Entity, With<EndGround>>,
    mut ambients: Option<ResMut<Assets<chimera_ui::AmbientMaterial>>>,
    mut planes: Option<ResMut<Assets<chimera_ui::LinesMaterial>>>,
    mut commands: Commands,
) {
    if !old.is_empty() && !theme.is_changed() {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let full = || Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() };
    let mut a = commands.spawn((EndGround, full(), Pickable::IGNORE, ZIndex(-2)));
    if let Some(m) = ambients.as_deref_mut() {
        a.insert(MaterialNode(m.add(chimera_ui::AmbientMaterial::new(&theme))));
    }
    let a = a.id();
    let mut l = commands.spawn((EndGround, full(), Pickable::IGNORE, ZIndex(-1)));
    if let Some(m) = planes.as_deref_mut() {
        l.insert(MaterialNode(m.add(chimera_ui::LinesMaterial::new(&theme))));
    }
    let l = l.id();
    commands.entity(*root).insert_children(0, &[a, l]);
}

/// The Leave button: leaves the match (`disconnect`, which the front end
/// takes through the logo pass back to the menu).
#[derive(Component)]
pub struct LeaveButton;

fn leave(
    pressed: Query<&super::widgets::Interaction, (With<LeaveButton>, Changed<super::widgets::Interaction>)>,
    console: Option<ResMut<chimera_core::console::Console>>,
) {
    if pressed.iter().any(|i| *i == super::widgets::Interaction::Pressed)
        && let Some(mut c) = console
    {
        c.submit("disconnect".to_string());
    }
}

/// The games' HUDs covered while the screen is up (it is the whole
/// screen): `hud::HudCover`.
fn hide_huds(view: Res<MatchEndView>, mut cover: ResMut<super::hud::HudCover>) {
    super::hud::HudCover::put(&mut cover, "match end", view.shown);
}

/// Hover from the cursor over a row; a click selects it.
#[allow(clippy::type_complexity)]
fn pick(
    window: Query<(&Window, &bevy::window::CursorOptions), With<PrimaryWindow>>,
    rows: Query<(&StandingsRow, &ComputedNode, &UiGlobalTransform, Option<&super::widgets::Interaction>)>,
    view: Res<MatchEndView>,
    mut sel: ResMut<EndSelection>,
) {
    if !view.shown {
        return;
    }
    let mut next = *sel;
    if let Ok((w, cursor)) = window.single()
        && cursor.visible
        && cursor.grab_mode == bevy::window::CursorGrabMode::None
    {
        let at = w.cursor_position().map(|p| p * w.scale_factor());
        next.hovered = at.and_then(|at| rows.iter().find(|(_, n, t, _)| n.contains_point(**t, at)).map(|(r, ..)| r.index));
    }
    if let Some((r, ..)) = rows.iter().find(|(.., i)| i.is_some_and(|i| *i == super::widgets::Interaction::Pressed)) {
        next.pinned = r.index;
    }
    if next.pinned >= view.rows.len().max(1) {
        next.pinned = 0;
    }
    if *sel != next {
        *sel = next;
    }
}

/// The view without what changes by the second or with the vote.
fn static_view(v: &MatchEndView) -> MatchEndView {
    MatchEndView {
        next: String::new(),
        vote: None,
        vote_closes: None,
        next_at: 0.0,
        sound: Vec::new(),
        sounds_asked: 0,
        ..v.clone()
    }
}

fn units_key(look: &Look) -> (u32, u32) {
    ((look.units.height * 8.0) as u32, (look.units.scale_factor * 1000.0) as u32)
}

/// `mm:ss` (`9:58`).
pub fn mmss(s: f32) -> String {
    let s = s.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The winner's line: all the rungs and the time, the final kill.
pub fn winner_line(view: &MatchEndView) -> String {
    let Some(w) = view.rows.iter().find(|r| r.winner) else {
        return "Nobody wins the match".into();
    };
    let mut parts = Vec::new();
    if view.rungs > 0 {
        parts.push(format!("All {} rungs in {}", view.rungs, mmss(view.length)));
    } else {
        parts.push(format!("{} kills in {}", w.kills, mmss(view.length)));
    }
    if let Some(v) = &w.climb.last_victim {
        let gun = if w.gun.is_empty() { String::new() } else { format!("{} ", w.gun) };
        parts.push(format!("final kill: {gun}on {v}"));
    }
    parts.join(" \u{2022} ")
}

/// The entrance: a fade from `at` s after the screen came up (`e-fade`,
/// .5 s), with a rise (`e-rise`: .8 cq) or a step in from the left
/// (`e-step`: 2.4 cq).
fn enter(look: &Look, came_up: f64, at: f32, rise: f32, step: f32) -> (Timeline, UiTransform) {
    let ease = look.theme.ease("out");
    let mut tracks = vec![Track::new(Prop::Opacity, 0.0, 1.0).at(at).dur(0.5).ease(ease)];
    if rise != 0.0 {
        tracks.push(Track::new(Prop::Y, rise, 0.0).at(at).dur(0.5).ease(ease));
    }
    if step != 0.0 {
        tracks.push(Track::new(Prop::X, -step, 0.0).at(at).dur(0.52).ease(ease));
    }
    (Timeline::new(tracks).started(came_up), UiTransform::default())
}

/// An absolutely placed box in the frame at (left, top) cq, `w` wide.
fn abs(look: &Look, l: f32, t: f32, w: Option<f32>, r: Option<f32>) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: look.val(l),
        top: look.val(t),
        width: w.map_or(Val::Auto, |w| look.val(w)),
        right: r.map_or(Val::Auto, |r| look.val(r)),
        flex_direction: FlexDirection::Column,
        ..default()
    }
}

/// The standings' columns: position, player, rung/total, kills; the gap.
const COLS: [f32; 4] = [1.8, 0.0, 4.4, 2.6];
const COL_GAP: f32 = 0.9;
const ROW_PAD: f32 = 0.8;
const ROW_PAD_RIGHT: f32 = ROW_PAD + 0.2;

/// Where the screen's blocks sit in its 100 x 56.25 cq frame, from
/// `[layout.matchend]` (the theme's data; these are the defaults): Leave
/// top left, the chat down the left column, the standings down the right
/// (the winner their first, taller row), in the middle the details line,
/// the climb with the stat tiles under it, and along the bottom the vote
/// cards (middle) and the countdown (right). `signal_chat` docks its box
/// with the same numbers, so the blocks never overlap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EndLayout {
    pub pad: f32,
    pub leave_top: f32,
    pub chat_x: f32,
    pub chat_w: f32,
    pub chat_top: f32,
    pub chat_h: f32,
    pub std_x: f32,
    pub std_w: f32,
    pub std_top: f32,
    pub std_h: f32,
    pub mid_x: f32,
    pub mid_w: f32,
    pub details_top: f32,
    pub graph_top: f32,
    pub graph_h: f32,
    pub graph_x: f32,
    pub graph_w: f32,
    pub vote_top: f32,
    pub vote_h: f32,
    pub stats_top: f32,
    pub stats_h: f32,
    pub foot_top: f32,
    pub foot_h: f32,
    pub row_h: f32,
    pub win_h: f32,
}

impl EndLayout {
    /// The frame's height, cq.
    pub const FRAME_H: f32 = 56.25;

    pub fn of(look: &Look) -> Self {
        let g = |k: &str, or: f32| look.layout("matchend", k, or);
        let pad = g("pad", 3.0);
        let gap = g("gap", 1.6);
        let bottom = Self::FRAME_H - g("bottom", 3.2);
        let chat_x = pad;
        let chat_w = g("chat_w", 21.5);
        let chat_top = g("chat_top", 7.4);
        let std_w = g("std_w", 30.0);
        let std_x = 100.0 - pad - std_w;
        let mid_x = chat_x + chat_w + gap;
        let foot_top = g("foot_top", 46.4);
        let std_top = g("std_top", pad);
        let graph_top = g("graph_top", 6.4);
        let graph_h = g("graph_h", 11.0);
        let graph_w = g("graph_w", 28.0).min(std_x - gap - mid_x);
        let vote_top = g("vote_top", 29.5);
        EndLayout {
            pad,
            leave_top: g("leave_top", pad),
            chat_x,
            chat_w,
            chat_top,
            chat_h: bottom - chat_top,
            std_x,
            std_w,
            std_top,
            std_h: foot_top - g("foot_gap", 1.4) - std_top,
            mid_x,
            mid_w: std_x - gap - mid_x,
            details_top: g("details_top", 3.3),
            graph_top,
            graph_h,
            graph_x: mid_x + (std_x - gap - mid_x - graph_w) / 2.0,
            graph_w,
            vote_top,
            vote_h: bottom - vote_top,
            stats_top: graph_top + Self::HEAD_H + graph_h + g("stats_gap", 1.2),
            stats_h: g("stats_h", 5.4),
            foot_top,
            foot_h: bottom - foot_top,
            row_h: g("row_h", 2.4),
            win_h: g("win_h", 5.8),
        }
    }

    /// The climb's head line above its chart, cq.
    pub const HEAD_H: f32 = 1.6;
}

/// The selection pointer's triangle at the window's size.
fn pointer_image(units: Res<chimera_ui::UiUnits>, mut images: Option<ResMut<Assets<Image>>>, mut state: ResMut<EndState>) {
    let tri_px = units.phys(units.cq(0.5)).round().max(2.0) as u32;
    if state.pointer.as_ref().is_some_and(|(p, _)| *p == tri_px) {
        return;
    }
    let Some(images) = images.as_deref_mut() else { return };
    let icon = chimera_ui::icons::Icon {
        name: "Pointer".into(),
        w: 28.0,
        h: 56.0,
        shapes: vec![vec![vec![[28.0, 0.0], [0.0, 28.0], [28.0, 56.0]]]],
    };
    state.pointer = Some((tri_px, images.add(chimera_ui::icons::image(&icon, tri_px))));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build_static(
    look: Look,
    view: Res<MatchEndView>,
    root: Single<Entity, With<SignalEndRoot>>,
    old: Query<Entity, With<StaticPart>>,
    mut state: ResMut<EndState>,
    mut slants: Option<ResMut<Assets<SlantMaterial>>>,
    mut commands: Commands,
) {
    let key = (static_view(&view), units_key(&look), look.theme.name.clone());
    if state.static_key.as_ref() == Some(&key) && !look.theme.is_changed() {
        return;
    }
    state.static_key = Some(key);
    for e in &old {
        commands.entity(e).despawn();
    }
    if !view.shown {
        return;
    }
    let came_up = state.came_up.unwrap_or(0.0);
    let hair = look.hair();
    let pointer = state.pointer.as_ref().map(|(_, h)| h.clone()).unwrap_or_default();
    let lay = EndLayout::of(&look);
    let ui = chimera_ui::controls::Ui { theme: &look.theme, kit: look.fonts.as_deref(), units: *look.units };
    let frame = commands
        .spawn((
            StaticPart,
            ChildOf(*root),
            Pickable::IGNORE,
            Node { width: look.val(100.0), height: look.val(56.25), flex_shrink: 0.0, align_self: AlignSelf::Center, ..default() },
        ))
        .id();
    let mut f = commands.entity(frame);
    f.with_children(|f| {
        // Corners.
        for (x, y) in [(1.6, 1.6), (98.4, 1.6), (1.6, 54.65), (98.4, 54.65)] {
            let s = 1.1;
            f.spawn((Node { height: look.val(s), ..abs(&look, x - s / 2.0, y - s / 2.0, Some(s), None) }, Pickable::IGNORE)).with_children(|p| {
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: percent(50.0), width: px(hair), height: percent(100.0), ..default() },
                    BackgroundColor(look.tone(Tone::Fg3)),
                ));
                p.spawn((
                    Node { position_type: PositionType::Absolute, top: percent(50.0), height: px(hair), width: percent(100.0), ..default() },
                    BackgroundColor(look.tone(Tone::Fg3)),
                ));
            });
        }
        // The details line, top centre: map, players, rungs, time.
        f.spawn((
            DetailsLine,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: look.val(2.2),
                height: look.val(1.6),
                ..abs(&look, lay.mid_x, lay.details_top, Some(lay.mid_w), None)
            },
            enter(&look, came_up, 0.3, 0.0, 0.0),
            UiParityId::new("end.hd.info"),
            Pickable::IGNORE,
        ))
        .with_children(|d| {
            let map = if view.map_game.is_empty() { view.map.clone() } else { format!("{} \u{2022} {}", view.map, view.map_game) };
            let mut items = vec![("pin", map)];
            items.push(("friends", view.rows.len().to_string()));
            if view.rungs > 0 {
                items.push(("ladder", view.rungs.to_string()));
            }
            items.push(("clock", mmss(view.length)));
            for (icon, text) in items.into_iter().filter(|(_, t)| !t.is_empty()) {
                d.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.5), min_width: px(0.0), ..default() }, Pickable::IGNORE)).with_children(|i| {
                    signal::icons::spawn(i, &look, icon, look.px(0.75), look.tone(Tone::Fg3));
                    i.spawn((look.tracked(Role::Mono, 0.62, 0.62 * 0.06, look.tone(Tone::Fg2), &text), Pickable::IGNORE));
                });
            }
        });

        // The stat tiles under the climb: yours, else the winner's.
        if let Some((subject, yours)) = stats_subject(&view) {
            f.spawn((
                StatsRow,
                Node { flex_direction: FlexDirection::Column, row_gap: look.val(0.5), height: look.val(lay.stats_h), ..abs(&look, lay.mid_x, lay.stats_top, Some(lay.mid_w), None) },
                enter(&look, came_up, 1.3, 0.8, 0.0),
                UiParityId::new("end.stats"),
                Pickable::IGNORE,
            ))
            .with_children(|s| {
                let who = if yours { "Your stats".to_string() } else { format!("{}'s stats", subject.name) };
                s.spawn((look.tracked(Role::Mono, 0.5, 0.05, look.tone(Tone::Fg3), &who.to_uppercase()), Ellipsize::max(who.to_uppercase(), lay.mid_w), Pickable::IGNORE));
                s.spawn((Node { flex_grow: 1.0, column_gap: look.val(0.7), ..default() }, Pickable::IGNORE)).with_children(|t| {
                    for (i, stat) in stats_of(subject, view.length).into_iter().enumerate() {
                        t.spawn((
                            StatTile { label: stat.label },
                            Node {
                                flex_grow: 1.0,
                                flex_basis: px(0.0),
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::Center,
                                row_gap: look.val(0.35),
                                padding: UiRect::horizontal(look.val(0.8)),
                                ..default()
                            },
                            BackgroundColor(look.theme.tone_a(Tone::Fg, 0.04)),
                            Outline::new(px(hair), px(0.0), look.theme.tone_a(Tone::Fg, 0.16)),
                            UiParityId(format!("end.stats.{}", i + 1)),
                            Pickable::IGNORE,
                        ))
                        .with_children(|c| {
                            c.spawn((look.tracked(Role::Mono, 0.48, 0.048, look.tone(Tone::Fg3), &stat.label.to_uppercase()), Pickable::IGNORE));
                            c.spawn((look.tracked(Role::DisplayBold, 1.5, 0.0, look.tone(Tone::Fg), &stat.value), Pickable::IGNORE));
                        });
                    }
                });
            });
        }

        // The standings, down the right column: the winner's taller row first.
        let head = 2.0;
        let others = view.rows.iter().filter(|r| !r.winner).count().max(1) as f32;
        let has_winner = view.rows.iter().any(|r| r.winner);
        let room = lay.std_h - head - if has_winner { lay.win_h } else { 0.0 };
        let row_h = ((room - 0.5) / others).clamp(1.5, lay.row_h);
        f.spawn((
            StandingsPanel,
            Node { height: look.val(lay.std_h), overflow: Overflow::clip_y(), ..abs(&look, lay.std_x, lay.std_top, Some(lay.std_w), None) },
            Pickable::IGNORE,
        ))
        .with_children(|s| {
            let cell = |i: usize| Node {
                width: if COLS[i] > 0.0 { look.val(COLS[i]) } else { Val::Auto },
                flex_grow: if COLS[i] > 0.0 { 0.0 } else { 1.0 },
                flex_shrink: if COLS[i] > 0.0 { 0.0 } else { 1.0 },
                min_width: px(0.0),
                overflow: Overflow::clip(),
                align_items: AlignItems::Center,
                ..default()
            };
            let right = |i: usize| Node { justify_content: JustifyContent::FlexEnd, ..cell(i) };
            let row_node = || Node {
                align_items: AlignItems::Center,
                column_gap: look.val(COL_GAP),
                padding: UiRect::new(look.val(ROW_PAD), look.val(ROW_PAD_RIGHT), px(0.0), px(0.0)),
                ..default()
            };
            s.spawn((
                Node {
                    height: look.val(head),
                    padding: UiRect::new(look.val(ROW_PAD), look.val(ROW_PAD_RIGHT), px(0.0), look.val(0.4)),
                    border: UiRect::bottom(px(hair)),
                    flex_shrink: 0.0,
                    align_items: AlignItems::FlexEnd,
                    column_gap: look.val(COL_GAP),
                    ..default()
                },
                BorderColor::all(look.tone(Tone::Fg4)),
                enter(&look, came_up, 0.65, 0.0, 0.0),
                UiParityId::new("end.std.th"),
                Pickable::IGNORE,
            ))
            .with_children(|h| {
                let th = |h: &mut ChildSpawnerCommands, i: usize, text: &str, r: bool, id: &str| {
                    h.spawn((if r { right(i) } else { cell(i) }, Pickable::IGNORE)).with_children(|c| {
                        c.spawn((look.tracked(Role::Mono, 0.5, 0.06, look.tone(Tone::Fg3), text), UiParityId(format!("end.std.th.{id}"))));
                    });
                };
                th(h, 0, "#", false, "pos");
                th(h, 1, "Player", false, "player");
                th(h, 2, "Rung", true, "rung");
                h.spawn((right(3), Pickable::IGNORE)).with_children(|c| {
                    let e = signal::mark(c, &look, Mark::Kill, look.px(0.66), look.tone(Tone::Fg3));
                    parity::name(c, e, "end.std.th.k");
                });
            });
            let top_wins = view.rows.iter().map(|r| r.wins).max().unwrap_or(0);
            // The winner first, then the rest in order.
            let order: Vec<usize> = (0..view.rows.len()).filter(|i| view.rows[*i].winner).chain((0..view.rows.len()).filter(|i| !view.rows[*i].winner)).collect();
            for (n, i) in order.into_iter().enumerate() {
                let row = &view.rows[i];
                let p = format!("end.std.row.{}", i + 1);
                let h = if row.winner { lay.win_h } else { row_h };
                let mut r = s.spawn((
                    StandingsRow { index: i, you: row.local },
                    // Its hover and pin are the selection's fills
                    // (`decorate_rows`: the climb previews the hovered).
                    chimera_ui::controls::own_states(),
                    Node { height: look.val(h), border: UiRect::bottom(px(hair)), flex_shrink: 0.0, ..row_node() },
                    BorderColor::all(look.tone(Tone::Fg5)),
                    BackgroundColor(Color::NONE),
                    enter(&look, came_up, 0.5 + n as f32 * 0.055, 0.0, 2.4 + n as f32 * 0.62),
                    UiParityId(p.clone()),
                ));
                if row.local {
                    r.insert(signal::you_row(&look));
                }
                if row.winner {
                    // The accent frame and a faint accent fill under the selection's.
                    r.insert((WinnerRow, Outline::new(px(look.px(0.14)), px(-look.px(0.14)), look.tone(Tone::Acc))));
                    winner_cells(&mut r, &look, row, &view, &p, top_wins, came_up, slants.as_deref_mut(), &cell, &right);
                } else {
                    row_cells(&mut r, &look, row, i, &view, &p, top_wins, h, slants.as_deref_mut(), &cell, &right, &pointer);
                }
            }
        });
    });
    // Leave, top left: the kit's secondary button with the arrow (its
    // hover and press states), in a place that fades in.
    let at = commands.spawn((StaticPart, Pickable::IGNORE, abs(&look, lay.pad, lay.leave_top, None, None), enter(&look, came_up, 0.2, 0.0, 0.0), ChildOf(frame))).id();
    let leave = ui.button_icon(&mut commands, at, chimera_ui::controls::Kind::Secondary, Some("arrow_left"), "Leave", None, chimera_ui::controls::Size::Small, chimera_ui::controls::State::NONE);
    commands.entity(leave).insert((LeaveButton, UiParityId::new("end.vote.leave")));
}

/// `n` of `total` as the standings show a rung (`07/14`; the rung alone
/// off a ladder).
pub fn rung_of(row: &MatchEndRow, total: u32) -> String {
    match (row.rung, total) {
        (None, _) => "-".into(),
        (Some(r), 0) => format!("{r:02}"),
        (Some(r), t) => format!("{r:02}/{t}"),
    }
}

/// The winner's sub-line: `Winner · 14/14 · Knife`.
pub fn winner_sub(row: &MatchEndRow, total: u32) -> String {
    let mut parts = vec!["Winner".to_string()];
    if row.rung.is_some() {
        parts.push(rung_of(row, total));
    }
    if !row.gun.is_empty() {
        parts.push(row.gun.clone());
    }
    parts.join(" \u{b7} ")
}

/// The winner's row: a trophy, the name large with the slab over it (and
/// the host chip and session wins), the sub-line under it, the kills.
#[allow(clippy::too_many_arguments)]
fn winner_cells(
    r: &mut EntityCommands,
    look: &Look,
    row: &MatchEndRow,
    view: &MatchEndView,
    p: &str,
    top_wins: u32,
    came_up: f64,
    mut slants: Option<&mut Assets<SlantMaterial>>,
    cell: &dyn Fn(usize) -> Node,
    right: &dyn Fn(usize) -> Node,
) {
    r.with_children(|r| {
        r.spawn((
            Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), bottom: px(0.0), width: look.val(0.26), ..default() },
            BackgroundColor(look.tone(Tone::Acc)),
            Pickable::IGNORE,
        ));
        r.spawn((Node { width: look.val(COLS[0] + 0.8), flex_shrink: 0.0, align_items: AlignItems::Center, ..default() }, Pickable::IGNORE)).with_children(|c| {
            let e = signal::mark(c, look, Mark::Trophy, look.px(2.0), look.tone(Tone::AccText));
            parity::name(c, e, format!("{p}.trophy"));
        });
        r.spawn((Node { flex_direction: FlexDirection::Column, row_gap: look.val(0.5), justify_content: JustifyContent::Center, align_items: AlignItems::FlexStart, ..cell(1) }, Pickable::IGNORE)).with_children(|c| {
            c.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.6), ..default() }, Pickable::IGNORE)).with_children(|n| {
                if row.bot {
                    let e = signal::bot_slot(n, look, true, 0.9, look.tone(Tone::Fg2));
                    parity::name(n, e, format!("{p}.bot"));
                }
                n.spawn((Node { align_self: AlignSelf::FlexStart, flex_shrink: 1.0, ..default() }, Pickable::IGNORE)).with_children(|w| {
                    w.spawn((
                        look.tracked(Role::DisplayBold, 1.35, 1.35 * 0.04, look.tone(Tone::Fg), &row.name.to_uppercase()),
                        Ellipsize::max(row.name.to_uppercase(), 12.5),
                        WinnerName,
                        Timeline::new([Track::new(Prop::Opacity, 0.0, 1.0).at(look.dur("matchend", "name_at", 0.9))]).started(came_up),
                        UiParityId(format!("{p}.name")),
                    ));
                    if let Some(slants) = slants.as_deref_mut() {
                        w.spawn((
                            WinnerSlab,
                            MaterialNode(slants.add(SlantMaterial::new(&look.theme, look.tone(Tone::Acc)))),
                            Node { position_type: PositionType::Absolute, left: look.val(-0.6), top: px(0.0), bottom: px(0.0), width: px(0.0), ..default() },
                            Pickable::IGNORE,
                        ));
                    }
                });
                let e = signal::host_tag(n, look, slants.as_deref_mut(), row.host, row.local, 0.62);
                parity::name(n, e, format!("{p}.host"));
                if row.wins > 0 {
                    n.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.18), flex_shrink: 0.0, ..default() }, super::race_chip::SessionWins(row.wins), Pickable::IGNORE))
                        .with_children(|w| {
                            let tone = if row.wins == top_wins { Tone::AccText } else { Tone::Fg3 };
                            signal::mark(w, look, Mark::Trophy, look.px(0.68), look.tone(tone));
                            w.spawn((look.tracked(Role::Mono, 0.56, 0.02, look.tone(tone), &row.wins.to_string()), UiParityId(format!("{p}.wins")), Pickable::IGNORE));
                            w.spawn((look.text(Role::Mono, 0.56, look.tone(Tone::AccText), "+1"), Pickable::IGNORE));
                        });
                }
            });
            c.spawn((
                look.tracked(Role::Mono, 0.56, 0.056, look.tone(Tone::AccText), &winner_sub(row, view.rungs)),
                Ellipsize::in_parent(winner_sub(row, view.rungs)),
                UiParityId(format!("{p}.sub")),
                Pickable::IGNORE,
            ));
        });
        r.spawn((right(3), Pickable::IGNORE)).with_children(|c| {
            c.spawn((look.tracked(Role::DisplayBold, 1.3, 0.0, look.tone(Tone::Fg), &row.kills.to_string()), UiParityId(format!("{p}.k"))));
        });
    });
}

/// One standings row's cells and its pointer.
#[allow(clippy::too_many_arguments)]
fn row_cells(
    r: &mut EntityCommands,
    look: &Look,
    row: &MatchEndRow,
    i: usize,
    view: &MatchEndView,
    p: &str,
    top_wins: u32,
    row_h: f32,
    mut slants: Option<&mut Assets<SlantMaterial>>,
    cell: &dyn Fn(usize) -> Node,
    right: &dyn Fn(usize) -> Node,
    pointer: &Handle<Image>,
) {
    r.with_children(|r| {
        r.spawn((cell(0), Pickable::IGNORE)).with_children(|c| {
            c.spawn((look.tracked(Role::Mono, 0.6, 0.0, look.tone(Tone::Fg3), &format!("{:02}", i + 1)), UiParityId(format!("{p}.pos"))));
        });
        r.spawn((Node { column_gap: look.val(0.5), ..cell(1) }, Pickable::IGNORE)).with_children(|c| {
            let e = signal::bot_slot(c, look, row.bot, 0.8, look.tone(Tone::Fg2));
            parity::name(c, e, format!("{p}.bot"));
            c.spawn((
                Node { flex_shrink: 1.0, min_width: px(0.0), overflow: Overflow::clip(), ..default() },
                look.tracked(Role::DisplayBold, 0.78, 0.78 * 0.03, look.tone(Tone::Fg), &row.name.to_uppercase()),
                Ellipsize::in_parent(row.name.to_uppercase()),
                UiParityId(format!("{p}.name")),
                Pickable::IGNORE,
            ));
            let e = signal::host_tag(c, look, slants.as_deref_mut(), row.host, row.local, 0.62);
            parity::name(c, e, format!("{p}.host"));
            if row.wins > 0 {
                // The session's leader in the accent.
                let tone = if row.wins == top_wins { Tone::AccText } else { Tone::Fg3 };
                c.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.18), flex_shrink: 0.0, ..default() }, super::race_chip::SessionWins(row.wins), Pickable::IGNORE))
                    .with_children(|w| {
                        signal::mark(w, look, Mark::Trophy, look.px(0.68), look.tone(tone));
                        w.spawn((look.tracked(Role::Mono, 0.56, 0.02, look.tone(tone), &row.wins.to_string()), UiParityId(format!("{p}.wins")), Pickable::IGNORE));
                    });
            }
        });
        r.spawn((right(2), Pickable::IGNORE)).with_children(|c| {
            c.spawn((look.tracked(Role::DisplayBold, 1.0, 0.0, look.tone(Tone::Fg), &rung_of(row, view.rungs)), UiParityId(format!("{p}.rung"))));
        });
        r.spawn((right(3), Pickable::IGNORE)).with_children(|c| {
            c.spawn((look.tracked(Role::Mono, 0.72, 0.0, look.tone(Tone::Fg), &row.kills.to_string()), UiParityId(format!("{p}.k"))));
        });
        // The selection's pointer (hover is the row's fill alone).
        r.spawn((
            RowPointer,
            ImageNode { image: pointer.clone(), color: look.tone(Tone::Fg), ..default() },
            Node {
                position_type: PositionType::Absolute,
                left: look.val(-0.5),
                top: px(look.px(row_h / 2.0) - look.px(0.25)),
                width: look.val(0.25),
                height: look.val(0.5),
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
        ));
    });
}

/// The chart's geometry, in its units (0.1 cq; the look book's 470 x 174
/// viewBox): x from the match's start to `span` s, marked every `step` s,
/// y by rung.
#[derive(Clone, Copy, Debug)]
pub struct ChartAxes {
    pub span: f32,
    pub step: f32,
    pub rungs: u32,
    /// Extra height under the look book's 174-unit drawing, so the plot
    /// can be taller than it is wide (`with_height`).
    pub dy: f32,
}

impl ChartAxes {
    pub const X0: f32 = 22.0;
    pub const X1: f32 = 462.0;
    /// The time marks to choose from, s, and the most marks along x.
    pub const STEPS: [f32; 8] = [30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0];
    pub const MAX_MARKS: f32 = 8.0;

    /// The match's length rounded up to the finest mark that leaves at
    /// most `MAX_MARKS` of them (6:03 to 7:00 in minutes; 1:10 to 1:30).
    pub fn new(length: f32, rungs: u32) -> Self {
        let length = length.max(1.0);
        let step = Self::STEPS.into_iter().find(|s| (length / s).ceil() <= Self::MAX_MARKS).unwrap_or(3600.0);
        let span = (length / step).ceil() * step;
        ChartAxes { span, step, rungs: rungs.max(2), dy: 0.0 }
    }

    /// The same axes drawn `dy` units taller.
    pub fn with_height(self, dy: f32) -> Self {
        ChartAxes { dy: dy.max(0.0), ..self }
    }

    pub fn x(&self, t: f32) -> f32 {
        Self::X0 + (Self::X1 - Self::X0) * (t / self.span).clamp(0.0, 1.0)
    }

    pub fn y(&self, rung: f32) -> f32 {
        134.0 + self.dy - (124.0 + self.dy) * (rung - 1.0) / (self.rungs - 1) as f32
    }

    /// The step line of `climb` until `end` s: its corners.
    pub fn points(&self, climb: &Climb, end: f32) -> Vec<Vec2> {
        let mut out = vec![Vec2::new(self.x(0.0), self.y(1.0))];
        let mut y = self.y(1.0);
        for &(t, r) in &climb.steps {
            let x = self.x(t.min(end));
            out.push(Vec2::new(x, y));
            y = self.y(r as f32);
            out.push(Vec2::new(x, y));
        }
        out.push(Vec2::new(self.x(end), y));
        out.dedup();
        out
    }
}

/// Segments of a polyline with where each lies along it (0-1).
pub fn segments(points: &[Vec2], stroke: f32) -> Vec<ClimbSeg> {
    let total: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>().max(1e-3);
    let mut at = 0.0;
    points
        .windows(2)
        .filter(|w| w[0] != w[1])
        .map(|w| {
            let len = w[0].distance(w[1]);
            let s = ClimbSeg { a: w[0], b: w[1], from: at / total, to: (at + len) / total, stroke };
            at += len;
            s
        })
        .collect()
}

/// A segment's node at `progress` (0-1 of the line drawn), `k` logical px
/// a chart unit.
fn seg_node(s: &ClimbSeg, k: f32, progress: f32) -> Node {
    let f = ((progress - s.from) / (s.to - s.from).max(1e-6)).clamp(0.0, 1.0);
    let b = s.a + (s.b - s.a) * f;
    let w = s.stroke * k;
    let (x0, x1) = (s.a.x.min(b.x) * k, s.a.x.max(b.x) * k);
    let (y0, y1) = (s.a.y.min(b.y) * k, s.a.y.max(b.y) * k);
    let horizontal = (s.b.y - s.a.y).abs() < 1e-3;
    let (left, top, width, height) = if f <= 0.0 {
        (x0, y0, 0.0, 0.0)
    } else if horizontal {
        (x0 - w / 2.0, y0 - w / 2.0, x1 - x0 + w, w)
    } else {
        (x0 - w / 2.0, y0 - w / 2.0, w, y1 - y0 + w)
    };
    Node { position_type: PositionType::Absolute, left: px(left), top: px(top), width: px(width), height: px(height), ..default() }
}

#[allow(clippy::too_many_arguments)]
fn build_climb(
    look: Look,
    view: Res<MatchEndView>,
    sel: Res<EndSelection>,
    clock: Res<UiClock>,
    root: Single<Entity, With<SignalEndRoot>>,
    old: Query<Entity, With<ClimbPart>>,
    mut state: ResMut<EndState>,
    mut commands: Commands,
) {
    let shown = sel.shown().min(view.rows.len().saturating_sub(1));
    let key = (shown, static_view(&view), units_key(&look));
    if state.climb_key.as_ref() == Some(&key) && !look.theme.is_changed() {
        return;
    }
    // A new choice draws; the first chart (and a resize) shows at once.
    let chose = state.climb_key.as_ref().is_some_and(|(i, v, u)| *i != shown && *v == key.1 && *u == key.2);
    state.drawing = if chose { Some(clock.now) } else { None };
    state.climb_key = Some(key);
    for e in &old {
        commands.entity(e).despawn();
    }
    if !view.shown || view.rows.is_empty() {
        return;
    }
    let came_up = state.came_up.unwrap_or(0.0);
    let row = &view.rows[shown];
    let lay = EndLayout::of(&look);
    // The chart is the look book's 47 cq wide drawing at a scale, stretched down to the layout's height.
    let scale = lay.graph_w / 47.0;
    let dy = (lay.graph_h / (0.1 * scale) - 174.0).max(0.0);
    let axes = ChartAxes::new(view.length, view.rungs.max(row.rung.unwrap_or(1))).with_height(dy);
    let k = look.units.cq(0.1) * scale;
    let fs = (0.64 * scale).max(0.5);
    let end = view.length;
    let fg = |a: f32| look.theme.tone_a(Tone::Fg, a);
    let label = |p: &mut ChildSpawnerCommands, text: &str, x: f32, baseline: f32, anchor: f32, color: Color| {
        // A label of the chart's 6.4-unit type, its baseline at `baseline`,
        // anchored at x (0 start, .5 middle, 1 end) in a 12-unit-wide box... any width.
        let w = 60.0;
        let justify = if anchor <= 0.0 {
            JustifyContent::FlexStart
        } else if anchor >= 1.0 {
            JustifyContent::FlexEnd
        } else {
            JustifyContent::Center
        };
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px((x - w * anchor) * k),
                top: px((baseline - 6.4 * 0.78) * k),
                width: px(w * k),
                justify_content: justify,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|t| {
            t.spawn((look.tracked(Role::Mono, fs, fs * 0.08, color, text), Pickable::IGNORE));
        });
    };
    let line = |p: &mut ChildSpawnerCommands, a: Vec2, b: Vec2, w: f32, color: Color| {
        let s = ClimbSeg { a, b, from: 0.0, to: 1.0, stroke: w };
        p.spawn((seg_node(&s, k, 1.0), BackgroundColor(color), Pickable::IGNORE));
    };
    let winner = row.winner;
    let accent = if winner { look.tone(Tone::AccText) } else { look.tone(Tone::Fg) };
    let hair = look.hair() / k;
    commands.spawn((ClimbPart, ChildOf(*root), Pickable::IGNORE, Node { width: look.val(100.0), height: look.val(56.25), position_type: PositionType::Absolute, align_self: AlignSelf::Center, ..default() }))
        .with_children(|f| {
            f.spawn((abs(&look, lay.graph_x, lay.graph_top, Some(lay.graph_w), None), enter(&look, came_up, 1.1, 0.0, 0.0), UiParityId::new("end.climb"), Pickable::IGNORE)).with_children(|c| {
                // The head: whose climb, what it came to.
                c.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.8), height: look.val(EndLayout::HEAD_H - 0.6), ..default() }, Pickable::IGNORE)).with_children(|h| {
                    h.spawn((
                        look.tracked(Role::Mono, 0.56, 0.056, look.tone(Tone::Fg), &row.name.to_uppercase()),
                        Ellipsize::max(row.name.to_uppercase(), 12.0),
                        UiParityId::new("end.climb.name"),
                    ));
                    let reach = match (winner, row.rung) {
                        (true, _) if view.rungs > 0 => format!("All {} rungs in {}", view.rungs, mmss(end)),
                        (_, Some(r)) if view.rungs > 0 => format!("Reached rung {r:02} of {}", view.rungs),
                        (_, Some(r)) => format!("Reached rung {r:02}"),
                        _ => String::new(),
                    };
                    h.spawn((look.tracked(Role::Mono, 0.56, 0.056, look.tone(Tone::Fg3), &reach), UiParityId::new("end.climb.stats")));
                });
                c.spawn((
                    ClimbChart { index: shown },
                    Node { width: look.val(47.0 * scale), height: look.val(17.4 * scale + dy * 0.1 * scale), margin: UiRect::top(look.val(0.6)), ..default() },
                    UiParityId::new("end.climb.chart"),
                    Pickable::IGNORE,
                ))
                .with_children(|g| {
                    // The grid: the time marks to the match's length rounded up, the first, middle and top rungs, the axis.
                    for i in 0..=(axes.span / axes.step).round() as u32 {
                        let m = i as f32 * axes.step;
                        let x = axes.x(m);
                        line(g, Vec2::new(x, 6.0), Vec2::new(x, 160.0 + dy), hair, fg(0.07));
                        label(g, &mmss(m), x, 171.0 + dy, 0.5, look.tone(Tone::Fg3));
                    }
                    let top = axes.rungs;
                    let mut marks = vec![1, top.div_ceil(2), top];
                    marks.dedup();
                    for r in marks {
                        let y = axes.y(r as f32);
                        line(g, Vec2::new(ChartAxes::X0, y), Vec2::new(ChartAxes::X1, y), hair, fg(0.07));
                        label(g, &format!("{r:02}"), 16.0, y + 2.2, 1.0, look.tone(Tone::Fg3));
                    }
                    line(g, Vec2::new(ChartAxes::X0, 140.5 + dy), Vec2::new(ChartAxes::X1, 140.5 + dy), hair, look.tone(Tone::Fg4));
                    label(g, "K", 16.0, 149.2 + dy, 1.0, look.tone(Tone::Fg3));
                    label(g, "D", 16.0, 159.2 + dy, 1.0, look.tone(Tone::Fg3));
                    // The others, faint.
                    for (j, other) in view.rows.iter().enumerate() {
                        if j == shown {
                            continue;
                        }
                        for s in segments(&axes.points(&other.climb, end), hair) {
                            g.spawn((seg_node(&s, k, 1.0), BackgroundColor(fg(0.2)), Pickable::IGNORE));
                        }
                    }
                    // The longest stall, boxed.
                    let draw = look.dur("matchend", "climb_draw", 0.7);
                    let spread = look.dur("matchend", "climb_ticks", 0.52);
                    if let Some((a, b, r)) = row.climb.longest_stall(end) {
                        let (x0, x1, y) = (axes.x(a), axes.x(b), axes.y(r as f32));
                        g.spawn((
                            Node { position_type: PositionType::Absolute, left: px(x0 * k), top: px((y - 5.0) * k), width: px((x1 - x0) * k), height: px(10.0 * k), ..default() },
                            BackgroundColor(fg(0.06)),
                            ClimbTick { kind: TickKind::Stall, delay: spread, dur: 0.26 },
                            Pickable::IGNORE,
                        ));
                        // Its outline, dashed.
                        for d in stall_dashes(x0, x1, y - 5.0, y + 5.0, hair) {
                            g.spawn((
                                Node { position_type: PositionType::Absolute, left: px(d.min.x * k), top: px(d.min.y * k), width: px((d.width() * k).max(look.hair())), height: px((d.height() * k).max(look.hair())), ..default() },
                                BackgroundColor(look.theme.tone_a(Tone::Fg, 0.42)),
                                ClimbTick { kind: TickKind::Dash, delay: spread, dur: 0.26 },
                                Pickable::IGNORE,
                            ));
                        }
                        // The label on an ink plate, clear of the line.
                        let at = StallLabel::place(&axes, a, b, r as u32, end);
                        let w = 120.0;
                        let left = if at.anchor_end { at.x - w } else { at.x };
                        g.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(left * k),
                                top: px(at.top * k),
                                width: px(w * k),
                                justify_content: if at.anchor_end { JustifyContent::FlexEnd } else { JustifyContent::FlexStart },
                                ..default()
                            },
                            Pickable::IGNORE,
                        ))
                        .with_children(|p| {
                            p.spawn((
                                Node { height: px(StallLabel::H * k), padding: UiRect::horizontal(px(StallLabel::PAD_X * k)), align_items: AlignItems::Center, ..default() },
                                BackgroundColor(look.theme.tone_a(Tone::Ink, 0.85)),
                                ClimbTick { kind: TickKind::Plate, delay: spread, dur: 0.26 },
                                UiParityId::new("end.climb.stall"),
                                Pickable::IGNORE,
                            ))
                            .with_children(|t| {
                                t.spawn((look.tracked(Role::Mono, fs, fs * 0.08, look.tone(Tone::Fg3), &format!("Stall {} \u{2022} R{r:02}", mmss(b - a))), Pickable::IGNORE));
                            });
                        });
                    }
                    // The chosen climb's line, drawn along its length.
                    for s in segments(&axes.points(&row.climb, end), 1.6) {
                        g.spawn((seg_node(&s, k, 1.0), BackgroundColor(accent), s, Pickable::IGNORE));
                    }
                    let delay = |x: f32, group: f32| 0.08 + group * 0.06 + (x - ChartAxes::X0) / (ChartAxes::X1 - ChartAxes::X0) * spread;
                    for &(t, r) in &row.climb.steps {
                        let (x, y) = (axes.x(t), axes.y(r as f32));
                        g.spawn((
                            Node { position_type: PositionType::Absolute, left: px((x - 1.6) * k), top: px((y - 1.6) * k), width: px(3.2 * k), height: px(3.2 * k), ..default() },
                            BackgroundColor(accent),
                            ClimbTick { kind: TickKind::Up, delay: delay(x, 0.0), dur: 0.18 },
                            Pickable::IGNORE,
                        ));
                    }
                    for (times, y, color, kind, group) in [
                        (&row.climb.kills, 144.0 + dy, look.tone(Tone::Fg2), TickKind::Kill, 1.0),
                        (&row.climb.deaths, 154.0 + dy, look.theme.tone_a(Tone::Threat, 0.75), TickKind::Death, 2.0),
                    ] {
                        for &t in times {
                            let x = axes.x(t);
                            g.spawn((
                                Node { position_type: PositionType::Absolute, left: px((x - 0.6) * k), top: px(y * k), width: px((1.2 * k).max(look.hair())), height: px(6.0 * k), ..default() },
                                BackgroundColor(color),
                                ClimbTick { kind, delay: delay(x, group), dur: 0.18 },
                                Pickable::IGNORE,
                            ));
                        }
                    }
                    // The final rung, named.
                    let fin_rung = row.rung.unwrap_or(1);
                    let fin = if winner && !row.gun.is_empty() { format!("{fin_rung:02} \u{2022} {}", row.gun) } else { format!("R{fin_rung:02}") };
                    let fin_y = axes.y(row.climb.steps.last().map_or(1.0, |s| s.1 as f32)) - 5.0;
                    g.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px((axes.x(end) - 120.0) * k),
                            top: px((fin_y - 6.4 * 0.78) * k),
                            width: px(120.0 * k),
                            justify_content: JustifyContent::FlexEnd,
                            ..default()
                        },
                        ClimbTick { kind: TickKind::Final, delay: spread, dur: 0.26 },
                        Pickable::IGNORE,
                    ))
                    .with_children(|t| {
                        t.spawn((look.tracked(Role::Mono, fs, fs * 0.08, accent, &fin.to_uppercase()), UiParityId::new("end.climb.final")));
                    });
                    let _ = draw;
                });
            });
        });
}

/// Rows' fills and pointers from the selection.
#[allow(clippy::type_complexity)]
fn decorate_rows(
    look: Look,
    sel: Res<EndSelection>,
    mut rows: Query<(&StandingsRow, &mut BackgroundColor, &Children)>,
    mut marks: Query<&mut Visibility, With<RowPointer>>,
    new: Query<(), Added<StandingsRow>>,
    // The entrance's fade writes the rows' fills too: again after it.
    faded: Query<(), (With<StandingsRow>, Changed<Timeline>)>,
) {
    if !sel.is_changed() && new.is_empty() && faded.is_empty() {
        return;
    }
    for (row, mut bg, children) in &mut rows {
        let hovered = sel.hovered == Some(row.index);
        let pinned = sel.pinned == row.index;
        let a = match (row.you, pinned, hovered) {
            (true, true, _) => 0.11,
            (true, false, _) => 0.07,
            (false, true, _) => 0.08,
            (false, false, true) => 0.04,
            _ => 0.0,
        };
        let c = if a > 0.0 { look.theme.tone_a(Tone::Fg, a) } else { Color::NONE };
        if bg.0 != c {
            bg.0 = c;
        }
        for c in children.iter() {
            if let Ok(mut v) = marks.get_mut(c) {
                v.set_if_neq(if pinned { Visibility::Inherited } else { Visibility::Hidden });
            }
        }
    }
}

/// The winner's slab, and the climb's line and ticks while it draws.
#[allow(clippy::type_complexity)]
fn animate(
    look: Look,
    clock: Res<UiClock>,
    state: Res<EndState>,
    name: Query<&ComputedNode, With<WinnerName>>,
    mut slab: Query<(&mut Node, &mut Visibility), (With<WinnerSlab>, Without<ClimbSeg>, Without<ClimbTick>)>,
    mut segs: Query<(&ClimbSeg, &mut Node), Without<WinnerSlab>>,
    mut ticks: Query<(&ClimbTick, Option<&mut BackgroundColor>, Option<&mut BorderColor>, Option<&Children>), Without<WinnerSlab>>,
    mut texts: Query<&mut TextColor>,
    mut drawn: Local<Option<(f64, bool)>>,
) {
    let Some(came_up) = state.came_up else { return };
    // The slab: grows across the name to 40 %, then leaves at its right.
    let t = (clock.now - came_up) as f32;
    let (at, dur) = (look.dur("matchend", "slab_at", 0.55), look.dur("matchend", "slab", 0.9));
    if let (Ok(name), Ok((mut node, mut vis))) = (name.single(), slab.single_mut()) {
        let w = name.size().x * name.inverse_scale_factor();
        let p = ((t - at) / dur).clamp(0.0, 1.0);
        let k = look.theme.ease("io").sample(p);
        let pad = look.units.cq(1.0);
        let lean = node_lean(&look, name.size().y * name.inverse_scale_factor());
        let (left, width) = if k < 0.4 {
            (-pad, (w + 2.0 * pad) * k / 0.4)
        } else {
            let q = (k - 0.4) / 0.6;
            let left = -pad + (w + 2.0 * pad) * q;
            (left, (w + pad - left).max(0.0) * (1.0 - q) + 0.0)
        };
        let show = p > 0.0 && p < 1.0 && width > 0.5;
        vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
        let (l, wd) = (px(left.round()), px((width + lean).round()));
        if node.left != l || node.width != wd {
            node.left = l;
            node.width = wd;
        }
    }
    // The climb's line and ticks.
    let draw = look.dur("matchend", "climb_draw", 0.7);
    let k = look.units.cq(0.1) * (EndLayout::of(&look).graph_w / 47.0);
    let elapsed = state.drawing.map(|d| (clock.now - d) as f32);
    let done = elapsed.is_none_or(|e| e > draw.max(1.2));
    let key = (state.drawing.unwrap_or(-1.0), done);
    if *drawn == Some(key) && done {
        return;
    }
    *drawn = Some(key);
    let progress = elapsed.map_or(1.0, |e| look.theme.ease("out").sample((e / draw).clamp(0.0, 1.0)));
    for (s, mut node) in &mut segs {
        let n = seg_node(s, k, progress);
        if node.left != n.left || node.width != n.width || node.height != n.height || node.top != n.top {
            *node = n;
        }
    }
    for (tick, bg, border, children) in &mut ticks {
        let a = elapsed.map_or(1.0, |e| ((e - tick.delay) / tick.dur).clamp(0.0, 1.0));
        // The final label has no fill of its own (a node's default
        // background is transparent black: never lit).
        if let Some(mut bg) = bg.filter(|_| tick.kind != TickKind::Final) {
            let base = match tick.kind {
                TickKind::Death => 0.75,
                TickKind::Stall => 0.06,
                _ => 1.0,
            };
            // Display-space alphas (`Theme::tone_a`) scaled by the fade.
            let c = match tick.kind {
                TickKind::Death => look.theme.tone_a(Tone::Threat, base * a),
                TickKind::Stall => look.theme.tone_a(Tone::Fg, base * a),
                TickKind::Dash => look.theme.tone_a(Tone::Fg, 0.42 * a),
                TickKind::Plate => look.theme.tone_a(Tone::Ink, 0.85 * a),
                _ => bg.0.with_alpha(a),
            };
            if bg.0 != c {
                bg.0 = c;
            }
        }
        if let Some(mut b) = border {
            let c = look.theme.tone_a(Tone::Fg, 0.42 * a);
            if b.top != c {
                *b = BorderColor::all(c);
            }
        }
        for c in children.into_iter().flat_map(|c| c.iter()) {
            if let Ok(mut t) = texts.get_mut(c) {
                let c = t.0.with_alpha(a);
                if t.0 != c {
                    t.0 = c;
                }
            }
        }
    }
}

/// A slanted node's lean over its height `h` (logical px).
fn node_lean(look: &Look, h: f32) -> f32 {
    h * look.theme.tan()
}

/// A staged screen's fixture (`ui_scene matchend <fixture.json>`;
/// tools/ui-parity/fixtures/matchend.json): the look book's match end.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EndFixture {
    pub map: String,
    pub map_game: String,
    pub title: String,
    pub rungs: u32,
    pub length: f32,
    pub rows: Vec<EndFixtureRow>,
    pub vote: Option<EndFixtureVote>,
    /// The countdown frozen at this many seconds left.
    pub left: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EndFixtureRow {
    pub name: String,
    pub rung: u32,
    pub kills: i32,
    pub deaths: u32,
    pub gun: String,
    pub bot: bool,
    pub host: bool,
    pub you: bool,
    pub winner: bool,
    pub wins: u32,
    pub ping: u16,
    pub loss: bool,
    /// Seconds of each rung-up (to rung 2, 3, ...).
    pub ups: Vec<f32>,
    pub kill_times: Vec<f32>,
    pub death_times: Vec<f32>,
    pub last_victim: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EndFixtureVote {
    pub maps: Vec<String>,
    pub games: Vec<String>,
    pub counts: Vec<u32>,
    pub mine: Option<usize>,
    pub winner: Option<usize>,
}

impl EndFixture {
    pub fn view(&self) -> MatchEndView {
        MatchEndView {
            shown: true,
            title: self.title.clone(),
            winner: self.rows.iter().find(|r| r.winner).map(|r| r.name.clone()),
            you_won: self.rows.iter().any(|r| r.winner && r.you),
            rows: self
                .rows
                .iter()
                .enumerate()
                .map(|(i, r)| MatchEndRow {
                    place: i + 1,
                    name: r.name.clone(),
                    kills: r.kills,
                    deaths: r.deaths,
                    gun: r.gun.clone(),
                    gun_full: r.gun.clone(),
                    wins: r.wins,
                    winner: r.winner,
                    local: r.you,
                    rung: Some(r.rung),
                    gun_id: None,
                    class: None,
                    bot: r.bot,
                    host: r.host,
                    ping: r.ping,
                    loss: r.loss,
                    climb: Climb {
                        steps: r.ups.iter().enumerate().map(|(k, t)| (*t, k as u16 + 2)).collect(),
                        kills: r.kill_times.clone(),
                        deaths: r.death_times.clone(),
                        last_victim: r.last_victim.clone(),
                    },
                })
                .collect(),
            vote: self.vote.as_ref().map(|v| MatchEndVote {
                maps: v.maps.clone(),
                counts: v.counts.clone(),
                mine: v.mine,
                winner: v.winner,
                games: v.games.clone(),
            }),
            map: self.map.clone(),
            map_game: self.map_game.clone(),
            rungs: self.rungs,
            length: self.length,
            ..default()
        }
    }
}

/// A made-up match end (`ui_scene matchend` without a fixture): eight
/// players, you the host in third, a vote under way.
pub fn staged_fixture() -> EndFixture {
    let mut rng = chimera_sim::minigame::auto::Rng::new(7);
    let names = [
        ("nadia.exe", 14, 31, 12, false, false, true, 4, 38, false),
        ("lowgrav_larry", 12, 26, 15, false, false, false, 1, 72, false),
        ("kestrel", 11, 22, 14, false, true, false, 2, 0, false),
        ("Sgt. Biscuit", 10, 21, 17, true, false, false, 0, 0, false),
        ("vex", 9, 18, 13, false, false, false, 1, 140, false),
        ("mothman", 7, 13, 19, false, false, false, 0, 240, true),
        ("Rook", 6, 11, 20, true, false, false, 0, 0, false),
        ("pixelpirate", 4, 6, 22, false, false, false, 0, 55, false),
    ];
    let end = 598.0;
    let mut uniform = |a: f32, b: f32| a + (b - a) * (rng.next() % 10_000) as f32 / 10_000.0;
    let rows = names
        .iter()
        .map(|&(name, rung, kills, deaths, bot, you, winner, wins, ping, loss)| {
            let mut ups: Vec<f32> = if winner {
                vec![31.0, 62.0, 100.0, 125.0, 178.0, 210.0, 252.0, 320.0, 351.0, 400.0, 468.0, 515.0, 560.0]
            } else {
                let last = uniform(430.0, 585.0);
                let mut u: Vec<f32> = (0..rung - 1).map(|_| uniform(15.0, last)).collect();
                u.sort_by(f32::total_cmp);
                for i in 1..u.len() {
                    u[i] = u[i].max(u[i - 1] + 9.0);
                }
                u
            };
            ups.iter_mut().for_each(|u| *u = u.min(590.0).round());
            let mut kill_times: Vec<f32> = ups.clone();
            while kill_times.len() < kills as usize {
                kill_times.push(uniform(8.0, end - 6.0).round());
            }
            kill_times.sort_by(f32::total_cmp);
            let mut death_times: Vec<f32> = (0..deaths).map(|_| uniform(10.0, end - 10.0).round()).collect();
            death_times.sort_by(f32::total_cmp);
            EndFixtureRow {
                name: name.into(),
                rung,
                kills,
                deaths,
                gun: if winner { "Knife".into() } else { String::new() },
                bot,
                host: you,
                you,
                winner,
                wins,
                ping,
                loss,
                ups,
                kill_times,
                death_times,
                last_victim: winner.then(|| "lowgrav_larry".into()),
            }
        })
        .collect();
    EndFixture {
        map: "de_dust2".into(),
        map_game: "CS:S".into(),
        title: "Gun game".into(),
        rungs: 14,
        length: end,
        rows,
        vote: Some(EndFixtureVote {
            maps: vec!["de_dust2".into(), "cs_office".into(), "dm_lockdown".into(), "warehouse".into()],
            games: vec!["CS:S".into(), "CS:S".into(), "HL2:DM".into(), "CA".into()],
            counts: vec![3, 1, 2, 0],
            mine: Some(2),
            winner: None,
        }),
        left: Some(13.0),
    }
}

/// Stages the screen (`ui_scene matchend [fixture]`), or clears it.
pub fn stage(world: &mut World, fixture: Option<&str>) -> Result<(), String> {
    let f = match fixture {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("ui_scene: {path}: {e}"))?;
            serde_json::from_str::<EndFixture>(&text).map_err(|e| format!("ui_scene: {path}: {e}"))?
        }
        None => staged_fixture(),
    };
    world.insert_resource(MatchEndStaged { view: f.view(), left: f.left });
    Ok(())
}

/// Clears a staged screen.
pub fn unstage(world: &mut World) {
    world.remove_resource::<MatchEndStaged>();
    if let Some(mut v) = world.get_resource_mut::<MatchEndView>() {
        *v = MatchEndView { sounds_asked: v.sounds_asked, ..default() };
    }
}

// ===== the vote band (step 2) =====

/// The vote band's part.
#[derive(Component)]
struct VotePart;

/// A map card: its candidate, whether it is yours, the vote locked, the
/// winner.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoteCard {
    pub index: usize,
    pub mine: bool,
    pub locked: bool,
    pub won: bool,
}

/// The one countdown's number: the vote's close, the next match's start.
#[derive(Component)]
pub struct VoteCountdown;

/// The countdown's tick bar.
#[derive(Component, Clone, Debug)]
pub struct VoteTicks {
    pub material: Handle<TickBarMaterial>,
    pub ticks: u32,
}

/// Seconds left on the one countdown at `now` (`SimClock`): to the vote's
/// close (the next match comes with it), else to the next match; 0 once
/// the vote is locked; frozen when staged.
pub fn seconds_left(view: &MatchEndView, now: f64, frozen: Option<f64>) -> f64 {
    if view.vote.as_ref().is_some_and(|v| !v.open()) {
        return 0.0;
    }
    if let Some(f) = frozen {
        return f.max(0.0);
    }
    (view.vote_closes.unwrap_or(view.next_at) - now).max(0.0)
}

/// The countdown's text (`0:13`).
pub fn countdown_text(left: f64) -> String {
    let s = left.ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The line under the countdown: what 0 does, then the winning map.
pub fn vote_hint(view: &MatchEndView) -> String {
    match &view.vote {
        Some(v) if !v.open() => match v.winner.and_then(|w| v.maps.get(w)) {
            Some(m) => format!("Next: {m} \u{2022} starting"),
            None => "Next match starting".into(),
        },
        Some(_) => "Vote closes and the next match starts at 00:00".into(),
        None => "The next match starts at 00:00".into(),
    }
}

/// `map` to a pair of colours, the cards' stand-in for a thumbnail.
fn map_tint(map: &str) -> (Color, Color) {
    let h = map.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let hue = (h % 360) as f32;
    (Color::hsl(hue, 0.20, 0.26), Color::hsl(220.0, 0.30, 0.09))
}

/// The pips a card shows (one a vote, the look book's eight).
pub const PIPS: usize = 8;

/// The vote band (rebuilt when the vote changes): the countdown, the map
/// cards, the way out.
#[allow(clippy::too_many_arguments)]
fn build_vote(
    look: Look,
    view: Res<MatchEndView>,
    root: Single<Entity, With<SignalEndRoot>>,
    old: Query<Entity, With<VotePart>>,
    mut state: ResMut<EndState>,
    mut slants: Option<ResMut<Assets<SlantMaterial>>>,
    mut ticks: Option<ResMut<Assets<TickBarMaterial>>>,
    mut commands: Commands,
) {
    let key = (view.vote.clone(), view.shown, units_key(&look));
    if state.vote_key.as_ref() == Some(&key) && !look.theme.is_changed() {
        return;
    }
    state.vote_key = Some(key);
    for e in &old {
        commands.entity(e).despawn();
    }
    if !view.shown {
        return;
    }
    let came_up = state.came_up.unwrap_or(0.0);
    let hair = look.hair();
    let tan = look.theme.tan();
    let lay = EndLayout::of(&look);
    let ui = chimera_ui::controls::Ui { theme: &look.theme, kit: look.fonts.as_deref(), units: *look.units };
    let n = 40;
    // Two columns; the rows share the band, so the cards keep one size
    // whatever the votes (more than four maps: more rows, smaller pictures).
    let maps_n = view.vote.as_ref().map_or(0, |v| v.maps.len());
    let rows = maps_n.div_ceil(2).max(2) as f32;
    let card_w = (lay.mid_w - CARD_GAP - 0.8) / 2.0;
    let card_h = ((lay.vote_h - 1.2 - CARD_GAP * (rows - 1.0)) / rows).max(CARD_BODY + 1.5);
    let thumb_h = card_h - CARD_BODY;
    commands
        .spawn((VotePart, ChildOf(*root), Pickable::IGNORE, Node { width: look.val(100.0), height: look.val(56.25), position_type: PositionType::Absolute, align_self: AlignSelf::Center, ..default() }))
        .with_children(|f| {
            f.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    padding: UiRect::top(look.val(0.9)),
                    border: UiRect::top(px(hair)),
                    align_items: AlignItems::FlexStart,
                    height: look.val(lay.vote_h),
                    ..abs(&look, lay.mid_x, lay.vote_top, Some(lay.mid_w), None)
                },
                BorderColor::all(look.tone(Tone::Fg4)),
                enter(&look, came_up, 1.5, 0.8, 0.0),
                UiParityId::new("end.vote"),
                Pickable::IGNORE,
            ))
            .with_children(|b| {
                // The map cards.
                b.spawn((Node { flex_grow: 1.0, flex_wrap: FlexWrap::Wrap, column_gap: look.val(CARD_GAP), row_gap: look.val(CARD_GAP), align_items: AlignItems::FlexStart, align_content: AlignContent::FlexStart, ..default() }, Pickable::IGNORE)).with_children(|cards| {
                    let Some(v) = &view.vote else { return };
                    let locked = !v.open();
                    let best = v.counts.iter().copied().max().unwrap_or(0);
                    let lead = (best > 0).then(|| v.counts.iter().position(|c| *c == best)).flatten();
                    for (i, map) in v.maps.iter().enumerate() {
                        let p = format!("end.vote.card.{}", i + 1);
                        let mine = v.mine == Some(i);
                        let won = v.winner == Some(i);
                        let count = v.counts.get(i).copied().unwrap_or(0);
                        let (line, w) = if locked && won {
                            (look.tone(Tone::Fg), look.px(0.14))
                        } else if mine {
                            (look.theme.tone_a(Tone::Acc, if locked { 0.35 } else { 1.0 }), look.px(0.14))
                        } else {
                            (look.theme.tone_a(Tone::Fg, if locked { 0.15 * 0.35 } else { 0.15 }), hair)
                        };
                        let mut card = cards.spawn((
                            VoteCard { index: i, mine, locked, won },
                            super::match_end::VoteButton(i),
                            // The kit's item: its fill and hover.
                            ui.item_bundle(Some((Tone::Ink, 0.75)), chimera_ui::controls::State { disabled: locked, ..chimera_ui::controls::State::NONE }, false),
                            Node { width: look.val(card_w), height: look.val(card_h), flex_shrink: 0.0, flex_direction: FlexDirection::Column, overflow: Overflow::clip(), ..default() },
                            Outline::new(px(w), px(-w), line),
                            UiParityId(p.clone()),
                        ));
                        // Locked, the others dim to 35 %.
                        let dim = if locked && !won { 0.35 } else { 1.0 };
                        let tone = |t: Tone| look.theme.tone_a(t, dim);
                        card.with_children(|c| {
                            let (a, b) = map_tint(map);
                            let (a, b) = (a.with_alpha(dim), b.with_alpha(dim));
                            c.spawn((
                                Node { height: look.val(thumb_h), ..default() },
                                BackgroundGradient::from(LinearGradient::degrees(110.0, vec![ColorStop::percent(a, 0.0), ColorStop::percent(b, 100.0)])),
                                UiParityId(format!("{p}.thumb")),
                                Pickable::IGNORE,
                            ))
                            .with_children(|t| {
                                t.spawn((
                                    Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
                                    BackgroundGradient::from(LinearGradient::to_bottom(vec![
                                        ColorStop::percent(Color::NONE, 40.0),
                                        ColorStop::percent(look.theme.tone_a(Tone::Ink, 0.7), 100.0),
                                    ])),
                                    Pickable::IGNORE,
                                ));
                            });
                            if mine {
                                c.spawn((Node { position_type: PositionType::Absolute, right: look.val(0.5), top: look.val(0.5), ..default() }, Pickable::IGNORE)).with_children(|t| {
                                    signal::tag(t, &look, TagKind::Accent, "Your vote", 0.62);
                                });
                            }
                            c.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexStart, row_gap: look.val(0.4), padding: UiRect::new(look.val(0.8), look.val(0.8), look.val(0.7), look.val(0.6)), ..default() }, Pickable::IGNORE)).with_children(|m| {
                                m.spawn((
                                    look.tracked(Role::DisplayBold, 0.78, 0.78 * 0.03, tone(Tone::Fg), &map.to_uppercase()),
                                    Ellipsize::in_parent(map.to_uppercase()),
                                    UiParityId(format!("{p}.name")),
                                ));
                                let game = v.games.get(i).cloned().unwrap_or_default();
                                m.spawn((look.tracked(Role::Mono, 0.48, 0.048, look.theme.tone_a(Tone::Fg, 0.42 * dim), &game.to_uppercase()), UiParityId(format!("{p}.game"))));
                                m.spawn((Node { align_items: AlignItems::Center, align_self: AlignSelf::Stretch, column_gap: look.val(0.5), margin: UiRect::top(look.val(0.15)), ..default() }, Pickable::IGNORE)).with_children(|r| {
                                    r.spawn((Node { column_gap: look.val(0.16), ..default() }, UiParityId(format!("{p}.pips")), Pickable::IGNORE)).with_children(|pips| {
                                        for k in 0..PIPS {
                                            let color = if k < count as usize {
                                                tone(if mine && k + 1 == count as usize { Tone::Acc } else { Tone::Fg })
                                            } else {
                                                look.theme.tone_a(Tone::Fg, 0.15 * dim)
                                            };
                                            // The box holds the lean; the layout counts .5 cq (as CSS's skew).
                                            let mut pip = pips.spawn((
                                                Node { width: look.val(0.5 + 0.5 * tan), height: look.val(0.5), margin: UiRect::right(look.val(-0.5 * tan)), ..default() },
                                                Pickable::IGNORE,
                                            ));
                                            if let Some(slants) = slants.as_deref_mut() {
                                                pip.insert(MaterialNode(slants.add(SlantMaterial::new(&look.theme, color))));
                                            }
                                        }
                                    });
                                    r.spawn((Node { flex_grow: 1.0, ..default() }, Pickable::IGNORE));
                                    let t = if lead == Some(i) { Tone::AccText } else { Tone::Fg };
                                    r.spawn((look.tracked(Role::DisplayBold, 0.74, 0.0, tone(t), &count.to_string()), UiParityId(format!("{p}.count"))));
                                });
                            });
                        });
                    }
                });
            });
            // The countdown, bottom right: "Next match 0:14 >" over its ticks.
            f.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::FlexEnd,
                    row_gap: look.val(0.7),
                    padding: UiRect::top(look.val(0.9)),
                    border: UiRect::top(px(hair)),
                    height: look.val(lay.foot_h),
                    ..abs(&look, lay.std_x, lay.foot_top, Some(lay.std_w), None)
                },
                BorderColor::all(look.tone(Tone::Fg4)),
                VoteCountdownColumn,
                enter(&look, came_up, 1.5, 0.8, 0.0),
                Pickable::IGNORE,
            ))
            .with_children(|c| {
                if view.vote.as_ref().is_some_and(|v| !v.open()) {
                    c.spawn((
                        look.tracked(Role::Mono, 0.5, 0.05, look.tone(Tone::Fg3), &vote_hint(&view)),
                        Ellipsize::max(vote_hint(&view), lay.std_w),
                        UiParityId::new("end.vote.hint"),
                    ));
                }
                c.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(1.0), ..default() }, Pickable::IGNORE)).with_children(|r| {
                    r.spawn((look.tracked(Role::Mono, 0.62, 0.062, look.tone(Tone::Fg3), "NEXT MATCH"), UiParityId::new("end.vote.tag")));
                    r.spawn((look.tracked(Role::DisplayBold, 2.2, 2.2 * 0.02, look.tone(Tone::Fg), "0:00"), VoteCountdown, UiParityId::new("end.vote.count")));
                    signal::icons::spawn(r, &look, "arrow_right", look.px(1.0), look.tone(Tone::AccText));
                });
                let u = *look.units;
                let layout = TickLayout::new(u.phys(u.cq(lay.std_w)).floor() as u32, u.phys(u.cq(0.9)).floor() as u32, n);
                let sf = u.scale_factor.max(1e-3);
                c.spawn((Node { height: look.val(0.9), ..default() }, Pickable::IGNORE)).with_children(|w| {
                    let mut bar = w.spawn((
                        Node { width: px(layout.w as f32 / sf), height: px(layout.h as f32 / sf), ..default() },
                        UiParityId::new("end.vote.ticks"),
                        Pickable::IGNORE,
                    ));
                    if let Some(ticks) = ticks.as_deref_mut() {
                        let material = ticks.add(TickBarMaterial::new(&look.theme, n, layout));
                        bar.insert((MaterialNode(material.clone()), VoteTicks { material, ticks: n }));
                    }
                });
            });
        });
}

/// The footer's right column: "Next match", the countdown and its ticks.
#[derive(Component)]
pub struct VoteCountdownColumn;

/// The gap between the map cards, cq; they sit two to a row.
const CARD_GAP: f32 = 0.9;
/// A card's height under its picture, cq: fixed, so voting (or hovering)
/// changes only colours, the outline and the badge, never the box.
const CARD_BODY: f32 = 5.4;

/// The one countdown's number and ticks, every frame from the clock.
fn countdown(
    look: Look,
    view: Res<MatchEndView>,
    staged: Option<Res<MatchEndStaged>>,
    clock: Option<Res<crate::core::SimClock>>,
    mut texts: Query<&mut Text, With<VoteCountdown>>,
    bars: Query<&VoteTicks>,
    mut mats: Option<ResMut<Assets<TickBarMaterial>>>,
) {
    if !view.shown {
        return;
    }
    let now = clock.map_or(0.0, |c| c.now);
    let left = seconds_left(&view, now, staged.and_then(|s| s.left));
    let text = countdown_text(left);
    for mut t in &mut texts {
        if t.0 != text {
            t.0 = text.clone();
        }
    }
    let total = look.dur("matchend", "vote", 20.0).max(1.0);
    let hot_from = look.dur("matchend", "vote_hot_from", 17.0);
    let Some(mats) = mats.as_deref_mut() else { return };
    for bar in &bars {
        let n = bar.ticks as f32;
        let lit = (((total as f64 - left) / total as f64).clamp(0.0, 1.0) as f32 * n).floor() as u32;
        let hot = (hot_from / total * n).floor() as u32;
        if let Some(m) = mats.get(&bar.material)
            && (m.counts.y != lit as f32 || m.counts.z != hot as f32)
            && let Some(mut m) = mats.get_mut(&bar.material)
        {
            m.set(lit, hot);
        }
    }
}

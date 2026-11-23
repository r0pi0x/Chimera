//! The match-end screen in the Signal look (docs/plans/active/ugg-ui.md
//! 3, M4 and "Match end v2"), drawn instead of the shell's (`match_end`)
//! when the app's theme turns the Signal HUD on (`signal::signal_on`: the
//! Ultimate Gun Game; Lucker Party and the plain apps keep theirs). Same
//! state (`match_end::MatchEndView`, from the results and the vote), same
//! vote input; this is presentation. The user's choice of 2026-11-22
//! (the "Lobby and Match End Directions" mockup, direction A, reviewed):
//!
//! On the ink, a 16:9 frame (100 x 56.25 cq, centred). Blocks sit where
//! `[layout.matchend]` puts them (`EndLayout`), each with a fixed place:
//!
//! - **Leave** (`LeaveButton`), the kit's button, top left; leaves for the
//!   menu (`disconnect`). The chat (`signal_chat`, docked with the same
//!   numbers) is the left column under it, down to the bottom.
//! - **The winner band** (`winner_moment::WinnerBand`), top centre: the
//!   winner's avatar, WINNER, the name large with the accent slab sweeping
//!   over it, the line (all the rungs in the time, the final kill), the
//!   session wins and the quick chat (GG, WP, Rematch?, which post to
//!   chat; GG with its burst). It is the winner moment's card, moved into
//!   place (`winner_moment`: one element that transforms).
//! - **Details** (`DetailsLine`) above it: map (and its game), players,
//!   rungs and the match's time, each after an icon.
//! - **The climb** (`RungBars`): a bar per rung for the selected player
//!   (you by default, else the winner; a click on a standings row picks
//!   another): its height the time spent on that rung, the time over it,
//!   the rung's number under it; the fastest rung solid accent, the
//!   longest stall in the warning tone, the rung reached but not finished
//!   outlined, rungs never reached an empty frame (`rung_times`). A key
//!   line under it. No gun names (they don't fit: "Plasma P").
//! - **Stat tiles** (`StatsRow`, `StatTile`; `stats_of`) under it, of the
//!   same player: place, rung reached, kills, K/D, best streak, fastest
//!   rung and longest stall ("Streak", "Fastest", "Stall"; times only).
//! - **The vote**: the map cards (`VoteCard`), two to a row (a 2x2 grid
//!   for four maps; more maps add rows and the cards shrink to fit), of a
//!   fixed size: the number key's cap, the picture with the map's name and
//!   game once over its foot, then the pips and the count. Open from the
//!   screen's first frame; keys 1-4 or a click vote
//!   (`match_end::VoteButton`, `vote_input`). At 0 the vote locks: the
//!   others dim, the winner is outlined.
//! - **Standings** (`StandingsPanel`, `StandingsRow`), the right column,
//!   top-aligned, above the countdown: position, avatar, the name with
//!   HOST/YOU and the session wins ("+1" on the winner's), rung/total,
//!   kills; the winner's row accented. Rows keep their size (`row_h`); a
//!   longer list scrolls in the kit's scroll view with its bar, and the
//!   kit's "more below" mark at the bottom edge says "+N more"
//!   (`MoreBelow`). Hover is a faint fill; the selected row a stronger one
//!   and a pointer.
//! - **The countdown** (`VoteCountdownColumn`), bottom right: "Next match",
//!   the number big and the tick bar filling the area's width
//!   (`VoteCountdown`, `VoteTicks`: lit as it runs, the accent from
//!   `vote_hot_from` of `vote` s). No text under it.
//! - **The ground** (`EndGround`): the shared ambient with the match
//!   end's bloom and the slanted planes, shown once the world has faded.
//!
//! Each part is rebuilt only when what it shows changes; the entrance
//! (fades and steps from `EndState::came_up`, the winner moment's move
//! when there was one) runs on the kit's clock (`UiClock`). Nothing blocks
//! input: every part takes clicks from its first frame, faded or not.
//! The cursor is free while the screen is up (`match_end::vote_cursor`);
//! the games' HUDs are hidden under it (`hud::HudCover`).
//! `ui_scene matchend [fixture] [moment]` stages it (`stage`; `moment`
//! plays the winner moment first). Nodes carry the look book's names
//! (`UiParityId`: `end.*`).
//!
//! Known shortcuts: no headshot tile (not tracked); map cards show a
//! tinted gradient (no map shots); a listen host's Leave gets no
//! confirmation (the Esc menu's warns about the people connected).

use bevy::{prelude::*, window::PrimaryWindow};
use chimera_sim::minigame::climb::Climb;
use chimera_ui::{Ellipsize, Role, SlantMaterial, Theme, TickBarMaterial, TickLayout, Timeline, Tone, Track, UiClock, UiParityId, parity, Prop, style::UiStyle};
use serde::Deserialize;

use super::{
    match_end::{MATCH_END_Z, MatchEndRow, MatchEndStaged, MatchEndView, MatchEndVote},
    signal::{self, Look, Mark, TagKind},
    winner_moment::{self, WinnerMoment},
};

/// Whether the Signal screen draws (`flags.hud.signal`).
pub fn themed(theme: Option<Res<Theme>>) -> bool {
    signal::signal_on(theme.as_deref())
}

/// The screen's root.
#[derive(Component)]
pub struct SignalEndRoot;

/// The header and the standings (rebuilt together).
#[derive(Component)]
struct StaticPart;

/// The climb's bars and the tiles (rebuilt with the selection).
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

/// The standings' scroll view's key (`ScrollViews`).
pub const STANDINGS_SCROLL: &str = "matchend.standings";

/// The winner's standings row.
#[derive(Component)]
pub struct WinnerRow;

/// The fade and "+N more" at the standings' bottom edge while players
/// are below the view (`n`: how many).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoreBelow {
    pub n: usize,
}

/// The details line at the top: map, players, rungs, time.
#[derive(Component)]
pub struct DetailsLine;

/// The stat tiles' row.
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

/// The climb's bars: whose (an index in the rows).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RungBars {
    pub index: usize,
}

/// One rung's bar.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RungBar {
    /// Counted from 1.
    pub rung: usize,
    pub kind: BarKind,
}

/// How a rung's bar is marked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarKind {
    Plain,
    Fastest,
    Stall,
    /// Reached, not finished: outlined.
    Open,
    /// Never reached: an empty frame.
    Unreached,
}

/// Whose stats the screen shows first: the local player's, else the
/// winner's (a spectator, or a player with no entry); true when yours.
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

/// Time spent on each rung (`secs[0]` is rung 1; None: never reached),
/// the rung reached but not finished, the fastest finished rung and the
/// longest stall (a finished rung, the longest time; marked only with
/// two or more finished rungs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RungTimes {
    pub secs: Vec<Option<f32>>,
    pub open: Option<usize>,
    pub fastest: Option<usize>,
    pub stall: Option<usize>,
}

impl RungTimes {
    pub fn kind(&self, i: usize) -> BarKind {
        match self.secs.get(i).copied().flatten() {
            None => BarKind::Unreached,
            Some(_) if self.open == Some(i) => BarKind::Open,
            Some(_) if self.fastest == Some(i) => BarKind::Fastest,
            Some(_) if self.stall == Some(i) => BarKind::Stall,
            Some(_) => BarKind::Plain,
        }
    }
}

/// `row`'s time per rung over `total` rungs (more when it went higher)
/// until the match's `end` s; the winner's last rung ends at the winning
/// kill and counts as finished.
pub fn rung_times(row: &MatchEndRow, end: f32, total: u32) -> RungTimes {
    let climb = &row.climb;
    let top = climb.steps.iter().map(|s| s.1 as usize).max().unwrap_or(1).max(row.rung.unwrap_or(1) as usize);
    let mut secs: Vec<Option<f32>> = vec![None; (total as usize).max(top).max(1)];
    let finish = if row.winner { climb.kills.last().copied().filter(|k| *k > climb.steps.last().map_or(0.0, |s| s.0)).unwrap_or(end).min(end) } else { end };
    let (mut at, mut rung) = (0.0f32, 1usize);
    for &(t, r) in climb.steps.iter().chain(std::iter::once(&(finish, 0))) {
        let t = t.min(finish);
        if let Some(s) = secs.get_mut(rung - 1) {
            *s = Some(s.unwrap_or(0.0) + (t - at).max(0.0));
        }
        at = t;
        rung = (r as usize).max(1);
    }
    let last = climb.steps.last().map_or(1, |s| s.1 as usize).max(1);
    let open = (!row.winner).then_some(last - 1);
    let done: Vec<(usize, f32)> = secs.iter().enumerate().filter(|(i, s)| Some(*i) != open && s.is_some_and(|s| s > 0.0)).map(|(i, s)| (i, s.unwrap_or(0.0))).collect();
    let (fastest, stall) = if done.len() >= 2 {
        (done.iter().min_by(|a, b| a.1.total_cmp(&b.1)).map(|d| d.0), done.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|d| d.0))
    } else {
        (done.first().map(|d| d.0), None)
    };
    RungTimes { secs, open, fastest, stall }
}

/// `1st`, `2nd`, `3rd`, `4th`, ... `11th`, `21st`.
pub fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// The tiles for `row`: place, rung reached, kills, K/D, best streak,
/// the fastest rung and the longest stall (times only; no gun names, no
/// "new best": session bests mean nothing yet). Headshots are not tracked.
pub fn stats_of(row: &MatchEndRow, length: f32, rungs: u32) -> Vec<EndStat> {
    let stat = |label, value: String| EndStat { label, value };
    let times = rung_times(row, length, rungs);
    let kd = if row.deaths == 0 { format!("{:.1}", row.kills.max(0) as f32) } else { format!("{:.2}", row.kills as f32 / row.deaths as f32) };
    let at = |i: Option<usize>| i.and_then(|i| times.secs[i]).map_or("-".into(), mmss);
    vec![
        stat("Place", ordinal(row.place)),
        stat("Rung", rung_of(row, rungs)),
        stat("Kills", row.kills.to_string()),
        stat("K/D", kd),
        stat("Streak", best_streak(&row.climb).to_string()),
        stat("Fastest", at(times.fastest)),
        stat("Stall", at(times.stall)),
    ]
}

/// A row's selection pointer.
#[derive(Component)]
struct RowPointer;

/// Which row's climb and stats show: the selected one (a click; yours
/// when the screen comes up, else the winner's); the hovered one gets a
/// fill.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct EndSelection {
    pub hovered: Option<usize>,
    pub pinned: usize,
}

impl EndSelection {
    pub fn shown(&self) -> usize {
        self.pinned
    }
}

/// The screen's own state: when it came up (`UiClock`; the winner
/// moment's move when there was one), what each part was built from.
#[derive(Resource, Clone, Debug, Default)]
pub struct EndState {
    pub came_up: Option<f64>,
    /// The clock at the last `show` (the parts' entrances start from it).
    pub now: f64,
    /// The rows' names it came up with.
    rows: Vec<(String, bool)>,
    static_key: Option<(MatchEndView, (u32, u32), String)>,
    climb_key: Option<(usize, MatchEndView, (u32, u32))>,
    vote_key: Option<(Option<MatchEndVote>, bool, (u32, u32))>,
    /// The left-pointing triangle by height in physical px.
    pointer: Option<(u32, Handle<Image>)>,
}

impl EndState {
    /// Every part built again (a skipped moment: their entrances done).
    pub fn rebuild(&mut self) {
        self.static_key = None;
        self.climb_key = None;
        self.vote_key = None;
    }
}

/// The screen's systems (added by `MatchEndPlugin`).
pub fn add(app: &mut App) {
    super::signal::setup(app);
    super::hud::hud_cover(app);
    winner_moment::add(app);
    app.init_resource::<EndSelection>().init_resource::<EndState>().add_systems(Startup, spawn).add_systems(
        Update,
        (
            show,
            ground,
            hide_huds,
            pick,
            pointer_image,
            build_static,
            build_climb,
            build_vote,
            winner_moment::band,
            gate_parts,
            fill_avatars,
            decorate_rows,
            more_below,
            countdown,
            leave,
        )
            .chain()
            .after(super::match_end::MatchEndFollow)
            .after(winner_moment::MomentSet)
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
        Pickable::IGNORE,
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

/// The root shown with the view or the winner moment, its ink as opaque
/// as the moment's world fade has it; when the screen came up.
fn show(
    view: Res<MatchEndView>,
    look: Look,
    clock: Res<UiClock>,
    moment: Res<WinnerMoment>,
    mut root: Query<(&mut Visibility, &mut BackgroundColor), With<SignalEndRoot>>,
    mut state: ResMut<EndState>,
    mut sel: ResMut<EndSelection>,
) {
    let times = winner_moment::MomentTimes::of(&look);
    state.now = clock.now;
    let up = view.shown || moment.running();
    for (mut v, mut bg) in &mut root {
        v.set_if_neq(if up { Visibility::Inherited } else { Visibility::Hidden });
        let ink = look.theme.tone_a(Tone::Ink, moment.scrim(clock.now, &times));
        if bg.0 != ink {
            bg.0 = ink;
        }
    }
    // Other players (a staged view replaced): the selection again.
    let names: Vec<(String, bool)> = view.rows.iter().map(|r| (r.name.clone(), r.local)).collect();
    if view.shown && state.came_up.is_some() && state.rows != names {
        state.came_up = None;
    }
    state.rows = names;
    match (view.shown, state.came_up) {
        (true, None) => {
            // With a moment the rest comes in as its card moves into place.
            state.came_up = Some(moment.move_at(&times).unwrap_or(clock.now));
            let first = view.rows.iter().position(|r| r.local).or_else(|| view.rows.iter().position(|r| r.winner)).unwrap_or(0);
            *sel = EndSelection { hovered: None, pinned: first };
        }
        (false, Some(_)) => {
            state.came_up = None;
            state.rebuild();
        }
        _ => {}
    }
    if moment.is_changed() && moment.skipped && state.came_up.is_some_and(|c| c > clock.now - 5.0) {
        state.came_up = Some(clock.now - 5.0);
        state.rebuild();
    }
}

/// While the winner moment's card holds, only the card shows: the other
/// parts (and the docked chat) come in as it moves into place. Their
/// input (the vote's keys, Esc) works all along.
#[allow(clippy::type_complexity)]
fn gate_parts(
    look: Look,
    clock: Res<UiClock>,
    moment: Res<WinnerMoment>,
    mut parts: Query<&mut Visibility, Or<(With<StaticPart>, With<ClimbPart>, With<VotePart>, With<super::signal_chat::ChatDockRoot>)>>,
) {
    let t = winner_moment::MomentTimes::of(&look);
    let wait = moment.running() && moment.move_at(&t).is_none_or(|m| clock.now < m) && !(moment.skipped && moment.shown_at.is_some());
    for mut v in &mut parts {
        v.set_if_neq(if wait { Visibility::Hidden } else { Visibility::Inherited });
    }
}

/// The ground: the shared ambient (`AmbientMaterial`, the match end's
/// bloom, no calm column) and the broad slanted planes over it
/// (`LinesMaterial`): the look book's "ambient + planes". Made once a
/// theme (again when it changes), under every part; shown once the world
/// has faded out.
#[derive(Component)]
pub struct EndGround;

#[allow(clippy::too_many_arguments)]
fn ground(
    theme: Res<Theme>,
    look: Look,
    clock: Res<UiClock>,
    moment: Res<WinnerMoment>,
    root: Single<Entity, With<SignalEndRoot>>,
    mut old: Query<(Entity, &mut Visibility), With<EndGround>>,
    mut ambients: Option<ResMut<Assets<chimera_ui::AmbientMaterial>>>,
    mut planes: Option<ResMut<Assets<chimera_ui::LinesMaterial>>>,
    mut commands: Commands,
) {
    let shown = moment.scrim(clock.now, &winner_moment::MomentTimes::of(&look)) >= 0.999;
    if !old.is_empty() && !theme.is_changed() {
        for (_, mut v) in &mut old {
            v.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
        }
        return;
    }
    for (e, _) in &old {
        commands.entity(e).despawn();
    }
    let vis = if shown { Visibility::Inherited } else { Visibility::Hidden };
    let full = || Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() };
    let mut a = commands.spawn((EndGround, full(), vis, Pickable::IGNORE, ZIndex(-2)));
    if let Some(m) = ambients.as_deref_mut() {
        a.insert(MaterialNode(m.add(chimera_ui::AmbientMaterial::new(&theme))));
    }
    let a = a.id();
    let mut l = commands.spawn((EndGround, full(), vis, Pickable::IGNORE, ZIndex(-1)));
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

/// The games' HUDs covered while the screen (or the winner moment) is up:
/// `hud::HudCover`.
fn hide_huds(view: Res<MatchEndView>, moment: Res<WinnerMoment>, mut cover: ResMut<super::hud::HudCover>) {
    super::hud::HudCover::put(&mut cover, "match end", view.shown || moment.running());
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

pub(super) fn units_key(look: &Look) -> (u32, u32) {
    ((look.units.height * 8.0) as u32, (look.units.scale_factor * 1000.0) as u32)
}

/// `mm:ss` (`9:58`).
pub fn mmss(s: f32) -> String {
    let s = s.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The band's line: `14/14 in 9:58 · Knife on lowgrav_larry` (short: it
/// shares the band with the quick chat).
pub fn band_line(view: &MatchEndView) -> String {
    let Some(w) = view.rows.iter().find(|r| r.winner) else {
        return "Nobody wins the match".into();
    };
    let mut parts = vec![if view.rungs > 0 { format!("{}/{} in {}", view.rungs, view.rungs, mmss(view.length)) } else { format!("{} kills in {}", w.kills, mmss(view.length)) }];
    match (&w.climb.last_victim, w.gun.is_empty()) {
        (Some(v), false) => parts.push(format!("{} on {v}", w.gun)),
        (Some(v), true) => parts.push(format!("last kill on {v}")),
        (None, false) => parts.push(w.gun.clone()),
        _ => {}
    }
    parts.join(" \u{b7} ")
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
fn enter(look: &Look, (came_up, now): (f64, f64), at: f32, rise: f32, step: f32) -> (Timeline, UiTransform) {
    let ease = look.theme.ease("out");
    // A start still to come (the winner moment's move): started now with
    // the wait added, so the part is at its first values (unseen) until
    // then, not drawn whole.
    let (came_up, at) = if came_up > now { (now, at + (came_up - now) as f32) } else { (came_up, at) };
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
pub(super) fn abs(look: &Look, l: f32, t: f32, w: Option<f32>, r: Option<f32>) -> Node {
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

/// A part's frame: the 100 x 56.25 cq box, centred.
pub(super) fn frame_node(look: &Look) -> Node {
    Node { width: look.val(100.0), height: look.val(56.25), position_type: PositionType::Absolute, align_self: AlignSelf::Center, ..default() }
}

/// The standings' columns: position, avatar, player, rung/total, kills.
const COLS: [f32; 5] = [1.5, 1.5, 0.0, 3.6, 2.0];
const COL_GAP: f32 = 0.7;
const ROW_PAD: f32 = 0.7;

/// Where the screen's blocks sit in its 100 x 56.25 cq frame, from
/// `[layout.matchend]` (the theme's data; these are the defaults): Leave
/// top left, the chat down the left column under it to the bottom; in the
/// middle the details line, the winner band, the climb's bars with their
/// key, the stat tiles and the vote; the standings down the right above
/// the countdown. `signal_chat` docks its box with the same numbers.
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
    pub band_top: f32,
    pub band_h: f32,
    pub climb_top: f32,
    pub climb_h: f32,
    pub stats_top: f32,
    pub stats_h: f32,
    pub vote_top: f32,
    pub vote_h: f32,
    pub foot_top: f32,
    pub foot_h: f32,
    pub row_h: f32,
    /// The winner moment's card: the band this much larger, centred.
    pub moment_scale: f32,
}

impl EndLayout {
    /// The frame's height, cq.
    pub const FRAME_H: f32 = 56.25;
    /// The climb's head line, cq.
    pub const HEAD_H: f32 = 1.6;
    /// The climb's key line under its bars, cq.
    pub const KEY_H: f32 = 1.3;

    pub fn of(look: &Look) -> Self {
        Self::from(|k: &str, or: f32| look.layout("matchend", k, or))
    }

    /// From `g` (a key's value, else the default).
    pub fn from(g: impl Fn(&str, f32) -> f32) -> Self {
        let pad = g("pad", 2.5);
        let gap = g("gap", 1.6);
        let bottom = Self::FRAME_H - g("bottom", 2.5);
        let chat_x = pad;
        let chat_w = g("chat_w", 21.0);
        let chat_top = g("chat_top", 6.6);
        let std_w = g("std_w", 26.0);
        let std_x = 100.0 - pad - std_w;
        let mid_x = chat_x + chat_w + gap;
        let mid_w = std_x - gap - mid_x;
        let foot_top = g("foot_top", 40.0);
        let std_top = g("std_top", pad);
        let band_top = g("band_top", 5.0);
        let band_h = g("band_h", 7.0);
        let climb_top = band_top + band_h + g("climb_gap", 1.2);
        let climb_h = g("climb_h", 10.6);
        let stats_top = climb_top + climb_h + g("stats_gap", 0.9);
        let stats_h = g("stats_h", 4.4);
        let vote_top = stats_top + stats_h + g("vote_gap", 1.2);
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
            mid_w,
            details_top: g("details_top", 2.8),
            band_top,
            band_h,
            climb_top,
            climb_h,
            stats_top,
            stats_h,
            vote_top,
            vote_h: bottom - vote_top,
            foot_top,
            foot_h: bottom - foot_top,
            row_h: g("row_h", 2.4),
            moment_scale: g("moment_scale", 2.0).min(96.0 / mid_w).max(1.0),
        }
    }

    /// The winner band's place (cq): left, top, width, height.
    pub fn band(&self) -> Rect {
        Rect::new(self.mid_x, self.band_top, self.mid_x + self.mid_w, self.band_top + self.band_h)
    }
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

/// A player's face: the avatar they chose (`face_byte`), the robot for a
/// bot, else one drawn from the name.
pub fn face_of(name: &str, bot: bool, byte: u8) -> chimera_ui::avatar::Face {
    use chimera_ui::avatar::{AvatarLook, Face};
    if bot {
        return Face::Bot;
    }
    Face::Look(AvatarLook::from_byte(byte).unwrap_or_else(|| AvatarLook::for_name(name)))
}

/// Where an avatar goes (`fill_avatars` draws it: the kit's `avatar::spawn`
/// needs the images mutable, which the screens' `Look` reads).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AvatarSlot {
    pub face: chimera_ui::avatar::Face,
    pub you: bool,
    /// cq.
    pub size: f32,
}

/// Draws each new `AvatarSlot`'s avatar into it.
pub(super) fn fill_avatars(
    theme: Res<Theme>,
    fonts: Option<Res<chimera_ui::KitFonts>>,
    units: Res<chimera_ui::UiUnits>,
    slots: Query<(Entity, &AvatarSlot), Added<AvatarSlot>>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut icons: Option<ResMut<chimera_ui::IconImages>>,
    mut commands: Commands,
) {
    let (Some(images), Some(icons)) = (images.as_deref_mut(), icons.as_deref_mut()) else { return };
    let ui = chimera_ui::controls::Ui { theme: &theme, kit: fonts.as_deref(), units: *units };
    for (e, s) in &slots {
        chimera_ui::avatar::spawn(&ui, &mut commands, e, images, icons, s.face, chimera_ui::avatar::AvatarMarks { you: s.you, ..default() }, s.size);
    }
}

/// A player's avatar byte as the lobby last showed it (by name), or
/// yours from the profile; `NO_AVATAR` when not known.
pub fn avatar_byte(world_lobby: Option<&super::prematch_lobby::LobbyView>, mine: Option<&super::profile::AvatarSetting>, name: &str, local: bool) -> u8 {
    if local && let Some(m) = mine.filter(|m| (0..32).contains(&m.0)) {
        return m.0 as u8;
    }
    world_lobby.and_then(|l| l.rows.iter().find(|r| r.name == name)).map_or(chimera_net::prematch::NO_AVATAR, |r| r.avatar)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build_static(
    look: Look,
    view: Res<MatchEndView>,
    root: Single<Entity, With<SignalEndRoot>>,
    old: Query<Entity, With<StaticPart>>,
    mut state: ResMut<EndState>,
    mut slants: Option<ResMut<Assets<SlantMaterial>>>,
    (lobby, mine): (Option<Res<super::prematch_lobby::LobbyView>>, Option<Res<super::profile::AvatarSetting>>),
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
    let came_up = (state.came_up.unwrap_or(0.0), state.now);
    let hair = look.hair();
    let pointer = state.pointer.as_ref().map(|(_, h)| h.clone()).unwrap_or_default();
    let lay = EndLayout::of(&look);
    let ui = chimera_ui::controls::Ui { theme: &look.theme, kit: look.fonts.as_deref(), units: *look.units };
    let frame = commands.spawn((StaticPart, ChildOf(*root), Pickable::IGNORE, Node { flex_shrink: 0.0, ..frame_node(&look) })).id();
    commands.entity(frame).with_children(|f| {
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
    });

    // The standings, down the right column: rows of one size in the
    // kit's scroll view; the fade and "+N more" over the bottom edge.
    let head = 2.0;
    let panel = commands
        .spawn((
            StaticPart,
            StandingsPanel,
            ChildOf(frame),
            Node { height: look.val(lay.std_h), ..abs(&look, lay.std_x, lay.std_top, Some(lay.std_w), None) },
            enter(&look, came_up, 0.45, 0.0, 0.0),
            Pickable::IGNORE,
        ))
        .id();
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
    commands.entity(panel).with_children(|s| {
        s.spawn((
            Node {
                height: look.val(head),
                padding: UiRect::new(look.val(ROW_PAD), look.val(ROW_PAD + 1.0), px(0.0), look.val(0.4)),
                border: UiRect::bottom(px(hair)),
                flex_shrink: 0.0,
                align_items: AlignItems::FlexEnd,
                column_gap: look.val(COL_GAP),
                ..default()
            },
            BorderColor::all(look.tone(Tone::Fg4)),
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
            th(h, 1, "", false, "avatar");
            th(h, 2, "Player", false, "player");
            th(h, 3, "Rung", true, "rung");
            h.spawn((right(4), Pickable::IGNORE)).with_children(|c| {
                let e = signal::mark(c, &look, Mark::Kill, look.px(0.66), look.tone(Tone::Fg3));
                parity::name(c, e, "end.std.th.k");
            });
        });
    });
    let parts = ui.scroll(&mut commands, panel, STANDINGS_SCROLL, UiStyle { h: Some(lay.std_h - head), shrink: Some(0.0), ..UiStyle::row() }, UiStyle::column());
    let top_wins = view.rows.iter().map(|r| r.wins).max().unwrap_or(0);
    // The winner first, then the rest in order.
    let order: Vec<usize> = (0..view.rows.len()).filter(|i| view.rows[*i].winner).chain((0..view.rows.len()).filter(|i| !view.rows[*i].winner)).collect();
    for (n, i) in order.into_iter().enumerate() {
        let row = &view.rows[i];
        let p = format!("end.std.row.{}", i + 1);
        let mut r = commands.spawn((
            StandingsRow { index: i, you: row.local },
            // Its hover and pin are the selection's fills (`decorate_rows`).
            chimera_ui::controls::own_states(),
            Node {
                height: look.val(lay.row_h),
                border: UiRect::bottom(px(hair)),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                column_gap: look.val(COL_GAP),
                padding: UiRect::new(look.val(ROW_PAD), look.val(ROW_PAD), px(0.0), px(0.0)),
                ..default()
            },
            BorderColor::all(look.tone(Tone::Fg5)),
            BackgroundColor(Color::NONE),
            enter(&look, came_up, 0.5 + n.min(12) as f32 * 0.04, 0.0, 1.6 + n.min(12) as f32 * 0.3),
            UiParityId(p.clone()),
            ChildOf(parts.content),
        ));
        if row.local {
            r.insert(signal::you_row(&look));
        }
        if row.winner {
            r.insert((WinnerRow, Outline::new(px(look.px(0.12)), px(-look.px(0.12)), look.theme.tone_a(Tone::Acc, 0.6))));
        }
        let r = r.id();
        row_cells(&mut commands, r, &look, row, i, &view, &p, top_wins, slants.as_deref_mut(), &cell, &right, &pointer);
        // The avatar, in its cell.
        let byte = avatar_byte(lobby.as_deref(), mine.as_deref(), &row.name, row.local);
        let holder = commands.spawn((cell(1), Pickable::IGNORE, AvatarSlot { face: face_of(&row.name, row.bot, byte), you: row.local, size: 1.5 })).id();
        commands.entity(r).insert_children(1, &[holder]);
    }
    // The kit's "more below" mark over the foot, its words "+N more"
    // (`more_below` counts them).
    let more = ui.scroll_more(&mut commands, &parts, "", 3.0);
    commands.entity(more).insert((MoreBelow::default(), UiParityId::new("end.std.more")));
    // Leave, top left: the kit's secondary button with the arrow, in a
    // place that fades in.
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

/// One standings row's cells (the avatar's cell goes in second) and its
/// pointer.
#[allow(clippy::too_many_arguments)]
fn row_cells(
    commands: &mut Commands,
    r: Entity,
    look: &Look,
    row: &MatchEndRow,
    i: usize,
    view: &MatchEndView,
    p: &str,
    top_wins: u32,
    mut slants: Option<&mut Assets<SlantMaterial>>,
    cell: &dyn Fn(usize) -> Node,
    right: &dyn Fn(usize) -> Node,
    pointer: &Handle<Image>,
) {
    let fg = if row.winner { Tone::AccText } else { Tone::Fg3 };
    commands.entity(r).with_children(|r| {
        r.spawn((cell(0), Pickable::IGNORE)).with_children(|c| {
            if row.winner {
                let e = signal::mark(c, look, Mark::Trophy, look.px(0.9), look.tone(Tone::AccText));
                parity::name(c, e, format!("{p}.trophy"));
            } else {
                c.spawn((look.tracked(Role::Mono, 0.6, 0.0, look.tone(fg), &format!("{:02}", row.place)), UiParityId(format!("{p}.pos"))));
            }
        });
        r.spawn((Node { column_gap: look.val(0.5), ..cell(2) }, Pickable::IGNORE)).with_children(|c| {
            if row.bot {
                let e = signal::bot_slot(c, look, true, 0.8, look.tone(Tone::Fg2));
                parity::name(c, e, format!("{p}.bot"));
            }
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
                        if row.winner {
                            w.spawn((look.text(Role::Mono, 0.56, look.tone(Tone::AccText), "+1"), Pickable::IGNORE));
                        }
                    });
            }
        });
        r.spawn((right(3), Pickable::IGNORE)).with_children(|c| {
            c.spawn((look.tracked(Role::DisplayBold, 0.9, 0.0, look.tone(if row.winner { Tone::AccText } else { Tone::Fg }), &rung_of(row, view.rungs)), UiParityId(format!("{p}.rung"))));
        });
        r.spawn((right(4), Pickable::IGNORE)).with_children(|c| {
            c.spawn((look.tracked(Role::Mono, 0.72, 0.0, look.tone(Tone::Fg), &row.kills.to_string()), UiParityId(format!("{p}.k"))));
        });
        // The selection's pointer (hover is the row's fill alone).
        r.spawn((
            RowPointer,
            ImageNode { image: pointer.clone(), color: look.tone(Tone::Fg), ..default() },
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(look.px(EndLayout::of(look).row_h / 2.0) - look.px(0.25)),
                width: look.val(0.25),
                height: look.val(0.5),
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
        ));
    });
    let _ = i;
}

/// The climb's bars and the tiles, of the selected player.
#[allow(clippy::too_many_arguments)]
fn build_climb(
    look: Look,
    view: Res<MatchEndView>,
    sel: Res<EndSelection>,
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
    // A new choice comes in at once (no entrance).
    let chose = state.climb_key.as_ref().is_some_and(|(i, v, u)| *i != shown && *v == key.1 && *u == key.2);
    state.climb_key = Some(key);
    for e in &old {
        commands.entity(e).despawn();
    }
    if !view.shown || view.rows.is_empty() {
        return;
    }
    let came_up = (if chose { 0.0 } else { state.came_up.unwrap_or(0.0) }, state.now);
    let row = &view.rows[shown];
    let lay = EndLayout::of(&look);
    let hair = look.hair();
    let times = rung_times(row, view.length, view.rungs);
    let n = times.secs.len();
    let max = times.secs.iter().flatten().copied().fold(1.0f32, f32::max);
    let gap = if n > 24 { 0.12 } else { 0.3 };
    let col_w = (lay.mid_w - gap * (n as f32 - 1.0)) / n as f32;
    // Labels only where they fit (the marked ones always).
    let labels = col_w >= 1.9;
    let bars_h = lay.climb_h - EndLayout::HEAD_H - EndLayout::KEY_H;
    let num_h = 1.2;
    let plot_h = bars_h - num_h - 1.1;
    commands.spawn((ClimbPart, ChildOf(*root), Pickable::IGNORE, frame_node(&look))).with_children(|f| {
        f.spawn((
            Node { height: look.val(lay.climb_h), ..abs(&look, lay.mid_x, lay.climb_top, Some(lay.mid_w), None) },
            enter(&look, came_up, 0.9, 0.6, 0.0),
            UiParityId::new("end.climb"),
            Pickable::IGNORE,
        ))
        .with_children(|c| {
            // The head: the climb, whose.
            c.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.8), height: look.val(EndLayout::HEAD_H), flex_shrink: 0.0, ..default() }, Pickable::IGNORE)).with_children(|h| {
                h.spawn((look.tracked(Role::DisplayBold, 0.7, 0.07, look.tone(Tone::Fg), "THE CLIMB"), Pickable::IGNORE));
                let who = format!("Time per rung \u{b7} {}{}", row.name, if row.local { " (you)" } else { "" });
                h.spawn((look.tracked(Role::Mono, 0.54, 0.054, look.tone(Tone::Fg3), &who.to_uppercase()), Ellipsize::max(who.to_uppercase(), lay.mid_w - 8.0), UiParityId::new("end.climb.name")));
            });
            c.spawn((RungBars { index: shown }, Node { height: look.val(bars_h), column_gap: look.val(gap), flex_shrink: 0.0, ..default() }, UiParityId::new("end.climb.chart"), Pickable::IGNORE)).with_children(|b| {
                for i in 0..n {
                    let kind = times.kind(i);
                    let t = times.secs[i];
                    b.spawn((Node { width: look.val(col_w), flex_shrink: 0.0, flex_direction: FlexDirection::Column, ..default() }, Pickable::IGNORE)).with_children(|col| {
                        // The plot: the time over the bar, the bar from the foot.
                        col.spawn((
                            Node { height: look.val(plot_h + 1.1), flex_direction: FlexDirection::Column, justify_content: JustifyContent::FlexEnd, align_items: AlignItems::Stretch, ..default() },
                            BackgroundColor(if kind == BarKind::Unreached { Color::NONE } else { look.theme.tone_a(Tone::Fg, 0.04) }),
                            Pickable::IGNORE,
                        ))
                        .with_children(|plot| {
                            let marked = matches!(kind, BarKind::Fastest | BarKind::Stall);
                            if let Some(t) = t.filter(|_| labels || marked) {
                                let tone = match kind {
                                    BarKind::Fastest => Tone::AccText,
                                    BarKind::Stall => Tone::Warn,
                                    _ => Tone::Fg2,
                                };
                                plot.spawn((Node { justify_content: JustifyContent::Center, height: look.val(1.1), align_items: AlignItems::Center, overflow: Overflow::clip(), ..default() }, Pickable::IGNORE)).with_children(|l| {
                                    l.spawn((look.tracked(Role::Mono, 0.5, 0.0, look.tone(tone), &mmss(t)), Pickable::IGNORE));
                                });
                            }
                            let h = t.map_or(plot_h, |t| (t / max * plot_h).max(0.25));
                            let (bg, outline) = match kind {
                                BarKind::Fastest => (look.tone(Tone::Acc), None),
                                BarKind::Stall => (look.theme.tone_a(Tone::Warn, 0.85), None),
                                BarKind::Open => (Color::NONE, Some(look.tone(Tone::AccText))),
                                BarKind::Unreached => (Color::NONE, Some(look.theme.tone_a(Tone::Fg, 0.12))),
                                BarKind::Plain => (look.theme.tone_a(Tone::Fg, if row.local { 0.32 } else { 0.24 }), None),
                            };
                            let mut bar = plot.spawn((RungBar { rung: i + 1, kind }, Node { height: look.val(h), flex_shrink: 0.0, ..default() }, BackgroundColor(bg), Pickable::IGNORE));
                            if let Some(o) = outline {
                                bar.insert(Outline::new(px(hair), px(-hair), o));
                            }
                        });
                        // The rung's number.
                        let tone = if Some(i) == times.open || (row.winner && i + 1 == n) { Tone::Fg } else { Tone::Fg3 };
                        col.spawn((Node { height: look.val(num_h), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, Pickable::IGNORE)).with_children(|l| {
                            if labels || (i + 1) % 5 == 0 || i == 0 || i + 1 == n {
                                l.spawn((look.tracked(Role::Mono, 0.5, 0.0, look.tone(tone), &format!("{}", i + 1)), Pickable::IGNORE));
                            }
                        });
                    });
                }
            });
            // The key.
            c.spawn((Node { height: look.val(EndLayout::KEY_H), align_items: AlignItems::Center, column_gap: look.val(1.6), flex_shrink: 0.0, ..default() }, UiParityId::new("end.climb.key"), Pickable::IGNORE)).with_children(|k| {
                let item = |k: &mut ChildSpawnerCommands, bg: Color, outline: Option<Color>, text: &str| {
                    k.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.45), ..default() }, Pickable::IGNORE)).with_children(|i| {
                        let mut sw = i.spawn((Node { width: look.val(0.9), height: look.val(0.6), ..default() }, BackgroundColor(bg), Pickable::IGNORE));
                        if let Some(o) = outline {
                            sw.insert(Outline::new(px(hair), px(-hair), o));
                        }
                        i.spawn((look.tracked(Role::Mono, 0.46, 0.06, look.tone(Tone::Fg3), &text.to_uppercase()), Pickable::IGNORE));
                    });
                };
                item(k, look.tone(Tone::Acc), None, "Fastest rung");
                item(k, look.theme.tone_a(Tone::Warn, 0.85), None, "Longest stall");
                item(k, Color::NONE, Some(look.tone(Tone::AccText)), "Reached, not finished");
            });
        });

        // The tiles, the same player's.
        f.spawn((
            StatsRow,
            Node { flex_direction: FlexDirection::Row, column_gap: look.val(0.5), height: look.val(lay.stats_h), ..abs(&look, lay.mid_x, lay.stats_top, Some(lay.mid_w), None) },
            enter(&look, came_up, 1.1, 0.6, 0.0),
            UiParityId::new("end.stats"),
            Pickable::IGNORE,
        ))
        .with_children(|t| {
            for (i, stat) in stats_of(row, view.length, view.rungs).into_iter().enumerate() {
                t.spawn((
                    StatTile { label: stat.label },
                    Node {
                        flex_grow: 1.0,
                        flex_basis: px(0.0),
                        min_width: px(0.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        row_gap: look.val(0.35),
                        padding: UiRect::horizontal(look.val(0.6)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(look.theme.tone_a(Tone::Fg, 0.04)),
                    Outline::new(px(hair), px(0.0), look.theme.tone_a(Tone::Fg, 0.16)),
                    UiParityId(format!("end.stats.{}", i + 1)),
                    Pickable::IGNORE,
                ))
                .with_children(|c| {
                    c.spawn((look.tracked(Role::Mono, 0.44, 0.044, look.tone(Tone::Fg3), &stat.label.to_uppercase()), Ellipsize::in_parent(stat.label.to_uppercase()), Pickable::IGNORE));
                    c.spawn((look.tracked(Role::DisplayBold, 1.4, 0.0, look.tone(Tone::Fg), &stat.value), Ellipsize::in_parent(stat.value.clone()), Pickable::IGNORE));
                });
            }
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

/// The standings' "+N more": how many rows are under the view's foot
/// (the kit's mark shows and hides itself, `scroll::show_more`).
fn more_below(
    look: Look,
    views: Option<Res<chimera_ui::ScrollViews>>,
    mut more: Query<(Entity, &mut MoreBelow)>,
    children: Query<&Children>,
    mut texts: Query<&mut Text>,
) {
    let row_h = EndLayout::of(&look).row_h;
    let below = views.and_then(|v| v.get(STANDINGS_SCROLL).cloned()).map_or(0, |s| {
        if !chimera_ui::scroll::more_below(&s) {
            return 0;
        }
        ((s.content - s.view - s.inside()) / row_h).max(0.0).ceil() as usize
    });
    for (e, mut m) in &mut more {
        if m.n == below && below > 0 {
            continue;
        }
        m.n = below;
        for c in children.iter_descendants(e) {
            if let Ok(mut t) = texts.get_mut(c) {
                let words = format!("+{below} MORE");
                if t.0 != words {
                    t.0 = words;
                }
            }
        }
    }
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

/// Stages the screen (`ui_scene matchend [fixture] [moment]`), or clears
/// it. With `moment` the winner moment plays first (its card now, the
/// screen as it moves into place).
pub fn stage(world: &mut World, fixture: Option<&str>) -> Result<(), String> {
    stage_with(world, fixture, false)
}

/// `stage`, the winner moment first when `moment`.
pub fn stage_with(world: &mut World, fixture: Option<&str>, moment: bool) -> Result<(), String> {
    let f = match fixture {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("ui_scene: {path}: {e}"))?;
            serde_json::from_str::<EndFixture>(&text).map_err(|e| format!("ui_scene: {path}: {e}"))?
        }
        None => staged_fixture(),
    };
    let view = f.view();
    if moment {
        winner_moment::stage(world, &view);
    }
    world.insert_resource(MatchEndStaged { view, left: f.left });
    Ok(())
}

/// Clears a staged screen.
pub fn unstage(world: &mut World) {
    world.remove_resource::<MatchEndStaged>();
    if let Some(mut v) = world.get_resource_mut::<MatchEndView>() {
        *v = MatchEndView { sounds_asked: v.sounds_asked, ..default() };
    }
    if let Some(mut m) = world.get_resource_mut::<WinnerMoment>() {
        *m = WinnerMoment::default();
    }
}

// ===== the vote =====

/// The vote's part.
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

/// `map` to a pair of colours, the cards' stand-in for a thumbnail.
fn map_tint(map: &str) -> (Color, Color) {
    let h = map.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let hue = (h % 360) as f32;
    (Color::hsl(hue, 0.20, 0.26), Color::hsl(220.0, 0.30, 0.09))
}

/// The pips a card shows (one a vote, the look book's eight).
pub const PIPS: usize = 8;

/// The gap between the map cards, cq.
const CARD_GAP: f32 = 0.9;
/// A card's height under its picture, cq: fixed, so voting (or hovering)
/// changes only colours, the outline and the badge, never the box.
const CARD_BODY: f32 = 2.6;
/// The vote's head line, cq.
const VOTE_HEAD: f32 = 1.5;

/// The vote (rebuilt when the vote changes): its head, the map cards, the
/// countdown.
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
    let came_up = (state.came_up.unwrap_or(0.0), state.now);
    let hair = look.hair();
    let tan = look.theme.tan();
    let lay = EndLayout::of(&look);
    let ui = chimera_ui::controls::Ui { theme: &look.theme, kit: look.fonts.as_deref(), units: *look.units };
    let n = 40;
    // Two columns; the rows share the area, so the cards keep one size
    // whatever the votes (more than four maps: more rows, smaller pictures).
    let maps_n = view.vote.as_ref().map_or(0, |v| v.maps.len());
    let rows = maps_n.div_ceil(2).max(2) as f32;
    let card_w = (lay.mid_w - CARD_GAP - 0.2) / 2.0;
    let card_h = ((lay.vote_h - VOTE_HEAD - CARD_GAP * (rows - 1.0)) / rows).max(CARD_BODY + 2.0);
    let thumb_h = card_h - CARD_BODY;
    let part = commands.spawn((VotePart, ChildOf(*root), Pickable::IGNORE, frame_node(&look))).id();
    let band = commands
        .spawn((
            Node { height: look.val(lay.vote_h), ..abs(&look, lay.mid_x, lay.vote_top, Some(lay.mid_w), None) },
            enter(&look, came_up, 0.35, 0.8, 0.0),
            UiParityId::new("end.vote"),
            Pickable::IGNORE,
            ChildOf(part),
        ))
        .id();
    commands.entity(band).with_children(|b| {
        b.spawn((Node { height: look.val(VOTE_HEAD), align_items: AlignItems::FlexStart, column_gap: look.val(0.8), flex_shrink: 0.0, ..default() }, Pickable::IGNORE)).with_children(|h| {
            h.spawn((look.tracked(Role::DisplayBold, 0.7, 0.07, look.tone(Tone::Fg), "NEXT MAP"), Pickable::IGNORE));
            let n = view.vote.as_ref().map_or(0, |v| v.maps.len());
            let hint = if n > 0 { format!("Vote \u{b7} keys 1-{} or click", n.min(9)) } else { String::new() };
            h.spawn((look.tracked(Role::Mono, 0.54, 0.054, look.tone(Tone::Fg3), &hint.to_uppercase()), UiParityId::new("end.vote.head")));
        });
    });
    let grid = commands
        .spawn((
            Node { flex_grow: 1.0, flex_wrap: FlexWrap::Wrap, column_gap: look.val(CARD_GAP), row_gap: look.val(CARD_GAP), align_items: AlignItems::FlexStart, align_content: AlignContent::FlexStart, ..default() },
            Pickable::IGNORE,
            ChildOf(band),
        ))
        .id();
    if let Some(v) = &view.vote {
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
            let card = commands
                .spawn((
                    VoteCard { index: i, mine, locked, won },
                    super::match_end::VoteButton(i),
                    // The kit's item: its fill and hover.
                    ui.item_bundle(Some((Tone::Ink, 0.75)), chimera_ui::controls::State { disabled: locked, ..chimera_ui::controls::State::NONE }, false),
                    Node { width: look.val(card_w), height: look.val(card_h), flex_shrink: 0.0, flex_direction: FlexDirection::Column, overflow: Overflow::clip(), ..default() },
                    Outline::new(px(w), px(-w), line),
                    UiParityId(p.clone()),
                    ChildOf(grid),
                ))
                .id();
            // Locked, the others dim to 35 %.
            let dim = if locked && !won { 0.35 } else { 1.0 };
            let tone = |t: Tone| look.theme.tone_a(t, dim);
            let (a, b) = map_tint(map);
            let (a, b) = (a.with_alpha(dim), b.with_alpha(dim));
            let thumb = commands
                .spawn((
                    Node { height: look.val(thumb_h), flex_shrink: 0.0, flex_direction: FlexDirection::Column, justify_content: JustifyContent::FlexEnd, padding: UiRect::new(look.val(0.8), look.val(0.8), px(0.0), look.val(0.6)), ..default() },
                    BackgroundGradient::from(LinearGradient::degrees(110.0, vec![ColorStop::percent(a, 0.0), ColorStop::percent(b, 100.0)])),
                    UiParityId(format!("{p}.thumb")),
                    Pickable::IGNORE,
                    ChildOf(card),
                ))
                .id();
            commands.entity(thumb).with_children(|t| {
                t.spawn((
                    Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() },
                    BackgroundGradient::from(LinearGradient::to_bottom(vec![ColorStop::percent(Color::NONE, 30.0), ColorStop::percent(look.theme.tone_a(Tone::Ink, 0.8), 100.0)])),
                    Pickable::IGNORE,
                ));
                // The map's name and game, once, over the picture's foot.
                t.spawn((Node { flex_direction: FlexDirection::Column, row_gap: look.val(0.25), min_width: px(0.0), ..default() }, Pickable::IGNORE)).with_children(|m| {
                    m.spawn((
                        look.tracked(Role::DisplayBold, 0.86, 0.86 * 0.03, tone(Tone::Fg), &map.to_uppercase()),
                        Ellipsize::in_parent(map.to_uppercase()),
                        UiParityId(format!("{p}.name")),
                    ));
                    let game = v.games.get(i).cloned().unwrap_or_default();
                    m.spawn((look.tracked(Role::Mono, 0.48, 0.048, look.theme.tone_a(Tone::Fg, 0.6 * dim), &game.to_uppercase()), UiParityId(format!("{p}.game"))));
                });
                if mine {
                    t.spawn((Node { position_type: PositionType::Absolute, right: look.val(0.5), top: look.val(0.5), ..default() }, Pickable::IGNORE)).with_children(|t| {
                        signal::tag(t, &look, TagKind::Accent, "Your vote", 0.62);
                    });
                }
            });
            // The number key's cap, top left.
            if i < 9 {
                let at = commands.spawn((Node { position_type: PositionType::Absolute, left: look.val(0.5), top: look.val(0.5), ..default() }, BackgroundColor(look.theme.tone_a(Tone::Ink, 0.75)), Pickable::IGNORE, ChildOf(thumb))).id();
                ui.kbd(&mut commands, at, &(i + 1).to_string(), 0.66, Tone::Fg);
            }
            // The body: the pips and the count.
            commands.entity(card).with_children(|c| {
                c.spawn((Node { height: look.val(CARD_BODY), align_items: AlignItems::Center, column_gap: look.val(0.5), padding: UiRect::horizontal(look.val(0.8)), flex_shrink: 0.0, ..default() }, Pickable::IGNORE)).with_children(|r| {
                    r.spawn((Node { column_gap: look.val(0.16), ..default() }, UiParityId(format!("{p}.pips")), Pickable::IGNORE)).with_children(|pips| {
                        for k in 0..PIPS {
                            let color = if k < count as usize {
                                tone(if mine && k + 1 == count as usize { Tone::Acc } else { Tone::Fg })
                            } else {
                                look.theme.tone_a(Tone::Fg, 0.15 * dim)
                            };
                            // The box holds the lean; the layout counts .5 cq (as CSS's skew).
                            let mut pip = pips.spawn((Node { width: look.val(0.5 + 0.5 * tan), height: look.val(0.5), margin: UiRect::right(look.val(-0.5 * tan)), ..default() }, Pickable::IGNORE));
                            if let Some(slants) = slants.as_deref_mut() {
                                pip.insert(MaterialNode(slants.add(SlantMaterial::new(&look.theme, color))));
                            }
                        }
                    });
                    r.spawn((Node { flex_grow: 1.0, ..default() }, Pickable::IGNORE));
                    let t = if lead == Some(i) { Tone::AccText } else { Tone::Fg };
                    r.spawn((look.tracked(Role::DisplayBold, 0.9, 0.0, tone(t), &count.to_string()), UiParityId(format!("{p}.count"))));
                });
            });
        }
    }
    // The countdown, bottom right: "Next match", the number big, the
    // ticks across the area.
    let u = *look.units;
    let w = lay.std_w;
    let bar_h = (lay.foot_h * 0.16).clamp(0.9, 2.4);
    let num = (lay.foot_h - bar_h - 3.0).clamp(2.2, 7.0) / 1.05;
    commands.entity(part).with_children(|f| {
        f.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Stretch,
                row_gap: look.val(0.7),
                padding: UiRect::top(look.val(0.9)),
                border: UiRect::top(px(hair)),
                height: look.val(lay.foot_h),
                ..abs(&look, lay.std_x, lay.foot_top, Some(w), None)
            },
            BorderColor::all(look.tone(Tone::Fg4)),
            VoteCountdownColumn,
            enter(&look, came_up, 0.35, 0.8, 0.0),
            Pickable::IGNORE,
        ))
        .with_children(|c| {
            c.spawn((look.tracked(Role::Mono, 0.62, 0.062, look.tone(Tone::Fg3), "NEXT MATCH"), UiParityId::new("end.vote.tag")));
            c.spawn((Node { align_items: AlignItems::FlexEnd, justify_content: JustifyContent::SpaceBetween, ..default() }, Pickable::IGNORE)).with_children(|r| {
                r.spawn((look.tracked(Role::DisplayBold, num, num * 0.02, look.tone(Tone::Fg), "0:00"), VoteCountdown, UiParityId::new("end.vote.count")));
                signal::icons::spawn(r, &look, "arrow_right", look.px(num * 0.6), look.tone(Tone::AccText));
            });
            let layout = TickLayout::new(u.phys(u.cq(w)).floor() as u32, u.phys(u.cq(bar_h)).floor() as u32, n);
            let sf = u.scale_factor.max(1e-3);
            c.spawn((Node { height: look.val(bar_h), flex_shrink: 0.0, ..default() }, Pickable::IGNORE)).with_children(|w| {
                let mut bar = w.spawn((Node { width: px(layout.w as f32 / sf), height: px(layout.h as f32 / sf), ..default() }, UiParityId::new("end.vote.ticks"), Pickable::IGNORE));
                if let Some(ticks) = ticks.as_deref_mut() {
                    let material = ticks.add(TickBarMaterial::new(&look.theme, n, layout));
                    bar.insert((MaterialNode(material.clone()), VoteTicks { material, ticks: n }));
                }
            });
        });
    });
}

/// The bottom right: "Next match", the countdown and its ticks.
#[derive(Component)]
pub struct VoteCountdownColumn;

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

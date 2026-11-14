//! The match-end screen in the Signal look (docs/plans/active/ugg-ui.md
//! 3 and M4), drawn instead of the shell's (`match_end`) when the app's
//! theme turns the Signal HUD on (`signal::signal_on`: the Ultimate Gun
//! Game; Lucker Party and the plain apps keep theirs). Same state
//! (`match_end::MatchEndView`, from the results and the vote), same gate
//! (the logo pass in and out), same vote input; this is presentation.
//!
//! On the ink, a 16:9 frame (100 x 56.25 cq, centred) with the look
//! book's numbers:
//!
//! - **Header**: the lockup, MATCH OVER, and the map, its game, the
//!   minigame and the players; a rule under it; `+` marks in the corners.
//! - **Winner** (`WinnerName`): the tag, the name big (3.6 cq) with an
//!   accent slab sweeping across it (`WinnerSlab`: from
//!   `motion.dur.matchend.slab_at` over `slab`, the name shown at
//!   `name_at`), and a line: all the rungs and the time, the final kill
//!   (the winner's gun and its last victim).
//! - **Standings** (`StandingsRow`): position, the bot slot and the name
//!   with HOST (or the fused YOU+HOST chip) and the session wins (trophy,
//!   the count with this match, "+1" on the winner's: the server's
//!   `ResultRow::wins`, `minigame::wins` per server run), the rung big (the winner's in the accent, with its
//!   bar), kills and deaths under their marks, the ping (meter and number;
//!   the loss mark for a connection losing packets). Your row is tinted
//!   and outlined; hover is a tick at the right edge and a faint fill
//!   (never an outline); the selected row (a click; the winner's at
//!   first) has a stronger fill and a pointer outside the row.
//! - **The climb** (`ClimbChart`): rung against match time for the
//!   hovered row, else the selected one: the others' climbs faint, the
//!   chosen one's steps (`ClimbSeg`) drawn along their length in
//!   `climb_draw` s, its rung-ups, kills and deaths as ticks fading in
//!   along x over `climb_ticks` s (`ClimbTick`), its longest stall boxed
//!   and labelled, its final rung named.
//! - **The vote** (`VoteCard`, one band along the bottom): one countdown
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
//!
//! - **Leave** (`LeaveButton`): leaves for the menu (`disconnect`,
//!   through the logo pass); the next match starts without you.
//! - **The ground** (`EndGround`): the shared ambient with the match
//!   end's bloom and the broad slanted planes over it at .6 % a layer
//!   (look book V's "ambient + planes"), both on the renderer's clock.
//!
//! Each part is rebuilt only when what it shows changes; the entrance
//! (fades and steps from the screen's coming up, `EndState::came_up`),
//! the slab and the climb's drawing run on the kit's clock (`UiClock`).
//! The cursor is free while the screen is up (`match_end::vote_cursor`);
//! the games' HUDs are hidden under it (`hide_huds`) and shown again
//! after.
//! `ui_scene matchend [fixture]` stages it (`stage`). Nodes carry the
//! look book's names (`UiParityId`: `end.*`).
//!
//! Known shortcuts: no gun on the standings (the look book has none);
//! the stall's box is solid, not dashed; map cards show a tinted
//! gradient (no map shots); a listen host's Leave gets no confirmation
//! (the Esc menu's warns about the people connected).

use bevy::{prelude::*, window::PrimaryWindow};
use chimera_sim::minigame::climb::Climb;
use chimera_ui::{Prop, Role, SlantMaterial, Theme, TickBarMaterial, TickLayout, Timeline, Tone, Track, UiClock, UiParityId, parity};
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

/// A row's hover tick and its selection pointer.
#[derive(Component)]
struct RowTick;
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
    Stall,
    Final,
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

/// The games' HUDs (`hud::HudRoot`) hidden while the screen is up (it
/// is the whole screen), shown again after: those this hid.
fn hide_huds(view: Res<MatchEndView>, mut roots: Query<(Entity, &mut Visibility), With<super::hud::HudRoot>>, mut hidden: Local<Vec<Entity>>) {
    if view.shown {
        for (e, mut v) in &mut roots {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
                hidden.push(e);
            }
        }
    } else if !hidden.is_empty() {
        for e in hidden.drain(..) {
            if let Ok((_, mut v)) = roots.get_mut(e) {
                *v = Visibility::Inherited;
            }
        }
    }
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

/// The standings' columns: position, player, rung, K, D, ping; the gap.
const COLS: [f32; 6] = [1.8, 0.0, 3.6, 2.4, 2.4, 3.9];
const COL_GAP: f32 = 0.9;
const ROW_PAD: f32 = 0.8;
/// The look book's grid ends this much short of the row's padding on the
/// right (measured: its rung, K, D and ping .64 cq further left than the
/// columns' sum puts them).
const ROW_PAD_RIGHT: f32 = ROW_PAD + 0.64;
const LEFT: f32 = 4.6;

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
        for (x, y) in [(2.4, 2.4), (97.6, 2.4), (2.4, 53.85), (97.6, 53.85)] {
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
        // Header.
        f.spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: look.val(3.0), ..abs(&look, LEFT, 3.8, None, Some(LEFT)) },
            enter(&look, came_up, 0.2, 0.0, 0.0),
            UiParityId::new("end.hd"),
            Pickable::IGNORE,
        ))
        .with_children(|h| {
            lockup(h, &look, 1.05);
            h.spawn((look.tracked(Role::DisplayBold, 1.2, 1.2 * 0.07, look.tone(Tone::Fg), "Match over"), UiParityId::new("end.hd.title")));
            h.spawn((Node { flex_grow: 1.0, ..default() }, Pickable::IGNORE));
            let mut info = vec![view.map.clone()];
            if !view.map_game.is_empty() {
                info.push(view.map_game.clone());
            }
            info.push(view.title.clone());
            info.push(format!("{} players", view.rows.len()));
            let info: Vec<String> = info.into_iter().filter(|s| !s.is_empty()).collect();
            h.spawn((look.tracked(Role::Mono, 0.62, 0.62 * 0.06, look.tone(Tone::Fg3), &info.join(" \u{2022} ")), UiParityId::new("end.hd.info")));
        });
        f.spawn((
            Node { height: px(hair), ..abs(&look, LEFT, 7.6, None, Some(LEFT)) },
            BackgroundColor(look.tone(Tone::Fg4)),
            Timeline::new([Track::new(Prop::ClipRight, 1.0, 0.0).at(0.25).dur(0.6).ease(look.theme.ease("out"))]).started(came_up),
            UiParityId::new("end.rule"),
            Pickable::IGNORE,
        ));

        // The winner.
        let winner = view.rows.iter().find(|r| r.winner);
        f.spawn((abs(&look, LEFT, 9.6, Some(46.0), None), Pickable::IGNORE)).with_children(|w| {
            w.spawn((Node::default(), enter(&look, came_up, 0.4, 0.0, 0.0), Pickable::IGNORE)).with_children(|t| {
                let e = signal::tag(t, &look, TagKind::Accent, if winner.is_some() { "Winner" } else { "Draw" }, 0.62);
                parity::name(t, e, "end.win.tag");
            });
            let name = winner.map_or("Draw".to_string(), |r| r.name.clone());
            w.spawn((Node { margin: UiRect::top(look.val(0.7)), align_self: AlignSelf::FlexStart, ..default() }, Pickable::IGNORE)).with_children(|n| {
                n.spawn((
                    look.tracked(Role::DisplayBold, 3.6, 0.072, look.tone(Tone::Fg), &name.to_uppercase()),
                    WinnerName,
                    Timeline::new([Track::new(Prop::Opacity, 0.0, 1.0).at(look.dur("matchend", "name_at", 0.9))]).started(came_up),
                    UiParityId::new("end.win.name"),
                ));
                if let Some(slants) = slants.as_deref_mut() {
                    n.spawn((
                        WinnerSlab,
                        MaterialNode(slants.add(SlantMaterial::new(&look.theme, look.tone(Tone::Acc)))),
                        Node { position_type: PositionType::Absolute, left: look.val(-1.0), top: px(0.0), bottom: px(0.0), width: px(0.0), ..default() },
                        Pickable::IGNORE,
                    ));
                }
            });
            w.spawn((
                Node { margin: UiRect::top(look.val(0.9)), ..default() },
                look.tracked(Role::Mono, 0.62, 0.062, look.tone(Tone::Fg2), &winner_line(&view)),
                enter(&look, came_up, 1.0, 0.0, 0.0),
                UiParityId::new("end.win.sub"),
            ));
        });

        // The standings.
        f.spawn((abs(&look, 55.0, 9.6, None, Some(LEFT)), Pickable::IGNORE)).with_children(|s| {
            s.spawn((Node::default(), enter(&look, came_up, 0.6, 0.0, 0.0), Pickable::IGNORE)).with_children(|t| {
                let e = signal::tag(t, &look, TagKind::Accent, "Standings", 0.62);
                parity::name(t, e, "end.std.tag");
            });
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
                    margin: UiRect::top(look.val(1.1)),
                    padding: UiRect::new(look.val(ROW_PAD), look.val(ROW_PAD_RIGHT), px(0.0), look.val(0.55)),
                    border: UiRect::bottom(px(hair)),
                    ..row_node()
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
                for (i, m, id) in [(3, Mark::Kill, "end.std.th.k"), (4, Mark::Death, "end.std.th.d")] {
                    h.spawn((right(i), Pickable::IGNORE)).with_children(|c| {
                        let e = signal::mark(c, &look, m, look.px(0.66), look.tone(Tone::Fg3));
                        parity::name(c, e, id);
                    });
                }
                th(h, 5, "Ping", true, "ping");
            });
            let top_wins = view.rows.iter().map(|r| r.wins).max().unwrap_or(0);
            for (i, row) in view.rows.iter().enumerate() {
                let p = format!("end.std.row.{}", i + 1);
                let mut r = s.spawn((
                    StandingsRow { index: i, you: row.local },
                    super::widgets::clickable(),
                    Node {
                        height: look.val(look.layout("matchend", "row_h", 2.62)),
                        border: UiRect::bottom(px(hair)),
                        flex_shrink: 0.0,
                        ..row_node()
                    },
                    BorderColor::all(look.tone(Tone::Fg5)),
                    BackgroundColor(Color::NONE),
                    enter(&look, came_up, 0.5 + i as f32 * 0.055, 0.0, 2.4 + i as f32 * 0.62),
                    UiParityId(p.clone()),
                ));
                if row.local {
                    r.insert(signal::you_row(&look));
                }
                row_cells(&mut r, &look, row, i, &p, top_wins, slants.as_deref_mut(), &cell, &right, &pointer);
            }
        });
    });
}

/// One standings row's cells, its hover tick and its pointer.
#[allow(clippy::too_many_arguments)]
fn row_cells(
    r: &mut EntityCommands,
    look: &Look,
    row: &MatchEndRow,
    i: usize,
    p: &str,
    top_wins: u32,
    mut slants: Option<&mut Assets<SlantMaterial>>,
    cell: &dyn Fn(usize) -> Node,
    right: &dyn Fn(usize) -> Node,
    pointer: &Handle<Image>,
) {
    let row_h = look.layout("matchend", "row_h", 2.62);
    r.with_children(|r| {
        if row.winner {
            r.spawn((
                Node { position_type: PositionType::Absolute, left: px(0.0), top: px(-look.hair()), bottom: px(-look.hair()), width: look.val(0.26), ..default() },
                BackgroundColor(look.tone(Tone::Acc)),
                Pickable::IGNORE,
            ));
        }
        r.spawn((cell(0), Pickable::IGNORE)).with_children(|c| {
            c.spawn((look.tracked(Role::Mono, 0.6, 0.0, look.tone(Tone::Fg3), &format!("{:02}", i + 1)), UiParityId(format!("{p}.pos"))));
        });
        r.spawn((Node { column_gap: look.val(0.5), ..cell(1) }, Pickable::IGNORE)).with_children(|c| {
            let e = signal::bot_slot(c, look, row.bot, 0.8, look.tone(Tone::Fg2));
            parity::name(c, e, format!("{p}.bot"));
            c.spawn((
                Node { flex_shrink: 1.0, min_width: px(0.0), overflow: Overflow::clip(), ..default() },
                look.tracked(Role::DisplayBold, 0.78, 0.78 * 0.03, look.tone(Tone::Fg), &row.name.to_uppercase()),
                UiParityId(format!("{p}.name")),
                Pickable::IGNORE,
            ));
            let e = signal::host_tag(c, look, slants.as_deref_mut(), row.host, row.local, 0.62);
            parity::name(c, e, format!("{p}.host"));
            if row.wins > 0 {
                // The session's leader in the accent; this match's win as +1.
                let tone = if row.wins == top_wins { Tone::AccText } else { Tone::Fg3 };
                c.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.18), flex_shrink: 0.0, ..default() }, super::race_chip::SessionWins(row.wins), Pickable::IGNORE))
                    .with_children(|w| {
                        signal::mark(w, look, Mark::Trophy, look.px(0.68), look.tone(tone));
                        // Matches won, this one counted, and marked +1.
                        w.spawn((look.tracked(Role::Mono, 0.56, 0.02, look.tone(tone), &row.wins.to_string()), UiParityId(format!("{p}.wins")), Pickable::IGNORE));
                        if row.winner {
                            w.spawn((look.text(Role::Mono, 0.56, look.tone(Tone::AccText), "+1"), Pickable::IGNORE));
                        }
                    });
            }
        });
        r.spawn((right(2), Pickable::IGNORE)).with_children(|c| {
            let tone = if row.winner { Tone::AccText } else { Tone::Fg };
            let rung = row.rung.map_or_else(|| "-".to_string(), |r| format!("{r:02}"));
            c.spawn((look.tracked(Role::DisplayBold, 1.3, 0.0, look.tone(tone), &rung), UiParityId(format!("{p}.rung"))));
        });
        r.spawn((right(3), Pickable::IGNORE)).with_children(|c| {
            c.spawn((look.tracked(Role::Mono, 0.72, 0.0, look.tone(Tone::Fg), &row.kills.to_string()), UiParityId(format!("{p}.k"))));
        });
        r.spawn((right(4), Pickable::IGNORE)).with_children(|c| {
            c.spawn((look.tracked(Role::Mono, 0.72, 0.0, look.tone(Tone::Fg3), &row.deaths.to_string()), UiParityId(format!("{p}.d"))));
        });
        r.spawn((right(5), Pickable::IGNORE)).with_children(|c| {
            c.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.17), flex_shrink: 0.0, ..default() }, UiParityId(format!("{p}.ping")), Pickable::IGNORE))
                .with_children(|c| ping_cell(c, look, row, slants.as_deref_mut()));
        });
        // The hover tick (hidden until hovered) and the selection's pointer.
        if let Some(slants) = slants.as_deref_mut() {
            r.spawn((chimera_ui::widgets::hover_tick(slants, &look.theme, &look.units, row_h), RowTick)).insert(Node {
                position_type: PositionType::Absolute,
                right: look.val(0.45),
                top: look.val(row_h * 0.22),
                width: look.val(0.22 + row_h * 0.56 * look.theme.tan()),
                height: look.val(row_h * 0.56),
                ..default()
            });
        }
        r.spawn((
            RowPointer,
            ImageNode { image: pointer.clone(), color: look.tone(Tone::Fg), ..default() },
            Node {
                position_type: PositionType::Absolute,
                right: look.val(-0.9),
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

/// The ping: a bot's dash, the listen host's LOCAL, else the meter and
/// the number (by the theme's thresholds) and, losing packets, the mark.
fn ping_cell(c: &mut ChildSpawnerCommands, look: &Look, row: &MatchEndRow, slants: Option<&mut Assets<SlantMaterial>>) {
    let size = 0.56;
    if row.bot {
        c.spawn((Node { min_width: look.val(size * 0.6 * 2.2), ..default() }, look.text(Role::Mono, size, look.tone(Tone::Fg4), "\u{2014}"), Pickable::IGNORE))
            .insert(TextLayout::new(Justify::Right, LineBreak::NoWrap));
        return;
    }
    if row.host && row.ping == 0 {
        c.spawn((look.text(Role::Mono, size * 0.92, look.tone(Tone::Fg3), "LOCAL"), Pickable::IGNORE));
        return;
    }
    let level = super::race_chip::ping_level(&look.theme, row.ping, row.loss);
    if let Some(slants) = slants {
        let parent = c.target_entity();
        let cmd = c.commands_mut();
        let m = chimera_ui::widgets::spawn_ping_meter(cmd, slants, &look.theme, &look.units, size * 0.92, level);
        cmd.entity(m).insert((super::race_chip::PingMeter(level.lit as u8), Pickable::IGNORE));
        cmd.entity(parent).add_child(m);
    }
    let number = if level.lit == 4 { look.tone(Tone::Fg3) } else { level.color(&look.theme) };
    c.spawn((Node { min_width: look.val(size * 0.6 * 2.2), ..default() }, look.tracked(Role::Mono, size, size * 0.02, number, &row.ping.to_string()), Pickable::IGNORE))
        .insert(TextLayout::new(Justify::Right, LineBreak::NoWrap));
    // The standings show the loss mark only (the look book hides LAG here).
    if row.loss {
        super::race_chip::ping_flag(c, look, level, true, size);
    }
}

/// The lockup at `lk` cq: the mark, ULTIMATE spread over the wordmark.
fn lockup(h: &mut ChildSpawnerCommands, look: &Look, lk: f32) {
    h.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.55 * lk), ..default() }, UiParityId::new("end.hd.lockup"), Pickable::IGNORE))
        .with_children(|l| {
            signal::icons::spawn(l, look, "mark", look.px(2.05 * lk), look.tone(Tone::Fg));
            l.spawn((Node { flex_direction: FlexDirection::Column, row_gap: look.val(0.3 * lk), ..default() }, Pickable::IGNORE)).with_children(|w| {
                w.spawn((Node { justify_content: JustifyContent::SpaceBetween, ..default() }, Pickable::IGNORE)).with_children(|u| {
                    for ch in "ULTIMATE".chars() {
                        u.spawn((look.text(Role::DisplayRegular, 0.62 * lk, look.tone(Tone::Fg2), &ch.to_string()), Pickable::IGNORE));
                    }
                });
                w.spawn((look.text(Role::Wordmark, lk, look.tone(Tone::Fg), "GUN GAME"), Pickable::IGNORE));
            });
        });
}

/// The chart's geometry, in its units (0.1 cq; the look book's 470 x 174
/// viewBox): x from the match's start to `span` s, y by rung.
#[derive(Clone, Copy, Debug)]
pub struct ChartAxes {
    pub span: f32,
    pub rungs: u32,
}

impl ChartAxes {
    pub const X0: f32 = 22.0;
    pub const X1: f32 = 462.0;

    /// Ten minutes, or the match's length rounded up to two-minute marks.
    pub fn new(length: f32, rungs: u32) -> Self {
        let span = (length / 120.0).ceil().max(5.0) * 120.0;
        ChartAxes { span, rungs: rungs.max(2) }
    }

    pub fn x(&self, t: f32) -> f32 {
        Self::X0 + (Self::X1 - Self::X0) * (t / self.span).clamp(0.0, 1.0)
    }

    pub fn y(&self, rung: f32) -> f32 {
        134.0 - 124.0 * (rung - 1.0) / (self.rungs - 1) as f32
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
    let axes = ChartAxes::new(view.length, view.rungs.max(row.rung.unwrap_or(1)));
    let k = look.units.cq(0.1);
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
            t.spawn((look.tracked(Role::Mono, 0.64, 0.64 * 0.08, color, text), Pickable::IGNORE));
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
            f.spawn((abs(&look, LEFT, 18.6, Some(47.0), None), enter(&look, came_up, 1.1, 0.0, 0.0), UiParityId::new("end.climb"), Pickable::IGNORE)).with_children(|c| {
                // The head: the tag, whose climb, what it came to.
                c.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.8), ..default() }, Pickable::IGNORE)).with_children(|h| {
                    let e = signal::tag(h, &look, TagKind::Accent, "The climb", 0.62);
                    parity::name(h, e, "end.climb.tag");
                    h.spawn((look.tracked(Role::Mono, 0.56, 0.056, look.tone(Tone::Fg), &row.name.to_uppercase()), UiParityId::new("end.climb.name")));
                    let reach = match (winner, row.rung) {
                        (true, _) if view.rungs > 0 => format!("All {} rungs in {}", view.rungs, mmss(end)),
                        (_, Some(r)) => format!("Reached rung {r:02}"),
                        _ => String::new(),
                    };
                    let stats = [reach, format!("{} K", row.kills), format!("{} D", row.deaths)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" \u{2022} ");
                    h.spawn((look.tracked(Role::Mono, 0.56, 0.056, look.tone(Tone::Fg3), &stats), UiParityId::new("end.climb.stats")));
                });
                c.spawn((
                    ClimbChart { index: shown },
                    Node { width: look.val(47.0), height: look.val(17.4), margin: UiRect::top(look.val(0.6)), ..default() },
                    UiParityId::new("end.climb.chart"),
                    Pickable::IGNORE,
                ))
                .with_children(|g| {
                    // The grid: two-minute marks, the first, middle and top rungs, the axis.
                    let mut m = 0.0;
                    while m <= axes.span + 0.5 {
                        let x = axes.x(m);
                        line(g, Vec2::new(x, 6.0), Vec2::new(x, 160.0), hair, fg(0.07));
                        label(g, &format!("{}:00", (m / 60.0).round() as u32), x, 171.0, 0.5, look.tone(Tone::Fg3));
                        m += 120.0;
                    }
                    let top = axes.rungs;
                    let mut marks = vec![1, top.div_ceil(2), top];
                    marks.dedup();
                    for r in marks {
                        let y = axes.y(r as f32);
                        line(g, Vec2::new(ChartAxes::X0, y), Vec2::new(ChartAxes::X1, y), hair, fg(0.07));
                        label(g, &format!("{r:02}"), 16.0, y + 2.2, 1.0, look.tone(Tone::Fg3));
                    }
                    line(g, Vec2::new(ChartAxes::X0, 140.5), Vec2::new(ChartAxes::X1, 140.5), hair, look.tone(Tone::Fg4));
                    label(g, "K", 16.0, 149.2, 1.0, look.tone(Tone::Fg3));
                    label(g, "D", 16.0, 159.2, 1.0, look.tone(Tone::Fg3));
                    // The others, faint.
                    for (j, other) in view.rows.iter().enumerate() {
                        if j == shown {
                            continue;
                        }
                        for s in segments(&axes.points(&other.climb, end), hair) {
                            g.spawn((seg_node(&s, k, 1.0), BackgroundColor(fg(0.12)), Pickable::IGNORE));
                        }
                    }
                    // The longest stall, boxed.
                    let draw = look.dur("matchend", "climb_draw", 0.7);
                    let spread = look.dur("matchend", "climb_ticks", 0.52);
                    if let Some((a, b, r)) = row.climb.longest_stall(end) {
                        let (x0, x1, y) = (axes.x(a), axes.x(b), axes.y(r as f32));
                        g.spawn((
                            Node { position_type: PositionType::Absolute, left: px(x0 * k), top: px((y - 5.0) * k), width: px((x1 - x0) * k), height: px(10.0 * k), border: UiRect::all(px(look.hair())), ..default() },
                            BackgroundColor(fg(0.06)),
                            BorderColor::all(look.tone(Tone::Fg3)),
                            ClimbTick { kind: TickKind::Stall, delay: spread, dur: 0.26 },
                            Pickable::IGNORE,
                        ));
                        label(g, &format!("Stall {} \u{2022} R{r:02}", mmss(b - a)), (x0 + x1) / 2.0, y - 8.0, 0.5, look.tone(Tone::Fg3));
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
                        (&row.climb.kills, 144.0, look.tone(Tone::Fg2), TickKind::Kill, 1.0),
                        (&row.climb.deaths, 154.0, look.theme.tone_a(Tone::Threat, 0.75), TickKind::Death, 2.0),
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
                        t.spawn((look.tracked(Role::Mono, 0.64, 0.64 * 0.08, accent, &fin.to_uppercase()), UiParityId::new("end.climb.final")));
                    });
                    let _ = draw;
                });
            });
        });
}

/// Rows' fills, hover ticks and pointers from the selection.
#[allow(clippy::type_complexity)]
fn decorate_rows(
    look: Look,
    sel: Res<EndSelection>,
    mut rows: Query<(&StandingsRow, &mut BackgroundColor, &Children)>,
    mut marks: Query<(&mut Visibility, Has<RowTick>), Or<(With<RowTick>, With<RowPointer>)>>,
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
            if let Ok((mut v, tick)) = marks.get_mut(c) {
                let on = if tick { hovered } else { pinned };
                v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
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
    let k = look.units.cq(0.1);
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

/// The countdown's text (`00:13`).
pub fn countdown_text(left: f64) -> String {
    let s = left.ceil() as u32;
    format!("{:02}:{:02}", s / 60, s % 60)
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
    let n = 40;
    commands
        .spawn((VotePart, ChildOf(*root), Pickable::IGNORE, Node { width: look.val(100.0), height: look.val(56.25), position_type: PositionType::Absolute, align_self: AlignSelf::Center, ..default() }))
        .with_children(|f| {
            f.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: look.val(1.4),
                    padding: UiRect::top(look.val(1.3)),
                    border: UiRect::top(px(hair)),
                    align_items: AlignItems::FlexStart,
                    ..abs(&look, LEFT, 39.4, None, Some(LEFT))
                },
                BorderColor::all(look.tone(Tone::Fg4)),
                enter(&look, came_up, 1.5, 0.8, 0.0),
                UiParityId::new("end.vote"),
                Pickable::IGNORE,
            ))
            .with_children(|b| {
                // The countdown: one, the vote's close and the next match.
                b.spawn((Node { width: look.val(14.6), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexStart, flex_shrink: 0.0, ..default() }, Pickable::IGNORE)).with_children(|c| {
                    c.spawn((Node::default(), Pickable::IGNORE)).with_children(|t| {
                        let e = signal::tag(t, &look, TagKind::Accent, "Next match", 0.62);
                        parity::name(t, e, "end.vote.tag");
                    });
                    c.spawn((
                        Node { margin: UiRect::top(look.val(0.6)), ..default() },
                        look.tracked(Role::DisplayBold, 2.4, 2.4 * 0.02, look.tone(Tone::Fg), "00:00"),
                        VoteCountdown,
                        UiParityId::new("end.vote.count"),
                    ));
                    let u = *look.units;
                    let layout = TickLayout::new(u.phys(u.cq(14.6)).floor() as u32, u.phys(u.cq(1.1)).floor() as u32, n);
                    let sf = u.scale_factor.max(1e-3);
                    c.spawn((Node { margin: UiRect::top(look.val(0.9)), height: look.val(1.1), ..default() }, Pickable::IGNORE)).with_children(|w| {
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
                    c.spawn((
                        Node { margin: UiRect::top(look.val(0.55)), max_width: look.val(14.6), ..default() },
                        look.tracked(Role::Mono, 0.5, 0.05, look.tone(Tone::Fg3), &vote_hint(&view)),
                        UiParityId::new("end.vote.hint"),
                    ))
                    .insert((TextLayout::new(Justify::Left, LineBreak::WordBoundary), bevy::text::LineHeight::RelativeToFont(1.45)));
                });
                // The map cards.
                b.spawn((Node { flex_grow: 1.0, column_gap: look.val(0.9), align_items: AlignItems::FlexStart, ..default() }, Pickable::IGNORE)).with_children(|cards| {
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
                            super::widgets::clickable(),
                            Node { flex_grow: 1.0, flex_basis: px(0.0), flex_direction: FlexDirection::Column, overflow: Overflow::clip(), ..default() },
                            BackgroundColor(look.theme.tone_a(Tone::Ink, 0.75)),
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
                                Node { height: look.val(4.6), ..default() },
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
                                m.spawn((look.tracked(Role::DisplayBold, 0.78, 0.78 * 0.03, tone(Tone::Fg), &map.to_uppercase()), UiParityId(format!("{p}.name"))));
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
                // The way out.
                b.spawn((Node { width: look.val(9.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: look.val(0.6), align_self: AlignSelf::FlexEnd, ..default() }, LeaveColumn, Pickable::IGNORE)).with_children(|c| {
                    c.spawn((
                        Node { max_width: look.val(9.0), ..default() },
                        look.tracked(Role::Mono, 0.48, 0.048, look.tone(Tone::Fg3), "Not staying? Leave any time; the match starts without you."),
                        UiParityId::new("end.vote.ex"),
                    ))
                    .insert((TextLayout::new(Justify::Left, LineBreak::WordBoundary), bevy::text::LineHeight::RelativeToFont(1.45)));
                    // Leave: outlined, the label and a triangle.
                    c.spawn((
                        LeaveButton,
                        super::widgets::clickable(),
                        Node {
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            padding: UiRect::new(look.val(0.7), look.val(0.6), look.val(0.5), look.val(0.45)),
                            ..default()
                        },
                        BackgroundColor(look.theme.tone_a(Tone::Ink, 0.9)),
                        Outline::new(px(hair), px(-hair), look.tone(Tone::Fg3)),
                        UiParityId::new("end.vote.leave"),
                    ))
                    .with_children(|t| {
                        t.spawn((look.tracked(Role::Mono, 0.64, 0.064, look.tone(Tone::Fg), "Leave"), Pickable::IGNORE));
                        t.spawn((
                            Node { width: look.val(0.36), height: look.val(0.44), ..default() },
                            ImageNode { image: state.pointer.as_ref().map(|(_, h)| h.clone()).unwrap_or_default(), color: look.tone(Tone::Fg), flip_x: true, ..default() },
                            Pickable::IGNORE,
                        ));
                    });
                });
            });
        });
}

/// The column the way out sits in (the Leave button joins it).
#[derive(Component)]
pub struct LeaveColumn;

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

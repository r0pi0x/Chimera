//! The winner moment and the winner band (docs/plans/active/ugg-ui.md,
//! "Match end v2"; the user's review of 2026-11-22): when someone wins,
//! the game is frozen for everyone (the server's end freeze,
//! `minigame::runtime::hold_the_end`) and a motion-graphics card says who
//! won (their avatar, WINNER, the name, the winning rung and gun) near
//! full screen over the frozen world; it holds, then moves and scales
//! into its exact place at the top of the match end (`match_end_signal`)
//! while the world fades out behind it and the rest of the screen comes
//! in around it. One element that transforms (`WinnerBand`: the same
//! entity, its `UiTransform` animated), not two screens swapped.
//!
//! - **Start** (`detect`): the minigame HUD's `match_won`/`match_lost`
//!   callout (the win, as every client hears it), under the Signal theme.
//!   The winner's name, avatar (`match_end_signal::avatar_byte`), rung and
//!   gun come from the characters and the ladder; the results replace
//!   them when they arrive. `ui_scene matchend [fixture] moment` stages
//!   one (`stage`).
//! - **Timeline** (`MomentTimes`, `[motion.dur.matchend]`: `moment_in`,
//!   `moment_hold`, `moment_move`, `world_fade`, `moment_scrim`,
//!   `slab_loop`): the card scales in over the world under a scrim, holds,
//!   and moves at `move_at`: `moment_in + moment_hold` after the win, or
//!   when the results come (the match end shown) if later. The world's
//!   scrim fades to the ink over `world_fade` from then; the screen's
//!   other parts enter from `move_at` (`match_end_signal::EndState`).
//! - **The world off**: once faded, the match end covers the world
//!   (`staging::WorldCover` "match end": its cameras off, its sounds
//!   held), so a network client whose server changes level unloads the
//!   map at once behind it (`staging::UnloadAtCover`; a standalone game
//!   is the server and keeps its map).
//! - **Input is never blocked**: the card takes no clicks; Esc, Leave and
//!   the vote work throughout (the vote opens as the screen shows); any
//!   key or click skips the moment (`skip`: the card straight into place,
//!   the world gone, the rest in).
//! - **The band** (`band`): built at the card's size while big (every
//!   size `moment_scale` times the band's) and again at the band's own
//!   size once in place, so its text is sharp at both ends; its accent
//!   slab sweeps across the name every `slab_loop` s while it is a card
//!   and once as it lands; a ring pulses out at the start and the landing
//!   (`BandPulse`). In place it adds the session wins and the quick chat:
//!   GG, WP, Rematch? (`QuickChat`: posted to chat, `chat::say`), GG with
//!   a pop and a "GG" rising off it (`GgFloat`).
//!
//! Known shortcuts: the card's avatar is the lobby's last one by name
//! (none sent in play); the winning gun is the ladder's shown weapon for
//! the winner's rung.

use bevy::prelude::*;
use chimera_sim::minigame::{hud::MinigameHud, ladder::{Ladder, Rung}};
use chimera_ui::{Ellipsize, Prop, Role, SlantMaterial, Timeline, Tone, Track, UiClock};

use super::{
    match_end::{MatchEndFollow, MatchEndView},
    match_end_signal::{self, EndLayout, EndState, SignalEndRoot},
    signal::{self, Look, Mark, UiOpacity},
};

/// The moment's systems after the view's update; the screen's draw after.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MomentSet;

/// Who won, as the card shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WinnerInfo {
    pub name: String,
    pub you: bool,
    pub bot: bool,
    /// `AvatarLook`'s byte (`prematch::NO_AVATAR`: drawn from the name).
    pub avatar: u8,
    /// The winning gun's name.
    pub gun: String,
    /// The rung (from 1) and the ladder's rungs.
    pub rung: Option<(u32, u32)>,
}

impl WinnerInfo {
    /// The card's line before the results: `14/14 · Knife`.
    pub fn line(&self) -> String {
        let mut parts = Vec::new();
        if let Some((r, t)) = self.rung {
            parts.push(if t > 0 { format!("{r:02}/{t}") } else { format!("Rung {r:02}") });
        }
        if !self.gun.is_empty() {
            parts.push(self.gun.clone());
        }
        parts.join(" \u{b7} ")
    }
}

/// The moment (module doc): when it began (`UiClock`), who, when the
/// match end showed, whether a key skipped it.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct WinnerMoment {
    pub started: Option<f64>,
    pub info: WinnerInfo,
    pub shown_at: Option<f64>,
    pub skipped: bool,
    /// The last win callout seen (its id), so one win starts one moment.
    pub seen: Option<u32>,
}

/// The moment's lengths (s) and its scrim, from the theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MomentTimes {
    pub intro: f32,
    pub hold: f32,
    pub moves: f32,
    pub fade: f32,
    /// The ink's opacity over the frozen world while the card holds.
    pub scrim: f32,
    pub slab_loop: f32,
}

impl MomentTimes {
    pub fn of(look: &Look) -> Self {
        let d = |k: &str, or: f32| look.dur("matchend", k, or);
        MomentTimes {
            intro: d("moment_in", 0.45).max(0.01),
            hold: d("moment_hold", 2.0),
            moves: d("moment_move", 0.8).max(0.01),
            fade: d("world_fade", 0.6).max(0.01),
            scrim: d("moment_scrim", 0.6).clamp(0.0, 1.0),
            slab_loop: d("slab_loop", 1.6).max(0.3),
        }
    }
}

impl WinnerMoment {
    pub fn running(&self) -> bool {
        self.started.is_some()
    }

    /// When the card moves into place (None until the match end shows).
    pub fn move_at(&self, t: &MomentTimes) -> Option<f64> {
        let (s, v) = (self.started?, self.shown_at?);
        Some(if self.skipped { v } else { (s + (t.intro + t.hold) as f64).max(v) })
    }

    /// The move's progress at `now` (0 big, 1 in place; 1 without a moment).
    pub fn progress(&self, now: f64, t: &MomentTimes) -> f32 {
        if !self.running() {
            return 1.0;
        }
        if self.skipped && self.shown_at.is_some() {
            return 1.0;
        }
        self.move_at(t).map_or(0.0, |m| ((now - m) as f32 / t.moves).clamp(0.0, 1.0))
    }

    /// The card is in place.
    pub fn settled(&self, now: f64, t: &MomentTimes) -> bool {
        self.progress(now, t) >= 1.0
    }

    /// The ink's opacity over the world at `now`: the scrim coming in with
    /// the card, then to opaque over the fade from the move (1 without a
    /// moment: the screen is the whole window).
    pub fn scrim(&self, now: f64, t: &MomentTimes) -> f32 {
        let Some(s) = self.started else { return 1.0 };
        if self.skipped && self.shown_at.is_some() {
            return 1.0;
        }
        match self.move_at(t) {
            Some(m) if now >= m => t.scrim + (1.0 - t.scrim) * ((now - m) as f32 / t.fade).clamp(0.0, 1.0),
            _ => t.scrim * ((now - s) as f32 / t.intro).clamp(0.0, 1.0),
        }
    }
}

/// The moment's systems (from `match_end_signal::add`).
pub fn add(app: &mut App) {
    app.init_resource::<WinnerMoment>()
        .add_systems(Update, detect.before(MatchEndFollow).run_if(match_end_signal::themed))
        .add_systems(Update, (track, skip, cover, quick_chat, floats).chain().in_set(MomentSet).after(MatchEndFollow).run_if(match_end_signal::themed));
}

/// A staged moment (`ui_scene matchend ... moment`): `view`'s winner, now.
pub fn stage(world: &mut World, view: &MatchEndView) {
    let now = world.get_resource::<UiClock>().map_or(0.0, |c| c.now);
    let info = view
        .rows
        .iter()
        .find(|r| r.winner)
        .map(|w| WinnerInfo {
            name: w.name.clone(),
            you: w.local,
            bot: w.bot,
            avatar: chimera_net::prematch::NO_AVATAR,
            gun: w.gun.clone(),
            rung: w.rung.map(|r| (r, view.rungs)),
        })
        .unwrap_or_default();
    world.insert_resource(WinnerMoment { started: Some(now), info, ..default() });
}

/// The win starts the moment (`match_won`/`match_lost` callouts); the
/// next match (the screen gone after it showed) or results that never
/// came end it.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn detect(
    hud: Option<Res<MinigameHud>>,
    clock: Res<UiClock>,
    view: Res<MatchEndView>,
    staged: Option<Res<super::match_end::MatchEndStaged>>,
    mut moment: ResMut<WinnerMoment>,
    chars: Query<(Entity, Option<&crate::net::NetCharacter>, Option<&Name>, Has<crate::core::LocalPlayer>, Option<&Rung>, Has<crate::core::Bot>)>,
    (ladder, catalog, gun_names, settings): (
        Option<Res<Ladder>>,
        Option<Res<chimera_sim::weapon::data::WeaponCatalog>>,
        Option<Res<super::minigame_scoreboard::GunNames>>,
        Option<Res<crate::net::NetSettings>>,
    ),
    (lobby, mine): (Option<Res<super::prematch_lobby::LobbyView>>, Option<Res<super::profile::AvatarSetting>>),
) {
    let now = clock.now;
    if moment.running() {
        let over = (moment.shown_at.is_some() && !view.shown) || (moment.shown_at.is_none() && !view.shown && staged.is_none() && moment.started.is_some_and(|s| now - s > 20.0));
        if over {
            let seen = moment.seen;
            *moment = WinnerMoment { seen, ..default() };
        }
        return;
    }
    let Some(c) = hud.as_ref().and_then(|h| h.callout.as_ref()) else { return };
    if moment.seen == Some(c.id) {
        return;
    }
    moment.seen = Some(c.id);
    let winner = match c.callout.cue {
        Some("match_won") => chars.iter().find(|c| c.3).map(|c| c.0),
        Some("match_lost") => c.callout.lines.iter().find_map(|l| l.player),
        _ => return,
    };
    if view.shown {
        return;
    }
    let found = winner.and_then(|e| chars.get(e).ok());
    let info = match found {
        Some((e, net, name, local, rung, bot)) => {
            let name = super::shown_name(e, local, net, name, settings.as_deref());
            let total = ladder.as_ref().map_or(0, |l| l.rungs.len() as u32);
            let gun = rung
                .and_then(|r| ladder.as_ref()?.shown_weapon(r.0, catalog.as_deref()))
                .map(|g| super::minigame_scoreboard::gun_name(g, gun_names.as_deref(), catalog.as_deref()))
                .unwrap_or_default();
            WinnerInfo {
                avatar: match_end_signal::avatar_byte(lobby.as_deref(), mine.as_deref(), &name, local),
                name,
                you: local,
                bot,
                gun,
                rung: rung.map(|r| (r.0 as u32 + 1, total)),
            }
        }
        None => WinnerInfo::default(),
    };
    *moment = WinnerMoment { started: Some(now), info, seen: moment.seen, ..default() };
}

/// When the match end showed (the move waits for it).
fn track(view: Res<MatchEndView>, clock: Res<UiClock>, mut moment: ResMut<WinnerMoment>) {
    if moment.running() && view.shown && moment.shown_at.is_none() {
        moment.shown_at = Some(clock.now);
    }
}

/// Any key or click skips the moment (it still does what it does: a
/// number votes, Esc opens the menu).
fn skip(look: Look, clock: Res<UiClock>, keys: Option<Res<ButtonInput<KeyCode>>>, mouse: Option<Res<ButtonInput<MouseButton>>>, mut moment: ResMut<WinnerMoment>) {
    if !moment.running() || moment.skipped || moment.settled(clock.now, &MomentTimes::of(&look)) {
        return;
    }
    let pressed = keys.is_some_and(|k| k.get_just_pressed().next().is_some()) || mouse.is_some_and(|m| m.get_just_pressed().next().is_some());
    if pressed {
        moment.skipped = true;
    }
}

/// The world covered once the match end is all there is
/// (`staging::WorldCover`): cameras off, sounds held, a client's map left
/// unloaded under it when its server changes level.
fn cover(look: Look, view: Res<MatchEndView>, clock: Res<UiClock>, moment: Res<WinnerMoment>, cover: Option<ResMut<super::staging::WorldCover>>) {
    let Some(mut cover) = cover else { return };
    let up = view.shown && moment.scrim(clock.now, &MomentTimes::of(&look)) >= 0.999;
    super::staging::WorldCover::put(&mut cover, "match end", up);
}

/// The band's frame (over the screen's other parts).
#[derive(Component)]
pub struct BandPart;

/// The winner band (the moment's card); `big` while it is the card.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WinnerBand {
    pub big: bool,
}

/// The winner's name and the slab sweeping over it.
#[derive(Component)]
pub struct WinnerName;
#[derive(Component)]
pub struct WinnerSlab;

/// The ring pulsing out from the band (the moment's start, the landing).
#[derive(Component)]
pub struct BandPulse;

/// A quick chat button: what it posts.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuickChat(pub &'static str);

/// The quick chat's words, GG first (it gets the burst).
pub const QUICK_CHAT: [&str; 3] = ["GG", "WP", "Rematch?"];

/// "GG" rising off its button, gone at `until` (`UiClock`).
#[derive(Component)]
pub struct GgFloat {
    pub until: f64,
}

type BandKey = (WinnerInfo, String, u32, bool, (u32, u32), String);

/// Builds and moves the band (module doc).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn band(
    look: Look,
    view: Res<MatchEndView>,
    moment: Res<WinnerMoment>,
    state: Res<EndState>,
    clock: Res<UiClock>,
    root: Single<Entity, With<SignalEndRoot>>,
    parts: Query<Entity, With<BandPart>>,
    mut bands: Query<(Entity, &mut WinnerBand, &mut Node, &mut UiTransform, &mut UiOpacity), Without<WinnerSlab>>,
    name: Query<&ComputedNode, With<WinnerName>>,
    mut slab: Query<(&mut Node, &mut Visibility), (With<WinnerSlab>, Without<WinnerBand>, Without<BandPulse>)>,
    mut pulse: Query<(&mut UiTransform, &mut Outline), (With<BandPulse>, Without<WinnerBand>)>,
    mut slants: Option<ResMut<Assets<SlantMaterial>>>,
    mut built: Local<Option<BandKey>>,
    mut commands: Commands,
) {
    let active = view.shown || moment.running();
    if !active {
        for e in &parts {
            commands.entity(e).despawn();
        }
        *built = None;
        return;
    }
    let now = clock.now;
    let t = MomentTimes::of(&look);
    let lay = EndLayout::of(&look);
    let k = lay.moment_scale;
    let p = moment.progress(now, &t);
    let big = moment.running() && p < 1.0;
    // Who: the results' winner once they're in, else the moment's.
    let row = view.rows.iter().find(|r| r.winner).filter(|_| view.shown);
    let info = match row {
        Some(r) => WinnerInfo {
            name: r.name.clone(),
            you: r.local,
            bot: r.bot,
            avatar: if moment.info.name == r.name { moment.info.avatar } else { chimera_net::prematch::NO_AVATAR },
            gun: r.gun.clone(),
            rung: r.rung.map(|x| (x, view.rungs)),
        },
        None => moment.info.clone(),
    };
    let line = if row.is_some() { match_end_signal::band_line(&view) } else if view.shown { "Nobody wins the match".into() } else { info.line() };
    let wins = row.map_or(0, |r| r.wins);
    let key: BandKey = (info.clone(), line.clone(), wins, big, match_end_signal::units_key(&look), look.theme.name.clone());
    let band_rect = lay.band();
    let existing = bands.iter().next().map(|b| b.0);
    if built.as_ref() != Some(&key) || existing.is_none() || look.theme.is_changed() {
        *built = Some(key);
        for e in &parts {
            commands.entity(e).despawn();
        }
        let frame = commands.spawn((BandPart, ChildOf(*root), ZIndex(10), Pickable::IGNORE, match_end_signal::frame_node(&look))).id();
        let s = if big { k } else { 1.0 };
        let (w, h) = (band_rect.width() * s, band_rect.height() * s);
        let (x, y) = if big { (50.0 - w / 2.0, EndLayout::FRAME_H / 2.0 - h / 2.0) } else { (band_rect.min.x, band_rect.min.y) };
        let opacity = if big { 0.0 } else { 1.0 };
        let b = commands
            .spawn((
                WinnerBand { big },
                Node { height: look.val(h), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: look.val(1.1 * s), padding: UiRect::new(look.val(1.4 * s), look.val(1.2 * s), px(0.0), px(0.0)), overflow: Overflow::clip(), ..match_end_signal::abs(&look, x, y, Some(w), None) },
                BackgroundColor(look.theme.tone_a(Tone::Ink, 0.94)),
                Outline::new(px(look.hair()), px(0.0), look.theme.tone_a(Tone::Acc, 0.35)),
                UiTransform::default(),
                UiOpacity(opacity),
                chimera_ui::UiParityId::new("end.winner"),
                Pickable::IGNORE,
                ChildOf(frame),
            ))
            .id();
        build_band(&mut commands, b, &look, &info, &line, wins, s, big, slants.as_deref_mut());
        return;
    }
    // The pose.
    let Some((_, _, _, mut tf, mut op)) = existing.and_then(|e| bands.get_mut(e).ok()) else { return };
    let ease_out = look.theme.ease("out");
    let io = look.theme.ease("io");
    let (scale, shift, alpha) = if big {
        let e = ease_out.sample(moment.started.map_or(1.0, |s| ((now - s) as f32 / t.intro).clamp(0.0, 1.0)));
        let q = io.sample(p);
        let s0 = 0.86 + 0.14 * e;
        let scale = s0 + (1.0 / k - s0) * q;
        let (cf, cb) = (band_rect.center(), Vec2::new(50.0, EndLayout::FRAME_H / 2.0));
        (scale, (cf - cb) * q, e)
    } else {
        let a = if moment.running() { 1.0 } else { state.came_up.map_or(1.0, |c| ease_out.sample(((now - c) as f32 / 0.5).clamp(0.0, 1.0))) };
        (1.0, Vec2::ZERO, a)
    };
    let next = UiTransform { translation: Val2::new(Val::Px(look.units.cq(shift.x)), Val::Px(look.units.cq(shift.y))), scale: Vec2::splat(scale), ..UiTransform::IDENTITY };
    tf.set_if_neq(next);
    if (op.0 - alpha).abs() > 1e-3 {
        op.0 = alpha;
    }
    // The slab: across the name every loop while it is the card, once as it lands.
    let starts: Vec<f64> = match (moment.started, big) {
        (Some(s), true) => {
            let n = ((now - s - 0.35) / t.slab_loop as f64).floor().max(0.0);
            vec![s + 0.35 + n * t.slab_loop as f64]
        }
        (Some(_), false) => moment.move_at(&t).map(|m| vec![m + t.moves as f64 + 0.05]).unwrap_or_default(),
        (None, _) => state.came_up.map(|c| vec![c + look.dur("matchend", "slab_at", 0.55) as f64]).unwrap_or_default(),
    };
    let dur = look.dur("matchend", "slab", 0.9);
    let sweep = starts.iter().rev().find(|s| now >= **s).map(|s| ((now - s) as f32 / dur).clamp(0.0, 1.0));
    if let (Ok(name), Ok((mut node, mut vis))) = (name.single(), slab.single_mut()) {
        let w = name.size().x * name.inverse_scale_factor();
        let pr = sweep.unwrap_or(0.0);
        let kk = io.sample(pr);
        let pad = look.units.cq(1.0);
        let lean = name.size().y * name.inverse_scale_factor() * look.theme.tan();
        let (left, width) = if kk < 0.4 {
            (-pad, (w + 2.0 * pad) * kk / 0.4)
        } else {
            let q = (kk - 0.4) / 0.6;
            let left = -pad + (w + 2.0 * pad) * q;
            (left, (w + pad - left).max(0.0) * (1.0 - q))
        };
        let show = sweep.is_some_and(|p| p > 0.0 && p < 1.0) && width > 0.5;
        vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
        let (l, wd) = (px(left.round()), px((width + lean).round()));
        if node.left != l || node.width != wd {
            node.left = l;
            node.width = wd;
        }
    }
    // The ring: out from the band at the start and at the landing.
    let ring_at = match (moment.started, big) {
        (Some(s), true) => Some(s + 0.1),
        (Some(_), false) => moment.move_at(&t).map(|m| m + t.moves as f64),
        (None, _) => state.came_up,
    };
    for (mut rt, mut o) in &mut pulse {
        let r = ring_at.map_or(1.0, |a| ((now - a) as f32 / 0.7).clamp(0.0, 1.0));
        let e = ease_out.sample(r);
        let next = UiTransform { scale: Vec2::new(1.0 + 0.06 * e, 1.0 + 0.35 * e), ..UiTransform::IDENTITY };
        rt.set_if_neq(next);
        let c = look.theme.tone_a(Tone::Acc, if r > 0.0 && r < 1.0 { 0.8 * (1.0 - e) } else { 0.0 });
        if o.color != c {
            o.color = c;
        }
    }
}

/// The band's content at `s` times its size.
#[allow(clippy::too_many_arguments)]
fn build_band(
    commands: &mut Commands,
    b: Entity,
    look: &Look,
    info: &WinnerInfo,
    line: &str,
    wins: u32,
    s: f32,
    big: bool,
    slants: Option<&mut Assets<SlantMaterial>>,
) {
    let ui = chimera_ui::controls::Ui { theme: &look.theme, kit: look.fonts.as_deref(), units: *look.units };
    let lay = EndLayout::of(look);
    let h = lay.band_h * s;
    commands.entity(b).with_children(|c| {
        // The accent wash, the left bar, the ring.
        c.spawn((Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() }, BackgroundColor(look.theme.tone_a(Tone::Acc, 0.1)), Pickable::IGNORE));
        c.spawn((Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), bottom: px(0.0), width: look.val(0.3 * s), ..default() }, BackgroundColor(look.tone(Tone::Acc)), Pickable::IGNORE));
        c.spawn((BandPulse, Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() }, Outline::new(px(look.px(0.15 * s)), px(0.0), Color::NONE), UiTransform::default(), Pickable::IGNORE));
        // The trophy.
        c.spawn((Node { flex_shrink: 0.0, ..default() }, Pickable::IGNORE)).with_children(|t| {
            signal::mark(t, look, Mark::Trophy, look.px(h * 0.36), look.tone(Tone::AccText));
        });
    });
    // The avatar.
    if !info.name.is_empty() {
        commands.spawn((
            Node { flex_shrink: 0.0, ..default() },
            Pickable::IGNORE,
            match_end_signal::AvatarSlot { face: match_end_signal::face_of(&info.name, info.bot, info.avatar), you: info.you, size: h * 0.62 },
            ChildOf(b),
        ));
    }
    let mut slants = slants;
    commands.entity(b).with_children(|c| {
        // Who: WINNER, the name with its slab, the line.
        c.spawn((Node { flex_direction: FlexDirection::Column, row_gap: look.val(0.35 * s), flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0.0), ..default() }, Pickable::IGNORE)).with_children(|w| {
            let label = if info.name.is_empty() { "MATCH OVER" } else if info.you { "YOU WIN" } else { "WINNER" };
            w.spawn((look.tracked(Role::Mono, 0.56 * s, 0.56 * s * 0.24, look.tone(Tone::AccText), label), chimera_ui::UiParityId::new("end.winner.k")));
            // The name's room: the band less the trophy, the avatar, the
            // gaps and padding, and in place the quick chat.
            let room = (lay.mid_w - lay.band_h * 0.98 - 1.1 * 3.0 - 2.6 - if big { 0.0 } else { 15.0 }).max(6.0) * s;
            w.spawn((Node { align_self: AlignSelf::FlexStart, ..default() }, Pickable::IGNORE)).with_children(|n| {
                let name = info.name.to_uppercase();
                n.spawn((look.tracked(Role::DisplayBold, 2.3 * s, 2.3 * s * 0.03, look.tone(Tone::Fg), &name), Ellipsize::max(name.clone(), room), WinnerName, chimera_ui::UiParityId::new("end.winner.name")));
                if let Some(slants) = slants.as_deref_mut() {
                    n.spawn((
                        WinnerSlab,
                        MaterialNode(slants.add(SlantMaterial::new(&look.theme, look.theme.tone_a(Tone::Acc, 0.75)))),
                        Node { position_type: PositionType::Absolute, left: look.val(-0.6), top: px(0.0), bottom: px(0.0), width: px(0.0), ..default() },
                        Visibility::Hidden,
                        Pickable::IGNORE,
                    ));
                }
            });
            if !line.is_empty() {
                w.spawn((look.tracked(Role::Mono, 0.62 * s, 0.62 * s * 0.1, look.tone(Tone::Fg2), &line.to_uppercase()), Node { max_width: percent(100.0), ..default() }, Ellipsize::in_parent(line.to_uppercase()), chimera_ui::UiParityId::new("end.winner.line")));
            }
        });
    });
    if big {
        return;
    }
    // In place: the session wins and the quick chat.
    let side = commands
        .spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: look.val(0.45), flex_shrink: 0.0, ..default() }, Pickable::IGNORE, ChildOf(b)))
        .id();
    if wins > 0 {
        commands.entity(side).with_children(|w| {
            w.spawn((look.tracked(Role::Mono, 0.5, 0.08, look.tone(Tone::Fg3), &format!("SESSION WINS {wins}")), super::race_chip::SessionWins(wins), Pickable::IGNORE));
        });
    }
    let row = commands.spawn((Node { column_gap: look.val(0.4), ..default() }, Pickable::IGNORE, ChildOf(side))).id();
    for (i, word) in QUICK_CHAT.iter().enumerate() {
        // The holder takes GG's pop (the kit's button keeps its own states).
        let holder = commands.spawn((Node::default(), UiTransform::default(), Timeline::new([]), Pickable::IGNORE, ChildOf(row))).id();
        let kind = if i == 0 { chimera_ui::controls::Kind::Primary } else { chimera_ui::controls::Kind::Secondary };
        let e = ui.button(commands, holder, kind, word, None, chimera_ui::controls::Size::Small, chimera_ui::controls::State::NONE);
        commands.entity(e).insert((QuickChat(word), chimera_ui::UiParityId(format!("end.winner.chat.{}", i + 1))));
    }
}

/// A quick chat button posts its words (`chat::say`); GG pops and sends a
/// "GG" up off it.
#[allow(clippy::type_complexity)]
fn quick_chat(
    look: Look,
    clock: Res<UiClock>,
    pressed: Query<(Entity, &QuickChat, &super::widgets::Interaction, &ChildOf), Changed<super::widgets::Interaction>>,
    mut holders: Query<&mut Timeline>,
    bands: Query<Entity, With<WinnerBand>>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut commands: Commands,
) {
    for (e, q, i, parent) in &pressed {
        if *i != super::widgets::Interaction::Pressed {
            continue;
        }
        let words = q.0;
        commands.queue(move |w: &mut World| super::chat::say(w, words, false));
        if words != QUICK_CHAT[0] {
            continue;
        }
        if let Ok(mut tl) = holders.get_mut(parent.parent()) {
            let back = look.theme.ease("out");
            *tl = Timeline::new([
                Track::new(Prop::ScaleX, 1.0, 1.18).dur(0.09),
                Track::new(Prop::ScaleY, 1.0, 1.18).dur(0.09),
                Track::new(Prop::ScaleX, 1.18, 1.0).at(0.09).dur(0.35).ease(back),
                Track::new(Prop::ScaleY, 1.18, 1.0).at(0.09).dur(0.35).ease(back),
            ])
            .started(clock.now);
        }
        // "GG" rising off the button, in the band's frame.
        let (Ok(band), Ok((n, t))) = (bands.single(), nodes.get(e)) else { continue };
        let Ok((bn, bt)) = nodes.get(band) else { continue };
        let inv = n.inverse_scale_factor();
        let at = (t.translation - (bt.translation - bn.size() / 2.0)) * inv;
        commands.spawn((
            GgFloat { until: clock.now + 0.9 },
            Node { position_type: PositionType::Absolute, left: px(at.x - look.units.cq(1.2)), top: px(at.y - look.units.cq(2.2)), ..default() },
            look.tracked(Role::DisplayBold, 1.2, 0.0, look.tone(Tone::AccText), "GG"),
            UiTransform::default(),
            Timeline::new([Track::new(Prop::Y, 0.0, -2.4).dur(0.9).ease(look.theme.ease("out")), Track::new(Prop::Opacity, 1.0, 0.0).at(0.3).dur(0.6)]).started(clock.now),
            Pickable::IGNORE,
            ChildOf(band),
        ));
    }
}

/// The GG floats gone when done.
fn floats(clock: Res<UiClock>, floats: Query<(Entity, &GgFloat)>, mut commands: Commands) {
    for (e, f) in &floats {
        if clock.now >= f.until {
            commands.entity(e).despawn();
        }
    }
}

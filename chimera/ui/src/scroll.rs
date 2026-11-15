//! The kit's scroll container (docs/plans/active/ugg-ui.md, M7): a list
//! taller than its box scrolls, with a Signal scroll bar, eased wheel
//! steps and a way to keep an entity in view (the right-button pan is
//! gone: the right button is Back, `shell`).
//!
//! - **Build**: `Ui::scroll(c, parent, key, frame, content)` gives a frame
//!   (a row: the view and the bar), the view (`ScrollView`, clipping,
//!   Bevy's `ScrollPosition`), the content (`ScrollContent`: the rows go
//!   under it) and the bar (`ScrollTrack`, with its `ScrollThumb`).
//!   Screens that rebuild their nodes on every change keep their place:
//!   a view's state lives in `ScrollViews` under its `key`, not on the
//!   node. `ScrollLocked` on the view stops its input (a modal over it, a
//!   key bind listening for the right button).
//! - **The bar**: a hairline track, a thumb as long as the visible share
//!   (at least `thumb_min` cq) with its ends cut on the 12° slant (a
//!   `UiSlant` turned upright), `Fg3` at rest, `Fg` while the pointer is
//!   on it, `Acc` while dragged. Shown only while the content overflows.
//!   Drag the thumb; a click on the track pages toward the click.
//! - **Wheel**: each notch moves the target `wheel_step` cq (a pixel
//!   wheel by its pixels); the offset follows it eased (exponential, time
//!   constant `ease` s), notches adding up.
//! - **No right-button pan** (design rules 11.1: the right button is
//!   Back, `shell::back_keys`): `ScrollState` keeps its pan and fling
//!   (`pan_start`, `pan_end`, the elastic band) for a later drag, but no
//!   input starts one.
//! - **In view**: `ScrollIntoView` on an entity under a view's content
//!   (`scroll_into_view(c, e)`) scrolls it in, eased, with
//!   `reveal_margin` cq around it, once laid out; the marker then goes.
//! - **Reduced motion** (`ReducedMotion`, the client's
//!   `ui_reduce_motion`): no easing (the offset jumps to the target), no
//!   fling, no elastic band.
//!
//! `ui_scroll <view> <cq|top|end>` puts a view somewhere at once
//! (screenshots, tests).
//!
//! The numbers are the theme's `[layout.scroll]` (`ScrollTuning`, data/
//! shell.toml's defaults). The physics (`ScrollState`) is plain data on
//! the UI clock (`UiClock`), so tests step it by hand.
//!
//! Known shortcuts: vertical only; no touch or pen input; the pan reads
//! the pointer through the view's `RelativeCursorPosition` (Bevy's
//! deprecated focus system), as the kit's controls do.

use std::collections::{BTreeMap, BTreeSet};

use bevy::{
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    ui::{ComputedNode, DeprecatedInteraction as Interaction, FocusPolicy, RelativeCursorPosition, ScrollPosition, UiGlobalTransform, UiSystems},
};

use crate::{
    controls::Ui,
    motion::UiClock,
    style::{Flow, UiSlant, UiStyle},
    theme::{Theme, Tone},
    units::UiUnits,
};

/// The layout table.
const L: &str = "scroll";

/// UI motion cut down (the client sets it from `ui_reduce_motion`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReducedMotion(pub bool);

/// The theme's numbers (module doc), cq and seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollTuning {
    pub wheel_step: f32,
    pub ease: f32,
    pub friction: f32,
    pub max_velocity: f32,
    pub overscroll: f32,
    pub spring: f32,
    pub window: f32,
    pub min_velocity: f32,
    pub reveal_margin: f32,
    pub bar_w: f32,
    pub thumb_min: f32,
    pub gap: f32,
}

impl Default for ScrollTuning {
    fn default() -> Self {
        ScrollTuning {
            wheel_step: 3.2,
            ease: 0.07,
            friction: 3.2,
            max_velocity: 220.0,
            overscroll: 3.0,
            spring: 16.0,
            window: 0.1,
            min_velocity: 1.0,
            reveal_margin: 0.6,
            bar_w: 0.5,
            thumb_min: 2.4,
            gap: 0.6,
        }
    }
}

impl ScrollTuning {
    pub fn from_theme(t: &Theme) -> Self {
        let d = ScrollTuning::default();
        let k = |key: &str, or: f32| t.layout_opt(L, key).unwrap_or(or);
        ScrollTuning {
            wheel_step: k("wheel_step", d.wheel_step),
            ease: k("ease", d.ease),
            friction: k("friction", d.friction),
            max_velocity: k("max_velocity", d.max_velocity),
            overscroll: k("overscroll", d.overscroll),
            spring: k("spring", d.spring),
            window: k("window", d.window),
            min_velocity: k("min_velocity", d.min_velocity),
            reveal_margin: k("reveal_margin", d.reveal_margin),
            bar_w: k("bar_w", d.bar_w),
            thumb_min: k("thumb_min", d.thumb_min),
            gap: k("gap", d.gap),
        }
    }
}

/// A right-button pan under way.
#[derive(Clone, Debug, PartialEq)]
struct Pan {
    /// The pointer's y where it began, and the offset then (cq).
    y0: f32,
    off0: f32,
    /// The unbanded offset at each step of the last `window` s.
    samples: Vec<(f64, f32)>,
}

/// A thumb drag: where on the thumb it was taken (cq from its top).
#[derive(Clone, Copy, Debug, PartialEq)]
struct ThumbDrag {
    grab: f32,
}

/// One view's scrolling (module doc), in cq from the content's top.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScrollState {
    /// Where it is now (outside 0..=max while on the elastic band).
    pub offset: f32,
    /// Where the eased scroll goes (a wheel, a page, a reveal).
    pub target: Option<f32>,
    /// A fling's speed, cq/s (+ down the content).
    pub velocity: f32,
    /// The farthest offset (content less view), as last laid out.
    pub max: f32,
    /// The view's and the content's heights, the track's, cq.
    pub view: f32,
    pub content: f32,
    pub track: f32,
    /// The pointer is on the thumb.
    pub hover_thumb: bool,
    pan: Option<Pan>,
    thumb: Option<ThumbDrag>,
}

impl ScrollState {
    /// The offset inside 0..=max (what Bevy's `ScrollPosition` holds).
    pub fn inside(&self) -> f32 {
        self.offset.clamp(0.0, self.max.max(0.0))
    }

    /// How far past an edge (- above the top, + below the bottom).
    pub fn over(&self) -> f32 {
        self.offset - self.inside()
    }

    /// The content is taller than the view (the bar shows).
    pub fn overflows(&self) -> bool {
        self.max > 0.01
    }

    pub fn panning(&self) -> bool {
        self.pan.is_some()
    }

    pub fn thumb_dragged(&self) -> bool {
        self.thumb.is_some()
    }

    /// At rest: no pan, fling, ease or band.
    pub fn at_rest(&self) -> bool {
        self.pan.is_none() && self.velocity == 0.0 && self.target.is_none() && self.over() == 0.0
    }

    fn base(&self) -> f32 {
        self.target.unwrap_or(self.offset)
    }

    /// Set the content's measure (cq); the offset kept inside the new range
    /// unless something moves it.
    pub fn measure(&mut self, view: f32, content: f32) {
        self.view = view;
        self.content = content;
        self.max = (content - view).max(0.0);
        if let Some(t) = self.target {
            self.target = Some(t.clamp(0.0, self.max));
        }
    }

    /// A press over the view: a fling or an ease stops where it is.
    pub fn stop(&mut self) {
        self.velocity = 0.0;
        self.target = None;
    }

    /// Go to `to` at once (a page that starts at its top).
    pub fn jump(&mut self, to: f32) {
        self.stop();
        self.pan = None;
        self.thumb = None;
        self.offset = to.clamp(0.0, self.max);
    }

    /// Ease to `to` (clamped); at once with reduced motion.
    pub fn ease_to(&mut self, to: f32, reduced: bool) {
        self.velocity = 0.0;
        let to = to.clamp(0.0, self.max);
        if reduced {
            self.offset = to;
            self.target = None;
        } else {
            self.target = Some(to);
        }
    }

    /// A wheel's move, cq (+ down the content).
    pub fn wheel(&mut self, d: f32, reduced: bool) {
        if self.pan.is_some() || self.thumb.is_some() {
            return;
        }
        let b = self.base().clamp(0.0, self.max);
        self.ease_to(b + d, reduced);
    }

    /// Bring the content's `top..bottom` (cq) into view, `margin` around it.
    pub fn reveal(&mut self, top: f32, bottom: f32, margin: f32, reduced: bool) {
        if self.pan.is_some() || self.thumb.is_some() {
            return;
        }
        let b = self.base();
        let to = if top - margin < b {
            top - margin
        } else if bottom + margin > b + self.view {
            (bottom + margin - self.view).min(top - margin)
        } else {
            return;
        };
        let to = to.clamp(0.0, self.max);
        if (to - b).abs() > 0.001 {
            self.ease_to(to, reduced);
        }
    }

    /// The right button went down with the pointer `y` cq down the view.
    pub fn pan_start(&mut self, y: f32, now: f64) {
        self.stop();
        self.thumb = None;
        self.pan = Some(Pan { y0: y, off0: self.offset, samples: vec![(now, self.offset)] });
    }

    /// The pointer at `y` while panning: the content follows 1:1, banded
    /// past an edge.
    pub fn pan_move(&mut self, y: f32, now: f64, tune: &ScrollTuning, reduced: bool) {
        let Some(p) = self.pan.as_mut() else { return };
        let raw = p.off0 - (y - p.y0);
        p.samples.push((now, raw));
        let keep = now - tune.window as f64;
        // Keep one sample at or before the window's start.
        while p.samples.len() > 2 && p.samples[1].0 <= keep {
            p.samples.remove(0);
        }
        self.offset = band(raw, self.max, if reduced { 0.0 } else { tune.overscroll });
    }

    /// The right button let go: a fling at the last `window`'s speed.
    pub fn pan_end(&mut self, now: f64, tune: &ScrollTuning, reduced: bool) {
        let Some(p) = self.pan.take() else { return };
        if reduced {
            self.velocity = 0.0;
            self.offset = self.inside();
            return;
        }
        let keep = now - tune.window as f64;
        let recent: Vec<&(f64, f32)> = p.samples.iter().filter(|s| s.0 >= keep - 1e-9).collect();
        let first = recent.first().copied().or(p.samples.last());
        let last = p.samples.last();
        self.velocity = match (first, last) {
            (Some(a), Some(b)) if b.0 - a.0 > 1e-6 => ((b.1 - a.1) / (b.0 - a.0) as f32).clamp(-tune.max_velocity, tune.max_velocity),
            _ => 0.0,
        };
        if self.velocity.abs() < tune.min_velocity {
            self.velocity = 0.0;
        }
    }

    /// Where the thumb is: its top and length, cq down a track `track` cq.
    pub fn thumb(&self, track: f32, tune: &ScrollTuning) -> (f32, f32) {
        if self.content <= 0.0 || !self.overflows() {
            return (0.0, track);
        }
        let len = (track * self.view / self.content).clamp(tune.thumb_min.min(track), track);
        let frac = self.inside() / self.max;
        ((track - len) * frac, len)
    }

    /// A left press on the track at `y` cq: on the thumb takes it, else a
    /// page toward the press.
    pub fn track_press(&mut self, y: f32, tune: &ScrollTuning, reduced: bool) {
        let (top, len) = self.thumb(self.track, tune);
        self.stop();
        if y >= top && y <= top + len {
            self.thumb = Some(ThumbDrag { grab: y - top });
        } else {
            let page = (self.view * 0.9).max(1.0);
            let b = self.base();
            self.ease_to(if y < top { b - page } else { b + page }, reduced);
        }
    }

    /// The thumb dragged to `y` cq on the track.
    pub fn thumb_move(&mut self, y: f32, tune: &ScrollTuning) {
        let Some(d) = self.thumb else { return };
        let (_, len) = self.thumb(self.track, tune);
        let room = (self.track - len).max(0.001);
        self.offset = ((y - d.grab) / room).clamp(0.0, 1.0) * self.max;
    }

    pub fn thumb_end(&mut self) {
        self.thumb = None;
    }

    /// One step of `dt` s: the fling, the spring, the ease.
    pub fn step(&mut self, dt: f32, tune: &ScrollTuning, reduced: bool) {
        if self.pan.is_some() || self.thumb.is_some() || dt <= 0.0 {
            return;
        }
        if reduced {
            self.velocity = 0.0;
            if let Some(t) = self.target.take() {
                self.offset = t;
            }
            self.offset = self.inside();
            return;
        }
        if self.velocity != 0.0 {
            self.target = None;
            self.offset += self.velocity * dt;
            let over = self.over();
            if over != 0.0 {
                // Past an edge: the spring brakes it hard; once it stops
                // going out, it eases back in (below) instead of bouncing.
                self.velocity -= over * tune.spring * tune.spring * dt;
                self.velocity *= (-tune.spring * dt).exp();
                if self.velocity * over <= 0.0 {
                    self.velocity = 0.0;
                }
                if over.abs() > tune.overscroll {
                    self.offset = self.inside() + tune.overscroll * over.signum();
                }
            } else {
                self.velocity *= (-tune.friction * dt).exp();
            }
            if self.velocity.abs() < tune.min_velocity {
                self.velocity = 0.0;
            }
            return;
        }
        let over = self.over();
        if over != 0.0 {
            let left = over * (-tune.spring * dt).exp();
            self.offset = self.inside() + if left.abs() < 0.01 { 0.0 } else { left };
            return;
        }
        if let Some(t) = self.target {
            let k = if tune.ease > 0.0 { 1.0 - (-dt / tune.ease).exp() } else { 1.0 };
            self.offset += (t - self.offset) * k;
            if (t - self.offset).abs() < 0.01 {
                self.offset = t;
                self.target = None;
            }
        }
    }
}

/// The elastic band: `raw` past 0..=max shown at most `limit` past it.
pub fn band(raw: f32, max: f32, limit: f32) -> f32 {
    let max = max.max(0.0);
    let past = |d: f32| if limit > 0.0 { limit * d / (d + limit) } else { 0.0 };
    if raw < 0.0 {
        -past(-raw)
    } else if raw > max {
        max + past(raw - max)
    } else {
        raw
    }
}

/// Every view's state by its key (module doc).
#[derive(Resource, Clone, Debug, Default)]
pub struct ScrollViews {
    pub states: BTreeMap<String, ScrollState>,
}

impl ScrollViews {
    pub fn get(&self, key: &str) -> Option<&ScrollState> {
        self.states.get(key)
    }

    pub fn state(&mut self, key: &str) -> &mut ScrollState {
        self.states.entry(key.to_string()).or_default()
    }

    /// Back to the top at once (a new page in the same view).
    pub fn reset(&mut self, key: &str) {
        if let Some(s) = self.states.get_mut(key) {
            s.jump(0.0);
        }
    }

    /// Forget a view (its screen closed).
    pub fn forget(&mut self, key: &str) {
        self.states.remove(key);
    }
}

/// A scrolling view (module doc): its state's key.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ScrollView {
    pub key: String,
}

/// The view takes no input (a modal over it, a key bind listening).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ScrollLocked;

/// The node the rows go under, inside `view`.
#[derive(Component, Clone, Copy, Debug)]
pub struct ScrollContent {
    pub view: Entity,
}

/// The bar beside `view`.
#[derive(Component, Clone, Copy, Debug)]
pub struct ScrollTrack {
    pub view: Entity,
}

/// The bar's thumb.
#[derive(Component, Clone, Copy, Debug)]
pub struct ScrollThumb {
    pub view: Entity,
}

/// Scroll this entity into its view (module doc).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ScrollIntoView;

/// Scroll `e` (under a view's content) into view once it is laid out.
pub fn scroll_into_view(c: &mut Commands, e: Entity) {
    c.entity(e).insert(ScrollIntoView);
}

/// What `Ui::scroll` built.
#[derive(Clone, Copy, Debug)]
pub struct ScrollParts {
    pub frame: Entity,
    pub view: Entity,
    pub content: Entity,
    pub track: Entity,
    pub thumb: Entity,
}

impl<'a> Ui<'a> {
    /// A scroll container under `parent` (module doc): `frame` sizes it in
    /// its parent (made a row that stretches), `content` styles the node
    /// the rows go under (a column, full width, by default).
    pub fn scroll(&self, c: &mut Commands, parent: Entity, key: &str, frame: UiStyle, content: UiStyle) -> ScrollParts {
        let tune = ScrollTuning::from_theme(self.theme);
        let u = &self.units;
        let frame = c
            .spawn((UiStyle { flow: Flow::Row, align: Some(AlignItems::Stretch), ..frame }, Node::default(), ChildOf(parent)))
            .id();
        let view = c
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_basis: Val::Px(0.0),
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollView { key: key.to_string() },
                ScrollPosition::default(),
                RelativeCursorPosition::default(),
                Name::new(format!("scroll {key}")),
                ChildOf(frame),
            ))
            .id();
        let content = c
            .spawn((
                UiStyle { flow: Flow::Column, w_pct: content.w_pct.or(Some(100.0)), shrink: content.shrink.or(Some(0.0)), ..content },
                Node::default(),
                UiTransform::IDENTITY,
                ScrollContent { view },
                ChildOf(view),
            ))
            .id();
        let track = c
            .spawn((
                Node {
                    width: Val::Px(u.snap(u.cq(tune.bar_w))),
                    flex_shrink: 0.0,
                    margin: UiRect::left(Val::Px(u.snap(u.cq(tune.gap)))),
                    ..default()
                },
                ScrollTrack { view },
                RelativeCursorPosition::default(),
                Interaction::default(),
                FocusPolicy::Block,
                Visibility::Hidden,
                ChildOf(frame),
            ))
            .id();
        // The hairline down its middle.
        let hair = u.hairline();
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(u.snap((u.cq(tune.bar_w) - hair) * 0.5)),
                width: Val::Px(hair),
                top: Val::ZERO,
                bottom: Val::ZERO,
                ..default()
            },
            BackgroundColor(self.color(Tone::Fg4, 1.0)),
            Pickable::IGNORE,
            ChildOf(track),
        ));
        let thumb = c
            .spawn((
                Node { position_type: PositionType::Absolute, ..default() },
                // Laid flat and turned upright: its ends take the slant.
                UiTransform { rotation: Rot2::degrees(90.0), ..UiTransform::IDENTITY },
                UiSlant::new(Tone::Fg3, 1.0),
                ScrollThumb { view },
                Pickable::IGNORE,
                ChildOf(track),
            ))
            .id();
        ScrollParts { frame, view, content, track, thumb }
    }
}

/// The input's systems (in `PreUpdate`, after Bevy's UI focus).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScrollInput;

/// Logical px to cq.
fn to_cq(units: &UiUnits, logical: f32) -> f32 {
    let one = units.cq(1.0);
    if one > 0.0 { logical / one } else { 0.0 }
}

/// A node's height in cq.
fn height_cq(units: &UiUnits, n: &ComputedNode) -> f32 {
    to_cq(units, n.size.y * n.inverse_scale_factor)
}

/// The pointer's y down a node, cq.
fn pointer_y(units: &UiUnits, n: &ComputedNode, pos: &RelativeCursorPosition) -> Option<f32> {
    pos.normalized.map(|p| (p.y + 0.5) * height_cq(units, n))
}

/// The wheel, the right-button pan, the bar (module doc).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn scroll_input(
    mut mouse: Option<ResMut<ButtonInput<MouseButton>>>,
    wheel: Option<Res<AccumulatedMouseScroll>>,
    clock: Res<UiClock>,
    theme: Res<Theme>,
    units: Res<UiUnits>,
    reduced: Option<Res<ReducedMotion>>,
    mut views: ResMut<ScrollViews>,
    view_q: Query<(&ScrollView, &RelativeCursorPosition, &ComputedNode, Has<ScrollLocked>)>,
    track_q: Query<(&ScrollTrack, &RelativeCursorPosition, &ComputedNode)>,
) {
    let tune = ScrollTuning::from_theme(&theme);
    let reduced = reduced.is_some_and(|r| r.0);
    let now = clock.now;
    let (right_down, mut right_up) = (false, false);
    for (v, pos, node, locked) in &view_q {
        let s = views.state(&v.key);
        if locked {
            s.pan = None;
            s.thumb = None;
            continue;
        }
        let y = pointer_y(&units, node, pos);
        if let Some(m) = mouse.as_deref() {
            // The right button is Back on every screen (`shell::back_keys`,
            // the design rules' 11.1): no pan starts from it any more.
            if m.just_pressed(MouseButton::Left) && pos.cursor_over {
                s.stop();
            }
            if s.pan.is_some() {
                if m.pressed(MouseButton::Right) {
                    if let Some(y) = y {
                        s.pan_move(y, now, &tune, reduced);
                    }
                } else {
                    s.pan_end(now, &tune, reduced);
                    right_up = true;
                }
            }
        }
        if let Some(w) = wheel.as_deref()
            && w.delta.y != 0.0
            && pos.cursor_over
        {
            let d = match w.unit {
                MouseScrollUnit::Line => w.delta.y * tune.wheel_step,
                MouseScrollUnit::Pixel => to_cq(&units, w.delta.y),
            };
            s.wheel(-d, reduced);
        }
    }
    for (t, pos, node) in &track_q {
        let Ok((v, _, _, locked)) = view_q.get(t.view) else { continue };
        let s = views.state(&v.key);
        let y = pointer_y(&units, node, pos);
        let (top, len) = s.thumb(s.track, &tune);
        let on_thumb = pos.cursor_over && y.is_some_and(|y| y >= top && y <= top + len);
        if s.hover_thumb != on_thumb {
            s.hover_thumb = on_thumb;
        }
        if locked || !s.overflows() {
            continue;
        }
        let Some(m) = mouse.as_deref() else { continue };
        if m.just_pressed(MouseButton::Left) && pos.cursor_over {
            if let Some(y) = y {
                s.track_press(y, &tune, reduced);
            }
        } else if s.thumb.is_some() {
            if m.pressed(MouseButton::Left) {
                if let Some(y) = y {
                    s.thumb_move(y, &tune);
                }
            } else {
                s.thumb_end();
            }
        }
    }
    if let Some(m) = mouse.as_deref_mut() {
        // Taken: not a context action, not the game's.
        if right_down {
            m.clear_just_pressed(MouseButton::Right);
        }
        if right_up {
            m.clear_just_released(MouseButton::Right);
        }
    }
}

/// Measures, reveals, steps and writes each view (module doc).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn scroll_step(
    clock: Res<UiClock>,
    theme: Res<Theme>,
    units: Res<UiUnits>,
    reduced: Option<Res<ReducedMotion>>,
    mut views: ResMut<ScrollViews>,
    mut view_q: Query<(Entity, &ScrollView, &ComputedNode, &mut ScrollPosition)>,
    mut content_q: Query<(Entity, &ScrollContent, &ComputedNode, &UiGlobalTransform, &mut UiTransform)>,
    reveal_q: Query<(Entity, &ComputedNode, &UiGlobalTransform), With<ScrollIntoView>>,
    parents: Query<&ChildOf>,
    mut track_q: Query<(&ScrollTrack, &ComputedNode, &mut Visibility)>,
    mut thumb_q: Query<(&ScrollThumb, &mut Node, &mut UiSlant)>,
    mut commands: Commands,
) {
    let tune = ScrollTuning::from_theme(&theme);
    let reduced = reduced.is_some_and(|r| r.0);
    let px = units.cq(1.0);
    // Measure: the content as last laid out.
    for (_, content, cn, _, _) in &content_q {
        let Ok((_, v, vn, _)) = view_q.get(content.view) else { continue };
        if vn.size.y <= 0.0 || cn.size.y <= 0.0 {
            continue;
        }
        let s = views.state(&v.key);
        s.measure(height_cq(&units, vn), height_cq(&units, cn));
    }
    for (t, tn, _) in &track_q {
        if tn.size.y <= 0.0 {
            continue;
        }
        if let Ok((_, v, _, _)) = view_q.get(t.view) {
            views.state(&v.key).track = height_cq(&units, tn);
        }
    }
    // Reveals, once laid out.
    for (e, n, gt) in &reveal_q {
        let mut up = parents.get(e).ok().map(|c| c.parent());
        let mut found = None;
        while let Some(a) = up {
            if let Ok((_, content, cn, cgt, _)) = content_q.get(a) {
                found = Some((content.view, *cn, *cgt));
                break;
            }
            up = parents.get(a).ok().map(|c| c.parent());
        }
        let Some((view, cn, cgt)) = found else {
            commands.entity(e).remove::<ScrollIntoView>();
            continue;
        };
        if n.size.y <= 0.0 || cn.size.y <= 0.0 {
            continue;
        }
        let Ok((_, v, _, _)) = view_q.get(view) else { continue };
        let inv = n.inverse_scale_factor;
        let content_top = cgt.translation.y - cn.size.y * 0.5;
        let top = to_cq(&units, (gt.translation.y - n.size.y * 0.5 - content_top) * inv);
        let bottom = top + height_cq(&units, n);
        // The content's own band and scroll are in both: no correction.
        views.state(&v.key).reveal(top, bottom, tune.reveal_margin, reduced);
        commands.entity(e).remove::<ScrollIntoView>();
    }
    // Step and write.
    let mut stepped = BTreeSet::new();
    for (_, v, _, mut pos) in &mut view_q {
        let s = views.state(&v.key);
        if stepped.insert(v.key.clone()) {
            s.step(clock.dt, &tune, reduced);
        }
        let y = s.inside() * px;
        if pos.0.y != y {
            pos.0.y = y;
        }
    }
    for (_, content, _, _, mut tf) in &mut content_q {
        let Ok((_, v, _, _)) = view_q.get(content.view) else { continue };
        let Some(s) = views.get(&v.key) else { continue };
        let want = Val2::px(0.0, -s.over() * px);
        if tf.translation != want {
            tf.translation = want;
        }
    }
    for (t, _, mut vis) in &mut track_q {
        let Ok((_, v, _, _)) = view_q.get(t.view) else { continue };
        let show = views.get(&v.key).is_some_and(|s| s.overflows());
        vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
    for (th, mut node, mut slant) in &mut thumb_q {
        let Ok((_, v, _, _)) = view_q.get(th.view) else { continue };
        let Some(s) = views.get(&v.key) else { continue };
        let (top, len) = s.thumb(s.track, &tune);
        let (bar, len_px) = (units.snap(units.cq(tune.bar_w)), units.snap(len * px));
        // Laid flat (len x bar) about its centre, then turned upright.
        let want = Node {
            position_type: PositionType::Absolute,
            width: Val::Px(len_px),
            height: Val::Px(bar),
            left: Val::Px(units.snap((bar - len_px) * 0.5)),
            top: Val::Px(units.snap(top * px + (len_px - bar) * 0.5)),
            ..default()
        };
        if *node != want {
            *node = want;
        }
        let tone = if s.thumb_dragged() {
            Tone::Acc
        } else if s.hover_thumb {
            Tone::Fg
        } else {
            Tone::Fg3
        };
        if slant.fill != tone {
            slant.fill = tone;
        }
    }
}

/// The scroll container's systems (in the kit's plugin).
pub struct ScrollPlugin;

impl Plugin for ScrollPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScrollViews>()
            .init_resource::<ReducedMotion>()
            .configure_sets(PreUpdate, ScrollInput.after(UiSystems::Focus))
            .add_systems(PreUpdate, scroll_input.in_set(ScrollInput))
            .add_systems(
                PostUpdate,
                scroll_step.after(crate::motion::advance_clock).after(crate::style::resolve_styles).in_set(crate::UiKitSystems),
            );
        use chimera_core::console::ConsoleAppExt;
        app.console_command(
            "chimera:ui_scroll",
            "ui_scroll [<view> <cq|top|end>]: put a scroll view there at once (screenshots, tests); alone: the views and where they are.",
            |w, a| {
                let mut views = w.get_resource_mut::<ScrollViews>().ok_or("no scroll views")?;
                let (Some(key), Some(at)) = (a.first(), a.get(1)) else {
                    let lines: Vec<String> = views.states.iter().map(|(k, s)| format!("{k}: {:.2} of {:.2} cq", s.offset, s.max)).collect();
                    return Ok(Some(lines.join("\n")));
                };
                let s = views.states.get_mut(key.as_str()).ok_or_else(|| format!("ui_scroll: no view `{key}`"))?;
                let to = match at.as_str() {
                    "top" => 0.0,
                    "end" => s.max,
                    n => n.parse::<f32>().map_err(|_| format!("ui_scroll: not a number: {n}"))?,
                };
                s.jump(to);
                Ok(None)
            },
        );
    }
}

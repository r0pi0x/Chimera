//! The motion kit (docs/plans/active/ugg-ui.md, 2.5): easings, tracks
//! and timelines, on a clock tests step by hand.
//!
//! - `Ease`: the prototype's curves (`expo_out`, `expo_in`,
//!   `cubic_in_out`, `quint_in_out`, `back(s)`, `hold`, CSS
//!   `bezier(x1,y1,x2,y2)`, `steps(n)`, `steps_none(n)`, `pulse`),
//!   named in the theme (`Ease::parse`).
//! - `UiClock`: UI time in seconds. Real time in play
//!   (`UiClock::advance_real`); `manual` in tests, stepped with `step`,
//!   so frames are reproducible.
//! - `Track`: one property from `from` to `to`, starting `at` seconds
//!   into its timeline, lasting `dur`, eased.
//! - `Timeline` (component): tracks over a few properties (`Prop`),
//!   sampled at absolute time (so 60 and 144 steps a second end alike).
//!   Of a property's tracks the latest that has started holds it; before
//!   the first, its `from`. `redirect` folds a property from where it is
//!   now to a new target (Tab released mid-open), `restart` plays it
//!   again (a chained rank-up).
//! - `step_timelines` writes each timeline's values (`Timeline::value`)
//!   only when they change; `apply_timelines` writes translate, scale
//!   (`UiTransform`) and width/height (`Node`, cq through `UiUnits`) on
//!   change. Opacity, clips, letter spacing and material parameters are
//!   read by the screen that owns the node (`Timeline::value`).

use bevy::prelude::*;

use crate::units::UiUnits;

/// An easing curve: `sample(p)` for `p` in 0..1 (clamped; `Pulse` takes
/// seconds-like input unclamped).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ease {
    Linear,
    /// `p == 1 ? 1 : 1 - 2^(-10p)`.
    ExpoOut,
    /// `p == 0 ? 0 : 2^(10p - 10)`.
    ExpoIn,
    /// `p < .5 ? 4p^3 : 1 - (-2p + 2)^3 / 2`.
    CubicInOut,
    /// `p < .5 ? 16p^5 : 1 - (-2p + 2)^5 / 2`.
    QuintInOut,
    /// `q = p - 1; 1 + (s + 1)q^3 + s q^2` (overshoots; the toast's roll
    /// is s = .9).
    Back(f32),
    /// `q = 2p - 1; .5 + .5 sign(q) |q|^3` (the logo pass: fast, slow
    /// while covering, fast).
    Hold,
    /// A CSS cubic-bezier (x1, y1, x2, y2).
    Bezier([f32; 4]),
    /// CSS `steps(n)` (jump-end): `floor(p n) / n`.
    Steps(u32),
    /// CSS `steps(n, jump-none)`: n levels from 0 to 1.
    StepsNone(u32),
    /// `x < .08 ? x / .08 : exp(-(x - .08) / .32)` (the air strike's
    /// edge), unclamped.
    Pulse,
}

impl Ease {
    /// The prototype's CSS curves.
    pub const OUT: Ease = Ease::Bezier([0.16, 1.0, 0.3, 1.0]);
    pub const IN: Ease = Ease::Bezier([0.7, 0.0, 0.84, 0.0]);
    pub const IN_OUT: Ease = Ease::Bezier([0.65, 0.0, 0.35, 1.0]);
    pub const POP: Ease = Ease::Bezier([0.34, 1.56, 0.64, 1.0]);

    pub fn sample(self, p: f32) -> f32 {
        if let Ease::Pulse = self {
            let x = p;
            return if x < 0.0 {
                0.0
            } else if x < 0.08 {
                x / 0.08
            } else {
                (-(x - 0.08) / 0.32).exp()
            };
        }
        let p = p.clamp(0.0, 1.0);
        match self {
            Ease::Linear => p,
            Ease::ExpoOut => {
                if p == 1.0 {
                    1.0
                } else {
                    1.0 - 2f32.powf(-10.0 * p)
                }
            }
            Ease::ExpoIn => {
                if p == 0.0 {
                    0.0
                } else {
                    2f32.powf(10.0 * p - 10.0)
                }
            }
            Ease::CubicInOut => {
                if p < 0.5 {
                    4.0 * p * p * p
                } else {
                    1.0 - (-2.0 * p + 2.0).powi(3) / 2.0
                }
            }
            Ease::QuintInOut => {
                if p < 0.5 {
                    16.0 * p.powi(5)
                } else {
                    1.0 - (-2.0 * p + 2.0).powi(5) / 2.0
                }
            }
            Ease::Back(s) => {
                let q = p - 1.0;
                1.0 + (s + 1.0) * q * q * q + s * q * q
            }
            Ease::Hold => {
                let q = 2.0 * p - 1.0;
                0.5 + 0.5 * q.signum() * q.abs().powi(3)
            }
            Ease::Bezier(c) => bezier(c, p),
            Ease::Steps(n) => {
                let n = n.max(1) as f32;
                ((p * n).floor() / n).min(1.0)
            }
            Ease::StepsNone(n) => {
                let n = n.max(2) as f32;
                (p * n).floor().min(n - 1.0) / (n - 1.0)
            }
            Ease::Pulse => unreachable!(),
        }
    }

    /// A curve by name: `linear`, `expo_out`, `expo_in`, `cubic_in_out`,
    /// `quint_in_out`, `hold`, `pulse`, `back(s)`, `bezier(a,b,c,d)`,
    /// `steps(n)`, `steps_none(n)`.
    pub fn parse(text: &str) -> Result<Ease, String> {
        let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let simple = match t.as_str() {
            "linear" => Some(Ease::Linear),
            "expo_out" => Some(Ease::ExpoOut),
            "expo_in" => Some(Ease::ExpoIn),
            "cubic_in_out" => Some(Ease::CubicInOut),
            "quint_in_out" => Some(Ease::QuintInOut),
            "hold" => Some(Ease::Hold),
            "pulse" => Some(Ease::Pulse),
            _ => None,
        };
        if let Some(e) = simple {
            return Ok(e);
        }
        let (name, args) = t
            .strip_suffix(')')
            .and_then(|t| t.split_once('('))
            .ok_or_else(|| format!("unknown easing `{text}`"))?;
        let nums: Vec<f32> = args
            .split(',')
            .map(|a| a.parse::<f32>().map_err(|e| format!("`{text}`: {e}")))
            .collect::<Result<_, _>>()?;
        match (name, nums.as_slice()) {
            ("back", [s]) => Ok(Ease::Back(*s)),
            ("bezier", [a, b, c, d]) => Ok(Ease::Bezier([*a, *b, *c, *d])),
            ("steps", [n]) => Ok(Ease::Steps(*n as u32)),
            ("steps_none", [n]) => Ok(Ease::StepsNone(*n as u32)),
            _ => Err(format!("unknown easing `{text}`")),
        }
    }
}

/// A CSS cubic-bezier's y at x = `p`: Newton on x(t), then bisection.
fn bezier([x1, y1, x2, y2]: [f32; 4], p: f32) -> f32 {
    if p <= 0.0 {
        return 0.0;
    }
    if p >= 1.0 {
        return 1.0;
    }
    let curve = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
    };
    let slope = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
    };
    let mut t = p;
    for _ in 0..8 {
        let x = curve(x1, x2, t) - p;
        if x.abs() < 1e-6 {
            return curve(y1, y2, t);
        }
        let d = slope(x1, x2, t);
        if d.abs() < 1e-6 {
            break;
        }
        t -= x / d;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    t = p;
    for _ in 0..40 {
        let x = curve(x1, x2, t);
        if (x - p).abs() < 1e-6 {
            break;
        }
        if x < p {
            lo = t;
        } else {
            hi = t;
        }
        t = (lo + hi) / 2.0;
    }
    curve(y1, y2, t)
}

/// UI time (module doc).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct UiClock {
    /// Seconds since the clock started.
    pub now: f64,
    /// The last step's length.
    pub dt: f32,
    /// Stepped by hand (`step`) instead of from real time.
    pub manual: bool,
}

impl UiClock {
    /// A clock that only `step` moves (tests).
    pub fn manual() -> Self {
        UiClock { manual: true, ..default() }
    }

    pub fn step(&mut self, dt: f32) {
        self.dt = dt;
        self.now += dt as f64;
    }
}

/// Moves the clock by the frame's real time unless it is manual.
pub fn advance_clock(time: Res<Time<Real>>, mut clock: ResMut<UiClock>) {
    if !clock.manual {
        clock.step(time.delta_secs());
    }
}

/// A property a timeline animates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prop {
    /// 0-1.
    Opacity,
    /// Translate x in cq.
    X,
    /// Translate y in cq.
    Y,
    /// Clip from the left, 0-1 of the width.
    ClipLeft,
    /// Clip from the right, 0-1.
    ClipRight,
    /// Width in cq.
    Width,
    /// Height in cq.
    Height,
    ScaleX,
    ScaleY,
    /// Letter spacing in em.
    Spacing,
    /// A material's parameter (the screen's own numbering).
    Param(u8),
}

/// One property's move (module doc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    pub prop: Prop,
    pub from: f32,
    pub to: f32,
    /// Start, seconds into the timeline.
    pub at: f32,
    /// Length in seconds (0: a jump at `at`).
    pub dur: f32,
    pub ease: Ease,
}

impl Track {
    /// `prop` from `from` to `to`, at 0, over 0 s, linear: set the rest
    /// with `at`, `dur`, `ease`.
    pub fn new(prop: Prop, from: f32, to: f32) -> Track {
        Track { prop, from, to, at: 0.0, dur: 0.0, ease: Ease::Linear }
    }

    pub fn at(mut self, at: f32) -> Track {
        self.at = at;
        self
    }

    pub fn dur(mut self, dur: f32) -> Track {
        self.dur = dur;
        self
    }

    pub fn ease(mut self, ease: Ease) -> Track {
        self.ease = ease;
        self
    }

    /// Its value `t` seconds into the timeline.
    pub fn value(&self, t: f32) -> f32 {
        let p = if self.dur <= 0.0 {
            if t >= self.at { 1.0 } else { 0.0 }
        } else {
            (t - self.at) / self.dur
        };
        let k = self.ease.sample(p);
        if k == 1.0 {
            // Exactly the target at the end (no rounding left over).
            return self.to;
        }
        self.from + (self.to - self.from) * k
    }

    pub fn end(&self) -> f32 {
        self.at + self.dur
    }
}

/// Tracks on one node (module doc).
#[derive(Component, Clone, Debug, Default)]
pub struct Timeline {
    pub tracks: Vec<Track>,
    /// When it started on the `UiClock`; `None`: at the next step.
    pub start: Option<f64>,
    values: Vec<(Prop, f32)>,
}

impl Timeline {
    pub fn new(tracks: impl IntoIterator<Item = Track>) -> Timeline {
        Timeline { tracks: tracks.into_iter().collect(), ..default() }
    }

    /// Started at `now` (instead of at the next step).
    pub fn started(mut self, now: f64) -> Timeline {
        self.start = Some(now);
        self
    }

    /// Seconds since it started (0 before).
    pub fn local(&self, now: f64) -> f32 {
        self.start.map_or(0.0, |s| (now - s) as f32)
    }

    /// `prop`'s value `t` seconds in (module doc); `None` if no track
    /// moves it.
    pub fn sample(&self, prop: Prop, t: f32) -> Option<f32> {
        let mut first: Option<&Track> = None;
        let mut current: Option<&Track> = None;
        for tr in self.tracks.iter().filter(|tr| tr.prop == prop) {
            if first.is_none_or(|f| tr.at < f.at) {
                first = Some(tr);
            }
            if tr.at <= t && current.is_none_or(|c| tr.at >= c.at) {
                current = Some(tr);
            }
        }
        match current {
            Some(c) => Some(c.value(t)),
            None => first.map(|f| f.from),
        }
    }

    /// `prop`'s value at the last step (`step_timelines`).
    pub fn value(&self, prop: Prop) -> Option<f32> {
        self.values.iter().find(|(p, _)| *p == prop).map(|(_, v)| *v)
    }

    /// `prop`'s last value, else `or`.
    pub fn value_or(&self, prop: Prop, or: f32) -> f32 {
        self.value(prop).unwrap_or(or)
    }

    /// Every property's values at absolute time `now`; true if any
    /// changed.
    pub fn step(&mut self, now: f64) -> bool {
        let start = *self.start.get_or_insert(now);
        let t = (now - start) as f32;
        let mut props: Vec<Prop> = Vec::new();
        for tr in &self.tracks {
            if !props.contains(&tr.prop) {
                props.push(tr.prop);
            }
        }
        let next: Vec<(Prop, f32)> = props.into_iter().filter_map(|p| self.sample(p, t).map(|v| (p, v))).collect();
        if next != self.values {
            self.values = next;
            true
        } else {
            false
        }
    }

    /// When the last track ends, seconds in.
    pub fn end(&self) -> f32 {
        self.tracks.iter().map(Track::end).fold(0.0, f32::max)
    }

    /// Every track has ended at `now`.
    pub fn done(&self, now: f64) -> bool {
        self.start.is_some() && self.local(now) >= self.end()
    }

    /// Plays from the start again at `now`.
    pub fn restart(&mut self, now: f64) {
        self.start = Some(now);
    }

    /// `prop` from where it is at `now` to `to` over `dur` (its other
    /// tracks dropped): an interruption that continues, no jump.
    pub fn redirect(&mut self, now: f64, prop: Prop, to: f32, dur: f32, ease: Ease) {
        let t = match self.start {
            Some(_) => self.local(now),
            None => {
                self.start = Some(now);
                0.0
            }
        };
        let from = self.sample(prop, t).unwrap_or(to);
        self.tracks.retain(|tr| tr.prop != prop);
        self.tracks.push(Track { prop, from, to, at: t, dur, ease });
    }
}

/// Steps every timeline on the clock; marks it changed only when a value
/// moved.
pub fn step_timelines(clock: Res<UiClock>, mut timelines: Query<&mut Timeline>) {
    for mut tl in &mut timelines {
        if tl.bypass_change_detection().step(clock.now) {
            tl.set_changed();
        }
    }
}

/// Translate, scale and size from changed timelines (module doc).
pub fn apply_timelines(
    units: Res<UiUnits>,
    mut nodes: Query<(&Timeline, Option<&mut UiTransform>, Option<&mut Node>), Changed<Timeline>>,
) {
    for (tl, transform, node) in &mut nodes {
        if let Some(mut tf) = transform {
            let mut next = *tf;
            if let Some(x) = tl.value(Prop::X) {
                next.translation.x = Val::Px(units.cq(x));
            }
            if let Some(y) = tl.value(Prop::Y) {
                next.translation.y = Val::Px(units.cq(y));
            }
            if let Some(s) = tl.value(Prop::ScaleX) {
                next.scale.x = s;
            }
            if let Some(s) = tl.value(Prop::ScaleY) {
                next.scale.y = s;
            }
            tf.set_if_neq(next);
        }
        if let Some(mut node) = node {
            if let Some(w) = tl.value(Prop::Width) {
                let v = Val::Px(units.snap(units.cq(w)));
                if node.width != v {
                    node.width = v;
                }
            }
            if let Some(h) = tl.value(Prop::Height) {
                let v = Val::Px(units.snap(units.cq(h)));
                if node.height != v {
                    node.height = v;
                }
            }
        }
    }
}

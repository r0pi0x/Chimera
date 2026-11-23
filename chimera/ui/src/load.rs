//! The kit's load and wait marks (the lobby's starting grid, the user's
//! review of 2026-11-22; docs/plans/active/ugg-ui.md "Lobby: the starting
//! grid"):
//!
//! - `Ui::load_line`: one player's or one job's load as a sheared bar
//!   filling the width it is given, its words at the right. Four looks
//!   (`Load`): **joining** (nothing to measure yet) an indeterminate bar,
//!   the whole track filled with dim blue slanted stripes sliding sideways
//!   (`LoadHatch`; the stripes are the track's own shader, drawn in the
//!   bar and nowhere else), with three blinking sheared squares
//!   (`LoadBlink`) before its word; **downloading** a static, white,
//!   finely striped half-translucent fill to its share over an empty
//!   track (not blue, not moving, not full width: told from joining at a
//!   glance); **loading** a white fill advancing; **ready** no bar at all,
//!   only "Ready", so the eye goes to whoever is still loading. The share
//!   is in the words ("Models 62%").
//! - `Ui::busy`: the waiting mark, a row of sheared squares with a lit
//!   head running along them and a fading trail (`BusyTick`), for
//!   anything that waits on something else (the lobby's start, a scan).
//!
//! All animate from `UiClock` with no rebuild (`step_loads` writes one
//! number a node: the stripes' offset, a square's alpha); with
//! `ReducedMotion` the stripes rest and the squares stay lit evenly.

use bevy::prelude::*;

use crate::{
    Role, Tone, UiClock, UiSlant, UiStyle,
    controls::Ui,
    scroll::ReducedMotion,
};

/// A load's state (`Ui::load_line`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Load {
    /// Connected, nothing to measure yet: the indeterminate bar.
    Joining,
    /// Fetching files, 0-1: the striped, half-translucent fill.
    Downloading(f32),
    /// Loading (the map, the models, the physics), 0-1: the white fill.
    Loading(f32),
    /// Done: no bar, the words only.
    Ready,
}

impl Load {
    /// The share done, when the look has one.
    pub fn share(self) -> Option<f32> {
        match self {
            Load::Downloading(f) | Load::Loading(f) => Some(f.clamp(0.0, 1.0)),
            _ => None,
        }
    }
}

/// A load line's root (its state, for tests).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LoadLine(pub Load);

/// A load line's bar (absent when ready).
#[derive(Component, Clone, Copy, Debug)]
pub struct LoadTrack;

/// A downloading bar's stripes' holder.
#[derive(Component, Clone, Copy, Debug)]
pub struct LoadStripes;

/// The indeterminate bar's stripes (a hatched `UiSlant` over the whole
/// track); `since` when they began.
#[derive(Component, Clone, Copy, Debug)]
pub struct LoadHatch {
    pub since: f64,
}

/// One of the joining mark's blinking squares (`i` of three); `since`
/// when it began.
#[derive(Component, Clone, Copy, Debug)]
pub struct LoadBlink {
    pub i: u32,
    pub since: f64,
}

/// One square of `Ui::busy` (`i` of `n`).
#[derive(Component, Clone, Copy, Debug)]
pub struct BusyTick {
    pub i: u32,
    pub n: u32,
}

/// The stripes' time to slide one period, s.
pub const HATCH_SECS: f64 = 1.1;
/// The stripes' period, cq (the bar is about .6 tall).
pub const HATCH_PERIOD: f32 = 1.2;
/// The stripes' lit share of a period.
pub const HATCH_LIT: f32 = 0.5;
/// The stripes' strength over the track.
pub const HATCH_ALPHA: f32 = 0.5;
/// A blink's length, s; each square starts `BLINK_STEP` after the last.
pub const BLINK_SECS: f64 = 1.0;
pub const BLINK_STEP: f64 = 0.2;
/// A busy mark's step from one square to the next, s.
pub const BUSY_STEP: f64 = 0.09;

/// The stripes' offset (a share of the period) `t` s after they began.
pub fn hatch_phase(t: f64, reduced: bool) -> f32 {
    if reduced { 0.0 } else { (t / HATCH_SECS).rem_euclid(1.0) as f32 }
}

/// Blinking square `i`'s alpha `t` s after it began: lit, then dim half
/// of each second, the squares a step apart.
pub fn blink_alpha(i: u32, t: f64, reduced: bool) -> f32 {
    if reduced {
        return 0.8;
    }
    let phase = ((t - i as f64 * BLINK_STEP) / BLINK_SECS).rem_euclid(1.0);
    if phase < 0.5 { 1.0 } else { 0.2 }
}

/// Square `i` of `n`'s alpha at `t`: the head full, a trail of three fading.
pub fn busy_alpha(i: u32, n: u32, t: f64, reduced: bool) -> f32 {
    if reduced || n == 0 {
        return 0.55;
    }
    let head = ((t / BUSY_STEP).floor() as i64).rem_euclid(n as i64 + 3) as i64;
    let behind = head - i as i64;
    match behind {
        0 => 1.0,
        1 => 0.6,
        2 => 0.35,
        _ => 0.15,
    }
}

impl Ui<'_> {
    /// A load line under `parent`, growing to its width: the bar
    /// (`bar_h` cq) then the words, `words_w` cq, at the right
    /// (module doc). The share is added to the words ("Models" becomes
    /// "Models 62%"). Returns the line.
    #[allow(clippy::too_many_arguments)]
    pub fn load_line(&self, c: &mut Commands, parent: Entity, load: Load, words: &str, now: f64, bar_h: f32, words_w: f32, size: f32) -> Entity {
        let line = c
            .spawn((
                UiStyle { gap: Some(0.9), grow: Some(1.0), shrink: Some(1.0), min_w: Some(0.0), ..UiStyle::row() },
                Node::default(),
                LoadLine(load),
                ChildOf(parent),
            ))
            .id();
        let full = || Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, right: Val::ZERO, ..default() };
        if load != Load::Ready {
            let track = c
                .spawn((
                    UiStyle { h: Some(bar_h), grow: Some(1.0), shrink: Some(1.0), min_w: Some(0.0), ..UiStyle::row() },
                    Node::default(),
                    UiSlant::new(Tone::Fg, 0.07),
                    LoadTrack,
                    ChildOf(line),
                ))
                .id();
            match load {
                Load::Joining => {
                    c.spawn((full(), UiSlant::new(Tone::AccText, HATCH_ALPHA).hatched(HATCH_PERIOD, HATCH_LIT, 0.0), LoadHatch { since: now }, Pickable::IGNORE, ChildOf(track)));
                }
                Load::Downloading(f) => {
                    let f = f.clamp(0.0, 1.0);
                    // Half-translucent under the stripes, cut on the slant.
                    c.spawn((full(), UiSlant::new(Tone::Fg, 0.22).clipped(0.0, 1.0 - f), Pickable::IGNORE, ChildOf(track)));
                    let clip = c
                        .spawn((
                            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: Val::Percent(f * 100.0), overflow: Overflow::clip(), ..default() },
                            LoadStripes,
                            Pickable::IGNORE,
                            ChildOf(track),
                        ))
                        .id();
                    // Fine stripes on the slant every .5 cq (the joining bar's are 1.2 apart), over the widest bar.
                    let lean = bar_h * self.theme.tan();
                    for k in 0..126 {
                        c.spawn((
                            Node { position_type: PositionType::Absolute, left: self.units.val(k as f32 * 0.5), top: Val::ZERO, bottom: Val::ZERO, width: self.units.val(0.2 + lean), ..default() },
                            UiSlant::new(Tone::Fg, 0.4),
                            Pickable::IGNORE,
                            ChildOf(clip),
                        ));
                    }
                }
                Load::Loading(f) => {
                    let f = f.clamp(0.0, 1.0);
                    if f > 0.0 {
                        c.spawn((full(), UiSlant::new(Tone::Fg, 1.0).clipped(0.0, 1.0 - f), Pickable::IGNORE, ChildOf(track)));
                    }
                }
                Load::Ready => {}
            }
        } else {
            self.boxed(c, line, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        }
        let text = match load.share() {
            Some(f) if words.is_empty() => format!("{:.0}%", f * 100.0),
            Some(f) => format!("{words} {:.0}%", f * 100.0),
            None => words.to_string(),
        };
        let w = self.boxed(c, line, UiStyle { w: Some(words_w), shrink: Some(0.0), justify: Some(JustifyContent::FlexEnd), ..UiStyle::row() });
        let (role, tone) = if load == Load::Ready { (Role::DisplayBold, Tone::Fg) } else { (Role::Mono, Tone::Fg2) };
        if load == Load::Joining {
            // The waiting mark: three sheared squares blinking in turn.
            let sq = self.boxed(c, w, UiStyle { gap: Some(0.18), shrink: Some(0.0), ..UiStyle::row() }.margin([0.0, 0.5, 0.0, 0.0]));
            let h = size * 0.7;
            let lean = h * self.theme.tan();
            for i in 0..3 {
                c.spawn((
                    UiStyle { w: Some(h + lean), h: Some(h), shrink: Some(0.0), ..UiStyle::row() },
                    Node::default(),
                    UiSlant::new(Tone::AccText, blink_alpha(i, 0.0, false)),
                    LoadBlink { i, since: now },
                    Pickable::IGNORE,
                    ChildOf(sq),
                ));
            }
        }
        self.text(c, w, role, size, tone, &text);
        line
    }

    /// The waiting mark under `parent`: `n` sheared squares `h` cq tall
    /// (module doc). Returns its row.
    pub fn busy(&self, c: &mut Commands, parent: Entity, n: u32, h: f32) -> Entity {
        let row = self.boxed(c, parent, UiStyle { gap: Some(h * 0.3), shrink: Some(0.0), ..UiStyle::row() });
        c.entity(row).insert(Name::new("busy"));
        let lean = h * self.theme.tan();
        for i in 0..n {
            c.spawn((
                UiStyle { w: Some(h + lean), h: Some(h), shrink: Some(0.0), ..UiStyle::row() },
                Node::default(),
                UiSlant::new(Tone::Fg, busy_alpha(i, n, 0.0, false)),
                BusyTick { i, n },
                Pickable::IGNORE,
                ChildOf(row),
            ));
        }
        row
    }
}

/// Moves the joining stripes, the blinking squares and the busy marks from
/// the clock (no rebuild).
#[allow(clippy::type_complexity)]
pub fn step_loads(
    clock: Res<UiClock>,
    reduced: Option<Res<ReducedMotion>>,
    mut hatches: Query<(&LoadHatch, &mut UiSlant), (Without<BusyTick>, Without<LoadBlink>)>,
    mut blinks: Query<(&LoadBlink, &mut UiSlant), (Without<BusyTick>, Without<LoadHatch>)>,
    mut ticks: Query<(&BusyTick, &mut UiSlant), (Without<LoadHatch>, Without<LoadBlink>)>,
) {
    let reduced = reduced.is_some_and(|r| r.0);
    for (h, mut slant) in &mut hatches {
        let phase = hatch_phase(clock.now - h.since, reduced);
        if slant.hatch.is_some_and(|(_, _, p)| p != phase) {
            slant.hatch = Some((HATCH_PERIOD, HATCH_LIT, phase));
        }
    }
    for (b, mut slant) in &mut blinks {
        let a = blink_alpha(b.i, clock.now - b.since, reduced);
        if slant.alpha != a {
            slant.alpha = a;
        }
    }
    for (t, mut slant) in &mut ticks {
        let a = busy_alpha(t.i, t.n, clock.now, reduced);
        if slant.alpha != a {
            slant.alpha = a;
        }
    }
}

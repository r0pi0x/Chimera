//! A trail key's points (`PolyTrail`; specs/combat_arms/effects_sounds.md
//! Follow-up (2026-10-31) 1, cases 77-81): the points laid every
//! `SectionInterval` while the key lives, each with its own age, texture
//! u and colour, and the one half-width `WidthStyle` gives the whole
//! strip. Pure logic: `fx` places the key and draws the strip as
//! Chimera's `Shape::Ribbon`.
//!
//! Known shortcuts (docs/tech-debt.md): only camera alignment (every
//! stock key's) is drawn; `Up`, `Right` and `Forward` alignment draw as
//! camera alignment.

use bevy::prelude::*;
use lithtech_formats::fxf::FxKey;

/// `WidthStyle`: how the half-width follows the number of points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WidthStyle {
    #[default]
    Constant,
    SmallToBig,
    SmallToSmall,
    BigToSmall,
}

impl WidthStyle {
    pub fn from_index(i: i32) -> Self {
        match i {
            1 => Self::SmallToBig,
            2 => Self::SmallToSmall,
            3 => Self::BigToSmall,
            _ => Self::Constant,
        }
    }

    /// The half-width (u) of a trail of width `w` and most points `max`
    /// holding `n`: one value for the whole strip.
    pub fn half_width(self, w: f32, max: usize, n: usize) -> f32 {
        let (max, grown) = (max as f32, n as f32 - 2.0);
        let v = match self {
            Self::Constant => w,
            Self::SmallToBig => w * grown / max,
            Self::SmallToSmall if (n as f32) < (max + 1.0) / 2.0 => w * grown / max,
            Self::SmallToSmall | Self::BigToSmall => w * (max - grown) / max,
        };
        v.max(0.0)
    }
}

/// A trail key's properties.
#[derive(Clone, Debug, PartialEq)]
pub struct TrailParams {
    /// `TrailWidth` (u): the texture repeats once per this much path.
    pub width: f32,
    pub style: WidthStyle,
    /// `TrailLen`: the most points.
    pub len: usize,
    /// `SectionLifespan` (s).
    pub lifespan: f32,
    /// `SectionInterval` (s).
    pub interval: f32,
    /// `UAdd`: texture repeats a second.
    pub u_add: f32,
    /// The key's colour keys (time, rgb + opacity).
    pub colors: Vec<(f32, [f32; 4])>,
    /// Metres per unit.
    pub unit: f32,
}

impl TrailParams {
    /// The key's properties at the editor defaults; none for a key with
    /// no points or no width (it is not created).
    pub fn from_key(k: &FxKey, unit: f32) -> Option<Self> {
        let len = k.int("TrailLen").unwrap_or(50).max(0) as usize;
        let width = k.float("TrailWidth").unwrap_or(20.0);
        (len > 0 && width > 0.0).then(|| Self {
            width,
            style: WidthStyle::from_index(k.combo("WidthStyle").unwrap_or(0)),
            len,
            lifespan: k.float("SectionLifespan").unwrap_or(1.0),
            interval: k.float("SectionInterval").unwrap_or(0.1),
            u_add: k.float("UAdd").unwrap_or(0.0),
            colors: k.color_keys(),
            unit,
        })
    }

    /// A section's colour at `age`: the colour keys over its own life,
    /// mod 1.
    pub fn color_at(&self, age: f32) -> Vec4 {
        let t = if self.lifespan > 0.0 { (age / self.lifespan).rem_euclid(1.0) } else { 0.0 };
        super::fx::color_at(&self.colors, t)
    }
}

/// One point of a trail.
#[derive(Clone, Debug, PartialEq)]
pub struct TrailPoint {
    /// Metres.
    pub at: Vec3,
    /// Texture u, before the scroll offset.
    pub u: f32,
    pub age: f32,
    /// rgb + opacity.
    pub color: Vec4,
}

/// How the key is this update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailMode {
    /// Alive: points are added and the newest follows the key.
    Active,
    /// Shutting down (smooth): no points added, none followed; the points
    /// age out.
    ShuttingDown,
}

/// A trail: its points (oldest first) and clocks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trail {
    pub points: Vec<TrailPoint>,
    elapsed: f32,
    offset: f32,
    /// The strip's half-width (u).
    half_width: f32,
    /// The number of points at the width step, before removal.
    counted: usize,
}

impl Trail {
    /// One update of `dt` seconds with the key at `at` (metres).
    pub fn update(&mut self, p: &TrailParams, dt: f32, at: Vec3, mode: TrailMode) {
        self.elapsed += dt;
        self.offset += p.u_add * dt;
        if mode == TrailMode::Active {
            let n = self.points.len();
            if n < p.len && (self.elapsed > p.interval || n == 1) {
                let u = self.points.last().map_or(0.0, |l| l.u + l.at.distance(at) / p.unit / p.width);
                self.points.push(TrailPoint {
                    at,
                    u,
                    age: 0.0,
                    color: Vec4::ONE,
                });
                self.elapsed = 0.0;
            }
            if let Some(newest) = self.points.last_mut() {
                newest.at = at;
            }
        }
        for pt in &mut self.points {
            pt.age += dt;
            pt.color = p.color_at(pt.age);
        }
        self.counted = self.points.len();
        self.half_width = p.style.half_width(p.width, p.len, self.counted);
        let last = self.points.len().saturating_sub(1);
        let mut i = 0;
        self.points.retain(|pt| {
            i += 1;
            i - 1 == last || pt.age < p.lifespan
        });
    }

    /// The key was hidden: every point goes and it starts over.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// How many points there were at the last update's width step, before
    /// the old ones were removed.
    pub fn counted(&self) -> usize {
        self.counted
    }

    /// The strip's half-width (metres).
    pub fn half_width_m(&self, unit: f32) -> f32 {
        self.half_width * unit
    }

    /// A point's texture u as drawn (the scroll offset added).
    pub fn drawn_u(&self, i: usize) -> f32 {
        self.points[i].u + self.offset
    }

    /// Whether anything is drawn (at least two points).
    pub fn drawn(&self) -> bool {
        self.points.len() >= 2
    }
}

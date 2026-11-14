//! Sizes (docs/plans/active/ugg-ui.md, 2.6 and 2.5 "Pixel snapping").
//!
//! The prototype measures in **cq**: 1 % of a 16:9 frame's width, 12.8 px
//! at 720 lines. The overlay is sized by the window's height (the shell's
//! `overlay::scale`, the same rule as `overlay_scale` here), so 1 cq =
//! 12.8 logical px times that scale: identical at 16:9, wider margins at
//! 21:9, the same layout at 4:3.
//!
//! `UiUnits` (a resource kept from the primary window each frame, or set
//! by hand in tests) converts: `cq` to logical px, `text_px` with the
//! theme's floor in physical pixels, `snap` to whole physical pixels,
//! `hairline` (1 physical px). `TickLayout` is the prototype's tick-bar
//! layout in physical pixels, so ticks never blur.

use bevy::{prelude::*, window::PrimaryWindow};

/// Logical px of 1 cq at 720 lines (1280 / 100).
pub const CQ_AT_720: f32 = 12.8;

/// The overlay's scale in a window `height` logical pixels tall: sizes
/// are for 720 lines, never under a third of it (the shell's
/// `overlay::scale`).
pub fn overlay_scale(height: f32) -> f32 {
    height.max(240.0) / 720.0
}

/// `v` cq in logical px in a window `height` logical px tall.
pub fn cq(v: f32, height: f32) -> f32 {
    v * CQ_AT_720 * overlay_scale(height)
}

/// A logical position or size rounded to whole physical pixels.
pub fn snap_px(logical: f32, scale_factor: f32) -> f32 {
    let sf = scale_factor.max(1e-3);
    (logical * sf).round() / sf
}

/// The window the UI is laid out in: its logical height and scale factor.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct UiUnits {
    /// Logical height.
    pub height: f32,
    /// Physical px per logical px.
    pub scale_factor: f32,
    /// No text under this many physical px (the theme's `min_text_px`).
    pub min_text_px: f32,
}

impl Default for UiUnits {
    fn default() -> Self {
        UiUnits { height: 720.0, scale_factor: 1.0, min_text_px: 9.0 }
    }
}

impl UiUnits {
    pub fn new(height: f32, scale_factor: f32) -> Self {
        UiUnits { height, scale_factor, ..default() }
    }

    /// The overlay scale.
    pub fn scale(&self) -> f32 {
        overlay_scale(self.height)
    }

    /// `v` cq in logical px.
    pub fn cq(&self, v: f32) -> f32 {
        cq(v, self.height)
    }

    /// `v` cq as a `Val`, snapped to physical pixels.
    pub fn val(&self, v: f32) -> Val {
        Val::Px(self.snap(self.cq(v)))
    }

    /// A text size of `v` cq in logical px: whole physical pixels (few
    /// glyph atlases), never under `min_text_px` physical.
    pub fn text_px(&self, v: f32) -> f32 {
        let phys = (self.cq(v) * self.scale_factor).round().max(self.min_text_px);
        phys / self.scale_factor.max(1e-3)
    }

    /// Logical px rounded to whole physical pixels.
    pub fn snap(&self, logical: f32) -> f32 {
        snap_px(logical, self.scale_factor)
    }

    /// One physical pixel in logical px (hairlines, never scaled).
    pub fn hairline(&self) -> f32 {
        1.0 / self.scale_factor.max(1e-3)
    }

    /// Logical px to physical.
    pub fn phys(&self, logical: f32) -> f32 {
        logical * self.scale_factor
    }
}

/// A tick bar's layout in physical pixels (the prototype's): `n` ticks in
/// a box `box_w` x `box_h` physical px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickLayout {
    /// Tick to tick.
    pub pitch: u32,
    /// A tick's width.
    pub tick: u32,
    /// Between ticks.
    pub gap: u32,
    /// The bar's width (no gap after the last tick).
    pub w: u32,
    /// A tick's height.
    pub h: u32,
}

impl TickLayout {
    /// `pitch = max(2, floor(box_w / n))`, `tick = max(1, round(pitch *
    /// .36))`, `w = n * pitch - gap`, `h = max(3, round(pitch * 1.5))`
    /// capped by the box (never under 3).
    pub fn new(box_w: u32, box_h: u32, n: u32) -> TickLayout {
        let n = n.max(1);
        let pitch = (box_w / n).max(2);
        let tick = ((pitch as f32 * 0.36).round() as u32).max(1);
        let gap = pitch - tick;
        let h = ((pitch as f32 * 1.5).round() as u32).max(3).min(box_h.max(3));
        TickLayout { pitch, tick, gap, w: n * pitch - gap, h }
    }
}

/// Keeps `UiUnits` from the primary window (its logical height and scale
/// factor) and the theme's text floor.
pub fn track_window(
    windows: Query<&Window, With<PrimaryWindow>>,
    theme: Option<Res<crate::theme::Theme>>,
    mut units: ResMut<UiUnits>,
) {
    let mut next = *units;
    if let Ok(w) = windows.single() {
        next.height = w.height();
        next.scale_factor = w.scale_factor();
    }
    if let Some(t) = theme {
        next.min_text_px = t.shape.min_text_px;
    }
    if next != *units {
        *units = next;
    }
}

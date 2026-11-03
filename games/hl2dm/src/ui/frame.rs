//! What a frame of the HUD draws, as a list (`Draw`), in window pixels:
//! the elements build it (`hud::Hl2Hud::frame`), `sync` turns it into
//! Bevy UI nodes (a pool of entities reused frame to frame, written only
//! on a change), and the tests read it, so the layout is checked without
//! a window.
//!
//! `Screen` is the window and its unit: one HUD unit is `h / 480` pixels
//! on both axes (specs/hl2dm/hud.md "Units and conventions"), a scaled
//! value is truncated to whole pixels, `r`/`c` anchors measure from the
//! right edge and the centre.

use bevy::prelude::*;
use chimera_world::map::hud::HudCoord;

/// The window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub w: f32,
    pub h: f32,
}

impl Screen {
    pub fn new(w: f32, h: f32) -> Screen {
        Screen { w, h }
    }

    /// 640 by 480: one unit a pixel.
    pub fn base() -> Screen {
        Screen::new(640.0, 480.0)
    }

    /// Pixels a unit takes.
    pub fn u(&self) -> f32 {
        self.h / 480.0
    }

    /// A length in units as whole pixels (truncated).
    pub fn scaled(&self, v: f32) -> f32 {
        (v * self.u()).trunc()
    }

    /// The virtual screen's width in units.
    pub fn units_wide(&self) -> f32 {
        self.w / self.u()
    }

    /// Pixels from the left edge of `c`, an x coordinate.
    pub fn x(&self, c: HudCoord) -> f32 {
        match c {
            HudCoord::Start(v) => self.scaled(v),
            HudCoord::Centre(v) => self.w / 2.0 + self.scaled(v),
            HudCoord::End(v) => self.w - self.scaled(v),
        }
    }

    /// Pixels from the top edge of `c`, a y coordinate.
    pub fn y(&self, c: HudCoord) -> f32 {
        match c {
            HudCoord::Start(v) => self.scaled(v),
            HudCoord::Centre(v) => self.h / 2.0 + self.scaled(v),
            HudCoord::End(v) => self.h - self.scaled(v),
        }
    }

    /// A box from layout units: left, top, wide, tall.
    pub fn rect(&self, x: HudCoord, y: HudCoord, wide: f32, tall: f32) -> Rect {
        let min = Vec2::new(self.x(x), self.y(y));
        Rect::from_corners(min, min + Vec2::new(self.scaled(wide), self.scaled(tall)))
    }
}

/// RGBA 0..255.
pub type Rgba = [u8; 4];

/// `c` with its alpha multiplied by `k` (0..1).
pub fn fade(c: Rgba, k: f32) -> Rgba {
    [c[0], c[1], c[2], (c[3] as f32 * k.clamp(0.0, 1.0)).round() as u8]
}

/// Where a text's anchor point sits (`hud_text::Align`).
pub use chimera_client::client::hud_text::Align;

/// One thing to draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// A filled box; `round`: the rounded box of `PaintBackgroundType 2`.
    Box { rect: Rect, color: Rgba, round: bool },
    /// A box fading across: alpha times `from` at its left edge to times
    /// `to` at its right (the damage flashes), added to the screen.
    Fade { rect: Rect, color: Rgba, from: f32, to: f32 },
    /// A circle's outline, one pixel wide.
    Ring { centre: Vec2, radius: f32, color: Rgba },
    /// Text in a scheme font (`font`, `tall` pixels), `at` the anchor.
    /// `clip`: only this rectangle of it shows.
    Text { font: &'static str, tall: f32, text: String, color: Rgba, at: Vec2, align: Align, clip: Option<Rect> },
    /// A rectangle of a sprite sheet (`sprite`, by `GameHud::sprites`
    /// key) drawn into `rect`, mirrored across when `flip`.
    Sprite { sprite: String, rect: Rect, color: Rgba, additive: bool, flip: [bool; 2] },
}

impl Draw {
    pub fn text(font: &'static str, tall: f32, text: impl Into<String>, color: Rgba, at: Vec2, align: Align) -> Draw {
        Draw::Text { font, tall, text: text.into(), color, at, align, clip: None }
    }

    /// This text showing only inside `rect`.
    pub fn clipped(self, rect: Rect) -> Draw {
        match self {
            Draw::Text { font, tall, text, color, at, align, .. } => Draw::Text { font, tall, text, color, at, align, clip: Some(rect) },
            other => other,
        }
    }

    /// The draw's text, when it is text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Draw::Text { text, .. } => Some(text),
            _ => None,
        }
    }
}

/// A frame's draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame(pub Vec<Draw>);

impl Frame {
    pub fn push(&mut self, d: Draw) {
        self.0.push(d);
    }

    /// The text draws, in order.
    pub fn texts(&self) -> impl Iterator<Item = &Draw> {
        self.0.iter().filter(|d| matches!(d, Draw::Text { .. }))
    }

    /// The first text draw whose text is `text`.
    pub fn find_text(&self, text: &str) -> Option<&Draw> {
        self.texts().find(|d| d.as_text() == Some(text))
    }

    /// The boxes (rounded or not), in order.
    pub fn boxes(&self) -> impl Iterator<Item = (&Rect, &Rgba)> {
        self.0.iter().filter_map(|d| match d {
            Draw::Box { rect, color, .. } => Some((rect, color)),
            _ => None,
        })
    }
}

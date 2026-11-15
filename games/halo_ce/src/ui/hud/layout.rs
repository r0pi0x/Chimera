//! Where Halo's HUD elements land (specs/halo_ce/hud.md, sections 1, 7
//! and 16), as pure functions of the design canvas: the safe frame of a
//! window, an element's anchor point and picture rectangle, and the glyph
//! positions of a number.
//!
//! **The canvas** (`Canvas`): the 640 by 480 design canvas scaled by the
//! window's height alone (u = height / 480, section 16's rule, ours): the
//! virtual canvas is `round(width / u)` units wide, the safe frame keeps
//! its 48 and 36 unit insets (x 48 to width - 48, y 36 to 444), left
//! anchors keep their offsets from the left safe edge, right anchors from
//! the right one, the centre stays central. A 4:3 window is the original
//! exactly; nothing here depends on Bevy's global UI scale (the nodes are
//! sized from the canvas, `Canvas::to_screen`).
//!
//! Rectangles are (left, top, right, bottom) in canvas units.

use blam_formats::hud::Anchor;

/// The design canvas's height (and a 4:3 window's width, 640).
pub const CANVAS_HEIGHT: i32 = 480;
pub const CANVAS_WIDTH: i32 = 640;
/// The safe frame's insets (section 1).
pub const SAFE_X: i32 = 48;
pub const SAFE_Y: i32 = 36;

/// A window's design canvas: its scale and its virtual width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Canvas {
    /// Screen pixels per canvas unit.
    pub scale: f32,
    /// The virtual canvas's width in units (640 for a 4:3 window).
    pub width: i32,
}

impl Canvas {
    /// The canvas of a `width` x `height` window (logical pixels).
    pub fn of(width: f32, height: f32) -> Canvas {
        let scale = (height / CANVAS_HEIGHT as f32).max(1e-3);
        Canvas { scale, width: (width / scale).round().max(1.0) as i32 }
    }

    /// The original's 640 by 480.
    pub const ORIGINAL: Canvas = Canvas { scale: 1.0, width: CANVAS_WIDTH };

    /// The safe frame (x0, y0, x1, y1).
    pub fn frame(&self) -> [i32; 4] {
        [SAFE_X, SAFE_Y, self.width - SAFE_X, CANVAS_HEIGHT - SAFE_Y]
    }

    /// Half the width added to the 640 canvas (what an element laid out
    /// on the whole 4:3 picture is shifted by to stay centred).
    pub fn extra_half(&self) -> i32 {
        (self.width - CANVAS_WIDTH) / 2
    }

    /// A canvas rectangle in screen pixels (left, top, width, height).
    pub fn to_screen(&self, r: [f32; 4]) -> [f32; 4] {
        [r[0] * self.scale, r[1] * self.scale, (r[2] - r[0]) * self.scale, (r[3] - r[1]) * self.scale]
    }
}

/// The anchor point of `offset` from `anchor` in `frame` (section 1, step
/// 1): a positive offset on a right or bottom anchor moves inward.
pub fn anchor_point(anchor: Anchor, offset: [i16; 2], frame: [i32; 4]) -> [i32; 2] {
    let [x0, y0, x1, y1] = frame;
    let (ox, oy) = (i32::from(offset[0]), i32::from(offset[1]));
    match anchor {
        Anchor::TopLeft => [x0 + ox, y0 + oy],
        Anchor::TopRight => [x1 - ox, y0 + oy],
        Anchor::BottomLeft => [x0 + ox, y1 - oy],
        Anchor::BottomRight => [x1 - ox, y1 - oy],
        Anchor::Center => [(x0 + x1).div_euclid(2) + ox, (y0 + y1).div_euclid(2) + oy],
    }
}

/// The bounds of a `size` picture about its anchor point on each axis
/// (section 1's table).
fn bounds(anchor: Anchor, w: f32, h: f32) -> ([f32; 2], [f32; 2]) {
    match anchor {
        Anchor::TopLeft => ([0.0, w], [0.0, h]),
        Anchor::TopRight => ([-w, 0.0], [0.0, h]),
        Anchor::BottomLeft => ([0.0, w], [-h, 0.0]),
        Anchor::BottomRight => ([-w, 0.0], [-h, 0.0]),
        Anchor::Center => ([-w / 2.0, w / 2.0], [-h / 2.0, h / 2.0]),
    }
}

/// The picture rectangle of a `size` picture at `point` by `anchor`'s
/// bounds, each bound times `scale` truncated toward zero (section 1,
/// step 2).
pub fn rect_at(anchor: Anchor, point: [i32; 2], size: [f32; 2], scale: [f32; 2]) -> [i32; 4] {
    let (bx, by) = bounds(anchor, size[0], size[1]);
    let t = |v: f32, s: f32| (v * s).trunc() as i32;
    [point[0] + t(bx[0], scale[0]), point[1] + t(by[0], scale[1]), point[0] + t(bx[1], scale[0]), point[1] + t(by[1], scale[1])]
}

/// An element's picture rectangle: `size` placed at `offset` from
/// `anchor` in `frame`, at `scale`.
pub fn rect(anchor: Anchor, offset: [i16; 2], size: [f32; 2], scale: [f32; 2], frame: [i32; 4]) -> [i32; 4] {
    rect_at(anchor, anchor_point(anchor, offset, frame), size, scale)
}

/// A number's value: an integer, or a real (the fractional numbers;
/// NaN draws nothing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Real(f32),
}

/// The glyphs a number element draws (section 7, "The number procedure"):
/// (glyph index into the digit sequence: 0 to 9, 10 ".", 12 "-", 13 "m",
/// 14 "Km"; the cursor x), right to left. `origin` is the anchor point,
/// `advance` the HUD number tag's screen width, `point` its decimal point
/// width. The glyph's sprite is drawn by `anchor`'s bounds at
/// (cursor, origin y).
#[allow(clippy::too_many_arguments)]
pub fn number_glyphs(
    value: Value,
    digits: i8,
    fractional: i8,
    leading_zeros: bool,
    metres: bool,
    anchor: Anchor,
    origin: [i32; 2],
    advance: i32,
    point: i32,
) -> Vec<(u8, i32)> {
    let digits = i32::from(digits.clamp(0, 9));
    let fractional = i32::from(fractional.clamp(0, 4));
    let (mut int, decimal) = match value {
        Value::Int(v) => (v, None),
        Value::Real(r) if r.is_nan() => return Vec::new(),
        Value::Real(r) => (r.trunc() as i64, Some(r)),
    };
    let mut decimal = decimal.filter(|_| fractional > 0).map(|r| {
        let ten_k = ((f64::from(r.abs()) * 10_000.0).round() as i64).rem_euclid(10_000);
        ten_k / 10_i64.pow((4 - fractional) as u32)
    });
    let km = metres && int > 999;
    if km {
        decimal = decimal.map(|_| (int % 1000) / 10_i64.pow(3u32.saturating_sub(fractional as u32)));
        int /= 1000;
    }
    let count = digits + decimal.map_or(0, |_| fractional + 1) + i32::from(metres);
    let fraction_room = if decimal.is_some() { point } else { 0 };
    let mut x = match anchor {
        Anchor::TopLeft | Anchor::BottomLeft => origin[0] + (count - 2) * advance + fraction_room,
        Anchor::Center => origin[0] + ((count - 1) * advance + point) / 2,
        Anchor::TopRight | Anchor::BottomRight => origin[0],
    };
    let mut out = Vec::new();
    if metres {
        out.push((if km { 14 } else { 13 }, x));
        x -= advance;
    }
    if let Some(mut d) = decimal.filter(|_| int >= 0) {
        for _ in 0..fractional {
            out.push(((d % 10) as u8, x));
            d /= 10;
            x -= advance;
        }
        x += advance - point;
        out.push((10, x));
        x -= advance;
    }
    let negative = int < 0;
    let mut v = int.abs() % 10_i64.pow(digits as u32);
    for _ in 0..digits {
        if v == 0 && !leading_zeros {
            break;
        }
        out.push(((v % 10) as u8, x));
        v /= 10;
        x -= advance;
    }
    if negative {
        out.push((12, x));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_window_keeps_the_height_and_moves_the_right_edge() {
        let c = Canvas::of(1920.0, 1080.0);
        assert_eq!(c.width, 853);
        assert!((c.scale - 2.25).abs() < 1e-6);
        assert_eq!(c.frame(), [48, 36, 805, 444]);
        assert_eq!(Canvas::of(1280.0, 960.0).frame(), [48, 36, 592, 444]);
    }
}

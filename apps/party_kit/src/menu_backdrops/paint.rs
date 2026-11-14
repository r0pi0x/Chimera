//! A small 2D painter for the menu's backdrops (`menu_backdrops`):
//! convex fills, linear and radial gradients, strokes and the kit's icons
//! as triangles with a colour per vertex, clipped by half-planes, in a
//! canvas's physical pixels (y down), into one mesh; texts and images as
//! items placed beside it. A gradient is exact: each of its bands is
//! clipped out of the shape and coloured at its corners (a band is
//! linear).

use bevy::{color::LinearRgba, math::Vec2};

/// Where a text sits on its x: its left, centre or right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Centre,
    Right,
}

/// What a text is drawn in.
#[derive(Clone, Debug, PartialEq)]
pub enum Face {
    /// A kit role (`chimera_ui::Role`).
    Role(chimera_ui::Role),
    /// A game's icon font (a kill icon's glyph).
    Font(bevy::prelude::Handle<bevy::prelude::Font>),
}

/// A text: its baseline's point, size in physical px, colour.
#[derive(Clone, Debug, PartialEq)]
pub struct TextItem {
    pub text: String,
    pub face: Face,
    pub px: f32,
    pub color: LinearRgba,
    pub at: Vec2,
    pub align: Align,
    /// Letter spacing in em.
    pub track: f32,
}

/// An image (a game's kill icon) in a box.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageItem {
    pub image: bevy::prelude::Handle<bevy::prelude::Image>,
    pub min: Vec2,
    pub size: Vec2,
    pub color: LinearRgba,
}

/// A half-plane kept: `n . p + c >= 0`.
#[derive(Clone, Copy, Debug)]
pub struct Half {
    pub n: Vec2,
    pub c: f32,
}

impl Half {
    fn at(&self, p: Vec2) -> f32 {
        self.n.dot(p) + self.c
    }

    /// Left of (or on) the line through `a` and `b` (y down: what lies
    /// on the right hand walking a to b in screen terms is dropped).
    pub fn left_of(a: Vec2, b: Vec2) -> Half {
        let d = b - a;
        let n = Vec2::new(d.y, -d.x);
        Half { n, c: -n.dot(a) }
    }

    /// The other side.
    pub fn flip(self) -> Half {
        Half { n: -self.n, c: -self.c }
    }
}

/// The painted frame (module doc).
#[derive(Default)]
pub struct Paint {
    pub pos: Vec<[f32; 3]>,
    pub col: Vec<[f32; 4]>,
    pub idx: Vec<u32>,
    pub texts: Vec<TextItem>,
    pub images: Vec<ImageItem>,
    clips: Vec<Half>,
    /// Multiplies every alpha (a group's opacity).
    pub alpha: f32,
    /// A sheen the composite lays over what is painted (`menu_backdrops.
    /// wesl`): across a band `t = x * s[0] + y * s[1] + s[2]` from 0 to
    /// 1 (screen px), at strength `s[3]`.
    pub sheen: Option<[f32; 4]>,
    /// Scratch polygons for clipping (no allocation a fill).
    scratch: (Vec<Vec2>, Vec<Vec2>),
}

/// `a` to `b` at `t`.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn mix(a: LinearRgba, b: LinearRgba, t: f32) -> LinearRgba {
    LinearRgba::new(
        lerp(a.red, b.red, t),
        lerp(a.green, b.green, t),
        lerp(a.blue, b.blue, t),
        lerp(a.alpha, b.alpha, t),
    )
}

/// Clips a convex polygon to a half-plane (Sutherland-Hodgman), `poly`
/// into `out`.
fn clip(poly: &[Vec2], h: Half, out: &mut Vec<Vec2>) {
    out.clear();
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (da, db) = (h.at(a), h.at(b));
        if da >= 0.0 {
            out.push(a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            let t = da / (da - db);
            out.push(a + (b - a) * t);
        }
    }
}

impl Paint {
    pub fn new() -> Paint {
        Paint {
            alpha: 1.0,
            ..Default::default()
        }
    }

    pub fn clear(&mut self) {
        self.pos.clear();
        self.col.clear();
        self.idx.clear();
        self.texts.clear();
        self.images.clear();
        self.clips.clear();
        self.alpha = 1.0;
        self.sheen = None;
    }

    /// Keeps only what lies in `h` until `pop`.
    pub fn push(&mut self, h: Half) {
        self.clips.push(h);
    }

    pub fn pop(&mut self, n: usize) {
        let k = self.clips.len().saturating_sub(n);
        self.clips.truncate(k);
    }

    /// Clips to a convex polygon (its corners in order, either way round).
    pub fn push_convex(&mut self, q: &[Vec2]) -> usize {
        let area: f32 = (0..q.len()).map(|i| q[i].perp_dot(q[(i + 1) % q.len()])).sum();
        for i in 0..q.len() {
            let h = Half::left_of(q[i], q[(i + 1) % q.len()]);
            self.push(if area > 0.0 { h.flip() } else { h });
        }
        q.len()
    }

    /// The clipped polygon, triangulated as a fan, coloured by `color`.
    pub fn fill_with(&mut self, pts: &[Vec2], color: impl Fn(Vec2) -> LinearRgba) {
        let (mut a, mut b) = std::mem::take(&mut self.scratch);
        a.clear();
        a.extend_from_slice(pts);
        let mut kept = true;
        for h in &self.clips {
            // Wholly inside: nothing to cut.
            if a.iter().all(|p| h.at(*p) >= 0.0) {
                continue;
            }
            clip(&a, *h, &mut b);
            std::mem::swap(&mut a, &mut b);
            if a.len() < 3 {
                kept = false;
                break;
            }
        }
        if kept {
            let base = self.pos.len() as u32;
            for p in &a {
                let mut c = color(*p);
                c.alpha *= self.alpha;
                self.pos.push([p.x, p.y, 0.0]);
                self.col.push([c.red, c.green, c.blue, c.alpha]);
            }
            for i in 1..a.len() as u32 - 1 {
                self.idx.extend([base, base + i, base + i + 1]);
            }
        }
        self.scratch = (a, b);
    }

    /// A flat fill.
    pub fn fill(&mut self, pts: &[Vec2], color: LinearRgba) {
        if color.alpha * self.alpha <= 0.0005 {
            return;
        }
        self.fill_with(pts, |_| color);
    }

    /// A linear gradient fill: `t = g . p + c`, stops by `t`.
    pub fn fill_grad(&mut self, pts: &[Vec2], g: Vec2, c: f32, stops: &[(f32, LinearRgba)]) {
        if stops.is_empty() {
            return;
        }
        let at = |t: f32| -> LinearRgba {
            if t <= stops[0].0 {
                return stops[0].1;
            }
            for w in stops.windows(2) {
                if t <= w[1].0 {
                    let k = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
                    return mix(w[0].1, w[1].1, k);
                }
            }
            stops[stops.len() - 1].1
        };
        let lin = |p: Vec2| g.dot(p) + c;
        let (tmin, tmax) = pts
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(lin(*p)), hi.max(lin(*p))));
        // Each band between stops (and the ends beyond) on its own; only
        // the bands the shape reaches, and a shape inside one band without
        // cutting.
        let n = stops.len();
        for i in 0..=n {
            let lo = if i == 0 { f32::NEG_INFINITY } else { stops[i - 1].0 };
            let hi = if i == n { f32::INFINITY } else { stops[i].0 };
            if tmax < lo || tmin > hi {
                continue;
            }
            // Skip invisible bands.
            let (a0, a1) = (at(lo.max(stops[0].0)).alpha, at(hi.min(stops[n - 1].0)).alpha);
            if a0 <= 0.0005 && a1 <= 0.0005 {
                continue;
            }
            if tmin >= lo && tmax <= hi {
                self.fill_with(pts, |p| at(lin(p)));
                return;
            }
            let mut pushed = 0;
            if lo.is_finite() && tmin < lo {
                self.push(Half { n: g, c: c - lo });
                pushed += 1;
            }
            if hi.is_finite() && tmax > hi {
                self.push(Half { n: -g, c: hi - c });
                pushed += 1;
            }
            self.fill_with(pts, |p| at(lin(p)));
            self.pop(pushed);
        }
    }

    /// A segment `w` px wide (square ends `ext` px beyond its points).
    pub fn line_ext(&mut self, a: Vec2, b: Vec2, w: f32, ext: f32, color: LinearRgba) {
        let d = (b - a).normalize_or_zero();
        if d == Vec2::ZERO {
            return;
        }
        let n = Vec2::new(-d.y, d.x) * (w * 0.5);
        let (a, b) = (a - d * ext, b + d * ext);
        self.fill(&[a + n, b + n, b - n, a - n], color);
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, w: f32, color: LinearRgba) {
        self.line_ext(a, b, w, 0.0, color);
    }

    /// A segment with its alpha along a gradient (`t = g . p + c`).
    pub fn line_grad(&mut self, a: Vec2, b: Vec2, w: f32, g: Vec2, c: f32, stops: &[(f32, LinearRgba)]) {
        let d = (b - a).normalize_or_zero();
        let n = Vec2::new(-d.y, d.x) * (w * 0.5);
        self.fill_grad(&[a + n, b + n, b - n, a - n], g, c, stops);
    }

    /// A closed outline.
    pub fn stroke(&mut self, pts: &[Vec2], w: f32, color: LinearRgba) {
        for i in 0..pts.len() {
            self.line_ext(pts[i], pts[(i + 1) % pts.len()], w, w * 0.5, color);
        }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: LinearRgba) {
        self.fill(
            &[
                Vec2::new(x, y),
                Vec2::new(x + w, y),
                Vec2::new(x + w, y + h),
                Vec2::new(x, y + h),
            ],
            color,
        );
    }

    /// A dashed rectangle's outline (`dash` on, `gap` off).
    pub fn dashed_rect(&mut self, x: f32, y: f32, w: f32, h: f32, lw: f32, dash: f32, gap: f32, color: LinearRgba) {
        let corners = [
            Vec2::new(x, y),
            Vec2::new(x + w, y),
            Vec2::new(x + w, y + h),
            Vec2::new(x, y + h),
        ];
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            let len = (b - a).length();
            let d = (b - a) / len.max(1e-6);
            let mut s = 0.0;
            while s < len {
                let e = (s + dash).min(len);
                self.line(a + d * s, a + d * e, lw, color);
                s += dash + gap;
            }
        }
    }

    /// Triangles (an icon's, in its units) placed by `map`.
    pub fn tris(&mut self, tris: &[[Vec2; 3]], map: impl Fn(Vec2) -> Vec2, color: LinearRgba) {
        if color.alpha * self.alpha <= 0.0005 {
            return;
        }
        for t in tris {
            self.fill(&[map(t[0]), map(t[1]), map(t[2])], color);
        }
    }

    /// Triangles filled with a linear gradient (`t = g . p + c`).
    pub fn tris_grad(
        &mut self,
        tris: &[[Vec2; 3]],
        map: impl Fn(Vec2) -> Vec2,
        g: Vec2,
        c: f32,
        stops: &[(f32, LinearRgba)],
    ) {
        for t in tris {
            self.fill_grad(&[map(t[0]), map(t[1]), map(t[2])], g, c, stops);
        }
    }

    /// A radial gradient about `centre`, `r` px out, stretched `sx` across;
    /// stops by the fraction of `r`.
    pub fn radial(&mut self, centre: Vec2, r: f32, sx: f32, stops: &[(f32, LinearRgba)], segments: usize) {
        let ring = |k: f32, i: usize| {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            centre + Vec2::new(a.cos() * sx, a.sin()) * (k * r)
        };
        for w in stops.windows(2) {
            let ((k0, c0), (k1, c1)) = (w[0], w[1]);
            if c0.alpha <= 0.0005 && c1.alpha <= 0.0005 {
                continue;
            }
            for i in 0..segments {
                let q = [ring(k0, i), ring(k0, i + 1), ring(k1, i + 1), ring(k1, i)];
                let (r0, r1) = (k0 * r, k1 * r);
                self.fill_with(&q, |p| {
                    let d = (p - centre) / Vec2::new(sx, 1.0);
                    let t = ((d.length() - r0) / (r1 - r0).max(1e-6)).clamp(0.0, 1.0);
                    mix(c0, c1, t)
                });
            }
        }
    }

    pub fn text(
        &mut self,
        text: impl Into<String>,
        face: Face,
        px: f32,
        color: LinearRgba,
        at: Vec2,
        align: Align,
        track: f32,
    ) {
        let mut color = color;
        color.alpha *= self.alpha;
        if color.alpha <= 0.002 {
            return;
        }
        self.texts.push(TextItem {
            text: text.into(),
            face,
            px: px.round().max(1.0),
            color,
            at,
            align,
            track,
        });
    }

    /// Triangles painted.
    pub fn triangles(&self) -> usize {
        self.idx.len() / 3
    }
}

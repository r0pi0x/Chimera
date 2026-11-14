//! Icons fitted by their visible ink: a game's kill icon and the kit's
//! fallback each carry their own padding (a bitmap's empty texels, a
//! glyph's place in its em and on its baseline, a fallback's rows on its
//! 16-unit grid), so laying out their boxes puts the guns at different
//! heights and sizes. Here every source's ink box is found and fitted
//! instead:
//!
//! - `image_ink`: an image's trimmed alpha bounds in texels (a texel counts
//!   from 10 % opacity, `INK_ALPHA`; `alpha_bounds` on plain RGBA);
//!   `image_lit_ink` counts only what shows light on a dark ground or
//!   tinted (opacity times its brightest channel from 10 %): a bitmap's
//!   dark outline or shadow (Combat Arms' kill icons have one) is padding
//!   there.
//! - `glyph_shape`: a font's glyph as flattened contours in em units (y
//!   down, the pen's origin on the baseline), its ink box and its
//!   triangles (`tessellate_nonzero`: the outline's nonzero fill, crossing
//!   contours included), so a glyph can be drawn as a mesh at any size.
//! - `icon_ink`: the kit's fallback icon's own bounds (its polygons').
//! - `fit_contain`: an ink box scaled to fit inside a box (keeping its
//!   aspect) and centred on it; `Fit::map` takes a point of the source.
//! - `IconInk`: the images' and glyphs' results cached by asset (a
//!   resource; `None` until the asset has loaded, then kept).
//!
//! Used by the menu's Ladder backdrop (`party_kit::menu_backdrops`); the
//! kill feed's rows find an image's ink rows with it
//! (`chimera_client::weapon_icons::ink_rows`).
//!
//! Known shortcuts: curves are flattened at fixed steps (8 a quadratic,
//! 12 a cubic), a glyph's ink box is its flattened points' (within a hair
//! of the curve's); a glyph is drawn unhinted at the font's default
//! variation.

use std::sync::Arc;

use bevy::{platform::collections::HashMap, prelude::*, render::render_resource::TextureFormat};
use skrifa::{
    MetadataProvider,
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
};

use crate::icons::Icon;

/// The opacity from which a texel is ink (10 %).
pub const INK_ALPHA: u8 = 26;

/// The box of the texels of a `w` x `h` RGBA image (row-major, 4 bytes a
/// texel) whose alpha is at least `min_alpha`: (min, one past max), or
/// none when no texel is.
pub fn alpha_bounds(w: u32, h: u32, rgba: &[u8], min_alpha: u8) -> Option<URect> {
    texel_bounds(w, h, rgba, |p| p[3] >= min_alpha)
}

/// The box of the texels of a `w` x `h` RGBA image that show light: their
/// opacity times their brightest channel at least `min` (of 255).
pub fn lit_bounds(w: u32, h: u32, rgba: &[u8], min: u8) -> Option<URect> {
    texel_bounds(w, h, rgba, |p| p[3] as u32 * p[0].max(p[1]).max(p[2]) as u32 >= min as u32 * 255)
}

fn texel_bounds(w: u32, h: u32, rgba: &[u8], ink: impl Fn(&[u8]) -> bool) -> Option<URect> {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || rgba.len() < w * h * 4 {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for y in 0..h {
        let row = &rgba[y * w * 4..(y + 1) * w * 4];
        let Some(first) = row.chunks_exact(4).position(&ink) else { continue };
        let last = row.chunks_exact(4).rposition(&ink).unwrap_or(first);
        x0 = x0.min(first);
        x1 = x1.max(last + 1);
        y0 = y0.min(y);
        y1 = y + 1;
    }
    (x0 != usize::MAX).then(|| URect::new(x0 as u32, y0 as u32, x1 as u32, y1 as u32))
}

/// `image`'s ink box in texels (module doc); none for an image whose
/// pixels aren't here as 8-bit RGBA, or with no ink.
pub fn image_ink(image: &Image) -> Option<Rect> {
    image_bounds(image, alpha_bounds)
}

/// `image`'s lit ink box in texels (`lit_bounds`, module doc), else its
/// alpha ink box (a dark icon); none as `image_ink`.
pub fn image_lit_ink(image: &Image) -> Option<Rect> {
    image_bounds(image, lit_bounds).or_else(|| image_ink(image))
}

fn image_bounds(image: &Image, find: fn(u32, u32, &[u8], u8) -> Option<URect>) -> Option<Rect> {
    if !matches!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb) {
        return None;
    }
    let r = find(image.width(), image.height(), image.data.as_deref()?, INK_ALPHA)?;
    Some(Rect::new(r.min.x as f32, r.min.y as f32, r.max.x as f32, r.max.y as f32))
}

/// The kit's fallback icon's ink box in its units (its polygons' bounds;
/// the whole icon when it has none).
pub fn icon_ink(icon: &Icon) -> Rect {
    points_ink(icon.shapes.iter().flatten().flatten().map(|p| Vec2::from(*p)))
        .unwrap_or(Rect::new(0.0, 0.0, icon.w, icon.h))
}

/// The bounds of `points`; none for none.
pub fn points_ink(points: impl IntoIterator<Item = Vec2>) -> Option<Rect> {
    let mut it = points.into_iter();
    let first = it.next()?;
    let (lo, hi) = it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p)));
    Some(Rect::from_corners(lo, hi))
}

/// An ink box placed in a box: a source point `p` lands at
/// `offset + p * scale` (`map`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub scale: f32,
    pub offset: Vec2,
}

impl Fit {
    /// Where source point `p` lands.
    pub fn map(&self, p: Vec2) -> Vec2 {
        self.offset + p * self.scale
    }

    /// Where a source rectangle lands.
    pub fn rect(&self, r: Rect) -> Rect {
        Rect::from_corners(self.map(r.min), self.map(r.max))
    }
}

/// `ink` (source units) scaled to fit inside a box `size` (keeping its
/// aspect: as large as fits) and centred on `centre`.
pub fn fit_contain(ink: Rect, centre: Vec2, size: Vec2) -> Fit {
    let s = ink.size().max(Vec2::splat(1e-6));
    let scale = (size.x / s.x).min(size.y / s.y);
    Fit {
        scale,
        offset: centre - ink.center() * scale,
    }
}

/// A glyph's outline (module doc): contours in em units, y down, the pen's
/// origin on the baseline; their ink box; the nonzero fill as triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphShape {
    pub contours: Vec<Vec<Vec2>>,
    pub ink: Rect,
    pub tris: Vec<[Vec2; 3]>,
}

/// `ch` of the font in `data` (TrueType or OpenType, the first face) as
/// a `GlyphShape`; none when the font doesn't parse, maps no glyph for
/// it or the glyph has no outline.
pub fn glyph_shape(data: &[u8], ch: char) -> Option<GlyphShape> {
    let font = skrifa::FontRef::from_index(data, 0).ok()?;
    let gid = font.charmap().map(ch)?;
    let glyph = font.outline_glyphs().get(gid)?;
    let upem = font.metrics(Size::unscaled(), LocationRef::default()).units_per_em.max(1) as f32;
    let mut pen = Flatten {
        k: 1.0 / upem,
        contours: Vec::new(),
        at: Vec2::ZERO,
    };
    glyph.draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut pen).ok()?;
    let contours: Vec<Vec<Vec2>> = pen.contours.into_iter().filter(|c| c.len() >= 3).collect();
    let ink = points_ink(contours.iter().flatten().copied())?;
    let tris = tessellate_nonzero(&contours);
    Some(GlyphShape { contours, ink, tris })
}

/// Collects an outline as polygons (em units, y down).
struct Flatten {
    k: f32,
    contours: Vec<Vec<Vec2>>,
    at: Vec2,
}

impl Flatten {
    fn pt(&self, x: f32, y: f32) -> Vec2 {
        Vec2::new(x * self.k, -y * self.k)
    }
    fn push(&mut self, p: Vec2) {
        if let Some(c) = self.contours.last_mut()
            && c.last() != Some(&p)
        {
            c.push(p);
        }
        self.at = p;
    }
}

impl OutlinePen for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.contours.push(vec![p]);
        self.at = p;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.push(p);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (a, c, b) = (self.at, self.pt(cx0, cy0), self.pt(x, y));
        for i in 1..=8 {
            let t = i as f32 / 8.0;
            let u = 1.0 - t;
            self.push(a * u * u + c * 2.0 * u * t + b * t * t);
        }
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (a, c0, c1, b) = (self.at, self.pt(cx0, cy0), self.pt(cx1, cy1), self.pt(x, y));
        for i in 1..=12 {
            let t = i as f32 / 12.0;
            let u = 1.0 - t;
            self.push(a * u * u * u + c0 * 3.0 * u * u * t + c1 * 3.0 * u * t * t + b * t * t * t);
        }
    }
    fn close(&mut self) {
        if let Some(c) = self.contours.last_mut()
            && c.len() > 1
            && c.first() == c.last()
        {
            c.pop();
        }
    }
}

/// Triangles covering the nonzero fill of `contours` (closed polygons, any
/// direction, crossing allowed): cut into horizontal slabs at every
/// vertex's height and every crossing of two edges, so within a slab no
/// edges cross; there the spans of nonzero winding are trapezoids, two
/// triangles each.
pub fn tessellate_nonzero(contours: &[Vec<Vec2>]) -> Vec<[Vec2; 3]> {
    // Edges top to bottom, with the winding they add.
    let mut edges: Vec<(Vec2, Vec2, i32)> = Vec::new();
    let mut ys: Vec<f32> = Vec::new();
    for c in contours {
        let n = c.len();
        for i in 0..n {
            let (a, b) = (c[i], c[(i + 1) % n]);
            ys.push(a.y);
            if a.y < b.y {
                edges.push((a, b, 1));
            } else if a.y > b.y {
                edges.push((b, a, -1));
            }
        }
    }
    let x_at = |e: &(Vec2, Vec2, i32), y: f32| e.0.x + (y - e.0.y) * (e.1.x - e.0.x) / (e.1.y - e.0.y);
    // Where edges cross.
    for i in 0..edges.len() {
        for j in i + 1..edges.len() {
            let (e, f) = (&edges[i], &edges[j]);
            let (y0, y1) = (e.0.y.max(f.0.y), e.1.y.min(f.1.y));
            if y1 <= y0 {
                continue;
            }
            let (d0, d1) = (x_at(e, y0) - x_at(f, y0), x_at(e, y1) - x_at(f, y1));
            if (d0 < 0.0 && d1 > 0.0) || (d0 > 0.0 && d1 < 0.0) {
                ys.push(y0 + (y1 - y0) * d0 / (d0 - d1));
            }
        }
    }
    ys.sort_by(f32::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let mut out = Vec::new();
    let mut cross: Vec<(f32, f32, f32, i32)> = Vec::new();
    for w in ys.windows(2) {
        let (y0, y1) = (w[0], w[1]);
        let ym = (y0 + y1) / 2.0;
        cross.clear();
        for e in &edges {
            if e.0.y <= ym && e.1.y >= ym {
                cross.push((x_at(e, ym), x_at(e, y0), x_at(e, y1), e.2));
            }
        }
        cross.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut wind = 0;
        let mut left: Option<(f32, f32, f32, i32)> = None;
        for &c in &cross {
            let was = wind;
            wind += c.3;
            if was == 0 && wind != 0 {
                left = Some(c);
            } else if was != 0 && wind == 0
                && let Some(l) = left.take()
            {
                let (a, b, cc, d) = (Vec2::new(l.1, y0), Vec2::new(c.1, y0), Vec2::new(c.2, y1), Vec2::new(l.2, y1));
                if b.x - a.x > 1e-7 {
                    out.push([a, b, cc]);
                }
                if cc.x - d.x > 1e-7 {
                    out.push([a, cc, d]);
                }
            }
        }
    }
    out
}

/// The images' ink boxes and the glyphs' shapes by asset (module doc).
#[derive(Resource, Default)]
pub struct IconInk {
    images: HashMap<AssetId<Image>, Option<Rect>>,
    glyphs: HashMap<(AssetId<Font>, char), Option<Arc<GlyphShape>>>,
}

/// What `IconInk` found for an asset.
#[derive(Clone, Debug, PartialEq)]
pub enum Ink<T> {
    /// The asset hasn't loaded yet: ask again.
    Pending,
    /// Loaded, but no ink was found (its pixels aren't here, no glyph).
    None,
    Found(T),
}

impl IconInk {
    /// `image`'s ink box in texels as it shows on a dark ground or tinted
    /// (`image_lit_ink`).
    pub fn image(&mut self, images: &Assets<Image>, image: &Handle<Image>) -> Ink<Rect> {
        let id = image.id();
        let found = match self.images.get(&id) {
            Some(r) => *r,
            None => {
                let Some(img) = images.get(id) else { return Ink::Pending };
                let r = image_lit_ink(img);
                self.images.insert(id, r);
                r
            }
        };
        found.map_or(Ink::None, Ink::Found)
    }

    /// `ch` of `font` as a shape (`glyph_shape`).
    pub fn glyph(&mut self, fonts: &Assets<Font>, font: &Handle<Font>, ch: char) -> Ink<Arc<GlyphShape>> {
        let key = (font.id(), ch);
        let found = match self.glyphs.get(&key) {
            Some(g) => g.clone(),
            None => {
                let Some(f) = fonts.get(font.id()) else { return Ink::Pending };
                let g = glyph_shape(f.data.data(), ch).map(Arc::new);
                self.glyphs.insert(key, g.clone());
                g
            }
        };
        found.map_or(Ink::None, Ink::Found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(tris: &[[Vec2; 3]]) -> f32 {
        tris.iter().map(|t| ((t[1] - t[0]).perp_dot(t[2] - t[0]) / 2.0).abs()).sum()
    }

    fn square(x: f32, y: f32, s: f32, ccw: bool) -> Vec<Vec2> {
        let mut v = vec![Vec2::new(x, y), Vec2::new(x + s, y), Vec2::new(x + s, y + s), Vec2::new(x, y + s)];
        if ccw {
            v.reverse();
        }
        v
    }

    #[test]
    fn nonzero_fill_unions_overlaps_and_keeps_holes() {
        // Two overlapping squares wound alike: their union (4 + 4 - 1).
        let tris = tessellate_nonzero(&[square(0.0, 0.0, 2.0, false), square(1.0, 1.0, 2.0, false)]);
        assert!((area(&tris) - 7.0).abs() < 1e-4, "{}", area(&tris));
        // A square with an opposite-wound square inside: a hole.
        let tris = tessellate_nonzero(&[square(0.0, 0.0, 4.0, false), square(1.0, 1.0, 2.0, true)]);
        assert!((area(&tris) - 12.0).abs() < 1e-4, "{}", area(&tris));
        // A bow tie (crossing edges of one contour): two triangles of 1.
        let bow = vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 2.0), Vec2::new(2.0, 0.0), Vec2::new(0.0, 2.0)];
        assert!((area(&tessellate_nonzero(&[bow])) - 2.0).abs() < 1e-4);
    }

    #[test]
    fn alpha_bounds_trims_padding() {
        let (w, h) = (8u32, 6u32);
        let mut px = vec![0u8; (w * h * 4) as usize];
        for (x, y) in [(2, 1), (5, 3), (3, 2)] {
            px[((y * w + x) * 4 + 3) as usize] = 255;
        }
        // Too faint to count.
        px[((5 * w) * 4 + 3) as usize] = INK_ALPHA - 1;
        assert_eq!(alpha_bounds(w, h, &px, INK_ALPHA), Some(URect::new(2, 1, 6, 4)));
        assert_eq!(alpha_bounds(w, h, &vec![0; px.len()], INK_ALPHA), None);
        // Opaque black around a white texel: an outline, not lit ink.
        let mut px = vec![0u8; (w * h * 4) as usize];
        for y in 1..5 {
            for x in 1..7 {
                px[((y * w + x) * 4 + 3) as usize] = 255;
            }
        }
        px[((2 * w + 3) * 4) as usize..((2 * w + 3) * 4 + 4) as usize].copy_from_slice(&[255, 255, 255, 255]);
        assert_eq!(alpha_bounds(w, h, &px, INK_ALPHA), Some(URect::new(1, 1, 7, 5)));
        assert_eq!(lit_bounds(w, h, &px, INK_ALPHA), Some(URect::new(3, 2, 4, 3)));
    }
}

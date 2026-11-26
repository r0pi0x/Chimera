//! The faces' lightmap blocks packed into one atlas (specs/goldsrc/bsp.md,
//! "Lightmaps"): each lit face's block of w × h RGB samples, with
//! a border of one sample repeated around it so filtering never reads a
//! neighbour, placed by a shelf packer in a fixed order (deterministic);
//! a white sample for faces without a lightmap (drawn fullbright, bsp.md
//! "Edge cases"). Sample (i, j) sits at texel (16 (floor(s_min/16) + i),
//! 16 (floor(t_min/16) + j)), so a point's place in its block is
//! s/16 − floor(s_min/16) samples (`Atlas::uv`).
//!
//! Light styles (bsp.md "Lightmaps", entities.md "light styles"): a face
//! lit by several styles has a block per style; the atlas holds their sum
//! at map start (style 0, the animated presets at full and the
//! switchable lights that start on, `StyleStart`), and each other style's
//! share of it as a `MapLightStyle` (its texels, border included), so the
//! renderer adds or takes it away as the logic switches it
//! (`map::LightStyles`) or its pattern plays.
//!
//! The drawn light (bsp.md, "Drawn light"): the atlas holds the raw sums
//! (stored byte / 255 times each style's letter scale, `m` = 1: the
//! spec's 22 per letter step relative to `m`, which is
//! `map::style_levels`), and `MapLightmap::curve` (`map::LightCurve`,
//! CS 1.6's gamma 2.5, brightness 1, lightgamma 2.5, gl_overbright 0)
//! turns each sum into the lightmap texel: the gain 2^(1/lightgamma) on
//! the style sum with the 1023 clamp, lightgamma, the brightness knee,
//! 1/gamma, /4 to a byte. The renderer applies it once per sample when
//! it builds the images and again for the blocks whose style changes
//! (`render::light_styles`), always on the sum, never per style.
//!
//! Known shortcuts:
//! - A switchable light's own `pattern` key is not in the atlas: the style
//!   starts on (its baked light); the logic plays the pattern at run time
//!   (`map::LightStyles::patterns`).
//! - bsp.md's table gives L 67 for stored 50 at brightness 0; the formula
//!   (the identity there, k = 272) gives 68, which this follows.
//! - The curve's cvars are CS 1.6's shipped values, fixed at convert
//!   time (the spec rebuilds every lightmap when one changes; here a
//!   change needs a new conversion), and dynamic lights add nothing to
//!   the sum. GoldSrc's own `gl_overbright 1` and the software renderer
//!   are not done (bsp.md, open question 1). The curved texel multiplies
//!   the texture in Chimera's linear light, where the original multiplies
//!   the bytes (the same as for every game's lightmap here).

use bevy::math::Vec2;

use crate::map::{MapLightStyle, MapLightmap};

/// Atlas width, samples.
const WIDTH: u32 = 1024;

/// A block's place in the atlas: its first sample's texel (border
/// excluded) and size.
#[derive(Clone, Copy, Debug, Default)]
pub struct Place {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// A block's samples per style beside style 0: (style, samples).
type Styled = Vec<(u8, Vec<[f32; 3]>)>;

/// Blocks waiting to be packed.
#[derive(Default)]
pub struct Atlas {
    blocks: Vec<(u32, u32, Vec<[f32; 3]>, Styled)>,
}

/// The packed atlas: the map's lightmap and each block's place.
pub struct Packed {
    pub map: MapLightmap,
    pub places: Vec<Place>,
}

/// How a light style starts (`Atlas::finish`): lit, and its brightness per
/// step if it animates (empty: it only switches).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StyleStart {
    pub on: bool,
    pub pattern: Vec<f32>,
}

impl StyleStart {
    /// Its brightness in the atlas at map start (`MapLightStyle::start`).
    fn level(&self) -> f32 {
        if !self.pattern.is_empty() || self.on { 1.0 } else { 0.0 }
    }
}

fn samples(w: u32, h: u32, rgb: &[u8]) -> Vec<[f32; 3]> {
    let mut out: Vec<[f32; 3]> = rgb
        .chunks_exact(3)
        .take((w * h) as usize)
        .map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0])
        .collect();
    out.resize((w * h) as usize, [0.0; 3]);
    out
}

impl Atlas {
    /// Add a block of `w` × `h` RGB bytes (rows along t); its index.
    pub fn add(&mut self, w: u32, h: u32, rgb: &[u8]) -> usize {
        self.add_styled(w, h, rgb, &[0])
    }

    /// Add a face's blocks of `w` × `h` RGB bytes, one per style of
    /// `styles` in order (bsp.md: back to back); its index.
    pub fn add_styled(&mut self, w: u32, h: u32, rgb: &[u8], styles: &[u8]) -> usize {
        let len = (w * h * 3) as usize;
        let mut base = None;
        let mut other = Vec::new();
        for (k, &style) in styles.iter().enumerate() {
            let block = samples(w, h, rgb.get(k * len..).unwrap_or(&[]));
            if style == 0 && base.is_none() {
                base = Some(block);
            } else {
                other.push((style, block));
            }
        }
        self.blocks.push((w, h, base.unwrap_or_else(|| vec![[0.0; 3]; (w * h) as usize]), other));
        self.blocks.len() - 1
    }

    /// A one-sample white block (fullbright faces); its index.
    pub fn white(&mut self) -> usize {
        self.blocks.push((1, 1, vec![[1.0; 3]], Vec::new()));
        self.blocks.len() - 1
    }

    /// Pack every block: tallest first (ties by index), left to right in
    /// shelves; the styles other than 0 start as `start` says (module doc).
    pub fn finish(self, start: &dyn Fn(u8) -> StyleStart) -> Packed {
        let mut order: Vec<usize> = (0..self.blocks.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(self.blocks[i].1), i));
        let mut places = vec![Place::default(); self.blocks.len()];
        let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
        for &i in &order {
            let (w, h, _, _) = &self.blocks[i];
            let (bw, bh) = (w + 2, h + 2);
            if x + bw > WIDTH {
                x = 0;
                y += shelf;
                shelf = 0;
            }
            places[i] = Place { x: x + 1, y: y + 1, w: *w, h: *h };
            x += bw;
            shelf = shelf.max(bh);
        }
        let height = (y + shelf).max(1);
        let mut rgb = vec![[0.0f32; 3]; (WIDTH * height) as usize];
        // Each style's share, by style number (deterministic order).
        let mut styles: std::collections::BTreeMap<u8, (StyleStart, MapLightStyle)> = Default::default();
        for (i, (w, h, base, other)) in self.blocks.iter().enumerate() {
            let p = places[i];
            let (bx0, by0) = (p.x - 1, p.y - 1);
            let at = |samples: &[[f32; 3]], bx: u32, by: u32| {
                let sx = bx.saturating_sub(1).min(w - 1);
                let sy = by.saturating_sub(1).min(h - 1);
                samples.get((sy * w + sx) as usize).copied().unwrap_or([1.0; 3])
            };
            for (style, samples) in other {
                let (s, ls) = styles.entry(*style).or_insert_with(|| {
                    let s = start(*style);
                    let ls = MapLightStyle { style: *style, on: s.on, pattern: s.pattern.clone(), ..Default::default() };
                    (s, ls)
                });
                let _ = s;
                ls.rects.push([bx0, by0, w + 2, h + 2]);
                for by in 0..h + 2 {
                    for bx in 0..w + 2 {
                        ls.texels.push((by0 + by) * WIDTH + bx0 + bx);
                        ls.rgb.push(at(samples, bx, by));
                    }
                }
            }
            for by in 0..h + 2 {
                for bx in 0..w + 2 {
                    let mut v = at(base, bx, by);
                    for (style, samples) in other {
                        let level = styles[style].0.level();
                        let add = at(samples, bx, by);
                        for k in 0..3 {
                            v[k] += level * add[k];
                        }
                    }
                    rgb[((by0 + by) * WIDTH + bx0 + bx) as usize] = v;
                }
            }
        }
        Packed {
            map: MapLightmap {
                width: WIDTH,
                height,
                rgb,
                bumped: None,
                curve: Some(crate::map::LightCurve::default()),
                styles: styles.into_values().map(|(_, s)| s).collect(),
            },
            places,
        }
    }
}

impl Packed {
    /// The atlas coordinate (0..1) of a point `luxel` samples into block
    /// `block` (sample centres at whole numbers).
    pub fn uv(&self, block: usize, luxel: Vec2) -> [f32; 2] {
        let p = self.places[block];
        let lx = luxel.x.clamp(0.0, (p.w - 1) as f32);
        let ly = luxel.y.clamp(0.0, (p.h - 1) as f32);
        [
            (p.x as f32 + lx + 0.5) / self.map.width as f32,
            (p.y as f32 + ly + 0.5) / self.map.height as f32,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_land_whole_with_borders() {
        let mut a = Atlas::default();
        let b = a.add(2, 1, &[255, 0, 0, 0, 255, 0]);
        let w = a.white();
        let p = a.finish(&|_| StyleStart::default());
        let place = p.places[b];
        let at = |x: u32, y: u32| p.map.rgb[(y * p.map.width + x) as usize];
        assert_eq!(at(place.x, place.y), [1.0, 0.0, 0.0]);
        assert_eq!(at(place.x + 1, place.y), [0.0, 1.0, 0.0]);
        // The border repeats the edge.
        assert_eq!(at(place.x - 1, place.y - 1), [1.0, 0.0, 0.0]);
        let wp = p.places[w];
        assert_eq!(at(wp.x, wp.y), [1.0; 3]);
        // The curve rides along (bsp.md "Drawn light").
        assert_eq!(p.map.curve, Some(crate::map::LightCurve::default()));
        let uv = p.uv(b, Vec2::new(1.0, 0.0));
        assert!((uv[0] * p.map.width as f32 - (place.x as f32 + 1.5)).abs() < 1e-3);
    }

    #[test]
    fn other_styles_are_shares_of_the_start() {
        let mut a = Atlas::default();
        // One sample lit by style 0 (0.2) and style 32 (0.4), on at start.
        let b = a.add_styled(1, 1, &[51, 51, 51, 102, 102, 102], &[0, 32]);
        let dark = a.add_styled(1, 1, &[0, 0, 0, 102, 102, 102], &[0, 33]);
        let p = a.finish(&|s| StyleStart { on: s == 32, pattern: Vec::new() });
        let at = |x: u32, y: u32| p.map.rgb[(y * p.map.width + x) as usize];
        let (place, d) = (p.places[b], p.places[dark]);
        assert!((at(place.x, place.y)[0] - 0.6).abs() < 1e-5);
        assert_eq!(at(d.x, d.y)[0], 0.0);
        assert_eq!(p.map.styles.len(), 2);
        let s32 = &p.map.styles[0];
        assert_eq!((s32.style, s32.on, s32.texels.len(), s32.rects.len()), (32, true, 9, 1));
        assert!((s32.rgb[4][0] - 0.4).abs() < 1e-5);
        assert!(!p.map.styles[1].on);
    }

    #[test]
    fn the_curve_matches_the_specs_table() {
        let c = crate::map::LightCurve::default();
        // bsp.md "Drawn light": stored (one style at m) -> L at CS 1.6's values.
        for (stored, l) in [
            (0, 0), (8, 15), (16, 31), (23, 45), (29, 56), (32, 62), (45, 88), (50, 98), (64, 116),
            (96, 146), (128, 182), (160, 221), (187, 254), (188, 255), (255, 255),
        ] {
            assert_eq!(c.level(stored as f32), l, "stored {stored}");
            assert_eq!(c.texel(stored as f32 / 255.0), l as f32 / 255.0);
        }
        // Brightness 0 column (the spec's table says 67 for stored 50; at
        // b = 0 the curve is the identity, k = 272, L = 68: its own
        // 'about 1.36 x stored' agrees).
        let flat = crate::map::LightCurve { brightness: 0.0, ..c.clone() };
        for (stored, l) in [(8, 10), (50, 68), (128, 174)] {
            assert_eq!(flat.level(stored as f32), l, "b = 0, stored {stored}");
        }
        // The sum goes through once: 40 + 60 at m, and 60 at a.
        assert_eq!(c.level(100.0), 151);
        assert_eq!(c.level(40.0), 78);
        assert_eq!(c.level(40.0 + 60.0 * 0.0), 78);
        // One style at z: clamped.
        assert_eq!(c.level(100.0 * 25.0 / 12.0), 255);
        assert_eq!(c.gain(), 338.0);
    }

    #[test]
    fn a_style_change_uses_the_same_curve() {
        let mut a = Atlas::default();
        let b = a.add_styled(1, 1, &[40, 40, 40, 60, 60, 60], &[0, 33]);
        let p = a.finish(&|_| StyleStart { on: true, pattern: Vec::new() });
        let place = p.places[b];
        let raw = p.map.rgb[(place.y * p.map.width + place.x) as usize];
        let c = p.map.curve.as_ref().unwrap();
        assert_eq!((c.texel(raw[0]) * 255.0).round(), 151.0, "both at m");
        let (relit, _) = p.map.relit(&|s| if s.style == 33 { 0.0 } else { s.start() });
        let at = relit[(place.y * p.map.width + place.x) as usize];
        assert_eq!((at[0] * 255.0).round(), 78.0, "style 33 at a");
    }
}

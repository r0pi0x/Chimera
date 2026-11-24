//! The faces' lightmap blocks packed into one atlas (specs/goldsrc/bsp.md,
//! "Lightmaps"): each lit face's style-0 block of w × h RGB samples, with
//! a border of one sample repeated around it so filtering never reads a
//! neighbour, placed by a shelf packer in a fixed order (deterministic);
//! a white sample for faces without a lightmap (drawn fullbright, bsp.md
//! "Edge cases"). Sample (i, j) sits at texel (16 (floor(s_min/16) + i),
//! 16 (floor(t_min/16) + j)), so a point's place in its block is
//! s/16 − floor(s_min/16) samples (`Atlas::uv`).
//!
//! Known shortcuts:
//! - Only style 0 is read: the stock maps have no other (bsp.md).
//! - The stored values are used as linear light, byte/255
//!   (`MapLook::light_scale` 1): the original's brightening is open
//!   (bsp.md, open question 1), to be matched by captures.

use bevy::math::Vec2;

use crate::map::MapLightmap;

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

/// Blocks waiting to be packed.
#[derive(Default)]
pub struct Atlas {
    blocks: Vec<(u32, u32, Vec<[f32; 3]>)>,
}

/// The packed atlas: the map's lightmap and each block's place.
pub struct Packed {
    pub map: MapLightmap,
    pub places: Vec<Place>,
}

impl Atlas {
    /// Add a block of `w` × `h` RGB bytes (rows along t); its index.
    pub fn add(&mut self, w: u32, h: u32, rgb: &[u8]) -> usize {
        let samples = rgb
            .chunks_exact(3)
            .take((w * h) as usize)
            .map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0])
            .collect();
        self.blocks.push((w, h, samples));
        self.blocks.len() - 1
    }

    /// A one-sample white block (fullbright faces); its index.
    pub fn white(&mut self) -> usize {
        self.blocks.push((1, 1, vec![[1.0; 3]]));
        self.blocks.len() - 1
    }

    /// Pack every block: tallest first (ties by index), left to right in
    /// shelves.
    pub fn finish(self) -> Packed {
        let mut order: Vec<usize> = (0..self.blocks.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(self.blocks[i].1), i));
        let mut places = vec![Place::default(); self.blocks.len()];
        let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
        for &i in &order {
            let (w, h, _) = &self.blocks[i];
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
        for (i, (w, h, samples)) in self.blocks.iter().enumerate() {
            let p = places[i];
            for by in 0..h + 2 {
                for bx in 0..w + 2 {
                    let sx = bx.saturating_sub(1).min(w - 1);
                    let sy = by.saturating_sub(1).min(h - 1);
                    let v = samples.get((sy * w + sx) as usize).copied().unwrap_or([1.0; 3]);
                    rgb[((p.y - 1 + by) * WIDTH + p.x - 1 + bx) as usize] = v;
                }
            }
        }
        Packed {
            map: MapLightmap {
                width: WIDTH,
                height,
                rgb,
                bumped: None,
                styles: Vec::new(),
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
        let p = a.finish();
        let place = p.places[b];
        let at = |x: u32, y: u32| p.map.rgb[(y * p.map.width + x) as usize];
        assert_eq!(at(place.x, place.y), [1.0, 0.0, 0.0]);
        assert_eq!(at(place.x + 1, place.y), [0.0, 1.0, 0.0]);
        // The border repeats the edge.
        assert_eq!(at(place.x - 1, place.y - 1), [1.0, 0.0, 0.0]);
        let wp = p.places[w];
        assert_eq!(at(wp.x, wp.y), [1.0; 3]);
        let uv = p.uv(b, Vec2::new(1.0, 0.0));
        assert!((uv[0] * p.map.width as f32 - (place.x as f32 + 1.5)).abs() < 1e-3);
    }
}

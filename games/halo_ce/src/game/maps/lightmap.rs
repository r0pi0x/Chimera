//! The level's lightmap pages packed into the engine's single lightmap
//! atlas (`MapLightmap`; specs/halo_ce/structure_bsp.md, "Lightmaps";
//! shaders.md, "Lightmap colour and gamma").
//!
//! - Each lit lightmap group has one page (a 16-bit colour image of the
//!   lightmap bitmap, decoded by `blam_formats::bitmap`). `Atlas` lays the
//!   pages out on shelves (tallest first, left to right) with a border of
//!   one texel copied from each page's edge, so bilinear filtering at a
//!   page's edge reads the edge itself, as the game's clamped sampler
//!   does, never a neighbour.
//! - Coordinates: a lightmap vertex's (u, v) in 0..1 of its page, (0, 0)
//!   the first texel of the first stored row, texel i's centre at
//!   (i + 1/2) / size, which is the engine's convention too, so no
//!   half-texel shift is needed: the atlas coordinate is (place + 1 +
//!   u x size) / atlas size (`Packed::uv`), landing on the same texel
//!   centres. The disc's coordinates stay in 0.0019..0.998 (half a texel
//!   in), so nothing reaches the border but the bilinear footprint.
//! - Brightness: the game multiplies the base texture by the lightmap
//!   texel as stored, no x2, no gamma conversion, on a gamma-encoded
//!   display (shaders.md). The engine multiplies the linear texture by a
//!   linear light and encodes the result as sRGB. A stored byte b is
//!   therefore taken as sRGB and decoded (`light_value`): then
//!   srgb(linear(tex) x linear(b)) is close to tex x b for every pair,
//!   as Combat Arms does for its gamma-space fixed-function lighting
//!   (with the map's look untonemapped, `MapLook::tonemapping` off, and
//!   `light_scale` 1: 1.0 is the texture at its own brightness).
//! - Texels outside every page stay `UNWRITTEN` (the smallest positive
//!   float, black to the eye), which no decoded byte gives, so a test can
//!   tell that no triangle reads one.

use bevy::prelude::default;

use crate::map::MapLightmap;

/// The widest atlas the converter makes (a GPU's usual 2D limit).
pub const ATLAS_MAX: u32 = 8192;

/// The value of the atlas texels no page covers.
pub const UNWRITTEN: [f32; 3] = [f32::from_bits(1); 3];

/// A stored lightmap byte as the engine's linear light: the byte as an
/// sRGB value, decoded (module doc).
pub fn light_value(byte: u8) -> f32 {
    let c = byte as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// One page: `width` x `height` linear RGB texels, top row first.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<[f32; 3]>,
}

impl Page {
    /// A page from RGBA8 texels (the bitmap decoder's), each byte through
    /// `light_value`.
    pub fn from_rgba8(width: u32, height: u32, rgba: &[u8]) -> Page {
        let lut: [f32; 256] = std::array::from_fn(|b| light_value(b as u8));
        Page {
            width,
            height,
            rgb: rgba
                .as_chunks::<4>()
                .0
                .iter()
                .map(|t| [lut[t[0] as usize], lut[t[1] as usize], lut[t[2] as usize]])
                .collect(),
        }
    }
}

/// Pages gathered for one atlas.
#[derive(Clone, Debug, Default)]
pub struct Atlas {
    pages: Vec<Page>,
}

/// The laid-out atlas: the engine's lightmap and where each page went.
#[derive(Clone, Debug)]
pub struct Packed {
    pub map: MapLightmap,
    /// Each page's top-left corner, its border included.
    pub places: Vec<(u32, u32)>,
    /// Each page's size (without the border).
    pub sizes: Vec<(u32, u32)>,
}

impl Atlas {
    /// Add a page; its index for `Packed::uv`.
    pub fn add(&mut self, page: Page) -> usize {
        self.pages.push(page);
        self.pages.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Shelf places (tallest first) at `width`, and the height used.
    fn shelves(&self, width: u32) -> (Vec<(u32, u32)>, u32) {
        let mut order: Vec<usize> = (0..self.pages.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse((self.pages[i].height, self.pages[i].width)), i));
        let mut places = vec![(0u32, 0u32); self.pages.len()];
        let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
        for &i in &order {
            let (w, h) = (self.pages[i].width + 2, self.pages[i].height + 2);
            if x > 0 && x + w > width {
                (x, y, shelf) = (0, y + shelf, 0);
            }
            places[i] = (x, y);
            x += w;
            shelf = shelf.max(h);
        }
        (places, (y + shelf).max(1).next_multiple_of(4))
    }

    /// Lay the pages out: the narrowest power-of-two width (at least the
    /// widest page and its border) whose shelves are no taller than it is
    /// wide, up to `ATLAS_MAX`.
    pub fn finish(self) -> Packed {
        let widest = self.pages.iter().map(|p| p.width + 2).max().unwrap_or(1);
        let mut width = widest.next_power_of_two().max(4);
        let (places, height) = loop {
            let (places, height) = self.shelves(width);
            if height <= width || width >= ATLAS_MAX {
                break (places, height);
            }
            width *= 2;
        };
        let mut rgb = vec![UNWRITTEN; (width * height) as usize];
        for (page, &(px, py)) in self.pages.iter().zip(&places) {
            let (w, h) = (page.width as usize, page.height as usize);
            if w == 0 || h == 0 {
                continue;
            }
            for ty in 0..h + 2 {
                // The source row, its first and last texels repeated.
                let sy = ty.saturating_sub(1).min(h - 1);
                let row = &page.rgb[sy * w..(sy + 1) * w];
                let at = (py as usize + ty) * width as usize + px as usize;
                rgb[at] = row[0];
                rgb[at + 1..at + 1 + w].copy_from_slice(row);
                rgb[at + w + 1] = row[w - 1];
            }
        }
        Packed {
            sizes: self.pages.iter().map(|p| (p.width, p.height)).collect(),
            places,
            map: MapLightmap {
                width,
                height,
                rgb,
                ..default()
            },
        }
    }
}

impl Packed {
    /// A coordinate of page `page` (0..1 of the page) as atlas
    /// coordinates (module doc: the same texel centres).
    pub fn uv(&self, page: usize, uv: [f32; 2]) -> [f32; 2] {
        let (x, y) = self.places[page];
        let (w, h) = self.sizes[page];
        [
            (x as f32 + 1.0 + uv[0] * w as f32) / self.map.width as f32,
            (y as f32 + 1.0 + uv[1] * h as f32) / self.map.height as f32,
        ]
    }

    /// The texel at (x, y) of the atlas.
    pub fn texel(&self, x: u32, y: u32) -> [f32; 3] {
        self.map.rgb[(y * self.map.width + x) as usize]
    }
}

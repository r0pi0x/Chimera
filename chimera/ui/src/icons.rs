//! The kit's icons (docs/plans/active/ugg-ui.md, section 4): the fallback
//! gun set (13 classes: pistol ... blunt) and the marks (the UGG mark,
//! crown, trophy, kill, death, warn), as polygon data in
//! `data/icons.toml`, our own design.
//!
//! - `Icons::shipped()`: the data. An icon is `w` x `h` units and a list of
//!   shapes; a shape's rings fill even-odd (holes are rings inside rings),
//!   shapes union.
//! - `rasterize(icon, px)`: a coverage image `px` physical pixels tall
//!   (scanlines: 8 rows a pixel, exact coverage across), so an icon is
//!   crisp at every size; `IconImages` caches one `Image` per (icon, px),
//!   drawn by an `ImageNode` tinted by the theme.
//! - `tessellate(icon)`: triangles (trapezoids between the vertices'
//!   heights, holes left out) for meshes (the menu's Assembly backdrop).
//! - `IconFor`: a gun class's fallback icon (`for_class`), a melee
//!   weapon's by its slot (`for_melee`), a weapon's data may name one.
//!   Which icon a place shows (plan section 6, item 1, as revised): each
//!   game's own kill icon wherever that game has one, our fallback only
//!   where none; `choose` applies the theme's `icons.prefer_game` (on in
//!   Signal; off: the fallback set everywhere), the caller (the shell,
//!   which knows the games' `WeaponIcons`) says whether a game icon
//!   exists.
//!
//! Known shortcuts: icons rasterize on the main thread when first asked
//! for (a few hundred microseconds at HUD sizes), not on a task.

use std::collections::BTreeMap;

use bevy::{
    asset::RenderAssetUsages,
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use serde::Deserialize;

/// The shipped icon data.
pub const ICONS_TOML: &str = include_str!("../data/icons.toml");

/// One icon (module doc).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Icon {
    /// Its display name (`Pistol`).
    pub name: String,
    pub w: f32,
    pub h: f32,
    /// Shapes of rings of `[x, y]` (y down).
    pub shapes: Vec<Vec<Vec<[f32; 2]>>>,
}

impl Icon {
    /// Width over height.
    pub fn aspect(&self) -> f32 {
        self.w / self.h
    }
}

/// Every icon by key (`pistol`, `mark`).
#[derive(Clone, Debug, PartialEq)]
pub struct Icons(pub BTreeMap<String, Icon>);

impl Icons {
    pub fn shipped() -> Icons {
        Icons::from_toml(ICONS_TOML).expect("chimera_ui's data/icons.toml parses")
    }

    pub fn from_toml(text: &str) -> Result<Icons, String> {
        toml::from_str::<BTreeMap<String, Icon>>(text).map(Icons).map_err(|e| format!("icons: {e}"))
    }

    pub fn get(&self, key: &str) -> Option<&Icon> {
        self.0.get(key)
    }
}

/// The fallback gun icons' keys, in the set's order.
pub const GUNS: [&str; 13] = [
    "pistol", "revolver", "smg", "rifle", "lmg", "shotgun", "sniper", "launcher", "energy", "crossbow", "grenade", "knife",
    "blunt",
];

/// Which fallback icon a weapon shows.
pub struct IconFor;

impl IconFor {
    /// A gun class's icon (`GunClass` names, lower case: `heavy` ->
    /// `lmg`, `carbine` -> `rifle`, `heavy_pistol` -> `revolver`,
    /// `marksman` -> `sniper`, `flamethrower` -> `launcher`); an icon's own
    /// key passes through; unknown: `rifle`.
    pub fn for_class(class: &str) -> &'static str {
        let c = class.to_ascii_lowercase();
        match c.as_str() {
            "heavy" | "machine_gun" | "machinegun" => "lmg",
            "carbine" | "assault_rifle" => "rifle",
            "heavy_pistol" => "revolver",
            "marksman" | "sniper_rifle" => "sniper",
            "flamethrower" | "rocket" | "rocket_launcher" => "launcher",
            "submachine_gun" => "smg",
            "melee" => "knife",
            other => GUNS.into_iter().find(|g| *g == other).unwrap_or("rifle"),
        }
    }

    /// A `GunClass`'s icon.
    pub fn gun_class(class: chimera_core::game_parts::GunClass) -> &'static str {
        use chimera_core::game_parts::GunClass as G;
        match class {
            G::Pistol => "pistol",
            G::HeavyPistol => "revolver",
            G::Smg => "smg",
            G::Carbine | G::Rifle => "rifle",
            G::Heavy => "lmg",
            G::Shotgun => "shotgun",
            G::Marksman | G::Sniper => "sniper",
            G::Launcher | G::Flamethrower => "launcher",
            G::Grenade => "grenade",
        }
    }

    /// A melee weapon's icon: the knife in the knife slot, else blunt.
    pub fn for_melee(knife_slot: bool) -> &'static str {
        if knife_slot { "knife" } else { "blunt" }
    }
}

/// What a place draws for a weapon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconSource {
    /// The weapon's game's own kill icon (`WeaponIcons`, the shell's).
    Game,
    /// Ours (`IconFor`).
    Fallback,
}

/// The game's own icon where it has one and the theme prefers it, else
/// ours (module doc).
pub fn choose(prefer_game: bool, game_has_icon: bool) -> IconSource {
    if prefer_game && game_has_icon { IconSource::Game } else { IconSource::Fallback }
}

/// Rows a pixel the rasterizer samples.
const SUB_ROWS: usize = 8;

/// `icon` as coverage (0-255), `px` pixels tall and as wide as its aspect
/// (rounded up): (width, height, row-major coverage).
pub fn rasterize(icon: &Icon, px: u32) -> (u32, u32, Vec<u8>) {
    let h = px.max(1);
    let scale = h as f32 / icon.h;
    let w = (icon.w * scale).ceil().max(1.0) as u32;
    let mut acc = vec![0f32; (w * h) as usize];
    let mut spans: Vec<(f32, f32)> = Vec::new();
    let mut xs: Vec<f32> = Vec::new();
    for y in 0..h {
        for s in 0..SUB_ROWS {
            let sy = (y as f32 + (s as f32 + 0.5) / SUB_ROWS as f32) / scale;
            spans.clear();
            for rings in &icon.shapes {
                xs.clear();
                for r in rings {
                    let n = r.len();
                    for i in 0..n {
                        let (a, b) = (r[(i + n - 1) % n], r[i]);
                        if (a[1] > sy) != (b[1] > sy) {
                            xs.push((a[0] + (sy - a[1]) * (b[0] - a[0]) / (b[1] - a[1])) * scale);
                        }
                    }
                }
                xs.sort_by(f32::total_cmp);
                for pair in xs.chunks_exact(2) {
                    spans.push((pair[0], pair[1]));
                }
            }
            // Union of the shapes' spans.
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut merged: Vec<(f32, f32)> = Vec::new();
            for &(a, b) in spans.iter() {
                match merged.last_mut() {
                    Some(m) if a <= m.1 => m.1 = m.1.max(b),
                    _ => merged.push((a, b)),
                }
            }
            let row = &mut acc[(y * w) as usize..((y + 1) * w) as usize];
            for (a, b) in merged {
                let (a, b) = (a.max(0.0), b.min(w as f32));
                if b <= a {
                    continue;
                }
                let (ia, ib) = (a.floor() as usize, (b.ceil() as usize).min(w as usize));
                for (x, cell) in row.iter_mut().enumerate().take(ib).skip(ia) {
                    let cov = (b.min(x as f32 + 1.0) - a.max(x as f32)).max(0.0);
                    *cell += cov / SUB_ROWS as f32;
                }
            }
        }
    }
    let data = acc.into_iter().map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8).collect();
    (w, h, data)
}

/// `rasterize` as a white `Image` whose alpha is the coverage (tint it
/// with the `ImageNode`'s colour).
pub fn image(icon: &Icon, px: u32) -> Image {
    let (w, h, cov) = rasterize(icon, px);
    let mut rgba = Vec::with_capacity(cov.len() * 4);
    for c in cov {
        rgba.extend_from_slice(&[255, 255, 255, c]);
    }
    Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Rasterized icons by (key, physical px), made on first use.
#[derive(Resource, Default)]
pub struct IconImages {
    icons: Option<Icons>,
    cache: HashMap<(String, u32), Handle<Image>>,
}

impl IconImages {
    /// `key`'s image `px` physical pixels tall; `None` for an unknown key.
    pub fn get(&mut self, images: &mut Assets<Image>, key: &str, px: u32) -> Option<Handle<Image>> {
        if let Some(h) = self.cache.get(&(key.to_string(), px)) {
            return Some(h.clone());
        }
        let icons = self.icons.get_or_insert_with(Icons::shipped);
        let icon = icons.get(key)?;
        let h = images.add(image(icon, px));
        self.cache.insert((key.to_string(), px), h.clone());
        Some(h)
    }

    /// The icon data.
    pub fn icons(&mut self) -> &Icons {
        self.icons.get_or_insert_with(Icons::shipped)
    }
}

/// Triangles covering `icon` (module doc), in its units: each shape cut
/// into horizontal slabs at its vertices' heights; within a slab no edges
/// cross (rings never do), so its even-odd spans are trapezoids, two
/// triangles each. Exact for any rings, holes included. Shapes that
/// overlap give overlapping triangles (one colour: no matter).
pub fn tessellate(icon: &Icon) -> Vec<[Vec2; 3]> {
    let mut out = Vec::new();
    for rings in &icon.shapes {
        let mut edges: Vec<(Vec2, Vec2)> = Vec::new();
        let mut ys: Vec<f32> = Vec::new();
        for r in rings {
            let n = r.len();
            for i in 0..n {
                let (a, b) = (Vec2::from(r[i]), Vec2::from(r[(i + 1) % n]));
                ys.push(a.y);
                if a.y != b.y {
                    edges.push(if a.y < b.y { (a, b) } else { (b, a) });
                }
            }
        }
        ys.sort_by(f32::total_cmp);
        ys.dedup();
        let x_at = |(a, b): (Vec2, Vec2), y: f32| a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y);
        let mut cross: Vec<(f32, f32, f32)> = Vec::new();
        for w in ys.windows(2) {
            let (y0, y1) = (w[0], w[1]);
            let ym = (y0 + y1) / 2.0;
            cross.clear();
            for &e in &edges {
                if e.0.y <= y0 && e.1.y >= y1 {
                    cross.push((x_at(e, ym), x_at(e, y0), x_at(e, y1)));
                }
            }
            cross.sort_by(|a, b| a.0.total_cmp(&b.0));
            for pair in cross.chunks_exact(2) {
                let (l, r) = (pair[0], pair[1]);
                let (a, b, c, d) = (Vec2::new(l.1, y0), Vec2::new(r.1, y0), Vec2::new(r.2, y1), Vec2::new(l.2, y1));
                if b.x - a.x > 1e-6 {
                    out.push([a, b, c]);
                }
                if c.x - d.x > 1e-6 {
                    out.push([a, c, d]);
                }
            }
        }
    }
    out
}

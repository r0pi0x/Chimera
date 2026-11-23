//! Runtime decals (bullet holes, knife slashes) for any game, as data:
//! the map carries decal definitions grouped by name (`MapDecals`), which
//! `render::decal` projects onto what a `PlaceDecal` message hits;
//! how high above surfaces impacts lie (`LIFT`).
//!
//! Runtime decals for any game as data: `MapDecals` (decal images in atlas
//! rectangles, weighted groups, surface material → group, players' spray
//! logos by name), the impacts' lift; `Spray`, a player's spray where the
//! simulation put it (`weapon::spray`), drawn by `render::spray`.

use std::collections::BTreeMap;

use bevy::prelude::*;

/// One decal image: a rectangle of a texture (often a decal atlas) and its
/// size on surfaces; how it meets the frame (`blend`), its colour and
/// strength (each placed decal draws its own between the bounds), how long
/// it lasts, and the group placed with it (a chain).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MapDecal {
    /// Index into `MapData::textures`.
    pub texture: usize,
    pub uv_min: Vec2,
    pub uv_max: Vec2,
    /// Width and height, meters.
    pub size: Vec2,
    pub blend: DecalBlend,
    /// How fog fades it toward neutral (no change to the surface): by the
    /// fog amount raised to this power. Its game's rule: Source's 0.4
    /// (`SOURCE_FOG_FADE`, specs/cs_source/shaders.md 5, DecalModulate),
    /// Halo's 1, linearly (`LINEAR_FOG_FADE`, specs/halo_ce/shaders.md
    /// "Fade and fog stage").
    pub fog_fade: f32,
    /// Colour bounds (lower, upper; the game's gamma-space factors): a
    /// placed decal's texel is multiplied by lower + (upper - lower) x a
    /// draw (specs/halo_ce/effects.md "Decals", placement step 6). White
    /// for most games' (no change).
    pub tint: [Vec3; 2],
    /// Strength bounds (Halo's intensity, the colour's alpha): a placed
    /// decal draws one and is moved toward its blend's neutral value by 1
    /// minus it, as by fog (specs/halo_ce/shaders.md "Fade and fog stage").
    pub intensity: Vec2,
    /// Lifetime bounds, seconds (0: until the map changes or the limit
    /// takes it); the last `decay` seconds of it fade it out.
    pub lifetime: Vec2,
    pub decay: Vec2,
    /// A group placed right after it at the same spot (Halo's "next decal
    /// in chain": the plasma burn's glows).
    pub then: Option<String>,
}

impl Default for MapDecal {
    fn default() -> Self {
        Self {
            texture: 0,
            uv_min: Vec2::ZERO,
            uv_max: Vec2::ONE,
            size: Vec2::ONE,
            blend: DecalBlend::Modulate2x,
            fog_fade: 0.0,
            tint: [Vec3::ONE; 2],
            intensity: Vec2::ONE,
            lifetime: Vec2::ZERO,
            decay: Vec2::ZERO,
            then: None,
        }
    }
}

impl MapDecal {
    /// Whether every placed copy looks the same and lasts (one shared
    /// material serves them all).
    pub fn uniform(&self) -> bool {
        self.tint[0] == self.tint[1] && self.intensity.x == self.intensity.y && self.lifetime == Vec2::ZERO
    }
}

/// Source's decals fade toward neutral in fog by the fog amount^0.4
/// (`MapDecal::fog_fade`).
pub const SOURCE_FOG_FADE: f32 = 0.4;
/// Halo's fade stage moves a decal toward its blend's neutral value by the
/// fog amount itself (`MapDecal::fog_fade`).
pub const LINEAR_FOG_FADE: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DecalBlend {
    /// The surface times 2 x texel (0.5 grey leaves it unchanged; Halo's
    /// "double multiply").
    Modulate2x,
    /// Alpha-blended decals (lit by the surface in the game); drawn as a
    /// stain for now.
    Alpha,
    /// The surface times the texel (white leaves it).
    Multiply,
    /// The texel added to the surface (black leaves it).
    Add,
    /// The texel taken from the surface (black leaves it).
    Subtract,
    /// The darker of texel and surface per channel (white leaves it).
    Min,
    /// The lighter of texel and surface per channel (black leaves it;
    /// Halo's alien blood).
    Max,
}

/// The map's runtime decals: definitions, named groups of weighted
/// variants, and the group a surface material letter takes when shot.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MapDecals {
    pub decals: Vec<MapDecal>,
    /// Group name -> (weight, index into `decals`).
    pub groups: BTreeMap<String, Vec<(f32, usize)>>,
    /// Surface game material letter -> group name.
    pub by_material: BTreeMap<char, String>,
    /// Players' spray logos (CS:S's `materials/vgui/logos/*.vtf`) by
    /// lower-case name (`spray_kilroy`) -> index into `decals`, at the size
    /// a spray is drawn.
    pub logos: BTreeMap<String, usize>,
}

impl MapDecals {
    /// The group `game`'s impact on a surface of game material `letter`
    /// takes: `game`'s own (`impact::own_material_group`, a guest's whose
    /// letter the map took), else the map's (`by_material`).
    pub fn material_group(&self, game: Option<&str>, letter: char) -> Option<&str> {
        if let Some(own) = game.map(|g| super::impact::own_material_group(g, letter))
            && let Some((name, _)) = self.groups.get_key_value(&own)
        {
            return Some(name);
        }
        self.by_material.get(&letter.to_ascii_uppercase()).map(String::as_str)
    }
}

/// A player's spray (CS:S's `impulse 201`) on its own entity: whose, the
/// logo's name (`MapDecals::logos`), where on the surface, the surface's
/// normal and the logo's up direction along it. The simulation keeps one
/// per player (`weapon::spray`); replicated, so every client draws it
/// from its own install's logos (`render::spray`), or from `picture` when
/// the player's logo is one they imported.
#[derive(Component, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Spray {
    #[entities]
    pub owner: Entity,
    pub logo: String,
    pub point: Vec3,
    pub normal: Vec3,
    pub up: Vec3,
    /// The picture itself when the logo is the player's own import (it
    /// travels with the spray, as Source uploads a custom spray); None for
    /// the install's logos, drawn by name.
    pub picture: Option<SprayPicture>,
}

/// The most a spray picture's side measures: imports are reduced to it, and
/// a network server refuses larger ones (64 x 64 RGBA is 16 KiB).
pub const SPRAY_PICTURE_MAX_SIDE: u32 = 64;

/// An imported spray's picture, RGBA8 rows from the top, at most
/// `SPRAY_PICTURE_MAX_SIDE` on a side.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SprayPicture {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

impl SprayPicture {
    /// Whether the sides are in range and the bytes match them (what a
    /// server checks of a client's picture).
    pub fn valid(&self) -> bool {
        let (w, h) = (self.width as usize, self.height as usize);
        let max = SPRAY_PICTURE_MAX_SIDE as usize;
        (1..=max).contains(&w) && (1..=max).contains(&h) && self.rgba.len() == w * h * 4
    }

    /// `rgba` (`width` x `height`) reduced to at most
    /// `SPRAY_PICTURE_MAX_SIDE` a side by averaging blocks (sides that
    /// aren't multiples of the factor lose their last texels). None for an
    /// empty or mismatched picture.
    pub fn reduced(width: u32, height: u32, rgba: &[u8]) -> Option<Self> {
        if width == 0 || height == 0 || rgba.len() != (width * height * 4) as usize {
            return None;
        }
        let max = SPRAY_PICTURE_MAX_SIDE;
        let (fx, fy) = ((width / max).max(1), (height / max).max(1));
        let (w, h) = (width / fx, height / fy);
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0u32; 4];
                for j in 0..fy {
                    for i in 0..fx {
                        let at = (((y * fy + j) * width + x * fx + i) * 4) as usize;
                        for (s, v) in sum.iter_mut().zip(&rgba[at..at + 4]) {
                            *s += *v as u32;
                        }
                    }
                }
                out.extend(sum.map(|s| (s / (fx * fy)) as u8));
            }
        }
        Some(Self { width: w as u16, height: h as u16, rgba: out }).filter(Self::valid)
    }
}

/// Lift off the surface (meters): above the map's own decals (0.15 units)
/// and overlays (up to 0.25 units), so the map's decals never fail the
/// depth test against an impact's (none write depth; impacts draw after
/// them: `render::decal::IMPACT_ORDER`).
pub const LIFT: f32 = 0.35 * 0.0254;

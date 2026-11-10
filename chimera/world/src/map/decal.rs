//! Runtime decals (bullet holes, knife slashes) for any game, as data:
//! the map carries decal definitions grouped by name (`MapDecals`), which
//! `render::decal` projects onto what a `PlaceDecal` message hits;
//! how high above surfaces impacts lie (`LIFT`, `DEPTH_DROP`: dynamic
//! shadows lie between).
//!
//! Runtime decals for any game as data: `MapDecals` (decal images in atlas
//! rectangles, weighted groups, surface material → group, players' spray
//! logos by name), the impacts' lift; `Spray`, a player's spray where the
//! simulation put it (`weapon::spray`), drawn by `render::spray`.

use std::collections::HashMap;

use bevy::prelude::*;

/// One decal image: a rectangle of a texture (often a decal atlas) and its
/// size on surfaces.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MapDecal {
    /// Index into `MapData::textures`.
    pub texture: usize,
    pub uv_min: Vec2,
    pub uv_max: Vec2,
    /// Width and height, meters.
    pub size: Vec2,
    pub blend: DecalBlend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DecalBlend {
    /// The surface times 2 x texel (0.5 grey leaves it unchanged).
    Modulate2x,
    /// Alpha-blended decals (lit by the surface in the game); drawn as a
    /// stain for now.
    Alpha,
}

/// The map's runtime decals: definitions, named groups of weighted
/// variants, and the group a surface material letter takes when shot.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MapDecals {
    pub decals: Vec<MapDecal>,
    /// Group name -> (weight, index into `decals`).
    pub groups: HashMap<String, Vec<(f32, usize)>>,
    /// Surface game material letter -> group name.
    pub by_material: HashMap<char, String>,
    /// Players' spray logos (CS:S's `materials/vgui/logos/*.vtf`) by
    /// lower-case name (`spray_kilroy`) -> index into `decals`, at the size
    /// a spray is drawn.
    pub logos: HashMap<String, usize>,
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
/// and overlays (up to 0.25 units), so impacts draw on top of them.
pub const LIFT: f32 = 0.35 * 0.0254;
/// How far below the decal its depth-only copy lies (meters): that copy
/// (0.30 units up) keeps overlays (up to 0.25) off impacts while every
/// impact's colour (0.35) still passes over every other impact's depth.
/// Dynamic shadows lie above it (`shadows::LIFT`), so they darken
/// impacts as the ground around them.
pub const DEPTH_DROP: f32 = 0.05 * 0.0254;

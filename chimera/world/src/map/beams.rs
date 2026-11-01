//! Beams and glows from map entities as map data, for any game
//! (specs/source/visual_entities.md): point_spotlight's shaft and halo,
//! env_laser and env_beam's beam, env_lightglow's glow. Drawing them is
//! `render::beams`'s.

use bevy::prelude::*;

use super::emitters::beam_segments;

/// A beam between two points (engine space, meters).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapBeam {
    /// The entity (index into `MapData::entities`) the logic switches it
    /// by.
    pub entity: Option<usize>,
    pub start: Vec3,
    pub end: Vec3,
    /// Widths at the start and end, meters (the strip is twice as wide).
    pub width: f32,
    pub end_width: f32,
    /// Colour x brightness, 0..1.
    pub color: Vec3,
    /// Shading: 1 out, 2 in, 3 both, 0 none; over `fade` meters (0: the
    /// whole beam).
    pub shade: u8,
    pub fade: f32,
    /// Index into `MapData::textures`.
    pub texture: usize,
    /// A spotlight's: the near-axis fade width (meters) and its halo.
    pub spot: Option<SpotHalo>,
    pub start_on: bool,
    /// Noise amplitude A (0-64; 0: a straight beam): the strip's points
    /// move sideways by fractal noise x A x the length / 100, new every
    /// frame (1 step 5).
    pub noise: f32,
    /// TextureScroll s: the texture slides along the beam, its offset
    /// frac(now x s x s) (1 step 8: map beams scroll at s² lengths a
    /// second, a quirk kept).
    pub scroll: f32,
    /// The texture repeats every 100 units along the beam; false: once
    /// over its length ("no tile", spotlights).
    pub tile: bool,
    /// The sprite's frames (indices into `MapData::textures`; empty: a
    /// still `texture`), the start frame and frames a second (1 step 3).
    pub frames: Vec<usize>,
    pub frame_start: f32,
    pub frame_rate: f32,
}

/// A point_spotlight's halo at the beam's start (2.3).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpotHalo {
    pub texture: usize,
    /// The spotlight's width W, meters.
    pub width: f32,
    /// rendercolor / 255.
    pub color: Vec3,
    /// The halo scale (60) in meters: half-size h x this.
    pub scale: f32,
    /// Occlusion proxy, meters.
    pub proxy: f32,
}

/// An env_lightglow (6): engine space, meters.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapGlow {
    pub entity: Option<usize>,
    pub position: Vec3,
    pub texture: usize,
    /// rendercolor / 255.
    pub color: Vec3,
    /// Half extents (horizontal, vertical) of the quad 100 units ahead,
    /// meters.
    pub half: Vec2,
    /// MinDist, MaxDist, OuterMaxDist, meters.
    pub min: f32,
    pub max: f32,
    pub outer: f32,
    /// "Visible only from front": its forward direction.
    pub front: Option<Vec3>,
    pub proxy: f32,
}

/// One unit, meters.
const UNIT: f32 = 0.0254;

impl MapBeam {
    /// Its length, units.
    pub fn length_units(&self) -> f32 {
        self.start.distance(self.end) / UNIT
    }

    /// The points its strip is drawn through (1 step 4; a spotlight's
    /// beam: one straight segment, 2.3).
    pub fn points(&self) -> usize {
        if self.spot.is_some() {
            return 2;
        }
        beam_segments(self.length_units(), self.width / UNIT, self.end_width / UNIT, self.noise).max(2)
    }

    /// The texture's offset along the beam at `now` seconds (1 step 8).
    pub fn scroll_offset(&self, now: f32) -> f32 {
        (now as f64 * self.scroll as f64 * self.scroll as f64).rem_euclid(1.0) as f32
    }

    /// The sprite frame shown at `now` (1 step 3), an index into `frames`.
    pub fn frame(&self, now: f32) -> usize {
        let count = self.frames.len().max(1);
        ((self.frame_start + now * self.frame_rate).max(0.0) as usize) % count
    }

    /// Whether its look changes from frame to frame (noise, scroll or an
    /// animated sprite).
    pub fn animated(&self) -> bool {
        self.noise != 0.0 || self.scroll != 0.0 || (self.frames.len() > 1 && self.frame_rate != 0.0)
    }
}

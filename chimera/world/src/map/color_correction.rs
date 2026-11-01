//! color_correction entities as map data (no spec; public entity docs): a
//! colour lookup table and how its weight falls off with the eye's
//! distance. Blending the weighted tables each frame is
//! `render::color_correction`'s.

use std::sync::Arc;

use bevy::prelude::*;

/// A table's size per channel.
pub const LUT_SIZE: usize = 32;

/// A `.raw` table's length in bytes.
pub const LUT_BYTES: usize = LUT_SIZE * LUT_SIZE * LUT_SIZE * 3;

/// A placed color_correction.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapColorCorrection {
    /// Index into `MapData::entities`.
    pub entity: Option<usize>,
    /// Engine space, meters.
    pub position: Vec3,
    /// Full weight within `min_falloff`, none past `max_falloff`, linear
    /// between (meters); a negative `max_falloff`: full weight everywhere.
    pub min_falloff: f32,
    pub max_falloff: f32,
    pub max_weight: f32,
    /// Seconds to fade in (Enable) and out (Disable).
    pub fade_in: f32,
    pub fade_out: f32,
    /// While it has weight, the others have none.
    pub exclusive: bool,
    pub start_on: bool,
    /// `LUT_BYTES` bytes, entry r + 32 g + 1024 b.
    #[serde(with = "crate::map::pod_serde::pod_arc")]
    pub lut: Arc<Vec<u8>>,
}

impl MapColorCorrection {
    /// Its weight at `distance` from the eye at full strength (before the
    /// fade).
    pub fn weight_at(&self, distance: f32) -> f32 {
        if self.max_falloff < 0.0 || distance <= self.min_falloff.max(0.0) {
            return self.max_weight;
        }
        if distance >= self.max_falloff {
            return 0.0;
        }
        let span = (self.max_falloff - self.min_falloff.max(0.0)).max(1e-6);
        self.max_weight * (1.0 - (distance - self.min_falloff.max(0.0)) / span)
    }
}

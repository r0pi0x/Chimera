//! Units: Source's (inches, Z up), which the movement's maths runs in, to the
//! engine's (metres, Y up) and back, and where a character's `Transform` sits
//! over its feet.

use bevy::prelude::*;

pub(crate) const METERS_PER_UNIT: f32 = 0.0254;

/// The transform sits at the standing box's centre.
/// The transform sits this far above the feet (a fixed convention, the
/// shared code's standing-box centre), whatever the hull sizes.
pub(crate) const ORIGIN_ABOVE_FEET: f32 = 36.0;

/// Engine (meters, Y up) to Source (units, Z up), positions and directions.
#[inline]
pub fn to_units(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.z, v.y) / METERS_PER_UNIT
}

#[inline]
pub fn to_engine(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y) * METERS_PER_UNIT
}

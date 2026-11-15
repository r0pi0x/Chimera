//! Halo's space to the engine's: the one place the world unit and the
//! axis mapping live (specs/halo_ce/cache_file.md, "The world": 1 world
//! unit = 3.048 m; +X forward, +Y left, +Z up, right-handed; yaw counter-
//! clockwise from +X seen from above). The engine's space is metres, +Y up,
//! right-handed, an `Intent` yaw of 0 looking along -Z.
//!
//! - Points: engine = (-y, z, -x) x 3.048 (`point`); directions the same
//!   rotation without the scale (`direction`). The spec's forward (+X),
//!   left (+Y) and up (+Z) become -Z, -X and +Y: the engine's forward at
//!   yaw 0, its left, its up.
//! - The mapping is a proper rotation (determinant +1), so handedness and
//!   the sense of a winding are kept: a triangle counter-clockwise about
//!   its normal in Halo's space stays so here. The structure BSP stores
//!   render triangles clockwise seen from the front (structure_bsp.md,
//!   "Surfaces and triangle lists"); the engine wants counter-clockwise
//!   from the front, so `render_triangle` swaps the second and third
//!   index. Collision polygons are already counter-clockwise about their
//!   outward normal (collision.md, "Recovering a surface's polygon") and
//!   keep their order.
//! - Yaw: a heading h (forward (cos h, sin h, 0)) maps to the engine
//!   forward (-sin h, 0, -cos h), which is `Quat::from_rotation_y(h)` x
//!   -Z: the engine yaw is h itself (`yaw`, derived from the mapped
//!   forward rather than assumed).

use bevy::math::Vec3;

/// The world unit in metres (`game::METERS_PER_UNIT`).
pub const METERS_PER_UNIT: f32 = crate::game::METERS_PER_UNIT;

/// A direction in Halo's axes as the engine's (no scale).
pub fn direction(v: [f32; 3]) -> Vec3 {
    Vec3::new(-v[1], v[2], -v[0])
}

/// A point in world units as the engine's, in metres.
pub fn point(p: [f32; 3]) -> Vec3 {
    direction(p) * METERS_PER_UNIT
}

/// An engine point back in Halo's world units (tests, traces).
pub fn to_game(p: Vec3) -> [f32; 3] {
    let q = p / METERS_PER_UNIT;
    [-q.z, -q.x, q.y]
}

/// A stored render triangle (clockwise from the front) as the engine's
/// counter-clockwise-from-the-front order.
pub fn render_triangle(t: [u32; 3]) -> [u32; 3] {
    [t[0], t[2], t[1]]
}

/// The engine's `Intent` yaw for a Halo heading in radians (forward
/// `(cos h, sin h, 0)`): the yaw whose forward,
/// `Quat::from_rotation_y(yaw) * -Z = (-sin yaw, 0, -cos yaw)`, is the
/// heading's forward mapped by `direction`.
pub fn yaw(heading: f32) -> f32 {
    let f = direction([heading.cos(), heading.sin(), 0.0]);
    (-f.x).atan2(-f.z)
}

/// A box in world units (`lower`, `upper` per axis x, y, z) as the
/// engine's (min, max) in metres.
pub fn bounds(x: [f32; 2], y: [f32; 2], z: [f32; 2]) -> (Vec3, Vec3) {
    let a = point([x[0], y[0], z[0]]);
    let b = point([x[1], y[1], z[1]]);
    (a.min(b), a.max(b))
}

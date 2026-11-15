//! Halo's game-state random generator and the shot direction table
//! (specs/halo_ce/weapons.md, "Constants" and section 7): a 32-bit linear
//! congruential generator (`HaloRng`: s' = 1664525 s + 1013904223), its
//! integer, integer range, real and real range draws exactly as the spec
//! states them, and the 1026 unit directions of the spread cone
//! (`directions`: an octahedron's faces subdivided 16 times per edge,
//! pushed out to the unit sphere).
//!
//! A weapon keeps its own generator state (`super::tick::HaloGun::rng`),
//! so a shot's draws happen in the spec's order (firing effect, misfire,
//! the cone per projectile, detonation) and replay the same on a client
//! predicting the shot.
//!
//! Known shortcuts:
//! - The direction table's order is not given by the spec (weapons.md
//!   Open question 2): ours lists the lattice points (a, b, c) with
//!   |a| + |b| + |c| = 16 by a, then b, then c ascending. The set and so
//!   the spread's distribution are the spec's; the exact pattern of a
//!   given seed is not the original's.
//! - The generator is per weapon, not one shared by the whole game state
//!   (the original's draws by effects and other objects in between are
//!   not interleaved).

use bevy::math::Vec3;

/// The generator's multiplier and increment (weapons.md, "Constants").
pub const RNG_A: u32 = 1664525;
pub const RNG_C: u32 = 1013904223;

/// The number of shot directions.
pub const DIRECTION_COUNT: usize = 1026;

/// Subdivisions per octahedron edge.
const SUBDIVISIONS: i32 = 16;

/// The game-state generator: one 32-bit seed, replaced on every draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HaloRng(pub u32);

impl HaloRng {
    /// Advance and return the new seed's top 16 bits (0..65535).
    pub fn int(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(RNG_A).wrapping_add(RNG_C);
        self.0 >> 16
    }

    /// lo + floor((hi - lo) r / 65536).
    pub fn int_range(&mut self, lo: i32, hi: i32) -> i32 {
        let r = self.int() as i64;
        lo + (((hi - lo) as i64 * r) >> 16) as i32
    }

    /// r / 65535: 0 to 1 inclusive.
    pub fn real(&mut self) -> f32 {
        self.int() as f32 / 65535.0
    }

    /// lo + (hi - lo) x a real draw.
    pub fn real_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.real()
    }
}

/// The lattice points of the subdivided octahedron, in the module doc's
/// order.
fn lattice() -> impl Iterator<Item = (i32, i32, i32)> {
    let n = SUBDIVISIONS;
    (-n..=n).flat_map(move |a| {
        let rest = n - a.abs();
        (-rest..=rest).flat_map(move |b| {
            let c = rest - b.abs();
            let both = if c == 0 { 1 } else { 2 };
            (0..both).map(move |k| (a, b, if k == 0 { -c } else { c }))
        })
    })
}

/// Table direction `i` (0 to 1025).
pub fn direction(i: usize) -> Vec3 {
    let (a, b, c) = lattice().nth(i).unwrap_or((0, 0, SUBDIVISIONS));
    Vec3::new(a as f32, b as f32, c as f32).normalize()
}

/// The 1026 unit directions (module doc for their order).
pub fn directions() -> Vec<Vec3> {
    lattice().map(|(a, b, c)| Vec3::new(a as f32, b as f32, c as f32).normalize()).collect()
}

/// `d` turned into the cone (minimum error, `theta_max`) by the spec's
/// draws (weapons.md 7): an integer range draw (0, 1026) picks a table
/// direction t; the axis d x t; a degenerate axis leaves d as it is
/// (one draw); otherwise a real range draw (`min_error`, `theta_max`) is
/// the tilt toward t.
pub fn cone(rng: &mut HaloRng, d: Vec3, min_error: f32, theta_max: f32) -> Vec3 {
    let t = direction(rng.int_range(0, DIRECTION_COUNT as i32) as usize);
    let axis = d.cross(t);
    if axis.length_squared() == 0.0 {
        return d;
    }
    let theta = rng.real_range(min_error, theta_max);
    bevy::math::Quat::from_axis_angle(axis.normalize(), theta) * d
}

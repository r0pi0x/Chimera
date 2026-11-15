//! Halo's game-state random generator and the shot direction table
//! (specs/halo_ce/weapons.md, "Constants" and section 7): a 32-bit linear
//! congruential generator (`HaloRng`: s' = 1664525 s + 1013904223), its
//! integer, integer range, real and real range draws exactly as the spec
//! states them, and the 1026 unit directions of the spread cone
//! (`table`: an octahedron's faces subdivided 16 times per edge, pushed
//! out to the unit sphere, built in the spec's construction and order, so
//! a seed gives the original's tilt directions).
//!
//! The table is in Halo's frame (x forward, y left, z up), as the spec's
//! checkpoints are; `direction` gives an entry in the engine's axes
//! (`maps::space::direction`), where the shots aim, so the pattern turned
//! into the engine's frame is the same pattern.
//!
//! A weapon keeps its own generator state (`super::tick::HaloGun::rng`),
//! so a shot's draws happen in the spec's order (firing effect, misfire,
//! the cone per projectile, detonation) and replay the same on a client
//! predicting the shot.
//!
//! Known shortcuts:
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
const SUBDIVISIONS: usize = 16;

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

/// The table under construction (weapons.md 7, "The direction table").
struct Builder {
    table: Vec<Vec3>,
    /// Each made edge (lower corner, higher corner) and its first entry.
    edges: Vec<((usize, usize), usize)>,
}

impl Builder {
    /// The entry of point `n` (0 to 16) counted from corner `u` on the
    /// edge from `u` to `v`, the edge's 15 inner points made at once the
    /// first time one is needed (from the lower-numbered corner).
    fn edge_point(&mut self, u: usize, v: usize, n: usize) -> usize {
        if n == 0 {
            return u;
        }
        if n == SUBDIVISIONS {
            return v;
        }
        let (lo, hi) = (u.min(v), u.max(v));
        let start = match self.edges.iter().find(|(e, _)| *e == (lo, hi)) {
            Some((_, s)) => *s,
            None => {
                let s = self.table.len();
                let (a, b) = (self.table[lo], self.table[hi]);
                for k in 1..SUBDIVISIONS {
                    let t = k as f32 / SUBDIVISIONS as f32;
                    self.table.push((a * (1.0 - t) + b * t).normalize());
                }
                self.edges.push(((lo, hi), s));
                s
            }
        };
        start + if u == lo { n } else { SUBDIVISIONS - n } - 1
    }
}

/// The directions in the spec's order, Halo's frame (module doc), up to
/// at least entry `last` (all 1026 for `DIRECTION_COUNT - 1`).
fn build(last: usize) -> Vec<Vec3> {
    let n = SUBDIVISIONS;
    let mut b = Builder {
        table: vec![Vec3::Z, Vec3::Y, Vec3::X, -Vec3::Y, -Vec3::X, -Vec3::Z],
        edges: Vec::new(),
    };
    let faces = [(0, 1, 2), (0, 2, 3), (0, 3, 4), (0, 4, 1), (5, 1, 4), (5, 4, 3), (5, 3, 2), (5, 2, 1)];
    for (fa, fb, fc) in faces {
        if b.table.len() > last {
            break;
        }
        // The face's grid points' entries, by (row, col), once visited.
        let mut ids = vec![[usize::MAX; SUBDIVISIONS + 1]; SUBDIVISIONS + 1];
        let mut visit = |b: &mut Builder, row: usize, col: usize| {
            if ids[row][col] != usize::MAX {
                return;
            }
            ids[row][col] = if row == 0 {
                fa
            } else if col == 0 {
                b.edge_point(fa, fc, row)
            } else if col == row {
                b.edge_point(fa, fb, row)
            } else if row == n {
                b.edge_point(fc, fb, col)
            } else {
                let (ac, ab) = (b.edge_point(fa, fc, row), b.edge_point(fa, fb, row));
                let (ac, ab) = (b.table[ac], b.table[ab]);
                let w = col as f32 / row as f32;
                b.table.push((ac * (1.0 - w) + ab * w).normalize());
                b.table.len() - 1
            };
        };
        for row in 1..=n {
            for col in 1..=row {
                if b.table.len() > last {
                    break;
                }
                visit(&mut b, row - 1, col - 1);
                visit(&mut b, row, col - 1);
                visit(&mut b, row, col);
                if col < row {
                    visit(&mut b, row - 1, col);
                }
            }
        }
    }
    b.table
}

/// The direction table in Halo's frame (x forward, y left, z up): the
/// spec's entries and order.
pub fn table() -> Vec<Vec3> {
    build(DIRECTION_COUNT - 1)
}

/// Table direction `i` (0 to 1025) in the engine's axes (module doc):
/// the table built up to it (a few hundred normalisations; no table is
/// kept between shots, as process-wide state is not).
pub fn direction(i: usize) -> Vec3 {
    let d = build(i).get(i).copied().unwrap_or(Vec3::Z);
    crate::game::maps::space::direction(d.to_array())
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

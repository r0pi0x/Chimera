//! The pill's collision move (specs/halo_ce/movement.md section 5, "The
//! move", on collision.md's "Moving a pill"): sweep the bottom sphere's
//! centre by a tick's displacement, sliding along what it meets (one plane,
//! the line of two, stopping at three), the velocity clipped by the final
//! clip set, and the virtual crease contact. In world units, engine axes
//! (+Y up): the level is whatever `Space` answers, the engine's colliders
//! in the game (`super::space`).
//!
//! Each hit moves the point to where the sweep touched and then projects
//! it exactly onto the contact's plane pushed out by the radius (and, for a
//! surface facing down, lowered by the segment: the pill's top meets it),
//! as the spec's mover does. That projection makes the stored position
//! exact however precise the engine's time of impact is: the motion's part
//! along the plane is kept whole by the slide (the part done before the hit
//! plus the remaining part), only its part into the plane is decided by the
//! plane, and the plane comes from the contact point and normal.
//!
//! A shape the spec's mover does not count as hit (the remaining motion
//! does not go into it by more than epsilon: sliding along a floor it
//! touches) still stops an engine sweep that starts touching it; the sweep
//! then starts again a hair off that surface (`LIFT`), and the point is put
//! back onto it afterwards.

use bevy::math::{Mat3, Vec3};

use super::data::MovementData;

/// How far (WU) a sweep restarts off a surface it touches and slides
/// along (module doc): far below the spec's epsilon.
pub const LIFT: f32 = 2e-5;

/// What a sweep of the pill meets first (a `Space` may sweep a slightly
/// thinner shape: the mover then takes the time from the contact's plane).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// The fraction of the motion done when it touched (from 0; above 1
    /// when the space sweeps further than asked).
    pub t: f32,
    /// The contact on the surface (WU).
    pub point: Vec3,
    /// The contact normal, out of the surface toward the pill.
    pub normal: Vec3,
}

/// The level as the mover sees it.
pub trait Space {
    /// Sweep the pill (bottom sphere centre `p`, segment `h`, the data's
    /// radius) by `motion` (WU): the first contact, ignoring one the pill
    /// starts touching and moves away from.
    fn cast(&self, p: Vec3, h: f32, motion: Vec3) -> Option<Hit>;
    /// Whether a sphere of the data's radius swept from `p` by `motion`
    /// meets anything (the standing-room test).
    fn sphere_blocked(&self, p: Vec3, motion: Vec3) -> bool;
}

/// A plane of the clip set: its normal and its offset for the point
/// (`p . n = offset` on it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    pub offset: f32,
}

/// The outcome of one sweep.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Moved {
    /// Where the point ended.
    pub p: Vec3,
    /// The contacts' normals in order (real ones, then the virtual one).
    pub contacts: Vec<Vec3>,
    /// The final clip set.
    pub clip: Vec<Plane>,
}

impl Moved {
    /// A velocity clipped by the final clip set (spec: one plane removes
    /// its normal's part, two keep the part along their line, three stop).
    pub fn clip_velocity(&self, v: Vec3) -> Vec3 {
        match self.clip.as_slice() {
            [] => v,
            [a] => v - a.normal * v.dot(a.normal),
            [a, b] => {
                let e = a.normal.cross(b.normal).normalize_or_zero();
                e * v.dot(e)
            }
            _ => Vec3::ZERO,
        }
    }
}

/// The plane's offset for the point of a pill of segment `h` touching
/// `point` with normal `n`: pushed out by the radius, and lowered by the
/// segment when it faces down (collision.md, the ceiling's slab).
fn offset(point: Vec3, n: Vec3, r: f32, h: f32) -> f32 {
    point.dot(n) + r - (h * n.y).min(0.0)
}

/// `p` moved onto both planes (the closest such point).
fn onto_two(p: Vec3, a: Plane, b: Plane) -> Vec3 {
    let ab = a.normal.dot(b.normal);
    let det = 1.0 - ab * ab;
    if det.abs() < 1e-6 {
        return p + b.normal * (b.offset - p.dot(b.normal));
    }
    let ra = a.offset - p.dot(a.normal);
    let rb = b.offset - p.dot(b.normal);
    let alpha = (ra - ab * rb) / det;
    let beta = (rb - ab * ra) / det;
    p + a.normal * alpha + b.normal * beta
}

/// The three planes' common point, if they have one.
fn onto_three(p: Vec3, a: Plane, b: Plane, c: Plane) -> Vec3 {
    let m = Mat3::from_cols(a.normal, b.normal, c.normal).transpose();
    if m.determinant().abs() < 1e-6 {
        return onto_two(p, b, c);
    }
    m.inverse() * Vec3::new(a.offset, b.offset, c.offset)
}

/// Sweep the point `p` of a pill of segment `h` by `motion` (spec section
/// 5, "The move").
pub fn sweep(space: &dyn Space, d: &MovementData, mut p: Vec3, h: f32, motion: Vec3) -> Moved {
    let eps = d.epsilon;
    let r = d.radius;
    let max = d.max_contacts as usize;
    let mut clip: Vec<Plane> = Vec::new();
    let mut contacts: Vec<Vec3> = Vec::new();
    // Surfaces the sweep was restarted off (module doc).
    let mut lifted: Vec<Plane> = Vec::new();
    let mut rem = motion;
    let mut restarts = 0;
    let settle = |p: &mut Vec3, lifted: &mut Vec<Plane>| {
        for l in lifted.drain(..) {
            let off = p.dot(l.normal) - l.offset;
            if off > 0.0 && off <= 4.0 * LIFT {
                *p -= l.normal * off;
            }
        }
    };
    loop {
        if contacts.len() >= max {
            // The rest of the motion is dropped.
            break;
        }
        if rem.abs().max_element() < eps {
            break;
        }
        let Some(hit) = space.cast(p, h, rem) else {
            p += rem;
            break;
        };
        let n = hit.normal;
        let plane = Plane {
            normal: n,
            offset: offset(hit.point, n, r, h),
        };
        // When the pill meets the plane: from the plane itself (the cast's
        // shape may be thinner than the pill), never after the cast's touch;
        // beyond the motion, nothing is met.
        let into = -rem.dot(n);
        let t = if into > 0.0 {
            ((p.dot(n) - plane.offset) / into).clamp(0.0, hit.t)
        } else {
            hit.t
        };
        if t > 1.0 {
            p += rem;
            break;
        }
        if rem.dot(n) >= -eps && restarts < 4 {
            restarts += 1;
            lifted.push(plane);
            p += rem * t + n * LIFT;
            rem *= 1.0 - t;
            continue;
        }
        restarts = 0;
        p += rem * t;
        p += n * (plane.offset - p.dot(n));
        settle(&mut p, &mut lifted);
        contacts.push(n);
        rem *= 1.0 - t;
        rem -= n * rem.dot(n);
        let into: Vec<Plane> = clip.iter().copied().filter(|q| rem.dot(q.normal) < -eps).collect();
        match into.as_slice() {
            [] => clip = vec![plane],
            [a] => {
                let e = a.normal.cross(n).normalize_or_zero();
                if e == Vec3::ZERO {
                    clip = vec![plane];
                    continue;
                }
                rem = e * rem.dot(e);
                let third = clip
                    .iter()
                    .copied()
                    .find(|q| q.normal != a.normal && rem.dot(q.normal) < -eps);
                if let Some(c) = third {
                    p = onto_three(p, c, *a, plane);
                    rem = Vec3::ZERO;
                    clip = vec![c, *a, plane];
                } else {
                    p = onto_two(p, *a, plane);
                    clip = vec![*a, plane];
                }
            }
            [a, b, ..] => {
                p = onto_three(p, *a, *b, plane);
                rem = Vec3::ZERO;
                clip = vec![*a, *b, plane];
            }
        }
    }
    settle(&mut p, &mut lifted);
    if contacts.len() < max
        && let Some(n) = virtual_contact(&clip)
    {
        contacts.push(n);
    }
    Moved { p, contacts, clip }
}

/// The extra contact of a two- or three-plane clip set (spec section 5),
/// if it normalises.
pub fn virtual_contact(clip: &[Plane]) -> Option<Vec3> {
    let steepest = |planes: &[Plane]| {
        planes
            .iter()
            .map(|q| q.normal)
            .filter(|n| n.y < 0.0)
            .min_by(|a, b| a.y.total_cmp(&b.y))
    };
    let n = match clip {
        [a, b] => {
            let e = a.normal.cross(b.normal);
            match steepest(clip) {
                Some(s) => {
                    // Of e x N_s and N_s x e, the one pointing out of the
                    // crease (the spec's "keeping the steepest plane's
                    // orientation" is unclear: our choice).
                    let c = e.cross(s);
                    if c.dot(a.normal + b.normal) < 0.0 { -c } else { c }
                }
                None => {
                    let len2 = e.length_squared();
                    if len2 < 1e-12 {
                        return None;
                    }
                    Vec3::Y - e * (e.y / len2)
                }
            }
        }
        [_, _, _] => match steepest(clip) {
            Some(s) => Vec3::Y - s * s.y,
            None => Vec3::Y,
        },
        _ => return None,
    };
    let len = n.length();
    (len > 1e-6).then(|| n / len)
}

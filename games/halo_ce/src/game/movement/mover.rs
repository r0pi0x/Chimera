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
//! The clip follows the spec's detailed rule: per contact the remaining
//! motion is the tick's displacement scaled by the product of the
//! (1 - t) so far (never the already clipped motion), clipped to the new
//! plane as the candidate; a candidate going into the first stored plane
//! makes the line of the new and that plane (and a line going into the
//! second stored plane stops at the three planes' point), else one going
//! into the second stored plane makes their line, else the clip set is
//! the new plane alone. The clip set is kept newest first. The virtual
//! contact of two planes lies in the steepest one, with the spec's signs.
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
    /// On a climbable surface (a ladder: movement.md, "Climbable surfaces").
    pub climbable: bool,
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
    /// The normals of those on climbable surfaces.
    pub climbable: Vec<Vec3>,
    /// The final clip set, the newest plane first.
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
    let mut climbable: Vec<Vec3> = Vec::new();
    // Surfaces the sweep was restarted off (module doc).
    let mut lifted: Vec<Plane> = Vec::new();
    let mut rem = motion;
    // The product of the (1 - t) so far.
    let mut scale = 1.0;
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
            scale *= 1.0 - t;
            continue;
        }
        restarts = 0;
        p += rem * t;
        p += n * (plane.offset - p.dot(n));
        settle(&mut p, &mut lifted);
        if hit.climbable {
            climbable.push(n);
        }
        contacts.push(n);
        // The clip (module doc): the tick's motion scaled by every (1 - t) so far, never the
        // clipped one, clipped to the new plane is the candidate; it is checked against the
        // stored planes, newest first.
        scale *= 1.0 - t;
        let v = motion * scale;
        let candidate = v - n * v.dot(n);
        let line = |other: Plane| {
            let e = n.cross(other.normal).normalize_or_zero();
            (e != Vec3::ZERO).then(|| e * v.dot(e))
        };
        let first = clip.first().copied();
        let second = clip.get(1).copied();
        let into = |q: Option<Plane>, m: Vec3| q.filter(|q| m.dot(q.normal) < -eps);
        if let Some(f) = into(first, candidate)
            && let Some(m) = line(f)
        {
            if let Some(s) = into(second, m) {
                p = onto_three(p, plane, f, s);
                rem = Vec3::ZERO;
                clip = vec![plane, f, s];
            } else {
                p = onto_two(p, plane, f);
                rem = m;
                clip = vec![plane, f];
            }
        } else if let Some(s) = into(second, candidate)
            && let Some(m) = line(s)
        {
            p = onto_two(p, plane, s);
            rem = m;
            clip = vec![plane, s];
        } else {
            rem = candidate;
            clip = vec![plane];
        }
    }
    settle(&mut p, &mut lifted);
    if contacts.len() < max
        && let Some(n) = virtual_contact(&clip)
    {
        contacts.push(n);
    }
    Moved { p, contacts, climbable, clip }
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
        // Newest first (`Moved::clip`).
        [new, old] => {
            let c = new.normal.dot(old.normal);
            match steepest(clip) {
                // In the steepest plane, perpendicular to the crease (spec section 5: these
                // two signs, its worked example in the tests).
                Some(s) if s == new.normal => old.normal - new.normal * c,
                Some(_) => new.normal - old.normal * c,
                None => {
                    let e = new.normal.cross(old.normal);
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

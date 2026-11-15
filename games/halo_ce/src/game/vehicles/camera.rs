//! The third-person seat camera (specs/halo_ce/vehicles.md, "Third-person
//! camera"), as pure functions of the look and the level:
//!
//! - `track_point`: the seat's camera track at a look pitch: the cubic
//!   polynomial through the four control points around it (Newton forward
//!   differences: the curve passes through every point), points evenly
//!   spaced from pitch −90 to +90 degrees.
//! - `command`: the focus F (the vehicle's centre of mass, or the seat's
//!   named camera marker, or the root node when the model lacks it: the
//!   caller's `super::carry`), the track point o in the look's horizontal
//!   heading frame (the vehicle's pitch and roll do not enter), the depth
//!   D = |o|, the probe point Q and the camera C = Q − max(D, 0.6) L (for
//!   D >= 0.6 exactly F + o in the heading frame), looking along L.
//! - `pull_in`: the ray from Q back along −L for D + 0.02 against the level
//!   (the caller's cast: structure and scenery, front faces); a hit at t1
//!   puts the camera D t1 from Q, unless one of the four rays to the end
//!   point moved by ±0.174 D along the camera's up and left hits nearer
//!   (t2), when a 10-step bisection on that displacement finds the edge s
//!   and the distance is D ((1 − s) t2 + s t1).
//!
//! Known shortcuts:
//! - The observer's smoothing (the 1 s and 0.5 s transitions) is not
//!   modelled: the camera snaps to the computed place each tick (the spec's
//!   open question 11).
//! - The spec does not say which perturbed ray the bisection follows when
//!   several hit nearer, nor which end of the bisected interval is the
//!   edge: the nearest such ray and the interval's midpoint are taken.
//! - The final 0.05 WU keep-off from water surfaces is not applied (the
//!   level's water is not among the engine's colliders).
//! - A third-person seat without a track would use the globals' default
//!   track, of which the spec gives three points only: such a seat (none
//!   on the disc) gets no third-person camera.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::data::{VehicleDef, seat_flags};
use super::level::{self, to_engine, to_halo_dir};
use super::physics::PhysicsDef;
use super::{HaloVehicle, Seated, VehicleDefs};
use crate::core::{Intent, MapView};

/// The camera's least distance behind the probe point.
pub const MIN_DEPTH: f32 = 0.6;
/// The pull-in ray's extra length.
pub const PULL_EXTRA: f32 = 0.02;
/// The perturbed rays' displacement, per unit of depth.
pub const PERTURB: f32 = 0.174;
/// Bisection steps on the displacement.
pub const BISECTIONS: usize = 10;

/// The track's offset (x, y, z in the heading frame, y 0) at look pitch
/// `pitch` (radians, clamped to ±π/2).
pub fn track_point(points: &[[f32; 2]], pitch: f32) -> Vec3 {
    let at = |i: usize| Vec3::new(points[i][0], 0.0, points[i][1]);
    let n = points.len();
    if n == 0 {
        return Vec3::ZERO;
    }
    if n == 1 {
        return at(0);
    }
    let half = std::f32::consts::FRAC_PI_2;
    let t = (pitch.clamp(-half, half) + half) / std::f32::consts::PI;
    let x = t * (n - 1) as f32;
    if n < 4 {
        // Piecewise linear (no such track on the disc).
        let i = (x.floor() as usize).min(n - 2);
        return at(i).lerp(at(i + 1), x - i as f32);
    }
    let j = x.floor() as usize;
    let k = j.saturating_sub(1).min(n - 4);
    let s = x - k as f32;
    let (p0, p1, p2, p3) = (at(k), at(k + 1), at(k + 2), at(k + 3));
    let d1 = p1 - p0;
    let d2 = p2 - 2.0 * p1 + p0;
    let d3 = p3 - 3.0 * p2 + 3.0 * p1 - p0;
    p0 + s * d1 + s * (s - 1.0) / 2.0 * d2 + s * (s - 1.0) * (s - 2.0) / 6.0 * d3
}

/// The camera's command for focus `focus`, track offset `o`, look yaw
/// `yaw` (the heading, radians about +z) and pitch `pitch` (radians).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Command {
    /// The probe point Q.
    pub probe: Vec3,
    /// The look direction L.
    pub look: Vec3,
    /// The depth D = |o|.
    pub depth: f32,
    /// Where the camera sits with nothing in the way.
    pub camera: Vec3,
    /// The camera's up and left.
    pub up: Vec3,
    pub left: Vec3,
}

/// The command (module doc).
pub fn command(focus: Vec3, o: Vec3, yaw: f32, pitch: f32) -> Command {
    let half = std::f32::consts::FRAC_PI_2;
    let theta = pitch.clamp(-half, half);
    let h = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
    let left = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
    let d = o.length();
    let look = h * theta.cos() + Vec3::Z * theta.sin();
    let probe = focus + h * (d * theta.cos() + o.x) + left * o.y + Vec3::Z * (d * theta.sin() + o.z);
    Command {
        probe,
        look,
        depth: d,
        camera: probe - look * d.max(MIN_DEPTH),
        up: Vec3::Z * theta.cos() - h * theta.sin(),
        left,
    }
}

/// The pulled-in camera of `c` (module doc); `cast(from, to)` is the hit
/// fraction of a segment against the level, if any.
pub fn pull_in(c: &Command, cast: &dyn Fn(Vec3, Vec3) -> Option<f32>) -> Vec3 {
    let d = c.depth;
    let end = c.probe - c.look * (d + PULL_EXTRA);
    let Some(t1) = cast(c.probe, end) else {
        return c.camera;
    };
    let mut nearer: Option<(f32, Vec3)> = None;
    for dir in [c.up, -c.up, c.left, -c.left] {
        let off = dir * (PERTURB * d);
        if let Some(t2) = cast(c.probe, end + off)
            && t2 < t1
            && nearer.is_none_or(|(n, _)| t2 < n)
        {
            nearer = Some((t2, off));
        }
    }
    let distance = match nearer {
        None => d * t1,
        Some((t2, off)) => {
            let (mut lo, mut hi) = (0.0f32, 1.0f32);
            for _ in 0..BISECTIONS {
                let mid = (lo + hi) / 2.0;
                match cast(c.probe, end + off * mid) {
                    Some(t) if t < t1 => hi = mid,
                    _ => lo = mid,
                }
            }
            let s = (lo + hi) / 2.0;
            d * ((1.0 - s) * t2 + s * t1)
        }
    };
    c.probe - c.look * distance
}

/// The key of a vehicle's root node's bind-pose position in its markers
/// (`super::place`): the focus of a seat whose named camera marker the
/// model lacks.
pub const ROOT_NODE: &str = "#root node";

/// The focus of seat `seat` of `v` (module doc): the centre of mass for an
/// empty camera marker name, else the named marker, else the root node.
pub fn focus(v: &HaloVehicle, d: &VehicleDef, phys: &PhysicsDef, seat: usize) -> Vec3 {
    let name = d.seats[seat].camera_marker.to_lowercase();
    if name.is_empty() {
        return v.drive.body.position;
    }
    match v.markers.get(&name).or_else(|| v.markers.get(ROOT_NODE)) {
        Some(m) => v.world(phys, *m),
        None => v.drive.body.position,
    }
}

/// The look's heading (radians about Halo's +z) from the engine's yaw.
pub fn heading(yaw: f32) -> f32 {
    let h = to_halo_dir(Quat::from_rotation_y(yaw) * Vec3::NEG_Z);
    h.y.atan2(h.x)
}

/// The camera seat `seat` of `v` gives a rider looking by `intent` (module
/// doc; None: a seat without a third-person camera), the pull-in's rays
/// cast against the level, past what `skip` names (characters, vehicles).
pub fn seat_view(
    v: &HaloVehicle,
    d: &VehicleDef,
    phys: &PhysicsDef,
    seat: usize,
    intent: &Intent,
    query: &SpatialQuery,
    skip: &dyn Fn(Entity) -> bool,
) -> Option<MapView> {
    let s = &d.seats[seat];
    let track = (s.flags & seat_flags::THIRD_PERSON_CAMERA != 0)
        .then(|| d.tracks.get(&s.track))
        .flatten()?;
    let pitch = intent.pitch.clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    let c = command(focus(v, d, phys, seat), track_point(&track.points, pitch), heading(intent.yaw), pitch);
    // Structure and scenery, front faces; not characters or vehicles.
    let filter = SpatialQueryFilter::default().with_mask(level::mask());
    let cast = |from: Vec3, to: Vec3| -> Option<f32> {
        let (a, b) = (to_engine(from), to_engine(to));
        let len = (b - a).length();
        let dir = Dir3::new(b - a).ok()?;
        let hit = query.cast_ray_predicate(a, dir, len, false, &filter, &|x| !skip(x))?;
        (hit.normal.dot(*dir) < 0.0).then_some(hit.distance / len)
    };
    Some(MapView {
        origin: to_engine(pull_in(&c, &cast)),
        rotation: Quat::from_rotation_y(intent.yaw) * Quat::from_rotation_x(pitch),
    })
}

/// Each tick (authoritative): a third-person seat's rider gets the seat's
/// camera as its `MapView` (`seat_view`); other riders their own eye.
#[allow(clippy::type_complexity)]
pub fn view(
    vehicles: Query<&HaloVehicle>,
    riders: Query<(Entity, &Seated, &Intent, Option<&MapView>)>,
    characters: Query<(), With<Intent>>,
    defs: Res<VehicleDefs>,
    query: SpatialQuery,
    mut commands: Commands,
) {
    for (e, seat, intent, have) in &riders {
        let Ok(v) = vehicles.get(seat.vehicle) else { continue };
        let (d, phys) = &defs.0[v.def];
        let skip = |x: Entity| characters.contains(x) || vehicles.contains(x);
        let want = seat_view(v, d, phys, seat.seat, intent, &query, &skip);
        match (want, have) {
            (Some(w), Some(h)) if *h == w => {}
            (Some(w), _) => {
                commands.entity(e).insert(w);
            }
            (None, Some(_)) => {
                commands.entity(e).remove::<MapView>();
            }
            (None, None) => {}
        }
    }
}

//! Seats and the seat camera (specs/halo_ce/vehicles.md, "Entering,
//! exiting and flipping", "Driving input and the camera"): what a player on
//! foot is offered (`offer`: the best seat by the spec's search, or the
//! flip of an overturned driverless vehicle), the passenger seat's yaw arc
//! (`clamp_yaw`), and the camera track's offset at a look pitch
//! (`track_offset`: a uniform cubic spline through the track's points from
//! pitch −90 to +90 degrees).
//!
//! Known shortcuts:
//! - A seat's distance is to its marker only: the entry point (the marker
//!   moved by the enter animation's first root displacement) needs the
//!   rider's animations.
//! - The spline is the uniform Catmull-Rom spline (the interpolating
//!   uniform cubic; the spec says "a uniform cubic spline through" the
//!   points without naming the basis).

use bevy::math::{Vec2, Vec3};

use super::HaloVehicle;
use super::data::{VehicleDef, seat_flags};
use super::drive::FLIP_UP_LIMIT;
use super::physics::PhysicsDef;

/// A seat is offered within this of the unit's centre (WU).
pub const SEAT_REACH: f32 = 1.0;
/// Non-driver seats count this much farther once a driver seat is a
/// candidate.
pub const DRIVER_PREFERENCE: f32 = 1.5;
/// The player must move slower than this (squared, (WU/t)²), and the
/// vehicle turn slower than its angular counterpart.
pub const ENTER_MAX_SPEED2: f32 = 0.01;
pub const ENTER_MAX_SPIN2: f32 = 0.01;

/// What a vehicle offers a player on foot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    Enter(usize),
    Flip,
}

/// Where a seat's marker is when the model has none (a synthetic map's
/// vehicle; the port's choice): driver front left, passenger front right,
/// gunner at the back.
pub fn fallback_marker(seat: usize) -> Vec3 {
    match seat {
        0 => Vec3::new(0.0, 0.25, 0.45),
        1 => Vec3::new(0.0, -0.25, 0.45),
        _ => Vec3::new(-0.6, 0.0, 0.7),
    }
}

/// The best offer of vehicle `v` to a player whose centre is `centre` and
/// whose squared speed is `speed2` (WU/t), with its distance.
pub fn offer(d: &VehicleDef, phys: &PhysicsDef, v: &HaloVehicle, centre: Vec3, speed2: f32) -> Option<(f32, Offer)> {
    let body = &v.drive.body;
    let near = (body.position - centre).length();
    if body.up.z <= FLIP_UP_LIMIT {
        let driverless = v.driver(d).is_none();
        return (driverless && v.drive.flip.is_none() && near < d.bounding_radius + SEAT_REACH)
            .then_some((near, Offer::Flip));
    }
    if speed2 >= ENTER_MAX_SPEED2 || body.angular_velocity.length_squared() >= ENTER_MAX_SPIN2 {
        return None;
    }
    let has_driver = v.driver(d).is_some();
    let mut best: Option<(f32, usize, bool)> = None;
    for (i, s) in d.seats.iter().enumerate() {
        if s.label.is_empty() || v.riders[i].is_some() {
            continue;
        }
        if s.flags & seat_flags::NOT_VALID_WITHOUT_DRIVER != 0 && !has_driver {
            continue;
        }
        let at = v.world(phys, v.seat_marker(d, i));
        let dist = (at - centre).length();
        if dist >= SEAT_REACH {
            continue;
        }
        let driver = s.flags & seat_flags::DRIVER != 0;
        let counted = |dist: f32, driver: bool, best_driver: bool| {
            if best_driver && !driver {
                dist * DRIVER_PREFERENCE
            } else {
                dist
            }
        };
        let better = match best {
            None => true,
            Some((b, _, bd)) => counted(dist, driver, bd) < b,
        };
        if better {
            best = Some((dist, i, driver));
        }
    }
    best.map(|(dist, i, _)| (dist, Offer::Enter(i)))
}

/// The passenger seat's yaw arc: a desired yaw (radians, relative to the
/// marker's yaw) outside [min, max] snaps to the nearer end by signed
/// angular difference; (0, 0) is no limit.
pub fn clamp_yaw(yaw: f32, range: [f32; 2]) -> f32 {
    if range == [0.0, 0.0] {
        return yaw;
    }
    let wrap = |a: f32| (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    let y = wrap(yaw);
    if y >= range[0] && y <= range[1] {
        return y;
    }
    let (a, b) = (wrap(y - range[0]).abs(), wrap(y - range[1]).abs());
    if a <= b { range[0] } else { range[1] }
}

/// The camera track's (x, z) offset at look pitch `pitch` (radians): t =
/// (pitch + π/2)/π along a uniform cubic spline through the points.
pub fn track_offset(points: &[[f32; 2]], pitch: f32) -> Vec2 {
    let n = points.len();
    if n == 0 {
        return Vec2::ZERO;
    }
    if n == 1 {
        return Vec2::from(points[0]);
    }
    let t = ((pitch + std::f32::consts::FRAC_PI_2) / std::f32::consts::PI).clamp(0.0, 1.0);
    let x = t * (n - 1) as f32;
    let i = (x.floor() as usize).min(n - 2);
    let u = x - i as f32;
    let p = |k: isize| Vec2::from(points[k.clamp(0, n as isize - 1) as usize]);
    let (p0, p1, p2, p3) = (p(i as isize - 1), p(i as isize), p(i as isize + 1), p(i as isize + 2));
    0.5 * (2.0 * p1
        + (p2 - p0) * u
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u * u
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * u * u * u)
}

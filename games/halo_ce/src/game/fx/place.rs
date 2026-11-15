//! Where a gun's effects go (specs/halo_ce/effects.md "Locations and
//! instances"): a location is a marker of the gun's model; an instance is
//! its frame in the world (`Frame`). Until the view model and the held
//! model expose their attachments, both views place a marker by the
//! camera: the first-person model's bind-pose marker (its frame is the
//! camera's: forward, left, up), the third person's the same point (a
//! seam: `third_person_frame`). The four weapons have no first-person
//! offset (weapons.md).
//!
//! **For the view model**: `first_person_frame` is the one place to
//! repoint; the marked line becomes the view model's attachment of the
//! marker's name in the world.

use bevy::prelude::*;

use super::load::FxGun;
use super::super::{METERS_PER_UNIT, maps::space};

/// A location instance in the world (engine space, metres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub at: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
}

impl Frame {
    /// A frame whose forward is `forward` (any up square to it).
    pub fn facing(at: Vec3, forward: Vec3) -> Frame {
        let forward = forward.normalize_or(Vec3::NEG_Z);
        let side = if forward.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let up = (side - forward * side.dot(forward)).normalize();
        Frame { at, forward, up }
    }

    pub fn left(&self) -> Vec3 {
        self.up.cross(self.forward)
    }

    /// A point given in the frame as Halo's (forward, left, up) in WU.
    pub fn point(&self, p: [f32; 3]) -> Vec3 {
        self.at + self.vector(p) * METERS_PER_UNIT
    }

    /// A direction given as (forward, left, up).
    pub fn vector(&self, v: [f32; 3]) -> Vec3 {
        self.forward * v[0] + self.left() * v[1] + self.up * v[2]
    }
}

/// The camera's frame: the eye and the view's rotation (the engine's yaw
/// and pitch).
pub fn camera(eye: Vec3, view: Quat) -> Frame {
    Frame { at: eye, forward: view * Vec3::NEG_Z, up: view * Vec3::Y }
}

/// Where the first-person gun's origin sits from the camera until the
/// view model places it (WU: forward, left, up): ours, by eye, so the
/// assault rifle's muzzle is low on the right of the view.
pub const GUN: [f32; 3] = [0.0, -0.05, -0.07];

/// Marker `name` of the first-person model, as the shooter sees it (the
/// camera-relative bind-pose marker; the camera itself when the model has
/// none).
pub fn first_person_frame(camera: Frame, gun: &FxGun, name: &str) -> Frame {
    // For the view model: replace with the view model's attachment `name`.
    let p = gun.fp_markers.get(name).copied().unwrap_or([0.0; 3]);
    Frame { at: camera.point([p[0] + GUN[0], p[1] + GUN[1], p[2] + GUN[2]]), ..camera }
}

/// Marker `name` as everyone else sees it (the seam for the held model's
/// marker: today the first person's point).
pub fn third_person_frame(camera: Frame, gun: &FxGun, name: &str) -> Frame {
    first_person_frame(camera, gun, name)
}

/// Halo's world vector (WU per tick) as the engine's (m/s).
pub fn velocity(v: [f32; 3]) -> Vec3 {
    space::direction(v) * METERS_PER_UNIT * 30.0
}

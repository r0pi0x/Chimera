//! Halo's aim assist (specs/halo_ce/weapons.md section 15): the
//! parameters in force for the weapon in hand at its zoom level
//! (`assist`), the levels by distance and angle (`level`), and the shot
//! bent toward the autoaim target within the deviation cone, with the
//! convergence correction toward the point the camera looks at (`bend`).
//! The target itself is picked by `systems::HaloTrace::target` (the
//! characters' positions are the simulation's).
//!
//! The target (the one guided projectiles follow: projectiles.md 12, the
//! needles and the charged bolt) is picked for every player;
//! `params::AimAssist::enabled` is the data's switch for it, on unless a
//! weapon's data turns it off. The bending of the shot toward it and the
//! convergence correction (`bend`) are the original's behaviour for a
//! controller and are off by default in every app (mouse and keyboard,
//! the only input the engine has): `Bend`, set by the console's
//! `halo_autoaim 1` (or a resource an app inserts). (Were the engine to
//! report a player's input device, `Bend` would follow it there.) The magnetism (look stickiness) is
//! left off: its levels only order the targets.
//!
//! A target's aim point (`aim_point`) is on its autoaim pill, the segment
//! from its pelvis to its head: the segment's point nearest the camera ray
//! (clamped to the segment), moved toward the ray by up to the autoaim
//! width. The convergence ray (step 1) starts the camera-to-holder distance
//! ahead of the camera, runs 128 WU and stops on the level and on any
//! character but the shooter (`systems::HaloTrace::look_at`); a miss is
//! its end.
//!
//! Known shortcuts: the pill's ends are the Spartan's pelvis and head
//! nodes in the identity pose above the character's feet, the feet
//! `EYE_HEIGHT` below its eye (`PELVIS_HEIGHT`, `HEAD_HEIGHT`; the
//! animated pose is not read); the
//! convergence ray meets characters by their hitboxes and not dead ones,
//! nor vehicles or other objects; no unit is "inconsequential" or
//! "ignored by autoaim" in a match.

use bevy::math::{Quat, Vec3};

use super::params::{Gun, weapon_flags};

/// Whether shots are bent toward the target and the look point (module
/// doc): off by default; `halo_autoaim 1` turns it on.
#[derive(bevy::prelude::Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bend(pub bool);

/// How far the convergence ray looks, WU (step 1).
pub const LOOK_RANGE: f32 = 128.0;

/// The Spartan's pelvis and head nodes above its feet in the identity
/// pose, WU (weapons.md 15's test rows): the ends of its autoaim pill.
pub const PELVIS_HEIGHT: f32 = 0.36;
pub const HEAD_HEIGHT: f32 = 0.56;
/// The standing Spartan's eye above its feet, WU (the same rows: the
/// camera at 0.62 over feet at 0).
pub const EYE_HEIGHT: f32 = 0.62;

/// The cyborg's autoaim width, WU (objects.md).
pub const AUTOAIM_WIDTH: f32 = 0.08;

/// A target's aim point (weapons.md 15): the point of the segment `pelvis`
/// to `head` nearest the line from `eye` along `dir` (the segment's
/// parameter clamped to 0..1), then moved toward that line, square to it,
/// by its distance or `width`, whichever is smaller. Any units, any axes.
pub fn aim_point(pelvis: Vec3, head: Vec3, width: f32, eye: Vec3, dir: Vec3) -> Vec3 {
    let d = dir.normalize_or_zero();
    let u = head - pelvis;
    let w0 = pelvis - eye;
    let (a, b) = (u.dot(u), u.dot(d));
    let denom = a - b * b;
    let t = if denom > 1e-12 { ((b * d.dot(w0) - u.dot(w0)) / denom).clamp(0.0, 1.0) } else { 0.0 };
    let p = pelvis + u * t;
    let on_ray = eye + d * (p - eye).dot(d);
    let off = on_ray - p;
    let len = off.length();
    if len <= 1e-9 {
        return p;
    }
    p + off / len * len.min(width)
}

/// The aim assist parameters in force (radians and world units, scaled by
/// the zoom's magnification).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Assist {
    pub autoaim_angle: f32,
    pub autoaim_range: f32,
    pub magnetism_angle: f32,
    pub magnetism_range: f32,
    /// The cone the shot may be turned within: max(deviation angle,
    /// autoaim angle) / m.
    pub deviation: f32,
}

/// The parameters of `g` at zoom level `zoom` (-1: none): none for a
/// weapon that "aim assists only when zoomed" while not zoomed.
pub fn assist(g: &Gun, zoom: i16) -> Option<Assist> {
    if zoom < 0 && g.has(weapon_flags::AIM_ASSISTS_ONLY_WHEN_ZOOMED) {
        return None;
    }
    let m = if zoom >= 0 { g.magnification(zoom) } else { 1.0 };
    let a = &g.aim_assist;
    Some(Assist {
        autoaim_angle: a.autoaim_angle / m,
        autoaim_range: a.autoaim_range * m,
        magnetism_angle: a.magnetism_angle / m,
        magnetism_range: a.magnetism_range * m,
        deviation: a.deviation_angle.max(a.autoaim_angle) / m,
    })
}

/// g(x, X): 1 up to X / 2, falling to 0 at X (0 for X <= 0).
pub fn level(x: f32, max: f32) -> f32 {
    if max <= 0.0 || x >= max {
        0.0
    } else if x <= max / 2.0 {
        1.0
    } else {
        (max - x) / (max / 2.0)
    }
}

/// The shot's direction `d` (unit) from `origin` bent (steps 1 to 4):
/// toward `look_at` (the convergence point), then toward the target's aim
/// point by its autoaim level (`target`: point and level), kept within
/// `deviation` of `d`.
pub fn bend(d: Vec3, origin: Vec3, look_at: Vec3, target: Option<(Vec3, f32)>, deviation: f32) -> Vec3 {
    let c = (look_at - origin).try_normalize().unwrap_or(d);
    let desired = match target {
        Some((point, l)) if l > 0.0 => {
            let t = (point - origin).try_normalize().unwrap_or(d);
            (c * (1.0 - l) + t * l).try_normalize().unwrap_or(c)
        }
        _ => c,
    };
    let angle = d.angle_between(desired);
    if angle <= deviation {
        return desired;
    }
    let Some(axis) = d.cross(desired).try_normalize() else { return d };
    Quat::from_axis_angle(axis, deviation) * d
}

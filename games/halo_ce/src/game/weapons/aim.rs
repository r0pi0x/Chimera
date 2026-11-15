//! Halo's aim assist (specs/halo_ce/weapons.md section 15): the
//! parameters in force for the weapon in hand at its zoom level
//! (`assist`), the levels by distance and angle (`level`), and the shot
//! bent toward the autoaim target within the deviation cone, with the
//! convergence correction toward the point the camera looks at (`bend`).
//! The target itself is picked by `systems::HaloTrace::target` (the
//! characters' positions are the simulation's).
//!
//! The autoaim (the target, the bending, and the target guided
//! projectiles follow: projectiles.md 12) runs for every player whatever
//! the input device; `params::AimAssist::enabled` is the data's switch,
//! on unless a weapon's data turns it off (off: no target, no guidance,
//! the convergence correction alone). The magnetism (look stickiness) is
//! left off: its levels only order the targets.
//!
//! Known shortcuts: the look point of step 1 is the level's (characters
//! do not stop the convergence ray, which starts at the camera) or the
//! ray's end 128 WU away (the spec leaves a miss open); a target's aim
//! point is its centre (no autoaim pill); no unit is "inconsequential"
//! or "ignored by autoaim" in a match.

use bevy::math::{Quat, Vec3};

use super::params::{Gun, weapon_flags};

/// How far the convergence ray looks, WU (step 1).
pub const LOOK_RANGE: f32 = 128.0;

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

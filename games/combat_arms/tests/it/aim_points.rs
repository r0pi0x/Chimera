//! Where the firing tests aim at a named hit sphere: its centre when a
//! ray from the shooter's eye reaches that sphere first;
//! else the nearest point of it that does (the spec's skeleton holds the
//! rifle's forearm before the chest, characters.md 11 and Quirk 8, so a
//! shot at the chest's centre from ahead meets the arm).

use avian3d::prelude::ColliderAabb;
use bevy::prelude::*;
use chimera_core::core::{Intent, MovementState};
use combat_arms::game::weapons::{self as ca, MALE};

/// The point `shooter` aims at on `p`'s sphere `name` (male table).
pub fn point(world: &World, shooter: Entity, p: Entity, name: &str) -> Vec3 {
    let geometry = |e: Entity| {
        ca::body_geometry(
            world.get::<Transform>(e)?,
            world.get::<Intent>(e)?,
            world.get::<MovementState>(e)?,
            world.get::<ColliderAabb>(e),
            *MALE,
        )
    };
    let i = MALE.iter().position(|s| s.name == name).unwrap();
    let (_, _, spheres) = geometry(p).unwrap();
    let (centre, radius) = spheres[i];
    let eye =
        (|| Some(world.get::<Transform>(shooter)?.translation + world.get::<MovementState>(shooter)?.eye_offset))();
    let Some(eye) = eye else { return centre };
    let wins = |at: Vec3| {
        let end = eye + (at - eye) * 2.0;
        ca::pick_sphere(&spheres, eye, end)
            .is_some_and(|j| j == i || (MALE[j].location == MALE[i].location && MALE[j].factor == MALE[i].factor))
    };
    let dir = (centre - eye).normalize();
    let side = dir.cross(Vec3::Y).normalize();
    let up = side.cross(dir);
    // With a margin: the shot's own small differences stay on it.
    let m = radius * 0.1;
    let wins = |at: Vec3| {
        [Vec3::ZERO, side * m, -side * m, up * m, -up * m]
            .iter()
            .all(|o| wins(at + *o))
    };
    if wins(centre) {
        return centre;
    }
    let mut best: Option<(f32, Vec3)> = None;
    for a in -12..=12 {
        for b in -12..=12 {
            let off = (side * a as f32 + up * b as f32) * radius * 0.08;
            let at = centre + off;
            if off.length() < radius && wins(at) && best.is_none_or(|(d, _)| off.length() < d) {
                best = Some((off.length(), at));
            }
        }
    }
    best.map_or(centre, |(_, at)| at)
}

//! What a player's view ray points at, for any game's HUD: the eye and
//! direction a character looks along as drawn this frame (`view_ray`: the
//! eased eye with its view punch), the character a ray hits first
//! (`character_under`: the world and props block it, a collider counts
//! as the character it or a parent belongs to), and whether a straight
//! line between two points is clear (`clear_line`). Target IDs and name
//! tags use it (CS:S's `ui::target_id`, Combat Arms' `ui::hud`); each
//! game keeps its own range, words and look.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    core::{Intent, MovementState, SOLID_LAYERS},
    map::interp::{RenderedView, eye_view},
};

/// A ray from an eye along a view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimRay {
    pub eye: Vec3,
    pub dir: Dir3,
}

impl AimRay {
    /// The point `distance` along the ray.
    pub fn at(&self, distance: f32) -> Vec3 {
        self.eye + self.dir * distance
    }
}

/// The view ray of a character at `at` (its eye as drawn: the eased one
/// when it has a `RenderedView`, with the view punch).
pub fn view_ray(at: &Transform, intent: &Intent, state: &MovementState, view: Option<&RenderedView>) -> AimRay {
    let v = eye_view(view, intent, state);
    let look = Quat::from_euler(EulerRot::YXZ, v.yaw + v.punch.y, v.pitch + v.punch.x, 0.0);
    AimRay {
        eye: at.translation + v.eye_offset,
        dir: Dir3::new(look * Vec3::NEG_Z).unwrap_or(Dir3::NEG_Z),
    }
}

/// The ray the crosshair marks: in the over-the-shoulder camera
/// (`third_person::Shoulder::ray`) the camera's centre ray, else `ray`
/// (`view_ray`). Target IDs and name tags look along it.
pub fn crosshair_ray(shoulder: Option<&super::third_person::Shoulder>, ray: AimRay) -> AimRay {
    match shoulder.and_then(|s| s.on.then_some(s.ray).flatten()) {
        Some((eye, dir)) => AimRay { eye, dir },
        None => ray,
    }
}

/// The character `e` belongs to: itself or a parent up to four levels
/// up for which `is_character` holds.
pub fn character_of(e: Entity, is_character: impl Fn(Entity) -> bool, parents: &Query<&ChildOf>) -> Option<Entity> {
    let mut at = e;
    for _ in 0..4 {
        if is_character(at) {
            return Some(at);
        }
        at = parents.get(at).ok()?.parent();
    }
    None
}

/// The character `ray` hits first within `range` metres (gameplay layers:
/// the world and props block it; `exclude`, the looker, doesn't), and
/// where; None when the first thing hit isn't a character.
pub fn character_under(
    spatial: &SpatialQuery,
    ray: AimRay,
    range: f32,
    exclude: Entity,
    is_character: impl Fn(Entity) -> bool,
    parents: &Query<&ChildOf>,
) -> Option<(Entity, Vec3)> {
    let filter = SpatialQueryFilter::from_excluded_entities([exclude]).with_mask(SOLID_LAYERS);
    let hit = spatial.cast_ray(ray.eye, ray.dir, range, true, &filter)?;
    Some((character_of(hit.entity, is_character, parents)?, ray.at(hit.distance)))
}

/// Whether nothing on the gameplay layers but `exclude` lies between
/// `from` and `to`.
pub fn clear_line(spatial: &SpatialQuery, from: Vec3, to: Vec3, exclude: &[Entity]) -> bool {
    let Ok((dir, length)) = Dir3::new_and_length(to - from) else {
        return true;
    };
    let filter = SpatialQueryFilter::from_excluded_entities(exclude.iter().copied()).with_mask(SOLID_LAYERS);
    spatial.cast_ray(from, dir, length, true, &filter).is_none()
}

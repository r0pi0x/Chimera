//! Which ragdolls the local view sees, for games whose ragdolls go on a
//! timer the view resets (`map::ragdoll::RagdollLifetime::Faded`; TF2's,
//! specs/tf2/ragdolls.md 4), and their forced fade drawn (spec 5).
//!
//! Ragdolls seen by the local view (specs/tf2/ragdolls.md 4): `see_ragdolls`
//! marks a ragdoll `RagdollInView` (a timed one's `RagdollTimer` reads it) while a box of ±1
//! unit at its root body is in an active main camera's frustum and, on a map
//! with visibility (and `r_novis` 0, the camera inside the map), in a cluster
//! potentially visible from the camera's (no line of sight, no distance
//! limit); `fade_drawn_ragdolls` draws a forced fade's alpha on the ragdoll's
//! drawn body (`RenderLook`, alpha mode).
//!
//! Known shortcuts (docs/tech-debt.md): the view is checked each frame but the
//! timer runs in the fixed tick (`map::ragdoll`'s `remove_ragdolls`), so a removal
//! lands on the first tick past it, not the first frame (spec Edge cases);
//! areaportals don't narrow the clusters (the spec's test is the PVS alone).
//! Why it's OK for now: a tick is 15 ms; the spec's test has no areas.

use bevy::{
    camera::primitives::{Aabb, Frustum},
    prelude::*,
};

use crate::map::{
    RENDER_TRANS_ALPHA, RenderLook,
    ragdoll::{Ragdoll, RagdollInView, RagdollTimer},
    vis::{ActiveVisibility, MapVisibility},
};
use crate::render::water::MainCameraFilter;

/// The in-view box's half size: 1 unit (spec 4), m.
pub const VIEW_BOX_HALF: f32 = 0.0254;

/// Whether a view at `eye` with `frustum` sees a ragdoll whose root body
/// is at `root` (spec 4): the ±1 unit box in the frustum and, with `vis`,
/// in a cluster potentially visible from the eye's (the eye outside the
/// map: every cluster).
pub fn sees_ragdoll(frustum: &Frustum, eye: Vec3, vis: Option<&MapVisibility>, root: Vec3) -> bool {
    let half = Vec3::splat(VIEW_BOX_HALF);
    if !frustum.intersects_obb_identity(&Aabb::from_min_max(root - half, root + half)) {
        return false;
    }
    let Some(v) = vis else { return true };
    let Some(from) = v.cluster_at(eye) else { return true };
    let mut clusters = Vec::new();
    v.clusters_in_box(root - half, root + half, &mut clusters);
    v.sees_any(from, &clusters)
}

/// Mark the ragdolls the local view sees this frame (`RagdollInView`;
/// a timed one's timer reads it, from the tick after it was made), after
/// the frusta are updated.
#[allow(clippy::type_complexity)]
pub fn see_ragdolls(
    cameras: Query<(&GlobalTransform, &Camera, &Frustum), MainCameraFilter>,
    ragdolls: Query<(Entity, &Ragdoll, Has<RagdollInView>)>,
    bodies: Query<&GlobalTransform>,
    vis: Option<Res<ActiveVisibility>>,
    novis: Option<Res<crate::render::vis::NoVis>>,
    mut commands: Commands,
) {
    let vis = vis.as_ref().map(|v| &*v.0).filter(|_| novis.is_none_or(|n| n.0 == 0));
    for (e, r, marked) in &ragdolls {
        if r.placed {
            continue;
        }
        let root = r.bodies.first().and_then(|b| bodies.get(*b).ok()).map(|t| t.translation());
        let seen = root.is_some_and(|root| {
            cameras
                .iter()
                .filter(|(_, c, _)| c.is_active)
                .any(|(t, _, f)| sees_ragdoll(f, t.translation(), vis, root))
        });
        if seen != marked {
            if seen {
                commands.entity(e).insert(RagdollInView);
            } else {
                commands.entity(e).remove::<RagdollInView>();
            }
        }
    }
}

/// A ragdoll fading out (forced fade) draws its body at the timer's alpha.
pub fn fade_drawn_ragdolls(
    ragdolls: Query<(&Ragdoll, &RagdollTimer), Changed<RagdollTimer>>,
    mut commands: Commands,
) {
    for (r, t) in &ragdolls {
        let (Some(alpha), Some(body)) = (t.alpha, r.visual) else { continue };
        if let Ok(mut body) = commands.get_entity(body) {
            body.insert(RenderLook {
                alpha,
                mode: RENDER_TRANS_ALPHA,
                ..default()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::CameraProjection;

    /// A camera at `eye` looking down -X.
    fn view(eye: Vec3) -> Frustum {
        let at = GlobalTransform::from(Transform::from_translation(eye).looking_at(eye - Vec3::X, Vec3::Y));
        Projection::Perspective(PerspectiveProjection::default()).compute_frustum(&at)
    }

    /// Two rooms split by x = 0: 0 <= x < 10 is cluster 0, x < 0 cluster
    /// 1; 1 sees only itself, 0 both.
    fn two_rooms() -> MapVisibility {
        MapVisibility {
            planes: vec![(Vec3::X, 0.0), (Vec3::X, 10.0)],
            nodes: vec![(0, [1, -2]), (1, [-3, -1])],
            leaf_clusters: vec![0, 1, -1],
            cluster_count: 2,
            visible: vec![vec![0b11], vec![0b10]],
            ..default()
        }
    }

    #[test]
    fn seen_in_the_frustum_and_a_visible_cluster() {
        // In front: seen, however far (no distance limit); behind: not.
        let f = view(Vec3::new(5.0, 0.0, 0.0));
        assert!(sees_ragdoll(&f, Vec3::new(5.0, 0.0, 0.0), None, Vec3::new(2.0, 0.0, 0.0)));
        assert!(sees_ragdoll(&f, Vec3::new(5.0, 0.0, 0.0), None, Vec3::new(-500.0, 0.0, 0.0)));
        assert!(!sees_ragdoll(&f, Vec3::new(5.0, 0.0, 0.0), None, Vec3::new(8.0, 0.0, 0.0)));
        // Room 0 sees room 1: no line of sight needed.
        let v = two_rooms();
        assert!(sees_ragdoll(&f, Vec3::new(5.0, 0.0, 0.0), Some(&v), Vec3::new(-3.0, 0.0, 0.0)));
        // Room 1 doesn't see room 0, even in the frustum.
        let f = view(Vec3::new(-1.0, 0.0, 0.0));
        let behind = view(Vec3::new(9.0, 0.0, 0.0));
        assert!(!sees_ragdoll(&behind, Vec3::new(-1.0, 0.0, 0.0), Some(&v), Vec3::new(5.0, 0.0, 0.0)));
        assert!(sees_ragdoll(&f, Vec3::new(-1.0, 0.0, 0.0), Some(&v), Vec3::new(-4.0, 0.0, 0.0)));
    }
}

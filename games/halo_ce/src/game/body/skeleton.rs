//! The Spartan's skeleton as the engine's (specs/halo_ce/models.md, "Nodes
//! and the skeleton", "Skinning"): the model's nodes as `MapBone`s in
//! Halo's own axes and world units, each node's stored quaternion as the
//! usual rotation (the stored (i, j, k, w) is the usual rotation of its
//! conjugate, so `usual` negates i, j and k), the vertices' two node
//! indices and weight as joints and joint weights, and the body's `root`:
//! Halo's axes (+X forward, +Y left, +Z up) to the engine's character frame
//! (forward -Z, left -X, up +Y), scaled by the world unit in metres
//! (`maps::space`'s mapping). The model's origin is the Spartan's feet
//! (its bounding box runs from z 0 to 0.7036 WU), so the root has no
//! translation.

use bevy::math::{Mat3, Quat, Vec3};
use blam_formats::model::{Model, Vertex};

use super::super::{METERS_PER_UNIT, maps::space};
use crate::map::{MapBone, anim::BonePose};

/// A stored quaternion (i, j, k, w; the game's convention) as the usual
/// rotation: its conjugate, normalised (a zero quaternion: no rotation).
pub fn usual(q: [f32; 4]) -> Quat {
    let r = Quat::from_xyzw(-q[0], -q[1], -q[2], q[3]);
    let len = r.length();
    if len > 1e-12 && len.is_finite() {
        r / len
    } else {
        Quat::IDENTITY
    }
}

/// Halo's axes to the engine's, as a rotation (`space::direction`).
pub fn axes() -> Quat {
    let m = Mat3::from_cols(
        space::direction([1.0, 0.0, 0.0]),
        space::direction([0.0, 1.0, 0.0]),
        space::direction([0.0, 0.0, 1.0]),
    );
    Quat::from_mat3(&m).normalize()
}

/// The body's root: from the skeleton's space (Halo's axes, world units)
/// to the character's (metres, the engine's axes, feet at the origin).
pub fn root() -> bevy::prelude::Transform {
    bevy::prelude::Transform {
        translation: Vec3::ZERO,
        rotation: axes(),
        scale: Vec3::splat(METERS_PER_UNIT),
    }
}

/// The model's nodes as bones, parents before children (the model spec
/// guarantees every parent index is below its child's).
pub fn bones(model: &Model) -> Vec<MapBone> {
    model
        .nodes
        .iter()
        .map(|n| MapBone {
            name: n.name.clone(),
            parent: n.parent,
            position: Vec3::from(n.translation),
            rotation: usual(n.rotation),
        })
        .collect()
}

/// The bones' bind pose as local poses (the animation set's defaults).
pub fn bind_locals(bones: &[MapBone]) -> Vec<BonePose> {
    bones.iter().map(|b| (b.rotation, b.position)).collect()
}

/// The bind pose in the skeleton's space (each bone's rotation and
/// position), composed in index order.
pub fn globals(bones: &[MapBone], locals: &[BonePose]) -> Vec<BonePose> {
    let mut out: Vec<BonePose> = Vec::with_capacity(locals.len());
    for (b, (q, p)) in bones.iter().zip(locals) {
        out.push(match b.parent.and_then(|i| out.get(i)) {
            Some((pq, pp)) => (*pq * *q, *pp + *pq * *p),
            None => (*q, *p),
        });
    }
    out
}

/// A vertex's joints and weights (models.md, "Skinning": node 0 with
/// weight w, node 1 with 1 - w; a rigid vertex all on node 0).
pub fn joints(v: &Vertex) -> ([u16; 4], [f32; 4]) {
    match v.node1 {
        Some(n1) => (
            [v.node0 as u16, n1 as u16, 0, 0],
            [v.weight0, 1.0 - v.weight0, 0.0, 0.0],
        ),
        None => ([v.node0 as u16, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
    }
}

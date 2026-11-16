//! The Spartan's hit volumes (specs/halo_ce/collision.md, "Model
//! collision"; health_shields.md, the head material): the collision
//! model's nodes are its hit volumes, each node's collision BSP in that
//! node's frame. The engine's hit volume is an oriented box on a bone
//! (`BoneBox`), so each node with a region and a BSP becomes the bounding
//! box of its first BSP's vertices in the node's frame, on the render
//! model's node of the same name (the posed skeleton then places it).
//!
//! Each box's region (`BodyRegion`, its `Hitgroup` with it): `Head` for a
//! node whose surfaces mostly use a material flagged "head" (the
//! cyborg's head material), else by the node's name (`region_of_name`:
//! spine1 the chest, spine the stomach, pelvis the stomach, thigh, calf
//! (shin), foot, clavicle and upper arm, forearm, hand per side). Halo's
//! damage seam maps the head region to the head material and shins and
//! feet to the legs (`damage`), so a Halo bullet's headshot and another
//! game's head multiplier both come from the same box.
//!
//! Known shortcuts: a node's BSP is approximated by its bounding box in
//! the node's frame (not its convex pieces), and only its first BSP (the
//! Spartan's region has one permutation); a box reaching above the head
//! box's bottom in the bind pose is cut below it (`below_the_head`), so
//! the wider bounding boxes of the chest and shoulders do not cover the
//! head.

use bevy::math::{Quat, Vec3};
use blam_formats::collision::CollisionModel;

use super::skeleton::globals;
use crate::{
    core::{BodyRegion, Hitbox},
    map::{BoneBox, MapBone, anim::BonePose},
};

/// The region of a shield node's box (the Jackal's `frame shield`): the
/// groin, which no Halo humanoid's node is (its pelvis is the stomach), so
/// the damage seam can tell the disc's hits (`RegionMaterials::shield`).
pub const SHIELD_REGION: BodyRegion = BodyRegion::Groin;

/// The region a node's name gives (lower case, `bip01 l calf`).
pub fn region_of_name(name: &str) -> BodyRegion {
    let n = name.to_ascii_lowercase();
    let left = n.contains(" l ");
    let right = n.contains(" r ");
    let side = |l: BodyRegion, r: BodyRegion, other: BodyRegion| {
        if left {
            l
        } else if right {
            r
        } else {
            other
        }
    };
    use BodyRegion::*;
    if n.ends_with("head") {
        Head
    } else if n.ends_with("neck") {
        Neck
    } else if n.ends_with("spine1") {
        Chest
    } else if n.ends_with("spine") || n.ends_with("pelvis") {
        Stomach
    } else if n.ends_with("thigh") {
        side(LeftThigh, RightThigh, Stomach)
    } else if n.ends_with("calf") {
        side(LeftShin, RightShin, Stomach)
    } else if n.ends_with("foot") {
        side(LeftFoot, RightFoot, Stomach)
    } else if n.ends_with("clavicle") || n.ends_with("upperarm") {
        side(LeftUpperArm, RightUpperArm, Chest)
    } else if n.ends_with("forearm") {
        side(LeftForearm, RightForearm, Chest)
    } else if n.ends_with("hand") {
        side(LeftHand, RightHand, Chest)
    } else if n.contains("shield") {
        // The Jackal's arm shield (`frame shield`): a region of its own,
        // which no Halo node otherwise takes (`SHIELD_REGION`).
        SHIELD_REGION
    } else {
        Generic
    }
}

/// The boxes (module doc) on `bones`, in the skeleton's units.
pub fn boxes(collision: &CollisionModel, bones: &[MapBone]) -> Vec<BoneBox> {
    let mut out = Vec::new();
    for node in &collision.nodes {
        let Some(bsp) = node
            .bsps
            .first()
            .filter(|b| node.region.is_some() && !b.vertices.is_empty())
        else {
            continue;
        };
        let Some(bone) = bones.iter().position(|b| b.name.eq_ignore_ascii_case(&node.name)) else {
            continue;
        };
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for v in &bsp.vertices {
            lo = lo.min(Vec3::from(v.point));
            hi = hi.max(Vec3::from(v.point));
        }
        // The material most of its surfaces use.
        let mut counts = std::collections::BTreeMap::<i16, usize>::new();
        for s in &bsp.surfaces {
            *counts.entry(s.material).or_default() += 1;
        }
        let material = counts.into_iter().max_by_key(|(_, c)| *c).map(|(m, _)| m);
        let head = material
            .and_then(|m| usize::try_from(m).ok())
            .and_then(|m| collision.materials.get(m))
            .is_some_and(|m| m.flags & 1 != 0);
        let region = if head {
            BodyRegion::Head
        } else {
            region_of_name(&node.name)
        };
        out.push(BoneBox {
            bone,
            center: (lo + hi) / 2.0,
            half: (hi - lo) / 2.0,
            group: region.hitgroup(),
        });
    }
    below_the_head(&mut out, bones);
    out
}

/// Cut every other box whose top (in the bind pose, up being Halo's +Z)
/// reaches above the head box's bottom down to it, along the box's axis
/// nearest up: a bounding box is wider than its node's shape (the chest's
/// spans the shoulders and the neck), and the engine's hit is the first
/// box a ray enters, so an uncut chest box would hide the head from the
/// front.
fn below_the_head(boxes: &mut [BoneBox], bones: &[MapBone]) {
    let g = globals(bones, &super::skeleton::bind_locals(bones));
    let reach = |b: &BoneBox| {
        let (q, p) = g[b.bone];
        let axes = [Vec3::X, Vec3::Y, Vec3::Z].map(|a| (q * a).z);
        let mid = (p + q * b.center).z;
        let spread: f32 = (0..3).map(|i| b.half[i] * axes[i].abs()).sum();
        (mid - spread, mid + spread, axes)
    };
    let Some(head) = boxes.iter().find(|b| b.group == crate::core::Hitgroup::Head) else {
        return;
    };
    let floor = reach(head).0;
    for b in boxes.iter_mut().filter(|b| b.group != crate::core::Hitgroup::Head) {
        let (bottom, top, axes) = reach(b);
        if top <= floor || bottom >= floor {
            continue;
        }
        let a = (0..3)
            .max_by(|i, j| axes[*i].abs().total_cmp(&axes[*j].abs()))
            .unwrap_or(0);
        let k = axes[a].abs();
        if k < 1e-3 {
            continue;
        }
        let cut = ((top - floor) / k).min(b.half[a] * 2.0 * 0.9);
        b.half[a] -= cut / 2.0;
        b.center[a] -= axes[a].signum() * cut / 2.0;
    }
}

/// The boxes' regions, in order (the region of a box's group alone loses
/// sides and parts: kept here from the same rule).
pub fn regions(bones: &[MapBone], boxes: &[BoneBox]) -> Vec<BodyRegion> {
    boxes
        .iter()
        .map(|b| {
            if b.group == crate::core::Hitgroup::Head {
                BodyRegion::Head
            } else {
                region_of_name(&bones[b.bone].name)
            }
        })
        .collect()
}

/// The hitboxes in the character's frame (metres, feet at the origin) for
/// the pose `locals`, through `root` (the body's skeleton-to-character
/// transform): what the engine's posing writes each tick, here as the
/// model's starting values.
pub fn hitboxes(
    boxes: &[BoneBox],
    regions: &[BodyRegion],
    bones: &[MapBone],
    locals: &[BonePose],
    root: &bevy::prelude::Transform,
) -> Vec<Hitbox> {
    let g = globals(bones, locals);
    let scale = root.scale.x;
    boxes
        .iter()
        .zip(regions)
        .map(|(b, r)| {
            let (q, p) = g.get(b.bone).copied().unwrap_or((Quat::IDENTITY, Vec3::ZERO));
            Hitbox::cuboid(
                root.rotation * (p + q * b.center) * scale,
                b.half * scale,
                root.rotation * q,
                *r,
            )
        })
        .collect()
}

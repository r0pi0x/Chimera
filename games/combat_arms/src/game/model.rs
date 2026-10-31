//! Combat Arms' models: a model file from the install
//! (`lithtech_formats::ltb`, specs/combat_arms/models_ltb.md) with its
//! child models (animation.md, "In child models") converted into
//! Chimera's neutral types, the way `cs_source` converts an MDL: a
//! `MapModel` whose meshes are skinned to a `MapRig` (bones and an
//! `anim::AnimSet`), so props, view models and bodies all draw and play
//! through Chimera's model and animation layer.
//!
//! - Axes and units ("Converting to a right-handed Y-up frame"): Z
//!   mirrored, triangle order swapped; the skeleton stays in world units
//!   (the rig's root scales it by `scale` metres per unit), mesh
//!   positions are model space at bind pose in metres.
//! - Skeleton: each node's local bind (B_parent^-1 B_j, mirrored), parents
//!   before children (the file's depth-first order).
//! - Meshes: LOD 0 of every piece, one `MapMesh` per piece, skinned with
//!   each vertex's nodes and weights (`ltb::Mesh::vertex_influences`):
//!   rigid pieces follow their node, bone sets and palettes blend.
//!   `texture` names each piece's skin (by its first texture index; the
//!   caller's skin list, e.g. a world object's `Skin`).
//! - Animations: the combined list (`lithtech_formats::anim::AnimList`:
//!   own, then each child's), each entry one looping `Sequence` sampled
//!   per animation.md's "Sampling one animation" (root translation
//!   included, rotation-only nodes at their bind offset).
//!
//! Known shortcuts (docs/tech-debt.md):
//! - Animations are resampled at `SAMPLE_FPS` frames per second
//!   (`VIEW_SAMPLE_FPS` for view models) (and
//!   exactly at the last key) for Chimera's frame-based `AnimSet`, which
//!   then interpolates between samples (nlerp instead of the spec's
//!   slerp between the original keys). LithTech's millisecond tracker
//!   (`lithtech_formats::anim::Tracker`: key strings, "done" at L + 1,
//!   transitions over the new animation's interpolation time) is not
//!   what drives these sequences yet: they snap in, loop, and fire no
//!   key strings.
//! - Only LOD 0 (Chimera's model levels of detail aren't used for rigged
//!   models); vertex-animated pieces are drawn at their stored positions
//!   (not animated); render priority is ignored, and the render style
//!   is the caller's to apply (`render_style`, by `slot_and_style` or the
//!   LOD's index); only a piece's first texture index is used.
//! - A missing or unreadable child, or one whose skeleton shape differs,
//!   fails the whole model (models_ltb.md "Child models"; animation.md
//!   open question 4 is left as models_ltb.md says).

use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;
use lithtech_formats::{
    anim::{self as lta, AnimList},
    ltb::{self, Model},
};

use crate::map::{
    MapBone, MapMesh, MapModel, MapRig,
    anim::{AnimSet, Animation, BonePose, Sequence, Track},
};

/// Frames per second animations are resampled at.
pub const SAMPLE_FPS: f32 = 30.0;
/// The same for first-person view models (`convert_at`): their fire
/// kicks last a few frames of 30 fps.
pub const VIEW_SAMPLE_FPS: f32 = 240.0;

/// A model file and its children, read and checked.
pub struct LoadedModel {
    /// Its archive path (normalized: lower case, `/`).
    pub path: String,
    pub model: Model,
    /// Own animations, then each child's.
    pub list: AnimList,
}

/// Normalize an archive path (`PROPS\X.LTB` -> `props/x.ltb`).
pub fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// Read model `path` and its children (named relative to its folder)
/// through `file`.
pub fn load(path: &str, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<LoadedModel, String> {
    let path = normalize(path);
    let read = |p: &str| -> Result<Model, String> {
        let bytes = file(p).ok_or_else(|| format!("{p}: not found or locked (keyed archive)"))?;
        Model::parse(&bytes).map_err(|e| format!("{p}: {e}"))
    };
    let model = read(&path)?;
    let folder = path.rsplit_once('/').map_or("", |(d, _)| d);
    let mut children = Vec::new();
    for name in &model.children {
        let child_path = normalize(&if folder.is_empty() {
            name.clone()
        } else {
            format!("{folder}/{name}")
        });
        let child = read(&child_path)?;
        if !child.children.is_empty() {
            return Err(format!("{child_path}: a child model with children of its own"));
        }
        if child.parents() != model.parents() {
            return Err(format!("{child_path}: its skeleton differs from {path}'s"));
        }
        children.push(child);
    }
    let refs: Vec<Option<&Model>> = children.iter().map(Some).collect();
    let list = AnimList::build(&model, &refs);
    Ok(LoadedModel { path, model, list })
}

/// Model `path` alone, without its child models (its own animations
/// only): a piece merged onto another model's skeleton, whose animations
/// are the other's.
pub fn load_own(path: &str, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<LoadedModel, String> {
    let path = normalize(path);
    let bytes = file(&path).ok_or_else(|| format!("{path}: not found or locked (keyed archive)"))?;
    let model = Model::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let none: Vec<Option<&Model>> = model.children.iter().map(|_| None).collect();
    let list = AnimList::build(&model, &none);
    Ok(LoadedModel { path, model, list })
}

/// The LODs `convert` makes meshes of, in its order: LOD 0 of each piece
/// with geometry.
pub fn drawn_lods(model: &Model) -> Vec<&ltb::Lod> {
    model
        .pieces
        .iter()
        .filter_map(|p| p.lods.first())
        .filter(|l| l.mesh.geometry().is_some())
        .collect()
}

/// A drawn LOD's second texture index (slot 1: an environment or rim
/// map under a layered render style, `render_style::Layer`; -1 none).
pub fn second_slot(lod: &ltb::Lod) -> i32 {
    lod.used_textures().get(1).copied().unwrap_or(-1)
}

/// A drawn LOD's skin slot (its first texture index; -1 none) and its
/// render-style index.
pub fn slot_and_style(lod: &ltb::Lod) -> (i32, i32) {
    (lod.used_textures().first().copied().unwrap_or(-1), lod.render_style)
}

/// `slot_and_style` with the second slot between (`second_slot`).
pub fn slots_and_style(lod: &ltb::Lod) -> (i32, i32, i32) {
    let (slot, style) = slot_and_style(lod);
    (slot, second_slot(lod), style)
}

fn mat(m: &ltb::Mat4) -> Mat4 {
    Mat4::from_cols_array_2d(m).transpose()
}

fn rh(v: ltb::Vec3) -> Vec3 {
    Vec3::from(ltb::to_rh_vec(v))
}

/// A stored quaternion in the right-handed frame, normalised (identity
/// for a zero one).
fn rh_quat(q: ltb::Quat) -> Quat {
    let q = Quat::from_array(ltb::to_rh_quat(q));
    if q.length_squared() > 0.0 { q.normalize() } else { Quat::IDENTITY }
}

/// The skeleton: each node's local bind pose (right-handed, world units).
pub fn bones(model: &Model) -> Vec<MapBone> {
    model
        .nodes
        .iter()
        .enumerate()
        .map(|(j, n)| {
            let local = mat(&ltb::to_rh_mat(&lta::bind_local(model, j)));
            let (_, rotation, position) = local.to_scale_rotation_translation();
            MapBone {
                name: n.name.clone(),
                parent: n.parent,
                position,
                rotation: rotation.normalize(),
            }
        })
        .collect()
}

/// Every entry of `list` as a looping sequence on the skeleton.
pub fn anim_set(model: &Model, list: &AnimList, bones: &[MapBone]) -> AnimSet {
    anim_set_at(model, list, bones, SAMPLE_FPS)
}

/// `anim_set`, resampled at `fps`.
pub fn anim_set_at(model: &Model, list: &AnimList, bones: &[MapBone], fps: f32) -> AnimSet {
    let defaults: Vec<BonePose> = bones.iter().map(|b| (b.rotation, b.position)).collect();
    let n = model.nodes.len();
    let mut set = AnimSet {
        defaults: defaults.clone(),
        bases: vec![defaults.clone()],
        parents: bones.iter().map(|b| b.parent).collect(),
        bone_names: bones.iter().map(|b| b.name.to_lowercase()).collect(),
        ..default()
    };
    let mut names = HashMap::new();
    for (i, entry) in list.entries.iter().enumerate() {
        let length = entry.length() as f32;
        let (frames, fps) = if entry.animation.keys.len() <= 1 || length <= 0.0 {
            (1, fps)
        } else {
            let frames = (length / 1000.0 * fps).ceil() as usize + 1;
            (frames, (frames - 1) as f32 * 1000.0 / length)
        };
        let tracks = (0..n)
            .map(|j| {
                let (mut rotation, mut position) = (Vec::with_capacity(frames), Vec::with_capacity(frames));
                for f in 0..frames {
                    let t = if frames > 1 { length * f as f32 / (frames - 1) as f32 } else { 0.0 };
                    let (p, q) = lta::sample(entry, j, t);
                    rotation.push(rh_quat(q));
                    position.push(if model.nodes[j].flags & ltb::NODE_ROTATION_ONLY != 0 && model.nodes[j].parent.is_some() {
                        bones[j].position
                    } else {
                        rh(p)
                    });
                }
                Track {
                    bone: j,
                    rotation: Some(rotation),
                    position: Some(position),
                }
            })
            .collect();
        set.animations.push(Animation {
            name: entry.name().to_string(),
            fps,
            frames,
            tracks,
            ..default()
        });
        set.sequences.push(Sequence {
            name: entry.name().to_string(),
            looping: true,
            snap: true,
            grid: (1, 1),
            anims: vec![i],
            bone_weights: vec![1.0; n],
            ..default()
        });
        names.entry(entry.name().to_lowercase()).or_insert(i);
    }
    set.names = names;
    set
}

/// The pose a model is drawn in when nothing animates it: its own
/// animation named `name` (any case), else its first entry, at t = 0
/// (models_ltb.md "Skinning": the engine draws A_j G_j^-1, A the
/// animated pose, not the bind pose). A third-person weapon's `Hand`
/// animation turns its root half a turn, so drawn in the bind pose the
/// gun points backwards.
pub fn rest_entry<'a>(loaded: &'a LoadedModel, name: &str) -> Option<&'a lta::Entry> {
    let list = &loaded.list;
    list.find(name).or((!list.entries.is_empty()).then_some(0)).map(|i| &list.entries[i])
}

/// Each node's global transform (right-handed, units) in `entry`'s pose
/// at t = 0, or at the bind pose without one.
pub fn posed_globals(model: &Model, entry: Option<&lta::Entry>) -> Vec<Mat4> {
    match entry {
        Some(entry) => {
            let pose: Vec<(ltb::Vec3, ltb::Quat)> = (0..model.nodes.len()).map(|j| lta::sample(entry, j, 0.0)).collect();
            lta::globals(model, &pose).iter().map(|g| mat(&ltb::to_rh_mat(g))).collect()
        }
        None => {
            let bones = bones(model);
            let mut out: Vec<Mat4> = Vec::with_capacity(bones.len());
            for b in &bones {
                let local = Mat4::from_rotation_translation(b.rotation, b.position);
                let g = b.parent.and_then(|p| out.get(p)).map_or(local, |parent| *parent * local);
                out.push(g);
            }
            out
        }
    }
}

/// `meshes` (`convert`'s, `scale` metres per unit) skinned once into
/// `entry`'s pose at t = 0, each vertex by its joints and weights: the
/// model as drawn in that pose, in model space.
pub fn pose_meshes(model: &Model, entry: &lta::Entry, meshes: &mut [MapMesh], scale: f32) {
    let pose: Vec<(ltb::Vec3, ltb::Quat)> = (0..model.nodes.len()).map(|j| lta::sample(entry, j, 0.0)).collect();
    let skins: Vec<Mat4> = lta::skin_matrices(model, &lta::globals(model, &pose))
        .iter()
        .map(|k| {
            let k = mat(&ltb::to_rh_mat(k));
            // Metres in and out: the translation scales.
            Mat4::from_scale(Vec3::splat(scale)) * k * Mat4::from_scale(Vec3::splat(1.0 / scale))
        })
        .collect();
    for mesh in meshes {
        for (v, p) in mesh.positions.iter_mut().enumerate() {
            let (joints, weights) = (mesh.joints.get(v), mesh.joint_weights.get(v));
            let (Some(joints), Some(weights)) = (joints, weights) else { continue };
            let mut out = Vec3::ZERO;
            for (j, w) in joints.iter().zip(weights).filter(|(_, w)| **w > 0.0) {
                out += skins.get(*j as usize).copied().unwrap_or(Mat4::IDENTITY).transform_point3(Vec3::from(*p)) * *w;
            }
            *p = out.to_array();
            if let Some(n) = mesh.normals.get_mut(v) {
                let mut sum = Vec3::ZERO;
                for (j, w) in joints.iter().zip(weights).filter(|(_, w)| **w > 0.0) {
                    sum += skins.get(*j as usize).copied().unwrap_or(Mat4::IDENTITY).transform_vector3(Vec3::from(*n)) * *w;
                }
                *n = sum.normalize_or_zero().to_array();
            }
        }
    }
}

/// The model's meshes (LOD 0 of each piece, metres, right-handed, skinned
/// to its nodes) and its rig; `texture` gives a piece's material name
/// from its LOD (empty: none).
pub fn convert(loaded: &LoadedModel, scale: f32, texture: &dyn Fn(&ltb::Lod) -> String) -> MapModel {
    convert_at(loaded, scale, texture, SAMPLE_FPS)
}

/// `convert`, its animations resampled at `fps`.
pub fn convert_at(loaded: &LoadedModel, scale: f32, texture: &dyn Fn(&ltb::Lod) -> String, fps: f32) -> MapModel {
    let model = &loaded.model;
    let bones = bones(model);
    let animations = Arc::new(anim_set_at(model, &loaded.list, &bones, fps));
    let mut meshes = Vec::new();
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for piece in &model.pieces {
        let Some(lod) = piece.lods.first() else { continue };
        let Some(g) = lod.mesh.geometry() else { continue };
        let positions: Vec<[f32; 3]> = g.positions.iter().map(|&p| (rh(p) * scale).to_array()).collect();
        for p in &positions {
            lo = lo.min(Vec3::from(*p));
            hi = hi.max(Vec3::from(*p));
        }
        let influences = lod.mesh.vertex_influences();
        meshes.push(MapMesh {
            material: texture(lod),
            positions,
            normals: g.normals.iter().map(|&n| rh(n).to_array()).collect(),
            uvs: g.uvs.clone(),
            joints: influences.iter().map(|v| v.map(|(n, _)| n as u16)).collect(),
            joint_weights: influences.iter().map(|v| v.map(|(_, w)| w)).collect(),
            indices: g
                .triangles
                .iter()
                .flat_map(|&t| ltb::to_rh_triangle(t).map(u32::from))
                .collect(),
            color: [200; 3],
            ..default()
        });
    }
    if lo.x > hi.x {
        (lo, hi) = (Vec3::ZERO, Vec3::ZERO);
    }
    MapModel {
        meshes,
        bounds: (lo, hi),
        rig: Some(Arc::new(MapRig {
            bones,
            root: Transform::from_scale(Vec3::splat(scale)),
            animations,
        })),
        ..default()
    }
}

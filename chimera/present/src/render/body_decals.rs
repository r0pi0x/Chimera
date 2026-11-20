//! Decals on character bodies (model decals) for any game: a
//! `map::PlaceBodyDecal` (a game's bullet, pellet or swing on a character)
//! puts a decal of its group on the drawn body where the shot meets it, and
//! it follows the animated skin (specs/tf2/effects.md 8b: TF2's
//! `Impact.Flesh`; CS:S's and HL2:DM's impact on a player). The body's
//! shown parts are skinned on the CPU once per hit body and frame (the
//! main world keeps no mesh data: from `CharacterModels`), the ray from the
//! shot's start through the hit picks the nearest triangle facing it, and
//! the triangles facing the shot within the decal's reach of that point
//! are clipped to its rectangle across the shot. Each clipped vertex keeps
//! its barycentric place in its triangle, so its bind-pose position,
//! normal and joint weights (the corners' blended, the four largest) come
//! from the bind mesh: the decal is bind-space geometry skinned by the
//! body's own joints (`SkinnedMesh`), one mesh per decal material per
//! body, a child of the body. The GPU skins it with the body; nothing is
//! done per frame; it goes with the body to a ragdoll (CS:S, HL2:DM)
//! unless its character has `BodyDecalsOffRagdoll` (TF2). Per body at most
//! `BodyDecalSettings::cap` (`r_maxmodeldecal`, 50: effects.md Open
//! questions 15's belief), the oldest replaced; `r_drawmodeldecals 0`
//! hides them. None on a character with `NoBodyDecals` (whose arrival
//! clears them), on the shooter himself, nor on a teammate with friendly
//! fire off. Cleared by `ClearBodyDecals` and when the character lives
//! again (`Dead` removed).
//!
//! Known shortcuts (docs/tech-debt.md): Body decals: projected onto level
//! of detail 0 and lifted 0.25 units along the bind normal, so a coarser
//! level may hide or float them a little; face flexes don't move them; a
//! vertex's joint weights are its triangle corners' blended (top four),
//! not exactly the skinned triangle's surface at bent joints; no
//! per-hitbox surface (HL2:DM police's `no_decal` bodies take them). Why
//! it's OK for now: hits read as blood on the body where they landed;
//! close-ups at joints unchecked against the games.

use std::collections::VecDeque;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues, skinning::SkinnedMeshInverseBindposes},
    prelude::*,
};

use super::{
    bodies::{CharacterBody, PlayModels},
    decal::{DecalAssets, DecalMaterial, basis, clip},
    fog,
};
use crate::core::{Dead, FriendlyFire, Team};
use crate::map::decal::MapDecal;
use crate::map::{
    BodyDecalsOffRagdoll, CharacterModels, ClearBodyDecals, MapMesh, NoBodyDecals, PlaceBodyDecal, Ragdoll,
};

const UNIT: f32 = 0.0254;
/// Lift off the body along its bind normal, meters (skinning blends a
/// decal vertex's weights, so it sits a little off the skinned surface at
/// bent joints).
pub const BODY_LIFT: f32 = 0.25 * UNIT;
/// How far past the hit the shot's ray looks for the body's mesh: 8 units,
/// lengthened by 10 % (effects.md 8b), meters.
const PAST_HIT: f32 = 8.0 * 1.1 * UNIT;
/// A triangle takes the decal only if it faces the shot at least this
/// much (cosine).
const FACING: f32 = 0.05;

/// `r_drawmodeldecals` (`draw`, 1: drawn) and `r_maxmodeldecal` (`cap`,
/// decals per body, 50; the oldest goes first).
#[derive(Resource, Clone, Copy, Debug)]
pub struct BodyDecalSettings {
    pub draw: u8,
    pub cap: usize,
}

impl Default for BodyDecalSettings {
    fn default() -> Self {
        Self { draw: 1, cap: 50 }
    }
}

/// One decal's geometry in the body's bind space, with its joints.
#[derive(Clone, Debug, Default)]
pub struct Piece {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Place within the decal (0..1), for the shader's edge fade.
    pub local: Vec<[f32; 2]>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

/// A body's decals, oldest first, and the drawn meshes (one per
/// material: `BodyDecalLayer`s, its children).
#[derive(Component, Default)]
pub struct BodyDecals {
    pub placed: VecDeque<(Handle<DecalMaterial>, Piece)>,
    layers: Vec<(Handle<DecalMaterial>, Entity)>,
}

impl BodyDecals {
    /// How many decals the body has.
    pub fn len(&self) -> usize {
        self.placed.len()
    }

    pub fn is_empty(&self) -> bool {
        self.placed.is_empty()
    }
}

/// A body's decal mesh (one material's).
#[derive(Component)]
pub struct BodyDecalLayer;

/// Skin matrices (joint's drawn transform × inverse bind pose).
fn skin_matrices(joints: &[Entity], inverse: &[Mat4], globals: &Query<&GlobalTransform>) -> Option<Vec<Mat4>> {
    joints
        .iter()
        .zip(inverse)
        .map(|(j, ib)| globals.get(*j).ok().map(|g| Mat4::from(g.affine()) * *ib))
        .collect()
}

/// A part's vertices where the pose puts them (world space).
pub fn pose_part(mesh: &MapMesh, skin: &[Mat4]) -> Vec<Vec3> {
    if mesh.joints.len() != mesh.positions.len() || mesh.joint_weights.len() != mesh.positions.len() {
        return Vec::new();
    }
    mesh.positions
        .iter()
        .zip(&mesh.joints)
        .zip(&mesh.joint_weights)
        .map(|((p, j), w)| {
            let p = Vec3::from(*p);
            let mut out = Vec3::ZERO;
            for k in 0..4 {
                if w[k] > 0.0
                    && let Some(m) = skin.get(j[k] as usize)
                {
                    out += m.transform_point3(p) * w[k];
                }
            }
            out
        })
        .collect()
}

/// Where the ray from `from` along `dir` (unit) first meets a triangle
/// facing it, within `max_t`.
fn ray_hit(parts: &[(&MapMesh, &[Vec3])], from: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
    let mut best: Option<f32> = None;
    for (mesh, posed) in parts {
        if posed.is_empty() {
            continue;
        }
        for t in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [posed[t[0] as usize], posed[t[1] as usize], posed[t[2] as usize]];
            let (e1, e2) = (b - a, c - a);
            if e1.cross(e2).dot(dir) >= 0.0 {
                continue;
            }
            // Möller-Trumbore.
            let p = dir.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-12 {
                continue;
            }
            let inv = 1.0 / det;
            let s = from - a;
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let v = dir.dot(q) * inv;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let d = e2.dot(q) * inv;
            if d >= 0.0 && d <= max_t && best.is_none_or(|b| d < b) {
                best = Some(d);
            }
        }
    }
    best
}

/// The joints and weights at barycentric `b` of corners `corners`: their
/// weights blended, the four largest, summing to 1.
fn blend_joints(mesh: &MapMesh, corners: [usize; 3], b: Vec3) -> ([u16; 4], [f32; 4]) {
    let mut acc: Vec<(u16, f32)> = Vec::with_capacity(12);
    for (c, f) in corners.into_iter().zip(b.to_array()) {
        for k in 0..4 {
            let w = mesh.joint_weights[c][k] * f;
            if w <= 0.0 {
                continue;
            }
            let j = mesh.joints[c][k];
            match acc.iter_mut().find(|(i, _)| *i == j) {
                Some(e) => e.1 += w,
                None => acc.push((j, w)),
            }
        }
    }
    acc.sort_by(|x, y| y.1.total_cmp(&x.1));
    acc.truncate(4);
    let total: f32 = acc.iter().map(|(_, w)| w).sum();
    let (mut joints, mut weights) = ([0u16; 4], [0f32; 4]);
    for (k, (j, w)) in acc.iter().enumerate() {
        joints[k] = *j;
        weights[k] = if total > 0.0 { w / total } else { 0.0 };
    }
    if total <= 0.0 {
        weights[0] = 1.0;
    }
    (joints, weights)
}

/// The decal's geometry on the posed body (`parts`: each shown part's bind
/// mesh and its posed vertices, `pose_part`) for a shot from `from` along
/// `dir` that hit at `point`; `right` and `down` across the shot. None
/// when the ray from `from` through `point` (8 units past it, + 10 %)
/// meets no part of the body.
pub fn project_on_body(
    parts: &[(&MapMesh, &[Vec3])],
    decal: &MapDecal,
    from: Vec3,
    point: Vec3,
    right: Vec3,
    down: Vec3,
) -> Option<Piece> {
    let dir = (point - from).normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    // Only where the shot's ray meets the body (effects.md 8b, 10.1): a
    // point off it (TF2's melee box centre left in the air) leaves none,
    // not one on whatever part lies near that point.
    let max_t = from.distance(point) + PAST_HIT;
    let centre = from + dir * ray_hit(parts, from, dir, max_t)?;
    let depth = decal.size.max_element() * 0.5;
    let uv = |p: Vec3| {
        let d = p - centre;
        Vec2::new(d.dot(right) / decal.size.x + 0.5, d.dot(down) / decal.size.y + 0.5)
    };
    let mut out = Piece::default();
    for (mesh, posed) in parts {
        if posed.is_empty() {
            continue;
        }
        for t in mesh.indices.chunks_exact(3) {
            let corners = [t[0] as usize, t[1] as usize, t[2] as usize];
            let [a, b, c] = corners.map(|i| posed[i]);
            let n = (b - a).cross(c - a).normalize_or_zero();
            if n.dot(-dir) < FACING {
                continue;
            }
            if [a, b, c].iter().all(|p| (*p - centre).dot(dir).abs() > depth) {
                continue;
            }
            let tuv = [uv(a), uv(b), uv(c)];
            if tuv.iter().all(|t| t.x < 0.0)
                || tuv.iter().all(|t| t.x > 1.0)
                || tuv.iter().all(|t| t.y < 0.0)
                || tuv.iter().all(|t| t.y > 1.0)
            {
                continue;
            }
            // The polygon carries barycentric places for positions.
            let poly = clip(vec![(Vec3::X, tuv[0]), (Vec3::Y, tuv[1]), (Vec3::Z, tuv[2])]);
            if poly.len() < 3 {
                continue;
            }
            let bind = corners.map(|i| Vec3::from(mesh.positions[i]));
            let face = (bind[1] - bind[0]).cross(bind[2] - bind[0]).normalize_or_zero();
            let normal = |i: usize| mesh.normals.get(i).map_or(face, |n| Vec3::from(*n));
            let base = out.positions.len() as u32;
            for (bary, t) in &poly {
                let p = bind[0] * bary.x + bind[1] * bary.y + bind[2] * bary.z;
                let nn = (normal(corners[0]) * bary.x + normal(corners[1]) * bary.y + normal(corners[2]) * bary.z)
                    .normalize_or(face);
                let (joints, weights) = blend_joints(mesh, corners, *bary);
                out.positions.push((p + nn * BODY_LIFT).to_array());
                out.normals.push(nn.to_array());
                out.uvs.push((decal.uv_min + (decal.uv_max - decal.uv_min) * *t).to_array());
                out.local.push(t.to_array());
                out.joints.push(joints);
                out.weights.push(weights);
            }
            for k in 1..poly.len() as u32 - 1 {
                out.indices.extend([base, base + k, base + k + 1]);
            }
        }
    }
    (!out.indices.is_empty()).then_some(out)
}

/// The skinned mesh of `pieces`.
fn layer_mesh<'a>(pieces: impl Iterator<Item = &'a Piece>) -> Mesh {
    let mut all = Piece::default();
    for p in pieces {
        let base = all.positions.len() as u32;
        all.positions.extend_from_slice(&p.positions);
        all.normals.extend_from_slice(&p.normals);
        all.uvs.extend_from_slice(&p.uvs);
        all.local.extend_from_slice(&p.local);
        all.joints.extend_from_slice(&p.joints);
        all.weights.extend_from_slice(&p.weights);
        all.indices.extend(p.indices.iter().map(|i| i + base));
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, all.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, all.normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, all.uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, all.local);
    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(all.joints));
    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, all.weights);
    mesh.insert_indices(Indices::U32(all.indices));
    // Culled by where the pose puts it, as the body's parts.
    let _ = mesh.generate_skinned_mesh_bounds();
    mesh
}

/// Draw `body`'s decals again: one mesh per material, layers no decal
/// uses any more removed.
fn rebuild(
    body: Entity,
    decals: &mut BodyDecals,
    skin: &bevy::mesh::skinning::SkinnedMesh,
    visible: bool,
    meshes: &mut Assets<Mesh>,
    commands: &mut Commands,
) {
    let mut kept = Vec::new();
    for (material, layer) in std::mem::take(&mut decals.layers) {
        if decals.placed.iter().any(|(m, _)| *m == material) {
            kept.push((material, layer));
        } else {
            commands.entity(layer).try_despawn();
        }
    }
    decals.layers = kept;
    let mut materials: Vec<Handle<DecalMaterial>> = Vec::new();
    for (m, _) in &decals.placed {
        if !materials.contains(m) {
            materials.push(m.clone());
        }
    }
    for material in materials {
        let mesh = meshes.add(layer_mesh(decals.placed.iter().filter(|(m, _)| *m == material).map(|(_, p)| p)));
        match decals.layers.iter().find(|(m, _)| *m == material) {
            Some((_, layer)) => {
                commands.entity(*layer).insert(Mesh3d(mesh));
            }
            None => {
                let layer = commands
                    .spawn((
                        Name::new("Body decals"),
                        BodyDecalLayer,
                        Mesh3d(mesh),
                        MeshMaterial3d(material.clone()),
                        skin.clone(),
                        bevy::camera::visibility::DynamicSkinnedMeshBounds,
                        Transform::default(),
                        if visible { Visibility::Inherited } else { Visibility::Hidden },
                        ChildOf(body),
                    ))
                    .id();
                decals.layers.push((material, layer));
            }
        }
    }
}

fn clear(decals: &mut BodyDecals, commands: &mut Commands) {
    for (_, layer) in decals.layers.drain(..) {
        commands.entity(layer).try_despawn();
    }
    decals.placed.clear();
}

/// Bodies' decals go: `ClearBodyDecals`, a character given `NoBodyDecals`,
/// one living again (`Dead` removed), and a ragdoll taking over the body
/// of a character with `BodyDecalsOffRagdoll`.
#[allow(clippy::too_many_arguments)]
pub fn clear_body_decals(
    mut asks: MessageReader<ClearBodyDecals>,
    mut alive: RemovedComponents<Dead>,
    hidden: Query<Entity, Added<NoBodyDecals>>,
    ragdolls: Query<&Ragdoll, Changed<Ragdoll>>,
    off: Query<(), With<BodyDecalsOffRagdoll>>,
    children: Query<&Children>,
    mut bodies: Query<&mut BodyDecals>,
    mut commands: Commands,
) {
    let mut characters: Vec<Entity> = asks.read().map(|c| c.0).collect();
    characters.extend(alive.read());
    characters.extend(hidden.iter());
    let mut drawn: Vec<Entity> = characters
        .iter()
        .filter_map(|c| children.get(*c).ok())
        .flat_map(|c| c.iter())
        .filter(|e| bodies.contains(*e))
        .collect();
    drawn.extend(ragdolls.iter().filter(|r| off.contains(r.owner)).filter_map(|r| r.visual));
    for body in drawn {
        if let Ok(mut d) = bodies.get_mut(body)
            && !d.is_empty()
        {
            clear(&mut d, &mut commands);
        }
    }
}

/// Place the body decals asked for this frame (module doc).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn place_body_decals(
    mut asks: MessageReader<PlaceBodyDecal>,
    settings: Option<Res<BodyDecalSettings>>,
    assets: Option<ResMut<DecalAssets>>,
    models: Option<Res<CharacterModels>>,
    play: Option<Res<PlayModels>>,
    characters: Query<(Option<&Children>, Has<NoBodyDecals>, Option<&Team>)>,
    mut bodies: Query<(&CharacterBody, Option<&mut BodyDecals>)>,
    globals: Query<&GlobalTransform>,
    (bindposes, meshes, materials, fog, friendly_fire): (
        Option<Res<Assets<SkinnedMeshInverseBindposes>>>,
        Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<DecalMaterial>>>,
        Option<Res<fog::SceneFog>>,
        Option<Res<FriendlyFire>>,
    ),
    mut layers: Query<&mut Visibility, With<BodyDecalLayer>>,
    mut commands: Commands,
) {
    let settings = settings.map_or_else(BodyDecalSettings::default, |s| {
        if s.is_changed() {
            let want = if s.draw != 0 { Visibility::Inherited } else { Visibility::Hidden };
            for mut v in &mut layers {
                if *v != want {
                    *v = want;
                }
            }
        }
        *s
    });
    let (Some(mut assets), Some(models), Some(play), Some(bindposes), Some(mut meshes), Some(mut materials)) =
        (assets, models, play, bindposes, meshes, materials)
    else {
        asks.clear();
        return;
    };
    if settings.cap == 0 {
        asks.clear();
        return;
    }
    let friendly_fire = friendly_fire.is_some_and(|f| f.0 != 0);
    // Each hit body's pose, once a frame.
    let mut posed: Vec<(Entity, Vec<(usize, Vec<Vec3>)>)> = Vec::new();
    let mut changed: Vec<(Entity, bevy::mesh::skinning::SkinnedMesh)> = Vec::new();
    let mut fresh: Vec<(Entity, bevy::mesh::skinning::SkinnedMesh, BodyDecals)> = Vec::new();
    for ask in asks.read() {
        let Ok((Some(children), hidden, team)) = characters.get(ask.target) else { continue };
        let teammates = matches!((team, characters.get(ask.owner).ok().and_then(|c| c.2)), (Some(a), Some(b)) if a == b && a.0 != 0);
        if hidden || ask.owner == ask.target || (teammates && !friendly_fire) {
            continue;
        }
        let Some(body) = children.iter().find(|c| bodies.contains(*c)) else { continue };
        let Ok((drawn, _)) = bodies.get(body) else { continue };
        let (Some(model), Some(built)) = (models.0.get(drawn.model), play.body(drawn.model)) else { continue };
        let Some(inverse) = bindposes.get(&built.bindposes) else { continue };
        let Some(skin) = skin_matrices(&drawn.joints, inverse, &globals) else { continue };
        let Some(index) = assets.pick(&ask.group) else { continue };
        let decal = assets.data.decals[index].clone();
        let Some(material) = assets.shared_material(index, &mut materials, fog.as_deref()) else { continue };
        let shown: Vec<usize> = (0..model.model.meshes.len()).filter(|k| built.shows(*k, drawn.body)).collect();
        let pose = match posed.iter().position(|(b, _)| *b == body) {
            Some(i) => i,
            None => {
                let parts = shown.iter().map(|k| (*k, pose_part(&model.model.meshes[*k], &skin))).collect();
                posed.push((body, parts));
                posed.len() - 1
            }
        };
        let parts: Vec<(&MapMesh, &[Vec3])> = posed[pose]
            .1
            .iter()
            .map(|(k, p)| (&model.model.meshes[*k], p.as_slice()))
            .collect();
        let dir = (ask.point - ask.from).normalize_or_zero();
        let (mut right, mut down) = basis(-dir);
        let turn = Quat::from_axis_angle(dir, assets.random() * std::f32::consts::TAU);
        (right, down) = (turn * right, turn * down);
        let Some(piece) = project_on_body(&parts, &decal, ask.from, ask.point, right, down) else { continue };
        let skin = bevy::mesh::skinning::SkinnedMesh { inverse_bindposes: built.bindposes.clone(), joints: drawn.joints.clone() };
        // A body's first decal: its component comes with the commands.
        let list = match bodies.get_mut(body) {
            Ok((_, Some(d))) => {
                if !changed.iter().any(|(b, _)| *b == body) {
                    changed.push((body, skin));
                }
                d.into_inner()
            }
            _ => {
                let i = match fresh.iter().position(|(b, ..)| *b == body) {
                    Some(i) => i,
                    None => {
                        fresh.push((body, skin, BodyDecals::default()));
                        fresh.len() - 1
                    }
                };
                &mut fresh[i].2
            }
        };
        list.placed.push_back((material, piece));
        while list.placed.len() > settings.cap {
            list.placed.pop_front();
        }
    }
    let visible = settings.draw != 0;
    for (body, skin) in changed {
        if let Ok((_, Some(mut decals))) = bodies.get_mut(body) {
            rebuild(body, &mut decals, &skin, visible, &mut meshes, &mut commands);
        }
    }
    for (body, skin, mut decals) in fresh {
        rebuild(body, &mut decals, &skin, visible, &mut meshes, &mut commands);
        commands.entity(body).insert(decals);
    }
}

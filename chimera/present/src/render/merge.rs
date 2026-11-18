//! Brush entities drawn together: maps with hundreds of breakables or
//! doors (a minigame map's wall of blocks) gave each entity a mesh per
//! material, and each mesh is a draw and a main-world entity of its own.
//! While a brush entity is where the map put it, whole and shown, its
//! opaque and alpha-tested meshes are drawn as part of a combined mesh
//! per material and chunk (`MergedChunk`), and its own meshes
//! (`MergedPiece`) stay hidden. When it moves, breaks, is removed or
//! turned off by the logic (`LogicHidden`) or hidden by its game
//! (`Visibility::Hidden` on the node), or shows broken panes
//! (`BrushPanes`), its triangles leave the combined mesh and its own
//! meshes are drawn again; when it is back as it started (a closed door,
//! a round restart) it rejoins. A node with a render colour other than
//! white (`BrushTint`) draws its own meshes too. The node itself (collider, logic, culling,
//! decals) is unchanged either way.
//!
//! Brush entities drawn merged (any game): while a brush entity node is at its
//! spawn pose, shown (neither `LogicHidden` nor `Visibility::Hidden`) and without broken panes, its opaque and alpha-tested
//! meshes are drawn through combined meshes per material and 512-unit chunk
//! (`MergedChunk`, under the map's root of static parts, culled by the clusters
//! of their parts) and its own meshes (`MergedPiece`) are hidden;
//! `sync_merged_brushes` splits a node out (its index ranges dropped from the
//! combined meshes, its own meshes shown) when it moves, is removed or broken
//! (`vis::LogicHidden`) or shows panes, and merges it back when it is home
//! again; `CHIMERA_MERGE_BRUSHES=0` turns it off.
//!
//! A node copied at run time (`map::copies`) draws its own meshes, never
//! merged (`show_copied_pieces`, a `map::MapHooks::copied` hook).
//!
//! Known shortcuts (docs/tech-debt.md): Merged brush entities (`render::merge`)
//! keep their own meshes spawned and hidden (main-world entities that still
//! cost visibility checks), and rewriting a combined mesh's indices when one
//! splits out makes Bevy re-check every mesh entity's pipeline that frame;
//! decals on brush entities go at a round restart (we assume CS:S re-creates
//! brush entities then; not measured) and blended brush meshes (glass) take
//! none. Why it's OK for now: The draw count was the problem (1768 -> 58 at the
//! lego blocks room); splits happen per break or door move, not per frame.

use std::{collections::HashMap, ops::Range};

use bevy::{mesh::Indices, prelude::*};

use crate::map::{
    BrushPanes, LogicHidden, MapMesh, VisClusters,
    copies::MapCopy,
    vis::{self, MapVisibility},
};

/// A brush entity node with meshes in combined chunks.
#[derive(Component, Clone, Debug)]
pub struct MergedBrush {
    /// Where the map placed it: it is drawn merged only while here.
    pub home: Transform,
    /// The combined meshes holding its triangles.
    pub chunks: Vec<Entity>,
    /// Drawn through the chunks (its own meshes hidden).
    pub merged: bool,
}

/// One of a merged brush entity's own meshes (hidden while it is merged).
#[derive(Component, Clone, Copy, Debug)]
pub struct MergedPiece;

/// A combined mesh of brush entities sharing a material in one chunk:
/// each node's index range in the full index list. Hidden (`LogicHidden`)
/// while none of them is merged.
#[derive(Component, Clone, Debug)]
pub struct MergedChunk {
    pub parts: Vec<(Entity, Range<usize>)>,
    pub indices: Vec<u32>,
}

/// Whether a brush entity mesh may be drawn merged: in the world (not the
/// 3D skybox, not water) and not blended (blended meshes are sorted one by
/// one; merged, their order would change).
pub fn mergeable(m: &MapMesh) -> bool {
    !m.skybox
        && m.water.is_none()
        && !matches!(m.alpha, crate::map::MapAlpha::Blend | crate::map::MapAlpha::Add)
        && m.normals.len() == m.positions.len()
        && m.uvs.len() == m.positions.len()
}

/// Off with `CHIMERA_MERGE_BRUSHES=0` (every brush entity draws its own
/// meshes), for measurements.
pub fn enabled() -> bool {
    chimera_core::env::var("CHIMERA_MERGE_BRUSHES").map_or(true, |v| v != "0")
}

/// Collects brush entity meshes into combined ones while the map spawns.
/// `K` is the material (with whatever else must match to draw together).
pub struct Merger<K> {
    size: f32,
    groups: Vec<(K, IVec3, MapMesh, Vec<(Entity, Range<usize>)>, Vec<(Vec3, Vec3)>)>,
    index: HashMap<(K, IVec3), usize>,
}

impl<K: Clone + Eq + std::hash::Hash> Merger<K> {
    pub fn new(size: f32) -> Self {
        Self {
            size,
            groups: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// Add node `node`'s mesh `m` (node space) placed at `home`, grouped
    /// by `key` and the chunk of the node's `centre` (world).
    pub fn add(&mut self, key: K, node: Entity, home: &Transform, centre: Vec3, m: &MapMesh) {
        let cell = (centre / self.size).floor().as_ivec3();
        let i = *self.index.entry((key.clone(), cell)).or_insert_with(|| {
            let mut template = m.clone();
            template.entity = None;
            template.positions.clear();
            template.normals.clear();
            template.uvs.clear();
            template.lightmap_uvs.clear();
            template.blend_weights.clear();
            template.uvs2.clear();
            template.indices.clear();
            self.groups.push((key, cell, template, Vec::new(), Vec::new()));
            self.groups.len() - 1
        });
        let (_, _, out, parts, boxes) = &mut self.groups[i];
        let base = out.positions.len() as u32;
        let start = out.indices.len();
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        for p in &m.positions {
            let w = home.transform_point(Vec3::from(*p));
            lo = lo.min(w);
            hi = hi.max(w);
            out.positions.push(w.to_array());
        }
        for n in &m.normals {
            out.normals.push((home.rotation * Vec3::from(*n)).to_array());
        }
        out.uvs.extend_from_slice(&m.uvs);
        if m.lightmap_uvs.len() == m.positions.len() {
            out.lightmap_uvs.extend_from_slice(&m.lightmap_uvs);
        }
        if m.blend_weights.len() == m.positions.len() {
            out.blend_weights.extend_from_slice(&m.blend_weights);
        }
        if m.uvs2.len() == m.positions.len() {
            out.uvs2.extend_from_slice(&m.uvs2);
        }
        out.indices.extend(m.indices.iter().map(|i| i + base));
        match parts.last_mut() {
            Some((n, r)) if *n == node && r.end == start => r.end = out.indices.len(),
            _ => parts.push((node, start..out.indices.len())),
        }
        if lo.x <= hi.x {
            boxes.push((lo, hi));
        }
    }

    /// The combined meshes: (key, mesh recentred, its centre, the nodes'
    /// index ranges, the clusters its parts touch).
    pub fn finish(
        self,
        vis: Option<&MapVisibility>,
    ) -> Vec<(K, MapMesh, Vec3, Vec<(Entity, Range<usize>)>, Vec<u32>)> {
        self.groups
            .into_iter()
            .map(|(key, _, mut mesh, parts, boxes)| {
                let clusters = match vis {
                    Some(v) => {
                        let mut out = Vec::new();
                        for (lo, hi) in boxes {
                            v.clusters_in_box(lo, hi, &mut out);
                        }
                        out.sort_unstable();
                        out.dedup();
                        out
                    }
                    None => Vec::new(),
                };
                let centre = vis::recentre(&mut mesh);
                (key, mesh, centre, parts, clusters)
            })
            .collect()
    }
}

/// Close enough to where the map put it to draw merged.
fn at_home(t: &Transform, home: &Transform) -> bool {
    t.translation.distance_squared(home.translation) < 1e-8 && t.rotation.angle_between(home.rotation) < 1e-4
}

/// Each frame: brush entities that left their merged state (moved,
/// removed, hidden, broken panes) are drawn from their own meshes and
/// their triangles leave the combined meshes; those back home rejoin. A
/// node its game hides with `Visibility::Hidden` (Combat Arms' objects:
/// a world model whose `Visible` is off, a mode's other bridge) leaves
/// them too: merged, it was drawn anyway (two bridges in one place on
/// NEPTUNE, z-fighting).
#[allow(clippy::type_complexity)]
pub fn sync_merged_brushes(
    mut nodes: Query<(
        &Transform,
        Has<LogicHidden>,
        Has<BrushPanes>,
        &mut MergedBrush,
        &Children,
        Option<&crate::map::BrushTint>,
        Option<&Visibility>,
    ), (Without<MergedPiece>, Without<MergedChunk>)>,
    mut pieces: Query<&mut Visibility, (With<MergedPiece>, Without<MergedChunk>)>,
    mut chunks: Query<(
        &MergedChunk,
        &Mesh3d,
        &mut Visibility,
        Option<&VisClusters>,
        Has<LogicHidden>,
    )>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    mut commands: Commands,
    mut dirty: Local<Vec<Entity>>,
) {
    let Some(mut meshes) = meshes else { return };
    dirty.clear();
    for (t, hidden, panes, mut merged, children, tint, shown) in &mut nodes {
        // A tinted node draws its own (tinted) meshes; a hidden one none.
        let hidden = hidden || shown == Some(&Visibility::Hidden);
        let want = !hidden && !panes && at_home(t, &merged.home) && tint.is_none_or(|t| t.is_white());
        if merged.merged == want {
            continue;
        }
        merged.merged = want;
        dirty.extend(merged.chunks.iter().copied());
        for child in children.iter() {
            if let Ok(mut v) = pieces.get_mut(child) {
                v.set_if_neq(if want {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                });
            }
        }
    }
    dirty.sort_unstable();
    dirty.dedup();
    for chunk in dirty.iter().copied() {
        let Ok((c, mesh, mut visibility, clusters, was_hidden)) = chunks.get_mut(chunk) else {
            continue;
        };
        let indices: Vec<u32> = c
            .parts
            .iter()
            .filter(|(n, _)| nodes.get(*n).is_ok_and(|(_, _, _, m, _, _, _)| m.merged))
            .flat_map(|(_, r)| c.indices[r.clone()].iter().copied())
            .collect();
        if indices.is_empty() {
            // Nothing left to draw: hidden, its last indices kept (an
            // empty index buffer isn't worth a special case).
            if !was_hidden {
                commands.entity(chunk).insert(LogicHidden);
            }
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }
        if was_hidden {
            commands.entity(chunk).remove::<LogicHidden>();
            let culled = clusters.is_some_and(|v| !v.potentially_visible);
            visibility.set_if_neq(if culled {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
        }
        if let Some(mut m) = meshes.get_mut(&mesh.0) {
            m.insert_indices(Indices::U32(indices));
        }
    }
}
/// A node copied at run time (`map::copies::copy_node`, a
/// `map::MapHooks::copied` hook): it and the brush entities under it are
/// never merged, and the pieces the original draws merged are drawn by
/// themselves.
pub fn show_copied_pieces(world: &mut World, copy: Entity) {
    debug_assert!(world.get::<MapCopy>(copy).is_some());
    let mut stack = vec![copy];
    while let Some(e) = stack.pop() {
        let Ok(mut node) = world.get_entity_mut(e) else { continue };
        node.remove::<MergedBrush>();
        if node.contains::<MergedPiece>() {
            node.remove::<MergedPiece>().insert(Visibility::Inherited);
        }
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MapHooks, copies::copy_node};

    /// A copy of a merged brush entity draws its own meshes: it isn't
    /// merged and its pieces show; the original stays merged.
    #[test]
    fn copies_draw_their_own_meshes() {
        let mut world = World::new();
        world.insert_resource(MapHooks {
            copied: vec![show_copied_pieces],
            ..default()
        });
        let node = world
            .spawn((
                Transform::default(),
                MergedBrush {
                    home: Transform::default(),
                    chunks: Vec::new(),
                    merged: true,
                },
            ))
            .id();
        let piece = world.spawn((MergedPiece, Visibility::Hidden, ChildOf(node))).id();
        let copy = copy_node(&mut world, node, Transform::from_xyz(1.0, 0.0, 0.0)).unwrap();
        assert!(world.get::<MergedBrush>(copy).is_none());
        let copied = world.get::<Children>(copy).unwrap()[0];
        assert!(world.get::<MergedPiece>(copied).is_none());
        assert_eq!(world.get::<Visibility>(copied), Some(&Visibility::Inherited));
        assert!(world.get::<MergedBrush>(node).is_some());
        assert!(world.get::<MergedPiece>(piece).is_some());
    }

    /// A node its game hides (`Visibility::Hidden`, as Combat Arms'
    /// objects hide a world model whose `Visible` is off) leaves the
    /// combined mesh, which then draws none of its triangles; shown
    /// again, it rejoins.
    #[test]
    fn a_hidden_node_leaves_the_combined_mesh() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>().add_systems(Update, sync_merged_brushes);
        let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 6]);
        mesh.insert_indices(Indices::U32(vec![0, 1, 2, 3, 4, 5]));
        let handle = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
        let world = app.world_mut();
        let node = |world: &mut World| {
            let n = world
                .spawn((Transform::default(), Visibility::Inherited, MergedBrush { home: Transform::default(), chunks: Vec::new(), merged: true }))
                .id();
            let piece = world.spawn((MergedPiece, Visibility::Hidden, ChildOf(n))).id();
            (n, piece)
        };
        let (a, _) = node(world);
        let (b, b_piece) = node(world);
        let chunk = world
            .spawn((
                MergedChunk { parts: vec![(a, 0..3), (b, 3..6)], indices: vec![0, 1, 2, 3, 4, 5] },
                Mesh3d(handle.clone()),
                Visibility::Inherited,
            ))
            .id();
        for n in [a, b] {
            world.get_mut::<MergedBrush>(n).unwrap().chunks = vec![chunk];
        }
        let drawn = |app: &App| -> Vec<u32> {
            match app.world().resource::<Assets<Mesh>>().get(&handle).unwrap().indices() {
                Some(Indices::U32(i)) => i.clone(),
                _ => panic!("no indices"),
            }
        };
        app.update();
        assert_eq!(drawn(&app), [0, 1, 2, 3, 4, 5]);
        app.world_mut().entity_mut(b).insert(Visibility::Hidden);
        app.update();
        assert!(!app.world().get::<MergedBrush>(b).unwrap().merged);
        assert_eq!(drawn(&app), [0, 1, 2], "the hidden node's triangles leave");
        assert_eq!(app.world().get::<Visibility>(b_piece), Some(&Visibility::Inherited), "its own meshes, hidden with it");
        app.world_mut().entity_mut(b).insert(Visibility::Inherited);
        app.update();
        assert_eq!(drawn(&app), [0, 1, 2, 3, 4, 5], "shown again, it rejoins");
    }
}

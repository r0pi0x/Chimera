//! An app that draws nothing (a dedicated server) keeps no render meshes
//! of the kit: `Undrawn` in its `LoadContext` makes every load drop the
//! vertex data of the bodies, the view models and the held models
//! (`strip`: a map's as `MapSources::load_with` returns it, a kept
//! dressing's as it is kept, so its copy onto each map is nothing). They
//! were 87 MB of TF2's kept dressing and as much again in every map
//! (docs/performance.md, "A server keeps no render meshes").
//!
//! What stays is what the simulation reads: each model's meshes by count
//! and name with their materials (a body part's choice, a skin), bounds,
//! collision, hitboxes, bones, animations and attachments, and a held
//! model's positions (`loose::asset` takes a dropped item's box from
//! them). The world's own meshes and its props' models stay whole (the
//! surfaces under a point, collision without a collision model). The
//! entries on disk are every app's and keep everything.
//!
//! Known shortcuts: not for an app that may yet draw (a listen host), or
//! a test that reads the kit's vertices.

use super::{MapData, MapMesh, MapModel};

/// In an app's `LoadContext`: the app draws nothing (module docs).
#[derive(Clone, Copy, Debug, Default)]
pub struct Undrawn;

/// `mesh` without what only the drawing reads: its vertices' normals,
/// coordinates, skinning, triangles and flexes, and (`positions` off)
/// their positions.
fn strip_mesh(mesh: &mut MapMesh, positions: bool) {
    if !positions {
        mesh.positions = Vec::new();
    }
    mesh.normals = Vec::new();
    mesh.uvs = Vec::new();
    mesh.uvs2 = Vec::new();
    mesh.joints = Vec::new();
    mesh.joint_weights = Vec::new();
    mesh.indices = Vec::new();
    mesh.lightmap_uvs = Vec::new();
    mesh.blend_weights = Vec::new();
    mesh.source_vertices = Vec::new();
    mesh.morphs = Vec::new();
}

/// `model`'s meshes and lower levels of detail stripped (`strip_mesh`).
fn strip_model(model: &mut MapModel, positions: bool) {
    for m in &mut model.meshes {
        strip_mesh(m, positions);
    }
    for m in model.lods.iter_mut().flat_map(|l| &mut l.meshes).flatten() {
        strip_mesh(m, false);
    }
}

/// Drop the kit's render meshes from `map` (module docs): the bodies' and
/// the view models' vertex data, the held models' but their positions.
pub fn strip(map: &mut MapData) {
    for c in &mut map.kit.characters {
        strip_model(&mut c.model, false);
    }
    for v in &mut map.kit.view_models {
        strip_model(&mut v.model, false);
    }
    for h in &mut map.kit.held {
        strip_model(&mut h.model, true);
        for (_, m) in &mut h.merged {
            strip_model(m, true);
        }
        if let Some(m) = &mut h.own {
            strip_model(m, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MapHeldModel, loose};
    use bevy::prelude::*;

    fn mesh() -> MapMesh {
        MapMesh {
            material: "m".into(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 2.0, 3.0], [1.0, 0.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            uvs: vec![[0.0, 0.0]; 3],
            joints: vec![[0; 4]; 3],
            joint_weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            texture: Some(4),
            ..Default::default()
        }
    }

    fn model() -> MapModel {
        MapModel { meshes: vec![mesh(), mesh()], mesh_names: vec!["a".into(), "b".into()], ..Default::default() }
    }

    #[test]
    fn the_kit_keeps_what_the_simulation_reads() {
        let mut map = MapData::default();
        map.meshes.push(mesh());
        map.models.push(model());
        map.kit.held.push(MapHeldModel {
            key: "gun".into(),
            model: model(),
            bone: "b".into(),
            muzzle: None,
            attachments: Vec::new(),
            merged: vec![("left".into(), model())],
            own: Some(model()),
            nose: None,
            turned: Vec::new(),
        });
        let root = Transform::from_scale(Vec3::splat(0.0254)).with_rotation(Quat::from_rotation_y(0.7));
        let before = loose::asset(&map.kit.held[0], root);
        strip(&mut map);
        // The world and its props are whole.
        assert_eq!(map.meshes[0].indices.len(), 3);
        assert_eq!(map.models[0].meshes[0].normals.len(), 3);
        // Held models: their positions, so a dropped one's box is the same.
        let held = &map.kit.held[0];
        for m in [&held.model, held.own.as_ref().unwrap(), &held.merged[0].1] {
            assert_eq!((m.meshes.len(), m.mesh_names.len()), (2, 2));
            assert!(m.meshes.iter().all(|m| m.positions.len() == 3 && m.normals.is_empty() && m.indices.is_empty()));
            assert!(m.meshes.iter().all(|m| m.material == "m" && m.texture == Some(4)));
        }
        let after = loose::asset(held, root);
        assert_eq!((before.half, before.frame), (after.half, after.frame));
        // A body's or a view model's: the meshes by count, name and
        // material, no vertices.
        let mut body = model();
        strip_model(&mut body, false);
        assert_eq!((body.meshes.len(), body.mesh_names.len()), (2, 2));
        assert!(body.meshes.iter().all(|m| m.positions.is_empty() && m.indices.is_empty() && m.joints.is_empty()));
        assert!(body.meshes.iter().all(|m| m.material == "m" && m.texture == Some(4)));
    }
}

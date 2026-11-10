//! New meshes' vertex data written to the GPU at most `MESH_BYTES_PER_FRAME`
//! a frame while a map comes in (docs/performance.md, "A map's swap in the
//! real app"). Bevy's per-frame upload budget (`RenderAssetBytesPerFrame`,
//! which the map load caps: `console::LoadCaps`) holds back images and
//! `RenderMesh`es, but its mesh allocator (`allocate_and_free_meshes`)
//! packs and writes every mesh extracted in the frame regardless: a step
//! of a map's spawn that added 30-60 MB of meshes cost the render thread
//! 150-380 M instructions (45-120 ms) in that one frame, nearly all in
//! `Mesh::write_packed_vertex_buffer_data`.
//!
//! `pace_mesh_uploads` (render world, before the allocator) keeps the
//! meshes over the frame's budget in `HeldMeshes` and hands them to the
//! allocator and `prepare_assets` in later frames, oldest first (at least
//! one a frame): only while the main world caps uploads (a load) or some
//! are held. A held mesh removed or extracted again is dropped from the
//! queue (the newer data wins). A mesh is drawn once its upload is done,
//! as with Bevy's own budget.
//!
//! The map's `Settling` waits for the uploads (`uploads_settled`, a
//! `MapHooks::settle` hook): nothing held and Bevy's budget not used up in
//! two render frames in a row, so the game is entered with every map
//! mesh and image on the GPU and lifting the cap moves nothing at once.

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use bevy::{
    asset::AssetId,
    mesh::Mesh,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        mesh::{RenderMesh, allocator::allocate_and_free_meshes},
        render_asset::{ExtractedAssets, RenderAssetBytesPerFrameLimiter, prepare_assets},
    },
};

/// The most mesh data (vertices and indices) the render world writes a
/// frame while pacing: about 25-40 M instructions of packing.
pub const MESH_BYTES_PER_FRAME: usize = 6 << 20;

pub struct UploadPacePlugin;

impl Plugin for UploadPacePlugin {
    fn build(&self, app: &mut App) {
        let state = UploadState::default();
        app.init_resource::<crate::map::MapHooks>();
        app.world_mut().resource_mut::<crate::map::MapHooks>().settle.push(uploads_settled);
        // Without a render world (headless) there is nothing to wait for:
        // no `UploadState`, and `uploads_settled` says done.
        if app.get_sub_app(RenderApp).is_some() {
            app.insert_resource(state.clone());
        }
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.insert_resource(state).init_resource::<HeldMeshes>().add_systems(
                Render,
                (
                    pace_mesh_uploads
                        .in_set(RenderSystems::PrepareAssets)
                        .before(allocate_and_free_meshes)
                        .before(prepare_assets::<RenderMesh>),
                    report_uploads.in_set(RenderSystems::PrepareResources),
                ),
            );
        }
    }
}

/// What the render world tells the main world about its uploads.
#[derive(Resource, Clone, Default)]
pub struct UploadState(Arc<UploadCounts>);

#[derive(Default)]
pub struct UploadCounts {
    /// Meshes held for later frames.
    held: AtomicUsize,
    /// Render frames in a row with nothing held or deferred.
    clear_frames: AtomicUsize,
}

impl UploadState {
    /// Meshes held for later frames, as of the last render frame.
    pub fn held(&self) -> usize {
        self.0.held.load(Ordering::Relaxed)
    }

    /// Whether the render world has had nothing waiting for two frames.
    pub fn settled(&self) -> bool {
        self.0.clear_frames.load(Ordering::Relaxed) >= 2
    }
}

/// `MapHooks::settle`: the map's uploads are done (no render world: done).
pub fn uploads_settled(w: &mut World) -> bool {
    w.get_resource::<UploadState>().is_none_or(UploadState::settled)
}

/// Meshes extracted but not yet handed to the allocator, oldest first.
#[derive(Resource, Default)]
pub struct HeldMeshes(VecDeque<(AssetId<Mesh>, Mesh, usize)>);

impl HeldMeshes {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A mesh's data as the allocator writes it.
pub fn mesh_bytes(mesh: &Mesh) -> usize {
    mesh.get_vertex_buffer_size() + mesh.get_index_buffer_bytes().map_or(0, <[u8]>::len)
}

fn pace_mesh_uploads(
    mut extracted: ResMut<ExtractedAssets<RenderMesh>>,
    mut held: ResMut<HeldMeshes>,
    limiter: Res<RenderAssetBytesPerFrameLimiter>,
) {
    let budget = (limiter.max_bytes.is_some() || !held.is_empty()).then_some(MESH_BYTES_PER_FRAME);
    pace(&mut extracted, &mut held, budget);
}

/// One frame's pacing: with a `budget`, the held meshes then this frame's
/// go on to the allocator until it is spent (at least one), the rest are
/// held; without one, everything goes on.
pub fn pace(extracted: &mut ExtractedAssets<RenderMesh>, held: &mut HeldMeshes, budget: Option<usize>) {
    // Held meshes since removed, or extracted again, are dropped.
    if !held.is_empty() {
        let fresh: bevy::platform::collections::HashSet<AssetId<Mesh>> =
            extracted.extracted.iter().map(|(id, _)| *id).collect();
        held.0.retain(|(id, _, _)| !extracted.removed.contains(id) && !fresh.contains(id));
    }
    let Some(budget) = budget else {
        // Nothing to pace (and nothing held: a budget is kept while any are).
        return;
    };
    let new = std::mem::take(&mut extracted.extracted);
    held.0.extend(new.into_iter().map(|(id, mesh)| {
        let bytes = mesh_bytes(&mesh);
        (id, mesh, bytes)
    }));
    let mut spent = 0;
    while let Some((_, _, bytes)) = held.0.front() {
        if spent > 0 && spent + bytes > budget {
            break;
        }
        spent += bytes;
        let (id, mesh, _) = held.0.pop_front().expect("a front");
        extracted.added.insert(id);
        extracted.extracted.push((id, mesh));
    }
    // What stays held is not this frame's: the allocator keeps the old
    // allocation (a modified mesh draws its old data) until it is released.
    for (id, _, _) in &held.0 {
        extracted.added.remove(id);
        extracted.modified.remove(id);
    }
}

/// After the uploads: how many meshes wait, and whether Bevy's budget held
/// anything back (it was used up), for `uploads_settled`.
fn report_uploads(held: Res<HeldMeshes>, limiter: Res<RenderAssetBytesPerFrameLimiter>, state: Res<UploadState>) {
    let used_up = limiter
        .max_bytes
        .is_some_and(|max| limiter.bytes_written.load(Ordering::Relaxed) >= max);
    state.0.held.store(held.len(), Ordering::Relaxed);
    if held.is_empty() && !used_up {
        state.0.clear_frames.fetch_add(1, Ordering::Relaxed);
    } else {
        state.0.clear_frames.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology};

    use super::*;

    fn mesh(vertices: usize) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; vertices])
    }

    fn id(n: u128) -> AssetId<Mesh> {
        AssetId::Uuid { uuid: bevy::asset::uuid::Uuid::from_u128(n) }
    }

    fn frame(meshes: Vec<(u128, usize)>) -> ExtractedAssets<RenderMesh> {
        let mut e = ExtractedAssets::<RenderMesh>::default();
        for (n, v) in meshes {
            e.extracted.push((id(n), mesh(v)));
            e.added.insert(id(n));
        }
        e
    }

    fn released(e: &ExtractedAssets<RenderMesh>) -> Vec<u128> {
        e.extracted
            .iter()
            .map(|(i, _)| match i {
                AssetId::Uuid { uuid } => uuid.as_u128(),
                _ => unreachable!(),
            })
            .collect()
    }

    /// 1 MiB of positions.
    const MIB: usize = (1 << 20) / 12;

    #[test]
    fn a_frame_writes_at_most_the_budget_and_the_rest_follow_in_order() {
        let mut held = HeldMeshes::default();
        let mut e = frame((0..10).map(|n| (n, MIB)).collect());
        pace(&mut e, &mut held, Some(3 << 20));
        assert_eq!(released(&e), vec![0, 1, 2]);
        assert!(e.added.len() == 3 && !e.added.contains(&id(5)));
        let mut e = frame(vec![(10, MIB)]);
        pace(&mut e, &mut held, Some(3 << 20));
        assert_eq!(released(&e), vec![3, 4, 5]);
        let mut all = vec![];
        while !held.is_empty() {
            let mut e = frame(vec![]);
            pace(&mut e, &mut held, Some(3 << 20));
            all.extend(released(&e));
        }
        assert_eq!(all, vec![6, 7, 8, 9, 10]);
    }

    #[test]
    fn one_mesh_over_the_budget_still_goes() {
        let mut held = HeldMeshes::default();
        let mut e = frame(vec![(1, 4 * MIB), (2, MIB)]);
        pace(&mut e, &mut held, Some(1 << 20));
        assert_eq!(released(&e), vec![1]);
        assert_eq!(held.len(), 1);
    }

    #[test]
    fn removed_or_replaced_held_meshes_are_dropped() {
        let mut held = HeldMeshes::default();
        let mut e = frame((0..4).map(|n| (n, MIB)).collect());
        pace(&mut e, &mut held, Some(1 << 20));
        assert_eq!(held.len(), 3);
        // 1 removed, 2 extracted again: only the new 2 and 3 are left.
        let mut e = frame(vec![(2, 2 * MIB)]);
        e.removed.insert(id(1));
        pace(&mut e, &mut held, Some(1 << 20));
        assert_eq!(released(&e), vec![3]);
        let mut e = frame(vec![]);
        pace(&mut e, &mut held, Some(1 << 20));
        assert_eq!(released(&e), vec![2]);
        assert_eq!(e.extracted[0].1.count_vertices(), 2 * MIB);
        assert!(held.is_empty());
    }

    #[test]
    fn settled_after_two_frames_with_nothing_waiting() {
        use bevy::ecs::system::RunSystemOnce;
        let state = UploadState::default();
        let mut w = World::new();
        w.insert_resource(state.clone());
        w.init_resource::<HeldMeshes>();
        let limiter = |written: usize| RenderAssetBytesPerFrameLimiter {
            max_bytes: Some(100),
            bytes_written: AtomicUsize::new(written),
        };
        // Bevy's budget used up: something was held back.
        w.insert_resource(limiter(100));
        w.run_system_once(report_uploads).unwrap();
        assert!(!state.settled());
        w.insert_resource(limiter(10));
        w.run_system_once(report_uploads).unwrap();
        assert!(!state.settled());
        w.run_system_once(report_uploads).unwrap();
        assert!(state.settled());
        // A held mesh starts the count again.
        w.resource_mut::<HeldMeshes>().0.push_back((id(1), mesh(1), 12));
        w.run_system_once(report_uploads).unwrap();
        assert!(!state.settled() && state.held() == 1);
        // Without a render world the hook doesn't wait.
        assert!(uploads_settled(&mut World::new()));
    }

    #[test]
    fn without_a_budget_nothing_is_held() {
        let mut held = HeldMeshes::default();
        let mut e = frame((0..10).map(|n| (n, MIB)).collect());
        pace(&mut e, &mut held, None);
        assert_eq!(released(&e).len(), 10);
        assert!(held.is_empty());
    }
}

//! A mesh is left out of the depth prepass while its main pass's pipeline is
//! still being made (`hold_prepass`): it then shows nothing until it can
//! show itself, instead of a hole.
//!
//! Bevy makes a mesh's pipelines on the async compute pool, one per pass,
//! and draws each pass's as soon as that one is ready. The depth prepass's
//! for an opaque prop has no fragment stage and is ready first; until the
//! main pass's is, the mesh wrote depth that nothing coloured, so the world
//! behind it failed the depth test and its pixels kept whatever the target
//! held: a model shown for the first time on a busy pool (the player's own
//! body and gun after `thirdperson`, a practice target's body and its gun) was
//! a flat, sky-coloured silhouette for 30-150 frames (docs/performance.md,
//! "Late models shown whole").
//!
//! After Bevy queues the prepass (`queue_prepass_material_meshes`), each mesh
//! it queued this frame whose main-pass pipeline in that view is not ready is
//! taken out of the prepass bins again and put on Bevy's own list of meshes to
//! queue next frame (`PendingPrepassMeshMaterialQueues`), until it is. A mesh
//! whose pipelines are made already (the common case) is untouched.

use bevy::{
    core_pipeline::prepass::{AlphaMask3dPrepass, Opaque3dPrepass},
    pbr::{
        PendingPrepassMeshMaterialQueues, RenderMeshInstances, SpecializedMaterialPipelineCache,
        SpecializedPrepassMaterialPipelineCache,
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        camera::DirtySpecializations,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{CachedPipelineState, PipelineCache},
        sync_world::MainEntityHashSet,
        view::{ExtractedView, RenderVisibleEntities},
    },
};

/// Adds `hold_prepass` to the render world.
pub struct PrepassGatePlugin;

impl Plugin for PrepassGatePlugin {
    fn build(&self, app: &mut App) {
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.add_systems(
                Render,
                hold_prepass
                    .in_set(RenderSystems::QueueMeshes)
                    .after(bevy::pbr::queue_prepass_material_meshes),
            );
        }
    }
}

/// Whether a mesh may write the prepass's depth: its main pass in the same
/// view has a pipeline (`main`: its state there; None: not specialized) and
/// it is ready.
pub fn prepass_allowed(main: Option<&CachedPipelineState>) -> bool {
    matches!(main, Some(CachedPipelineState::Ok(_)))
}

/// Take the meshes queued into the prepass this frame whose main pass cannot
/// draw yet back out, to be queued again next frame (module doc).
#[allow(clippy::too_many_arguments)]
fn hold_prepass(
    views: Query<(&ExtractedView, &RenderVisibleEntities)>,
    main: Option<Res<SpecializedMaterialPipelineCache>>,
    prepass: Option<Res<SpecializedPrepassMaterialPipelineCache>>,
    pipelines: Res<PipelineCache>,
    dirty: Option<Res<DirtySpecializations>>,
    instances: Option<Res<RenderMeshInstances>>,
    pending: Option<ResMut<PendingPrepassMeshMaterialQueues>>,
    opaque: Option<ResMut<ViewBinnedRenderPhases<Opaque3dPrepass>>>,
    masked: Option<ResMut<ViewBinnedRenderPhases<AlphaMask3dPrepass>>>,
    mut seen: Local<MainEntityHashSet>,
    mut held: Local<Vec<(Entity, bevy::render::sync_world::MainEntity)>>,
) {
    let (Some(main), Some(prepass), Some(dirty), Some(instances), Some(mut pending)) =
        (main, prepass, dirty, instances, pending)
    else {
        return;
    };
    let (mut opaque, mut masked) = (opaque, masked);
    for (view, visible) in &views {
        let id = view.retained_view_entity;
        let (Some(view_main), Some(view_prepass)) = (main.get(&id), prepass.get(&id)) else { continue };
        let Some(meshes) = visible.get::<Mesh3d>() else { continue };
        let Some(queues) = pending.get_mut(&id) else { continue };
        held.clear();
        // What Bevy's prepass queue went through this frame (the same
        // iterator over the same lists).
        for (render, entity) in dirty.iter_to_queue(id, meshes, &queues.prev_frame, &mut seen) {
            if !view_prepass.contains_key(entity)
                || queues.current_frame.contains(&(*render, *entity))
                || instances.render_mesh_queue_data(*entity).is_none()
            {
                // Not in the prepass, not queued (Bevy retries it), or gone.
                continue;
            }
            let state = view_main.get(entity).map(|p| pipelines.get_render_pipeline_state(*p));
            if !prepass_allowed(state) {
                held.push((*render, *entity));
            }
        }
        for &(render, entity) in held.iter() {
            if let Some(p) = opaque.as_mut().and_then(|p| p.get_mut(&id)) {
                p.remove(entity);
            }
            if let Some(p) = masked.as_mut().and_then(|p| p.get_mut(&id)) {
                p.remove(entity);
            }
            queues.current_frame.insert((render, entity));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No depth from a mesh whose main pass has no pipeline yet, or one still
    /// waiting to be made (a made one needs a device: the scene shows it).
    #[test]
    fn no_prepass_before_the_main_pass_can_draw() {
        assert!(!prepass_allowed(None));
        assert!(!prepass_allowed(Some(&CachedPipelineState::Queued)));
    }
}

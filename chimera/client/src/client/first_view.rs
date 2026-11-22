//! Whether a map's first view can be drawn as it is: every mesh the cameras
//! see on the GPU, its material prepared (its textures uploaded), its
//! pipeline compiled. Bevy draws an entity only once all three are in, and
//! skips it until then: a game entered before they were showed its map's
//! models and materials coming in in front of the player (the pipelines
//! compile on tasks, the uploads are capped while a map loads).
//!
//! The render world counts, each frame, what its views see and cannot draw
//! yet (`count_unready`: per view, each visible mesh's `RenderMesh`, its
//! material's `PreparedMaterial`, its specialized pipeline's state) and the
//! pipelines still compiling anywhere (`PipelineCache::waiting_pipelines`:
//! shadow and prepass ones too); the main world reads the counts
//! (`FirstView`). What else a load still builds registers its own count
//! (`chimera_world::map::loading::LoadPending`: the late-built bodies,
//! held and view models; Combat Arms' coming ladder's models).
//!
//! Behind a party app's cover the world's cameras draw unseen while a map
//! settles, beside a copy drawing the whole map from above and each view
//! model camera drawing one part of each kind of view model
//! (`staging::overview`, `chimera_present`'s `WarmViewModels`), so what
//! the player's first view needs, wherever it spawns, is counted here.
//! A network client's ladder models load behind its loading screen too
//! (the server's manifest carries the ladder: `chimera_net::
//! minigame_net::server_ladder`).
//!
//! Known shortcuts (docs/tech-debt.md): the render counts see only what a
//! camera draws while the map settles; where no cover hides an overview
//! (a game's own shell entering a map) the first view's pipelines are
//! only those its camera sees from where it waits. A client's sky camera
//! (a 3D skybox) that doesn't draw while the map settles leaves what it
//! alone draws to compile when the cover goes (TF2's ctf_2fort: 16 props
//! for about 8 frames on a client).
//!
//! The map's `Settling` waits for it (`first_view_ready`, a
//! `MapHooks::settle` hook): nothing pending in two render frames in a row.
//! The game is entered (`map::LocalMapLoad` goes: the lobby's "loaded")
//! only then, or when the settle's time runs out, whose log line names
//! what was still pending (`pending_words`).
//!
//! `CHIMERA_FIRST_VIEW_GATE=0`: the settle doesn't wait for it (the A/B).
//! `CHIMERA_FIRST_VIEW_LOG=<frames>`: the counts logged every frame from the
//! map's spawn until that many frames after entering the game (`in+<n>`),
//! and that many frames after the last `staging::WorldCover` goes
//! (`shown+<n>`: a party app's match starting behind its lobby), with the
//! names of a few meshes not drawn yet (measuring).

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use bevy::{
    mesh::Mesh3d,
    pbr::{PreparedMaterial, RenderMaterialInstances, RenderMeshInstances, SpecializedMaterialPipelineCache},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        erased_render_asset::ErasedRenderAssets,
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_resource::{CachedPipelineState, PipelineCache},
        view::{ExtractedView, RenderVisibleEntities},
    },
};

pub struct FirstViewPlugin;

impl Plugin for FirstViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::map::MapHooks>();
        app.init_resource::<crate::map::loading::LoadPending>();
        app.world_mut().resource_mut::<crate::map::MapHooks>().settle.push(first_view_ready);
        app.add_systems(Last, log_counts.run_if(|| log_limit().is_some()));
        // Without a render world (headless) there is nothing to wait for:
        // no `FirstView`, and the hook counts only the main world's work.
        let Some(render) = app.get_sub_app_mut(RenderApp) else { return };
        let state = FirstView::default();
        render
            .insert_resource(state.clone())
            .add_systems(Render, count_unready.in_set(RenderSystems::PhaseSort));
        app.insert_resource(state);
    }
}

/// What the render world could not draw in its last frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unready {
    /// Meshes the views see.
    pub visible: usize,
    /// Of those, whose mesh is not on the GPU.
    pub meshes: usize,
    /// Whose material is not prepared (its textures not uploaded).
    pub materials: usize,
    /// Not specialized yet, or its pipeline still compiling.
    pub pipelines: usize,
    /// Pipelines compiling or queued anywhere (shadows and prepasses too).
    pub compiling: usize,
}

impl Unready {
    /// Visible meshes not drawn, and pipelines compiling.
    pub fn pending(&self) -> usize {
        self.meshes + self.materials + self.pipelines + self.compiling
    }
}

/// The render world's counts, shared with the main world.
#[derive(Resource, Clone, Default)]
pub struct FirstView(Arc<Counts>);

#[derive(Default)]
pub struct Counts {
    visible: AtomicUsize,
    meshes: AtomicUsize,
    materials: AtomicUsize,
    pipelines: AtomicUsize,
    compiling: AtomicUsize,
    /// Render frames in a row with nothing pending.
    clear_frames: AtomicUsize,
    /// Render frames counted.
    frames: AtomicUsize,
    /// A few of the last frame's visible meshes not drawn yet (their main
    /// world entities), for the log.
    sample: std::sync::Mutex<Vec<Entity>>,
}

impl FirstView {
    /// The last render frame's counts.
    pub fn unready(&self) -> Unready {
        let c = &self.0;
        Unready {
            visible: c.visible.load(Ordering::Relaxed),
            meshes: c.meshes.load(Ordering::Relaxed),
            materials: c.materials.load(Ordering::Relaxed),
            pipelines: c.pipelines.load(Ordering::Relaxed),
            compiling: c.compiling.load(Ordering::Relaxed),
        }
    }

    /// Nothing pending in two render frames in a row.
    pub fn ready(&self) -> bool {
        self.0.clear_frames.load(Ordering::Relaxed) >= 2
    }

    /// Render frames counted so far.
    pub fn frames(&self) -> usize {
        self.0.frames.load(Ordering::Relaxed)
    }

    /// A few of the last frame's visible meshes not drawn yet.
    pub fn sample(&self) -> Vec<Entity> {
        self.0.sample.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn store(&self, u: Unready) {
        let c = &self.0;
        c.visible.store(u.visible, Ordering::Relaxed);
        c.meshes.store(u.meshes, Ordering::Relaxed);
        c.materials.store(u.materials, Ordering::Relaxed);
        c.pipelines.store(u.pipelines, Ordering::Relaxed);
        c.compiling.store(u.compiling, Ordering::Relaxed);
        c.frames.fetch_add(1, Ordering::Relaxed);
        if u.pending() == 0 {
            c.clear_frames.fetch_add(1, Ordering::Relaxed);
        } else {
            c.clear_frames.store(0, Ordering::Relaxed);
        }
    }
}

/// How many meshes not drawn yet the log names.
const SAMPLE: usize = 6;

/// After the frame's specialization and queues: what the views see and
/// cannot draw yet.
#[allow(clippy::too_many_arguments)]
fn count_unready(
    views: Query<(&ExtractedView, &RenderVisibleEntities)>,
    meshes: Res<RenderAssets<RenderMesh>>,
    instances: Res<RenderMeshInstances>,
    materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    material_instances: Res<RenderMaterialInstances>,
    specialized: Res<SpecializedMaterialPipelineCache>,
    cache: Res<PipelineCache>,
    state: Res<FirstView>,
) {
    let mut u = Unready {
        compiling: cache.waiting_pipelines().count(),
        ..default()
    };
    let mut sample = Vec::new();
    for (view, visible) in &views {
        let Some(class) = visible.get::<Mesh3d>() else { continue };
        let view_cache = specialized.get(&view.retained_view_entity);
        for (_, main) in class.iter_visible() {
            u.visible += 1;
            let mesh_in = instances.mesh_asset_id(*main).is_some_and(|id| meshes.get(id).is_some());
            let material = material_instances.instances.get(main).map(|m| m.asset_id);
            // A mesh without a material is someone's own pipeline's.
            let material_in = material.is_none_or(|id| materials.get(id).is_some());
            let before = u.pending();
            if !mesh_in {
                u.meshes += 1;
            } else if !material_in {
                u.materials += 1;
            } else if material.is_some() {
                let ready = view_cache
                    .and_then(|c| c.get(main))
                    .is_some_and(|id| matches!(cache.get_render_pipeline_state(*id), CachedPipelineState::Ok(_)));
                if !ready {
                    u.pipelines += 1;
                }
            }
            if u.pending() != before && sample.len() < SAMPLE {
                sample.push(main.id());
            }
        }
    }
    if let Ok(mut s) = state.0.sample.lock() {
        *s = sample;
    }
    state.store(u);
}

/// The main world's own work still under way that the first view needs
/// (`LoadPending`), by name.
pub fn main_pending(w: &World) -> Vec<(&'static str, usize)> {
    let Some(p) = w.get_resource::<crate::map::loading::LoadPending>() else {
        return Vec::new();
    };
    p.counts(w).into_iter().filter(|(_, n)| *n > 0).collect()
}

/// `MapHooks::settle`: the first view can be drawn as it is (no render
/// world: only the main world's work).
pub fn first_view_ready(w: &mut World) -> bool {
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *OFF.get_or_init(|| chimera_core::env::var("CHIMERA_FIRST_VIEW_GATE").is_ok_and(|v| v == "0")) {
        return true;
    }
    main_pending(w).is_empty() && w.get_resource::<FirstView>().is_none_or(FirstView::ready)
}

/// What is still pending, in words (the settle's timeout names it).
pub fn pending_words(w: &World) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(u) = w.get_resource::<FirstView>().map(FirstView::unready) {
        for (n, what) in [
            (u.meshes, "meshes not on the GPU"),
            (u.materials, "materials not prepared"),
            (u.pipelines, "visible meshes' pipelines not ready"),
            (u.compiling, "pipelines compiling"),
        ] {
            if n > 0 {
                parts.push(format!("{n} {what}"));
            }
        }
    }
    for (name, n) in main_pending(w) {
        parts.push(format!("{n} {name}"));
    }
    if parts.is_empty() { "nothing".into() } else { parts.join(", ") }
}

/// `CHIMERA_FIRST_VIEW_LOG=<frames>`: the counts each frame while a map
/// loads and that many frames into the game (`in+<n>`), and that many
/// frames after the world's last cover goes (`shown+<n>`).
fn log_counts(
    world: &World,
    mut after: Local<Option<u32>>,
    mut shown: Local<Option<u32>>,
    mut was_loading: Local<bool>,
    mut was_covered: Local<bool>,
) {
    let Some(limit) = log_limit() else { return };
    let loading = world.contains_resource::<crate::map::LocalMapLoad>();
    if loading {
        *after = None;
    } else if *was_loading {
        *after = Some(0);
    }
    *was_loading = loading;
    let covered = world.get_resource::<super::staging::WorldCover>().is_some_and(|c| c.covered());
    if covered {
        *shown = None;
    } else if *was_covered {
        *shown = Some(0);
    }
    *was_covered = covered;
    let mut labels = Vec::new();
    if loading {
        labels.push("loading".to_string());
    }
    if let Some(n) = after.filter(|n| *n < limit && !loading) {
        labels.push(format!("in+{n}"));
        *after = Some(n + 1);
    }
    if let Some(n) = shown.filter(|n| *n < limit) {
        labels.push(format!("shown+{n}"));
        *shown = Some(n + 1);
    }
    if labels.is_empty() {
        return;
    }
    // The 3D cameras drawing (by name), whose views the counts are.
    let cams: Vec<String> = world
        .try_query::<(Entity, &Camera, Has<Camera3d>)>()
        .map(|mut q| q.iter(world).filter(|(_, c, d3)| *d3 && c.is_active).map(|(e, ..)| name_of(world, e)).collect())
        .unwrap_or_default();
    let state = world.get_resource::<FirstView>();
    let u = state.map(FirstView::unready).unwrap_or_default();
    let main: Vec<String> = main_pending(world).into_iter().map(|(k, v)| format!("{k}={v}")).collect();
    let names: Vec<String> = state.map(FirstView::sample).unwrap_or_default().into_iter().map(|e| name_of(world, e)).collect();
    info!(
        "first_view {} visible={} meshes={} materials={} pipelines={} compiling={} main=[{}] cameras=[{}]{}",
        labels.join(" "),
        u.visible,
        u.meshes,
        u.materials,
        u.pipelines,
        u.compiling,
        main.join(","),
        cams.join("; "),
        if names.is_empty() { String::new() } else { format!(" e.g. [{}]", names.join("; ")) }
    );
}

/// `CHIMERA_FIRST_VIEW_LOG`'s frames, if set.
fn log_limit() -> Option<u32> {
    static FRAMES: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    *FRAMES.get_or_init(|| chimera_core::env::var("CHIMERA_FIRST_VIEW_LOG").ok().and_then(|v| v.parse().ok()))
}

/// `e`'s name, or its nearest named ancestor's (the log's sample).
fn name_of(world: &World, e: Entity) -> String {
    let mut at = Some(e);
    for _ in 0..8 {
        let Some(x) = at.and_then(|x| world.get_entity(x).ok()) else { break };
        if let Some(n) = x.get::<Name>() {
            return n.as_str().to_string();
        }
        at = x.get::<ChildOf>().map(|c| c.parent());
    }
    format!("{e}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ready after two render frames in a row with nothing pending; any
    /// pending count starts it again.
    #[test]
    fn ready_after_two_clear_frames() {
        let s = FirstView::default();
        let busy = Unready { visible: 10, pipelines: 2, ..default() };
        let clear = Unready { visible: 10, ..default() };
        s.store(busy);
        assert!(!s.ready());
        s.store(clear);
        assert!(!s.ready());
        s.store(clear);
        assert!(s.ready());
        s.store(Unready { compiling: 1, ..clear });
        assert!(!s.ready() && s.unready().compiling == 1 && s.frames() == 4);
    }

    /// The settle waits for the main world's registered work too, and
    /// names it.
    #[test]
    fn main_world_work_holds_the_settle() {
        let mut w = World::new();
        w.insert_resource(FirstView::default());
        let s = w.resource::<FirstView>().clone();
        s.store(Unready::default());
        s.store(Unready::default());
        let mut p = crate::map::loading::LoadPending::default();
        p.register("bodies building", |w| w.get_resource::<Count>().map_or(0, |c| c.0));
        w.insert_resource(p);
        w.insert_resource(Count(2));
        assert!(!first_view_ready(&mut w));
        assert_eq!(pending_words(&w), "2 bodies building");
        w.insert_resource(Count(0));
        assert!(first_view_ready(&mut w));
        s.store(Unready { meshes: 3, ..default() });
        assert!(!first_view_ready(&mut w));
        assert_eq!(pending_words(&w), "3 meshes not on the GPU");
    }

    #[derive(Resource)]
    struct Count(usize);
}

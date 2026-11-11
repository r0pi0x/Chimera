//! `chimera_mem`: what holds the process's memory, in one readout
//! (docs/performance.md, "A session's memory"): the mappings by kind and
//! glibc's counts (`chimera_core::metrics::memory`), the GPU allocator's blocks
//! (wgpu's report), the bytes in the main world's image and mesh stores (with
//! `<n>`: the largest groups of images whose bytes it keeps, by size, format
//! and usage), and
//! the render world's own numbers, which a console command can't reach: a
//! render system writes them about once a second into `RenderWorldStats`.
//! Also how many `PropMaterial`s the store holds against those an entity
//! draws, and `chimera_mem pad <n>`: `n` undrawn materials held (0 drops them),
//! to measure what they cost the render thread.
//!
//! `memory::release_after_change` on the map's file: 20 s after each map
//! spawns, `memory::release_free` on a thread gives glibc's free pages back to
//! the system. A map change frees gigabytes that glibc kept (9 GB "free in
//! arenas" after eight maps, still counted resident or swapped).

use std::sync::{Arc, Mutex};

use bevy::{
    ecs::{archetype::Archetypes, entity::Entities},
    mesh::Mesh,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        mesh::{RenderMesh, allocator::MeshAllocator},
        render_asset::RenderAssets,
        render_resource::PipelineCache,
        renderer::RenderDevice,
        texture::GpuImage,
    },
};

use crate::metrics::memory::{Regions, malloc, size};

pub struct MemoryPlugin;

impl Plugin for MemoryPlugin {
    fn build(&self, app: &mut App) {
        use crate::console::ConsoleAppExt;
        let stats = RenderWorldStats::default();
        app.insert_resource(stats.clone());
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.insert_resource(stats).add_systems(Render, render_stats.in_set(RenderSystems::Cleanup));
        }
        app.add_systems(Update, crate::metrics::memory::release_after_change::<crate::map::MapFile>);
        app.console_command(
            "chimera:mem",
            "chimera_mem [<files>]: what holds this process's memory: mappings by kind (heap, anonymous, device, \
             file), the allocator's bytes, the GPU allocator's blocks, the image and mesh stores' bytes, the render \
             world's; with the <files> largest mapped files and device mappings, and the largest groups of images the \
             main world keeps the bytes of.",
            |w, a| {
                if a.first().map(String::as_str) == Some("pad") {
                    let n = a.get(1).and_then(|n| n.parse().ok()).ok_or("pad: how many?")?;
                    return Ok(Some(pad(w, n)));
                }
                Ok(Some(readout(w, a.first().and_then(|n| n.parse().ok()).unwrap_or(0))))
            },
        );
    }
}

/// The render world's numbers, shared with the main world.
#[derive(Resource, Clone, Default)]
pub struct RenderWorldStats(Arc<Mutex<RenderStats>>);

#[derive(Clone, Debug, Default)]
struct RenderStats {
    entities: u32,
    archetypes: usize,
    images: usize,
    /// The textures' bytes as their sizes and formats say, mips included.
    image_bytes: u64,
    meshes: usize,
    slabs: usize,
    slab_bytes: u64,
    pipelines: usize,
    waiting_pipelines: usize,
}

#[allow(clippy::too_many_arguments)]
fn render_stats(
    shared: Res<RenderWorldStats>,
    entities: &Entities,
    archetypes: &Archetypes,
    images: Option<Res<RenderAssets<GpuImage>>>,
    meshes: Option<Res<RenderAssets<RenderMesh>>>,
    allocator: Option<Res<MeshAllocator>>,
    pipelines: Option<Res<PipelineCache>>,
    mut at: Local<Option<std::time::Instant>>,
) {
    let now = std::time::Instant::now();
    if at.is_some_and(|t| (now - t).as_secs_f32() < 1.0) {
        return;
    }
    *at = Some(now);
    let mut s = RenderStats {
        entities: entities.count_spawned(),
        archetypes: archetypes.len(),
        ..default()
    };
    if let Some(images) = images {
        for (_, i) in images.iter() {
            s.images += 1;
            s.image_bytes += texture_bytes(i);
        }
    }
    if let Some(meshes) = meshes {
        s.meshes = meshes.iter().count();
    }
    if let Some(a) = allocator {
        s.slabs = a.slab_count();
        s.slab_bytes = a.slabs_size();
    }
    if let Some(p) = pipelines {
        s.pipelines = p.pipelines().count();
        s.waiting_pipelines = p.waiting_pipelines().count();
    }
    *shared.0.lock().unwrap() = s;
}

/// A texture's bytes, every mip of every layer.
fn texture_bytes(i: &GpuImage) -> u64 {
    let d = &i.texture_descriptor;
    let (bw, bh) = d.format.block_dimensions();
    let block = d.format.block_copy_size(None).unwrap_or(4) as u64;
    let layers = d.size.depth_or_array_layers.max(1) as u64;
    (0..d.mip_level_count.max(1))
        .map(|m| {
            let w = (d.size.width >> m).max(1).div_ceil(bw) as u64;
            let h = (d.size.height >> m).max(1).div_ceil(bh) as u64;
            w * h * block * layers
        })
        .sum()
}

/// `chimera_mem pad <n>`: hold `n` copies of a material no entity draws (0
/// drops them), each a GPU buffer and bind group, to measure what undrawn
/// materials cost the render thread now and after they're gone
/// (docs/performance.md, "A session's memory").
#[derive(Resource, Default)]
struct Pad(Vec<Handle<crate::render::prop_material::PropMaterial>>);

fn pad(w: &mut World, n: usize) -> String {
    use crate::render::prop_material::PropMaterial;
    let mut held = std::mem::take(&mut w.get_resource_or_init::<Pad>().0);
    held.truncate(n);
    let mut store = w.resource_mut::<Assets<PropMaterial>>();
    while held.len() < n {
        held.push(store.add(PropMaterial::default()));
    }
    let len = held.len();
    w.resource_mut::<Pad>().0 = held;
    format!("holding {len} undrawn materials")
}

/// How many of a material store's assets entities draw with, and how many sit
/// in the store held by something else (a cache, a model kept for later).
fn material_users<M: bevy::pbr::Material>(w: &mut World) -> Option<(usize, usize)> {
    let store = w.get_resource::<Assets<M>>()?.len();
    let mut q = w.query::<&MeshMaterial3d<M>>();
    let used: std::collections::BTreeSet<AssetId<M>> = q.iter(w).map(|m| m.0.id()).collect();
    Some((store, used.len()))
}

/// The material handles the map's model caches hold: the bodies' (parts and
/// skins), the held models', the gibs'.
fn cached_materials(w: &World) -> String {
    use crate::render::bodies::{HeldAssets, PlayModels};
    let mut out = Vec::new();
    if let Some(p) = w.get_resource::<PlayModels>() {
        let parts: usize = p.bodies.iter().map(|b| b.parts.len()).sum();
        let skins: usize = p.bodies.iter().map(|b| b.skins.iter().map(Vec::len).sum::<usize>()).sum();
        out.push(format!("{} bodies {parts} parts {skins} skin materials", p.bodies.len()));
    }
    if let Some(h) = w.get_resource::<HeldAssets>() {
        let n: usize = h.0.values().map(|p| p.whole.len() + p.own.len() + p.merged.iter().map(|m| m.1.len()).sum::<usize>()).sum();
        out.push(format!("{} held models {n} parts", h.0.len()));
    }
    out.join(", ")
}

fn readout(w: &mut World, files: usize) -> String {
    let props = material_users::<crate::render::prop_material::PropMaterial>(w);
    let cached = cached_materials(w);
    let w: &World = w;
    let mut lines = Vec::new();
    match Regions::read() {
        Some(r) => {
            lines.push(format!(
                "process {}: heap {} (swapped {}), anon {} (swapped {}), device {}, files {}, stacks {}",
                size(r.total()),
                size(r.heap.rss),
                size(r.heap.swap),
                size(r.anon.rss),
                size(r.anon.swap),
                size(r.device.total()),
                size(r.file.total()),
                size(r.stacks.total()),
            ));
            for (kind, list) in [("file", &r.files), ("device", &r.devices)] {
                for (name, b) in list.iter().take(files) {
                    lines.push(format!("  {kind} {name}: {}", size(*b)));
                }
            }
        }
        None => lines.push("process: no smaps here".into()),
    }
    if let Some(m) = malloc() {
        lines.push(format!("malloc: in use {} (of it mmapped {}), free in arenas {}", size(m.in_use), size(m.mmapped), size(m.free)));
    }
    if let Some(d) = w.get_resource::<RenderDevice>()
        && let Some(r) = d.wgpu_device().generate_allocator_report()
    {
        lines.push(format!(
            "gpu allocator: {} allocated in {} allocations, {} reserved in {} blocks",
            size(r.total_allocated_bytes),
            r.allocations.len(),
            size(r.total_reserved_bytes),
            r.blocks.len()
        ));
        if files > 0 {
            let mut by_name = std::collections::BTreeMap::<&str, (usize, u64)>::new();
            for a in &r.allocations {
                let e = by_name.entry(a.name.as_str()).or_default();
                e.0 += 1;
                e.1 += a.size;
            }
            let mut rows: Vec<_> = by_name.into_iter().collect();
            rows.sort_by_key(|r| std::cmp::Reverse(r.1.0));
            for (name, (n, b)) in rows.iter().take(files * 2) {
                lines.push(format!("  gpu {name}: {n}, {}", size(*b)));
            }
        }
    }
    let image_bytes = w
        .get_resource::<Assets<Image>>()
        .map_or((0, 0), |a| (a.len(), a.iter().map(|(_, i)| i.data.as_ref().map_or(0, |d| d.len() as u64)).sum::<u64>()));
    let mesh_bytes = w.get_resource::<Assets<Mesh>>().map_or((0, 0), |a| {
        let bytes = a.iter().map(|(_, m)| mesh_bytes(m)).sum::<u64>();
        (a.len(), bytes)
    });
    lines.push(format!(
        "stores: {} images holding {}, {} meshes holding {}",
        image_bytes.0,
        size(image_bytes.1),
        mesh_bytes.0,
        size(mesh_bytes.1)
    ));
    if files > 0 {
        lines.extend(held_images(w, files));
    }
    if let Some((store, used)) = props {
        let dormant = w.get_resource::<crate::render::prop_material::DormantProps>().map_or(0, |d| d.live());
        lines.push(format!(
            "PropMaterial: {store} in the store (each a GPU buffer), {used} drawn by an entity, {dormant} dormant (not \
             drawn: no GPU buffer); cached: {cached}"
        ));
    }
    if let Some(s) = w.get_resource::<RenderWorldStats>() {
        let s = s.0.lock().unwrap().clone();
        lines.push(format!(
            "render world: {} entities, {} archetypes, {} textures of {}, {} meshes in {} slabs of {}, {} pipelines ({} waiting)",
            s.entities,
            s.archetypes,
            s.images,
            size(s.image_bytes),
            s.meshes,
            s.slabs,
            size(s.slab_bytes),
            s.pipelines,
            s.waiting_pipelines
        ));
    }
    lines.join("\n")
}

/// The images whose bytes the main world holds (a `MAIN_WORLD` usage, or
/// not yet extracted), grouped by size, format and usage: the `n` largest
/// groups.
fn held_images(w: &World, n: usize) -> Vec<String> {
    let Some(images) = w.get_resource::<Assets<Image>>() else { return Vec::new() };
    let mut groups = std::collections::BTreeMap::<String, (usize, u64)>::new();
    for (_, i) in images.iter() {
        let Some(d) = i.data.as_ref().filter(|d| !d.is_empty()) else { continue };
        let t = &i.texture_descriptor;
        let key = format!("{}x{} {:?} {:?}", t.size.width, t.size.height, t.format, i.asset_usage);
        let e = groups.entry(key).or_default();
        e.0 += 1;
        e.1 += d.len() as u64;
    }
    let mut rows: Vec<_> = groups.into_iter().collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.1.1));
    rows.into_iter().take(n).map(|(k, (c, b))| format!("  image {k}: {c}, {}", size(b))).collect()
}

/// The bytes a mesh holds on the CPU: none once only the render world has it.
fn mesh_bytes(m: &Mesh) -> u64 {
    let vertices = m.try_attributes().map_or(0, |attrs| attrs.map(|(_, v)| v.get_bytes().len() as u64).sum::<u64>());
    let indices = m.try_indices_option().ok().flatten().map_or(0, |i| match i {
        bevy::mesh::Indices::U16(v) => v.len() as u64 * 2,
        bevy::mesh::Indices::U32(v) => v.len() as u64 * 4,
    });
    vertices + indices
}

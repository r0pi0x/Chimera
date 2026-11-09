//! Performance tools: the `chimera_perf` overlay (frame times, main-world
//! CPU time, GPU time per render pass, entity and mesh counts, visibility
//! culling), `r_novis`, `chimera_census` (`chimera_core::metrics::census`:
//! what the world holds), and `--bench` (time each `--views` view instead
//! of capturing it; see docs/performance.md).
//!
//! `chimera_perf` overlay (frame times, main-world CPU, render-world time
//! (`RenderTime`, written by the render world), GPU per render pass, entities,
//! meshes and triangles drawn, cameras drawing, visibility, dynamic bodies awake; `chimera_perf 3` also
//! what writes transforms, meshes and materials each frame and the UI trees laid out), the same as text (`chimera_perf_log
//! 1`, and `PerfReport` in bugreport's report.txt), `r_novis`,
//! `chimera_skyocclude`, `host_timescale`, `fps_max` (`FrameLimit`: 300 as CS:S,
//! sleeps at the end of a frame; off without a window and under `--bench`), the
//! water cvars (`render::water::WaterSettings`); `capture.rs` has `--bench` (its
//! table, the percentiles table, `bench.json`/`bench.csv`); the distributions
//! and the hitch log are in `frame_metrics.rs`.

use std::{collections::VecDeque, time::Instant};

use bevy::{
    camera::primitives::Aabb,
    diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin},
    prelude::*,
    render::diagnostic::RenderDiagnosticsPlugin,
};

use crate::{
    console::resource_cvar,
    map::VisClusters,
    render::vis::{NoVis, VisStats},
};

pub struct PerfPlugin;

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        use crate::console::ConsoleAppExt;
        app.console_command("chimera:census", chimera_core::metrics::census::HELP, chimera_core::metrics::census::command);
        app.init_resource::<AfterLines>().add_systems(Update, run_after_lines);
        app.console_command(
            "chimera:after",
            "after <seconds> <command...>: run a console line that many seconds from now, beside the queue: a script of \
             `wait`s holds up whatever a command queues itself (`map` behind `ugg_new`), this doesn't.",
            |w, a| {
                let seconds: f32 = a.first().and_then(|s| s.parse().ok()).ok_or("after: how many seconds?")?;
                let line = a.get(1..).unwrap_or_default().join(" ");
                let due = Instant::now() + std::time::Duration::from_secs_f32(seconds.max(0.0));
                w.resource_mut::<AfterLines>().0.push((due, line));
                Ok(None)
            },
        );
        app.add_plugins((EntityCountDiagnosticsPlugin::default(), RenderDiagnosticsPlugin))
            .init_resource::<Perf>()
            .init_resource::<FrameTimes>()
            .init_resource::<MeshTriangles>()
            .init_resource::<Churn>()
            .init_resource::<PerfLog>()
            .init_resource::<PerfReport>()
            .insert_resource(HostTimescale(1.0))
            .add_systems(PreStartup, timescale_from_command_line)
            .add_systems(First, apply_timescale.after(bevy::time::TimeSystems))
            .add_systems(Startup, spawn_overlay)
            .add_systems(First, start_frame)
            .add_systems(
                Last,
                (
                    record_triangles,
                    (
                        name_modified::<Mesh3d>,
                        name_modified::<MeshMaterial3d<crate::render::prop_material::PropMaterial>>,
                        name_modified::<MeshMaterial3d<StandardMaterial>>,
                        name_modified::<MeshMaterial3d<crate::render::sprite_material::SpriteMaterial>>,
                        name_modified::<MeshMaterial3d<crate::render::particles::ParticleDrawMaterial>>,
                        name_modified::<MeshMaterial3d<crate::render::decal::DecalMaterial>>,
                        name_modified::<MeshMaterial3d<crate::render::world_material::WorldMaterial>>,
                    )
                        .run_if(|p: Res<Perf>| p.show >= 3),
                    count_ui.run_if(|p: Res<Perf>| p.show >= 3),
                    count_churn,
                    end_frame,
                    perf_report,
                    draw_overlay,
                    hide_overlay,
                    limit_frames,
                )
                    .chain(),
            )
            .init_resource::<FrameLimit>();
        resource_cvar::<FrameLimit, f32>(
            app,
            "fps_max",
            "Most frames a second the game draws (0: no cap; vsync may cap lower).",
            |l| &mut l.fps_max,
        );
        // The render world's time per frame (its `Render` schedule, run on
        // the render thread beside the next frame's main world).
        let render_time = RenderTime::default();
        app.insert_resource(render_time.clone());
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render.insert_resource(render_time);
        }
        app.add_plugins(super::frame_metrics::FrameMetricsPlugin);
        #[cfg(feature = "trace")]
        app.add_plugins(super::trace_capture::TraceCapturePlugin);
        resource_cvar::<PerfLog, u8>(
            app,
            "chimera:perf_log",
            "1: log the chimera_perf readout as text once a second (bugreport's report.txt always has the latest).",
            |p| &mut p.0,
        );
        resource_cvar::<Perf, u8>(
            app,
            "chimera:perf",
            "1: performance overlay (frame times, CPU, GPU passes, meshes, visibility, per-frame changes); \
             2: also every render pass; 3: also log what writes transforms, meshes and materials, and the UI trees, each second.",
            |p| &mut p.show,
        );
        resource_cvar::<HostTimescale, f32>(
            app,
            "host_timescale",
            "Game time speed (1 normal); 0 freezes simulation, physics and effects (for identical captures).",
            |t| &mut t.0,
        );
        if app.world().contains_resource::<NoVis>() {
            resource_cvar::<NoVis, u8>(
                app,
                "r_novis",
                "1: draw every map part, ignoring the map's visibility data (prop fade distances still apply).",
                |v| &mut v.0,
            );
        }
        if app.world().contains_resource::<crate::render::vis::PortalsOpenAll>() {
            resource_cvar::<crate::render::vis::PortalsOpenAll, u8>(
                app,
                "r_portalsopenall",
                "1: treat every areaportal as open and don't clip the view through them (PVS culling only).",
                |v| &mut v.0,
            );
        }
        if app.world().contains_resource::<crate::render::vis::Occlusion>() {
            resource_cvar::<crate::render::vis::Occlusion, u8>(
                app,
                "r_occlusion",
                "0: ignore the map's occluders (func_occluder); 1: hide map parts fully behind active ones.",
                |v| &mut v.0,
            );
        }
        if app.world().contains_resource::<crate::render::sky_occluder::SkyOcclusion>() {
            resource_cvar::<crate::render::sky_occluder::SkyOcclusion, u8>(
                app,
                "chimera:skyocclude",
                "1: sky faces hide what lies behind them (props, water, brush entities in the view's PVS); 0 (default): \
                 they don't (unchecked against CS:S).",
                |v| &mut v.0,
            );
        }
        water_cvars(app);
        decal_cvars(app);
    }
}

/// `r_cleardecals`, `chimera_round_cleardecals` and `mp_decals`
/// (`render::decal`).
pub fn decal_cvars(app: &mut App) {
    use crate::{
        console::ConsoleAppExt,
        render::decal::{ClearDecals, DecalSettings},
    };
    app.init_resource::<DecalSettings>().add_message::<ClearDecals>();
    app.console_command("r_cleardecals", "Remove every bullet hole, slash and blood decal.", |w, _| {
        w.write_message(ClearDecals);
        Ok(None)
    });
    resource_cvar::<DecalSettings, u8>(
        app,
        "chimera:round_cleardecals",
        "1: remove every impact decal at each round start (the game keeps them until the map changes).",
        |s| &mut s.round_clear,
    );
    resource_cvar::<DecalSettings, usize>(
        app,
        "mp_decals",
        "Most bullet hole, slash and blood decals drawn at once; the oldest goes first.",
        |s| &mut s.limit,
    );
    let mut console = app.world_mut().resource_mut::<crate::console::Console>();
    console.archive("chimera_round_cleardecals");
    console.archive("mp_decals");
}

/// Source's water settings (specs/cs_source/water.md section 1) on
/// `render::water::WaterSettings`.
fn water_cvars(app: &mut App) {
    use crate::render::water::WaterSettings;
    app.init_resource::<WaterSettings>();
    resource_cvar::<WaterSettings, u8>(
        app,
        "r_waterforceexpensive",
        "1: planar water reflections even where a material turns $forceexpensive off.",
        |s| &mut s.force_expensive,
    );
    resource_cvar::<WaterSettings, u8>(
        app,
        "r_waterforcereflectentities",
        "1: water reflections show models (props, players) as well as the world.",
        |s| &mut s.reflect_entities,
    );
    resource_cvar::<WaterSettings, u8>(app, "r_WaterDrawReflection", "0: no planar water reflections.", |s| {
        &mut s.draw_reflection
    });
    resource_cvar::<WaterSettings, u8>(
        app,
        "r_WaterDrawRefraction",
        "0: water shows nothing below its surface (cubemap only).",
        |s| &mut s.draw_refraction,
    );
    resource_cvar::<WaterSettings, u8>(app, "mat_drawwater", "0: hide water surfaces.", |s| &mut s.draw_water);
    let mut console = app.world_mut().resource_mut::<crate::console::Console>();
    for name in ["r_waterforceexpensive", "r_waterforcereflectentities"] {
        console.archive(name);
    }
}

/// `after`'s lines and when each is due.
#[derive(Resource, Default)]
struct AfterLines(Vec<(Instant, String)>);

fn run_after_lines(mut lines: ResMut<AfterLines>, console: Option<ResMut<crate::console::Console>>) {
    let Some(mut console) = console else { return };
    if lines.0.is_empty() {
        return;
    }
    let now = Instant::now();
    lines.0.retain(|(due, line)| {
        if *due > now {
            return true;
        }
        console.submit(line.clone());
        false
    });
}

/// `host_timescale`: how fast game (virtual) time runs; 0 pauses it.
#[derive(Resource)]
struct HostTimescale(f32);

/// A `+host_timescale` on the command line holds from the first frame
/// (console lines run a frame or two later), so a frozen run is the same
/// every time.
fn timescale_from_command_line(args: Res<super::ClientArgs>, mut scale: ResMut<HostTimescale>) {
    for line in &args.0.console {
        let mut words = line.split_whitespace();
        if words.next() == Some("host_timescale")
            && let Some(v) = words.next().and_then(|v| v.trim_matches('"').parse().ok())
        {
            scale.0 = v;
        }
    }
}

fn apply_timescale(
    scale: Res<HostTimescale>,
    mut time: ResMut<Time<Virtual>>,
    physics: Option<ResMut<Time<avian3d::prelude::Physics>>>,
) {
    use avian3d::prelude::PhysicsTime;
    if !scale.is_changed() {
        return;
    }
    if scale.0 <= 0.0 {
        time.pause();
    } else {
        time.unpause();
        time.set_relative_speed(scale.0);
    }
    if let Some(mut physics) = physics {
        if scale.0 <= 0.0 {
            physics.pause();
        } else {
            physics.unpause();
        }
    }
}

#[derive(Resource, Default)]
struct Perf {
    show: u8,
}

/// Recent frames: wall time between frames and the main world's CPU time
/// (First to Last), seconds, over about the last second.
#[derive(Resource, Default)]
pub struct FrameTimes {
    started: Option<Instant>,
    last_start: Option<Instant>,
    pub frames: VecDeque<(f32, f32)>,
}

impl FrameTimes {
    /// Average, 95th percentile and maximum frame time, and the average
    /// main-world CPU time, milliseconds.
    pub fn summary(&self) -> (f32, f32, f32, f32) {
        let mut v: Vec<f32> = self.frames.iter().map(|f| f.0).collect();
        v.sort_by(f32::total_cmp);
        let n = v.len().max(1) as f32;
        let avg = v.iter().sum::<f32>() / n;
        let p95 = v.get(((v.len() as f32 * 0.95) as usize).min(v.len().saturating_sub(1))).copied().unwrap_or(0.0);
        let max = v.last().copied().unwrap_or(0.0);
        let cpu = self.frames.iter().map(|f| f.1).sum::<f32>() / n;
        (avg * 1e3, p95 * 1e3, max * 1e3, cpu * 1e3)
    }
}

/// The render world's latest `Render` schedule time, microseconds (written
/// by the render world, read by the main world).
#[derive(Resource, Clone, Default)]
pub struct RenderTime(std::sync::Arc<std::sync::atomic::AtomicU32>);

impl RenderTime {
    /// Milliseconds.
    pub fn ms(&self) -> f32 {
        self.0.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1e3
    }

    pub fn set_ms(&self, ms: f64) {
        self.0.store((ms * 1e3) as u32, std::sync::atomic::Ordering::Relaxed);
    }
}

fn start_frame(mut times: ResMut<FrameTimes>) {
    let now = Instant::now();
    times.last_start = times.started.replace(now);
}

/// `fps_max`: the most frames a second the window draws (CS:S's default
/// 300; 0 no cap). Off without a window (tests, the dedicated server)
/// and while `--bench` times views.
#[derive(Resource, Clone, Debug)]
pub struct FrameLimit {
    pub fps_max: f32,
    /// When the next frame may start.
    next: Option<Instant>,
}

impl Default for FrameLimit {
    fn default() -> Self {
        Self {
            fps_max: 300.0,
            next: None,
        }
    }
}

impl FrameLimit {
    /// How long to wait at `now` before the next frame, and the frame
    /// after's start. A frame that ran late starts the count again (no
    /// catching up with short frames).
    pub fn wait(&mut self, now: Instant) -> std::time::Duration {
        if self.fps_max <= 0.0 {
            self.next = None;
            return std::time::Duration::ZERO;
        }
        // Source clamps fps_max's low end; 10 keeps a typo playable.
        let frame = std::time::Duration::from_secs_f64(1.0 / self.fps_max.max(10.0) as f64);
        let start = match self.next {
            Some(next) if next > now => next,
            _ => now,
        };
        self.next = Some(start + frame);
        start - now
    }
}

fn limit_frames(mut limit: ResMut<FrameLimit>, windows: Query<(), With<bevy::window::PrimaryWindow>>) {
    if windows.is_empty() {
        limit.next = None;
        return;
    }
    let wait = limit.wait(Instant::now());
    if !wait.is_zero() {
        std::thread::sleep(wait);
    }
}

fn end_frame(mut times: ResMut<FrameTimes>) {
    let (Some(start), Some(prev)) = (times.started, times.last_start) else {
        return;
    };
    let frame = (start - prev).as_secs_f32();
    let cpu = start.elapsed().as_secs_f32();
    times.frames.push_back((frame, cpu));
    while times.frames.iter().map(|f| f.0).sum::<f32>() > 1.0 && times.frames.len() > 1 {
        times.frames.pop_front();
    }
}

#[derive(Component)]
struct PerfText;

fn spawn_overlay(mut commands: Commands, fonts: Res<super::fonts::UiFonts>) {
    commands.spawn((
        PerfText,
        Text::default(),
        fonts.debug(13.0),
        TextColor(Color::srgb(1.0, 1.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            top: px(8.0),
            right: px(8.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

/// Triangles per mesh asset, noted when a mesh is added or changed (the
/// render world takes the data of render-only meshes once extracted).
#[derive(Resource, Default)]
pub struct MeshTriangles(std::collections::HashMap<AssetId<Mesh>, usize>);

fn record_triangles(
    mut events: MessageReader<AssetEvent<Mesh>>,
    assets: Res<Assets<Mesh>>,
    mut triangles: ResMut<MeshTriangles>,
) {
    for e in events.read() {
        match e {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                if let Some(m) = assets.get(*id)
                    && let Ok(Some(i)) = m.try_indices_option()
                {
                    triangles.0.insert(*id, i.len() / 3);
                }
            }
            AssetEvent::Removed { id } | AssetEvent::Unused { id } => {
                triangles.0.remove(id);
            }
            _ => {}
        }
    }
}

/// GPU time of a frame, ms: the top-level render passes' timestamps
/// (smoothed). None without GPU timestamps (not Vulkan/DX12).
pub fn gpu_ms(diagnostics: &DiagnosticsStore) -> Option<f64> {
    let mut any = false;
    let mut sum = 0.0;
    for d in diagnostics.iter() {
        if let Some(pass) = d.path().as_str().strip_prefix("render/").and_then(|p| p.strip_suffix("/elapsed_gpu"))
            && !pass.contains('/')
        {
            any = true;
            sum += frame_mean(d).unwrap_or(0.0);
        }
    }
    any.then_some(sum)
}

/// A render pass's time per frame over its recent frames, all cameras
/// together. Each camera's run of a pass is its own measurement under the
/// same name (one sync of the render world's diagnostics gives them one
/// time), so the latest value or the moving average alone would be one
/// camera's: before the view-model camera stopped drawing with nothing
/// on its layer, mostly its empty passes.
pub fn frame_mean(d: &bevy::diagnostic::Diagnostic) -> Option<f64> {
    let (mut frames, mut sum, mut last) = (0u32, 0.0, None);
    for m in d.measurements() {
        if last != Some(m.time) {
            frames += 1;
            last = Some(m.time);
        }
        sum += m.value;
    }
    (frames > 0).then(|| sum / frames as f64)
}

/// A render pass's time in the latest frame, all cameras together (see
/// `frame_mean`).
pub fn frame_latest(d: &bevy::diagnostic::Diagnostic) -> Option<f64> {
    let t = d.measurement()?.time;
    Some(d.measurements().filter(|m| m.time == t).map(|m| m.value).sum())
}

/// CPU time this process has used so far, all threads (seconds); None
/// where it isn't read (only Linux so far). Unlike frame times, it barely
/// moves with other programs' load.
pub fn process_cpu_seconds() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        // /proc/self/stat: utime and stime are fields 14 and 15, in clock
        // ticks (100 per second on Linux).
        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        let rest = &stat[stat.rfind(')')? + 2..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        let ticks: f64 = fields.get(11)?.parse::<f64>().ok()? + fields.get(12)?.parse::<f64>().ok()?;
        Some(ticks / 100.0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Mesh counts this frame: total, drawn in some view, and their triangles.
pub fn mesh_counts(
    meshes: &Query<(&Mesh3d, &ViewVisibility), With<Aabb>>,
    triangles: &MeshTriangles,
) -> (usize, usize, usize) {
    let (mut total, mut drawn, mut tris) = (0, 0, 0);
    for (mesh, seen) in meshes {
        total += 1;
        if seen.get() {
            drawn += 1;
            tris += triangles.0.get(&mesh.0.id()).copied().unwrap_or(0);
        }
    }
    (total, drawn, tris)
}

/// What changes each frame, averaged over about a second: transforms
/// written (each dirties its tree for transform propagation; one under the
/// map's root dirties the map's), and mesh and material assets modified
/// (each is prepared for the GPU again, and any modified mesh makes every
/// mesh entity re-check its pipeline). Writes that store the same value
/// count too: avoid them.
#[derive(Resource, Default)]
struct Churn {
    frames: u32,
    started: Option<Instant>,
    sums: [u32; 4],
    names: std::collections::HashMap<String, u32>,
    /// `chimera_perf 3`: modified mesh and material assets over the
    /// second, by the kind and the name of an entity drawing them.
    assets: std::collections::HashMap<String, u32>,
    /// Last second's averages: transforms, of them under the map's root,
    /// meshes, materials.
    shown: [f32; 4],
}

#[allow(clippy::too_many_arguments)]
fn count_churn(
    perf: Res<Perf>,
    log: Res<PerfLog>,
    mut churn: ResMut<Churn>,
    changed: Query<Entity, Changed<Transform>>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    map_parts: Query<(), With<crate::map::MapPart>>,
    mut meshes: MessageReader<AssetEvent<Mesh>>,
    mut props: MessageReader<AssetEvent<crate::render::prop_material::PropMaterial>>,
    mut standard: MessageReader<AssetEvent<StandardMaterial>>,
    mut sprites: MessageReader<AssetEvent<crate::render::sprite_material::SpriteMaterial>>,
    mut particles: MessageReader<AssetEvent<crate::render::particles::ParticleDrawMaterial>>,
    mut decals: MessageReader<AssetEvent<crate::render::decal::DecalMaterial>>,
    mut world: MessageReader<AssetEvent<crate::render::world_material::WorldMaterial>>,
) {
    fn modified<A: Asset>(events: &mut MessageReader<AssetEvent<A>>) -> u32 {
        events.read().filter(|e| matches!(e, AssetEvent::Modified { .. })).count() as u32
    }
    let mesh_count = modified(&mut meshes);
    let material_count = modified(&mut props)
        + modified(&mut standard)
        + modified(&mut sprites)
        + modified(&mut particles)
        + modified(&mut decals)
        + modified(&mut world);
    if perf.show == 0 && log.0 == 0 {
        churn.started = None;
        return;
    }
    let churn = &mut *churn;
    let (mut total, mut in_map) = (0, 0);
    for e in &changed {
        total += 1;
        // The root of its tree, and the nearest name on the way up.
        let (mut root, mut label) = (e, names.get(e).ok());
        while let Ok(child_of) = parents.get(root) {
            root = child_of.parent();
            label = label.or_else(|| names.get(root).ok());
        }
        let label = label.map(|n| n.as_str().to_string());
        if root != e && map_parts.contains(root) {
            in_map += 1;
        }
        if perf.show >= 3 {
            let label: String = label.unwrap_or_else(|| "?".into()).chars().filter(|c| !c.is_ascii_digit()).collect();
            *churn.names.entry(label).or_default() += 1;
        }
    }
    churn.frames += 1;
    for (s, v) in churn.sums.iter_mut().zip([total, in_map, mesh_count, material_count]) {
        *s += v;
    }
    let now = Instant::now();
    let started = *churn.started.get_or_insert(now);
    if (now - started).as_secs_f32() < 1.0 {
        return;
    }
    let n = churn.frames.max(1) as f32;
    churn.shown = churn.sums.map(|s| s as f32 / n);
    if perf.show >= 3 {
        let mut names: Vec<_> = churn.names.drain().collect();
        names.sort_by(|a, b| b.1.cmp(&a.1));
        let top: Vec<String> = names.iter().take(12).map(|(k, v)| format!("{k}: {:.1}", *v as f32 / n)).collect();
        let mut assets: Vec<_> = churn.assets.drain().collect();
        assets.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let assets: Vec<String> = assets.iter().take(16).map(|(k, v)| format!("{k}: {:.1}", *v as f32 / n)).collect();
        info!(
            "per frame: {:.1} transforms written ({:.1} under the map's root), {:.1} meshes and {:.1} materials modified; \
             transforms by name: {}; assets modified by user: {}",
            churn.shown[0],
            churn.shown[1],
            churn.shown[2],
            churn.shown[3],
            top.join(", "),
            assets.join(", ")
        );
    }
    churn.frames = 0;
    churn.sums = [0; 4];
    churn.started = Some(now);
}

/// `chimera_perf 3`: count each modified asset of `C`'s kind (a mesh or a
/// material) under the name of an entity drawing it (the nearest `Name` up
/// its tree, digits left out), for `count_churn`'s log line: what rewrites
/// a mesh or material every frame (each write prepares it for the GPU
/// again, whether or not it changed).
fn name_modified<C: bevy::asset::AsAssetId>(
    mut events: MessageReader<AssetEvent<C::Asset>>,
    users: Query<(Entity, &C)>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    mut churn: ResMut<Churn>,
) {
    let modified: std::collections::HashSet<AssetId<C::Asset>> = events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    if modified.is_empty() {
        return;
    }
    let kind = std::any::type_name::<C::Asset>().rsplit("::").next().unwrap_or("asset");
    let mut found = std::collections::HashSet::new();
    for (e, c) in &users {
        let id = c.as_asset_id();
        if !modified.contains(&id) || !found.insert(id) {
            continue;
        }
        let mut at = e;
        let mut label = names.get(at).ok();
        while label.is_none()
            && let Ok(p) = parents.get(at)
        {
            at = p.parent();
            label = names.get(at).ok();
        }
        let label: String = label
            .map_or_else(|| "?".into(), |n| n.as_str().to_string())
            .chars()
            .filter(|c| !c.is_ascii_digit())
            .collect();
        *churn.assets.entry(format!("{kind} {label}")).or_default() += 1;
    }
    let unused = modified.len() - found.len();
    if unused > 0 {
        *churn.assets.entry(format!("{kind} (no entity)")).or_default() += unused as u32;
    }
}

/// `chimera_perf 3`: the UI trees laid out each frame (`bevy_ui` lays out
/// every root's tree, hidden or not), logged once a second: roots and their
/// nodes, of them in hidden trees, and nodes whose `Node` or text changed
/// this frame (each is synced to the layout again), by root name (an
/// unnamed root by its first named node, else by its own component types,
/// `<HudRoot>`).
#[allow(clippy::type_complexity)]
fn count_ui(
    roots: Query<(Entity, Option<&Name>, &InheritedVisibility), (With<Node>, Without<ChildOf>)>,
    children: Query<&Children>,
    nodes: Query<(), With<Node>>,
    changed: Query<Entity, (With<Node>, Or<(Changed<Node>, Changed<bevy::ui::ContentSize>, Changed<Text>)>)>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    texts: Query<&Text>,
    (entities, archetypes, components): (&bevy::ecs::entity::Entities, &bevy::ecs::archetype::Archetypes, &bevy::ecs::component::Components),
    mut at: Local<Option<Instant>>,
    mut changes: Local<(u32, std::collections::HashMap<String, u32>)>,
) {
    // An unnamed node by its own component types (not Bevy's): which
    // HUD or panel it is.
    let types = |e: Entity| -> String {
        let Ok(loc) = entities.get_spawned(e) else { return format!("{e}") };
        let ours: Vec<String> = archetypes[loc.archetype_id]
            .iter_components()
            .filter_map(|id| components.get_info(id))
            .map(|i| i.name().to_string())
            .filter(|n| !n.starts_with("bevy_"))
            .map(|n| n.rsplit("::").next().unwrap_or(&n).to_string())
            .take(2)
            .collect();
        if ours.is_empty() { format!("{e}") } else { format!("<{}>", ours.join("+")) }
    };
    changes.0 += 1;
    for e in &changed {
        // The nearest name up the tree, else the root; and the text, if any.
        let (mut at, mut label) = (e, names.get(e).ok().map(|n| n.as_str().to_string()));
        while let Ok(p) = parents.get(at) {
            at = p.parent();
            if label.is_none() {
                label = names.get(at).ok().map(|n| n.as_str().to_string());
            }
        }
        let mut label = label.unwrap_or_else(|| format!("root {}", types(at)));
        if let Ok(t) = texts.get(e) {
            label.push_str(&format!(" {:?}", t.0.chars().take(24).collect::<String>()));
        }
        *changes.1.entry(label).or_default() += 1;
    }
    let now = Instant::now();
    if at.is_some_and(|t| (now - t).as_secs_f32() < 1.0) {
        return;
    }
    *at = Some(now);
    let (mut all, mut hidden, mut hidden_roots) = (0usize, 0usize, 0usize);
    let mut trees: Vec<(String, usize, bool)> = Vec::new();
    for (root, name, vis) in &roots {
        let n = 1 + children.iter_descendants(root).filter(|c| nodes.contains(*c)).count();
        all += n;
        if !vis.get() {
            hidden += n;
            hidden_roots += 1;
        }
        // Unnamed roots by their first named node, else their own types.
        let label = name
            .or_else(|| children.iter_descendants(root).find_map(|c| names.get(c).ok()))
            .map_or_else(|| types(root), |n| n.as_str().to_string());
        trees.push((label, n, vis.get()));
    }
    trees.sort_by(|a, b| b.1.cmp(&a.1));
    let frames = changes.0.max(1) as f32;
    let mut changed: Vec<_> = changes.1.drain().collect();
    changed.sort_by(|a, b| b.1.cmp(&a.1));
    changes.0 = 0;
    info!(
        "ui: {all} nodes in {} roots, {hidden} in {hidden_roots} hidden roots; trees: {}; changed per frame by root: {}",
        trees.len(),
        trees
            .iter()
            .take(16)
            .map(|(n, c, v)| format!("{n} {c}{}", if *v { "" } else { " (hidden)" }))
            .collect::<Vec<_>>()
            .join(", "),
        changed
            .iter()
            .take(12)
            .map(|(n, c)| format!("{n}: {:.1}", *c as f32 / frames))
            .collect::<Vec<_>>()
            .join(", ")
    );
}

/// `chimera_perf_log 1`: log the readout once a second.
#[derive(Resource, Default)]
struct PerfLog(u8);

/// The latest `chimera_perf` readout as text lines: refreshed every frame
/// while the overlay or the log is on, else once a second (for
/// `bugreport`).
#[derive(Resource, Default)]
pub struct PerfReport {
    pub lines: Vec<String>,
    at: Option<Instant>,
    logged: Option<Instant>,
    /// The world's archetype count at the last readout: a count that keeps
    /// rising means some entity gets a new mix of components all the time,
    /// and every system's queries then check each new archetype.
    archetypes: usize,
}

#[allow(clippy::too_many_arguments)]
fn perf_report(
    churn: Res<Churn>,
    perf: Res<Perf>,
    log: Res<PerfLog>,
    times: Res<FrameTimes>,
    diagnostics: Res<DiagnosticsStore>,
    vis: Res<VisStats>,
    parts: Query<(), With<VisClusters>>,
    meshes: Query<(&Mesh3d, &ViewVisibility), With<Aabb>>,
    assets: Res<MeshTriangles>,
    mut report: ResMut<PerfReport>,
    interp: (Option<Res<crate::map::interp::Interpolation>>, Res<Time<Fixed>>),
    net: (Option<Res<crate::net::predict::NetGraph>>, Option<Res<crate::core::NetRole>>),
    render_time: Option<Res<RenderTime>>,
    metrics: (
        Option<ResMut<super::frame_metrics::FrameMetrics>>,
        Option<Res<crate::metrics::TickMetrics>>,
    ),
    bodies: (crate::map::prop_physics::BodyCounts, Query<&Name>),
    (cameras, archetypes): (Query<&Camera>, &bevy::ecs::archetype::Archetypes),
) {
    let now = Instant::now();
    let due = report.at.is_none_or(|t| (now - t).as_secs_f32() >= 1.0);
    if perf.show == 0 && log.0 == 0 && !due {
        return;
    }
    report.at = Some(now);
    report.lines = perf_lines(perf.show.max(log.0), &churn, &times, &diagnostics, &vis, parts.iter().count(), &meshes, &assets);
    // Each camera drawing runs its own passes (the view model's only with
    // something on its layer).
    let drawing = cameras.iter().filter(|c| c.is_active).count();
    let archetypes_before = report.archetypes;
    if let Some(line) = report.lines.iter_mut().find(|l| l.starts_with("entities ")) {
        line.push_str(&format!(", cameras {drawing}/{} drawing", cameras.iter().count()));
        let n = archetypes.len();
        line.push_str(&format!(", archetypes {n} (+{})", n.saturating_sub(archetypes_before)));
    }
    report.archetypes = archetypes.len();
    if let Some(r) = render_time
        && let Some(line) = report.lines.get_mut(1)
    {
        line.push_str(&format!(", render world {:.2} ms", r.ms()));
    }
    // Distributions over the last frames and ticks.
    if let Some(mut m) = metrics.0 {
        let lines = m.lines(metrics.1.as_deref());
        let at = 2.min(report.lines.len());
        report.lines.splice(at..at, lines);
    }
    if let Some(i) = interp.0 {
        report.lines.push(interp_line(&i, &interp.1));
    }
    report.lines.push(bodies_line(perf.show.max(log.0), &bodies.0, &bodies.1));
    if let Some(g) = net.0.filter(|_| net.1.is_some_and(|r| *r == crate::core::NetRole::Client)) {
        report.lines.extend(g.lines());
    }
    if log.0 != 0 && report.logged.is_none_or(|t| (now - t).as_secs_f32() >= 1.0) {
        report.logged = Some(now);
        info!("chimera_perf: {}", report.lines.join(" | "));
    }
}

/// Simulated bodies: dynamic ones and how many are awake (each awake one
/// costs the solver, transform propagation and its shadow's checks every
/// tick); `show` 3 or more names the awake ones (up to 12).
fn bodies_line(show: u8, bodies: &crate::map::prop_physics::BodyCounts, names: &Query<&Name>) -> String {
    let (dynamic, awake) = bodies.counts();
    let mut line = format!("physics: {dynamic} dynamic bodies, {awake} awake");
    if show >= 3 && awake > 0 {
        let named: Vec<String> = bodies
            .awake()
            .take(12)
            .map(|e| names.get(e).map_or_else(|_| format!("{e}"), |n| n.as_str().to_string()))
            .collect();
        line.push_str(&format!(" ({})", named.join(", ")));
    }
    line
}

/// Drawing between ticks (`cl_interpolate`, `map::interp`): the tick
/// rate, this frame's blend (overstep fraction) and ticks, snaps so far.
pub fn interp_line(i: &crate::map::interp::Interpolation, fixed: &Time<Fixed>) -> String {
    let hz = fixed.timestep().as_secs_f64().recip();
    if i.enabled == 0 {
        format!("interpolation off (cl_interpolate 0): latest of {hz:.1} Hz ticks drawn, {} this frame", i.ticks)
    } else {
        format!(
            "interpolation: {hz:.1} Hz ticks, drawn at {:.2} between the last two, {} this frame, {} snaps",
            i.fraction, i.ticks, i.snaps
        )
    }
}

/// The readout's lines (`show` 2 or more: every render pass).
#[allow(clippy::too_many_arguments)]
fn perf_lines(
    show: u8,
    churn: &Churn,
    times: &FrameTimes,
    diagnostics: &DiagnosticsStore,
    vis: &VisStats,
    tagged_parts: usize,
    meshes: &Query<(&Mesh3d, &ViewVisibility), With<Aabb>>,
    assets: &MeshTriangles,
) -> Vec<String> {
    let (avg, p95, max, cpu) = times.summary();
    let mut lines = vec![
        format!("{:.0} fps  frame {avg:.2} ms (p95 {p95:.2}, max {max:.2})", 1e3 / avg.max(1e-3)),
        format!("main world CPU {cpu:.2} ms"),
    ];
    // GPU and render-world CPU time per render pass (Vulkan/DX12 give GPU
    // timestamps; elsewhere only CPU).
    let mut passes: Vec<(String, f64, f64)> = Vec::new();
    for d in diagnostics.iter() {
        let path = d.path().as_str();
        if let Some(pass) = path.strip_prefix("render/").and_then(|p| p.strip_suffix("/elapsed_gpu")) {
            let cpu = diagnostics
                .get(&bevy::diagnostic::DiagnosticPath::new(format!("render/{pass}/elapsed_cpu")))
                .and_then(frame_mean)
                .unwrap_or(0.0);
            passes.push((pass.to_string(), frame_mean(d).unwrap_or(0.0), cpu));
        }
    }
    passes.sort_by(|a, b| b.1.total_cmp(&a.1));
    if !passes.is_empty() {
        let gpu = gpu_ms(diagnostics).unwrap_or(0.0);
        lines.push(format!("GPU {gpu:.2} ms (top-level passes)"));
        let shown = if show > 1 { passes.len() } else { 6 };
        for (pass, g, c) in passes.iter().take(shown) {
            lines.push(format!("  {pass}: gpu {g:.2} cpu {c:.2}"));
        }
    }
    let entities = diagnostics
        .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
    let (total, drawn, triangles) = mesh_counts(meshes, assets);
    lines.push(format!("entities {entities:.0}  meshes {drawn}/{total} drawn, {}k triangles", triangles / 1000));
    let [t, m, meshes_changed, materials] = churn.shown;
    lines.push(format!(
        "per frame: {t:.0} transforms written ({m:.0} under the map), {meshes_changed:.1} meshes, {materials:.1} materials modified"
    ));
    if vis.cluster.is_some() && vis.areas > 1 {
        lines.push(format!(
            "areas: in {}, reaching {}/{}, {} areaportals closed",
            vis.area, vis.visible_areas, vis.areas, vis.closed_portals
        ));
    }
    if vis.occluders > 0 {
        lines.push(format!(
            "occluders: {}/{} active, hiding {} map parts",
            vis.active_occluders, vis.occluders, vis.occluded_parts
        ));
    }
    match vis.cluster {
        Some(c) => lines.push(format!(
            "vis: cluster {c}, sees {}/{} clusters, {}/{} map parts",
            vis.visible_clusters, vis.clusters, vis.visible_parts, vis.parts
        )),
        None if vis.clusters > 0 => lines.push(format!("vis: off or outside the map ({tagged_parts} parts all drawn)")),
        None => lines.push("vis: map has no visibility data".into()),
    }
    lines
}

fn draw_overlay(
    perf: Res<Perf>,
    report: Res<PerfReport>,
    mut text: Single<(&mut Text, &mut Visibility), With<PerfText>>,
) {
    if perf.show == 0 {
        return;
    }
    let (text, visibility) = &mut *text;
    text.0 = report.lines.join("\n");
    visibility.set_if_neq(Visibility::Inherited);
}

/// Hide the overlay when `chimera_perf` goes back to 0.
fn hide_overlay(perf: Res<Perf>, mut text: Query<&mut Visibility, With<PerfText>>) {
    if perf.show == 0 {
        for mut v in &mut text {
            v.set_if_neq(Visibility::Hidden);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn fps_max_spaces_frames_without_catching_up() {
        let mut l = FrameLimit {
            fps_max: 100.0,
            next: None,
        };
        let t0 = Instant::now();
        // The first frame starts at once; the next 10 ms after it.
        assert_eq!(l.wait(t0), Duration::ZERO);
        assert_eq!(l.wait(t0 + Duration::from_millis(4)), Duration::from_millis(6));
        // A slow frame (30 ms later): no wait, and the count starts again.
        assert_eq!(l.wait(t0 + Duration::from_millis(50)), Duration::ZERO);
        assert_eq!(l.wait(t0 + Duration::from_millis(51)), Duration::from_millis(9));
        // 0: no cap.
        l.fps_max = 0.0;
        assert_eq!(l.wait(t0 + Duration::from_millis(52)), Duration::ZERO);
        assert_eq!(FrameLimit::default().fps_max, 300.0);
    }
}

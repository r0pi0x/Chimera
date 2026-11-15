//! Agent-facing capture modes:
//! - `--screenshot` / `--frames`: run a fixed number of frames, optionally
//!   save a screenshot of the window, then exit. The screenshot waits past
//!   `--frames` while the render world's pipelines still compile (at most
//!   `SHOT_PIPELINE_WAIT`; `PendingPipelines`): until the first ones are
//!   made nothing reaches the window, and a loaded machine (the post-push
//!   tests, other agents' builds) took 40 frames of a black image in about
//!   one offscreen run in five. The exit comes the frame after the shot
//!   arrives (`ShotSaved`), not in its observer: that runs during the
//!   render world's extraction (`gpu_readback`'s `sync_readbacks`), where
//!   `exit_drain` may already have looked for the exit and missed it, so
//!   the run ended while pipelines compiled (the driver crash it avoids).
//! - `screenshot <file.png>` (console, also over the remote console):
//!   save the window now, so a live run can be photographed step by step.
//! - `--views <file.json> --capture-dir <dir>`: visit a list of camera views
//!   and save one PNG per view, rendered off-screen at a fixed size so the
//!   result doesn't depend on how the window manager sizes the window.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        render_resource::{CachedPipelineState, PipelineCache, TextureFormat},
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
};
use serde::Deserialize;

use super::{ClientArgs, FirstPersonCamera};
use crate::console::ConsoleAppExt;
use crate::core::{Intent, LocalPlayer, Velocity};

const DEFAULT_FRAMES: u32 = 60;
/// Size of `--views` captures (CS:S comparisons use the same).
pub const VIEW_SIZE: UVec2 = UVec2::new(1280, 720);
/// Frames to let a view settle (interpolation, streaming) before capture.
const SETTLE_FRAMES: u32 = 30;
/// The first view settles longer: shader pipelines compile in the
/// background during the first frames, and meshes wait for them.
const FIRST_SETTLE_FRAMES: u32 = 150;
/// Frames timed per view by `--bench`, after settling.
const BENCH_FRAMES: u32 = 200;
/// The longest `--screenshot` waits past `--frames` for pipelines.
const SHOT_PIPELINE_WAIT: Duration = Duration::from_secs(30);

/// The render world's pipelines queued or compiling after its last frame,
/// shared by both worlds (`count_pending_pipelines`). Starts at 1: none
/// counted yet.
#[derive(Resource, Clone)]
struct PendingPipelines(Arc<AtomicUsize>);

impl Default for PendingPipelines {
    fn default() -> Self {
        Self(Arc::new(AtomicUsize::new(1)))
    }
}

fn count_pending_pipelines(cache: Option<Res<PipelineCache>>, pending: Res<PendingPipelines>) {
    let n = cache.map_or(0, |c| {
        c.pipelines()
            .filter(|p| matches!(p.state, CachedPipelineState::Queued | CachedPipelineState::Creating(_)))
            .count()
    });
    pending.0.store(n, Ordering::Relaxed);
}

/// `--screenshot`'s shot has arrived: the run ends next frame.
#[derive(Resource)]
struct ShotSaved;

/// One camera view, in engine space: eye position (meters) and look angles
/// in degrees (yaw 0 = -Z, positive pitch looks up).
#[derive(Clone, Debug, Deserialize)]
pub struct View {
    pub name: String,
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Resource)]
struct ViewRun {
    views: Vec<View>,
    dir: PathBuf,
    target: Handle<Image>,
    index: usize,
    frame: u32,
    waiting: bool,
    /// `--bench`: this view's frame times (seconds) and the results so far.
    times: Vec<f32>,
    /// Main-world CPU time of each timed frame (seconds).
    main_times: Vec<f32>,
    /// Render-world time of each timed frame (ms).
    render_times: Vec<f32>,
    /// Process CPU time when this view's timing started.
    cpu_start: Option<f64>,
    results: Vec<BenchRow>,
    /// Per-frame metrics of this view and of every view so far, and the
    /// simulation ticks' totals when this view's timing started.
    samples: BenchSamples,
    all: BenchSamples,
    ticks_start: Option<(u64, crate::metrics::Counts)>,
}

/// `--bench`: one value per timed frame for each metric
/// (`frame_metrics::FrameMetrics`), for percentiles.
#[derive(Default, Clone)]
struct BenchSamples {
    series: Vec<(&'static str, Vec<f64>)>,
}

impl BenchSamples {
    fn push(&mut self, name: &'static str, v: f64) {
        match self.series.iter_mut().find(|s| s.0 == name) {
            Some(s) => s.1.push(v),
            None => self.series.push((name, vec![v])),
        }
    }

    fn extend(&mut self, other: &BenchSamples) {
        for (n, v) in &other.series {
            for x in v {
                self.push(n, *x);
            }
        }
    }

    fn percentiles(&self, name: &str) -> Option<crate::metrics::Percentiles> {
        self.series
            .iter()
            .find(|s| s.0 == name)
            .and_then(|s| crate::metrics::Percentiles::of(&s.1))
    }

    /// Every metric's percentiles, in a fixed order.
    fn summary(&self) -> Vec<(&'static str, crate::metrics::Percentiles)> {
        BENCH_METRICS
            .iter()
            .filter_map(|n| Some((*n, self.percentiles(n)?)))
            .collect()
    }
}

/// What `--bench` records per frame (JSON and CSV keys).
const BENCH_METRICS: [&str; 16] = [
    "frame_ms",
    "main_ms",
    "main_cpu_ms",
    "main_instructions",
    "main_frame_instructions",
    "main_cycles",
    "main_cache_misses",
    "main_branch_misses",
    "render_ms",
    "render_cpu_ms",
    "render_instructions",
    "render_cycles",
    "render_cache_misses",
    "gpu_ms",
    "bodies_awake",
    "cameras_drawing",
];

/// A view's simulation ticks: count and means per tick.
#[derive(Clone, Copy, Default, serde::Serialize)]
struct TickSummary {
    ticks: u64,
    cpu_ms_mean: f64,
    instructions_mean: Option<f64>,
}

struct BenchRow {
    name: String,
    avg: f32,
    p95: f32,
    max: f32,
    /// Main-world CPU ms per frame (First to Last, `perf::FrameTimes`):
    /// with pipelined rendering, a frame takes the longer of this and the
    /// render world's time.
    main: f32,
    /// Render-world ms per frame (its `Render` schedule, `perf::RenderTime`).
    render: f32,
    meshes: (usize, usize, usize),
    parts: (usize, usize),
    /// Process CPU (all threads) and GPU ms per frame, when known.
    cpu: Option<f32>,
    gpu: Option<f32>,
    dist: Vec<(&'static str, crate::metrics::Percentiles)>,
    ticks: TickSummary,
}

#[derive(Component)]
struct ViewShot;

/// Makes Bevy re-measure every camera whose render target is an image of a
/// size other than the one it last measured.
///
/// Bevy's `camera_system` measures a camera's target again only when the
/// window or image asset changed, or the camera was added or got a new
/// projection, never when its `RenderTarget` component is swapped. A camera
/// moved to an image after its first frame (the `--views` capture, the sky
/// and view model cameras following it) kept the window's size as its
/// target size, so its depth texture came out window-sized (shared by all
/// cameras of that target) while the colour attachment was the image's, and
/// wgpu rejected the pass whenever the window and the image differed.
fn resync_image_targets(
    images: Res<Assets<Image>>,
    mut cameras: Query<(&Camera, &RenderTarget, &mut Projection)>,
) {
    for (camera, target, mut projection) in &mut cameras {
        let RenderTarget::Image(image) = target else { continue };
        let Some(image) = images.get(&image.handle) else { continue };
        if camera.computed.target_info.as_ref().map(|i| i.physical_size) != Some(image.size()) {
            projection.set_changed();
        }
    }
}

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingPipelines>()
            .add_systems(Update, (countdown, start_views, run_views.after(start_views)));
        let pending = app.world().resource::<PendingPipelines>().clone();
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render
                .insert_resource(pending)
                .add_systems(Render, count_pending_pipelines.in_set(RenderSystems::Cleanup));
        } else {
            pending.0.store(0, Ordering::Relaxed);
        }
        app.add_systems(
            PostUpdate,
            resync_image_targets.before(bevy::camera::CameraUpdateSystems),
        );
        app.console_command(
            "bugreport",
            "bugreport [note]: save a screenshot, your setpos/setang, map, weapon, build and recent console lines to a folder to send with a bug.",
            |w, a| {
                let dir = chimera_core::env::user_folder(&dirs::data_local_dir().ok_or("no per-user data folder")?)
                    .join("bugreports")
                    .join(chrono_stamp());
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                let info = report_text(w, &a.join(" "));
                std::fs::write(dir.join("report.txt"), &info).map_err(|e| format!("report: {e}"))?;
                w.spawn(Screenshot::primary_window())
                    .observe(save_to_disk(dir.join("screenshot.png")));
                Ok(Some(format!("bug report saved to {}", dir.display())))
            },
        );
        app.console_command(
            "screenshot",
            "screenshot <file.png>: save the window as it is now.",
            |w, a| {
                let path = PathBuf::from(a.first().ok_or("screenshot <file.png>")?);
                w.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
                Ok(Some(format!("saving {}", path.display())))
            },
        );
        app.console_command(
            "jpeg",
            "Save the window as it is now to the screenshots folder, named after the map (CS:S's F5; a PNG here).",
            |w, _| {
                let dir = chimera_core::env::user_folder(&dirs::data_local_dir().ok_or("no per-user data folder")?)
                    .join("screenshots");
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                let map = w
                    .get_resource::<crate::map::LoadedMapName>()
                    .map_or("screenshot".to_string(), |m| m.0.rsplit(':').next().unwrap_or("map").to_string());
                let path = numbered_shot(&dir, &map);
                w.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
                Ok(Some(format!("saving {}", path.display())))
            },
        );
    }
}

/// CS:S's screenshot names: the map and the first free four-digit number
/// (`de_dust20000.png`).
pub fn numbered_shot(dir: &std::path::Path, map: &str) -> PathBuf {
    let name = |n: u32| dir.join(format!("{map}{n:04}.png"));
    (0..10000).map(name).find(|p| !p.exists()).unwrap_or_else(|| name(9999))
}

fn countdown(
    mut commands: Commands,
    args: Res<ClientArgs>,
    pending: Res<PendingPipelines>,
    saved: Option<Res<ShotSaved>>,
    mut frame: Local<u32>,
    mut waiting: Local<Option<Instant>>,
    mut exit: MessageWriter<AppExit>,
) {
    let args = &args.0;
    if args.views.is_some() || (args.screenshot.is_none() && args.frames.is_none()) {
        return;
    }
    if saved.is_some() {
        exit.write(AppExit::Success);
        return;
    }
    let target = args.frames.unwrap_or(DEFAULT_FRAMES);
    if *frame < target {
        *frame += 1;
    }
    if *frame != target {
        return;
    }
    match &args.screenshot {
        Some(path) => {
            let left = pending.0.load(Ordering::Relaxed);
            let since = *waiting.get_or_insert_with(Instant::now);
            if left > 0 && since.elapsed() < SHOT_PIPELINE_WAIT {
                return;
            }
            if left > 0 {
                warn!("screenshot: {left} pipelines still compiling after {}s", SHOT_PIPELINE_WAIT.as_secs());
            }
            info!("saving screenshot to {}", path.display());
            // Once: the frame count moves past the target.
            *frame += 1;
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path.clone()))
                .observe(|_: On<ScreenshotCaptured>, mut commands: Commands| {
                    commands.insert_resource(ShotSaved);
                });
        }
        None => {
            exit.write(AppExit::Success);
        }
    }
}

fn start_views(
    mut commands: Commands,
    args: Res<ClientArgs>,
    run: Option<Res<ViewRun>>,
    camera: Option<Single<Entity, With<FirstPersonCamera>>>,
    mut images: ResMut<Assets<Image>>,
    mut exit: MessageWriter<AppExit>,
    mut windows: Query<&mut Window>,
) {
    let (Some(path), None, Some(camera)) = (&args.0.views, run, camera) else {
        return;
    };
    let views: Vec<View> = match std::fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            error!("--views {}: {e}", path.display());
            exit.write(AppExit::error());
            return;
        }
    };
    let dir = args.0.capture_dir.clone().unwrap_or_else(|| PathBuf::from("."));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        error!("--capture-dir {}: {e}", dir.display());
        exit.write(AppExit::error());
        return;
    }
    let size = args.0.view_size.unwrap_or(VIEW_SIZE);
    let target = images.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    commands
        .entity(*camera)
        .insert(RenderTarget::Image(target.clone().into()));
    if args.0.bench {
        // Time what the GPU can do, not the display's refresh.
        for mut w in &mut windows {
            w.present_mode = bevy::window::PresentMode::AutoNoVsync;
        }
        commands.queue(|w: &mut World| {
            if let Some(mut l) = w.get_resource_mut::<super::perf::FrameLimit>() {
                l.fps_max = 0.0;
            }
        });
        info!("timing {} views at {}x{}", views.len(), size.x, size.y);
    } else {
        info!("capturing {} views into {}", views.len(), dir.display());
    }
    commands.insert_resource(ViewRun {
        views,
        dir,
        target,
        index: 0,
        frame: 0,
        waiting: false,
        times: Vec::new(),
        main_times: Vec::new(),
        render_times: Vec::new(),
        cpu_start: None,
        results: Vec::new(),
        samples: BenchSamples::default(),
        all: BenchSamples::default(),
        ticks_start: None,
    });
}

#[allow(clippy::too_many_arguments)]
fn run_views(
    mut commands: Commands,
    (args, hooks): (Res<ClientArgs>, Option<Res<super::AppHooks>>),
    run: Option<ResMut<ViewRun>>,
    mut player: Query<(&mut Transform, &mut Intent, &mut Velocity), With<LocalPlayer>>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<Real>>,
    meshes: Query<(&Mesh3d, &ViewVisibility), With<bevy::camera::primitives::Aabb>>,
    assets: Res<super::perf::MeshTriangles>,
    vis: Res<crate::render::vis::VisStats>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    frame_times: Res<super::perf::FrameTimes>,
    render_time: Res<super::perf::RenderTime>,
    metrics: (
        Res<super::frame_metrics::FrameMetrics>,
        Option<Res<crate::metrics::TickMetrics>>,
    ),
    bodies: crate::map::prop_physics::BodyCounts,
    cameras: Query<&Camera>,
) {
    let Some(mut run) = run else { return };
    if run.waiting {
        return;
    }
    let Some(view) = run.views.get(run.index).cloned() else {
        if args.0.bench {
            print_bench(&run.results);
            let overall = run.all.summary();
            let build = hooks.map_or("unknown", |h| h.build);
            write_bench_files(&run.dir, &run.results, &overall, args.0.view_size.unwrap_or(VIEW_SIZE), build);
        } else {
            info!("all views captured");
        }
        exit.write(AppExit::Success);
        return;
    };
    let Ok((mut transform, mut intent, mut velocity)) = player.single_mut() else {
        return;
    };
    // Hold the camera on the view every frame (movement must be noclip).
    transform.translation = Vec3::from(view.position);
    intent.yaw = view.yaw.to_radians();
    intent.pitch = view.pitch.to_radians();
    intent.move_axis = Vec2::ZERO;
    velocity.0 = Vec3::ZERO;
    run.frame += 1;
    let settle = if run.index == 0 { FIRST_SETTLE_FRAMES } else { SETTLE_FRAMES };
    if run.frame < settle {
        return;
    }
    if args.0.bench {
        // Frame times as seen at this point of each frame: the time since
        // the previous frame started.
        if run.times.is_empty() {
            run.cpu_start = super::perf::process_cpu_seconds();
            run.ticks_start = metrics.1.as_ref().map(|t| (t.ticks, t.total));
        }
        run.times.push(time.delta_secs());
        // The latest complete frame's metrics (the previous frame's).
        let (m, s) = (&metrics.0, &mut run.samples);
        s.push("frame_ms", time.delta_secs_f64() * 1e3);
        s.push("main_ms", m.last.main_ms);
        s.push("main_cpu_ms", m.last.main.cpu_ns as f64 / 1e6);
        if let Some(hw) = m.last.main.hw {
            s.push("main_instructions", hw.instructions as f64);
            s.push("main_cycles", hw.cycles as f64);
            s.push("main_cache_misses", hw.cache_misses as f64);
            s.push("main_branch_misses", hw.branch_misses as f64);
        }
        if let Some(i) = m.last.main_frame_instructions {
            s.push("main_frame_instructions", i as f64);
        }
        if let Some(r) = m.last_render {
            s.push("render_ms", r.ms);
            s.push("render_cpu_ms", r.counts.cpu_ns as f64 / 1e6);
            if let Some(hw) = r.counts.hw {
                s.push("render_instructions", hw.instructions as f64);
                s.push("render_cycles", hw.cycles as f64);
                s.push("render_cache_misses", hw.cache_misses as f64);
            }
        }
        if let Some(g) = m.last.gpu_ms {
            s.push("gpu_ms", g);
        }
        s.push("bodies_awake", bodies.counts().1 as f64);
        // Each camera drawing runs its own passes (the view model's only
        // with something on its layer).
        s.push("cameras_drawing", cameras.iter().filter(|c| c.is_active).count() as f64);
        if let Some((_, main)) = frame_times.frames.back() {
            run.main_times.push(*main);
        }
        run.render_times.push(render_time.ms());
        if run.frame < settle + BENCH_FRAMES {
            return;
        }
        let mut t = std::mem::take(&mut run.times);
        t.sort_by(f32::total_cmp);
        let avg = t.iter().sum::<f32>() / t.len() as f32;
        let main = std::mem::take(&mut run.main_times);
        let render = std::mem::take(&mut run.render_times);
        let samples = std::mem::take(&mut run.samples);
        run.all.extend(&samples);
        let ticks = match (run.ticks_start.take(), metrics.1.as_ref()) {
            (Some((n0, c0)), Some(t)) if t.ticks > n0 => {
                let n = t.ticks - n0;
                let d = t.total - c0;
                TickSummary {
                    ticks: n,
                    cpu_ms_mean: d.cpu_ns as f64 / 1e6 / n as f64,
                    instructions_mean: d.hw.map(|h| h.instructions as f64 / n as f64),
                }
            }
            _ => TickSummary::default(),
        };
        let row = BenchRow {
            dist: samples.summary(),
            ticks,
            main: main.iter().sum::<f32>() / main.len().max(1) as f32 * 1e3,
            render: render.iter().sum::<f32>() / render.len().max(1) as f32,
            name: view.name.clone(),
            avg: avg * 1e3,
            p95: t[(t.len() * 95 / 100).min(t.len() - 1)] * 1e3,
            max: t[t.len() - 1] * 1e3,
            meshes: super::perf::mesh_counts(&meshes, &assets),
            parts: (vis.visible_parts, vis.parts),
            cpu: run
                .cpu_start
                .zip(super::perf::process_cpu_seconds())
                .map(|(a, b)| ((b - a) * 1e3 / t.len() as f64) as f32),
            gpu: super::perf::gpu_ms(&diagnostics).map(|g| g as f32),
        };
        run.results.push(row);
        run.index += 1;
        run.frame = 0;
        return;
    }
    run.waiting = true;
    let path = run.dir.join(format!("{}.png", view.name));
    commands
        .spawn((ViewShot, Screenshot::image(run.target.clone())))
        .observe(save_to_disk(path))
        .observe(|_: On<ScreenshotCaptured>, mut run: ResMut<ViewRun>| {
            run.index += 1;
            run.frame = 0;
            run.waiting = false;
        });
}

/// Print `--bench` results: one row per view, then the averages. Frame
/// times follow machine load; process CPU per frame (all threads), main-world
/// CPU and GPU time less.
fn print_bench(rows: &[BenchRow]) {
    let opt = |v: Option<f32>| v.map_or("n/a".to_string(), |v| format!("{v:.2}"));
    println!(
        "{:<24} {:>8} {:>8} {:>8} {:>8} {:>9} {:>8} {:>8} {:>10} {:>9} {:>11}",
        "view", "avg ms", "p95 ms", "max ms", "main ms", "render ms", "cpu ms", "gpu ms", "meshes", "tris (k)", "vis parts"
    );
    for r in rows {
        println!(
            "{:<24} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>9.2} {:>8} {:>8} {:>10} {:>9} {:>11}",
            r.name,
            r.avg,
            r.p95,
            r.max,
            r.main,
            r.render,
            opt(r.cpu),
            opt(r.gpu),
            format!("{}/{}", r.meshes.1, r.meshes.0),
            r.meshes.2 / 1000,
            format!("{}/{}", r.parts.0, r.parts.1),
        );
    }
    let n = rows.len().max(1) as f32;
    let mean = |f: &dyn Fn(&BenchRow) -> f32| rows.iter().map(f).sum::<f32>() / n;
    let mean_opt = |f: &dyn Fn(&BenchRow) -> Option<f32>| {
        rows.iter().map(f).collect::<Option<Vec<f32>>>().map(|v| v.iter().sum::<f32>() / n)
    };
    println!(
        "{:<24} {:>8.2} {:>8.2} {:>8} {:>8.2} {:>9.2} {:>8} {:>8} {:>10.0} {:>9.0}",
        "MEAN",
        mean(&|r| r.avg),
        mean(&|r| r.p95),
        "",
        mean(&|r| r.main),
        mean(&|r| r.render),
        opt(mean_opt(&|r| r.cpu)),
        opt(mean_opt(&|r| r.gpu)),
        mean(&|r| r.meshes.1 as f32),
        mean(&|r| r.meshes.2 as f32) / 1000.0,
    );
}

type Dist = [(&'static str, crate::metrics::Percentiles)];

/// The distribution table: frame time percentiles and 1% low, each world's
/// CPU time and instructions per frame (load-independent; compare these
/// across runs), ticks' mean instructions.
fn print_bench_dist(rows: &[BenchRow], overall: &Dist) {
    use crate::metrics::short;
    println!(
        "{:<24} {:>8} {:>8} {:>8} {:>8} {:>7} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "view",
        "p50 ms",
        "p99 ms",
        "p99.9 ms",
        "max ms",
        "1%low",
        "main cpu",
        "rend cpu",
        "main ins*",
        "main p99*",
        "rend ins",
        "rend p99",
        "tick ins"
    );
    let line = |name: &str, dist: &Dist, tick: Option<f64>| {
        let get = |k: &str| dist.iter().find(|d| d.0 == k).map(|d| d.1);
        let f = get("frame_ms").unwrap_or_default();
        let ms = |k: &str| get(k).map_or("n/a".into(), |p| format!("{:.2}", p.p50));
        let ins = |k: &str, p99: bool| get(k).map_or("n/a".into(), |p| short(if p99 { p.p99 } else { p.p50 }));
        println!(
            "{:<24} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>7.0} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
            name,
            f.p50,
            f.p99,
            f.p999,
            f.max,
            f.low_1pct_fps(),
            ms("main_cpu_ms"),
            ms("render_cpu_ms"),
            ins("main_frame_instructions", false),
            ins("main_frame_instructions", true),
            ins("render_instructions", false),
            ins("render_instructions", true),
            tick.map_or("n/a".into(), short),
        );
    };
    for r in rows {
        line(&r.name, &r.dist, r.ticks.instructions_mean);
    }
    let ticks: Vec<f64> = rows.iter().filter_map(|r| r.ticks.instructions_mean).collect();
    let tick = (!ticks.is_empty()).then(|| ticks.iter().sum::<f64>() / ticks.len() as f64);
    line("ALL", overall, tick);
    println!(
        "*: main world instructions per frame less its simulation ticks' (tick ins: per tick); hardware counters: {}",
        crate::metrics::hw_status()
    );
}

/// `bench.json` and `bench.csv` in the capture folder: every metric's
/// percentiles per view and over all views, for scripts.
fn write_bench_files(dir: &std::path::Path, rows: &[BenchRow], overall: &Dist, size: UVec2, build: &str) {
    print_bench_dist(rows, overall);
    let dist_json = |d: &Dist| {
        serde_json::Value::Object(
            d.iter()
                .map(|(k, p)| (k.to_string(), serde_json::to_value(p).unwrap()))
                .collect(),
        )
    };
    let views: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "metrics": dist_json(&r.dist),
                "ticks": r.ticks,
                "process_cpu_ms": r.cpu,
                "meshes_drawn": r.meshes.1,
                "meshes": r.meshes.0,
                "triangles_drawn": r.meshes.2,
                "vis_parts": [r.parts.0, r.parts.1],
            })
        })
        .collect();
    let json = serde_json::json!({
        "build": build,
        "view_size": [size.x, size.y],
        "hardware_counters": crate::metrics::hw_status(),
        "views": views,
        "overall": { "metrics": dist_json(overall) },
    });
    let mut csv = String::from("view,metric,n,mean,p50,p90,p99,p99.9,max,worst_1pct_mean\n");
    let all = rows
        .iter()
        .map(|r| (r.name.as_str(), r.dist.as_slice()))
        .chain(std::iter::once(("ALL", overall)));
    for (name, dist) in all {
        for (k, p) in dist {
            csv += &format!(
                "{name},{k},{},{},{},{},{},{},{},{}\n",
                p.n, p.mean, p.p50, p.p90, p.p99, p.p999, p.max, p.worst_1pct_mean
            );
        }
    }
    for (file, text) in [
        ("bench.json", serde_json::to_string_pretty(&json).unwrap_or_default()),
        ("bench.csv", csv),
    ] {
        let path = dir.join(file);
        match std::fs::write(&path, text) {
            Ok(()) => println!("wrote {}", path.display()),
            Err(e) => error!("{}: {e}", path.display()),
        }
    }
}

/// A folder name from the current time (UTC seconds since 1970).
fn chrono_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("report-{secs}")
}

/// What a bug report says besides the screenshot.
pub fn report_text(w: &mut World, note: &str) -> String {
    use crate::{console::Console, core::LocalPlayer};
    let version = w.get_resource::<crate::core::BuildVersion>().map_or("unknown", |v| v.0);
    let mut out = format!("Chimera build {version} ({})\n", crate::net::NET_VERSION);
    if !note.is_empty() {
        out += &format!("note: {note}\n");
    }
    if let Some(m) = w.get_resource::<crate::map::LoadedMapName>() {
        out += &format!("map: {}\n", m.0);
    }
    let player = w.query_filtered::<Entity, With<LocalPlayer>>().iter(w).next();
    if let Some(p) = player {
        if let (Some(t), Some(i)) = (w.get::<Transform>(p), w.get::<crate::core::Intent>(p)) {
            // The feet, as setpos takes them (a line to paste back).
            let feet = t.translation
                + w.get::<crate::core::MovementState>(p).map_or(Vec3::ZERO, |s| Vec3::Y * s.hull_min.y);
            let s = Vec3::new(feet.x, -feet.z, feet.y) / 0.0254;
            out += &format!(
                "setpos {:.2} {:.2} {:.2};setang {:.2} {:.2} 0.00\n",
                s.x,
                s.y,
                s.z,
                -i.pitch.to_degrees(),
                (i.yaw.to_degrees() + 90.0).rem_euclid(360.0)
            );
        }
        let weapon = w
            .get::<crate::weapon::Inventory>(p)
            .and_then(|inv| inv.active)
            .and_then(|e| w.get::<crate::weapon::Weapon>(e))
            .map(|wpn| wpn.id);
        out += &format!("weapon: {}\n", weapon.unwrap_or("none"));
        if let Some(h) = w.get::<crate::core::Health>(p) {
            out += &format!("health: {:.0}\n", h.current * 100.0);
        }
    }
    if let Some(p) = w.get_resource::<super::perf::PerfReport>()
        && !p.lines.is_empty()
    {
        out += "\nperformance (chimera_perf, last second):\n";
        for l in &p.lines {
            out += &format!("{l}\n");
        }
    }
    if let Some(c) = w.get_resource::<Console>() {
        out += "\nlast console lines:\n";
        let n = c.output.len();
        for l in &c.output[n.saturating_sub(40)..] {
            out += &format!("{}\n", l.text);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn jpeg_names_take_the_next_free_number() {
        let dir = std::env::temp_dir().join(format!("chimera-jpeg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(super::numbered_shot(&dir, "de_dust2"), dir.join("de_dust20000.png"));
        std::fs::write(dir.join("de_dust20000.png"), b"").unwrap();
        assert_eq!(super::numbered_shot(&dir, "de_dust2"), dir.join("de_dust20001.png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod retarget_tests {
    use bevy::{
        camera::RenderTarget,
        prelude::*,
        render::{camera::camera_system, render_resource::TextureFormat, texture::ManualTextureViews},
        window::{WindowCreated, WindowResized, WindowScaleFactorChanged},
    };

    use super::resync_image_targets;

    /// A camera moved to an image of another size is measured as that image
    /// (not as its first target, the window's size), the regression behind
    /// `refcmp capture-ours` failing with a window-sized depth texture.
    #[test]
    fn retargeted_camera_takes_the_new_image_size() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Image>();
        app.init_resource::<ManualTextureViews>();
        app.add_message::<WindowResized>()
            .add_message::<WindowCreated>()
            .add_message::<WindowScaleFactorChanged>();
        app.add_systems(PostUpdate, (resync_image_targets, camera_system).chain());
        let add = |app: &mut App, w, h| {
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .add(Image::new_target_texture(w, h, TextureFormat::Rgba8Unorm, None))
        };
        let window_sized = add(&mut app, 1920, 1080);
        let camera = app
            .world_mut()
            .spawn((Camera3d::default(), RenderTarget::Image(window_sized.into())))
            .id();
        app.update();
        let size = |app: &App| app.world().get::<Camera>(camera).unwrap().physical_target_size();
        assert_eq!(size(&app), Some(UVec2::new(1920, 1080)));
        let view_sized = add(&mut app, 1280, 720);
        // The new image's Added event is gone by the swap (as for the sky
        // and view model cameras, which follow the main one a frame later).
        app.update();
        app.update();
        app.world_mut().entity_mut(camera).insert(RenderTarget::Image(view_sized.into()));
        app.update();
        assert_eq!(size(&app), Some(UVec2::new(1280, 720)));
    }
}

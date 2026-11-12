//! `--offscreen <WxH>`: the whole client with no window system, for
//! measurements, screenshots and clips on a machine with a GPU (or a
//! software adapter) but no display or compositor. `app::default_plugins`
//! leaves winit out (Bevy's schedule runner drives the frames), but the
//! primary `Window` entity stays, pinned to that size at scale 1, so
//! everything that sizes or targets by it (cameras' `RenderTarget`s, the
//! default UI camera, the HUD's layout, the console) works unchanged. Its
//! surface is the one thing missing: the render world gets an
//! `ExtractedWindow` for it (`extract_window`; Bevy's own extraction
//! skips windows without a native handle) whose swap-chain view is an
//! image's (`attach_image`), so every camera that draws to the window,
//! with the render graph, post effects and pipelined rendering, draws into
//! that image, and presenting is a no-op.
//!
//! Screenshots of the window (`--screenshot`, the `screenshot` command,
//! `bugreport`, freeze cams) can't go through Bevy's window path, which
//! needs a surface: an observer turns each into a one-shot readback of the
//! image (`ReadbackOnce`) and triggers the same `ScreenshotCaptured` on
//! the same entity when it arrives, so their observers (`save_to_disk`)
//! run as they would.
//!
//! Also here, with or without a window: `--dump <dir>` (`FrameDumps`)
//! saves every `--dump-every`th frame (default 60) as `frame_NNNNNN.png`,
//! encoding on threads of their own, and with `--dump-every 1` and ffmpeg
//! on the PATH writes `clip.mp4` at the frames' real rate when the run
//! ends. With `--offscreen` the sound is recorded too
//! (`chimera_present::audio::record`, added by `app::default_plugins`):
//! `audio.wav` in the same directory, and `frames.tsv` gives each saved
//! frame's sample position in it, the time its sounds start, so
//! `tools/dump_to_mp4.py` puts frames and sound together in sync however
//! slowly the frames rendered. An offscreen run prints its frame times (wall, main world,
//! render world: percentiles over the whole run) when it ends;
//! `--frame-log` has each frame (`frame_metrics`).
//!
//! No sound device either (`silence_audio_device`, called by `app::run`
//! before anything starts a thread): on Linux ALSA's default device is
//! its null one for the run, unless `ALSA_CONFIG_PATH` is set. Bevy's
//! audio opens the default output while its plugin builds, on the main
//! thread, and where that fails (no `XDG_RUNTIME_DIR`, as in agents'
//! and tools' shells: PipeWire's and Pulse's plugins can't reach the
//! server) rodio tries every ALSA device in turn; PipeWire's JACK one
//! then sometimes never returns from closing its client
//! (`jack_client_close` waiting in `pw_thread_loop_stop`), and the run
//! froze for good right after the GPU adapter's log line, before any
//! frame (7 of 51 starts on the dev box under load, 2026-11-10).
//!
//! Known shortcuts:
//! - The image is `Rgba8UnormSrgb`; a window's surface is usually
//!   `Bgra8UnormSrgb`. Pipelines specialize on the view's format, so the
//!   work is the same, but a pass that can't draw to one of them would
//!   show only here.
//! - No vsync and no presentation wait: the frame's GPU time shows in the
//!   render world only when the GPU falls behind (`fps_max` still spaces
//!   frames; `+fps_max 0` for none).
//! - The dumps whose readback hasn't arrived when the run ends (the last
//!   one or two) are lost.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

use bevy::{
    asset::RenderAssetUsages,
    camera::{CameraUpdateSystems, RenderTarget},
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        gpu_readback::{ReadbackComplete, ReadbackOnce},
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        sync_world::RenderEntity,
        texture::GpuImage,
        view::{
            prepare_view_attachments,
            screenshot::{Captured, Screenshot, ScreenshotCaptured},
            window::{ExtractedWindow, SurfaceData, prepare_windows},
        },
    },
    window::{PrimaryWindow, WindowRef},
};

use super::ClientArgs;

/// ALSA's configuration for an offscreen run: every output into the
/// null device (`silence_audio_device`).
const NULL_ALSA_CONFIG: &str = "pcm.!default { type null }\nctl.!default { type hw card 0 }\n";

/// `--offscreen` on Linux: point ALSA (`ALSA_CONFIG_PATH`) at a
/// configuration whose default output is the null device, so Bevy's
/// audio opens it at once and never searches the system's devices (see
/// the module doc). Does nothing without `--offscreen`, elsewhere than
/// Linux, or when `ALSA_CONFIG_PATH` is set already. Call it before the
/// process starts any thread: it sets an environment variable.
pub fn silence_audio_device(args: &super::Args) {
    if args.offscreen.is_none() || !cfg!(target_os = "linux") || std::env::var_os("ALSA_CONFIG_PATH").is_some() {
        return;
    }
    let path = std::env::temp_dir().join("chimera-offscreen-alsa.conf");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(NULL_ALSA_CONFIG) {
        // Whole or not at all, for runs starting side by side.
        let part = path.with_extension(format!("conf.{}", std::process::id()));
        if std::fs::write(&part, NULL_ALSA_CONFIG).and_then(|()| std::fs::rename(&part, &path)).is_err() {
            let _ = std::fs::remove_file(&part);
            eprintln!("offscreen: couldn't write {}: the sound device stays", path.display());
            return;
        }
    }
    // SAFETY: called from `app::run` before the app (or anything else of
    // the process) has started a thread.
    unsafe { std::env::set_var("ALSA_CONFIG_PATH", &path) };
}

/// Frames between `--dump` frames without `--dump-every`.
const DEFAULT_DUMP_EVERY: u32 = 60;

/// The image an offscreen run's window is drawn into, and its size.
#[derive(Resource, Clone)]
pub struct Offscreen {
    pub image: Handle<Image>,
    pub size: UVec2,
}

/// The render world's copy of `Offscreen::image`.
#[derive(Resource, Clone, Copy)]
struct OffscreenImage(AssetId<Image>, UVec2);

/// Marks a window screenshot turned into a readback of the image.
#[derive(Component)]
struct OffscreenShot;

pub struct OffscreenPlugin;

impl Plugin for OffscreenPlugin {
    fn build(&self, app: &mut App) {
        let Some(args) = app.world().get_resource::<ClientArgs>().map(|a| a.0.clone()) else {
            return;
        };
        if let Some(dir) = args.dump_dir.clone() {
            app.insert_resource(FrameDumps::new(dir, args.dump_every.unwrap_or(DEFAULT_DUMP_EVERY)))
                .add_systems(Last, dump_frame);
        }
        let Some(size) = args.offscreen else { return };
        let Some(mut images) = app.world_mut().get_resource_mut::<Assets<Image>>() else {
            return;
        };
        let mut image = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
        // Read back for screenshots; never needed on the CPU otherwise.
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        image.asset_usage = RenderAssetUsages::RENDER_WORLD;
        let image = images.add(image);
        info!("offscreen: drawing the window into a {}x{} image", size.x, size.y);
        app.insert_resource(Offscreen {
            image: image.clone(),
            size,
        })
        .insert_resource(RunTimes {
            size,
            last: None,
            wall: Vec::new(),
            main: Vec::new(),
            render: Vec::new(),
        })
        .add_observer(redirect_screenshot)
        .add_observer(readback_to_screenshot)
        .add_systems(Startup, no_ime)
        .add_systems(First, record_frame)
        .add_systems(PostUpdate, pin_window.before(CameraUpdateSystems));
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render
                .insert_resource(OffscreenImage(image.id(), size))
                .add_systems(ExtractSchedule, extract_window)
                .add_systems(
                    Render,
                    attach_image
                        .in_set(RenderSystems::PrepareViews)
                        .after(prepare_assets::<GpuImage>)
                        .after(prepare_windows)
                        .before(prepare_view_attachments),
                );
        }
    }
}

/// No text input method without winit: egui's IME placement asks winit
/// for the window and warns every frame it isn't there.
fn no_ime(settings: Option<ResMut<bevy_egui::EguiGlobalSettings>>) {
    if let Some(mut s) = settings {
        s.enable_ime = false;
    }
}

/// Keep the window at the image's size and scale 1: video settings
/// (`mat_setvideomode`, the user's video config) resize it as they would
/// a real one, and cameras sized for another size than the image's would
/// draw past it.
fn pin_window(offscreen: Res<Offscreen>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    for mut window in &mut windows {
        let size = offscreen.size;
        if window.resolution.physical_size() != size || window.resolution.scale_factor() != 1.0 {
            window.resolution.set_scale_factor_override(Some(1.0));
            window.resolution.set_physical_resolution(size.x, size.y);
        }
    }
}

/// The primary window into the render world as Bevy's `extract_windows`
/// would put it, without a surface (that needs a native handle).
fn extract_window(
    mut commands: Commands,
    windows: Extract<Query<(RenderEntity, &Window), With<PrimaryWindow>>>,
    mut extracted: Query<&mut ExtractedWindow>,
) {
    for (entity, window) in &windows {
        let (width, height) = (
            window.resolution.physical_width().max(1),
            window.resolution.physical_height().max(1),
        );
        match extracted.get_mut(entity) {
            Ok(mut e) => {
                e.size_changed = e.physical_width != width || e.physical_height != height;
                e.physical_width = width;
                e.physical_height = height;
            }
            Err(_) => {
                commands.entity(entity).insert((
                    ExtractedWindow {
                        physical_width: width,
                        physical_height: height,
                        present_mode: window.present_mode,
                        desired_maximum_frame_latency: window.desired_maximum_frame_latency,
                        swap_chain_texture_view: None,
                        swap_chain_texture: None,
                        swap_chain_texture_format: None,
                        swap_chain_texture_view_format: None,
                        size_changed: false,
                        present_mode_changed: false,
                        alpha_mode: window.composite_alpha_mode,
                        needs_initial_present: false,
                    },
                    PrimaryWindow,
                ));
            }
        }
    }
}

/// The image as the window's swap-chain view, before the views' targets
/// are made (`prepare_view_attachments`), when the sizes agree.
fn attach_image(
    image: Res<OffscreenImage>,
    images: Res<RenderAssets<GpuImage>>,
    mut windows: Query<&mut ExtractedWindow, Without<SurfaceData>>,
) {
    let Some(gpu) = images.get(image.0) else { return };
    for mut window in &mut windows {
        let fits = UVec2::new(window.physical_width, window.physical_height) == image.1;
        window.swap_chain_texture_view = fits.then(|| gpu.texture_view.clone());
        window.swap_chain_texture_format = Some(gpu.texture_descriptor.format);
        window.swap_chain_texture_view_format = Some(gpu.view_format());
    }
}

/// A screenshot of the window, offscreen: a readback of the image instead
/// (`readback_to_screenshot` finishes it).
fn redirect_screenshot(
    add: On<Add<Screenshot>>,
    shots: Query<&Screenshot>,
    primary: Query<(), With<PrimaryWindow>>,
    offscreen: Res<Offscreen>,
    mut commands: Commands,
) {
    let Ok(shot) = shots.get(add.entity) else { return };
    let of_window = match &shot.0 {
        RenderTarget::Window(WindowRef::Primary) => true,
        RenderTarget::Window(WindowRef::Entity(w)) => primary.contains(*w),
        _ => false,
    };
    if of_window {
        commands
            .entity(add.entity)
            .remove::<Screenshot>()
            .insert((OffscreenShot, ReadbackOnce::texture(offscreen.image.clone())));
    }
}

/// The readback's pixels as the `ScreenshotCaptured` Bevy would have
/// triggered, on the screenshot's entity (`Captured`: Bevy despawns it).
fn readback_to_screenshot(
    done: On<ReadbackComplete>,
    shots: Query<(), With<OffscreenShot>>,
    offscreen: Res<Offscreen>,
    mut commands: Commands,
) {
    let entity = done.entity;
    if !shots.contains(entity) {
        return;
    }
    let image = rgba_image(&done.data, offscreen.size);
    commands.entity(entity).remove::<OffscreenShot>();
    commands.trigger(ScreenshotCaptured { entity, image });
    commands.entity(entity).insert(Captured);
}

/// A readback's bytes (rows padded to 256 bytes) as an RGBA image.
pub fn rgba_image(data: &[u8], size: UVec2) -> Image {
    let row = size.x as usize * 4;
    let padded = row.div_ceil(256) * 256;
    let mut pixels = Vec::with_capacity(row * size.y as usize);
    for y in 0..size.y as usize {
        let start = y * if data.len() >= padded * size.y as usize { padded } else { row };
        pixels.extend_from_slice(data.get(start..start + row).unwrap_or(&[]));
    }
    pixels.resize(row * size.y as usize, 0);
    Image::new(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    )
}

/// An offscreen run's frames: wall time (`First` to `First`), main and
/// render world ms (`frame_metrics`), for the summary at the end.
#[derive(Resource)]
struct RunTimes {
    size: UVec2,
    last: Option<Instant>,
    wall: Vec<f64>,
    main: Vec<f64>,
    render: Vec<f64>,
}

fn record_frame(mut times: ResMut<RunTimes>, metrics: Option<Res<super::frame_metrics::FrameMetrics>>) {
    let now = Instant::now();
    if let Some(last) = times.last.replace(now) {
        times.wall.push((now - last).as_secs_f64() * 1e3);
        if let Some(m) = metrics {
            times.main.push(m.last.main_ms);
            if let Some(r) = &m.last_render {
                times.render.push(r.ms);
            }
        }
    }
}

/// The run's frame times when it ends (the app dropped: what ends a run,
/// a screenshot's arrival, can come after the frame's last schedule).
impl Drop for RunTimes {
    fn drop(&mut self) {
        let (size, times) = (self.size, &*self);
        eprintln!("offscreen {}x{}: {} frames (ms: p50 p90 p99 max)", size.x, size.y, times.wall.len());
        for (name, values) in [("frame", &times.wall), ("main", &times.main), ("render", &times.render)] {
            if let Some(p) = chimera_core::metrics::Percentiles::of(values) {
                eprintln!("  {name:<7}{:>8.2}{:>8.2}{:>8.2}{:>8.2}", p.p50, p.p90, p.p99, p.max);
            }
        }
    }
}

/// `--dump <dir>`: every `every`th frame saved there as a numbered PNG.
#[derive(Resource)]
pub struct FrameDumps {
    dir: PathBuf,
    every: u32,
    frame: u32,
    saved: u32,
    /// Real time of each dump asked for, for the clip's rate.
    times: Vec<f64>,
    /// `frames.tsv`: each frame's sample position in the recorded sound.
    positions: Option<std::io::BufWriter<std::fs::File>>,
    writers: Arc<Mutex<Vec<std::thread::JoinHandle<()>>>>,
}

impl FrameDumps {
    fn new(dir: PathBuf, every: u32) -> Self {
        Self {
            dir,
            every: every.max(1),
            frame: 0,
            saved: 0,
            times: Vec::new(),
            positions: None,
            writers: Arc::default(),
        }
    }
}

fn dump_frame(
    mut commands: Commands,
    mut dumps: ResMut<FrameDumps>,
    time: Res<Time<Real>>,
    audio: Option<Res<chimera_present::audio::record::AudioRecording>>,
) {
    dumps.frame += 1;
    if dumps.frame % dumps.every != 0 {
        return;
    }
    if dumps.saved == 0
        && let Err(e) = std::fs::create_dir_all(&dumps.dir)
    {
        error!("--dump {}: {e}", dumps.dir.display());
    }
    let name = format!("frame_{:06}.png", dumps.saved);
    if let Some(audio) = audio {
        write_position(&mut dumps, &name, audio.position());
    }
    let path = dumps.dir.join(name);
    dumps.saved += 1;
    dumps.times.push(time.elapsed_secs_f64());
    let writers = dumps.writers.clone();
    commands
        .spawn(Screenshot::primary_window())
        .observe(move |shot: On<ScreenshotCaptured>| {
            // Encoding takes tens of ms: not on the frame.
            let (image, path) = (shot.image.clone(), path.clone());
            let handle = std::thread::spawn(move || match image.try_into_dynamic() {
                Ok(img) => {
                    if let Err(e) = img.to_rgb8().save(&path) {
                        eprintln!("--dump {}: {e}", path.display());
                    }
                }
                Err(e) => eprintln!("--dump {}: {e}", path.display()),
            });
            if let Ok(mut w) = writers.lock() {
                w.push(handle);
            }
        });
}

/// A line of `frames.tsv`: the frame's file and its sample position in
/// `audio.wav` (the header names the rate).
fn write_position(dumps: &mut FrameDumps, name: &str, position: u64) {
    use std::io::Write;
    if dumps.positions.is_none() {
        let path = dumps.dir.join("frames.tsv");
        match std::fs::File::create(&path) {
            Ok(f) => {
                let mut f = std::io::BufWriter::new(f);
                let rate = chimera_present::audio::record::RATE;
                let _ = writeln!(f, "# frame\tsample in audio.wav, {rate} a second");
                dumps.positions = Some(f);
            }
            Err(e) => error!("--dump {}: {e}", path.display()),
        }
    }
    if let Some(f) = &mut dumps.positions
        && writeln!(f, "{name}\t{position}").and_then(|()| f.flush()).is_err()
    {
        dumps.positions = None;
    }
}

/// At the end (the app dropped): wait for the PNGs, and with every frame
/// dumped and ffmpeg installed, put them together as `clip.mp4` at their
/// real rate.
impl Drop for FrameDumps {
    fn drop(&mut self) {
        finish_dumps(self);
    }
}

fn finish_dumps(dumps: &FrameDumps) {
    let writers = dumps.writers.lock().map(|mut w| std::mem::take(&mut *w)).unwrap_or_default();
    let written = writers.len();
    for w in writers {
        let _ = w.join();
    }
    eprintln!("--dump: {written} frames in {}", dumps.dir.display());
    let (Some(first), Some(last)) = (dumps.times.first(), dumps.times.last()) else { return };
    if dumps.every != 1 || written < 2 || last <= first {
        return;
    }
    let fps = (dumps.times.len() - 1) as f64 / (last - first);
    let ffmpeg = std::process::Command::new("ffmpeg").arg("-version").output();
    if !ffmpeg.is_ok_and(|o| o.status.success()) {
        eprintln!("--dump: no ffmpeg for a clip (frames at {fps:.1} a second)");
        return;
    }
    let clip = dumps.dir.join("clip.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-framerate", &format!("{fps:.3}"), "-i"])
        .arg(dumps.dir.join("frame_%06d.png"))
        .args(["-frames:v", &written.to_string(), "-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(&clip)
        .status();
    match status {
        Ok(s) if s.success() => eprintln!("--dump: {} ({fps:.1} frames a second)", clip.display()),
        other => eprintln!("--dump: ffmpeg failed: {other:?}"),
    }
}

//! A frame's cost in play, offscreen (a GPU, no window or display:
//! docs/performance.md, "Performance pass" and "Work counts"): Ultimate
//! Gun Game's simulation with Bevy's whole renderer, bots playing a seeded
//! ladder, a camera at a bot's eyes drawing into an image, frames of
//! 1/60 s. Each frame's main schedule, the render world's extraction and
//! its own update are counted apart by work counts (`metrics::work`: the
//! systems' user-space instructions on this thread; every schedule
//! single-threaded, no pipelined rendering, the
//! scenario repeatable: the same build counts the same on every run), with
//! the costliest systems of each, the meshes drawn and the bytes of meshes
//! and textures uploaded a frame (deterministic counts too). GPU time from
//! timestamp queries (Vulkan, DX12) and wall time are printed apart: they
//! vary with the machine, never judge by them. No HUD and no local
//! player: the simulation, the bodies' and the map's drawing, the render
//! world. It also prints how many prop materials and meshes changed a
//! frame (each one prepared or allocated again in the render world). The
//! thing to run under `perf` for what a frame's time goes to where no
//! window can open:
//!
//! ```text
//! cargo build --profile profiling -p ultimate_gun_game --example frame_bench
//! target/profiling/examples/frame_bench [<map>] [--bots <n>] [--frames <n>] [--size <w>x<h>]
//! ```
//!
//! (`cs_source:de_dust2`, 6 bots, 600 frames, 1280x720 without options;
//! needs the installs. Linux: instructions need hardware counters,
//! `perf_event_paranoid` <= 2.)

use bevy::{
    app::PluginsState,
    camera::RenderTarget,
    prelude::*,
    render::{RenderPlugin, pipelined_rendering::PipelinedRenderingPlugin, render_resource::TextureFormat},
    time::TimeUpdateStrategy,
    window::ExitCondition,
    winit::WinitPlugin,
};
use chimera_client::client::console;
use chimera_core::{
    core::{Health, Intent, SpawnPoint},
    metrics::work::{self, WorkCounts},
};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim, ui::visuals::CaVisualsPlugin};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

#[derive(Component)]
struct Eye;

/// Prop materials and meshes changed (added or modified) since the count
/// was last taken.
#[derive(Resource, Default)]
struct Churn {
    materials: u64,
    meshes: u64,
}

fn count_churn(
    mut materials: MessageReader<AssetEvent<chimera_present::render::prop_material::PropMaterial>>,
    mut meshes: MessageReader<AssetEvent<Mesh>>,
    mut churn: ResMut<Churn>,
) {
    fn changed<A: Asset>(e: &&AssetEvent<A>) -> bool {
        matches!(e, AssetEvent::Added { .. } | AssetEvent::Modified { .. })
    }
    churn.materials += materials.read().filter(changed).count() as u64;
    churn.meshes += meshes.read().filter(changed).count() as u64;
}

/// Bytes of meshes and textures the render world uploaded this frame
/// (Bevy's `RenderAssetBytesPerFrameLimiter`, with no limit set: counting
/// only), read before its cleanup resets it.
static UPLOADED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn read_uploaded(limiter: Res<bevy::render::render_asset::RenderAssetBytesPerFrameLimiter>) {
    let n = limiter.bytes_written.load(std::sync::atomic::Ordering::Relaxed) as u64;
    UPLOADED.store(n, std::sync::atomic::Ordering::Relaxed);
}

/// Meshes some view draws (visible this frame).
fn meshes_drawn(world: &mut World) -> u64 {
    world.query::<(&Mesh3d, &ViewVisibility)>().iter(world).filter(|(_, v)| v.get()).count() as u64
}

fn app(size: (u32, u32)) -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            // One compute thread (as `harness::Sim`'s), held while counting
            // (`work::set_enabled`): parallel queries inline, scopes' tasks
            // on this thread, the counts the same on every run.
            .set(bevy::app::TaskPoolPlugin {
                task_pool_options: chimera_world::harness::sim_task_pools(),
            })
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                close_when_requested: false,
                ..default()
            })
            .set(RenderPlugin::default())
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>(),
    )
    .add_plugins(chimera_world::physics_plugins())
    .add_plugins(CaSim)
    .add_plugins((
        CaVisualsPlugin,
        chimera_present::MapPlugin::empty(),
        PartyPartsPlugin,
        UltimateGunGamePlugin { seed: 36336, rungs: 13, ..Default::default() },
    ))
    .add_plugins(chimera_present::PresentationPlugins)
    // GPU time per pass (timestamps; a timing, printed apart).
    .add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin)
    .insert_resource(bevy::render::render_asset::RenderAssetBytesPerFrame::new(usize::MAX))
    .insert_resource(chimera_core::tasks::WaitForTasks)
    .insert_resource(party_kit::client_hooks())
    .init_resource::<Churn>()
    .add_systems(Update, console::finish_map_load)
    .add_systems(Last, count_churn);
    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.sub_app_mut(bevy::render::RenderApp)
        .add_systems(bevy::render::Render, read_uploaded.in_set(bevy::render::RenderSystems::Render));
    app.finish();
    app.cleanup();
    chimera_world::harness::single_threaded_schedules(&mut app);
    // Every system counted (before any schedule ran), and Avian's tree
    // optimizing in the tick's order, not on a task beside it.
    work::count_systems(app.world_mut());
    work::count_systems(app.sub_app_mut(bevy::render::RenderApp).world_mut());
    if let Some(mut o) = app.world_mut().get_resource_mut::<avian3d::collider_tree::ColliderTreeOptimization>() {
        o.use_compute_task = false;
    }
    // The render world's too: its systems on this thread, all counted.
    let render = app.sub_app_mut(bevy::render::RenderApp).world_mut();
    for (_, schedule) in render.resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    }
    chimera_core::core::set_tick_length(app.world_mut(), std::time::Duration::from_secs_f64(TICK_INTERVAL));
    let w = app.world_mut();
    let image = Image::new_target_texture(size.0, size.1, TextureFormat::Rgba8UnormSrgb, None);
    let image = w.resource_mut::<Assets<Image>>().add(image);
    w.spawn((
        Eye,
        Camera3d::default(),
        Camera::default(),
        RenderTarget::Image(image.into()),
        Transform::default(),
    ));
    app
}

/// One frame, counted in parts (work counts): the main schedule, the
/// render world's extraction and its own update.
fn frame(app: &mut App) -> [u64; 3] {
    let n = || work::now().unwrap_or(0);
    let bevy::app::SubApps { main, sub_apps } = app.sub_apps_mut();
    let a = n();
    main.run_default_schedule();
    let b = n();
    for sub in sub_apps.values_mut() {
        sub.extract(main.world_mut());
    }
    let d = n();
    for sub in sub_apps.values_mut() {
        sub.update();
    }
    let e = n();
    main.world_mut().clear_trackers();
    [b.saturating_sub(a), d.saturating_sub(b), e.saturating_sub(d)]
}

/// The camera at the first living character's eyes, looking where it
/// looks (the map's first spawn while nobody lives).
fn place_eye(app: &mut App) {
    let w = app.world_mut();
    let at = w
        .query::<(&Transform, &Intent, &Health)>()
        .iter(w)
        .find(|(_, _, h)| h.current > 0.0)
        .map(|(t, i, _)| Transform::from_translation(t.translation + Vec3::Y * 0.6).with_rotation(i.yaw_rotation()))
        .or_else(|| {
            let spawn = w.query_filtered::<&Transform, With<SpawnPoint>>().iter(w).next().copied()?;
            Some(Transform::from_translation(spawn.translation + Vec3::Y * 1.6).with_rotation(spawn.rotation))
        });
    let Some(at) = at else { return };
    for mut t in w.query_filtered::<&mut Transform, With<Eye>>().iter_mut(w) {
        *t = at;
    }
}

fn main() {
    let mut map = "cs_source:de_dust2".to_string();
    let (mut bots, mut frames, mut size) = (6u32, 600usize, (1280u32, 720u32));
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{a}: missing value"));
        match a.as_str() {
            "--bots" => bots = value().parse().expect("--bots <n>"),
            "--frames" => frames = value().parse().expect("--frames <n>"),
            "--size" => {
                let v = value();
                let (w, h) = v.split_once('x').expect("--size <w>x<h>");
                size = (w.parse().expect("width"), h.parse().expect("height"));
            }
            _ => map = a,
        }
    }
    if let Err(e) = party_kit::load_map(&map, 0) {
        eprintln!("{map}: {e} (needs the installs)");
        return;
    }
    let mut app = app(size);
    // Every frame is 1/60 s of the game, whatever the box does (the
    // loading frames too: a wall-clock frame there would run as many
    // ticks as the load took, and the match after would play otherwise).
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(1.0 / 60.0)));
    work::set_enabled(true);
    // Shaders and pipelines first.
    for _ in 0..60 {
        frame(&mut app);
    }
    let started = std::time::Instant::now();
    console::start_map_load(app.world_mut(), &map, None);
    while console::map_loading(app.world()) {
        assert!(started.elapsed().as_secs() < 300, "{map}: not in after 300 s");
        frame(&mut app);
    }
    eprintln!("{map}: in after {:.1} s", started.elapsed().as_secs_f64());
    {
        let w = app.world_mut();
        // The match (already on when the plugin started it with the map).
        if let Err(e) = chimera_sim::minigame::start(w, MINIGAME) {
            eprintln!("the match: {e}");
        }
        let mut s = w.resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = bots;
        s.quota_set = true;
    }
    // The bots in, their navigation mesh made, the first fights.
    for _ in 0..600 {
        frame(&mut app);
        place_eye(&mut app);
    }
    let mut parts: [Vec<u64>; 3] = Default::default();
    let (mut drawn, mut uploaded) = (Vec::new(), Vec::new());
    let mut gpu = Vec::new();
    *app.world_mut().resource_mut::<Churn>() = Churn::default();
    let _ = WorkCounts::take();
    let t = std::time::Instant::now();
    for _ in 0..frames {
        let f = frame(&mut app);
        for k in 0..3 {
            parts[k].push(f[k]);
        }
        drawn.push(meshes_drawn(app.world_mut()));
        uploaded.push(UPLOADED.swap(0, std::sync::atomic::Ordering::Relaxed));
        if let Some(g) = app
            .world()
            .get_resource::<bevy::diagnostic::DiagnosticsStore>()
            .and_then(chimera_client::client::frame_metrics::gpu_latest_ms)
        {
            gpu.push(g);
        }
        place_eye(&mut app);
    }
    let wall = t.elapsed().as_secs_f64() * 1e3 / frames.max(1) as f64;
    let systems = WorkCounts::take().per(frames as u64);
    let w = app.world_mut();
    let churn = w.resource::<Churn>();
    let per = |n: u64| n as f64 / frames.max(1) as f64;
    let (materials, meshes) = (per(churn.materials), per(churn.meshes));
    let characters = w.query::<&Health>().iter(w).count();
    let entities = w.entities().len();
    println!("{map}, {characters} characters, {entities} entities, {frames} frames at {}x{}", size.0, size.1);
    println!("changed a frame: {materials:.1} prop materials, {meshes:.1} meshes");
    let mean = |v: &[u64]| v.iter().sum::<u64>() as f64 / v.len().max(1) as f64;
    println!(
        "a frame: {:.0} meshes drawn, {:.1} KB of meshes and textures uploaded (mean; deterministic counts)",
        mean(&drawn),
        mean(&uploaded) / 1024.0
    );
    gpu.sort_by(f64::total_cmp);
    let gpu_line = gpu.get(gpu.len() / 2).map_or("no GPU timestamps".to_string(), |g| format!("GPU p50 {g:.2} ms"));
    println!("timings (vary with the machine, never a gate): wall {wall:.2} ms a frame with counting; {gpu_line}");
    if work::now().is_none() {
        println!("no hardware counters here ({}): instructions not counted", chimera_core::metrics::hw_status());
        return;
    }
    for (name, mut v) in ["main schedule", "extraction", "render world"].into_iter().zip(parts) {
        v.sort_unstable();
        let at = |q: f64| v[((v.len() - 1) as f64 * q) as usize] as f64 / 1e6;
        let total = v.iter().sum::<u64>() as f64 / v.len().max(1) as f64 / 1e6;
        println!(
            "{name:14} mean {total:7.2}M  p50 {:7.2}M  p90 {:7.2}M  p99 {:7.2}M  max {:7.2}M instructions a frame",
            at(0.5),
            at(0.9),
            at(0.99),
            at(1.0)
        );
    }
    println!("costliest systems, instructions a frame (main world, extraction, render world):");
    for (label, ins, runs) in systems.top(30) {
        println!("{:10.3}M {runs:4}  {label}", ins as f64 / 1e6);
    }
}

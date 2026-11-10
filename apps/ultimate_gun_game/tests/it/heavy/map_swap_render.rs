//! What a map change costs the render world, offscreen (a GPU, no window
//! or display: docs/performance.md, "A map's swap"): the gun game's
//! simulation with Bevy's whole renderer (no window, no pipelined rendering:
//! each frame's main schedule, the render world's extraction and its own
//! update are counted apart, as the main thread and the render thread split
//! them in the app; every schedule single-threaded here, so all on this
//! thread), a camera drawing into an image from the map's first
//! spawn, the shell's `map` flow over the maps of `map_swap_frames`.
//! Prints each map's worst frame of each part from the map command to
//! entering the game; `CHIMERA_SPAWN_AT_ONCE=1` spawns each map in one
//! frame instead (the A/B). A tool, not a check: runs only with
//! `CHIMERA_RENDER_BENCH=1` (and the installs, and a GPU).
//!
//! `CHIMERA_RENDER_BENCH=1 cargo test -p ultimate_gun_game --features dev --test it heavy::map_swap_render -- --nocapture`

use bevy::{
    app::PluginsState,
    camera::RenderTarget,
    prelude::*,
    render::{RenderPlugin, pipelined_rendering::PipelinedRenderingPlugin, render_resource::TextureFormat},
    window::ExitCondition,
    winit::WinitPlugin,
};
use chimera_client::client::console::{self, MapLoadStage};
use chimera_core::metrics::ThreadCounters;
use chimera_core::core::SpawnPoint;
use combat_arms::{game::TICK_INTERVAL, harness::CaSim, ui::visuals::CaVisualsPlugin};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::UltimateGunGamePlugin;

const MAPS: [&str; 4] = [
    "cs_source:de_dust2",
    "cs_source:cs_office",
    "hl2dm:dm_lockdown",
    "combat_arms:warehouse",
];

#[derive(Component)]
struct Eye;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
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
    .insert_resource(party_kit::client_hooks())
    .add_systems(Update, console::finish_map_load);
    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();
    chimera_world::harness::single_threaded_schedules(&mut app);
    // The render world's too: its systems on this thread, all counted.
    let render = app.sub_app_mut(bevy::render::RenderApp).world_mut();
    for (_, schedule) in render.resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    }
    chimera_core::core::set_tick_length(app.world_mut(), std::time::Duration::from_secs_f64(TICK_INTERVAL));
    let w = app.world_mut();
    let image = Image::new_target_texture(1280, 720, TextureFormat::Rgba8UnormSrgb, None);
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

/// One frame, counted in parts: the main schedule, the render world's
/// extraction and its own update (this thread's instructions: every
/// schedule runs single-threaded here, the render world's systems too).
fn frame(app: &mut App, c: &mut ThreadCounters) -> [u64; 3] {
    let n = |c: &mut ThreadCounters| c.read().hw.map_or(0, |h| h.instructions);
    let bevy::app::SubApps { main, sub_apps } = app.sub_apps_mut();
    let a = n(c);
    main.run_default_schedule();
    let b = n(c);
    for sub in sub_apps.values_mut() {
        sub.extract(main.world_mut());
    }
    let d = n(c);
    for sub in sub_apps.values_mut() {
        sub.update();
    }
    let e = n(c);
    main.world_mut().clear_trackers();
    [b.saturating_sub(a), d.saturating_sub(b), e.saturating_sub(d)]
}

/// The camera at the map's first spawn, at eye height.
fn place_eye(app: &mut App) {
    let w = app.world_mut();
    let Some(at) = w.query_filtered::<&Transform, With<SpawnPoint>>().iter(w).next().copied() else { return };
    for mut t in w.query_filtered::<&mut Transform, With<Eye>>().iter_mut(w) {
        *t = Transform::from_translation(at.translation + Vec3::Y * 1.6).with_rotation(at.rotation);
    }
}

#[test]
fn map_swap_render() {
    if chimera_core::env::var("CHIMERA_RENDER_BENCH").is_err() {
        eprintln!("a tool: CHIMERA_RENDER_BENCH=1 runs it");
        return;
    }
    if !MAPS.iter().all(|id| party_kit::load_map(id, 0).is_ok()) {
        eprintln!("skipping: needs the installs");
        return;
    }
    let mut app = app();
    let mut c = ThreadCounters::current();
    // Shaders and pipelines first.
    for _ in 0..60 {
        frame(&mut app, &mut c);
    }
    for id in MAPS {
        let started = std::time::Instant::now();
        console::start_map_load(app.world_mut(), id, None);
        let mut worst = [[0u64; 3]; 3];
        let mut frames = [0usize; 3];
        let mut ms = [0f64; 3];
        while console::map_loading(app.world()) {
            assert!(started.elapsed().as_secs() < 180, "{id}: not in after 180 s");
            let before = console::map_load_stage(app.world());
            let t = std::time::Instant::now();
            let parts = frame(&mut app, &mut c);
            let after = console::map_load_stage(app.world());
            let phase = match (before, after) {
                (MapLoadStage::Loading, MapLoadStage::Loading) => 0,
                (MapLoadStage::Loading | MapLoadStage::Spawning, _) => 1,
                _ => 2,
            };
            frames[phase] += 1;
            ms[phase] += t.elapsed().as_secs_f64() * 1e3;
            for k in 0..3 {
                worst[phase][k] = worst[phase][k].max(parts[k]);
            }
            place_eye(&mut app);
        }
        let m = |x: u64| x as f64 / 1e6;
        eprintln!(
            "{id:22} {:5.2} s; spawning {:2} frames {:6.1} ms: main {:6.1} extract {:6.1} render {:6.1} M; settling {:2} frames {:6.1} ms: main {:6.1} extract {:6.1} render {:6.1} M; loading worst main {:5.1} render {:5.1} M",
            started.elapsed().as_secs_f64(),
            frames[1],
            ms[1],
            m(worst[1][0]),
            m(worst[1][1]),
            m(worst[1][2]),
            frames[2],
            ms[2],
            m(worst[2][0]),
            m(worst[2][1]),
            m(worst[2][2]),
            m(worst[0][0]),
            m(worst[0][2]),
        );
        // In the game a second.
        for _ in 0..60 {
            frame(&mut app, &mut c);
        }
    }
}

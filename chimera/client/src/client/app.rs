//! What every game binary does with its app once its plugins are in,
//! shared by the apps (apps/cs_source, apps/combat_arms): `run` puts
//! every schedule of the main and render worlds but the extract schedule
//! on single-threaded executors (`single_threaded_schedules`;
//! `CHIMERA_EXECUTOR=multi` keeps Bevy's default), writes the systems' order instead of running when
//! `CHIMERA_SYSTEM_DUMP=<file>` is set (`dump_systems`,
//! docs/OBSERVABILITY.md), and runs the app.
//!
//! `default_plugins` is Bevy's `DefaultPlugins` as every game's client
//! takes them: the window from the command line (`window`: `--window`,
//! `--scale`), the log into the in-game console too, and with
//! `--offscreen` no window system at all (no winit: Bevy's schedule
//! runner drives the frames; `offscreen` draws the window into an image).
//!
//! The build's version stamp, the other piece every app shares, is a
//! build script's: chimera/core/build_stamp.rs, which each app's build.rs
//! includes.

use bevy::{ecs::schedule::SingleThreadedExecutor, prelude::*};

/// Bevy's `DefaultPlugins` as a game's client takes them: the primary
/// window titled `title` (`window`), warnings and errors in the in-game
/// console too, and with `--offscreen` no winit (nothing opens a window or
/// needs a display) and Bevy's schedule runner looping frames as fast as
/// they come (`fps_max` still spaces them); the window entity stays, so
/// everything that sizes itself by it works, and `offscreen` gives the
/// renderer an image in place of its surface.
pub fn default_plugins(args: &super::Args, title: &str) -> bevy::app::PluginGroupBuilder {
    let group = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(window(args, title)),
            ..default()
        })
        .set(bevy::log::LogPlugin {
            custom_layer: super::console::log_layer,
            ..default()
        });
    match args.offscreen {
        Some(_) => group
            .disable::<bevy::winit::WinitPlugin>()
            .add(bevy::app::ScheduleRunnerPlugin::run_loop(std::time::Duration::ZERO)),
        None => group,
    }
}

/// The primary window from the command line: `--window` (physical pixels
/// whatever the display's scaling, a fixed size: tiling window managers
/// float such windows instead of resizing them), `--scale` (as a display
/// scaled that much), `--offscreen` (that size at scale 1, or `--scale`).
pub fn window(args: &super::Args, title: &str) -> Window {
    let fixed = args.offscreen.or(args.window);
    Window {
        title: title.into(),
        resolution: match (fixed, args.scale) {
            (Some(s), scale) => {
                bevy::window::WindowResolution::new(s.x, s.y).with_scale_factor_override(scale.unwrap_or(1.0))
            }
            (None, Some(scale)) => bevy::window::WindowResolution::default().with_scale_factor_override(scale),
            (None, None) => default(),
        },
        resizable: fixed.is_none(),
        resize_constraints: fixed
            .map(|s| {
                // In logical pixels.
                let s = s.as_vec2() / args.scale.unwrap_or(1.0);
                bevy::window::WindowResizeConstraints {
                    min_width: s.x,
                    min_height: s.y,
                    max_width: s.x,
                    max_height: s.y,
                }
            })
            .unwrap_or_default(),
        ..default()
    }
}

/// Run a game's app as its binary does: the executors
/// (`single_threaded_schedules`, unless `CHIMERA_EXECUTOR=multi`), then
/// with `CHIMERA_SYSTEM_DUMP=<file>` the systems' order written there and
/// the process ended (`dump_systems`), else the app run.
pub fn run(mut app: App) -> AppExit {
    if chimera_core::env::var("CHIMERA_EXECUTOR").as_deref() != Ok("multi") {
        single_threaded_schedules(&mut app);
    }
    if let Some(path) = chimera_core::env::var_os("CHIMERA_SYSTEM_DUMP") {
        dump_systems(&mut app, std::path::Path::new(&path));
        // Without the event loop having run, dropping the app crashes.
        std::process::exit(0);
    }
    app.run()
}

/// `CHIMERA_SYSTEM_DUMP=<file>`: the app built as it would run (plugins
/// finished), every schedule's systems in the order they run, the main
/// world's then the render world's (`harness::system_order`), written to
/// the file instead of running; for refactors that move the client's
/// systems between plugins or crates, and for what an added system does
/// to the order of others (docs/OBSERVABILITY.md). `CHIMERA_AMBIGUITY=1`
/// also logs the render world's `Render` schedule's ambiguous pairs.
pub fn dump_systems(app: &mut App, path: &std::path::Path) {
    use bevy::app::PluginsState;
    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    // The render world before `cleanup`: pipelined rendering's cleanup
    // moves it to its own thread, out of the app. Its extract systems
    // need a main world to initialize against: an empty one.
    let render = app.get_sub_app_mut(bevy::render::RenderApp).map(|sub| {
        sub.world_mut().init_resource::<bevy::render::MainWorld>();
        // `CHIMERA_AMBIGUITY=1`: log the `Render` schedule's ambiguous
        // system pairs (systems with conflicting access and no order;
        // the single-threaded executor runs them in whatever order the
        // graph sorts to, which other systems' constraints can change).
        if chimera_core::env::var("CHIMERA_AMBIGUITY").as_deref() == Ok("1") {
            sub.edit_schedule(bevy::render::Render, |s| {
                let mut settings = s.get_build_settings();
                settings.ambiguity_detection = bevy::ecs::schedule::LogLevel::Warn;
                s.set_build_settings(settings);
            });
        }
        let text = chimera_world::harness::system_order(sub.world_mut());
        sub.world_mut().remove_resource::<bevy::render::MainWorld>();
        text
    });
    app.cleanup();
    let mut text = chimera_world::harness::system_order(app.world_mut());
    if let Some(render) = render {
        text += "render world:\n";
        text += &render;
    }
    match std::fs::write(path, text) {
        Ok(()) => eprintln!("systems in order written to {}", path.display()),
        Err(e) => eprintln!("error: CHIMERA_SYSTEM_DUMP {}: {e}", path.display()),
    }
}

/// Run every schedule (but the render world's extract) on one thread each
/// instead of Bevy's multi-threaded executor: the main world's (chimera_world's
/// `harness::single_threaded_schedules`, which the dedicated server uses
/// alone) and the render world's.
///
/// A frame runs about 1500 systems (main and render world), nearly all of
/// them tiny: handing each to a worker thread and waking the next cost more
/// than the systems did, and much more on a busy machine. With one thread
/// per schedule the main and render worlds took a quarter and a sixth less
/// time on de_dust2 (and bot matches) here; systems that iterate many
/// entities still spread over the task pool themselves (transform
/// propagation, visibility), and the main and render worlds still run side
/// by side (pipelined rendering). docs/performance.md, "Frame time pass".
/// `CHIMERA_EXECUTOR=multi` keeps Bevy's default for comparisons.
pub fn single_threaded_schedules(app: &mut App) {
    chimera_world::harness::single_threaded_schedules(app);
    // On macOS the render world keeps Bevy's executor: it runs on the
    // pipelined render thread, and the window surface must be created on
    // the main thread (AppKit's NSView), which only the multi-threaded
    // executor hands its main-thread systems to; a single-threaded one runs
    // them where it is and panics ("can only access NSView on the main
    // thread").
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        return;
    }
    // Every render-world schedule: `Render` and the graph's, and the
    // ones Bevy left on its multi-threaded executor: the sub-app's own
    // update schedule (one system that runs `Render`; a task handed to a
    // worker and waited for every frame) and its `First`. Not the extract
    // schedule: it runs while the main world waits, and on one thread its
    // ~150 systems put 0.6 ms more a frame on the main thread
    // (docs/performance.md, "Party frame-rate pass", third pass).
    if let Some(sub) = app.get_sub_app_mut(bevy::render::RenderApp) {
        chimera_world::harness::set_executors(sub.world_mut(), SingleThreadedExecutor::new);
        sub.edit_schedule(bevy::render::ExtractSchedule, |s| {
            s.set_executor(bevy::ecs::schedule::MultiThreadedExecutor::new());
            // Its commands are applied on the render thread, later
            // (`ExtractCommands`), as Bevy sets it up.
            s.set_apply_final_deferred(false);
        });
    }
}

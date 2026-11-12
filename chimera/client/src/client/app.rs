//! A game's client app, once for every game (docs/plans/active/
//! architecture-pass.md, step 7), as `chimera_net::dedicated` is its
//! dedicated server: an app crate says what is its own as data
//! (`GameApp`: the game it plays, its window title, its maps, its
//! simulation, its HUD and front end, and the optional `GameAppHooks`)
//! and its binary is `run(&APP)`. The shape is here: the command line
//! (`Args`; `--help` the app's own text, `GameApp::about`, then the
//! shell's options written for its game, `client::usage` from its
//! `GameInfo`), what the machine lacks, the start map, then in order
//! the map's plugin, the install's asset sources, the window and the log
//! (`default_plugins`), Avian (`chimera_world::physics_plugins`), the
//! game's simulation (`GameApp::sim`), the presentation, the build's
//! version, the network, the client shell with the game's HUD among its
//! plugins (`GameApp::hud`), the game's front end (`GameApp::front_end`)
//! and the app's last word (`GameAppHooks::launch`: a main menu, a
//! launch's seed). Every client app is built on it: CS:S's, HL2:DM's,
//! Combat Arms', the mashup demo's and the party apps' (apps/*).
//!
//! What only some apps have, `GameAppHooks` (all off in
//! `GameAppHooks::NONE`): the command line as the app plays it (`args`:
//! a default map, minigame or bot count), what the machine lacks before
//! anything loads (`requirements`: told on stderr; a lack that stops the
//! app ends it, on Windows after Enter), a map to start on without
//! `--map` (`default_map`, else the greybox), a start of its own (`start`:
//! the party's main menu over a background map), the start map at the
//! console's `mat_hdr_level` (`hdr_start`), what the app's map loads
//! should know of it before it builds (`load_context`: the start map
//! loads in that context, which is the app's `LoadContext` from then on,
//! as the dedicated server's), `--debug-view` and
//! `--lightmap-only` on it (`debug_views`), its dedicated server's binary
//! for the usage text (`server`), what it adds last (`launch`) and every
//! game it plays (`games`: the startup fetch's, `startup`).
//! `--offscreen` and the window are `default_plugins`'s, for every app,
//! and with `--offscreen` and `--dump` the sound recorded into the dump
//! directory's `audio.wav` instead of played (`audio::record`).
//!
//! `run_app` is what every game binary does with its app once its
//! plugins are in: it puts
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

use bevy::{app::PluginGroupBuilder, ecs::schedule::SingleThreadedExecutor, prelude::*};
use chimera_present::assets::{InstallSource, InstallSourcesPlugin};

use super::{AppHooks, Args, ClientPlugins};
use crate::{
    game_registry::GameInfo,
    map::{LoadContext, MapData, MapDebugView, MapLoadOptions},
};

/// A game's client app (module doc): the game's own, as data. Its app
/// crate holds one in a `static`; the binary is `run(&APP)`.
pub struct GameApp {
    /// The binary's name (the usage text).
    pub bin: &'static str,
    /// The game it plays (the usage text: its maps' id, its name, its
    /// movement cvars).
    pub info: &'static GameInfo,
    /// The window's title.
    pub title: &'static str,
    /// What `--help` says before the shell's options (empty: nothing).
    pub about: fn() -> String,
    /// The build's stamp (`core::BuildVersion`: `version`, bug reports):
    /// the app crate's `env!("CHIMERA_VERSION")`, from its build.rs.
    pub version: &'static str,
    /// The build's commit (`env!("CHIMERA_GIT")`; `AppHooks::build`).
    pub build: &'static str,
    /// How its maps load (`AppHooks`'s maps).
    pub maps: AppMaps,
    /// The installs read as asset sources (`<id>://`), before
    /// `AssetPlugin` (empty: none).
    pub installs: fn() -> Vec<InstallSource>,
    /// The game's simulation, its gameplay and what it sets from the
    /// command line, after Avian and before the presentation.
    pub sim: fn(&mut App, &Args),
    /// Its HUD among the shell's plugins (`ClientPlugins`; `no_hud`: the
    /// shell's own).
    pub hud: fn(PluginGroupBuilder) -> PluginGroupBuilder,
    /// The rest of its front end (menus, looks, the field of view), after
    /// the shell.
    pub front_end: fn(&mut App, &Args),
    /// What only some apps have (`GameAppHooks::NONE`: nothing).
    pub hooks: GameAppHooks,
}

/// How an app's maps load (`AppHooks`: the greybox's hooks are
/// `chimera_world::greybox`'s).
#[derive(Clone, Copy)]
pub struct AppMaps {
    /// Map `id` (`game:name`) with `options`.
    pub load: fn(&str, &MapLoadOptions) -> Result<MapData, String>,
    /// Map `id` from a copy of its file (a server's map downloaded).
    pub load_file: fn(&str, &std::path::Path, &MapLoadOptions) -> Result<MapData, String>,
    /// The bytes of map `id`'s file, for the network's map offer.
    pub file_bytes: fn(&str) -> Option<Vec<u8>>,
}

impl AppMaps {
    /// Maps by id only (`load`): none from a file, no file to offer.
    pub const fn by_id(load: fn(&str, &MapLoadOptions) -> Result<MapData, String>) -> Self {
        Self {
            load,
            load_file: |id, _, _| Err(format!("{id}: this app loads maps by id only, not from a file")),
            file_bytes: |_| None,
        }
    }
}

/// An app's optional parts (module doc). An app sets the ones it has over
/// `NONE`: `GameAppHooks { default_map: Some(..), ..GameAppHooks::NONE }`.
#[derive(Clone, Copy)]
pub struct GameAppHooks {
    /// The command line as the app plays it (defaults it adds).
    pub args: Option<fn(Args) -> Args>,
    /// What this machine lacks: the text told on stderr before anything
    /// loads (empty: nothing), and whether it stops the app.
    pub requirements: Option<fn() -> (String, bool)>,
    /// The map without `--map` (None: the greybox).
    pub default_map: Option<&'static str>,
    /// A start map of the app's own (None from it: the usual one).
    pub start: Option<fn(&'static GameApp, &Args) -> Option<StartMap>>,
    /// Load the start map at the console's starting `mat_hdr_level`
    /// (`hdr::startup_level`) rather than 0.
    pub hdr_start: bool,
    /// `--debug-view` and `--lightmap-only` on the start map.
    pub debug_views: bool,
    /// Its dedicated server's binary, for the usage text.
    pub server: Option<&'static str>,
    /// What the app adds last (a main menu, a launch's seed).
    pub launch: Option<fn(&mut App, &Args)>,
    /// Every game the app plays, for the startup fetch (`startup`; None:
    /// its own, `GameApp::info`).
    pub games: Option<fn() -> Vec<&'static GameInfo>>,
    /// What the app's map loads should know of it before its plugins
    /// build (`GameApp::load_context`): the start map loads in that
    /// context, the app's from then on. Combat Arms' front end puts its
    /// install data and that its view models load on demand there.
    pub load_context: Option<fn(&LoadContext)>,
}

impl GameAppHooks {
    /// None of them.
    pub const NONE: Self = Self {
        args: None,
        requirements: None,
        default_map: None,
        start: None,
        hdr_start: false,
        debug_views: false,
        server: None,
        launch: None,
        games: None,
        load_context: None,
    };
}

/// The shell's own HUD: `GameApp::hud` of a game without one.
pub fn no_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell
}

/// No install read as assets: `GameApp::installs` of an app without.
pub fn no_installs() -> Vec<InstallSource> {
    Vec::new()
}

/// The app this process runs (`app` sets it): its descriptor, for its
/// own hooks (the title of a menu).
#[derive(Resource, Clone, Copy)]
pub struct RunningApp(pub &'static GameApp);

/// The map an app starts on.
pub enum StartMap {
    /// The greybox, and the map systems without a map (`map <name>` loads
    /// one later).
    Greybox,
    /// The map systems without a map, and no greybox.
    Empty,
    /// A map loaded (its id as asked for), in `context`
    /// (`GameApp::load_context`): the app's from then on, so what its
    /// games read for it (install data, conversions) is the app's too.
    Loaded { id: String, data: MapData, context: LoadContext },
}

impl StartMap {
    /// The map `args` start `game` on: its own (`GameAppHooks::start`),
    /// else `--map` (or `default_map`) loaded, else the greybox.
    pub fn of(game: &'static GameApp, args: &Args) -> Result<Self, String> {
        if let Some(start) = game.hooks.start.and_then(|start| start(game, args)) {
            return Ok(start);
        }
        let Some(id) = args.game_map().or(game.hooks.default_map) else {
            return Ok(Self::Greybox);
        };
        // At the HDR level the console will start with (mat_hdr_level).
        let level = if game.hooks.hdr_start { super::hdr::startup_level(&args.console) } else { 0 };
        let context = game.load_context();
        let options = MapLoadOptions { context: context.clone(), ..MapLoadOptions::hdr(level) };
        match (game.maps.load)(id, &options) {
            Ok(data) => Ok(Self::loaded_in(id, data, context)),
            Err(e) => Err(format!("--map {id}: {e}")),
        }
    }

    /// `data` loaded as `id` with no app's context, its numbers told on
    /// stderr.
    pub fn loaded(id: &str, data: MapData) -> Self {
        Self::loaded_in(id, data, LoadContext::default())
    }

    /// `data` loaded as `id` in `context`, its numbers told on stderr.
    pub fn loaded_in(id: &str, data: MapData, context: LoadContext) -> Self {
        eprintln!(
            "loaded {id}: {} triangles, {} textures, {} spawns, {} warnings",
            data.triangle_count(),
            data.textures.len(),
            data.spawns.len(),
            data.warnings.len()
        );
        for w in &data.warnings {
            eprintln!("  warning: {w}");
        }
        Self::Loaded { id: id.to_string(), data, context }
    }
}

impl GameApp {
    /// A context for the app's map loads (`map::LoadContext`) as its
    /// `GameAppHooks::load_context` readies it: the start map's.
    pub fn load_context(&self) -> LoadContext {
        let context = LoadContext::default();
        if let Some(ready) = self.hooks.load_context {
            ready(&context);
        }
        context
    }

    /// What `--help` prints: `about`, then the shell's options for this
    /// game (`client::usage`).
    pub fn usage(&self) -> String {
        let about = (self.about)();
        let shell = super::usage(self.bin, self.info, self.hooks.server);
        if about.is_empty() { shell } else { format!("{about}\n{shell}") }
    }

    /// The command line as this app plays it: `args` with the app's
    /// defaults (`GameAppHooks::args`).
    pub fn args(&self, args: Args) -> Args {
        match self.hooks.args {
            Some(defaults) => defaults(args),
            None => args,
        }
    }

    /// What this machine lacks (`GameAppHooks::requirements`): the text to
    /// tell (empty: nothing) and whether it stops the app.
    pub fn requirements(&self) -> (String, bool) {
        self.hooks.requirements.map_or((String::new(), false), |check| check())
    }

    /// What the shell is told of the app (`AppHooks`): its commit, its
    /// maps and the greybox's own hooks.
    pub fn app_hooks(&self) -> AppHooks {
        AppHooks {
            build: self.build,
            load_map: self.maps.load,
            load_map_file: self.maps.load_file,
            map_file_bytes: self.maps.file_bytes,
            unload_greybox: chimera_world::greybox::unload,
            respawn_greybox: chimera_world::greybox::respawn,
        }
    }
}

/// The game's binary: the command line (`--help`: `GameApp::usage`), the
/// startup fetch (`startup`: a progress window, then the game in a new
/// process), what the machine lacks, the start map, the app (`app`), run
/// (`run_app`).
/// Exits the process on a bad command line (2), or a lack that stops it or
/// a start map that doesn't load (1).
pub fn run(game: &'static GameApp) -> AppExit {
    let args = game.args(Args::parse_for(&game.usage()));
    super::offscreen::silence_audio_device(&args);
    // The files the games read at startup, behind a progress window, when
    // they come from the asset service and aren't here yet.
    if let Some(exit) = super::startup::run_if_needed(game, &args) {
        return exit;
    }
    // Big blocks go back to the system when freed (the first map load's
    // scratch: docs/performance.md, "A client's memory"), as the dedicated
    // server's do; from the start map on.
    crate::metrics::memory::big_blocks_mapped();
    let (lacks, fatal) = game.requirements();
    if fatal {
        exit_fatal(&lacks);
    }
    if !lacks.is_empty() {
        eprintln!("{lacks}");
    }
    let start = StartMap::of(game, &args).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    run_app(app(game, args, start))
}

/// Ends a client app that can't start: `text` on stderr, and on Windows a
/// wait for Enter first, so a double-clicked window stays open long enough
/// to read it.
pub fn exit_fatal(text: &str) -> ! {
    eprintln!("{text}");
    if cfg!(windows) {
        eprintln!("\nPress Enter to close.");
        let _ = std::io::stdin().read_line(&mut String::new());
    }
    std::process::exit(1)
}

/// The engine's parts of a client app, in the order `compose` adds them
/// (tests: a recorder in place of `add_engine`).
pub enum Engine<'a> {
    /// The start map's plugin, drawn as `view`.
    Map(StartMap, MapDebugView),
    /// The installs as asset sources.
    Installs(Vec<InstallSource>),
    /// The window and the log (`default_plugins`), titled.
    Window(&'a Args, &'static str),
    /// Avian as Chimera runs it.
    Physics,
    /// What the map's drawing needs (`chimera_present::PresentationPlugins`).
    Presentation,
    /// Network play: a listen server, or a client (`connect`).
    Network,
    /// The client shell's plugins with the game's HUD among them.
    Shell(PluginGroupBuilder),
}

/// `game`'s app on `start` for `args`, before `run_app`.
pub fn app(game: &'static GameApp, args: Args, start: StartMap) -> App {
    let mut app = App::new();
    compose(&mut app, game, args, start, &add_engine);
    app
}

/// The engine's part `part` added to `app`.
pub fn add_engine(app: &mut App, part: Engine<'_>) {
    match part {
        Engine::Map(StartMap::Loaded { data, .. }, view) => {
            app.add_plugins(chimera_present::MapPlugin {
                view,
                ..chimera_present::MapPlugin::new(data)
            });
        }
        // The greybox (behind the main menu unless the run starts
        // playing), and map systems without a map, so `map <name>` can
        // load one.
        Engine::Map(StartMap::Greybox, _) => {
            app.add_plugins((chimera_world::greybox::GreyboxMapPlugin, chimera_present::MapPlugin::empty()));
        }
        Engine::Map(StartMap::Empty, _) => {
            app.add_plugins(chimera_present::MapPlugin::empty());
        }
        Engine::Installs(sources) => {
            app.add_plugins(InstallSourcesPlugin(sources));
        }
        Engine::Window(args, title) => {
            app.add_plugins(default_plugins(args, title));
        }
        // Rendering eases between ticks with `map::interp`.
        Engine::Physics => {
            app.add_plugins(chimera_world::physics_plugins());
        }
        Engine::Presentation => {
            app.add_plugins(chimera_present::PresentationPlugins);
        }
        Engine::Network => {
            app.add_plugins(chimera_net::net::NetPlugin);
        }
        Engine::Shell(group) => {
            app.add_plugins(group);
        }
    }
}

/// `game`'s app in the order of the module doc, the engine's parts by
/// `engine`.
pub fn compose(app: &mut App, game: &'static GameApp, args: Args, start: StartMap, engine: &dyn Fn(&mut App, Engine<'_>)) {
    app.insert_resource(RunningApp(game));
    // The start map's context is the app's (`StartMap::Loaded`).
    if let StartMap::Loaded { context, .. } = &start {
        app.insert_resource(context.clone());
    }
    let view = match &start {
        StartMap::Loaded { .. } if game.hooks.debug_views => debug_view(&args).unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(2);
        }),
        _ => MapDebugView::Normal,
    };
    engine(app, Engine::Map(start, view));
    let installs = (game.installs)();
    if !installs.is_empty() {
        engine(app, Engine::Installs(installs));
    }
    engine(app, Engine::Window(&args, game.title));
    engine(app, Engine::Physics);
    (game.sim)(app, &args);
    engine(app, Engine::Presentation);
    app.insert_resource(chimera_core::core::BuildVersion(game.version));
    engine(app, Engine::Network);
    let shell = ClientPlugins {
        args: args.clone(),
        hooks: game.app_hooks(),
    };
    engine(app, Engine::Shell((game.hud)(shell.build())));
    (game.front_end)(app, &args);
    if let Some(launch) = game.hooks.launch {
        launch(app, &args);
    }
}

/// The start map's debug view: `--debug-view lighting|albedo`, else
/// `--lightmap-only`'s lighting, else none.
pub fn debug_view(args: &Args) -> Result<MapDebugView, String> {
    Ok(match (args.debug_view.as_deref(), args.lightmap_only) {
        (Some("lighting"), _) => MapDebugView::Lighting { scale: 0.25 },
        (Some("albedo"), _) => MapDebugView::Albedo,
        (Some(other), _) => return Err(format!("--debug-view {other}: expected lighting or albedo")),
        (None, true) => MapDebugView::Lighting { scale: 1.0 },
        (None, false) => MapDebugView::Normal,
    })
}


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
    let group = match args.offscreen {
        Some(_) => group
            .disable::<bevy::winit::WinitPlugin>()
            .add(bevy::app::ScheduleRunnerPlugin::run_loop(std::time::Duration::ZERO)),
        None => group,
    };
    // A recording (`--offscreen` with `--dump`): the sound into
    // `<dir>/audio.wav`, timed by the frames, instead of a device.
    match (args.offscreen, &args.dump_dir) {
        (Some(_), Some(dir)) => group
            .disable::<bevy::audio::AudioPlugin>()
            .add_after::<bevy::audio::AudioPlugin>(chimera_present::audio::record::RecordAudioPlugin {
                path: dir.join("audio.wav"),
            }),
        _ => group,
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
pub fn run_app(mut app: App) -> AppExit {
    if chimera_core::env::var("CHIMERA_EXECUTOR").as_deref() != Ok("multi") {
        single_threaded_schedules(&mut app);
    }
    if let Some(path) = chimera_core::env::var_os("CHIMERA_SYSTEM_DUMP") {
        dump_systems(&mut app, std::path::Path::new(&path));
        // Without the event loop having run, dropping the app crashes.
        std::process::exit(0);
    }
    let exit = app.run();
    // The game started again with a server's game files (`relaunch`):
    // this process only waits for it, so let the app (its GPU device, its
    // loaded files: 400-550 MB measured) go first.
    if super::relaunch::restarted() {
        drop(app);
        crate::metrics::memory::release_free();
    }
    super::relaunch::wait_for_restart().unwrap_or(exit)
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

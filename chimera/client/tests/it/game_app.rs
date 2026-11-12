//! The client app once (`client::app`, docs/plans/active/
//! architecture-pass.md step 7): a `GameApp` descriptor's hooks, each
//! called where the module doc says, with what it says; the engine's
//! parts recorded in their place (`compose` with a recorder in place of
//! `add_engine`), so no window or renderer is built.

use bevy::prelude::*;
use chimera_client::client::{
    Args,
    app::{AppMaps, Engine, GameApp, GameAppHooks, RunningApp, StartMap, compose, no_hud, no_installs},
};
use chimera_core::{core::BuildVersion, game_registry::GameInfo};
use chimera_present::assets::InstallSource;
use chimera_world::map::{MapData, MapDebugView, MapLoadOptions};

static GAME: GameInfo = GameInfo {
    id: "a_game",
    short_name: "A",
    content_dir: "a",
    tick_interval: 0.015,
    open: |_| Err(std::io::ErrorKind::NotFound.into()),
    map_cycle: |_| Vec::new(),
    map_names: |_| Vec::new(),
    map_file: |_| None,
    steam: None,
    import_map: |_| Err("no".into()),
    default_binds_cfg: "cfg/default.cfg",
    default_binds: &[],
    old_default_binds: &[],
    movement_cvars: &[("a_speed", "how fast"), ("a_jump", "how high")],
    meters_per_unit: 0.0254,
    console_namespaces: &[],
};

/// Maps `a_game:<name>` load (named `<name>`, `hdr_level` spawns, so a
/// test sees the level asked for); anything else doesn't.
fn load(id: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let name = id.strip_prefix("a_game:").ok_or_else(|| format!("{id}: not a_game's"))?;
    Ok(MapData {
        name: name.into(),
        spawns: vec![Default::default(); options.hdr_level as usize],
        ..default()
    })
}

/// What a test app did, in order.
#[derive(Resource, Default)]
struct Calls(Vec<String>);

fn call(app: &mut App, what: &str) {
    app.world_mut().get_resource_or_init::<Calls>().0.push(what.into());
}

/// The HUD hook's mark among the shell's plugins.
struct HudMark;
impl Plugin for HudMark {
    fn build(&self, _: &mut App) {}
}

static APP: GameApp = GameApp {
    bin: "a_bin",
    info: &GAME,
    title: "A Game",
    about: || "a_bin: A on Chimera.\n".into(),
    version: "v-test",
    build: "b-test",
    maps: AppMaps::by_id(load),
    installs: || vec![InstallSource::with("a_game", || None)],
    sim: |app, args| call(app, &format!("sim {}", args.console.join(";"))),
    hud: |shell| shell.add(HudMark),
    front_end: |app, _| {
        // The shell is in before the front end: its args are.
        let shell = app.world().contains_resource::<chimera_client::client::ClientArgs>();
        call(app, &format!("front_end (shell first: {shell})"));
    },
    hooks: GameAppHooks {
        args: Some(|mut args| {
            args.console.push("added".into());
            args
        }),
        requirements: Some(|| ("warning: no keys".into(), false)),
        server: Some("a_server"),
        launch: Some(|app, _| call(app, "launch")),
        ..GameAppHooks::NONE
    },
};

/// Nothing but what every app has.
static BARE: GameApp = GameApp {
    bin: "bare",
    info: &GAME,
    title: "Bare",
    about: String::new,
    version: "v",
    build: "b",
    maps: AppMaps::by_id(load),
    installs: no_installs,
    sim: |_, _| {},
    hud: no_hud,
    front_end: |_, _| {},
    hooks: GameAppHooks::NONE,
};

/// The engine's parts recorded (no window, renderer or network), the
/// shell's plugin group looked into for the HUD's mark.
fn record(app: &mut App, part: Engine<'_>) {
    let what = match part {
        Engine::Map(StartMap::Greybox, _) => "map greybox".to_string(),
        Engine::Map(StartMap::Empty, _) => "map empty".to_string(),
        Engine::Map(StartMap::Loaded { id, .. }, view) => format!("map {id} {view:?}"),
        Engine::Installs(sources) => format!("installs {}", sources.iter().map(|s| s.id).collect::<Vec<_>>().join(",")),
        Engine::Window(_, title) => format!("window {title}"),
        Engine::Physics => "physics".into(),
        Engine::Presentation => "presentation".into(),
        Engine::Network => "network".into(),
        Engine::Shell(group) => {
            let hud = group.contains::<HudMark>();
            // The shell's setup only: its args and hooks, as the real
            // group's first plugin sets them.
            app.insert_resource(chimera_client::client::ClientArgs(Args::default()));
            format!("shell (hud: {hud})")
        }
    };
    call(app, &what);
}

fn composed(game: &'static GameApp, args: Args, start: StartMap) -> App {
    let mut app = App::new();
    compose(&mut app, game, args, start, &record);
    app
}

fn args(line: &str) -> Args {
    Args::parse_from(line.split_whitespace().map(String::from)).unwrap()
}

#[test]
fn the_hooks_run_in_their_places_among_the_engines_parts() {
    let a = APP.args(args("+x 1"));
    assert_eq!(a.console, ["x 1", "added"], "the app's defaults after the command line's");
    let app = composed(&APP, a, StartMap::Greybox);
    assert_eq!(
        app.world().resource::<Calls>().0,
        [
            "map greybox",
            "installs a_game",
            "window A Game",
            "physics",
            "sim x 1;added",
            "presentation",
            "network",
            "shell (hud: true)",
            "front_end (shell first: true)",
            "launch",
        ]
    );
    assert_eq!(app.world().resource::<BuildVersion>().0, "v-test");
    assert!(std::ptr::eq(app.world().resource::<RunningApp>().0, &APP));
    assert_eq!(APP.app_hooks().build, "b-test");
    assert_eq!(APP.requirements(), ("warning: no keys".to_string(), false));
}

#[test]
fn an_app_without_hooks_has_only_the_engines_parts() {
    let app = composed(&BARE, BARE.args(Args::default()), StartMap::Empty);
    assert_eq!(
        app.world().resource::<Calls>().0,
        ["map empty", "window Bare", "physics", "presentation", "network", "shell (hud: false)"],
        "no installs plugin without installs"
    );
    assert_eq!(BARE.requirements(), (String::new(), false));
    assert!(BARE.args(Args::default()).console.is_empty());
}

#[test]
fn the_start_map_is_the_command_lines_the_apps_default_or_the_greybox() {
    let start = |game: &'static GameApp, line: &str| match StartMap::of(game, &args(line)) {
        Ok(StartMap::Greybox) => "greybox".to_string(),
        Ok(StartMap::Empty) => "empty".to_string(),
        Ok(StartMap::Loaded { id, data, .. }) => format!("{id} ({}, {} spawns)", data.name, data.spawns.len()),
        Err(e) => format!("error: {e}"),
    };
    assert_eq!(start(&BARE, ""), "greybox");
    assert_eq!(start(&BARE, "--map greybox"), "greybox");
    assert_eq!(start(&BARE, "--map a_game:one"), "a_game:one (one, 0 spawns)");
    assert_eq!(start(&BARE, "--map b:two"), "error: --map b:two: b:two: not a_game's");

    static DEFAULT_MAP: GameApp = GameApp {
        hooks: GameAppHooks {
            default_map: Some("a_game:home"),
            ..GameAppHooks::NONE
        },
        ..BARE
    };
    assert_eq!(start(&DEFAULT_MAP, ""), "a_game:home (home, 0 spawns)");
    assert_eq!(start(&DEFAULT_MAP, "--map greybox"), "a_game:home (home, 0 spawns)", "the greybox isn't a map");
    assert_eq!(start(&DEFAULT_MAP, "--map a_game:other"), "a_game:other (other, 0 spawns)");

    // At the console's starting HDR level (`hdr_start`), else at 0.
    static HDR: GameApp = GameApp {
        hooks: GameAppHooks {
            hdr_start: true,
            ..GameAppHooks::NONE
        },
        ..BARE
    };
    assert_eq!(start(&HDR, "--map a_game:lit +mat_hdr_level 2"), "a_game:lit (lit, 2 spawns)");
    assert_eq!(start(&BARE, "--map a_game:lit +mat_hdr_level 2"), "a_game:lit (lit, 0 spawns)");

    // A start of the app's own (a main menu over a background map) when
    // the command line doesn't play; the usual one when it does.
    static MENU: GameApp = GameApp {
        hooks: GameAppHooks {
            start: Some(|game, args| {
                (args.map.is_none()).then(|| match (game.maps.load)("a_game:behind_menu", &MapLoadOptions::default()) {
                    Ok(data) => StartMap::loaded("a_game:behind_menu", data),
                    Err(_) => StartMap::Empty,
                })
            }),
            ..GameAppHooks::NONE
        },
        ..BARE
    };
    assert_eq!(start(&MENU, ""), "a_game:behind_menu (behind_menu, 0 spawns)", "never the greybox");
    assert_eq!(start(&MENU, "--map greybox"), "greybox");
}

#[test]
fn the_debug_views_apply_to_a_loaded_start_map_of_an_app_with_them() {
    static VIEWS: GameApp = GameApp {
        hooks: GameAppHooks {
            debug_views: true,
            ..GameAppHooks::NONE
        },
        ..BARE
    };
    let map = |game: &'static GameApp, line: &str| {
        let a = args(line);
        let start = StartMap::of(game, &a).unwrap();
        composed(game, a, start).world().resource::<Calls>().0[0].clone()
    };
    assert_eq!(map(&VIEWS, "--map a_game:x --debug-view albedo"), "map a_game:x Albedo");
    assert_eq!(map(&VIEWS, "--map a_game:x --lightmap-only"), format!("map a_game:x {:?}", MapDebugView::Lighting { scale: 1.0 }));
    assert_eq!(map(&BARE, "--map a_game:x --debug-view albedo"), "map a_game:x Normal", "not without `debug_views`");
    assert_eq!(map(&VIEWS, "--debug-view albedo"), "map greybox");
    assert!(chimera_client::client::app::debug_view(&args("--debug-view nope")).is_err());
}

#[test]
fn the_usage_text_is_the_games() {
    let usage = APP.usage();
    assert!(usage.starts_with("a_bin: A on Chimera.\n\nusage: a_bin [options]\n"), "{usage}");
    assert!(usage.contains("e.g. a_game:<name>, or greybox"), "{usage}");
    assert!(usage.contains("(repeatable; a_game: a_speed, a_jump)"), "{usage}");
    assert!(usage.contains("menus (A's join"), "{usage}");
    assert!(usage.contains("  host:    a_bin -port 27016"), "{usage}");
    assert!(usage.ends_with("  server:  cargo run --bin a_server -- -port 27016 +map greybox"), "{usage}");
    let bare = BARE.usage();
    assert!(bare.starts_with("usage: bare [options]\n"), "no about: {bare}");
    assert!(bare.ends_with("  join:    bare +connect 127.0.0.1:27016"), "no server: {bare}");
    assert!(!bare.contains("cs_source") && !usage.contains("cs_source"), "no other game's name");
    for line in usage.lines() {
        assert!(line.chars().count() <= 104, "too long: {line}");
    }
    assert_eq!(Args::parse_from(["--help".to_string()]).unwrap_err(), chimera_client::client::HELP);
}

#[test]
fn an_app_built_on_another_keeps_what_it_doesnt_set() {
    static OTHER: GameApp = GameApp {
        bin: "other",
        title: "Other",
        hooks: GameAppHooks {
            launch: None,
            ..APP.hooks
        },
        ..APP
    };
    let app = composed(&OTHER, OTHER.args(Args::default()), StartMap::Greybox);
    let calls = &app.world().resource::<Calls>().0;
    assert!(calls.contains(&"window Other".to_string()) && calls.contains(&"sim added".to_string()), "{calls:?}");
    assert!(!calls.contains(&"launch".to_string()));
    assert_eq!(OTHER.requirements().0, "warning: no keys");
}

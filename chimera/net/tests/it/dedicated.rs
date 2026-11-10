//! The dedicated server's optional parts (`chimera_net::dedicated::
//! ServerHooks`) on the engine's base and the greybox, with stand-in
//! games and no install: the player count; several games' maps (a bare
//! name's game, the loader of whole ids); the map cycle following the
//! map loaded; the map's file offered (`MapFiles`); the greybox's own
//! tick, from the start and after `changelevel greybox`; a minigame (its
//! console line, the banner, a map change restarting its match); what the
//! machine lacks, asked only off the greybox; the binary's launch hook.

use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use chimera_core::{console::Console, core::MapCycle, game_registry::GameInfo, slots::Loadout};
use chimera_net::{
    dedicated::{self, GameServer, Parsed, ServerHooks, ServerMaps, ServerMinigame, ServerPlugin, StartMap},
    minigame_net::MinigameNetPlugin,
    net::{NetSettings, maps::MapFiles},
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{ActiveMinigame, MatchResults, MinigameAppExt, MinigamePlugin},
    movement::placeholder,
    testkit,
};
use chimera_world::{
    harness::Sim,
    map::{LoadedMapName, MapData, MapLoadOptions},
};

/// A stand-in game with no install: 50 ticks a second, Source-like map files.
static GAME: GameInfo = GameInfo {
    id: "stand_in",
    short_name: "SI",
    content_dir: "si",
    tick_interval: 0.02,
    open: |_| Err(std::io::ErrorKind::NotFound.into()),
    map_cycle: |_| vec!["one".into(), "two".into()],
    map_names: |_| Vec::new(),
    map_file: |n| Some(format!("maps/{n}.bsp")),
    steam: None,
    import_map: |_| Err("no".into()),
    default_binds_cfg: "cfg/default.cfg",
    default_binds: &[],
    old_default_binds: &[],
    movement_cvars: &[],
    meters_per_unit: 0.0254,
    console_namespaces: &[],
};

fn no_map(name: &str, _: &MapLoadOptions) -> Result<MapData, String> {
    Err(format!("no map {name} here"))
}

const PLAIN: GameServer = GameServer {
    info: &GAME,
    title: "Stand-in",
    a_title: "a Stand-in",
    usage: "usage: stand_in_server",
    version: "test",
    sim: |app| {
        app.add_plugins(SimBasePlugins);
    },
    parts: |app| {
        app.insert_resource(Loadout {
            movement: placeholder::ID,
        });
    },
    load_map: no_map,
    hooks: ServerHooks::NONE,
};

static PLAIN_SERVER: GameServer = PLAIN;

/// The greybox at 32 Hz, the map cycle, the map's file, twelve players.
static HOOKED: GameServer = GameServer {
    hooks: ServerHooks {
        maxplayers: 12,
        map_cycle: true,
        map_files: true,
        greybox_tick: Some(1.0 / 32.0),
        ..ServerHooks::NONE
    },
    ..PLAIN
};

const DEF: &str = r#"
id = "kit:gun_game"
name = "Kit gun game"
players = { min = 2, max = 4 }
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "60s"
rotate = false

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
"#;

/// A minigame server whose bare names are another game's maps, which
/// tells what the machine lacks and has a launch hook.
static MINIGAME: GameServer = GameServer {
    title: "Kit Party",
    parts: |app| {
        app.add_plugins((MinigamePlugin, MinigameNetPlugin))
            .insert_resource(Loadout {
                movement: placeholder::ID,
            });
        testkit::add_weapons(app);
        app.add_minigames(&[("kit_gun_game.toml", DEF)])
            .expect("the definition reads");
    },
    hooks: ServerHooks {
        maps: Some(ServerMaps {
            bare: "other",
            load: |id, _| Err(format!("loaded {id} by id")),
            help: "greybox, other:<name>",
        }),
        minigame: Some(ServerMinigame {
            default: "kit:gun_game",
            first_map: |_| Ok("greybox".into()),
        }),
        requirements: Some(|| ("error: no install".into(), true)),
        launch: Some(|app| {
            app.insert_resource(Launched);
        }),
        ..ServerHooks::NONE
    },
    ..PLAIN
};

#[derive(Resource)]
struct Launched;

/// `server` from its command line, its console lines run.
fn server(server: &'static GameServer, args: &[&str]) -> Sim {
    let Ok(Parsed::Run(options)) = server.parse(args.iter().map(|s| s.to_string())) else {
        panic!("options")
    };
    let start = StartMap::load(server, &options).expect("the greybox");
    let mut sim = Sim::with((), |app| {
        app.add_plugins(ServerPlugin::new(server, start, &options));
    });
    sim.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            GAME.tick_interval,
        )));
    sim.app.update();
    sim
}

fn tick(sim: &Sim) -> Duration {
    sim.app.world().resource::<Time<Fixed>>().timestep()
}

#[test]
fn without_hooks_a_server_has_none_of_them() {
    let mut sim = server(&PLAIN_SERVER, &[]);
    let w = sim.app.world();
    assert_eq!(w.resource::<NetSettings>().maxplayers, 8);
    assert_eq!(
        tick(&sim),
        Duration::from_secs_f64(GAME.tick_interval),
        "the game's tick on the greybox"
    );
    assert!(!w.contains_resource::<MapCycle>() && !w.contains_resource::<MapFiles>());
    assert_eq!(PLAIN_SERVER.map_id("a"), "stand_in:a");
    assert!(
        PLAIN_SERVER
            .load_map("other:a")
            .unwrap_err()
            .contains("not a Stand-in map")
    );
    assert_eq!(PLAIN_SERVER.requirements("stand_in:a"), (String::new(), false));
    dedicated::change_level(sim.app.world_mut(), "greybox").unwrap();
    sim.app.update();
    assert_eq!(tick(&sim), Duration::from_secs_f64(GAME.tick_interval));
}

#[test]
fn the_hooks_set_players_cycle_files_and_the_greyboxs_tick() {
    let mut sim = server(&HOOKED, &[]);
    let w = sim.app.world();
    assert_eq!(w.resource::<NetSettings>().maxplayers, 12);
    assert_eq!(
        tick(&sim),
        Duration::from_secs_f64(1.0 / 32.0),
        "the greybox's own tick"
    );
    // The cycle follows the map loaded (its maps the install's: none here;
    // none loaded on the greybox it started on).
    assert_eq!(w.resource::<MapCycle>().current, "");
    sim.app
        .world_mut()
        .insert_resource(LoadedMapName("stand_in:two".into()));
    sim.app.update();
    assert_eq!(sim.app.world().resource::<MapCycle>().current, "two");
    // The map's file: the game's maps only, from its install (none here),
    // downloads into its content cache.
    let files = sim.app.world().resource::<MapFiles>().clone();
    assert!((files.read)("stand_in:two").is_none() && (files.read)("other:two").is_none());
    assert_eq!(files.cache, GAME.content_cache());
    // A map change to the greybox keeps the greybox's tick.
    sim.app
        .insert_resource(Time::<Fixed>::from_duration(Duration::from_secs_f64(
            GAME.tick_interval,
        )));
    dedicated::change_level(sim.app.world_mut(), "greybox").unwrap();
    assert_eq!(tick(&sim), Duration::from_secs_f64(1.0 / 32.0));
}

#[test]
fn a_minigame_server_plays_its_minigame_on_other_games_maps() {
    // Bare names are the other game's, loaded by whole id.
    assert_eq!(MINIGAME.map_id("a"), "other:a");
    assert_eq!(MINIGAME.load_map("other:a").unwrap_err(), "loaded other:a by id");
    // What the machine lacks: not asked on the greybox.
    assert_eq!(MINIGAME.requirements("greybox"), (String::new(), false));
    assert_eq!(MINIGAME.requirements("a"), ("error: no install".into(), true));

    let mut sim = server(&MINIGAME, &["+ip", "127.0.0.1", "-port", "0"]);
    let id = sim.app.world().resource::<ActiveMinigame>().def.id.clone();
    assert_eq!(id, "kit:gun_game", "the default minigame, by its console line");
    let addr = dedicated::listen(sim.app.world_mut()).expect("listen");
    let banner = sim
        .app
        .world()
        .resource::<Console>()
        .output
        .last()
        .unwrap()
        .text
        .clone();
    assert_eq!(
        banner,
        format!(
            "Kit Party dedicated server test ({}): kit:gun_game on greybox, UDP {addr}, 8 players",
            chimera_net::net::NET_VERSION
        )
    );
    // A map change starts the minigame's match over.
    sim.app.world_mut().insert_resource(MatchResults::default());
    dedicated::change_level(sim.app.world_mut(), "greybox").unwrap();
    assert!(
        !sim.app.world().contains_resource::<MatchResults>(),
        "the match restarted"
    );
}

#[test]
fn the_binarys_app_adds_the_launch_hook() {
    let Ok(Parsed::Run(options)) = MINIGAME.parse(Vec::new()) else {
        panic!("options")
    };
    let app = dedicated::app(&MINIGAME, StartMap::Greybox, &options);
    assert!(app.world().contains_resource::<Launched>());
    let app = dedicated::app(&PLAIN_SERVER, StartMap::Greybox, &options);
    assert!(!app.world().contains_resource::<Launched>());
}

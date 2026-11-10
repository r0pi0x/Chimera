//! A game's dedicated server, once for every game
//! (docs/plans/active/architecture-pass.md, step 4): its simulation and
//! the network with no window, render or audio. A game's server crate
//! says what is its own as data (`GameServer`: its `GameInfo`, its
//! plugins, how a map of its install loads, its usage text and build
//! stamp) and its binary is `run(&SERVER)`; the options, the start map,
//! `ServerPlugin`, `changelevel`, the stdin console and the run loop are
//! here. HL2:DM's and Combat Arms' servers are built on it
//! (apps/hl2dm_server, apps/combat_arms_server); their tests build
//! `ServerPlugin` the same way the binary does.
//!
//!   <game>_server [-port <n>] [+map <name>] [+<command> ...]
//!
//! - Maps: `+map greybox` (the default) or `+map <game>:<name>` (a bare
//!   name works too) from the install (`GameServer::load_map`), converted
//!   lean (no textures); `changelevel <map>` (or `map <map>`) loads
//!   another in the background and moves every player there, a fresh
//!   match (`gameplay::new_game`). Clients load the map from their own
//!   install by name.
//! - Players join with the game's app (`+connect <ip>:<port>`); a server
//!   takes eight unless told otherwise (`maxplayers`).
//! - `run`: at most 300 frames a second (`FPS_MAX`), schedules on one
//!   thread each (`single_threaded_schedules`; `CHIMERA_EXECUTOR=multi`
//!   keeps Bevy's), the console on stdin and its output on stdout,
//!   Ctrl+C as `quit`.
//!
//! Known shortcuts: no `MapFiles` (a client without the map can't
//! download it: maps load from each player's install); changing to the
//! greybox keeps the game's tick (the greybox has no tick of its own);
//! CS:S's server (apps/cs_source_server) and the party apps'
//! (`party_kit::server`) are still their own copies of this shape.

use std::{
    io::BufRead,
    sync::{Mutex, mpsc},
    time::Duration,
};

use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use chimera_core::{
    console::{self, Console, ConsoleAppExt, Level},
    core::{self, BuildVersion},
    game_registry::GameInfo,
    gameplay::{self, Gameplay, GameplayAppExt},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    map::{self, LoadedMapName, MapData, MapLoadOptions, MapSimPlugin},
};

use crate::net::{self, NetPlugin, NetSettings};

/// What a game's dedicated server is made of (module doc): the game's
/// own, as data. Its server crate holds one in a `static`.
pub struct GameServer {
    /// The game it serves: its id names its maps (`<id>:<name>`), its
    /// tick is the server's.
    pub info: &'static GameInfo,
    /// The game's name in the banner (`HL2:DM`).
    pub title: &'static str,
    /// The same with its article, for messages (`an HL2:DM`).
    pub a_title: &'static str,
    /// The `--help` text.
    pub usage: &'static str,
    /// The build's stamp (the banner, `version`): the server crate's
    /// `env!("CHIMERA_VERSION")`, from its build.rs.
    pub version: &'static str,
    /// The game's simulation (its `SimPlugins`), added first.
    pub sim: fn(&mut App),
    /// The game's parts (its `<Game>Plugins`) and what its server sets,
    /// added after the network.
    pub parts: fn(&mut App),
    /// A map of the install by name (no `<id>:` before it), with the
    /// server's load options (lean: no textures).
    pub load_map: fn(&str, &MapLoadOptions) -> Result<MapData, String>,
}

/// The game a server app serves (`ServerPlugin` sets it).
#[derive(Resource, Clone, Copy)]
pub struct Serving(pub &'static GameServer);

/// The command line: the map, and the console lines to run first (in
/// order; `-port <n>` is `hostport <n>`, first).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub map: String,
    pub console: Vec<String>,
}

/// What `parse` asks of the caller beyond options.
#[derive(Debug, PartialEq)]
pub enum Parsed {
    Run(Options),
    Help,
}

/// The command line (without the program's name).
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, String> {
    let mut map = net::GREYBOX.to_string();
    let mut console = Vec::new();
    let mut it = args.into_iter().peekable();
    while let Some(arg) = it.next() {
        if arg == "-h" || arg == "--help" {
            return Ok(Parsed::Help);
        }
        if arg == "-port" {
            let port = it.next().ok_or("-port: missing value")?;
            port.parse::<u16>().map_err(|_| format!("-port: not a port: {port}"))?;
            console.insert(0, format!("hostport {port}"));
            continue;
        }
        let Some(name) = arg.strip_prefix('+').filter(|n| !n.is_empty()) else {
            return Err(format!("unknown option {arg}"));
        };
        let mut words = vec![name.to_string()];
        while let Some(next) = it.peek() {
            if next.starts_with('+') && next.len() > 1 || next == "-port" {
                break;
            }
            words.push(it.next().unwrap());
        }
        if name.eq_ignore_ascii_case("map") {
            map = words.get(1).cloned().ok_or("+map: missing name")?;
        } else {
            console.push(words.iter().map(|w| console::quote(w)).collect::<Vec<_>>().join(" "));
        }
    }
    Ok(Parsed::Run(Options { map, console }))
}

impl GameServer {
    /// A map's id as the server serves it: `greybox`, or `<game>:<name>`
    /// (a bare name is the game's).
    pub fn map_id(&self, name: &str) -> String {
        let id = net::normalize_map(name);
        if id == net::GREYBOX || id.contains(':') {
            id
        } else {
            format!("{}:{id}", self.info.id)
        }
    }

    /// Map `id` from the install (`<game>:<name>`).
    pub fn load_map(&self, id: &str) -> Result<MapData, String> {
        self.load_map_in(id, &map::LoadContext::default())
    }

    /// `load_map` for the app whose context is `context`
    /// (`map::LoadContext`: its install data).
    pub fn load_map_in(&self, id: &str, context: &map::LoadContext) -> Result<MapData, String> {
        let game = self.info.id;
        match id.split_once(':') {
            Some((other, _)) if other != game => Err(format!("{id}: not {} map ({game}:<name>)", self.a_title)),
            // A server draws nothing: no textures.
            Some((_, name)) => (self.load_map)(name, &map::MapLoadOptions::lean().in_context(context)),
            None => (self.load_map)(id, &map::MapLoadOptions::lean().in_context(context)),
        }
    }
}

/// The map a server starts on: the greybox, or one loaded from the install.
pub enum StartMap {
    Greybox,
    /// `context`: the context it was loaded in (`map::LoadContext`), the
    /// server app's from then on, so the install data it read stays held.
    Loaded {
        id: String,
        data: MapData,
        context: map::LoadContext,
    },
}

impl StartMap {
    /// `options.map`, loaded as `server`'s game loads it.
    pub fn load(server: &GameServer, options: &Options) -> Result<Self, String> {
        let id = server.map_id(&options.map);
        if id == net::GREYBOX {
            return Ok(Self::Greybox);
        }
        let context = map::LoadContext::default();
        let data = server
            .load_map_in(&id, &context)
            .map_err(|e| format!("+map {id}: {e}"))?;
        Ok(Self::Loaded { id, data, context })
    }
}

/// Everything the server runs above the headless base (`run`'s
/// `MinimalPlugins` with its run loop, a test's `Sim`): the game's
/// simulation and parts (`GameServer::sim`, `parts`), the map's systems alone (`MapSimPlugin`: nothing
/// drawn or played) with the start map, the network, the console's
/// commands (`changelevel`, `map`, `quit`) and the command line's
/// console lines, submitted to run on the first frame. Listening is
/// `listen`'s, once those lines ran (they set the port and the player
/// count).
pub struct ServerPlugin {
    pub server: &'static GameServer,
    pub map: std::sync::Mutex<Option<StartMap>>,
    pub console: Vec<String>,
}

impl ServerPlugin {
    pub fn new(server: &'static GameServer, map: StartMap, options: &Options) -> Self {
        Self {
            server,
            map: std::sync::Mutex::new(Some(map)),
            console: options.console.clone(),
        }
    }
}

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        let start = self.map.lock().unwrap().take();
        if let Some(StartMap::Loaded { context, .. }) = &start {
            app.insert_resource(context.clone());
        }
        let server = self.server;
        app.insert_resource(Serving(server));
        (server.sim)(app);
        match start {
            Some(StartMap::Loaded { id, data, .. }) => {
                app.add_plugins(MapSimPlugin::new(data))
                    .insert_resource(LoadedMapName(id));
            }
            // The map systems too: `changelevel` loads one later.
            _ => {
                app.add_plugins((GreyboxMapPlugin, MapSimPlugin::empty()));
            }
        }
        // The game is the gameplay: its rules and tick.
        app.set_gameplay(Gameplay::of(server.info))
            .insert_resource(BuildVersion(server.version))
            .add_plugins(NetPlugin);
        (server.parts)(app);
        // A dedicated server takes eight players unless told otherwise.
        app.insert_resource(NetSettings {
            maxplayers: 8,
            ..default()
        })
        .add_systems(Update, finish_level_change)
        .console_command(
            "changelevel",
            &format!(
                "changelevel <map>: move the server and its players to another map (greybox or {}:<name>).",
                server.info.id
            ),
            |w, a| change_level(w, a.first().ok_or("changelevel <map>")?),
        )
        .console_command(
            "map",
            "map <map>: the same as changelevel on a dedicated server.",
            |w, a| change_level(w, a.first().ok_or("map <map>")?),
        )
        .console_command("quit", "Stop the server.", quit)
        .console_command("exit", "Stop the server.", quit);
        let mut console = app.world_mut().resource_mut::<Console>();
        for line in &self.console {
            console.submit(line.clone());
        }
    }
}

fn quit(world: &mut World, _: &[String]) -> Result<Option<String>, String> {
    net::disconnect(world, "Server shutting down.");
    world.write_message(AppExit::Success);
    Ok(None)
}

/// Start listening (UDP on `hostport`, `ip`) and say so on the console:
/// the address bound.
pub fn listen(world: &mut World) -> Result<std::net::SocketAddr, String> {
    let addr = net::server::listen(world)?;
    let max = world.resource::<NetSettings>().maxplayers;
    let map = net::current_map(world);
    let server = world.resource::<Serving>().0;
    world.resource_mut::<Console>().info(format!(
        "Chimera {} dedicated server {} ({}): {map} on UDP {addr}, {max} players",
        server.title,
        server.version,
        net::NET_VERSION
    ));
    Ok(addr)
}

/// A map loading in the background for `changelevel`.
#[derive(Resource)]
struct LevelLoad {
    id: String,
    task: bevy::tasks::Task<Result<MapData, String>>,
}

/// `changelevel <map>`: the greybox at once; a map of the game loaded in
/// the background (the game plays on), then swapped in
/// (`finish_level_change`).
pub fn change_level(world: &mut World, name: &str) -> Result<Option<String>, String> {
    let server = world.resource::<Serving>().0;
    let id = server.map_id(name);
    if id == net::GREYBOX {
        swap_map(world, net::GREYBOX, None);
        return Ok(Some("changed level to the greybox".into()));
    }
    if world.contains_resource::<LevelLoad>() {
        return Err("a map is loading already".into());
    }
    // Clients hear of it now (`net::maps`: the map's name changes first).
    world.insert_resource(LoadedMapName(id.clone()));
    let context = map::LoadContext::of(world);
    let task = bevy::tasks::AsyncComputeTaskPool::get().spawn({
        let id = id.clone();
        async move { server.load_map_in(&id, &context) }
    });
    world.insert_resource(LevelLoad { id: id.clone(), task });
    Ok(Some(format!("changing level to {id}...")))
}

/// The map `id` in place of the one playing (None: the greybox), and a
/// fresh match on it (`gameplay::new_game`).
pub fn swap_map(world: &mut World, id: &str, data: Option<MapData>) {
    match data {
        Some(data) => {
            greybox::unload(world);
            map::change_map(world, data, map::MapDebugView::Normal);
            world.insert_resource(LoadedMapName(id.to_string()));
        }
        None => {
            map::unload_map(world);
            greybox::unload(world);
            greybox::respawn(world);
            world.remove_resource::<map::ActiveMapLook>();
            world.insert_resource(LoadedMapName(net::GREYBOX.into()));
        }
    }
    // At the gameplay's tick (the game's).
    let game_tick = world.resource::<Serving>().0.info.tick_interval;
    let tick = world
        .get_resource::<Gameplay>()
        .map_or(Duration::from_secs_f64(game_tick), |g| g.tick);
    core::set_tick_length(world, tick);
    gameplay::new_game(world);
}

fn finish_level_change(world: &mut World) {
    let Some(mut load) = world.get_resource_mut::<LevelLoad>() else {
        return;
    };
    let Some(result) = bevy::tasks::block_on(bevy::tasks::poll_once(&mut load.task)) else {
        return;
    };
    let id = load.id.clone();
    world.remove_resource::<LevelLoad>();
    match result {
        Ok(data) => {
            let summary = format!("loaded {id}: {} spawns", data.spawns.len());
            swap_map(world, &id, Some(data));
            world.resource_mut::<Console>().info(summary);
        }
        Err(e) => {
            // Back to the map that plays: clients never left it.
            let server = world.resource::<Serving>().0;
            let back = world
                .get_resource::<map::MapFile>()
                .map(|f| server.map_id(&f.name))
                .unwrap_or_else(|| net::GREYBOX.into());
            world.insert_resource(LoadedMapName(back));
            world
                .resource_mut::<Console>()
                .print(console::Level::Error, format!("changelevel {id}: {e}"));
        }
    }
}

/// Frames a second at most: a packet waits at most a frame (3.3 ms).
pub const FPS_MAX: f64 = 300.0;

/// The server's binary: the command line, the start map, `ServerPlugin`
/// on a headless run loop, the console on stdin and its output on
/// stdout. Returns when the server stops; exits the process on a bad
/// command line (2) or a start map that doesn't load (1).
pub fn run(server: &'static GameServer) {
    let usage = server.usage;
    let options = match parse(std::env::args().skip(1)) {
        Ok(Parsed::Run(o)) => o,
        Ok(Parsed::Help) => {
            println!("{usage}");
            return;
        }
        Err(e) => {
            eprintln!("{e}\n\n{usage}");
            std::process::exit(2);
        }
    };
    let start = StartMap::load(server, &options).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    if let StartMap::Loaded { id, data, .. } = &start {
        eprintln!(
            "loaded {id}: {} spawns, {} warnings",
            data.spawns.len(),
            data.warnings.len()
        );
    }
    let mut app = App::new();
    chimera_world::harness::add_headless_with(
        &mut app,
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / FPS_MAX))),
    );
    // Ctrl+C exits as `quit` does (AppExit: `net::udp` hangs up on every
    // client).
    app.add_plugins((bevy::log::LogPlugin::default(), bevy::app::TerminalCtrlCHandlerPlugin))
        .add_plugins(ServerPlugin::new(server, start, &options))
        .add_systems(Update, (read_stdin, start_serving, print_console).chain());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    app.insert_resource(Stdin(Mutex::new(rx)));
    if chimera_core::env::var("CHIMERA_EXECUTOR").as_deref() != Ok("multi") {
        chimera_world::harness::single_threaded_schedules(&mut app);
    }
    app.run();
}

/// Lines typed on stdin.
#[derive(Resource)]
struct Stdin(Mutex<mpsc::Receiver<String>>);

fn read_stdin(stdin: Res<Stdin>, mut console: ResMut<Console>) {
    let rx = stdin.0.lock().unwrap();
    while let Ok(line) = rx.try_recv() {
        console.print(Level::Input, format!("] {line}"));
        console.submit(line);
    }
}

/// Listen once the command line's console lines have run (they set the
/// port and player count).
fn start_serving(world: &mut World, mut started: Local<bool>) {
    if *started {
        return;
    }
    *started = true;
    if let Err(e) = listen(world) {
        eprintln!("error: {e}");
        world.write_message(AppExit::error());
    }
}

/// Console output (command results, errors) to stdout.
fn print_console(console: Res<Console>, mut printed: Local<u64>) {
    let new = console.printed.saturating_sub(*printed) as usize;
    *printed = console.printed;
    let lines = &console.output;
    for line in &lines[lines.len().saturating_sub(new)..] {
        if line.level != Level::Input {
            println!("{}", line.text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Options {
        match parse(args.iter().map(|s| s.to_string())).unwrap() {
            Parsed::Run(o) => o,
            Parsed::Help => panic!("help"),
        }
    }

    #[test]
    fn the_command_line_becomes_console_lines() {
        let o = run(&["+map", "a_map", "+maxplayers", "4", "-port", "27040", "+mode", "TDM"]);
        assert_eq!(o.map, "a_map");
        assert_eq!(o.console, ["hostport 27040", "maxplayers 4", "mode TDM"]);
        assert_eq!(run(&[]).map, "greybox");
        assert!(parse(["nope".to_string()]).is_err());
        assert_eq!(parse(["--help".to_string()]).unwrap(), Parsed::Help);
    }
}

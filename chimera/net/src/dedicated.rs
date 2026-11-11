//! A game's dedicated server, once for every game
//! (docs/plans/active/architecture-pass.md, steps 4 and 6): its
//! simulation and the network with no window, render or audio. A game's
//! server crate says what is its own as data (`GameServer`: its
//! `GameInfo`, its plugins, how a map of its install loads, its usage
//! text and build stamp, and the optional `ServerHooks`) and its binary
//! is `run(&SERVER)`; the options, the start map, `ServerPlugin`,
//! `changelevel`, the stdin console and the run loop are here. Every
//! dedicated server is built on it: CS:S's, HL2:DM's and Combat Arms'
//! (apps/<game>_server) and the party apps' (`party_kit::server`); their
//! tests build `ServerPlugin` the same way the binary does.
//!
//!   <game>_server [-port <n>] [+minigame <id>] [+map <name>] [+<command> ...]
//!
//! - Maps: `+map greybox` (the default) or `+map <game>:<name>` (a bare
//!   name works too) from the install (`GameServer::load_map`), converted
//!   lean (no textures); `changelevel <map>` (or `map <map>`) loads
//!   another in the background and moves every player there, a fresh
//!   match (`gameplay::new_game`). Clients load the map from their own
//!   install by name.
//! - Players join with the game's app (`+connect <ip>:<port>`); a server
//!   takes eight unless told otherwise (`maxplayers`; `ServerHooks::
//!   maxplayers`).
//! - `run`: at most 300 frames a second (`FPS_MAX`), schedules on one
//!   thread each (`single_threaded_schedules`; `CHIMERA_EXECUTOR=multi`
//!   keeps Bevy's), the console on stdin and its output on stdout,
//!   Ctrl+C as `quit`. `tick_stats` prints the last ticks' wall and CPU
//!   ms and instructions (`metrics::TickMetrics`, when the game's sim adds
//!   it) and the process's resident memory, for watching a live server.
//! - What only some servers have, `ServerHooks` (all off in
//!   `ServerHooks::NONE`): several games' maps (`maps`: a loader of whole
//!   ids, the game a bare name is); the map cycle (`map_cycle`:
//!   `core::MapCycle` follows the map loaded, its maps the install's,
//!   `GameInfo::map_cycle`); the map's file for clients without it
//!   (`map_files`: `net::maps::MapFiles` from `GameInfo::map_file` in the
//!   mounted install); the greybox's own tick (`greybox_tick`); a minigame
//!   (`minigame`: `+minigame <id>`, its pool's first map without `+map`,
//!   started by the console line `minigame <id>` after the command line's
//!   others, restarted on a map change, named in the banner); what the
//!   machine lacks for a map of the installs, told before it loads
//!   (`requirements`); and what the binary adds that the tests don't
//!   (`launch`).
//!
//! Known shortcuts: a server without `map_files` offers no map file (a
//! client without the map can't download it: maps load from each
//! player's install); without `greybox_tick`, changing to the greybox
//! keeps the game's tick (the greybox has no tick of its own); a
//! minigame's map pool isn't rotated by the server (a map change keeps
//! the minigame, its match starts over).

use std::{
    io::BufRead,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use chimera_core::{
    console::{self, Console, ConsoleAppExt, Level},
    core::{self, BuildVersion, MapCycle},
    game_registry::GameInfo,
    gameplay::{self, Gameplay, GameplayAppExt},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    map::{self, LoadedMapName, MapData, MapLoadOptions, MapSimPlugin},
};

use crate::net::{self, NetPlugin, NetSettings, maps::MapFiles};

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
    /// server's load options (lean: no textures). With `hooks.maps`,
    /// that hook loads instead.
    pub load_map: fn(&str, &MapLoadOptions) -> Result<MapData, String>,
    /// What only some servers have (`ServerHooks::NONE`: nothing).
    pub hooks: ServerHooks,
}

/// A server's optional parts (module doc). A server sets the ones it has
/// over `NONE`: `ServerHooks { map_cycle: true, ..ServerHooks::NONE }`.
#[derive(Clone, Copy)]
pub struct ServerHooks {
    /// Players a server takes unless told otherwise (`maxplayers`).
    pub maxplayers: u32,
    /// Maps of several games' installs (None: the game's alone, by
    /// `GameServer::load_map`).
    pub maps: Option<ServerMaps>,
    /// Keep `core::MapCycle` (the rules' next map when a game ends) on the
    /// map loaded, its maps from the install's cycle
    /// (`GameInfo::map_cycle`).
    pub map_cycle: bool,
    /// Offer the served map's file to clients without it
    /// (`net::maps::MapFiles`: `GameInfo::map_file` read from the mounted
    /// install; downloads cached in the game's content cache).
    pub map_files: bool,
    /// The greybox's tick, seconds (None: the game's), on it from the
    /// start and after `changelevel greybox`.
    pub greybox_tick: Option<f64>,
    /// The server plays a minigame (`ServerMinigame`).
    pub minigame: Option<ServerMinigame>,
    /// What this machine lacks for a map of the installs (not asked on
    /// the greybox): the text told on stderr before the map loads (empty:
    /// nothing), and whether it stops the server.
    pub requirements: Option<fn() -> (String, bool)>,
    /// What the binary adds after `ServerPlugin` and the tests don't (a
    /// fresh seed, a launch-only plugin).
    pub launch: Option<fn(&mut App)>,
}

impl ServerHooks {
    /// None of them; eight players.
    pub const NONE: Self = Self {
        maxplayers: 8,
        maps: None,
        map_cycle: false,
        map_files: false,
        greybox_tick: None,
        minigame: None,
        requirements: None,
        launch: None,
    };
}

/// Maps of several games (`ServerHooks::maps`).
#[derive(Clone, Copy)]
pub struct ServerMaps {
    /// The game a bare map name (no `<game>:`) is.
    pub bare: &'static str,
    /// Map `id` (`<game>:<name>`) with the server's load options.
    pub load: fn(&str, &MapLoadOptions) -> Result<MapData, String>,
    /// The maps `changelevel` takes, for its help
    /// (`greybox, a:<name>, b:<name>`).
    pub help: &'static str,
}

/// A server that plays a minigame (`ServerHooks::minigame`): `+minigame
/// <id>` on its command line.
#[derive(Clone, Copy)]
pub struct ServerMinigame {
    /// The minigame without `+minigame`.
    pub default: &'static str,
    /// The first map of minigame `id`'s pool (the start map without
    /// `+map`), or why there's none.
    pub first_map: fn(&str) -> Result<String, String>,
}

/// The game a server app serves (`ServerPlugin` sets it).
#[derive(Resource, Clone, Copy)]
pub struct Serving(pub &'static GameServer);

/// The command line: the map, the minigame (a server with one), and the
/// console lines to run first (in order; `-port <n>` is `hostport <n>`,
/// first; the minigame's line last).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub map: String,
    pub minigame: Option<String>,
    pub console: Vec<String>,
}

/// What `parse` asks of the caller beyond options.
#[derive(Debug, PartialEq)]
pub enum Parsed {
    Run(Options),
    Help,
}

/// The command line (without the program's name) of a server without a
/// minigame (`GameServer::parse` for any).
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, String> {
    parse_with(args, None)
}

fn parse_with(args: impl IntoIterator<Item = String>, minigame: Option<&ServerMinigame>) -> Result<Parsed, String> {
    let mut map = None;
    let mut chosen = None;
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
            map = Some(words.get(1).cloned().ok_or("+map: missing name")?);
        } else if minigame.is_some() && name.eq_ignore_ascii_case("minigame") {
            chosen = Some(words.get(1).cloned().ok_or("+minigame: missing id")?);
        } else {
            console.push(words.iter().map(|w| console::quote(w)).collect::<Vec<_>>().join(" "));
        }
    }
    let Some(minigame) = minigame else {
        let map = map.unwrap_or_else(|| net::GREYBOX.to_string());
        return Ok(Parsed::Run(Options {
            map,
            minigame: None,
            console,
        }));
    };
    let chosen = chosen.unwrap_or_else(|| minigame.default.to_string());
    let map = match map {
        Some(m) => m,
        None => (minigame.first_map)(&chosen)?,
    };
    console.push(format!("minigame {chosen}"));
    Ok(Parsed::Run(Options {
        map,
        minigame: Some(chosen),
        console,
    }))
}

impl GameServer {
    /// This server's command line (without the program's name): `+minigame`
    /// is an option of a server with a minigame (`ServerHooks::minigame`).
    pub fn parse(&self, args: impl IntoIterator<Item = String>) -> Result<Parsed, String> {
        parse_with(args, self.hooks.minigame.as_ref())
    }

    /// A map's id as the server serves it: `greybox`, or `<game>:<name>`
    /// (a bare name is the game's, or `ServerMaps::bare`'s).
    pub fn map_id(&self, name: &str) -> String {
        let id = net::normalize_map(name);
        if id == net::GREYBOX || id.contains(':') {
            id
        } else {
            let bare = self.hooks.maps.map_or(self.info.id, |m| m.bare);
            format!("{bare}:{id}")
        }
    }

    /// Map `id` from the install (`<game>:<name>`).
    pub fn load_map(&self, id: &str) -> Result<MapData, String> {
        self.load_map_in(id, &map::LoadContext::default())
    }

    /// `load_map` for the app whose context is `context`
    /// (`map::LoadContext`: its install data).
    pub fn load_map_in(&self, id: &str, context: &map::LoadContext) -> Result<MapData, String> {
        // A server draws nothing: no textures.
        let options = map::MapLoadOptions::lean().in_context(context);
        if let Some(maps) = &self.hooks.maps {
            return (maps.load)(id, &options);
        }
        let game = self.info.id;
        match id.split_once(':') {
            Some((other, _)) if other != game => Err(format!("{id}: not {} map ({game}:<name>)", self.a_title)),
            Some((_, name)) => (self.load_map)(name, &options),
            None => (self.load_map)(id, &options),
        }
    }

    /// What this machine lacks to serve map `id` (`ServerHooks::
    /// requirements`; nothing on the greybox): the text to tell (empty:
    /// nothing) and whether it stops the server.
    pub fn requirements(&self, id: &str) -> (String, bool) {
        match self.hooks.requirements {
            Some(check) if self.map_id(id) != net::GREYBOX => check(),
            _ => (String::new(), false),
        }
    }

    /// The map `id` in place of the one playing (None: the greybox), and a
    /// fresh match on it (`gameplay::new_game`; the minigame's too, a
    /// server with one), at the gameplay's tick (`greybox_tick` on the
    /// greybox, a server with one).
    pub fn swap_map(&self, world: &mut World, id: &str, data: Option<MapData>) {
        let greybox_tick = match data {
            Some(data) => {
                greybox::unload(world);
                map::change_map(world, data, map::MapDebugView::Normal);
                world.insert_resource(LoadedMapName(id.to_string()));
                None
            }
            None => {
                map::unload_map(world);
                greybox::unload(world);
                greybox::respawn(world);
                world.remove_resource::<map::ActiveMapLook>();
                world.insert_resource(LoadedMapName(net::GREYBOX.into()));
                self.hooks.greybox_tick
            }
        };
        // At the gameplay's tick (the game's).
        let tick = match greybox_tick {
            Some(secs) => Duration::from_secs_f64(secs),
            None => world
                .get_resource::<Gameplay>()
                .map_or(Duration::from_secs_f64(self.info.tick_interval), |g| g.tick),
        };
        core::set_tick_length(world, tick);
        gameplay::new_game(world);
        if self.hooks.minigame.is_some() && world.contains_resource::<chimera_sim::minigame::ActiveMinigame>() {
            chimera_sim::minigame::restart(world);
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
/// simulation and parts (`GameServer::sim`, `parts`), the map's systems
/// alone (`MapSimPlugin`: nothing drawn or played) with the start map,
/// the network, the hooks' parts (`ServerHooks`), the console's commands
/// (`changelevel`, `map`, `quit`) and the command line's console lines,
/// submitted to run on the first frame. Listening is `listen`'s, once
/// those lines ran (they set the port and the player count).
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
        let hooks = &server.hooks;
        app.insert_resource(Serving(server));
        (server.sim)(app);
        let on_greybox = !matches!(start, Some(StartMap::Loaded { .. }));
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
        app.set_gameplay(Gameplay::of(server.info));
        if let Some(secs) = hooks.greybox_tick.filter(|_| on_greybox) {
            app.insert_resource(Time::<Fixed>::from_duration(Duration::from_secs_f64(secs)));
        }
        app.insert_resource(BuildVersion(server.version)).add_plugins(NetPlugin);
        (server.parts)(app);
        if hooks.map_cycle {
            app.init_resource::<MapCycle>().add_systems(Update, follow_map_cycle);
        }
        if hooks.map_files {
            let info = server.info;
            app.insert_resource(MapFiles {
                read: Arc::new(move |id: &str| install_map_file(info, id)),
                cache: info.content_cache(),
            });
        }
        let maps = match &hooks.maps {
            Some(m) => m.help.to_string(),
            None => format!("greybox or {}:<name>", server.info.id),
        };
        app.insert_resource(NetSettings {
            maxplayers: hooks.maxplayers,
            ..default()
        })
        .add_systems(Update, finish_level_change)
        .console_command(
            "changelevel",
            &format!("changelevel <map>: move the server and its players to another map ({maps})."),
            |w, a| change_level(w, a.first().ok_or("changelevel <map>")?),
        )
        .console_command(
            "map",
            "map <map>: the same as changelevel on a dedicated server.",
            |w, a| change_level(w, a.first().ok_or("map <map>")?),
        )
        .console_command(
            "tick_stats",
            "The last ticks' wall and CPU ms and main-thread instructions, and the process's memory (Linux).",
            |w, _| Ok(Some(tick_stats(w))),
        )
        .console_command("quit", "Stop the server.", quit)
        .console_command("exit", "Stop the server.", quit);
        let mut console = app.world_mut().resource_mut::<Console>();
        for line in &self.console {
            console.submit(line.clone());
        }
    }
}

/// The bytes of map `id`'s file in `info`'s mounted install
/// (`GameInfo::map_file`), for `MapFiles`; None for another game's map
/// or one without a file.
pub fn install_map_file(info: &GameInfo, id: &str) -> Option<Vec<u8>> {
    let (game, name) = id.split_once(':')?;
    if game != info.id {
        return None;
    }
    let file = (info.map_file)(name)?;
    info.mount()?.read(&file).ok()
}

/// The rules' map cycle follows the map loaded (`ServerHooks::
/// map_cycle`); its maps from the install's cycle, once.
fn follow_map_cycle(name: Option<Res<LoadedMapName>>, serving: Res<Serving>, mut cycle: ResMut<MapCycle>) {
    let Some(name) = name.filter(|n| n.is_changed()) else {
        return;
    };
    cycle.current = net::maps::map_name(&name.0).to_string();
    if cycle.maps.is_empty() {
        let info = serving.0.info;
        cycle.maps = info.mount().map(|m| (info.map_cycle)(&m)).unwrap_or_default();
    }
}

/// `tick_stats`: each simulation tick's cost over the last ticks
/// (`metrics::TickMetrics`, when the game's sim adds it; the schedules
/// run on the main thread, so its instructions are the tick's work) and
/// the process's resident memory, for watching a live server.
pub fn tick_stats(world: &mut World) -> String {
    let mut lines = match world.get_resource::<chimera_core::metrics::TickMetrics>() {
        Some(m) if m.ticks > 0 => {
            let mut l = m.work.summary("tick");
            l.insert(0, format!("ticks   : {} since the start", m.ticks));
            l
        }
        _ => vec!["ticks   : not measured (no TickMetricsPlugin in this server)".into()],
    };
    if let Some(kb) = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).map(str::to_owned))
    {
        lines.push(format!("memory  : {}", kb.trim_start_matches("VmRSS:").trim()));
    }
    lines.join("\n")
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
    let text = if server.hooks.minigame.is_some() {
        let minigame = world
            .get_resource::<chimera_sim::minigame::ActiveMinigame>()
            .map_or("no minigame".to_string(), |m| m.def.id.clone());
        format!(
            "{} dedicated server {} ({}): {minigame} on {map}, UDP {addr}, {max} players",
            server.title,
            server.version,
            net::NET_VERSION
        )
    } else {
        format!(
            "Chimera {} dedicated server {} ({}): {map} on UDP {addr}, {max} players",
            server.title,
            server.version,
            net::NET_VERSION
        )
    };
    world.resource_mut::<Console>().info(text);
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

/// `GameServer::swap_map` for the server this app serves (`Serving`).
pub fn swap_map(world: &mut World, id: &str, data: Option<MapData>) {
    world.resource::<Serving>().0.swap_map(world, id, data);
}

fn finish_level_change(world: &mut World) {
    let wait = chimera_core::tasks::waits(world);
    let Some(mut load) = world.get_resource_mut::<LevelLoad>() else {
        return;
    };
    let Some(result) = chimera_core::tasks::poll(&mut load.task, wait) else {
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

/// The server's binary: the command line, what the machine lacks
/// (`ServerHooks::requirements`), the start map, `ServerPlugin` and the
/// launch hook on a headless run loop (`app`), the console on stdin and
/// its output on stdout. Returns when the server stops; exits the
/// process on a bad command line (2), or a lack that stops it or a start
/// map that doesn't load (1).
pub fn run(server: &'static GameServer) {
    let usage = server.usage;
    let options = match server.parse(std::env::args().skip(1)) {
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
    let (lacks, fatal) = server.requirements(&options.map);
    if !lacks.is_empty() {
        eprintln!("{lacks}");
    }
    if fatal {
        std::process::exit(1);
    }
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
    if let Some(minigame) = &options.minigame {
        eprintln!("minigame {minigame}");
    }
    let mut app = self::app(server, start, &options);
    // Ctrl+C exits as `quit` does (AppExit: `net::udp` hangs up on every
    // client).
    app.add_plugins((bevy::log::LogPlugin::default(), bevy::app::TerminalCtrlCHandlerPlugin))
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

/// The binary's app before its log, stdin and executor: the headless
/// base on a run loop of at most `FPS_MAX` frames a second,
/// `ServerPlugin`, then `ServerHooks::launch`.
pub fn app(server: &'static GameServer, start: StartMap, options: &Options) -> App {
    let mut app = App::new();
    chimera_world::harness::add_headless_with(
        &mut app,
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / FPS_MAX))),
    );
    app.add_plugins(ServerPlugin::new(server, start, options));
    if let Some(launch) = server.hooks.launch {
        launch(&mut app);
    }
    app
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
        assert_eq!(o.minigame, None);
        assert_eq!(run(&[]).map, "greybox");
        // Without a minigame, `+minigame` is a console line like any other.
        assert_eq!(run(&["+minigame", "x"]).console, ["minigame x"]);
        assert!(parse(["nope".to_string()]).is_err());
        assert_eq!(parse(["--help".to_string()]).unwrap(), Parsed::Help);
    }

    #[test]
    fn a_minigame_server_takes_the_minigame_and_its_first_map() {
        let hook = ServerMinigame {
            default: "x:one",
            first_map: |id| match id {
                "x:one" => Ok("a:first".into()),
                "x:two" => Ok("b:second".into()),
                _ => Err(format!("no minigame `{id}`")),
            },
        };
        let run = |args: &[&str]| match parse_with(args.iter().map(|s| s.to_string()), Some(&hook)).unwrap() {
            Parsed::Run(o) => o,
            Parsed::Help => panic!("help"),
        };
        let o = run(&["+maxplayers", "4", "-port", "27040"]);
        assert_eq!((o.minigame.as_deref(), o.map.as_str()), (Some("x:one"), "a:first"));
        assert_eq!(
            o.console,
            ["hostport 27040", "maxplayers 4", "minigame x:one"],
            "its line last"
        );
        let o = run(&["+minigame", "x:two"]);
        assert_eq!((o.minigame.as_deref(), o.map.as_str()), (Some("x:two"), "b:second"));
        let o = run(&["+map", "greybox", "+minigame", "x:two"]);
        assert_eq!(o.map, "greybox", "+map over the pool's first");
        assert!(parse_with(["+minigame".to_string(), "x:none".to_string()], Some(&hook)).is_err());
    }
}

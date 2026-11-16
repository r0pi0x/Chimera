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
//!   another in the background (its data and what its spawn computes from
//!   the data alone, `map::prepare`) while the game plays on, then brings
//!   it in over frames as a front end does (`finish_level_change`: the
//!   old map out and the new one spawned a step a frame, `SPAWN_BUDGET`,
//!   its colliders in the frames after the map's, the simulation paused
//!   until the last is in, so no tick sees part of a map and none is
//!   owed after), a fresh match on it (`gameplay::new_game`) and every
//!   player moved there. What needs the map's physics then
//!   (`map::MapHooks::settle`: the bots' navigation mesh on a task, the
//!   spawn spots a slice a frame) is done while it plays, and the
//!   server's map isn't ready until then (`map::LocalMapLoad`: a
//!   minigame's next match and its lobby wait for it). No frame of a
//!   change is long (docs/performance.md, "A dedicated server's map
//!   change"), so the server's clock loses no time to it and its clients'
//!   clocks need no jump. Floods are never whole in a tick
//!   (`nav_gen::SliceFloods`): the start map settles the same way from
//!   the server's first frames, its bots standing and its players
//!   spawning at the map's spawn points until theirs are made.
//!   Clients load the map from their own install by name.
//! - Players join with the game's app (`+connect <ip>:<port>`); a server
//!   takes eight unless told otherwise (`maxplayers`; `ServerHooks::
//!   maxplayers`).
//! - `run`: at most 300 frames a second (`FPS_MAX`), schedules on one
//!   thread each (`single_threaded_schedules`; `CHIMERA_EXECUTOR=multi`
//!   keeps Bevy's), the console on stdin and its output on stdout,
//!   Ctrl+C as `quit`; either ends the process at once after what must be
//!   done (clients told, the UPnP mapping removed, the session's log
//!   flushed: `chimera_core::shutdown`). `tick_stats` prints the last
//!   ticks' wall and CPU ms and instructions (`metrics::TickMetrics`, when
//!   the game's sim adds it) and the process's resident memory, for
//!   watching a live server.
//!   Every `HEALTH_SECS` (20) a `health ...` line (memory, the ticks'
//!   cost, players, the map) and a `tick_stats ...` line are printed and
//!   kept in the session's log (`ServerHealthPlugin`; `run` starts the
//!   session: `<data_local>/chimera/logs/server-<time>.log`, its panic
//!   hook, a dead last session's log named on stderr).
//!   20 s after each map change the allocator's free pages go back to the
//!   system (`metrics::memory::release_after_change`; glibc kept 1.3-1.8 GB
//!   of them on Ultimate Gun Game's server, more each map).
//! - No player ragdolls (`map::ragdoll::RagdollSettings::enabled` 0): each
//!   client makes its own from the death it is told (specs/cs_source/
//!   ragdolls.md 2.1: the server never simulates them), so the server
//!   keeps no pose record for them either (`ragdoll::record_poses`).
//!   Map-placed ragdolls are the server's still (`net::ragdolls`).
//! - What only some servers have, `ServerHooks` (all off in
//!   `ServerHooks::NONE`): several games' maps (`maps`: a loader of whole
//!   ids, the game a bare name is); the map cycle (`map_cycle`:
//!   `core::MapCycle` follows the map loaded, its maps the install's,
//!   `GameInfo::map_cycle`); the map's file for clients without it
//!   (`map_files`: `net::maps::MapFiles` from `GameInfo::map_file` in the
//!   mounted install); the greybox's own tick (`greybox_tick`); a minigame
//!   (`minigame`: `+minigame <id>`, its pool's first map without `+map`,
//!   started by the console line `minigame <id>` after the command line's
//!   others, restarted on a map change, named in the banner; a host's
//!   `changelevel` while it plays is its next match there, with the
//!   lobby: `net::host_changelevel`); what the
//!   machine lacks for a map of the installs, told before it loads
//!   (`requirements`); what the binary adds that the tests don't
//!   (`launch`); the games whose files it reads (`games`, else the
//!   game's alone).
//! - Game files without an install (`net::server_assets`): `run` fetches,
//!   before the server is built, each game's startup sets and the start
//!   map's from the asset service with the server's own credential (a hub
//!   account redeemed once with `+chimera:hub <invite link> [username]`,
//!   taken off the console lines, or a token); the first match's sets
//!   come before it listens, a `changelevel`'s before its map loads.
//!
//! Known shortcuts: a server without `map_files` offers no map file (a
//! client without the map can't download it: maps load from each
//! player's install); without `greybox_tick`, changing to the greybox
//! keeps the game's tick (the greybox has no tick of its own); a
//! minigame's map pool isn't rotated by the server (a map change keeps
//! the minigame, its match starts over); the start map is spawned whole
//! before the server listens, and a change's settling is out of time
//! after `SETTLE_MAX_SECS` of wall time (its floods go on in play).

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
    /// The games whose files the server reads (None: `GameServer::info`'s
    /// alone): those without an install come from the asset service
    /// (`net::server_assets`).
    pub games: Option<fn() -> Vec<&'static GameInfo>>,
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
        games: None,
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
        if net::is_code_map(&id) || id.contains(':') {
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
            Some(check) if !net::is_code_map(&self.map_id(id)) => check(),
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
                let id = greybox::select(world, id);
                // Its kit first, when this server's maps dress code-built
                // maps (`map::sources::CODE_MAP_KIT`), then the map.
                if let Ok(kit) = self.load_map_in(&greybox::kit_id(id), &map::LoadContext::of(world)) {
                    map::change_map(world, kit, map::MapDebugView::Normal);
                }
                greybox::respawn(world);
                world.remove_resource::<map::ActiveMapLook>();
                world.insert_resource(LoadedMapName(id.into()));
                self.hooks.greybox_tick
            }
        };
        self.new_match(world, greybox_tick);
    }

    /// A fresh match on the map now in the world (`swap_map`'s, and a
    /// `changelevel`'s once its map is all in): the gameplay's tick
    /// (`greybox_tick` on the greybox), `gameplay::new_game`, the
    /// minigame's match over again.
    fn new_match(&self, world: &mut World, greybox_tick: Option<f64>) {
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
        if net::is_code_map(&id) {
            // Its kit, when this server's maps dress code-built maps
            // (`map::sources::CODE_MAP_KIT`): loaded as the start map,
            // the code-built map spawned beside it (`ServerPlugin`).
            let context = map::LoadContext::default();
            return Ok(match server.load_map_in(&greybox::kit_id(&id), &context) {
                Ok(data) => Self::Loaded { id, data, context },
                Err(_) => Self::Greybox,
            });
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
    /// The code-built map a `StartMap::Greybox` start spawns (`+map range`:
    /// the range; `greybox::CodeMap`).
    pub code_map: &'static str,
}

impl ServerPlugin {
    pub fn new(server: &'static GameServer, map: StartMap, options: &Options) -> Self {
        Self {
            server,
            map: std::sync::Mutex::new(Some(map)),
            console: options.console.clone(),
            code_map: greybox::code_map(&options.map).unwrap_or(greybox::GREYBOX),
        }
    }
}

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        let start = self.map.lock().unwrap().take();
        // Big blocks go back to the system when freed (a map change's).
        chimera_core::metrics::memory::big_blocks_mapped();
        if let Some(StartMap::Loaded { context, .. }) = &start {
            app.insert_resource(context.clone());
        }
        let server = self.server;
        let hooks = &server.hooks;
        app.insert_resource(Serving(server));
        // No player ragdolls: they are each client's own (ragdolls.md 2.1,
        // the server never simulates them; `net::weapons`' `Killed` seeds
        // them), so neither are their bodies' pose records
        // (`map::ragdoll::record_poses`).
        app.insert_resource(map::ragdoll::RagdollSettings {
            enabled: 0,
            ..default()
        });
        (server.sim)(app);
        let on_greybox = match &start {
            Some(StartMap::Loaded { id, .. }) => net::is_code_map(id),
            _ => true,
        };
        match start {
            // A code-built map's kit: the map spawned beside it.
            Some(StartMap::Loaded { id, data, .. }) if net::is_code_map(&id) => {
                app.insert_resource(greybox::CodeMap(greybox::code_map(&id).unwrap_or(greybox::GREYBOX)))
                    .add_plugins((GreyboxMapPlugin, MapSimPlugin::new(data)))
                    .insert_resource(LoadedMapName(id));
            }
            Some(StartMap::Loaded { id, data, .. }) => {
                app.add_plugins(MapSimPlugin::new(data))
                    .insert_resource(LoadedMapName(id));
            }
            // The map systems too: `changelevel` loads one later.
            _ => {
                app.insert_resource(greybox::CodeMap(self.code_map))
                    .add_plugins((GreyboxMapPlugin, MapSimPlugin::empty()));
                if self.code_map != greybox::GREYBOX {
                    app.insert_resource(LoadedMapName(self.code_map.into()));
                }
            }
        }
        // The game is the gameplay: its rules and tick.
        app.set_gameplay(Gameplay::of(server.info));
        if let Some(secs) = hooks.greybox_tick.filter(|_| on_greybox) {
            app.insert_resource(Time::<Fixed>::from_duration(Duration::from_secs_f64(secs)));
        }
        app.insert_resource(BuildVersion(server.version)).add_plugins(NetPlugin);
        // The server lists it under the app's own name until `hostname` says otherwise.
        app.insert_resource(net::query::Hosting {
            hostname: server.title.to_string(),
            ..default()
        });
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
        // Floods of the map's physics (the bots' navigation mesh, the spawn
        // spots) on a task or a slice a tick, never whole in one; the start
        // map's from its first frames (a slice a frame, as a change's).
        if !at_once() {
            app.init_resource::<map::nav_gen::SliceFloods>();
            if !on_greybox {
                app.insert_resource(LevelSettle {
                    tick: 0,
                    started: std::time::Instant::now(),
                });
            }
        }
        let maps = match &hooks.maps {
            Some(m) => m.help.to_string(),
            None => format!("greybox, range or {}:<name>", server.info.id),
        };
        app.insert_resource(NetSettings {
            maxplayers: hooks.maxplayers,
            ..default()
        })
        .add_plugins(ServerHealthPlugin)
        .add_systems(Update, finish_level_change)
        // glibc's free pages back a while after each map change.
        .add_systems(Update, chimera_core::metrics::memory::release_after_change::<map::MapFile>)
        .console_command(
            "changelevel",
            &format!(
                "changelevel <map>: move the server and its players to another map ({maps}); in a party app, the \
                 minigame's next match there (its lobby); with no map, its next match as a rotation's."
            ),
            |w, a| match a.first() {
                Some(m) => change_level(w, m),
                // A party app's minigame: its next match, the map drawn
                // as a rotation's (for the players present).
                None => net::host_next_level(w).unwrap_or(Err("changelevel <map>".into())),
            },
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

/// Seconds between a dedicated server's health lines.
pub const HEALTH_SECS: f32 = 20.0;

/// The server's health lines (`chimera_core::diagnostics`): every
/// `HEALTH_SECS` a `health ...` line (memory, the ticks' wall time and
/// instructions, players, worst ping and loss, the map) and a
/// `tick_stats ...` line (`tick_stats` on one line), printed on the
/// console (stdout: the server's log) and into the session's log.
pub struct ServerHealthPlugin;

impl Plugin for ServerHealthPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<chimera_core::diagnostics::DiagnosticsPlugin>() {
            app.add_plugins(chimera_core::diagnostics::DiagnosticsPlugin {
                health: chimera_core::diagnostics::HealthSettings { interval: HEALTH_SECS, print: true },
            });
        }
        chimera_core::diagnostics::add_health_part(app, server_health);
        chimera_core::diagnostics::add_health_extra(app, |w| Some(format!("tick_stats {}", tick_stats(w).replace('\n', " | "))));
        chimera_core::diagnostics::add_context_part(app, |w| {
            let map = w.get_resource::<LoadedMapName>().map_or("greybox".to_string(), |m| m.0.clone());
            let minigame = w
                .get_resource::<chimera_sim::minigame::ActiveMinigame>()
                .map_or(String::new(), |m| format!("minigame: {}\n", m.def.id));
            format!("map: {map}\n{minigame}")
        });
    }
}

fn server_health(world: &mut World, h: &mut chimera_core::diagnostics::HealthLine) {
    if world.get_resource::<chimera_core::core::NetRole>().is_some_and(|r| *r == chimera_core::core::NetRole::Server) {
        let players = net::server::players(world);
        h.push("players", players.len());
        if let Some(ping) = players.iter().filter_map(|p| p.ping_ms).reduce(f64::max) {
            h.push("ping_max", format!("{ping:.0}ms"));
        }
        if let Some(loss) = players.iter().map(|p| p.loss).reduce(f64::max) {
            h.push("loss_max", format!("{:.1}%", loss * 100.0));
        }
    }
    let map = world.get_resource::<LoadedMapName>().map_or("greybox".to_string(), |m| m.0.clone());
    h.push("map", map);
    h.push("entities", world.entities().count_spawned());
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

/// A map loading in the background for `changelevel`: its data, and the
/// parts of its spawn that need only the data (`map::prepare`: the
/// colliders' shapes, the props' brushes).
#[derive(Resource)]
struct LevelLoad {
    id: String,
    task: bevy::tasks::Task<Result<(MapData, map::prepare::Prepared), String>>,
}

/// The loaded map coming into the world a step a frame
/// (`map::prepare::begin_map_change`, `SPAWN_BUDGET`; its colliders after
/// the map's frame), the simulation waiting (`Time<Virtual>` paused): no
/// tick sees part of a map.
#[derive(Resource)]
struct LevelSpawn {
    id: String,
    summary: String,
    /// `Time<Virtual>` was paused already.
    was_paused: bool,
}

/// The map is in and plays; what needs its physics (`map::MapHooks::
/// settle`: the bots' navigation mesh on a task, the spawn spots a slice
/// a frame) is under way, and after a `changelevel` the server's map
/// isn't ready until it is done (`map::LocalMapLoad`: a minigame's next
/// match waits for it). The start map's too, from the first frames.
#[derive(Resource)]
struct LevelSettle {
    /// The simulation's tick when the map came in.
    tick: u64,
    started: std::time::Instant,
}

/// What of a frame a map's spawn takes while it comes in (a server's
/// frame is 3.3 ms, `FPS_MAX`): the network is still answered a hundred
/// times a second meanwhile.
pub const SPAWN_BUDGET: Duration = Duration::from_millis(8);

/// Ticks a spawned map runs before its settling starts: the collision is
/// in the spatial queries' trees after the first physics steps.
const SETTLE_AFTER_TICKS: u64 = 3;

/// The longest the settling holds the server's map back; a flood not
/// done by then goes on in play (`nav_gen::SliceFloods`).
const SETTLE_MAX_SECS: f32 = 30.0;

/// `CHIMERA_SPAWN_AT_ONCE=1`: a `changelevel`'s map comes in whole in one
/// frame and floods run whole in a tick (no `nav_gen::SliceFloods`, no
/// settling), as before the change was spread: the A/B for measuring
/// (`heavy::server_map_change`).
fn at_once() -> bool {
    chimera_core::env::var("CHIMERA_SPAWN_AT_ONCE").is_ok_and(|v| v == "1")
}

/// Whether a `changelevel`'s map is on its way into the world or settling
/// there (not while it only loads: the game plays on then).
pub fn level_coming_in(world: &World) -> bool {
    world.contains_resource::<LevelSpawn>() || world.contains_resource::<LevelSettle>()
}

/// A spawn or settling under way is called off (another map comes): the
/// simulation runs again.
fn call_off_level(world: &mut World) {
    if let Some(spawn) = world.remove_resource::<LevelSpawn>()
        && !spawn.was_paused
        && let Some(mut time) = world.get_resource_mut::<Time<Virtual>>()
    {
        time.unpause();
    }
    world.remove_resource::<LevelSettle>();
    world.remove_resource::<map::LocalMapLoad>();
}

/// `changelevel <map>`: the greybox at once; a map of the game loaded in
/// the background (the game plays on), then brought in over the next
/// frames (`finish_level_change`).
pub fn change_level(world: &mut World, name: &str) -> Result<Option<String>, String> {
    let server = world.resource::<Serving>().0;
    let id = server.map_id(name);
    // A party app's minigame: a new match there (its lobby), not only the map.
    if let Some(r) = net::host_changelevel(world, &id) {
        return r;
    }
    if net::is_code_map(&id) {
        swap_map(world, &id, None);
        return Ok(Some(format!("changed level to the {id}")));
    }
    if world.contains_resource::<LevelLoad>() || world.contains_resource::<LevelSpawn>() {
        return Err("a map is loading already".into());
    }
    // Clients hear of it now (`net::maps`: the map's name changes first).
    world.insert_resource(LoadedMapName(id.clone()));
    let context = map::LoadContext::of(world);
    // The spawn's own work on the data is done there too (`map::prepare`).
    let prepare = map::prepare::preparation(world);
    // The match's game files the server reads from the asset store come
    // first (`net::server_assets`): none fetched while it plays.
    let files = net::server_assets::fetch_now(world);
    let task = bevy::tasks::AsyncComputeTaskPool::get().spawn({
        let id = id.clone();
        async move {
            if let Some(job) = files {
                let _ = job.wait();
            }
            let data = server.load_map_in(&id, &context)?;
            let prepared = prepare.run(&data);
            Ok((data, prepared))
        }
    });
    world.insert_resource(LevelLoad { id: id.clone(), task });
    Ok(Some(format!("changing level to {id}...")))
}

/// `GameServer::swap_map` for the server this app serves (`Serving`): in
/// this frame, whole (a test's; `changelevel` spreads it).
pub fn swap_map(world: &mut World, id: &str, data: Option<MapData>) {
    call_off_level(world);
    world.resource::<Serving>().0.swap_map(world, id, data);
}

/// A `changelevel`'s map, each frame: its load polled; once loaded, the
/// old map out and the new one in a step a frame, the simulation waiting
/// (`LevelSpawn`); then a fresh match on it while it settles
/// (`LevelSettle`). docs/performance.md, "A dedicated server's map
/// change".
fn finish_level_change(world: &mut World) {
    if world.contains_resource::<LevelSpawn>() {
        spawn_level_steps(world, SPAWN_BUDGET);
        return;
    }
    let wait = chimera_core::tasks::waits(world);
    let result = world
        .get_resource_mut::<LevelLoad>()
        .and_then(|mut load| chimera_core::tasks::poll(&mut load.task, wait));
    let Some(result) = result else {
        // No map to bring in: the one that plays settles on.
        if world.contains_resource::<LevelSettle>() {
            settle_level(world);
        }
        return;
    };
    let Some(LevelLoad { id, .. }) = world.remove_resource::<LevelLoad>() else {
        return;
    };
    match result {
        Ok((data, prepared)) => {
            let summary = format!("loaded {id}: {} spawns", data.spawns.len());
            // The map left is done settling.
            call_off_level(world);
            greybox::unload(world);
            // The server's map isn't ready until it has settled.
            world.insert_resource(map::LocalMapLoad { map: id.clone() });
            map::prepare::begin_map_change(world, data, prepared, map::MapDebugView::Normal);
            let was_paused = world.get_resource::<Time<Virtual>>().is_none_or(|t| t.is_paused());
            if let Some(mut time) = world.get_resource_mut::<Time<Virtual>>() {
                time.pause();
            }
            world.insert_resource(LevelSpawn { id, summary, was_paused });
            // `CHIMERA_SPAWN_AT_ONCE=1`: every step, the colliders and the
            // new match in this frame, as before the change was spread
            // (an A/B for measuring).
            if at_once() {
                while world.contains_resource::<LevelSpawn>() {
                    spawn_level_steps(world, Duration::MAX);
                }
            }
            // The old map went in this frame: the new one's steps from the
            // next.
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

/// A frame of `LevelSpawn`: the map's next steps; once it is all in, the
/// simulation runs again, with a fresh match on it, while it settles.
fn spawn_level_steps(world: &mut World, budget: Duration) {
    if !map::prepare::spawn_some(world, budget) {
        return;
    }
    let Some(LevelSpawn { id, summary, was_paused }) = world.remove_resource::<LevelSpawn>() else {
        return;
    };
    if !was_paused && let Some(mut time) = world.get_resource_mut::<Time<Virtual>>() {
        time.unpause();
    }
    world.insert_resource(LoadedMapName(id));
    world.resource::<Serving>().0.new_match(world, None);
    world.resource_mut::<Console>().info(summary);
    if at_once() {
        world.remove_resource::<map::LocalMapLoad>();
        return;
    }
    let tick = world.get_resource::<core::SimClock>().map_or(0, |c| c.tick);
    world.insert_resource(LevelSettle {
        tick,
        started: std::time::Instant::now(),
    });
}

/// A frame of `LevelSettle`: every hook's slice (`map::MapHooks::settle`),
/// and the server's map is ready when they are all done.
fn settle_level(world: &mut World) {
    let settle = world.resource::<LevelSettle>();
    let (from, started) = (settle.tick, settle.started);
    let tick = world.get_resource::<core::SimClock>().map_or(from + SETTLE_AFTER_TICKS, |c| c.tick);
    if tick < from + SETTLE_AFTER_TICKS {
        return;
    }
    let hooks: Vec<fn(&mut World) -> bool> =
        world.get_resource::<map::MapHooks>().map(|h| h.settle.clone()).unwrap_or_default();
    let mut done = true;
    for hook in hooks {
        done &= hook(world);
    }
    if !done && started.elapsed().as_secs_f32() < SETTLE_MAX_SECS {
        return;
    }
    info!("map settled in {:.2} s", started.elapsed().as_secs_f32());
    world.remove_resource::<LevelSettle>();
    world.remove_resource::<map::LocalMapLoad>();
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
    // A hub account for the server's own game files (`+chimera:hub <invite
    // link> [username]`, once): redeemed now, never a console line.
    let options = take_hub_link(options);
    // The service first whenever one is set (a hub account included), as on a client, unless
    // `+chimera:asset_use_install 1` (or `[assets] use_install`) chooses
    // the installs; before anything mounts a game.
    crate::net::assets::set_install_default_here(&options.console, false);
    // Its game files from the asset service where it has no install
    // (`net::server_assets`), before anything reads them.
    let files = {
        let games = server.hooks.games.map_or_else(|| vec![server.info], |g| g());
        let cvar = |name: &str| {
            options.console.iter().rev().find_map(|l| {
                let words = console::parse(l).into_iter().next().unwrap_or_default();
                words.first().filter(|w| w.eq_ignore_ascii_case(name))?;
                words.get(1).cloned()
            })
        };
        let url = cvar(net::assets::URL_CVAR).unwrap_or_default();
        let fingerprint = cvar(net::assets::FINGERPRINT_CVAR).unwrap_or_default();
        net::server_assets::startup(&games, &url, &fingerprint, &server.map_id(&options.map))
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
    // The session's log (`<data_local>/chimera/logs/server-<time>.log`),
    // the crash hooks; a server that died last time is told.
    let session = chimera_core::diagnostics::data_root().and_then(|root| {
        chimera_core::diagnostics::start(chimera_core::diagnostics::SessionOptions {
            root,
            prefix: "server".into(),
            app: std::env::current_exe()
                .ok()
                .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .unwrap_or_else(|| format!("{}_server", server.info.id)),
            build: server.version.into(),
            net_version: net::NET_VERSION.into(),
        })
    });
    let mut app = self::app(server, start, &options);
    if let Some(files) = files {
        app.insert_resource(files);
    }
    if let Some((session, previous)) = session {
        eprintln!("session log: {}", session.log_path().display());
        for p in previous {
            eprintln!(
                "the last server session ({} {}) ended without a clean exit: its log is {}{}",
                p.info.app,
                p.info.build,
                p.log_path().map_or("gone".into(), |l| l.display().to_string()),
                p.held().map_or(String::new(), |h| format!(", its crash report {}", h.display()))
            );
            // Kept for the host to read; the marker goes.
            let _ = std::fs::remove_file(&p.marker);
        }
        app.insert_resource(chimera_core::diagnostics::SessionLog(session));
    }
    // `quit` and Ctrl+C end the process once the steps that persist have
    // run (`chimera_core::shutdown`; `+chimera:clean_exit 1` drops the app).
    app.add_plugins((
        bevy::log::LogPlugin { custom_layer: chimera_core::diagnostics::log_layer, ..Default::default() },
        bevy::app::TerminalCtrlCHandlerPlugin,
        chimera_core::shutdown::ShutdownPlugin,
    ))
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

/// `+chimera:hub <link> [username]` taken off `options`' console lines
/// and redeemed (`net::assets::join_hub`): the server's hub account, its
/// file the server user's `account.toml` (`CHIMERA_ACCOUNT` names
/// another), whose tickets fetch its game files from then on.
fn take_hub_link(mut options: Options) -> Options {
    let command = net::assets::HUB_COMMAND;
    options.console.retain(|line| {
        let words = console::parse(line).into_iter().next().unwrap_or_default();
        if !words.first().is_some_and(|w| w.eq_ignore_ascii_case(command)) {
            return true;
        }
        match words.get(1) {
            Some(link) => match net::assets::join_hub(link, words.get(2).map(String::as_str)) {
                Ok(said) => eprintln!("{said}"),
                Err(e) => eprintln!("error: {command}: {e}"),
            },
            None => eprintln!("usage: +{command} <hub link with ?code=> [username]"),
        }
        false
    });
    options
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
    // The first match's game files (`net::server_assets`), before anyone
    // can join.
    if let Some(job) = net::server_assets::fetch_now(world) {
        let _ = job.wait_saying(net::server_assets::PROGRESS_EVERY);
    }
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

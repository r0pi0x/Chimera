//! The HL2:DM dedicated server (apps/hl2dm_server, crate `hl2dm_server`):
//! HL2:DM's simulation (`hl2dm::SimPlugins` + `Hl2dmPlugins`) and the
//! network with no window, render or audio, as `combat_arms_server` is
//! Combat Arms'. The binary (main.rs) adds the run loop, the log, the
//! console on stdin and its output on stdout; what it serves is
//! `ServerPlugin`, which the tests build the same way (tests/it/).
//!
//!   hl2dm_server [-port <n>] [+map <name>] [+<command> ...]
//!
//! - Maps: `+map greybox` (the default) or `+map hl2dm:<name>` (a bare
//!   name works too) from the install (`hl2dm::game::maps`), converted
//!   lean (no textures); `changelevel <map>` (or `map <map>`) loads
//!   another in the background and moves every player there, a fresh
//!   match (`gameplay::new_game`). Clients load the map from their own
//!   install by name.
//! - Players join with the HL2:DM app's `+connect <ip>:<port>`; a server
//!   takes eight unless told otherwise (`maxplayers`). M1: no rules yet
//!   (deathmatch and team deathmatch are M7), no bots.
//!
//! Lifted nothing: the pieces are Chimera's (`net`, `map::MapSimPlugin`,
//! the headless harness's `add_headless_with`, `single_threaded_schedules`)
//! and HL2:DM's (`game::*`); Combat Arms' server's main.rs has the same
//! shape (its options, stdin console and run loop).
//!
//! Known shortcuts: no map cycle; no `MapFiles` (a client without the map
//! can't download it: maps load from each player's install); changing
//! to the greybox keeps HL2:DM's 15 ms tick (the greybox has no tick of
//! its own).

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{
    console::{self, Console, ConsoleAppExt},
    core::{self, BuildVersion},
    gameplay::{self, Gameplay, GameplayAppExt},
};
use chimera_net::net::{self, NetPlugin, NetSettings};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    map::{self, LoadedMapName, MapData, MapSimPlugin},
};
use hl2dm::game::{GAME, Hl2dmPlugins, INFO, TICK_INTERVAL, maps};

/// This build's version (the banner, `version`): stamped by build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

pub const USAGE: &str = "\
usage: hl2dm_server [-port <n>] [+map <name>] [+<command> [args...]]
  -port <n>        UDP port (hostport; default 27015)
  +map <name>      greybox (default) or an HL2:DM map from the install, e.g. hl2dm:dm_lockdown
  +<command>       any console command or cvar, e.g. +maxplayers 8
Type console commands on stdin: status, changelevel <map>, quit.";

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

/// A map's id as the server serves it: `greybox`, or `hl2dm:<name>` (a
/// bare name is HL2:DM's).
pub fn map_id(name: &str) -> String {
    let id = net::normalize_map(name);
    if id == net::GREYBOX || id.contains(':') {
        id
    } else {
        format!("{GAME}:{id}")
    }
}

/// Map `id` from the install (`hl2dm:<name>`).
pub fn load_map(id: &str) -> Result<MapData, String> {
    load_map_in(id, &map::LoadContext::default())
}

/// `load_map` for the app whose context is `context` (`map::LoadContext`).
pub fn load_map_in(id: &str, context: &map::LoadContext) -> Result<MapData, String> {
    match id.split_once(':') {
        Some((game, _)) if game != GAME => Err(format!("{id}: not an HL2:DM map ({GAME}:<name>)")),
        // A server draws nothing: no textures.
        Some((_, name)) => maps::load_installed_map(name, &map::MapLoadOptions::lean().in_context(context)),
        None => maps::load_installed_map(id, &map::MapLoadOptions::lean().in_context(context)),
    }
}

/// The map a server starts on: the greybox, or one loaded from the install.
pub enum StartMap {
    Greybox,
    /// `context`: the context it was loaded in (`map::LoadContext`), the
    /// server app's from then on, so the install data it read stays held.
    Loaded { id: String, data: MapData, context: map::LoadContext },
}

impl StartMap {
    /// `options.map`, loaded.
    pub fn load(options: &Options) -> Result<Self, String> {
        let id = map_id(&options.map);
        if id == net::GREYBOX {
            return Ok(Self::Greybox);
        }
        let context = map::LoadContext::default();
        let data = load_map_in(&id, &context).map_err(|e| format!("+map {id}: {e}"))?;
        Ok(Self::Loaded { id, data, context })
    }
}

/// Everything the server runs above the headless base (the binary's
/// `MinimalPlugins` with its run loop, a test's `Sim`): HL2:DM's
/// simulation and parts, the map's systems alone (`MapSimPlugin`: nothing
/// drawn or played) with the start map, the network, the console's
/// commands (`changelevel`, `map`, `quit`) and the command line's
/// console lines, submitted to run on the first frame. Listening is
/// `listen`'s, once those lines ran (they set the port and the player
/// count).
pub struct ServerPlugin {
    pub map: std::sync::Mutex<Option<StartMap>>,
    pub console: Vec<String>,
}

impl ServerPlugin {
    pub fn new(map: StartMap, options: &Options) -> Self {
        Self {
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
        app.add_plugins(hl2dm::SimPlugins);
        match start {
            Some(StartMap::Loaded { id, data, .. }) => {
                app.add_plugins(MapSimPlugin::new(data)).insert_resource(LoadedMapName(id));
            }
            // The map systems too: `changelevel` loads one later.
            _ => {
                app.add_plugins((GreyboxMapPlugin, MapSimPlugin::empty()));
            }
        }
        // HL2:DM is the gameplay: its rules and tick.
        app.set_gameplay(Gameplay::of(&INFO))
            .insert_resource(BuildVersion(VERSION))
            .add_plugins((NetPlugin, Hl2dmPlugins))
            // A dedicated server takes eight players unless told otherwise.
            .insert_resource(NetSettings {
                maxplayers: 8,
                ..default()
            })
            .add_systems(Update, finish_level_change)
            .console_command(
                "changelevel",
                "changelevel <map>: move the server and its players to another map (greybox or hl2dm:<name>).",
                |w, a| change_level(w, a.first().ok_or("changelevel <map>")?),
            )
            .console_command("map", "map <map>: the same as changelevel on a dedicated server.", |w, a| {
                change_level(w, a.first().ok_or("map <map>")?)
            })
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
    world.resource_mut::<Console>().info(format!(
        "Chimera HL2:DM dedicated server {VERSION} ({}): {map} on UDP {addr}, {max} players",
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

/// `changelevel <map>`: the greybox at once; an HL2:DM map loaded in
/// the background (the game plays on), then swapped in
/// (`finish_level_change`).
pub fn change_level(world: &mut World, name: &str) -> Result<Option<String>, String> {
    let id = map_id(name);
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
        async move { load_map_in(&id, &context) }
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
    // At the gameplay's tick (HL2:DM's).
    let tick = world
        .get_resource::<Gameplay>()
        .map_or(Duration::from_secs_f64(TICK_INTERVAL), |g| g.tick);
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
            let back = world
                .get_resource::<map::MapFile>()
                .map(|f| map_id(&f.name))
                .unwrap_or_else(|| net::GREYBOX.into());
            world.insert_resource(LoadedMapName(back));
            world
                .resource_mut::<Console>()
                .print(console::Level::Error, format!("changelevel {id}: {e}"));
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
        let o = run(&["+map", "dm_lockdown", "+maxplayers", "4", "-port", "27040"]);
        assert_eq!(o.map, "dm_lockdown");
        assert_eq!(o.console, ["hostport 27040", "maxplayers 4"]);
        assert_eq!(run(&[]).map, "greybox");
        assert!(parse(["nope".to_string()]).is_err());
        assert_eq!(parse(["--help".to_string()]).unwrap(), Parsed::Help);
    }

    #[test]
    fn map_ids_are_hl2dms() {
        assert_eq!(map_id("dm_lockdown"), "hl2dm:dm_lockdown");
        assert_eq!(map_id("hl2dm:dm_lockdown"), "hl2dm:dm_lockdown");
        assert_eq!(map_id("chimera:greybox"), "greybox");
        assert!(load_map("cs_source:de_dust2").is_err());
    }

    #[test]
    fn the_build_is_stamped() {
        assert!(!VERSION.is_empty());
    }
}

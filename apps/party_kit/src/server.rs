//! The party's dedicated server (the `server` module of apps/party_kit;
//! binaries apps/lucker_party_server and apps/ultimate_gun_game_server):
//! the party's simulation (`PartyPartsPlugin` on Combat Arms' simulation)
//! and the network playing a minigame, with no window, render or audio,
//! on Chimera's dedicated server (`chimera_net::dedicated`: the options,
//! the start map, `ServerPlugin`, `changelevel`, the run loop and the
//! console on stdin), as every game's server is. What is the party's is
//! `PARTY` (Lucker Party's `SERVER`; another party app's server is
//! `GameServer { usage, hooks: .., ..PARTY }`, apps/ultimate_gun_game_server);
//! the binaries run it, and the tests build its `ServerPlugin` the same way
//! (tests/server/).
//!
//!   <server> [-port <n>] [+minigame <id>] [+map <name>] [+<command> ...]
//!
//! - Minigames (`ServerHooks::minigame`): `+minigame <id>`
//!   (`originals:gun_game_dust2` without one), started once the command
//!   line's console lines run; the map is its pool's first unless `+map`
//!   names one. `minigames`, `minigame <id>`, `minigame_status`,
//!   `minigame_restart` on the console; `change_match <id> [map]` plays
//!   another minigame with the players connected (`chimera_net::
//!   minigame_net`: its map, else this one if in its pool, else its
//!   pool's first, by `changelevel`).
//! - Maps of several games (`ServerHooks::maps`): `greybox`,
//!   `cs_source:<name>`, `combat_arms:<name>` (a bare name is CS:S's),
//!   loaded without textures (`crate::load_map_with`); `changelevel <map>`
//!   (or `map <map>`) loads another in the background and moves every
//!   player there, a fresh match (the minigame's starts over).
//! - Before a map of the installs loads, what this machine lacks is told
//!   (`crate::requirements`), and a lack that stops the game stops the
//!   server (`ServerHooks::requirements`).
//! - Joining: a client's mount doctor report is checked against the match
//!   manifest (`net::manifest`): one lacking the map's game is refused,
//!   told which.
//! - Bots: Combat Arms' (`MaxPlayer`, `BotLevel`, `ca_bot_add`), as the
//!   minigame's `bots` slot names them.
//! - Players join with the `lucker_party` client's `+connect <ip>:<port>`;
//!   a server takes sixteen unless told otherwise (`maxplayers`). The
//!   binaries' ladders take a fresh seed at launch (`ServerHooks::launch`).
//!
//! Known shortcuts: no `MapFiles` (a client loads the map from its own
//! install by id); the minigame's map pool isn't rotated; a map change
//! keeps the minigame (its match starts over).

pub use chimera_net::dedicated::{
    GameServer, Options, Parsed, ServerHooks, ServerMaps, ServerMinigame, ServerPlugin, StartMap, change_level, listen,
};
use chimera_world::map::{self, MapData};
use combat_arms::game::{CombatArmsPlugins, INFO};

use crate::{DEFAULT_MINIGAME, PartyPartsPlugin};

/// This build's version (the banner, `version`): the party's stamp.
pub const VERSION: &str = crate::VERSION;

pub const USAGE: &str = "\
usage: lucker_party_server [-port <n>] [+minigame <id>] [+map <name>] [+<command> [args...]]
  -port <n>          UDP port (hostport; default 27015)
  +minigame <id>     the minigame (default originals:gun_game_dust2); its first map unless +map
  +map <name>        greybox, cs_source:<name> or combat_arms:<name> (a bare name is CS:S's)
  +<command>         any console command or cvar, e.g. +maxplayers 8 +MaxPlayer 3 (bots)
Type console commands on stdin: status, minigames, minigame_status, minigame_restart,
change_match <minigame> [map], changelevel <map>, quit.";

/// The party's server (module doc): Combat Arms' simulation and gameplay
/// until the minigame (its console line) takes the rules slot, the
/// party's parts, several games' maps, a minigame.
pub const PARTY: GameServer = GameServer {
    info: &INFO,
    title: "Lucker Party",
    a_title: "a Lucker Party",
    usage: USAGE,
    version: VERSION,
    sim: |app| {
        app.add_plugins(combat_arms::SimPlugins);
    },
    parts: |app| {
        app.add_plugins((CombatArmsPlugins, PartyPartsPlugin));
    },
    // The base game's own; `hooks.maps` loads the party's.
    load_map: combat_arms::game::world::load_with,
    hooks: ServerHooks {
        maxplayers: 16,
        maps: Some(ServerMaps {
            bare: cs_source::game::GAME,
            load: crate::load_map_with,
            help: "greybox, cs_source:<name>, combat_arms:<name>",
        }),
        minigame: Some(ServerMinigame {
            default: DEFAULT_MINIGAME,
            first_map: crate::first_map,
        }),
        requirements: Some(requirements),
        launch: Some(launch),
        ..ServerHooks::NONE
    },
};

/// Lucker Party's dedicated server (`PARTY`).
pub static SERVER: GameServer = PARTY;

/// What this machine lacks (`crate::requirements`), told
/// (`crate::report`).
pub fn requirements() -> (String, bool) {
    crate::report(&crate::requirements())
}

/// The binaries' own: a ladder's rung extras differ every launch.
pub fn launch(app: &mut bevy::app::App) {
    app.insert_resource(crate::launch_rung_seed());
}

/// Lucker Party's server's command line (without the program's name).
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, String> {
    SERVER.parse(args)
}

/// A map's id as the server serves it: `greybox`, or `game:<name>` (a
/// bare name is CS:S's).
pub fn map_id(name: &str) -> String {
    SERVER.map_id(name)
}

/// Map `id` as a server loads it: no textures.
pub fn load_map(id: &str) -> Result<MapData, String> {
    SERVER.load_map(id)
}

/// `load_map` for the app whose context is `context` (`map::LoadContext`).
pub fn load_map_in(id: &str, context: &map::LoadContext) -> Result<MapData, String> {
    SERVER.load_map_in(id, context)
}

/// The map `id` in place of the one playing (None: the greybox), and a
/// fresh match on it, the minigame's too (`GameServer::swap_map`), in
/// any app with the party's parts (a test without `ServerPlugin`).
pub fn swap_map(world: &mut bevy::ecs::world::World, id: &str, data: Option<MapData>) {
    SERVER.swap_map(world, id, data);
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
    fn the_command_line_names_the_minigame_and_its_map() {
        let o = run(&[
            "+minigame",
            "originals:gun_game_dust2",
            "-port",
            "27040",
            "+maxplayers",
            "4",
        ]);
        assert_eq!(o.minigame.as_deref(), Some("originals:gun_game_dust2"));
        assert_eq!(o.map, "cs_source:de_dust2", "the pool's first");
        assert_eq!(
            o.console,
            ["hostport 27040", "maxplayers 4", "minigame originals:gun_game_dust2"]
        );
        let o = run(&["+map", "combat_arms:warehouse"]);
        assert_eq!(
            (o.minigame.as_deref(), o.map.as_str()),
            (Some(DEFAULT_MINIGAME), "combat_arms:warehouse")
        );
        assert!(parse(["+minigame".to_string(), "originals:nothing".to_string()]).is_err());
        assert!(parse(["nope".to_string()]).is_err());
        assert_eq!(parse(["--help".to_string()]).unwrap(), Parsed::Help);
    }

    #[test]
    fn map_ids() {
        assert_eq!(map_id("de_dust2"), "cs_source:de_dust2");
        assert_eq!(map_id("combat_arms:warehouse"), "combat_arms:warehouse");
        assert_eq!(map_id("chimera:greybox"), "greybox");
    }

    #[test]
    fn requirements_are_not_asked_on_the_greybox() {
        assert_eq!(SERVER.requirements("greybox"), (String::new(), false));
    }
}

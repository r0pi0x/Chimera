//! The HL2:DM dedicated server (apps/hl2dm_server, crate `hl2dm_server`):
//! HL2:DM's simulation (`hl2dm::SimPlugins` + `Hl2dmPlugins`) and the
//! network with no window, render or audio, on Chimera's dedicated
//! server (`chimera_net::dedicated`: the options, the start map,
//! `ServerPlugin`, `changelevel`, the run loop and the console on stdin).
//! What is HL2:DM's is `SERVER`; the binary (main.rs) runs it, and the
//! tests build its `ServerPlugin` the same way (tests/it/).
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
//!   takes eight unless told otherwise (`maxplayers`). HL2:DM's rules
//!   from the start map on (`game::rules`, M7: deathmatch, or team
//!   deathmatch with `mp_teamplay 1`; the intermission's end changes to
//!   the cycle's next map with `changelevel`); no bots.
//!
//! Known shortcuts: the map cycle is HL2:DM's shipped one (the install's
//! `mapcycle.txt` isn't read); no `MapFiles` (a client without the map
//! can't download it: maps load from each player's install); changing
//! to the greybox keeps HL2:DM's 15 ms tick (the greybox has no tick of
//! its own).

pub use chimera_net::dedicated::{
    GameServer, Options, Parsed, ServerHooks, ServerPlugin, StartMap, change_level, listen, parse, swap_map,
};
use chimera_world::map::{self, MapData};
use hl2dm::game::{Hl2dmPlugins, INFO, maps};

/// This build's version (the banner, `version`): stamped by build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

pub const USAGE: &str = "\
usage: hl2dm_server [-port <n>] [+map <name>] [+<command> [args...]]
  -port <n>        UDP port (hostport; default 27015)
  +map <name>      greybox (default) or an HL2:DM map from the install, e.g. hl2dm:dm_lockdown
  +<command>       any console command or cvar, e.g. +maxplayers 8
Type console commands on stdin: status, changelevel <map>, quit.";

/// HL2:DM's dedicated server (`chimera_net::dedicated`).
pub static SERVER: GameServer = GameServer {
    info: &INFO,
    title: "HL2:DM",
    a_title: "an HL2:DM",
    usage: USAGE,
    version: VERSION,
    sim: |app| {
        app.add_plugins(hl2dm::SimPlugins);
    },
    parts: |app| {
        app.add_plugins(Hl2dmPlugins)
            // The rules from the start map (`changelevel` starts each new
            // one: `gameplay::new_game`).
            .insert_resource(hl2dm::game::rules::AutoStart(true));
    },
    load_map: maps::load_installed_map,
    hooks: ServerHooks::NONE,
};

/// A map's id as the server serves it: `greybox`, or `hl2dm:<name>` (a
/// bare name is HL2:DM's).
pub fn map_id(name: &str) -> String {
    SERVER.map_id(name)
}

/// Map `id` from the install (`hl2dm:<name>`).
pub fn load_map(id: &str) -> Result<MapData, String> {
    SERVER.load_map(id)
}

/// `load_map` for the app whose context is `context` (`map::LoadContext`).
pub fn load_map_in(id: &str, context: &map::LoadContext) -> Result<MapData, String> {
    SERVER.load_map_in(id, context)
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

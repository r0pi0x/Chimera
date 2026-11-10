//! The Combat Arms dedicated server (apps/combat_arms_server, crate
//! `combat_arms_server`): Combat Arms' simulation (`combat_arms::
//! SimPlugins` + `CombatArmsPlugins`) and the network with no window,
//! render or audio, on Chimera's dedicated server (`chimera_net::
//! dedicated`: the options, the start map, `ServerPlugin`, `changelevel`,
//! the run loop and the console on stdin). What is Combat Arms' is
//! `SERVER`; the binary (main.rs) runs it, and the tests build its
//! `ServerPlugin` the same way (tests/it/).
//!
//!   combat_arms_server [-port <n>] [+map <name>] [+<command> ...]
//!
//! - Maps: `+map greybox` (the default) or `+map combat_arms:<name>` (a
//!   bare name works too) from the install (`combat_arms::game::world`);
//!   `changelevel <map>` (or `map <map>`) loads another in the background
//!   and moves every player there, a fresh match (`gameplay::new_game`).
//!   Clients load the map from their own install by name (Combat Arms'
//!   maps have no file hash: `game::world`, Q15), so no `net::maps::
//!   MapFiles`: nothing to download.
//! - Game modes: `ca_mode` (a room key: `TDM`, `TSV`, `TMM`, `CTF`,
//!   `FFA`, `LMS`, `BNT`, `QRT`, `SBT`, `HQM`, `SQM2`, `VTDM`, `VWRM`,
//!   `HG`, `ESL`, or `none`), `ca_goal`, `ca_restart`
//!   and the other `ca_*` settings (`combat_arms::game::rules`); TDM by
//!   default (the rules' own default).
//! - Bots: `MaxPlayer` (the quota), `BotLevel`, `ca_bot_add`,
//!   `ca_bot_kick` (`combat_arms::game::bots`).
//! - Players join with the Combat Arms app's `+connect <ip>:<port>`; a
//!   server takes eight unless told otherwise (`maxplayers`).
//!
//! Known shortcuts: no map cycle (a match that ends stays on its map, as
//! the rules leave it); no `MapFiles` (a client without the map can't
//! download it: Combat Arms' maps load from each player's install);
//! changing to the greybox keeps Combat Arms' 15 ms tick (the greybox has
//! no tick of its own).

pub use chimera_net::dedicated::{
    GameServer, Options, Parsed, ServerHooks, ServerPlugin, StartMap, change_level, listen, parse, swap_map,
};
use chimera_world::map::{self, MapData};
use combat_arms::game::{CombatArmsPlugins, INFO};

/// This build's version (the banner, `version`): stamped by build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

pub const USAGE: &str = "\
usage: combat_arms_server [-port <n>] [+map <name>] [+<command> [args...]]
  -port <n>        UDP port (hostport; default 27015)
  +map <name>      greybox (default) or a Combat Arms map from the install, e.g. combat_arms:warehouse
  +<command>       any console command or cvar, e.g. +maxplayers 8 +ca_mode TDM +ca_goal 50 +MaxPlayer 6 +BotLevel 1
Type console commands on stdin: status, ca_bot_add, ca_bot_kick, MaxPlayer <n>, ca_mode <mode>,
ca_restart, changelevel <map>, quit.";

/// Combat Arms' dedicated server (`chimera_net::dedicated`).
pub static SERVER: GameServer = GameServer {
    info: &INFO,
    title: "Combat Arms",
    a_title: "a Combat Arms",
    usage: USAGE,
    version: VERSION,
    sim: |app| {
        app.add_plugins(combat_arms::SimPlugins);
    },
    parts: |app| {
        app.add_plugins(CombatArmsPlugins);
    },
    load_map: combat_arms::game::world::load_with,
    hooks: ServerHooks::NONE,
};

/// A map's id as the server serves it: `greybox`, or `combat_arms:<name>`
/// (a bare name is Combat Arms').
pub fn map_id(name: &str) -> String {
    SERVER.map_id(name)
}

/// Map `id` from the install (`combat_arms:<name>`).
pub fn load_map(id: &str) -> Result<MapData, String> {
    SERVER.load_map(id)
}

/// `load_map` for the app whose context is `context` (`map::LoadContext`:
/// its install data).
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
        let o = run(&[
            "+map",
            "warehouse",
            "+ca_mode",
            "TDM",
            "+ca_goal",
            "2",
            "-port",
            "27040",
            "+MaxPlayer",
            "4",
        ]);
        assert_eq!(o.map, "warehouse");
        assert_eq!(o.console, ["hostport 27040", "ca_mode TDM", "ca_goal 2", "MaxPlayer 4"]);
        assert_eq!(run(&[]).map, "greybox");
        assert!(parse(["nope".to_string()]).is_err());
        assert_eq!(parse(["--help".to_string()]).unwrap(), Parsed::Help);
    }

    #[test]
    fn map_ids_are_combat_arms() {
        assert_eq!(map_id("warehouse"), "combat_arms:warehouse");
        assert_eq!(map_id("combat_arms:warehouse"), "combat_arms:warehouse");
        assert_eq!(map_id("chimera:greybox"), "greybox");
        assert!(load_map("cs_source:de_dust2").is_err());
    }

    #[test]
    fn the_build_is_stamped() {
        assert!(!VERSION.is_empty());
    }
}

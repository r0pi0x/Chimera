//! The Halo dedicated server (apps/halo_ce_server, crate `halo_ce_server`):
//! Halo's simulation (`halo_ce::SimPlugins` + `HaloPlugins`) and the
//! network with no window, render or audio, on Chimera's dedicated server
//! (`chimera_net::dedicated`: the options, the start map, `ServerPlugin`,
//! `changelevel`, the run loop and the console on stdin). What is Halo's is
//! `SERVER`; the binary (main.rs) runs it, and the tests build its
//! `ServerPlugin` the same way (tests/it/).
//!
//!   halo_ce_server [-port <n>] [+map <name>] [+<command> ...]
//!
//! - Maps: `+map greybox` (the default) or `+map halo_ce:<name>` (a bare
//!   name works too) from the disc (`halo_ce::game::maps`); until its
//!   loader is written a named map fails with the loader's message.
//! - Players join with the Halo app's `+connect <ip>:<port>`.
//!
//! Known shortcuts: no game rules yet (they come with their specs):
//! players walk on the greybox with the stand-in movement; no `MapFiles`
//! (maps load from each player's disc); the greybox keeps Halo's 30 Hz
//! tick.

pub use chimera_net::dedicated::{
    GameServer, Options, Parsed, ServerHooks, ServerPlugin, StartMap, change_level, listen, parse, swap_map,
};
use chimera_world::map::{self, MapData};
use halo_ce::game::{HaloPlugins, INFO, MAP_LOADER};

/// This build's version (the banner, `version`): stamped by build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

pub const USAGE: &str = "\
usage: halo_ce_server [-port <n>] [+map <name>] [+<command> [args...]]
  -port <n>        UDP port (hostport; default 27015)
  +map <name>      greybox (default) or a Halo map from the disc, e.g. halo_ce:hangemhigh
  +<command>       any console command or cvar, e.g. +maxplayers 8
Type console commands on stdin: status, changelevel <map>, quit.";

/// Halo's dedicated server (`chimera_net::dedicated`).
pub static SERVER: GameServer = GameServer {
    info: &INFO,
    title: "Halo",
    a_title: "a Halo",
    usage: USAGE,
    version: VERSION,
    sim: |app| {
        app.add_plugins(halo_ce::SimPlugins);
    },
    parts: |app| {
        app.add_plugins(HaloPlugins)
            .insert_resource(halo_ce::game::rules::AutoStart(true))
            .init_resource::<halo_ce::game::respawn::HaloRules>();
    },
    load_map: MAP_LOADER.load,
    hooks: ServerHooks::NONE,
};

/// A map's id as the server serves it: `greybox`, or `halo_ce:<name>` (a
/// bare name is Halo's).
pub fn map_id(name: &str) -> String {
    SERVER.map_id(name)
}

/// Map `id` from the install (`halo_ce:<name>`).
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
        let o = run(&["+map", "hangemhigh", "+maxplayers", "4", "-port", "27040"]);
        assert_eq!(o.map, "hangemhigh");
        assert_eq!(o.console, ["hostport 27040", "maxplayers 4"]);
        assert_eq!(run(&[]).map, "greybox");
        assert!(parse(["nope".to_string()]).is_err());
        assert_eq!(parse(["--help".to_string()]).unwrap(), Parsed::Help);
    }

    #[test]
    fn map_ids_are_halos() {
        assert_eq!(map_id("hangemhigh"), "halo_ce:hangemhigh");
        assert_eq!(map_id("halo_ce:hangemhigh"), "halo_ce:hangemhigh");
        assert_eq!(map_id("chimera:greybox"), "greybox");
        assert!(load_map("cs_source:de_dust2").is_err());
    }

    #[test]
    fn the_usage_names_the_game_and_the_build_is_stamped() {
        assert!(USAGE.contains("halo_ce_server") && USAGE.contains("halo_ce:"));
        assert!(!VERSION.is_empty());
    }
}

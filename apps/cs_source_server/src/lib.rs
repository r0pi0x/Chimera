//! The CS:S dedicated server (apps/cs_source_server, crate
//! `cs_source_server`), Source's srcds in spirit: CS:S's simulation
//! (`cs_source::SimPlugins` + `CsSourcePlugins` without the player and
//! view model animation) and the network with no window, render or
//! audio, on Chimera's dedicated server (`chimera_net::dedicated`: the
//! options, the start map, `ServerPlugin`, `changelevel`, the run loop
//! and the console on stdin). What is CS:S's is `SERVER`; the binary
//! (main.rs) runs it, and the tests build its `ServerPlugin` the same way
//! (tests/it/).
//!
//!   cs_source_server [-port <n>] [+map <name>] [+maxplayers <n>] [+<command> ...]
//!
//! - Maps: `+map greybox` (the default) or `+map de_dust2` (or
//!   `cs_source:de_dust2`) from the install (`cs_source::game::
//!   load_installed_map`), converted lean (no textures); `changelevel
//!   <map>` (or `map <map>`) loads another in the background and moves
//!   every player there, a fresh game. The greybox runs at the engine's
//!   default tick (64 Hz, `core::DEFAULT_TICK_HZ`), a CS:S map at CS:S's.
//! - The rules' map cycle (`rules::game_end`: the next map when a game
//!   ends) follows the map loaded; its maps from the install's map cycle
//!   (`ServerHooks::map_cycle`).
//! - Clients without the served map download it from the server
//!   (`ServerHooks::map_files`: `net::maps::MapFiles`, the map's file as
//!   the game reads it).
//! - Bots: `bot_add`, `bot_kick`, `bot_quota` (`cs_source::bot`).
//! - Players join with the game's `connect <ip>:<port>`; a server takes
//!   eight unless told otherwise (`maxplayers`).
//!
//! Released built alone: `cargo build --release -p cs_source_server`
//! (Chimera's simulation crates and `cs_source` without `ui`: no
//! chimera_present, client shell or front end links).
//!
//! Known shortcuts (docs/tech-debt.md): The dedicated server runs at most
//! 300 frames a second (`chimera_net::dedicated::FPS_MAX`, srcds's
//! `fps_max` default as documented by the community; no cvar), each
//! reading packets and running the ticks due; an idle de_dust2 server
//! still costs ~15 % of a core (its physics and map logic every tick).
//! Why it's OK for now: Was a frame every millisecond (a third of a core
//! idle).

pub use chimera_net::dedicated::{
    GameServer, Options, Parsed, ServerHooks, ServerPlugin, StartMap, change_level, listen, parse, swap_map,
};
use chimera_world::map::{self, MapData};
use cs_source::game::{CsSourcePlugins, INFO, movement, player_anim, view_anim};

/// This build's version (the banner, `version`): stamped by build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

pub const USAGE: &str = "\
usage: cs_source_server [-port <n>] [+map <name>] [+maxplayers <n>] [+<command> [args...]]
  -port <n>        UDP port (hostport; default 27015)
  +map <name>      greybox (default) or a CS:S map from the install, e.g. de_dust2
  +<command>       any console command or cvar, e.g. +maxplayers 8 +bot_quota 4
Type console commands on stdin: status, bot_add, bot_kick, bot_quota, changelevel <map>, quit.";

/// CS:S's dedicated server (`chimera_net::dedicated`).
pub static SERVER: GameServer = GameServer {
    info: &INFO,
    title: "CS:S",
    a_title: "a CS:S",
    usage: USAGE,
    version: VERSION,
    sim: |app| {
        app.add_plugins(cs_source::SimPlugins);
    },
    parts: |app| {
        use bevy::app::PluginGroup;
        // Without the player and view model animation, as always.
        app.add_plugins(
            CsSourcePlugins
                .build()
                .disable::<player_anim::PlayerAnimPlugin>()
                .disable::<view_anim::ViewAnimPlugin>(),
        )
        .insert_resource(chimera_core::slots::Loadout { movement: movement::ID });
    },
    load_map: cs_source::game::load_installed_map,
    hooks: ServerHooks {
        map_cycle: true,
        map_files: true,
        // The greybox keeps the engine's tick (it has none of its own).
        greybox_tick: Some(1.0 / chimera_core::core::DEFAULT_TICK_HZ),
        ..ServerHooks::NONE
    },
};

/// A map's id as the server serves it: `greybox`, or `cs_source:<name>`
/// (a bare name is CS:S's).
pub fn map_id(name: &str) -> String {
    SERVER.map_id(name)
}

/// Map `id` from the install (`cs_source:<name>`).
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
        let o = run(&[
            "+map",
            "de_dust2",
            "+maxplayers",
            "4",
            "-port",
            "27040",
            "+bot_quota",
            "2",
        ]);
        assert_eq!(o.map, "de_dust2");
        assert_eq!(o.console, ["hostport 27040", "maxplayers 4", "bot_quota 2"]);
        assert_eq!(run(&[]).map, "greybox");
        assert!(parse(["nope".to_string()]).is_err());
        assert_eq!(parse(["--help".to_string()]).unwrap(), Parsed::Help);
    }

    #[test]
    fn map_ids_are_css() {
        assert_eq!(map_id("de_dust2"), "cs_source:de_dust2");
        assert_eq!(map_id("cs_source:de_dust2"), "cs_source:de_dust2");
        assert_eq!(map_id("chimera:greybox"), "greybox");
        assert!(load_map("hl2dm:dm_lockdown").is_err());
    }

    #[test]
    fn the_build_is_stamped() {
        assert!(!VERSION.is_empty());
    }
}

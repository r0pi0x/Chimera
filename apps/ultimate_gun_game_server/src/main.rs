//! Ultimate Gun Game's dedicated server (apps/ultimate_gun_game_server,
//! binary `ultimate_gun_game_server`): the party's server
//! (`party_kit::server::PARTY`, on Chimera's `chimera_net::dedicated::run`)
//! with the Ultimate Gun Game's parts at launch
//! (`ultimate_gun_game::UltimateGunGamePlugin::launch`: a fresh seed,
//! printed, and a new ladder and a random map at every rotation), playing
//! the automatic gun game (`+minigame` names another) on its pool's first
//! map unless `+map` names one (`ultimate_gun_game::first_map`).
//! `ugg_new [seed] [rungs]` on the console makes a new ladder for the
//! players connected (a seed replays one); `parts` lists what each game
//! lends. Options: `--help` (`USAGE`).

use chimera_net::dedicated::{GameServer, ServerHooks, ServerMinigame};
use party_kit::server::PARTY;

const USAGE: &str = "\
usage: ultimate_gun_game_server [-port <n>] [+minigame <id>] [+map <name>] [+<command> [args...]]
  -port <n>          UDP port (hostport; default 27015)
  +minigame <id>     the minigame (default ultimate_gun_game:auto); its first map unless +map
  +map <name>        greybox, cs_source:<name> or combat_arms:<name> (a bare name is CS:S's)
  +<command>         any console command or cvar, e.g. +maxplayers 8 +MaxPlayer 3 (bots)
Type console commands on stdin: status, minigames, minigame_status, minigame_restart,
change_match <minigame> [map], ugg_new [seed] [rungs], parts, changelevel <map>, quit.";

/// The party's server playing the automatic gun game.
static SERVER: GameServer = GameServer {
    title: "Ultimate Gun Game",
    a_title: "an Ultimate Gun Game",
    usage: USAGE,
    hooks: ServerHooks {
        minigame: Some(ServerMinigame {
            default: ultimate_gun_game::MINIGAME,
            first_map: ultimate_gun_game::first_map,
        }),
        // The party's launch (a fresh rung seed) and the Ultimate Gun Game's
        // parts with a fresh seed.
        launch: Some(|app| {
            party_kit::server::launch(app);
            app.add_plugins(ultimate_gun_game::UltimateGunGamePlugin::launch());
        }),
        ..PARTY.hooks
    },
    ..PARTY
};

fn main() {
    chimera_net::dedicated::run(&SERVER);
}

//! The party's dedicated server, its game-free half: Chimera's dedicated
//! server (`chimera_net::dedicated`, re-exported) and the binaries' launch
//! (`launch`). The party's `GameServer` itself names its games, so it is
//! the roster's (apps/party_roster, `party_roster::server::PARTY`).

pub use chimera_net::dedicated::{
    GameServer, Options, Parsed, ServerHooks, ServerMaps, ServerMinigame, ServerPlugin, StartMap, change_level, listen,
};

/// This build's version (the banner, `version`): the party's stamp.
pub const VERSION: &str = crate::VERSION;

/// The binaries' own: a ladder's rung extras differ every launch, and
/// each new match waits in the pre-match lobby (`mp_lobby 1`;
/// `chimera_net::prematch`). A dedicated server has no host to press
/// Start, so its first lobby counts down once everyone is ready or after
/// the longest wait (`mp_lobby_host_start 0`; the command line may set
/// it back to 1 and start it with `lobby_start` on its console).
pub fn launch(app: &mut bevy::app::App) {
    app.insert_resource(crate::launch_rung_seed());
    chimera_net::prematch::enable(app);
    let mut s = *app.world().resource::<chimera_net::prematch::LobbySettings>();
    s.host_start = 0;
    app.insert_resource(s);
}

//! Spectators over the network (registered with `minigame_net`;
//! docs/plans/active/ugg-ui.md, "Spectators"): who may watch a minigame
//! instead of playing, and how they come and go. The simulation's side
//! (out of play and back) is `chimera_sim::minigame::spectators`.
//!
//! - **Joining as one** (`cl_spectator 1`, a client's userinfo; the party
//!   apps' Join page and `+cl_spectator 1` on the command line): the
//!   server makes the new player a spectator as it comes (`joined_watching`)
//!   while the server has room (`mp_spectators_max`); else it plays.
//! - **Switching** (`spectator [0|1]`, a player command: a client's goes
//!   to its server for its player; no argument flips it): from the lobby
//!   or the Escape menu, while the host allows it (`mp_spectate_switch 1`,
//!   the default) and, to watch, while there is room. Back to playing,
//!   the minigame takes the player in as a joiner (in the lobby it is
//!   staged with everyone).
//! - **The cap** (`mp_spectators_max`, default 8; 0: no spectators): how
//!   many may watch at once.
//! - **Their client**: the same snapshots as everyone (replication is
//!   per client, not per role), and no inputs: a spectating client sends
//!   no movement commands, only its acknowledgements
//!   (`net::predict::send_commands`); its camera is the client's
//!   (`chimera_client::client::spectate`: first person through a player's
//!   eyes, chase, free, jump cycling, attack and attack2 the next and
//!   previous player).
//! - **The rules around them**: a spectator is never staged, waited for
//!   or counted by the pre-match lobby (`prematch::serve`: not in "n of m
//!   loaded", not a player for `mp_lobby_min_players`; it does count as
//!   someone here for the longest wait), never spawns, scores or stands in
//!   the standings (`minigame::runtime::players`), and votes (the map vote
//!   and `callvote`) only with `sv_vote_allow_spectators 1`. Its client
//!   loading the map takes it into the game as a watcher, not into play
//!   (`server::receive_loaded`).
//!
//! The cvars and the command are engine names (`chimera:`,
//! `console_names`).
//!
//! Known shortcuts: a spectator refused at the join (no room) is told on
//! the server's log only; a bot never spectates.

use bevy::prelude::*;

use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{NetRole, RemotePlayer, SimSet, Spectating, UserInfo},
    minigame::{self, ActiveMinigame},
};

/// The userinfo key (and client cvar) a player asks to join as a
/// spectator with.
pub const JOIN_KEY: &str = "cl_spectator";

/// The server's spectator settings (`mp_spectators_max`,
/// `mp_spectate_switch`); replicated to clients (`mp_*`), whose menus
/// offer the switch by them.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpectatorSettings {
    /// The most spectators at once; 0: none.
    pub max: u32,
    /// 1: players may switch between playing and watching (lobby, Escape
    /// menu); 0: only joining as one.
    pub switch: u8,
}

impl Default for SpectatorSettings {
    fn default() -> Self {
        Self { max: 8, switch: 1 }
    }
}

/// A client's own wish (`cl_spectator`): join servers as a spectator.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JoinAsSpectator(pub u8);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<SpectatorSettings>().init_resource::<JoinAsSpectator>();
    resource_cvar::<SpectatorSettings, u32>(
        app,
        "mp_spectators_max",
        "Server: the most players watching at once (joining as spectators or switching to it); 0: no spectators.",
        |s| &mut s.max,
    );
    resource_cvar::<SpectatorSettings, u8>(
        app,
        "mp_spectate_switch",
        "Server: 1: players may switch between playing and spectating (the lobby, the Escape menu); 0: only joining as one.",
        |s| &mut s.switch,
    );
    resource_cvar::<JoinAsSpectator, u8>(app, JOIN_KEY, "1: join servers as a spectator (watch, don't play).", |s| &mut s.0);
    {
        let mut console = app.world_mut().resource_mut::<crate::console::Console>();
        console.userinfo(JOIN_KEY);
        console.player_command("chimera:spectator");
    }
    app.console_command(
        "spectator",
        "spectator [0|1]: watch the match instead of playing (1) or play again (0); no argument flips it. The host \
         allows it (mp_spectate_switch, mp_spectators_max).",
        |w, a| {
            let want = match a.first().map(String::as_str) {
                None => None,
                Some("0") => Some(false),
                Some("1") => Some(true),
                Some(s) => return Err(format!("not 0 or 1: {s}")),
            };
            let me = crate::console::command_player(w)?;
            switch(w, me, want).map(Some)
        },
    );
    app.add_systems(FixedUpdate, joined_watching.before(SimSet::Rules).run_if(resource_equals(NetRole::Server)));
}

/// How many watch now.
pub fn count(world: &mut World) -> u32 {
    world.query_filtered::<(), With<Spectating>>().iter(world).count() as u32
}

/// Whether one more may watch (`mp_spectators_max`).
pub fn room(world: &mut World) -> bool {
    let max = world.get_resource::<SpectatorSettings>().copied().unwrap_or_default().max;
    count(world) < max
}

/// Whether players may switch here now (`mp_spectate_switch` and a
/// minigame playing); what the menus offer.
pub fn switch_allowed(world: &World) -> bool {
    world.get_resource::<SpectatorSettings>().is_some_and(|s| s.switch != 0 && s.max > 0)
}

/// `player` watches (`want` true), plays again (false) or flips (None),
/// as the server allows (module doc). The server's (or a standalone
/// game's); a client's `spectator` goes to its server.
pub fn switch(world: &mut World, player: Entity, want: Option<bool>) -> Result<String, String> {
    if !world.contains_resource::<ActiveMinigame>() {
        return Err("no match to watch here".into());
    }
    let watching = world.get::<Spectating>(player).is_some();
    let want = want.unwrap_or(!watching);
    if want == watching {
        return Ok(if watching { "already spectating" } else { "already playing" }.into());
    }
    if !switch_allowed(world) {
        return Err("The host doesn't allow switching to and from spectating.".into());
    }
    let name = world.get::<Name>(player).map(|n| n.to_string()).unwrap_or_default();
    if want {
        if !room(world) {
            return Err("No room for another spectator.".into());
        }
        minigame::spectators::watch(world, player)?;
        info!("{name} spectates");
        Ok("spectating".into())
    } else {
        minigame::spectators::play(world, player)?;
        info!("{name} plays");
        Ok("playing".into())
    }
}

/// A remote player who asked to join as a spectator (`cl_spectator 1` in
/// its userinfo) watches from its arrival, while there is room.
fn joined_watching(new: Query<(Entity, &UserInfo, Option<&Name>), (Added<UserInfo>, With<RemotePlayer>)>, mut commands: Commands) {
    let asked: Vec<(Entity, String)> = new
        .iter()
        .filter(|(_, info, _)| info.flag(JOIN_KEY) == Some(true))
        .map(|(e, _, n)| (e, n.map(|n| n.to_string()).unwrap_or_default()))
        .collect();
    if asked.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| {
        for (e, name) in asked {
            if !world.contains_resource::<ActiveMinigame>() {
                return;
            }
            if !room(world) {
                info!("{name} asked to spectate; no room (mp_spectators_max): plays");
                continue;
            }
            if minigame::spectators::watch(world, e).is_ok() {
                info!("{name} joined as a spectator");
            }
        }
    });
}

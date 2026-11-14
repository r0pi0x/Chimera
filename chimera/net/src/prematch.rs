//! The pre-match lobby (registered with `minigame_net`): on a server,
//! each new match (`minigame::start`: the first after the server starts,
//! each map change and rotation; `minigame::Minigames::starts` counts them)
//! waits on its loaded map until every connected player has it, shows
//! everyone who plays and how far along they are, then counts down and
//! starts the round with everyone spawning together. Off unless the host
//! turns it on (`mp_lobby 1`; the party apps' dedicated servers do at
//! launch, their main menu's Play does for a listen host); a standalone
//! game never has one; late joiners of a running match join as before.
//!
//! - **The hold** (`minigame::PreMatch`, the simulation's): players join
//!   and spawn as usual but stand frozen (the match's end freeze) and the
//!   phases don't step. It ends with `minigame::restart`: everyone joins
//!   again, spawns together and the round starts as any match does
//!   (waiting for the definition's `players.min`).
//! - **Each player's status** (`LobbyRow`, `status::*`): bots are ready
//!   at once, a listen host once its own load is done
//!   (`map::LocalMapLoad`); a remote player is ready once its client
//!   reported the server's map loaded (`server::ClientMap`), else what its
//!   client reports (`LobbyReport`, client -> server: its join's stage and
//!   the download's bytes, from `client::JoinProgress`; sent when the
//!   stage changes, the bytes at most every `REPORT_EVERY` s, again every
//!   `REPORT_AGAIN` s): connecting, downloading (bytes done and total),
//!   loading the map. Ping is the connection's round trip; `wins` the
//!   player's matches won this server run (`minigame::wins::MatchWins`,
//!   by name, bots too).
//! - **Readying up** (`mp_lobby_ready_up 1`, default 0; the party apps'
//!   Play page sets it for a listen host): a remote player on the map is
//!   "not ready" (`status::NOT_READY`) until it runs `lobby_ready` (a
//!   player command: a client's goes to the server, `net::cheats`' way;
//!   no new message), which flips it (`LobbyReady` on its client entity,
//!   cleared when a lobby begins). The host and bots are ready as above.
//! - **The lobby to everyone** (`NetLobby`, one replicated entity while
//!   the lobby is on, written when a status, a count or the second
//!   changes and at most every `WRITE_EVERY` s for bytes and pings): the
//!   map and the mode (the minigame's name), the rows, how many players
//!   it waits for, the seconds left of the longest wait and of the
//!   countdown. A client reads it while it is still downloading too
//!   (replication starts at the join's admission). The client shell
//!   draws it (`chimera_client::client::prematch_lobby`).
//! - **The countdown** (`mp_lobby_countdown` s, default 5): once every
//!   connected player is ready and at least `mp_lobby_min_players` humans
//!   (default 1) are connected; or once `mp_lobby_max_wait` s (default
//!   90) have passed since the lobby began or the first human connected,
//!   whichever is later, with the stragglers joining as late joiners when
//!   they are ready. A countdown under way isn't reset by a joiner. With
//!   no human connected it waits (nothing to play for).
//! - **Console**: `lobby` (the rows, the server's or as a client heard
//!   them), `lobby_start [s]` (the host: the countdown now, of `s`
//!   seconds, default `mp_lobby_countdown`; 0 starts at once),
//!   `lobby_ready [0|1]` (a player's, above). The client's panel puts both
//!   on F3 while it shows (`chimera_client::client::prematch_lobby`).
//!
//! A match change during the lobby (`MatchHold`) drops it; the next start
//! begins another. A start while the lobby is up (the automatic gun game
//! starts its match twice) keeps it: one lobby, one log line. The
//! `mp_lobby*` cvars and `lobby*` commands are engine names
//! (`chimera:`, `console_names`), not CS:S's. A map change no longer waits for the clients to load
//! the map before the next match starts when the lobby is on
//! (`minigame_net`'s `CLIENT_GRACE`): the lobby waits for them, showing
//! who.
//!
//! Known shortcuts: a player still in the first start's download (before
//! the game's window, `asset_startup`) hasn't connected and isn't listed;
//! `mp_lobby_min_players` waits for them. Off on a listen server unless
//! the host sets `mp_lobby 1` (the party apps' Play page does; a game's
//! `--map ... +maxplayers` doesn't). Taking readiness back doesn't stop a
//! countdown under way; a client of an older build can't ready up (the
//! host's start or the longest wait starts the match) and shows a
//! not-ready player as "Connecting".

use bevy::prelude::*;
use bevy_replicon::{prelude::*, shared::backend::connected_client::NetworkId};
use serde::{Deserialize, Serialize};

use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{NetRole, SimClock, SimSet},
    minigame::{self, ActiveMinigame, Minigames, PreMatch},
    net::{
        HOST_ID, NetCharacter, NetHooks,
        client::{JoinProgress, JoinStage},
        server::{ClientMap, Player},
    },
};

/// A player's status in the lobby (`LobbyRow::status`, `LobbyReport`).
pub mod status {
    /// Connected; joining (no report yet, or the handshake).
    pub const CONNECTING: u8 = 0;
    /// Fetching the match's files (`LobbyRow::done` of `total` bytes).
    pub const DOWNLOADING: u8 = 1;
    /// Loading the map (or changing to it).
    pub const LOADING: u8 = 2;
    /// On the map, waiting for the round.
    pub const READY: u8 = 3;
    /// On the map but not ready yet: a player of a lobby where players
    /// ready up (`mp_lobby_ready_up 1`) who hasn't (`lobby_ready`). A
    /// client of an older build shows it as "Connecting".
    pub const NOT_READY: u8 = 4;

    /// The status's words.
    pub fn name(s: u8) -> &'static str {
        match s {
            DOWNLOADING => "Downloading",
            LOADING => "Loading map",
            READY => "Ready",
            NOT_READY => "Not ready",
            _ => "Connecting",
        }
    }
}

/// The lobby's settings (cvars `mp_lobby`, `mp_lobby_countdown`,
/// `mp_lobby_max_wait`, `mp_lobby_min_players`, `mp_lobby_ready_up`).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct LobbySettings {
    /// 1: a server holds each new match in the lobby.
    pub enabled: u8,
    /// The countdown, s.
    pub countdown: f32,
    /// The longest wait for players, s, before the countdown starts anyway.
    pub max_wait: f32,
    /// Humans connected before everyone's being ready starts the countdown.
    pub min_players: u32,
    /// 1: a remote player on the map is ready once it says so
    /// (`lobby_ready`); 0: once it is on the map.
    pub ready_up: u8,
}

impl Default for LobbySettings {
    fn default() -> Self {
        Self {
            enabled: 0,
            countdown: 5.0,
            max_wait: 90.0,
            min_players: 1,
            ready_up: 0,
        }
    }
}

/// Turn the lobby on in `app` (a party server's launch): the cvar's value
/// from the start, which the command line may still change.
pub fn enable(app: &mut App) {
    let mut s = app.world().get_resource::<LobbySettings>().copied().unwrap_or_default();
    s.enabled = 1;
    app.insert_resource(s);
}

/// Client -> server: where this client's join has come (`status::*`, from
/// its `JoinProgress`) and the download's bytes while it downloads.
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LobbyReport {
    pub status: u8,
    pub done: u64,
    pub total: u64,
}

/// The server: a client's last `LobbyReport`, on its client entity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientLobbyStatus(pub LobbyReport);

/// The server: on a client's entity once its player readied up in this
/// lobby (`lobby_ready`); every lobby begins without.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LobbyReady;

/// One player in the lobby.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct LobbyRow {
    /// The client's id (`NetCharacter::owner`; `HOST_ID` for a listen
    /// server's host); None for a bot.
    pub id: Option<u64>,
    pub name: String,
    /// `status::*`.
    pub status: u8,
    /// Downloading: bytes done and in all (0: unknown).
    pub done: u64,
    pub total: u64,
    /// Round trip, ms (a remote player's).
    pub ping_ms: Option<u16>,
    /// Matches won this server run (`minigame::wins::MatchWins`, by name).
    pub wins: u32,
}

impl LobbyRow {
    pub fn ready(&self) -> bool {
        self.status == status::READY
    }

    pub fn bot(&self) -> bool {
        self.id.is_none()
    }
}

/// The lobby as everyone sees it, on one replicated entity while it is on.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NetLobby {
    /// The map (`net::normalize_map`'s form: `cs_source:de_dust2`).
    pub map: String,
    /// The minigame's name.
    pub mode: String,
    /// Humans first (the host, then remote players as they joined), then
    /// bots.
    pub rows: Vec<LobbyRow>,
    /// Players it waits for: those not ready, and humans still to come
    /// for `mp_lobby_min_players`.
    pub waiting: u32,
    /// Seconds (rounded up) before the longest wait starts the countdown
    /// anyway; None while no limit runs (no human yet) or counting down.
    pub wait_left: Option<u32>,
    /// Seconds (rounded up) left of the countdown, once it began.
    pub countdown: Option<u32>,
}

/// The server's lobby while it is on.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Lobby {
    /// When it began (`SimClock`).
    pub began: f64,
    /// When the first human was connected during it.
    pub humans_since: Option<f64>,
    /// When the countdown ends, once it began.
    pub countdown_ends: Option<f64>,
    /// When `NetLobby` was last written.
    written: f64,
}

/// The lobby shows: on a client, the match on screen hasn't started (what
/// it fetches on demand counts as loading: `net::on_demand`).
fn lobby_shown(_: On<Add<NetLobby>>, showing: Option<ResMut<crate::net::on_demand::LobbyShowing>>) {
    if let Some(mut s) = showing {
        s.0 = true;
    }
}

fn lobby_gone(_: On<Remove<NetLobby>>, showing: Option<ResMut<crate::net::on_demand::LobbyShowing>>) {
    if let Some(mut s) = showing {
        s.0 = false;
    }
}

/// How often (s) at most a client reports its download's bytes.
pub const REPORT_EVERY: f64 = 0.25;
/// A client reports again this often (s) even when nothing changed (a
/// report sent before the server admitted it is lost).
pub const REPORT_AGAIN: f64 = 2.0;
/// How often (s) at most bytes and pings rewrite `NetLobby`.
pub const WRITE_EVERY: f64 = 0.25;

/// The protocol and the systems (`minigame_net`'s part; it runs `begin`
/// and `serve` after its own tick's end).
pub(crate) fn register(app: &mut App) {
    app.init_resource::<LobbySettings>()
        .replicate::<NetLobby>()
        .add_client_message::<LobbyReport>(Channel::Ordered)
        .add_observer(lobby_shown)
        .add_observer(lobby_gone);
    resource_cvar::<LobbySettings, u8>(
        app,
        "mp_lobby",
        "Server: 1 holds each new match (the first, each map change) in a pre-match lobby until every connected \
         player has the map, then counts down; 0 starts at once.",
        |s| &mut s.enabled,
    );
    resource_cvar::<LobbySettings, f32>(
        app,
        "mp_lobby_countdown",
        "Server: the pre-match lobby's countdown, s, once everyone is ready.",
        |s| &mut s.countdown,
    );
    resource_cvar::<LobbySettings, f32>(
        app,
        "mp_lobby_max_wait",
        "Server: the longest the pre-match lobby waits for players, s (from its start or the first player's \
         arrival), before the countdown starts anyway; the rest join when ready.",
        |s| &mut s.max_wait,
    );
    resource_cvar::<LobbySettings, u32>(
        app,
        "mp_lobby_min_players",
        "Server: players (not bots) connected before the pre-match lobby counts down for being ready (the most \
         it waits is still mp_lobby_max_wait).",
        |s| &mut s.min_players,
    );
    resource_cvar::<LobbySettings, u8>(
        app,
        "mp_lobby_ready_up",
        "Server: 1: a player on the pre-match lobby's map is ready once it says so (lobby_ready, F3); 0: once it \
         has the map.",
        |s| &mut s.ready_up,
    );
    app.console_command(
        "lobby_start",
        "lobby_start [s]: end the pre-match lobby now: the countdown starts (s seconds, default mp_lobby_countdown; \
         0 starts the match at once).",
        |w, a| {
            let secs = match a.first() {
                Some(s) => Some(s.parse::<f64>().ok().filter(|s| *s >= 0.0).ok_or_else(|| format!("not seconds: {s}"))?),
                None => None,
            };
            start_now(w, secs).map(Some)
        },
    )
    .console_command(
        "lobby_ready",
        "lobby_ready [0|1]: you are ready for the pre-match lobby's match (a player's; no argument: flips it). \
         Counts where players ready up (mp_lobby_ready_up 1).",
        |w, a| {
            let want = match a.first().map(String::as_str) {
                None => None,
                Some("0") => Some(false),
                Some("1") => Some(true),
                Some(s) => return Err(format!("not 0 or 1: {s}")),
            };
            ready_up(w, want).map(Some)
        },
    )
    .console_command(
        "lobby",
        "The pre-match lobby: each player's status, what it waits for.",
        |w, _| Ok(Some(lobby_text(w))),
    );
    // A network client's `lobby_ready` goes to its server, for its player
    // (`net::cheats`' way).
    app.world_mut().resource_mut::<crate::console::Console>().player_command("chimera:lobby_ready");
    app.world_mut().resource_mut::<NetHooks>().stopped_serving.push(stopped_serving);
    let server = || resource_equals(NetRole::Server);
    app.add_systems(PreUpdate, receive_reports.after(ServerSystems::Receive).run_if(server()))
        .add_systems(FixedUpdate, begin.before(SimSet::Rules).run_if(server()))
        .add_systems(
            PostUpdate,
            report.before(ClientSystems::Send).run_if(resource_equals(NetRole::Client)),
        );
}

/// Whether a server holds new matches in the lobby (`mp_lobby`).
pub fn lobby_on(world: &World) -> bool {
    world.get_resource::<NetRole>() == Some(&NetRole::Server)
        && world.get_resource::<LobbySettings>().is_some_and(|s| s.enabled != 0)
}

/// Serving stopped: the lobby goes.
fn stopped_serving(world: &mut World) {
    end(world, false);
}

/// The status a join's stage reports.
pub fn status_of(stage: JoinStage) -> u8 {
    match stage {
        JoinStage::Idle | JoinStage::Connecting | JoinStage::ServerInfo => status::CONNECTING,
        JoinStage::Verifying | JoinStage::Downloading => status::DOWNLOADING,
        JoinStage::ChangingLevel | JoinStage::LoadingMap => status::LOADING,
        JoinStage::Joined => status::READY,
    }
}

/// A client: its join's status to the server (`LobbyReport`): at once
/// when the status changes, the bytes at most every `REPORT_EVERY` s,
/// the same again every `REPORT_AGAIN` s.
fn report(
    progress: Option<Res<JoinProgress>>,
    time: Res<Time<Real>>,
    mut last: Local<Option<(LobbyReport, f64)>>,
    mut out: MessageWriter<LobbyReport>,
) {
    let Some(p) = progress else { return };
    let status = status_of(p.stage);
    let (done, total) = match (&p.download, status) {
        (Some(d), status::DOWNLOADING) => (d.done, d.total),
        _ => (0, 0),
    };
    let now_report = LobbyReport { status, done, total };
    let now = time.elapsed_secs_f64();
    let due = match *last {
        None => true,
        Some((r, _)) if r.status != status => true,
        Some((r, at)) if r != now_report => now - at >= REPORT_EVERY,
        Some((_, at)) => now - at >= REPORT_AGAIN,
    };
    if due {
        out.write(now_report);
        *last = Some((now_report, now));
    }
}

/// The server: each admitted client's report, kept on its entity.
fn receive_reports(mut reports: MessageReader<FromClient<LobbyReport>>, players: Query<(), With<Player>>, mut commands: Commands) {
    for r in reports.read() {
        let Some(client) = r.client_id.entity() else { continue };
        if players.contains(client) {
            commands.entity(client).insert(ClientLobbyStatus(r.message));
        }
    }
}

/// A new match (`Minigames::starts`) on a server with the lobby on: held in the
/// lobby. Waits for the server to start serving (a dedicated server's
/// first match starts before it listens) while that match hasn't begun.
pub(crate) fn begin(world: &mut World, mut seen: Local<u64>) {
    let starts = world.get_resource::<Minigames>().map_or(0, |m| m.starts);
    if starts == *seen {
        return;
    }
    *seen = starts;
    // A second start while the lobby is up (the automatic gun game starts
    // its match twice; `start` lifted `PreMatch`) keeps the one lobby:
    // held again, no reset and no second log line.
    if world.contains_resource::<Lobby>() {
        if lobby_on(world) && world.get_resource::<ActiveMinigame>().is_some() && !minigame::held(world) {
            world.insert_resource(PreMatch);
        }
        return;
    }
    if !lobby_on(world) || world.get_resource::<ActiveMinigame>().is_none() || minigame::held(world) {
        return;
    }
    let now = world.resource::<SimClock>().now;
    // Each lobby begins with nobody readied up.
    let readied: Vec<Entity> = world.query_filtered::<Entity, With<LobbyReady>>().iter(world).collect();
    for e in readied {
        world.entity_mut(e).remove::<LobbyReady>();
    }
    world.insert_resource(PreMatch);
    world.insert_resource(Lobby {
        began: now,
        humans_since: None,
        countdown_ends: None,
        written: f64::MIN,
    });
    let s = *world.resource::<LobbySettings>();
    info!(
        "pre-match lobby on {}: waiting for the players (at most {} s once one is here)",
        crate::net::current_map(world),
        s.max_wait
    );
}

/// The lobby gone: `PreMatch` lifted and `NetLobby` despawned; `start`:
/// the match begins (`minigame::restart`: everyone spawns together).
fn end(world: &mut World, start: bool) {
    world.remove_resource::<Lobby>();
    let had = world.remove_resource::<PreMatch>().is_some();
    let entities: Vec<Entity> = world.query_filtered::<Entity, With<NetLobby>>().iter(world).collect();
    for e in entities {
        world.despawn(e);
    }
    if start && had && world.contains_resource::<ActiveMinigame>() {
        minigame::restart(world);
        info!("pre-match lobby over: the match starts");
    }
}

/// The map the match is on (the loaded one: `maps::ServedMap` follows it
/// a frame later, and a client's `ClientMap` of the map left mustn't
/// count).
fn serving_map(world: &World) -> String {
    crate::net::current_map(world)
}

/// Every player now: the host, the remote players (admitted), the bots.
pub fn rows(world: &mut World) -> Vec<LobbyRow> {
    let map = serving_map(world);
    let mut out = Vec::new();
    let mut others = Vec::new();
    // A listen host is ready once its own load is done (spawned and
    // settled behind its loading screen).
    let host = if crate::net::maps::local_load_under_way(world) { status::LOADING } else { status::READY };
    let ready_up = world.get_resource::<LobbySettings>().is_some_and(|s| s.ready_up != 0);
    for c in world.query::<&NetCharacter>().iter(world) {
        match c.owner {
            Some(HOST_ID) => out.push(LobbyRow {
                id: Some(HOST_ID),
                name: c.name.clone(),
                status: host,
                ..default()
            }),
            None => others.push(LobbyRow {
                id: None,
                name: c.name.clone(),
                status: status::READY,
                ..default()
            }),
            Some(_) => {}
        }
    }
    let mut remote: Vec<(u64, LobbyRow)> = world
        .query::<(
            &Player,
            &NetworkId,
            Option<&ConnectedClientStats>,
            Option<&ClientMap>,
            Option<&ClientLobbyStatus>,
            Has<LobbyReady>,
        )>()
        .iter(world)
        .map(|(p, id, stats, have, report, readied)| {
            let on_map = have.is_some_and(|m| m.0.eq_ignore_ascii_case(&map));
            let r = report.map(|r| r.0).unwrap_or_default();
            let status = match (on_map, ready_up && !readied) {
                (true, false) => status::READY,
                (true, true) => status::NOT_READY,
                _ => r.status.min(status::LOADING),
            };
            let (done, total) = if status == status::DOWNLOADING { (r.done, r.total) } else { (0, 0) };
            (
                id.get(),
                LobbyRow {
                    id: Some(id.get()),
                    name: p.name.clone(),
                    status,
                    done,
                    total,
                    ping_ms: stats.map(|s| (s.rtt * 1000.0).round().clamp(0.0, 9999.0) as u16),
                    wins: 0,
                },
            )
        })
        .collect();
    remote.sort_by_key(|(id, _)| *id);
    out.extend(remote.into_iter().map(|(_, r)| r));
    others.sort_by(|a, b| a.name.cmp(&b.name));
    out.extend(others);
    // The session wins, by name (bots too).
    if let Some(wins) = world.get_resource::<minigame::wins::MatchWins>() {
        for r in &mut out {
            r.wins = wins.of(&r.name);
        }
    }
    out
}

/// The lobby's tick (after the minigame's): who is ready, the countdown
/// begun or over, `NetLobby` written.
pub(crate) fn serve(world: &mut World) {
    let Some(mut lobby) = world.get_resource::<Lobby>().copied() else { return };
    // Another match is coming (a map change): this lobby is gone.
    if minigame::held(world) || !world.contains_resource::<ActiveMinigame>() {
        end(world, false);
        return;
    }
    let now = world.resource::<SimClock>().now;
    let s = *world.resource::<LobbySettings>();
    let rows = rows(world);
    let humans = rows.iter().filter(|r| !r.bot()).count() as u32;
    if humans > 0 && lobby.humans_since.is_none() {
        lobby.humans_since = Some(now);
    }
    let not_ready = rows.iter().filter(|r| !r.ready()).count() as u32;
    let to_come = s.min_players.saturating_sub(humans);
    let wait_ends = lobby.humans_since.map(|h| h.max(lobby.began) + s.max_wait as f64);
    if lobby.countdown_ends.is_none() {
        if humans > 0 && not_ready == 0 && to_come == 0 {
            info!("pre-match lobby: everyone is ready; the match starts in {} s", s.countdown);
            lobby.countdown_ends = Some(now + s.countdown.max(0.0) as f64);
        } else if wait_ends.is_some_and(|w| now >= w) {
            let late: Vec<&str> = rows.iter().filter(|r| !r.ready()).map(|r| r.name.as_str()).collect();
            info!(
                "pre-match lobby: waited {} s; the match starts in {} s{}",
                s.max_wait,
                s.countdown,
                if late.is_empty() { String::new() } else { format!(" ({} join when ready)", late.join(", ")) }
            );
            lobby.countdown_ends = Some(now + s.countdown.max(0.0) as f64);
        }
    }
    if lobby.countdown_ends.is_some_and(|t| now + 1e-9 >= t) {
        end(world, true);
        return;
    }
    let secs = |t: f64| (t - now).max(0.0).ceil() as u32;
    let def = world.resource::<ActiveMinigame>().def;
    let want = NetLobby {
        map: serving_map(world),
        mode: def.name.clone(),
        rows,
        waiting: not_ready + to_come,
        wait_left: wait_ends.filter(|_| lobby.countdown_ends.is_none()).map(secs),
        countdown: lobby.countdown_ends.map(secs),
    };
    let have = world.query::<(Entity, &NetLobby)>().iter(world).next().map(|(e, n)| (e, n.clone()));
    match have {
        None => {
            world.spawn((Name::new("Pre-match lobby"), Replicated, want));
            lobby.written = now;
        }
        Some((e, n)) if n != want => {
            // Statuses, counts and seconds at once; bytes and pings at
            // most every `WRITE_EVERY` s.
            let coarse = |l: &NetLobby| {
                let rows: Vec<(Option<u64>, String, u8)> = l.rows.iter().map(|r| (r.id, r.name.clone(), r.status)).collect();
                (rows, l.waiting, l.wait_left, l.countdown, l.map.clone(), l.mode.clone())
            };
            if coarse(&n) != coarse(&want) || now - lobby.written >= WRITE_EVERY {
                world.entity_mut(e).insert(want);
                lobby.written = now;
            }
        }
        Some(_) => {}
    }
    world.insert_resource(lobby);
}

/// `lobby_start [s]`: the countdown now (`s` s, default the cvar's; 0:
/// the match at once).
pub fn start_now(world: &mut World, secs: Option<f64>) -> Result<String, String> {
    if world.get_resource::<NetRole>() == Some(&NetRole::Client) {
        return Err("the server starts the match; you are a client of it".into());
    }
    let Some(mut lobby) = world.get_resource::<Lobby>().copied() else {
        return Err("no pre-match lobby now".into());
    };
    let secs = secs.unwrap_or(world.resource::<LobbySettings>().countdown as f64);
    let now = world.resource::<SimClock>().now;
    lobby.countdown_ends = Some(now + secs);
    world.insert_resource(lobby);
    info!("pre-match lobby: started by the host; the match starts in {secs} s");
    Ok(format!("the match starts in {secs} s"))
}

/// `lobby_ready [0|1]`: the player it runs for (a network client's, sent
/// on by its console) ready for the match or not (`want`; None flips it).
pub fn ready_up(world: &mut World, want: Option<bool>) -> Result<String, String> {
    if world.get_resource::<NetRole>() != Some(&NetRole::Server) {
        return Err("no pre-match lobby here".into());
    }
    let Some(character) = world.get_resource::<crate::console::CommandPlayer>().map(|p| p.0) else {
        return Err("you host: lobby_start starts the match".into());
    };
    let client = world
        .query::<(Entity, &Player)>()
        .iter(world)
        .find(|(_, p)| p.character == character)
        .map(|(e, _)| e)
        .ok_or("not a network player")?;
    if !world.contains_resource::<Lobby>() {
        return Err("no pre-match lobby now".into());
    }
    let ready = want.unwrap_or(!world.entity(client).contains::<LobbyReady>());
    let name = world.get::<Player>(client).map(|p| p.name.clone()).unwrap_or_default();
    if ready {
        world.entity_mut(client).insert(LobbyReady);
    } else {
        world.entity_mut(client).remove::<LobbyReady>();
    }
    info!("pre-match lobby: {name} is {}", if ready { "ready" } else { "not ready" });
    Ok(if ready { "ready" } else { "not ready" }.into())
}

/// `lobby`'s text: the lobby (this server's, or as a client heard it).
fn lobby_text(world: &mut World) -> String {
    let Some(l) = world.query::<&NetLobby>().iter(world).next().cloned() else {
        return "no pre-match lobby now".into();
    };
    let mut lines = vec![format!(
        "{} on {}: waiting for {}{}{}",
        l.mode,
        l.map,
        l.waiting,
        l.wait_left.map(|s| format!(", at most {s} s")).unwrap_or_default(),
        l.countdown.map(|s| format!(", starts in {s} s")).unwrap_or_default()
    )];
    for r in &l.rows {
        let bytes = if r.status == status::DOWNLOADING {
            format!(" {:.1}/{:.1} MB", r.done as f64 / 1e6, r.total as f64 / 1e6)
        } else {
            String::new()
        };
        let ping = r.ping_ms.map(|p| format!(" {p} ms")).unwrap_or_default();
        let who = if r.bot() { " (bot)" } else { "" };
        lines.push(format!("  {}{who}: {}{bytes}{ping}", r.name, status::name(r.status)));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_joins_stage_is_its_status() {
        assert_eq!(status_of(JoinStage::Connecting), status::CONNECTING);
        assert_eq!(status_of(JoinStage::Downloading), status::DOWNLOADING);
        assert_eq!(status_of(JoinStage::ChangingLevel), status::LOADING);
        assert_eq!(status_of(JoinStage::LoadingMap), status::LOADING);
        assert_eq!(status_of(JoinStage::Joined), status::READY);
    }
}

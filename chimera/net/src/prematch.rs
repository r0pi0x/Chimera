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
//! - **The hold** (`minigame::PreMatch`, the simulation's): nobody plays.
//!   Every character (remote players, a listen host's, bots) is staged
//!   (`minigame::Staged`: no weapons, dead, not solid, out of the game),
//!   those joining as they come (a client's map loaded during the lobby
//!   doesn't take its player into the game: `server::EntryHeld`);
//!   the phases don't step and the end freeze holds. It ends with
//!   `minigame::restart` and everyone let in (`let_in`: the join rules'
//!   `enter`), so all spawn together on the first rung and the round
//!   starts as any match does (waiting for the definition's
//!   `players.min`); a player whose client still loads comes in when it
//!   has the map.
//! - **Each player's status** (`LobbyRow`, `status::*`): bots are ready
//!   at once, a listen host once its own load is done
//!   (`map::LocalMapLoad`); a remote player is ready once its client
//!   reported the server's map loaded (`server::ClientMap`), else what its
//!   client reports (`LobbyReport`, client -> server: its join's stage and
//!   the download's bytes, from `client::JoinProgress`; sent when the
//!   stage changes, the bytes at most every `REPORT_EVERY` s, again every
//!   `REPORT_AGAIN` s): connecting, downloading (bytes done and total),
//!   loading the map; while downloading or loading, the stage under way
//!   and its share from the client's own work (`LobbyReport::step`,
//!   `permille`: `crate::net::load_stages`, the download, the map's load,
//!   building models, physics; the next map's files fetched ahead count as
//!   a download). A listen host's row is its own load's stage. Ping is the connection's round trip; `wins` the
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
//! - **Settings** (`NetLobby::settings`, `settings_now`): what a player
//!   wants to know before the match starts, written with the lobby: the
//!   minigame's length, then each `LobbyFact` an app registered (`add_fact`:
//!   a label, a server cvar, its values as words; the party's from its host
//!   settings' `lobby = true`), as label and value; a value of "" (the
//!   minigame's own) is left out.
//! - **The host's start** (`mp_lobby_host_start`, default 1; the party
//!   apps' Host page shows it): the server run's first lobby waits for the
//!   host's `lobby_start` (a dedicated server's console or an RCON admin) (`Lobby::host_start`, `NetLobby::host_start`):
//!   neither everyone being ready nor the longest wait counts it down.
//!   Later lobbies (map changes) start as below.
//! - **Shown first** (`show_first`, `HostLobbyFirst`): a listen host's
//!   menu shows its lobby from Start, before the map is in (the host's
//!   row loading, the bots its game keeps: `first_bots`; nothing
//!   replicated); `begin` replaces it with the server's.
//! - **Bots at once**: the rows list as many bots as the game keeps
//!   (`BotHooks::quota`), those not added yet (they come once the map is
//!   in) as "Bot", so the bots don't appear one by one.
//! - **Between maps** (`begin_between`, `BetweenMaps`; docs/plans/active/
//!   lobby-between-maps.md): a match's change to another map
//!   (`minigame_net::change_match`: a rotation after the match-end
//!   screen, a vote's map, a host's `changelevel`) begins the lobby at
//!   once, everyone staged, naming the map coming in, with every row
//!   loading it (its stage and share) while each machine loads it; the
//!   match made there takes it over as its own lobby (`begin`), its
//!   longest wait counted from then. A change called off (the match no
//!   longer held) ends it.
//! - **Avatars**: each row carries its player's `cl_avatar` userinfo
//!   (`LobbyRow::avatar`, `NO_AVATAR` for a bot or none).
//! - **The countdown** (`mp_lobby_countdown` s, default 5): once every
//!   connected player is ready and at least `mp_lobby_min_players` humans
//!   (default 1) are connected; or once `mp_lobby_max_wait` s (default
//!   90) have passed since the lobby began or the first human connected,
//!   whichever is later, with the stragglers joining as late joiners when
//!   they are ready. Neither starts while the server fetches its own
//!   game files for the match (`net::server_assets`). A countdown under way isn't reset by a joiner. With
//!   no human connected it waits (nothing to play for).
//! - **Console**: `lobby` (the rows, the server's or as a client heard
//!   them), `lobby_start [s]` (the host: the countdown now, of `s`
//!   seconds, default `mp_lobby_countdown`; 0 starts at once),
//!   `lobby_ready [0|1]` (a player's, above). The client's panel puts both
//!   on F3 while it shows (`chimera_client::client::prematch_lobby`).
//!
//! A match change during the lobby (`MatchHold`) drops it; the next start
//! begins another. A minigame whose definition says `lobby = false` (the
//! sandbox) never has one, between maps or at its start (`lobby_on`,
//! `NoLobby`): it is entered at once. A start while the lobby is up (the automatic gun game
//! starts its match twice) keeps it: one lobby, one log line. The
//! `mp_lobby*` cvars and `lobby*` commands are engine names
//! (`chimera:`, `console_names`), not CS:S's. A map change no longer waits for the clients to load
//! the map before the next match starts when the lobby is on
//! (`minigame_net`'s `CLIENT_GRACE`): the lobby waits for them, showing
//! who.
//!
//! The host's start (F3, Start) in the lobby shown first waits for the
//! server's lobby (`StartAsked`), which then counts down at once.
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
        server::{ClientMap, EntryHeld, Player},
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
    /// 1 (the default): the server run's first lobby waits for the host's
    /// start (`lobby_start`: a listen host's Start, a dedicated server's
    /// console or an RCON admin): nothing counts it down by itself; 0: it
    /// counts down once everyone is ready or after the longest wait.
    /// Later lobbies start once everyone has loaded.
    pub host_start: u8,
}

impl Default for LobbySettings {
    fn default() -> Self {
        Self {
            enabled: 0,
            countdown: 5.0,
            max_wait: 90.0,
            min_players: 1,
            ready_up: 0,
            host_start: 1,
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
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyReport {
    pub status: u8,
    pub done: u64,
    pub total: u64,
    /// The load's running stage (`load_stages::LoadStep::index`; `NO_STEP`:
    /// none) and its share in thousandths.
    pub step: u8,
    pub permille: u16,
}

/// `LobbyReport::step`, `LobbyRow::step` for no stage.
pub const NO_STEP: u8 = 255;

impl Default for LobbyReport {
    fn default() -> Self {
        Self { status: status::CONNECTING, done: 0, total: 0, step: NO_STEP, permille: 0 }
    }
}

/// The server: a client's last `LobbyReport`, on its client entity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientLobbyStatus(pub LobbyReport);

/// The server: on a client's entity once its player readied up in this
/// lobby (`lobby_ready`); every lobby begins without.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LobbyReady;

/// One player in the lobby.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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
    /// Downloading or loading: the stage under way
    /// (`load_stages::LoadStep::index`: the download, the map, the models,
    /// the physics; `NO_STEP`: not known) and its share in thousandths.
    pub step: u8,
    pub permille: u16,
    /// Round trip, ms (a remote player's).
    pub ping_ms: Option<u16>,
    /// Matches won this server run (`minigame::wins::MatchWins`, by name).
    pub wins: u32,
    /// The player's avatar (its `cl_avatar` userinfo: glyph x 8 +
    /// colour, `chimera_ui::avatar::AvatarLook`'s byte); `NO_AVATAR` for a
    /// bot or a player who sent none.
    pub avatar: u8,
    /// The player's party (players who came together share it; 0: alone):
    /// the server's number for its `cl_party` (`PartyNumbers`), given
    /// to parties with two or more players here.
    pub party: u8,
}

/// `LobbyRow::avatar` for none.
pub const NO_AVATAR: u8 = 255;

/// A character's avatar byte from its userinfo (`cl_avatar`, 0-31).
pub fn avatar_of(info: Option<&crate::core::UserInfo>) -> u8 {
    info.and_then(|i| i.0.get("cl_avatar"))
        .and_then(|v| v.trim().parse::<i32>().ok())
        .filter(|v| (0..32).contains(v))
        .map_or(NO_AVATAR, |v| v as u8)
}

impl Default for LobbyRow {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            status: status::CONNECTING,
            done: 0,
            total: 0,
            step: NO_STEP,
            permille: 0,
            ping_ms: None,
            wins: 0,
            avatar: NO_AVATAR,
            party: 0,
        }
    }
}

impl LobbyRow {
    /// The stage under way and its share (0-1), when told.
    pub fn stage(&self) -> Option<(crate::net::load_stages::LoadStep, f32)> {
        let step = crate::net::load_stages::LoadStep::from_index(self.step)?;
        matches!(self.status, status::DOWNLOADING | status::LOADING).then(|| (step, (self.permille.min(1000) as f32) / 1000.0))
    }

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
    /// It waits for the host's start (`LobbySettings::host_start`, the
    /// first lobby): nothing starts it by itself.
    pub host_start: bool,
    /// The match's settings a player wants to know before it starts
    /// (label, value as words): the mode's length and what the host's
    /// `LobbyFacts` name (docs/plans/active/lobby-between-maps.md, step 5).
    pub settings: Vec<(String, String)>,
}

/// A match setting the lobby tells every player (`NetLobby::settings`),
/// from a server cvar: an app registers the ones its host page sets
/// (`add_fact`; the party's from data/host_settings.toml).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbyFact {
    /// The setting's name as shown ("Cross-game balance").
    pub label: String,
    /// The server cvar holding it.
    pub cvar: String,
    /// The cvar's values as words ("1" is "On"); a value not listed shows
    /// as it is.
    pub words: Vec<(String, String)>,
    /// Values that say nothing (the minigame's own: ""), left out.
    pub hide: Vec<String>,
}

/// The facts the lobby tells (`LobbyFact`), in order.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct LobbyFacts(pub Vec<LobbyFact>);

/// Registers a fact the lobby tells.
pub fn add_fact(app: &mut App, fact: LobbyFact) {
    app.init_resource::<LobbyFacts>().world_mut().resource_mut::<LobbyFacts>().0.push(fact);
}

/// The settings to tell now: the mode's length (when `mode` is the active
/// minigame's), then each registered fact's cvar value in words.
pub fn settings_now(world: &mut World, mode: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(def) = world.get_resource::<ActiveMinigame>().map(|m| m.def).filter(|d| d.name == mode) {
        let t = def.time_limit.round() as u32;
        out.push(("Length".to_string(), if t >= 60 && t % 60 == 0 { format!("{} min", t / 60) } else { format!("{t} s") }));
    }
    let facts = world.get_resource::<LobbyFacts>().map(|f| f.0.clone()).unwrap_or_default();
    for f in facts {
        let cvar = world.get_resource::<crate::console::Console>().and_then(|c| c.cvar(&f.cvar).cloned());
        let Some(value) = cvar.and_then(|c| (c.get)(world)) else { continue };
        let value = value.trim().to_string();
        if f.hide.iter().any(|h| h.trim().eq_ignore_ascii_case(&value)) {
            continue;
        }
        let shown = f.words.iter().find(|(v, _)| v.trim().eq_ignore_ascii_case(&value)).map_or(value, |(_, w)| w.clone());
        out.push((f.label, shown));
    }
    out
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
    /// Waits for the host's start (the server run's first lobby with
    /// `mp_lobby_host_start 1`).
    pub host_start: bool,
    /// Between maps (`begin_between`): the next match's map still coming
    /// in (`BetweenMaps`); nothing counts down until the match is made
    /// there.
    pub between: bool,
}

/// The lobby between maps (`begin_between`): the map coming in and the
/// mode to be played there.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct BetweenMaps {
    pub map: String,
    pub mode: String,
}

/// Lobbies begun this server run (the first may wait for the host).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LobbiesBegun(pub u32);

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
        "mp_lobby_host_start",
        "Server: 1 (default): the first pre-match lobby of the server's run waits for the host's start \
         (lobby_start); nothing counts down by itself. 0: it counts down once everyone is ready or after \
         mp_lobby_max_wait. Later lobbies start once everyone has the map.",
        |s| &mut s.host_start,
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
        .add_systems(Update, first_bots)
        .add_systems(
            PostUpdate,
            report.before(ClientSystems::Send).run_if(resource_equals(NetRole::Client)),
        );
}

/// Whether a server holds new matches in the lobby (`mp_lobby`).
pub fn lobby_on(world: &World) -> bool {
    world.get_resource::<NetRole>() == Some(&NetRole::Server)
        && world.get_resource::<LobbySettings>().is_some_and(|s| s.enabled != 0)
        && !world.contains_resource::<NoLobby>()
}

/// The match coming or playing is entered at once (its definition's
/// `lobby = false`: the sandbox): no lobby for it (`lobby_on`). Kept by
/// `minigame_net` from the match's definition.
#[derive(Resource, Clone, Copy, Debug)]
pub struct NoLobby;

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
fn report(world: &mut World, mut last: Local<Option<(LobbyReport, f64)>>) {
    let Some(p) = world.get_resource::<JoinProgress>() else { return };
    let now_report = report_of(p.stage, &crate::net::load_stages::stages(world));
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    let due = match *last {
        None => true,
        Some((r, _)) if r.status != now_report.status || r.step != now_report.step => true,
        Some((r, at)) if r != now_report => now - at >= REPORT_EVERY,
        Some((_, at)) => now - at >= REPORT_AGAIN,
    };
    if due {
        world.write_message(now_report);
        *last = Some((now_report, now));
    }
}

/// A client's report: its join's status (`status_of`), and while it
/// downloads or loads, the stage under way (`load_stages`: the next map's
/// files fetched ahead count as downloading) with its share and the
/// download's bytes.
pub fn report_of(stage: JoinStage, stages: &[crate::net::load_stages::Stage]) -> LobbyReport {
    use crate::net::load_stages::{LoadStep, StageState, permille};
    let mut status = status_of(stage);
    let running = stages.iter().find_map(|s| match s.state {
        StageState::Running(f) => Some((s, f)),
        _ => None,
    });
    let mut r = LobbyReport { status, ..default() };
    if let Some((s, share)) = running {
        if s.step == LoadStep::Download {
            status = status::DOWNLOADING;
        } else if status != status::DOWNLOADING {
            status = status::LOADING;
        }
        r = LobbyReport { status, step: s.step.index(), permille: permille(share), ..r };
        if let Some((done, total)) = s.bytes.filter(|_| status == status::DOWNLOADING) {
            (r.done, r.total) = (done, total);
        }
    }
    r
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
    if let Some(mut lobby) = world.get_resource::<Lobby>().copied() {
        if lobby_on(world) && world.get_resource::<ActiveMinigame>().is_some() && !minigame::held(world) {
            world.insert_resource(PreMatch);
            // The lobby between maps: the match is made on its map, so it
            // goes on as this match's lobby, its longest wait counted from
            // now (a host's start asked meanwhile kept).
            if lobby.between {
                lobby.between = false;
                lobby.began = world.resource::<SimClock>().now;
                lobby.humans_since = None;
                world.insert_resource(lobby);
                world.remove_resource::<BetweenMaps>();
                info!(
                    "pre-match lobby: the match is ready on {}; waiting for the players (at most {} s)",
                    crate::net::current_map(world),
                    world.resource::<LobbySettings>().max_wait
                );
            }
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
    world.insert_resource(EntryHeld);
    let s = *world.resource::<LobbySettings>();
    let begun = world.get_resource::<LobbiesBegun>().copied().unwrap_or_default().0;
    world.insert_resource(LobbiesBegun(begun + 1));
    world.insert_resource(Lobby {
        began: now,
        humans_since: None,
        countdown_ends: None,
        written: f64::MIN,
        host_start: s.host_start != 0 && begun == 0,
        between: false,
    });
    // The host's lobby shown first gives way to this one (`serve` writes
    // it this tick), its host's start, if asked meanwhile, taken now.
    let asked = world.remove_resource::<StartAsked>();
    hide_first(world);
    info!(
        "pre-match lobby on {}: waiting for the players (at most {} s once one is here)",
        crate::net::current_map(world),
        s.max_wait
    );
    if let Some(StartAsked(secs)) = asked {
        let _ = start_now(world, secs);
    }
}

/// A match's change to another map (`minigame_net::change_match`: a
/// rotation after the match-end screen, a vote's map, a host's
/// `changelevel`) with the lobby on: the lobby begins at once, between
/// the maps, naming the map coming in (`map`) and the mode (`mode`), every
/// player's row loading it while it is fetched and loaded on every
/// machine; when the match is made there (`begin`) the same lobby goes on
/// as its pre-match lobby. A change while a lobby is up takes it over.
pub fn begin_between(world: &mut World, map: &str, mode: &str) {
    if !lobby_on(world) {
        // A match entered at once (`MinigameDef::lobby` false): a lobby
        // up for the match left goes with it.
        if world.contains_resource::<Lobby>() {
            end(world, false);
        }
        return;
    }
    let between = BetweenMaps { map: crate::net::normalize_map(map), mode: mode.to_string() };
    if world.get_resource::<Lobby>().is_some_and(|l| l.between) {
        world.insert_resource(between);
        return;
    }
    if world.contains_resource::<Lobby>() {
        end(world, false);
    }
    let now = world.resource::<SimClock>().now;
    let readied: Vec<Entity> = world.query_filtered::<Entity, With<LobbyReady>>().iter(world).collect();
    for e in readied {
        world.entity_mut(e).remove::<LobbyReady>();
    }
    // Nobody plays while it shows: everyone staged (`PreMatch`) from now.
    world.insert_resource(PreMatch);
    world.insert_resource(EntryHeld);
    let s = *world.resource::<LobbySettings>();
    let begun = world.get_resource::<LobbiesBegun>().copied().unwrap_or_default().0;
    world.insert_resource(LobbiesBegun(begun + 1));
    world.insert_resource(Lobby {
        began: now,
        humans_since: None,
        countdown_ends: None,
        written: f64::MIN,
        host_start: s.host_start != 0 && begun == 0,
        between: true,
    });
    hide_first(world);
    info!("pre-match lobby between maps: {} coming in", between.map);
    world.insert_resource(between);
}

/// The host's start asked in its lobby shown first (`start_now`), for
/// when the server's lobby begins.
#[derive(Resource, Clone, Copy, Debug)]
struct StartAsked(Option<f64>);

/// A listen host's lobby shown from its Start, before its map is in
/// (`show_first`): its own row loading, nothing replicated. The server's
/// lobby takes its place when it begins (`begin`).
#[derive(Component, Clone, Copy, Debug)]
pub struct HostLobbyFirst;

/// A listen host's lobby at once (the party apps' Host's Start with the
/// lobby on): the map `map` and mode `mode` with the host (`host`, its
/// name) loading, shown while the map loads and the match is made behind
/// it, so the host goes from the menu to the lobby and not through the
/// game. The server's lobby replaces it when it begins (`begin`);
/// `hide_first` takes it away (a cancelled or failed load).
pub fn show_first(world: &mut World, map: &str, mode: &str, host: &str) {
    hide_first(world);
    world.spawn((
        Name::new("Pre-match lobby (the host's map loading)"),
        HostLobbyFirst,
        NetLobby {
            map: crate::net::normalize_map(map),
            mode: mode.to_string(),
            rows: vec![LobbyRow {
                id: Some(HOST_ID),
                name: host.to_string(),
                status: status::LOADING,
                step: NO_STEP,
                avatar: NO_AVATAR,
                ..default()
            }],
            waiting: 1,
            wait_left: None,
            countdown: None,
            host_start: true,
            settings: Vec::new(),
        },
    ));
}

/// A bot the game keeps but hasn't added yet (`BotHooks::quota`): ready,
/// as bots are.
fn bot_to_come() -> LobbyRow {
    LobbyRow { id: None, name: "Bot".into(), status: status::READY, step: NO_STEP, avatar: NO_AVATAR, ..default() }
}

/// The host's lobby shown first (`show_first`) lists the game's bots from
/// the start (its quota, set by the Start's lines a frame after it shows).
fn first_bots(world: &mut World) {
    let mut q = world.query_filtered::<&NetLobby, With<HostLobbyFirst>>();
    let Some(lobby) = q.iter(world).next() else { return };
    let have = lobby.rows.iter().filter(|r| r.id.is_none()).count();
    let want = (crate::net::BotHooks::active(world).quota)(world) as usize;
    if have == want {
        return;
    }
    let mut q = world.query_filtered::<&mut NetLobby, With<HostLobbyFirst>>();
    for mut lobby in q.iter_mut(world) {
        lobby.rows.retain(|r| r.id.is_some());
        lobby.rows.extend((0..want).map(|_| bot_to_come()));
    }
}

/// The host's lobby shown first (`show_first`) taken away, with a start
/// asked in it.
pub fn hide_first(world: &mut World) {
    world.remove_resource::<StartAsked>();
    let first: Vec<Entity> = world.query_filtered::<Entity, With<HostLobbyFirst>>().iter(world).collect();
    for e in first {
        world.despawn(e);
    }
}

/// The lobby gone: `PreMatch` lifted and `NetLobby` despawned; `start`:
/// the match begins (`minigame::restart`: everyone spawns together).
fn end(world: &mut World, start: bool) {
    world.remove_resource::<Lobby>();
    world.remove_resource::<BetweenMaps>();
    world.remove_resource::<EntryHeld>();
    let had = world.remove_resource::<PreMatch>().is_some();
    let entities: Vec<Entity> = world.query_filtered::<Entity, With<NetLobby>>().iter(world).collect();
    for e in entities {
        world.despawn(e);
    }
    if start && had && world.contains_resource::<ActiveMinigame>() {
        minigame::restart(world);
        info!("pre-match lobby over: the match starts");
    }
    let_in(world);
}

/// Everyone the lobby held out (`minigame::Staged`) let in: into the game
/// by the join rules (`JoinRules::enter`), whereupon the runtime takes
/// them in and spawns them together on the first rung; a remote player
/// whose client hasn't the map yet stays out of the game
/// (`core::Connecting`) until it reports it loaded (`server::
/// receive_loaded`), as a late joiner.
fn let_in(world: &mut World) {
    let staged: Vec<Entity> = world.query_filtered::<Entity, With<minigame::Staged>>().iter(world).collect();
    if staged.is_empty() {
        return;
    }
    let map = serving_map(world);
    let loading: Vec<Entity> = world
        .query::<(&Player, Option<&ClientMap>)>()
        .iter(world)
        .filter(|(_, have)| !have.is_some_and(|m| m.0.eq_ignore_ascii_case(&map)))
        .map(|(p, _)| p.character)
        .collect();
    let rules = crate::net::JoinRules::active(world);
    for e in staged {
        minigame::runtime::unstage(world, e);
        if !loading.contains(&e) {
            (rules.enter)(world, e);
        }
    }
}

/// The map the match is on (the loaded one: `maps::ServedMap` follows it
/// a frame later, and a client's `ClientMap` of the map left mustn't
/// count).
fn serving_map(world: &World) -> String {
    match world.get_resource::<BetweenMaps>() {
        Some(b) => b.map.clone(),
        None => crate::net::current_map(world),
    }
}

/// The lobby's map is the one in here (not yet, while the server changes
/// to it).
fn map_in(world: &World) -> bool {
    let want = serving_map(world);
    crate::net::maps::loaded_map(world).is_some_and(|(id, _)| id.eq_ignore_ascii_case(&want))
}

/// The server's small number for each party with two or more players
/// here (`LobbyRow::party`, 1-255), kept while any of it stays.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct PartyNumbers(pub std::collections::BTreeMap<String, u8>);

impl PartyNumbers {
    /// Numbers for the parties in `seen` (one entry per player) with two
    /// or more players; the others' freed.
    pub fn update<'a>(&mut self, seen: impl IntoIterator<Item = &'a str>) {
        let mut count = std::collections::BTreeMap::<&str, usize>::new();
        for p in seen {
            *count.entry(p).or_default() += 1;
        }
        self.0.retain(|p, _| count.get(p.as_str()).is_some_and(|n| *n >= 2));
        for (p, n) in count {
            if n >= 2 && !self.0.contains_key(p) {
                let used: Vec<u8> = self.0.values().copied().collect();
                if let Some(free) = (1..=255u8).find(|k| !used.contains(k)) {
                    self.0.insert(p.to_string(), free);
                }
            }
        }
    }

    pub fn of(&self, party: Option<&str>) -> u8 {
        party.and_then(|p| self.0.get(p).copied()).unwrap_or(0)
    }
}

/// Every player now: the host, the remote players (admitted), the bots.
pub fn rows(world: &mut World) -> Vec<LobbyRow> {
    let map = serving_map(world);
    let mut out = Vec::new();
    let mut others = Vec::new();
    // A listen host is ready once its own load is done (spawned and
    // settled behind its loading screen).
    let host = if crate::net::maps::local_load_under_way(world) || !map_in(world) { status::LOADING } else { status::READY };
    let (host_step, host_permille) = match crate::net::load_stages::current(world).filter(|_| host == status::LOADING) {
        Some((step, share)) => (step.index(), crate::net::load_stages::permille(share)),
        None => (NO_STEP, 0),
    };
    let ready_up = world.get_resource::<LobbySettings>().is_some_and(|s| s.ready_up != 0);
    let mut avatars = std::collections::BTreeMap::new();
    // Each player's party (`cl_party`), numbered for the rows.
    let mut parties = std::collections::BTreeMap::<u64, String>::new();
    for (c, info) in world.query::<(&NetCharacter, Option<&crate::core::UserInfo>)>().iter(world) {
        if let (Some(id), Some(p)) = (c.owner, info.and_then(|i| i.party())) {
            parties.insert(id, p.to_string());
        }
    }
    let mut numbers = world.get_resource::<PartyNumbers>().cloned().unwrap_or_default();
    numbers.update(parties.values().map(String::as_str));
    if world.get_resource::<PartyNumbers>() != Some(&numbers) {
        world.insert_resource(numbers.clone());
    }
    let party_of = |id: u64| numbers.of(parties.get(&id).map(String::as_str));
    for (c, info) in world.query::<(&NetCharacter, Option<&crate::core::UserInfo>)>().iter(world) {
        match c.owner {
            Some(HOST_ID) => out.push(LobbyRow {
                id: Some(HOST_ID),
                name: c.name.clone(),
                status: host,
                step: host_step,
                permille: host_permille,
                avatar: avatar_of(info),
                party: party_of(HOST_ID),
                ..default()
            }),
            None => others.push(LobbyRow {
                id: None,
                name: c.name.clone(),
                status: status::READY,
                step: NO_STEP,
                avatar: NO_AVATAR,
                ..default()
            }),
            Some(id) => {
                avatars.insert(id, avatar_of(info));
            }
        }
    }
    // The bots the game keeps, listed at once: those not in yet (added
    // after the map is in) stand as "Bot" (`BotHooks::quota`).
    let quota = (crate::net::BotHooks::active(world).quota)(world) as usize;
    others.extend((others.len()..quota).map(|_| bot_to_come()));
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
            // The stage the client told, while it downloads or loads.
            let (step, permille) = if matches!(status, status::DOWNLOADING | status::LOADING) && r.step != NO_STEP {
                (r.step, r.permille)
            } else {
                (NO_STEP, 0)
            };
            (
                id.get(),
                LobbyRow {
                    id: Some(id.get()),
                    name: p.name.clone(),
                    status,
                    done,
                    total,
                    step,
                    permille,
                    ping_ms: stats.map(|s| (s.rtt * 1000.0).round().clamp(0.0, 9999.0) as u16),
                    wins: 0,
                    avatar: avatars.get(&id.get()).copied().unwrap_or(NO_AVATAR),
                    party: party_of(id.get()),
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
    // Between maps: the rows only, until the match is made there (`begin`);
    // a change called off (the match no longer held) ends it.
    if lobby.between {
        if !minigame::held(world) {
            end(world, false);
            return;
        }
        let now = world.resource::<SimClock>().now;
        let rows = rows(world);
        let between = world.resource::<BetweenMaps>().clone();
        let to_come = world.resource::<LobbySettings>().min_players.saturating_sub(rows.iter().filter(|r| !r.bot()).count() as u32);
        let want = NetLobby {
            map: between.map,
            mode: between.mode,
            waiting: rows.iter().filter(|r| !r.ready()).count() as u32 + to_come,
            rows,
            wait_left: None,
            countdown: lobby.countdown_ends.map(|t| (t - now).max(0.0).ceil() as u32),
            host_start: lobby.host_start && lobby.countdown_ends.is_none(),
            settings: Vec::new(),
        };
        let want = NetLobby { settings: settings_now(world, &want.mode), ..want };
        write_lobby(world, &mut lobby, want, now);
        world.insert_resource(lobby);
        return;
    }
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
    // The server's own game files for the match first (`server_assets`).
    let files_coming = crate::net::server_assets::fetching(world);
    if lobby.countdown_ends.is_none() && !lobby.host_start && !files_coming {
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
    if lobby.countdown_ends.is_some_and(|t| now + 1e-9 >= t) && !files_coming {
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
        wait_left: wait_ends.filter(|_| lobby.countdown_ends.is_none() && !lobby.host_start).map(secs),
        countdown: lobby.countdown_ends.map(secs),
        host_start: lobby.host_start && lobby.countdown_ends.is_none(),
        settings: Vec::new(),
    };
    let want = NetLobby { settings: settings_now(world, &want.mode), ..want };
    write_lobby(world, &mut lobby, want, now);
    world.insert_resource(lobby);
}

/// `NetLobby` written as `want` (spawned the first time): statuses,
/// counts and seconds at once, bytes, shares and pings at most every
/// `WRITE_EVERY` s.
fn write_lobby(world: &mut World, lobby: &mut Lobby, want: NetLobby, now: f64) {
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
                let rows: Vec<(Option<u64>, String, u8, u8)> =
                    l.rows.iter().map(|r| (r.id, r.name.clone(), r.status, r.step)).collect();
                (rows, l.waiting, l.wait_left, l.countdown, l.map.clone(), l.mode.clone())
            };
            if coarse(&n) != coarse(&want) || now - lobby.written >= WRITE_EVERY {
                world.entity_mut(e).insert(want);
                lobby.written = now;
            }
        }
        Some(_) => {}
    }
}

/// `lobby_start [s]`: the countdown now (`s` s, default the cvar's; 0:
/// the match at once).
pub fn start_now(world: &mut World, secs: Option<f64>) -> Result<String, String> {
    if world.get_resource::<NetRole>() == Some(&NetRole::Client) {
        return Err("the server starts the match; you are a client of it".into());
    }
    let Some(mut lobby) = world.get_resource::<Lobby>().copied() else {
        // The host's lobby shown first (its map still loading): the start
        // waits for the server's lobby (`begin`).
        if world.query_filtered::<(), With<HostLobbyFirst>>().iter(world).next().is_some() {
            world.insert_resource(StartAsked(secs));
            return Ok("the match starts once the map is in".into());
        }
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
        let mut bytes = if r.status == status::DOWNLOADING {
            format!(" {:.1}/{:.1} MB", r.done as f64 / 1e6, r.total as f64 / 1e6)
        } else {
            String::new()
        };
        if let Some((step, share)) = r.stage() {
            bytes += &format!(" ({} {:.0}%)", step.words(), share * 100.0);
        }
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

    /// A report carries the stage under way: a download's bytes, a load's
    /// stage and share; the next map fetched ahead while joined is a
    /// download.
    #[test]
    fn a_report_carries_the_stage_under_way() {
        use crate::net::load_stages::{LoadStep, Stage, StageState};
        let st = |step, state, bytes| Stage { step, state, bytes, files: None };
        let r = report_of(JoinStage::LoadingMap, &[
            st(LoadStep::Download, StageState::Done, Some((10, 10))),
            st(LoadStep::Map, StageState::Done, None),
            st(LoadStep::Models, StageState::Running(Some(0.25)), None),
        ]);
        assert_eq!((r.status, r.step, r.permille, r.done, r.total), (status::LOADING, 2, 250, 0, 0));
        let r = report_of(JoinStage::Joined, &[st(LoadStep::Download, StageState::Running(Some(0.5)), Some((5, 10)))]);
        assert_eq!((r.status, r.step, r.permille, r.done, r.total), (status::DOWNLOADING, 0, 500, 5, 10));
        let r = report_of(JoinStage::Joined, &[]);
        assert_eq!((r.status, r.step), (status::READY, NO_STEP));
    }
}

//! The match's game files from the host's asset service
//! (docs/plans/active/asset-service.md, "Playtest delivery": the plan's
//! mode 1 "lite"): a client with no install of a game joins and gets that
//! game's files over HTTPS from the service the server names, with progress
//! on the loading screen, before it spawns.
//!
//! - **The server** names its asset service with `chimera:asset_url`
//!   (`https://<public address>:<port>`) and `chimera:asset_fingerprint`
//!   (its certificate's SHA-256; empty: the certificate of the asset
//!   service on this machine, `chimera_assets::server::tls_dir`). The match
//!   manifest then carries an `AssetOffer` (`offer`): the URL and
//!   fingerprint, and the sets the match needs now per game (`base`,
//!   `players`, the map's `map/<name>`, each ladder and kit weapon's
//!   `weapon/<name>`: the weapons and pool come from `AssetHooks`, which
//!   `minigame_net` fills) and later (the pool's other maps, prefetched).
//!   A joining client that says it can fetch (`ASSETS_KEY` in its userinfo:
//!   it has a token) counts as having the offered games (`count_offered`),
//!   so a client with no install isn't refused before it hears the offer.
//! - **The client** (`begin`, before it accepts a `Welcome` or
//!   `ChangeLevel`): a game its own install provides is never fetched (the
//!   hybrid model). One the process reads from the asset store alone
//!   (`AssetSession::served`, opened at startup from a bound access or
//!   `[assets]`) has its offered sets fetched on a thread (`fetch`: the
//!   service's manifest, the sets' records the store lacks, in batches:
//!   `chimera_assets::sets::fetch`),
//!   the bytes shown as the join's `Downloading` stage (`DownloadInfo`,
//!   "game files"); then the join goes on (`client::proceed`). The later
//!   sets are fetched once in the game (`poll_prefetch`, one set at a time)
//!   at a capped rate (`pace_prefetch`): the client's
//!   `chimera:asset_prefetch_rate` (KB/s, `PREFETCH_RATE_DEFAULT`; 0: none),
//!   at most the server's `chimera:asset_prefetch_max` (in the offer,
//!   `AssetOffer::prefetch_rate`; 0: no prefetch for anyone), held while the
//!   client is joining or changing level (a foreground fetch, a map load).
//!   A game neither installed nor served: with a token
//!   (`chimera_assets::access`, `chimera:asset_token`), the token is bound
//!   to the offered service and the game starts again joining the same
//!   server (`AssetRelaunch`, which the client app acts on: the mounts are opened at startup, so a
//!   game that had no files then gets them only in a new process); without
//!   one, the join fails saying how to enter it. A game read from the
//!   store with no service and a token bound to none (entered again on
//!   this start, which forgets the binding) is bound and started again the
//!   same way, else nothing new would come. A refused token, an
//!   unreachable service or another certificate than the bound one fails
//!   the join with the reason on screen (`JoinFailure::Download`).
//!
//! The startup fetch (what the games read before the window, behind the
//! client's progress window) is `asset_startup`'s.
//!
//! Known shortcuts: the first join with a new token restarts the game
//! once; sets the service doesn't have are skipped (their files come on
//! demand as the loaders read them); files fetched on demand (a map's
//! load) don't hold the prefetch, only the join and level change around
//! them do.

use std::{
    collections::BTreeSet,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_assets::{
    AssetSettings,
    access::Access,
    pace::Pace,
    progress::Transfer,
    sets::{self, Wanted},
};
use chimera_core::{
    console::{ConsoleAppExt, resource_cvar},
    game_registry::Games,
};

use super::{
    ChangeLevel, Welcome,
    client::{self, DownloadInfo, JoinFailure, JoinProgress, JoinStage},
    manifest::{GameMount, LocalMounts, MatchManifest, MountReport},
};
use crate::core::NetRole;

/// The server's asset service URL (cvar).
pub const URL_CVAR: &str = "chimera:asset_url";
/// Its certificate's fingerprint (cvar).
pub const FINGERPRINT_CVAR: &str = "chimera:asset_fingerprint";
/// The server's cap on every client's prefetch, KB/s (cvar; negative:
/// none, 0: no prefetch).
pub const PREFETCH_MAX_CVAR: &str = "chimera:asset_prefetch_max";
/// The client's prefetch rate, KB/s (cvar, archived; 0: no prefetch).
pub const PREFETCH_RATE_CVAR: &str = "chimera:asset_prefetch_rate";
/// `PREFETCH_RATE_CVAR`'s default: 2 MB/s.
pub const PREFETCH_RATE_DEFAULT: u32 = 2000;
/// Enter this player's asset token (command).
pub const TOKEN_COMMAND: &str = "chimera:asset_token";
/// The userinfo key of a client that can fetch from an offered service.
pub const ASSETS_KEY: &str = "_assets";
/// What the loading screen calls the download.
pub const FILES: &str = "game files";

/// What a match's files are and where to get them (in `MatchManifest`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetOffer {
    /// `https://host:port`.
    pub url: String,
    /// The service's certificate's SHA-256, hex.
    pub fingerprint: String,
    /// What the match needs before a player spawns, per game.
    pub now: Vec<GameSets>,
    /// What later matches may need (the pool's other maps): prefetched.
    pub later: Vec<GameSets>,
    /// The server's cap on the prefetch, KB/s (`PREFETCH_MAX_CVAR`; 0:
    /// none); None: the client's own rate.
    pub prefetch_rate: Option<u32>,
}

/// The prefetch's rate in KB/s: the client's, at most the server's cap
/// (0: no prefetch).
pub fn prefetch_rate(client_kb: u32, server_kb: Option<u32>) -> u32 {
    server_kb.map_or(client_kb, |s| client_kb.min(s))
}

/// On a client: the prefetch rate, KB/s (`PREFETCH_RATE_CVAR`).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefetchRate(pub u32);

impl Default for PrefetchRate {
    fn default() -> Self {
        Self(PREFETCH_RATE_DEFAULT)
    }
}

/// Sets of one game (`base`, `map/de_dust2`, `weapon/ak47`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct GameSets {
    pub game: String,
    pub sets: Vec<String>,
}

fn add(list: &mut Vec<GameSets>, game: &str, set: String) {
    let i = match list.iter().position(|g| g.game == game) {
        Some(i) => i,
        None => {
            list.push(GameSets {
                game: game.to_string(),
                sets: Vec::new(),
            });
            list.len() - 1
        }
    };
    if !list[i].sets.contains(&set) {
        list[i].sets.push(set);
    }
}

impl AssetOffer {
    /// The games it offers now.
    pub fn games(&self) -> impl Iterator<Item = &str> {
        self.now.iter().map(|g| g.game.as_str())
    }
}

/// On a server: where this host's asset service is (`URL_CVAR`,
/// `FINGERPRINT_CVAR`). Empty URL: none offered.
#[derive(Resource, Clone, Debug)]
pub struct AssetHost {
    pub url: String,
    pub fingerprint: String,
    /// The cap on every client's prefetch, KB/s (`PREFETCH_MAX_CVAR`;
    /// negative: none).
    pub prefetch_max: i64,
    /// Said once that no fingerprint was found.
    warned: bool,
}

impl Default for AssetHost {
    fn default() -> Self {
        Self::new("", "")
    }
}

impl AssetHost {
    /// The service at `url` with the certificate `fingerprint` (empty: the
    /// one on this machine).
    pub fn new(url: impl Into<String>, fingerprint: impl Into<String>) -> Self {
        Self { url: url.into(), fingerprint: fingerprint.into(), prefetch_max: -1, warned: false }
    }

    /// The fingerprint to hand out: the cvar's, else that of the asset
    /// service's certificate on this machine.
    pub fn fingerprint(&self) -> Option<String> {
        if !self.fingerprint.trim().is_empty() {
            return chimera_assets::tls::normalize_fingerprint(&self.fingerprint);
        }
        let store = chimera_assets::server::default_store()?;
        chimera_assets::tls::fingerprint_in(&chimera_assets::server::tls_dir(&store))
    }
}

/// On a client: the asset settings this process opened its mounts with,
/// and which games it reads from the store alone (made at the first join:
/// `session`; tests insert their own).
#[derive(Resource, Clone, Debug, Default)]
pub struct AssetSession {
    /// The store and the service (`remote`), as `mount::served` resolved
    /// them.
    pub settings: Option<AssetSettings>,
    /// Games whose files come from the store (and the service) alone.
    pub served: BTreeSet<String>,
    /// The access file (None: the user's, `Access::path`).
    pub access: Option<PathBuf>,
}

impl AssetSession {
    fn access_path(&self) -> Option<PathBuf> {
        self.access.clone().or_else(Access::path)
    }

    /// Whether this client can fetch from an offered service (it has a
    /// service or a token).
    pub fn can_fetch(&self) -> bool {
        self.settings.as_ref().is_some_and(|s| s.remote.is_some())
            || self
                .access_path()
                .is_some_and(|p| Access::load_from(&p).token.is_some())
    }
}

/// The client's session, made from the local config and the registered
/// games the first time it is asked.
pub fn session(world: &mut World) -> AssetSession {
    if let Some(s) = world.get_resource::<AssetSession>() {
        return s.clone();
    }
    let settings = chimera_core::mount::config::LocalConfig::load()
        .ok()
        .map(|c| c.assets.resolve());
    let served = world
        .get_resource::<Games>()
        .map(|g| g.all().filter(|g| g.served_alone()).map(|g| g.id.to_string()).collect())
        .unwrap_or_default();
    let s = AssetSession {
        settings,
        served,
        access: None,
    };
    world.insert_resource(s.clone());
    s
}

/// On a client: the game ends to start again with the asset service it
/// was just given (`begin`); the server it was joining, if known. The
/// client app starts the new process and quits
/// (`chimera_client::client::relaunch`).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct AssetRelaunch(pub Option<SocketAddr>);

/// Extra things a match's offer names, from the parts that know them
/// (`minigame_net`: its ladder's and kit's weapons, `game:weapon`, and its
/// map pool): registered per part, asked in order.
#[derive(Resource, Clone, Default)]
pub struct AssetHooks(pub Vec<fn(&World) -> (Vec<String>, Vec<String>)>);

/// What `begin` decided.
pub(super) enum Next {
    /// Go on joining (nothing to fetch).
    Go,
    /// Fetching: the join goes on when it's done (`poll_fetch`).
    Wait,
    /// Left (failed, or ending to start again).
    Left,
}

/// What joining does once the files are here.
#[derive(Clone, Debug)]
pub(super) enum Then {
    Welcome(Welcome),
    Change(ChangeLevel),
}

/// A fetch under way.
#[derive(Resource)]
struct AssetFetch {
    then: Then,
    job: Option<JoinHandle<Result<String, String>>>,
    transfer: Arc<Mutex<Option<Arc<Transfer>>>>,
    from: String,
}

/// The later sets to prefetch once in the game, the server's cap, and the
/// prefetch running (its pace: `pace_prefetch` sets its rate and pause).
#[derive(Resource, Default)]
struct Prefetch {
    waiting: Vec<GameSets>,
    cap: Option<u32>,
    running: Option<(JoinHandle<()>, Arc<Pace>)>,
}

/// On a client: the prefetch's state, for tests and the console.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrefetchState {
    /// The rate it reads at, KB/s (0: none).
    pub rate_kb: u32,
    /// Running now (a set being fetched).
    pub running: bool,
    /// Held: the client is joining or changing level.
    pub paused: bool,
    /// Sets still to start.
    pub waiting: usize,
}

/// The prefetch as it stands (None: nothing offered).
pub fn prefetch_state(world: &World) -> Option<PrefetchState> {
    let p = world.get_resource::<Prefetch>()?;
    let client = world.get_resource::<PrefetchRate>().copied().unwrap_or_default().0;
    let running = p.running.as_ref().filter(|(j, _)| !j.is_finished());
    Some(PrefetchState {
        rate_kb: prefetch_rate(client, p.cap),
        running: running.is_some(),
        paused: running.is_some_and(|(_, pace)| pace.paused()),
        waiting: p.waiting.iter().map(|g| g.sets.len()).sum(),
    })
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<AssetHost>()
        .init_resource::<AssetHooks>()
        .init_resource::<PrefetchRate>();
    resource_cvar::<AssetHost, String>(
        app,
        URL_CVAR,
        "Server: the asset service clients without a game's install fetch its files from (https://<public address>:<port>); empty: none.",
        |h| &mut h.url,
    );
    resource_cvar::<AssetHost, String>(
        app,
        FINGERPRINT_CVAR,
        "Server: the asset service's certificate fingerprint (chimera_asset_server prints it); empty: the one on this machine.",
        |h| &mut h.fingerprint,
    );
    resource_cvar::<AssetHost, i64>(
        app,
        PREFETCH_MAX_CVAR,
        "Server: the most KB/s each client prefetches the pool's other maps at, in the background (0: no prefetch; \
         -1: the clients' own rate).",
        |h| &mut h.prefetch_max,
    );
    resource_cvar::<PrefetchRate, u32>(
        app,
        PREFETCH_RATE_CVAR,
        "KB/s the game fetches later maps' files at in the background, while playing (0: none; a server may cap it).",
        |r| &mut r.0,
    );
    app.world_mut()
        .resource_mut::<chimera_core::console::Console>()
        .archive(PREFETCH_RATE_CVAR);
    app.console_command(
        TOKEN_COMMAND,
        "Enter your asset token (from the asset service's admin), kept for this user; the next server offering its service binds it.",
        |world, args| {
            let token = args.get(1).ok_or("usage: chimera:asset_token <token>")?;
            let path = world.get_resource::<AssetSession>().and_then(|s| s.access.clone()).or_else(Access::path);
            let path = path.ok_or("no per-user data folder")?;
            save_token(&path, token)?;
            Ok(Some(format!("asset token saved ({})", path.display())))
        },
    );
    app.add_systems(
        Update,
        (poll_fetch, poll_prefetch, pace_prefetch)
            .chain()
            .run_if(resource_equals(NetRole::Client)),
    );
}

/// The user's access file (`chimera_assets::access`).
pub fn access_path() -> Option<PathBuf> {
    Access::path()
}

/// Whether this user has entered an asset token.
pub fn has_token() -> bool {
    access_path().is_some_and(|p| Access::load_from(&p).token.is_some())
}

/// Keep `token` in the access file at `path` (any binding forgotten).
pub fn save_token(path: &std::path::Path, token: &str) -> Result<(), String> {
    let mut access = Access::load_from(path);
    access.set_token(token)?;
    access.save_to(path)
}

// --- The server.

/// The server's offer for manifest `m` (built from the rest of it): none
/// without `chimera:asset_url`.
pub fn offer(world: &mut World, m: &mut MatchManifest) {
    let host = world.get_resource::<AssetHost>().cloned().unwrap_or_default();
    let url = host.url.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        return;
    }
    let Some(fingerprint) = host.fingerprint() else {
        if let Some(mut h) = world.get_resource_mut::<AssetHost>().filter(|h| !h.warned) {
            h.warned = true;
            warn!(
                "{URL_CVAR} is set, but there is no certificate fingerprint ({FINGERPRINT_CVAR}, or an asset service \
                 on this machine): no game files offered"
            );
        }
        return;
    };
    let games = world.get_resource::<Games>();
    let known = |g: &str| games.is_none_or(|gs| gs.get(g).is_some());
    let mut offer = AssetOffer {
        url,
        fingerprint,
        prefetch_rate: u32::try_from(host.prefetch_max).ok(),
        ..default()
    };
    let mut in_play: Vec<String> = m.games.iter().map(|g| g.game.clone()).collect();
    in_play.extend(m.weapons.iter().cloned());
    let (mut weapons, mut pool) = (Vec::new(), Vec::new());
    for hook in world.get_resource::<AssetHooks>().cloned().unwrap_or_default().0 {
        let (w, p) = hook(world);
        weapons.extend(w);
        pool.extend(p);
    }
    in_play.extend(
        weapons
            .iter()
            .filter_map(|w| w.split_once(':').map(|(g, _)| g.to_string())),
    );
    for g in in_play.iter().filter(|g| known(g)) {
        add(&mut offer.now, g, "base".into());
        add(&mut offer.now, g, "players".into());
    }
    if let Some((game, name)) = m.map.split_once(':')
        && known(game)
    {
        add(&mut offer.now, game, format!("map/{name}"));
    }
    for (game, name) in weapons
        .iter()
        .filter_map(|w| w.split_once(':'))
        .filter(|(g, _)| known(g))
    {
        add(&mut offer.now, game, format!("weapon/{name}"));
        // CS:S's sets are named without the `weapon_` its ids start with.
        if let Some(short) = name.strip_prefix("weapon_") {
            add(&mut offer.now, game, format!("weapon/{short}"));
        }
    }
    for map in pool {
        if let Some((game, name)) = map.split_once(':')
            && known(game)
            && map != m.map
        {
            add(&mut offer.later, game, format!("map/{name}"));
        }
    }
    if !offer.now.is_empty() {
        m.assets = Some(offer);
    }
}

/// A joining client that can fetch (`ASSETS_KEY`) counts as having the
/// games `manifest` offers.
pub fn count_offered(manifest: &MatchManifest, userinfo: &[(String, String)], report: &mut MountReport) {
    let Some(offer) = &manifest.assets else { return };
    if !userinfo
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case(ASSETS_KEY) && v == "1")
    {
        return;
    }
    for g in offer.games() {
        if !report.has(g) {
            report.games.push(GameMount {
                game: g.to_string(),
                installed: true,
                build: None,
                notes: Vec::new(),
            });
        }
    }
}

/// What a refusal for a missing game adds when the server offers its game
/// files: how to get them.
pub fn refusal_hint(manifest: &MatchManifest) -> Option<String> {
    manifest.assets.as_ref().map(|_| {
        format!(
            " This server offers its game files: start the game once with +{TOKEN_COMMAND} <your token> (the \
             server's admin gives you one), then join again."
        )
    })
}

// --- The client.

/// Before accepting `manifest` (with `then` to do after): fetch what its
/// offer needs of the games this process reads from the store, or start
/// again with a newly bound service, or say why not.
pub(super) fn begin(world: &mut World, manifest: &MatchManifest, then: Then) -> Next {
    let Some(offer) = manifest.assets.clone() else {
        return Next::Go;
    };
    let session = session(world);
    let local = world.get_resource::<LocalMounts>().map(|l| l.0.clone());
    // Games this process has no files of at all (a client without a report
    // has everything).
    let missing: Vec<String> = offer
        .games()
        .filter(|g| local.as_ref().is_some_and(|l| !l.has(g)))
        .map(String::from)
        .collect();
    if !missing.is_empty() {
        return without_files(world, &offer, &session, &missing);
    }
    let served = |g: &GameSets| session.served.contains(&g.game);
    // Games read from the store alone with no service to fill it, and a
    // token not bound to any: the token was entered again on this start
    // (which forgets its binding), so the store holds what an earlier
    // session fetched and nothing more would come. Bind it and start
    // again, as for a game with no files (2026-11-11 rehearsal: a tester
    // starting with the same `+chimera:asset_token` line twice joined
    // with no fetching at all, every new file a miss).
    if !session.settings.as_ref().is_some_and(|s| s.remote.is_some()) {
        let unfilled: Vec<String> = offer
            .games()
            .filter(|g| session.served.contains(*g))
            .map(String::from)
            .collect();
        let unbound = session.access_path().is_some_and(|p| {
            let a = Access::load_from(&p);
            a.token.is_some() && a.remote().is_none()
        });
        if !unfilled.is_empty() && unbound {
            return without_files(world, &offer, &session, &unfilled);
        }
    }
    let running = world.remove_resource::<Prefetch>().and_then(|p| p.running);
    world.insert_resource(Prefetch {
        waiting: offer.later.iter().filter(|g| served(g)).cloned().collect(),
        cap: offer.prefetch_rate,
        running,
    });
    let wants: Vec<GameSets> = offer.now.iter().filter(|g| served(g)).cloned().collect();
    let Some(settings) = session
        .settings
        .clone()
        .filter(|s| s.remote.is_some() && s.store.is_some())
    else {
        return Next::Go;
    };
    if wants.is_empty() {
        return Next::Go;
    }
    let from = settings.remote.as_ref().map(|r| r.url.clone()).unwrap_or_default();
    info!(
        "fetching the match's {FILES} from {from}: {}",
        wants
            .iter()
            .map(|g| format!("{} ({})", g.game, g.sets.join(", ")))
            .collect::<Vec<_>>()
            .join("; ")
    );
    let transfer: Arc<Mutex<Option<Arc<Transfer>>>> = Arc::default();
    let shared = transfer.clone();
    let job = std::thread::spawn(move || {
        sets::fetch(&settings, &wanted(&wants), FILES, &shared, sets::Options::default())
    });
    client::set_stage(world, JoinStage::Downloading);
    world.resource_mut::<JoinProgress>().download = Some(DownloadInfo {
        file: FILES.into(),
        from: "asset service".into(),
        done: 0,
        total: 0,
    });
    world.insert_resource(AssetFetch {
        then,
        job: Some(job),
        transfer,
        from,
    });
    Next::Wait
}

/// The offer names games this process has no files of.
fn without_files(world: &mut World, offer: &AssetOffer, session: &AssetSession, missing: &[String]) -> Next {
    let names = missing
        .iter()
        .map(|g| super::manifest::game_name(world.get_resource::<Games>(), g))
        .collect::<Vec<_>>()
        .join(" and ");
    let Some(path) = session.access_path() else {
        client::fail(
            world,
            JoinFailure::Mounts,
            &format!("{}{names}: no per-user data folder", super::manifest::MISSING),
        );
        return Next::Left;
    };
    let mut access = Access::load_from(&path);
    if access.token.is_none() {
        let why = format!(
            "{}{names}, which isn't installed here. The server offers its game files: start the game once with \
             +{TOKEN_COMMAND} <your token> (the server's admin gives you one), then join again.",
            super::manifest::MISSING
        );
        client::fail(world, JoinFailure::Mounts, &why);
        return Next::Left;
    }
    let using = session.settings.as_ref().and_then(|s| s.remote.clone());
    let fail = |world: &mut World, error: String| {
        let reason = format!("Couldn't get the match's {FILES}: {error}");
        client::fail(
            world,
            JoinFailure::Download {
                file: FILES.into(),
                error,
            },
            &reason,
        );
        Next::Left
    };
    if let Err(e) = access.bind(&offer.url, &offer.fingerprint) {
        return fail(world, e);
    }
    if let Some(remote) = using.filter(|u| Some(u) == access.remote().as_ref()) {
        // This process already reads from that service, which didn't serve
        // the game when the game started: say why (a refused token, an
        // unreachable service), else that it lacks the game.
        let why = chimera_assets::Remote::shared(&remote)
            .and_then(|r| r.whoami().map_err(|e| e.to_string()))
            .err()
            .map(|e| sets::explain(&offer.url, &e))
            .unwrap_or_else(|| {
                format!("the asset service at {} doesn't serve {names}; tell the server's admin", offer.url)
            });
        return fail(world, why);
    }
    if let Err(e) = access.save_to(&path) {
        return fail(world, e);
    }
    let server = world.get_resource::<client::ServerAddress>().map(|a| a.0);
    info!(
        "the asset token is bound to {} now; the game starts again to read {names} from it{}",
        offer.url,
        server.map_or(String::new(), |s| format!(", joining {s}"))
    );
    world.insert_resource(AssetRelaunch(server));
    // The player's own leaving: no failure to show; the game starts again.
    super::disconnect(world, client::BY_USER);
    Next::Left
}

/// `GameSets` as `sets::fetch` takes them.
fn wanted(list: &[GameSets]) -> Vec<Wanted> {
    list.iter().map(|g| Wanted { game: g.game.clone(), sets: g.sets.clone() }).collect()
}

/// The fetch's progress into the loading screen; once done, the join goes
/// on, or fails with the reason.
fn poll_fetch(world: &mut World) {
    let Some(f) = world.get_resource::<AssetFetch>() else {
        return;
    };
    let transfer = f.transfer.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let finished = f.job.as_ref().is_none_or(|j| j.is_finished());
    if let Some(t) = transfer {
        let (_, bytes, total) = t.done();
        if let Some(d) = world
            .resource_mut::<JoinProgress>()
            .download
            .as_mut()
            .filter(|d| d.file == FILES)
        {
            d.done = bytes;
            d.total = total;
        }
    }
    if !finished {
        return;
    }
    let mut f = world.remove_resource::<AssetFetch>().expect("checked");
    let result = f.job.take().map_or(Err("no fetch".to_string()), |j| {
        j.join().unwrap_or(Err("the fetch panicked".into()))
    });
    match result {
        Ok(summary) => {
            info!("{summary}");
            world.resource_mut::<JoinProgress>().download = None;
            client::proceed(world, f.then);
        }
        Err(error) => {
            let reason = format!("Couldn't get the match's {FILES} from {}: {error}", f.from);
            client::fail(
                world,
                JoinFailure::Download {
                    file: FILES.into(),
                    error,
                },
                &reason,
            );
        }
    }
}

/// In the game: the later sets, one at a time, in the background, at the
/// prefetch's rate (`pace_prefetch`).
fn poll_prefetch(world: &mut World) {
    if !world.contains_resource::<client::Joined>() {
        return;
    }
    let client = world.get_resource::<PrefetchRate>().copied().unwrap_or_default().0;
    let Some(mut p) = world.get_resource_mut::<Prefetch>() else {
        return;
    };
    let rate = prefetch_rate(client, p.cap);
    if rate == 0 || p.running.as_ref().is_some_and(|(j, _)| !j.is_finished()) || p.waiting.is_empty() {
        return;
    }
    let waiting = std::mem::take(&mut p.waiting);
    let settings = world.get_resource::<AssetSession>().and_then(|s| s.settings.clone());
    let Some(settings) = settings.filter(|s| s.remote.is_some()) else {
        return;
    };
    let pace = Arc::new(Pace::kb_per_sec(rate.into()));
    let shared = pace.clone();
    let job = std::thread::spawn(move || {
        for g in waiting {
            for set in &g.sets {
                if shared.stopped() {
                    return;
                }
                let one = [Wanted { game: g.game.clone(), sets: vec![set.clone()] }];
                let options = sets::Options { pace: Some(&shared), ..default() };
                match sets::fetch(&settings, &one, "prefetched files", &Mutex::new(None), options) {
                    Ok(summary) => info!("prefetched {}:{set}: {summary}", g.game),
                    Err(e) => info!("prefetching {}:{set}: {e}", g.game),
                }
            }
        }
    });
    world.resource_mut::<Prefetch>().running = Some((job, pace));
}

/// The running prefetch's rate (the client's, at most the server's cap)
/// and its pause: held while the client isn't in the game (joining,
/// changing level, fetching the match's files in the foreground).
fn pace_prefetch(
    prefetch: Option<Res<Prefetch>>,
    rate: Option<Res<PrefetchRate>>,
    joined: Option<Res<client::Joined>>,
    fetch: Option<Res<AssetFetch>>,
    progress: Option<Res<JoinProgress>>,
) {
    let Some((_, pace)) = prefetch.as_ref().and_then(|p| p.running.as_ref()) else {
        return;
    };
    let cap = prefetch.as_ref().and_then(|p| p.cap);
    let kb = prefetch_rate(rate.map_or(PREFETCH_RATE_DEFAULT, |r| r.0), cap);
    pace.set_rate(u64::from(kb) * 1000);
    let busy = fetch.is_some() || joined.is_none() || progress.is_some_and(|p| !matches!(p.stage, JoinStage::Idle | JoinStage::Joined));
    if pace.paused() != busy {
        pace.pause(busy);
    }
}

/// The client left: no fetch goes on for it; a prefetch stops (its
/// batch cut short).
pub(super) fn reset(world: &mut World) {
    world.remove_resource::<AssetFetch>();
    if let Some(p) = world.remove_resource::<Prefetch>()
        && let Some((_, pace)) = p.running
    {
        pace.stop();
    }
}

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
//!   `players` and `common`: `chimera_assets::resolve::MATCH_SETS`, the
//!   map's `map/<name>`, each ladder and kit weapon's
//!   `weapon/<name>` and the starting weapons a joiner holds until the
//!   minigame arms it: the weapons come from `AssetHooks`, which
//!   `minigame_net` fills; other sets a game names there, Combat Arms'
//!   rung bodies' `character/<record>` and guns' `parts/<id>`). Its
//!   `later` (the pool's other maps, which clients before 2026-11-15
//!   prefetched at 2 MB/s for the whole session) stays empty: the next
//!   map comes when the vote locks it in (below).
//!   A joining client that says it can fetch (`ASSETS_KEY` in its userinfo:
//!   it has a token) counts as having the offered games (`count_offered`),
//!   so a client with no install isn't refused before it hears the offer.
//! - **The client** (`begin`, before it accepts a `Welcome` or
//!   `ChangeLevel`): a game its own install provides is never fetched (the
//!   hybrid model), but for the match's map when the install lacks it (a
//!   community map): the offer's `map/<name>` set, binding the token for it
//!   if needed. Whichever game the map is of, once the sets are here its
//!   file is made sure of (`ensure_map`: kept in the content cache when
//!   the mounts don't give it as the server has it), so the map's verify
//!   step never asks the game server (2026-11-14: "The server has no file
//!   to send for cs_source:gg_mario_vs_wario"). One the process reads from the asset store alone
//!   (`AssetSession::served`, opened at startup from a bound access or
//!   `[assets]`) has its offered sets fetched on a thread (`fetch`: the
//!   service's manifest, the sets' records the store lacks, in batches:
//!   `chimera_assets::sets::fetch`; from `begin` until the fill claims
//!   its records, `pending::starting` holds on-demand misses in its store, so models
//!   the new match starts loading at once wait for the fetch instead of
//!   coming a group each), the bytes shown as the join's `Downloading` stage (`DownloadInfo`,
//!   "game files"); then the join goes on (`client::proceed`). Besides the
//!   offer's sets the client fetches, for the games it reads from the
//!   store, each game's `common` (a server before 2026-11-15 doesn't name
//!   it) and what its parts name for the match (`JoinSets`: Combat Arms'
//!   first music tracks), so nothing the match plays waits for a fetch.
//!   With the pre-match lobby the join's download runs behind the lobby,
//!   before anyone plays.
//! - **The next map, ahead** (`map_ahead`): once the match-end screen's
//!   next map is known (`minigame::NextMatch`: the vote's winner when it
//!   closes, else the rotation's pick), its game's match sets and its
//!   `map/<name>` are fetched at full speed, during the match-end screen
//!   (`MapAhead`); the level change's own download then waits for it and
//!   finds them here.
//! - **Streams** (`Streams`): sets a part asks for in the background, each
//!   due in so many seconds (the next music track, before the one playing
//!   ends): one at a time, paced to finish at about half the time left
//!   (`stream_rate`; at least `STREAM_MIN_RATE`, at most the link's
//!   estimate, `StreamLink`), held while a foreground download runs (a
//!   join, a level change, the next map ahead).
//!   A game neither installed nor served: with a token
//!   (`chimera_assets::access`, `chimera:asset_token`), the token is bound
//!   to the offered service, in the access file and in this process
//!   (`access::bind_live`: mounts opened before read from it too), and the
//!   join goes on in the same game (`bind_here`): the game's startup sets
//!   and the offer's sets are fetched with the same progress, then what
//!   was read at startup without them is read again (`remount`: the game
//!   counted as served, the mount report, the parts' `AssetsArrived`
//!   observers: a game's data, the asset sources' failed loads); without a
//!   token the join fails saying how to enter it. A game read from the
//!   store with no service and a token bound to none (entered again on
//!   this start, which forgets the binding) is bound the same way, else
//!   nothing new would come. A refused token, an unreachable service or
//!   another certificate than the bound one fails the join with the reason
//!   on screen (`JoinFailure::Download`).
//! - **A hub account** (`chimera_assets::account`, `+chimera:hub <invite
//!   link>`; docs/plans/active/hub.md) goes before a legacy token: when the
//!   offered service has the account's hub certificate
//!   (`AssetSession::account_remote`), the join uses it with the account's
//!   current ticket, binding nothing in the access file; a service of
//!   another certificate gets the legacy token, never the ticket. A
//!   signed-in client says it can fetch (`ASSETS_KEY`, `has_token`), and
//!   keeps the ticket fresh in the background (`keep_account_fresh`).
//!
//! The startup fetch (what the games read at startup, fetched in the
//! background from the start when a service is bound and the store lacks
//! it) is `asset_startup`'s; the client app keeps it as `StartupFiles`,
//! and a join that fetches waits for it first, its bytes on the loading
//! screen.
//!
//! Quitting stops the streams (`stop_downloads`, a `chimera_core::shutdown`
//! step); every record is written whole or not at all, so the process may
//! end in the middle of any fetch.
//!
//! What a game built at startup from its files (Combat Arms' install data,
//! weapon set, attachment, movement and gear records, its HUD's install
//! UI and weapon names) is built again by its `AssetsArrived` observers
//! before the join checks the match (`remount` applies what they queue
//! first): the weapon catalogue then has the game's install weapons a
//! ladder names, at the registry indices the server has. The client's
//! mount report, now with those games, goes to the server again with its
//! userinfo (`client::machine_info`, `chat::send_userinfo`), so the next
//! match's checks and ladders (`minigame_net::playable_on`,
//! `installed_by_all`) count them.
//!
//! Known shortcuts: a bind's `remount` reads again only what its hooks
//! name (a part with no observer keeps what it read at startup without the
//! files); sets the service doesn't have are skipped (their files come on
//! demand as the loaders read them); files fetched on demand (a map's
//! load) don't hold the streams, only the join and level change around
//! them do; the next map ahead is the vote's winner only (a host's
//! `changelevel` elsewhere comes with the level change, as before).

use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_assets::{
    AssetSettings,
    access::Access,
    account,
    pace::Pace,
    progress::Transfer,
    resolve,
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
/// The server's cap on older clients' prefetch of the offer's `later`
/// sets, KB/s (cvar; negative: none, 0: no prefetch). The offer's `later`
/// is empty now, so it matters to no client.
pub const PREFETCH_MAX_CVAR: &str = "chimera:asset_prefetch_max";
/// The least a stream reads at, bytes a second (a track due far off still
/// arrives in a few minutes).
pub const STREAM_MIN_RATE: u64 = 64_000;
/// What a stream is paced to: done when this share of its time left is
/// spent.
pub const STREAM_SHARE: f64 = 0.5;
/// Enter this player's asset token (command).
pub const TOKEN_COMMAND: &str = "chimera:asset_token";
/// Sign this PC into a hub with an invite link (command line only, before
/// the app starts: `+chimera:hub <link> [username]`; docs/plans/active/hub.md).
pub const HUB_COMMAND: &str = "chimera:hub";
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
    /// Sets clients before 2026-11-15 prefetch in the background: empty
    /// (kept for them: the next map comes at the vote's lock instead).
    pub later: Vec<GameSets>,
    /// Older clients' prefetch cap, KB/s (`PREFETCH_MAX_CVAR`).
    pub prefetch_rate: Option<u32>,
}

/// A stream's rate, bytes a second: `remaining` stored bytes done within
/// `STREAM_SHARE` of `secs_left`, at least `STREAM_MIN_RATE`, at most
/// `link` (the link's estimate; a stream due sooner than the link allows
/// reads at the link's rate).
pub fn stream_rate(remaining: u64, secs_left: f64, link: u64) -> u64 {
    let want = remaining as f64 / (secs_left * STREAM_SHARE).max(0.5);
    (want.ceil() as u64).max(STREAM_MIN_RATE).min(link.max(1))
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

/// On a client: the asset service the server joined last offered (URL
/// and certificate fingerprint; None: it offered none). Feedback reports
/// go there (`client::feedback`, docs/plans/active/feedback-reports.md
/// 2.6).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct JoinedService(pub Option<(String, String)>);

/// On a client: the games the match being joined requires (its manifest's
/// required ones, the map's game). A game bound for the join that the
/// service doesn't serve fails the join only when it is one of these; a
/// wanted one (another game's guns in a ladder, which the service may not
/// publish yet) is left out with a note, as an uninstalled wanted game is.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
struct JoinRequired(Vec<String>);

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
    /// The hub account's file (None: the user's,
    /// `chimera_assets::account::AccountFile::path`).
    pub account: Option<PathBuf>,
}

impl AssetSession {
    fn access_path(&self) -> Option<PathBuf> {
        self.access.clone().or_else(Access::path)
    }

    /// This user's hub account, if signed in (its session made live, so
    /// requests carry its current ticket).
    pub fn account_session(&self) -> Option<Arc<account::Session>> {
        match &self.account {
            Some(path) => account::session_at(path),
            None => account::user_session(),
        }
    }

    /// The offered service with the account's current ticket, when it is
    /// the account's hub (the same certificate): preferred over a legacy
    /// token. None for another service (a ticket never goes there), with
    /// no account, or with no valid ticket.
    pub fn account_remote(&self, offer: &AssetOffer) -> Option<chimera_assets::RemoteConfig> {
        self.account_session()?.remote_for(&offer.url, &offer.fingerprint)
    }

    /// Whether this client can fetch from an offered service (it has a
    /// service, a token or a hub account).
    pub fn can_fetch(&self) -> bool {
        self.settings.as_ref().is_some_and(|s| s.remote.is_some())
            || self
                .access_path()
                .is_some_and(|p| Access::load_from(&p).token.is_some())
            || self.account_session().is_some()
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
        account: None,
    };
    world.insert_resource(s.clone());
    s
}

/// On a client, triggered once a join bound the token in this process and
/// the games' files arrived (`remount`): the parts that read something at
/// startup without those files observe it and read it again (the mount
/// report, a front end's own; a game's data; the asset sources' failed
/// loads).
#[derive(Event, Clone, Debug)]
pub struct AssetsArrived {
    /// The games now read from the store.
    pub games: Vec<String>,
}

/// On a client: the startup sets being fetched in the background
/// (`asset_startup::StartupFetch`: the client app starts it when a service
/// is bound and the store lacks them). A join that fetches waits for it
/// first, its bytes on the loading screen.
#[derive(Resource)]
pub struct StartupFiles(pub super::asset_startup::StartupFetch);

/// What one part names for a match's offer (an `AssetHooks` answer).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetNames {
    /// Weapons the match hands out (`game:weapon`): their `weapon/<id>`
    /// sets, and their games' `base` and `players`.
    pub weapons: Vec<String>,
    /// Other sets the match needs now (`game:set`, as published: a game's
    /// rung bodies, `combat_arms:character/<record>`).
    pub sets: Vec<String>,
}

/// Extra things a match's offer names, from the parts that know them
/// (`minigame_net`: its ladder's and kit's weapons; a
/// game: what its rung extras drew, Combat Arms' bodies): registered per
/// part, asked in order.
#[derive(Resource, Clone, Default)]
pub struct AssetHooks(pub Vec<fn(&World) -> AssetNames>);

/// On a client: sets a part wants with the match's download, beyond the
/// offer's, as `(game, set)` (Combat Arms' first music tracks: the
/// client picks them, the server doesn't know them); only the games read
/// from the store are fetched.
#[derive(Resource, Clone, Default)]
pub struct JoinSets(pub Vec<fn(&World) -> Vec<(String, String)>>);

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

impl Then {
    /// The match's map and its file's hash.
    fn map(&self) -> (&str, Option<[u8; 32]>) {
        match self {
            Then::Welcome(w) => (&w.map, w.map_hash),
            Then::Change(c) => (&c.map, c.map_hash),
        }
    }
}

/// The match's map file, which the offer names a set for: made sure of
/// before the join goes on (`ensure_map`), so the map's verify step finds
/// it here and never asks the game server for it.
#[derive(Clone)]
struct MapNeed {
    game: String,
    map: String,
    /// The map's file as its game names it (`maps::offer_file`).
    file: String,
    hash: [u8; 32],
    files: super::maps::MapFiles,
    /// Its game is installed here (not read from the store): its set is
    /// fetched only when the install lacks the file (a community map),
    /// and a failure leaves the map to the usual download.
    installed: bool,
}

/// The `MapNeed` of `then`'s map, when the offer names its `map/<name>` set
/// and this process can keep a map file (`MapFiles::cache`).
fn map_need(world: &World, offer: &AssetOffer, session: &AssetSession, then: &Then) -> Option<MapNeed> {
    let (map, hash) = then.map();
    let (game, name) = map.split_once(':')?;
    let set = format!("map/{name}");
    offer.now.iter().find(|g| g.game == game && g.sets.contains(&set))?;
    let file = super::maps::offer_file(world.get_resource::<Games>(), map)?;
    let files = world.get_resource::<super::maps::MapFiles>()?.clone();
    files.cache.as_ref()?;
    Some(MapNeed {
        game: game.to_string(),
        map: map.to_string(),
        file,
        hash: hash?,
        files,
        installed: !session.served.contains(game),
    })
}

/// Whether the map's file is here as the server has it (the install, the
/// store standing in for it, or the content cache).
fn map_found(need: &MapNeed) -> bool {
    matches!(
        super::maps::check_local(&need.files, &need.map, Some(&need.file), need.hash),
        super::maps::LocalCopy::Found(_)
    )
}

/// After the map's set arrived: when the map's file still isn't found as
/// the server has it (an install that lacks it, whose mount the store
/// isn't layered under at another build), its bytes from the store into
/// the content cache, where the map's verify step finds and loads it.
fn ensure_map(settings: &AssetSettings, need: &MapNeed) -> Result<(), String> {
    if map_found(need) {
        return Ok(());
    }
    let store = settings.store.as_ref().ok_or("no asset store")?;
    let build = store.latest_build(&need.game).ok_or_else(|| format!("no manifest of {}", need.game))?;
    let manifest = chimera_assets::Manifest::load(&store.manifest_path(&need.game, &build))?;
    let path = chimera_core::mount::normalize(&need.file);
    let record = manifest
        .files
        .get(&path)
        .ok_or_else(|| format!("{} build {build} lists no {path}", need.game))?;
    let bytes = store.get(&record.id, record.size).map_err(|e| format!("{path}: {e}"))?;
    if super::maps::sha256(&bytes) != need.hash {
        return Err(format!("the asset service's {path} isn't the server's map"));
    }
    let cache = need.files.cache.as_ref().ok_or("no content cache")?;
    let out = super::maps::cache_file(cache, &need.file);
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    chimera_assets::store::write_atomic(&out, &bytes).map_err(|e| format!("{}: {e}", out.display()))?;
    info!("{path} from the asset service kept at {}", out.display());
    Ok(())
}

/// A fetch under way.
#[derive(Resource)]
struct AssetFetch {
    then: Then,
    /// Games whose files this join bound the token for (`bind_here`):
    /// read again once they arrived (`remount`).
    bound: Vec<String>,
    job: Option<JoinHandle<Result<String, String>>>,
    transfer: Arc<Mutex<Option<Arc<Transfer>>>>,
    from: String,
}

/// On a client: the next map's sets being fetched ahead, at full speed,
/// once the match-end screen knows the map (`map_ahead`).
#[derive(Resource)]
pub struct MapAhead {
    /// The map (`game:name`).
    pub map: String,
    job: Option<JoinHandle<Result<String, String>>>,
    transfer: Arc<Mutex<Option<Arc<Transfer>>>>,
    /// How it ended (None: running).
    pub result: Option<Result<String, String>>,
}

impl MapAhead {
    /// Running now.
    pub fn running(&self) -> bool {
        self.job.as_ref().is_some_and(|j| !j.is_finished())
    }

    /// Stored bytes fetched and to fetch (0, 0 until known).
    pub fn bytes(&self) -> (u64, u64) {
        let t = self.transfer.lock().unwrap_or_else(|e| e.into_inner()).clone();
        t.map_or((0, 0), |t| {
            let (_, done, total) = t.done();
            (done, total)
        })
    }
}

/// On a client: the link's rate as a stream assumes it, bytes a second
/// (the most a stream reads at; tests set a slow link).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamLink(pub u64);

impl Default for StreamLink {
    fn default() -> Self {
        // 4 MB/s: a share of a host's upload, as the 2026-11-14 playtest
        // had it (6 MB/s for everyone).
        Self(4_000_000)
    }
}

/// One stream asked for.
#[derive(Clone, Debug, PartialEq)]
struct StreamWant {
    game: String,
    set: String,
    /// When it is due (`Time`'s elapsed seconds, real time).
    due: f64,
}

/// On a client: sets wanted in the background by a time (see the module
/// doc). Parts ask with `request` and look with `ready`.
#[derive(Resource, Default)]
pub struct Streams {
    queue: Vec<StreamWant>,
    running: Option<(StreamWant, JoinHandle<Result<String, String>>, Arc<Pace>, Arc<Mutex<Option<Arc<Transfer>>>>)>,
    ready: BTreeSet<(String, String)>,
    failed: BTreeSet<(String, String)>,
    /// Requests whose readiness wasn't looked at yet (`run_streams`).
    fresh: Vec<(String, String, f64)>,
}

/// A stream's state, for tests and the console.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StreamState {
    /// The set fetched now (`game`, `set`).
    pub running: Option<(String, String)>,
    /// Its rate, bytes a second.
    pub rate: u64,
    /// Held for a foreground download.
    pub paused: bool,
    /// Sets waiting.
    pub queued: usize,
}

impl Streams {
    /// Ask for `game`'s `set` within `secs` seconds (again: the sooner of
    /// the two times). A set already here is ready at the next frame.
    pub fn request(&mut self, game: &str, set: &str, secs: f64) {
        let key = (game.to_string(), set.to_string());
        if self.ready.contains(&key) || self.failed.contains(&key) {
            return;
        }
        self.fresh.push((key.0, key.1, secs.max(0.0)));
    }

    /// Whether `game`'s `set` is here whole (asked for and arrived, or
    /// held already).
    pub fn ready(&self, game: &str, set: &str) -> bool {
        self.ready.contains(&(game.to_string(), set.to_string()))
    }

    /// Whether it was asked for and couldn't come (not on the service).
    pub fn failed(&self, game: &str, set: &str) -> bool {
        self.failed.contains(&(game.to_string(), set.to_string()))
    }

    pub fn state(&self) -> StreamState {
        let running = self.running.as_ref().filter(|r| !r.1.is_finished());
        StreamState {
            running: running.map(|r| (r.0.game.clone(), r.0.set.clone())),
            rate: running.map_or(0, |r| r.2.rate()),
            paused: running.is_some_and(|r| r.2.paused()),
            queued: self.queue.len(),
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    use chimera_core::shutdown::{ShutdownAppExt, Stage};
    app.shutdown_flush(Stage::Persist, "downloads", stop_downloads);
    app.init_resource::<AssetHost>()
        .init_resource::<AssetHooks>()
        .init_resource::<JoinSets>()
        .init_resource::<Streams>()
        .init_resource::<StreamLink>();
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
        "Server: older clients' cap on prefetching the offer's later maps, KB/s (none are offered now).",
        |h| &mut h.prefetch_max,
    );
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
        (poll_fetch, map_ahead, run_streams)
            .chain()
            .run_if(resource_equals(NetRole::Client)),
    );
}

/// The user's access file (`chimera_assets::access`).
pub fn access_path() -> Option<PathBuf> {
    Access::path()
}

/// Whether this user can fetch from an offered asset service: an asset
/// token entered, or signed in to a hub (`chimera_assets::account`).
pub fn has_token() -> bool {
    access_path().is_some_and(|p| Access::load_from(&p).token.is_some()) || account::has_account()
}

/// Keep a signed-in account's ticket fresh in the background while the
/// game runs (`chimera_assets::account::keep_fresh`: checked every minute,
/// refreshed past half its life).
pub fn keep_account_fresh() {
    account::keep_fresh(std::time::Duration::from_secs(60));
}

/// Keep `token` in the access file at `path` (any binding forgotten).
pub fn save_token(path: &std::path::Path, token: &str) -> Result<(), String> {
    let mut access = Access::load_from(path);
    access.set_token(token)?;
    access.save_to(path)
}

/// `+chimera:hub <link> [username]`: redeem the invite in `link` and keep
/// this user's account file (`chimera_assets::account`); what to tell
/// the player.
pub fn join_hub(link: &str, username: Option<&str>) -> Result<String, String> {
    let path = chimera_assets::account::AccountFile::path().ok_or("no per-user data folder")?;
    chimera_assets::account::join_hub(link, username, &path)
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
    // The map's game too: a game whose `base` isn't a startup set
    // (`sets::startup_sets`: Halo's) gets it with the match that plays its map.
    in_play.extend(m.map.split_once(':').map(|(g, _)| g.to_string()));
    let (mut weapons, mut named) = (Vec::new(), Vec::new());
    for hook in world.get_resource::<AssetHooks>().cloned().unwrap_or_default().0 {
        let n = hook(world);
        weapons.extend(n.weapons);
        named.extend(n.sets);
    }
    in_play.extend(
        weapons
            .iter()
            .chain(&named)
            .filter_map(|w| w.split_once(':').map(|(g, _)| g.to_string())),
    );
    for g in in_play.iter().filter(|g| known(g)) {
        for set in std::iter::once(resolve::BASE).chain(resolve::MATCH_SETS.iter().copied()) {
            add(&mut offer.now, g, set.into());
        }
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
        // Weapon sets are named without the `weapon_` Source ids start
        // with (`chimera_pack sets`).
        add(&mut offer.now, game, format!("weapon/{}", name.strip_prefix("weapon_").unwrap_or(name)));
    }
    for (game, set) in named.iter().filter_map(|s| s.split_once(':')).filter(|(g, _)| known(g)) {
        add(&mut offer.now, game, set.to_string());
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
            " This server offers its game files: start the game once with +{HUB_COMMAND} <your invite link> or \
             +{TOKEN_COMMAND} <your token> (the server's admin gives you one), then join again."
        )
    })
}

// --- The client.

/// Before accepting `manifest` (with `then` to do after): fetch what its
/// offer needs of the games this process reads from the store, binding
/// the token to the offered service first when this process has none
/// for them (`bind_here`), or say why not.
pub(super) fn begin(world: &mut World, manifest: &MatchManifest, then: Then) -> Next {
    world.insert_resource(JoinedService(
        manifest.assets.as_ref().map(|o| (o.url.clone(), o.fingerprint.clone())),
    ));
    world.insert_resource(JoinRequired(
        manifest.games.iter().filter(|n| n.required).map(|n| n.game.clone()).collect(),
    ));
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
    // This process reads from the offered service already: a game it has
    // no files of is one the service doesn't serve. Only a required one
    // fails the join (`bind_here` says why); a wanted one (another game's
    // guns in a ladder) is left out with a note, as at a first join
    // (2026-11-15: every start after the first left each UGG match with
    // "the asset service doesn't serve tf2").
    let missing = if reads_offered(&session, &offer) {
        let required = world.get_resource::<JoinRequired>().cloned().unwrap_or_default().0;
        let (lacking, wanted): (Vec<String>, Vec<String>) = missing.into_iter().partition(|g| required.contains(g));
        for g in &wanted {
            info!(
                "the asset service doesn't serve {}: the match plays, but what it draws of it won't load",
                super::manifest::game_name(world.get_resource::<Games>(), g)
            );
        }
        lacking
    } else {
        missing
    };
    if !missing.is_empty() {
        return bind_here(world, &offer, &session, &missing, then);
    }
    let served = |g: &GameSets| session.served.contains(&g.game);
    // Games read from the store alone with no service to fill it, and a
    // token not bound to any: the token was entered again on this start
    // (which forgets its binding), so the store holds what an earlier
    // session fetched and nothing more would come. Bind it, as for a game
    // with no files (2026-11-11 rehearsal: a tester starting with the same
    // `+chimera:asset_token` line twice joined with no fetching at all,
    // every new file a miss).
    if !session.settings.as_ref().is_some_and(|s| s.remote.is_some()) {
        let unfilled: Vec<String> = offer
            .games()
            .filter(|g| session.served.contains(*g))
            .map(String::from)
            .collect();
        let unbound = session.access_path().is_some_and(|p| {
            let a = Access::load_from(&p);
            a.token.is_some() && a.remote().is_none()
        }) || session.account_remote(&offer).is_some();
        if !unfilled.is_empty() && unbound {
            return bind_here(world, &offer, &session, &unfilled, then);
        }
    }
    let wants: Vec<GameSets> = with_client_sets(world, offer.now.iter().filter(|g| served(g)).cloned().collect());
    let need = map_need(world, &offer, &session, &then);
    let settings = session.settings.clone().filter(|s| s.remote.is_some() && s.store.is_some());
    // An installed game's map the install may lack (a community map): its
    // set from the service, binding the token for it if it is bound to
    // none (2026-11-14: a player with CS:S installed joining a server on
    // gg_mario_vs_wario asked the game server, which has no file to send).
    let settings = match settings {
        Some(s) => s,
        None if need.as_ref().is_some_and(|n| n.installed) && can_bind(&session, &offer) => match bind(world, &offer, &session) {
            Ok(s) => s,
            Err(e) => {
                info!("the match's map from the asset service: {e}");
                return Next::Go;
            }
        },
        None => return Next::Go,
    };
    if wants.is_empty() && need.is_none() {
        return Next::Go;
    }
    start_fetch(world, settings, wanted(&wants), false, then, Vec::new(), need);
    Next::Wait
}

/// Whether this process reads from `offer`'s service already: the one
/// `bind_here` would bind (the hub account's, else the token's bound to
/// the offer) is the one its session uses.
fn reads_offered(session: &AssetSession, offer: &AssetOffer) -> bool {
    let Some(using) = session.settings.as_ref().and_then(|s| s.remote.as_ref()) else {
        return false;
    };
    let target = session.account_remote(offer).or_else(|| {
        let mut access = Access::load_from(&session.access_path()?);
        access.bind(&offer.url, &offer.fingerprint).ok()?;
        access.remote()
    });
    target.is_some_and(|t| t.url == using.url && t.fingerprint == using.fingerprint)
}

/// Whether this client has a hub account for `offer`'s service or a token
/// bound to no service (`bind` can bind it).
fn can_bind(session: &AssetSession, offer: &AssetOffer) -> bool {
    session.account_remote(offer).is_some()
        || session.access_path().is_some_and(|p| {
            let a = Access::load_from(&p);
            a.token.is_some() && a.remote().is_none()
        })
}

/// The client's own additions to `wants` (the offer's sets of the games
/// it fetches): each game's `resolve::MATCH_SETS` (an older server names
/// no `common`) and what `JoinSets` names of those games.
fn with_client_sets(world: &World, mut wants: Vec<GameSets>) -> Vec<GameSets> {
    let games: Vec<String> = wants.iter().map(|g| g.game.clone()).collect();
    for g in &games {
        for set in resolve::MATCH_SETS {
            add(&mut wants, g, set.to_string());
        }
    }
    for hook in world.get_resource::<JoinSets>().cloned().unwrap_or_default().0 {
        for (game, set) in hook(world) {
            if games.contains(&game) {
                add(&mut wants, &game, set);
            }
        }
    }
    wants
}

/// Fetch `wants` on a thread (after the startup sets fetching in the
/// background, `StartupFiles`), the bytes shown as the join's
/// `Downloading` stage; the join goes on once they are here
/// (`poll_fetch`), `bound`'s games read again first (`remount`).
#[allow(clippy::too_many_arguments)]
fn start_fetch(
    world: &mut World,
    settings: AssetSettings,
    wants: Vec<Wanted>,
    skip_unlisted: bool,
    then: Then,
    bound: Vec<String>,
    need: Option<MapNeed>,
) {
    let from = settings.remote.as_ref().map(|r| r.url.clone()).unwrap_or_default();
    info!(
        "fetching the match's {FILES} from {from}: {}",
        wants
            .iter()
            .map(|g| format!("{} ({})", g.game, g.sets.join(", ")))
            .collect::<Vec<_>>()
            .join("; ")
    );
    // The startup sets still coming in the background: waited for first,
    // their bytes on the loading screen.
    let (startup, transfer) = match world.remove_resource::<StartupFiles>() {
        Some(StartupFiles(fetch)) => fetch.into_parts(),
        None => (None, Arc::default()),
    };
    // The next map fetched ahead: waited for, so its records aren't asked
    // for twice.
    let ahead = world.get_resource_mut::<MapAhead>().and_then(|mut a| a.job.take());
    let shared = transfer.clone();
    // From now until the fill claims its records, on-demand misses wait
    // for it (`pending::starting`): the new match's ladder, arriving with
    // the level change, starts loading its models at once.
    let store = settings.store.clone().expect("a store to fetch into");
    let starting = startup.is_none().then(|| chimera_assets::pending::starting(&store));
    let job = std::thread::spawn(move || {
        if let Some(job) = startup {
            match job.join() {
                Ok(Ok(summary)) => info!("startup {FILES}: {summary}"),
                Ok(Err(e)) => info!("startup {FILES}: {e}"),
                Err(_) => info!("startup {FILES}: the fetch panicked"),
            }
        }
        if let Some(job) = ahead {
            match job.join() {
                Ok(Ok(summary)) => info!("the next map's {FILES}: {summary}"),
                Ok(Err(e)) => info!("the next map's {FILES}: {e}"),
                Err(_) => info!("the next map's {FILES}: the fetch panicked"),
            }
        }
        let starting = starting.unwrap_or_else(|| chimera_assets::pending::starting(&store));
        let options = sets::Options {
            starting: Some(&starting),
            skip_unlisted,
            ..default()
        };
        let summary = if wants.is_empty() {
            drop(starting);
            String::new()
        } else {
            sets::fetch(&settings, &wants, FILES, &shared, options)?
        };
        // The map's file: an installed game's set only when the install
        // lacks it (its failure leaves the map to the usual download).
        if let Some(need) = need {
            if need.installed && !map_found(&need) {
                let name = super::maps::map_name(&need.map);
                let one = [Wanted { game: need.game.clone(), sets: vec![format!("map/{name}")] }];
                if let Err(e) = sets::fetch(&settings, &one, FILES, &shared, sets::Options::default()) {
                    info!("{}'s set from the asset service: {e}", need.map);
                }
            }
            if let Err(e) = ensure_map(&settings, &need) {
                info!("{} from the asset service: {e}", need.map);
            }
        }
        Ok(summary)
    });
    client::set_stage(world, JoinStage::Downloading);
    world.resource_mut::<JoinProgress>().download = Some(DownloadInfo {
        file: FILES.into(),
        from: "asset service".into(),
        done: 0,
        total: 0,
        files_done: 0,
        files_total: 0,
    });
    world.insert_resource(AssetFetch {
        then,
        bound,
        job: Some(job),
        transfer,
        from,
    });
}

/// The offer names `games`, which this process has no files of, or reads
/// from the store with no service: bind the token to the offered service
/// (the access file and this process: `access::bind_live`) and fetch
/// their startup sets and the offer's in this game, the join going on
/// once they are here; else say why not.
fn bind_here(world: &mut World, offer: &AssetOffer, session: &AssetSession, games: &[String], then: Then) -> Next {
    let names = games
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
    let account = session.account_remote(offer);
    if account.is_none() && Access::load_from(&path).token.is_none() {
        let why = format!(
            "{}{names}, which isn't installed here. The server offers its game files: start the game once with \
             +{HUB_COMMAND} <your invite link> or +{TOKEN_COMMAND} <your token> (the server's admin gives you \
             one), then join again.",
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
    // The service it would bind: the account's hub with a ticket, else the
    // legacy token's.
    let target = match account {
        Some(remote) => Some(remote),
        None => {
            let mut access = Access::load_from(&path);
            if let Err(e) = access.bind(&offer.url, &offer.fingerprint) {
                return fail(world, e);
            }
            access.remote()
        }
    };
    let same = |u: &chimera_assets::RemoteConfig| {
        target.as_ref().is_some_and(|t| t.url == u.url && t.fingerprint == u.fingerprint)
    };
    if let Some(remote) = using.filter(|u| same(u)) {
        // This process already reads from that service, which didn't serve
        // the game: say why (a refused token, an unreachable service),
        // else that it lacks the game.
        let why = chimera_assets::Remote::shared(&remote)
            .and_then(|r| r.whoami().map_err(|e| e.to_string()))
            .err()
            .map(|e| sets::explain(&offer.url, &e))
            .unwrap_or_else(|| {
                format!("the asset service at {} doesn't serve {names}; tell the server's admin", offer.url)
            });
        return fail(world, why);
    }
    let settings = match bind(world, offer, session) {
        Ok(s) => s,
        Err(e) => return fail(world, e),
    };
    info!("{names} come from the asset service now");
    // The games' startup sets (what a start with the service would have
    // fetched first) and the offer's, of these games and those served.
    let bound: BTreeSet<&str> = games.iter().map(String::as_str).collect();
    let fetched = |g: &str| bound.contains(g) || session.served.contains(g);
    let mut wants: Vec<Wanted> = games
        .iter()
        .map(|g| Wanted { game: g.clone(), sets: sets::startup_sets(g).iter().map(|s| s.to_string()).collect() })
        .collect();
    let now = with_client_sets(world, offer.now.iter().filter(|g| fetched(&g.game)).cloned().collect());
    for g in &now {
        match wants.iter_mut().find(|w| w.game == g.game) {
            Some(w) => w.sets.extend(g.sets.iter().filter(|s| !w.sets.contains(s)).cloned().collect::<Vec<_>>()),
            None => wants.push(Wanted { game: g.game.clone(), sets: g.sets.clone() }),
        }
    }
    let mut need = map_need(world, offer, session, &then);
    if let Some(n) = need.as_mut() {
        n.installed = !fetched(&n.game);
    }
    // Games the service doesn't list are skipped here and found missing
    // after (`remount`), with the reason.
    start_fetch(world, settings, wants, true, then, games.to_vec(), need);
    Next::Wait
}

/// Use the offered service from now on: with the hub account's ticket
/// when it is the account's hub (`AssetSession::account_remote`), else
/// with the legacy token, bound to it in the access file. Either way in
/// this process too (`access::bind_live`), the session reading from it
/// from now on. The session's settings with the service; Err: why not.
fn bind(world: &mut World, offer: &AssetOffer, session: &AssetSession) -> Result<AssetSettings, String> {
    let remote = match session.account_remote(offer) {
        Some(remote) => {
            info!("the hub account's ticket is used for {}", offer.url);
            remote
        }
        None => {
            let path = session.access_path().ok_or("no per-user data folder")?;
            let mut access = Access::load_from(&path);
            access.bind(&offer.url, &offer.fingerprint)?;
            access.save_to(&path)?;
            access.remote().ok_or("the token isn't bound")?
        }
    };
    let mut settings = session.settings.clone().unwrap_or_else(|| {
        chimera_core::mount::config::LocalConfig::load()
            .map(|c| c.assets.resolve())
            .unwrap_or_else(|_| chimera_assets::AssetsConfig::default().resolve())
    });
    if settings.store.is_none() {
        return Err("no asset store folder".into());
    }
    settings.remote = Some(remote.clone());
    chimera_assets::access::bind_live(remote);
    let mut now = session.clone();
    now.settings = Some(settings.clone());
    world.insert_resource(now);
    info!("the asset service at {} is used now", offer.url);
    Ok(settings)
}

/// After a bind's files arrived: `bound`'s games the store now has are
/// read from it (served, in the mount report), then the parts read again
/// what they read at startup (`AssetsArrived`; what its observers queue
/// is applied before this returns, so the join's checks see it). The
/// games that arrived.
fn remount(world: &mut World, bound: &[String]) -> Vec<String> {
    let store = world
        .get_resource::<AssetSession>()
        .and_then(|s| s.settings.as_ref())
        .and_then(|s| s.store.clone());
    let arrived: Vec<String> = bound
        .iter()
        .filter(|g| store.as_ref().is_some_and(|s| s.latest_build(g).is_some()))
        .cloned()
        .collect();
    if let Some(mut s) = world.get_resource_mut::<AssetSession>() {
        s.served.extend(arrived.iter().cloned());
    }
    if let Some(mut local) = world.get_resource_mut::<LocalMounts>() {
        for g in &arrived {
            match local.0.games.iter_mut().find(|m| m.game == *g) {
                Some(m) => m.installed = true,
                None => local.0.games.push(GameMount {
                    game: g.clone(),
                    installed: true,
                    build: None,
                    notes: Vec::new(),
                }),
            }
        }
    }
    world.trigger(AssetsArrived { games: arrived.clone() });
    // What the observers queued (a game's data and weapon set made again)
    // is done before the join goes on and checks the match against it.
    world.flush();
    info!(
        "game files read again for {}",
        if arrived.is_empty() { "no game".to_string() } else { arrived.join(", ") }
    );
    arrived
}

/// `GameSets` as `sets::fetch` takes them.
fn wanted(list: &[GameSets]) -> Vec<Wanted> {
    list.iter().map(|g| Wanted { game: g.game.clone(), sets: g.sets.clone() }).collect()
}

/// The fetch's progress into the loading screen; once done, the join goes
/// on (a bind's games read again first: `remount`), or fails with the
/// reason.
fn poll_fetch(world: &mut World) {
    let Some(f) = world.get_resource::<AssetFetch>() else {
        return;
    };
    let transfer = f.transfer.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let finished = f.job.as_ref().is_none_or(|j| j.is_finished());
    if let Some(t) = transfer {
        let (files, bytes, total) = t.done();
        if let Some(d) = world
            .resource_mut::<JoinProgress>()
            .download
            .as_mut()
            .filter(|d| d.file == FILES)
        {
            d.done = bytes;
            d.total = total;
            d.files_done = files;
            d.files_total = t.files();
        }
    }
    if !finished {
        return;
    }
    let mut f = world.remove_resource::<AssetFetch>().expect("checked");
    let result = f.job.take().map_or(Err("no fetch".to_string()), |j| {
        j.join().unwrap_or(Err("the fetch panicked".into()))
    });
    let fail = |world: &mut World, error: String, from: &str| {
        let reason = format!("Couldn't get the match's {FILES} from {from}: {error}");
        client::fail(
            world,
            JoinFailure::Download {
                file: FILES.into(),
                error,
            },
            &reason,
        );
    };
    match result {
        Ok(summary) => {
            info!("{summary}");
            if !f.bound.is_empty() {
                let arrived = remount(world, &f.bound);
                let required = world.get_resource::<JoinRequired>().cloned().unwrap_or_default().0;
                let (lacking, wanted): (Vec<&String>, Vec<&String>) =
                    f.bound.iter().filter(|g| !arrived.contains(g)).partition(|g| required.contains(g));
                let name = |g: &String| super::manifest::game_name(world.get_resource::<Games>(), g);
                for g in wanted {
                    info!(
                        "the asset service doesn't serve {}: the match plays, but what it draws of it won't load",
                        name(g)
                    );
                }
                let lacking: Vec<String> = lacking.into_iter().map(name).collect();
                if !lacking.is_empty() {
                    let why = format!(
                        "the asset service doesn't serve {}; tell the server's admin",
                        lacking.join(" and ")
                    );
                    return fail(world, why, &f.from);
                }
            }
            world.resource_mut::<JoinProgress>().download = None;
            client::proceed(world, f.then);
        }
        Err(error) => fail(world, error, &f.from),
    }
}

/// The match-end screen knows the next map (`minigame::NextMatch`: the
/// vote's winner, or the rotation's pick): its game's match sets and its
/// `map/<name>` fetched now at full speed, when the game is read from the
/// store (`MapAhead`; once per map).
fn map_ahead(world: &mut World) {
    let Some(map) = world.get_resource::<chimera_sim::minigame::NextMatch>().and_then(|n| n.map.clone()) else {
        return;
    };
    if world.get_resource::<MapAhead>().is_some_and(|a| a.map == map) || !world.contains_resource::<client::Joined>() {
        return;
    }
    let Some((game, name)) = map.split_once(':') else { return };
    let Some(session) = world.get_resource::<AssetSession>().cloned() else { return };
    let Some(settings) = session.settings.clone().filter(|s| s.remote.is_some() && s.store.is_some()) else {
        return;
    };
    if !session.served.contains(game) {
        return;
    }
    let base_held = sets::held(&settings, game, &[resolve::BASE]);
    let mut names = resolve::match_sets(&[format!("map/{name}")]);
    names.retain(|s| s != resolve::BASE || !base_held);
    let wants = vec![Wanted { game: game.to_string(), sets: names }];
    let transfer: Arc<Mutex<Option<Arc<Transfer>>>> = Arc::default();
    let shared = transfer.clone();
    info!("fetching the next map's {FILES} ahead: {map}");
    let job = std::thread::spawn(move || {
        sets::fetch(&settings, &wants, "the next map's files", &shared, sets::Options::default())
    });
    world.insert_resource(MapAhead { map, job: Some(job), transfer, result: None });
}

/// The streams: new requests looked at (a set held already is ready at
/// once), the one running finished or paced (`stream_rate` against its
/// time left; held while a foreground download runs), the next started.
fn run_streams(world: &mut World) {
    let now = world.get_resource::<Time<Real>>().map_or(0.0, |t| t.elapsed_secs_f64());
    let busy = world.contains_resource::<AssetFetch>()
        || world.get_resource::<MapAhead>().is_some_and(MapAhead::running)
        || !world.contains_resource::<client::Joined>()
        || world.get_resource::<JoinProgress>().is_some_and(|p| !matches!(p.stage, JoinStage::Idle | JoinStage::Joined));
    let link = world.get_resource::<StreamLink>().copied().unwrap_or_default().0;
    let settings = world.get_resource::<AssetSession>().and_then(|s| s.settings.clone()).filter(|s| s.store.is_some());
    // Ahead's end noted.
    if let Some(mut a) = world.get_resource_mut::<MapAhead>()
        && a.job.as_ref().is_some_and(|j| j.is_finished())
    {
        let result = a.job.take().map(|j| j.join().unwrap_or(Err("the fetch panicked".into())));
        match &result {
            Some(Ok(summary)) => info!("the next map's {FILES} ({}): {summary}", a.map),
            Some(Err(e)) => info!("the next map's {FILES} ({}): {e}", a.map),
            None => {}
        }
        a.result = result;
    }
    let Some(mut streams) = world.get_resource_mut::<Streams>() else { return };
    for (game, set, secs) in std::mem::take(&mut streams.fresh) {
        let key = (game.clone(), set.clone());
        if streams.ready.contains(&key) {
            continue;
        }
        if settings.as_ref().is_some_and(|s| sets::held(s, &game, &[set.as_str()])) {
            streams.ready.insert(key);
            continue;
        }
        let due = now + secs;
        let running = streams.running.as_ref().is_some_and(|r| r.0.game == game && r.0.set == set);
        match streams.queue.iter_mut().find(|w| w.game == game && w.set == set) {
            Some(w) => w.due = w.due.min(due),
            None if running => {}
            None => streams.queue.push(StreamWant { game, set, due }),
        }
    }
    // The one running: finished, or paced to its time left.
    if let Some((want, job, pace, transfer)) = streams.running.as_ref() {
        if job.is_finished() {
            let (want, job, _, _) = streams.running.take().expect("checked");
            let key = (want.game.clone(), want.set.clone());
            match job.join().unwrap_or(Err("the fetch panicked".into())) {
                Ok(summary) => {
                    info!("streamed {}:{}: {summary}", want.game, want.set);
                    if settings.as_ref().is_some_and(|s| sets::held(s, &want.game, &[want.set.as_str()])) {
                        streams.ready.insert(key);
                    } else {
                        streams.failed.insert(key);
                    }
                }
                Err(e) => {
                    info!("streaming {}:{}: {e}", want.game, want.set);
                    streams.failed.insert(key);
                }
            }
        } else {
            let (done, total) = transfer
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .map_or((0, 0), |t| {
                    let (_, d, t) = t.done();
                    (d, t)
                });
            pace.set_rate(stream_rate(total.saturating_sub(done), want.due - now, link));
            if pace.paused() != busy {
                pace.pause(busy);
            }
        }
    }
    if streams.running.is_some() || streams.queue.is_empty() || busy {
        return;
    }
    let Some(settings) = settings.filter(|s| s.remote.is_some()) else { return };
    // The soonest due first.
    streams.queue.sort_by(|a, b| a.due.total_cmp(&b.due));
    let want = streams.queue.remove(0);
    let pace = Arc::new(Pace::new(stream_rate(0, want.due - now, link)));
    let transfer: Arc<Mutex<Option<Arc<Transfer>>>> = Arc::default();
    let (p, t) = (pace.clone(), transfer.clone());
    let one = vec![Wanted { game: want.game.clone(), sets: vec![want.set.clone()] }];
    let job = std::thread::spawn(move || {
        let options = sets::Options { pace: Some(&p), ..default() };
        sets::fetch(&settings, &one, "streamed files", &t, options)
    });
    streams.running = Some((want, job, pace, transfer));
}

/// On a client: the streams' state (None: no streams resource).
pub fn stream_state(world: &World) -> Option<StreamState> {
    world.get_resource::<Streams>().map(Streams::state)
}

/// The game quits: a stream stops at its next chunk. Nothing is cut
/// mid-file: the store writes each record whole beside its place and
/// renames it in (`store::write_atomic`), so a fetch ended anywhere leaves
/// whole records, and the next fetch asks for the rest.
fn stop_downloads(world: &mut World) {
    if let Some((_, _, pace, _)) = world.get_resource::<Streams>().and_then(|s| s.running.as_ref()) {
        pace.stop();
    }
}

/// The client left: no fetch goes on for it; a stream stops (its batch
/// cut short); the next map ahead is forgotten.
pub(super) fn reset(world: &mut World) {
    world.remove_resource::<AssetFetch>();
    world.remove_resource::<MapAhead>();
    if let Some(mut s) = world.get_resource_mut::<Streams>() {
        if let Some((_, _, pace, _)) = s.running.take() {
            pace.stop();
        }
        s.queue.clear();
        s.fresh.clear();
    }
}

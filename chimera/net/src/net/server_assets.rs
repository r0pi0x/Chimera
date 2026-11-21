//! A game server's own game files from the asset service
//! (docs/playtest-hosting.md, "The server's own game files"): a server
//! without a game's install (or without its keys) reads that game from
//! the asset store, as a player does, and fetches what it reads with a
//! credential of its own, before the match, never during it. Then the
//! data a server builds from the files (TF2's guns' numbers from their
//! decrypted scripts, which become the `tf2:weapons` server documents
//! every client plays with) are the real ones, not the specs' fallback.
//!
//! - **The credential** (`credential`), in order: the asset service the
//!   process resolved (`[assets] url` and `token`, `CHIMERA_ASSET_URL`
//!   and `CHIMERA_ASSET_TOKEN`, the user's hub account file, a token bound
//!   to a service: `chimera_assets::AssetsConfig::resolve`); the hub
//!   account of the session (`AssetSession::account_session`); else a
//!   token with no service named (`CHIMERA_ASSET_TOKEN` or `[assets] token`
//!   alone, or the access file's unbound token) sent to the service this
//!   server offers its players (`chimera:asset_url`, pinned by
//!   `chimera:asset_fingerprint` or this machine's asset service's
//!   certificate: `AssetHost`); a service the server offers that the
//!   resolved one names without a fingerprint is pinned the same way. A
//!   **listen server** is the host's process: its session is the host's,
//!   so it fetches with the host's ticket or token (`for_host`, at
//!   `net::server::listen`). A **dedicated server** has its own: a hub
//!   account of role `host` redeemed once with `+chimera:hub <invite>`
//!   (its device key and tickets in the server user's `account.toml`), or,
//!   for an asset service with no hub, that service's token in its
//!   environment or `[assets]`.
//! - **At start** (`startup`, a dedicated server's `run`, before the app
//!   is built: what its parts read at build time, TF2's scripts among
//!   them): each game it reads from the store alone (no install, or
//!   `use_install = false`, or, with the service first, the apps'
//!   default (`chimera_assets::config::InstallDefault`), every game once
//!   it has a credential) gets its startup sets (`base`), and the start
//!   map's game its match sets (`base`, `players`, `common`) and
//!   `map/<name>`, as one blocking download with a progress line every
//!   `PROGRESS_EVERY`. A token sent to the offered service is bound in the
//!   process (`access::bind_live`), so the mounts opened after it fetch
//!   from that service too.
//! - **Before each match** (`watch`, on the server while `ServerFiles` is
//!   there): the match's sets as the offer names them
//!   (`assets::match_sets`: every game in play's match sets, the map's,
//!   the ladder's and kit's `weapon/<id>`, a game's rung bodies), of the
//!   games read from the store, named again only when what they are made
//!   of changed (`MatchInputs`: the map loaded and served, a map change
//!   for the next match, the minigame and its definitions, the ladder,
//!   the slots, the games, the weapons, the starting weapons, the
//!   server's files, and what a part registers with `watch_input`, such
//!   as Halo's campaign bodies): each frame compares those resources'
//!   change ticks with the ones of the last look, a few lookups; the
//!   look builds the match's manifest and every part's asset names
//!   (63 M instructions in the sandbox, every game's guns, in a dev
//!   build). It ran every second (`CHECK_EVERY`, now gone) and on the
//!   listen host of a sandbox froze a frame for about 100 ms each time
//!   (2026-11-19's report: the frame inspector's spikes, "Update watch";
//!   docs/performance.md, "The sandbox's jitter"). When the sets changed
//!   since the last look, a thread looks at
//!   the store (every served game's manifest read, each record of the
//!   sets; never on the main thread: done there every second it froze a
//!   listen host's frames for 0.4-1 s, docs/performance.md, "Stutter on
//!   Beaver Creek") and fetches what it lacks
//!   (`ServerFetching` while it runs; a change while one runs starts
//!   another, done only after the first: `Job`). A dedicated server's
//!   `changelevel` loads its map only once the fetch is done
//!   (`fetch_now`), the first match's sets come before it listens, and
//!   the pre-match lobby doesn't count down meanwhile
//!   (`net::maps::local_load_under_way`, `prematch`); a match without a
//!   lobby (the sandbox: `MinigameDef::lobby`) starts without waiting for
//!   it (`minigame_net`). A fetch of the
//!   same sets isn't tried again until they change (a set the service
//!   lacks stays missing).
//!
//! - **A sandbox's picks** (`net::units`): a gun a player picks that the
//!   server's store lacks is fetched with the same credential
//!   (`ServerFiles`) in the background before the rung switch applies it
//!   (`chimera_sim::minigame::gun_files::gun_ready`); held while a match's
//!   fetch here runs.
//!
//! Known shortcuts: a listen host's own map load doesn't wait for the
//! fetch (its loader's misses wait for the records the fetch claimed,
//! `pending`, or come on demand); the sets a ladder draws when its match
//! starts (an automatic ladder's) are fetched then, during the lobby;
//! every set the offer names is fetched, though the server reads few of
//! a gun's files (its script, not its sounds).

use std::{
    collections::BTreeSet,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use bevy::{ecs::change_detection::Tick, prelude::*};
use chimera_assets::{
    AssetSettings, RemoteConfig, access,
    progress::{Transfer, mb},
    remote,
    sets::{self, Wanted},
};
use chimera_core::game_registry::GameInfo;

use super::{
    asset_startup::AppGame,
    assets::{self, AssetHost, AssetSession},
    manifest,
};

/// What the server's log calls its downloads.
pub const WHAT: &str = "the server's game files";
/// How often a blocking download says how far it is.
pub const PROGRESS_EVERY: Duration = Duration::from_secs(5);

/// Where the server's credential came from (its log says so once).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// The service the process resolved (`[assets]`, the environment,
    /// the user's hub account or bound token).
    Config,
    /// The session's hub account (its current ticket).
    Account,
    /// A token with no service named, sent to the service the server
    /// offers.
    Offered,
}

impl Via {
    fn words(self) -> &'static str {
        match self {
            Via::Config => "the configured asset service",
            Via::Account => "the hub account's asset service",
            Via::Offered => "the asset service it offers, with its token",
        }
    }
}

/// What a server fetches its own game files with: the store and the
/// service (`settings.remote` is set), and the games it reads from the
/// store alone.
#[derive(Resource, Clone, Debug)]
pub struct ServerFiles {
    pub settings: AssetSettings,
    pub served: BTreeSet<String>,
    pub via: Via,
}

/// A fetch of the server's under way, or done (`result`).
pub struct Job {
    pub wanted: Vec<Wanted>,
    /// A fetch begun before this one (for an older look at the match):
    /// this one is done only once that one is too.
    after: Option<Arc<Job>>,
    result: Mutex<Option<Result<String, String>>>,
    done: Condvar,
    transfer: Arc<Mutex<Option<Arc<Transfer>>>>,
}

impl Job {
    /// Its result once done (None: still fetching).
    pub fn result(&self) -> Option<Result<String, String>> {
        if self.after.as_ref().is_some_and(|a| a.result().is_none()) {
            return None;
        }
        self.result.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Waits until it is done.
    pub fn wait(&self) -> Result<String, String> {
        if let Some(a) = &self.after {
            let _ = a.wait();
        }
        let mut r = self.result.lock().unwrap_or_else(|e| e.into_inner());
        while r.is_none() {
            r = self.done.wait(r).unwrap_or_else(|e| e.into_inner());
        }
        r.clone().unwrap_or(Err("no result".into()))
    }

    /// `wait`, saying how far it is every `every` on stderr.
    pub fn wait_saying(&self, every: Duration) -> Result<String, String> {
        if let Some(a) = &self.after {
            let _ = a.wait_saying(every);
        }
        loop {
            let r = self.result.lock().unwrap_or_else(|e| e.into_inner());
            let (r, _) = self.done.wait_timeout(r, every).unwrap_or_else(|e| e.into_inner());
            if let Some(r) = r.clone() {
                return r;
            }
            drop(r);
            if let Some(t) = self.transfer.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                let (_, done, total) = t.done();
                eprintln!("assets: {WHAT}: {} of {} so far", mb(done), mb(total));
            }
        }
    }
}

/// The server's fetch under way (the match waits for it).
#[derive(Resource, Clone)]
pub struct ServerFetching(pub Arc<Job>);

/// What the last fetch was of (not tried again until the sets change).
#[derive(Resource, Clone, Default)]
struct Fetched {
    wanted: Vec<Wanted>,
    /// The match's inputs' change ticks at the last look (`MatchInputs`).
    inputs: Vec<Option<Tick>>,
    map: String,
    /// Looks at the store started (`store_looks`).
    looks: usize,
    /// Looks at the match's sets (`match_looks`).
    computed: usize,
}

/// What the match's sets are made of (`watch`): readers of resources'
/// change ticks (None: the resource isn't there). A look at the match's
/// sets comes only when one of them reads otherwise than at the last.
#[derive(Resource, Clone, Default)]
pub struct MatchInputs(pub Vec<fn(&World) -> Option<Tick>>);

fn ticks_of<R: Resource>(world: &World) -> Option<Tick> {
    world.get_resource_change_ticks::<R>().map(|t| t.changed)
}

/// Resource `R` counts among what the match's sets are made of: a change
/// to it (or its coming or going) makes the server look at the sets again
/// (a part's setting its asset names read: Halo's campaign bodies).
pub fn watch_input<R: Resource>(app: &mut App) {
    app.world_mut().get_resource_or_init::<MatchInputs>().0.push(ticks_of::<R>);
}

/// The inputs' change ticks now.
fn inputs_now(world: &World) -> Vec<Option<Tick>> {
    world.get_resource::<MatchInputs>().map_or_else(Vec::new, |i| i.0.iter().map(|f| f(world)).collect())
}

/// Starts fetching `wanted` with `files` on a thread.
pub fn start(files: &ServerFiles, wanted: Vec<Wanted>) -> Arc<Job> {
    start_after(files, wanted, None)
}

/// `start`, done only once `after` is done too.
fn start_after(files: &ServerFiles, wanted: Vec<Wanted>, after: Option<Arc<Job>>) -> Arc<Job> {
    spawn_job(files, wanted, after, None)
}

/// `start_after`, looking at the store first on the job's thread: done at
/// once (an empty summary) when it holds every record of `wanted`, else
/// logged and fetched (for the match on `map`).
fn start_checked(files: &ServerFiles, wanted: Vec<Wanted>, after: Option<Arc<Job>>, map: String) -> Arc<Job> {
    spawn_job(files, wanted, after, Some(map))
}

fn spawn_job(files: &ServerFiles, wanted: Vec<Wanted>, after: Option<Arc<Job>>, check_for: Option<String>) -> Arc<Job> {
    let job = Arc::new(Job {
        after,
        wanted: wanted.clone(),
        result: Mutex::new(None),
        done: Condvar::new(),
        transfer: Arc::default(),
    });
    let (settings, j) = (files.settings.clone(), job.clone());
    let run = move || {
        let needed = check_for.as_ref().is_none_or(|map| {
            let missing = sets::missing(&settings, &j.wanted);
            if missing {
                info!("assets: fetching {WHAT} for the match on {map}: {}", describe(&j.wanted));
            }
            missing
        });
        let r = if needed {
            let options = sets::Options { skip_unlisted: true, ..Default::default() };
            sets::fetch(&settings, &j.wanted, WHAT, &j.transfer, options)
        } else {
            Ok(String::new())
        };
        *j.result.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
        j.done.notify_all();
    };
    if let Err(e) = std::thread::Builder::new().name("server files".into()).spawn(run) {
        *job.result.lock().unwrap_or_else(|e| e.into_inner()) = Some(Err(format!("couldn't start the download: {e}")));
    }
    job
}

/// The server's credential (module doc): `resolved` (the process's
/// service), else `account` (the session's hub account's), else `token`
/// with `offered` (the service this server names, its URL and
/// fingerprint); a resolved service at the offered URL without a
/// fingerprint is pinned to the offered one.
pub fn credential(
    resolved: Option<&RemoteConfig>,
    account: Option<RemoteConfig>,
    token: Option<&str>,
    offered: Option<(&str, &str)>,
) -> Option<(RemoteConfig, Via)> {
    let same = |a: &str, b: &str| a.trim().trim_end_matches('/').eq_ignore_ascii_case(b.trim().trim_end_matches('/'));
    if let Some(r) = resolved {
        let mut r = r.clone();
        if r.fingerprint.is_none()
            && let Some((_, fp)) = offered.filter(|(url, _)| same(url, &r.url))
        {
            r.fingerprint = Some(fp.to_string());
        }
        return Some((r, Via::Config));
    }
    if let Some(r) = account {
        return Some((r, Via::Account));
    }
    let token = token.map(str::trim).filter(|t| !t.is_empty())?;
    let (url, fingerprint) = offered.filter(|(u, f)| !u.trim().is_empty() && !f.trim().is_empty())?;
    Some((
        RemoteConfig {
            url: url.trim().trim_end_matches('/').to_string(),
            token: token.to_string(),
            key: None,
            fingerprint: Some(fingerprint.to_string()),
        },
        Via::Offered,
    ))
}

/// A token configured with no service (`CHIMERA_ASSET_TOKEN`, else
/// `[assets] token`).
fn configured_token(config_token: Option<&str>) -> Option<String> {
    std::env::var(remote::TOKEN_ENV)
        .ok()
        .or_else(|| config_token.map(str::to_string))
        .filter(|t| !t.trim().is_empty())
}

/// The service `host` names (its URL and certificate fingerprint), if
/// any.
fn offered(host: &AssetHost) -> Option<(String, String)> {
    let url = host.url.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        return None;
    }
    Some((url, host.fingerprint()?))
}

/// The server's files from `session` (a listen host's: the host's own
/// settings, account and access file) and the service `host` offers, with
/// `token` a token configured with no service; None without a
/// credential or with no game read from the store alone.
pub fn from_session(session: &AssetSession, host: &AssetHost, token: Option<&str>) -> Option<ServerFiles> {
    if session.served.is_empty() {
        return None;
    }
    let mut settings = session.settings.clone()?;
    let account = session.account_session().and_then(|s| s.remote());
    let access_token = session.access_path().and_then(|p| access::Access::load_from(&p).token);
    let token = token.map(str::to_string).or(access_token);
    let offered = offered(host);
    let (remote, via) = credential(
        settings.remote.as_ref(),
        account,
        token.as_deref(),
        offered.as_ref().map(|(u, f)| (u.as_str(), f.as_str())),
    )
    .or_else(|| access::live_remote().map(|r| (r, Via::Config)))?;
    settings.remote = Some(remote);
    Some(ServerFiles { settings, served: session.served.clone(), via })
}

/// A listen server's files (`net::server::listen`): the host's session
/// (`assets::session`) and the service it offers, unless the app has
/// them already (a dedicated server's, from `startup`).
pub fn for_host(world: &mut World) {
    if world.contains_resource::<ServerFiles>() {
        return;
    }
    let session = assets::session(world);
    let host = world.get_resource::<AssetHost>().cloned().unwrap_or_default();
    let config_token = chimera_core::mount::config::LocalConfig::load().ok().and_then(|c| c.assets.token);
    if let Some(files) = from_session(&session, &host, configured_token(config_token.as_deref()).as_deref()) {
        info!(
            "assets: this server fetches {} ({}) from {}",
            files.served.iter().cloned().collect::<Vec<_>>().join(", "),
            files.via.words(),
            files.settings.remote.as_ref().map_or("", |r| r.url.as_str())
        );
        world.insert_resource(files);
    }
}

/// The sets `games` read at startup and, for the start map `map`
/// (`<game>:<name>`), its game's match sets and `map/<name>`: each game
/// read from the store alone (`served`) only.
pub fn startup_wanted(served: &BTreeSet<String>, map: Option<&str>) -> Vec<Wanted> {
    let mut out: Vec<Wanted> = served
        .iter()
        .map(|g| Wanted { game: g.clone(), sets: sets::startup_sets(g).iter().map(|s| s.to_string()).collect() })
        .collect();
    if let Some((game, name)) = map.and_then(|m| m.split_once(':'))
        && served.contains(game)
    {
        let i = out.iter().position(|w| w.game == game).unwrap_or_else(|| {
            out.push(Wanted { game: game.into(), sets: Vec::new() });
            out.len() - 1
        });
        let sets = std::iter::once(chimera_assets::resolve::BASE)
            .chain(chimera_assets::resolve::MATCH_SETS.iter().copied())
            .map(str::to_string)
            .chain(std::iter::once(format!("map/{name}")));
        for s in sets {
            if !out[i].sets.contains(&s) {
                out[i].sets.push(s);
            }
        }
    }
    out.retain(|w| !w.sets.is_empty());
    out
}

/// A dedicated server's files before its app is built (module doc): this
/// machine's config and installs for `games`, the credential, the service
/// offered by the command line's `offered_url` and `offered_fingerprint`
/// (the `chimera:asset_*` cvars' values, empty if not given); the
/// startup's and the start map's sets fetched now, saying how it goes on
/// stderr. None: no game is read from the store alone, or no credential
/// (said once).
pub fn startup(games: &[&'static GameInfo], offered_url: &str, offered_fingerprint: &str, map: &str) -> Option<ServerFiles> {
    use chimera_core::mount::{config::LocalConfig, install};
    let config = LocalConfig::load().ok()?;
    let settings = config.assets.resolve();
    let apps: Vec<AppGame> = games
        .iter()
        .map(|g| AppGame { game: g.id.to_string(), installed: install::resolve(&config, g.id, g.steam.as_ref()).install.is_some() })
        .collect();
    // The games read from the store: those not installed, and with the
    // service first (the apps' default, nothing chosen) the installed ones
    // too once there is a credential (the token sent to the offered
    // service is bound in the process, `access::bind_live`, which
    // `AssetsConfig::resolve` then names, so the mounts read the store).
    let first = settings.service_first
        || (config.assets.install_chosen().is_none()
            && chimera_assets::config::install_default() != chimera_assets::config::InstallDefault::Install);
    let served_with = |first: bool| -> BTreeSet<String> {
        apps.iter().filter(|g| !(g.installed && settings.use_install && !first)).map(|g| g.game.clone()).collect()
    };
    let without = served_with(settings.service_first);
    let session = AssetSession { settings: Some(settings.clone()), served: served_with(first), access: None, account: None };
    let host = AssetHost::new(offered_url, offered_fingerprint);
    let token = configured_token(config.assets.token.as_deref());
    let files = from_session(&session, &host, token.as_deref()).or_else(|| {
        let session = AssetSession { served: without.clone(), ..session.clone() };
        from_session(&session, &host, token.as_deref())
    });
    let Some(files) = files else {
        let session = AssetSession { served: without, ..session };
        if !session.served.is_empty() {
            eprintln!(
                "assets: no install of {} and no credential for the asset service: the server reads only what its \
                 asset store holds (docs/playtest-hosting.md, \"The server's own game files\")",
                session.served.iter().cloned().collect::<Vec<_>>().join(", ")
            );
        }
        return None;
    };
    Some(fetch_at_start(files, map))
}

/// `startup`'s download with `files` for start map `map`: the
/// credential bound in the process when it is a token sent to the offered
/// service, the account's ticket kept fresh, then the sets fetched.
pub fn fetch_at_start(files: ServerFiles, map: &str) -> ServerFiles {
    let remote = files.settings.remote.clone();
    let url = remote.as_ref().map_or(String::new(), |r| r.url.clone());
    if files.via == Via::Offered
        && let Some(r) = remote
    {
        // The mounts opened from now on fetch from it too (`late_fetcher`).
        access::bind_live(r);
    }
    if files.via != Via::Offered {
        assets::keep_account_fresh();
    }
    let wanted = startup_wanted(&files.served, Some(map));
    eprintln!(
        "assets: the server reads {} from the asset store, with {} ({url})",
        files.served.iter().cloned().collect::<Vec<_>>().join(", "),
        files.via.words()
    );
    if !wanted.is_empty() {
        eprintln!("assets: fetching {WHAT} before it starts: {}", describe(&wanted));
        match start(&files, wanted).wait_saying(PROGRESS_EVERY) {
            Ok(summary) => eprintln!("assets: {summary}"),
            Err(e) => eprintln!("warning: {WHAT} didn't arrive ({e}); what the server reads comes on demand"),
        }
    }
    files
}

fn describe(wanted: &[Wanted]) -> String {
    wanted.iter().map(|w| format!("{} {}", w.game, w.sets.join(", "))).collect::<Vec<_>>().join("; ")
}

/// The match's sets now, of the games read from the store alone.
pub fn match_wanted(world: &mut World, served: &BTreeSet<String>) -> Vec<Wanted> {
    let mut m = manifest::current(world);
    // The map coming (a `changelevel`'s, named before it loads), not the
    // one still served until then.
    let coming = crate::net::current_map(world);
    if coming != crate::net::GREYBOX {
        m.map = coming;
    }
    assets::match_sets(world, &m)
        .into_iter()
        .filter(|g| served.contains(&g.game))
        .map(|g| Wanted { game: g.game, sets: g.sets })
        .collect()
}

/// Looks at the match's sets now and, when they changed since the last
/// look, starts a job that looks at the store and fetches what it lacks
/// (both on its thread: a look reads every served game's manifest and
/// each record of the sets, up to a second for five games' manifests,
/// which every second on the main thread froze a listen host's frames:
/// docs/performance.md, "Stutter on Beaver Creek"); the fetch under way
/// (just started, or one before), if any. The same sets aren't looked at
/// again (nor fetched: a set the service lacks stays missing) until they
/// change.
pub fn fetch_now(world: &mut World) -> Option<Arc<Job>> {
    let running = world.get_resource::<ServerFetching>().map(|f| f.0.clone());
    let files = world.get_resource::<ServerFiles>()?.clone();
    let wanted = match_wanted(world, &files.served);
    let mut fetched = world.get_resource::<Fetched>().cloned().unwrap_or_default();
    // Read after the look (which may insert what it reads: `ServedMap`).
    fetched.inputs = inputs_now(world);
    fetched.computed += 1;
    fetched.map = crate::net::current_map(world);
    let again = fetched.wanted == wanted;
    // A fetch under way for an older look at the match (the map changes
    // while it runs): another for the sets now, done after it.
    let job = (!wanted.is_empty() && !again).then(|| {
        fetched.wanted = wanted.clone();
        fetched.looks += 1;
        start_checked(&files, wanted, running.clone(), fetched.map.clone())
    });
    world.insert_resource(fetched);
    let Some(job) = job else { return running };
    world.insert_resource(ServerFetching(job.clone()));
    Some(job)
}

/// How many looks at the store for the match's sets this server started
/// (`fetch_now`: one each time the sets change; a test's measure that the
/// same sets aren't looked at again).
pub fn store_looks(world: &World) -> usize {
    world.get_resource::<Fetched>().map_or(0, |f| f.looks)
}

/// How many looks at the match's sets this server made (`fetch_now`; a
/// test's measure that a match that doesn't change isn't looked at again).
pub fn match_looks(world: &World) -> usize {
    world.get_resource::<Fetched>().map_or(0, |f| f.computed)
}

/// Whether the server's own fetch is under way (the match waits for it).
pub fn fetching(world: &World) -> bool {
    world.contains_resource::<ServerFetching>()
}

/// Each frame on a server with `ServerFiles`: a fetch's end said, and,
/// when what the match's sets are made of changed (`MatchInputs`: the map
/// loaded or coming among them), the match's sets looked at (`fetch_now`,
/// queued: it builds the match's manifest).
fn watch(
    world: &World,
    fetching: Option<Res<ServerFetching>>,
    fetched: Option<Res<Fetched>>,
    mut commands: Commands,
) {
    if let Some(job) = fetching
        && let Some(result) = job.0.result()
    {
        commands.remove_resource::<ServerFetching>();
        match result {
            // Empty: the store held the sets already (nothing fetched).
            Ok(summary) if summary.is_empty() => {}
            Ok(summary) => info!("assets: {summary}"),
            Err(e) => warn!("assets: {WHAT} didn't arrive ({e}); what the server reads comes on demand"),
        }
    }
    let same = fetched.is_some_and(|f| f.computed > 0 && f.inputs == inputs_now(world));
    if !same {
        commands.queue(|w: &mut World| {
            fetch_now(w);
        });
    }
}

pub(crate) fn plugin(app: &mut App) {
    // What the match's sets are made of (`match_sets`, `manifest::current`
    // and the minigame's and parts' asset names): the map loaded, served
    // and coming, the minigame, its definitions and ladder, the slots, the
    // games, the weapons, the server's files.
    watch_input::<crate::map::LoadedMapName>(app);
    watch_input::<super::maps::ServedMap>(app);
    watch_input::<chimera_sim::minigame::ActiveMinigame>(app);
    watch_input::<chimera_sim::minigame::Minigames>(app);
    watch_input::<chimera_sim::minigame::ladder::Ladder>(app);
    watch_input::<chimera_core::gameplay::Gameplay>(app);
    watch_input::<chimera_core::game_registry::Games>(app);
    watch_input::<chimera_sim::weapon::data::WeaponCatalog>(app);
    watch_input::<chimera_sim::weapon::StartingWeapons>(app);
    watch_input::<ServerFiles>(app);
    app.add_systems(
        Update,
        watch
            .run_if(resource_exists::<ServerFiles>)
            .run_if(resource_equals(crate::core::NetRole::Server)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(url: &str, fp: Option<&str>) -> RemoteConfig {
        RemoteConfig { url: url.into(), token: "chimera1_x".into(), key: None, fingerprint: fp.map(str::to_string) }
    }

    #[test]
    fn the_credential_is_the_configured_service_else_the_account_else_a_token_at_the_offered_one() {
        let offered = Some(("https://h:1", "ab"));
        // The configured service, pinned to the offered certificate when it is the offered one.
        let (r, via) = credential(Some(&remote("https://h:1/", None)), None, Some("t"), offered).unwrap();
        assert_eq!((via, r.fingerprint.as_deref()), (Via::Config, Some("ab")));
        let (r, _) = credential(Some(&remote("http://127.0.0.1:8731", None)), None, None, offered).unwrap();
        assert_eq!(r.fingerprint, None, "another service keeps its own trust");
        // The account.
        let (r, via) = credential(None, Some(remote("https://hub:2", Some("cd"))), Some("t"), offered).unwrap();
        assert_eq!((via, r.url.as_str()), (Via::Account, "https://hub:2"));
        // A token alone goes to the offered service, pinned.
        let (r, via) = credential(None, None, Some(" chimera1_t "), offered).unwrap();
        assert_eq!((via, r.url.as_str(), r.token.as_str(), r.fingerprint.as_deref()), (Via::Offered, "https://h:1", "chimera1_t", Some("ab")));
        assert!(credential(None, None, Some("t"), None).is_none(), "no service to send it to");
        assert!(credential(None, None, None, offered).is_none(), "no credential");
        assert!(credential(None, None, Some("t"), Some(("https://h:1", ""))).is_none(), "never unpinned");
    }

    #[test]
    fn the_startup_wants_each_served_games_base_and_the_start_maps_sets() {
        let served = BTreeSet::from(["a".to_string(), "halo_ce".to_string()]);
        let w = startup_wanted(&served, Some("a:dust"));
        assert_eq!(w.len(), 1, "Halo's base comes with a match: {w:?}");
        assert_eq!(w[0].game, "a");
        assert_eq!(w[0].sets, ["base", "players", "common", "map/dust"]);
        let w = startup_wanted(&served, Some("halo_ce:hangemhigh"));
        assert_eq!(w[1].sets, ["base", "players", "common", "map/hangemhigh"]);
        assert!(startup_wanted(&served, Some("other:map")).iter().all(|w| w.game != "other"), "an installed game's");
        assert!(startup_wanted(&BTreeSet::new(), Some("greybox")).is_empty());
    }
}

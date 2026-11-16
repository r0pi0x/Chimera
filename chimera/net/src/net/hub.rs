//! The hub from a game process (docs/plans/active/hub.md, 4.2-4.3, S2):
//! a server listing itself, and a client reading the hub's document and
//! list. The protocol is `chimera_assets::hub_doc`; the service is
//! `chimera_services::hub`.
//!
//! - **Listing** (`HubListing`): with `chimera:hub 1` (`sv_hub`; `[hub]
//!   list = true` in chimera.local.toml sets it at a binary's start:
//!   `apply_config`) a process serving over UDP (a dedicated server, a
//!   listen server) sends a heartbeat to its account's hub every
//!   `hub_doc::HEARTBEAT_SECS` and soon after its info changes (at most
//!   every `CHANGE_GAP`), from a thread, with that account's ticket (the
//!   user's `account.toml`, or `[hub] account`: a host account's file).
//!   The heartbeat is `query::LocalServerInfo` with the app's game
//!   (`HubGame`), the game port and this machine's private address. The
//!   hub's answer (where it lists the server, whether it reached its UDP
//!   port) is the `status` line `hub`; an unreachable port's hint is
//!   printed once. Stopping (`chimera:hub 0`, serving over) delists at
//!   once; so does quitting, in the quick exit's `Persist` stage (waits at
//!   most `LEAVE_WAIT`), else the listing expires in 90 s.
//! - **Reading** (`Hub`, a client's): the account's hub (its origin and
//!   certificate), the signed document cached in `hub.toml` and
//!   `hub-discovery.json` (used at once at start, fetched again on a
//!   thread when older than `DOCUMENT_AGE`) and the server list
//!   (`refresh_list`, the Join page's Internet tab). A refusal or no
//!   answer leaves the cached document and the last list, marked offline;
//!   LAN, Favourites, History and Direct never wait for it. Apps start it
//!   from the user's account (`use_user_hub`); tests set one.
//!
//! Known shortcuts: heartbeats go to `/hub/heartbeat` on the account's
//! hub, not the document's `register` endpoint (the same today); a server
//! lists one LAN address (the one toward the internet); the list is asked
//! again only when Join opens or F5.

use std::{
    net::{IpAddr, SocketAddr, UdpSocket},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use bevy::prelude::*;
use chimera_assets::{
    account::{self, Session},
    hub_doc::{self, Discovery, Failure, Fetched, Heartbeat, HeartbeatAnswer, HubCache, Reach, ServerList},
};

use super::query::LocalServerInfo;
use crate::{console::resource_cvar, core::NetRole};

/// The least time between two heartbeats for a change.
pub const CHANGE_GAP: Duration = Duration::from_secs(2);
/// How long quitting waits for the hub to delist.
pub const LEAVE_WAIT: Duration = Duration::from_secs(2);
/// A document older than this is fetched again.
pub const DOCUMENT_AGE: Duration = Duration::from_secs(300);

/// The game (app) this process plays on the hub (`ultimate_gun_game`):
/// what a heartbeat says, and the Internet tab's filter. Apps set it.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct HubGame(pub String);

impl Default for HubGame {
    fn default() -> Self {
        Self("chimera".into())
    }
}

// ---------------------------------------------------------------------------
// Listing.

/// Where the listing stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ListState {
    /// `chimera:hub 0`, or not serving over UDP.
    #[default]
    Off,
    /// No account to list with.
    NoAccount(String),
    /// The first heartbeat under way.
    Sending,
    Listed(HeartbeatAnswer),
    /// The last heartbeat failed (kept trying).
    Failed(String),
}

impl ListState {
    /// The `status` line.
    pub fn line(&self) -> Option<String> {
        Some(match self {
            ListState::Off => return None,
            ListState::NoAccount(why) => format!("not listed: {why}"),
            ListState::Sending => "listing on the hub...".into(),
            ListState::Listed(a) => match (a.reachable, &a.hint) {
                (Reach::No, Some(h)) => format!("listed at {}, but {h}", a.address),
                (Reach::Unknown, _) => format!("listed at {} (the hub is checking its UDP port)", a.address),
                _ => format!("listed at {}", a.address),
            },
            ListState::Failed(why) => format!("heartbeat failed: {why} (trying again)"),
        })
    }
}

enum Msg {
    Beat(Box<Heartbeat>),
    Leave(mpsc::Sender<()>),
}

/// The heartbeat thread of a listing.
struct Worker {
    tx: mpsc::Sender<Msg>,
    states: mpsc::Receiver<ListState>,
    last: Option<Heartbeat>,
}

/// `chimera:hub` and the listing.
#[derive(Resource, Default)]
pub struct HubListing {
    /// `chimera:hub` (`sv_hub`): 1 lists this server on the account's hub.
    pub enabled: u8,
    /// The account file to list with (None: the user's).
    pub account: Option<PathBuf>,
    pub state: ListState,
    worker: Mutex<Option<Worker>>,
    /// The hint was printed.
    told: bool,
}

/// `[hub]` in chimera.local.toml: what a binary applies at start.
pub fn apply_config(app: &mut App) {
    let Ok(c) = chimera_core::mount::config::LocalConfig::load() else { return };
    let mut l = app.world_mut().get_resource_or_insert_with(HubListing::default);
    if c.hub.list {
        l.enabled = 1;
    }
    if let Some(a) = c.hub.account {
        l.account = Some(a);
    }
}

/// This machine's address toward the internet (no packet is sent), when
/// it is a private one: the LAN address a heartbeat lists.
fn lan_ip() -> Option<IpAddr> {
    let s = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(("192.0.2.1", 9)).ok()?;
    let ip = s.local_addr().ok()?.ip();
    match ip {
        IpAddr::V4(v4) if v4.is_private() => Some(ip),
        _ => None,
    }
}

fn heartbeat_of(info: &LocalServerInfo, game: &str, port: u16, lan: Option<IpAddr>) -> Heartbeat {
    let i = &info.0;
    Heartbeat {
        instance: format!("{:016x}", i.instance.unwrap_or(0)),
        port,
        lan: lan.map(|ip| vec![SocketAddr::new(ip, port).to_string()]).unwrap_or_default(),
        name: i.name.clone(),
        game: game.to_string(),
        minigame: i.minigame().map(str::to_string),
        map: i.map.clone(),
        players: i.players,
        max: i.max_players,
        bots: i.bots,
        password: i.password,
        dedicated: i.dedicated,
        net: i.version.clone(),
    }
}

/// The heartbeat thread: the latest info sent every `every` (the hub's
/// answer says) and soon after a change; a leave on its way out.
fn run_worker(session: Arc<Session>, rx: mpsc::Receiver<Msg>, states: mpsc::Sender<ListState>) {
    let file = session.file();
    let (origin, fingerprint) = (file.url.clone(), file.fingerprint.clone());
    let register = Discovery::url(&origin, "/hub/heartbeat");
    let leave = Discovery::url(&origin, "/hub/leave");
    let mut beat: Option<Heartbeat> = None;
    let mut sent: Option<(Heartbeat, Instant)> = None;
    let mut every = Duration::from_secs(hub_doc::HEARTBEAT_SECS);
    loop {
        let wait = match (&beat, &sent) {
            (Some(b), Some((s, at))) if b != s => CHANGE_GAP.saturating_sub(at.elapsed()),
            (Some(_), Some((_, at))) => every.saturating_sub(at.elapsed()),
            (Some(_), None) => Duration::ZERO,
            (None, _) => Duration::from_secs(3600),
        };
        match rx.recv_timeout(wait) {
            Ok(Msg::Beat(b)) => {
                beat = Some(*b);
                continue;
            }
            Ok(Msg::Leave(done)) => {
                if let (Some(b), Some(_)) = (&beat, &sent)
                    && let Ok(t) = session.ticket()
                    && let Err(e) = hub_doc::leave(&leave, fingerprint.as_deref(), &t, &b.instance)
                {
                    warn!("hub: leaving: {e}");
                }
                let _ = done.send(());
                return;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let Some(b) = beat.clone() else { continue };
        let result = session
            .ticket()
            .map_err(|e| Failure { status: None, message: format!("no ticket ({e})") })
            .and_then(|t| hub_doc::heartbeat(&register, fingerprint.as_deref(), &t, &b));
        sent = Some((b, Instant::now()));
        let state = match result {
            Ok(a) => {
                every = Duration::from_secs(a.every.clamp(5, 300));
                ListState::Listed(a)
            }
            // Too soon: the hub has the listing; the next beat comes.
            Err(Failure { status: Some(429), message }) if message.contains("at most every") => continue,
            Err(e) => ListState::Failed(e.to_string()),
        };
        if states.send(state).is_err() {
            return;
        }
    }
}

fn session_for(path: Option<&std::path::Path>) -> Result<Arc<Session>, String> {
    let s = match path {
        Some(p) => account::session_at(p),
        None => account::user_session(),
    };
    s.ok_or_else(|| "no account on this machine (sign in to a hub first: +chimera:hub <invite link>)".into())
}

/// Keep the listing while serving over UDP with `chimera:hub 1`.
fn keep_listed(
    role: Res<NetRole>,
    info: Res<LocalServerInfo>,
    game: Res<HubGame>,
    server: Option<Res<super::udp::UdpServer>>,
    mut listing: ResMut<HubListing>,
) {
    let port = server.as_ref().and_then(|s| s.local_addr()).map(|a| a.port());
    let want = *role == NetRole::Server && listing.enabled != 0 && info.0.instance.is_some();
    let l = &mut *listing;
    let mut worker = l.worker.lock().unwrap_or_else(|e| e.into_inner());
    let Some(port) = port.filter(|_| want) else {
        if let Some(w) = worker.take() {
            let (done, _) = mpsc::channel();
            let _ = w.tx.send(Msg::Leave(done));
        }
        l.state = ListState::Off;
        return;
    };
    if worker.is_none() {
        if matches!(l.state, ListState::NoAccount(_)) {
            return;
        }
        match session_for(l.account.as_deref()) {
            Ok(session) => {
                let (tx, rx) = mpsc::channel();
                let (stx, srx) = mpsc::channel();
                std::thread::Builder::new()
                    .name("hub-heartbeat".into())
                    .spawn(move || run_worker(session, rx, stx))
                    .expect("a thread");
                *worker = Some(Worker { tx, states: srx, last: None });
                l.state = ListState::Sending;
                l.told = false;
            }
            Err(e) => {
                warn!("hub: {e}");
                l.state = ListState::NoAccount(e);
                return;
            }
        }
    }
    let w = worker.as_mut().expect("started");
    let beat = heartbeat_of(&info, &game.0, port, lan_ip());
    if w.last.as_ref() != Some(&beat) {
        w.last = Some(beat.clone());
        let _ = w.tx.send(Msg::Beat(Box::new(beat)));
    }
    let mut newest = None;
    while let Ok(s) = w.states.try_recv() {
        newest = Some(s);
    }
    drop(worker);
    if let Some(s) = newest {
        if let ListState::Listed(a) = &s {
            if !l.told {
                info!("hub: {}", s.line().unwrap_or_default());
            }
            l.told |= a.reachable != Reach::Unknown;
        } else if let ListState::Failed(_) = &s
            && l.state != s
        {
            warn!("hub: {}", s.line().unwrap_or_default());
        }
        l.state = s;
    }
}

/// Quitting while listed: delist (waiting at most `LEAVE_WAIT`).
fn leave_on_exit(world: &mut World) {
    let Some(l) = world.get_resource::<HubListing>() else { return };
    let Some(w) = l.worker.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let (done, wait) = mpsc::channel();
    if w.tx.send(Msg::Leave(done)).is_ok() {
        let _ = wait.recv_timeout(LEAVE_WAIT);
    }
}

// ---------------------------------------------------------------------------
// Reading.

enum Fetch {
    Document(Result<Fetched, Failure>),
    List(Result<ServerList, Failure>),
}

/// A client's hub: its document (cached) and its server list.
#[derive(Resource, Default)]
pub struct Hub {
    /// The hub's origin (none: no hub; the Internet tab says so).
    pub origin: Option<String>,
    pub fingerprint: Option<String>,
    /// The account file whose tickets ask (None: the user's).
    pub account: Option<PathBuf>,
    /// `hub.toml` (None: nothing cached; tests).
    pub cache: Option<PathBuf>,
    /// The pinned document key.
    pub key: Option<String>,
    pub dismissed: Vec<String>,
    /// The document: fetched, or the cached one.
    pub doc: Option<Discovery>,
    /// When the document was last fetched in this run.
    pub fetched: Option<Instant>,
    /// The last list, and when it came.
    pub list: Option<ServerList>,
    pub listed_at: Option<Instant>,
    /// Why the last call failed (shown: the hub offline, a refusal).
    pub error: Option<String>,
    /// A call under way.
    pub busy: bool,
    /// Bumped at each answer (the browser rebuilds its rows).
    pub generation: u64,
    pending: Mutex<Vec<mpsc::Receiver<Fetch>>>,
}

impl Hub {
    /// The hub of the account file at `account` (None: the user's), with
    /// the cache at `cache`: its document at once if it verifies.
    pub fn of_account(account: Option<PathBuf>, cache: Option<PathBuf>) -> Option<Self> {
        let path = account.clone().or_else(account::AccountFile::path)?;
        let file = account::AccountFile::load_from(&path).ok().flatten()?;
        let mut hub = Hub { origin: Some(file.url.clone()), fingerprint: file.fingerprint.clone(), account, cache, ..default() };
        if let Some(c) = hub.cache.as_deref().and_then(HubCache::load_from).filter(|c| c.url == file.url) {
            hub.doc = c.cached_document(hub.cache.as_deref().expect("cache"));
            hub.key = c.key;
            hub.dismissed = c.dismissed;
        }
        Some(hub)
    }

    pub fn signed_in(&self) -> bool {
        self.origin.is_some()
    }

    /// Offline: the last call failed.
    pub fn offline(&self) -> bool {
        self.error.is_some()
    }

    fn spawn(&self, job: impl FnOnce() -> Fetch + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("hub-fetch".into())
            .spawn(move || {
                let _ = tx.send(job());
            })
            .expect("a thread");
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).push(rx);
    }

    fn session(&self) -> Result<Arc<Session>, String> {
        session_for(self.account.as_deref())
    }

    /// Fetch the document if none was fetched in this run or it is older
    /// than `DOCUMENT_AGE`.
    pub fn refresh_document(&mut self) {
        let Some(origin) = self.origin.clone() else { return };
        if self.fetched.is_some_and(|t| t.elapsed() < DOCUMENT_AGE) {
            return;
        }
        self.fetched = Some(Instant::now());
        self.busy = true;
        let (fingerprint, key, session) = (self.fingerprint.clone(), self.key.clone(), self.session());
        self.spawn(move || {
            let r = session.map_err(|e| Failure { status: None, message: e }).and_then(|s| {
                let t = s.ticket().map_err(|e| Failure { status: None, message: e })?;
                hub_doc::fetch_document(&origin, fingerprint.as_deref(), &t, key.as_deref())
            });
            Fetch::Document(r)
        });
    }

    /// Ask the server list again (and the document when it is old).
    pub fn refresh_list(&mut self) {
        self.refresh_document();
        let Some(origin) = self.origin.clone() else { return };
        let url = Discovery::url(&origin, self.doc.as_ref().map_or("/hub/servers", |d| d.servers.url.as_str()));
        self.busy = true;
        let (fingerprint, session) = (self.fingerprint.clone(), self.session());
        self.spawn(move || {
            let r = session.map_err(|e| Failure { status: None, message: e }).and_then(|s| {
                let t = s.ticket().map_err(|e| Failure { status: None, message: e })?;
                hub_doc::fetch_servers(&url, fingerprint.as_deref(), &t)
            });
            Fetch::List(r)
        });
    }

    /// Take the answers that came.
    pub fn poll(&mut self) {
        let answers: Vec<Fetch> = {
            let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            let mut out = Vec::new();
            p.retain(|rx| match rx.try_recv() {
                Ok(f) => {
                    out.push(f);
                    false
                }
                Err(mpsc::TryRecvError::Empty) => true,
                Err(mpsc::TryRecvError::Disconnected) => false,
            });
            out
        };
        if answers.is_empty() {
            return;
        }
        for a in answers {
            match a {
                Fetch::Document(Ok(f)) => {
                    self.error = None;
                    if self.key.as_deref() != Some(f.key.as_str()) {
                        self.key = Some(f.key.clone());
                    }
                    if let Some(path) = self.cache.clone() {
                        let c = HubCache {
                            url: self.origin.clone().unwrap_or_default(),
                            fingerprint: self.fingerprint.clone(),
                            key: self.key.clone(),
                            dismissed: self.dismissed.clone(),
                        };
                        if let Err(e) = c.save_to(&path).and_then(|()| HubCache::save_document(&path, &f)) {
                            warn!("hub: keeping its document: {e}");
                        }
                    }
                    self.doc = Some(f.doc);
                }
                Fetch::List(Ok(l)) => {
                    self.error = None;
                    self.list = Some(l);
                    self.listed_at = Some(Instant::now());
                }
                Fetch::Document(Err(e)) | Fetch::List(Err(e)) => {
                    // A document refused may be fetched again at the next ask.
                    if e.status.is_none() || e.status == Some(401) {
                        self.fetched = None;
                    }
                    self.error = Some(e.to_string());
                }
            }
        }
        self.busy = !self.pending.lock().unwrap_or_else(|e| e.into_inner()).is_empty();
        self.generation += 1;
    }

    /// Dismiss news item `id` (kept in `hub.toml`).
    pub fn dismiss(&mut self, id: &str) {
        if !self.dismissed.iter().any(|d| d == id) {
            self.dismissed.push(id.to_string());
        }
        if let (Some(path), Some(origin)) = (self.cache.clone(), self.origin.clone()) {
            let c = HubCache { url: origin, fingerprint: self.fingerprint.clone(), key: self.key.clone(), dismissed: self.dismissed.clone() };
            let _ = c.save_to(&path);
        }
    }
}

/// The user's account's hub, its cache in the user's folder (an app's
/// start: never under tests), its document fetched.
pub fn use_user_hub(mut commands: Commands, existing: Option<Res<Hub>>) {
    if existing.is_some_and(|h| h.signed_in()) {
        return;
    }
    if let Some(mut hub) = Hub::of_account(None, HubCache::path()) {
        hub.refresh_document();
        commands.insert_resource(hub);
    }
}

fn poll_hub(hub: Option<ResMut<Hub>>) {
    if let Some(mut h) = hub
        && !h.pending.lock().unwrap_or_else(|e| e.into_inner()).is_empty()
    {
        h.poll();
    }
}

pub(super) fn plugin(app: &mut App) {
    use chimera_core::shutdown::{ShutdownAppExt, Stage};
    app.init_resource::<HubListing>().init_resource::<HubGame>();
    resource_cvar::<HubListing, u8>(
        app,
        "chimera:hub",
        "1: while serving, list this server on the hub of this machine's account (heartbeats; delisted when it stops)",
        |l| &mut l.enabled,
    );
    app.add_systems(Update, (keep_listed, poll_hub));
    app.shutdown_flush(Stage::Persist, "hub leave", leave_on_exit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heartbeat_from_the_servers_info() {
        let info = LocalServerInfo(super::super::query::ServerInfo {
            name: "Ours".into(),
            map: "de_dust2".into(),
            players: 3,
            max_players: 8,
            bots: 1,
            dedicated: true,
            version: "0.1.0/net54".into(),
            instance: Some(0xab),
            keywords: Some("cs_source,minigame=ultimate_gun_game:auto".into()),
            ..default()
        });
        let b = heartbeat_of(&info, "ultimate_gun_game", 27031, Some("192.168.1.9".parse().unwrap()));
        assert_eq!(b.instance, "00000000000000ab");
        assert_eq!(b.minigame.as_deref(), Some("ultimate_gun_game:auto"));
        assert_eq!(b.lan, vec!["192.168.1.9:27031".to_string()]);
        assert_eq!((b.port, b.players, b.max, b.bots), (27031, 3, 8, 1));
    }

    #[test]
    fn status_lines() {
        assert_eq!(ListState::Off.line(), None);
        let a = HeartbeatAnswer { address: "203.0.113.5:27015".into(), reachable: Reach::No, hint: Some("forward it".into()), every: 30 };
        assert!(ListState::Listed(a).line().unwrap().contains("forward it"));
    }
}

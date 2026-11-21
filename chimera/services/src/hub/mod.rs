//! The hub service (docs/plans/active/hub.md, section 4): the discovery
//! document, the server registry and its reachability probe. `Hub` is its
//! logic over `hub.db` (`db`, `HUB_MIGRATIONS`), `http` its routes, binary
//! `chimera_hub` (loopback 8742, behind the front at `/hub`).
//!
//! - **Who**: every route but the public key needs an account's ticket
//!   (`services` audience), checked locally with the accounts service's
//!   public keys (`<store>/server/accounts/ticket.pub`) and its revocation
//!   feed, polled every 30 s (`chimera_assets::accounts::TicketAuth`): no
//!   anonymous access, the list included (decision 10).
//! - **The document** (`document`, `GET /hub`): `hub_doc::Discovery`
//!   from the active identity's `hub/hub.toml` (`crate::identity`;
//!   `main`'s is `<store>/server/hub/hub.toml`; `HubConfig`: name,
//!   MOTD, news, the oldest network version and a download link, the
//!   public address, endpoints moved elsewhere, `next_fingerprint`; read
//!   again when it changes, so an edit needs no restart) and the process
//!   (the ticket keys, the asset service's fingerprint and manifest key),
//!   signed with the manifest key (`<store>/server/signing.key`, the
//!   asset service's) in `X-Chimera-Signature`; `GET /hub/key` serves its
//!   public half to anyone.
//! - **Heartbeats** (`heartbeat`): a ticket whose `list` right is above
//!   0 lists a server (403 without it); one listing per instance, at most
//!   `list` at once per account (429 past it), a heartbeat at most every
//!   `Options::min_interval` unless its info changed (then at most a
//!   second apart; 429). The address is the one the hub sees (the front's
//!   `X-Chimera-Peer`, believed from loopback only) with the body's port:
//!   a body never names a machine. A heartbeat from this machine (a
//!   loopback peer) is listed at hub.toml's `public_address` when set.
//!   `lan` keeps private IPv4 addresses only. Text is cleaned: names to
//!   64 characters with no control characters, ids to `[a-z0-9_:.-]`,
//!   anything else 400.
//! - **Expiry and leaving** (`sweep`): a listing without a heartbeat for
//!   `Options::expiry` (90 s) goes, as does one whose account or device
//!   the revocation feed names after its ticket; `POST /hub/leave` delists
//!   at once.
//! - **The probe**: after a listing's first heartbeat and every
//!   `Options::probe_every` (5 minutes), at most once a minute, one
//!   A2S_INFO from the hub's UDP socket to the address it saw the
//!   heartbeat come from (`chimera_formats::a2s::query_blocking`, on a
//!   thread). A listing the hub couldn't reach is hidden from others and
//!   shown to its own account with a hint, which the heartbeat's answer
//!   carries too.
//! - **The list** (`list`, `GET /hub/servers`): the live listings, at most
//!   `hub_doc::MAX_LISTED`, from memory.
//! - **The owner** (`admin_list`, `delist`; loopback with the services'
//!   secret, `chimera_hub servers` and `chimera_hub delist <instance>`):
//!   every listing with its account, reachable or not, and taking one
//!   off the list: its heartbeats are refused (403) for `DELIST_SECS`
//!   (or the time given), so it doesn't come back with the next one. The
//!   refusals are in memory: a restart of the hub forgets them.
//! - **Parties** (`party`, `POST /hub/party/...`): session-only parties
//!   joined by invite code (docs/plans/active/parties.md).
//! - `hub.db` keeps the listings across a restart (no backup: they come
//!   back with the next heartbeats).
//!
//! Known shortcuts: a server on the hub's own machine is probed on
//! loopback (it answering says nothing of the router's forward); a
//! listing's revocation is noticed at the next sweep (each request, and
//! every few seconds); the probe is IPv4 and IPv6 alike but nothing else
//! of the hub was tried over IPv6.

pub mod http;
pub mod party;

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak, mpsc},
    time::{Duration, SystemTime},
};

use chimera_assets::{
    accounts::TicketAuth,
    hub_doc::{
        AccountsEndpoint, AssetsEndpoint, ClientInfo, Discovery, EXPIRY_SECS, Endpoint, HEARTBEAT_SECS, Heartbeat,
        HeartbeatAnswer, Listing, MAX_LISTED, News, Reach, SCHEMA, ServerList,
    },
    store::hex,
    ticket::{Claims, Id},
};
use ed25519_dalek::SigningKey;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::{
    accounts::{Clock, R, Refusal},
    db,
};

/// The hub's loopback port by default.
pub const DEFAULT_PORT: u16 = crate::config::HUB_PORT;
/// How long the document is good for, s.
pub const DOCUMENT_SECS: u64 = 7 * 86_400;

fn refuse(status: u16, message: impl Into<String>) -> Refusal {
    Refusal { status, message: message.into() }
}

fn internal(e: impl std::fmt::Display) -> Refusal {
    eprintln!("hub: {e}");
    refuse(500, "the hub failed; try again later")
}

/// Where the hub keeps its files in a server store.
#[derive(Clone, Debug)]
pub struct Paths {
    pub store: PathBuf,
    /// The active hub identity's folder (`crate::identity::root`).
    pub root: PathBuf,
}

impl Paths {
    /// The store with identity `main` (the store's own folders).
    pub fn new(store: impl Into<PathBuf>) -> Self {
        let store = store.into();
        Self { root: crate::identity::root(&store, crate::identity::MAIN), store }
    }
    /// The store with its active identity (`services.toml`'s `[hub]`).
    pub fn active(store: impl Into<PathBuf>) -> Result<Self, String> {
        let store = store.into();
        Ok(Self { root: crate::identity::active_root(&store)?, store })
    }
    pub fn server(&self) -> PathBuf {
        self.store.join("server")
    }
    pub fn dir(&self) -> PathBuf {
        self.root.join("hub")
    }
    pub fn db(&self) -> PathBuf {
        self.dir().join("hub.db")
    }
    /// The host's settings (`HubConfig`).
    pub fn config(&self) -> PathBuf {
        self.dir().join("hub.toml")
    }
    /// The document key: the asset service's manifest key.
    pub fn signing_key(&self) -> PathBuf {
        self.server().join("signing.key")
    }
    pub fn ticket_pub(&self) -> PathBuf {
        chimera_assets::accounts::ticket_keys_path(&self.store)
    }
    pub fn secret(&self) -> PathBuf {
        chimera_assets::accounts::service_secret_path(&self.store)
    }
    pub fn tls(&self) -> PathBuf {
        self.server().join("tls")
    }
    /// The identity's backups (`main`'s: `<store>/server/backups`).
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }
    pub fn access_log(&self) -> PathBuf {
        self.dir().join("access.log")
    }
}

/// The active identity's `hub/hub.toml` (`main`'s:
/// `<store>/server/hub/hub.toml`), written by the host; every key
/// optional. A blank name shows `crate::identity::DEFAULT_NAME`.
///
/// ```toml
/// name = "<hub name>"
/// motd = "Playtest Saturday 13:00 UTC"
/// public_address = "<public address>"   # servers on this machine are listed there
/// min_net = 54
/// recommended_build = "0.1.0"
/// download = "<release page>"
/// [[news]]
/// id = "2026-11-14a"
/// date = "2026-11-14"
/// title = "..."
/// text = "..."
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub motd: String,
    /// The IP servers on this machine are listed at.
    #[serde(default)]
    pub public_address: Option<String>,
    #[serde(default)]
    pub min_net: u32,
    #[serde(default)]
    pub recommended_build: String,
    #[serde(default)]
    pub download: String,
    #[serde(default)]
    pub next_fingerprint: Option<String>,
    /// Endpoints moved off this hub's origin (absolute URLs).
    #[serde(default)]
    pub assets_url: Option<String>,
    #[serde(default)]
    pub assets_fingerprint: Option<String>,
    #[serde(default)]
    pub feedback_url: Option<String>,
    #[serde(default)]
    pub news: Vec<News>,
}

impl HubConfig {
    pub fn parse(text: &str) -> Result<Self, String> {
        let c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if let Some(a) = &c.public_address
            && a.parse::<IpAddr>().is_err()
        {
            return Err(format!("public_address {a:?}: an IP address"));
        }
        Ok(c)
    }
}

/// The service's settings.
#[derive(Clone, Debug)]
pub struct Options {
    /// A listing without a heartbeat for this long goes, s.
    pub expiry: u64,
    /// The heartbeat interval asked of servers, s.
    pub every: u64,
    /// The least time between two heartbeats with the same info, s.
    pub min_interval: u64,
    /// How often a listing is probed again, s.
    pub probe_every: u64,
    /// How long the probe waits for an answer.
    pub probe_wait: Duration,
    /// How often the revocation feed is asked (tests: shorter).
    pub feed_every: Duration,
    /// A party member unseen this long is dropped, s.
    pub member_expiry: u64,
    /// A leader offline this long, another member online: the lead
    /// passes, s.
    pub lead_handover: u64,
    /// Party calls an account may make a minute.
    pub party_calls: usize,
    /// Wrong party codes an account or an address may send in
    /// `party::CODE_WINDOW`.
    pub code_failures: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            expiry: EXPIRY_SECS,
            every: HEARTBEAT_SECS,
            min_interval: 10,
            probe_every: 300,
            probe_wait: Duration::from_millis(1500),
            feed_every: chimera_assets::accounts::FEED_EVERY,
            member_expiry: 6 * 3600,
            lead_handover: 120,
            party_calls: 60,
            code_failures: 10,
        }
    }
}

/// A listing in memory.
#[derive(Clone, Debug)]
struct Entry {
    account: Id,
    device: Id,
    issued: u64,
    /// Listed at.
    address: SocketAddr,
    /// Where the heartbeat came from (what the probe asks).
    probe: SocketAddr,
    beat: Heartbeat,
    reachable: Reach,
    probed_at: u64,
    probing: bool,
    first_seen: u64,
    last: u64,
}

impl Entry {
    fn hint(&self) -> Option<String> {
        (self.reachable == Reach::No).then(|| {
            format!(
                "the hub can't reach UDP {} at {}: forward that port on the router to this machine, or try net_upnp 1",
                self.address.port(),
                self.address.ip()
            )
        })
    }

    fn listing(&self, mine: bool) -> Listing {
        let b = &self.beat;
        Listing {
            addr: self.address.to_string(),
            lan: b.lan.clone(),
            instance: b.instance.clone(),
            name: b.name.clone(),
            game: b.game.clone(),
            minigame: b.minigame.clone(),
            map: b.map.clone(),
            players: b.players,
            max: b.max,
            bots: b.bots,
            password: b.password,
            dedicated: b.dedicated,
            net: b.net.clone(),
            reachable: self.reachable,
            since: self.first_seen,
            mine,
            hint: if mine { self.hint() } else { None },
        }
    }
}

fn reach_text(r: Reach) -> &'static str {
    match r {
        Reach::Unknown => "unknown",
        Reach::Yes => "yes",
        Reach::No => "no",
    }
}

fn reach_of(text: &str) -> Reach {
    match text {
        "yes" => Reach::Yes,
        "no" => Reach::No,
        _ => Reach::Unknown,
    }
}

fn id_of(bytes: Vec<u8>) -> Id {
    let mut id = [0u8; 16];
    let n = bytes.len().min(16);
    id[..n].copy_from_slice(&bytes[..n]);
    id
}

/// Whether `ip` is on a private network (a LAN address worth listing).
fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1])),
        IpAddr::V6(_) => false,
    }
}

fn clean_id(what: &str, text: &str, extra: &[u8], max: usize) -> R<String> {
    let t = text.trim().to_ascii_lowercase();
    if t.is_empty() || t.len() > max || !t.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || extra.contains(&b)) {
        return Err(refuse(400, format!("{what} {text:?}: at most {max} of [a-z0-9{}]", String::from_utf8_lossy(extra))));
    }
    Ok(t)
}

/// A heartbeat's text cleaned (names cut to 64 characters; anything with
/// control characters or outside the ids' alphabet refused).
pub fn clean(beat: &Heartbeat) -> R<Heartbeat> {
    let mut b = beat.clone();
    if b.instance.len() != 16 || !b.instance.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(refuse(400, "instance: 16 hex digits"));
    }
    b.instance = b.instance.to_ascii_lowercase();
    if b.port == 0 {
        return Err(refuse(400, "port: the game's UDP port"));
    }
    if b.name.chars().any(char::is_control) {
        return Err(refuse(400, "name: no control characters"));
    }
    b.name = b.name.trim().chars().take(64).collect();
    b.game = clean_id("game", &b.game, b"_:-", 64)?;
    b.minigame = match b.minigame.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => Some(clean_id("minigame", m, b"_:.-", 64)?),
        None => None,
    };
    b.map = if b.map.trim().is_empty() { String::new() } else { clean_id("map", &b.map, b"_:.-", 64)? };
    if b.net.is_empty() || b.net.len() > 32 || !b.net.bytes().all(|c| c.is_ascii_alphanumeric() || b"_./-".contains(&c)) {
        return Err(refuse(400, "net: the build's network version"));
    }
    b.lan = beat
        .lan
        .iter()
        .filter_map(|a| a.parse::<SocketAddr>().ok())
        .filter(|a| private(a.ip()) && a.port() != 0)
        .take(4)
        .map(|a| a.to_string())
        .collect();
    b.max = b.max.max(b.players.min(255));
    Ok(b)
}

/// How long a delisted server's heartbeats are refused without a time
/// given, s.
pub const DELIST_SECS: u64 = 3600;

/// A listing as the owner sees it (`GET /admin/servers`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminListing {
    /// The listing's account (hex).
    pub account: String,
    /// Where the heartbeats come from (what the probe asks).
    pub seen: String,
    /// The last heartbeat (unix seconds).
    pub last: u64,
    #[serde(flatten)]
    pub listing: Listing,
}

/// `POST /admin/delist`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delist {
    pub instance: String,
    /// Refuse its heartbeats for this long, s (none: `DELIST_SECS`).
    #[serde(default)]
    pub secs: Option<u64>,
}

/// Its answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delisted {
    /// It was listed.
    pub was_listed: bool,
    /// Its heartbeats are refused until (unix seconds).
    pub until: u64,
}

/// The hub service.
pub struct Hub {
    pub paths: Paths,
    conn: Mutex<Connection>,
    listings: Mutex<HashMap<String, Entry>>,
    /// Instances the owner took off the list, and until when.
    delisted: Mutex<HashMap<String, u64>>,
    auth: TicketAuth,
    key: SigningKey,
    pub clock: Clock,
    options: Options,
    config: Mutex<(Option<(SystemTime, u64)>, HubConfig)>,
    probes: Mutex<Option<mpsc::Sender<(String, SocketAddr)>>>,
    parties: Mutex<party::Book>,
}

impl Hub {
    /// Open the hub's database (migrated, its listings loaded), the
    /// document key and the ticket check (with the revocation feed from
    /// `accounts_url`, none: without), and start the probe.
    pub fn open(paths: Paths, options: Options, clock: Clock, accounts_url: Option<String>) -> Result<Arc<Self>, String> {
        std::fs::create_dir_all(paths.dir()).map_err(|e| format!("{}: {e}", paths.dir().display()))?;
        let conn = db::open(&paths.db(), db::HUB_MIGRATIONS, &paths.backups())?;
        let key = chimera_assets::sign::load_or_create_key(&paths.signing_key())
            .map_err(|e| format!("{}: {e}", paths.signing_key().display()))?;
        let mut auth = TicketAuth::new(paths.ticket_pub());
        if let Some(url) = accounts_url {
            auth = auth.with_feed(url, paths.secret(), options.feed_every);
        }
        let hub = Arc::new(Self {
            paths,
            conn: Mutex::new(conn),
            listings: Mutex::default(),
            delisted: Mutex::default(),
            auth,
            key,
            clock,
            options,
            config: Mutex::new((None, HubConfig::default())),
            probes: Mutex::new(None),
            parties: Mutex::default(),
        });
        hub.load()?;
        hub.load_parties()?;
        let (tx, rx) = mpsc::channel();
        *hub.probes.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
        let weak = Arc::downgrade(&hub);
        std::thread::Builder::new()
            .name("hub-probe".into())
            .spawn(move || probe_loop(weak, rx))
            .map_err(|e| e.to_string())?;
        Ok(hub)
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.listings.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn now(&self) -> u64 {
        self.clock.now()
    }

    /// The database's schema version.
    pub fn db_version(&self) -> usize {
        db::version(&self.db()).unwrap_or(0)
    }

    /// The ticket check's word for `/health` (the revocation feed's age).
    pub fn auth_status(&self) -> String {
        use chimera_assets::accounts::Authenticator;
        self.auth.status().unwrap_or_default()
    }

    fn load(&self) -> Result<(), String> {
        let conn = self.db();
        let mut stmt = conn
            .prepare("SELECT instance, account_id, device_id, issued, address, probe, beat, reachable, probed_at, first_seen, last_heartbeat FROM servers")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, i64>(9)?,
                    r.get::<_, i64>(10)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut map = self.entries();
        for row in rows {
            let (instance, account, device, issued, address, probe, beat, reachable, probed_at, first, last) =
                row.map_err(|e| e.to_string())?;
            let (Ok(address), Ok(probe), Ok(beat)) =
                (address.parse(), probe.parse(), serde_json::from_str::<Heartbeat>(&beat))
            else {
                continue;
            };
            map.insert(
                instance,
                Entry {
                    account: id_of(account),
                    device: id_of(device),
                    issued: issued.max(0) as u64,
                    address,
                    probe,
                    beat,
                    reachable: reach_of(&reachable),
                    probed_at: probed_at.max(0) as u64,
                    probing: false,
                    first_seen: first.max(0) as u64,
                    last: last.max(0) as u64,
                },
            );
        }
        Ok(())
    }

    fn store(&self, instance: &str, e: &Entry) -> R<()> {
        self.db()
            .execute(
                "INSERT OR REPLACE INTO servers (instance, account_id, device_id, issued, address, probe, beat, reachable, probed_at, first_seen, last_heartbeat)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    instance,
                    e.account.to_vec(),
                    e.device.to_vec(),
                    e.issued as i64,
                    e.address.to_string(),
                    e.probe.to_string(),
                    serde_json::to_string(&e.beat).unwrap_or_default(),
                    reach_text(e.reachable),
                    e.probed_at as i64,
                    e.first_seen as i64,
                    e.last as i64
                ],
            )
            .map(drop)
            .map_err(internal)
    }

    fn forget(&self, instances: &[String]) {
        let conn = self.db();
        for i in instances {
            if let Err(e) = conn.execute("DELETE FROM servers WHERE instance = ?1", params![i]) {
                eprintln!("hub: {e}");
            }
        }
    }

    /// The host's settings (`hub.toml`), read again when it changed; the
    /// last good one (or the defaults) when it doesn't parse.
    pub fn config(&self) -> HubConfig {
        let path = self.paths.config();
        let stamp = std::fs::metadata(&path).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
        let mut cache = self.config.lock().unwrap_or_else(|e| e.into_inner());
        if cache.0 != stamp {
            cache.0 = stamp;
            if stamp.is_none() {
                cache.1 = HubConfig::default();
            } else {
                match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| HubConfig::parse(&t)) {
                    Ok(c) => cache.1 = c,
                    Err(e) => eprintln!("hub: {}: {e} (keeping the last settings)", path.display()),
                }
            }
        }
        cache.1.clone()
    }

    /// The document key's public half (hex).
    pub fn public_key(&self) -> String {
        chimera_assets::sign::public_hex(&self.key)
    }

    /// A ticket's claims, or why not (401).
    pub fn check(&self, ticket: Option<&str>) -> R<Claims> {
        let ticket = ticket.ok_or_else(|| refuse(401, "a ticket is needed (Authorization: Bearer <ticket>): sign in to the hub"))?;
        if !ticket.starts_with(chimera_assets::ticket::PREFIX) {
            return Err(refuse(401, "an account's ticket is needed"));
        }
        self.auth.check(ticket, self.now()).map_err(|e| match e {
            None => refuse(401, "this device or account was revoked"),
            Some(why) => refuse(401, format!("the ticket was refused: {why}")),
        })
    }

    /// The signed document: its body and signature (hex).
    pub fn document(&self) -> (Vec<u8>, String) {
        let c = self.config();
        let now = self.now();
        let ticket_keys = std::fs::read_to_string(self.paths.ticket_pub())
            .ok()
            .and_then(|t| chimera_assets::ticket::TicketKeys::parse(&t).ok())
            .map(|k| k.keys.iter().map(|(id, key)| (id.clone(), hex(key.as_bytes()))).collect())
            .unwrap_or_default();
        let ours = chimera_assets::tls::fingerprint_in(&self.paths.tls());
        let doc = Discovery {
            schema: SCHEMA,
            name: crate::identity::shown_name(&c.name),
            issued: now,
            expires: now + DOCUMENT_SECS,
            accounts: AccountsEndpoint { url: "/accounts".into(), ticket_keys },
            servers: Endpoint { url: "/hub/servers".into(), fingerprint: None },
            register: Endpoint { url: "/hub/heartbeat".into(), fingerprint: None },
            leave: Endpoint { url: "/hub/leave".into(), fingerprint: None },
            assets: AssetsEndpoint {
                url: c.assets_url.clone().unwrap_or_else(|| "/".into()),
                fingerprint: c.assets_fingerprint.clone().or(ours),
                key: self.public_key(),
            },
            feedback: Endpoint { url: c.feedback_url.clone().unwrap_or_else(|| "/feedback".into()), fingerprint: None },
            news: c.news.clone(),
            motd: c.motd.clone(),
            client: ClientInfo { min_net: c.min_net, recommended_build: c.recommended_build.clone(), download: c.download.clone() },
            next_fingerprint: c.next_fingerprint.clone(),
        };
        let body = serde_json::to_vec(&doc).unwrap_or_default();
        let signature = chimera_assets::sign::sign(&self.key, &body);
        (body, signature)
    }

    /// Expired listings and revoked accounts' go.
    pub fn sweep(&self) {
        self.sweep_parties();
        let now = self.now();
        let expiry = self.options.expiry;
        let gone: Vec<String> = {
            let revoked = self.auth.revocations();
            let r = revoked.read().unwrap_or_else(|e| e.into_inner());
            let mut map = self.entries();
            let gone: Vec<String> = map
                .iter()
                .filter(|(_, e)| {
                    let by = |m: &HashMap<Id, u64>, id: &Id| m.get(id).is_some_and(|at| e.issued <= *at);
                    now >= e.last + expiry || by(&r.accounts, &e.account) || by(&r.devices, &e.device)
                })
                .map(|(i, _)| i.clone())
                .collect();
            for i in &gone {
                map.remove(i);
            }
            gone
        };
        if !gone.is_empty() {
            self.forget(&gone);
        }
    }

    /// The listings live now (the sweep done first).
    pub fn count(&self) -> usize {
        self.sweep();
        self.entries().len()
    }

    /// `POST /hub/heartbeat`: `claims`' server seen from `peer` (the
    /// address the hub sees; loopback: this machine).
    pub fn heartbeat(&self, claims: &Claims, beat: &Heartbeat, peer: IpAddr) -> R<HeartbeatAnswer> {
        if claims.list == 0 {
            return Err(refuse(403, "this account may not list servers (ask the hub's owner for the `list` right)"));
        }
        let beat = clean(beat)?;
        self.sweep();
        let now = self.now();
        {
            let mut off = self.delisted.lock().unwrap_or_else(|e| e.into_inner());
            off.retain(|_, until| *until > now);
            if let Some(until) = off.get(&beat.instance) {
                return Err(refuse(403, format!("the hub's owner took this server off the list (for another {} min)", (until - now).div_ceil(60))));
            }
        }
        let probe = SocketAddr::new(peer, beat.port);
        let listed_ip = match (peer.is_loopback(), self.config().public_address) {
            (true, Some(a)) => a.parse().unwrap_or(peer),
            _ => peer,
        };
        let address = SocketAddr::new(listed_ip, beat.port);
        let (entry, ask_probe) = {
            let mut map = self.entries();
            let others = map.iter().filter(|(i, e)| e.account == claims.account && **i != beat.instance).count();
            let existing = map.get(&beat.instance).map(|e| (e.account, e.last, e.beat == beat));
            match existing {
                Some((account, _, _)) if account != claims.account => {
                    return Err(refuse(409, "another account lists this server instance"));
                }
                Some((_, last, same)) => {
                    let since = now.saturating_sub(last);
                    if (same && since < self.options.min_interval) || since < 1 {
                        return Err(refuse(429, format!("heartbeats at most every {} s", self.options.min_interval)));
                    }
                    let e = map.get_mut(&beat.instance).expect("listed");
                    if e.probe != probe {
                        // Moved: asked again.
                        e.reachable = Reach::Unknown;
                        e.probed_at = 0;
                    }
                    e.beat = beat.clone();
                    e.address = address;
                    e.probe = probe;
                    e.last = now;
                    e.device = claims.device;
                    e.issued = claims.issued;
                }
                None => {
                    if others >= claims.list as usize {
                        return Err(refuse(
                            429,
                            format!("this account lists {others} server(s) already, its most ({}): stop one first", claims.list),
                        ));
                    }
                    map.insert(
                        beat.instance.clone(),
                        Entry {
                            account: claims.account,
                            device: claims.device,
                            issued: claims.issued,
                            address,
                            probe,
                            beat: beat.clone(),
                            reachable: Reach::Unknown,
                            probed_at: 0,
                            probing: false,
                            first_seen: now,
                            last: now,
                        },
                    );
                }
            }
            let entry = map.get_mut(&beat.instance).expect("listed");
            // The probe: first, then every `probe_every`, never more than
            // once a minute.
            let due = entry.probed_at == 0 || now >= entry.probed_at + self.options.probe_every.max(60);
            let ask = due && !entry.probing;
            if ask {
                entry.probing = true;
            }
            (entry.clone(), ask)
        };
        self.store(&beat.instance, &entry)?;
        if ask_probe && let Some(tx) = self.probes.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let _ = tx.send((beat.instance.clone(), probe));
        }
        Ok(HeartbeatAnswer { address: address.to_string(), reachable: entry.reachable, hint: entry.hint(), every: self.options.every })
    }

    /// `POST /hub/leave`: `claims`' listing of `instance` goes.
    pub fn leave(&self, claims: &Claims, instance: &str) -> R<()> {
        let instance = instance.trim().to_ascii_lowercase();
        let mut map = self.entries();
        match map.get(&instance) {
            Some(e) if e.account == claims.account => {
                map.remove(&instance);
                drop(map);
                self.forget(&[instance]);
                Ok(())
            }
            Some(_) => Err(refuse(403, "another account lists this server")),
            None => Ok(()),
        }
    }

    /// `GET /hub/servers` for `claims`: the live listings, those the hub
    /// couldn't reach only to their own account.
    pub fn list(&self, claims: &Claims) -> ServerList {
        self.sweep();
        let map = self.entries();
        let mut servers: Vec<Listing> = map
            .values()
            .filter(|e| e.reachable != Reach::No || e.account == claims.account)
            .map(|e| e.listing(e.account == claims.account))
            .collect();
        servers.sort_by(|a, b| a.since.cmp(&b.since).then_with(|| a.instance.cmp(&b.instance)));
        servers.truncate(MAX_LISTED);
        ServerList { servers, now: self.now() }
    }

    /// Whether `given` is the services' secret (the owner's calls; the
    /// file the accounts service makes at its first start).
    pub fn is_secret(&self, given: &str) -> bool {
        let Ok(secret) = std::fs::read_to_string(self.paths.secret()) else { return false };
        let (a, b) = (given.trim().as_bytes(), secret.trim().as_bytes());
        !b.is_empty() && a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }

    /// Every live listing for the owner: unreachable ones too, with the
    /// account and where its heartbeats come from.
    pub fn admin_list(&self) -> Vec<AdminListing> {
        self.sweep();
        let map = self.entries();
        let mut out: Vec<AdminListing> = map
            .values()
            .map(|e| AdminListing { account: hex(&e.account), seen: e.probe.to_string(), last: e.last, listing: Listing { hint: e.hint(), ..e.listing(false) } })
            .collect();
        out.sort_by(|a, b| a.listing.since.cmp(&b.listing.since).then_with(|| a.listing.instance.cmp(&b.listing.instance)));
        out
    }

    /// The owner takes `instance` off the list; its heartbeats are refused
    /// for `secs` (none: `DELIST_SECS`).
    pub fn delist(&self, instance: &str, secs: Option<u64>) -> R<Delisted> {
        let instance = clean_id("instance", &instance.trim().to_ascii_lowercase(), b"", 32)?;
        let until = self.now() + secs.unwrap_or(DELIST_SECS).clamp(1, 30 * 86_400);
        let was_listed = self.entries().remove(&instance).is_some();
        if was_listed {
            self.forget(std::slice::from_ref(&instance));
        }
        self.delisted.lock().unwrap_or_else(|e| e.into_inner()).insert(instance, until);
        Ok(Delisted { was_listed, until })
    }

    /// A probe's answer for `instance` (still at `probe`).
    fn probed(&self, instance: &str, probe: SocketAddr, reached: bool) {
        let now = self.now();
        let entry = {
            let mut map = self.entries();
            let Some(e) = map.get_mut(instance) else { return };
            e.probing = false;
            if e.probe != probe {
                return;
            }
            e.reachable = if reached { Reach::Yes } else { Reach::No };
            e.probed_at = now;
            e.clone()
        };
        if !reached {
            eprintln!("hub: {instance} at {} didn't answer the probe: hidden from others", entry.address);
        }
        let _ = self.store(instance, &entry);
    }
}

/// The probe's thread: one query at a time from one UDP socket.
fn probe_loop(hub: Weak<Hub>, rx: mpsc::Receiver<(String, SocketAddr)>) {
    let socket = |v6: bool| UdpSocket::bind(if v6 { "[::]:0" } else { "0.0.0.0:0" });
    let (mut v4, mut v6) = (socket(false).ok(), None);
    while let Ok((instance, addr)) = rx.recv() {
        let Some(h) = hub.upgrade() else { return };
        let wait = h.options.probe_wait;
        drop(h);
        let sock = if addr.is_ipv6() {
            if v6.is_none() {
                v6 = socket(true).ok();
            }
            v6.as_ref()
        } else {
            if v4.is_none() {
                v4 = socket(false).ok();
            }
            v4.as_ref()
        };
        let reached = sock.and_then(|s| chimera_formats::a2s::query_blocking(s, addr, wait)).is_some_and(|i| i.is_ours());
        let Some(h) = hub.upgrade() else { return };
        h.probed(&instance, addr, reached);
    }
}

/// `public_address` of a hub.toml, for a status line.
pub fn describe_config(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(t) => match HubConfig::parse(&t) {
            Ok(c) => format!("{} (public address {})", path.display(), c.public_address.as_deref().unwrap_or("unset")),
            Err(e) => format!("{}: {e}", path.display()),
        },
        Err(_) => format!("{} (none: the defaults)", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beat() -> Heartbeat {
        Heartbeat {
            instance: "00112233445566AA".into(),
            port: 27031,
            lan: vec!["192.168.1.20:27031".into(), "203.0.113.9:27031".into(), "junk".into()],
            name: "  Friday ladder  ".into(),
            game: "ultimate_gun_game".into(),
            minigame: Some("ultimate_gun_game:auto".into()),
            map: "DE_dust2".into(),
            players: 3,
            max: 8,
            bots: 1,
            password: false,
            dedicated: true,
            net: "0.1.0/net54".into(),
        }
    }

    #[test]
    fn heartbeats_are_cleaned() {
        let b = clean(&beat()).unwrap();
        assert_eq!(b.instance, "00112233445566aa");
        assert_eq!(b.name, "Friday ladder");
        assert_eq!(b.map, "de_dust2");
        assert_eq!(b.lan, vec!["192.168.1.20:27031".to_string()], "private addresses only");
        let long = Heartbeat { name: "x".repeat(100), ..beat() };
        assert_eq!(clean(&long).unwrap().name.len(), 64);
        for bad in [
            Heartbeat { name: "a\u{7}b".into(), ..beat() },
            Heartbeat { game: "Ultimate Gun Game!".into(), ..beat() },
            Heartbeat { instance: "xyz".into(), ..beat() },
            Heartbeat { net: "".into(), ..beat() },
            Heartbeat { port: 0, ..beat() },
        ] {
            assert_eq!(clean(&bad).unwrap_err().status, 400, "{bad:?}");
        }
    }

    #[test]
    fn the_config_parses() {
        let c = HubConfig::parse("name = \"Test\"\nmotd = \"hi\"\npublic_address = \"203.0.113.5\"\nmin_net = 54\n[[news]]\nid = \"a\"\ntitle = \"t\"\n").unwrap();
        assert_eq!((c.name.as_str(), c.min_net, c.news.len()), ("Test", 54, 1));
        assert!(HubConfig::parse("public_address = \"hub.example\"").is_err());
    }
}

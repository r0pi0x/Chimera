//! The asset service's HTTP side (docs/plans/active/asset-service.md,
//! section 5, M2): a store served to accounts with tokens. Plain HTTP/1.1
//! on a local address (`chimera_asset_server`, default 127.0.0.1:8731);
//! HTTPS is the job of what stands in front (`tailscale serve`, a reverse
//! proxy), so tokens and files never cross a network in the clear.
//!
//! - `GET /key`: the server's public key (hex), unauthenticated: what
//!   manifests are signed with.
//! - Everything else wants `Authorization: Bearer <token>`: none, an
//!   unknown or a revoked token is 401 (`accounts`). With
//!   `Service::with_tickets` (a hub's) a token is an account ticket alone
//!   (`ticket`, docs/plans/active/hub.md 2.4), refused within one
//!   revocation poll of its device's or account's revocation; a service
//!   with no hub (`Service::new`) takes its own tokens (`accounts::Internal`).
//! - `GET /whoami`: `<kind> <name> <token>` and the games it may fetch.
//! - `GET /games`: `<game> <build> <manifest id>` per line, oldest build
//!   first, for the games the account may fetch.
//! - `GET /m/<game>/<build>`: the manifest's exact bytes, signed
//!   (`X-Chimera-Signature`, `X-Chimera-Manifest-Id`); 403 for a game the
//!   account may not fetch.
//! - `GET /r/<id>`: the record as stored (zstd), immutable; 403 unless a
//!   manifest of a game the account may fetch refers to it, 404 when no
//!   manifest does.
//! - `POST /batch`: ids, one per line (at most `BATCH_MAX`); the answer is
//!   one frame per id in order, `<id> <status> <length>\n` then `length`
//!   bytes of the stored record (status 200; 403 and 404 with length 0).
//! - `POST /feedback`: a feedback report (a zip, `X-Chimera-Report-Id`),
//!   stored by `feedback::inbox` under `<store>/server/feedback/` (201,
//!   200 already stored, 202, 400, 413, 429 with `Retry-After`).
//! - `/feedback/admin/...`: the triage page and its JSON
//!   (`feedback::page`), only to this machine's own connections on the
//!   plain listener (not through the TLS front; a local `Host`), no token;
//!   404 to everyone else.
//! - `GET /health` (this machine's own connections only, as the page):
//!   `ok <version> uptime=<s> games=<n> revocations=<age>`, for
//!   `chimera_accounts status`.
//! - The other hub services' paths (`/accounts/`, `/hub`, `/internal/`,
//!   `/admin/`) are 421: a request the front misrouted is loud.
//!
//! Manifests are read again when the manifest folder changes (checked at
//! most once a second), so a publish needs no restart; one without a
//! signature is signed with the server's key when loaded. Every request
//! is one line of `<store>/server/access.log`: time, account, method,
//! path, status, records sent, bytes sent, the client's address, and
//! ` on-demand` for a client's on-demand fetch (`?on-demand` on its path:
//! `remote::ON_DEMAND`; a file a match read outside the sets it fetched).
//! The line is written once the
//! answer has been sent (a batch's bytes are counted while sending), so a
//! client may have its answer before the line is in the log.
//!
//! `Service::with_latency` (`chimera_asset_server --latency-ms`) delays
//! every answer, to measure clients as over a slower link (Tailscale, a
//! VPS) from this machine.
//!
//! Limits (`Limits`, section 8; the service may face the internet behind
//! its TLS front, `tls::front`): each account may make `rate` requests a
//! second on average (bursts of `burst`) and fetch `daily_bytes` of records
//! a day; an address that sends `bad_tokens` unknown or revoked tokens in a
//! minute is refused for `ban`. Past a limit the answer is 429 with the
//! reason. Behind a front, each request's address is the client's (the
//! hub's front: `X-Chimera-Peer`, trusted from loopback connections only,
//! `tls::forwarded_peer`; the in-process one: `tls::Peers`), and the log
//! names it (`ip=`); a relayed request never reaches the feedback page.
//!
//! Known shortcuts: a revoke stops a token's next request, not a batch
//! already being sent; a batch answer is sent with chunked encoding, so a client
//! learns its length only at the end; no range requests; the manifest
//! check's once-a-second limit and the pacing (`pace`) run on the real
//! clock, so tests of them sleep (the allowlisted `thread::sleep`s in
//! `chimera/assets/tests/it/service.rs` and `chimera/net/tests/it/assets.rs`).

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Read, Write},
    net::IpAddr,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::SigningKey;
use tiny_http::{Header, Method, Request, Response, ResponseBox};

use crate::{
    Manifest, Store,
    accounts::{Accounts, Authenticator, Internal, Principal, Verdict},
    sign,
    store::{is_id, is_name, sha256_hex, sort_builds},
};

/// The port `chimera_asset_server` listens on by default (TCP, localhost).
pub const DEFAULT_PORT: u16 = 8731;
/// Most ids one batch may ask for.
pub const BATCH_MAX: usize = 20_000;

/// The server's own folder in its store (accounts, key, log).
pub fn server_dir(store: &Store) -> PathBuf {
    store.root.join("server")
}

/// The server's TLS certificate's folder (`tls::Identity`).
pub fn tls_dir(store: &Store) -> PathBuf {
    server_dir(store).join("tls")
}

/// The feedback reports' folder (`feedback::inbox`).
pub fn feedback_dir(store: &Store) -> PathBuf {
    server_dir(store).join("feedback")
}

/// The server's signing key file.
pub fn key_path(store: &Store) -> PathBuf {
    server_dir(store).join("signing.key")
}

/// The default store of a server on this machine:
/// `<data_local>/chimera/asset-server` (`~/.local/share/...`,
/// `%LOCALAPPDATA%\...`). `chimera_pack publish` and `user` use it too, so
/// on one box they write straight into what the server serves.
pub fn default_store() -> Option<Store> {
    Some(Store::new(dirs::data_local_dir()?.join("chimera").join("asset-server")))
}

/// What the service lets one account or address do (section 8).
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Requests a second an account may make, on average.
    pub rate: f64,
    /// Requests an account may make at once before `rate` holds it.
    pub burst: f64,
    /// Bytes of records an account may fetch a day (UTC); 0: no cap.
    pub daily_bytes: u64,
    /// Requests with an unknown or revoked token an address may make in a
    /// minute before it is refused for `ban`.
    pub bad_tokens: u32,
    pub ban: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            rate: 100.0,
            burst: 1000.0,
            daily_bytes: 20_000_000_000,
            bad_tokens: 30,
            ban: Duration::from_secs(600),
        }
    }
}

/// An account's use so far.
struct Meter {
    allowance: f64,
    last: Instant,
    day: u64,
    bytes: u64,
}

/// An address's bad tokens.
#[derive(Default)]
struct Strikes {
    minute: u64,
    count: u32,
    banned_until: Option<Instant>,
}

#[derive(Default)]
struct Guard {
    accounts: HashMap<String, Meter>,
    addresses: HashMap<IpAddr, Strikes>,
}

/// One build's manifest as served.
struct Served {
    text: Arc<Vec<u8>>,
    signature: String,
    id: String,
}

#[derive(Default)]
struct Catalog {
    /// Fingerprint of the manifest folder when read.
    stamp: Vec<(PathBuf, Option<SystemTime>, u64)>,
    checked: Option<Instant>,
    games: BTreeMap<String, BTreeMap<String, Served>>,
    /// Record id -> the games whose manifests refer to it.
    owners: HashMap<String, BTreeSet<String>>,
}

/// The service: a store, its key, who may ask.
pub struct Service {
    store: Store,
    key: SigningKey,
    auth: Vec<Box<dyn Authenticator>>,
    catalog: Mutex<Catalog>,
    log: Mutex<Option<File>>,
    /// Added before each answer (measuring a slow link).
    latency: Duration,
    limits: Limits,
    guard: Mutex<Guard>,
    /// Behind the TLS front: whose each relayed connection is.
    peers: Option<crate::tls::Peers>,
    /// Feedback reports (`POST /feedback`, the page).
    feedback: crate::feedback::inbox::Inbox,
    /// Host names besides the loopback ones the triage page answers to
    /// (lowercase; from a loopback connection only).
    admin_hosts: Vec<String>,
    /// This machine's host name, read at start.
    own_host: Option<String>,
    started: Instant,
}

/// This machine's host name from the OS (lowercase), if it has one.
fn own_host_name() -> Option<String> {
    #[cfg(unix)]
    let name = {
        let mut buf = [0u8; 256];
        // SAFETY: `buf` is valid for 256 bytes; gethostname writes at most that.
        let ok = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } == 0;
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        ok.then(|| String::from_utf8_lossy(&buf[..end]).into_owned())
    };
    #[cfg(not(unix))]
    let name = std::env::var("COMPUTERNAME").ok();
    name.map(|n| n.trim().trim_end_matches('.').to_ascii_lowercase()).filter(|n| !n.is_empty())
}

/// The triage page's rule: from loopback, not relayed by the TLS front, to
/// a `Host` whose name is a loopback one, this machine's own (`own`, and
/// `<own>.local`), a Tailscale MagicDNS name (`*.ts.net`: resolves only
/// inside a tailnet, so a rebinding page can't produce it) or one of
/// `extra`. Names are matched exactly (or by the `.ts.net` suffix), never
/// "any Host" (DNS rebinding).
fn admin_allowed(loopback: bool, relayed: bool, host: &str, own: Option<&str>, extra: &[String]) -> bool {
    let host = host.to_ascii_lowercase();
    let host = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or("").to_string(),
        None => host.split(':').next().unwrap_or("").to_string(),
    };
    let host = host.trim_end_matches('.');
    if !loopback || relayed || host.is_empty() {
        return false;
    }
    matches!(host, "localhost" | "127.0.0.1" | "::1")
        || own.is_some_and(|o| host == o || host.strip_suffix(".local") == Some(o))
        || host.strip_suffix(".ts.net").is_some_and(|label| !label.is_empty())
        || extra.iter().any(|h| h == host)
}

#[cfg(test)]
mod admin_tests {
    use super::admin_allowed;

    fn ok(relayed: bool, host: &str) -> bool {
        admin_allowed(true, relayed, host, Some("mybox"), &["proxy.example".to_string()])
    }

    #[test]
    fn the_admin_rule_takes_known_names_from_loopback_only() {
        assert!(ok(false, "localhost:8731") && ok(false, "[::1]:8731") && ok(false, "127.0.0.1"));
        assert!(ok(false, "mybox") && ok(false, "MyBox.local:80") && ok(false, "mybox."));
        assert!(ok(false, "Box.Tail1234.TS.net") && ok(false, "box.tail1234.ts.net:443") && ok(false, "box.tail1234.ts.net."));
        assert!(ok(false, "Proxy.Example"));
        // Relayed by the front, or not from loopback: refused.
        assert!(!ok(true, "box.tail1234.ts.net") && !ok(true, "localhost"));
        assert!(!admin_allowed(false, false, "box.tail1234.ts.net", None, &[]));
        // Other names, suffix and prefix tricks: refused.
        for bad in ["evil.example", "ts.net.evil.example", "evilts.net", "ts.net", ".ts.net", "mybox.evil.example", "xmybox", "evil.mybox.local", ""] {
            assert!(!ok(false, bad), "{bad}");
        }
        assert!(!admin_allowed(true, false, "mybox", None, &[]));
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("ascii header")
}

fn text(status: u16, body: impl Into<String>) -> ResponseBox {
    Response::from_string(body.into())
        .with_status_code(status)
        .with_header(header("Content-Type", "text/plain; charset=utf-8"))
        .boxed()
}

/// What one request did, for the log.
struct Outcome {
    response: ResponseBox,
    status: u16,
    records: u64,
    /// Bytes of records sent (counted while sending a batch).
    bytes: Arc<AtomicU64>,
}

impl Outcome {
    fn new(status: u16, response: ResponseBox) -> Self {
        Self { response, status, records: 0, bytes: Arc::new(AtomicU64::new(0)) }
    }

    fn text(status: u16, body: impl Into<String>) -> Self {
        Self::new(status, text(status, body))
    }
}

impl Service {
    /// The service of `store` with no hub: its own tokens
    /// (`<store>/server/accounts.toml`, `chimera_pack token add`) only.
    pub fn new(store: Store) -> io::Result<Self> {
        let accounts = Accounts::path(&store.root);
        Self::with_authenticators(store, vec![Box::new(Internal::new(accounts))])
    }

    /// The service of `store` taking the accounts service's tickets
    /// (`TicketAuth`, keys from `<store>/server/accounts/ticket.pub`, the
    /// revocation feed from `accounts_url` every `every`): a hub's, which
    /// takes its accounts' tickets and nothing else.
    pub fn with_tickets(store: Store, accounts_url: &str, every: Duration) -> io::Result<Self> {
        let tickets = crate::accounts::TicketAuth::new(crate::accounts::ticket_keys_path(&store.root)).with_feed(
            accounts_url.to_string(),
            crate::accounts::service_secret_path(&store.root),
            every,
        );
        Self::with_authenticators(store, vec![Box::new(tickets)])
    }

    /// With these account kinds, asked in order (Steam's goes here later).
    pub fn with_authenticators(store: Store, auth: Vec<Box<dyn Authenticator>>) -> io::Result<Self> {
        fs::create_dir_all(server_dir(&store))?;
        let key = sign::load_or_create_key(&key_path(&store))?;
        let log = OpenOptions::new().create(true).append(true).open(server_dir(&store).join("access.log"))?;
        // Addresses and account names: owner-only.
        sign::restrict(&server_dir(&store).join("access.log"));
        Ok(Self {
            key,
            auth,
            catalog: Mutex::new(Catalog::default()),
            log: Mutex::new(Some(log)),
            latency: Duration::ZERO,
            limits: Limits::default(),
            guard: Mutex::default(),
            peers: None,
            feedback: crate::feedback::inbox::Inbox::open(feedback_dir(&store)),
            admin_hosts: Vec::new(),
            own_host: own_host_name(),
            store,
            started: Instant::now(),
        })
    }

    /// Every answer delayed by `latency`, as over a slower link.
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    /// With these limits.
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Feedback reports with these limits and this command per report
    /// (`feedback::inbox`).
    pub fn with_feedback(mut self, limits: crate::feedback::inbox::FeedbackLimits, on_report: Option<String>) -> Self {
        let inbox = crate::feedback::inbox::Inbox::open(feedback_dir(&self.store));
        self.feedback = inbox.with_limits(limits).with_on_report(on_report);
        self
    }

    /// Host names (besides `localhost` and the loopback addresses) the
    /// triage page answers to, e.g. a private network's proxy name. Exact,
    /// case-insensitive; the other conditions of `local_admin` stay.
    pub fn with_admin_hosts(mut self, hosts: Vec<String>) -> Self {
        self.admin_hosts = hosts.into_iter().map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty()).collect();
        self
    }

    /// Behind a TLS front: `peers` says whose each relayed connection is.
    pub fn with_peers(mut self, peers: crate::tls::Peers) -> Self {
        self.peers = Some(peers);
        self
    }

    /// The client's address of `request`: behind the hub's front, the one
    /// its `X-Chimera-Peer` names (trusted from loopback only); behind the
    /// in-process TLS front, the one it relayed; else the connection's own.
    fn address(&self, request: &Request) -> Option<IpAddr> {
        let at = *request.remote_addr()?;
        if let Some(ip) = self.forwarded(request) {
            return Some(ip);
        }
        let relayed = self
            .peers
            .as_ref()
            .and_then(|p| p.lock().unwrap_or_else(|e| e.into_inner()).get(&at).copied());
        Some(relayed.unwrap_or(at.ip()))
    }

    /// The client `X-Chimera-Peer` names on a loopback connection (the hub's
    /// front relayed the request).
    fn forwarded(&self, request: &Request) -> Option<IpAddr> {
        let at = request.remote_addr()?;
        let value = request
            .headers()
            .iter()
            .find(|h| h.field.equiv(crate::tls::PEER_HEADER))
            .map(|h| h.value.as_str());
        crate::tls::forwarded_peer(at, value)
    }

    /// Whether `request` came over a connection a front relayed (the
    /// in-process TLS front, or any with `X-Chimera-Peer`).
    fn relayed(&self, request: &Request) -> bool {
        let Some(at) = request.remote_addr() else { return true };
        request.headers().iter().any(|h| h.field.equiv(crate::tls::PEER_HEADER))
            || self
                .peers
                .as_ref()
                .is_some_and(|p| p.lock().unwrap_or_else(|e| e.into_inner()).contains_key(at))
    }

    /// Whether `request` may see the feedback page: from this machine, not
    /// relayed by the TLS front, to a local `Host` (no DNS rebinding).
    fn local_admin(&self, request: &Request) -> bool {
        let Some(at) = request.remote_addr() else { return false };
        let host = request.headers().iter().find(|h| h.field.equiv("Host")).map(|h| h.value.as_str()).unwrap_or("");
        admin_allowed(at.ip().is_loopback(), self.relayed(request), host, self.own_host.as_deref(), &self.admin_hosts)
    }

    /// `/feedback/admin...` (`feedback::page`).
    fn admin(&self, request: &mut Request, method: &Method, parts: &[&str]) -> Outcome {
        if parts.is_empty() {
            return Outcome::new(
                301,
                Response::empty(301).with_header(header("Location", "/feedback/admin/")).boxed(),
            );
        }
        let mut body = Vec::new();
        if *method == Method::Post {
            let _ = request.as_reader().take(64 * 1024).read_to_end(&mut body);
        }
        let admin_header = request
            .headers()
            .iter()
            .any(|h| h.field.equiv("X-Chimera-Admin") && h.value.as_str().trim() == "1");
        let r = crate::feedback::page::handle(&self.feedback, *method == Method::Post, parts, &body, admin_header);
        Outcome::new(
            r.status,
            Response::from_data(r.body)
                .with_status_code(r.status)
                .with_header(header("Content-Type", r.content_type))
                .with_header(header("X-Content-Type-Options", "nosniff"))
                .with_header(header("Cache-Control", "no-store"))
                .with_header(header(
                    "Content-Security-Policy",
                    "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src 'self'; connect-src 'self'; frame-ancestors 'none'",
                ))
                .boxed(),
        )
    }

    /// `POST /feedback`: a report from `who`.
    fn feedback(&self, request: &mut Request, who: &Principal, ip: Option<IpAddr>) -> Outcome {
        let max = self.feedback.limits().max_body;
        if request.body_length().is_some_and(|n| n > max) {
            return Outcome::text(413, format!("a report is at most {max} bytes\n"));
        }
        let mut body = Vec::new();
        if request.as_reader().take(max as u64 + 1).read_to_end(&mut body).is_err() {
            return Outcome::text(400, "the report didn't arrive whole\n");
        }
        let id = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("X-Chimera-Report-Id"))
            .map(|h| h.value.as_str().to_string());
        let answer = self.feedback.receive(&who.label(), who.owner, ip, id.as_deref(), &body);
        let mut response = text(answer.status, answer.body);
        if let Some(wait) = answer.retry_after {
            response = response.with_header(header("Retry-After", &wait.to_string()));
        }
        Outcome::new(answer.status, response)
    }

    /// Whether `ip` is refused for bad tokens now.
    fn banned(&self, ip: Option<IpAddr>) -> bool {
        let Some(ip) = ip else { return false };
        let guard = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .addresses
            .get(&ip)
            .and_then(|s| s.banned_until)
            .is_some_and(|t| Instant::now() < t)
    }

    /// `ip` sent a bad token.
    fn strike(&self, ip: Option<IpAddr>) {
        let Some(ip) = ip else { return };
        let mut guard = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        let s = guard.addresses.entry(ip).or_default();
        let minute = now_secs() / 60;
        if s.minute != minute {
            s.minute = minute;
            s.count = 0;
        }
        s.count += 1;
        if s.count >= self.limits.bad_tokens {
            s.banned_until = Some(Instant::now() + self.limits.ban);
        }
    }

    /// Whether `who` may make a request now (and one more is counted): the
    /// refusal if not.
    fn throttle(&self, who: &Principal) -> Option<Outcome> {
        let mut guard = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let day = now_secs() / 86_400;
        let m = guard.accounts.entry(who.label()).or_insert(Meter {
            allowance: self.limits.burst,
            last: now,
            day,
            bytes: 0,
        });
        if m.day != day {
            m.day = day;
            m.bytes = 0;
        }
        m.allowance =
            (m.allowance + now.duration_since(m.last).as_secs_f64() * self.limits.rate).min(self.limits.burst);
        m.last = now;
        if self.limits.daily_bytes > 0 && m.bytes >= self.limits.daily_bytes {
            return Some(Outcome::text(
                429,
                format!(
                    "this account fetched its {} for today; try again tomorrow (UTC)\n",
                    crate::progress::mb(m.bytes)
                ),
            ));
        }
        if m.allowance < 1.0 {
            return Some(Outcome::text(429, "too many requests: slow down\n"));
        }
        m.allowance -= 1.0;
        None
    }

    /// `who` was sent `bytes` of records.
    fn count(&self, who: &Principal, bytes: u64) {
        let mut guard = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = guard.accounts.get_mut(&who.label()) {
            m.bytes += bytes;
        }
    }

    /// `GET /health` (this machine only): `ok <version> uptime=<s>
    /// games=<n>` and each account kind's word (the revocation feed's age).
    pub fn health(&self) -> String {
        let mut line = format!(
            "ok {} uptime={}s games={}",
            env!("CARGO_PKG_VERSION"),
            self.started.elapsed().as_secs(),
            self.catalog().games.len()
        );
        for word in self.auth.iter().filter_map(|a| a.status()) {
            line.push(' ');
            line.push_str(&word);
        }
        line.push('\n');
        line
    }

    pub fn public_key(&self) -> String {
        sign::public_hex(&self.key)
    }

    fn fingerprint(&self) -> Vec<(PathBuf, Option<SystemTime>, u64)> {
        let mut out = Vec::new();
        for game in self.store.games() {
            for e in fs::read_dir(self.store.manifest_dir(&game)).into_iter().flatten().flatten() {
                let meta = e.metadata().ok();
                out.push((e.path(), meta.as_ref().and_then(|m| m.modified().ok()), meta.map_or(0, |m| m.len())));
            }
        }
        out.sort();
        out
    }

    /// The catalog, read again if the manifests changed.
    fn catalog(&self) -> std::sync::MutexGuard<'_, Catalog> {
        let mut c = self.catalog.lock().unwrap_or_else(|e| e.into_inner());
        if c.checked.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
            return c;
        }
        c.checked = Some(Instant::now());
        let stamp = self.fingerprint();
        if stamp == c.stamp && !c.stamp.is_empty() {
            return c;
        }
        let mut games: BTreeMap<String, BTreeMap<String, Served>> = BTreeMap::new();
        let mut owners: HashMap<String, BTreeSet<String>> = HashMap::new();
        for game in self.store.games().into_iter().filter(|g| is_name(g)) {
            for build in self.store.builds(&game).into_iter().filter(|b| is_name(b)) {
                let path = self.store.manifest_path(&game, &build);
                let Ok(bytes) = fs::read(&path) else { continue };
                let manifest = match std::str::from_utf8(&bytes).map_err(|e| e.to_string()).and_then(Manifest::parse) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("{}: {e} (not served)", path.display());
                        continue;
                    }
                };
                let sig_path = sign::sig_path(&path);
                let signature = match fs::read_to_string(&sig_path) {
                    Ok(s) if sign::verify(&self.public_key(), &bytes, s.trim()).is_ok() => s.trim().to_string(),
                    _ => match sign::sign_manifest_file(&self.key, &path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("{}: can't sign: {e}", path.display());
                            continue;
                        }
                    },
                };
                for f in manifest.files.values() {
                    owners.entry(f.id.clone()).or_default().insert(game.clone());
                }
                let id = sha256_hex(&bytes);
                games.entry(game.clone()).or_default().insert(build, Served { text: Arc::new(bytes), signature, id });
            }
        }
        // Signing may have written `.sig` files: take the stamp after.
        c.stamp = self.fingerprint();
        c.games = games;
        c.owners = owners;
        c
    }

    fn who(&self, request: &Request) -> Result<Principal, Outcome> {
        let token = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Authorization"))
            .and_then(|h| h.value.as_str().strip_prefix("Bearer "))
            .map(str::trim);
        let Some(token) = token.filter(|t| !t.is_empty()) else {
            return Err(Outcome::text(401, "a token is needed (Authorization: Bearer <token>)\n"));
        };
        for auth in &self.auth {
            match auth.authenticate(token) {
                Some(Verdict::Granted(p)) => return Ok(p),
                Some(Verdict::Revoked) => return Err(Outcome::text(401, "this token was revoked\n")),
                Some(Verdict::Refused(why)) => return Err(Outcome::text(401, format!("{why}\n"))),
                None => {}
            }
        }
        Err(Outcome::text(401, "unknown token\n"))
    }

    /// Whether `who` may have record `id`: 200, 403 or 404.
    fn record_status(catalog: &Catalog, who: &Principal, id: &str) -> u16 {
        match catalog.owners.get(id) {
            Some(games) if games.iter().any(|g| who.games.may(g)) => 200,
            Some(_) => 403,
            None => 404,
        }
    }

    /// Answer one request (and log it).
    pub fn handle(&self, mut request: Request) {
        let path = request.url().split('?').next().unwrap_or("").to_string();
        let on_demand = request.url().split_once('?').is_some_and(|(_, q)| q.split('&').any(|p| p == "on-demand"));
        let method = request.method().clone();
        let ip = self.address(&request);
        let (who, outcome) = match self.route(&mut request, &method, &path, ip) {
            Ok((who, outcome)) => (Some(who), outcome),
            Err(outcome) => (None, outcome),
        };
        let Outcome { response, status, records, bytes } = outcome;
        let length = response.data_length().map(|n| n as u64);
        if !self.latency.is_zero() {
            std::thread::sleep(self.latency);
        }
        let _ = request.respond(response);
        let sent = length
            .filter(|_| status == 200 && records == 1)
            .unwrap_or_else(|| bytes.load(Ordering::Relaxed));
        if let Some(w) = &who {
            self.count(w, sent);
        }
        let line = format!(
            "{} {} {method} {path} {status} records={records} bytes={sent} ip={}{}\n",
            now_secs(),
            who.map_or("-".into(), |w| w.label()),
            ip.map_or("-".into(), |i| i.to_string()),
            if on_demand { " on-demand" } else { "" },
        );
        if let Some(log) = self.log.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = log.write_all(line.as_bytes());
        }
    }

    fn route(
        &self,
        request: &mut Request,
        method: &Method,
        path: &str,
        ip: Option<IpAddr>,
    ) -> Result<(Principal, Outcome), Outcome> {
        if path == "/key" {
            return Err(Outcome::text(200, format!("{}\n", self.public_key())));
        }
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        if *method == Method::Get && parts.as_slice() == ["health"] {
            let local = request.remote_addr().is_some_and(|a| a.ip().is_loopback()) && !self.relayed(request);
            if !local {
                return Err(Outcome::text(404, "no such endpoint\n"));
            }
            return Err(Outcome::text(200, self.health()));
        }
        // Another hub service's paths (docs/plans/active/hub.md 2.3): loud,
        // never answered as if they were ours.
        if matches!(parts.first(), Some(&("accounts" | "hub" | "internal" | "admin"))) {
            return Err(Outcome::text(421, "not the asset service's path\n"));
        }
        if let ["feedback", "admin", rest @ ..] = parts.as_slice() {
            if !self.local_admin(request) {
                return Err(Outcome::text(404, "no such endpoint\n"));
            }
            return Err(self.admin(request, method, rest));
        }
        let known = matches!(
            (method, parts.as_slice()),
            (Method::Get, ["whoami" | "games"])
                | (Method::Get, ["m", _, _])
                | (Method::Get, ["r", _])
                | (Method::Post, ["batch" | "feedback"])
        );
        if !known {
            return Err(Outcome::text(404, "no such endpoint\n"));
        }
        if self.banned(ip) {
            return Err(Outcome::text(
                429,
                "too many bad tokens from this address: wait a few minutes\n",
            ));
        }
        let who = self.who(request).inspect_err(|o| {
            if o.status == 401 {
                self.strike(ip);
            }
        })?;
        if let Some(refused) = self.throttle(&who) {
            return Ok((who, refused));
        }
        let outcome = match parts.as_slice() {
            ["whoami"] => {
                let games = match &who.games {
                    crate::accounts::Games::All => "*".to_string(),
                    crate::accounts::Games::Only(g) => g.iter().cloned().collect::<Vec<_>>().join(","),
                };
                Outcome::text(200, format!("{} {} {}\ngames {games}\n", who.kind, who.name, who.token))
            }
            ["games"] => {
                let catalog = self.catalog();
                let mut out = String::new();
                for (game, builds) in catalog.games.iter().filter(|(g, _)| who.games.may(g)) {
                    let mut names: Vec<String> = builds.keys().cloned().collect();
                    sort_builds(&mut names);
                    for b in names {
                        out.push_str(&format!("{game} {b} {}\n", builds[&b].id));
                    }
                }
                Outcome::text(200, out)
            }
            ["m", game, build] => {
                if !who.games.may(game) {
                    return Ok((who, Outcome::text(403, format!("this account may not fetch {game}\n"))));
                }
                let catalog = self.catalog();
                match catalog.games.get(*game).and_then(|b| b.get(*build)) {
                    Some(s) => Outcome::new(
                        200,
                        Response::from_data(s.text.as_ref().clone())
                            .with_header(header("Content-Type", "text/plain; charset=utf-8"))
                            .with_header(header("X-Chimera-Signature", &s.signature))
                            .with_header(header("X-Chimera-Manifest-Id", &s.id))
                            .boxed(),
                    ),
                    None => Outcome::text(404, format!("no manifest {game} {build}\n")),
                }
            }
            ["r", id] => {
                if !is_id(id) {
                    return Ok((who, Outcome::text(400, "not a record id\n")));
                }
                let status = Self::record_status(&self.catalog(), &who, id);
                if status != 200 {
                    return Ok((
                        who,
                        Outcome::text(
                            status,
                            if status == 403 { "not a record of your games\n" } else { "no such record\n" },
                        ),
                    ));
                }
                match File::open(self.store.record_path(id)) {
                    Ok(file) => {
                        let mut o = Outcome::new(
                            200,
                            Response::from_file(file)
                                .with_header(header("Content-Type", "application/zstd"))
                                .with_header(header("Cache-Control", "private, max-age=31536000, immutable"))
                                .boxed(),
                        );
                        o.records = 1;
                        o
                    }
                    Err(_) => Outcome::text(404, "record missing from the store\n"),
                }
            }
            ["batch"] => {
                let mut body = String::new();
                if request.as_reader().take(BATCH_MAX as u64 * 66 + 1024).read_to_string(&mut body).is_err() {
                    return Ok((who, Outcome::text(400, "a batch is ids, one per line\n")));
                }
                let ids: Vec<&str> = body.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
                if ids.len() > BATCH_MAX || !ids.iter().all(|id| is_id(id)) {
                    return Ok((
                        who,
                        Outcome::text(400, format!("a batch is at most {BATCH_MAX} record ids, one per line\n")),
                    ));
                }
                let catalog = self.catalog();
                let items: Vec<(String, u16)> =
                    ids.iter().map(|id| (id.to_string(), Self::record_status(&catalog, &who, id))).collect();
                drop(catalog);
                let records = items.iter().filter(|i| i.1 == 200).count() as u64;
                let bytes = Arc::new(AtomicU64::new(0));
                let reader = BatchReader {
                    store: self.store.clone(),
                    items: items.into_iter(),
                    current: Cursor::new(Vec::new()),
                    bytes: bytes.clone(),
                };
                let response = Response::new(
                    200.into(),
                    vec![header("Content-Type", "application/octet-stream")],
                    reader,
                    None,
                    None,
                )
                .boxed();
                Outcome { response, status: 200, records, bytes }
            }
            ["feedback"] => self.feedback(request, &who, ip),
            _ => Outcome::text(404, "no such endpoint\n"),
        };
        Ok((who, outcome))
    }
}

/// A batch answer, read record by record while it is sent.
struct BatchReader {
    store: Store,
    items: std::vec::IntoIter<(String, u16)>,
    current: Cursor<Vec<u8>>,
    bytes: Arc<AtomicU64>,
}

impl Read for BatchReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let n = self.current.read(buf)?;
            if n > 0 || buf.is_empty() {
                return Ok(n);
            }
            let Some((id, mut status)) = self.items.next() else {
                return Ok(0);
            };
            let data = if status == 200 {
                fs::read(self.store.record_path(&id)).unwrap_or_else(|_| {
                    status = 404;
                    Vec::new()
                })
            } else {
                Vec::new()
            };
            self.bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
            let mut frame = format!("{id} {status} {}\n", data.len()).into_bytes();
            frame.extend_from_slice(&data);
            self.current = Cursor::new(frame);
        }
    }
}

/// A running server; stopped when dropped.
pub struct Running {
    pub addr: SocketAddr,
    pub service: Arc<Service>,
    server: Arc<tiny_http::Server>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl Running {
    /// `http://<addr>`.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Serve until the process ends.
    pub fn wait(mut self) {
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for _ in &self.threads {
            self.server.unblock();
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

/// Serve `service` on `listen` (`127.0.0.1:0` picks a free port) with
/// `threads` workers.
pub fn serve(service: Service, listen: &str, threads: usize) -> io::Result<Running> {
    let listener =
        std::net::TcpListener::bind(listen).map_err(|e| io::Error::new(e.kind(), format!("{listen}: {e}")))?;
    no_delay(&listener);
    let server = Arc::new(
        tiny_http::Server::from_listener(listener, None).map_err(|e| io::Error::other(format!("{listen}: {e}")))?,
    );
    let addr = server.server_addr().to_ip().ok_or_else(|| io::Error::other("not an IP listener"))?;
    let service = Arc::new(service);
    // Read the manifests once before the first request.
    drop(service.catalog());
    let stop = Arc::new(AtomicBool::new(false));
    let threads = (0..threads.max(1))
        .map(|_| {
            let (server, service, stop) = (server.clone(), service.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match server.recv() {
                        Ok(request) => service.handle(request),
                        // Unblocked to stop (the loop's test), or a failed accept.
                        Err(_) => continue,
                    }
                }
            })
        })
        .collect();
    Ok(Running { addr, service, server, stop, threads })
}

/// TCP_NODELAY on `listener`, which the connections it accepts inherit
/// (Linux and Windows): tiny_http sends a response's head and body in
/// separate writes, and with Nagle's algorithm the last segment would
/// wait for the client's delayed ACK.
fn no_delay(listener: &std::net::TcpListener) {
    let one = 1i32;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: a valid socket and a pointer to an int of the size given.
        let _ = unsafe {
            libc::setsockopt(
                listener.as_raw_fd(),
                libc::IPPROTO_TCP,
                libc::TCP_NODELAY,
                (&one as *const i32).cast(),
                std::mem::size_of::<i32>() as libc::socklen_t,
            )
        };
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        use windows_sys::Win32::Networking::WinSock::{IPPROTO_TCP, SOCKET, TCP_NODELAY, setsockopt};
        // SAFETY: a valid socket and a pointer to an int of the size given.
        let _ = unsafe {
            setsockopt(listener.as_raw_socket() as SOCKET, IPPROTO_TCP, TCP_NODELAY, (&one as *const i32).cast(), 4)
        };
    }
    #[cfg(not(any(unix, windows)))]
    let _ = (listener, one);
}

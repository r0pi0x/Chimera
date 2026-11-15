//! The accounts service's HTTP side (docs/plans/active/hub.md, 2.3-2.4,
//! 5, 7.1): plain HTTP/1.1 on loopback (`DEFAULT_PORT` 8741), behind the
//! front (S1b) or its own TLS listener (`chimera_accounts serve --tls`,
//! the asset service's certificate). JSON in and out; a refusal is its
//! status with a line of text.
//!
//! Public (through the front), `/accounts/...`:
//! - `POST /accounts/redeem` (an invite or link code, a username, the
//!   device's public key), `POST /accounts/recover`, `POST
//!   /accounts/ticket` (a device's signed request, or `Authorization:
//!   Bearer chimera1_...`): no ticket needed, rate limited.
//! - `POST /accounts/link`, `POST /accounts/display-name`, `GET
//!   /accounts/me`: with `Authorization: Bearer <ticket>`.
//!
//! This machine only (a loopback connection relayed by no front: neither
//! the in-process TLS one nor the hub's, whose `X-Chimera-Peer` marks a
//! request as relayed; with the services' secret in `X-Chimera-Service`):
//! - `GET /internal/revoked?since=<seq>`: the revocation feed.
//! - `/admin/...`: the owner's API (`admin`'s command line and the page):
//!   `POST invite`, `GET invites`, `POST invite/revoke`, `GET accounts`,
//!   `GET account?username=` (show), `POST account/set`, `POST
//!   account/refresh`, `POST account/delete`, `POST account/revoke`,
//!   `POST device/revoke`, `POST ban`, `POST import-legacy`, `POST
//!   backup`.
//! - `GET /admin/`: the owner's page (`page`; no secret needed to load it,
//!   only to use it; a local `Host` too).
//! - `GET /health`: `ok <version> uptime=<s> db=<schema> accounts=<n>`
//!   (loopback, no secret).
//!
//! The client's address (limits, bans, the log) is the one the hub's
//! front names in `X-Chimera-Peer`, believed only on a loopback
//! connection (`chimera_assets::tls::forwarded_peer`).
//!
//! Any other path is 421 (not this service's), so a stray request is loud.
//! Each request is one line of `<store>/server/accounts/access.log`: time,
//! method, path (no query), status, the client's address; never a body,
//! a code, a key or a ticket.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use chimera_assets::{
    account::{RecoverRequest, RedeemRequest, TicketRequest},
    accounts::SERVICE_HEADER,
    tls::{PEER_HEADER, Peers, forwarded_peer},
};
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Request, Response, ResponseBox};

use super::{AccountChange, Accounts, InviteSpec, R, Refusal, refuse};

/// Largest request body.
const MAX_BODY: u64 = 256 * 1024;

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("ascii header")
}

fn text(status: u16, body: impl Into<String>) -> ResponseBox {
    Response::from_string(body.into())
        .with_status_code(status)
        .with_header(header("Content-Type", "text/plain; charset=utf-8"))
        .with_header(header("Cache-Control", "no-store"))
        .boxed()
}

fn json(status: u16, value: &impl Serialize) -> ResponseBox {
    Response::from_data(serde_json::to_vec(value).unwrap_or_default())
        .with_status_code(status)
        .with_header(header("Content-Type", "application/json"))
        .with_header(header("Cache-Control", "no-store"))
        .boxed()
}

fn answer<T: Serialize>(status: u16, r: R<T>) -> (u16, ResponseBox) {
    match r {
        Ok(v) => (status, json(status, &v)),
        Err(Refusal { status, message }) => (status, text(status, format!("{message}\n"))),
    }
}

fn body<T: for<'de> Deserialize<'de>>(request: &mut Request) -> R<T> {
    if request.body_length().is_some_and(|n| n as u64 > MAX_BODY) {
        return Err(refuse(413, "too large"));
    }
    let mut bytes = Vec::new();
    request
        .as_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| refuse(400, "the request didn't arrive whole"))?;
    serde_json::from_slice(&bytes).map_err(|e| refuse(400, format!("not the JSON expected: {e}")))
}

fn header_value(request: &Request, name: &'static str) -> Option<String> {
    request.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str().to_string())
}

fn bearer(request: &Request) -> Option<String> {
    header_value(request, "Authorization")
        .and_then(|v| v.strip_prefix("Bearer ").map(|t| t.trim().to_string()))
        .filter(|t| !t.is_empty())
}

/// The admin page (`page`), with headers that keep it to itself.
fn page() -> ResponseBox {
    Response::from_string(super::page::PAGE)
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(header("X-Content-Type-Options", "nosniff"))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header(
            "Content-Security-Policy",
            "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'",
        ))
        .with_header(header("Referrer-Policy", "no-referrer"))
        .boxed()
}

/// The HTTP side of an `Accounts`.
pub struct Http {
    pub accounts: Arc<Accounts>,
    peers: Option<Peers>,
    log: Mutex<Option<File>>,
    started: std::time::Instant,
}

#[derive(Deserialize)]
struct Name {
    name: String,
}
#[derive(Deserialize)]
struct Username {
    username: String,
}
#[derive(Deserialize)]
struct Device {
    device: String,
}
#[derive(Deserialize)]
struct Ban {
    target: String,
    #[serde(default)]
    days: Option<u64>,
    reason: String,
}
#[derive(Deserialize)]
struct Legacy {
    toml: String,
}
#[derive(Deserialize)]
struct DisplayName {
    display_name: String,
}

impl Http {
    pub fn new(accounts: Arc<Accounts>, peers: Option<Peers>) -> Self {
        let log = OpenOptions::new().create(true).append(true).open(accounts.paths.access_log()).ok();
        // Addresses and usernames: owner-only.
        crate::restrict(&accounts.paths.access_log());
        Self { accounts, peers, log: Mutex::new(log), started: std::time::Instant::now() }
    }

    fn relayed(&self, at: &SocketAddr) -> Option<IpAddr> {
        self.peers.as_ref().and_then(|p| p.lock().unwrap_or_else(|e| e.into_inner()).get(at).copied())
    }

    /// The client's address: behind the hub's front, the one its
    /// `X-Chimera-Peer` names (trusted from loopback only); behind the
    /// in-process TLS front, the one it relayed; else the connection's.
    fn address(&self, request: &Request) -> Option<IpAddr> {
        let at = *request.remote_addr()?;
        if let Some(ip) = forwarded_peer(&at, header_value(request, PEER_HEADER).as_deref()) {
            return Some(ip);
        }
        Some(self.relayed(&at).unwrap_or(at.ip()))
    }

    /// From this machine, relayed by no front.
    fn local(&self, request: &Request) -> bool {
        request.remote_addr().is_some_and(|at| at.ip().is_loopback() && self.relayed(at).is_none())
            && header_value(request, PEER_HEADER).is_none()
    }

    /// `local`, to a `Host` naming this machine (no DNS rebinding): what a
    /// browser may load the page from.
    fn local_page(&self, request: &Request) -> bool {
        let host = header_value(request, "Host").unwrap_or_default().to_ascii_lowercase();
        let host = match host.strip_prefix('[') {
            Some(v6) => v6.split(']').next().unwrap_or("").to_string(),
            None => host.split(':').next().unwrap_or("").to_string(),
        };
        self.local(request) && matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1")
    }

    fn trusted(&self, request: &Request) -> bool {
        self.local(request) && header_value(request, SERVICE_HEADER).is_some_and(|s| self.accounts.is_secret(s.trim()))
    }

    pub fn handle(&self, mut request: Request) {
        let path = request.url().split('?').next().unwrap_or("").to_string();
        let query = request.url().split_once('?').map(|(_, q)| q.to_string()).unwrap_or_default();
        let method = request.method().clone();
        let ip = self.address(&request);
        let (status, response) = self.route(&mut request, &method, &path, &query, ip);
        let _ = request.respond(response);
        let line = format!(
            "{} {method} {path} {status} ip={}\n",
            self.accounts.clock.now(),
            ip.map_or("-".into(), |i| i.to_string())
        );
        if let Some(log) = self.log.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = log.write_all(line.as_bytes());
        }
    }

    fn route(&self, request: &mut Request, method: &Method, path: &str, query: &str, ip: Option<IpAddr>) -> (u16, ResponseBox) {
        let a = &self.accounts;
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match (method, parts.as_slice()) {
            (Method::Get, ["health"]) if self.local(request) => (
                200,
                text(
                    200,
                    format!(
                        "ok {} uptime={}s db={} accounts={}\n",
                        env!("CARGO_PKG_VERSION"),
                        self.started.elapsed().as_secs(),
                        a.db_version(),
                        a.list_accounts().map_or(0, |l| l.len())
                    ),
                ),
            ),
            (_, ["health"]) => (404, text(404, "no such endpoint\n")),
            // The owner's page (it holds nothing secret: its calls need the
            // secret, `page`).
            (Method::Get, ["admin"]) if self.local_page(request) => (
                301,
                Response::empty(301).with_header(header("Location", "/admin/")).boxed(),
            ),
            (Method::Get, ["admin", ""]) if self.local_page(request) => (200, page()),
            (_, ["internal", ..]) | (_, ["admin", ..]) if !self.trusted(request) => (404, text(404, "no such endpoint\n")),
            (Method::Get, ["internal", "revoked"]) => {
                let since = query.split('&').find_map(|p| p.strip_prefix("since=")).and_then(|v| v.parse().ok()).unwrap_or(0);
                match a.revoked_since(since) {
                    Ok(feed) => (200, text(200, feed)),
                    Err(e) => (e.status, text(e.status, e.message)),
                }
            }
            (Method::Post, ["accounts", "redeem"]) => {
                answer(201, body::<RedeemRequest>(request).and_then(|r| a.redeem(&r, ip)))
            }
            (Method::Post, ["accounts", "recover"]) => {
                answer(201, body::<RecoverRequest>(request).and_then(|r| a.recover(&r, ip)))
            }
            (Method::Post, ["accounts", "ticket"]) => {
                let token = bearer(request);
                let signed = if token.is_some() { None } else { Some(body::<TicketRequest>(request)) };
                let r = match signed {
                    Some(Err(e)) => Err(e),
                    Some(Ok(req)) => a.ticket(Some(&req), None, ip),
                    None => a.ticket(None, token.as_deref(), ip),
                };
                answer(200, r.map(|ticket| chimera_assets::account::TicketAnswer { ticket }))
            }
            (Method::Post, ["accounts", "link"]) => answer(201, self.with_ticket(request).and_then(|t| a.link(&t))),
            (Method::Post, ["accounts", "display-name"]) => answer(
                200,
                self.with_ticket(request)
                    .and_then(|t| Ok((t, body::<DisplayName>(request)?)))
                    .and_then(|(t, d)| a.set_display_name(&t, &d.display_name))
                    .map(|display_name| serde_json::json!({ "display_name": display_name })),
            ),
            (Method::Get, ["accounts", "me"]) => answer(200, self.with_ticket(request).and_then(|t| a.me(&t))),
            (_, ["accounts", ..]) => (404, text(404, "no such endpoint\n")),
            // The owner's API (trusted above).
            (Method::Post, ["admin", "invite"]) => answer(201, body::<InviteSpec>(request).and_then(|s| a.invite(&s))),
            (Method::Get, ["admin", "invites"]) => answer(200, a.list_invites()),
            (Method::Post, ["admin", "invite", "revoke"]) => {
                answer(200, body::<Name>(request).and_then(|n| a.revoke_invite(&n.name)))
            }
            (Method::Get, ["admin", "accounts"]) => answer(200, a.list_accounts()),
            (Method::Get, ["admin", "account"]) => {
                let username = query.split('&').find_map(|p| p.strip_prefix("username=")).unwrap_or_default();
                answer(200, a.show_account(username))
            }
            (Method::Post, ["admin", "account", "set"]) => {
                answer(200, body::<AccountChange>(request).and_then(|c| a.set_account(&c)))
            }
            (Method::Post, ["admin", "account", "refresh"]) => {
                answer(200, body::<Username>(request).and_then(|u| a.refresh_account(&u.username)))
            }
            (Method::Post, ["admin", "account", "delete"]) => {
                answer(200, body::<Username>(request).and_then(|u| a.delete_account(&u.username)))
            }
            (Method::Post, ["admin", "account", "revoke"]) => {
                answer(200, body::<Username>(request).and_then(|u| a.revoke_account(&u.username)))
            }
            (Method::Post, ["admin", "device", "revoke"]) => {
                answer(200, body::<Device>(request).and_then(|d| a.revoke_device(&d.device)))
            }
            (Method::Post, ["admin", "ban"]) => {
                answer(200, body::<Ban>(request).and_then(|b| a.ban(&b.target, b.days, &b.reason)))
            }
            (Method::Post, ["admin", "import-legacy"]) => {
                answer(200, body::<Legacy>(request).and_then(|l| a.import_legacy(&l.toml)))
            }
            (Method::Post, ["admin", "backup"]) => {
                answer(200, a.backup().map(|p| serde_json::json!({ "path": p.display().to_string() })))
            }
            (_, ["admin", ..]) | (_, ["internal", ..]) => (404, text(404, "no such endpoint\n")),
            _ => (421, text(421, "not the accounts service's path\n")),
        }
    }

    fn with_ticket(&self, request: &Request) -> R<String> {
        bearer(request).ok_or_else(|| refuse(401, "a ticket is needed (Authorization: Bearer <ticket>)"))
    }
}

/// A running accounts service; stopped when dropped.
pub struct Running {
    pub addr: SocketAddr,
    pub http: Arc<Http>,
    server: Arc<tiny_http::Server>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl Running {
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

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

/// Serve `http` on `listen` (`127.0.0.1:0`: a free port) with `threads`
/// workers, and the nightly backup (checked every `backup_check`).
pub fn serve(http: Http, listen: &str, threads: usize, backup_check: Option<Duration>) -> std::io::Result<Running> {
    let server = Arc::new(
        tiny_http::Server::http(listen).map_err(|e| std::io::Error::other(format!("{listen}: {e}")))?,
    );
    let addr = server.server_addr().to_ip().ok_or_else(|| std::io::Error::other("not an IP listener"))?;
    let http = Arc::new(http);
    let stop = Arc::new(AtomicBool::new(false));
    let mut handles: Vec<JoinHandle<()>> = (0..threads.max(1))
        .map(|_| {
            let (server, http, stop) = (server.clone(), http.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    if let Ok(request) = server.recv() {
                        http.handle(request);
                    }
                }
            })
        })
        .collect();
    if let Some(every) = backup_check {
        let (http, stop) = (http.clone(), stop.clone());
        handles.push(std::thread::spawn(move || {
            let mut waited = Duration::ZERO;
            while !stop.load(Ordering::Relaxed) {
                if waited.is_zero() && http.accounts.backup_due() {
                    match http.accounts.backup() {
                        Ok(p) => eprintln!("accounts: nightly backup {} (integrity ok)", p.display()),
                        Err(e) => eprintln!("accounts: nightly backup failed: {}", e.message),
                    }
                }
                std::thread::sleep(Duration::from_millis(250));
                waited += Duration::from_millis(250);
                if waited >= every {
                    waited = Duration::ZERO;
                }
            }
        }));
    }
    Ok(Running { addr, http, server, stop, threads: handles })
}

//! The hub service's HTTP side (docs/plans/active/hub.md, 4): plain
//! HTTP/1.1 on loopback (`DEFAULT_PORT` 8742), behind the front at `/hub`
//! and `/hub/`. JSON in and out; a refusal is its status with a line of
//! text.
//!
//! - `GET /hub`: the signed discovery document (`X-Chimera-Signature`).
//! - `GET /hub/servers`: the list. `POST /hub/heartbeat`, `POST
//!   /hub/leave`: a game server's listing.
//! - All of them with `Authorization: Bearer <ticket>` (401 without, 403
//!   without the `list` right for a heartbeat).
//! - `POST /hub/party/{create,join,state,leave,kick,promote,lead}`: the
//!   parties (`super::party`, `chimera_assets::party_doc`), with a ticket.
//! - `GET /hub/key`: the document key (hex), to anyone.
//! - `GET /health` (this machine only, relayed by no front): `ok
//!   <version> uptime=<s> db=<schema> listings=<n> revocations=<age>`.
//! - `GET /admin/servers`, `POST /admin/delist` (`Delist`): the owner's
//!   (`chimera_hub servers`, `chimera_hub delist`), this machine only and
//!   with the services' secret in `X-Chimera-Service`; the front never
//!   relays `/admin`, and a relayed request is refused here too (404).
//!
//! The client's address is the one the front names in `X-Chimera-Peer`,
//! believed only on a loopback connection
//! (`chimera_assets::tls::forwarded_peer`); a request straight to the
//! loopback port is this machine's. Any path outside `/hub` is 421. Each
//! request is a line of `<store>/server/hub/access.log` (time, method,
//! path without the query, status, address); never a ticket or a body.

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
    accounts::SERVICE_HEADER,
    hub_doc::{Heartbeat, Leave, SIGNATURE_HEADER},
    tls::{PEER_HEADER, forwarded_peer},
};
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Request, Response, ResponseBox};

use super::{Hub, refuse};
use crate::accounts::{R, Refusal};

const MAX_BODY: u64 = 64 * 1024;

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

fn answer<T: Serialize>(r: R<T>) -> (u16, ResponseBox) {
    match r {
        Ok(v) => (200, json(200, &v)),
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

/// The HTTP side of a `Hub`.
pub struct Http {
    pub hub: Arc<Hub>,
    log: Mutex<Option<File>>,
    started: std::time::Instant,
}

impl Http {
    pub fn new(hub: Arc<Hub>) -> Self {
        let log = OpenOptions::new().create(true).append(true).open(hub.paths.access_log()).ok();
        Self { hub, log: Mutex::new(log), started: std::time::Instant::now() }
    }

    /// The client's address: the front's `X-Chimera-Peer` on a loopback
    /// connection, else the connection's.
    fn address(&self, request: &Request) -> Option<IpAddr> {
        let at = *request.remote_addr()?;
        Some(forwarded_peer(&at, header_value(request, PEER_HEADER).as_deref()).unwrap_or(at.ip()))
    }

    /// From this machine, relayed by no front.
    fn local(request: &Request) -> bool {
        request.remote_addr().is_some_and(|at| at.ip().is_loopback()) && header_value(request, PEER_HEADER).is_none()
    }

    pub fn handle(&self, mut request: Request) {
        let path = request.url().split('?').next().unwrap_or("").to_string();
        let method = request.method().clone();
        let ip = self.address(&request);
        let (status, response) = self.route(&mut request, &method, &path, ip);
        let _ = request.respond(response);
        let line = format!("{} {method} {path} {status} ip={}\n", self.hub.clock.now(), ip.map_or("-".into(), |i| i.to_string()));
        if let Some(log) = self.log.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = log.write_all(line.as_bytes());
        }
    }

    fn route(&self, request: &mut Request, method: &Method, path: &str, ip: Option<IpAddr>) -> (u16, ResponseBox) {
        let h = &self.hub;
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match (method, parts.as_slice()) {
            (Method::Get, ["health"]) if Self::local(request) => (
                200,
                text(
                    200,
                    format!(
                        "ok {} uptime={}s db={} listings={} parties={} {}\n",
                        env!("CARGO_PKG_VERSION"),
                        self.started.elapsed().as_secs(),
                        h.db_version(),
                        h.count(),
                        h.party_count(),
                        h.auth_status()
                    ),
                ),
            ),
            (_, ["health"]) => (404, text(404, "no such endpoint\n")),
            (_, ["admin", ..]) if !Self::local(request) => (404, text(404, "no such endpoint\n")),
            (_, ["admin", ..]) if !header_value(request, SERVICE_HEADER).is_some_and(|s| h.is_secret(&s)) => {
                (401, text(401, "the services' secret is needed\n"))
            }
            (Method::Get, ["admin", "servers"]) => (200, json(200, &h.admin_list())),
            (Method::Post, ["admin", "delist"]) => answer(body::<super::Delist>(request).and_then(|d| h.delist(&d.instance, d.secs))),
            (_, ["admin", ..]) => (404, text(404, "no such endpoint\n")),
            (Method::Get, ["hub", "key"]) => (200, text(200, format!("{}\n", h.public_key()))),
            (Method::Get, ["hub"] | ["hub", ""]) => match h.check(bearer(request).as_deref()) {
                Ok(_) => {
                    let (body, signature) = h.document();
                    (
                        200,
                        Response::from_data(body)
                            .with_header(header("Content-Type", "application/json"))
                            .with_header(header("Cache-Control", "no-store"))
                            .with_header(header(SIGNATURE_HEADER, &signature))
                            .boxed(),
                    )
                }
                Err(e) => answer::<()>(Err(e)),
            },
            (Method::Get, ["hub", "servers"]) => answer(h.check(bearer(request).as_deref()).map(|c| h.list(&c))),
            (Method::Post, ["hub", "heartbeat"]) => {
                let r = h.check(bearer(request).as_deref()).and_then(|c| {
                    let beat: Heartbeat = body(request)?;
                    let peer = ip.ok_or_else(|| refuse(400, "no address"))?;
                    h.heartbeat(&c, &beat, peer)
                });
                answer(r)
            }
            (Method::Post, ["hub", "leave"]) => {
                let r = h.check(bearer(request).as_deref()).and_then(|c| {
                    let l: Leave = body(request)?;
                    h.leave(&c, &l.instance).map(|()| serde_json::json!({ "left": true }))
                });
                answer(r)
            }
            (Method::Post, ["hub", "party", what]) => self.party(request, what, ip),
            (_, ["hub", ..]) => (404, text(404, "no such endpoint\n")),
            _ => (421, text(421, "not the hub's path\n")),
        }
    }
}

impl Http {
    /// `POST /hub/party/<what>` (`super::party`).
    fn party(&self, request: &mut Request, what: &str, ip: Option<IpAddr>) -> (u16, ResponseBox) {
        use chimera_assets::party_doc::{Create, Join, Lead, OnMember, State, Token};
        let h = &self.hub;
        let claims = match h.check(bearer(request).as_deref()) {
            Ok(c) => c,
            Err(e) => return answer::<()>(Err(e)),
        };
        match what {
            "create" => answer(body::<Create>(request).and_then(|b| h.party_create(&claims, &b))),
            "join" => answer(body::<Join>(request).and_then(|b| h.party_join(&claims, &b, ip))),
            "state" => answer(body::<State>(request).and_then(|b| h.party_state(&claims, &b))),
            "leave" => answer(body::<Token>(request).and_then(|b| h.party_leave(&claims, &b.token)).map(|()| serde_json::json!({}))),
            "kick" => answer(body::<OnMember>(request).and_then(|b| h.party_kick(&claims, &b))),
            "promote" => answer(body::<OnMember>(request).and_then(|b| h.party_promote(&claims, &b))),
            "lead" => answer(body::<Lead>(request).and_then(|b| h.party_lead(&claims, &b, ip))),
            _ => (404, text(404, "no such endpoint\n")),
        }
    }
}

/// A running hub service; stopped when dropped.
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
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

/// Serve `http` on `listen` (`127.0.0.1:0`: a free port) with `threads`
/// workers, and a sweep of expired listings every few seconds.
pub fn serve(http: Http, listen: &str, threads: usize) -> std::io::Result<Running> {
    let server = Arc::new(tiny_http::Server::http(listen).map_err(|e| std::io::Error::other(format!("{listen}: {e}")))?);
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
    let (h, s) = (http.clone(), stop.clone());
    handles.push(std::thread::spawn(move || {
        while !s.load(Ordering::Relaxed) {
            h.hub.sweep();
            std::thread::park_timeout(Duration::from_secs(5));
        }
    }));
    Ok(Running { addr, http, server, stop, threads: handles })
}

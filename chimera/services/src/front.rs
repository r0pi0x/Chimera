//! The hub's front (docs/plans/active/hub.md, 2.3): the one public TLS
//! port, with the certificate players pin (the asset service's,
//! `<store>/server/tls/`, so every tester's binding keeps working), in
//! front of the services' plain loopback listeners. Binary
//! `chimera_front`; no state of its own.
//!
//! - **Routing by path**: the first request on a connection picks the
//!   service by its path's first segment (`route`): `/accounts/...` the
//!   accounts service, `/hub` the hub, `/feedback` the feedback route (the
//!   asset service's until S5), everything else the asset service (old
//!   clients use root paths). The connection is relayed to that service
//!   for its whole life; a later request on it for another service is
//!   answered **421** and the connection closed (our clients keep a pool
//!   per service). The services answer 421 to paths not their own too.
//! - **Never relayed** (404 from the front): `/admin`, `/internal`,
//!   `/health` and `/feedback/admin`, in any case, however many leading
//!   slashes; those exist on the loopback listeners only.
//! - **The client's address**: every request head gets `X-Chimera-Peer:
//!   <ip:port>` (any the client sent is dropped), so the services' access
//!   logs, per-address limits and bans see the real peer; they believe it
//!   only on a loopback connection. To do that the front reads request
//!   heads and `Content-Length` bodies; a chunked request body is 411
//!   (our clients never send one), a head over 16 KiB 431, a malformed
//!   one 400. Answers pass through untouched.
//! - **Per-address limits** (`tls::FrontLimits`): connections at once per
//!   address and in all, the handshake's time, an idle timeout.
//! - A service that doesn't answer is 502 with its name ("try again
//!   later"); the others go on.
//! - `GET /health` from this machine (a loopback peer): `ok <version>
//!   uptime=<s> connections=<n>`, answered by the front itself.
//!
//! Known shortcuts: requests on one connection must not be pipelined (a
//! client sends the next once the answer arrived, as ureq and browsers
//! do): a 421 or 404 the front makes mid-connection would otherwise
//! interleave with an answer under way; no graceful drain on SIGTERM
//! (connections under way are cut; clients retry).

use std::{
    collections::HashMap,
    io::{self, Read, Write},
    net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use chimera_assets::tls::{FrontLimits, Identity, PEER_HEADER};
use rustls::{ServerConfig, ServerConnection};

/// A request head's largest size.
pub const MAX_HEAD: usize = 16 * 1024;

/// Where the front sends each kind of path (loopback listeners).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Routes {
    pub assets: SocketAddr,
    pub accounts: SocketAddr,
    pub hub: SocketAddr,
    /// `/feedback` (the asset service's until S5).
    pub feedback: SocketAddr,
}

/// What a request's path is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// A service: its name (for messages) and listener.
    Service(&'static str, SocketAddr),
    /// The front's own `/health` (a loopback peer only).
    Health,
    /// Never relayed: 404.
    Refused,
}

/// The route of `target` (a request line's target, query and all).
pub fn route(routes: &Routes, target: &str) -> Route {
    let path = target.split(['?', '#']).next().unwrap_or("");
    let mut parts = path.trim_start_matches('/').split('/');
    let first = parts.next().unwrap_or("").to_ascii_lowercase();
    let second = parts.next().unwrap_or("").to_ascii_lowercase();
    match first.as_str() {
        "admin" | "internal" => Route::Refused,
        "health" => Route::Health,
        "feedback" if second == "admin" => Route::Refused,
        "feedback" => Route::Service("feedback", routes.feedback),
        "accounts" => Route::Service("accounts", routes.accounts),
        "hub" => Route::Service("hub", routes.hub),
        _ => Route::Service("assets", routes.assets),
    }
}

/// Why a request head was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bad {
    /// 431.
    TooLarge,
    /// 411: a body without `Content-Length`.
    Chunked,
    /// 400.
    Malformed,
}

impl Bad {
    fn answer(self) -> (u16, &'static str, &'static str) {
        match self {
            Bad::TooLarge => (431, "Request Header Fields Too Large", "the request's head is too large\n"),
            Bad::Chunked => (411, "Length Required", "a request body needs a Content-Length\n"),
            Bad::Malformed => (400, "Bad Request", "not a request the hub understands\n"),
        }
    }
}

/// A piece of the client's byte stream, rewritten.
#[derive(Debug, PartialEq, Eq)]
pub enum Piece {
    /// A request head with `X-Chimera-Peer` set: its target, its bytes.
    Head { target: String, bytes: Vec<u8> },
    Body(Vec<u8>),
}

/// Splits the client's requests into heads and bodies, rewriting heads.
#[derive(Debug)]
pub struct Requests {
    peer: SocketAddr,
    buf: Vec<u8>,
    body_left: u64,
}

impl Requests {
    pub fn new(peer: SocketAddr) -> Self {
        Self { peer, buf: Vec::new(), body_left: 0 }
    }

    /// Take `data`; the pieces complete so far.
    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<Piece>, Bad> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            if self.body_left > 0 {
                let n = (self.body_left.min(self.buf.len() as u64)) as usize;
                if n == 0 {
                    break;
                }
                out.push(Piece::Body(self.buf.drain(..n).collect()));
                self.body_left -= n as u64;
                continue;
            }
            // Empty lines between requests are allowed.
            while self.buf.starts_with(b"\r\n") {
                self.buf.drain(..2);
            }
            let Some(end) = self.buf.windows(4).position(|w| w == b"\r\n\r\n") else {
                if self.buf.len() > MAX_HEAD {
                    return Err(Bad::TooLarge);
                }
                break;
            };
            if end + 4 > MAX_HEAD {
                return Err(Bad::TooLarge);
            }
            let head: Vec<u8> = self.buf.drain(..end + 4).collect();
            let (target, bytes, length) = rewrite(&head[..end], self.peer)?;
            out.push(Piece::Head { target, bytes });
            self.body_left = length;
        }
        Ok(out)
    }
}

/// A head (without its final blank line) with `X-Chimera-Peer` set: its
/// target, its bytes, its body's length.
fn rewrite(head: &[u8], peer: SocketAddr) -> Result<(String, Vec<u8>, u64), Bad> {
    let text = std::str::from_utf8(head).map_err(|_| Bad::Malformed)?;
    let mut lines = text.split("\r\n");
    let first = lines.next().ok_or(Bad::Malformed)?;
    let mut words = first.split(' ');
    let (method, target, version) = (words.next(), words.next(), words.next());
    let (Some(method), Some(target), Some(version), None) = (method, target, version, words.next()) else {
        return Err(Bad::Malformed);
    };
    if method.is_empty() || !method.bytes().all(|b| b.is_ascii_uppercase()) || !target.starts_with('/') || !version.starts_with("HTTP/1.") {
        return Err(Bad::Malformed);
    }
    let mut out = format!("{first}\r\n");
    let mut length: Option<u64> = None;
    for line in lines {
        if line.starts_with([' ', '\t']) {
            // Folded headers: obsolete, and a way to hide one.
            return Err(Bad::Malformed);
        }
        let (name, value) = line.split_once(':').ok_or(Bad::Malformed)?;
        if name.is_empty() || name.ends_with([' ', '\t']) || !name.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(Bad::Malformed);
        }
        if name.eq_ignore_ascii_case(PEER_HEADER) {
            continue;
        }
        if name.eq_ignore_ascii_case("Transfer-Encoding") {
            return Err(Bad::Chunked);
        }
        if name.eq_ignore_ascii_case("Content-Length") {
            let n: u64 = value.trim().parse().map_err(|_| Bad::Malformed)?;
            if length.is_some_and(|l| l != n) {
                return Err(Bad::Malformed);
            }
            length = Some(n);
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str(&format!("{PEER_HEADER}: {peer}\r\n\r\n"));
    Ok((target.to_string(), out.into_bytes(), length.unwrap_or(0)))
}

/// The front's counters (its `/health`).
#[derive(Default)]
struct Stats {
    connections: AtomicUsize,
}

/// A running front; stopped when dropped (connections under way end on
/// their own).
pub struct Front {
    pub addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Front {
    /// `https://<addr>`.
    pub fn url(&self) -> String {
        format!("https://{}", self.addr)
    }

    /// Serve until the process ends.
    pub fn wait(mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Front {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Accept TLS on `listen` with `identity`'s certificate and relay each
/// connection to the service its first request's path names.
pub fn serve(listen: &str, identity: &Identity, routes: Routes, limits: FrontLimits) -> io::Result<Front> {
    let config = identity.server_config()?;
    let listener = TcpListener::bind(listen).map_err(|e| io::Error::new(e.kind(), format!("{listen}: {e}")))?;
    let addr = listener.local_addr()?;
    let stop = Arc::new(AtomicBool::new(false));
    let counts: Arc<Mutex<HashMap<IpAddr, usize>>> = Arc::default();
    let stats = Arc::new(Stats::default());
    let started = Instant::now();
    let stop2 = stop.clone();
    let thread = std::thread::Builder::new().name("front".into()).spawn(move || {
        for stream in listener.incoming() {
            if stop2.load(Ordering::Relaxed) {
                break;
            }
            let Ok(stream) = stream else { continue };
            let Ok(peer) = stream.peer_addr() else { continue };
            let ip = peer.ip();
            {
                let mut c = counts.lock().unwrap_or_else(|e| e.into_inner());
                let n = c.entry(ip).or_default();
                if *n >= limits.per_address || stats.connections.load(Ordering::Relaxed) >= limits.total {
                    eprintln!("front: refused a connection from {ip}: too many at once");
                    continue;
                }
                *n += 1;
                stats.connections.fetch_add(1, Ordering::Relaxed);
            }
            let (config, counts, stats) = (config.clone(), counts.clone(), stats.clone());
            let _ = std::thread::Builder::new().name("front-conn".into()).spawn(move || {
                let health = || format!("ok {} uptime={}s connections={}\n", env!("CARGO_PKG_VERSION"), started.elapsed().as_secs(), stats.connections.load(Ordering::Relaxed));
                if let Err(e) = connection(stream, peer, config, &routes, limits, &health)
                    && !matches!(
                        e.kind(),
                        io::ErrorKind::UnexpectedEof
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::BrokenPipe
                            | io::ErrorKind::NotConnected
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                    )
                {
                    eprintln!("front: {ip}: {e}");
                }
                let mut c = counts.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(n) = c.get_mut(&ip) {
                    *n -= 1;
                    if *n == 0 {
                        c.remove(&ip);
                    }
                }
                stats.connections.fetch_sub(1, Ordering::Relaxed);
            });
        }
    })?;
    Ok(Front { addr, stop, thread: Some(thread) })
}

type Tls = Arc<Mutex<ServerConnection>>;

/// Send an answer the front makes itself, then close the connection.
fn answer(conn: &Tls, client: &TcpStream, status: u16, reason: &str, body: &str) -> io::Result<()> {
    let mut out = client.try_clone()?;
    let mut c = conn.lock().unwrap_or_else(|e| e.into_inner());
    let text = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    c.writer().write_all(text.as_bytes())?;
    c.send_close_notify();
    while c.wants_write() {
        c.write_tls(&mut out)?;
    }
    let _ = out.shutdown(Shutdown::Both);
    Ok(())
}

/// The relay to one service: its listener and the thread sending its
/// answers to the client.
struct Backend {
    addr: SocketAddr,
    stream: TcpStream,
    down: JoinHandle<io::Result<()>>,
}

/// One client connection: the handshake, then its requests to the service
/// the first one names, their answers back.
fn connection(
    mut client: TcpStream,
    peer: SocketAddr,
    config: Arc<ServerConfig>,
    routes: &Routes,
    limits: FrontLimits,
    health: &dyn Fn() -> String,
) -> io::Result<()> {
    let _ = client.set_nodelay(true);
    client.set_read_timeout(Some(limits.handshake))?;
    client.set_write_timeout(Some(limits.handshake))?;
    let mut conn = ServerConnection::new(config).map_err(io::Error::other)?;
    while conn.is_handshaking() {
        conn.complete_io(&mut client)?;
    }
    client.set_read_timeout(Some(limits.idle))?;
    client.set_write_timeout(Some(limits.idle))?;
    let conn: Tls = Arc::new(Mutex::new(conn));
    let mut requests = Requests::new(peer);
    let mut backend: Option<Backend> = None;
    let mut raw = vec![0u8; 64 * 1024];
    let mut plain = Vec::new();
    let result = (|| -> io::Result<()> {
        loop {
            // Decrypted bytes already waiting (sent behind the handshake),
            // then what arrives.
            plain.clear();
            let mut closed = false;
            {
                let mut c = conn.lock().unwrap_or_else(|e| e.into_inner());
                let state = c.process_new_packets().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                let available = state.plaintext_bytes_to_read();
                if available > 0 {
                    plain.resize(available, 0);
                    c.reader().read_exact(&mut plain)?;
                }
                closed |= state.peer_has_closed();
                while c.wants_write() {
                    c.write_tls(&mut client)?;
                }
            }
            if plain.is_empty() && !closed {
                let n = match client.read(&mut raw) {
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                };
                if n == 0 {
                    return Ok(());
                }
                let mut rest = &raw[..n];
                let mut c = conn.lock().unwrap_or_else(|e| e.into_inner());
                while !rest.is_empty() {
                    c.read_tls(&mut rest)?;
                    let state = match c.process_new_packets() {
                        Ok(state) => state,
                        Err(e) => {
                            // Tell the client why before giving up.
                            let _ = c.write_tls(&mut client);
                            return Err(io::Error::new(io::ErrorKind::InvalidData, e));
                        }
                    };
                    // Drained record by record: rustls holds little
                    // plaintext unread.
                    let available = state.plaintext_bytes_to_read();
                    if available > 0 {
                        let at = plain.len();
                        plain.resize(at + available, 0);
                        c.reader().read_exact(&mut plain[at..])?;
                    }
                    closed |= state.peer_has_closed();
                }
                while c.wants_write() {
                    c.write_tls(&mut client)?;
                }
                if plain.is_empty() && !closed {
                    continue;
                }
            }
            let pieces = match requests.feed(&plain) {
                Ok(p) => p,
                Err(bad) => {
                    let (status, reason, body) = bad.answer();
                    return answer(&conn, &client, status, reason, body);
                }
            };
            for piece in pieces {
                match piece {
                    Piece::Head { target, bytes } => match route(routes, &target) {
                        Route::Refused => return answer(&conn, &client, 404, "Not Found", "no such endpoint\n"),
                        Route::Health if peer.ip().is_loopback() && backend.is_none() => {
                            return answer(&conn, &client, 200, "OK", &health());
                        }
                        Route::Health => return answer(&conn, &client, 404, "Not Found", "no such endpoint\n"),
                        Route::Service(name, addr) => {
                            match &mut backend {
                                Some(b) if b.addr != addr => {
                                    return answer(
                                        &conn,
                                        &client,
                                        421,
                                        "Misdirected Request",
                                        "another service's path on this connection: open a new one\n",
                                    );
                                }
                                Some(b) => b.stream.write_all(&bytes)?,
                                None => {
                                    let Ok(stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(5)) else {
                                        return answer(
                                            &conn,
                                            &client,
                                            502,
                                            "Bad Gateway",
                                            &format!("the hub's {name} service isn't answering: try again later\n"),
                                        );
                                    };
                                    let _ = stream.set_nodelay(true);
                                    let down = {
                                        let (conn, client, back) = (conn.clone(), client.try_clone()?, stream.try_clone()?);
                                        std::thread::Builder::new().name("front-down".into()).spawn(move || down(conn, client, back))?
                                    };
                                    let mut b = Backend { addr, stream, down };
                                    b.stream.write_all(&bytes)?;
                                    backend = Some(b);
                                }
                            }
                        }
                    },
                    Piece::Body(bytes) => {
                        if let Some(b) = &mut backend {
                            b.stream.write_all(&bytes)?;
                        }
                    }
                }
            }
            if closed {
                return Ok(());
            }
        }
    })();
    // The client is done sending: the service sees the end of its
    // requests, answers what is under way and closes.
    if let Some(b) = backend {
        let _ = b.stream.shutdown(Shutdown::Write);
        let down = b.down.join().unwrap_or_else(|_| Err(io::Error::other("relay thread panicked")));
        let _ = client.shutdown(Shutdown::Both);
        return result.and(down);
    }
    let _ = client.shutdown(Shutdown::Both);
    result
}

/// A service's answers, encrypted, to the client, until the service closes.
fn down(conn: Tls, mut client: TcpStream, mut back: TcpStream) -> io::Result<()> {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match back.read(&mut buf) {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => 0,
        };
        let mut c = conn.lock().unwrap_or_else(|e| e.into_inner());
        if n == 0 {
            c.send_close_notify();
        } else {
            c.writer().write_all(&buf[..n])?;
        }
        while c.wants_write() {
            c.write_tls(&mut client)?;
        }
        if n == 0 {
            let _ = client.shutdown(Shutdown::Write);
            return Ok(());
        }
    }
}

/// The front's `/health` at `url` (`https://127.0.0.1:<port>`, from this
/// machine), its certificate pinned: `chimera_accounts status`.
pub fn health(url: &str, fingerprint: &str) -> Result<String, String> {
    let agent = chimera_assets::remote::agent(Some(fingerprint))?;
    let mut r = agent.get(&format!("{}/health", url.trim_end_matches('/'))).call().map_err(|e| e.to_string())?;
    let status = r.status().as_u16();
    let text = r.body_mut().read_to_string().map_err(|e| e.to_string())?;
    if status != 200 {
        return Err(format!("{status} {}", text.trim()));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn routes() -> Routes {
        let at = |p: u16| SocketAddr::from(([127, 0, 0, 1], p));
        Routes { assets: at(1), accounts: at(2), hub: at(3), feedback: at(4) }
    }

    #[test]
    fn paths_route_by_their_first_segment() {
        let r = routes();
        let s = |t: &str| route(&r, t);
        assert_eq!(s("/r/abc"), Route::Service("assets", r.assets));
        assert_eq!(s("/key"), Route::Service("assets", r.assets));
        assert_eq!(s("/"), Route::Service("assets", r.assets));
        assert_eq!(s("/accounts/redeem"), Route::Service("accounts", r.accounts));
        assert_eq!(s("/hub?x=1"), Route::Service("hub", r.hub));
        assert_eq!(s("/hub/servers"), Route::Service("hub", r.hub));
        assert_eq!(s("/hubble"), Route::Service("assets", r.assets));
        assert_eq!(s("/feedback"), Route::Service("feedback", r.feedback));
        for refused in ["/admin/", "//admin/invites", "/ADMIN", "/internal/revoked?since=0", "/feedback/admin/list", "/Feedback/Admin"] {
            assert_eq!(s(refused), Route::Refused, "{refused}");
        }
        assert_eq!(s("/health"), Route::Health);
    }

    #[test]
    fn heads_get_the_peer_and_lose_a_forged_one() {
        let peer: SocketAddr = "198.51.100.4:61000".parse().unwrap();
        let mut r = Requests::new(peer);
        let pieces = r
            .feed(b"POST /batch HTTP/1.1\r\nHost: h\r\nx-chimera-peer: 10.0.0.1:1\r\nContent-Length: 5\r\n\r\nab")
            .unwrap();
        let Piece::Head { target, bytes } = &pieces[0] else { panic!() };
        assert_eq!(target, "/batch");
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(!text.contains("10.0.0.1"), "{text}");
        assert!(text.ends_with("X-Chimera-Peer: 198.51.100.4:61000\r\n\r\n"), "{text}");
        assert_eq!(pieces[1], Piece::Body(b"ab".to_vec()));
        // The rest of the body, then the next request in pieces.
        let pieces = r.feed(b"cdeGET /key HTTP/1.1\r\n").unwrap();
        assert_eq!(pieces, vec![Piece::Body(b"cde".to_vec())]);
        let pieces = r.feed(b"\r\n").unwrap();
        assert!(matches!(&pieces[0], Piece::Head { target, .. } if target == "/key"));
    }

    #[test]
    fn bad_heads_are_refused() {
        let peer: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let bad = |t: &[u8]| Requests::new(peer).feed(t).unwrap_err();
        assert_eq!(bad(b"POST /f HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"), Bad::Chunked);
        assert_eq!(bad(b"GET http://elsewhere/ HTTP/1.1\r\n\r\n"), Bad::Malformed);
        assert_eq!(bad(b"GET / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\n"), Bad::Malformed);
        assert_eq!(bad(b"GET / HTTP/1.1\r\nA: b\r\n folded\r\n\r\n"), Bad::Malformed);
        assert_eq!(bad(&[b'a'; MAX_HEAD + 10]), Bad::TooLarge);
    }
}

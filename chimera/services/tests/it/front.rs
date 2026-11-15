//! The hub behind its front (docs/plans/active/hub.md 2.3, S1b): the
//! accounts service and the asset service on free loopback ports over one
//! scratch store, `front::serve` on another with a fixed certificate, as
//! on the host. Through the front: invite, redeem, ticket and download
//! with the certificate pinned; an old client's root paths with a legacy
//! token; feedback; accounts stopped (downloads go on, a redeem fails
//! cleanly, a second start reuses its ticket); the asset service stopped
//! (accounts unaffected); a forged `X-Chimera-Peer` dropped (limits see
//! the real peer); admin paths refused; another service's path on a
//! connection answered 421.

use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::Arc,
    time::Duration,
};

use chimera_assets::{
    FileRecord, Manifest, Remote, RemoteConfig, Store,
    account::{self, AccountFile, HubLink, Session},
    accounts::Accounts as LegacyAccounts,
    feedback::{ReportState, bundle},
    server,
    tls::{FrontLimits, Identity},
};
use chimera_services::{
    accounts::{Accounts, Clock, InviteSpec, Invited, Options, Paths, http},
    admin::Admin,
    front::{self, Routes},
};

use super::scratch;

struct Hub {
    dir: std::path::PathBuf,
    store: Store,
    paths: Paths,
    clock: Clock,
    accounts: Option<http::Running>,
    accounts_addr: SocketAddr,
    assets: Option<server::Running>,
    front: front::Front,
    fingerprint: String,
    admin: Admin,
    record: String,
}

/// A loopback port nothing listens on (the hub service, not built yet).
fn free_port() -> SocketAddr {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap()
}

impl Hub {
    fn new(name: &str, limits: server::Limits) -> Self {
        let dir = scratch(&format!("front_{name}"));
        let store = Store::new(dir.join("server-store"));
        let bytes = b"fixture's bytes through the front";
        let put = store.put(bytes).unwrap();
        let mut m = Manifest::new("fixture", "1", None);
        m.files.insert("data/file.txt".into(), FileRecord { id: put.id.clone(), size: bytes.len() as u64, stored: put.stored });
        m.save(&store.manifest_path("fixture", "1")).unwrap();
        let paths = Paths::new(store.root.clone());
        let clock = Clock::default();
        let accounts = Arc::new(Accounts::open(paths.clone(), Options::default(), clock.clone()).unwrap());
        let running = http::serve(http::Http::new(accounts, None), "127.0.0.1:0", 2, None).unwrap();
        let service = server::Service::with_tickets(store.clone(), &running.url(), Duration::from_millis(200))
            .unwrap()
            .with_limits(limits);
        let assets = server::serve(service, "127.0.0.1:0", 2).unwrap();
        let identity = Identity::from_seed(&[11; 32], [12; 16]);
        let routes = Routes { assets: assets.addr, accounts: running.addr, hub: free_port(), feedback: assets.addr };
        let front = front::serve("127.0.0.1:0", &identity, routes, FrontLimits::default()).unwrap();
        let secret = fs::read_to_string(paths.secret()).unwrap();
        let admin = Admin::new(&running.url(), secret.trim());
        Self {
            dir,
            store,
            paths,
            clock,
            accounts_addr: running.addr,
            accounts: Some(running),
            assets: Some(assets),
            front,
            fingerprint: identity.fingerprint(),
            admin,
            record: put.id,
        }
    }

    fn invite(&self, name: &str) -> String {
        let invited: Invited = self
            .admin
            .post("/admin/invite", &InviteSpec { name: name.into(), content: Some("fixture".into()), ..Default::default() })
            .unwrap();
        invited.code
    }

    /// The hub link as the owner sends it: the front's address and its
    /// certificate's fingerprint.
    fn link(&self) -> HubLink {
        HubLink { host: self.front.addr.to_string(), fingerprint: Some(self.fingerprint.clone()), code: None }
    }

    fn remote(&self, token: &str) -> Remote {
        Remote::new(RemoteConfig {
            url: self.front.url(),
            token: token.into(),
            key: None,
            fingerprint: Some(self.fingerprint.clone()),
        })
        .unwrap()
    }

    fn access_log(&self) -> String {
        fs::read_to_string(self.store.root.join("server").join("access.log")).unwrap_or_default()
    }

    /// Raw requests on one TLS connection to the front: each answer's
    /// status line and body, in order (stops at a closed connection).
    fn raw(&self, requests: &[String]) -> Vec<(u16, String)> {
        let config = chimera_assets::tls::pinned_client_config(&self.fingerprint).unwrap();
        let name = rustls::pki_types::ServerName::try_from("hub").unwrap();
        let conn = rustls::ClientConnection::new(config, name).unwrap();
        let tcp = TcpStream::connect(self.front.addr).unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut tls = rustls::StreamOwned::new(conn, tcp);
        let mut out = Vec::new();
        for request in requests {
            if tls.write_all(request.as_bytes()).and_then(|_| tls.flush()).is_err() {
                break;
            }
            match read_answer(&mut tls) {
                Some(a) => out.push(a),
                None => break,
            }
        }
        out
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// One HTTP/1.1 answer with a `Content-Length` (or up to the close).
fn read_answer(r: &mut impl Read) -> Option<(u16, String)> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if r.read(&mut byte).ok()? == 0 {
            return None;
        }
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head).to_string();
    let status = head.split(' ').nth(1)?.parse().ok()?;
    let length = head
        .lines()
        .find_map(|l| l.split_once(':').filter(|(n, _)| n.eq_ignore_ascii_case("content-length")).map(|(_, v)| v.trim().parse::<usize>()))
        .and_then(Result::ok);
    let mut body = Vec::new();
    match length {
        Some(n) => {
            body.resize(n, 0);
            r.read_exact(&mut body).ok()?;
        }
        None => {
            let _ = r.read_to_end(&mut body);
        }
    }
    Some((status, String::from_utf8_lossy(&body).to_string()))
}

fn get(path: &str, extra: &str) -> String {
    format!("GET {path} HTTP/1.1\r\nHost: hub\r\n{extra}\r\n")
}

/// The whole path through the one public port: the owner invites, the
/// player redeems over the pinned link, its ticket downloads the record;
/// the asset service logs the player's real address; an old client's
/// root paths with a legacy token still reach the asset service with the
/// same fingerprint.
#[test]
fn invite_redeem_ticket_and_download_through_the_front() {
    let hub = Hub::new("flow", server::Limits::default());
    let code = hub.invite("Alice");
    let (file, recovery) = account::redeem(&hub.link(), &code, None, "test pc").unwrap();
    assert!(recovery.is_some());
    assert_eq!(file.url, hub.front.url(), "the account's hub is the front");
    let session = Session::new(file, None);
    let remote = Remote::new(session.remote().expect("the account's asset service")).unwrap();
    assert!(remote.whoami().unwrap().starts_with("account alice"));
    assert!(remote.record(&hub.record).is_ok());
    let log = hub.access_log();
    assert!(log.lines().any(|l| l.contains("GET /r/") && l.contains("account:alice") && l.contains("ip=127.0.0.1")), "{log}");

    // An old client: a legacy token on root paths, the same certificate.
    let path = LegacyAccounts::path(&hub.store.root);
    let mut legacy = LegacyAccounts::load(&path).unwrap();
    let (_, token) = legacy.add("bob", None, "old pc").unwrap();
    legacy.save(&path).unwrap();
    let old = hub.remote(&token);
    assert!(old.whoami().unwrap().starts_with("internal bob"));
    assert!(old.record(&hub.record).is_ok());
    assert!(hub.raw(&[get("/key", "")])[0].0 == 200, "/key stays public");
}

/// A report through the front reaches the feedback route (the asset
/// service's until S5) with an account's ticket.
#[test]
fn feedback_goes_through_the_front() {
    let hub = Hub::new("feedback", server::Limits::default());
    let (file, _) = account::redeem(&hub.link(), &hub.invite("Carol"), None, "test pc").unwrap();
    let remote = hub.remote(file.ticket.as_deref().unwrap());
    let report = bundle::build(&ReportState { text: "the front relays reports".into(), ..ReportState::default() }).unwrap();
    let (status, _, body) =
        remote.post_raw("/feedback", "application/zip", &[("X-Chimera-Report-Id", &report.id)], &report.bytes).unwrap();
    assert_eq!(status, 201, "{body}");
    let stored = hub.store.root.join("server").join("feedback");
    assert!(fs::read_dir(&stored).map(|d| d.count()).unwrap_or(0) > 0, "nothing stored in {}", stored.display());
    // After the record download on the same pooled connection: the same
    // service behind both, no 421.
    assert!(remote.record(&hub.record).is_ok());
}

/// Accounts stopped mid-test: downloads with a ticket go on; a new redeem
/// fails cleanly (502, saying which service); a second client start reads
/// its ticket from the file and downloads with accounts still down, even
/// past half the ticket's life (the refresh fails, the old ticket serves).
/// The asset service stopped (another hub): accounts unaffected.
#[test]
fn a_service_down_leaves_the_others_working() {
    let mut hub = Hub::new("down", server::Limits::default());
    // A ticket issued 4 hours ago (6-hour tickets: past half-life).
    hub.clock.advance(-4 * 3600);
    let (file, _) = account::redeem(&hub.link(), &hub.invite("Dave"), None, "test pc").unwrap();
    hub.clock.advance(4 * 3600);
    let path = hub.dir.join("account.toml");
    file.save_to(&path).unwrap();
    // (In memory: its refresh now mustn't replace the file's old ticket.)
    let first = Remote::new(Session::new(file, None).remote().unwrap()).unwrap();
    assert!(first.record(&hub.record).is_ok());

    hub.accounts = None;
    assert!(first.record(&hub.record).is_ok(), "downloads go on with accounts down");
    let e = account::redeem(&hub.link(), &hub.invite_offline(), None, "test pc").unwrap_err();
    assert!(e.contains("502") && e.contains("accounts"), "{e}");

    // A second start: the file's ticket, accounts still down.
    let again = Session::new(AccountFile::load_from(&path).unwrap().unwrap(), Some(path.clone()));
    let remote = Remote::new(again.remote().expect("the kept ticket while valid")).unwrap();
    assert!(remote.record(&hub.record).is_ok());

    // Another hub: the asset service gone, accounts unaffected (redeeming
    // and tickets work), downloads say the service isn't answering.
    let mut other = Hub::new("down_assets", server::Limits::default());
    other.assets = None;
    let (file, _) = account::redeem(&other.link(), &other.invite("Erin"), None, "test pc").unwrap();
    assert!(account::request_ticket(&file).is_ok(), "accounts unaffected by the asset service");
    let e = Remote::new(Session::new(file, None).remote().unwrap()).unwrap().whoami().unwrap_err();
    assert!(e.to_string().contains("502"), "{e}");
}

impl Hub {
    /// An invite code minted while the service is down can't be: one made
    /// before isn't needed for the refusal, any code shape will do.
    fn invite_offline(&self) -> String {
        "ABC-DEF-GHJ".into()
    }
}

/// A client's own `X-Chimera-Peer` is dropped: the asset service logs and
/// limits the real peer, so forged addresses neither hide a client nor
/// dodge the bad-token ban.
#[test]
fn a_forged_peer_header_is_ignored() {
    let limits = server::Limits { bad_tokens: 3, ..server::Limits::default() };
    let hub = Hub::new("peer", limits);
    let bad = |i: u32| get("/whoami", &format!("Authorization: Bearer chimera1_nope{i}\r\nX-Chimera-Peer: 203.0.113.{i}:4000\r\n"));
    let answers = hub.raw(&[bad(1), bad(2), bad(3), bad(4)]);
    let statuses: Vec<u16> = answers.iter().map(|a| a.0).collect();
    assert_eq!(statuses, [401, 401, 401, 429], "{answers:?}");
    let log = hub.access_log();
    assert!(!log.contains("203.0.113"), "a forged address reached the log:\n{log}");
    assert!(log.lines().filter(|l| l.contains("GET /whoami") && l.contains("ip=127.0.0.1")).count() >= 3, "{log}");
    // The accounts service too: its log names the real peer.
    let _ = hub.raw(&[format!("POST /accounts/redeem HTTP/1.1\r\nHost: hub\r\nX-Chimera-Peer: 203.0.113.9:1\r\nContent-Length: 2\r\n\r\n{{}}")]);
    let log = fs::read_to_string(hub.paths.access_log()).unwrap();
    assert!(log.contains("POST /accounts/redeem") && !log.contains("203.0.113"), "{log}");
}

/// What exists on this machine only is never relayed, in any spelling;
/// a relayed request (`X-Chimera-Peer` on a loopback connection) isn't
/// "this machine" to the services either; another service's path on a
/// connection is 421; a chunked body is 411.
#[test]
fn admin_paths_are_refused_from_outside() {
    let hub = Hub::new("admin", server::Limits::default());
    let secret = fs::read_to_string(hub.paths.secret()).unwrap();
    let with_secret = format!("X-Chimera-Service: {}\r\n", secret.trim());
    for path in ["/admin/", "/admin/invites", "//admin/accounts", "/ADMIN/", "/internal/revoked?since=0", "/feedback/admin/", "/Feedback/Admin/list"] {
        let a = hub.raw(&[get(path, &with_secret)]);
        assert_eq!(a[0].0, 404, "{path}: {a:?}");
    }
    // Through a path the front does relay, the service still refuses.
    assert_eq!(hub.raw(&[get("/accounts/../admin/invites", &with_secret)])[0].0, 404);
    // The front's own health: this machine's peer only (here: loopback).
    let health = hub.raw(&[get("/health", "")]);
    assert!(health[0].0 == 200 && health[0].1.starts_with("ok "), "{health:?}");

    // Directly on loopback with a peer header: relayed, so not local.
    let direct = |path: &str, extra: &str| -> u16 {
        let mut s = TcpStream::connect(hub.accounts_addr).unwrap();
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n{extra}\r\n").as_bytes()).unwrap();
        read_answer(&mut s).unwrap().0
    };
    assert_eq!(direct("/admin/invites", &with_secret), 200, "this machine with the secret");
    assert_eq!(direct("/admin/invites", &format!("{with_secret}X-Chimera-Peer: 127.0.0.1:9\r\n")), 404);
    assert_eq!(direct("/health", "X-Chimera-Peer: 127.0.0.1:9\r\n"), 404);
    assert_eq!(direct("/admin/", "X-Chimera-Peer: 127.0.0.1:9\r\n"), 404, "the page too");
    assert_eq!(direct("/admin/", ""), 200, "the page from this machine");

    // One connection, one service: the second request is another's.
    let a = hub.raw(&[get("/key", ""), get("/accounts/me", "")]);
    assert_eq!((a[0].0, a[1].0), (200, 421), "{a:?}");
    // The asset service answers 421 to the other services' paths too.
    let mut s = TcpStream::connect(hub.assets.as_ref().unwrap().addr).unwrap();
    s.write_all(b"GET /accounts/me HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").unwrap();
    assert_eq!(read_answer(&mut s).unwrap().0, 421);
    // A chunked body.
    let chunked = "POST /batch HTTP/1.1\r\nHost: hub\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".to_string();
    assert_eq!(hub.raw(&[chunked])[0].0, 411);
}

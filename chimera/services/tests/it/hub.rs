//! The hub service (docs/plans/active/hub.md, section 4, S2) on free
//! loopback ports over a scratch store, beside the accounts service (its
//! tickets and revocation feed), the asset service and the front, as on
//! the host: the signed document (it verifies with `/hub/key`, the asset
//! service's manifest key; a tampered body doesn't), no ticket no
//! answer, heartbeats (401 without a ticket, 403 without the `list`
//! right, 429 past it), the listed address the peer's (a forged
//! `X-Chimera-Peer` through the front ignored; this machine's at
//! `public_address`), the probe against a real `Responder` and a closed
//! port (hidden from others, shown to its own account with a hint),
//! expiry, leaving, a revoked account's listing dropped, a restart
//! keeping the list, the owner's list and delist (loopback and the
//! services' secret only); parties (`hub::party`): made, joined by code,
//! led, followed, left, kicked, size and wrong-code limits, a restart,
//! the lead handed on and expiry.

use std::{
    fs,
    net::{SocketAddr, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use chimera_assets::{
    Store,
    account::{self, HubLink},
    hub_doc::{self, Heartbeat, Reach},
    server,
    tls::{FrontLimits, Identity},
};
use chimera_formats::a2s::{GAME_FOLDER, PROTOCOL, Responder, ServerInfo};
use chimera_services::{
    accounts::{self, Accounts, Clock, InviteSpec, Invited},
    admin::Admin,
    front::{self, Routes},
    hub::{self, Hub},
};

use super::scratch;

const FEED: Duration = Duration::from_millis(150);

struct Services {
    store: Store,
    clock: Clock,
    accounts: Arc<Accounts>,
    accounts_run: accounts::http::Running,
    hub_run: Option<hub::http::Running>,
    _assets: server::Running,
    front: front::Front,
    fingerprint: String,
    admin: Admin,
}

impl Services {
    fn new(name: &str) -> Self {
        let dir = scratch(&format!("hub_{name}"));
        let store = Store::new(dir.join("server-store"));
        let clock = Clock::default();
        let accounts = Arc::new(
            Accounts::open(accounts::Paths::new(store.root.clone()), accounts::Options::default(), clock.clone()).unwrap(),
        );
        let accounts_run = accounts::http::serve(accounts::http::Http::new(accounts.clone(), None), "127.0.0.1:0", 2, None).unwrap();
        let service = server::Service::with_tickets(store.clone(), &accounts_run.url(), FEED).unwrap();
        let assets = server::serve(service, "127.0.0.1:0", 2).unwrap();
        let hub_run = Self::start_hub(&store, &clock, &accounts_run.url(), "127.0.0.1:0");
        let identity = Identity::from_seed(&[21; 32], [22; 16]);
        let routes = Routes { assets: assets.addr, accounts: accounts_run.addr, hub: hub_run.addr, feedback: assets.addr };
        let front = front::serve("127.0.0.1:0", &identity, routes, FrontLimits::default()).unwrap();
        let secret = fs::read_to_string(accounts.paths.secret()).unwrap();
        let admin = Admin::new(&accounts_run.url(), secret.trim());
        Self {
            store,
            clock,
            accounts,
            accounts_run,
            hub_run: Some(hub_run),
            _assets: assets,
            front,
            fingerprint: identity.fingerprint(),
            admin,
        }
    }

    fn start_hub(store: &Store, clock: &Clock, accounts_url: &str, listen: &str) -> hub::http::Running {
        let options = hub::Options { feed_every: FEED, probe_wait: Duration::from_millis(600), ..Default::default() };
        let h = Hub::open(hub::Paths::new(store.root.clone()), options, clock.clone(), Some(accounts_url.to_string())).unwrap();
        hub::http::serve(hub::http::Http::new(h), listen, 2).unwrap()
    }

    fn hub(&self) -> &Arc<Hub> {
        &self.hub_run.as_ref().unwrap().http.hub
    }

    /// The hub straight on loopback (this machine).
    fn hub_url(&self) -> String {
        self.hub_run.as_ref().unwrap().url()
    }

    /// A signed-in account's ticket (`role`, `list` slots).
    fn ticket(&self, name: &str, role: &str, list: Option<u32>) -> String {
        let invited: Invited = self
            .admin
            .post("/admin/invite", &InviteSpec { name: name.into(), role: Some(role.into()), list, ..Default::default() })
            .unwrap();
        let link = HubLink { host: self.accounts_run.addr.to_string(), fingerprint: None, code: None };
        let (file, _) = account::redeem(&link, &invited.code, Some(name), "test").unwrap();
        file.ticket.unwrap()
    }

    fn write_config(&self, text: &str) {
        let path = self.hub().paths.config();
        fs::write(&path, text).unwrap();
    }
}

fn beat(instance: &str, port: u16) -> Heartbeat {
    Heartbeat {
        instance: instance.into(),
        port,
        lan: vec!["192.168.1.20:27031".into()],
        name: "Test server".into(),
        game: "ultimate_gun_game".into(),
        minigame: Some("ultimate_gun_game:auto".into()),
        map: "de_dust2".into(),
        players: 2,
        max: 8,
        bots: 1,
        password: false,
        dedicated: true,
        net: "0.1.0/net54".into(),
    }
}

/// A game server's query port: a real `Responder` on a thread.
struct Answering {
    port: u16,
    stop: Arc<AtomicBool>,
}

impl Answering {
    fn new() -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        let port = socket.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        std::thread::spawn(move || {
            let mut r = Responder::new(99);
            let info = ServerInfo {
                protocol: PROTOCOL,
                name: "Test server".into(),
                folder: GAME_FOLDER.into(),
                version: "0.1.0/net54".into(),
                port: Some(port),
                ..Default::default()
            };
            let started = std::time::Instant::now();
            let mut buf = [0u8; 1400];
            while !s.load(Ordering::Relaxed) {
                if let Ok((n, from)) = socket.recv_from(&mut buf)
                    && let Some(reply) = r.handle(from, &buf[..n], started.elapsed(), &info)
                {
                    let _ = socket.send_to(&reply, from);
                }
            }
        });
        Self { port, stop }
    }
}

impl Drop for Answering {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// A UDP port nothing answers on.
fn closed_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    for _ in 0..200 {
        if ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out: {what}");
}

#[test]
fn the_document_is_signed_with_the_asset_services_key_and_needs_a_ticket() {
    let s = Services::new("document");
    s.write_config("name = \"Test hub\"\nmotd = \"Playtest at 13:00\"\nmin_net = 54\ndownload = \"https://example.invalid/r\"\n[[news]]\nid = \"a\"\ndate = \"2026-11-14\"\ntitle = \"New maps\"\n");
    let ticket = s.ticket("reader", "tester", Some(0));
    // Through the front, as a player: the key is the asset service's.
    let origin = s.front.url();
    let key = hub_doc::fetch_key(&origin, Some(&s.fingerprint)).unwrap();
    let served = chimera_assets::Remote::new(chimera_assets::RemoteConfig {
        url: origin.clone(),
        token: ticket.clone(),
        key: None,
        fingerprint: Some(s.fingerprint.clone()),
    })
    .unwrap()
    .server_key()
    .unwrap();
    assert_eq!(key, served, "the hub signs with the asset service's manifest key (`GET /key`)");
    let got = hub_doc::fetch_document(&origin, Some(&s.fingerprint), &ticket, None).unwrap();
    assert_eq!(got.key, key);
    let d = &got.doc;
    assert_eq!((d.name.as_str(), d.motd.as_str(), d.client.min_net, d.news.len()), ("Test hub", "Playtest at 13:00", 54, 1));
    assert_eq!(d.assets.fingerprint.as_deref(), None, "no certificate in this store's tls folder");
    assert_eq!(d.assets.key, key);
    assert_eq!(d.accounts.ticket_keys.len(), 1);
    assert!(d.too_old("0.1.0/net53") && !d.too_old("0.1.0/net54"));
    // A tampered body doesn't verify; another key neither.
    let mut bad = got.body.clone();
    let at = bad.windows(8).position(|w| w == b"Test hub").unwrap();
    bad[at] = b'B';
    assert!(hub_doc::verify(&bad, &got.signature, &key).is_err());
    let other = chimera_assets::sign::public_hex(&ed25519_dalek::SigningKey::from_bytes(&[3; 32]));
    assert!(hub_doc::fetch_document(&origin, Some(&s.fingerprint), &ticket, Some(&other)).is_err());
    // Edited settings show at the next fetch, no restart.
    s.write_config("name = \"Renamed hub\"\nmotd = \"\"\n");
    let again = hub_doc::fetch_document(&origin, Some(&s.fingerprint), &ticket, Some(&key)).unwrap();
    assert_eq!(again.doc.name, "Renamed hub");
    // No ticket, no document and no list (decision 10).
    for path in ["/hub", "/hub/servers"] {
        let err = hub_doc::fetch_servers(&format!("{origin}{path}"), Some(&s.fingerprint), "chimera2t.forged.x").unwrap_err();
        assert_eq!(err.status, Some(401), "{path}: {err}");
    }
    let agent = chimera_assets::remote::agent(Some(&s.fingerprint)).unwrap();
    let r = agent.get(&format!("{origin}/hub/servers")).call().unwrap();
    assert_eq!(r.status().as_u16(), 401, "no ticket at all");
}

#[test]
fn heartbeats_need_the_list_right_and_stay_within_it() {
    let s = Services::new("rights");
    let url = format!("{}/hub/heartbeat", s.hub_url());
    let none = hub_doc::heartbeat(&url, None, "chimera2t.forged.x", &beat("00000000000000a1", 27031)).unwrap_err();
    assert_eq!(none.status, Some(401));
    let reader = s.ticket("reader", "tester", Some(0));
    let no = hub_doc::heartbeat(&url, None, &reader, &beat("00000000000000a1", 27031)).unwrap_err();
    assert_eq!(no.status, Some(403), "{no}");
    let tester = s.ticket("tester", "tester", None);
    hub_doc::heartbeat(&url, None, &tester, &beat("00000000000000a1", 27031)).unwrap();
    let second = hub_doc::heartbeat(&url, None, &tester, &beat("00000000000000a2", 27032)).unwrap_err();
    assert_eq!(second.status, Some(429), "a tester lists one server: {second}");
    // The same again at once: too soon; changed: fine a second later.
    let again = hub_doc::heartbeat(&url, None, &tester, &beat("00000000000000a1", 27031)).unwrap_err();
    assert_eq!(again.status, Some(429));
    s.clock.advance(2);
    let mut changed = beat("00000000000000a1", 27031);
    changed.players = 5;
    hub_doc::heartbeat(&url, None, &tester, &changed).unwrap();
    // Another account can't take the instance over.
    let host = s.ticket("hoster", "host", None);
    s.clock.advance(20);
    let theirs = hub_doc::heartbeat(&url, None, &host, &beat("00000000000000a1", 27031)).unwrap_err();
    assert_eq!(theirs.status, Some(409));
    // Malformed text is refused.
    let mut bad = beat("00000000000000a3", 27033);
    bad.name = "a\u{1}b".into();
    assert_eq!(hub_doc::heartbeat(&url, None, &host, &bad).unwrap_err().status, Some(400));
}

#[test]
fn the_listed_address_is_the_peers_and_a_forged_header_is_ignored() {
    let s = Services::new("address");
    let host = s.ticket("hoster", "host", None);
    let up = Answering::new();
    // Through the front from this machine, a forged header in the request:
    // the front replaces it with the real peer (loopback).
    let origin = s.front.url();
    let agent = chimera_assets::remote::agent(Some(&s.fingerprint)).unwrap();
    let r = agent
        .post(&format!("{origin}/hub/heartbeat"))
        .header("Authorization", &format!("Bearer {host}"))
        .header("X-Chimera-Peer", "203.0.113.77:1234")
        .header("Content-Type", "application/json")
        .send(&serde_json::to_vec(&beat("00000000000000b1", up.port)).unwrap()[..])
        .unwrap();
    assert_eq!(r.status().as_u16(), 200);
    let answer: hub_doc::HeartbeatAnswer = serde_json::from_reader(r.into_body().into_reader()).unwrap();
    assert_eq!(answer.address, format!("127.0.0.1:{}", up.port), "the real peer, not the forged one");
    assert_eq!(answer.every, 30);
    // A forged header straight to the hub from elsewhere can't happen (it
    // listens on loopback); this machine's own header is believed.
    // This machine's servers are listed at `public_address`.
    s.write_config("public_address = \"203.0.113.5\"\n");
    let b2 = beat("00000000000000b2", up.port);
    let host2 = s.ticket("hoster2", "host", None);
    let a2 = hub_doc::heartbeat(&format!("{}/hub/heartbeat", s.hub_url()), None, &host2, &b2).unwrap();
    assert_eq!(a2.address, format!("203.0.113.5:{}", up.port));
    let list = hub_doc::fetch_servers(&format!("{}/hub/servers", s.hub_url()), None, &host).unwrap();
    let l = list.servers.iter().find(|l| l.instance == "00000000000000b2").unwrap();
    assert_eq!(l.lan, vec!["192.168.1.20:27031".to_string()]);
    assert!(!l.mine);
    // The probe asks the address the heartbeat came from and no other:
    // not the public address it is listed at (nothing of ours answers
    // there), not the LAN address its body names. It reaches the server.
    wait_until("the probe", || {
        hub_doc::fetch_servers(&format!("{}/hub/servers", s.hub_url()), None, &host2)
            .unwrap()
            .servers
            .iter()
            .any(|l| l.instance == "00000000000000b2" && l.reachable == Reach::Yes)
    });
    let seen = s.hub().admin_list().into_iter().find(|l| l.listing.instance == "00000000000000b2").unwrap();
    assert_eq!(seen.seen, format!("127.0.0.1:{}", up.port));
    assert_eq!(seen.listing.addr, format!("203.0.113.5:{}", up.port));
}

#[test]
fn the_owner_lists_everything_and_delists() {
    use chimera_services::hub::{AdminListing, Delist, Delisted};
    let s = Services::new("delist");
    let url = s.hub_url();
    let host = s.ticket("hoster", "host", None);
    let reader = s.ticket("reader", "tester", Some(0));
    let up = Answering::new();
    let closed = closed_port();
    let beat_url = format!("{url}/hub/heartbeat");
    hub_doc::heartbeat(&beat_url, None, &host, &beat("00000000000000f1", up.port)).unwrap();
    hub_doc::heartbeat(&beat_url, None, &host, &beat("00000000000000f2", closed)).unwrap();
    let count = |t: &str| hub_doc::fetch_servers(&format!("{url}/hub/servers"), None, t).unwrap().servers.len();
    wait_until("the closed one hidden", || count(&reader) == 1);
    // The owner's calls: this machine, with the services' secret.
    let secret = fs::read_to_string(s.accounts.paths.secret()).unwrap();
    let owner = Admin::new(&url, secret.trim());
    let all: Vec<AdminListing> = owner.get("/admin/servers").unwrap();
    assert_eq!(all.len(), 2, "the unreachable one too: {all:?}");
    assert!(all.iter().all(|l| l.account.len() == 32));
    assert!(all.iter().any(|l| l.listing.instance == "00000000000000f2" && l.listing.reachable == Reach::No));
    // Without the secret, or relayed by the front: no.
    let stranger = Admin::new(&url, "not-the-secret");
    assert!(stranger.get::<Vec<AdminListing>>("/admin/servers").unwrap_err().contains("401"));
    let agent = chimera_assets::remote::agent(Some(&s.fingerprint)).unwrap();
    let relayed = agent.get(&format!("{}/hub/../admin/servers", s.front.url())).header("X-Chimera-Service", secret.trim()).call().unwrap();
    assert_ne!(relayed.status().as_u16(), 200, "never through the front");
    // Delisted: gone at once, and its next heartbeats are refused.
    let d: Delisted = owner.post("/admin/delist", &Delist { instance: "00000000000000F1".into(), secs: Some(600) }).unwrap();
    assert!(d.was_listed);
    assert_eq!(count(&reader), 0);
    s.clock.advance(31);
    let refused = hub_doc::heartbeat(&beat_url, None, &host, &beat("00000000000000f1", up.port)).unwrap_err();
    assert_eq!(refused.status, Some(403), "{refused}");
    assert!(refused.message.contains("off the list"), "{refused}");
    // After the time given it lists again.
    s.clock.advance(600);
    hub_doc::heartbeat(&beat_url, None, &host, &beat("00000000000000f1", up.port)).unwrap();
    wait_until("listed again", || count(&reader) == 1);
    // One that isn't listed: said so, still refused.
    let d: Delisted = owner.post("/admin/delist", &Delist { instance: "00000000000000aa".into(), secs: None }).unwrap();
    assert!(!d.was_listed);
}

#[test]
fn the_probe_hides_an_unreachable_server_from_others() {
    let s = Services::new("probe");
    let url = s.hub_url();
    let host = s.ticket("hoster", "host", None);
    let other = s.ticket("other", "tester", Some(0));
    let up = Answering::new();
    let closed = closed_port();
    hub_doc::heartbeat(&format!("{url}/hub/heartbeat"), None, &host, &beat("00000000000000c1", up.port)).unwrap();
    let first = hub_doc::heartbeat(&format!("{url}/hub/heartbeat"), None, &host, &beat("00000000000000c2", closed)).unwrap();
    assert_eq!(first.reachable, Reach::Unknown, "not probed yet");
    let reach = |ticket: &str, i: &str| {
        hub_doc::fetch_servers(&format!("{url}/hub/servers"), None, ticket).unwrap().servers.into_iter().find(|l| l.instance == i)
    };
    wait_until("both probed", || {
        reach(&host, "00000000000000c1").is_some_and(|l| l.reachable == Reach::Yes)
            && reach(&host, "00000000000000c2").is_some_and(|l| l.reachable == Reach::No)
    });
    // Others see the reachable one only; its host sees both, with a hint.
    assert!(reach(&other, "00000000000000c1").is_some());
    assert!(reach(&other, "00000000000000c2").is_none(), "hidden from others");
    let mine = reach(&host, "00000000000000c2").unwrap();
    assert!(mine.mine && mine.hint.as_deref().is_some_and(|h| h.contains(&format!("UDP {closed}"))), "{mine:?}");
    // The heartbeat's answer says so too.
    s.clock.advance(31);
    let later = hub_doc::heartbeat(&format!("{url}/hub/heartbeat"), None, &host, &beat("00000000000000c2", closed)).unwrap();
    assert_eq!(later.reachable, Reach::No);
    assert!(later.hint.is_some());
}

#[test]
fn listings_expire_leave_and_go_with_a_revoked_account() {
    let s = Services::new("expiry");
    let url = s.hub_url();
    let host = s.ticket("hoster", "host", None);
    let reader = s.ticket("reader", "tester", Some(0));
    let up = Answering::new();
    let count = |t: &str| hub_doc::fetch_servers(&format!("{url}/hub/servers"), None, t).unwrap().servers.len();
    hub_doc::heartbeat(&format!("{url}/hub/heartbeat"), None, &host, &beat("00000000000000d1", up.port)).unwrap();
    assert_eq!(count(&reader), 1);
    // Two missed beats: still there; the third: gone.
    s.clock.advance(89);
    assert_eq!(count(&reader), 1);
    s.clock.advance(1);
    assert_eq!(count(&reader), 0, "90 s without a heartbeat");
    // Back with the next one; then leaving at once.
    hub_doc::heartbeat(&format!("{url}/hub/heartbeat"), None, &host, &beat("00000000000000d1", up.port)).unwrap();
    assert_eq!(count(&reader), 1);
    hub_doc::leave(&format!("{url}/hub/leave"), None, &reader, "00000000000000d1").unwrap_err();
    hub_doc::leave(&format!("{url}/hub/leave"), None, &host, "00000000000000d1").unwrap();
    assert_eq!(count(&reader), 0);
    // A revoked account's listing goes with the revocation feed.
    s.clock.advance(20);
    hub_doc::heartbeat(&format!("{url}/hub/heartbeat"), None, &host, &beat("00000000000000d2", up.port)).unwrap();
    assert_eq!(count(&reader), 1);
    s.accounts.revoke_account("hoster").unwrap();
    wait_until("the revocation feed", || count(&reader) == 0);
}

#[test]
fn a_hub_restart_keeps_the_list() {
    let mut s = Services::new("restart");
    let host = s.ticket("hoster", "host", None);
    let up = Answering::new();
    hub_doc::heartbeat(&format!("{}/hub/heartbeat", s.hub_url()), None, &host, &beat("00000000000000e1", up.port)).unwrap();
    let addr = s.hub_run.as_ref().unwrap().addr;
    s.hub_run = None;
    // Free the port, then the same one again.
    std::thread::sleep(Duration::from_millis(100));
    s.hub_run = Some(Services::start_hub(&s.store, &s.clock, &s.accounts_run.url(), &addr.to_string()));
    let list = hub_doc::fetch_servers(&format!("{}/hub/servers", s.hub_url()), None, &host).unwrap();
    assert_eq!(list.servers.len(), 1);
    assert_eq!(list.servers[0].addr, SocketAddr::from(([127, 0, 0, 1], up.port)).to_string());
    assert!(s.hub().count() == 1);
}

// --- Parties (docs/plans/active/parties.md).

mod parties {
    use chimera_assets::party_doc::{self, Create, Join, Lead, Me, MoveKind, OnMember, State, Target};

    use super::*;

    fn me(name: &str) -> Me {
        Me { name: name.into(), avatar: 3 }
    }

    fn state(s: &Services, ticket: &str, token: &str, at: Option<&str>) -> Result<party_doc::PartyView, party_doc::Failure> {
        party_doc::state(&s.hub_url(), None, ticket, &State { token: token.into(), me: me("x"), at: at.map(str::to_string) })
    }

    #[test]
    fn a_party_is_made_joined_by_its_code_led_and_left() {
        let s = Services::new("party_flow");
        let origin = s.hub_url();
        let a = s.ticket("alice", "tester", Some(0));
        let b = s.ticket("bob", "tester", Some(0));
        // No ticket, no party.
        assert_eq!(party_doc::create(&origin, None, "chimera2t.forged.x", &Create { me: me("A") }).unwrap_err().status, Some(401));
        let made = party_doc::create(&origin, None, &a, &Create { me: me("Alice") }).unwrap();
        assert!(made.token.starts_with(party_doc::TOKEN_PREFIX));
        assert_eq!(made.party.code.len(), 7, "{}", made.party.code);
        assert_eq!(made.party.id.len(), 12);
        assert!(made.party.leading());
        // Joined by the code, typed loosely.
        let typed = made.party.code.to_lowercase().replace('-', " ");
        let joined = party_doc::join(&origin, None, &b, &Join { code: typed, me: me("Bob") }).unwrap();
        assert_eq!(joined.party.id, made.party.id);
        assert_eq!(joined.party.members.len(), 2);
        assert!(!joined.party.leading());
        assert!(joined.party.members.iter().all(|m| m.online));
        // A member may not lead.
        let lead = |t: &str, target: Option<Target>, together: bool| {
            party_doc::lead(&origin, None, &b, &Lead { token: t.into(), target, together, hosted: false })
        };
        assert_eq!(lead(&joined.token, None, true).unwrap_err().status, Some(403));
        // The leader takes the party to a server: one move.
        let server = Target { addr: "192.0.2.7:27031".into(), name: "Ours".into() };
        let v = party_doc::lead(&origin, None, &a, &Lead { token: made.token.clone(), target: Some(server.clone()), together: false, hosted: false }).unwrap();
        assert_eq!((v.moves, v.kind), (1, MoveKind::Join));
        // Telling it again is no new move.
        let v = party_doc::lead(&origin, None, &a, &Lead { token: made.token.clone(), target: Some(server.clone()), together: false, hosted: false }).unwrap();
        assert_eq!(v.moves, 1);
        let seen = state(&s, &b, &joined.token, Some("192.0.2.7:27031")).unwrap();
        assert_eq!(seen.target.as_ref().map(|t| t.addr.as_str()), Some("192.0.2.7:27031"));
        assert_eq!(seen.me().unwrap().at.as_deref(), Some("192.0.2.7:27031"));
        // Leave with my party: a leave move naming the server left.
        let v = party_doc::lead(&origin, None, &a, &Lead { token: made.token.clone(), target: None, together: true, hosted: false }).unwrap();
        assert_eq!((v.moves, v.kind, v.left.as_deref(), v.target.is_none()), (2, MoveKind::Leave, Some("192.0.2.7:27031"), true));
        // Leave alone: nobody moves.
        let v = party_doc::lead(&origin, None, &a, &Lead { token: made.token.clone(), target: None, together: false, hosted: false }).unwrap();
        assert_eq!(v.moves, 2);
        // Promote, then the old leader is a member.
        let bob = seen.you.clone();
        let v = party_doc::promote(&origin, None, &a, &OnMember { token: made.token.clone(), member: bob.clone() }).unwrap();
        assert_eq!(v.leader().unwrap().id, bob);
        assert_eq!(party_doc::promote(&origin, None, &a, &OnMember { token: made.token.clone(), member: bob.clone() }).unwrap_err().status, Some(403));
        // The leader leaves: the lead passes; the last one out ends it and frees the code.
        party_doc::leave(&origin, None, &b, &joined.token).unwrap();
        let v = state(&s, &a, &made.token, None).unwrap();
        assert!(v.leading());
        assert!(party_doc::gone(&state(&s, &b, &joined.token, None).unwrap_err()));
        party_doc::leave(&origin, None, &a, &made.token).unwrap();
        assert_eq!(s.hub().party_count(), 0);
        let again = party_doc::join(&origin, None, &b, &Join { code: made.party.code.clone(), me: me("Bob") }).unwrap_err();
        assert_eq!(again.status, Some(404));
    }

    #[test]
    fn kicks_sizes_and_wrong_codes_are_limited() {
        let s = Services::new("party_limits");
        let origin = s.hub_url();
        let a = s.ticket("lead", "tester", Some(0));
        let made = party_doc::create(&origin, None, &a, &Create { me: me("Lead") }).unwrap();
        let mut tokens = Vec::new();
        for i in 1..party_doc::MAX_MEMBERS {
            let t = s.ticket(&format!("member{i}"), "tester", Some(0));
            let j = party_doc::join(&origin, None, &t, &Join { code: made.party.code.clone(), me: me(&format!("M{i}")) }).unwrap();
            tokens.push((t, j));
        }
        let late = s.ticket("late", "tester", Some(0));
        assert_eq!(party_doc::join(&origin, None, &late, &Join { code: made.party.code.clone(), me: me("Late") }).unwrap_err().status, Some(409));
        // Kicked: the token is no member's any more.
        let (kt, kj) = &tokens[0];
        let v = party_doc::kick(&origin, None, &a, &OnMember { token: made.token.clone(), member: kj.party.you.clone() }).unwrap();
        assert_eq!(v.members.len(), party_doc::MAX_MEMBERS - 1);
        assert!(party_doc::gone(&state(&s, kt, &kj.token, None).unwrap_err()));
        // A member can't kick.
        let (mt, mj) = &tokens[1];
        assert_eq!(party_doc::kick(&origin, None, mt, &OnMember { token: mj.token.clone(), member: v.you.clone() }).unwrap_err().status, Some(403));
        // Wrong codes: 404 each, then 429 for the window.
        for _ in 0..10 {
            assert_eq!(party_doc::join(&origin, None, &late, &Join { code: "ZZZ-ZZZ".into(), me: me("Late") }).unwrap_err().status, Some(404));
        }
        let blocked = party_doc::join(&origin, None, &late, &Join { code: made.party.code.clone(), me: me("Late") }).unwrap_err();
        assert_eq!(blocked.status, Some(429), "{blocked}");
        s.clock.advance(hub::party::CODE_WINDOW as i64);
        // The window over (and the kicked one's place free): in.
        party_doc::join(&origin, None, &late, &Join { code: made.party.code.clone(), me: me("Late") }).unwrap();
    }

    #[test]
    fn parties_outlast_a_restart_and_expire_when_nobody_comes_back() {
        let mut s = Services::new("party_expiry");
        let origin = s.hub_url();
        let a = s.ticket("lead", "tester", Some(0));
        let b = s.ticket("other", "tester", Some(0));
        let made = party_doc::create(&origin, None, &a, &Create { me: me("Lead") }).unwrap();
        let joined = party_doc::join(&origin, None, &b, &Join { code: made.party.code.clone(), me: me("Other") }).unwrap();
        // The hub restarts (on another port): the party and both tokens are still good.
        s.hub_run = None;
        s.hub_run = Some(Services::start_hub(&s.store, &s.clock, &s.accounts_run.url(), "127.0.0.1:0"));
        assert_eq!(state(&s, &a, &made.token, None).unwrap().members.len(), 2);
        // The leader goes quiet; the other stays: after the handover time the lead passes.
        s.clock.advance(130);
        state(&s, &b, &joined.token, None).unwrap();
        s.hub().sweep();
        let v = state(&s, &b, &joined.token, None).unwrap();
        assert!(v.leading(), "{v:?}");
        assert!(!v.members.iter().find(|m| m.id == made.party.you).unwrap().online);
        // Nobody for six hours: gone.
        s.clock.advance(6 * 3600 + 1);
        s.hub().sweep();
        assert_eq!(s.hub().party_count(), 0);
        // (A fresh ticket: the old ones expired meanwhile.)
        let fresh = s.ticket("later", "tester", Some(0));
        assert!(party_doc::gone(&state(&s, &fresh, &made.token, None).unwrap_err()));
    }

    #[test]
    fn a_hosting_leader_is_found_where_the_hub_sees_it() {
        let s = Services::new("party_hosted");
        s.write_config("public_address = \"198.51.100.4\"\n");
        let a = s.ticket("hosting", "tester", Some(0));
        let made = party_doc::create(&s.hub_url(), None, &a, &Create { me: me("Host") }).unwrap();
        let lead = Lead { token: made.token.clone(), target: Some(Target { addr: "10.9.9.9:27040".into(), name: "My game".into() }), together: false, hosted: true };
        // From this machine: hub.toml's public address, the body's port only.
        let v = party_doc::lead(&s.hub_url(), None, &a, &lead).unwrap();
        assert_eq!(v.target.unwrap().addr, "198.51.100.4:27040");
    }
}
